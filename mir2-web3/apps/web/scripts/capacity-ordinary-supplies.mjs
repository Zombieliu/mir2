import { performance } from 'node:perf_hooks';
import { collectSafeFundingVenison } from './quest-agent/protocol-funding.mjs';
import { harvestDirection } from './quest-agent/protocol-combat.mjs';
import { hasAuthoritativePlayerDeath, observedPlayerHp } from './quest-agent/protocol-observation.mjs';

export const ORDINARY_SUPPLY_COMMANDS = Object.freeze(['harvest', 'interact', 'selectNpcDialog', 'sellItem', 'buyItem']);
const ALLOWED = new Set(['clientVersion', 'keepAlive', 'walk', 'run', 'turn', 'attack', 'attackV2', 'harvest', 'pickUp', ...ORDINARY_SUPPLY_COMMANDS]);
const JOHN = Object.freeze({ name: 'Merchant_John', x: 292, y: 603 });
const RUBEN = Object.freeze({ name: 'Merchant_Ruben', x: 288, y: 608 });
const HP = '(HP)DrugSmall';
const sleepDefault = ms => new Promise(resolve => setTimeout(resolve, ms));
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const integer = (value, label, min = 0, max = Number.MAX_SAFE_INTEGER) => {
  if (!Number.isSafeInteger(value) || value < min || value > max) throw new Error(`ordinary supplies: invalid ${label}`);
  return value;
};
const template = item => Number(item?.itemIndex ?? item?.item_index ?? (item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info)?.item_index ?? (item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info)?.itemIndex);
const owner = snapshot => (snapshot?.entities ?? []).find(entity => String(entity.objectId) === String(snapshot?.playerObjectId));
const quantity = item => integer(Number(item.quantity), 'held quantity', 1);
const isHp = item => template(item) === 658 && item?.name === HP;
const held = snapshot => [...(snapshot?.inventoryItems ?? []), ...(snapshot?.beltItems ?? [])];
const hpStock = snapshot => {
  const ids = new Set();
  return held(snapshot).filter(isHp).reduce((sum, item) => {
    const id = integer(Number(item.uniqueId), 'held HP uniqueId');
    if (ids.has(id)) throw new Error('ordinary supplies: duplicate held HP identity');
    ids.add(id); return sum + quantity(item);
  }, 0);
};
const venison = snapshot => (snapshot?.inventoryItems ?? []).filter(item => item?.name === 'Venison');
const venisonStock = snapshot => venison(snapshot).reduce((sum, item) => sum + quantity(item), 0);
const point = (value, label) => ({ x: integer(value?.x, `${label}.x`, 0, 65535), y: integer(value?.y, `${label}.y`, 0, 65535) });

function roomForHp(snapshot) {
  const capacity = Number(snapshot.maxBagSlots ?? Number(snapshot.inventoryCapacity) - 6);
  integer(capacity, 'authoritative bag capacity', 1, 80);
  const occupied = new Set();
  let room = 0;
  for (const item of snapshot.inventoryItems ?? []) {
    const slot = integer(Number(item.slot), 'inventory slot', 0, 39);
    const offset = item.container === 'bag1' ? 0 : item.container === 'bag2' ? 40 : NaN;
    integer(offset, 'inventory container', 0, 40);
    if (slot + offset >= capacity || occupied.has(slot + offset)) throw new Error('ordinary supplies: invalid occupied bag slot');
    occupied.add(slot + offset);
    if (isHp(item)) room += Math.max(0, 20 - quantity(item));
  }
  for (const item of snapshot.beltItems ?? []) if (isHp(item)) room += Math.max(0, 20 - quantity(item));
  return room + (capacity - occupied.size) * 20;
}

/**
 * Explicit, quiescent map-0 preparation. No activation, revival, travel command,
 * measurement resupply or direct transport access. The caller supplies ordinary
 * navigation using the facade passed to navigate, and calls observe AFTER its
 * normal receiver has produced the actual sequence/monotonic receipt.
 */
export function createOrdinarySupplies(client, options = {}) {
  const enabled = options.preparationProfile === 'ordinary-economic-v1' || (options.targetHp ?? 0) > 0;
  const targetHp = integer(options.targetHp ?? (enabled ? 12 : 0), 'targetHp', enabled ? 1 : 0, 40);
  if (options.preparationProfile != null && !['held-only', 'ordinary-economic-v1'].includes(options.preparationProfile)) throw new Error('ordinary supplies: unknown preparation profile');
  if (options.preparationProfile === 'held-only' && targetHp !== 0) throw new Error('ordinary supplies: held-only cannot purchase');
  const now = options.now ?? (() => performance.now());
  const sleep = options.sleep ?? sleepDefault;
  const timeoutMs = integer(options.timeoutMs ?? 600_000, 'timeoutMs', 1, 600_000);
  const evidenceMs = integer(options.evidenceMs ?? 12_000, 'evidenceMs', 1, 12_000);
  const maxCommands = integer(options.maxCommands ?? 2048, 'maxCommands', 1, 4096);
  const maxNavigationCommands = integer(options.maxNavigationCommands ?? 2048, 'maxNavigationCommands', 1, 4096);
  const maxHunts = integer(options.maxHunts ?? 8, 'maxHunts', 1, 8);
  const maxEmptySearchRefreshes = integer(options.maxEmptySearchRefreshes ?? 20, 'maxEmptySearchRefreshes', 1, 20);
  const rows = [], proofs = [], commandRows = [], queue = [];
  const counts = {};
  let state = enabled ? 'idle' : 'disabled', active = false, used = false, draining = false;
  let started = null, deadline = null, identity = null, before = null, after = null, failure = null;
  let lastObserved = -1, lastObservedMono = -Infinity, commands = 0, movementCommands = 0, pendingSaleId = null, lastSent = null, observationFault = null;
  let funding = null, sold = 0, bought = 0, goldEarned = 0, goldSpent = 0, actualStagingReached = false;

  function scalar(snapshot) {
    const actor = owner(snapshot);
    if (!actor) throw new Error('ordinary supplies: missing owner');
    const hp = observedPlayerHp(snapshot), maxHp = Number(snapshot.playerMaxHp ?? actor.maxHp);
    if (!Number.isFinite(hp) || hp <= 0 || !Number.isFinite(maxHp) || maxHp <= 0 || hp > maxHp || hasAuthoritativePlayerDeath(snapshot)) throw new Error('ordinary supplies: dead or invalid owner health');
    return { ownerId: integer(Number(snapshot.playerObjectId), 'ownerId', 1), map: String(snapshot.mapFileName), ...point(actor, 'owner'), hp, maxHp,
      gold: integer(Number(snapshot.gold), 'gold'), hpStock: hpStock(snapshot), venison: venisonStock(snapshot),
      saleRemaining: pendingSaleId == null ? null : venison(snapshot).filter(item => Number(item.uniqueId) === pendingSaleId).reduce((sum, item) => sum + quantity(item), 0),
      npcId: Number(snapshot.activeNpcDialog?.npcObjectId) || null,
      buyLink: (snapshot.activeNpcDialog?.links ?? []).some(link => String(link?.target).toLowerCase() === '@buysell' && link.disabled !== true && link.enabled !== false),
      sellLink: (snapshot.activeNpcDialog?.links ?? []).some(link => ['@sell', '@buysell'].includes(String(link?.target).toLowerCase()) && link.disabled !== true && link.enabled !== false) };
  }
  function guard() {
    if (!active || !options.isPreparation?.()) throw new Error('ordinary supplies: quiescent preparation required');
    if (now() >= deadline) throw new Error('ordinary supplies: preparation wall budget exhausted');
    if (client.failure) throw client.failure;
    if (client.closed) throw new Error('ordinary supplies: transport closed');
    if (observationFault) throw new Error(observationFault);
    const value = scalar(client.snapshot);
    if (value.map !== '0' || value.ownerId !== identity || value.hp / value.maxHp < 0.7) throw new Error('ordinary supplies: owner/map/health changed');
    return value;
  }
  function observe(message, receipt) {
    if ((!active && state !== 'ready') || !options.isPreparation?.() || receipt?.direction !== 'received' || !Number.isSafeInteger(receipt.sequence) || !Number.isFinite(receipt.monotonicMs)) return;
    if (['MapChanged', 'MapInformation'].includes(message?.packet)) observationFault = 'ordinary supplies: map transition during preparation';
    if (message?.type !== 'worldSnapshot') return;
    if (receipt.sequence <= lastObserved || receipt.monotonicMs < lastObservedMono) return;
    lastObserved = receipt.sequence; lastObservedMono = receipt.monotonicMs;
    try {
      const value = scalar(message.payload);
      if (value.ownerId !== identity || value.map !== '0') observationFault = 'ordinary supplies: received owner/map changed';
      rows.push({ sequence: receipt.sequence, monotonicMs: receipt.monotonicMs, ...value });
      if (rows.length > 64) rows.shift();
    } catch { /* Incomplete/invalid observations cannot satisfy a proof. guard still checks live state. */ }
  }
  async function pause(ms) {
    await drain(); guard();
    if (ms >= deadline - now()) throw new Error('ordinary supplies: preparation wall budget exhausted');
    await sleep(ms); guard();
  }
  function enqueue(command) {
    guard();
    if (!command || !ALLOWED.has(command.type)) throw new Error('ordinary supplies: forbidden command');
    if (['attack', 'attackV2'].includes(command.type)) {
      const target = (client.snapshot.entities ?? []).find(entity => entity.objectId === command.objectId);
      if (String(target?.kind).toLowerCase() !== 'monster' || target.name !== 'Deer' || target.dead || !(target.hp > 0)) throw new Error('ordinary supplies: only live Deer may be hunted');
    }
    if (command.type === 'harvest' && !(client.snapshot.entities ?? []).some(entity => entity.name === 'Deer' && entity.dead === true && distance(owner(client.snapshot), entity) === 1 && harvestDirection(owner(client.snapshot), entity) === command.direction)) throw new Error('ordinary supplies: only adjacent Deer corpse may be harvested');
    if (command.type === 'pickUp' && !(client.snapshot.groundDrops ?? []).some(drop => drop.objectId === command.objectId && drop.name === 'Venison' && distance(owner(client.snapshot), drop) === 0)) throw new Error('ordinary supplies: only same-tile visible Venison may be picked up');
    if (queue.length >= 8) throw new Error('ordinary supplies: send queue limit');
    queue.push({ ...command });
  }
  async function drain() {
    if (draining) throw new Error('ordinary supplies: reentrant send drain');
    draining = true;
    try {
      while (queue.length) {
        guard();
        await client.pace(); guard();
        let command = queue.shift();
        if (command.type === 'attack' && client.attackResults === 'v2') {
          if (typeof client.nextAttackV2 !== 'function') throw new Error('ordinary supplies: v2 physical attack unavailable');
          command = client.nextAttackV2(command.objectId);
        }
        if (commands >= maxCommands || (['walk', 'run'].includes(command.type) && movementCommands >= maxNavigationCommands)) throw new Error('ordinary supplies: command budget exhausted');
        const sentAtMs = now(), sendCursor = client.sequence;
        client.send(command); commands++; counts[command.type] = (counts[command.type] ?? 0) + 1;
        lastSent = { cursor: sendCursor, sentAtMs, deadline: Math.min(deadline, sentAtMs + evidenceMs) };
        if (['walk', 'run'].includes(command.type)) movementCommands++;
        if (commandRows.length < 32) commandRows.push({ type: command.type, sendCursor, sentAtMs });
      }
    } finally { draining = false; }
  }
  async function wait(predicate, label, ms = evidenceMs) {
    await drain(); guard();
    const stop = Math.min(deadline, now() + Math.min(ms, evidenceMs));
    return client.wait(() => { guard(); if (now() > stop) throw new Error(`ordinary supplies: evidence expired (${label})`); return predicate(); }, label, Math.max(1, stop - now()));
  }
  async function transmit(command) {
    await drain(); guard(); enqueue(command); await drain(); return { ...lastSent };
  }
  const fresh = (row, mark) => row.sequence > mark.cursor && row.monotonicMs >= mark.sentAtMs && row.monotonicMs <= mark.deadline && row.ownerId === identity && row.map === '0';
  async function packet(command, packetName) {
    const mark = await transmit(command);
    return wait(() => (client.events ?? []).find(event => event.direction === 'received' && event.packet === packetName && fresh({ ...event, ownerId: identity, map: '0' }, mark)), packetName);
  }
  async function refresh() {
    const mark = await transmit({ type: 'clientVersion' });
    return wait(() => rows.find(row => fresh(row, mark)), 'fresh ordinary supply snapshot');
  }
  const facade = new Proxy(client, { get(target, key) {
    if (key === 'send') return enqueue;
    if (key === 'wait') return wait;
    if (key === 'request') return packet;
    if (key === 'pace') return async () => { await drain(); guard(); await client.pace(); guard(); };
    if (key === 'record') return () => {}; // Existing helper diagnostics must not expand retained payloads.
    const value = Reflect.get(target, key, target);
    return typeof value === 'function' ? value.bind(target) : value;
  } });
  async function navigate(target, range, stop, navOptions) {
    await drain(); guard();
    const pickupTile = (client.snapshot.groundDrops ?? []).some(drop => drop.objectId === target?.objectId && drop.name === 'Venison');
    await options.navigate(facade, target, pickupTile ? 0 : range, stop, navOptions);
    await drain(); guard();
  }
  function liveNpc(spec) {
    const actor = owner(client.snapshot);
    return (client.snapshot.entities ?? []).find(entity => String(entity.kind).toLowerCase() === 'npc' && entity.name === spec.name && entity.x === spec.x && entity.y === spec.y && Number.isSafeInteger(entity.objectId) && entity.objectId > 0 && distance(actor, entity) <= 16);
  }
  async function shop(spec, sale = false) {
    await navigate(spec, 16); const npc = liveNpc(spec);
    if (!npc) throw new Error('ordinary supplies: exact live merchant unavailable');
    const mark = await transmit({ type: 'interact', objectId: npc.objectId });
    await wait(() => rows.find(row => fresh(row, mark) && row.npcId === npc.objectId && (sale ? row.sellLink : row.buyLink)), 'fresh merchant dialog');
    const link = client.snapshot.activeNpcDialog?.links?.find(item => (sale ? ['@sell', '@buysell'] : ['@buysell']).includes(String(item.target).toLowerCase()) && item.disabled !== true && item.enabled !== false);
    if (!link || !liveNpc(spec) || Number(client.snapshot.activeNpcDialog.npcObjectId) !== npc.objectId) throw new Error('ordinary supplies: merchant dialog changed');
    const result = await packet({ type: 'selectNpcDialog', target: link.target }, sale ? 'NPCSell' : 'NPCGoods');
    if (!liveNpc(spec) || Number(client.snapshot.activeNpcDialog.npcObjectId) !== npc.objectId) throw new Error('ordinary supplies: merchant changed during service');
    if (sale) return null;
    if (Number(result.payload?.panelType) !== 0 || !Array.isArray(result.payload?.list)) throw new Error('ordinary supplies: invalid goods panel');
    const row = result.payload.list.find(item => template(item) === 658 && item.name === HP);
    if (!row) throw new Error('ordinary supplies: missing exact HP goods row');
    const id = row.id ?? row.uniqueId ?? row.unique_id;
    if (!((typeof id === 'number' && Number.isSafeInteger(id) && id >= 0) || (typeof id === 'string' && /^(0|[1-9]\d*)$/.test(id) && BigInt(id) <= 18446744073709551615n))) throw new Error('ordinary supplies: invalid live shop row id');
    return { id, price: integer(Number(row.price), 'live HP price', 1) };
  }
  async function sellUntil(requiredGold) {
    if (guard().gold >= requiredGold || !venison(client.snapshot).length) return;
    state = 'selling'; await shop(JOHN, true);
    // Never sell starter equipment, quest items, potions or any other material.
    for (let sale = 0; sale < maxHunts + 2 && guard().gold < requiredGold; sale++) {
      const item = venison(client.snapshot)[0]; if (!item) break;
      const id = integer(Number(item.uniqueId), 'Venison uniqueId', 1), count = quantity(item), price = integer(Number(item.sellValue), 'live Venison sellValue', 1);
      pendingSaleId = id; const initial = guard();
      const mark = await transmit({ type: 'sellItem', uniqueId: id, count });
      const ack = await wait(() => (client.events ?? []).find(event => event.direction === 'received' && event.packet === 'SellItem' && fresh({ ...event, ownerId: identity, map: '0' }, mark) && event.payload?.success === true && Number(event.payload.uniqueId) === id && Number(event.payload.count) === count), 'fresh exact Venison sale ACK');
      const proof = await wait(() => rows.find(row => fresh(row, mark) && row.gold === initial.gold + price * count && row.venison === initial.venison - count && row.saleRemaining === 0), 'fresh exact Venison sale');
      proofs.push({ kind: 'sale', sequence: proof.sequence, monotonicMs: proof.monotonicMs, ackSequence: ack.sequence, quantity: count, gold: price * count });
      sold += count; goldEarned += price * count; pendingSaleId = null;
    }
  }
  function result() {
    return { profile: enabled ? 'ordinary-economic-v1' : 'held-only', status: state, targetHp, before, after, actualStagingReached,
      commands, movementCommands, commandCounts: { ...counts }, commandRows: [...commandRows], commandRowsTruncated: commands > commandRows.length,
      purchased: bought, soldVenison: sold, goldEarned, goldSpent, funding, proofs: [...proofs], elapsedMs: started == null ? 0 : Math.max(0, now() - started), failure,
      commandCountsScope: 'module-owned commands; external keepAlive shares the original client budget',
      measurementCredit: false, sustainableDurationProven: false };
  }
  function assertReady({ afterSequence = -1, maxAgeMs = evidenceMs } = {}) {
    integer(afterSequence, 'fresh ready cursor', -1); integer(maxAgeMs, 'ready freshness', 1, evidenceMs);
    if (state !== 'ready' || !options.isPreparation?.() || observationFault) throw new Error('ordinary supplies: cohort readiness unavailable');
    const value = scalar(client.snapshot), staging = point(options.stagingPoint, 'explicit stagingPoint');
    const receipt = rows.findLast(row => row.sequence > afterSequence && row.monotonicMs <= now() && row.monotonicMs >= now() - maxAgeMs && row.ownerId === identity && row.map === '0' && row.hpStock >= targetHp && row.x === staging.x && row.y === staging.y);
    if (!receipt || value.ownerId !== identity || value.map !== '0' || value.x !== staging.x || value.y !== staging.y || value.hpStock < targetHp || value.hp / value.maxHp < 0.7 || client.closed || client.failure) throw new Error('ordinary supplies: fresh cohort readiness failed');
    if ((client.snapshot.entities ?? []).some(entity => String(entity.kind).toLowerCase() === 'monster' && String(entity.disposition).toLowerCase() === 'hostile' && !entity.dead && Number(entity.hp) > 0 && distance(staging, entity) <= 8)) throw new Error('ordinary supplies: hostile near staging');
    return { ...value, sequence: receipt.sequence, monotonicMs: receipt.monotonicMs, actualStagingReached: true };
  }
  async function prepare() {
    if (!enabled) return result();
    if (used) throw new Error('ordinary supplies: preparation is single-use');
    used = true;
    started = now(); deadline = started + timeoutMs; active = true; state = 'refreshing';
    try {
      if (typeof client?.pace !== 'function' || typeof client?.send !== 'function' || typeof client?.wait !== 'function' || typeof options.navigate !== 'function' || typeof options.isPreparation !== 'function') throw new Error('ordinary supplies: required client/navigation/preparation adapters missing');
      const staging = point(options.stagingPoint, 'explicit stagingPoint');
      identity = integer(Number(client.snapshot?.playerObjectId), 'initial ownerId', 1);
      before = await refresh(); guard();
      if (before.hpStock < targetHp) {
        let goods = await shop(RUBEN), requiredGold = (targetHp - guard().hpStock) * goods.price;
        await sellUntil(requiredGold);
        if (guard().gold < requiredGold) {
          if (!options.route || !Array.isArray(options.route.quests)) throw new Error('ordinary supplies: authoritative Deer route required');
          if (client.attackResults === 'v2' && (client.attackV2Supported !== true || typeof client.nextAttackV2 !== 'function')) throw new Error('ordinary supplies: v2 physical attack unavailable');
          state = 'funding';
          // 200 is only a bounded hunt-count estimate. Actual sale values and
          // live shop prices decide readiness; insufficient proceeds fail.
          const wanted = venisonStock(client.snapshot) + Math.max(1, Math.ceil((requiredGold - guard().gold) / 200));
          funding = await collectSafeFundingVenison(facade, { route: options.route, requiredCount: wanted, maxHunts, maxEmptySearchRefreshes,
            travel: async map => { if (map !== '0') throw new Error('ordinary supplies: map travel forbidden'); guard(); },
            navigate, sleep: pause, refreshWhileWaiting: refresh, sustain: async () => { await drain(); guard(); } });
          await refresh(); await sellUntil(requiredGold);
        }
        goods = await shop(RUBEN); state = 'buying';
        while (guard().hpStock < targetHp) {
          const initial = guard(), count = Math.min(20, targetHp - initial.hpStock);
          if (roomForHp(client.snapshot) < count) throw new Error('ordinary supplies: inventory full');
          if (initial.gold < goods.price * count) throw new Error('ordinary supplies: insufficient earned gold');
          const mark = await transmit({ type: 'buyItem', itemIndex: goods.id, count, panelType: 0 });
          const proof = await wait(() => rows.find(row => fresh(row, mark) && row.gold === initial.gold - goods.price * count && row.hpStock === initial.hpStock + count && row.venison === initial.venison), 'fresh exact HP purchase');
          proofs.push({ kind: 'purchase', sequence: proof.sequence, monotonicMs: proof.monotonicMs, quantity: count, gold: goods.price * count });
          bought += count; goldSpent += goods.price * count;
        }
      }
      state = 'staging'; await navigate(staging, 0); after = await refresh(); guard();
      if (after.x !== staging.x || after.y !== staging.y || after.hpStock < targetHp) throw new Error('ordinary supplies: staging/stock not ready');
      if ((client.snapshot.entities ?? []).some(entity => String(entity.kind).toLowerCase() === 'monster' && String(entity.disposition).toLowerCase() === 'hostile' && !entity.dead && Number(entity.hp) > 0 && distance(staging, entity) <= 8)) throw new Error('ordinary supplies: hostile near staging');
      actualStagingReached = true; state = 'ready'; return result();
    } catch (error) {
      state = 'failed'; failure = String(error?.message ?? '').startsWith('ordinary supplies:') ? error.message : 'ordinary supplies: underlying ordinary operation failed'; throw error;
    } finally { active = false; queue.length = 0; pendingSaleId = null; }
  }
  return Object.freeze({ prepare, observe, assertReady, status: result, dispose() { active = false; rows.length = 0; queue.length = 0; } });
}
