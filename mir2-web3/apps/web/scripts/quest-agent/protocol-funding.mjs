import { interactQuest } from './protocol-quest-actions.mjs';
import { expectedItemRewards, verifyItemRewards } from './protocol-rewards.mjs';
import { clearTravelBlockingMonster, harvestDirection } from './protocol-combat.mjs';
import { delay } from './protocol-client.mjs';
import { distance, selfPlayer } from './protocol-play.mjs';

const VILLAGE_FUNDING_ANCHOR = Object.freeze({ x: 288, y: 608 });
const VILLAGE_FUNDING_RADIUS = 120;

/** Conservative Venison target for rebuilding a quest supply budget. */
export function safeFundingVenisonTargetCount(requiredHpStock, {
  className = '',
  requiredMpStock = 0,
  additionalGold = 0,
} = {}) {
  const hpStock = Number(requiredHpStock);
  if (!Number.isSafeInteger(hpStock) || hpStock <= 0) {
    throw new TypeError('requiredHpStock must be a positive integer');
  }
  const mpStock = Number(requiredMpStock);
  if (!Number.isSafeInteger(mpStock) || mpStock < 0) {
    throw new TypeError('requiredMpStock must be a nonnegative integer');
  }
  const extraGold = Number(additionalGold);
  if (!Number.isSafeInteger(extraGold) || extraGold < 0) {
    throw new TypeError('additionalGold must be a nonnegative integer');
  }
  const caster = ['wizard', 'taoist'].includes(String(className).trim().toLowerCase());
  const mpBudget = caster ? Math.ceil(mpStock / 4) : 0;
  // Live Butcher/Ruben prices show one Venison funding about five HP drugs or
  // four MP drugs. Budget both resources explicitly so an HP-first shop pass
  // cannot strand a caster with an unusable skill bar.
  return Math.max(2, Math.ceil(hpStock / 5) + mpBudget + Math.ceil(extraGold / 200));
}

/**
 * A living player already in the beginner village can rebuild an empty supply
 * budget through the passive Deer/Venison economy. Keep the health floor here
 * so callers never send a freshly revived or critically injured character back
 * into even the low-risk funding field.
 */
export function canUseSafeDeerFunding(snapshot, {
  minimumHealthRatio = 0.7,
} = {}) {
  const hp = Number(snapshot?.playerHp ?? 0);
  const maxHp = Number(snapshot?.playerMaxHp ?? 0);
  return String(snapshot?.mapFileName ?? '') === '0' &&
    hp > 0 && maxHp > 0 && hp / maxHp >= Number(minimumHealthRatio);
}

/**
 * Turn in one already-complete ordinary quest when its gold reward can restore
 * a stranded newcomer's potion budget. This never accepts or advances a quest;
 * the authoritative log must already expose ReadyToTurnIn.
 */
export async function finishReadySupplyFundingQuest(owner, {
  route,
  travel,
  navigate,
  interact = interactQuest,
} = {}) {
  if (!owner?.snapshot) throw new TypeError('owner must provide an authoritative snapshot');
  if (!route || !Array.isArray(route.quests)) throw new TypeError('route must provide quests');
  if (typeof travel !== 'function') throw new TypeError('travel must be a function');
  if (typeof navigate !== 'function') throw new TypeError('navigate must be a function');
  if (typeof interact !== 'function') throw new TypeError('interact must be a function');

  const readyIds = new Set((owner.snapshot.questLog ?? [])
    .filter(quest => normalized(quest?.stage) === 'readytoturnin')
    .map(quest => Number(quest.questId)));
  const quest = route.quests.find(candidate =>
    readyIds.has(Number(candidate?.questId)) && candidate?.finishNpc &&
    Number(candidate?.rewards?.gold ?? 0) > 0);
  if (!quest) return null;

  const before = {
    gold: Number(owner.snapshot.gold),
    mapFileName: String(owner.snapshot.mapFileName),
  };
  await travel(quest.finishNpc.mapFileName);
  const turnInBefore = {
    gold: Number(owner.snapshot.gold),
    mapFileName: String(owner.snapshot.mapFileName),
  };
  const beforeItems = structuredClone(owner.snapshot);
  const reward = (quest.rewards?.selectableItems ?? []).find(item =>
    Number(item?.count ?? 0) > 0 &&
    (Number(item?.requiredClass ?? 0) & Number(route.classMask ?? 0)) !== 0 &&
    (Number(item?.requiredGender ?? 0) & 1) !== 0);
  const selectedItemIndex = reward?.selectionIndex ?? -1;
  const finish = await interact(
    owner,
    quest,
    { type: 'finish', selectedItemIndex },
    navigate,
  );

  const completed = owner.snapshot.questLog?.find(candidate =>
    Number(candidate?.questId) === Number(quest.questId));
  const expectedGold = turnInBefore.gold + Number(quest.rewards.gold);
  if (normalized(completed?.stage) !== 'completed' || Number(owner.snapshot.gold) !== expectedGold) {
    throw new Error(`q${quest.questId} supply funding was not authoritatively completed at ${expectedGold} gold`);
  }
  return {
    questId: Number(quest.questId),
    before,
    turnInBefore,
    after: {
      gold: Number(owner.snapshot.gold),
      mapFileName: String(owner.snapshot.mapFileName),
    },
    finish,
    itemRewards: verifyItemRewards(
      beforeItems,
      owner.snapshot,
      expectedItemRewards(quest, selectedItemIndex),
    ),
  };
}

/**
 * Recover a bankrupt newcomer through Crystal's ordinary Deer economy. Deer
 * are passive, every completed harvest yields Venison, and the normal supply
 * trip can sell that meat before buying potions. No quest state is changed and
 * every movement, attack, harvest and pickup goes through the public protocol.
 */
export async function collectSafeFundingVenison(owner, {
  route,
  travel,
  navigate,
  action,
  approachRange,
  sustain,
  refreshWhileWaiting,
  requiredCount = 2,
  maxHunts = 8,
  maxEmptySearchRefreshes = 20,
  clearMonster = clearTravelBlockingMonster,
  sleep = delay,
} = {}) {
  if (!owner?.snapshot || typeof owner.send !== 'function' || typeof owner.wait !== 'function') {
    throw new TypeError('owner must provide an authoritative protocol client');
  }
  if (!route || !Array.isArray(route.quests)) throw new TypeError('route must provide quests');
  if (typeof travel !== 'function') throw new TypeError('travel must be a function');
  if (typeof navigate !== 'function') throw new TypeError('navigate must be a function');
  if (typeof clearMonster !== 'function') throw new TypeError('clearMonster must be a function');

  const targetCount = positiveInteger(requiredCount, 'requiredCount');
  const huntLimit = positiveInteger(maxHunts, 'maxHunts');
  const emptySearchLimit = positiveInteger(maxEmptySearchRefreshes, 'maxEmptySearchRefreshes');
  const beforeCount = inventoryQuantity(owner.snapshot, 'Venison');
  await travel('0');
  const spawnCandidates = fundingSpawnCandidates(route, 'Deer', '0');
  if (spawnCandidates.length === 0) throw new Error('safe Deer funding has no authoritative map-0 spawn');
  const localSpawnCandidates = spawnCandidates.filter(candidate =>
    pointDistance(VILLAGE_FUNDING_ANCHOR, candidate.position) <= VILLAGE_FUNDING_RADIUS);
  const fundingSearchPoints = localFundingSearchPoints(
    localSpawnCandidates.length > 0 ? localSpawnCandidates : spawnCandidates,
  );

  let hunts = 0;
  let huntAttempts = 0;
  let emptySearchRefreshes = 0;
  const contestedDeerIds = new Set();
  while (inventoryQuantity(owner.snapshot, 'Venison') < targetCount && huntAttempts < huntLimit) {
    let target = liveFundingDeer(owner.snapshot, contestedDeerIds);
    if (!target) {
      const actor = selfPlayer(owner);
      const ordered = [...fundingSearchPoints].sort((left, right) =>
        pointDistance(actor, left.position) - pointDistance(actor, right.position) ||
        Number(right.count ?? 0) - Number(left.count ?? 0));
      for (const candidate of ordered) {
        const travelDistance = pointDistance(selfPlayer(owner), candidate.position);
        const successfulStepBudget = Math.min(
          2_048,
          Math.max(96, travelDistance * 2 + 192),
        );
        try {
          await navigate(candidate.position, 8, () => Boolean(liveFundingDeer(owner.snapshot, contestedDeerIds)), {
            hostileAvoidanceRadius: 2,
            // Map 0 has entrances hundreds of cells from the beginner fields.
            // Bound the trip by authoritative distance plus a fixed detour
            // allowance instead of truncating every long but healthy route at
            // 96 successful movement attempts.
            // Mir maps contain long walls and building footprints, so the
            // shortest collision path can be materially longer than Chebyshev
            // distance. Reserve one full detour distance plus a fixed margin
            // for actual movement, while keeping retry/replan attempts on a
            // separate bounded budget. A nearby spawn can still require many
            // replans around walls and live actors without walking too far.
            maxSuccessfulSteps: successfulStepBudget,
            maxAttempts: 2_048,
          });
        } catch (error) {
          if (!String(error?.message ?? '').startsWith('No walk path')) throw error;
          if (typeof owner.record === 'function') {
            owner.record('diagnostic', {
              type: 'safeSupplyFundingSpawnUnreachable',
              mapFileName: '0',
              position: { x: Number(candidate.position.x), y: Number(candidate.position.y) },
            });
          }
          continue;
        }
        target = liveFundingDeer(owner.snapshot, contestedDeerIds);
        if (target) break;
      }
    }
    if (!target) {
      emptySearchRefreshes += 1;
      if (emptySearchRefreshes > emptySearchLimit) {
        throw new Error(
          `safe Deer funding found no live Deer after ${emptySearchLimit} bounded refreshes`,
        );
      }
      if (typeof owner.record === 'function') {
        owner.record('diagnostic', {
          type: 'safeSupplyFundingAwaitRespawn',
          refresh: emptySearchRefreshes,
          limit: emptySearchLimit,
          mapFileName: '0',
        });
      }
      if (typeof refreshWhileWaiting === 'function') {
        await refreshWhileWaiting(owner);
      } else {
        const afterSequence = Number(owner.sequence ?? 0);
        owner.send({ type: 'clientVersion' });
        await owner.wait(
          () => (owner.events ?? []).some(event =>
            Number(event?.sequence) > afterSequence && event?.direction === 'received' &&
            event?.type === 'worldSnapshot'),
          'safe Deer funding respawn refresh',
          6_000,
        );
      }
      await sleep(3_000);
      continue;
    }
    emptySearchRefreshes = 0;

    const targetId = Number(target.objectId);
    const engagementSequence = Number(owner.sequence ?? 0);
    huntAttempts += 1;
    try {
      await clearMonster(owner, target, navigate, {
        // Crystal treats passive Deer as huntable neutral creatures: melee
        // can hit them, while hostile-only Wizard/Taoist magic is rejected.
        // Keep the funding loop on the public attack command for every class.
        action: physicalFundingAttack,
        approachRange: 1,
        sustain,
        attackCadenceMs: 650,
        combatHostileClearance: 0,
        combatHostileClearanceFallback: 0,
        maxMovingTargetNoProgressMs: 20_000,
        refreshWhileWaiting,
      });
    } catch (error) {
      if (!recoverableFundingTargetLoss(error)) throw error;
      contestedDeerIds.add(targetId);
      if (typeof owner.record === 'function') {
        owner.record('diagnostic', {
          type: 'safeSupplyFundingTargetContested',
          objectId: targetId,
          reason: String(error?.message ?? error),
          attempt: huntAttempts,
        });
      }
      continue;
    }
    hunts += 1;
    let corpse = entityById(owner.snapshot, targetId);
    if (!corpse || (corpse.dead !== true && Number(corpse.hp) > 0)) continue;
    if (!receivedPacketAfter(owner, engagementSequence, 'ObjectDied', targetId)) {
      try {
        await owner.wait(() =>
          receivedPacketAfter(owner, engagementSequence, 'ObjectDied', targetId) ||
          !entityById(owner.snapshot, targetId),
        `Deer ${targetId} authoritative death`, 6_000);
      } catch (error) {
        if (!String(error?.message ?? '').startsWith('Timeout waiting for')) throw error;
      }
      corpse = entityById(owner.snapshot, targetId);
      if (!receivedPacketAfter(owner, engagementSequence, 'ObjectDied', targetId) || !corpse) {
        recordFundingCorpseUnavailable(owner, targetId, 'deathNotConfirmed');
        continue;
      }
    }
    const harvested = await harvestFundingCorpse(
      owner,
      corpse,
      navigate,
      inventoryQuantity(owner.snapshot, 'Venison') + 1,
      sleep,
    );
    if (!harvested) recordFundingCorpseUnavailable(owner, targetId, 'noVenisonAward');
  }

  const afterCount = inventoryQuantity(owner.snapshot, 'Venison');
  const collected = afterCount - beforeCount;
  if (afterCount < targetCount) {
    throw new Error(`safe Deer funding holds ${afterCount}/${targetCount} Venison after ${hunts} hunts and ${huntAttempts} attempts`);
  }
  if (typeof owner.record === 'function') {
    owner.record('diagnostic', {
      type: 'safeSupplyFunding',
      method: 'DeerVenison',
      collected,
      hunts,
      mapFileName: String(owner.snapshot?.mapFileName ?? ''),
    });
  }
  return { method: 'DeerVenison', collected, hunts, beforeCount, afterCount };
}

async function physicalFundingAttack(client, target) {
  const objectId = Number(target?.objectId);
  if (!Number.isSafeInteger(objectId) || objectId <= 0) {
    throw new Error('safe Deer funding requires an authoritative target objectId');
  }
  const command = { type: 'attack', objectId };
  client.send(command);
  return { kind: 'attack', targetId: objectId, command };
}

function recoverableFundingTargetLoss(error) {
  const message = String(error?.message ?? '');
  return /^target \d+ left the authoritative snapshot before (?:attack|death was confirmed)$/i.test(message) ||
    /^target \d+ made no authoritative combat progress after \d+ attacks$/i.test(message);
}

function liveFundingDeer(snapshot, ignoredObjectIds) {
  const actor = selfPlayer({ snapshot });
  return (snapshot?.entities ?? [])
    .filter(entity => normalized(entity?.kind) === 'monster' &&
      normalized(entity?.name) === 'deer' && entity?.dead !== true && Number(entity?.hp) > 0 &&
      !ignoredObjectIds.has(Number(entity?.objectId)))
    .sort((left, right) => pointDistance(actor, left) - pointDistance(actor, right) ||
      Number(left.objectId) - Number(right.objectId))[0] ?? null;
}

async function harvestFundingCorpse(owner, initialCorpse, navigate, targetInventoryCount, sleep) {
  const objectId = Number(initialCorpse.objectId);
  for (let pass = 0; pass < 16; pass += 1) {
    if (inventoryQuantity(owner.snapshot, 'Venison') >= targetInventoryCount) return true;
    if (await pickUpVisibleVenison(owner, navigate, targetInventoryCount)) return true;
    const corpse = entityById(owner.snapshot, objectId);
    if (!corpse || (corpse.dead !== true && Number(corpse.hp) > 0)) {
      if (await waitForVisibleVenison(owner, navigate, targetInventoryCount, sleep)) return true;
      return false;
    }
    try {
      await navigate(corpse, 1);
    } catch (error) {
      if (entityById(owner.snapshot, objectId)) throw error;
      if (await waitForVisibleVenison(owner, navigate, targetInventoryCount, sleep)) return true;
      return false;
    }
    const actor = selfPlayer(owner);
    const refreshed = entityById(owner.snapshot, objectId);
    if (!actor || !refreshed || distance(actor, refreshed) > 1) {
      if (!refreshed) return false;
      throw new Error(`Deer corpse ${objectId} is not adjacent for funding harvest`);
    }
    const afterSequence = Number(owner.sequence ?? 0);
    owner.send({ type: 'harvest', direction: harvestDirection(actor, refreshed) });
    try {
      await owner.wait(() =>
        inventoryQuantity(owner.snapshot, 'Venison') >= targetInventoryCount ||
        visibleDrop(owner.snapshot, 'Venison') ||
        !entityById(owner.snapshot, objectId),
      `Deer corpse ${objectId} funding harvest`, 6_000);
    } catch (error) {
      if (!String(error?.message ?? '').startsWith('Timeout waiting for')) throw error;
      if (receivedPacketAfter(owner, afterSequence, 'ObjectHarvest', objectId) ||
          receivedPacketAfter(owner, afterSequence, 'ObjectHarvested', objectId)) return false;
      continue;
    }
    if (inventoryQuantity(owner.snapshot, 'Venison') >= targetInventoryCount) return true;
    if (await pickUpVisibleVenison(owner, navigate, targetInventoryCount)) return true;
    if (!entityById(owner.snapshot, objectId) &&
        await waitForVisibleVenison(owner, navigate, targetInventoryCount, sleep)) return true;
    if (receivedPacketAfter(owner, afterSequence, 'ObjectHarvest', objectId) ||
        receivedPacketAfter(owner, afterSequence, 'ObjectHarvested', objectId)) return false;
    await sleep(2_100);
  }
  return false;
}

function recordFundingCorpseUnavailable(owner, objectId, reason) {
  if (typeof owner.record !== 'function') return;
  owner.record('diagnostic', {
    type: 'safeSupplyFundingCorpseUnavailable',
    objectId: Number(objectId),
    reason: String(reason),
  });
}

async function waitForVisibleVenison(owner, navigate, targetInventoryCount, sleep) {
  const deadline = Date.now() + 3_000;
  while (Date.now() < deadline) {
    if (inventoryQuantity(owner.snapshot, 'Venison') >= targetInventoryCount) return true;
    if (await pickUpVisibleVenison(owner, navigate, targetInventoryCount)) return true;
    owner.send({ type: 'clientVersion' });
    await sleep(200);
  }
  return inventoryQuantity(owner.snapshot, 'Venison') >= targetInventoryCount;
}

async function pickUpVisibleVenison(owner, navigate, targetInventoryCount) {
  const drop = visibleDrop(owner.snapshot, 'Venison');
  if (!drop) return false;
  await navigate(drop, 1);
  const objectId = Number(drop.objectId);
  owner.send({ type: 'pickUp', objectId });
  await owner.wait(() =>
    inventoryQuantity(owner.snapshot, 'Venison') >= targetInventoryCount ||
    !(owner.snapshot?.groundDrops ?? []).some(entry => Number(entry.objectId) === objectId),
  `Venison pickup ${objectId}`, 6_000);
  return inventoryQuantity(owner.snapshot, 'Venison') >= targetInventoryCount;
}

function fundingSpawnCandidates(route, monsterName, mapFileName) {
  const wanted = normalized(monsterName);
  const candidates = [];
  for (const quest of route.quests ?? []) {
    for (const objective of quest?.objectives?.kill ?? []) {
      if (normalized(objective?.monsterName) !== wanted) continue;
      candidates.push(...(objective.spawnCandidates ?? []));
    }
    for (const objective of quest?.objectives?.item ?? []) {
      for (const source of objective?.sources ?? []) {
        if (normalized(source?.monsterName) !== wanted) continue;
        candidates.push(...(source.spawnCandidates ?? []));
      }
    }
  }
  const unique = new Map();
  for (const candidate of candidates) {
    if (String(candidate?.mapFileName ?? '') !== String(mapFileName) || !candidate?.position) continue;
    const key = `${candidate.position.x},${candidate.position.y}`;
    if (!unique.has(key)) unique.set(key, candidate);
  }
  return [...unique.values()];
}

function localFundingSearchPoints(candidates) {
  const unique = new Map();
  for (const candidate of candidates) {
    const center = candidate?.position;
    if (!center) continue;
    const spread = Math.max(0, Math.ceil(Number(candidate?.spread ?? 0)));
    const offset = spread > 0 ? Math.min(32, Math.max(8, spread - 8)) : 0;
    const offsets = offset > 0 ? [-offset, 0, offset] : [0];
    for (const dx of offsets) {
      for (const dy of offsets) {
        const position = {
          x: Math.max(0, Math.round(Number(center.x) + dx)),
          y: Math.max(0, Math.round(Number(center.y) + dy)),
        };
        if (pointDistance(VILLAGE_FUNDING_ANCHOR, position) > VILLAGE_FUNDING_RADIUS) continue;
        const key = `${position.x},${position.y}`;
        if (!unique.has(key)) unique.set(key, { ...candidate, position });
      }
    }
  }
  return [...unique.values()];
}

function visibleDrop(snapshot, itemName) {
  const wanted = normalized(itemName);
  return (snapshot?.groundDrops ?? []).find(drop =>
    normalized(drop?.name ?? drop?.loot?.name ?? drop?.loot?.key) === wanted);
}

function inventoryQuantity(snapshot, itemName) {
  const wanted = normalized(itemName);
  return [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])]
    .filter(item => normalized(item?.name ?? item?.key) === wanted)
    .reduce((total, item) => total + Math.max(1, Number(item?.quantity ?? 1)), 0);
}

function entityById(snapshot, objectId) {
  return (snapshot?.entities ?? []).find(entity => Number(entity?.objectId) === Number(objectId));
}

function receivedPacketAfter(owner, sequence, packet, objectId = null) {
  return (owner?.events ?? []).some(event =>
    event.sequence > sequence && event.direction === 'received' && event.packet === packet &&
    (objectId == null || Number(event.payload?.objectId ?? event.payload?.info?.objectId) === Number(objectId)));
}

function pointDistance(left, right) {
  if (!left || !right) return Number.POSITIVE_INFINITY;
  return Math.max(
    Math.abs(Number(left.x) - Number(right.x)),
    Math.abs(Number(left.y) - Number(right.y)),
  );
}

function positiveInteger(value, label) {
  const number = Number(value);
  if (!Number.isSafeInteger(number) || number <= 0) throw new TypeError(`${label} must be a positive integer`);
  return number;
}

function normalized(value) {
  return String(value ?? '').trim().toLowerCase();
}
