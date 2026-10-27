import test from 'node:test';
import assert from 'node:assert/strict';
import { createNavigator, NavigationStepGuarded, NavigationStalled, isTerminalNavigationNoPath } from './quest-agent/protocol-play.mjs';
import { applyProtocolObservation } from './quest-agent/protocol-observation.mjs';
import { selectPreparationStagingPoint } from './playtest-capacity-prepare.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable, planProtocolNavigation } from './quest-agent/protocol-navigation.mjs';
import { createOrdinarySupplies, ORDINARY_SUPPLY_COMMANDS, ordinaryPreparationClearanceGeometry } from './capacity-ordinary-supplies.mjs';
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
    snapshot: { playerObjectId: actor.objectId, mapFileName: '0', playerHp: actor.hp, playerMaxHp: actor.maxHp, gold: config.gold ?? 0, inventoryCapacity: 46, mapTransfers:config.mapTransfers??[],
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
    config.onSnapshot?.({client,actor,packet});
    const message = { type: 'worldSnapshot', payload: structuredClone(client.snapshot) };
    const receipt = { ...message, direction: 'received', sequence: sequence ?? ++client.sequence, monotonicMs: ++clock };
    client.events.push(receipt); hook(message, receipt);
  }
  let preparation = true;
  const supply = createOrdinarySupplies(client, { preparationProfile: 'ordinary-economic-v1', targetHp: config.target ?? 12, route,
    stagingPoint: config.stagingPoint ?? { x: 290, y: 607 }, ...(config.fundingNavigationMap===undefined?{}:{fundingNavigationMap:config.fundingNavigationMap}), resolveStagingPoint: config.resolveStagingPoint, ordinaryProtection: config.ordinaryProtection, isPreparation: () => preparation, now: () => clock,
    sleep: async ms => { config.onSleep?.({client,actor,ms,clock,packet}); clock += ms; }, maxHunts: config.maxHunts ?? 8, maxEmptySearchRefreshes:config.maxEmptySearchRefreshes ?? 20, timeoutMs: config.timeoutMs ?? 600_000,
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

const w8Positions = [
  [
    206644,
    "scarecrow",
    293,
    614
  ],
  [
    206610,
    "scarecrow",
    300,
    597
  ],
  [
    206013,
    "rakingcat",
    301,
    596
  ],
  [
    206026,
    "rakingcat",
    320,
    589
  ],
  [
    206117,
    "hookingcat",
    321,
    588
  ],
  [
    206623,
    "scarecrow",
    321,
    631
  ],
  [
    206032,
    "rakingcat",
    322,
    595
  ],
  [
    206039,
    "rakingcat",
    325,
    583
  ],
  [
    206130,
    "hookingcat",
    327,
    582
  ],
  [
    206105,
    "hookingcat",
    315,
    578
  ],
  [
    206008,
    "rakingcat",
    327,
    574
  ],
  [
    206033,
    "rakingcat",
    333,
    578
  ],
  [
    206136,
    "hookingcat",
    337,
    589
  ],
  [
    206136,
    "hookingcat",
    338,
    588
  ],
  [
    206124,
    "hookingcat",
    340,
    577
  ],
  [
    206129,
    "hookingcat",
    342,
    600
  ],
  [
    206136,
    "hookingcat",
    339,
    587
  ]
];
const w8Occupancy = [{"x":320,"y":589,"objectId":206026,"kind":"monster"},{"x":321,"y":588,"objectId":206117,"kind":"monster"},{"x":321,"y":598,"objectId":214501,"kind":"monster"},{"x":322,"y":595,"objectId":206032,"kind":"monster"},{"x":322,"y":587,"objectId":206208,"kind":"monster"},{"x":325,"y":583,"objectId":206039,"kind":"monster"},{"x":325,"y":600,"objectId":206201,"kind":"monster"},{"x":326,"y":599,"objectId":206220,"kind":"monster"},{"x":327,"y":582,"objectId":206130,"kind":"monster"},{"x":315,"y":578,"objectId":206105,"kind":"monster"},{"x":330,"y":597,"objectId":206542,"kind":"monster"},{"x":331,"y":581,"objectId":206221,"kind":"monster"},{"x":327,"y":574,"objectId":206008,"kind":"monster"},{"x":333,"y":578,"objectId":206033,"kind":"monster"},{"x":337,"y":589,"objectId":206136,"kind":"monster"},{"x":340,"y":577,"objectId":206124,"kind":"monster"},{"x":340,"y":586,"objectId":206227,"kind":"monster"},{"x":342,"y":600,"objectId":206129,"kind":"monster"},{"x":342,"y":606,"objectId":206703,"kind":"monster"},{"x":342,"y":592,"objectId":206716,"kind":"monster"},{"x":343,"y":592,"objectId":206214,"kind":"monster"},{"x":343,"y":586,"objectId":206713,"kind":"monster"},{"x":344,"y":591,"objectId":210400,"kind":"monster"},{"x":345,"y":599,"objectId":206719,"kind":"monster"},{"x":345,"y":580,"objectId":214502,"kind":"monster"},{"x":347,"y":576,"objectId":206215,"kind":"monster"}];
const w8Transfers = [{"mapFileName":"0","toMapFileName":"0122","bounds":{"maxX":347,"maxY":188,"minX":347,"minY":188}},{"mapFileName":"0","toMapFileName":"0102","bounds":{"maxX":308,"maxY":264,"minX":308,"minY":264}},{"mapFileName":"0","toMapFileName":"0108","bounds":{"maxX":326,"maxY":288,"minX":326,"minY":288}},{"mapFileName":"0","toMapFileName":"0104","bounds":{"maxX":322,"maxY":247,"minX":322,"minY":247}},{"mapFileName":"0","toMapFileName":"0107","bounds":{"maxX":354,"maxY":319,"minX":354,"minY":319}},{"mapFileName":"0","toMapFileName":"0125","bounds":{"maxX":295,"maxY":251,"minX":295,"minY":251}},{"mapFileName":"0","toMapFileName":"0125","bounds":{"maxX":295,"maxY":250,"minX":295,"minY":250}},{"mapFileName":"0","toMapFileName":"0103","bounds":{"maxX":335,"maxY":299,"minX":335,"minY":299}},{"mapFileName":"0","toMapFileName":"0103","bounds":{"maxX":335,"maxY":300,"minX":335,"minY":300}},{"mapFileName":"0","toMapFileName":"0103","bounds":{"maxX":344,"maxY":308,"minX":344,"minY":308}},{"mapFileName":"0","toMapFileName":"0103","bounds":{"maxX":345,"maxY":308,"minX":345,"minY":308}},{"mapFileName":"0","toMapFileName":"0105","bounds":{"maxX":295,"maxY":284,"minX":295,"minY":284}},{"mapFileName":"0","toMapFileName":"0105","bounds":{"maxX":295,"maxY":285,"minX":295,"minY":285}},{"mapFileName":"0","toMapFileName":"0105","bounds":{"maxX":305,"maxY":276,"minX":305,"minY":276}},{"mapFileName":"0","toMapFileName":"0106","bounds":{"maxX":307,"maxY":311,"minX":307,"minY":311}},{"mapFileName":"0","toMapFileName":"0106","bounds":{"maxX":307,"maxY":310,"minX":307,"minY":310}},{"mapFileName":"0","toMapFileName":"0106","bounds":{"maxX":317,"maxY":302,"minX":317,"minY":302}},{"mapFileName":"0","toMapFileName":"0106","bounds":{"maxX":317,"maxY":301,"minX":317,"minY":301}},{"mapFileName":"0","toMapFileName":"0101","bounds":{"maxX":289,"maxY":292,"minX":289,"minY":292}},{"mapFileName":"0","toMapFileName":"0101","bounds":{"maxX":286,"maxY":296,"minX":286,"minY":296}},{"mapFileName":"0","toMapFileName":"0128","bounds":{"maxX":278,"maxY":239,"minX":278,"minY":239}},{"mapFileName":"0","toMapFileName":"0129","bounds":{"maxX":284,"maxY":236,"minX":284,"minY":236}},{"mapFileName":"0","toMapFileName":"0123","bounds":{"maxX":271,"maxY":256,"minX":271,"minY":256}},{"mapFileName":"0","toMapFileName":"0124","bounds":{"maxX":283,"maxY":268,"minX":283,"minY":268}},{"mapFileName":"0","toMapFileName":"0110","bounds":{"maxX":396,"maxY":230,"minX":396,"minY":230}},{"mapFileName":"0","toMapFileName":"0110","bounds":{"maxX":396,"maxY":229,"minX":396,"minY":229}},{"mapFileName":"0","toMapFileName":"0111","bounds":{"maxX":400,"maxY":225,"minX":400,"minY":225}},{"mapFileName":"0","toMapFileName":"0111","bounds":{"maxX":399,"maxY":225,"minX":399,"minY":225}},{"mapFileName":"0","toMapFileName":"0112","bounds":{"maxX":404,"maxY":221,"minX":404,"minY":221}},{"mapFileName":"0","toMapFileName":"0112","bounds":{"maxX":403,"maxY":221,"minX":403,"minY":221}},{"mapFileName":"0","toMapFileName":"0137","bounds":{"maxX":414,"maxY":243,"minX":414,"minY":243}},{"mapFileName":"0","toMapFileName":"0137","bounds":{"maxX":415,"maxY":241,"minX":415,"minY":241}},{"mapFileName":"0","toMapFileName":"0137","bounds":{"maxX":416,"maxY":241,"minX":416,"minY":241}},{"mapFileName":"0","toMapFileName":"0121","bounds":{"maxX":315,"maxY":613,"minX":315,"minY":613}},{"mapFileName":"0","toMapFileName":"0130","bounds":{"maxX":266,"maxY":630,"minX":266,"minY":630}},{"mapFileName":"0","toMapFileName":"0131","bounds":{"maxX":271,"maxY":621,"minX":271,"minY":621}},{"mapFileName":"0","toMapFileName":"0136","bounds":{"maxX":295,"maxY":649,"minX":295,"minY":649}},{"mapFileName":"0","toMapFileName":"0132","bounds":{"maxX":283,"maxY":636,"minX":283,"minY":636}},{"mapFileName":"0","toMapFileName":"0132","bounds":{"maxX":282,"maxY":636,"minX":282,"minY":636}},{"mapFileName":"0","toMapFileName":"0132","bounds":{"maxX":282,"maxY":635,"minX":282,"minY":635}},{"mapFileName":"0","toMapFileName":"0138","bounds":{"maxX":274,"maxY":636,"minX":274,"minY":636}},{"mapFileName":"0","toMapFileName":"0141","bounds":{"maxX":302,"maxY":622,"minX":302,"minY":622}},{"mapFileName":"0","toMapFileName":"0141","bounds":{"maxX":311,"maxY":631,"minX":311,"minY":631}},{"mapFileName":"0","toMapFileName":"0140","bounds":{"maxX":307,"maxY":627,"minX":307,"minY":627}},{"mapFileName":"0","toMapFileName":"0115","bounds":{"maxX":314,"maxY":474,"minX":314,"minY":474}},{"mapFileName":"0","toMapFileName":"D011","bounds":{"maxX":43,"maxY":111,"minX":43,"minY":111}},{"mapFileName":"0","toMapFileName":"D401","bounds":{"maxX":664,"maxY":215,"minX":664,"minY":215}},{"mapFileName":"0","toMapFileName":"D401","bounds":{"maxX":663,"maxY":215,"minX":663,"minY":215}},{"mapFileName":"0","toMapFileName":"1","bounds":{"maxX":324,"maxY":41,"minX":324,"minY":41}},{"mapFileName":"0","toMapFileName":"1","bounds":{"maxX":325,"maxY":42,"minX":325,"minY":42}},{"mapFileName":"0","toMapFileName":"1","bounds":{"maxX":326,"maxY":43,"minX":326,"minY":43}},{"mapFileName":"0","toMapFileName":"1","bounds":{"maxX":326,"maxY":42,"minX":326,"minY":42}},{"mapFileName":"0","toMapFileName":"1","bounds":{"maxX":325,"maxY":41,"minX":325,"minY":41}},{"mapFileName":"0","toMapFileName":"1","bounds":{"maxX":325,"maxY":40,"minX":325,"minY":40}},{"mapFileName":"0","toMapFileName":"1","bounds":{"maxX":326,"maxY":40,"minX":326,"minY":40}},{"mapFileName":"0","toMapFileName":"2","bounds":{"maxX":666,"maxY":86,"minX":666,"minY":86}},{"mapFileName":"0","toMapFileName":"2","bounds":{"maxX":667,"maxY":87,"minX":667,"minY":87}},{"mapFileName":"0","toMapFileName":"2","bounds":{"maxX":667,"maxY":88,"minX":667,"minY":88}},{"mapFileName":"0","toMapFileName":"HELL00","bounds":{"maxX":316,"maxY":649,"minX":316,"minY":649}},{"mapFileName":"0","toMapFileName":"HELL00","bounds":{"maxX":315,"maxY":650,"minX":315,"minY":650}},{"mapFileName":"0","toMapFileName":"HELL00","bounds":{"maxX":316,"maxY":650,"minX":316,"minY":650}},{"mapFileName":"0","toMapFileName":"HELL00","bounds":{"maxX":315,"maxY":649,"minX":315,"minY":649}},{"mapFileName":"0","toMapFileName":"HELL00","bounds":{"maxX":315,"maxY":648,"minX":315,"minY":648}},{"mapFileName":"0","toMapFileName":"HELL00","bounds":{"maxX":314,"maxY":649,"minX":314,"minY":649}},{"mapFileName":"0","toMapFileName":"0127","bounds":{"maxX":310,"maxY":600,"minX":310,"minY":600}},{"mapFileName":"0","toMapFileName":"0101","bounds":{"maxX":286,"maxY":296,"minX":286,"minY":296}},{"mapFileName":"0","toMapFileName":"0122","bounds":{"maxX":347,"maxY":189,"minX":347,"minY":189}},{"mapFileName":"0","toMapFileName":"D001","bounds":{"maxX":147,"maxY":33,"minX":147,"minY":33}},{"mapFileName":"0","toMapFileName":"D001","bounds":{"maxX":148,"maxY":34,"minX":148,"minY":34}}];

function w8Forbidden(positions,start={x:331,y:590},halfHeight=8) {
  const points=new Map();
  for(const [objectId,name,x,y] of positions)for(let cy=y-8;cy<=y+8;cy++)for(let cx=x-8;cx<=x+8;cx++){
    const d=Math.max(Math.abs(cx-x),Math.abs(cy-y)),before=Math.max(Math.abs(start.x-x),Math.abs(start.y-y));
    if(d!==0&&cx>=278&&cx<=298&&cy>=606&&cy<=626)continue;
    const oldMetric=Math.max(Math.abs(start.x-x)*halfHeight,Math.abs(start.y-y)*8);
    const metric=Math.max(Math.abs(cx-x)*halfHeight,Math.abs(cy-y)*8),boundary=8*halfHeight;
    if(d===0||(oldMetric<=boundary?metric<oldMetric||d<before:metric<=boundary))points.set(cx+','+cy,{x:cx,y:cy});
  }
  return [...points.values()];
}
test('W8 exact17-point2140-cell history with26occupancy and69transfers has a path only after the necessary old halos expire',async()=>{
  const map=await loadProtocolCollisionMap('0'),start={x:331,y:590},target={x:289,y:617};
  const transfers=w8Transfers.flatMap(t=>{const p=[];for(let y=t.bounds.minY;y<=t.bounds.maxY;y++)for(let x=t.bounds.minX;x<=t.bounds.maxX;x++)p.push({x,y});return p;});
  assert.equal(transfers.length,69);assert.equal(w8Occupancy.length,26);assert.equal(w8Positions.length,17);
  const run=(removed=[],occupancy=w8Occupancy,doors=transfers)=>{
    const forbidden=w8Forbidden(w8Positions.filter(p=>!removed.includes(p[0])));
    return {count:forbidden.length,path:planProtocolNavigation({map,start,target,desiredDistance:0,dynamicObstacles:[...forbidden,...occupancy,...doors]})};
  };
  assert.equal(run().count,2140);assert.equal(run().path,null);
  assert.equal(run([],[]).path,null);assert.equal(run([],w8Occupancy,[]).path,null);
  assert.equal(run([206013]).path,null);assert.equal(run([206610]).path,null);
  const both=run([206013,206610]);assert.equal(both.count,1818);assert.equal(both.path.steps.length,73);
  const all=run([206013,206610,206644,206623]);assert.equal(all.count,1477);assert.equal(all.path.steps.length,56);
});

async function w8HistoryFixture(config={}) {
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),sourceMap=await loadProtocolCollisionMap('0');
  // Expiry remains independently testable after the source-backed rectangle
  // removes W8's original artificial square wall. This explicitly synthetic
  // single-entry barrier is NOT a claim about the canonical map's geometry.
  const map={...sourceMap,blocked:sourceMap.blocked.slice()};
  for(let y=0;y<map.height;y++)if(y!==615)map.blocked[y*map.width+299]=1;
  const names={scarecrow:'Scarecrow',rakingcat:'RakingCat',hookingcat:'HookingCat'};
  const closed=new Set([206013,206610,206644,206623]),unique=new Map(w8Positions.map(p=>[p[0],p]));
  const current=w8Occupancy.map(p=>{
    const threat=unique.get(p.objectId),prior=w6NearbyNeutral.find(e=>e.objectId===p.objectId);
    return threat?hostile(p.objectId,p.x,p.y,{name:names[threat[1]],ownerName:null,class:null}):
      {...prior,...p,dead:false,disposition:'neutral',ownerName:null,class:null};
  });
  const old=w8Positions.filter(p=>closed.has(p[0])).map(([id,name,x,y])=>hostile(id,x,y,{name:names[name],ownerName:null,class:null}));
  // Replay source DTOs and real cache reads. The static selected observations
  // are deliberately not claimed to be a second live run or simultaneous AOI.
  let f,navigator,stage=0,expiry=0,movesBeforeWait=0,sleeps=0,firstTerminal,hazards=0;
  f=fixture({ownerId:1000,ownerHp:18,ownerMaxHp:18,items:[item(658,12,2)],gold:15,
    ownerPosition:{x:331,y:590},stagingPoint:{x:289,y:617},ordinaryProtection:protection,
    hostiles:[...current,...old],applyPackets:true,noMovementSnapshots:true,
    timeoutMs:config.timeoutMs??600000,maxCommands:config.maxCommands??2048,
    onSleep:({ms,clock,client,actor,packet})=>{
      if(stage===2){
        sleeps++;assert.ok(ms<=250);assert.equal(client.sent.filter(c=>['walk','run'].includes(c.type)).length,movesBeforeWait);
        assert.equal(actor.hp,18);assert.ok(clock<expiry);
        assert.equal(f.supply.status().plannerThreatMemory.positions,17,'waiting never clears the retained trail');
        config.duringWait?.({client,actor,packet,f,clock});
      }
    },
    onMove:({client,actor})=>{client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;},
    navigate:async args=>{
      navigator??=createNavigator(args.facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});
      // Do not spread accessors: preserve the actual forbidden-before-allowed
      // read order and the real per-planning-read cache view.
      const nav=()=>navigator(args.target,args.range,args.stop,new Proxy(args.navOptions,{get:(o,k)=>k==='maxNoPathRefreshes'?0:Reflect.get(o,k)}));
      if(stage===0){
        args.packet('ObjectMonster',{objectId:206136,name:'HookingCat',location:{x:339,y:587},ai:0,masterObjectId:0,dead:false});
        args.packet('ObjectWalk',{objectId:206136,x:338,y:588,direction:7});
        await assert.rejects(nav(),e=>isTerminalNavigationNoPath(e));
        // Mirror the unequal W8 history ages: do not let a minimum-expiry
        // implementation pass by giving all four points the same lifetime.
        if(!config.liveClosed)args.packet('ObjectRemove',{objectId:206013});
        for(const [delay,id] of [[32,206610],[2131,206644],[3776,206623]]){
          f.advance(delay);await assert.rejects(nav(),isTerminalNavigationNoPath);
          expiry=f.now()+20000;
          if(!config.liveClosed)args.packet('ObjectRemove',{objectId:id});
        }
        f.advance(11500);
        args.packet('ObjectWalk',{objectId:206136,x:337,y:589,direction:5});
        args.client.snapshot.entities=args.client.snapshot.entities.map(e=>e.objectId===206136?
          hostile(206136,337,589,{name:'HookingCat',ownerName:null,class:null}):e);
        args.snapshot();stage=1;
      }
      if(stage===1&&hazards<(config.hazards??0)){
        hazards++;
        const hazard=args.navOptions.beforeMovement({movementType:'run',from:{x:331,y:590},physicalCells:[{x:331,y:589},{x:331,y:588}]});
        assert.equal(hazard.reason,'hostile-clearance');throw new NavigationStepGuarded(hazard);
      }
      if(stage===1){
        stage=2;movesBeforeWait=args.client.sent.filter(c=>['walk','run'].includes(c.type)).length;
        try {await nav();assert.fail('old exact halos must still block');}catch(error){
          firstTerminal=error;assert.equal(error.navigationDiagnostics.explicitForbiddenTotal,w8Forbidden(w8Positions,{x:331,y:590},6).length);
          assert.equal(error.navigationDiagnostics.hostileMemoryTotal,16);
          assert.equal(error.navigationDiagnostics.occupancyTotal,config.liveClosed?30:26);
          assert.equal(f.supply.status().plannerThreatMemory.positions,17);
          if(config.fake)throw Object.assign(new Error(error.message),{navigationDiagnostics:error.navigationDiagnostics});
          throw error;
        }
      }
      assert.ok(f.now()>=expiry,'no route retry before the original last closed expiry');
      if(config.repeatTerminal)throw firstTerminal;
      stage=3;return nav();
    }});
  f.client.snapshot.entities=f.client.snapshot.entities.filter(e=>e.kind!=='npc');f.client.snapshot.mapTransfers=structuredClone(w8Transfers);
  return {f,metrics:()=>({sleeps,firstTerminal,expiry,stage})};
}
test('staging waits for original unequal history expiry on an explicitly synthetic single-entry map, then freshraw and same target',async()=>{
  const {f,metrics}=await w8HistoryFixture();const result=await f.supply.prepare();
  assert.equal(result.status,'ready');assert.ok(metrics().sleeps>1);assert.equal(result.pathReplanCount,1);
  assert.equal(result.pathReplans[0].reason,'closed-history-natural-expiry');assert.equal(result.pathReplans[0].historyPoints,4);
  assert.ok(result.pathReplans[0].waitMs>8000&&result.pathReplans[0].waitMs<9000);
  const waited=result.pathReplans[0];
  assert.equal(waited.history.length,4);assert.equal(waited.waitMs,Math.max(...waited.history.map(p=>p.remainingMs)));
  assert.ok(Math.min(...waited.history.map(p=>p.remainingMs))<3000,'original unequal history expiries remain inspectable');
  assert.deepEqual(waited.history.map(p=>p.objectId).sort(),[206013,206610,206623,206644]);
  assert.ok(waited.history.every(p=>Number.isInteger(p.x)&&Number.isInteger(p.y)&&p.remainingMs>0&&p.remainingMs<=20000));
  assert.equal(waited.actualPassiveWaitMs,waited.waitMs);assert.equal(waited.waitCompleted,true);assert.equal(waited.naturalExpiryReached,true);
  assert.ok(waited.freshSnapshotSequence>waited.snapshotSequence);assert.equal(waited.commandsAfterRefresh,waited.commands+1);
  assert.ok(f.client.events.some(e=>e.type==='worldSnapshot'&&e.sequence===waited.freshSnapshotSequence));
  const retainedHistory=structuredClone(waited.history);waited.history[0].remainingMs=0;waited.history.pop();
  assert.deepEqual(f.supply.status().pathReplans[0].history,retainedHistory,'status never exposes mutable internal history evidence');
  assert.deepEqual(result.pathReplans[0].target,{x:289,y:617});assert.equal(result.after.hp,18);
  assert.equal(result.after.hpStock,12);assert.equal(result.after.gold,15);assert.equal(result.purchased,0);assert.equal(result.funding,null);
  assert.ok(!f.client.sent.some(c=>['useItem','attack','attackV2','buyItem','sellItem'].includes(c.type)));
  assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,3,'initial, one post-expiry refresh, final readiness');
  assert.ok(result.commands<2048&&result.elapsedMs<600000);
});
test('a copied NoPath diagnostic is never a recovery authorization',async()=>{
  const {f,metrics}=await w8HistoryFixture({fake:true});await assert.rejects(f.supply.prepare(),/No walk path/);
  assert.equal(metrics().sleeps,0);assert.equal(f.supply.status().pathReplanCount,0);
  assert.equal(f.client.sent.filter(c=>['walk','run'].includes(c.type)).length,0);
});
test('expiry waiting and before-movement hazards share the same three per-actor replan credits',async t=>{
  for(const hazards of [1,3])await t.test(String(hazards),async()=>{
    const {f,metrics}=await w8HistoryFixture({hazards});
    if(hazards===1){const result=await f.supply.prepare();assert.equal(result.pathReplanCount,2);assert.equal(result.pathHazardCount,1);}
    else {await assert.rejects(f.supply.prepare(),isTerminalNavigationNoPath);assert.equal(f.supply.status().pathReplanCount,3);assert.equal(metrics().sleeps,0);}
    assert.equal(f.client.sent.some(c=>c.type==='run'&&c.direction==='Up'),false,'the guarded physical step is never sent');
  });
});
test('genuine terminal NoPath with only current live obstacles has no closed-history wait',async()=>{
  const {f,metrics}=await w8HistoryFixture({liveClosed:true});
  await assert.rejects(f.supply.prepare(),isTerminalNavigationNoPath);
  assert.equal(metrics().sleeps,0);assert.equal(f.supply.status().pathReplanCount,0);
  assert.equal(f.client.sent.length,1);
});
test('a recurring terminal position is scalar diagnosis only and cannot reuse expired history for another wait',async()=>{
  const {f}=await w8HistoryFixture({repeatTerminal:true});await assert.rejects(f.supply.prepare(),isTerminalNavigationNoPath);
  const result=f.supply.status(),cycle=result.navigationCycleDiagnostics;
  assert.equal(cycle.total,2);assert.equal(cycle.views.length,2);assert.equal(cycle.views[1].repeatedRetainedPosition,true);
  assert.equal(result.pathReplanCount,1);assert.equal(result.pathReplans[0].waitCompleted,true);
  cycle.views[0].from.x=0;cycle.views.pop();
  assert.equal(f.supply.status().navigationCycleDiagnostics.views.length,2);
  assert.equal(f.supply.status().navigationCycleDiagnostics.views[0].from.x,331);
});
test('closed history waiting keeps wall, command, ownership, health and transport failures terminal',async t=>{
  for(const failure of ['wall','commands','hp','map','closed','unknown-reuse'])await t.test(failure,async()=>{
    const {f}=await w8HistoryFixture({timeoutMs:failure==='wall'?19000:600000,maxCommands:failure==='commands'?1:2048,
      duringWait:({client,actor,packet})=>{
        if(failure==='hp'){actor.hp=1;client.snapshot.playerHp=1;}
        if(failure==='map')client.snapshot.mapFileName='2';
        if(failure==='closed')client.closed=true;
        if(failure==='unknown-reuse')packet('ObjectMonster',{objectId:206013,name:'UnknownMonster',location:{x:301,y:596},ai:99,masterObjectId:0,dead:false});
      }});
    await assert.rejects(f.supply.prepare(),failure==='wall'?/wall budget/:failure==='commands'?/command budget/:failure==='unknown-reuse'?/No walk path/:/owner\/map\/health changed|transport/);
    assert.equal(f.supply.status().actualStagingReached,false);
    const waited=f.supply.status().pathReplans[0];
    assert.equal(waited.waitCompleted,false);assert.ok(waited.actualPassiveWaitMs>0);
    assert.equal(waited.naturalExpiryReached,failure==='commands');
    assert.equal(waited.freshSnapshotSequence,null);assert.equal(waited.commandsAfterRefresh,null);
    if(failure!=='commands')assert.ok(waited.actualPassiveWaitMs<waited.waitMs);
    assert.equal(f.client.sent.some(c=>['walk','run','useItem','buyItem'].includes(c.type)),false);
  });
});

// W9 public exact NoPath view: seven certified points, sixteen occupants.
const w9Positions=[[206644,"scarecrow",293,614],[206610,"scarecrow",300,597],[206026,"rakingcat",320,589],[206623,"scarecrow",321,631],[206032,"rakingcat",322,595],[206136,"hookingcat",337,589],[206129,"hookingcat",342,600]];
const w9Occupancy=[{"x":314,"y":589,"objectId":206705,"kind":"monster"},{"x":318,"y":595,"objectId":206220,"kind":"monster"},{"x":320,"y":589,"objectId":206026,"kind":"monster"},{"x":321,"y":597,"objectId":206201,"kind":"monster"},{"x":321,"y":598,"objectId":214501,"kind":"monster"},{"x":322,"y":595,"objectId":206032,"kind":"monster"},{"x":328,"y":604,"objectId":200341,"kind":"monster"},{"x":330,"y":614,"objectId":206509,"kind":"monster"},{"x":330,"y":597,"objectId":206542,"kind":"monster"},{"x":337,"y":589,"objectId":206136,"kind":"monster"},{"x":342,"y":600,"objectId":206129,"kind":"monster"},{"x":342,"y":606,"objectId":206703,"kind":"monster"},{"x":342,"y":592,"objectId":206716,"kind":"monster"},{"x":343,"y":592,"objectId":206214,"kind":"monster"},{"x":344,"y":591,"objectId":210400,"kind":"monster"},{"x":345,"y":599,"objectId":206719,"kind":"monster"}];
// Static v2 preparation catalogue copied as scalar coordinates, no external runtime dependency.
const w9Candidates=[[288,615],[287,616],[289,616],[288,617],[289,615],[287,617],[289,617],[288,614],[286,616],[290,616],[288,618],[287,614],[289,614],[286,615],[290,615],[286,617],[290,617],[287,618],[289,618],[290,614],[286,618],[290,618],[288,613],[285,616],[291,616],[288,619],[287,613],[289,613],[285,615],[291,615],[285,617],[291,617],[287,619],[289,619],[286,613],[290,613],[291,614],[285,618],[291,618],[286,619],[290,619],[291,613],[285,619],[291,619],[288,612],[284,616],[292,616],[288,620],[287,612],[289,612],[284,617],[292,617],[287,620],[289,620],[286,612],[290,612],[284,614],[292,614],[284,618],[292,618],[286,620],[290,620],[285,612],[291,612],[284,613],[292,613],[284,619],[292,619],[285,620],[291,620],[284,612],[292,612],[284,620],[292,620],[288,611],[283,616],[293,616],[288,621],[287,611],[289,611],[283,615],[283,617],[293,617],[287,621],[289,621],[286,611],[290,611],[283,614],[293,614],[283,618],[293,618],[286,621],[290,621],[285,611],[291,611],[283,613],[293,613],[283,619],[293,619],[285,621],[291,621],[284,611],[292,611],[283,612],[293,612],[283,620],[293,620],[284,621],[283,611],[293,611],[283,621],[288,610],[282,616],[294,616],[288,622],[287,610],[289,610],[282,615],[294,615],[282,617],[294,617],[287,622],[289,622],[286,610],[290,610],[282,614],[294,614],[282,618],[294,618],[286,622],[290,622],[285,610],[282,613],[294,613],[285,622],[291,622],[284,610],[292,610],[282,612],[294,612],[294,620],[284,622],[283,610],[293,610],[282,611],[294,611],[294,621],[283,622],[282,610],[294,610],[282,622],[294,622],[288,609],[281,616],[295,616],[288,623],[287,609],[281,615],[295,615],[281,617],[295,617],[287,623],[289,623],[286,609],[281,614],[295,614],[295,618],[286,623],[290,623],[285,609],[281,613],[295,613],[295,619],[285,623],[291,623],[284,609],[281,612],[295,612],[295,620],[292,623],[283,609],[281,611],[295,621],[293,623],[282,609],[294,609],[281,610],[295,610],[295,622],[282,623],[294,623],[281,609],[295,609],[281,623],[295,623],[280,616],[296,616],[288,624],[287,608],[280,615],[296,615],[296,617],[287,624],[289,624],[286,608],[280,614],[296,614],[296,618],[286,624],[290,624],[285,608],[280,613],[291,624],[284,608],[280,612],[296,620],[292,624],[283,608],[293,608],[280,611],[296,621],[293,624],[282,608],[294,608],[280,610],[296,622],[294,624],[295,608],[280,609],[296,609],[296,623],[281,624],[295,624],[280,608],[296,608],[280,624],[296,624],[297,616],[288,625],[287,607],[279,615],[297,615],[287,625],[289,625],[286,607],[279,614],[297,614],[290,625],[285,607],[279,613],[297,613],[297,619],[291,625],[284,607],[279,612],[297,612],[297,620],[284,625],[292,625],[283,607],[293,607],[279,611],[297,621],[283,625],[293,625],[282,607],[294,607],[297,622],[294,625],[281,607],[295,607],[279,609],[297,623],[295,625],[280,607],[296,607],[279,608],[297,608],[297,624],[280,625],[296,625],[279,607],[297,607],[279,625],[297,625],[288,606],[298,616],[288,626],[298,615],[289,626],[286,606],[278,614],[298,614],[290,626],[285,606],[278,613],[298,619],[285,626],[291,626],[292,606],[278,612],[298,620],[284,626],[292,626],[293,606],[278,611],[298,621],[283,626],[293,626],[294,606],[278,610],[298,622],[282,626],[294,626],[281,606],[295,606],[278,609],[298,623],[295,626],[280,606],[296,606],[278,624],[298,624],[296,626],[279,606],[297,606],[298,607],[278,625],[298,625],[279,626],[297,626],[278,606],[298,606],[278,626],[298,626]].map(([x,y])=>({x,y}));

test('W9 full seven-point canonical geometry changes from1527-square-blocked to1223-rectangle43steps and334of335 candidates',async()=>{
  const map=await loadProtocolCollisionMap('0'),start={x:330,y:605},target={x:289,y:617},doors=w8Transfers.map(t=>({x:t.bounds.minX,y:t.bounds.minY}));
  const old=w8Forbidden(w9Positions,start),current=w8Forbidden(w9Positions,start,6);
  assert.equal(old.length,1527);assert.equal(current.length,1223);assert.equal(w9Occupancy.length,16);assert.equal(doors.length,69);
  assert.equal(planProtocolNavigation({map,start,target,desiredDistance:0,dynamicObstacles:[...old,...w9Occupancy,...doors]}),null);
  const dynamicObstacles=[...current,...w9Occupancy,...doors],path=planProtocolNavigation({map,start,target,desiredDistance:0,dynamicObstacles});
  assert.equal(path.steps.length,43);assert.equal(w9Candidates.length,335);
  assert.equal(w9Candidates.filter(target=>planProtocolNavigation({map,start,target,desiredDistance:0,dynamicObstacles})).length,334);
});
test('ordinary AI0 rectangle boundary and inside retreat guards use both weighted and physical distance',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  const cases=[
    ['outside-y7',[0,8],[0,7],false],['enter-y6',[0,7],[0,6],true],['enter-x8',[9,0],[8,0],true],
    ['outside-x9-y7',[9,7],[8,7],false],['inside-y6',[0,6],[0,5],true],
    ['inside-corner-physical-approach',[8,6],[7,6],true],['inside-corner-outside-but-closer',[8,6],[7,7],true],
    ['inside-retreat',[8,6],[9,5],false],['inside-maintains-both',[2,4],[3,4],false],['occupancy',[1,0],[0,0],true],
  ];
  for(const [name,from,next,blocked] of cases)await t.test(name,async()=>{
    let checked=false;const position={x:320,y:300};
    const f=fixture({items:[item(658,12,2)],ownerPosition:{x:position.x+from[0],y:position.y+from[1]},
      stagingPoint:{x:position.x+from[0],y:position.y+from[1]},ordinaryProtection:protection,
      hostiles:[hostile(71,position.x,position.y,{name:'Scarecrow',ownerName:null,class:null})],
      navigate:async({navOptions,actor})=>{
        const cell={x:position.x+next[0],y:position.y+next[1]},hazard=navOptions.beforeMovement({movementType:'walk',from:{x:actor.x,y:actor.y},physicalCells:[cell]});
        assert.equal(!!hazard,blocked);
        const obstacles=navOptions.forbiddenPoints;
        assert.equal(obstacles.some(p=>p.x===cell.x&&p.y===cell.y),blocked,'planner and beforeMovement agree');
        checked=true;throw new Error('geometry fixture complete');
      }});
    await assert.rejects(f.supply.prepare(),/geometry fixture complete/);assert.equal(checked,true);assert.equal(f.client.sent.length,1);
  });
});
test('unknown owned nonAI0 expired and current conflicts retain the original square',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  for(const fault of ['unknown','owned','nonAI0','expired','class','name-conflict','global-quarantine'])await t.test(fault,async()=>{
    const extra=fault==='global-quarantine'?Array.from({length:1025},(_,i)=>hostile(10000+i,600,500,{ownerName:null,class:null})):[];
    let checked=false;const f=fixture({items:[item(658,12,2)],ownerPosition:{x:320,y:308},stagingPoint:{x:320,y:308},ordinaryProtection:protection,
      hostiles:[hostile(71,320,300,{name:fault==='unknown'?'UnknownMonster':'Scarecrow',ownerName:fault==='owned'?'foreign':null,
        ai:fault==='nonAI0'?1:0,class:fault==='class'?'Warrior':null}),...extra],
      navigate:async({client,actor,navOptions})=>{
        if(fault==='expired')f.advance(20001);
        if(fault==='name-conflict')client.snapshot.entities.find(e=>e.objectId===71).name='HookingCat';
        assert.ok(navOptions.beforeMovement({movementType:'walk',from:{x:actor.x,y:actor.y},physicalCells:[{x:320,y:307}]}));
        checked=true;throw new Error('square fallback verified');
      }});
    await assert.rejects(f.supply.prepare(),/square fallback verified/);assert.equal(checked,true);
  });
});
test('explicit larger named or default clearance is never reduced to the ordinary rectangle',()=>{
  assert.deepEqual(ordinaryPreparationClearanceGeometry({qualifiedNormalAi0:true}),{x:8,y:6,rectangle:true});
  for(const request of [{radius:9},{namedRadius:10},{radius:12,namedRadius:9}])
    assert.deepEqual(ordinaryPreparationClearanceGeometry({...request,qualifiedNormalAi0:true}),
      {x:Math.max(request.radius??8,request.namedRadius??0),y:Math.max(request.radius??8,request.namedRadius??0),rectangle:false});
  assert.deepEqual(ordinaryPreparationClearanceGeometry({qualifiedNormalAi0:false}),{x:8,y:8,rectangle:false});
});
test('strict same-identity wire reentry preserves the rectangle for its original authenticated position trail',async()=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');let checked=false;
  const f=fixture({items:[item(658,12,2)],ownerPosition:{x:324,y:308},stagingPoint:{x:324,y:308},ordinaryProtection:protection,applyPackets:true,
    navigate:async({packet,navOptions})=>{
      packet('ObjectMonster',wireMonster(71,{location:{x:320,y:300}}));void navOptions.forbiddenPoints;
      packet('ObjectRemove',{objectId:71});f.advance(13500);
      packet('ObjectMonster',wireMonster(71,{location:{x:329,y:300}}));
      const points=navOptions.forbiddenPoints;
      assert.ok(navOptions.allowedHostileObjectIds.includes(71));assert.equal(f.supply.status().plannerThreatMemory.positions,2);
      assert.equal(points.some(p=>p.x===320&&p.y===307),false,'a new certificate object for the same identity does not widen old y7 to a square');
      assert.equal(points.some(p=>p.x===320&&p.y===306),true);checked=true;throw new Error('same normal trail checked');
    }});
  await assert.rejects(f.supply.prepare(),/same normal trail checked/);assert.equal(checked,true);
});
test('Run checks its intermediate cell at beforeMovement and send using the same rectangle',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  for(const phase of ['before-movement','send'])await t.test(phase,async()=>{
    let saw=false;const f=fixture({items:[item(658,12,2)],ownerPosition:{x:320,y:308},stagingPoint:{x:320,y:308},
      ordinaryProtection:protection,hostiles:[hostile(71,320,300,{name:'Scarecrow',ownerName:null,class:null})],
      navigate:async({facade,actor,navOptions})=>{
        if(phase==='before-movement'){
          const h=navOptions.beforeMovement({movementType:'run',from:{x:actor.x,y:actor.y},physicalCells:[{x:320,y:307},{x:320,y:306}]});
          assert.equal(h.physicalCells.length,2);saw=true;throw new NavigationStepGuarded(h);
        }
        saw=true;facade.send({type:'run',direction:'Up'});await facade.wait(()=>false,'Run landing enters y6');
      }});
    await assert.rejects(f.supply.prepare());assert.equal(saw,true);
    assert.equal(f.client.sent.some(c=>c.type==='run'),false);
    assert.ok(f.supply.status().pathHazards.every(h=>h.phase===phase));
  });
});
test('the new-target rectangle never masks an actual released hit or known poison at readiness',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  await t.test('released-hit',async()=>{
    let armed=false,damaged=false;
    const f=fixture({items:[item(658,12,2)],ownerPosition:{x:320,y:308},stagingPoint:{x:320,y:307},ordinaryProtection:protection,
      hostiles:[hostile(71,320,300,{name:'Scarecrow',ownerName:null,class:null})],
      onPace:({client,actor,packet,snapshot})=>{if(armed&&!damaged){damaged=true;
        packet('ObjectAttack',{objectId:71,location:{x:320,y:300},direction:4});
        actor.hp=60;client.snapshot.playerHp=60;snapshot();}},
      navigate:async({facade,actor,navOptions})=>{
        assert.ok(!navOptions.beforeMovement({movementType:'walk',from:{x:actor.x,y:actor.y},physicalCells:[{x:320,y:307}]}));
        armed=true;facade.send({type:'walk',direction:'Up'});await facade.wait(()=>false,'released damage before dispatch');
      }});
    await assert.rejects(f.supply.prepare(),/owner\/map\/health changed/);
    assert.equal(damaged,true);assert.equal(f.client.sent.some(c=>c.type==='walk'),false);
    assert.equal(f.supply.status().preparationThreatModel.damageImmunity,false);
  });
  await t.test('known-poison',async()=>{
    const f=fixture({items:[item(658,12,2)],ordinaryProtection:protection});
    f.client.snapshot.inSafeZone=true;f.client.snapshot.playerPoison=4;
    await assert.rejects(f.supply.prepare(),/negative status at staging/);
    assert.equal(f.supply.status().actualStagingReached,false);assert.equal(f.client.snapshot.playerPoison,4);
  });
});
test('a real canonical return survives multiple AOI departures and reentries without repeating the W9 wall loop',async t=>{
  const protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0'),map=await loadProtocolCollisionMap('0');
  const names={scarecrow:'Scarecrow',rakingcat:'RakingCat',hookingcat:'HookingCat'},closed=new Set([206644,206610,206623]);
  const threats=w9Positions.map(([id,name,x,y])=>hostile(id,x,y,{name:names[name],hp:name==='scarecrow'?20:name==='rakingcat'?32:30,ownerName:null,class:null}));
  const neutrals=w9Occupancy.filter(p=>!threats.some(e=>e.objectId===p.objectId)).map(p=>({...p,name:'Deer',ai:2,hp:25,dead:false,disposition:'neutral',ownerName:null,class:null}));
  let f,navigator,seeded=false,moves=0,departures=0,reentries=0,firstOptions,firstTarget;
  const updateAoi=({client,actor,packet})=>{
    moves++;
    for(const entity of threats){
      const visible=distanceForTest(actor,entity)<=16,exists=client.snapshot.entities.some(e=>e.objectId===entity.objectId);
      if(!visible&&exists){packet('ObjectRemove',{objectId:entity.objectId});departures++;}
      if(visible&&!exists){packet('ObjectMonster',{objectId:entity.objectId,name:entity.name,location:{x:entity.x,y:entity.y},ai:0,masterObjectId:0,dead:false});reentries++;}
      if(visible&&moves%2===0)packet('ObjectTurn',{objectId:entity.objectId,x:entity.x,y:entity.y,direction:0});
    }
    client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;
  };
  f=fixture({ownerId:1000,ownerHp:18,ownerMaxHp:18,items:[item(658,12,2)],gold:15,
    ownerPosition:{x:330,y:605},stagingPoint:{x:289,y:617},hostiles:[...threats,...neutrals],ordinaryProtection:protection,
    applyPackets:true,noMovementSnapshots:true,onMove:updateAoi,
    onSnapshot:({client})=>{client.snapshot.entities=client.snapshot.entities.map(e=>threats.find(t=>t.objectId===e.objectId)?{...threats.find(t=>t.objectId===e.objectId)}:e);},
    navigate:async({facade,target,range,stop,navOptions,packet})=>{
      navigator??=createNavigator(facade,{loadCollisionMap:async()=>map,now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});
      if(!seeded){
        seeded=true;firstOptions=navOptions;firstTarget=target;
        void navOptions.forbiddenPoints;
        for(const id of closed)packet('ObjectRemove',{objectId:id});
        assert.equal(navOptions.forbiddenPoints.length,1223);
        // Ordinary real-map movements replay repeated AOI crossings, not
        // teleport or manual owner transforms. All use the original facade.
        for(let i=0;i<2;i++){await navigator({x:305,y:612},0,stop,navOptions);await navigator({x:330,y:605},0,stop,navOptions);}
      }
      assert.equal(navOptions,firstOptions);assert.equal(target,firstTarget);
      return navigator(target,range,stop,navOptions);
    }});
  f.client.snapshot.entities=f.client.snapshot.entities.filter(e=>e.kind!=='npc');f.client.snapshot.mapTransfers=structuredClone(w8Transfers);
  const result=await f.supply.prepare();assert.equal(result.status,'ready');
  assert.equal(result.pathReplanCount,0);assert.equal(result.pathHazardCount,0);
  assert.ok(departures>8&&reentries>8);assert.ok(result.elapsedMs>20000&&result.elapsedMs<600000);
  assert.equal(result.commands,result.movementCommands+2);assert.equal(f.client.sent.filter(c=>c.type==='clientVersion').length,2);
  assert.equal(result.after.hp,18);assert.equal(result.after.hpStock,12);assert.equal(result.after.gold,15);
  assert.deepEqual({x:result.after.x,y:result.after.y},{x:289,y:617});
  assert.equal(result.preparationThreatModel.damageImmunity,false);assert.equal(result.preparationThreatModel.newTargetAcquisitionOnly,true);
  assert.equal(result.navigationCycleDiagnostics.total,0);
  t.diagnostic(JSON.stringify({scope:'static W9 observed geometry with actual wire AOI lifecycle replay; not live acceptance',moves:result.movementCommands,departures,reentries,elapsedMs:result.elapsedMs}));
});

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
  assert.equal(result.status, 'ready'); assert.equal(result.funding.collected, 2); assert.equal(result.funding.hunts, 2);
  for (const type of ['attackV2', 'harvest', 'pickUp', 'sellItem', 'buyItem']) assert.ok(f.client.sent.some(command => command.type === type), type);
  assert.equal(f.client.sent.some(c => ['attack', 'townRevive', 'teleport', 'qa.giveItem'].includes(c.type)), false);
  assert.equal(result.after.hpStock, 12); assert.equal(result.after.gold, 20);
  assert.equal(result.after.venison, 0, 'capital completion does not hunt an unneeded third meat');
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
    assert.equal(result.status, 'ready'); assert.equal(result.funding.hunts, 2); assert.equal(result.funding.collected, 2);
    assert.equal(f.client.sent.filter(command => command.type === 'harvest').length, 12);
    assert.equal(f.client.events.filter(event => event.packet === 'ObjectHarvest').length, 12);
    assert.equal(f.client.events.filter(event => event.packet === 'ObjectHarvested').length, 2);
    assert.equal(f.client.sent.filter(command => command.type === 'pickUp').length, directHarvest ? 0 : 2);
    assert.equal(result.after.hpStock, 12); assert.equal(result.after.gold, 20); assert.equal(result.after.venison, 0);
    assert.equal(result.soldVenison, 2); assert.equal(result.purchased, 12); assert.equal(result.proofs.length, 3);
    assert.ok(result.elapsedMs >= 60_000 && result.elapsedMs < 600_000, 'virtual clock covers both genuine six-pass harvests');
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
    const f = fixture({ deer: 4, v2: true, target:16, targetLossAtPace: loss });
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
  const bounded = fixture({ deer: 4, v2: true, target:16, targetLossAtPace: 'dead', maxHunts: 3 });
  await assert.rejects(bounded.supply.prepare(), /after \d+ hunts and 3 attempts/);
  assert.equal(bounded.client.sent.filter(command => command.type === 'attackV2').length, 2);
  assert.equal(bounded.client.sent.some(command => command.type === 'buyItem'), false);
});

test('a nearby non-Deer threat excludes the dangerous Deer before a bounded safe alternative hunt', async () => {
  const f = fixture({ deer: 4, v2: true, target:16, nonDeerBlocker: true, distantAlternatives: true, stagingPoint:{x:310,y:607} });
  const result = await f.supply.prepare();
  assert.equal(result.status, 'ready'); assert.equal(result.funding.collected, 3);
  assert.equal(f.client.sent.some(command => command.objectId === 99 && ['attack', 'attackV2'].includes(command.type)), false);
  assert.equal(result.fundingTargetRejectionCount,0,'unselected nonpreferred Deer is not a failed attempt');
  assert.deepEqual(result.fundingApproaches.map(row=>row.objectId),[21,22,23]);
  assert.equal(result.funding.huntAttempts,3);
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

test('ordinary qualified-only dangerous Deer use the original bounded search without being selected or credited',async()=>{
  const f=fixture({deer:8,v2:true,hostiles:[hostile(71,295,604)]});
  await assert.rejects(f.supply.prepare(),/after 20 bounded refreshes/);
  assert.equal(f.client.sent.some(c=>['attack','attackV2','harvest','buyItem'].includes(c.type)),false);
  assert.equal(f.supply.status().fundingTargetRejectionCount,0);
  assert.equal(f.supply.status().funding,null);
  assert.ok(f.supply.status().commands<2048);
  assert.equal(f.supply.status().fundingApproaches.length,0);
});

test('friendly, unclassified or owned Deer are not ordinary neutral funding targets',async t=>{
  for(const kind of ['friendly','unknown','owned'])await t.test(kind,async()=>{
    let f;f=fixture({deer:4,v2:true,distantAlternatives:true,stagingPoint:{x:310,y:607},navigate:actualNavigatorFor(()=>f)});const target=f.client.snapshot.entities.find(e=>e.objectId===20);
    if(kind==='friendly')target.disposition='friendly';
    if(kind==='unknown')delete target.disposition;
    if(kind==='owned')target.ownerName='another-player';
    assert.equal((await f.supply.prepare()).status,'ready');
    assert.equal(f.client.sent.some(c=>c.type==='attackV2'&&c.objectId===20),false);
    assert.equal(f.supply.status().fundingTargetRejectionCount,0);
    assert.equal(f.supply.status().funding.huntAttempts,2);
    const fallback=fixture({deer:1,v2:true,maxHunts:1,maxEmptySearchRefreshes:1});const only=fallback.client.snapshot.entities.find(e=>e.objectId===20);
    if(kind==='friendly')only.disposition='friendly';if(kind==='unknown')delete only.disposition;if(kind==='owned')only.ownerName='another-player';
    await assert.rejects(fallback.supply.prepare(),/after 1 bounded refreshes/);
    assert.equal(fallback.supply.status().fundingTargetRejectionCount,0);
    assert.equal(fallback.supply.status().fundingApproaches.length,0);
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
  f=fixture({deer:4,v2:true,target:16,distantAlternatives:true,stagingPoint:{x:310,y:607},
    onPace:({client,snapshot})=>{if(armed&&!changed){changed=true;client.snapshot.entities.push(hostile(71,290,606));snapshot();}},
    navigate:async context=>{if(context.target.objectId===20)armed=true;return navigate(context);}});
  assert.equal((await f.supply.prepare()).status,'ready');
  assert.equal(f.client.sent.some(c=>c.type==='attackV2'&&c.objectId===20),false);
  assert.ok(f.supply.status().fundingTargetRejections.some(row=>row.reason==='hostile-near-deer'&&row.phase.startsWith('approach-')));
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
    await assert.rejects(funding.supply.prepare(),/after 20 bounded refreshes/);
    assert.equal(funding.client.sent.some(command=>['attack','attackV2','harvest'].includes(command.type)),false);
    assert.equal(funding.supply.status().fundingTargetRejectionCount,0);assert.equal(funding.supply.status().fundingApproaches.length,0);
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
  f=fixture({deer:3,v2:true,target:16,deerPositions:[{x:310,y:594},{x:312,y:594},{x:314,y:594}],ownerPosition:{x:288,y:615},stagingPoint:{x:288,y:615},
    hostiles:threats,ordinaryProtection:protection,navigate:actualNavigatorFor(()=>f),
    onMove:({client,actor})=>{client.snapshot.inSafeZone=actor.x>=278&&actor.x<=298&&actor.y>=606&&actor.y<=626;}});
  f.client.snapshot.inSafeZone=true;
  const result=await f.supply.prepare();
  assert.equal(result.status,'ready');assert.equal(result.funding.hunts,3,JSON.stringify(result.funding));assert.equal(result.funding.collected,3);
  assert.equal(result.purchased,16);assert.equal(result.soldVenison,3);assert.equal(result.goldEarned,750);assert.equal(result.after.venison,0);assert.equal(result.after.hpStock,16);
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

test('ordinary capital quote proves two actual sale prices instead of hunting the estimated third',async()=>{
  const f=fixture({deer:3,v2:true,items:[item(658,1,2)]});
  const result=await f.supply.prepare();
  assert.equal(result.funding.collected,2);assert.equal(result.funding.estimatedRequiredCount,3);
  assert.equal(result.funding.huntAttempts,2);assert.equal(result.soldVenison,2);assert.equal(result.purchased,11);
  assert.equal(result.after.hpStock,12);assert.equal(result.actualStagingReached,true);
  const proof=result.fundingCapitalProofs.at(-1);
  assert.equal(proof.capital,500);assert.equal(proof.requiredGold,440);assert.equal(proof.initialHpPrice,40);
  assert.equal(proof.initialHpQuote.price,40);assert.ok(proof.initialHpQuote.sequence<proof.sequence);
  assert.equal(proof.quoteAgeMs,proof.monotonicMs-proof.initialHpQuote.monotonicMs);assert.ok(proof.quoteAgeMs>=0);
  const quoteEvent=f.client.events.find(e=>e.sequence===proof.initialHpQuote.sequence);
  assert.equal(quoteEvent.packet,'NPCGoods');assert.equal(quoteEvent.monotonicMs,proof.initialHpQuote.monotonicMs);
  assert.equal(proof.gold,0);assert.equal(proof.items.length,2);assert.equal(proof.planningOnly,true);assert.equal(proof.measurementCredit,false);
  proof.initialHpQuote.price=1;assert.equal(f.supply.status().fundingCapitalProofs.at(-1).initialHpQuote.price,40);
  proof.items[0].sellValue=1;assert.equal(f.supply.status().fundingCapitalProofs.at(-1).items[0].sellValue,250);
});
test('capital uses atomic fresh raw wallet and quotes even if live snapshot changes after receipt',async()=>{
  let versions=0;
  const f=fixture({deer:3,v2:true,onSnapshot:({client})=>{
    if(client.sent.at(-1)?.type==='clientVersion')versions++;
  }});
  const originalWait=f.client.wait;
  f.client.wait=async function(predicate,label,timeout) {
    const row=await originalWait.call(this,predicate,label,timeout);
    if(label.includes('fresh')&&row?.capitalQuote&&row.capitalQuote.items.length===2) this.snapshot.gold=1;
    return row;
  };
  const result=await f.supply.prepare();const proof=result.fundingCapitalProofs.at(-1);
  assert.equal(proof.gold,0);assert.equal(proof.capital,500);assert.ok(versions>=3);
});
test('ordinary capital quote rejects malformed, missing, duplicate and overflowing live values',async t=>{
  const corruptions={
    missing:i=>{delete i.sellValue;},zero:i=>{i.sellValue=0;},negative:i=>{i.sellValue=-1;},nan:i=>{i.sellValue=NaN;},
    infinite:i=>{i.sellValue=Infinity;},string:i=>{i.sellValue='250';},
    overflow:i=>{i.sellValue=Number.MAX_SAFE_INTEGER;i.quantity=2;},
    duplicate:(i,client)=>{client.snapshot.inventoryItems.push({...item(658,1,i.uniqueId),slot:10});},
    duplicateOther:(i,client)=>{client.snapshot.inventoryItems.push({...i,name:'Wood',slot:10});},
    malformed:(i,client)=>{client.snapshot.inventoryItems.push(null);}
  };
  for(const [name,corrupt] of Object.entries(corruptions))await t.test(name,async()=>{
    let changed=false;
    const f=fixture({deer:3,v2:true,onSnapshot:({client})=>{
      if(!changed&&client.sent.at(-1)?.type==='clientVersion'){
        const i=client.snapshot.inventoryItems.find(i=>i?.name==='Venison');
        if(i){changed=true;corrupt(i,client);}
      }
    }});
    await assert.rejects(f.supply.prepare());assert.equal(f.supply.status().status,'failed');
    assert.equal(f.client.sent.some(c=>c.type==='buyItem'),false);assert.equal(f.supply.status().actualStagingReached,false);
  });
});
test('capital refresh health and identity changes are terminal without spending another hunt',async t=>{
  for(const fault of ['health','name','map'])await t.test(fault,async()=>{
    let changed=false;
    const f=fixture({deer:3,v2:true,onSnapshot:({client,actor})=>{
      if(!changed&&client.sent.at(-1)?.type==='clientVersion'&&client.snapshot.inventoryItems.some(i=>i.name==='Venison')){
        changed=true;if(fault==='health'){actor.hp=10;client.snapshot.playerHp=10;}
        if(fault==='name')actor.name='other';if(fault==='map')client.snapshot.mapFileName='2';
      }
    }});
    await assert.rejects(f.supply.prepare());assert.equal(f.client.sent.filter(c=>c.type==='attackV2').length,1);
    assert.equal(f.client.sent.some(c=>c.type==='buyItem'),false);
  });
});

test('ordinary low-value quotes continue after the estimate but stop at eight attempts',async()=>{
  const f=fixture({deer:8,v2:true,onSnapshot:({client})=>{
    for(const i of client.snapshot.inventoryItems)if(i.name==='Venison')i.sellValue=1;
  }});
  await assert.rejects(f.supply.prepare(),/capital insufficient.*8 hunts and 8 attempts/);
  assert.equal(f.client.sent.filter(c=>c.type==='attackV2').length,8);
  assert.equal(f.supply.status().fundingCapitalProofs.at(-1).capital,8);
  assert.equal(f.client.sent.some(c=>c.type==='buyItem'),false);
});
test('capital is only planning credit and a higher returning Ruben price still fails',async()=>{
  let returning=false;
  const config={deer:3,v2:true,price:40,onSnapshot:({client})=>{
    if(client.sent.some(c=>c.type==='sellItem'))returning=true;
    if(returning)config.price=100;
  }};
  const f=fixture(config);await assert.rejects(f.supply.prepare(),/insufficient earned gold/);
  assert.equal(f.supply.status().fundingCapitalProofs.at(-1).satisfied,true);
  assert.equal(f.supply.status().purchased,0);assert.equal(f.supply.status().actualStagingReached,false);
});

test('non-Venison material never provides capital and belt UIDs still participate in uniqueness',async()=>{
  const wood={...item(221,1,50),name:'WoodenSword',sellValue:Number.MAX_SAFE_INTEGER};
  const f=fixture({deer:3,v2:true,items:[wood]});const result=await f.supply.prepare();
  assert.equal(result.funding.hunts,2);assert.equal(result.fundingCapitalProofs.at(-1).capital,500);
  assert.equal(f.client.snapshot.inventoryItems.some(i=>i.uniqueId===50),true);
  const duplicate=fixture({deer:3,v2:true,items:[wood]});duplicate.client.snapshot.beltItems=[item(658,1,50)];
  await assert.rejects(duplicate.supply.prepare(),/duplicate capital inventory UID/);
  assert.equal(duplicate.client.sent.some(c=>c.type==='attackV2'),false);
});

test('real ordinary funding navigator stops on actual AOI Remove without another movement or attack',async()=>{
  let f,removed=false,movesAtRemoval=0,navigator;
  const start={x:290,y:607};
  f=fixture({deer:1,v2:true,maxHunts:1,deerPositions:[{x:276,y:607}],applyPackets:true,noMovementSnapshots:true,
    onMove:({client,actor,packet})=>{
      if(!removed&&Math.max(Math.abs(actor.x-276),Math.abs(actor.y-607))>16){
        removed=true;movesAtRemoval=client.sent.filter(c=>['walk','run'].includes(c.type)).length;
        packet('ObjectRemove',{objectId:20});
      }
    },
    navigate:async({facade,target,range,stop,navOptions})=>{
      if(target.name==='Deer')assert.equal(navOptions.detectPositionCycles,true);
      else assert.equal(navOptions.detectPositionCycles,undefined);
      navigator??=createNavigator(facade,{loadCollisionMap:async()=>{
        const blocked=new Uint8Array(360*660);for(let y=590;y<=624;y++)blocked[y*360+282]=1;
        return {mapFileName:'0',width:360,height:660,blocked};},now:f.now,delay:async ms=>f.advance(ms),movementResponseTimeoutMs:2000});
      return navigator(target,range,stop,navOptions);
    }});
  await assert.rejects(f.supply.prepare(),/capital insufficient.*0 hunts and 1 attempts/);
  assert.equal(removed,true);assert.ok(movesAtRemoval>0);
  assert.equal(f.client.sent.filter(c=>['walk','run'].includes(c.type)).length,movesAtRemoval);
  assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);
  const row=f.supply.status().fundingApproaches[0];
  assert.equal(row.objectId,20);assert.equal(row.status,'target-lost');assert.equal(row.navigationCalls,1);
  assert.deepEqual(row.from,start);assert.deepEqual(row.target,{x:276,y:607});assert.equal(row.ownerName,'ordinary-owner');
  assert.ok(row.endedSequence>row.sequence);assert.ok(row.elapsedMs>0);
  row.target.x=0;assert.equal(f.supply.status().fundingApproaches[0].target.x,276);
});
test('approach target loss consumes its attempt and never reselects or attacks the old ID',async()=>{
  let f,removed=false,atRemoval=0;
  const real=actualNavigatorFor(()=>f);
  f=fixture({deer:3,v2:true,applyPackets:true,deerPositions:[{x:300,y:604},{x:302,y:604},{x:304,y:604}],
    onMove:({client,packet})=>{if(!removed){removed=true;atRemoval=client.sent.length;packet('ObjectRemove',{objectId:20});}},navigate:real});
  const result=await f.supply.prepare();
  assert.equal(result.funding.huntAttempts,3);assert.equal(result.funding.hunts,2);assert.equal(result.funding.collected,2);
  assert.equal(f.client.sent.some(c=>c.type==='attackV2'&&c.objectId===20),false);
  assert.deepEqual(result.fundingApproaches.map(r=>r.objectId),[20,21,22]);assert.equal(result.fundingApproaches[0].status,'target-lost');
  assert.equal(f.client.sent[atRemoval].type,'clientVersion','reselection starts with the original fresh capital refresh, not a stale movement');
  assert.equal(result.after.hpStock,12);assert.equal(result.pathReplanCount,0);
});
test('beforeMovement and paced actual dispatch both recheck the bound selected Deer',async t=>{
  for(const phase of ['before-movement','send'])await t.test(phase,async()=>{
    let armed=false;
    const f=fixture({deer:1,v2:true,maxHunts:1,applyPackets:true,
      onPace:({packet})=>{if(armed){armed=false;packet('ObjectRemove',{objectId:20});}},
      navigate:async({facade,target,actor,packet,navOptions})=>{
        if(target.name!=='Deer')return;
        if(phase==='before-movement'){
          packet('ObjectRemove',{objectId:20});
          navOptions.beforeMovement({from:{x:actor.x,y:actor.y},physicalCells:[{x:actor.x+1,y:actor.y}],movementType:'walk'});
        }else{armed=true;facade.send({type:'walk',direction:'Right'});await facade.wait(()=>false,'bound funding queued move');}
      }});
    await assert.rejects(f.supply.prepare(),/capital insufficient.*1 attempts/);
    assert.equal(f.client.sent.some(c=>['walk','run','attackV2'].includes(c.type)),false);
    const result=f.supply.status();assert.equal(result.fundingApproaches[0].status,'target-lost');
    assert.equal(result.fundingTargetRejections.at(-1).phase,phase==='send'?'approach-send':'approach-before-movement');
  });
});
test('pending commands and original owner wall or command guards remain terminal during approach target loss',async t=>{
  for(const fault of ['queued','hp','map','name','closed','wall','commands'])await t.test(fault,async()=>{
    let armed=false,f;
    f=fixture({deer:2,v2:true,applyPackets:true,maxCommands:fault==='commands'?4:2048,
      onPace:({client,actor,packet})=>{if(armed){armed=false;packet('ObjectRemove',{objectId:20});
        if(fault==='hp'){actor.hp=10;client.snapshot.playerHp=10;}if(fault==='map')client.snapshot.mapFileName='2';
        if(fault==='name')actor.name='other';if(fault==='closed')client.closed=true;if(fault==='wall')f.advance(600001);}},
      navigate:async({facade,target})=>{if(target.name!=='Deer')return;armed=true;facade.send({type:'walk',direction:'Right'});
        if(fault==='queued')facade.send({type:'turn',direction:'Right'});
        await facade.wait(()=>false,'bound funding pending commands');}});
    await assert.rejects(f.supply.prepare(),fault==='queued'?/target loss with pending commands/:fault==='commands'?/command budget/:fault==='wall'?/wall budget/:fault==='closed'?/transport closed/:/owner\/map\/health changed/);
    assert.equal(f.client.sent.some(c=>['walk','run','turn','attackV2'].includes(c.type)),false);
    assert.equal(f.supply.status().fundingApproaches.length,1);assert.equal(f.supply.status().fundingApproaches[0].status,'terminal');
    assert.equal(f.supply.status().fundingCapitalProofCount,1);assert.equal(f.supply.status().pathReplanCount,0);
  });
});
test('actual navigator position cycle in a bound funding approach stays terminal without target-loss retry',async()=>{
  let f,moves=0;const real=actualNavigatorFor(()=>f);
  f=fixture({deer:3,v2:true,deerPositions:[{x:300,y:607},{x:301,y:607},{x:302,y:607}],
    onMove:({actor})=>{actor.x=++moves%2?291:290;actor.y=607;},navigate:real});
  let original;
  await assert.rejects(f.supply.prepare(),error=>{original=error;return error instanceof NavigationStalled;});
  assert.equal(original.reason,'position-cycle');assert.equal(f.supply.status().fundingApproaches.length,1);
  assert.equal(f.supply.status().fundingApproaches[0].status,'terminal');assert.equal(f.supply.status().fundingTargetRejectionCount,0);
  assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);assert.equal(f.supply.status().fundingCapitalProofCount,1);
});

test('funding approach external failure text stays private and the exact original error remains terminal',async t=>{
  for(const source of ['client.failure','transport'])await t.test(source,async()=>{
    const secret='Bearer-private-token-should-never-be-retained';const original=new Error(`${source}: ${secret}`);
    const f=fixture({deer:2,v2:true,navigate:async({client,target})=>{
      if(target.name!=='Deer')return;
      if(source==='client.failure')client.failure=original;
      throw original;
    }});
    await assert.rejects(f.supply.prepare(),error=>error===original);
    const result=f.supply.status();assert.equal(result.fundingApproaches.length,1);
    assert.equal(result.fundingApproaches[0].status,'terminal');assert.equal(result.fundingApproaches[0].reason,'ordinary-operation-failed');
    assert.equal(JSON.stringify(result).includes(secret),false);assert.equal(result.fundingCapitalProofCount,1);
    assert.equal(f.client.sent.some(c=>['walk','run','attackV2'].includes(c.type)),false);
  });
});

test('ordinary known-safe priority selects farther eligible Deer without charging the seven unselected unsafe candidates',async()=>{
  const f=fixture({deer:9,v2:true,nonDeerBlocker:true,stagingPoint:{x:310,y:607},
    deerPositions:[...Array.from({length:7},(_,i)=>({x:283+i,y:604})),{x:300,y:604},{x:302,y:604}]});
  const result=await f.supply.prepare();assert.equal(result.status,'ready');assert.equal(result.funding.huntAttempts,2);
  assert.deepEqual(result.fundingApproaches.map(row=>row.objectId),[27,28]);assert.equal(result.fundingTargetRejectionCount,0);
  assert.equal(result.funding.hunts,2);assert.equal(result.purchased,12);assert.equal(result.after.hpStock,12);
  assert.equal(f.client.sent.some(c=>c.type==='attackV2'&&c.objectId<27),false);
  assert.ok(f.client.snapshot.entities.filter(e=>e.name==='Deer'&&!e.dead).length>=7,'nonpreferred candidates were not removed or declared attempted');
});

function ordinaryDiscoveryFixture(extra={}) {
  let f,introduced=false,rawSupplied=false;const real=actualNavigatorFor(()=>f);
  f=fixture({deer:0,v2:true,target:6,maxHunts:1,maxEmptySearchRefreshes:1,applyPackets:true,
    maxCommands:extra.maxCommands??2048,
    onSnapshot:context=>{
      if(introduced&&!rawSupplied&&context.client.sent.at(-1)?.type==='clientVersion'){
        rawSupplied=true;Object.assign(context.client.snapshot.entities.find(e=>e.objectId===20),{hp:1,dead:false});
        extra.fresh?.(context);
      }
    },
    onPace:context=>{if(introduced)extra.pace?.(context,f);},
    navigate:async context=>{
      if(context.range===8&&context.target.objectId==null&&!introduced){
        introduced=true;context.packet('ObjectMonster',{objectId:20,name:'Deer',location:{x:291,y:607},ai:2,masterObjectId:0,dead:false});
      }
      return real(context);
    }});
  return f;
}
test('ordinary actual ObjectMonster discovery ends real NAV then one paced raw proves HP before hunting',async()=>{
  const f=ordinaryDiscoveryFixture();const result=await f.supply.prepare();
  assert.equal(result.status,'ready');assert.equal(result.funding.hunts,1);assert.equal(result.funding.huntAttempts,1);
  assert.equal(result.after.hpStock,6);assert.equal(result.fundingDiscoveries.length,1);
  const proof=result.fundingDiscoveries[0];assert.equal(proof.objectId,20);assert.equal(proof.status,'refreshed');
  assert.equal(proof.discoveryOnly,true);assert.equal(proof.liveQualification,false);assert.equal(proof.refresh,1);assert.equal(proof.cueRefresh,1);
  assert.ok(proof.freshSnapshotSequence>proof.beforeRefreshSequence);assert.ok(proof.triggerSequence<=proof.beforeRefreshSequence);
  const wire=f.client.events.find(e=>e.sequence===proof.triggerSequence);assert.equal(wire.packet,'ObjectMonster');assert.equal(wire.payload.hp,undefined);
  assert.equal(f.client.events.find(e=>e.sequence===proof.freshSnapshotSequence).type,'worldSnapshot');
  const attack=f.client.sent.find(c=>c.type==='attackV2');assert.ok(attack.at>=proof.monotonicMs);
  proof.searchPoint.x=0;assert.notEqual(f.supply.status().fundingDiscoveries[0].searchPoint.x,0);
});
test('fresh owned unsafe invalid dead and still unknown candidates never gain attack permission from a discovery cue',async t=>{
  for(const fault of ['owned','unsafe','invalid','dead','unknown'])await t.test(fault,async()=>{
    const f=ordinaryDiscoveryFixture({fresh:({client})=>{
      const e=client.snapshot.entities.find(e=>e.objectId===20);
      if(fault==='owned')e.ownerName='another-player';if(fault==='invalid')e.hp='1';
      if(fault==='dead')Object.assign(e,{hp:0,dead:true});if(fault==='unknown')e.hp=null;
      if(fault==='unsafe')client.snapshot.entities.push(hostile(71,291,608));
    }});
    await assert.rejects(f.supply.prepare());const result=f.supply.status();
    assert.equal(result.fundingDiscoveries.length,1);assert.equal(result.fundingDiscoveries[0].status,'refreshed');
    assert.equal(result.actualStagingReached,false);assert.equal(f.client.sent.some(c=>['attackV2','harvest','buyItem'].includes(c.type)),false);
    assert.equal(result.fundingTargetRejectionCount,0,'fresh ineligible candidates are not selected in qualified-only mode');
    assert.equal(result.fundingApproaches.length,0);
  });
});
test('discovery raw refresh keeps exact external failures and original ownership wall and command guards terminal',async t=>{
  for(const fault of ['external','hp','map','name','closed','wall','commands'])await t.test(fault,async()=>{
    const original=new Error('private external refresh failure');let armed=true;
    const f=ordinaryDiscoveryFixture({maxCommands:fault==='commands'?4:2048,pace:({client,actor},fixture)=>{
      if(!armed)return;armed=false;
      if(fault==='external')client.failure=original;
      if(fault==='hp'){actor.hp=10;client.snapshot.playerHp=10;}if(fault==='map')client.snapshot.mapFileName='2';
      if(fault==='name')actor.name='another-owner';if(fault==='closed')client.closed=true;if(fault==='wall')fixture.advance(600001);
    }});
    await assert.rejects(f.supply.prepare(),error=>fault==='external'?error===original:/ordinary supplies:/.test(error.message));
    const result=f.supply.status();assert.equal(result.fundingDiscoveries.length,1);assert.equal(result.fundingDiscoveries[0].status,'terminal');
    assert.equal(result.fundingDiscoveries[0].freshSnapshotSequence,undefined);assert.equal(result.fundingDiscoveries[0].reason,'ordinary-refresh-failed');
    assert.equal(result.actualStagingReached,false);assert.equal(f.client.sent.some(c=>['walk','run','attackV2','harvest','buyItem'].includes(c.type)),false);
  });
});

test('a discovery cue cannot bypass pending navigation queue drain or turn its error into a raw refresh',async()=>{
  const original=new Error('queued navigation transport failure');let armed=false;
  const f=fixture({deer:0,v2:true,target:6,applyPackets:true,
    onPace:({client})=>{if(armed)client.failure=original;},
    navigate:async({facade,target,range,stop,packet})=>{
      if(range!==8||target.objectId!=null)return;
      packet('ObjectMonster',{objectId:20,name:'Deer',location:{x:291,y:607},ai:2,masterObjectId:0,dead:false});
      assert.equal(stop(),true);facade.send({type:'turn',direction:'Right'});facade.send({type:'keepAlive'});armed=true;
    }});
  await assert.rejects(f.supply.prepare(),error=>error===original);
  assert.equal(f.supply.status().fundingDiscoveries.length,0);assert.equal(f.supply.status().actualStagingReached,false);
  assert.equal(f.client.sent.some(c=>['turn','keepAlive','walk','run','attackV2'].includes(c.type)),false);
});

test('selected-target public terminal enums recognize only complete messages for the exact bound ID',async t=>{
  const messages={
    'no-authoritative-progress':'target 20 made no authoritative combat progress after 4 attacks',
    'no-walk-path':'target 20 has no walk path from the current map region',
    'raw-target-loss':'target 20 left the authoritative snapshot before attack',
    'ordinary-operation-failed':'target 21 left the authoritative snapshot before attack',
  };
  for(const [reason,message] of Object.entries(messages))await t.test(reason,async()=>{
    const f=fixture({deer:1,v2:true,maxHunts:1,navigate:async({target})=>{if(target.name==='Deer')throw new Error(message);}});
    await assert.rejects(f.supply.prepare(),/0 hunts and 1 attempts/);const row=f.supply.status().fundingApproaches[0];
    assert.equal(row.reason,reason);assert.equal(row.status,'terminal');assert.equal(JSON.stringify(row).includes(message),false);
    assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);
  });
});

test('ordinary qualified-only selection races still consume eight actual attempts and exclude every old ID',async()=>{
  const f=fixture({deer:8,v2:true,navigate:async({target,stop})=>{
    if(target.name!=='Deer')return;target.ownerName='another-owner';stop();
  }});
  await assert.rejects(f.supply.prepare(),/0 hunts and 8 attempts/);const result=f.supply.status();
  assert.deepEqual(result.fundingApproaches.map(row=>row.objectId),[20,21,22,23,24,25,26,27]);
  assert.equal(result.fundingTargetRejectionCount,8);assert.ok(result.fundingApproaches.every(row=>row.status==='target-lost'));
  assert.ok(result.fundingTargetRejections.every(row=>row.reason==='not-neutral-deer'&&row.phase==='approach-stop'));
  assert.equal(f.client.sent.some(c=>['attackV2','harvest','buyItem'].includes(c.type)),false);
});


const fundingOpenMap=(width=640,height=640)=>({mapFileName:'0',width,height,blocked:new Uint8Array(width*height)});
test('fundingNavigationMap is explicit and validates map0 bitmap shape without changing legacy helpers',()=>{
  for(const map of [null,{},fundingOpenMap(0,1),{...fundingOpenMap(),mapFileName:'1'},{...fundingOpenMap(),blocked:[]},
    {...fundingOpenMap(),blocked:new Uint8Array(1)},{...fundingOpenMap(),width:1.5},fundingOpenMap(2001,2000)]){
    assert.throws(()=>fixture({fundingNavigationMap:map}),/fundingNavigationMap/);
  }
  assert.doesNotThrow(()=>fixture({fundingNavigationMap:fundingOpenMap()}));assert.doesNotThrow(()=>fixture());
});

test('W15 four actual selected positions have no static AOI16 route and remain unselected with the pinned real map',async(t)=>{
  const map=await loadProtocolCollisionMap('0');
  for(const [id,start,target] of [[206500,{x:270,y:635},{x:256,y:621}],[206521,{x:273,y:614},{x:261,y:608}],
    [206708,{x:275,y:612},{x:259,y:596}],[206706,{x:271,y:616},{x:255,y:609}]])await t.test(String(id),async()=>{
    let armed=false;const f=fixture({deer:1,v2:true,fundingNavigationMap:map,maxEmptySearchRefreshes:1,
      deerPositions:[target],onSnapshot:({client,actor})=>{if(client.events.some(e=>e.packet==='NPCGoods'))Object.assign(actor,start);},
      navigate:async({target:goal,stop})=>{if(goal.name==='Deer')armed=true;else if(stop)assert.equal(stop(),false);}});
    f.client.snapshot.entities.find(e=>e.name==='Deer').objectId=id;
    await assert.rejects(f.supply.prepare(),/after 1 bounded refreshes/);
    const status=f.supply.status();assert.equal(armed,false);assert.equal(status.fundingApproaches.length,0);
    assert.ok(status.fundingCandidateQueries.some(q=>q.objectId===id&&q.reason==='static-aoi-unreachable'));
    assert.equal(f.client.sent.some(c=>['attackV2','walk','run'].includes(c.type)),false);
  });
});

test('funding local crop respects map edges, diagonal corner collision and every transfer without a global detour',async(t)=>{
  const sentinel=new Error('fixture local candidate reached');
  for(const mode of ['open-edge','diagonal-corner','transfer','outside-crop-wall'])await t.test(mode,async()=>{
    const map=fundingOpenMap(32,32),start={x:2,y:2},target={x:0,y:0};let selected=false;
    if(mode==='diagonal-corner'){map.blocked[1*32+2]=1;map.blocked[2*32+1]=1;map.blocked[3*32+2]=1;map.blocked[2*32+3]=1;}
    const mapTransfers=mode==='transfer'?[{mapFileName:'0',bounds:{minX:0,maxX:1,minY:0,maxY:1}}]:[];
    if(mode==='outside-crop-wall')for(let y=0;y<18;y++)map.blocked[y*32+1]=1;
    const f=fixture({deer:1,v2:true,fundingNavigationMap:map,mapTransfers,maxEmptySearchRefreshes:1,deerPositions:[target],
      onSnapshot:({client,actor})=>{if(client.events.some(e=>e.packet==='NPCGoods'))Object.assign(actor,start);},
      navigate:async({target:goal})=>{if(goal.name==='Deer'){selected=true;throw sentinel;}}});
    await assert.rejects(f.supply.prepare(),error=>mode==='open-edge'?error===sentinel:/bounded refreshes/.test(error.message));
    assert.equal(selected,mode==='open-edge');assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);
  });
});

async function fundingHistoryQueryFixture({target={x:281,y:596},expire=false,unknown=false}={}){
  const map=await loadProtocolCollisionMap('0'),protection=await loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},'0');
  const sentinel=new Error('fixture eligible after history query');let f,remembered=false,selected=false,waited=false;
  f=fixture({ownerPosition:{x:293,y:612},deer:1,deerPositions:[target],v2:true,fundingNavigationMap:map,ordinaryProtection:protection,applyPackets:true,
    maxEmptySearchRefreshes:1,hostiles:[hostile(206604,275,600,{name:'Scarecrow',...(unknown?{ai:undefined}: {})})],
    navigate:async({target:goal,navOptions,packet,stop})=>{
      if(goal.name==='Merchant_Ruben'&&!remembered){void navOptions.forbiddenPoints;remembered=true;packet('ObjectRemove',{objectId:206604});}
      else if(goal.name==='Deer'){selected=true;if(unknown)assert.equal(navOptions.allowedHostileObjectIds.includes(206604),false);throw sentinel;}
      else if(expire&&!waited){waited=true;f.advance(20_001);if(stop)stop();}
    }});
  // Unknown AI remains a genuine unknown DTO, not the source-backed omitted-AI case.
  if(unknown)f.client.snapshot.entities.find(e=>e.objectId===206604).name='UnknownHostile';
  try{await f.supply.prepare();assert.fail('fixture must terminate');}catch(error){
    if(expire||unknown||target.y>=606)assert.equal(error,sentinel);else assert.match(error.message,/bounded refreshes/);
  }
  return {f,selected};
}
test('only certified unexpired retained goals reject; natural expiry and protected safe-zone cells stay distinct',async(t)=>{
  await t.test('actual W15 last goal fully covered by closed normal history',async()=>{
    const {f,selected}=await fundingHistoryQueryFixture();assert.equal(selected,false);
    assert.ok(f.supply.status().fundingCandidateQueries.some(q=>q.reason==='certified-goal-blocked'));
    assert.equal(f.supply.status().plannerThreatMemory.packetRenewals,0);
  });
  await t.test('original expiry naturally removes this query rejection without renewing any history',async()=>{
    const {f,selected}=await fundingHistoryQueryFixture({expire:true});assert.equal(selected,true);
    assert.equal(f.supply.status().fundingApproaches.length,1);assert.equal(f.supply.status().plannerThreatMemory.packetRenewals,0);
  });
  await t.test('current authored safe cells do not become historical AI0 forbidden goals',async()=>{
    const {f,selected}=await fundingHistoryQueryFixture({target:{x:289,y:617}});assert.equal(selected,true);
    assert.equal(f.supply.status().fundingCandidateQueries.some(q=>q.reason==='certified-goal-blocked'),false);
  });
  await t.test('unknown has no certified-query exemption or allowed ID',async()=>{
    const {f,selected}=await fundingHistoryQueryFixture({unknown:true});assert.equal(selected,true);
    assert.equal(f.supply.status().fundingCandidateQueries.some(q=>q.reason==='certified-goal-blocked'),false);
    assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);
  });
});

test('exact selected approach AOI ring and beforeMovement cover Walk1 and both Run2 cells without sending out-of-view steps',async(t)=>{
  for(const type of ['walk','run'])await t.test(type,async()=>{
    let checked=false;const origin={x:type==='walk'?290:289,y:607},goal={x:274,y:607};
    const f=fixture({ownerPosition:origin,deer:1,deerPositions:[goal],v2:true,fundingNavigationMap:fundingOpenMap(),maxEmptySearchRefreshes:1,
      navigate:async({target,navOptions,actor})=>{if(target.name!=='Deer')return;checked=true;
        assert.ok(navOptions.forbiddenPoints.some(p=>p.x===291&&p.y===607));
        navOptions.beforeMovement({from:{x:actor.x,y:actor.y},physicalCells:Array.from({length:type==='run'?2:1},(_,i)=>({x:actor.x+i+1,y:actor.y})),movementType:type});}});
    await assert.rejects(f.supply.prepare(),/bounded refreshes/);const row=f.supply.status().fundingApproaches[0];
    assert.equal(checked,true);assert.equal(row.status,'target-lost');assert.equal(row.reason,'approach-leaves-aoi');
    assert.equal(f.client.sent.some(c=>['walk','run','attackV2'].includes(c.type)),false);
  });
});
test('pace-updated selected target drives the send AOI gate and stale queued metadata cannot replay',async(t)=>{
  for(const queued of [false,true])await t.test(queued?'pending-command-fatal':'single-unsent-target-loss',async()=>{
    let armed=false,changed=false;
    const f=fixture({ownerPosition:{x:289,y:607},deer:1,deerPositions:[{x:274,y:607}],v2:true,fundingNavigationMap:fundingOpenMap(),maxEmptySearchRefreshes:1,
      onPace:({client})=>{if(armed&&!changed){changed=true;client.snapshot.entities.find(e=>e.name==='Deer').x=273;}},
      navigate:async({target,facade,navOptions,actor})=>{if(target.name!=='Deer')return;
        assert.equal(navOptions.beforeMovement({from:{x:actor.x,y:actor.y},physicalCells:[{x:290,y:607}],movementType:'walk'}),undefined);
        armed=true;facade.send({type:'walk',direction:'Right'});if(queued)facade.send({type:'turn',direction:'Right'});await facade.wait(()=>false,'funding boundary wait');}});
    await assert.rejects(f.supply.prepare(),queued?/target loss with pending commands/:/bounded refreshes/);
    assert.equal(changed,true);assert.equal(f.client.sent.some(c=>['walk','run','turn','attackV2'].includes(c.type)),false);
    assert.equal(f.supply.status().fundingApproaches[0].reason,'approach-leaves-aoi');
  });
});


test('funding AOI ring follows the same selected ID latest position rather than retaining a stale target cell',async()=>{
  const sentinel=new Error('fixture dynamic ring checked');let checked=false;
  const f=fixture({ownerPosition:{x:289,y:607},deer:1,deerPositions:[{x:274,y:607}],v2:true,fundingNavigationMap:fundingOpenMap(),
    navigate:async({target,navOptions,client})=>{if(target.name!=='Deer')return;
      assert.ok(navOptions.forbiddenPoints.some(p=>p.x===291&&p.y===607));
      client.snapshot.entities.find(e=>e.objectId===target.objectId).x=275;
      const updated=navOptions.forbiddenPoints;assert.ok(updated.some(p=>p.x===292&&p.y===607));
      assert.equal(updated.some(p=>p.x===291&&p.y===607),false);checked=true;throw sentinel;}});
  await assert.rejects(f.supply.prepare(),error=>error===sentinel);assert.equal(checked,true);assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);
});
test('funding map query rejects malformed or non-current transfer metadata instead of granting a path',async()=>{
  for(const transfers of [null,[{bounds:{minX:1,maxX:0,minY:0,maxY:0}}],[{mapFileName:'2',bounds:{minX:1,maxX:1,minY:0,maxY:0}}],
    [{bounds:{minX:1.5,maxX:2,minY:0,maxY:0}}]]){
    const f=fixture({deer:1,v2:true,fundingNavigationMap:fundingOpenMap()});f.client.snapshot.mapTransfers=transfers;
    await assert.rejects(f.supply.prepare(),/funding map transfer/);assert.equal(f.supply.status().fundingApproaches.length,0);
    assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);
  }
});
test('funding static query and AOI checks never replace the original command health identity transport or wall guards',async(t)=>{
  await t.test('shared command budget',async()=>{
    const f=fixture({deer:1,v2:true,fundingNavigationMap:fundingOpenMap(),maxCommands:4});
    await assert.rejects(f.supply.prepare(),/command budget exhausted/);assert.equal(f.client.sent.length,4);
    assert.equal(f.client.sent.some(c=>c.type==='attackV2'),false);
  });
  for(const fault of ['hp','name','map','closed','wall'])await t.test(fault,async()=>{
    let armed=false,f;
    f=fixture({ownerPosition:{x:289,y:607},deer:1,deerPositions:[{x:274,y:607}],v2:true,fundingNavigationMap:fundingOpenMap(),
      onPace:({client,actor})=>{if(!armed)return;if(fault==='hp'){client.snapshot.playerHp=10;actor.hp=10;}
        if(fault==='name')actor.name='foreign-owner';if(fault==='map')client.snapshot.mapFileName='2';
        if(fault==='closed')client.closed=true;if(fault==='wall')f.advance(600_001);},
      navigate:async({target,facade})=>{if(target.name!=='Deer')return;armed=true;facade.send({type:'walk',direction:'Right'});await facade.wait(()=>false,'funding original guard');}});
    await assert.rejects(f.supply.prepare(),fault==='closed'?/transport closed/:fault==='wall'?/wall budget exhausted/:/owner.map.health changed/);
    assert.equal(f.client.sent.some(c=>['walk','attackV2'].includes(c.type)),false);
    assert.notEqual(f.supply.status().fundingApproaches[0].reason,'approach-leaves-aoi');
  });
});
