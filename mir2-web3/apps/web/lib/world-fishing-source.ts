/** Typed world evidence only. This model has no clock, cast decision or transport. */
export type WorldFishingDirection = "Up" | "UpRight" | "Right" | "DownRight"
  | "Down" | "DownLeft" | "Left" | "UpLeft";
export type WorldFishingOwner = Readonly<{
  socket: object;
  connectionGeneration: number;
  sessionGeneration: number;
  sceneRevision: number;
  playerObjectId: number;
  mapFileName: string;
}>;
export type WorldFishingSelf = Readonly<{
  x: number; y: number; direction: WorldFishingDirection; dead: boolean;
}>;
export type WorldFishingActionFamily = "standing" | "walking" | "running" | "attackMelee" | "attackRange"
  | "spell" | "struck" | "dying" | "reviving";
export type WorldFishingActionEvidence = Readonly<{
  family: WorldFishingActionFamily; id: number; token: string; receivedAtMs: number | null;
}>;
export type WorldFishingSourceRecord = Readonly<{
  owner: WorldFishingOwner;
  sourceKey: string;
  revision: number;
  continuityRevision: number;
  animationKnown: boolean;
  bootstrapKnown: boolean;
  actionEvidence: WorldFishingActionEvidence | null;
  self: WorldFishingSelf | null;
  fishing: boolean | null;
  transformType: number | null;
}>;

type Facts = Pick<WorldFishingSourceRecord, "self" | "fishing" | "transformType" | "animationKnown" | "bootstrapKnown" | "actionEvidence">;
const MAX_U32 = 4_294_967_295;
// Module-owned epochs avoid reusing a source key across different model instances.
let nextSourceEpoch = 0;

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
function integer(value: unknown, minimum: number, maximum: number): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= minimum && value <= maximum;
}
function direction(value: unknown): value is WorldFishingDirection {
  return value === "Up" || value === "UpRight" || value === "Right" || value === "DownRight"
    || value === "Down" || value === "DownLeft" || value === "Left" || value === "UpLeft";
}
function normalizeOwner(value: unknown): WorldFishingOwner | null {
  if (!record(value)) return null;
  const socket = value.socket, connectionGeneration = value.connectionGeneration;
  const sessionGeneration = value.sessionGeneration, sceneRevision = value.sceneRevision;
  const playerObjectId = value.playerObjectId, mapFileName = value.mapFileName;
  if (!socket || typeof socket !== "object" || !integer(connectionGeneration, 1, Number.MAX_SAFE_INTEGER)
    || !integer(sessionGeneration, 1, Number.MAX_SAFE_INTEGER) || !integer(sceneRevision, 1, Number.MAX_SAFE_INTEGER)
    || !integer(playerObjectId, 1, MAX_U32) || typeof mapFileName !== "string"
    || mapFileName.length === 0 || mapFileName.length > 512) return null;
  // Preserve physical socket identity; never freeze the socket itself.
  return Object.freeze({ socket, connectionGeneration, sessionGeneration, sceneRevision, playerObjectId, mapFileName });
}
export function sameOwner(left: WorldFishingOwner, right: WorldFishingOwner): boolean {
  const a = normalizeOwner(left), b = normalizeOwner(right);
  return !!a && !!b && a.socket === b.socket && a.connectionGeneration === b.connectionGeneration
    && a.sessionGeneration === b.sessionGeneration && a.sceneRevision === b.sceneRevision
    && a.playerObjectId === b.playerObjectId && a.mapFileName === b.mapFileName;
}
function typedSelf(value: unknown): WorldFishingSelf | null {
  if (!record(value)) return null;
  const x = value.x, y = value.y, facing = value.direction, dead = value.dead;
  if (!integer(x, -2147483648, 2147483647) || !integer(y, -2147483648, 2147483647)
    || !direction(facing) || typeof dead !== "boolean") return null;
  return Object.freeze({ x, y, direction: facing, dead });
}
function fishingFact(value: unknown): boolean | null { return typeof value === "boolean" ? value : null; }
function transformFact(value: unknown): number | null { return integer(value, -32768, 32767) ? value : null; }
function invalidOptionalFacts(value: Record<string, unknown>): boolean {
  const fishing = value.fishing, transformType = value.transformType;
  return (fishing !== null && fishing !== undefined && typeof fishing !== "boolean")
    || (transformType !== null && transformType !== undefined && !integer(transformType, -32768, 32767));
}
function ownPayload(value: unknown, owner: WorldFishingOwner, selfOnly: boolean): Record<string, unknown> | null {
  if (!record(value)) return null;
  if (!Object.hasOwn(value, "objectId")) return selfOnly ? value : null;
  return value.objectId === owner.playerObjectId ? value : null;
}
function actionPoint(payload: Record<string, unknown>): Readonly<{ x: number; y: number }> | null {
  const hasX = Object.hasOwn(payload, "x"), hasY = Object.hasOwn(payload, "y");
  const location = payload.location;
  const hasLocation = Object.hasOwn(payload, "location");
  if (hasX || hasY) {
    const x = payload.x, y = payload.y;
    if (!hasX || !hasY || !integer(x, -2147483648, 2147483647) || !integer(y, -2147483648, 2147483647)) return null;
    if (hasLocation && (!record(location) || location.x !== x || location.y !== y)) return null;
    return Object.freeze({ x, y });
  }
  if (!hasLocation || !record(location)) return null;
  const x = location.x, y = location.y;
  return integer(x, -2147483648, 2147483647) && integer(y, -2147483648, 2147483647)
    ? Object.freeze({ x, y }) : null;
}

export class WorldFishingSource {
  private held: WorldFishingSourceRecord | null = null;
  private revisionClock = 0;
  private continuityClock = 0;
  private exhausted = false;
  private actionClock = 0;
  // Unknown initial data may bootstrap once; a retired known interval never can.
  private bootstrapEligible = true;

  private commit(facts: Facts, newOwner?: WorldFishingOwner, sourceKey?: string): WorldFishingSourceRecord | null {
    if (this.exhausted || this.revisionClock >= Number.MAX_SAFE_INTEGER) {
      this.exhausted = true; this.held = null; return null;
    }
    const previous = this.held, owner = newOwner ?? previous?.owner, key = sourceKey ?? previous?.sourceKey;
    if (!owner || !key || key.length > 512) return null;
    this.revisionClock += 1;
    this.held = Object.freeze({ owner, sourceKey: key, revision: this.revisionClock,
      continuityRevision: this.continuityClock, self: facts.self, fishing: facts.fishing, transformType: facts.transformType,
      animationKnown: facts.animationKnown, bootstrapKnown: facts.bootstrapKnown, actionEvidence: facts.actionEvidence });
    return this.held;
  }
  private advanceContinuity(): boolean {
    if (this.exhausted || this.continuityClock >= Number.MAX_SAFE_INTEGER) {
      this.exhausted = true; this.held = null; return false;
    }
    this.continuityClock += 1;
    return true;
  }
  private withdraw(changes: Partial<Pick<Facts, "self" | "fishing" | "transformType">> = {}): WorldFishingSourceRecord | null {
    const previous = this.held;
    if (!previous || !this.advanceContinuity()) return null;
    return this.commit({ self: previous.self, fishing: previous.fishing, transformType: previous.transformType,
      ...changes, animationKnown: false, bootstrapKnown: false, actionEvidence: null });
  }

  observeSnapshot(owner: WorldFishingOwner, raw: unknown): WorldFishingSourceRecord | null {
    let acceptedOwner = false;
    try {
      const normalized = normalizeOwner(owner);
      if (!normalized || this.exhausted) return null;
      acceptedOwner = true;
      const fresh = !this.held || !sameOwner(this.held.owner, normalized);
      if (fresh) {
        this.bootstrapEligible = true;
        if (nextSourceEpoch >= Number.MAX_SAFE_INTEGER || !this.advanceContinuity()) {
          this.exhausted = true; this.held = null; return null;
        }
        nextSourceEpoch += 1;
        if (!this.commit({ self: null, fishing: null, transformType: null, animationKnown: false, bootstrapKnown: false, actionEvidence: null },
          normalized, "world-fishing-source:" + nextSourceEpoch)) return null;
      }
      if (!record(raw) || raw.playerObjectId !== normalized.playerObjectId || raw.mapFileName !== normalized.mapFileName
        || !Array.isArray(raw.entities)) return this.withdraw({ self: null, fishing: null, transformType: null });
      let own: Record<string, unknown> | null = null;
      for (const entity of raw.entities) {
        if (!record(entity) || entity.objectId !== normalized.playerObjectId) continue;
        if (own) return this.withdraw({ self: null, fishing: null, transformType: null });
        own = entity;
      }
      const self = own?.kind === "selfPlayer" ? typedSelf(own) : null;
      if (!own || !self) return this.withdraw({ self: null, fishing: null, transformType: null });
      const previous = this.held;
      if (!previous) return null;
      if (invalidOptionalFacts(own)) return this.withdraw({ self, fishing: fishingFact(own.fishing), transformType: transformFact(own.transformType) });
      const firstKnownSnapshot = this.bootstrapEligible;
      this.bootstrapEligible = false;
      const fishing = fishingFact(own.fishing), transformType = transformFact(own.transformType);
      if (!firstKnownSnapshot && previous.self && previous.self.x === self.x && previous.self.y === self.y
        && previous.self.direction === self.direction && previous.self.dead === self.dead
        && previous.fishing === fishing && previous.transformType === transformType) return previous;
      return this.commit({ self, fishing, transformType,
        animationKnown: firstKnownSnapshot || previous.animationKnown, bootstrapKnown: firstKnownSnapshot || previous.bootstrapKnown,
        actionEvidence: previous.actionEvidence });
    } catch { return acceptedOwner ? this.withdraw({ self: null, fishing: null, transformType: null }) : null; }
  }

  observePacket(owner: WorldFishingOwner, name: string, payload: unknown, receivedAtMs?: number): void {
    let matchedOwner = false;
    try {
      const normalized = normalizeOwner(owner), previous = this.held;
      if (!normalized || !previous || this.exhausted || !sameOwner(previous.owner, normalized)) return;
      matchedOwner = true;
      if (name === "MapChanged" || name === "Disconnect") { this.retire(); return; }
      const own = ownPayload(payload, normalized, name === "UserLocation");
      if (!own) return;
      if (name === "ObjectRemove" || name === "ObjectHide" || name === "Hide") { this.retire(); return; }
      if (name === "FishingUpdate") {
        const fishing = fishingFact(own.fishing);
        if (fishing === null) this.withdraw({ fishing: null });
        else this.commit({ ...previous, fishing });
        return;
      }
      if (name === "TransformUpdate") {
        const transformType = transformFact(own.transformType);
        if (transformType === null) this.withdraw({ transformType: null });
        else this.commit({ ...previous, transformType });
        return;
      }
      // These packets carry no canonical complete local action event here.
      if (name === "ObjectHarvest" || name === "ObjectHarvested" || name === "Harvest") { this.withdraw(); return; }
      if (name === "ObjectPlayer") {
        const self = typedSelf(own);
        if (!self) this.withdraw({ self: null, fishing: null, transformType: null });
        else if (invalidOptionalFacts(own)) this.withdraw({ self, fishing: fishingFact(own.fishing), transformType: transformFact(own.transformType) });
        else this.commit({ ...previous, self, fishing: fishingFact(own.fishing), transformType: transformFact(own.transformType) });
        return;
      }
      if (name !== "ObjectWalk" && name !== "ObjectRun" && name !== "ObjectTurn" && name !== "ObjectAttack"
        && name !== "ObjectRangeAttack" && name !== "ObjectMagic" && name !== "ObjectStruck" && name !== "ObjectDied"
        && name !== "ObjectRevive" && name !== "ObjectRevived" && name !== "UserLocation") return;
      const point = actionPoint(own), facing = own.direction;
      const dead = name === "ObjectDied" ? true : name === "ObjectRevive" || name === "ObjectRevived" ? false : previous.self?.dead;
      if (!point || !direction(facing) || typeof dead !== "boolean") {
        const self = previous.self && (name === "ObjectDied" || name === "ObjectRevive" || name === "ObjectRevived")
          ? Object.freeze({ ...previous.self, dead: name === "ObjectDied" }) : null;
        this.withdraw({ self }); return;
      }
      const self = Object.freeze({ x: point.x, y: point.y, direction: facing, dead });
      // UserLocation is a transform correction/ACK, not a local animation event.
      if (name === "UserLocation") { this.commit({ ...previous, self }); return; }
      if (this.actionClock >= Number.MAX_SAFE_INTEGER) { this.exhausted = true; this.held = null; return; }
      if (!previous.animationKnown && !this.advanceContinuity()) return;
      this.actionClock += 1;
      const family: WorldFishingActionFamily = name === "ObjectWalk" ? "walking" : name === "ObjectRun" ? "running"
        : name === "ObjectTurn" ? "standing" : name === "ObjectAttack" ? "attackMelee"
        : name === "ObjectRangeAttack" ? "attackRange" : name === "ObjectMagic" ? "spell"
        : name === "ObjectStruck" ? "struck" : name === "ObjectDied" ? "dying" : "reviving";
      const actionEvidence = Object.freeze({ family, id: this.actionClock,
        token: previous.sourceKey + ":action:" + this.actionClock,
        receivedAtMs: integer(receivedAtMs, 0, Number.MAX_SAFE_INTEGER) ? receivedAtMs : null });
      this.bootstrapEligible = false;
      this.commit({ self, fishing: previous.fishing, transformType: previous.transformType,
        animationKnown: true, bootstrapKnown: previous.bootstrapKnown, actionEvidence });
    } catch {
      // Only a recognized, matching owner can retire its own evidence.
      if (matchedOwner) this.withdraw({ self: null, fishing: null, transformType: null });
    }
  }

  retire(): void { this.withdraw({ self: null, fishing: null, transformType: null }); }
  current(owner: WorldFishingOwner): WorldFishingSourceRecord | null {
    try { return this.held && !this.exhausted && sameOwner(this.held.owner, owner) ? this.held : null; }
    catch { return null; }
  }
}
