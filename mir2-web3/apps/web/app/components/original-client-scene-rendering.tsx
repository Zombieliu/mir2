"use client";

import { ORIGINAL_UI, type ClientScreen } from "../../lib/original-ui";
import {
  frameMetaForIndex,
  normalizeSceneSpriteLibraryKey,
  type OriginalSceneSpriteFrameMeta,
  type OriginalSceneSpriteLibraryMeta,
} from "../../lib/original-scene-sprite-meta";
import { SELECT_PORTRAIT_ANIMATIONS, type SelectPortraitKey } from "../../lib/select-portraits";
import type { OriginalMapRegion, OriginalMapSpriteFrame } from "../../lib/scene-types";
import { transientFrameCycle } from "./original-client-scene-motion";
export {
  GameSceneBackdrop,
  buildViewportMapSprites,
  handleSceneAssetImageError,
  handleSceneAssetImageLoad,
  mapSpriteBlendMode,
  mapSpriteRenderPath,
  rescueStalledSceneAssetImages,
  sceneAssetCandidateUrls,
  sceneAssetRuntimeStats,
} from "./original-client-scene-map-rendering";
export {
  cameraMotionOffsetForEntity,
  entityAnimationStateForEntity,
  entityMotionOffsetForEntity,
  isEntityAttacking,
  isEntityReviving,
  isEntityStruck,
  projectileProgress,
  refreshEntityMotionSnapshots,
} from "./original-client-scene-motion";
import {
  CRYSTAL_MOVE_FRAME_COUNT,
  CRYSTAL_MOVE_FRAME_INTERVAL_MS,
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  VIEWPORT_ENTITY_LEFT_ORIGIN,
  VIEWPORT_ENTITY_TOP_ORIGIN,
  VIEWPORT_RANGE_X,
  VIEWPORT_RANGE_Y,
  VIEWPORT_TILE_LEFT_ORIGIN,
  VIEWPORT_TILE_TOP_ORIGIN,
  argbToCssColor,
  viewportDepthForCell,
  type SceneBackdropTile,
  type ViewportMapSprite,
  type ViewportMapSprites,
  type ViewportOffset,
} from "./original-client-scene-layout";
export {
  CRYSTAL_MOVE_INPUT_INTERVAL_MS,
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  MAX_PREDICTED_PLAYER_LEAD_TILES,
  VIEWPORT_CELL_HEIGHT,
  VIEWPORT_CELL_WIDTH,
  VIEWPORT_ENTITY_LEFT_ORIGIN,
  VIEWPORT_ENTITY_TOP_ORIGIN,
  VIEWPORT_MOUSE_TILE_CENTER_X,
  VIEWPORT_MOUSE_TILE_CENTER_Y,
  VIEWPORT_OFFSET_X,
  VIEWPORT_OFFSET_Y,
  VIEWPORT_RANGE_X,
  VIEWPORT_RANGE_Y,
  VIEWPORT_TILE_CENTER_X,
  VIEWPORT_TILE_CENTER_Y,
  VIEWPORT_TILE_LEFT_ORIGIN,
  VIEWPORT_TILE_TOP_ORIGIN,
  argbToCssColor,
  ratio,
  viewportDepthForCell,
  type SceneBackdropTile,
  type ViewportMapSprite,
  type ViewportMapSprites,
  type ViewportOffset,
} from "./original-client-scene-layout";
import type {
  DisplayEntity,
  DisplayProjectile,
  DisplayQuest,
  DisplayWorld,
  EntityClassKey,
  EntityGenderKey,
  EntityMotionSnapshot,
  EntitySprite,
  EntitySpriteAnimationState,
  SelectCharacterEntry,
} from "./original-client-types";

type ViewportSpriteLayer = Pick<
  OriginalSceneSpriteFrameMeta,
  "path" | "width" | "height" | "x" | "y"
>;

export type ViewportEntitySprite = {
  mount: ViewportSpriteLayer | null;
  rearWeapons: ViewportSpriteLayer[];
  body: ViewportSpriteLayer | null;
  hair: ViewportSpriteLayer | null;
  frontWeapons: ViewportSpriteLayer[];
  preloadFrames: ViewportSpriteLayer[];
  preloadPaths: string[];
  nameplateTop: number;
};

type ViewportEntityBounds = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};

type QuestIconKey =
  | "questionWhite"
  | "exclamationYellow"
  | "questionYellow"
  | "exclamationGreen"
  | "questionGreen";

const SCENE_SPRITE_FRAME_INTERVAL_MS = 120;
const CRYSTAL_QUEST_ICON_FRAME_INTERVAL_MS = 500;
const PLAYER_ATLAS_PRELOAD_DIRECTIONS = [
  "Up",
  "UpRight",
  "Right",
  "DownRight",
  "Down",
  "DownLeft",
  "Left",
  "UpLeft",
];

type ViewportSpriteAnimationMeta = {
  frameBaseOffset: number;
  weaponFrameOffset: number | null;
  frameCount: number;
  directionStride: number;
  frameIntervalMs?: number;
  reverse?: boolean;
};

export function buildViewportEntitySprite(
  entity: DisplayEntity,
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>,
  sceneFrameIndex: number,
  now: number,
  animationState: EntitySpriteAnimationState,
  motionSnapshot?: EntityMotionSnapshot,
): ViewportEntitySprite | null {
  const sprite = resolvedEntitySprite(entity, libraries, animationState);
  if (!sprite) {
    return null;
  }

  const animation = spriteAnimationMetaForEntity(entity, sprite, animationState);
  if (!animation) {
    return null;
  }

  const frameCycle = spriteFrameCycleForEntity(
    entity,
    sceneFrameIndex,
    now,
    animationState,
    animation,
    motionSnapshot,
  );
  const frameIndex =
    animation.frameBaseOffset +
    directionIndex(entity.direction) * animation.directionStride +
    frameCycle;
  const bodyLibraryKey = normalizeSceneSpriteLibraryKey(sprite.bodyLibrary);
  const hairLibraryKey = sprite.hairLibrary
    ? normalizeSceneSpriteLibraryKey(sprite.hairLibrary)
    : null;
  const weaponLibraryKey = sprite.weaponLibrary
    ? normalizeSceneSpriteLibraryKey(sprite.weaponLibrary)
    : null;
  const secondaryWeaponLibraryKey = sprite.weaponLibrarySecondary
    ? normalizeSceneSpriteLibraryKey(sprite.weaponLibrarySecondary)
    : null;
  const fallbackFrameIndex =
    sprite.frameBaseOffset +
    directionIndex(entity.direction) * Math.max(sprite.directionStride, 1);
  const bodyFrame = frameMetaForIndexWithFallback(libraries[bodyLibraryKey], frameIndex, fallbackFrameIndex);
  const hairFrame = hairLibraryKey
    ? frameMetaForIndexWithFallback(libraries[hairLibraryKey], frameIndex, fallbackFrameIndex)
    : null;
  const weaponFrameIndex =
    animation.weaponFrameOffset === null
      ? null
      : animation.weaponFrameOffset + directionIndex(entity.direction) * animation.directionStride + frameCycle;
  const fallbackWeaponFrameIndex =
    sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
      ? null
      : sprite.weaponFrameOffset + directionIndex(entity.direction) * Math.max(sprite.directionStride, 1);
  const primaryWeaponFrame =
    weaponLibraryKey && weaponFrameIndex !== null
      ? frameMetaForIndexWithFallback(libraries[weaponLibraryKey], weaponFrameIndex, fallbackWeaponFrameIndex)
      : null;
  const secondaryWeaponFrame =
    secondaryWeaponLibraryKey && weaponFrameIndex !== null
      ? frameMetaForIndexWithFallback(libraries[secondaryWeaponLibraryKey], weaponFrameIndex, fallbackWeaponFrameIndex)
      : null;
  const weaponPlacement = weaponPlacementForDirection(entity.direction);
  const classKey = entityClassKey(entity);
  const primaryWeaponLayer = viewportSpriteLayer(primaryWeaponFrame);
  const secondaryWeaponLayer = viewportSpriteLayer(secondaryWeaponFrame);
  const rearWeapons =
    classKey === "assassin"
      ? assassinRearWeaponsForDirection(entity.direction, primaryWeaponLayer, secondaryWeaponLayer)
      : weaponPlacement === "rear"
        ? [primaryWeaponLayer, secondaryWeaponLayer].filter((layer): layer is ViewportSpriteLayer => Boolean(layer))
        : [];
  const frontWeapons =
    classKey === "assassin"
      ? assassinFrontWeaponsForDirection(entity.direction, primaryWeaponLayer, secondaryWeaponLayer)
      : weaponPlacement === "front"
        ? [primaryWeaponLayer, secondaryWeaponLayer].filter((layer): layer is ViewportSpriteLayer => Boolean(layer))
        : [];
  // Crystal draws the mount as the bottom layer at `DrawFrame - 416 + MountOffset`
  // (`PlayerObject.cs:5084-5090`). The server only fills `mountLibrary` while riding
  // (and suppresses the weapon layers then), so this resolves to null on foot. We key
  // the mount frame off the rider's body frame so it tracks movement; if the
  // `Mount/NN` library is not yet in the manifest the layer resolves to null and is
  // simply skipped (the mount atlas is asset-gated behind the R2 release).
  const mountLibraryKey = sprite.mountLibrary
    ? normalizeSceneSpriteLibraryKey(sprite.mountLibrary)
    : null;
  const mountFrameIndex =
    (sprite.mountFrameOffset ?? animation.frameBaseOffset) +
    directionIndex(entity.direction) * animation.directionStride +
    frameCycle;
  const mountFrame = mountLibraryKey
    ? frameMetaForIndexWithFallback(libraries[mountLibraryKey], mountFrameIndex, fallbackFrameIndex)
    : null;
  const preloadAnimations = atlasPreloadAnimationsForEntity(entity, sprite, animationState, animation);
  const preloadFrames = animationPreloadFramesForEntity({
    libraries,
    animations: preloadAnimations,
    directions: atlasPreloadDirectionsForEntity(entity),
    bodyLibraryKey,
    hairLibraryKey,
    weaponLibraryKey,
    secondaryWeaponLibraryKey,
    fallbackFrameIndex,
    fallbackWeaponFrameIndex,
  });

  return {
    mount: viewportSpriteLayer(mountFrame),
    rearWeapons,
    body: viewportSpriteLayer(bodyFrame),
    hair: viewportSpriteLayer(hairFrame),
    frontWeapons,
    preloadFrames,
    preloadPaths: [...new Set(preloadFrames.map((frame) => frame.path))],
    nameplateTop: computeNameplateTop(bodyFrame, hairFrame, primaryWeaponFrame ?? secondaryWeaponFrame),
  };
}

function entityClassKey(entity: DisplayEntity): EntityClassKey {
  return entity.classKey ?? "warrior";
}

function entityGenderKey(entity: DisplayEntity): EntityGenderKey {
  return entity.genderKey ?? "male";
}

function sceneLibraryExists(
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>,
  libraryKey: string | null | undefined,
) {
  if (!libraryKey) {
    return false;
  }

  return normalizeSceneSpriteLibraryKey(libraryKey) in libraries;
}

function resolvedEntitySprite(
  entity: DisplayEntity,
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>,
  animationState: EntitySpriteAnimationState,
): EntitySprite | null {
  const sprite = entity.sprite;
  if (!sprite) {
    return null;
  }

  if (entity.kind === "monster" || entity.kind === "npc") {
    return sprite;
  }

  const classKey = entityClassKey(entity);
  const genderKey = entityGenderKey(entity);
  const bodyBaseOffset = genderKey === "female" ? 808 : 0;
  const weaponBaseOffset = genderKey === "female" ? 416 : 0;
  const spriteBodyLibrary = sprite.bodyLibrary;
  const spriteHairLibrary = sprite.hairLibrary;
  const spriteWeaponLibrary = sprite.weaponLibrary;
  const spriteWeaponLibrarySecondary = sprite.weaponLibrarySecondary;
  const spriteAltBodyLibrary = sprite.altBodyLibrary;
  const spriteAltHairLibrary = sprite.altHairLibrary;
  const spriteAltWeaponLibrary = sprite.altWeaponLibrary;
  const spriteAltWeaponLibrarySecondary = sprite.altWeaponLibrarySecondary;
  const isArcherAlt = Boolean(spriteAltBodyLibrary?.startsWith("ARArmour/"));
  const isAssassinAlt = Boolean(spriteAltBodyLibrary?.startsWith("AArmour/"));

  if (
    isArcherAlt &&
    (animationState === "walking" || animationState === "running" || animationState === "attackRange")
  ) {
    const altWeaponLibrary =
      spriteAltWeaponLibrary && sceneLibraryExists(libraries, spriteAltWeaponLibrary)
        ? spriteAltWeaponLibrary
        : sprite.weaponLibrary;
    return {
      ...sprite,
      bodyLibrary:
        spriteAltBodyLibrary && sceneLibraryExists(libraries, spriteAltBodyLibrary)
          ? spriteAltBodyLibrary
          : sprite.bodyLibrary,
      hairLibrary:
        spriteAltHairLibrary && sceneLibraryExists(libraries, spriteAltHairLibrary)
          ? spriteAltHairLibrary
          : sprite.hairLibrary,
      weaponLibrary: altWeaponLibrary,
      weaponLibrarySecondary: null,
      frameBaseOffset:
        sprite.altFrameBaseOffset !== undefined && sprite.altFrameBaseOffset !== null
          ? sprite.altFrameBaseOffset
          : bodyBaseOffset,
      weaponFrameOffset:
        sprite.altWeaponFrameOffset !== undefined && sprite.altWeaponFrameOffset !== null
          ? sprite.altWeaponFrameOffset
          : sprite.weaponFrameOffset,
    };
  }

  if (
    isAssassinAlt &&
    (animationState === "standing" ||
      animationState === "walking" ||
      animationState === "running" ||
      animationState === "attackMelee" ||
      animationState === "struck" ||
      animationState === "dying" ||
      animationState === "dead" ||
      animationState === "reviving")
  ) {
    return {
      ...sprite,
      bodyLibrary:
        spriteAltBodyLibrary && sceneLibraryExists(libraries, spriteAltBodyLibrary)
          ? spriteAltBodyLibrary
          : sprite.bodyLibrary,
      hairLibrary:
        spriteAltHairLibrary && sceneLibraryExists(libraries, spriteAltHairLibrary)
          ? spriteAltHairLibrary
          : sprite.hairLibrary,
      weaponLibrary:
        spriteAltWeaponLibrary && sceneLibraryExists(libraries, spriteAltWeaponLibrary)
          ? spriteAltWeaponLibrary
          : sprite.weaponLibrary,
      weaponLibrarySecondary:
        spriteAltWeaponLibrarySecondary && sceneLibraryExists(libraries, spriteAltWeaponLibrarySecondary)
          ? spriteAltWeaponLibrarySecondary
          : sprite.weaponLibrarySecondary,
      frameBaseOffset:
        sprite.altFrameBaseOffset !== undefined && sprite.altFrameBaseOffset !== null
          ? sprite.altFrameBaseOffset
          : bodyBaseOffset,
      weaponFrameOffset:
        sprite.altWeaponFrameOffset !== undefined && sprite.altWeaponFrameOffset !== null
          ? sprite.altWeaponFrameOffset
          : spriteAltWeaponLibrary && sceneLibraryExists(libraries, spriteAltWeaponLibrary)
            ? weaponBaseOffset
          : sprite.weaponFrameOffset,
    };
  }

  return sprite;
}

function viewportSpriteLayer(frame: OriginalSceneSpriteFrameMeta | null): ViewportSpriteLayer | null {
  if (!frame) {
    return null;
  }

  return {
    path: frame.path,
    width: frame.width,
    height: frame.height,
    x: frame.x,
    y: frame.y,
  };
}

function entitySpriteLayers(sprite: ViewportEntitySprite | null): ViewportSpriteLayer[] {
  if (!sprite) {
    return [];
  }

  return [sprite.mount, sprite.body, sprite.hair, ...sprite.rearWeapons, ...sprite.frontWeapons].filter(
    (layer): layer is ViewportSpriteLayer => Boolean(layer),
  );
}

function entitySpriteVisualBounds(sprite: ViewportEntitySprite | null): ViewportEntityBounds | null {
  const layers = entitySpriteLayers(sprite);
  if (!layers.length) {
    return null;
  }

  return layers.reduce<ViewportEntityBounds>(
    (bounds, layer) => ({
      left: Math.min(bounds.left, layer.x),
      top: Math.min(bounds.top, layer.y),
      right: Math.max(bounds.right, layer.x + layer.width),
      bottom: Math.max(bounds.bottom, layer.y + layer.height),
    }),
    {
      left: Number.POSITIVE_INFINITY,
      top: Number.POSITIVE_INFINITY,
      right: Number.NEGATIVE_INFINITY,
      bottom: Number.NEGATIVE_INFINITY,
    },
  );
}

function expandEntityBounds(bounds: ViewportEntityBounds, minWidth: number, minHeight: number): ViewportEntityBounds {
  const width = bounds.right - bounds.left;
  const height = bounds.bottom - bounds.top;
  const centerX = (bounds.left + bounds.right) / 2;
  const bottom = Math.max(bounds.bottom, 0);
  const nextWidth = Math.max(width, minWidth);
  const nextHeight = Math.max(height, minHeight);

  return {
    left: centerX - nextWidth / 2,
    top: Math.min(bounds.top, bottom - nextHeight),
    right: centerX + nextWidth / 2,
    bottom,
  };
}

export function entitySpriteHitBounds(sprite: ViewportEntitySprite | null): ViewportEntityBounds {
  const bounds = entitySpriteVisualBounds(sprite);
  if (!bounds) {
    return { left: -24, top: -64, right: 24, bottom: 0 };
  }

  return expandEntityBounds(bounds, 48, 64);
}

function frameMetaForIndexWithFallback(
  library: OriginalSceneSpriteLibraryMeta | null | undefined,
  frameIndex: number,
  fallbackFrameIndex: number | null,
) {
  return (
    frameMetaForIndex(library, frameIndex) ??
    (fallbackFrameIndex === null ? null : frameMetaForIndex(library, fallbackFrameIndex)) ??
    library?.frames[0] ??
    null
  );
}

function animationPreloadFramesForEntity({
  libraries,
  animations,
  directions,
  bodyLibraryKey,
  hairLibraryKey,
  weaponLibraryKey,
  secondaryWeaponLibraryKey,
  fallbackFrameIndex,
  fallbackWeaponFrameIndex,
}: {
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>;
  animations: ViewportSpriteAnimationMeta[];
  directions: Array<string | undefined>;
  bodyLibraryKey: string;
  hairLibraryKey: string | null;
  weaponLibraryKey: string | null;
  secondaryWeaponLibraryKey: string | null;
  fallbackFrameIndex: number | null;
  fallbackWeaponFrameIndex: number | null;
}) {
  const frames: ViewportSpriteLayer[] = [];
  for (const animation of animations) {
    for (const preloadDirection of directions) {
      const bodyFrameIndices = animationFrameIndices(
        animation.frameBaseOffset,
        preloadDirection,
        animation.directionStride,
        animation.frameCount,
      );
      const weaponFrameIndices =
        animation.weaponFrameOffset === null
          ? []
          : animationFrameIndices(
              animation.weaponFrameOffset,
              preloadDirection,
              animation.directionStride,
              animation.frameCount,
            );
      frames.push(
        ...frameLayersForIndices(libraries[bodyLibraryKey], bodyFrameIndices, fallbackFrameIndex),
        ...(hairLibraryKey
          ? frameLayersForIndices(libraries[hairLibraryKey], bodyFrameIndices, fallbackFrameIndex)
          : []),
        ...(weaponLibraryKey
          ? frameLayersForIndices(libraries[weaponLibraryKey], weaponFrameIndices, fallbackWeaponFrameIndex)
          : []),
        ...(secondaryWeaponLibraryKey
          ? frameLayersForIndices(libraries[secondaryWeaponLibraryKey], weaponFrameIndices, fallbackWeaponFrameIndex)
          : []),
      );
    }
  }
  const uniqueFrames = new Map<string, ViewportSpriteLayer>();
  for (const frame of frames) {
    uniqueFrames.set(`${frame.path}|${frame.width}x${frame.height}`, frame);
  }
  return [...uniqueFrames.values()];
}

function atlasPreloadAnimationsForEntity(
  entity: DisplayEntity,
  sprite: EntitySprite,
  animationState: EntitySpriteAnimationState,
  currentAnimation: ViewportSpriteAnimationMeta,
) {
  const states: EntitySpriteAnimationState[] =
    entity.kind === "npc"
      ? [animationState]
      : entity.kind === "monster"
        ? ["standing", "walking", animationState]
        : ["standing", "walking", "running", animationState];
  const animations = new Map<string, ViewportSpriteAnimationMeta>();
  const addAnimation = (animation: ViewportSpriteAnimationMeta | null) => {
    if (!animation) {
      return;
    }
    animations.set(
      [
        animation.frameBaseOffset,
        animation.weaponFrameOffset ?? "none",
        animation.frameCount,
        animation.directionStride,
        animation.reverse ? "reverse" : "forward",
      ].join(":"),
      animation,
    );
  };

  for (const state of states) {
    addAnimation(spriteAnimationMetaForEntity(entity, sprite, state));
  }
  addAnimation(currentAnimation);
  return [...animations.values()];
}

function atlasPreloadDirectionsForEntity(entity: DisplayEntity) {
  if (entity.kind === "selfPlayer" || entity.kind === "player") {
    return PLAYER_ATLAS_PRELOAD_DIRECTIONS;
  }
  return [entity.direction];
}

function animationFrameIndices(
  frameBaseOffset: number,
  direction: string | undefined,
  directionStride: number,
  frameCount: number,
) {
  const stride = Math.max(directionStride, 1);
  const count = Math.max(frameCount, 1);
  const base = frameBaseOffset + directionIndex(direction) * stride;

  return Array.from({ length: count }, (_, frameOffset) => base + frameOffset);
}

function frameLayersForIndices(
  library: OriginalSceneSpriteLibraryMeta | null | undefined,
  frameIndices: number[],
  fallbackFrameIndex: number | null,
) {
  if (!library) {
    return [];
  }

  const frames = frameIndices
    .map((frameIndex) => viewportSpriteLayer(frameMetaForIndexWithFallback(library, frameIndex, fallbackFrameIndex)))
    .filter((frame): frame is ViewportSpriteLayer => Boolean(frame));
  const uniqueFrames = new Map<string, ViewportSpriteLayer>();
  for (const frame of frames) {
    uniqueFrames.set(`${frame.path}|${frame.width}x${frame.height}`, frame);
  }
  return [...uniqueFrames.values()];
}

function spriteFrameCycleForEntity(
  entity: DisplayEntity,
  sceneFrameIndex: number,
  now: number,
  animationState: EntitySpriteAnimationState,
  animation: ViewportSpriteAnimationMeta,
  motionSnapshot?: EntityMotionSnapshot,
) {
  const frameCount = Math.max(animation.frameCount, 1);
  if (frameCount <= 1) {
    return 0;
  }

  const frameIntervalMs = animation.frameIntervalMs ?? 100;
  let cycle = sceneFrameIndex % frameCount;

  switch (animationState) {
    case "standing": {
      const standingStartedAt =
        motionSnapshot && motionSnapshot.expiresAt > 0 && motionSnapshot.expiresAt <= now
          ? motionSnapshot.expiresAt
          : 0;
      const phaseMs =
        standingStartedAt === 0 && entity.kind === "monster"
          ? stableEntityAnimationPhaseMs(entity, frameIntervalMs, frameCount)
          : 0;
      cycle = loopingFrameCycle(
        now + phaseMs,
        standingStartedAt,
        frameIntervalMs,
        frameCount,
      );
      break;
    }
    case "dead":
      cycle = 0;
      break;
    case "walking":
    case "running":
      cycle = loopingFrameCycle(
        now,
        motionSnapshot?.startedAt ?? entity.movementStartedAt ?? now,
        frameIntervalMs,
        frameCount,
      );
      break;
    case "attackMelee":
    case "attackRange":
      cycle = transientFrameCycle(now, entity.attackStartedAt, frameIntervalMs, frameCount);
      break;
    case "struck":
      cycle = transientFrameCycle(now, entity.struckStartedAt, frameIntervalMs, frameCount);
      break;
    case "dying":
      cycle = transientFrameCycle(now, entity.dieStartedAt, frameIntervalMs, frameCount);
      break;
    case "reviving":
      cycle = transientFrameCycle(now, entity.reviveStartedAt, frameIntervalMs, frameCount);
      break;
    default:
      break;
  }

  return animation.reverse ? frameCount - 1 - cycle : cycle;
}

function loopingFrameCycle(now: number, startedAt: number, frameIntervalMs: number, frameCount: number) {
  if (frameCount <= 1) {
    return 0;
  }

  const elapsed = Math.max(now - startedAt, 0);
  return Math.floor(elapsed / Math.max(frameIntervalMs, 1)) % frameCount;
}

function stableEntityAnimationPhaseMs(entity: DisplayEntity, frameIntervalMs: number, frameCount: number) {
  const numericId = Number(entity.objectId);
  const seed = Number.isFinite(numericId)
    ? numericId
    : Array.from(entity.objectId).reduce((total, char) => total + char.charCodeAt(0), 0);
  return (Math.abs(seed) % Math.max(frameCount, 1)) * Math.max(frameIntervalMs, 1);
}

function spriteAnimationMetaForEntity(
  entity: DisplayEntity,
  sprite: EntitySprite,
  animationState: EntitySpriteAnimationState,
): ViewportSpriteAnimationMeta | null {
  if (entity.kind === "npc") {
    return {
      frameBaseOffset: sprite.frameBaseOffset,
      weaponFrameOffset:
        sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
          ? null
          : sprite.weaponFrameOffset,
      frameCount: Math.max(sprite.frameCount, 1),
      directionStride: Math.max(sprite.directionStride, 1),
      frameIntervalMs: 450,
    };
  }

  if (entity.kind === "monster") {
    switch (animationState) {
      case "walking":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 32,
          weaponFrameOffset: null,
          frameCount: 6,
          directionStride: 6,
          frameIntervalMs: 100,
        };
      case "attackMelee":
      case "attackRange":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 80,
          weaponFrameOffset: null,
          frameCount: 6,
          directionStride: 6,
          frameIntervalMs: 100,
        };
      case "struck":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 128,
          weaponFrameOffset: null,
          frameCount: 2,
          directionStride: 2,
          frameIntervalMs: 200,
        };
      case "dying":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 144,
          weaponFrameOffset: null,
          frameCount: 10,
          directionStride: 10,
          frameIntervalMs: 100,
        };
      case "reviving":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 144,
          weaponFrameOffset: null,
          frameCount: 10,
          directionStride: 10,
          frameIntervalMs: 100,
          reverse: true,
        };
      case "dead":
        return {
          frameBaseOffset: sprite.frameBaseOffset + 153,
          weaponFrameOffset: null,
          frameCount: 1,
          directionStride: 1,
        };
      default:
        return {
          frameBaseOffset: sprite.frameBaseOffset,
          weaponFrameOffset: null,
          frameCount: Math.max(sprite.frameCount, 1),
          directionStride: Math.max(sprite.directionStride, 1),
          frameIntervalMs: 500,
        };
    }
  }

  const archerAlt =
    Boolean(sprite.bodyLibrary.startsWith("ARArmour/")) &&
    (animationState === "walking" || animationState === "running" || animationState === "attackRange");

  switch (animationState) {
    case "walking":
      return {
        frameBaseOffset: sprite.frameBaseOffset + (archerAlt ? 0 : 32),
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + (archerAlt ? 0 : 32),
        frameCount: 6,
        directionStride: 6,
        frameIntervalMs: 100,
      };
    case "running":
      return {
        frameBaseOffset: sprite.frameBaseOffset + (archerAlt ? 48 : 80),
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + (archerAlt ? 48 : 112),
        frameCount: 6,
        directionStride: 6,
        frameIntervalMs: 100,
      };
    case "attackMelee":
      return {
        frameBaseOffset:
          entity.attackAnimation === "melee2"
            ? sprite.frameBaseOffset + 184
            : entity.attackAnimation === "melee3"
              ? sprite.frameBaseOffset + 232
              : entity.attackAnimation === "melee4"
                ? sprite.frameBaseOffset + 416
                : sprite.frameBaseOffset + 136,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : entity.attackAnimation === "melee2"
              ? sprite.weaponFrameOffset + 216
              : entity.attackAnimation === "melee3"
                ? sprite.weaponFrameOffset + 264
                : entity.attackAnimation === "melee4"
                  ? sprite.weaponFrameOffset + 448
                  : sprite.weaponFrameOffset + 168,
        frameCount: entity.attackAnimation === "melee3" ? 8 : 6,
        directionStride: entity.attackAnimation === "melee3" ? 8 : 6,
        frameIntervalMs: 100,
      };
    case "attackRange":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 96,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 96,
        frameCount: 8,
        directionStride: 8,
        frameIntervalMs: 100,
      };
    case "struck":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 360,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 392,
        frameCount: 3,
        directionStride: 3,
        frameIntervalMs: 100,
      };
    case "dying":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 384,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 416,
        frameCount: 4,
        directionStride: 4,
        frameIntervalMs: 100,
      };
    case "dead":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 387,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 419,
        frameCount: 1,
        directionStride: 1,
      };
    case "reviving":
      return {
        frameBaseOffset: sprite.frameBaseOffset + 384,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset + 416,
        frameCount: 4,
        directionStride: 4,
        frameIntervalMs: 100,
        reverse: true,
      };
    default:
      return {
        frameBaseOffset: sprite.frameBaseOffset,
        weaponFrameOffset:
          sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
            ? null
            : sprite.weaponFrameOffset,
        frameCount: Math.max(sprite.frameCount, 1),
        directionStride: Math.max(sprite.directionStride, 1),
        frameIntervalMs: 500,
      };
  }
}

function directionIndex(direction?: string) {
  switch (direction) {
    case "Up":
      return 0;
    case "UpRight":
      return 1;
    case "Right":
      return 2;
    case "DownRight":
      return 3;
    case "Down":
      return 4;
    case "DownLeft":
      return 5;
    case "Left":
      return 6;
    case "UpLeft":
      return 7;
    default:
      return 4;
  }
}

function computeNameplateTop(
  bodyFrame: OriginalSceneSpriteFrameMeta | null,
  hairFrame: OriginalSceneSpriteFrameMeta | null,
  weaponFrame?: OriginalSceneSpriteFrameMeta | null,
) {
  const displayTop = bodyFrame?.y ?? -48;
  return displayTop - 10;
}

function nameplateTopOffset(sprite: ViewportEntitySprite | null) {
  return sprite?.nameplateTop ?? -60;
}

function fallbackEntityVisualCenterLeftOffset(entity: DisplayEntity) {
  return entity.kind === "npc" ? 40 : 25;
}

function entityVisualCenterLeftOffset(entity: DisplayEntity, sprite: ViewportEntitySprite | null) {
  const bounds = entitySpriteVisualBounds(sprite);
  return bounds ? (bounds.left + bounds.right) / 2 : fallbackEntityVisualCenterLeftOffset(entity);
}

export function entityQuestIconLeftOffset(entity: DisplayEntity, sprite: ViewportEntitySprite | null) {
  if (sprite?.body) {
    return sprite.body.x + sprite.body.width / 2 - 28;
  }
  return entityVisualCenterLeftOffset(entity, sprite) - 28;
}

export function entityQuestIconTopOffset(sprite: ViewportEntitySprite | null) {
  if (sprite?.body) {
    return sprite.body.y - 40;
  }
  const bounds = entitySpriteVisualBounds(sprite);
  return (bounds?.top ?? nameplateTopOffset(sprite)) - 40;
}

export function entityNameplateLeftOffset(entity: DisplayEntity, sprite: ViewportEntitySprite | null) {
  if (entity.kind === "npc") {
    return entityVisualCenterLeftOffset(entity, sprite);
  }
  return 25;
}

export function entityNameplateTopOffset(entity: DisplayEntity, sprite: ViewportEntitySprite | null) {
  const lineAdjustment =
    (entity.kind === "npc" || entity.kind === "monster") && entity.name.includes("_")
      ? -((entity.name.split("_").filter(Boolean).length - 1) * 10) / 2
      : 0;
  return nameplateTopOffset(sprite) + lineAdjustment;
}

function weaponPlacementForDirection(direction?: string) {
  switch (direction) {
    case "Left":
    case "Up":
    case "UpLeft":
    case "DownLeft":
      return "rear";
    default:
      return "front";
  }
}

function assassinRearWeaponsForDirection(
  direction: string | undefined,
  primaryWeapon: ViewportSpriteLayer | null,
  secondaryWeapon: ViewportSpriteLayer | null,
) {
  switch (direction) {
    case "Left":
    case "Up":
    case "UpLeft":
    case "DownLeft":
      return [primaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
    default:
      return [secondaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
  }
}

function assassinFrontWeaponsForDirection(
  direction: string | undefined,
  primaryWeapon: ViewportSpriteLayer | null,
  secondaryWeapon: ViewportSpriteLayer | null,
) {
  switch (direction) {
    case "UpRight":
    case "Right":
    case "DownRight":
    case "Down":
      return [primaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
    default:
      return [secondaryWeapon].filter((layer): layer is ViewportSpriteLayer => Boolean(layer));
  }
}

export function questIconForEntity(
  entity: DisplayEntity,
  questLog: DisplayQuest[],
  animationFrameIndex: number,
) {
  if (entity.kind !== "npc") {
    return null;
  }

  const activeQuest = questLog.find((quest) => quest.stage !== "completed") ?? null;
  if (!activeQuest) {
    return null;
  }
  if (!entity.questIds?.includes(activeQuest.questId)) {
    return null;
  }

  const iconKey: QuestIconKey =
    activeQuest.stage === "available"
      ? "exclamationYellow"
      : activeQuest.stage === "inProgress"
        ? "questionWhite"
        : activeQuest.stage === "readyToTurnIn"
          ? "questionYellow"
          : "questionGreen";

  const frames = ORIGINAL_UI.game.questIcons[iconKey];
  const questAnimationFrameIndex = Math.floor(
    (animationFrameIndex * SCENE_SPRITE_FRAME_INTERVAL_MS) / CRYSTAL_QUEST_ICON_FRAME_INTERVAL_MS,
  );
  return frames[questAnimationFrameIndex % frames.length] ?? null;
}

export function portraitFramesForCharacter(character: SelectCharacterEntry) {
  const key = `${character.classKey}${character.gender === "male" ? "Male" : "Female"}` as SelectPortraitKey;
  return SELECT_PORTRAIT_ANIMATIONS[key];
}

export function entityNameplateColor(entity: DisplayEntity) {
  return argbToCssColor(entity.nameColourArgb) ?? (entity.kind === "npc" ? "#00ff00" : "#ffffff");
}
