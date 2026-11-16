// External prepared-state mechanics cohort. No QA/debug/admin requests.
// Credentials arrive only over the owned wrapper's stdin, never argv/files.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import { parseArgs } from 'node:util';
import { ProtocolClient, delay } from './vendor/protocol-client.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath, protocolMapCellIsWalkable } from './vendor/protocol-navigation.mjs';
import { observedPlayerHp, hasAuthoritativePlayerDeath } from './vendor/protocol-observation.mjs';
import { selectOrdinaryTransfer, assertOrdinaryTransferCurrent } from './ordinary_transfer.mjs';
import { DELTAS, ACCEPTANCE_FALSE, MOVE_BLOCK_MASK, CAST_BLOCK_MASK, self, dist, facing, location, ownerState, liveBlockers, qualifySeven, escapeProof, driftProof, holdStopProof, eightOccupancy, rejectionProof, deathProof, deadActionProof, castCommitProof, prematureCastProof, applyDenseObservation, ordinaryMagicEligibility } from './dense_observation.mjs';

const ROOT = import.meta.dirname, ELF_SHA = '1ed52738885292b734c4055a9fd7d585b79d519779084bca2a617d0a9f5eb9ab';
const WebSocketTransport = createRequire(import.meta.url)('./vendor/ws.cjs');
const SECRET_KEY = /password|secret|pepper|token|authorization|cookie|private.?key|mac.?key/i;
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const eventAfter = (c, seq, packet, pred = () => true) => c.events.find(e => e.sequence > seq && e.direction === 'received' && e.packet === packet && pred(e.payload ?? {}));
const stamp = () => Date.now();
export function redact(value, secrets = []) {
  if (Array.isArray(value)) return value.map(v => redact(v, secrets));
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, SECRET_KEY.test(k) ? '[redacted]' : redact(v, secrets)]));
  if (typeof value === 'string') for (const secret of secrets) if (secret) value = value.split(secret).join('[redacted]');
  return value;
}
function remember(value, secrets) {
  if (!value || typeof value !== 'object') return;
  for (const [k, v] of Object.entries(value)) if (SECRET_KEY.test(k) && typeof v === 'string' && v && !secrets.includes(v)) secrets.push(v); else remember(v, secrets);
}
function fingerprint(c) {
  const items = field => (c.snapshot?.[field] ?? []).map(i => ({ uniqueId: String(i.uniqueId), name: i.name, key: i.key, count: Number(i.quantity ?? i.count ?? 1), slot: String(i.slot ?? i.equipSlot ?? ''), durability: i.durabilityCurrent ?? null })).sort((a, b) => a.uniqueId.localeCompare(b.uniqueId));
  return { ...location(c.snapshot), class: self(c.snapshot)?.class, level: self(c.snapshot)?.level, gold: c.snapshot?.gold, dead: self(c.snapshot)?.dead === true, knownSpells: (c.snapshot?.knownSkills ?? []).map(s => s.spell ?? s.magicName ?? s.name).sort(), inventory: items('inventoryItems'), equipment: items('equipmentItems'), belt: items('beltItems') };
}
class DenseClient extends ProtocolClient {
  constructor(url, output, secrets) { super(url, output); this.secrets = secrets; this.inGame = false; this.lastMedicineMs = -Infinity; }
  record(kind, value) {
    remember(value, this.secrets);
    const safe = redact(value, this.secrets);
    const event = { ...safe, movementDirection: safe.direction, sequence: ++this.sequence, tsMs: stamp(), at: new Date().toISOString(), direction: kind, state: ownerState(this.snapshot, value?.payload?.objectId) };
    this.events.push(event); // No ring buffer: every failure and late receipt is retained.
    this.writeQueue = this.writeQueue.then(() => fs.appendFile(this.output, JSON.stringify(event) + '\n'));
    this.writeQueue.catch(e => { this.failure = e; });
    return event;
  }
  // Override the base implementation so nested payload secrets are remembered
  // before its shallow redaction would otherwise discard the password string.
  send(command) { remember(command, this.secrets); return super.send(command); }
  observeGatewayMessage(message) {
    this.snapshot = applyDenseObservation(this.snapshot, message, stamp());
    this.record('received', message);
    if (message.type === 'error') this.failure = new Error(`Gateway rejected command: ${message.message ?? 'unknown error'}`);
    if (message.packet === 'DamageIndicator' && Number(message.payload?.objectId) === Number(this.snapshot?.playerObjectId)) this.scheduleDamageVitalsRefresh();
  }
  async connect() {
    const u = new URL(this.url); u.protocol = 'http:';
    this.ws = new WebSocketTransport(this.url, { origin: u.origin, handshakeTimeout: 10000 });
    this.ws.addEventListener('message', e => { try { this.observeGatewayMessage(JSON.parse(e.data)); } catch (error) { this.failure = error; } });
    this.ws.addEventListener('error', () => { this.failure = new Error('Owned loopback WebSocket failed'); });
    this.ws.addEventListener('close', () => { this.closed = true; });
    await this.wait(() => this.ws.readyState === 1, 'WebSocket open', 10000);
    await this.request({ type: 'clientVersion' }, 'ClientVersion');
    this.keepalive = setInterval(() => { if (this.ws.readyState === 1) this.send({ type: 'keepAlive', time: stamp() }); }, 2000);
  }
  async refresh() { const seq = this.sequence; this.send({ type: 'clientVersion' }); await this.wait(() => this.events.find(e => e.sequence > seq && e.type === 'worldSnapshot'), 'fresh ordinary snapshot', 5000); }
  async logout() { const ack = await this.request({ type: 'logOut' }, 'LogOutSuccess', 20000); this.inGame = false; return ack; }
  async close() {
    clearInterval(this.keepalive);
    if (this.damageVitalsRefreshTimer != null) this.cancelTimeout(this.damageVitalsRefreshTimer);
    const original = this.failure; this.failure = null;
    try { if (this.ws?.readyState === 1 && this.inGame) await this.logout(); } finally { this.failure ??= original; this.ws?.close(); }
    const deadline = stamp() + 5000;
    while (this.ws && this.ws.readyState !== 3 && stamp() < deadline) await delay(25);
    if (this.ws && this.ws.readyState !== 3) { this.ws.terminate(); throw new Error('Owned QA socket normal close timed out'); }
    await this.writeQueue;
  }
}
async function start(c, account, fresh = false) {
  if (fresh && (await c.request({ type: 'newAccount', accountId: account.accountId, password: account.password }, 'NewAccount')).payload?.result !== 8) throw new Error('Ordinary account creation rejected');
  const login = await c.request({ type: 'login', accountId: account.accountId, password: account.password }, 'LoginSuccess');
  if (fresh && login.payload?.characters?.length) throw new Error('Fresh fixture account was not empty');
  const character = fresh ? (await c.request({ type: 'newCharacter', name: account.name, class: account.className, gender: 'Male' }, 'NewCharacterSuccess')).payload?.character : login.payload?.characters?.find(x => x.name === account.name);
  if (!Number.isInteger(character?.index)) throw new Error('Missing ordinary character selection');
  const ack = await c.request({ type: 'startGame', characterIndex: character.index }, 'StartGame');
  if (ack.payload?.result !== 4) throw new Error('Ordinary StartGame rejected');
  c.inGame = true; await c.wait(() => self(c.snapshot)?.name === account.name, 'ordinary character bootstrap'); await c.refresh();
  return { characterIndex: character.index, startAckSequence: ack.sequence, fingerprint: fingerprint(c), playerHp: c.snapshot.playerHp, playerMaxHp: c.snapshot.playerMaxHp, playerMp: c.snapshot.playerMp, playerMaxMp: c.snapshot.playerMaxMp, playerCrystalStats: c.snapshot.playerCrystalStats };
}
async function equipStarter(c) {
  const receipts = [];
  for (const [slot, to] of [['weapon', 0], ['armour', 1], ['torch', 3]]) {
    if ((c.snapshot?.equipmentItems ?? []).some(i => (i.equipSlot ?? i.slot) === slot)) continue;
    const item = c.snapshot?.inventoryItems?.find(i => i.equipSlot === slot);
    if (!item) continue;
    const ack = await c.request({ type: 'equipItem', grid: 'inventory', uniqueId: item.uniqueId, to }, 'EquipItem');
    if (!ack.payload?.success) throw new Error('Ordinary starter equipment rejected');
    await c.refresh(); receipts.push({ item: item.name, uniqueId: item.uniqueId, slot, ackSequence: ack.sequence });
  }
  return receipts;
}
async function movement(c, plan, timeoutMs = 2200) {
  const before = location(c.snapshot), sequence = c.sequence, sentMs = stamp();
  c.send({ type: plan.type ?? 'walk', direction: plan.direction });
  const sentSequence = c.sequence;
  const ack = await c.wait(() => eventAfter(c, sequence, 'UserLocation', p => Number(p.location?.x ?? p.x) === plan.target.x && Number(p.location?.y ?? p.y) === plan.target.y), 'ordinary target move ACK', timeoutMs);
  return { before, sentSequence, sentMs, ackSequence: ack.sequence, ackMs: ack.tsMs, target: plan.target, latencyMs: ack.tsMs - sentMs };
}
const maps = new Map();
async function collision(id) { if (!maps.has(id)) maps.set(id, await loadProtocolCollisionMap(id, { packagedMapRoot: path.join(ROOT, 'maps') })); return maps.get(id); }
function ordinaryStep(c, map, target) { return { type: 'walk', direction: facing(self(c.snapshot), target), target }; }
async function walkNear(c, target, budget, { range = 1, transferTarget = null, knownTransfer = null, transferMetadata = null } = {}) {
  const started = stamp(), steps = [];
  while (stamp() - started < budget) {
    await c.refresh(); const here = location(c.snapshot), me = self(c.snapshot);
    if (!me || hasAuthoritativePlayerDeath(c.snapshot)) throw new Error('Travel interrupted by actual death');
    if (transferTarget && here.map === transferTarget) return { status: 'passed', steps };
    if (!transferTarget && dist(me, target) <= range) return { status: 'passed', steps };
    const map = await collision(here.map), obstacles = liveBlockers(c.snapshot);
    if (knownTransfer) assertOrdinaryTransferCurrent(c.snapshot, transferMetadata, knownTransfer, map);
    const possible = [];
    for (const [dx, dy] of [[0, 0], ...Object.values(DELTAS)]) {
      const end = { x: target.x + dx, y: target.y + dy };
      if (dist(end, target) > range || obstacles.some(b => b.x === end.x && b.y === end.y)) continue;
      const route = findProtocolWalkPath({ map, start: me, target: end, dynamicObstacles: obstacles, staticWalkableOverrides: knownTransfer ? [knownTransfer.source] : [], maxExpanded: 120000 });
      if (route?.length >= 2) possible.push(route);
    }
    possible.sort((a, b) => a.length - b.length);
    if (!possible.length) throw new Error('No ordinary static/occupancy-legal travel path; no QA fallback');
    const plan = ordinaryStep(c, map, possible[0][1]), beforeSeq = c.sequence;
    if (transferTarget && dist(plan.target, target) === 0) {
      c.send({ type: 'walk', direction: plan.direction });
      await c.wait(() => String(c.snapshot?.mapFileName) === transferTarget, 'legal map transfer', 5000);
      await c.refresh(); return { status: 'passed', steps: [...steps, { sentSequence: beforeSeq + 1, transfer: transferTarget }] };
    }
    try { steps.push(await movement(c, plan)); }
    catch (e) { if (transferTarget && location(c.snapshot).map === transferTarget) return { status: 'passed', steps: [...steps, { actualTransferEarlierInSourceBounds: true, targetMap: transferTarget, retainedMoveWait: e.message }] }; throw e; }
    await delay(650);
  }
  throw new Error('Ordinary travel exceeded declared wall-clock budget; preserved as incomplete');
}
async function transfer(c, metadata, targetMap, budget) {
  await c.refresh();
  const map = await collision(location(c.snapshot).map);
  const choice = selectOrdinaryTransfer({ snapshot: c.snapshot, metadata, targetMap, map });
  c.record('diagnostic', { type: 'ordinaryTransferCandidatePaths', sourceMap: location(c.snapshot).map,
    targetMap, attempts: choice.attempts, selectedSource: choice.selection?.source ?? null });
  if (!choice.selection) throw new Error('No source-verified ordinary transfer with a legal static/occupancy path to ' + targetMap);
  return walkNear(c, choice.selection.source, budget, { range: 0, transferTarget: targetMap,
    knownTransfer: choice.selection, transferMetadata: metadata });
}
async function openBuy(c, npc) {
  await walkNear(c, npc, 120000); await c.refresh();
  const live = c.snapshot.entities.find(e => e.kind === 'npc' && e.name === npc.name && e.x === npc.x && e.y === npc.y);
  if (!live) throw new Error('Normal shop NPC absent from live AOI');
  c.send({ type: 'interact', objectId: live.objectId });
  await c.wait(() => Number(c.snapshot?.activeNpcDialog?.npcObjectId) === Number(live.objectId), 'ordinary NPC dialog', 5000);
  const link = c.snapshot.activeNpcDialog.links.find(l => /^@buy(sell)?$/i.test(String(l.target).trim()) && !l.disabled && l.enabled !== false);
  if (!link) throw new Error('Normal NPC dialog has no enabled buy service');
  const goods = await c.request({ type: 'selectNpcDialog', target: link.target }, 'NPCGoods', 8000);
  if (Number(goods.payload?.panelType) !== 0 || !Array.isArray(goods.payload?.list)) throw new Error('Invalid ordinary shop goods panel');
  return { npc: { name: live.name, objectId: live.objectId }, goods: goods.payload.list, goodsSequence: goods.sequence };
}
function countItem(c, name) { return [...(c.snapshot?.inventoryItems ?? []), ...(c.snapshot?.beltItems ?? [])].filter(i => i.name === name).reduce((n, i) => n + Number(i.quantity ?? i.count ?? 1), 0); }
function heldItem(c, name) { return [...(c.snapshot?.inventoryItems ?? []), ...(c.snapshot?.beltItems ?? [])].find(i => i.name === name); }
async function buy(c, shop, name, quantity) {
  const row = shop.goods.find(r => r.name === name), rowId = row?.id ?? row?.uniqueId ?? row?.unique_id;
  if (!row || !/^[0-9]+$/.test(String(rowId)) || !(Number(row.price) > 0)) return { status: 'blocked', reason: 'Required canonical goods absent from ordinary NPC shop', name };
  const oldGold = Number(c.snapshot.gold), oldCount = countItem(c, name), price = Number(row.price);
  if (oldGold < price * quantity) return { status: 'blocked', reason: 'Bounded normal wallet cannot afford goods', name, quantity, price };
  const seq = c.sequence; c.send({ type: 'buyItem', itemIndex: rowId, count: quantity, panelType: 0 });
  // BuyItem implementations need not emit a dedicated receipt; require exact
  // subsequent ordinary snapshot wallet debit and new item quantity instead.
  await c.wait(() => c.events.some(e => e.sequence > seq && (e.packet === 'GainedGold' || e.packet === 'LoseGold' || e.type === 'worldSnapshot' || e.packet === 'NewItem')), 'normal purchase activity', 8000);
  await c.refresh();
  if (Number(c.snapshot.gold) !== oldGold - price * quantity || countItem(c, name) !== oldCount + quantity) throw new Error('Purchase lacked exact debit/quantity authority proof');
  return { status: 'passed', name, quantity, unitPrice: price, walletBefore: oldGold, walletAfter: Number(c.snapshot.gold), quantityBefore: oldCount, quantityAfter: countItem(c, name), uniqueIds: [...(c.snapshot.inventoryItems ?? []), ...(c.snapshot.beltItems ?? [])].filter(i => i.name === name).map(i => i.uniqueId), sentSequence: seq + 1, merchant: shop.npc };
}
function learned(c, spell) { return (c.snapshot?.knownSkills ?? []).some(s => (s.spell ?? s.magicName ?? s.name) === spell); }
async function use(c, item) {
  const before = { hp: c.snapshot.playerHp, mp: c.snapshot.playerMp, count: countItem(c, item.name) };
  const grid = (c.snapshot.beltItems ?? []).some(i => i.uniqueId === item.uniqueId) ? 'belt' : 'inventory';
  const ack = await c.request({ type: 'useItem', uniqueId: item.uniqueId, grid }, 'UseItem', 8000); await c.refresh();
  return { status: ack.payload?.success === true ? 'passed' : 'failed', name: item.name, uniqueId: item.uniqueId, ackSequence: ack.sequence, before, after: { hp: c.snapshot.playerHp, mp: c.snapshot.playerMp, count: countItem(c, item.name) } };
}
async function purchases(c, account, metadata) {
  const receipts = { medicines: [], skillBook: null, medicineUse: null, equipment: await equipStarter(c) };
  const merchant = metadata.npcs.find(n => n.name === 'Merchant_Ruben');
  const shop = await openBuy(c, merchant);
  receipts.medicines.push(await buy(c, shop, '(HP)DrugSmall', 20));
  receipts.medicines.push(await buy(c, shop, '(MP)DrugSmall', 10));
  // Actual UseItem also proves belt/bag packet acceptance; full-pool use is
  // reported honestly and does not claim positive healing yet.
  const hp = heldItem(c, '(HP)DrugSmall'); if (hp) receipts.medicineUse = await use(c, hp);
  const spell = account.className === 'Wizard' ? 'FireBall' : account.className === 'Taoist' ? 'Healing' : null;
  if (spell) {
    try {
      receipts.bookEntry = await transfer(c, metadata, '0132', 180000);
      const books = await openBuy(c, metadata.npcs.find(n => n.name === 'Librarian_Brian'));
      receipts.skillBook = await buy(c, books, spell, 1);
      if (receipts.skillBook.status === 'passed') {
        const book = heldItem(c, spell); if (!book) throw new Error('Purchased skill book missing exact UID');
        receipts.bookUse = await use(c, book); await c.refresh();
        receipts.learnedSpell = learned(c, spell) ? spell : null;
        if (receipts.bookUse.status !== 'passed' || !receipts.learnedSpell) throw new Error('Ordinary book use did not advertise real learned spell');
      }
      receipts.bookExit = await transfer(c, metadata, '0', 120000);
    } catch (error) { receipts.skillBook = { status: 'blocked', reason: error.message, spell }; }
  } else receipts.skillBook = { status: 'not-applicable', reason: 'Warrior ordinary melee escape case has no active spell requirement' };
  receipts.fingerprint = fingerprint(c); return receipts;
}
async function normalMedicine(c) {
  if (hasAuthoritativePlayerDeath(c.snapshot) || stamp() - c.lastMedicineMs < 6200 || observedPlayerHp(c.snapshot) > Number(c.snapshot.playerMaxHp) * 0.65) return null;
  const item = heldItem(c, '(HP)DrugSmall'); if (!item) return null;
  c.lastMedicineMs = stamp();
  const receipt = await use(c, item); c.record('diagnostic', { type: 'ordinaryLowHpMedicine', receipt }); return receipt;
}
async function quiet(c, anchor, anchorSequence, anchorMs, durationMs = 5200) {
  const end = stamp() + durationMs;
  while (stamp() < end) { await delay(500); await c.refresh(); }
  return driftProof(c.events, { anchorSequence, anchorMs, anchor, endMs: stamp(), ownerId: c.snapshot.playerObjectId, durationMs: 5000 });
}
async function findPressure(c, metadata, budgetMs, mode = 'seven') {
  const map = await collision('D021'), source = metadata.maps.find(m => m.map === 'D021');
  const startedMs = stamp(), trials = [], heals = [];
  let nextTravel = 0;
  while (stamp() - startedMs < budgetMs) {
    await c.refresh();
    if (location(c.snapshot).map !== 'D021') return { status: 'blocked', reason: 'Ordinary/prepared D021 entry not complete', trials };
    if (hasAuthoritativePlayerDeath(c.snapshot)) return { status: 'failed', reason: 'Actual death while naturally forming group', trials, death: deathProof(c.events, c.snapshot.playerObjectId, { afterSequence: 0, fromMs: startedMs, endMs: stamp() }) };
    const q = mode === 'eight' ? eightOccupancy(c.snapshot, map) : qualifySeven(c.events, c.snapshot, map, source, stamp());
    const me = self(c.snapshot), adjacent = liveBlockers(c.snapshot).filter(e => e.kind === 'monster' && e.disposition === 'hostile' && dist(e, me) === 1).length;
    trials.push({ tsMs: stamp(), location: location(c.snapshot), visibleCaveBats: c.snapshot.entities.filter(e => e.name === 'CaveBat' && !e.dead).length, adjacent, qualification: q });
    if (q.status === 'qualified') return { status: 'qualified', qualification: q, startedMs, elapsedMs: stamp() - startedMs, trials, heals };
    const medicine = await normalMedicine(c); if (medicine) heals.push(medicine);
    // Give nearby original hostile actors time to approach and complete two
    // full normal attack cycles. No event/spawn/moveTo command is available.
    if (adjacent >= 5) { await delay(400); continue; }
    if (stamp() >= nextTravel) {
      const candidates = [];
      for (let x = Math.max(5, me.x - 8); x <= Math.min(map.width - 6, me.x + 8); x++) for (let y = Math.max(5, me.y - 8); y <= Math.min(map.height - 6, me.y + 8); y++) {
        const point = { x, y }, count = c.snapshot.entities.filter(e => e.name === 'CaveBat' && e.ai === 0 && e.disposition === 'hostile' && !e.dead && dist(e, point) <= 7).length;
        if (count < 7 || dist(point, me) === 0 || !Object.values(DELTAS).every(([dx, dy]) => protocolMapCellIsWalkable(map, { x: x + dx, y: y + dy }))) continue;
        const route = findProtocolWalkPath({ map, start: me, target: point, dynamicObstacles: liveBlockers(c.snapshot), maxExpanded: 1000 });
        if (route?.length >= 2) candidates.push({ route, score: count * 10 - route.length });
      }
      candidates.sort((a, b) => b.score - a.score);
      if (candidates.length) try { await movement(c, ordinaryStep(c, map, candidates[0].route[1])); } catch (error) { c.record('diagnostic', { type: 'naturalLureMoveRejectedOrLate', error: error.message }); }
      nextTravel = stamp() + 1400;
    }
    await delay(400);
  }
  return { status: 'inconclusive', reason: 'Natural original spawn/lure did not meet declared geometry and real-hit preconditions within budget', elapsedMs: stamp() - startedMs, trials, heals };
}
async function attackEscape(c, metadata, formation) {
  const map = await collision('D021'), source = metadata.maps.find(m => m.map === 'D021');
  await c.refresh(); const qualification = qualifySeven(c.events, c.snapshot, map, source, stamp());
  if (qualification.status !== 'qualified') return { status: 'inconclusive', reason: 'Precondition expired before ordinary attack', qualification };
  const target = c.snapshot.entities.find(e => qualification.attackerIds.includes(e.objectId)), startSeq = c.sequence;
  c.send({ type: 'attack', objectId: target.objectId });
  const commit = await c.wait(() => eventAfter(c, startSeq, 'ObjectAttack', p => Number(p.objectId) === Number(c.snapshot.playerObjectId)), 'real owner attack commitment', 5000);
  const refreshedQualification = qualifySeven(c.events, c.snapshot, map, source, stamp());
  if (refreshedQualification.status !== 'qualified') return { status: 'inconclusive', reason: 'Seven attackers/empty exit expired after committed attack; retained barrier', qualification, refreshedQualification, committedAckSequence: commit.sequence };
  const exit = refreshedQualification.exit, before = location(c.snapshot), sentMs = stamp();
  c.send({ type: 'walk', direction: exit.direction }); const sentSequence = c.sequence;
  // Continue gathering after a late/no ACK instead of swallowing the failure.
  let ackError = null;
  try { await c.wait(() => eventAfter(c, sentSequence, 'UserLocation', p => Number(p.location?.x ?? p.x) === exit.target.x && Number(p.location?.y ?? p.y) === exit.target.y), 'one ordinary escape ACK', 3200); } catch (error) { ackError = error.message; }
  await c.refresh();
  const proof = escapeProof(c.events, { qualification: refreshedQualification, sentSequence, sentMs, endMs: stamp(), ownerId: c.snapshot.playerObjectId, before, target: exit.target, boundMs: 1500, committedKind: 'attack', committedAckSequence: commit.sequence });
  proof.ackWaitError = ackError;
  const ack = c.events.find(e => e.sequence === proof.ackSequence), anchor = ack?.state ?? before;
  proof.stop = await quiet(c, anchor, ack?.sequence ?? c.sequence, ack?.tsMs ?? stamp());
  proof.committedEffects = c.events.filter(e => e.sequence > commit.sequence && ['DamageIndicator', 'ObjectStruck', 'ObjectDied'].includes(e.packet) && Number(e.payload?.objectId) === Number(target.objectId));
  if (proof.stop.status === 'failed') proof.status = 'failed';
  return proof;
}
async function heldEscapeStop(c, metadata) {
  const map = await collision('D021'), source = metadata.maps.find(m => m.map === 'D021');
  await c.refresh(); const q = qualifySeven(c.events, c.snapshot, map, source, stamp());
  if (q.status !== 'qualified') return { status: 'inconclusive', reason: 'Seven real attackers unavailable for held-input case', qualification: q };
  const startedLocation = location(c.snapshot), [dx, dy] = DELTAS[q.exit.direction], initialSequence = c.sequence, startedMs = stamp();
  const heldIntervalMs = 90, heldDurationMs = 850;
  while (stamp() - startedMs < heldDurationMs) {
    // Live occupancy/static collision checked before every transmitted intent.
    const at = self(c.snapshot), target = { x: at.x + dx, y: at.y + dy };
    if (hasAuthoritativePlayerDeath(c.snapshot) || !protocolMapCellIsWalkable(map, target, liveBlockers(c.snapshot))) break;
    c.send({ type: 'walk', direction: q.exit.direction }); await delay(heldIntervalMs);
  }
  const stopSequence = c.sequence, stopMs = stamp(), atStop = location(c.snapshot);
  // The pending tail gets at most 1.5 seconds. Then collect five more seconds;
  // do not choose a new anchor after draining an unlimited stale queue.
  const end = stamp() + 6800;
  while (stamp() < end) { await delay(500); await c.refresh(); }
  return { ...holdStopProof(c.events, { stopSequence, stopMs, atStop, endMs: stamp(), ownerId: c.snapshot.playerObjectId }), qualification: q, heldIntervalMs, heldDurationMs, initialSequence, startedLocation, nativeRightButtonLatchAccepted: false };
}
async function castEscape(c, account, metadata) {
  const spell = account.className === 'Wizard' ? 'FireBall' : account.className === 'Taoist' ? 'Healing' : null;
  if (!spell) return { status: 'not-applicable', reason: 'No active Warrior cast in this cohort' };
  if (!learned(c, spell)) return { status: 'blocked', reason: 'Required spell not learned from ordinary purchased book', spell };
  const map = await collision('D021'), source = metadata.maps.find(m => m.map === 'D021');
  await c.refresh(); const q = qualifySeven(c.events, c.snapshot, map, source, stamp());
  if (q.status !== 'qualified' || Number(c.snapshot.playerPoison ?? 0) & CAST_BLOCK_MASK) return { status: 'inconclusive', reason: 'Original hit pressure or genuine cast control precondition missing', qualification: q };
  const me = self(c.snapshot), target = spell === 'Healing' ? me : c.snapshot.entities.find(e => q.attackerIds.includes(e.objectId));
  const eligibility = ordinaryMagicEligibility(c.snapshot, spell, target?.objectId);
  if (eligibility.status !== 'eligible') return { status: 'inconclusive', reason: 'Actual ordinary target or sufficient absolute MP missing before cast', qualification: q, eligibility };
  const beforeState = ownerState(c.snapshot), beforeMp = beforeState.mp, castSentMs = stamp();
  const command = { type: 'magic', spell, objectId: c.snapshot.playerObjectId, direction: facing(me, target) ?? me.direction ?? 'Right', targetId: target.objectId, x: target.x, y: target.y };
  c.send(command); const castSentSequence = c.sequence;
  const ack = await c.wait(() => eventAfter(c, castSentSequence, 'MagicCast', p => p.spell === spell), 'actual requested spell commit', 5000);
  await c.wait(() => eventAfter(c, castSentSequence, 'ObjectMagic', p => Number(p.objectId) === Number(c.snapshot.playerObjectId) && p.spell === spell && p.cast === true), 'paired requested owner cast=true', 5000);
  await c.refresh(); const commitEndMs = stamp();
  const committed = castCommitProof(c.events, { ownerId: c.snapshot.playerObjectId, castSentSequence, castSentMs, spell, beforeMp, beforeState, untilMs: commitEndMs });
  if (hasAuthoritativePlayerDeath(c.snapshot)) return { status: 'failed', reason: 'Actual owner death before post-cast escape', castCommit: committed, qualification: q };
  if (committed.status !== 'qualified') return { status: 'inconclusive', reason: 'Requested cast pairing and absolute MP spend were not both observed before escape', castCommit: committed, qualification: q };
  const afterCommit = qualifySeven(c.events, c.snapshot, map, source, stamp());
  if (afterCommit.status !== 'qualified') return { status: 'inconclusive', reason: 'Real seven-hit/empty exit precondition expired after cast commitment', qualification: q, afterCommit, committedAckSequence: ack.sequence };
  const before = location(c.snapshot), sentMs = stamp(); c.send({ type: 'walk', direction: afterCommit.exit.direction }); const sentSequence = c.sequence;
  let ackError = null;
  try { await c.wait(() => eventAfter(c, sentSequence, 'UserLocation', p => Number(p.location?.x ?? p.x) === afterCommit.exit.target.x && Number(p.location?.y ?? p.y) === afterCommit.exit.target.y), 'post-cast escape ACK', 3200); } catch (error) { ackError = error.message; }
  await c.refresh();
  const proof = escapeProof(c.events, { qualification: afterCommit, sentSequence, sentMs, endMs: stamp(), ownerId: c.snapshot.playerObjectId, before, target: afterCommit.exit.target, boundMs: 1500, committedKind: 'cast', committedAckSequence: ack.sequence });
  proof.castCommit = castCommitProof(c.events, { ownerId: c.snapshot.playerObjectId, castSentSequence, castSentMs, spell, beforeMp, beforeState, escapeSentSequence: sentSequence, commitEndMs, untilMs: stamp() });
  proof.ackWaitError = ackError;
  if (proof.castCommit.status !== 'qualified' && proof.status === 'passed') proof.status = 'inconclusive';
  const moveAck = c.events.find(e => e.sequence === proof.ackSequence);
  proof.stop = await quiet(c, moveAck?.state ?? before, moveAck?.sequence ?? c.sequence, moveAck?.tsMs ?? stamp());
  proof.castCommit = castCommitProof(c.events, { ownerId: c.snapshot.playerObjectId, castSentSequence, castSentMs, spell, beforeMp, beforeState, escapeSentSequence: sentSequence, commitEndMs, untilMs: stamp() });
  if (proof.stop.status === 'failed') proof.status = 'failed';
  // Recast is a separate bounded check; missing exposure never becomes a pass.
  proof.prematureRecast = { status: 'inconclusive', reason: 'This case preserves escape/effect ordering; a separately qualified early recast window is required', sourceGlobalSpellMs: 1800 };
  return proof;
}
async function earlyRecast(c, account, metadata) {
  const spell = account.className === 'Wizard' ? 'FireBall' : account.className === 'Taoist' ? 'Healing' : null;
  if (!spell) return { status: 'not-applicable', reason: 'No active Warrior cast in this cohort' };
  if (!learned(c, spell)) return { status: 'blocked', reason: 'No normally learned eligible spell', spell };
  await c.refresh(); const q = qualifySeven(c.events, c.snapshot, await collision('D021'), metadata.maps.find(m => m.map === 'D021'), stamp());
  if (q.status !== 'qualified' || (Number(c.snapshot.playerPoison ?? 0) & CAST_BLOCK_MASK)) return { status: 'inconclusive', reason: 'Natural pressure/cast prerequisites expired', qualification: q };
  const me = self(c.snapshot), target = spell === 'Healing' ? me : c.snapshot.entities.find(e => q.attackerIds.includes(e.objectId));
  const eligibility = ordinaryMagicEligibility(c.snapshot, spell, target?.objectId);
  if (eligibility.status !== 'eligible') return { status: 'inconclusive', reason: 'Actual requested target or sufficient absolute MP missing before first cast', qualification: q, eligibility };
  const command = { type: 'magic', spell, objectId: c.snapshot.playerObjectId, targetId: target.objectId, x: target.x, y: target.y, direction: facing(me, target) ?? me.direction ?? 'Right' };
  const beforeFirstState = ownerState(c.snapshot), beforeFirstMp = beforeFirstState.mp, firstSentMs = stamp(); c.send(command); const firstSentSequence = c.sequence;
  await c.wait(() => eventAfter(c, firstSentSequence, 'MagicCast', p => p.spell === spell), 'first actual requested spell commit', 5000);
  await c.wait(() => eventAfter(c, firstSentSequence, 'ObjectMagic', p => Number(p.objectId) === Number(c.snapshot.playerObjectId) && p.spell === spell && p.cast === true), 'first paired requested owner cast=true', 5000);
  await c.refresh();
  const firstCast = castCommitProof(c.events, { ownerId: c.snapshot.playerObjectId, castSentSequence: firstSentSequence, castSentMs: firstSentMs, spell, beforeMp: beforeFirstMp, beforeState: beforeFirstState, untilMs: stamp() });
  if (hasAuthoritativePlayerDeath(c.snapshot)) return { status: 'failed', reason: 'Actual owner death before premature second input', spell, firstCast, firstSentMs };
  const secondEligibility = ordinaryMagicEligibility(c.snapshot, spell, target.objectId);
  if (stamp() - firstSentMs > 1200 || firstCast.status !== 'qualified' || secondEligibility.status !== 'eligible') return { status: 'inconclusive', reason: 'Requested first pairing/absolute MP spend/eligible second target could not leave a valid premature window', spell, firstSentMs, firstCast, secondEligibility };
  const secondTarget = c.snapshot.entities.find(e => Number(e.objectId) === Number(target.objectId)), secondMe = self(c.snapshot);
  const secondCommand = { ...command, x: secondTarget.x, y: secondTarget.y, direction: facing(secondMe, secondTarget) ?? secondMe.direction ?? 'Right' };
  const beforeSecondState = ownerState(c.snapshot), beforeSecondMp = beforeSecondState.mp, secondSentMs = stamp(); c.send(secondCommand); const secondSentSequence = c.sequence;
  const end = stamp() + 950; while (stamp() < end) { await delay(250); await c.refresh(); }
  return { ...prematureCastProof(c.events, { ownerId: c.snapshot.playerObjectId, spell, firstSentSequence, firstSentMs, beforeFirstMp, beforeFirstState, secondSentMs, secondSentSequence, beforeSecondMp, beforeSecondState, endMs: stamp() }), qualification: q, secondEligibility, knownSkillClockSnapshot: (c.snapshot.knownSkills ?? []).find(s => s.spell === spell) };
}
async function fullSurround(c, metadata, budget) {
  const formation = await findPressure(c, metadata, budget, 'eight');
  if (formation.status !== 'qualified') return { status: formation.status, formation, reason: 'Eight-way natural occupancy case separate from seven-attacker exit case' };
  const map = await collision('D021'), q = formation.qualification, cell = q.cells.find(e => ['Up', 'Right', 'Down', 'Left'].includes(e.direction)), before = location(c.snapshot);
  const sentMs = stamp(); c.send({ type: 'walk', direction: cell.direction }); const sentSequence = c.sequence;
  const end = stamp() + 2000; while (stamp() < end) { await delay(500); await c.refresh(); }
  const rejection = rejectionProof(c.events, { sentSequence, before, blockerId: cell.blocker.objectId, ownerId: c.snapshot.playerObjectId, requiredWindowMs: 1800, sentMs, endMs: stamp() });
  if (rejection.status !== 'passed') return { status: rejection.status, formation, rejection };
  let death = null;
  for (let i = 0; i < 20 && !hasAuthoritativePlayerDeath(c.snapshot); i++) {
    const seq = c.sequence; c.send({ type: 'attack', objectId: cell.blocker.objectId }); await delay(800); await c.refresh();
    death = eventAfter(c, seq, 'ObjectDied', p => Number(p.objectId) === Number(cell.blocker.objectId)); if (death) break;
    await normalMedicine(c);
  }
  if (!death) return { status: hasAuthoritativePlayerDeath(c.snapshot) ? 'failed' : 'inconclusive', formation, rejection, reason: 'Ordinary kill did not produce selected blocking actor death' };
  const seven = qualifySeven(c.events, c.snapshot, map, metadata.maps.find(m => m.map === 'D021'), stamp());
  if (seven.status !== 'qualified' || seven.attackerIds.length < 7) return { status: 'inconclusive', formation, rejection, deathSequence: death.sequence, reason: 'Other seven real attacking IDs no longer qualified after opening exit', remainingPressure: seven };
  try {
    const walk = await movement(c, { type: 'walk', direction: cell.direction, target: cell.tile }, 3200), stop = await quiet(c, location(c.snapshot), walk.ackSequence, walk.ackMs);
    return { status: walk.latencyMs <= 1500 && stop.status === 'passed' ? 'passed' : 'failed', formation, rejection, deathSequence: death.sequence, remainingPressure: seven, walk, stop };
  } catch (error) { return { status: 'failed', formation, rejection, deathSequence: death.sequence, reason: error.message }; }
}
async function actualDeath(c, account, metadata, budgetMs) {
  const first = c.sequence, started = stamp(), map = await collision('D021');
  // No attacks, casts or medicines in the death clone. Let the unchanged real
  // hostile AI kill the canonical owner; never set HP directly.
  while (stamp() - started < budgetMs && !eventAfter(c, first, 'Death')) {
    await c.refresh(); const me = self(c.snapshot), hostile = c.snapshot.entities.filter(e => e.name === 'CaveBat' && e.ai === 0 && e.disposition === 'hostile' && !e.dead);
    if (!hostile.some(e => dist(e, me) <= 1)) {
      const destinations = hostile.flatMap(h => Object.values(DELTAS).map(([dx, dy]) => ({ x: h.x + dx, y: h.y + dy }))).map(target => findProtocolWalkPath({ map, start: me, target, dynamicObstacles: liveBlockers(c.snapshot), maxExpanded: 3000 })).filter(r => r?.length >= 2).sort((a, b) => a.length - b.length);
      if (destinations.length) try { await movement(c, ordinaryStep(c, map, destinations[0][1])); } catch (e) { c.record('diagnostic', { type: 'ordinaryDeathLureMoveRejected', reason: e.message }); }
    }
    await delay(500);
  }
  await c.refresh(); const proof = deathProof(c.events, c.snapshot.playerObjectId, { afterSequence: first, fromMs: started, endMs: stamp() });
  if (proof.status !== 'qualified') return { status: 'inconclusive', reason: 'Actual hostile damage/death did not form within declared budget', death: proof };
  const before = location(c.snapshot), me = self(c.snapshot), target = c.snapshot.entities.find(e => e.kind === 'monster' && !e.dead && dist(e, me) === 1), ownerId = c.snapshot.playerObjectId;
  c.send({ type: 'walk', direction: 'Right' }); c.send({ type: 'run', direction: 'Right' });
  if (target) c.send({ type: 'attack', objectId: target.objectId });
  const spell = account.className === 'Wizard' ? 'FireBall' : account.className === 'Taoist' ? 'Healing' : null;
  const magicTarget = spell === 'Healing' ? me : target, magicEligibility = spell ? ordinaryMagicEligibility(c.snapshot, spell, magicTarget?.objectId) : null;
  if (magicEligibility?.status === 'eligible') c.send({ type: 'magic', spell, objectId: ownerId, targetId: magicTarget.objectId, x: magicTarget.x, y: magicTarget.y, direction: facing(me, magicTarget) ?? me.direction ?? 'Right' });
  const commandsEndSequence = c.sequence; const end = stamp() + 5200;
  while (stamp() < end) { await delay(500); await c.refresh(); }
  const blocked = deadActionProof(c.events, { deathSequence: proof.deathSequence, commandsEndSequence, fromMs: c.events.find(e => e.sequence === proof.deathSequence).tsMs, endMs: stamp(), before, ownerId, spell });
  const result = { status: blocked.status, death: proof, deadActions: blocked, attackBaselineEligible: !!target, attackTargetId: target?.objectId ?? null, magicApplicable: magicEligibility?.status === 'eligible', magicEligibility, castBarrierVerified: blocked.castBarrierVerified, preparedDeathClone: true };
  result.deadFingerprint = fingerprint(c); result.deadLogoutAck = (await c.logout()).sequence;
  await c.close();
  // Caller performs the normal relogin/revive/persist stage on a new socket.
  return result;
}
async function runClass(input, account, metadata) {
  const row = { className: account.className, phase: input.phase, status: 'running', cases: [], ...ACCEPTANCE_FALSE };
  const c = new DenseClient(input.origin.replace(/^http:/, 'ws:') + '/ws', path.join(input.output, account.className + '.jsonl'), input.secrets);
  const clients = [c];
  try {
    await c.connect(); row.bootstrap = await start(c, account, input.phase === 'bootstrap');
    if (input.phase === 'bootstrap') row.starterEquipment = await equipStarter(c);
    else if (input.phase === 'purchases') row.purchases = await purchases(c, account, metadata);
    else if (input.phase === 'travel') {
      const at = location(c.snapshot).map;
      if (at === '0132') row.bookExit = await transfer(c, metadata, '0', 120000);
      if (location(c.snapshot).map === '0') row.travel1 = await transfer(c, metadata, '1', input.travelBudgetMs);
      if (location(c.snapshot).map === '1') row.travel2 = await transfer(c, metadata, 'D021', input.travelBudgetMs);
      if (location(c.snapshot).map !== 'D021') throw new Error('Legal route did not enter original D021');
    } else if (input.phase === 'dense') {
      for (const kind of ['attackEscape', 'castEscape', 'heldStop', 'prematureRecast']) {
        if (account.className === 'Warrior' && ['castEscape', 'prematureRecast'].includes(kind)) { row.cases.push({ kind, status: 'not-applicable', reason: 'Warrior ordinary attack is applicable; no active cast spell required' }); continue; }
        for (let trial = 0; trial < input.trials; trial++) {
          let caseRow;
          try {
            const formation = await findPressure(c, metadata, input.formationBudgetMs);
            caseRow = { kind, trial: trial + 1, formation, status: formation.status };
            if (formation.status === 'qualified') {
              caseRow.result = kind === 'attackEscape' ? await attackEscape(c, metadata, formation) : kind === 'castEscape' ? await castEscape(c, account, metadata) : kind === 'heldStop' ? await heldEscapeStop(c, metadata) : await earlyRecast(c, account, metadata);
              caseRow.status = caseRow.result.status;
            }
          } catch (e) { caseRow = { kind, trial: trial + 1, status: 'failed', error: e.message, criticalReceiptSequences: c.events.filter(e => ['DamageIndicator', 'ObjectAttack', 'ObjectStruck', 'UserLocation', 'Death', 'ObjectMagic', 'MagicCast'].includes(e.packet)).slice(-80).map(e => e.sequence) }; }
          row.cases.push(caseRow);
          if (hasAuthoritativePlayerDeath(c.snapshot)) break; // Failure retained; no revive to fake continuing trial.
        }
        if (hasAuthoritativePlayerDeath(c.snapshot)) break;
      }
      if (input.includeSurround && !hasAuthoritativePlayerDeath(c.snapshot)) row.cases.push({ kind: 'fullSurround', result: await fullSurround(c, metadata, input.formationBudgetMs) });
    } else if (input.phase === 'death') {
      row.death = await actualDeath(c, account, metadata, input.deathBudgetMs);
      if (row.death.death?.status === 'qualified') {
        const relog = new DenseClient(input.origin.replace(/^http:/, 'ws:') + '/ws', path.join(input.output, account.className + '-dead-relogin.jsonl'), input.secrets); clients.push(relog); await relog.connect();
        row.deadRelogin = await start(relog, account); row.deadPersistenceMatched = JSON.stringify(row.deadRelogin.fingerprint) === JSON.stringify(row.death.deadFingerprint) && hasAuthoritativePlayerDeath(relog.snapshot);
        if (!row.deadPersistenceMatched) throw new Error('Normal relogin did not preserve actual terminal death');
        const seq = relog.sequence; relog.send({ type: 'townRevive' });
        const revived = await relog.wait(() => eventAfter(relog, seq, 'Revived'), 'ordinary town revival', 10000); await relog.refresh();
        const bind = account.expectedBindPoint;
        row.revival = { ackSequence: revived.sequence, location: location(relog.snapshot), hp: relog.snapshot.playerHp, actualAlive: !hasAuthoritativePlayerDeath(relog.snapshot) && Number(relog.snapshot.playerHp) > 0, expectedBindPoint: bind ?? null, town: !!bind && location(relog.snapshot).map === bind.map_file_name && location(relog.snapshot).x === bind.position.x && location(relog.snapshot).y === bind.position.y };
        row.revival.stop = await quiet(relog, location(relog.snapshot), relog.sequence, stamp());
        if (!row.revival.actualAlive || !row.revival.town || row.revival.stop.status !== 'passed') row.death.status = 'failed';
        row.revivedFingerprint = fingerprint(relog); row.revivedLogoutAck = (await relog.logout()).sequence; await relog.close();
        const after = new DenseClient(input.origin.replace(/^http:/, 'ws:') + '/ws', path.join(input.output, account.className + '-revived-relogin.jsonl'), input.secrets); clients.push(after); await after.connect(); row.revivedRelogin = await start(after, account); row.revivedPersistenceMatched = JSON.stringify(row.revivedRelogin.fingerprint) === JSON.stringify(row.revivedFingerprint); row.logoutAckSequence = (await after.logout()).sequence;
        if (!row.revivedPersistenceMatched) row.death.status = 'failed';
      }
    } else throw new Error('Unsupported driver phase');
    if (c.inGame) {
      await c.refresh(); row.finalFingerprint = fingerprint(c); row.logoutAckSequence = (await c.logout()).sequence; await c.close();
      const relogin = new DenseClient(input.origin.replace(/^http:/, 'ws:') + '/ws', path.join(input.output, account.className + '-' + input.phase + '-relogin.jsonl'), input.secrets); clients.push(relogin); await relogin.connect();
      row.relogin = await start(relogin, account); row.persistenceMatched = JSON.stringify(row.relogin.fingerprint) === JSON.stringify(row.finalFingerprint); row.finalLogoutAckSequence = (await relogin.logout()).sequence;
      if (!row.persistenceMatched) throw new Error('Normal logout/relogin changed transform, wallet, spell set or exact item UID/durability');
    }
    row.status = row.cases.some(r => r.status === 'failed' || r.result?.status === 'failed') || row.death?.status === 'failed' ? 'failed' : row.cases.some(r => ['inconclusive', 'blocked'].includes(r.status) || ['inconclusive', 'blocked'].includes(r.result?.status)) || row.death?.status === 'inconclusive' ? 'inconclusive' : 'completed';
  } catch (e) { row.status = 'failed'; row.error = redact(e.message, input.secrets); }
  finally { row.cleanupErrors = []; for (const client of clients) try { await client.close(); } catch (e) { row.cleanupErrors.push(redact(e.message, input.secrets)); } }
  if (row.cleanupErrors.length) row.status = 'failed';
  row.normalLogoutConfirmed = clients.every(x => !x.inGame);
  return row;
}
async function planStaging(metadata) {
  const map = await collision('D021'), meta = metadata.maps.find(m => m.map === 'D021');
  const candidates = [];
  for (let x = 35; x < Math.min(80, map.width - 4); x++) for (let y = 30; y < Math.min(80, map.height - 4); y++) {
    const snapshot = { mapFileName: 'D021', playerObjectId: 1000, playerHp: 1, entities: [{ objectId: 1000, x, y, kind: 'player', dead: false }] };
    const q = qualifySeven([], snapshot, map, meta, 0);
    if (q.exit) candidates.push({ x, y, distance: dist({ x, y }, { x: 58, y: 57 }), corridor: q.exit.corridor });
  }
  candidates.sort((a, b) => a.distance - b.distance || a.x - b.x || a.y - b.y);
  if (!candidates.length) throw new Error('No source-static walkable staging near ordinary spawn region');
  return { map: 'D021', title: 'WoomaTempleEntrance', x: candidates[0].x, y: candidates[0].y, corridor: candidates[0].corridor, staticWalkable: true, sourceValidated: true, mapGzipSha256: sha(await fs.readFile(path.join(ROOT, 'maps/d021.map.gz'))), liveOccupancyVerified: false, preparedTravel: true };
}
async function main() {
  const { values } = parseArgs({ options: { 'plan-staging': { type: 'boolean' } } });
  const metadata = JSON.parse(await fs.readFile(path.join(ROOT, 'WORLD-INPUTS.json'), 'utf8'));
  if (values['plan-staging']) { console.log(JSON.stringify(await planStaging(metadata))); return; }
  let data = ''; for await (const chunk of process.stdin) { data += chunk; if (data.length > 24000) throw new Error('Bounded stdin input exceeded'); }
  const input = JSON.parse(data); data = '';
  if (input.binarySha256 !== ELF_SHA || !/^http:\/\/127\.0\.0\.1:[1-9][0-9]{0,4}$/.test(input.origin ?? '') || !['bootstrap', 'purchases', 'travel', 'dense', 'death'].includes(input.phase)) throw new Error('Only staged exact ELF loopback fixture is accepted');
  const output = path.resolve(input.output);
  if (output === ROOT || !output.startsWith(ROOT + path.sep)) throw new Error('Receipts must be a fresh child of this external fixture directory');
  await fs.mkdir(output, { recursive: false, mode: 0o700 }); input.output = output;
  if (input.accounts?.length !== 3 || new Set(input.accounts.map(a => a.className)).size !== 3 || input.accounts.some(a => !/^dn[a-f0-9]+[0-2]$/.test(a.accountId) || !/^[a-f0-9]{32}$/.test(a.password))) throw new Error('Fresh in-memory three-class fixture credentials required');
  input.secrets = input.accounts.map(a => a.password);
  const report = { schema: 'mir2.prepared-dense-network-phase.v2', phase: input.phase, preparedState: input.phase !== 'bootstrap', sourceRevision: '6032ef8b3e27dd97bad0b20c8676ef9f185db64b', binarySha256: ELF_SHA, unsafeQaEnabled: false, originalMonsterStatsClock: true, rawFailuresPreserved: true, classes: [], ...ACCEPTANCE_FALSE };
  for (const account of input.accounts) { const row = await runClass(input, account, metadata); report.classes.push(row); console.log(JSON.stringify({ className: row.className, phase: input.phase, status: row.status, cleanupErrors: row.cleanupErrors.length })); }
  report.normalLogoutConfirmed = report.classes.every(r => r.normalLogoutConfirmed && !r.cleanupErrors.length);
  report.driverCompleted = report.classes.length === 3 && report.normalLogoutConfirmed;
  report.mechanicsAllCasesPassed = input.phase === 'dense' && report.classes.every(r => r.status === 'completed' && r.cases.filter(c => c.status !== 'not-applicable').length >= input.trials * (r.className === 'Warrior' ? 2 : 4) && r.cases.every(c => c.status === 'not-applicable' || c.status === 'passed' || c.result?.status === 'passed'));
  await fs.writeFile(path.join(output, 'report.json'), JSON.stringify(redact(report, input.secrets), null, 2) + '\n');
  // Preserve failed classes while continuing others. Exit status is structural
  // execution only; the public verdict must inspect each case, never exit 0.
  if (!report.driverCompleted) process.exitCode = 2;
}
if (process.argv[1] && path.resolve(process.argv[1]) === path.join(ROOT, 'dense_network_driver.mjs')) main().catch(error => { console.error(JSON.stringify({ fatalFixtureError: error.message, ...ACCEPTANCE_FALSE })); process.exitCode = 2; });
