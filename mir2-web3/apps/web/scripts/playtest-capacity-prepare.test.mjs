import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { CapacityClient } from './playtest-capacity-soak.mjs';
import { runPreparationBank, selectPreparationStagingPoint, validatePreparationContinuation, sameContinuationEvidencePath } from './playtest-capacity-prepare.mjs';

async function fixture(t, { count = 100, maxPrepared = 2, ledger, prepareFailure = -1, drainFailure = -1,
  staleReadiness = false, progress, initialReady = false, login } = {}) {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-prepare-bank-test-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const privateRoot = path.join(root, 'private'); await fs.mkdir(privateRoot, { mode: 0o700 });
  const poolFile = path.join(privateRoot, 'capacity-pool.json'), output = path.join(root, 'reports');
  const accounts = Array.from({ length: count }, (_, i) => ({ accountId: 'lt' + i.toString(36).padStart(8, '0'),
    name: 'L' + i.toString(36).padStart(8, '0'), password: 'test-password-' + String(i).padStart(3, '0'),
    state: initialReady ? 'ready' : 'registered', ...(initialReady ? { characterIndex: i + 1 } : {}), provisioning: 'fixture-accounts-only' }));
  const endpoint = 'ws://127.0.0.1:17510/ws';
  const pool = { schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint, accounts, unknownPoolMetadata: { retained: true } };
  await fs.writeFile(poolFile, JSON.stringify(pool));
  const ledgerFile = path.join(privateRoot, 'capacity-auth-budget.json');
  if (ledger) await fs.writeFile(ledgerFile, JSON.stringify({ schemaVersion: 1, purpose: 'capacity-auth-budget', endpoint, attempts: ledger }));
  const scenarioFile = path.join(root, 'scenario.json');
  await fs.writeFile(scenarioFile, JSON.stringify({ mapFileName: '0', preparation: {
    stagingPoints: accounts.map((_, i) => ({ x: 100 + i, y: 200 })), route: { quests: [{ id: 'authoritative-fixture-deer' }] } } }));
  const events = [], contexts = [], clients = []; let active = 0, maximumActive = 0;
  class Monitor {
    constructor() { this.stopped = false; this.latest = { sampledAtMs: Date.now() }; }
    async start() { events.push('monitor:start'); this.assert(); }
    assert() { if (this.stopped) throw new Error(this.reason); }
    stop(reason) { if (!this.stopped) { this.stopped = true; this.reason = reason; } }
    close() { events.push('monitor:close'); }
  }
  const monitor = new Monitor();
  const scalar = (client, sequence, stock = 12) => ({ ownerId: 1000, ownerName: client.label, map: '0', x: 100 + client.index,
    y: 200, hp: 18, maxHp: 18, hpStock: stock, gold: 7, sequence, monotonicMs: sequence * 20,
    inSafeZone: true, nearbyHostileCount: 0, actualStagingReached: true });
  const services = {
    createMonitor: () => monitor,
    createClient: (_endpoint, account, context) => {
      const index = accounts.findIndex(a => a.name === account.name);
      const client = { account, label: account.name, index, context, sequence: 5, ready: false, closed: false, inGame: false,
        secrets: [account.password], recentSecrets: [],
        async refresh() { events.push('refresh:' + index); this.sequence++; return this.snapshot; } };
      contexts.push(context); clients.push(client); return client;
    },
    async login(client, context, purpose) {
      events.push('login:' + client.index); assert.equal(purpose, 'admission'); assert.equal(context.pool.accounts.length, count);
      assert.equal(client.ready, false); assert.equal(context.phase, 'preparation');
      assert.equal(context.engine, undefined); assert.equal(context.continuous, undefined);
      if (login) return login(client, context, purpose);
      context.loginBudget.reserve(purpose);
      await fs.writeFile(context.ledgerFile, JSON.stringify({ schemaVersion: 1, purpose: 'capacity-auth-budget', endpoint, attempts: context.loginBudget.attempts }));
      client.account.state = 'ready'; client.account.characterIndex ??= client.index + 1;
      await fs.writeFile(context.accountFile, JSON.stringify(context.pool));
      client.snapshot = { mapFileName: '0' }; client.inGame = true; active++; maximumActive = Math.max(maximumActive, active);
    },
    async loadMap(name, options) { assert.equal(name, '0'); assert.equal(options.mapRoot, root); return { fixture: true }; },
    async recover(client, context) {
      events.push('recover:' + client.index); assert.equal(client.ready, false); assert.equal(context.phase, 'preparation');
      return { status: 'not-needed', doses: 0, preparationOnly: true, measurementCredit: false };
    },
    async equip(client) { events.push('equip:' + client.index); },
    async prepare(client, context, config) {
      events.push('prepare:' + client.index); assert.equal(client.ready, false); assert.equal(context.phase, 'preparation');
      assert.deepEqual(config.stagingPoint, { x: 100 + client.index, y: 200 });
      const result = { status: 'ready', targetHp: 12, before: scalar(client, 10, initialReady ? 12 : 1), after: scalar(client, 20),
        purchased: initialReady ? 0 : 11, soldVenison: initialReady ? 0 : 3, funding: initialReady ? null : { method: 'DeerVenison', collected: 3, hunts: 3 },
        goldEarned: 447, goldSpent: 440, commands: 75, movementCommands: 14, elapsedMs: 141000, actualStagingReached: true };
      client.ordinarySupplies = { status: () => result, dispose: () => events.push('dispose:' + client.index),
        assertReady({ afterSequence }) { events.push('ready:' + client.index); assert.equal(client.sequence, afterSequence + 1);
          if (staleReadiness) throw new Error('ordinary supplies: fresh cohort readiness failed');
          return scalar(client, client.sequence); } };
      if (client.index === prepareFailure) throw new Error('fixture prepare failed ' + client.account.password);
      return result;
    },
    async awaitPending(list) { assert.equal(list.length, 1); events.push('pending:' + list[0].index); },
    async drain(client, context) { events.push('drain:' + client.index); client.paused = true; client.draining = true;
      client.closed = true; if (client.inGame) active--; client.inGame = false;
      if (client.index === drainFailure) context.cleanupErrors.push({ actor: client.label, step: 'normalLogout', error: 'fixture save error' });
      return { account: client.account, state: { name: client.label, mapFileName: '0', x: 100 + client.index, y: 200 } }; },
  };
  const options = { realm: 'isolated', endpoint, poolFile, monitor: path.join(root, 'monitor.json'), output, scenarioFile,
    mapRoot: root, maxPrepared, finiteWallSeconds: 18000, targetHp: 12,
    ...(!ledger ? { loginNotBefore: new Date(Date.now() - 1000).toISOString() } : {}),
    ...(progress ? { onProgress: value => progress(value) } : {}) };
  return { root, pool, poolFile, ledgerFile, scenarioFile, options, services, events, contexts, clients, monitor,
    maximumActive: () => maximumActive, remainingActive: () => active };
}

test('preparation runs one worker, saves before next login and preserves all100 pool entries', async t => {
  const f = await fixture(t, { maxPrepared: 25 });
  const originalTail = JSON.stringify(f.pool.accounts.slice(25));
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, true); assert.equal(report.preparedCount, 25); assert.equal(report.completePoolAccountCount, 100);
  assert.equal(report.completePoolPreserved, true); assert.equal(f.maximumActive(), 1); assert.equal(f.remainingActive(), 0);
  assert.ok(f.events.indexOf('drain:0') < f.events.indexOf('login:1'));
  const savedPool = JSON.parse(await fs.readFile(f.poolFile, 'utf8'));
  assert.equal(savedPool.accounts.length, 100); assert.equal(JSON.stringify(savedPool.accounts.slice(25)), originalTail);
  assert.deepEqual(savedPool.unknownPoolMetadata, { retained: true });
  assert.deepEqual(report.checkpoints.map(c => c.preparedCount), [25]);
  assert.equal(report.checkpoints[0].coldCopyPerformed, false);
  assert.equal(report.preparationOnly, true); assert.equal(report.measurementPerformed, false); assert.equal(report.cohortActivated, false);
  assert.equal(report.capacityAccepted, false); assert.equal(report.p0Accepted, false);
  assert.equal(report.testExecution, true); assert.equal(report.ordinaryPreparationEvidenceEligible, false);
  assert.equal(report.durableReloginVerificationPerformed, false); assert.equal(report.actorResults[0].normalLogout, true);
  assert.equal(report.actorResults[0].before.hpStock, 1); assert.equal(report.actorResults[0].after.hpStock, 12);
  await assert.rejects(fs.access(path.join(path.dirname(f.poolFile), 'capacity-run.lock')));
  const publicText = await fs.readFile(report.reportPath, 'utf8'); assert.ok(!publicText.includes(f.pool.accounts[0].password));
});

test('ready bank characters are normally logged in again and reuse only actual held stock', async t => {
  const f = await fixture(t, { maxPrepared: 2, initialReady: true });
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, true); assert.deepEqual(f.events.filter(e => e.startsWith('login:')), ['login:0', 'login:1']);
  assert.equal(report.initialReadyCharacters, 100);
  assert.ok(report.actorResults.every(a => a.reusedInventoryWithoutEconomicWork && a.before.hpStock === 12 && a.purchased === 0));
  assert.deepEqual(report.actorResults.map(a => a.characterIndex), [1, 2]);
});

test('one persistent ledger is inherited and updated for each real admission without verification relogins', async t => {
  const prior = [{ at: Date.now() - 1000, kind: 'verification' }];
  const f = await fixture(t, { maxPrepared: 2, ledger: prior });
  const report = await runPreparationBank(f.options, f.services);
  const ledger = JSON.parse(await fs.readFile(f.ledgerFile, 'utf8'));
  assert.equal(report.ok, true); assert.equal(report.authBudget.inherited, true); assert.equal(report.authBudget.initialAttempts, 1);
  assert.deepEqual(ledger.attempts.map(a => a.kind), ['verification', 'admission', 'admission']);
  assert.equal(ledger.attempts[0].at, prior[0].at);
});

test('existing auth ledger cannot be overridden by a new login origin', async t => {
  const f = await fixture(t, { ledger: [{ at: Date.now(), kind: 'admission' }] });
  const before = await fs.readFile(f.ledgerFile, 'utf8');
  f.options.loginNotBefore = new Date().toISOString();
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.match(report.error, /Existing auth ledger must not be overridden/);
  assert.equal(f.clients.length, 0); assert.equal(await fs.readFile(f.ledgerFile, 'utf8'), before);
});

test('absent auth ledger keeps the original901s initial wait unless an origin is explicit', async t => {
  let observed;
  const f = await fixture(t, { login: async (_client, context) => { observed = context.loginBudget.delay('admission'); throw new Error('fixture stopped before any admission'); } });
  delete f.options.loginNotBefore;
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.ok(observed > 900000 && observed <= 901000);
  await assert.rejects(fs.access(f.ledgerFile));
});

test('a failed preparation stops further admission and normally closes its active actor', async t => {
  const f = await fixture(t, { maxPrepared: 5, prepareFailure: 1 });
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.equal(report.preparedCount, 1); assert.equal(report.failedActor.name, f.pool.accounts[1].name);
  assert.deepEqual(f.events.filter(e => e.startsWith('login:')), ['login:0', 'login:1']);
  assert.ok(f.events.includes('drain:1')); assert.equal(f.remainingActive(), 0);
  assert.ok(!report.error.includes(f.pool.accounts[1].password)); assert.match(report.error, /\[redacted\]/);
});

test('stale readiness and normal logout errors cannot count as prepared', async t => {
  const stale = await fixture(t, { maxPrepared: 2, staleReadiness: true });
  const staleReport = await runPreparationBank(stale.options, stale.services);
  assert.equal(staleReport.preparedCount, 0); assert.equal(staleReport.ok, false); assert.ok(stale.events.includes('drain:0'));
  const broken = await fixture(t, { maxPrepared: 2, drainFailure: 0 });
  const brokenReport = await runPreparationBank(broken.options, broken.services);
  assert.equal(brokenReport.preparedCount, 0); assert.equal(brokenReport.ok, false);
  assert.deepEqual(broken.events.filter(e => e.startsWith('login:')), ['login:0']);
  assert.ok(brokenReport.cleanupErrors.some(e => e.step === 'normalLogout'));
});

test('AbortSignal after a saved actor stops the next admission without activating anyone', async t => {
  const controller = new AbortController();
  const f = await fixture(t, { maxPrepared: 5, progress: value => { if (value.stage === 'prepared') controller.abort(); } });
  f.options.signal = controller.signal;
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.preparedCount, 1); assert.equal(report.ok, false); assert.equal(report.cohortActivated, false);
  assert.equal(f.clients.length, 1); assert.equal(f.remainingActive(), 0); assert.match(report.error, /AbortSignal/);
});

test('finite wall and complete scenario bounds fail before sending later commands', async t => {
  const f = await fixture(t, { maxPrepared: 2 }); let clock = 0;
  f.options.finiteWallSeconds = 1; f.services.now = () => clock;
  const equip = f.services.equip; f.services.equip = async client => { await equip(client); clock = 1001; };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.equal(report.preparedCount, 0); assert.match(report.error, /wall-clock/);
  assert.ok(!f.events.includes('prepare:0')); assert.ok(f.events.includes('drain:0'));
  const short = await fixture(t, { maxPrepared: 2 });
  const scenario = JSON.parse(await fs.readFile(short.scenarioFile, 'utf8')); scenario.preparation.stagingPoints = scenario.preparation.stagingPoints.slice(0, 2);
  await fs.writeFile(short.scenarioFile, JSON.stringify(scenario));
  const invalid = await runPreparationBank(short.options, short.services);
  assert.equal(invalid.ok, false); assert.equal(short.clients.length, 0); assert.match(invalid.error, /distinct explicit/);
});


test('late staging chooses declared preferred point, otherwise skips live occupants and saved assignments', () => {
  const owner = { objectId: 1000, name: 'ordinary-owner', kind: 'selfPlayer', x: 10, y: 10 };
  const snapshot = { playerObjectId: 1000, mapFileName: '0', entities: [owner,
    { objectId: 2, kind: 'player', x: 11, y: 10 }, { objectId: 3, kind: 'monster', x: 12, y: 10 }] };
  const receipt = { ownerId: 1000, ownerName: owner.name, map: '0', x: 10, y: 10, sequence: 4, monotonicMs: 50 };
  const candidates = [{ x: 11, y: 10 }, { x: 12, y: 10 }, { x: 13, y: 10 }, { x: 14, y: 10 }];
  const savedPoints = new Set(['13,10']);
  const selected = selectPreparationStagingPoint({ snapshot, receipt, defaultPoint: candidates[0], candidates, savedPoints });
  assert.deepEqual(selected, { x: 14, y: 10 }); assert.equal(savedPoints.size, 1);
  assert.equal(snapshot.entities[1].x, 11); assert.equal(snapshot.entities[2].x, 12);
  snapshot.entities[1].dead = true;
  assert.deepEqual(selectPreparationStagingPoint({ snapshot, receipt, defaultPoint: candidates[0], candidates, savedPoints }), { x: 11, y: 10 });
});

test('late staging never leaves the current visible declared catalogue or uses a map transfer', () => {
  const snapshot = { playerObjectId: 1000, mapFileName: '0', entities: [{ objectId: 1000, kind: 'selfPlayer', name: 'ordinary-owner', x: 10, y: 10 }],
    mapTransfers: [{ bounds: { minX: 11, maxX: 11, minY: 10, maxY: 10 } }] };
  const receipt = { ownerId: 1000, ownerName: 'ordinary-owner', map: '0', x: 10, y: 10, sequence: 4, monotonicMs: 50 };
  const args = { snapshot, receipt, defaultPoint: { x: 15, y: 10 }, candidates: [{ x: 11, y: 10 }, { x: 27, y: 10 }], savedPoints: new Set() };
  assert.throws(() => selectPreparationStagingPoint(args), /No fresh visible/);
  snapshot.mapFileName = '2'; assert.throws(() => selectPreparationStagingPoint(args), /same-owner map0/);
  snapshot.mapFileName = '0'; receipt.ownerName = 'changed-owner'; assert.throws(() => selectPreparationStagingPoint(args), /same-owner map0/);
});


test('a persisted pool truncation cannot be hidden by a still-complete in-memory pool', async t => {
  const f = await fixture(t, { maxPrepared: 1 });
  const normalDrain = f.services.drain;
  f.services.drain = async (client, context) => {
    const saved = await normalDrain(client, context);
    await fs.writeFile(context.accountFile, JSON.stringify({ ...context.pool, accounts: context.pool.accounts.slice(0, 1) }));
    return saved;
  };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.preparedCount, 1); assert.equal(report.ok, false); assert.equal(report.completePoolPreserved, false);
  assert.ok(report.cleanupErrors.some(e => e.step === 'completePoolPreservation'));
});


test('invalid cancellation input rejects before creating output or starting monitor', async t => {
  const f = await fixture(t);
  f.options.signal = {};
  await assert.rejects(runPreparationBank(f.options, f.services), /AbortSignal/);
  assert.deepEqual(f.events, []); await assert.rejects(fs.access(f.options.output));
});


test('the default real CapacityClient constructor receives fixed v2 preparation context', async t => {
  const f = await fixture(t, { maxPrepared: 1, initialReady: true });
  const fixtureLogin = f.services.login;
  delete f.services.createClient; // Exercise the production constructor, without a real transport.
  let constructed;
  f.services.login = async (client, context, purpose) => {
    constructed = client;
    assert.ok(client instanceof CapacityClient);
    assert.equal(client.attackResults, 'v2'); assert.equal(context.attackResults, 'v2');
    client.index = 0;
    client.refresh = async () => { client.sequence++; return client.snapshot; };
    return fixtureLogin(client, context, purpose);
  };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, true); assert.ok(constructed); assert.equal(report.attackResults, 'v2');
});

test('explicit legacy preparation is rejected before any monitor or client is created', async t => {
  const f = await fixture(t, { maxPrepared: 1 }); f.options.attackResults = 'legacy';
  await assert.rejects(runPreparationBank(f.options, f.services), /requires attackResults v2/);
  assert.deepEqual(f.events, []); assert.equal(f.clients.length, 0); await assert.rejects(fs.access(f.options.output));
});


test('final normal drain position, map and name must match readiness before indexing a saved actor', async t => {
  for (const mismatch of [{ x: 999 }, { mapFileName: '2' }, { name: 'replacement-owner' }]) {
    const f = await fixture(t, { maxPrepared: 2 });
    const normalDrain = f.services.drain;
    f.services.drain = async (client, context) => { const saved = await normalDrain(client, context); Object.assign(saved.state, mismatch); return saved; };
    const report = await runPreparationBank(f.options, f.services);
    assert.equal(report.ok, false); assert.equal(report.preparedCount, 0); assert.deepEqual(report.preparedIndex, []);
    assert.match(report.error, /Final normal saved position or identity/);
    assert.deepEqual(f.events.filter(e => e.startsWith('login:')), ['login:0']);
  }
});

test('ordinary recovery runs once after login before equip and helper, without activation', async t => {
  const f = await fixture(t, { maxPrepared: 2 });
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, true);
  for (const i of [0, 1]) {
    assert.ok(f.events.indexOf('login:' + i) < f.events.indexOf('recover:' + i));
    assert.ok(f.events.indexOf('recover:' + i) < f.events.indexOf('equip:' + i));
    assert.ok(f.events.indexOf('equip:' + i) < f.events.indexOf('prepare:' + i));
  }
  assert.deepEqual(f.events.filter(e => e.startsWith('recover:')), ['recover:0', 'recover:1']);
  assert.ok(report.actorResults.every(r => r.preparationRecovery.measurementCredit === false && r.preparationRecovery.preparationOnly === true));
  assert.equal(report.measurementPerformed, false); assert.equal(report.cohortActivated, false);
});

test('held recovery failure stops admission and normally drains without equipping or preparing', async t => {
  const f = await fixture(t, { maxPrepared: 5 });
  f.services.recover = async client => {
    client.preparationHpRecoveryResult = { status: 'failed', preparationOnly: true, measurementCredit: false, failure: 'held dose unproven' };
    throw new Error('held dose unproven');
  };
  const drain = f.services.drain;
  f.services.drain = async (client, context) => { assert.equal(client.preparationCommandBudget, null); return drain(client, context); };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.equal(report.preparedCount, 0);
  assert.deepEqual(f.events.filter(e => e.startsWith('login:')), ['login:0']);
  assert.equal(f.events.some(e => /^(equip|prepare):/.test(e)), false); assert.ok(f.events.includes('drain:0'));
  assert.equal(report.failedActor.preparationRecovery.status, 'failed'); assert.equal(f.remainingActive(), 0);
});

test('recovery and equip consume the same600s helper budget while auth waiting remains excluded', async t => {
  const f = await fixture(t, { maxPrepared: 1 }); let clock = 10, received;
  const login = f.services.login, prepare = f.services.prepare;
  f.services.monotonicNow = () => clock;
  f.services.login = async (...args) => { clock += 901000; return login(...args); };
  f.services.recover = async (client, context, options) => {
    assert.equal(options.actorDeadlineMs, 1501010); assert.equal(client.ready, false);
    clock += 1000; return { status: 'verified', doses: 1, preparationOnly: true, measurementCredit: false };
  };
  f.services.equip = async () => { clock += 500; };
  f.services.prepare = async (...args) => { received = args[2].timeoutMs; return prepare(...args); };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, true); assert.equal(received, 598500);
  assert.equal(report.actorResults[0].actorBudgetMs, 600000); assert.equal(report.actorResults[0].helperBudgetMs, 598500);
});

test('an exhausted recovery budget cannot be restarted for the helper or next actor', async t => {
  const f = await fixture(t, { maxPrepared: 5 }); let clock = 0;
  f.services.monotonicNow = () => clock;
  f.services.recover = async () => { clock += 600001; return { status: 'not-needed', preparationOnly: true, measurementCredit: false }; };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.match(report.error, /actor600s budget exhausted/);
  assert.equal(f.events.some(e => /^(equip|prepare):/.test(e)), false);
  assert.deepEqual(f.events.filter(e => e.startsWith('login:')), ['login:0']); assert.ok(f.events.includes('drain:0'));
});

test('recovery and equip spend the existing2048 command allowance before the helper with one readiness refresh reserved', async t => {
  const f = await fixture(t, { maxPrepared: 1 }); const prepare = f.services.prepare; let received;
  f.services.recover = async client => { client.preparationCommandBudget.sent += 3;
    return { status: 'verified', preparationOnly: true, measurementCredit: false }; };
  f.services.equip = async client => { client.preparationCommandBudget.sent += 2; };
  f.services.prepare = async (...args) => { received = args[2].maxCommands; return prepare(...args); };
  const drain = f.services.drain;
  f.services.drain = async (client, context) => { assert.equal(client.preparationCommandBudget, null); return drain(client, context); };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, true); assert.equal(received, 2042); assert.equal(report.actorResults[0].preHelperCommands, 5);
  assert.equal(report.actorResults[0].helperCommandBudget, 2042);
});


// These are schema fixtures, not runtime qualification. Every run below uses
// the explicit test-services seam and remains production-ineligible.
async function continuationFixture(t, options = {}) {
  const f = await fixture(t, { maxPrepared: 100, ledger: [], ...options });
  for (let i = 0; i < 43; i++) Object.assign(f.pool.accounts[i], { state: 'ready', characterIndex: i + 1 });
  await fs.writeFile(f.poolFile, JSON.stringify(f.pool));
  const scenario = JSON.parse(await fs.readFile(f.scenarioFile, 'utf8'));
  Object.assign(scenario.preparation, { candidatePolicy: 'fresh-unoccupied-distinct-saved-v1',
    stagingCandidates: Array.from({ length: 335 }, (_, i) => ({ x: 100 + i, y: 200 })) });
  await fs.writeFile(f.scenarioFile, JSON.stringify(scenario));
  f.services.isWalkable = () => true;
  const originalPrepare = f.services.prepare;
  f.services.prepare = async (...args) => {
    const result = await originalPrepare(...args);
    result.resolvedStagingPoint = { x: 100 + args[0].index, y: 200 };
    return result;
  };
  // Preserve the real LoginBudget semantics while advancing only its unit clock
  // across full admission windows; do not reset attempts or bypass reserve().
  const originalLogin = f.services.login; let budgetClock = Date.now();
  f.services.login = async (client, context, purpose) => {
    context.loginBudget.clock = () => budgetClock;
    budgetClock += context.loginBudget.delay(purpose);
    return originalLogin(client, context, purpose);
  };
  f.groups = [];
  f.writeGroup = async (start = 0, count = 42, mutate = () => {}) => {
    const directory = path.join(f.root, 'history-' + f.groups.length); await fs.mkdir(directory);
    const files = Object.fromEntries(['report', 'preparedIndex', 'cleanup', 'receipt'].map(k => [k,
      path.join(directory, k === 'report' ? 'fixture.preparation-report.json' : k + '.json')]));
    const records = Array.from({ length: count }, (_, j) => {
      const i = start + j;
      return { name: f.pool.accounts[i].name, characterIndex: i + 1, map: '0', x: 100 + i, y: 200,
        sequence: 23, monotonicMs: 460, inSafeZone: true, normalLogout: true, durableReloginReadbackPending: true };
    });
    const actors = records.map(row => {
      const proof = { ownerId: 1000, ownerName: row.name, map: row.map, x: row.x, y: row.y,
        hp: 18, maxHp: 18, hpStock: 12, inSafeZone: true, actualStagingReached: true };
      return { name: row.name, characterIndex: row.characterIndex,
        before: { ...proof, hpStock: 1, sequence: 10, monotonicMs: 200 },
        after: { ...proof, sequence: 20, monotonicMs: 400 },
        freshReadiness: { ...proof, sequence: 23, monotonicMs: 460 }, targetHp: 12,
        normalLogout: true, finalNormalDrainPositionMatched: true, actualStagingReached: true };
    });
    const report = { schemaVersion: 1, mode: 'ordinary-economic-preparation-only', preparationOnly: true, testExecution: false, attackResults: 'v2',
      reportPath: files.report, workerConcurrency: 1, maxPrepared: 100, completePoolAccountCount: 100,
      completePoolPreserved: true, measurementPerformed: false, cohortActivated: false, capacityAccepted: false, p0Accepted: false,
      cleanupErrors: [], ok: false, error: 'original partial failure retained', ordinaryPreparationEvidenceEligible: false,
      preparedCount: count, preparedIndex: records, actorResults: actors,
      failedActor: { name: f.pool.accounts[start + count]?.name ?? 'no-failed-actor' } };
    const cleanup = { schemaVersion: 1, packageId: 'fixture-history-' + f.groups.length,
      checks: Object.fromEntries(['sixCountersZero', 'monitorStoppedBeforeGateway', 'monitorExitZero', 'monitorTerminalPreserved',
        'monitorSamplerAbsent', 'normalOwnedShutdown', 'gatewayProcessAbsent', 'driverProcessAbsent', 'monitorProcessAbsent', 'portsFree',
        'postSourceUnchanged'].map(k => [k, true])), driverExit: { exitCode: 1, signal: null, error: null },
      gatewayExit: { exitCode: 0, signal: null, error: null }, monitorExit: { exitCode: 0, signal: null, error: null },
      remainingListeners: [], remainingMonitorChildren: [], logErrors: [] };
    const index = { schemaVersion: 1, packageId: cleanup.packageId, records };
    const receipt = { schemaVersion: 1, packageId: cleanup.packageId, preparationOnly: true, workerCount: 1, maxPrepared: 100,
      cleanupComplete: true, measurementPerformed: false, gameplayEngineActivated: false, capacityAccepted: false, p0Accepted: false,
      productionPreparationEvidence: false, preparedIndexValid: false, preparationComplete: false, coldBankEligible: false };
    mutate({ report, cleanup, index, receipt });
    const pin = async file => { const bytes = await fs.readFile(file); return { file, bytes: bytes.length,
      sha256: crypto.createHash('sha256').update(bytes).digest('hex') }; };
    await fs.writeFile(files.report, JSON.stringify(report)); const reportPin = await pin(files.report);
    index.sourceOriginalReport = reportPin;
    await fs.writeFile(files.preparedIndex, JSON.stringify(index)); await fs.writeFile(files.cleanup, JSON.stringify(cleanup));
    Object.assign(receipt, { rawOriginalReport: reportPin, preparedIndexRecord: await pin(files.preparedIndex),
      cleanupRecord: await pin(files.cleanup), rawOriginalVerdict: { attackResults: 'v2', ok: report.ok, error: report.error,
        preparedCount: report.preparedCount, testExecution: false, ordinaryPreparationEvidenceEligible: false } });
    await fs.writeFile(files.receipt, JSON.stringify(receipt));
    const group = Object.fromEntries(await Promise.all(Object.entries(files).map(async ([k, file]) => [k, await pin(file)])));
    f.groups.push(group); f.options.continuation = { mode: 'historical-prepared-index-v1', evidence: f.groups };
    return { group, files, report, index, cleanup, receipt };
  };
  f.preflight = () => validatePreparationContinuation(f.options.continuation, { pool: f.pool, endpoint: f.options.endpoint,
    maxPrepared: 100, targetHp: 12, stagingPoints: scenario.preparation.stagingPoints,
    stagingCandidates: scenario.preparation.stagingCandidates, map: { fixture: true }, isWalkable: f.services.isWalkable });
  return f;
}

test('continuation selects actual original43..100 only, preserves historical reservations and separates58 fresh from42 notFresh', async t => {
  const f = await continuationFixture(t); await f.writeGroup();
  const originalPrefix = JSON.stringify(f.pool.accounts.slice(0, 42));
  const canary = path.join(f.root, 'untouched-store-canary'); await fs.writeFile(canary, 'not a Store API');
  let reservationsChecked = false;
  const prepare = f.services.prepare;
  f.services.prepare = async (client, context, config) => {
    if (client.index === 42) {
      const owner = { objectId: 1000, name: client.label, kind: 'selfPlayer', x: 141, y: 200 };
      const selected = config.resolveStagingPoint({ snapshot: { playerObjectId: 1000, mapFileName: '0', entities: [owner] },
        receipt: { ownerId: 1000, ownerName: client.label, map: '0', x: 141, y: 200, sequence: 9, monotonicMs: 180 },
        defaultPoint: { x: 141, y: 200 } });
      assert.deepEqual(selected, { x: 142, y: 200 }); reservationsChecked = true;
    }
    return prepare(client, context, config);
  };
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, true); assert.equal(report.preparedCount, 58); assert.equal(report.newPreparedCount, 58);
  assert.equal(report.expectedThisRun, 58); assert.equal(report.historicalPreparedCount, 42); assert.equal(report.combinedSavedCount, 100);
  assert.equal(report.historicalNotFresh, true); assert.equal(reservationsChecked, true);
  assert.deepEqual(f.clients.map(c => c.index), Array.from({ length: 58 }, (_, i) => i + 42));
  assert.equal(f.clients[0].account.state, 'ready'); // failed43 was ready but is still prepared normally
  assert.ok(f.events.includes('login:42')); assert.ok(!f.events.includes('login:41'));
  assert.equal(report.actorResults.length, 58); assert.equal(report.preparedIndex.length, 58);
  assert.equal(report.preparedIndex[0].originalPoolIndex, 42); assert.equal(report.preparedIndex[57].originalPoolIndex, 99);
  assert.ok(report.historicalPreparedIndex.every(p => p.freshThisRun === false && p.origin === 'historical'));
  assert.ok(report.preparedIndex.every(p => p.freshThisRun === true && p.origin === 'current-run'));
  assert.equal(report.sourceOriginalVerdicts[0].ok, false); assert.match(report.sourceOriginalVerdicts[0].error, /partial failure/);
  for (const key of ['ordinaryPreparationEvidenceEligible', 'continuationPreparationEvidenceEligible', 'all100FreshPreparedThisRun',
    'fullDurableReadinessVerified', 'durableReloginVerificationPerformed', 'first4FreshReusePerformedThisRun', 'measurementPerformed',
    'cohortActivated', 'capacityAccepted', 'p0Accepted']) assert.equal(report[key], false, key);
  assert.equal(report.testExecution, true); assert.equal(f.maximumActive(), 1); assert.equal(f.remainingActive(), 0);
  assert.equal(JSON.stringify(JSON.parse(await fs.readFile(f.poolFile)).accounts.slice(0, 42)), originalPrefix);
  assert.equal(await fs.readFile(canary, 'utf8'), 'not a Store API');
});

test('continuation failure does not promote failed ready43 or admit later actors', async t => {
  const f = await continuationFixture(t, { prepareFailure: 42 }); await f.writeGroup();
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.equal(report.preparedCount, 0); assert.equal(report.newPreparedCount, 0);
  assert.equal(report.historicalPreparedCount, 42); assert.equal(report.combinedSavedCount, 42);
  assert.equal(report.preparedIndex.length, 0); assert.equal(report.failedActor.name, f.pool.accounts[42].name);
  assert.deepEqual(f.clients.map(c => c.index), [42]); assert.ok(f.events.includes('drain:42'));
  assert.equal(report.continuationPreparationEvidenceEligible, false);
});

test('continuation pure preflight crossbinds old false reports and exact copies without auth or client creation', async t => {
  const f = await continuationFixture(t); const h = await f.writeGroup();
  const ledger = await fs.readFile(f.ledgerFile), pool = await fs.readFile(f.poolFile);
  const plan = await f.preflight();
  assert.equal(plan.scope, 'public-history-not-fresh; no current Store or auth provenance attested');
  assert.equal(plan.sourceOriginalVerdicts[0].ok, false); assert.equal(plan.expectedThisRun, 58);
  assert.ok(Object.isFrozen(plan.historicalPreparedIndex[0].sourceReportPin));
  assert.equal(f.clients.length, 0); assert.equal(f.events.length, 0);
  assert.deepEqual(await fs.readFile(f.ledgerFile), ledger); assert.deepEqual(await fs.readFile(f.poolFile), pool);
  const copyDir = path.join(f.root, 'archive'); await fs.mkdir(copyDir);
  for (const pin of Object.values(h.group)) { const destination = path.join(copyDir, path.basename(pin.file));
    await fs.copyFile(pin.file, destination, fs.constants.COPYFILE_EXCL); pin.file = destination; }
  assert.equal((await f.preflight()).historicalPreparedCount, 42);
});

test('continuation rejects malformed or conflicting public history before zero admissions and without ledger reset', async t => {
  const cases = [
    ['reordered', ({ report }) => { report.preparedIndex.reverse(); report.actorResults.reverse(); }],
    ['duplicate point', ({ report }) => { report.preparedIndex[1].x = report.preparedIndex[0].x; }],
    ['wrong identity', ({ report }) => { report.actorResults[0].after.ownerName = 'foreign'; }],
    ['duplicate character', ({ report }) => { report.preparedIndex[1].characterIndex = report.preparedIndex[0].characterIndex; }],
    ['stale receipt', ({ report }) => { report.actorResults[0].freshReadiness.sequence = report.actorResults[0].after.sequence; }],
    ['outside catalogue', ({ report }) => { report.preparedIndex[0].x = 9999; }],
    ['missing save', ({ report }) => { report.actorResults[0].finalNormalDrainPositionMatched = false; }],
    ['low stock', ({ report }) => { report.actorResults[0].freshReadiness.hpStock = 11; }],
    ['failed included', ({ report }) => { report.failedActor.name = report.actorResults[0].name; }],
    ['cleanup false', ({ cleanup }) => { cleanup.checks.sixCountersZero = false; }],
    ['monitor exit1', ({ cleanup }) => { cleanup.monitorExit.exitCode = 1; }],
    ['source seam', ({ report }) => { report.testExecution = true; }],
    ['not original100', ({ report }) => { report.completePoolAccountCount = 42; }],
    ['map mismatch', ({ report }) => { report.actorResults[0].after.map = '1'; }],
  ];
  for (const [name, mutate] of cases) {
    await t.test(name, async t => {
      const f = await continuationFixture(t); await f.writeGroup(0, 42, mutate);
      const before = await fs.readFile(f.ledgerFile); const report = await runPreparationBank(f.options, f.services);
      assert.equal(report.ok, false); assert.equal(f.clients.length, 0); assert.equal(report.preparedCount, 0);
      assert.deepEqual(await fs.readFile(f.ledgerFile), before); assert.equal(report.ordinaryPreparationEvidenceEligible, false);
    });
  }
});

test('continuation pins are raw checked and internally bound, including hash, index and cleanup substitutions', async t => {
  for (const kind of ['hash', 'receipt-binding', 'index-binding', 'duplicate-group', 'directory']) {
    await t.test(kind, async t => {
      const f = await continuationFixture(t); const h = await f.writeGroup();
      if (kind === 'hash') h.group.report.sha256 = '0'.repeat(64);
      if (kind === 'directory') h.group.report.file = path.dirname(h.group.report.file);
      if (kind === 'duplicate-group') f.groups.push(h.group);
      if (kind === 'receipt-binding' || kind === 'index-binding') {
        const key = kind === 'receipt-binding' ? 'receipt' : 'preparedIndex';
        const value = JSON.parse(await fs.readFile(h.files[key]));
        (key === 'receipt' ? value.cleanupRecord : value.sourceOriginalReport).sha256 = '0'.repeat(64);
        const raw = Buffer.from(JSON.stringify(value)); await fs.writeFile(h.files[key], raw);
        h.group[key] = { file: h.files[key], bytes: raw.length, sha256: crypto.createHash('sha256').update(raw).digest('hex') };
      }
      const report = await runPreparationBank(f.options, f.services);
      assert.equal(report.ok, false); assert.equal(f.clients.length, 0);
    });
  }
});

test('continuation accepts disjoint ordered fresh-success groups but rejects group reordering and empty complements', async t => {
  const f = await continuationFixture(t); await f.writeGroup(0, 20); await f.writeGroup(20, 22);
  assert.equal((await f.preflight()).historicalPreparedCount, 42);
  f.groups.reverse(); await assert.rejects(f.preflight(), /original pool prefix/);
  const all = await continuationFixture(t); for (let i = 0; i < 100; i++) Object.assign(all.pool.accounts[i], { state: 'ready', characterIndex: i + 1 });
  await all.writeGroup(0, 100); await assert.rejects(all.preflight(), /empty complement/);
});

test('continuation requires original ledger, forbids origin overrides and rejects unknown or short pool modes before admission', async t => {
  for (const kind of ['missing ledger', 'origin override', 'short target', 'unknown mode']) {
    await t.test(kind, async t => {
      const f = await continuationFixture(t); await f.writeGroup();
      if (kind === 'missing ledger') await fs.unlink(f.ledgerFile);
      if (kind === 'origin override') f.options.loginNotBefore = new Date().toISOString();
      if (kind === 'short target') f.options.maxPrepared = 58;
      if (kind === 'unknown mode') f.options.continuation.mode = 'skip-ready';
      const report = await runPreparationBank(f.options, f.services);
      assert.equal(report.ok, false); assert.equal(f.clients.length, 0); assert.equal(report.preparedCount, 0);
    });
  }
});


test('continuation partial current successes remain separate and later failure does not promote the complement', async t => {
  const f = await continuationFixture(t, { prepareFailure: 44 }); await f.writeGroup();
  const report = await runPreparationBank(f.options, f.services);
  assert.equal(report.ok, false); assert.equal(report.preparedCount, 2); assert.equal(report.newPreparedCount, 2);
  assert.equal(report.expectedThisRun, 58); assert.equal(report.historicalPreparedCount, 42); assert.equal(report.combinedSavedCount, 44);
  assert.deepEqual(f.clients.map(c => c.index), [42, 43, 44]);
  assert.deepEqual(report.preparedIndex.map(p => p.originalPoolIndex), [42, 43]);
  assert.equal(report.failedActor.name, f.pool.accounts[44].name);
  assert.equal(report.continuationPreparationEvidenceEligible, false);
});


test('continuation path identity casefolds Windows only and never authorizes a parent symlink', async t => {
  const lower = path.resolve('case-distinct-parent', 'report.json');
  const upper = path.resolve('CASE-DISTINCT-PARENT', 'report.json');
  assert.equal(sameContinuationEvidencePath(lower, upper, 'win32'), true);
  assert.equal(sameContinuationEvidencePath(lower, upper, 'linux'), false);
  assert.equal(sameContinuationEvidencePath(lower, upper, 'darwin'), false);
  const f = await continuationFixture(t); const h = await f.writeGroup();
  const parentLink = path.join(f.root, 'history-parent-link');
  await fs.symlink(path.dirname(h.group.report.file), parentLink, process.platform === 'win32' ? 'junction' : 'dir');
  h.group.report.file = path.join(parentLink, path.basename(h.group.report.file));
  await assert.rejects(f.preflight(), /symlink ancestors/);
  assert.equal(f.clients.length, 0);
});
