import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import readline from 'node:readline';
import path from 'node:path';

// Read-only audit of ordinary runner receipts against the actual saved character.
// The account store is private input; only selected game state is exported.
const [evidenceRoot, accountStorePath] = process.argv.slice(2);
if (!evidenceRoot || !accountStorePath) {
  throw new Error('Usage: node verify-newcomer-v2-evidence.mjs <evidence-root> <accounts.json>');
}
const config = JSON.parse(await fs.readFile(new URL('../../../../config/quest-guidance/newcomer-journey-v2.json', import.meta.url), 'utf8'));
const expectedIds = [...config.quests, ...config.growthRewards].map(row => Number(row.id));
if (expectedIds.length !== 26 || new Set(expectedIds).size !== 26) throw new Error('Invalid V2 runtime contract');
const accounts = JSON.parse(await fs.readFile(accountStorePath, 'utf8')).accounts;
const normalize = value => String(value ?? '').replace(/[^a-z]/gi, '').toLowerCase();
const results = [];
for (const className of ['Warrior', 'Wizard', 'Taoist']) {
  const directory = path.join(evidenceRoot, className.toLowerCase());
  let report;
  try { report = JSON.parse(await fs.readFile(path.join(directory, `${className}.report.json`), 'utf8')); }
  catch (error) { if (error.code === 'ENOENT') { results.push({ className, status: 'no-report', completedUnits: 0 }); continue; } throw error; }
  if (report.v2?.profile !== 'newcomer-v2') throw new Error(`${className}: wrong report cadence`);
  let snapshot;
  try { snapshot = JSON.parse(await fs.readFile(path.join(directory, `${className}.snapshot.json`), 'utf8')); }
  catch (error) { if (error.code === 'ENOENT') { results.push({ className, status: 'running-no-final-snapshot', completedUnits: 0 }); continue; } throw error; }
  const actor = snapshot.entities?.find(row => Number(row.objectId) === Number(snapshot.playerObjectId));
  const account = Object.values(accounts).find(row => row.characters?.some(character => character.name === report.name && Number(character.index) === Number(report.characterIndex)));
  const save = account?.saves?.[String(report.characterIndex)];
  const savedQuests = !save ? [] : Array.isArray(save.quest_states_json)
    ? save.quest_states_json.map(row => typeof row === 'string' ? JSON.parse(row) : row)
    : JSON.parse(save.quest_states_json);
  const snapshotCompleted = new Set((snapshot.questLog ?? []).filter(row => normalize(row.stage) === 'completed').map(row => Number(row.questId)));
  const savedCompleted = new Set(savedQuests.filter(row => normalize(row.stage) === 'completed').map(row => Number(row.quest_id)));
  let logoutSent = null;
  let logoutSuccess = null;
  let firstOwnedAttack = null;
  let firstOwnedStruck = null;
  let firstLearnedSkill = null;
  const lines = readline.createInterface({ input: createReadStream(report.traceFile), crlfDelay: Infinity });
  for await (const line of lines) {
    if (!line) continue;
    const event = JSON.parse(line);
    if (event.direction === 'sent' && event.type === 'logOut') logoutSent = event;
    if (event.direction !== 'received') continue;
    if (event.packet === 'LogOutSuccess' && logoutSent && event.sequence > logoutSent.sequence) logoutSuccess = event;
    if (event.packet === 'ObjectAttack' && Number(event.payload?.objectId) === Number(actor?.objectId)) firstOwnedAttack ??= event;
    if (event.packet === 'ObjectStruck' && Number(event.payload?.attackerId) === Number(actor?.objectId)) firstOwnedStruck ??= event;
    if (event.packet === 'NewMagic') firstLearnedSkill ??= event;
  }
  const transformMatches = !!actor && !!save && actor.name === report.name && save.character?.name === report.name &&
    Number(actor.level) === Number(save.character?.level) && String(snapshot.mapFileName) === String(save.map_file_name) &&
    Number(actor.x) === Number(save.position?.x) && Number(actor.y) === Number(save.position?.y) &&
    actor.direction === save.direction && Number(snapshot.gold) === Number(save.gold) &&
    Number(snapshot.playerExperience) === Number(save.experience);
  const persistedCompletedIds = expectedIds.filter(id => snapshotCompleted.has(id) && savedCompleted.has(id));
  const verifiedCompletedIds = logoutSuccess && transformMatches ? persistedCompletedIds : [];
  const ordinaryStartedAt = report.v2.ordinaryStartedAt;
  const sinceStart = event => event ? Date.parse(event.at) - Date.parse(ordinaryStartedAt) : null;
  results.push({
    className, name: report.name, runId: report.runId, status: report.v2.status,
    level: actor?.level ?? null, completedUnits: verifiedCompletedIds.length,
    snapshotCompletedIds: expectedIds.filter(id => snapshotCompleted.has(id)),
    savedCompletedIds: expectedIds.filter(id => savedCompleted.has(id)),
    verifiedCompletedIds, logoutCommitted: !!logoutSuccess, transformMatches,
    saveRevision: save?.revision ?? null, ordinaryStartedAt,
    ordinaryElapsedMs: report.ordinaryElapsedMs, pause: report.v2.pause ?? null,
    actionTimingScope: 'latest-resume-trace-only',
    firstOwnedAttackMs: sinceStart(firstOwnedAttack), firstOwnedStruckMs: sinceStart(firstOwnedStruck),
    firstLearnedSkillMs: sinceStart(firstLearnedSkill),
    completed: verifiedCompletedIds.length === 26 && Number(actor?.level) >= 30 && report.completed === true,
    visualAccepted: false,
  });
}
const output = { schema: 1, profile: 'newcomer-v2', checkedAt: new Date().toISOString(), denominator: 78,
  completedUnits: results.reduce((sum, row) => sum + row.completedUnits, 0),
  allClassesCompleted: results.length === 3 && results.every(row => row.completed === true),
  measuredTime: false, visualAccepted: false, globalParityPercent: null, classes: results };
await fs.writeFile(path.join(evidenceRoot, 'ordinary-evidence.json'), JSON.stringify(output, null, 2) + '\n');
console.log(JSON.stringify(output, null, 2));
