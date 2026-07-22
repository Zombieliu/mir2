import type {
  CrystalEntityAnimationAction,
  CrystalEntityAnimationPose,
  DisplayEntity,
  EntityMotionSnapshot,
  EntitySpriteAnimationState,
} from "./original-client-types";

type EntityAnimationRuntime = {
  resolveMir2EntityAnimationPoses?: (snapshotJson: string) => string;
  resetMir2EntityAnimations?: () => void;
};

type ResolveEntityAnimationInput = {
  runtime: EntityAnimationRuntime | null | undefined;
  worldKey: string;
  worldSeed: number;
  now: number;
  entities: Array<{
    entity: DisplayEntity;
    state: EntitySpriteAnimationState;
    motionSnapshot?: EntityMotionSnapshot;
  }>;
};

type RuntimeAnimationOutput = {
  worldKey?: unknown;
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
  entities,
}: ResolveEntityAnimationInput): Record<string, CrystalEntityAnimationPose> {
  if (!runtime?.resolveMir2EntityAnimationPoses) {
    return {};
  }

  const payload = {
    worldKey,
    worldSeed: Math.max(0, Math.trunc(worldSeed)) >>> 0,
    nowMs: Math.max(0, Math.trunc(now)),
    entities: entities.map(({ entity, state, motionSnapshot }) => ({
      objectId: entity.objectId,
      kind: entity.kind,
      direction: entity.direction,
      ...animationEventForEntity(entity, state, motionSnapshot),
    })),
  };

  try {
    const decoded = JSON.parse(runtime.resolveMir2EntityAnimationPoses(JSON.stringify(payload))) as RuntimeAnimationOutput;
    if (decoded.worldKey !== worldKey || !Array.isArray(decoded.poses)) {
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
