// Optional ordinary-player lifecycle checks on the caller's isolated realm.
// Every pass needs live packet receipts and current state. This is not visual,
// two-computer, cross-Cell, durable-failover or capacity acceptance.
import { createNavigator } from './quest-agent/protocol-play.mjs';
import { planProtocolNavigation } from './quest-agent/protocol-navigation.mjs';
import { hasAuthoritativePlayerDeath, observedPlayerHp } from './quest-agent/protocol-observation.mjs';

export const FULL_LIFECYCLE_CHECKS = ['sharedKill', 'sharedDrop', 'uniquePickup', 'playerDeath', 'townRevive'];
// Witnessed ordinary route, not a teleport or an assertion that these cells are
// currently free. Each segment is planned again against terrain and live AOI.
export const WESTERN_EXPOSURE_CORRIDOR = Object.freeze([[288, 617], [286, 617], [285, 616], [284, 616],
  [282, 616], [283, 615], [283, 614], [284, 613], [284, 612], [286, 612], [287, 611],
  [289, 611], [291, 611], [292, 611], [293, 610], [294, 610], [294, 609],
  [296, 607], [298, 605], [299, 604], [299, 602], [299, 600]].map(([x, y]) => Object.freeze({ x, y })));
// Deer/Hen require Crystal's ordinary corpse-harvest path. A kill alone is
// deliberately not used as their loot proof by this pickup-focused profile.
export const isLifecycleLootTarget = entity => entity?.kind === 'monster' &&
  ['Scarecrow', 'HookingCat', 'RakingCat'].includes(entity.name) && !entity.dead && Number(entity.hp) > 0;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const self = client => client.snapshot?.entities?.find(entity =>
  Number(entity.objectId) === Number(client.snapshot?.playerObjectId));
const distance = (a, b) => Math.max(Math.abs(Number(a?.x) - Number(b?.x)), Math.abs(Number(a?.y) - Number(b?.y)));
const packets = (client, cursor, packet) => client.events.filter(event =>
  event.direction === 'received' && event.sequence > cursor && event.packet === packet);
const sent = (client, cursor, type, objectId) => client.events.find(event =>
  event.direction === 'sent' && event.sequence > cursor && event.type === type &&
  (objectId == null || event.objectId === objectId));
const point = value => ({ x: Number(value?.location?.x ?? value?.x), y: Number(value?.location?.y ?? value?.y) });
const samePoint = (a, b) => Number.isInteger(a.x) && Number.isInteger(a.y) && a.x === b.x && a.y === b.y;
const id = value => Number.isInteger(value) && value > 0 && value <= 0xffffffff;
const uid = value => (Number.isSafeInteger(value) && value > 0) ||
  (typeof value === 'string' && /^[1-9][0-9]{0,19}$/.test(value) && BigInt(value) <= 0xffffffffffffffffn);

export function validateLifecycleCommand(command) {
  if (command?.type === 'pickUp' && Object.keys(command).sort().join(',') === 'objectId,type' && id(command.objectId)) return;
  if (command?.type === 'townRevive' && Object.keys(command).join(',') === 'type') return;
  if (command?.type === 'attack' && Object.keys(command).sort().join(',') === 'objectId,type' && id(command.objectId)) return;
  if (command?.type === 'useItem' && Object.keys(command).sort().join(',') === 'grid,type,uniqueId' &&
      command.grid === 'inventory' && Number.isSafeInteger(command.uniqueId) && command.uniqueId > 0) return;
  if (command?.type === 'equipItem' && Object.keys(command).sort().join(',') === 'grid,to,type,uniqueId' &&
      command.grid === 'inventory' && Number.isSafeInteger(command.uniqueId) && command.uniqueId > 0 && command.to === 1) return;
  if (command?.type === 'removeItem' && Object.keys(command).sort().join(',') === 'grid,to,type,uniqueId' &&
      command.grid === 'inventory' && Number.isSafeInteger(command.uniqueId) && command.uniqueId > 0 &&
      Number.isInteger(command.to) && command.to >= 0 && command.to < 40) return;
  throw new Error('Invalid ordinary lifecycle command; exact ordinary pickup, attack, held-item use, town revival or armour equipment fields are required');
}

export function lifecycleExposureTargets(snapshot) {
  const owner = self({ snapshot });
  if (!owner) return [];
  return (snapshot.entities ?? []).filter(entity => entity.kind === 'monster' &&
    ['HookingCat', 'RakingCat'].includes(entity.name) && id(entity.objectId) && !entity.dead &&
    Number(entity.hp) > 0 && Number.isInteger(entity.x) && Number.isInteger(entity.y) &&
    entity.disposition !== 'friendly' && !entity.ownerName && !(Number(entity.masterObjectId) > 0) &&
    distance(owner, entity) <= 18)
    .sort((a, b) => distance(owner, a) - distance(owner, b) || a.objectId - b.objectId);
}

function exposureObstacles(snapshot) {
  const owner = self({ snapshot });
  const obstacles = (snapshot.entities ?? []).filter(entity => entity.objectId !== owner?.objectId && !entity.dead);
  for (const transfer of snapshot.mapTransfers ?? []) {
    const bounds = transfer.bounds;
    if (!bounds || ![bounds.minX, bounds.maxX, bounds.minY, bounds.maxY].every(Number.isInteger) ||
        bounds.maxX - bounds.minX > 100 || bounds.maxY - bounds.minY > 100) continue;
    for (let y = bounds.minY; y <= bounds.maxY; y++) for (let x = bounds.minX; x <= bounds.maxX; x++) obstacles.push({ x, y });
  }
  return obstacles;
}

export function planExposureSegment(snapshot, map, target, radius = 0) {
  if (!Number.isInteger(target?.x) || !Number.isInteger(target?.y) || ![0, 1].includes(radius)) return null;
  const owner = self({ snapshot });
  if (!owner) return null;
  const plan = planProtocolNavigation({ map, start: owner, target, desiredDistance: radius,
    dynamicObstacles: exposureObstacles(snapshot), maxExpanded: 5000 });
  return plan && plan.path.length <= 81 ? plan : null;
}

export function exposureCorridorEntry(snapshot, map) {
  if (String(snapshot?.mapFileName) !== '0') return null;
  return WESTERN_EXPOSURE_CORRIDOR.map((target, index) => ({ target, index,
    plan: planExposureSegment(snapshot, map, target, exposureObstacles(snapshot).some(entity => samePoint(point(entity), target)) ? 1 : 0) }))
    .filter(entry => entry.plan)
    .sort((a, b) => a.plan.path.length - b.plan.path.length || a.index - b.index)[0] ?? null;
}

// Current generic wild-monster acquisition is dx<=8 && dy<=6. AOI is wider.
// This guides the observer outside cats' acquisition rectangle while retaining
// an owner-relative <=10-tile observation position; it grants no server rights.
export const outsideExposureAggro = (position, cats) => cats.every(cat =>
  Math.abs(position.x - cat.x) > 8 || Math.abs(position.y - cat.y) > 6);

export function planExposureObserver(snapshot, victim, cats, map) {
  const observer = self({ snapshot });
  if (!observer || !victim || !cats.length) return null;
  if (distance(observer, victim) <= 10 && outsideExposureAggro(observer, cats)) {
    return { target: point(observer), plan: { path: [point(observer)], steps: [] } };
  }
  const targets = [];
  for (const radius of [8, 10]) for (const [dx, dy] of [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]]) {
    const target = { x: victim.x + radius * dx, y: victim.y + radius * dy };
    if (!outsideExposureAggro(target, cats)) continue;
    const plan = planExposureSegment(snapshot, map, target);
    if (plan && plan.path.every((position, index, path) => index === 0 || cats.every(cat =>
      outsideExposureAggro(path[index - 1], [cat]) ? outsideExposureAggro(position, [cat]) :
        distance(position, cat) >= distance(path[index - 1], cat)))) targets.push({ target, plan });
  }
  return targets.sort((a, b) => a.plan.path.length - b.plan.path.length)[0] ?? null;
}

export async function useHeldExposureHpPotion(client, deadline) {
  await client.refresh();
  const before = client.snapshot;
  if (hasAuthoritativePlayerDeath(before)) return null;
  const hp = observedPlayerHp(before), maximum = Number(before.playerMaxHp);
  if (!(maximum > 0) || hp >= maximum * .6) return null;
  const item = before.inventoryItems?.find(value => /^\(hp\)drugsmall$/i.test(String(value.name)) &&
    Number.isSafeInteger(value.uniqueId) && value.uniqueId > 0 && Number(value.quantity ?? value.count ?? 0) > 0);
  if (!item) return null;
  const quantity = Number(item.quantity ?? item.count), cursor = client.sequence;
  if (!Number.isSafeInteger(quantity)) throw new Error('Held small HP potion quantity is not exact');
  const command = { type: 'useItem', grid: 'inventory', uniqueId: item.uniqueId };
  validateLifecycleCommand(command);
  const receipt = await client.request(command, 'UseItem', remaining(deadline, 8000));
  if (receipt.sequence <= cursor || receipt.payload?.success !== true || receipt.payload.uniqueId !== item.uniqueId ||
      String(receipt.payload.grid).toLowerCase() !== 'inventory') throw new Error('Held small HP potion lacks an exact fresh UseItem receipt');
  await client.refresh();
  const after = client.snapshot.inventoryItems?.find(value => value.uniqueId === item.uniqueId);
  if (quantity - Number(after?.quantity ?? after?.count ?? 0) !== 1) throw new Error('Held small HP potion consumption was not confirmed by the fresh inventory');
  return { uniqueId: item.uniqueId, name: item.name, beforeQuantity: quantity,
    afterQuantity: quantity - 1, receipt: receipt.sequence, phase: 'approach-only' };
}

export function sharedKillEvidence(clients, combat) {
  if (clients.length !== 2 || !id(combat?.targetId) || !Array.isArray(combat?.lifecycleStart?.cursors) ||
      combat.lifecycleStart.cursors.length !== 2 ||
      new Set((combat.hits ?? []).map(hit => hit.attackerName)).size !== 2) return null;
  const deaths = clients.map((client, index) => packets(client, combat.lifecycleStart.cursors[index], 'ObjectDied')
    .find(event => event.payload?.objectId === combat.targetId &&
      (combat.hits ?? []).every(hit => hit.targetId === combat.targetId &&
        Number.isInteger(hit.receipts?.[index]?.struck) && hit.receipts[index].struck <= event.sequence)));
  if (!deaths.every(Boolean) || !samePoint(point(deaths[0].payload), point(deaths[1].payload)) ||
      clients.some((client, index) => packets(client, combat.lifecycleStart.cursors[index], 'ObjectRevived')
        .some(event => event.payload?.objectId === combat.targetId && event.sequence < deaths[index].sequence))) return null;
  return { targetId: combat.targetId, target: combat.target, mapFileName: combat.lifecycleStart.mapFileName,
    location: point(deaths[0].payload), attackerNames: [...new Set(combat.hits.map(hit => hit.attackerName))],
    deathReceipts: deaths.map(event => event.sequence), hits: combat.hits };
}

function dropSignature(drop) {
  return JSON.stringify({ name: drop.name, x: drop.x, y: drop.y, quantity: drop.quantity,
    sourceMonster: drop.sourceMonster, loot: drop.loot });
}

export function sharedDropEvidence(clients, combat, kill) {
  if (!kill || clients.some(client => String(client.snapshot?.mapFileName) !== kill.mapFileName)) return null;
  const baseline = new Set(combat.lifecycleStart.baselineDropIds ?? []);
  const candidates = (clients[0].snapshot?.groundDrops ?? []).filter(drop => id(drop.objectId) &&
    !baseline.has(drop.objectId) && drop.sourceMonster === kill.target && distance(drop, kill.location) <= 3)
    .sort((a, b) => Number(b.loot?.kind === 'gold') - Number(a.loot?.kind === 'gold'));
  for (const drop of candidates) {
    const gold = drop.loot?.kind === 'gold';
    if ((!gold && drop.loot?.kind !== 'inventoryItem') ||
        (gold && (!Number.isSafeInteger(drop.loot.amount) || drop.loot.amount <= 0))) continue;
    const visible = clients.map(client => client.snapshot.groundDrops?.find(other =>
      other.objectId === drop.objectId && dropSignature(other) === dropSignature(drop)));
    const spawns = clients.map((client, index) => packets(client, kill.deathReceipts[index], gold ? 'ObjectGold' : 'ObjectItem')
      .find(event => event.payload?.objectId === drop.objectId && samePoint(point(event.payload), point(drop)) &&
        (gold ? event.payload.gold === drop.loot.amount : event.payload.name === drop.name)));
    if (visible.every(Boolean) && spawns.every(Boolean)) return {
      drop: structuredClone(drop), spawnReceipts: spawns.map(event => event.sequence),
      sourceScope: 'Fresh shared spawn after the witnessed death, within three tiles, with the same snapshot sourceMonster name. Public packets do not expose a unique source-monster ID.',
    };
  }
  return null;
}

const carried = snapshot => ['inventoryItems', 'equipmentItems'].flatMap(key =>
  (snapshot?.[key] ?? []).map(item => ({ container: key, uniqueId: String(item.uniqueId),
    itemIndex: Number(item.itemIndex ?? item.index), count: Number(item.count ?? item.quantity ?? 1) })))
  .sort((a, b) => a.container.localeCompare(b.container) || a.uniqueId.localeCompare(b.uniqueId));
const economic = snapshot => ({ gold: snapshot?.gold, carried: carried(snapshot) });
const countIndex = (snapshot, index) => carried(snapshot).filter(item => item.itemIndex === index)
  .reduce((sum, item) => sum + item.count, 0);

export function uniquePickupEvidence(clients, cursors, before, drop) {
  if (clients.length !== 2 || before.length !== 2 || !id(drop?.objectId) ||
      clients.some((client, index) => String(client.snapshot?.mapFileName) !== String(before[index]?.mapFileName) ||
        (client.snapshot.groundDrops ?? []).some(item => item.objectId === drop.objectId))) return null;
  const requests = clients.map((client, index) => sent(client, cursors[index], 'pickUp', drop.objectId));
  if (!requests.every(Boolean)) return null;
  const removals = clients.map((client, index) => packets(client, cursors[index], 'ObjectRemove')
    .find(event => event.payload?.objectId === drop.objectId));
  if (!removals.every(Boolean)) return null;
  const receipts = clients.map((client, index) => [
    ...packets(client, cursors[index], 'GainedGold'), ...packets(client, cursors[index], 'GainedItem'),
  ]);
  if (receipts.flat().length !== 1) return null;
  const winner = receipts.findIndex(events => events.length === 1), gain = receipts[winner][0];
  if (gain.sequence <= requests[winner].sequence) return null;
  if (drop.loot?.kind === 'gold') {
    if (gain.packet !== 'GainedGold' || gain.payload?.gold !== drop.loot.amount ||
        clients.some((client, index) => !Number.isSafeInteger(before[index].gold) ||
          client.snapshot.gold - before[index].gold !== (index === winner ? drop.loot.amount : 0) ||
          JSON.stringify(carried(client.snapshot)) !== JSON.stringify(carried(before[index])))) return null;
  } else if (drop.loot?.kind === 'inventoryItem') {
    const item = gain.payload?.item, exact = drop.loot.exactItem?.item;
    if (gain.packet !== 'GainedItem' || !uid(item?.unique_id) || !Number.isInteger(item?.item_index) ||
        !Number.isInteger(item.count) || item.count <= 0 || item.count !== drop.quantity ||
        !exact || exact.item_index !== item.item_index || exact.count !== item.count ||
        (drop.loot.exactItem.uidAssigned && (!uid(exact.unique_id) || String(exact.unique_id) !== String(item.unique_id))) ||
        before.some(snapshot => carried(snapshot).some(old => old.uniqueId === String(item.unique_id))) ||
        clients.some((client, index) => client.snapshot.gold !== before[index].gold ||
          countIndex(client.snapshot, item.item_index) - countIndex(before[index], item.item_index) !==
            (index === winner ? item.count : 0)) ||
        !carried(clients[winner].snapshot).some(value =>
          value.uniqueId === String(item.unique_id) && value.itemIndex === item.item_index && value.count === item.count) ||
        JSON.stringify(carried(clients[1 - winner].snapshot)) !== JSON.stringify(carried(before[1 - winner]))) return null;
    const expected = carried(before[winner]);
    expected.push({ container: 'inventoryItems', uniqueId: String(item.unique_id),
      itemIndex: item.item_index, count: item.count });
    expected.sort((a, b) => a.container.localeCompare(b.container) || a.uniqueId.localeCompare(b.uniqueId));
    if (JSON.stringify(carried(clients[winner].snapshot)) !== JSON.stringify(expected)) return null;
  } else return null;
  return { objectId: drop.objectId, winner, requestReceipts: requests.map(event => event.sequence),
    gainPacket: gain.packet, gainReceipt: gain.sequence,
    gain: gain.payload, removalReceipts: removals.map(event => event.sequence),
    before: before.map(economic), after: clients.map(client => economic(client.snapshot)),
    scope: 'One fresh gain, exact conserved gold or one new item UID; stack-merge item deliveries are not inferred.' };
}

export function pickupReplayEvidence(clients, cursors, before, drop) {
  if (clients.some((client, index) => packets(client, cursors[index], 'GainedGold').length ||
      packets(client, cursors[index], 'GainedItem').length ||
      (client.snapshot?.groundDrops ?? []).some(item => item.objectId === drop.objectId) ||
      String(client.snapshot?.mapFileName) !== String(before[index].mapFileName) ||
      JSON.stringify(economic(client.snapshot)) !== JSON.stringify(economic(before[index])))) return null;
  const requests = clients.map((client, index) => sent(client, cursors[index], 'pickUp', drop.objectId));
  if (!requests.every(Boolean)) return null;
  const freshSnapshotSequences = clients.map(client =>
    client.events.findLast(event => event.direction === 'received' && event.type === 'worldSnapshot' &&
      event.sequence > requests[clients.indexOf(client)].sequence)?.sequence);
  if (freshSnapshotSequences.some(value => !Number.isInteger(value))) return null;
  return { objectId: drop.objectId, requestReceipts: requests.map(event => event.sequence), freshSnapshotSequences };
}

export function playerDeathEvidence(clients, context) {
  if (!context?.initialAlive || clients.length !== 2 ||
      clients.some((client, index) => self(client)?.name !== context.names[index] ||
        String(client.snapshot?.mapFileName) !== context.mapFileName)) return null;
  const deaths = packets(clients[0], context.cursors[0], 'Death');
  const observer = packets(clients[1], context.cursors[1], 'ObjectDied')
    .filter(event => event.payload?.objectId === context.observerId);
  if (deaths.length !== 1 || observer.length !== 1 || !hasAuthoritativePlayerDeath(clients[0].snapshot) ||
      packets(clients[0], context.cursors[0], 'Revived').some(event => event.sequence < deaths[0].sequence) ||
      !samePoint(point(deaths[0].payload), point(observer[0].payload))) return null;
  return { actor: context.names[0], observerId: context.observerId, mapFileName: context.mapFileName,
    location: point(deaths[0].payload), ownerDeathReceipt: deaths[0].sequence,
    observerDeathReceipt: observer[0].sequence };
}

export function townReviveEvidence(clients, context, cursors) {
  const owner = clients[0], revived = packets(owner, cursors[0], 'Revived');
  const request = sent(owner, cursors[0], 'townRevive');
  const snapshot = owner.events.findLast(event => event.direction === 'received' && event.type === 'worldSnapshot' &&
    event.sequence > revived[0]?.sequence);
  if (revived.length !== 1 || !request || request.sequence >= revived[0].sequence ||
      !snapshot || self(owner)?.name !== context.names[0] ||
      hasAuthoritativePlayerDeath(owner.snapshot) || !Number.isFinite(owner.snapshot?.playerHp) ||
      owner.snapshot.playerHp <= 0 || observedPlayerHp(owner.snapshot) <= 0) return null;
  const map = String(owner.snapshot?.mapFileName ?? '');
  if (!map) return null;
  const observerPacket = map === context.mapFileName
    ? [...packets(clients[1], cursors[1], 'ObjectRevived'), ...packets(clients[1], cursors[1], 'ObjectPlayer')]
      .find(event => event.payload?.objectId === context.observerId &&
        (event.packet === 'ObjectRevived' || event.payload?.name === context.names[0] && event.payload?.dead === false))
    : packets(clients[1], cursors[1], 'ObjectRemove').find(event => event.payload?.objectId === context.observerId);
  if (!observerPacket) return null;
  if (map !== context.mapFileName && !packets(owner, cursors[0], 'MapInformation').length &&
      !packets(owner, cursors[0], 'MapChanged').length) return null;
  return { actor: context.names[0], fromMap: context.mapFileName, toMap: map,
    hp: observedPlayerHp(owner.snapshot), location: point(self(owner)),
    requestReceipt: request.sequence, revivedReceipt: revived[0].sequence,
    aliveSnapshotReceipt: snapshot.sequence, observerPacket: observerPacket.packet,
    observerReceipt: observerPacket.sequence };
}

export async function prepareOrdinaryExposure(client, deadline) {
  await client.refresh();
  const armour = client.snapshot.equipmentItems?.find(item => item.slot === 'armour' || item.equipSlot === 'armour');
  if (!armour) return { armourRemoved: false, reason: 'No currently equipped armour' };
  const occupied = new Set((client.snapshot.inventoryItems ?? []).map(item => Number(item.slot)));
  const to = Array.from({ length: 40 }, (_, slot) => slot).find(slot => !occupied.has(slot));
  if (to == null) notRun('No real empty first-Bag slot for ordinary armour removal');
  const command = { type: 'removeItem', grid: 'inventory', uniqueId: armour.uniqueId, to };
  validateLifecycleCommand(command);
  const receipt = await client.request(command, 'RemoveItem', remaining(deadline, 8000));
  if (receipt.payload?.success !== true || receipt.payload.uniqueId !== armour.uniqueId || receipt.payload.to !== to) {
    throw new Error('Ordinary armour removal lacks an exact successful RemoveItem receipt');
  }
  await client.refresh();
  if (client.snapshot.equipmentItems.some(item => item.slot === 'armour' || item.equipSlot === 'armour') ||
      !client.snapshot.inventoryItems.some(item => item.uniqueId === armour.uniqueId && Number(item.slot) === to)) {
    throw new Error('Fresh snapshot did not confirm the same armour UID moved into the real empty Bag slot');
  }
  return { armourRemoved: true, uniqueId: armour.uniqueId, to, receipt: receipt.sequence };
}

export async function restoreOrdinaryExposure(client, exposure, deadline) {
  if (!exposure?.armourRemoved) return { restored: false, reason: 'No armour was removed' };
  await client.refresh();
  const item = client.snapshot.inventoryItems?.find(value => value.uniqueId === exposure.uniqueId);
  if (!item) return { restored: false, reason: 'Original armour UID is absent from the current Bag; no replacement or grant used' };
  if (hasAuthoritativePlayerDeath(client.snapshot)) throw new Error('Cannot restore armour on a corpse');
  const command = { type: 'equipItem', grid: 'inventory', uniqueId: item.uniqueId, to: 1 }, cursor = client.sequence;
  validateLifecycleCommand(command);
  const receipt = await client.request(command, 'EquipItem', remaining(deadline, 8000));
  if (receipt.sequence <= cursor || receipt.payload?.success !== true || receipt.payload.uniqueId !== item.uniqueId ||
      receipt.payload.to !== 1 || String(receipt.payload.grid).toLowerCase() !== 'inventory') {
    throw new Error('Ordinary armour restoration lacks an exact fresh EquipItem receipt');
  }
  await client.refresh();
  if (!client.snapshot.equipmentItems?.some(value => value.uniqueId === item.uniqueId &&
      (value.slot === 'armour' || value.equipSlot === 'armour')) ||
      client.snapshot.inventoryItems?.some(value => value.uniqueId === item.uniqueId)) {
    throw new Error('Fresh state did not confirm restoration of the same armour UID');
  }
  return { restored: true, uniqueId: item.uniqueId, receipt: receipt.sequence };
}

function notRun(reason) {
  const error = new Error(reason); error.code = 'LIFECYCLE_NOT_RUN'; throw error;
}
function remaining(deadline, maximum) {
  const value = Math.min(maximum, deadline - Date.now());
  if (value <= 0) notRun('Run deadline expired before this ordinary lifecycle action');
  return value;
}

export async function runLifecycleChecks({ clients, combat, loadMap, deadline, onCheck, nextCombat, onAttempt }) {
  const results = {};
  const record = async (key, result) => { results[key] = result; await onCheck?.(key, result); };
  const check = async (key, action) => {
    try { const evidence = await action(); await record(key, { status: 'passed', evidence }); return evidence; }
    catch (error) {
      await record(key, { status: error.code === 'LIFECYCLE_NOT_RUN' ? 'not_run' : 'failed',
        reason: error.message, evidence: error.evidence });
      return null;
    }
  };
  if (!combat) {
    for (const key of FULL_LIFECYCLE_CHECKS) await record(key, { status: 'not_run', reason: 'Shared combat prerequisite did not pass' });
    return results;
  }
  const navigators = clients.map(client => createNavigator(client, { loadCollisionMap: loadMap, movementResponseTimeoutMs: 5000 }));
  const navigate = async (index, target, radius, until, options = {}) => {
    const result = await navigators[index](target, radius, () => Date.now() >= until, {
      maxSuccessfulSteps: 45, maxAttempts: 60, maxNoPathRefreshes: 2,
      ...options,
      beforeMovement(details) {
        if (Date.now() >= until) throw new Error('Ordinary lifecycle navigation deadline expired');
        return options.beforeMovement?.(details);
      },
    });
    if (!result.reached) throw new Error('Ordinary lifecycle navigation did not reach its current target');
    return result;
  };
  let kill, drop, activeCombat = combat;
  for (let attempt = 1; attempt <= 3; attempt++) {
    kill = await check('sharedKill', async () => {
      const existing = sharedKillEvidence(clients, activeCombat);
      if (existing) return existing;
      const until = Date.now() + remaining(deadline, 60000);
      for (let attempt = 0; attempt < 32 && Date.now() < until; attempt++) {
        const evidence = sharedKillEvidence(clients, activeCombat);
        if (evidence) return evidence;
        const actor = attempt % 2, client = clients[actor];
        if (clients.some(value => hasAuthoritativePlayerDeath(value.snapshot))) throw new Error('Actor died before shared kill evidence completed');
        if (clients.some(value => String(value.snapshot?.mapFileName) !== activeCombat.lifecycleStart.mapFileName)) {
          throw new Error('Map changed during the shared target life');
        }
        const target = client.snapshot?.entities?.find(entity => entity.objectId === activeCombat.targetId && !entity.dead);
        if (!target) throw new Error('Shared target disappeared without fresh death receipts on both clients');
        await navigate(actor, target, 1, until); await sleep(650);
        if (distance(self(client), target) <= 1) client.send({ type: 'attack', objectId: activeCombat.targetId });
        await sleep(350);
      }
      const evidence = sharedKillEvidence(clients, activeCombat);
      if (!evidence) throw new Error('No matching shared target death within the bounded ordinary attack budget');
      return evidence;
    });
    drop = await check('sharedDrop', async () => {
      if (!kill) notRun('Shared kill did not pass');
      remaining(deadline, 10000);
      await Promise.all(clients.map(client => client.refresh()));
      const evidence = sharedDropEvidence(clients, activeCombat, kill);
      if (!evidence) throw new Error('No fresh source-name/location-correlated drop visible with matching spawn packets on both clients');
      return evidence;
    });
    await onAttempt?.({ attempt, combat: activeCombat, sharedKill: results.sharedKill, sharedDrop: results.sharedDrop });
    if (drop || !nextCombat || attempt === 3 || Date.now() >= deadline ||
        clients.some(client => hasAuthoritativePlayerDeath(client.snapshot))) break;
    try { activeCombat = await nextCombat(); }
    catch (error) {
      await onAttempt?.({ attempt: attempt + 1, search: { status: 'failed', reason: error.message } });
      break;
    }
  }
  await check('uniquePickup', async () => {
    if (!drop) notRun('Shared drop did not pass');
    const until = Date.now() + remaining(deadline, 20000);
    for (let index = 0; index < clients.length; index++) await navigate(index, drop.drop, 1, until);
    await Promise.all(clients.map(client => client.refresh()));
    const before = clients.map(client => structuredClone(client.snapshot));
    const cursors = clients.map(client => client.sequence);
    for (const client of clients) client.send({ type: 'pickUp', objectId: drop.drop.objectId });
    await clients[0].wait(() => clients.some((client, index) => packets(client, cursors[index], 'GainedGold').length ||
      packets(client, cursors[index], 'GainedItem').length), 'one contested ordinary pickup gain', remaining(until, 5000));
    await Promise.all(clients.map(client => client.refresh()));
    const evidence = uniquePickupEvidence(clients, cursors, before, drop.drop);
    if (!evidence) throw new Error('Pickup lacks one exact gain, shared removal, or conserved carried state');
    const replayBefore = clients.map(client => structuredClone(client.snapshot)), replayCursors = clients.map(client => client.sequence);
    for (const client of clients) client.send({ type: 'pickUp', objectId: drop.drop.objectId });
    await sleep(500);
    await Promise.all(clients.map(client => client.refresh()));
    const replay = pickupReplayEvidence(clients, replayCursors, replayBefore, drop.drop);
    if (!replay || replay.freshSnapshotSequences.some(value => !Number.isInteger(value))) {
      throw new Error('Repeated pickup changed carried state, awarded twice, or lacks fresh snapshot barriers');
    }
    return { ...evidence, replay, replayObservationWindowMs: 500 };
  });
  let deathContext;
  const death = await check('playerDeath', async () => {
    if (clients.some(client => hasAuthoritativePlayerDeath(client.snapshot))) notRun('A participant was already dead before the ordinary encounter phase');
    if (clients.some(client => observedPlayerHp(client.snapshot) <= 0)) notRun('A participant lacks a confirmed living owner state before the encounter');
    const map = String(clients[0].snapshot?.mapFileName);
    if (String(clients[1].snapshot?.mapFileName) !== map) notRun('Participants are not on the same map');
    const observerId = clients[1].snapshot?.entities?.find(entity => entity.name === self(clients[0])?.name)?.objectId;
    if (!id(observerId)) notRun('Observer does not currently see the ordinary victim actor');
    deathContext = { names: clients.map(client => self(client).name), cursors: clients.map(client => client.sequence),
      observerId, mapFileName: map, initialAlive: true };
    const until = Date.now() + remaining(deadline, 330000), approachUntil = Math.min(until, Date.now() + 150000);
    const exposure = { navigation: [], unavailableHints: [], provocations: [], samples: [], supplies: [] };
    const proof = () => playerDeathEvidence(clients, deathContext);
    const refresh = async () => {
      await Promise.all(clients.map(client => client.refresh()));
      if (clients.some(client => client.failure || client.closed)) throw new Error('Participant transport failed during ordinary exposure');
      if (clients.some(client => String(client.snapshot?.mapFileName) !== map)) throw new Error('Ordinary exposure unexpectedly changed map');
      if (hasAuthoritativePlayerDeath(clients[1].snapshot)) throw new Error('The observing participant died during ordinary exposure');
    };
    const move = async (index, target, radius = 0, options = {}) => {
      const plan = planExposureSegment(clients[index].snapshot, await loadMap(map), target, radius);
      if (!plan) return false;
      const cursor = clients[index].sequence, before = point(self(clients[index]));
      try { await navigate(index, target, radius, until, { autoUseSupplies: false, maxSuccessfulSteps: 80, maxAttempts: 120, ...options }); }
      catch (error) {
        exposure.navigation.push({ actor: index, target, radius, fromSequence: cursor, throughSequence: clients[index].sequence,
          before, after: point(self(clients[index])), error: error.message, preflightPath: plan.path });
        if (!proof()) throw error;
        return true;
      }
      exposure.navigation.push({ actor: index, target, radius, fromSequence: cursor, throughSequence: clients[index].sequence,
        before, after: point(self(clients[index])), preflightPath: plan.path });
      return true;
    };
    const followObserver = async () => {
      if (proof() || hasAuthoritativePlayerDeath(clients[0].snapshot)) return;
      const cats = [...new Map(clients.flatMap(client => lifecycleExposureTargets(client.snapshot))
        .map(cat => [cat.objectId, cat])).values()];
      if (!cats.length) {
        if (distance(self(clients[0]), self(clients[1])) > 6) {
          await navigate(1, self(clients[0]), 6, until, { autoUseSupplies: false });
        }
        return;
      }
      const observerPlan = planExposureObserver(clients[1].snapshot, self(clients[0]), cats, await loadMap(map));
      if (!observerPlan) throw new Error('No current walkable observer position outside cats\' acquisition range and within AOI');
      if (observerPlan.plan.steps.length && !await move(1, observerPlan.target, 0, {
        beforeMovement({ from, physicalCells }) {
          const currentCats = [...new Map(clients.flatMap(client => lifecycleExposureTargets(client.snapshot))
            .map(cat => [cat.objectId, cat])).values()];
          for (const cat of currentCats) for (const cell of physicalCells) {
            if (outsideExposureAggro(from, [cat]) ? !outsideExposureAggro(cell, [cat]) : distance(cell, cat) < distance(from, cat)) {
              throw new Error('Observer movement would approach or reenter a current cat acquisition rectangle');
            }
          }
        },
      })) throw new Error('The safe observer destination became unavailable before movement');
    };
    const advance = async (target, radius = 0) => {
      const plan = planExposureSegment(clients[0].snapshot, await loadMap(map), target, radius);
      if (!plan) return false;
      if (plan.path.length > 1 && !await move(0, plan.path[Math.min(3, plan.path.length - 1)])) return false;
      await refresh(); await followObserver(); return true;
    };
    try {
      exposure.armour = await prepareOrdinaryExposure(clients[0], until);
      const potion = await useHeldExposureHpPotion(clients[0], until);
      if (potion) exposure.supplies.push(potion);
      await refresh();
      if (!lifecycleExposureTargets(clients[0].snapshot).length && !proof()) {
        const entry = exposureCorridorEntry(clients[0].snapshot, await loadMap(map));
        if (!entry) notRun('No currently observed ordinary cat and no bounded map-0 western corridor path; no privileged fallback');
        exposure.corridorEntry = { index: entry.index, preflightPath: entry.plan.path };
        for (const target of WESTERN_EXPOSURE_CORRIDOR.slice(entry.index)) {
          if (proof() || lifecycleExposureTargets(clients[0].snapshot).some(cat => distance(self(clients[0]), cat) <= 6)) break;
          const occupied = exposureObstacles(clients[0].snapshot).some(entity => samePoint(point(entity), target)), radius = occupied ? 1 : 0;
          for (let segment = 0; segment < 27 && distance(self(clients[0]), target) > radius && Date.now() < approachUntil; segment++) {
            if (proof() || !await advance(target, radius)) {
              if (!proof()) exposure.unavailableHints.push({ target, radius, reason: 'No complete path against current public occupancy' });
              break;
            }
          }
        }
      }
      if (!proof() && !lifecycleExposureTargets(clients[0].snapshot).length) notRun('No fresh ordinary live cats after the bounded western approach');
      const provoked = new Set(), exposureUntil = Math.min(until, Date.now() + 180000);
      while (Date.now() < exposureUntil) {
        if (proof()) return { ...proof(), exposure, causeScope: 'Ordinary cat exposure; public Death does not identify a unique killing source.' };
        await refresh();
        if (proof()) break;
        if (hasAuthoritativePlayerDeath(clients[0].snapshot)) {
          await clients[0].wait(proof, 'owner Death and the same observer ObjectDied', remaining(exposureUntil, 3000));
          break;
        }
        await followObserver();
        const target = lifecycleExposureTargets(clients[0].snapshot).find(cat => !provoked.has(cat.objectId));
        if (target && provoked.size < 2) {
          for (let segment = 0; segment < 12 && !proof() && Date.now() < exposureUntil; segment++) {
            const current = lifecycleExposureTargets(clients[0].snapshot).find(cat => cat.objectId === target.objectId);
            if (!current || distance(self(clients[0]), current) <= 1 || !await advance(current, 1)) break;
          }
          await refresh();
          const current = lifecycleExposureTargets(clients[0].snapshot).find(cat => cat.objectId === target.objectId && distance(self(clients[0]), cat) <= 1);
          if (current && !hasAuthoritativePlayerDeath(clients[0].snapshot)) {
            const command = { type: 'attack', objectId: current.objectId };
            validateLifecycleCommand(command); clients[0].send(command); provoked.add(current.objectId);
            exposure.provocations.push({ objectId: current.objectId, name: current.name, location: point(current),
              actor: point(self(clients[0])), commandSequence: clients[0].sequence });
            await onAttempt?.({ phase: 'ordinary-cat-exposure', evidence: structuredClone(exposure) });
          }
        }
        exposure.samples.push({ cursors: clients.map(client => client.sequence), hp: clients.map(client => observedPlayerHp(client.snapshot)),
          actor: point(self(clients[0])), observer: point(self(clients[1])), currentCats: lifecycleExposureTargets(clients[0].snapshot).map(cat => ({ objectId: cat.objectId, ...point(cat) })) });
        if (proof()) break;
        await sleep(500);
      }
      const evidence = proof();
      if (!evidence) throw new Error('No paired owner Death and observer ObjectDied within the bounded ordinary two-cat exposure');
      return { ...evidence, exposure, causeScope: 'Ordinary cat exposure; public Death does not identify a unique killing source.' };
    } catch (error) { error.evidence = { ...exposure, ...(error.evidence ?? {}) }; throw error; }
  });
  await check('townRevive', async () => {
    if (!death) notRun('No passed player death evidence; townRevive is never sent to a living actor');
    const cursors = clients.map(client => client.sequence), until = Date.now() + remaining(deadline, 10000);
    await clients[0].request({ type: 'townRevive' }, 'Revived', remaining(until, 8000));
    await Promise.all(clients.map(client => client.refresh()));
    const evidence = await clients[0].wait(() => townReviveEvidence(clients, deathContext, cursors),
      'fresh revived owner state and source-map observer lifecycle', remaining(until, 8000));
    const armourRestoration = await restoreOrdinaryExposure(clients[0], death.exposure.armour,
      Date.now() + remaining(deadline, 10000));
    return { ...evidence, armourRestoration };
  });
  return results;
}
