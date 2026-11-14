import test from 'node:test';
import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs/promises';
import crypto from 'node:crypto';
import { gunzipSync } from 'node:zlib';
import { parseCrystalCollisionMap, findProtocolWalkPath, protocolMapCellIsWalkable }
  from '../../../apps/web/scripts/quest-agent/protocol-navigation.mjs';
import { createNavigator } from '../../../apps/web/scripts/quest-agent/protocol-play.mjs';
import { poolFingerprints, validateExistingPool, validateLedger, validateMapProof, assertMonitorIdentity,
  buildLayout, walkToTarget, freshSafeState, prepareBatches, optionsFrom, canonical, MAP_METADATA_SHA256, NPC_METADATA_SHA256,
  validateTargetLayout, createTargetSelectionState, selectPreparationTarget, validateResumeEvidence, readPublicResumeFile, RESUME_V4_PINS }
  from './prepare_existing_public_cohort.mjs';

const endpoint = 'wss://165.154.65.136.sslip.io/playtest/ws', webBase = 'https://numeron.obelisk.build';
function pool() { return { schema: 'mir2.cloud-reference-private-actors.v2', endpoint, webBase,
  actors: Array.from({ length: 100 }, (_, ordinal) => ({ ordinal, name: `Prep${ordinal.toString().padStart(3, '0')}`,
    walletIdentity: { kind: 'synthetic-ed25519-wallet', secretKey: `fake-not-a-key-${ordinal}`, address: `fake-address-${ordinal}`,
      accountId: `fake-account-${ordinal}`, characterIndex: ordinal }, identityHistory: [] })) }; }
function pins(p) { const h = poolFingerprints(p); return { endpoint, webBase, poolIdentitySha256: h.identityFingerprintSha256, poolActorsSha256: h.canonicalActorArraySha256 }; }
function map(width = 60, height = 60) { return { mapFileName: '0', width, height, bounds: { minX: 0, minY: 0, maxX: width - 1, maxY: height - 1 }, blocked: new Uint8Array(width * height), doors: [] }; }
const row = () => ({ map_index: 1, map_file_name: '0', safe_zones: [{ location: { x: 30, y: 30 }, size: 10 }], movements: [] });
const layout = () => buildLayout(map(), row(), []);
function proof() { return { schema: 'mir2.scene-live-map-proof.v6', mapsComplete: true, readOnly: true,
  sourceRevision: 'a'.repeat(40), binarySha256: 'b'.repeat(64), identityBefore: { pid: 99, startTicks: 123, invocationId: 'c'.repeat(32) },
  identityAfter: { pid: 99, startTicks: 123, invocationId: 'c'.repeat(32) }, files: [{ path: '0.map.gz', bytes: 1234, sha256: 'd'.repeat(64) }] }; }

test('all100 exact existing nested identities are required before any login', () => {
  const p = pool(), expected = pins(p); assert.equal(validateExistingPool(p, expected).actors.length, 100);
  delete p.actors[1].walletIdentity.characterIndex; p.actors[1].characterIndex = 1;
  assert.throws(() => validateExistingPool(p, expected), /existing_nested_wallet_character_required/);
});
test('identity and complete actor-array fingerprints reject replacements and incidental pool writes', () => {
  const p = pool(), expected = pins(p); p.actors[0].walletIdentity.secretKey += '-changed';
  assert.throws(() => validateExistingPool(p, expected), /fingerprint_changed/);
  const q = pool(), qPins = pins(q); q.actors[0].cookie = 'fake-cookie';
  assert.throws(() => validateExistingPool(q, qPins), /fingerprint_changed/);
});
test('missing pool, duplicate identities and changed endpoint cannot auto-create', () => {
  const p = pool(), expected = pins(p); p.actors[2].name = p.actors[1].name;
  assert.throws(() => validateExistingPool(p, expected), /duplicate/);
  assert.throws(() => validateExistingPool({ ...pool(), endpoint: 'wss://other.invalid/ws' }, expected), /existing_100_pool/);
  assert.throws(() => validateExistingPool({ ...pool(), actors: [] }, expected), /existing_100_pool/);
});
test('unchanged real auth budget retains attempts and all original admission/total limits', () => {
  const now = Date.now(), attempts = Array.from({ length: 25 }, () => ({ at: now, kind: 'admission' }));
  const ledger = { schemaVersion: 1, purpose: 'cloud-reference-public-auth-budget', endpoint, webBase, attempts };
  const budget = validateLedger(ledger, { endpoint, webBase });
  assert.equal(budget.attempts, attempts); assert.ok(budget.delay('admission') > 900000); assert.equal(budget.delay('verification'), 0);
  assert.throws(() => validateLedger({ ...ledger, endpoint: 'other' }, { endpoint, webBase }), /existing_auth_ledger/);
});
test('live proof must be exact-map complete and before/after identity stable', () => {
  assert.equal(validateMapProof(proof()).row.bytes, 1234);
  for (const change of [p => { p.mapsComplete = false; }, p => { p.identityAfter.startTicks++; }, p => { p.files.push(p.files[0]); }]) {
    const p = proof(); change(p); assert.throws(() => validateMapProof(p));
  }
  const binding = validateMapProof(proof()), sample = { targetProcess: { pid: 99, startTicks: 123 }, targetUnit: { MainPID: 99 }, health: { revision: 'a'.repeat(40) } };
  assertMonitorIdentity(sample, binding); sample.targetProcess.startTicks++;
  assert.throws(() => assertMonitorIdentity(sample, binding), /identity_mismatch/);
});
test('100 destinations are unique, native-safe and every fixed five-person group is compact', () => {
  const l = layout(); assert.equal(new Set(l.targets.map(t => `${t.target.x},${t.target.y}`)).size, 100);
  for (const t of l.targets) assert.ok(Math.max(Math.abs(t.target.x - 30), Math.abs(t.target.y - 30)) <= 8);
  for (let group = 0; group < 20; group++) {
    const cells = l.targets.filter(t => t.group === group);
    assert.equal(cells.length, 5); assert.ok(cells.every(a => cells.every(b => Math.max(Math.abs(a.target.x - b.target.x), Math.abs(a.target.y - b.target.y)) <= 4)));
  }
});
test('targets exclude static NPCs, all source transfers and doors; insufficient real space fails', () => {
  const m = map(), r = row(); m.doors = [{ x: 30, y: 31 }]; r.movements = [{ source: { x: 32, y: 30 } }];
  const l = buildLayout(m, r, [{ map_file_name: '0', location: { x: 30, y: 30 } }]);
  for (const t of l.targets) for (const p of [{ x: 30, y: 31 }, { x: 32, y: 30 }, { x: 30, y: 30 }])
    assert.ok(Math.max(Math.abs(t.target.x - p.x), Math.abs(t.target.y - p.y)) > 1);
  const blocked = map(); blocked.blocked.fill(1);
  assert.throws(() => buildLayout(blocked, row(), []), /insufficient_unique/);
});
test('live-matched map0 origin 312,601 escapes the old added door moat without changing any collision or V1 target', async () => {
  // These bytes have the exact gzip digest in the failed run's reviewed live proof.
  const stored = await fs.readFile(new URL('../../../apps/web/lib/generated/crystal-map-pack/0.map.gz', import.meta.url));
  assert.equal(crypto.createHash('sha256').update(stored).digest('hex'), 'bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5');
  const m = parseCrystalCollisionMap(gunzipSync(stored), '0'), before = Buffer.from(m.blocked);
  const metaBytes = await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_respawn_manifest.json', import.meta.url));
  const npcBytes = await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_npc_info_manifest.json', import.meta.url));
  assert.equal(crypto.createHash('sha256').update(metaBytes).digest('hex'), MAP_METADATA_SHA256);
  assert.equal(crypto.createHash('sha256').update(npcBytes).digest('hex'), NPC_METADATA_SHA256);
  const r = JSON.parse(metaBytes).maps.find(r => r.map_file_name === '0'), npcs = JSON.parse(npcBytes).npcs;
  const l = buildLayout(m, r, npcs), start = { x: 312, y: 601 }, target = { x: 290, y: 614 };
  assert.equal(crypto.createHash('sha256').update(canonical(l.targets)).digest('hex'), '065c90ea7a103fdce4e2902075f1851393b88e9d3bc62400fc099b2c2b62154f');
  const route = obstacles => findProtocolWalkPath({ map: m, start, target, dynamicObstacles: obstacles, maxExpanded: 100000 });
  assert.equal(route([]).length - 1, 24);
  const oldDoorMoat = m.doors.flatMap(d => Array.from({ length: 9 }, (_, i) => ({ x: d.x + i % 3 - 1, y: d.y + Math.floor(i / 3) - 1 })));
  assert.equal(route([...l.hazards, ...oldDoorMoat]), null);
  const repaired = route(l.hazards); assert.ok(repaired); assert.equal(repaired.length - 1, 24);
  assert.deepEqual(Buffer.from(m.blocked), before);
  assert.ok(m.doors.every(d => !protocolMapCellIsWalkable(m, d)));
  const forbidden = new Set(l.hazards.map(p => `${p.x},${p.y}`));
  assert.ok(repaired.every(p => protocolMapCellIsWalkable(m, p) && !forbidden.has(`${p.x},${p.y}`)));
  for (const p of npcs.filter(n => n.map_file_name === '0').map(n => n.location)) assert.ok(forbidden.has(`${p.x},${p.y}`));
  for (const p of r.movements.map(t => t.source))
    for (let y = p.y - 1; y <= p.y + 1; y++) for (let x = p.x - 1; x <= p.x + 1; x++) assert.ok(forbidden.has(`${x},${y}`));
});

function targetClient(ordinal, point, entities = []) {
  const actor = pool().actors[ordinal];
  return { actor, ordinal, connection: ordinal + 1, inGame: true, closed: false, sequence: 635,
    events: [{ type: 'worldSnapshot', sequence: 635 }], snapshot: { mapFileName: '0', playerObjectId: 1000, playerHp: 18,
      mapTransfers: [], entities: [{ ...point, objectId: 1000, name: actor.name, kind: 'player', dead: false }, ...entities] } };
}

test('free reachable targets 0 through12 remain exact with no unnecessary relocation', () => {
  const l = layout(), state = createTargetSelectionState(l), original = canonical(l.targets), records = [];
  const context = { map: map(), assert() {}, emit(row) { records.push(row); } };
  for (let i = 0; i <= 12; i++) {
    const c = targetClient(i, l.targets[i].target);
    assert.deepEqual(selectPreparationTarget(c, l.targets[i], l, state, context), state.originalTargets[i].target);
  }
  assert.equal(canonical(l.targets), original); assert.equal(records.length, 13);
  assert.ok(records.every(r => !r.changed && r.reason === 'planned_target_free_and_reachable'));
});

test('real actor13 occupied goal gets the nearest legal reserved-safe alternative without relaxing collisions', async () => {
  const stored = await fs.readFile(new URL('../../../apps/web/lib/generated/crystal-map-pack/0.map.gz', import.meta.url));
  assert.equal(crypto.createHash('sha256').update(stored).digest('hex'), 'bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5');
  const m = parseCrystalCollisionMap(gunzipSync(stored), '0'), before = Buffer.from(m.blocked);
  const meta = await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_respawn_manifest.json', import.meta.url));
  const npc = await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_npc_info_manifest.json', import.meta.url));
  assert.equal(crypto.createHash('sha256').update(meta).digest('hex'), MAP_METADATA_SHA256);
  assert.equal(crypto.createHash('sha256').update(npc).digest('hex'), NPC_METADATA_SHA256);
  const row = JSON.parse(meta).maps.find(r => r.map_file_name === '0'), l = buildLayout(m, row, JSON.parse(npc).npcs);
  const state = createTargetSelectionState(l), reserved = new Set(state.reserved), records = [];
  for (let i = 0; i <= 12; i++) state.actual.set(i, { ...l.targets[i].target });
  const occupied = [{ objectId: 271801, kind: 'monster', x: 291, y: 616, dead: false },
    ...[10, 11, 12].map((i, n) => ({ objectId: [50086, 50087, 50088][n], kind: 'player', ...l.targets[i].target, dead: false }))];
  const c = targetClient(13, { x: 295, y: 628 }, occupied), context = { map: m, assert() {}, emit(r) { records.push(r); } };
  assert.deepEqual(l.targets[13].target, { x: 291, y: 616 });
  assert.equal(findProtocolWalkPath({ map: m, start: c.snapshot.entities[0], target: l.targets[13].target,
    dynamicObstacles: [...l.hazards, ...occupied], maxExpanded: 100000 }), null); // Actual V2 failure.
  const selected = selectPreparationTarget(c, l.targets[13], l, state, context);
  assert.deepEqual(selected, { x: 293, y: 616 });
  const route = findProtocolWalkPath({ map: m, start: c.snapshot.entities[0], target: selected,
    dynamicObstacles: [...l.hazards, ...occupied], maxExpanded: 100000 });
  assert.ok(route && route.length > 1); assert.ok(validateTargetLayout(l));
  assert.equal(new Set(l.targets.map(t => `${t.target.x},${t.target.y}`)).size, 100);
  assert.equal(reserved.has(`${selected.x},${selected.y}`), false);
  const blocked = new Set([...l.hazards, ...occupied].map(p => `${p.x},${p.y}`));
  assert.ok(route.every(p => protocolMapCellIsWalkable(m, p) && !blocked.has(`${p.x},${p.y}`)));
  assert.ok(m.doors.every(d => !protocolMapCellIsWalkable(m, d))); assert.deepEqual(Buffer.from(m.blocked), before);
  assert.equal(records[0].evidenceCursor, 635); assert.equal(records[0].snapshotSequence, 635);
  assert.equal(records[0].reason, 'planned_target_occupied'); assert.equal(records[0].originalOccupants[0].objectId, 271801);
  assert.ok(records[0].movementMargin && !reserved.has(`${records[0].movementMargin.x},${records[0].movementMargin.y}`));
  // A subsequent replacement cannot take a previously chosen actual target or its retained movement margin.
  const occupied14 = [...occupied, { objectId: 900014, kind: 'monster', ...l.targets[14].target, dead: false }];
  const next = selectPreparationTarget(targetClient(14, { x: 295, y: 628 }, occupied14), l.targets[14], l, state, context);
  assert.notDeepEqual(next, selected); assert.notDeepEqual(next, records[0].movementMargin); assert.ok(validateTargetLayout(l));
});

test('target selection refuses pending or previously sent movement and never reselects after an unknown write', () => {
  for (const pending of [true, false]) {
    const l = layout(), state = createTargetSelectionState(l), before = canonical(l.targets), records = [];
    const c = targetClient(0, { x: 10, y: 10 });
    if (pending) c.pendingMovementAck = { sequence: 634 };
    else c.events.push({ direction: 'sent', type: 'walk', sequence: 634 });
    assert.throws(() => selectPreparationTarget(c, l.targets[0], l, state,
      { map: map(), assert() {}, emit(r) { records.push(r); } }), /no_pending_or_prior_walk/);
    assert.equal(canonical(l.targets), before); assert.equal(state.actual.size, 0); assert.equal(records.length, 0);
  }
});

test('unreachable alternatives exhaust a finite original path budget and leave the100 targets unchanged', () => {
  const m = map(), l = layout(), state = createTargetSelectionState(l), before = canonical(l.targets), records = [];
  for (let y = 9; y <= 11; y++) for (let x = 9; x <= 11; x++) if (x !== 10 || y !== 10) m.blocked[y * m.width + x] = 1;
  assert.throws(() => selectPreparationTarget(targetClient(0, { x: 10, y: 10 }), l.targets[0], l, state,
    { map: m, assert() {}, emit(r) { records.push(r); } }), /no_legal_reachable_preparation_target/);
  assert.equal(canonical(l.targets), before); assert.equal(state.actual.size, 0); assert.equal(records.length, 1);
  assert.equal(records[0].selected, false); assert.equal(records[0].pathQueries, 64); assert.equal(records[0].actualTarget, null);
});

test('selection requires an actual authenticated snapshot cursor rather than manufacturing evidence', () => {
  const l = layout(), state = createTargetSelectionState(l), c = targetClient(0, { x: 10, y: 10 }); c.events = [];
  assert.throws(() => selectPreparationTarget(c, l.targets[0], l, state, { map: map(), assert() {}, emit() {} }), /evidence_cursor/);
});

function walkingClient({ late = false, run = false, unknown = false } = {}) {
  const actor = pool().actors[0], owner = { name: actor.name, objectId: 1000, kind: 'player', x: 10, y: 10, dead: false };
  const c = { actor, ordinal: 0, connection: 1, inGame: true, closed: false, sequence: 0, events: [],
    snapshot: { mapFileName: '0', playerObjectId: 1000, playerHp: 100, entities: [owner], mapTransfers: [] }, async pace() {},
    async wait(predicate) { const v = predicate(); if (!v) throw new Error('fake wait timeout'); return v; } };
  let calls = 0;
  const navigatorFactory = (client, deps) => {
    assert.equal(deps.movementResponseTimeoutMs, 2500);
    return async (next, distance, stop, options) => {
      calls++; assert.equal(Math.max(Math.abs(next.x - owner.x), Math.abs(next.y - owner.y)), 1);
      assert.equal(options.autoUseSupplies, false); assert.equal(options.maxEmergencyEscapesPerNavigation, 0);
      assert.equal(options.maxSuccessfulSteps, 1); assert.equal(options.maxNoPathRefreshes, 0);
      if (unknown) throw new Error('unknown response; no resend');
      const sentAt = performance.now(), dx = next.x - owner.x, dy = next.y - owner.y;
      const movementDirection = ({ '0,-1': 'Up', '1,-1': 'UpRight', '1,0': 'Right', '1,1': 'DownRight',
        '0,1': 'Down', '-1,1': 'DownLeft', '-1,0': 'Left', '-1,-1': 'UpLeft' })[`${dx},${dy}`];
      const requestSequence = ++client.sequence;
      client.events.push({ sequence: requestSequence, direction: 'sent', movementDirection, type: run ? 'run' : 'walk', monotonicMs: sentAt });
      client.events.push({ sequence: ++client.sequence, direction: 'received', packet: 'UserLocation', movementRequestSequence: requestSequence,
        monotonicMs: sentAt + (late ? 2501 : 10), payload: { ...next } });
      owner.x = next.x; owner.y = next.y; return { reached: true };
    };
  };
  return { c, navigatorFactory, calls: () => calls };
}
const walkContext = () => ({ deadline: Date.now() + 10000, assert() {}, emit() {} });
test('strict walk uses the next path cell, real owner ACK and bounded one-cell navigator', async () => {
  const f = walkingClient(); assert.equal(await walkToTarget(f.c, { x: 12, y: 10 }, walkContext(), map(), [], f), 2); assert.equal(f.calls(), 2);
});
test('the real ordinary navigator emits Walk rather than Run for the adjacent capped step', async () => {
  const f = walkingClient(), c = f.c, sent = []; c.record = () => {};
  c.send = command => {
    sent.push(command); assert.equal(command.type, 'walk'); assert.equal(command.direction, 'Right');
    const at = performance.now(), requestSequence = ++c.sequence;
    c.events.push({ sequence: requestSequence, direction: 'sent', movementDirection: command.direction, type: command.type, monotonicMs: at });
    c.snapshot.entities[0].x++;
    c.events.push({ sequence: ++c.sequence, direction: 'received', packet: 'UserLocation', movementRequestSequence: requestSequence, monotonicMs: at + 1,
      payload: { x: c.snapshot.entities[0].x, y: c.snapshot.entities[0].y } });
  };
  assert.equal(await walkToTarget(c, { x: 11, y: 10 }, walkContext(), map(), []), 1);
  assert.equal(sent.length, 1);
});
test('late owner ACK and accidental Run stay failed, never become successful preparation', async () => {
  for (const settings of [{ late: true }, { run: true }]) {
    const f = walkingClient(settings); await assert.rejects(walkToTarget(f.c, { x: 11, y: 10 }, walkContext(), map(), [], f)); assert.equal(f.calls(), 1);
  }
});
test('unknown write outcome is not retried and stale owner receipts cannot satisfy new walk', async () => {
  const f = walkingClient({ unknown: true });
  f.c.events.push({ sequence: 0, direction: 'received', packet: 'UserLocation', monotonicMs: performance.now(), payload: { x: 11, y: 10 } });
  await assert.rejects(walkToTarget(f.c, { x: 11, y: 10 }, walkContext(), map(), [], f)); assert.equal(f.calls(), 1);
});

test('real actor23 obstacle transition reproduces the old one-step error and accepts only its confirmed ordinary Walk', async () => {
  const stored = await fs.readFile(new URL('../../../apps/web/lib/generated/crystal-map-pack/0.map.gz', import.meta.url));
  assert.equal(crypto.createHash('sha256').update(stored).digest('hex'), 'bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5');
  const m = parseCrystalCollisionMap(gunzipSync(stored), '0');
  const meta = JSON.parse(await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_respawn_manifest.json', import.meta.url)));
  const npcs = JSON.parse(await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_npc_info_manifest.json', import.meta.url)));
  const l = buildLayout(m, meta.maps.find(r => r.map_file_name === '0'), npcs.npcs);
  const make = () => {
    const f = walkingClient(), c = f.c, owner = c.snapshot.entities[0], sent = [];
    owner.x = 298; owner.y = 632;
    const monster = { objectId: 206632, kind: 'monster', x: 298, y: 630, dead: false }; c.snapshot.entities.push(monster);
    c.record = () => {};
    c.send = command => {
      assert.equal(command.type, 'walk'); sent.push(command);
      const vector = ({ Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1],
        DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] })[command.direction]; assert.ok(vector);
      const at = performance.now(), sequence = ++c.sequence;
      c.events.push({ sequence, type: 'walk', direction: 'sent', movementDirection: command.direction, monotonicMs: at });
      owner.x += vector[0]; owner.y += vector[1];
      c.events.push({ sequence: ++c.sequence, direction: 'received', packet: 'UserLocation', movementRequestSequence: sequence,
        monotonicMs: at + 1, payload: { x: owner.x, y: owner.y } });
    };
    return { c, monster, sent };
  };
  // Same live coordinates as sequences637->638. The old requested adjacent
  // target becomes a two-step detour under unchanged no-corner-cut collision.
  const red = make(); red.monster.y = 631;
  const oldNavigator = createNavigator(red.c, { loadCollisionMap: async () => m, delay: async () => {}, movementResponseTimeoutMs: 2500 });
  await assert.rejects(oldNavigator({ x: 297, y: 631 }, 0, () => false, { autoUseSupplies: false,
    maxEmergencyEscapesPerNavigation: 0, maxSuccessfulSteps: 1, maxAttempts: 2, maxNoPathRefreshes: 0, forbiddenPoints: l.hazards }),
  e => e.message === 'Navigation successful step budget exceeded (1)' && e.successfulSteps === 1);
  assert.equal(red.sent.length, 1); assert.equal(red.sent[0].direction, 'Left');
  assert.deepEqual({ x: red.c.snapshot.entities[0].x, y: red.c.snapshot.entities[0].y }, { x: 297, y: 632 });
  for (const transition of ['duringPace', 'duringNavigator']) {
    const f = make(), records = []; let first = true;
    f.c.pace = async () => { if (transition === 'duringPace') f.monster.y = 631; };
    const factory = (client, deps) => {
      const nav = createNavigator(client, { ...deps, delay: async () => {} });
      return (...args) => { if (first) { first = false; if (transition === 'duringNavigator') f.monster.y = 631; } return nav(...args); };
    };
    const count = await walkToTarget(f.c, { x: 285, y: 617 },
      { deadline: Date.now() + 10000, assert() {}, emit(r) { records.push(r); } }, m, l.hazards, { navigatorFactory: factory });
    const receipts = records.filter(r => r.kind === 'preparationWalk');
    assert.equal(receipts.length, f.sent.length); assert.equal(count, receipts.length);
    assert.deepEqual(receipts[0].target, { x: 297, y: 632 });
    assert.equal(receipts[0].navigationOutcome, transition === 'duringPace' ? 'navigator_reached' : 'confirmed_single_walk_budget');
    assert.deepEqual(receipts[0].plannedTarget, transition === 'duringPace' ? { x: 297, y: 632 } : { x: 297, y: 631 });
    assert.ok(receipts.every(r => r.deadlineAtMs - r.sentAtMs === 2500 && r.ownerAtMs <= r.deadlineAtMs));
    assert.deepEqual({ x: f.c.snapshot.entities[0].x, y: f.c.snapshot.entities[0].y }, { x: 285, y: 617 });
  }
});

test('a step-budget error without one timely direction-correlated owner ACK never authorizes continuation', async () => {
  for (const mode of ['missingSend', 'missingAck', 'lateAck', 'wrongRequest', 'wrongDirection']) {
    const f = walkingClient(); let calls = 0;
    const factory = (client, deps) => {
      const normal = f.navigatorFactory(client, deps);
      return async (...args) => {
        calls++;
        if (mode !== 'missingSend') {
          await normal(...args);
          const ack = client.events.at(-1), sent = client.events.at(-2);
          if (mode === 'missingAck') client.events.pop();
          if (mode === 'lateAck') ack.monotonicMs = sent.monotonicMs + 2501;
          if (mode === 'wrongRequest') ack.movementRequestSequence--;
          if (mode === 'wrongDirection') sent.movementDirection = 'Left';
        }
        throw Object.assign(new Error('Navigation successful step budget exceeded (1)'), { successfulSteps: 1 });
      };
    };
    await assert.rejects(walkToTarget(f.c, { x: 11, y: 10 }, walkContext(), map(), [], { navigatorFactory: factory }));
    assert.equal(calls, 1);
  }
});

test('unrelated navigator errors remain sticky failures even after an observed step', async () => {
  const f = walkingClient(); let calls = 0;
  const factory = (client, deps) => {
    const normal = f.navigatorFactory(client, deps);
    return async (...args) => { calls++; await normal(...args); throw new Error('unrelated bounded fixture failure'); };
  };
  await assert.rejects(walkToTarget(f.c, { x: 11, y: 10 }, walkContext(), map(), [], { navigatorFactory: factory }), /unrelated bounded/);
  assert.equal(calls, 1);
});
test('live occupancy and malformed transfer bounds reject before a movement is sent', async () => {
  const occupied = walkingClient(); occupied.c.snapshot.entities.push({ objectId: 9000, x: 11, y: 10, dead: false, kind: 'npc' });
  await assert.rejects(walkToTarget(occupied.c, { x: 11, y: 10 }, walkContext(), map(), [], occupied), /no_ordinary/);
  assert.equal(occupied.calls(), 0);
  const malformed = walkingClient(); malformed.c.snapshot.mapTransfers = [{ bounds: { minX: 0, minY: 0, maxX: 999999999, maxY: 999999999 } }];
  await assert.rejects(walkToTarget(malformed.c, { x: 11, y: 10 }, walkContext(), map(), [], malformed), /malformed_live_transfer/);
  assert.equal(malformed.calls(), 0);
});
test('fresh safety requires a new ordinary snapshot, correct live owner, exact target and safe flag', async () => {
  const f = walkingClient(), c = f.c;
  c.send = () => { c.events.push({ sequence: ++c.sequence, direction: 'received', type: 'worldSnapshot', monotonicMs: performance.now() }); };
  c.snapshot.inSafeZone = true; assert.equal((await freshSafeState(c, { x: 10, y: 10 }, walkContext())).inSafeZone, true);
  c.snapshot.inSafeZone = false; await assert.rejects(freshSafeState(c, { x: 10, y: 10 }, walkContext()), /fresh_safe_target/);
  c.snapshot.inSafeZone = true; c.snapshot.playerHp = 0;
  await assert.rejects(freshSafeState(c, { x: 10, y: 10 }, walkContext()), /living_map0/);
});

function batches({ failWalkAt, closeFailsAt, stalePeer = false } = {}) {
  const p = pool(), l = layout(), active = new Map(), order = []; let maxActive = 0;
  const refresh = () => { for (const c of active.values()) c.snapshot.entities = [...active.values()].map(other => ({
    objectId: other === c ? 1000 : 50000 + other.ordinal, kind: 'player', name: other.actor.name,
    x: other.point.x + (stalePeer && other !== c ? 1 : 0), y: other.point.y, dead: other.dead === true })); };
  class FakeClient {
    constructor(e, actor, ordinal) { this.actor = actor; this.ordinal = ordinal; this.label = actor.name; this.connection = ordinal + 1; this.point = { x: 10, y: 10 }; this.closed = false;
      this.sequence = 1; this.events = [{ type: 'worldSnapshot', sequence: 1 }]; }
    async wait(predicate) { if (!predicate()) throw new Error('actual mutual AOI missing'); return true; }
    async close() { order.push(`close${this.ordinal}`); active.delete(this.ordinal); this.closed = true; this.inGame = false;
      this.logoutReceipt = { packet: 'LogOutSuccess', elapsedMs: 2 }; refresh(); if (closeFailsAt === this.ordinal) throw new Error('fake close failed'); }
  }
  const context = { endpoint, map: map(), assert() {}, emit() {}, assertPoolUnchanged() { validateExistingPool(p, pins(p)); } };
  const deps = { Client: FakeClient,
    async loginActor(c) { order.push(`login${c.ordinal}`); active.set(c.ordinal, c); maxActive = Math.max(maxActive, active.size);
      c.inGame = true; c.snapshot = { mapFileName: '0', playerObjectId: 1000, playerHp: 100, inSafeZone: false, entities: [] }; refresh(); },
    async walk(c, target) { order.push(`walk${c.ordinal}`); if (c.ordinal === failWalkAt) { c.dead = true; c.snapshot.playerHp = 0; throw new Error('ordinary field death'); }
      c.point = { ...target }; c.snapshot.inSafeZone = true; refresh(); return 1; },
    async fresh(c, target) { order.push(`safe${c.ordinal}`); assert.deepEqual(c.point, target); return { actorOrdinal: c.ordinal, inSafeZone: true, x: c.point.x, y: c.point.y }; } };
  const report = { prepared: [], groups: [], cleanup: [] };
  return { p, l, context, deps, report, order, maxActive: () => maxActive, active };
}
test('all100 prepare in original five-person groups; each reaches safety before next login', async () => {
  const f = batches(); await prepareBatches(f.p.actors, f.l, f.context, f.report, f.deps);
  assert.equal(f.maxActive(), 5); assert.equal(f.active.size, 0); assert.equal(f.report.prepared.length, 100); assert.equal(f.report.groups.length, 20);
  assert.ok(f.report.cleanup.every(r => r.normalLogout));
  for (let i = 0; i < 99; i++) assert.ok(f.order.indexOf(`safe${i}`) < f.order.indexOf(`login${i + 1}`));
  assert.ok(f.order.indexOf('close4') < f.order.indexOf('login5'));
});
test('death during travel fails current group but normal close is independent of living-state sampling', async () => {
  const f = batches({ failWalkAt: 1 }); await assert.rejects(prepareBatches(f.p.actors, f.l, f.context, f.report, f.deps), /field death/);
  assert.equal(f.active.size, 0); assert.deepEqual(f.report.cleanup.map(r => r.actorOrdinal), [0, 1]);
  assert.ok(f.report.cleanup.every(r => r.normalLogout)); assert.ok(!f.order.includes('login2'));
});
test('one close failure never skips the other four owned closes and blocks next group', async () => {
  const f = batches({ closeFailsAt: 0 }); await assert.rejects(prepareBatches(f.p.actors, f.l, f.context, f.report, f.deps), /normal_close_failed/);
  assert.equal(f.active.size, 0); assert.equal(f.report.cleanup.length, 5); assert.equal(f.report.cleanup[0].normalLogout, false);
  assert.ok(!f.order.includes('login5'));
});
test('same-map nearby but stale peer coordinates are not counted as actual mutual AOI', async () => {
  const f = batches({ stalePeer: true }); await assert.rejects(prepareBatches(f.p.actors, f.l, f.context, f.report, f.deps), /mutual AOI/);
  assert.equal(f.report.groups.length, 0); assert.equal(f.report.cleanup.length, 5);
});
test('CLI retains ordinary endpoint, finite wall, exact metadata and no login-not-before bypass', () => {
  const outside = path.join(os.tmpdir(), 'public-cohort-offline-only');
  const args = ['--private-directory', path.join(outside, 'private'), '--output', path.join(outside, 'output'), '--monitor', path.join(outside, 'monitor.json'),
    '--web-base', webBase, '--endpoint', endpoint, '--wall-seconds', '7200', '--pool-identity-sha256', 'a'.repeat(64),
    '--pool-actors-sha256', 'b'.repeat(64), '--map-root', path.join(outside, 'maps'), '--live-map-proof', path.join(outside, 'proof.json'),
    '--live-map-proof-sha256', 'c'.repeat(64), '--map-metadata', path.join(outside, 'maps.json'), '--map-metadata-sha256', MAP_METADATA_SHA256,
    '--npc-metadata', path.join(outside, 'npcs.json'), '--npc-metadata-sha256', NPC_METADATA_SHA256];
  assert.equal(optionsFrom(args).wallSeconds, 7200);
  assert.throws(() => optionsFrom([...args, '--resume-report', path.join(outside, 'resume.json')]), /paired_public_resume/);
  assert.equal(optionsFrom([...args, '--resume-report', path.join(outside, 'resume.json'), '--resume-report-sha256', 'd'.repeat(64)])['resume-report-sha256'], 'd'.repeat(64));
  assert.throws(() => optionsFrom([...args, '--login-not-before', '0']), /unknown/);
  const changed = [...args]; changed[changed.indexOf('--endpoint') + 1] = 'wss://other.invalid/ws';
  assert.throws(() => optionsFrom(changed), /original_public_endpoint/);
});

test('real actor35 old NPC moat exhausts64 paths but exact NPC tiles preserve the original seven-step route', async () => {
  const bytes = await fs.readFile(new URL('../../../apps/web/lib/generated/crystal-map-pack/0.map.gz', import.meta.url));
  assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'), 'bbd02c7d0125fe78983e2dea5f4aecc73b53d4b91517bde6fdbee9853bb183a5');
  const m = parseCrystalCollisionMap(gunzipSync(bytes), '0'), blockedBefore = Buffer.from(m.blocked);
  const metadata = JSON.parse(await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_respawn_manifest.json', import.meta.url)));
  const npcs = JSON.parse(await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_npc_info_manifest.json', import.meta.url)));
  const r = metadata.maps.find(r => r.map_file_name === '0'), l = buildLayout(m, r, npcs.npcs);
  const occupied = [[1,287,615],[2,284,615],[3,284,606],[4,294,619],[5,296,613],[6,292,603],[8,288,608],[42,291,610],[43,295,605],
    [200339,276,604],[200340,280,600],[200716,291,620],[206007,294,592],[206302,287,611],[206320,281,596],[206324,289,604],
    [206339,280,609],[206515,284,611],[206524,294,606],[206530,288,603],[206536,291,600],[206545,271,596],[206604,281,600],
    [206610,286,601],[206619,285,591],[206641,287,616],[206700,270,599],[206711,276,603],[206714,275,611],[271801,291,616],[271806,292,613]]
    .map(([objectId,x,y]) => ({objectId,x,y,kind:objectId<100?'npc':'monster',dead:false}));
  // The raw packets did not expose a blocking flag: conservatively block all30.
  const spells = [[46,278,606],[47,279,606],[48,280,606],[49,281,606],[50,284,606],[51,285,606],[52,286,606],[53,287,606],
    [54,288,606],[55,292,606],[56,293,606],[57,294,606],[58,295,606],[59,296,606],[60,297,606],[61,298,606],[62,298,607],
    [63,278,609],[64,278,610],[65,278,611],[66,278,612],[67,278,613],[68,278,614],[69,298,614],[70,298,615],[71,298,616],
    [72,298,619],[73,298,620],[74,298,621],[75,298,622]].map(([objectId,x,y])=>({objectId,x,y,kind:'spell',dead:false}));
  const transfers = [[347,188],[308,264],[326,288],[322,247],[354,319],[295,251],[295,250],[335,299],[335,300],[344,308],[345,308],
    [295,284],[295,285],[305,276],[307,311],[307,310],[317,302],[317,301],[289,292],[286,296],[278,239],[284,236],[271,256],
    [283,268],[396,230],[396,229],[400,225],[399,225],[404,221],[403,221],[414,243],[415,241],[416,241],[315,613],[266,630],
    [271,621],[295,649],[283,636],[282,636],[282,635],[274,636],[302,622],[311,631],[307,627],[314,474],[43,111],[664,215],
    [663,215],[324,41],[325,42],[326,43],[326,42],[325,41],[325,40],[326,40],[666,86],[667,87],[667,88],[316,649],[315,650],
    [316,650],[315,649],[315,648],[314,649],[310,600],[286,296],[347,189],[147,33],[148,34]].map(([x,y])=>({x,y}));
  const setup = layout => {
    const state=createTargetSelectionState(layout);
    layout.targets[13].target={x:293,y:616};
    for(let i=0;i<35;i++)state.actual.set(i,{...layout.targets[i].target});
    state.margins.add('294,616');
    const c=targetClient(35,{x:285,y:606},[...occupied,...spells]);
    c.sequence=679;c.events=[{type:'worldSnapshot',sequence:678}];
    c.snapshot.mapTransfers=transfers.map(({x,y})=>({bounds:{minX:x,maxX:x,minY:y,maxY:y}}));
    const records=[];return{state,c,records,context:{map:m,assert(){},emit(r){records.push(r);}}};
  };
  const moat=npcs.npcs.filter(n=>n.map_file_name==='0').flatMap(n=>Array.from({length:9},(_,i)=>({x:n.location.x+i%3-1,y:n.location.y+Math.floor(i/3)-1})));
  const old=buildLayout(m,r,npcs.npcs);old.hazards.push(...moat);const red=setup(old);
  assert.throws(()=>selectPreparationTarget(red.c,old.targets[35],old,red.state,red.context),/no_legal_reachable/);
  assert.equal(red.records[0].pathQueries,64);assert.equal(red.records[0].actualTarget,null);
  const green=setup(l);assert.deepEqual(l.targets[35].target,{x:287,y:612});
  assert.deepEqual(selectPreparationTarget(green.c,l.targets[35],l,green.state,green.context),{x:287,y:612});
  const obstacles=[...l.hazards,...occupied,...spells,...transfers];
  const route=findProtocolWalkPath({map:m,start:{x:285,y:606},target:l.targets[35].target,dynamicObstacles:obstacles,maxExpanded:100000});
  assert.deepEqual(route,[{x:285,y:606},{x:285,y:607},{x:285,y:608},{x:285,y:609},{x:285,y:610},{x:286,y:611},{x:286,y:612},{x:287,y:612}]);
  const forbidden=new Set(obstacles.map(p=>`${p.x},${p.y}`));
  assert.ok(route.slice(1).every(p=>protocolMapCellIsWalkable(m,p)&&!forbidden.has(`${p.x},${p.y}`)));
  assert.ok(!l.goalCells.some(p=>p.x===285&&p.y===606));assert.equal(green.records[0].changed,false);
  assert.deepEqual(Buffer.from(m.blocked),blockedBefore);assert.ok(validateTargetLayout(l));
});

test('NPC neighbours are transit only while occupied NPC spell transfer and door tiles stay blocked', () => {
  const m=map(),r=row(),npc={map_file_name:'0',location:{x:30,y:30}},l=buildLayout(m,r,[npc]);
  assert.ok(l.hazards.some(p=>p.x===30&&p.y===30));assert.ok(!l.hazards.some(p=>p.x===31&&p.y===30));
  assert.ok(!l.goalCells.some(p=>Math.max(Math.abs(p.x-30),Math.abs(p.y-30))<=1));
  assert.equal(findProtocolWalkPath({map:m,start:{x:29,y:30},target:{x:30,y:30},dynamicObstacles:l.hazards}),null);
  assert.ok(findProtocolWalkPath({map:m,start:{x:31,y:30},target:{x:31,y:31},dynamicObstacles:l.hazards}));
  for(const kind of ['npc','monster','spell','player'])assert.equal(findProtocolWalkPath({map:m,start:{x:31,y:30},target:{x:31,y:31},
    dynamicObstacles:[...l.hazards,{x:31,y:31,kind}]}),null);
});

function resumeFixture() {
  const l=layout(), p=pool(), options={...pins(p),'live-map-proof-sha256':'a'.repeat(64)}, binding=validateMapProof(proof());
  const manifest={schema:'mir2.public-existing-cohort-resume-manifest.v1',...Object.fromEntries(Object.entries(RESUME_V4_PINS).map(([k,v])=>[k,{path:path.join(os.tmpdir(),`${k}.json`),sha256:v}])),
    freshBefore:{path:path.join(os.tmpdir(),'fresh-before.json'),sha256:'f'.repeat(64)}};
  const fp=poolFingerprints(p), original=structuredClone(l.targets), prepared=original.slice(0,35).map((t,i)=>({actorOrdinal:i,group:Math.floor(i/5),connection:i+1,
    snapshotSequence:100+i,snapshotAtMs:1000+i,mapFileName:'0',inSafeZone:true,hp:18,x:t.target.x,y:t.target.y}));
  const selections=original.slice(0,35).map((t,i)=>({actorOrdinal:i,selected:true,originalTarget:{...t.target},actualTarget:{...t.target},movementMargin:null}));
  const prior={schema:'mir2.public-existing-cohort-preparation.v1',runId:'old-failed-v4',ok:false,failure:{code:'no_legal_reachable_preparation_target'},
    preparationOnly:true,capacityEvidence:false,ccuQualified:false,actorsRequested:100,poolUnchanged:true,poolBefore:fp,poolAfter:fp,
    mapFileName:'0',safeZoneIndex:0,storedMapSha256:binding.row.sha256,mapMetadataSha256:MAP_METADATA_SHA256,npcMetadataSha256:NPC_METADATA_SHA256,
    liveMapProofSha256:options['live-map-proof-sha256'],sourceRevision:binding.sourceRevision,binarySha256FromProof:binding.binarySha256,liveMapIdentity:binding.identity,
    startedAtUtc:new Date(200).toISOString(),endedAtUtc:new Date(300).toISOString(),originalTargets:original,prepared,
    groups:Array.from({length:7},(_,g)=>({group:g,actorOrdinals:Array.from({length:5},(_,i)=>g*5+i),actualMutualAoi:true,maximumDistance:4})),
    targetSelections:selections,targetReceiptSha256:RESUME_V4_PINS.targets,
    cleanup:Array.from({length:36},(_,i)=>({actorOrdinal:i,connection:i+1,initiallyInGame:true,normalLogout:true,logoutPacket:'LogOutSuccess',logoutElapsedMs:10}))};
  const targets={schema:'mir2.public-existing-cohort-targets.v1',runId:prior.runId,preparedAll:false,poolIdentitySha256:options.poolIdentitySha256,
    mapFileName:'0',storedMapSha256:binding.row.sha256,liveMapProofSha256:prior.liveMapProofSha256,mapMetadataSha256:MAP_METADATA_SHA256,nativeSafeZone:l.zone,
    originalTargets:structuredClone(original),targets:structuredClone(original),targetSelections:selections,preparedOrdinals:Array.from({length:35},(_,i)=>i)};
  const preflight=(phase,at)=>({schema:'mir2.existing-public-pool-preflight.v3',phase,passed:true,identitiesGenerated:false,ledgerWrittenOrReset:false,readOnlyPrivateInputs:true,
    sourceLockSha256:'581ceed8f6b9f0c32b7b49035211fa3ccd703dbd5c2b64e673d8387d983a899d',sourcePolicy:{passed:true,policyId:'exact-old-runtime-except-top-level-web-package-scripts-v1'},
    pool:{actorCount:100,...fp,capturedAtMs:at}});
  const before=preflight('before',100),after=preflight('after',400),freshBefore=preflight('before',500);
  after.comparison={baselineReceiptSha256:RESUME_V4_PINS.before,actorIdentityUnchanged:true,entireCanonicalActorArrayUnchanged:true,allUnexpiredLedgerAttemptsRetained:true};
  const events=prepared.map(r=>({actorOrdinal:r.actorOrdinal,connection:r.connection,sequence:r.snapshotSequence,direction:'received',kind:'worldSnapshot',
    ownerNameMatches:true,ownerNameHash:'e'.repeat(64),expectedNameHash:'e'.repeat(64),mapFileName:'0',ownerHp:r.hp,ownerX:r.x,ownerY:r.y,monotonicMs:r.snapshotAtMs}));
  for(const r of prior.cleanup)events.push({actorOrdinal:r.actorOrdinal,connection:r.connection,kind:'logoutReceipt',packet:'LogOutSuccess',elapsedMs:10,monotonicMs:2000,observedAfter5000Ms:false},
    ...['validCloseFrameReceived','webSocketClose'].map((edge,i)=>({actorOrdinal:r.actorOrdinal,connection:r.connection,kind:'webSocketClosing',edge,closeFrameSent:true,closeFrameReceived:true,monotonicMs:2010+i})));
  const job={schema:'mir2.cloud-client-job-result.v4',actualExitCode:1,sourceLockSha256:before.sourceLockSha256,allOwnedMembersExited:true,
    jobClose:{closed:true,allMembersExited:true,killAppliedByClose:false,rootExitCodes:{cloudClient:1},failedLaunches:[]},forceKillUsed:false,
    finalJobSample:{jobActiveProcesses:0,members:[],memberPids:[]},clientRuntimeSourcePolicyPassed:true,runtimeSourceObservations:{beforeLaunch:{passed:true},afterExit:{passed:true}},
    processEnvelopePassed:false,allLiveResourceSamplesQualified:false,processCreationToExitResourceEvidenceComplete:false,startupResourceGapCount:1,normalDrainMarker:null};
  return{manifest,evidence:{report:prior,targets,events,before,after,job,freshBefore},parameters:{options,layout:l,binding,nowMs:600},p};
}

test('resume reuses exactly35 closed historical actors and normal-admits only65 without promoting old flags', async () => {
  const r=resumeFixture(), resume=validateResumeEvidence(r.manifest,r.evidence,r.parameters), f=batches();
  f.context.resume=resume;
  await prepareBatches(f.p.actors,f.l,f.context,f.report,f.deps);
  assert.equal(f.report.inheritedPrepared.length,35);assert.equal(f.report.prepared.length,65);
  assert.equal(f.report.inheritedGroups.length,7);assert.equal(f.report.groups.length,13);assert.equal(f.report.cleanup.length,65);
  assert.equal(f.order.filter(s=>s.startsWith('login')).length,65);assert.equal(f.order[0],'login35');
  assert.ok(!f.order.includes('login0'));assert.ok(!f.order.includes('login34'));assert.equal(f.active.size,0);assert.equal(f.maxActive(),5);
  assert.equal(resume.summary.priorRunPassed,false);assert.equal(resume.summary.priorActualExitCode,1);
  assert.equal(resume.summary.priorProcessEnvelopePassed,false);assert.equal(resume.summary.priorProcessCreationToExitResourceEvidenceComplete,false);
  assert.equal(resume.summary.closeCodeObserved,null);assert.equal(resume.summary.all100FreshThisRun,false);
});

test('resume refuses wrong pool partial groups snapshot mismatch abnormal close and owned-job kill', () => {
  const changes=[
    r=>{r.evidence.report.poolAfter.identityFingerprintSha256='0'.repeat(64);},
    r=>{r.evidence.report.prepared.pop();},r=>{r.evidence.report.groups.pop();},r=>{r.evidence.report.prepared[0].hp=0;},
    r=>{r.evidence.report.prepared[0].mapFileName='1';},r=>{r.evidence.events[0].ownerX++;},
    r=>{r.evidence.report.cleanup[35].normalLogout=false;},r=>{r.evidence.events.find(e=>e.edge==='webSocketClose').monotonicMs=8000;},
    r=>{r.evidence.job.jobClose.killAppliedByClose=true;},r=>{r.evidence.job.allOwnedMembersExited=false;},
    r=>{r.evidence.targets.targets[1].target={...r.evidence.targets.targets[0].target};},
    r=>{r.evidence.after.comparison.baselineReceiptSha256='0'.repeat(64);},r=>{r.evidence.freshBefore.pool.capturedAtMs=0;},
    r=>{r.parameters.nowMs=301000;},r=>{r.manifest.report.sha256='0'.repeat(64);},r=>{r.manifest.unreviewed={};},
  ];
  for(const change of changes){const r=resumeFixture();change(r);assert.throws(()=>validateResumeEvidence(r.manifest,r.evidence,r.parameters));}
});

test('resume cannot turn a new failure green or skip cleanup of its current group', async () => {
  const r=resumeFixture(),f=batches({failWalkAt:36});f.context.resume=validateResumeEvidence(r.manifest,r.evidence,r.parameters);
  await assert.rejects(prepareBatches(f.p.actors,f.l,f.context,f.report,f.deps),/field death/);
  assert.deepEqual(f.report.cleanup.map(r=>r.actorOrdinal),[35,36]);assert.ok(f.report.cleanup.every(r=>r.normalLogout));
  assert.equal(f.report.inheritedPrepared.length,35);assert.equal(f.report.prepared.length,1);assert.ok(!f.order.includes('login37'));
});

test('public resume input refuses private descendants bad hashes oversized files and hard links', async () => {
  const root=await fs.mkdtemp(path.join(os.tmpdir(),'mir2-public-resume-')), privateRoot=path.join(root,'private');await fs.mkdir(privateRoot);
  const file=path.join(root,'public.json'),bytes=Buffer.from('{}');await fs.writeFile(file,bytes);
  const digest=crypto.createHash('sha256').update(bytes).digest('hex');
  assert.deepEqual(await readPublicResumeFile(file,digest,16,privateRoot),bytes);
  await assert.rejects(readPublicResumeFile(file,'0'.repeat(64),16,privateRoot),/input_hash_changed/);
  await assert.rejects(readPublicResumeFile(file,digest,1,privateRoot),/bounded_input/);
  const privateFile=path.join(privateRoot,'forbidden.json');await fs.writeFile(privateFile,bytes);
  await assert.rejects(readPublicResumeFile(privateFile,digest,16,privateRoot),/public_outside/);
  const linked=path.join(root,'linked.json');await fs.link(file,linked);
  await assert.rejects(readPublicResumeFile(linked,digest,16,privateRoot),/linked_resume_input/);
});
test('actual CLI rejects before private reads/network and emits only a fixed failure code', () => {
  const r = spawnSync(process.execPath, [fileURLToPath(new URL('./prepare_existing_public_cohort.mjs', import.meta.url)), '--login-not-before', '0'], { encoding: 'utf8' });
  assert.equal(r.status, 1); assert.equal(r.stdout, '');
  assert.deepEqual(JSON.parse(r.stderr), { preparationOnly: true, capacityEvidence: false, ok: false, errorCode: 'preparation_preflight_or_evidence_failed' });
});
