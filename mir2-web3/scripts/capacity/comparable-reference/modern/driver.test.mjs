import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { MAP_SHA, hash, validatePlan, PendingGate, RetainedReceiveCursor, equipmentFingerprint, savedState, scoreWindows, freezeWindowFailures, localLoginDelay, decodeNavigation } from './contract.mjs';
import { ModernClient, endpoint, offerAction, observeActions, chatMatches, equipmentRoundtrip, runComparison,
  runtimeSourceProof, parseComparisonOptions, loadAdapterSources, runScheduledActions } from './driver.mjs';
import { EventEmitter } from 'node:events';

// The flat frozen bundle carries this identical definition beside the tests.
const rulesBytes = await fs.readFile(new URL('./workload.v2.json', import.meta.url)).catch(error => {
  if (error.code !== 'ENOENT') throw error; return fs.readFile(new URL('../workload.v2.json', import.meta.url));
}), rules = JSON.parse(rulesBytes);
function plan(population = 2) {
  const p = { schema: 'mir2.comparable.plan.v1', qualificationLayer: 'game-core-local', scenario: 'dense-movement-chat', purpose: 'functional',
    population, durationMs: population === 2 ? 60000 : 300000, planId: '0123456789abcdefabcd', map: structuredClone(rules.map), actorTemplate: structuredClone(rules.actorTemplate),
    definitionSha256: hash(rulesBytes), scheduling: { version: 2, chatPhaseRule: 'same-primary-phase-with-midpoint-offset', primaryIntervalMs: 2000,
      minimumSameActorOfferGapMs: 1000, chatOffsetWithinPrimaryIntervalMs: 1000 },
    generatorSourceSha256: 'd'.repeat(64), assessment: { windowMs: 60000, perActor: true, allCompleteWindowsRequired: true,
      failedActionCanBecomeCompleted: false, notSentIsTimeout: false, notSentIsCompleted: false, safetyOrEvidenceFailureIsHardFailure: true },
    equipmentActorOrdinals: population === 2 ? [0, 1] : [0, 1, 2], equipmentRoundsPerActor: 2,
    thresholds: structuredClone(rules.thresholds), resourceContract: structuredClone(rules.resourceContract), causality: structuredClone(rules.causality),
    actors: Array.from({ length: population }, (_, ordinal) => { const x = population === 2 ? 10 : 10 + (ordinal % 10) * 2,
      y = population === 2 ? 10 + ordinal : 10 + Math.floor(ordinal / 10);
    return { ordinal, group: 0, peerOrdinals: [(ordinal + 1) % population], start: { x, y }, lane: [{ x, y }, { x: x + 1, y }] }; }), actions: [] };
  if (population !== 2) p.purpose = 'capacity';
  for (const a of p.actors) {
    const actions = [];
    for (let at = Math.floor(a.ordinal * 2000 / population), step = 0; at < p.durationMs; at += 2000, step++) actions.push({ actorOrdinal: a.ordinal, kind: 'walk', dueMs: at, deadlineMs: at + 2000,
      direction: step % 2 ? 'Left' : 'Right', target: a.lane[step % 2 ? 0 : 1] });
    for (let at = 15000 + Math.floor(a.ordinal * 2000 / population); at < p.durationMs; at += 30000) actions.push({ actorOrdinal: a.ordinal, kind: 'chat', dueMs: at, deadlineMs: at + 2500 });
    actions.sort((a, b) => a.dueMs - b.dueMs || a.kind.localeCompare(b.kind));
    for (const [i, action] of actions.entries()) { action.ticketSequence = i + 1; if (action.kind === 'chat') { action.nonce = `m2cmp:${p.planId}:${a.ordinal}:${i + 1}`; action.nonceSha256 = hash(action.nonce); } p.actions.push(action); }
  }
  p.actions.sort((a, b) => a.dueMs - b.dueMs || a.actorOrdinal - b.actorOrdinal || a.ticketSequence - b.ticketSequence); return p;
}
function snapshot(i = 0) {
  return { mapFileName: 'DogYoHyun', playerObjectId: 1000, playerHp: 674, playerMp: 151, gold: 0, inventoryCapacity: 46, maxBagSlots: 40,
    entities: [{ objectId: 1000, kind: 'player', name: `Hero${i}`, level: 40, class: 'Warrior', gender: 'Male', x: 10, y: 10 + i, direction: 'Down', dead: false }],
    inventoryItems: Array.from({ length: 9 }, (_, slot) => ({ slot, uniqueId: 20000 + i * 100 + slot, key: 'crystal-item-662', quantity: 20 })), beltItems: [],
    equipmentItems: [['weapon', 221, 4000], ['armour', 317, 5000], ['braceletLeft', 482, 7000], ['braceletRight', 482, 7000]].map(([slot, index, durability], n) =>
      ({ slot, uniqueId: 10001 + i * 100 + n, key: `crystal-item-${index}`, quantity: 1, durabilityCurrent: durability, durabilityMax: durability,
        tooltipSource: { userItem: { uniqueId: 10001 + i * 100 + n, itemIndex: index } } })) };
}
function clients() {
  const emitted = [], sent = []; let now = 0;
  const cs = [0, 1].map(ordinal => {
    const c = new ModernClient({ url: 'ws://127.0.0.1:19010/ws', actor: { ordinal, name: `Hero${ordinal}` }, connection: ordinal + 1,
      now: () => now, emit: row => emitted.push(row) });
    c.observe({ type: 'worldSnapshot', payload: snapshot(ordinal) }); c.inGame = true;
    c.ws = { readyState: 1, send: raw => sent.push({ actor: ordinal, command: JSON.parse(raw) }) }; return c;
  });
  cs[0].sharedPlayers.set('Hero1', 50051); cs[1].sharedPlayers.set('Hero0', 50050);
  return { cs, emitted, sent, clock: t => { now = t; } };
}
test('complete common fixed plan is accepted; idle/sparse plans and weakened thresholds fail', () => {
  assert.equal(validatePlan(plan()).actions.length, 64);
  for (const mutate of [p => p.actions.pop(), p => { p.thresholds.walkDeadlineMs = 3000; }, p => { p.actors[1].lane = p.actors[0].lane; },
    p => { p.qualificationLayer = 'cloud-production'; }, p => { p.scenario = 'dispersed-melee'; }, p => { p.map.rawSha256 = '0'.repeat(64); }]) {
    const p = plan(); mutate(p); assert.throws(() => validatePlan(p));
  }
});
test('100 actors keep chats 1000ms from their own walks; the old 10ms actor99 collision is rejected', () => {
  const p = validatePlan(plan(100));
  for (const actor of p.actors) {
    const actions = p.actions.filter(a => a.actorOrdinal === actor.ordinal);
    assert.ok(actions.slice(1).every((a, i) => a.dueMs - actions[i].dueMs >= 1000));
    assert.equal(actions.filter(a => a.kind === 'chat').length, 10);
  }
  assert.equal(p.actions.find(a => a.actorOrdinal === 99 && a.kind === 'chat').dueMs, 16980);
  const old = structuredClone(p);
  for (const action of old.actions.filter(a => a.kind === 'chat')) { action.dueMs -= Math.floor(action.actorOrdinal * 2000 / 100) - Math.floor(action.actorOrdinal * 1000 / 100); action.deadlineMs = action.dueMs + 2500; }
  old.actions.sort((a, b) => a.dueMs - b.dueMs || a.actorOrdinal - b.actorOrdinal || a.ticketSequence - b.ticketSequence);
  assert.throws(() => validatePlan(old), /Complete fixed cadence/);
  const weakened = structuredClone(p); weakened.scheduling.minimumSameActorOfferGapMs = 10;
  assert.throws(() => validatePlan(weakened), /Corrected common/);
});
test('only explicit loopback plain WS endpoints are legal', () => {
  assert.equal(endpoint('ws://127.0.0.1:19010/ws'), 'ws://127.0.0.1:19010/ws');
  for (const bad of ['wss://example.com/ws', 'ws://example.com/ws', 'ws://localhost:1/ws', 'ws://127.0.0.1:1/ws?token=x', 'ws://u:p@127.0.0.1:1/ws']) assert.throws(() => endpoint(bad));
});
test('navigation is the original LSB0 bitset with exact count/exclusions, not a rows projection', () => {
  const bits = Buffer.alloc(80000); bits.fill(255, 0, Math.floor(404451 / 8)); bits[Math.floor(404451 / 8)] = (1 << (404451 % 8)) - 1;
  const nav = { schema: 'mir2.comparable.navigation.v1', map: rules.map, source: 'original-library-cell-valid-and-movement-exclusion',
    gameCodeUnmodified: true, encoding: 'row-major-lsb0-bitset', eligibleBitsBase64: bits.toString('base64'), eligibleCount: 404451,
    exclusions: { movementRadius: 3, npcRadius: 3, allDoorCells: true, safeZones: true } };
  const eligible = decodeNavigation(nav, rules.map); assert.equal(eligible(10, 10), true); assert.equal(eligible(799, 799), false);
  for (const mutate of [n => { n.eligibleCount--; }, n => { n.eligibleBitsBase64 = 'AA=='; }, n => { n.exclusions.allDoorCells = false; }, n => { n.encoding = 'walkableRows'; }]) {
    const bad = structuredClone(nav); mutate(bad); assert.throws(() => decodeNavigation(bad, rules.map));
  }
});
test('timeout fences sends until unique late closure and never restores success', () => {
  const rows = [], g = new PendingGate(r => rows.push(r));
  assert.equal(g.start({ kind: 'walk', ticketSequence: 1, deadlineAtMs: 2000 }, 0), true); g.expire(2000);
  assert.equal(g.start({ kind: 'chat', ticketSequence: 2, deadlineAtMs: 2500 }, 2100), false);
  assert.equal(g.pending.ticketSequence, 1);
  const result = g.close([{ atMs: 2300, receiveSeq: 19 }], 2301); assert.equal(result.passed, false); assert.equal(result.late, true);
  assert.equal(g.failed, true); assert.equal(rows[1].event, 'notSent'); assert.equal(g.start({ deadlineAtMs: 5000 }, 2400), true);
});
test('receipt timestamps decide deadline, including owner1700ms vs peer60ms', () => {
  const g = new PendingGate(); g.start({ deadlineAtMs: 1000 }, 0);
  const late = g.close([{ atMs: 1700 }, { atMs: 60 }], 1800); assert.equal(late.passed, false); assert.equal(late.timedOut, true);
  const h = new PendingGate(); h.start({ deadlineAtMs: 2000 }, 0);
  assert.equal(h.close([{ atMs: 1900 }, { atMs: 1950 }], 2050).passed, true);
});
test('walk requires fresh exact owner and actual peer namespace with receive sequence/time', () => {
  const { cs, emitted, clock } = clients(), p = plan(), action = p.actions[0];
  offerAction(cs[0], cs, p, action, 0, 0); clock(30);
  cs[0].observe({ type: 'packet', packet: 'UserLocation', payload: { location: action.target, direction: 'Right' } });
  clock(50); cs[1].observe({ type: 'packet', packet: 'ObjectWalk', payload: { objectId: 50050, location: action.target } }); observeActions(cs);
  const terminal = emitted.find(e => e.event === 'actionTerminal'); assert.equal(terminal.passed, true);
  assert.deepEqual(terminal.expectedPeerOrdinals, [1]); assert.equal(terminal.expectedSharedIds[0].objectId, 50050);
  assert.equal(terminal.receipts.length, 2); assert.equal(terminal.receipts[1].receiveSeq, 2); assert.equal(terminal.receipts[1].atMs, 50);
  assert.deepEqual(terminal.actualTarget, { x: 11, y: 10 });
});
test('owner ID1000 cannot substitute for peer ID; late real peer remains failed', () => {
  const { cs, emitted, clock } = clients(), p = plan(), action = p.actions[0]; offerAction(cs[0], cs, p, action, 0, 0);
  clock(40); cs[0].observe({ type: 'packet', packet: 'UserLocation', payload: { location: action.target } });
  cs[1].observe({ type: 'packet', packet: 'ObjectWalk', payload: { objectId: 1000, location: action.target } }); observeActions(cs);
  assert.ok(cs[0].gate.pending); cs[0].gate.expire(2000); clock(2300);
  cs[1].observe({ type: 'packet', packet: 'ObjectWalk', payload: { objectId: 50050, location: action.target } }); observeActions(cs);
  assert.equal(emitted.findLast(e => e.event === 'actionTerminal').passed, false);
});
test('old receipts cannot complete a new action, and actual correction is rejection rather than timeout', () => {
  const { cs, emitted, clock } = clients(), p = plan(), action = p.actions[0];
  cs[1].observe({ type: 'packet', packet: 'ObjectWalk', payload: { objectId: 50050, location: action.target } });
  offerAction(cs[0], cs, p, action, 0, 0); clock(59);
  cs[0].observe({ type: 'packet', packet: 'UserLocation', payload: { location: { x: 10, y: 10 } } }); observeActions(cs);
  const row = emitted.find(e => e.event === 'actionTerminal'); assert.equal(row.accepted, false); assert.equal(row.passed, false);
  assert.equal(row.timedOut, false); assert.equal(cs[0].gate.pending, null);
});
test('Chat uses exact nonce and shared sender on owner and every expected peer', () => {
  const { cs, emitted, clock } = clients(), p = plan(), action = p.actions.find(a => a.actorOrdinal === 0 && a.kind === 'chat');
  clock(action.dueMs); offerAction(cs[0], cs, p, action, 0, action.dueMs);
  assert.equal(chatMatches({ objectId: 50050, text: `prefix${action.nonce}` }, 50050, action.nonce), false);
  clock(action.dueMs + 20); cs[0].observe({ type: 'packet', packet: 'ObjectChat', payload: { objectId: 1000, text: action.nonce } }); observeActions(cs); assert.ok(cs[0].gate.pending);
  for (const c of cs) c.observe({ type: 'packet', packet: 'ObjectChat', payload: { objectId: 50050, text: `Hero0:${action.nonce}` } }); observeActions(cs);
  const row = emitted.find(e => e.event === 'actionTerminal'); assert.equal(row.passed, true); assert.equal(row.nonceHash, action.nonceSha256); assert.equal(row.receipts.length, 2);
});
test('busy and deadline-before-send offers preserve denominator and do not send', () => {
  const { cs, sent, emitted } = clients(), p = plan(); offerAction(cs[0], cs, p, p.actions[0], 0, 0);
  const second = p.actions.find(a => a.actorOrdinal === 0 && a.ticketSequence === 2);
  offerAction(cs[0], cs, p, second, 0, second.dueMs); assert.equal(sent.length, 1); assert.equal(emitted.find(e => e.event === 'notSent').reason, 'previousActionPending');
  const other = clients(); offerAction(other.cs[0], other.cs, p, p.actions[0], 0, 2001);
  assert.equal(other.sent.length, 0); assert.equal(other.emitted.find(e => e.event === 'notSent').reason, 'deadlineBeforeSend');
});
test('transport send uncertainty remains fenced and cannot be healed green by ACKs', () => {
  const { cs, emitted, clock } = clients(), p = plan(); cs[0].ws.send = () => { throw new Error('send failed'); };
  offerAction(cs[0], cs, p, p.actions[0], 0, 0); assert.ok(cs[0].gate.pending); clock(20);
  cs[0].observe({ type: 'packet', packet: 'UserLocation', payload: { location: p.actions[0].target } });
  cs[1].observe({ type: 'packet', packet: 'ObjectWalk', payload: { objectId: 50050, location: p.actions[0].target } }); observeActions(cs);
  assert.equal(emitted.find(e => e.event === 'actionTerminal').passed, false);
});
test('complete carried fingerprint rejects absent metrics and detects metadata, UID, gold or capacity changes', () => {
  const s = snapshot(), before = equipmentFingerprint(s); assert.equal(MAP_SHA.length, 64);
  for (const mutate of [s => { s.gold++; }, s => { s.maxBagSlots++; }, s => { s.equipmentItems[0].uniqueId++; }, s => { s.equipmentItems[0].metadata = { added: 1 }; }]) {
    const copy = structuredClone(s); mutate(copy); assert.notEqual(equipmentFingerprint(copy), before);
  }
  const missing = structuredClone(s); delete missing.beltItems; assert.throws(() => equipmentFingerprint(missing));
});
test('two equipment rounds require four exact ACKs and fresh states with full restoration', async () => {
  const { cs, sent, clock } = clients(), original = structuredClone(cs[0].snapshot), rows = []; let tick = 0, next = 2, persisted;
  cs[0].snapshot.equipmentItems.reverse(); // Re-equipping can move Weapon to the end of the saved vector.
  const attach = c => { c.ws = { readyState: 1, send: raw => {
    const command = JSON.parse(raw); sent.push(command); persisted = structuredClone(original);
    if (command.type === 'removeItem') { persisted.inventoryItems.push({ ...persisted.equipmentItems[0], slot: command.to }); persisted.equipmentItems = persisted.equipmentItems.slice(1); }
    clock(tick += 10); c.observe({ type: 'packet', packet: command.type === 'removeItem' ? 'RemoveItem' : 'EquipItem', payload: { ...command, success: true } });
    // Even this matching old-connection refresh must not supply state proof.
    c.observe({ type: 'worldSnapshot', payload: persisted });
  } }; return c; };
  attach(cs[0]);
  const refresh = async (old, deadline) => {
    const c = attach(new ModernClient({ url: old.url, actor: old.actor, connection: next++, now: old.now, emit: e => rows.push(e) }));
    c.snapshot = structuredClone(persisted); c.inGame = true; clock(tick += 10);
    const e = c.observe({ type: 'worldSnapshot', payload: persisted });
    return { client: c, proof: { snapshot: { observerOrdinal: 0, connection: c.connection, receiveSeq: e.receiveSeq, atMs: e.atMs } }, lifecycle: { outerDeadlineAtMs: deadline } };
  };
  const result = await equipmentRoundtrip(cs[0], 2, e => rows.push(e), refresh); assert.equal(result.passed, true); assert.equal(result.rounds.length, 4);
  assert.equal(sent.length, 4); assert.ok(result.rounds.every(r => r.ack.connection !== r.state.connection));
  assert.ok(sent.every(c => c.uniqueId === original.equipmentItems.find(i => i.slot === 'weapon').uniqueId));
});
test('matching old-socket equipment snapshot cannot claim a fresh state', async () => {
  const { cs } = clients(), c = cs[0];
  c.ws.send = raw => { const cmd = JSON.parse(raw); c.observe({ type: 'packet', packet: 'RemoveItem', payload: { ...cmd, success: true } });
    const after = snapshot(); after.inventoryItems = [{ ...after.equipmentItems[0], slot: cmd.to }]; after.equipmentItems = []; c.observe({ type: 'worldSnapshot', payload: after }); };
  await assert.rejects(equipmentRoundtrip(c, 1, () => {}, async old => ({ client: old, proof: { snapshot: { connection: old.connection, atMs: 0 } } })), /fresh-socket/);
});
test('lifecycle timeout retains fence until exact late response, without retries', async () => {
  const { cs, sent, clock, emitted } = clients(), c = cs[0]; let time = 0; c.now = () => (time += 10);
  await assert.rejects(c.request({ type: 'logOut' }, e => e.packet === 'LogOutSuccess', 25, 'logout'), /deadline/);
  assert.equal(sent.length, 1); assert.ok(c.pending?.expired);
  await assert.rejects(c.request({ type: 'walk', direction: 'Right' }, () => true, 100, 'forbidden next'), /Previous/);
  c.observe({ type: 'packet', packet: 'KeepAlive', payload: { time: 1 } }); c.reconcileLate(); assert.ok(c.pending);
  clock(80); c.observe({ type: 'packet', packet: 'LogOutSuccess', payload: {} }); c.reconcileLate(); assert.equal(c.pending, null);
  assert.equal(emitted.find(e => e.event === 'lateLifecycleClosure').passed, false);
});
test('raw login/QA/MoveTo/Stage5 are outside adapter whitelist, and secrets are redacted', () => {
  const { cs, emitted } = clients(), c = cs[0];
  for (const type of ['passkeyLogin', 'qa.giveItem', 'stage5Command', 'moveTo']) assert.throws(() => c.send({ type }));
  c.send({ type: 'login', accountId: 'local-account', password: 'private-value' });
  const text = JSON.stringify(emitted); assert.ok(!text.includes('private-value')); assert.ok(!text.includes('local-account'));
});
test('per-actor/minute gates retain missing/failed offers; owner slow is not hidden by fast peer', () => {
  const p = plan(), results = p.actions.map(a => ({ ...a, sentAtMs: a.dueMs + 1, completedAtMs: a.dueMs + 20, scheduleDelayMs: 1, passed: true }));
  assert.ok(scoreWindows(p, results).every(w => w.passed));
  const slow = structuredClone(results); slow[0].completedAtMs = slow[0].dueMs + 1700; slow[0].passed = false; slow[0].timedOut = true;
  const windows = scoreWindows(p, slow); assert.equal(windows[0].passed, false); assert.equal(windows[0].timeout, 1); assert.equal(windows[1].passed, true);
  const notSent = scoreWindows(p, results.slice(1))[0]; assert.equal(notSent.notSent, 1); assert.equal(notSent.timeout, 0); assert.equal(notSent.passed, true);
  const rejected = structuredClone(results); rejected[0].passed = false; rejected[0].accepted = false;
  const rejectedWindow = scoreWindows(p, rejected)[0]; assert.equal(rejectedWindow.rejected, 1); assert.equal(rejectedWindow.timeout, 0); assert.equal(rejectedWindow.passed, true);
});
test('existing failed minute cannot be washed green by a later summary', () => {
  const before = [{ actorOrdinal: 0, minute: 0, passed: false }, { actorOrdinal: 1, minute: 0, passed: true }];
  const after = [{ actorOrdinal: 0, minute: 0, passed: true }, { actorOrdinal: 1, minute: 0, passed: true }];
  const merged = freezeWindowFailures(before, after); assert.equal(merged[0].passed, false); assert.equal(merged[1].passed, true);
});
test('normal30/901s budget retains every attempt and waits for enough whole-phase slots', () => {
  const history = Array.from({ length: 30 }, (_, i) => ({ atMs: 1000 + i, purpose: i < 2 ? 'admission' : 'equipmentState' }));
  assert.equal(localLoginDelay(history, 2000), 900000); assert.equal(localLoginDelay(history, 2000, 14), 900013);
  assert.equal(localLoginDelay(history, 902029), 0); assert.equal(history.length, 30);
  assert.throws(() => localLoginDelay(history, 2000, 31)); assert.throws(() => localLoginDelay([{ purpose: 'x' }], 2000));
});
test('saved state includes final authoritative transform and full carried state', () => {
  const { cs } = clients(), before = savedState(cs[0].snapshot);
  cs[0].observe({ type: 'packet', packet: 'UserLocation', payload: { location: { x: 11, y: 10 }, direction: 'Right' } });
  const actual = savedState(cs[0].snapshot); assert.equal(actual.x, 11); assert.notEqual(hash(actual), hash(before)); assert.equal(actual.items, before.items);
});
test('high-frequency large rosters retain only current full source and16 immutable history projections', () => {
  let tick = 0, received = 0, first;
  const c = new ModernClient({ url: 'ws://127.0.0.1:19010/ws', actor: { ordinal: 0, name: 'Hero0' }, connection: 1,
    now: () => tick, emit: e => { if (e.event === 'received' && e.type === 'worldSnapshot') { first ??= e; received++; } } });
  const large = snapshot(); large.inventoryItems = Array.from({ length: 40 }, (_, i) => ({ uniqueId: 30000 + i, slot: i,
    key: 'crystal-item-662', quantity: 1, description: 'large-roster-'.repeat(350) }));
  large.entities.push(...Array.from({ length: 99 }, (_, i) => ({ objectId: 50000 + i, kind: 'player', name: `Peer${i}`, x: i, y: 20, level: 40 })));
  let strong;
  for (tick = 0; tick < 240; tick++) {
    large.entities[0].x = tick; c.observe({ type: 'worldSnapshot', payload: large }, 1000);
    if (!tick) strong = c.ownedSnapshotEvidence('test-retained-strong-roster');
  }
  assert.equal(received, 240); assert.equal(c.snapshotHistory.length, 16); assert.equal(c.events.filter(e => e.type === 'worldSnapshot').length, 0);
  assert.equal(c.latestSnapshotEvent.receiveSeq, 240); assert.equal(c.snapshot.inventoryItems, c.latestSnapshotEvent.payload.inventoryItems);
  assert.equal(first.owner.x, 0); assert.equal(first.entities.length, 100); assert.ok(Object.isFrozen(c.snapshotHistory[0].payload.entities[0]));
  const firstHash = first.carriedFingerprint;
  c.observe({ type: 'packet', packet: 'UserLocation', payload: { location: { x: 555, y: 51 } } });
  assert.equal(c.latestSnapshotEvent.payload.entities[0].x, 239); assert.equal(first.owner.x, 0);
  c.snapshot.inventoryItems[0].description = 'changed-current';
  assert.equal(first.carriedFingerprint, firstHash); assert.notEqual(strong.state.inventoryItems[0].description, 'changed-current');
  const retainedBytes = Buffer.byteLength(JSON.stringify({ current: c.snapshot, history: c.snapshotHistory }));
  assert.ok(retainedBytes < 1000000, `retained projection bytes ${retainedBytes}`);
  assert.equal(strong.sourceReceipt.receiveSeq, 1); assert.equal(strong.fingerprint, firstHash);
});
test('exact steady counters exclude bootstrap, gear and deadline-tail payload bytes', () => {
  const { cs, clock } = clients(), c = cs[0];
  clock(10); c.observe({ type: 'packet', packet: 'ClientVersion', payload: { result: 1 } }, 10000);
  c.send({ type: 'keepAlive', time: 10 }); const bootstrapTotal = c.sentBytes;
  c.trafficWindow = { startAtMs: 100, endAtMs: 200, sent: 0, received: 0 };
  clock(99); c.observe({ type: 'packet', packet: 'KeepAlive', payload: {} }, 2000);
  clock(100); const start = c.counterSample(); c.send({ type: 'chat', message: 'steady-nonce' });
  clock(199); c.observe({ type: 'packet', packet: 'ObjectChat', payload: { text: 'steady-nonce' } }, 123);
  clock(200); const end = c.counterSample(); c.observe({ type: 'packet', packet: 'UserLocation', payload: { location: { x: 11, y: 10 } } }, 5000);
  c.send({ type: 'keepAlive', time: 200 });
  assert.equal(c.trafficWindow.received, 123); assert.equal(c.trafficWindow.sent, Buffer.byteLength(JSON.stringify({ type: 'chat', message: 'steady-nonce' })));
  assert.ok(c.receivedBytes > c.trafficWindow.received); assert.ok(c.sentBytes > bootstrapTotal + c.trafficWindow.sent);
  assert.equal(end.actualCounterAtMs - start.actualCounterAtMs, 100); assert.equal(end.receivedBytes - start.receivedBytes, 123);
});

// SDK mock only: emits actual ordinary envelope shapes, never binds a socket.
function sdkMockServer(actors, plan) {
  const states = actors.map((a, i) => { const state = snapshot(i); if (plan) Object.assign(state.entities[0], plan.actors[i].start); return state; }), sockets = new Set(), commands = []; let sharedId = 50050;
  const emit = (s, packet, payload) => s.emit('message', Buffer.from(JSON.stringify({ type: 'packet', packet, payload })), false);
  class Socket extends EventEmitter {
    constructor(url, options) { super(); this.readyState = 0; this._closeFrameReceived = false; this.options = options; sockets.add(this);
      queueMicrotask(() => { this.readyState = 1; emit(this, 'Connected', {});
        this.emit('message', Buffer.from(JSON.stringify({ type: 'worldSnapshot', payload: { ...snapshot(), playerObjectId: 0, playerHp: 0,
          entities: [], inventoryItems: [], equipmentItems: [], beltItems: [] } })), false); }); }
    send(text) {
      const c = JSON.parse(text); commands.push({ command: c.type, ordinal: this.ordinal, deadlineHandshake: this.options.handshakeTimeout });
      if (c.type === 'clientVersion') emit(this, 'ClientVersion', { result: 1 });
      if (c.type === 'login') { this.ordinal = actors.findIndex(a => a.accountId === c.accountId); assert.ok(this.ordinal >= 0); emit(this, 'LoginSuccess', { characters: [{ index: 0, name: actors[this.ordinal].name, level: 40 }] }); }
      if (c.type === 'startGame') {
        this.inGame = true; this.sharedId = sharedId++; const a = actors[this.ordinal], state = states[this.ordinal], self = state.entities[0];
        emit(this, 'StartGame', { result: 4 }); emit(this, 'UserInformation', { name: a.name, objectId: 1000, hp: 674, level: 40, location: { x: self.x, y: self.y } });
        emit(this, 'MapInformation', { mapIndex: 440, fileName: 'DogYoHyun' });
        for (const peer of sockets) if (peer !== this && peer.inGame) { emit(this, 'ObjectPlayer', { objectId: peer.sharedId, name: actors[peer.ordinal].name }); emit(peer, 'ObjectPlayer', { objectId: this.sharedId, name: a.name }); }
        this.emit('message', Buffer.from(JSON.stringify({ type: 'worldSnapshot', payload: state })), false);
      }
      if (['removeItem', 'equipItem'].includes(c.type)) {
        const state = states[this.ordinal], baseline = snapshot(this.ordinal), worn = baseline.equipmentItems[0];
        if (c.type === 'removeItem') { state.inventoryItems = [...baseline.inventoryItems, { ...worn, slot: c.to }]; state.equipmentItems = baseline.equipmentItems.slice(1); }
        else { state.inventoryItems = baseline.inventoryItems; state.equipmentItems = baseline.equipmentItems; }
        emit(this, c.type === 'removeItem' ? 'RemoveItem' : 'EquipItem', { grid: c.grid, uniqueId: c.uniqueId, to: c.to, success: true });
        this.emit('message', Buffer.from(JSON.stringify({ type: 'worldSnapshot', payload: state })), false);
      }
      if (c.type === 'walk') {
        const self = states[this.ordinal].entities[0]; self.x += c.direction === 'Right' ? 1 : -1; self.direction = c.direction;
        emit(this, 'UserLocation', { location: { x: self.x, y: self.y }, direction: self.direction });
        for (const peer of sockets) if (peer !== this && peer.inGame) emit(peer, 'ObjectWalk', { objectId: this.sharedId, location: { x: self.x, y: self.y }, direction: self.direction });
      }
      if (c.type === 'chat') for (const peer of sockets) if (peer.inGame) emit(peer, 'ObjectChat', { objectId: this.sharedId, text: `${actors[this.ordinal].name}: ${c.message}` });
      if (c.type === 'logOut') { this.inGame = false; emit(this, 'LogOutSuccess', {}); for (const peer of sockets) if (peer.inGame) emit(peer, 'ObjectRemove', { objectId: this.sharedId }); }
    }
    close() { this.readyState = 3; this._closeFrameReceived = true; this.emit('close', 1000); }
    terminate() { this.readyState = 3; this.emit('close', 1006); }
  }
  return { WebSocket: Socket, commands };
}
function reviewedCandidate(schema = 'mir2.comparable.spectator-scene-build.v1') {
  const manifest = { schema, baseRevision: '31e9b6a74661c31d6515dd7322b466285f975d71',
    changesKind: schema === 'mir2.comparable.observer-timing-build.v1' ? 'observer-timing-only' : 'spectator-scene-projection',
    buildActualExitCode: 0, binarySha256: 'b'.repeat(64), sourceInputManifestSha256: 'c'.repeat(64) };
  const candidateBuild = { manifest, sha256: hash(JSON.stringify(manifest)) };
  return { candidateBuild, runtime: { sourceRevision: manifest.baseRevision, ...manifest, buildManifestSha256: candidateBuild.sha256 } };
}

test('reviewed candidate metadata labels both exact overlays and retains legacy source pins', () => {
  assert.deepEqual(runtimeSourceProof({ sourceRevision: 'a'.repeat(40), binarySha256: 'b'.repeat(64) }),
    { sourceProof: 'owned-controller-receipt', sourceRevisionIsBaseOnly: false, additionalSource: null });
  for (const schema of ['mir2.comparable.observer-timing-build.v1', 'mir2.comparable.spectator-scene-build.v1']) {
    const { candidateBuild, runtime } = reviewedCandidate(schema), proof = runtimeSourceProof(runtime, candidateBuild);
    assert.equal(proof.sourceRevisionIsBaseOnly, true); assert.equal(proof.additionalSource.buildManifestSchema, schema);
    assert.equal(proof.additionalSource.changesKind, candidateBuild.manifest.changesKind);
    assert.equal(proof.additionalSource.observerTimingOnly, schema.includes('observer-timing'));
    assert.equal(proof.additionalSource.cleanBaseBuild, false); assert.equal(proof.additionalSource.behaviorFixClaimed, false);
    assert.equal(proof.additionalSource.capacityClaimAllowed, false);
    assert.throws(() => runtimeSourceProof(runtime), /requires explicit --build-manifest/);
  }
});

test('candidate cross-labels, unknown schemas, failed builds and every runtime/hash mismatch fail closed', () => {
  for (const mutate of [
    c => { c.manifest.schema = 'mir2.comparable.any-build.v1'; }, c => { c.manifest.schema = 'toString'; },
    c => { c.manifest.schema = '__proto__'; }, c => { c.manifest.changesKind = 'observer-timing-only'; },
    c => { c.manifest.baseRevision = 'a'.repeat(40); }, c => { c.manifest.buildActualExitCode = 1; },
    c => { c.manifest.binarySha256 = 'bad'; }, c => { c.manifest.sourceInputManifestSha256 = ''; }, c => { c.sha256 = ''; },
  ]) { const { candidateBuild, runtime } = reviewedCandidate(); mutate(candidateBuild); assert.throws(() => runtimeSourceProof(runtime, candidateBuild)); }
  for (const key of ['sourceRevision', 'baseRevision', 'changesKind', 'buildActualExitCode', 'binarySha256', 'sourceInputManifestSha256', 'buildManifestSha256', 'buildManifestSchema']) {
    const { candidateBuild, runtime } = reviewedCandidate(); runtime[key] = 'wrong';
    assert.throws(() => runtimeSourceProof(runtime, candidateBuild), /pins|required|binding/);
  }
});

test('CLI provenance options are explicit C-only paths and keep melee opt-in requirements', () => {
  const abs = name => path.resolve(process.platform === 'win32' ? 'C:/synthetic-comparable' : '/synthetic-comparable', name);
  const base = ['--plan', abs('plan.json'), '--cohort', abs('cohort.json'), '--runtime-receipt', abs('runtime.json'),
    '--ledger', abs('ledger.json'), '--output', abs('report'), '--ws-module', abs('ws.cjs')];
  assert.equal(parseComparisonOptions(base)['--plan'], abs('plan.json'));
  assert.throws(() => parseComparisonOptions([...base, '--build-manifest', abs('build.json')]), /C-only|adapter-sources/);
  for (const option of ['--adapter-sources', '--build-manifest']) for (const bad of ['', 'relative.json', 'E:/candidate.json', 'https://example.com/file']) {
    assert.throws(() => parseComparisonOptions([...base, option, bad]));
  }
  assert.throws(() => parseComparisonOptions([...base, '--adapter-sources', '', '--adapter-sources', abs('sources.json')]), /duplicate/);
  assert.throws(() => parseComparisonOptions(base, ['--combat-contract']), /combat-contract/);
  if (process.platform === 'win32') {
    const opts = parseComparisonOptions([...base, '--build-manifest', abs('build.json'), '--adapter-sources', abs('sources.json'), '--combat-contract', abs('combat.json')], ['--combat-contract']);
    assert.equal(opts['--adapter-sources'], abs('sources.json'));
  }
});

test('candidate or declared adapter provenance without verified executing bundle is rejected before auth or WS', async () => {
  const p = plan(), actors = [0, 1].map(ordinal => ({ ordinal, accountId: `actor${ordinal}`, name: `Hero${ordinal}`, characterIndex: 0, password: 'test' }));
  let socketCalls = 0, reservations = 0;
  const { runtime: candidateRuntime, candidateBuild } = reviewedCandidate();
  const baseline = { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory', endpoint: 'ws://127.0.0.1:19010/ws' };
  for (const [runtime, candidate] of [[{ ...candidateRuntime, ...baseline }, candidateBuild],
    [{ ...baseline, sourceRevision: 'a'.repeat(40), binarySha256: 'b'.repeat(64), adapterSourceManifestSha256: 'e'.repeat(64), adapterEntrypointSha256: 'f'.repeat(64) }, null]]) {
    await assert.rejects(runComparison({ plan: p, cohort: { schema: 'mir2.comparable.modern-cohort.v1', qualificationLayer: 'game-core-local', mapSha256: MAP_SHA, actors }, runtime,
      candidateBuild: candidate, url: baseline.endpoint, WebSocket: class { constructor() { socketCalls++; } }, ledger: {},
      saveLedger: async () => { reservations++; }, emit() {} }), /Verified --adapter-sources required/);
  }
  assert.equal(socketCalls, 0); assert.equal(reservations, 0);
});

test('actual C bundle entrypoint, dependency bytes and external WS source are verified before SDK-only execution', { skip: process.platform !== 'win32' }, async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mir2-adapter-source-sdk-'));
  assert.match(root, /^c:[\\/]/i);
  for (const name of ['driver.mjs', 'contract.mjs', 'combat.mjs']) await fs.copyFile(new URL(name, import.meta.url), path.join(root, name));
  const harness = [
    "import fs from 'node:fs/promises'; import assert from 'node:assert/strict'; import { EventEmitter } from 'node:events';",
    "import { loadAdapterSources, runtimeSourceProof, runComparison } from './driver.mjs';",
    "import { MAP_SHA, hash } from './contract.mjs'; import './combat.mjs';",
    `const rulesBytes=Buffer.from(${JSON.stringify(rulesBytes.toString('base64'))},'base64'),rules=JSON.parse(rulesBytes);`,
    plan.toString(), snapshot.toString(), sdkMockServer.toString(),
    "const runtime=JSON.parse(await fs.readFile(process.argv[2],'utf8')), manifest=process.argv[3], ws=process.argv[4];",
    "try { const adapterSources=await loadAdapterSources(manifest,runtime,ws);assert.ok(Object.isFrozen(adapterSources.adapterSource.files[0]));assert.throws(()=>{adapterSources.adapterSource.files[0].sha256='changed';},TypeError);",
    "if(process.argv[5]==='proofOnly'){ console.log(JSON.stringify(adapterSources)); } else {",
    `const p=plan(),actors=[0,1].map(ordinal=>({ordinal,accountId:'actor'+ordinal,name:'Hero'+ordinal,characterIndex:0,password:'synthetic-password'}));`,
    "const sdk=sdkMockServer(actors,p), emitted=[],reservations=[]; let clock=0;",
    "const candidateBuild={manifest:runtime.candidateManifest,sha256:runtime.buildManifestSha256},ledger={schema:'mir2.comparable.local-auth-ledger.v1',endpoint:runtime.endpoint,attempts:[]};",
    "const report=await runComparison({plan:p,cohort:{schema:'mir2.comparable.modern-cohort.v1',qualificationLayer:'game-core-local',mapSha256:MAP_SHA,actors},runtime,candidateBuild,adapterSources,url:runtime.endpoint,WebSocket:sdk.WebSocket,ledger,saveLedger:async l=>reservations.push(l.attempts.length),emit:e=>emitted.push(e),now:()=>clock,wait:async ms=>{clock+=ms;}});",
    "assert.equal(report.ok,true);assert.equal(report.actions.length,64);assert.equal(ledger.attempts.length,12);assert.equal(report.equipment.flatMap(r=>r.rounds).length,8);assert.equal(report.cleanup.length+report.savedVerification.length,4);",
    "assert.equal(report.sourceRevisionIsBaseOnly,true);assert.equal(report.additionalSource.cleanBaseBuild,false);assert.equal(report.adapterSourceManifestSha256,runtime.adapterSourceManifestSha256);assert.equal(report.adapterEntrypointSha256,runtime.adapterEntrypointSha256);",
    "console.log(JSON.stringify({synthetic:true,liveEvidence:false,actions:report.actions.length,gear:report.equipment.flatMap(r=>r.rounds).length,auth:ledger.attempts.length,...adapterSources})); }",
    "} catch(error) { console.error(error.message); process.exitCode=1; }",
  ].join('\n');
  await fs.writeFile(path.join(root, 'melee-driver.mjs'), harness);
  const ws = path.join(root, 'ws.cjs'), wsBytes = Buffer.from('module.exports = class SyntheticOnly {};\n'); await fs.writeFile(ws, wsBytes);
  const files = [];
  for (const name of ['driver.mjs', 'contract.mjs', 'combat.mjs', 'melee-driver.mjs']) { const content = await fs.readFile(path.join(root, name)); files.push({ path: name, bytes: content.length, sha256: hash(content) }); }
  const manifest = { schema: 'mir2.comparable.modern-adapter-sources.v1', entrypoint: 'melee-driver.mjs', files, wsModule: { path: ws, bytes: wsBytes.length, sha256: hash(wsBytes) } };
  const manifestPath = path.join(root, 'adapter-sources.json'), runtimePath = path.join(root, 'runtime.json'), { runtime, candidateBuild } = reviewedCandidate();
  Object.assign(runtime, { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory', endpoint: 'ws://127.0.0.1:19010/ws',
    candidateManifest: candidateBuild.manifest, adapterSourceManifestSha256: hash(JSON.stringify(manifest)), adapterEntrypointSha256: files.find(f => f.path === manifest.entrypoint).sha256 });
  const write = async (m = manifest, r = runtime) => { await fs.writeFile(manifestPath, JSON.stringify(m)); await fs.writeFile(runtimePath, JSON.stringify(r)); };
  const invoke = async (mode = 'proofOnly') => promisify(execFile)(process.execPath, [path.join(root, 'melee-driver.mjs'), runtimePath, manifestPath, ws, mode], { timeout: 30000, maxBuffer: 1048576 });
  await write(); const full = JSON.parse((await invoke('fullSDK')).stdout);
  assert.equal(full.actions, 64); assert.equal(full.gear, 8); assert.equal(full.auth, 12); assert.equal(full.liveEvidence, false);
  assert.equal(full.adapterSource.files.length, 4);
  await assert.rejects(loadAdapterSources(manifestPath, runtime, ws), /unrelated to the executing driver/);
  for (const mutate of [m => { m.entrypoint = 'driver.mjs'; }, m => { m.files[1].sha256 = '0'.repeat(64); },
    m => { m.files[1].bytes++; }, m => { m.files[0].path = '../driver.mjs'; }, m => { m.files.push(m.files[0]); },
    m => { m.files = m.files.filter(f => f.path !== 'combat.mjs'); }, m => { m.wsModule.path = path.join(root, 'different.cjs'); },
    m => { m.wsModule.sha256 = '0'.repeat(64); }, m => { m.schema = 'unknown'; }]) {
    const bad = structuredClone(manifest); mutate(bad); await write(bad, { ...runtime, adapterSourceManifestSha256: hash(JSON.stringify(bad)) });
    await assert.rejects(invoke(), error => error.code === 1);
  }
  for (const key of ['adapterSourceManifestSha256', 'adapterEntrypointSha256']) { await write(manifest, { ...runtime, [key]: '0'.repeat(64) }); await assert.rejects(invoke(), /Runtime adapter source/); }
  const original = await fs.readFile(path.join(root, 'contract.mjs')); await fs.appendFile(path.join(root, 'contract.mjs'), '\n// changed after manifest\n');
  await write(); await assert.rejects(invoke(), /file bytes\/hash mismatch/); await fs.writeFile(path.join(root, 'contract.mjs'), original);
  await fs.writeFile(path.join(root, 'extra.mjs'), 'export const syntheticOnly = true;\n');
  await fs.appendFile(path.join(root, 'contract.mjs'), "\nimport './extra.mjs';\n");
  const graphBad = structuredClone(manifest), changed = await fs.readFile(path.join(root, 'contract.mjs'));
  Object.assign(graphBad.files.find(f => f.path === 'contract.mjs'), { bytes: changed.length, sha256: hash(changed) });
  await write(graphBad, { ...runtime, adapterSourceManifestSha256: hash(JSON.stringify(graphBad)) });
  await assert.rejects(invoke(), /static adapter dependency is absent/); await fs.writeFile(path.join(root, 'contract.mjs'), original);
  const linked = path.join(root, 'linked'); await fs.symlink(root, linked, 'junction');
  await assert.rejects(loadAdapterSources(path.join(linked, 'adapter-sources.json'), runtime, ws), /Linked adapter source/);
  await write(); assert.equal(JSON.parse((await invoke()).stdout).adapterSourceManifestSha256, runtime.adapterSourceManifestSha256);
});

test('complete SDK-mocked comparison uses 12 ordinary Login messages, 8 gear fresh states, 4 final exits and fixed actions', async () => {
  const p = process.env.MIR2_COMPARABLE_SYNTHETIC_PLAN ? validatePlan(JSON.parse(await fs.readFile(process.env.MIR2_COMPARABLE_SYNTHETIC_PLAN, 'utf8'))) : plan();
  const actors = [0, 1].map(ordinal => ({ ordinal, accountId: `actor${ordinal}`, name: `Hero${ordinal}`, characterIndex: 0, password: 'local-test-password' }));
  p.equipmentActorOrdinals = [0, 1]; p.equipmentRoundsPerActor = 2; p.savedVerificationActorOrdinals = [0, 1];
  const sdk = sdkMockServer(actors, p), emitted = [], reservations = []; let clock = 0;
  const url = 'ws://127.0.0.1:19010/ws', ledger = { schema: 'mir2.comparable.local-auth-ledger.v1', endpoint: url, attempts: [] };
  const report = await runComparison({ plan: p, cohort: { schema: 'mir2.comparable.modern-cohort.v1', qualificationLayer: 'game-core-local', mapSha256: MAP_SHA, actors },
    runtime: { schema: 'mir2.comparable.modern-runtime.v1', qualificationLayer: 'game-core-local', backend: 'file', sessionCache: 'in-memory', sourceRevision: 'a'.repeat(40), binarySha256: 'b'.repeat(64), endpoint: url },
    url, WebSocket: sdk.WebSocket, ledger, saveLedger: async l => { reservations.push(l.attempts.length); }, emit: e => emitted.push(e), now: () => clock, wait: async ms => { clock += ms; } });
  assert.equal(report.ok, true); assert.equal(report.runCompleted, true); assert.equal(report.admitted, 2); assert.equal(report.actions.length, 64);
  assert.equal(report.equipment.flatMap(r => r.rounds).length, 8); assert.ok(report.equipment.flatMap(r => r.rounds).every(r => r.ack.connection !== r.state.connection && r.stateElapsedMs <= 15000));
  assert.equal(report.cleanup.length + report.savedVerification.length, 4); assert.ok(report.savedVerification.every(r => r.passed));
  assert.equal(ledger.attempts.length, 12); assert.deepEqual(reservations, Array.from({ length: 12 }, (_, i) => i + 1));
  assert.equal(sdk.commands.filter(c => c.command === 'login').length, 12); assert.equal(report.capacityClaimAllowed, false); assert.equal(report.productionStackQualified, false);
  assert.equal(report.steadyApplicationTraffic.actors.length, 2);
  assert.ok(report.steadyApplicationTraffic.actors.every(r => r.actualDurationMs === 60000 && r.bytesEnd.actualCounterAtMs === report.measurementEpochMs + 60000 && r.sentBytes > 0 && r.receivedBytes > 0));
  assert.ok(emitted.filter(e => e.event === 'actionTerminal').every(e => e.receipts.every(r => Number.isInteger(r.connection) && Number.isInteger(r.receiveSeq) && Number.isFinite(r.atMs))));
  if (process.env.MIR2_COMPARABLE_SYNTHETIC_OUTPUT) {
    const output = process.env.MIR2_COMPARABLE_SYNTHETIC_OUTPUT;
    assert.match(output.replaceAll('\\', '/'), /^C:\/Users\/Administrator\/AppData\/Local\/Temp\/mir2-comparable-capacity-20261113\/modern-audit\/synthetic-/i);
    const base = Date.parse('2026-11-13T00:00:00.000Z');
    const normalize = value => Array.isArray(value) ? value.map(normalize) : value && typeof value === 'object' ? (() => {
      const result = Object.fromEntries(Object.entries(value).map(([k, v]) => [k, normalize(v)]));
      if (Number.isFinite(result.atMs)) { result.atUtc = new Date(base + result.atMs).toISOString(); if ('receiveAtUtc' in result) result.receiveAtUtc = result.atUtc; }
      return result;
    })() : value;
    const sample = normalize(report); sample.synthetic = true; sample.liveEvidence = false; sample.sourceProof = 'SDK-MOCK-NO-GATEWAY';
    sample.startedAtUtc = new Date(base).toISOString(); sample.measurementStartedAtUtc = new Date(base + report.measurementEpochMs).toISOString();
    sample.measurementPlannedEndAtUtc = new Date(base + report.measurementEpochMs + p.durationMs).toISOString();
    sample.measurementEndedAtUtc = new Date(base + report.observationEndedAtMs).toISOString();
    sample.observationEndedAtUtc = sample.measurementEndedAtUtc;
    sample.completedAtUtc = new Date(base + clock).toISOString(); sample.planSha256 = hash(await fs.readFile(process.env.MIR2_COMPARABLE_SYNTHETIC_PLAN));
    const lines = emitted.map(e => JSON.stringify({ ...normalize(e), synthetic: true, liveEvidence: false })).join('\n') + '\n';
    sample.eventsSha256 = hash(lines); await fs.mkdir(output);
    await fs.writeFile(output + '/sample.events.jsonl', lines, { flag: 'wx' }); await fs.writeFile(output + '/sample.report.json', JSON.stringify(sample, null, 2) + '\n', { flag: 'wx' });
    await fs.copyFile(process.env.MIR2_COMPARABLE_SYNTHETIC_PLAN, output + '/sample.plan.json', 1);
  }
});

test('overdue scheduler with100 retained6000 histories yields to real timer and file I/O before finishing offers', async () => {
  const p = plan(100), actions = p.actions.slice(0, 512), original = hash(actions), captures = [];
  let reads = 0, timerRan = false, ioRan = false, callbacksBeforeLast = false;
  const nonce = 'bounded-io-fixture';
  const cs = Array.from({ length: 100 }, (_, ordinal) => {
    const raw = Array.from({ length: 6000 }, (_, i) => ({ receiveSeq: i + 1, packet: 'KeepAlive', payload: {}, atMs: i }));
    const events = new Proxy(raw, { get(target, key, receiver) { if (/^\d+$/.test(String(key))) reads++; return Reflect.get(target, key, receiver); } });
    const c = { ordinal, connection: ordinal + 1, events, gate: new PendingGate(row => captures.push(row)), reconcileLate() {} };
    c.gate.start({ kind: 'chat', deadlineAtMs: 2500 }, 0);
    Object.assign(c.gate.pending, { expected: [{ client: c, packet: 'ObjectChat', match: e => chatMatches(e.payload, 50000 + ordinal, nonce) }], cursors: new Map([[ordinal, 6000]]) });
    return c;
  });
  const timer = setTimeout(() => { timerRan = true; }, 0);
  const io = fs.readFile(new URL('./driver.mjs', import.meta.url)).then(() => {
    ioRan = true;
    for (const c of cs) c.events.push({ receiveSeq: 6001, packet: 'ObjectChat', payload: { objectId: 50000 + c.ordinal, text: nonce }, atMs: 1000000, atUtc: '2026-11-14T00:00:00Z' });
  });
  const offered = [];
  try {
    await runScheduledActions(actions, { epoch: 0, now: () => 1000000, wait: async () => assert.fail('All test offers are already overdue'),
      advance: () => observeActions(cs), offer(action, current) {
        assert.equal(current, 1000000); offered.push(action);
        if (offered.length === actions.length) callbacksBeforeLast = timerRan && ioRan;
      } });
  } finally { await io; clearTimeout(timer); }
  observeActions(cs);
  assert.equal(callbacksBeforeLast, true, 'overdue offers must not monopolize the event loop');
  assert.deepEqual(offered, actions); assert.equal(hash(actions), original, 'absolute due/deadline and action count unchanged');
  assert.ok(reads < 1000000, `Incremental receipt lookup must not repeat head scans: ${reads}`);
  assert.equal(captures.filter(r => r.event === 'actionTerminal').length, 100);
  assert.ok(captures.filter(r => r.event === 'actionTerminal').every(r => !r.passed && r.timedOut), 'late I/O cannot become a pass');
});

test('incremental retained cursor equals first-match semantics through trims, duplicates and history replacement', () => {
  const match = e => e.packet === 'ObjectChat' && chatMatches(e.payload, 50001, 'correct-nonce');
  const event = (receiveSeq, objectId, text) => ({ receiveSeq, packet: 'ObjectChat', payload: { objectId, text } });
  let events = [event(1, 50001, 'correct-nonce'), event(2, 99999, 'correct-nonce'), event(3, 50001, 'wrong-nonce')];
  const cursor = new RetainedReceiveCursor(1, match);
  const verify = () => assert.equal(cursor.matches(events)[0], events.find(e => e.receiveSeq > 1 && match(e)));
  verify(); events.push(event(4, 50001, 'correct-nonce')); verify();
  events.push(event(5, 50001, 'correct-nonce')); verify(); assert.equal(cursor.matches(events)[0].receiveSeq, 4);
  events.splice(0, 4); verify(); assert.equal(cursor.matches(events)[0].receiveSeq, 5);
  events.splice(0); verify(); events.push(event(6, 50001, 'correct-nonce')); verify();
  events = [event(7, 50001, 'wrong-nonce'), event(8, 50001, 'correct-nonce')]; verify();
});

test('one receive can satisfy separate witnesses and a wrong-then-right owner batch retains the original predicate order', () => {
  const f = clients(), p = plan(), action = p.actions[0];
  offerAction(f.cs[0], f.cs, p, action, 0, 0); observeActions(f.cs);
  f.clock(10); f.cs[0].observe({ type: 'packet', packet: 'UserLocation', payload: { x: 9, y: 10 } });
  f.clock(11); f.cs[0].observe({ type: 'packet', packet: 'UserLocation', payload: { x: 11, y: 10 } });
  f.clock(12); f.cs[1].observe({ type: 'packet', packet: 'ObjectWalk', payload: { objectId: 50050, x: 11, y: 10 } });
  const pending = f.cs[0].gate.pending; pending.expected.push(pending.expected[1]);
  observeActions(f.cs);
  const terminal = f.emitted.find(e => e.event === 'actionTerminal');
  assert.equal(terminal.passed, true); assert.equal(terminal.receipts.length, 3);
  assert.equal(terminal.receipts[0].receiveSeq, f.cs[0].sequence);
  assert.equal(terminal.receipts[1].receiveSeq, terminal.receipts[2].receiveSeq);
  assert.doesNotThrow(() => JSON.stringify(f.emitted));
  assert.ok(!JSON.stringify(f.emitted).includes('"selected":') && !JSON.stringify(f.emitted).includes('"history":'));
});

test('cached owner receipt is evicted with front trim; late duplicates cannot be reused by a later nonce', () => {
  const f = clients(), p = plan(), chats = p.actions.filter(a => a.actorOrdinal === 0 && a.kind === 'chat');
  const [first, second] = chats; f.clock(first.dueMs); offerAction(f.cs[0], f.cs, p, first, 0, first.dueMs);
  f.clock(first.dueMs + 10); f.cs[0].observe({ type: 'packet', packet: 'ObjectChat', payload: { objectId: 50050, text: first.nonce } });
  const evicted = f.cs[0].sequence; observeActions(f.cs); assert.ok(f.cs[0].gate.pending);
  f.cs[0].events.splice(0); observeActions(f.cs); f.cs[0].gate.expire(first.deadlineMs);
  f.clock(first.deadlineMs + 10); f.cs[1].observe({ type: 'packet', packet: 'ObjectChat', payload: { objectId: 50050, text: first.nonce } });
  observeActions(f.cs); assert.ok(f.cs[0].gate.pending, 'trimmed owner match must not survive outside retained history');
  f.clock(first.deadlineMs + 20); f.cs[0].observe({ type: 'packet', packet: 'ObjectChat', payload: { objectId: 50050, text: first.nonce } });
  observeActions(f.cs); const late = f.emitted.find(e => e.event === 'actionTerminal');
  assert.equal(late.passed, false); assert.equal(late.timedOut, true); assert.ok(late.receipts[0].receiveSeq > evicted);
  f.clock(second.dueMs); offerAction(f.cs[0], f.cs, p, second, 0, second.dueMs);
  for (const c of f.cs) c.observe({ type: 'packet', packet: 'ObjectChat', payload: { objectId: 50050, text: first.nonce } });
  observeActions(f.cs); assert.ok(f.cs[0].gate.pending, 'old nonce cannot satisfy a new pending action');
  f.clock(second.dueMs + 30); for (const c of f.cs) c.observe({ type: 'packet', packet: 'ObjectChat', payload: { objectId: 50050, text: second.nonce } });
  observeActions(f.cs); assert.equal(f.emitted.filter(e => e.event === 'actionTerminal').at(-1).passed, true);
  assert.equal(f.cs[0].gate.failed, true, 'earlier timeout stays sticky'); assert.doesNotThrow(() => JSON.stringify(f.emitted));
});

