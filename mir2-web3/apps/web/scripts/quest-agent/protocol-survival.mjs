const DEFAULT_MAX_RESTOCKS = 32;
const DEFAULT_MINIMUM_HP_STOCK = 1;
const REFRESH_TIMEOUT_MS = 12_000;
const PASSIVE_RECOVERY_POLL_MS = 3_100;
const DANGEROUS_EXPEDITION_QUEST_IDS = new Set([54, 60, 62, 65, 98, 99, 113, 114]);

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

/** A Taoist should not resume a dangerous kill expedition without spell fuel. */
export function requiresTaoistAmuletRestock(snapshot, questId, className, minimum = 1) {
  const normalizedClass = String(className ?? '').trim().toLowerCase();
  if (normalizedClass !== 'taoist' || !DANGEROUS_EXPEDITION_QUEST_IDS.has(Number(questId))) return false;
  if (!(snapshot?.knownSkills ?? []).some(skill => String(skill?.spell ?? '') === 'SoulFireBall')) return false;
  const required = nonnegativeInteger(minimum, 'minimum Amulet stock');
  return amuletStock(snapshot) < required;
}

/** Full village departure stock; intentionally separate from the field trigger above. */
export function journeyMpRestockTargetForQuest(questId, className, {
  fallback = 12,
} = {}) {
  const normalizedClass = String(className ?? '').trim().toLowerCase();
  if (!['wizard', 'taoist'].includes(normalizedClass)) return 0;
  if ([54, 65].includes(Number(questId))) return 80;
  if (Number(questId) === 42 && normalizedClass === 'wizard') return 32;
  return nonnegativeInteger(fallback, 'fallback MP target');
}

/**
 * Minimum useful stock after a partial village shop pass. The full targets
 * remain 80 so a funded character still buys the preferred reserve, while a
 * nearly stocked character does not enter a fragile funding loop for the last
 * few bottles.
 */
export function journeyExpeditionDepartureFloorForQuest(questId, className) {
  const id = Number(questId);
  const normalizedClass = String(className ?? '').trim().toLowerCase();
  const expedition = DANGEROUS_EXPEDITION_QUEST_IDS.has(id);
  return {
    hp: expedition ? 64 : 0,
    mp: [54, 65].includes(id) && ['wizard', 'taoist'].includes(normalizedClass) ? 64 : 0,
  };
}

/** Begin drinking before a long caster objective can exhaust the active pool. */
export function questCombatMpUseThresholdForQuest(questId, className, {
  fallback = 0.3,
} = {}) {
  const normalizedClass = String(className ?? '').trim().toLowerCase();
  if ([54, 65].includes(Number(questId)) && ['wizard', 'taoist'].includes(normalizedClass)) return 0.75;
  const value = Number(fallback);
  return Number.isFinite(value) ? Math.max(0, Math.min(1, value)) : 0.3;
}

export function questPostRetreatRecoveryRatio(questId, className) {
  const normalizedClass = String(className ?? '').trim().toLowerCase();
  return [54, 65].includes(Number(questId)) && ['wizard', 'taoist'].includes(normalizedClass)
    ? 0.65
    : 0.75;
}

export function questEmergencyEscapeHpRatio(questId, className = '') {
  const id = Number(questId);
  const normalizedClass = String(className).trim().toLowerCase();
  // The live q60 Wizard lost 24 of its 72 HP across two consecutive shared
  // snapshots while eight monsters had closed to six tiles. At the generic
  // 35% threshold the UseItem request began at 7 HP and lost the race with
  // the next hit. Escape before that measured two-hit window.
  // R85 measured q98 collapsing from 83/90 to 26/90 between two navigation
  // policy polls. Start the escape after the first effective hit so an unlucky
  // random landing still leaves enough HP to survive the item cooldown.
  if ([98, 99].includes(id) && normalizedClass === 'wizard') return 0.9;
  // R76 repeatedly found only 0/2 and 1/1 Dung groups in D022. The Taoist can
  // take those measured pulls with Healing, but must escape before the pack
  // reaches the generic one-third-health floor.
  if ([98, 99].includes(id) && normalizedClass === 'taoist') return 0.65;
  if ([60, 65, 113].includes(id) && normalizedClass === 'wizard') return 0.65;
  return DANGEROUS_EXPEDITION_QUEST_IDS.has(id) ? 0.35 : 0;
}

export function journeyNavigationEmergencyEscapeBudget(className = '') {
  // A 72 HP q60 Wizard can still land inside another live pack. The R63 trace
  // exhausted the generic two-relocation cap, then died with one ordinary
  // RandomTeleport still in inventory. Let the fragile class consume the full
  // four-scroll departure reserve when successive authoritative landings stay unsafe.
  // R79 completed q114, then died with six scrolls still carried because two
  // successive random landings both remained inside dense D715 packs and the
  // old non-Wizard navigation cap refused a third use. Four stays bounded and
  // preserves half of the eight-scroll expedition stock for combat recovery.
  return 4;
}

/** HP band where a fragile caster may reuse a public escape after 3 seconds. */
export function journeyEmergencyTeleportCriticalHpRatio(className = '') {
  const normalizedClass = String(className).trim().toLowerCase();
  // R94 reached 52/90 HP after one Wooma exchange, but the shared 12-second
  // cooldown deferred escape until 17/90. Two already-scheduled Crystal melee
  // hits then landed after the relocation and killed the player. Crystal keeps
  // those delayed same-map hits valid, so fragile casters must leave before
  // their remaining HP falls below the measured queued-damage window.
  return ['wizard', 'taoist'].includes(normalizedClass) ? 0.65 : 0.35;
}

/** Public-shop departure reserve measured for each dangerous route. */
export function journeyEmergencyTeleportDepartureTarget(questId) {
  const id = Number(questId);
  if (!DANGEROUS_EXPEDITION_QUEST_IDS.has(id)) return 0;
  // The R89 Wooma crossing consumed the full generic four-scroll reserve
  // before D022. R78 then consumed all four in successive unsafe Stone Tomb
  // landings while q114 still had one kill left. Eight preserves the bounded
  // navigation budget plus a complete combat escape reserve after arrival.
  return [98, 99, 114].includes(id) ? 8 : 4;
}

/** Keep the public escape reserve usable until a dangerous expedition is handed in. */
export function hasJourneyEmergencyEscapeQuest(snapshot) {
  return (snapshot?.questLog ?? []).some(quest =>
    DANGEROUS_EXPEDITION_QUEST_IDS.has(Number(quest?.questId)) &&
    ['inprogress', 'readytoturnin'].includes(normalizedStage(quest?.stage)));
}

const LONG_EVASIVE_RECOVERY_QUESTS = new Set([30, 33, 36, 42, 49, 54, 60, 62, 65, 98, 99, 113, 114]);
const WIDE_DANGER_RECOVERY_QUESTS = new Set([42, 60, 62, 65, 98, 99, 113, 114]);

export function evasiveRecoveryTimeoutMsForQuest(questId) {
  return LONG_EVASIVE_RECOVERY_QUESTS.has(Number(questId)) ? 90_000 : 45_000;
}

export function evasiveRecoveryDangerDistanceForQuest(questId) {
  return WIDE_DANGER_RECOVERY_QUESTS.has(Number(questId)) ? 8 : 7;
}

export function questRetreatProfile(questId, className = '') {
  const id = Number(questId);
  const normalizedClass = String(className).trim().toLowerCase();
  const fragileMineExpedition = [54, 65].includes(id) && ['wizard', 'taoist'].includes(normalizedClass);
  const healingMineExpedition = [54, 65].includes(id) && normalizedClass === 'taoist';
  const rangedInsectExpedition = [60, 113].includes(id) && ['wizard', 'taoist'].includes(normalizedClass);
  const rangedWoomaExpedition = [98, 99].includes(id) && ['wizard', 'taoist'].includes(normalizedClass);
  const rangedMinePassage = id === 65 && fragileMineExpedition;
  const warriorWoomaHunt = id === 99 && normalizedClass === 'warrior';
  const warriorMineralMineHunt = id === 118 && normalizedClass === 'warrior';
  return {
    allowLowHealthFollowerRecovery: [30, 33, 36, 49, 54, 60, 62, 65, 98, 99, 113, 114].includes(id),
    // R27 showed that stopping a stocked q54 caster on the first glancing hit
    // turns the D401 crossing into an unrewarded fight: the surrounding pack
    // converges while Wizard/Taoist spend the MP reserved for D406. Keep the
    // normal low-health interrupt below, but let every stocked q54 class keep
    // moving while it remains above that class's retreat threshold.
    // The map1 approach to D2041 is also long enough for incidental Omas to
    // trail a ranged q60 player. Stopping at full health to evade that weak
    // field pack repeatedly returned the Taoist to the start of the same edge.
    // q98/q99 use the same long map1 approach as the insect expeditions. R83
    // returned to town before reaching D021 after stopping for ordinary field
    // mobs even though it remained healthy and fully supplied. Keep crossing
    // the approach while healthy; the existing cave target-density and
    // low-health escape gates still take over after arrival.
    continueTravelWhileHealthy: [54, 62, 65].includes(id) ||
      rangedInsectExpedition || rangedWoomaExpedition,
    // R37-R42 showed that repeated evasion spends the random-teleport reserve
    // without crossing D401/D2041. These expeditions carry large proven
    // potion stocks, so clear a monster that actually hit the player before
    // retrying the route. RandomTeleport remains available at critical HP.
    preferTravelAggressorCombat: [54, 62, 65].includes(id),
    maxTravelThreatEvasionsPerEdge: [62, 65].includes(id) ? 3 : 1,
    // R26 proved that a Taoist can be boxed into D401 by one CaveMaggot plus
    // adjacent quest zombies after the first breakout kill. Healing and the
    // expedition potion reserve make two further bounded kills safer than
    // aborting a living route in that sealed pocket. Keep the lower Wizard
    // limit because the same trace band showed its much smaller HP pool can
    // die during a single corridor breakout.
    maxRetreatBreakoutKills: ([54, 62, 65].includes(id) && !fragileMineExpedition) || healingMineExpedition ? 3 : 1,
    multiAggressorRetreatRatio: [54, 65].includes(id) ? (fragileMineExpedition ? 0.75 : 0.55) : undefined,
    retreatAtActiveAggressorCount: [54, 65].includes(id) ? (fragileMineExpedition ? 1 : 3) : undefined,
    unsafeRetreatSteps: id === 62 || fragileMineExpedition ? 24 :
      ([30, 33, 36].includes(id) ? 16 : 12),
    unsafeRetreatSafeDistance: id === 62 || fragileMineExpedition ? 8 :
      ([30, 33, 36].includes(id) ? 10 : 6),
    // Live D2041 snapshots consistently expose SpiderFrog candidates with one
    // adjacent and up to four nearby passive monsters. Rejecting every such
    // candidate made both ranged classes clear unrelated monsters and leave
    // the cave without quest credit. R74 then measured the same bounded 1/2
    // shape around q65's D421 passage blockers; its strict 0/1 limit produced
    // no spells and only consumed escape scrolls. Admit the already proven 1/4
    // density for both ranged passages; the proven-aggressor and health gates
    // still interrupt an unsafe engagement.
    ...((rangedInsectExpedition || rangedWoomaExpedition || rangedMinePassage) ? {
      maxTargetAdjacent: 1,
      maxTargetNearby: 4,
    } : warriorWoomaHunt ? {
      // R61 proved the same final WoomaFighter closes from 1/2 to 2/3 while
      // the full-health Warrior approaches. The target was already down to
      // 53/150 HP and q99's bounded focus policy keeps that exact target, so
      // admit the measured closing pack long enough to land the final blows.
      maxTargetAdjacent: 2,
      maxTargetNearby: 3,
    } : warriorMineralMineHunt ? {
      // R79 completed the isolated HungryZombie pulls, then every remaining
      // CursedZombie source settled into the same measured 2-adjacent/5-nearby
      // D2031 pack. The level-28 Warrior remained above 80% HP while repeatedly
      // retreating from those candidates. Admit that exact density so the
      // normal proven-aggressor clearing loop can open the pack.
      maxTargetAdjacent: 2,
      maxTargetNearby: 5,
    } : {}),
    ...(rangedWoomaExpedition ? {
      // R97 reached a FlamingWooma through the zero-clearance corridor and
      // reduced it to one percent before a joining pack forced target loss.
      // A healthy ranged character should land the bounded final cast rather
      // than discard all of that public-protocol combat progress.
      focusTargetThroughAggressors: true,
      finishableTargetHealthRatio: 0.1,
      finishableTargetMinimumPlayerHpRatio: 0.7,
    } : {}),
  };
}

/** Minimum passive-monster clearance for the second spawn-search path. */
export function questSpawnSearchHostileClearanceFallback(questId) {
  const id = Number(questId);
  if (id === 30) return 2;
  // R95 enumerated all 677 D022 FlamingWooma waypoints from the entrance and
  // rejected every one because a one-cell passive-monster halo disconnected
  // the cave. Target density is checked separately and a proven hit interrupts
  // navigation, so a zero-clearance fallback permits the ordinary corridor
  // without weakening combat survival limits.
  if ([54, 98, 99].includes(id)) return 0;
  return 1;
}

export function questRetreatBiasPosition(questId, snapshot, className = '') {
  const id = Number(questId);
  const mapFileName = String(snapshot?.mapFileName ?? '');
  // q62 has no objective source on D2041; its reachable source is D2042.
  // Retreating toward the D2041 entrance erased the entire crossing after
  // every pack. Preserve safe forward progress toward the real 2F transfer.
  if (id === 62 && mapFileName === 'D2041') return { x: 262, y: 13 };
  // The fragile ranged classes use their already proven D406 q54 route for
  // q65. If they resume inside D421, move equal-safety retreats toward the
  // map-2 entrance; the durable Warrior still continues to D422.
  if (id === 65 && mapFileName === 'D421') {
    return ['wizard', 'taoist'].includes(String(className).trim().toLowerCase())
      ? { x: 30, y: 374 }
      : { x: 361, y: 19 };
  }
  if (id === 65 && ['wizard', 'taoist'].includes(String(className).trim().toLowerCase())) {
    return ({
      D401: { x: 76, y: 15 },
      D411: { x: 95, y: 49 },
    })[mapFileName] ?? null;
  }
  if (id !== 54) return null;
  return ({
    D401: { x: 76, y: 15 },
    D411: { x: 95, y: 49 },
    D421: { x: 361, y: 19 },
  })[mapFileName] ?? null;
}

export function shouldPreferObjectiveMapOverCurrent(questId, questState = null, className = '') {
  const id = Number(questId);
  const normalizedClass = String(className).trim().toLowerCase();
  // q49's Natural Cave source is intentionally safer than its current-map Oma
  // alternative.
  if (id === 49) return true;
  // R75/R76 spent two long D421 crossings clearing passage zombies and still
  // did not reach D422; the same Wizard already completed q54 through D406,
  // which contains thirty authoritative Zombie1 spawns.
  if (id === 65) return ['wizard', 'taoist'].includes(normalizedClass);
  if (id !== 54) return false;

  // R27 and R45 showed both caster classes repeatedly spending their D406 MP
  // and escape reserves on dense, reward-poor D401 packs. They can complete
  // every q54 zombie variant in D406, so treat D401 as a transit layer for
  // Wizard and Taoist while the durable Warrior may still clear it directly.
  if (questState && ['wizard', 'taoist'].includes(normalizedClass)) return true;

  // D401 is useful while q54 still needs its Zombie2/3/4/5 population, so an
  // unconditional D406 preference would create a long reward-free crossing.
  // Once Zombie1 is the sole unfinished objective for any class, however, R46 proved that
  // returning from D406 to D401 exposes the player to a dense shared pack and
  // can consume the entire escape reserve without progress. Finish that tail
  // in the configured D406 source instead.
  const pending = (questState?.objectives ?? []).filter(objective => {
    const current = Number(objective?.current ?? 0);
    const required = Number(objective?.required ?? 0);
    return required > 0 && current < required;
  });
  return pending.length === 1 && String(pending[0]?.label ?? '').trim().toLowerCase() === 'kill zombie1';
}

export function preferredObjectiveMapsForQuest(questId, className = '') {
  const id = Number(questId);
  if (id === 49) return ['D011'];
  if (id === 54) return ['D406'];
  if (id === 65 && ['wizard', 'taoist'].includes(String(className).trim().toLowerCase())) {
    return ['D406'];
  }
  return [];
}

export function createPostEngagementSupplyGate({
  travel,
  navigateNear,
  restock,
  minimumHpStock = DEFAULT_MINIMUM_HP_STOCK,
  minimumMpStock = 0,
  minimumAmuletStock = 0,
  maxRestocks = DEFAULT_MAX_RESTOCKS,
} = {}) {
  if (typeof travel !== 'function') throw new TypeError('travel must be a function');
  if (typeof navigateNear !== 'function') throw new TypeError('navigateNear must be a function');
  if (typeof restock !== 'function') throw new TypeError('restock must be a function');
  const defaultHp = nonnegativeInteger(minimumHpStock, 'minimumHpStock');
  const defaultMp = nonnegativeInteger(minimumMpStock, 'minimumMpStock');
  const defaultAmulet = nonnegativeInteger(minimumAmuletStock, 'minimumAmuletStock');
  const restockLimit = positiveInteger(maxRestocks, 'maxRestocks');
  let restockCount = 0;

  return async function ensureSupplies(client, {
    minimumHpStock: requestedHp = defaultHp,
    minimumMpStock: requestedMp = defaultMp,
    minimumAmuletStock: requestedAmulet = defaultAmulet,
    requiredAfterRestockHpStock = requestedHp,
    requiredAfterRestockMpStock = requestedMp,
    requiredAfterRestockAmuletStock = requestedAmulet,
    requiredAfterRestockEmergencyTeleportStock: requestedEmergencyTeleport = 0,
    minimumEmergencyTeleportStock: requestedEmergencyTeleportTrigger = requestedEmergencyTeleport,
    returnMapFileName = null,
    forceRestock = false,
  } = {}) {
    requireSnapshot(client);
    const triggerHp = nonnegativeInteger(requestedHp, 'minimumHpStock');
    const triggerMp = nonnegativeInteger(requestedMp, 'minimumMpStock');
    const triggerAmulet = nonnegativeInteger(requestedAmulet, 'minimumAmuletStock');
    const departureHp = nonnegativeInteger(requiredAfterRestockHpStock, 'requiredAfterRestockHpStock');
    const departureMp = nonnegativeInteger(requiredAfterRestockMpStock, 'requiredAfterRestockMpStock');
    const departureAmulet = nonnegativeInteger(
      requiredAfterRestockAmuletStock,
      'requiredAfterRestockAmuletStock',
    );
    const departureEmergencyTeleport = nonnegativeInteger(
      requestedEmergencyTeleport,
      'requiredAfterRestockEmergencyTeleportStock',
    );
    const triggerEmergencyTeleport = nonnegativeInteger(
      requestedEmergencyTeleportTrigger,
      'minimumEmergencyTeleportStock',
    );
    let hp = hpDrugCount(client.snapshot);
    let mp = mpDrugCount(client.snapshot);
    let amulet = amuletStock(client.snapshot);
    let emergencyTeleport = emergencyTeleportStock(client.snapshot);
    if (!forceRestock && hp >= triggerHp && mp >= triggerMp && amulet >= triggerAmulet &&
        emergencyTeleport >= triggerEmergencyTeleport) {
      return {
        status: 'sufficient',
        hp,
        mp,
        ...(triggerAmulet > 0 ? { amulet } : {}),
        restockCount,
      };
    }
    if (restockCount >= restockLimit) {
      throw new Error(`restock limit exceeded (${restockCount}/${restockLimit})`);
    }

    const originalMap = String(client.snapshot.mapFileName ?? '');
    if (originalMap !== '0') await travel('0');
    restockCount += 1;
    const targetHp = Math.max(triggerHp, departureHp);
    const targetMp = Math.max(triggerMp, departureMp);
    const targetAmulet = Math.max(triggerAmulet, departureAmulet);
    const supplyOptions = { targetHp };
    if (targetMp > 0) supplyOptions.targetMp = targetMp;
    if (targetAmulet > 0) supplyOptions.targetAmulet = targetAmulet;
    supplyOptions.lowStockHp = targetHp;
    if (targetMp > 0) supplyOptions.lowStockMp = targetMp;
    if (targetAmulet > 0) supplyOptions.lowStockAmulet = targetAmulet;
    const restockResult = await restock(client, navigateNear, supplyOptions);

    hp = hpDrugCount(client.snapshot);
    mp = mpDrugCount(client.snapshot);
    amulet = amuletStock(client.snapshot);
    emergencyTeleport = emergencyTeleportStock(client.snapshot);
    if (hp < departureHp || mp < departureMp || amulet < departureAmulet ||
        emergencyTeleport < departureEmergencyTeleport) {
      const details = [
        hp < departureHp ? `HP ${departureHp}` : null,
        mp < departureMp ? `MP ${departureMp}` : null,
        amulet < departureAmulet ? `Amulet ${departureAmulet}` : null,
        emergencyTeleport < departureEmergencyTeleport
          ? `RandomTeleport ${departureEmergencyTeleport}`
          : null,
      ]
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
      ...(targetAmulet > 0 ? { amulet: amuletStock(client.snapshot) } : {}),
      restockCount,
      ...(returnMap && returnMap !== '0' ? { returnMap } : {}),
    };
  };
}

export function hpDrugCount(snapshot) {
  return itemQuantity(snapshot, item => /^\(HP\)Drug/i.test(String(item?.name ?? item?.key ?? '')));
}

export function isLivingEvasiveRecoveryTimeout(snapshot, error) {
  return healthRatio(snapshot) > 0 &&
    String(error?.message ?? error) === 'Evasive HP recovery timed out';
}

export function isLivingUnsafePackRetreatFailure(snapshot, error) {
  return healthRatio(snapshot) > 0 &&
    String(error?.message ?? error).startsWith('unsafe hostile pack retreat failed from ');
}

/** A live AOI target disappearing or being stranded by an escape is a normal world race. */
export function isLivingLostCombatTarget(snapshot, error) {
  return healthRatio(snapshot) > 0 &&
    /^target \d+ (?:left the authoritative snapshot before (?:attack|death was confirmed)|has no walk path from the current map region)$/i
      .test(String(error?.message ?? error));
}

/** Dense expedition occupants are transient; keep the authoritative field position and replan. */
export function isLivingExpeditionNoWalkPath(snapshot, questId, error) {
  return healthRatio(snapshot) > 0 &&
    DANGEROUS_EXPEDITION_QUEST_IDS.has(Number(questId)) &&
    String(error?.message ?? error).startsWith('No walk path');
}

export function requiresExpeditionEscapeRestock(snapshot, questId, minimum = 1) {
  return DANGEROUS_EXPEDITION_QUEST_IDS.has(Number(questId)) &&
    emergencyTeleportStock(snapshot) < nonnegativeInteger(minimum, 'emergency teleport minimum');
}

/** Departure-only stock must not block a completed objective that is already safe to turn in. */
export function journeyExpeditionSupplyActive(snapshot, questId) {
  if (!DANGEROUS_EXPEDITION_QUEST_IDS.has(Number(questId))) return false;
  const quest = (snapshot?.questLog ?? []).find(entry =>
    Number(entry?.questId) === Number(questId));
  return normalizedStage(quest?.stage) === 'inprogress';
}

/** Keep a partly used field reserve, but leave for town once no escape remains. */
export function journeyEmergencyEscapeRestockTarget(snapshot, questId, {
  force = false,
  target = 4,
} = {}) {
  if (!journeyExpeditionSupplyActive(snapshot, questId)) return 0;
  const required = nonnegativeInteger(target, 'emergency teleport target');
  return force || String(snapshot?.mapFileName ?? '') === '0' || emergencyTeleportStock(snapshot) === 0
    ? required
    : 0;
}

export function mpDrugCount(snapshot) {
  return itemQuantity(snapshot, item => /^\(MP\)Drug/i.test(String(item?.name ?? item?.key ?? '')));
}

export function amuletStock(snapshot) {
  return itemQuantity(snapshot, item =>
    String(item?.name ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'amulet' ||
    String(item?.key ?? '').toLowerCase() === 'crystal-item-712',
  { includeEquipment: true });
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
  emergencyEscape = null,
  emergencyEscapeHpRatio = 0,
  maxEmergencyEscapes = 1,
  biasPosition = null,
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
  let emergencyEscapes = 0;
  let emergencyEscapeFailed = false;
  let emergencyEscapeRetryAt = Number.NEGATIVE_INFINITY;
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

    const adjacent = dangerous.filter(entity => chebyshev(actor, entity) <= 1).length;
    const withinThree = dangerous.filter(entity => chebyshev(actor, entity) <= 3).length;
    if (typeof emergencyEscape === 'function' && !emergencyEscapeFailed &&
        now() >= emergencyEscapeRetryAt &&
        emergencyEscapes < Math.max(0, Number(maxEmergencyEscapes) || 0) &&
        healthRatio(client.snapshot) <= Math.max(0, Number(emergencyEscapeHpRatio) || 0) &&
        (adjacent >= 2 || withinThree >= 3)) {
      const before = { x: Number(actor.x), y: Number(actor.y) };
      recordSurvivalDiagnostic(client, {
        type: 'recoveryEmergencyEscapeAttempt',
        hpRatio: healthRatio(client.snapshot),
        adjacent,
        withinThree,
        from: before,
      });
      try {
        const result = await emergencyEscape(client, { current: before, adjacent, withinThree });
        if (result?.deferred === true) {
          emergencyEscapeRetryAt = now() + Math.max(1, Number(result.retryAfterMs) || 1);
          recordSurvivalDiagnostic(client, {
            type: 'recoveryEmergencyEscapeDeferred',
            hpRatio: healthRatio(client.snapshot),
            adjacent,
            withinThree,
            from: before,
            retryAfterMs: Number(result.retryAfterMs ?? 0),
          });
        } else {
          const after = snapshotPlayer(client.snapshot);
          if ((result === true || result?.success === true || result?.to != null) && after &&
              (Number(after.x) !== before.x || Number(after.y) !== before.y)) {
            emergencyEscapes += 1;
            recordSurvivalDiagnostic(client, {
              type: 'recoveryEmergencyEscapeSuccess',
              hpRatio: healthRatio(client.snapshot),
              adjacent,
              withinThree,
              from: before,
              to: { x: Number(after.x), y: Number(after.y) },
            });
            continue recovery;
          }
          emergencyEscapeFailed = true;
        }
      } catch (error) {
        emergencyEscapeFailed = true;
        recordSurvivalDiagnostic(client, {
          type: 'recoveryEmergencyEscapeFailure',
          hpRatio: healthRatio(client.snapshot),
          adjacent,
          withinThree,
          from: before,
          message: String(error?.message ?? error),
        });
      }
    }

    if (dangerous.length > 0) {
      const requestedBias = typeof biasPosition === 'function'
        ? biasPosition(client.snapshot)
        : biasPosition;
      const hasBias = requestedBias && Number.isFinite(Number(requestedBias.x)) &&
        Number.isFinite(Number(requestedBias.y));
      const offsets = retreatOffsets(retreatSteps);
      if (hasBias) {
        offsets.sort((left, right) => {
          const leftTarget = { x: Number(actor.x) + left.x, y: Number(actor.y) + left.y };
          const rightTarget = { x: Number(actor.x) + right.x, y: Number(actor.y) + right.y };
          const leftClearance = Math.min(...dangerous.map(entity => chebyshev(leftTarget, entity)));
          const rightClearance = Math.min(...dangerous.map(entity => chebyshev(rightTarget, entity)));
          return rightClearance - leftClearance ||
            chebyshev(leftTarget, requestedBias) - chebyshev(rightTarget, requestedBias);
        });
      }
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

function recordSurvivalDiagnostic(client, payload) {
  if (typeof client?.record === 'function') client.record('diagnostic', payload);
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

function itemQuantity(snapshot, predicate, { includeEquipment = false } = {}) {
  let total = 0;
  const items = [
    ...(snapshot?.inventoryItems ?? []),
    ...(snapshot?.beltItems ?? []),
    ...(includeEquipment ? (snapshot?.equipmentItems ?? []) : []),
  ];
  for (const item of items) {
    if (!predicate(item)) continue;
    const quantity = Number(item?.quantity ?? item?.count ?? 1);
    if (Number.isFinite(quantity) && quantity > 0) total += quantity;
  }
  return total;
}

function emergencyTeleportStock(snapshot) {
  return itemQuantity(snapshot, item =>
    String(item?.name ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'randomteleport' ||
    String(item?.key ?? '').toLowerCase() === 'crystal-item-717');
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
