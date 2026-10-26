import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { CapacityClient } from './playtest-capacity-soak.mjs';
import { runPreparationBank, selectPreparationStagingPoint } from './playtest-capacity-prepare.mjs';

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
