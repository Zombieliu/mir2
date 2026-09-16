import assert from 'node:assert/strict';
import test from 'node:test';
import { completeFlagObjectives } from './protocol-flag-objectives.mjs';

class FakeClient {
  constructor(snapshot, respond) { this.snapshot = snapshot; this.respond = respond; this.sequence = 0; this.events = []; this.sent = []; }
  send(command) { this.sent.push(command); this.respond(this, command); }
  receive(payload) { this.snapshot = payload; this.events.push({ sequence: ++this.sequence, direction: 'received', type: 'worldSnapshot', payload }); }
  async wait(predicate, label) { const value = predicate(); if (!value) throw new Error(`Fake timeout waiting for ${label}`); return value; }
}

function q111Snapshot(current = 0) {
  return {
    mapFileName: 'D701',
    entities: [{ objectId: 81143, kind: 'npc', name: 'WierdPillar', x: 38, y: 123 }],
    questLog: [{ questId: 111, stage: 'InProgress', objectives: [{ label: 'Find the WierdPillar', current, required: 1, done: current >= 1 }] }],
    activeNpcDialog: null,
  };
}

const q111 = { questId: 111, objectives: { flag: [{ number: 521, message: 'Find the WierdPillar', setters: [{ npc: { objectId: 1143, name: 'WierdPillar', mapFileName: 'D701', position: { x: 38, y: 123 } }, targetSequence: [] }] }] } };

test('q111 pillar uses static metadata only for travel and resolves the live NPC before interaction', async () => {
  const client = new FakeClient(q111Snapshot(), (self, command) => {
    if (command.type === 'interact') self.receive(q111Snapshot(1));
  });
  const navigation = [];
  const travel = [];
  const result = await completeFlagObjectives(client, q111, async (...args) => navigation.push(args), async map => travel.push(map));
  assert.deepEqual(result, [521]);
  assert.deepEqual(navigation, [[{ x: 38, y: 123 }, 1]]);
  assert.deepEqual(travel, []);
  assert.deepEqual(client.sent, [{ type: 'interact', objectId: 81143 }]);
});

test('cross-map setter travels normally and selects only a target present in the live dialog', async () => {
  const routeQuest = { questId: 200, objectives: { flag: [{ number: 9, message: 'Touch Rune', setters: [{ npc: { objectId: 77, name: 'Rune', mapFileName: 'B', position: { x: 4, y: 6 } }, targetSequence: ['@Touch'] }] }] } };
  const client = new FakeClient({ mapFileName: 'A', entities: [], questLog: [{ questId: 200, objectives: [{ label: 'Touch Rune', current: 0, required: 1, done: false }] }] }, (self, command) => {
    if (command.type === 'interact') self.receive({ ...self.snapshot, activeNpcDialog: { npcObjectId: 7077, links: [{ text: 'Touch', target: '@touch' }] } });
    if (command.type === 'selectNpcDialog') self.receive({ ...self.snapshot, activeNpcDialog: null, questLog: [{ questId: 200, objectives: [{ label: 'Touch Rune', current: 1, required: 1, done: true }] }] });
  });
  const traveled = [];
  const result = await completeFlagObjectives(client, routeQuest, async () => {}, async map => {
    traveled.push(map);
    client.snapshot = { ...client.snapshot, mapFileName: map, entities: [{ objectId: 7077, kind: 'npc', name: 'Rune', x: 4, y: 6 }] };
  });
  assert.deepEqual(traveled, ['B']);
  assert.deepEqual(result, [9]);
  assert.deepEqual(client.sent.map(entry => entry.type), ['interact', 'selectNpcDialog']);
  assert.equal(client.sent[1].target, '@touch');
});

test('a static target absent from the live dialog is never sent', async () => {
  const routeQuest = { questId: 201, objectives: { flag: [{ number: 10, message: 'Open Gate', setters: [{ npc: { objectId: 8, name: 'Gate', mapFileName: 'A', position: { x: 1, y: 1 } }, targetSequence: ['@Open'] }] }] } };
  const client = new FakeClient({ mapFileName: 'A', entities: [{ objectId: 8, kind: 'npc', name: 'Gate', x: 1, y: 1 }], questLog: [{ questId: 201, objectives: [{ label: 'Open Gate', current: 0, required: 1, done: false }] }] }, (self, command) => {
    if (command.type === 'interact') self.receive({ ...self.snapshot, activeNpcDialog: { npcObjectId: 8, links: [{ text: 'Exit', target: '@Exit' }] } });
  });
  await assert.rejects(() => completeFlagObjectives(client, routeQuest, async () => {}, async () => {}), /did not advance through a live setter/);
  assert.deepEqual(client.sent, [{ type: 'interact', objectId: 8 }]);
});

test('already-complete objectives produce no travel or commands', async () => {
  const client = new FakeClient(q111Snapshot(1), () => assert.fail('must not send'));
  assert.deepEqual(await completeFlagObjectives(client, q111, async () => assert.fail('must not navigate'), async () => assert.fail('must not travel')), [521]);
});
