// Public aggregate counts and wall-clock phase bounds only. This module does
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
