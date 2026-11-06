import type {
  CrystalEntityAnimationAction,
  CrystalEntityAnimationPose,
  DisplayEntity,
  EntityMotionSnapshot,
  EntitySpriteAnimationState,
} from "./original-client-types";
import type { WorldFishingActionEvidence, WorldFishingActionFamily, WorldFishingOwner, WorldFishingSourceRecord } from "../../lib/world-fishing-source";

type EntityAnimationRuntime = {
  getMir2EntityActionPose?: (queryJson: string) => string;
  resolveMir2EntityAnimationPoses?: (snapshotJson: string) => string;
  resetMir2EntityAnimations?: () => void;
};

type ResolveEntityAnimationInput = {
  runtime: EntityAnimationRuntime | null | undefined;
  worldKey: string;
  worldSeed: number;
  now: number;
  selfAuthority?: import("../../lib/world-fishing-source").WorldFishingSourceRecord | null;
  entities: Array<{
    entity: DisplayEntity;
    state: EntitySpriteAnimationState;
    motionSnapshot?: EntityMotionSnapshot;
  }>;
};

type RuntimeAnimationOutput = {
  worldKey?: unknown;
  nowMs?: unknown;
  poses?: unknown;
};

const ANIMATION_STATES = new Set<EntitySpriteAnimationState>([
  "standing",
  "harvesting",
  "walking",
  "running",
  "attackMelee",
  "attackRange",
  "struck",
  "dying",
  "dead",
  "reviving",
]);

const ANIMATION_ACTIONS = new Set<CrystalEntityAnimationAction>([
  "standing",
  "harvest",
  "walking",
  "running",
  "attack1",
  "attack2",
  "attack3",
  "attack4",
  "attackRange1",
  "spell",
  "struck",
  "die",
  "dead",
  "revive",
]);

export function resolveCrystalEntityAnimationPoses({
  runtime,
  worldKey,
  worldSeed,
  now,
  selfAuthority,
  entities,
}: ResolveEntityAnimationInput): Record<string, CrystalEntityAnimationPose> {
  if (!runtime?.resolveMir2EntityAnimationPoses) {
    return {};
  }

  const payload = {
    worldKey,
    worldSeed: Math.max(0, Math.trunc(worldSeed)) >>> 0,
    nowMs: Math.max(0, Math.trunc(now)),
    entities: entities.map(({ entity, state, motionSnapshot }) => {
      const own = selfAuthority && entity.objectId === String(selfAuthority.owner.playerObjectId);
      const event = own ? selfAnimationEvent(selfAuthority, entity, state, motionSnapshot)
        : { ...animationEventForEntity(entity, state, motionSnapshot), actionKnown: false };
      return {
        objectId: entity.objectId, kind: entity.kind, direction: entity.direction,
        action: event.action, ...(event.actionToken === undefined ? {} : { actionToken: event.actionToken }),
        ...(own ? { actionSourceKey: selfAuthority.sourceKey, actionSourceRevision: selfAuthority.revision,
          bootstrapKnown: selfAuthority.bootstrapKnown && selfAuthority.actionEvidence === null,
          actionKnown: event.actionKnown } : {}),
      };
    }),
  };

  try {
    const decoded = JSON.parse(runtime.resolveMir2EntityAnimationPoses(JSON.stringify(payload))) as RuntimeAnimationOutput;
    if (decoded.worldKey !== worldKey || decoded.nowMs !== payload.nowMs || !Array.isArray(decoded.poses)) {
      return {};
    }

    const poses: Record<string, CrystalEntityAnimationPose> = {};
    for (const candidate of decoded.poses) {
      if (!isRuntimeAnimationPose(candidate)) {
        continue;
      }
      poses[candidate.objectId] = candidate;
    }
    return poses;
  } catch {
    // Runtime load/fallback transitions are allowed to retain the legacy JS
    // selector for one render; the next 100 ms tick retries the WASM owner.
    return {};
  }
}

export function animationEventForEntity(
  entity: DisplayEntity,
  state: EntitySpriteAnimationState,
  motionSnapshot?: EntityMotionSnapshot,
): { action: CrystalEntityAnimationAction; actionToken?: string } {
  switch (state) {
    case "walking":
    case "running": {
      const startedAt = entity.movementStartedAt ?? motionSnapshot?.startedAt;
      return {
        action: state,
        ...(startedAt === undefined ? {} : { actionToken: `move:${startedAt}:${state}` }),
      };
    }
    case "attackMelee": {
      const action = meleeAction(entity.attackAnimation);
      return {
        action,
        ...(entity.attackStartedAt === undefined
          ? {}
          : { actionToken: `attack:${entity.attackStartedAt}:${action}` }),
      };
    }
    case "attackRange": {
      const action = entity.attackAnimation === "spell" ? "spell" : "attackRange1";
      return {
        action,
        ...(entity.attackStartedAt === undefined
          ? {}
          : { actionToken: `attack:${entity.attackStartedAt}:${action}` }),
      };
    }
    case "struck":
      return {
        action: "struck",
        ...(entity.struckStartedAt === undefined
          ? {}
          : { actionToken: `struck:${entity.struckStartedAt}` }),
      };
    case "dying":
      return {
        action: "die",
        actionToken: `life:${entity.dieStartedAt ?? "dying"}`,
      };
    case "dead":
      return {
        action: "dead",
        actionToken: `life:${entity.dieStartedAt ?? "dead"}`,
      };
    case "reviving":
      return {
        action: "revive",
        actionToken: `revive:${entity.reviveStartedAt ?? "reviving"}`,
      };
    default:
      return { action: "standing" };
  }
}

export function entityAnimationRuntimeFromWindow(): EntityAnimationRuntime | null {
  if (typeof window === "undefined") {
    return null;
  }
  return (window as typeof window & { __mir2BevyRuntime?: EntityAnimationRuntime })
    .__mir2BevyRuntime ?? null;
}

export function createCrystalAnimationWorldSeed(): number {
  if (typeof window !== "undefined") {
    const requested = Number.parseInt(
      new URLSearchParams(window.location.search).get("crystalAnimationSeed") ?? "",
      10,
    );
    if (Number.isFinite(requested) && requested >= 0) {
      return requested >>> 0;
    }
    if (globalThis.crypto?.getRandomValues) {
      return globalThis.crypto.getRandomValues(new Uint32Array(1))[0] ?? 0;
    }
  }
  return Math.floor(Math.random() * 0x1_0000_0000) >>> 0;
}

function meleeAction(attackAnimation: DisplayEntity["attackAnimation"]): CrystalEntityAnimationAction {
  switch (attackAnimation) {
    case "melee2":
      return "attack2";
    case "melee3":
      return "attack3";
    case "melee4":
      return "attack4";
    default:
      return "attack1";
  }
}

function isRuntimeAnimationPose(value: unknown): value is CrystalEntityAnimationPose {
  if (!value || typeof value !== "object") {
    return false;
  }
  const pose = value as Partial<CrystalEntityAnimationPose>;
  return (
    typeof pose.objectId === "string" &&
    typeof pose.incarnation === "number" &&
    typeof pose.animationState === "string" &&
    ANIMATION_STATES.has(pose.animationState as EntitySpriteAnimationState) &&
    typeof pose.action === "string" &&
    ANIMATION_ACTIONS.has(pose.action as CrystalEntityAnimationAction) &&
    typeof pose.direction === "string" &&
    typeof pose.logicalFrameIndex === "number" &&
    typeof pose.queueDepth === "number"
  );
}

type DeliveredSelfEvent = Readonly<{ action: CrystalEntityAnimationAction; actionToken: string }>;
const deliveredSelfEvents = new WeakMap<WorldFishingActionEvidence, DeliveredSelfEvent>();
const lastDeliveredDisplayTime = new WeakMap<WorldFishingOwner, number>();
const CANONICAL_DIRECTIONS = new Set(["Up", "UpRight", "Right", "DownRight", "Down", "DownLeft", "Left", "UpLeft"]);
function eventTime(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

// Only actual display/motion metadata identifies a transient event. No selector
// default, frame duration or action label is used to reconstruct a missing one.
function actualSelfEvent(entity: DisplayEntity, state: EntitySpriteAnimationState,
  motion?: EntityMotionSnapshot): { family: WorldFishingActionFamily; time: number; event: DeliveredSelfEvent } | null {
  let family: WorldFishingActionFamily, time: unknown;
  if (state === "walking" || state === "running") {
    family = state;
    time = entity.movementAnimation === state && eventTime(entity.movementStartedAt) ? entity.movementStartedAt
      : motion?.animationState === state ? motion.startedAt : undefined;
  } else if (state === "attackMelee" && ["melee1", "melee2", "melee3", "melee4"].includes(entity.attackAnimation ?? "")) {
    family = "attackMelee"; time = entity.attackStartedAt;
  } else if (state === "attackRange" && (entity.attackAnimation === "range" || entity.attackAnimation === "spell")) {
    family = entity.attackAnimation === "spell" ? "spell" : "attackRange"; time = entity.attackStartedAt;
  } else if (state === "struck") { family = "struck"; time = entity.struckStartedAt;
  } else if (state === "dying" || state === "dead") { family = "dying"; time = entity.dieStartedAt;
  } else if (state === "reviving") { family = "reviving"; time = entity.reviveStartedAt;
  } else return null;
  if (!eventTime(time)) return null;
  const event = family === "walking" || family === "running" ? { action: family, actionToken: "move:" + time + ":" + family }
    : animationEventForEntity(entity, state, motion);
  // Movement's actual canonical timestamp may come from motion while an older
  // entity field still exists. Bind the selected metadata, not that stale field.
  const actionToken = family === "walking" || family === "running" ? "move:" + time + ":" + family : event.actionToken;
  if (typeof actionToken !== "string" || !actionToken) return null;
  return { family, time, event: Object.freeze({ action: event.action, actionToken }) };
}

function selfAnimationEvent(source: WorldFishingSourceRecord, entity: DisplayEntity,
  state: EntitySpriteAnimationState, motion?: EntityMotionSnapshot):
  { action: CrystalEntityAnimationAction; actionToken?: string; actionKnown: boolean } {
  const fallback = animationEventForEntity(entity, state, motion);
  if (!source.animationKnown || !source.self || entity.kind !== "selfPlayer"
    || entity.x !== source.self.x || entity.y !== source.self.y || entity.direction !== source.self.direction
    || !CANONICAL_DIRECTIONS.has(entity.direction ?? "")) return { ...fallback, actionKnown: false };
  const evidence = source.actionEvidence;
  if (!evidence) {
    // A complete source bootstrap can use real transient metadata; an unknown
    // interval cannot be healed by the selector's default Standing.
    if (!source.bootstrapKnown) return { ...fallback, actionKnown: false };
    if (state === "standing") return { ...fallback, actionKnown: true };
    const actual = actualSelfEvent(entity, state, motion);
    if (!actual) return { ...fallback, actionKnown: false };
    const previousTime = lastDeliveredDisplayTime.get(source.owner);
    if (previousTime === undefined || actual.time > previousTime) lastDeliveredDisplayTime.set(source.owner, actual.time);
    return { ...actual.event, actionKnown: true };
  }
  const cached = deliveredSelfEvents.get(evidence);
  if (cached) return { ...cached, actionKnown: true };
  if (evidence.family === "standing") {
    const delivered = Object.freeze({ action: "standing" as const, actionToken: evidence.token });
    deliveredSelfEvents.set(evidence, delivered);
    return { ...delivered, actionKnown: true };
  }
  const actual = actualSelfEvent(entity, state, motion), lastTime = lastDeliveredDisplayTime.get(source.owner);
  if (!eventTime(evidence.receivedAtMs) || !actual || actual.family !== evidence.family
    || actual.time < evidence.receivedAtMs || (lastTime !== undefined && actual.time <= lastTime))
    return { ...fallback, actionKnown: false };
  const delivered = Object.freeze({ action: actual.event.action, actionToken: evidence.token + ":" + actual.event.actionToken });
  deliveredSelfEvents.set(evidence, delivered);
  lastDeliveredDisplayTime.set(source.owner, actual.time);
  // Subsequent default Standing reuses this consumed event; Rust owns its end.
  return { ...delivered, actionKnown: true };
}

export type CrystalEntityActionPose = Readonly<{ worldKey: string; objectId: string; actionSourceKey: string;
  sourceRevision: number; continuityRevision: number; bridgeEpoch: number; incarnation: number;
  lastNowMs: number; action: string; direction: string }>;
export function readCrystalEntityActionPose(runtime: EntityAnimationRuntime,
  commit: import("../../lib/world-fishing-input").WorldFishingAnimationCommit,
  source: WorldFishingSourceRecord,
  nowMs: number): CrystalEntityActionPose | null {
  try {
    const context = commit.context, committedSource = context.record, owner = source.owner;
    const getter = runtime.getMir2EntityActionPose, contextRuntime = context.runtime;
    const worldKey = commit.worldKey, worldSeed = commit.worldSeed, atMs = commit.atMs, token = commit.token;
    const sourceKey = source.sourceKey, sourceRevision = source.revision, objectId = owner.playerObjectId;
    if (typeof getter !== "function" || !eventTime(nowMs) || !eventTime(atMs) || atMs > nowMs || nowMs - atMs > 250
      || !eventTime(worldSeed) || typeof worldKey !== "string" || !worldKey || worldKey.length > 512
      || contextRuntime !== runtime || committedSource !== source || typeof sourceKey !== "string" || !sourceKey || sourceKey.length > 512
      || !eventTime(sourceRevision) || sourceRevision === 0 || !eventTime(objectId) || objectId === 0 || objectId > 4_294_967_295) return null;
    const current = () => commit.context === context && context.runtime === contextRuntime && contextRuntime === runtime
      && context.record === committedSource && committedSource === source && source.owner === owner && owner.playerObjectId === objectId
      && commit.worldKey === worldKey && commit.worldSeed === worldSeed && commit.atMs === atMs && commit.token === token
      && source.sourceKey === sourceKey && source.revision === sourceRevision && runtime.getMir2EntityActionPose === getter;
    const query = JSON.stringify({ version: 1, worldKey, worldSeed, objectId: String(objectId), actionSourceKey: sourceKey });
    if (new TextEncoder().encode(query).byteLength > 2048 || !current()) return null;
    const raw = getter.call(runtime, query);
    if (!current() || typeof raw !== "string" || raw.length > 2048 || new TextEncoder().encode(raw).byteLength > 2048) return null;
    const d: unknown = JSON.parse(raw);
    if (!d || typeof d !== "object" || Array.isArray(d)) return null;
    const value = d as Record<string, unknown>, keys = ["version","known","worldKey","objectId","actionSourceKey",
      "sourceRevision","continuityRevision","bridgeEpoch","incarnation","lastNowMs","action","direction"];
    if (Object.keys(value).length !== keys.length || !keys.every(key=>Object.hasOwn(value,key))
      || value.version !== 1 || value.known !== true || value.worldKey !== worldKey
      || value.objectId !== String(objectId) || value.actionSourceKey !== sourceKey
      || value.sourceRevision !== sourceRevision || !["sourceRevision","continuityRevision","bridgeEpoch","incarnation"].every(
        key=>typeof value[key] === "number" && Number.isSafeInteger(value[key]) && (value[key] as number)>0)
      || value.lastNowMs !== atMs
      || typeof value.action !== "string" || !["standing","harvest","show","hide","walking","running","attack1","attack2",
        "attack3","attack4","attackRange1","attackRange2","dashAttack","spell","struck","die","dead","skeleton","revive"].includes(value.action)
      || typeof value.direction !== "string" || !CANONICAL_DIRECTIONS.has(value.direction) || !current()) return null;
    const {version: _version, known: _known, ...pose} = value;
    return Object.freeze(pose) as CrystalEntityActionPose;
  } catch { return null; }
}
