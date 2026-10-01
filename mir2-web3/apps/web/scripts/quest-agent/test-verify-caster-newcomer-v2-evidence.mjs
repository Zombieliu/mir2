import assert from 'node:assert/strict';
import test from 'node:test';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { auditCasterNewcomerV2Evidence } from './verify-caster-newcomer-v2-evidence.mjs';

// Synthetic, ordinary-protocol fixtures. These are not captured player runs.
const IDS = [...Array.from({ length: 22 }, (_, index) => 2110001 + index), 2120015, 2120020, 2120025, 2120030];
const START = '2026-10-01T00:00:00.000Z';
const ORDINARY_DEADLINE = '2026-10-01T02:00:00.000Z';
const FUNCTIONAL_START = '2026-10-01T02:01:00.000Z';
const FUNCTIONAL_DEADLINE = '2026-10-01T04:01:00.000Z';
const CHECKED_AT = '2026-10-01T03:00:00.000Z';
const FIRST_RUN = '2026-10-01T00-00-00-000Z';
const LAST_RUN = '2026-10-01T02-02-00-000Z';

function snapshot(className, ids, level) {
  return { mapFileName: '0', playerObjectId: 1000, gold: 5000, playerExperience: 600,
    entities: [{ objectId: 1000, kind: 'selfPlayer', name: `${className}Fixture`, class: className,
      level, x: 328, y: 264, direction: 'Right', hp: 100, dead: false }],
    questLog: ids.map(questId => ({ questId, stage: 'completed' })) };
}

function trace(className, { fresh, inherited = [], claims, runId, finalIds = [...inherited, ...claims],
  noAck = false, noFinalFrame = false, noChangeId = null, admin = false } = {}) {
  const events = [];
  const base = Date.parse(fresh ? START : '2026-10-01T02:02:00.000Z');
  const push = event => events.push({ ...event, sequence: events.length + 1,
    at: new Date(base + (events.length + 1) * 10).toISOString() });
  const sent = (type, fields = {}) => push({ direction: 'sent', type, ...fields });
  const packet = (name, payload) => push({ direction: 'received', type: 'packet', packet: name, payload });
  const world = payload => push({ direction: 'received', type: 'worldSnapshot', payload });
  const character = level => ({ name: `${className}Fixture`, index: 1, class: className, level });
  if (fresh) sent('newAccount', { accountId: `PRIVATE-ACCOUNT-${className}`, password: 'SECRET_PASSWORD' });
  sent('login', { accountId: `PRIVATE-ACCOUNT-${className}`, password: 'SECRET_PASSWORD' });
  if (fresh) {
    sent('newCharacter', { name: `${className}Fixture`, class: className });
    packet('NewCharacterSuccess', { character: character(0) });
  }
  sent('startGame', { characterIndex: 1 });
  packet('StartGame', { result: 4 });
  packet('UserInformation', { objectId: 1000, name: `${className}Fixture`, class: className, level: fresh ? 1 : 17 });
  if (!noFinalFrame) world(snapshot(className, inherited, fresh ? 1 : 17));
  packet('CompleteQuest', { completedQuests: inherited });
  const accumulated = [...inherited];
  for (const id of claims) {
    sent('finishQuest', { questIndex: id });
    if (id !== noChangeId) packet('ChangeQuest', { questId: id, completed: true });
    accumulated.push(id);
    packet('CompleteQuest', { completedQuests: [...accumulated] });
  }
  if (admin) sent('qa.giveItem', { name: 'SECRET_ADMIN_ITEM' });
  if (!noFinalFrame) world(snapshot(className, finalIds, fresh ? 17 : 30));
  sent('logOut');
  world({ mapFileName: '', playerObjectId: null, entities: [], questLog: [] });
  if (!noAck) packet('LogOutSuccess', { characters: [character(fresh ? 17 : 30)] });
  return { runId, events };
}

function report(className, directory, runId, fresh) {
  const result = { runId, className, name: `${className}Fixture`, characterIndex: 1,
    transport: 'normal-local-websocket', traceFile: path.join(directory, `${className}.${runId}.trace.jsonl`),
    startedAt: fresh ? START : '2026-10-01T02:02:00.000Z', completed: !fresh, visualAccepted: false,
    v2: { profile: 'newcomer-v2', status: fresh ? 'paused' : 'completed',
      ordinaryStartedAt: START, deadlineAt: ORDINARY_DEADLINE, recoveries: [] } };
  if (!fresh) {
    Object.assign(result.v2, { functionalRecheckStartedAt: FUNCTIONAL_START, functionalRecheckDeadlineAt: FUNCTIONAL_DEADLINE });
    result.v2TimingLedger = { ordinaryStartedAt: START, recoveries: [], resumed: true,
      functionalRecheck: true, functionalRecheckStartedAt: FUNCTIONAL_START, functionalRecheckDeadlineAt: FUNCTIONAL_DEADLINE };
  }
  return result;
}

async function fixture(t, options = {}) {
  const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-caster-evidence-fixture-'));
  t.after(async () => {
    // Verify the exact task-created temp directory before recursive cleanup.
    assert.equal(path.dirname(path.resolve(directory)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(directory).startsWith('mir2-caster-evidence-fixture-'));
    await rm(directory, { recursive: true, force: true });
  });
  const files = {};
  for (const className of ['Wizard', 'Taoist']) {
    const folder = path.join(directory, className.toLowerCase());
    await mkdir(folder);
    const settings = options[className] ?? {};
    const inherited = IDS.slice(0, 10);
    const claims = IDS.slice(10).filter(id => id !== settings.omittedId);
    const finalIds = IDS.filter(id => id !== settings.omittedId);
    const first = report(className, folder, FIRST_RUN, true);
    const final = report(className, folder, LAST_RUN, false);
    const firstTrace = trace(className, { fresh: true, inherited: [], claims: inherited, runId: FIRST_RUN });
    const finalTrace = trace(className, { fresh: false, inherited, claims, finalIds, runId: LAST_RUN, ...settings });
    const state = snapshot(className, finalIds, 30);
    const character = { index: 1, name: `${className}Fixture`, class: className, level: 30 };
    const save = { revision: 2, character, map_file_name: '0', position: { x: 328, y: 264 },
      direction: 'Right', gold: 5000, experience: 600,
      quest_states_json: finalIds.map(quest_id => JSON.stringify({ quest_id, stage: 'completed' })) };
    const store = { accounts: { [`PRIVATE-ACCOUNT-${className}`]: { password: 'SECRET_PASSWORD',
      storage_password: 'SECRET_STORAGE_PASSWORD', characters: [character], saves: { '1': save } } } };
    const storePath = path.join(folder, 'accounts.json');
    const canonicalPath = path.join(folder, `${className}.report.json`);
    const archivedPath = path.join(folder, `${className}.${LAST_RUN}.report.json`);
    const firstReportPath = path.join(folder, `${className}.${FIRST_RUN}.report.json`);
    await Promise.all([
      writeFile(firstReportPath, JSON.stringify(first)), writeFile(archivedPath, JSON.stringify(final)),
      writeFile(canonicalPath, JSON.stringify(final)), writeFile(path.join(folder, `${className}.snapshot.json`), JSON.stringify(state)),
      writeFile(first.traceFile, firstTrace.events.map(event => JSON.stringify(event)).join('\n') + '\n'),
      writeFile(final.traceFile, finalTrace.events.map(event => JSON.stringify(event)).join('\n') + '\n'),
      writeFile(storePath, JSON.stringify(store)),
    ]);
    files[className] = { storePath, canonicalPath, archivedPath, firstReportPath, final, first, save,
      store, finalTrace, snapshotPath: path.join(folder, `${className}.snapshot.json`) };
  }
  return { directory, files, audit: () => auditCasterNewcomerV2Evidence({ evidenceRoot: directory,
    wizardStorePath: files.Wizard.storePath, taoistStorePath: files.Taoist.storePath, checkedAt: CHECKED_AT }) };
}

const hasReason = (row, code) => row.reasons.some(reason => reason.code === code);
async function replaceFinal(files) {
  await Promise.all([writeFile(files.canonicalPath, JSON.stringify(files.final)),
    writeFile(files.archivedPath, JSON.stringify(files.final))]);
}

test('52 receipted, normally logged-out caster nodes remain separate from the expired ordinary clock', async t => {
  const f = await fixture(t);
  const before = await readFile(f.files.Wizard.storePath, 'utf8');
  const result = await f.audit();
  assert.equal(result.completedUnits, 52);
  assert.equal(result.denominator, 52);
  assert.deepEqual(result.scopeClasses, ['Wizard', 'Taoist']);
  assert.equal(result.allClassesCompleted, true);
  assert.equal(result.functionalCompletionWithinDeadline, true);
  assert.equal(result.freshTimedCompletion, false);
  assert.equal(result.visualAccepted, false);
  for (const row of result.classes) {
    assert.equal(row.rawFinalSnapshot.selection, 'pre-logout-complete-frame');
    assert.equal(row.timing.ordinary.expiredAtAudit, true);
    assert.equal(row.timing.ordinary.completedWithinDeadline, false);
    assert.equal(row.timing.functional.inheritedCompletedIds.length, 10);
    assert.equal(row.timing.functional.newlyReceiptedIds.length, 16);
    assert.equal(row.timing.deaths.preserved, true);
    assert.equal(row.receipts.length, 26);
  }
  const publicJson = JSON.stringify(result);
  assert.equal(publicJson.includes('PRIVATE-ACCOUNT'), false);
  assert.equal(publicJson.includes('SECRET_'), false);
  assert.equal(publicJson.includes('password'), false);
  assert.equal(await readFile(f.files.Wizard.storePath, 'utf8'), before);
});

test('crossed stores and repeated store paths cannot certify another caster', async t => {
  const f = await fixture(t);
  const crossed = await auditCasterNewcomerV2Evidence({ evidenceRoot: f.directory,
    wizardStorePath: f.files.Taoist.storePath, taoistStorePath: f.files.Wizard.storePath, checkedAt: CHECKED_AT });
  assert.equal(crossed.completedUnits, 0);
  assert.ok(crossed.classes.every(row => hasReason(row, 'non-unique-store-character')));
  const repeated = await auditCasterNewcomerV2Evidence({ evidenceRoot: f.directory,
    wizardStorePath: f.files.Wizard.storePath, taoistStorePath: f.files.Wizard.storePath, checkedAt: CHECKED_AT });
  assert.equal(repeated.independentStores, false);
  assert.equal(repeated.completedUnits, 0);
});

test('a runner snapshot cannot substitute for an empty raw trace final frame', async t => {
  const f = await fixture(t, { Wizard: { noFinalFrame: true } });
  const result = await f.audit();
  assert.equal(result.classes[0].completedUnits, 0);
  assert.ok(hasReason(result.classes[0], 'missing-complete-authoritative-final-frame'));
  assert.equal(result.completedUnits, 26);
});

test('normal logout needs its received success ACK even when the store and final frame match', async t => {
  const f = await fixture(t, { Wizard: { noAck: true } });
  const result = await f.audit();
  assert.equal(result.classes[0].logoutCommitted, false);
  assert.equal(result.classes[0].completedUnits, 0);
  assert.ok(hasReason(result.classes[0], 'missing-normal-logout-success'));
});

test('missing nodes and bootstrap completion lists cannot manufacture a Finish receipt', async t => {
  const f = await fixture(t, { Wizard: { omittedId: 2110022 }, Taoist: { noChangeId: 2110021 } });
  const result = await f.audit();
  assert.equal(result.completedUnits, 50);
  assert.equal(result.classes[0].completedUnits, 25);
  assert.equal(result.classes[1].completedUnits, 25);
  assert.equal(result.classes[1].savedCompletedIds.length, 26);
  assert.equal(result.classes[1].receiptedCompletedIds.includes(2110021), false);
  assert.equal(result.allClassesCompleted, false);
  assert.equal(result.status, 'partial');
});

test('forged ordinary or functional deadlines invalidate certification without changing original time', async t => {
  const f = await fixture(t);
  f.files.Wizard.final.v2.deadlineAt = '2026-10-01T05:00:00.000Z';
  f.files.Taoist.final.v2.functionalRecheckDeadlineAt = '2026-10-01T05:01:00.000Z';
  await Promise.all([replaceFinal(f.files.Wizard), replaceFinal(f.files.Taoist)]);
  const result = await f.audit();
  assert.equal(result.completedUnits, 0);
  assert.ok(hasReason(result.classes[0], 'ordinary-clock-changed'));
  assert.ok(hasReason(result.classes[1], 'functional-clock-changed'));
  assert.equal(result.classes[0].timing.ordinary.deadlineAt, ORDINARY_DEADLINE);
  assert.equal(result.freshTimedCompletion, false);
});

test('foreign class, duplicate store ownership, and mismatched XP cannot pass saved identity', async t => {
  const f = await fixture(t);
  const wizard = f.files.Wizard;
  wizard.store.accounts['PRIVATE-ACCOUNT-Wizard'].characters.push({ ...wizard.save.character });
  const taoist = f.files.Taoist;
  taoist.save.experience += 1;
  await Promise.all([writeFile(wizard.storePath, JSON.stringify(wizard.store)), writeFile(taoist.storePath, JSON.stringify(taoist.store))]);
  const result = await f.audit();
  assert.equal(result.completedUnits, 0);
  assert.ok(hasReason(result.classes[0], 'non-unique-store-character'));
  assert.ok(hasReason(result.classes[1], 'authoritative-save-mismatch'));
});

test('run ID mismatch and admin commands cannot be relabelled as ordinary caster play', async t => {
  const f = await fixture(t, { Taoist: { admin: true } });
  f.files.Wizard.final.traceFile = f.files.Taoist.final.traceFile;
  await replaceFinal(f.files.Wizard);
  const result = await f.audit();
  assert.equal(result.completedUnits, 0);
  assert.ok(hasReason(result.classes[0], 'resume-identity-or-trace-mismatch'));
  assert.ok(hasReason(result.classes[1], 'non-ordinary-command'));
  assert.equal(JSON.stringify(result).includes('SECRET_ADMIN_ITEM'), false);
});

test('a resume cannot silently reset its original confirmed recovery ledger', async t => {
  const f = await fixture(t);
  const recovery = { phase: 'quest', questId: 2110004, deathAt: '2026-10-01T00:10:00.000Z',
    before: { mapFileName: '0', objectId: 1000, x: 1, y: 2, hp: 0 },
    revive: { at: '2026-10-01T00:10:01.000Z',
      before: { map: '0', objectId: 1000, x: 1, y: 2, hp: 0, dead: true },
      after: { map: '0', objectId: 1000, x: 3, y: 4, hp: 20, dead: false } },
    revivedAt: '2026-10-01T00:10:01.000Z',
    after: { mapFileName: '0', objectId: 1000, x: 3, y: 4, hp: 20 } };
  f.files.Wizard.first.v2.recoveries = [recovery];
  await writeFile(f.files.Wizard.firstReportPath, JSON.stringify(f.files.Wizard.first));
  const result = await f.audit();
  assert.equal(result.classes[0].completedUnits, 0);
  assert.ok(hasReason(result.classes[0], 'death-ledger-reset'));
  assert.equal(result.classes[0].timing.deaths.originalOrdinaryLedger.length, 1);
  assert.equal(result.classes[0].timing.deaths.preserved, false);
});
