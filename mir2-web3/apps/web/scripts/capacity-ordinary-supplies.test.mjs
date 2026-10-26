import test from 'node:test';
import assert from 'node:assert/strict';
import { createNavigator, NavigationStepGuarded } from './quest-agent/protocol-play.mjs';
import { applyProtocolObservation } from './quest-agent/protocol-observation.mjs';
import { selectPreparationStagingPoint } from './playtest-capacity-prepare.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';
import { createOrdinarySupplies, ORDINARY_SUPPLY_COMMANDS } from './capacity-ordinary-supplies.mjs';
import { loadOrdinaryProtection } from './capacity-ordinary-protection.mjs';

const route = { quests: [{ objectives: { item: [], kill: [{ monsterName: 'Deer', spawnCandidates: [{ mapFileName: '0', position: { x: 290, y: 604 }, count: 4 }] }] } }] };
const item = (id, quantity, uniqueId, slot = 0) => ({ itemIndex: id, name: id === 658 ? '(HP)DrugSmall' : 'Venison', quantity, uniqueId, container: 'bag1', slot, sellValue: 250 });
function fixture(config = {}) {
  let clock = 1, hook = () => {}, seq = 0, currentDeer = null;
  const harvestPasses = new Map();
  let attackPaceArmed = false, attackPaceChanged = false;
  const actor = { objectId: config.ownerId ?? 1, kind: 'selfPlayer', name: 'ordinary-owner', class: 'Warrior', x: config.ownerPosition?.x ?? 290, y: config.ownerPosition?.y ?? 607, hp: config.ownerHp ?? 100, maxHp: config.ownerMaxHp ?? 100, dead: false };
  const client = {
    label: config.label ?? 'ordinary-owner', sequence: 0, events: [], sent: [], slots: [], attackResults: config.v2 ? 'v2' : 'legacy', attackV2Supported: config.v2 === true, closed: false,
    snapshot: { playerObjectId: actor.objectId, mapFileName: '0', playerHp: actor.hp, playerMaxHp: actor.maxHp, gold: config.gold ?? 0, inventoryCapacity: 46,
      inventoryItems: config.items ?? [], beltItems: [], groundDrops: [], entities: [actor,
        { objectId: 6, kind: 'npc', name: 'Merchant_John', x: 292, y: 603 },
        { objectId: 5, kind: 'npc', name: 'Merchant_Ruben', x: 288, y: 608 },
        ...(config.nonDeerBlocker ? [{ objectId: 99, kind: 'monster', name: 'Scarecrow', x: 289, y: 605, hp: 25, dead: false }] : []),
        ...Array.from({ length: config.deer ?? 0 }, (_, i) => ({ objectId: 20 + i, kind: 'monster', name: 'Deer', x: config.deerPositions?.[i]?.x ?? (config.distantAlternatives && i>0 ? 310+i : 290+i), y: config.deerPositions?.[i]?.y ?? 604, hp: 1, dead: false, disposition: 'neutral' })), ...(config.hostiles ?? [])] },
    async pace() {
      config.onPace?.({ client:this, actor, snapshot, packet });
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
        const deltas = { Up:[0,-1], UpRight:[1,-1], Right:[1,0], DownRight:[1,1], Down:[0,1], DownLeft:[-1,1], Left:[-1,0], UpLeft:[-1,-1] };
        const delta = deltas[command.direction], stride = command.type === 'run' ? 2 : 1;
        Object.assign(actor, command.destination ?? { x:actor.x+delta[0]*stride, y:actor.y+delta[1]*stride });
        config.onMove?.({ client:this, actor, packet });
        packet('UserLocation', { objectId: actor.objectId, x: actor.x, y: actor.y });
        if (!config.noMovementSnapshots) snapshot();
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
    if (config.applyPackets) client.snapshot = applyProtocolObservation(client.snapshot,receipt);
    client.events.push(receipt); hook(receipt, receipt); return receipt;
  }
  function snapshot(sequence) {
    const message = { type: 'worldSnapshot', payload: structuredClone(client.snapshot) };
    const receipt = { ...message, direction: 'received', sequence: sequence ?? ++client.sequence, monotonicMs: ++clock };
    client.events.push(receipt); hook(message, receipt);
  }
  let preparation = true;
  const supply = createOrdinarySupplies(client, { preparationProfile: 'ordinary-economic-v1', targetHp: config.target ?? 12, route,
    stagingPoint: config.stagingPoint ?? { x: 290, y: 607 }, resolveStagingPoint: config.resolveStagingPoint, ordinaryProtection: config.ordinaryProtection, isPreparation: () => preparation, now: () => clock,
    sleep: async ms => { clock += ms; }, maxHunts: config.maxHunts ?? 8, timeoutMs: config.timeoutMs ?? 600_000,
    maxCommands: config.maxCommands ?? 2048,
    navigate: async (facade, target, range, stop, navOptions) => {
      config.inspectNavigation?.({target,range,navOptions});
      if (config.navigate) return config.navigate({ facade, client, actor, snapshot, packet, target, range, stop, navOptions });
      if (config.nonDeerBlocker && target.objectId === 20) throw new Error('No walk path to this Deer');
      if (config.navigationFailure) throw new Error('fixture blocked ordinary route');
      if (config.failPhase) preparation = false;
      const names = ['Up','UpRight','Right','DownRight','Down','DownLeft','Left','UpLeft'];
      const offsets = [[0,-1],[1,-1],[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1]];
      while (Math.max(Math.abs(actor.x-target.x),Math.abs(actor.y-target.y)) > range) {
        const dx = Math.sign(target.x-actor.x), dy = Math.sign(target.y-actor.y);
        const direction = names[offsets.findIndex(pair => pair[0]===dx&&pair[1]===dy)];
        const destination = { x:actor.x+dx, y:actor.y+dy };
        await facade.pace();
        const hazard = navOptions.beforeMovement({ from:{x:actor.x,y:actor.y}, physicalCells:[destination], movementType:'walk' });
        if (hazard) throw new Error('fixture path unsafe');
        facade.send({ type:'walk', direction });
        await facade.wait(() => actor.x===destination.x&&actor.y===destination.y, 'ordinary navigation');
      }
      if (config.targetLossAtPace && target.objectId === 20) attackPaceArmed = true;
    } });
  hook = supply.observe;
  return { client, supply, snapshot, packet, now:()=>clock, advance: ms => { clock += ms; }, endPreparation: () => { preparation = false; } };
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
  assert.ok(f.supply.status().commandRows.length <= 32);
  assert.equal(f.supply.status().commandRowsTruncated, f.supply.status().commands > 32);
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

test('a nearby non-Deer threat excludes the dangerous Deer before a bounded safe alternative hunt', async () => {
  const f = fixture({ deer: 4, v2: true, nonDeerBlocker: true, distantAlternatives: true, stagingPoint:{x:310,y:607} });
  const result = await f.supply.prepare();
  assert.equal(result.status, 'ready'); assert.equal(result.funding.collected, 3);
  assert.equal(f.client.sent.some(command => command.objectId === 99 && ['attack', 'attackV2'].includes(command.type)), false);
  assert.equal(result.fundingTargetRejections[0].targetId, 20);
  assert.equal(result.fundingTargetRejections[0].reason, 'hostile-near-deer');
  assert.equal(result.fundingTargetRejections[0].blocker.name, 'Scarecrow');
  assert.equal(result.fundingTargetRejections[0].phase, 'selection');
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


const hostile = (objectId,x,y,extra={}) => ({objectId,kind:'monster',name:'HookingCat',x,y,hp:30,dead:false,disposition:'hostile',ai:0,...extra});
function actualNavigatorFor(fixtureAccessor) {
  let navigator;
  return async ({facade,target,range,stop,navOptions}) => {
    const f=fixtureAccessor();
    navigator ??= createNavigator(facade,{ loadCollisionMap:async()=>({mapFileName:'0',width:360,height:660,blocked:new Uint8Array(360*660)}),
      now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000 });
    return navigator(target,range,stop,navOptions);
  };
}

test('all preparation routes retain at least eight hostile cells even when funding requests zero clearance', async()=>{
  const routes=[];
  const f=fixture({deer:3,v2:true,inspectNavigation:row=>routes.push(row)});
  assert.equal((await f.supply.prepare()).status,'ready');
  assert.ok(routes.some(row=>row.target.name==='Deer'));
  assert.ok(routes.some(row=>row.target.name==='Merchant_Ruben'));
  assert.ok(routes.some(row=>row.target.name==='Merchant_John'));
  assert.ok(routes.every(row=>row.navOptions.hostileAvoidanceRadius>=8&&row.navOptions.allowedHostileObjectIds.length===0));
  assert.ok(routes.every(row=>typeof row.navOptions.beforeMovement==='function'));
});

test('real navigator can leave the three-Scarecrow birth halos without a safe-zone exemption or extra refreshes',async()=>{
  const threats=[hostile(71,287,616,{name:'Scarecrow'}),hostile(72,293,614,{name:'Scarecrow'}),hostile(73,286,623,{name:'Scarecrow'})];
  let f;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:288,y:616},stagingPoint:{x:288,y:600},hostiles:threats,navigate:actualNavigatorFor(()=>f)});
  f.client.snapshot.inSafeZone=true;
  assert.equal((await f.supply.prepare()).status,'ready');
  assert.equal(f.client.sent.filter(command=>command.type==='clientVersion').length,2);
  assert.ok(f.client.sent.some(command=>['walk','run'].includes(command.type)));
  assert.equal(f.supply.status().pathHazardCount,0);
});

test('real navigator detours around the entire eight-cell halo, including unknown-AI hostile observations',async t=>{
  for(const unknown of [false,true])await t.test(String(unknown),async()=>{
    let f;
    const threat=hostile(71,300,607,unknown?{disposition:undefined,ai:undefined}:{});
    f=fixture({items:[item(658,12,500)],stagingPoint:{x:310,y:607},hostiles:[threat],navigate:actualNavigatorFor(()=>f)});
    assert.equal((await f.supply.prepare()).status,'ready');
    let position={x:290,y:607};
    const offsets={Up:[0,-1],UpRight:[1,-1],Right:[1,0],DownRight:[1,1],Down:[0,1],DownLeft:[-1,1],Left:[-1,0],UpLeft:[-1,-1]};
    for(const command of f.client.sent.filter(command=>['walk','run'].includes(command.type))){
      const [dx,dy]=offsets[command.direction];
      for(let step=0;step<(command.type==='run'?2:1);step++){
        position={x:position.x+dx,y:position.y+dy};
        assert.ok(Math.max(Math.abs(position.x-threat.x),Math.abs(position.y-threat.y))>8);
      }
    }
    assert.deepEqual(position,{x:310,y:607});
  });
});

test('a hostile arriving during queued Run pacing blocks its intermediate or landing cell before actual send',async t=>{
  for(const intermediate of [true,false])await t.test(String(intermediate),async()=>{
    let armed=false,changed=false;
    const f=fixture({items:[item(658,12,500)],stagingPoint:{x:292,y:607},
      onPace:({client,snapshot})=>{if(armed&&!changed){changed=true;client.snapshot.entities.push(hostile(71,intermediate?291:300,607));snapshot();}},
      navigate:async({facade})=>{armed=true;facade.send({type:'run',direction:'Right'});await facade.wait(()=>false,'queued ordinary Run');}});
    f.client.snapshot.inSafeZone=true;
    await assert.rejects(f.supply.prepare(),/unsafe hostile approach/);
    assert.equal(f.client.sent.some(command=>command.type==='run'),false);
    const row=f.supply.status().pathHazards[0];
    assert.equal(row.phase,'send');assert.equal(row.radius,8);assert.equal(row.physicalCells.length,2);
    assert.deepEqual(row.physicalCells,[{x:291,y:607},{x:292,y:607}]);
    assert.equal(row.blocker.objectId,71);
    assert.equal(f.supply.status().actualStagingReached,false);
  });
});

test('inside a halo ordinary movement may maintain or increase distance but cannot approach the threat',async t=>{
  for(const away of [true,false])await t.test(String(away),async()=>{
    const f=fixture({items:[item(658,12,500)],stagingPoint:{x:away?291:289,y:607},hostiles:[hostile(71,287,607)],
      navigate:async({facade})=>{facade.send({type:'walk',direction:away?'Right':'Left'});await facade.wait(()=>true,'ordinary halo exit');}});
    f.client.snapshot.inSafeZone=true;
    if(away){assert.equal((await f.supply.prepare()).status,'ready');assert.equal(f.client.sent.filter(c=>c.type==='walk').length,1);}
    else{await assert.rejects(f.supply.prepare(),/unsafe hostile approach/);assert.equal(f.client.sent.some(c=>c.type==='walk'),false);}
  });
});

test('all dangerous Deer exhaust the original eight hunt attempts without any attack or fake funding credit',async()=>{
  const f=fixture({deer:8,v2:true,hostiles:[hostile(71,295,604)]});
  await assert.rejects(f.supply.prepare(),/after 0 hunts and 8 attempts/);
  assert.equal(f.client.sent.some(c=>['attack','attackV2','harvest','buyItem'].includes(c.type)),false);
  assert.equal(f.supply.status().fundingTargetRejectionCount,8);
  assert.ok(f.supply.status().fundingTargetRejections.every(row=>row.reason==='hostile-near-deer'&&row.phase==='selection'));
  assert.equal(f.supply.status().funding,null);
  assert.ok(f.supply.status().commands<2048);
});

test('friendly, unclassified or owned Deer are not ordinary neutral funding targets',async t=>{
  for(const kind of ['friendly','unknown','owned'])await t.test(kind,async()=>{
    let f;f=fixture({deer:4,v2:true,distantAlternatives:true,stagingPoint:{x:310,y:607},navigate:actualNavigatorFor(()=>f)});const target=f.client.snapshot.entities.find(e=>e.objectId===20);
    if(kind==='friendly')target.disposition='friendly';
    if(kind==='unknown')delete target.disposition;
    if(kind==='owned')target.ownerName='another-player';
    assert.equal((await f.supply.prepare()).status,'ready');
    assert.equal(f.client.sent.some(c=>c.type==='attackV2'&&c.objectId===20),false);
    assert.equal(f.supply.status().fundingTargetRejections[0].reason,'not-neutral-deer');
  });
});

test('an owner transform changed during movement pacing cannot retarget an already queued direction',async()=>{
  let armed=false,changed=false;
  const f=fixture({items:[item(658,12,500)],stagingPoint:{x:291,y:607},
    onPace:({actor,snapshot})=>{if(armed&&!changed){changed=true;actor.y++;snapshot();}},
    navigate:async({facade})=>{armed=true;facade.send({type:'walk',direction:'Right'});await facade.wait(()=>false,'queued move');}});
  await assert.rejects(f.supply.prepare(),/unsafe hostile approach/);
  assert.equal(f.client.sent.some(c=>c.type==='walk'),false);
  assert.equal(f.supply.status().pathHazards[0].reason,'owner-transform-changed');
});


test('fresh danger during a real funding approach uses the existing finite target-loss path',async()=>{
  let f,armed=false,changed=false;
  const navigate=actualNavigatorFor(()=>f);
  f=fixture({deer:4,v2:true,distantAlternatives:true,stagingPoint:{x:310,y:607},
    onPace:({client,snapshot})=>{if(armed&&!changed){changed=true;client.snapshot.entities.push(hostile(71,290,606));snapshot();}},
    navigate:async context=>{if(context.target.objectId===20)armed=true;return navigate(context);}});
  assert.equal((await f.supply.prepare()).status,'ready');
  assert.equal(f.client.sent.some(c=>c.type==='attackV2'&&c.objectId===20),false);
  assert.ok(f.supply.status().pathHazardCount>=1);
  assert.equal(f.supply.status().funding.collected,3);
});

test('path hazard history is bounded at 32 scalar rows and unrelated navigation errors stay terminal',async()=>{
  const original=new Error('fixture no safe candidate');
  const f=fixture({items:[item(658,12,500)],hostiles:[hostile(71,300,607)],
    navigate:async({actor,navOptions})=>{for(let i=0;i<40;i++)assert.ok(navOptions.beforeMovement({
      movementType:'run',from:{x:actor.x,y:actor.y},physicalCells:[{x:291,y:607},{x:292,y:607}]}));throw original;}});
  await assert.rejects(f.supply.prepare(),error=>error===original);
  const result=f.supply.status();
  assert.equal(result.pathHazardCount,40);assert.equal(result.pathHazards.length,32);assert.equal(result.pathHazardsTruncated,true);
  assert.equal(f.client.sent.some(c=>['walk','run'].includes(c.type)),false);
  assert.equal(result.actualStagingReached,false);
});


test('nonempty unknown and malformed dispositions remain threats for Deer selection and queued Run dispatch',async t=>{
  for(const disposition of ['Unknown','unsupported','Neutral',0,false,{},['neutral']])await t.test(JSON.stringify(disposition),async()=>{
    const funding=fixture({deer:8,v2:true,hostiles:[hostile(71,295,604,{disposition})]});
    await assert.rejects(funding.supply.prepare(),/after 0 hunts and 8 attempts/);
    assert.equal(funding.client.sent.some(command=>['attack','attackV2','harvest'].includes(command.type)),false);
    assert.equal(funding.supply.status().fundingTargetRejections[0].reason,'hostile-near-deer');
    let armed=false,changed=false;
    const moving=fixture({items:[item(658,12,500)],stagingPoint:{x:292,y:607},
      onPace:({client,snapshot})=>{if(armed&&!changed){changed=true;client.snapshot.entities.push(hostile(71,300,607,{disposition}));snapshot();}},
      navigate:async({facade})=>{armed=true;facade.send({type:'run',direction:'Right'});await facade.wait(()=>false,'queued unknown-threat Run');}});
    await assert.rejects(moving.supply.prepare(),/unsafe hostile approach/);
    assert.equal(moving.client.sent.some(command=>command.type==='run'),false);
    assert.equal(moving.supply.status().pathHazards[0].phase,'send');
  });
});

test('only canonical neutral and friendly dispositions are exempt from the preparation threat halo',async t=>{
  for(const disposition of ['neutral','friendly'])await t.test(disposition,async()=>{
    const f=fixture({items:[item(658,12,500)],stagingPoint:{x:292,y:607},hostiles:[hostile(71,300,607,{disposition})],
      navigate:async({facade})=>{facade.send({type:'run',direction:'Right'});await facade.wait(()=>true,'ordinary nonhostile Run');}});
    assert.equal((await f.supply.prepare()).status,'ready');
    assert.equal(f.client.sent.filter(command=>command.type==='run').length,1);
    assert.equal(f.supply.status().pathHazardCount,0);
  });
});


test('canonical static protection admits a real three-Scarecrow return route without another refresh',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  const threats=[hostile(71,287,616,{name:'Scarecrow'}),hostile(72,293,614,{name:'Scarecrow'}),hostile(73,286,623,{name:'Scarecrow'})];
  let f;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:288,y:600},stagingPoint:{x:288,y:615},hostiles:threats,
    ordinaryProtection:protection,navigate:actualNavigatorFor(()=>f),
    onMove:({client,actor})=>{client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  f.client.snapshot.inSafeZone=false;
  const result=await f.supply.prepare();
  assert.equal(result.status,'ready');
  assert.deepEqual({x:result.after.x,y:result.after.y},{x:288,y:615});
  assert.equal(f.client.sent.filter(command=>command.type==='clientVersion').length,2);
  assert.ok(f.client.sent.some(command=>['walk','run'].includes(command.type)));
  assert.equal(result.pathHazardCount,0);
  assert.equal(result.protectionSummary.selectedMapMatches,true);
  assert.equal(result.protectionSummary.normalAi0SafeZoneOnly,true);
  assert.equal(result.protectionSummary.allDamageImmunity,false);
  assert.ok(Object.isFrozen(result.protectionSummary));
});

test('a protected Run landing never excuses an unprotected intermediate cell or the reverse',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  for(const entry of [true,false])await t.test(entry?'outside then inside':'inside then outside',async()=>{
    const from=entry?{x:277,y:604}:{x:297,y:607};
    const cells=entry?[{x:278,y:605},{x:279,y:606}]:[{x:298,y:606},{x:299,y:605}];
    const blocker=entry?hostile(71,285,606,{name:'Scarecrow'}):hostile(71,303,606,{name:'Scarecrow'});
    const f=fixture({items:[item(658,12,500)],ownerPosition:from,stagingPoint:cells[1],hostiles:[blocker],ordinaryProtection:protection,
      navigate:async({facade})=>{facade.send({type:'run',direction:entry?'DownRight':'UpRight'});await facade.wait(()=>true,'mixed-protection Run');}});
    f.client.snapshot.inSafeZone=true;
    await assert.rejects(f.supply.prepare(),/unsafe hostile approach/);
    assert.equal(f.client.sent.some(command=>command.type==='run'),false);
    assert.equal(f.supply.status().pathHazards[0].phase,'send');
    assert.deepEqual(f.supply.status().pathHazards[0].physicalCells,cells);
  });
});

test('static ordinary protection never admits occupied cells, unknown AI or owned monsters',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  for(const [label,extra,occupied] of [
    ['occupied',{name:'Scarecrow'},true],['unknown name',{name:'UnknownMonster'},false],
    ['non AI0',{name:'Scarecrow',ai:1},false],['malformed AI',{name:'Scarecrow',ai:'0'},false],
    ['owned',{name:'Scarecrow',ownerName:'someone'},false],['empty owner',{name:'Scarecrow',ownerName:''},false],
    ['unknown disposition',{name:'Scarecrow',disposition:'Unknown'},false],
  ])await t.test(label,async()=>{
    const f=fixture({items:[item(658,12,500)],ownerPosition:{x:288,y:614},stagingPoint:{x:288,y:615},
      hostiles:[hostile(71,288,occupied?615:616,extra)],ordinaryProtection:protection,
      navigate:async({facade})=>{facade.send({type:'walk',direction:'Down'});await facade.wait(()=>true,'protected ordinary Walk');}});
    f.client.snapshot.inSafeZone=true;
    await assert.rejects(f.supply.prepare(),/unsafe hostile approach/);
    assert.equal(f.client.sent.some(command=>command.type==='walk'),false);
    assert.equal(f.supply.status().pathHazards[0].phase,'send');
  });
});

test('missing or hand-made protection retains the original strict movement barrier',async t=>{
  for(const ordinaryProtection of [undefined,{mapFileName:'0',safeZones:[{location:{x:288,y:616},size:10}],ai0MonsterNames:['Scarecrow']}])await t.test(ordinaryProtection?'hand-made spec':'no spec',async()=>{
    const f=fixture({items:[item(658,12,500)],ownerPosition:{x:288,y:614},stagingPoint:{x:288,y:615},hostiles:[hostile(71,288,616,{name:'Scarecrow'})],ordinaryProtection,
      navigate:async({facade})=>{facade.send({type:'walk',direction:'Down'});await facade.wait(()=>true,'untrusted-protection Walk');}});
    f.client.snapshot.inSafeZone=true;
    await assert.rejects(f.supply.prepare(),/unsafe hostile approach/);
    assert.equal(f.client.sent.some(command=>command.type==='walk'),false);
    assert.equal(f.supply.status().protectionSummary,null);
  });
});


test('real ordinary navigation round-trips from village to Deer funding and back to merchants and staging',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  const threats=[hostile(71,287,616,{name:'Scarecrow'}),hostile(72,293,614,{name:'Scarecrow'}),hostile(73,286,623,{name:'Scarecrow'}),
    hostile(74,330,594,{name:'UnclassifiedThreat',ai:undefined})];
  let f;
  f=fixture({deer:3,v2:true,deerPositions:[{x:310,y:594},{x:312,y:594},{x:314,y:594}],ownerPosition:{x:288,y:615},stagingPoint:{x:288,y:615},
    hostiles:threats,ordinaryProtection:protection,navigate:actualNavigatorFor(()=>f),
    onMove:({client,actor})=>{client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  f.client.snapshot.inSafeZone=true;
  const result=await f.supply.prepare();
  assert.equal(result.status,'ready');assert.equal(result.funding.hunts,3,JSON.stringify(result.funding));assert.equal(result.funding.collected,3);
  assert.equal(result.purchased,12);assert.equal(result.soldVenison,2);assert.equal(result.goldEarned,500);assert.equal(result.after.venison,1);assert.equal(result.after.hpStock,12);
  assert.deepEqual({x:result.after.x,y:result.after.y},{x:288,y:615});
  assert.equal(f.client.sent.filter(c=>c.type==='attackV2').length,3);
  assert.ok(f.client.sent.some(c=>c.type==='interact'&&c.objectId===6));
  assert.ok(f.client.sent.some(c=>c.type==='interact'&&c.objectId===5));
  assert.ok(result.movementCommands>30);assert.ok(result.commands<2048);assert.ok(result.elapsedMs<600000);
  assert.equal(result.pathHazardCount,0);
});

test('protected planner obstacles are re-read using the live owner transform without bypassing final readiness',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  let checked=false;
  const f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:600},stagingPoint:{x:289,y:600},
    hostiles:[hostile(71,293,600,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({facade,navOptions})=>{
      const before=navOptions.forbiddenPoints;
      assert.ok(before.some(cell=>cell.x===291&&cell.y===600));
      facade.send({type:'walk',direction:'Left'});await facade.wait(()=>true,'ordinary halo withdrawal');
      const after=navOptions.forbiddenPoints;
      assert.ok(after.some(cell=>cell.x===290&&cell.y===600),'a former origin becomes a closer forbidden cell after withdrawal');
      assert.equal(before.some(cell=>cell.x===290&&cell.y===600),false);
      assert.equal(after.some(cell=>cell.x===293&&cell.y===600),true,'occupied tile remains forbidden');
      checked=true;
    }});
  await assert.rejects(f.supply.prepare(),/hostile near staging/);assert.equal(checked,true);
});

test('trusted Scarecrow AOI loss cannot restore a coarse halo over the protected289617 return target',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  let f,removed=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},
    hostiles:[hostile(206641,287,616,{name:'Scarecrow'}),hostile(206632,286,623,{name:'Scarecrow'})],
    ordinaryProtection:protection,navigate:actualNavigatorFor(()=>f),
    onMove:({client,actor})=>{if(!removed){removed=true;client.snapshot.entities=client.snapshot.entities.filter(e=>![206641,206632].includes(e.objectId));}
      client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  f.client.snapshot.inSafeZone=false;
  const result=await f.supply.prepare();
  assert.equal(result.status,'ready');assert.equal(removed,true);assert.equal(result.after.x,289);assert.equal(result.after.y,617);
  assert.equal(result.pathHazardCount,0);assert.ok(result.commands<2048);assert.ok(result.elapsedMs<600000);
});

test('trusted AOI memory rebuilds every recent outside-zone trail and occupied cell, not just the final point',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,checked=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:600},stagingPoint:{x:290,y:600},
    hostiles:[hostile(71,293,600,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({client,navOptions,snapshot})=>{
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      assert.ok(navOptions.forbiddenPoints.some(p=>p.x===293&&p.y===600));
      const crow=client.snapshot.entities.find(e=>e.objectId===71);crow.x=295;snapshot();
      void navOptions.allowedHostileObjectIds;
      client.snapshot.entities=client.snapshot.entities.filter(e=>e.objectId!==71);snapshot();
      const trail=navOptions.forbiddenPoints;
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      assert.ok(trail.some(p=>p.x===293&&p.y===600));assert.ok(trail.some(p=>p.x===295&&p.y===600));
      assert.ok(trail.some(p=>p.x===291&&p.y===600),'old outside-zone closer cells remain blocked');
      checked=true;
    }});
  assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

for(const [label,change] of [
  ['unknown name',e=>{e.name='UnknownMonster';}],['unknown disposition',e=>{delete e.disposition;}],
  ['owned',e=>{e.ownerName='someone';}],['empty owner',e=>{e.ownerName='';}],['non AI0',e=>{e.ai=1;}],
  ['malformed AI',e=>{e.ai='0';}],['foreign kind',e=>{e.kind='npc';}],['foreign class',e=>{e.class='Warrior';}],['neutral',e=>{e.disposition='neutral';}],
  ['dead',e=>{e.dead=true;e.hp=0;}],['unknown life',e=>{delete e.dead;}],
  ['duplicate ID',(e,client)=>{client.snapshot.entities.push({...e});}],
])test('trusted old ID is revoked immediately on '+label+' and cannot hide its old shared coarse memory',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let checked=false;
  const f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:607},stagingPoint:{x:290,y:607},
    hostiles:[hostile(71,293,614,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({client,navOptions,snapshot})=>{
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      const entity=client.snapshot.entities.find(e=>e.objectId===71);change(entity,client);
      assert.equal(navOptions.allowedHostileObjectIds.includes(71),false,'live all-entity scan revokes without waiting for a raw snapshot');
      client.snapshot.entities=client.snapshot.entities.filter(e=>e.objectId!==71);snapshot();
      assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);checked=true;
    }});
  assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

test('unknown hostile memory remains a complete shared coarse barrier after the observed entity leaves AOI',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,warmed=false;
  const go=actualNavigatorFor(()=>f);
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},
    hostiles:[hostile(71,287,616,{name:'UnknownMonster',ai:undefined})],ordinaryProtection:protection,
    navigate:async args=>{if(!warmed){await go({...args,target:{x:304,y:612},range:0});warmed=true;
      args.client.snapshot.entities=args.client.snapshot.entities.filter(e=>e.objectId!==71);args.snapshot();}
      assert.equal(args.navOptions.allowedHostileObjectIds.includes(71),false);return go(args);}});
  await assert.rejects(f.supply.prepare(),/No walk path/);assert.equal(warmed,true);
  assert.equal(f.supply.status().status,'failed');assert.equal(f.supply.status().actualStagingReached,false);
});

test('fresh raw receipts renew trust beyond20s but repeated getters alone never renew that qualification',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,checked=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:607},stagingPoint:{x:290,y:607},
    hostiles:[hostile(71,293,614,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({navOptions,snapshot})=>{
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      for(let i=0;i<21;i++){f.advance(1000);void navOptions.forbiddenPoints;void navOptions.allowedHostileObjectIds;}
      assert.equal(navOptions.allowedHostileObjectIds.includes(71),false,'getter reads cannot authenticate an old snapshot forever');
      snapshot();assert.ok(navOptions.allowedHostileObjectIds.includes(71),'new raw receipt renews qualification');
      f.client.snapshot.entities=f.client.snapshot.entities.filter(e=>e.objectId!==71);snapshot();
      f.advance(20001);assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);checked=true;
    }});
  assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

test('a quarantined ID cannot regain an exemption before old unknown shared trails have expired',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,checked=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:607},stagingPoint:{x:290,y:607},
    hostiles:[hostile(71,293,614,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({client,navOptions,snapshot})=>{
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));const e=client.snapshot.entities.find(e=>e.objectId===71);
      e.ownerName='someone';snapshot();assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      e.ownerName=null;snapshot();assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      f.advance(19998);assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      f.advance(2);snapshot();assert.ok(navOptions.allowedHostileObjectIds.includes(71));client.snapshot.entities=client.snapshot.entities.filter(e=>e.objectId!==71);snapshot();checked=true;
    }});
  assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

test('normal moving navigation longer than20s keeps genuine receipt-qualified protection without resetting budgets',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:350,y:593},stagingPoint:{x:289,y:617},
    hostiles:[hostile(71,287,616,{name:'Scarecrow'})],ordinaryProtection:protection,navigate:actualNavigatorFor(()=>f),
    onMove:({client,actor})=>{client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.ok(result.elapsedMs>20000);
  assert.ok(result.elapsedMs<600000);assert.ok(result.commands<2048);assert.equal(result.pathHazardCount,0);
});

test('trusted trail overflow revokes the corresponding allowed ID and falls back to shared coarse protection',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,checked=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:607},stagingPoint:{x:290,y:607},
    hostiles:[hostile(71,1000,600,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({client,navOptions,snapshot})=>{
      const entity=client.snapshot.entities.find(e=>e.objectId===71);
      for(let i=0;i<2049;i++){entity.x=1000+i;snapshot();void navOptions.allowedHostileObjectIds;}
      assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      const memory=f.supply.status().plannerThreatMemory;assert.ok(memory.coarseFallbackCount>0);assert.ok(memory.positions<=memory.positionLimit);checked=true;
    }});
  assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

test('real canonical loader map and navigator return304613 to289617 after the two observed Crow IDs leave AOI',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  assert.equal(protocolMapCellIsWalkable(map,{x:304,y:613}),true);assert.equal(protocolMapCellIsWalkable(map,{x:289,y:617}),true);
  let f,navigator,removed=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},
    hostiles:[hostile(206641,287,616,{name:'Scarecrow'}),hostile(206632,286,623,{name:'Scarecrow'}),hostile(206644,293,614,{name:'Scarecrow'}),
      {objectId:4,kind:'npc',name:'CraftsLady_Jude',x:294,y:619,dead:false},{objectId:55,kind:'npc',name:'Blacksmith_Smith',x:296,y:613,dead:false},
      {objectId:42,kind:'npc',name:'Merchant_Scott',x:291,y:610,dead:false}],ordinaryProtection:protection,
    navigate:async({facade,target,range,stop,navOptions})=>{navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});return navigator(target,range,stop,navOptions);},
    onMove:({client,actor})=>{if(!removed){removed=true;client.snapshot.entities=client.snapshot.entities.filter(e=>![206641,206632].includes(e.objectId));}
      client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.equal(removed,true);
  assert.deepEqual({x:result.after.x,y:result.after.y},{x:289,y:617});assert.equal(result.pathHazardCount,0);
  assert.equal(result.after.inSafeZone,true);assert.ok(result.commands<2048);assert.ok(result.elapsedMs<600000);
});

test('forbidden-before-allowed reads crossing quarantine expiry retain the complete outside-zone halo',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,checked=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:600},stagingPoint:{x:290,y:600},
    hostiles:[hostile(71,293,600,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({client,navOptions,snapshot})=>{
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));const e=client.snapshot.entities.find(e=>e.objectId===71);
      e.ownerName='someone';snapshot();const blockedAt=f.now();assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      e.ownerName=null;snapshot();assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      f.advance(blockedAt+19999-f.now());const forbidden=navOptions.forbiddenPoints;
      assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      f.advance(1);assert.equal(navOptions.allowedHostileObjectIds.includes(71),false,'a later getter cannot enlarge the earlier planning view');
      void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(71),'a new coherent planner round may observe the expiry');
      assert.ok(forbidden.some(p=>p.x===291&&p.y===600),'earlier forbidden view already contains outside halo');
      assert.ok(forbidden.some(p=>p.x===293&&p.y===600),'remembered occupied cell is never omitted');
      client.snapshot.entities=client.snapshot.entities.filter(e=>e.objectId!==71);snapshot();checked=true;
    }});
  assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

test('ID-cap overflow remains globally quarantined when slots free before an unknown shared trail expires',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,checked=false;
  const ids=Array.from({length:1024},(_,i)=>hostile(5000+i,1000+i,600,{name:'Scarecrow'}));
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:290,y:607},stagingPoint:{x:290,y:607},hostiles:ids,ordinaryProtection:protection,
    navigate:async({client,navOptions,snapshot})=>{
      void navOptions.allowedHostileObjectIds;const base=f.now();
      client.snapshot.entities=client.snapshot.entities.filter(e=>e.kind!=='monster');
      client.snapshot.entities.push(hostile(9999,287,616,{name:'UnknownMonster',ai:undefined}));
      f.advance(1000);snapshot();void navOptions.allowedHostileObjectIds;const overflowAt=f.now();
      assert.equal(f.supply.status().plannerThreatMemory.exemptionGloballyQuarantined,true);
      f.advance(base+20001-f.now());const target=client.snapshot.entities.find(e=>e.objectId===9999);target.name='Scarecrow';target.ai=0;snapshot();
      assert.equal(navOptions.allowedHostileObjectIds.includes(9999),false,'free identity slots cannot erase the remaining unknown-trail horizon');
      f.advance(overflowAt+19999-f.now());void navOptions.forbiddenPoints;
      f.advance(1);assert.equal(navOptions.allowedHostileObjectIds.includes(9999),false,'expiry crossing cannot enlarge an earlier forbidden view');
      void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(9999));
      assert.ok(f.supply.status().plannerThreatMemory.identities<=1024);
      client.snapshot.entities=client.snapshot.entities.filter(e=>e.objectId!==9999);snapshot();checked=true;
    }});
  assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

test('own hostile guards before movement and queued dispatch safely replan without replaying the unsafe command',async t=>{
  for(const phase of ['before-movement','send'])await t.test(phase,async()=>{
    let f,calls=0,armed=false,firstOptions,firstTarget,firstStop;
    const threat=hostile(206039,325,583,{name:'RakingCat',hp:undefined,disposition:undefined});
    const go=actualNavigatorFor(()=>f);
    f=fixture({items:[item(658,12,500)],ownerPosition:{x:331,y:590},stagingPoint:{x:289,y:617},
      noMovementSnapshots:true,applyPackets:true,
      onPace:({client})=>{if(armed){armed=false;client.snapshot.entities.push(threat);}},
      navigate:async context=>{
        calls++;
        if(calls===1){
          firstOptions=context.navOptions;firstTarget=context.target;firstStop=context.stop;
          if(phase==='send'){armed=true;context.facade.send({type:'run',direction:'Up'});await context.facade.wait(()=>false,'guarded queued run');}
          else {
            context.client.snapshot.entities.push(threat);
            const hazard=context.navOptions.beforeMovement({movementType:'run',from:{x:331,y:590},physicalCells:[{x:331,y:589},{x:331,y:588}]});
            assert.ok(hazard);throw new NavigationStepGuarded(hazard);
          }
        }
        assert.equal(context.navOptions,firstOptions);assert.equal(context.target,firstTarget);assert.equal(context.stop,firstStop);
        return go(context);
      }});
    f.client.snapshot.inSafeZone=true;
    const result=await f.supply.prepare();
    assert.equal(result.status,'ready');assert.equal(calls,2);assert.equal(result.pathReplanCount,1);
    assert.equal(result.pathReplans[0].phase,phase);assert.equal(result.pathReplans[0].movementCommands,0);
    assert.equal(result.pathHazardCount,1);assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,2);
    let from={x:331,y:590};const delta={Up:[0,-1],UpRight:[1,-1],Right:[1,0],DownRight:[1,1],Down:[0,1],DownLeft:[-1,1],Left:[-1,0],UpLeft:[-1,-1]};
    for(const c of f.client.sent.filter(c=>['run','walk'].includes(c.type))){
      const [dx,dy]=delta[c.direction],prior=distanceForTest(from,threat);
      for(let i=1;i<=(c.type==='run'?2:1);i++){
        const next={x:from.x+dx*i,y:from.y+dy*i},d=distanceForTest(next,threat);
        assert.ok(d>0&&(prior<=8?d>=prior:d>8),'no guarded or newly unsafe physical cell was dispatched');
      }
      const n=c.type==='run'?2:1;from={x:from.x+dx*n,y:from.y+dy*n};
    }
    assert.deepEqual(from,{x:289,y:617});assert.ok(result.commands<2048&&result.elapsedMs<600000);
  });
});
const distanceForTest=(a,b)=>Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y));

test('replans are at most three per actor and do not reset the wall or command budget',async t=>{
  for(const limit of ['replans','wall','commands'])await t.test(limit,async()=>{
    let f,calls=0;
    f=fixture({items:[item(658,12,500)],stagingPoint:{x:292,y:607},hostiles:[hostile(71,300,607)],
      timeoutMs:limit==='wall'?500:600000,maxCommands:limit==='commands'?1:2048,
      navigate:async({facade,actor,navOptions})=>{
        calls++;
        if(limit==='wall')f.advance(300);
        if(limit==='commands'&&calls===2){facade.send({type:'keepAlive'});await facade.wait(()=>true,'retry budget');}
        const hazard=navOptions.beforeMovement({movementType:'run',from:{x:actor.x,y:actor.y},physicalCells:[{x:291,y:607},{x:292,y:607}]});
        throw new NavigationStepGuarded(hazard);
      }});
    await assert.rejects(f.supply.prepare(),limit==='replans'?/Navigation step guarded/:limit==='wall'?/wall budget/:/command budget/);
    assert.equal(f.supply.status().pathReplanCount,limit==='replans'?3:1);
    assert.equal(calls,limit==='replans'?4:2);assert.equal(f.client.sent.length,1);
    assert.equal(f.supply.status().actualStagingReached,false);
  });
});

test('a queued side effect or changed HP map transport is never cleared to make a send guard retryable',async t=>{
  for(const fault of ['queued','hp','map','transport'])await t.test(fault,async()=>{
    let armed=false,calls=0;
    const f=fixture({items:[item(658,12,500)],stagingPoint:{x:292,y:607},
      onPace:({client,actor})=>{if(armed){armed=false;client.snapshot.entities.push(hostile(71,300,607));
        if(fault==='hp'){actor.hp=10;client.snapshot.playerHp=10;}
        if(fault==='map')client.snapshot.mapFileName='2';if(fault==='transport')client.closed=true;}},
      navigate:async({facade})=>{calls++;armed=true;facade.send({type:'run',direction:'Right'});
        if(fault==='queued')facade.send({type:'turn',direction:'Right'});
        await facade.wait(()=>false,'unsafe queued move');}});
    await assert.rejects(f.supply.prepare());assert.equal(calls,1);assert.equal(f.supply.status().pathReplanCount,0);
    assert.equal(f.client.sent.some(c=>['run','turn'].includes(c.type)),false);
  });
});

test('raw-certified individual position receipts renew only their own identity beyond20s without new raw snapshots',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f;
  f=fixture({items:[item(658,12,500)],stagingPoint:{x:290,y:607},applyPackets:true,ordinaryProtection:protection,
    hostiles:[hostile(71,293,600,{name:'Scarecrow'}),hostile(72,1000,600,{name:'Scarecrow'})],
    navigate:async({client,navOptions,packet})=>{
      const rawCount=client.events.filter(e=>e.type==='worldSnapshot').length;
      for(let i=0;i<25;i++){
        f.advance(1000);packet(['ObjectWalk','ObjectRun','ObjectTurn'][i%3],{objectId:71,location:{x:293+i%2,y:600},direction:0});
        void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      }
      assert.equal(navOptions.allowedHostileObjectIds.includes(72),false);
      assert.equal(client.events.filter(e=>e.type==='worldSnapshot').length,rawCount);
      assert.ok(navOptions.forbiddenPoints.some(p=>p.x===293&&p.y===600));
      assert.ok(navOptions.forbiddenPoints.some(p=>p.x===294&&p.y===600));
      assert.equal(f.supply.status().plannerThreatMemory.packetRenewals,25);
    }});
  f.client.snapshot.inSafeZone=true;assert.equal((await f.supply.prepare()).status,'ready');
});

const wireMonster=(id=71,extra={})=>({objectId:id,name:'Scarecrow',location:{x:287,y:616},ai:0,dead:false,masterObjectId:0,...extra});
test('actual ObjectMonster shape withoutHP or disposition supports bounded individual receipts and cached AOI history',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f;
  f=fixture({items:[item(658,12,500)],applyPackets:true,ordinaryProtection:protection,
    navigate:async({client,navOptions,packet})=>{
      packet('ObjectMonster',wireMonster(71,{location:{x:293,y:600}}));
      const e=client.snapshot.entities.find(e=>e.objectId===71);
      assert.equal(Object.hasOwn(e,'hp'),false);assert.equal(Object.hasOwn(e,'disposition'),false);
      const rawCount=client.events.filter(e=>e.type==='worldSnapshot').length;
      for(let i=0;i<25;i++){
        f.advance(1000);packet('ObjectTurn',{objectId:71,location:{x:293+i%2,y:600},direction:0});
        void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      }
      const count=f.supply.status().plannerThreatMemory.packetRenewals;
      packet('ObjectRemove',{objectId:71});assert.equal(client.snapshot.entities.some(e=>e.objectId===71),false);
      for(let i=0;i<19;i++){f.advance(1000);packet('UserLocation',{location:{x:290,y:607}});packet('UseItem',{success:true});void navOptions.forbiddenPoints;}
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      assert.ok(navOptions.forbiddenPoints.some(p=>p.x===293&&p.y===600));
      f.advance(1001);assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
      assert.equal(f.supply.status().plannerThreatMemory.packetRenewals,count);
      assert.equal(client.events.filter(e=>e.type==='worldSnapshot').length,rawCount);
    }});
  f.client.snapshot.inSafeZone=true;assert.equal((await f.supply.prepare()).status,'ready');
});

test('real canonical map navigation handles real ObjectRemove AOI departure without recreating the protected target coarse barrier',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  let f,navigator,removed=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},applyPackets:true,noMovementSnapshots:true,
    hostiles:[hostile(206641,287,616,{name:'Scarecrow'}),hostile(206632,286,623,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({facade,target,range,stop,navOptions})=>{navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});return navigator(target,range,stop,navOptions);},
    onMove:({client,actor,packet})=>{if(!removed){removed=true;packet('ObjectRemove',{objectId:206641});packet('ObjectRemove',{objectId:206632});}
      client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.equal(removed,true);
  assert.deepEqual({x:result.after.x,y:result.after.y},{x:289,y:617});assert.equal(result.pathHazardCount,0);
  assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,2);
});

test('real incremental lifecycle and identity conflicts revoke normal history and quarantine sameID reuse',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  for(const [name,payload] of [
    ['ObjectName',{objectId:71,name:'UnknownMonster'}],['ObjectPlayer',{objectId:71,name:'player',location:{x:287,y:616}}],
    ['ObjectRevived',{objectId:71}],['ObjectDied',{objectId:71}],
    ['ObjectMonster',wireMonster(71,{masterObjectId:987})],['ObjectMonster',wireMonster(71,{ai:1})],
    ['ObjectMonster',wireMonster(71,{name:'UnknownMonster'})],['ObjectMonster',wireMonster(71,{class:'Warrior'})],
    ['ObjectMonster',wireMonster(71,{disposition:'friendly'})],
  ])await t.test(name+JSON.stringify(payload),async()=>{
    let checked=false;
    const f=fixture({items:[item(658,12,500)],applyPackets:true,ordinaryProtection:protection,
      navigate:async({client,navOptions,packet})=>{
        packet('ObjectMonster',wireMonster());void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(71));
        packet('ObjectRemove',{objectId:71});packet(name,payload);
        assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
        assert.equal(f.supply.status().plannerThreatMemory.positions,0);
        packet('ObjectRemove',{objectId:71});packet('ObjectMonster',wireMonster());
        assert.equal(navOptions.allowedHostileObjectIds.includes(71),false,'unknown old shared trails cannot be hidden by a new normal packet');
        client.snapshot.entities=client.snapshot.entities.filter(e=>e.objectId!==71);checked=true;
      }});
    f.client.snapshot.inSafeZone=true;assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
  });
});

test('self ACK duplicate receipts and getters do not renew a wire identity, and position packets cannot reopen an expired or removed live segment',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  for(const removed of [false,true])await t.test(String(removed),async()=>{
    let f;
    f=fixture({items:[item(658,12,500)],applyPackets:true,ordinaryProtection:protection,
      navigate:async({client,navOptions,packet})=>{
        packet('ObjectMonster',wireMonster());
        const receipt=packet('ObjectTurn',{objectId:71,location:{x:287,y:616},direction:0});
        const renewals=f.supply.status().plannerThreatMemory.packetRenewals;
        if(removed)packet('ObjectRemove',{objectId:71});
        for(let i=0;i<21;i++){
          f.advance(1000);packet('UserLocation',{location:{x:290,y:607}});packet('UseItem',{success:true});
          f.supply.observe(receipt,receipt);void navOptions.forbiddenPoints;
        }
        assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
        packet('ObjectTurn',{objectId:71,location:{x:287,y:616},direction:0});
        assert.equal(navOptions.allowedHostileObjectIds.includes(71),false);
        assert.equal(f.supply.status().plannerThreatMemory.packetRenewals,renewals);
        client.snapshot.entities=client.snapshot.entities.filter(e=>e.objectId!==71);
      }});
    f.client.snapshot.inSafeZone=true;assert.equal((await f.supply.prepare()).status,'ready');
  });
});

test('normal ObjectRemove reentry gains a new live segment without delaying matching normal proofs or dropping old outside trails',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  const f=fixture({items:[item(658,12,500)],applyPackets:true,ordinaryProtection:protection,
    navigate:async({client,navOptions,packet,snapshot})=>{
      packet('ObjectMonster',wireMonster(71,{location:{x:293,y:600}}));void navOptions.forbiddenPoints;
      packet('ObjectRemove',{objectId:71});
      packet('ObjectMonster',wireMonster(71,{location:{x:295,y:600}}));
      const trail=navOptions.forbiddenPoints;
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));
      assert.ok(trail.some(p=>p.x===293&&p.y===600));assert.ok(trail.some(p=>p.x===295&&p.y===600));
      const e=client.snapshot.entities.find(e=>e.objectId===71);Object.assign(e,{hp:20,disposition:'hostile'});
      packet('ObjectRemove',{objectId:71});client.snapshot.entities.push(e);snapshot();
      void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(71),'matching complete raw proof can also reopen the live segment');
    }});
  f.client.snapshot.inSafeZone=true;assert.equal((await f.supply.prepare()).status,'ready');
});

test('normal real ObjectRemove then ObjectMonster reentry preserves the canonical return route without fresh-per-step snapshots',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  let f,navigator,reentered=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},applyPackets:true,noMovementSnapshots:true,
    hostiles:[hostile(206641,287,616,{name:'Scarecrow'}),hostile(206632,286,623,{name:'Scarecrow'})],ordinaryProtection:protection,
    navigate:async({facade,target,range,stop,navOptions})=>{navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});return navigator(target,range,stop,navOptions);},
    onMove:({client,actor,packet})=>{if(!reentered){reentered=true;
      packet('ObjectRemove',{objectId:206641});packet('ObjectRemove',{objectId:206632});
      packet('ObjectMonster',wireMonster(206641));packet('ObjectMonster',wireMonster(206632,{location:{x:286,y:623}}));}
      client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.equal(reentered,true);
  assert.deepEqual({x:result.after.x,y:result.after.y},{x:289,y:617});assert.equal(result.pathHazardCount,0);
  assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,2);
});

test('quiet normal history retains only its original planner position lifetime across real AOI departure and still revokes unknown reuse',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  for(const unknown of [false,true])await t.test(String(unknown),async()=>{
    let f,navigator,warmed=false,departed=false,base;
    f=fixture({items:[item(658,12,500)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},applyPackets:true,noMovementSnapshots:true,
      hostiles:[hostile(206641,287,616,{name:'Scarecrow'}),hostile(206632,286,623,{name:'Scarecrow'})],ordinaryProtection:protection,
      navigate:async({facade,target,range,stop,navOptions,packet})=>{
        navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});
        base=f.now();f.advance(15000);
        await navigator({x:304,y:612},0,stop,navOptions);warmed=true;
        assert.ok(f.now()>=base+21000&&f.now()<base+35000);
        if(unknown)packet('ObjectMonster',wireMonster(206641,{name:'UnknownMonster'}));
        void navOptions.forbiddenPoints;
        assert.equal(navOptions.allowedHostileObjectIds.includes(206641),!unknown,
          'closed normal history must cover shared positions after raw trust expires; unknown never receives that exemption');
        return navigator(target,range,stop,navOptions);
      },
      onMove:({client,actor,packet})=>{if(!departed){departed=true;
        packet('ObjectRemove',{objectId:206641});packet('ObjectRemove',{objectId:206632});
        f.advance(base+21000-f.now());}
        client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
    if(unknown)await assert.rejects(f.supply.prepare(),/No walk path/);
    else{
      const result=await f.supply.prepare();assert.equal(result.status,'ready');
      assert.deepEqual({x:result.after.x,y:result.after.y},{x:289,y:617});
      assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,2);
    }
    assert.equal(warmed,true);assert.equal(departed,true);
  });
});

test('actual raw null owner and class followed by wire omission reopens normal AOI without extra quarantine',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  let f,navigator,options,reentered=false;
  f=fixture({items:[item(658,12,500)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},applyPackets:true,noMovementSnapshots:true,
    hostiles:[hostile(206641,287,616,{name:'Scarecrow',ownerName:null,class:null}),hostile(206632,286,623,{name:'Scarecrow',ownerName:null,class:null})],
    ordinaryProtection:protection,
    navigate:async({facade,target,range,stop,navOptions})=>{options=navOptions;
      navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});
      return navigator(target,range,stop,navOptions);},
    onMove:({client,actor,packet})=>{if(!reentered){reentered=true;
      packet('ObjectRemove',{objectId:206641});packet('ObjectRemove',{objectId:206632});
      packet('ObjectMonster',wireMonster(206641));packet('ObjectMonster',wireMonster(206632,{location:{x:286,y:623}}));
      for(const id of [206641,206632]){
        const entity=client.snapshot.entities.find(e=>e.objectId===id);
        assert.equal(Object.hasOwn(entity,'ownerName'),false);assert.equal(Object.hasOwn(entity,'class'),false);
        assert.ok(options.allowedHostileObjectIds.includes(id),'wire omission and raw null denote the same absent optional value');
      }}
      client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.equal(reentered,true);
  assert.deepEqual({x:result.after.x,y:result.after.y},{x:289,y:617});assert.equal(result.pathHazardCount,0);
  assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,2);
});

// Bounded public W6 recording frame21, capturedAtMs1793054598595: only
// monster IDs/coordinates/state within16 of the failed actor's331590 start.
// This is a static replay of that observation, not a prediction of live AI.
const w6NearbyThreats = [
  [206008,'RakingCat',327,574,32],[206026,'RakingCat',320,589,32],[206032,'RakingCat',322,595,32],
  [206033,'RakingCat',333,578,32],[206039,'RakingCat',325,583,32],[206105,'HookingCat',315,578,30],
  [206117,'HookingCat',321,588,30],[206124,'HookingCat',340,577,30],[206129,'HookingCat',342,600,30],
  [206130,'HookingCat',327,582,30],[206136,'HookingCat',338,588,30],
].map(([id,name,x,y,hp])=>hostile(id,x,y,{name,hp,ownerName:null,class:null}));
const w6NearbyNeutral = [
  [200341,'Royal_Guard',328,604,null,6],[206201,'Deer',325,600,25,2],[206208,'Deer',322,587,25,2],
  [206214,'Deer',343,592,25,2],[206215,'Deer',347,576,25,2],[206220,'Deer',326,599,25,2],
  [206221,'Deer',331,581,25,2],[206227,'Deer',340,586,25,2],[206542,'Deer',330,597,25,2],
  [206703,'Deer',342,606,25,2],[206713,'Deer',343,586,25,2],[206716,'Deer',342,592,25,2],
  [206719,'Deer',345,599,25,2],[210400,'ChestnutTree',344,591,10,3],[214501,'ChestnutTree2',321,598,10,3],
  [214502,'ChestnutTree2',345,580,10,3],[229100,'LargeMushroom',317,577,null,3],
].map(([objectId,name,x,y,hp,ai])=>({objectId,kind:'monster',name,x,y,hp,ai,dead:false,disposition:'neutral',ownerName:null,class:null}));

test('W6 cold331590 static observed threats can approach the declared region on real map0 before the actual strict resolver selects a point',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  const defaultPoint={x:289,y:615},selected={x:290,y:615},candidates=[defaultPoint,selected],savedPoints=new Set(['289,615']);
  let f,navigator,resolverCalls=0;const routes=[];
  f=fixture({ownerId:1000,ownerHp:18,ownerMaxHp:18,items:[item(658,12,2)],gold:15,
    ownerPosition:{x:331,y:590},stagingPoint:defaultPoint,ordinaryProtection:protection,applyPackets:true,noMovementSnapshots:true,
    hostiles:structuredClone([...w6NearbyThreats,...w6NearbyNeutral]),
    resolveStagingPoint:input=>{
      resolverCalls++;assert.equal(routes.length,1);assert.equal(f.supply.status().status,'staging');
      assert.equal(f.supply.status().actualStagingReached,false);
      assert.equal(f.client.events.filter(e=>e.type==='worldSnapshot').length,2,'approach must request new raw occupancy');
      assert.equal(input.receipt.sequence,f.client.sequence);assert.equal(input.receipt.x,input.snapshot.entities[0].x);
      assert.ok(distanceForTest(input.receipt,defaultPoint)<=16);assert.ok(distanceForTest(input.receipt,defaultPoint)>0);
      return selectPreparationStagingPoint({...input,candidates,savedPoints});
    },
    navigate:async({facade,target,range,stop,navOptions})=>{
      routes.push({target:{...target},range});
      navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});
      return navigator(target,range,stop,navOptions);
    },onMove:({client,actor})=>{client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  f.client.snapshot.inSafeZone=false;const owner=f.client.snapshot.entities[0];
  assert.throws(()=>selectPreparationStagingPoint({snapshot:f.client.snapshot,receipt:{ownerId:1000,ownerName:owner.name,map:'0',x:331,y:590,sequence:1,monotonicMs:1},defaultPoint,candidates,savedPoints}),/No fresh visible/);
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.equal(resolverCalls,1);
  assert.equal(result.before.nearbyHostileCount,3);assert.equal(result.before.hp,18);assert.equal(result.before.hpStock,12);assert.equal(result.before.gold,15);
  assert.deepEqual(routes,[{target:defaultPoint,range:16},{target:selected,range:0}]);
  assert.deepEqual(result.resolvedStagingPoint,selected);assert.equal(result.after.hp,18);assert.equal(result.after.hpStock,12);assert.equal(result.after.gold,15);
  assert.equal(result.purchased,0);assert.equal(result.soldVenison,0);assert.equal(result.funding,null);
  assert.ok(result.commands<2048&&result.elapsedMs<600000);assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,3);
  assert.ok(result.movementCommands>0);assert.equal(savedPoints.size,1,'helper never claims or saves a candidate');
  t.diagnostic(JSON.stringify({scope:'W6 frame21 static nearby monster replay on canonical map0; not live AI prediction',
    commands:result.commands,movementCommands:result.movementCommands,elapsedMs:result.elapsedMs,pathReplanCount:result.pathReplanCount,
    resolverCalls,finalPoint:result.resolvedStagingPoint}));
});

test('nearby strict resolver rejection never approaches or retries, while an unsuccessful distant approach still requires fresh visible occupancy',async t=>{
  for(const far of [false,true])await t.test(String(far),async()=>{
    let f,calls=0;const routes=[],defaultPoint={x:289,y:615},savedPoints=new Set(far?[]:['289,615']);
    f=fixture({items:[item(658,12,2)],ownerPosition:far?{x:331,y:590}:{x:290,y:615},stagingPoint:defaultPoint,
      navigate:async({target,range})=>{routes.push({target,range});return {reached:false};},
      resolveStagingPoint:input=>{calls++;if(far)assert.equal(f.client.events.filter(e=>e.type==='worldSnapshot').length,2);
        return selectPreparationStagingPoint({...input,candidates:[defaultPoint],savedPoints});}});
    await assert.rejects(f.supply.prepare(),/No fresh visible unoccupied distinct declared staging candidate/);
    assert.equal(calls,1);assert.equal(routes.length,far?1:0);
    if(far)assert.equal(routes[0].range,16);
    assert.equal(f.supply.status().actualStagingReached,false);assert.equal(f.supply.status().resolvedStagingPoint,null);
    assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,far?2:1);
    assert.equal(f.supply.status().movementCommands,0);
  });
});

test('the approach cannot reset the actor wall or command budget or invoke the resolver after exhaustion',async t=>{
  for(const exhausted of ['wall','commands'])await t.test(exhausted,async()=>{
    let f,resolverCalls=0;
    f=fixture({items:[item(658,12,2)],ownerPosition:{x:331,y:590},stagingPoint:{x:289,y:615},
      timeoutMs:exhausted==='wall'?500:600000,maxCommands:exhausted==='commands'?1:2048,
      navigate:async({facade,range})=>{
        assert.equal(range,16);assert.equal(f.supply.status().status,'staging');
        if(exhausted==='wall')f.advance(501);
        facade.send({type:'walk',direction:'Down'});await facade.wait(()=>true,'approach budget');
      },resolveStagingPoint:({defaultPoint})=>{resolverCalls++;return defaultPoint;}});
    await assert.rejects(f.supply.prepare(),exhausted==='wall'?/wall budget exhausted/:/command budget exhausted/);
    assert.equal(resolverCalls,0);assert.equal(f.client.sent.length,1);assert.equal(f.supply.status().movementCommands,0);
    assert.equal(f.supply.status().actualStagingReached,false);
  });
});

// WorldEntitySnapshot (simulation/config.rs): nullable owner/class/HP/AI,
// and no wire-only masterObjectId. These are two distinct source DTO shapes.
const rawNormalCrow=(objectId=206644,x=293,y=614)=>({objectId,kind:'monster',name:'Scarecrow',ownerName:null,ai:0,x,y,
  direction:'Down',class:null,gender:null,level:null,hp:20,maxHp:20,light:0,nameColourArgb:-1,dead:false,disposition:'hostile'});
test('a real wire normal certificate transitions to independent fullraw ownership and health without a wire master field',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let checked=false;
  const f=fixture({items:[item(658,12,2)],applyPackets:true,ordinaryProtection:protection,
    navigate:async({client,packet,navOptions,snapshot})=>{
      packet('ObjectMonster',wireMonster(206644,{location:{x:293,y:614}}));
      void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(206644));
      const raw=rawNormalCrow();assert.equal(Object.hasOwn(raw,'masterObjectId'),false);
      client.snapshot.entities=client.snapshot.entities.map(e=>e.objectId===206644?raw:e);snapshot();
      void navOptions.forbiddenPoints;
      assert.ok(navOptions.allowedHostileObjectIds.includes(206644),'a missing wire-only field is not a raw ownership conflict');
      packet('ObjectTurn',{objectId:206644,location:{x:293,y:614},direction:0});
      void navOptions.forbiddenPoints;assert.ok(navOptions.allowedHostileObjectIds.includes(206644));checked=true;
    }});
  f.client.snapshot.inSafeZone=true;assert.equal((await f.supply.prepare()).status,'ready');assert.equal(checked,true);
});

test('W7 final static raw NPC and monster positions admit a canonical map0 route from305612 to289617',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  let f,navigator;
  const npcs=[[4,'CraftsLady_Jude',294,619],[5,'Blacksmith_Smith',296,613],[6,'Merchant_John',292,603],
    [7,'Merchant_Whitney',305,608],[42,'Merchant_Scott',291,610],[43,'MaterialDealer_Reece',295,605]]
    .map(([objectId,name,x,y])=>({objectId,kind:'npc',name,x,y,dead:false,ownerName:null,class:null}));
  const neutral=[[200325,'ArcherGuard',295,624,9999,57],[200716,'Guard',291,620,9999,6],[206324,'Deer',289,604,25,2],
    [206524,'Deer',294,606,25,2],[206536,'Deer',291,600,25,2],[271801,'Hen',291,616,5,1],[271806,'Hen',292,613,5,1]]
    .map(([objectId,name,x,y,hp,ai])=>({objectId,kind:'monster',name,x,y,hp,ai,dead:false,disposition:'neutral',ownerName:null,class:null}));
  f=fixture({ownerId:1000,ownerHp:18,ownerMaxHp:18,items:[item(658,12,2)],gold:15,ownerPosition:{x:305,y:612},stagingPoint:{x:289,y:617},
    ordinaryProtection:protection,applyPackets:true,noMovementSnapshots:true,
    hostiles:[rawNormalCrow(),rawNormalCrow(206610,300,597),hostile(206013,301,596,{name:'RakingCat',hp:32,ownerName:null,class:null}),...neutral],
    navigate:async({facade,target,range,stop,navOptions})=>{navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});return navigator(target,range,stop,navOptions);},
    onMove:({client,actor})=>{client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  f.client.snapshot.entities=f.client.snapshot.entities.filter(e=>e.kind!=='npc').concat(npcs);
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.equal(result.pathHazardCount,0);
  t.diagnostic(JSON.stringify({scope:'W7 frame85 static local replay, excludes unrecorded client trail history',commands:result.commands,
    movementCommands:result.movementCommands,elapsedMs:result.elapsedMs,finalPoint:{x:result.after.x,y:result.after.y}}));
});

test('failed ordinary status preserves bounded actual qualification receipts and the exact shared NoPath cache without changing commands',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let f,original;
  const go=actualNavigatorFor(()=>f);
  f=fixture({items:[item(658,12,2)],ownerPosition:{x:304,y:613},stagingPoint:{x:289,y:617},ordinaryProtection:protection,applyPackets:true,
    hostiles:[hostile(71,287,616,{name:'UnknownMonster',ai:undefined})],
    navigate:async args=>{
      for(let i=0;i<40;i++)args.snapshot();
      void args.navOptions.forbiddenPoints;
      const before=f.supply.status().qualificationEventCount;
      for(let i=0;i<40;i++){void args.navOptions.forbiddenPoints;void args.navOptions.allowedHostileObjectIds;}
      assert.equal(f.supply.status().qualificationEventCount,before,'same planner-only revocation is deduplicated');
      args.packet('ObjectTurn',{objectId:71,x:287,y:616,direction:'Down',credential:'fixture-do-not-retain'});
      return go(args);
    }});
  await assert.rejects(f.supply.prepare(),error=>{original=error;return /No walk path/.test(error.message);});
  const result=f.supply.status(),diagnostic=result.navigationNoPath;
  assert.equal(result.status,'failed');assert.equal(result.actualStagingReached,false);
  assert.ok(result.qualificationEventCount>32);assert.equal(result.qualificationEvents.length,32);assert.equal(result.qualificationEventsTruncated,true);
  assert.ok(result.qualificationRepeatedReads>0);
  const event=result.qualificationEvents.find(e=>e.source==='ObjectTurn');
  assert.ok(event&&event.sequence>0&&event.monotonicMs>0);assert.equal(event.input.x,287);assert.equal(event.input.hpPresent,false);
  assert.equal(diagnostic.shared,original.navigationDiagnostics);assert.equal(diagnostic.shared.source,'shared-navigator-exact-cache');
  const memory=diagnostic.shared.hostileMemory.find(e=>e.objectId===71),state=diagnostic.qualifications.find(e=>e.objectId===71);
  assert.equal(memory.allowed,false);assert.equal(memory.currentlyVisible,true);assert.equal(memory.x,287);assert.equal(memory.y,616);
  assert.equal(state.qualification,'none');assert.ok(state.blockedRemainingMs>0);assert.equal(state.lastReason,'planner-current-not-qualified');
  assert.equal(state.current.name,'UnknownMonster');assert.equal(state.current.aiPresent,true);assert.equal(state.current.ai,null);
  assert.equal(result.movementCommands,0);assert.equal(result.commands,4);assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,4);
  assert.equal(JSON.stringify(result).includes('fixture-do-not-retain'),false);
  assert.equal(Object.isFrozen(diagnostic),true);assert.equal(Object.isFrozen(state),true);
});

test('throwing error message getters and conversions cannot replace the original ordinary navigation error',async t=>{
  for(const mode of ['getter','conversion'])await t.test(mode,async()=>{
    const original=new Error('original fixture navigation error'),secondary=new Error('fixture message inspection failed');
    if(mode==='getter')Object.defineProperty(original,'message',{get(){throw secondary;}});
    else Object.defineProperty(original,'message',{value:{toString(){throw secondary;}}});
    const f=fixture({items:[item(658,12,2)],navigate:async()=>{throw original;}});
    await assert.rejects(f.supply.prepare(),error=>error===original);
    const result=f.supply.status();assert.equal(result.status,'failed');assert.equal(result.navigationNoPath,null);
    assert.equal(result.failure,'ordinary supplies: underlying ordinary operation failed');
    assert.equal(result.commands,1);assert.equal(result.movementCommands,0);assert.equal(result.pathReplanCount,0);
  });
});
