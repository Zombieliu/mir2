import { observedPlayerHp } from './protocol-observation.mjs';
import { supersededProgressionGearForSale } from './policy.mjs';

const RUBEN = Object.freeze({ name: 'Merchant_Ruben', x: 288, y: 608 });
const SCOTT = Object.freeze({ name: 'Merchant_Scott', x: 291, y: 610 });
// Crystal NPC-info: BichonProvince/BichonWall/Potion1, loaded object id 20.
// Resolve its public object id from the live AOI, rather than treating the
// source NPC index (28) as an interaction id.
const BICHON_WALL_SAMUEL = Object.freeze({ name: 'Alchemist_Samuel', x: 324, y: 291 });
const BLACKSMITH = Object.freeze({ name: 'Blacksmith_Smith', x: 296, y: 613 });
const BUTCHER = Object.freeze({ name: 'Merchant_John', x: 292, y: 603 });
const MATERIAL_DEALER = Object.freeze({ name: 'MaterialDealer_Reece', x: 295, y: 605 });
const SUPPLIES = Object.freeze({
  hp: Object.freeze({ itemIndex: 658, name: '(HP)DrugSmall', target: 6, catalogPrice: 40, stackSize: 20 }),
  hpMedium: Object.freeze({ itemIndex: 660, name: '(HP)DrugMedium', target: 6, catalogPrice: 110, stackSize: 20 }),
  mp: Object.freeze({ itemIndex: 659, name: '(MP)DrugSmall', target: 6, catalogPrice: 40, stackSize: 20 }),
  amulet: Object.freeze({ itemIndex: 712, name: 'Amulet', target: 6, catalogPrice: 25, stackSize: 100 }),
});
const RANDOM_TELEPORT = Object.freeze({
  itemIndex: 717,
  name: 'RandomTeleport',
  catalogPrice: 100,
  stackSize: 20,
});
// crystal_item_manifest.json: item_index 719, price 1000, stack_size 20.
// Merchant Ruben's original Grocery trade list does not include this drop-only
// scroll, so restocking it is deliberately opt-in and still requires a live
// NPC goods row before sending BuyItem.
const TOWN_TELEPORT = Object.freeze({
  itemIndex: 719,
  name: 'TownTeleport',
  catalogPrice: 1000,
  stackSize: 20,
});
const LOW_STOCK = 3;
const GOLD_RESERVE = 100;
const WAIT_MS = 12_000;
const BELT_SLOT_COUNT = 6;
const FIRST_BAG_PAGE_SLOTS = 40;
const SOUL_FIRE_BALL_REQUIRED_LEVEL = 18;
const WARRIOR_WEAPONS = Object.freeze([
  Object.freeze({ itemIndex: 227, name: 'BronzeAxe', catalogPrice: 1500, requiredLevel: 13 }),
  Object.freeze({ itemIndex: 226, name: 'IronSword', catalogPrice: 1100, requiredLevel: 10 }),
  Object.freeze({ itemIndex: 225, name: 'ShortSword', catalogPrice: 1000, requiredLevel: 7 }),
  Object.freeze({ itemIndex: 224, name: 'BronzeSword', catalogPrice: 900, requiredLevel: 5 }),
  Object.freeze({ itemIndex: 222, name: 'Dagger', catalogPrice: 500, requiredLevel: 2 }),
  Object.freeze({ itemIndex: 221, name: 'WoodenSword', catalogPrice: 50, requiredLevel: 1 }),
]);
const CLASS_WEAPONS = Object.freeze({
  warrior: WARRIOR_WEAPONS,
  wizard: Object.freeze([
    Object.freeze({ itemIndex: 254, name: 'Trident', catalogPrice: 4000, requiredLevel: 15 }),
    Object.freeze({ itemIndex: 223, name: 'EbonySword', catalogPrice: 4000, requiredLevel: 4 }),
    Object.freeze({ itemIndex: 226, name: 'IronSword', catalogPrice: 1100, requiredLevel: 10 }),
    Object.freeze({ itemIndex: 221, name: 'WoodenSword', catalogPrice: 50, requiredLevel: 1 }),
  ]),
  taoist: Object.freeze([
    Object.freeze({ itemIndex: 268, name: 'Scimitar', catalogPrice: 4000, requiredLevel: 15 }),
    Object.freeze({ itemIndex: 226, name: 'IronSword', catalogPrice: 1100, requiredLevel: 10 }),
    Object.freeze({ itemIndex: 221, name: 'WoodenSword', catalogPrice: 50, requiredLevel: 1 }),
  ]),
});
const TAOIST_FOREST_WEAPONS = Object.freeze([
  Object.freeze({ itemIndex: 226, name: 'IronSword', catalogPrice: 1100, requiredLevel: 10 }),
  Object.freeze({ itemIndex: 224, name: 'BronzeSword', catalogPrice: 900, requiredLevel: 5 }),
]);
const TAOIST_FOREST_MINIMUM_STOCK = Object.freeze({ hp: 6, mp: 6, randomTeleport: 1, townTeleport: 1 });

/** Gold needed for the preferred ordinary replacement when a Warrior is unarmed or under-geared. */
export function warriorWeaponFundingGold(snapshot) {
  return warriorWeaponRequirement(snapshot)?.preferred.catalogPrice ?? 0;
}

/** Gold needed to replace an unarmed journey character through the live weapon shop. */
export function journeyWeaponFundingGold(snapshot) {
  return classWeaponRequirement(snapshot)?.preferred.catalogPrice ?? 0;
}

/**
 * Restock ordinary beginner supplies through Merchant Ruben's live shop.
 *
 * This routine is deliberately narrow: it works only on map 0, buys small HP
 * potions for every supported class and small MP potions for Wizard/Taoist,
 * and stocks Amulets only after a Taoist can use learned SoulFireBall,
 * and accepts success only after a fresh authoritative snapshot proves both
 * the exact gold debit and the exact item-quantity increase.
 */
export async function restockInVillage(client, navigateNear, options = {}) {
  const initial = client?.snapshot;
  if (!initial || typeof initial !== 'object') throw new Error('Cannot restock without an authoritative snapshot');
  if (String(initial.mapFileName ?? '') !== '0') {
    return { status: 'notInVillage', mapFileName: initial.mapFileName ?? null };
  }

  const className = playerClass(initial);
  const targets = {
    hp: positiveOption(options.targetHp, SUPPLIES.hp.target, 'targetHp'),
    hpMedium: positiveOrZeroOption(options.targetHpMedium, 0, 'targetHpMedium'),
    mp: positiveOption(options.targetMp, SUPPLIES.mp.target, 'targetMp'),
    amulet: positiveOption(options.targetAmulet, SUPPLIES.amulet.target, 'targetAmulet'),
  };
  const lowStock = positiveOption(options.lowStock, LOW_STOCK, 'lowStock');
  const lowStockByKind = {
    hp: positiveOption(options.lowStockHp, lowStock, 'lowStockHp'),
    hpMedium: positiveOrZeroOption(options.lowStockHpMedium, 0, 'lowStockHpMedium'),
    mp: positiveOption(options.lowStockMp, lowStock, 'lowStockMp'),
    amulet: positiveOption(options.lowStockAmulet, lowStock, 'lowStockAmulet'),
  };
  const reserveGold = nonnegativeOption(options.reserveGold, GOLD_RESERVE, 'reserveGold');
  const essentialTaoistForestWeapon = options.ensureTaoistForestWeapon === true;
  const weaponRequirement = essentialTaoistForestWeapon
    ? taoistForestWeaponRequirement(initial)
    : options.ensureClassWeapon === true
      ? classWeaponRequirement(initial)
    : options.ensureWarriorWeapon === true
      ? warriorWeaponRequirement(initial)
      : null;
  const weaponNeeded = weaponRequirement != null;
  const preferredWeaponCost = weaponRequirement?.preferred.catalogPrice ?? 0;
  const relevant = className === 'taoist' ? ['hp', 'mp', ...(canUseSoulFireBall(initial) ? ['amulet'] : [])]
    : className === 'wizard' ? ['hp', 'mp'] : ['hp'];
  const beforeStock = stock(initial);
  const needed = relevant.filter(kind => beforeStock[kind] < lowStockByKind[kind]);
  const mediumOptIn = className === 'wizard' && targets.hpMedium > 0;
  const hpMediumBefore = hpMediumDrugCount(initial);
  const needHpMedium = mediumOptIn && hpMediumBefore < lowStockByKind.hpMedium;
  // This is an opt-in emergency reserve. Ordinary village restocking never
  // opens the shop for RandomTeleport and therefore keeps its old behavior.
  const emergencyTeleportCount = emergencyTeleportOption(options.emergencyTeleportCount);
  const needEmergencyTeleport = randomTeleportStock(initial) < emergencyTeleportCount;
  const emergencyTownTeleportCount = emergencyTownTeleportOption(options.emergencyTownTeleportCount);
  const needEmergencyTownTeleport = townTeleportStock(initial) < emergencyTownTeleportCount;
  const initialGold = integer(initial.gold, 'snapshot.gold');
  if (essentialTaoistForestWeapon && weaponNeeded && !taoistForestSupplyReady(initial)) {
    return {
      status: 'needsMandatorySupplies',
      stock: stock(initial),
      randomTeleport: randomTeleportStock(initial),
      townTeleport: townTeleportStock(initial),
      required: TAOIST_FOREST_MINIMUM_STOCK,
      gold: initialGold,
      reserveGold,
    };
  }
  if (needed.length === 0 && !weaponNeeded && !needEmergencyTeleport && !needEmergencyTownTeleport && !needHpMedium) {
    const equippedAmulet = className === 'taoist' ? await equipHeldAmulet(client) : null;
    return { status: 'sufficient', stock: stock(client.snapshot), gold: initialGold, equippedAmulet };
  }

  let spendableGold = spendable(initialGold, reserveGold);
  const cheapestCandidates = [
    ...needed.map(kind => SUPPLIES[kind].catalogPrice),
    ...(weaponNeeded ? [weaponRequirement.allowFallback
      ? weaponRequirement.catalog.at(-1).catalogPrice
      : weaponRequirement.preferred.catalogPrice] : []),
    ...(needEmergencyTownTeleport ? [TOWN_TELEPORT.catalogPrice] : []),
  ];
  const cheapestNeeded = cheapestCandidates.length > 0 ? Math.min(...cheapestCandidates) : 0;
  // This narrowly scoped q7 readiness purchase may use the existing account's
  // last reserve only after its independent HP/MP/transport floor is proven.
  // All ordinary/V1 restocking retains the generic reserve unchanged.
  const cheapestBudget = essentialTaoistForestWeapon ? initialGold : spendableGold;
  const gearCandidates = options.liquidateSuperseded === true
    ? supersededProgressionGearForSale(initial, options.progressionCandidates)
        .filter(item => normalized(item?.equipSlot) === 'weapon')
    : [];
  const materialCandidates = options.liquidateObsoleteMaterials === true
    ? obsoleteMaterialsForSale(initial, {
        progressionCandidates: options.progressionCandidates,
        protectedItemNames: options.protectedItemNames,
      })
    : [];
  const meatCandidates = materialCandidates.filter(item => normalized(item?.name) === 'venison');
  const generalMaterialCandidates = materialCandidates.filter(item => normalized(item?.name) !== 'venison');
  const liquidationCandidates = [...materialCandidates, ...gearCandidates];
  if (cheapestBudget < cheapestNeeded && liquidationCandidates.length === 0 && !needEmergencyTeleport && !needEmergencyTownTeleport) {
    return needsFunds(beforeStock, initialGold, spendableGold, reserveGold);
  }
  if (typeof navigateNear !== 'function') throw new Error('navigateNear is required to reach Merchant Ruben');

  const desiredSpendable = preferredWeaponCost + needed.reduce((total, kind) =>
    total + Math.max(0, targets[kind] - stock(client.snapshot)[kind]) *
      SUPPLIES[kind].catalogPrice, 0) +
    Math.max(0, emergencyTeleportCount - randomTeleportStock(client.snapshot)) *
      RANDOM_TELEPORT.catalogPrice +
    Math.max(0, emergencyTownTeleportCount - townTeleportStock(client.snapshot)) *
      TOWN_TELEPORT.catalogPrice;
  const sales = [];
  if (spendableGold < desiredSpendable && liquidationCandidates.length > 0) {
    const groups = [
      {
        candidates: meatCandidates,
        position: BUTCHER,
        open: () => openSellService(client, liveButcher, 'Butcher John'),
      },
      {
        candidates: generalMaterialCandidates,
        position: MATERIAL_DEALER,
        open: () => openSellService(client, liveMaterialDealer, 'Material Dealer Reece'),
      },
      {
        candidates: gearCandidates,
        position: BLACKSMITH,
        open: () => openSellService(client, liveBlacksmith, 'Blacksmith Smith'),
      },
    ];
    for (const group of groups) {
      spendableGold = await sellCandidateGroupUntilSpendable(
        client,
        navigateNear,
        group,
        desiredSpendable,
        reserveGold,
        sales,
        options.clearBlockingMonster,
      );
    }
  }
  if (essentialTaoistForestWeapon ? integer(client.snapshot.gold, 'snapshot.gold') < cheapestNeeded : spendableGold < cheapestNeeded) {
    return {
      ...needsFunds(stock(client.snapshot), integer(client.snapshot.gold, 'snapshot.gold'), spendableGold, reserveGold),
      sales,
    };
  }

  let weaponPurchase = null;
  let townPurchaseBeforeOrdinaryRestock = null;
  let freshTownDeficit = false;
  if (weaponNeeded) {
    await navigateSupplyService(client, navigateNear, BLACKSMITH, options.clearBlockingMonster);
    const { npc: blacksmith, goodsEvent } = await openBuyService(
      client,
      liveBlacksmith,
      'Blacksmith Smith',
    );
    const goods = goodsEvent?.payload;
    if (!goods || Number(goods.panelType) !== 0 || !Array.isArray(goods.list)) {
      throw new Error('Blacksmith Smith returned an invalid buy goods panel');
    }
    // Navigation and dialog work are live time. Re-prove the q7 exception's
    // recovery floor after those actions and before its irreversible purchase.
    if (essentialTaoistForestWeapon && !taoistForestSupplyReady(client.snapshot)) {
      return {
        status: 'needsMandatorySupplies',
        stock: stock(client.snapshot),
        randomTeleport: randomTeleportStock(client.snapshot),
        townTeleport: townTeleportStock(client.snapshot),
        required: TAOIST_FOREST_MINIMUM_STOCK,
        gold: integer(client.snapshot.gold, 'snapshot.gold'),
        reserveGold,
        sales,
      };
    }
    const affordableGold = essentialTaoistForestWeapon
      ? integer(client.snapshot.gold, 'snapshot.gold')
      : spendable(integer(client.snapshot.gold, 'snapshot.gold'), reserveGold);
    const weapon = affordableClassWeapon(
      goods.list,
      playerLevel(client.snapshot),
      affordableGold,
      weaponRequirement.catalog,
      { exactItemIndex: weaponRequirement.allowFallback ? null : weaponRequirement.preferred.itemIndex },
    );
    if (!weapon) {
      return {
        ...needsFunds(stock(client.snapshot), integer(client.snapshot.gold, 'snapshot.gold'), affordableGold, reserveGold),
        sales,
      };
    }
    const beforeGold = integer(client.snapshot.gold, 'snapshot.gold');
    const afterCommand = client.sequence;
    const beforeWeaponIds = new Set([
      ...(client.snapshot?.inventoryItems ?? []),
      ...(client.snapshot?.equipmentItems ?? []),
    ].map(item => Number(item?.uniqueId)).filter(Number.isSafeInteger));
    client.send({
      type: 'buyItem',
      itemIndex: shopRowId(weapon.row),
      count: 1,
      panelType: 0,
    });
    await client.wait(
      () => hasFreshWeaponPurchaseProof(
        client,
        afterCommand,
        beforeGold - weapon.unitPrice,
        weapon.itemIndex,
      ),
      `authoritative purchase of ${weapon.name}`,
      WAIT_MS,
    );
    const purchased = (client.snapshot?.inventoryItems ?? []).find(item =>
      templateIndex(item) === weapon.itemIndex &&
      validUniqueId(item?.uniqueId) &&
      !beforeWeaponIds.has(Number(item.uniqueId)));
    if (!purchased) {
      throw new Error(`${weapon.name} purchase produced no new authoritative inventory item`);
    }
    const equipAck = await client.request({
      type: 'equipItem',
      grid: 'inventory',
      uniqueId: purchased.uniqueId,
      to: 0,
    }, 'EquipItem', WAIT_MS);
    if (equipAck?.payload?.success !== true) {
      throw new Error(`Equip rejected for purchased ${weapon.name}`);
    }
    await client.wait(
      () => (client.snapshot?.equipmentItems ?? []).some(item =>
        Number(item?.uniqueId) === Number(purchased.uniqueId) &&
        normalized(item?.slot ?? item?.equipSlot) === 'weapon'),
      `equipped purchased ${weapon.name}`,
      WAIT_MS,
    );
    weaponPurchase = {
      itemIndex: weapon.itemIndex,
      name: weapon.name,
      shopItemId: shopRowId(weapon.row),
      quantity: 1,
      unitPrice: weapon.unitPrice,
      cost: weapon.unitPrice,
      uniqueId: Number(purchased.uniqueId),
      equipped: true,
      merchant: {
        objectId: blacksmith.objectId,
        name: blacksmith.name,
        x: Number(blacksmith.x),
        y: Number(blacksmith.y),
      },
    };
    spendableGold = spendable(integer(client.snapshot.gold, 'snapshot.gold'), reserveGold);
  }

  if (needed.length === 0 && !needEmergencyTeleport && !needEmergencyTownTeleport && !needHpMedium) {
    return {
      status: 'restocked',
      before: { gold: initialGold, ...beforeStock },
      after: { gold: integer(client.snapshot.gold, 'snapshot.gold'), ...stock(client.snapshot) },
      purchases: [],
      sales,
      weaponPurchase,
      equippedAmulet: null,
    };
  }

  if (needed.length === 0 && !needEmergencyTeleport && needEmergencyTownTeleport && !needHpMedium) {
    const townPurchase = await purchaseEmergencyTownTeleport(
      client,
      navigateNear,
      emergencyTownTeleportCount,
      reserveGold,
      options.clearBlockingMonster,
    );
    if (!townPurchase.purchase) {
      const currentGold = integer(client.snapshot.gold, 'snapshot.gold');
      return needsFunds(stock(client.snapshot), currentGold, spendable(currentGold, reserveGold), reserveGold);
    }
    const currentGold = integer(client.snapshot.gold, 'snapshot.gold');
    const freshStock = stock(client.snapshot);
    freshTownDeficit = relevant.some(kind => freshStock[kind] < lowStockByKind[kind]);
    if (freshTownDeficit) {
      // Scott travel can consume a floor stack (for example SoulFireBall
      // Amulets) after the initial sufficient-stock decision. Take one
      // ordinary Ruben pass from the live snapshot; do not recurse or alter
      // the normal supply priority.
      townPurchaseBeforeOrdinaryRestock = townPurchase.purchase;
      spendableGold = spendable(currentGold, reserveGold);
    } else {
      const equippedAmulet = className === 'taoist' ? await equipHeldAmulet(client) : null;
      return {
        status: 'restocked',
        merchant: { objectId: townPurchase.npc.objectId, name: townPurchase.npc.name, x: Number(townPurchase.npc.x), y: Number(townPurchase.npc.y) },
        before: { gold: initialGold, ...beforeStock },
        after: { gold: currentGold, ...freshStock },
        purchases: [townPurchase.purchase],
        sales,
        weaponPurchase,
        equippedAmulet,
      };
    }
  }

  await navigateSupplyService(client, navigateNear, RUBEN, options.clearBlockingMonster);
  let { npc, goodsEvent } = await openBuySellService(client, liveRuben, 'Merchant Ruben');
  let purchaseKinds;
  let rows;
  let emergencyRow;
  let hpMediumRow;
  const rebuildRubenPlan = event => {
    const goods = event?.payload;
    if (!goods || Number(goods.panelType) !== 0 || !Array.isArray(goods.list)) {
      throw new Error('Merchant Ruben returned an invalid buy goods panel');
    }
    // Navigation, blocker combat and liquidation can consume supplies. Rebuild
    // the shopping list from the fresh shop snapshot instead of freezing the
    // pre-travel stock decision.
    purchaseKinds = relevant.filter(kind => stock(client.snapshot)[kind] < lowStockByKind[kind]);
    rows = Object.fromEntries(purchaseKinds.map(kind => [kind, supplyRow(goods.list, SUPPLIES[kind])]));
    for (const kind of purchaseKinds) {
      if (!rows[kind]) throw new Error(`Merchant Ruben shop is missing ${SUPPLIES[kind].name}`);
    }
    emergencyRow = needEmergencyTeleport ? supplyRow(goods.list, RANDOM_TELEPORT) : null;
    if (needEmergencyTeleport && !emergencyRow) {
      throw new Error(`Merchant Ruben shop is missing ${RANDOM_TELEPORT.name} (${RANDOM_TELEPORT.itemIndex})`);
    }
    hpMediumRow = needHpMedium ? supplyRow(goods.list, SUPPLIES.hpMedium) : null;
  };
  rebuildRubenPlan(goodsEvent);

  // The first liquidation target is calculated before travel. If service
  // combat consumed a fresh low-stock supply, fund the rebuilt Ruben plan and
  // its pending TownTeleport once before any ordinary purchase can consume
  // the scroll's capital. This is deliberately one material-service pass.
  const pendingTownCost = needEmergencyTownTeleport && !townPurchaseBeforeOrdinaryRestock
    ? Math.max(0, emergencyTownTeleportCount - townTeleportStock(client.snapshot)) * TOWN_TELEPORT.catalogPrice
    : 0;
  const freshOrdinaryCost = purchaseKinds.reduce((total, kind) =>
    total + Math.max(0, targets[kind] - stock(client.snapshot)[kind]) *
      positiveInteger(rows[kind].price, `${SUPPLIES[kind].name} price`), 0) +
    (needEmergencyTeleport
      ? Math.max(0, emergencyTeleportCount - randomTeleportStock(client.snapshot)) *
        positiveInteger(emergencyRow.price, `${RANDOM_TELEPORT.name} price`)
      : 0) +
    (needHpMedium && hpMediumRow && Number.isSafeInteger(Number(hpMediumRow.price)) && Number(hpMediumRow.price) > 0
      ? Math.max(0, targets.hpMedium - hpMediumDrugCount(client.snapshot)) * Number(hpMediumRow.price)
      : 0) +
    pendingTownCost;
  const freshSpendableGold = spendable(integer(client.snapshot.gold, 'snapshot.gold'), reserveGold);
  if (pendingTownCost > 0 && freshSpendableGold < freshOrdinaryCost) {
    const soldIds = new Set(sales.map(sale => Number(sale.uniqueId)));
    // Re-evaluate from the live snapshot: a sold item id can be recycled by a
    // later authoritative snapshot, and it must never be treated as the old
    // eligible material merely because the stale candidate had that id.
    const freshMaterials = options.liquidateObsoleteMaterials === true
      ? obsoleteMaterialsForSale(client.snapshot, {
          progressionCandidates: options.progressionCandidates,
          protectedItemNames: options.protectedItemNames,
        })
      : [];
    const remainingMeat = freshMaterials.filter(candidate =>
      normalized(candidate?.name) === 'venison' && !soldIds.has(Number(candidate.uniqueId)));
    const remainingMaterials = freshMaterials.filter(candidate =>
      normalized(candidate?.name) !== 'venison' && !soldIds.has(Number(candidate.uniqueId)));
    const topUpGroup = remainingMeat.length > 0
      ? {
          candidates: remainingMeat,
          position: BUTCHER,
          open: () => openSellService(client, liveButcher, 'Butcher John'),
          currentCandidates: () => obsoleteMaterialsForSale(client.snapshot, {
            progressionCandidates: options.progressionCandidates,
            protectedItemNames: options.protectedItemNames,
          }).filter(candidate => normalized(candidate?.name) === 'venison' && !soldIds.has(Number(candidate.uniqueId))),
        }
      : remainingMaterials.length > 0
        ? {
            candidates: remainingMaterials,
            position: MATERIAL_DEALER,
            open: () => openSellService(client, liveMaterialDealer, 'Material Dealer Reece'),
            currentCandidates: () => obsoleteMaterialsForSale(client.snapshot, {
              progressionCandidates: options.progressionCandidates,
              protectedItemNames: options.protectedItemNames,
            }).filter(candidate => normalized(candidate?.name) !== 'venison' && !soldIds.has(Number(candidate.uniqueId))),
          }
        : null;
    if (topUpGroup) {
      spendableGold = await sellCandidateGroupUntilSpendable(
        client,
        navigateNear,
        topUpGroup,
        freshOrdinaryCost,
        reserveGold,
        sales,
        options.clearBlockingMonster,
      );
      // The material service owns the active NPC dialog. Re-open Ruben and
      // replace every row and deficit with the returned authoritative panel;
      // this is not another liquidation pass.
      await navigateSupplyService(client, navigateNear, RUBEN, options.clearBlockingMonster);
      ({ npc, goodsEvent } = await openBuySellService(client, liveRuben, 'Merchant Ruben'));
      rebuildRubenPlan(goodsEvent);
    }
  }

  // Calculate the full plan once so it always preserves the emergency gold
  // reserve. HP is intentionally first because every class depends on it.
  let budget = Math.min(
    spendableGold,
    spendable(integer(client.snapshot?.gold, 'snapshot.gold'), reserveGold),
  );
  const plan = [];
  for (const kind of purchaseKinds) {
    const row = rows[kind];
    const unitPrice = positiveInteger(row.price, `${SUPPLIES[kind].name} price`);
    const deficit = Math.max(0, targets[kind] - stock(client.snapshot)[kind]);
    const quantity = Math.min(deficit, Math.floor(budget / unitPrice));
    if (quantity > 0) {
      for (const chunk of purchaseChunks(client.snapshot, kind, quantity)) {
        plan.push({ kind, row, quantity: chunk, unitPrice, cost: chunk * unitPrice });
      }
      budget -= quantity * unitPrice;
    }
  }
  if (needEmergencyTeleport) {
    const unitPrice = positiveInteger(emergencyRow.price, `${RANDOM_TELEPORT.name} price`);
    const existing = randomTeleportStock(client.snapshot);
    const quantity = Math.min(
      Math.max(0, emergencyTeleportCount - existing),
      Math.max(0, Math.floor(budget / unitPrice)),
    );
    if (quantity > 0) {
      plan.push({ kind: 'randomTeleport', row: emergencyRow, quantity, unitPrice, cost: quantity * unitPrice, beforeQuantity: existing });
      budget -= quantity * unitPrice;
    }
  }
  let hpMediumSkipped = null;
  if (needHpMedium) {
    if (!hpMediumRow || !Number.isSafeInteger(Number(hpMediumRow.price)) || Number(hpMediumRow.price) <= 0) {
      hpMediumSkipped = 'missingLiveShopRow';
    } else {
      const unitPrice = Number(hpMediumRow.price);
      const existing = hpMediumDrugCount(client.snapshot);
      const quantity = Math.min(
        Math.max(0, targets.hpMedium - existing),
        Math.max(0, Math.floor(budget / unitPrice)),
      );
      if (quantity > 0) {
        for (const chunk of purchaseChunks(client.snapshot, 'hpMedium', quantity)) {
          plan.push({ kind: 'hpMedium', row: hpMediumRow, quantity: chunk, unitPrice, cost: chunk * unitPrice });
        }
        budget -= quantity * unitPrice;
      } else {
        hpMediumSkipped = 'insufficientFunds';
      }
    }
  }
  if (plan.length === 0) {
    if (needHpMedium) {
      return {
        status: 'restocked',
        before: { gold: initialGold, ...beforeStock },
        after: { gold: integer(client.snapshot.gold, 'snapshot.gold'), ...stock(client.snapshot) },
        purchases: [],
        sales,
        weaponPurchase,
        ...(hpMediumSkipped ? { hpMediumSkipped } : {}),
        equippedAmulet: className === 'taoist' ? await equipHeldAmulet(client) : null,
      };
    }
    if (freshTownDeficit) {
      const currentGold = integer(client.snapshot.gold, 'snapshot.gold');
      return {
        ...needsFunds(stock(client.snapshot), currentGold, spendable(currentGold, reserveGold), reserveGold),
        sales,
        purchases: townPurchaseBeforeOrdinaryRestock ? [townPurchaseBeforeOrdinaryRestock] : [],
        weaponPurchase,
      };
    }
    return needsFunds(beforeStock, initialGold, spendableGold, reserveGold);
  }

  const authoritativeBefore = {
    gold: integer(client.snapshot.gold, 'snapshot.gold'),
    ...stock(client.snapshot),
  };
  const purchases = townPurchaseBeforeOrdinaryRestock ? [townPurchaseBeforeOrdinaryRestock] : [];
  for (const purchase of plan) {
    const beforeGold = integer(client.snapshot.gold, 'snapshot.gold');
    const beforeQuantity = purchase.kind === 'randomTeleport'
      ? randomTeleportStock(client.snapshot)
      : purchase.kind === 'hpMedium'
        ? hpMediumDrugCount(client.snapshot)
        : stock(client.snapshot)[purchase.kind];
    const afterCommand = client.sequence;
    client.send({
      type: 'buyItem',
      // Despite the command field name, the protocol requires the fresh shop
      // row id here, not the item's template index.
      itemIndex: shopRowId(purchase.row),
      count: purchase.quantity,
      panelType: 0,
    });
    await client.wait(
      () => purchase.kind === 'randomTeleport'
        ? hasFreshRandomTeleportPurchaseProof(client, afterCommand, beforeGold - purchase.cost, beforeQuantity + purchase.quantity)
        : purchase.kind === 'hpMedium'
          ? hasFreshHpMediumPurchaseProof(client, afterCommand, beforeGold - purchase.cost, beforeQuantity + purchase.quantity)
        : hasFreshPurchaseProof(client, afterCommand, purchase.kind, beforeGold - purchase.cost, beforeQuantity + purchase.quantity),
      `authoritative purchase of ${purchase.kind === 'randomTeleport' ? RANDOM_TELEPORT.name : SUPPLIES[purchase.kind].name}`,
      WAIT_MS,
    );
    purchases.push({
      itemIndex: purchase.kind === 'randomTeleport' ? RANDOM_TELEPORT.itemIndex : SUPPLIES[purchase.kind].itemIndex,
      name: purchase.kind === 'randomTeleport' ? RANDOM_TELEPORT.name : SUPPLIES[purchase.kind].name,
      shopItemId: shopRowId(purchase.row),
      quantity: purchase.quantity,
      unitPrice: purchase.unitPrice,
      cost: purchase.cost,
    });
  }

  if (needEmergencyTownTeleport && !townPurchaseBeforeOrdinaryRestock) {
    const townPurchase = await purchaseEmergencyTownTeleport(
      client,
      navigateNear,
      emergencyTownTeleportCount,
      reserveGold,
      options.clearBlockingMonster,
    );
    if (!townPurchase.purchase) {
      return {
        ...needsFunds(stock(client.snapshot), integer(client.snapshot.gold, 'snapshot.gold'), spendable(integer(client.snapshot.gold, 'snapshot.gold'), reserveGold), reserveGold),
        sales,
        purchases,
        weaponPurchase,
      };
    }
    purchases.push(townPurchase.purchase);
  }

  const equippedAmulet = className === 'taoist' ? await equipHeldAmulet(client) : null;
  return {
    status: 'restocked',
    merchant: { objectId: npc.objectId, name: npc.name, x: Number(npc.x), y: Number(npc.y) },
    before: authoritativeBefore,
    after: { gold: integer(client.snapshot.gold, 'snapshot.gold'), ...stock(client.snapshot) },
    purchases,
    sales,
    weaponPurchase,
    ...(hpMediumSkipped ? { hpMediumSkipped } : {}),
    equippedAmulet,
  };
}

/**
 * Buy exactly one ordinary Basic HP Potion for the V2 purchase condition.
 *
 * Samuel is the real Bichon Wall Potion1 merchant, so this stays local to the
 * city route and does not add the regular village MP/Amulet restock policy.
 * Completion requires one fresh snapshot with the precise live shop debit and
 * an inventory-or-belt HP-potion increase.
 */
export async function purchaseV2BasicHpPotion(client, navigateNear, options = {}) {
  const initial = client?.snapshot;
  if (!initial || typeof initial !== 'object') throw new Error('Cannot purchase a V2 Basic Potion without an authoritative snapshot');
  if (String(initial.mapFileName ?? '') !== '0') {
    throw new Error(`Alchemist Samuel Basic Potion purchase requires map 0, received ${initial.mapFileName ?? 'unknown'}`);
  }

  await navigateSupplyService(client, navigateNear, BICHON_WALL_SAMUEL, options.clearBlockingMonster);
  const { npc, goodsEvent } = await openBuySellService(client, liveBichonWallSamuel, 'Alchemist Samuel');
  const goods = goodsEvent?.payload;
  if (!goods || Number(goods.panelType) !== 0 || !Array.isArray(goods.list)) {
    throw new Error('Alchemist Samuel returned an invalid buy goods panel');
  }
  const row = supplyRow(goods.list, SUPPLIES.hp);
  if (!row) throw new Error(`Alchemist Samuel shop is missing ${SUPPLIES.hp.name} (${SUPPLIES.hp.itemIndex})`);
  const unitPrice = positiveInteger(row.price, `${SUPPLIES.hp.name} price`);
  const beforeGold = integer(client.snapshot?.gold, 'snapshot.gold');
  if (beforeGold < unitPrice) {
    throw new Error(`Alchemist Samuel Basic Potion purchase requires ${unitPrice} gold, has ${beforeGold}`);
  }
  const beforeTotalQuantity = stock(client.snapshot).hp;
  const beforeQuantity = basicHpPotionBagBeltCount(client.snapshot);
  const afterCommand = Number(client.sequence);
  client.send({ type: 'buyItem', itemIndex: shopRowId(row), count: 1, panelType: 0 });
  await client.wait(
    () => hasFreshPurchaseProof(
      client,
      afterCommand,
      'hp',
      beforeGold - unitPrice,
      beforeTotalQuantity + 1,
      { expectedBagBeltHp: beforeQuantity + 1 },
    ),
    `authoritative purchase of ${SUPPLIES.hp.name} from Alchemist Samuel`,
    WAIT_MS,
  );
  return {
    merchant: { objectId: numericId(npc.objectId, 'Alchemist Samuel objectId'), name: npc.name, x: Number(npc.x), y: Number(npc.y) },
    purchase: { itemIndex: SUPPLIES.hp.itemIndex, name: SUPPLIES.hp.name, shopItemId: shopRowId(row), quantity: 1, unitPrice, cost: unitPrice },
  };
}

/**
 * Use one authoritative RandomTeleport stack through the normal public
 * useItem command. Success requires both a fresh quantity decrease and a
 * fresh authoritative player-coordinate change; an ack alone is insufficient.
 */
export async function useRandomTeleport(client, options = {}) {
  const item = findRandomTeleport(client?.snapshot, options.uniqueId);
  if (!item) throw new Error('No authoritative RandomTeleport item is available');
  const grid = options.grid ?? randomTeleportGrid(client.snapshot, item);
  const beforeQuantity = randomTeleportStock(client.snapshot);
  const beforePlayer = playerPosition(client.snapshot);
  if (!beforePlayer) throw new Error('RandomTeleport requires an authoritative player position');
  const afterCommand = Number(client.sequence ?? 0);
  const ack = await client.request({ type: 'useItem', uniqueId: item.uniqueId, grid }, 'UseItem', WAIT_MS);
  if (ack?.payload?.success !== true) throw new Error('RandomTeleport UseItem was rejected');
  // UseItem receipts and UserLocation can arrive independently. Request one
  // bounded authoritative snapshot so item consumption and relocation are
  // proven together rather than inferred from either packet alone.
  client.send({ type: 'clientVersion' });
  await client.wait(
    () => hasFreshRandomTeleportUseProof(client, afterCommand, beforeQuantity, beforePlayer),
    'authoritative RandomTeleport use',
    WAIT_MS,
  );
  const after = playerPosition(client.snapshot);
  return {
    uniqueId: Number(item.uniqueId), grid,
    quantityBefore: beforeQuantity,
    quantityAfter: randomTeleportStock(client.snapshot),
    from: beforePlayer,
    to: after,
  };
}

/**
 * Use one authoritative TownTeleport stack through the normal public
 * useItem command. Success requires fresh consumption plus an authoritative
 * coordinate or map change; an ack alone is insufficient.
 */
export async function useTownTeleport(client, options = {}) {
  const item = findTownTeleport(client?.snapshot, options.uniqueId);
  if (!item) throw new Error('No authoritative TownTeleport item is available');
  const grid = options.grid ?? townTeleportGrid(client.snapshot, item);
  const beforeQuantity = townTeleportStock(client.snapshot);
  const beforePlayer = playerPosition(client.snapshot);
  if (!beforePlayer) throw new Error('TownTeleport requires an authoritative player position');
  const beforeMapFileName = String(client.snapshot?.mapFileName ?? '');
  const afterCommand = Number(client.sequence ?? 0);
  const ack = await client.request({ type: 'useItem', uniqueId: item.uniqueId, grid }, 'UseItem', WAIT_MS);
  if (ack?.payload?.success !== true) throw new Error('TownTeleport UseItem was rejected');
  client.send({ type: 'clientVersion' });
  await client.wait(
    () => hasFreshTownTeleportUseProof(client, afterCommand, beforeQuantity, beforePlayer, beforeMapFileName),
    'authoritative TownTeleport use',
    WAIT_MS,
  );
  const after = playerPosition(client.snapshot);
  return {
    uniqueId: Number(item.uniqueId), grid,
    quantityBefore: beforeQuantity,
    quantityAfter: townTeleportStock(client.snapshot),
    from: beforePlayer,
    to: after,
    fromMapFileName: beforeMapFileName,
    toMapFileName: String(client.snapshot?.mapFileName ?? ''),
  };
}

/**
 * Share one cooldown across navigation, combat retreat, and recovery loops.
 * Crystal can relocate a player into another hostile pack, so immediately
 * spending the next scroll is usually wasteful. A shorter critical cooldown
 * still permits a genuinely dying player to make another bounded attempt.
 */
export function createRandomTeleportEmergencyEscape({
  cooldownMs = 12_000,
  criticalCooldownMs = 3_000,
  criticalHpRatio = 0.35,
  now = Date.now,
  teleport = useRandomTeleport,
} = {}) {
  let lastUsedAt = Number.NEGATIVE_INFINITY;
  return async function emergencyEscape(client) {
    const hp = observedPlayerHp(client?.snapshot);
    const maxHp = Math.max(1, Number(client?.snapshot?.playerMaxHp ?? 0));
    const hpRatio = hp / maxHp;
    const requiredCooldown = hpRatio <= criticalHpRatio
      ? Math.max(0, Number(criticalCooldownMs) || 0)
      : Math.max(0, Number(cooldownMs) || 0);
    const elapsedMs = now() - lastUsedAt;
    if (elapsedMs < requiredCooldown) {
      return {
        deferred: true,
        retryAfterMs: Math.ceil(requiredCooldown - elapsedMs),
        hpRatio,
      };
    }
    const result = await teleport(client);
    if (result !== false) lastUsedAt = now();
    return result;
  };
}

/** Count usable public-protocol emergency scrolls in bag and belt. */
export function randomTeleportCount(snapshot) {
  return randomTeleportStock(snapshot);
}

/** Count usable public-protocol TownTeleport scrolls in bag and belt. */
export function townTeleportCount(snapshot) {
  return townTeleportStock(snapshot);
}

function liveRuben(snapshot) {
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'npc' &&
    normalized(entity?.name) === normalized(RUBEN.name) &&
    Number(entity?.x) === RUBEN.x && Number(entity?.y) === RUBEN.y
  ) ?? null;
}

function liveScott(snapshot) {
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'npc' &&
    normalized(entity?.name) === normalized(SCOTT.name) &&
    Number(entity?.x) === SCOTT.x && Number(entity?.y) === SCOTT.y
  ) ?? null;
}

function liveBichonWallSamuel(snapshot) {
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'npc' &&
    normalized(entity?.name) === normalized(BICHON_WALL_SAMUEL.name) &&
    Number(entity?.x) === BICHON_WALL_SAMUEL.x && Number(entity?.y) === BICHON_WALL_SAMUEL.y
  ) ?? null;
}

function liveBlacksmith(snapshot) {
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'npc' &&
    normalized(entity?.name) === normalized(BLACKSMITH.name) &&
    Number(entity?.x) === BLACKSMITH.x && Number(entity?.y) === BLACKSMITH.y
  ) ?? null;
}

function liveButcher(snapshot) {
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'npc' &&
    normalized(entity?.name) === normalized(BUTCHER.name) &&
    Number(entity?.x) === BUTCHER.x && Number(entity?.y) === BUTCHER.y
  ) ?? null;
}

function liveMaterialDealer(snapshot) {
  return (snapshot?.entities ?? []).find(entity =>
    String(entity?.kind ?? '').toLowerCase() === 'npc' &&
    normalized(entity?.name) === normalized(MATERIAL_DEALER.name) &&
    Number(entity?.x) === MATERIAL_DEALER.x && Number(entity?.y) === MATERIAL_DEALER.y
  ) ?? null;
}

async function navigateSupplyService(client, navigateNear, target, clearBlockingMonster) {
  const destination = { x: Number(target.x), y: Number(target.y) };
  for (let attempt = 0; attempt < 4; attempt += 1) {
    try {
      const actor = (client.snapshot?.entities ?? []).find(entity =>
        Number(entity?.objectId) === Number(client.snapshot?.playerObjectId));
      const travelDistance = actor && Number.isFinite(Number(actor.x)) && Number.isFinite(Number(actor.y))
        ? Math.max(
            Math.abs(Number(actor.x) - destination.x),
            Math.abs(Number(actor.y) - destination.y),
          )
        : 0;
      if (travelDistance > 96) {
        return await navigateNear(destination, 1, undefined, {
          maxAttempts: Math.min(2_048, travelDistance + 96),
        });
      }
      return await navigateNear(destination, 1);
    } catch (error) {
      if (!String(error?.message ?? '').startsWith('No walk path') ||
          typeof clearBlockingMonster !== 'function' || attempt >= 3) throw error;
      const actor = (client.snapshot?.entities ?? []).find(entity =>
        Number(entity?.objectId) === Number(client.snapshot?.playerObjectId));
      const blocker = (client.snapshot?.entities ?? [])
        .filter(entity => normalized(entity?.kind) === 'monster' &&
          normalized(entity?.disposition) === 'hostile' && entity?.dead !== true &&
          Number(entity?.hp) > 0 && actor && (
            tileDistance(entity, actor) <= 1 || tileDistance(entity, destination) <= 2
          ))
        .sort((left, right) => Number(tileDistance(left, actor) > 1) - Number(tileDistance(right, actor) > 1) ||
          Number(left.hp ?? 0) - Number(right.hp ?? 0) ||
          tileDistance(left, destination) - tileDistance(right, destination) ||
          Number(left.objectId) - Number(right.objectId))[0];
      if (!blocker) throw error;
      if (typeof client.record === 'function') {
        client.record('diagnostic', {
          type: 'supplyServiceBlockerCombat',
          objectId: Number(blocker.objectId),
          name: String(blocker.name ?? ''),
          attempt: attempt + 1,
        });
      }
      await clearBlockingMonster(client, blocker);
    }
  }
  throw new Error(`Unable to reach supply service at ${target.x},${target.y}`);
}

async function sellCandidateGroupUntilSpendable(
  client,
  navigateNear,
  group,
  requiredSpendable,
  reserveGold,
  sales,
  clearBlockingMonster,
) {
  let availableGold = spendable(integer(client.snapshot.gold, 'snapshot.gold'), reserveGold);
  if (availableGold >= requiredSpendable || group.candidates.length === 0) return availableGold;
  await navigateSupplyService(client, navigateNear, group.position, clearBlockingMonster);
  const { npc: seller } = await group.open();
  const candidates = typeof group.currentCandidates === 'function'
    ? group.currentCandidates()
    : group.candidates;
  for (const candidate of candidates) {
    if (availableGold >= requiredSpendable) break;
    const quantity = positiveInteger(candidate.quantity ?? 1, `${candidate.name} quantity`);
    const unitValue = positiveInteger(candidate.sellValue, `${candidate.name} sellValue`);
    const beforeGold = integer(client.snapshot.gold, 'snapshot.gold');
    const afterCommand = Number(client.sequence ?? 0);
    const acknowledgement = await client.request({
      type: 'sellItem',
      uniqueId: numericId(candidate.uniqueId, `${candidate.name} uniqueId`),
      count: quantity,
    }, 'SellItem', WAIT_MS);
    if (acknowledgement?.payload?.success !== true) {
      throw new Error(`${seller.name} rejected sale of ${candidate.name}`);
    }
    await client.wait(
      () => hasFreshSaleProof(
        client,
        afterCommand,
        candidate.uniqueId,
        beforeGold + unitValue * quantity,
      ),
      `authoritative sale of ${candidate.name}`,
      WAIT_MS,
    );
    sales.push({
      uniqueId: Number(candidate.uniqueId),
      name: String(candidate.name),
      quantity,
      gold: unitValue * quantity,
    });
    availableGold = spendable(integer(client.snapshot.gold, 'snapshot.gold'), reserveGold);
  }
  return availableGold;
}

async function purchaseEmergencyTownTeleport(client, navigateNear, targetCount, reserveGold, clearBlockingMonster) {
  const existing = townTeleportStock(client.snapshot);
  if (existing >= targetCount) return { purchase: null, npc: null };
  const beforeGold = integer(client.snapshot.gold, 'snapshot.gold');
  const availableGold = spendable(beforeGold, reserveGold);
  if (availableGold < TOWN_TELEPORT.catalogPrice) return { purchase: null, npc: null };

  await navigateSupplyService(client, navigateNear, SCOTT, clearBlockingMonster);
  const { npc, goodsEvent } = await openBuySellService(client, liveScott, 'Merchant Scott');
  const goods = goodsEvent?.payload;
  if (!goods || Number(goods.panelType) !== 0 || !Array.isArray(goods.list)) {
    throw new Error('Merchant Scott returned an invalid buy goods panel');
  }
  const row = supplyRow(goods.list, TOWN_TELEPORT);
  if (!row) throw new Error(`Merchant Scott shop is missing ${TOWN_TELEPORT.name} (${TOWN_TELEPORT.itemIndex})`);
  const unitPrice = positiveInteger(row.price, `${TOWN_TELEPORT.name} price`);
  const quantity = Math.min(
    Math.max(0, targetCount - townTeleportStock(client.snapshot)),
    Math.max(0, Math.floor(availableGold / unitPrice)),
  );
  if (quantity <= 0) return { purchase: null, npc };

  const currentGold = integer(client.snapshot.gold, 'snapshot.gold');
  const beforeQuantity = townTeleportStock(client.snapshot);
  const afterCommand = client.sequence;
  client.send({
    type: 'buyItem',
    itemIndex: shopRowId(row),
    count: quantity,
    panelType: 0,
  });
  await client.wait(
    () => hasFreshTownTeleportPurchaseProof(client, afterCommand, currentGold - quantity * unitPrice, beforeQuantity + quantity),
    `authoritative purchase of ${TOWN_TELEPORT.name}`,
    WAIT_MS,
  );
  return {
    npc,
    purchase: {
      itemIndex: TOWN_TELEPORT.itemIndex,
      name: TOWN_TELEPORT.name,
      shopItemId: shopRowId(row),
      quantity,
      unitPrice,
      cost: quantity * unitPrice,
    },
  };
}

function tileDistance(left, right) {
  return Math.max(
    Math.abs(Number(left?.x) - Number(right?.x)),
    Math.abs(Number(left?.y) - Number(right?.y)),
  );
}

async function openBuySellService(client, findNpc, label) {
  const npc = await client.wait(() => findNpc(client.snapshot), `live ${label}`, WAIT_MS);
  const afterInteract = client.sequence;
  client.send({ type: 'interact', objectId: numericId(npc.objectId, `${label} objectId`) });
  await client.wait(
    () => client.sequence > afterInteract && dialogFor(client.snapshot, npc.objectId),
    `${label} dialog`,
    WAIT_MS,
  );
  const link = buySellLink(client.snapshot);
  if (!link) throw new Error(`${label} dialog has no enabled @BuySell link`);
  const goodsEvent = await client.request(
    { type: 'selectNpcDialog', target: link.target },
    'NPCGoods',
    WAIT_MS,
  );
  return { npc, goodsEvent };
}

async function openBuyService(client, findNpc, label) {
  const npc = await client.wait(() => findNpc(client.snapshot), `live ${label}`, WAIT_MS);
  const afterInteract = client.sequence;
  client.send({ type: 'interact', objectId: numericId(npc.objectId, `${label} objectId`) });
  await client.wait(
    () => client.sequence > afterInteract && dialogFor(client.snapshot, npc.objectId),
    `${label} dialog`,
    WAIT_MS,
  );
  const link = serviceLink(client.snapshot, '@buy') ?? serviceLink(client.snapshot, '@buysell');
  if (!link) throw new Error(`${label} dialog has no enabled @Buy or @BuySell link`);
  const goodsEvent = await client.request(
    { type: 'selectNpcDialog', target: link.target },
    'NPCGoods',
    WAIT_MS,
  );
  return { npc, goodsEvent };
}

async function openSellService(client, findNpc, label) {
  const npc = await client.wait(() => findNpc(client.snapshot), `live ${label}`, WAIT_MS);
  const afterInteract = client.sequence;
  client.send({ type: 'interact', objectId: numericId(npc.objectId, `${label} objectId`) });
  await client.wait(
    () => client.sequence > afterInteract && dialogFor(client.snapshot, npc.objectId),
    `${label} dialog`,
    WAIT_MS,
  );
  // Crystal merchants may expose selling either as a dedicated @Sell action
  // or as the combined @BuySell action. The combined service emits NPCGoods
  // followed by NPCSell, so waiting for NPCSell proves that the same ordinary
  // client sale capability is active before any SellItem command is sent.
  const link = serviceLink(client.snapshot, '@sell') ?? serviceLink(client.snapshot, '@buysell');
  if (!link) throw new Error(`${label} dialog has no enabled @Sell or @BuySell link`);
  const sellEvent = await client.request(
    { type: 'selectNpcDialog', target: link.target },
    'NPCSell',
    WAIT_MS,
  );
  return { npc, sellEvent };
}

function dialogFor(snapshot, objectId) {
  return String(snapshot?.activeNpcDialog?.npcObjectId) === String(objectId);
}

function buySellLink(snapshot) {
  return serviceLink(snapshot, '@buysell');
}

function serviceLink(snapshot, target) {
  if (!snapshot?.activeNpcDialog) return null;
  return (snapshot.activeNpcDialog.links ?? []).find(link =>
    String(link?.target ?? '').trim().toLowerCase() === target &&
    link?.disabled !== true && link?.enabled !== false
  ) ?? null;
}

function supplyRow(list, supply) {
  return list
    .filter(row => templateIndex(row) === supply.itemIndex && normalized(row?.name) === normalized(supply.name))
    .sort((a, b) => Number(a?.count ?? 1) - Number(b?.count ?? 1))[0] ?? null;
}

function affordableClassWeapon(list, level, availableGold, catalog, { exactItemIndex = null } = {}) {
  for (const candidate of catalog) {
    if (level < candidate.requiredLevel) continue;
    if (exactItemIndex != null && candidate.itemIndex !== exactItemIndex) continue;
    const row = list.find(item =>
      templateIndex(item) === candidate.itemIndex &&
      normalized(item?.name) === normalized(candidate.name) &&
      Number.isSafeInteger(Number(item?.price)) && Number(item.price) > 0);
    if (row && Number(row.price) <= availableGold) {
      return { ...candidate, row, unitPrice: Number(row.price) };
    }
  }
  return null;
}

function hasFreshPurchaseProof(client, afterSequence, kind, expectedGold, expectedQuantity, options = {}) {
  return client.events.some(event =>
    event?.sequence > afterSequence &&
    event?.direction === 'received' &&
    event?.type === 'worldSnapshot' &&
    Number(event?.payload?.gold) === expectedGold &&
    stock(event?.payload)[kind] === expectedQuantity &&
    (options.expectedBagBeltHp == null || basicHpPotionBagBeltCount(event?.payload) === options.expectedBagBeltHp)
  );
}

function hasFreshRandomTeleportPurchaseProof(client, afterSequence, expectedGold, expectedQuantity) {
  return client.events.some(event =>
    event?.sequence > afterSequence &&
    event?.direction === 'received' &&
    event?.type === 'worldSnapshot' &&
    Number(event?.payload?.gold) === expectedGold &&
    randomTeleportStock(event?.payload) === expectedQuantity
  );
}

function hasFreshHpMediumPurchaseProof(client, afterSequence, expectedGold, expectedQuantity) {
  return client.events.some(event =>
    event?.sequence > afterSequence &&
    event?.direction === 'received' &&
    event?.type === 'worldSnapshot' &&
    Number(event?.payload?.gold) === expectedGold &&
    hpMediumDrugCount(event?.payload) === expectedQuantity
  );
}

function hasFreshTownTeleportPurchaseProof(client, afterSequence, expectedGold, expectedQuantity) {
  return client.events.some(event =>
    event?.sequence > afterSequence &&
    event?.direction === 'received' &&
    event?.type === 'worldSnapshot' &&
    Number(event?.payload?.gold) === expectedGold &&
    townTeleportStock(event?.payload) === expectedQuantity
  );
}

function hasFreshRandomTeleportUseProof(client, afterSequence, beforeQuantity, beforePlayer) {
  return client.events.some(event => {
    if (event?.sequence <= afterSequence || event?.direction !== 'received' || event?.type !== 'worldSnapshot') return false;
    const payload = event.payload;
    const afterPlayer = playerPosition(payload);
    return randomTeleportStock(payload) < beforeQuantity &&
      afterPlayer && (afterPlayer.x !== beforePlayer.x || afterPlayer.y !== beforePlayer.y);
  });
}

function hasFreshTownTeleportUseProof(client, afterSequence, beforeQuantity, beforePlayer, beforeMapFileName) {
  return client.events.some(event => {
    if (event?.sequence <= afterSequence || event?.direction !== 'received' || event?.type !== 'worldSnapshot') return false;
    const payload = event.payload;
    const afterPlayer = playerPosition(payload);
    const afterMapFileName = String(payload?.mapFileName ?? '');
    return townTeleportStock(payload) < beforeQuantity &&
      afterPlayer && (afterMapFileName !== beforeMapFileName || afterPlayer.x !== beforePlayer.x || afterPlayer.y !== beforePlayer.y);
  });
}

function hasFreshWeaponPurchaseProof(client, afterSequence, expectedGold, expectedItemIndex) {
  return client.events.some(event =>
    event?.sequence > afterSequence &&
    event?.direction === 'received' &&
    event?.type === 'worldSnapshot' &&
    Number(event?.payload?.gold) === expectedGold &&
    [...(event?.payload?.inventoryItems ?? []), ...(event?.payload?.equipmentItems ?? [])]
      .some(item => templateIndex(item) === expectedItemIndex)
  );
}

function hasFreshSaleProof(client, afterSequence, uniqueId, expectedGold) {
  return client.events.some(event =>
    event?.sequence > afterSequence &&
    event?.direction === 'received' &&
    event?.type === 'worldSnapshot' &&
    Number(event?.payload?.gold) === expectedGold &&
    !(event?.payload?.inventoryItems ?? []).some(item =>
      Number(item?.uniqueId) === Number(uniqueId))
  );
}

function stock(snapshot) {
  const result = { hp: 0, mp: 0, amulet: 0 };
  for (const item of [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? []), ...(snapshot?.equipmentItems ?? [])]) {
    const kind = templateIndex(item) === SUPPLIES.hp.itemIndex || normalized(item?.name) === normalized(SUPPLIES.hp.name)
      ? 'hp'
      : templateIndex(item) === SUPPLIES.mp.itemIndex || normalized(item?.name) === normalized(SUPPLIES.mp.name)
        ? 'mp'
        : templateIndex(item) === SUPPLIES.amulet.itemIndex || normalized(item?.name) === normalized(SUPPLIES.amulet.name)
          ? 'amulet'
          : null;
    if (kind) result[kind] += Math.max(0, Number(item?.quantity ?? 1));
  }
  return result;
}

function basicHpPotionBagBeltCount(snapshot) {
  return [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])]
    .filter(item => templateIndex(item) === SUPPLIES.hp.itemIndex || normalized(item?.name) === normalized(SUPPLIES.hp.name))
    .reduce((total, item) => total + Math.max(0, Number(item?.quantity ?? 1)), 0);
}

export function hpMediumDrugCount(snapshot) {
  return [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? []), ...(snapshot?.equipmentItems ?? [])]
    .filter(item => templateIndex(item) === SUPPLIES.hpMedium.itemIndex ||
      normalized(item?.name) === normalized(SUPPLIES.hpMedium.name))
    .reduce((total, item) => total + Math.max(0, Number(item?.quantity ?? 1)), 0);
}

function randomTeleportStock(snapshot) {
  return [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])]
    .filter(item => templateIndex(item) === RANDOM_TELEPORT.itemIndex &&
      normalized(item?.name) === normalized(RANDOM_TELEPORT.name))
    .reduce((total, item) => total + Math.max(0, Number(item?.quantity ?? 1)), 0);
}

function townTeleportStock(snapshot) {
  return [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])]
    .filter(item => templateIndex(item) === TOWN_TELEPORT.itemIndex &&
      normalized(item?.name) === normalized(TOWN_TELEPORT.name))
    .reduce((total, item) => total + Math.max(0, Number(item?.quantity ?? 1)), 0);
}

function findRandomTeleport(snapshot, uniqueId = null) {
  const items = [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])]
    .filter(item => templateIndex(item) === RANDOM_TELEPORT.itemIndex &&
      normalized(item?.name) === normalized(RANDOM_TELEPORT.name) && heldQuantity(item) !== null && validUniqueId(item?.uniqueId));
  return uniqueId == null ? items[0] ?? null : items.find(item => Number(item.uniqueId) === Number(uniqueId)) ?? null;
}

function findTownTeleport(snapshot, uniqueId = null) {
  const items = [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])]
    .filter(item => templateIndex(item) === TOWN_TELEPORT.itemIndex &&
      normalized(item?.name) === normalized(TOWN_TELEPORT.name) && heldQuantity(item) !== null && validUniqueId(item?.uniqueId));
  return uniqueId == null ? items[0] ?? null : items.find(item => Number(item.uniqueId) === Number(uniqueId)) ?? null;
}

function randomTeleportGrid(snapshot, item) {
  if ((snapshot?.beltItems ?? []).some(entry => Number(entry?.uniqueId) === Number(item?.uniqueId))) return 'belt';
  return 'inventory';
}

function townTeleportGrid(snapshot, item) {
  if ((snapshot?.beltItems ?? []).some(entry => Number(entry?.uniqueId) === Number(item?.uniqueId))) return 'belt';
  return 'inventory';
}

function playerPosition(snapshot) {
  const player = (snapshot?.entities ?? []).find(entry => String(entry?.objectId) === String(snapshot?.playerObjectId));
  return validPoint(player) ? { x: Number(player.x), y: Number(player.y) } : null;
}

function validPoint(value) {
  return Number.isFinite(Number(value?.x)) && Number.isFinite(Number(value?.y));
}

function purchaseChunks(snapshot, kind, requested) {
  const stackSize = SUPPLIES[kind].stackSize;
  let remaining = requested;
  const chunks = [];
  const items = [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? []), ...(snapshot?.equipmentItems ?? [])]
    .filter(item => supplyKind(item) === kind);
  for (const item of items) {
    const capacity = Math.max(0, stackSize - Math.max(0, Number(item?.quantity ?? 1)));
    if (capacity === 0) continue;
    const chunk = Math.min(remaining, capacity);
    if (chunk > 0) chunks.push(chunk);
    remaining -= chunk;
    if (remaining === 0) return chunks;
  }
  while (remaining > 0) {
    const chunk = Math.min(remaining, stackSize);
    chunks.push(chunk);
    remaining -= chunk;
  }
  return chunks;
}

function supplyKind(item) {
  return templateIndex(item) === SUPPLIES.hp.itemIndex || normalized(item?.name) === normalized(SUPPLIES.hp.name)
    ? 'hp'
    : templateIndex(item) === SUPPLIES.hpMedium.itemIndex || normalized(item?.name) === normalized(SUPPLIES.hpMedium.name)
      ? 'hpMedium'
    : templateIndex(item) === SUPPLIES.mp.itemIndex || normalized(item?.name) === normalized(SUPPLIES.mp.name)
      ? 'mp'
      : templateIndex(item) === SUPPLIES.amulet.itemIndex || normalized(item?.name) === normalized(SUPPLIES.amulet.name)
        ? 'amulet'
        : null;
}

export async function equipHeldAmulet(client) {
  if (!canUseSoulFireBall(client?.snapshot)) return null;
  const alreadyEquipped = (client.snapshot?.equipmentItems ?? []).find(item =>
    normalized(item?.slot ?? item?.equipSlot) === 'amulet' && isStandardAmulet(item) && heldQuantity(item) !== null);
  if (alreadyEquipped) return { uniqueId: alreadyEquipped.uniqueId, quantity: heldQuantity(alreadyEquipped) };
  let held = [...(client.snapshot?.inventoryItems ?? [])].find(item =>
    isStandardAmulet(item) && validUniqueId(item.uniqueId) && heldQuantity(item) !== null && amuletEquipEligible(client.snapshot, item));
  if (!held) {
    const beltHeld = (client.snapshot?.beltItems ?? []).find(item =>
      isStandardAmulet(item) && validUniqueId(item.uniqueId) && heldQuantity(item) !== null && amuletEquipEligible(client.snapshot, item));
    if (!beltHeld) return null;
    const beltQuantity = heldQuantity(beltHeld);
    const emptyBagSlot = firstEmptyBagSlot(client.snapshot);
    if (emptyBagSlot === null) return null;
    const from = boundedSlot(beltHeld.slot, 'Amulet belt slot');
    const to = BELT_SLOT_COUNT + emptyBagSlot;
    const moveAck = await client.request({ type: 'moveItem', grid: 'belt', from, to }, 'MoveItem');
    if (moveAck?.payload?.success !== true) throw new Error('Move from belt to inventory rejected for Amulet');
    await client.wait(
      () => {
        const inInventory = (client.snapshot?.inventoryItems ?? []).find(item =>
          Number(item?.uniqueId) === Number(beltHeld.uniqueId) && heldQuantity(item) === beltQuantity);
        const stillInBelt = (client.snapshot?.beltItems ?? []).some(item =>
          Number(item?.uniqueId) === Number(beltHeld.uniqueId));
        return inInventory && !stillInBelt ? inInventory : null;
      },
      'Amulet moved from belt to inventory',
      WAIT_MS,
    );
    held = (client.snapshot?.inventoryItems ?? []).find(item =>
      Number(item?.uniqueId) === Number(beltHeld.uniqueId));
    if (!held || !amuletEquipEligible(client.snapshot, held)) {
      throw new Error('Moved Amulet is not authoritative or eligible in inventory');
    }
  }
  if (!held) return null;
  const expectedQuantity = heldQuantity(held);
  const ack = await client.request({ type: 'equipItem', grid: 'inventory', uniqueId: held.uniqueId, to: 9 }, 'EquipItem');
  if (ack?.payload?.success !== true) throw new Error('Equip rejected for Amulet');
  await client.wait(
    () => (client.snapshot?.equipmentItems ?? []).some(item =>
      Number(item?.uniqueId) === Number(held.uniqueId)
        && normalized(item?.slot ?? item?.equipSlot) === 'amulet'
        && heldQuantity(item) === expectedQuantity),
    'equipped Amulet',
    WAIT_MS,
  );
  const equipped = (client.snapshot?.equipmentItems ?? []).find(item => Number(item?.uniqueId) === Number(held.uniqueId));
  return { uniqueId: equipped.uniqueId, quantity: expectedQuantity };
}

function firstEmptyBagSlot(snapshot) {
  const maxBagSlots = Number(snapshot?.maxBagSlots ?? Number(snapshot?.inventoryCapacity) - BELT_SLOT_COUNT);
  if (!Number.isSafeInteger(maxBagSlots) || maxBagSlots <= 0 || maxBagSlots > 80) return null;
  const occupied = new Set();
  for (const item of snapshot?.inventoryItems ?? []) {
    const slot = Number(item?.slot);
    if (!Number.isSafeInteger(slot) || slot < 0) continue;
    const container = normalized(item?.container);
    if (container === 'bag1') occupied.add(slot);
    else if (container === 'bag2') occupied.add(FIRST_BAG_PAGE_SLOTS + slot);
  }
  for (let slot = 0; slot < maxBagSlots; slot += 1) {
    if (!occupied.has(slot)) return slot;
  }
  return null;
}

function boundedSlot(value, label) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed >= BELT_SLOT_COUNT) {
    throw new Error(`${label} must be an authoritative belt slot`);
  }
  return parsed;
}

function isStandardAmulet(item) {
  return templateIndex(item) === SUPPLIES.amulet.itemIndex || normalized(item?.name) === normalized(SUPPLIES.amulet.name);
}

function amuletEquipEligible(snapshot, item) {
  const source = item?.tooltipSource ?? item?.tooltip_source;
  const info = source?.realInfo ?? source?.real_info ?? source?.info;
  if (!info) return false;
  const requiredClass = Number(info.requiredClass ?? info.required_class ?? 0);
  if (requiredClass !== 0 && (requiredClass & 4) === 0) return false;
  const requiredType = Number(info.requiredType ?? info.required_type ?? 0);
  const requiredAmount = Number(info.requiredAmount ?? info.required_amount ?? 0);
  if (requiredType !== 0) return false;
  const actor = (snapshot?.entities ?? []).find(entity => String(entity?.objectId) === String(snapshot?.playerObjectId));
  return Number(actor?.level ?? 0) >= requiredAmount;
}

function canUseSoulFireBall(snapshot) {
  if (playerClass(snapshot) !== 'taoist') return false;
  const actor = (snapshot?.entities ?? []).find(entity => String(entity?.objectId) === String(snapshot?.playerObjectId));
  const level = Number(actor?.level);
  if (!Number.isSafeInteger(level) || level < SOUL_FIRE_BALL_REQUIRED_LEVEL) return false;
  return (snapshot?.knownSkills ?? []).some(skill => skill?.spell === 'SoulFireBall');
}

function validUniqueId(value) {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 0;
}

function heldQuantity(item) {
  const quantity = Number(item?.quantity);
  return Number.isSafeInteger(quantity) && quantity > 0 ? quantity : null;
}

function templateIndex(item) {
  const source = item?.tooltipSource ?? item?.tooltip_source;
  const info = source?.realInfo ?? source?.real_info ?? source?.info;
  const value = item?.itemIndex ?? item?.item_index ?? info?.itemIndex ?? info?.item_index;
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function obsoleteMaterialsForSale(snapshot, { progressionCandidates = [], protectedItemNames = [] } = {}) {
  const protectedNames = new Set([
    ...progressionCandidates.map(candidate => candidate?.name),
    ...protectedItemNames,
    ...Object.values(SUPPLIES).map(supply => supply.name),
    RANDOM_TELEPORT.name,
    TOWN_TELEPORT.name,
  ].map(normalized).filter(Boolean));
  return (snapshot?.inventoryItems ?? []).filter(item => {
    const name = normalized(item?.name);
    const equipSlot = normalized(item?.equipSlot ?? item?.slotName);
    return name && !protectedNames.has(name) && !equipSlot &&
      validUniqueId(item?.uniqueId) && heldQuantity(item) != null &&
      Number.isSafeInteger(Number(item?.sellValue)) && Number(item.sellValue) > 0;
  });
}

function playerClass(snapshot) {
  const actor = (snapshot?.entities ?? []).find(entity => String(entity?.objectId) === String(snapshot?.playerObjectId));
  return normalized(actor?.class);
}

function playerLevel(snapshot) {
  const actor = (snapshot?.entities ?? []).find(entity =>
    String(entity?.objectId) === String(snapshot?.playerObjectId));
  const level = Number(actor?.level);
  return Number.isSafeInteger(level) && level >= 0 ? level : 0;
}

function hasHeldWeapon(snapshot) {
  return [...(snapshot?.inventoryItems ?? []), ...(snapshot?.equipmentItems ?? [])].some(item =>
    normalized(item?.slot ?? item?.equipSlot) === 'weapon' ||
    Number(item?.tooltipSource?.realInfo?.item_type ??
      item?.tooltipSource?.real_info?.item_type ??
      item?.tooltipSource?.info?.item_type ??
      item?.tooltipSource?.info?.itemType) === 1
  );
}

function warriorWeaponRequirement(snapshot) {
  if (playerClass(snapshot) !== 'warrior') return null;
  return classWeaponRequirement(snapshot);
}

function classWeaponRequirement(snapshot) {
  const className = playerClass(snapshot);
  const catalog = CLASS_WEAPONS[className];
  if (!catalog) return null;
  const level = playerLevel(snapshot);
  const preferredIndex = catalog.findIndex(weapon => level >= weapon.requiredLevel);
  if (preferredIndex < 0) return null;
  const heldWeapons = [...(snapshot?.inventoryItems ?? []), ...(snapshot?.equipmentItems ?? [])]
    .filter(item => normalized(item?.slot ?? item?.equipSlot) === 'weapon' ||
      Number(item?.tooltipSource?.realInfo?.item_type ??
        item?.tooltipSource?.real_info?.item_type ??
        item?.tooltipSource?.info?.item_type ??
        item?.tooltipSource?.info?.itemType) === 1);
  if (heldWeapons.length === 0) {
    return { preferred: catalog[preferredIndex], allowFallback: true, catalog };
  }

  // Known ordinary shop weapons follow the explicit tier order above. An
  // unknown quest/reward weapon is preserved because its progression tier
  // cannot safely be inferred from its display name or randomized attack.
  let bestOrdinaryIndex = catalog.length;
  for (const item of heldWeapons) {
    const ordinaryIndex = catalog.findIndex(weapon => weapon.itemIndex === templateIndex(item));
    if (ordinaryIndex < 0) return null;
    bestOrdinaryIndex = Math.min(bestOrdinaryIndex, ordinaryIndex);
  }
  return bestOrdinaryIndex > preferredIndex
    ? { preferred: catalog[preferredIndex], allowFallback: false, catalog }
    : null;
}

function taoistForestWeaponRequirement(snapshot) {
  if (playerClass(snapshot) !== 'taoist' || playerLevel(snapshot) < 11) return null;
  const heldWeapons = [...(snapshot?.inventoryItems ?? []), ...(snapshot?.equipmentItems ?? [])]
    .filter(item => normalized(item?.slot ?? item?.equipSlot) === 'weapon' ||
      Number(item?.tooltipSource?.realInfo?.item_type ??
        item?.tooltipSource?.real_info?.item_type ??
        item?.tooltipSource?.info?.item_type ??
        item?.tooltipSource?.info?.itemType) === 1);
  // This is a replacement for the documented WoodenSword-only q7 state, not
  // a broad inference that arbitrary quest or dropped weapons are inadequate.
  if (heldWeapons.some(item => templateIndex(item) !== 221)) return null;
  return {
    preferred: TAOIST_FOREST_WEAPONS[0],
    allowFallback: true,
    catalog: TAOIST_FOREST_WEAPONS,
  };
}

function taoistForestSupplyReady(snapshot) {
  const held = stock(snapshot);
  return held.hp >= TAOIST_FOREST_MINIMUM_STOCK.hp &&
    held.mp >= TAOIST_FOREST_MINIMUM_STOCK.mp &&
    randomTeleportStock(snapshot) >= TAOIST_FOREST_MINIMUM_STOCK.randomTeleport &&
    townTeleportStock(snapshot) >= TAOIST_FOREST_MINIMUM_STOCK.townTeleport;
}

function spendable(gold, reserveGold = GOLD_RESERVE) {
  return Math.max(0, gold - reserveGold);
}

function needsFunds(stockValue, gold, spendableGold, reserveGold = GOLD_RESERVE) {
  return { status: 'needsFunds', stock: stockValue, gold, spendableGold, reserveGold };
}

function positiveOption(value, fallback, label) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new TypeError(`${label} must be a positive integer`);
  return parsed;
}

function positiveOrZeroOption(value, fallback, label) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error(`${label} must be a nonnegative integer`);
  return parsed;
}

function nonnegativeOption(value, fallback, label) {
  if (value == null) return fallback;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new TypeError(`${label} must be a non-negative integer`);
  return parsed;
}

function emergencyTeleportOption(value) {
  if (value == null) return 0;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > 8) {
    throw new TypeError('emergencyTeleportCount must be an integer from 0 to 8');
  }
  return parsed;
}

function emergencyTownTeleportOption(value) {
  if (value == null) return 0;
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0 || parsed > TOWN_TELEPORT.stackSize) {
    throw new TypeError(`emergencyTownTeleportCount must be an integer from 0 to ${TOWN_TELEPORT.stackSize}`);
  }
  return parsed;
}

function normalized(value) {
  return String(value ?? '').replace(/[^a-z0-9]/gi, '').toLowerCase();
}

function integer(value, label) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error(`${label} must be a non-negative integer`);
  return parsed;
}

function positiveInteger(value, label) {
  const parsed = integer(value, label);
  if (parsed <= 0) throw new Error(`${label} must be positive`);
  return parsed;
}

function numericId(value, label) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error(`${label} must be a non-negative safe integer`);
  return parsed;
}

function shopRowId(row) {
  const value = row?.id ?? row?.uniqueId ?? row?.unique_id;
  if ((typeof value === 'number' && Number.isSafeInteger(value) && value >= 0) ||
      (typeof value === 'string' && /^\d+$/.test(value))) return value;
  throw new Error(`Merchant Ruben returned an invalid shop row id for ${row?.name ?? 'an item'}`);
}
