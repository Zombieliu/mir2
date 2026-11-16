// Public observations and wall-clock phase bounds only. This module does
// not plan gameplay, inspect credentials, change load or decide acceptance.
import { capacityActorLimit } from './playtest-capacity-soak-core.mjs';

const fields = ['total', 'players', 'monsters', 'npcs', 'other', 'groundDropsReported'];
const zero = () => Object.fromEntries(fields.map(field => [field, 0]));
const maxima = (left, right) => Object.fromEntries(fields.map(field => [field, Math.max(left[field], right[field])]));
const scope = 'Observed independent maxima at nominal 1000ms sampling cadence, not exhaustive peaks. total counts snapshot.entities; groundDropsReported counts snapshot.groundDrops separately and is not added to total. Maxima may come from different observers.';

export class PublicDensity {
  constructor(policy = {}) {
    this.maximumControlledActors = capacityActorLimit(policy);
    this.capacityMetadata = this.maximumControlledActors === 500 ? { capacityTargetActors: 500, maximumControlledActors: 500 } : {};
    this.sampleCount = 0; this.maps = new Map();
  }
  sample(clients) {
    if (!Array.isArray(clients) || clients.length > this.maximumControlledActors) throw new Error('Density requires at most ' + this.maximumControlledActors + ' controlled actors');
    const maps = new Map(); let cohortActive = 0, cohortCombatActive = 0;
    for (const client of clients) {
      const snapshot = client.snapshot;
      if (!client.ready || client.paused || client.closed || !client.inGame || !Number.isInteger(snapshot?.playerObjectId) ||
          !Array.isArray(snapshot.entities) || !snapshot.entities.some(entity => entity.objectId === snapshot.playerObjectId)) continue;
      const mapFileName = /^[a-z0-9_-]{1,80}$/i.test(String(snapshot.mapFileName)) ? String(snapshot.mapFileName) : '[unavailable]';
      const counts = zero(); counts.total = snapshot.entities.length;
      counts.groundDropsReported = Array.isArray(snapshot.groundDrops) ? snapshot.groundDrops.length : 0;
      for (const entity of snapshot.entities) {
        const kind = String(entity.kind ?? '').toLowerCase();
        counts[['selfplayer', 'remoteplayer', 'player'].includes(kind) ? 'players' : kind === 'monster' ? 'monsters' : kind === 'npc' ? 'npcs' : 'other']++;
      }
      const map = maps.get(mapFileName) ?? { mapFileName, cohortActive: 0, cohortCombatActive: 0, observerSamples: 0, maxVisible: zero() };
      map.cohortActive++; map.observerSamples++; cohortActive++;
      if (client.role === 'combat') { map.cohortCombatActive++; cohortCombatActive++; }
      map.maxVisible = maxima(map.maxVisible, counts); maps.set(mapFileName, map);
    }
    for (const [key, value] of maps) {
      if (!this.maps.has(key) && this.maps.size >= 100) throw new Error('Density map history exceeds the bounded cohort scope');
      const prior = this.maps.get(key) ?? { mapFileName: key, maxCohortActive: 0, maxCohortCombatActive: 0, observerSamples: 0, maxVisible: zero() };
      prior.maxCohortActive = Math.max(prior.maxCohortActive, value.cohortActive);
      prior.maxCohortCombatActive = Math.max(prior.maxCohortCombatActive, value.cohortCombatActive);
      prior.observerSamples += value.observerSamples; prior.maxVisible = maxima(prior.maxVisible, value.maxVisible); this.maps.set(key, prior);
    }
    this.sampleCount++;
    return { samplingIntervalMs: 1000, scope, ...this.capacityMetadata, cohortActive, cohortCombatActive, maps: [...maps.values()].sort((a, b) => a.mapFileName.localeCompare(b.mapFileName)) };
  }
  summary() {
    return { samplingIntervalMs: 1000, scope, ...this.capacityMetadata, sampleCount: this.sampleCount,
      maps: structuredClone([...this.maps.values()].sort((a, b) => a.mapFileName.localeCompare(b.mapFileName))) };
  }
}

export function beginCapacityPhase(context, name, kind, targetActors, at = Date.now()) {
  const maximumControlledActors = capacityActorLimit(context.policy ?? {});
  if (!['bootstrap', 'measurement'].includes(kind) || typeof name !== 'string' || name.length > 80 ||
      !Number.isInteger(targetActors) || targetActors < 1 || targetActors > maximumControlledActors || !Number.isSafeInteger(at) || at < 0) throw new Error('Invalid bounded capacity phase');
  const phase = { name, kind, targetActors, ...(maximumControlledActors === 500 ? { capacityTargetActors: 500, maximumControlledActors } : {}), startedAtMs: at, endedAtMs: null, completed: false };
  (context.phaseWindows ??= []).push(phase);
  context.writer.add({ type: 'phaseStarted', at, ...phase });
  return phase;
}

export function finishCapacityPhase(context, phase, completed, at = Date.now()) {
  if (phase.endedAtMs !== null) return phase;
  if (typeof completed !== 'boolean' || !Number.isSafeInteger(at) || at < phase.startedAtMs) throw new Error('Invalid capacity phase end clock');
  phase.endedAtMs = at; phase.completed = completed;
  // completed means that the interval finished, never that its quality passed.
  context.writer.add({ type: 'phaseEnded', at, ...phase });
  return phase;
}

const sustainScope = 'Passive actual worldSnapshot scalars only; stock is HP item 658 across complete inventory and belt arrays. Stock changes are net observations, not proof of consumption. Matching native HP recovery ACKs, inventory consumption proofs and observed HP increases are counted separately; none proves potion healing. No boundary requests a packet or decides acceptance.';
const exactNonnegative = value => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
const publicName = value => typeof value === 'string' && value.length > 0 && value.length <= 80 ? value : null;
const itemTemplate = item => item?.itemIndex ?? item?.item_index ??
  (item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info)?.item_index ??
  (item?.tooltipSource?.realInfo ?? item?.tooltipSource?.real_info ?? item?.tooltipSource?.info)?.itemIndex;
function hpStock(snapshot) {
  if (!Array.isArray(snapshot.inventoryItems) || !Array.isArray(snapshot.beltItems)) return null;
  const seen = new Set(); let stock = 0;
  for (const items of [snapshot.inventoryItems, snapshot.beltItems]) for (const item of items) {
    const id = item?.uniqueId, template = exactNonnegative(itemTemplate(item)), quantity = exactNonnegative(item?.quantity);
    const key = typeof id === 'number' && Number.isSafeInteger(id) && id >= 0 ? String(id) :
      typeof id === 'string' && /^(0|[1-9][0-9]*)$/.test(id) ? id : null;
    if (key === null || seen.has(key) || template === null || quantity === null) return null;
    seen.add(key);
    if (template === 658) { stock += quantity; if (!Number.isSafeInteger(stock)) return null; }
  }
  return stock;
}
const sameSustainOwner = (a, b) => Boolean(a?.owner && b?.owner && a.connection === b.connection &&
  a.owner.name === b.owner.name && a.owner.mapFileName === b.owner.mapFileName && a.owner.localId === b.owner.localId);
function sustainAccumulator(at, latest) {
  return { startedAtMonotonicMs: at, stockStart: latest?.complete.identity && latest.complete.hpStock ? {
    connection: latest.connection, owner: { ...latest.owner }, hpStock: latest.hpStock,
    sequence: latest.sequence, monotonicMs: latest.monotonicMs } : null,
  worldSnapshots: 0, completeStockSnapshots: 0, unknownStockSnapshots: 0, identityChanges: 0,
  minimumObservedHp: null, maximumObservedHp: null, hpRecoveryAcknowledgements: 0,
  hpRecoveryConsumptionProofs: 0, hpRecoveryProvenConsumedQuantity: 0, hpRecoveryHpIncreaseProofs: 0 };
}

// One latest scalar and one current accumulator per actor. No packet, item list,
// snapshot history or per-minute history is retained by this observer.
export class SustainObservations {
  constructor(policy = {}, emit = () => {}) {
    this.maximumControlledActors = capacityActorLimit(policy); this.emit = emit; this.actors = new Map();
  }
  entry(client) {
    const actor = publicName(client?.label);
    if (actor === null || !Number.isSafeInteger(client.lifecycleConnection)) return null;
    if (!this.actors.has(actor)) {
      if (this.actors.size >= this.maximumControlledActors) return null;
      this.actors.set(actor, { actor, latest: null, active: false, connection: null, accumulator: null });
    }
    return this.actors.get(actor);
  }
  observe(client, message, receipt, receivedAtMs = Date.now()) {
    if (message?.type !== 'worldSnapshot' || receipt?.direction !== 'received' ||
        !Number.isSafeInteger(receipt.sequence) || !Number.isFinite(receipt.monotonicMs) ||
        !Number.isSafeInteger(receivedAtMs)) return;
    const state = this.entry(client); if (!state) return;
    if (state.latest?.connection === client.lifecycleConnection && receipt.sequence <= state.latest.sequence) return;
    if (state.latest && receipt.monotonicMs < state.latest.monotonicMs) return;
    const snapshot = message.payload ?? {}, localId = exactNonnegative(snapshot.playerObjectId);
    const owner = Array.isArray(snapshot.entities) ? snapshot.entities.find(entity => entity?.objectId === localId) : null;
    const name = publicName(owner?.name), mapFileName = publicName(snapshot.mapFileName);
    const identity = localId !== null && localId > 0 && name === client.label && mapFileName !== null;
    const hp = exactNonnegative(snapshot.playerHp), maximumHp = exactNonnegative(snapshot.playerMaxHp);
    const stock = hpStock(snapshot), gold = exactNonnegative(snapshot.gold);
    const scalar = { connection: client.lifecycleConnection, owner: identity ? { name, mapFileName, localId } : null,
      sequence: receipt.sequence, monotonicMs: receipt.monotonicMs, receivedAtMs,
      hp: identity ? hp : null, maximumHp: identity && maximumHp > 0 ? maximumHp : null,
      hpStock: identity ? stock : null, gold: identity ? gold : null,
      inSafeZone: identity && typeof snapshot.inSafeZone === 'boolean' ? snapshot.inSafeZone : null,
      complete: { identity, hp: identity && hp !== null, maximumHp: identity && maximumHp !== null && maximumHp > 0,
        hpStock: identity && stock !== null, gold: identity && gold !== null } };
    const prior = state.latest; state.latest = scalar;
    if (!state.active || state.connection !== client.lifecycleConnection) return;
    const acc = state.accumulator; acc.worldSnapshots++;
    if (scalar.complete.hpStock) acc.completeStockSnapshots++; else acc.unknownStockSnapshots++;
    if (!sameSustainOwner(prior, scalar)) acc.identityChanges++;
    if (scalar.complete.hp) {
      acc.minimumObservedHp = acc.minimumObservedHp === null ? hp : Math.min(acc.minimumObservedHp, hp);
      acc.maximumObservedHp = acc.maximumObservedHp === null ? hp : Math.max(acc.maximumObservedHp, hp);
    }
  }
  activate(client, phase, atMonotonicMs = performance.now(), at = Date.now()) {
    const state = this.entry(client); if (!state || state.active) return;
    state.connection = client.lifecycleConnection; state.role = client.role; state.active = true;
    const latest = state.latest?.connection === state.connection ? state.latest : null;
    state.accumulator = sustainAccumulator(atMonotonicMs, latest); state.offeredEndedAtMonotonicMs = null;
    this.publish(state, 'activation', phase, atMonotonicMs, at);
  }
  bindResumedConnection(client) {
    const state = this.actors.get(client.label); if (!state?.active) return;
    if (state.connection !== client.lifecycleConnection) state.accumulator.identityChanges++;
    state.connection = client.lifecycleConnection;
  }
  recordHpRecovery(client, kind, quantity = 0, firstProof = true) {
    const state = this.actors.get(client.label);
    if (!state?.active || state.connection !== client.lifecycleConnection) return;
    const acc = state.accumulator;
    if (kind === 'ack') acc.hpRecoveryAcknowledgements++;
    else if (kind === 'hp-increase') acc.hpRecoveryHpIncreaseProofs++;
    else if (kind === 'consumption' && Number.isSafeInteger(quantity) && quantity > 0) {
      if (firstProof) acc.hpRecoveryConsumptionProofs++;
      acc.hpRecoveryProvenConsumedQuantity += quantity;
    }
  }
  tick(phase, atMonotonicMs = performance.now(), at = Date.now()) {
    for (const state of this.actors.values()) if (state.active && state.offeredEndedAtMonotonicMs === null &&
        atMonotonicMs - state.accumulator.startedAtMonotonicMs >= 60000) {
      this.publish(state, 'minute', phase, atMonotonicMs, at);
      state.accumulator = sustainAccumulator(atMonotonicMs, state.latest?.connection === state.connection ? state.latest : null);
    }
  }
  stop(atMonotonicMs) {
    for (const state of this.actors.values()) if (state.active) state.offeredEndedAtMonotonicMs = atMonotonicMs;
  }
  drain(client, phase, atMonotonicMs = performance.now(), at = Date.now()) {
    const state = this.actors.get(client.label);
    if (!state?.active || state.connection !== client.lifecycleConnection) return;
    this.publish(state, 'drain', phase, atMonotonicMs, at); state.active = false; state.accumulator = null;
  }
  publish(state, boundary, phase, end, at) {
    const acc = state.accumulator, elapsedMs = end - acc.startedAtMonotonicMs;
    const latest = state.latest?.connection === state.connection ? state.latest : null;
    const ageMs = latest && end >= latest.monotonicMs ? end - latest.monotonicMs : null;
    const comparable = acc.identityChanges === 0 && sameSustainOwner(acc.stockStart, latest) && latest.complete.hpStock &&
      latest.sequence > acc.stockStart.sequence && latest.monotonicMs > acc.stockStart.monotonicMs;
    const stockChange = comparable ? latest.hpStock - acc.stockStart.hpStock : null;
    const { stockStart, ...counts } = acc;
    this.emit({ type: 'sustain', boundary, actor: state.actor, role: state.role, phase, at, scope: sustainScope,
      connection: state.connection, emittedAtMonotonicMs: end, offeredEndedAtMonotonicMs: state.offeredEndedAtMonotonicMs,
      elapsedMs, completeMinute: boundary === 'minute' && elapsedMs >= 60000,
      latest: latest ? { ...latest, owner: latest.owner ? { ...latest.owner } : null, complete: { ...latest.complete }, ageMs,
        observedDuringInterval: latest.monotonicMs >= acc.startedAtMonotonicMs } : null,
      observations: { ...counts, stockStart: stockStart ? { ...stockStart, owner: { ...stockStart.owner } } : null,
        observedNetHpStockChange: stockChange,
        hpRecoveryProvenConsumedPerMinute: elapsedMs > 0 ? acc.hpRecoveryProvenConsumedQuantity * 60000 / elapsedMs : null } });
  }
}
