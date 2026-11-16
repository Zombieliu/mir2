import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { EventEmitter } from 'node:events';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { hash, MAP_SHA, equipmentFingerprint } from './contract.mjs';
import { runComparison } from './driver.mjs';
import { COMBAT_CONTRACT_SHA256, POTION_POLL_MS, ModernCombatClient, validateCombatPlan, beginCombatTracking,
  offerCombatAction, observeCombatActions, pollPotions, scoreCombatWindows, createMeleeEngine, verifyCombatFixture } from './combat.mjs';

const contractBytes = await fs.readFile(new URL('../combat-contract.v1.json', import.meta.url));
const ruleBytes = await fs.readFile(new URL('../workload.v2.json', import.meta.url)), rules = JSON.parse(ruleBytes);
function plan(population = 2) {
  const p = { schema: 'mir2.comparable.plan.v1', qualificationLayer: 'game-core-local', scenario: 'dispersed-melee',
    purpose: population === 2 ? 'functional' : 'capacity', population, durationMs: population === 2 ? 60000 : 300000,
    capacityClaimAllowed: false, definitionSha256: hash(ruleBytes), generatorSourceSha256: 'a'.repeat(64),
    planId: '0123456789abcdefabcd', map: structuredClone(rules.map), navigationSha256: 'b'.repeat(64),
    actorTemplate: structuredClone(rules.actorTemplate), resourceContract: structuredClone(rules.resourceContract),
    causality: structuredClone(rules.causality), thresholds: structuredClone(rules.thresholds),
    combatContractSha256: COMBAT_CONTRACT_SHA256, combatContractCanonicalSha256: hash(JSON.parse(contractBytes)), combatContract: JSON.parse(contractBytes),
    scheduling: { version: 2, chatPhaseRule: 'same-primary-phase-with-midpoint-offset', primaryIntervalMs: 1200,
      minimumSameActorOfferGapMs: 600, chatOffsetWithinPrimaryIntervalMs: 600 },
    assessment: { windowMs: 60000, perActor: true, allCompleteWindowsRequired: true, failedActionCanBecomeCompleted: false,
      notSentIsTimeout: false, notSentIsCompleted: false, safetyOrEvidenceFailureIsHardFailure: true },
    equipmentActorOrdinals: population === 2 ? [0, 1] : [0, 1, 2], equipmentRoundsPerActor: 2, actors: [], actions: [] };
  const groups = []; for (let i = 0; i < population;) { const size = population - i === 3 ? 3 : 2; groups.push(Array.from({ length: size }, () => i++)); }
  for (const [group, ordinals] of groups.entries()) for (const [index, ordinal] of ordinals.entries()) {
    const start = { x: 10 + (group % 10) * 45 + (index === 1 ? 1 : 0), y: 10 + Math.floor(group / 10) * 45 + (index === 2 ? 1 : 0) };
    p.actors.push({ ordinal, group, peerOrdinals: ordinals.filter(i => i !== ordinal), start, lane: [start, { ...start }],
      attackTargetOrdinal: ordinals[(index + 1) % ordinals.length] });
    const actions = [], phase = Math.floor(ordinal * 1200 / population);
    for (let dueMs = phase; dueMs < p.durationMs; dueMs += 1200) actions.push({ actorOrdinal: ordinal, kind: 'attack', dueMs, deadlineMs: dueMs + 2500, targetActorOrdinal: ordinals[(index + 1) % ordinals.length] });
    for (let dueMs = 15000 + phase; dueMs < p.durationMs; dueMs += 30000) actions.push({ actorOrdinal: ordinal, kind: 'chat', dueMs, deadlineMs: dueMs + 2500 });
    actions.sort((a, b) => a.dueMs - b.dueMs || a.kind.localeCompare(b.kind));
    for (const [i, action] of actions.entries()) { action.ticketSequence = i + 1; if (action.kind === 'chat') {
      action.nonce = `m2cmp:${p.planId}:${ordinal}:${i + 1}`; action.nonceSha256 = hash(action.nonce);
    } p.actions.push(action); }
  }
  p.actors.sort((a, b) => a.ordinal - b.ordinal); p.actions.sort((a, b) => a.dueMs - b.dueMs || a.actorOrdinal - b.actorOrdinal || a.ticketSequence - b.ticketSequence);
  return p;
}
function snapshot(ordinal, start, hp = 674) {
  return { mapFileName: 'DogYoHyun', playerObjectId: 1000, playerHp: hp, playerMaxHp: 674, playerMp: 151, gold: 0, inventoryCapacity: 46, maxBagSlots: 40,
    inSafeZone: false, knownSkills: [], activeBuffs: [], heroVitals: null, heroInventoryItems: [], heroEquipmentItems: [],
    stage5Systems: { hero: null, intelligentCreatures: [], summonedIntelligentCreatureType: 99 },
    entities: [{ objectId: 1000, kind: 'player', name: `Hero${ordinal}`, class: 'Warrior', gender: 'Male', level: 40, dead: false, direction: 'Down', ...start }],
    inventoryItems: Array.from({ length: 9 }, (_, slot) => ({ slot, uniqueId: 20000 + ordinal * 100 + slot, key: 'crystal-item-662', quantity: 20 })),
    equipmentItems: [['weapon', 221, 4000, [[4, 2], [5, 4]]], ['armour', 317, 5000, [[0, 2], [1, 2], [3, 1]]],
      ['braceletLeft', 482, 7000, [[10, 2]]], ['braceletRight', 482, 7000, [[10, 2]]]].map(([slot, index, durability, stats], i) => ({
        slot, uniqueId: 10000 + ordinal * 100 + i, key: `crystal-item-${index}`, quantity: 1,
        durabilityCurrent: durability, durabilityMax: durability, addedAttack: 0, addedDefence: 0, addedLuck: 0,
        tooltipSource: { info: { item_index: index, item_set: 0, unique: 0, stats: stats.map(([stat, value]) => ({ stat, value })) },
          userItem: { unique_id: 10000 + ordinal * 100 + i, item_index: index, count: 1, current_dura: durability, max_dura: durability,
            added_stats: [], awake_values: [], slots: [], refined_value: 0, awake_type: 0, refine_added: 0, gem_count: 0 } } })), beltItems: [] };
}
function fixture({ hp = 674, population = 2 } = {}) {
  let now = 0; const p = plan(population), rows = [], commands = [];
  const clients = p.actors.map(a => {
    const c = new ModernCombatClient({ url: 'ws://127.0.0.1:19010/ws', actor: { ordinal: a.ordinal, name: `Hero${a.ordinal}` }, connection: a.ordinal + 1,
      now: () => now, emit: r => rows.push(structuredClone(r)) });
    const ui = c.observe({ type: 'packet', packet: 'UserInformation', payload: { objectId: 1000, name: c.actor.name, hp, mp: 151,
      hasHero: false, magicCount: 0, intelligentCreatureCount: 0, creatureSummoned: false } });
    c.observe({ type: 'worldSnapshot', payload: snapshot(a.ordinal, a.start, hp) }); c.inGame = true;
    c.bootstrapProof = { user: { receiveSeq: ui.receiveSeq } };
    c.ws = { readyState: 1, send: s => commands.push({ actorOrdinal: a.ordinal, atMs: now, command: JSON.parse(s) }) };
    beginCombatTracking(c); return c;
  });
  for (const c of clients) for (const other of clients) if (c !== other) c.sharedPlayers.set(other.actor.name, 50050 + other.ordinal);
  const packet = (ordinal, name, payload, time = now) => { now = time; return clients[ordinal].observe({ type: 'packet', packet: name, payload }); };
  return { clients, p, rows, commands, packet, clock: t => { now = t; }, now: () => now };
}
function offer(f, action = f.p.actions[0]) { return offerCombatAction(f.clients[action.actorOrdinal], f.clients, f.p, action, 0, f.now()); }
function animations(f, { includeLast = true } = {}) {
  f.packet(0, 'UserLocation', { x: 10, y: 10, direction: 'Right' }, 10);
  const peers = f.p.actors[0].peerOrdinals;
  for (const peer of includeLast ? peers : peers.slice(0, -1)) f.packet(peer, 'ObjectAttack', { objectId: 50050, location: { x: 10, y: 10 }, direction: 'Right', spell: 0 }, 20);
}
function hit(f, { hp = 654, amount = 20, attacker = 50050, victim = 1000, time = 30 } = {}) {
  // Real shared PvP order: exact HealthChanged operation precedes the public hit.
  f.packet(1, 'HealthChanged', { hp, mp: 151, typed: true }, time);
  f.packet(1, 'ObjectStruck', { objectId: victim, attackerId: attacker, location: { x: 11, y: 10 }, direction: 'Down' }, time + 1);
  f.packet(1, 'DamageIndicator', { object_id: victim, damage: amount, damage_type: 0, typed: true }, time + 2);
}
const terminal = f => f.rows.find(r => r.event === 'actionTerminal' && r.kind === 'attack');

test('external exact combat bytes and embedded contract are mandatory; corrected 104 offers accepted', () => {
  assert.equal(validateCombatPlan(plan(), contractBytes).actions.length, 104);
  for (const mutate of [p => { p.combatContract.potions.minIntervalMs = 1; }, p => { p.combatContractSha256 = '0'.repeat(64); },
    p => { p.thresholds.attackDeadlineMs = 5000; }, p => { p.scheduling.minimumSameActorOfferGapMs = 10; }, p => { p.actors[1].attackTargetOrdinal = 1; },
    p => { p.actions.pop(); }]) { const p = plan(); mutate(p); assert.throws(() => validateCombatPlan(p, contractBytes)); }
  assert.throws(() => validateCombatPlan(plan()), /external/);
  assert.throws(() => validateCombatPlan(plan(), Buffer.concat([contractBytes, Buffer.from('\n')])), /external/);
});
test('melee CLI requires external contract before reading inputs and dense CLI rejects the combat flag', () => {
  const args = ['--plan', 'C:/comparison/plan.json', '--cohort', 'C:/comparison/private/actors.ready.json',
    '--runtime-receipt', 'C:/comparison/runtime.json', '--ledger', 'C:/comparison/private/ledger.json',
    '--output', 'C:/comparison/fresh', '--ws-module', 'C:/comparison/ws.cjs'];
  const melee = spawnSync(process.execPath, [fileURLToPath(new URL('./melee-driver.mjs', import.meta.url)), ...args], { encoding: 'utf8', windowsHide: true });
  assert.equal(melee.status, 1); assert.match(melee.stderr, /Absolute --combat-contract required/);
  const dense = spawnSync(process.execPath, [fileURLToPath(new URL('./driver.mjs', import.meta.url)), ...args, '--combat-contract', 'C:/comparison/contract.json'],
    { encoding: 'utf8', windowsHide: true });
  assert.equal(dense.status, 1); assert.match(dense.stderr, /Unknown\/duplicate option/);
});
test('all 100 actors have 52 offers each minute and 600ms midpoint separation', () => {
  const p = validateCombatPlan(plan(100), contractBytes);
  for (const a of p.actors) {
    const actions = p.actions.filter(x => x.actorOrdinal === a.ordinal);
    assert.ok(actions.slice(1).every((x, i) => x.dueMs - actions[i].dueMs >= 600));
    for (let minute = 0; minute < 5; minute++) assert.equal(actions.filter(x => Math.floor(x.dueMs / 60000) === minute).length, 52);
  }
});
test('combat whitelist uses ordinary direction/mode/UID and rejects raw target/debug commands', () => {
  const f = fixture(), c = f.clients[0];
  c.send({ type: 'changeAMode', mode: 5 }); c.send({ type: 'attackDirection', direction: 'Right', spell: 0 });
  for (const command of [{ type: 'attack', objectId: 50051 }, { type: 'attackV2', objectId: 50051 }, { type: 'moveTo', x: 11, y: 10 },
    { type: 'attackDirection', direction: 'Right', spell: 1 }, { type: 'changeAMode', mode: 0 }, { type: 'useItem', key: 'potion' }]) assert.throws(() => c.send(command));
  assert.deepEqual(f.commands.map(r => r.command.type), ['changeAMode', 'attackDirection']);
});
test('fresh combat baseline rejects skills, buffs, heroes, pets and bonus gear without clearing them', () => {
  const f = fixture(); verifyCombatFixture(f.clients[0], f.p);
  for (const mutate of [s => s.knownSkills.push({ spell: 1 }), s => s.activeBuffs.push({ type: 1 }),
    s => { s.heroVitals = { hp: 10 }; }, s => s.stage5Systems.intelligentCreatures.push({ petType: 1 }),
    s => { s.equipmentItems[0].tooltipSource.info.stats.push({ stat: 19, value: 1 }); },
    s => { s.equipmentItems[0].tooltipSource.userItem.added_stats.push({ stat: 4, value: 1 }); }]) {
    const g = fixture(); mutate(g.clients[0].latestSnapshotEvent.payload);
    const before = hash(g.clients[0].latestSnapshotEvent.payload); assert.throws(() => verifyCombatFixture(g.clients[0], g.p));
    assert.equal(hash(g.clients[0].latestSnapshotEvent.payload), before);
  }
});
test('owner location and every peer animation alone cannot prove a hit', () => {
  const f = fixture(); offer(f); animations(f); observeCombatActions(f.clients, f.now());
  assert.ok(f.clients[0].gate.pending); assert.equal(terminal(f), undefined);
});
test('modern HP-before-Struck order binds exact attacker/victim and positive raw damage', () => {
  const f = fixture(); offer(f); animations(f); hit(f); observeCombatActions(f.clients, f.now());
  const r = terminal(f); assert.equal(r.passed, true); assert.deepEqual(r.outcome, { kind: 'hit', damageMagnitude: 20, hpBefore: 674, hpAfter: 654, receipts: undefined });
  assert.equal(r.actualTarget.objectId, 50051); assert.equal(r.actualTarget.ownerObjectId, 1000);
  assert.equal(r.receipts.find(r => r.packet === 'DamageIndicator').payload.object_id, 1000);
  assert.ok(r.receipts.every(r => Number.isInteger(r.connection) && Number.isInteger(r.receiveSeq)));
});
test('a three-person group requires both declared peers; target evidence cannot substitute for the other peer', () => {
  const f = fixture({ population: 3 }); offer(f); animations(f, { includeLast: false }); hit(f); observeCombatActions(f.clients, f.now());
  assert.equal(terminal(f), undefined); assert.ok(f.clients[0].gate.pending);
  f.packet(2, 'ObjectAttack', { objectId: 50050, location: { x: 10, y: 10 }, direction: 'Right', spell: 0 }, 40);
  observeCombatActions(f.clients, f.now()); assert.equal(terminal(f).passed, true);
  assert.deepEqual(terminal(f).expectedPeerOrdinals, [1, 2]);
});
test('interleaved healing and exact damage remain distinct operations even if net HP rises', () => {
  const f = fixture({ hp: 450 }); offer(f); animations(f);
  f.packet(1, 'HealthChanged', { hp: 500, mp: 151 }, 25); hit(f, { hp: 480 }); observeCombatActions(f.clients, f.now());
  assert.equal(terminal(f).passed, true); assert.equal(terminal(f).outcome.hpBefore, 500); assert.equal(terminal(f).outcome.hpAfter, 480);
  assert.equal(f.clients[1].combat.health.length, 2);
});
test('wrong owner namespace never substitutes for target local victim identity', () => {
  const f = fixture(); offer(f); animations(f); hit(f, { victim: 50051 }); observeCombatActions(f.clients, f.now());
  assert.equal(terminal(f), undefined); assert.ok(f.clients[0].gate.pending);
});
test('wrong attacker or competing target damage is an evidence hard failure', () => {
  for (const kind of ['attacker', 'hp', 'damage']) {
    const f = fixture(); offer(f); animations(f); hit(f, kind === 'attacker' ? { attacker: 59999 } : {});
    if (kind === 'hp') f.packet(1, 'HealthChanged', { hp: 653, mp: 151 }, 40);
    if (kind === 'damage') f.packet(1, 'DamageIndicator', { object_id: 1000, damage: 1, damage_type: 0 }, 40);
    observeCombatActions(f.clients, f.now()); assert.equal(f.clients[0].combat.hardFailure, true); assert.equal(terminal(f), undefined);
  }
});
test('unpaired HP drop or wrong amount cannot complete the action', () => {
  const f = fixture(); offer(f); animations(f); f.packet(1, 'HealthChanged', { hp: 654, mp: 151 }, 25); observeCombatActions(f.clients, f.now());
  assert.equal(terminal(f), undefined); hit(f, { hp: 654, amount: 21 }); observeCombatActions(f.clients, f.now());
  assert.equal(f.clients[0].combat.hardFailure, true);
});
test('explicit uncontested miss and attacker-bound zero block are valid distinct terminals', () => {
  for (const type of [0, 1]) {
    const f = fixture(); offer(f); animations(f);
    if (!type) f.packet(1, 'ObjectStruck', { objectId: 1000, attackerId: 50050 }, 30);
    f.packet(1, 'DamageIndicator', { object_id: 1000, damage: 0, damage_type: type }, 31); observeCombatActions(f.clients, f.now());
    assert.equal(terminal(f).passed, true); assert.equal(terminal(f).outcome.kind, type ? 'explicitMiss' : 'explicitZeroBlock');
  }
});
test('miss with a competing animation aimed at the victim is not uncontested', () => {
  const f = fixture(); offer(f); animations(f);
  f.packet(1, 'ObjectAttack', { objectId: 59999, location: { x: 11, y: 11 }, direction: 'Up', spell: 0 }, 30);
  f.packet(1, 'DamageIndicator', { object_id: 1000, damage: 0, damage_type: 1 }, 31); observeCombatActions(f.clients, f.now());
  assert.equal(f.clients[0].combat.hardFailure, true); assert.equal(terminal(f), undefined);
});
test('timed out attack fences the next primary; unique late evidence cannot restore green', () => {
  const f = fixture(); offer(f); animations(f); observeCombatActions(f.clients, 2500); f.clock(2600);
  offer(f, f.p.actions.find(a => a.actorOrdinal === 0 && a.ticketSequence === 2));
  assert.equal(f.commands.filter(r => r.command.type === 'attackDirection').length, 1);
  hit(f, { time: 2601 }); observeCombatActions(f.clients, f.now());
  assert.equal(terminal(f).passed, false); assert.equal(terminal(f).timedOut, true); assert.equal(f.clients[0].gate.pending, null);
});
test('observer connection replacement cannot supply old action evidence', () => {
  const f = fixture(); offer(f); animations(f); hit(f);
  f.clients[1] = Object.assign(Object.create(Object.getPrototypeOf(f.clients[1])), f.clients[1], { connection: 99 });
  observeCombatActions(f.clients, f.now());
  assert.equal(f.clients[0].combat.hardFailure, true); assert.equal(terminal(f), undefined);
});
test('potion starts only at HP <=70%, idle, polls25ms and preserves the shared pending gate', () => {
  assert.equal(POTION_POLL_MS, 25); const f = fixture(); pollPotions(f.clients, 0); assert.equal(f.commands.length, 0);
  f.packet(0, 'HealthChanged', { hp: 450, mp: 151 }, 25); pollPotions(f.clients, 25);
  assert.equal(f.commands.length, 1); assert.deepEqual(f.commands[0].command, { type: 'useItem', uniqueId: 20000, grid: 'inventory' });
  offer(f); assert.equal(f.commands.length, 1); assert.ok(f.rows.some(r => r.event === 'notSent' && r.reason === 'previousActionPending'));
});
function potionResult(f, quantity = 19, time = 60) {
  f.packet(0, 'UseItem', { uniqueId: 20000, grid: 'Inventory', success: true }, time - 10);
  const s = snapshot(0, f.p.actors[0].start, 450); s.inventoryItems[0].quantity = quantity;
  f.clock(time); f.clients[0].observe({ type: 'worldSnapshot', payload: s }); observeCombatActions(f.clients, f.now());
}
test('UseItem ACK and arbitrary healing are insufficient; exact post-ACK owned stock minus one proves use', () => {
  const f = fixture({ hp: 450 }); pollPotions([f.clients[0]], 0);
  f.packet(0, 'UseItem', { uniqueId: 20000, grid: 'Inventory', success: true }, 30);
  f.packet(0, 'HealthChanged', { hp: 480, mp: 151 }, 40); observeCombatActions(f.clients, 40); assert.ok(f.clients[0].gate.pending);
  potionResult(f); const r = f.rows.find(r => r.event === 'potionTerminal'); assert.equal(r.passed, true);
  assert.equal(r.proof.totalBefore - r.proof.totalAfter, 1); assert.ok(r.proof.stock.receiveSeq > r.proof.ack.receiveSeq);
  assert.equal(f.rows.filter(r => r.event === 'potionSourceState' || r.event === 'potionStateEvidence').length, 2);
});
test('stock decrement two and ordinary rejection are hard failures, not potion success', () => {
  const f = fixture({ hp: 450 }); pollPotions([f.clients[0]], 0); potionResult(f, 18);
  assert.equal(f.clients[0].combat.hardFailure, true); assert.equal(f.rows.find(r => r.event === 'potionTerminal'), undefined);
  const g = fixture({ hp: 450 }); pollPotions([g.clients[0]], 0); g.packet(0, 'UseItem', { uniqueId: 20000, grid: 'Inventory', success: false }, 10);
  observeCombatActions(g.clients, 10); assert.equal(g.clients[0].combat.hardFailure, true); assert.equal(g.clients[0].gate.pending, null);
});
test('actual potion sends are at least2000ms apart and no second use occurs while attack is pending', () => {
  const f = fixture({ hp: 450 }); pollPotions([f.clients[0]], 0); potionResult(f);
  for (let t = 75; t < 2000; t += 25) { f.clock(t); pollPotions([f.clients[0]], t); }
  assert.equal(f.commands.length, 1); f.clock(2000); pollPotions([f.clients[0]], 2000); assert.equal(f.commands.length, 2);
  const g = fixture({ hp: 450 }); offer(g); pollPotions([g.clients[0]], 0); assert.equal(g.commands.length, 1); assert.equal(g.commands[0].command.type, 'attackDirection');
});
test('late potion evidence retires its unique pending but is hard failure and never retried', () => {
  const f = fixture({ hp: 450 }); pollPotions([f.clients[0]], 0); observeCombatActions(f.clients, 2500);
  assert.equal(f.clients[0].combat.hardFailure, true); potionResult(f, 19, 2600);
  const r = f.rows.find(r => r.event === 'potionTerminal'); assert.equal(r.passed, false); assert.equal(r.timedOut, true);
  assert.equal(f.clients[0].gate.pending, null); f.clock(5000); pollPotions([f.clients[0]], 5000); assert.equal(f.commands.length, 1);
});

test('incremental combat windows retain ambiguity checks after caching a hit and respect trimmed owner receipts', () => {
  const f = fixture(); offer(f);
  f.packet(0, 'UserLocation', { x: 10, y: 10, direction: 'Right' }, 10); hit(f);
  observeCombatActions(f.clients, 40); assert.equal(terminal(f), undefined);
  f.clients[0].events.splice(0);
  f.packet(1, 'ObjectAttack', { objectId: 50050, location: { x: 10, y: 10 }, direction: 'Right', spell: 0 }, 50);
  observeCombatActions(f.clients, 50); assert.equal(terminal(f), undefined, 'trimmed owner evidence cannot be cached forever');
  f.packet(0, 'UserLocation', { x: 10, y: 10, direction: 'Right' }, 60); observeCombatActions(f.clients, 60);
  assert.equal(terminal(f).passed, true);
  const g = fixture(); offer(g); g.packet(0, 'UserLocation', { x: 10, y: 10, direction: 'Right' }, 10); hit(g);
  observeCombatActions(g.clients, 40);
  g.packet(1, 'DamageIndicator', { object_id: 1000, damage: 20, damage_type: 0, typed: true }, 50);
  observeCombatActions(g.clients, 50); assert.equal(g.clients[0].combat.hardFailure, true); assert.equal(terminal(g), undefined);
});

test('cached potion ACK A may be replaced by retained duplicate B only after A is trimmed', () => {
  const f = fixture({ hp: 450 }), client = f.clients[0]; pollPotions([client], 0);
  const a = f.packet(0, 'UseItem', { uniqueId: 20000, grid: 'Inventory', success: true }, 10);
  observeCombatActions(f.clients, 10); assert.ok(client.gate.pending);
  const b = f.packet(0, 'UseItem', { uniqueId: 20000, grid: 'Inventory', success: true }, 20);
  observeCombatActions(f.clients, 20); assert.ok(client.gate.pending);
  client.events.splice(0, client.events.indexOf(a) + 1);
  const s = snapshot(0, f.p.actors[0].start, 450); s.inventoryItems[0].quantity = 19;
  f.clock(30); client.observe({ type: 'worldSnapshot', payload: s }); observeCombatActions(f.clients, 30);
  const result = f.rows.find(r => r.event === 'potionTerminal'); assert.equal(result.passed, true);
  assert.equal(result.proof.ack.receiveSeq, b.receiveSeq); assert.notEqual(result.proof.ack.receiveSeq, a.receiveSeq);
  assert.doesNotThrow(() => JSON.stringify(f.rows));
});
test('ordinary death is hard failure and never permits a new primary or debug respawn', () => {
  const f = fixture(); f.packet(0, 'Death', { location: { x: 10, y: 10 }, direction: 'Down' }, 10); offer(f);
  assert.equal(f.clients[0].combat.hardFailure, true); assert.equal(f.commands.length, 0);
});
test('LogOutSuccess freezes its preceding HP/dura operations before a later anonymous snapshot', () => {
  const f = fixture(), c = f.clients[0], original = equipmentFingerprint(c.latestSnapshotEvent.payload);
  f.packet(0, 'HealthChanged', { hp: 660, mp: 150 }, 10);
  f.packet(0, 'DuraChanged', { uniqueId: 10000, currentDura: 3999 }, 11);
  const ack = f.packet(0, 'LogOutSuccess', {}, 12);
  c.observe({ type: 'worldSnapshot', payload: { ...snapshot(0, f.p.actors[0].start), playerHp: 0, playerObjectId: 0, entities: [] } });
  const saved = createMeleeEngine(contractBytes).captureSavedState(c, ack);
  assert.equal(saved.state.hp, 660); assert.equal(saved.state.mp, 150); assert.equal(saved.carried.equipmentItems[0].durabilityCurrent, 3999);
  assert.equal(saved.carried.equipmentItems[0].tooltipSource.userItem.current_dura, 3999);
  assert.equal(saved.source.patches.length, 2); assert.equal(saved.source.logOutSuccess.receiveSeq, ack.receiveSeq);
  assert.notEqual(saved.state.items, original); assert.equal(c.combat.ownedSource.payload.equipmentItems[0].durabilityCurrent, 4000);
});
test('fresh save verification uses initial ordinary UserInformation pools rather than subsequent mutable HP', () => {
  const f = fixture({ hp: 450 }), c = f.clients[0];
  f.packet(0, 'HealthChanged', { hp: 500, mp: 150 }, 10);
  const actual = createMeleeEngine(contractBytes).captureVerificationState(c);
  assert.equal(actual.state.hp, 450); assert.equal(actual.state.mp, 151); assert.equal(c.snapshot.playerHp, 500);
  assert.equal(actual.source.userInformation.receiveSeq, c.bootstrapProof.user.receiveSeq);
});
test('per-actor minute scoring keeps notSent distinct, sent-only scheduler and timeout sticky', () => {
  const p = plan(), results = p.actions.map(a => ({ ...a, passed: true, timedOut: false, accepted: true, sentAtMs: a.dueMs, completedAtMs: a.dueMs + 20, scheduleDelayMs: 0 }));
  assert.ok(scoreCombatWindows(p, results).every(w => w.passed));
  results[0] = { ...p.actions[0], passed: false, timedOut: false }; let w = scoreCombatWindows(p, results)[0];
  assert.equal(w.passed, true); assert.equal(w.notSent, 1); assert.equal(w.timeout, 0); assert.equal(w.scheduleP95Ms, 0);
  results[0] = { ...results[1], ...p.actions[0], passed: false, timedOut: true }; w = scoreCombatWindows(p, results)[0];
  assert.equal(w.passed, false); assert.equal(w.timeout, 1);
});

function mockMeleeServer(actors, p, options = {}) {
  const states = actors.map((a, i) => snapshot(i, p.actors[i].start)), sockets = new Set(), sent = [], logins = actors.map(() => 0);
  let sharedId = 50050;
  const packet = (s, name, payload) => s.emit('message', Buffer.from(JSON.stringify({ type: 'packet', packet: name, payload })), false);
  const full = s => s.emit('message', Buffer.from(JSON.stringify({ type: 'worldSnapshot', payload: states[s.ordinal] })), false);
  const itemDura = (s, amount = 1) => { const item = states[s.ordinal].equipmentItems.find(i => i.slot === 'weapon');
    item.durabilityCurrent -= amount; item.tooltipSource.userItem.current_dura = item.durabilityCurrent;
    packet(s, 'DuraChanged', { uniqueId: item.uniqueId, currentDura: item.durabilityCurrent }); };
  class Socket extends EventEmitter {
    constructor(url, options) { super(); this.readyState = 0; this._closeFrameReceived = false; this.options = options; sockets.add(this);
      queueMicrotask(() => { this.readyState = 1; packet(this, 'Connected', {}); }); }
    send(raw) {
      const c = JSON.parse(raw); sent.push({ command: c.type, actorOrdinal: this.ordinal, verification: this.verification === true });
      if (c.type === 'clientVersion') packet(this, 'ClientVersion', { result: 1 });
      if (c.type === 'login') { this.ordinal = actors.findIndex(a => a.accountId === c.accountId); assert.ok(this.ordinal >= 0); logins[this.ordinal]++;
        this.verification = logins[this.ordinal] === (p.equipmentActorOrdinals.includes(this.ordinal) ? p.equipmentRoundsPerActor * 2 + 2 : 2);
        packet(this, 'LoginSuccess', { characters: [{ index: 0, name: actors[this.ordinal].name, level: 40 }] }); }
      if (c.type === 'startGame') {
        this.inGame = true; this.sharedId = sharedId++; const state = states[this.ordinal], self = state.entities[0];
        self.name = actors[this.ordinal].name;
        if (this.verification && options.freshStateMismatch) self.direction = 'Down';
        packet(this, 'StartGame', { result: 4 }); packet(this, 'UserInformation', { objectId: 1000, name: self.name, class: 'Warrior', gender: 'Male',
          level: 40, hp: state.playerHp, mp: state.playerMp, maxHp: 674, location: { x: self.x, y: self.y },
          hasHero: false, magicCount: 0, intelligentCreatureCount: 0, creatureSummoned: false });
        packet(this, 'MapInformation', { fileName: 'DogYoHyun', mapIndex: 440 });
        for (const peer of sockets) if (peer !== this && peer.inGame) {
          packet(this, 'ObjectPlayer', { objectId: peer.sharedId, name: actors[peer.ordinal].name });
          packet(peer, 'ObjectPlayer', { objectId: this.sharedId, name: self.name });
        }
        full(this);
      }
      if (['removeItem', 'equipItem'].includes(c.type)) {
        const state = states[this.ordinal];
        if (c.type === 'removeItem') { const item = state.equipmentItems.find(i => i.uniqueId === c.uniqueId);
          state.equipmentItems = state.equipmentItems.filter(i => i.uniqueId !== c.uniqueId); state.inventoryItems.push({ ...item, slot: c.to }); }
        else { const item = state.inventoryItems.find(i => i.uniqueId === c.uniqueId);
          state.inventoryItems = state.inventoryItems.filter(i => i.uniqueId !== c.uniqueId); state.equipmentItems.push({ ...item, slot: 'weapon' }); }
        packet(this, c.type === 'removeItem' ? 'RemoveItem' : 'EquipItem', { grid: 'Inventory', uniqueId: c.uniqueId, to: c.to, success: true }); full(this);
      }
      if (c.type === 'changeAMode') { this.combatMode = true; packet(this, 'ChangeAMode', { mode: c.mode }); }
      if (c.type === 'attackDirection') {
        const targetOrdinal = p.actors[this.ordinal].attackTargetOrdinal;
        const target = [...sockets].find(s => s.inGame && s.ordinal === targetOrdinal), own = states[this.ordinal], victim = states[targetOrdinal];
        own.entities[0].direction = c.direction; packet(this, 'UserLocation', { x: own.entities[0].x, y: own.entities[0].y, direction: c.direction });
        victim.playerHp -= 6; packet(target, 'HealthChanged', { hp: victim.playerHp, mp: victim.playerMp, typed: true });
        for (const peer of sockets) if (peer.inGame) {
          packet(peer, 'ObjectAttack', { objectId: peer === this ? 1000 : this.sharedId,
            location: { x: own.entities[0].x, y: own.entities[0].y }, direction: c.direction, spell: 0 });
          packet(peer, 'ObjectStruck', { objectId: peer === target ? 1000 : target.sharedId, attackerId: peer === this ? 1000 : this.sharedId });
          packet(peer, 'DamageIndicator', { object_id: peer === target ? 1000 : target.sharedId, damage: 6, damage_type: 0, typed: true });
        }
        itemDura(this); full(this); full(target);
      }
      if (c.type === 'useItem') {
        const state = states[this.ordinal], item = state.inventoryItems.find(i => i.uniqueId === c.uniqueId); item.quantity--;
        packet(this, 'UseItem', { uniqueId: c.uniqueId, grid: 'Inventory', success: true });
        state.playerHp = Math.min(674, state.playerHp + 110); packet(this, 'HealthChanged', { hp: state.playerHp, mp: state.playerMp }); full(this);
      }
      if (c.type === 'chat') for (const peer of sockets) if (peer.inGame) packet(peer, 'ObjectChat', { objectId: this.sharedId, text: `${actors[this.ordinal].name}: ${c.message}` });
      if (c.type === 'logOut') {
        if (this.verification && options.verificationLogoutFailure) {
          if (options.verificationLogoutFailure === 'reject') packet(this, 'LogOutFailed', {});
          else options.onUnknownLogout?.();
          return; // An unknown response must never cause a second Logout write.
        }
        if (this.combatMode) { const state = states[this.ordinal]; state.playerHp = Math.min(674, state.playerHp + 2);
          packet(this, 'HealthChanged', { hp: state.playerHp, mp: state.playerMp }); itemDura(this); }
        this.inGame = false; packet(this, 'LogOutSuccess', {});
        for (const peer of sockets) if (peer.inGame) packet(peer, 'ObjectRemove', { objectId: this.sharedId });
        this.emit('message', Buffer.from(JSON.stringify({ type: 'worldSnapshot', payload: { ...snapshot(this.ordinal, p.actors[this.ordinal].start),
          playerObjectId: 0, playerHp: 0, entities: [], inventoryItems: [], equipmentItems: [], beltItems: [] } })), false);
      }
    }
    close() { this.readyState = 3; this.inGame = false;
      this._closeFrameReceived = !(this.verification && options.verificationCloseFailure); this.emit('close', this._closeFrameReceived ? 1000 : 1006); }
    terminate() { this.readyState = 3; this.emit('close', 1006); }
  }
  return { WebSocket: Socket, sent, states, sockets };
}

async function runFreshSaveScenario(options) {
  const p = plan(), actors = p.actors.map(a => ({ ordinal: a.ordinal, accountId: `mockactor${a.ordinal}`, name: `Hero${a.ordinal}`, password: 'mock-only-password', characterIndex: 0 }));
  let clock = 0;
  const sdk = mockMeleeServer(actors, p, { ...options, onUnknownLogout() { clock += 5001; } }), rows = [];
  const report = await runComparison({ plan: p, cohort: { schema: 'mir2.comparable.modern-cohort.v1', qualificationLayer: 'game-core-local', mapSha256: MAP_SHA, actors },
    runtime: { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory', sourceRevision: 'a'.repeat(40),
      binarySha256: 'b'.repeat(64), endpoint: 'ws://127.0.0.1:19010/ws' }, url: 'ws://127.0.0.1:19010/ws', WebSocket: sdk.WebSocket,
    emit: row => rows.push(JSON.parse(JSON.stringify(row))), now: () => clock, wait: async ms => { clock += Math.max(1, ms); },
    ledger: { schema: 'mir2.comparable.local-auth-ledger.v1', endpoint: 'ws://127.0.0.1:19010/ws', attempts: [] }, saveLedger: async () => {}, engine: createMeleeEngine(contractBytes) });
  return { report, sdk, rows };
}

test('fresh saved-state mismatch stays failed but every fresh socket receives normal Logout then Close', async () => {
  const { report, sdk, rows } = await runFreshSaveScenario({ freshStateMismatch: true });
  assert.equal(report.ok, false); assert.equal(report.actions.length, 104); assert.ok(report.actions.every(a => a.passed));
  assert.ok(report.windows.every(w => w.passed)); assert.equal(report.savedVerification.length, 2);
  assert.equal(sdk.sent.filter(r => r.verification && r.command === 'logOut').length, 2);
  for (const r of report.savedVerification) {
    assert.equal(r.failure, 'Fresh-socket relogin full saved state differs'); assert.equal(r.statePreserved, false);
    assert.notEqual(r.before.direction, r.after.direction); assert.equal(r.before.hp, r.after.hp);
    assert.equal(r.before.items, r.after.items); assert.equal(r.normalLogout, true); assert.equal(r.transportClose, true); assert.equal(r.passed, false);
  }
  assert.equal(rows.filter(r => r.event === 'savedVerificationState' && r.statePreserved === false).length, 2);
  assert.ok([...sdk.sockets].every(s => s.readyState === 3));
});

test('comparison mismatch, rejected Logout and failed Close are all retained without retry', async () => {
  const { report, sdk } = await runFreshSaveScenario({ freshStateMismatch: true, verificationLogoutFailure: 'reject', verificationCloseFailure: true });
  assert.equal(report.ok, false);
  assert.equal(sdk.sent.filter(r => r.verification && r.command === 'logOut').length, 2);
  for (const r of report.savedVerification) {
    assert.equal(r.failure, 'Fresh-socket relogin full saved state differs'); assert.match(r.logoutFailure, /Verification logout refused/);
    assert.match(r.closeFailure, /No real WebSocket Close frame/); assert.equal(r.normalLogout, false); assert.equal(r.transportClose, false); assert.equal(r.passed, false);
  }
  assert.ok([...sdk.sockets].every(s => s.readyState === 3));
});

test('unknown fresh Logout outcome is not resent and still closes the transport after its original5s budget', async () => {
  const { report, sdk } = await runFreshSaveScenario({ freshStateMismatch: true, verificationLogoutFailure: 'unknown' });
  assert.equal(report.ok, false);
  assert.equal(sdk.sent.filter(r => r.verification && r.command === 'logOut').length, 2);
  for (const r of report.savedVerification) {
    assert.equal(r.failure, 'Fresh-socket relogin full saved state differs'); assert.match(r.logoutFailure, /Timed out|deadline/i);
    assert.equal(r.normalLogout, false); assert.equal(r.transportClose, true); assert.equal(r.passed, false);
  }
  assert.ok([...sdk.sockets].every(s => s.readyState === 3));
});

test('complete ordinary melee SDK flow:104 primaries, reactive potions, gear and ACK-frozen fresh saves', async () => {
  const p = process.env.MIR2_COMPARABLE_MELEE_PLAN ? validateCombatPlan(JSON.parse(await fs.readFile(process.env.MIR2_COMPARABLE_MELEE_PLAN)), contractBytes) : plan();
  const actors = p.actors.map(a => ({ ordinal: a.ordinal, accountId: `mockactor${a.ordinal}`, name: `Hero${a.ordinal}`, password: 'mock-only-password', characterIndex: 0 }));
  const sdk = mockMeleeServer(actors, p), rows = [], reservations = []; let clock = 0;
  const report = await runComparison({ plan: p, cohort: { schema: 'mir2.comparable.modern-cohort.v1', qualificationLayer: 'game-core-local', mapSha256: MAP_SHA, actors },
    runtime: { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory', sourceRevision: 'a'.repeat(40),
      binarySha256: 'b'.repeat(64), endpoint: 'ws://127.0.0.1:19010/ws' }, url: 'ws://127.0.0.1:19010/ws', WebSocket: sdk.WebSocket,
    emit: row => rows.push(JSON.parse(JSON.stringify(row))), now: () => clock, wait: async ms => { clock += Math.max(1, ms); },
    ledger: { schema: 'mir2.comparable.local-auth-ledger.v1', endpoint: 'ws://127.0.0.1:19010/ws', attempts: [] },
    saveLedger: async l => reservations.push(l.attempts.length), engine: createMeleeEngine(contractBytes) });
  assert.equal(report.failure, undefined); assert.equal(report.ok, true); assert.equal(report.admitted, 2); assert.equal(report.actions.length, 104);
  assert.ok(report.actions.every(a => a.passed && !a.timedOut)); assert.ok(report.windows.every(w => w.offered === 52 && w.passed));
  assert.equal(report.equipment.flatMap(r => r.rounds).length, 8); assert.ok(report.reactivePotions.issued >= 2); assert.equal(report.reactivePotions.passed, true);
  assert.equal(report.cleanup.length, 2); assert.ok(report.cleanup.every(r => r.normalLogout && r.transportClose && r.savedStateSource.logOutSuccess));
  assert.equal(report.savedVerification.length, 2); assert.ok(report.savedVerification.every(r => r.passed && r.savedStateSource.userInformation));
  assert.ok(report.savedVerification.every(r => r.statePreserved && r.normalLogout && r.transportClose));
  assert.equal(reservations.length, 12); assert.equal(sdk.sent.filter(r => r.command === 'login').length, 12);
  assert.equal(sdk.sent.filter(r => r.command === 'changeAMode').length, 2); assert.equal(sdk.sent.filter(r => r.command === 'attackDirection').length, 100);
  assert.ok(report.observationEndedAtMs >= report.measurementEpochMs + 60000); assert.equal(report.capacityClaimAllowed, false);
  if (process.env.MIR2_COMPARABLE_MELEE_OUTPUT) {
    const output = process.env.MIR2_COMPARABLE_MELEE_OUTPUT;
    assert.match(output.replaceAll('\\', '/'), /^C:\/Users\/Administrator\/AppData\/Local\/Temp\/mir2-comparable-capacity-20261113\/modern-audit\/synthetic-melee-/i);
    await fs.mkdir(output);
    const redact = value => Array.isArray(value) ? value.map(redact) : value && typeof value === 'object' ?
      Object.fromEntries(Object.entries(value).map(([k, v]) => [k, /password|token|secret|accountId/i.test(k) ? '[redacted]' : redact(v)])) : value;
    const events = rows.map(r => JSON.stringify({ ...redact(r), synthetic: true, liveEvidence: false })).join('\n') + '\n';
    const sample = { ...redact(report), synthetic: true, liveEvidence: false, sourceProof: 'SDK-MOCK-NO-GATEWAY', eventsSha256: hash(events) };
    await fs.writeFile(output + '/sample.events.jsonl', events, { flag: 'wx' });
    if (process.env.MIR2_COMPARABLE_MELEE_PLAN) { const bytes = await fs.readFile(process.env.MIR2_COMPARABLE_MELEE_PLAN); sample.planSha256 = hash(bytes);
      await fs.writeFile(output + '/sample.plan.json', bytes, { flag: 'wx' }); }
    await fs.writeFile(output + '/sample.report.json', JSON.stringify(sample, null, 2) + '\n', { flag: 'wx' });
  }
});
