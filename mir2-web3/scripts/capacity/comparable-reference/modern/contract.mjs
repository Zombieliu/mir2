// Pure contracts for the isolated File/InMemory game-core-local comparison.
import crypto from 'node:crypto';

export const MAP_SHA = 'd34167876db45bcac3d879e3ca1e8909efc38fffedbfc1511cef4c4215c2f608';
export const LOGIN_WINDOW_MS = 901000;
export const canonical = value => Array.isArray(value) ? value.map(canonical) : value && typeof value === 'object'
  ? Object.fromEntries(Object.keys(value).sort().map(k => [k, canonical(value[k])])) : value;
export const hash = value => crypto.createHash('sha256').update(typeof value === 'string' || Buffer.isBuffer(value)
  ? value : JSON.stringify(canonical(value))).digest('hex');
export const requireThat = (value, message) => { if (!value) throw new Error(message); };
export const owner = snapshot => snapshot?.entities?.find(e => Number(e.objectId) === Number(snapshot.playerObjectId));
export const percentile = (values, q = .95) => values.length ? [...values].sort((a, b) => a - b)[Math.ceil(values.length * q) - 1] : null;

// ModernClient assigns strictly increasing receiveSeq and only appends or trims
// the front of this history. Cache references only to still-retained matches;
// neither the history limit nor packet capture/causal/deadline rules change.
function firstAfterSequence(events, sequence) {
  let lo = 0, hi = events.length;
  while (lo < hi) { const mid = (lo + hi) >>> 1; if (events[mid].receiveSeq <= sequence) lo = mid + 1; else hi = mid; }
  return lo;
}
export class RetainedReceiveCursor {
  constructor(floor, predicate) {
    this.floor = floor; this.predicate = predicate; this.through = floor; this.history = null; this.selected = [];
  }
  matches(events) {
    if (events !== this.history) { this.history = events; this.through = this.floor; this.selected = []; }
    if (!events.length) { this.selected = []; return this.selected; }
    // A replaced/reset stream is not an extension of the previously seen one.
    const last = events.at(-1).receiveSeq;
    if (last < this.through) { this.through = this.floor; this.selected = []; }
    const earliest = events[0].receiveSeq;
    const removed = firstAfterSequence(this.selected, earliest - 1);
    if (removed) this.selected.splice(0, removed);
    if (last <= this.through) return this.selected;
    const end = events.length;
    for (let i = firstAfterSequence(events, this.through); i < end; i++) {
      const event = events[i];
      if (event.receiveSeq > this.floor && this.predicate(event)) this.selected.push(event);
      this.through = event.receiveSeq;
    }
    return this.selected;
  }
}

export function decodeNavigation(nav, map) {
  requireThat(nav?.schema === 'mir2.comparable.navigation.v1' && hash(nav.map) === hash(map) && nav.map?.rawSha256 === MAP_SHA &&
    nav.encoding === 'row-major-lsb0-bitset' && nav.source === 'original-library-cell-valid-and-movement-exclusion' && nav.gameCodeUnmodified === true &&
    nav.exclusions?.movementRadius === 3 && nav.exclusions.npcRadius === 3 && nav.exclusions.allDoorCells === true && nav.exclusions.safeZones === true &&
    typeof nav.eligibleBitsBase64 === 'string', 'Original bitset navigation contract required');
  const bits = Buffer.from(nav.eligibleBitsBase64, 'base64');
  requireThat(bits.length === 80000 && bits.toString('base64') === nav.eligibleBitsBase64, 'Exact canonical80000-byte navigation bitset required');
  const count = bits.reduce((total, byte) => { let n = byte; while (n) { total++; n &= n - 1; } return total; }, 0);
  requireThat(count === 404451 && count === nav.eligibleCount, 'Navigation eligible count mismatch');
  return (x, y) => Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0 && x < 800 && y < 800 &&
    Boolean(bits[(y * 800 + x) >> 3] & (1 << ((y * 800 + x) & 7)));
}

export function validatePlan(plan) {
  requireThat(plan?.schema === 'mir2.comparable.plan.v1' && plan.qualificationLayer === 'game-core-local', 'Only game-core-local plan is supported');
  requireThat(plan.scenario === 'dense-movement-chat', 'This adapter does not yet implement dispersed-melee');
  requireThat(plan.definitionSha256 === '9172f5786dd1fad2bcbd3d0fc173c4070c738322710707128b9466e245a4e6e5' &&
    plan.scheduling?.version === 2 && plan.scheduling.chatPhaseRule === 'same-primary-phase-with-midpoint-offset' &&
    plan.scheduling.primaryIntervalMs === 2000 && plan.scheduling.minimumSameActorOfferGapMs === 1000 &&
    plan.scheduling.chatOffsetWithinPrimaryIntervalMs === 1000, 'Corrected common v2 scheduling definition required');
  requireThat((plan.purpose === 'functional' && plan.population === 2 && plan.durationMs === 60000) ||
    (plan.purpose === 'capacity' && [25, 50, 100].includes(plan.population) && plan.durationMs === 300000), 'Purpose/population/duration mismatch');
  requireThat(plan.map?.index === 440 && plan.map.fileName === 'DogYoHyun' && plan.map.rawSha256 === MAP_SHA &&
    plan.map.width === 800 && plan.map.height === 800, 'Active original Map440 required');
  requireThat(plan.actorTemplate?.class === 'Warrior' && plan.actorTemplate.level === 40 && plan.actorTemplate.gender === 'Male', 'L40 male Warrior fixture required');
  requireThat(Array.isArray(plan.actors) && plan.actors.length === plan.population && Array.isArray(plan.actions), 'Complete plan actors/actions required');
  requireThat(/^[0-9a-f]{20}$/.test(plan.planId), 'Pinned plan ID required');
  requireThat(/^[0-9a-f]{64}$/.test(plan.generatorSourceSha256) && plan.assessment?.windowMs === 60000 && plan.assessment.perActor === true &&
    plan.assessment.allCompleteWindowsRequired === true && plan.assessment.failedActionCanBecomeCompleted === false &&
    plan.assessment.notSentIsTimeout === false && plan.assessment.notSentIsCompleted === false &&
    plan.assessment.safetyOrEvidenceFailureIsHardFailure === true, 'Explicit per-actor minute assessment required');
  const cells = new Set(), tickets = new Set();
  for (const [ordinal, actor] of plan.actors.entries()) {
    requireThat(actor.ordinal === ordinal && Array.isArray(actor.peerOrdinals) && actor.peerOrdinals.length > 0 &&
      new Set(actor.peerOrdinals).size === actor.peerOrdinals.length && actor.peerOrdinals.every(p => Number.isSafeInteger(p) && p >= 0 && p < plan.population && p !== ordinal), 'Invalid actor/peer namespace');
    requireThat(actor.lane?.length === 2 && actor.lane.every(p => Number.isInteger(p.x) && Number.isInteger(p.y) && p.x > 0 && p.y > 0 && p.x < 800 && p.y < 800) &&
      actor.start.x === actor.lane[0].x && actor.start.y === actor.lane[0].y && actor.lane[1].x === actor.start.x + 1 && actor.lane[1].y === actor.start.y, 'Exact two-cell horizontal lane required');
    for (const p of actor.lane) { const key = `${p.x},${p.y}`; requireThat(!cells.has(key), 'Movement corridors overlap'); cells.add(key); }
  }
  for (const action of plan.actions) {
    const key = `${action.actorOrdinal}:${action.ticketSequence}`;
    requireThat(Number.isSafeInteger(action.actorOrdinal) && action.actorOrdinal >= 0 && action.actorOrdinal < plan.population &&
      Number.isSafeInteger(action.ticketSequence) && action.ticketSequence > 0 && !tickets.has(key), 'Duplicate/invalid ticket'); tickets.add(key);
    requireThat(['walk', 'chat'].includes(action.kind) && Number.isSafeInteger(action.dueMs) && action.dueMs >= 0 && action.dueMs < plan.durationMs &&
      Number.isSafeInteger(action.deadlineMs) && action.deadlineMs > action.dueMs, 'Invalid action or absolute deadline');
    requireThat(action.deadlineMs - action.dueMs === plan.thresholds[`${action.kind}DeadlineMs`], 'Deadline drift');
    if (action.kind === 'walk') requireThat(['Right', 'Left'].includes(action.direction) &&
      plan.actors[action.actorOrdinal].lane.some(p => p.x === action.target?.x && p.y === action.target?.y), 'Walk target outside declared lane');
    else requireThat(typeof action.nonce === 'string' && /^m2cmp:[a-z0-9:]+$/.test(action.nonce) && action.nonce.length <= 100 &&
      action.nonceSha256 === hash(action.nonce), 'Exact chat nonce/hash required');
  }
  requireThat(plan.actions.every((a, i) => !i || a.dueMs > plan.actions[i - 1].dueMs ||
    (a.dueMs === plan.actions[i - 1].dueMs && a.actorOrdinal >= plan.actions[i - 1].actorOrdinal)), 'Globally ordered fixed offers required');
  for (const actor of plan.actors) {
    const expected = [];
    for (let due = Math.floor(actor.ordinal * 2000 / plan.population), step = 0; due < plan.durationMs; due += 2000, step++)
      expected.push({ kind: 'walk', dueMs: due, direction: step % 2 ? 'Left' : 'Right', target: actor.lane[step % 2 ? 0 : 1] });
    for (let due = 15000 + Math.floor(actor.ordinal * 2000 / plan.population); due < plan.durationMs; due += 30000) expected.push({ kind: 'chat', dueMs: due });
    expected.sort((a, b) => a.dueMs - b.dueMs || a.kind.localeCompare(b.kind));
    const actual = plan.actions.filter(a => a.actorOrdinal === actor.ordinal).sort((a, b) => a.dueMs - b.dueMs || a.kind.localeCompare(b.kind));
    requireThat(actual.length === expected.length && actual.every((a, i) => a.ticketSequence === i + 1 && a.kind === expected[i].kind && a.dueMs === expected[i].dueMs &&
      (a.kind === 'chat' ? a.nonce === `m2cmp:${plan.planId}:${actor.ordinal}:${i + 1}` : a.direction === expected[i].direction &&
        a.target.x === expected[i].target.x && a.target.y === expected[i].target.y)), 'Complete fixed cadence required for every actor');
  }
  requireThat(plan.thresholds.equipmentAckDeadlineMs === 15000 && plan.thresholds.equipmentStateDeadlineMs === 15000 &&
    plan.thresholds.logoutAckDeadlineMs === 5000 && plan.thresholds.transportCloseDeadlineMs === 5000 &&
    plan.causality?.absoluteDeadlineFromPlannedTime === true && plan.causality?.maximumUnresolvedActionsPerActor === 1 &&
    plan.causality?.lateReceiptCanRestoreSuccess === false, 'Lifecycle/causality contract drift');
  const fixed = { minimumCompletionRatio: .95, maximumTimeoutRatioExclusive: .01, movementP95Ms: 750, chatP95Ms: 1000,
    scheduleP95Ms: 250, walkDeadlineMs: 2000, chatDeadlineMs: 2500 };
  requireThat(Object.entries(fixed).every(([k, v]) => plan.thresholds[k] === v), 'Common v1 thresholds drifted');
  requireThat(plan.resourceContract?.logicalCpus === 4 && plan.resourceContract.memoryCommitBytes === 8589934592 &&
    plan.resourceContract.budgetScope === 'all-declared-server-processes', 'Shared total resource contract required');
  requireThat(JSON.stringify(plan.equipmentActorOrdinals) === JSON.stringify(plan.purpose === 'functional' ? [0, 1] : [0, 1, 2]) &&
    plan.equipmentRoundsPerActor === 2, 'Shared equipment phase selection required');
  return plan;
}

// One action remains fenced after its deadline until its unique evidence closes.
// Timing out can never be converted into a successful action by late delivery.
export class PendingGate {
  constructor(emit = () => {}) { this.pending = null; this.failed = false; this.emit = emit; }
  start(spec, now) {
    if (this.pending) { this.failed = true; this.emit({ event: 'notSent', reason: 'previousActionPending', ...spec, atMs: now }); return false; }
    this.pending = { ...spec, receipts: [], timedOut: false }; return true;
  }
  expire(now) {
    if (this.pending && !this.pending.timedOut && now >= this.pending.deadlineAtMs) {
      this.pending.timedOut = true; this.failed = true; this.emit({ event: 'actionTimeout', ...this.pending, atMs: now });
    }
  }
  close(receipts, now, accepted = true) {
    if (!this.pending) throw new Error('No unique pending action');
    const action = this.pending;
    // Receipt time, rather than polling time, owns the deadline verdict.
    const last = receipts.length ? Math.max(...receipts.map(r => r.atMs)) : now;
    const timedOut = action.timedOut || last > action.deadlineAtMs;
    const passed = accepted && !timedOut && !action.sendFailed;
    if (!passed) this.failed = true;
    this.pending = null;
    const result = { ...action, receipts, completedAtMs: last, accepted, passed, timedOut, late: timedOut };
    this.emit({ event: 'actionTerminal', ...result }); return result;
  }
}

export function equipmentFingerprint(snapshot) {
  requireThat(Array.isArray(snapshot?.inventoryItems) && Array.isArray(snapshot?.equipmentItems) &&
    Array.isArray(snapshot?.beltItems) && Number.isFinite(snapshot?.gold) && Number.isFinite(snapshot?.inventoryCapacity) &&
    Number.isFinite(snapshot?.maxBagSlots), 'Complete carried state required');
  const ordered = key => [...snapshot[key]].sort((a, b) => String(a.uniqueId).localeCompare(String(b.uniqueId)));
  return hash({ inventory: ordered('inventoryItems'), equipment: ordered('equipmentItems'), belt: ordered('beltItems'),
    gold: snapshot.gold, inventoryCapacity: snapshot.inventoryCapacity, maxBagSlots: snapshot.maxBagSlots });
}

export function snapshotProjection(snapshot) {
  const keys = ['objectId', 'kind', 'name', 'x', 'y', 'direction', 'class', 'gender', 'level', 'dead'];
  const entities = Object.freeze(Array.isArray(snapshot?.entities) ? snapshot.entities.map(e =>
    Object.freeze(Object.fromEntries(keys.filter(k => e[k] !== undefined).map(k => [k, e[k]])))) : []);
  const complete = ['inventoryItems', 'equipmentItems', 'beltItems'].every(k => Array.isArray(snapshot?.[k])) &&
    ['gold', 'inventoryCapacity', 'maxBagSlots'].every(k => Number.isFinite(snapshot?.[k]));
  return Object.freeze({ playerObjectId: snapshot?.playerObjectId ?? null, mapFileName: snapshot?.mapFileName ?? null,
    playerHp: snapshot?.playerHp ?? null, playerMp: snapshot?.playerMp ?? null,
    entities, owner: entities.find(e => Number(e.objectId) === Number(snapshot?.playerObjectId)) ?? null,
    entityCount: Array.isArray(snapshot?.entities) ? snapshot.entities.length : null, entitiesSha256: hash(entities),
    carriedFingerprint: complete ? equipmentFingerprint(snapshot) : null });
}

export function savedState(snapshot) {
  const p = owner(snapshot); requireThat(p && p.dead !== true && snapshot.playerHp > 0, 'Living exact owner required');
  return { map: snapshot.mapFileName, x: p.x, y: p.y, direction: p.direction, name: p.name, level: p.level,
    class: p.class, gender: p.gender, hp: snapshot.playerHp, mp: snapshot.playerMp, items: equipmentFingerprint(snapshot) };
}

export function scoreWindows(plan, results) {
  const rows = [];
  for (const actor of plan.actors) for (let minute = 0; minute < plan.durationMs / 60000; minute++) {
    const expected = plan.actions.filter(a => a.actorOrdinal === actor.ordinal && Math.floor(a.dueMs / 60000) === minute);
    const actual = expected.map(a => results.find(r => r.actorOrdinal === a.actorOrdinal && r.ticketSequence === a.ticketSequence));
    const complete = actual.filter(r => r?.passed), timeout = actual.filter(r => r?.timedOut && Number.isFinite(r?.sentAtMs)).length;
    const latency = kind => percentile(complete.filter(r => r.kind === kind).map(r => r.completedAtMs - r.sentAtMs));
    const schedule = percentile(actual.filter(r => Number.isFinite(r?.sentAtMs)).map(r => r.scheduleDelayMs));
    const fraction = expected.length ? complete.length / expected.length : null;
    const p95Move = latency('walk'), p95Chat = latency('chat');
    const passed = fraction !== null && fraction >= plan.thresholds.minimumCompletionRatio && timeout / expected.length < plan.thresholds.maximumTimeoutRatioExclusive &&
      p95Move !== null && p95Move <= plan.thresholds.movementP95Ms && p95Chat !== null && p95Chat <= plan.thresholds.chatP95Ms &&
      schedule !== null && schedule <= plan.thresholds.scheduleP95Ms;
    rows.push({ actorOrdinal: actor.ordinal, minute, offered: expected.length, completed: complete.length, timeout,
      notSent: actual.filter(r => !Number.isFinite(r?.sentAtMs)).length,
      rejected: actual.filter(r => r?.accepted === false && Number.isFinite(r?.sentAtMs)).length,
      completionRatio: fraction, movementP95Ms: p95Move,
      chatP95Ms: p95Chat, scheduleP95Ms: schedule, passed });
  }
  return rows;
}

export function freezeWindowFailures(previous, current) {
  return current.map(row => previous.some(p => p.actorOrdinal === row.actorOrdinal && p.minute === row.minute && p.passed === false)
    ? { ...row, passed: false, earlierFailedAssessmentRetained: true } : row);
}

export function localLoginDelay(attempts, wallMs, required = 1) {
  requireThat(Array.isArray(attempts) && Number.isSafeInteger(required) && required > 0 && required <= 30 &&
    attempts.every(a => Number.isFinite(a.atMs) && a.atMs >= 0 && typeof a.purpose === 'string'), 'Invalid independent local login ledger');
  const recent = attempts.filter(a => wallMs - a.atMs < LOGIN_WINDOW_MS).map(a => a.atMs).sort((a, b) => a - b);
  const retire = recent.length + required - 30;
  return retire <= 0 ? 0 : Math.max(1, recent[retire - 1] + LOGIN_WINDOW_MS - wallMs);
}
