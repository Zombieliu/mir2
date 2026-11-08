const ENGAGEMENT_TTL_MS = 6_000;
const HP_POTIONS = new Set(["(HP)DrugSmall", "(HP)DrugMedium", "(HP)DrugLarge", "(HP)DrugXL"]);
const validPoint = value => Number.isInteger(value?.x) && Number.isInteger(value?.y);
const distance = (left, right) => Math.max(Math.abs(left.x - right.x), Math.abs(left.y - right.y));
const live = entity => entity && entity.dead !== true && !(typeof entity.hp === "number" && entity.hp <= 0);
const monster = entity => entity?.kind === "monster" && live(entity) && entity.disposition !== "friendly";
const validUid = value => typeof value === "bigint" ? value > 0n && value <= 0xffff_ffff_ffff_ffffn : Number.isSafeInteger(value) && value > 0;
const validObjectId = value => Number.isInteger(value) && value >= 0 && value <= 0xffff_ffff;
const memberNames = members => new Set((Array.isArray(members) ? members : []).map(value => typeof value === "string" ? value : value?.name).filter(value => typeof value === "string"));
const knownHealth = value => Number.isFinite(value?.hp) && value.hp >= 0 && Number.isFinite(value?.maxHp) && value.maxHp > 0;

export function facingDirection(from, to) {
  return [
    ["UpLeft", "Up", "UpRight"],
    ["Left", "Down", "Right"],
    ["DownLeft", "Down", "DownRight"],
  ][Math.sign(to.y - from.y) + 1][Math.sign(to.x - from.x) + 1];
}

/** Pure rules policy. Feedback affects preferences; actual server observations decide facts. */
export function decideCompanionAction({ state, policy, leaderName, nowMs = Date.now(), homePosition = null, followDistance = 2, leashDistance = 6 }) {
  const policyVersion = Number.isSafeInteger(policy?.policyVersion) ? policy.policyVersion : 0;
  const result = (kind, reason, fields = {}) => ({ kind, reason, policyVersion, ...fields });
  if (!Number.isFinite(nowMs)) return result("wait", "clock-unknown");
  if (!Number.isInteger(followDistance) || !Number.isInteger(leashDistance) || followDistance < 1 || leashDistance < followDistance || leashDistance > 16) return result("wait", "invalid-party-distance-config");
  if (!state?.ready || !validPoint(state.player)) return result("wait", "world-not-ready");
  const self = state.player;
  const hp = state.playerHp;
  const maxHp = state.playerMaxHp;
  if (self.dead === true || hp === 0) return result("wait", "dead");
  if (!Number.isFinite(hp) || !Number.isFinite(maxHp) || hp < 0 || maxHp <= 0) return result("wait", "health-unknown");
  const mapAllowlist = Array.isArray(policy?.permissions?.mapAllowlist) ? policy.permissions.mapAllowlist : ["0", "0141"];
  if (!mapAllowlist.includes(state.mapFileName)) return result("wait", "map-not-authorized");
  const ratio = hp / maxHp;
  const entities = (Array.isArray(state.entities) ? state.entities : []).filter(value => validPoint(value) && Number.isInteger(value.objectId));
  const byId = new Map(entities.map(value => [value.objectId, value]));
  const localOwnId = state.playerObjectId ?? self.objectId;
  // selfZoneObjectId is an explicit trusted Gateway binding, never inferred
  // from unknown attackers or drop owners that happen to have another ID.
  const ownId = validObjectId(state.selfZoneObjectId) ? state.selfZoneObjectId : localOwnId;
  const ownIds = new Set([localOwnId, ownId].filter(validObjectId));
  if (ownId !== localOwnId) byId.set(ownId, self);
  const selfThreats = new Map();
  const supporting = new Map();
  const names = memberNames(state.party?.members);
  const grouped = typeof leaderName === "string" && names.has(leaderName) && names.has(self.name);
  const leader = grouped ? entities.find(value => value.kind === "player" && value.name === leaderName && live(value)) : null;
  const allies = new Set(ownIds);
  if (grouped) for (const entity of entities) if (entity.kind === "player" && names.has(entity.name)) allies.add(entity.objectId);
  for (const engagement of Array.isArray(state.engagements) ? state.engagements : []) {
    if (!Number.isFinite(engagement.observedAt) || engagement.observedAt > nowMs || nowMs - engagement.observedAt > ENGAGEMENT_TTL_MS) continue;
    const attacker = byId.get(engagement.attackerId);
    const target = byId.get(engagement.targetId);
    if (ownIds.has(engagement.targetId) && monster(attacker)) selfThreats.set(attacker.objectId, attacker);
    if (allies.has(engagement.attackerId) && monster(target)) supporting.set(target.objectId, target);
    if (allies.has(engagement.targetId) && monster(attacker)) supporting.set(attacker.objectId, attacker);
  }
  const adjacentHostiles = entities.filter(value => monster(value) && value.disposition === "hostile" && distance(self, value) <= 1);
  const immediateThreats = new Map([...selfThreats, ...adjacentHostiles.map(value => [value.objectId, value])]);
  const retreat = reason => {
    let target = validPoint(homePosition) && (!homePosition.mapFileName || homePosition.mapFileName === state.mapFileName) ? { x: homePosition.x, y: homePosition.y } : null;
    if (!target) {
      const threat = [...immediateThreats.values()].sort((a, b) => distance(self, a) - distance(self, b))[0];
      if (threat) target = { x: Math.max(0, self.x + Math.sign(self.x - threat.x) * 4), y: Math.max(0, self.y + Math.sign(self.y - threat.y) * 4) };
    }
    return target ? result("retreat", reason, { target, desiredDistance: 0 }) : result("wait", "retreat-destination-unknown");
  };
  const potion = state.inventoryStale === true ? null : [...(state.beltItems ?? []), ...(state.inventoryItems ?? [])].find(item =>
    HP_POTIONS.has(item.name) && validUid(item.uniqueId) && Number.isSafeInteger(item.quantity) && item.quantity > 0 &&
    ["belt", "bag1", "bag2"].includes(item.container));
  if (ratio <= 0.65 && potion) return result("usePotion", "health-low-use-owned-potion", { uniqueId: potion.uniqueId, grid: potion.container === "belt" ? "belt" : "inventory" });
  if (state.recallRequested === true) return retreat("owner-recall");
  if (ratio <= 0.35 || immediateThreats.size >= 2 || (ratio <= 0.65 && !potion)) return retreat("unsafe-withdraw");

  const pendingInvite = state.party?.pendingInvite;
  const inviteFresh = Number.isFinite(pendingInvite?.observedAt) && pendingInvite.observedAt <= nowMs && nowMs - pendingInvite.observedAt <= 30_000;
  if (!grouped && inviteFresh && pendingInvite.name === leaderName && typeof leaderName === "string" && leaderName.length) return result("acceptInvite", "configured-leader-invited", { leaderName });
  if (grouped && !leader) return result("wait", "leader-not-visible");

  const supportTargets = [...supporting.values()]
    .filter(value => !grouped || (leader && distance(leader, value) <= leashDistance))
    .sort((a, b) => distance(self, a) - distance(self, b) || a.objectId - b.objectId);
  const combatActive = supportTargets.length > 0 || immediateThreats.size > 0;
  const canTake = drop => {
    if (!validPoint(drop) || !Number.isInteger(drop.objectId) || distance(self, drop) > 3) return false;
    const ownedByOther = drop.ownerObjectId != null && !allies.has(drop.ownerObjectId);
    // Unknown expiry is not evidence that a foreign ownership lock ended.
    return !ownedByOther || drop.ownershipRemainingTicks != null && drop.ownershipRemainingTicks <= 0;
  };
  const drops = (Array.isArray(state.groundDrops) ? state.groundDrops : [])
    .filter(canTake).sort((a, b) => distance(self, a) - distance(self, b) || a.objectId - b.objectId);
  const capacityBlocked = state.freeBagSlots === 0 || state.currentWeight >= state.maxWeight && state.maxWeight > 0;
  const drop = capacityBlocked ? null : drops.find(value => !grouped || distance(leader, value) <= leashDistance);
  const outsideLeashDrop = !capacityBlocked && grouped ? drops.find(value => distance(leader, value) > leashDistance) : null;
  const loot = () => result("loot", "collect-observed-allowed-drop", { targetId: drop.objectId, target: { x: drop.x, y: drop.y }, desiredDistance: 0 });
  const attack = target => result("attack", grouped ? "assist-confirmed-party-engagement" : "defend-confirmed-engagement", { targetId: target.objectId, target: { x: target.x, y: target.y }, direction: facingDirection(self, target), desiredDistance: 1 });
  // ObjectPlayer does not carry HP. GroupMemberInfo is the ordinary roster's
  // health source; a matching known roster value wins over an older AOI value.
  const leaderInfo = (Array.isArray(state.party?.memberInfo) ? state.party.memberInfo : []).find(value => value?.name === leaderName);
  const leaderHealth = knownHealth(leaderInfo) ? leaderInfo : leader;
  const leaderHealthy = leader && leaderInfo?.online !== false && knownHealth(leaderHealth) && leaderHealth.hp / leaderHealth.maxHp >= 0.7;
  const collectionSafe = ratio >= 0.8 && immediateThreats.size === 0 && (!grouped || leaderHealthy);
  if (grouped) {
    if (policy?.party?.assistBeforeLoot === true && supportTargets.length) return attack(supportTargets[0]);
    if (drop && collectionSafe && (policy?.party?.lootOnlyAfterCombat !== true || !combatActive)) return loot();
    if (supportTargets.length) return attack(supportTargets[0]);
    if (policy?.party?.askBeforeLeaving === true && outsideLeashDrop && collectionSafe && !combatActive && distance(self, leader) <= leashDistance) {
      return result("requestApproval", "loot-would-leave-party-leash", { targetId: outsideLeashDrop.objectId, target: { x: outsideLeashDrop.x, y: outsideLeashDrop.y }, leaderName });
    }
    if (distance(self, leader) > Math.max(1, followDistance)) return result("follow", "stay-with-party-leader", { target: { x: leader.x, y: leader.y }, desiredDistance: Math.max(1, followDistance) });
    if (drop && !combatActive && ratio >= 0.8) return loot();
    return result("wait", combatActive ? "party-combat-still-active" : "near-party-leader");
  }
  if (immediateThreats.size) return attack([...immediateThreats.values()].sort((a, b) => distance(self, a) - distance(self, b))[0]);
  if (policy?.solo?.considerLootWhenSafe !== false && drop && collectionSafe) return loot();
  return result("wait", "no-authorized-party-activity");
}
