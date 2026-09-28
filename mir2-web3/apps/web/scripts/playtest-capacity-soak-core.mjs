// Bounded measurement primitives. No network, account creation, or game mutation.
import fs from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { finished } from 'node:stream/promises';
import { inspectMonitor, SafetyStop } from './playtest-load-smoke.mjs';
import { validateEndpoint, inventoryFingerprint } from './playtest-multiplayer-smoke.mjs';

export const LOGIN_WINDOW_MS = 15 * 60 * 1000 + 1000;
export const self = client => client.snapshot?.entities?.find(entity => entity.objectId === client.snapshot.playerObjectId);
export const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
export const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const PRIVATE = /password|secret|token|credential|authorization|cookie|assertion|private.?key/i;

export function sanitize(value, secrets = []) {
  if (Array.isArray(value)) return value.map(item => sanitize(item, secrets));
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) =>
    [key, PRIVATE.test(key) ? '[redacted]' : sanitize(item, secrets)]));
  if (typeof value === 'string') for (const secret of secrets) if (secret) value = value.split(secret).join('[redacted]');
  return value;
}

export function capacityPolicy(options = {}) {
  const profile = options.profile ?? 'standard';
  if (!['standard', 'probe'].includes(profile)) throw new Error('Profile must be standard or probe');
  const stages = String(options.stages ?? (profile === 'probe' ? '1,2' : '10,20,30,40,50')).split(',').map(Number);
  if (!stages.length || stages.length > 15 || stages.some((n, i) => !Number.isInteger(n) || n < 1 ||
      n > (profile === 'probe' ? 5 : 100) || (i && n <= stages[i - 1]))) throw new Error('Stages must increase, with an absolute maximum of 100 actors');
  const stageSeconds = Number(options.stageSeconds ?? (profile === 'probe' ? 30 : 300));
  const soakSeconds = Number(options.soakSeconds ?? (profile === 'probe' ? 30 : 3600));
  const wallSeconds = Number(options.wallSeconds ?? (profile === 'probe' ? 1800 : 10800));
  const combatActors = Number(options.combatActors ?? 0), resumeSamples = Number(options.resumeSamples ?? (profile === 'probe' ? 0 : 2));
  const saveSamples = Number(options.saveSamples ?? (profile === 'probe' ? 0 : 2));
  const layout = options.layout ?? 'dispersed';
  if (!['dispersed', 'hotspot'].includes(layout) || !Number.isInteger(stageSeconds) || stageSeconds < (profile === 'probe' ? 20 : 120) || stageSeconds > 600 ||
      !Number.isInteger(soakSeconds) || soakSeconds < (profile === 'probe' ? 20 : 3600) || soakSeconds > 14400 ||
      !Number.isInteger(wallSeconds) || wallSeconds < 60 || wallSeconds > 43200 ||
      !Number.isInteger(combatActors) || combatActors < 0 || combatActors > Math.min(25, stages.at(-1)) ||
      !Number.isInteger(resumeSamples) || resumeSamples < 0 || resumeSamples > 5 ||
      !Number.isInteger(saveSamples) || saveSamples < 0 || saveSamples > 5) throw new Error('Invalid finite scenario or duration bounds');
  if (profile === 'standard' && (resumeSamples < 1 || saveSamples < 1)) throw new Error('Standard acceptance requires native resume and save samples');
  if (resumeSamples > stages.at(-1) - combatActors || saveSamples > stages.at(-1)) throw new Error('Declared resume/save samples exceed the eligible actor cohort');
  return Object.freeze({ profile, stages, stageSeconds, soakSeconds, wallSeconds, layout, combatActors, resumeSamples, saveSamples,
    commandsPerSecond: 3, intentionIntervalMs: 1000, maximumSendLatenessMs: 250,
    movementP95Ms: 750, chatP95Ms: 1000, actionTimeoutMs: 2000, timeoutExclusive: 0.01,
    maximumMissedPlanRatio: 0.05, maximumCorrectionRatio: 0.05, minimumMovesPerMinute: 30,
    minimumCombatHitsPerMinute: 3, minimumCombatAttacksPerMinute: 12, aoiRange: 16, aoiGraceMs: 1000,
    maximumMemorySlopeMiBPerHour: 32, memoryWarmupSeconds: 300,
    maximumDriverLagP99Ms: 250, fullAcceptanceEligible: profile === 'standard' });
}

export function validateCapacityPool(pool, endpoint) {
  if (pool?.schemaVersion !== 1 || pool.purpose !== 'isolated-load-actors' || pool.realm !== 'isolated' ||
      validateEndpoint(pool.endpoint) !== endpoint || !Array.isArray(pool.accounts) || pool.accounts.length < 1 || pool.accounts.length > 100) throw new Error('Invalid isolated capacity account pool');
  const ids = new Set(), names = new Set();
  for (const account of pool.accounts) {
    if (!/^lt[a-z0-9]{8,16}$/.test(account.accountId ?? '') || !/^L[a-z0-9]{8}$/.test(account.name ?? '') ||
        typeof account.password !== 'string' || account.password.length < 12 || account.password.length > 20 ||
        !['registered', 'ready'].includes(account.state) || !account.provisioning ||
        (account.state === 'ready' && !Number.isInteger(account.characterIndex))) throw new Error('Only pre-provisioned dedicated lt actors are allowed, with explicit provisioning source');
    ids.add(account.accountId); names.add(account.name);
  }
  if (ids.size !== pool.accounts.length || names.size !== pool.accounts.length) throw new Error('Duplicate capacity actor');
  return pool;
}

// Persist this ledger before sending Login. A failed request still consumes its
// real peer bucket; no retry, UA rotation, or successful-login reset is assumed.
export class LoginBudget {
  constructor(attempts = [], notBefore = 0, clock = Date.now) {
    if (!Array.isArray(attempts) || attempts.some(item => !Number.isFinite(item.at) || !['admission', 'verification'].includes(item.kind))) throw new Error('Invalid persistent auth budget');
    this.attempts = attempts; this.notBefore = notBefore; this.clock = clock;
  }
  prune() { this.attempts = this.attempts.filter(item => this.clock() - item.at < LOGIN_WINDOW_MS); }
  delay(kind = 'admission') {
    this.prune(); const now = this.clock();
    const total = this.attempts, group = total.filter(item => item.kind === kind), limit = kind === 'admission' ? 25 : 5;
    const waits = [Math.max(0, this.notBefore - now)];
    if (total.length >= 30) waits.push(total[0].at + LOGIN_WINDOW_MS - now);
    if (group.length >= limit) waits.push(group[0].at + LOGIN_WINDOW_MS - now);
    return Math.max(...waits, 0);
  }
  reserve(kind = 'admission') {
    if (!['admission', 'verification'].includes(kind) || this.delay(kind)) throw new Error('Ordinary login window is not available');
    this.attempts.push({ at: this.clock(), kind });
  }
}

// Upper-bound quantiles: 1 ms buckets through 2s, 10 ms through 10s,
// 100 ms through 60s. Fixed memory independent of duration and sample count.
export class Histogram {
  constructor() { this.buckets = new Uint32Array(3302); this.count = 0; this.maximum = 0; }
  add(value) {
    if (!Number.isFinite(value) || value < 0) return;
    const index = value <= 2000 ? Math.ceil(value) : value <= 10000 ? 2000 + Math.ceil((value - 2000) / 10) : Math.min(3301, 2800 + Math.ceil((value - 10000) / 100));
    this.buckets[index]++; this.count++; this.maximum = Math.max(this.maximum, value);
  }
  quantile(p) {
    if (!this.count) return null;
    let cumulative = 0;
    for (let i = 0; i < this.buckets.length; i++) {
      cumulative += this.buckets[i];
      if (cumulative >= Math.ceil(this.count * p)) return i <= 2000 ? i : i <= 2800 ? 2000 + (i - 2000) * 10 : i <= 3300 ? 10000 + (i - 2800) * 100 : this.maximum;
    }
    return this.maximum;
  }
  summary() { return { count: this.count, p50Ms: this.quantile(.5), p95Ms: this.quantile(.95), p99Ms: this.quantile(.99), maxMs: this.maximum }; }
}

export class Measurements {
  constructor() { this.metrics = new Map(); this.actors = new Map(); this.samples = 0; }
  add(sample) {
    this.samples++;
    if (!this.metrics.has(sample.kind)) this.metrics.set(sample.kind, { count: 0, status: {}, sent: new Histogram(), planned: new Histogram() });
    const metric = this.metrics.get(sample.kind); metric.count++; metric.status[sample.status] = (metric.status[sample.status] ?? 0) + 1;
    if (['success', 'corrected', 'positive-unobserved', 'miss'].includes(sample.status)) { metric.sent.add(sample.latencyMs); metric.planned.add(sample.plannedLatencyMs); }
    if (!this.actors.has(sample.actor)) this.actors.set(sample.actor, { plans: 0, gaps: 0, moves: 0, attacks: 0, hits: 0, damage: 0, lastSuccessAt: 0 });
    const actor = this.actors.get(sample.actor);
    if (sample.plannedGameplay) actor.plans++;
    if (['plan-gap', 'busy', 'navigation-blocked', 'budget-gap', 'action-locked'].includes(sample.status)) actor.gaps++;
    if (['walk', 'run'].includes(sample.kind) && sample.status === 'success') actor.moves++;
    if (sample.kind === 'attack' && !['plan-gap', 'busy', 'budget-gap', 'navigation-blocked', 'action-locked'].includes(sample.status)) actor.attacks++;
    if (sample.kind === 'attack' && sample.status === 'success' && sample.damage > 0 && sample.sharedHealthVerified) { actor.hits++; actor.damage += sample.damage; }
    if (sample.status === 'success' && (['walk', 'run'].includes(sample.kind) || (sample.kind === 'attack' && sample.damage > 0))) actor.lastSuccessAt = sample.at;
  }
  summary() {
    return Object.fromEntries([...this.metrics].map(([kind, value]) => [kind, { count: value.count, status: value.status,
      fromSend: value.sent.summary(), fromPlan: value.planned.summary() }]));
  }
}

export function assessMeasurements(measurements, actors, seconds, policy, { requireChat = true, aoi } = {}) {
  const metrics = measurements.summary(), ratio = n => n / Math.max(.01, seconds / 60);
  const activity = actors.map(actor => {
    const sample = measurements.actors.get(actor.name) ?? { plans: 0, gaps: 0, moves: 0, attacks: 0, hits: 0, damage: 0 };
    const combat = actor.role === 'combat';
    return { actor: actor.name, role: actor.role, ...sample, passed: sample.plans >= Math.floor(seconds * .8) &&
      sample.gaps / Math.max(1, sample.plans) <= policy.maximumMissedPlanRatio &&
      (combat ? ratio(sample.hits) >= policy.minimumCombatHitsPerMinute && ratio(sample.attacks) >= policy.minimumCombatAttacksPerMinute : ratio(sample.moves) >= policy.minimumMovesPerMinute) };
  });
  const gameKinds = ['walk', 'run', 'turn', 'chat', 'attack'];
  const counts = gameKinds.reduce((totals, kind) => {
    for (const [status, count] of Object.entries(metrics[kind]?.status ?? {})) totals[status] = (totals[status] ?? 0) + count;
    return totals;
  }, {});
  const attempts = Object.values(counts).reduce((a, b) => a + b, 0);
  const timeoutRatio = (counts.timeout ?? 0) / Math.max(1, attempts);
  const moveCount = (metrics.walk?.count ?? 0) + (metrics.run?.count ?? 0);
  const corrections = (metrics.walk?.status.corrected ?? 0) + (metrics.run?.status.corrected ?? 0);
  const moveMetrics = ['walk', 'run'].map(kind => metrics[kind]).filter(Boolean);
  const movementPassed = moveCount > 0 && moveMetrics.every(metric => metric.fromSend.p95Ms != null && metric.fromSend.p95Ms <= policy.movementP95Ms && metric.fromPlan.p95Ms <= policy.movementP95Ms + policy.maximumSendLatenessMs);
  const chatPassed = !requireChat || (metrics.chat?.fromSend.p95Ms != null && metrics.chat.fromSend.p95Ms <= policy.chatP95Ms && (metrics.peerChat?.status.success ?? 0) > 0);
  const combatExpected = actors.some(actor => actor.role === 'combat');
  const combatCovered = combatExpected && activity.filter(actor => actor.role === 'combat').every(actor => actor.passed);
  return { passed: activity.length > 0 && activity.every(actor => actor.passed) && movementPassed && chatPassed &&
    timeoutRatio < policy.timeoutExclusive && corrections / Math.max(1, moveCount) <= policy.maximumCorrectionRatio &&
    !['error', 'rejected', 'transportError'].some(status => counts[status]) && (aoi == null || (aoi.samples > 0 && aoi.persistentMissing === 0 && aoi.ghosts === 0 && aoi.duplicates === 0)),
  seconds, activity, timeoutRatio, correctionRatio: corrections / Math.max(1, moveCount), movementPassed, chatPassed,
  combatCoverage: combatCovered ? 'shared-positive-damage' : combatExpected ? 'scenario-insufficient' : 'not-requested', metrics, aoi };
}

// This gate has its own clock and buffers: switching measured stages, waiting
// for another login window, or pausing ONE native-resume actor cannot reset it.
// Histograms and the last summary per actor are bounded; every window is also
// emitted to the evidence stream, including failures before formal stages.
export class ContinuousAcceptance {
  constructor(policy, { emit = () => {}, onFailure = () => {} } = {}) {
    this.policy = policy; this.emit = emit; this.onFailure = onFailure; this.actors = new Map();
    this.failed = false; this.failureCount = 0; this.firstFailure = null; this.windows = 0; this.shortSegments = 0;
  }
  fail(reason, evidence) {
    this.failureCount++;
    const failure = { reason, evidence };
    this.emit({ type: 'continuousFailure', ...failure });
    if (!this.failed) { this.failed = true; this.firstFailure = failure; this.onFailure(reason); }
  }
  join(name, role, now = performance.now()) {
    if (!this.actors.has(name)) this.actors.set(name, { name, role, active: true, lastAt: now, activeMs: 0,
      totalActiveMs: 0, pausedMs: 0, measurements: new Measurements(), phases: new Set(), windows: 0 });
  }
  advance(actor, now) {
    if (now < actor.lastAt) { this.fail('Continuous activity clock moved backward', { actor: actor.name }); return; }
    const elapsed = now - actor.lastAt;
    if (actor.active) { actor.activeMs += elapsed; actor.totalActiveMs += elapsed; }
    else actor.pausedMs += elapsed;
    actor.lastAt = now;
    if (actor.activeMs >= 60000) this.finishWindow(actor, false);
  }
  sync(states, now = performance.now()) {
    const known = new Map(states.map(state => [state.name, state]));
    for (const actor of this.actors.values()) {
      this.advance(actor, now);
      actor.active = Boolean(known.get(actor.name)?.active);
    }
  }
  pause(name, now = performance.now()) {
    const actor = this.actors.get(name);
    if (actor) { this.advance(actor, now); actor.active = false; }
  }
  add(sample, now = performance.now()) {
    const actor = this.actors.get(sample.actor);
    if (!actor || !['walk', 'run', 'turn', 'chat', 'attack', 'peerChat', 'selfChat'].includes(sample.kind) && !sample.kind.startsWith('fanout-')) return;
    this.advance(actor, now); actor.measurements.add(sample); actor.phases.add(sample.phase ?? 'unknown');
    if (['rejected', 'transportError', 'error'].includes(sample.status)) this.fail('A participating actor received a real gameplay failure',
      { actor: sample.actor, phase: sample.phase, kind: sample.kind, status: sample.status });
  }
  observeAoi(observation, phase) {
    if (observation.issues.length) this.fail('Persistent AOI loss, ghost or duplicate during active load', { phase, ...observation });
  }
  finishWindow(actor, final) {
    const seconds = actor.activeMs / 1000, metrics = actor.measurements.summary();
    if (!seconds && !actor.measurements.samples) return;
    const activity = actor.measurements.actors.get(actor.name) ?? { plans: 0, gaps: 0, moves: 0, attacks: 0, hits: 0 };
    const statuses = Object.entries(metrics).filter(([kind]) => ['walk', 'run', 'turn', 'chat', 'attack'].includes(kind))
      .flatMap(([, metric]) => Object.entries(metric.status));
    const total = statuses.reduce((sum, [, count]) => sum + count, 0);
    const timeouts = statuses.filter(([status]) => status === 'timeout').reduce((sum, [, count]) => sum + count, 0);
    const movement = ['walk', 'run'].map(kind => metrics[kind]).filter(Boolean);
    const movementCount = movement.reduce((sum, metric) => sum + metric.count, 0);
    const corrections = movement.reduce((sum, metric) => sum + (metric.status.corrected ?? 0), 0);
    const enoughDuration = seconds >= 20;
    const rate = count => count * 60 / Math.max(seconds, .001);
    const progressed = rate(activity.moves) >= this.policy.minimumMovesPerMinute ||
      (actor.role === 'combat' && rate(activity.hits) >= this.policy.minimumCombatHitsPerMinute && rate(activity.attacks) >= this.policy.minimumCombatAttacksPerMinute);
    const activityPassed = !enoughDuration || (activity.plans >= Math.floor(seconds * .8) && progressed && activity.gaps / Math.max(1, activity.plans) <= this.policy.maximumMissedPlanRatio);
    const latencyPassed = movement.every(metric => !metric.fromSend.count || (metric.fromSend.p95Ms <= this.policy.movementP95Ms &&
      metric.fromPlan.p95Ms != null && metric.fromPlan.p95Ms <= this.policy.movementP95Ms + this.policy.maximumSendLatenessMs)) &&
      (!metrics.chat?.fromSend.count || metrics.chat.fromSend.p95Ms <= this.policy.chatP95Ms);
    const fanoutPassed = Object.entries(metrics).filter(([kind]) => kind.startsWith('fanout-')).every(([, metric]) =>
      (metric.status.timeout ?? 0) / Math.max(1, metric.count) < this.policy.timeoutExclusive && (!metric.fromSend.count || metric.fromSend.p95Ms <= this.policy.chatP95Ms));
    const verdict = { actor: actor.name, role: actor.role, phases: [...actor.phases], seconds, final,
      activityDurationSufficient: enoughDuration, activity, activityPassed, latencyPassed, fanoutPassed,
      timeoutRatio: timeouts / Math.max(1, total), correctionRatio: corrections / Math.max(1, movementCount), metrics };
    verdict.passed = activityPassed && latencyPassed && fanoutPassed && verdict.timeoutRatio < this.policy.timeoutExclusive &&
      verdict.correctionRatio <= this.policy.maximumCorrectionRatio;
    if (!enoughDuration) this.shortSegments++;
    this.windows++; actor.windows++; actor.lastWindow = verdict; this.emit({ type: 'continuousWindow', ...verdict });
    if (!verdict.passed) this.fail('Continuous activity window failed, regardless of the measured stage', verdict);
    actor.activeMs = 0; actor.measurements = new Measurements(); actor.phases.clear();
  }
  finish(now = performance.now()) {
    for (const actor of this.actors.values()) { this.advance(actor, now); actor.active = false; this.finishWindow(actor, true); }
  }
  summary() {
    return { passed: !this.failed, failureCount: this.failureCount, firstFailure: this.firstFailure,
      assessedWindows: this.windows, shortSegments: this.shortSegments,
      shortSegmentRule: 'Under 20 active seconds: protocol/latency/error gates still apply; no activity-rate claim',
      actors: [...this.actors.values()].map(actor => ({ name: actor.name, role: actor.role, activeSeconds: actor.totalActiveMs / 1000,
        plannedPauseSeconds: actor.pausedMs / 1000, assessedWindows: actor.windows })) };
  }
}

export function fingerprint(client) {
  const player = self(client);
  return { name: player?.name, mapFileName: String(client.snapshot?.mapFileName ?? ''), x: player?.x, y: player?.y,
    direction: player?.direction, inventory: inventoryFingerprint(client.snapshot) };
}

// Denominator comes from authoritative owners, never from already-received AOI.
export class AoiCoverage {
  constructor() { this.missingSince = new Map(); this.extraSince = new Map(); this.reset(); }
  reset() { this.stats = { samples: 0, expectedPairs: 0, observedPairs: 0, persistentMissing: 0, ghosts: 0, duplicates: 0, minimumCoverage: 1, maximumExpectedPerObserver: 0 }; }
  sample(clients, now = performance.now()) {
    const live = clients.filter(client => client.inGame && !client.closed && !client.paused && self(client));
    const liveNames = new Set(live.map(client => client.label)), knownNames = new Set(clients.map(client => client.label));
    for (const pending of [this.missingSince, this.extraSince]) for (const key of pending.keys()) {
      if (key.split(':').some(name => !liveNames.has(name))) pending.delete(key);
    }
    const issues = []; let expected = 0, observed = 0;
    for (const receiver of live) {
      // A planned transport pause permits temporary absence, never two copies
      // of that identity in the still-active observer's world.
      const counts = new Map();
      for (const entity of receiver.snapshot.entities) if (knownNames.has(entity.name) && ['player', 'remoteplayer', 'selfplayer'].includes(String(entity.kind).toLowerCase())) {
        counts.set(entity.name, (counts.get(entity.name) ?? 0) + 1);
      }
      for (const [name, count] of counts) if (count > 1) { this.stats.duplicates++; issues.push({ observer: receiver.label, owner: name, reason: 'duplicate' }); }
      let expectedHere = 0;
      for (const owner of live) {
        if (receiver === owner) continue;
        const key = `${receiver.label}:${owner.label}`;
        const shouldSee = receiver.snapshot.mapFileName === owner.snapshot.mapFileName && distance(self(receiver), self(owner)) <= 16;
        const entries = receiver.snapshot.entities.filter(entity => entity.name === owner.label && ['player', 'remoteplayer', 'selfplayer'].includes(String(entity.kind).toLowerCase()));
        if (shouldSee) {
          expected++; expectedHere++;
          if (entries.length === 1) { observed++; this.missingSince.delete(key); }
          else {
            if (!this.missingSince.has(key)) this.missingSince.set(key, now);
            if (now - this.missingSince.get(key) > 1000) { this.stats.persistentMissing++; issues.push({ observer: receiver.label, owner: owner.label, reason: 'missing' }); }
          }
          this.extraSince.delete(key);
        } else {
          this.missingSince.delete(key);
          if (entries.length) {
            if (!this.extraSince.has(key)) this.extraSince.set(key, now);
            if (now - this.extraSince.get(key) > 1000) { this.stats.ghosts++; issues.push({ observer: receiver.label, owner: owner.label, reason: 'ghost' }); }
          } else this.extraSince.delete(key);
        }
      }
      this.stats.maximumExpectedPerObserver = Math.max(this.stats.maximumExpectedPerObserver, expectedHere);
    }
    this.stats.samples++; this.stats.expectedPairs += expected; this.stats.observedPairs += observed;
    this.stats.minimumCoverage = Math.min(this.stats.minimumCoverage, expected ? observed / expected : 1);
    return { expectedPairs: expected, observedPairs: observed, activeOwners: live.length, issues: issues.slice(0, 10) };
  }
}

export function verifiedCombatHit(ownerEvents, peerEvents, targetId, ownerAttackerId, peerAttackerId) {
  function evidence(events, attackerId) {
    const strikes = events.filter(event => event.packet === 'ObjectStruck' && event.payload?.objectId === targetId);
    if (strikes.length !== 1 || strikes[0].payload?.attackerId !== attackerId) return null;
    const strike = strikes[0];
    const damage = events.find(event => event.packet === 'DamageIndicator' && event.payload?.objectId === targetId &&
      event.payload.damage > 0 && Math.abs(event.sequence - strike.sequence) <= 3 && Math.abs(event.monotonicMs - strike.monotonicMs) <= 100);
    if (!damage || !Number.isFinite(damage.payload.damage)) return null;
    // Actual Zone order is Struck -> Damage -> Health -> optional Died.
    // Some projections swap Damage/Struck; HP must still follow BOTH. AOI
    // re-entry can send unrelated ObjectHealth earlier in this same wait.
    const lastSequence = Math.max(strike.sequence, damage.sequence), lastAt = Math.max(strike.monotonicMs, damage.monotonicMs);
    const health = events.find(event => event.packet === 'ObjectHealth' && event.payload?.objectId === targetId &&
      Number.isFinite(event.payload.percent) && event.payload.percent >= 0 && event.payload.percent <= 100 &&
      event.sequence > lastSequence && event.sequence - lastSequence <= 3 && event.monotonicMs >= lastAt && event.monotonicMs - lastAt <= 100);
    return health ? { damage: damage.payload.damage, health: health.payload.percent,
      at: health.monotonicMs, receipt: strike.sequence, healthReceipt: health.sequence } : null;
  }
  const owner = evidence(ownerEvents, ownerAttackerId), peer = evidence(peerEvents, peerAttackerId);
  if (!owner || !peer || owner.health !== peer.health || owner.damage !== peer.damage) return null;
  return { damage: owner.damage, healthPercent: owner.health, sharedHealthVerified: true, monotonicMs: Math.max(owner.at, peer.at), receipts: [owner.receipt, peer.receipt] };
}

export class BoundedEvidence {
  constructor(file, maxBufferedBytes = 4 * 1024 * 1024) {
    this.stream = createWriteStream(file, { flags: 'wx' }); this.maxBufferedBytes = maxBufferedBytes;
    this.stream.on('error', error => { this.failure = error; }); this.rows = 0; this.maximumBufferedBytes = 0;
  }
  add(value) {
    if (this.failure) return;
    const line = JSON.stringify(sanitize(value)) + '\n';
    if (this.stream.writableLength + Buffer.byteLength(line) > this.maxBufferedBytes) { this.failure = new Error('Bounded evidence writer overflow; load stopped instead of silently losing samples'); return; }
    this.stream.write(line); this.rows++;
    this.maximumBufferedBytes = Math.max(this.maximumBufferedBytes, this.stream.writableLength);
  }
  async close() { this.stream.end(); await finished(this.stream); if (this.failure) throw this.failure; }
}

export function memoryTrend(samples) {
  const points = samples.filter(sample => Number.isFinite(sample.memory?.currentBytes));
  if (points.length < 2) return { samples: points.length, seconds: 0, slopeMiBPerHour: null };
  const first = points[0].sampledAtMs, xs = points.map(point => (point.sampledAtMs - first) / 3600000), ys = points.map(point => point.memory.currentBytes / 1048576);
  const xMean = xs.reduce((a, b) => a + b) / xs.length, yMean = ys.reduce((a, b) => a + b) / ys.length;
  let numerator = 0, denominator = 0;
  for (let i = 0; i < xs.length; i++) { numerator += (xs[i] - xMean) * (ys[i] - yMean); denominator += (xs[i] - xMean) ** 2; }
  return { samples: points.length, seconds: (points.at(-1).sampledAtMs - first) / 1000,
    slopeMiBPerHour: denominator ? numerator / denominator : null, startMiB: ys[0], endMiB: ys.at(-1), peakMiB: Math.max(...ys) };
}

export class TimeBucketSamples {
  constructor(bucketMs = 3000, capacity = 1201) { this.bucketMs = bucketMs; this.capacity = capacity; this.samples = []; this.rawSamples = 0; }
  add(sample) {
    this.rawSamples++;
    const bucket = Math.floor(sample.sampledAtMs / this.bucketMs), last = this.samples.at(-1);
    if (last && bucket < Math.floor(last.sampledAtMs / this.bucketMs)) throw new Error('Monitor time moved backward across retained buckets');
    if (last && bucket === Math.floor(last.sampledAtMs / this.bucketMs)) this.samples[this.samples.length - 1] = sample;
    else this.samples.push(sample);
    if (this.samples.length > this.capacity) this.samples.shift();
  }
}

export class CapacityMonitor {
  constructor(file, writer) { this.file = file; this.writer = writer; this.timeline = new TimeBucketSamples(); this.stopped = false; this.maximumHistory = this.timeline.capacity; }
  get history() { return this.timeline.samples; }
  stop(reason) { if (!this.stopped) { this.stopped = true; this.reason = reason; this.writer.add({ type: 'safetyStop', at: Date.now(), reason }); } }
  async poll() {
    if (this.stopped || this.polling) return;
    this.polling = true;
    try {
      const sample = inspectMonitor(JSON.parse(await fs.readFile(this.file, 'utf8')));
      if (sample.sampledAtMs !== this.latest?.sampledAtMs) {
        this.latest = sample; this.timeline.add(sample);
        this.writer.add({ type: 'monitor', sample });
        this.peakServerActive = Math.max(this.peakServerActive ?? 0, Number(sample.health?.currentActiveSessions ?? 0));
      }
    } catch (error) { this.stop(error.message); }
    finally { this.polling = false; }
  }
  assert() {
    if (this.writer.failure) this.stop(this.writer.failure.message);
    if (!this.stopped) try { inspectMonitor(this.latest); } catch (error) { this.stop(error.message); }
    if (this.stopped) throw new SafetyStop(this.reason);
  }
  async start() { await this.poll(); this.assert(); this.timer = setInterval(() => void this.poll(), 500); }
  async afterAdmission(at) {
    const deadline = Date.now() + 8000;
    while (Date.now() < deadline) { this.assert(); if (this.latest.sampledAtMs > at) return; await sleep(100); }
    this.stop('No fresh parent monitor sample after admission'); this.assert();
  }
  close() { clearInterval(this.timer); }
}
