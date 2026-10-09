// Ordinary public-protocol acceptance for an explicitly selected isolated realm.
// This is a two-socket functional check, not native visual or load acceptance.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { applyProtocolObservation, hasAuthoritativePlayerDeath } from './quest-agent/protocol-observation.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath } from './quest-agent/protocol-navigation.mjs';
import { createNavigator, equipStarterGear } from './quest-agent/protocol-play.mjs';
import { decodePlaytestFrame } from './playtest-catalog-transport.mjs';

const REPO_ROOT = path.resolve(import.meta.dirname, '../../../..');
// Same pinned Next bundled transport used by the existing capture scripts.
// Node's global WebSocket cannot portably set the required browser Origin.
const WebSocketTransport = createRequire(import.meta.url)('next/dist/compiled/ws');
const COMMANDS = new Set(['clientVersion', 'newAccount', 'login', 'newCharacter', 'startGame',
  'keepAlive', 'walk', 'run', 'turn', 'chat', 'attack', 'equipItem', 'useItem', 'logOut']);
const PRIVATE_KEY = /password|secret|token|authorization|cookie|assertion|private.?key/i;
const REQUIRED_CHECKS = ['authGuards', 'twoAccounts', 'mutualVisibility', 'movement', 'turn', 'chat',
  'chatLatency', 'sharedCombat', 'mapRoundTrip', 'logout', 'reloginPersistence'];
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const self = client => (client.snapshot?.entities ?? []).find(entity =>
  Number(entity.objectId) === Number(client.snapshot?.playerObjectId));
const distance = (a, b) => Math.max(Math.abs(Number(a.x) - Number(b.x)), Math.abs(Number(a.y) - Number(b.y)));
const position = client => ({ mapFileName: String(client.snapshot?.mapFileName ?? ''),
  x: Number(self(client)?.x), y: Number(self(client)?.y), direction: self(client)?.direction });
const packetAfter = (client, after, packet, predicate = () => true) => client.events.find(event =>
  event.sequence > after && event.direction === 'received' && event.packet === packet && predicate(event.payload));

export function observedPlayerId(client, name) {
  // The owner-facing protocol normalizes the self to 1000; observers receive
  // the shared Zone ID (50000+). Identity must be resolved in each receiver's
  // namespace rather than comparing two owners' normalized IDs.
  const entity = (client.snapshot?.entities ?? []).find(item => item.name === name &&
    ['player', 'remoteplayer', 'selfplayer'].includes(String(item.kind).toLowerCase()));
  return entity ? Number(entity.objectId) : null;
}

export function validateEndpoint(raw) {
  const url = new URL(raw);
  if (url.username || url.password || url.search || url.hash) throw new Error('Endpoint must not contain credentials, query parameters, or fragments');
  const loopback = ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname);
  if (url.protocol !== 'wss:' && !(url.protocol === 'ws:' && loopback)) {
    throw new Error('Remote gateways require WSS; plain WS is permitted only on loopback');
  }
  return url.toString();
}

export function endpointOrigin(raw) {
  const url = new URL(validateEndpoint(raw));
  url.protocol = url.protocol === 'wss:' ? 'https:' : 'http:';
  return url.origin;
}

export function redact(value, secrets = []) {
  if (Array.isArray(value)) return value.map(item => redact(item, secrets));
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, PRIVATE_KEY.test(key) ? '[redacted]' : redact(item, secrets)]));
  if (typeof value === 'string') {
    for (const secret of secrets) if (secret) value = value.split(secret).join('[redacted]');
    return value;
  }
  return value;
}

export function validateCredentials(value) {
  if (!Array.isArray(value) || value.length !== 2) throw new Error('Exactly two private account entries are required');
  const accounts = new Set(), names = new Set();
  for (const entry of value) {
    if (!/^[a-zA-Z0-9]{5,20}$/.test(entry.accountId ?? '') || /^demo$/i.test(entry.accountId)) throw new Error('Use distinct non-demo account IDs with 5–20 letters/digits');
    if (!/^[a-zA-Z][a-zA-Z0-9]{3,9}$/.test(entry.name ?? '')) throw new Error('Character names must contain 4–10 letters/digits');
    if (typeof entry.password !== 'string' || entry.password.length < 12 || entry.password.length > 20) throw new Error('Private passwords must contain 12–20 characters');
    accounts.add(entry.accountId.toLowerCase()); names.add(entry.name.toLowerCase());
  }
  if (accounts.size !== 2 || names.size !== 2) throw new Error('Both accounts and character names must be distinct');
  return value;
}

export function inventoryFingerprint(snapshot) {
  const items = key => (snapshot?.[key] ?? []).map(item => ({
    uniqueId: String(item.uniqueId), itemIndex: Number(item.itemIndex ?? item.index ?? 0),
    name: String(item.name ?? ''), count: Number(item.count ?? item.quantity ?? 1),
    slot: String(item.slot ?? item.equipSlot ?? ''),
  })).sort((left, right) => left.uniqueId.localeCompare(right.uniqueId));
  return { gold: Number(snapshot?.gold ?? 0), inventory: items('inventoryItems'), equipment: items('equipmentItems') };
}

export function isAuthRejection(event) {
  // The production gateway deliberately sanitizes its auth errors to this
  // public code. An overall pass also requires the same ordinary commands to
  // succeed after fresh registration/login, so an unsupported endpoint cannot
  // satisfy the complete authentication check with generic failures alone.
  return event?.type === 'error' && (event.code === 'commandRejected' ||
    /authenticat|log[ -]?in (?:is )?required|not logged in/i.test(String(event.message ?? event.error ?? '')));
}

export function exposesAuthenticatedGameplay(event) {
  if (['NewCharacterSuccess', 'UserInformation'].includes(event?.packet)) return true;
  if (event?.type !== 'worldSnapshot') return false;
  const snapshot = event.payload;
  // The ordinary pre-login shell contains public templates and no owner.
  // A delayed shell snapshot is not evidence of an authenticated StartGame.
  return snapshot?.playerObjectId != null && (snapshot?.entities ?? []).some(entity =>
    Number(entity.objectId) === Number(snapshot.playerObjectId));
}

export function chatMatches(payload, objectId, message) {
  return Number(payload?.objectId) === objectId && String(payload?.text ?? payload?.message ?? '').includes(message);
}

export function chatLatencyCheck(observerLatencyMs) {
  const maximumMs = 1000;
  return { status: Number.isFinite(observerLatencyMs) && observerLatencyMs >= 0 && observerLatencyMs <= maximumMs
    ? 'passed' : 'failed', observerLatencyMs, maximumMs,
  scope: 'two sockets measured by the same monotonic host clock; excludes human and visual latency' };
}

export function sharedHitEvidence(clients, cursors, targetId, attackerId) {
  const attackerIds = Array.isArray(attackerId) ? attackerId : clients.map(() => attackerId);
  const observed = clients.map((client, index) => {
    const struck = packetAfter(client, cursors[index], 'ObjectStruck', payload =>
      Number(payload?.objectId) === targetId && Number(payload?.attackerId) === attackerIds[index]);
    const health = packetAfter(client, cursors[index], 'ObjectHealth', payload =>
      Number(payload?.objectId) === targetId && Number(payload?.percent) < 100);
    const died = packetAfter(client, cursors[index], 'ObjectDied', payload => Number(payload?.objectId) === targetId);
    const damage = packetAfter(client, cursors[index], 'DamageIndicator', payload =>
      Number(payload?.objectId) === targetId && Number(payload?.damage) > 0);
    return { struck, health, died, damage };
  });
  const bothStruck = observed.every(item => item.struck);
  const equalHealth = observed.every(item => item.health) &&
    Number(observed[0].health.payload.percent) === Number(observed[1].health.payload.percent);
  const bothDied = observed.every(item => item.died);
  if (!bothStruck || !(equalHealth || bothDied) || !observed.some(item => item.damage)) return null;
  return { targetId, attackerIds, healthPercent: equalHealth ? Number(observed[0].health.payload.percent) : 0,
    died: Boolean(bothDied), receipts: observed.map(item => Object.fromEntries(Object.entries(item)
      .filter(([, event]) => event).map(([key, event]) => [key, event.sequence]))) };
}

export function finishReport(report) {
  report.ok = !report.error && !report.cleanupErrors?.length && REQUIRED_CHECKS.every(key => report.checks?.[key]?.status === 'passed');
  report.status = report.ok ? 'passed' : 'incomplete';
  report.finishedAt = new Date().toISOString();
  report.visualAccepted = false;
  report.twoPhysicalComputersTested = false;
  report.loadAccepted = false;
  return report;
}

export class PlaytestClient {
  constructor(url, label, tracePath, secrets = []) {
    this.url = validateEndpoint(url); this.label = label; this.tracePath = tracePath; this.secrets = [...secrets];
    this.events = []; this.sequence = 0; this.snapshot = null; this.writeQueue = Promise.resolve();
    this.lastWalkAt = 0; this.inGame = false; this.closed = false; this.catalogGzipOptedIn = false;
  }
  record(direction, value) {
    // Tokens are never reused: reconnect acceptance deliberately logs in again.
    const rememberSecrets = item => {
      if (!item || typeof item !== 'object') return;
      for (const [key, nested] of Object.entries(item)) {
        if (PRIVATE_KEY.test(key) && typeof nested === 'string' && nested && !this.secrets.includes(nested)) this.secrets.push(nested);
        else rememberSecrets(nested);
      }
    };
    rememberSecrets(value);
    const safe = redact(value, this.secrets);
    const event = { ...safe, movementDirection: safe.direction, direction, sequence: ++this.sequence,
      at: new Date().toISOString(), monotonicMs: performance.now() };
    this.events.push(event);
    if (this.events.length > 15000) this.events.splice(0, 1000);
    // Retain all public packet receipts; summarize bulky refresh snapshots.
    const trace = event.type === 'worldSnapshot' ? { ...event, payload: {
      ...position(this), playerObjectId: this.snapshot?.playerObjectId,
      inventory: inventoryFingerprint(this.snapshot), entityCount: this.snapshot?.entities?.length,
    } } : event;
    this.writeQueue = this.writeQueue.then(() => fs.appendFile(this.tracePath, JSON.stringify(trace) + '\n'));
    this.writeQueue.catch(error => { this.failure = error; });
    return event;
  }
  observe(message) {
    this.snapshot = applyProtocolObservation(this.snapshot, message);
    if (message?.packet === 'UserInformation' ||
        (message?.type === 'worldSnapshot' && exposesAuthenticatedGameplay(message))) this.inGame = true;
    if (message?.packet === 'LogOutSuccess') this.inGame = false;
    this.record('received', message);
  }
  async connect() {
    this.ws = new WebSocketTransport(this.url, { origin: endpointOrigin(this.url), handshakeTimeout: 10000 });
    this.ws.addEventListener('message', event => {
      if (this.failure) return;
      try {
        // Validate the whole compressed batch before applying its first entry.
        for (const message of decodePlaytestFrame(event.data, this.catalogGzipOptedIn)) this.observe(message);
      } catch (error) {
        this.failure = error;
        this.ws.close(1002, 'Invalid gateway frame');
      }
    });
    this.ws.addEventListener('error', () => { this.failure = new Error('WebSocket connection failed'); });
    this.ws.addEventListener('close', () => { this.closed = true; });
    await this.wait(() => this.ws.readyState === WebSocketTransport.OPEN, 'WebSocket open');
    await this.request({ type: 'clientVersion' }, 'ClientVersion');
    await this.wait(() => this.events.some(event => event.packet === 'Connected'), 'Connected handshake');
    this.timer = setInterval(() => {
      if (this.ws.readyState === WebSocketTransport.OPEN) this.send({ type: 'keepAlive', time: Date.now() });
    }, 2000);
  }
  send(command) {
    if (!COMMANDS.has(command.type)) throw new Error(`Not an allowed ordinary playtest command: ${command.type}`);
    this.record('sent', command);
    this.ws.send(JSON.stringify(command));
  }
  async wait(predicate, label, timeoutMs = 20000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (this.failure) throw this.failure;
      const result = predicate();
      if (result) return result;
      if (this.closed) throw new Error(`${this.label} disconnected during ${label}`);
      await sleep(25);
    }
    throw new Error(`${this.label} timed out waiting for ${label}`);
  }
  async request(command, packet, timeoutMs = 20000) {
    const after = this.sequence;
    this.send(command);
    return this.wait(() => {
      const rejection = this.events.find(event => event.sequence > after && event.direction === 'received' && event.type === 'error');
      if (rejection) throw new Error(`Gateway rejected ${command.type}: ${rejection.message ?? 'unknown error'}`);
      return packetAfter(this, after, packet);
    }, packet, timeoutMs);
  }
  async refresh() {
    const after = this.sequence;
    this.send({ type: 'clientVersion' });
    await this.wait(() => this.events.some(event => event.sequence > after && event.type === 'worldSnapshot'), 'fresh public snapshot');
    return this.snapshot;
  }
  async logout() {
    if (!this.inGame) return null;
    const ack = await this.request({ type: 'logOut' }, 'LogOutSuccess');
    this.inGame = false;
    return ack;
  }
  async close() {
    clearInterval(this.timer);
    let logoutError;
    try { if (this.ws?.readyState === 1 && this.inGame) await this.logout(); }
    catch (error) { logoutError = error; }
    finally { this.ws?.close(); }
    // Relogin starts only after a completed close handshake; a stale route
    // lease after this point cannot be blamed on a still-open test socket.
    const deadline = Date.now() + 5000;
    while (this.ws && this.ws.readyState !== WebSocketTransport.CLOSED && Date.now() < deadline) await sleep(25);
    if (this.ws && this.ws.readyState !== WebSocketTransport.CLOSED) {
      this.ws.terminate();
      logoutError ??= new Error(`${this.label} WebSocket close was not acknowledged within 5 seconds`);
    }
    await this.writeQueue;
    if (logoutError) throw logoutError;
  }
}

export async function usingClient(client, action) {
  let failure;
  try { return await action(client); }
  catch (error) { failure = error; throw error; }
  finally {
    try { await client.close(); }
    catch (error) {
      if (failure) failure.cleanupError = error.message;
      else throw error;
    }
  }
}

export async function authenticate(client, credentials, create) {
  if (create) {
    const registration = await client.request({ type: 'newAccount', accountId: credentials.accountId, password: credentials.password }, 'NewAccount');
    if (registration.payload?.result !== 8) throw new Error(`Fresh registration rejected (${registration.payload?.result}); no existing account fallback`);
    await sleep(250);
  }
  const login = await client.request({ type: 'login', accountId: credentials.accountId, password: credentials.password }, 'LoginSuccess');
  let character = login.payload?.characters?.find(item => item.name === credentials.name);
  if (create) {
    if (login.payload?.characters?.length) throw new Error('Freshly registered account unexpectedly contains characters');
    const created = await client.request({ type: 'newCharacter', name: credentials.name, class: 'Warrior', gender: 'Male' }, 'NewCharacterSuccess');
    character = created.payload?.character;
  }
  if (!Number.isInteger(character?.index)) throw new Error('Expected ordinary character is absent; no replacement character created');
  const after = client.sequence;
  client.snapshot = null;
  client.send({ type: 'startGame', characterIndex: character.index });
  await client.wait(() => {
    const rejection = client.events.find(event => event.sequence > after && event.direction === 'received' && event.type === 'error');
    if (rejection) throw new Error(`Gateway rejected startGame: ${rejection.message ?? 'unknown error'}`);
    return self(client)?.name === credentials.name && client.snapshot?.mapFileName &&
      client.events.some(event => event.sequence > after && event.type === 'worldSnapshot');
  }, 'StartGame snapshot');
  return { name: character.name, characterIndex: character.index, objectId: self(client).objectId, ...position(client) };
}

async function authenticationGuards(makeClient) {
  const receipts = [];
  for (const command of [{ type: 'startGame', characterIndex: 0 },
    { type: 'newCharacter', name: `G${crypto.randomBytes(4).toString('hex')}`, class: 'Warrior', gender: 'Male' }]) {
    const client = makeClient(`guard-${command.type}`);
    await usingClient(client, async () => {
      await client.connect();
      const after = client.sequence;
      client.send(command);
      const rejection = await client.wait(() => {
        const events = client.events.filter(event => event.sequence > after && event.direction === 'received');
        if (events.some(exposesAuthenticatedGameplay)) {
          throw new Error(`Unauthenticated ${command.type} exposed gameplay state`);
        }
        return events.find(isAuthRejection);
      }, `unauthenticated ${command.type} rejection`, 5000);
      receipts.push({ command: command.type, receipt: rejection.sequence, code: rejection.code, message: rejection.message });
    });
  }
  return receipts;
}

function navigationOptions(deadline, extra = {}) {
  return { maxSuccessfulSteps: 500, maxAttempts: 500, maxNoPathRefreshes: 2,
    beforeMovement() { if (Date.now() >= deadline) throw new Error('Bounded playtest travel deadline expired'); }, ...extra };
}

async function moveAside(client, loadMap, deadline) {
  const map = await loadMap(client.snapshot.mapFileName);
  const from = { ...self(client) };
  const occupied = client.snapshot.entities.filter(item => item.objectId !== from.objectId && !item.dead && Number.isFinite(item.x) && Number.isFinite(item.y));
  for (const [dx, dy] of [[3, 0], [-3, 0], [0, 3], [0, -3], [2, 2], [-2, -2]]) {
    const target = { x: from.x + dx, y: from.y + dy };
    const route = findProtocolWalkPath({ map, start: from, target, dynamicObstacles: occupied, maxExpanded: 1500 });
    if (!route || route.length > 12) continue;
    await createNavigator(client, { loadCollisionMap: loadMap, movementResponseTimeoutMs: 5000 })(
      target, 0, () => false, navigationOptions(deadline, { maxSuccessfulSteps: 15, maxAttempts: 20 }));
    if (distance(from, self(client)) > 0) return { from: { x: from.x, y: from.y }, to: position(client) };
  }
  throw new Error('No ordinary unoccupied parking route near actual spawn');
}

async function socialChecks(clients, report, loadMap, deadline) {
  const [a, b] = clients;
  const names = clients.map(client => self(client).name);
  await Promise.all(clients.map((client, index) => client.wait(() => packetAfter(client, 0, 'ObjectPlayer',
    payload => payload?.name === names[1 - index]), 'other player ObjectPlayer')));
  const ids = [observedPlayerId(b, names[0]), observedPlayerId(a, names[1])];
  report.checks.mutualVisibility = { status: 'passed', names, observerObjectIds: ids };
  const movementAfter = b.sequence;
  const from = position(a);
  await moveAside(a, loadMap, deadline);
  const to = position(a);
  const movement = await b.wait(() => ['ObjectWalk', 'ObjectRun'].map(packet => packetAfter(b, movementAfter, packet,
    payload => Number(payload?.objectId) === ids[0] && Number(payload?.location?.x ?? payload?.x) === to.x &&
      Number(payload?.location?.y ?? payload?.y) === to.y)).find(Boolean), 'observer movement matching owner');
  report.checks.movement = { status: 'passed', from, to, observerReceipt: movement.sequence };
  await sleep(650);
  const turnAfter = b.sequence;
  const direction = self(a).direction === 'Up' ? 'Right' : 'Up';
  await a.request({ type: 'turn', direction }, 'UserLocation');
  const turn = await b.wait(() => packetAfter(b, turnAfter, 'ObjectTurn', payload =>
    Number(payload?.objectId) === ids[0] && payload?.direction === direction), 'observer turn');
  report.checks.turn = { status: 'passed', direction, observerReceipt: turn.sequence };
  const chatAfter = a.sequence, text = `playtest ${crypto.randomBytes(5).toString('hex')}`, chatSentAt = performance.now();
  b.send({ type: 'chat', message: text });
  const chat = await a.wait(() => packetAfter(a, chatAfter, 'ObjectChat', payload =>
    chatMatches(payload, ids[1], text)), 'observer chat');
  report.checks.chat = { status: 'passed', observerReceipt: chat.sequence, observerLatencyMs: Math.round(chat.monotonicMs - chatSentAt) };
  report.checks.chatLatency = chatLatencyCheck(report.checks.chat.observerLatencyMs);
}

async function sharedCombat(clients, loadMap, deadline) {
  const names = new Set(['Scarecrow', 'Hen', 'Deer', 'HookingCat', 'RakingCat']);
  const [a, b] = clients;
  for (const client of clients) await equipStarterGear(client);
  const navigation = clients.map(client => createNavigator(client, { loadCollisionMap: loadMap, movementResponseTimeoutMs: 5000 }));
  const candidates = a.snapshot.entities.filter(entity => entity.kind === 'monster' && names.has(entity.name) &&
    !entity.dead && Number(entity.hp ?? 1) > 0 && distance(self(a), entity) <= 18)
    .sort((left, right) => distance(self(a), left) - distance(self(a), right));
  const attempts = [];
  for (const candidate of candidates.slice(0, 4)) {
    if (Date.now() >= deadline) break;
    const targetId = Number(candidate.objectId);
    const evidence = [];
    try {
      for (let index = 0; index < 2; index++) {
        let target = clients[index].snapshot.entities.find(entity => Number(entity.objectId) === targetId);
        if (!target || target.dead) break;
        await navigation[index](target, 1, () => Date.now() >= deadline,
          navigationOptions(deadline, { maxSuccessfulSteps: 45, maxAttempts: 60 }));
      }
      for (let attack = 0; attack < 12 && Date.now() < deadline; attack++) {
        const index = attack % 2, client = clients[index];
        const target = client.snapshot.entities.find(entity => Number(entity.objectId) === targetId);
        if (!target || target.dead || hasAuthoritativePlayerDeath(client.snapshot)) break;
        await navigation[index](target, 1, () => Date.now() >= deadline,
          navigationOptions(deadline, { maxSuccessfulSteps: 8, maxAttempts: 12 }));
        await sleep(650);
        if (distance(self(client), target) > 1) continue;
        const cursors = clients.map(item => item.sequence), attackerName = self(client).name;
        const attackerIds = clients.map(item => observedPlayerId(item, attackerName));
        client.send({ type: 'attack', objectId: targetId });
        try {
          const hit = await client.wait(() => sharedHitEvidence(clients, cursors, targetId, attackerIds), 'shared positive hit', 2500);
          evidence.push({ ...hit, attackerName });
          if (new Set(evidence.map(item => item.attackerName)).size === 2) return { targetId, target: candidate.name, hits: evidence };
        } catch (error) { attempts.push({ targetId, attackerName, error: error.message }); }
      }
    } catch (error) { attempts.push({ targetId, error: error.message }); }
  }
  const error = new Error('Both actors did not prove positive hits and matching public health on the same ordinary monster within the bounded search');
  error.evidence = attempts;
  throw error;
}

function transferPoint(transfer, origin) {
  const bounds = transfer?.bounds;
  if (![bounds?.minX, bounds?.minY, bounds?.maxX, bounds?.maxY].every(Number.isInteger)) return null;
  return { x: Math.max(bounds.minX, Math.min(bounds.maxX, origin.x)), y: Math.max(bounds.minY, Math.min(bounds.maxY, origin.y)) };
}

async function chooseTransfer(client, loadMap, wantedMap) {
  await client.refresh();
  const map = await loadMap(client.snapshot.mapFileName), origin = self(client);
  const choices = [];
  for (const transfer of client.snapshot.mapTransfers ?? []) {
    if (!transfer.toMapFileName || String(transfer.toMapFileName) === String(client.snapshot.mapFileName) ||
      (wantedMap && String(transfer.toMapFileName) !== wantedMap)) continue;
    const target = transferPoint(transfer, origin);
    if (!target) continue;
    // Only a currently advertised source doorway can override a source wall.
    const overrides = [];
    const bounds = transfer.bounds;
    if ((bounds.maxX - bounds.minX + 1) * (bounds.maxY - bounds.minY + 1) > 64) continue;
    for (let y = bounds.minY; y <= bounds.maxY; y++) for (let x = bounds.minX; x <= bounds.maxX; x++) overrides.push({ x, y });
    const route = findProtocolWalkPath({ map, start: origin, target, staticWalkableOverrides: overrides, maxExpanded: 100000 });
    if (route && route.length <= 500) choices.push({ transfer, target, steps: route.length - 1 });
  }
  choices.sort((left, right) => left.steps - right.steps);
  if (!choices.length) throw new Error('No advertised ordinary doorway reachable within the 500-step bound');
  return choices[0];
}

async function cross(client, choice, loadMap, deadline) {
  const from = position(client), after = client.sequence;
  const navigator = createNavigator(client, { loadCollisionMap: loadMap, movementResponseTimeoutMs: 8000 });
  await navigator(choice.target, 0, () => client.snapshot.mapFileName !== from.mapFileName,
    navigationOptions(deadline, { liveTransferKey: choice.transfer.key }));
  await client.wait(() => String(client.snapshot.mapFileName) === String(choice.transfer.toMapFileName) &&
    packetAfter(client, after, 'MapInformation'), 'ordinary doorway MapInformation', 8000);
  await client.refresh();
  return { from, to: position(client), transferKey: choice.transfer.key, packetSequence: packetAfter(client, after, 'MapInformation').sequence };
}

async function mapRoundTrip(clients, loadMap, deadline) {
  const originalMap = String(clients[0].snapshot.mapFileName);
  const selected = await chooseTransfer(clients[0], loadMap);
  const receipts = [];
  for (let index = 0; index < clients.length; index++) {
    const client = clients[index];
    const choice = index === 0 ? selected : await chooseTransfer(client, loadMap, String(selected.transfer.toMapFileName));
    receipts.push({ actor: client.label, outbound: await cross(client, choice, loadMap, deadline) });
    await moveAside(client, loadMap, deadline);
  }
  for (const client of clients) {
    const other = clients.find(item => item !== client);
    await client.wait(() => observedPlayerId(client, self(other).name) != null, 'mutual visibility in destination');
  }
  for (let index = 0; index < clients.length; index++) {
    const client = clients[index];
    receipts[index].return = await cross(client, await chooseTransfer(client, loadMap, originalMap), loadMap, deadline);
    await moveAside(client, loadMap, deadline);
  }
  return receipts;
}

export function outsideRepository(value) {
  const absolute = path.resolve(value), relative = path.relative(REPO_ROOT, absolute);
  if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) {
    throw new Error('Private credentials must be outside the repository');
  }
  return absolute;
}

async function privateCredentials(directory, runId) {
  const root = outsideRepository(directory);
  const privateDir = path.join(root, runId);
  await fs.mkdir(privateDir, { recursive: true, mode: 0o700 });
  if (process.platform === 'win32') {
    const owner = execFileSync('whoami.exe', [], { encoding: 'utf8', windowsHide: true }).trim();
    execFileSync('icacls.exe', [privateDir, '/inheritance:r', '/grant:r', `${owner}:(OI)(CI)F`], { stdio: 'pipe', windowsHide: true });
  } else await fs.chmod(privateDir, 0o700);
  const suffix = crypto.randomBytes(4).toString('hex');
  const credentials = validateCredentials(['a', 'b'].map(label => ({ accountId: `pt${label}${suffix}`,
    name: `${label.toUpperCase()}${suffix}`, password: crypto.randomBytes(10).toString('hex') })));
  const credentialPath = path.join(privateDir, 'accounts.json');
  await fs.writeFile(credentialPath, JSON.stringify(credentials, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  return { credentials, credentialPath };
}

export async function resumeCredentials(reportPath, endpoint) {
  const sourcePath = path.resolve(reportPath);
  const previous = JSON.parse(await fs.readFile(sourcePath, 'utf8'));
  if (previous.schemaVersion !== 1 || validateEndpoint(previous.endpoint) !== endpoint ||
      previous.checks?.twoAccounts?.status !== 'passed' || !previous.privateCredentialsPath) {
    throw new Error('Resume needs a matching-realm report with two successfully bootstrapped ordinary accounts');
  }
  const credentialPath = outsideRepository(previous.privateCredentialsPath);
  const credentials = validateCredentials(JSON.parse(await fs.readFile(credentialPath, 'utf8')));
  const expected = previous.accounts ?? [];
  if (credentials.some((entry, index) => entry.accountId !== expected[index]?.accountId || entry.name !== expected[index]?.name)) {
    throw new Error('Private account identities do not match the source report');
  }
  return { credentials, credentialPath, resumedFrom: { reportPath: sourcePath, runId: previous.runId,
    registrationMode: 'existing-isolated-accounts-from-recorded-prior-run', sourceStartedAt: previous.startedAt } };
}

export async function runPlaytest(options) {
  const endpoint = validateEndpoint(options.endpoint);
  if (options.realm !== 'isolated') throw new Error('--realm isolated is required; never target the existing public realm');
  if (!options.output || (!options.privateDir && !options.resumeReport)) throw new Error('--output and --private-dir (or --resume-report) are required');
  const durationSeconds = Number(options.durationSeconds ?? 600);
  if (!Number.isInteger(durationSeconds) || durationSeconds < 60 || durationSeconds > 1800) throw new Error('Duration must be 60–1800 seconds');
  const output = path.resolve(options.output), runId = `${Date.now()}-${crypto.randomBytes(4).toString('hex')}`;
  await fs.mkdir(output, { recursive: true });
  const { credentials, credentialPath, resumedFrom } = options.resumeReport
    ? await resumeCredentials(options.resumeReport, endpoint)
    : await privateCredentials(options.privateDir, runId);
  const secrets = credentials.map(entry => entry.password);
  const makeClient = label => new PlaytestClient(endpoint, label, path.join(output, `${runId}-${label}.trace.jsonl`), secrets);
  const report = { schemaVersion: 1, runId, endpoint, startedAt: new Date().toISOString(),
    scope: 'ordinary-public-protocol-two-independent-sockets', privateCredentialsPath: credentialPath,
    checks: {}, accounts: credentials.map(({ accountId, name }) => ({ accountId, name })) };
  report.registrationMode = resumedFrom ? 'recorded-prior-run-accounts' : 'two-fresh-ordinary-registrations';
  if (resumedFrom) report.resumedFrom = resumedFrom;
  const clients = [], saved = [], deadline = Date.now() + durationSeconds * 1000;
  const mapCache = new Map();
  const loadMap = async name => {
    if (!mapCache.has(name)) mapCache.set(name, await loadProtocolCollisionMap(name, options.mapRoot ? { mapRoot: options.mapRoot } : {}));
    return mapCache.get(name);
  };
  const checkpoint = () => fs.writeFile(path.join(output, `${runId}.report.json`), JSON.stringify(redact(report, secrets), null, 2) + '\n');
  const step = async (key, action) => {
    try { report.checks[key] = { status: 'passed', evidence: await action() }; }
    catch (error) { report.checks[key] = { status: 'failed', error: error.message, evidence: error.evidence,
      cleanupError: error.cleanupError }; }
    await checkpoint();
    options.onProgress?.({ check: key, status: report.checks[key].status });
    return report.checks[key].status === 'passed';
  };
  try {
    if (!await step('authGuards', () => authenticationGuards(makeClient))) throw new Error('Authentication guard failed; account creation stopped');
    const actors = [];
    for (let index = 0; index < 2; index++) {
      const client = makeClient(index ? 'b' : 'a'); clients.push(client);
      await client.connect();
      actors.push(await authenticate(client, credentials[index], !resumedFrom));
      if (index === 0) await moveAside(client, loadMap, deadline);
    }
    report.checks.twoAccounts = { status: 'passed', evidence: actors };
    await socialChecks(clients, report, loadMap, deadline);
    await checkpoint();
    await step('sharedCombat', () => sharedCombat(clients, loadMap, Math.min(deadline, Date.now() + 90000)));
    await step('mapRoundTrip', () => mapRoundTrip(clients, loadMap, deadline));
    await step('logout', async () => {
      const receipts = [];
      for (const client of clients) {
        await client.refresh();
        saved.push({ transform: position(client), inventory: inventoryFingerprint(client.snapshot) });
        receipts.push({ actor: client.label, sequence: (await client.logout()).sequence });
        await client.close();
      }
      return receipts;
    });
    if (report.checks.logout?.status === 'passed') await step('reloginPersistence', async () => {
      const evidence = [];
      // Verify sequentially and log out each actor before the next joins. This
      // avoids an occupied login tile being mistaken for a persistence defect.
      for (let index = 0; index < 2; index++) {
        const client = makeClient(`relogin-${index}`);
        await usingClient(client, async () => {
          await client.connect();
          await authenticate(client, credentials[index], false);
          const actual = { transform: position(client), inventory: inventoryFingerprint(client.snapshot) };
          if (JSON.stringify(actual) !== JSON.stringify(saved[index])) {
            const error = new Error('Ordinary relogin transform or carried item identity/count/gold differs from pre-logout snapshot');
            error.evidence = { actor: index, expected: saved[index], actual }; throw error;
          }
          const ack = await client.logout();
          evidence.push({ actor: index, saved: actual, logoutReceipt: ack.sequence });
        });
      }
      return evidence;
    });
  } catch (error) { report.error = error.message; }
  finally {
    for (const client of clients) {
      try { await client.close(); }
      catch (error) { (report.cleanupErrors ??= []).push(error.message); }
    }
    for (const key of REQUIRED_CHECKS) {
      report.checks[key] ??= { status: 'skipped', reason: 'An earlier prerequisite did not complete' };
    }
    finishReport(report);
    if (report.cleanupErrors?.length) { report.ok = false; report.status = 'incomplete'; }
    await checkpoint();
  }
  return { ...redact(report, secrets), reportPath: path.join(output, `${runId}.report.json`) };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const options = {};
  const keys = { '--endpoint': 'endpoint', '--output': 'output', '--private-dir': 'privateDir',
    '--realm': 'realm', '--duration-seconds': 'durationSeconds', '--map-root': 'mapRoot', '--resume-report': 'resumeReport' };
  try {
    if (process.argv.includes('--help')) {
      console.log('node playtest-multiplayer-smoke.mjs --endpoint wss://TEST-REALM/ws --realm isolated --output REPORT_DIR --private-dir PRIVATE_DIR [--duration-seconds 600] [--map-root MAP_DIR] [--resume-report PRIOR_REPORT]');
    } else {
      for (let index = 2; index < process.argv.length; index += 2) {
        const key = keys[process.argv[index]];
        if (!key || !process.argv[index + 1]) throw new Error('Unknown or missing command-line option; use --help (credentials are never command-line arguments)');
        options[key] = process.argv[index + 1];
      }
      const result = await runPlaytest({ ...options, onProgress: status => console.log(JSON.stringify(status)) });
      console.log(JSON.stringify({ ok: result.ok, reportPath: result.reportPath, checks: result.checks }, null, 2));
      if (!result.ok) process.exitCode = 1;
    }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
