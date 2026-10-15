import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import {
  validateEndpoint, validateCredentials, redact, inventoryFingerprint,
  isAuthRejection, sharedHitEvidence, finishReport, PlaytestClient, runPlaytest, outsideRepository, endpointOrigin,
  exposesAuthenticatedGameplay,
  observedPlayerId,
  chatMatches,
  resumeCredentials,
  chatLatencyCheck, authenticate, usingClient,
} from './playtest-multiplayer-smoke.mjs';

const { Server: WebSocketServer } = createRequire(import.meta.url)('next/dist/compiled/ws');

test('external endpoints require TLS and cannot leak credentials in trace URLs', () => {
  assert.equal(validateEndpoint('wss://test.example/playtest/ws'), 'wss://test.example/playtest/ws');
  assert.equal(validateEndpoint('ws://127.0.0.1:19810/ws'), 'ws://127.0.0.1:19810/ws');
  for (const url of ['ws://192.168.1.3/ws', 'http://localhost/ws', 'wss://user:password@test.example/ws',
    'wss://test.example/ws?token=secret', 'wss://test.example/ws#secret']) {
    assert.throws(() => validateEndpoint(url));
  }
});

test('Origin uses the endpoint scheme and authority without copying its WebSocket path', () => {
  assert.equal(endpointOrigin('wss://test.example:9443/playtest/ws'), 'https://test.example:9443');
  assert.equal(endpointOrigin('ws://127.0.0.1:20110/ws'), 'http://127.0.0.1:20110');
});

test('the actual WebSocket handshake sends the derived Origin without weakening the server guard', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-origin-'));
  const output = path.join(dir, 'trace.jsonl');
  let observedOrigin;
  const server = new WebSocketServer({ host: '127.0.0.1', port: 0, verifyClient(info) {
    observedOrigin = info.origin;
    return info.origin === `http://127.0.0.1:${server.address().port}`;
  } });
  server.on('connection', socket => socket.on('message', raw => {
    if (JSON.parse(raw).type === 'clientVersion') {
      socket.send(JSON.stringify({ type: 'packet', packet: 'Connected' }));
      socket.send(JSON.stringify({ type: 'packet', packet: 'ClientVersion', payload: { result: 1 } }));
    }
  }));
  await new Promise(resolve => server.once('listening', resolve));
  const client = new PlaytestClient(`ws://127.0.0.1:${server.address().port}/ws`, 'origin', output);
  try {
    await client.connect();
    assert.equal(observedOrigin, `http://127.0.0.1:${server.address().port}`);
  } finally {
    await client.close();
    await new Promise(resolve => server.close(resolve));
    await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; });
    await fs.rmdir(dir);
  }
});

test('the runner refuses unspecified realms before writing credentials or making connections', async () => {
  await assert.rejects(runPlaytest({ endpoint: 'wss://test.example/ws' }), /realm isolated/);
});

test('credentials cannot be created in the project or its enclosing Git checkout', () => {
  assert.throws(() => outsideRepository(path.resolve(import.meta.dirname, '../../..')), /outside the repository/);
  assert.throws(() => outsideRepository(path.resolve(import.meta.dirname, '../../../../private')), /outside the repository/);
  assert.equal(outsideRepository(path.join(os.tmpdir(), 'playtest-private')), path.resolve(os.tmpdir(), 'playtest-private'));
});

test('each run requires two distinct strong private account identities', () => {
  const pair = [{ accountId: 'ptabcd1234', name: 'Aabcd1234', password: 'a'.repeat(20) },
    { accountId: 'ptbbcd1234', name: 'Bbcd1234', password: 'b'.repeat(20) }];
  assert.deepEqual(validateCredentials(pair), pair);
  for (const bad of [[pair[0]], [pair[0], pair[0]], [pair[0], { ...pair[1], name: pair[0].name }],
    [pair[0], { ...pair[1], password: 'demo' }], [pair[0], { ...pair[1], accountId: 'demo' }]]) {
    assert.throws(() => validateCredentials(bad));
  }
});

test('resuming a bounded QA run cannot reuse credentials from a different realm or account pair', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-resume-'));
  const privatePath = path.join(dir, 'accounts.json'), reportPath = path.join(dir, 'report.json');
  const credentials = [{ accountId: 'ptatest123', name: 'Atest123', password: 'a'.repeat(20) },
    { accountId: 'ptbtest123', name: 'Btest123', password: 'b'.repeat(20) }];
  const report = { schemaVersion: 1, endpoint: 'ws://127.0.0.1:20110/ws', runId: 'prior',
    checks: { twoAccounts: { status: 'passed' } }, privateCredentialsPath: privatePath,
    accounts: credentials.map(({ accountId, name }) => ({ accountId, name })) };
  try {
    await fs.writeFile(privatePath, JSON.stringify(credentials), { mode: 0o600 });
    await fs.writeFile(reportPath, JSON.stringify(report));
    assert.equal((await resumeCredentials(reportPath, report.endpoint)).resumedFrom.runId, 'prior');
    await assert.rejects(resumeCredentials(reportPath, 'wss://other.example/ws'), /matching-realm/);
    report.accounts[0].name = 'OtherName';
    await fs.writeFile(reportPath, JSON.stringify(report));
    await assert.rejects(resumeCredentials(reportPath, report.endpoint), /do not match/);
  } finally { await fs.unlink(privatePath); await fs.unlink(reportPath); await fs.rmdir(dir); }
});

test('recursive credential fields and echoed secret values never survive report redaction', () => {
  const value = { payload: { reconnectToken: 'token', accounts: [{ password: 'p', secretAnswer: 'a' }] },
    message: 'server echoed private-value', harmless: 7 };
  const safe = redact(value, ['private-value']);
  assert.deepEqual(safe, { payload: { reconnectToken: '[redacted]', accounts: [{ password: '[redacted]', secretAnswer: '[redacted]' }] },
    message: 'server echoed [redacted]', harmless: 7 });
  assert.equal(value.payload.accounts[0].password, 'p');
});

test('failed login or absent character is not proof of an authentication guard', () => {
  assert.equal(isAuthRejection({ type: 'error', message: 'authenticated account is required before StartGame' }), true);
  assert.equal(isAuthRejection({ type: 'error', code: 'commandRejected', message: 'The session command could not be completed.' }), true);
  for (const value of [{ packet: 'NewCharacter', payload: { result: 0 } },
    { type: 'error', message: 'character does not exist' }, { type: 'error', message: 'invalid character name' }]) {
    assert.equal(isAuthRejection(value), false);
  }
});

test('a delayed public login shell cannot be confused with an owned character bootstrap', () => {
  assert.equal(exposesAuthenticatedGameplay({ type: 'worldSnapshot', payload: {
    playerObjectId: null, entities: [{ objectId: 0, name: 'template' }], inventoryItems: [{ name: 'template potion' }],
  } }), false);
  assert.equal(exposesAuthenticatedGameplay({ type: 'worldSnapshot', payload: {
    playerObjectId: 51, entities: [{ objectId: 51, name: 'PrivateCharacter' }],
  } }), true);
  assert.equal(exposesAuthenticatedGameplay({ packet: 'UserInformation' }), true);
  assert.equal(exposesAuthenticatedGameplay({ packet: 'NewCharacterSuccess' }), true);
});

test('ObjectChat uses the real wire text field and checks the sender identity', () => {
  assert.equal(chatMatches({ objectId: 50001, text: 'PlayerB: playtest unique' }, 50001, 'playtest unique'), true);
  assert.equal(chatMatches({ objectId: 50001, text: 'PlayerB: playtest unique' }, 1000, 'playtest unique'), false);
  assert.equal(chatMatches({ objectId: 50001, text: 'PlayerB: old message' }, 50001, 'playtest unique'), false);
});

test('delivered chat still fails acceptance if a stationary observer waits over one second', () => {
  for (const milliseconds of [0, 10, 1000]) assert.equal(chatLatencyCheck(milliseconds).status, 'passed');
  for (const milliseconds of [1001, 11445, -1, Infinity, NaN]) assert.equal(chatLatencyCheck(milliseconds).status, 'failed');
});

test('save comparison detects lost stacks, gold, character inventory or equipment identities', () => {
  const snapshot = { gold: 123, inventoryItems: [
    { uniqueId: 2, itemIndex: 9, name: 'Drug', count: 4 }, { uniqueId: 1, itemIndex: 8, name: 'Scroll', count: 2 },
  ], equipmentItems: [{ uniqueId: 3, itemIndex: 1, name: 'Sword', slot: 'weapon', count: 1 }] };
  const reordered = structuredClone(snapshot); reordered.inventoryItems.reverse();
  assert.deepEqual(inventoryFingerprint(snapshot), inventoryFingerprint(reordered));
  for (const mutation of [copy => copy.inventoryItems[0].count--, copy => copy.gold--,
    copy => copy.equipmentItems[0].uniqueId++, copy => copy.inventoryItems.pop()]) {
    const changed = structuredClone(snapshot); mutation(changed);
    assert.notDeepEqual(inventoryFingerprint(snapshot), inventoryFingerprint(changed));
  }
});

const event = (packet, payload, sequence) => ({ packet, payload, sequence, direction: 'received' });
function hitClient(percent = 70) {
  return { events: [event('ObjectStruck', { objectId: 99, attackerId: 51 }, 10),
    event('DamageIndicator', { objectId: 99, damage: 5 }, 11),
    event('ObjectHealth', { objectId: 99, percent }, 12)] };
}

test('a combat pass requires fresh attacker-attributed strike, positive damage and matching observer health', () => {
  const a = hitClient(), b = hitClient();
  assert.equal(sharedHitEvidence([a, b], [9, 9], 99, 51).healthPercent, 70);
  assert.equal(sharedHitEvidence([a, b], [12, 12], 99, 51), null, 'old receipts are not reusable');
  assert.equal(sharedHitEvidence([a, b], [9, 9], 99, 52), null, 'another attacker is not proof');
  assert.equal(sharedHitEvidence([a, hitClient(80)], [9, 9], 99, 51), null, 'split-world health differs');
  const noDamage = hitClient(); noDamage.events = noDamage.events.filter(item => item.packet !== 'DamageIndicator');
  assert.equal(sharedHitEvidence([noDamage, noDamage], [9, 9], 99, 51), null, 'a strike animation alone is not damage');
  const noStrike = hitClient(); noStrike.events = noStrike.events.filter(item => item.packet !== 'ObjectStruck');
  assert.equal(sharedHitEvidence([a, noStrike], [9, 9], 99, 51), null, 'owner-only receipt does not establish shared combat');
});

test('shared player evidence respects normalized owner versus observer object IDs', () => {
  const a = hitClient(), b = hitClient();
  a.snapshot = { entities: [{ objectId: 1000, name: 'ActorA', kind: 'selfPlayer' }, { objectId: 50001, name: 'ActorB', kind: 'player' }] };
  b.snapshot = { entities: [{ objectId: 1000, name: 'ActorB', kind: 'selfPlayer' }, { objectId: 50000, name: 'ActorA', kind: 'player' }] };
  assert.equal(observedPlayerId(a, 'ActorA'), 1000);
  assert.equal(observedPlayerId(b, 'ActorA'), 50000);
  assert.equal(observedPlayerId(a, 'Missing'), null);
  a.events[0].payload.attackerId = 1000;
  b.events[0].payload.attackerId = 50000;
  assert.ok(sharedHitEvidence([a, b], [9, 9], 99, [1000, 50000]));
  assert.equal(sharedHitEvidence([a, b], [9, 9], 99, [1000, 1000]), null);
});

test('both observers receiving the same attributed death is accepted without a rounded health packet', () => {
  const a = hitClient(), b = hitClient();
  for (const client of [a, b]) {
    client.events = client.events.filter(item => item.packet !== 'ObjectHealth');
    client.events.push(event('ObjectDied', { objectId: 99 }, 13));
  }
  assert.equal(sharedHitEvidence([a, b], [9, 9], 99, 51).died, true);
});

test('skipped or failed map/combat/persistence checks can never create an overall pass', () => {
  const keys = ['authGuards', 'twoAccounts', 'mutualVisibility', 'movement', 'turn', 'chat', 'chatLatency', 'sharedCombat', 'mapRoundTrip', 'logout', 'reloginPersistence'];
  const all = () => Object.fromEntries(keys.map(key => [key, { status: 'passed' }]));
  assert.equal(finishReport({ checks: all() }).ok, true);
  for (const key of keys) for (const status of ['failed', 'skipped', 'running']) {
    const checks = all(); checks[key].status = status;
    assert.equal(finishReport({ checks }).ok, false);
  }
  const report = finishReport({ checks: all() });
  assert.equal(report.visualAccepted, false);
  assert.equal(report.twoPhysicalComputersTested, false);
  assert.equal(report.loadAccepted, false);
  assert.equal(finishReport({ checks: all(), error: 'unexpected harness failure' }).ok, false);
  assert.equal(finishReport({ checks: all(), cleanupErrors: ['logout was not acknowledged'] }).ok, false);
});

test('wire allowlist blocks teleport, grants, passkey impersonation and raw world mutation', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-wire-'));
  const output = path.join(dir, 'trace.jsonl');
  try {
    const wire = [], client = new PlaytestClient('wss://test.example/ws', 'test', output, ['private-pass']);
    client.ws = { send: raw => wire.push(JSON.parse(raw)) };
    for (const type of ['transferMap', 'moveTo', 'qaControl', 'qa.giveItem', 'event.spawn', 'passkeyLogin', 'stage5Command']) {
      assert.throws(() => client.send({ type, key: 'crystal:0:1:1' }), /Not an allowed/);
    }
    client.send({ type: 'chat', message: 'ordinary local speech' });
    client.send({ type: 'login', accountId: 'pttest', password: 'private-pass' });
    client.record('received', { type: 'identitySession', payload: { sessionToken: 'private-nested-token' } });
    client.record('received', { type: 'error', message: 'server echoed private-nested-token' });
    await client.writeQueue;
    assert.equal(wire.length, 2);
    assert.equal(wire[1].password, 'private-pass', 'actual authentication keeps the supplied secret only on the TLS wire');
    assert.doesNotMatch(await fs.readFile(output, 'utf8'), /private-pass/);
    assert.doesNotMatch(await fs.readFile(output, 'utf8'), /private-nested-token/);
  } finally { await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; }); await fs.rmdir(dir); }
});

test('command rejection terminates the wait immediately, and late receipts from prior requests cannot satisfy it', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-receipts-'));
  const output = path.join(dir, 'trace.jsonl');
  try {
    const client = new PlaytestClient('ws://127.0.0.1:19810/ws', 'test', output);
    client.record('received', { type: 'packet', packet: 'LoginSuccess', payload: { characters: [] } });
    client.ws = { send() { client.record('received', { type: 'error', message: 'invalid password' }); } };
    await assert.rejects(client.request({ type: 'login', accountId: 'pttest', password: 'x'.repeat(20) }, 'LoginSuccess', 100), /Gateway rejected login/);
    await client.writeQueue;
  } finally { await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; }); await fs.rmdir(dir); }
});

test('a rejected StartGame is reported directly and does not attempt an in-game logout', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-startgame-'));
  const output = path.join(dir, 'trace.jsonl');
  try {
    const client = new PlaytestClient('ws://127.0.0.1:19810/ws', 'test', output);
    const sent = [];
    client.ws = { readyState: 1, send(raw) {
      const command = JSON.parse(raw); sent.push(command.type);
      if (command.type === 'login') client.observe({ type: 'packet', packet: 'LoginSuccess', payload: {
        characters: [{ index: 3, name: 'Atest123' }],
      } });
      else if (command.type === 'startGame') client.observe({ type: 'error', message: 'character is already online or route lease is unavailable' });
    }, close() { this.readyState = 3; } };
    await assert.rejects(usingClient(client, () => authenticate(client,
      { accountId: 'ptatest123', name: 'Atest123', password: 'a'.repeat(20) }, false)), /Gateway rejected startGame: character is already online/);
    assert.equal(client.inGame, false);
    assert.deepEqual(sent, ['login', 'startGame']);
  } finally { await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; }); await fs.rmdir(dir); }
});

test('cleanup failure cannot replace the initial command failure', async () => {
  const failure = new Error('StartGame rejected');
  await assert.rejects(usingClient({ close: async () => { throw new Error('LogOut timed out'); } }, async () => { throw failure; }), error => {
    assert.equal(error.message, 'StartGame rejected');
    assert.equal(error.cleanupError, 'LogOut timed out');
    return true;
  });
});
