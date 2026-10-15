import assert from 'node:assert/strict';
import test from 'node:test';
import { refreshCombatWorldSnapshot } from './protocol-refresh.mjs';
import { applyProtocolObservation } from './protocol-observation.mjs';

function clientWithSchedule(schedule) {
  return {
    sequence: 10, sent: [], elapsed: 0,
    snapshot: { playerObjectId: 1000, playerHp: 159, playerMaxHp: 159, entities: [{ objectId: 1000, dead: false, hp: 159 }] },
    events: [{ sequence: 5, direction: 'received', type: 'worldSnapshot' }],
    send(command) { this.sent.push(command); ++this.sequence; },
    async wait(predicate, label, timeout) {
      assert.equal(predicate(), false, 'an old snapshot cannot acknowledge the new probe');
      for (const entry of schedule) {
        if (entry.at > timeout) break;
        this.elapsed = entry.at;
        const event = { ...entry.message, sequence: ++this.sequence, direction: 'received' };
        this.events.push(event);
        if (entry.message.type === 'packet') this.snapshot = applyProtocolObservation(this.snapshot, entry.message);
        if (predicate()) return event;
      }
      this.elapsed = timeout;
      throw new Error(`Timeout waiting for ${label}`);
    },
  };
}

test('a delayed 10.61-second snapshot completes one cooldown probe', async () => {
  const client = clientWithSchedule([
    { at: 2000, message: { type: 'packet', packet: 'KeepAlive' } },
    { at: 6100, message: { type: 'packet', packet: 'KeepAlive' } },
    { at: 10610, message: { type: 'worldSnapshot' } },
  ]);
  await refreshCombatWorldSnapshot(client);
  assert.equal(client.elapsed, 10610);
  assert.deepEqual(client.sent, [{ type: 'clientVersion' }]);
});

test('death interrupts cooldown refresh before the full timeout', async () => {
  const client = clientWithSchedule([{ at: 1100, message: { type: 'packet', packet: 'Death' } }]);
  await assert.rejects(refreshCombatWorldSnapshot(client), /Player died during quest combat/);
  assert.equal(client.elapsed, 1100);
});

test('a missing fresh snapshot still fails at a bounded deadline', async () => {
  const client = clientWithSchedule([{ at: 2000, message: { type: 'packet', packet: 'KeepAlive' } }]);
  await assert.rejects(refreshCombatWorldSnapshot(client), /Timeout waiting for combat cooldown worldSnapshot/);
  assert.equal(client.elapsed, 20000);
  assert.equal(client.sent.length, 1);
});
