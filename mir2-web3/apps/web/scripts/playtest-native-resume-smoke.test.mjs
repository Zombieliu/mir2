import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { NativeResumeClient, assertReplayRejected, assertResumeEvidence,
  matchesOwnerStep, resumeFingerprint, validateResumeControl } from './playtest-native-resume-smoke.mjs';

const ticket = Buffer.alloc(32, 17).toString('base64url');
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
