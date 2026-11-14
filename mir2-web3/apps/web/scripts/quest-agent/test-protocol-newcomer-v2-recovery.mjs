import assert from 'node:assert/strict';
import test from 'node:test';
import { createV2DefensiveRecovery } from './protocol-newcomer-v2-recovery.mjs';

function fixture({ className = 'Taoist', cooldown = 0, hp = 43, mp = 83, hpMedicine = 0,
  hostiles = [], accepted = true, healingGain = 42, debitMp = true, releaseCooldown = true } = {}) {
  let clock = 0;
  let pendingHealing = false;
  let magicCount = 0;
  let refreshes = 0;
  const actor = { objectId: 1000, kind: 'selfPlayer', class: className, gender: 'Male', level: 16,
    x: 10, y: 10, direction: 'Right', hp, maxHp: 96, dead: false };
  const client = {
    sequence: 0, events: [], sent: [], requested: [], diagnostics: [],
    snapshot: { playerObjectId: actor.objectId, playerHp: hp, playerMaxHp: 96, playerMp: mp, playerMaxMp: 83,
      mapFileName: 'D001', inSafeZone: false, entities: [actor, ...hostiles],
      knownSkills: className === 'Taoist' ? [{ spell: 'Healing', mpCost: 6, cooldownRemainingTicks: cooldown, cooldownRemainingMs: cooldown ? 532 : 0 }] : [],
      inventoryItems: [], equipmentItems: [], beltItems: hpMedicine ? [{ uniqueId: 18, name: '(HP)DrugMedium', quantity: hpMedicine, container: 'belt' }] : [] },
    receive(type, payload, packet = null) {
      this.sequence += 1;
      this.events.push({ sequence: this.sequence, direction: 'received', type, ...(packet ? { packet } : {}), payload: structuredClone(payload) });
    },
    send(command) {
      this.sent.push(structuredClone(command));
      this.sequence += 1;
      if (command.type === 'clientVersion') {
        refreshes += 1;
        if (releaseCooldown && refreshes >= 2 && this.snapshot.knownSkills[0]) this.snapshot.knownSkills[0].cooldownRemainingTicks = 0;
        if (pendingHealing) {
          pendingHealing = false;
          this.snapshot.playerHp = Math.min(96, this.snapshot.playerHp + healingGain);
          this.snapshot.entities[0].hp = this.snapshot.playerHp;
          if (debitMp) this.snapshot.playerMp -= 6;
        }
        this.receive('worldSnapshot', this.snapshot);
      } else if (command.type === 'magic') {
        magicCount += 1;
        pendingHealing = accepted === true;
        this.receive('packet', { objectId: accepted === 'foreign' ? 77 : 1000, spell: 'Healing',
          targetId: 1000, cast: accepted !== false }, 'ObjectMagic');
      }
    },
    async request(command, packet) {
      this.requested.push(structuredClone(command));
      this.send(command);
      const item = this.snapshot.beltItems.find(candidate => candidate.uniqueId === command.uniqueId);
      if (item) item.quantity -= 1;
      this.receive('worldSnapshot', this.snapshot);
      return { packet, payload: { success: true } };
    },
    async wait(predicate, label) {
      if (!predicate()) throw new Error(`Timeout waiting for ${label}`);
      return true;
    },
    record(_direction, payload) { this.diagnostics.push(payload); },
  };
  return { client, recovery: createV2DefensiveRecovery({ now: () => clock, sleep: async ms => { clock += ms; } }),
    magicCount: () => magicCount, refreshes: () => refreshes };
}

test('V2 ordinary preparation and combat sustain remain potion-only', async () => {
  for (const phase of [undefined, 'attack', 'search', 'prepare']) {
    const { client, recovery, magicCount } = fixture();
    await recovery.sustain(client, { hpThreshold: 0.75, mpThreshold: 0.35 }, { phase });
    assert.equal(magicCount(), 0, phase);
  }
});

test('V2 retreat uses normal self-Healing only from observed clearance, with no offense or ACK block', async () => {
  const { client, recovery } = fixture({ hpMedicine: 2 });
  const result = await recovery.sustain(client, {}, { phase: 'retreat' });
  assert.deepEqual(client.sent[0], { type: 'magic', objectId: 1000, spell: 'Healing', direction: 'Right', targetId: 1000,
    x: 10, y: 10, spellTargetLock: true });
  assert.equal(result.classRecovery.pending, true);
  assert.equal(client.requested.length, 0);
  assert.equal(client.sent.some(command => ['attack', 'walk', 'run'].includes(command.type)), false);
});

test('V2 unsafe retreat does not stop for self-Healing or blocking medicine requests', async () => {
  const { client, recovery, magicCount } = fixture({ hpMedicine: 2,
    hostiles: [{ objectId: 240303, kind: 'monster', disposition: 'hostile', x: 11, y: 10, hp: 34, dead: false }] });
  await recovery.sustain(client, {}, { phase: 'retreat' });
  const result = await recovery.recoverAfterUnsafeRetreat(client);
  assert.equal(magicCount(), 0);
  assert.equal(client.requested.length, 0);
  assert.equal(result.status, 'unsafeDeferred');
  assert.equal(client.sent.filter(command => command.type === 'useItem').length, 1);
});

test('V2 post-retreat waits for fresh shared cooldown and verifies owner cast plus exact HP/MP', async () => {
  const { client, recovery, magicCount, refreshes } = fixture({ cooldown: 1 });
  const result = await recovery.recoverAfterUnsafeRetreat(client);
  assert.equal(magicCount(), 1);
  assert.ok(refreshes() >= 3);
  assert.deepEqual(client.sent.slice(0, 2), [{ type: 'clientVersion' }, { type: 'clientVersion' }]);
  assert.equal(result.status, 'healingConfirmed');
  assert.equal(result.heals.length, 1);
  assert.equal(result.heals[0].beforeHp, 43);
  assert.equal(result.heals[0].hp, 85);
  assert.equal(result.heals[0].beforeMp, 83);
  assert.equal(result.heals[0].mp, 77);
  assert.equal(client.sent.some(command => command.type === 'attack'), false);
});

test('V2 persistent shared cooldown exhausts the recovery window without a guessed cast', async () => {
  const { client, recovery, magicCount } = fixture({ cooldown: 1, releaseCooldown: false });
  const result = await recovery.recoverAfterUnsafeRetreat(client);
  assert.equal(result.status, 'cooldownDeferred');
  assert.equal(magicCount(), 0);
  assert.equal(client.sent.every(command => command.type === 'clientVersion'), true);
});

test('V2 foreign and rejected self-magic receipts cannot confirm Healing', async () => {
  for (const accepted of ['foreign', false]) {
    const { client, recovery, magicCount } = fixture({ accepted });
    await assert.rejects(recovery.recoverAfterUnsafeRetreat(client), /accepted defensive self-Healing/);
    assert.equal(magicCount(), 1);
    assert.equal(client.diagnostics.some(event => event.type === 'v2DefensiveHealingConfirmed'), false);
  }
});

test('V2 accepted cast alone cannot substitute for exact HP recovery and mana debit', async () => {
  for (const options of [{ healingGain: 0 }, { debitMp: false }]) {
    const { client, recovery, magicCount } = fixture(options);
    await assert.rejects(recovery.recoverAfterUnsafeRetreat(client), /exact HP recovery and MP debit/);
    assert.equal(magicCount(), 1);
    assert.equal(client.diagnostics.some(event => event.type === 'v2DefensiveHealingConfirmed'), false);
  }
});

test('V2 post-retreat retains the two-cast bound without inventing full health', async () => {
  const { client, recovery, magicCount } = fixture({ healingGain: 1 });
  const result = await recovery.recoverAfterUnsafeRetreat(client);
  assert.equal(magicCount(), 2);
  assert.equal(result.heals.length, 2);
  assert.equal(client.snapshot.playerHp, 45);
  assert.equal(client.snapshot.playerMp, 71);
});

test('V2 recovery never emits Healing for another class, absent MP, or blocking status', async () => {
  for (const options of [{ className: 'Wizard' }, { className: 'Warrior' }, { mp: 0 }, { mp: null }]) {
    const { client, recovery, magicCount } = fixture(options);
    await recovery.recoverAfterUnsafeRetreat(client);
    assert.equal(magicCount(), 0);
  }
  const { client, recovery, magicCount } = fixture();
  client.snapshot.playerPoison = 16;
  await recovery.sustain(client, {}, { phase: 'retreat' });
  await recovery.recoverAfterUnsafeRetreat(client);
  assert.equal(magicCount(), 0);
});

test('V2 a pursuer entering the live margin during refresh prevents a healing wait or cast', async () => {
  const { client, recovery, magicCount } = fixture({ cooldown: 1 });
  const send = client.send.bind(client);
  client.send = command => {
    if (command.type === 'clientVersion') client.snapshot.entities.push({ objectId: 240303, kind: 'monster',
      disposition: 'hostile', x: 12, y: 10, hp: 34, dead: false });
    send(command);
  };
  await recovery.recoverAfterUnsafeRetreat(client);
  assert.equal(magicCount(), 0);
});

test('V2 unknown cooldown and safe-zone labels cannot substitute for recovery evidence', async () => {
  const absent = fixture();
  delete absent.client.snapshot.knownSkills[0].cooldownRemainingTicks;
  await absent.recovery.sustain(absent.client, {}, { phase: 'retreat' });
  await absent.recovery.recoverAfterUnsafeRetreat(absent.client);
  assert.equal(absent.magicCount(), 0);
  const unsafe = fixture({ hostiles: [{ objectId: 240303, kind: 'monster', disposition: 'hostile',
    x: 11, y: 10, hp: 34, dead: false }] });
  unsafe.client.snapshot.inSafeZone = true;
  await unsafe.recovery.sustain(unsafe.client, {}, { phase: 'retreat' });
  await unsafe.recovery.recoverAfterUnsafeRetreat(unsafe.client);
  assert.equal(unsafe.magicCount(), 0);
});
