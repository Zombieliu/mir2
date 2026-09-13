import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { ProtocolClient, startGameBootstrapEvidence } from './protocol-client.mjs';

test('start-game bootstrap accepts the exact personal snapshot when UserInformation is omitted', () => {
  const client = {
    events: [{ sequence: 10, direction: 'received', packet: 'LoginSuccess' }],
    snapshot: {
      playerObjectId: 50_009,
      entities: [{ objectId: 50_009, name: 'JourneyWizard', kind: 'selfPlayer' }],
    },
  };
  assert.deepEqual(startGameBootstrapEvidence(client, 10, 'JourneyWizard'), {
    source: 'personalSnapshot',
    player: client.snapshot.entities[0],
  });
  assert.equal(startGameBootstrapEvidence(client, 10, 'AnotherCharacter'), null);
  client.events.push({ sequence: 11, direction: 'received', packet: 'UserInformation' });
  assert.equal(startGameBootstrapEvidence(client, 10, 'AnotherCharacter').source, 'UserInformation');
});

test('trace append retries a transient Windows busy-file error', async () => {
  let attempts = 0;
  const delays = [];
  const client = new ProtocolClient('ws://127.0.0.1:17810/ws', 'unused.jsonl', {
    appendFile: async () => {
      attempts += 1;
      if (attempts === 1) throw Object.assign(new Error('busy'), { code: 'EBUSY' });
    },
    traceRetryDelay: async milliseconds => { delays.push(milliseconds); },
  });
  client.record('diagnostic', { type: 'retryProbe' });
  await client.writeQueue;
  assert.equal(attempts, 2);
  assert.deepEqual(delays, [25]);
});

test('trace append tolerates sustained Windows scanner share violations', async () => {
  let attempts = 0;
  const delays = [];
  const client = new ProtocolClient('ws://127.0.0.1:17810/ws', 'unused.jsonl', {
    appendFile: async () => {
      attempts += 1;
      if (attempts <= 7) {
        const code = attempts % 2 === 0 ? 'EACCES' : 'EBUSY';
        throw Object.assign(new Error('scanner lock'), { code });
      }
    },
    traceRetryDelay: async milliseconds => { delays.push(milliseconds); },
  });
  client.record('diagnostic', { type: 'sustainedRetryProbe' });
  await client.writeQueue;
  assert.equal(attempts, 8);
  assert.deepEqual(delays, [25, 50, 100, 200, 400, 800, 1_000]);
});

test('trace redacts incoming identity tokens and outgoing credentials', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-trace-redaction-'));
  const output = path.join(directory, 'trace.jsonl');
  try {
    const client = new ProtocolClient('ws://127.0.0.1:17810/ws', output);
    client.record('received', { type: 'identitySession', token: 'fixture-bearer-secret' });
    client.record('sent', { type: 'login', password: 'fixture-password-secret', secretAnswer: 'fixture-answer-secret' });
    await client.writeQueue;
    const contents = await fs.readFile(output, 'utf8');
    assert.doesNotMatch(contents, /fixture-(bearer|password|answer)-secret/);
    assert.equal(client.events[0].token, '[redacted]');
    assert.equal(client.events[1].password, '[redacted]');
    assert.equal(client.events[1].secretAnswer, '[redacted]');
  } finally {
    await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; });
    await fs.rmdir(directory);
  }
});

test('quest definitions survive event eviction and reset for a new character session', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-quest-definition-cache-'));
  const output = path.join(directory, 'trace.jsonl');
  try {
    const client = new ProtocolClient('ws://127.0.0.1:17810/ws', output);
    client.record('received', {
      type: 'packet',
      packet: 'NewQuestInfo',
      payload: { info: { index: 2_100_015, reward_gold: 5_000 } },
    });
    for (let index = 0; index < 10_000; index++) {
      client.record('diagnostic', { type: 'eventEvictionProbe', index });
    }
    await client.writeQueue;

    assert.equal(client.events.some(event => event.packet === 'NewQuestInfo'), false);
    assert.deepEqual(client.questDefinition(2_100_015), {
      index: 2_100_015,
      reward_gold: 5_000,
    });

    client.ws = { send() {} };
    client.send({ type: 'startGame', characterIndex: 2 });
    assert.equal(client.questDefinition(2_100_015), null);
    await client.writeQueue;
  } finally {
    await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; });
    await fs.rmdir(directory);
  }
});

test('self damage requests coalesced authoritative vitals snapshots through the public command allowlist', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-damage-vitals-refresh-'));
  const output = path.join(directory, 'trace.jsonl');
  let now = 1_000;
  let nextTimer = 1;
  const timers = new Map();
  const client = new ProtocolClient('ws://127.0.0.1:17810/ws', output, {
    now: () => now,
    setTimeout: (callback, delayMs) => {
      const id = nextTimer++;
      timers.set(id, { callback, delayMs });
      return id;
    },
    clearTimeout: id => timers.delete(id),
  });
  const wire = [];
  client.ws = { readyState: 1, send: value => wire.push(JSON.parse(value)) };
  client.snapshot = { playerObjectId: 50_000, playerHp: 89, entities: [{ objectId: 50_000, hp: 89 }] };
  try {
    client.observeGatewayMessage({ type: 'packet', packet: 'DamageIndicator', payload: {
      objectId: 50_000, damage: 7, damageType: 0, typed: true,
    } });
    assert.deepEqual(wire, [{ type: 'clientVersion' }]);
    assert.equal(client.snapshot.playerHp, 89, 'damage indicator must not invent an absolute HP delta');

    now = 1_200;
    client.observeGatewayMessage({ type: 'packet', packet: 'DamageIndicator', payload: {
      objectId: 50_000, damage: 9, damageType: 0, typed: true,
    } });
    client.observeGatewayMessage({ type: 'packet', packet: 'DamageIndicator', payload: {
      objectId: 50_000, damage: 4, damageType: 0, typed: true,
    } });
    assert.equal(timers.size, 1, 'damage bursts coalesce behind one trailing refresh');
    assert.equal([...timers.values()][0].delayMs, 300);
    assert.equal(wire.length, 1);

    now = 1_500;
    const trailing = [...timers.entries()][0];
    timers.delete(trailing[0]);
    trailing[1].callback();
    assert.deepEqual(wire, [{ type: 'clientVersion' }, { type: 'clientVersion' }]);

    client.observeGatewayMessage({ type: 'packet', packet: 'DamageIndicator', payload: {
      objectId: 88_001, damage: 99, damageType: 0, typed: true,
    } });
    assert.equal(wire.length, 2, 'damage to another public object must not refresh self vitals');
    await client.writeQueue;
  } finally {
    await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; });
    await fs.rmdir(directory);
  }
});

test('ordinary merchant sale is allowed while QA and direct-state commands remain forbidden', async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-merchant-command-'));
  const output = path.join(directory, 'trace.jsonl');
  const wire = [];
  const client = new ProtocolClient('ws://127.0.0.1:17810/ws', output);
  client.ws = { send: value => wire.push(JSON.parse(value)) };
  try {
    client.send({ type: 'sellItem', uniqueId: 901, count: 1 });
    assert.deepEqual(wire, [{ type: 'sellItem', uniqueId: 901, count: 1 }]);
    assert.throws(() => client.send({ type: 'qa.giveItem', itemIndex: 658 }), /Forbidden journey command/);
    assert.throws(() => client.send({ type: 'moveTo', x: 10, y: 10 }), /Forbidden journey command/);
    await client.writeQueue;
  } finally {
    await fs.unlink(output).catch(error => { if (error.code !== 'ENOENT') throw error; });
    await fs.rmdir(directory);
  }
});
