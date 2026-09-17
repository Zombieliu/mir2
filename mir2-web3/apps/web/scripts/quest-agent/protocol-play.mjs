import { hasAuthoritativePlayerDeath, observedPlayerHp } from './protocol-observation.mjs';
import { loadProtocolCollisionMap, planProtocolNavigation } from './protocol-navigation.mjs';
import { delay } from './protocol-client.mjs';
import { useSupplies } from './protocol-loadout.mjs';
import { selfActionBlockMask } from './protocol-status.mjs';
import { safeZoneBlocksEmergencyEscape } from './protocol-survival.mjs';

export const selfPlayer = client => client.snapshot?.entities.find(e => e.objectId === client.snapshot.playerObjectId);
export const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
export class NavigationStalled extends Error {
  constructor({ reason, mapId, position, target, bestDistance, successfulSteps }) {
    super(`Navigation stalled (${reason}) on ${mapId} at ${position.x},${position.y} toward ${target.x},${target.y}`);
    this.name = 'NavigationStalled';
    this.code = 'NAVIGATION_STALLED';
    this.reason = reason;
    this.mapId = mapId;
    this.position = { x: Number(position.x), y: Number(position.y) };
    this.target = { x: Number(target.x), y: Number(target.y) };
    this.bestDistance = Number(bestDistance);
    this.successfulSteps = Number(successfulSteps);
  }
}
// A caller-scoped movement guard can stop a planned physical step after the
// cadence wait has supplied the latest AOI.  It deliberately carries no
// recovery behaviour: the owning combat loop decides whether the returned
// live blocker can be cleared or whether ordinary retreat is required.
export class NavigationStepGuarded extends Error {
  constructor(hazard) {
    super(`Navigation step guarded on ${String(hazard?.mapId ?? 'unknown')}`);
    this.name = 'NavigationStepGuarded';
    this.hazard = hazard ?? null;
  }
}
export async function reviveInTown(client) {
  const before = { map: client.snapshot.mapFileName, ...selfPlayer(client) };
  if (!hasAuthoritativePlayerDeath(client.snapshot)) return null;
  await client.request({ type: 'townRevive' }, 'Revived');
  const after = client.sequence;
  client.send({ type: 'clientVersion' });
  await client.wait(() => client.events.some(e => e.sequence > after && e.type === 'worldSnapshot') &&
    observedPlayerHp(client.snapshot) > 0 && !hasAuthoritativePlayerDeath(client.snapshot), 'town revival snapshot');
  return { at: new Date().toISOString(), before, after: { map: client.snapshot.mapFileName, ...selfPlayer(client) } };
}
const directionNames = { up: 'Up', 'up+right': 'UpRight', right: 'Right', 'down+right': 'DownRight', down: 'Down', 'down+left': 'DownLeft', left: 'Left', 'up+left': 'UpLeft' };

export async function equipStarterGear(client) {
  for (const [slot, to] of [['weapon', 0], ['armour', 1], ['torch', 3], ['necklace', 4], ['ringLeft', 7]]) {
    if (client.snapshot.equipmentItems.some(i => i.slot === slot || i.equipSlot === slot)) continue;
    const item = client.snapshot.inventoryItems.find(i => i.equipSlot === slot);
    if (!item) continue;
    const ack = await client.request({ type: 'equipItem', grid: 'inventory', uniqueId: item.uniqueId, to }, 'EquipItem');
    if (!ack.payload.success) throw new Error(`Equip rejected for ${item.name}`);
    await client.wait(() => client.snapshot.equipmentItems.some(i => i.uniqueId === item.uniqueId), `equipment ${item.name}`);
  }
}

export async function collectNearbyGold(client, navigateNear) {
  const self = selfPlayer(client);
  const drop = client.snapshot.groundDrops.find(d => d.loot?.kind === 'gold' && distance(self, d) <= 3);
  if (!drop) return;
  await navigateNear(drop, 1);
  const before = client.snapshot.gold;
  client.send({ type: 'pickUp', objectId: drop.objectId });
  try { await client.wait(() => client.snapshot.gold > before, 'nearby gold pickup', 1500); }
  catch (error) { if (client.failure || client.closed) throw error; }
}

function pointInBounds(point, bounds) {
  return Number(point?.x) >= Number(bounds?.minX) && Number(point?.x) <= Number(bounds?.maxX) &&
    Number(point?.y) >= Number(bounds?.minY) && Number(point?.y) <= Number(bounds?.maxY);
}

function liveTransferStaticWalkableOverrides(snapshot, target, options) {
  const key = options?.liveTransferKey;
  if (key == null) return [];
  if (snapshot?.mapSnapshotPending) {
    throw new Error('Cannot use a live transfer collision override while the map snapshot is pending');
  }
  const currentMap = String(snapshot?.mapFileName ?? '');
  const transfer = (snapshot?.mapTransfers ?? []).find(candidate =>
    String(candidate?.key ?? '') === String(key) &&
    String(candidate?.mapFileName ?? currentMap) === currentMap &&
    pointInBounds(target, candidate?.bounds)
  );
  if (!transfer) {
    throw new Error(`Live map transfer ${key} no longer authorizes target ${target?.x},${target?.y}`);
  }
  // Crystal often represents one visible doorway as several adjacent movement
  // records. Authorizing only the selected record can block the corridor that
  // leads to it, because the neighbouring records are then treated as hazards.
  // Every record with the same authoritative destination is safe for this hop:
  // entering any of them produces the map change the traveler is waiting for.
  const destinationMap = String(transfer?.toMapFileName ?? '');
  const overrides = [];
  for (const candidate of snapshot?.mapTransfers ?? []) {
    if (String(candidate?.mapFileName ?? currentMap) !== currentMap ||
        String(candidate?.toMapFileName ?? '') !== destinationMap) continue;
    const bounds = candidate?.bounds;
    const minX = Number(bounds?.minX);
    const maxX = Number(bounds?.maxX);
    const minY = Number(bounds?.minY);
    const maxY = Number(bounds?.maxY);
    if (![minX, maxX, minY, maxY].every(Number.isInteger) || minX > maxX || minY > maxY) continue;
    for (let y = minY; y <= maxY; y += 1) {
      for (let x = minX; x <= maxX; x += 1) overrides.push({ x, y });
    }
  }
  return overrides;
}

function liveTransferObstacles(snapshot, staticWalkableOverrides) {
  if (snapshot?.mapSnapshotPending) return [];
  const currentMap = String(snapshot?.mapFileName ?? '');
  const allowed = new Set(staticWalkableOverrides.map(point => `${point.x},${point.y}`));
  const obstacles = [];
  for (const transfer of snapshot?.mapTransfers ?? []) {
    if (String(transfer?.mapFileName ?? currentMap) !== currentMap) continue;
    const bounds = transfer?.bounds;
    const minX = Number(bounds?.minX);
    const maxX = Number(bounds?.maxX);
    const minY = Number(bounds?.minY);
    const maxY = Number(bounds?.maxY);
    if (![minX, maxX, minY, maxY].every(Number.isInteger) || minX > maxX || minY > maxY) continue;
    for (let y = minY; y <= maxY; y += 1) {
      for (let x = minX; x <= maxX; x += 1) {
        if (!allowed.has(`${x},${y}`)) obstacles.push({ x, y });
      }
    }
  }
  return obstacles;
}

function explicitForbiddenPoints(options) {
  return (options?.forbiddenPoints ?? [])
    .filter(point => Number.isFinite(Number(point?.x)) && Number.isFinite(Number(point?.y)))
    .map(point => ({ x: Number(point.x), y: Number(point.y) }));
}

function normalizedHostileName(value) {
  return String(value ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase();
}

function namedHostileClearance(options) {
  const requested = options?.hostileAvoidanceByName;
  if (!requested || typeof requested !== 'object' || Array.isArray(requested)) return new Map();
  const clearance = new Map();
  for (const [name, radius] of Object.entries(requested)) {
    const normalized = normalizedHostileName(name);
    if (!normalized) continue;
    clearance.set(normalized, nonnegativeIntegerOption(radius, 0));
  }
  return clearance;
}

function clearanceRadiusForHostile(entity, defaultRadius, namedClearance) {
  return Math.max(defaultRadius, namedClearance.get(normalizedHostileName(entity?.name)) ?? 0);
}

function hasRequestedHostileClearance(options) {
  return nonnegativeIntegerOption(options?.hostileAvoidanceRadius, 0) > 0 ||
    [...namedHostileClearance(options).values()].some(radius => radius > 0);
}

function hostileClearanceObstacles(snapshot, options, memory, observedAt, memoryDurationMs) {
  const defaultRadius = nonnegativeIntegerOption(options?.hostileAvoidanceRadius, 0);
  const namedClearance = namedHostileClearance(options);
  const allowed = new Set((options?.allowedHostileObjectIds ?? []).map(Number));
  const mapFileName = String(snapshot?.mapFileName ?? '');
  for (const [key, remembered] of memory) {
    if (remembered.expiresAt <= observedAt) memory.delete(key);
  }
  for (const entity of snapshot?.entities ?? []) {
    if (String(entity?.kind ?? '').toLowerCase() !== 'monster') continue;
    const objectId = Number(entity?.objectId);
    if (!Number.isFinite(objectId)) continue;
    const keyPrefix = `${mapFileName}:${objectId}:`;
    const disposition = String(entity?.disposition ?? '').toLowerCase();
    if ((disposition && disposition !== 'hostile') || entity?.dead === true ||
      (entity?.hp != null && Number(entity.hp) <= 0)) {
      for (const key of memory.keys()) {
        if (key.startsWith(keyPrefix)) memory.delete(key);
      }
      continue;
    }
    if (Number.isFinite(Number(entity?.x)) && Number.isFinite(Number(entity?.y))) {
      // Retain the recent path as well as the latest point. A pursuing monster
      // otherwise drags one moving clearance square around the player and can
      // make a shortest-path planner orbit forever at the edge of that square.
      // With no requested clearance, retain one last-known occupied tile per
      // monster instead of its whole movement trail. Exact-tile memory is for
      // AOI stability; turning a moving monster's trail into a wall would make
      // narrow maps falsely unreachable.
      if (clearanceRadiusForHostile(entity, defaultRadius, namedClearance) === 0) {
        for (const key of memory.keys()) {
          if (key.startsWith(keyPrefix)) memory.delete(key);
        }
      }
      const key = `${keyPrefix}${Number(entity.x)},${Number(entity.y)}`;
      memory.set(key, {
        mapFileName,
        objectId,
        x: Number(entity.x),
        y: Number(entity.y),
        name: normalizedHostileName(entity.name),
        expiresAt: observedAt + memoryDurationMs,
      });
    }
  }
  // Zero-clearance travel still remembers the monster's exact occupied tile.
  // AOI boundaries can otherwise alternate two stationary monsters in and out
  // of the snapshot and make A* choose opposite detours forever. Radius zero
  // keeps ordinary travel permissive while stabilising those exact obstacles.
  const obstacles = [];
  const player = (snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === Number(snapshot?.playerObjectId)) ?? null;
  for (const entity of memory.values()) {
    if (entity.mapFileName !== mapFileName || allowed.has(entity.objectId)) continue;
    const radius = Math.max(defaultRadius, namedClearance.get(entity.name) ?? 0);
    const playerDistance = player && Number.isFinite(Number(player?.x)) && Number.isFinite(Number(player?.y))
      ? Math.max(Math.abs(Number(player.x) - entity.x), Math.abs(Number(player.y) - entity.y))
      : Number.POSITIVE_INFINITY;
    for (let y = entity.y - radius; y <= entity.y + radius; y += 1) {
      for (let x = entity.x - radius; x <= entity.x + radius; x += 1) {
        // A clearance square is an entry barrier, not a cage. When a monster
        // has already moved inside the requested buffer, keep its own tile and
        // every closer tile blocked while allowing steps that maintain or grow
        // the player's distance. This lets ordinary movement disengage from a
        // pursuer instead of reporting every neighbouring tile unreachable.
        if (playerDistance <= radius &&
          Math.max(Math.abs(x - entity.x), Math.abs(y - entity.y)) >= playerDistance) continue;
        obstacles.push({ x, y });
      }
    }
  }
  return obstacles;
}

function currentMapReceiptBoundary(client, mapId) {
  let boundary = 0;
  let latestMapReceipt = null;
  for (const event of client?.events ?? []) {
    if (event?.direction !== 'received' ||
        !['MapChanged', 'MapInformation'].includes(String(event?.packet ?? ''))) continue;
    const sequence = Number(event?.sequence) || 0;
    if (!latestMapReceipt || sequence >= latestMapReceipt.sequence) {
      latestMapReceipt = {
        sequence,
        mapFileName: String(event?.payload?.fileName ?? event?.payload?.mapFileName ?? ''),
      };
    }
    if (String(event?.payload?.fileName ?? event?.payload?.mapFileName ?? '') === mapId) {
      boundary = Math.max(boundary, sequence);
    }
  }
  // Packet receipts have no map field. Do not allow a hit observed before an
  // intervening map transition to prove a present-map aggressor.
  if (latestMapReceipt && latestMapReceipt.mapFileName !== mapId) return Number.POSITIVE_INFINITY;
  return boundary;
}

function currentPlayerLifeReceiptBoundary(client, playerObjectId) {
  let boundary = 0;
  for (const event of client?.events ?? []) {
    if (event?.direction !== 'received') continue;
    const sequence = Number(event?.sequence) || 0;
    const packet = String(event?.packet ?? '');
    // Death and Revived are self-only packets. ObjectDied/ObjectRevived are
    // AOI packets, so only the current owner's objectId is a life boundary.
    // The protocol calls the self revive packet Revived; it has no payload.
    if (packet === 'Death' || packet === 'Revived' ||
        ((packet === 'ObjectDied' || packet === 'ObjectRevived') &&
          Number(event?.payload?.objectId) === playerObjectId)) {
      boundary = Math.max(boundary, sequence);
    }
  }
  return boundary;
}

function provenNearbyEmergencyAggressors(client, nearbyHostiles, mapId, nowMs, withinMs) {
  const playerObjectId = Number(client?.snapshot?.playerObjectId);
  if (!Number.isFinite(playerObjectId)) return [];
  const boundary = Math.max(
    currentMapReceiptBoundary(client, mapId),
    currentPlayerLifeReceiptBoundary(client, playerObjectId),
  );
  const liveNearbyIds = new Set(nearbyHostiles.map(entity => Number(entity?.objectId))
    .filter(Number.isFinite));
  if (liveNearbyIds.size === 0 || !Number.isFinite(nowMs)) return [];
  const attackers = new Set();
  for (const event of client?.events ?? []) {
    if (event?.direction !== 'received' || event?.packet !== 'ObjectStruck' ||
        (Number(event?.sequence) || 0) <= boundary ||
        Number(event?.payload?.objectId) !== playerObjectId) continue;
    const eventAt = Date.parse(String(event?.at ?? ''));
    if (!Number.isFinite(eventAt) || eventAt > nowMs || nowMs - eventAt > withinMs) continue;
    const attackerId = Number(event?.payload?.attackerId);
    if (liveNearbyIds.has(attackerId)) attackers.add(attackerId);
  }
  return nearbyHostiles.filter(entity => attackers.has(Number(entity?.objectId)));
}

function freshDirectMonsterHits(client, mapId, nowMs, withinMs, afterSequence) {
  const playerObjectId = Number(client?.snapshot?.playerObjectId);
  if (!Number.isFinite(playerObjectId) || !Number.isFinite(nowMs)) return [];
  const boundary = Math.max(
    Number(afterSequence) || 0,
    currentMapReceiptBoundary(client, mapId),
    currentPlayerLifeReceiptBoundary(client, playerObjectId),
  );
  const liveMonsters = new Map((client?.snapshot?.entities ?? [])
    .filter(entity => entity?.kind === 'monster' && entity?.dead !== true && Number(entity?.hp ?? 1) > 0)
    .map(entity => [Number(entity?.objectId), entity])
    .filter(([objectId]) => Number.isFinite(objectId)));
  const hits = [];
  for (const event of client?.events ?? []) {
    const sequence = Number(event?.sequence) || 0;
    if (event?.direction !== 'received' || event?.packet !== 'ObjectStruck' ||
        sequence <= boundary || Number(event?.payload?.objectId) !== playerObjectId) continue;
    const eventAt = Date.parse(String(event?.at ?? ''));
    if (!Number.isFinite(eventAt) || eventAt > nowMs || nowMs - eventAt > withinMs) continue;
    const attacker = liveMonsters.get(Number(event?.payload?.attackerId));
    if (attacker) hits.push({ event, attacker, sequence });
  }
  return hits;
}

const EMERGENCY_RELOCATION_MEMORY_RESET_DISTANCE = 24;

function activeOwnerTransform(snapshot) {
  if (snapshot?.mapSnapshotPending === true) return null;
  const objectId = Number(snapshot?.playerObjectId);
  if (!Number.isSafeInteger(objectId)) return null;
  const owner = (snapshot?.entities ?? []).find(entity => Number(entity?.objectId) === objectId);
  const rawMapFileName = snapshot?.mapFileName;
  const rawX = owner?.x;
  const rawY = owner?.y;
  const x = Number(rawX);
  const y = Number(rawY);
  if (!owner || owner.dead === true || rawMapFileName == null || String(rawMapFileName).trim() === '' ||
      rawX == null || rawY == null ||
      (typeof rawX === 'string' && rawX.trim() === '') ||
      (typeof rawY === 'string' && rawY.trim() === '') ||
      !Number.isFinite(x) || !Number.isFinite(y)) return null;
  return { mapFileName: String(rawMapFileName), objectId, x, y };
}

function sameOwnerTransform(left, right) {
  return left != null && right != null &&
    String(left.mapFileName) === String(right.mapFileName) &&
    Number(left.objectId) === Number(right.objectId) &&
    Number(left.x) === Number(right.x) && Number(left.y) === Number(right.y);
}

function sameMapOwner(left, right) {
  return left != null && right != null &&
    String(left.mapFileName) === String(right.mapFileName) &&
    Number(left.objectId) === Number(right.objectId);
}

function hasFiniteExternalTransform(transform) {
  const rawMapFileName = transform?.mapFileName;
  const rawObjectId = transform?.objectId;
  const rawX = transform?.x;
  const rawY = transform?.y;
  return rawMapFileName != null && String(rawMapFileName).trim() !== '' &&
    rawObjectId != null && Number.isSafeInteger(Number(rawObjectId)) &&
    rawX != null && rawY != null &&
    !(typeof rawX === 'string' && rawX.trim() === '') &&
    !(typeof rawY === 'string' && rawY.trim() === '') &&
    Number.isFinite(Number(rawX)) && Number.isFinite(Number(rawY));
}

export function createNavigator(client, dependencies = {}) {
  const loadCollisionMap = dependencies.loadCollisionMap ?? loadProtocolCollisionMap;
  const sleep = dependencies.delay ?? delay;
  const now = dependencies.now ?? Date.now;
  const defaultEmergencyEscape = typeof dependencies.emergencyEscape === 'function'
    ? dependencies.emergencyEscape
    : null;
  const defaultEmergencyEscapeHpRatio = ratioOption(dependencies.emergencyEscapeHpRatio, 0);
  const defaultEmergencyEscapeCriticalHpRatio = ratioOption(
    dependencies.emergencyEscapeCriticalHpRatio,
    0.25,
  );
  const defaultEmergencyEscapeDangerDistance = nonnegativeIntegerOption(
    dependencies.emergencyEscapeDangerDistance,
    3,
  );
  const defaultEmergencyEscapeRequiresProvenAggressors =
    dependencies.emergencyEscapeRequiresProvenAggressors === true;
  const defaultEmergencyEscapeAggressorEvidenceWindowMs = positiveIntegerOption(
    dependencies.emergencyEscapeAggressorEvidenceWindowMs,
    5_000,
  );
  const defaultMaxEmergencyEscapes = nonnegativeIntegerOption(
    dependencies.maxEmergencyEscapesPerNavigation,
    defaultEmergencyEscape ? 1 : 0,
  );
  const hostileMemoryDurationMs = positiveIntegerOption(dependencies.hostileMemoryDurationMs, 20_000);
  const exactHostileMemoryDurationMs = positiveIntegerOption(
    dependencies.exactHostileMemoryDurationMs,
    120_000,
  );
  // Shared-Zone movement is serialized behind the authoritative world writer.
  // A three-client acceptance run can therefore take several seconds to
  // return UserLocation even though the move is valid. Wait for the actual
  // owner response instead of turning a local timer expiry into collision.
  const movementResponseTimeoutMs = positiveIntegerOption(
    dependencies.movementResponseTimeoutMs,
    60_000,
  );
  const maps = new Map();
  const hostileMemory = new Map();
  let lastActiveOwner = null;
  const navigateNear = async function navigateNear(target, desiredDistance = 1, stopWhen = () => false, options = {}) {
    const mapId = client.snapshot.mapFileName;
    if (!maps.has(mapId)) maps.set(mapId, await loadCollisionMap(mapId));
    const maxSuccessfulSteps = positiveIntegerOption(options.maxSuccessfulSteps, 2500);
    const maxAttempts = positiveIntegerOption(options.maxAttempts, 2500);
    const maxNoPathRefreshes = nonnegativeIntegerOption(options.maxNoPathRefreshes, 3);
    const detectPositionCycles = options.detectPositionCycles === true;
    const maxNonImprovingSteps = options.maxNonImprovingSteps == null
      ? 0
      : positiveIntegerOption(options.maxNonImprovingSteps, 0);
    const emergencyEscape = typeof options.emergencyEscape === 'function'
      ? options.emergencyEscape
      : defaultEmergencyEscape;
    const emergencyEscapeHpRatio = ratioOption(
      options.emergencyEscapeHpRatio,
      defaultEmergencyEscapeHpRatio,
    );
    const emergencyEscapeCriticalHpRatio = ratioOption(
      options.emergencyEscapeCriticalHpRatio,
      defaultEmergencyEscapeCriticalHpRatio,
    );
    const emergencyEscapeDangerDistance = nonnegativeIntegerOption(
      options.emergencyEscapeDangerDistance,
      defaultEmergencyEscapeDangerDistance,
    );
    const emergencyEscapeRequiresProvenAggressors =
      options.emergencyEscapeRequiresProvenAggressors == null
        ? defaultEmergencyEscapeRequiresProvenAggressors
        : options.emergencyEscapeRequiresProvenAggressors === true;
    const emergencyEscapeAggressorEvidenceWindowMs = positiveIntegerOption(
      options.emergencyEscapeAggressorEvidenceWindowMs,
      defaultEmergencyEscapeAggressorEvidenceWindowMs,
    );
    const onFreshDirectHit = typeof options.onFreshDirectHit === 'function'
      ? options.onFreshDirectHit
      : null;
    const maxEmergencyEscapes = nonnegativeIntegerOption(
      options.maxEmergencyEscapesPerNavigation,
      defaultMaxEmergencyEscapes,
    );
    const rejected = [];
    let failures = 0;
    let successfulSteps = 0;
    let emergencyEscapes = 0;
    let emergencyEscapeFailed = false;
    let emergencyEscapeRetryAt = Number.NEGATIVE_INFINITY;
    let lastFreshDirectHitSequence = 0;
    let bestDistance = distance(selfPlayer(client), target);
    let nonImprovingSteps = 0;
    let lastMovementBlockMask = 0;
    let positionsSinceImprovement = new Set([`${selfPlayer(client).x},${selfPlayer(client).y}`]);
    let remaining = [];
    for (let count = 0; count < maxAttempts; count++) {
      if (client.failure) throw client.failure;
      if (client.closed) throw new Error('Protocol client closed during navigation');
      const self = selfPlayer(client);
      lastActiveOwner = activeOwnerTransform(client.snapshot) ?? lastActiveOwner;
      if (stopWhen()) return { reached: false, successfulSteps };
      if (hasAuthoritativePlayerDeath(client.snapshot)) throw new Error('Player died during navigation');
      const movementBlockMask = selfActionBlockMask(client);
      if (movementBlockMask !== 0) {
        remaining = [];
        if (movementBlockMask !== lastMovementBlockMask) {
          client.record('diagnostic', {
            type: 'navigationMovementControlBlocked',
            mapId,
            position: { x: Number(self.x), y: Number(self.y) },
            poison: Number(self.poison ?? client.snapshot?.playerPoison ?? 0),
            movementBlockMask,
          });
        }
        lastMovementBlockMask = movementBlockMask;
        await sleep(250);
        continue;
      }
      lastMovementBlockMask = 0;
      const safeZoneEscapeBlocked = safeZoneBlocksEmergencyEscape(client, { now });
      const inSafeZone = client.snapshot?.inSafeZone === true;
      const nearbyHostiles = (client.snapshot?.entities ?? []).filter(entity =>
        entity?.kind === 'monster' && entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
        !inSafeZone && entity?.disposition !== 'friendly' &&
        distance(self, entity) <= emergencyEscapeDangerDistance);
      const hpRatio = observedPlayerHp(client.snapshot) /
        Math.max(1, Number(client.snapshot.playerMaxHp));
      // A caller can opt into a nonblocking reaction to a real current-life,
      // current-map hit. The callback never supplies evidence to the escape
      // decision and is not awaited, so it cannot delay movement or weaken the
      // normal proven-aggressor and critical-health guards.
      if (onFreshDirectHit) {
        const freshHits = freshDirectMonsterHits(
          client,
          String(client.snapshot?.mapFileName ?? mapId),
          Number(now()),
          emergencyEscapeAggressorEvidenceWindowMs,
          lastFreshDirectHitSequence,
        );
        for (const hit of freshHits) {
          lastFreshDirectHitSequence = Math.max(lastFreshDirectHitSequence, hit.sequence);
          try {
            onFreshDirectHit(client, {
              attacker: hit.attacker,
              receipt: hit.event,
              hpRatio,
              mapFileName: String(client.snapshot?.mapFileName ?? mapId),
            });
          } catch (error) {
            client.record('diagnostic', {
              type: 'navigationFreshDirectHitRecoveryFailure',
              message: String(error?.message ?? error),
            });
          }
        }
      }
      const safeZoneAttackReceipt = inSafeZone && !safeZoneEscapeBlocked;
      const provenNearbyHostiles = emergencyEscapeRequiresProvenAggressors
        ? provenNearbyEmergencyAggressors(
          client,
          nearbyHostiles,
          String(client.snapshot?.mapFileName ?? mapId),
          Number(now()),
          emergencyEscapeAggressorEvidenceWindowMs,
        )
        : nearbyHostiles;
      const shouldEmergencyEscape = safeZoneAttackReceipt || provenNearbyHostiles.length >= 2 ||
        (provenNearbyHostiles.length > 0 && hpRatio <= emergencyEscapeCriticalHpRatio);
      if (emergencyEscape && !emergencyEscapeFailed && now() >= emergencyEscapeRetryAt &&
          emergencyEscapes < maxEmergencyEscapes &&
          hpRatio <= emergencyEscapeHpRatio && shouldEmergencyEscape) {
        const before = { mapFileName: mapId, x: Number(self.x), y: Number(self.y) };
        client.record('diagnostic', {
          type: 'navigationEmergencyEscapeAttempt',
          hpRatio,
          nearby: nearbyHostiles.length,
          ...(emergencyEscapeRequiresProvenAggressors
            ? { provenNearby: provenNearbyHostiles.length }
            : {}),
          safeZoneAttackReceipt,
          dangerDistance: emergencyEscapeDangerDistance,
          from: before,
        });
        try {
          const result = await emergencyEscape(client, { current: before, nearbyHostiles });
          if (result?.deferred === true) {
            emergencyEscapeRetryAt = now() + Math.max(1, Number(result.retryAfterMs) || 1);
            client.record('diagnostic', {
              type: 'navigationEmergencyEscapeDeferred',
              hpRatio,
              nearby: nearbyHostiles.length,
              from: before,
              retryAfterMs: Number(result.retryAfterMs ?? 0),
            });
          } else {
            const after = selfPlayer(client);
            const changed = after && (String(client.snapshot.mapFileName) !== mapId ||
              Number(after.x) !== before.x || Number(after.y) !== before.y);
            if (result !== false && changed) {
              emergencyEscapes += 1;
              failures = 0;
              rejected.length = 0;
              remaining = [];
              client.record('diagnostic', {
                type: 'navigationEmergencyEscapeSuccess',
                hpRatio: observedPlayerHp(client.snapshot) /
                  Math.max(1, Number(client.snapshot.playerMaxHp)),
                nearby: nearbyHostiles.length,
                safeZoneAttackReceipt,
                from: before,
                to: {
                  mapFileName: String(client.snapshot.mapFileName),
                  x: Number(after.x),
                  y: Number(after.y),
                },
              });
              if (client.snapshot.mapFileName !== mapId) return { reached: false, successfulSteps };
              continue;
            }
            emergencyEscapeFailed = true;
            client.record('diagnostic', {
              type: 'navigationEmergencyEscapeFailure',
              hpRatio,
              nearby: nearbyHostiles.length,
              from: before,
              message: 'Emergency escape did not change the authoritative player position',
            });
          }
        } catch (error) {
          emergencyEscapeFailed = true;
          client.record('diagnostic', {
            type: 'navigationEmergencyEscapeFailure',
            hpRatio,
            nearby: nearbyHostiles.length,
            from: before,
            message: String(error?.message ?? error),
          });
        }
      }
      if (options.autoUseSupplies !== false &&
          observedPlayerHp(client.snapshot) < client.snapshot.playerMaxHp * 0.6 &&
          now() - (client.lastTravelSupplyAt ?? 0) >= 2000) {
        client.lastTravelSupplyAt = now();
        // Route movement can need HP recovery, but spending MP while no spell
        // is being cast can strand a caster before the actual engagement.
        await useSupplies(client, { mpThreshold: 0 });
      }
      if (client.snapshot.mapFileName !== mapId) return { reached: false, successfulSteps };
      if (distance(self, target) <= desiredDistance) return { reached: true, successfulSteps };
      const staticWalkableOverrides = liveTransferStaticWalkableOverrides(client.snapshot, target, options);
      // Enter a transfer only when the caller selected its live authoritative
      // key. Ordinary same-map navigation treats every other transfer source
      // as a hazard; otherwise a route to an NPC/monster can accidentally cross
      // a doorway and change maps mid-action.
      const dynamicObstacles = [
        ...rejected,
        ...liveTransferObstacles(client.snapshot, staticWalkableOverrides),
        ...explicitForbiddenPoints(options),
        ...hostileClearanceObstacles(
          client.snapshot,
          options,
          hostileMemory,
          now(),
          !hasRequestedHostileClearance(options)
            ? exactHostileMemoryDurationMs
            : hostileMemoryDurationMs,
        ),
        ...client.snapshot.entities.filter(e => e.objectId !== self.objectId && !e.dead),
      ];
      const obstacleKeys = new Set(dynamicObstacles
        .filter(point => Number.isFinite(Number(point?.x)) && Number.isFinite(Number(point?.y)))
        .map(point => `${Number(point.x)},${Number(point.y)}`));
      // AOI reveals monsters while a route is already in flight. Replan as
      // soon as any newly visible danger intersects the remaining route,
      // rather than waiting until that danger occupies the very next tile.
      // Waiting one tile at a time lets an aggro pack close around a running
      // player before the old route is invalidated.
      const remainingRouteBlocked = remaining.some(candidate =>
        obstacleKeys.has(`${Number(candidate?.to?.x)},${Number(candidate?.to?.y)}`));
      if (!remaining.length || distance(remaining[0].from, self) !== 0 || remainingRouteBlocked) {
        remaining = planProtocolNavigation({ map: maps.get(mapId), start: self, target, desiredDistance, dynamicObstacles, staticWalkableOverrides })?.steps ?? [];
      }
      const step = remaining[0];
      if (!step) {
        if (++failures > maxNoPathRefreshes) throw new Error(`No walk path on ${mapId} from ${self.x},${self.y} to ${target.x},${target.y}`);
        client.record('diagnostic', { type: 'navigationReplan', mapId, mapSource: maps.get(mapId).sourcePath, width: maps.get(mapId).width, height: maps.get(mapId).height, desiredDistance, start: { x: self.x, y: self.y }, target, rejected: [...rejected], obstacleCount: dynamicObstacles.length });
        const after = client.sequence;
        client.send({ type: 'clientVersion' });
        await client.wait(() => client.events.some(e => e.sequence > after && e.type === 'worldSnapshot'), 'fresh navigation snapshot');
        rejected.length = 0;
        await sleep(750);
        continue;
      }
      const plannedFrom = { x: self.x, y: self.y };
      if (successfulSteps >= maxSuccessfulSteps) {
        throw new Error(`Navigation successful step budget exceeded (${maxSuccessfulSteps})`);
      }
      const running = maxSuccessfulSteps - successfulSteps >= 2 &&
        remaining.length >= 2 && remaining[1].direction === step.direction &&
        !obstacleKeys.has(`${Number(remaining[1].to.x)},${Number(remaining[1].to.y)}`);
      await sleep(Math.max(0, 650 - (now() - (client.lastWalkAt ?? 0))));
      // Control/death packets can arrive during cadence waiting without
      // changing position. Recheck before sending an otherwise valid step.
      if (hasAuthoritativePlayerDeath(client.snapshot)) {
        throw new Error('Player died during navigation');
      }
      if (selfActionBlockMask(client) !== 0) {
        remaining = [];
        continue;
      }
      // A fresh world update can expose a transfer after this route was
      // planned. Recheck every physical cell immediately before dispatch,
      // including the intermediate and destination cells of a two-tile Run.
      // This closes the R87 race where a normal combat route ran from
      // D022(338,356) through the live 338,355/338,354 exit and left the cave.
      const dispatchTransferKeys = new Set(
        liveTransferObstacles(client.snapshot, staticWalkableOverrides)
          .map(point => `${Number(point.x)},${Number(point.y)}`),
      );
      const liveSelf = selfPlayer(client);
      // Cadence waiting can overlap the authoritative response to an earlier
      // action. Never dispatch a route step planned from a transform that is
      // no longer current, and never let that earlier response satisfy this
      // movement's acknowledgement wait.
      if (distance(liveSelf, plannedFrom) > 0) {
        remaining = [];
        continue;
      }
      const before = { x: Number(liveSelf.x), y: Number(liveSelf.y) };
      const dx = Math.sign(Number(step.to.x) - Number(before.x));
      const dy = Math.sign(Number(step.to.y) - Number(before.y));
      const dispatchSteps = running ? 2 : 1;
      const protectedStep = Array.from({ length: dispatchSteps }, (_, index) => ({
        x: Number(liveSelf.x) + dx * (index + 1),
        y: Number(liveSelf.y) + dy * (index + 1),
      })).find(point => dispatchTransferKeys.has(`${point.x},${point.y}`));
      if (protectedStep) {
        rejected.push(protectedStep);
        remaining = [];
        client.record('diagnostic', {
          type: 'navigationTransferDispatchGuard',
          mapId,
          movementType: running ? 'run' : 'walk',
          transferCell: protectedStep,
        });
        if (++failures > maxNoPathRefreshes) {
          throw new Error(`No walk path on ${mapId} from ${liveSelf.x},${liveSelf.y} to ${target.x},${target.y}`);
        }
        continue;
      }
      // Evaluate caller-owned physical safety only after cadence and the
      // current-transform check.  A Run contains two server cells, both of
      // which must be admitted against the fresh AOI before its packet is
      // sent. Callers decide whether the guard covers ordinary transit,
      // combat movement, or both; a guarded caller must map its own hazard to
      // a bounded recovery path rather than recursively re-entering a route
      // resolver.
      if (typeof options.beforeMovement === 'function') {
        const physicalCells = Array.from({ length: dispatchSteps }, (_, index) => ({
          x: Number(liveSelf.x) + dx * (index + 1),
          y: Number(liveSelf.y) + dy * (index + 1),
        }));
        const hazard = options.beforeMovement({
          client,
          mapId,
          from: before,
          physicalCells,
          movementType: running ? 'run' : 'walk',
          target: { x: Number(target.x), y: Number(target.y) },
        });
        if (hazard) {
          client.record('diagnostic', {
            type: 'navigationBeforeMovementGuard',
            mapId,
            from: before,
            physicalCells,
            movementType: running ? 'run' : 'walk',
            ...(typeof hazard === 'object' ? hazard : {}),
          });
          throw new NavigationStepGuarded({
            ...(typeof hazard === 'object' ? hazard : {}),
            mapId,
            from: before,
            physicalCells,
            movementType: running ? 'run' : 'walk',
          });
        }
      }
      client.lastWalkAt = now();
      const movementResponseAfter = client.sequence;
      client.send({ type: running ? 'run' : 'walk', direction: directionNames[step.direction] });
      let movedDistance = 0;
      const movementInterruptedByDeath = () => {
        if (hasAuthoritativePlayerDeath(client.snapshot)) return true;
        return client.events.some(event =>
          event.sequence > movementResponseAfter && event.direction === 'received' &&
          event.packet === 'Death');
      };
      try {
        await client.wait(() => {
          if (movementInterruptedByDeath()) return true;
          if (distance(selfPlayer(client), before) > 0) return true;
          return client.events.some(event =>
            event.sequence > movementResponseAfter && event.direction === 'received' &&
            event.packet === 'UserLocation');
        }, 'authoritative movement response', movementResponseTimeoutMs);
        if (movementInterruptedByDeath()) {
          throw new Error('Player died during navigation');
        }
        movedDistance = distance(selfPlayer(client), before);
        if (movedDistance > 0) {
          successfulSteps += movedDistance;
          const consumed = remaining.findIndex(s => distance(s.to, selfPlayer(client)) === 0);
          if (consumed >= 0) remaining.splice(0, consumed + 1); else remaining = [];
          failures = 0;
        } else {
          const correctionBlockMask = selfActionBlockMask(client);
          if (correctionBlockMask !== 0) {
            remaining = [];
            if (correctionBlockMask !== lastMovementBlockMask) {
              client.record('diagnostic', {
                type: 'navigationMovementControlBlocked',
                mapId,
                position: before,
                poison: Number(selfPlayer(client)?.poison ?? client.snapshot?.playerPoison ?? 0),
                movementBlockMask: correctionBlockMask,
              });
            }
            lastMovementBlockMask = correctionBlockMask;
            await sleep(250);
            continue;
          }
          // An unchanged UserLocation is an authoritative collision
          // correction. Only this response is allowed to poison the cell.
          rejected.push(step.to);
          remaining = [];
          if (++failures >= 8) {
            throw new Error(`Authoritative movement rejected repeatedly on ${mapId}`);
          }
        }
      } catch (error) {
        if (client.failure) throw client.failure;
        if (client.closed) throw error;
        // Death authoritatively cancels the in-flight movement. It is neither
        // a lost movement response nor a reason to wait for the normal timeout.
        if (movementInterruptedByDeath()) throw error;
        // A local WebSocket is ordered and reliable. When the authoritative
        // result is unknown, resending can execute both the delayed original
        // and the retry and move two run segments at once. End this runner so
        // its normal reconnect resumes from the saved authoritative transform.
        remaining = [];
        client.record('diagnostic', {
          type: 'navigationMovementResponseTimeout',
          mapId,
          position: before,
          target: step.to,
          timeoutMs: movementResponseTimeoutMs,
        });
        throw error;
      }
      if (movedDistance > 0 && (detectPositionCycles || maxNonImprovingSteps > 0)) {
        const current = selfPlayer(client);
        const currentDistance = distance(current, target);
        const positionKey = `${current.x},${current.y}`;
        if (currentDistance < bestDistance) {
          bestDistance = currentDistance;
          nonImprovingSteps = 0;
          positionsSinceImprovement = new Set([positionKey]);
        } else {
          nonImprovingSteps += movedDistance;
          if (detectPositionCycles && positionsSinceImprovement.has(positionKey)) {
            throw new NavigationStalled({
              reason: 'position-cycle', mapId, position: current, target,
              bestDistance, successfulSteps,
            });
          }
          positionsSinceImprovement.add(positionKey);
          if (maxNonImprovingSteps > 0 && nonImprovingSteps >= maxNonImprovingSteps) {
            throw new NavigationStalled({
              reason: 'no-progress', mapId, position: current, target,
              bestDistance, successfulSteps,
            });
          }
        }
      }
    }
    throw new Error(`Navigation attempt budget exceeded (${maxAttempts})`);
  };
  // External combat/recovery code may complete a normal same-map emergency
  // scroll while this navigator is idle. Its hostile trail belongs to the
  // old field position, but only a fresh full snapshot can prove that the
  // same owner actually relocated. Ordinary movement, ObjectRemove, and
  // unproved coordinate changes intentionally retain their existing memory.
  navigateNear.resetHostileMemoryAfterVerifiedEmergencyRelocation = ({
    beforeSequence,
    before,
    relocation,
  } = {}) => {
    const current = activeOwnerTransform(client.snapshot);
    const relocationFrom = {
      mapFileName: relocation?.fromMapFileName,
      objectId: before?.objectId,
      x: relocation?.from?.x,
      y: relocation?.from?.y,
    };
    const relocationTo = {
      mapFileName: relocation?.toMapFileName,
      objectId: before?.objectId,
      x: relocation?.to?.x,
      y: relocation?.to?.y,
    };
    if (!hasFiniteExternalTransform(before) || !hasFiniteExternalTransform(relocationFrom) ||
        !hasFiniteExternalTransform(relocationTo)) return false;
    const capturedBefore = {
      mapFileName: String(before?.mapFileName ?? ''),
      objectId: Number(before?.objectId),
      x: Number(before?.x),
      y: Number(before?.y),
    };
    const from = {
      mapFileName: String(relocationFrom.mapFileName),
      objectId: Number(before?.objectId),
      x: Number(relocation?.from?.x),
      y: Number(relocation?.from?.y),
    };
    const to = {
      mapFileName: String(relocationTo.mapFileName),
      objectId: Number(before?.objectId),
      x: Number(relocation?.to?.x),
      y: Number(relocation?.to?.y),
    };
    const boundary = Number(beforeSequence);
    if (!current || !lastActiveOwner || !Number.isSafeInteger(boundary) ||
        !Number.isSafeInteger(from.objectId) || !sameMapOwner(lastActiveOwner, current) ||
        !sameMapOwner(capturedBefore, current) || !sameOwnerTransform(from, capturedBefore) ||
        !sameOwnerTransform(to, current) ||
        !(Number(relocation?.quantityAfter) < Number(relocation?.quantityBefore)) ||
        distance(from, current) < EMERGENCY_RELOCATION_MEMORY_RESET_DISTANCE) return false;
    const freshCurrentOwnerSnapshot = client.events?.some(event =>
      Number(event?.sequence) > boundary && event?.direction === 'received' &&
      event?.type === 'worldSnapshot' && sameOwnerTransform(activeOwnerTransform(event.payload), current),
    );
    if (!freshCurrentOwnerSnapshot) return false;
    const cleared = hostileMemory.size;
    hostileMemory.clear();
    client.record('diagnostic', {
      type: 'navigationHostileMemoryResetAfterEmergencyRelocation',
      mapId: current.mapFileName,
      objectId: current.objectId,
      from: { x: from.x, y: from.y },
      to: { x: current.x, y: current.y },
      distance: distance(from, current),
      cleared,
    });
    lastActiveOwner = current;
    return true;
  };
  return navigateNear;
}

function positiveIntegerOption(value, fallback) {
  if (value == null) return fallback;
  const parsed = Math.trunc(Number(value));
  if (!Number.isFinite(parsed) || parsed <= 0) throw new TypeError('Navigation limits must be positive integers');
  return parsed;
}

function nonnegativeIntegerOption(value, fallback) {
  if (value == null) return fallback;
  const parsed = Math.trunc(Number(value));
  if (!Number.isFinite(parsed) || parsed < 0) throw new TypeError('Navigation clearance must be a nonnegative integer');
  return parsed;
}

function ratioOption(value, fallback) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > 1) {
    throw new TypeError('Navigation ratio must be a number from 0 to 1');
  }
  return parsed;
}
