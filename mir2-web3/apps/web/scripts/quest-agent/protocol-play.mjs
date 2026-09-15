import { loadProtocolCollisionMap, planProtocolNavigation } from './protocol-navigation.mjs';
import { delay } from './protocol-client.mjs';
import { useSupplies } from './protocol-loadout.mjs';

export const selfPlayer = client => client.snapshot?.entities.find(e => e.objectId === client.snapshot.playerObjectId);
export const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const MOVEMENT_BLOCKING_POISON_MASK = 8 | 16 | 32 | 256;
function selfMovementBlockMask(client) {
  const self = selfPlayer(client);
  return Number(self?.poison ?? client.snapshot?.playerPoison ?? 0) & MOVEMENT_BLOCKING_POISON_MASK;
}
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
export async function reviveInTown(client) {
  const before = { map: client.snapshot.mapFileName, ...selfPlayer(client) };
  if (!before.dead && client.snapshot.playerHp > 0) return null;
  await client.request({ type: 'townRevive' }, 'Revived');
  const after = client.sequence;
  client.send({ type: 'clientVersion' });
  await client.wait(() => client.events.some(e => e.sequence > after && e.type === 'worldSnapshot') && client.snapshot.playerHp > 0 && !selfPlayer(client).dead, 'town revival snapshot');
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

function hostileClearanceObstacles(snapshot, options, memory, observedAt, memoryDurationMs) {
  const radius = nonnegativeIntegerOption(options?.hostileAvoidanceRadius, 0);
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
      if (radius === 0) {
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
  return async function navigateNear(target, desiredDistance = 1, stopWhen = () => false, options = {}) {
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
    let bestDistance = distance(selfPlayer(client), target);
    let nonImprovingSteps = 0;
    let lastMovementBlockMask = 0;
    let positionsSinceImprovement = new Set([`${selfPlayer(client).x},${selfPlayer(client).y}`]);
    let remaining = [];
    for (let count = 0; count < maxAttempts; count++) {
      if (client.failure) throw client.failure;
      if (client.closed) throw new Error('Protocol client closed during navigation');
      if (stopWhen()) return { reached: false, successfulSteps };
      const self = selfPlayer(client);
      if (self.dead || client.snapshot.playerHp <= 0) throw new Error('Player died during navigation');
      const movementBlockMask = selfMovementBlockMask(client);
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
      const nearbyHostiles = (client.snapshot?.entities ?? []).filter(entity =>
        entity?.kind === 'monster' && entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
        entity?.disposition !== 'friendly' && distance(self, entity) <= emergencyEscapeDangerDistance);
      const hpRatio = Number(client.snapshot.playerHp) /
        Math.max(1, Number(client.snapshot.playerMaxHp));
      const shouldEmergencyEscape = nearbyHostiles.length >= 2 ||
        (nearbyHostiles.length > 0 && hpRatio <= emergencyEscapeCriticalHpRatio);
      if (emergencyEscape && !emergencyEscapeFailed && now() >= emergencyEscapeRetryAt &&
          emergencyEscapes < maxEmergencyEscapes &&
          hpRatio <= emergencyEscapeHpRatio && shouldEmergencyEscape) {
        const before = { mapFileName: mapId, x: Number(self.x), y: Number(self.y) };
        client.record('diagnostic', {
          type: 'navigationEmergencyEscapeAttempt',
          hpRatio,
          nearby: nearbyHostiles.length,
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
                hpRatio: Number(client.snapshot.playerHp) /
                  Math.max(1, Number(client.snapshot.playerMaxHp)),
                nearby: nearbyHostiles.length,
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
          client.snapshot.playerHp < client.snapshot.playerMaxHp * 0.6 &&
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
        ...hostileClearanceObstacles(
          client.snapshot,
          options,
          hostileMemory,
          now(),
          nonnegativeIntegerOption(options?.hostileAvoidanceRadius, 0) === 0
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
      const before = { x: self.x, y: self.y };
      if (successfulSteps >= maxSuccessfulSteps) {
        throw new Error(`Navigation successful step budget exceeded (${maxSuccessfulSteps})`);
      }
      const running = maxSuccessfulSteps - successfulSteps >= 2 &&
        remaining.length >= 2 && remaining[1].direction === step.direction &&
        !obstacleKeys.has(`${Number(remaining[1].to.x)},${Number(remaining[1].to.y)}`);
      await sleep(Math.max(0, 650 - (now() - (client.lastWalkAt ?? 0))));
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
      client.lastWalkAt = now();
      const movementResponseAfter = client.sequence;
      client.send({ type: running ? 'run' : 'walk', direction: directionNames[step.direction] });
      let movedDistance = 0;
      const movementInterruptedByDeath = () => {
        const currentSelf = selfPlayer(client);
        if (currentSelf?.dead === true || Number(client.snapshot.playerHp) <= 0) return true;
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
          const correctionBlockMask = selfMovementBlockMask(client);
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
