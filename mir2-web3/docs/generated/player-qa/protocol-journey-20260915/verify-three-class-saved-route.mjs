#!/usr/bin/env node
import fs from 'node:fs';
import readline from 'node:readline';
import crypto from 'node:crypto';
import path from 'node:path';

const ROOT = process.env.MIR2_PROTOCOL_ROOT ?? 'C:/mir2-protocol-journey-20260911';
const CONFIG = process.env.MIR2_NEWCOMER_CONFIG ?? path.resolve('config/quest-guidance/newcomer-journey-v1.json');
const EXPECTED = [
  { className: 'Warrior', name: 'JWa3693f66', index: 8 },
  { className: 'Wizard', name: 'JWdebac6d4', index: 9 },
  { className: 'Taoist', name: 'JT3f4a4e1f', index: 10 },
];
const MILESTONES = [2100015, 2100020, 2100025, 2100030];
const safeRead = path => JSON.parse(fs.readFileSync(path, 'utf8'));
const sha = path => crypto.createHash('sha256').update(fs.readFileSync(path)).digest('hex');
const stateList = raw => {
  const normalize = values => values.flatMap(value => {
    if (typeof value !== 'string') return [value];
    try { return [JSON.parse(value)]; } catch { return []; }
  });
  if (Array.isArray(raw)) return normalize(raw);
  if (raw && typeof raw === 'object') return normalize(Object.values(raw));
  if (typeof raw !== 'string' || !raw.trim()) return [];
  try { return JSON.parse(raw); } catch { return JSON.parse(`[${raw}]`); }
};
const completedIds = entries => new Set(entries.filter(q => String(q?.stage).toLowerCase() === 'completed').map(q => Number(q.quest_id ?? q.questId)).filter(Number.isFinite));
const selfOf = snap => (snap?.entities ?? []).find(e => Number(e.objectId) === Number(snap?.playerObjectId));
const selected = snap => { const self = selfOf(snap); return snap ? { level: self?.level ?? null, map: snap.mapFileName ?? null, x: self?.x ?? null, y: self?.y ?? null, hp: snap.playerHp ?? self?.hp ?? null, mp: snap.playerMp ?? null } : null; };

async function finalTrace(path, report, expected) {
  if (!path || !fs.existsSync(path)) return { eligible: false, reason: 'trace-missing' };
  const input = fs.createReadStream(path, 'utf8');
  const rl = readline.createInterface({ input, crlfDelay: Infinity });
  let latestOwner = null; let logoutRequest = null; let logout = null;
  for await (const line of rl) {
    let e; try { e = JSON.parse(line); } catch { continue; }
    if ((e?.type === 'logOut' && e?.direction === 'sent') ||
        (e?.direction === 'sent' && e?.packet === 'LogOut')) logoutRequest = e;
    if (e?.type === 'worldSnapshot' && !logoutRequest) {
      const self = selfOf(e.payload);
      const ownerId = Number(e.payload?.playerObjectId);
      if (self && Number.isFinite(ownerId) && Number(self.objectId) === ownerId) latestOwner = e;
    }
    if (e?.direction === 'received' && e?.packet === 'LogOutSuccess' && logoutRequest) logout = e;
  }
  const eligible = Boolean(logout && report?.completed === true && (!report.finishedAt || Date.parse(logout.at) <= Date.parse(report.finishedAt) + 5000));
  return { eligible, reason: eligible ? 'completed-with-public-LogOutSuccess' : (logout ? 'report-not-final-completed' : 'public-LogOutSuccess-missing'), snapshot: latestOwner?.payload ?? null, logoutRequestAt: logoutRequest?.at ?? null, logoutAt: logout?.at ?? null };
}

const config = safeRead(CONFIG);
const mandatoryByClass = Object.fromEntries(EXPECTED.map(({ className }) => [className, [...new Set(config.chapters.flatMap(ch => [...(ch.questIds ?? []), ...(ch.classQuestIds?.[className] ?? [])]))].map(Number)]));
const storePath = `${ROOT}/gateway/accounts.json`;
const store = safeRead(storePath);
const output = { gate: 'FAIL_UNTIL_FINAL_PROOF', denominator: { mandatory: 55, milestones: 4, total: 59 }, classes: [], sourceFileHash: { config: sha(CONFIG), store: sha(storePath) } };

for (const expected of EXPECTED) {
  const reportPath = `${ROOT}/${expected.className}.report.json`;
  const snapshotPath = `${ROOT}/${expected.className}.snapshot.json`;
  const report = fs.existsSync(reportPath) ? safeRead(reportPath) : {};
  const snapshot = fs.existsSync(snapshotPath) ? safeRead(snapshotPath) : null;
  const tracePath = report.traceFile ?? `${ROOT}/${expected.className}.trace.jsonl`;
  const accountEntry = Object.entries(store.accounts ?? {}).find(([, account]) => (account.characters ?? []).some(c => c.name === expected.name && Number(c.index) === expected.index));
  const accountId = accountEntry?.[0] ?? null;
  const character = accountEntry?.[1]?.characters?.find(c => c.name === expected.name && Number(c.index) === expected.index) ?? null;
  const save = accountEntry?.[1]?.saves?.[String(expected.index)] ?? null;
  const savedQuest = stateList(save?.quest_states_json);
  const savedDone = completedIds(savedQuest);
  const savedMilestones = MILESTONES.filter(id => savedDone.has(id));
  const trace = await finalTrace(tracePath, report, expected);
  const final = trace.eligible ? trace.snapshot : null;
  const finalQuest = stateList(final?.questLog);
  const finalDone = completedIds(finalQuest);
  const required = mandatoryByClass[expected.className] ?? [];
  const requiredWithMilestones = [...required, ...MILESTONES];
  const savedMissing = requiredWithMilestones.filter(id => !savedDone.has(id));
  const finalMissing = final ? requiredWithMilestones.filter(id => !finalDone.has(id)) : requiredWithMilestones;
  const savedSafe = save ? { revision: save.revision ?? null, level: save.character?.level ?? null, map: save.map_file_name ?? null, x: save.position?.x ?? null, y: save.position?.y ?? null, hp: save.hp ?? null, mp: save.mp ?? null, mandatoryCompleted: required.filter(id => savedDone.has(id)).length, milestoneCompleted: savedMilestones.length } : null;
  const finalSelf = final ? selfOf(final) : null;
  const finalSafe = final ? { ...selected(final), ownerId: finalSelf?.objectId ?? null, name: finalSelf?.name ?? null, className: finalSelf?.class ?? null, dead: finalSelf?.dead ?? null, mandatoryCompleted: required.filter(id => finalDone.has(id)).length, milestoneCompleted: MILESTONES.filter(id => finalDone.has(id)).length } : null;
  const mismatches = [];
  const proofFields = ['level','map','x','y','hp','mp'];
  const known = value => value !== null && value !== undefined && value !== '' && (typeof value !== 'number' || Number.isFinite(value));
  if (final) for (const key of proofFields) { const sv = savedSafe?.[key], fv = finalSafe?.[key]; if (!known(sv) || !known(fv)) mismatches.push(`${key}:missing-proof-field`); else if (String(sv) !== String(fv)) mismatches.push(`${key}:${sv}!=${fv}`); }
  if (report.className !== expected.className) mismatches.push(`report.className:${report.className ?? 'missing'}!=${expected.className}`);
  if (report.name !== expected.name) mismatches.push(`report.name:${report.name ?? 'missing'}!=${expected.name}`);
  if (Number(report.characterIndex) !== expected.index) mismatches.push(`report.characterIndex:${report.characterIndex ?? 'missing'}!=${expected.index}`);
  if (!Number.isFinite(Number(finalSafe?.ownerId))) mismatches.push('final.ownerId:missing-proof-field');
  if (finalSafe?.name !== expected.name) mismatches.push(`final.name:${finalSafe?.name ?? 'missing'}!=${expected.name}`);
  if (finalSafe?.className !== expected.className) mismatches.push(`final.className:${finalSafe?.className ?? 'missing'}!=${expected.className}`);
  if (finalSafe?.dead !== false) mismatches.push(`final.dead:${String(finalSafe?.dead)}!=false`);
  if (!(Number.isFinite(Number(finalSafe?.hp)) && Number(finalSafe.hp) > 0)) mismatches.push('final.hp:not-positive-authoritative');
  output.sourceFileHash[expected.className] = {};
  for (const path of [reportPath, snapshotPath, tracePath]) if (fs.existsSync(path)) output.sourceFileHash[expected.className][path.split(/[\\/]/).pop()] = sha(path);
  output.classes.push({ className: expected.className, character: { name: expected.name, index: expected.index, accountId }, requiredIdCount: required.length, report: { completed: report.completed === true, error: report.error ?? null, finishedAt: report.finishedAt ?? null, finalEvidence: trace.reason }, saved: savedSafe, savedMissingMandatoryOrMilestoneIds: savedMissing, final: finalSafe, finalMissingMandatoryOrMilestoneIds: finalMissing, mismatches });
}
const validClass = c => c.requiredIdCount === 55 && c.report.finalEvidence.startsWith('completed-with-public-LogOutSuccess') && c.savedMissingMandatoryOrMilestoneIds.length === 0 && c.finalMissingMandatoryOrMilestoneIds.length === 0 && c.mismatches.length === 0 && Number.isFinite(c.saved?.level) && c.saved.level >= 30 && Number.isFinite(c.final?.level) && c.final.level >= 30;
if (output.classes.every(validClass)) output.gate = 'PASS';
else output.gate = 'FAIL_UNTIL_FINAL_PROOF';
console.log(JSON.stringify(output, null, 2));
if (output.gate !== 'PASS') process.exitCode = 2;

