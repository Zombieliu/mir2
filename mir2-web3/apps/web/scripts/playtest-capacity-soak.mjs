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
import { CATALOG_GZIP_CAPABILITY } from './playtest-catalog-transport.mjs';
import { PublicDensity, SustainObservations, beginCapacityPhase, finishCapacityPhase } from './playtest-capacity-observability.mjs';
import { CommandBudget, SafetyStop } from './playtest-load-smoke.mjs';
import { loadProtocolCollisionMap, findProtocolWalkPath, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';
import { hasAuthoritativePlayerDeath, observedPlayerHp } from './quest-agent/protocol-observation.mjs';
import { selfActionBlockMask } from './quest-agent/protocol-status.mjs';
import { createNavigator, equipStarterGear } from './quest-agent/protocol-play.mjs';
import { startEmergencyHpRecovery } from './quest-agent/protocol-loadout.mjs';
import { createOrdinarySupplies } from './capacity-ordinary-supplies.mjs';
import { loadOrdinaryProtection } from './capacity-ordinary-protection.mjs';
import { preparationStagingCatalogue, selectPreparationStagingPoint } from './capacity-ordinary-staging.mjs';
import { capacityPolicy, capacityActorLimit, validateCapacityPool, LoginBudget, LOGIN_WINDOW_MS, Measurements, assessMeasurements, movementDiagnosticMayContinue,
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
const COMBAT_CHASE_DEFERRAL_LIMIT = 16;
const COMBAT_CHASE_DEFERRAL_MS = 30000;
const COMBAT_CHASE_MAX_MOVES = 40;
const COMBAT_CHASE_MAX_MS = 30000;
const RECENT_CHASE_PREFERENCE_MS = 3000;
const COMBAT_SCOUT_HINT_OBSERVER_LIMIT = 100;
const COMBAT_SCOUT_HINT_ENTITY_LIMIT = 2048;
const COMBAT_SCOUT_HINT_LIMIT = 4;
const COMBAT_SCOUT_SELECTION_ROW_LIMIT = 32;
export const PENDING_ATTACK_PACKET_LIMIT = 64;
export const ATTACK_OUTCOME_CAPABILITY = 'nativeAttackOutcomeV1';
const MAX_ATTACK_REQUEST_ID = 18446744073709551615n;
const ATTACK_RESULT_CODES = { unavailable: new Set(['unsupported', 'executionUnavailable', 'authenticationRequired', 'requestIdNotFresh']),
  unknown: new Set(['outcomeUnknown', 'previousOutcomeUnknown']) };
const ATTACK_TRACE_PACKETS = new Set(['ObjectAttack', 'ObjectWalk', 'ObjectRun', 'ObjectTurn', 'ObjectStruck',
  'DamageIndicator', 'ObjectHealth', 'ObjectDied', 'ObjectRevive', 'ObjectRemove', 'ObjectTeleportOut']);

// Deliberately select public identity/position fields. Never copy a packet or
// snapshot wholesale into the persistent diagnostic stream.
function lifecyclePoint(value) {
  if (!value || !Number.isFinite(value.x) || !Number.isFinite(value.y)) return null;
  return { x: value.x, y: value.y };
}

function attackTraceLife(entity) {
  if (!entity) return null;
  return { ...(Number.isInteger(entity.objectId) ? { objectId: entity.objectId } : {}),
    ...(lifecyclePoint(entity) ?? {}),
    ...(typeof entity.dead === 'boolean' ? { dead: entity.dead } : {}),
    ...Object.fromEntries(['hp', 'maxHp'].filter(key => Number.isFinite(entity[key])).map(key => [key, entity[key]])) };
}

export function attackResultsMode(value = 'legacy') {
  if (!['legacy', 'v2'].includes(value)) throw new Error('Attack results must be legacy or v2');
  return value;
}

function validAttackRequestId(value) {
  return typeof value === 'string' && /^[1-9][0-9]{0,19}$/.test(value) && BigInt(value) <= MAX_ATTACK_REQUEST_ID;
}

export function matchAttackResultV2(events, request) {
  const matches = events.filter(event => event.type === 'attackResultV2' && event.protocolVersion === 2 &&
    event.requestId === request.requestId && event.objectId === request.objectId &&
    validAttackRequestId(event.requestId) && ['launched', 'notLaunched', 'unavailable', 'unknown'].includes(event.result) &&
    (['launched', 'notLaunched'].includes(event.result) ? event.code === undefined : ATTACK_RESULT_CODES[event.result].has(event.code)));
  // Identical cache replays are harmless. Conflicting results are never used
  // to choose the more favourable outcome or to credit damage.
  if (new Set(matches.map(event => `${event.result}:${event.code ?? ''}`)).size > 1) throw new Error('Conflicting correlated attackV2 outcomes');
  return matches[0] ?? null;
}

export function beginPendingAttackEvidence(client, observer, owner, target, sentAtMs, cursor, observerCursor) {
  if (client.pendingAttackEvidence) throw new Error('Only one pending attack evidence window per actor');
  const trace = { actor: client.label, phase: client.context.phase, connection: client.lifecycleConnection,
    sentAtMs, sentAtWallMs: Date.now(), sendCursor: cursor, owner: attackTraceLife(owner), target: attackTraceLife(target),
    receivers: [{ client, cursor, role: 'owner', attackerId: owner.objectId }], packets: [], packetCount: 0, active: true };
  if (observer && observer !== client) trace.receivers.push({ client: observer, cursor: observerCursor,
    role: 'observer', attackerId: observedPlayerId(observer, client.label) });
  for (const receiver of trace.receivers) (receiver.client.pendingAttackPacketTraces ??= new Set()).add(trace);
  client.pendingAttackEvidence = trace;
  return trace;
}

// These receipts are diagnostic only: neither a same-coordinate UserLocation
// nor target movement proves that this particular attack was rejected.
export function recordPendingAttackPacket(client, event) {
  if (event.direction !== 'received' || !['packet', 'attackResultV2'].includes(event.type)) return;
  for (const trace of client.pendingAttackPacketTraces ?? []) {
    const receiver = trace.receivers.find(entry => entry.client === client), payload = event.payload ?? {};
    if (!trace.active || !receiver || event.sequence <= receiver.cursor || event.monotonicMs < trace.sentAtMs) continue;
    if (event.type === 'attackResultV2') {
      if (receiver.role !== 'owner' || event.objectId !== trace.target?.objectId || !validAttackRequestId(event.requestId) ||
          event.protocolVersion !== 2 || !['launched', 'notLaunched', 'unavailable', 'unknown'].includes(event.result)) continue;
      trace.packetCount++;
      if (trace.packets.length < PENDING_ATTACK_PACKET_LIMIT) trace.packets.push({ receiver: 'owner', sequence: event.sequence,
        monotonicMs: event.monotonicMs, atMs: Date.now(), type: 'attackResultV2', protocolVersion: 2,
        requestId: event.requestId, objectId: event.objectId, result: event.result,
        ...(ATTACK_RESULT_CODES[event.result]?.has(event.code) ? { code: event.code } : {}) });
      continue;
    }
    const ownerLocation = receiver.role === 'owner' && event.packet === 'UserLocation';
    if (!ownerLocation && (!ATTACK_TRACE_PACKETS.has(event.packet) ||
        ![receiver.attackerId, trace.target?.objectId].filter(Number.isInteger).includes(payload.objectId))) continue;
    trace.packetCount++;
    if (trace.packets.length >= PENDING_ATTACK_PACKET_LIMIT) continue;
    const projected = { ...Object.fromEntries(['objectId', 'attackerId', 'damage', 'percent', 'hp', 'maxHp']
      .filter(key => Number.isFinite(payload[key])).map(key => [key, payload[key]])),
      ...(lifecyclePoint(payload) ?? {}), ...(lifecyclePoint(payload.location) ? { location: lifecyclePoint(payload.location) } : {}),
      ...(typeof payload.dead === 'boolean' ? { dead: payload.dead } : {}),
      ...(STEPS.some(([, , direction]) => direction === payload.direction) ? { direction: payload.direction } : {}) };
    trace.packets.push({ receiver: receiver.role, sequence: event.sequence, monotonicMs: event.monotonicMs,
      atMs: Date.now(), packet: event.packet, payload: projected });
  }
}

export function finishPendingAttackEvidence(client, context, outcome) {
  const trace = client.pendingAttackEvidence;
  if (!trace) return;
  trace.active = false;
  for (const receiver of trace.receivers) receiver.client.pendingAttackPacketTraces.delete(trace);
  client.pendingAttackEvidence = null;
  const endAtMs = performance.now(), endedAtWallMs = Date.now();
  context.writer?.add({ type: 'pendingAttackEvidence', actor: trace.actor, phase: trace.phase, connection: trace.connection,
    at: endedAtWallMs, sentAtMs: trace.sentAtMs, sentAtWallMs: trace.sentAtWallMs, sendCursor: trace.sendCursor,
    endedAtMs: endAtMs, endedAtWallMs, owner: trace.owner, target: trace.target, outcome,
    ...(trace.requestId ? { protocolVersion: 2, requestId: trace.requestId } : {}),
    maximumPackets: PENDING_ATTACK_PACKET_LIMIT, packetCount: trace.packetCount, truncated: trace.packetCount > trace.packets.length,
    packets: trace.packets, rule: 'Observed public packet fields only; no server incarnation is present or inferred. Diagnostic evidence never changes timeout, miss, rejection or damage accounting.' });
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
    if (context.policy?.activityMode === 'native') {
      context.accountRestorativeUses ??= new Map();
      if (!context.accountRestorativeUses.has(account.accountId)) context.accountRestorativeUses.set(account.accountId, Object.create(null));
      this.journeyRestorativeUseAt = context.accountRestorativeUses.get(account.accountId);
    }
    this.bytes = { sent: 0, received: 0 }; this.recentSecrets = []; this.movementCount = 0; this.targetCursor = 0; this.failedTargets = new Map();
    this.combatPursuit = null; this.chaseDeferrals = new Map();
    this.combatScout = null; this.combatScoutCursor = 0;
    this.evidencePending = new Set(); this.nativeRunPrimedUntil = 0;
    this.lifecycleConnection = context.nextLifecycleConnection = (context.nextLifecycleConnection ?? 0) + 1;
    this.lifecycleNames = new Set([this.label, ...(context.pool?.accounts ?? []).slice(0, capacityActorLimit(context.policy)).map(item => item.name)].filter(name => typeof name === 'string'));
    this.lifecycleIdentities = new Map(); this.lifecycleMissing = new Set(); this.lifecycleOrphans = new Set();
    this.lifecycleRows = 0; this.lifecycleIdentityEvictions = 0; this.lifecycleUnchangedSnapshots = 0;
    this.navigationBlocked = false; this.navigationBlockedRows = 0;
    this.rejectedWalkCells = new Map(); this.movementCorrectionStreak = false; this.movementCorrectionRows = 0;
    this.selfTransformRows = 0;
    this.navigationHint = null; this.navigationClosing = false;
    this.pendingAttackPacketTraces = new Set(); this.pendingAttackEvidence = null;
    this.attackResults = attackResultsMode(context.attackResults);
    this.attackV2Supported = false; this.attackV2Sequence = 0n; this.lastSentAttackV2 = 0n;
  }
  record(direction, value) {
    const preparationBudget = this.preparationCommandBudget;
    if (direction === 'sent' && preparationBudget) {
      if (!Number.isSafeInteger(preparationBudget.sent) || preparationBudget.sent < 0 || preparationBudget.maximum !== 2048 ||
          preparationBudget.sent >= preparationBudget.maximum) throw new Error('Preparation shared2048 command budget exhausted');
      preparationBudget.sent++;
    }
    // Never retain worldSnapshot arrays in event history, nor an ever-growing
    // list of rotated tickets. The latest native ticket remains memory-only.
    const compact = value.type === 'worldSnapshot' ? { type: value.type, payload: { mapFileName: this.snapshot?.mapFileName,
      playerObjectId: this.snapshot?.playerObjectId, entities: self(this) ? [{ ...self(this) }] : [] } } : value;
    const safe = sanitize(compact, this.secrets.concat(this.recentSecrets));
    const event = { ...safe, movementDirection: safe.direction, direction, sequence: ++this.sequence, monotonicMs: performance.now() };
    if (event.type === 'worldSnapshot') event.payload = { mapFileName: this.snapshot?.mapFileName,
      playerObjectId: this.snapshot?.playerObjectId, entities: self(this) ? [{ ...self(this) }] : [] };
    this.events.push(event); if (this.events.length > 2048) this.events.splice(0, 256);
    recordPendingAttackPacket(this, event);
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
    this.ordinarySupplies?.observe(message, this.events.at(-1));
    this.context.sustain?.observe(this, message, this.events.at(-1), Date.now());
    this.context.aoi?.reconcileReceived(this.context.clients ?? [], this.label);
    recordNativeHpRecoveryEvidence(this, message, this.events.at(-1));
    recordPreparationHpRecovery(this, message, this.events.at(-1));
    const navigationHint = this.navigationHint, navigationReceipt = this.events.at(-1);
    if (navigationHint && ((message.packet === 'UserLocation' && navigationReceipt.sequence > navigationHint.sendCursor &&
        navigationReceipt.monotonicMs >= navigationHint.sentAtMs) || ['MapInformation', 'MapChanged'].includes(message.packet) ||
        this.snapshot?.mapFileName !== navigationHint.mapFileName || !lifecyclePoint(self(this)) || hasAuthoritativePlayerDeath(this.snapshot))) {
      releaseNavigationHint(this, navigationHint);
    }
    if (['MapInformation', 'MapChanged'].includes(message.packet)) {
      this.combatPursuit = null; this.chaseDeferrals.clear();
      this.combatScout = null; this.combatScoutCursor = 0;
    }
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
    if (command?.type === 'attack' && this.attackResults === 'v2') throw new Error('Legacy attack is forbidden in attack-results v2 mode');
    if (command?.type === 'attackV2') {
      if (this.attackResults !== 'v2' || !this.attackV2Supported || command.protocolVersion !== 2 ||
          Object.keys(command).sort().join(',') !== 'objectId,protocolVersion,requestId,type' || !validAttackRequestId(command.requestId) ||
          BigInt(command.requestId) <= this.lastSentAttackV2 || !Number.isInteger(command.objectId) || command.objectId <= 0 || command.objectId > 4294967295) {
        throw new Error('Invalid or unsupported attackV2 request; no legacy fallback');
      }
      if (!this.draining) this.context.guard.assert();
      this.budget.reserve(); this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
      this.lastSentAttackV2 = BigInt(command.requestId); this.record('sent', command); this.ws.send(JSON.stringify(command));
      return;
    }
    if (!this.draining) this.context.guard.assert();
    this.budget.reserve(); this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
    PlaytestClient.prototype.send.call(this, command);
  }
  control(command) {
    const v2Capabilities = this.attackResults === 'v2' && command?.type === 'clientCapabilities' &&
      Object.keys(command).sort().join(',') === 'capabilities,type' &&
      JSON.stringify(command.capabilities) === JSON.stringify(['nativeResumeV1', CATALOG_GZIP_CAPABILITY, ATTACK_OUTCOME_CAPABILITY]);
    if (!v2Capabilities) validateResumeControl(command);
    if (!this.draining) this.context.guard.assert();
    this.budget.reserve(); this.bytes.sent += Buffer.byteLength(JSON.stringify(command));
    if (v2Capabilities) {
      this.record('sent', command); this.ws.send(JSON.stringify(command)); this.catalogGzipOptedIn = true;
    } else NativeResumeClient.prototype.control.call(this, command);
  }
  nextAttackV2(objectId) {
    if (!this.attackV2Supported) throw new Error('attackV2 capability not confirmed; no legacy fallback');
    if (++this.attackV2Sequence > MAX_ATTACK_REQUEST_ID) throw new Error('attackV2 request id exhausted; no retry');
    return { type: 'attackV2', protocolVersion: 2, requestId: this.attackV2Sequence.toString(), objectId };
  }
  async pace() {
    while (this.budget.remainingWait()) {
      if (!this.draining) this.context.guard.assert();
      await sleep(Math.min(50, this.budget.remainingWait()));
    }
  }
  async wait(predicate, label, timeoutMs = 15000, deadlinePredicate = null) {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (!this.draining) this.context.guard.assert();
      if (this.failure) throw this.failure;
      const result = predicate(); if (result) return result;
      if (this.closed) throw new Error(`${this.label} transport closed during ${label}`);
      await sleep(20);
    }
    // V2 attacks provide a timestamp-bounded predicate. Read evidence already
    // recorded by the deadline even if this polling timer resumes just after
    // it; do not wait again or change ordinary request timeout semantics.
    if (deadlinePredicate) {
      if (!this.draining) this.context.guard.assert();
      if (this.failure) throw this.failure;
      if (this.closed) throw new Error(`${this.label} transport closed during ${label}`);
      const result = deadlinePredicate(); if (result) return result;
    }
    throw Object.assign(new Error(`${this.label} timed out waiting for ${label}`), {
      code: 'CAPACITY_WAIT_TIMEOUT', waitLabel: label,
    });
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
    this.ws.addEventListener('close', () => releaseNavigationHint(this, this.navigationHint));
    this.ws.addEventListener('message', event => { this.bytes.received += Buffer.byteLength(event.data); });
    try { await promise; } finally { clearInterval(this.timer); }
    const cursor = this.sequence;
    await this.pace(); this.control({ type: 'clientCapabilities', capabilities: ['nativeResumeV1', CATALOG_GZIP_CAPABILITY,
      ...(this.attackResults === 'v2' ? [ATTACK_OUTCOME_CAPABILITY] : [])] });
    if (this.attackResults === 'v2') {
      await this.wait(() => this.events.find(event => event.direction === 'received' && event.sequence > cursor &&
        event.type === 'attackCapabilitiesV2' && event.protocolVersion === 2 && event.capability === ATTACK_OUTCOME_CAPABILITY &&
        event.supported === true && event.scope === 'connection'), 'confirmed attackV2 capability; no legacy fallback', 15000);
      this.attackV2Supported = true;
    }
  }
  async refresh() { await this.pace(); return PlaytestClient.prototype.refresh.call(this); }
  async close() {
    this.navigationClosing = true;
    const hint = this.navigationHint; releaseNavigationHint(this, hint);
    try { return await super.close(); }
    finally { releaseNavigationHint(this, hint); }
  }
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

export function validateNativeExplicitHomes(map, origin, count, anchors, patrolSpan = 2, transfers = [], stationaryObstacles = [], policy = {}) {
  const cells = map?.width * map?.height;
  if (!Number.isInteger(map?.width) || !Number.isInteger(map?.height) || map.width < 1 || map.height < 1 ||
      !Number.isInteger(cells) || cells < 1 || cells > 4_000_000 || map.blocked?.length !== cells ||
      !Number.isInteger(count) || count < 1 || count > capacityActorLimit(policy) || !Array.isArray(anchors) || anchors.length !== count || ![2, 4].includes(patrolSpan)) {
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
      observerHomes: movementActorIndices.map(index => homes[index]), observerNames: new Set(),
      observerBindings: movementActorIndices.map((actorIndex, homeIndex) => ({ actorIndex, homeIndex, home: homes[actorIndex] })) };
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

// Declared homes are immutable geometry. Only an actually active, living
// movement observer can make its own declared home available to this cohort.
// In particular, an explicit cluster with no active observer is closed, not
// the unrestricted null-cluster case. No range-to-fighter filter belongs here.
function activeGameplayOwner(client, mapFileName) {
  if (typeof mapFileName !== 'string' || client.ready !== true || client.inGame !== true || client.closed || client.paused ||
      client.draining || client.navigationClosing || client.snapshot?.mapFileName !== mapFileName) return null;
  const owner = self(client);
  return lifecyclePoint(owner) && !hasAuthoritativePlayerDeath(client.snapshot) ? owner : null;
}
export function activeCombatCoverage(client, context) {
  const cluster = client.combatCluster;
  if (!cluster) return null;
  const entries = (cluster.observerBindings ?? []).flatMap(binding => {
    if (typeof binding.name !== 'string' || typeof client.snapshot.mapFileName !== 'string') return [];
    const observer = context.clients.find(other => other !== client && other.label === binding.name &&
      cluster.observerNames.has(other.label) && other.role === 'movement' && activeGameplayOwner(other, client.snapshot.mapFileName));
    return observer ? [{ ...binding, observer }] : [];
  });
  return { patrolSpan: cluster.patrolSpan, observerHomes: entries.map(entry => entry.home), entries };
}

export function spacingFourScenario(map, origin, center, count, mapFileName, transfers = [], stationaryObstacles = [], policy = {}) {
  if (!Number.isInteger(count) || count < 1 || count > capacityActorLimit(policy) || ![origin, center].every(point => Number.isInteger(point.x) && Number.isInteger(point.y))) throw new Error(`Scenario requires integer coordinates and 1–${capacityActorLimit(policy)} actors`);
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
  const scenario = spacingFourScenario(map, origin, center, Number(options.scenarioCount ?? 50), String(options.scenarioMap), transfers, stationaryObstacles, options);
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
export const NAVIGATION_HINT_MAX_CELLS = 7;
export function movementHintCells(origin, plan) {
  const first = lifecyclePoint(plan?.first), target = lifecyclePoint(plan?.target), start = lifecyclePoint(origin), kind = plan?.command?.type;
  if (!start || !first || !target || !['walk', 'run'].includes(kind)) return null;
  const dx = first.x - start.x, dy = first.y - start.y, steps = kind === 'run' ? 2 : 1;
  if (Math.max(Math.abs(dx), Math.abs(dy)) !== 1 || target.x !== start.x + steps * dx || target.y !== start.y + steps * dy ||
      plan.command.direction !== directionOf(start, first)) return null;
  const cells = new Map([[`${start.x},${start.y}`, start]]);
  const add = point => cells.set(`${point.x},${point.y}`, point);
  for (let step = 1; step <= steps; step++) {
    const before = { x: start.x + (step - 1) * dx, y: start.y + (step - 1) * dy };
    add({ x: start.x + step * dx, y: start.y + step * dy });
    if (dx && dy) { add({ x: before.x + dx, y: before.y }); add({ x: before.x, y: before.y + dy }); }
  }
  return [...cells.values()];
}
// These are conservative tool-local navigation hints, never authoritative
// occupancy or AOI. Keep every original AOI obstacle, including stale peer cells.
export function navigationObstacles(client) {
  const owner = self(client), context = client.context, mapFileName = client.snapshot?.mapFileName;
  const occupied = client.snapshot.entities.filter(entity => entity.objectId !== owner.objectId && !entity.dead).concat(rejectedWalkObstacles(client));
  if (context?.policy?.activityMode !== 'native') return occupied;
  const clients = context?.clients ?? [];
  if (clients.length > capacityActorLimit(context.policy)) throw new Error(capacityActorLimit(context.policy) === 100
    ? 'Controlled navigation cohort exceeds the original100 actor ceiling' : 'Controlled navigation cohort exceeds the declared actor ceiling (500)');
  for (const other of clients) {
    if (other === client) continue; // Every receiver may independently own ID1000.
    const actual = activeGameplayOwner(other, mapFileName);
    if (!actual) continue;
    occupied.push(lifecyclePoint(actual));
    const hint = other.navigationHint;
    if (hint?.mapFileName === mapFileName && hint.connection === other.lifecycleConnection) occupied.push(...hint.cells);
  }
  return occupied;
}
export function navigationPlanIsClear(client, plan, map = client.context.map) {
  if (client.context?.policy?.activityMode !== 'native') return true;
  if (typeof client.snapshot?.mapFileName !== 'string' || (client.context.mapName != null && client.snapshot.mapFileName !== client.context.mapName)) return false;
  const origin = lifecyclePoint(self(client)), cells = movementHintCells(origin, plan);
  if (!cells) return false;
  const occupied = new Set(navigationObstacles(client).map(point => `${point.x},${point.y}`));
  // An ordinary spawn can be shared; as in A*, allow leaving its origin.
  return cells.every(point => (point.x === origin.x && point.y === origin.y) ||
    (protocolMapCellIsWalkable(map, point) && !occupied.has(`${point.x},${point.y}`)));
}
export function claimNavigationHint(client, plan, sentAtMs = performance.now()) {
  if (client.context?.policy?.activityMode !== 'native' || client.navigationHint || !activeGameplayOwner(client, client.snapshot?.mapFileName)) return null;
  const origin = lifecyclePoint(self(client)), cells = movementHintCells(origin, plan);
  if (!cells || cells.length > NAVIGATION_HINT_MAX_CELLS) return null;
  const hint = Object.freeze({ connection: client.lifecycleConnection, mapFileName: client.snapshot.mapFileName, kind: plan.command.type,
    origin: Object.freeze(origin), first: Object.freeze(lifecyclePoint(plan.first)), target: Object.freeze(lifecyclePoint(plan.target)), cells: Object.freeze(cells.map(Object.freeze)),
    sendCursor: client.sequence, sentAtMs });
  client.navigationHint = hint; return hint;
}
export function releaseNavigationHint(client, hint) {
  if (hint && client.navigationHint === hint) client.navigationHint = null;
}
function movement(client, target, map, routeAllowed, planningObstacles) {
  const owner = self(client), occupied = planningObstacles ?? navigationObstacles(client);
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
  if (routeAllowed && !route.every(routeAllowed)) return blocked('route-outside-scout-coverage-or-transfer');
  if (route.length < 2) return blocked('already-at-target');
  const first = route[1];
  if ((client.snapshot.mapTransfers ?? []).some(transfer => inside(first, transfer.bounds))) return blocked('next-step-transfer', first);
  const direction = directionOf(owner, first); if (!direction) return blocked('invalid-unit-step', first);
  const navigation = { goal: lifecyclePoint(target), remainingSteps: route.length - 1 };
  const wantsRun = client.context?.policy.activityMode === 'native' ? performance.now() < client.nativeRunPrimedUntil : client.movementCount % 10 === 9;
  if (wantsRun && route.length > 2 && route[2].x - first.x === first.x - owner.x && route[2].y - first.y === first.y - owner.y &&
      !(client.snapshot.mapTransfers ?? []).some(transfer => inside(route[2], transfer.bounds))) {
    return { command: { type: 'run', direction }, target: route[2], first, navigation };
  }
  return { command: { type: 'walk', direction }, target: first, first, navigation };
}
export function patrol(client, context, home = client.home, routeAllowed) {
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
    const occupied = new Set(navigationObstacles(client).map(entity => `${entity.x},${entity.y}`));
    const clear = point => protocolMapCellIsWalkable(map, point) && !occupied.has(`${point.x},${point.y}`);
    const doorway = point => (client.snapshot.mapTransfers ?? []).some(transfer => inside(point, transfer.bounds));
    for (let offset = 0; offset < points.length; offset++) {
      const index = (initialCursor + offset) % points.length, [dx, dy] = points[index];
      const corner = { x: home.x + dx, y: home.y + dy }, deltaX = corner.x - owner.x, deltaY = corner.y - owner.y;
      const length = Math.max(Math.abs(deltaX), Math.abs(deltaY));
      if (length < 2 || (deltaX && deltaY && Math.abs(deltaX) !== Math.abs(deltaY))) continue;
      const x = Math.sign(deltaX), y = Math.sign(deltaY), target = { x: owner.x + 2 * x, y: owner.y + 2 * y };
      const first = { x: owner.x + x, y: owner.y + y };
      if (!clear(first) || !clear(target) || doorway(first) || doorway(target) ||
          (routeAllowed && (!routeAllowed(owner) || !routeAllowed(first) || !routeAllowed(target)))) continue;
      if (x && y && [{ x: owner.x + x, y: owner.y }, { x: owner.x, y: owner.y + y },
        { x: first.x + x, y: first.y }, { x: first.x, y: first.y + y }].some(point => !clear(point))) continue;
      client.targetCursor = index;
      return { command: { type: 'run', direction: directionOf(owner, first) }, first, target };
    }
  }
  const preferRun = native && performance.now() < client.nativeRunPrimedUntil;
  let firstWalk;
  for (let offset = 0; offset < points.length; offset++) {
    // Native patrol advances exactly one corner. Mutating the starting cursor
    // inside this loop used to skip a corner and endlessly cut across NPCs.
    // Keep the old baseline traversal for comparisons with its prior reports.
    const index = ((context.policy?.activityMode === 'native' ? initialCursor : client.targetCursor) + offset) % points.length, [dx, dy] = points[index];
    const target = { x: home.x + dx, y: home.y + dy };
    if (distance(owner, target) === 0) { client.targetCursor = (index + 1) % points.length; continue; }
    const plan = movement(client, target, map, routeAllowed);
    if (!plan) continue;
    // A detour can leave the actor one step from this corner while another
    // original corner already has two legal straight steps. Inspect only the
    // same four native targets; movement() retains its4000-node path and all
    // collision/transfer/route checks. Never turn a Walk into a forced Run.
    if (!preferRun || plan.command.type === 'run') { client.targetCursor = index; return plan; }
    firstWalk ??= { plan, index };
  }
  if (firstWalk) { client.targetCursor = firstWalk.index; return firstWalk.plan; }
  return null;
}

function recordNavigationBlocked(client, context, phase) {
  if (client.navigationBlocked) return;
  client.navigationBlocked = true;
  const owner = self(client), names = client.lifecycleNames ?? new Set([client.label,
    ...(context.pool?.accounts ?? []).slice(0, capacityActorLimit(context.policy)).map(account => account.name)]);
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
  const coverage = activeCombatCoverage(client, context);
  return nearbyObservers(client, context).filter(other => !coverage || coverage.entries.some(entry => entry.observer === other));
}
function ordinaryCombatMonster(entity, context) {
  return entity?.kind === 'monster' && !entity.dead && Number(entity.hp ?? 1) > 0 &&
    String(entity.disposition ?? '').trim().toLowerCase() !== 'friendly' && !String(entity.ownerName ?? '').trim() &&
    WEAK_MONSTERS.includes(entity.name) && context.monsterNames.includes(entity.name);
}
function availableCombatTarget(client, context, entity, now) {
  return ordinaryCombatMonster(entity, context) && distance(entity, self(client)) <= 16 &&
    (client.failedTargets.get(entity.objectId)?.until ?? 0) <= now &&
    (context.policy?.activityMode !== 'native' || client.chaseDeferrals?.get(entity.objectId)?.mapFileName !== String(client.snapshot.mapFileName) ||
      client.chaseDeferrals.get(entity.objectId).until <= now) &&
    (!context.targetClaims.has(entity.objectId) || context.targetClaims.get(entity.objectId) === client.label);
}
function livePursuitTarget(client, context, pursuit = client.combatPursuit, now = Date.now()) {
  return pursuit?.mapFileName === String(client.snapshot.mapFileName) ? client.snapshot.entities.find(entity =>
    entity.objectId === pursuit.objectId && entity.name === pursuit.name && availableCombatTarget(client, context, entity, now)) : null;
}
export function combatCandidates(client, context, now = Date.now()) {
  const owner = self(client);
  const coverage = activeCombatCoverage(client, context);
  const observed = client.combatCluster ? new Set(combatObservers(client, context).flatMap(other => other.snapshot.entities.map(entity => entity.objectId))) : null;
  return client.snapshot.entities.filter(entity => availableCombatTarget(client, context, entity, now) && insideCombatCoverage(coverage, entity))
    .sort((a, b) => (observed ? Number(observed.has(b.objectId)) - Number(observed.has(a.objectId)) : 0) || distance(a, owner) - distance(b, owner));
}

function newCombatPursuit(client, target, now, origin = self(client), cursor = client.sequence) {
  return { objectId: target.objectId, name: target.name, mapFileName: String(client.snapshot.mapFileName),
    bestDistance: distance(origin, target), lastProgressAt: now, nonImprovingMoves: 0,
    lastReceiptSequence: cursor, attackSent: false, targetPoint: lifecyclePoint(target),
    startedAt: now, activeElapsedMs: 0, confirmedMoves: 0, visited: new Map(),
    coordinationFirstReceiptMonoMs: null, coordinationLastReceiptMonoMs: null };
}
function rememberCombatTarget(client, target, now = Date.now()) {
  const mapFileName = String(client.snapshot.mapFileName);
  if (client.combatPursuit?.objectId !== target.objectId || client.combatPursuit.mapFileName !== mapFileName) {
    client.combatPursuit = newCombatPursuit(client, target, now);
  }
}
function postStrikeChaseSegment(client, context, plan, origin, cursor, now = Date.now()) {
  const previous = client.combatPursuit;
  if (context.policy?.activityMode !== 'native' || !previous?.attackSent ||
      !['walk', 'run'].includes(plan.command.type) || previous.objectId !== plan.chaseTargetId ||
      previous.mapFileName !== String(client.snapshot.mapFileName) || !lifecyclePoint(plan.target) ||
      distance(origin, plan.target) === 0) return null;
  const target = livePursuitTarget(client, context, previous, now);
  return target ? { previous, next: newCombatPursuit(client, target, now, origin, cursor) } : null;
}

// Progress is measured only at a successful, nonzero authoritative movement
// receipt. Selecting a plan, waiting, turning, or a corrected step proves none.
export function recordCombatPursuitMovement(client, context, plan, origin, receipt, status, now = Date.now()) {
  recordCombatScoutMovement(client, context, plan, origin, receipt, status, now);
  const pursuit = client.combatPursuit, actual = receipt?.payload, kind = plan.command.type;
  if (context.policy?.activityMode !== 'native' || !pursuit || pursuit.attackSent || status !== 'success' ||
      !['walk', 'run'].includes(kind) || pursuit.objectId !== plan.chaseTargetId ||
      pursuit.mapFileName !== String(client.snapshot.mapFileName) || receipt.packet !== 'UserLocation' ||
      receipt.sequence <= pursuit.lastReceiptSequence || !lifecyclePoint(actual) || distance(origin, actual) === 0 ||
      !(distance(actual, plan.target) === 0 || (kind === 'run' && distance(actual, plan.first) === 0))) return;
  const target = livePursuitTarget(client, context, pursuit, now);
  if (!target) { client.combatPursuit = null; return; }
  pursuit.lastReceiptSequence = receipt.sequence;
  pursuit.confirmedMoves++;
  pursuit.lastMovementAt = now;
  // Only the authoritative receipt's own clock can establish this optional
  // planning preference. Processing an old ACK does not refresh its age.
  const receiptMonoMs = receipt.monotonicMs, previousMonoMs = pursuit.coordinationLastReceiptMonoMs;
  if (Number.isFinite(receiptMonoMs) && receiptMonoMs >= 0 &&
      (!Number.isFinite(previousMonoMs) || receiptMonoMs >= previousMonoMs)) {
    if (!Number.isFinite(previousMonoMs) || receiptMonoMs - previousMonoMs > RECENT_CHASE_PREFERENCE_MS ||
        !Number.isFinite(pursuit.coordinationFirstReceiptMonoMs) || pursuit.coordinationFirstReceiptMonoMs < 0 ||
        pursuit.coordinationFirstReceiptMonoMs > previousMonoMs) pursuit.coordinationFirstReceiptMonoMs = receiptMonoMs;
    pursuit.coordinationLastReceiptMonoMs = receiptMonoMs;
  } else {
    pursuit.coordinationFirstReceiptMonoMs = null; pursuit.coordinationLastReceiptMonoMs = null;
  }
  const visitKey = `${actual.x},${actual.y}`, visits = (pursuit.visited.get(visitKey) ?? 0) + 1;
  pursuit.visited.set(visitKey, visits);
  const targetStable = distance(target, plan.chaseTargetPoint ?? {}) === 0 && distance(target, pursuit.targetPoint) === 0;
  const navigation = plan.navigation;
  let routeImproved = false;
  if (targetStable && lifecyclePoint(navigation?.goal) && Number.isInteger(navigation.remainingSteps) && navigation.remainingSteps > 0) {
    const consumed = kind === 'run' && distance(actual, plan.first) !== 0 ? 2 : 1;
    const remaining = navigation.remainingSteps - consumed, key = `${target.x},${target.y}:${navigation.goal.x},${navigation.goal.y}`;
    const prior = pursuit.routeProgress;
    if (!prior || prior.key !== key) {
      // Only the first path can use its pre-move length as a baseline. A new
      // goal after replanning cannot manufacture progress on every ACK.
      routeImproved = !prior && !pursuit.routeInitialized && remaining >= 0 && remaining < navigation.remainingSteps;
      pursuit.routeProgress = { key, bestRemaining: remaining };
    } else if (remaining >= 0 && remaining < prior.bestRemaining) {
      routeImproved = true; prior.bestRemaining = remaining;
    }
    pursuit.routeInitialized = true;
  } else pursuit.routeProgress = null;
  pursuit.targetPoint = lifecyclePoint(target);
  const currentDistance = distance(actual, target);
  const improved = currentDistance < pursuit.bestDistance || routeImproved;
  pursuit.bestDistance = Math.min(pursuit.bestDistance, currentDistance);
  if (improved) { pursuit.lastProgressAt = now; pursuit.nonImprovingMoves = 0; }
  else pursuit.nonImprovingMoves++;
  const elapsedMs = now - pursuit.lastProgressAt;
  const pursuitMs = pursuit.activeElapsedMs + Math.max(0, now - pursuit.startedAt);
  const reason = visits >= 3 ? 'route-loop' : pursuit.confirmedMoves >= COMBAT_CHASE_MAX_MOVES ? 'movement-budget' :
    pursuitMs >= COMBAT_CHASE_MAX_MS ? 'time-budget' :
    pursuit.nonImprovingMoves >= 2 && (pursuit.nonImprovingMoves >= 4 || elapsedMs >= 6000) ? 'no-progress' : null;
  if (!reason) return;
  const deferred = client.chaseDeferrals ??= new Map();
  for (const [id, entry] of deferred) if (entry.until <= now || entry.mapFileName !== pursuit.mapFileName) deferred.delete(id);
  if (deferred.size >= COMBAT_CHASE_DEFERRAL_LIMIT) deferred.delete(deferred.keys().next().value);
  deferred.set(pursuit.objectId, { mapFileName: pursuit.mapFileName, until: now + COMBAT_CHASE_DEFERRAL_MS });
  context.writer?.add({ type: 'combatChaseDeferred', actor: client.label, phase: context.phase, at: now,
    targetId: pursuit.objectId, mapFileName: pursuit.mapFileName, nonImprovingMoves: pursuit.nonImprovingMoves,
    elapsedMs, pursuitMs, confirmedMoves: pursuit.confirmedMoves, reason,
    ttlMs: COMBAT_CHASE_DEFERRAL_MS, actual: lifecyclePoint(actual), target: lifecyclePoint(target), attacksSent: 0 });
  client.combatPursuit = null;
}

export function restoreCombatPursuit(original, restored, context, now = Date.now()) {
  restored.combatCluster = original.combatCluster;
  restored.chaseDeferrals = new Map([...(original.chaseDeferrals ?? [])].filter(([, entry]) =>
    entry.until > now && entry.mapFileName === String(restored.snapshot.mapFileName)).slice(-COMBAT_CHASE_DEFERRAL_LIMIT));
  const pursuit = original.combatPursuit;
  const target = livePursuitTarget(restored, context, pursuit, now);
  // Resume retains the choice and attack-written state, but offline time and a
  // previous socket's receipt sequence cannot count as new chase evidence.
  const pausedAt = Math.min(now, Math.max(original.combatPursuitPausedAt ?? now, pursuit?.lastMovementAt ?? 0,
    original.combatScout?.lastMovementAt ?? 0));
  restored.combatPursuit = target ? { ...pursuit, bestDistance: distance(self(restored), target),
    lastProgressAt: now, nonImprovingMoves: 0, lastReceiptSequence: restored.sequence,
    activeElapsedMs: pursuit.activeElapsedMs + Math.max(0, pausedAt - pursuit.startedAt),
    startedAt: now, lastMovementAt: now, targetPoint: lifecyclePoint(target), routeProgress: null,
    coordinationFirstReceiptMonoMs: null, coordinationLastReceiptMonoMs: null,
    visited: new Map(pursuit.visited) } : null;
  const scout = original.combatScout;
  restored.combatScoutCursor = original.combatScoutCursor;
  restored.combatScout = scout?.mapFileName === String(restored.snapshot.mapFileName) ? { ...scout,
    activeElapsedMs: (scout.activeElapsedMs ?? 0) + Math.max(0, pausedAt - scout.startedAt),
    startedAt: now, lastMovementAt: now, lastReceiptSequence: restored.sequence, visited: new Map(scout.visited) } : null;
}

function returnToCombatCoverage(client, context, coverage = activeCombatCoverage(client, context)) {
  const owner = self(client), cluster = client.combatCluster;
  if (!cluster) return patrol(client, context);
  if (!coverage.entries.length) return null;
  const recovering = !insideCombatCoverage(coverage, owner);
  const homes = [client.home, ...coverage.observerHomes].filter(home => insideCombatCoverage(coverage, home));
  const allowed = point => insideCombatCoverage(coverage, point) && !(client.snapshot.mapTransfers ?? []).some(transfer => inside(point, transfer.bounds));
  for (const home of homes.sort((a, b) => distance(a, owner) - distance(b, owner))) {
    const plan = patrol(client, context, home, recovering ? undefined : allowed);
    if (plan && (recovering || (insideCombatCoverage(coverage, plan.first) && insideCombatCoverage(coverage, plan.target)))) return plan;
  }
  return null;
}

function advanceCombatScout(client, context, reason, now) {
  const scout = client.combatScout;
  if (!scout) return;
  client.combatScoutCursor = (scout.homeIndex + 1) % client.combatCluster.observerHomes.length;
  context.writer?.add({ type: 'combatScoutAdvanced', actor: client.label, phase: context.phase, at: now,
    reason, homeIndex: scout.homeIndex, confirmedMoves: scout.confirmedMoves,
    elapsedMs: (scout.activeElapsedMs ?? 0) + now - scout.startedAt });
  client.combatScout = null;
}

export function recordCombatScoutMovement(client, context, plan, origin, receipt, status, now = Date.now()) {
  const scout = client.combatScout, actual = receipt?.payload, kind = plan.command.type;
  if (context.policy?.activityMode !== 'native' || !scout || !plan.scouting || scout.homeIndex !== plan.scoutHomeIndex ||
      scout.mapFileName !== String(client.snapshot.mapFileName) || status !== 'success' || !['walk', 'run'].includes(kind) ||
      receipt?.packet !== 'UserLocation' || receipt.sequence <= scout.lastReceiptSequence || !lifecyclePoint(actual) ||
      distance(origin, actual) === 0 || !(distance(actual, plan.target) === 0 || (kind === 'run' && distance(actual, plan.first) === 0))) return;
  scout.lastReceiptSequence = receipt.sequence; scout.confirmedMoves++; scout.lastMovementAt = now;
  const key = `${actual.x},${actual.y}`, visits = (scout.visited.get(key) ?? 0) + 1; scout.visited.set(key, visits);
  const reason = visits >= 3 ? 'route-loop' : scout.confirmedMoves >= 24 ? 'movement-budget' :
    (scout.activeElapsedMs ?? 0) + now - scout.startedAt >= 20000 ? 'time-budget' :
    distance(actual, scout.goal) === 0 ? 'waypoint-reached' : null;
  if (reason) advanceCombatScout(client, context, reason, now);
}

function preferUnsharedScoutWaypoints(client, context, hints, now, diagnostic) {
  Object.assign(diagnostic, { hintCount: hints.length, activeOtherWaypointCount: null,
    sharedHintCount: null, unsharedHintCount: null, hintPreferenceReason: 'insufficient-hints' });
  if (hints.length < 2) return hints;
  const mapName = client.snapshot.mapFileName, otherWaypoints = new Set();
  for (let index = 0; index < Math.min(context.clients.length, capacityActorLimit(context.policy)); index++) {
    const other = context.clients[index], scout = other.combatScout;
    if (other === client || other.role !== 'combat' || !activeGameplayOwner(other, mapName) || !scout ||
        scout.mapFileName !== mapName || !Number.isFinite(scout.startedAt) || scout.startedAt > now ||
        !Number.isFinite(scout.activeElapsedMs ?? 0) || (scout.activeElapsedMs ?? 0) < 0 ||
        !Number.isInteger(scout.confirmedMoves) || scout.confirmedMoves < 0 || scout.confirmedMoves >= 24 ||
        (scout.activeElapsedMs ?? 0) + now - scout.startedAt >= 20000 || !Number.isInteger(scout.homeIndex)) continue;
    const home = other.combatCluster?.observerHomes?.[scout.homeIndex];
    if (home && Number.isInteger(home.x) && Number.isInteger(home.y)) otherWaypoints.add(home.x + ',' + home.y);
  }
  diagnostic.activeOtherWaypointCount = otherWaypoints.size;
  if (!otherWaypoints.size) {
    diagnostic.sharedHintCount = 0; diagnostic.unsharedHintCount = hints.length;
    diagnostic.hintPreferenceReason = 'no-active-other-waypoints';
    return hints;
  }
  const unshared = [], shared = [];
  for (const hint of hints) {
    const home = client.combatCluster.observerHomes[hint.homeIndex];
    (otherWaypoints.has(home.x + ',' + home.y) ? shared : unshared).push(hint);
  }
  diagnostic.sharedHintCount = shared.length; diagnostic.unsharedHintCount = unshared.length;
  diagnostic.hintPreferenceReason = unshared.length && shared.length ? 'reordered' :
    shared.length ? 'all-hints-shared' : 'all-hints-unshared';
  // A local home index is not a cross-cluster identity. These are same-map
  // declared waypoint coordinates, only a planning preference, never a claim.
  // Preserve original order when no usable distinction exists.
  return unshared.length && shared.length ? [...unshared, ...shared] : hints;
}

function combatScoutHints(client, context, coverage, initialCursor, fallbackHomeIndex, now, allowed, occupied) {
  const hints = [], cluster = client.combatCluster, range = stableFanoutRange('attack', context.policy);
  let truncated = cluster.observerHomes.length > COMBAT_SCOUT_HINT_OBSERVER_LIMIT;
  for (let offset = 0; offset < Math.min(cluster.observerHomes.length, COMBAT_SCOUT_HINT_OBSERVER_LIMIT); offset++) {
    const homeIndex = (initialCursor + offset) % cluster.observerHomes.length;
    if (homeIndex === fallbackHomeIndex) continue;
    const binding = coverage.entries.find(entry => entry.homeIndex === homeIndex);
    if (!binding) continue;
    const observer = activeGameplayOwner(binding.observer, client.snapshot.mapFileName);
    if (!observer) continue;
    const entities = binding.observer.snapshot.entities;
    truncated ||= entities.length > COMBAT_SCOUT_HINT_ENTITY_LIMIT;
    for (let index = 0; index < Math.min(entities.length, COMBAT_SCOUT_HINT_ENTITY_LIMIT); index++) {
      const target = entities[index], point = lifecyclePoint(target), id = target?.objectId;
      if (!Number.isInteger(id) || id <= 0 || id > 0xffffffff || !point || !Number.isInteger(point.x) || !Number.isInteger(point.y) ||
          !ordinaryCombatMonster(target, context) || !allowed(point) || distance(observer, point) > range ||
          (client.failedTargets.get(id)?.until ?? 0) > now ||
          (client.chaseDeferrals?.get(id)?.mapFileName === String(client.snapshot.mapFileName) && client.chaseDeferrals.get(id).until > now) ||
          (context.targetClaims.has(id) && context.targetClaims.get(id) !== client.label)) continue;
      const attackTile = STEPS.map(([dx, dy]) => ({ x: point.x + dx, y: point.y + dy })).find(tile =>
        allowed(tile) && protocolMapCellIsWalkable(context.map, tile) && distance(observer, tile) <= range &&
        !occupied.some(entity => entity.x === tile.x && entity.y === tile.y));
      if (!attackTile) continue;
      // Peer-only knowledge can suggest a waypoint, never add an owner AOI
      // entity or bypass the real two-snapshot witness at attack selection/send.
      hints.push({ homeIndex, objectId: id, target: point, observer: lifecyclePoint(observer), attackTile });
      break;
    }
    if (hints.length === COMBAT_SCOUT_HINT_LIMIT) { truncated = true; break; }
  }
  return { hints, truncated };
}

function recordCombatScoutSelection(client, context, selection, now) {
  if (!context.writer) return;
  let diagnostic = client.combatScoutSelectionDiagnostic;
  if (!diagnostic || diagnostic.phase !== context.phase) diagnostic = client.combatScoutSelectionDiagnostic = { phase: context.phase, rows: 0 };
  if (diagnostic.rows >= COMBAT_SCOUT_SELECTION_ROW_LIMIT) return;
  diagnostic.rows++;
  context.writer.add({ type: 'combatScoutSelection', actor: client.label, phase: context.phase, at: now,
    ...(diagnostic.rows === COMBAT_SCOUT_SELECTION_ROW_LIMIT ? { truncated: true, rowLimit: COMBAT_SCOUT_SELECTION_ROW_LIMIT } :
      { ...selection, truncated: false }) });
}

function scoutCombatCoverage(client, context, now = Date.now(), coverage = activeCombatCoverage(client, context)) {
  const cluster = client.combatCluster, owner = self(client);
  if (!cluster?.observerHomes?.length || !coverage.entries.length || client.pending || !insideCombatCoverage(coverage, owner)) return null;
  if (client.combatScout?.mapFileName !== String(client.snapshot.mapFileName)) client.combatScout = null;
  if (client.combatScout && (client.combatScout.activeElapsedMs ?? 0) + now - client.combatScout.startedAt >= 20000) advanceCombatScout(client, context, 'time-budget', now);
  const occupied = navigationObstacles(client);
  const allowed = point => insideCombatCoverage(coverage, point) && !(client.snapshot.mapTransfers ?? []).some(transfer => inside(point, transfer.bounds));
  const initialCursor = client.combatScout?.homeIndex ?? client.combatScoutCursor ?? 0;
  let searches = 0;
  const findPlan = (homeIndex, hinted = false, hintGoal = null) => {
    const binding = coverage.entries.find(entry => entry.homeIndex === homeIndex);
    if (!binding) return null;
    const home = binding.home;
    // A temporarily inactive declared waypoint must not reset a different
    // waypoint's existing finite clock, ACK count or position-loop budget.
    if (client.combatScout && client.combatScout.homeIndex !== homeIndex) return null;
    const usableGoal = goal => lifecyclePoint(goal) && Number.isInteger(goal.x) && Number.isInteger(goal.y) &&
      distance(goal, owner) > 0 && protocolMapCellIsWalkable(context.map, goal) && allowed(goal) &&
      !occupied.some(entity => entity.x === goal.x && entity.y === goal.y);
    const retainedTarget = !hinted && client.combatScout?.targetWaypoint === true && usableGoal(client.combatScout.goal) ? client.combatScout.goal : null;
    const goals = hinted ? [hintGoal].filter(usableGoal) : STEPS.map(([dx, dy]) => ({ x: home.x + dx, y: home.y + dy }))
      .filter(usableGoal).sort((a, b) => distance(a, owner) - distance(b, owner));
    if (!hinted && client.combatScout?.homeIndex === homeIndex) goals.sort((a, b) => Number(distance(b, client.combatScout.goal) === 0) - Number(distance(a, client.combatScout.goal) === 0));
    // Keep a validated hint waypoint across ACK ticks; failed or invalid goals
    // fall through to the existing home goals within the same search budget.
    if (retainedTarget && !goals.some(goal => distance(goal, retainedTarget) === 0)) goals.unshift(retainedTarget);
    for (const goal of goals) {
      if (searches >= 4) return null;
      searches++;
      const plan = hinted ? movement(client, goal, context.map, allowed, occupied) : movement(client, goal, context.map, allowed);
      if (!plan) continue;
      return { plan, homeIndex, goal, targetWaypoint: hinted || Boolean(retainedTarget && distance(goal, retainedTarget) === 0) };
    }
    return null;
  };
  let fallback = null;
  for (let offset = 0; offset < cluster.observerHomes.length && searches < 4; offset++) {
    fallback = findPlan((initialCursor + offset) % cluster.observerHomes.length);
    if (fallback) break;
  }
  if (fallback) {
    let selected = fallback, hint = null, hintScan = null;
    const fallbackSearches = searches;
    if (!client.combatScout && searches < 4) {
      hintScan = combatScoutHints(client, context, coverage, initialCursor, fallback.homeIndex, now, allowed, occupied);
      const preferredHints = preferUnsharedScoutWaypoints(client, context, hintScan.hints, now, hintScan);
      hintScan.hintOrderChangedByActiveScoutPreference = preferredHints !== hintScan.hints;
      for (const candidate of preferredHints) {
        const preferred = findPlan(candidate.homeIndex, true, candidate.attackTile);
        if (preferred) { selected = preferred; hint = candidate; break; }
        if (searches >= 4) break;
      }
    }
    const { plan, homeIndex, goal, targetWaypoint } = selected;
    if (!client.combatScout) {
      client.combatScout = { mapFileName: String(client.snapshot.mapFileName), homeIndex, goal, startedAt: now,
        activeElapsedMs: 0, confirmedMoves: 0, lastReceiptSequence: client.sequence, visited: new Map(),
        ...(targetWaypoint ? { targetWaypoint: true } : {}) };
    } else {
      client.combatScout.goal = goal;
      if (!targetWaypoint) delete client.combatScout.targetWaypoint;
    }
    client.combatScoutCursor = homeIndex;
    if (hintScan?.hints.length) recordCombatScoutSelection(client, context, {
      reason: hint ? 'observer-target-waypoint' : 'cyclic-fallback', fallbackHomeIndex: fallback.homeIndex,
      selectedHomeIndex: homeIndex, selectedWaypoint: { x: goal.x, y: goal.y },
      selectedWaypointSource: targetWaypoint ? 'hint-attack-tile' : 'declared-home',
      pathSearches: searches, hintPathSearches: searches - fallbackSearches,
      hintScanTruncated: hintScan.truncated, hintOrderChangedByActiveScoutPreference: hintScan.hintOrderChangedByActiveScoutPreference,
      hintCount: hintScan.hintCount, activeOtherWaypointCount: hintScan.activeOtherWaypointCount,
      sharedHintCount: hintScan.sharedHintCount, unsharedHintCount: hintScan.unsharedHintCount, hintPreferenceReason: hintScan.hintPreferenceReason,
      ...(hint ? { hintTargetId: hint.objectId, hintTarget: hint.target, actualObserver: hint.observer,
        potentialAttackTile: hint.attackTile } : {}) }, now);
    return { ...plan, scouting: true, scoutHomeIndex: homeIndex };
  }
  // Failed paths never become movement evidence. The next legal opportunity
  // can consider another finite observer waypoint while normal fallback keeps
  // its original blocked/latency/offered-load accounting.
  client.combatScoutCursor = (initialCursor + 1) % cluster.observerHomes.length;
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
function nativeAttackWitness(client, context, target, owner = self(client)) {
  const range = stableFanoutRange('attack', context.policy);
  return combatObservers(client, context).map(observer => {
    const peer = activeGameplayOwner(observer, client.snapshot.mapFileName);
    const attackerId = observedPlayerId(observer, client.label);
    const attacker = observer.snapshot.entities.find(entity => entity.objectId === attackerId && entity.name === client.label && entity.dead !== true);
    const observedTarget = observer.snapshot.entities.find(entity => entity.objectId === target.objectId &&
      entity.name === target.name && ordinaryCombatMonster(entity, context));
    const separation = peer ? Math.max(distance(peer, owner), distance(peer, target)) : Infinity;
    return peer && attacker && observedTarget && Number.isInteger(attackerId) && separation <= range
      ? { observer, separation } : null;
  }).filter(Boolean).sort((a, b) => a.separation - b.separation)[0]?.observer ?? null;
}
function recentNativePursuitLeaders(context, mapFileName, nowMonoMs) {
  const leaders = new Map();
  if (!Number.isFinite(nowMonoMs) || nowMonoMs < 0) return leaders;
  for (const hunter of context.clients) {
    if (hunter.role !== 'combat' || typeof hunter.label !== 'string' || !activeGameplayOwner(hunter, mapFileName)) continue;
    const pursuit = hunter.combatPursuit;
    if (!pursuit || pursuit.attackSent || pursuit.mapFileName !== mapFileName) continue;
    const first = pursuit.coordinationFirstReceiptMonoMs, last = pursuit.coordinationLastReceiptMonoMs;
    if (!Number.isFinite(first) || !Number.isFinite(last) || first < 0 || first > last ||
        last > nowMonoMs || nowMonoMs - last > RECENT_CHASE_PREFERENCE_MS) continue;
    const target = hunter.snapshot.entities.find(entity => entity.objectId === pursuit.objectId && entity.name === pursuit.name);
    if (!target || !availableCombatTarget(hunter, context, target, Date.now())) continue;
    const prior = leaders.get(pursuit.objectId);
    if (!prior || first < prior.firstReceiptMonoMs || (first === prior.firstReceiptMonoMs && hunter.label < prior.label)) {
      leaders.set(pursuit.objectId, { label: hunter.label, firstReceiptMonoMs: first });
    }
  }
  return leaders;
}

function combatPlan(client, context, nowMonoMs = performance.now()) {
  const owner = self(client), native = context.policy?.activityMode === 'native';
  if (context.policy?.activityMode === 'native' && !client.combatArrival) {
    if (distance(owner, client.home) !== 0) {
      const plan = movement(client, client.home, context.map);
      return plan ? { ...plan, initialCombatTransit: true } : null;
    }
    recordCombatArrival(client, context);
  }
  const coverage = activeCombatCoverage(client, context);
  if (!insideCombatCoverage(coverage, owner)) return returnToCombatCoverage(client, context, coverage);
  for (const [id, failure] of client.failedTargets) if (failure.until < Date.now() - 30000) client.failedTargets.delete(id);
  let candidates = combatCandidates(client, context);
  const observers = combatObservers(client, context);
  const witness = target => native ? nativeAttackWitness(client, context, target) :
    observers.find(other => other.snapshot.entities.some(entity => entity.objectId === target.objectId));
  const approaches = native ? new Map() : null;
  if (native) {
    // An approach must lead to a genuinely witnessed strike, rather than
    // orbiting an adjacent monster that no current observer can witness.
    // These eight cheap geometry/membership checks do not expand path budgets.
    candidates = candidates.filter(target => {
      const destinations = STEPS.map(([dx, dy]) => ({ x: target.x + dx, y: target.y + dy }))
        .filter(destination => insideCombatCoverage(coverage, destination) && nativeAttackWitness(client, context, target, destination));
      approaches.set(target.objectId, destinations);
      return destinations.length > 0;
    });
    const pursuit = client.combatPursuit;
    const selected = pursuit?.mapFileName === String(client.snapshot.mapFileName)
      ? candidates.find(target => target.objectId === pursuit.objectId && target.name === pursuit.name) : null;
    if (!selected && !livePursuitTarget(client, context, pursuit)) client.combatPursuit = null;
    const adjacent = candidates.filter(target => distance(owner, target) === 1 && witness(target));
    candidates = [...new Set([...(selected && adjacent.includes(selected) ? [selected] : []), ...adjacent,
      ...(selected ? [selected] : []), ...candidates])];
    // Reorder only the original four-candidate window. A claimed target with
    // the only usable path remains a fallback within the same search budget.
    // This is a group planning preference, never an exclusive combat claim.
    const leaders = recentNativePursuitLeaders(context, client.snapshot.mapFileName, nowMonoMs);
    const immediate = new Set(adjacent.map(target => target.objectId));
    const priority = target => immediate.has(target.objectId) ? 0 :
      leaders.has(target.objectId) && leaders.get(target.objectId).label !== client.label ? 2 : 1;
    candidates = candidates.slice(0, 4).sort((a, b) => priority(a) - priority(b));
  }
  for (const target of candidates.slice(0, 4)) {
    const observer = witness(target);
    if (distance(owner, target) === 1 && observer) {
      if (native && !client.pending) rememberCombatTarget(client, target);
      const facing = directionOf(owner, target);
      if (owner.direction !== facing) return { command: { type: 'turn', direction: facing }, hunting: target.name };
      return { command: { type: 'attack', objectId: target.objectId }, targetEntity: target, observer, hunting: target.name,
        peerAttackerId: observedPlayerId(observer, client.label) };
    }
    const adjacent = (native ? approaches.get(target.objectId) : STEPS.map(([dx, dy]) => ({ x: target.x + dx, y: target.y + dy })))
      .sort((a, b) => distance(a, owner) - distance(b, owner));
    for (const destination of adjacent) {
      if (!insideCombatCoverage(coverage, destination)) continue;
      const plan = movement(client, destination, context.map, point => insideCombatCoverage(coverage, point) &&
        !(client.snapshot.mapTransfers ?? []).some(transfer => inside(point, transfer.bounds)));
      if (plan && insideCombatCoverage(coverage, plan.first) && insideCombatCoverage(coverage, plan.target)) {
        if (native && !client.pending) rememberCombatTarget(client, target);
        return { ...plan, hunting: target.name, ...(native ? { chaseTargetId: target.objectId, chaseTargetPoint: lifecyclePoint(target) } : {}) };
      }
    }
    // A temporarily unavailable path must not erase real chase ACKs. Otherwise
    // chase / fallback-patrol / chase can revisit one cell forever while each
    // replan resets the loop and finite pursuit budgets.
    if (native && !client.pending && client.combatPursuit?.objectId === target.objectId &&
        client.combatPursuit.confirmedMoves === 0) client.combatPursuit = null;
  }
  return (native ? scoutCombatCoverage(client, context, Date.now(), coverage) : null) ?? returnToCombatCoverage(client, context, coverage);
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
  return client.role === 'combat' ? combatPlan(client, context, now) : patrol(client, context);
}

export async function action(client, context, planned, preparedPlan) {
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
  if (native && kind === 'attack') {
    const target = client.snapshot.entities.find(entity => entity.objectId === command.objectId && ordinaryCombatMonster(entity, context));
    const observer = target && nativeAttackWitness(client, context, target);
    if (!observer) {
      context.add({ actor: client.label, phase, kind, status: 'evidence-wait', plannedGameplay: true,
        attackWitnessUnavailable: true, ...nativeMeta }); return;
    }
    // Re-resolve both the receiver's actor namespace and its cursor immediately
    // before this send. A prepared observer may have moved since selection.
    plan = { ...plan, targetEntity: target, observer, peerAttackerId: observedPlayerId(observer, client.label) };
  }
  const observers = nearbyObservers(client, context, stableFanoutRange(kind, context.policy)).map(peer => ({ peer, cursor: peer.sequence,
    objectId: observedPlayerId(peer, client.label) }));
  const observerCursor = plan.observer?.sequence, cursor = client.sequence, sentAt = performance.now();
  if (sentAt - plannedAt > context.policy.maximumSendLatenessMs) {
    context.add({ actor: client.label, phase, kind, status: 'plan-gap', plannedGameplay: true, sendLatenessMs: sentAt - plannedAt, ...nativeMeta }); return;
  }
  if (kind === 'attack') context.targetClaims.set(command.objectId, client.label);
  const row = { actor: client.label, phase, kind, plannedGameplay: true, scheduledAtMs: plannedAt, sentAtMs: sentAt,
    plannedTarget: plan.target, targetId: command.objectId, hunting: plan.hunting, chaseTargetId: plan.chaseTargetId,
    ...(plan.scouting ? { scouting: true, scoutHomeIndex: plan.scoutHomeIndex } : {}), ...nativeMeta,
    ...(native && client.role === 'combat' ? { declaredHome: client.home, initialCombatTransit: Boolean(plan.initialCombatTransit),
      targetPosition: plan.targetEntity ? { x: plan.targetEntity.x, y: plan.targetEntity.y } : undefined } : {}) };
  const v2Attack = kind === 'attack' && client.attackResults === 'v2';
  if (v2Attack) row.attackResults = 'v2';
  let attackOutcome = 'unresolved', navigationHint;
  try {
    if (native && ['walk', 'run'].includes(kind)) {
      if (!navigationPlanIsClear(client, plan, context.map) || !(navigationHint = claimNavigationHint(client, plan, sentAt))) {
        context.add({ ...row, status: 'navigation-blocked', navigationCoordinationConflict: true }); return;
      }
    }
    client.waitReason = ['attack', 'chat'].includes(kind) ? 'evidence-wait' : 'owner-ack-wait';
    if (kind === 'attack') beginPendingAttackEvidence(client, plan.observer, owner,
      plan.targetEntity ?? client.snapshot.entities.find(entity => entity.objectId === command.objectId), sentAt, cursor, observerCursor);
    const wireCommand = v2Attack ? client.nextAttackV2(command.objectId) : command;
    if (v2Attack) { row.requestId = wireCommand.requestId; client.pendingAttackEvidence.requestId = wireCommand.requestId; }
    // A launched strike ends its previous chase. Only a new, actually sent
    // movement can begin another segment; selecting or rejecting a plan cannot.
    const postStrike = postStrikeChaseSegment(client, context, plan, owner, cursor);
    client.send(wireCommand);
    if (postStrike && client.combatPursuit === postStrike.previous) client.combatPursuit = postStrike.next;
    if (v2Attack) row.attackRequestSent = true;
    if (native && kind === 'attack') {
      rememberCombatTarget(client, plan.targetEntity);
      if (!v2Attack) client.combatPursuit.attackSent = true;
    }
    if (native && kind === 'attack') recordHuntAttempt(client, owner, row.targetPosition);
    if (native) {
      client.plan.sent(sentAt, planned.cadence.cooldownMs);
      if (kind === 'chat') client.nextNativeChatAt = sentAt + context.policy.nativeChatIntervalMs;
      if (kind === 'turn') client.nextNativeTurnAt = sentAt + context.policy.nativeTurnIntervalMs;
    }
    const acknowledgement = () => {
      const own = fresh(client, cursor, sentAt).filter(event => !v2Attack || event.monotonicMs <= sentAt + 900);
      const rejection = own.find(event => event.type === 'error');
      if (rejection) { const error = new Error('Public gameplay command rejected'); error.code = 'PUBLIC_COMMAND_REJECTED'; throw error; }
      if (kind === 'attack') {
        if (v2Attack) {
          const result = matchAttackResultV2(own, wireCommand);
          if (!result) return null;
          row.attackResult = result.result;
          if (['unavailable', 'unknown'].includes(result.result)) {
            const error = new Error(`attackV2 ${result.result}: ${result.code}; no retry`);
            error.terminalAttackResult = true; throw error;
          }
          if (result.result === 'notLaunched') return { ...result, notLaunched: true };
          if (native && client.combatPursuit?.objectId === command.objectId) client.combatPursuit.attackSent = true;
        }
        return verifiedCombatHit(own, fresh(plan.observer, observerCursor, sentAt)
          .filter(event => !v2Attack || event.monotonicMs <= sentAt + 900), command.objectId, owner.objectId, plan.peerAttackerId);
      }
      if (kind === 'chat') {
        const peer = observers[0]?.peer ?? client, after = observers[0]?.cursor ?? cursor;
        return fresh(peer, after, sentAt).find(event => ['ObjectChat', 'Chat'].includes(event.packet) &&
          String(event.payload?.text ?? event.payload?.message ?? '').includes(command.message));
      }
      return own.find(event => event.packet === 'UserLocation');
    };
    const receipt = await client.wait(acknowledgement, `${kind} authoritative acknowledgement`,
      kind === 'attack' ? 900 : context.policy.actionTimeoutMs, v2Attack ? acknowledgement : null);
    let status = receipt.notLaunched ? 'not-launched' : 'success';
    if (['walk', 'run'].includes(kind)) {
      const actual = receipt.payload;
      if (!(actual.x === plan.target.x && actual.y === plan.target.y) &&
          !(kind === 'run' && actual.x === plan.first.x && actual.y === plan.first.y)) status = 'corrected';
      if (status === 'success') { client.movementCount++; client.nativeRunPrimedUntil = sentAt + 1200; client.navigationBlocked = false; client.movementCorrectionStreak = false; }
      else if (native) row.temporaryAvoidance = recordMovementCorrection(client, context, owner, plan, actual, phase, receipt);
      row.actual = { x: actual.x, y: actual.y }; row.direction = actual.direction;
      row.movedCells = distance(owner, actual);
      recordCombatPursuitMovement(client, context, plan, owner, receipt, status);
      recordCombatArrival(client, context);
    } else if (kind === 'turn' && receipt.payload?.direction !== command.direction) status = 'corrected';
    context.add({ ...row, status, latencyMs: receipt.monotonicMs - sentAt, plannedLatencyMs: receipt.monotonicMs - plannedAt,
      damage: receipt.damage, sharedHealthVerified: receipt.sharedHealthVerified, healthPercent: receipt.healthPercent });
    if (kind === 'attack') {
      attackOutcome = status;
      if (status === 'success') {
        client.failedTargets.delete(command.objectId);
        if (native) { client.combatHunting.positiveHits++; client.combatHunting.totalDamage += receipt.damage; }
      }
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
    const swung = kind === 'attack' && (v2Attack ? row.attackResult === 'launched' :
      fresh(client, cursor, sentAt).some(event => event.packet === 'ObjectAttack' && event.payload?.objectId === owner.objectId));
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
    attackOutcome = actionFailureStatus(error, swung, client.closed);
    context.add({ ...row, status: attackOutcome,
      latencyMs: performance.now() - sentAt, plannedLatencyMs: performance.now() - plannedAt, error: sanitize(error.message, client.secrets),
      targetStillAdjacent: kind === 'attack' ? client.snapshot.entities.some(entity => entity.objectId === command.objectId && !entity.dead && distance(entity, self(client)) === 1) : undefined,
      ambiguousTargetQuarantined: kind === 'attack' && native && !(error instanceof SafetyStop) });
    if (error instanceof SafetyStop || client.closed || error.terminalAttackResult) throw error;
  } finally {
    releaseNavigationHint(client, navigationHint);
    client.waitReason = null;
    if (kind === 'attack') {
      try { finishPendingAttackEvidence(client, context, attackOutcome); }
      finally { if (context.targetClaims.get(command.objectId) === client.label) context.targetClaims.delete(command.objectId); }
    }
  }
}

async function auxiliary(client, context) {
  if (client.hpRecoveryPending || client.hpRecoveryEvidence) return;
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

// Temporary preparation-only observer: retain one latest raw scalar, never a
// complete snapshot or packet history. Native maintenance keeps its own rules.
function preparationItemTemplate(item) {
  const source = item?.tooltipSource;
  const ids = [item?.itemIndex, item?.item_index, source?.info?.itemIndex, source?.info?.item_index,
    source?.realInfo?.itemIndex, source?.realInfo?.item_index, source?.real_info?.itemIndex, source?.real_info?.item_index].filter(value => value !== undefined);
  if (!ids.length || ids.some(value => !Number.isSafeInteger(value) || value <= 0)) throw new Error('Preparation HP recovery inventory template is unknown');
  // GetRealItem may legitimately select a different class/level row (e.g.
  // SpiritBlade info1 -> realInfo3). Resolve it like the ordinary helper.
  const resolved = source?.realInfo ?? source?.real_info ?? source?.info;
  const effective = item?.itemIndex ?? item?.item_index ?? resolved?.item_index ?? resolved?.itemIndex;
  if (!Number.isSafeInteger(effective) || effective <= 0 ||
      (item.name === '(HP)DrugSmall' && ids.some(value => value !== 658))) throw new Error('Preparation HP recovery inventory template is unknown or HP658 identity conflicts');
  return effective;
}
function preparationHpScalar(raw, expected, receipt, selected) {
  if (!raw || raw.playerObjectId !== expected.ownerObjectId || raw.mapFileName !== expected.mapFileName) throw new Error('Preparation HP recovery owner or map changed');
  const owner = raw.entities?.find(e => e.objectId === raw.playerObjectId);
  if (!owner || owner.name !== expected.ownerName) throw new Error('Preparation HP recovery owner changed');
  if (owner.dead !== false || raw.playerHp === 0) throw new Error('Preparation HP recovery owner is dead or life state is unknown');
  if (!Number.isSafeInteger(raw.playerHp) || raw.playerHp <= 0 || !Number.isSafeInteger(raw.playerMaxHp) || raw.playerMaxHp <= 0 || raw.playerHp > raw.playerMaxHp ||
      !Array.isArray(raw.inventoryItems) || !Array.isArray(raw.beltItems)) throw new Error('Preparation HP recovery needs exact raw HP and complete inventory plus belt');
  const candidates = [], matching = [], ids = new Set(), slots = new Set();
  for (const [grid, items] of [['belt', raw.beltItems], ['inventory', raw.inventoryItems]]) for (const item of items) {
    if (!item || typeof item !== 'object' || Array.isArray(item)) throw new Error('Preparation HP recovery inventory record is malformed');
    const uidValue = item.uniqueId;
    const uid = typeof uidValue === 'number' ? uidValue : typeof uidValue === 'string' && /^(0|[1-9]\d*)$/.test(uidValue) ? Number(uidValue) : NaN;
    const quantity = item.quantity, template = preparationItemTemplate(item);
    if (!Number.isSafeInteger(uid) || uid < 0 || ids.has(uid) || !Number.isSafeInteger(quantity) || quantity <= 0 || quantity > 0xffff_ffff ||
        typeof item.name !== 'string' || !item.name.length || !Number.isSafeInteger(item.slot) || item.slot < 0 || item.slot > 255 ||
        !(grid === 'belt' ? item.container === 'belt' : ['bag1', 'bag2', 'quest'].includes(item.container))) throw new Error('Preparation HP recovery inventory UID quantity or container is malformed');
    const slotKey = item.container + ':' + item.slot;
    if (slots.has(slotKey)) throw new Error('Preparation HP recovery duplicate inventory slot');
    ids.add(uid); slots.add(slotKey);
    if (selected && uid === selected.uniqueId) matching.push({ ...item, template });
    if (!/^\(hp\)drug/i.test(item.name)) continue;
    if (item.container === 'quest') throw new Error('Preparation HP recovery HP item is not ordinarily usable');
    candidates.push({ uniqueId: uid, quantity, grid, exact658: template === 658 && item.name === '(HP)DrugSmall' });
  }
  let selectedQuantity = null;
  if (selected) {
    const item = matching[0];
    if (item && (item.template !== 658 || item.name !== '(HP)DrugSmall')) throw new Error('Preparation HP recovery held UID changed');
    selectedQuantity = item ? item.quantity : 0;
  }
  return { ownerObjectId: raw.playerObjectId, ownerName: owner.name, map: raw.mapFileName,
    hp: raw.playerHp, maximumHp: raw.playerMaxHp, sequence: receipt.sequence, monotonicMs: receipt.monotonicMs,
    hpCandidateCount: candidates.length, candidate: candidates.length === 1 ? candidates[0] : null, selectedQuantity };
}
function recordPreparationHpRecovery(client, message, receipt) {
  const pending = client.preparationHpRecoveryObservation;
  if (!pending || receipt?.direction !== 'received' || receipt.sequence <= pending.cursor || receipt.monotonicMs < pending.startedAtMs) return;
  if (receipt.monotonicMs > (pending.sentAtMs === null ? pending.beforeDeadlineMs : pending.deadlineMs)) return;
  try {
    const owner = self(client);
    if (client.closed || !owner || owner.objectId !== pending.ownerObjectId || owner.name !== pending.ownerName ||
        client.snapshot.mapFileName !== pending.mapFileName) throw new Error('Preparation HP recovery owner or transport changed');
    if (hasAuthoritativePlayerDeath(client.snapshot)) throw new Error('Preparation HP recovery owner is dead');
    if (message.type === 'error') throw new Error('Preparation HP recovery public error');
    if (message.type === 'worldSnapshot') {
      const scalar = preparationHpScalar(message.payload, pending, receipt, pending.selected);
      if (pending.sentAtMs === null) pending.before = scalar;
      else { pending.after = scalar; pending.afterInvalidated = false; }
    }
    if (pending.sentAtMs !== null && message.packet === 'UseItem' && Number(message.payload?.uniqueId) === pending.selected.uniqueId &&
        String(message.payload?.grid).toLowerCase() === pending.selected.grid) {
      if (message.payload.success !== true) throw new Error('Preparation HP recovery UseItem rejected');
      pending.acknowledgement ??= { sequence: receipt.sequence, monotonicMs: receipt.monotonicMs };
    }
    // Ordinary potions emit self ObjectHealth percentages, not exact personal
    // HealthChanged. Either is only a cue for one raw refresh, never HP proof.
    if (pending.sentAtMs !== null && (message.packet === 'HealthChanged' ||
        (message.packet === 'ObjectHealth' && message.payload?.objectId === pending.ownerObjectId))) {
      pending.afterInvalidated = true;
      pending.healthCue = { source: message.packet, sequence: receipt.sequence, monotonicMs: receipt.monotonicMs };
    }
  } catch (error) { pending.failure ??= error; }
}
export async function recoverPreparationHeldHp(client, context, { actorDeadlineMs = performance.now() + 600000 } = {}) {
  const began = performance.now(), row = { type: 'preparationHpRecovery', actor: client.label, phase: context.phase,
    preparationOnly: true, measurementCredit: false, hpThreshold: .7, maximumDoses: 1, evidenceWindowMs: 2000 };
  let pending = null;
  const check = () => {
    context.guard.assert();
    if (context.phase !== 'preparation' || client.ready !== false || !client.inGame || client.closed || client.paused || client.draining) throw new Error('Preparation HP recovery requires an inactive ordinary owner');
    if (!Number.isFinite(actorDeadlineMs) || performance.now() >= actorDeadlineMs || performance.now() - began >= 600000) throw new Error('Preparation actor600s budget exhausted');
    if (pending?.failure) throw pending.failure;
  };
  const waitFor = async (predicate, label, deadline) => {
    const read = () => { check(); return predicate(); };
    const remaining = deadline - performance.now();
    if (!(remaining > 0)) throw new Error('Preparation HP recovery2s evidence deadline exhausted');
    return client.wait(read, label, remaining, read);
  };
  try {
    check();
    if (client.pending || client.auxPending || client.hpRecoveryPending || client.hpRecoveryEvidence || client.preparationHpRecoveryObservation) throw new Error('Preparation HP recovery cannot overlap another action');
    const owner = self(client);
    if (!owner || owner.name !== client.label || client.snapshot?.mapFileName !== '0' || hasAuthoritativePlayerDeath(client.snapshot)) throw new Error('Preparation HP recovery requires a live same-owner map0 snapshot');
    pending = { ownerObjectId: owner.objectId, ownerName: owner.name, mapFileName: '0', cursor: client.sequence,
      startedAtMs: performance.now(), sentAtMs: null, deadlineMs: null, before: null, after: null, selected: null, failure: null };
    client.preparationHpRecoveryObservation = pending;
    await client.pace(); check(); pending.before = null; pending.cursor = client.sequence; pending.startedAtMs = performance.now();
    pending.beforeDeadlineMs = Math.min(actorDeadlineMs, pending.startedAtMs + 2000);
    client.send({ type: 'clientVersion' });
    await waitFor(() => pending.before, 'fresh raw preparation HP snapshot', pending.beforeDeadlineMs);
    const before = pending.before; row.before = { ...before };
    if (before.hp / before.maximumHp >= .7) {
      const result = { ...row, at: Date.now(), status: 'not-needed', doses: 0, elapsedMs: performance.now() - began };
      client.preparationHpRecoveryResult = result; context.writer?.add(result); return result;
    }
    if (before.hpCandidateCount !== 1 || !before.candidate?.exact658) throw new Error('Preparation HP recovery requires one unambiguous held HP658 DrugSmall UID');
    await client.pace(); check();
    const live = self(client), item = before.candidate;
    if (!live || live.objectId !== pending.ownerObjectId || live.name !== pending.ownerName || client.snapshot.mapFileName !== '0' || hasAuthoritativePlayerDeath(client.snapshot)) throw new Error('Preparation HP recovery owner changed before dose');
    // Re-evaluate the same raw observation after pacing; newer complete raw
    // inventory must still identify the exact item selected before sending.
    const current = pending.before;
    if (performance.now() - current.monotonicMs <= 2000 && current.hp / current.maximumHp >= .7 && current.maximumHp === before.maximumHp) {
      const result = { ...row, before: { ...current }, at: Date.now(), status: 'not-needed', doses: 0, naturalRecoveryObserved: true, elapsedMs: performance.now() - began };
      client.preparationHpRecoveryResult = result; context.writer?.add(result); return result;
    }
    if (performance.now() - current.monotonicMs > 2000 || current.hpCandidateCount !== 1 || !current.candidate?.exact658 || current.candidate.uniqueId !== item.uniqueId || current.candidate.grid !== item.grid ||
        current.candidate.quantity !== item.quantity || current.hp !== before.hp || current.maximumHp !== before.maximumHp) throw new Error('Preparation HP recovery state changed before dose');
    // The generic emergency selector reads the cache. Check its full current
    // input before sending, while keeping raw receipts as the only proof.
    const cached = preparationHpScalar(client.snapshot, pending, { sequence: client.sequence, monotonicMs: performance.now() }, null);
    if (cached.hpCandidateCount !== 1 || !cached.candidate?.exact658 || cached.candidate.uniqueId !== item.uniqueId || cached.candidate.grid !== item.grid ||
        cached.candidate.quantity !== item.quantity || cached.hp !== before.hp || cached.maximumHp !== before.maximumHp) throw new Error('Preparation HP recovery selector input changed before dose');
    pending.selected = item; pending.cursor = client.sequence; pending.sentAtMs = performance.now();
    pending.deadlineMs = Math.min(actorDeadlineMs, pending.sentAtMs + 2000); row.sentAtMs = pending.sentAtMs;
    const dose = startEmergencyHpRecovery(client, { hpThreshold: .7, restorativeReuseDelayMs: 6000, now: pending.sentAtMs })[0];
    if (!dose || Number(dose.uniqueId) !== item.uniqueId || dose.command.grid !== item.grid) throw new Error('Preparation HP recovery held-item selector declined or changed');
    row.uniqueId = item.uniqueId; row.grid = item.grid; row.quantityBefore = item.quantity;
    const complete = () => {
      const after = pending.after;
      return pending.acknowledgement && after && !pending.afterInvalidated && after.sequence > pending.cursor && after.monotonicMs >= pending.sentAtMs && after.monotonicMs <= pending.deadlineMs &&
        after.maximumHp === before.maximumHp && after.hp > before.hp && after.hp / after.maximumHp >= .7 && after.selectedQuantity === item.quantity - 1;
    };
    await waitFor(() => complete() || (pending.acknowledgement && pending.healthCue), 'preparation HP acknowledgement and health refresh cue', pending.deadlineMs);
    if (!complete()) {
      await client.pace(); check();
      if (performance.now() >= pending.deadlineMs) throw new Error('Preparation HP recovery2s evidence deadline exhausted');
      row.rawRefreshCue = { ...pending.healthCue };
      client.send({ type: 'clientVersion' }); // At most one post-dose snapshot; the proof clock is never restarted.
      await waitFor(complete, 'preparation raw HP increase and exact held consumption', pending.deadlineMs);
    }
    const result = { ...row, at: Date.now(), status: 'verified', doses: 1, acknowledgement: pending.acknowledgement,
      after: { ...pending.after }, consumedQuantity: 1, observedExactHpIncrease: pending.after.hp - before.hp,
      proofMeaning: 'Both actual consumption and exact HP increase observed; no causal healing or measurement credit claimed.', elapsedMs: performance.now() - began };
    client.preparationHpRecoveryResult = result; context.writer?.add(result); return result;
  } catch (error) {
    const result = { ...row, at: Date.now(), status: 'failed', failure: sanitize(error.message, client.secrets ?? []),
      acknowledgement: pending?.acknowledgement ?? null, after: pending?.after ?? null, elapsedMs: performance.now() - began };
    client.preparationHpRecoveryResult = result; context.writer?.add(result); context.guard.stop(result.failure); throw error;
  } finally { if (client.preparationHpRecoveryObservation === pending) client.preparationHpRecoveryObservation = null; }
}

const NATIVE_HP_REUSE_MS = 6000, NATIVE_HP_EVIDENCE_MS = 2000;
function exactNativeHp(snapshot) {
  const value = snapshot?.playerHp ?? snapshot?.entities?.find(entity => entity.objectId === snapshot.playerObjectId)?.hp;
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}
function heldNativeHpItem(snapshot) {
  for (const item of [...(snapshot?.beltItems ?? []), ...(snapshot?.inventoryItems ?? [])]) {
    const uniqueId = Number(item.uniqueId), quantity = Number(item.quantity ?? 1);
    if (/^\(hp\)drug/i.test(String(item.name)) && Number.isSafeInteger(uniqueId) && uniqueId >= 0 &&
        Number.isSafeInteger(quantity) && quantity > 0) return { uniqueId, quantity,
      grid: String(item.container).toLowerCase() === 'belt' ? 'belt' : 'inventory' };
  }
  return null;
}
function nativeHpNotice(client, context, reason) {
  const notices = client.nativeHpRecoveryNotices ??= new Set();
  if (notices.has(reason)) return;
  notices.add(reason);
  context.writer?.add({ type: 'nativeHpRecovery', actor: client.label, phase: context.phase, at: Date.now(), status: 'not-sent', reason });
}
function recordNativeHpRecoveryEvidence(client, message, receipt) {
  const pending = client.hpRecoveryEvidence;
  if (!pending || receipt?.direction !== 'received' || receipt.sequence <= pending.cursor ||
      receipt.monotonicMs < pending.sentAtMs || receipt.monotonicMs > pending.deadlineMs) return;
  const owner = self(client), snapshot = client.snapshot;
  if (!owner || owner.objectId !== pending.ownerObjectId || owner.name !== pending.ownerName ||
      snapshot.mapFileName !== pending.mapFileName) { pending.failure = 'owner-changed'; return; }
  if (hasAuthoritativePlayerDeath(snapshot)) { pending.failure = 'owner-dead'; return; }
  if (message.type === 'error') { pending.failure = 'public-error'; return; }
  const proof = { sequence: receipt.sequence, monotonicMs: receipt.monotonicMs };
  if (message.packet === 'UseItem' && Number(message.payload?.uniqueId) === pending.uniqueId &&
      String(message.payload?.grid).toLowerCase() === pending.grid) {
    if (message.payload.success !== true) pending.failure = 'rejected';
    else {
      if (!pending.ack) client.context.sustain?.recordHpRecovery(client, 'ack');
      pending.ack = proof;
    }
  }
  if (message.type === 'worldSnapshot' || message.packet === 'HealthChanged') {
    const hp = exactNativeHp(snapshot);
    if (hp != null && hp > pending.hpBefore) {
      const exactHp = message.type === 'worldSnapshot' ? message.payload?.playerHp : message.payload?.hp;
      if (!pending.sustainHpIncreaseObserved && Number.isFinite(exactHp) && exactHp > pending.hpBefore) {
        pending.sustainHpIncreaseObserved = true; client.context.sustain?.recordHpRecovery(client, 'hp-increase');
      }
      pending.hpIncrease = { ...proof, hp, exactHp: Number.isFinite(exactHp) ? exactHp : null };
    }
  }
  // Only a complete, freshly received inventory can prove removal. Missing
  // arrays and later snapshots outside the original deadline prove nothing.
  if (message.type === 'worldSnapshot' && Array.isArray(snapshot.beltItems) && Array.isArray(snapshot.inventoryItems)) {
    const item = [...snapshot.beltItems, ...snapshot.inventoryItems].find(item => Number(item.uniqueId) === pending.uniqueId);
    const quantity = item ? Number(item.quantity ?? 1) : 0;
    if (Number.isSafeInteger(quantity) && quantity >= 0 && quantity < pending.quantityBefore) {
      const raw = message.payload, complete = Array.isArray(raw?.beltItems) && Array.isArray(raw?.inventoryItems);
      const freshItems = complete ? [...raw.beltItems, ...raw.inventoryItems].filter(item => Number(item?.uniqueId) === pending.uniqueId) : [];
      const freshQuantity = complete && freshItems.length <= 1 ? (freshItems.length ? freshItems[0].quantity : 0) : null;
      const afterQuantity = Number.isSafeInteger(freshQuantity) && freshQuantity >= 0 ? freshQuantity : null;
      const provenDelta = afterQuantity !== null && afterQuantity < pending.quantityBefore ? pending.quantityBefore - afterQuantity : null;
      const previousDelta = pending.sustainProvenConsumptionDelta ?? 0;
      if (provenDelta > previousDelta) {
        pending.sustainProvenConsumptionDelta = provenDelta;
        client.context.sustain?.recordHpRecovery(client, 'consumption', provenDelta - previousDelta, previousDelta === 0);
      }
      pending.consumption = { ...proof, quantity, quantityBefore: pending.quantityBefore, afterQuantity, provenDelta };
    }
  }
}

// This auxiliary operation neither pauses nor completes a native opportunity.
// Its ordinary send still reserves the account's original three-command budget.
export function offerNativeHpRecovery(client, context) {
  if (context.policy?.activityMode !== 'native' || client.pending || client.auxPending || client.hpRecoveryPending || client.hpRecoveryEvidence) return null;
  const owner = activeGameplayOwner(client, context.mapName);
  if (!owner) return null;
  const hp = exactNativeHp(client.snapshot), maximum = client.snapshot.playerMaxHp;
  if (!(hp > 0) || !Number.isFinite(maximum) || !(maximum > 0)) { nativeHpNotice(client, context, 'unknown-exact-health'); return null; }
  if (observedPlayerHp(client.snapshot) / maximum > .6) return null;
  const item = heldNativeHpItem(client.snapshot);
  if (!item) { nativeHpNotice(client, context, 'no-held-hp-item'); return null; }
  const now = performance.now(), lastUse = Number(client.journeyRestorativeUseAt?.hp);
  if (Number.isFinite(lastUse) && now - lastUse < NATIVE_HP_REUSE_MS) return null;
  if (client.budget.remainingWait()) { nativeHpNotice(client, context, 'command-budget'); return null; }
  const sentAtMs = performance.now(), pending = { ...item, quantityBefore: item.quantity, hpBefore: hp,
    ownerObjectId: owner.objectId, ownerName: owner.name, mapFileName: client.snapshot.mapFileName,
    cursor: client.sequence, sentAtMs, deadlineMs: sentAtMs + NATIVE_HP_EVIDENCE_MS };
  client.hpRecoveryEvidence = pending;
  const row = { type: 'nativeHpRecovery', actor: client.label, phase: context.phase, at: Date.now(),
    uniqueId: item.uniqueId, grid: item.grid, quantityBefore: item.quantity, hpBefore: hp, maximumHp: maximum };
  try {
    const dose = startEmergencyHpRecovery(client, { hpThreshold: .6, restorativeReuseDelayMs: NATIVE_HP_REUSE_MS, now })[0];
    if (!dose) { client.hpRecoveryEvidence = null; nativeHpNotice(client, context, 'helper-declined'); return null; }
    if (Number(dose.uniqueId) !== item.uniqueId || dose.command.grid !== item.grid) throw new Error('HP helper item did not match the held-item proof');
    context.writer?.add({ ...row, status: 'sent' });
  } catch {
    client.hpRecoveryEvidence = null;
    context.writer?.add({ ...row, status: 'send-failed' });
    context.guard.stop('Ordinary HP recovery send failed'); return null;
  }
  const operation = client.wait(() => {
    if (pending.failure) { const error = new Error('Ordinary HP recovery failed'); error.hpRecoveryFailure = pending.failure; throw error; }
    return pending.ack && (pending.consumption || pending.hpIncrease) ? pending : null;
  }, 'ordinary HP acknowledgement and effect', NATIVE_HP_EVIDENCE_MS).then(proof => {
    context.writer?.add({ ...row, at: Date.now(), status: 'verified', acknowledgement: proof.ack,
      consumed: Boolean(proof.consumption), consumption: proof.consumption ?? null,
      afterQuantity: proof.consumption?.afterQuantity ?? null, provenConsumedQuantity: proof.consumption?.provenDelta ?? null,
      hpIncreaseObserved: Boolean(proof.hpIncrease), hpIncrease: proof.hpIncrease ?? null });
  }).catch(error => {
    const status = error instanceof SafetyStop ? 'cancelled' : error.hpRecoveryFailure ?? 'timeout';
    context.writer?.add({ ...row, at: Date.now(), status, acknowledgement: pending.ack ?? null,
      consumption: pending.consumption ?? null, afterQuantity: pending.consumption?.afterQuantity ?? null,
      provenConsumedQuantity: pending.consumption?.provenDelta ?? null, hpIncrease: pending.hpIncrease ?? null });
    if (!(error instanceof SafetyStop)) context.guard.stop(`Ordinary HP recovery ${status}`);
  }).finally(() => {
    if (client.hpRecoveryEvidence === pending) client.hpRecoveryEvidence = null;
    if (client.hpRecoveryPending === operation) client.hpRecoveryPending = null;
  });
  client.hpRecoveryPending = operation;
  return operation;
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
  if (client.hpRecoveryPending || client.hpRecoveryEvidence) {
    // Take the opportunity first: maintenance must not hide offered gameplay
    // or missed plans while its ordinary useItem evidence is unresolved.
    context.add({ ...row, status: 'evidence-wait', maintenanceReason: 'hp-recovery' }); return;
  }
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

export function offerNativeClientWork(client, context, now = performance.now()) {
  offerNativeHpRecovery(client, context);
  offerNativeAction(client, context, now);
  if (!client.pending && !client.auxPending && !client.hpRecoveryPending && !client.hpRecoveryEvidence &&
      (Date.now() >= client.nextRtt || Date.now() >= client.nextRefresh)) {
    client.auxPending = auxiliary(client, context).catch(error => context.guard.stop(`Auxiliary transport failed: ${error.message}`))
      .finally(() => { client.auxPending = null; });
  }
}

export function offerPreparationKeepAlive(client, context) {
  if (context.policy.preparationProfile !== 'ordinary-economic-v1' || client.ready || !client.inGame ||
      !client.ordinarySupplies || client.paused || client.draining || client.closed) return;
  checkOwner(client, context);
  if (client.ordinarySupplies.status().status !== 'ready' || client.auxPending ||
      client.hpRecoveryPending || client.hpRecoveryEvidence || client.budget.remainingWait() || Date.now() < client.nextRtt) return;
  client.auxPending = auxiliary(client, context).catch(error => context.guard.stop(`Preparation transport failed: ${error.message}`))
    .finally(() => { client.auxPending = null; });
}

function startEngine(context) {
  context.engine = setInterval(() => {
    try {
      context.guard.assert();
      for (const client of context.clients) {
        if (!client.ready) { offerPreparationKeepAlive(client, context); continue; }
        if (client.paused || client.draining) continue;
        checkOwner(client, context);
        if (context.policy.activityMode === 'native') { offerNativeClientWork(client, context); continue; }
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

export async function ordinaryLogin(client, context, purpose = 'admission') {
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

export async function measure(context, name, seconds, { now = Date.now, sleep: wait = sleep } = {}) {
  context.phase = name; context.current = new Measurements(); context.window = new Measurements(); context.aoi.reset();
  let lastAt;
  const readClock = () => {
    const at = now();
    if (!Number.isSafeInteger(at) || at < 0 || lastAt !== undefined && at < lastAt) {
      const reason = 'Measurement phase clock is invalid or moved backward';
      context.guard.stop(reason); context.guard.assert(); throw new SafetyStop(reason);
    }
    lastAt = at; return at;
  };
  const started = readClock();
  const phase = beginCapacityPhase(context, name, 'measurement', context.clients.filter(client => client.ready).length, started);
  try {
  const windows = [], minimumCompleteWindows = Math.floor(seconds / 60);
  let ended = started, windowStart = started;
  while (ended - started < seconds * 1000 || windows.length < minimumCompleteWindows) {
    context.guard.assert(); await wait(100); ended = readClock(); context.guard.assert();
    if (ended - windowStart >= 60000) {
      const closedAt = ended;
      const assessment = assessMeasurements(context.window, context.clients.filter(client => client.ready).map(client => ({ name: client.label, role: client.role })),
        (closedAt - windowStart) / 1000, context.policy, { requireChat: context.clients.length > 1 });
      windows.push({ elapsedSeconds: (closedAt - started) / 1000, startedAtMs: windowStart, endedAtMs: closedAt, ...assessment });
      context.writer.add({ type: 'window', phase: name, startedAtMs: windowStart, endedAtMs: closedAt, assessment });
      context.window = new Measurements(); windowStart = closedAt;
    }
  }
  const actors = context.clients.filter(client => client.ready).map(client => ({ name: client.label, role: client.role }));
  const measuredSeconds = (ended - started) / 1000;
  const assessment = assessMeasurements(context.current, actors, measuredSeconds, context.policy, {
    requireChat: actors.length > 1 && (context.policy.activityMode !== 'native' || seconds * 1000 > context.policy.nativeChatIntervalMs), aoi: { ...context.aoi.stats } });
  const fanout = Object.entries(assessment.metrics).filter(([kind]) => kind.startsWith('fanout-'));
  const fanoutPassed = fanout.every(([, value]) => (value.status.timeout ?? 0) / Math.max(1, value.count) < .01 && value.fromSend.p95Ms <= 1000);
  // seconds retains the configured profile contract; rates use actual elapsed time.
  const output = { name, controlledActors: actors.length, completed: true, ...assessment, fanoutPassed,
    seconds, measuredSeconds, rollingWindows: windows, startedAtMs: started, endedAtMs: ended,
    passed: assessment.passed && fanoutPassed && windows.every(window => window.passed) };
  finishCapacityPhase(context, phase, true, output.endedAtMs);
  context.writer.add({ type: 'stageComplete', stage: output });
  context.current = null; context.window = null; context.phase = 'ramp';
  return output;
  } finally { finishCapacityPhase(context, phase, false, lastAt); }
}

export async function awaitPendingActions(clients) {
  // Capture once. New actions may continue for unpaused actors, but any old
  // observer obligation must finish before its planned transport is removed.
  const pending = clients.flatMap(client => [client.pending, client.auxPending, client.hpRecoveryPending, ...(client.evidencePending ?? [])]).filter(Boolean);
  await Promise.allSettled(pending);
  // A captured owner action can create its independent fanout waiter just
  // before resolving. Include that waiter before transport teardown as well.
  await Promise.allSettled(clients.flatMap(client => [...(client.evidencePending ?? [])]));
}

async function resumeActor(original, context) {
  original.paused = true; original.combatPursuitPausedAt = Date.now(); context.continuous.pause(original.label);
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
    restoreCombatPursuit(original, restored, context);
    restored.combatArrival = original.combatArrival; restored.combatHunting = original.combatHunting;
    restored.activityStartedAtMs = original.activityStartedAtMs;
    initializePlan(restored, context, performance.now() + 1000); restored.nextRtt = Date.now() + 5000; restored.nextRefresh = Date.now() + 60000;
    context.clients[context.clients.indexOf(original)] = restored; original.ready = false;
    context.sustain?.bindResumedConnection(restored);
    return { actor: original.label, status: 'passed', elapsedMs: performance.now() - started, characterIndex: original.account.characterIndex,
      before, restoredState, resumedOwnerTurnReceipt: liveAck.sequence, generation: restored.nativeTicket.generation, replayRejected: true, loginFallback: false };
  } catch (error) {
    // Keep the resumed socket reachable for final normal logout if validation
    // fails after ownership was already transferred.
    restored.draining = true; replay.draining = true;
    await Promise.allSettled([restored.close(), replay.close()]); throw error;
  }
}

export async function drain(client, context) {
  client.paused = true; context.continuous?.pause(client.label);
  await Promise.allSettled([client.pending, client.auxPending, client.hpRecoveryPending].filter(Boolean));
  client.draining = true;
  let state;
  try { if (client.inGame && !client.closed) { await client.refresh(); state = fingerprint(client); } }
  catch (error) { context.cleanupErrors.push({ actor: client.label, step: 'finalSnapshot', error: sanitize(error.message, client.secrets) }); }
  context.sustain?.drain(client, context.phase);
  try { await client.close(); }
  catch (error) { context.cleanupErrors.push({ actor: client.label, step: 'normalLogout', error: sanitize(error.message, client.secrets) }); }
  client.ready = false; return state ? { account: client.account, state } : null;
}

export function ordinaryPreparationConfiguration(policy, scenario) {
  if ((policy.preparationProfile ?? 'held-only') === 'held-only') return null;
  const preparation = scenario.preparation;
  const points = preparation?.stagingPoints ?? [preparation?.stagingPoint];
  if (!Array.isArray(points) || points.length !== (policy.stages?.at(-1) ?? 1) || points.some(staging =>
      !Number.isSafeInteger(staging?.x) || !Number.isSafeInteger(staging?.y) || staging.x < 0 || staging.y < 0 || staging.x > 65535 || staging.y > 65535 ||
      (staging.mapFileName != null && String(staging.mapFileName) !== '0')) ||
      new Set(points.map(staging => `${staging.x},${staging.y}`)).size !== points.length ||
      !Array.isArray(preparation?.route?.quests) || !preparation.route.quests.length ||
      (scenario.mapFileName != null && String(scenario.mapFileName) !== '0')) {
    throw new Error('ordinary-economic-v1 requires scenario.preparation with distinct explicit map-0 stagingPoints for every actor and an authoritative Deer route');
  }
  const candidates = preparationStagingCatalogue(preparation, points);
  return { stagingPoints: points.map(({ x, y }) => ({ x, y })), route: preparation.route,
    ...(candidates ? { candidatePolicy: preparation.candidatePolicy, stagingCandidates: candidates } : {}) };
}

export async function prepareCapacityActor(client, context, configuration, { createSupplies = createOrdinarySupplies } = {}) {
  const setupStartedAtMs = performance.now();
  if (client.ready || client.paused || client.draining || context.phase !== 'preparation') throw new Error('Ordinary preparation requires an inactive new actor');
  if (String(client.snapshot?.mapFileName) !== '0' || !protocolMapCellIsWalkable(context.map, configuration.stagingPoint) ||
      (client.snapshot.mapTransfers ?? []).some(transfer => inside(configuration.stagingPoint, transfer.bounds))) {
    throw new Error('Ordinary preparation requires a walkable staging point on the current map without a transfer');
  }
  if (String(context.mapName) !== String(client.snapshot.mapFileName)) throw new Error('Ordinary preparation protection requires the current same map');
  const candidates = preparationStagingCatalogue(configuration, [configuration.stagingPoint], context.map, client.snapshot.mapTransfers ?? []);
  const assignments = candidates ? (context.preparationStagingAssignments ??= new Set()) : null;
  const ordinaryProtection = await loadOrdinaryProtection(configuration.route, context.mapName);
  context.guard.assert();
  if (client.ready || client.paused || client.draining || context.phase !== 'preparation' ||
      client.snapshot?.mapFileName !== ordinaryProtection.mapFileName || String(context.mapName) !== ordinaryProtection.mapFileName ||
      configuration.route?.source?.runtimeProfileId !== ordinaryProtection.runtimeProfileId) throw new Error('Ordinary preparation protection map or profile changed while loading');
  const remainingTimeoutMs = configuration.timeoutMs === undefined ? undefined : Math.floor(configuration.timeoutMs - (performance.now() - setupStartedAtMs));
  if (remainingTimeoutMs !== undefined && (!Number.isSafeInteger(configuration.timeoutMs) || !(remainingTimeoutMs > 0))) throw new Error('Ordinary preparation actor600s budget exhausted during protection setup');
  // A scenario cannot inject a replacement safety spec through configuration.
  const navigators = new WeakMap();
  client.ordinarySupplies = createSupplies(client, {
    preparationProfile: 'ordinary-economic-v1', targetHp: context.policy.preparationHpTarget,
    ...configuration, ...(remainingTimeoutMs === undefined ? {} : { timeoutMs: remainingTimeoutMs }), ordinaryProtection, fundingNavigationMap:context.map, isPreparation: () => context.phase === 'preparation' && !client.ready && !client.draining,
    ...(candidates ? { resolveStagingPoint: input => selectPreparationStagingPoint({ ...input, candidates, savedPoints: assignments }) } : {}),
    navigate: (facade, target, range, stop, options) => {
      if (!navigators.has(facade)) navigators.set(facade, createNavigator(facade, {
        loadCollisionMap: async map => {
          if (String(map) !== String(context.mapName)) throw new Error('Ordinary preparation cannot change maps');
          return context.map;
        }, movementResponseTimeoutMs: context.policy.actionTimeoutMs,
      }));
      return navigators.get(facade)(target, range, stop, options);
    },
  });
  let result;
  try {
    result = await client.ordinarySupplies.prepare();
    if (result.status !== 'ready') throw new Error('Ordinary preparation did not become ready');
    if (candidates) {
      const readiness = client.ordinarySupplies.assertReady({ afterSequence: result.after?.sequence - 1 });
      const point = result.resolvedStagingPoint;
      if (!point || result.actualStagingReached !== true || result.after?.ownerName !== client.label ||
          readiness.ownerName !== client.label || readiness.map !== '0' || readiness.inSafeZone !== true ||
          !candidates.some(candidate => candidate.x === point.x && candidate.y === point.y) ||
          point.x !== readiness.x || point.y !== readiness.y || result.after.x !== point.x || result.after.y !== point.y ||
          assignments.has(`${point.x},${point.y}`)) throw new Error('Ordinary resolved staging assignment did not pass fresh readiness');
    }
  } catch (error) {
    try {
      context.writer.add({ type: 'ordinaryPreparation', actor: client.label, at: Date.now(), status: 'failed',
        result: sanitize(client.ordinarySupplies.status(), client.secrets.concat(client.recentSecrets)) });
    } finally { throw error; } // Diagnostics cannot replace the original preparation failure.
  }
  context.writer.add({ type: 'ordinaryPreparation', actor: client.label, at: Date.now(), result });
  if (candidates) assignments.add(`${result.resolvedStagingPoint.x},${result.resolvedStagingPoint.y}`);
  return result;
}

export async function activatePreparedCohort(cohort, context) {
  // Earlier actors may have waited while later actors earned supplies. Recheck
  // every current live snapshot before opening the new cohort's offered load.
  const cursors = new Map();
  for (let offset = 0; offset < cohort.length; offset += 16) {
    await Promise.all(cohort.slice(offset, offset + 16).map(async client => {
      await client.auxPending;
      client.auxPending = (async () => {
        context.guard.assert(); await client.pace(); cursors.set(client, client.sequence); await client.refresh();
      })();
      try { await client.auxPending; } finally { client.auxPending = null; }
    }));
  }
  for (const client of cohort) client.ordinarySupplies.assertReady({ afterSequence: cursors.get(client) });
  const startedAt = performance.now();
  for (const [index, client] of cohort.entries()) {
    client.ready = true;
    initializePlan(client, context, startedAt + 500 + (index % 10) * 40);
    client.nextRtt = Date.now() + 5000; client.nextRefresh = Date.now() + 60000;
    client.activityStartedAtMs = startedAt; context.continuous.join(client.label, client.role, startedAt);
    context.sustain?.activate(client, context.phase, startedAt);
  }
  context.writer.add({ type: 'cohortActivated', profile: 'ordinary-economic-v1', at: Date.now(),
    newActors: cohort.length, controlledActors: context.clients.length, activityStartedAtMs: startedAt,
    existingActorsContinue: true });
}

export async function runCapacity(options) {
  const declaredTarget = options.targetActors;
  if (declaredTarget !== undefined && capacityActorLimit({ targetActors: declaredTarget }) === 500) {
    throw new Error('The 500 online / 500 continuously fighting contract is not implemented; targetActors=500 execution is blocked, including the historical 50-fighter workload.');
  }
  if (options.realm !== 'isolated') throw new Error('--realm isolated is required');
  const endpoint = validateEndpoint(options.endpoint), policy = capacityPolicy(Object.create(options,
    { targetActors: { value: declaredTarget } })), attackResults = attackResultsMode(options.attackResults);
  if (!options.accountsFile || !options.monitor || !options.output) throw new Error('--accounts-file, --monitor and --output are required');
  const output = outsideRepository(options.output); await fs.mkdir(output, { recursive: true });
  const runId = `${Date.now()}-${crypto.randomBytes(4).toString('hex')}`, writer = new BoundedEvidence(path.join(output, `${runId}.samples.jsonl`));
  const guard = new CapacityMonitor(options.monitor, writer), aggregate = new Measurements();
  const context = { endpoint, policy, attackResults, runId, writer, guard, aggregate, clients: [], allClients: [], targetClaims: new Map(),
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
  const report = { schemaVersion: 1, runId, endpoint, policy, startedAt: new Date().toISOString(), stages: [], nativeResume: [], savedVerification: [], phaseWindows: [],
    ordinaryAccountsOnly: true, visualAccepted: false, capacityUpperBound: null, acceptanceScope: 'One real egress, ordinary same-map movement/AOI and the explicitly declared combat subgroup' };
  report.preparation = { profile: policy.preparationProfile ?? 'held-only', targetHpStock: policy.preparationHpTarget ?? 0,
    activationRule: policy.preparationProfile === 'ordinary-economic-v1' ? 'New cohort remains inactive until every actor completes bounded ordinary preparation and passes a fresh staging/health/stock recheck; existing active actors continue unchanged.' :
      'Original canonical per-actor activation after login and equipment; already-held supplies only.',
    stageResupply: false, sustainableDurationProven: false };
  Object.defineProperty(report, 'measurementClock', { enumerable: true,
    value: Object.freeze({ scope: 'All measurement phases; seconds retains configured duration and measuredSeconds is actual elapsed wall time.',
      minimumCompleteWindowMs: 60000,
      completeWindowCountRule: 'At least floor(configured seconds / 60) complete windows, each starting at the previous actual close, plus the full configured phase duration.',
      durationRule: 'Activity and rate assessment use actual measuredSeconds; no short window is relabelled as sixty seconds or duplicated after a stall.',
      partialWindowRule: 'The final partial minute remains in whole-phase and continuous assessment, without counting as a complete rolling window.' }) });
  if (policy.activityMode === 'native') Object.defineProperty(report, 'navigationCoordination', { enumerable: true,
    value: Object.freeze({ scope: 'Native tool-local conservative union of original AOI obstacles, active controlled owner receipts and one in-flight movement hint per connection; never authoritative occupancy, snapshots or AOI.',
      maximumHintCellsPerConnection: NAVIGATION_HINT_MAX_CELLS, blockedOpportunityRule: 'A conflicting prepared move is still an offered navigation-blocked/native opportunity; original cadence, completion and correction gates remain.' }) });
  if (policy.activityMode === 'native') Object.defineProperty(report, 'nativePatrolPlanning', { enumerable: true,
    value: Object.freeze({ scope: 'Native primed patrol fallback only; original fast path and at most four existing corner targets remain.',
      maximumFallbackCornerTargets: 4, pathExpansionLimit: 4000,
      selectionRule: 'Prefer the first existing collision-aware path with two legal collinear Run steps; otherwise retain the original first Walk.',
      primingRule: 'Only while native Run priming is active; original cadence, transfer/coverage checks and offered-opportunity accounting remain.' }) });
  if (policy.activityMode === 'native') Object.defineProperty(report, 'nativeHpMaintenance', { enumerable: true,
    value: Object.freeze({ scope: 'Stage maintenance uses already-held ordinary HP potions only; no MP, purchase, grants or revival.',
      hpThreshold: .6, reuseDelayMs: NATIVE_HP_REUSE_MS, evidenceDeadlineMs: NATIVE_HP_EVIDENCE_MS,
      commandBudgetPerSecond: 3, rule: 'Fresh matching UseItem success plus complete-inventory consumption or exact HP increase within the same deadline; consumption is not proof of healing.',
      activityRule: 'HP recovery is offered before gameplay and auxiliary work. Pending HP evidence records original offered native opportunities as evidence-wait with maintenanceReason; it never credits movement, attack or completion.' }) });
  if (policy.activityMode === 'native') Object.defineProperty(report, 'nativeChasePreference', { enumerable: true,
    value: Object.freeze({ freshReceiptMs: RECENT_CHASE_PREFERENCE_MS,
      scope: 'Client-side preference within the original first4 candidates, not an exclusive claim or a capacity result.',
      evidenceRule: 'Only fresh matching nonzero chase UserLocation receipts establish monotonic first/last ACK priority; active living same-map fighters and live targets only.',
      selectionRule: 'Keep adjacent witnessed attacks first, then prefer targets without an earlier recent pursuer; retain every original first4 candidate and path fallback.',
      budgetRule: 'Original4-target/8-goal/4000-node searches, cadence, offered/completed accounting and finite chase budgets are unchanged.',
      restoreRule: 'Resume clears receipt preference only; original chase elapsed time, ACK count and visited positions remain.' }) });
  if (policy.activityMode === 'native') Object.defineProperty(report, 'nativeAttackWitness', { enumerable: true,
    value: Object.freeze({ scope: 'New native attack witness selection; the original full AOI membership range remains16.',
      maximumOwnerAndTargetDistance: stableFanoutRange('attack', policy),
      rule: 'Choose the nearest active living same-map observer that actually sees both attacker and target; reselect immediately before send. No witness leaves the native opportunity incomplete.',
      approachRule: 'Native approach paths require a current active living same-map observer whose actual snapshot contains both attacker and target and whose actual position is within6 of the target and prospective attack tile; otherwise continue other eligible targets or ordinary scouting.',
      damageRule: 'Original dual-end damage/health evidence,900ms deadline and ambiguous-target quarantine remain unchanged.' }) });
  if (attackResults === 'v2') Object.defineProperty(report, 'attackResults', { enumerable: true,
    value: Object.freeze({ mode: 'v2', capability: ATTACK_OUTCOME_CAPABILITY,
      rule: 'No legacy fallback. Activity attacks count correlated launched outcomes only; hits still require the original dual-end damage/health evidence. Hunting attempts are offered requests, not launched attacks.' }) });
  context.phaseWindows = report.phaseWindows; context.density = new PublicDensity(policy);
  context.sustain = new SustainObservations(policy, row => writer.add(row));
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
    context.pool = validateCapacityPool(JSON.parse(await fs.readFile(context.accountFile, 'utf8')), endpoint, policy);
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
    const preparationConfiguration = ordinaryPreparationConfiguration(policy, scenario);
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
        context.sustain.tick(context.phase);
        writer.add({ type: 'coverage', phase: context.phase, at: Date.now(), aoi, driver, density: context.density.sample(context.clients) }); lag.reset();
        report.peakControlledActors = Math.max(report.peakControlledActors ?? 0, aoi.activeOwners);
      } catch (error) { if (!(error instanceof SafetyStop)) guard.stop(error.message); }
    }, 1000);
    startEngine(context);
    const combatIndices = new Set(Array.from({ length: policy.combatActors }, (_, i) => Math.floor((i + .5) * policy.stages.at(-1) / policy.combatActors)));
    for (const count of policy.stages) {
      context.phase = preparationConfiguration ? 'preparation' : 'ramp';
      const bootstrap = beginCapacityPhase(context, `bootstrap-${count}`, 'bootstrap', count);
      const preparation = preparationConfiguration ? beginCapacityPhase(context, `preparation-${count}`, 'bootstrap', count) : null;
      const newCohort = [];
      try {
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
            const proof = validateNativeExplicitHomes(context.map, self(client), policy.stages.at(-1), scenario.anchors, context.patrolSpan, client.snapshot.mapTransfers ?? [], stationaryObstacles, policy);
            context.homes = proof.homes;
            report.scenario.explicitNativeHomes = { visitedCells: proof.visitedCells, mapCells: proof.mapCells, footprintCells: proof.footprintCells };
          } else context.homes = chooseHomes(context.map, self(client), policy.stages.at(-1), policy.layout, client.snapshot.mapTransfers ?? [], scenario.anchors ?? [], policy.activityMode === 'native' ? 3 : 1, stationaryObstacles);
          context.combatClusters = validateNativeCombatClusters(scenario.combatClusters, context.homes, combatIndices, context.patrolSpan);
          for (const cluster of context.combatClusters) {
            cluster.observerNames = new Set(cluster.movementActorIndices.map(index => context.pool.accounts[index].name));
            for (const binding of cluster.observerBindings) binding.name = context.pool.accounts[binding.actorIndex].name;
          }
          if (context.combatClusters.length) report.scenario.combatClusters = context.combatClusters.map(cluster => ({ combatActorIndices: cluster.combatActorIndices,
            movementActorIndices: cluster.movementActorIndices, observerFootprintCoverageRadius: 14 }));
          report.scenario.mapFileName = context.mapName; report.scenario.anchors = context.homes;
          if (policy.activityMode === 'native') report.scenario.knownStationaryNpcCount = new Set(stationaryObstacles.map(point => `${point.x},${point.y}`)).size;
        }
        checkOwner(client, context);
        await equipStarterGear(client);
        client.role = combatIndices.has(index) ? 'combat' : 'movement'; client.home = context.homes[index]; client.ready = !preparationConfiguration;
        client.combatCluster = context.combatClusters.find(cluster => cluster.combatActorIndices.includes(index));
        context.clients.push(client);
        if (preparationConfiguration) {
          newCohort.push(client);
          client.nextRtt = Date.now() + 10000; client.nextRefresh = Infinity;
          await prepareCapacityActor(client, context, { route: preparationConfiguration.route,
            stagingPoint: preparationConfiguration.stagingPoints[index],
            ...(preparationConfiguration.stagingCandidates ? { candidatePolicy: preparationConfiguration.candidatePolicy,
              stagingCandidates: preparationConfiguration.stagingCandidates } : {}) });
        } else {
          initializePlan(client, context, performance.now() + 500 + (index % 10) * 40);
          client.nextRtt = Date.now() + 5000; client.nextRefresh = Date.now() + 60000;
          client.activityStartedAtMs = performance.now(); context.continuous.join(client.label, client.role, client.activityStartedAtMs);
          context.sustain.activate(client, context.phase, client.activityStartedAtMs);
        }
        writer.add({ type: 'admitted', actor: client.label, role: client.role, characterIndex: client.account.characterIndex, at: Date.now(), controlledActors: context.clients.length });
        options.onProgress?.({ stage: 'admitted', controlledActors: context.clients.length, target: count });
        await guard.afterAdmission(Date.now());
      }
      if (preparationConfiguration) {
        await activatePreparedCohort(newCohort, context);
        finishCapacityPhase(context, preparation, true);
      }
      await settleClients(context);
      finishCapacityPhase(context, bootstrap, true);
      } finally { if (preparation) finishCapacityPhase(context, preparation, false); finishCapacityPhase(context, bootstrap, false); }
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
    context.sustain.stop(stoppedAt);
    for (const client of context.clients) { client.paused = true; context.continuous.pause(client.label, stoppedAt); }
    await awaitPendingActions(context.clients);
    for (const client of context.allClients) client.ordinarySupplies?.dispose();
    const saved = [];
    // Bounded drain concurrency avoids turning safety shutdown into a save storm.
    const live = context.allClients.filter(client => client.ws && !client.closed);
    for (let offset = 0; offset < live.length; offset += 4) {
      const results = await Promise.all(live.slice(offset, offset + 4).map(client => drain(client, context)));
      saved.push(...results.filter(Boolean));
    }
    for (const client of context.clients) context.sustain.drain(client, context.phase);
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
    report.density = context.density.summary();
    report.peakServerActive = guard.peakServerActive; report.bytes = context.allClients.reduce((sum, client) => ({ sent: sum.sent + client.bytes.sent, received: sum.received + client.bytes.received }), { sent: 0, received: 0 });
    report.bytesDefinition = 'Physical WebSocket payload bytes, including catalog batch/gzip headers and excluding WebSocket/TLS framing';
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
      continuousRunningRule: policy.preparationProfile === 'ordinary-economic-v1' ?
        'The Run ratio and cell-rate gates begin at new-cohort activation and include anchor travel; existing active actors retain those gates throughout later-cohort preparation and login-window waits' :
        'The Run ratio and cell-rate gates also apply throughout ramp, anchor travel and ordinary login-window waits',
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
    'activityMode', 'movementIntervalMs', 'generateScenario', 'scenarioMap', 'scenarioOrigin', 'scenarioCenter', 'scenarioCount', 'attackResults',
    'targetActors', 'preparationProfile', 'preparationHpTarget']);
  args = args.flatMap(value => value.startsWith('--attack-results=') ? ['--attack-results', value.slice('--attack-results='.length)] : [value]);
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
