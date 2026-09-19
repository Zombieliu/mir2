import assert from 'node:assert/strict';
import test from 'node:test';
import {
  createRandomTeleportEmergencyEscape,
  equipHeldAmulet,
  randomTeleportCount,
  restockInVillage,
  townTeleportCount,
  useTownTeleport,
  useRandomTeleport,
  journeyWeaponFundingGold,
  purchaseV2BasicHpPotion,
  warriorWeaponFundingGold,
} from './protocol-supplies.mjs';

const HP = 658;
const HP_MEDIUM = 660;
const MP = 659;
const AMULET = 712;
const RANDOM_TELEPORT = 717;
const TOWN_TELEPORT = 719;

function item(itemIndex, quantity, name = itemIndex === HP ? '(HP)DrugSmall' : itemIndex === HP_MEDIUM ? '(HP)DrugMedium' : itemIndex === MP ? '(MP)DrugSmall' : 'Amulet', uniqueId = itemIndex) {
  return { name, uniqueId, quantity, container: 'bag1', tooltipSource: { info: {
    item_index: itemIndex, required_type: 0, required_amount: itemIndex === AMULET ? 18 : 0,
    required_class: itemIndex === AMULET ? 31 : 0,
  } } };
}

function snapshot({ className = 'Warrior', level = 20, gold = 1000, hp = 0, hpMedium = 0, mp = 0, amulet = 0, beltAmulet = 0, knownSkills = [], mapFileName = '0', npc = true } = {}) {
  return {
    mapFileName,
    gold,
    playerObjectId: 1,
    entities: [
      { objectId: 1, kind: 'player', class: className, level, x: 290, y: 608 },
      ...(npc ? [
        { objectId: 88, kind: 'npc', name: 'Merchant_Ruben', x: 288, y: 608 },
        { objectId: 42, kind: 'npc', name: 'Merchant_Scott', x: 291, y: 610 },
        { objectId: 5, kind: 'npc', name: 'Blacksmith_Smith', x: 296, y: 613 },
        { objectId: 6, kind: 'npc', name: 'Merchant_John', x: 292, y: 603 },
        { objectId: 43, kind: 'npc', name: 'MaterialDealer_Reece', x: 295, y: 605 },
      ] : []),
    ],
    inventoryItems: [
      ...(hp ? [item(HP, hp)] : []),
      ...(hpMedium ? [item(HP_MEDIUM, hpMedium)] : []),
      ...(mp ? [item(MP, mp)] : []),
      ...(amulet ? [item(AMULET, amulet)] : []),
    ],
    beltItems: beltAmulet ? [{ ...item(AMULET, beltAmulet, 'Amulet', 800712), container: 'belt', slot: 4 }] : [],
    equipmentItems: [],
    knownSkills,
    maxBagSlots: 40,
    inventoryCapacity: 46,
    activeNpcDialog: null,
  };
}

class FakeClient {
  constructor(state, options = {}) {
    this.snapshot = structuredClone(state);
    this.sequence = 0;
    this.events = [];
    this.sent = [];
    this.options = options;
  }
  send(command) {
    this.sent.push(structuredClone(command));
    if (command.type === 'clientVersion' && this.options.refreshNpc) {
      this.receive({ ...this.snapshot, entities: [...this.snapshot.entities, structuredClone(this.options.refreshNpc)] });
    }
    if (command.type === 'interact' && !this.options.noDialog) {
      this.receive({ ...this.snapshot, activeNpcDialog: {
        npcObjectId: command.objectId,
        links: this.options.links ?? ([6, 43].includes(Number(command.objectId))
          ? [{ text: 'Sell Materials', target: '@Sell' }]
          : Number(command.objectId) === 5
            ? [
                { text: 'Buy Weapon', target: '@Buy' },
                { text: 'Sell Weapon', target: '@Sell' },
              ]
            : [{ text: 'Buy and sell', target: '@BuySell' }]),
      } });
    }
    if (command.type === 'buyItem' && !this.options.rejectBuy) {
      const objectId = this.snapshot?.activeNpcDialog?.npcObjectId;
      const availableGoods = Object.hasOwn(this.options.goodsByNpc ?? {}, String(objectId))
        ? this.options.goodsByNpc[String(objectId)]
        : this.options.goods ?? goods();
      const row = availableGoods.find(entry => String(entry.id) === String(command.itemIndex));
      if (this.options.enforceMerchantOwnership && !row) {
        throw new Error(`merchant ${objectId} does not own buy row ${command.itemIndex}`);
      }
      const index = Number(row?.itemIndex);
      const quantity = Number(command.count);
      const next = structuredClone(this.snapshot);
      next.gold -= Number(row.price) * quantity + Number(this.options.wrongGoldDelta ?? 0);
      const existing = next.inventoryItems.find(entry => Number(entry.tooltipSource?.info?.item_index) === index);
      if (existing) existing.quantity += quantity + Number(this.options.wrongItemDelta ?? 0);
      else next.inventoryItems.push({
        ...item(index, quantity + Number(this.options.wrongItemDelta ?? 0), row.name),
        ...(row.equipSlot ? { equipSlot: row.equipSlot } : {}),
        tooltipSource: { info: {
          item_index: index,
          item_type: Number(row.itemType ?? 0),
          required_type: 0,
          required_amount: Number(row.requiredLevel ?? 0),
          required_class: Number(row.requiredClass ?? 0),
        } },
      });
      this.receive(next);
    }
  }
  async request(command, packet) {
    if (packet === 'SellItem') {
      this.sent.push(structuredClone(command));
      if (this.options.rejectSell) return { packet, payload: { success: false } };
      const next = structuredClone(this.snapshot);
      const index = next.inventoryItems.findIndex(entry =>
        Number(entry.uniqueId) === Number(command.uniqueId));
      const sold = next.inventoryItems[index];
      if (index >= 0) next.inventoryItems.splice(index, 1);
      next.gold += Number(sold?.sellValue ?? 0) * Number(command.count);
      this.receive(next);
      return this.packet('SellItem', {
        uniqueId: command.uniqueId,
        count: command.count,
        success: true,
      });
    }
    if (packet === 'MoveItem') {
      this.sent.push(structuredClone(command));
      if (this.options.rejectMove) return { packet, payload: { success: false } };
      const next = structuredClone(this.snapshot);
      const index = next.beltItems.findIndex(entry => Number(entry.slot) === Number(command.from));
      if (index >= 0 && !this.options.noMoveSnapshot) {
        const [moved] = next.beltItems.splice(index, 1);
        const logicalSlot = Number(command.to) - 6;
        next.inventoryItems.push({
          ...moved,
          container: logicalSlot < 40 ? 'bag1' : 'bag2',
          slot: logicalSlot < 40 ? logicalSlot : logicalSlot - 40,
        });
        this.receive(next);
      }
      return { packet, payload: { success: true } };
    }
    if (packet === 'EquipItem') {
      this.sent.push(structuredClone(command));
      if (this.options.rejectEquip) return { packet, payload: { success: false } };
      const next = structuredClone(this.snapshot);
      const index = next.inventoryItems.findIndex(entry => Number(entry.uniqueId) === Number(command.uniqueId));
      if (index >= 0) {
        const [equipped] = next.inventoryItems.splice(index, 1);
        const slot = Number(command.to) === 0 ? 'weapon' : 'amulet';
        next.equipmentItems = [
          ...(next.equipmentItems ?? []).filter(entry => entry.slot !== slot),
          { ...equipped, slot },
        ];
        this.receive(next);
      }
      return { packet, payload: { success: true } };
    }
    if (packet === 'UseItem') {
      this.sent.push(structuredClone(command));
      if (this.options.rejectUse || this.options.falseAckNoConsume) return { packet, payload: { success: false } };
      const next = structuredClone(this.snapshot);
      const items = command.grid === 'belt' ? next.beltItems : command.grid === 'equipment' ? next.equipmentItems : next.inventoryItems;
      const index = items.findIndex(entry => Number(entry.uniqueId) === Number(command.uniqueId));
      if (index < 0) return { packet, payload: { success: false } };
      const itemIndex = Number(items[index]?.tooltipSource?.info?.item_index);
      if (Number(items[index].quantity ?? 1) > 1) items[index].quantity -= 1;
      else items.splice(index, 1);
      const actor = next.entities.find(entry => Number(entry.objectId) === Number(next.playerObjectId));
      if (itemIndex === TOWN_TELEPORT && this.options.townTeleportMap != null) {
        next.mapFileName = String(this.options.townTeleportMap);
      }
      if (!(itemIndex === TOWN_TELEPORT && this.options.townTeleportSamePosition === true)) {
        actor.x += 7;
        actor.y += 3;
      }
      this.receive(next);
      return { packet, payload: { success: true } };
    }
    const after = this.sequence;
    this.send(command);
    if (packet === 'NPCGoods' && !this.options.noGoods) {
      const objectId = this.snapshot?.activeNpcDialog?.npcObjectId;
      this.packet('NPCGoods', {
        list: Object.hasOwn(this.options.goodsByNpc ?? {}, String(objectId))
          ? this.options.goodsByNpc[String(objectId)]
          : this.options.goods ?? goods(),
        panelType: this.options.panelType ?? 0,
        rate: 1,
      });
    }
    if (packet === 'NPCSell') this.packet('NPCSell', {});
    return this.wait(() => this.events.find(event => event.sequence > after && event.packet === packet), packet);
  }
  receive(next) {
    this.snapshot = next;
    this.events.push({ sequence: ++this.sequence, direction: 'received', type: 'worldSnapshot', payload: structuredClone(next) });
  }
  packet(packet, payload) {
    const event = { sequence: ++this.sequence, direction: 'received', packet, payload };
    this.events.push(event);
    return event;
  }
  async wait(predicate, label) {
    const value = predicate();
    if (!value) throw new Error(`Timeout waiting for ${label}`);
    return value;
  }
}

function goods() {
  return [
    { id: '9007199254740993', uniqueId: '9007199254740993', itemIndex: HP, name: '(HP)DrugSmall', price: 40, count: 1 },
    { id: 66001, uniqueId: 66001, itemIndex: HP_MEDIUM, name: '(HP)DrugMedium', price: 110, count: 1 },
    { id: 72, uniqueId: 72, itemIndex: MP, name: '(MP)DrugSmall', price: 40, count: 1 },
    { id: 73, uniqueId: 73, itemIndex: AMULET, name: 'Amulet', price: 25, count: 1 },
  ];
}

function emergencyGoods() {
  return [...goods(), { id: 71701, uniqueId: 71701, itemIndex: RANDOM_TELEPORT, name: 'RandomTeleport', price: 100, count: 1 }];
}

function townEmergencyGoods() {
  return [...emergencyGoods(), { id: 71901, uniqueId: 71901, itemIndex: TOWN_TELEPORT, name: 'TownTeleport', price: 1000, count: 1 }];
}

function weaponGoods() {
  return [
    { id: 25401, uniqueId: 25401, itemIndex: 254, name: 'Trident', price: 4000,
      count: 1, equipSlot: 'weapon', itemType: 1, requiredLevel: 15, requiredClass: 7 },
    { id: 26801, uniqueId: 26801, itemIndex: 268, name: 'Scimitar', price: 4000,
      count: 1, equipSlot: 'weapon', itemType: 1, requiredLevel: 15, requiredClass: 7 },
    { id: 22701, uniqueId: 22701, itemIndex: 227, name: 'BronzeAxe', price: 1500,
      count: 1, equipSlot: 'weapon', itemType: 1, requiredLevel: 13, requiredClass: 7 },
    { id: 22401, uniqueId: 22401, itemIndex: 224, name: 'BronzeSword', price: 900,
      count: 1, equipSlot: 'weapon', itemType: 1, requiredLevel: 5, requiredClass: 7 },
    { id: 22601, uniqueId: 22601, itemIndex: 226, name: 'IronSword', price: 1100,
      count: 1, equipSlot: 'weapon', itemType: 1, requiredLevel: 10, requiredClass: 7 },
    { id: 22101, uniqueId: 22101, itemIndex: 221, name: 'WoodenSword', price: 50,
      count: 1, equipSlot: 'weapon', itemType: 1, requiredLevel: 1, requiredClass: 7 },
  ];
}

function taoistFreshMaterialTopUpState(gold, { includeVenison = true } = {}) {
  const state = snapshot({
    className: 'Taoist', level: 18, gold, hp: 80, mp: 11, amulet: 99,
    knownSkills: [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }],
  });
  state.inventoryItems.push(item(TOWN_TELEPORT, 1, 'TownTeleport', 719100));
  if (includeVenison) {
    state.inventoryItems.push(
      { name: 'Venison', uniqueId: 719101, quantity: 1, container: 'bag1', sellValue: 249 },
      { name: 'Venison', uniqueId: 719102, quantity: 1, container: 'bag1', sellValue: 226 },
    );
  }
  return state;
}

function freshRubenNavigation(client, { unknownGold = false } = {}) {
  let consumed = false;
  return async target => {
    if (consumed || Number(target.x) !== 288 || Number(target.y) !== 608) return;
    consumed = true;
    const next = structuredClone(client.snapshot);
    next.inventoryItems.find(entry => Number(entry.tooltipSource?.info?.item_index) === AMULET).quantity -= 10;
    if (unknownGold) next.gold = 'unknown';
    client.receive(next);
  };
}

const freshMaterialTopUpOptions = Object.freeze({
  emergencyTownTeleportCount: 2,
  targetHp: 80,
  targetMp: 12,
  targetAmulet: 100,
  lowStockMp: 4,
  lowStockAmulet: 100,
  liquidateObsoleteMaterials: true,
  reserveGold: 0,
});

test('reports preferred weapon funding for an unarmed or under-geared Warrior', () => {
  const unarmed = snapshot({ className: 'Warrior', level: 18, hp: 6 });
  assert.equal(warriorWeaponFundingGold(unarmed), 1500);
  unarmed.equipmentItems.push({ ...item(222, 1, 'Dagger', 991), slot: 'weapon' });
  assert.equal(warriorWeaponFundingGold(unarmed), 1500);
  unarmed.equipmentItems = [{ ...item(227, 1, 'BronzeAxe', 992), slot: 'weapon' }];
  assert.equal(warriorWeaponFundingGold(unarmed), 0);
  assert.equal(warriorWeaponFundingGold(snapshot({ className: 'Wizard', level: 18, hp: 6 })), 0);
});

test('reports and buys the live class weapon when a caster lost every weapon', async () => {
  assert.equal(journeyWeaponFundingGold(snapshot({ className: 'Wizard', level: 23 })), 4000);
  assert.equal(journeyWeaponFundingGold(snapshot({ className: 'Taoist', level: 23 })), 4000);
  const state = snapshot({ className: 'Taoist', level: 23, gold: 5000, hp: 6, mp: 6 });
  const client = new FakeClient(state, { goods: weaponGoods() });
  const result = await restockInVillage(client, async () => {}, {
    ensureClassWeapon: true,
    reserveGold: 0,
  });
  assert.equal(result.status, 'restocked');
  assert.equal(result.weaponPurchase.name, 'Scimitar');
  assert.ok(client.snapshot.equipmentItems.some(entry =>
    entry.name === 'Scimitar' && entry.slot === 'weapon'));
  assert.equal(journeyWeaponFundingGold(client.snapshot), 0);
});

test('under-geared Warrior upgrades Dagger to the preferred BronzeAxe without a lower-tier fallback', async () => {
  const state = snapshot({ className: 'Warrior', level: 18, gold: 2000, hp: 6 });
  state.equipmentItems.push({ ...item(222, 1, 'Dagger', 991), slot: 'weapon' });
  const client = new FakeClient(state, { goods: weaponGoods() });
  const result = await restockInVillage(client, async () => {}, {
    ensureWarriorWeapon: true,
    reserveGold: 0,
  });
  assert.equal(result.status, 'restocked');
  assert.equal(result.weaponPurchase.name, 'BronzeAxe');
  assert.ok(client.snapshot.equipmentItems.some(entry =>
    entry.name === 'BronzeAxe' && entry.slot === 'weapon'));

  const poorState = snapshot({ className: 'Warrior', level: 18, gold: 1000, hp: 6 });
  poorState.equipmentItems.push({ ...item(222, 1, 'Dagger', 993), slot: 'weapon' });
  const poor = new FakeClient(poorState, { goods: weaponGoods() });
  const shortfall = await restockInVillage(poor, async () => {}, {
    ensureWarriorWeapon: true,
    reserveGold: 0,
  });
  assert.equal(shortfall.status, 'needsFunds');
  assert.equal(poor.sent.length, 0);
});

test('unarmed Warrior buys the best affordable live Blacksmith weapon with exact proof', async () => {
  const client = new FakeClient(
    snapshot({ className: 'Warrior', level: 18, gold: 2000, hp: 6 }),
    { goods: weaponGoods() },
  );
  const navigations = [];
  const result = await restockInVillage(client, async (...args) => navigations.push(args), {
    ensureWarriorWeapon: true,
    reserveGold: 0,
  });

  assert.equal(result.status, 'restocked');
  assert.equal(result.weaponPurchase.name, 'BronzeAxe');
  assert.equal(result.weaponPurchase.shopItemId, 22701);
  assert.equal(result.weaponPurchase.equipped, true);
  assert.equal(result.after.gold, 500);
  assert.ok(client.snapshot.equipmentItems.some(entry =>
    entry.name === 'BronzeAxe' && entry.slot === 'weapon'));
  assert.deepEqual(navigations, [[{ x: 296, y: 613 }, 1]]);
  assert.deepEqual(client.sent.at(-2), {
    type: 'buyItem', itemIndex: 22701, count: 1, panelType: 0,
  });
  assert.deepEqual(client.sent.at(-1), {
    type: 'equipItem', grid: 'inventory', uniqueId: 227, to: 0,
  });
});

test('unarmed Warrior falls back to the strongest affordable exact catalog row', async () => {
  const client = new FakeClient(
    snapshot({ className: 'Warrior', level: 18, gold: 1000, hp: 6 }),
    { goods: weaponGoods() },
  );
  const result = await restockInVillage(client, async () => {}, {
    ensureWarriorWeapon: true,
    reserveGold: 0,
  });
  assert.equal(result.weaponPurchase.name, 'BronzeSword');
  assert.equal(result.after.gold, 100);
});

test('weapon purchase proves the live Blacksmith rate-adjusted price', async () => {
  const adjusted = weaponGoods().map(row =>
    row.name === 'BronzeAxe' ? { ...row, price: 1800 } : row);
  const client = new FakeClient(
    snapshot({ className: 'Warrior', level: 18, gold: 2000, hp: 6 }),
    { goods: adjusted },
  );
  const result = await restockInVillage(client, async () => {}, {
    ensureWarriorWeapon: true,
    reserveGold: 0,
  });
  assert.equal(result.weaponPurchase.name, 'BronzeAxe');
  assert.equal(result.weaponPurchase.unitPrice, 1800);
  assert.equal(result.after.gold, 200);
});

function taoistForestState({ gold = 960, hp = 7, mp = 20, randomTeleport = 2, townTeleport = 1 } = {}) {
  const state = snapshot({ className: 'Taoist', level: 11, gold, hp, mp });
  state.inventoryItems.push(
    ...Array.from({ length: randomTeleport }, (_, index) => item(RANDOM_TELEPORT, 1, 'RandomTeleport', 71700 + index)),
    ...Array.from({ length: townTeleport }, (_, index) => item(TOWN_TELEPORT, 1, 'TownTeleport', 71900 + index)),
  );
  state.equipmentItems.push({ ...item(221, 1, 'WoodenSword', 22100), slot: 'weapon', tooltipSource: { info: {
    item_index: 221, item_type: 1, required_type: 0, required_amount: 1, required_class: 7,
  } } });
  return state;
}

test('q7 Taoist uses the live BronzeSword fallback at 960 only after its held survival floor is proven', async () => {
  const client = new FakeClient(taoistForestState(), { goods: weaponGoods(), enforceMerchantOwnership: true });
  const result = await restockInVillage(client, async () => {}, { ensureTaoistForestWeapon: true });
  assert.equal(result.status, 'restocked');
  assert.equal(result.weaponPurchase.name, 'BronzeSword');
  assert.equal(result.after.gold, 60);
  assert.deepEqual(client.sent.filter(command => command.type === 'buyItem'), [
    { type: 'buyItem', itemIndex: 22401, count: 1, panelType: 0 },
  ]);
  assert.ok(client.snapshot.equipmentItems.some(entry => entry.name === 'BronzeSword' && entry.slot === 'weapon'));
});

test('q7 Taoist prefers the live IronSword when its ordinary account gold covers it', async () => {
  const client = new FakeClient(taoistForestState({ gold: 1200 }), { goods: weaponGoods() });
  const result = await restockInVillage(client, async () => {}, { ensureTaoistForestWeapon: true });
  assert.equal(result.status, 'restocked');
  assert.equal(result.weaponPurchase.name, 'IronSword');
  assert.equal(result.after.gold, 100);
});

test('q7 Taoist forest upgrade fails before shopping when held survival supplies are insufficient', async () => {
  const client = new FakeClient(taoistForestState({ hp: 5 }), { goods: weaponGoods() });
  const result = await restockInVillage(client, async () => {}, { ensureTaoistForestWeapon: true });
  assert.equal(result.status, 'needsMandatorySupplies');
  assert.equal(client.sent.length, 0);
});

test('q7 Taoist rechecks held survival supplies after Blacksmith travel before spending the reserve', async () => {
  const client = new FakeClient(taoistForestState({ hp: 9 }), { goods: weaponGoods() });
  const result = await restockInVillage(client, async () => {
    const next = structuredClone(client.snapshot);
    next.inventoryItems.find(entry => Number(entry.tooltipSource?.info?.item_index) === HP).quantity = 5;
    client.receive(next);
  }, { ensureTaoistForestWeapon: true });
  assert.equal(result.status, 'needsMandatorySupplies');
  assert.equal(client.sent.some(command => command.type === 'buyItem'), false);
  assert.equal(client.snapshot.gold, 960);
});

test('q7 Taoist forest upgrade still requires a fresh exact purchase receipt', async () => {
  const client = new FakeClient(taoistForestState(), { goods: weaponGoods(), wrongGoldDelta: 1 });
  await assert.rejects(
    restockInVillage(client, async () => {}, { ensureTaoistForestWeapon: true }),
    /Timeout waiting for authoritative purchase of BronzeSword/,
  );
  assert.equal(client.snapshot.equipmentItems.some(entry => entry.name === 'BronzeSword'), false);
});

test('ordinary class weapon restocking retains its reserve and does not use the q7 exception', async () => {
  const client = new FakeClient(taoistForestState(), { goods: weaponGoods() });
  const result = await restockInVillage(client, async () => {}, { ensureClassWeapon: true });
  assert.equal(result.status, 'needsFunds');
  assert.equal(result.reserveGold, 100);
  assert.equal(client.sent.length, 0);
});

test('q7 Taoist leaves an existing non-Wooden weapon in place and never buys a weaker fallback', async () => {
  const state = taoistForestState();
  state.equipmentItems = [{ ...item(226, 1, 'IronSword', 22600), slot: 'weapon', tooltipSource: { info: {
    item_index: 226, item_type: 1, required_type: 0, required_amount: 10, required_class: 7,
  } } }];
  const client = new FakeClient(state, { goods: weaponGoods() });
  const result = await restockInVillage(client, async () => {}, { ensureTaoistForestWeapon: true });
  assert.equal(result.status, 'sufficient');
  assert.equal(client.sent.length, 0);
});

test('recomputes caster stock after service travel consumes the departure-floor MP bottle', async () => {
  const state = snapshot({ className: 'Taoist', level: 14, gold: 941, hp: 5, mp: 4 });
  const client = new FakeClient(state, { goods: goods() });
  let navigations = 0;
  const result = await restockInVillage(client, async () => {
    navigations += 1;
    if (navigations === 1) {
      const next = structuredClone(client.snapshot);
      next.inventoryItems.find(entry => Number(entry.tooltipSource?.info?.item_index) === MP).quantity = 3;
      client.receive(next);
    }
  }, {
    targetHp: 24,
    targetMp: 12,
    lowStockHp: 12,
    lowStockMp: 4,
    reserveGold: 0,
  });

  assert.equal(result.status, 'restocked');
  assert.equal(result.after.hp, 24);
  assert.ok(result.after.mp >= 4);
  assert.ok(result.purchases.some(entry => entry.itemIndex === MP));
});

test('does nothing outside map 0 or when relevant stock is sufficient', async () => {
  let navigations = 0;
  const outside = new FakeClient(snapshot({ mapFileName: '1', hp: 0 }));
  assert.deepEqual(await restockInVillage(outside, async () => navigations++), { status: 'notInVillage', mapFileName: '1' });
  const stocked = new FakeClient(snapshot({ className: 'Wizard', hp: 3, mp: 4 }));
  assert.equal((await restockInVillage(stocked, async () => navigations++)).status, 'sufficient');
  assert.equal(navigations, 0);
  assert.deepEqual([...outside.sent, ...stocked.sent], []);
});

test('ordinary restock does not buy RandomTeleport unless explicitly requested', async () => {
  const client = new FakeClient(snapshot({ gold: 1000, hp: 3 }), { goods: emergencyGoods() });
  const result = await restockInVillage(client, async () => {});
  assert.equal(result.status, 'sufficient');
  assert.equal(client.sent.some(entry => entry.type === 'buyItem' && entry.itemIndex === 71701), false);
  assert.equal(client.snapshot.inventoryItems.some(entry => Number(entry.tooltipSource?.info?.item_index) === RANDOM_TELEPORT), false);
});

test('Wizard-only HP Medium opt-in buys the live Ruben row to target six', async () => {
  const client = new FakeClient(snapshot({ className: 'Wizard', gold: 800, hp: 6, hpMedium: 0, mp: 1 }), { goods: goods() });
  const result = await restockInVillage(client, async () => {}, {
    targetHp: 6,
    lowStockHp: 6,
    targetHpMedium: 6,
    lowStockHpMedium: 3,
    targetMp: 1,
    lowStockMp: 1,
    reserveGold: 100,
  });
  assert.equal(result.status, 'restocked');
  assert.equal(result.after.gold, 140);
  assert.equal(client.snapshot.inventoryItems.find(entry => entry.name === '(HP)DrugMedium')?.quantity, 6);
  assert.deepEqual(client.sent.filter(command => command.type === 'buyItem').map(command => command.itemIndex), [66001]);
  assert.equal(result.purchases[0].itemIndex, HP_MEDIUM);
  assert.equal(result.purchases[0].unitPrice, 110);
});

test('HP Medium opt-in skips safely when the fresh Ruben row is missing or unaffordable', async () => {
  const missingRow = new FakeClient(snapshot({ className: 'Wizard', gold: 500, hp: 6, mp: 1 }), {
    goods: goods().filter(row => row.itemIndex !== HP_MEDIUM),
  });
  const missingResult = await restockInVillage(missingRow, async () => {}, {
    targetHp: 6, lowStockHp: 6, targetHpMedium: 6, lowStockHpMedium: 3,
    targetMp: 1, lowStockMp: 1, reserveGold: 100,
  });
  assert.equal(missingResult.status, 'restocked');
  assert.equal(missingRow.sent.some(command => command.itemIndex === 66001), false);

  const poor = new FakeClient(snapshot({ className: 'Wizard', gold: 150, hp: 6, mp: 1 }), { goods: goods() });
  const poorResult = await restockInVillage(poor, async () => {}, {
    targetHp: 6, lowStockHp: 6, targetHpMedium: 6, lowStockHpMedium: 3,
    targetMp: 1, lowStockMp: 1, reserveGold: 100,
  });
  assert.equal(poorResult.status, 'restocked');
  assert.equal(poor.sent.some(command => command.itemIndex === 66001), false);
});

test('ordinary restock does not buy TownTeleport unless explicitly requested', async () => {
  const client = new FakeClient(snapshot({ gold: 1100, hp: 6 }), {
    goodsByNpc: { '8': emergencyGoods(), '42': townEmergencyGoods() },
  });
  const result = await restockInVillage(client, async () => {});
  assert.equal(result.status, 'sufficient');
  assert.equal(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 42), false);
  assert.equal(client.sent.some(entry => entry.type === 'buyItem' && entry.itemIndex === 71901), false);
  assert.equal(townTeleportCount(client.snapshot), 0);
});

test('opt-in TownTeleport reserve uses Scott goods when Ruben lacks TownTeleport', async () => {
  const client = new FakeClient(snapshot({ gold: 1100, hp: 6 }), {
    goodsByNpc: { '8': emergencyGoods(), '42': townEmergencyGoods() },
  });
  const result = await restockInVillage(client, async () => {}, {
    emergencyTownTeleportCount: 1,
    reserveGold: 0,
  });
  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.purchases, [{
    itemIndex: TOWN_TELEPORT,
    name: 'TownTeleport',
    shopItemId: 71901,
    quantity: 1,
    unitPrice: 1000,
    cost: 1000,
  }]);
  assert.equal(result.after.gold, 100);
  assert.equal(townTeleportCount(client.snapshot), 1);
  assert.equal(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 42), true);
  assert.equal(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 88), false);
  assert.deepEqual(client.sent.at(-1), { type: 'buyItem', itemIndex: 71901, count: 1, panelType: 0 });
});

test('TownTeleport restock refreshes a missing live Scott before opening his real goods', async () => {
  const state = snapshot({ gold: 1100, hp: 6 });
  state.entities = state.entities.filter(entity => entity.objectId !== 42);
  const client = new FakeClient(state, {
    refreshNpc: { objectId: 42, kind: 'npc', name: 'Merchant_Scott', x: 291, y: 610 },
    goodsByNpc: { '42': townEmergencyGoods() },
  });
  const result = await restockInVillage(client, async () => {}, {
    emergencyTownTeleportCount: 1,
    reserveGold: 0,
  });
  assert.equal(result.status, 'restocked');
  assert.equal(townTeleportCount(client.snapshot), 1);
  const refreshIndex = client.sent.findIndex(command => command.type === 'clientVersion');
  const interactIndex = client.sent.findIndex(command => command.type === 'interact' && command.objectId === 42);
  assert.ok(refreshIndex >= 0 && interactIndex > refreshIndex);
});

test('Scott travel refreshes a Taoist Amulet floor from one fresh Ruben receipt', async () => {
  const knownSkills = [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }];
  const state = snapshot({ className: 'Taoist', level: 18, gold: 1164, hp: 3, mp: 8, amulet: 100, knownSkills });
  state.inventoryItems.push(item(TOWN_TELEPORT, 1, 'TownTeleport', 719000));
  state.entities.push(
    { objectId: 200, kind: 'monster', name: 'Scarecrow', disposition: 'hostile', x: 291, y: 609, hp: 4, dead: false },
    { objectId: 201, kind: 'monster', name: 'Scarecrow', disposition: 'hostile', x: 292, y: 610, hp: 4, dead: false },
  );
  const client = new FakeClient(state, {
    goodsByNpc: { '42': townEmergencyGoods(), '88': goods() },
  });
  let ScottAttempts = 0;
  const cleared = [];
  const result = await restockInVillage(client, async target => {
    if (Number(target.x) === 291 && Number(target.y) === 610 && ScottAttempts++ < 2) {
      throw new Error('No walk path on 0 to Merchant Scott');
    }
  }, {
    emergencyTownTeleportCount: 2,
    targetHp: 3,
    targetMp: 8,
    targetAmulet: 100,
    lowStockAmulet: 100,
    reserveGold: 0,
    clearBlockingMonster: async (_owner, blocker) => {
      cleared.push(Number(blocker.objectId));
      const next = structuredClone(client.snapshot);
      const amulet = next.inventoryItems.find(entry => Number(entry.tooltipSource?.info?.item_index) === AMULET);
      amulet.quantity -= 2;
      Object.assign(next.entities.find(entry => Number(entry.objectId) === Number(blocker.objectId)), { hp: 0, dead: true });
      client.receive(next);
    },
  });

  assert.equal(result.status, 'restocked');
  assert.deepEqual(cleared, [200, 201]);
  assert.deepEqual(result.purchases.map(entry => [entry.itemIndex, entry.quantity]), [[TOWN_TELEPORT, 1], [AMULET, 4]]);
  assert.deepEqual(client.sent.filter(entry => entry.type === 'buyItem').map(entry => [entry.itemIndex, entry.count]), [[71901, 1], [73, 4]]);
  assert.equal(result.after.amulet, 100);
  assert.equal(townTeleportCount(client.snapshot), 2);
  assert.equal(result.after.gold, 64);
});

test('post-Scott Amulet refresh fails closed from live gold without spending the reserve', async () => {
  const knownSkills = [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }];
  const state = snapshot({ className: 'Taoist', level: 18, gold: 1000, hp: 3, mp: 8, amulet: 100, knownSkills });
  state.inventoryItems.push(item(TOWN_TELEPORT, 1, 'TownTeleport', 719001));
  state.entities.push(
    { objectId: 210, kind: 'monster', name: 'Scarecrow', disposition: 'hostile', x: 291, y: 609, hp: 4, dead: false },
    { objectId: 211, kind: 'monster', name: 'Scarecrow', disposition: 'hostile', x: 292, y: 610, hp: 4, dead: false },
  );
  const client = new FakeClient(state, {
    goodsByNpc: { '42': townEmergencyGoods(), '88': goods() },
  });
  let ScottAttempts = 0;
  const result = await restockInVillage(client, async target => {
    if (Number(target.x) === 291 && Number(target.y) === 610 && ScottAttempts++ < 2) {
      throw new Error('No walk path on 0 to Merchant Scott');
    }
  }, {
    emergencyTownTeleportCount: 2,
    targetHp: 3,
    targetMp: 8,
    targetAmulet: 100,
    lowStockAmulet: 100,
    reserveGold: 0,
    clearBlockingMonster: async (_owner, blocker) => {
      const next = structuredClone(client.snapshot);
      next.inventoryItems.find(entry => Number(entry.tooltipSource?.info?.item_index) === AMULET).quantity -= 2;
      Object.assign(next.entities.find(entry => Number(entry.objectId) === Number(blocker.objectId)), { hp: 0, dead: true });
      client.receive(next);
    },
  });

  assert.equal(result.status, 'needsFunds');
  assert.equal(result.gold, 0);
  assert.equal(result.spendableGold, 0);
  assert.equal(result.reserveGold, 0);
  assert.equal(result.stock.amulet, 96);
  assert.deepEqual(result.purchases.map(entry => entry.itemIndex), [TOWN_TELEPORT]);
  assert.deepEqual(client.sent.filter(entry => entry.type === 'buyItem').map(entry => entry.itemIndex), [71901]);
});

test('Scott TownTeleport failures and unknown live gold do not fall through to Ruben', async () => {
  const knownSkills = [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }];
  const rejected = new FakeClient(
    snapshot({ className: 'Taoist', level: 18, gold: 1100, hp: 3, mp: 8, amulet: 100, knownSkills }),
    { goodsByNpc: { '42': townEmergencyGoods(), '88': goods() }, rejectBuy: true },
  );
  await assert.rejects(() => restockInVillage(rejected, async () => {}, {
    emergencyTownTeleportCount: 1, targetHp: 3, targetMp: 8, targetAmulet: 100, lowStockAmulet: 100, reserveGold: 0,
  }), /authoritative purchase of TownTeleport/);
  assert.equal(rejected.sent.some(entry => entry.type === 'interact' && entry.objectId === 88), false);

  const unknown = new FakeClient(
    snapshot({ className: 'Taoist', level: 18, gold: 1100, hp: 3, mp: 8, amulet: 100, knownSkills }),
    { goodsByNpc: { '42': townEmergencyGoods(), '88': goods() } },
  );
  await assert.rejects(() => restockInVillage(unknown, async target => {
    if (Number(target.x) === 291 && Number(target.y) === 610) {
      const next = structuredClone(unknown.snapshot);
      next.gold = 'unknown';
      unknown.receive(next);
    }
  }, {
    emergencyTownTeleportCount: 1, targetHp: 3, targetMp: 8, targetAmulet: 100, lowStockAmulet: 100, reserveGold: 0,
  }), /snapshot\.gold/);
  assert.equal(unknown.sent.some(entry => entry.type === 'buyItem'), false);
  assert.equal(unknown.sent.some(entry => entry.type === 'interact' && entry.objectId === 88), false);
});

test('fresh Ruben funding sells one live Venison before Amulet and TownTeleport purchases', async () => {
  const client = new FakeClient(taoistFreshMaterialTopUpState(1201), {
    goodsByNpc: { '6': [], '43': [], '88': goods(), '42': townEmergencyGoods() },
    enforceMerchantOwnership: true,
  });
  const result = await restockInVillage(client, freshRubenNavigation(client), freshMaterialTopUpOptions);

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.sales, [{ uniqueId: 719101, name: 'Venison', quantity: 1, gold: 249 }]);
  assert.deepEqual(result.purchases.map(entry => [entry.itemIndex, entry.quantity]), [[AMULET, 11], [TOWN_TELEPORT, 1]]);
  assert.deepEqual(client.sent.filter(entry => entry.type === 'sellItem').map(entry => entry.uniqueId), [719101]);
  assert.deepEqual(client.sent.filter(entry => entry.type === 'buyItem').map(entry => [entry.itemIndex, entry.count]), [[73, 11], [71901, 1]]);
  assert.deepEqual(client.sent.filter(entry => entry.type === 'interact').map(entry => entry.objectId), [88, 6, 88, 42]);
  assert.equal(result.after.gold, 175);
  assert.equal(townTeleportCount(client.snapshot), 2);
  assert.equal(client.snapshot.inventoryItems.some(entry => Number(entry.uniqueId) === 719102), true);
});

test('fresh Ruben funding skips the material top-up when the live budget already covers both purchases', async () => {
  const client = new FakeClient(taoistFreshMaterialTopUpState(1275), {
    goodsByNpc: { '6': [], '43': [], '88': goods(), '42': townEmergencyGoods() },
    enforceMerchantOwnership: true,
  });
  const result = await restockInVillage(client, freshRubenNavigation(client), freshMaterialTopUpOptions);

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.sales, []);
  assert.equal(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 6), false);
  assert.equal(result.after.gold, 0);
  assert.equal(townTeleportCount(client.snapshot), 2);
});

test('fresh Ruben funding preserves the reserve and fails closed when no eligible material can top up', async () => {
  const reserve = new FakeClient(taoistFreshMaterialTopUpState(1301), {
    goodsByNpc: { '6': [], '43': [], '88': goods(), '42': townEmergencyGoods() },
    enforceMerchantOwnership: true,
  });
  const reserveResult = await restockInVillage(reserve, freshRubenNavigation(reserve), {
    ...freshMaterialTopUpOptions,
    reserveGold: 100,
  });
  assert.equal(reserveResult.status, 'restocked');
  assert.equal(reserveResult.after.gold, 275);
  assert.ok(reserveResult.after.gold >= 100);
  assert.deepEqual(reserveResult.sales.map(entry => entry.uniqueId), [719101]);

  const noEligible = new FakeClient(taoistFreshMaterialTopUpState(1201, { includeVenison: false }), {
    goodsByNpc: { '88': goods(), '42': townEmergencyGoods() },
  });
  const shortfall = await restockInVillage(noEligible, freshRubenNavigation(noEligible), freshMaterialTopUpOptions);
  assert.equal(shortfall.status, 'needsFunds');
  assert.deepEqual(shortfall.sales, []);
  assert.equal(noEligible.sent.some(entry => entry.type === 'sellItem'), false);
  assert.deepEqual(noEligible.sent.filter(entry => entry.type === 'buyItem').map(entry => entry.itemIndex), [73]);
});

test('fresh Ruben funding rejects a sale and ignores a recycled non-material item id', async () => {
  const rejected = new FakeClient(taoistFreshMaterialTopUpState(1201), {
    goodsByNpc: { '6': [], '43': [], '88': goods(), '42': townEmergencyGoods() },
    rejectSell: true,
  });
  await assert.rejects(() => restockInVillage(rejected, freshRubenNavigation(rejected), freshMaterialTopUpOptions), /rejected sale of Venison/);
  assert.equal(rejected.sent.some(entry => entry.type === 'buyItem'), false);

  const recycledState = taoistFreshMaterialTopUpState(1201);
  recycledState.inventoryItems = recycledState.inventoryItems.filter(entry => Number(entry.uniqueId) !== 719102);
  const recycled = new FakeClient(recycledState, {
    goodsByNpc: { '6': [], '43': [], '88': goods(), '42': townEmergencyGoods() },
  });
  const navigate = freshRubenNavigation(recycled);
  const recycledResult = await restockInVillage(recycled, async target => {
    await navigate(target);
    if (Number(target.x) === 292 && Number(target.y) === 603) {
      const next = structuredClone(recycled.snapshot);
      const index = next.inventoryItems.findIndex(entry => Number(entry.uniqueId) === 719101);
      next.inventoryItems[index] = item(AMULET, 1, 'Amulet', 719101);
      recycled.receive(next);
    }
  }, freshMaterialTopUpOptions);
  assert.equal(recycledResult.status, 'needsFunds');
  assert.equal(recycled.sent.some(entry => entry.type === 'sellItem'), false);
  assert.deepEqual(recycled.sent.filter(entry => entry.type === 'interact').map(entry => entry.objectId), [88, 6, 88]);

  const unknown = new FakeClient(taoistFreshMaterialTopUpState(1201), {
    goodsByNpc: { '6': [], '43': [], '88': goods(), '42': townEmergencyGoods() },
  });
  await assert.rejects(() => restockInVillage(
    unknown,
    freshRubenNavigation(unknown, { unknownGold: true }),
    freshMaterialTopUpOptions,
  ), /snapshot\.gold/);
  assert.equal(unknown.sent.some(entry => entry.type === 'sellItem' || entry.type === 'buyItem'), false);
});

test('TownTeleport funding shortfall reports needsFunds after Ruben potions without visiting Scott', async () => {
  const client = new FakeClient(snapshot({ className: 'Wizard', gold: 200, hp: 0, mp: 0 }), {
    goodsByNpc: { '88': goods(), '42': townEmergencyGoods() },
  });
  const result = await restockInVillage(client, async () => {}, {
    targetHp: 1,
    targetMp: 1,
    lowStockHp: 1,
    lowStockMp: 1,
    emergencyTownTeleportCount: 1,
    reserveGold: 0,
  });
  assert.equal(result.status, 'needsFunds');
  assert.deepEqual(result.purchases.map(entry => entry.itemIndex), [HP, MP]);
  assert.equal(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 42), false);
  assert.equal(townTeleportCount(client.snapshot), 0);
});

test('explicit eight-scroll route reserve buys exact Merchant Ruben rows and proves deltas', async () => {
  const client = new FakeClient(snapshot({ gold: 1100, hp: 6 }), { goods: emergencyGoods() });
  const result = await restockInVillage(client, async () => {}, {
    emergencyTeleportCount: 8,
    reserveGold: 0,
  });
  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.purchases, [{
    itemIndex: RANDOM_TELEPORT,
    name: 'RandomTeleport',
    shopItemId: 71701,
    quantity: 8,
    unitPrice: 100,
    cost: 800,
  }]);
  assert.equal(result.after.gold, 300);
  assert.equal(result.after.hp, 6);
  assert.equal(randomTeleportCount(client.snapshot), 8);
  assert.equal(client.sent.at(-1).itemIndex, 71701);
});

test('emergency reserve liquidates Venison until all requested RandomTeleport scrolls are affordable', async () => {
  const state = snapshot({ gold: 0, hp: 6 });
  state.inventoryItems.push(
    { name: 'Venison', uniqueId: 718001, quantity: 1, container: 'bag1', sellValue: 211 },
    { name: 'Venison', uniqueId: 718002, quantity: 1, container: 'bag1', sellValue: 201 },
  );
  const client = new FakeClient(state, { goods: emergencyGoods() });
  const navigations = [];

  const result = await restockInVillage(client, async (...args) => navigations.push(args), {
    emergencyTeleportCount: 4,
    reserveGold: 0,
    liquidateObsoleteMaterials: true,
  });

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.sales.map(sale => sale.name), ['Venison', 'Venison']);
  assert.equal(randomTeleportCount(client.snapshot), 4);
  assert.equal(result.after.gold, 12);
  assert.deepEqual(navigations, [
    [{ x: 292, y: 603 }, 1],
    [{ x: 288, y: 608 }, 1],
  ]);
});

test('emergency reserve is a target and does not repurchase an existing full stock', async () => {
  const state = snapshot({ gold: 500, hp: 6 });
  state.inventoryItems.push(item(RANDOM_TELEPORT, 2, 'RandomTeleport', 717000));
  const client = new FakeClient(state, { goods: emergencyGoods() });
  const result = await restockInVillage(client, async () => {}, {
    emergencyTeleportCount: 2,
    reserveGold: 0,
  });
  assert.equal(result.status, 'sufficient');
  assert.equal(client.snapshot.gold, 500);
  assert.deepEqual(client.sent, []);
});

test('RandomTeleport use requires an ack, a fresh quantity decrease, and changed position', async () => {
  const state = snapshot({ gold: 500, hp: 6 });
  state.inventoryItems.push(item(RANDOM_TELEPORT, 2, 'RandomTeleport', 717001));
  const client = new FakeClient(state);
  const result = await useRandomTeleport(client);
  assert.deepEqual(result, {
    uniqueId: 717001,
    grid: 'inventory',
    quantityBefore: 2,
    quantityAfter: 1,
    from: { x: 290, y: 608 },
    to: { x: 297, y: 611 },
  });
  assert.deepEqual(client.sent.at(-2), { type: 'useItem', uniqueId: 717001, grid: 'inventory' });
  assert.deepEqual(client.sent.at(-1), { type: 'clientVersion' });
  assert.equal(randomTeleportCount(client.snapshot), 1);
});

test('RandomTeleport use fails closed on a rejected ack', async () => {
  const state = snapshot({ gold: 500, hp: 6 });
  state.inventoryItems.push(item(RANDOM_TELEPORT, 1, 'RandomTeleport', 717002));
  const client = new FakeClient(state, { rejectUse: true });
  await assert.rejects(() => useRandomTeleport(client), /UseItem was rejected/);
});

test('TownTeleport use proves consumption and accepts a map change with unchanged coordinates', async () => {
  const state = snapshot({ gold: 500, hp: 6, mapFileName: '5' });
  state.inventoryItems.push(item(TOWN_TELEPORT, 1, 'TownTeleport', 719001));
  const client = new FakeClient(state, { townTeleportMap: '0', townTeleportSamePosition: true });
  const result = await useTownTeleport(client);
  assert.deepEqual(result, {
    uniqueId: 719001,
    grid: 'inventory',
    quantityBefore: 1,
    quantityAfter: 0,
    from: { x: 290, y: 608 },
    to: { x: 290, y: 608 },
    fromMapFileName: '5',
    toMapFileName: '0',
  });
  assert.deepEqual(client.sent.at(-2), { type: 'useItem', uniqueId: 719001, grid: 'inventory' });
  assert.deepEqual(client.sent.at(-1), { type: 'clientVersion' });
  assert.equal(townTeleportCount(client.snapshot), 0);
});

test('TownTeleport UseItem rejection leaves the authoritative item unconsumed', async () => {
  const state = snapshot({ gold: 500, hp: 6 });
  state.inventoryItems.push(item(TOWN_TELEPORT, 1, 'TownTeleport', 719002));
  const client = new FakeClient(state, { rejectUse: true });
  await assert.rejects(() => useTownTeleport(client), /UseItem was rejected/);
  assert.equal(townTeleportCount(client.snapshot), 1);
});

test('TownTeleport false ack cannot be treated as consumption', async () => {
  const state = snapshot({ gold: 500, hp: 6 });
  state.inventoryItems.push(item(TOWN_TELEPORT, 1, 'TownTeleport', 719003));
  const client = new FakeClient(state, { falseAckNoConsume: true });
  await assert.rejects(() => useTownTeleport(client), /UseItem was rejected/);
  assert.equal(townTeleportCount(client.snapshot), 1);
});

test('shared emergency escape cooldown conserves scrolls across independent loops', async () => {
  let timestamp = 10_000;
  let uses = 0;
  const escape = createRandomTeleportEmergencyEscape({
    now: () => timestamp,
    cooldownMs: 12_000,
    criticalCooldownMs: 3_000,
    teleport: async () => ({ to: { x: ++uses, y: uses } }),
  });
  const client = { snapshot: { playerHp: 60, playerMaxHp: 100 } };

  assert.deepEqual(await escape(client), { to: { x: 1, y: 1 } });
  timestamp += 2_000;
  assert.deepEqual(await escape(client), {
    deferred: true,
    retryAfterMs: 10_000,
    hpRatio: 0.6,
  });
  assert.equal(uses, 1);

  client.snapshot.playerHp = 20;
  timestamp += 1_000;
  assert.deepEqual(await escape(client), { to: { x: 2, y: 2 } });
  assert.equal(uses, 2);
});

test('insufficient funds preserves the 100 gold reserve and sends no command', async () => {
  const client = new FakeClient(snapshot({ gold: 139, hp: 0 }));
  let navigated = false;
  const result = await restockInVillage(client, async () => { navigated = true; });
  assert.deepEqual(result, { status: 'needsFunds', stock: { hp: 0, mp: 0, amulet: 0 }, gold: 139, spendableGold: 39, reserveGold: 100 });
  assert.equal(navigated, false);
  assert.deepEqual(client.sent, []);
});

test('journey mode sells only provably superseded quest gear before buying survival stock', async () => {
  const state = snapshot({ gold: 17, hp: 2 });
  state.inventoryItems.push({
    name: 'BronzeWarriorSword',
    uniqueId: 901,
    quantity: 1,
    container: 'bag1',
    equipSlot: 'weapon',
    sellValue: 550,
  });
  state.equipmentItems.push({
    name: 'BronzeShortSword',
    uniqueId: 902,
    slot: 'weapon',
  });
  const client = new FakeClient(state);
  const navigations = [];
  const result = await restockInVillage(client, async (...args) => navigations.push(args), {
    targetHp: 24,
    lowStock: 8,
    reserveGold: 0,
    liquidateSuperseded: true,
    progressionCandidates: [
      { name: 'BronzeWarriorSword', questId: 6 },
      { name: 'BronzeShortSword', questId: 23 },
    ],
  });

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.sales, [{
    uniqueId: 901,
    name: 'BronzeWarriorSword',
    quantity: 1,
    gold: 550,
  }]);
  assert.equal(result.purchases[0].quantity, 14);
  assert.equal(result.after.hp, 16);
  assert.equal(result.after.gold, 7);
  assert.equal(client.snapshot.equipmentItems[0].name, 'BronzeShortSword');
  assert.deepEqual(navigations, [
    [{ x: 296, y: 613 }, 1],
    [{ x: 288, y: 608 }, 1],
  ]);
});

test('journey liquidation accepts Crystal Blacksmith combined BuySell service', async () => {
  const state = snapshot({ gold: 17, hp: 2 });
  state.inventoryItems.push({
    name: 'BronzeWarriorSword',
    uniqueId: 903,
    quantity: 1,
    container: 'bag1',
    equipSlot: 'weapon',
    sellValue: 550,
  });
  state.equipmentItems.push({
    name: 'BronzeShortSword',
    uniqueId: 904,
    slot: 'weapon',
  });
  const client = new FakeClient(state, {
    links: [
      { text: 'View', target: '@BuySell' },
      { text: 'Repair', target: '@Repair' },
    ],
  });

  const result = await restockInVillage(client, async () => {}, {
    targetHp: 8,
    lowStock: 8,
    reserveGold: 0,
    liquidateSuperseded: true,
    progressionCandidates: [
      { name: 'BronzeWarriorSword', questId: 6 },
      { name: 'BronzeShortSword', questId: 23 },
    ],
  });

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.sales.map(sale => sale.name), ['BronzeWarriorSword']);
  assert.ok(client.sent.some(entry =>
    entry.type === 'selectNpcDialog' && entry.target === '@BuySell'));
});

test('journey recovery sells obsolete materials but preserves an active quest item', async () => {
  const state = snapshot({ gold: 27, hp: 0 });
  state.inventoryItems.push(
    { name: 'CannibalLeaf', uniqueId: 910, quantity: 2, container: 'bag1', sellValue: 50 },
    { name: 'SpiderTeeth', uniqueId: 911, quantity: 2, container: 'bag1', sellValue: 50 },
    { name: 'JadeRing', uniqueId: 912, quantity: 1, container: 'bag1', sellValue: 400 },
    { name: 'RandomTeleport', uniqueId: 913, quantity: 2, container: 'bag1', sellValue: 50 },
    { name: 'TownTeleport', uniqueId: 914, quantity: 2, container: 'bag1', sellValue: 500 },
  );
  const client = new FakeClient(state);
  const navigations = [];
  const result = await restockInVillage(client, async (...args) => navigations.push(args), {
    targetHp: 5,
    lowStock: 4,
    reserveGold: 0,
    liquidateObsoleteMaterials: true,
    protectedItemNames: ['JadeRing'],
  });

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.sales.map(sale => sale.name), ['CannibalLeaf', 'SpiderTeeth']);
  assert.equal(result.after.hp, 5);
  assert.equal(result.after.gold, 27);
  assert.ok(client.snapshot.inventoryItems.some(entry => entry.name === 'JadeRing'));
  assert.ok(client.snapshot.inventoryItems.some(entry => entry.name === 'RandomTeleport'));
  assert.ok(client.snapshot.inventoryItems.some(entry => entry.name === 'TownTeleport' && entry.quantity === 2));
  assert.ok(!client.sent.some(entry => entry.type === 'sellItem' && entry.uniqueId === 914));
  assert.deepEqual(navigations, [
    [{ x: 295, y: 605 }, 1],
    [{ x: 288, y: 608 }, 1],
  ]);
  assert.ok(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 43));
  assert.ok(client.sent.some(entry => entry.type === 'selectNpcDialog' && entry.target === '@Sell'));
});

test('journey recovery sells harvested Venison to Butcher John before buying potions', async () => {
  const state = snapshot({ className: 'Taoist', level: 12, gold: 25, hp: 0, mp: 12 });
  state.inventoryItems.push(
    { name: 'Venison', uniqueId: 920, quantity: 1, container: 'bag1', sellValue: 229 },
    { name: 'Venison', uniqueId: 921, quantity: 1, container: 'bag1', sellValue: 225 },
  );
  const client = new FakeClient(state);
  const navigations = [];
  const result = await restockInVillage(client, async (...args) => navigations.push(args), {
    targetHp: 24,
    targetMp: 12,
    lowStock: 4,
    reserveGold: 0,
    liquidateObsoleteMaterials: true,
  });

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.sales.map(sale => sale.name), ['Venison', 'Venison']);
  assert.equal(result.after.hp, 11);
  assert.equal(result.after.mp, 12);
  assert.equal(result.after.gold, 39);
  assert.deepEqual(navigations, [
    [{ x: 292, y: 603 }, 1],
    [{ x: 288, y: 608 }, 1],
  ]);
  assert.ok(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 6));
  assert.equal(client.sent.some(entry => entry.type === 'interact' && entry.objectId === 43), false);
});

test('Warrior buys HP only through the fresh Ruben row id and proves exact snapshot deltas', async () => {
  const client = new FakeClient(snapshot({ gold: 1000, hp: 2, mp: 0 }), { goods: goods() });
  client.packet('NPCGoods', { list: [{ ...goods()[0], id: 999 }], panelType: 0 }); // stale evidence
  const navigated = [];
  const result = await restockInVillage(client, async (...args) => navigated.push(args));
  assert.deepEqual(navigated, [[{ x: 288, y: 608 }, 1]]);
  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.before, { gold: 1000, hp: 2, mp: 0, amulet: 0 });
  assert.deepEqual(result.after, { gold: 840, hp: 6, mp: 0, amulet: 0 });
  assert.deepEqual(result.purchases, [{
    itemIndex: HP, name: '(HP)DrugSmall', shopItemId: '9007199254740993', quantity: 4, unitPrice: 40, cost: 160,
  }]);
  assert.deepEqual(client.sent.map(entry => entry.type), ['interact', 'selectNpcDialog', 'buyItem']);
  assert.deepEqual(client.sent.at(-1), { type: 'buyItem', itemIndex: '9007199254740993', count: 4, panelType: 0 });
});

function samuelSnapshot({ gold = 1000, hp = 0, samuelObjectId = 205 } = {}) {
  const state = snapshot({ gold, hp, npc: false });
  Object.assign(state.entities[0], { x: 328, y: 264 });
  state.entities.push({ objectId: samuelObjectId, kind: 'npc', name: 'Alchemist_Samuel', x: 324, y: 291 });
  return state;
}

test('V2 Basic Potion uses the live Bichon Wall Samuel object and its fresh row receipt', async () => {
  const client = new FakeClient(samuelSnapshot({ samuelObjectId: 205 }), {
    goodsByNpc: { 205: [{ id: '92005', itemIndex: HP, name: '(HP)DrugSmall', price: 40, count: 1 }] },
    enforceMerchantOwnership: true,
  });
  const navigations = [];
  const result = await purchaseV2BasicHpPotion(client, async (...args) => navigations.push(args));
  assert.deepEqual(navigations, [[{ x: 324, y: 291 }, 1]]);
  assert.deepEqual(result, {
    merchant: { objectId: 205, name: 'Alchemist_Samuel', x: 324, y: 291 },
    purchase: { itemIndex: HP, name: '(HP)DrugSmall', shopItemId: '92005', quantity: 1, unitPrice: 40, cost: 40 },
  });
  assert.deepEqual(client.sent, [
    { type: 'interact', objectId: 205 },
    { type: 'selectNpcDialog', target: '@BuySell' },
    { type: 'buyItem', itemIndex: '92005', count: 1, panelType: 0 },
  ]);
  assert.equal(client.snapshot.gold, 960);
  assert.equal(client.snapshot.inventoryItems.find(entry => Number(entry.tooltipSource?.info?.item_index) === HP)?.quantity, 1);
});

test('V2 Samuel purchase fails closed for a wrong goods row, absent receipt, or insufficient gold', async () => {
  const wrongGoods = new FakeClient(samuelSnapshot(), {
    goodsByNpc: { 205: [{ id: 'wrong-hp-row', itemIndex: HP, name: '(HP)DrugMedium', price: 40, count: 1 }] },
  });
  await assert.rejects(() => purchaseV2BasicHpPotion(wrongGoods, async () => {}), /missing \(HP\)DrugSmall/);

  const noReceipt = new FakeClient(samuelSnapshot(), {
    goodsByNpc: { 205: [{ id: '92005', itemIndex: HP, name: '(HP)DrugSmall', price: 40, count: 1 }] },
    rejectBuy: true,
  });
  await assert.rejects(() => purchaseV2BasicHpPotion(noReceipt, async () => {}), /authoritative purchase/);

  const insufficientGold = new FakeClient(samuelSnapshot({ gold: 39 }), {
    goodsByNpc: { 205: [{ id: '92005', itemIndex: HP, name: '(HP)DrugSmall', price: 40, count: 1 }] },
  });
  await assert.rejects(() => purchaseV2BasicHpPotion(insufficientGold, async () => {}), /requires 40 gold, has 39/);
  assert.equal(insufficientGold.sent.some(command => command.type === 'buyItem'), false);
});

test('restock clears an adjacent hostile that seals the supply service approach', async () => {
  const state = snapshot({ gold: 1000, hp: 0 });
  state.entities.push({
    objectId: 200,
    kind: 'monster',
    name: 'Scarecrow',
    disposition: 'hostile',
    x: 291,
    y: 608,
    hp: 20,
    dead: false,
  });
  const client = new FakeClient(state, { goods: goods() });
  const navigations = [];
  const cleared = [];
  const diagnostics = [];
  client.record = (_direction, payload) => diagnostics.push(payload);

  const result = await restockInVillage(client, async (...args) => {
    navigations.push(args);
    if (navigations.length === 1) throw new Error('No walk path on 0 from 290,608 to 288,608');
  }, {
    clearBlockingMonster: async (_owner, blocker) => {
      cleared.push(Number(blocker.objectId));
      Object.assign(blocker, { hp: 0, dead: true });
    },
  });

  assert.equal(result.status, 'restocked');
  assert.equal(navigations.length, 2);
  assert.deepEqual(cleared, [200]);
  assert.ok(diagnostics.some(entry =>
    entry.type === 'supplyServiceBlockerCombat' && entry.objectId === 200));
});

test('restock clears a distant hostile that seals every final service approach', async () => {
  const state = snapshot({ gold: 1000, hp: 0 });
  Object.assign(state.entities.find(entry => entry.objectId === 1), { x: 288, y: 616 });
  state.entities.push(
    { objectId: 201, kind: 'monster', name: 'Scarecrow', disposition: 'hostile', x: 287, y: 609, hp: 4, dead: false },
    { objectId: 202, kind: 'monster', name: 'Scarecrow', disposition: 'hostile', x: 288, y: 609, hp: 20, dead: false },
    { objectId: 203, kind: 'monster', name: 'Scarecrow', disposition: 'hostile', x: 289, y: 609, hp: 20, dead: false },
  );
  const client = new FakeClient(state, { goods: goods() });
  const navigations = [];
  const cleared = [];

  const result = await restockInVillage(client, async (...args) => {
    navigations.push(args);
    if (navigations.length === 1) throw new Error('No walk path on 0 from 288,616 to 288,608');
  }, {
    clearBlockingMonster: async (_owner, blocker) => {
      cleared.push(Number(blocker.objectId));
      Object.assign(blocker, { hp: 0, dead: true });
    },
  });

  assert.equal(result.status, 'restocked');
  assert.equal(navigations.length, 2);
  assert.deepEqual(cleared, [201]);
});

test('a distant village service receives a route budget derived from authoritative distance', async () => {
  const state = snapshot({ className: 'Warrior', gold: 1000, hp: 0 });
  Object.assign(state.entities.find(entry => entry.objectId === 1), { x: 43, y: 111 });
  const client = new FakeClient(state, { goods: goods() });
  const navigations = [];

  await restockInVillage(client, async (...args) => navigations.push(args), {
    reserveGold: 0,
  });

  assert.equal(navigations.length, 1);
  assert.deepEqual(navigations[0].slice(0, 2), [{ x: 288, y: 608 }, 1]);
  assert.equal(navigations[0][2], undefined);
  assert.equal(navigations[0][3].maxAttempts, 593);
});

test('large restock fills the partial potion stack before buying the overflow chunk', async () => {
  const client = new FakeClient(snapshot({ gold: 1107, hp: 3 }));
  const result = await restockInVillage(client, async () => {}, {
    targetHp: 24,
    lowStock: 8,
    reserveGold: 0,
  });

  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.purchases.map(entry => entry.quantity), [17, 4]);
  assert.deepEqual(client.sent.filter(entry => entry.type === 'buyItem').map(entry => entry.count), [17, 4]);
  assert.equal(result.after.hp, 24);
  assert.equal(result.after.gold, 267);
});

test('Taoist does not detour for Amulets before level 18 or without learned SoulFireBall', async () => {
  let navigations = 0;
  const underLevel = new FakeClient(snapshot({
    className: 'Taoist', level: 10, hp: 3, mp: 3, amulet: 1,
    knownSkills: [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }],
  }));
  const underLevelResult = await restockInVillage(underLevel, async () => { navigations += 1; });
  assert.equal(underLevelResult.status, 'sufficient');
  assert.equal(underLevelResult.equippedAmulet, null);
  assert.deepEqual(underLevel.sent, []);

  const unlearned = new FakeClient(snapshot({ className: 'Taoist', level: 20, hp: 3, mp: 3 }));
  const unlearnedResult = await restockInVillage(unlearned, async () => { navigations += 1; });
  assert.equal(unlearnedResult.status, 'sufficient');
  assert.equal(unlearnedResult.equippedAmulet, null);
  assert.deepEqual(unlearned.sent, []);

  const malformedLevel = new FakeClient(snapshot({
    className: 'Taoist', level: 'unknown', hp: 3, mp: 3, amulet: 1,
    knownSkills: [{ spell: 'SoulFireBall' }],
  }));
  assert.equal((await restockInVillage(malformedLevel, async () => { navigations += 1; })).status, 'sufficient');
  assert.deepEqual(malformedLevel.sent, []);
  assert.equal(navigations, 0);
});

test('caster stocks both HP and MP while preserving the fixed gold reserve', async () => {
  const client = new FakeClient(snapshot({
    className: 'Taoist', level: 18, gold: 2000, hp: 1, mp: 2,
    knownSkills: [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }],
  }), { goods: goods() });
  const result = await restockInVillage(client, async () => {});
  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.after, { gold: 1490, hp: 6, mp: 6, amulet: 6 });
  assert.deepEqual(result.purchases.map(entry => [entry.itemIndex, entry.quantity]), [[HP, 5], [MP, 4], [AMULET, 6]]);
  assert.equal(result.purchases.reduce((sum, entry) => sum + entry.cost, 0), 510);
  assert.ok(result.after.gold >= 100);
  assert.deepEqual(result.equippedAmulet, { uniqueId: AMULET, quantity: 6 });
  assert.deepEqual(client.sent.at(-1), { type: 'equipItem', grid: 'inventory', uniqueId: AMULET, to: 9 });
});

test('equips an eligible held Amulet into slot 9 even when total stock is already sufficient', async () => {
  const knownSkills = [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }];
  const client = new FakeClient(snapshot({ className: 'Taoist', level: 18, hp: 3, mp: 3, amulet: 3, knownSkills }));
  const result = await restockInVillage(client, async () => { throw new Error('must not navigate'); });
  assert.equal(result.status, 'sufficient');
  assert.deepEqual(result.equippedAmulet, { uniqueId: AMULET, quantity: 3 });
  assert.deepEqual(client.sent, [{ type: 'equipItem', grid: 'inventory', uniqueId: AMULET, to: 9 }]);

  const tooYoung = new FakeClient(snapshot({ className: 'Taoist', level: 17, hp: 3, mp: 3, amulet: 3, knownSkills }));
  const guarded = await restockInVillage(tooYoung, async () => {});
  assert.equal(guarded.equippedAmulet, null);
  assert.deepEqual(tooYoung.sent, []);
});

test('moves a milestone Amulet stack from its authoritative belt slot before equipping slot 9', async () => {
  const state = snapshot({
    className: 'Taoist', level: 18, hp: 3, mp: 3, beltAmulet: 100,
    knownSkills: [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }],
  });
  state.inventoryItems.push(
    { ...item(HP, 1, '(HP)DrugSmall', 900001), slot: 0 },
    { ...item(MP, 1, '(MP)DrugSmall', 900002), slot: 1 },
  );
  const client = new FakeClient(state);

  const result = await equipHeldAmulet(client);

  assert.deepEqual(result, { uniqueId: 800712, quantity: 100 });
  assert.deepEqual(client.sent, [
    { type: 'moveItem', grid: 'belt', from: 4, to: 8 },
    { type: 'equipItem', grid: 'inventory', uniqueId: 800712, to: 9 },
  ]);
  assert.equal(client.snapshot.beltItems.some(entry => entry.uniqueId === 800712), false);
  assert.ok(client.snapshot.equipmentItems.some(entry =>
    entry.uniqueId === 800712 && entry.slot === 'amulet' && entry.quantity === 100));
});

test('belt Amulet movement remains fail closed for level, space, rejection, and stale snapshots', async () => {
  const knownSkills = [{ spell: 'SoulFireBall', offensive: true, cooldownRemainingTicks: 0, mpCost: 4 }];
  const wrongClass = new FakeClient(snapshot({ className: 'Wizard', level: 18, beltAmulet: 100 }));
  assert.equal(await equipHeldAmulet(wrongClass), null);
  assert.deepEqual(wrongClass.sent, []);

  const tooYoung = new FakeClient(snapshot({ className: 'Taoist', level: 17, beltAmulet: 100, knownSkills }));
  assert.equal(await equipHeldAmulet(tooYoung), null);
  assert.deepEqual(tooYoung.sent, []);

  const fullState = snapshot({ className: 'Taoist', level: 18, beltAmulet: 100, knownSkills });
  fullState.maxBagSlots = 2;
  fullState.inventoryItems = [
    { ...item(HP, 1, '(HP)DrugSmall', 900011), slot: 0 },
    { ...item(MP, 1, '(MP)DrugSmall', 900012), slot: 1 },
  ];
  const full = new FakeClient(fullState);
  assert.equal(await equipHeldAmulet(full), null);
  assert.deepEqual(full.sent, []);

  const rejected = new FakeClient(
    snapshot({ className: 'Taoist', level: 18, beltAmulet: 100, knownSkills }),
    { rejectMove: true },
  );
  await assert.rejects(() => equipHeldAmulet(rejected), /Move from belt to inventory rejected/);
  assert.deepEqual(rejected.sent, [{ type: 'moveItem', grid: 'belt', from: 4, to: 6 }]);

  const stale = new FakeClient(
    snapshot({ className: 'Taoist', level: 18, beltAmulet: 100, knownSkills }),
    { noMoveSnapshot: true },
  );
  await assert.rejects(() => equipHeldAmulet(stale), /Amulet moved from belt to inventory/);
  assert.deepEqual(stale.sent, [{ type: 'moveItem', grid: 'belt', from: 4, to: 6 }]);

  for (const quantity of [undefined, 0, 1.5, 'unknown']) {
    const malformedState = snapshot({ className: 'Taoist', level: 18, knownSkills });
    malformedState.beltItems = [{ ...item(AMULET, quantity, 'Amulet', 800713), container: 'belt', slot: 4 }];
    const malformed = new FakeClient(malformedState);
    assert.equal(await equipHeldAmulet(malformed), null);
    assert.deepEqual(malformed.sent, []);
  }
});

test('a tight caster budget buys a bounded HP partial and never spends the reserve', async () => {
  const client = new FakeClient(snapshot({ className: 'Wizard', gold: 200, hp: 0, mp: 0 }), { goods: goods() });
  const result = await restockInVillage(client, async () => {});
  assert.deepEqual(result.after, { gold: 120, hp: 2, mp: 0, amulet: 0 });
  assert.equal(result.after.gold >= 100, true);
  assert.equal(result.purchases.length, 1);
});

test('Wizard with 600 gold reaches both six-potion targets and preserves the reserve', async () => {
  const client = new FakeClient(snapshot({ className: 'Wizard', gold: 600, hp: 0, mp: 0 }), { goods: goods() });
  const result = await restockInVillage(client, async () => {});
  assert.equal(result.status, 'restocked');
  assert.deepEqual(result.after, { gold: 120, hp: 6, mp: 6, amulet: 0 });
  assert.deepEqual(result.purchases.map(entry => [entry.itemIndex, entry.quantity, entry.cost]), [
    [HP, 6, 240],
    [MP, 6, 240],
  ]);
  assert.equal(result.after.gold >= 100, true);
});

test('requires a live exact Ruben and an enabled exact BuySell link', async () => {
  const missing = new FakeClient(snapshot({ npc: false }));
  await assert.rejects(() => restockInVillage(missing, async () => {}), /live Merchant Ruben/);
  const badLink = new FakeClient(snapshot(), { links: [{ text: 'Buy', target: '@Buy' }] });
  await assert.rejects(() => restockInVillage(badLink, async () => {}), /no enabled @BuySell link/);
  const disabled = new FakeClient(snapshot(), { links: [{ target: '@BuySell', disabled: true }] });
  await assert.rejects(() => restockInVillage(disabled, async () => {}), /no enabled @BuySell link/);
});

test('rejects stale, missing, or malformed goods instead of guessing a shop row', async () => {
  const noGoods = new FakeClient(snapshot(), { noGoods: true });
  noGoods.packet('NPCGoods', { list: goods(), panelType: 0 });
  await assert.rejects(() => restockInVillage(noGoods, async () => {}), /NPCGoods/);
  const missingPotion = new FakeClient(snapshot(), { goods: [goods()[1]] });
  await assert.rejects(() => restockInVillage(missingPotion, async () => {}), /missing \(HP\)DrugSmall/);
  const wrongPanel = new FakeClient(snapshot(), { panelType: 1 });
  await assert.rejects(() => restockInVillage(wrongPanel, async () => {}), /invalid buy goods panel/);
});

test('silent rejection and incorrect authoritative deltas cannot prove purchase', async () => {
  const rejected = new FakeClient(snapshot(), { rejectBuy: true });
  await assert.rejects(() => restockInVillage(rejected, async () => {}), /authoritative purchase/);
  const wrongGold = new FakeClient(snapshot(), { wrongGoldDelta: 1 });
  await assert.rejects(() => restockInVillage(wrongGold, async () => {}), /authoritative purchase/);
  const wrongItem = new FakeClient(snapshot(), { wrongItemDelta: -1 });
  await assert.rejects(() => restockInVillage(wrongItem, async () => {}), /authoritative purchase/);
});
