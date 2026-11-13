// Preparation only: existing public wallets, ordinary login/walk/logout, no QA or save edits.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';
import { SignedClient, login, selectProbeActors, probeTransportForActor, secureDirectory } from './public_signed_probe.mjs';
import { LoginBudget, CapacityMonitor, BoundedEvidence } from '../../../apps/web/scripts/playtest-capacity-soak-core.mjs';
import { outsideRepository, validateEndpoint, observedPlayerId } from '../../../apps/web/scripts/playtest-multiplayer-smoke.mjs';
import { createNavigator } from '../../../apps/web/scripts/quest-agent/protocol-play.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath, protocolMapCellIsWalkable } from '../../../apps/web/scripts/quest-agent/protocol-navigation.mjs';

export const MAP_METADATA_SHA256 = '907db052030a97036e404429373dd3ad3e0e838e279613080e92c3be28bf3ff2';
export const NPC_METADATA_SHA256 = 'dcc31e181b1f329c6da3658846ff18adb13731b18d899f8914b7628d54074105';
const ENDPOINT = 'wss://165.154.65.136.sslip.io/playtest/ws', WEB_BASE = 'https://numeron.obelisk.build';
const HASH = /^[0-9a-f]{64}$/, pointKey = p => `${p.x},${p.y}`;
const WALK_VECTORS = { up: [0, -1], upright: [1, -1], right: [1, 0], downright: [1, 1],
  down: [0, 1], downleft: [-1, 1], left: [-1, 0], upleft: [-1, -1] };
const self = c => c.snapshot?.entities?.find(e => e.objectId === c.snapshot.playerObjectId);
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const need = (ok, code) => { if (!ok) throw Object.assign(new Error(code), { code }); };
export const canonical = value => JSON.stringify(value, (_, v) => v && !Array.isArray(v) && typeof v === 'object'
  ? Object.fromEntries(Object.keys(v).sort().map(k => [k, v[k]])) : v);

// Exactly the existing public-pool preflight projection; only its aggregate hash is public.
export function poolFingerprints(pool) {
  const identities = pool.actors.map(a => ({ ordinal: a.ordinal, name: a.name,
    walletIdentity: Object.fromEntries(['kind', 'secretKey', 'address', 'accountId', 'characterIndex'].map(k => [k, a.walletIdentity[k]])),
    retainedLegacyIdentity: Object.fromEntries(['accountId', 'cookie', 'characterIndex', 'identityHistory']
      .filter(k => Object.hasOwn(a, k)).map(k => [k, a[k]])) }));
  return { identityFingerprintSha256: hash(canonical(identities)), canonicalActorArraySha256: hash(canonical(pool.actors)) };
}

export function validateExistingPool(pool, options) {
  need(pool?.schema === 'mir2.cloud-reference-private-actors.v2' && pool.endpoint === options.endpoint &&
    pool.webBase === options.webBase && Array.isArray(pool.actors) && pool.actors.length === 100, 'existing_100_pool_required');
  for (const [ordinal, a] of pool.actors.entries()) {
    const w = a?.walletIdentity;
    need(a?.ordinal === ordinal && typeof a.name === 'string' && /^[A-Za-z][A-Za-z0-9]{3,9}$/.test(a.name) &&
      w?.kind === 'synthetic-ed25519-wallet' && ['secretKey', 'address', 'accountId'].every(k => typeof w[k] === 'string' && w[k].length > 0) &&
      Number.isSafeInteger(w.characterIndex) && w.characterIndex >= 0, 'existing_nested_wallet_character_required');
  }
  for (const project of [a => a.name, a => a.walletIdentity.address, a => a.walletIdentity.accountId])
    need(new Set(pool.actors.map(project)).size === 100, 'duplicate_existing_identity');
  const fingerprints = poolFingerprints(pool);
  need(fingerprints.identityFingerprintSha256 === options.poolIdentitySha256 &&
    fingerprints.canonicalActorArraySha256 === options.poolActorsSha256, 'existing_pool_fingerprint_changed');
  // Explicit selection is the existing no-auto-create guard, not a pool migration.
  return { actors: selectProbeActors(pool, 100, pool.actors.map(a => a.ordinal)).map(s => s.actor), fingerprints };
}

export function validateLedger(ledger, options) {
  need(ledger?.schemaVersion === 1 && ledger.purpose === 'cloud-reference-public-auth-budget' &&
    ledger.endpoint === options.endpoint && ledger.webBase === options.webBase && Array.isArray(ledger.attempts) &&
    ledger.attempts.every(a => Number.isFinite(a.at) && ['admission', 'verification'].includes(a.kind)), 'existing_auth_ledger_required');
  return new LoginBudget(ledger.attempts, 0); // Existing 25+5 / total30 / 901000ms policy, including failed attempts.
}

export function validateMapProof(proof) {
  need(proof?.schema === 'mir2.scene-live-map-proof.v6' && proof.mapsComplete === true && proof.readOnly === true &&
    /^[0-9a-f]{40}$/.test(proof.sourceRevision) && HASH.test(proof.binarySha256) &&
    canonical(proof.identityBefore) === canonical(proof.identityAfter), 'matched_live_map_proof_required');
  const identity = proof.identityBefore;
  need(Number.isSafeInteger(identity?.pid) && identity.pid > 1 && Number.isSafeInteger(identity.startTicks) && identity.startTicks > 0 &&
    /^[0-9a-f]{32}$/.test(identity.invocationId) && Array.isArray(proof.files), 'live_map_identity_required');
  const rows = proof.files.filter(f => f && typeof f.path === 'string' && path.posix.basename(f.path) === '0.map.gz');
  need(rows.length === 1 && HASH.test(rows[0].sha256) && Number.isSafeInteger(rows[0].bytes) && rows[0].bytes > 0, 'unique_live_map0_required');
  return { identity, row: rows[0], sourceRevision: proof.sourceRevision, binarySha256: proof.binarySha256 };
}

export function assertMonitorIdentity(sample, binding) {
  need(sample?.targetProcess?.pid === binding.identity.pid && sample.targetProcess.startTicks === binding.identity.startTicks &&
    sample.targetUnit?.MainPID === binding.identity.pid && sample.health?.revision === binding.sourceRevision,
  'live_map_proof_and_monitor_identity_mismatch');
}

function liveTransferCells(snapshot, map) {
  const cells = [];
  for (const t of snapshot.mapTransfers ?? []) {
    const b = t?.bounds; need(b && ['minX', 'maxX', 'minY', 'maxY'].every(k => Number.isInteger(b[k])) &&
      b.minX >= 0 && b.minY >= 0 && b.maxX >= b.minX && b.maxY >= b.minY && b.maxX < map.width && b.maxY < map.height,
    'malformed_live_transfer_bounds');
    need(cells.length + (b.maxX - b.minX + 1) * (b.maxY - b.minY + 1) <= 100000, 'bounded_live_transfer_cells_required');
    for (let y = b.minY; y <= b.maxY; y++) for (let x = b.minX; x <= b.maxX; x++) cells.push({ x, y });
  }
  return cells;
}

export function buildLayout(map, row, npcRows, safeZoneIndex = 0) {
  need(row?.map_file_name === '0' && row.map_index === 1 && Array.isArray(row.safe_zones) &&
    Array.isArray(row.movements) && Array.isArray(npcRows), 'map0_metadata_required');
  const zone = row.safe_zones[safeZoneIndex];
  need(Number.isInteger(zone?.location?.x) && Number.isInteger(zone.location.y) && Number.isInteger(zone.size) && zone.size >= 4,
    'native_safe_zone_required');
  const hazards = [], targetOnlyHazards = [];
  const exclude = (p, radius, destination = hazards) => {
    need(Number.isInteger(p?.x) && Number.isInteger(p.y), 'malformed_map_hazard');
    for (let y = p.y - radius; y <= p.y + radius; y++) for (let x = p.x - radius; x <= p.x + radius; x++) destination.push({ x, y });
  };
  for (const npc of npcRows.filter(n => n.map_file_name === '0')) exclude(npc.location, 1);
  for (const movement of row.movements) exclude(movement.source, 1);
  for (const door of map.doors ?? []) {
    // Final destinations retain the V1 one-cell door clearance. Transit retains
    // the real closed-door collision/cell, without enclosing legal saved origins.
    exclude(door, 1, targetOnlyHazards); exclude(door, 0);
  }
  const forbidden = new Set([...hazards, ...targetOnlyHazards].map(pointKey)), cells = [];
  // Stay two cells inside the native safe square, with one globally unique destination per existing actor.
  for (let y = zone.location.y - zone.size + 2; y <= zone.location.y + zone.size - 2; y++)
    for (let x = zone.location.x - zone.size + 2; x <= zone.location.x + zone.size - 2; x++) {
      const p = { x, y };
      if (protocolMapCellIsWalkable(map, p) && !forbidden.has(pointKey(p))) cells.push(p);
    }
  cells.sort((a, b) => distance(a, zone.location) - distance(b, zone.location) || a.y - b.y || a.x - b.x);
  const used = new Set(), targets = [];
  for (let group = 0; group < 20; group++) {
    let cluster;
    for (const anchor of cells) {
      const near = cells.filter(p => !used.has(pointKey(p)) && distance(p, anchor) <= 2)
        .sort((a, b) => distance(a, anchor) - distance(b, anchor) || a.y - b.y || a.x - b.x);
      if (near.length >= 5) { cluster = near.slice(0, 5); break; }
    }
    need(cluster, 'insufficient_unique_compact_safe_targets');
    for (const [i, target] of cluster.entries()) { used.add(pointKey(target)); targets.push({ actorOrdinal: group * 5 + i, group, target }); }
  }
  return { safeZoneIndex, zone: { location: zone.location, size: zone.size }, targets, hazards, goalCells: cells };
}

export function validateTargetLayout(layout) {
  const goalCells = new Set(layout.goalCells.map(pointKey));
  need(layout.targets.length === 100 && layout.targets.every((t, i) => t.actorOrdinal === i &&
    t.group === Math.floor(i / 5) && goalCells.has(pointKey(t.target))) &&
    new Set(layout.targets.map(t => pointKey(t.target))).size === 100, 'invalid_unique_safe_target_layout');
  for (let group = 0; group < 20; group++) {
    const members = layout.targets.filter(t => t.group === group);
    need(members.every(a => members.every(b => distance(a.target, b.target) <= 12)), 'target_group_not_compact');
  }
  return true;
}

export function createTargetSelectionState(layout) {
  validateTargetLayout(layout);
  return { originalTargets: layout.targets.map(t => ({ ...t, target: { ...t.target } })),
    reserved: new Set(layout.targets.map(t => pointKey(t.target))), actual: new Map(), margins: new Set(), selections: [] };
}

export function selectPreparationTarget(c, target, layout, state, context) {
  context.assert();
  need(!c.pendingMovementAck && !state.actual.has(c.ordinal) &&
    !(c.events ?? []).some(e => e.direction === 'sent' && ['walk', 'run'].includes(e.type)),
  'target_selection_requires_no_pending_or_prior_walk');
  const owner = livingOwner(c), original = state.originalTargets[c.ordinal]?.target;
  const snapshotSequence = [...(c.events ?? [])].reverse().find(e => e.type === 'worldSnapshot')?.sequence ??
    c.bootstrapProof?.snapshotSequence;
  need(target.actorOrdinal === c.ordinal && Number.isSafeInteger(c.sequence) &&
    Number.isSafeInteger(snapshotSequence) && snapshotSequence > 0 && snapshotSequence <= c.sequence && original,
  'target_selection_evidence_cursor_required');
  const entities = c.snapshot.entities.filter(e => e.objectId !== owner.objectId && !e.dead);
  need(entities.length <= 4096 && entities.every(e => Number.isInteger(e.objectId) &&
    Number.isInteger(e.x) && Number.isInteger(e.y)), 'bounded_target_obstacles_required');
  const occupied = entities.map(e => ({ objectId: e.objectId, kind: String(e.kind ?? 'unknown').slice(0, 32),
    x: e.x, y: e.y, dead: false }));
  const transfers = liveTransferCells(c.snapshot, context.map);
  const obstacles = [...occupied, ...layout.hazards, ...transfers], blocked = new Set(obstacles.map(pointKey));
  const goals = new Set(layout.goalCells.map(pointKey)), actual = new Set([...state.actual.values()].map(pointKey));
  const peers = layout.targets.filter(t => t.group === target.group && t.actorOrdinal !== c.ordinal).map(t => t.target);
  let pathQueries = 0;
  const route = to => { pathQueries++; return findProtocolWalkPath({ map: context.map, start: owner, target: to,
    dynamicObstacles: obstacles, maxExpanded: 100000 }); };
  const originalOccupants = occupied.filter(e => e.x === original.x && e.y === original.y);
  const originalPath = goals.has(pointKey(original)) && !blocked.has(pointKey(original)) ? route(original) : null;
  const reason = originalPath ? 'planned_target_free_and_reachable' : originalOccupants.length ?
    'planned_target_occupied' : 'planned_target_unreachable';
  let selected = originalPath ? { point: { ...original }, path: originalPath, margin: null } : null;
  let candidatesConsidered = 0;
  if (!selected) {
    // Four peers bound this intersection to at most 25x25 cells. Goal cells
    // already retain the native safe inset, door moat, NPC and transfer masks.
    const candidates = layout.goalCells.filter(p => !state.reserved.has(pointKey(p)) && !actual.has(pointKey(p)) && !state.margins.has(pointKey(p)) &&
      !blocked.has(pointKey(p)) && protocolMapCellIsWalkable(context.map, p) && peers.every(q => distance(p, q) <= 12))
      .sort((a, b) => distance(a, original) - distance(b, original) || distance(a, owner) - distance(b, owner) || a.y - b.y || a.x - b.x);
    need(candidates.length <= 625, 'bounded_dynamic_target_candidates_required');
    for (const candidate of candidates) {
      const margin = [{ x: candidate.x, y: candidate.y - 1 }, { x: candidate.x + 1, y: candidate.y },
        { x: candidate.x, y: candidate.y + 1 }, { x: candidate.x - 1, y: candidate.y }].find(p =>
        goals.has(pointKey(p)) && !state.reserved.has(pointKey(p)) && !actual.has(pointKey(p)) &&
        !blocked.has(pointKey(p)) && protocolMapCellIsWalkable(context.map, p) && peers.every(q => distance(p, q) <= 12));
      if (!margin) continue;
      if (pathQueries >= 64) break;
      candidatesConsidered++;
      const path = route(candidate);
      if (path && path.length - 1 <= 1000) { selected = { point: { ...candidate }, path, margin }; break; }
    }
  }
  const evidence = { kind: 'preparationTargetSelection', actorOrdinal: c.ordinal, connection: c.connection,
    atUtc: new Date().toISOString(), monotonicMs: performance.now(), evidenceCursor: c.sequence, snapshotSequence,
    mapFileName: '0', owner: { objectId: owner.objectId, x: owner.x, y: owner.y, hp: c.snapshot.playerHp },
    originalTarget: { ...original }, actualTarget: selected?.point ?? null, reason, originalOccupants,
    occupied, liveTransferCells: transfers, candidatesConsidered, pathQueries, maximumPathQueries: 64,
    reservedTargetCount: state.reserved.size, priorActualTargetCount: state.actual.size, reservedMovementMarginCount: state.margins.size,
    selected: Boolean(selected), changed: Boolean(selected && pointKey(selected.point) !== pointKey(original)),
    movementMargin: selected?.margin ?? null, pathSteps: selected ? selected.path.length - 1 : null,
    pathSha256: selected ? hash(canonical(selected.path)) : null };
  if (selected) validateTargetLayout({ ...layout, targets: layout.targets.map(t => t === target ? { ...t, target: selected.point } : t) });
  context.emit(evidence); state.selections.push(evidence);
  need(selected, 'no_legal_reachable_preparation_target');
  target.target = selected.point; state.actual.set(c.ordinal, selected.point);
  if (selected.margin) state.margins.add(pointKey(selected.margin));
  return selected.point;
}

function livingOwner(c) {
  const p = self(c);
  need(c.inGame && !c.closed && p?.name === c.actor.name && !p.dead && c.snapshot.playerHp > 0 &&
    String(c.snapshot.mapFileName) === '0' && Number.isInteger(p.x) && Number.isInteger(p.y), 'living_map0_owner_required');
  return p;
}

export async function freshSafeState(c, target, context) {
  context.assert(); const after = c.sequence;
  await c.pace(); c.send({ type: 'clientVersion' });
  const receipt = await c.wait(() => c.events.find(e => e.sequence > after && e.direction === 'received' && e.type === 'worldSnapshot'),
    'fresh preparation safe-zone snapshot', 15000);
  const p = livingOwner(c);
  need(c.snapshot.inSafeZone === true && p.x === target.x && p.y === target.y, 'fresh_safe_target_not_observed');
  return { actorOrdinal: c.ordinal, connection: c.connection, snapshotSequence: receipt.sequence,
    snapshotAtMs: receipt.monotonicMs, mapFileName: '0', x: p.x, y: p.y, hp: c.snapshot.playerHp, inSafeZone: true };
}

export async function walkToTarget(c, target, context, map, hazards, { navigatorFactory = createNavigator } = {}) {
  let successfulSteps = 0;
  const navigator = navigatorFactory(c, { loadCollisionMap: async id => { need(String(id) === '0', 'unexpected_map_transfer'); return map; },
    movementResponseTimeoutMs: 2500 });
  for (;;) {
    context.assert(); const observed = livingOwner(c);
    if (observed.x === target.x && observed.y === target.y) return successfulSteps;
    need(successfulSteps < 1000, 'finite_walk_step_budget_exhausted');
    // Pacing can receive a changed obstacle field. Plan from the authority that
    // exists after waiting, rather than carry an earlier diagonal into it.
    await c.pace(); context.assert();
    const from = { ...livingOwner(c) };
    if (from.x === target.x && from.y === target.y) return successfulSteps;
    const occupied = c.snapshot.entities.filter(e => e.objectId !== from.objectId && !e.dead);
    const transfers = liveTransferCells(c.snapshot, map);
    const forbidden = [...hazards, ...transfers];
    const route = findProtocolWalkPath({ map, start: from, target, dynamicObstacles: [...occupied, ...forbidden], maxExpanded: 100000 });
    need(route?.length > 1, 'no_ordinary_safe_walk_route');
    const next = route[1], after = c.sequence; // Existing path includes its origin.
    context.emit({ kind: 'preparationWalkPlan', actorOrdinal: c.ordinal, connection: c.connection,
      evidenceCursor: after, atUtc: new Date().toISOString(), monotonicMs: performance.now(),
      from: { x: from.x, y: from.y }, plannedTarget: next, finalTarget: { ...target } });
    // A one-cell target plus a one-cell successful-step cap prevents Navigator's automatic Run.
    let result, navigationOutcome = 'navigator_reached';
    try {
      result = await navigator(next, 0, () => Date.now() >= context.deadline,
        { autoUseSupplies: false, maxEmergencyEscapesPerNavigation: 0, maxSuccessfulSteps: 1, maxAttempts: 2, maxNoPathRefreshes: 0,
          forbiddenPoints: forbidden, beforeMovement() { context.assert(); livingOwner(c); return null; } });
    } catch (error) {
      // A changed obstacle can make the navigator's one legal step a detour.
      // This exception alone proves nothing: the exact sent Walk and its
      // timely matching owner ACK must independently close below.
      if (error instanceof Error && error.message === 'Navigation successful step budget exceeded (1)' && error.successfulSteps === 1)
        navigationOutcome = 'confirmed_single_walk_budget';
      else throw error;
    }
    if (navigationOutcome === 'navigator_reached') need(result?.reached === true, 'ordinary_walk_not_reached');
    const sent = c.events.filter(e => e.sequence > after && e.direction === 'sent' && ['walk', 'run'].includes(e.type));
    need(sent.length === 1 && sent[0].type === 'walk' && Number.isFinite(sent[0].monotonicMs), 'strict_single_walk_required');
    const direction = typeof sent[0].movementDirection === 'string' ? sent[0].movementDirection.replace(/[+_\s-]/g, '').toLowerCase() : '';
    need(Object.hasOwn(WALK_VECTORS, direction), 'ordinary_walk_direction_required'); const vector = WALK_VECTORS[direction];
    const actualNext = { x: from.x + vector[0], y: from.y + vector[1] };
    need(protocolMapCellIsWalkable(map, actualNext) && !forbidden.some(p => pointKey(p) === pointKey(actualNext)), 'ordinary_walk_left_legal_route');
    if (navigationOutcome === 'navigator_reached') need(pointKey(actualNext) === pointKey(next), 'navigator_reached_wrong_adjacent_target');
    const deadline = sent[0].monotonicMs + 2500;
    const ack = await c.wait(() => c.events.find(e => e.sequence > sent[0].sequence && e.direction === 'received' &&
      e.packet === 'UserLocation' && e.movementRequestSequence === sent[0].sequence && e.monotonicMs <= deadline &&
      (e.payload?.location?.x ?? e.payload?.x) === actualNext.x && (e.payload?.location?.y ?? e.payload?.y) === actualNext.y),
    'exact preparation one-step owner ACK', Math.max(1, deadline - performance.now()));
    const current = livingOwner(c);
    need(!c.pendingMovementAck && current.x === actualNext.x && current.y === actualNext.y, 'authoritative_walk_target_changed');
    context.emit({ kind: 'preparationWalk', actorOrdinal: c.ordinal, connection: c.connection, from: { x: from.x, y: from.y },
      plannedTarget: next, target: actualNext, requestDirection: sent[0].movementDirection, navigationOutcome,
      sendSequence: sent[0].sequence, ownerSequence: ack.sequence, sentAtMs: sent[0].monotonicMs,
      deadlineAtMs: deadline, ownerAtMs: ack.monotonicMs });
    successfulSteps++;
  }
}

export async function prepareBatches(actors, layout, context, report,
  { Client = SignedClient, loginActor = login, walk = walkToTarget, fresh = freshSafeState } = {}) {
  const targetState = createTargetSelectionState(layout);
  report.originalTargets = targetState.originalTargets; report.targetSelections = targetState.selections;
  for (let group = 0; group < 20; group++) {
    context.assert(); const batch = []; let failure;
    try {
      for (const target of layout.targets.filter(t => t.group === group)) {
        context.assert(); const actor = actors[target.actorOrdinal];
        need(actor.walletIdentity && Number.isSafeInteger(actor.walletIdentity.characterIndex), 'existing_nested_wallet_character_required');
        const c = new Client(context.endpoint, actor, actor.ordinal, context); batch.push(c);
        // No next admission until this actor is alive at its verified native safe destination.
        await loginActor(c, context, 'admission');
        selectPreparationTarget(c, target, layout, targetState, context);
        const steps = await walk(c, target.target, context, context.map, layout.hazards);
        const evidence = await fresh(c, target.target, context);
        report.prepared.push({ ...evidence, group, successfulWalkSteps: steps });
        context.assertPoolUnchanged();
      }
      for (const c of batch) await c.wait(() => {
        livingOwner(c);
        return batch.filter(p => p !== c).every(p => {
          const owner = livingOwner(p), observed = c.snapshot.entities.find(e => e.objectId === observedPlayerId(c, p.label));
          return distance(self(c), owner) <= 12 && observed && !observed.dead && observed.x === owner.x && observed.y === owner.y;
        });
      }, 'five existing actors actually mutually visible', 5000);
      for (const speaker of batch) need(new Set(batch.filter(c => c !== speaker).map(c => observedPlayerId(c, speaker.label))).size === 1,
        'group_shared_identity_disagreement');
      // The early members may have moved/died while later members joined; recheck all fresh positions before saving.
      for (const c of batch) await fresh(c, layout.targets[c.ordinal].target, context);
      report.groups.push({ group, actorOrdinals: batch.map(c => c.ordinal), actualMutualAoi: true, maximumDistance: Math.max(...batch.flatMap(a => batch.map(b => distance(self(a), self(b))))) });
    } catch (error) { failure = error; }
    finally {
      for (const c of batch) {
        const initiallyInGame = c.inGame;
        try {
          await c.close();
          report.cleanup.push({ actorOrdinal: c.ordinal, connection: c.connection, initiallyInGame,
            normalLogout: !c.inGame && c.closed && (!initiallyInGame || c.logoutReceipt?.packet === 'LogOutSuccess' && c.logoutReceipt.elapsedMs <= 5000),
            logoutPacket: c.logoutReceipt?.packet ?? null, logoutElapsedMs: c.logoutReceipt?.elapsedMs ?? null });
        }
        catch { report.cleanup.push({ actorOrdinal: c.ordinal, normalLogout: false }); failure ??= Object.assign(new Error('normal_close_failed'), { code: 'normal_close_failed' }); }
      }
    }
    if (failure) throw failure;
    need(report.cleanup.slice(-5).every(r => r.normalLogout), 'normal_close_failed');
  }
}

export function optionsFrom(args) {
  const allowed = new Set(['private-directory', 'output', 'monitor', 'web-base', 'endpoint', 'wall-seconds', 'pool-identity-sha256',
    'pool-actors-sha256', 'map-root', 'live-map-proof', 'live-map-proof-sha256', 'map-metadata', 'map-metadata-sha256',
    'npc-metadata', 'npc-metadata-sha256', 'safe-zone-index']);
  const o = {};
  for (let i = 0; i < args.length; i += 2) { const key = args[i]?.slice(2);
    need(args[i]?.startsWith('--') && args[i + 1] !== undefined && allowed.has(key) && !Object.hasOwn(o, key), 'unknown_or_repeated_option'); o[key] = args[i + 1]; }
  for (const key of allowed) if (!['wall-seconds', 'safe-zone-index'].includes(key)) need(o[key], 'required_option_missing');
  need(validateEndpoint(o.endpoint) === ENDPOINT && o['web-base'] === WEB_BASE, 'original_public_endpoint_required');
  for (const key of ['pool-identity-sha256', 'pool-actors-sha256', 'live-map-proof-sha256']) need(HASH.test(o[key]), 'explicit_hash_required');
  need(o['map-metadata-sha256'] === MAP_METADATA_SHA256 && o['npc-metadata-sha256'] === NPC_METADATA_SHA256, 'reviewed_82_metadata_pins_required');
  for (const key of ['private-directory', 'output', 'monitor', 'map-root', 'live-map-proof', 'map-metadata', 'npc-metadata']) need(path.isAbsolute(o[key]), 'absolute_path_required');
  const wallSeconds = Number(o['wall-seconds'] ?? 7200), safeZoneIndex = Number(o['safe-zone-index'] ?? 0);
  need(Number.isInteger(wallSeconds) && wallSeconds >= 120 && wallSeconds <= 14400 && Number.isInteger(safeZoneIndex) && safeZoneIndex >= 0 && safeZoneIndex < 6, 'finite_preparation_bounds_required');
  return { ...o, endpoint: ENDPOINT, webBase: WEB_BASE, wallSeconds, safeZoneIndex,
    privateDirectory: outsideRepository(o['private-directory']), output: outsideRepository(o.output),
    poolIdentitySha256: o['pool-identity-sha256'], poolActorsSha256: o['pool-actors-sha256'] };
}

async function readBounded(file, maxBytes, digest) {
  const st = await fs.stat(file); need(st.isFile() && st.size > 0 && st.size <= maxBytes, 'bounded_input_required');
  const bytes = await fs.readFile(file); need(bytes.length <= maxBytes && (!digest || hash(bytes) === digest), 'input_hash_changed'); return bytes;
}

export async function runPreparation(options) {
  // Public pinned data is checked before reading any existing private identities or opening a socket.
  const proof = JSON.parse(await readBounded(options['live-map-proof'], 4 * 1024 * 1024, options['live-map-proof-sha256']));
  const binding = validateMapProof(proof);
  const metadata = JSON.parse(await readBounded(options['map-metadata'], 32 * 1024 * 1024, MAP_METADATA_SHA256));
  const npcs = JSON.parse(await readBounded(options['npc-metadata'], 8 * 1024 * 1024, NPC_METADATA_SHA256));
  const mapFile = path.join(options['map-root'], '0.map.gz');
  const mapBytes = await readBounded(mapFile, 32 * 1024 * 1024, binding.row.sha256);
  need(mapBytes.length === binding.row.bytes, 'live_map_bytes_changed');
  const map = await loadProtocolCollisionMap('0', { mapRoot: options['map-root'], packagedMapRoot: options['map-root'], clientRoot: options['map-root'] });
  need(path.resolve(map.sourcePath) === path.resolve(mapFile), 'unreviewed_map_fallback');
  const row = metadata.maps?.find(m => m.map_file_name === '0');
  const layout = buildLayout(map, row, npcs.npcs, options.safeZoneIndex);
  need(options.privateDirectory !== options.output, 'private_and_public_paths_must_differ');
  await fs.realpath(options.privateDirectory); // Must already exist; never create a pool or ledger.
  const lockFile = path.join(options.privateDirectory, '.cloud-reference.lock'), lock = await fs.open(lockFile, 'wx', 0o600);
  const runId = Date.now() + '-' + crypto.randomBytes(3).toString('hex');
  let writer, guard, timer, report, stop;
  try {
    const actorsFile = path.join(options.privateDirectory, 'actors.private.json'), ledgerFile = path.join(options.privateDirectory, 'auth-ledger.json');
    const pool = JSON.parse(await readBounded(actorsFile, 2 * 1024 * 1024));
    const checked = validateExistingPool(pool, options), ledger = JSON.parse(await readBounded(ledgerFile, 2 * 1024 * 1024));
    const loginBudget = validateLedger(ledger, options);
    await secureDirectory(options.privateDirectory); await fs.mkdir(options.output, { recursive: true });
    writer = new BoundedEvidence(path.join(options.output, runId + '.events.jsonl'));
    guard = new CapacityMonitor(options.monitor, writer);
    report = { schema: 'mir2.public-existing-cohort-preparation.v1', runId, startedAtUtc: new Date().toISOString(),
      preparationOnly: true, capacityEvidence: false, ccuQualified: false, actorsRequested: 100, maximumOwnedConcurrent: 5,
      poolBefore: checked.fingerprints, priorAuthAttemptCount: ledger.attempts.length, mapFileName: '0', safeZoneIndex: layout.safeZoneIndex,
      mapMetadataSha256: MAP_METADATA_SHA256, npcMetadataSha256: NPC_METADATA_SHA256,
      liveMapProofSha256: options['live-map-proof-sha256'], storedMapSha256: binding.row.sha256,
      liveMapIdentity: binding.identity, sourceRevision: binding.sourceRevision, binarySha256FromProof: binding.binarySha256,
      prepared: [], groups: [], cleanup: [], ok: false };
    const context = { endpoint: options.endpoint, webBase: options.webBase, pool, actorsFile, ledgerFile, loginBudget, map,
      connection: 0, guard, deadline: Date.now() + options.wallSeconds * 1000, transportForActor: i => probeTransportForActor('mixed', i),
      admissionTransportRecovery: [], firstAttemptAdmissionOk: true,
      secrets: pool.actors.flatMap(a => [a.cookie, a.accountId, a.walletIdentity.secretKey, a.walletIdentity.address, a.walletIdentity.accountId].filter(Boolean)),
      assert() { guard.assert(); assertMonitorIdentity(guard.latest, binding); need(Date.now() < this.deadline, 'finite_preparation_wall_exhausted'); },
      emit(value) { need(writer.rows < 1000000 && !writer.failure, 'preparation_capture_failed'); writer.add(value); },
      assertPoolUnchanged() { validateExistingPool(pool, options); } };
    stop = () => guard.stop('Operator requested ordinary preparation drain');
    process.on('SIGINT', stop); process.on('SIGTERM', stop);
    try {
      await guard.start(); context.assert();
      need(['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases', 'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight']
        .every(k => guard.latest.health?.[k] === 0), 'initial_target_not_empty');
      timer = setTimeout(() => guard.stop('Finite preparation wall exhausted'), options.wallSeconds * 1000);
      await prepareBatches(checked.actors, layout, context, report);
      context.assert(); validateTargetLayout(layout);
      report.ok = report.prepared.length === 100 && report.groups.length === 20 && report.cleanup.length === 100 && report.cleanup.every(r => r.normalLogout);
    } catch (error) {
      report.failure = { code: /^[a-z0-9_]+$/.test(error.code ?? '') ? error.code : 'preparation_failed',
        errorClass: ['Error', 'TypeError', 'RangeError', 'ProbeWaitDeadline', 'SafetyStop'].includes(error.name) ? error.name : 'ExternalError' };
    } finally {
      clearTimeout(timer); guard.close(); process.off('SIGINT', stop); process.off('SIGTERM', stop);
      try {
        report.poolAfter = validateExistingPool(JSON.parse(await readBounded(actorsFile, 2 * 1024 * 1024)), options).fingerprints;
        report.poolUnchanged = canonical(report.poolAfter) === canonical(report.poolBefore);
      } catch { report.poolUnchanged = false; }
      report.ok &&= report.poolUnchanged && !guard.stopped && !writer.failure;
      report.monitorStopped = guard.stopped; report.endedAtUtc = new Date().toISOString();
      const targetReceipt = { schema: 'mir2.public-existing-cohort-targets.v1', runId, preparationOnly: true, capacityEvidence: false,
        preparedAll: report.ok, poolIdentitySha256: checked.fingerprints.identityFingerprintSha256,
        liveMapProofSha256: options['live-map-proof-sha256'], mapMetadataSha256: MAP_METADATA_SHA256, storedMapSha256: binding.row.sha256,
        mapFileName: '0', nativeSafeZone: layout.zone, originalTargets: report.originalTargets ?? layout.targets,
        targets: layout.targets, targetSelections: report.targetSelections ?? [], preparedOrdinals: report.prepared.map(a => a.actorOrdinal),
        evidenceBoundary: 'fresh authoritative pre-logout positions plus normal Logout/Close; no private database readback; next load must verify its real landing positions' };
      await writer.close();
      const targetBytes = Buffer.from(JSON.stringify(targetReceipt, null, 2) + '\n');
      await fs.writeFile(path.join(options.output, runId + '.targets.json'), targetBytes, { flag: 'wx' });
      report.targetReceiptSha256 = hash(targetBytes);
      await fs.writeFile(path.join(options.output, runId + '.report.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
    }
    return report;
  } finally { if (guard) guard.close(); await lock.close(); await fs.unlink(lockFile); }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  Promise.resolve().then(() => runPreparation(optionsFrom(process.argv.slice(2)))).then(report => {
    console.log(JSON.stringify({ runId: report.runId, preparationOnly: true, capacityEvidence: false, ok: report.ok,
      preparedActors: report.prepared.length, groups: report.groups.length, normalCloses: report.cleanup.filter(r => r.normalLogout).length,
      targetReceiptSha256: report.targetReceiptSha256 })); process.exitCode = report.ok ? 0 : 1;
  }).catch(() => { console.error(JSON.stringify({ preparationOnly: true, capacityEvidence: false, ok: false, errorCode: 'preparation_preflight_or_evidence_failed' })); process.exitCode = 1; });
}
