import { equipHeldAmulet } from './protocol-supplies.mjs';
import { selfActionBlockMask } from './protocol-status.mjs';

const CLASS_MASK = Object.freeze({ warrior: 1, wizard: 2, taoist: 4 });
const CLASS_ATTACK_STATS = Object.freeze({ warrior: [4, 5], wizard: [6, 7], taoist: [8, 9] });
const SLOT_INDEX = Object.freeze({
  weapon: 0, armour: 1, helmet: 2, torch: 3, necklace: 4,
  braceletleft: 5, braceletright: 6, ringleft: 7, ringright: 8,
  amulet: 9, belt: 10, boots: 11, stone: 12, mount: 13,
});
const OPPOSITE_PAIRED_SLOT = Object.freeze({
  braceletleft: "braceletright", braceletright: "braceletleft",
  ringleft: "ringright", ringright: "ringleft",
});
const REQUIREMENT_STAT = Object.freeze({ 1: 1, 2: 3, 3: 5, 4: 7, 5: 9, 7: 0, 8: 2, 9: 4, 10: 6, 11: 8 });
const APPROVED_BOOKS = Object.freeze({
  warrior: Object.freeze(["Fencing", "Slaying"]),
  wizard: Object.freeze(["FireBall", "GreatFireBall"]),
  taoist: Object.freeze(["Healing", "SpiritSword", "SoulFireBall"]),
});
const AMULET_ITEM_INDEX = 712;
const DEFAULT_RESTORATIVE_REUSE_DELAY_MS = 6_000;

/**
 * Prepare only the loadout choices that can be justified by the authoritative
 * snapshot. Unknown requirements or incomparable worn items are left alone.
 */
export async function prepareLoadout(client) {
  const actor = player(client?.snapshot);
  if (!actor) throw new Error("Cannot prepare loadout without the authoritative player entity");
  const className = normalized(actor.class);
  if (!CLASS_MASK[className]) return { equipped: [], learned: [], consumed: [] };

  const result = { equipped: [], learned: [], consumed: [] };
  await equipProvableUpgrades(client, actor, className, result.equipped);
  await learnApprovedBooks(client, actor, className, result.learned);
  result.consumed.push(...await useSupplies(client));
  return result;
}

/** Use only already-held basic HP/MP supplies at explicit bounded thresholds. */
export async function useSupplies(client, options = {}) {
  const consumed = [];
  if (playerDead(client?.snapshot)) return consumed;
  await useRestorative(client, "hp", consumed, ratioThreshold(options.hpThreshold, 0.6), options);
  await useRestorative(client, "mp", consumed, ratioThreshold(options.mpThreshold, 0.3), options);
  return consumed;
}

/**
 * Start an emergency HP dose without waiting for its delayed inventory
 * acknowledgement. A retreat cannot spend several incoming attack cycles
 * blocked inside client.request(); later snapshots still provide the normal
 * authoritative proof and ordinary recovery uses the verified path above.
 */
export function startEmergencyHpRecovery(client, options = {}) {
  const snapshot = client?.snapshot;
  if (playerDead(snapshot)) return [];
  const current = Number(snapshot?.playerHp ?? 0);
  const maximum = Number(snapshot?.playerMaxHp ?? 0);
  if (!(maximum > 0) || current / maximum > ratioThreshold(options.hpThreshold, 0.85)) return [];
  const now = restorativeNow(options);
  if (restorativeUsePending(client, "hp", now, options)) return [];
  const item = restorativeItem(snapshot, "hp");
  if (!item) return [];
  const command = { type: "useItem", uniqueId: item.uniqueId, grid: gridFor(item) };
  markRestorativeUse(client, "hp", now);
  client.send(command);
  return [{ pool: "hp", name: item.name, uniqueId: item.uniqueId, command, pending: true }];
}

/**
 * Use an already-learned Taoist Healing spell as a defensive action while an
 * interaction such as carving prevents the normal combat action chooser from
 * running. Callers keep the phase and threshold explicit so this cannot emit
 * an offensive spell or replace ordinary combat targeting.
 */
export async function useClassRecovery(client, options = {}) {
  const snapshot = client?.snapshot;
  if (playerDead(snapshot)) return null;
  const actor = player(snapshot);
  if (normalized(actor?.class) !== "taoist") return null;
  if (healthRatio(snapshot) > ratioThreshold(options.hpThreshold, 0.6)) return null;
  const healing = usableSkill(snapshot, "Healing");
  if (!healing) return null;
  const command = magicCommand(actor, actor, healing.spell);
  client.send(command);
  return { kind: "magic", spell: healing.spell, targetId: actor.objectId, command };
}

/**
 * Choose how close navigation should bring the player before combat. This is
 * only a tactical distance hint: combatAction still rechecks the same exact
 * learned-skill, cooldown, and MP state before sending a spell.
 */
export function combatApproachRange(client, _target) {
  const snapshot = client?.snapshot;
  const actor = player(snapshot);
  const className = normalized(actor?.class);
  if (className === "wizard") return preferredWizardSkill(snapshot) ? 6 : 1;
  if (className === "taoist") {
    return affordableSkill(snapshot, "SoulFireBall") && heldAmuletQuantity(snapshot) >= 1 ? 6 : 1;
  }
  return 1;
}

/** Perform one conservative combat action from authoritative state. */
export async function combatAction(client, target) {
  const snapshot = client?.snapshot;
  const actor = player(snapshot);
  if (!actor) throw new Error("Cannot fight without the authoritative player entity");
  if (!target || target.dead || !Number.isInteger(Number(target.objectId))) throw new Error("Combat target must be a live authoritative entity");
  const actionBlockMask = selfActionBlockMask(client);
  if (actionBlockMask) {
    return { kind: "wait", targetId: Number(target.objectId), delayMs: 650, actionBlockMask };
  }

  const className = normalized(actor.class);
  if (className === "taoist" && healthRatio(snapshot) <= 0.6) {
    const healing = usableSkill(snapshot, "Healing");
    if (healing) {
      const command = magicCommand(actor, actor, healing.spell);
      client.send(command);
      return { kind: "magic", spell: healing.spell, targetId: actor.objectId, command };
    }
  }

  if (className === "wizard" && tileDistance(actor, target) <= 9) {
    const preferred = preferredWizardSkill(snapshot);
    if (preferred && Number(preferred.cooldownRemainingTicks ?? 0) > 0) {
      return {
        kind: "wait", spell: preferred.spell,
        targetId: Number(target.objectId), delayMs: 650,
      };
    }
    if (preferred) {
      const command = magicCommand(actor, target, preferred.spell);
      client.send(command);
      return { kind: "magic", spell: preferred.spell, targetId: target.objectId, command };
    }
  }

  if (className === "taoist" && tileDistance(actor, target) <= 9 &&
      affordableSkill(snapshot, "SoulFireBall") && heldAmuletQuantity(snapshot) >= 1) {
    if (equippedAmuletQuantity(snapshot) < 1) await equipHeldAmulet(client);
    const liveSnapshot = client.snapshot;
    const liveActor = player(liveSnapshot);
    const liveTarget = (liveSnapshot?.entities ?? []).find(entity =>
      Number(entity?.objectId) === Number(target.objectId));
    if (!liveActor || !liveTarget || liveTarget.dead === true || Number(liveTarget.hp) <= 0) {
      return { kind: "wait", spell: "SoulFireBall", targetId: Number(target.objectId), delayMs: 1 };
    }
    const soulFireBall = equippedAmuletQuantity(liveSnapshot) >= 1
      ? affordableSkill(liveSnapshot, "SoulFireBall")
      : null;
    if (soulFireBall && Number(soulFireBall.cooldownRemainingTicks ?? 0) > 0) {
      return {
        kind: "wait", spell: soulFireBall.spell,
        targetId: Number(target.objectId), delayMs: 650,
      };
    }
    if (soulFireBall) {
      const command = magicCommand(liveActor, liveTarget, soulFireBall.spell);
      client.send(command);
      return { kind: "magic", spell: soulFireBall.spell, targetId: liveTarget.objectId, command };
    }
  }

  return meleeCombatAction(client, target);
}

/** Perform one authoritative basic attack without spending class ammunition. */
export async function meleeCombatAction(client, target) {
  const actor = player(client?.snapshot);
  if (!actor) throw new Error("Cannot fight without the authoritative player entity");
  if (!target || target.dead || !Number.isInteger(Number(target.objectId))) {
    throw new Error("Combat target must be a live authoritative entity");
  }
  const actionBlockMask = selfActionBlockMask(client);
  if (actionBlockMask) {
    return { kind: "wait", targetId: Number(target.objectId), delayMs: 650, actionBlockMask };
  }
  const command = { type: "attack", objectId: Number(target.objectId) };
  client.send(command);
  return { kind: "attack", targetId: Number(target.objectId), command };
}

async function equipProvableUpgrades(client, actor, className, equipped) {
  const attempted = new Set();
  while (true) {
    const snapshot = client.snapshot;
    let candidate = null;
    let destination = null;
    for (const item of snapshot.inventoryItems ?? []) {
      const slot = normalized(item.equipSlot);
      if (!slot || !validUniqueId(item.uniqueId) || attempted.has(item.uniqueId) || SLOT_INDEX[slot] === undefined) continue;
      if (!itemEligible(item, actor, snapshot, className)) continue;
      const target = eligibleEquipDestination(item, snapshot, className, slot);
      if (!target) continue;
      candidate = item;
      destination = target;
      break;
    }
    if (!candidate) return;
    attempted.add(candidate.uniqueId);
    const ack = await client.request({
      type: "equipItem", grid: "inventory", uniqueId: candidate.uniqueId, to: SLOT_INDEX[destination],
    }, "EquipItem");
    if (ack?.payload?.success !== true) throw new Error(`Equip rejected for ${candidate.name}`);
    await client.wait(
      () => (client.snapshot?.equipmentItems ?? []).some(item => Number(item.uniqueId) === Number(candidate.uniqueId)),
      `equipment ${candidate.name}`,
    );
    equipped.push(candidate.name);
  }
}

function eligibleEquipDestination(candidate, snapshot, className, preferredSlot) {
  const occupied = new Map((snapshot.equipmentItems ?? []).map(item => [
    normalized(item.slot ?? item.equipSlot), item,
  ]));
  const alternatives = [preferredSlot];
  const opposite = OPPOSITE_PAIRED_SLOT[preferredSlot];
  if (opposite) alternatives.push(opposite);

  // Paired jewellery metadata names one canonical side. Fill either empty hand
  // before considering a replacement, so an upgrade does not displace a
  // useful bracelet or ring while its symmetric slot is vacant.
  const empty = alternatives.find(slot => !occupied.has(slot));
  if (empty) return empty;
  return alternatives.find(slot => provablyBetter(candidate, occupied.get(slot), className, snapshot, slot)) ?? null;
}

async function learnApprovedBooks(client, actor, className, learned) {
  for (const spell of APPROVED_BOOKS[className] ?? []) {
    if (hasSkill(client.snapshot, spell)) continue;
    const book = (client.snapshot.inventoryItems ?? []).find(item =>
      normalized(item.name) === normalized(spell)
        && validUniqueId(item.uniqueId)
        && Number(field(template(item), "itemType", "item_type")) === 20
        && itemEligible(item, actor, client.snapshot, className));
    if (!book) continue;
    const ack = await client.request({ type: "useItem", uniqueId: book.uniqueId, grid: gridFor(book) }, "UseItem");
    if (ack?.payload?.success !== true) throw new Error(`Skill book rejected for ${spell}`);
    await client.wait(() => hasSkill(client.snapshot, spell), `learned skill ${spell}`);
    learned.push(spell);
  }
}

async function useRestorative(client, pool, consumed, threshold, options) {
  const snapshot = client.snapshot;
  const current = Number(snapshot?.[`player${pool === "hp" ? "Hp" : "Mp"}`]);
  const maximum = Number(snapshot?.[`playerMax${pool === "hp" ? "Hp" : "Mp"}`]);
  if (!(threshold > 0) || !(maximum > 0) || current / maximum > threshold) return;
  const now = restorativeNow(options);
  if (restorativeUsePending(client, pool, now, options)) return;
  const item = restorativeItem(snapshot, pool);
  if (!item) return;
  const beforeQuantity = Number(item.quantity ?? 1);
  markRestorativeUse(client, pool, now);
  const ack = await client.request({ type: "useItem", uniqueId: item.uniqueId, grid: gridFor(item) }, "UseItem");
  if (ack?.payload?.success !== true) {
    clearRestorativeUse(client, pool, now);
    // A failed UseItem can race the delayed death packet or an inventory
    // refresh. It is a recoverable combat observation, not proof that the
    // whole journey is invalid. Refresh once so the caller can either observe
    // death and revive or retry from fresh authoritative item state later.
    const afterSequence = Number(client.sequence ?? 0);
    if (typeof client.record === "function") {
      client.record("diagnostic", {
        type: "restorativeRejected",
        pool,
        uniqueId: Number(item.uniqueId),
        name: String(item.name),
      });
    }
    client.send({ type: "clientVersion" });
    try {
      await client.wait(
        () => playerDead(client.snapshot) || (client.events ?? []).some(event =>
          Number(event?.sequence) > afterSequence && event?.direction === "received" &&
          event?.type === "worldSnapshot"),
        `${pool.toUpperCase()} restorative rejection refresh`,
        3_000,
      );
    } catch (error) {
      if (client.failure || client.closed) throw error;
    }
    return;
  }
  await client.wait(() => {
    const next = client.snapshot;
    const nextValue = Number(next?.[`player${pool === "hp" ? "Hp" : "Mp"}`]);
    const remaining = [...(next?.beltItems ?? []), ...(next?.inventoryItems ?? [])].find(candidate => Number(candidate.uniqueId) === Number(item.uniqueId));
    return nextValue > current || !remaining || Number(remaining.quantity ?? 0) < beforeQuantity;
  }, `${pool.toUpperCase()} restorative ${item.name}`);
  consumed.push(item.name);
}

function restorativeUsePending(client, pool, now, options) {
  const lastUseAt = Number(client?.journeyRestorativeUseAt?.[pool]);
  if (!Number.isFinite(lastUseAt)) return false;
  const requestedDelay = Number(options?.restorativeReuseDelayMs);
  const delayMs = Number.isFinite(requestedDelay)
    ? Math.max(0, requestedDelay)
    : DEFAULT_RESTORATIVE_REUSE_DELAY_MS;
  return now - lastUseAt < delayMs;
}

function markRestorativeUse(client, pool, now) {
  if (!client.journeyRestorativeUseAt) client.journeyRestorativeUseAt = Object.create(null);
  client.journeyRestorativeUseAt[pool] = now;
}

function clearRestorativeUse(client, pool, usedAt) {
  if (Number(client?.journeyRestorativeUseAt?.[pool]) === usedAt) {
    delete client.journeyRestorativeUseAt[pool];
  }
}

function restorativeNow(options) {
  const value = typeof options?.now === "function" ? options.now() : options?.now;
  return Number.isFinite(Number(value)) ? Number(value) : Date.now();
}

function restorativeItem(snapshot, pool) {
  const pattern = pool === "hp" ? /^\(hp\)drug/i : /^\(mp\)drug/i;
  return [...(snapshot?.beltItems ?? []), ...(snapshot?.inventoryItems ?? [])]
    .find(candidate => pattern.test(String(candidate.name)) && validUniqueId(candidate.uniqueId) && Number(candidate.quantity ?? 1) > 0);
}

function ratioThreshold(value, fallback) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.max(0, Math.min(1, parsed)) : fallback;
}

function playerDead(snapshot) {
  const actor = player(snapshot);
  return !actor || actor.dead === true || Number(actor.hp) <= 0 ||
    Number(snapshot?.playerHp) <= 0;
}

function itemEligible(item, actor, snapshot, className) {
  const info = template(item);
  if (!info) return false;
  const requiredClass = Number(field(info, "requiredClass", "required_class") ?? 0);
  if (requiredClass !== 0 && (requiredClass & CLASS_MASK[className]) === 0) return false;
  const requiredGender = Number(field(info, "requiredGender", "required_gender") ?? 0);
  const genderMask = normalized(actor.gender) === "female" ? 2 : 1;
  if (requiredGender !== 0 && (requiredGender & genderMask) === 0) return false;
  const type = Number(field(info, "requiredType", "required_type") ?? 0);
  const amount = Number(field(info, "requiredAmount", "required_amount") ?? 0);
  const level = Number(actor.level ?? 0);
  if (type === 0) return level >= amount;
  if (type === 6) return level <= amount;
  const stat = REQUIREMENT_STAT[type];
  if (stat === undefined) return false;
  return statValue(snapshot.playerCrystalStats, stat) >= amount;
}

function provablyBetter(candidate, worn, className, snapshot, slot) {
  const next = itemStats(candidate);
  const current = itemStats(worn);
  if (!next || !current) return false;
  const relevant = new Set([0, 1, 2, 3, 10, 11, 12, 13, 14, 15, 19, 20, 21, 22, 23, 30, 31, 32, 33, 34, 35, 36, ...(CLASS_ATTACK_STATS[className] ?? [])]);
  let better = false;
  let regressed = false;
  for (const stat of relevant) {
    const delta = (next.get(stat) ?? 0) - (current.get(stat) ?? 0);
    if (delta < 0) regressed = true;
    if (delta > 0) better = true;
  }
  if (!regressed && better) return true;
  return slot === "weapon"
    && usesProvenMeleeWeaponUpgrade(className, snapshot)
    && meleeWeaponExpectedDamageDominates(next, current, snapshot, relevant);
}

function usesProvenMeleeWeaponUpgrade(className, snapshot) {
  if (className === "warrior" || className === "wizard") return true;
  return className === "taoist" && !(snapshot?.knownSkills ?? []).some(skill => skill?.offensive === true);
}

function meleeWeaponExpectedDamageDominates(next, current, snapshot, relevant) {
  // A lower DC bound can still be a real upgrade when the wider range produces
  // at least as much expected damage after every possible nonnegative armour
  // value. Keep every other combat stat nondecreasing, and only use the uniform
  // Crystal roll when the authoritative current and replacement Luck are zero.
  for (const stat of relevant) {
    if (stat === 4 || stat === 5) continue;
    if ((next.get(stat) ?? 0) < (current.get(stat) ?? 0)) return false;
  }

  const effectiveMin = authoritativeStatValue(snapshot?.playerCrystalStats, 4);
  const effectiveMax = authoritativeStatValue(snapshot?.playerCrystalStats, 5);
  const effectiveLuck = authoritativeSparseStatValue(snapshot?.playerCrystalStats, 15);
  if (effectiveMin === null || effectiveMax === null || effectiveLuck !== 0) return false;

  const nextLuck = effectiveLuck + (next.get(15) ?? 0) - (current.get(15) ?? 0);
  if (nextLuck !== 0) return false;
  const replacementMin = effectiveMin + (next.get(4) ?? 0) - (current.get(4) ?? 0);
  const replacementMax = effectiveMax + (next.get(5) ?? 0) - (current.get(5) ?? 0);
  if (![effectiveMin, effectiveMax, replacementMin, replacementMax].every(Number.isSafeInteger)) return false;

  const currentMax = Math.max(0, effectiveMax);
  const currentMin = Math.min(Math.max(0, effectiveMin), currentMax);
  const candidateMax = Math.max(0, replacementMax);
  const candidateMin = Math.min(Math.max(0, replacementMin), candidateMax);
  return uniformDamageDominatesAtEveryArmour(candidateMin, candidateMax, currentMin, currentMax);
}

function uniformDamageDominatesAtEveryArmour(nextMin, nextMax, currentMin, currentMax) {
  const highestRelevantArmour = Math.max(nextMax, currentMax);
  // Real Crystal combat bounds are small. Refuse pathological snapshots rather
  // than spending unbounded work on data that cannot justify an automatic swap.
  if (highestRelevantArmour > 10_000) return false;
  let strictlyBetter = false;
  for (let armour = 0; armour <= highestRelevantArmour; armour += 1) {
    const next = uniformNetDamage(nextMin, nextMax, armour);
    const current = uniformNetDamage(currentMin, currentMax, armour);
    const nextScaled = next.sum * current.count;
    const currentScaled = current.sum * next.count;
    if (nextScaled < currentScaled) return false;
    if (nextScaled > currentScaled) strictlyBetter = true;
  }
  return strictlyBetter;
}

function uniformNetDamage(min, max, armour) {
  const count = BigInt(max - min + 1);
  const firstDamage = Math.max(min, armour + 1);
  if (firstDamage > max) return { sum: 0n, count };
  const positiveCount = BigInt(max - firstDamage + 1);
  const firstNet = BigInt(firstDamage - armour);
  const lastNet = BigInt(max - armour);
  return { sum: positiveCount * (firstNet + lastNet) / 2n, count };
}

function authoritativeStatValue(stats, wanted) {
  const entry = (stats ?? []).find(candidate => Number(candidate.stat) === wanted);
  if (!entry) return null;
  const value = Number(entry.value);
  return Number.isSafeInteger(value) ? value : null;
}

function authoritativeSparseStatValue(stats, wanted) {
  if (!Array.isArray(stats)) return null;
  const entry = stats.find(candidate => Number(candidate.stat) === wanted);
  if (!entry) return 0;
  return typeof entry.value === "number" && Number.isSafeInteger(entry.value) ? entry.value : null;
}

function itemStats(item) {
  const source = item?.tooltipSource;
  const info = source?.realInfo ?? source?.real_info ?? source?.info;
  if (!info || !Array.isArray(info.stats)) return null;
  const totals = new Map();
  addStats(totals, info.stats);
  const userItem = source?.userItem ?? source?.user_item;
  addStats(totals, userItem?.addedStats ?? userItem?.added_stats);
  const sockets = (source?.realSocketInfos?.length ? source.realSocketInfos : null)
    ?? (source?.real_socket_infos?.length ? source.real_socket_infos : null)
    ?? source?.socketInfos
    ?? source?.socket_infos
    ?? [];
  for (const socket of sockets) addStats(totals, socket?.stats);
  return totals;
}

function addStats(target, stats) {
  for (const entry of stats ?? []) {
    const stat = Number(entry.stat);
    const value = Number(entry.value);
    if (Number.isInteger(stat) && Number.isFinite(value)) target.set(stat, (target.get(stat) ?? 0) + value);
  }
}

function affordableSkill(snapshot, exactSpell) {
  const skill = (snapshot?.knownSkills ?? []).find(candidate => candidate.spell === exactSpell);
  if (!skill) return null;
  if (Number(snapshot.playerMp ?? 0) < Number(skill.mpCost ?? 0)) return null;
  return skill;
}

function preferredWizardSkill(snapshot) {
  const greatFireBall = affordableSkill(snapshot, "GreatFireBall");
  const fireBall = affordableSkill(snapshot, "FireBall");
  return [greatFireBall, fireBall].find(skill => Number(skill?.cooldownRemainingTicks ?? 0) <= 0)
    ?? greatFireBall
    ?? fireBall;
}

function equippedAmuletQuantity(snapshot) {
  return (snapshot?.equipmentItems ?? []).reduce((total, item) => {
    if (normalized(item?.slot ?? item?.equipSlot) !== "amulet") return total;
    const exactAmulet = templateIndex(item) === AMULET_ITEM_INDEX || normalized(item?.name) === "amulet";
    return exactAmulet ? total + Math.max(0, Number(item?.quantity ?? 1)) : total;
  }, 0);
}

function heldAmuletQuantity(snapshot) {
  return ['equipmentItems', 'inventoryItems', 'beltItems'].reduce((total, key) =>
    total + (snapshot?.[key] ?? []).reduce((subtotal, item) => {
      const exactAmulet = templateIndex(item) === AMULET_ITEM_INDEX || normalized(item?.name) === "amulet";
      return exactAmulet ? subtotal + Math.max(0, Number(item?.quantity ?? 1)) : subtotal;
    }, 0), 0);
}

function usableSkill(snapshot, exactSpell) {
  const skill = affordableSkill(snapshot, exactSpell);
  if (!skill || Number(skill.cooldownRemainingTicks ?? 0) > 0) return null;
  return skill;
}

function magicCommand(actor, target, spell) {
  return {
    type: "magic", objectId: Number(actor.objectId), spell,
    direction: directionToward(actor, target), targetId: Number(target.objectId),
    x: Number(target.x), y: Number(target.y), spellTargetLock: true,
  };
}

function directionToward(from, to) {
  const dx = Math.sign(Number(to.x) - Number(from.x));
  const dy = Math.sign(Number(to.y) - Number(from.y));
  if (dx === 0 && dy === 0) return validDirection(from.direction) ? from.direction : "Down";
  return new Map([
    ["0,-1", "Up"], ["1,-1", "UpRight"], ["1,0", "Right"], ["1,1", "DownRight"],
    ["0,1", "Down"], ["-1,1", "DownLeft"], ["-1,0", "Left"], ["-1,-1", "UpLeft"],
  ]).get(`${dx},${dy}`);
}

function validDirection(value) {
  return ["Up", "UpRight", "Right", "DownRight", "Down", "DownLeft", "Left", "UpLeft"].includes(value);
}

function healthRatio(snapshot) {
  const maximum = Number(snapshot?.playerMaxHp ?? 0);
  return maximum > 0 ? Number(snapshot?.playerHp ?? 0) / maximum : 1;
}

function tileDistance(a, b) { return Math.max(Math.abs(Number(a.x) - Number(b.x)), Math.abs(Number(a.y) - Number(b.y))); }
function hasSkill(snapshot, spell) { return (snapshot?.knownSkills ?? []).some(skill => skill.spell === spell); }
function player(snapshot) { return snapshot?.entities?.find(entity => Number(entity.objectId) === Number(snapshot.playerObjectId)); }
function template(item) { return item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info ?? null; }
function templateIndex(item) {
  const info = template(item);
  const value = item?.itemIndex ?? item?.item_index ?? info?.itemIndex ?? info?.item_index;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}
function statValue(stats, wanted) { return Number((stats ?? []).find(entry => Number(entry.stat) === wanted)?.value ?? 0); }
function gridFor(item) { return normalized(item.container) === "belt" ? "belt" : "inventory"; }
function normalized(value) { return String(value ?? "").replace(/[^a-z0-9]/gi, "").toLowerCase(); }
function field(value, camel, snake) { return value?.[camel] ?? value?.[snake]; }
function validUniqueId(value) { const id = Number(value); return Number.isSafeInteger(id) && id >= 0; }
