import assert from 'node:assert/strict';
import test from 'node:test';
import { createV2SupplyReadiness, v2SupplyPolicy, v2SupplyStatus, v2SupplyStock } from './newcomer-v2-supply-policy.mjs';

const item = (name, quantity, extra = {}) => ({ name, quantity, ...extra });
const quest = (questId = 2110015, requirements = {}) => ({ questId, objectives: { flag: [{ requirements }] } });
function snapshot(className = 'Wizard', level = 22) {
  return { playerObjectId: 1, playerHp: 100, playerMaxHp: 100, mapFileName: '0',
    entities: [{ objectId: 1, class: className, level }], knownSkills: [],
    inventoryItems: [item('(HP)DrugSmall', 4), item('(MP)DrugSmall', 12), item('TownTeleport', 2)],
    beltItems: [], equipmentItems: [] };
}

test('V2 caster floors and preferred potion tiers change only at configured boundaries', () => {
  for (const [level, tier, minimum, target] of [[7, 'small', 4, 12], [15, 'small', 4, 12],
    [16, 'medium', 8, 20], [24, 'medium', 8, 20], [25, 'large', 12, 24], [30, 'large', 12, 24]]) {
    const policy = v2SupplyPolicy(snapshot('Wizard', level), quest(), 'Wizard');
    assert.equal(policy.potionTier, tier);
    assert.equal(policy.mp.minimum, minimum);
    assert.equal(policy.mp.target, target);
  }
  assert.equal(v2SupplyPolicy(snapshot('Wizard', 1), quest(2110001), 'Wizard').mp.minimum, 0);
  assert.equal(v2SupplyPolicy(snapshot('Warrior', 28), quest(), 'Warrior').mp.minimum, 0);
});

test('Taoist practice flags require future casts even before the book has been learned', () => {
  const state = snapshot('Taoist', 18);
  const policy = v2SupplyPolicy(state, quest(2110011, { taoist: ['SoulFireBall learned', 'owned poison effect committed'] }), 'Taoist');
  assert.equal(policy.amulet.minimum, 4);
  assert.equal(policy.poison.minimum, 4);
  assert.deepEqual(v2SupplyStatus(state, policy).missing.map(value => value.kind), ['amulet', 'poison']);
  const child = snapshot('Taoist', 13);
  child.knownSkills = [{ spell: 'Poisoning' }, { spell: 'SoulFireBall' }];
  const low = v2SupplyPolicy(child, quest(), 'Taoist');
  assert.equal(low.poison.minimum, 0);
  assert.equal(low.amulet.minimum, 0);
});

test('V2 requirements stay scoped to the current class and use the canonical lowercase keys', () => {
  const state = snapshot('Taoist', 28);
  const onlyWizard = quest(2110021, { wizard: ['SoulFireBall learned', 'owned poison effect committed'] });
  const policy = v2SupplyPolicy(state, onlyWizard, 'Taoist');
  assert.equal(policy.amulet.minimum, 0);
  assert.equal(policy.poison.minimum, 0);
});

test('Wooma retains its 100/48 lawful Amulet minima', () => {
  const state = snapshot('Taoist', 28);
  state.knownSkills = [{ spell: 'SoulFireBall' }];
  assert.equal(v2SupplyPolicy(state, quest(2110020), 'Taoist').amulet.minimum, 100);
  assert.equal(v2SupplyPolicy(state, quest(2110021), 'Taoist').amulet.minimum, 48);
});

test('carried stock merges belt and equipment, accepts all potion tiers, and excludes foreign balances', () => {
  const state = snapshot('Taoist', 28);
  state.inventoryItems = [item('(HP)DrugSmall', 2), item('(HP)DrugMedium', 3), item('GreenPoison', 3),
    item('Amulet', 20, { uniqueId: 11 }), item('Amulet', 99, { container: 'storage' })];
  state.beltItems = [item('(HP)DrugLarge', 4), item('RedPoison', 2)];
  state.equipmentItems = [item('Amulet', 20, { uniqueId: 11, slot: 'amulet' }), item('Amulet', 7, { uniqueId: 12, slot: 'amulet' })];
  state.storageItems = [item('Amulet', 500)];
  state.mailItems = [item('GreenPoison', 500)];
  state.marketItems = [item('(MP)DrugSmall', 500)];
  assert.deepEqual(v2SupplyStock(state), { hp: 9, mp: 0, amulet: 27, poison: 5, townTeleport: 0, randomTeleport: 0 });
});

test('only lawful equipment slot names or explicit wire indices count material charges', () => {
  const state = snapshot('Taoist', 28);
  state.inventoryItems = [];
  state.equipmentItems = [
    item('Amulet', 3, { slot: 'amulet' }), item('GreenPoison', 4, { slot: 'braceletRight' }),
    item('RedPoison', 5, { slot: 6 }), item('Amulet', 6, { slot: 9 }),
    item('Amulet', 100, { slot: 'weapon' }), item('GreenPoison', 100, { slot: 'braceletLeft' }),
    item('RedPoison', 100, { slot: 'unknown' }), item('Amulet', 100),
    item('(HP)DrugLarge', 100, { slot: 'amulet' }),
  ];
  assert.deepEqual(v2SupplyStock(state), { hp: 0, mp: 0, amulet: 9, poison: 9, townTeleport: 0, randomTeleport: 0 });
});

test('shop previews cannot satisfy carried stock or hide a later legal instance with the same UID', () => {
  const state = snapshot('Taoist', 28);
  state.inventoryItems = [
    item('Amulet', 500, { uniqueId: 1, tooltipSource: { user_item: { is_shop_item: true } } }),
    item('Amulet', 500, { uniqueId: 2, tooltipSource: { userItem: { isShopItem: true } } }),
    item('GreenPoison', 500, { isShopItem: true }), item('RedPoison', 500, { is_shop_item: true }),
    item('Amulet', 7, { uniqueId: 1 }),
  ];
  assert.equal(v2SupplyStock(state).amulet, 7);
  assert.equal(v2SupplyStock(state).poison, 0);
});

test('unusable, sealed, broken, zero, malformed and wrong-class stock cannot satisfy departure', () => {
  const state = snapshot('Wizard', 28);
  state.inventoryItems = [
    item('(MP)DrugSmall', 20, { expired: true }), item('(MP)DrugSmall', 20, { sealed: true }),
    item('(MP)DrugSmall', 20, { canUse: false }), item('(MP)DrugSmall', 0),
    item('(MP)DrugSmall', -1), item('(MP)DrugSmall', 2.5), item('(MP)DrugSmall', 'unknown'),
    item('(MP)DrugSmall', 2, { durabilityMax: 10, durabilityCurrent: 0 }),
    item('(MP)DrugSmall', 8, { tooltipSource: { info: { required_class: 4 } } }),
    item('(MP)DrugSmall', 8, { tooltipSource: { info: { required_amount: 35 } } }),
  ];
  assert.equal(v2SupplyStock(state).mp, 0);
  state.inventoryItems = [item('Amulet', 100)];
  state.entities[0].level = 17;
  assert.equal(v2SupplyStock(state).amulet, 0);
});

test('instance expiry and active seals are excluded while unexpired ordinary stacks remain usable', () => {
  const state = snapshot('Taoist', 28);
  const now = Date.now();
  const binary = milliseconds => String(621355968000000000n + BigInt(milliseconds) * 10000n);
  state.inventoryItems = [
    item('Amulet', 10, { tooltipSource: { user_item: { expire_info: { expiry_binary_datetime: binary(now - 1) } } } }),
    item('Amulet', 20, { tooltipSource: { user_item: { expire_info: { expiry_binary_datetime: binary(now + 60_000) } } } }),
    item('Amulet', 30, { tooltipSource: { user_item: { sealed_info: { expiry_binary_datetime: binary(now + 60_000) } } } }),
    item('Amulet', 40, { tooltipSource: { user_item: { sealed_info: { expiry_binary_datetime: 'unknown' } } } }),
    item('Amulet', 50, { shape: 2 }),
  ];
  assert.equal(v2SupplyStock(state, { now }).amulet, 20);
});

function readinessOptions(extra = {}) {
  return { className: 'Wizard', restock: async () => ({ status: 'sufficient' }), useTownTeleport: async () => {},
    useSupplies: async () => {}, waitForHealth: async () => {}, ...extra };
}

test('a claimed restock success with zero MP stays blocked', async () => {
  const client = { snapshot: snapshot() };
  client.snapshot.inventoryItems = client.snapshot.inventoryItems.filter(value => !value.name.startsWith('(MP)'));
  let calls = 0;
  const ensure = createV2SupplyReadiness(readinessOptions({ restock: async () => { calls++; return { status: 'restocked' }; } }));
  const result = await ensure(client, quest(), async () => {});
  assert.equal(result.status, 'blocked');
  assert.equal(calls, 1);
  assert.match(result.message, /mp 0\/8/);
});

test('full health and HP medicine cannot bypass missing Taoist poison', async () => {
  const client = { snapshot: snapshot('Taoist', 28) };
  client.snapshot.knownSkills = [{ spell: 'SoulFireBall' }, { spell: 'Poisoning' }];
  client.snapshot.equipmentItems = [item('Amulet', 100, { slot: 'amulet' })];
  const ensure = createV2SupplyReadiness(readinessOptions({ className: 'Taoist' }));
  const result = await ensure(client, quest(), async () => {});
  assert.equal(result.status, 'blocked');
  assert.deepEqual(result.missing, [{ kind: 'poison', have: 0, minimum: 4 }]);
});

test('fresh recovery consumption is rechecked even after successful purchases', async () => {
  const client = { snapshot: snapshot() };
  client.snapshot.playerHp = 10;
  const ensure = createV2SupplyReadiness(readinessOptions({
    useSupplies: async () => {
      client.snapshot.playerHp = 60;
      client.snapshot.inventoryItems.find(value => value.name.startsWith('(HP)')).quantity--;
    },
  }));
  const result = await ensure(client, quest(), async () => {});
  assert.equal(result.status, 'blocked');
  assert.deepEqual(result.missing, [{ kind: 'hp', have: 3, minimum: 4 }]);
});

test('ordinary return route is used without a scroll; no route cannot become ready', async () => {
  const client = { snapshot: snapshot() };
  client.snapshot.mapFileName = 'D022';
  client.snapshot.inventoryItems = [item('(HP)DrugSmall', 4)];
  const travelCalls = [];
  const ensure = createV2SupplyReadiness(readinessOptions({ restock: async () => {
    client.snapshot.inventoryItems.push(item('(MP)DrugLarge', 20), item('TownTeleport', 2));
    return { status: 'restocked' };
  } }));
  assert.equal((await ensure(client, quest(), async () => {})).status, 'blocked');
  const result = await ensure(client, quest(), async () => {}, { travel: async map => { travelCalls.push(map); client.snapshot.mapFileName = map; } });
  assert.equal(result.status, 'ready');
  assert.deepEqual(travelCalls, ['0']);
});

test('a forbidden TownTeleport falls back to one legal walk route and a blocked walk stops once', async () => {
  const client = { snapshot: snapshot() };
  client.snapshot.mapFileName = 'D022';
  client.snapshot.inventoryItems = client.snapshot.inventoryItems.filter(value => !value.name.startsWith('(MP)'));
  let scrolls = 0;
  let routes = 0;
  const ensure = createV2SupplyReadiness(readinessOptions({ useTownTeleport: async () => { scrolls++; throw new Error('NoTownTeleport'); } }));
  const result = await ensure(client, quest(), async () => {}, { travel: async () => { routes++; throw new Error('No walk path'); } });
  assert.equal(result.status, 'blocked');
  assert.match(result.message, /legal return route is blocked/);
  assert.equal(scrolls, 1);
  assert.equal(routes, 1);
});

test('the original V2 deadline survives a failed supply return', async () => {
  const client = { snapshot: snapshot() };
  client.snapshot.mapFileName = 'D022';
  client.snapshot.inventoryItems = [];
  const error = Object.assign(new Error('deadline reached'), { reason: 'deadline' });
  const ensure = createV2SupplyReadiness(readinessOptions());
  await assert.rejects(ensure(client, quest(), async () => {}, { travel: async () => { throw error; } }), failure => failure === error);
});
