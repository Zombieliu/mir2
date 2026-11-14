// Ordinary loopback WS adapter. File + in-process cache only, not production capacity.
import fs from 'node:fs/promises';
import path from 'node:path';
import { createRequire } from 'node:module';
import { pathToFileURL, fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';
import { MAP_SHA, LOGIN_WINDOW_MS, hash, canonical, requireThat as check, owner, validatePlan,
  PendingGate, RetainedReceiveCursor, equipmentFingerprint, savedState, scoreWindows, freezeWindowFailures, localLoginDelay, snapshotProjection } from './contract.mjs';

const sleep = ms => new Promise(r => setTimeout(r, Math.max(1, ms)));
const at = () => new Date().toISOString();
const COMMANDS = new Set(['clientVersion', 'login', 'newAccount', 'newCharacter', 'startGame', 'walk', 'chat', 'equipItem', 'removeItem', 'keepAlive', 'logOut']);
const CATALOG = new Set(['NewItemInfo', 'NewQuestInfo', 'NewRecipeInfo', 'GameShopInfo']);
const SLOT_NAMES = ['weapon', 'armour', 'helmet', 'torch', 'necklace', 'braceletLeft', 'braceletRight', 'ringLeft', 'ringRight', 'amulet', 'belt', 'boots', 'stone', 'mount'];
const CANDIDATE_BASE_REVISION = '31e9b6a74661c31d6515dd7322b466285f975d71';
const ADAPTER_SOURCE_PROOF = Symbol('verified-current-adapter-sources');
// Keep cursor/client references outside pending evidence: cleanGate must never
// serialize matcher caches, circular client objects, or receiver histories.
const lifecycleReceipts = new WeakMap(), actionReceipts = new WeakMap();
function lifecycleReceipt(client, pending) {
  let cursor = lifecycleReceipts.get(pending);
  if (!cursor) { cursor = new RetainedReceiveCursor(pending.cursor, pending.matcher); lifecycleReceipts.set(pending, cursor); }
  return cursor.matches(client.events)[0];
}

export async function runScheduledActions(actions, { epoch, now, wait, advance, offer, firstOffer }) {
  let offered = false;
  for (const action of actions) {
    while (now() < epoch + action.dueMs) { advance(); await wait(Math.min(10, epoch + action.dueMs - now())); }
    if (!offered) { firstOffer?.(); offered = true; }
    advance(); offer(action, now());
    // Even an already-overdue backlog must release the JS thread to socket,
    // timer and file callbacks. Time spent yielding stays on the original clock.
    await new Promise(resolve => setImmediate(resolve));
  }
}
export function endpoint(value) {
  const u = new URL(value);
  check(u.protocol === 'ws:' && ['127.0.0.1', '[::1]'].includes(u.hostname) && u.port && ['/ws', '/playtest/ws'].includes(u.pathname) &&
    !u.username && !u.password && !u.search && !u.hash, 'Only explicit literal-loopback ordinary WS endpoint allowed'); return u.toString();
}
function redact(value) {
  if (Array.isArray(value)) return value.map(redact);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) =>
    [k, /password|secret|token|authorization|cookie|credential|accountId/i.test(k) ? '[redacted]' : redact(v)]));
  return value;
}
const receiveReceipt = (client, e) => ({ observerOrdinal: client.ordinal, connection: client.connection, receiveSeq: e.receiveSeq,
  receiveAtUtc: e.atUtc, atMs: e.atMs, packet: e.packet ?? e.type, objectId: e.payload?.objectId ?? null,
  location: e.payload?.location ?? (e.payload?.x !== undefined ? { x: e.payload.x, y: e.payload.y } : null),
  textSha256: e.packet === 'ObjectChat' ? hash(String(e.payload?.text ?? e.payload?.message ?? '')) : null });

export class ModernClient {
  constructor({ url, actor, connection, WebSocket, emit = () => {}, now = () => performance.now() }) {
    Object.assign(this, { url: endpoint(url), actor, ordinal: actor.ordinal, connection, WebSocket, emit, now });
    this.sequence = 0; this.events = []; this.snapshotHistory = []; this.latestSnapshotEvent = null;
    this.snapshot = null; this.sharedPlayers = new Map(); this.failure = null;
    this.inGame = false; this.closed = false; this.pending = null; this.sentBytes = 0; this.receivedBytes = 0;
    this.gate = new PendingGate(row => { this.emit({ ...cleanGate(row), actorOrdinal: this.ordinal, connection: this.connection, atUtc: at() }); });
  }
  observe(message, wireBytes = 0) {
    this.receivedBytes += wireBytes;
    const e = { ...message, receiveSeq: ++this.sequence, atMs: this.now(), atUtc: at() };
    this.countSteadyBytes('received', wireBytes, e.atMs);
    const p = e.payload ?? {};
    if (e.type === 'worldSnapshot') {
      const full = structuredClone(p);
      this.latestSnapshotEvent = { ...e, payload: full };
      // Rosters are shared by the current full source and projected live view.
      // Only owner transform is patched between snapshots, on a separate entity copy.
      this.snapshot = { ...full, entities: full.entities?.map(entity => ({ ...entity })) };
      for (const entity of p.entities ?? []) if (entity.name && entity.kind === 'player' && Number(entity.objectId) !== Number(p.playerObjectId)) this.sharedPlayers.set(entity.name, Number(entity.objectId));
    } else if (e.packet === 'UserLocation' && this.snapshot) {
      const self = owner(this.snapshot), pos = p.location ?? p;
      if (self && Number.isFinite(pos.x) && Number.isFinite(pos.y)) Object.assign(self, { x: pos.x, y: pos.y, ...(p.direction === undefined ? {} : { direction: p.direction }) });
    } else if (e.packet === 'ObjectPlayer') {
      if (p.name && Number.isSafeInteger(p.objectId)) this.sharedPlayers.set(p.name, p.objectId);
    } else if (e.packet === 'ObjectRemove') {
      for (const [name, id] of this.sharedPlayers) if (id === Number(p.objectId)) this.sharedPlayers.delete(name);
    }
    if (e.packet === 'UserInformation') this.inGame = true;
    if (e.packet === 'LogOutSuccess') this.inGame = false;
    if (e.type === 'error') this.failure = new Error('Gateway rejected ordinary command');
    const projection = e.type === 'worldSnapshot' ? snapshotProjection(p) : null;
    if (projection) {
      this.snapshotHistory.push({ ...e, payload: projection });
      if (this.snapshotHistory.length > 16) this.snapshotHistory.shift();
    } else if (!CATALOG.has(e.packet)) { this.events.push(e); if (this.events.length > 6000) this.events.splice(0, 1000); }
    const evidence = projection ? { type: e.type, ...projection, map: p.mapFileName }
      : CATALOG.has(e.packet) ? { type: e.type, packet: e.packet, payloadSha256: hash(p) } : message;
    this.emit({ event: 'received', actorOrdinal: this.ordinal, connection: this.connection, receiveSeq: e.receiveSeq, atMs: e.atMs, atUtc: e.atUtc, wireBytes, ...redact(evidence) });
    const history = projection ? this.snapshotHistory.at(-1) : e;
    this.onObservation?.(this, history); return history;
  }
  countSteadyBytes(direction, bytes, atMs) {
    const w = this.trafficWindow;
    if (w && atMs >= w.startAtMs && atMs < w.endAtMs) w[direction] += bytes;
  }
  counterSample() { return { actorOrdinal: this.ordinal, connection: this.connection, sentBytes: this.sentBytes,
    receivedBytes: this.receivedBytes, actualCounterAtMs: this.now() }; }
  ownedSnapshotEvidence(phase) {
    const source = this.latestSnapshotEvent;
    check(source && owner(source.payload)?.name === this.actor.name && source.payload.playerHp > 0, 'Fresh owned source snapshot unavailable');
    const state = structuredClone(carriedState(source.payload)), fingerprint = equipmentFingerprint(source.payload);
    check(fingerprint === equipmentFingerprint(this.snapshot), 'Projected carried state is not bound to its full source snapshot');
    const row = { event: 'ownedSnapshotEvidence', phase, actorOrdinal: this.ordinal, connection: this.connection,
      atMs: this.now(), atUtc: at(), sourceReceipt: receiveReceipt(this, source), state, fingerprint,
      owner: snapshotProjection(source.payload).owner, mapFileName: source.payload.mapFileName };
    this.emit(row); return row;
  }
  send(command) {
    check(COMMANDS.has(command?.type), 'Command outside ordinary comparable whitelist');
    check(this.ws?.readyState === 1 && !this.closed, 'Original socket is not open');
    const encoded = JSON.stringify(command), sentAtMs = this.now();
    const bytes = Buffer.byteLength(encoded);
    this.emit({ event: 'sent', actorOrdinal: this.ordinal, connection: this.connection, atMs: sentAtMs, atUtc: at(), wireBytes: bytes, command: redact(command) });
    this.sentBytes += bytes; this.countSteadyBytes('sent', bytes, sentAtMs); this.ws.send(encoded); return sentAtMs;
  }
  async wait(predicate, deadlineAtMs, label, { ignoreFailure = false } = {}) {
    for (;;) {
      const found = predicate();
      if (found) { check(!Number.isFinite(found.atMs) || found.atMs <= deadlineAtMs, `${label} arrived after absolute deadline`); return found; }
      if (!ignoreFailure && this.failure) throw this.failure;
      check(!this.closed, `${label}: transport closed`);
      check(this.now() < deadlineAtMs, `${label}: absolute deadline exceeded`);
      await sleep(Math.min(10, deadlineAtMs - this.now()));
    }
  }
  async connect(outerDeadline = Infinity) {
    check(this.now() < outerDeadline, 'Outer fresh-state deadline expired before connect');
    const origin = this.url.replace(/^ws:/, 'http:').replace(/\/(playtest\/)?ws$/, '');
    this.ws = new this.WebSocket(this.url, { origin, perMessageDeflate: false, handshakeTimeout: Math.min(10000, Math.ceil(outerDeadline - this.now())) });
    this.ws.on('message', (data, isBinary) => {
      try { check(!isBinary, 'Plain-Web profile received unsolicited binary frame'); this.observe(JSON.parse(String(data)), typeof data === 'string' ? Buffer.byteLength(data) : data.length); }
      catch { this.failure = new Error('Invalid ordinary gateway frame'); }
    });
    this.ws.on('error', () => { this.failure = new Error('WebSocket transport error'); });
    this.ws.on('close', (code) => { this.closed = true; this.emit({ event: 'transportClosed', actorOrdinal: this.ordinal, connection: this.connection,
      code, validCloseFrameReceived: Boolean(this.ws._closeFrameReceived), atMs: this.now(), atUtc: at() }); });
    await this.wait(() => this.ws.readyState === 1, Math.min(this.now() + 10000, outerDeadline), 'connect');
    const version = await this.request({ type: 'clientVersion' }, e => e.packet === 'ClientVersion', Math.min(this.now() + 15000, outerDeadline), 'clientVersion');
    check(version.payload?.result === 1, 'Ordinary version handshake rejected');
    this.timer = setInterval(() => { if (this.ws.readyState === 1) { try { this.send({ type: 'keepAlive', time: Date.now() }); } catch {} } }, 2000);
  }
  async request(command, matcher, deadlineAtMs, label, options = {}) {
    check(!this.pending && !this.gate.pending, 'Previous unique action has not closed');
    check(this.now() < deadlineAtMs, 'Absolute request deadline expired before send');
    const cursor = this.sequence;
    const pending = { label, cursor, deadlineAtMs, matcher }; this.pending = pending;
    try {
      this.send(command);
      return await this.wait(() => lifecycleReceipt(this, pending), deadlineAtMs, label, options);
    } finally {
      // An expired request fences future game sends until its exact late ACK.
      const ack = lifecycleReceipt(this, pending);
      if (ack) this.pending = null;
      else pending.expired = true;
    }
  }
  async expectAfter(cursor, matcher, deadlineAtMs, label) {
    check(!this.pending && !this.gate.pending, 'Previous unique action has not closed');
    const pending = { label, cursor, deadlineAtMs, matcher }; this.pending = pending;
    try { return await this.wait(() => lifecycleReceipt(this, pending), deadlineAtMs, label); }
    finally { if (lifecycleReceipt(this, pending)) this.pending = null; else pending.expired = true; }
  }
  reconcileLate() {
    if (this.pending?.expired) {
      const p = this.pending, e = lifecycleReceipt(this, p);
      if (e) { this.emit({ event: 'lateLifecycleClosure', label: p.label, ...receiveReceipt(this, e), passed: false }); this.pending = null; }
    }
  }
  async login({ deadlineAtMs = Infinity } = {}) {
    await this.connect(deadlineAtMs);
    const reply = await this.request({ type: 'login', accountId: this.actor.accountId, password: this.actor.password }, e => e.packet === 'LoginSuccess', Math.min(this.now() + 15000, deadlineAtMs), 'ordinaryLogin');
    const character = reply.payload?.characters?.find(c => c.name === this.actor.name && c.index === this.actor.characterIndex);
    check(character && character.level === 40, 'Exact preseeded L40 character missing; no replacement or demo');
    const cursor = this.sequence; this.snapshot = null;
    const start = await this.request({ type: 'startGame', characterIndex: character.index }, e => e.packet === 'StartGame', Math.min(this.now() + 20000, deadlineAtMs), 'startGame');
    check(start.payload?.result === 4, 'StartGame rejected');
    const deadline = Math.min(start.atMs + 60000, deadlineAtMs);
    const proof = await this.wait(() => {
      const user = this.events.find(e => e.receiveSeq > cursor && e.packet === 'UserInformation' && e.payload?.name === this.actor.name);
      const map = this.events.find(e => e.receiveSeq > cursor && e.packet === 'MapInformation');
      const snapshot = this.latestSnapshotEvent?.receiveSeq > Math.max(user?.receiveSeq ?? Infinity, map?.receiveSeq ?? Infinity) &&
        owner(this.latestSnapshotEvent.payload)?.name === this.actor.name ? this.latestSnapshotEvent : null;
      const p = owner(snapshot?.payload), ui = user?.payload, loc = ui?.location ?? ui;
      return user && map && snapshot && user.receiveSeq > start.receiveSeq && map.receiveSeq > start.receiveSeq &&
        ui.objectId === snapshot.payload.playerObjectId && ui.hp > 0 && p?.dead !== true && snapshot.payload.playerHp > 0 && p?.level === 40 &&
        String(map.payload.fileName) === 'DogYoHyun' && String(snapshot.payload.mapFileName) === 'DogYoHyun' &&
        p.x === loc.x && p.y === loc.y ? { atMs: snapshot.atMs, start: receiveReceipt(this, start), user: receiveReceipt(this, user),
          map: receiveReceipt(this, map), snapshot: receiveReceipt(this, snapshot) } : null;
    }, deadline, 'fresh exact owner bootstrap');
    this.bootstrapProof = proof;
    this.ownedSnapshotEvidence('authenticatedBootstrap');
    this.emit({ event: 'admitted', actorOrdinal: this.ordinal, connection: this.connection, ...proof, state: savedState(this.snapshot) }); return proof;
  }
  async close(outerDeadline = Infinity) {
    clearInterval(this.timer);
    if (!this.ws || this.ws.readyState === 3) return { passed: this.closed && Boolean(this.ws?._closeFrameReceived), elapsedMs: 0 };
    const began = this.now(); this.emit({ event: 'clientCloseStarted', actorOrdinal: this.ordinal, connection: this.connection, atMs: began, atUtc: at() });
    this.ws.close(1000, 'comparison complete');
    try { await this.wait(() => this.closed, Math.min(began + 5000, outerDeadline), 'independent WS close', { ignoreFailure: true }); }
    catch (e) { this.ws.terminate(); throw e; }
    const result = { passed: Boolean(this.ws._closeFrameReceived), elapsedMs: this.now() - began };
    check(result.passed, 'No real WebSocket Close frame received'); return result;
  }
}

export function observedSharedId(observer, actor) {
  const id = observer.sharedPlayers.get(actor.name);
  if (!Number.isSafeInteger(id) || id <= 0 || id === Number(observer.snapshot?.playerObjectId)) {
    const error = new Error('Fresh peer shared namespace missing'); error.hardEvidence = true; throw error;
  } return id;
}
export function chatMatches(payload, id, nonce) {
  if (Number(payload?.objectId) !== id) return false;
  const text = String(payload?.text ?? payload?.message ?? '');
  return text.endsWith(nonce) && (text.length === nonce.length || /[\s:]/.test(text[text.length - nonce.length - 1]));
}

export function offerAction(client, clients, plan, action, epoch, now = performance.now()) {
  const peers = plan.actors[client.ordinal].peerOrdinals.map(i => clients[i]);
  const spec = { ...action, deadlineAtMs: epoch + action.deadlineMs, dueAtMs: epoch + action.dueMs,
    offeredAtMs: now, offeredDelayMs: Math.max(0, now - epoch - action.dueMs),
    expectedPeerOrdinals: peers.map(p => p.ordinal), nonceHash: action.nonce ? hash(action.nonce) : null };
  if (!client.gate.start(spec, now)) return null;
  if (now >= spec.deadlineAtMs || client.pending || client.failure || client.closed) {
    client.gate.failed = true; const pending = client.gate.pending; client.gate.pending = null;
    const row = { ...pending, event: 'notSent', reason: now >= spec.deadlineAtMs ? 'deadlineBeforeSend' : 'clientUnavailable', passed: false,
      hardEvidence: Boolean(client.failure || client.closed), timedOut: false };
    client.emit(row); return row;
  }
  const expected = [], cursors = new Map([client, ...peers].map(c => [c.ordinal, c.sequence]));
  let sendAttempted = false;
  try {
    if (action.kind === 'walk') {
      const before = owner(client.snapshot), delta = action.direction === 'Right' ? 1 : -1;
      check(before && before.x + delta === action.target.x && before.y === action.target.y, 'Planned lane target no longer matches authoritative owner');
      expected.push({ client, packet: 'UserLocation', match: e => (e.payload?.location?.x ?? e.payload?.x) === action.target.x && (e.payload?.location?.y ?? e.payload?.y) === action.target.y });
      for (const peer of peers) { const id = observedSharedId(peer, client.actor); expected.push({ client: peer, packet: 'ObjectWalk', sharedId: id,
        match: e => Number(e.payload?.objectId) === id && (e.payload?.location?.x ?? e.payload?.x) === action.target.x && (e.payload?.location?.y ?? e.payload?.y) === action.target.y }); }
      Object.assign(client.gate.pending, { actualTarget: action.target, expectedSharedIds: expected.filter(e => e.sharedId).map(e => ({ observerOrdinal: e.client.ordinal, connection: e.client.connection, objectId: e.sharedId })), before: { x: before.x, y: before.y } });
    } else {
      const ids = peers.map(p => observedSharedId(p, client.actor));
      if (new Set(ids).size !== 1) { const error = new Error('Peers disagree about chat speaker shared ID'); error.hardEvidence = true; throw error; }
      for (const observer of [client, ...peers]) expected.push({ client: observer, packet: 'ObjectChat', sharedId: ids[0], match: e => chatMatches(e.payload, ids[0], action.nonce) });
      Object.assign(client.gate.pending, { actualTarget: { nonceHash: hash(action.nonce) }, expectedSharedIds: expected.map(e => ({ observerOrdinal: e.client.ordinal, connection: e.client.connection, objectId: e.sharedId })) });
    }
    Object.assign(client.gate.pending, { expected, cursors });
    sendAttempted = true;
    const sent = client.send(action.kind === 'walk' ? { type: 'walk', direction: action.direction } : { type: 'chat', message: action.nonce });
    Object.assign(client.gate.pending, { sentAtMs: sent, scheduleDelayMs: Math.max(0, sent - spec.dueAtMs) });
    client.emit({ event: 'actionSent', ...spec, sentAtMs: sent, scheduleDelayMs: Math.max(0, sent - spec.dueAtMs),
      actualTarget: client.gate.pending.actualTarget, expectedSharedIds: client.gate.pending.expectedSharedIds,
      receiveSequenceFloors: [client, ...peers].map(c => ({ observerOrdinal: c.ordinal, actorOrdinal: c.ordinal, connection: c.connection, receiveSeq: cursors.get(c.ordinal) })) });
  } catch (error) {
    if (sendAttempted) {
      client.gate.failed = true; Object.assign(client.gate.pending, { sendFailed: true, sentAtMs: client.now(), failure: 'Uncertain transport send' });
      client.emit({ event: 'sendUncertain', ...spec, passed: false, hardEvidence: true }); return null;
    }
    const row = { ...spec, event: 'notSent', reason: 'preconditionFailed', failure: error.message, hardEvidence: Boolean(error.hardEvidence), passed: false };
    client.gate.failed = true; client.gate.pending = null; client.emit(row); return row;
  }
  return null;
}

export function observeActions(clients) {
  for (const speaker of clients) {
    speaker.reconcileLate();
    const p = speaker.gate.pending; if (!p?.expected) continue;
    let cache = actionReceipts.get(p);
    if (!cache) {
      cache = { witnesses: new WeakMap(),
        correction: new RetainedReceiveCursor(p.cursors.get(speaker.ordinal), e => e.packet === 'UserLocation') };
      actionReceipts.set(p, cache);
    }
    const receipts = p.expected.map(w => {
      let cursor = cache.witnesses.get(w);
      if (!cursor) { cursor = new RetainedReceiveCursor(p.cursors.get(w.client.ordinal), e => e.packet === w.packet && w.match(e)); cache.witnesses.set(w, cursor); }
      return cursor.matches(w.client.events)[0];
    });
    // A real authoritative rejection closes the attempted move immediately.
    if (p.kind === 'walk' && !receipts[0]) {
      const correction = cache.correction.matches(speaker.events)[0];
      if (correction) { speaker.gate.close([receiveReceipt(speaker, correction)], correction.atMs, false); continue; }
    }
    if (receipts.every(Boolean)) speaker.gate.close(receipts.map((e, i) => receiveReceipt(p.expected[i].client, e)), Math.max(...receipts.map(e => e.atMs)));
  }
}

export async function equipmentRoundtrip(client, rounds, emit, freshState) {
  check(typeof freshState === 'function', 'Equipment state requires ordinary logout/close/fresh-socket bootstrap');
  const result = { actorOrdinal: client.ordinal, passed: false, rounds: [] }, original = equipmentFingerprint(client.snapshot);
  const originalSource = client.ownedSnapshotEvidence('beforeEquipmentMutation');
  emit({ event: 'equipmentOriginalState', actorOrdinal: client.ordinal, connection: client.connection,
    sourceReceipt: originalSource.sourceReceipt, admissionSnapshot: client.bootstrapProof?.snapshot,
    atMs: client.now(), atUtc: at(), state: { inventoryItems: client.snapshot.inventoryItems,
    equipmentItems: client.snapshot.equipmentItems, beltItems: client.snapshot.beltItems, gold: client.snapshot.gold,
    inventoryCapacity: client.snapshot.inventoryCapacity, maxBagSlots: client.snapshot.maxBagSlots }, fingerprint: original });
  const item = client.snapshot.equipmentItems.find(i => i.slot === 'weapon' && Number.isSafeInteger(Number(i.uniqueId)) && Number(i.uniqueId) > 0);
  check(item, 'Real equipped ordinary item required'); const uniqueId = item.uniqueId, slot = SLOT_NAMES.indexOf(item.slot);
  const occupied = new Set(client.snapshot.inventoryItems.map(i => Number(i.slot)));
  const bag = Array.from({ length: Math.min(40, client.snapshot.maxBagSlots) }, (_, i) => i).find(i => !occupied.has(i)); check(bag !== undefined, 'Empty bag cell required');
  for (let round = 0; round < rounds; round++) for (const [type, packet, to] of [['removeItem', 'RemoveItem', bag], ['equipItem', 'EquipItem', slot]]) {
    const began = client.now(), ack = await client.request({ type, grid: 'inventory', uniqueId, to }, e => e.packet === packet && e.payload?.uniqueId === uniqueId && e.payload?.to === to && String(e.payload?.grid).toLowerCase() === 'inventory', began + 15000, type);
    check(ack.payload.success === true, 'Equipment ACK refused');
    const ackReceipt = receiveReceipt(client, ack), previousConnection = client.connection, refreshed = await freshState(client, ack.atMs + 15000);
    client = refreshed.client; const snapshot = client.snapshot, fresh = refreshed.proof.snapshot;
    check(client.connection !== previousConnection && fresh.connection === client.connection && fresh.atMs <= ack.atMs + 15000 &&
      owner(snapshot)?.name === client.actor.name && snapshot.mapFileName === 'DogYoHyun' && snapshot.playerHp > 0 &&
      (type === 'removeItem' ? snapshot.inventoryItems?.some(i => i.uniqueId === uniqueId && Number(i.slot) === bag) && !snapshot.equipmentItems?.some(i => i.uniqueId === uniqueId)
        : snapshot.equipmentItems?.some(i => i.uniqueId === uniqueId && i.slot === item.slot) && !snapshot.inventoryItems?.some(i => i.uniqueId === uniqueId) && equipmentFingerprint(snapshot) === original),
    'Equipment lacks complete fresh-socket saved state within original15s');
    const row = { round, type, uniqueId, to, ackElapsedMs: ack.atMs - began, stateElapsedMs: fresh.atMs - ack.atMs,
      ack: ackReceipt, state: fresh, refreshLifecycle: refreshed.lifecycle,
      freshState: carriedState(snapshot),
      stateFingerprint: equipmentFingerprint(snapshot), passed: true };
    result.rounds.push(row); emit({ event: 'equipmentStep', actorOrdinal: client.ordinal, ...row });
  }
  result.passed = equipmentFingerprint(client.snapshot) === original; check(result.passed, 'Carried-state fingerprint changed after roundtrip'); return result;
}

export async function freshEquipmentState(old, deadline, { make, replace, reserve, now }) {
  const began = now(), ack = await old.request({ type: 'logOut' }, e => ['LogOutSuccess', 'LogOutFailed'].includes(e.packet), Math.min(now() + 5000, deadline), 'equipmentNormalLogout');
  check(ack.packet === 'LogOutSuccess', 'Equipment normal logout refused');
  const close = await old.close(deadline); await reserve('equipmentState', old.ordinal, deadline);
  const fresh = make(old.actor); replace(old.ordinal, fresh); const proof = await fresh.login({ deadlineAtMs: deadline });
  return { client: fresh, proof, lifecycle: { oldConnection: old.connection, newConnection: fresh.connection,
    logoutAck: receiveReceipt(old, ack), logoutElapsedMs: ack.atMs - began, close, outerDeadlineAtMs: deadline } };
}

function cleanGate(row) {
  const { expected, cursors, matcher, ...safe } = row; return safe;
}

export function runtimeSourceProof(runtime, candidateBuild = null) {
  check(/^[0-9a-f]{40}$/.test(runtime?.sourceRevision) && /^[0-9a-f]{64}$/.test(runtime?.binarySha256), 'Exact source/binary pins required');
  if (!candidateBuild) {
    const overlayDeclared = runtime.changesKind !== undefined || runtime.sourceInputManifestSha256 !== undefined ||
      runtime.buildManifestSchema !== undefined || runtime.sourceRevisionIsBaseOnly === true || runtime.additionalSource != null ||
      (runtime.baseRevision !== undefined && runtime.baseRevision !== runtime.sourceRevision);
    check(!overlayDeclared, 'Runtime overlay requires explicit --build-manifest');
    return { sourceProof: 'owned-controller-receipt', sourceRevisionIsBaseOnly: false, additionalSource: null };
  }
  const manifest = candidateBuild.manifest, isSha = v => typeof v === 'string' && /^[0-9a-f]{64}$/.test(v);
  const kinds = {
    'mir2.comparable.observer-timing-build.v1': { changesKind: 'observer-timing-only', proof: 'base-plus-observer-timing-overlay', observerTimingOnly: true },
    'mir2.comparable.spectator-scene-build.v1': { changesKind: 'spectator-scene-projection', proof: 'base-plus-spectator-scene-projection', observerTimingOnly: false },
  };
  const reviewed = Object.hasOwn(kinds, manifest?.schema) ? kinds[manifest.schema] : null;
  check(reviewed && manifest.baseRevision === CANDIDATE_BASE_REVISION && manifest.changesKind === reviewed.changesKind &&
    manifest.buildActualExitCode === 0 && isSha(manifest.binarySha256) && isSha(manifest.sourceInputManifestSha256) && isSha(candidateBuild.sha256),
  'Reviewed candidate build manifest invalid');
  check(runtime.sourceRevision === CANDIDATE_BASE_REVISION && runtime.baseRevision === manifest.baseRevision &&
    runtime.changesKind === manifest.changesKind && runtime.buildActualExitCode === manifest.buildActualExitCode &&
    runtime.binarySha256 === manifest.binarySha256 && runtime.sourceInputManifestSha256 === manifest.sourceInputManifestSha256 &&
    runtime.buildManifestSha256 === candidateBuild.sha256 &&
    (runtime.buildManifestSchema === undefined || runtime.buildManifestSchema === manifest.schema), 'Runtime candidate manifest binding mismatch');
  return { sourceProof: reviewed.proof, sourceRevisionIsBaseOnly: true,
    additionalSource: { buildManifestSchema: manifest.schema, observerTimingOnly: reviewed.observerTimingOnly,
      buildManifestSha256: candidateBuild.sha256, sourceInputManifestSha256: manifest.sourceInputManifestSha256,
      baseRevision: manifest.baseRevision, changesKind: manifest.changesKind, buildActualExitCode: manifest.buildActualExitCode,
      binarySha256: manifest.binarySha256, cleanBaseBuild: false, behaviorFixClaimed: false, capacityClaimAllowed: false } };
}

function sameSourcePath(a, b) {
  const normalize = value => process.platform === 'win32' ? path.resolve(value).toLowerCase() : path.resolve(value);
  return normalize(a) === normalize(b);
}

async function sourceFile(file, maximum) {
  check(path.isAbsolute(file), 'Absolute source file required');
  for (let current = file;; current = path.dirname(current)) {
    const stat = await fs.lstat(current);
    check(!stat.isSymbolicLink(), 'Linked adapter source path rejected');
    if (path.dirname(current) === current) break;
  }
  check(sameSourcePath(await fs.realpath(file), file), 'Adapter source path escapes its physical path');
  const stat = await fs.lstat(file);
  check(stat.isFile() && stat.size > 0 && stat.size <= maximum, 'Bounded regular adapter source file required');
  return fs.readFile(file);
}

function staticSourceImports(text) {
  const imports = [], statements = [
    /(?:^|[;\r\n])\s*import\s+(?:[^;]*?\s+from\s+)?['"]([^'"]+)['"]/g,
    /(?:^|[;\r\n])\s*export\s+(?:\{[^}]*\}|\*\s*(?:as\s+\w+)?)\s*from\s*['"]([^'"]+)['"]/g,
  ];
  for (const statement of statements) for (const match of text.matchAll(statement)) {
    const specifier = match[1];
    if (specifier.startsWith('node:')) continue;
    check(/^\.\/[A-Za-z0-9][A-Za-z0-9._-]*\.mjs$/.test(specifier), 'Unbound or escaping static adapter import');
    imports.push(specifier.slice(2));
  }
  return imports;
}

export async function loadAdapterSources(manifestFile, runtime, wsModuleFile) {
  check(path.isAbsolute(manifestFile) && /^c:[\\/]/i.test(manifestFile), 'Absolute C-only --adapter-sources required');
  const bytes = await sourceFile(manifestFile, 65536), manifest = JSON.parse(bytes), root = path.dirname(manifestFile);
  check(manifest.schema === 'mir2.comparable.modern-adapter-sources.v1' && ['driver.mjs', 'melee-driver.mjs'].includes(manifest.entrypoint) &&
    Array.isArray(manifest.files) && manifest.files.length >= 2 && manifest.files.length <= 16, 'Adapter source manifest schema/entrypoint invalid');
  const names = new Set(), files = new Map(), publicRows = []; let totalBytes = 0;
  for (const row of manifest.files) {
    check(typeof row.path === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]*\.mjs$/.test(row.path) && !names.has(row.path.toLowerCase()) &&
      Number.isSafeInteger(row.bytes) && row.bytes > 0 && row.bytes <= 1048576 && /^[0-9a-f]{64}$/.test(row.sha256), 'Adapter source row invalid or duplicate');
    names.add(row.path.toLowerCase()); totalBytes += row.bytes; check(totalBytes <= 4194304, 'Adapter source manifest exceeds byte budget');
    const content = await sourceFile(path.join(root, row.path), 1048576);
    check(content.length === row.bytes && hash(content) === row.sha256, 'Adapter source file bytes/hash mismatch');
    files.set(row.path, content.toString('utf8')); publicRows.push({ path: row.path, bytes: row.bytes, sha256: row.sha256 });
  }
  check(files.has('driver.mjs') && files.has('contract.mjs') && files.has(manifest.entrypoint) &&
    (manifest.entrypoint !== 'melee-driver.mjs' || files.has('combat.mjs')), 'Required executed adapter sources missing');
  check(sameSourcePath(path.join(root, 'driver.mjs'), fileURLToPath(import.meta.url)), 'Adapter manifest is unrelated to the executing driver module');
  check(process.argv[1] && sameSourcePath(path.join(root, manifest.entrypoint), await fs.realpath(process.argv[1])), 'Adapter manifest entrypoint differs from actual process entrypoint');
  const visited = new Set(), pending = [manifest.entrypoint, 'driver.mjs'];
  while (pending.length) {
    const name = pending.pop(); if (visited.has(name)) continue;
    check(files.has(name), 'Actual static adapter dependency is absent from manifest'); visited.add(name);
    for (const dependency of staticSourceImports(files.get(name))) pending.push(dependency);
  }
  const ws = manifest.wsModule;
  check(ws && typeof ws.path === 'string' && path.isAbsolute(ws.path) && /^c:[\\/]/i.test(ws.path) &&
    Number.isSafeInteger(ws.bytes) && ws.bytes > 0 && ws.bytes <= 4194304 && /^[0-9a-f]{64}$/.test(ws.sha256) &&
    typeof wsModuleFile === 'string' && sameSourcePath(ws.path, wsModuleFile), 'Explicit WebSocket source binding invalid');
  const wsBytes = await sourceFile(wsModuleFile, 4194304);
  check(wsBytes.length === ws.bytes && hash(wsBytes) === ws.sha256, 'Executed WebSocket module bytes/hash mismatch');
  const manifestSha256 = hash(bytes), entry = publicRows.find(row => row.path === manifest.entrypoint);
  check(runtime.adapterSourceManifestSha256 === manifestSha256 && runtime.adapterEntrypointSha256 === entry.sha256,
    'Runtime adapter source manifest/entrypoint hash mismatch');
  const proof = { adapterSourceManifestSha256: manifestSha256, adapterEntrypointSha256: entry.sha256,
    adapterSource: Object.freeze({ schema: manifest.schema, entrypoint: manifest.entrypoint, files: Object.freeze(publicRows.map(Object.freeze)),
      wsModule: Object.freeze({ path: ws.path, bytes: ws.bytes, sha256: ws.sha256 }), dependencyScope: 'static-esm-closure-plus-explicit-websocket-module' }) };
  Object.defineProperty(proof, ADAPTER_SOURCE_PROOF, { value: true }); return Object.freeze(proof);
}

function adapterSourceFields(runtime, candidateBuild, proof) {
  const required = candidateBuild || runtime.adapterSourceManifestSha256 !== undefined || runtime.adapterEntrypointSha256 !== undefined;
  check(!required || proof?.[ADAPTER_SOURCE_PROOF] === true, 'Verified --adapter-sources required for candidate or declared adapter provenance');
  if (!proof) return {};
  check(proof[ADAPTER_SOURCE_PROOF] === true && proof.adapterSourceManifestSha256 === runtime.adapterSourceManifestSha256 &&
    proof.adapterEntrypointSha256 === runtime.adapterEntrypointSha256, 'Adapter source proof differs from runtime');
  return { adapterSourceManifestSha256: proof.adapterSourceManifestSha256, adapterEntrypointSha256: proof.adapterEntrypointSha256,
    adapterSource: proof.adapterSource };
}
export function carriedState(snapshot) {
  return { inventoryItems: snapshot.inventoryItems, equipmentItems: snapshot.equipmentItems, beltItems: snapshot.beltItems,
    gold: snapshot.gold, inventoryCapacity: snapshot.inventoryCapacity, maxBagSlots: snapshot.maxBagSlots };
}
export function verifyInitialFixture(snapshot, start) {
  const p = owner(snapshot);
  check(p?.level === 40 && p.x === start.x && p.y === start.y && snapshot.playerHp === 674 && snapshot.playerMp === 151 &&
    snapshot.gold === 0 && snapshot.inventoryCapacity === 46 && snapshot.maxBagSlots === 40 && snapshot.beltItems?.length === 0 &&
    snapshot.inventoryItems?.length === 9 && snapshot.inventoryItems.every(i => i.key === 'crystal-item-662' && Number(i.quantity) === 20), 'Initial fixture vitals/items/capacity differ');
  const expected = [['weapon', 221, 4000], ['armour', 317, 5000], ['braceletLeft', 482, 7000], ['braceletRight', 482, 7000]];
  check(snapshot.equipmentItems?.length === 4 && expected.every(([slot, index, durability]) => snapshot.equipmentItems.some(i => i.slot === slot &&
    i.key === `crystal-item-${index}` && i.quantity === 1 && i.durabilityCurrent === durability && i.durabilityMax === durability &&
    Number(i.tooltipSource?.userItem?.itemIndex ?? i.tooltipSource?.userItem?.item_index) === index)), 'Initial ordinary equipment differs');
  const ids = [...snapshot.inventoryItems, ...snapshot.equipmentItems].map(i => Number(i.uniqueId));
  check(ids.every(i => Number.isSafeInteger(i) && i > 0) && new Set(ids).size === ids.length, 'Initial carried UID identity differs');
}
export async function runComparison({ plan, cohort, runtime, url, WebSocket, emit, ledger, saveLedger, now = () => performance.now(), wait = sleep, engine = null, candidateBuild = null, adapterSources = null }) {
  (engine?.validatePlan ?? validatePlan)(plan); endpoint(url);
  const Client = engine?.Client ?? ModernClient, score = engine?.scoreWindows ?? scoreWindows;
  check(cohort.schema === 'mir2.comparable.modern-cohort.v1' && cohort.qualificationLayer === 'game-core-local' && cohort.mapSha256 === MAP_SHA && cohort.actors.length === plan.population, 'Wrong isolated cohort');
  check(cohort.actors.every((a, i) => a.ordinal === i && Number.isSafeInteger(a.characterIndex) && a.characterIndex >= 0 &&
    typeof a.name === 'string' && typeof a.accountId === 'string' && !a.accountId.startsWith('sui:') && typeof a.password === 'string') &&
    new Set(cohort.actors.map(a => a.accountId)).size === plan.population && new Set(cohort.actors.map(a => a.name)).size === plan.population, 'Ordinary exact cohort identity mismatch');
  check(runtime.schema === 'mir2.comparable.modern-runtime.v1' && runtime.qualificationLayer === 'game-core-local' && runtime.backend === 'file' && runtime.sessionCache === 'in-memory' &&
    /^[0-9a-f]{40}$/.test(runtime.sourceRevision) && /^[0-9a-f]{64}$/.test(runtime.binarySha256) && runtime.endpoint === url, 'Source-pinned File/InMemory runtime receipt required');
  const sourceProof = runtimeSourceProof(runtime, candidateBuild);
  const adapterProof = adapterSourceFields(runtime, candidateBuild, adapterSources);
  const report = { schema: 'mir2.comparable.modern-report.v1', processId: process.pid, qualificationLayer: 'game-core-local', scenario: plan.scenario,
    planId: plan.planId, generatorSourceSha256: plan.generatorSourceSha256, assessment: plan.assessment, resourceContract: plan.resourceContract,
    sourceRevision: runtime.sourceRevision, binarySha256: runtime.binarySha256, ...sourceProof, ...adapterProof,
    backend: 'file', sessionCache: 'in-memory', transport: 'plain-Web-loopback', populationRequested: plan.population,
    admitted: 0, actions: [], equipment: [], cleanup: [], savedVerification: [], windows: [], startedAtUtc: at(), runCompleted: false,
    capacityClaimAllowed: false, productionStackQualified: false, fullCombatQualified: false, physicalTriClientQualified: false, ok: false };
  if (engine) Object.assign(report, engine.reportFields);
  const clients = [], allClients = [], rows = new Map(); let connection = 0, sticky = false, hardEvidenceFailure = false, measurementFinished = false;
  const record = raw => { const row = cleanGate(raw); emit(row); if (row.hardEvidence) { sticky = true; hardEvidenceFailure = true; }
    engine?.record?.(row);
    if (['actionTerminal', 'actionTimeout', 'notSent'].includes(row.event) && Number.isInteger(row.ticketSequence)) rows.set(`${row.actorOrdinal}:${row.ticketSequence}`, row); };
  let measurementEpoch, measurementEnd;
  const advance = (current = clients, enforceSafety = true) => {
    (engine?.observeActions ?? observeActions)(current, now());
    for (const c of current) c.gate.expire(now());
    if (engine && enforceSafety) {
      engine.checkSafety(current);
      if (now() >= measurementEpoch && now() < measurementEnd) engine.poll?.(current, now());
      engine.checkSafety(current);
    }
  };
  const ensureLoginSlots = async required => {
    check(required <= 30, 'Phase needs more than one normal login window');
    while (localLoginDelay(ledger.attempts, Date.now(), required)) {
      emit({ event: 'authWindowWait', reason: 'reserveCompleteEquipmentPhaseBeforeMutation', required, atUtc: at(), admitted: report.admitted });
      await wait(1000);
    }
  };
  const reserve = async (purpose, actorOrdinal, deadline = Infinity) => {
    check(ledger.schema === 'mir2.comparable.local-auth-ledger.v1' && ledger.endpoint === url && Array.isArray(ledger.attempts), 'Dedicated local ledger required');
    for (;;) {
      const wall = Date.now(), delay = localLoginDelay(ledger.attempts, wall);
      if (!delay) { check(now() < deadline, 'Outer phase deadline expired before ordinary login'); ledger.attempts.push({ atMs: wall, purpose }); await saveLedger(ledger);
        emit({ event: 'authReserved', actorOrdinal, purpose, attemptNumber: ledger.attempts.length, atUtc: at() }); return; }
      check(now() < deadline, 'Outer phase cannot wait for another auth window');
      emit({ event: 'authWindowWait', purpose, atUtc: at(), admitted: report.admitted }); await wait(Math.min(1000, delay, Math.max(1, deadline - now())));
    }
  };
  const make = actor => { const c = new Client({ url, actor, connection: ++connection, WebSocket, emit: record, now }); allClients.push(c); return c; };
  try {
    for (const actor of cohort.actors) { await reserve('admission', actor.ordinal); const c = make(actor); clients[actor.ordinal] = c; await c.login(); report.admitted++; }
    for (const [i, c] of clients.entries()) {
      const expected = plan.actors[i].start, p = owner(c.snapshot);
      check(p.x === expected.x && p.y === expected.y, 'Exact fixture start transformed; no repositioning fallback');
      verifyInitialFixture(c.snapshot, expected);
      for (const peer of plan.actors[i].peerOrdinals) observedSharedId(clients[peer], c.actor);
    }
    const gearActors = plan.equipmentActorOrdinals ?? [0, 1], gearRounds = plan.equipmentRoundsPerActor ?? 2;
    await ensureLoginSlots(gearActors.length * gearRounds * 2 + (plan.savedVerificationActorOrdinals ?? [0, 1]).length);
    const refresh = (old, deadline) => freshEquipmentState(old, deadline, { make, replace: (i, fresh) => { clients[i] = fresh; }, reserve, now });
    for (const i of gearActors) report.equipment.push(await equipmentRoundtrip(clients[i], gearRounds, record, refresh));
    if (engine) await engine.setup(clients, plan);
    const epoch = now() + 100; report.measurementStartedAtUtc = new Date(Date.now() + 100).toISOString(); report.measurementEpochMs = epoch;
    report.measurementPlannedEndAtUtc = new Date(Date.now() + 100 + plan.durationMs).toISOString();
    const steadyEnd = epoch + plan.durationMs;
    measurementEpoch = epoch; measurementEnd = steadyEnd;
    for (const c of clients) c.trafficWindow = { startAtMs: epoch, endAtMs: steadyEnd, sent: 0, received: 0 };
    let countersStart;
    await runScheduledActions(plan.actions, { epoch, now, wait, advance,
      firstOffer() { countersStart = clients.map(c => c.counterSample()); for (const sample of countersStart) record({ event: 'steadyCounter', phase: 'start', ...sample }); },
      offer(action, current) { (engine?.offerAction ?? offerAction)(clients[action.actorOrdinal], clients, plan, action, epoch, current); } });
    const drainEnd = epoch + Math.max(plan.durationMs, ...plan.actions.map(a => a.deadlineMs));
    while (now() < steadyEnd) { advance(); await wait(Math.min(10, steadyEnd - now())); }
    // Capture the steady end before the original-deadline tail is observed.
    const countersEnd = clients.map(c => c.counterSample());
    for (const sample of countersEnd) record({ event: 'steadyCounter', phase: 'end', ...sample });
    report.steadyApplicationTraffic = { scope: 'JSON WebSocket payload bytes submitted/received; excludes WS framing/control/TCP/IP; no deflate',
      plannedStartAtMs: epoch, plannedEndAtMs: steadyEnd, plannedDurationMs: plan.durationMs, exactWindowMethod: 'per-frame epoch<=atMs<epoch+durationMs',
      actors: clients.map((c, i) => { const start = countersStart[i], end = countersEnd[i]; check(start.connection === end.connection, 'Steady counter connection changed');
        return { actorOrdinal: c.ordinal, connection: c.connection,
          bytesStart: { sent: start.sentBytes, received: start.receivedBytes, actualCounterAtMs: start.actualCounterAtMs },
          bytesEnd: { sent: end.sentBytes, received: end.receivedBytes, actualCounterAtMs: end.actualCounterAtMs },
          actualDurationMs: end.actualCounterAtMs - start.actualCounterAtMs,
          startSampleDeviationMs: start.actualCounterAtMs - epoch, endSampleDeviationMs: end.actualCounterAtMs - steadyEnd,
          sentBytes: c.trafficWindow.sent, receivedBytes: c.trafficWindow.received }; }) };
    while (now() < drainEnd) { advance(); await wait(Math.min(10, drainEnd - now())); }
    advance();
    report.observationEndedAtUtc = at(); report.observationEndedAtMs = now(); report.measurementEndedAtUtc = report.observationEndedAtUtc;
    report.observationTailMs = drainEnd - epoch - plan.durationMs;
    report.actions = [...rows.values()]; report.windows = score(plan, report.actions);
    hardEvidenceFailure ||= clients.some(c => Boolean(c.failure) || c.closed || !c.inGame) || report.actions.length !== plan.actions.length;
    sticky ||= hardEvidenceFailure; measurementFinished = true;
    await wait(plan.settleBeforeLogoutMs ?? 11000);
  } catch (error) { sticky = true; report.failure = error.message; }
  finally {
    const saved = new Map(), savedFull = new Map();
    for (const c of clients.filter(Boolean)) {
      const row = { actorOrdinal: c.ordinal, normalLogout: false, transportClose: false };
      try {
        const end = now() + 5000;
        while ((c.gate.pending || c.pending) && now() < end) { advance(clients.filter(Boolean), false); c.reconcileLate(); await wait(10); }
        check(!c.gate.pending && !c.pending, 'Unique pending action never closed; no new logout request sent');
        saved.set(c.ordinal, savedState(c.snapshot));
        savedFull.set(c.ordinal, structuredClone(carriedState(c.snapshot)));
        const began = now(), ack = await c.request({ type: 'logOut' }, e => ['LogOutSuccess', 'LogOutFailed'].includes(e.packet), began + 5000, 'normalLogout', { ignoreFailure: true });
        row.logoutAckMs = ack.atMs - began; row.logoutReceipt = receiveReceipt(c, ack); check(ack.packet === 'LogOutSuccess', 'Normal logout refused'); row.normalLogout = true;
        if (engine?.captureSavedState) {
          const captured = engine.captureSavedState(c, ack);
          saved.set(c.ordinal, captured.state); savedFull.set(c.ordinal, captured.carried); row.savedStateSource = captured.source;
        }
      } catch (e) { sticky = true; row.failure = e.message; }
      try { row.close = await c.close(); row.transportClose = row.close.passed; } catch (e) { sticky = true; row.closeFailure = e.message; }
      report.cleanup.push(row);
    }
    for (const ordinal of plan.savedVerificationActorOrdinals ?? [0, plan.population - 1]) {
      const row = { actorOrdinal: ordinal, passed: false, normalLogout: false, transportClose: false }, prior = saved.get(ordinal); let c;
      try {
        check(prior && report.cleanup.find(r => r.actorOrdinal === ordinal)?.normalLogout && report.cleanup.find(r => r.actorOrdinal === ordinal)?.transportClose, 'Original exact owner did not exit cleanly');
        await reserve('verification', ordinal); c = make(cohort.actors[ordinal]); await c.login();
        const verifiedState = engine?.captureVerificationState?.(c), actual = verifiedState?.state ?? savedState(c.snapshot);
        row.beforeHash = hash(prior); row.afterHash = hash(actual); row.before = { ...prior, carried: savedFull.get(ordinal) };
        row.after = { ...actual, carried: verifiedState?.carried ?? carriedState(c.snapshot) }; row.bootstrap = c.bootstrapProof;
        if (verifiedState) row.savedStateSource = verifiedState.source;
        row.statePreserved = JSON.stringify(canonical(actual)) === JSON.stringify(canonical(prior));
        emit({ event: 'savedVerificationState', actorOrdinal: ordinal, connection: c.connection, before: row.before, after: row.after, bootstrap: row.bootstrap,
          statePreserved: row.statePreserved, ...(verifiedState ? { savedStateSource: verifiedState.source } : {}) });
        check(row.statePreserved, 'Fresh-socket relogin full saved state differs');
      } catch (e) { sticky = true; row.failure = e.message; }
      finally {
        if (c) {
          if (c.inGame && !c.closed) {
            try {
              // Exactly one ordinary Logout attempt, even after a comparison failure.
              // An unknown/rejected result is retained; Close never retries it.
              const began = now(), ack = await c.request({ type: 'logOut' }, e => ['LogOutSuccess', 'LogOutFailed'].includes(e.packet), began + 5000, 'verificationLogout', { ignoreFailure: true });
              row.logoutAckMs = ack.atMs - began; row.logoutReceipt = receiveReceipt(c, ack);
              check(ack.packet === 'LogOutSuccess', 'Verification logout refused'); row.normalLogout = true;
            } catch (e) { sticky = true; row.logoutFailure = e.message; }
          }
          try { row.close = await c.close(); row.transportClose = row.close.passed; }
          catch (e) { sticky = true; row.closeFailure = e.message; }
        }
        row.passed = row.statePreserved === true && row.normalLogout && row.transportClose;
      }
      report.savedVerification.push(row);
    }
    if (engine) { const valid = engine.evidenceValid(allClients); sticky ||= !valid; hardEvidenceFailure ||= !valid; engine.finalizeReport(report); }
    report.actions = [...rows.values()]; report.windows = freezeWindowFailures(report.windows, score(plan, report.actions));
    report.evidenceValid = measurementFinished && !hardEvidenceFailure;
    report.measurementSloPassed = report.evidenceValid && report.windows.every(w => w.passed);
    for (const c of allClients) clearInterval(c.timer);
    report.applicationBytes = allClients.map(c => ({ actorOrdinal: c.ordinal, connection: c.connection, sent: c.sentBytes, received: c.receivedBytes }));
    report.runCompleted = true; report.completedAtUtc = at();
    report.ok = !sticky && report.admitted === plan.population && report.equipment.every(r => r.passed) && report.windows.every(w => w.passed) &&
      report.cleanup.length === plan.population && report.cleanup.every(r => r.normalLogout && r.transportClose) && report.savedVerification.every(r => r.passed);
  }
  return report;
}

export function parseComparisonOptions(args, extraOptions = []) {
  const options = {};
  check(args.length % 2 === 0, 'Explicit option/value pairs required');
  const required = ['--plan', '--cohort', '--runtime-receipt', '--ledger', '--output', '--ws-module', ...extraOptions];
  const allowed = [...required, '--build-manifest', '--adapter-sources'];
  for (let i = 0; i < args.length; i += 2) { check(allowed.includes(args[i]) && !Object.hasOwn(options, args[i]), 'Unknown/duplicate option'); options[args[i]] = args[i + 1]; }
  for (const key of required) check(options[key] && path.isAbsolute(options[key]), `Absolute ${key} required`);
  if (Object.hasOwn(options, '--build-manifest')) check(typeof options['--build-manifest'] === 'string' &&
    path.isAbsolute(options['--build-manifest']) && /^c:[\\/]/i.test(options['--build-manifest']), 'Absolute C-only --build-manifest required');
  if (Object.hasOwn(options, '--adapter-sources')) check(typeof options['--adapter-sources'] === 'string' &&
    path.isAbsolute(options['--adapter-sources']) && /^c:[\\/]/i.test(options['--adapter-sources']), 'Absolute C-only --adapter-sources required');
  check(!options['--build-manifest'] || options['--adapter-sources'], 'Candidate build requires explicit --adapter-sources');
  if (process.platform === 'win32') check(/^c:[\\/]/i.test(options['--output']), 'All artifacts must be on C drive');
  return options;
}

export async function comparisonMain({ engineFactory = null, extraOptions = [] } = {}) {
  const options = parseComparisonOptions(process.argv.slice(2), extraOptions);
  const load = async key => JSON.parse(await fs.readFile(options[key], 'utf8'));
  const planBytes = await fs.readFile(options['--plan']), plan = JSON.parse(planBytes), engine = engineFactory ? await engineFactory(options, plan) : null;
  (engine?.validatePlan ?? validatePlan)(plan);
  const cohort = await load('--cohort'), runtime = await load('--runtime-receipt'), ledger = await load('--ledger');
  let candidateBuild = null;
  if (options['--build-manifest']) {
    const bytes = await fs.readFile(options['--build-manifest']); candidateBuild = { manifest: JSON.parse(bytes), sha256: hash(bytes) };
  }
  runtimeSourceProof(runtime, candidateBuild); // Reject bad provenance before evidence, sockets or auth reservations.
  const adapterSources = options['--adapter-sources'] ? await loadAdapterSources(options['--adapter-sources'], runtime, options['--ws-module']) : null;
  adapterSourceFields(runtime, candidateBuild, adapterSources);
  check(cohort.planSha256 === hash(planBytes) && runtime.planSha256 === hash(planBytes), 'Plan file SHA differs from fixture/runtime pin');
  check(path.resolve(cohort.accountStore) === path.resolve(runtime.accountStore), 'Runtime account store differs from isolated fixture');
  check(path.dirname(options['--ledger']) === path.dirname(options['--cohort']) && ledger.purpose === 'comparable-local-only' && ledger.planSha256 === hash(planBytes), 'Dedicated local ledger/path required');
  const evidence = await fs.open(options['--output'] + '.events.jsonl', 'wx'); let queue = Promise.resolve(), writeFailure;
  const emit = row => { check(!writeFailure, 'Evidence writer failed'); const line = JSON.stringify(redact(row)) + '\n';
    queue = queue.then(() => evidence.write(line)).catch(e => { writeFailure = e; }); };
  const saveLedger = async value => { const next = options['--ledger'] + '.next'; await fs.writeFile(next, JSON.stringify(value), { flag: 'wx', mode: 0o600 }); await fs.rename(next, options['--ledger']); };
  const WebSocket = createRequire(import.meta.url)(options['--ws-module']);
  let report;
  try { report = await runComparison({ plan, cohort, runtime, url: endpoint(runtime.endpoint), WebSocket, emit, ledger, saveLedger, engine, candidateBuild, adapterSources }); }
  finally { await queue; await evidence.close(); }
  check(!writeFailure, 'Evidence write failed');
  report.planSha256 = hash(planBytes); report.eventsSha256 = hash(await fs.readFile(options['--output'] + '.events.jsonl'));
  await fs.writeFile(options['--output'] + '.report.json', JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ ok: report.ok, runCompleted: report.runCompleted, admitted: report.admitted, report: options['--output'] + '.report.json', qualificationLayer: 'game-core-local', capacityClaimAllowed: false }));
  process.exitCode = report.ok ? 0 : 1;
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) comparisonMain().catch(e => { console.error(JSON.stringify({ ok: false, error: e.message })); process.exitCode = 1; });
