import { observedPlayerHp, observedEntityHealthRatio } from './protocol-observation.mjs';
const DEFAULT_MAX_RESTOCKS = 32;
const DEFAULT_MINIMUM_HP_STOCK = 1;
const REFRESH_TIMEOUT_MS = 12_000;
const PASSIVE_RECOVERY_POLL_MS = 3_100;
const DANGEROUS_EXPEDITION_QUEST_IDS = new Set([54, 60, 62, 65, 89, 98, 99, 113, 114]);

/**
 * A fresh authoritative safe-zone flag suppresses field-threat reactions. The
 * only exception is a recent explicit player-hit receipt, which keeps this
 * guard compatible with a real PvP attacker if one is ever exposed here.
 */
export function safeZoneBlocksEmergencyEscape(client, {
  now = Date.now,
  withinMs = 5_000,
} = {}) {
  if (client?.snapshot?.inSafeZone !== true) return false;
  const playerObjectId = Number(client.snapshot?.playerObjectId);
  if (!Number.isFinite(playerObjectId)) return true;
  const nowMs = typeof now === 'function' ? Number(now()) : Number(now);
  const windowMs = Math.max(0, Number(withinMs) || 0);
  const mapBoundarySequence = (client.events ?? [])
    .filter(event => event?.direction === 'received' &&
      ['MapChanged', 'MapInformation', 'Revived', 'StartGame'].includes(String(event?.packet ?? '')))
    .reduce((latest, event) => Math.max(latest, Number(event?.sequence) || 0), 0);
  const hasRecentPlayerHit = (client.events ?? []).some(event => {
    if (event?.direction !== 'received' ||
        (Number(event?.sequence) || 0) <= mapBoundarySequence) return false;
    const eventAt = Date.parse(String(event?.at ?? ''));
    if (!Number.isFinite(eventAt) || !Number.isFinite(nowMs) ||
        eventAt > nowMs || nowMs - eventAt > windowMs) return false;
    const objectId = Number(event?.payload?.objectId);
    if (objectId !== playerObjectId) return false;
    if (event?.packet === 'DamageIndicator') return Number(event?.payload?.damage) > 0;
    return event?.packet === 'ObjectStruck' && Number(event?.payload?.attackerId) > 0;
  });
  return !hasRecentPlayerHit;
}

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

/** Spell fuel for at least one measured full-health expedition target. */
export function journeyAmuletSupplyPolicyForQuest(questId, className, snapshot = null) {
  const id = Number(questId);
  if (String(className ?? '').trim().toLowerCase() !== 'taoist' ||
      !DANGEROUS_EXPEDITION_QUEST_IDS.has(id)) return { minimum: 0, departure: 0 };
  // R97 measured 7 damage per SoulFireBall against a 285 HP WoomaSoldier.
  // Thirty-two casts cannot complete that full-health pull. Reserve forty-
  // eight before acquisition and leave town with one hundred for misses and
  // the next two short encounters; other expeditions retain their proven budget.
  // R125's q89 Taoist field run consumed 24 Amulet from a 32-item departure
  // stack before returning through town. Keep q89's lower field trigger at
  // 32 and fund the same 100-item departure reserve used by q98/q99.
  if (id === 89) return { minimum: 32, departure: 100 };
  if (![98, 99].includes(id)) return { minimum: 12, departure: 32 };
  let minimum = 48;
  // Do not abandon q98's final measured WoomaSoldier after a reconnect when
  // the existing stack can finish its visible wound. This never funds a new
  // full-health pull, and the town departure reserve remains one hundred.
  if (id === 98 && remainingPureKillQuestObjectives(snapshot, id) === 1 &&
      healthRatio(snapshot) >= 0.9) {
    const actor = snapshotPlayer(snapshot);
    const wounded = (snapshot?.entities ?? []).filter(entity =>
      entity?.kind === 'monster' && entity?.dead !== true &&
      String(entity.name).toLowerCase() === 'woomasoldier' &&
      Number(entity.maxHp) === 285 && actor && chebyshev(actor, entity) <= 6 &&
      observedEntityHealthRatio(entity) <= 0.25);
    for (const target of wounded) {
      const conservativeHp = Math.min(285, Math.ceil((observedEntityHealthRatio(target) + 0.01) * 285));
      minimum = Math.min(minimum, Math.ceil(conservativeHp / 7) + 4);
    }
  }
  return { minimum, departure: 100 };
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
    mp: [54, 65].includes(id) && ['wizard', 'taoist'].includes(normalizedClass) ? 64 :
      [98, 99].includes(id) && normalizedClass === 'taoist' ? 12 : 0,
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
  if ([60, 65, 89, 113].includes(id) && normalizedClass === 'wizard') return 0.65;
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
  // R110 consumed all four q89 scrolls before its objective search resumed.
  // Preserve four combat/recovery uses after the bounded navigation allowance.
  return [89, 98, 99, 114].includes(id) ? 8 : 4;
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
  const fragileUndeadMineHunt = id === 89 && normalizedClass === 'wizard';
  const warriorWoomaHunt = id === 99 && normalizedClass === 'warrior';
  const warriorMineralMineHunt = id === 118 && normalizedClass === 'warrior';
  const warriorPrajnaHunt = [122, 123].includes(id) && normalizedClass === 'warrior';
  return {
    allowLowHealthFollowerRecovery: [30, 33, 36, 49, 54, 60, 62, 65, 89, 98, 99, 113, 114, 122, 123].includes(id),
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
    continueTravelWhileHealthy: [54, 62, 65].includes(id) || fragileUndeadMineHunt ||
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
    multiAggressorRetreatRatio: [54, 65].includes(id) ? (fragileMineExpedition ? 0.75 : 0.55) :
      (fragileUndeadMineHunt ? 0.75 : undefined),
    retreatAtActiveAggressorCount: [54, 65].includes(id) ? (fragileMineExpedition ? 1 : 3) :
      (fragileUndeadMineHunt ? 2 : undefined),
    ...(fragileUndeadMineHunt ? { directAggressorDistance: 8 } : {}),
    unsafeRetreatSteps: id === 62 || fragileMineExpedition || fragileUndeadMineHunt ? 24 :
      ([30, 33, 36].includes(id) ? 16 : 12),
    unsafeRetreatSafeDistance: id === 62 || fragileMineExpedition || fragileUndeadMineHunt ? 8 :
      ([30, 33, 36].includes(id) ? 10 : 6),
    // Live D2041 snapshots consistently expose SpiderFrog candidates with one
    // adjacent and up to four nearby passive monsters. Rejecting every such
    // candidate made both ranged classes clear unrelated monsters and leave
    // the cave without quest credit. R74 then measured the same bounded 1/2
    // shape around q65's D421 passage blockers; its strict 0/1 limit produced
    // no spells and only consumed escape scrolls. Admit the already proven 1/4
    // density for both ranged passages; the proven-aggressor and health gates
    // still interrupt an unsafe engagement.
    ...(fragileUndeadMineHunt ? {
      maxTargetAdjacent: 0,
      maxTargetNearby: 2,
    } : (rangedInsectExpedition || rangedWoomaExpedition || rangedMinePassage) ? {
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
    } : warriorPrajnaHunt ? {
      // R87 proved D2061 contains live objective bone monsters omitted from
      // the imported source list. A level-29 Warrior reduced one BoneArcher
      // to 37% before a joining pack switched it to unrelated ghouls. Admit
      // the observed pack and finish that bounded objective while healthy.
      maxTargetAdjacent: 2,
      maxTargetNearby: 5,
    } : {}),
    ...(fragileUndeadMineHunt ? {
      // R115 reached a valid CursedZombie only after clearing the single
      // entrance Shaman. Let the ordinary proven-aggressor pass clear that
      // one blocker; the existing two-aggressor and 75%-HP gates still
      // interrupt a dangerous pull.
      focusTargetThroughAggressors: false,
      // R116 showed the navigator independently treating a passive nearby
      // CursedZombie as a second attacker and Town-teleporting at 53% HP.
      // q89 keeps the normal .65 eligibility threshold, but needs two fresh
      // direct attacker receipts before that navigation escape is used.
      emergencyEscapeRequiresProvenAggressors: true,
      finishableTargetHealthRatio: 0.25,
      finishableTargetMinimumPlayerHpRatio: 0.7,
    } : rangedWoomaExpedition ? {
      // R97 reached a FlamingWooma through the zero-clearance corridor and
      // reduced it to one percent before a joining pack forced target loss.
      // A healthy ranged character should land the bounded final cast rather
      // than discard all of that public-protocol combat progress.
      focusTargetThroughAggressors: true,
      // R106 repeatedly returned to the same WoomaFighter at 19-24% and full
      // player health, then abandoned it when another pack entered the
      // approach window. Preserve that accumulated public-combat progress at
      // the measured 25% boundary; the 70% player-health floor still wins.
      finishableTargetHealthRatio: 0.25,
      finishableTargetMinimumPlayerHpRatio: 0.7,
    } : warriorPrajnaHunt ? {
      focusTargetThroughAggressors: true,
      finishableTargetHealthRatio: 0.5,
      finishableTargetMinimumPlayerHpRatio: 0.65,
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
  minimumHpMediumStock = 0,
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
    minimumHpMediumStock: requestedHpMedium = minimumHpMediumStock,
    minimumMpStock: requestedMp = defaultMp,
    minimumAmuletStock: requestedAmulet = defaultAmulet,
    requiredAfterRestockHpStock = requestedHp,
    requiredAfterRestockHpMediumStock = requestedHpMedium,
    requiredAfterRestockMpStock = requestedMp,
    requiredAfterRestockAmuletStock = requestedAmulet,
    requiredAfterRestockEmergencyTeleportStock: requestedEmergencyTeleport = 0,
    minimumEmergencyTeleportStock: requestedEmergencyTeleportTrigger = requestedEmergencyTeleport,
    requiredAfterRestockEmergencyTownTeleportStock: requestedEmergencyTownTeleport = 0,
    minimumEmergencyTownTeleportStock: requestedEmergencyTownTeleportTrigger = requestedEmergencyTownTeleport,
    returnMapFileName = null,
    forceRestock = false,
  } = {}) {
    requireSnapshot(client);
    const triggerHp = nonnegativeInteger(requestedHp, 'minimumHpStock');
    const triggerHpMedium = nonnegativeInteger(requestedHpMedium, 'minimumHpMediumStock');
    const triggerMp = nonnegativeInteger(requestedMp, 'minimumMpStock');
    const triggerAmulet = nonnegativeInteger(requestedAmulet, 'minimumAmuletStock');
    const departureHp = nonnegativeInteger(requiredAfterRestockHpStock, 'requiredAfterRestockHpStock');
    const departureHpMedium = nonnegativeInteger(requiredAfterRestockHpMediumStock, 'requiredAfterRestockHpMediumStock');
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
    const departureEmergencyTownTeleport = nonnegativeInteger(
      requestedEmergencyTownTeleport,
      'requiredAfterRestockEmergencyTownTeleportStock',
    );
    const triggerEmergencyTownTeleport = nonnegativeInteger(
      requestedEmergencyTownTeleportTrigger,
      'minimumEmergencyTownTeleportStock',
    );
    let hp = hpDrugCount(client.snapshot);
    let hpMedium = hpMediumDrugCount(client.snapshot);
    let mp = mpDrugCount(client.snapshot);
    let amulet = amuletStock(client.snapshot);
    let emergencyTeleport = emergencyTeleportStock(client.snapshot);
    let emergencyTownTeleport = townTeleportStock(client.snapshot);
    if (!forceRestock && hp >= triggerHp && hpMedium >= triggerHpMedium && mp >= triggerMp && amulet >= triggerAmulet &&
        emergencyTeleport >= triggerEmergencyTeleport &&
        emergencyTownTeleport >= triggerEmergencyTownTeleport) {
      return {
        status: 'sufficient',
        hp,
        ...(triggerHpMedium > 0 ? { hpMedium } : {}),
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
    const targetHpMedium = Math.max(triggerHpMedium, departureHpMedium);
    const targetMp = Math.max(triggerMp, departureMp);
    const targetAmulet = Math.max(triggerAmulet, departureAmulet);
    const supplyOptions = { targetHp };
    if (targetHpMedium > 0) supplyOptions.targetHpMedium = targetHpMedium;
    if (targetMp > 0) supplyOptions.targetMp = targetMp;
    if (targetAmulet > 0) supplyOptions.targetAmulet = targetAmulet;
    if (departureEmergencyTownTeleport > 0) {
      supplyOptions.emergencyTownTeleportCount = departureEmergencyTownTeleport;
    }
    supplyOptions.lowStockHp = targetHp;
    if (targetHpMedium > 0) supplyOptions.lowStockHpMedium = triggerHpMedium;
    if (targetMp > 0) supplyOptions.lowStockMp = targetMp;
    if (targetAmulet > 0) supplyOptions.lowStockAmulet = targetAmulet;
    const restockResult = await restock(client, navigateNear, supplyOptions);

    hp = hpDrugCount(client.snapshot);
    hpMedium = hpMediumDrugCount(client.snapshot);
    mp = mpDrugCount(client.snapshot);
    amulet = amuletStock(client.snapshot);
    emergencyTeleport = emergencyTeleportStock(client.snapshot);
    emergencyTownTeleport = townTeleportStock(client.snapshot);
    if (hp < departureHp || mp < departureMp || amulet < departureAmulet ||
        emergencyTeleport < departureEmergencyTeleport ||
        emergencyTownTeleport < departureEmergencyTownTeleport) {
      const details = [
        hp < departureHp ? `HP ${departureHp}` : null,
        mp < departureMp ? `MP ${departureMp}` : null,
        amulet < departureAmulet ? `Amulet ${departureAmulet}` : null,
        emergencyTeleport < departureEmergencyTeleport
          ? `RandomTeleport ${departureEmergencyTeleport}`
          : null,
        emergencyTownTeleport < departureEmergencyTownTeleport
          ? `TownTeleport ${departureEmergencyTownTeleport}`
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
      ...(targetHpMedium > 0 ? { hpMedium: hpMediumDrugCount(client.snapshot), hpMediumReady: hpMedium >= departureHpMedium } : {}),
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

export function hpMediumDrugCount(snapshot) {
  return itemQuantity(snapshot, item => /^\(HP\)DrugMedium/i.test(String(item?.name ?? item?.key ?? '')));
}

/**
 * The Wizard's q89 entrance has a measured five-hit CursedShaman volley.
 * This is deliberately a narrow policy selector: callers still require a
 * fresh direct packet and use the ordinary held-item recovery mechanism.
 */
export function q89WizardFreshCursedShamanRecoveryOptions(snapshot, attacker, className) {
  if (String(className ?? '').trim().toLowerCase() !== 'wizard') return null;
  const activeQ89 = (snapshot?.questLog ?? []).some(entry =>
    Number(entry?.questId) === 89 && normalizedStage(entry?.stage) === 'inprogress');
  if (!activeQ89 || String(attacker?.name ?? '').trim().toLowerCase() !== 'cursedshaman') return null;
  const maximum = Number(snapshot?.playerMaxHp ?? 0);
  const hp = observedPlayerHp(snapshot);
  if (!(maximum > 0) || hp <= 0 || hp / maximum > 0.85 || hpMediumDrugCount(snapshot) <= 0) return null;
  return { hpThreshold: 0.85, preferredHpPotion: 'medium', restorativeReuseDelayMs: 2_500 };
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
  const message = String(error?.message ?? error);
  return healthRatio(snapshot) > 0 &&
    DANGEROUS_EXPEDITION_QUEST_IDS.has(Number(questId)) &&
    (message.startsWith('No walk path') ||
      /^Travel from \S+ to \S+ is blocked by monster \d+$/.test(message));
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

/** q89 Wizard's TownTeleport reserve is a departure check, never a field floor. */
export function journeyEmergencyTownTeleportRestockTarget(snapshot, questId, className, {
  force = false,
  target = 2,
} = {}) {
  if (String(className ?? '').trim().toLowerCase() !== 'wizard' ||
      Number(questId) !== 89 || !journeyExpeditionSupplyActive(snapshot, questId)) return 0;
  const required = nonnegativeInteger(target, 'emergency TownTeleport target');
  return force || String(snapshot?.mapFileName ?? '') === '0' ? required : 0;
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

function townTeleportStock(snapshot) {
  return itemQuantity(snapshot, item =>
    String(item?.name ?? '').replace(/[^a-z]/gi, '').toLowerCase() === 'townteleport' ||
    String(item?.key ?? '').toLowerCase() === 'crystal-item-719',
  );
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
    const safeZoneEscapeBlocked = safeZoneBlocksEmergencyEscape(client, { now });
    const inSafeZone = client.snapshot?.inSafeZone === true;
    const dangerous = livingHostiles(client.snapshot)
      .filter(entity => chebyshev(actor, entity) < dangerDistance)
      .filter(() => !inSafeZone)
      .sort((left, right) => chebyshev(actor, left) - chebyshev(actor, right) ||
        Number(left?.objectId ?? 0) - Number(right?.objectId ?? 0));

    const adjacent = dangerous.filter(entity => chebyshev(actor, entity) <= 1).length;
    const withinThree = dangerous.filter(entity => chebyshev(actor, entity) <= 3).length;
    const safeZoneAttackReceipt = inSafeZone && !safeZoneEscapeBlocked;
    if (typeof emergencyEscape === 'function' && !emergencyEscapeFailed &&
        now() >= emergencyEscapeRetryAt &&
        emergencyEscapes < Math.max(0, Number(maxEmergencyEscapes) || 0) &&
        healthRatio(client.snapshot) <= Math.max(0, Number(emergencyEscapeHpRatio) || 0) &&
        (safeZoneAttackReceipt || adjacent >= 2 || withinThree >= 3)) {
      const before = { x: Number(actor.x), y: Number(actor.y) };
      recordSurvivalDiagnostic(client, {
        type: 'recoveryEmergencyEscapeAttempt',
        hpRatio: healthRatio(client.snapshot),
        adjacent,
        withinThree,
        safeZoneAttackReceipt,
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
  if (!actor || actor.dead === true || Number(actor.hp) <= 0 || observedPlayerHp(snapshot) <= 0) {
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
    hp: observedPlayerHp(snapshot),
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
