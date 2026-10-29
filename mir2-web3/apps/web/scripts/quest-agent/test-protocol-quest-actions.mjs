import assert from 'node:assert/strict';
import test from 'node:test';
import { interactQuest } from './protocol-quest-actions.mjs';

class FakeClient {
  constructor(snapshot, respond) {
    this.snapshot = snapshot;
    this.respond = respond;
    this.sequence = 0;
    this.events = [];
    this.sent = [];
  }
  send(command) {
    this.sent.push(command);
    this.respond(this, command);
  }
  receive(payload) {
    this.snapshot = payload;
    this.events.push({ sequence: ++this.sequence, direction: 'received', type: 'worldSnapshot', payload });
  }
  async wait(predicate, label) {
    const result = predicate();
    if (!result) throw new Error(`Fake timeout waiting for ${label}`);
    return result;
  }
}

const npc = { objectId: 44, kind: 'npc', name: 'Quest Keeper', x: 10, y: 20 };
const routeNpc = { objectId: 44, name: 'Quest_Keeper', position: { x: 10, y: 20 } };

test('NPC accept navigates, opens the actual quest page, and proves ACK plus stage', async () => {
  const client = new FakeClient({ entities: [npc], questLog: [{ questId: 7, stage: 'Available' }] }, (self, command) => {
    if (command.type === 'interact') self.receive({ ...self.snapshot, activeNpcDialog: { npcObjectId: 44, links: [{ text: 'Quest', target: '@quest:show:7' }] } });
    if (command.type === 'selectNpcDialog') self.receive({ ...self.snapshot, activeNpcDialog: { npcObjectId: 44, links: [{ text: 'Accept', target: '@quest:accept:7' }] } });
    if (command.type === 'acceptQuest') self.receive({ ...self.snapshot, questLog: [{ questId: 7, stage: 'InProgress' }], questOperationAck: { operation: 'acceptQuest', requestId: command.requestId, npcIndex: 44, questIndex: 7, success: true } });
  });
  const navigated = [];
  const result = await interactQuest(client, { questId: 7, startNpc: routeNpc }, 'accept', async (...args) => navigated.push(args));
  assert.deepEqual(navigated, [[{ x: 10, y: 20 }, 16]]);
  assert.deepEqual(client.sent.map(entry => entry.type), ['interact', 'selectNpcDialog', 'acceptQuest']);
  assert.equal(result.quest.stage, 'InProgress');
  assert.equal(result.acknowledgement.success, true);
});

test('NPC metadata alone cannot authorize an operation after range navigation', async () => {
  const client = new FakeClient({ entities: [], questLog: [{ questId: 7, stage: 'Available' }] }, () => {
    assert.fail('no command may be sent without the exact live NPC');
  });
  const navigated = [];
  await assert.rejects(
    () => interactQuest(client, { questId: 7, startNpc: routeNpc }, 'accept', async (...args) => navigated.push(args)),
    /live accept NPC/,
  );
  assert.deepEqual(navigated, [[{ x: 10, y: 20 }, 16]]);
  assert.deepEqual(client.sent, []);
});

test('reward definition rejection prevents accepting an otherwise available NPC quest', async () => {
  const client = new FakeClient({ entities: [npc], questLog: [{ questId: 7, stage: 'Available' }] }, (self, command) => {
    if (command.type === 'interact') self.receive({ ...self.snapshot, activeNpcDialog: { npcObjectId: 44, links: [{ text: 'Accept', target: '@quest:accept:7' }] } });
  });
  await assert.rejects(() => interactQuest(client, { questId: 7, startNpc: routeNpc }, {
    type: 'accept',
    verifyDefinition: async (actualClient, questId, kind) => {
      assert.equal(actualClient, client);
      assert.equal(questId, 7);
      assert.equal(kind, 'accept');
      assert.equal(client.snapshot.activeNpcDialog.npcObjectId, 44);
      throw new Error('reward definition mismatch');
    },
  }, async () => {}), /reward definition mismatch/);
  assert.deepEqual(client.sent.map(entry => entry.type), ['interact']);
  assert.equal(client.snapshot.questLog[0].stage, 'Available');
});

test('NPC finish preserves the selected reward index and requires its actual dialog link', async () => {
  const client = new FakeClient({ entities: [npc], questLog: [{ questId: 9, stage: 'ReadyToTurnIn' }] }, (self, command) => {
    if (command.type === 'interact') self.receive({ ...self.snapshot, activeNpcDialog: { npcObjectId: 44, links: [{ text: 'Sword', target: '@quest:finish:9:2' }] } });
    if (command.type === 'finishQuest') self.receive({ ...self.snapshot, questLog: [{ questId: 9, stage: 'Completed' }], questOperationAck: { operation: 'finishQuest', requestId: command.requestId, questIndex: 9, selectedItemIndex: 2, success: true } });
  });
  const result = await interactQuest(client, { questId: 9, finishNpc: routeNpc }, { type: 'finish', selectedItemIndex: 2 }, async () => {});
  assert.equal(result.command.selectedItemIndex, 2);
  assert.equal(result.quest.stage, 'Completed');
});

test('Diary metadata permits a no-NPC operation without navigation or interaction', async () => {
  const client = new FakeClient({ entities: [], questLog: [{ questId: 22, stage: 'Available' }] }, (self, command) => {
    if (command.type === 'acceptQuest') self.receive({ ...self.snapshot, questLog: [{ questId: 22, stage: 'ReadyToTurnIn' }], questOperationAck: { operation: 'acceptQuest', requestId: command.requestId, npcIndex: 0, questIndex: 22, success: true } });
  });
  let navigated = false;
  const result = await interactQuest(client, { questId: 22, startNpc: null, specialHandlers: ['quest-diary-accept'] }, 'accept', async () => { navigated = true; });
  assert.equal(navigated, false);
  assert.deepEqual(client.sent.map(entry => entry.type), ['acceptQuest']);
  assert.equal(result.command.npcIndex, 0);
});

test('missing NPC and Diary metadata fails closed', async () => {
  const client = new FakeClient({ entities: [], questLog: [] }, () => {});
  await assert.rejects(() => interactQuest(client, { questId: 30, startNpc: null }, 'accept', async () => {}), /neither NPC nor Diary metadata/);
  assert.deepEqual(client.sent, []);
});

test('an unrelated or rejected acknowledgement cannot prove an operation', async () => {
  const client = new FakeClient({ entities: [], questLog: [{ questId: 22, stage: 'Available' }] }, (self, command) => {
    self.receive({ ...self.snapshot, questLog: [{ questId: 22, stage: 'InProgress' }], questOperationAck: { operation: 'acceptQuest', requestId: `${command.requestId}-wrong`, npcIndex: 0, questIndex: 22, success: true } });
  });
  await assert.rejects(() => interactQuest(client, { questId: 22, startInDiary: true }, 'accept', async () => {}), /acknowledgement/);
});
