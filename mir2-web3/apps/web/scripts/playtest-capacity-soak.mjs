// Opt-in, ordinary-player capacity/soak driver for one isolated realm. This
// script never registers accounts, changes admission policy, or edits saves.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { monitorEventLoopDelay, performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';
import { PlaytestClient, outsideRepository, validateEndpoint, observedPlayerId } from './playtest-multiplayer-smoke.mjs';
import { NativeResumeClient, validateResumeControl, assertResumeEvidence, assertReplayRejected, resumeFingerprint } from './playtest-native-resume-smoke.mjs';
import { CommandBudget, SafetyStop } from './playtest-load-smoke.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';
import { hasAuthoritativePlayerDeath } from './quest-agent/protocol-observation.mjs';
import { selfActionBlockMask } from './quest-agent/protocol-status.mjs';
import { equipStarterGear } from './quest-agent/protocol-play.mjs';
import { capacityPolicy, validateCapacityPool, LoginBudget, LOGIN_WINDOW_MS, Measurements, assessMeasurements,
  AoiCoverage, ContinuousAcceptance, verifiedCombatHit, BoundedEvidence, CapacityMonitor, memoryTrend, self, distance, sleep, sanitize, fingerprint } from './playtest-capacity-soak-core.mjs';

const STEPS = [[0, -1, 'Up'], [1, -1, 'UpRight'], [1, 0, 'Right'], [1, 1, 'DownRight'],
  [0, 1, 'Down'], [-1, 1, 'DownLeft'], [-1, 0, 'Left'], [-1, -1, 'UpLeft']];
const WEAK_MONSTERS = ['Scarecrow', 'Hen', 'Deer', 'HookingCat', 'RakingCat'];
const fresh = (client, cursor, at) => client.events.filter(event => event.direction === 'received' && event.sequence > cursor && event.monotonicMs >= at);
const packet = (client, after, name, predicate = () => true) => client.events.find(event => event.direction === 'received' && event.sequence > after && event.packet === name && predicate(event.payload));
const inside = (point, bounds) => bounds && point.x >= bounds.minX && point.x <= bounds.maxX && point.y >= bounds.minY && point.y <= bounds.maxY;

async function privateFile(file) {
  const absolute = outsideRepository(file), directory = path.dirname(absolute);
  if (!(path.basename(directory) === 'private' || path.basename(directory).startsWith('load-private-'))) throw new Error('Account pool must use an outside-repository private or load-private-* directory');
  if (process.platform === 'win32') {
    const owner = execFileSync('whoami.exe', [], { encoding: 'utf8', windowsHide: true }).trim();
    execFileSync('icacls.exe', [directory, '/inheritance:r', '/grant:r', `${owner}:(OI)(CI)F`], { stdio: 'pipe', windowsHide: true });
  } else await fs.chmod(directory, 0o700);
  return absolute;
}
async function writePrivate(file, data) {
  const temporary = `${file}.${crypto.randomBytes(4).toString('hex')}.tmp`;
  await fs.writeFile(temporary, JSON.stringify(data, null, 2) + '\n', { flag: 'wx', mode: 0o600 });
  for (let attempt = 0; ; attempt++) {
    try { await fs.rename(temporary, file); break; }
    catch (error) { if (!['EPERM', 'EACCES'].includes(error.code) || attempt >= 10) throw error; await sleep(10); }
  }
}

export function matchesFanoutReceipt(event, kind, command, objectId, actual) {
  if (kind === 'chat') return ['ObjectChat', 'Chat'].includes(event.packet) && String(event.payload?.text ?? event.payload?.message ?? '').includes(command.message);
  if (event.payload?.objectId !== objectId) return false;
  if (kind === 'turn') return event.packet === 'ObjectTurn' && event.payload.direction === command.direction;
  const location = event.payload.location ?? event.payload;
  return ['ObjectWalk', 'ObjectRun'].includes(event.packet) && location.x === actual.x && location.y === actual.y;
}
export function actionFailureStatus(error, swung, closed) {
  if (error instanceof SafetyStop) return 'cancelled';
  if (error.code === 'PUBLIC_COMMAND_REJECTED') return 'rejected';
  if (closed) return 'transportError';
  if (swung && /timed out/.test(error.message)) return 'miss';
  return /timed out/.test(error.message) ? 'timeout' : 'error';
}

export class CapacityClient extends NativeResumeClient {
  constructor(endpoint, account, context) {
    super(endpoint, account.name, null, [account.password]);
    this.account = account; this.context = context;
    context.accountBudgets ??= new Map();
    if (!context.accountBudgets.has(account.accountId)) context.accountBudgets.set(account.accountId, new CommandBudget());
    this.budget = context.accountBudgets.get(account.accountId);
    this.bytes = { sent: 0, received: 0 }; this.recentSecrets = []; this.movementCount = 0; this.targetCursor = 0; this.failedTargets = new Map();
  }
  record(direction, value) {
    // Never retain worldSnapshot arrays in event history, nor an ever-growing
    // list of rotated tickets. The latest native ticket remains memory-only.
    const compact = value.type === 'worldSnapshot' ? { type: value.type, payload: { mapFileName: this.snapshot?.mapFileName,
      playerObjectId: this.snapshot?.playerObjectId, entities: self(this) ? [{ ...self(this) }] : [] } } : value;
    const safe = sanitize(compact, this.secrets.concat(this.recentSecrets));
    const event = { ...safe, movementDirection: safe.direction, direction, sequence: ++this.sequence, monotonicMs: performance.now() };
    if (event.type === 'worldSnapshot') event.payload = { mapFileName: this.snapshot?.mapFileName,
      playerObjectId: this.snapshot?.playerObjectId, entities: self(this) ? [{ ...self(this) }] : [] };
    this.events.push(event); if (this.events.length > 2048) this.events.splice(0, 256);
    if (event.type === 'error') this.context.writer.add({ type: 'serverError', actor: this.label, event });
    return event;
  }
  observe(message) {
    if (message.type === 'resumeCredential' && typeof message.credential === 'string') {
      this.recentSecrets.push(message.credential); this.recentSecrets = this.recentSecrets.slice(-3);
    }
    super.observe(message);
  }
  send(command) {
    if (!this.draining) this.context.guard.assert();
    this.budget.reserve(); this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
    PlaytestClient.prototype.send.call(this, command);
  }
  control(command) {
    validateResumeControl(command);
    if (!this.draining) this.context.guard.assert();
    this.budget.reserve(); this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
    this.record('sent', command); this.ws.send(JSON.stringify(command));
  }
  async pace() {
    while (this.budget.remainingWait()) {
      if (!this.draining) this.context.guard.assert();
      await sleep(Math.min(50, this.budget.remainingWait()));
    }
  }
  async wait(predicate, label, timeoutMs = 15000) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (!this.draining) this.context.guard.assert();
      if (this.failure) throw this.failure;
      const result = predicate(); if (result) return result;
      if (this.closed) throw new Error(`${this.label} transport closed during ${label}`);
      await sleep(20);
    }
    throw new Error(`${this.label} timed out waiting for ${label}`);
  }
  async request(command, name, timeoutMs = 15000) {
    await this.pace(); const started = performance.now();
    try {
      const receipt = await PlaytestClient.prototype.request.call(this, command, name, timeoutMs);
      if (['login', 'newCharacter', 'startGame', 'logOut'].includes(command.type)) this.context.add({ actor: this.label,
        kind: command.type, status: 'success', latencyMs: performance.now() - started });
      return receipt;
    } catch (error) {
      if (['login', 'newCharacter', 'startGame', 'logOut'].includes(command.type)) this.context.add({ actor: this.label,
        kind: command.type, status: 'rejected', latencyMs: performance.now() - started, error: sanitize(error.message, this.secrets) });
      throw error;
    }
  }
  async connect() {
    const promise = super.connect();
    this.ws.addEventListener('message', event => { this.bytes.received += Buffer.byteLength(event.data); });
    try { await promise; } finally { clearInterval(this.timer); }
    await this.pace(); this.control({ type: 'clientCapabilities', capabilities: ['nativeResumeV1'] });
  }
  async refresh() { await this.pace(); return PlaytestClient.prototype.refresh.call(this); }
  async logout() {
    if (!this.inGame) return null;
    const ack = await this.request({ type: 'logOut' }, 'LogOutSuccess', 5000); this.inGame = false; return ack;
  }
}

// Anchors are reachable static cells chosen once. Every actual step still uses
// public Walk/Run with current occupancy checks; these are never teleports.
export function chooseHomes(map, origin, count, layout, transfers = [], explicit = []) {
  const blockedTransfer = point => transfers.some(transfer => inside(point, transfer.bounds));
  const queue = [{ x: origin.x, y: origin.y }], seen = new Set([`${origin.x},${origin.y}`]), reachable = [];
  for (let cursor = 0; cursor < queue.length && cursor < 40000; cursor++) {
    const point = queue[cursor];
    if (protocolMapCellIsWalkable(map, point) && !blockedTransfer(point)) reachable.push(point);
    for (const [dx, dy] of STEPS.filter(([x, y]) => !x || !y)) {
      const next = { x: point.x + dx, y: point.y + dy }, key = `${next.x},${next.y}`;
      if (seen.has(key) || distance(next, origin) > 96 || !protocolMapCellIsWalkable(map, next) || blockedTransfer(next)) continue;
      seen.add(key); queue.push(next);
    }
  }
  const width = Math.ceil(Math.sqrt(count)), spacing = layout === 'hotspot' ? 2 : 6, used = new Set();
  return Array.from({ length: count }, (_, index) => {
    const wanted = explicit[index] ?? { x: origin.x + (index % width - (width - 1) / 2) * spacing,
      y: origin.y + (Math.floor(index / width) - (width - 1) / 2) * spacing };
    let selected, best = Infinity;
    for (const point of reachable) {
      if (used.has(`${point.x},${point.y}`)) continue;
      const score = distance(point, wanted) * 1000 + distance(point, origin);
      if (score >= best) continue;
      if (STEPS.filter(([dx, dy]) => !dx || !dy).filter(([dx, dy]) =>
        protocolMapCellIsWalkable(map, { x: point.x + dx, y: point.y + dy }) && !blockedTransfer({ x: point.x + dx, y: point.y + dy })).length < 2) continue;
      selected = point; best = score;
    }
    if (!selected || (explicit[index] && distance(selected, explicit[index]) > 2)) throw new Error('Scenario lacks enough reachable patrol anchors');
    used.add(`${selected.x},${selected.y}`); return selected;
  });
}

export function directionOf(from, to) {
  const dx = to.x - from.x, dy = to.y - from.y;
  return STEPS.find(([x, y]) => x === dx && y === dy)?.[2] ?? null;
}
function movement(client, target, map) {
  const owner = self(client), occupied = client.snapshot.entities.filter(entity => entity.objectId !== owner.objectId && !entity.dead);
  const route = findProtocolWalkPath({ map, start: owner, target, dynamicObstacles: occupied, maxExpanded: 4000 });
  if (!route || route.length < 2) return null;
  const first = route[1];
  if ((client.snapshot.mapTransfers ?? []).some(transfer => inside(first, transfer.bounds))) return null;
  const direction = directionOf(owner, first); if (!direction) return null;
  if (client.movementCount % 10 === 9 && route.length > 2 && route[2].x - first.x === first.x - owner.x && route[2].y - first.y === first.y - owner.y &&
      !(client.snapshot.mapTransfers ?? []).some(transfer => inside(route[2], transfer.bounds))) {
    return { command: { type: 'run', direction }, target: route[2], first };
  }
  return { command: { type: 'walk', direction }, target: first, first };
}
function patrol(client, context) {
  const owner = self(client), map = context.map;
  if (distance(owner, client.home) > 2) return movement(client, client.home, map);
  const points = [[2, 0], [2, 2], [0, 2], [0, 0], [-1, 0], [0, -1]];
  for (let offset = 0; offset < points.length; offset++) {
    const index = (client.targetCursor + offset) % points.length, [dx, dy] = points[index];
    const target = { x: client.home.x + dx, y: client.home.y + dy };
    if (distance(owner, target) === 0) { client.targetCursor = (index + 1) % points.length; continue; }
    const plan = movement(client, target, map);
    if (plan) { client.targetCursor = index; return plan; }
  }
  return null;
}
function nearbyObservers(client, context, margin = 16) {
  return context.clients.filter(other => other !== client && other.inGame && !other.closed && !other.paused &&
    other.snapshot?.mapFileName === client.snapshot.mapFileName && self(other) && distance(self(other), self(client)) <= margin);
}
export function combatCandidates(client, context, now = Date.now()) {
  const owner = self(client);
  return client.snapshot.entities.filter(entity => entity.kind === 'monster' && !entity.dead && Number(entity.hp ?? 1) > 0 &&
    String(entity.disposition ?? '').trim().toLowerCase() !== 'friendly' && !String(entity.ownerName ?? '').trim() &&
    WEAK_MONSTERS.includes(entity.name) && context.monsterNames.includes(entity.name) && distance(entity, owner) <= 16 &&
    (client.failedTargets.get(entity.objectId)?.until ?? 0) <= now &&
    (!context.targetClaims.has(entity.objectId) || context.targetClaims.get(entity.objectId) === client.label))
    .sort((a, b) => distance(a, owner) - distance(b, owner));
}

export function stableFanoutRange(kind) {
  // Both owners may already have a two-cell Run in flight. Complete AOI
  // membership is still checked at range 16 by AoiCoverage; only action-latency
  // probes use this explicitly narrower, stable initial geometry.
  return kind === 'chat' ? 14 : 12;
}
function combatPlan(client, context) {
  const owner = self(client);
  for (const [id, failure] of client.failedTargets) if (failure.until < Date.now() - 30000) client.failedTargets.delete(id);
  const candidates = combatCandidates(client, context);
  for (const target of candidates.slice(0, 4)) {
    const observer = nearbyObservers(client, context).find(other => other.snapshot.entities.some(entity => entity.objectId === target.objectId));
    if (distance(owner, target) === 1 && observer) {
      const facing = directionOf(owner, target);
      if (owner.direction !== facing) return { command: { type: 'turn', direction: facing }, hunting: target.name };
      return { command: { type: 'attack', objectId: target.objectId }, targetEntity: target, observer,
        peerAttackerId: observedPlayerId(observer, client.label) };
    }
    const adjacent = STEPS.map(([dx, dy]) => ({ x: target.x + dx, y: target.y + dy }))
      .sort((a, b) => distance(a, owner) - distance(b, owner));
    for (const destination of adjacent) {
      const plan = movement(client, destination, context.map);
      if (plan) return { ...plan, hunting: target.name };
    }
  }
  return patrol(client, context);
}

export class FixedPlan {
  constructor(at, interval = 1000) { this.nextAt = at; this.interval = interval; this.index = 0; }
  take(now) {
    if (now < this.nextAt) return null;
    const count = Math.floor((now - this.nextAt) / this.interval) + 1;
    const due = this.nextAt + (count - 1) * this.interval;
    this.nextAt += count * this.interval; this.index += count;
    return { due, missed: count - 1, index: this.index };
  }
}

function checkOwner(client, context) {
  if (!self(client) || hasAuthoritativePlayerDeath(client.snapshot)) throw new Error(`Actor ${client.label} died or lost presence; no automatic resurrection`);
  if (String(client.snapshot.mapFileName) !== context.mapName) throw new Error('Actor left the declared same-map scenario');
}
async function action(client, context, planned) {
  const phase = context.phase, owner = { ...self(client) }, plannedAt = planned.due;
  let plan;
  if (selfActionBlockMask(client)) {
    context.add({ actor: client.label, phase, kind: client.role === 'combat' ? 'attack' : 'walk', status: 'action-locked', plannedGameplay: true }); return;
  }
  if (planned.index % 20 === 5) plan = { command: { type: 'chat', message: `capacity ${context.runId} ${client.label} ${planned.index}` } };
  else if (planned.index % 15 === 0) plan = { command: { type: 'turn', direction: owner.direction === 'Up' ? 'Right' : 'Up' } };
  else plan = client.role === 'combat' ? combatPlan(client, context) : patrol(client, context);
  if (!plan) { context.add({ actor: client.label, phase, kind: 'walk', status: 'navigation-blocked', plannedGameplay: true }); return; }
  const command = plan.command, kind = command.type;
  if (client.budget.remainingWait()) { context.add({ actor: client.label, phase, kind, status: 'budget-gap', plannedGameplay: true }); return; }
  const observers = nearbyObservers(client, context, stableFanoutRange(kind)).map(peer => ({ peer, cursor: peer.sequence,
    objectId: observedPlayerId(peer, client.label) }));
  const observerCursor = plan.observer?.sequence, cursor = client.sequence, sentAt = performance.now();
  if (sentAt - plannedAt > context.policy.maximumSendLatenessMs) {
    context.add({ actor: client.label, phase, kind, status: 'plan-gap', plannedGameplay: true, sendLatenessMs: sentAt - plannedAt }); return;
  }
  if (kind === 'attack') context.targetClaims.set(command.objectId, client.label);
  const row = { actor: client.label, phase, kind, plannedGameplay: true, scheduledAtMs: plannedAt, sentAtMs: sentAt,
    plannedTarget: plan.target, targetId: command.objectId, hunting: plan.hunting };
  try {
    client.send(command);
    const receipt = await client.wait(() => {
      const own = fresh(client, cursor, sentAt), rejection = own.find(event => event.type === 'error');
      if (rejection) { const error = new Error('Public gameplay command rejected'); error.code = 'PUBLIC_COMMAND_REJECTED'; throw error; }
      if (kind === 'attack') return verifiedCombatHit(own, fresh(plan.observer, observerCursor, sentAt), command.objectId, owner.objectId, plan.peerAttackerId);
      if (kind === 'chat') {
        const peer = observers[0]?.peer ?? client, after = observers[0]?.cursor ?? cursor;
        return fresh(peer, after, sentAt).find(event => ['ObjectChat', 'Chat'].includes(event.packet) &&
          String(event.payload?.text ?? event.payload?.message ?? '').includes(command.message));
      }
      return own.find(event => event.packet === 'UserLocation');
    }, `${kind} authoritative acknowledgement`, kind === 'attack' ? 900 : context.policy.actionTimeoutMs);
    let status = 'success';
    if (['walk', 'run'].includes(kind)) {
      const actual = receipt.payload;
      if (!(actual.x === plan.target.x && actual.y === plan.target.y) &&
          !(kind === 'run' && actual.x === plan.first.x && actual.y === plan.first.y)) status = 'corrected';
      if (status === 'success') client.movementCount++;
      row.actual = { x: actual.x, y: actual.y }; row.direction = actual.direction;
    } else if (kind === 'turn' && receipt.payload?.direction !== command.direction) status = 'corrected';
    context.add({ ...row, status, latencyMs: receipt.monotonicMs - sentAt, plannedLatencyMs: receipt.monotonicMs - plannedAt,
      damage: receipt.damage, sharedHealthVerified: receipt.sharedHealthVerified, healthPercent: receipt.healthPercent });
    if (kind === 'attack') client.failedTargets.delete(command.objectId);
    if (kind === 'chat') context.add({ actor: client.label, phase, kind: observers.length ? 'peerChat' : 'selfChat', status: 'success', latencyMs: receipt.monotonicMs - sentAt });
    if (status === 'success' && ['walk', 'run', 'turn', 'chat'].includes(kind)) {
      // Wait only until this action's common deadline. No additional commands
      // are issued and every initially expected observer stays in the denominator.
      const deadline = sentAt + context.policy.actionTimeoutMs;
      const match = entry => fresh(entry.peer, entry.cursor, sentAt).find(event => matchesFanoutReceipt(event, kind, command, entry.objectId, row.actual));
      while (performance.now() < deadline && observers.some(entry => !match(entry))) { context.guard.assert(); await sleep(20); }
      const distribution = new Map(); let delivered = 0;
      for (const entry of observers) {
        const seen = match(entry);
        context.add({ actor: client.label, phase, kind: `fanout-${kind}`, observer: entry.peer.label,
          status: seen ? 'success' : 'timeout', latencyMs: seen ? seen.monotonicMs - sentAt : performance.now() - sentAt, evidence: !seen });
        if (seen) { delivered++; const bucket = Math.ceil(seen.monotonicMs - sentAt); distribution.set(bucket, (distribution.get(bucket) ?? 0) + 1); }
      }
      if (observers.length) context.writer.add({ type: 'fanoutDistribution', actor: client.label, phase, kind, expected: observers.length,
        delivered, latencyUpperBoundsMs: [...distribution], at: Date.now() });
    }
  } catch (error) {
    const swung = kind === 'attack' && fresh(client, cursor, sentAt).some(event => event.packet === 'ObjectAttack' && event.payload?.objectId === owner.objectId);
    if (kind === 'attack' && !(error instanceof SafetyStop)) {
      const prior = client.failedTargets.get(command.objectId);
      client.failedTargets.set(command.objectId, { failures: (prior?.failures ?? 0) + 1, until: Date.now() + (prior ? 30000 : 3000) });
      if (client.failedTargets.size > 64) client.failedTargets.delete(client.failedTargets.keys().next().value);
    }
    context.add({ ...row, status: actionFailureStatus(error, swung, client.closed),
      latencyMs: performance.now() - sentAt, plannedLatencyMs: performance.now() - plannedAt, error: sanitize(error.message, client.secrets),
      targetStillAdjacent: kind === 'attack' ? client.snapshot.entities.some(entity => entity.objectId === command.objectId && !entity.dead && distance(entity, self(client)) === 1) : undefined });
    if (error instanceof SafetyStop || client.closed) throw error;
  } finally { if (kind === 'attack' && context.targetClaims.get(command.objectId) === client.label) context.targetClaims.delete(command.objectId); }
}

async function auxiliary(client, context) {
  const phase = context.phase, refresh = Date.now() >= client.nextRefresh;
  if (client.budget.remainingWait()) return;
  const kind = refresh ? 'refresh' : 'rtt', cursor = client.sequence, at = performance.now(), time = Date.now();
  if (refresh) client.nextRefresh = time + 60000;
  else client.nextRtt = time + 10000;
  try {
    client.send(refresh ? { type: 'clientVersion' } : { type: 'keepAlive', time });
    const ack = await client.wait(() => refresh ? client.events.find(event => event.sequence > cursor && event.type === 'worldSnapshot') :
      packet(client, cursor, 'KeepAlive', value => String(value.time) === String(time)), kind, 2000);
    context.add({ actor: client.label, phase, kind, status: 'success', latencyMs: ack.monotonicMs - at });
  } catch (error) {
    context.add({ actor: client.label, phase, kind, status: error instanceof SafetyStop ? 'cancelled' : 'timeout', error: sanitize(error.message, client.secrets) });
    if (client.closed) throw error;
  }
}

function startEngine(context) {
  context.engine = setInterval(() => {
    try {
      context.guard.assert();
      for (const client of context.clients) {
        if (!client.ready || client.paused || client.draining) continue;
        checkOwner(client, context);
        const planned = client.plan.take(performance.now());
        if (planned) {
          for (let n = 0; n < Math.min(planned.missed, 60); n++) context.add({ actor: client.label, kind: 'walk', status: 'plan-gap', plannedGameplay: true });
          if (planned.missed > 60) context.guard.stop('Driver stalled over 60 planned seconds');
          if (client.pending) context.add({ actor: client.label, kind: client.role === 'combat' ? 'attack' : 'walk', status: 'busy', plannedGameplay: true });
          else client.pending = action(client, context, planned).catch(error => {
            if (!(error instanceof SafetyStop)) context.guard.stop(`Actor ${client.label}: ${sanitize(error.message, client.secrets)}`);
          }).finally(() => { client.pending = null; });
        }
        if (!client.pending && !client.auxPending && (Date.now() >= client.nextRtt || Date.now() >= client.nextRefresh)) {
          client.auxPending = auxiliary(client, context).catch(error => context.guard.stop(`Auxiliary transport failed: ${error.message}`))
            .finally(() => { client.auxPending = null; });
        }
      }
    } catch (error) { if (!(error instanceof SafetyStop)) context.guard.stop(error.message); }
  }, 25);
}

async function ordinaryLogin(client, context, purpose = 'admission') {
  let lastNotice = 0;
  while (context.loginBudget.delay(purpose)) {
    context.guard.assert();
    if (Date.now() - lastNotice > 30000) {
      context.writer.add({ type: 'authWindowWait', at: Date.now(), seconds: Math.ceil(context.loginBudget.delay(purpose) / 1000), alreadyActive: context.clients.filter(item => item.ready).length });
      lastNotice = Date.now();
    }
    await sleep(Math.min(250, context.loginBudget.delay(purpose)));
  }
  context.loginBudget.reserve(purpose);
  await writePrivate(context.ledgerFile, { schemaVersion: 1, endpoint: context.endpoint, purpose: 'capacity-auth-budget', attempts: context.loginBudget.attempts });
  const started = performance.now(); await client.connect();
  const ack = await client.request({ type: 'login', accountId: client.account.accountId, password: client.account.password }, 'LoginSuccess');
  const characters = ack.payload?.characters ?? [];
  let selected = characters.find(item => item.name === client.label);
  if (!selected && client.account.state === 'registered' && !characters.length) {
    const created = await client.request({ type: 'newCharacter', name: client.label, class: 'Warrior', gender: 'Male' }, 'NewCharacterSuccess');
    selected = created.payload?.character;
  }
  if (!Number.isInteger(selected?.index) || (client.account.state === 'ready' && selected.index !== client.account.characterIndex)) throw new Error('Expected dedicated character is absent or changed; no account fallback');
  client.account.characterIndex = selected.index; client.account.state = 'ready'; await writePrivate(context.accountFile, context.pool);
  client.snapshot = null; const after = client.sequence;
  const start = await client.request({ type: 'startGame', characterIndex: selected.index }, 'StartGame', 20000);
  if (start.payload?.result !== 4) throw new Error('StartGame denied; an open socket is not an active player');
  await client.wait(() => self(client)?.name === client.label && client.events.some(event => event.sequence > after && event.type === 'worldSnapshot'), 'authoritative player bootstrap', 20000);
  context.add({ actor: client.label, kind: 'bootstrap', status: 'success', latencyMs: performance.now() - started });
}

async function settleClients(context) {
  context.phase = 'settle'; const deadline = Date.now() + 180000;
  while (Date.now() < deadline) {
    context.guard.assert();
    if (context.clients.filter(client => client.ready && client.role === 'movement').every(client => distance(self(client), client.home) <= 3)) return;
    await sleep(250);
  }
  throw new Error('Ordinary paths did not reach the declared scenario within 180 seconds');
}

async function measure(context, name, seconds) {
  context.phase = name; context.current = new Measurements(); context.window = new Measurements(); context.aoi.reset();
  const started = Date.now(), deadline = started + seconds * 1000, windows = [];
  let nextWindow = started + 60000, windowStart = started;
  while (Date.now() < deadline) {
    context.guard.assert(); await sleep(100);
    if (Date.now() >= nextWindow) {
      const assessment = assessMeasurements(context.window, context.clients.filter(client => client.ready).map(client => ({ name: client.label, role: client.role })),
        (Date.now() - windowStart) / 1000, context.policy, { requireChat: context.clients.length > 1 });
      windows.push({ elapsedSeconds: (Date.now() - started) / 1000, ...assessment });
      context.writer.add({ type: 'window', phase: name, assessment });
      context.window = new Measurements(); windowStart = Date.now(); nextWindow += 60000;
    }
  }
  const actors = context.clients.filter(client => client.ready).map(client => ({ name: client.label, role: client.role }));
  const assessment = assessMeasurements(context.current, actors, seconds, context.policy, { requireChat: actors.length > 1, aoi: { ...context.aoi.stats } });
  const fanout = Object.entries(assessment.metrics).filter(([kind]) => kind.startsWith('fanout-'));
  const fanoutPassed = fanout.every(([, value]) => (value.status.timeout ?? 0) / Math.max(1, value.count) < .01 && value.fromSend.p95Ms <= 1000);
  const output = { name, controlledActors: actors.length, completed: true, ...assessment, fanoutPassed,
    rollingWindows: windows, passed: assessment.passed && fanoutPassed && windows.every(window => window.passed) };
  context.writer.add({ type: 'stageComplete', stage: output });
  context.current = null; context.window = null; context.phase = 'ramp';
  return output;
}

export async function awaitPendingActions(clients) {
  // Capture once. New actions may continue for unpaused actors, but any old
  // observer obligation must finish before its planned transport is removed.
  const pending = clients.flatMap(client => [client.pending, client.auxPending]).filter(Boolean);
  await Promise.allSettled(pending);
}

async function resumeActor(original, context) {
  original.paused = true; context.continuous.pause(original.label);
  await awaitPendingActions(context.clients);
  await original.refresh();
  const before = resumeFingerprint(original.snapshot, original.label);
  await original.wait(() => original.nativeTicket?.expiresAtMs > Date.now() + 10000, 'fresh native resume credential');
  const ticket = { ...original.nativeTicket }, started = performance.now();
  await original.loseTransport(); await sleep(1000);
  const restored = new CapacityClient(context.endpoint, original.account, context), replay = new CapacityClient(context.endpoint, original.account, context);
  context.allClients.push(restored, replay);
  try {
    await restored.connect(); const cursor = restored.sequence;
    await restored.pace(); restored.control({ type: 'resumeSession', credential: ticket.credential });
    const resumed = await restored.wait(() => {
      if (restored.events.some(event => event.sequence > cursor && ['resumeRejected', 'error'].includes(event.type))) throw new Error('Native resume rejected; ordinary login cannot substitute');
      return restored.events.find(event => event.sequence > cursor && event.type === 'sessionResumed');
    }, 'sessionResumed', 10000);
    const snapshot = await restored.wait(() => restored.events.find(event => event.sequence > resumed.sequence && event.type === 'worldSnapshot' && event.payload?.entities?.some(entity => entity.name === original.label)), 'resumed authoritative snapshot', 5000);
    await restored.wait(() => restored.nativeTicket?.generation === ticket.generation + 1, 'rotated resume credential', 5000);
    const restoredState = resumeFingerprint(restored.snapshot, original.label);
    assertResumeEvidence({ before, after: restoredState, characterIndex: original.account.characterIndex,
      resumed, firstGeneration: ticket.generation, rotatedGeneration: restored.nativeTicket.generation,
      ticketChanged: restored.nativeTicket.credential !== ticket.credential, snapshotSequence: snapshot.sequence,
      sentTypes: restored.events.filter(event => event.direction === 'sent').map(event => event.type) });
    await replay.connect(); await replay.pace(); replay.control({ type: 'resumeSession', credential: ticket.credential });
    await replay.wait(() => replay.events.some(event => event.type === 'resumeRejected'), 'old ticket rejection', 5000);
    assertReplayRejected(replay.events); replay.draining = true; await replay.close();
    const direction = self(restored).direction === 'Up' ? 'Right' : 'Up';
    const liveAck = await restored.request({ type: 'turn', direction }, 'UserLocation', 5000);
    if (liveAck.payload?.direction !== direction) throw new Error('Resumed owner no longer accepts ordinary authoritative commands');
    restored.home = original.home; restored.role = original.role; restored.ready = true;
    restored.plan = new FixedPlan(performance.now() + 1000); restored.nextRtt = Date.now() + 5000; restored.nextRefresh = Date.now() + 60000;
    context.clients[context.clients.indexOf(original)] = restored; original.ready = false;
    return { actor: original.label, status: 'passed', elapsedMs: performance.now() - started, characterIndex: original.account.characterIndex,
      before, restoredState, resumedOwnerTurnReceipt: liveAck.sequence, generation: restored.nativeTicket.generation, replayRejected: true, loginFallback: false };
  } catch (error) {
    // Keep the resumed socket reachable for final normal logout if validation
    // fails after ownership was already transferred.
    restored.draining = true; replay.draining = true;
    await Promise.allSettled([restored.close(), replay.close()]); throw error;
  }
}

async function drain(client, context) {
  client.paused = true; context.continuous?.pause(client.label);
  await Promise.allSettled([client.pending, client.auxPending].filter(Boolean));
  client.draining = true;
  let state;
  try { if (client.inGame && !client.closed) { await client.refresh(); state = fingerprint(client); } }
  catch (error) { context.cleanupErrors.push({ actor: client.label, step: 'finalSnapshot', error: sanitize(error.message, client.secrets) }); }
  try { await client.close(); }
  catch (error) { context.cleanupErrors.push({ actor: client.label, step: 'normalLogout', error: sanitize(error.message, client.secrets) }); }
  client.ready = false; return state ? { account: client.account, state } : null;
}

export async function runCapacity(options) {
  if (options.realm !== 'isolated') throw new Error('--realm isolated is required');
  const endpoint = validateEndpoint(options.endpoint), policy = capacityPolicy(options);
  if (!options.accountsFile || !options.monitor || !options.output) throw new Error('--accounts-file, --monitor and --output are required');
  const output = outsideRepository(options.output); await fs.mkdir(output, { recursive: true });
  const runId = `${Date.now()}-${crypto.randomBytes(4).toString('hex')}`, writer = new BoundedEvidence(path.join(output, `${runId}.samples.jsonl`));
  const guard = new CapacityMonitor(options.monitor, writer), aggregate = new Measurements();
  const context = { endpoint, policy, runId, writer, guard, aggregate, clients: [], allClients: [], targetClaims: new Map(),
    aoi: new AoiCoverage(), phase: 'preflight', cleanupErrors: [], monsterNames: WEAK_MONSTERS,
    add(sample) {
      const row = { at: Date.now(), phase: this.phase, ...sample };
      aggregate.add(row);
      this.continuous?.add(row);
      if (row.phase === this.phase) { this.current?.add(row); this.window?.add(row); }
      if (row.evidence !== false) writer.add({ type: 'action', ...row });
    } };
  context.continuous = new ContinuousAcceptance(policy, {
    emit: event => writer.add({ ...event, at: Date.now() }),
    onFailure: reason => guard.stop(reason),
  });
  const report = { schemaVersion: 1, runId, endpoint, policy, startedAt: new Date().toISOString(), stages: [], nativeResume: [], savedVerification: [],
    ordinaryAccountsOnly: true, visualAccepted: false, capacityUpperBound: null, acceptanceScope: 'One real egress, ordinary same-map movement/AOI and the explicitly declared combat subgroup' };
  const lag = monitorEventLoopDelay({ resolution: 20 }); lag.enable();
  let timer, sampler, draining = false, driverLagFailures = 0, initialMemoryAt, finalStage, lockFile, lock;
  const onSignal = () => guard.stop('Operator requested normal load drain');
  process.on('SIGINT', onSignal); process.on('SIGTERM', onSignal);
  try {
    await guard.start();
    context.accountFile = await privateFile(options.accountsFile);
    lockFile = path.join(path.dirname(context.accountFile), 'capacity-run.lock');
    lock = await fs.open(lockFile, 'wx', 0o600);
    await lock.writeFile(JSON.stringify({ runId, pid: process.pid, endpoint, startedAt: Date.now() }));
    context.pool = validateCapacityPool(JSON.parse(await fs.readFile(context.accountFile, 'utf8')), endpoint);
    if (context.pool.accounts.length < policy.stages.at(-1)) throw new Error('Not enough separately provisioned ordinary accounts for declared stages');
    report.provisioning = [...new Set(context.pool.accounts.map(account => account.provisioning))];
    report.registrationLoadTested = false; report.simultaneousLoginBurstTested = false; report.mixedClassCombatTested = false;
    report.baselineActiveSessions = Number(guard.latest.health?.currentActiveSessions ?? 0);
    if (Number(guard.latest.health?.maxActiveSessions ?? 0) < report.baselineActiveSessions + policy.stages.at(-1)) throw new Error('Declared stages exceed current isolated admission cap after reserving existing players');
    context.ledgerFile = path.join(path.dirname(context.accountFile), 'capacity-auth-budget.json');
    let ledger;
    try { ledger = JSON.parse(await fs.readFile(context.ledgerFile, 'utf8')); }
    catch (error) { if (error.code !== 'ENOENT') throw error; }
    if (ledger && (ledger.schemaVersion !== 1 || ledger.purpose !== 'capacity-auth-budget' || ledger.endpoint !== endpoint)) throw new Error('Existing auth ledger belongs to a different realm or schema');
    const notBefore = options.loginNotBefore ? Date.parse(options.loginNotBefore) : ledger ? 0 : Date.now() + LOGIN_WINDOW_MS;
    if (!Number.isFinite(notBefore)) throw new Error('--login-not-before must be an explicit ISO time');
    context.loginBudget = new LoginBudget(ledger?.attempts ?? [], notBefore);
    report.initialLoginNotBefore = new Date(Math.max(Date.now(), notBefore)).toISOString();
    let scenario = {};
    if (options.scenarioFile) scenario = JSON.parse(await fs.readFile(options.scenarioFile, 'utf8'));
    if (scenario.monsterNames) {
      if (!Array.isArray(scenario.monsterNames) || !scenario.monsterNames.length || scenario.monsterNames.some(name => !WEAK_MONSTERS.includes(name))) throw new Error('Combat scenario must use ordinary starter-monster whitelist; guards are never attack targets');
      context.monsterNames = scenario.monsterNames;
    }
    report.scenario = { layout: policy.layout, monsterNames: context.monsterNames, combatActors: policy.combatActors };
    timer = setTimeout(() => guard.stop('Finite capacity wall-clock budget exhausted'), policy.wallSeconds * 1000);
    sampler = setInterval(() => {
      if (draining) return;
      try {
        guard.assert();
        const p99Ms = lag.percentile(99) / 1e6, driver = { rssBytes: process.memoryUsage().rss, eventLoopLagP99Ms: p99Ms, eventLoopLagMaxMs: lag.max / 1e6 };
        if (p99Ms > policy.maximumDriverLagP99Ms) driverLagFailures++; else driverLagFailures = 0;
        if (driverLagFailures >= 3) guard.stop('Load generator event-loop lag exceeded 250ms for 3 samples; result cannot measure server capacity');
        context.continuous.sync(context.clients.map(client => ({ name: client.label,
          active: client.ready && !client.paused && !client.closed && client.inGame && Boolean(self(client)) })));
        const aoi = context.aoi.sample(context.clients);
        context.continuous.observeAoi(aoi, context.phase);
        writer.add({ type: 'coverage', phase: context.phase, at: Date.now(), aoi, driver }); lag.reset();
        report.peakControlledActors = Math.max(report.peakControlledActors ?? 0, aoi.activeOwners);
      } catch (error) { if (!(error instanceof SafetyStop)) guard.stop(error.message); }
    }, 1000);
    startEngine(context);
    const combatIndices = new Set(Array.from({ length: policy.combatActors }, (_, i) => Math.floor((i + .5) * policy.stages.at(-1) / policy.combatActors)));
    for (const count of policy.stages) {
      context.phase = 'ramp';
      while (context.clients.length < count) {
        guard.assert(); const index = context.clients.length;
        const client = new CapacityClient(endpoint, context.pool.accounts[index], context); context.allClients.push(client);
        await ordinaryLogin(client, context);
        if (!context.map) {
          context.mapName = String(client.snapshot.mapFileName);
          if (scenario.mapFileName && String(scenario.mapFileName) !== context.mapName) throw new Error('Ordinary character is not on the declared scenario map');
          context.map = await loadProtocolCollisionMap(context.mapName, options.mapRoot ? { mapRoot: options.mapRoot, packagedMapRoot: options.mapRoot } : {});
          context.homes = chooseHomes(context.map, self(client), policy.stages.at(-1), policy.layout, client.snapshot.mapTransfers ?? [], scenario.anchors ?? []);
          report.scenario.mapFileName = context.mapName; report.scenario.anchors = context.homes;
        }
        checkOwner(client, context);
        await equipStarterGear(client);
        client.role = combatIndices.has(index) ? 'combat' : 'movement'; client.home = context.homes[index]; client.ready = true;
        client.plan = new FixedPlan(performance.now() + 500 + (index % 10) * 40);
        client.nextRtt = Date.now() + 5000; client.nextRefresh = Date.now() + 60000;
        context.clients.push(client);
        context.continuous.join(client.label, client.role);
        writer.add({ type: 'admitted', actor: client.label, role: client.role, characterIndex: client.account.characterIndex, at: Date.now(), controlledActors: context.clients.length });
        options.onProgress?.({ stage: 'admitted', controlledActors: context.clients.length, target: count });
        await guard.afterAdmission(Date.now());
      }
      await settleClients(context);
      const stage = await measure(context, `stage-${count}`, policy.stageSeconds); report.stages.push(stage);
      options.onProgress?.({ stage: stage.name, passed: stage.passed });
      if (!stage.passed) throw new Error(`Stage ${count} did not satisfy the fixed activity, latency or coverage gates`);
      report.highestPassedStage = count;
    }
    initialMemoryAt = Date.now(); finalStage = await measure(context, 'soak', policy.soakSeconds); report.stages.push(finalStage);
    report.memoryTrend = memoryTrend(guard.history.filter(sample => sample.sampledAtMs >= initialMemoryAt + policy.memoryWarmupSeconds * 1000));
    if (!finalStage.passed) throw new Error('Soak failed fixed rolling activity or delivery gates');
    if (policy.profile === 'standard' && (report.memoryTrend.seconds < 1800 || report.memoryTrend.slopeMiBPerHour > policy.maximumMemorySlopeMiBPerHour)) throw new Error('Insufficient stable-memory duration or continuing memory growth above the declared threshold');
    context.phase = 'native-resume-under-load';
    for (const original of context.clients.filter(client => client.role === 'movement').slice(0, policy.resumeSamples)) {
      guard.assert(); const result = await resumeActor(original, context); report.nativeResume.push(result);
      writer.add({ type: 'nativeResume', result }); await sleep(2000);
    }
    if (report.nativeResume.length !== policy.resumeSamples) throw new Error('Not enough movement actors for the predeclared native resume sample');
    if (policy.resumeSamples) {
      const observation = await measure(context, 'post-resume-observation', policy.profile === 'probe' ? 20 : 60);
      report.stages.push(observation);
      if (!observation.passed) throw new Error('The restored cohort did not preserve gameplay and AOI under continued load');
    }
  } catch (error) { report.error = sanitize(error.message, context.pool?.accounts.map(account => account.password) ?? []); }
  finally {
    draining = true; context.phase = 'drain'; clearInterval(context.engine); clearInterval(sampler);
    // Offered load ends for the entire cohort now. Later drain batches still
    // await their real outstanding receipts, but must not accrue idle time
    // while earlier batches save and close their sessions.
    const stoppedAt = performance.now();
    for (const client of context.clients) { client.paused = true; context.continuous.pause(client.label, stoppedAt); }
    await awaitPendingActions(context.clients);
    const saved = [];
    // Bounded drain concurrency avoids turning safety shutdown into a save storm.
    const live = context.allClients.filter(client => client.ws && !client.closed);
    for (let offset = 0; offset < live.length; offset += 4) {
      const results = await Promise.all(live.slice(offset, offset + 4).map(client => drain(client, context)));
      saved.push(...results.filter(Boolean));
    }
    context.continuous.finish();
    if (!report.error && !guard.stopped && !context.cleanupErrors.length) {
      context.phase = 'save-verification';
      for (const sample of saved.slice(0, policy.saveSamples)) {
        const client = new CapacityClient(endpoint, sample.account, context); context.allClients.push(client);
        try {
          await ordinaryLogin(client, context, 'verification');
          const actual = fingerprint(client);
          if (JSON.stringify(actual) !== JSON.stringify(sample.state)) throw new Error('Normal logout/relogin changed authoritative transform, inventory or equipment');
          report.savedVerification.push({ actor: client.label, status: 'passed', state: actual });
        } catch (error) { report.savedVerification.push({ actor: client.label, status: 'failed', error: sanitize(error.message, client.secrets) }); break; }
        finally { await drain(client, context); }
      }
    }
    clearTimeout(timer); guard.close(); lag.disable(); process.off('SIGINT', onSignal); process.off('SIGTERM', onSignal);
    if (lock) {
      try { await lock.close(); await fs.unlink(lockFile); }
      catch (error) { context.cleanupErrors.push({ step: 'driverLockRelease', error: error.message }); }
    }
    report.finishedAt = new Date().toISOString(); report.safetyStopped = guard.stopped; report.safetyReason = guard.reason;
    report.cleanupErrors = context.cleanupErrors; report.metrics = aggregate.summary(); report.samples = aggregate.samples;
    report.continuousAcceptance = context.continuous.summary();
    report.peakServerActive = guard.peakServerActive; report.bytes = context.allClients.reduce((sum, client) => ({ sent: sum.sent + client.bytes.sent, received: sum.received + client.bytes.received }), { sent: 0, received: 0 });
    report.bytesDefinition = 'WebSocket JSON payload bytes, excluding TLS and framing';
    report.boundedStorage = { retainedMonitorSamples: guard.history.length, maximumMonitorSamples: guard.maximumHistory,
      monitorTimeBucketMs: guard.timeline.bucketMs, rawMonitorSamples: guard.timeline.rawSamples,
      maximumEventsPerConnection: 2048, writerMaximumBufferedBytes: writer.maximumBufferedBytes, writerLimitBytes: writer.maxBufferedBytes };
    report.ok = !report.error && !guard.stopped && !context.cleanupErrors.length && !context.continuous.failed && Boolean(finalStage?.passed) &&
      report.savedVerification.length === policy.saveSamples && report.savedVerification.every(result => result.status === 'passed');
    report.movementCapacityAccepted = report.ok && policy.fullAcceptanceEligible;
    report.combatCapacityAccepted = report.movementCapacityAccepted && finalStage?.combatCoverage === 'shared-positive-damage';
    report.playableCapacityAccepted = report.movementCapacityAccepted && report.combatCapacityAccepted;
    report.acceptedControlledActors = report.playableCapacityAccepted ? policy.stages.at(-1) : null;
    writer.add({ type: 'finished', ok: report.ok, playableCapacityAccepted: report.playableCapacityAccepted });
    try { await writer.close(); } catch (error) { report.ok = false; report.movementCapacityAccepted = false; report.combatCapacityAccepted = false; report.playableCapacityAccepted = false; report.acceptedControlledActors = null; report.evidenceError = error.message; }
    report.reportPath = path.join(output, `${runId}.report.json`);
    await fs.writeFile(report.reportPath, JSON.stringify(sanitize(report, context.pool?.accounts.map(account => account.password) ?? []), null, 2) + '\n');
  }
  return report;
}

export function parseArguments(args) {
  const result = {}, allowed = new Set(['realm', 'endpoint', 'accountsFile', 'monitor', 'output', 'profile', 'stages', 'stageSeconds', 'soakSeconds',
    'wallSeconds', 'layout', 'combatActors', 'resumeSamples', 'saveSamples', 'mapRoot', 'loginNotBefore', 'scenarioFile']);
  for (let i = 0; i < args.length; i += 2) {
    if (!args[i].startsWith('--') || args[i + 1] == null) throw new Error('Arguments must use --name value');
    const key = args[i].slice(2).replace(/-([a-z])/g, (_, char) => char.toUpperCase());
    if (!allowed.has(key) || Object.hasOwn(result, key)) throw new Error(`Unknown or repeated option: ${args[i]}`);
    result[key] = args[i + 1];
  }
  return result;
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  try {
    const report = await runCapacity({ ...parseArguments(process.argv.slice(2)), onProgress: value => process.stdout.write(JSON.stringify(value) + '\n') });
    process.stdout.write(JSON.stringify({ reportPath: report.reportPath, ok: report.ok, playableCapacityAccepted: report.playableCapacityAccepted,
      highestPassedStage: report.highestPassedStage, safetyStopped: report.safetyStopped, error: report.error }, null, 2) + '\n');
    process.exitCode = report.ok ? 0 : 1;
  } catch (error) { process.stderr.write(error.message + '\n'); process.exitCode = 1; }
}
