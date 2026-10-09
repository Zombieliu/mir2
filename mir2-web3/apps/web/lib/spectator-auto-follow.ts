/** Rank only the delayed, redacted frames already delivered to this spectator. */
type Entity = {
  objectId: number;
  kind: string;
  name: string;
  x: number;
  y: number;
  hp?: number | null;
  maxHp?: number | null;
  dead: boolean;
};

type World = {
  mapFileName?: string | null;
  playerObjectId: number | null;
  playerHp?: number | null;
  playerMaxHp?: number | null;
  entities: Entity[];
};

export type SpectatorFollowFrame = {
  readOnly: true;
  map: string;
  target: string | null;
  director: boolean;
  camera: { x: number; y: number } | null;
  recordingId: string | null;
  sequence: number | null;
  capturedAtMs: number | null;
  events: Array<{
    kind: string;
    atMs: number;
    objectId: number | null;
    payload: Record<string, unknown>;
  }>;
};

export type SpectatorFollowDecision = {
  source: 'delayedSnapshot';
  mode: 'automatic' | 'manual' | 'manualUnavailable' | 'serverCamera';
  objectId: number | null;
  name: string | null;
  score: number;
  reason: 'recentDamage' | 'nearbyCombat' | 'holding' | 'idle' | 'manual' | 'unavailable' | 'serverCamera';
  heldForMs: number;
};

type Heat = { score: number; lastActivityAt: number; reason: 'recentDamage' | 'nearbyCombat' };
type DirectedFrame<T> = { world: T; decision: SpectatorFollowDecision };
const MIN_HOLD_MS = 8_000;
const RESET_GAP_MS = 15_000;
const MAX_ENTITIES = 256;
const MAX_EVENTS = 1_024;

function validId(id: number | null): id is number {
  return id !== null && Number.isSafeInteger(id) && id > 0 && id < 0xffffffff;
}

function alivePlayer(entity: Entity) {
  return (entity.kind === 'player' || entity.kind === 'selfPlayer')
    && validId(entity.objectId) && entity.dead === false
    && (entity.hp == null || (Number.isFinite(entity.hp) && entity.hp > 0));
}

function distance(a: Entity, b: Entity) {
  return Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
}

function frameKey(frame: SpectatorFollowFrame): string | null {
  if (frame.readOnly !== true || !frame.map || !frame.recordingId
      || !Number.isSafeInteger(frame.sequence) || (frame.sequence ?? 0) <= 0
      || !Number.isSafeInteger(frame.capturedAtMs) || (frame.capturedAtMs ?? 0) <= 0) return null;
  return JSON.stringify([frame.map, frame.recordingId, frame.sequence, frame.capturedAtMs]);
}

export class SpectatorAutoFollow {
  private identity: string | null = null;
  private lastSequence = 0;
  private lastAt = 0;
  private selectedId: number | null = null;
  private selectedAt = 0;
  private manualTarget: string | null = null;
  private heat = new Map<number, Heat>();

  reset() {
    this.identity = null;
    this.lastSequence = 0;
    this.lastAt = 0;
    this.selectedId = null;
    this.selectedAt = 0;
    this.manualTarget = null;
    this.heat.clear();
  }

  direct<T extends World>(world: T, frame: SpectatorFollowFrame): DirectedFrame<T> {
    const at = frame.capturedAtMs ?? 0;
    const identity = JSON.stringify([frame.map, frame.recordingId]);
    if (identity !== this.identity || at < this.lastAt || (frame.sequence ?? 0) < this.lastSequence
        || at - this.lastAt > RESET_GAP_MS) {
      this.reset();
      this.identity = identity;
    }
    const players = world.entities.filter(alivePlayer).slice(0, MAX_ENTITIES);
    // A merged AOI may contain more monsters than one session's entity budget.
    // Always include every bounded player candidate, regardless of array order.
    const byId = new Map([...world.entities.filter((entity) => entity.kind !== 'player'
      && entity.kind !== 'selfPlayer').slice(0, MAX_ENTITIES), ...players]
      .map((entity) => [entity.objectId, entity]));
    const aliveIds = new Set(players.map((player) => player.objectId));
    for (const id of this.heat.keys()) if (!aliveIds.has(id)) this.heat.delete(id);
    // Capture time, rather than wall time, keeps pause/seek and public delay honest.
    if ((frame.sequence ?? 0) > this.lastSequence && at > this.lastAt) {
      const decay = 2 ** (-(at - this.lastAt) / 6_000);
      for (const heat of this.heat.values()) heat.score *= decay;
      for (const event of frame.events.slice(0, MAX_EVENTS)) {
        if (!Number.isSafeInteger(event.atMs) || event.atMs > at
            || event.atMs <= this.lastAt || at - event.atMs > 4_000 || !validId(event.objectId)) continue;
        const entity = byId.get(event.objectId);
        if (!entity) continue;
        const from = event.payload.from;
        const to = event.payload.to;
        if (event.kind === 'health' && typeof from === 'number' && typeof to === 'number'
            && Number.isFinite(from) && Number.isFinite(to) && from > to && from > 0 && to >= 0) {
          const fraction = Math.min(1, (from - to) / Math.max(1, entity.maxHp ?? from));
          if (aliveIds.has(entity.objectId)) {
            this.addHeat(entity.objectId, 30 + fraction * 100, event.atMs, 'recentDamage');
          } else if (entity.kind === 'monster') {
            this.nearbyHeat(players, entity, 20 + fraction * 40, event.atMs);
          }
        } else if (event.kind === 'death' && entity.kind === 'monster' && entity.dead === true) {
          this.nearbyHeat(players, entity, 35, event.atMs);
        }
      }
      this.lastSequence = frame.sequence ?? 0;
      this.lastAt = at;
    }
    const scored = players.map((player) => {
      const heat = this.heat.get(player.objectId);
      const active = heat && at - heat.lastActivityAt <= 6_000;
      // Static low HP or merely standing by many monsters is not recent combat.
      const danger = active && player.hp != null && player.maxHp
        && player.hp / player.maxHp <= 0.25 ? 20 : 0;
      const nearby = active ? [...byId.values()].filter((entity) => entity.kind === 'monster'
        && entity.dead === false && (entity.hp == null || entity.hp > 0)
        && distance(player, entity) <= 8).length : 0;
      return { player, score: (heat?.score ?? 0) + danger + Math.min(6, nearby) * 5,
        reason: active ? heat.reason : 'idle' as SpectatorFollowDecision['reason'] };
    }).sort((a, b) => b.score - a.score || a.player.objectId - b.player.objectId);
    let chosen = scored.find(({ player }) => player.objectId === this.selectedId);
    let reason = chosen?.reason ?? 'idle';
    const requested = frame.target?.trim() || null;
    if (requested !== this.manualTarget && this.selectedId !== null) this.selectedAt = at;
    let mode: SpectatorFollowDecision['mode'] = 'automatic';
    if (frame.director || frame.camera) {
      // Privileged camera/director behavior stays with the server; no token or
      // zero-delay/control request is generated by this ordinary auto-follow.
      return { world, decision: { source: 'delayedSnapshot', mode: 'serverCamera',
        objectId: world.playerObjectId, name: null, score: 0, reason: 'serverCamera', heldForMs: 0 } };
    }
    const manual = requested
      ? scored.find(({ player }) => player.name.toLowerCase() === requested.toLowerCase()) : null;
    if (manual) {
      chosen = manual;
      reason = 'manual';
      mode = 'manual';
    } else {
      if (requested) mode = 'manualUnavailable';
      const best = scored[0];
      if (!chosen) {
        chosen = best;
        reason = chosen?.reason ?? 'unavailable';
      } else if (best && best.player.objectId !== chosen.player.objectId
          && at - this.selectedAt >= MIN_HOLD_MS
          && best.score >= chosen.score + 30 && best.score >= chosen.score * 1.35) {
        chosen = best;
        reason = best.reason;
      } else if (best && best.player.objectId !== chosen.player.objectId && best.score > chosen.score) {
        reason = 'holding';
      }
    }
    if (chosen?.player.objectId !== this.selectedId || requested !== this.manualTarget) this.selectedAt = at;
    this.selectedId = chosen?.player.objectId ?? null;
    this.manualTarget = requested;
    const decision: SpectatorFollowDecision = { source: 'delayedSnapshot', mode,
      objectId: this.selectedId, name: chosen?.player.name ?? null,
      score: Math.round(chosen?.score ?? 0), reason,
      heldForMs: Math.max(0, at - this.selectedAt) };
    if (!chosen) {
      return { world: { ...world, playerObjectId: null, playerHp: null, playerMaxHp: null,
        entities: world.entities.map((entity) => entity.kind === 'selfPlayer'
          ? { ...entity, kind: 'player' } : entity) }, decision };
    }
    const player = chosen.player;
    return { world: { ...world, playerObjectId: player.objectId,
      playerHp: player.hp ?? null, playerMaxHp: player.maxHp ?? null,
      entities: world.entities.map((entity) => entity.objectId === player.objectId
        ? { ...entity, kind: 'selfPlayer' }
        : entity.kind === 'selfPlayer' ? { ...entity, kind: 'player' } : entity) }, decision };
  }

  private addHeat(id: number, score: number, at: number, reason: Heat['reason']) {
    const previous = this.heat.get(id);
    this.heat.set(id, { score: Math.min(500, (previous?.score ?? 0) + score),
      lastActivityAt: at, reason });
  }

  private nearbyHeat(players: Entity[], monster: Entity, score: number, at: number) {
    for (const player of players) {
      const gap = distance(player, monster);
      if (gap <= 8) this.addHeat(player.objectId, score * (9 - gap) / 9, at, 'nearbyCombat');
    }
  }
}

/** Gateway sends each world immediately followed by metadata for that frame.
 * Control acknowledgements contain metadata alone: never score those against
 * an unrelated cached world, or render the default player before its metadata.
 */
export class SpectatorFollowStream<T extends World> {
  private pending: T | null = null;
  private paired: { key: string; world: T } | null = null;
  private selector = new SpectatorAutoFollow();

  reset() {
    this.pending = null;
    this.paired = null;
    this.selector.reset();
  }

  receiveWorld(world: T) { this.pending = world; }

  receiveStatus(status: SpectatorFollowFrame): DirectedFrame<T> | null {
    const key = frameKey(status);
    if (!key) { this.reset(); return null; }
    if (this.pending) {
      const world = this.pending;
      this.pending = null;
      if (world.mapFileName !== status.map) { this.reset(); return null; }
      this.paired = { key, world };
    }
    if (this.paired?.key !== key) { this.paired = null; return null; }
    return this.selector.direct(this.paired.world, status);
  }
}
