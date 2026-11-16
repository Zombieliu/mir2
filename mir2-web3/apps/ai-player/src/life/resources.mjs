const HP_POTIONS = new Set(["(HP)DrugSmall", "(HP)DrugMedium", "(HP)DrugLarge", "(HP)DrugXL"]);
const uint = (value, max = Number.MAX_SAFE_INTEGER) => Number.isSafeInteger(value) && value >= 0 && value <= max;
const uid = value => typeof value === "bigint" ? value > 0n && value <= 0xffff_ffff_ffff_ffffn : uint(value) && value > 0;

function potionCount(items) {
  if (!Array.isArray(items)) return null;
  const seen = new Set(); let total = 0;
  for (const item of items) {
    if (!HP_POTIONS.has(item?.name)) continue;
    if (!uid(item.uniqueId) || !["bag1", "bag2", "belt"].includes(item.container) || !uint(item.quantity, 0xffff_ffff) || seen.has(String(item.uniqueId))) return null;
    seen.add(String(item.uniqueId)); total += item.quantity;
    if (!uint(total)) return null;
  }
  return total;
}

/** A current ordinary observation, never an estimate of missing resources.
 * usableItems can be the controller's quarantine-filtered owned items. */
export function projectLifeResources(state = {}, { observedAtMs = Date.now(), medicineUnresolved = false, usableItems } = {}) {
  const ready = state.ready === true && uint(observedAtMs);
  const inventoryCurrent = ready && state.inventoryStale === false && Array.isArray(state.inventoryItems) && Array.isArray(state.beltItems);
  const hp = ready && uint(state.playerHp, 0x7fff_ffff) && uint(state.playerMaxHp, 0x7fff_ffff) && state.playerMaxHp > 0 && state.playerHp <= state.playerMaxHp ? state.playerHp : null;
  const maxHp = ready && uint(state.playerMaxHp, 0x7fff_ffff) && state.playerMaxHp > 0 ? state.playerMaxHp : null;
  const gold = ready && uint(state.gold, 0xffff_ffff) ? state.gold : null;
  const hpPotionCount = inventoryCurrent ? potionCount([...state.inventoryItems, ...state.beltItems]) : null;
  const unresolved = medicineUnresolved === true || state.medicineUnresolved === true;
  const usableHpPotionCount = inventoryCurrent && !unresolved ? potionCount(usableItems ?? [...state.inventoryItems, ...state.beltItems]) : null;
  const freeBagSlots = inventoryCurrent && uint(state.freeBagSlots, 65535) ? state.freeBagSlots : null;
  const currentWeight = inventoryCurrent && uint(state.currentWeight, 0xffff_ffff) ? state.currentWeight : null;
  const maxWeight = inventoryCurrent && uint(state.maxWeight, 0xffff_ffff) && state.maxWeight > 0 ? state.maxWeight : null;
  return { observedAtMs: ready ? observedAtMs : null,
    known: !unresolved && [hp, maxHp, gold, hpPotionCount, usableHpPotionCount, freeBagSlots, currentWeight, maxWeight].every(value => value !== null),
    hp, maxHp, gold, hpPotionCount, usableHpPotionCount, freeBagSlots, currentWeight, maxWeight, inventoryCurrent, medicineUnresolved: unresolved };
}
