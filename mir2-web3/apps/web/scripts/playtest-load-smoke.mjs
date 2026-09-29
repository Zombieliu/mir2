// Bounded ordinary-player load on an explicitly isolated realm. No admin, save
// edits, existing player accounts, automatic revival, or rate-limit bypasses.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';
import { PlaytestClient, authenticate, inventoryFingerprint, outsideRepository,
  redact, validateEndpoint } from './playtest-multiplayer-smoke.mjs';
import { findProtocolWalkPath, loadProtocolCollisionMap } from './quest-agent/protocol-navigation.mjs';
import { hasAuthoritativePlayerDeath } from './quest-agent/protocol-observation.mjs';
import { equipStarterGear } from './quest-agent/protocol-play.mjs';

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const self = client => client.snapshot?.entities?.find(entity => entity.objectId === client.snapshot.playerObjectId);
const SECRET = /password|secret|token|credential|authorization|cookie|assertion|private.?key/i;
const GAMEPLAY = new Set(['walk', 'run', 'turn', 'chat', 'attack']);
const ORDINARY_LOAD_MONSTERS = new Set(['Hen', 'Deer', 'Scarecrow', 'RakingCat', 'HookingCat']);
export class SafetyStop extends Error {}

export function ordinaryAdjacentTarget(entity, player) {
  // Guards are encoded as monsters too, but protected guards do not acknowledge
  // ordinary attacks. A generic kind check invalidated the first two-bot probe.
  return entity?.kind === 'monster' && ORDINARY_LOAD_MONSTERS.has(entity.name) &&
    !entity.dead && Number(entity.hp) > 0 && player != null &&
    Math.max(Math.abs(entity.x - player.x), Math.abs(entity.y - player.y)) === 1;
}

export function loadPolicy(options = {}) {
  const profile = options.profile ?? 'standard';
  if (!['standard', 'probe'].includes(profile)) throw new Error('Profile must be standard or probe');
  const stages = String(options.stages ?? (profile === 'probe' ? '1,2' : '1,2,3,5,8,12,15')).split(',').map(Number);
  if (!stages.length || stages.length > 7 || stages.some((n, i) => !Number.isInteger(n) || n < 1 ||
      n > (profile === 'probe' ? 2 : 15) || (i > 0 && n <= stages[i - 1]))) throw new Error('Stages must be increasing, bounded positive actor counts');
  const stageSeconds = Number(options.stageSeconds ?? (profile === 'probe' ? 30 : 90));
  const steadySeconds = Number(options.steadySeconds ?? (profile === 'probe' ? 0 : 300));
  const wallSeconds = Number(options.wallSeconds ?? (profile === 'probe' ? 240 : 1800));
  if (!Number.isInteger(stageSeconds) || stageSeconds < (profile === 'probe' ? 30 : 90) || stageSeconds > 120 ||
      !Number.isInteger(steadySeconds) || (profile === 'standard' && steadySeconds < 300) || steadySeconds < 0 || steadySeconds > 600 ||
      !Number.isInteger(wallSeconds) || wallSeconds < 60 || wallSeconds > (profile === 'probe' ? 300 : 3600)) throw new Error('Invalid bounded stage, steady-state or wall-clock duration');
  return { profile, stages, stageSeconds, steadySeconds, wallSeconds, commandsPerSecond: 3,
    movementP95Ms: 750, chatP95Ms: 1000, timeoutRatioExclusive: 0.01, monitorMaxAgeMs: 15000,
    capacityAcceptanceEligible: profile === 'standard' };
}

export function inspectMonitor(sample, now = Date.now()) {
  const at = typeof sample?.sampledAt === 'number' ? sample.sampledAt : Date.parse(sample?.sampledAt);
  if (!Number.isFinite(at) || now - at > 15000 || at - now > 5000) throw new SafetyStop('Monitor sample missing, stale over 15 seconds, or from the future');
  if (typeof sample.stop !== 'boolean') throw new SafetyStop('Monitor lacks an explicit stop flag');
  if (sample.stop) throw new SafetyStop(`Monitor requested drain: ${sample.reason ?? 'unspecified reason'}`);
  return { ...sample, sampledAtMs: at };
}

export class CommandBudget {
  constructor(clock = () => performance.now()) { this.clock = clock; this.sent = []; }
  remainingWait() {
    const now = this.clock(); this.sent = this.sent.filter(at => now - at < 1000);
    return this.sent.length < 3 ? 0 : Math.max(1, 1000 - (now - this.sent[0]));
  }
  reserve() {
    if (this.remainingWait()) throw new Error('Hard per-actor budget exceeded: at most 3 commands per second');
    this.sent.push(this.clock());
  }
}

export function percentile(values, p) {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  return sorted.length ? sorted[Math.max(0, Math.ceil(sorted.length * p) - 1)] : null;
}
export function metricSummary(samples) {
  const result = {};
  for (const kind of new Set(samples.map(sample => sample.kind))) {
    const group = samples.filter(sample => sample.kind === kind);
    const latency = group.filter(sample => ['success', 'corrected', 'miss'].includes(sample.status)).map(sample => sample.latencyMs);
    result[kind] = { count: group.length, status: Object.fromEntries([...new Set(group.map(sample => sample.status))]
      .map(status => [status, group.filter(sample => sample.status === status).length])),
    p50Ms: percentile(latency, 0.5), p95Ms: percentile(latency, 0.95), p99Ms: percentile(latency, 0.99) };
  }
  return result;
}
export function assessStage(samples, actorNames, seconds, policy) {
  const attempts = samples.filter(sample => GAMEPLAY.has(sample.kind) && !['skipped', 'cancelled'].includes(sample.status));
  const movement = attempts.filter(sample => ['walk', 'run'].includes(sample.kind) && ['success', 'corrected'].includes(sample.status));
  const chats = attempts.filter(sample => sample.kind === 'chat' && sample.status === 'success');
  const timeoutRatio = attempts.length ? attempts.filter(sample => sample.status === 'timeout').length / attempts.length : null;
  const movementP95Ms = percentile(movement.map(sample => sample.latencyMs), 0.95);
  const chatP95Ms = percentile(chats.map(sample => sample.latencyMs), 0.95);
  const activity = actorNames.map(actor => ({ actor,
    gameplayAcks: attempts.filter(sample => sample.actor === actor && ['success', 'corrected', 'miss'].includes(sample.status)).length,
    successfulMoves: movement.filter(sample => sample.actor === actor && sample.status === 'success').length }));
  const enoughActivity = activity.every(actor => actor.gameplayAcks >= Math.max(5, Math.floor(seconds / 8)) && actor.successfulMoves >= Math.max(3, Math.floor(seconds / 15)));
  const errors = attempts.filter(sample => ['rejected', 'transportError', 'error'].includes(sample.status)).length;
  const hits = attempts.filter(sample => sample.kind === 'attack' && sample.status === 'success' && sample.damage > 0);
  return { passed: movementP95Ms !== null && movementP95Ms <= policy.movementP95Ms &&
    chatP95Ms !== null && chatP95Ms <= policy.chatP95Ms && timeoutRatio !== null &&
    timeoutRatio < policy.timeoutRatioExclusive && enoughActivity && errors === 0,
  movementP95Ms, chatP95Ms, timeoutRatio, errors, enoughActivity, activity,
  peerChatSamples: chats.filter(sample => sample.receiver === 'peer').length,
  selfChatSamples: chats.filter(sample => sample.receiver === 'self').length,
  positiveCombatHits: hits.length,
  combatCoverage: hits.length ? 'positive-damage-observed' : 'not-covered', combatCapacityAccepted: false,
  combatPositiveHitMetrics: metricSummary(hits).attack ?? null,
  metrics: metricSummary(samples) };
}

export function positiveAttackReceipt(events, after, targetId, attackerId) {
  const fresh = events.filter(event => event.sequence > after && event.direction === 'received');
  const struck = fresh.find(event => event.packet === 'ObjectStruck' &&
    event.payload?.objectId === targetId && event.payload?.attackerId === attackerId);
  if (!struck) return null;
  // Both packet orders occur across owner/observer projection paths. Bind a
  // positive indicator to this actor's same-target strike in its local batch.
  const damage = fresh.find(event => event.packet === 'DamageIndicator' && event.payload?.objectId === targetId &&
    event.payload?.damage > 0 && Math.abs(event.sequence - struck.sequence) <= 3 &&
    Math.abs(event.monotonicMs - struck.monotonicMs) <= 100);
  if (!damage) return null;
  const otherStrike = fresh.some(event => event.packet === 'ObjectStruck' && event.payload?.objectId === targetId &&
    event.payload?.attackerId !== attackerId && Math.abs(event.sequence - damage.sequence) <= Math.abs(struck.sequence - damage.sequence) &&
    Math.abs(event.monotonicMs - damage.monotonicMs) <= 100);
  return otherStrike ? null : { ...damage, monotonicMs: Math.max(damage.monotonicMs, struck.monotonicMs), damage: damage.payload.damage };
}

export function eventsAfterSend(events, afterSequence, sentAt) {
  return events.filter(event => event.sequence > afterSequence && event.direction === 'received' && event.monotonicMs >= sentAt);
}

export function newLoadAccount(index, nonce = crypto.randomBytes(3).toString('hex')) {
  const suffix = `${nonce}${index.toString(36).padStart(2, '0')}`;
  return { accountId: `lt${suffix}`, name: `L${suffix}`, password: crypto.randomBytes(10).toString('hex'), state: 'planned', provisioning: 'ordinary-registration' };
}
export function validatePool(pool, endpoint) {
  if (pool?.schemaVersion !== 1 || pool.purpose !== 'isolated-load-actors' || pool.realm !== 'isolated' ||
      validateEndpoint(pool.endpoint) !== endpoint || !Array.isArray(pool.accounts) || pool.accounts.length < 1 || pool.accounts.length > 15) throw new Error('Invalid isolated load account pool');
  const ids = new Set(), names = new Set();
  for (const account of pool.accounts) {
    if (!/^lt[a-z0-9]{8,16}$/.test(account.accountId ?? '') || !/^L[a-z0-9]{8}$/.test(account.name ?? '') ||
        typeof account.password !== 'string' || account.password.length < 12 || account.password.length > 20 ||
        !['planned', 'registered', 'ready'].includes(account.state) ||
        (account.state === 'ready' && !Number.isInteger(account.characterIndex))) throw new Error('Pool must contain only dedicated ordinary load actors with valid private credentials');
    ids.add(account.accountId); names.add(account.name);
  }
  if (ids.size !== pool.accounts.length || names.size !== pool.accounts.length) throw new Error('Load actor identities must be unique');
  return pool;
}

async function securePoolPath(file) {
  const absolute = outsideRepository(file), directory = path.dirname(absolute);
  if (!(path.basename(directory) === 'private' || path.basename(directory).startsWith('load-private-'))) throw new Error('Use a dedicated outside-repository private or load-private-* directory');
  await fs.mkdir(directory, { recursive: true, mode: 0o700 });
  if (process.platform === 'win32') {
    const owner = execFileSync('whoami.exe', [], { encoding: 'utf8', windowsHide: true }).trim();
    execFileSync('icacls.exe', [directory, '/inheritance:r', '/grant:r', `${owner}:(OI)(CI)F`], { stdio: 'pipe', windowsHide: true });
  } else await fs.chmod(directory, 0o700);
  return absolute;
}
async function savePool(file, pool) {
  const temporary = `${file}.${crypto.randomBytes(4).toString('hex')}.tmp`;
  await fs.writeFile(temporary, JSON.stringify(pool, null, 2) + '\n', { mode: 0o600, flag: 'wx' });
  await fs.rename(temporary, file);
}

class EvidenceWriter {
  constructor(file) { this.file = file; this.pending = []; this.queue = Promise.resolve(); }
  add(value) { this.pending.push(JSON.stringify(value)); if (this.pending.length >= 20) this.flush(); }
  flush() {
    if (this.pending.length) {
      const lines = this.pending.splice(0).join('\n') + '\n';
      this.queue = this.queue.then(() => fs.appendFile(this.file, lines));
      this.queue.catch(error => { this.failure = error; });
    }
    return this.queue;
  }
}

export class MonitorGuard {
  constructor(file, writer, { read = file => fs.readFile(file, 'utf8'), clock = Date.now } = {}) {
    this.file = file; this.writer = writer; this.read = read; this.clock = clock; this.stopped = false; this.history = [];
  }
  stop(reason) { if (!this.stopped) { this.stopped = true; this.reason = reason; this.writer?.add({ type: 'safetyStop', at: this.clock(), reason }); } }
  async poll() {
    if (this.stopped) return;
    try {
      const raw = JSON.parse(await this.read(this.file));
      const sample = inspectMonitor(raw, this.clock());
      if (sample.sampledAtMs !== this.latest?.sampledAtMs) {
        this.latest = sample; this.history.push(sample); this.writer?.add({ type: 'monitor', sample });
      }
    } catch (error) { this.stop(error.message); }
  }
  assert() {
    if (this.writer?.failure) this.stop(`Evidence write failed: ${this.writer.failure.message}`);
    if (!this.stopped) { try { inspectMonitor(this.latest, this.clock()); } catch (error) { this.stop(error.message); } }
    if (this.stopped) throw new SafetyStop(this.reason);
  }
  async start() { await this.poll(); this.assert(); this.timer = setInterval(() => void this.poll(), 500); }
  close() { clearInterval(this.timer); }
  async afterAdmission(since) {
    const until = this.clock() + 8000;
    while (this.latest?.sampledAtMs <= since && this.clock() < until) { this.assert(); await sleep(100); }
    this.assert();
    if (this.latest.sampledAtMs <= since) { this.stop('No fresh monitor sample after actor admission'); this.assert(); }
  }
}

class LoadClient extends PlaytestClient {
  constructor(endpoint, account, context) {
    super(endpoint, account.name, '', [account.password]);
    this.context = context; this.account = account; this.budget = new CommandBudget(); this.bytes = { sent: 0, received: 0 };
  }
  record(direction, value) {
    const remember = input => {
      if (!input || typeof input !== 'object') return;
      for (const [key, item] of Object.entries(input)) {
        if (SECRET.test(key) && typeof item === 'string' && item && !this.secrets.includes(item)) this.secrets.push(item);
        else remember(item);
      }
    };
    remember(value);
    const safe = redact(value, this.secrets);
    const event = { ...safe, movementDirection: safe.direction, direction, sequence: ++this.sequence, monotonicMs: performance.now() };
    if (event.type === 'worldSnapshot') event.payload = { mapFileName: this.snapshot?.mapFileName, playerObjectId: this.snapshot?.playerObjectId };
    this.events.push(event); if (this.events.length > 3000) this.events.splice(0, 500);
    if (event.type === 'error') this.context.writer.add({ type: 'serverError', actor: this.label, event });
    return event;
  }
  send(command) {
    if (!this.draining) this.context.guard.assert();
    this.budget.reserve(); this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
    if (command.type === 'startGame') this.startGameSentAt = performance.now();
    super.send(command);
  }
  observe(message) {
    super.observe(message);
    if (message.packet === 'StartGame' && this.startGameSentAt !== undefined) {
      this.context.sample({ kind: 'startGame', actor: this.label, stage: this.context.stage,
        status: message.payload?.result === 4 ? 'success' : 'rejected', result: message.payload?.result,
        latencyMs: performance.now() - this.startGameSentAt });
      this.startGameSentAt = undefined;
    }
  }
  async pace() {
    while (this.budget.remainingWait()) { if (!this.draining) this.context.guard.assert(); await sleep(Math.min(this.budget.remainingWait(), 50)); }
  }
  async submit(command) { await this.pace(); const at = performance.now(); this.send(command); return at; }
  async wait(predicate, label, timeoutMs = 10000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (!this.draining) this.context.guard.assert();
      if (this.failure) throw this.failure;
      const result = predicate(); if (result) return result;
      if (this.closed) throw new Error(`${this.label} disconnected during ${label}`);
      await sleep(20);
    }
    throw new Error(`${this.label} timed out waiting for ${label}`);
  }
  async request(command, packet, timeoutMs = 10000) {
    await this.pace(); const at = performance.now();
    try {
      const result = await super.request(command, packet, timeoutMs);
      if (['login', 'newAccount', 'newCharacter'].includes(command.type)) this.context.sample({
        actor: this.label, kind: command.type, stage: this.context.stage,
        status: command.type === 'newAccount' && result.payload?.result !== 8 ? 'rejected' : 'success', latencyMs: performance.now() - at });
      return result;
    } catch (error) {
      if (['login', 'newAccount', 'newCharacter'].includes(command.type)) this.context.sample({
        actor: this.label, kind: command.type, stage: this.context.stage, status: 'rejected', latencyMs: performance.now() - at,
        error: redact(error.message, this.secrets) });
      throw error;
    }
  }
  async connect() {
    const connecting = super.connect();
    // Install before the opening handshake can emit its first incoming frame.
    this.ws.addEventListener('message', event => { this.bytes.received += Buffer.byteLength(event.data); });
    try { await connecting; } finally { clearInterval(this.timer); }
  }
  async refresh() { await this.pace(); return super.refresh(); }
  async logout() {
    if (!this.inGame) return null;
    const result = await this.request({ type: 'logOut' }, 'LogOutSuccess', 5000); this.inGame = false; return result;
  }
}

function fingerprint(client) {
  const player = self(client);
  return { mapFileName: client.snapshot?.mapFileName, name: player?.name, x: player?.x, y: player?.y,
    direction: player?.direction, inventory: inventoryFingerprint(client.snapshot) };
}
function movementPlan(client, map) {
  const player = self(client), anchor = client.anchor;
  const choices = [[1, 0, 'Right'], [0, 1, 'Down'], [-1, 0, 'Left'], [0, -1, 'Up']];
  for (let i = 0; i < choices.length; i++) {
    const [dx, dy, direction] = choices[(client.moveCursor + i) % choices.length];
    const cells = client.actions % 10 === 7 ? 2 : 1;
    const target = { x: player.x + dx * cells, y: player.y + dy * cells };
    if (Math.max(Math.abs(target.x - anchor.x), Math.abs(target.y - anchor.y)) > 6) continue;
    if ((client.snapshot.mapTransfers ?? []).some(transfer => {
      const b = transfer.bounds;
      return b && [1, cells].some(n => player.x + dx * n >= b.minX && player.x + dx * n <= b.maxX &&
        player.y + dy * n >= b.minY && player.y + dy * n <= b.maxY);
    })) continue;
    const route = findProtocolWalkPath({ map, start: player, target, maxExpanded: 300,
      dynamicObstacles: client.snapshot.entities.filter(entity => entity.objectId !== player.objectId && !entity.dead) });
    if (route?.length !== cells + 1) continue;
    if (cells === 2 && (route[1].x !== player.x + dx || route[1].y !== player.y + dy)) continue;
    client.moveCursor = (client.moveCursor + i + (client.actions % 3 === 0 ? 1 : 0)) % 4;
    return { type: cells === 2 ? 'run' : 'walk', direction, target, first: { x: player.x + dx, y: player.y + dy } };
  }
  return null;
}

async function activity(client, clients, context, mapCache) {
  const stage = context.stage, player = self(client);
  const now = Date.now(); let kind, command, receiver = client, metadata = {};
  if (!player || hasAuthoritativePlayerDeath(client.snapshot)) throw new Error('Actor died or lost authoritative presence; no automatic revival');
  if (now >= client.nextRtt) {
    kind = 'rtt'; command = { type: 'keepAlive', time: now }; client.nextRtt = now + 5000;
  } else if (now >= client.nextRefresh) {
    kind = 'refresh'; command = { type: 'clientVersion' }; client.nextRefresh = now + 12000;
  } else if (now >= client.nextChat) {
    kind = 'chat'; command = { type: 'chat', message: `load ${context.runId.slice(-8)} ${client.label} ${client.actions}` };
    client.nextChat = now + 20000;
    receiver = clients.find(other => other !== client && other.inGame && !other.stopping &&
      other.snapshot?.mapFileName === client.snapshot.mapFileName && other.snapshot.entities.some(entity => entity.name === client.label)) ?? client;
    metadata.receiver = receiver === client ? 'self' : 'peer'; metadata.receiverActor = receiver.label;
  } else {
    const target = client.snapshot.entities.find(entity => ordinaryAdjacentTarget(entity, player));
    if (target && now >= client.nextAttack) {
      kind = 'attack'; command = { type: 'attack', objectId: target.objectId }; client.nextAttack = now + 3500; metadata.targetId = target.objectId;
    } else if (client.actions % 7 === 5) {
      kind = 'turn'; command = { type: 'turn', direction: player.direction === 'Up' ? 'Right' : 'Up' };
    } else {
      const mapName = String(client.snapshot.mapFileName);
      if (!mapCache.has(mapName)) mapCache.set(mapName, loadProtocolCollisionMap(mapName, context.mapRoot ? { mapRoot: context.mapRoot } : {}));
      const plan = movementPlan(client, await mapCache.get(mapName));
      if (plan) { kind = plan.type; command = { type: plan.type, direction: plan.direction }; metadata = { target: plan.target, first: plan.first }; }
      else { kind = 'turn'; command = { type: 'turn', direction: player.direction === 'Up' ? 'Right' : 'Up' }; metadata.navigationBlocked = true; }
    }
  }
  await client.pace();
  const after = client.sequence, receiverAfter = receiver.sequence, at = performance.now();
  client.send(command); client.actions++;
  try {
    const receipt = await receiver.wait(() => {
      const ownFresh = eventsAfterSend(client.events, after, at);
      const error = ownFresh.find(event => event.type === 'error'); if (error) throw new Error(`Rejected: ${error.message ?? error.code}`);
      const fresh = eventsAfterSend(receiver.events, receiverAfter, at);
      if (kind === 'rtt') return fresh.find(event => event.packet === 'KeepAlive' && String(event.payload?.time) === String(command.time));
      if (kind === 'refresh') return fresh.find(event => event.type === 'worldSnapshot');
      if (kind === 'chat') return fresh.find(event => ['ObjectChat', 'Chat'].includes(event.packet) && String(event.payload?.text ?? event.payload?.message ?? '').includes(command.message));
      if (kind === 'attack') {
        return positiveAttackReceipt(fresh, after, command.objectId, player.objectId);
      }
      return fresh.find(event => event.packet === 'UserLocation');
    }, `${kind} correlated acknowledgement`, kind === 'attack' ? 1500 : 2000);
    let status = 'success';
    if (['walk', 'run'].includes(kind)) {
      const location = receipt.payload;
      const atTarget = location?.x === metadata.target.x && location?.y === metadata.target.y;
      const degraded = kind === 'run' && location?.x === metadata.first.x && location?.y === metadata.first.y;
      if (!atTarget && !degraded) status = 'corrected';
      metadata.actual = { x: location?.x, y: location?.y }; metadata.degradedRun = degraded && !atTarget;
    } else if (kind === 'turn' && receipt.payload?.direction !== command.direction) status = 'corrected';
    context.sample({ actor: client.label, stage, kind, status, latencyMs: receipt.monotonicMs - at,
      receipt: receipt.sequence, damage: receipt.damage, ...metadata });
    if (GAMEPLAY.has(kind)) client.lastGameplayAck = Date.now();
  } catch (error) {
    const swung = kind === 'attack' && eventsAfterSend(client.events, after, at).find(event => event.packet === 'ObjectAttack' && event.payload?.objectId === player.objectId);
    const status = error instanceof SafetyStop ? 'cancelled' : swung ? 'miss' : /timed out/.test(error.message) ? 'timeout' :
      /Rejected/.test(error.message) ? 'rejected' : client.closed ? 'transportError' : 'error';
    context.sample({ actor: client.label, stage, kind, status, latencyMs: swung ? swung.monotonicMs - at : performance.now() - at,
      error: redact(error.message, client.secrets), ...metadata });
    if (swung) client.lastGameplayAck = Date.now();
    if (error instanceof SafetyStop || client.closed) throw error;
  }
}

async function actorLoop(client, clients, context, mapCache) {
  try {
    while (!client.stopping) {
      context.guard.assert(); const started = Date.now();
      await activity(client, clients, context, mapCache);
      context.peakActiveBots = Math.max(context.peakActiveBots, clients.filter(actor => actor.inGame && !actor.stopping && Date.now() - actor.lastGameplayAck <= 5000).length);
      await sleep(Math.max(20, 500 - (Date.now() - started)));
    }
  } catch (error) {
    if (!(error instanceof SafetyStop)) context.guard.stop(`Actor ${client.label}: ${redact(error.message, client.secrets)}`);
  }
}

async function drain(client, context) {
  client.stopping = true;
  await client.loop;
  client.draining = true;
  let saved;
  try {
    if (client.inGame && !client.closed) { await client.refresh(); saved = fingerprint(client); }
    await client.close();
    context.writer.add({ type: 'actorDrained', actor: client.label, normalLogoutAttempted: Boolean(saved), saved });
  } catch (error) {
    context.cleanupErrors.push({ actor: client.label, error: redact(error.message, client.secrets) });
    try { await client.close(); } catch {}
  }
  return saved;
}

export async function runLoad(options) {
  if (!['prepare', 'run'].includes(options.mode) || options.realm !== 'isolated') throw new Error('--mode prepare|run and --realm isolated are required');
  if (!options.monitor || !options.accountsFile || !options.output) throw new Error('--monitor, --accounts-file and --output are required');
  const endpoint = validateEndpoint(options.endpoint), policy = loadPolicy(options);
  const output = outsideRepository(options.output); await fs.mkdir(output, { recursive: true });
  const runId = `${Date.now()}-${crypto.randomBytes(4).toString('hex')}`;
  const writer = new EvidenceWriter(path.join(output, `${runId}.samples.jsonl`));
  const guard = new MonitorGuard(path.resolve(options.monitor), writer);
  const samples = [], clients = [], saved = [], mapCache = new Map();
  const context = { runId, writer, guard, mapRoot: options.mapRoot, stage: 'prepare', peakActiveBots: 0, cleanupErrors: [],
    sample(sample) { samples.push(sample); writer.add({ type: 'action', at: Date.now(), ...sample }); } };
  const report = { schemaVersion: 1, runId, endpoint, mode: options.mode, startedAt: new Date().toISOString(), policy,
    monitorPath: path.resolve(options.monitor), stages: [], highestPassedStage: 0, highestFiveMinuteStableBots: null,
    capacityScope: 'ordinary movement, turning and chat at the current deployed admission cap', combatCapacityAccepted: false,
    capacityUpperBound: null, humanAccountsControlled: false, preparationStartsGame: false,
    twoPhysicalComputersTested: false, nativeVisualAccepted: false };
  let pool, accountPath, timer;
  const makeClient = account => { const client = new LoadClient(endpoint, account, context); clients.push(client); return client; };
  const runStage = async (target, seconds, name) => {
    context.stage = name; const started = Date.now();
    writer.add({ type: 'stageStarted', stage: name, targetControlledBots: target, seconds, at: started });
    options.onProgress?.({ stage: name, targetControlledBots: target, status: 'started' });
    while (Date.now() - started < seconds * 1000) { guard.assert(); await sleep(100); }
    const activeNames = clients.filter(client => client.inGame && !client.stopping).map(client => client.label);
    const assessment = assessStage(samples.filter(sample => sample.stage === name), activeNames, seconds, policy);
    assessment.passed &&= activeNames.length === target;
    const result = { name, controlledBots: target, seconds, completed: true, ...assessment };
    report.stages.push(result); writer.add({ type: 'stageCompleted', ...result });
    options.onProgress?.({ stage: name, status: assessment.passed ? 'passed' : 'failed', movementP95Ms: assessment.movementP95Ms, chatP95Ms: assessment.chatP95Ms, timeoutRatio: assessment.timeoutRatio });
    return assessment.passed;
  };
  try {
    await guard.start(); report.baselineServerActive = guard.latest.health?.currentActiveSessions ?? null;
    timer = setTimeout(() => guard.stop('Bounded wall-clock budget exhausted'), policy.wallSeconds * 1000);
    accountPath = await securePoolPath(options.accountsFile);
    try { pool = validatePool(JSON.parse(await fs.readFile(accountPath, 'utf8')), endpoint); }
    catch (error) {
      if (error.code !== 'ENOENT' || options.mode !== 'prepare') throw error;
      const count = Number(options.count ?? 2);
      if (!Number.isInteger(count) || count < 1 || count > 15) throw new Error('Prepare count must be 1–15');
      const nonce = crypto.randomBytes(3).toString('hex');
      pool = { schemaVersion: 1, purpose: 'isolated-load-actors', endpoint, realm: 'isolated', createdAt: new Date().toISOString(),
        accounts: Array.from({ length: count }, (_, index) => newLoadAccount(index, nonce)) };
      await savePool(accountPath, pool);
    }
    report.accountIds = pool.accounts.map(account => account.accountId);
    report.accountProvisioning = pool.accounts.map(account => ({ accountId: account.accountId,
      source: account.provisioning ?? 'unspecified-existing-private-pool' }));
    if (options.mode === 'prepare') {
      for (const account of pool.accounts) {
        if (account.state === 'ready') continue;
        guard.assert(); const client = makeClient(account); await client.connect();
        if (account.state === 'planned') {
          const registration = await client.request({ type: 'newAccount', accountId: account.accountId, password: account.password }, 'NewAccount');
          if (registration.payload?.result !== 8) throw new Error(`Ordinary registration rejected (${registration.payload?.result}); preparation stopped without retries`);
          account.state = 'registered'; await savePool(accountPath, pool);
        }
        const login = await client.request({ type: 'login', accountId: account.accountId, password: account.password }, 'LoginSuccess');
        let character = login.payload?.characters?.find(value => value.name === account.name);
        if (login.payload?.characters?.length && !character) throw new Error('Dedicated account unexpectedly contains an unrelated character');
        if (!character) character = (await client.request({ type: 'newCharacter', name: account.name, class: 'Warrior', gender: 'Male' }, 'NewCharacterSuccess')).payload?.character;
        if (!Number.isInteger(character?.index)) throw new Error('Ordinary character creation was not acknowledged');
        account.characterIndex = character.index; account.state = 'ready'; await savePool(accountPath, pool);
        client.draining = true; await client.close();
        options.onProgress?.({ mode: 'prepare', ready: pool.accounts.filter(value => value.state === 'ready').length, requested: pool.accounts.length });
      }
      report.readyAccounts = pool.accounts.filter(account => account.state === 'ready').length;
    } else {
      const accounts = pool.accounts.filter(account => account.state === 'ready');
      const stages = policy.stages.filter(count => count <= accounts.length);
      report.unattemptedForMissingAccounts = policy.stages.filter(count => count > accounts.length);
      if (!stages.length) throw new Error('No prepared ordinary accounts for the first stage; run prepare separately');
      for (const target of stages) {
        context.stage = `ramp-${target}`;
        while (clients.filter(client => client.inGame && !client.stopping).length < target) {
          guard.assert(); const account = accounts[clients.length], client = makeClient(account);
          await client.connect(); const start = performance.now(); const actor = await authenticate(client, account, false);
          if (actor.characterIndex !== account.characterIndex) throw new Error('StartGame selected an unexpected character');
          context.sample({ kind: 'bootstrap', actor: client.label, stage: context.stage, status: 'success', latencyMs: performance.now() - start });
          await equipStarterGear(client);
          client.anchor = { x: actor.x, y: actor.y }; client.actions = 0; client.moveCursor = clients.length % 4;
          client.nextRtt = Date.now(); client.nextRefresh = Date.now() + 12000;
          client.nextChat = Date.now() + 3000 + clients.length * 350; client.nextAttack = Date.now() + 1000;
          client.loop = actorLoop(client, clients, context, mapCache);
          await guard.afterAdmission(Date.now());
        }
        if (!await runStage(target, policy.stageSeconds, `stage-${target}`)) break;
        report.highestPassedStage = target;
      }
      if (policy.steadySeconds && report.highestPassedStage > 0) {
        const live = clients.filter(client => client.inGame && !client.stopping);
        for (const client of live.slice(report.highestPassedStage)) saved.push({ client, state: await drain(client, context) });
        if (await runStage(report.highestPassedStage, policy.steadySeconds, `steady-${report.highestPassedStage}`) && policy.steadySeconds >= 300) report.highestFiveMinuteStableBots = report.highestPassedStage;
      }
    }
  } catch (error) {
    report.error = redact(error.message, pool?.accounts.map(account => account.password) ?? []);
    report.interruptedStage = context.stage;
    if (context.stage.startsWith('stage-') || context.stage.startsWith('steady-')) report.stages.push({ name: context.stage, completed: false, reason: report.error });
  } finally {
    context.stage = 'drain';
    // Stop all actor loops before awaiting any one actor's normal logout.
    for (const client of clients) client.stopping = true;
    await Promise.all(clients.filter(client => !client.closed).map(async client => saved.push({ client, state: await drain(client, context) })));
    if (options.mode === 'run' && !guard.stopped && !context.cleanupErrors.length && !report.error) {
      report.sampledRelogin = [];
      for (const prior of saved.filter(item => item.state).slice(0, 2)) {
        const client = new LoadClient(endpoint, prior.client.account, context); clients.push(client);
        try {
          guard.assert(); await client.connect(); await authenticate(client, client.account, false);
          const actual = fingerprint(client);
          if (JSON.stringify(actual) !== JSON.stringify(prior.state)) throw new Error('Sampled relogin transform/inventory differs from normal logout');
          report.sampledRelogin.push({ actor: client.label, status: 'passed', state: actual });
        } catch (error) { report.sampledRelogin.push({ actor: client.label, status: 'failed', error: redact(error.message, client.secrets) }); }
        finally { client.draining = true; try { await client.close(); } catch (error) { context.cleanupErrors.push({ actor: client.label, error: redact(error.message, client.secrets) }); } }
      }
    } else if (options.mode === 'run') report.sampledRelogin = { status: 'skipped', reason: 'Safety stop, earlier failure, or incomplete drain prohibits adding load' };
    clearTimeout(timer); guard.close();
    report.safetyStopped = guard.stopped; report.safetyReason = guard.reason; report.cleanupErrors = context.cleanupErrors;
    report.peakActiveBots = context.peakActiveBots;
    report.peakObservedServerActive = Math.max(report.baselineServerActive ?? 0, ...guard.history.map(sample => Number(sample.health?.currentActiveSessions ?? 0)));
    report.bytes = clients.reduce((total, client) => ({ sent: total.sent + client.bytes.sent, received: total.received + client.bytes.received }), { sent: 0, received: 0 });
    report.bytesDefinition = 'WebSocket JSON payload bytes, excluding TLS/framing/transport overhead';
    report.metrics = metricSummary(samples); report.finishedAt = new Date().toISOString();
    report.ok = !report.error && !guard.stopped && !context.cleanupErrors.length &&
      (options.mode === 'prepare' ? report.readyAccounts === pool?.accounts.length : report.stages.length > 0 &&
        report.stages.every(stage => stage.completed && stage.passed) && !report.sampledRelogin?.some?.(sample => sample.status === 'failed'));
    report.capacityAcceptancePassed = Boolean(report.ok && policy.capacityAcceptanceEligible && report.highestFiveMinuteStableBots);
    const secrets = clients.flatMap(client => client.secrets).concat(pool?.accounts.map(account => account.password) ?? []);
    writer.add({ type: 'finished', ok: report.ok, safetyStopped: report.safetyStopped });
    try { await writer.flush(); } catch (error) { report.ok = false; report.capacityAcceptancePassed = false; report.evidenceError = redact(error.message, secrets); }
    report.reportPath = path.join(output, `${runId}.report.json`);
    await fs.writeFile(report.reportPath, JSON.stringify(redact(report, secrets), null, 2) + '\n');
  }
  return report;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const keys = { '--mode': 'mode', '--endpoint': 'endpoint', '--realm': 'realm', '--accounts-file': 'accountsFile',
    '--monitor': 'monitor', '--output': 'output', '--count': 'count', '--profile': 'profile', '--stages': 'stages',
    '--stage-seconds': 'stageSeconds', '--steady-seconds': 'steadySeconds', '--wall-seconds': 'wallSeconds', '--map-root': 'mapRoot' };
  try {
    if (process.argv.includes('--help')) console.log('Required: --mode prepare|run --endpoint WSS --realm isolated --accounts-file OUTSIDE_REPO/private/accounts.json --monitor monitor.json --output REPORTS. Prepare: --count 2. Short diagnostic: --profile probe --stages 1 --stage-seconds 30 --steady-seconds 0. Standard default: 1,2,3,5,8,12,15 actors, 90s stages, 300s final steady; use --stages 1,2,3,5,8,12,14 when one human remains online.');
    else {
      const options = {};
      for (let i = 2; i < process.argv.length; i += 2) {
        if (!keys[process.argv[i]] || !process.argv[i + 1]) throw new Error('Unknown or missing argument; use --help');
        options[keys[process.argv[i]]] = process.argv[i + 1];
      }
      const report = await runLoad({ ...options, onProgress: value => console.log(JSON.stringify(value)) });
      console.log(JSON.stringify({ ok: report.ok, reportPath: report.reportPath, safetyStopped: report.safetyStopped,
        error: report.error, highestPassedStage: report.highestPassedStage, highestFiveMinuteStableBots: report.highestFiveMinuteStableBots,
        peakActiveBots: report.peakActiveBots, peakObservedServerActive: report.peakObservedServerActive }, null, 2));
      if (!report.ok) process.exitCode = 1;
    }
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
