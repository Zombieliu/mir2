import fs from 'node:fs';

export const V2_SUPPLY_CONFIG = JSON.parse(fs.readFileSync(
  new URL('../../../../config/quest-guidance/newcomer-supplies.json', import.meta.url), 'utf8'));

const CLASS_MASK = { warrior: 1, wizard: 2, taoist: 4 };
const normal = value => String(value ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase();
export const V2_SUPPLY_ITEMS = Object.freeze([
  ...[['Small', 658, 40, 1], ['Medium', 660, 110, 2], ['Large', 662, 225, 3]]
    .flatMap(([size, index, price, weight]) => [
      { itemIndex: index, name: `(HP)Drug${size}`, kind: 'hp', price, weight, stackSize: 20, minimumLevel: 0 },
      { itemIndex: index + 1, name: `(MP)Drug${size}`, kind: 'mp', price, weight, stackSize: 20, minimumLevel: 0 },
    ]),
  { itemIndex: 712, name: 'Amulet', kind: 'amulet', price: 25, weight: 1, stackSize: 500, minimumLevel: 18 },
  { itemIndex: 710, name: 'GreenPoison', kind: 'poison', price: 40, weight: 1, stackSize: 500, minimumLevel: 14 },
  { itemIndex: 711, name: 'RedPoison', kind: 'poison', price: 40, weight: 1, stackSize: 500, minimumLevel: 14 },
  { itemIndex: 719, name: 'TownTeleport', kind: 'townTeleport', price: 1000, weight: 1, stackSize: 20, minimumLevel: 0 },
  { itemIndex: 717, name: 'RandomTeleport', kind: 'randomTeleport', price: 100, weight: 1, stackSize: 20, minimumLevel: 0 },
].map(Object.freeze));

function actor(snapshot) {
  return (snapshot?.entities ?? []).find(value => String(value?.objectId) === String(snapshot?.playerObjectId));
}

export function v2SupplyItem(item) {
  const source = item?.tooltipSource ?? item?.tooltip_source;
  const info = source?.realInfo ?? source?.real_info ?? source?.info;
  const index = Number(item?.itemIndex ?? item?.item_index ?? info?.item_index ?? info?.itemIndex ??
    String(item?.key ?? '').match(/^crystal-item-(\d+)$/)?.[1]);
  if (Number.isSafeInteger(index)) return V2_SUPPLY_ITEMS.find(value => value.itemIndex === index) ?? null;
  return V2_SUPPLY_ITEMS.find(value => value.name === item?.name) ?? null;
}

function binaryDateMilliseconds(value) {
  try {
    let ticks = BigInt(value) & ((1n << 62n) - 1n);
    if (ticks > 3155378975999999999n) ticks -= 1n << 62n;
    return Number((ticks - 621355968000000000n) / 10000n);
  } catch { return null; }
}

/** Only carried, usable owner stock; no storage, mail, shop or auction balances. */
export function v2SupplyStock(snapshot, { now = Date.now() } = {}) {
  const stock = { hp: 0, mp: 0, amulet: 0, poison: 0, townTeleport: 0, randomTeleport: 0 };
  const owner = actor(snapshot);
  const level = Number(owner?.level ?? 0);
  const classMask = CLASS_MASK[normal(owner?.class)] ?? 0;
  const seen = new Set();
  for (const [items, equipped] of [[snapshot?.inventoryItems, false], [snapshot?.beltItems, false], [snapshot?.equipmentItems, true]]) {
    for (const item of items ?? []) {
      const supply = v2SupplyItem(item);
      if (!supply || (equipped && !['amulet', 'poison'].includes(supply.kind))) continue;
      // EquipmentItemSnapshot serializes EquipmentSlot as camelCase. Native
      // normalized_slot also accepts the explicit 6/9 wire index and these
      // legacy aliases; never use equipment array order as a slot fallback.
      if (equipped && !['amulet', 'braceletright', 'braceletr', '6', '9'].includes(normal(item.slot))) continue;
      if (item.container && !['bag1', 'bag2', 'inventory', 'belt', 'equipment'].includes(normal(item.container))) continue;
      const quantity = Number(item.quantity ?? item.count);
      if (!Number.isSafeInteger(quantity) || quantity <= 0) continue;
      const source = item.tooltipSource ?? item.tooltip_source;
      const info = source?.realInfo ?? source?.real_info ?? source?.info;
      const instance = source?.userItem ?? source?.user_item;
      const expiry = instance?.expireInfo ?? instance?.expire_info;
      const expiryMs = expiry ? binaryDateMilliseconds(expiry.expiryBinaryDatetime ?? expiry.expiry_binary_datetime) : null;
      const sealed = instance?.sealedInfo ?? instance?.sealed_info;
      const sealedValue = sealed?.expiryBinaryDatetime ?? sealed?.expiry_binary_datetime ?? item.sealedExpiryTimeBinaryDatetime;
      const sealedMs = sealedValue != null ? binaryDateMilliseconds(sealedValue) : null;
      if (item.canUse === false || item.usable === false || item.expired === true || item.sealed === true ||
          item.isShopItem === true || item.is_shop_item === true || instance?.isShopItem === true || instance?.is_shop_item === true ||
          instance?.expired === true || (expiry && (expiryMs == null || expiryMs <= now)) ||
          (sealed && sealedMs == null) || (sealedMs != null && sealedMs > now)) continue;
      if (Number(item.durabilityMax ?? 0) > 0 && Number(item.durabilityCurrent ?? 0) <= 0) continue;
      if (Number(info?.required_type ?? info?.requiredType ?? 0) !== 0 ||
          level < Math.max(supply.minimumLevel, Number(info?.required_amount ?? info?.requiredAmount ?? 0))) continue;
      const requiredClass = Number(info?.required_class ?? info?.requiredClass ?? 0);
      if (requiredClass && !(requiredClass & classMask)) continue;
      const requiredGender = Number(info?.required_gender ?? info?.requiredGender ?? 3);
      const gender = normal(owner?.gender) === 'male' ? 1 : normal(owner?.gender) === 'female' ? 2 : 0;
      if (requiredGender !== 3 && !(requiredGender & gender)) continue;
      const shape = item.shape ?? info?.shape;
      if (shape != null && ((supply.kind === 'amulet' && Number(shape) !== 0) ||
          (supply.kind === 'poison' && ![1, 2].includes(Number(shape))))) continue;
      const uid = item.uniqueId ?? item.unique_id;
      if (uid != null && seen.has(String(uid))) continue;
      if (uid != null) seen.add(String(uid));
      stock[supply.kind] += quantity;
    }
  }
  return stock;
}

/** Departure minima are QA guards, not a claim that a full chapter is survivable. */
export function v2SupplyPolicy(snapshot, quest, className) {
  const level = Number(actor(snapshot)?.level ?? 0);
  const caster = ['wizard', 'taoist'].includes(normal(className));
  const taoist = normal(className) === 'taoist';
  const known = new Set((snapshot?.knownSkills ?? []).map(skill => String(skill.spell)));
  const requirements = (quest?.objectives?.flag ?? []).flatMap(flag => flag?.requirements?.[normal(className)] ?? []).join(' ');
  const tier = V2_SUPPLY_CONFIG.potionTiers.findLast(value => level >= value.minLevel) ?? V2_SUPPLY_CONFIG.potionTiers[0];
  const usesMp = caster && (level >= V2_SUPPLY_CONFIG.casterMinimumLevel || known.size > 0);
  const usesAmulet = taoist && level >= V2_SUPPLY_CONFIG.amulet.minimumLevel &&
    (known.has('SoulFireBall') || known.has('SummonSkeleton') || /SoulFireBall|SummonSkeleton|Amulet/.test(requirements));
  const usesPoison = taoist && level >= V2_SUPPLY_CONFIG.poison.minimumLevel &&
    (known.has('Poisoning') || /Poisoning|poison/i.test(requirements));
  const id = Number(quest?.questId);
  const reserve = V2_SUPPLY_CONFIG.wizardTownReserve;
  const wizardTown = normal(className) === 'wizard' && id >= reserve.firstQuestId && id <= reserve.lastQuestId;
  return {
    minimumHpRatio: V2_SUPPLY_CONFIG.minimumHpRatio,
    potionTier: tier.name,
    hp: { ...V2_SUPPLY_CONFIG.hp, itemIndex: tier.hpItemIndex },
    mp: { minimum: usesMp ? tier.mpMinimum : 0, target: usesMp ? tier.mpTarget : 0, itemIndex: tier.mpItemIndex },
    amulet: { minimum: usesAmulet ? Number(V2_SUPPLY_CONFIG.amulet.questMinimum[id] ?? V2_SUPPLY_CONFIG.amulet.minimum) : 0,
      target: usesAmulet ? V2_SUPPLY_CONFIG.amulet.target : 0, itemIndex: 712 },
    poison: { minimum: usesPoison ? V2_SUPPLY_CONFIG.poison.minimum : 0,
      target: usesPoison ? V2_SUPPLY_CONFIG.poison.target : 0, itemIndex: 710 },
    townTeleport: { minimum: wizardTown ? reserve.minimum : 0, target: wizardTown ? reserve.target : 0, itemIndex: 719 },
    randomTeleport: { minimum: 0, target: 0, itemIndex: 717 },
  };
}

export function v2SupplyStatus(snapshot, policy) {
  const stock = v2SupplyStock(snapshot);
  const missing = Object.keys(stock).filter(kind => stock[kind] < policy[kind].minimum)
    .map(kind => ({ kind, have: stock[kind], minimum: policy[kind].minimum }));
  return { ready: missing.length === 0, stock, missing };
}

/** One bounded, ordinary resupply attempt followed by a complete fresh recheck. */
export function createV2SupplyReadiness({ className, restock, useTownTeleport, useSupplies, waitForHealth }) {
  return async (client, quest, navigate, { travel } = {}) => {
    const policy = v2SupplyPolicy(client.snapshot, quest, className);
    const status = () => v2SupplyStatus(client.snapshot, policy);
    const hpRatio = () => Number(client.snapshot?.playerHp ?? 0) / Math.max(1, Number(client.snapshot?.playerMaxHp ?? 1));
    const blocked = message => ({ status: 'blocked', message: `q${quest.questId} ${message}`, ...status() });
    if (status().ready && hpRatio() >= policy.minimumHpRatio) return { status: 'ready', ...status() };
    if (String(client.snapshot?.mapFileName ?? '') !== '0') {
      if (v2SupplyStock(client.snapshot).townTeleport > 0) {
        try { await useTownTeleport(client); }
        catch (error) { if (/deadline/i.test(error?.reason ?? '')) throw error; }
      }
      if (String(client.snapshot?.mapFileName ?? '') !== '0' && typeof travel === 'function') {
        try { await travel('0'); }
        catch (error) {
          if (/deadline/i.test(error?.reason ?? '')) throw error;
          return blocked(`legal return route is blocked: ${error.message}`);
        }
      }
      if (String(client.snapshot?.mapFileName ?? '') !== '0') return blocked('needs supplies and a legal return route to Bichon');
    }
    const result = await restock(client, navigate, { policy, travel });
    if (!['restocked', 'sufficient'].includes(result?.status) || !status().ready) {
      return blocked(`cannot depart: ${result?.reason ?? result?.status ?? 'unconfirmed resupply'}; missing ${status().missing.map(value => `${value.kind} ${value.have}/${value.minimum}`).join(', ')}`);
    }
    if (hpRatio() < policy.minimumHpRatio) {
      await useSupplies(client, { hpThreshold: 0.65, mpThreshold: 0.35 });
      if (hpRatio() < policy.minimumHpRatio) {
        try { await waitForHealth(client, { requiredRatio: policy.minimumHpRatio, timeoutMs: 30_000 }); }
        catch (error) {
          if (/deadline/i.test(error?.reason ?? '')) throw error;
          return blocked('remains below safe departure HP after ordinary recovery');
        }
      }
    }
    // Recovery and town navigation may consume the last MP/HP/material. An
    // earlier shop result must never make that stale stock count as ready.
    return hpRatio() >= policy.minimumHpRatio && status().ready
      ? { status: 'ready', ...status() }
      : blocked('fresh departure check still lacks health or usable supplies');
  };
}
