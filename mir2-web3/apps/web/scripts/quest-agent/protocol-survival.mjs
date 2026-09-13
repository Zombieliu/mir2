const DEFAULT_MAX_RESTOCKS = 32;
const DEFAULT_MINIMUM_HP_STOCK = 1;
const REFRESH_TIMEOUT_MS = 12_000;
const PASSIVE_RECOVERY_POLL_MS = 3_100;

export function hpRestockTargetForActiveQuests(snapshot, {
  fallback = 24,
  targets = {},
} = {}) {
  let target = nonnegativeInteger(fallback, 'fallback HP target');
  for (const quest of snapshot?.questLog ?? []) {
    if (normalizedStage(quest?.stage) !== 'inprogress') continue;
    const configured = targets?.[Number(quest?.questId)];
    if (configured == null) continue;
    target = Math.max(target, nonnegativeInteger(configured, `quest ${quest.questId} HP target`));
  }
  return target;
}

export function minimumJourneyMpStockForQuest(questId, className) {
  const normalizedClass = String(className ?? '').trim().toLowerCase();
  if (!['wizard', 'taoist'].includes(normalizedClass)) return 0;
  if (normalizedClass === 'taoist' && Number(questId) === 42) return 0;
  if (Number(questId) === 54) return 12;
  return 4;
}

/** Full village departure stock; intentionally separate from the field trigger above. */
export function journeyMpRestockTargetForQuest(questId, className, {
  fallback = 12,
} = {}) {
  const normalizedClass = String(className ?? '').trim().toLowerCase();
  if (!['wizard', 'taoist'].includes(normalizedClass)) return 0;
  if (Number(questId) === 54) return 80;
  if (Number(questId) === 42 && normalizedClass === 'wizard') return 32;
  return nonnegativeInteger(fallback, 'fallback MP target');
}

const LONG_EVASIVE_RECOVERY_QUESTS = new Set([30, 33, 36, 42, 49, 54, 60, 62]);
const WIDE_DANGER_RECOVERY_QUESTS = new Set([42, 60, 62]);

export function evasiveRecoveryTimeoutMsForQuest(questId) {
  return LONG_EVASIVE_RECOVERY_QUESTS.has(Number(questId)) ? 90_000 : 45_000;
}

export function evasiveRecoveryDangerDistanceForQuest(questId) {
  return WIDE_DANGER_RECOVERY_QUESTS.has(Number(questId)) ? 8 : 7;
}

export function questRetreatProfile(questId, className = '') {
  const id = Number(questId);
  const normalizedClass = String(className).trim().toLowerCase();
  const fragileQ54Expedition = id === 54 && ['wizard', 'taoist'].includes(normalizedClass);
  return {
    allowLowHealthFollowerRecovery: [30, 33, 36, 49, 54, 60, 62].includes(id),
    continueTravelWhileHealthy: [54, 62].includes(id) && !fragileQ54Expedition,
    maxTravelThreatEvasionsPerEdge: id === 62 ? 3 : 1,
    maxRetreatBreakoutKills: [54, 62].includes(id) && !fragileQ54Expedition ? 3 : 1,
    multiAggressorRetreatRatio: id === 54 ? (fragileQ54Expedition ? 0.75 : 0.55) : undefined,
    retreatAtActiveAggressorCount: id === 54 ? (fragileQ54Expedition ? 1 : 3) : undefined,
    unsafeRetreatSteps: id === 62 || fragileQ54Expedition ? 24 :
      ([30, 33, 36].includes(id) ? 16 : 12),
    unsafeRetreatSafeDistance: id === 62 || fragileQ54Expedition ? 8 :
      ([30, 33, 36].includes(id) ? 10 : 6),
  };
}

export function questRetreatBiasPosition(questId, snapshot) {
  const id = Number(questId);
  const mapFileName = String(snapshot?.mapFileName ?? '');
  // q62 has no objective source on D2041; its reachable source is D2042.
  // Retreating toward the D2041 entrance erased the entire crossing after
  // every pack. Preserve safe forward progress toward the real 2F transfer.
  if (id === 62 && mapFileName === 'D2041') return { x: 262, y: 13 };
  if (id !== 54) return null;
  return ({
    D401: { x: 76, y: 15 },
    D411: { x: 95, y: 49 },
    D421: { x: 361, y: 19 },
  })[mapFileName] ?? null;
}

export function shouldPreferObjectiveMapOverCurrent(questId) {
  // q49's Natural Cave source is intentionally safer than its current-map Oma
  // alternative. q54 is different: D401 already contains required Zombie2/3
  // targets, so forcing D406 first creates a long, reward-free mine crossing.
  return Number(questId) === 49;
}

export function createPostEngagementSupplyGate({
  travel,
  navigateNear,
  restock,
  minimumHpStock = DEFAULT_MINIMUM_HP_STOCK,
  minimumMpStock = 0,
  maxRestocks = DEFAULT_MAX_RESTOCKS,
} = {}) {
  if (typeof travel !== 'function') throw new TypeError('travel must be a function');
  if (typeof navigateNear !== 'function') throw new TypeError('navigateNear must be a function');
  if (typeof restock !== 'function') throw new TypeError('restock must be a function');
  const defaultHp = nonnegativeInteger(minimumHpStock, 'minimumHpStock');
  const defaultMp = nonnegativeInteger(minimumMpStock, 'minimumMpStock');
  const restockLimit = positiveInteger(maxRestocks, 'maxRestocks');
  let restockCount = 0;

  return async function ensureSupplies(client, {
    minimumHpStock: requestedHp = defaultHp,
    minimumMpStock: requestedMp = defaultMp,
    requiredAfterRestockHpStock = requestedHp,
    requiredAfterRestockMpStock = requestedMp,
    returnMapFileName = null,
    forceRestock = false,
  } = {}) {
    requireSnapshot(client);
    const triggerHp = nonnegativeInteger(requestedHp, 'minimumHpStock');
    const triggerMp = nonnegativeInteger(requestedMp, 'minimumMpStock');
    const departureHp = nonnegativeInteger(requiredAfterRestockHpStock, 'requiredAfterRestockHpStock');
    const departureMp = nonnegativeInteger(requiredAfterRestockMpStock, 'requiredAfterRestockMpStock');
    let hp = hpDrugCount(client.snapshot);
    let mp = mpDrugCount(client.snapshot);
    if (!forceRestock && hp >= triggerHp && mp >= triggerMp) {
      return { status: 'sufficient', hp, mp, restockCount };
    }
    if (restockCount >= restockLimit) {
      throw new Error(`restock limit exceeded (${restockCount}/${restockLimit})`);
    }

    const originalMap = String(client.snapshot.mapFileName ?? '');
    if (originalMap !== '0') await travel('0');
    restockCount += 1;
    const targetHp = Math.max(triggerHp, departureHp);
    const targetMp = Math.max(triggerMp, departureMp);
    const supplyOptions = { targetHp };
    if (targetMp > 0) supplyOptions.targetMp = targetMp;
    supplyOptions.lowStockHp = targetHp;
    if (targetMp > 0) supplyOptions.lowStockMp = targetMp;
    const restockResult = await restock(client, navigateNear, supplyOptions);

    hp = hpDrugCount(client.snapshot);
    mp = mpDrugCount(client.snapshot);
    if (hp < departureHp || mp < departureMp) {
      const details = [hp < departureHp ? `HP ${departureHp}` : null, mp < departureMp ? `MP ${departureMp}` : null]
        .filter(Boolean).join(' and ');
      throw new Error(`needsFunds: unable to restock supplies to ${details}`);
    }

    const returnMap = returnMapFileName == null ? originalMap : String(returnMapFileName);
    if (returnMap && returnMap !== '0') {
      await travel(returnMap);
      await refreshSnapshot(client, 'post-restock return worldSnapshot');
    }
    return {
      status: 'restocked',
      hp: hpDrugCount(client.snapshot),
      mp: mpDrugCount(client.snapshot),
      restockCount,
      ...(returnMap && returnMap !== '0' ? { returnMap } : {}),
    };
  };
}

export function hpDrugCount(snapshot) {
  return itemQuantity(snapshot, item => /^\(HP\)Drug/i.test(String(item?.name ?? item?.key ?? '')));
}

export function mpDrugCount(snapshot) {
  return itemQuantity(snapshot, item => /^\(MP\)Drug/i.test(String(item?.name ?? item?.key ?? '')));
}

export function canResumeStockedCombatExpedition(snapshot) {
  const actor = snapshotPlayer(snapshot);
  const activeQ54 = (snapshot?.questLog ?? []).some(quest =>
    Number(quest?.questId) === 54 && normalizedStage(quest?.stage) === 'inprogress');
  const className = String(actor?.class ?? '').trim().toLowerCase();
  const casterStocked = !['wizard', 'taoist'].includes(className) || mpDrugCount(snapshot) >= 4;
  return activeQ54 && ['warrior', 'wizard', 'taoist'].includes(className) &&
    healthRatio(snapshot) > 0.45 && hpDrugCount(snapshot) >= 24 && casterStocked;
}

export function canFinishNearCompleteKillQuestWithoutHpStock(snapshot, questId, {
  minimumHealthRatio = 0.9,
  maximumRemainingKills = 2,
} = {}) {
  if (hpDrugCount(snapshot) !== 0 || healthRatio(snapshot) < Number(minimumHealthRatio)) return false;
  const remaining = remainingPureKillQuestObjectives(snapshot, questId);
  return remaining != null && remaining > 0 && remaining <= Number(maximumRemainingKills);
}

export function canFinishNearCompleteKillQuestWithReducedHpStock(snapshot, questId, {
  minimumHealthRatio = 0.9,
  potionsPerRemainingKill = 4,
  maximumRemainingKills = 2,
} = {}) {
  if (healthRatio(snapshot) < Number(minimumHealthRatio)) return false;
  const remaining = remainingPureKillQuestObjectives(snapshot, questId);
  return remaining != null && remaining > 0 && remaining <= Number(maximumRemainingKills) &&
    hpDrugCount(snapshot) >= remaining * Number(potionsPerRemainingKill);
}

export function journeyResumeDisposition(snapshot, {
  dangerDistance = 7,
  readyHealthRatio = 0.75,
  oneHitHealthRatio = 0.1,
} = {}) {
  const readyQuest = (snapshot?.questLog ?? []).find(quest =>
    normalizedStage(quest?.stage) === 'readytoturnin');
  if (readyQuest) return { status: 'readyToTurnIn', questId: Number(readyQuest.questId) };
  const actor = snapshotPlayer(snapshot);
  if (!actor) return { status: 'ready', adjacent: 0, nearby: 0 };
  const hostiles = livingHostiles(snapshot);
  const distances = hostiles.map(entity => chebyshev(actor, entity));
  const adjacent = distances.filter(distance => distance <= 1).length;
  const nearby = distances.filter(distance => distance < dangerDistance).length;
  const ratio = healthRatio(snapshot);
  if (nearby === 0 || ratio >= readyHealthRatio) return { status: 'ready', adjacent, nearby };
  if (ratio <= oneHitHealthRatio) return { status: 'awaitDeath', adjacent, nearby };
  return { status: 'evade', adjacent, nearby };
}

export function questNeedsPostRetreatRecovery(snapshot, questId) {
  const quest = (snapshot?.questLog ?? []).find(entry => Number(entry?.questId) === Number(questId));
  return normalizedStage(quest?.stage) === 'inprogress';
}

export async function waitForPassiveHealthRecovery(client, {
  requiredRatio = 0.75,
  timeoutMs = 90_000,
  pollMs = PASSIVE_RECOVERY_POLL_MS,
  sleep = defaultSleep,
  now = Date.now,
} = {}) {
  requireSnapshot(client);
  const startedAt = now();
  let refreshes = 0;
  while (healthRatio(client.snapshot) < requiredRatio) {
    const elapsed = now() - startedAt;
    if (elapsed + pollMs > timeoutMs) throw new Error('Passive HP recovery timed out');
    await sleep(pollMs);
    await refreshSnapshot(client, 'passive HP recovery worldSnapshot');
    refreshes += 1;
  }
  const { hp, maxHp } = health(client.snapshot);
  return { status: 'recovered', hp, maxHp, refreshes };
}

export async function recoverHealthWhileEvading(client, navigateNear, {
  requiredRatio = 0.75,
  timeoutMs = 45_000,
  pollMs = PASSIVE_RECOVERY_POLL_MS,
  dangerPollMs = pollMs,
  retreatSteps = 6,
  dangerDistance = 7,
  maxEvasiveMoves = 4,
  sustainCadenceMs = 6_000,
  sustain = null,
  sleep = defaultSleep,
  now = Date.now,
} = {}) {
  requireSnapshot(client);
  if (typeof navigateNear !== 'function') throw new TypeError('navigateNear must be a function');
  const startedAt = now();
  let refreshes = 0;
  let evasiveMoves = 0;
  let candidateRotation = 0;
  let lastSustainAt = Number.NEGATIVE_INFINITY;

  recovery: while (healthRatio(client.snapshot) < requiredRatio) {
    assertLivingPlayer(client.snapshot);
    if (now() - startedAt >= timeoutMs) throw new Error('Evasive HP recovery timed out');
    const actor = snapshotPlayer(client.snapshot);
    if (!actor) throw new Error('Evasive HP recovery has no authoritative player');
    const dangerous = livingHostiles(client.snapshot)
      .filter(entity => chebyshev(actor, entity) < dangerDistance)
      .sort((left, right) => chebyshev(actor, left) - chebyshev(actor, right) ||
        Number(left?.objectId ?? 0) - Number(right?.objectId ?? 0));

    if (dangerous.length > 0) {
      const offsets = retreatOffsets(retreatSteps);
      const allowedHostileObjectIds = dangerous.map(entity => Number(entity.objectId))
        .filter(Number.isSafeInteger);
      let successfulMovesThisPoll = 0;
      for (let index = 0; index < offsets.length && successfulMovesThisPoll < maxEvasiveMoves; index += 1) {
        const offset = offsets[(index + candidateRotation) % offsets.length];
        const before = { x: Number(actor.x), y: Number(actor.y) };
        const target = { x: before.x + offset.x, y: before.y + offset.y };
        try {
          await navigateNear(target, 0, undefined, {
            hostileAvoidanceRadius: 0,
            maxNoPathRefreshes: 0,
            allowedHostileObjectIds,
          });
          evasiveMoves += 1;
          successfulMovesThisPoll += 1;
          candidateRotation = (candidateRotation + 1) % offsets.length;
          break;
        } catch (error) {
          const after = snapshotPlayer(client.snapshot);
          if (after && (Number(after.x) !== before.x || Number(after.y) !== before.y)) {
            evasiveMoves += 1;
            candidateRotation = (candidateRotation + 1) % offsets.length;
            continue recovery;
          }
        }
      }
    }

    assertLivingPlayer(client.snapshot);
    if (typeof sustain === 'function' && now() - lastSustainAt >= sustainCadenceMs) {
      await sustain(client);
      assertLivingPlayer(client.snapshot);
      lastSustainAt = now();
      if (healthRatio(client.snapshot) >= requiredRatio) break;
    }

    const waitMs = dangerous.length > 0 ? dangerPollMs : pollMs;
    if (now() - startedAt + waitMs > timeoutMs) throw new Error('Evasive HP recovery timed out');
    await sleep(waitMs);
    await refreshSnapshot(client, 'evasive HP recovery worldSnapshot');
    refreshes += 1;
  }

  const { hp, maxHp } = health(client.snapshot);
  return { status: 'recovered', hp, maxHp, refreshes, evasiveMoves };
}

function assertLivingPlayer(snapshot) {
  const actor = snapshotPlayer(snapshot);
  if (!actor || actor.dead === true || Number(actor.hp) <= 0 || Number(snapshot?.playerHp) <= 0) {
    throw new Error('Player died during quest combat');
  }
  return actor;
}

function remainingPureKillQuestObjectives(snapshot, questId) {
  const quest = (snapshot?.questLog ?? []).find(entry => Number(entry?.questId) === Number(questId));
  if (!quest || normalizedStage(quest.stage) !== 'inprogress') return null;
  const objectives = Array.isArray(quest.objectives) ? quest.objectives : [];
  if (objectives.length === 0 || objectives.some(objective =>
    !String(objective?.label ?? '').trim().toLowerCase().startsWith('kill '))) return null;
  return objectives.reduce((total, objective) => total + Math.max(
    0,
    Number(objective?.required ?? 0) - Number(objective?.current ?? 0),
  ), 0);
}

async function refreshSnapshot(client, label) {
  const afterSequence = Number(client.sequence ?? 0);
  client.send({ type: 'clientVersion' });
  await client.wait(
    () => (client.events ?? []).some(event => event?.sequence > afterSequence &&
      event?.direction === 'received' && event?.type === 'worldSnapshot'),
    label,
    REFRESH_TIMEOUT_MS,
  );
}

function retreatOffsets(distance) {
  const radius = positiveInteger(distance, 'retreatSteps');
  const half = Math.max(1, Math.floor(radius / 2));
  return [
    { x: -radius, y: radius },
    { x: radius, y: radius },
    { x: radius, y: -radius },
    { x: -radius, y: -radius },
    { x: -radius, y: 0 },
    { x: radius, y: 0 },
    { x: 0, y: radius },
    { x: 0, y: -radius },
    { x: -radius, y: half },
    { x: radius, y: half },
    { x: -radius, y: -half },
    { x: radius, y: -half },
    { x: -half, y: radius },
    { x: half, y: radius },
    { x: -half, y: -radius },
    { x: half, y: -radius },
  ];
}

function itemQuantity(snapshot, predicate) {
  let total = 0;
  for (const item of [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])]) {
    if (!predicate(item)) continue;
    const quantity = Number(item?.quantity ?? item?.count ?? 1);
    if (Number.isFinite(quantity) && quantity > 0) total += quantity;
  }
  return total;
}

function snapshotPlayer(snapshot) {
  return (snapshot?.entities ?? []).find(entity =>
    Number(entity?.objectId) === Number(snapshot?.playerObjectId)) ?? null;
}

function livingHostiles(snapshot) {
  return (snapshot?.entities ?? []).filter(entity =>
    String(entity?.kind ?? '').trim().toLowerCase() === 'monster' &&
    String(entity?.disposition ?? 'hostile').trim().toLowerCase() === 'hostile' &&
    entity?.dead !== true && Number(entity?.hp ?? 1) > 0 &&
    Number.isFinite(Number(entity?.x)) && Number.isFinite(Number(entity?.y)));
}

function chebyshev(left, right) {
  return Math.max(Math.abs(Number(left.x) - Number(right.x)), Math.abs(Number(left.y) - Number(right.y)));
}

function health(snapshot) {
  const actor = snapshotPlayer(snapshot);
  return {
    hp: Number(snapshot?.playerHp ?? actor?.hp ?? 0),
    maxHp: Number(snapshot?.playerMaxHp ?? actor?.maxHp ?? 0),
  };
}

function healthRatio(snapshot) {
  const { hp, maxHp } = health(snapshot);
  return hp > 0 && maxHp > 0 ? hp / maxHp : 0;
}

function normalizedStage(value) {
  return String(value ?? '').replace(/[^a-z]/gi, '').toLowerCase();
}

function requireSnapshot(client) {
  if (!client?.snapshot) throw new Error('authoritative snapshot is required');
}

function positiveInteger(value, label) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new TypeError(`${label} must be a positive integer`);
  return parsed;
}

function nonnegativeInteger(value, label) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new TypeError(`${label} must be a non-negative integer`);
  return parsed;
}

function defaultSleep(milliseconds) {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}
