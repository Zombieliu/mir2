import test from 'node:test';
import assert from 'node:assert/strict';
import { createOrdinarySupplies, ORDINARY_SUPPLY_COMMANDS } from './capacity-ordinary-supplies.mjs';

const route = { quests: [{ objectives: { item: [], kill: [{ monsterName: 'Deer', spawnCandidates: [{ mapFileName: '0', position: { x: 290, y: 604 }, count: 4 }] }] } }] };
const item = (id, quantity, uniqueId, slot = 0) => ({ itemIndex: id, name: id === 658 ? '(HP)DrugSmall' : 'Venison', quantity, uniqueId, container: 'bag1', slot, sellValue: 250 });
function fixture(config = {}) {
  let clock = 1, hook = () => {}, seq = 0, currentDeer = null;
  const harvestPasses = new Map();
  let attackPaceArmed = false, attackPaceChanged = false;
  const actor = { objectId: config.ownerId ?? 1, kind: 'selfPlayer', name: 'ordinary-owner', class: 'Warrior', x: 290, y: 607, hp: 100, maxHp: 100, dead: false };
  const client = {
    label: config.label ?? 'ordinary-owner', sequence: 0, events: [], sent: [], slots: [], attackResults: config.v2 ? 'v2' : 'legacy', attackV2Supported: config.v2 === true, closed: false,
    snapshot: { playerObjectId: actor.objectId, mapFileName: '0', playerHp: 100, playerMaxHp: 100, gold: config.gold ?? 0, inventoryCapacity: 46,
      inventoryItems: config.items ?? [], beltItems: [], groundDrops: [], entities: [actor,
        { objectId: 6, kind: 'npc', name: 'Merchant_John', x: 292, y: 603 },
        { objectId: 5, kind: 'npc', name: 'Merchant_Ruben', x: 288, y: 608 },
        ...(config.nonDeerBlocker ? [{ objectId: 99, kind: 'monster', name: 'Scarecrow', x: 289, y: 605, hp: 25, dead: false }] : []),
        ...Array.from({ length: config.deer ?? 0 }, (_, i) => ({ objectId: 20 + i, kind: 'monster', name: 'Deer', x: 290 + i, y: 604, hp: 1, dead: false }))] },
    async pace() {
      if (attackPaceArmed && !attackPaceChanged) {
        attackPaceChanged = true;
        const target = this.snapshot.entities.find(entity => entity.objectId === 20);
        const loss = config.targetLossAtPace;
        if (loss === 'missing') this.snapshot.entities = this.snapshot.entities.filter(entity => entity !== target);
        if (loss === 'dead') Object.assign(target, { dead: true, hp: 0 });
        if (loss === 'dead-string') target.dead = 'true';
        if (loss === 'dead-one') target.dead = 1;
        if (loss === 'dead-zero') target.dead = 0;
        if (loss === 'dead-null') target.dead = null;
        if (loss === 'dead-missing') delete target.dead;
        if (loss === 'dead-false-string') target.dead = 'false';
        if (loss === 'zero-hp') target.hp = 0;
        if (loss === 'unknown-hp') target.hp = null;
        if (loss === 'renamed') target.name = 'Scarecrow';
        if (loss === 'canonical-id') target.objectId = '20';
        if (loss === 'malformed-id') target.objectId = '020';
        if (loss === 'ambiguous-id') this.snapshot.entities.push({ ...target, objectId: '20' });
        if (loss === 'owner-name') actor.name = 'foreign-owner';
        if (loss === 'owner-map') this.snapshot.mapFileName = '2';
        if (loss === 'owner-health') { actor.hp = 10; this.snapshot.playerHp = 10; }
        if (loss === 'transport') this.closed = true;
        snapshot();
      }
      this.slots = this.slots.filter(t => clock - t < 1000);
      if (this.slots.length >= 3) clock = this.slots[0] + 1000;
    },
    nextAttackV2(objectId) { return { type: 'attackV2', objectId, protocolVersion: 2, requestId: String(++seq) }; },
    send(command) {
      this.slots = this.slots.filter(t => clock - t < 1000);
      assert.ok(this.slots.length < 3, 'the original rolling budget includes keepAlive');
      this.slots.push(clock); this.sent.push({ ...command, at: clock }); this.sequence++;
      if (command.type === 'keepAlive') return;
      if (command.type === 'clientVersion') snapshot();
      if (command.type === 'interact' && !config.noDialog) {
        this.snapshot.activeNpcDialog = { npcObjectId: config.wrongNpc ? 999 : command.objectId,
          links: [{ target: command.objectId === 6 ? '@Sell' : '@BuySell', disabled: config.disabledLink === true }] };
        snapshot();
      }
      if (command.type === 'selectNpcDialog') {
        const sale = command.target === '@Sell';
        const foreignDialog = config.foreignServiceDialog === true || config.foreignServiceDialog === (sale ? 'sell' : 'buy');
        if (!config.noServicePacket) packet(sale ? 'NPCSell' : 'NPCGoods', sale ? {} : { panelType: config.panel ?? 0,
          list: [{ id: 7658, itemIndex: config.wrongRow ? 659 : 658, name: '(HP)DrugSmall', price: config.price ?? 40 }] },
        config.staleServicePacket ? this.sequence - 1 : undefined);
        if (config.closeServiceDialog || foreignDialog || config.serviceMapChange || config.serviceOwnerChange || config.replaceServiceNpc) {
          const objectId = this.snapshot.activeNpcDialog?.npcObjectId;
          this.snapshot.activeNpcDialog = foreignDialog ? { npcObjectId: 999, links: [] } : null;
          if (config.serviceMapChange) this.snapshot.mapFileName = '2';
          if (config.serviceOwnerChange) this.snapshot.playerObjectId = 2;
          if (config.replaceServiceNpc) this.snapshot.entities.find(entity => entity.objectId === objectId).objectId = 999;
          snapshot();
        }
      }
      if (command.type === 'sellItem') {
        const index = this.snapshot.inventoryItems.findIndex(entry => entry.uniqueId === command.uniqueId), held = this.snapshot.inventoryItems[index];
        assert.equal(held?.name, 'Venison');
        this.snapshot.inventoryItems.splice(index, 1); this.snapshot.gold += held.sellValue * command.count;
        packet('SellItem', { ...command, success: true }); snapshot();
      }
      if (command.type === 'buyItem') {
        assert.equal(command.itemIndex, 7658, 'buy uses live row ID, not template ID');
        if (config.changeMap) this.snapshot.mapFileName = '2';
        if (config.changeOwner) this.snapshot.playerObjectId = 2;
        if (config.changeOwnerName) actor.name = 'replacement-owner';
        if (config.late) clock += 12_001;
        if (!config.ackOnly) {
          this.snapshot.gold -= command.count * (config.price ?? 40) + (config.wrongDebit ? 1 : 0);
          const stack = this.snapshot.inventoryItems.find(entry => entry.itemIndex === 658 && entry.quantity + command.count <= 20);
          if (stack) stack.quantity += command.count;
          else this.snapshot.inventoryItems.push(item(658, command.count, 900 + this.sent.length, this.snapshot.inventoryItems.length));
          snapshot(config.stale ? this.sequence - 1 : undefined);
        } else packet('BuyItem', { success: true });
      }
      if (['attack', 'attackV2'].includes(command.type)) {
        if (config.v2) assert.equal(command.type, 'attackV2');
        currentDeer = this.snapshot.entities.find(entity => Number(entity.objectId) === command.objectId);
        Object.assign(currentDeer, { dead: true, hp: 0 });
        packet('ObjectDied', { objectId: command.objectId }); snapshot();
      }
      if (command.type === 'harvest') {
        assert.equal(currentDeer?.dead, true);
        const passes = (harvestPasses.get(currentDeer.objectId) ?? 0) + 1;
        harvestPasses.set(currentDeer.objectId, passes);
        if (config.multiPassHarvest) {
          packet('ObjectHarvest', { objectId: actor.objectId });
          if (config.harvestHealthDrop) { this.snapshot.playerHp = 8; actor.hp = 8; }
          if (passes < 6) { snapshot(); return; }
          assert.equal(passes, 6, 'ordinary Deer only transfers after five skin passes');
          if (config.directHarvest) {
            this.snapshot.inventoryItems.push(item(999, 1, 500 + Number(currentDeer.objectId), this.snapshot.inventoryItems.length));
            packet('GainedItem', { item: { uniqueId: 500 + Number(currentDeer.objectId), itemIndex: 853, count: 1 } });
            packet('ObjectHarvested', { objectId: currentDeer.objectId }); snapshot(); return;
          }
        }
        this.snapshot.groundDrops.push({ objectId: 100 + Number(currentDeer.objectId), name: 'Venison', x: currentDeer.x, y: currentDeer.y, loot: { kind: 'inventoryItem', name: 'Venison' } });
        packet('ObjectHarvested', { objectId: currentDeer.objectId }); snapshot();
      }
      if (command.type === 'pickUp') {
        const index = this.snapshot.groundDrops.findIndex(drop => drop.objectId === command.objectId), drop = this.snapshot.groundDrops[index];
        assert.deepEqual({ x: actor.x, y: actor.y }, { x: drop.x, y: drop.y });
        this.snapshot.groundDrops.splice(index, 1);
        this.snapshot.inventoryItems.push(item(999, 1, 500 + command.objectId, this.snapshot.inventoryItems.length)); snapshot();
      }
      if (['walk', 'run'].includes(command.type)) {
        Object.assign(actor, command.destination); packet('UserLocation', { objectId: 1, x: actor.x, y: actor.y }); snapshot();
      }
    },
    async wait(predicate, label, timeout) {
      const end = clock + timeout;
      while (clock <= end) {
        const value = predicate(); if (value) return value;
        if (label.includes('funding harvest') && config.harvestWaitError) throw config.harvestWaitError;
        if (label.includes('funding harvest') && config.closeOnHarvestWait) { this.closed = true; throw new Error('fixture transport closed'); }
        clock += 100;
      }
      const error = new Error(`${this.label} timed out waiting for ${label}`);
      error.code = 'CAPACITY_WAIT_TIMEOUT'; error.waitLabel = label; this.lastWaitTimeout = error; throw error;
    },
  };
  function packet(name, payload, sequence) {
    const receipt = { type: 'packet', packet: name, payload, direction: 'received', sequence: sequence ?? ++client.sequence, monotonicMs: ++clock };
    client.events.push(receipt); hook(receipt, receipt); return receipt;
  }
  function snapshot(sequence) {
    const message = { type: 'worldSnapshot', payload: structuredClone(client.snapshot) };
    const receipt = { ...message, direction: 'received', sequence: sequence ?? ++client.sequence, monotonicMs: ++clock };
    client.events.push(receipt); hook(message, receipt);
  }
  let preparation = true;
  const supply = createOrdinarySupplies(client, { preparationProfile: 'ordinary-economic-v1', targetHp: config.target ?? 12, route,
    stagingPoint: { x: 290, y: 607 }, resolveStagingPoint: config.resolveStagingPoint, isPreparation: () => preparation, now: () => clock,
    sleep: async ms => { clock += ms; }, maxHunts: config.maxHunts ?? 8, timeoutMs: config.timeoutMs ?? 600_000,
    maxCommands: config.maxCommands ?? 2048,
    navigate: async (facade, target, range) => {
      if (config.nonDeerBlocker && target.objectId === 20) throw new Error('No walk path to this Deer');
      if (config.navigationFailure) throw new Error('fixture blocked ordinary route');
      if (config.failPhase) preparation = false;
      const destination = { x: target.x - (range > 0 ? 1 : 0), y: target.y };
      facade.send({ type: 'walk', direction: 'Right', destination });
      await facade.wait(() => actor.x === destination.x && actor.y === destination.y, 'ordinary navigation');
      if (config.targetLossAtPace && target.objectId === 20) attackPaceArmed = true;
    } });
  hook = supply.observe;
  return { client, supply, snapshot, advance: ms => { clock += ms; }, endPreparation: () => { preparation = false; } };
}

test('default is held-only noop and economic profile has explicit bounded inputs', async () => {
  const result = await createOrdinarySupplies({}).prepare();
  assert.equal(result.status, 'disabled'); assert.equal(result.targetHp, 0); assert.equal(result.commands, 0);
  assert.throws(() => createOrdinarySupplies({}, { preparationProfile: 'held-only', targetHp: 12 }), /held-only/);
  assert.throws(() => createOrdinarySupplies({}, { targetHp: 41 }), /targetHp/);
  assert.deepEqual(ORDINARY_SUPPLY_COMMANDS, ['harvest', 'interact', 'selectNpcDialog', 'sellItem', 'buyItem']);
});

test('same local owner ID 1000 and map cannot conceal a changed character name', async () => {
  const changed = fixture({ ownerId: 1000, gold: 480, changeOwnerName: true });
  await assert.rejects(changed.supply.prepare(), /owner\/map\/health changed|received owner\/map changed/);
  assert.equal(changed.client.snapshot.playerObjectId, 1000); assert.equal(changed.client.snapshot.mapFileName, '0');
  assert.equal(changed.supply.status().status, 'failed'); assert.equal(changed.supply.status().actualStagingReached, false);
  const ready = fixture({ ownerId: 1000, gold: 480 });
  const result = await ready.supply.prepare();
  assert.equal(result.before.ownerName, 'ordinary-owner'); assert.equal(result.after.ownerName, 'ordinary-owner');
  ready.client.snapshot.entities.find(entity => entity.objectId === 1000).name = 'replacement-owner';
  ready.snapshot(); assert.throws(() => ready.supply.assertReady(), /readiness unavailable/);
  const wrongLabel = fixture({ gold: 480, label: 'wrong-label' });
  await assert.rejects(wrongLabel.supply.prepare(), /owner\/map\/health changed/);
  assert.equal(wrongLabel.client.sent.length, 0);
});

test('earned ordinary gold buys only live HP rows in bounded batches and proves exact stock/debit', async () => {
  const f = fixture({ gold: 1200, target: 25 });
  f.client.send({ type: 'keepAlive' }); f.client.send({ type: 'keepAlive' }); f.client.send({ type: 'keepAlive' });
  const result = await f.supply.prepare();
  assert.equal(result.status, 'ready'); assert.equal(result.after.hpStock, 25); assert.equal(result.after.gold, 200);
  assert.deepEqual(f.client.sent.filter(c => c.type === 'buyItem').map(c => c.count), [20, 5]);
  assert.equal(result.proofs.length, 2); assert.equal(result.goldSpent, 1000); assert.equal(result.measurementCredit, false);
  const cursor = f.client.sequence; f.snapshot();
  assert.equal(f.supply.assertReady({ afterSequence: cursor }).hpStock, 25);
  f.advance(12_001); assert.throws(() => f.supply.assertReady(), /fresh cohort readiness/);
});

test('held Venison alone funds purchases without selling any starter gear', async () => {
  const f = fixture({ items: [item(999, 2, 500), { ...item(221, 1, 777, 1), name: 'WoodenSword' }] });
  const result = await f.supply.prepare();
  assert.equal(result.soldVenison, 2); assert.equal(result.goldEarned, 500); assert.equal(result.purchased, 12);
  assert.equal(f.client.sent.filter(c => c.type === 'attack').length, 0);
  assert.deepEqual(f.client.sent.filter(c => c.type === 'sellItem').map(c => c.uniqueId), [500]);
  assert.equal(f.client.snapshot.inventoryItems.some(i => i.name === 'WoodenSword'), true);
});

test('zero-wallet preparation uses actual Deer combat, harvest, same-tile pickup and ordinary sale', async () => {
  const f = fixture({ deer: 3, v2: true });
  const result = await f.supply.prepare();
  assert.equal(result.status, 'ready'); assert.equal(result.funding.collected, 3); assert.equal(result.funding.hunts, 3);
  for (const type of ['attackV2', 'harvest', 'pickUp', 'sellItem', 'buyItem']) assert.ok(f.client.sent.some(command => command.type === type), type);
  assert.equal(f.client.sent.some(c => ['attack', 'townRevive', 'teleport', 'qa.giveItem'].includes(c.type)), false);
  assert.equal(result.after.hpStock, 12); assert.equal(result.after.gold, 20);
  assert.equal(result.after.venison, 1, 'sell only the meat needed for this purchase');
});

test('stale, late, ACK-only and non-exact debit cannot prove a purchase', async t => {
  for (const option of ['stale', 'late', 'ackOnly', 'wrongDebit']) await t.test(option, async () => {
    const f = fixture({ gold: 480, [option]: true });
    await assert.rejects(f.supply.prepare()); assert.equal(f.supply.status().status, 'failed');
    assert.equal(f.supply.status().actualStagingReached, false);
  });
});

test('wrong merchant, disabled link, wrong goods and wrong panel reject before buying', async t => {
  for (const config of [{ wrongNpc: true }, { disabledLink: true }, { wrongRow: true }, { panel: 1 }]) await t.test(JSON.stringify(config), async () => {
    const f = fixture({ gold: 480, ...config });
    await assert.rejects(f.supply.prepare()); assert.equal(f.client.sent.some(c => c.type === 'buyItem'), false);
  });
});

test('owner/map transitions, death, nonpreparation, routes and finite budgets fail without activation', async t => {
  for (const config of [{ changeMap: true }, { changeOwner: true }, { failPhase: true }, { navigationFailure: true }, { maxCommands: 1 }, { timeoutMs: 1 }]) await t.test(JSON.stringify(config), async () => {
    const f = fixture({ gold: 480, ...config }); await assert.rejects(f.supply.prepare());
    assert.equal(f.supply.status().status, 'failed'); assert.equal(f.supply.status().actualStagingReached, false);
  });
  const dead = fixture({ gold: 480 }); dead.client.snapshot.playerHp = 0;
  await assert.rejects(dead.supply.prepare(), /health/);
});

test('full inventory and insufficient finite hunting proceeds never send an unaffordable buy', async () => {
  const full = fixture({ gold: 480, items: Array.from({ length: 40 }, (_, i) => ({ ...item(221, 1, 100 + i, i), name: 'WoodenSword' })) });
  await assert.rejects(full.supply.prepare(), /inventory full/); assert.equal(full.client.sent.some(c => c.type === 'buyItem'), false);
  const poor = fixture({ items: [{ ...item(999, 3, 500), sellValue: 1 }], deer: 1, maxHunts: 1 });
  await assert.rejects(poor.supply.prepare()); assert.equal(poor.client.sent.some(c => c.type === 'buyItem'), false);
});

test('cohort readiness is revalidated after peers prepare and never trusts old ready state', async () => {
  const f = fixture({ gold: 480 }); await f.supply.prepare();
  f.client.snapshot.entities.push({ objectId: 100, kind: 'monster', disposition: 'hostile', hp: 10, x: 291, y: 607 });
  f.snapshot(); assert.throws(() => f.supply.assertReady(), /hostile near staging/);
  f.endPreparation(); assert.throws(() => f.supply.assertReady(), /unavailable/);
  await assert.rejects(f.supply.prepare(), /single-use/);
});

function addStagingHostiles(client) {
  client.snapshot.entities.push(...[[291, 607], [290, 614], [297, 607]].map(([x, y], index) => ({
    objectId: 100 + index, kind: 'monster', name: 'Scarecrow', ai: 0, disposition: 'hostile', hp: 20, dead: false, x, y,
  })));
}

test('fresh authoritative staging membership permits normal nearby hostiles and records the qualified protection', async () => {
  const f = fixture({ items: [item(658, 12, 500)] });
  f.client.snapshot.inSafeZone = true; addStagingHostiles(f.client);
  const result = await f.supply.prepare();
  assert.equal(result.status, 'ready'); assert.equal(result.after.inSafeZone, true);
  assert.equal(result.after.nearbyHostileCount, 3); assert.equal(result.after.currentNearbyHostileCount, 3);
  assert.equal(result.after.stagingProtection, 'authoritative-safe-zone-normal-monsters');
  assert.match(result.stagingSafetyScope, /Existing poison and released effects may still damage/);
  assert.equal(result.after.playerPoison, null); assert.equal(result.after.ownerPoison, null);
  const cursor = f.client.sequence; f.snapshot();
  const ready = f.supply.assertReady({ afterSequence: cursor });
  assert.ok(ready.sequence > cursor); assert.equal(ready.inSafeZone, true); assert.equal(ready.nearbyHostileCount, 3);
  assert.equal(f.client.sent.some(command => command.type === 'attack' || command.type === 'attackV2'), false);
});

test('false missing and nonboolean membership retain the original eight-cell hostile staging guard', async t => {
  for (const membership of [false, undefined, 'true']) await t.test(String(membership), async () => {
    const f = fixture({ items: [item(658, 12, 500)] });
    f.client.snapshot.inSafeZone = membership; addStagingHostiles(f.client);
    await assert.rejects(f.supply.prepare(), /hostile near staging/);
    assert.equal(f.supply.status().actualStagingReached, false);
    assert.equal(f.supply.status().after.inSafeZone, membership === false ? false : null);
    assert.equal(f.supply.status().after.nearbyHostileCount, 3);
  });
});

test('cohort membership never reuses an old true receipt after false missing stale or mismatched staging evidence', async t => {
  for (const boundary of ['false', 'missing', 'old-sequence', 'expired', 'wrong-position']) await t.test(boundary, async () => {
    const f = fixture({ items: [item(658, 12, 500)] });
    f.client.snapshot.inSafeZone = true; addStagingHostiles(f.client); await f.supply.prepare();
    const cursor = f.client.sequence;
    if (boundary === 'false') f.client.snapshot.inSafeZone = false;
    if (boundary === 'missing') delete f.client.snapshot.inSafeZone;
    if (boundary === 'wrong-position') f.client.snapshot.entities[0].x++;
    f.snapshot(boundary === 'old-sequence' ? cursor - 1 : undefined);
    if (boundary === 'expired') f.advance(12_001);
    // The live cache's boolean or coordinates are not a new authoritative receipt.
    f.client.snapshot.inSafeZone = true; f.client.snapshot.entities[0].x = 290;
    assert.throws(() => f.supply.assertReady({ afterSequence: cursor }),
      boundary === 'false' || boundary === 'missing' ? /hostile near staging/ : /fresh cohort readiness/);
  });
});

test('a newer invalid snapshot retires the older true staging receipt even after the live cache becomes legal again', async () => {
  const f = fixture({ items: [item(658, 12, 500)] });
  f.client.snapshot.inSafeZone = true; addStagingHostiles(f.client); await f.supply.prepare();
  const cursor = f.client.sequence; f.snapshot();
  assert.equal(f.supply.assertReady({ afterSequence: cursor }).inSafeZone, true);
  f.client.snapshot.gold = -1; f.snapshot(); // Scalar validation rejects this actual newer receipt.
  f.client.snapshot.gold = 0; // A valid live cache is not another WorldSnapshot.
  assert.throws(() => f.supply.assertReady({ afterSequence: cursor }), /fresh cohort readiness/);
  f.snapshot(); assert.equal(f.supply.assertReady({ afterSequence: cursor }).inSafeZone, true);
});

test('known poison or Dazed still rejects preparation and cohort readiness inside a fresh authoritative safe zone', async t => {
  for (const field of ['playerPoison', 'ownerPoison', 'dazed']) await t.test(field, async () => {
    for (const phase of ['prepare', 'cohort']) {
      const f = fixture({ items: [item(658, 12, 500)] });
      f.client.snapshot.inSafeZone = true; addStagingHostiles(f.client);
      if (phase === 'cohort') await f.supply.prepare();
      if (field === 'playerPoison') f.client.snapshot.playerPoison = 1;
      if (field === 'ownerPoison') f.client.snapshot.entities[0].poison = 4;
      if (field === 'dazed') f.client.snapshot.entities[0].dazed = true;
      if (phase === 'prepare') await assert.rejects(f.supply.prepare(), /negative status at staging/);
      else { const cursor = f.client.sequence; f.snapshot(); assert.throws(() => f.supply.assertReady({ afterSequence: cursor }), /negative status at staging/); }
    }
  });
});

test('existing held HP stacks reduce purchases and are counted only once across bag and belt', async () => {
  const f = fixture({ gold: 80, items: [item(658, 10, 500)] });
  const result = await f.supply.prepare();
  assert.equal(result.purchased, 2); assert.equal(result.after.hpStock, 12); assert.equal(result.after.gold, 0);
  const invalid = fixture({ gold: 80, items: [item(658, 10, 500), item(658, 10, 500, 1)] });
  await assert.rejects(invalid.supply.prepare(), /duplicate/);
});

test('an empty authoritative Deer field exhausts its finite refresh budget', async () => {
  const f = fixture();
  await assert.rejects(f.supply.prepare(), /bounded refreshes/);
  assert.equal(f.supply.status().status, 'failed');
  assert.equal(f.client.sent.some(c => c.type === 'buyItem'), false);
  assert.ok(f.supply.status().commands < 2048);
  assert.equal(f.supply.status().commandRows.length, 32);
  assert.equal(f.supply.status().commandRowsTruncated, true);
});

test('v2 physical hunting never silently falls back to legacy attack', async () => {
  const f = fixture({ deer: 3, v2: true }); f.client.nextAttackV2 = undefined;
  await assert.rejects(f.supply.prepare(), /v2 physical attack unavailable/);
  assert.equal(f.client.sent.some(c => c.type === 'attack'), false);
});

test('fresh NPCGoods and NPCSell services may close their text dialog without losing merchant binding', async () => {
  const f = fixture({ closeServiceDialog: true, items: [item(999, 2, 500)] });
  const result = await f.supply.prepare();
  assert.equal(result.status, 'ready'); assert.equal(result.after.hpStock, 12);
  assert.equal(result.soldVenison, 2); assert.equal(result.purchased, 12);
  assert.equal(f.client.snapshot.activeNpcDialog, null);
  assert.ok(f.client.events.some(event => event.packet === 'NPCGoods'));
  assert.ok(f.client.events.some(event => event.packet === 'NPCSell'));
  assert.equal(result.proofs.filter(proof => proof.kind === 'sale').length, 1);
  assert.equal(result.proofs.filter(proof => proof.kind === 'purchase').length, 1);
});

test('closing a service dialog cannot excuse foreign dialog, changed NPC/owner/map or stale service receipt', async t => {
  for (const option of ['foreignServiceDialog', 'replaceServiceNpc', 'serviceOwnerChange', 'serviceMapChange', 'staleServicePacket', 'noServicePacket']) await t.test(option, async () => {
    const f = fixture({ gold: 480, closeServiceDialog: true, [option]: true });
    await assert.rejects(f.supply.prepare());
    assert.equal(f.supply.status().status, 'failed');
    assert.equal(f.client.sent.some(command => ['buyItem', 'sellItem'].includes(command.type)), false);
    assert.equal(f.supply.status().actualStagingReached, false);
  });
  await t.test('foreign NPCSell dialog', async () => {
    const f = fixture({ closeServiceDialog: true, foreignServiceDialog: 'sell', items: [item(999, 2, 500)] });
    await assert.rejects(f.supply.prepare(), /merchant changed during service/);
    assert.ok(f.client.events.some(event => event.packet === 'NPCSell'));
    assert.equal(f.client.sent.some(command => ['buyItem', 'sellItem'].includes(command.type)), false);
  });
});

test('real five-pass Deer skinning retries typed deadlines and the sixth transfer earns actual stock', async t => {
  for (const directHarvest of [true, false]) await t.test(directHarvest ? 'direct inventory transfer' : 'same-tile public pickup', async () => {
    const f = fixture({ deer: 3, v2: true, multiPassHarvest: true, directHarvest });
    const result = await f.supply.prepare();
    assert.equal(result.status, 'ready'); assert.equal(result.funding.hunts, 3); assert.equal(result.funding.collected, 3);
    assert.equal(f.client.sent.filter(command => command.type === 'harvest').length, 18);
    assert.equal(f.client.events.filter(event => event.packet === 'ObjectHarvest').length, 18);
    assert.equal(f.client.events.filter(event => event.packet === 'ObjectHarvested').length, 3);
    assert.equal(f.client.sent.filter(command => command.type === 'pickUp').length, directHarvest ? 0 : 3);
    assert.equal(result.after.hpStock, 12); assert.equal(result.after.gold, 20); assert.equal(result.after.venison, 1);
    assert.equal(result.soldVenison, 2); assert.equal(result.purchased, 12); assert.equal(result.proofs.length, 3);
    assert.ok(result.elapsedMs >= 90_000 && result.elapsedMs < 600_000, 'virtual clock covers all fifteen six-second empty harvest waits');
  });
});

test('timeout-looking non-deadline errors and mismatched typed labels retain their identity', async t => {
  const lookalike = new Error('ordinary-owner timed out waiting for Deer corpse 20 funding harvest');
  const safety = new Error('timed out guard'); safety.name = 'SafetyStop';
  const wrongLabel = new Error('ordinary-owner timed out waiting for unrelated wait');
  wrongLabel.code = 'CAPACITY_WAIT_TIMEOUT'; wrongLabel.waitLabel = 'unrelated wait';
  for (const original of [lookalike, safety, wrongLabel]) await t.test(original.name + ':' + original.message, async () => {
    const f = fixture({ deer: 3, multiPassHarvest: true, harvestWaitError: original });
    await assert.rejects(f.supply.prepare(), error => error === original);
    assert.equal(f.client.sent.filter(command => command.type === 'harvest').length, 1);
    assert.equal(f.supply.status().status, 'failed'); assert.equal(f.supply.status().actualStagingReached, false);
  });
});

test('global wall budget, health change and closed transport are never retried as empty skin passes', async t => {
  for (const config of [{ timeoutMs: 7000 }, { harvestHealthDrop: true }, { closeOnHarvestWait: true }]) await t.test(JSON.stringify(config), async () => {
    const f = fixture({ deer: 3, multiPassHarvest: true, ...config });
    await assert.rejects(f.supply.prepare(), /wall budget|health changed|transport closed/);
    assert.equal(f.client.sent.filter(command => command.type === 'harvest').length, 1);
    assert.equal(f.client.sent.some(command => command.type === 'buyItem'), false);
    assert.equal(f.supply.status().status, 'failed');
  });
});

test('normalized deadline errors retain code, exact wait label and original cause', async () => {
  const platform = fixture({ gold: 480, ackOnly: true });
  await assert.rejects(platform.supply.prepare(), error => error.code === 'CAPACITY_WAIT_TIMEOUT' &&
    error.waitLabel === 'fresh exact HP purchase' && error.message === 'Timeout waiting for fresh exact HP purchase' &&
    error.cause === platform.client.lastWaitTimeout && error.cause.message.includes('timed out waiting for'));
  const monotonic = fixture({ gold: 480, late: true });
  await assert.rejects(monotonic.supply.prepare(), error => error.code === 'CAPACITY_WAIT_TIMEOUT' &&
    error.waitLabel === 'fresh exact HP purchase' && error.message === 'Timeout waiting for fresh exact HP purchase');
});


test('late staging resolver runs once after purchase and freezes its copied point for fresh readiness', async () => {
  let calls = 0, selected = { x: 291, y: 607 };
  const f = fixture({ gold: 480, resolveStagingPoint: ({ snapshot, receipt, defaultPoint }) => {
    calls++; assert.equal(snapshot.inventoryItems.filter(i => i.itemIndex === 658).reduce((n, i) => n + i.quantity, 0), 12);
    assert.ok(receipt.sequence > 0 && receipt.monotonicMs > 0); assert.deepEqual(defaultPoint, { x: 290, y: 607 });
    return selected;
  } });
  const result = await f.supply.prepare();
  assert.equal(calls, 1); assert.deepEqual(result.resolvedStagingPoint, { x: 291, y: 607 });
  selected.x = 400;
  assert.equal(f.supply.assertReady().x, 291); assert.equal(result.targetHp, 12);
  assert.equal(f.client.sent.filter(c => c.type === 'buyItem').length, 1);
  await assert.rejects(f.supply.prepare(), /single-use/); assert.equal(calls, 1);
});

test('late staging resolver reuses current fresh snapshot without adding a refresh or economic command', async () => {
  const held = [item(658, 12, 900)];
  const base = fixture({ items: structuredClone(held) }); await base.supply.prepare();
  let calls = 0;
  const picked = fixture({ items: structuredClone(held), resolveStagingPoint: ({ defaultPoint }) => { calls++; return defaultPoint; } });
  await picked.supply.prepare();
  assert.equal(calls, 1);
  assert.deepEqual(picked.client.sent.map(c => c.type), base.client.sent.map(c => c.type));
  assert.equal(picked.supply.status().purchased, 0); assert.equal(picked.supply.status().funding, null);
});

test('no candidate or an asynchronous resolver fails closed without restarting preparation budgets', async () => {
  let calls = 0;
  const noPoint = fixture({ items: [item(658, 12, 900)], resolveStagingPoint: () => { calls++; return null; } });
  await assert.rejects(noPoint.supply.prepare(), /resolved stagingPoint/);
  assert.equal(calls, 1); assert.equal(noPoint.supply.status().actualStagingReached, false);
  await assert.rejects(noPoint.supply.prepare(), /single-use/); assert.equal(calls, 1);
  const asyncPoint = fixture({ items: [item(658, 12, 900)], resolveStagingPoint: () => Promise.resolve({ x: 291, y: 607 }) });
  await assert.rejects(asyncPoint.supply.prepare(), /must be synchronous/);
});

test('late point choice cannot reset the global wall or bypass owner/map and final fresh safety', async () => {
  let late;
  late = fixture({ items: [item(658, 12, 900)], resolveStagingPoint: ({ defaultPoint }) => { late.advance(600001); return defaultPoint; } });
  await assert.rejects(late.supply.prepare(), /wall budget exhausted/);
  let changed;
  changed = fixture({ items: [item(658, 12, 900)], resolveStagingPoint: ({ snapshot, defaultPoint }) => { snapshot.mapFileName = '2'; return defaultPoint; } });
  await assert.rejects(changed.supply.prepare(), /owner\/map\/health changed/);
  assert.equal(changed.supply.status().actualStagingReached, false);
});

// The public funding helper may reselect only after an exact, bounded target loss.
test('a queued Deer lost during ordinary pacing is not attacked and consumes the original hunt budget', async t => {
  for (const loss of ['missing', 'dead', 'zero-hp', 'unknown-hp', 'renamed']) await t.test(loss, async () => {
    const f = fixture({ deer: 4, v2: true, targetLossAtPace: loss });
    const result = await f.supply.prepare();
    assert.equal(result.status, 'ready');
    assert.equal(result.funding.collected, 3);
    assert.equal(f.client.sent.some(command => command.type === 'attackV2' && command.objectId === 20), false);
    assert.ok(result.fundingTargetRejections.length >= 1 && result.fundingTargetRejections.length <= 8);
    assert.equal(result.fundingTargetRejections[0].targetId, 20);
    assert.equal(result.fundingTargetRejections[0].phase, 'send');
    assert.equal(result.fundingTargetRejections[0].reason, loss === 'missing' ? 'missing' : loss === 'renamed' ? 'not-deer' : 'not-confirmed-live');
    assert.equal(result.fundingTargetRejections[0].ownerName, 'ordinary-owner');
    assert.equal(result.fundingTargetRejections[0].map, '0');
    assert.equal(result.measurementCredit, false);
  });
  const bounded = fixture({ deer: 4, v2: true, targetLossAtPace: 'dead', maxHunts: 3 });
  await assert.rejects(bounded.supply.prepare(), /after \d+ hunts and 3 attempts/);
  assert.equal(bounded.client.sent.filter(command => command.type === 'attackV2').length, 2);
  assert.equal(bounded.client.sent.some(command => command.type === 'buyItem'), false);
});

test('a non-Deer approach blocker remains unattackable without aborting a bounded alternative Deer hunt', async () => {
  const f = fixture({ deer: 4, v2: true, nonDeerBlocker: true });
  const result = await f.supply.prepare();
  assert.equal(result.status, 'ready'); assert.equal(result.funding.collected, 3);
  assert.equal(f.client.sent.some(command => command.objectId === 99 && ['attack', 'attackV2'].includes(command.type)), false);
  assert.equal(result.fundingTargetRejections[0].targetId, 99);
  assert.equal(result.fundingTargetRejections[0].targetName, 'Scarecrow');
  assert.equal(result.fundingTargetRejections[0].phase, 'enqueue');
});

test('target loss cannot disguise owner safety changes or transport failures during pacing', async t => {
  for (const loss of ['owner-name', 'owner-map', 'owner-health', 'transport']) await t.test(loss, async () => {
    const f = fixture({ deer: 4, v2: true, targetLossAtPace: loss });
    await assert.rejects(f.supply.prepare(), /owner\/map\/health changed|transport closed|received owner\/map changed/);
    assert.equal(f.client.sent.filter(command => command.type === 'attackV2').length, 0);
    assert.equal(f.client.sent.some(command => command.type === 'buyItem'), false);
  });
});


test('exact canonical IDs match without permitting ambiguous or malformed target identities', async t => {
  const canonical = fixture({ deer: 3, v2: true, targetLossAtPace: 'canonical-id' });
  assert.equal((await canonical.supply.prepare()).status, 'ready');
  assert.ok(canonical.client.sent.some(command => command.type === 'attackV2' && command.objectId === 20));
  assert.equal(canonical.supply.status().fundingTargetRejectionCount, 0);
  for (const loss of ['malformed-id', 'ambiguous-id']) await t.test(loss, async () => {
    const f = fixture({ deer: 4, v2: true, targetLossAtPace: loss });
    assert.equal((await f.supply.prepare()).status, 'ready');
    assert.equal(f.client.sent.some(command => command.type === 'attackV2' && command.objectId === 20), false);
    assert.equal(f.supply.status().fundingTargetRejections[0].reason, loss === 'malformed-id' ? 'missing' : 'ambiguous-id');
  });
});


test('only boolean dead false confirms a live target after pacing; malformed or absent state cannot be hunted', async t => {
  for (const loss of ['dead-string', 'dead-one', 'dead-zero', 'dead-null', 'dead-missing', 'dead-false-string']) await t.test(loss, async () => {
    const f = fixture({ deer: 4, v2: true, targetLossAtPace: loss });
    assert.equal((await f.supply.prepare()).status, 'ready');
    assert.equal(f.client.sent.some(command => command.type === 'attackV2' && command.objectId === 20), false);
    const rejection = f.supply.status().fundingTargetRejections.find(row => row.targetId === 20 && row.phase === 'send');
    assert.equal(rejection.reason, 'not-confirmed-live');
    assert.equal(rejection.targetDead, null);
    assert.equal(f.client.sent.some(command => command.type === 'attack'), false);
  });
});
