import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { loadPolicy, inspectMonitor, MonitorGuard, CommandBudget, percentile, metricSummary,
  assessStage, newLoadAccount, validatePool, positiveAttackReceipt, eventsAfterSend, ordinaryAdjacentTarget, runLoad } from './playtest-load-smoke.mjs';

const WebSocketServer = createRequire(import.meta.url)('next/dist/compiled/ws').Server;

test('observed protected Guard cannot be mistaken for a missing combat acknowledgement', () => {
  const player = { x: 292, y: 620 };
  const guard = { kind: 'monster', name: 'Guard', hp: 9999, maxHp: 9999, x: 291, y: 620, dead: false };
  assert.equal(ordinaryAdjacentTarget(guard, player), false);
  const scarecrow = { ...guard, name: 'Scarecrow', hp: 20, maxHp: 20 };
  assert.equal(ordinaryAdjacentTarget(scarecrow, player), true);
  assert.equal(ordinaryAdjacentTarget({ ...scarecrow, hp: 0 }, player), false);
  assert.equal(ordinaryAdjacentTarget({ ...scarecrow, dead: true }, player), false);
  assert.equal(ordinaryAdjacentTarget({ ...scarecrow, x: 290 }, player), false);
  assert.equal(ordinaryAdjacentTarget({ ...scarecrow, kind: 'npc' }, player), false);
});
async function withFixture(action) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-load-test-'));
  try { return await action(directory); }
  finally {
    const resolved = path.resolve(directory);
    assert.equal(path.dirname(resolved), path.resolve(os.tmpdir()));
    assert.match(path.basename(resolved), /^playtest-load-test-/);
    await fs.rm(resolved, { recursive: true, force: true });
  }
}
function healthy(now = Date.now()) { return { sampledAt: now, stop: false, health: { currentActiveSessions: 1 } }; }

test('default load is bounded with immutable initial acceptance thresholds', () => {
  const policy = loadPolicy();
  assert.deepEqual(policy.stages, [1, 2, 3, 5, 8, 12, 15]);
  assert.equal(policy.stageSeconds, 90); assert.equal(policy.steadySeconds, 300);
  assert.equal(policy.commandsPerSecond, 3); assert.equal(policy.movementP95Ms, 750);
  assert.equal(policy.chatP95Ms, 1000); assert.equal(policy.timeoutRatioExclusive, 0.01);
  assert.equal(loadPolicy({ profile: 'probe', stages: '1', stageSeconds: 30, steadySeconds: 0 }).capacityAcceptanceEligible, false);
  for (const options of [{ stages: '16' }, { stages: '2,1' }, { stages: '1,1' }, { stages: '0' },
    { wallSeconds: 0 }, { stageSeconds: 10 }, { steadySeconds: 0 }, { profile: 'probe', stages: '3' }]) assert.throws(() => loadPolicy(options));
});

test('monitor fails closed on missing, stale, malformed, future and stop samples', () => {
  const now = 1700000000000;
  assert.equal(inspectMonitor(healthy(now), now).sampledAtMs, now);
  assert.equal(inspectMonitor({ ...healthy(now), sampledAt: new Date(now).toISOString() }, now).sampledAtMs, now);
  for (const sample of [undefined, {}, { sampledAt: now }, healthy(now - 15001), healthy(now + 5001),
    { ...healthy(now), stop: true, reason: 'memory guard' }]) assert.throws(() => inspectMonitor(sample, now));
});

test('monitor drain remains sticky even if a later file becomes healthy', async () => {
  let input = JSON.stringify(healthy(100000)); const records = [];
  const guard = new MonitorGuard('fixture', { add: value => records.push(value) }, { read: async () => input, clock: () => 100000 });
  await guard.poll(); assert.doesNotThrow(() => guard.assert());
  input = '{invalid'; await guard.poll(); assert.throws(() => guard.assert());
  input = JSON.stringify(healthy(100000)); await guard.poll(); assert.throws(() => guard.assert());
  assert.equal(records.filter(record => record.type === 'safetyStop').length, 1);
});

test('sliding command budget forbids fourth command inside any second', () => {
  let now = 0; const budget = new CommandBudget(() => now);
  budget.reserve(); now = 10; budget.reserve(); now = 100; budget.reserve();
  assert.throws(() => budget.reserve()); assert.equal(budget.remainingWait(), 900);
  now = 999; assert.throws(() => budget.reserve());
  now = 1000; budget.reserve(); assert.throws(() => budget.reserve());
  now = 1010; budget.reserve();
});

test('dedicated account pool excludes player accounts and binds the isolated endpoint', () => {
  const endpoint = 'ws://127.0.0.1:20110/ws';
  const pool = { schemaVersion: 1, purpose: 'isolated-load-actors', realm: 'isolated', endpoint,
    accounts: [newLoadAccount(0, 'abcdef'), newLoadAccount(1, 'abcdef')] };
  assert.equal(validatePool(pool, endpoint), pool);
  for (const mutate of [value => { value.accounts[0].accountId = 'demo'; },
    value => { value.accounts[0].accountId = 'ptac9c7c7b9'; },
    value => { value.accounts[0].name = 'Ac9c7c7b9'; },
    value => { value.accounts[1] = value.accounts[0]; },
    value => { value.accounts[0].state = 'ready'; },
    value => { value.endpoint = 'wss://another-realm.example/ws'; }]) {
    const copy = structuredClone(pool); mutate(copy); assert.throws(() => validatePool(copy, endpoint));
  }
});

test('keepalive-only traffic cannot pass a player workload stage', () => {
  const samples = Array.from({ length: 100 }, () => ({ actor: 'Labcdef00', kind: 'rtt', status: 'success', latencyMs: 1 }));
  const result = assessStage(samples, ['Labcdef00'], 90, loadPolicy());
  assert.equal(result.passed, false); assert.equal(result.enoughActivity, false);
  assert.equal(result.movementP95Ms, null); assert.equal(result.chatP95Ms, null);
});

test('stage gates retain corrections/timeouts and fail slow tails or absent actor work', () => {
  const actor = 'Labcdef00';
  const samples = Array.from({ length: 100 }, () => ({ actor, kind: 'walk', status: 'success', latencyMs: 50 }));
  samples.push({ actor, kind: 'chat', status: 'success', latencyMs: 80, receiver: 'peer' });
  assert.equal(assessStage(samples, [actor], 90, loadPolicy()).passed, true);
  assert.equal(assessStage(samples, [actor], 90, loadPolicy()).combatCoverage, 'not-covered');
  assert.equal(assessStage(samples, [actor], 90, loadPolicy()).combatCapacityAccepted, false);
  assert.equal(assessStage(samples, [actor, 'Labcdef01'], 90, loadPolicy()).passed, false);
  const slow = structuredClone(samples); for (const sample of slow.slice(0, 8)) sample.latencyMs = 751;
  assert.equal(assessStage(slow, [actor], 90, loadPolicy()).passed, false);
  const timed = structuredClone(samples); timed[0].status = 'timeout'; timed[1].status = 'timeout';
  assert.equal(assessStage(timed, [actor], 90, loadPolicy()).passed, false);
  const corrected = structuredClone(samples); corrected[0].status = 'corrected';
  assert.equal(metricSummary(corrected).walk.status.corrected, 1);
  assert.equal(percentile([10, 20, 30, 40], 0.95), 40);
});

test('positive combat requires own actual strike and positive damage with either packet order', () => {
  const struck = { sequence: 11, monotonicMs: 50, direction: 'received', packet: 'ObjectStruck', payload: { objectId: 42, attackerId: 1000 } };
  const damage = { sequence: 12, monotonicMs: 51, direction: 'received', packet: 'DamageIndicator', payload: { objectId: 42, damage: 3 } };
  assert.equal(positiveAttackReceipt([struck, damage], 10, 42, 1000).damage, 3);
  assert.equal(positiveAttackReceipt([{ ...damage, sequence: 11 }, { ...struck, sequence: 12 }], 10, 42, 1000).damage, 3);
  assert.equal(positiveAttackReceipt([struck, damage], 12, 42, 1000), null);
  assert.equal(positiveAttackReceipt([damage], 10, 42, 1000), null);
  assert.equal(positiveAttackReceipt([struck, { ...damage, payload: { objectId: 42, damage: 0 } }], 10, 42, 1000), null);
  assert.equal(positiveAttackReceipt([struck, { ...damage, sequence: 30 }], 10, 42, 1000), null);
  assert.equal(positiveAttackReceipt([struck, { ...struck, sequence: 12, payload: { objectId: 42, attackerId: 50001 } }, { ...damage, sequence: 13 }], 10, 42, 1000), null);
  assert.equal(positiveAttackReceipt([{ ...struck, sequence: 11, payload: { objectId: 42, attackerId: 50001 } }, damage, { ...struck, sequence: 13 }], 10, 42, 1000), null);
});

test('ACK correlation excludes stale packets received during command-budget waiting', () => {
  const events = [{ sequence: 11, direction: 'received', monotonicMs: 99, packet: 'UserLocation' },
    { sequence: 12, direction: 'sent', monotonicMs: 100, type: 'walk' },
    { sequence: 13, direction: 'received', monotonicMs: 130, packet: 'UserLocation' }];
  assert.deepEqual(eventsAfterSend(events, 10, 100).map(event => event.sequence), [13]);
});

test('probe drains an admitted actor with normal LogOut on monitor stop and never resumes load', async () => withFixture(async directory => {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 });
  await new Promise(resolve => server.once('listening', resolve));
  const endpoint = `ws://127.0.0.1:${server.address().port}/ws`, monitor = path.join(directory, 'monitor.json');
  const account = { ...newLoadAccount(0, 'abcdef'), state: 'ready', characterIndex: 0 }, types = [];
  await fs.mkdir(path.join(directory, 'private'));
  await fs.writeFile(path.join(directory, 'private', 'accounts.json'), JSON.stringify({ schemaVersion: 1, purpose: 'isolated-load-actors',
    realm: 'isolated', endpoint, createdAt: new Date().toISOString(), accounts: [account] }));
  await fs.writeFile(monitor, JSON.stringify(healthy()));
  let stoppingWrite = Promise.resolve();
  server.on('connection', socket => {
    let inGame = false;
    const send = (packet, payload) => socket.send(JSON.stringify({ type: 'packet', packet, payload }));
    const snapshot = () => socket.send(JSON.stringify({ type: 'worldSnapshot', payload: { mapFileName: '0',
      playerObjectId: inGame ? 1000 : null, playerHp: 30, playerMaxHp: 30, inventoryItems: [], equipmentItems: [],
      entities: inGame ? [{ objectId: 1000, kind: 'player', name: account.name, x: 328, y: 264, direction: 'Down', hp: 30, maxHp: 30, dead: false }] : [] } }));
    send('Connected', {});
    socket.on('message', data => {
      const input = JSON.parse(data); types.push(input.type);
      if (input.type === 'clientVersion') { send('ClientVersion', {}); snapshot(); }
      if (input.type === 'login') send('LoginSuccess', { characters: [{ name: account.name, index: 0 }] });
      if (input.type === 'startGame') {
        inGame = true; send('StartGame', { result: 4 }); snapshot();
        stoppingWrite = fs.writeFile(`${monitor}.next`, JSON.stringify({ ...healthy(), stop: true, reason: 'fixture memory threshold' }))
          .then(() => fs.rename(`${monitor}.next`, monitor));
      }
      if (input.type === 'keepAlive') send('KeepAlive', { time: input.time });
      if (input.type === 'logOut') { inGame = false; send('LogOutSuccess', { characters: [] }); }
    });
  });
  try {
    const report = await runLoad({ mode: 'run', profile: 'probe', stages: '1', stageSeconds: 30, steadySeconds: 0,
      realm: 'isolated', endpoint, monitor, accountsFile: path.join(directory, 'private', 'accounts.json'), output: path.join(directory, 'reports') });
    await stoppingWrite;
    assert.equal(report.safetyStopped, true); assert.equal(report.ok, false); assert.equal(report.capacityAcceptancePassed, false);
    assert.equal(types.filter(type => type === 'startGame').length, 1);
    assert.equal(types.filter(type => type === 'login').length, 1);
    assert.equal(types.filter(type => type === 'logOut').length, 1);
    assert.equal(report.sampledRelogin.status, 'skipped'); assert.deepEqual(report.cleanupErrors, []);
  } finally { await stoppingWrite; for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve)); }
}));

test('missing monitor prevents any account creation or network startup', async () => withFixture(async directory => {
  const accountFile = path.join(directory, 'private', 'accounts.json');
  const report = await runLoad({ mode: 'prepare', realm: 'isolated', endpoint: 'ws://127.0.0.1:1/ws',
    monitor: path.join(directory, 'missing.json'), accountsFile: accountFile, output: path.join(directory, 'reports'), count: 2 });
  assert.equal(report.ok, false); assert.equal(report.safetyStopped, true);
  await assert.rejects(fs.stat(accountFile), /ENOENT/);
  assert.deepEqual(report.bytes, { sent: 0, received: 0 });
}));

test('prepare uses ordinary fresh registration/character creation and never StartGame', async () => withFixture(async directory => {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 });
  await new Promise(resolve => server.once('listening', resolve));
  const endpoint = `ws://127.0.0.1:${server.address().port}/ws`, types = [];
  server.on('connection', socket => {
    const send = (packet, payload) => socket.send(JSON.stringify({ type: 'packet', packet, payload }));
    send('Connected', {});
    socket.on('message', data => {
      const input = JSON.parse(data); types.push(input.type);
      if (input.type === 'clientVersion') send('ClientVersion', {});
      if (input.type === 'newAccount') send('NewAccount', { result: 8 });
      if (input.type === 'login') send('LoginSuccess', { characters: [] });
      if (input.type === 'newCharacter') send('NewCharacterSuccess', { character: { name: input.name, index: types.length } });
    });
  });
  try {
    const monitor = path.join(directory, 'monitor.json'); await fs.writeFile(monitor, JSON.stringify(healthy()));
    const accountFile = path.join(directory, 'private', 'accounts.json');
    const report = await runLoad({ mode: 'prepare', endpoint, realm: 'isolated', accountsFile: accountFile,
      count: 2, monitor, output: path.join(directory, 'reports') });
    assert.equal(report.ok, true); assert.equal(report.readyAccounts, 2);
    assert.equal(types.filter(type => type === 'newAccount').length, 2);
    assert.equal(types.filter(type => type === 'newCharacter').length, 2);
    assert.equal(types.includes('startGame'), false); assert.equal(types.includes('keepAlive'), false);
    const pool = JSON.parse(await fs.readFile(accountFile, 'utf8'));
    const files = await fs.readdir(path.join(directory, 'reports'));
    for (const file of files) {
      const publicEvidence = await fs.readFile(path.join(directory, 'reports', file), 'utf8');
      for (const account of pool.accounts) assert.equal(publicEvidence.includes(account.password), false);
    }
  } finally { for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve)); }
}));

test('registration rejection stops preparation without retrying or creating further accounts', async () => withFixture(async directory => {
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0 });
  await new Promise(resolve => server.once('listening', resolve)); let registrations = 0;
  server.on('connection', socket => {
    socket.send(JSON.stringify({ type: 'packet', packet: 'Connected', payload: {} }));
    socket.on('message', data => {
      const input = JSON.parse(data);
      if (input.type === 'clientVersion') socket.send(JSON.stringify({ type: 'packet', packet: 'ClientVersion', payload: {} }));
      if (input.type === 'newAccount') { registrations++; socket.send(JSON.stringify({ type: 'error', code: 'rateLimited', message: 'Account creation rate limit reached' })); }
    });
  });
  try {
    const monitor = path.join(directory, 'monitor.json'); await fs.writeFile(monitor, JSON.stringify(healthy()));
    const report = await runLoad({ mode: 'prepare', endpoint: `ws://127.0.0.1:${server.address().port}/ws`, realm: 'isolated',
      accountsFile: path.join(directory, 'private', 'accounts.json'), count: 2, monitor, output: path.join(directory, 'reports') });
    assert.equal(report.ok, false); assert.equal(registrations, 1); assert.equal(report.peakActiveBots, 0);
    assert.match(report.error, /rate limit/i);
  } finally { for (const socket of server.clients) socket.terminate(); await new Promise(resolve => server.close(resolve)); }
}));
