/**
 * Deterministic policy for the first autonomous real-client quest slice.
 *
 * This module is deliberately transport-agnostic. It consumes the same
 * read-only client state a player can see and returns an intent. The runner is
 * responsible for realizing that intent with mouse/keyboard input only.
 */

export const QUEST_AGENT_CONTRACT = Object.freeze({
  version: 1,
  inputTransport: Object.freeze([
    "Input.dispatchMouseEvent",
    "Input.dispatchKeyEvent",
    "Input.insertText",
  ]),
  readOnlySurfaces: Object.freeze([
    "window.__mir2Stage5.state",
    "window.render_game_to_text",
    "DOM geometry and accessible labels",
    "same-origin rendered scene and collision assets",
    "WebSocket frame observation",
  ]),
  forbiddenClientCommands: Object.freeze([
    "transferMap",
    "moveTo",
    "acceptQuest",
    "finishQuest",
    "abandonQuest",
    "shareQuest",
    "stage5Command",
    "qa.openNpcDialog",
    "qa.giveItem",
    "event.spawn",
  ]),
  forbiddenChatPrefixes: Object.freeze(["@MOB", "@SETQUEST", "@LEVEL", "@GIVE", "@MOVE"]),
});

/** Treat an authoritative zero-HP actor as dead even if its render flag lags. */
export function entityIsLiveActor(entity) {
  if (!entity || entity.dead === true) return false;
  if (entity.hp == null || entity.hp === "") return true;
  const hp = Number(entity.hp);
  return !Number.isFinite(hp) || hp > 0;
}

/**
 * Keep a target-specific death authoritative while the renderer still exposes
 * the old object. Crystal may leave a corpse mounted with a lagging `dead`
 * flag and no usable HP value; treating that object as a new monster can split
 * one delayed EXP award across two apparent kills.
 *
 * An object id becomes eligible again only after it has disappeared from a
 * complete AOI snapshot and later reappears with definite positive HP. The
 * bounded hold is a defensive escape hatch for an object that never produces
 * either lifecycle observation.
 */
export function reconcileConfirmedDeadMonsterObjects(
  confirmedDeadObjects,
  entities,
  now = Date.now(),
  maxHoldMs = 10 * 60_000,
) {
  const observedAt = finiteNumber(now, Date.now());
  const holdMs = Math.max(1, finiteNumber(maxHoldMs, 10 * 60_000));
  const currentEntities = Array.isArray(entities) ? entities : null;
  const entityByObjectId = new Map(
    (currentEntities ?? [])
      .filter((entity) => entity?.objectId != null)
      .map((entity) => [String(entity.objectId), entity]),
  );
  const reconciled = new Map();

  for (const [rawObjectId, rawRecord] of confirmedDeadObjects instanceof Map
    ? confirmedDeadObjects
    : []) {
    const objectId = String(rawObjectId);
    const confirmedAt = finiteNumber(
      rawRecord && typeof rawRecord === "object"
        ? rawRecord.confirmedAt
        : rawRecord,
      Number.NaN,
    );
    if (!Number.isFinite(confirmedAt) || observedAt - confirmedAt >= holdMs) {
      continue;
    }

    const absenceObserved = Boolean(
      rawRecord && typeof rawRecord === "object"
        ? rawRecord.absenceObserved
        : false,
    );
    const entity = entityByObjectId.get(objectId) ?? null;
    if (currentEntities && !entity) {
      reconciled.set(objectId, { confirmedAt, absenceObserved: true });
      continue;
    }

    const hp = entity?.hp == null || entity?.hp === ""
      ? Number.NaN
      : Number(entity.hp);
    const definitePositiveHpRespawn = Boolean(
      entity &&
      entity.dead !== true &&
      Number.isFinite(hp) &&
      hp > 0,
    );
    if (absenceObserved && definitePositiveHpRespawn) continue;
    reconciled.set(objectId, { confirmedAt, absenceObserved });
  }

  return reconciled;
}

/**
 * Produce short, reversible walking targets for a visible safe-room recovery
 * loop. The runtime still validates Crystal collision before sending any
 * input; this policy only keeps pacing bounded around the arrival tile.
 */
export function safeRecoveryPaceTargets(anchor, distance = 2) {
  const x = Number(anchor?.x);
  const y = Number(anchor?.y);
  if (!Number.isFinite(x) || !Number.isFinite(y)) return [];
  const offset = Math.max(1, Math.floor(finiteNumber(distance, 2)));
  return [
    { x: x + offset, y },
    { x, y: y + offset },
    { x: x - offset, y },
    { x, y: y - offset },
  ].filter((point) => point.x >= 0 && point.y >= 0);
}

/**
 * Preserve a short authoritative-position history across bounded navigation
 * calls and identify the A,B,A,B pattern produced by a moving occupancy loop.
 * Resource-sensitive travel deliberately uses two-attempt chunks, so a visit
 * counter local to one call can never observe the third return to either cell.
 */
export function assessNavigationPositionCycle({
  history = [],
  position,
  now = Date.now(),
  windowMs = 30_000,
  maxEntries = 8,
} = {}) {
  const observedAt = finiteNumber(now, Date.now());
  const window = Math.max(1, finiteNumber(windowMs, 30_000));
  const limit = Math.max(4, Math.floor(finiteNumber(maxEntries, 8)));
  const recent = (Array.isArray(history) ? history : [])
    .map((entry) => ({
      x: Number(entry?.x),
      y: Number(entry?.y),
      at: Number(entry?.at),
    }))
    .filter((entry) => (
      Number.isFinite(entry.x) &&
      Number.isFinite(entry.y) &&
      Number.isFinite(entry.at) &&
      entry.at >= observedAt - window &&
      entry.at <= observedAt
    ));
  const x = Number(position?.x);
  const y = Number(position?.y);
  if (Number.isFinite(x) && Number.isFinite(y)) {
    recent.push({ x, y, at: observedAt });
  }
  if (recent.length > limit) recent.splice(0, recent.length - limit);

  const tail = recent.slice(-4);
  const same = (left, right) => left?.x === right?.x && left?.y === right?.y;
  const cycling = tail.length === 4 &&
    same(tail[0], tail[2]) &&
    same(tail[1], tail[3]) &&
    !same(tail[0], tail[1]);
  return {
    history: recent,
    cycling,
    cycleCells: cycling
      ? [
          { x: tail[0].x, y: tail[0].y },
          { x: tail[1].x, y: tail[1].y },
        ]
      : [],
  };
}

/**
 * Track net progress toward one visible recovery transfer. Dynamic actors can
 * make every immediate movement probe fail while the static route remains
 * connected; callers may rotate to another ordinary portal only after this
 * bounded window expires without improving the best observed distance.
 */
export function assessRecoveryTransferProgress({
  transferKey,
  distance,
  now = Date.now(),
  previous = null,
  stalledAfterMs = 45_000,
} = {}) {
  const key = String(transferKey ?? "");
  const currentDistance = Math.max(0, finiteNumber(distance, Number.POSITIVE_INFINITY));
  const currentAt = Math.max(0, finiteNumber(now, Date.now()));
  const previousKey = String(previous?.transferKey ?? "");
  const previousBestDistance = Number(previous?.bestDistance);
  const previousProgressAt = Number(previous?.lastProgressAt);
  const reset =
    key !== previousKey ||
    !Number.isFinite(previousBestDistance) ||
    !Number.isFinite(previousProgressAt);
  const progressed = !reset && currentDistance < previousBestDistance;
  const bestDistance = reset
    ? currentDistance
    : Math.min(previousBestDistance, currentDistance);
  const lastProgressAt = reset || progressed ? currentAt : previousProgressAt;
  const stallWindow = Math.max(1, finiteNumber(stalledAfterMs, 45_000));
  return {
    transferKey: key,
    bestDistance,
    lastProgressAt,
    stalled: Number.isFinite(currentDistance) && currentAt - lastProgressAt >= stallWindow,
  };
}

/**
 * Resolve the same F1-F8 skill-bar choice as the visible client, but admit
 * only an immediately targetable offensive spell. Ground skills need a
 * second physical tile click and self/toggle/passive skills are not attacks,
 * so callers must handle those separately instead of guessing.
 */
export function offensiveCombatSkillHotkey(knownSkills) {
  const skills = Array.isArray(knownSkills) ? knownSkills : [];
  const usable = (skill) => Boolean(
    skill &&
    skill.offensive === true &&
    (skill.castKind == null || skill.castKind === "target") &&
    finiteNumber(skill.cooldownRemainingTicks, 0) <= 0 &&
    String(skill.spell ?? skill.key ?? "").length > 0
  );
  for (let slot = 1; slot <= 8; slot += 1) {
    const skill = skills.find((entry) => Number(entry?.hotkey) === slot)
      ?? skills[slot - 1]
      ?? null;
    if (usable(skill)) return { slot, skill };
  }
  return null;
}

/**
 * Resolve the visible F1-F8 slot for Crystal Healing when it can be cast on
 * the player. Ground-targeted group heals need a second physical click and
 * are deliberately left to a separate adapter.
 */
export function restorativeSelfSkillHotkey(knownSkills) {
  const skills = Array.isArray(knownSkills) ? knownSkills : [];
  const usable = (skill) => {
    const spell = String(skill?.spell ?? skill?.name ?? skill?.key ?? "")
      .replace(/[^a-z0-9]/gi, "")
      .toLowerCase();
    return Boolean(
      skill &&
      spell === "healing" &&
      skill.offensive !== true &&
      (skill.castKind == null || skill.castKind === "target" || skill.castKind === "self") &&
      finiteNumber(skill.cooldownRemainingTicks, 0) <= 0
    );
  };
  for (let slot = 1; slot <= 8; slot += 1) {
    const skill = skills.find((entry) => Number(entry?.hotkey) === slot)
      ?? skills[slot - 1]
      ?? null;
    if (usable(skill)) return { slot, skill };
  }
  return null;
}

/** Keep full visual evidence at semantic goals and bounded grind checkpoints. */
export function shouldCaptureGoalFrame(
  goal,
  before,
  after,
  sequence,
  grindInterval = 100,
) {
  if (goal?.kind !== "grind") return true;
  if (finiteNumber(before?.playerLevel, 0) !== finiteNumber(after?.playerLevel, 0)) {
    return true;
  }
  const current = Math.max(1, Math.floor(finiteNumber(sequence, 1)));
  const interval = Math.max(1, Math.floor(finiteNumber(grindInterval, 100)));
  return current === 1 || current % interval === 0;
}

export function missingStarterEquipment(state, wanted) {
  const inventory = Array.isArray(state?.inventoryItems) ? state.inventoryItems : [];
  const equipment = Array.isArray(state?.equipmentItems) ? state.equipmentItems : [];
  return wanted.filter(({ name, slot }) => (
    inventory.some((item) => item?.name === name) &&
    !equipment.some((item) => item?.slot === slot)
  ));
}

/**
 * Select a rendered ground drop that can be picked up without starting a
 * navigation chase. The caller still has to realize the pickup through the
 * visible client; this helper only ranks read-only world state.
 */
export function nearestGroundDropByName(
  state,
  itemName,
  maxDistance = 1,
  ignoredObjectIds = [],
) {
  const player = state?.player;
  if (!player) return null;
  const wanted = normalizePolicyName(itemName);
  const distanceLimit = Math.max(0, finiteNumber(maxDistance, 1));
  const ignored = new Set(
    Array.from(ignoredObjectIds ?? [], (value) => String(value)),
  );
  return (Array.isArray(state?.groundDrops) ? state.groundDrops : [])
    .filter((drop) => {
      const renderedName = normalizePolicyName(drop?.name);
      // The scene renders authoritative gold quantity as e.g. "51 Gold".
      // Accept that visible label only for the Gold resource; quest/item names
      // retain exact matching so similarly named loot cannot be substituted.
      const nameMatches = renderedName === wanted || (
        wanted === "gold" && /^\d+gold$/.test(renderedName)
      );
      return !ignored.has(String(drop?.objectId ?? "")) &&
        nameMatches &&
        Math.max(
          Math.abs(finiteNumber(drop?.x, 0) - finiteNumber(player.x, 0)),
          Math.abs(finiteNumber(drop?.y, 0) - finiteNumber(player.y, 0)),
        ) <= distanceLimit;
    })
    .sort((left, right) => {
      const leftDistance = Math.max(
        Math.abs(finiteNumber(left?.x, 0) - finiteNumber(player.x, 0)),
        Math.abs(finiteNumber(left?.y, 0) - finiteNumber(player.y, 0)),
      );
      const rightDistance = Math.max(
        Math.abs(finiteNumber(right?.x, 0) - finiteNumber(player.x, 0)),
        Math.abs(finiteNumber(right?.y, 0) - finiteNumber(player.y, 0)),
      );
      return leftDistance - rightDistance ||
        String(left?.objectId ?? "").localeCompare(String(right?.objectId ?? ""));
    })[0] ?? null;
}

export function nearestHealthPotionGroundDrop(state, maxDistance = 8) {
  const player = state?.player;
  if (!player) return null;
  const distanceLimit = Math.max(0, finiteNumber(maxDistance, 8));
  return (Array.isArray(state?.groundDrops) ? state.groundDrops : [])
    .filter((drop) => (
      /\(hp\).*drug|health.*potion/i.test(String(drop?.name ?? "")) &&
      Math.max(
        Math.abs(finiteNumber(drop?.x, 0) - finiteNumber(player.x, 0)),
        Math.abs(finiteNumber(drop?.y, 0) - finiteNumber(player.y, 0)),
      ) <= distanceLimit
    ))
    .sort((left, right) => {
      const leftDistance = Math.max(
        Math.abs(finiteNumber(left?.x, 0) - finiteNumber(player.x, 0)),
        Math.abs(finiteNumber(left?.y, 0) - finiteNumber(player.y, 0)),
      );
      const rightDistance = Math.max(
        Math.abs(finiteNumber(right?.x, 0) - finiteNumber(player.x, 0)),
        Math.abs(finiteNumber(right?.y, 0) - finiteNumber(player.y, 0)),
      );
      return leftDistance - rightDistance ||
        String(left?.objectId ?? "").localeCompare(String(right?.objectId ?? ""));
    })[0] ?? null;
}

/**
 * Decide whether a character standing in the normal supply area should earn
 * the missing minimum potion money through ordinary hunting before departing
 * for a dangerous quest field.
 */
export function shouldFundHealthPotions(
  state,
  {
    homeMapFileName = "0",
    merchant = null,
    merchantRadius = 180,
    minimumGold = 40,
    minimumPotions = 1,
  } = {},
) {
  if (!state?.player || String(state?.mapFileName ?? "") !== String(homeMapFileName)) return false;
  if (merchant) {
    const distance = Math.max(
      Math.abs(finiteNumber(state.player.x, 0) - finiteNumber(merchant.x, 0)),
      Math.abs(finiteNumber(state.player.y, 0) - finiteNumber(merchant.y, 0)),
    );
    if (distance > Math.max(0, finiteNumber(merchantRadius, 180))) return false;
  }
  const potionQuantity = [
    ...(Array.isArray(state?.beltItems) ? state.beltItems : []),
    ...(Array.isArray(state?.inventoryItems) ? state.inventoryItems : []),
  ]
    .filter((item) => /\(hp\).*drug|health.*potion/i.test(String(item?.name ?? item?.key ?? "")))
    .reduce((total, item) => total + Math.max(1, finiteNumber(item?.quantity, 1)), 0);
  return potionQuantity < Math.max(0, finiteNumber(minimumPotions, 1)) &&
    finiteNumber(state?.gold, 0) < Math.max(0, finiteNumber(minimumGold, 40));
}

/**
 * Plan a visible potion purchase without spending the character's last gold
 * on a batch too small to change the available funding strategy.
 *
 * A full departure batch always wins when it is affordable. Otherwise the
 * first purchase stops exactly at the working-stock threshold; that stock is
 * enough for the runner to use the deterministic Deer -> Venison -> Butcher
 * loop while the hard quest-departure gate continues to require the full
 * batch. Once working stock exists, gold is retained until the full remainder
 * is affordable (except when replacing working stock that was consumed).
 */
export function planHealthPotionPurchase({
  currentQuantity = 0,
  gold = 0,
  unitPrice = 40,
  departureStock = 10,
  workingStock = 5,
} = {}) {
  const current = Math.max(0, Math.floor(finiteNumber(currentQuantity, 0)));
  const departure = Math.max(current, Math.floor(finiteNumber(departureStock, 10)));
  const working = Math.min(
    departure,
    Math.max(0, Math.floor(finiteNumber(workingStock, 5))),
  );
  const price = finiteNumber(unitPrice, 0);
  if (price <= 0 || current >= departure) return 0;

  const affordable = Math.max(0, Math.floor(finiteNumber(gold, 0) / price));
  const departureMissing = departure - current;
  if (affordable >= departureMissing) return departureMissing;

  const workingMissing = Math.max(0, working - current);
  return workingMissing > 0 && affordable >= workingMissing
    ? workingMissing
    : 0;
}

export function nearestBlockingHostile(
  state,
  requestedMonsterName,
  cooldownUntil = new Map(),
  now = Date.now(),
  candidateFilter = null,
  preferredObjectId = null,
) {
  const player = state?.player;
  if (!player) return null;
  return (Array.isArray(state?.entities) ? state.entities : [])
    .filter((entity) => (
      entity?.kind === "monster" &&
      entity?.disposition === "hostile" &&
      entityIsLiveActor(entity) &&
      String(entity?.name ?? "") !== String(requestedMonsterName ?? "") &&
      Math.max(
        Math.abs(finiteNumber(entity?.x, 0) - finiteNumber(player.x, 0)),
        Math.abs(finiteNumber(entity?.y, 0) - finiteNumber(player.y, 0)),
      ) <= 1 &&
      finiteNumber(cooldownUntil.get?.(String(entity?.objectId)), 0) <= now &&
      (typeof candidateFilter !== "function" || candidateFilter(entity))
    ))
    .sort((left, right) => {
      const leftPreferred = preferredObjectId != null &&
        String(left?.objectId ?? "") === String(preferredObjectId);
      const rightPreferred = preferredObjectId != null &&
        String(right?.objectId ?? "") === String(preferredObjectId);
      if (leftPreferred !== rightPreferred) return leftPreferred ? -1 : 1;
      const rightAttack = finiteNumber(right?.attackUntil, 0);
      const leftAttack = finiteNumber(left?.attackUntil, 0);
      if (rightAttack !== leftAttack) return rightAttack - leftAttack;
      return String(left?.objectId ?? "").localeCompare(String(right?.objectId ?? ""));
    })[0] ?? null;
}

/** Count live hostile actors occupying the immediate movement ring. */
export function denseAdjacentHostileCount(state, radius = 1) {
  const player = state?.player;
  if (!player) return 0;
  const maximumDistance = Math.max(0, finiteNumber(radius, 1));
  return (Array.isArray(state?.entities) ? state.entities : [])
    .filter((entity) => {
      const distance = Math.max(
        Math.abs(finiteNumber(entity?.x, 0) - finiteNumber(player.x, 0)),
        Math.abs(finiteNumber(entity?.y, 0) - finiteNumber(player.y, 0)),
      );
      return (
        entity?.kind === "monster" &&
        entity?.disposition === "hostile" &&
        entityIsLiveActor(entity) &&
        distance > 0 &&
        distance <= maximumDistance
      );
    }).length;
}

/**
 * Incidental combat is only a last-resort way to free one occupied movement
 * tile. Require a real level advantage before treating an unrelated attacker
 * as disposable; equal- or higher-level monsters belong to route avoidance,
 * not a potentially potion-draining fight that cannot advance the quest.
 */
export function incidentalTravelThreatIsTrivial(
  monsterLevel,
  playerLevel,
  minimumLevelAdvantage = 3,
) {
  const monster = finiteNumber(monsterLevel, Number.NaN);
  const player = finiteNumber(playerLevel, Number.NaN);
  const advantage = Math.max(0, finiteNumber(minimumLevelAdvantage, 3));
  return Number.isFinite(monster) && Number.isFinite(player) &&
    monster <= player - advantage;
}

/**
 * A travel-clear click is only trying to open one occupied tile. If two real
 * attacks over four seconds produce no packet for that exact object, waiting
 * the full quest-combat window only burns recovery time while the player stays
 * surrounded. Keep ordinary quest combat conservative because its target may
 * legitimately have a slower first response.
 */
export function combatNoResponseBudget(incidentalTravelThreat = false) {
  return incidentalTravelThreat
    ? { minimumElapsedMs: 4_000, minimumAttackCount: 2 }
    : { minimumElapsedMs: 15_000, minimumAttackCount: 5 };
}

/**
 * Return whether the rendered entity has performed an attack recently enough
 * to be treated as an active travel threat. Merely standing next to a hostile
 * is not sufficient: collision-aware movement can route around an idle actor,
 * while chasing every adjacent spawn turns ordinary travel into random grind.
 */
export function entityAttackIsRecent(entity, now = Date.now(), withinMs = 3_500) {
  const lastAttackAt = Math.max(
    finiteNumber(entity?.attackStartedAt, 0),
    finiteNumber(entity?.attackUntil, 0),
  );
  return lastAttackAt > 0 &&
    lastAttackAt >= finiteNumber(now, 0) - Math.max(0, finiteNumber(withinMs, 0));
}

/** Return the nearest rendered hostile that is actively attacking the player. */
export function nearestActiveHostile(
  state,
  {
    excludeObjectId = null,
    maxDistance = 8,
    now = Date.now(),
    withinMs = 15_000,
  } = {},
) {
  const player = state?.player;
  if (!player) return null;
  const distance = (entity) => Math.max(
    Math.abs(finiteNumber(entity?.x, 0) - finiteNumber(player.x, 0)),
    Math.abs(finiteNumber(entity?.y, 0) - finiteNumber(player.y, 0)),
  );
  return (Array.isArray(state?.entities) ? state.entities : [])
    .filter((entity) => (
      entity?.kind === "monster" &&
      entity?.disposition === "hostile" &&
      entityIsLiveActor(entity) &&
      String(entity?.objectId ?? "") !== String(excludeObjectId ?? "") &&
      distance(entity) <= Math.max(0, finiteNumber(maxDistance, 8)) &&
      entityAttackIsRecent(entity, now, withinMs)
    ))
    .sort((left, right) => (
      distance(left) - distance(right) ||
      finiteNumber(right?.attackUntil, 0) - finiteNumber(left?.attackUntil, 0) ||
      String(left?.objectId ?? "").localeCompare(String(right?.objectId ?? ""))
    ))[0] ?? null;
}

/**
 * Return the nearest live hostile whose exact object id is still under the
 * target-specific no-response quarantine. This is separate from an ordinary
 * combat cooldown: navigation should physically create separation from an
 * unresponsive pursuer instead of selecting it for another attack cycle.
 */
export function nearestQuarantinedHostile(
  state,
  quarantinedUntil,
  {
    maxDistance = 6,
    now = Date.now(),
  } = {},
) {
  const player = state?.player;
  if (!player || !(quarantinedUntil instanceof Map)) return null;
  const observedAt = finiteNumber(now, Date.now());
  const radius = Math.max(0, finiteNumber(maxDistance, 6));
  const distance = (entity) => Math.max(
    Math.abs(finiteNumber(entity?.x, 0) - finiteNumber(player.x, 0)),
    Math.abs(finiteNumber(entity?.y, 0) - finiteNumber(player.y, 0)),
  );
  return (Array.isArray(state?.entities) ? state.entities : [])
    .filter((entity) => (
      entity?.kind === "monster" &&
      entity?.disposition === "hostile" &&
      entityIsLiveActor(entity) &&
      Number(quarantinedUntil.get(String(entity?.objectId ?? "")) ?? 0) > observedAt &&
      distance(entity) <= radius
    ))
    .sort((left, right) => (
      distance(left) - distance(right) ||
      Number(quarantinedUntil.get(String(right?.objectId ?? "")) ?? 0) -
        Number(quarantinedUntil.get(String(left?.objectId ?? "")) ?? 0) ||
      String(left?.objectId ?? "").localeCompare(String(right?.objectId ?? ""))
    ))[0] ?? null;
}

/**
 * Rank a bounded eight-direction retreat fan away from an attacker. The first
 * candidate preserves the historical direct-away vector; callers with live
 * collision data can rotate through the remaining candidates when that exact
 * endpoint lies inside a wall or sealed building pocket.
 */
export function retreatPointsFromHostile(state, hostile, span = 8) {
  const player = state?.player;
  if (!player || !hostile) return [];
  const retreatSpan = Math.max(1, Math.floor(finiteNumber(span, 8)));
  const dx = finiteNumber(player.x, 0) - finiteNumber(hostile.x, 0);
  const dy = finiteNumber(player.y, 0) - finiteNumber(hostile.y, 0);
  // Exact overlap has no geometric preference. Pick a deterministic cardinal
  // direction and let collision-aware navigation choose a legal detour.
  const awayX = dx === 0 && dy === 0 ? -1 : Math.sign(dx);
  const awayY = dx === 0 && dy === 0 ? 0 : Math.sign(dy);
  const directions = [
    [-1, -1], [0, -1], [1, -1],
    [-1, 0], [1, 0],
    [-1, 1], [0, 1], [1, 1],
  ].map(([x, y], index) => ({ x, y, index }));
  directions.sort((left, right) => (
    (right.x * awayX + right.y * awayY) -
      (left.x * awayX + left.y * awayY) ||
    (Math.abs(left.x) + Math.abs(left.y)) -
      (Math.abs(right.x) + Math.abs(right.y)) ||
    left.index - right.index
  ));
  return directions
    .map((direction) => ({
      x: finiteNumber(player.x, 0) + direction.x * retreatSpan,
      y: finiteNumber(player.y, 0) + direction.y * retreatSpan,
    }))
    .filter((point) => point.x >= 0 && point.y >= 0);
}

/** Pick the preferred short visible-input retreat vector from the ranked fan. */
export function retreatPointFromHostile(state, hostile, span = 8) {
  return retreatPointsFromHostile(state, hostile, span)[0] ?? null;
}

/**
 * Rank rendered combat candidates from the outside of a pack inward. A real
 * melee player should not click the closest member of a tight group when an
 * isolated edge member is visible: reaching the closest target can put every
 * neighbour in attack range before the first kill completes.
 *
 * Immediate neighbours dominate the score, then the wider four-tile pack.
 * Player distance remains the final geometric tie-break so isolated targets
 * do not cause an unbounded chase across the map.
 */
export function rankCombatTargetsByIsolation(
  state,
  candidates,
  { immediateRadius = 1, packRadius = 4 } = {},
) {
  const player = state?.player;
  const liveMonsters = (Array.isArray(state?.entities) ? state.entities : [])
    .filter((entity) => (
      entity?.kind === "monster" &&
      entityIsLiveActor(entity) &&
      entity?.disposition !== "friendly"
    ));
  const distanceBetween = (left, right) => Math.max(
    Math.abs(finiteNumber(left?.x, 0) - finiteNumber(right?.x, 0)),
    Math.abs(finiteNumber(left?.y, 0) - finiteNumber(right?.y, 0)),
  );
  const scored = (Array.isArray(candidates) ? candidates : [])
    .filter(entityIsLiveActor)
    .map((candidate, index) => {
      const neighbours = liveMonsters.filter((entity) => (
        String(entity?.objectId ?? "") !== String(candidate?.objectId ?? "")
      ));
      return {
        candidate,
        index,
        immediate: neighbours.filter((entity) => (
          distanceBetween(candidate, entity) <= Math.max(0, finiteNumber(immediateRadius, 1))
        )).length,
        pack: neighbours.filter((entity) => (
          distanceBetween(candidate, entity) <= Math.max(0, finiteNumber(packRadius, 4))
        )).length,
        playerDistance: player ? distanceBetween(player, candidate) : 0,
      };
    });
  return scored
    .sort((left, right) => (
      left.immediate - right.immediate ||
      left.pack - right.pack ||
      left.playerDistance - right.playerDistance ||
      String(left.candidate?.objectId ?? "")
        .localeCompare(String(right.candidate?.objectId ?? "")) ||
      left.index - right.index
    ))
    .map(({ candidate }) => candidate);
}

/**
 * Prefer an attacker already forcing combat; otherwise decline a crowded
 * adjacent target when a safer same-name edge target is known nearby. Returning
 * null tells the caller to approach the better target before clicking.
 */
export function chooseImmediateMeleeTarget(
  state,
  candidates,
  {
    engagementRadius = 1,
    searchRadius = 16,
    now = Date.now(),
    activeAttackWindowMs = 15_000,
  } = {},
) {
  const player = state?.player;
  if (!player) return null;
  const distance = (entity) => Math.max(
    Math.abs(finiteNumber(entity?.x, 0) - finiteNumber(player.x, 0)),
    Math.abs(finiteNumber(entity?.y, 0) - finiteNumber(player.y, 0)),
  );
  const ranked = rankCombatTargetsByIsolation(state, candidates);
  const immediate = ranked.filter(
    (entity) => distance(entity) <= Math.max(0, finiteNumber(engagementRadius, 1)),
  );
  if (!immediate.length) return null;
  const active = immediate.find((entity) =>
    entityAttackIsRecent(entity, now, activeAttackWindowMs)
  );
  if (active) return active;
  const safestKnown = ranked.find(
    (entity) => distance(entity) <= Math.max(0, finiteNumber(searchRadius, 16)),
  );
  if (safestKnown && distance(safestKnown) > engagementRadius) return null;
  return immediate[0];
}

/**
 * Convert a technically successful quest fight into an auditable preparation
 * signal when it exhausted the character's ordinary healing resources. A
 * near-death win is not sustainable evidence for an autonomous farming loop.
 */
function policyHealthPotionQuantity(state) {
  return [
    ...(Array.isArray(state?.beltItems) ? state.beltItems : []),
    ...(Array.isArray(state?.inventoryItems) ? state.inventoryItems : []),
  ]
    .filter((item) => /\(hp\).*drug|health.*potion/i.test(String(item?.name ?? item?.key ?? "")))
    .reduce((total, item) => total + Math.max(1, finiteNumber(item?.quantity, 1)), 0);
}

/**
 * An ordinary journey should stop once its combat reserve becomes unsafe.
 * A shelter escape that starts after the reserve is already exhausted cannot
 * use that same absolute threshold: it would reject the first unchanged frame
 * and prevent every physical retreat. Keep the guard only while the character
 * still has both non-critical health and at least one emergency potion. The
 * caller still uses normal movement, death detection, and visible revival.
 */
export function shouldEnforceShelterEscapeResourceBudget(
  state,
  { criticalHealthRatio = 0.2 } = {},
) {
  const hp = Math.max(0, finiteNumber(state?.playerHp, 0));
  const maxHp = Math.max(0, finiteNumber(state?.playerMaxHp, 0));
  const healthRatio = maxHp > 0 ? hp / maxHp : 1;
  return (
    hp > 0 &&
    healthRatio > Math.max(0, finiteNumber(criticalHealthRatio, 0.2)) &&
    policyHealthPotionQuantity(state) > 0
  );
}

export function assessQuestCombatResourceStrain(
  before,
  after,
  { criticalHealthRatio = 0.2, criticalPotionUse = 5 } = {},
) {
  const potionsBefore = policyHealthPotionQuantity(before);
  const potionsAfter = policyHealthPotionQuantity(after);
  const potionsUsed = Math.max(0, potionsBefore - potionsAfter);
  const hp = Math.max(0, finiteNumber(after?.playerHp, 0));
  const maxHp = Math.max(0, finiteNumber(after?.playerMaxHp, 0));
  const healthRatio = maxHp > 0 ? hp / maxHp : 1;
  const depleted = potionsBefore > 0 && potionsAfter === 0;
  const criticalHealth = hp > 0 && healthRatio <= Math.max(0, finiteNumber(criticalHealthRatio, 0.2));
  const excessivePotionUse = potionsUsed >= Math.max(1, finiteNumber(criticalPotionUse, 5));
  return {
    severe: depleted || criticalHealth || excessivePotionUse,
    depleted,
    criticalHealth,
    excessivePotionUse,
    potionsBefore,
    potionsAfter,
    potionsUsed,
    hp,
    maxHp,
    healthRatio,
  };
}

/**
 * Distinguish a genuinely stalled grind source from a noisy goal label. Live
 * runs can report a retryable target error after a collateral normal-client
 * kill, so only a failed goal with no authoritative level/EXP gain advances
 * the stall counter. A real gain immediately clears the source's history.
 */
export function assessGrindingSourceStall(
  goal,
  before,
  after,
  {
    failed = false,
    previousStalls = 0,
    threshold = 3,
    now = Date.now(),
    cooldownMs = 10 * 60_000,
  } = {},
) {
  if (goal?.kind !== "grind") {
    return {
      tracked: false,
      progressed: false,
      stallCount: 0,
      cooldownUntil: null,
    };
  }
  const beforeLevel = finiteNumber(before?.playerLevel, 0);
  const afterLevel = finiteNumber(after?.playerLevel, beforeLevel);
  const beforeExperience = finiteNumber(before?.playerExperience, 0);
  const afterExperience = finiteNumber(after?.playerExperience, beforeExperience);
  const progressed =
    afterLevel > beforeLevel ||
    (afterLevel === beforeLevel && afterExperience > beforeExperience);
  if (progressed) {
    return {
      tracked: true,
      progressed: true,
      stallCount: 0,
      cooldownUntil: null,
    };
  }
  const stallCount = failed
    ? Math.max(0, Math.trunc(finiteNumber(previousStalls, 0))) + 1
    : Math.max(0, Math.trunc(finiteNumber(previousStalls, 0)));
  const shouldCoolDown = failed && stallCount >= Math.max(1, Math.trunc(finiteNumber(threshold, 3)));
  return {
    tracked: true,
    progressed: false,
    stallCount,
    cooldownUntil: shouldCoolDown
      ? finiteNumber(now, Date.now()) + Math.max(0, finiteNumber(cooldownMs, 10 * 60_000))
      : null,
  };
}

/**
 * Keep only resource-strain observations that have not subsequently been
 * disproved by a confirmed normal-client kill or an explicit preparation
 * completion. Ordering matters: a later severe fight must survive an older
 * recovery, while legacy records without timestamps are resolved by any
 * timestamped recovery for the same monster.
 */
export function unresolvedCombatResourceStrains(strains, recoveries) {
  const latestRecoveryAt = new Map();
  for (const recovery of Array.isArray(recoveries) ? recoveries : []) {
    const monsterKey = normalizePolicyName(recovery?.monsterName);
    const at = finiteNumber(recovery?.at, Number.NaN);
    if (!monsterKey || !Number.isFinite(at)) continue;
    latestRecoveryAt.set(
      monsterKey,
      Math.max(finiteNumber(latestRecoveryAt.get(monsterKey), Number.NEGATIVE_INFINITY), at),
    );
  }
  return (Array.isArray(strains) ? strains : []).filter((strain) => {
    const monsterKey = normalizePolicyName(strain?.monsterName);
    if (!monsterKey) return false;
    const recoveryAt = latestRecoveryAt.get(monsterKey);
    if (!Number.isFinite(recoveryAt)) return true;
    const strainAt = finiteNumber(strain?.at, Number.NEGATIVE_INFINITY);
    return strainAt > recoveryAt;
  });
}

/**
 * A severe resource loss is also an unfinished supply action. Preserve that
 * one-shot recall across supervised process boundaries until later, auditable
 * combat evidence resolves the strain. Older reports omitted `severe`, but
 * every row in their strain collection had already passed the same predicate.
 */
export function combatMemoryRequiresSupplyRecall(
  strains,
  recoveries,
  {
    currentPotionQuantity = null,
    requiredPotionQuantity = null,
  } = {},
) {
  const hasStockBoundary =
    currentPotionQuantity != null && requiredPotionQuantity != null;
  const currentStock = finiteNumber(currentPotionQuantity, Number.NaN);
  const requiredStock = finiteNumber(requiredPotionQuantity, Number.NaN);
  if (
    hasStockBoundary &&
    Number.isFinite(currentStock) &&
    Number.isFinite(requiredStock) &&
    currentStock >= Math.max(0, requiredStock)
  ) return false;
  return unresolvedCombatResourceStrains(strains, recoveries)
    .some((strain) => strain?.severe !== false);
}

/**
 * Build a routing halo only for monsters that are meaningfully above the
 * player's level. Ordinary field creatures still occupy their exact server
 * tile, but no longer turn a dense spawn area into one artificial wall.
 * Unknown monsters retain a conservative one-tile halo.
 */
export function dangerousHostileAvoidanceCells(
  state,
  grindingCatalog = [],
  { radius = 2, maxSafeLevelDelta = 2, safeMonsterNames = [] } = {},
) {
  const playerLevel = Number(state?.playerLevel ?? 0);
  const certifiedSafeNames = new Set(
    (Array.isArray(safeMonsterNames) ? safeMonsterNames : [])
      .map((name) => normalizePolicyName(name))
      .filter(Boolean),
  );
  const levelByName = new Map(
    (Array.isArray(grindingCatalog) ? grindingCatalog : [])
      .map((entry) => [normalizePolicyName(entry?.monsterName), Number(entry?.level)])
      .filter(([name, level]) => name && Number.isFinite(level)),
  );
  const cells = [];
  const seen = new Set();
  for (const entity of Array.isArray(state?.entities) ? state.entities : []) {
    if (!entityIsLiveActor(entity) || entity?.kind !== "monster" || entity?.disposition === "friendly") continue;
    const normalizedName = normalizePolicyName(entity?.name);
    if (certifiedSafeNames.has(normalizedName)) continue;
    const monsterLevel = levelByName.get(normalizedName);
    const avoidanceRadius = Number.isFinite(monsterLevel)
      ? monsterLevel > playerLevel + maxSafeLevelDelta ? radius : 0
      : 1;
    for (let dx = -avoidanceRadius; dx <= avoidanceRadius; dx += 1) {
      for (let dy = -avoidanceRadius; dy <= avoidanceRadius; dy += 1) {
        if (dx === 0 && dy === 0) continue;
        const x = Number(entity.x) + dx;
        const y = Number(entity.y) + dy;
        const key = `${x},${y}`;
        if (!Number.isFinite(x) || !Number.isFinite(y) || seen.has(key)) continue;
        seen.add(key);
        cells.push({ x, y });
      }
    }
  }
  return cells;
}

/**
 * Rank real respawn regions for a long trip. Distance remains the main cost,
 * but a tightly packed field gets a deterministic risk penalty so a fragile
 * character does not repeatedly choose the shortest route into the densest
 * edge of a dangerous spawn.
 */
export function rankRespawnFieldsForTravel(
  player,
  fields,
  {
    densityWeight = 500,
    hazards = [],
    exposureWeight = 12,
    terminalExposureWeight = 10_000,
    terminalExposureCap = 1_000,
  } = {},
) {
  return (Array.isArray(fields) ? fields : [])
    .map((field, index) => {
      const spread = Math.max(1, finiteNumber(field?.spread, 1));
      const count = Math.max(0, finiteNumber(field?.count, 0));
      const approach = nearestRespawnApproachPoint(player, field);
      // A source position is the centre of a spawn region, not the place a
      // real player has to reach before seeing its monsters. Score the nearest
      // interior edge instead. This prevents a large source from looking safe
      // only because its centre sits beyond a hostile band, and it avoids
      // forcing the client through the densest part of the requested spawn.
      const scoringField = {
        ...field,
        ...approach,
        spread: Math.max(8, spread * 0.15),
      };
      const distance = player
        ? Math.max(
            Math.abs(finiteNumber(approach?.x, 0) - finiteNumber(player.x, 0)),
            Math.abs(finiteNumber(approach?.y, 0) - finiteNumber(player.y, 0)),
          )
        : 0;
      const exposure = respawnCorridorExposure(player, scoringField, hazards);
      const terminalExposure = respawnTerminalExposure(scoringField, hazards);
      // Source tables overlap heavily and may describe several monster
      // families at the same coordinate. Terminal exposure is therefore a
      // useful risk warning, not a route-length oracle. Bound its contribution
      // so duplicated spawn rectangles cannot make the planner prefer a
      // several-hundred-tile lethal detour over a nearby, live-reachable edge.
      const terminalPenalty = Math.min(
        Math.max(0, finiteNumber(terminalExposureCap, 1_000)),
        terminalExposure * Math.max(0, finiteNumber(terminalExposureWeight, 10_000)),
      );
      return {
        field,
        index,
        score: distance +
          (count / spread) * densityWeight +
          exposure * exposureWeight +
          terminalPenalty,
      };
    })
    .sort((left, right) => left.score - right.score || left.index - right.index)
    .map((entry) => entry.field);
}

/**
 * Choose the closest point far enough inside a respawn region to reveal its
 * entities. Crystal source coordinates are region centres and `spread` is a
 * radius; walking to the centre needlessly crosses most of the spawn pack.
 */
export function nearestRespawnApproachPoint(player, field) {
  const centerX = finiteNumber(field?.x, 0);
  const centerY = finiteNumber(field?.y, 0);
  const spread = Math.max(0, finiteNumber(field?.spread, 0));
  if (!player || spread <= 16) {
    return { x: Math.max(0, Math.round(centerX)), y: Math.max(0, Math.round(centerY)) };
  }
  const dx = finiteNumber(player.x, centerX) - centerX;
  const dy = finiteNumber(player.y, centerY) - centerY;
  const distance = Math.max(Math.abs(dx), Math.abs(dy));
  if (distance <= 0) {
    return { x: Math.max(0, Math.round(centerX)), y: Math.max(0, Math.round(centerY)) };
  }
  const approachRadius = spread * 0.85;
  return {
    x: Math.max(0, Math.round(centerX + (dx / distance) * approachRadius)),
    y: Math.max(0, Math.round(centerY + (dy / distance) * approachRadius)),
  };
}

/**
 * Give a long collision-routed field approach enough physical input attempts
 * to go around large town buildings. The stationary-chunk and unreachable
 * guards remain the termination conditions; straight-line distance alone is
 * not a safe upper bound for a real walkable route.
 */
export function respawnTravelAttemptBudget(distance) {
  const normalizedDistance = Math.max(0, finiteNumber(distance, 0));
  if (normalizedDistance <= 30) return 15;
  return Math.min(480, Math.max(60, Math.ceil(normalizedDistance * 3)));
}

/**
 * Estimate hostile density where a target respawn region overlaps other
 * aggressive source regions. Corridor safety alone is insufficient: a short
 * route can still end inside several dense, higher-level spawn tables. The
 * overlap ratio keeps edge-touching regions cheaper than coincident centres.
 */
export function respawnTerminalExposure(field, hazards = []) {
  if (!field) return 0;
  const fieldSpread = Math.max(1, finiteNumber(field?.spread, 1));
  let exposure = 0;
  for (const hazard of Array.isArray(hazards) ? hazards : []) {
    const hazardSpread = Math.max(1, finiteNumber(hazard?.spread, 1));
    const overlapReach = fieldSpread + hazardSpread;
    const distance = Math.max(
      Math.abs(finiteNumber(field?.x, 0) - finiteNumber(hazard?.x, 0)),
      Math.abs(finiteNumber(field?.y, 0) - finiteNumber(hazard?.y, 0)),
    );
    if (distance > overlapReach) continue;
    const overlapRatio = Math.max(0, 1 - distance / overlapReach);
    exposure += (
      Math.max(0, finiteNumber(hazard?.count, 0)) /
      hazardSpread
    ) * overlapRatio;
  }
  return exposure;
}

/**
 * Approximate how long a straight journey remains inside known aggressive
 * source respawn regions. Sampling every four tiles is intentionally coarse:
 * this is a deterministic field-choice signal, not a replacement for the
 * live collision planner. A route that immediately exits a dense cat field
 * should beat one which traverses its full diameter even when the destination
 * field itself has a slightly better count/spread ratio.
 */
export function respawnCorridorExposure(player, field, hazards = []) {
  if (!player || !field) return 0;
  const startX = finiteNumber(player.x, 0);
  const startY = finiteNumber(player.y, 0);
  const endX = finiteNumber(field.x, startX);
  const endY = finiteNumber(field.y, startY);
  const distance = Math.max(Math.abs(endX - startX), Math.abs(endY - startY));
  if (distance <= 0) return 0;
  const sampleStep = 4;
  let exposure = 0;
  for (let offset = 0; offset <= distance; offset += sampleStep) {
    const ratio = Math.min(1, offset / distance);
    const x = startX + (endX - startX) * ratio;
    const y = startY + (endY - startY) * ratio;
    for (const hazard of Array.isArray(hazards) ? hazards : []) {
      const radius = Math.max(1, finiteNumber(hazard?.spread, 1));
      if (
        Math.max(
          Math.abs(x - finiteNumber(hazard?.x, 0)),
          Math.abs(y - finiteNumber(hazard?.y, 0)),
        ) > radius
      ) continue;
      exposure += (Math.max(0, finiteNumber(hazard?.count, 0)) / radius) * sampleStep;
    }
  }
  return exposure;
}

/**
 * Prefer a simple orthogonal elbow when the direct line to a respawn patrol
 * cuts through a known aggressive source rectangle. The live navigator still
 * owns collision and movement; this only supplies one ordinary intermediate
 * waypoint derived from the same authoritative respawn data.
 */
export function respawnCorridorAvoidanceWaypoint(
  player,
  field,
  hazards = [],
  {
    minimumImprovementRatio = 0.75,
    minimumLegDistance = 8,
    perpendicularOffsets = [],
    progressRatios = [0.33, 0.5, 0.67],
    candidateIndex = 0,
  } = {},
) {
  if (!player || !field || !Array.isArray(hazards) || hazards.length === 0) return null;
  const distance = (left, right) => Math.max(
    Math.abs(finiteNumber(left?.x, 0) - finiteNumber(right?.x, 0)),
    Math.abs(finiteNumber(left?.y, 0) - finiteNumber(right?.y, 0)),
  );
  const directExposure = respawnCorridorExposure(player, field, hazards);
  if (directExposure <= 0) return null;
  const directDistance = distance(player, field);
  const startX = finiteNumber(player.x, 0);
  const startY = finiteNumber(player.y, 0);
  const endX = finiteNumber(field.x, startX);
  const endY = finiteNumber(field.y, startY);
  const dx = endX - startX;
  const dy = endY - startY;
  // An orthogonal elbow is enough for a compact hazard. A long, nearly
  // vertical or horizontal hostile band needs a progressive lateral waypoint
  // instead: both simple elbows can keep the entire dangerous segment and
  // incorrectly report no improvement. Callers opt into these extra
  // candidates with bounded offsets; this function still only returns a map
  // coordinate for the ordinary collision navigator to walk toward.
  const progressiveCandidates = (Array.isArray(progressRatios) ? progressRatios : [])
    .flatMap((ratioValue) => {
      const ratio = Math.max(0.1, Math.min(0.9, finiteNumber(ratioValue, 0.5)));
      const base = {
        x: startX + dx * ratio,
        y: startY + dy * ratio,
      };
      return (Array.isArray(perpendicularOffsets) ? perpendicularOffsets : [])
        .map((offset) => Math.max(0, finiteNumber(offset, 0)))
        .filter((offset) => offset > 0)
        .flatMap((offset) => Math.abs(dy) >= Math.abs(dx)
          ? [
              { x: base.x - offset, y: base.y },
              { x: base.x + offset, y: base.y },
            ]
          : [
              { x: base.x, y: base.y - offset },
              { x: base.x, y: base.y + offset },
            ]);
    });
  const candidates = [
    { x: finiteNumber(field.x, 0), y: finiteNumber(player.y, 0) },
    { x: finiteNumber(player.x, 0), y: finiteNumber(field.y, 0) },
    ...progressiveCandidates,
  ]
    .map((waypoint) => ({
      x: Math.max(0, finiteNumber(waypoint.x, 0)),
      y: Math.max(0, finiteNumber(waypoint.y, 0)),
    }))
    .filter((waypoint) =>
      distance(player, waypoint) > Math.max(0, finiteNumber(minimumLegDistance, 8)) &&
      distance(waypoint, field) > Math.max(0, finiteNumber(minimumLegDistance, 8)) &&
      // Every intermediate point must make real destination progress. This
      // prevents repeated long-route planning from alternating between two
      // equally safe lateral points on successive policy turns.
      distance(waypoint, field) < directDistance
    )
    .map((waypoint) => {
      const detourExposure =
        respawnCorridorExposure(player, waypoint, hazards) +
        respawnCorridorExposure(waypoint, field, hazards);
      const detourDistance = distance(player, waypoint) + distance(waypoint, field);
      return {
        waypoint,
        detourExposure,
        score: detourExposure * 12 + detourDistance,
      };
    })
    .sort((left, right) => left.score - right.score);
  const directScore = directExposure * 12 + directDistance;
  const improved = candidates.filter(
    (candidate) => candidate.score <
      directScore * Math.max(0, finiteNumber(minimumImprovementRatio, 0.75)),
  );
  const selected = improved[Math.max(0, Math.trunc(finiteNumber(candidateIndex, 0)))] ?? null;
  if (!selected) return null;
  return {
    x: Math.max(0, Math.round(selected.waypoint.x)),
    y: Math.max(0, Math.round(selected.waypoint.y)),
    directExposure,
    detourExposure: selected.detourExposure,
    candidateIndex: Math.max(0, Math.trunc(finiteNumber(candidateIndex, 0))),
  };
}

/**
 * Return only quest gear that is provably superseded in the same equipment
 * slot by a later quest reward currently worn by the player. This is the
 * conservative liquidation set the autonomous client may sell for supplies.
 */
export function supersededProgressionGearForSale(state, progressionCandidates = []) {
  const questIdByName = new Map(
    (Array.isArray(progressionCandidates) ? progressionCandidates : [])
      .map((candidate) => [String(candidate?.name ?? ""), Number(candidate?.questId)])
      .filter(([name, questId]) => name && Number.isFinite(questId)),
  );
  const equippedQuestIdBySlot = new Map(
    (Array.isArray(state?.equipmentItems) ? state.equipmentItems : [])
      .map((item) => [String(item?.slot ?? ""), questIdByName.get(String(item?.name ?? ""))])
      .filter(([slot, questId]) => slot && Number.isFinite(questId)),
  );
  return (Array.isArray(state?.inventoryItems) ? state.inventoryItems : [])
    .filter((item) => {
      const itemQuestId = questIdByName.get(String(item?.name ?? ""));
      const equippedQuestId = equippedQuestIdBySlot.get(String(item?.equipSlot ?? ""));
      return item?.container === "bag1" &&
        Boolean(item?.equipSlot) &&
        Number(item?.sellValue ?? 0) > 0 &&
        Number.isFinite(itemQuestId) &&
        Number.isFinite(equippedQuestId) &&
        itemQuestId < equippedQuestId;
    })
    .sort((left, right) =>
      Number(right.sellValue ?? 0) - Number(left.sellValue ?? 0) ||
      String(left.name ?? "").localeCompare(String(right.name ?? ""))
    );
}

/**
 * Select a bag duplicate only when the same named progression item remains
 * equipped. The equipped copy is the retained item; the caller must also
 * provide an explicit merchant-backed allow-list for the duplicate loot.
 */
export function duplicateEquippedItemsForSale(state, allowedNames = []) {
  const allowed = new Set(
    (Array.isArray(allowedNames) ? allowedNames : [])
      .map(normalizePolicyName)
      .filter(Boolean),
  );
  const equippedNames = new Set(
    (Array.isArray(state?.equipmentItems) ? state.equipmentItems : [])
      .map((item) => normalizePolicyName(item?.name))
      .filter(Boolean),
  );
  return (Array.isArray(state?.inventoryItems) ? state.inventoryItems : [])
    .filter((item) => {
      const name = normalizePolicyName(item?.name);
      return item?.container === "bag1" &&
        allowed.has(name) &&
        equippedNames.has(name) &&
        Number(item?.sellValue ?? 0) > 0;
    })
    .sort((left, right) =>
      Number(right.sellValue ?? 0) - Number(left.sellValue ?? 0) ||
      String(left.name ?? "").localeCompare(String(right.name ?? ""))
    );
}

/** Select bag copies that a Crystal-data-derived catalogue classifies as
 * ordinary off-class vendor loot, retaining the merchant proof on each row. */
export function ordinarySupplyLootForSale(state, catalogue = []) {
  const byName = new Map(
    (Array.isArray(catalogue) ? catalogue : [])
      .map((entry) => [normalizePolicyName(entry?.name), entry])
      .filter(([name, entry]) => name && entry?.merchantKey),
  );
  return (Array.isArray(state?.inventoryItems) ? state.inventoryItems : [])
    .flatMap((item) => {
      const proof = byName.get(normalizePolicyName(item?.name));
      if (
        !proof || item?.container !== "bag1" ||
        Number(item?.sellValue ?? 0) <= 0
      ) return [];
      return [{ ...item, liquidationMerchantKey: String(proof.merchantKey) }];
    })
    .sort((left, right) =>
      Number(right.sellValue ?? 0) - Number(left.sellValue ?? 0) ||
      String(left.name ?? "").localeCompare(String(right.name ?? ""))
    );
}

/**
 * Select only ordinary bag material that is duplicated by an independently
 * tracked active quest-container count. Selling this copy cannot decrement the
 * authoritative quest objective, and the allow-list prevents liquidation of
 * unknown loot merely because it happens to share a name.
 */
export function surplusQuestMaterialsForSale(state, allowedNames = []) {
  const allowed = new Set(
    (Array.isArray(allowedNames) ? allowedNames : []).map(normalizePolicyName).filter(Boolean),
  );
  const inventory = Array.isArray(state?.inventoryItems) ? state.inventoryItems : [];
  const questQuantityByName = new Map();
  for (const item of inventory.filter((entry) => entry?.container === "quest")) {
    const name = normalizePolicyName(item?.name);
    if (!name) continue;
    questQuantityByName.set(
      name,
      (questQuantityByName.get(name) ?? 0) + Math.max(1, finiteNumber(item?.quantity, 1)),
    );
  }
  const activeObjectiveCurrentByName = new Map();
  for (const quest of Array.isArray(state?.questLog) ? state.questLog : []) {
    if (!["inprogress", "readytoturnin"].includes(normalizedQuestStage(quest?.stage))) continue;
    for (const objective of Array.isArray(quest?.objectives) ? quest.objectives : []) {
      const label = normalizePolicyName(objective?.label);
      for (const name of allowed) {
        if (!label.includes(name)) continue;
        activeObjectiveCurrentByName.set(
          name,
          Math.max(
            activeObjectiveCurrentByName.get(name) ?? 0,
            finiteNumber(objective?.current, 0),
          ),
        );
      }
    }
  }
  return inventory
    .filter((item) => {
      const name = normalizePolicyName(item?.name);
      const objectiveCurrent = activeObjectiveCurrentByName.get(name) ?? 0;
      return item?.container === "bag1" &&
        !item?.equipSlot &&
        allowed.has(name) &&
        Number(item?.sellValue ?? 0) > 0 &&
        objectiveCurrent > 0 &&
        (questQuantityByName.get(name) ?? 0) >= objectiveCurrent;
    })
    .sort((left, right) =>
      Number(right.sellValue ?? 0) - Number(left.sellValue ?? 0) ||
      String(left.name ?? "").localeCompare(String(right.name ?? ""))
    );
}

function normalizePolicyName(value) {
  return String(value ?? "").trim().toLowerCase().replace(/[^a-z0-9]+/g, "");
}

export function selectBestAvailableEquipmentUpgrade(state, candidates, level) {
  const inventory = Array.isArray(state?.inventoryItems) ? state.inventoryItems : [];
  const equipment = Array.isArray(state?.equipmentItems) ? state.equipmentItems : [];
  const resolvedSlots = new Set();
  for (const candidate of candidates) {
    if (Number(level) < Number(candidate.minLevel)) continue;
    const inventoryItem = inventory.find((item) => item?.name === candidate.name);
    const equippedItem = equipment.find((item) => item?.name === candidate.name);
    if (!inventoryItem && !equippedItem) continue;
    const slot = candidate.slot ?? inventoryItem?.equipSlot ?? equippedItem?.slot ?? equippedItem?.equipSlot;
    if (!slot || resolvedSlots.has(slot)) continue;
    resolvedSlots.add(slot);
    if (equippedItem) continue;
    return { ...candidate, slot };
  }
  return null;
}

/** Rank equipped items that a normal repair merchant should service. */
export function equipmentRepairCandidates(
  state,
  { thresholdRatio = 0.25, slots = [] } = {},
) {
  const allowedSlots = new Set((Array.isArray(slots) ? slots : []).map(String));
  const slotPriority = new Map([
    ["weapon", 0],
    ["armour", 1],
    ["helmet", 2],
    ["belt", 3],
    ["boots", 4],
    ["necklace", 5],
    ["braceletLeft", 6],
    ["braceletRight", 7],
    ["ringLeft", 8],
    ["ringRight", 9],
  ]);
  const threshold = Math.max(0, Math.min(1, finiteNumber(thresholdRatio, 0.25)));
  return (Array.isArray(state?.equipmentItems) ? state.equipmentItems : [])
    .filter((item) => {
      const slot = String(item?.slot ?? "");
      const current = finiteNumber(item?.durabilityCurrent, Number.NaN);
      const maximum = finiteNumber(item?.durabilityMax, Number.NaN);
      return slot &&
        (allowedSlots.size === 0 || allowedSlots.has(slot)) &&
        Number.isFinite(current) &&
        Number.isFinite(maximum) &&
        maximum > 0 &&
        current < maximum &&
        current / maximum <= threshold;
    })
    .sort((left, right) => {
      const leftRatio = finiteNumber(left.durabilityCurrent, 0) /
        Math.max(1, finiteNumber(left.durabilityMax, 1));
      const rightRatio = finiteNumber(right.durabilityCurrent, 0) /
        Math.max(1, finiteNumber(right.durabilityMax, 1));
      return leftRatio - rightRatio ||
        finiteNumber(slotPriority.get(String(left.slot)), 99) -
          finiteNumber(slotPriority.get(String(right.slot)), 99) ||
        String(left.name ?? "").localeCompare(String(right.name ?? ""));
    });
}

export const BICHON_Q1_Q5_ROUTE = Object.freeze({
  id: "warrior-bichon-q1-q5-v1",
  mapFileName: "0",
  quests: Object.freeze([1, 2, 3, 4, 5]),
  npcs: Object.freeze({
    assistant: Object.freeze({ npcIndex: 3, label: "Assistant Jane", x: 284, y: 606 }),
    craftLady: Object.freeze({ npcIndex: 4, label: "Craft Lady", x: 294, y: 619 }),
    blacksmith: Object.freeze({ npcIndex: 5, label: "Blacksmith", x: 296, y: 613 }),
    butcher: Object.freeze({ npcIndex: 6, label: "Butcher John", x: 292, y: 603 }),
  }),
  fields: Object.freeze({
    Deer: Object.freeze([
      Object.freeze({ x: 273, y: 614 }),
      Object.freeze({ x: 295, y: 625 }),
      Object.freeze({ x: 284, y: 606 }),
    ]),
    Scarecrow: Object.freeze([
      // The original beginner spawn directly south-west of the Bichon bind
      // point is the productive q2/q5 patrol. Keep it first so a failed/stale
      // AOI target does not send a level-1 character across the whole map.
      Object.freeze({ x: 270, y: 625 }),
      Object.freeze({ x: 290, y: 615 }),
      Object.freeze({ x: 300, y: 610 }),
    ]),
  }),
  equipment: Object.freeze({
    q3WarriorChoiceTarget: "@quest:finish:3:0",
    q3WarriorChoiceName: "SharpDagger",
  }),
});

export const BICHON_Q1_Q9_ROUTE = Object.freeze({
  id: "warrior-bichon-q1-q9-v1",
  mapFileName: BICHON_Q1_Q5_ROUTE.mapFileName,
  quests: Object.freeze([1, 2, 3, 4, 5, 6, 7, 8, 9]),
  npcs: Object.freeze({
    ...BICHON_Q1_Q5_ROUTE.npcs,
    masterWa: Object.freeze({ npcIndex: 10, label: "Master Wa", x: 110, y: 317 }),
    mirGuide: Object.freeze({ npcIndex: 26, label: "MirGuide Peter", x: 328, y: 258 }),
    merchantRuben: Object.freeze({ npcIndex: 8, label: "Merchant Ruben", x: 288, y: 608 }),
    materialDealerReece: Object.freeze({ npcIndex: 43, label: "Material Dealer Reece", x: 295, y: 605 }),
  }),
  fields: Object.freeze({
    ...BICHON_Q1_Q5_ROUTE.fields,
    HookingCat: Object.freeze([
      Object.freeze({ x: 340, y: 550, spread: 50 }),
      Object.freeze({ x: 180, y: 420, spread: 50 }),
      Object.freeze({ x: 150, y: 130, spread: 50 }),
      Object.freeze({ x: 110, y: 80, spread: 60 }),
      Object.freeze({ x: 510, y: 410, spread: 50 }),
    ]),
    Oma: Object.freeze([
      Object.freeze({ x: 110, y: 440, spread: 70 }),
      Object.freeze({ x: 220, y: 470, spread: 70 }),
      Object.freeze({ x: 140, y: 500, spread: 50 }),
      Object.freeze({ x: 90, y: 240, spread: 60 }),
    ]),
    RakingCat: Object.freeze([
      Object.freeze({ x: 180, y: 420, spread: 50 }),
      Object.freeze({ x: 340, y: 550, spread: 50 }),
      Object.freeze({ x: 140, y: 100, spread: 50 }),
      Object.freeze({ x: 510, y: 410, spread: 50 }),
    ]),
  }),
  equipment: Object.freeze({
    ...BICHON_Q1_Q5_ROUTE.equipment,
    q6WarriorChoiceTarget: "@quest:finish:6:0",
    q6WarriorChoiceName: "BronzeWarriorSword",
  }),
});

const ACTIVE_STAGES = new Set(["inprogress", "readytoturnin"]);

export function normalizedQuestStage(value) {
  return String(value ?? "").replace(/[^a-z]/gi, "").toLowerCase();
}

export function questState(snapshot, questId) {
  const quests = Array.isArray(snapshot?.questLog) ? snapshot.questLog : [];
  return quests.find((quest) => Number(quest?.questId) === Number(questId)) ?? null;
}

export function questStage(snapshot, questId) {
  return normalizedQuestStage(questState(snapshot, questId)?.stage);
}

export function questIsActive(snapshot, questId) {
  return ACTIVE_STAGES.has(questStage(snapshot, questId));
}

export function questIsCompleted(snapshot, questId) {
  return questStage(snapshot, questId) === "completed";
}

export function objectiveForMonster(quest, monsterName) {
  const wanted = normalizeName(monsterName);
  const objectives = Array.isArray(quest?.objectives) ? quest.objectives : [];
  return objectives.find((objective) => normalizeName(objective?.label).includes(wanted)) ?? null;
}

export function objectiveProgress(quest, monsterName) {
  const objective = objectiveForMonster(quest, monsterName);
  return {
    current: finiteNumber(objective?.current, finiteNumber(quest?.current, 0)),
    required: Math.max(1, finiteNumber(objective?.required, finiteNumber(quest?.required, 1))),
    label: objective?.label ?? null,
  };
}

/**
 * Turn a Crystal respawn rectangle into AOI-sized patrol waypoints. Respawn
 * coordinates are region centres, not promises that a monster occupies the
 * centre tile; deterministic slots can all land around the perimeter.
 */
export function expandRespawnPatrolFields(fields, { player = null, hazards = [] } = {}) {
  const result = [];
  const seen = new Set();
  for (const field of fields ?? []) {
    const centerX = finiteNumber(field?.x, 0);
    const centerY = finiteNumber(field?.y, 0);
    const spread = Math.max(0, finiteNumber(field?.spread, 0));
    const step = spread > 16 ? Math.max(12, Math.min(28, Math.floor(spread / 2))) : 0;
    const approach = nearestRespawnApproachPoint(player, field);
    const approachOffset = [approach.x - centerX, approach.y - centerY];
    let offsets = step > 0
      ? [
          approachOffset,
          [0, 0],
          [-step, -step], [step, -step],
          [-step, step], [step, step],
          [-step, 0], [step, 0], [0, -step], [0, step],
        ]
      : [[0, 0]];
    if (offsets.length > 1 && Array.isArray(hazards) && hazards.length > 0) {
      // Keep the nearest interior edge first, then visit the remaining AOI
      // patrol samples from lower terminal overlap to higher overlap. The old
      // centre-first order could walk directly from an empty safe edge into a
      // coincident Oma/Yeti pack even when another sample in the same
      // authoritative respawn rectangle was both safer and just as valid.
      const [approachFirst, ...remaining] = offsets;
      const riskSpread = Math.max(8, spread * 0.15);
      const pointForOffset = ([dx, dy]) => ({
        ...field,
        x: centerX + dx,
        y: centerY + dy,
        spread: riskSpread,
      });
      const playerDistance = (point) => player
        ? Math.max(
            Math.abs(finiteNumber(point.x, 0) - finiteNumber(player.x, 0)),
            Math.abs(finiteNumber(point.y, 0) - finiteNumber(player.y, 0)),
          )
        : 0;
      remaining.sort((left, right) => {
        const leftPoint = pointForOffset(left);
        const rightPoint = pointForOffset(right);
        return respawnTerminalExposure(leftPoint, hazards) -
            respawnTerminalExposure(rightPoint, hazards) ||
          playerDistance(leftPoint) - playerDistance(rightPoint);
      });
      offsets = [approachFirst, ...remaining];
    }
    for (const [dx, dy] of offsets) {
      const point = {
        ...field,
        x: Math.max(0, Math.round(centerX + dx)),
        y: Math.max(0, Math.round(centerY + dy)),
        patrolCenterX: Math.max(0, Math.round(centerX)),
        patrolCenterY: Math.max(0, Math.round(centerY)),
      };
      const key = `${String(point.mapFileName ?? "")}:${point.x}:${point.y}`;
      if (seen.has(key)) continue;
      seen.add(key);
      result.push(point);
    }
  }
  return result;
}

/**
 * Preserve a locally planned endpoint when the first executable step must
 * temporarily move sideways or away from the final target. Replanning from
 * every corrected tile without this hint can undo that first step forever at
 * a concave wall even though the complete collision path makes progress.
 */
export function collisionPathNeedsStickyDetour(player, target, path) {
  if (!player || !target || !Array.isArray(path) || path.length < 2) return false;
  const endpoint = path[path.length - 1];
  const distance = (left, right) => Math.max(
    Math.abs(finiteNumber(left?.x, 0) - finiteNumber(right?.x, 0)),
    Math.abs(finiteNumber(left?.y, 0) - finiteNumber(right?.y, 0)),
  );
  const currentDistance = distance(player, target);
  return distance(endpoint, target) < currentDistance &&
    distance(path[1], target) >= currentDistance;
}

/**
 * Freeze a dynamic-avoidance path only at an endpoint that makes net progress
 * toward the real destination. The executable path may still move sideways or
 * backwards around an actor, but retaining a regressive 32-step endpoint for
 * the full sticky TTL turns a transient avoidance manoeuvre into a long detour.
 */
export function selectProgressingCollisionDetour(
  path,
  player,
  target,
  { preferredSteps = 32 } = {},
) {
  if (!Array.isArray(path) || path.length < 2 || !player || !target) return null;
  const distance = (point) => Math.max(
    Math.abs(finiteNumber(point?.x, 0) - finiteNumber(target.x, 0)),
    Math.abs(finiteNumber(point?.y, 0) - finiteNumber(target.y, 0)),
  );
  const currentDistance = distance(player);
  const preferredIndex = Math.min(
    Math.max(1, Math.trunc(finiteNumber(preferredSteps, 32))),
    path.length - 1,
  );
  const preferred = path[preferredIndex];
  if (preferred && distance(preferred) < currentDistance) {
    return { x: finiteNumber(preferred.x, 0), y: finiteNumber(preferred.y, 0) };
  }
  const firstProgress = path
    .slice(preferredIndex + 1)
    .find((point) => distance(point) < currentDistance);
  return firstProgress
    ? { x: finiteNumber(firstProgress.x, 0), y: finiteNumber(firstProgress.y, 0) }
    : null;
}

/**
 * A target outside the loaded collision region should normally be approached
 * through the region edge facing it. If a wall prevents the best reachable
 * point from touching that edge, continuing to replan toward that interior
 * point oscillates at the wall. In that case the navigator must first reach a
 * perpendicular region edge so the next ordinary movement chunk can reveal a
 * route around the obstacle.
 */
export function collisionPathNeedsPerpendicularFrontier(player, target, bounds, endpoint) {
  if (!player || !target || !bounds || !endpoint) return false;
  const targetOutside =
    finiteNumber(target.x, 0) < finiteNumber(bounds.minX, 0) ||
    finiteNumber(target.x, 0) > finiteNumber(bounds.maxX, 0) ||
    finiteNumber(target.y, 0) < finiteNumber(bounds.minY, 0) ||
    finiteNumber(target.y, 0) > finiteNumber(bounds.maxY, 0);
  if (!targetOutside) return false;

  const dx = finiteNumber(target.x, 0) - finiteNumber(player.x, 0);
  const dy = finiteNumber(target.y, 0) - finiteNumber(player.y, 0);
  if (Math.abs(dx) >= Math.abs(dy)) {
    return dx >= 0
      ? finiteNumber(endpoint.x, 0) < finiteNumber(bounds.maxX, 0) - 1
      : finiteNumber(endpoint.x, 0) > finiteNumber(bounds.minX, 0) + 1;
  }
  return dy >= 0
    ? finiteNumber(endpoint.y, 0) < finiteNumber(bounds.maxY, 0) - 1
    : finiteNumber(endpoint.y, 0) > finiteNumber(bounds.minY, 0) + 1;
}

/**
 * Expand a long-route collision search only after its cheap local corridors
 * fail. Small Crystal maps can afford one true full-map fallback; larger maps
 * retain a bounded adaptive fallback so a short route never allocates an
 * unbounded world-sized BFS merely because a distant wall exists.
 */
export function collisionAtlasSearchMargins(
  distance,
  {
    mapWidth = null,
    mapHeight = null,
    fullMapCellLimit = 1_000_000,
    maximumFallbackMargin = 700,
  } = {},
) {
  const routeDistance = Math.max(0, finiteNumber(distance, 0));
  const baseMargin = Math.min(160, Math.max(72, Math.ceil(routeDistance * 0.25)));
  const expandedMargin = Math.min(350, Math.max(240, baseMargin * 2));
  const width = Math.max(0, Math.floor(finiteNumber(mapWidth, 0)));
  const height = Math.max(0, Math.floor(finiteNumber(mapHeight, 0)));
  const fullMapEligible = width > 0 && height > 0 &&
    width * height <= Math.max(1, finiteNumber(fullMapCellLimit, 1_000_000));
  const fallbackMargin = fullMapEligible
    ? Math.max(width, height)
    : Math.min(
        Math.max(1, finiteNumber(maximumFallbackMargin, 700)),
        Math.max(384, Math.ceil(routeDistance * 2)),
      );
  return [...new Set([baseMargin, expandedMargin, fallbackMargin])]
    .filter((margin) => Number.isFinite(margin) && margin > 0)
    .sort((left, right) => left - right);
}

/**
 * Find a shortest eight-direction walk over a fully observed static collision
 * rectangle. This is intentionally a pure policy helper: the browser runner
 * obtains those cells from the same map asset endpoint used by the real
 * client, then executes the returned path only with ordinary direction input.
 */
export function findCollisionGridPath({
  start,
  target,
  desiredDistance = 0,
  bounds,
  blocked = [],
  occupied = [],
}) {
  if (!start || !target || !bounds) return null;
  const minX = Math.trunc(finiteNumber(bounds.minX, 0));
  const maxX = Math.trunc(finiteNumber(bounds.maxX, -1));
  const minY = Math.trunc(finiteNumber(bounds.minY, 0));
  const maxY = Math.trunc(finiteNumber(bounds.maxY, -1));
  const width = maxX - minX + 1;
  const height = maxY - minY + 1;
  if (width <= 0 || height <= 0 || width * height > 1_500_000) return null;

  const sx = Math.trunc(finiteNumber(start.x, minX));
  const sy = Math.trunc(finiteNumber(start.y, minY));
  const tx = Math.trunc(finiteNumber(target.x, sx));
  const ty = Math.trunc(finiteNumber(target.y, sy));
  const inBounds = (x, y) => x >= minX && x <= maxX && y >= minY && y <= maxY;
  if (!inBounds(sx, sy)) return null;
  const indexOf = (x, y) => (y - minY) * width + (x - minX);
  const pointFor = (index) => ({
    x: minX + index % width,
    y: minY + Math.floor(index / width),
  });
  const blockedGrid = new Uint8Array(width * height);
  for (const value of [...blocked, ...occupied]) {
    const point = collisionGridPoint(value);
    if (point && inBounds(point.x, point.y)) blockedGrid[indexOf(point.x, point.y)] = 1;
  }
  const startIndex = indexOf(sx, sy);
  blockedGrid[startIndex] = 0;

  const predecessor = new Int32Array(width * height);
  predecessor.fill(-2);
  predecessor[startIndex] = -1;
  const queue = new Int32Array(width * height);
  queue[0] = startIndex;
  let head = 0;
  let tail = 1;
  let goalIndex = -1;
  const distanceToTarget = (x, y) => Math.max(Math.abs(tx - x), Math.abs(ty - y));
  const dx = Math.sign(tx - sx);
  const dy = Math.sign(ty - sy);
  const directions = collisionGridDirections(dx, dy);

  while (head < tail) {
    const currentIndex = queue[head++];
    const current = pointFor(currentIndex);
    if (distanceToTarget(current.x, current.y) <= Math.max(0, desiredDistance)) {
      goalIndex = currentIndex;
      break;
    }
    for (const [stepX, stepY] of directions) {
      const nextX = current.x + stepX;
      const nextY = current.y + stepY;
      if (!inBounds(nextX, nextY)) continue;
      const nextIndex = indexOf(nextX, nextY);
      if (blockedGrid[nextIndex] || predecessor[nextIndex] !== -2) continue;
      if (stepX !== 0 && stepY !== 0) {
        const horizontalIndex = indexOf(current.x + stepX, current.y);
        const verticalIndex = indexOf(current.x, current.y + stepY);
        if (blockedGrid[horizontalIndex] || blockedGrid[verticalIndex]) continue;
      }
      predecessor[nextIndex] = currentIndex;
      queue[tail++] = nextIndex;
    }
  }
  if (goalIndex < 0) return null;

  const path = [];
  for (let index = goalIndex; index >= 0; index = predecessor[index]) {
    path.push(pointFor(index));
  }
  path.reverse();
  return path;
}

/**
 * Continuous run input is less precise than the unit-cell collision path it
 * follows: one accepted Run advances two cells and a key-hold can have one
 * final command in flight while the release is being processed. Keep that
 * gesture away from every non-target map transfer. Near a doorway the runner
 * falls back to one audited direction step and replans from the authoritative
 * position, so it can never trade route speed for an accidental map change.
 */
export function continuousCollisionRunAvoidsTransfers({
  start,
  direction,
  plannedSteps,
  mapTransfers = [],
  safetyRadius = 3,
  releaseOvershootSteps = 2,
}) {
  const startX = Math.trunc(finiteNumber(start?.x, Number.NaN));
  const startY = Math.trunc(finiteNumber(start?.y, Number.NaN));
  const stepX = Math.sign(finiteNumber(direction?.x, 0));
  const stepY = Math.sign(finiteNumber(direction?.y, 0));
  const steps = Math.max(0, Math.trunc(finiteNumber(plannedSteps, 0)));
  if (
    !Number.isFinite(startX) || !Number.isFinite(startY) ||
    (stepX === 0 && stepY === 0) || steps === 0
  ) return false;

  const radius = Math.max(0, Math.trunc(finiteNumber(safetyRadius, 3)));
  const overshoot = Math.max(0, Math.trunc(finiteNumber(releaseOvershootSteps, 2)));
  const transfers = (Array.isArray(mapTransfers) ? mapTransfers : [])
    .map((transfer) => ({
      minX: Math.trunc(finiteNumber(transfer?.minX, Number.NaN)),
      maxX: Math.trunc(finiteNumber(transfer?.maxX, Number.NaN)),
      minY: Math.trunc(finiteNumber(transfer?.minY, Number.NaN)),
      maxY: Math.trunc(finiteNumber(transfer?.maxY, Number.NaN)),
    }))
    .filter((transfer) => Object.values(transfer).every(Number.isFinite));
  if (!transfers.length) return true;

  for (let index = 0; index <= steps + overshoot; index += 1) {
    const x = startX + stepX * index;
    const y = startY + stepY * index;
    const touchesProtectedTransfer = transfers.some((transfer) => (
      x >= transfer.minX - radius && x <= transfer.maxX + radius &&
      y >= transfer.minY - radius && y <= transfer.maxY + radius
    ));
    if (touchesProtectedTransfer) return false;
  }
  return true;
}

/**
 * Crystal often represents one doorway as several adjacent MovementInfo rows.
 * While intentionally travelling to a destination, every source cell in that
 * destination cluster is a valid physical entrance. Protecting the siblings
 * of the currently nearest row can surround that row with artificial walls
 * and make a real doorway mathematically unreachable.
 */
export function protectedTransfersForNavigation(
  mapTransfers = [],
  allowedDestinationMapFileName = null,
) {
  const transfers = Array.isArray(mapTransfers) ? mapTransfers : [];
  if (allowedDestinationMapFileName == null) return [...transfers];
  const allowed = String(allowedDestinationMapFileName);
  return transfers.filter(
    (transfer) => String(transfer?.toMapFileName ?? "") !== allowed,
  );
}

/**
 * A long static route should not be replaced merely because a moving actor is
 * somewhere far along it. Inspect a bounded lookahead of physical steps
 * (including both orthogonal corner cells for diagonals); occupancy beyond
 * that window is re-evaluated after later authoritative movement.
 */
export function collisionPathHasImmediateDynamicBlock(path, occupied = [], lookaheadSteps = 1) {
  if (!Array.isArray(path) || path.length < 2) return false;
  const occupiedKeys = new Set(
    (Array.isArray(occupied) ? occupied : [])
      .map(collisionGridPoint)
      .filter(Boolean)
      .map((point) => `${point.x},${point.y}`),
  );
  const inspected = [];
  const limit = Math.min(
    path.length - 1,
    Math.max(1, Math.trunc(finiteNumber(lookaheadSteps, 1))),
  );
  for (let index = 1; index <= limit; index += 1) {
    const current = collisionGridPoint(path[index - 1]);
    const next = collisionGridPoint(path[index]);
    if (!current || !next) continue;
    inspected.push(`${next.x},${next.y}`);
    if (next.x !== current.x && next.y !== current.y) {
      inspected.push(`${next.x},${current.y}`, `${current.x},${next.y}`);
    }
  }
  return inspected.some((key) => occupiedKeys.has(key));
}

function collisionGridPoint(value) {
  if (value && typeof value === "object") {
    const x = Number(value.x);
    const y = Number(value.y);
    return Number.isFinite(x) && Number.isFinite(y) ? { x: Math.trunc(x), y: Math.trunc(y) } : null;
  }
  const [rawX, rawY] = String(value ?? "").split(",", 2);
  const x = Number(rawX);
  const y = Number(rawY);
  return Number.isFinite(x) && Number.isFinite(y) ? { x: Math.trunc(x), y: Math.trunc(y) } : null;
}

function collisionGridDirections(dx, dy) {
  const preferred = [
    [dx, dy], [dx, 0], [0, dy],
    [dx, -dy], [-dx, dy],
    [0, -dy], [-dx, 0], [-dx, -dy],
  ];
  const fallback = [
    [0, -1], [1, 0], [0, 1], [-1, 0],
    [1, -1], [1, 1], [-1, 1], [-1, -1],
  ];
  const seen = new Set();
  return [...preferred, ...fallback].filter(([stepX, stepY]) => {
    if (stepX === 0 && stepY === 0) return false;
    const key = `${stepX},${stepY}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function allQ1Q5Completed(snapshot) {
  return BICHON_Q1_Q5_ROUTE.quests.every((questId) => questIsCompleted(snapshot, questId));
}

export function allQ1Q9Completed(snapshot) {
  return BICHON_Q1_Q9_ROUTE.quests.every((questId) => questIsCompleted(snapshot, questId));
}

/**
 * Returns the next semantic goal. The order mirrors the original Crystal
 * beginner chain while accepting q5 early so q2/q4 kills count naturally.
 */
export function planNextQ1Q5(snapshot) {
  const q1 = questState(snapshot, 1);
  if (!q1 || normalizedQuestStage(q1.stage) === "available") {
    return talkGoal("accept", 1, "assistant");
  }
  if (normalizedQuestStage(q1.stage) === "readytoturnin") {
    return talkGoal("finish", 1, "craftLady");
  }
  if (!questIsCompleted(snapshot, 1)) {
    return waitGoal(1, "q1 is active but not ready; wait for authoritative quest refresh");
  }

  const q2 = questState(snapshot, 2);
  if (!q2 || normalizedQuestStage(q2.stage) === "available") {
    return talkGoal("accept", 2, "craftLady");
  }

  const q5 = questState(snapshot, 5);
  if ((!q5 || normalizedQuestStage(q5.stage) === "available") && questIsActive(snapshot, 2)) {
    return talkGoal("accept", 5, "blacksmith");
  }

  if (normalizedQuestStage(q2.stage) === "inprogress") {
    return huntGoal(2, "Scarecrow", false);
  }
  if (normalizedQuestStage(q2.stage) === "readytoturnin") {
    return talkGoal("finish", 2, "assistant");
  }
  if (!questIsCompleted(snapshot, 2)) {
    return waitGoal(2, "q2 is neither active nor completed");
  }

  const q3 = questState(snapshot, 3);
  if (!q3 || normalizedQuestStage(q3.stage) === "available") {
    return talkGoal("accept", 3, "assistant");
  }
  if (normalizedQuestStage(q3.stage) === "readytoturnin") {
    return {
      ...talkGoal("finish", 3, "butcher"),
      rewardChoiceTarget: BICHON_Q1_Q5_ROUTE.equipment.q3WarriorChoiceTarget,
    };
  }
  if (!questIsCompleted(snapshot, 3)) {
    return waitGoal(3, "q3 is active but not ready; wait for authoritative quest refresh");
  }

  const q4 = questState(snapshot, 4);
  if (!q4 || normalizedQuestStage(q4.stage) === "available") {
    return talkGoal("accept", 4, "butcher");
  }
  if (normalizedQuestStage(q4.stage) === "inprogress") {
    return huntGoal(4, "Deer", true);
  }
  if (normalizedQuestStage(q4.stage) === "readytoturnin") {
    return talkGoal("finish", 4, "butcher");
  }
  if (!questIsCompleted(snapshot, 4)) {
    return waitGoal(4, "q4 is neither active nor completed");
  }

  const latestQ5 = questState(snapshot, 5);
  if (!latestQ5 || normalizedQuestStage(latestQ5.stage) === "available") {
    return talkGoal("accept", 5, "blacksmith");
  }
  if (normalizedQuestStage(latestQ5.stage) === "inprogress") {
    const deer = objectiveProgress(latestQ5, "Deer");
    const scarecrow = objectiveProgress(latestQ5, "Scarecrow");
    const deerRemaining = Math.max(0, deer.required - deer.current);
    const scarecrowRemaining = Math.max(0, scarecrow.required - scarecrow.current);
    if (deerRemaining === 0 && scarecrowRemaining === 0) {
      return waitGoal(5, "q5 objective counts are complete; wait for ready-to-turn-in refresh");
    }
    return huntGoal(5, deerRemaining >= scarecrowRemaining ? "Deer" : "Scarecrow", false);
  }
  if (normalizedQuestStage(latestQ5.stage) === "readytoturnin") {
    return talkGoal("finish", 5, "blacksmith");
  }
  if (questIsCompleted(snapshot, 5) && allQ1Q5Completed(snapshot)) {
    return { kind: "done", questIds: [...BICHON_Q1_Q5_ROUTE.quests] };
  }
  return waitGoal(5, "q5 is in an unexpected state");
}

/** Continue the original fresh-Warrior beginner arc after the proven q1-q5 slice. */
export function planNextQ1Q9(snapshot) {
  if (!allQ1Q5Completed(snapshot)) return planNextQ1Q5(snapshot);

  const q6 = questState(snapshot, 6);
  if (!q6 || normalizedQuestStage(q6.stage) === "available") {
    return talkGoal("accept", 6, "blacksmith");
  }
  if (normalizedQuestStage(q6.stage) === "inprogress") {
    return huntGoal(6, "HookingCat", false);
  }
  if (normalizedQuestStage(q6.stage) === "readytoturnin") {
    return {
      ...talkGoal("finish", 6, "blacksmith"),
      rewardChoiceTarget: BICHON_Q1_Q9_ROUTE.equipment.q6WarriorChoiceTarget,
    };
  }
  if (!questIsCompleted(snapshot, 6)) return waitGoal(6, "q6 is in an unexpected state");

  const q7 = questState(snapshot, 7);
  if (!q7 || normalizedQuestStage(q7.stage) === "available") {
    return talkGoal("accept", 7, "assistant");
  }
  if (normalizedQuestStage(q7.stage) === "readytoturnin") {
    return talkGoal("finish", 7, "masterWa");
  }
  if (!questIsCompleted(snapshot, 7)) return waitGoal(7, "q7 is in an unexpected state");

  const q8 = questState(snapshot, 8);
  if (!q8 || normalizedQuestStage(q8.stage) === "available") {
    return talkGoal("accept", 8, "masterWa");
  }
  if (normalizedQuestStage(q8.stage) === "inprogress") {
    const oma = objectiveProgress(q8, "Oma");
    const rakingCat = objectiveProgress(q8, "RakingCat");
    const omaRemaining = Math.max(0, oma.required - oma.current);
    const rakingCatRemaining = Math.max(0, rakingCat.required - rakingCat.current);
    if (omaRemaining === 0 && rakingCatRemaining === 0) {
      return waitGoal(8, "q8 objective counts are complete; wait for ready-to-turn-in refresh");
    }
    // q8's two populations overlap only partially across a very large map.
    // Keep fighting a live, still-needed target already in the player's AOI
    // instead of crossing Bichon after every alternating objective count.
    const visibleNeededMonster = firstVisibleMonster(snapshot, [
      ...(omaRemaining > 0 ? ["Oma"] : []),
      ...(rakingCatRemaining > 0 ? ["RakingCat"] : []),
    ]);
    if (visibleNeededMonster) return huntGoal(8, visibleNeededMonster, false);
    return huntGoal(8, omaRemaining >= rakingCatRemaining ? "Oma" : "RakingCat", false);
  }
  if (normalizedQuestStage(q8.stage) === "readytoturnin") {
    return talkGoal("finish", 8, "masterWa");
  }
  if (!questIsCompleted(snapshot, 8)) return waitGoal(8, "q8 is in an unexpected state");

  const q9 = questState(snapshot, 9);
  if (!q9 || normalizedQuestStage(q9.stage) === "available") {
    return talkGoal("accept", 9, "masterWa");
  }
  if (normalizedQuestStage(q9.stage) === "readytoturnin") {
    return talkGoal("finish", 9, "mirGuide");
  }
  if (questIsCompleted(snapshot, 9) && allQ1Q9Completed(snapshot)) {
    return { kind: "done", questIds: [...BICHON_Q1_Q9_ROUTE.quests] };
  }
  return waitGoal(9, "q9 is in an unexpected state");
}

export function auditOutgoingBrowserCommand(command, { recentInputs = [] } = {}) {
  if (!command || typeof command !== "object") return { ok: true, ignored: true };
  const type = String(command.type ?? "");
  if (type === "transferMap" && hasUiInputCorrelation(recentInputs, {
    action: "enter-visible-map-transfer",
    transferKey: String(command.key ?? ""),
  })) {
    return { ok: true, uiGenerated: true };
  }
  if (type === "acceptQuest" && hasUiInputCorrelation(recentInputs, {
    action: "quest-diary-accept",
    questId: Number(command.questIndex),
  })) {
    return { ok: true, uiGenerated: true };
  }
  if (type === "finishQuest" && hasUiInputCorrelation(recentInputs, {
    action: "quest-diary-finish",
    questId: Number(command.questIndex),
    selectedItemIndex: Number(command.selectedItemIndex ?? -1),
  })) {
    return { ok: true, uiGenerated: true };
  }
  if (QUEST_AGENT_CONTRACT.forbiddenClientCommands.includes(type)) {
    return { ok: false, reason: `forbidden direct client command: ${type}` };
  }
  if (type === "chat") {
    const message = String(command.message ?? "").trim().toUpperCase();
    const prefix = QUEST_AGENT_CONTRACT.forbiddenChatPrefixes.find((candidate) => message.startsWith(candidate));
    if (prefix) return { ok: false, reason: `forbidden privileged chat prefix: ${prefix}` };
  }
  return { ok: true };
}

function hasUiInputCorrelation(inputs, expected) {
  return inputs.some((input) => {
    if (input?.kind !== "mouse" || input?.action !== expected.action) return false;
    if (expected.transferKey !== undefined && String(input.transferKey ?? "") !== expected.transferKey) {
      return false;
    }
    if (expected.questId !== undefined && Number(input.questId) !== expected.questId) return false;
    if (
      expected.selectedItemIndex !== undefined &&
      Number(input.selectedItemIndex ?? -1) !== expected.selectedItemIndex
    ) return false;
    return true;
  });
}

function talkGoal(action, questId, npcKey) {
  return {
    kind: "talk",
    action,
    questId,
    npcKey,
    target: `@quest:${action}:${questId}`,
  };
}

function huntGoal(questId, monsterName, harvest) {
  return { kind: "hunt", questId, monsterName, harvest };
}

function waitGoal(questId, reason) {
  return { kind: "wait", questId, reason };
}

function firstVisibleMonster(snapshot, names) {
  const wanted = new Map(names.map((name) => [normalizeName(name), name]));
  for (const entity of snapshot?.entities ?? []) {
    if (!entityIsLiveActor(entity) || String(entity?.kind ?? "").toLowerCase() !== "monster") continue;
    const match = wanted.get(normalizeName(entity?.name));
    if (match) return match;
  }
  return null;
}

function normalizeName(value) {
  return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase();
}

function finiteNumber(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}
