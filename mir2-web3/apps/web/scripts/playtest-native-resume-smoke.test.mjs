import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { NativeResumeClient, assertReplayRejected, assertResumeEvidence,
  matchesOwnerStep, resumeFingerprint, validateResumeControl, resumeReadinessState,
  waitForRetainedReadiness, fetchResumeHealth } from './playtest-native-resume-smoke.mjs';

const ticket = Buffer.alloc(32, 17).toString('base64url');
const emptyCapacity = { currentWsConnections: 0, currentActiveSessions: 0, currentReconnectLeases: 0,
  currentLoginInFlight: 0, currentNewCharacterInFlight: 0, currentStartGameInFlight: 0 };
const health = overrides => ({ capacity: { ...emptyCapacity, ...overrides } });
const liveTicket = expiresAtMs => ({ credential: ticket, generation: 1, expiresAtMs });

test('retained readiness retries only real preparing counters and never sends or consumes a resume ticket', async () => {
  let clock = 1000, calls = 0;
  const observed = [];
  const result = await waitForRetainedReadiness({ endpoint: 'ws://127.0.0.1:17410/ws',
    baseline: emptyCapacity, ticket: liveTicket(30000), ticketUseCount: 0,
    now: () => clock, pause: async ms => { clock += ms; }, onSample: sample => observed.push(sample),
    fetchHealth: async () => ++calls === 1
      ? health({ currentWsConnections: 1, currentActiveSessions: 1 })
      : health({ currentActiveSessions: 1, currentReconnectLeases: 1 }) });
  assert.equal(calls, 2);
  assert.equal(result.elapsedMs, 250);
  assert.equal(result.ticketUseCount, 0);
  assert.equal(observed.length, 2);
  assert.deepEqual(observed.map(value => value.currentReconnectLeases), [0, 1]);
  assert.doesNotMatch(JSON.stringify(result), new RegExp(ticket));
});

test('readiness timeout, credential expiry and discarded owner never invent successful resume readiness', async () => {
  for (const expiresAtMs of [30000, 1750]) {
    let clock = 1000, calls = 0;
    await assert.rejects(waitForRetainedReadiness({ endpoint: 'ws://127.0.0.1:17410/ws',
      baseline: emptyCapacity, ticket: liveTicket(expiresAtMs), ticketUseCount: 0, timeoutMs: 1000,
      now: () => clock, pause: async ms => { clock += ms; },
      fetchHealth: async () => { calls++; return health({ currentActiveSessions: 1 }); } }),
    expiresAtMs === 30000 ? /timed out/ : /expired/);
    assert.ok(calls <= 4, 'only the bounded preparing polls are eligible');
  }
  let calls = 0;
  await assert.rejects(waitForRetainedReadiness({ endpoint: 'ws://127.0.0.1:17410/ws',
    baseline: emptyCapacity, ticket: liveTicket(30000), ticketUseCount: 0, now: () => 1000,
    fetchHealth: async () => { calls++; return health({}); } }), /sole original disconnected owner/);
  assert.equal(calls, 1, 'discarded owner is terminal, not a transient retry');
});

test('invalid, expired or used tickets and nonisolated baselines fail before any readiness request', async () => {
  let calls = 0;
  const base = { endpoint: 'ws://127.0.0.1:17410/ws', baseline: emptyCapacity,
    ticket: liveTicket(30000), ticketUseCount: 0, now: () => 1000,
    fetchHealth: async () => { calls++; return health({ currentActiveSessions: 1, currentReconnectLeases: 1 }); } };
  for (const override of [{ ticket: { ...liveTicket(30000), credential: 'invalid' } },
    { ticket: liveTicket(999) }, { ticket: { ...liveTicket(30000), generation: 0 } },
    { ticketUseCount: 1 }, { baseline: { ...emptyCapacity, currentActiveSessions: 1 } }, { timeoutMs: 10001 }]) {
    await assert.rejects(waitForRetainedReadiness({ ...base, ...override }));
  }
  assert.equal(calls, 0);
});

test('missing counters, concurrent users and public HTTP authentication failures are never retried', async () => {
  assert.throws(() => resumeReadinessState({ capacity: {} }), /complete nonnegative/);
  for (const overrides of [{ currentActiveSessions: 2 }, { currentActiveSessions: 1, currentReconnectLeases: 2 },
    { currentActiveSessions: 1, currentWsConnections: 2 }, { currentActiveSessions: 1, currentLoginInFlight: 1 }]) {
    let calls = 0;
    await assert.rejects(waitForRetainedReadiness({ endpoint: 'ws://127.0.0.1:17410/ws',
      baseline: emptyCapacity, ticket: liveTicket(30000), ticketUseCount: 0, now: () => 1000,
      fetchHealth: async () => { calls++; return health(overrides); } }), /sole original disconnected owner/);
    assert.equal(calls, 1);
  }
  const originalFetch = globalThis.fetch;
  let fetchCalls = 0;
  globalThis.fetch = async () => { fetchCalls++; return { ok: false, status: 401 }; };
  try {
    await assert.rejects(fetchResumeHealth('ws://127.0.0.1:17410/ws'), /HTTP rejected: 401/);
    assert.equal(fetchCalls, 1);
  } finally { globalThis.fetch = originalFetch; }
});
async function removeFixtureDirectory(directory) {
  const resolved = path.resolve(directory);
  assert.equal(path.dirname(resolved), path.resolve(os.tmpdir()));
  assert.match(path.basename(resolved), /^playtest-native-(resume|loss)-test-/);
  await fs.rm(resolved, { recursive: true, force: true });
}
const snapshot = () => ({ playerObjectId: 1000, mapFileName: '0', gold: 20,
  entities: [{ objectId: 1000, kind: 'player', name: 'ActorB', x: 302, y: 626, direction: 'Right' }],
  inventoryItems: [{ uniqueId: 7, name: 'Drug', count: 4, slot: 2 }], equipmentItems: [] });
const evidence = () => ({ before: resumeFingerprint(snapshot(), 'ActorB'), after: resumeFingerprint(snapshot(), 'ActorB'),
  characterIndex: 1, resumed: { type: 'sessionResumed', protocol: 'nativeResumeV1', characterIndex: 1, generation: 4, sequence: 18 },
  firstGeneration: 3, rotatedGeneration: 4, ticketChanged: true, snapshotSequence: 19,
  sentTypes: ['clientVersion', 'clientCapabilities', 'resumeSession'] });

test('resume controls forbid account overrides, debug commands and malformed secrets', () => {
  assert.doesNotThrow(() => validateResumeControl({ type: 'clientCapabilities', capabilities: ['nativeResumeV1'] }));
  assert.doesNotThrow(() => validateResumeControl({ type: 'resumeSession', credential: ticket }));
  for (const input of [{ type: 'resumeSession', credential: ticket, accountId: 'demo' },
    { type: 'resumeSession', credential: ticket, characterIndex: 1 },
    { type: 'resumeSession', credential: 'bad-secret' }, { type: 'moveTo', x: 1, y: 2 },
    { type: 'clientCapabilities', capabilities: ['unknown'] }]) assert.throws(() => validateResumeControl(input));
});

test('credential receipt and replay traces never contain the raw ticket', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-native-resume-test-'));
  const trace = path.join(directory, 'trace.jsonl');
  try {
    const client = new NativeResumeClient('ws://127.0.0.1:20110/ws', 'test', trace);
    client.observe({ type: 'resumeCredential', protocol: 'nativeResumeV1', credential: ticket, generation: 3, expiresAtMs: Date.now() + 30000 });
    let sent;
    client.ws = { send(value) { sent = JSON.parse(value); } };
    client.control({ type: 'resumeSession', credential: ticket });
    await client.writeQueue;
    assert.equal(client.nativeTicket.credential, ticket, 'ticket is usable only in memory');
    assert.equal(sent.credential, ticket, 'real wire transport receives the actual ticket');
    assert.equal(JSON.stringify(client.events).includes(ticket), false);
    assert.equal((await fs.readFile(trace, 'utf8')).includes(ticket), false);
    assert.equal(client.events[0].credential, '[redacted]');
    assert.equal(client.events[1].credential, '[redacted]');
  } finally { await removeFixtureDirectory(directory); }
});

test('resume requires bound character, rotated generation and a later authoritative snapshot', () => {
  assert.doesNotThrow(() => assertResumeEvidence(evidence()));
  for (const mutate of [value => { value.resumed.characterIndex = 0; }, value => { value.resumed.generation = 3; },
    value => { value.rotatedGeneration = 5; }, value => { value.ticketChanged = false; },
    value => { value.snapshotSequence = 17; }]) {
    const value = evidence(); mutate(value); assert.throws(() => assertResumeEvidence(value));
  }
});

test('ordinary relogin is rejected as evidence of native resume', () => {
  for (const type of ['login', 'startGame', 'newAccount', 'newCharacter']) {
    const value = evidence(); value.sentTypes.push(type); assert.throws(() => assertResumeEvidence(value));
  }
});

test('one-step movement uses fresh flattened authoritative UserLocation receipts', () => {
  const target = { x: 303, y: 626 };
  const packet = { sequence: 12, packet: 'UserLocation', payload: { x: 303, y: 626, direction: 'Right' } };
  assert.equal(matchesOwnerStep(packet, 11, target), true);
  assert.equal(matchesOwnerStep(packet, 12, target), false);
  assert.equal(matchesOwnerStep({ ...packet, packet: 'ObjectWalk' }, 11, target), false);
  assert.equal(matchesOwnerStep({ ...packet, payload: { x: 302, y: 626 } }, 11, target), false);
});

test('resume detects lost inventory, changed position and duplicate owner', () => {
  const value = evidence(); value.after.inventory.inventory[0].count = 3;
  assert.throws(() => assertResumeEvidence(value));
  const moved = evidence(); moved.after.x += 1; assert.throws(() => assertResumeEvidence(moved));
  const duplicate = snapshot(); duplicate.entities.push({ ...duplicate.entities[0], objectId: 50001 });
  assert.throws(() => resumeFingerprint(duplicate, 'ActorB'));
});

test('replay denial must not expose a resumed or otherwise authenticated owner', () => {
  const rejected = { type: 'resumeRejected', code: 'unavailable', sequence: 20 };
  assert.deepEqual(assertReplayRejected([rejected, { type: 'worldSnapshot', payload: { entities: [] } }]), { code: 'unavailable', receipt: 20 });
  assert.throws(() => assertReplayRejected([]));
  assert.throws(() => assertReplayRejected([rejected, { type: 'sessionResumed' }]));
  assert.throws(() => assertReplayRejected([rejected, { type: 'worldSnapshot', payload: snapshot() }]));
});

test('transport-loss step terminates socket without sending a game logout', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'playtest-native-loss-test-'));
  try {
    const client = new NativeResumeClient('ws://127.0.0.1:20110/ws', 'test', path.join(directory, 'trace.jsonl'));
    client.inGame = true;
    client.ws = { readyState: 1, terminate() { client.closed = true; }, send() { assert.fail('transport loss sent an application command'); } };
    await client.loseTransport();
    assert.equal(client.events[0].type, 'transportTerminated');
    assert.equal(client.events[0].logoutSent, false);
  } finally { await removeFixtureDirectory(directory); }
});
