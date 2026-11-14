// Public signed guest flow plus a bounded ordinary-player functional probe.
// This never reads a signing key, sends QA commands, edits saves, or certifies CCU.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';
import { PlaytestClient, outsideRepository, validateEndpoint, observedPlayerId, redact,
  inventoryFingerprint } from '../../../apps/web/scripts/playtest-multiplayer-smoke.mjs';
import { decodePlaytestFrame, CATALOG_GZIP_CAPABILITY } from '../../../apps/web/scripts/playtest-catalog-transport.mjs';
import { LoginBudget, LOGIN_WINDOW_MS, CapacityMonitor, BoundedEvidence, verifiedCombatHit,
  nativeCadence } from '../../../apps/web/scripts/playtest-capacity-soak-core.mjs';
import { CommandBudget } from '../../../apps/web/scripts/playtest-load-smoke.mjs';
import { createNavigator, equipStarterGear } from '../../../apps/web/scripts/quest-agent/protocol-play.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath } from '../../../apps/web/scripts/quest-agent/protocol-navigation.mjs';
import { exchangeSuiWallet, migrateGuestPool, PublicAuthFetchTimeout } from './public_sui_wallet_auth.mjs';
import { selfActionBlockMask } from '../../../apps/web/scripts/quest-agent/protocol-status.mjs';

const WebSocket = createRequire(new URL('../../../apps/web/package.json', import.meta.url))('next/dist/compiled/ws');
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const self = c => c.snapshot?.entities?.find(e => e.objectId === c.snapshot.playerObjectId);
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const at = () => new Date().toISOString();
const exactFields = (value, names) => Object.keys(value).sort().join(',') === [...names].sort().join(',');
const check = (condition, message) => { if (!condition) throw new Error(message); };
const nameHash = name => crypto.createHash('sha256').update(String(name ?? '')).digest('hex');
export function bootstrapEvidence(client, cursor, expectedName) {
  const fresh = packet => client.events.find(e => e.direction === 'received' && e.sequence > cursor && e.packet === packet);
  const start = fresh('StartGame'), user = fresh('UserInformation'), map = fresh('MapInformation');
  const snapshot = client.events.find(e => e.sequence > Math.max(cursor, user?.sequence ?? cursor, map?.sequence ?? cursor) &&
    e.direction === 'received' && e.type === 'worldSnapshot' && e.ownerNameMatches === true);
  const owner = self(client), ui = user?.payload, location = ui?.location ?? ui;
  const correction = client.events.filter(e => e.direction === 'received' && e.packet === 'UserLocation' &&
    e.sequence > (user?.sequence ?? Infinity) && e.sequence < (snapshot?.sequence ?? -1)).at(-1);
  const corrected = correction?.payload?.location ?? correction?.payload;
  const samePosition = owner?.x === location?.x && owner?.y === location?.y;
  const zoneAdjusted = !samePosition && Boolean(correction && owner) &&
    Number.isInteger(corrected?.x) && Number.isInteger(corrected?.y) &&
    corrected.x === owner.x && corrected.y === owner.y;
  const ready = start?.payload?.result === 4 && user?.sequence > start.sequence && map?.sequence > start.sequence &&
    ui?.name === expectedName && Number.isSafeInteger(ui.objectId) && ui.objectId > 0 && ui.hp > 0 &&
    Number.isInteger(location?.x) && location.x >= 0 && Number.isInteger(location?.y) && location.y >= 0 && snapshot?.sequence > Math.max(user.sequence, map.sequence) &&
    owner?.name === expectedName && owner.dead !== true && client.snapshot?.playerHp > 0 &&
    client.snapshot.playerObjectId === ui.objectId && Number.isInteger(owner.x) && owner.x >= 0 && Number.isInteger(owner.y) && owner.y >= 0 &&
    (samePosition || zoneAdjusted) && String(client.snapshot.mapFileName) === String(map?.payload?.fileName);
  const receipts=[start,user,map,snapshot,...(zoneAdjusted?[correction]:[])];
  const monotonicMs=ready&&receipts.every(e=>Number.isFinite(e?.monotonicMs))?Math.max(...receipts.map(e=>e.monotonicMs)):null;
  return { ready: Boolean(ready), monotonicMs, startSequence: start?.sequence ?? 0, userSequence: user?.sequence ?? 0,
    mapSequence: map?.sequence ?? 0, snapshotSequence: snapshot?.sequence ?? 0,
    uiNameMatches: ui?.name === expectedName, uiNameHash: nameHash(ui?.name), expectedNameHash: nameHash(expectedName),
    uiObjectId: ui?.objectId ?? 0, snapshotOwnerId: client.snapshot?.playerObjectId ?? 0,
    ownerNameMatches: owner?.name === expectedName, ownerNameHash: nameHash(owner?.name),
    hp: client.snapshot?.playerHp ?? 0, x: owner?.x ?? -1, y: owner?.y ?? -1,
    uiX: location?.x ?? -1, uiY: location?.y ?? -1, authoritativePositionAdjusted: Boolean(zoneAdjusted),
    positionCorrectionSequence: zoneAdjusted ? correction.sequence : 0,
    positionDeltaX: Number.isInteger(owner?.x) && Number.isInteger(location?.x) ? owner.x-location.x : 0,
    positionDeltaY: Number.isInteger(owner?.y) && Number.isInteger(location?.y) ? owner.y-location.y : 0 };
}
export function preparationAttackReadiness({ lastMovementAckAt, movementKind = 'walk', poison = 0, lastAttackAt, attackIntervalMs = 1600 }) {
  const movementCooldownMs = (movementKind === 'turn' ? 350 : 600) * (Number(poison) & 4 ? 2 : 1);
  return { movementCooldownMs, readyAtMs: Math.max(Number.isFinite(lastMovementAckAt) ? lastMovementAckAt + movementCooldownMs + 50 : -Infinity,
    Number.isFinite(lastAttackAt) ? lastAttackAt + attackIntervalMs : -Infinity) };
}
export function parseActorOrdinals(value, count) {
  if (value === undefined) return undefined;
  const parts=String(value).split(',');
  check(parts.every(v=>/^\d+$/.test(v)),'Actor ordinals must be comma-separated nonnegative integer indices');
  const ordinals=parts.map(Number);
  check(ordinals.length===count&&ordinals.every(Number.isSafeInteger)&&new Set(ordinals).size===ordinals.length,
    'Actor ordinals must be unique and match --actors');
  return ordinals;
}
export function selectProbeActors(pool, count, ordinals) {
  check(Array.isArray(pool.actors),'Dedicated private cohort actors are missing');
  if(ordinals===undefined)check(pool.actors.length===count,'Larger existing pools require explicit --actor-ordinals');
  const indices=ordinals===undefined?pool.actors.map((_,i)=>i):parseActorOrdinals(ordinals.join(','),count);
  return indices.map((originalOrdinal,localOrdinal)=>{
    const actor=pool.actors[originalOrdinal];
    check(actor&&actor.ordinal===originalOrdinal,'Selected original actor index is missing or inconsistent');
    if(ordinals!==undefined)check(actor.walletIdentity?.kind==='synthetic-ed25519-wallet'&&
      typeof actor.walletIdentity.secretKey==='string'&&actor.walletIdentity.secretKey.length>0&&
      typeof actor.walletIdentity.address==='string'&&actor.walletIdentity.address.length>0&&
      typeof actor.walletIdentity.accountId==='string'&&actor.walletIdentity.accountId.length>0&&Number.isSafeInteger(actor.walletIdentity.characterIndex)&&actor.walletIdentity.characterIndex>=0,
      'Explicit actor selection requires an existing dedicated wallet and character');
    return {localOrdinal,originalOrdinal,actor};
  });
}

export function probeTransportForActor(profile = 'native-gzip', originalOrdinal) {
  check(['native-gzip', 'plain-Web', 'mixed'].includes(profile), 'Transport profile must be native-gzip, plain-Web or mixed');
  check(Number.isSafeInteger(originalOrdinal) && originalOrdinal >= 0, 'Original actor ordinal must be a nonnegative integer');
  return profile === 'mixed' ? originalOrdinal % 2 === 0 ? 'plain-Web' : 'native-gzip' : profile;
}

export function optionsFrom(args) {
  const options = {};
  const allowed = new Set(['web-base', 'endpoint', 'private-directory', 'output', 'monitor', 'actors', 'seconds', 'combat', 'wall-seconds', 'login-not-before', 'map-root', 'auth', 'actor-ordinals', 'equipment-roundtrip', 'transport-profile']);
  for (let i = 0; i < args.length; i += 2) {
    check(args[i]?.startsWith('--') && args[i + 1] !== undefined, 'Use --name value pairs');
    const key = args[i].slice(2); check(allowed.has(key) && options[key] === undefined, 'Unknown/repeated option'); options[key] = args[i + 1];
  }
  options['web-base'] ??= 'https://numeron.obelisk.build';
  options.endpoint ??= 'wss://165.154.65.136.sslip.io/playtest/ws';
  const web = new URL(options['web-base']);
  check(web.protocol === 'https:' && !web.username && !web.password && !web.search && !web.hash && web.pathname === '/', 'Use an explicit HTTPS Web origin');
  options.webBase = web.origin; options.endpoint = validateEndpoint(options.endpoint);
  options.count = Number(options.actors ?? 3); options.seconds = Number(options.seconds ?? 60);
  options.wallSeconds = Number(options['wall-seconds'] ?? 600); options.combat ??= 'required';
  options.auth ??= 'sui-wallet'; check(options.auth === 'sui-wallet', 'Use the official public sui-wallet authentication flow');
  check(Number.isInteger(options.count) && options.count >= 2 && options.count <= 3, 'Initial functional cohort is bounded to 2..3 actors');
  options.actorOrdinals=parseActorOrdinals(options['actor-ordinals'],options.count);
  options.equipmentRoundtrip=options['equipment-roundtrip']??'off';
  check(['on','off'].includes(options.equipmentRoundtrip),'Equipment roundtrip must be on or off');
  options.transportProfile = options['transport-profile'] ?? 'native-gzip';
  probeTransportForActor(options.transportProfile, 0);
  check([30, 60].includes(options.seconds) && ['off', 'required'].includes(options.combat), 'Use 30/60 seconds and combat off/required');
  check(Number.isInteger(options.wallSeconds) && options.wallSeconds >= 120 && options.wallSeconds <= 3600, 'Wall must be 120..3600 seconds');
  for (const key of ['private-directory', 'output', 'monitor']) check(options[key] && path.isAbsolute(options[key]), 'Absolute private-directory/output/monitor required');
  options.privateDirectory = outsideRepository(options['private-directory']); options.output = outsideRepository(options.output);
  check(options.privateDirectory !== options.output, 'Private and public directories must differ');
  return options;
}
export async function atomicPrivate(file, value) {
  const temporary = file + '.' + crypto.randomBytes(4).toString('hex') + '.tmp';
  await fs.writeFile(temporary, JSON.stringify(value, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  await fs.rename(temporary, file);
}
export async function secureDirectory(directory) {
  await fs.mkdir(directory, { recursive: true, mode: 0o700 });
  check((await fs.lstat(directory)).isDirectory() && !(await fs.lstat(directory)).isSymbolicLink(), 'Private directory must be regular');
  if (process.platform === 'win32') {
    const owner = execFileSync('whoami.exe', [], { encoding: 'utf8', windowsHide: true }).trim();
    execFileSync('icacls.exe', [directory, '/inheritance:r', '/grant:r', `${owner}:(OI)(CI)F`], { stdio: 'pipe', windowsHide: true });
  } else await fs.chmod(directory, 0o700);
}
export function guestCookie(headers) {
  const values = headers.getSetCookie?.() ?? [headers.get('set-cookie') ?? ''];
  for (const value of values) {
    const match = /(?:^|,\s*)mir2_channel_guest=([0-9a-f-]{36})(?:;|$)/i.exec(value);
    if (match) return 'mir2_channel_guest=' + match[1];
  }
  return null;
}
export async function exchangeGuest(webBase, actor, { fetchImpl = fetch, remember = () => {} } = {}) {
  const post = async (route, body, cookie) => {
    const response = await fetchImpl(webBase + route, { method: 'POST', redirect: 'error',
      headers: { 'content-type': 'application/json', accept: 'application/json', ...(cookie ? { cookie } : {}) },
      body: JSON.stringify(body), signal: AbortSignal.timeout(8000) });
    check(response.ok, 'Public authentication HTTP rejected: ' + response.status + ' at ' + route);
    return { response, body: await response.json() };
  };
  const guest = await post('/api/channels/guest', { provider: 'directGuest' }, actor.cookie);
  const cookie = guestCookie(guest.response.headers) ?? actor.cookie;
  check(typeof cookie === 'string' && /^mir2_channel_guest=[0-9a-f-]{36}$/i.test(cookie), 'Guest endpoint omitted a durable dedicated cookie');
  actor.cookie = cookie; remember(cookie);
  check(guest.body.provider === 'directGuest' && typeof guest.body.credential === 'string', 'Guest endpoint omitted public proof');
  remember(guest.body.credential);
  const exchanged = await post('/api/channels/session/exchange', { provider: 'directGuest', credential: guest.body.credential });
  const proof = exchanged.body;
  check(typeof proof.accountId === 'string' && proof.accountId.length > 0 && typeof proof.token === 'string' && proof.token.includes('.') && proof.provider === 'directGuest', 'Exchange omitted signed Gateway identity');
  remember(proof.token);
  check(!actor.accountId || proof.accountId === actor.accountId, 'Guest cookie mapped to a different account on relogin');
  actor.accountId = proof.accountId;
  return { accountId: proof.accountId, token: proof.token, expiresAt: proof.expiresAt };
}

export class ProbeWaitDeadline extends Error {
  constructor(clientLabel, waitLabel, deadlineAtMs) {
    super(`${clientLabel} timed out waiting for ${waitLabel}`);
    this.waitLabel = waitLabel; this.deadlineAtMs = deadlineAtMs;
  }
}

export class SignedClient extends PlaytestClient {
  constructor(endpoint, actor, ordinal, context) {
    super(endpoint, actor.name, null, []); this.actor = actor; this.ordinal = ordinal; this.context = context;
    this.budget = new CommandBudget(); this.bytes = { sent: 0, received: 0 }; this.connection = ++context.connection;
    this.catalogFrames = 0; this.catalogEnvelopes = 0;
    this.transportProfile = context.transportForActor?.(ordinal) ?? context.transportProfile ?? 'native-gzip';
    check(['plain-Web', 'native-gzip'].includes(this.transportProfile), 'Unknown real client transport profile');
    this.setupBudgetMs = this.transportProfile === 'plain-Web' ? 60000 : 20000;
  }
  observe(message) {
    this.uiBefore = null;
    if (message.packet === 'UserInformation') {
      const p = message.payload ?? {}, old = self(this);
      this.uiBefore = { beforeOwnerId: this.snapshot?.playerObjectId ?? 0, beforeOwnerNameHash: nameHash(old?.name),
        beforeOwnerNameMatches: old?.name === this.actor.name, uiNameMatches: p.name === this.actor.name,
        uiNameHash: nameHash(p.name), uiObjectId: p.objectId ?? 0 };
      const freshStart = this.events.some(e => e.sequence > (this.bootstrapCursor ?? Infinity) && e.packet === 'StartGame' && e.payload?.result === 4);
      if (freshStart && p.name === this.actor.name && Number.isSafeInteger(p.objectId) && p.objectId > 0 && p.hp > 0 && this.snapshot)
        this.snapshot.playerObjectId = p.objectId; // Real Web UserInformation owner contract.
    }
    super.observe(message);
    if (['LogOutSuccess', 'LogOutFailed'].includes(message.packet) && this.logoutSentAt !== undefined) {
      this.logoutReceipt = { actorOrdinal: this.ordinal, connection: this.connection, packet: message.packet,
        requestSequence: this.logoutRequestSequence, responseSequence: this.sequence,
        elapsedMs: performance.now() - this.logoutSentAt, observedAfter5000Ms: performance.now() - this.logoutSentAt > 5000 };
      this.context.emit({ kind: 'logoutReceipt', atUtc: at(), monotonicMs: performance.now(), ...this.logoutReceipt });
    }
  }
  record(direction, value) {
    const safe = redact(value, this.context.secrets);
    const event = { ...safe, movementDirection: safe.direction, direction, sequence: ++this.sequence, monotonicMs: performance.now() };
    if (direction === 'received' && value.packet === 'UserLocation' && this.pendingMovementAck && event.sequence > this.pendingMovementAck.sequence) {
      this.lastMovementAckAt = event.monotonicMs; this.lastMovementKind = this.pendingMovementAck.kind;
      event.movementRequestSequence = this.pendingMovementAck.sequence; this.pendingMovementAck = null;
    }
    if (value.type === 'worldSnapshot') event.ownerNameMatches = self(this)?.name === this.actor.name;
    // Bounded in-memory packet receipts are used for matching; persistent evidence is scalar only.
    if (event.type === 'worldSnapshot') event.payload = { playerObjectId: this.snapshot?.playerObjectId, mapFileName: this.snapshot?.mapFileName };
    this.events.push(event); if (this.events.length > 4096) this.events.splice(0, 256);
    this.equipmentRegressionObserver?.(event);
    const p = value.payload ?? value;
    const row = { actorOrdinal: this.ordinal, connection: this.connection, sequence: event.sequence,
      direction, kind: value.packet ?? value.type, atUtc: at(), monotonicMs: event.monotonicMs };
    for (const key of ['objectId', 'attackerId', 'damage', 'percent', 'hp', 'mp', 'result', 'success', 'time'])
      if (typeof p[key] === 'boolean' || typeof p[key] === 'number' && Number.isFinite(p[key])) row[key] = p[key];
    const point = p.location ?? p;
    if (Number.isFinite(point.x) && Number.isFinite(point.y)) { row.x = point.x; row.y = point.y; }
    if (event.movementRequestSequence) { row.movementRequestSequence = event.movementRequestSequence; row.movementKind = this.lastMovementKind; }
    if (direction === 'sent' && value.type === 'attack') {
      const readiness = preparationAttackReadiness({ lastMovementAckAt: this.lastMovementAckAt, movementKind: this.lastMovementKind,
        poison: self(this)?.poison ?? this.snapshot?.playerPoison ?? 0 });
      row.lastMovementAckAgeMs = Number.isFinite(this.lastMovementAckAt) ? event.monotonicMs-this.lastMovementAckAt : -1;
      row.movementCooldownMs = readiness.movementCooldownMs; row.afterObservedMovementReady = event.monotonicMs >= readiness.readyAtMs;
    }
    if (direction === 'received' && ['UserInformation', 'MapInformation', 'StartGame'].includes(value.packet) || value.type === 'worldSnapshot') {
      Object.assign(row, { bootstrapCursor: this.bootstrapCursor ?? 0, ownerObjectId: this.snapshot?.playerObjectId ?? 0,
        ownerNameMatches: self(this)?.name === this.actor.name, ownerNameHash: nameHash(self(this)?.name),
        expectedNameHash: nameHash(this.actor.name), ownerHp: this.snapshot?.playerHp ?? 0,
        ownerX: self(this)?.x ?? -1, ownerY: self(this)?.y ?? -1,
        mapFileName: String(value.packet === 'MapInformation' ? p.fileName ?? '' : this.snapshot?.mapFileName ?? ''),
        ...(value.packet === 'UserInformation' ? this.uiBefore ?? {} : {}) });
    }
    this.context.emit(row); return event;
  }
  assert() { if (!this.draining) this.context.assert(); }
  async wait(predicate, label, timeout = 15000) {
    const deadline=performance.now()+timeout;
    for(;;){
      if(this.failure)throw this.failure;this.assert();
      const result=predicate(),now=performance.now();
      if(result&&(Number.isFinite(result.monotonicMs)?result.monotonicMs<=deadline:now<=deadline))return result;
      if(this.closed)throw new Error(`${this.label} disconnected during ${label}`);
      const remaining=deadline-now;
      if(remaining<=0)throw new ProbeWaitDeadline(this.label,label,deadline);
      await sleep(Math.min(25,remaining));
    }
  }
  async pace() { while (this.budget.remainingWait()) { this.assert(); await sleep(Math.min(25, this.budget.remainingWait())); } }
  async request(command, packet, timeout = 15000) {
    await this.pace();const cursor=this.sequence;this.send(command);
    const sent=this.events.find(e=>e.sequence>cursor&&e.direction==='sent'&&e.type===command.type);
    check(Number.isFinite(sent?.monotonicMs),'Request omitted its real send receipt');
    const deadline=sent.monotonicMs+timeout;
    if(command.type==='startGame')this.startGameTiming={sentAtMs:sent.monotonicMs,deadlineAtMs:deadline,budgetMs:timeout,
      requestCursor:cursor,requestSequence:sent.sequence};
    const response=await this.wait(()=>{
      const rejection=this.events.find(e=>e.sequence>cursor&&e.direction==='received'&&e.type==='error');
      if(rejection)throw new Error(`Gateway rejected ${command.type}: ${rejection.message??'unknown error'}`);
      return this.events.find(e=>e.sequence>cursor&&e.direction==='received'&&e.packet===packet&&
        Number.isFinite(e.monotonicMs)&&e.monotonicMs<=deadline);
    },packet,deadline-performance.now());
    if(command.type==='startGame')Object.assign(this.startGameTiming,{ackAtMs:response.monotonicMs,
      ackElapsedMs:response.monotonicMs-sent.monotonicMs,pollObservedAtMs:performance.now()});
    return response;
  }
  send(command) {
    check(!this.draining || ['keepAlive', 'logOut'].includes(command?.type), 'Only keepAlive/logOut cleanup commands are allowed during drain');
    this.assert(); this.budget.reserve();
    if (['walk', 'run', 'turn'].includes(command?.type)) this.pendingMovementAck = { kind: command.type, sequence: this.sequence + 1 };
    if (command?.type === 'startGame') this.bootstrapSentAt = performance.now();
    if (command?.type === 'logOut') { this.logoutSentAt = performance.now(); this.logoutRequestSequence = this.sequence + 1; }
    const signed = command?.type === 'passkeyLogin';
    if (command?.type === 'clientCapabilities') {
      check(exactFields(command, ['type', 'capabilities']) && JSON.stringify(command.capabilities) === JSON.stringify([CATALOG_GZIP_CAPABILITY]), 'Only ordinary catalog-gzip capability is enabled');
      this.catalogGzipOptedIn = true; this.record('sent', command); this.ws.send(JSON.stringify(command));
    } else if (signed) {
      check(exactFields(command, ['type', 'accountId', 'token']) && command.accountId === this.allowedLoginAccountId &&
        command.token === this.allowedLoginToken && typeof command.token === 'string', 'Signed login requires this actor freshly exchanged proof');
      this.allowedLoginToken = null; this.record('sent', command); this.ws.send(JSON.stringify(command));
    } else super.send(command); // Keep the existing ordinary whitelist; no QA/admin additions.
    this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
  }
  heartbeat() {
    if (this.ws.readyState === WebSocket.OPEN && !this.budget.remainingWait() && (this.draining || !this.context.guard?.stopped)) {
      try { this.send({ type: 'keepAlive', time: Date.now() }); }
      catch (error) { if (this.draining || !this.context.guard?.stopped) this.failure = error; }
    }
  }
  observeWebSocketClosing() {
    const ws = this.ws, edges = new Set();
    this.webSocketClosingDiagnostics = [];
    let receiverConcludeAttached = false;
    const flag = value => typeof value === 'boolean' ? value : null;
    this.traceWebSocketClosing = (edge, extra = {}) => {
      // These observers cannot turn a transport/evidence error into a lifecycle
      // action, change the close verdict, or interrupt ws's own event handlers.
      if (edges.has(edge)) return;
      edges.add(edge);
      try {
        const socket = ws._socket;
        const row = { kind: 'webSocketClosing', edge, actorOrdinal: this.ordinal, connection: this.connection,
          atUtc: at(), monotonicMs: performance.now(), receiverConcludeAttached,
          closeFrameSent: flag(ws._closeFrameSent), closeFrameReceived: flag(ws._closeFrameReceived),
          readyState: Number.isInteger(ws.readyState) ? ws.readyState : null,
          bufferedAmount: Number.isSafeInteger(ws.bufferedAmount) && ws.bufferedAmount >= 0 ? ws.bufferedAmount : null,
          socketEnded: flag(socket?.readableEnded), socketWritableEnded: flag(socket?.writableEnded),
          socketWritableFinished: flag(socket?.writableFinished), socketDestroyed: flag(socket?.destroyed), ...extra };
        this.webSocketClosingDiagnostics.push(row); this.context.emit(row);
      } catch { /* Diagnostic failures never affect protocol or close handling. */ }
    };
    const attach = () => {
      try {
        // This compiled ws version emits conclude only after validating the
        // complete Close frame. Ignore its code/reason arguments entirely.
        if (typeof ws._receiver?.once === 'function') {
          ws._receiver.once('conclude', () => this.traceWebSocketClosing('validCloseFrameReceived'));
          receiverConcludeAttached = true;
        }
        ws._socket?.once('finish', () => this.traceWebSocketClosing('socketWritableFinish'));
        ws._socket?.once('end', () => this.traceWebSocketClosing('socketReadableEnd'));
        ws._socket?.once('close', () => this.traceWebSocketClosing('socketClose'));
      } catch { /* Optional private observability must not alter ws behavior. */ }
    };
    try {
      ws.once('open', attach);
      ws.once('close', () => this.traceWebSocketClosing('webSocketClose'));
    } catch { /* Leave all existing lifecycle listeners in control. */ }
  }
  async connect() {
    this.webSocketCreated = true;
    this.ws = new WebSocket(this.url, { origin: this.context.webBase, handshakeTimeout: 10000 });
    this.observeWebSocketClosing();
    this.ws.addEventListener('message', event => {
      try {
        this.bytes.received += typeof event.data === 'string' ? Buffer.byteLength(event.data) : event.data.byteLength;
        const decoded = decodePlaytestFrame(event.data, this.catalogGzipOptedIn);
        if (typeof event.data !== 'string') {
          this.catalogFrames++; this.catalogEnvelopes += decoded.length;
          this.context.emit({ kind: 'catalogGzipFrame', actorOrdinal: this.ordinal, connection: this.connection,
            atUtc: at(), monotonicMs: performance.now(), wireBytes: event.data.byteLength, envelopes: decoded.length });
        }
        for (const message of decoded) this.observe(message);
      } catch (error) { this.failure = error; this.ws.close(1002, 'Invalid gateway frame'); }
    });
    this.ws.addEventListener('error', () => { this.failure = new Error('WebSocket connection failed'); });
    this.ws.addEventListener('close', () => { this.closed = true; });
    await this.wait(() => this.ws.readyState === WebSocket.OPEN, 'public WebSocket open');
    if (this.transportProfile === 'native-gzip') this.send({ type: 'clientCapabilities', capabilities: [CATALOG_GZIP_CAPABILITY] });
    await this.request({ type: 'clientVersion' }, 'ClientVersion');
    await this.wait(() => this.events.some(e => e.packet === 'Connected'), 'Connected handshake');
    this.timer = setInterval(() => this.heartbeat(), 2000);
  }
  async logout() {
    if (!this.inGame) return;
    const cursor = this.sequence;
    await this.pace(); this.send({ type: 'logOut' });
    const response = await this.wait(() => this.events.find(e => e.sequence > cursor && e.direction === 'received' &&
      ['LogOutSuccess', 'LogOutFailed'].includes(e.packet)), 'explicit normal logout response', 5000);
    check(response.packet === 'LogOutSuccess', 'Original combat lock refused normal logout'); this.inGame = false;
  }
  async close() {
    this.draining = true;
    this.traceWebSocketClosing?.('clientCloseStarted');
    try { await super.close(); }
    catch (error) {
      if (error.message === `${this.label} WebSocket close was not acknowledged within 5 seconds`)
        this.traceWebSocketClosing?.('closeEventDeadlineExceeded', { deadlineMs: 5000, afterForcedTerminate: true });
      throw error;
    }
    finally { await this.writeQueue; }
  }
}

export function state(c) {
  const p = self(c); check(p && !p.dead && c.inGame && !c.closed, 'Exact living authenticated owner required');
  return { map: String(c.snapshot.mapFileName), x: p.x, y: p.y, direction: p.direction,
    inventory: inventoryFingerprint(c.snapshot) };
}

const EQUIPMENT_SLOTS=['weapon','armour','helmet','torch','necklace','braceletLeft','braceletRight','ringLeft','ringRight','amulet','belt','boots','stone','mount'];
const equipmentSlot=item=>EQUIPMENT_SLOTS.indexOf(String(item.slot??item.equipSlot));
const canonical=value=>Array.isArray(value)?value.map(canonical):value&&typeof value==='object'?
  Object.fromEntries(Object.keys(value).sort().map(key=>[key,canonical(value[key])])):value;
export function equipmentStateHash(snapshot) {
  check(Array.isArray(snapshot?.inventoryItems)&&Array.isArray(snapshot?.equipmentItems),'Complete authoritative item rosters required');
  const roster=key=>(snapshot[key]??[]).map(canonical).sort((a,b)=>String(a.uniqueId).localeCompare(String(b.uniqueId)));
  return crypto.createHash('sha256').update(JSON.stringify({inventory:roster('inventoryItems'),equipment:roster('equipmentItems'),
    belt:roster('beltItems'),inventoryCapacity:snapshot.inventoryCapacity,maxBagSlots:snapshot.maxBagSlots,gold:snapshot.gold})).digest('hex');
}
export function equipmentRoundtripPlan(snapshot) {
  check(Number.isInteger(snapshot?.maxBagSlots)&&snapshot.maxBagSlots>0,'Authoritative bag capacity missing');
  check(Array.isArray(snapshot.inventoryItems)&&Array.isArray(snapshot.equipmentItems),'Complete authoritative item rosters required');
  const item=snapshot.equipmentItems?.find(i=>equipmentSlot(i)>=0&&Number.isSafeInteger(i.uniqueId)&&i.uniqueId>0);
  check(item,'Equipment regression requires an already equipped item; it cannot skip an equipped cohort');
  const occupied=new Set(snapshot.inventoryItems.map(i=>Number(i.slot)));
  const bagSlot=Array.from({length:Math.min(40,snapshot.maxBagSlots)},(_,i)=>i).find(i=>!occupied.has(i));
  check(bagSlot!==undefined,'Equipment regression requires a real empty first-Bag cell');
  return {uniqueId:item.uniqueId,equipmentSlot:equipmentSlot(item),bagSlot};
}
export function equipmentRegressionActors(value,count) {
  if(value===undefined)return [];
  const parts=String(value).split(',');
  check(parts.length<=100&&parts.every(p=>/^\d+$/.test(p)),'Equipment actor list must contain at most100 nonnegative indices');
  const result=parts.map(Number);
  check(result.every(i=>Number.isSafeInteger(i)&&i<count),'Equipment actor index is outside the requested cohort');
  return result;
}
export async function equipmentRoundtrip(c,result={actorOrdinal:c.ordinal,passed:false,steps:[]}) {
  const before=state(c),ownerObjectId=c.snapshot.playerObjectId,plan=equipmentRoundtripPlan(c.snapshot),beforeHash=equipmentStateHash(c.snapshot);
  check(!c.equipmentRegressionObserver,'Equipment regression already active on this connection');
  Object.assign(result,{actorOrdinal:c.ordinal,originalOrdinal:c.originalOrdinal??c.ordinal,connection:c.connection,passed:false,
    startedAtMs:performance.now(),beforeFingerprintHash:beforeHash,equipmentSlot:plan.equipmentSlot,bagSlot:plan.bagSlot,
    itemIdentityHash:crypto.createHash('sha256').update(String(plan.uniqueId)).digest('hex')});
  result.steps??=[];let current;
  const emit=(phase,event)=>c.context.emit({kind:'equipmentRegressionReceipt',actorOrdinal:c.ordinal,connection:c.connection,
    operation:current.operation,phase,sequence:event.sequence,monotonicMs:event.monotonicMs,atUtc:at(),
    itemIdentityHash:result.itemIdentityHash,targetSlot:current.to,ackSuccess:current.ackSuccess??false,
    ackWithinBudget:current.ackWithinBudget??false,stateWithinBudget:current.stateWithinBudget??false,
    ...(phase==='state'?{fingerprintHash:current.stateFingerprintHash}:{})});
  c.equipmentRegressionObserver=event=>{
    if(!current)return;
    if(event.direction==='sent'&&event.type===current.operation&&event.sequence>current.cursorBefore){
      current.sentSequence=event.sequence;current.sentAtMs=event.monotonicMs;current.ackDeadlineAtMs=event.monotonicMs+15000;emit('sent',event);
    }
    const payload=event.payload;
    if(event.direction==='received'&&event.sequence>(current.sentSequence??Infinity)&&event.packet===current.packet&&
      payload?.uniqueId===plan.uniqueId&&payload.to===current.to&&String(payload.grid).toLowerCase()==='inventory'&&current.ackSequence===undefined){
      Object.assign(current,{ackSequence:event.sequence,ackAtMs:event.monotonicMs,ackElapsedMs:event.monotonicMs-current.sentAtMs,
        ackWithinBudget:event.monotonicMs<=current.ackDeadlineAtMs,ackSuccess:payload.success===true,stateDeadlineAtMs:event.monotonicMs+15000});emit('ack',event);
    }
    if(event.direction!=='received'||event.type!=='worldSnapshot'||event.sequence<=(current.ackSequence??Infinity)||!current.ackSuccess)return;
    const snapshot=c.snapshot,p=self(c);
    if(!c.inGame||!p||p.name!==c.actor.name||p.dead||snapshot.playerHp<=0||snapshot.playerObjectId!==ownerObjectId||String(snapshot.mapFileName)!==before.map||
      !Array.isArray(snapshot.inventoryItems)||!Array.isArray(snapshot.equipmentItems))return;
    const hash=equipmentStateHash(snapshot),inBag=snapshot.inventoryItems.some(i=>i.uniqueId===plan.uniqueId&&Number(i.slot)===plan.bagSlot),
      equipped=snapshot.equipmentItems.some(i=>i.uniqueId===plan.uniqueId&&equipmentSlot(i)===plan.equipmentSlot);
    const matched=current.operation==='removeItem'?inBag&&!snapshot.equipmentItems.some(i=>i.uniqueId===plan.uniqueId):
      equipped&&!snapshot.inventoryItems.some(i=>i.uniqueId===plan.uniqueId)&&hash===beforeHash;
    if(!matched||current.stateSequence!==undefined)return;
    Object.assign(current,{stateSequence:event.sequence,stateAtMs:event.monotonicMs,stateElapsedMs:event.monotonicMs-current.ackAtMs,
      stateWithinBudget:event.monotonicMs<=current.stateDeadlineAtMs,stateFingerprintHash:hash});
    event.equipmentRegressionProof={monotonicMs:event.monotonicMs,fingerprintHash:hash};emit('state',event);
  };
  try{
    for(const [operation,packet,to]of[['removeItem','RemoveItem',plan.bagSlot],['equipItem','EquipItem',plan.equipmentSlot]]){
      current={operation,packet,to,cursorBefore:c.sequence,ackBudgetMs:15000,stateBudgetMs:15000,passed:false};result.steps.push(current);
      const ack=await c.request({type:operation,grid:'inventory',uniqueId:plan.uniqueId,to},packet,15000);
      check(ack.payload?.success===true&&ack.payload.uniqueId===plan.uniqueId&&ack.payload.to===to&&
        String(ack.payload.grid).toLowerCase()==='inventory'&&current.ackSequence===ack.sequence,'Equipment regression lacks the exact fresh successful item ACK');
      await c.wait(()=>c.events.find(e=>e.sequence>ack.sequence&&e.equipmentRegressionProof&&e.monotonicMs<=current.stateDeadlineAtMs),
        'fresh complete equipment regression state',current.stateDeadlineAtMs-performance.now());
      check(current.ackWithinBudget&&current.stateWithinBudget,'Equipment regression exceeded an original phase deadline');current.passed=true;
    }
    result.afterFingerprintHash=equipmentStateHash(c.snapshot);check(result.afterFingerprintHash===beforeHash,'Equipment roundtrip did not restore exact carried item state');
    result.passed=true;result.completedAtMs=performance.now();delete c.equipmentRegressionObserver;return result;
  }catch(error){
    result.failure=redact(error.message,[...c.context.secrets,c.label]);result.failedAtMs=performance.now();
    // Retain this read-only observer through cleanup so late real receipts stay
    // auditable. They never change the failed verdict or trigger another action.
    throw new Error(result.failure);
  }
}
export async function preconnectWalletProof(c, context, purpose, {
  exchange = exchangeSuiWallet, persistPrivate = atomicPrivate, wait = sleep,
} = {}) {
  check(['admission', 'verification'].includes(purpose), 'Unknown ordinary login purpose');
  const actor = c.actor, name = actor.name, walletFields = ['kind', 'secretKey', 'address', 'accountId', 'characterIndex'];
  let pinnedWallet = actor.walletIdentity ? walletFields.map(key => actor.walletIdentity[key]) : null;
  const sameIdentity = () => c.actor === actor && actor.name === name && (!pinnedWallet ||
    actor.walletIdentity && walletFields.every((key, index) => actor.walletIdentity[key] === pinnedWallet[index]));
  const preconnect = () => !c.webSocketCreated && !c.ws && c.sequence === 0 && !c.inGame;
  const record = row => {
    const evidence = { kind: 'admissionTransportRecovery', actorOrdinal: c.ordinal, connection: c.connection,
      purpose, phase: 'preconnect-public-proof', maximumAttempts: 3, atUtc: at(), monotonicMs: performance.now(), ...row };
    (context.admissionTransportRecovery ??= []).push(evidence); context.emit(evidence);
  };
  for (let attempt = 1; attempt <= 3; attempt++) {
    check(sameIdentity(), 'Dedicated wallet or character changed before public auth attempt');
    check(preconnect(), 'Public proof attempts require a client that has never created a WebSocket');
    context.assert();
    while (context.loginBudget.delay(purpose)) { context.assert(); await wait(Math.min(100, context.loginBudget.delay(purpose))); }
    context.assert(); context.loginBudget.reserve(purpose);
    // Every attempt, including failed transport, consumes the same real budget.
    await persistPrivate(context.ledgerFile, { schemaVersion: 1, purpose: 'cloud-reference-public-auth-budget', endpoint: context.endpoint,
      webBase: context.webBase, attempts: context.loginBudget.attempts });
    context.emit({ kind: 'publicAuthAttemptReserved', actorOrdinal: c.ordinal, connection: c.connection,
      purpose, attempt, maximumAttempts: 3, atUtc: at(), monotonicMs: performance.now() });
    const started = performance.now(); let proof;
    try {
      proof = await exchange(context.webBase, actor, { remember: value => context.secrets.push(value),
        persist: () => persistPrivate(context.actorsFile, context.pool) });
    } catch (error) {
      if (purpose === 'admission') context.firstAttemptAdmissionOk = false;
      const timeout = error instanceof PublicAuthFetchTimeout;
      const unchanged = sameIdentity(), beforeSocket = preconnect();
      const retry = timeout && unchanged && beforeSocket && attempt < 3;
      record({ attempt, outcome: 'failed', retryScheduled: retry, retryExhausted: timeout && attempt === 3,
        failureCode: !unchanged ? 'PUBLIC_AUTH_IDENTITY_CHANGED' : timeout ? error.code : 'PUBLIC_AUTH_NON_RETRYABLE',
        route: timeout ? error.route : null, elapsedMs: timeout ? error.elapsedMs : null,
        attemptElapsedMs: performance.now() - started, httpTimeoutMs: timeout ? error.timeoutMs : null,
        beforeWebSocket: beforeSocket });
      if (!unchanged) throw new Error('Dedicated wallet or character changed during public auth attempt');
      if (!retry) throw error;
      // A newly generated synthetic wallet has been persisted by exchange(). Pin
      // that identity for every retry; an existing wallet was pinned at entry.
      if (!pinnedWallet && actor.walletIdentity) pinnedWallet = walletFields.map(key => actor.walletIdentity[key]);
      continue;
    }
    if (attempt > 1) record({ attempt, outcome: 'recovered', attemptElapsedMs: performance.now() - started,
      beforeWebSocket: preconnect(), scope: 'Public proof only; WebSocket/bootstrap and steady SLO remain separate' });
    return proof;
  }
}
export async function login(c, context, purpose, proofOptions) {
  const proof = await preconnectWalletProof(c, context, purpose, proofOptions);
  await (proofOptions?.persistPrivate ?? atomicPrivate)(context.actorsFile, context.pool);
  await c.connect(); c.allowedLoginToken = proof.token; c.allowedLoginAccountId = proof.accountId;
  const ack = await c.request({ type: 'passkeyLogin', accountId: proof.accountId, token: proof.token }, 'LoginSuccess');
  const characters = ack.payload?.characters ?? [];
  let selected = characters.find(v => v.name === c.actor.name);
  const identity = c.actor.walletIdentity;
  if (identity.characterIndex === undefined) {
    check(characters.length === 0, 'Fresh synthetic guest unexpectedly owns a character');
    selected = (await c.request({ type: 'newCharacter', name: c.actor.name, class: 'Warrior', gender: 'Male' }, 'NewCharacterSuccess')).payload?.character;
  }
  check(Number.isInteger(selected?.index) && (identity.characterIndex === undefined || selected.index === identity.characterIndex), 'Dedicated wallet character absent or changed');
  identity.characterIndex = selected.index; await atomicPrivate(context.actorsFile, context.pool);
  await completeSignedBootstrap(c, context, purpose, selected.index);
}
export async function completeSignedBootstrap(c, context, purpose, characterIndex) {
  const cursor = c.sequence, actor = c.actor, name = actor.name, socket = c.ws, identity = actor.walletIdentity;
  const identityFields = ['kind', 'secretKey', 'address', 'accountId', 'characterIndex'];
  const pinnedIdentity = identityFields.map(key => identity?.[key]);
  c.bootstrapCursor = cursor;
  context.emit({ kind: 'bootstrapCacheReset', actorOrdinal: c.ordinal, connection: c.connection, atUtc: at(), monotonicMs: performance.now(),
    beforeOwnerId: c.snapshot?.playerObjectId ?? 0, beforeOwnerNameMatches: self(c)?.name === c.actor.name,
    beforeOwnerNameHash: nameHash(self(c)?.name), expectedNameHash: nameHash(c.actor.name), bootstrapCursor: cursor });
  c.snapshot = null; // Match the canonical ordinary authenticate helper.
  const bootstrapLabel = 'fresh native UI/map and complete authoritative owner snapshot';
  let start, evidence, phase = 'startGameAck', bootstrapDeadline, diagnosticAcceptedLate = false;
  try {
    start = await c.request({ type: 'startGame', characterIndex }, 'StartGame', 20000);
    check(start.payload?.result === 4, 'StartGame rejected; open socket is not a player');
    phase = 'postAckCompleteBootstrap'; bootstrapDeadline = start.monotonicMs + c.setupBudgetMs;
    evidence = await c.wait(() => { const proof = bootstrapEvidence(c, cursor, name);
      return proof.ready && Number.isFinite(proof.monotonicMs) && proof.monotonicMs <= bootstrapDeadline ? proof : null;
    }, bootstrapLabel, bootstrapDeadline - performance.now());
  } catch (error) {
    // Only the two controlled bootstrap deadline sites can enter observation.
    // Auth, rejection, decoding, transport and guard errors retain their path.
    const timing = c.startGameTiming;
    if (!(error instanceof ProbeWaitDeadline) || error.waitLabel !== (phase === 'startGameAck' ? 'StartGame' : bootstrapLabel) ||
      !Number.isFinite(timing?.deadlineAtMs) || !Number.isFinite(timing.sentAtMs)) throw error;
    const originalDeadlineAtMs = phase === 'startGameAck' ? timing.deadlineAtMs : bootstrapDeadline;
    const originalBudgetMs = phase === 'startGameAck' ? timing.budgetMs : c.setupBudgetMs;
    const observationDeadlineAtMs = originalDeadlineAtMs + 60000;
    context.firstAttemptAdmissionOk = false; context.allSloPassed = false;
    const safe = message => redact(message, [...(context.secrets ?? []), c.label, name]);
    const observedAtMs = performance.now();
    const miss = { kind: 'admissionSloMiss', actorOrdinal: c.ordinal, connection: c.connection, purpose, phase,
      atUtc: at(), monotonicMs: observedAtMs, observedAtMs, bootstrapCursor: cursor,
      startGameRequestCursor: timing.requestCursor, startGameRequestSequence: timing.requestSequence,
      startGameSentAtMs: timing.sentAtMs, originalDeadlineAtMs, originalBudgetMs,
      originalStartGameAckBudgetMs: timing.budgetMs, originalPostAckBootstrapBudgetMs: c.setupBudgetMs,
      transportProfile: c.transportProfile, observationDeadlineAtMs,
      failure: { name: error.name, message: safe(error.message) } };
    (context.admissionSloMisses ??= []).push(miss); context.emit(miss);
    if (context.observeLateBootstrap !== true || context.campaignMode !== 'explore') throw error;
    const unchanged = () => {
      check(c.ws === socket && socket?.readyState === WebSocket.OPEN && !c.closed, 'Original authenticated bootstrap socket is no longer open');
      check(c.actor === actor && actor.name === name && actor.walletIdentity === identity &&
        identityFields.every((key, index) => identity?.[key] === pinnedIdentity[index]) && c.bootstrapCursor === cursor &&
        c.startGameTiming === timing, 'Original bootstrap identity, cursor or request changed');
      const sent = c.events.filter(e => e.direction === 'sent' && e.sequence > timing.requestCursor && e.type === 'startGame');
      check(sent.length === 1 && sent[0].sequence === timing.requestSequence && sent[0].characterIndex === characterIndex,
        'Original StartGame send receipt changed');
      check(!c.events.some(e => e.sequence > timing.requestCursor && e.direction === 'received' && e.type === 'error'), 'Gateway rejected original bootstrap');
      const starts = c.events.filter(e => e.sequence > timing.requestCursor && e.direction === 'received' && e.packet === 'StartGame');
      check(starts.every(e => e.payload?.result === 4 && Number.isFinite(e.monotonicMs)), 'StartGame rejected or malformed during observation');
      const users = c.events.filter(e => e.sequence > cursor && e.direction === 'received' && e.packet === 'UserInformation');
      check(users.every(e => e.payload?.name === name && Number.isSafeInteger(e.payload.objectId) && e.payload.objectId > 0 && e.payload.hp > 0),
        'Wrong or malformed authenticated owner during observation');
      check(!self(c) || self(c).name === name, 'Wrong snapshot owner during observation');
    };
    try {
      if (c.failure) throw c.failure; c.assert(); unchanged();
      context.diagnosticObserveUsed = true;
      context.emit({ ...miss, kind: 'bootstrapDiagnosticObservationStarted', additionalBudgetMs: 60000 });
      // No request is sent here. Both waits share the first miss's absolute
      // deadline, including when the original StartGame ACK itself was late.
      start = await c.wait(() => { unchanged(); return c.events.find(e => e.sequence > timing.requestCursor &&
        e.direction === 'received' && e.packet === 'StartGame' && Number.isFinite(e.monotonicMs) && e.monotonicMs <= observationDeadlineAtMs);
      }, 'late original StartGame receipt observation', observationDeadlineAtMs - performance.now());
      if (phase === 'startGameAck') Object.assign(timing, {
        ackAtMs: start.monotonicMs, ackElapsedMs: start.monotonicMs - timing.sentAtMs, pollObservedAtMs: performance.now() });
      evidence = await c.wait(() => { unchanged(); const proof = bootstrapEvidence(c, cursor, name);
        return proof.ready && Number.isFinite(proof.monotonicMs) && proof.monotonicMs <= observationDeadlineAtMs ? proof : null;
      }, 'late original complete bootstrap observation', observationDeadlineAtMs - performance.now());
      diagnosticAcceptedLate = true;
      context.emit({ kind: 'bootstrapDiagnosticLateCompletion', actorOrdinal: c.ordinal, connection: c.connection, purpose, phase,
        atUtc: at(), monotonicMs: performance.now(), bootstrapCursor: cursor, originalDeadlineAtMs, originalBudgetMs,
        observationDeadlineAtMs, completedReceiptAtMs: evidence.monotonicMs, strictSetupDeadlinePassed: false, diagnosticAcceptedLate: true });
    } catch (observationError) {
      context.emit({ kind: 'bootstrapDiagnosticObservationAborted', actorOrdinal: c.ordinal, connection: c.connection, purpose, phase,
        atUtc: at(), monotonicMs: performance.now(), originalDeadlineAtMs, observationDeadlineAtMs,
        failure: { name: observationError.name, message: safe(observationError.message) } });
      throw error; // The original failed admission remains the primary failure.
    }
  }
  const postAckBootstrapStartedAtMs = start.monotonicMs;
  c.bootstrapProof = { kind: 'bootstrapVerified', actorOrdinal: c.ordinal, connection: c.connection, atUtc: at(), monotonicMs: performance.now(),
    elapsedMs: performance.now() - c.bootstrapSentAt, catalogFrames: c.catalogFrames, catalogEnvelopes: c.catalogEnvelopes,
    startGameAckElapsedMs:c.startGameTiming.ackElapsedMs,startGameAckBudgetMs:c.startGameTiming.budgetMs,
    fullBootstrapElapsedMs:evidence.monotonicMs-c.startGameTiming.sentAtMs,
    postAckBootstrapElapsedMs:evidence.monotonicMs-postAckBootstrapStartedAtMs,postAckBootstrapBudgetMs:c.setupBudgetMs,
    budgetScope:'StartGame ACK and post-ACK complete bootstrap have separate budgets; total elapsed is reported separately',
    setupApplicationBytesSent: c.bytes.sent, setupApplicationBytesReceived: c.bytes.received,
    transportProfile: c.transportProfile, setupBudgetMs: c.setupBudgetMs,
    strictSetupDeadlinePassed: !diagnosticAcceptedLate, diagnosticAcceptedLate,
    ...evidence };
  context.emit(c.bootstrapProof);
  state(c); context.emit({ kind: purpose === 'admission' ? 'admitted' : 'relogged', actorOrdinal: c.ordinal, atUtc: at(), monotonicMs: performance.now() });
}
async function socialMove(clients, c, loadMap, context) {
  const peers = clients.filter(v => v !== c); const before = state(c), cursors = peers.map(p => p.sequence);
  const ids = peers.map(p => observedPlayerId(p, c.label)); check(ids.every(Number.isInteger), 'Actor missing from actual peer AOI');
  const map = await loadMap(before.map);
  const occupied = c.snapshot.entities.filter(e => e.objectId !== self(c).objectId && !e.dead);
  let moved = false;
  for (const [dx, dy] of [[2, 0], [-2, 0], [0, 2], [0, -2], [1, 1], [-1, -1]]) {
    const target = { x: before.x + dx, y: before.y + dy };
    const route = findProtocolWalkPath({ map, start: before, target, dynamicObstacles: occupied, maxExpanded: 1500 });
    if (!route || !route.length || route.length > 6) continue;
    await c.pace(); await createNavigator(c, { loadCollisionMap: loadMap, movementResponseTimeoutMs: 2500 })(target, 0,
      () => Date.now() >= context.deadline, { maxSuccessfulSteps: 8, maxAttempts: 12, maxNoPathRefreshes: 1,
        beforeMovement() { c.assert(); return null; } });
    moved = distance(before, self(c)) > 0; if (moved) break;
  }
  check(moved, 'No bounded ordinary movement route succeeded'); const after = state(c);
  await Promise.all(peers.map((p, i) => p.wait(() => p.events.find(e => e.sequence > cursors[i] && ['ObjectWalk', 'ObjectRun'].includes(e.packet) &&
    e.payload?.objectId === ids[i] && (e.payload.location?.x ?? e.payload.x) === after.x && (e.payload.location?.y ?? e.payload.y) === after.y), 'peer authoritative movement', 2500)));
  return { actorOrdinal: c.ordinal, passed: true, beforeX: before.x, beforeY: before.y, afterX: after.x, afterY: after.y, peerWitnesses: peers.length };
}
export function sharedChatIdentity(clients, speaker) {
  const peerIds = clients.filter(p => p !== speaker).map(p => observedPlayerId(p, speaker.label));
  check(peerIds.length > 0 && peerIds.every(id => Number.isSafeInteger(id) && id > 0) && new Set(peerIds).size === 1,
    'Every peer must agree on the actual shared chat speaker identity');
  return peerIds[0];
}
export function exactProbeChat(payload, sharedId, nonce) {
  if (!/^cg[0-9a-f]{10}$/.test(nonce) || Number(payload?.objectId) !== sharedId) return false;
  const text = String(payload?.text ?? payload?.message ?? '');
  return new RegExp('(?:^|[\\s:])' + nonce + '$').test(text);
}
export async function chatAll(clients, c, { enforceLatency = true } = {}) {
  // ObjectChat retains the Zone/shared sender ID even on the owner's connection;
  // its owner-normalized snapshot self ID1000 is not the chat sender identity.
  const sharedId = sharedChatIdentity(clients, c), cursors = clients.map(p => p.sequence);
  const message = 'cg' + crypto.randomBytes(5).toString('hex'); await c.pace(); const sent = performance.now(); c.send({ type: 'chat', message });
  const receipts = await Promise.all(clients.map((p, i) => p.wait(() => p.events.find(e => e.sequence > cursors[i] && e.packet === 'ObjectChat' &&
    exactProbeChat(e.payload, sharedId, message)), 'ordinary chat on every observer', 2500)));
  const latencyMs = Math.max(...receipts.map(e => e.monotonicMs - sent));
  if (enforceLatency) check(latencyMs <= 1000, 'Chat witness exceeded1000ms');
  return { actorOrdinal: c.ordinal, passed: latencyMs <= 1000, observers: receipts.length, sharedObjectId: sharedId, sentAtMs: sent, latencyMs };
}
export async function weakPve(clients, c, loadMap, context) {
  const observer = clients[(c.ordinal + 1) % clients.length]; const names = new Set(['Hen', 'Deer', 'Scarecrow']);
  const candidates = c.snapshot.entities.filter(e => e.kind === 'monster' && names.has(e.name) && !e.dead && distance(self(c), e) <= 18)
    .sort((a, b) => distance(self(c), a) - distance(self(c), b)).slice(0, 4);
  check(candidates.length > 0, 'No live ordinary weak monster in actual AOI; no spawn/teleport fallback');
  for (const candidate of candidates) {
    for (const client of [c, observer]) {
      const target = client.snapshot.entities.find(e => e.objectId === candidate.objectId && !e.dead); if (!target) break;
      await client.pace(); await createNavigator(client, { loadCollisionMap: loadMap, movementResponseTimeoutMs: 2500 })(target, client === c ? 1 : 4,
        () => Date.now() >= context.deadline, { maxSuccessfulSteps: 35, maxAttempts: 50, maxNoPathRefreshes: 1,
          beforeMovement() { client.assert(); return null; } });
    }
    const target = c.snapshot.entities.find(e => e.objectId === candidate.objectId && !e.dead);
    if (!target || distance(self(c), target) !== 1 || !observer.snapshot.entities.some(e => e.objectId === target.objectId && !e.dead)) continue;
    const interval = Math.max(1600, nativeCadence(c.snapshot, 'attack', { movementIntervalMs: 750 }).intervalMs);
    const waitStarted = performance.now(), actionDeadline = waitStarted + 5000;
    while (true) {
      c.assert(); check(performance.now() < actionDeadline, 'Native preparation movement/status action lock did not clear within5s');
      const readiness = preparationAttackReadiness({ lastMovementAckAt: c.lastMovementAckAt, movementKind: c.lastMovementKind,
        poison: self(c)?.poison ?? c.snapshot?.playerPoison ?? 0, lastAttackAt: c.lastAttackAt, attackIntervalMs: interval });
      if (!c.pendingMovementAck && !selfActionBlockMask(c) && performance.now() >= readiness.readyAtMs) {
        await c.pace();
        const afterPace = preparationAttackReadiness({ lastMovementAckAt: c.lastMovementAckAt, movementKind: c.lastMovementKind,
          poison: self(c)?.poison ?? c.snapshot?.playerPoison ?? 0, lastAttackAt: c.lastAttackAt, attackIntervalMs: interval });
        if (!c.pendingMovementAck && !selfActionBlockMask(c) && performance.now() >= afterPace.readyAtMs) break;
      }
      await sleep(25);
    }
    context.emit({ kind: 'preparationNativeActionReady', actorOrdinal: c.ordinal, atUtc: at(), monotonicMs: performance.now(),
      waitedMs: performance.now()-waitStarted, lastMovementAckAgeMs: Number.isFinite(c.lastMovementAckAt) ? performance.now()-c.lastMovementAckAt : -1,
      movementCooldownMs: preparationAttackReadiness({ movementKind: c.lastMovementKind, poison: self(c)?.poison ?? c.snapshot?.playerPoison ?? 0 }).movementCooldownMs });
    const liveTarget = c.snapshot.entities.find(e => e.objectId === target.objectId && !e.dead);
    check(liveTarget && distance(self(c), liveTarget) === 1 && observer.snapshot.mapFileName === c.snapshot.mapFileName &&
      observer.snapshot.entities.some(e => e.objectId === target.objectId && !e.dead) &&
      Number.isInteger(observedPlayerId(observer, c.label)), 'Native preparation target/adjacency/AOI changed during action lock wait');
    const views = [c, observer], cursors = views.map(p => p.sequence), attackerIds = views.map(p => observedPlayerId(p, c.label));
    const sent = performance.now(); c.lastAttackAt = sent; c.send({ type: 'attack', objectId: target.objectId });
    try {
      const hit = await c.wait(() => verifiedCombatHit(...views.map((p, i) => p.events.filter(e => e.sequence > cursors[i] && e.monotonicMs >= sent)),
        target.objectId, ...attackerIds), 'ordered dual-observer positive PvE HP evidence', 2000);
      check(hit.monotonicMs - sent <= 1000, 'Positive PvE witness exceeded1000ms');
      return { actorOrdinal: c.ordinal, passed: true, targetObjectId: target.objectId, damage: hit.damage,
        healthPercent: hit.healthPercent, latencyMs: hit.monotonicMs - sent, observers: 2 };
    } catch (error) {
      // A target with an unresolved prior attempt is never used again for this actor.
      context.emit({ kind: 'pveTargetQuarantined', actorOrdinal: c.ordinal, objectId: target.objectId, atUtc: at(), monotonicMs: performance.now() });
      c.assert();
    }
  }
  throw new Error('No uniquely witnessed positive weak PvE hit within bounded ordinary candidates');
}

export async function runProbe(options) {
  const transportProfile = options.transportProfile ?? 'native-gzip'; probeTransportForActor(transportProfile, 0);
  await secureDirectory(options.privateDirectory); await fs.mkdir(options.output, { recursive: true });
  const lockFile = path.join(options.privateDirectory, '.cloud-reference.lock'); const lock = await fs.open(lockFile, 'wx', 0o600);
  const runId = Date.now() + '-' + crypto.randomBytes(3).toString('hex');
  const eventsFile = path.join(options.output, runId + '.events.jsonl'), reportFile = path.join(options.output, runId + '.report.json');
  const writer = new BoundedEvidence(eventsFile); const guard = new CapacityMonitor(options.monitor, writer);
  const report = { schema: 'mir2.cloud-reference-public-signed-probe.v2', runId, startedAtUtc: at(),
    capacityEvidence: false, ccuQualified: false, functionalOnly: true, ordinaryAccountsOnly: true,
    signedPublicGuestFlow: false, signedPublicWalletFlow: true, authMode: 'official-sui-wallet',
    signingScope: 'PersonalMessage only; no chain RPC, transactions, balances or production signing key',
    actorsRequested: options.count, activitySecondsRequested: options.seconds, transportProfileRequested: transportProfile, admissionTransportRecovery: [],
    combatRequired: options.combat === 'required',equipmentRoundtripRequested:options.equipmentRoundtrip==='on',equipmentRegression:[], movement: [], chat: [], pve: [], savedVerification: [], cleanup: [], ok: false };
  const context = { endpoint: options.endpoint, webBase: options.webBase, connection: 0, secrets: [],
    admissionTransportRecovery: report.admissionTransportRecovery, firstAttemptAdmissionOk: true,
    deadline: Date.now() + options.wallSeconds * 1000, ledgerFile: path.join(options.privateDirectory, 'auth-ledger.json'),
    actorsFile: path.join(options.privateDirectory, 'actors.private.json'),
    assert() { guard.assert(); check(Date.now() < this.deadline, 'Bounded functional wall deadline expired'); },
    emit(row) { check(writer.rows < 100000, 'Anonymous raw event bound exceeded'); writer.add(row); } };
  const clients = []; const live = []; let phase = 'monitor-preflight'; let timer;
  const stop = () => guard.stop('Operator requested functional probe drain'); process.on('SIGINT', stop); process.on('SIGTERM', stop);
  try {
    await guard.start(); timer = setTimeout(() => guard.stop('Bounded functional wall exhausted'), options.wallSeconds * 1000);
    let ledger;
    try { ledger = JSON.parse(await fs.readFile(context.ledgerFile, 'utf8')); } catch (error) { if (error.code !== 'ENOENT') throw error; }
    if(options.actorOrdinals!==undefined)check(ledger,'Explicit actor selection requires the existing real auth ledger');
    if (ledger) check(ledger.schemaVersion === 1 && ledger.purpose === 'cloud-reference-public-auth-budget' && ledger.endpoint === context.endpoint &&
      ledger.webBase === context.webBase && options['login-not-before'] === undefined, 'Existing real auth ledger cannot be replaced/overridden');
    const notBefore = ledger ? 0 : options['login-not-before'] === undefined ? Date.now() + LOGIN_WINDOW_MS : Date.parse(options['login-not-before']);
    check(Number.isFinite(notBefore), 'Invalid explicit auth window origin'); context.loginBudget = new LoginBudget(ledger?.attempts ?? [], notBefore);
    try { context.pool = JSON.parse(await fs.readFile(context.actorsFile, 'utf8')); } catch (error) {
      if (error.code !== 'ENOENT') throw error;
      check(options.actorOrdinals===undefined,'Explicit actor selection cannot create a new private pool');
      context.pool = { schema: 'mir2.cloud-reference-private-guests.v1', endpoint: context.endpoint, webBase: context.webBase,
        actors: Array.from({ length: options.count }, (_, ordinal) => ({ ordinal, name: 'Cg' + crypto.randomBytes(4).toString('hex') })) };
      await atomicPrivate(context.actorsFile, context.pool);
    }
    if (context.pool.schema === 'mir2.cloud-reference-private-guests.v1') {
      check(options.actorOrdinals===undefined,'Explicit actor selection requires the existing wallet actor pool');
      const archive = path.join(options.privateDirectory, 'actors.private.guest-v1.json');
      try { await fs.writeFile(archive, JSON.stringify(context.pool, null, 2) + '\n', { flag: 'wx', mode: 0o600 }); }
      catch (error) { if (error.code !== 'EEXIST') throw error; }
      context.pool = migrateGuestPool(context.pool); await atomicPrivate(context.actorsFile, context.pool);
    }
    check(context.pool.schema === 'mir2.cloud-reference-private-actors.v2' && context.pool.endpoint === context.endpoint && context.pool.webBase === context.webBase &&
      Array.isArray(context.pool.actors), 'Dedicated private cohort identity or endpoint changed');
    const selectedActors=selectProbeActors(context.pool,options.count,options.actorOrdinals);
    context.transportForActor = ordinal => probeTransportForActor(transportProfile, selectedActors[ordinal].originalOrdinal);
    report.actorOrdinalMap=selectedActors.map(({localOrdinal,originalOrdinal})=>({localOrdinal,originalOrdinal}));
    context.secrets.push(...context.pool.actors.flatMap(a => [a.cookie, a.accountId].filter(Boolean)));
    phase = 'ordinary-signed-admission';
    for (const {actor,localOrdinal,originalOrdinal} of selectedActors) { const c = new SignedClient(context.endpoint, actor, localOrdinal, context);c.originalOrdinal=originalOrdinal;clients.push(c); live.push(c); await login(c, context, 'admission'); }
    report.admittedActors = clients.length;
    phase = 'mutual-AOI';
    for (const c of clients) await c.wait(() => clients.filter(p => p !== c).every(p => Number.isInteger(observedPlayerId(c, p.label))), 'every ordinary peer AOI', 5000);
    report.mutualAoiPassed = true;
    const loadMap = id => loadProtocolCollisionMap(id, options['map-root'] ? { mapRoot: options['map-root'], packagedMapRoot: options['map-root'] } : {});
    phase = 'ordinary-equipment'; for (const c of clients) await equipStarterGear(c);
    if(options.equipmentRoundtrip==='on'){
      phase='ordinary-equipment-roundtrip';for(const c of clients){const result={actorOrdinal:c.ordinal,round:1,passed:false,steps:[]};
        report.equipmentRegression.push(result);await equipmentRoundtrip(c,result);}
    }
    const activityStart = performance.now(); report.activityStartedAtUtc = at();
    phase = 'ordinary-movement-and-chat';
    for (const c of clients) { report.movement.push(await socialMove(clients, c, loadMap, context)); report.chat.push(await chatAll(clients, c)); }
    if (options.combat === 'required') { phase = 'ordinary-weak-PvE'; for (const c of clients) report.pve.push(await weakPve(clients, c, loadMap, context)); }
    phase = 'ordinary-functional-observation';
    while (performance.now() - activityStart < options.seconds * 1000) {
      context.assert(); for (const c of clients) state(c); await sleep(100);
    }
    report.activityEndedAtUtc = at(); report.activityElapsedMs = performance.now() - activityStart;
    report.movementContinuousLoadTested = false; report.continuousCombatLoadTested = false;
    phase = 'normal-logout'; await sleep(11000);
    const expected = clients.map(c => state(c));
    for (const c of clients) { await c.close(); report.cleanup.push({ actorOrdinal: c.ordinal, normalLogout: !c.inGame && c.closed }); }
    phase = 'signed-relogin-persistence';
    for (const c of clients) {
      const again = new SignedClient(context.endpoint, c.actor, c.ordinal, context); again.originalOrdinal = c.originalOrdinal; live.push(again);
      await login(again, context, 'verification');
      check(JSON.stringify(state(again)) === JSON.stringify(expected[c.ordinal]), 'Logout/relogin changed exact transform/inventory/equipment');
      report.savedVerification.push({ actorOrdinal: c.ordinal, passed: true }); await again.close();
    }
    report.ok = report.movement.length === options.count && report.chat.length === options.count && report.savedVerification.length === options.count &&
      (options.combat === 'off' || report.pve.length === options.count);
  } catch (error) { report.failure = { phase, name: error.name, message: redact(error.message, context.secrets) }; }
  finally {
    clearTimeout(timer);
    for (const c of live) if (c.ws && !c.closed) { try { await c.close(); } catch (error) { report.ok = false; report.cleanup.push({ actorOrdinal: c.ordinal, normalLogout: false, failure: redact(error.message, context.secrets) }); } }
    for (const c of live) { clearInterval(c.timer); c.ws?.terminate(); }
    guard.close(); process.off('SIGINT', stop); process.off('SIGTERM', stop);
    report.bytes = live.reduce((sum, c) => ({ sent: sum.sent + c.bytes.sent, received: sum.received + c.bytes.received }), { sent: 0, received: 0 });
    report.bytesScope = 'WebSocket payload only; not TLS/IP or public NIC throughput';
    report.catalogTransport = { capability: CATALOG_GZIP_CAPABILITY, advertisedBeforeClientVersion: live.every(c => c.catalogGzipOptedIn),
      receivedFrames: live.reduce((sum, c) => sum + c.catalogFrames, 0), decodedEnvelopes: live.reduce((sum, c) => sum + c.catalogEnvelopes, 0) };
    report.bootstrapDiagnostics = live.map(c => ({ actorOrdinal: c.ordinal, connection: c.connection,
      ...(c.bootstrapProof ?? (c.bootstrapCursor !== undefined ? bootstrapEvidence(c, c.bootstrapCursor, c.actor.name) : {})),
      bootstrapSentAtMs: c.bootstrapSentAt ?? null,startGameTiming:c.startGameTiming??null }));
    report.logoutReceipts = live.map(c => c.logoutReceipt).filter(Boolean);
    report.webSocketClosingDiagnostics = live.flatMap(c => c.webSocketClosingDiagnostics ?? []);
    report.attemptedAdmissionActors = clients.length;
    report.completedAdmissionActors = clients.filter(c => c.bootstrapProof).length;
    report.firstAttemptAdmissionOk = context.firstAttemptAdmissionOk && clients.length > 0 && clients.every(c => c.bootstrapProof);
    report.completedAtUtc = at(); report.resourceStopReason = guard.reason ?? null;
    report.pveTested = report.pve.length === options.count; report.eventsFile = eventsFile;
    try { await writer.close(); } catch (error) { report.ok = false; report.evidenceFailure = redact(error.message, context.secrets); }
    report.eventRows = writer.rows; report.eventsSha256 = crypto.createHash('sha256').update(await fs.readFile(eventsFile)).digest('hex');
    report.sourceSha256 = crypto.createHash('sha256').update(await fs.readFile(new URL(import.meta.url))).digest('hex');
    report.authAdapterSourceSha256 = crypto.createHash('sha256').update(await fs.readFile(new URL('./public_sui_wallet_auth.mjs', import.meta.url))).digest('hex');
    await fs.writeFile(reportFile, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
    await lock.close(); await fs.unlink(lockFile);
  }
  return { reportFile, eventsFile, report };
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try { const result = await runProbe(optionsFrom(process.argv.slice(2))); console.log(JSON.stringify({ reportFile: result.reportFile,
    ok: result.report.ok, pveTested: result.report.pveTested, capacityEvidence: false, failure: result.report.failure ?? null })); process.exitCode = result.report.ok ? 0 : 1; }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
