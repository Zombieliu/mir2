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
import { capacityPolicy, validateCapacityPool, LoginBudget, LOGIN_WINDOW_MS, Measurements, assessMeasurements, movementDiagnosticMayContinue,
  AoiCoverage, ContinuousAcceptance, NativePlan, nativeCadence, verifiedCombatHit, BoundedEvidence, CapacityMonitor, memoryTrend, self, distance, sleep, sanitize, fingerprint } from './playtest-capacity-soak-core.mjs';

const STEPS = [[0, -1, 'Up'], [1, -1, 'UpRight'], [1, 0, 'Right'], [1, 1, 'DownRight'],
  [0, 1, 'Down'], [-1, 1, 'DownLeft'], [-1, 0, 'Left'], [-1, -1, 'UpLeft']];
const WEAK_MONSTERS = ['Scarecrow', 'Hen', 'Deer', 'HookingCat', 'RakingCat'];
const fresh = (client, cursor, at) => client.events.filter(event => event.direction === 'received' && event.sequence > cursor && event.monotonicMs >= at);
const packet = (client, after, name, predicate = () => true) => client.events.find(event => event.direction === 'received' && event.sequence > after && event.packet === name && predicate(event.payload));
const inside = (point, bounds) => bounds && point.x >= bounds.minX && point.x <= bounds.maxX && point.y >= bounds.minY && point.y <= bounds.maxY;
const PLAYER_LIFECYCLE_PACKETS = new Set(['ObjectPlayer', 'ObjectRemove', 'ObjectTeleportOut', 'MapChanged', 'MapInformation']);
const PLAYER_MOTION_PACKETS = new Set(['ObjectWalk', 'ObjectRun', 'ObjectTurn']);
const LIFECYCLE_IDENTITY_LIMIT = 256;
const NAVIGATION_ATTEMPT_LIMIT = 40;
const NAVIGATION_OCCUPANT_LIMIT = 32;
const REJECTED_WALK_CELL_LIMIT = 16;
const REJECTED_WALK_CELL_TTL_MS = 3000;

// Deliberately select public identity/position fields. Never copy a packet or
// snapshot wholesale into the persistent diagnostic stream.
function lifecyclePoint(value) {
  if (!value || !Number.isFinite(value.x) || !Number.isFinite(value.y)) return null;
  return { x: value.x, y: value.y };
}
function lifecyclePlayer(entity, names) {
  if (!entity || !names.has(entity.name) || !Number.isInteger(entity.objectId)) return null;
  const point = lifecyclePoint(entity.location ?? entity);
  return { objectId: entity.objectId, name: entity.name, ...(point ?? {}) };
}
function lifecycleProjection(snapshot, names) {
  const players = []; let count = 0;
  for (const entity of snapshot?.entities ?? []) {
    if (!['player', 'remoteplayer', 'selfplayer'].includes(String(entity.kind).toLowerCase())) continue;
    const player = lifecyclePlayer(entity, names);
    if (player) { count++; if (players.length < LIFECYCLE_IDENTITY_LIMIT) players.push(player); }
  }
  players.sort((a, b) => a.objectId - b.objectId || a.name.localeCompare(b.name));
  const owner = snapshot?.entities?.find(entity => entity.objectId === snapshot.playerObjectId);
  return { mapFileName: typeof snapshot?.mapFileName === 'string' ? snapshot.mapFileName.slice(0, 80) : null,
    playerObjectId: Number.isInteger(snapshot?.playerObjectId) ? snapshot.playerObjectId : null,
    self: lifecyclePlayer(owner, names), players, count, truncated: count > players.length };
}
function lifecycleMembership(projection) {
  return JSON.stringify([projection.mapFileName, projection.playerObjectId, projection.count,
    lifecyclePoint(projection.self),
    projection.players.map(player => [player.objectId, player.name])]);
}

function selfTransform(snapshot) {
  const owner = snapshot?.entities?.find(entity => entity.objectId === snapshot.playerObjectId);
  const direction = typeof owner?.direction === 'string' ? owner.direction.slice(0, 32) : Number.isFinite(owner?.direction) ? owner.direction : undefined;
  return { playerObjectId: snapshot?.playerObjectId ?? null, position: lifecyclePoint(owner), ...(direction !== undefined ? { direction } : {}) };
}

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
    this.evidencePending = new Set(); this.nativeRunPrimedUntil = 0;
    this.lifecycleConnection = context.nextLifecycleConnection = (context.nextLifecycleConnection ?? 0) + 1;
    this.lifecycleNames = new Set([this.label, ...(context.pool?.accounts ?? []).slice(0, 100).map(item => item.name)].filter(name => typeof name === 'string'));
    this.lifecycleIdentities = new Map(); this.lifecycleMissing = new Set(); this.lifecycleOrphans = new Set();
    this.lifecycleRows = 0; this.lifecycleIdentityEvictions = 0; this.lifecycleUnchangedSnapshots = 0;
    this.navigationBlocked = false; this.navigationBlockedRows = 0;
    this.rejectedWalkCells = new Map(); this.movementCorrectionStreak = false; this.movementCorrectionRows = 0;
    this.selfTransformRows = 0;
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
    const objectId = message.payload?.objectId;
    const orphanMotion = PLAYER_MOTION_PACKETS.has(message.packet) && this.lifecycleMissing.has(objectId) && !this.lifecycleOrphans.has(objectId);
    const lifecycle = message.type === 'worldSnapshot' || PLAYER_LIFECYCLE_PACKETS.has(message.packet) || orphanMotion;
    const before = lifecycle ? lifecycleProjection(this.snapshot, this.lifecycleNames) : null;
    // Observe every public packet's actual effect, including combat packets
    // such as ObjectStruck; never assume an RTT echo caused a nearby change.
    const ownerTransform = message.type === 'packet' && typeof message.packet === 'string';
    const beforeSelf = ownerTransform ? selfTransform(this.snapshot) : null;
    super.observe(message);
    if (lifecycle) this.recordPlayerLifecycle(message, before, orphanMotion);
    if (ownerTransform) {
      const afterSelf = selfTransform(this.snapshot);
      if (JSON.stringify(beforeSelf.position) !== JSON.stringify(afterSelf.position)) {
        const receipt = this.events.at(-1), payload = message.payload ?? {};
        const location = lifecyclePoint(payload.location), position = lifecyclePoint(payload);
        const direction = typeof payload.direction === 'string' ? payload.direction.slice(0, 32) : Number.isFinite(payload.direction) ? payload.direction : undefined;
        this.context.writer?.add({ type: 'selfTransform', actor: this.label, connection: this.lifecycleConnection,
          at: Date.now(), sequence: receipt.sequence, monotonicMs: receipt.monotonicMs, phase: this.context.phase, source: message.packet.slice(0, 80),
          mapFileName: typeof this.snapshot?.mapFileName === 'string' ? this.snapshot.mapFileName.slice(0, 80) : null,
          wire: { ...(Number.isInteger(objectId) ? { objectId } : {}), ...(position ?? {}), ...(location ? { location } : {}),
            ...(direction !== undefined ? { direction } : {}) }, before: beforeSelf, after: afterSelf });
        this.selfTransformRows++;
      }
    }
  }
  recordPlayerLifecycle(message, before, orphanMotion) {
    const after = lifecycleProjection(this.snapshot, this.lifecycleNames), receipt = this.events.at(-1);
    const remember = player => {
      if (!this.lifecycleIdentities.has(player.objectId) && this.lifecycleIdentities.size >= LIFECYCLE_IDENTITY_LIMIT) {
        const oldest = this.lifecycleIdentities.keys().next().value;
        this.lifecycleIdentities.delete(oldest); this.lifecycleMissing.delete(oldest); this.lifecycleOrphans.delete(oldest);
        this.lifecycleIdentityEvictions++;
      }
      this.lifecycleIdentities.set(player.objectId, { ...player });
    };
    for (const player of before.players) remember(player);
    const objectId = message.payload?.objectId;
    const wirePlayer = message.packet === 'ObjectPlayer' ? lifecyclePlayer(message.payload, this.lifecycleNames) : null;
    const projectedBefore = before.players.find(player => player.objectId === objectId);
    const remembered = this.lifecycleIdentities.get(objectId);
    const peer = wirePlayer ?? projectedBefore ?? remembered;
    if (wirePlayer) remember(wirePlayer);
    for (const player of after.players) {
      remember(player); this.lifecycleMissing.delete(player.objectId); this.lifecycleOrphans.delete(player.objectId);
    }
    const present = new Set(after.players.map(player => player.objectId));
    for (const player of before.players) if (!present.has(player.objectId) && this.lifecycleIdentities.has(player.objectId)) this.lifecycleMissing.add(player.objectId);
    if (wirePlayer && !present.has(objectId) && this.lifecycleIdentities.has(objectId)) this.lifecycleMissing.add(objectId);
    const snapshotReplacement = message.type === 'worldSnapshot';
    const mapReplacement = ['MapChanged', 'MapInformation'].includes(message.packet);
    if (snapshotReplacement || mapReplacement) {
      if (lifecycleMembership(before) === lifecycleMembership(after)) { this.lifecycleUnchangedSnapshots++; return; }
    } else if (!peer) return; // No arbitrary non-cohort player information.
    if (orphanMotion) this.lifecycleOrphans.add(objectId);
    const select = projection => ({ ...projection,
      players: snapshotReplacement || mapReplacement ? projection.players : projection.players.filter(player => player.objectId === objectId || player.name === peer.name) });
    const wirePoint = lifecyclePoint(message.payload?.location ?? message.payload);
    this.context.writer?.add({ type: 'playerLifecycle', actor: this.label, connection: this.lifecycleConnection,
      at: Date.now(), sequence: receipt.sequence, monotonicMs: receipt.monotonicMs, phase: this.context.phase,
      source: snapshotReplacement ? 'worldSnapshot' : message.packet,
      ...(snapshotReplacement || mapReplacement ? {} : { peer: { objectId, name: peer.name },
        beforePresent: before.players.some(player => player.objectId === objectId), afterPresent: after.players.some(player => player.objectId === objectId),
        identitySource: wirePlayer ? 'packet' : projectedBefore ? 'projection-before' : 'bounded-identity-cache',
        wire: { objectId, ...(wirePlayer ? { name: wirePlayer.name } : {}), ...(wirePoint ?? {}) }, orphanMotion: Boolean(orphanMotion) }),
      before: select(before), after: select(after) });
    this.lifecycleRows++;
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
export async function loadStationaryNpcObstacles(mapFileName) {
  const manifest = JSON.parse(await fs.readFile(new URL('../../../packages/game-data/data/generated/crystal_npc_info_manifest.json', import.meta.url), 'utf8'));
  const entries = manifest.npcs.filter(npc => String(npc.map_file_name) === String(mapFileName));
  if (entries.some(npc => !Number.isInteger(npc.location?.x) || !Number.isInteger(npc.location?.y))) throw new Error('Invalid authored NPC collision metadata');
  return entries.map(npc => ({ ...npc.location, name: npc.name }));
}

export function chooseHomes(map, origin, count, layout, transfers = [], explicit = [], footprintSize = 1, stationaryObstacles = []) {
  const stationary = new Set(stationaryObstacles.map(point => `${point.x},${point.y}`));
  const blockedTransfer = point => transfers.some(transfer => inside(point, transfer.bounds));
  const clear = point => protocolMapCellIsWalkable(map, point) && !blockedTransfer(point) && !stationary.has(`${point.x},${point.y}`);
  const queue = [{ x: origin.x, y: origin.y }], seen = new Set([`${origin.x},${origin.y}`]), reachable = [];
  for (let cursor = 0; cursor < queue.length && cursor < 40000; cursor++) {
    const point = queue[cursor];
    if (clear(point)) reachable.push(point);
    for (const [dx, dy] of STEPS.filter(([x, y]) => !x || !y)) {
      const next = { x: point.x + dx, y: point.y + dy }, key = `${next.x},${next.y}`;
      if (seen.has(key) || distance(next, origin) > 96 || !clear(next)) continue;
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
      if (footprintSize > 1) {
        let available = true;
        for (let dy = 0; dy < footprintSize && available; dy++) for (let dx = 0; dx < footprintSize; dx++) {
          const cell = { x: point.x + dx, y: point.y + dy };
          if (!clear(cell) || used.has(`${cell.x},${cell.y}`)) { available = false; break; }
        }
        if (!available) continue;
      }
      const score = distance(point, wanted) * 1000 + distance(point, origin);
      if (score >= best) continue;
      if (STEPS.filter(([dx, dy]) => !dx || !dy).filter(([dx, dy]) => clear({ x: point.x + dx, y: point.y + dy })).length < 2) continue;
      selected = point; best = score;
    }
    if (!selected || (explicit[index] && distance(selected, explicit[index]) > 2)) throw new Error('Scenario lacks enough reachable patrol anchors');
    for (let dy = 0; dy < footprintSize; dy++) for (let dx = 0; dx < footprintSize; dx++) used.add(`${selected.x + dx},${selected.y + dy}`);
    return selected;
  });
}

export function validateNativeExplicitHomes(map, origin, count, anchors, patrolSpan = 2, transfers = [], stationaryObstacles = []) {
  const cells = map?.width * map?.height;
  if (!Number.isInteger(map?.width) || !Number.isInteger(map?.height) || map.width < 1 || map.height < 1 ||
      !Number.isInteger(cells) || cells < 1 || cells > 4_000_000 || map.blocked?.length !== cells ||
      !Number.isInteger(count) || count < 1 || count > 100 || !Array.isArray(anchors) || anchors.length !== count || ![2, 4].includes(patrolSpan)) {
    throw new Error('Native explicit scenario requires complete bounded anchors and patrolSpan 2 or 4');
  }
  const inMap = point => Number.isInteger(point?.x) && Number.isInteger(point?.y) && point.x >= 0 && point.y >= 0 && point.x < map.width && point.y < map.height;
  if (!inMap(origin)) throw new Error('Native scenario origin is outside the real map');
  const blocked = new Uint8Array(map.blocked), cellIndex = point => point.y * map.width + point.x;
  for (const point of stationaryObstacles) if (inMap(point)) blocked[cellIndex(point)] = 1;
  for (const transfer of transfers) {
    const b = transfer.bounds;
    if (!b || ![b.minX, b.maxX, b.minY, b.maxY].every(Number.isInteger) || b.minX > b.maxX || b.minY > b.maxY) throw new Error('Invalid scenario transfer bounds');
    for (let y = Math.max(0, b.minY); y <= Math.min(map.height - 1, b.maxY); y++) {
      for (let x = Math.max(0, b.minX); x <= Math.min(map.width - 1, b.maxX); x++) blocked[y * map.width + x] = 1;
    }
  }
  const used = new Set(), wanted = new Set(), homes = anchors.map(point => {
    if (!inMap(point)) throw new Error('Native scenario anchor is outside the real map');
    const home = { x: point.x, y: point.y };
    for (let dy = 0; dy <= patrolSpan; dy++) for (let dx = 0; dx <= patrolSpan; dx++) {
      const cell = { x: home.x + dx, y: home.y + dy }, index = cellIndex(cell);
      if (!inMap(cell) || blocked[index] || used.has(index)) throw new Error('Native patrol footprint overlaps terrain, an NPC, transfer or another actor');
      used.add(index);
    }
    wanted.add(cellIndex(home)); return home;
  });
  if (blocked[cellIndex(origin)]) throw new Error('Native scenario origin is statically blocked');
  // This one-time proof is bounded by the actual map, not an arbitrary radius.
  // Runtime movement still uses ordinary intents and its unchanged 4000-node A*.
  const seen = new Uint8Array(cells), queue = new Int32Array(cells); let head = 0, tail = 1;
  queue[0] = cellIndex(origin); seen[queue[0]] = 1;
  while (head < tail && wanted.size) {
    const current = queue[head++], x = current % map.width, y = Math.floor(current / map.width); wanted.delete(current);
    for (const [dx, dy] of [[0, -1], [1, 0], [0, 1], [-1, 0]]) {
      const nx = x + dx, ny = y + dy, index = ny * map.width + nx;
      if (nx < 0 || nx >= map.width || ny < 0 || ny >= map.height || blocked[index] || seen[index]) continue;
      seen[index] = 1; queue[tail++] = index;
    }
  }
  if (wanted.size) throw new Error('Native explicit scenario contains unreachable anchors');
  return { homes, visitedCells: head, mapCells: cells, footprintCells: used.size };
}

export function validateNativeCombatClusters(definitions, homes, combatIndices, patrolSpan = 2) {
  if (definitions == null) return [];
  if (!Array.isArray(definitions) || !definitions.length || definitions.length > combatIndices.size) throw new Error('Native combat clusters must be bounded and nonempty');
  const used = new Set(), fighters = new Set();
  const clusters = definitions.map(definition => {
    const combatActorIndices = definition.combatActorIndices, movementActorIndices = definition.movementActorIndices;
    if (![combatActorIndices, movementActorIndices].every(values => Array.isArray(values) && values.length > 0)) throw new Error('Each cluster requires declared fighters and movement observers');
    for (const [values, combat] of [[combatActorIndices, true], [movementActorIndices, false]]) for (const index of values) {
      if (!Number.isInteger(index) || index < 0 || index >= homes.length || used.has(index) || combatIndices.has(index) !== combat) throw new Error('Invalid or repeated combat cluster actor index');
      used.add(index); if (combat) fighters.add(index);
    }
    const cluster = { combatActorIndices: [...combatActorIndices], movementActorIndices: [...movementActorIndices], patrolSpan,
      observerHomes: movementActorIndices.map(index => homes[index]), observerNames: new Set() };
    if (combatActorIndices.some(index => !insideCombatCoverage(cluster, homes[index]))) throw new Error('Combat home lacks declared observer coverage');
    return cluster;
  });
  if (used.size !== homes.length || fighters.size !== combatIndices.size) throw new Error('Combat clusters must cover the complete declared cohort');
  return clusters;
}

export function insideCombatCoverage(cluster, point) {
  return !cluster || cluster.observerHomes.some(home => Math.max(Math.abs(point.x - home.x), Math.abs(point.x - home.x - cluster.patrolSpan),
    Math.abs(point.y - home.y), Math.abs(point.y - home.y - cluster.patrolSpan)) <= 14);
}

export function spacingFourScenario(map, origin, center, count, mapFileName, transfers = [], stationaryObstacles = []) {
  if (!Number.isInteger(count) || count < 1 || count > 100 || ![origin, center].every(point => Number.isInteger(point.x) && Number.isInteger(point.y))) throw new Error('Scenario requires integer coordinates and 1–100 actors');
  const width = Math.ceil(Math.sqrt(count));
  const requested = Array.from({ length: count }, (_, i) => ({ x: center.x + (i % width - (width - 1) / 2) * 4,
    y: center.y + (Math.floor(i / width) - (width - 1) / 2) * 4 }));
  const anchors = chooseHomes(map, origin, count, 'hotspot', transfers, requested, 3, stationaryObstacles);
  const counts = anchors.map(owner => anchors.filter(peer => peer !== owner && distance(peer, owner) <= 16).length);
  return { mapFileName, anchors, monsterNames: WEAK_MONSTERS, generated: { staticOnly: true, spacing: 4, origin, center,
    actors: count, patrolFootprint: '3x3 clear, non-overlapping cells excluding known stationary NPCs', stationaryNpcCount: stationaryObstacles.length,
    meanVisiblePeers: counts.reduce((sum, n) => sum + n, 0) / count,
    minimumVisiblePeers: Math.min(...counts), maximumVisiblePeers: Math.max(...counts),
    limitation: 'Static collision and doorway planning only; no login, movement, live occupancy, monster supply or performance acceptance' } };
}

export async function generateCapacityScenario(options) {
  if (!options.mapRoot || !options.scenarioMap || !options.scenarioOrigin || !options.generateScenario) throw new Error('Scenario generation requires --map-root, --scenario-map, --scenario-origin and --generate-scenario');
  const point = value => { const parts = String(value).split(',').map(Number); if (parts.length !== 2 || !parts.every(Number.isInteger)) throw new Error('Coordinates must be x,y integers'); return { x: parts[0], y: parts[1] }; };
  const origin = point(options.scenarioOrigin), center = point(options.scenarioCenter ?? options.scenarioOrigin);
  const map = await loadProtocolCollisionMap(options.scenarioMap, { mapRoot: options.mapRoot, packagedMapRoot: options.mapRoot });
  // Resolve committed map transfers without contacting a Gateway or reading an account file.
  const manifestPath = new URL('../lib/generated/crystal_respawn_manifest.json', import.meta.url);
  const manifest = JSON.parse(await fs.readFile(manifestPath, 'utf8'));
  const metadata = manifest.maps.find(map => String(map.map_file_name) === String(options.scenarioMap));
  if (!metadata) throw new Error('Unknown authored scenario map');
  const transfers = metadata.movements.map(move => ({ bounds: { minX: move.source.x, maxX: move.source.x, minY: move.source.y, maxY: move.source.y } }));
  const stationaryObstacles = await loadStationaryNpcObstacles(options.scenarioMap);
  const scenario = spacingFourScenario(map, origin, center, Number(options.scenarioCount ?? 50), String(options.scenarioMap), transfers, stationaryObstacles);
  const file = outsideRepository(options.generateScenario); await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, JSON.stringify(scenario, null, 2) + '\n', { flag: 'wx' });
  return { file, ...scenario.generated };
}

export function directionOf(from, to) {
  const dx = to.x - from.x, dy = to.y - from.y;
  return STEPS.find(([x, y]) => x === dx && y === dy)?.[2] ?? null;
}
function rejectedWalkObstacles(client) {
  if (client.context?.policy.activityMode !== 'native' || !client.rejectedWalkCells) return [];
  const now = performance.now(), mapFileName = String(client.snapshot.mapFileName);
  for (const [key, cell] of client.rejectedWalkCells) {
    if (cell.expiresAt <= now || cell.mapFileName !== mapFileName) client.rejectedWalkCells.delete(key);
  }
  return [...client.rejectedWalkCells.values()];
}
function recordMovementCorrection(client, context, owner, plan, actual, phase, receipt) {
  let avoidance;
  // A Run rejection cannot identify which of two cells failed. Only an
  // unchanged-owner Walk identifies one refused destination to try avoiding;
  // this is a temporary route choice, not a claim about the rejection cause.
  if (plan.command.type === 'walk' && actual.x === owner.x && actual.y === owner.y && distance(owner, plan.first) === 1) {
    rejectedWalkObstacles(client);
    const cells = client.rejectedWalkCells, key = `${plan.first.x},${plan.first.y}`;
    cells.delete(key);
    if (cells.size >= REJECTED_WALK_CELL_LIMIT) cells.delete(cells.keys().next().value);
    cells.set(key, { ...lifecyclePoint(plan.first), mapFileName: String(client.snapshot.mapFileName),
      expiresAt: performance.now() + REJECTED_WALK_CELL_TTL_MS });
    avoidance = { ...lifecyclePoint(plan.first), ttlMs: REJECTED_WALK_CELL_TTL_MS };
    if (client.role === 'movement') client.targetCursor = (client.targetCursor + 1) % 4;
  }
  if (!client.movementCorrectionStreak) {
    client.movementCorrectionStreak = true;
    const occupants = []; let count = 0;
    for (const entity of client.snapshot.entities) {
      if (entity.objectId === owner.objectId || !lifecyclePoint(entity) || distance(entity, owner) > 6) continue;
      count++;
      const kind = String(entity.kind).toLowerCase();
      occupants.push({ ...(Number.isInteger(entity.objectId) ? { objectId: entity.objectId } : {}),
        kind: ['player', 'remoteplayer', 'selfplayer', 'monster', 'npc'].includes(kind) ? kind : 'other',
        ...lifecyclePoint(entity), dead: Boolean(entity.dead), hidden: Boolean(entity.hidden) });
      occupants.sort((a, b) => distance(a, owner) - distance(b, owner) || (a.objectId ?? 0) - (b.objectId ?? 0));
      if (occupants.length > NAVIGATION_OCCUPANT_LIMIT) occupants.pop();
    }
    context.writer?.add({ type: 'movementCorrection', actor: client.label, connection: client.lifecycleConnection,
      at: Date.now(), monotonicMs: receipt.monotonicMs, sequence: receipt.sequence, phase,
      kind: plan.command.type, mapFileName: String(client.snapshot.mapFileName).slice(0, 80),
      origin: lifecyclePoint(owner), first: lifecyclePoint(plan.first), target: lifecyclePoint(plan.target), actual: lifecyclePoint(actual),
      ...(Number.isFinite(owner.hp) ? { ownHp: owner.hp } : {}), ...(Number.isFinite(owner.poison) ? { ownPoison: owner.poison } : {}),
      ...(Number.isFinite(client.snapshot.playerPoison) ? { playerPoison: client.snapshot.playerPoison } : {}),
      temporaryAvoidance: avoidance ?? null, nearbyOccupants: occupants, nearbyOccupantCount: count, occupantsTruncated: count > occupants.length });
    client.movementCorrectionRows++;
  }
  return avoidance;
}
function movement(client, target, map) {
  const owner = self(client), occupied = client.snapshot.entities.filter(entity => entity.objectId !== owner.objectId && !entity.dead)
    .concat(rejectedWalkObstacles(client));
  const blocked = (reason, nextStep) => {
    if (client.navigationAttempts) {
      client.navigationAttemptCount++;
      if (client.navigationAttempts.length < NAVIGATION_ATTEMPT_LIMIT) client.navigationAttempts.push({ target: lifecyclePoint(target),
        reason, staticWalkable: protocolMapCellIsWalkable(map, target),
        targetOccupied: occupied.some(entity => entity.x === target.x && entity.y === target.y), ...(nextStep ? { nextStep } : {}) });
    }
    return null;
  };
  const route = findProtocolWalkPath({ map, start: owner, target, dynamicObstacles: occupied, maxExpanded: 4000 });
  if (!route) return blocked('no-path-or-expansion-limit');
  if (route.length < 2) return blocked('already-at-target');
  const first = route[1];
  if ((client.snapshot.mapTransfers ?? []).some(transfer => inside(first, transfer.bounds))) return blocked('next-step-transfer', first);
  const direction = directionOf(owner, first); if (!direction) return blocked('invalid-unit-step', first);
  const wantsRun = client.context?.policy.activityMode === 'native' ? performance.now() < client.nativeRunPrimedUntil : client.movementCount % 10 === 9;
  if (wantsRun && route.length > 2 && route[2].x - first.x === first.x - owner.x && route[2].y - first.y === first.y - owner.y &&
      !(client.snapshot.mapTransfers ?? []).some(transfer => inside(route[2], transfer.bounds))) {
    return { command: { type: 'run', direction }, target: route[2], first };
  }
  return { command: { type: 'walk', direction }, target: first, first };
}
function patrol(client, context, home = client.home) {
  const owner = self(client), map = context.map;
  // A live obstacle can push a native actor outside its 3x3 patrol. Returning
  // only to home then traps it if that one corner is occupied; try the same
  // four declared corners using ordinary collision-aware paths instead.
  const native = context.policy?.activityMode === 'native', span = native ? context.patrolSpan ?? 2 : 2;
  if (!native && distance(owner, home) > 2) return movement(client, home, map);
  const points = native ? [[span, 0], [span, span], [0, span], [0, 0]] : [[2, 0], [2, 2], [0, 2], [0, 0], [-1, 0], [0, -1]];
  const initialCursor = client.targetCursor;
  if (context.policy?.activityMode === 'native' && performance.now() < client.nativeRunPrimedUntil &&
      owner.x >= home.x && owner.x <= home.x + span && owner.y >= home.y && owner.y <= home.y + span) {
    // Prefer an ordinary clear two-cell corner over a one-two-one detour to a
    // blocked edge. Check the exact same two steps and diagonal side cells as
    // A*, without allocating another full-map search for each of four corners.
    const occupied = new Set(client.snapshot.entities.filter(entity => entity.objectId !== owner.objectId && !entity.dead).concat(rejectedWalkObstacles(client))
      .map(entity => `${entity.x},${entity.y}`));
    const clear = point => protocolMapCellIsWalkable(map, point) && !occupied.has(`${point.x},${point.y}`);
    const doorway = point => (client.snapshot.mapTransfers ?? []).some(transfer => inside(point, transfer.bounds));
    for (let offset = 0; offset < points.length; offset++) {
      const index = (initialCursor + offset) % points.length, [dx, dy] = points[index];
      const corner = { x: home.x + dx, y: home.y + dy }, deltaX = corner.x - owner.x, deltaY = corner.y - owner.y;
      const length = Math.max(Math.abs(deltaX), Math.abs(deltaY));
      if (length < 2 || (deltaX && deltaY && Math.abs(deltaX) !== Math.abs(deltaY))) continue;
      const x = Math.sign(deltaX), y = Math.sign(deltaY), target = { x: owner.x + 2 * x, y: owner.y + 2 * y };
      const first = { x: owner.x + x, y: owner.y + y };
      if (!clear(first) || !clear(target) || doorway(first) || doorway(target)) continue;
      if (x && y && [{ x: owner.x + x, y: owner.y }, { x: owner.x, y: owner.y + y },
        { x: first.x + x, y: first.y }, { x: first.x, y: first.y + y }].some(point => !clear(point))) continue;
      client.targetCursor = index;
      return { command: { type: 'run', direction: directionOf(owner, first) }, first, target };
    }
  }
  for (let offset = 0; offset < points.length; offset++) {
    // Native patrol advances exactly one corner. Mutating the starting cursor
    // inside this loop used to skip a corner and endlessly cut across NPCs.
    // Keep the old baseline traversal for comparisons with its prior reports.
    const index = ((context.policy?.activityMode === 'native' ? initialCursor : client.targetCursor) + offset) % points.length, [dx, dy] = points[index];
    const target = { x: home.x + dx, y: home.y + dy };
    if (distance(owner, target) === 0) { client.targetCursor = (index + 1) % points.length; continue; }
    const plan = movement(client, target, map);
    if (plan) { client.targetCursor = index; return plan; }
  }
  return null;
}

function recordNavigationBlocked(client, context, phase) {
  if (client.navigationBlocked) return;
  client.navigationBlocked = true;
  const owner = self(client), names = client.lifecycleNames ?? new Set([client.label,
    ...(context.pool?.accounts ?? []).slice(0, 100).map(account => account.name)]);
  const occupants = []; let count = 0;
  for (const entity of client.snapshot.entities) {
    if (entity.objectId === owner.objectId || entity.dead || !lifecyclePoint(entity) ||
        Math.min(distance(entity, owner), distance(entity, client.home)) > 6) continue;
    count++;
    const kind = String(entity.kind).toLowerCase(), publicName = ['monster', 'npc'].includes(kind) || names.has(entity.name);
    occupants.push({ ...(Number.isInteger(entity.objectId) ? { objectId: entity.objectId } : {}),
      kind: ['player', 'remoteplayer', 'selfplayer', 'monster', 'npc'].includes(kind) ? kind : 'other',
      ...(publicName && typeof entity.name === 'string' ? { name: entity.name.slice(0, 80) } : {}), x: entity.x, y: entity.y });
    occupants.sort((a, b) => Math.min(distance(a, owner), distance(a, client.home)) - Math.min(distance(b, owner), distance(b, client.home)) ||
      (a.objectId ?? 0) - (b.objectId ?? 0));
    if (occupants.length > NAVIGATION_OCCUPANT_LIMIT) occupants.pop();
  }
  context.writer?.add({ type: 'navigationBlocked', actor: client.label, connection: client.lifecycleConnection,
    at: Date.now(), monotonicMs: performance.now(), sequence: client.sequence, phase, role: client.role,
    mapFileName: typeof client.snapshot.mapFileName === 'string' ? client.snapshot.mapFileName.slice(0, 80) : null,
    self: lifecyclePoint(owner), home: lifecyclePoint(client.home), targetCursor: client.targetCursor,
    maxExpanded: 4000, attempts: client.navigationAttempts ?? [], attemptCount: client.navigationAttemptCount ?? 0,
    attemptsTruncated: (client.navigationAttemptCount ?? 0) > (client.navigationAttempts?.length ?? 0),
    nearbyOccupants: occupants, nearbyOccupantCount: count, occupantsTruncated: count > occupants.length });
  client.navigationBlockedRows = (client.navigationBlockedRows ?? 0) + 1;
}
function nearbyObservers(client, context, margin = 16) {
  return context.clients.filter(other => other !== client && other.inGame && !other.closed && !other.paused &&
    other.snapshot?.mapFileName === client.snapshot.mapFileName && self(other) && distance(self(other), self(client)) <= margin);
}
function combatObservers(client, context) {
  return nearbyObservers(client, context).filter(other => !client.combatCluster || client.combatCluster.observerNames.has(other.label));
}
export function combatCandidates(client, context, now = Date.now()) {
  const owner = self(client);
  const observed = client.combatCluster ? new Set(combatObservers(client, context).flatMap(other => other.snapshot.entities.map(entity => entity.objectId))) : null;
  return client.snapshot.entities.filter(entity => entity.kind === 'monster' && !entity.dead && Number(entity.hp ?? 1) > 0 &&
    String(entity.disposition ?? '').trim().toLowerCase() !== 'friendly' && !String(entity.ownerName ?? '').trim() &&
    WEAK_MONSTERS.includes(entity.name) && context.monsterNames.includes(entity.name) && distance(entity, owner) <= 16 &&
    insideCombatCoverage(client.combatCluster, entity) &&
    (client.failedTargets.get(entity.objectId)?.until ?? 0) <= now &&
    (!context.targetClaims.has(entity.objectId) || context.targetClaims.get(entity.objectId) === client.label))
    .sort((a, b) => (observed ? Number(observed.has(b.objectId)) - Number(observed.has(a.objectId)) : 0) || distance(a, owner) - distance(b, owner));
}

function returnToCombatCoverage(client, context) {
  const owner = self(client), cluster = client.combatCluster;
  if (!cluster) return patrol(client, context);
  const recovering = !insideCombatCoverage(cluster, owner);
  for (const home of [client.home, ...cluster.observerHomes].sort((a, b) => distance(a, owner) - distance(b, owner))) {
    const plan = patrol(client, context, home);
    if (plan && (recovering || (insideCombatCoverage(cluster, plan.first) && insideCombatCoverage(cluster, plan.target)))) return plan;
  }
  return null;
}

export function stableFanoutRange(kind, policy) {
  // Both owners may already have a two-cell Run in flight. Complete AOI
  // membership is still checked at range 16 by AoiCoverage; only action-latency
  // probes use this explicitly narrower, stable initial geometry.
  // During the full 2s receipt deadline a native peer can finish its already
  // in-flight run and three more two-cell steps. Keep that geometry explicit;
  // the independent AOI sampler still covers the complete 16-cell window.
  if (policy?.activityMode === 'native') return kind === 'chat' ? 8 : 6;
  return kind === 'chat' ? 14 : 12;
}
function combatPlan(client, context) {
  const owner = self(client);
  if (context.policy?.activityMode === 'native' && !client.combatArrival) {
    if (distance(owner, client.home) !== 0) {
      const plan = movement(client, client.home, context.map);
      return plan ? { ...plan, initialCombatTransit: true } : null;
    }
    recordCombatArrival(client, context);
  }
  if (!insideCombatCoverage(client.combatCluster, owner)) return returnToCombatCoverage(client, context);
  for (const [id, failure] of client.failedTargets) if (failure.until < Date.now() - 30000) client.failedTargets.delete(id);
  const candidates = combatCandidates(client, context);
  for (const target of candidates.slice(0, 4)) {
    const observer = combatObservers(client, context).find(other => other.snapshot.entities.some(entity => entity.objectId === target.objectId));
    if (distance(owner, target) === 1 && observer) {
      const facing = directionOf(owner, target);
      if (owner.direction !== facing) return { command: { type: 'turn', direction: facing }, hunting: target.name };
      return { command: { type: 'attack', objectId: target.objectId }, targetEntity: target, observer, hunting: target.name,
        peerAttackerId: observedPlayerId(observer, client.label) };
    }
    const adjacent = STEPS.map(([dx, dy]) => ({ x: target.x + dx, y: target.y + dy }))
      .sort((a, b) => distance(a, owner) - distance(b, owner));
    for (const destination of adjacent) {
      if (!insideCombatCoverage(client.combatCluster, destination)) continue;
      const plan = movement(client, destination, context.map);
      if (plan && insideCombatCoverage(client.combatCluster, plan.first) && insideCombatCoverage(client.combatCluster, plan.target)) return { ...plan, hunting: target.name };
    }
  }
  return returnToCombatCoverage(client, context);
}

function recordCombatArrival(client, context) {
  const owner = self(client);
  if (context.policy?.activityMode !== 'native' || client.role !== 'combat' || client.combatArrival ||
      !owner || distance(owner, client.home) !== 0) return;
  client.combatArrival = { at: Date.now(), phase: context.phase, mapFileName: client.snapshot.mapFileName,
    declaredHome: { ...client.home }, actual: { x: owner.x, y: owner.y }, authoritativeSequence: client.sequence,
    activeElapsedMs: Number.isFinite(client.activityStartedAtMs) ? performance.now() - client.activityStartedAtMs : null };
  context.writer?.add({ type: 'combatArrival', actor: client.label, ...client.combatArrival });
}

function recordHuntAttempt(client, owner, target) {
  const pointBounds = point => ({ minX: point.x, maxX: point.x, minY: point.y, maxY: point.y });
  const hunt = client.combatHunting ??= { attempts: 0, positiveHits: 0, totalDamage: 0, firstAt: Date.now(),
    ownerBounds: pointBounds(owner), targetBounds: pointBounds(target), maximumTargetDistanceFromHome: 0 };
  for (const [bounds, point] of [[hunt.ownerBounds, owner], [hunt.targetBounds, target]]) {
    bounds.minX = Math.min(bounds.minX, point.x); bounds.maxX = Math.max(bounds.maxX, point.x);
    bounds.minY = Math.min(bounds.minY, point.y); bounds.maxY = Math.max(bounds.maxY, point.y);
  }
  hunt.attempts++; hunt.lastAt = Date.now();
  hunt.maximumTargetDistanceFromHome = Math.max(hunt.maximumTargetDistanceFromHome, distance(client.home, target));
}

export function actorsReachedScenario(context) {
  return context.clients.filter(client => client.ready).every(client => client.role === 'movement'
    ? distance(self(client), client.home) <= (context.policy.activityMode === 'native' ? (context.patrolSpan ?? 2) + 1 : 3)
    : context.policy.activityMode !== 'native' || Boolean(client.combatArrival));
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
export function selectNativeAction(client, context, now = performance.now()) {
  client.navigationAttempts = context.policy?.activityMode === 'native' ? [] : null; client.navigationAttemptCount = 0;
  if (now >= client.nextNativeChatAt) return { command: { type: 'chat', message: `capacity ${context.runId} ${client.label} ${client.plan.index + 1}` } };
  if (now >= client.nextNativeTurnAt && client.role !== 'combat') return { command: { type: 'turn', direction: self(client).direction === 'Up' ? 'Right' : 'Up' } };
  return client.role === 'combat' ? combatPlan(client, context) : patrol(client, context);
}

async function action(client, context, planned, preparedPlan) {
  const phase = context.phase, owner = { ...self(client) }, plannedAt = planned.due;
  const native = context.policy.activityMode === 'native';
  const nativeMeta = native ? { nativeOpportunity: true, plannedIntervalMs: planned.intervalMs, pacingKnown: planned.cadence.pacingKnown,
    cadence: planned.cadence } : {};
  let plan;
  if (selfActionBlockMask(client)) {
    context.add({ actor: client.label, phase, kind: client.role === 'combat' ? 'attack' : 'walk', status: 'action-locked', plannedGameplay: true, ...nativeMeta }); return;
  }
  if (native) plan = preparedPlan;
  else if (planned.index % 20 === 5) plan = { command: { type: 'chat', message: `capacity ${context.runId} ${client.label} ${planned.index}` } };
  else if (planned.index % 15 === 0) plan = { command: { type: 'turn', direction: owner.direction === 'Up' ? 'Right' : 'Up' } };
  else plan = client.role === 'combat' ? combatPlan(client, context) : patrol(client, context);
  if (!plan) {
    if (native) recordNavigationBlocked(client, context, phase);
    context.add({ actor: client.label, phase, kind: 'walk', status: 'navigation-blocked', plannedGameplay: true, ...nativeMeta }); return;
  }
  const command = plan.command, kind = command.type;
  if (client.budget.remainingWait()) { context.add({ actor: client.label, phase, kind, status: 'budget-gap', plannedGameplay: true, ...nativeMeta }); return; }
  const observers = nearbyObservers(client, context, stableFanoutRange(kind, context.policy)).map(peer => ({ peer, cursor: peer.sequence,
    objectId: observedPlayerId(peer, client.label) }));
  const observerCursor = plan.observer?.sequence, cursor = client.sequence, sentAt = performance.now();
  if (sentAt - plannedAt > context.policy.maximumSendLatenessMs) {
    context.add({ actor: client.label, phase, kind, status: 'plan-gap', plannedGameplay: true, sendLatenessMs: sentAt - plannedAt, ...nativeMeta }); return;
  }
  if (kind === 'attack') context.targetClaims.set(command.objectId, client.label);
  const row = { actor: client.label, phase, kind, plannedGameplay: true, scheduledAtMs: plannedAt, sentAtMs: sentAt,
    plannedTarget: plan.target, targetId: command.objectId, hunting: plan.hunting, ...nativeMeta,
    ...(native && client.role === 'combat' ? { declaredHome: client.home, initialCombatTransit: Boolean(plan.initialCombatTransit),
      targetPosition: plan.targetEntity ? { x: plan.targetEntity.x, y: plan.targetEntity.y } : undefined } : {}) };
  try {
    client.waitReason = ['attack', 'chat'].includes(kind) ? 'evidence-wait' : 'owner-ack-wait';
    client.send(command);
    if (native && kind === 'attack') recordHuntAttempt(client, owner, row.targetPosition);
    if (native) {
      client.plan.sent(sentAt, planned.cadence.cooldownMs);
      if (kind === 'chat') client.nextNativeChatAt = sentAt + context.policy.nativeChatIntervalMs;
      if (kind === 'turn') client.nextNativeTurnAt = sentAt + context.policy.nativeTurnIntervalMs;
    }
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
      if (status === 'success') { client.movementCount++; client.nativeRunPrimedUntil = sentAt + 1200; client.navigationBlocked = false; client.movementCorrectionStreak = false; }
      else if (native) row.temporaryAvoidance = recordMovementCorrection(client, context, owner, plan, actual, phase, receipt);
      row.actual = { x: actual.x, y: actual.y }; row.direction = actual.direction;
      row.movedCells = distance(owner, actual);
      recordCombatArrival(client, context);
    } else if (kind === 'turn' && receipt.payload?.direction !== command.direction) status = 'corrected';
    context.add({ ...row, status, latencyMs: receipt.monotonicMs - sentAt, plannedLatencyMs: receipt.monotonicMs - plannedAt,
      damage: receipt.damage, sharedHealthVerified: receipt.sharedHealthVerified, healthPercent: receipt.healthPercent });
    if (kind === 'attack') {
      client.failedTargets.delete(command.objectId);
      if (native) { client.combatHunting.positiveHits++; client.combatHunting.totalDamage += receipt.damage; }
    }
    if (kind === 'chat') context.add({ actor: client.label, phase, kind: observers.length ? 'peerChat' : 'selfChat', status: 'success', latencyMs: receipt.monotonicMs - sentAt });
    if (status === 'success' && ['walk', 'run', 'turn', 'chat'].includes(kind)) {
      const verifyObservers = async () => {
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
      };
      if (native && observers.length) {
        const evidence = verifyObservers().catch(error => {
          if (!(error instanceof SafetyStop)) context.guard.stop(`Observer evidence failed: ${sanitize(error.message, client.secrets)}`);
        }).finally(() => client.evidencePending.delete(evidence));
        client.evidencePending.add(evidence);
      } else await verifyObservers();
    }
  } catch (error) {
    const swung = kind === 'attack' && fresh(client, cursor, sentAt).some(event => event.packet === 'ObjectAttack' && event.payload?.objectId === owner.objectId);
    if (kind === 'attack' && !(error instanceof SafetyStop)) {
      const prior = client.failedTargets.get(command.objectId);
      // The protocol has no strike request ID. Even an observed swing followed
      // by silence may have damage in flight. Native mode cannot re-use this
      // actor/target pair later and mistake that old damage for a new attack.
      client.failedTargets.set(command.objectId, { failures: (prior?.failures ?? 0) + 1,
        until: native ? Infinity : Date.now() + (prior ? 30000 : 3000), ambiguous: native });
      if (client.failedTargets.size > 64) {
        if (native) context.guard.stop('Native ambiguous-attack quarantine exceeded 64 targets; no old strike identities are evicted');
        else client.failedTargets.delete(client.failedTargets.keys().next().value);
      }
    }
    context.add({ ...row, status: actionFailureStatus(error, swung, client.closed),
      latencyMs: performance.now() - sentAt, plannedLatencyMs: performance.now() - plannedAt, error: sanitize(error.message, client.secrets),
      targetStillAdjacent: kind === 'attack' ? client.snapshot.entities.some(entity => entity.objectId === command.objectId && !entity.dead && distance(entity, self(client)) === 1) : undefined,
      ambiguousTargetQuarantined: kind === 'attack' && native && !(error instanceof SafetyStop) });
    if (error instanceof SafetyStop || client.closed) throw error;
  } finally { client.waitReason = null; if (kind === 'attack' && context.targetClaims.get(command.objectId) === client.label) context.targetClaims.delete(command.objectId); }
}

async function auxiliary(client, context) {
  const phase = context.phase, refresh = Date.now() >= client.nextRefresh;
  if (client.budget.remainingWait()) return;
  const kind = refresh ? 'refresh' : 'rtt', cursor = client.sequence, at = performance.now(), time = Date.now();
  if (refresh) {
    client.nextRefresh = time + 60000;
    // One refresh also proves current transport liveness. Do not schedule a
    // second auxiliary command beside two legal native movement commands.
    if (context.policy.activityMode === 'native') client.nextRtt = time + 10000;
  }
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

function initializePlan(client, context, startAt) {
  client.plan = context.policy.activityMode === 'native' ? new NativePlan(startAt) : new FixedPlan(startAt);
  client.nextNativeChatAt = startAt + 5000; client.nextNativeTurnAt = startAt + context.policy.nativeTurnIntervalMs;
}

export function offerNativeAction(client, context, now = performance.now()) {
  if (now < Math.max(client.plan.nextAt, client.plan.legalAt)) return;
  const plan = selectNativeAction(client, context, now), kind = plan?.command.type ?? 'walk';
  const cadence = nativeCadence(client.snapshot, kind, context.policy), planned = client.plan.take(now, cadence.intervalMs);
  if (!planned) return;
  planned.cadence = cadence;
  const row = { actor: client.label, kind, plannedGameplay: true, nativeOpportunity: true,
    plannedIntervalMs: cadence.intervalMs, pacingKnown: cadence.pacingKnown, cadence };
  for (let n = 0; n < Math.min(planned.missed, 60); n++) context.add({ ...row, status: 'plan-gap' });
  if (planned.missed > 60) { context.guard.stop('Native driver stalled over 60 legal action opportunities'); return; }
  if (client.pending || client.evidencePending.size >= context.policy.maximumEvidenceInFlight) {
    // Damage has no request id. Keep a single unresolved attack so one late
    // strike cannot be credited to two requests. This observer/instrumentation
    // wait stays in offered-load coverage, but is never called server busy.
    context.add({ ...row, status: client.pending ? client.waitReason ?? 'owner-ack-wait' : 'evidence-wait' }); return;
  }
  client.pending = action(client, context, planned, plan).catch(error => {
    if (!(error instanceof SafetyStop)) context.guard.stop(`Actor ${client.label}: ${sanitize(error.message, client.secrets)}`);
  }).finally(() => { client.pending = null; });
}

function startEngine(context) {
  context.engine = setInterval(() => {
    try {
      context.guard.assert();
      for (const client of context.clients) {
        if (!client.ready || client.paused || client.draining) continue;
        checkOwner(client, context);
        if (context.policy.activityMode === 'native') offerNativeAction(client, context);
        else {
          const planned = client.plan.take(performance.now());
          if (planned) {
          for (let n = 0; n < Math.min(planned.missed, 60); n++) context.add({ actor: client.label, kind: 'walk', status: 'plan-gap', plannedGameplay: true });
          if (planned.missed > 60) context.guard.stop('Driver stalled over 60 planned seconds');
          if (client.pending) context.add({ actor: client.label, kind: client.role === 'combat' ? 'attack' : 'walk', status: 'busy', plannedGameplay: true });
          else client.pending = action(client, context, planned).catch(error => {
            if (!(error instanceof SafetyStop)) context.guard.stop(`Actor ${client.label}: ${sanitize(error.message, client.secrets)}`);
          }).finally(() => { client.pending = null; });
          }
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
    if (actorsReachedScenario(context)) return;
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
  const assessment = assessMeasurements(context.current, actors, seconds, context.policy, {
    requireChat: actors.length > 1 && (context.policy.activityMode !== 'native' || seconds * 1000 > context.policy.nativeChatIntervalMs), aoi: { ...context.aoi.stats } });
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
  const pending = clients.flatMap(client => [client.pending, client.auxPending, ...(client.evidencePending ?? [])]).filter(Boolean);
  await Promise.allSettled(pending);
  // A captured owner action can create its independent fanout waiter just
  // before resolving. Include that waiter before transport teardown as well.
  await Promise.allSettled(clients.flatMap(client => [...(client.evidencePending ?? [])]));
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
    restored.failedTargets = original.failedTargets;
    restored.combatArrival = original.combatArrival; restored.combatHunting = original.combatHunting;
    restored.activityStartedAtMs = original.activityStartedAtMs;
    initializePlan(restored, context, performance.now() + 1000); restored.nextRtt = Date.now() + 5000; restored.nextRefresh = Date.now() + 60000;
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
    emit: event => {
      writer.add({ ...event, at: Date.now() });
      if (policy.profile === 'movement' && event.type === 'continuousFailure' && !movementDiagnosticMayContinue(policy, event.evidence)) guard.stop(event.reason);
    },
    onFailure: reason => { if (policy.profile !== 'movement') guard.stop(reason); },
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
    if ((scenario.patrolSpan != null || scenario.combatClusters != null) &&
        (policy.activityMode !== 'native' || !Array.isArray(scenario.anchors) || scenario.anchors.length !== policy.stages.at(-1))) {
      throw new Error('Patrol span and combat clusters require a complete explicit native scenario');
    }
    context.patrolSpan = scenario.patrolSpan ?? 2;
    if (![2, 4].includes(context.patrolSpan)) throw new Error('Native patrolSpan must be 2 or 4');
    if (scenario.monsterNames) {
      if (!Array.isArray(scenario.monsterNames) || !scenario.monsterNames.length || scenario.monsterNames.some(name => !WEAK_MONSTERS.includes(name))) throw new Error('Combat scenario must use ordinary starter-monster whitelist; guards are never attack targets');
      context.monsterNames = scenario.monsterNames;
    }
    report.scenario = { layout: policy.layout, monsterNames: context.monsterNames, combatActors: policy.combatActors, patrolSpan: context.patrolSpan };
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
          const stationaryObstacles = policy.activityMode === 'native' ? [
            ...await loadStationaryNpcObstacles(context.mapName),
            ...client.snapshot.entities.filter(entity => String(entity.kind).toLowerCase() === 'npc' && !entity.dead),
          ] : [];
          if (policy.activityMode === 'native' && scenario.anchors?.length) {
            const proof = validateNativeExplicitHomes(context.map, self(client), policy.stages.at(-1), scenario.anchors, context.patrolSpan, client.snapshot.mapTransfers ?? [], stationaryObstacles);
            context.homes = proof.homes;
            report.scenario.explicitNativeHomes = { visitedCells: proof.visitedCells, mapCells: proof.mapCells, footprintCells: proof.footprintCells };
          } else context.homes = chooseHomes(context.map, self(client), policy.stages.at(-1), policy.layout, client.snapshot.mapTransfers ?? [], scenario.anchors ?? [], policy.activityMode === 'native' ? 3 : 1, stationaryObstacles);
          context.combatClusters = validateNativeCombatClusters(scenario.combatClusters, context.homes, combatIndices, context.patrolSpan);
          for (const cluster of context.combatClusters) cluster.observerNames = new Set(cluster.movementActorIndices.map(index => context.pool.accounts[index].name));
          if (context.combatClusters.length) report.scenario.combatClusters = context.combatClusters.map(cluster => ({ combatActorIndices: cluster.combatActorIndices,
            movementActorIndices: cluster.movementActorIndices, observerFootprintCoverageRadius: 14 }));
          report.scenario.mapFileName = context.mapName; report.scenario.anchors = context.homes;
          if (policy.activityMode === 'native') report.scenario.knownStationaryNpcCount = new Set(stationaryObstacles.map(point => `${point.x},${point.y}`)).size;
        }
        checkOwner(client, context);
        await equipStarterGear(client);
        client.role = combatIndices.has(index) ? 'combat' : 'movement'; client.home = context.homes[index]; client.ready = true;
        client.combatCluster = context.combatClusters.find(cluster => cluster.combatActorIndices.includes(index));
        initializePlan(client, context, performance.now() + 500 + (index % 10) * 40);
        client.nextRtt = Date.now() + 5000; client.nextRefresh = Date.now() + 60000;
        context.clients.push(client);
        client.activityStartedAtMs = performance.now(); context.continuous.join(client.label, client.role, client.activityStartedAtMs);
        writer.add({ type: 'admitted', actor: client.label, role: client.role, characterIndex: client.account.characterIndex, at: Date.now(), controlledActors: context.clients.length });
        options.onProgress?.({ stage: 'admitted', controlledActors: context.clients.length, target: count });
        await guard.afterAdmission(Date.now());
      }
      await settleClients(context);
      const stage = await measure(context, `stage-${count}`, policy.stageSeconds); report.stages.push(stage);
      options.onProgress?.({ stage: stage.name, passed: stage.passed });
      report.highestMeasuredStage = count;
      if (!stage.passed && !movementDiagnosticMayContinue(policy, stage)) throw new Error(`Stage ${count} did not satisfy the fixed activity, latency or coverage gates`);
      if (stage.passed) report.highestPassedStage = count;
    }
    initialMemoryAt = Date.now(); finalStage = await measure(context, 'soak', policy.soakSeconds); report.stages.push(finalStage);
    report.memoryTrend = memoryTrend(guard.history.filter(sample => sample.sampledAtMs >= initialMemoryAt + policy.memoryWarmupSeconds * 1000));
    if (!finalStage.passed && !movementDiagnosticMayContinue(policy, finalStage)) throw new Error('Soak failed fixed rolling activity or delivery gates');
    if (policy.profile !== 'probe' && (report.memoryTrend.slopeMiBPerHour == null || report.memoryTrend.seconds < 1800 || report.memoryTrend.slopeMiBPerHour > policy.maximumMemorySlopeMiBPerHour)) throw new Error('Insufficient stable-memory duration or continuing memory growth above the declared threshold');
    context.phase = 'native-resume-under-load';
    for (const original of context.clients.filter(client => client.role === 'movement').slice(0, policy.resumeSamples)) {
      guard.assert(); const result = await resumeActor(original, context); report.nativeResume.push(result);
      writer.add({ type: 'nativeResume', result }); await sleep(2000);
    }
    if (report.nativeResume.length !== policy.resumeSamples) throw new Error('Not enough movement actors for the predeclared native resume sample');
    if (policy.resumeSamples) {
      const observation = await measure(context, 'post-resume-observation', policy.profile === 'probe' ? 20 : 60);
      report.stages.push(observation);
      if (!observation.passed && !movementDiagnosticMayContinue(policy, observation)) throw new Error('The restored cohort did not preserve gameplay and AOI under continued load');
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
    report.activityMode = policy.activityMode;
    report.actorActivity = [...aggregate.actors].filter(([name]) => context.clients.some(client => client.label === name)).map(([actor, activity]) => ({ actor, ...activity }));
    if (policy.activityMode === 'native') report.nativeLoadScope = {
      movementIntervalMs: policy.movementIntervalMs, movementMinimumMs: 600, singleUnacknowledgedMovement: true,
      attackFormula: 'max(max(550,1400-60*AttackSpeed-min(14*level,370)),600)+50ms',
      nativeVsZoneFloorDifference: 'Native 550ms floor and Zone 600ms floor remain unchanged; this driver respects both',
      chatIntervalMs: policy.nativeChatIntervalMs, maximumObserverEvidenceInFlight: policy.maximumEvidenceInFlight,
      actionFanoutProbeRange: 6, chatFanoutProbeRange: 8, fullAoiMembershipRange: 16,
      evidenceWaitMeaning: 'Instrumentation wait stays in offered-load denominator; insufficient coverage does not establish server overload',
      ambiguousAttackRule: 'Unverified attacks permanently quarantine that actor/target pair for this run; more than 64 such targets stops the test',
      continuousRunningRule: 'The Run ratio and cell-rate gates also apply throughout ramp, anchor travel and ordinary login-window waits',
      limitations: ['Warrior starter melee only', 'No caster AOE, summons or cross-map raid proof', 'No native GPU/render acceptance', 'No simultaneous auth burst or automatic revival'] };
    if (policy.activityMode === 'native') report.combatScenario = {
      rule: 'First reach the declared home by ordinary Walk/Run, then hunt; initial transit stays in continuous activity and stage measurement starts only after arrival',
      searchRadiusFromCurrentPosition: 16, monsterNames: context.monsterNames,
      huntingRangeMeaning: 'Actual attack positions; chasing may leave the initial home. Authored spawns do not prove live target supply',
      actors: context.clients.filter(client => client.role === 'combat').map(client => ({ actor: client.label,
        declaredHome: client.home, arrival: client.combatArrival ?? null, hunting: client.combatHunting ?? null })) };
    report.boundedStorage = { retainedMonitorSamples: guard.history.length, maximumMonitorSamples: guard.maximumHistory,
      monitorTimeBucketMs: guard.timeline.bucketMs, rawMonitorSamples: guard.timeline.rawSamples,
      maximumEventsPerConnection: 2048, writerMaximumBufferedBytes: writer.maximumBufferedBytes, writerLimitBytes: writer.maxBufferedBytes,
      movementCorrection: { maximumRejectedWalkCells: REJECTED_WALK_CELL_LIMIT, rejectedWalkCellTtlMs: REJECTED_WALK_CELL_TTL_MS,
        maximumNearbyOccupants: NAVIGATION_OCCUPANT_LIMIT, rows: context.allClients.reduce((sum, client) => sum + client.movementCorrectionRows, 0),
        rule: 'First correction per uninterrupted movement streak; unchanged-owner Walk temporarily avoids its single refused cell without changing failure accounting' },
      navigationBlocked: { maximumAttempts: NAVIGATION_ATTEMPT_LIMIT, maximumNearbyOccupants: NAVIGATION_OCCUPANT_LIMIT,
        rows: context.allClients.reduce((sum, client) => sum + client.navigationBlockedRows, 0),
        rule: 'First blocked action opportunity until a successful movement acknowledgement; ordinary blocked opportunities remain counted' },
      selfTransform: { rows: context.allClients.reduce((sum, client) => sum + client.selfTransformRows, 0),
        rule: 'Only public packets changing owner coordinates; fixed position/direction fields streamed through the existing writer, no retained aggregate' },
      playerLifecycle: { maximumIdentitiesPerConnection: LIFECYCLE_IDENTITY_LIMIT, maximumProjectionPlayers: LIFECYCLE_IDENTITY_LIMIT,
        rows: context.allClients.reduce((sum, client) => sum + client.lifecycleRows, 0),
        identityEvictions: context.allClients.reduce((sum, client) => sum + client.lifecycleIdentityEvictions, 0),
        unchangedSnapshotsOmitted: context.allClients.reduce((sum, client) => sum + client.lifecycleUnchangedSnapshots, 0),
        movementRule: 'Ordinary movement is not copied; retain only the first known-player movement while its projection is absent, until reappearance' } };
    report.diagnosticCompleted = policy.profile === 'movement' && !report.error && !guard.stopped && !context.cleanupErrors.length &&
      Boolean(finalStage?.completed) && report.nativeResume.length === policy.resumeSamples &&
      report.savedVerification.length === policy.saveSamples && report.savedVerification.every(result => result.status === 'passed');
    report.ok = policy.profile !== 'movement' && !report.error && !guard.stopped && !context.cleanupErrors.length && !context.continuous.failed &&
      report.stages.every(stage => stage.passed) && Boolean(finalStage?.passed) &&
      report.savedVerification.length === policy.saveSamples && report.savedVerification.every(result => result.status === 'passed');
    report.movementCapacityAccepted = report.ok && policy.fullAcceptanceEligible;
    report.combatCapacityAccepted = report.movementCapacityAccepted && finalStage?.combatCoverage === 'shared-positive-damage';
    report.playableCapacityAccepted = report.movementCapacityAccepted && report.combatCapacityAccepted;
    report.acceptedControlledActors = report.playableCapacityAccepted ? policy.stages.at(-1) : null;
    writer.add({ type: 'finished', ok: report.ok, playableCapacityAccepted: report.playableCapacityAccepted });
    try { await writer.close(); } catch (error) { report.ok = false; report.diagnosticCompleted = false; report.movementCapacityAccepted = false; report.combatCapacityAccepted = false; report.playableCapacityAccepted = false; report.acceptedControlledActors = null; report.evidenceError = error.message; }
    report.reportPath = path.join(output, `${runId}.report.json`);
    await fs.writeFile(report.reportPath, JSON.stringify(sanitize(report, context.pool?.accounts.map(account => account.password) ?? []), null, 2) + '\n');
  }
  return report;
}

export function parseArguments(args) {
  const result = {}, allowed = new Set(['realm', 'endpoint', 'accountsFile', 'monitor', 'output', 'profile', 'stages', 'stageSeconds', 'soakSeconds',
    'wallSeconds', 'layout', 'combatActors', 'resumeSamples', 'saveSamples', 'mapRoot', 'loginNotBefore', 'scenarioFile',
    'activityMode', 'movementIntervalMs', 'generateScenario', 'scenarioMap', 'scenarioOrigin', 'scenarioCenter', 'scenarioCount']);
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
    const options = parseArguments(process.argv.slice(2));
    if (options.generateScenario) {
      process.stdout.write(JSON.stringify(await generateCapacityScenario(options), null, 2) + '\n');
    } else {
    const report = await runCapacity({ ...options, onProgress: value => process.stdout.write(JSON.stringify(value) + '\n') });
    process.stdout.write(JSON.stringify({ reportPath: report.reportPath, ok: report.ok, playableCapacityAccepted: report.playableCapacityAccepted,
      highestPassedStage: report.highestPassedStage, safetyStopped: report.safetyStopped, error: report.error }, null, 2) + '\n');
    process.exitCode = report.ok ? 0 : 1;
    }
  } catch (error) { process.stderr.write(error.message + '\n'); process.exitCode = 1; }
}
