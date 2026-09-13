import { loadProtocolCollisionMap, planProtocolNavigation } from './protocol-navigation.mjs';
import { combatApproachRange } from './protocol-loadout.mjs';
import { distance, selfPlayer } from './protocol-play.mjs';

/**
 * Wrap an ordinary ranged combat action with a bounded caster retreat. Every
 * chosen cell is proven by the same Crystal collision planner used by normal
 * travel. The exported name is retained for existing callers.
 */
export function createWizardKitingAction(baseAction, navigateNear, options = {}) {
  if (typeof baseAction !== 'function') throw new TypeError('baseAction must be a function');
  if (typeof navigateNear !== 'function') throw new TypeError('navigateNear must be a function');
  const loadCollisionMap = options.loadCollisionMap ?? loadProtocolCollisionMap;
  const maxRetreatSteps = positiveInteger(options.maxRetreatSteps, 3);
  const maxRetreatCellsPerTarget = positiveInteger(options.maxRetreatCellsPerTarget, 12);
  const maxNavigationAttempts = positiveInteger(options.maxNavigationAttempts, 8);
  const triggerDistance = positiveInteger(options.triggerDistance, 2);
  const maxTargetDistance = positiveInteger(options.maxTargetDistance, 9);
  const maxExpanded = positiveInteger(options.maxExpanded, 256);
  const fightWhenBlocked = options.fightWhenBlocked === true;
  const maps = new Map();
  let encounterKey = null;
  let encounterRetreatedCells = 0;

  return async function wizardKitingAction(client, target) {
    const actor = selfPlayer(client);
    if (!actor) throw new Error('Cannot kite without the authoritative player entity');
    const targetId = Number(target?.objectId);
    const hostiles = visibleHostileMonsters(client?.snapshot);
    // combatApproachRange is authoritative for both Wizard projectile spells
    // and Taoist SoulFireBall with an equipped Amulet. Warriors and casters
    // without a usable ranged loadout remain at range one and never kite.
    const rangedReady = combatApproachRange(client, target) > 1;
    const closeThreat = hostiles.some(entity => distance(actor, entity) <= triggerDistance);
    if (!rangedReady || !closeThreat) return baseAction(client, target);
    if (!Number.isSafeInteger(targetId) || targetId <= 0) {
      throw new Error('Wizard kite target has no valid objectId');
    }
    const mapId = String(client.snapshot?.mapFileName ?? '');
    if (!mapId) throw new Error('Cannot kite without an authoritative mapFileName');
    const nextEncounterKey = `${mapId}:${targetId}`;
    if (nextEncounterKey !== encounterKey) {
      encounterKey = nextEncounterKey;
      encounterRetreatedCells = 0;
    }

    const remainingBudget = maxRetreatCellsPerTarget - encounterRetreatedCells;
    if (remainingBudget <= 0) {
      if (fightWhenBlocked) {
        recordFallback(client, targetId, 'retreatCellBudgetExceeded');
        return baseAction(client, target);
      }
      throw new Error(`Wizard retreat cell budget exceeded for target ${targetId} (${maxRetreatCellsPerTarget})`);
    }
    const stepBudget = Math.min(maxRetreatSteps, remainingBudget);
    if (!maps.has(mapId)) maps.set(mapId, await loadCollisionMap(mapId));
    const plan = chooseRetreatPlan({
      map: maps.get(mapId), actor, target, hostiles,
      dynamicObstacles: (client.snapshot?.entities ?? []).filter(entity =>
        Number(entity?.objectId) !== Number(actor.objectId) && entity?.dead !== true),
      stepBudget, maxTargetDistance, maxExpanded,
    });
    if (!plan) {
      if (fightWhenBlocked) {
        recordFallback(client, targetId, 'noCollisionSafeRetreat');
        return baseAction(client, target);
      }
      throw new Error(`No collision-safe Wizard retreat on ${mapId} from ${actor.x},${actor.y}`);
    }

    const before = { x: Number(actor.x), y: Number(actor.y) };
    let navigation;
    try {
      navigation = await navigateNear(plan.destination, 0, () => false, {
        maxSuccessfulSteps: stepBudget,
        maxAttempts: maxNavigationAttempts,
      });
    } catch (error) {
      const navigationBlocked = /^No walk path\b/.test(String(error?.message ?? ''));
      const boundedProgress = /^Navigation successful step budget exceeded\b/.test(
        String(error?.message ?? ''),
      );
      if (!fightWhenBlocked || (!navigationBlocked && !boundedProgress)) throw error;
      const blockedActor = selfPlayer(client);
      if (String(client.snapshot?.mapFileName ?? '') !== mapId) {
        throw new Error(`Wizard retreat changed map before combat action (${mapId})`);
      }
      if (!blockedActor || blockedActor.dead === true || Number(blockedActor.hp) <= 0 ||
          Number(client.snapshot?.playerHp) <= 0) {
        throw new Error('Player died during Wizard retreat');
      }
      const blockedTarget = entityById(client.snapshot, targetId);
      if (!blockedTarget) {
        throw new Error(`target ${targetId} left the authoritative snapshot during Wizard retreat`);
      }
      if (boundedProgress) {
        navigation = { reached: false, successfulSteps: null };
      } else {
        recordFallback(client, targetId, 'retreatNavigationNoPath');
        return baseAction(client, blockedTarget);
      }
    }
    const after = selfPlayer(client);
    if (String(client.snapshot?.mapFileName ?? '') !== mapId) {
      throw new Error(`Wizard retreat changed map before combat action (${mapId})`);
    }
    if (!after || after.dead === true || Number(after.hp) <= 0 || Number(client.snapshot?.playerHp) <= 0) {
      throw new Error('Player died during Wizard retreat');
    }
    const displacement = after ? distance(before, after) : 0;
    // The navigator's successfulSteps can include an authoritative movement
    // that arrived just before this retreat began. The before/after player
    // transform is the exact bound for this action.
    const moved = displacement;
    const reportedSteps = Number.isSafeInteger(Number(navigation?.successfulSteps))
      ? Number(navigation.successfulSteps)
      : null;
    if (moved <= 0 || moved > stepBudget) {
      if (fightWhenBlocked && moved <= 0) {
        recordFallback(client, targetId, 'retreatNavigationStalled');
        const stalledTarget = entityById(client.snapshot, targetId);
        if (!stalledTarget) {
          throw new Error(`target ${targetId} left the authoritative snapshot during Wizard retreat`);
        }
        return baseAction(client, stalledTarget);
      }
      recordFallback(client, targetId, 'retreatDisplacementOutOfBounds', {
        moved,
        reportedSteps,
        stepBudget,
      });
      throw new Error(`Wizard retreat did not complete within its ${stepBudget}-cell budget`);
    }
    if (navigation?.reached === false) {
      recordFallback(client, targetId, 'partialRetreatProgress');
    }
    encounterRetreatedCells += moved;

    const refreshed = entityById(client.snapshot, targetId);
    if (!refreshed) throw new Error(`target ${targetId} left the authoritative snapshot during Wizard retreat`);
    if (refreshed.dead === true || Number(refreshed.hp) <= 0) {
      return { kind: 'retreated', targetId };
    }
    return baseAction(client, refreshed);
  };
}

function recordFallback(client, targetId, reason, details = {}) {
  if (typeof client?.record !== 'function') return;
  client.record('diagnostic', {
    type: 'wizardKiteFallback',
    reason,
    targetId,
    mapFileName: String(client.snapshot?.mapFileName ?? ''),
    ...details,
  });
}

export function visibleHostileMonsters(snapshot) {
  return (snapshot?.entities ?? []).filter(entity =>
    normalized(entity?.kind) === 'monster' &&
    normalized(entity?.disposition) === 'hostile' &&
    entity?.dead !== true &&
    !(entity?.hp != null && Number(entity.hp) <= 0) &&
    validPoint(entity));
}

function chooseRetreatPlan({
  map, actor, target, hostiles, dynamicObstacles, stepBudget, maxTargetDistance, maxExpanded,
}) {
  const initialSafety = minimumDistance(actor, hostiles);
  const candidates = [];
  for (let y = Number(actor.y) - stepBudget; y <= Number(actor.y) + stepBudget; y += 1) {
    for (let x = Number(actor.x) - stepBudget; x <= Number(actor.x) + stepBudget; x += 1) {
      const destination = { x, y };
      if (distance(actor, destination) === 0 || distance(target, destination) > maxTargetDistance) continue;
      const plan = planProtocolNavigation({
        map,
        start: actor,
        target: destination,
        desiredDistance: 0,
        dynamicObstacles,
        maxExpanded,
      });
      if (!plan || plan.steps.length === 0 || plan.steps.length > stepBudget) continue;
      let previousSafety = initialSafety;
      let monotonic = true;
      for (const point of plan.path.slice(1)) {
        const safety = minimumDistance(point, hostiles);
        if (safety < previousSafety) { monotonic = false; break; }
        previousSafety = safety;
      }
      if (!monotonic || previousSafety <= initialSafety) continue;
      candidates.push({
        destination,
        steps: plan.steps.length,
        safety: previousSafety,
        targetDistance: distance(target, destination),
      });
    }
  }
  return candidates.sort((left, right) =>
    right.safety - left.safety || left.steps - right.steps ||
    left.targetDistance - right.targetDistance ||
    left.destination.y - right.destination.y || left.destination.x - right.destination.x)[0] ?? null;
}

function minimumDistance(point, entities) {
  return Math.min(...entities.map(entity => distance(point, entity)));
}

function entityById(snapshot, objectId) {
  return (snapshot?.entities ?? []).find(entity => Number(entity?.objectId) === Number(objectId));
}

function validPoint(value) {
  return Number.isFinite(Number(value?.x)) && Number.isFinite(Number(value?.y));
}

function normalized(value) {
  return String(value ?? '').trim().toLowerCase();
}

function positiveInteger(value, fallback) {
  const parsed = Math.trunc(Number(value));
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}
