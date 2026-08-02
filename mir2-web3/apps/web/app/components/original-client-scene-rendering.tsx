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
import type { BevyPresentationEntityMotion } from "./original-client-presentation-pose";
export {
  GameSceneBackdrop,
  buildViewportMapSprites,
  handleSceneAssetImageError,
  handleSceneAssetImageLoad,
  mapSpriteBlendMode,
  mapSpriteRenderPath,
  resolvedMapSpriteBlendMode,
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
  rebaseViewportEntitiesToRenderPlayer,
  refreshEntityMotionSnapshots,
} from "./original-client-scene-motion";
import {
  CRYSTAL_MOVE_FRAME_COUNT,
  CRYSTAL_MOVE_FRAME_INTERVAL_MS,
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  DEFAULT_VIEWPORT_LAYOUT,
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
  viewportLayoutForStage,
  type SceneBackdropTile,
  type ViewportMapSprite,
  type ViewportMapSprites,
  type ViewportOffset,
  type ViewportLayout,
} from "./original-client-scene-layout";
export {
  CRYSTAL_MOVE_INPUT_INTERVAL_MS,
  EMPTY_VIEWPORT_MAP_SPRITES,
  EMPTY_VIEWPORT_OFFSET,
  DEFAULT_VIEWPORT_LAYOUT,
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
  viewportLayoutForStage,
  type SceneBackdropTile,
  type ViewportMapSprite,
  type ViewportMapSprites,
  type ViewportOffset,
  type ViewportLayout,
} from "./original-client-scene-layout";
import type {
  CrystalEntityAnimationAction,
  CrystalEntityAnimationPose,
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
import {
  crystalPlayerAnimationMeta,
  type CrystalPlayerActionKey,
} from "./original-client-player-frames";
import { crystalEntityAnimationMeta } from "./original-client-entity-frames";

type ViewportSpriteLayer = Pick<
  OriginalSceneSpriteFrameMeta,
  "path" | "width" | "height" | "x" | "y" | "shadowX" | "shadowY" | "maskPath"
>;

type ViewportEffectLayer = ViewportSpriteLayer & { blend: boolean };

export type ViewportEntitySprite = {
  mount: ViewportSpriteLayer | null;
  rearWeapons: ViewportSpriteLayer[];
  body: ViewportSpriteLayer | null;
  hair: ViewportSpriteLayer | null;
  frontWeapons: ViewportSpriteLayer[];
  effect: ViewportEffectLayer | null;
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

const SCENE_SPRITE_FRAME_INTERVAL_MS = 100;
const CRYSTAL_QUEST_ICON_FRAME_INTERVAL_MS = 500;
const ENTITY_ATLAS_PRELOAD_DIRECTIONS = [
  "Up",
  "UpRight",
  "Right",
  "DownRight",
  "Down",
  "DownLeft",
  "Left",
  "UpLeft",
];

const ENTITY_ATLAS_PRELOAD_STATES: readonly EntitySpriteAnimationState[] = [
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
];

type ViewportEffectAnimationMeta = {
  frameBaseOffset: number;
  frameCount: number;
  directionStride: number;
  frameIntervalMs?: number;
  reverse?: boolean;
  blend?: boolean;
};

type ViewportSpriteAnimationMeta = ViewportEffectAnimationMeta & {
  frameBaseOffset: number;
  mountFrameBaseOffset?: number;
  weaponFrameOffset: number | null;
  effect?: ViewportEffectAnimationMeta;
};

export function buildViewportEntitySprite(
  entity: DisplayEntity,
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>,
  sceneFrameIndex: number,
  now: number,
  animationState: EntitySpriteAnimationState,
  motionSnapshot?: EntityMotionSnapshot,
  presentationMotion?: BevyPresentationEntityMotion | null,
  animationPose?: CrystalEntityAnimationPose | null,
): ViewportEntitySprite | null {
  const sprite = resolvedEntitySprite(entity, libraries, animationState, animationPose?.action);
  if (!sprite) {
    return null;
  }

  const bodyLibraryKey = normalizeSceneSpriteLibraryKey(sprite.bodyLibrary);
  const animation = spriteAnimationMetaForEntity(
    entity,
    sprite,
    animationState,
    libraries[bodyLibraryKey],
    animationPose?.action,
  );
  if (!animation) return null;

  const frameCycle = spriteFrameCycleForEntity(
    entity,
    sceneFrameIndex,
    now,
    animationState,
    animation,
    motionSnapshot,
    presentationMotion,
    animationPose,
  );
  const presentationDirection =
    animationPose?.direction ?? presentationMotion?.direction ?? entity.direction;
  const frameIndex =
    animation.frameBaseOffset +
    directionIndex(presentationDirection) * animation.directionStride +
    frameCycle;
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
    directionIndex(presentationDirection) * Math.max(sprite.directionStride, 1);
  const bodyFrame = frameMetaForIndexWithFallback(libraries[bodyLibraryKey], frameIndex, fallbackFrameIndex);
  const effectCycle = animation.effect
    ? spriteFrameCycleForEntity(
        entity,
        sceneFrameIndex,
        now,
        animationState,
        { ...animation.effect, weaponFrameOffset: null },
        motionSnapshot,
        presentationMotion,
        animationPose,
      )
    : 0;
  const effectFrameIndex = animation.effect
    ? animation.effect.frameBaseOffset +
      directionIndex(presentationDirection) * animation.effect.directionStride +
      effectCycle
    : null;
  const effectFrame = effectFrameIndex === null
    ? null
    : frameMetaForIndexWithFallback(libraries[bodyLibraryKey], effectFrameIndex, null);
  const hairFrame = hairLibraryKey
    ? frameMetaForIndexWithFallback(libraries[hairLibraryKey], frameIndex, fallbackFrameIndex)
    : null;
  const weaponFrameIndex =
    animation.weaponFrameOffset === null
      ? null
      : animation.weaponFrameOffset + directionIndex(presentationDirection) * animation.directionStride + frameCycle;
  const fallbackWeaponFrameIndex =
    sprite.weaponFrameOffset === undefined || sprite.weaponFrameOffset === null
      ? null
      : sprite.weaponFrameOffset + directionIndex(presentationDirection) * Math.max(sprite.directionStride, 1);
  const primaryWeaponFrame =
    weaponLibraryKey && weaponFrameIndex !== null
      ? frameMetaForIndexWithFallback(libraries[weaponLibraryKey], weaponFrameIndex, fallbackWeaponFrameIndex)
      : null;
  const secondaryWeaponFrame =
    secondaryWeaponLibraryKey && weaponFrameIndex !== null
      ? frameMetaForIndexWithFallback(libraries[secondaryWeaponLibraryKey], weaponFrameIndex, fallbackWeaponFrameIndex)
      : null;
  const weaponPlacement = weaponPlacementForDirection(presentationDirection);
  const classKey = entityClassKey(entity);
  const ridingMount = Boolean(sprite.mountLibrary);
  const primaryWeaponLayer = viewportSpriteLayer(primaryWeaponFrame);
  const secondaryWeaponLayer = viewportSpriteLayer(secondaryWeaponFrame);
  const rearWeapons = ridingMount
    ? []
    : classKey === "assassin"
      ? assassinRearWeaponsForDirection(presentationDirection, primaryWeaponLayer, secondaryWeaponLayer)
      : weaponPlacement === "rear"
        ? [primaryWeaponLayer, secondaryWeaponLayer].filter((layer): layer is ViewportSpriteLayer => Boolean(layer))
        : [];
  const frontWeapons = ridingMount
    ? []
    : classKey === "assassin"
      ? assassinFrontWeaponsForDirection(presentationDirection, primaryWeaponLayer, secondaryWeaponLayer)
      : weaponPlacement === "front"
        ? [primaryWeaponLayer, secondaryWeaponLayer].filter((layer): layer is ViewportSpriteLayer => Boolean(layer))
        : [];
  // Crystal draws the mount below the rider at `DrawFrame - 416 + MountOffset`.
  // Mounted body frames start at 416, while mount libraries start at zero.
  const mountLibraryKey = sprite.mountLibrary
    ? normalizeSceneSpriteLibraryKey(sprite.mountLibrary)
    : null;
  const mountFrameIndex =
    (animation.mountFrameBaseOffset ?? sprite.mountFrameOffset ?? 0) +
    directionIndex(presentationDirection) * animation.directionStride +
    frameCycle;
  const fallbackMountFrameIndex = directionIndex(presentationDirection) * 4;
  const mountFrame = mountLibraryKey
    ? frameMetaForIndexWithFallback(libraries[mountLibraryKey], mountFrameIndex, fallbackMountFrameIndex)
    : null;
  const preloadAnimations = atlasPreloadAnimationsForEntity(
    entity,
    sprite,
    animationState,
    animation,
    libraries[bodyLibraryKey],
  );
  const preloadFrames = animationPreloadFramesForEntity({
    libraries,
    animations: preloadAnimations,
    directions: atlasPreloadDirectionsForEntity(entity),
    bodyLibraryKey,
    hairLibraryKey,
    weaponLibraryKey,
    secondaryWeaponLibraryKey,
    mountLibraryKey,
    fallbackFrameIndex,
    fallbackWeaponFrameIndex,
    fallbackMountFrameIndex,
  });

  return {
    mount: viewportSpriteLayer(mountFrame),
    rearWeapons,
    body: viewportSpriteLayer(bodyFrame),
    hair: viewportSpriteLayer(hairFrame),
    frontWeapons,
    effect:
      effectFrame && animation.effect
        ? { ...viewportSpriteLayer(effectFrame)!, blend: Boolean(animation.effect.blend) }
        : null,
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
  animationAction?: CrystalEntityAnimationAction,
): EntitySprite | null {
  const sprite = entity.sprite;
  if (!sprite) {
    return null;
  }

  if (entity.kind === "monster" || entity.kind === "npc") {
    return sprite;
  }

  // Mounted actions live in the common CArmour/CHair frame table. Archer and
  // assassin alternate atlases do not carry Crystal's 416+ mount actions.
  if (sprite.mountLibrary) {
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
  const attackAnimation = attackAnimationForCrystalAction(animationAction) ?? entity.attackAnimation;

  if (
    isArcherAlt &&
    (animationState === "walking" ||
      animationState === "running" ||
      (animationState === "attackRange" && attackAnimation !== "spell"))
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
    shadowX: frame.shadowX,
    shadowY: frame.shadowY,
    maskPath: frame.maskPath,
  };
}

function entitySpriteLayers(sprite: ViewportEntitySprite | null): ViewportSpriteLayer[] {
  if (!sprite) {
    return [];
  }

  return [sprite.mount, sprite.body, sprite.hair, ...sprite.rearWeapons, ...sprite.frontWeapons, sprite.effect].filter(
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
  mountLibraryKey,
  fallbackFrameIndex,
  fallbackWeaponFrameIndex,
  fallbackMountFrameIndex,
}: {
  libraries: Record<string, OriginalSceneSpriteLibraryMeta>;
  animations: ViewportSpriteAnimationMeta[];
  directions: Array<string | undefined>;
  bodyLibraryKey: string;
  hairLibraryKey: string | null;
  weaponLibraryKey: string | null;
  secondaryWeaponLibraryKey: string | null;
  mountLibraryKey: string | null;
  fallbackFrameIndex: number | null;
  fallbackWeaponFrameIndex: number | null;
  fallbackMountFrameIndex: number | null;
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
      const mountFrameIndices =
        animation.mountFrameBaseOffset === undefined
          ? []
          : animationFrameIndices(
              animation.mountFrameBaseOffset,
              preloadDirection,
              animation.directionStride,
              animation.frameCount,
            );
      const effectFrameIndices = animation.effect
        ? animationFrameIndices(
            animation.effect.frameBaseOffset,
            preloadDirection,
            animation.effect.directionStride,
            animation.effect.frameCount,
          )
        : [];
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
        ...(mountLibraryKey
          ? frameLayersForIndices(libraries[mountLibraryKey], mountFrameIndices, fallbackMountFrameIndex)
          : []),
        ...frameLayersForIndices(libraries[bodyLibraryKey], effectFrameIndices, null),
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
  bodyLibrary: OriginalSceneSpriteLibraryMeta | null | undefined,
) {
  // Keep the atlas source set stable across short-lived actions. Building only
  // the current action made a turn, hit, or attack generate a new atlas key;
  // the 600 ms attack could finish before that asynchronous swap completed.
  const states: readonly EntitySpriteAnimationState[] =
    entity.kind === "npc" ? ["standing", "harvesting"] : ENTITY_ATLAS_PRELOAD_STATES;
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
        animation.blend ? "blend" : "normal",
        animation.effect
          ? `${animation.effect.frameBaseOffset}/${animation.effect.frameCount}/${animation.effect.directionStride}`
          : "no-effect",
      ].join(":"),
      animation,
    );
  };

  for (const state of states) {
    addAnimation(spriteAnimationMetaForEntity(entity, sprite, state, bodyLibrary));
  }
  if (entity.kind !== "monster" && entity.kind !== "npc" && !sprite.mountLibrary) {
    // attackMelee selects one variant from the live packet. Preload every
    // Crystal melee family so attackType changes never mutate atlas residency.
    for (const action of ["attack1", "attack2", "attack3", "attack4"] as const) {
      addAnimation(playerAnimationMetaForAction(sprite, action));
    }
    addAnimation(playerAnimationMetaForAction(sprite, "spell"));
  }
  addAnimation(currentAnimation);
  return [...animations.values()];
}

function atlasPreloadDirectionsForEntity(_entity: DisplayEntity) {
  // Monsters turn while chasing and NPCs can be redirected by packets. Keeping
  // all eight directions resident avoids a per-turn atlas-key change.
  return ENTITY_ATLAS_PRELOAD_DIRECTIONS;
}

function animationFrameIndices(
  frameBaseOffset: number,
  direction: string | undefined,
  directionStride: number,
  frameCount: number,
) {
  const stride = Math.max(directionStride, 0);
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
  presentationMotion?: BevyPresentationEntityMotion | null,
  animationPose?: CrystalEntityAnimationPose | null,
) {
  const frameCount = Math.max(animation.frameCount, 1);
  if (frameCount <= 1) {
    return 0;
  }

  const frameIntervalMs = animation.frameIntervalMs ?? 100;
  if (animationPose) {
    const poseFrame = Math.min(
      Math.max(Math.trunc(animationPose.logicalFrameIndex), 0),
      frameCount - 1,
    );
    return animation.reverse ? frameCount - 1 - poseFrame : poseFrame;
  }
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
      cycle = presentationMotion
        ? Math.min(presentationMotion.frameIndex, frameCount - 1)
        : loopingFrameCycle(
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

function playerAnimationMetaForAction(
  sprite: EntitySprite,
  action: CrystalPlayerActionKey,
  includeWeapon = true,
): ViewportSpriteAnimationMeta {
  return crystalPlayerAnimationMeta(
    action,
    sprite.frameBaseOffset,
    sprite.weaponFrameOffset,
    includeWeapon,
  );
}

function spriteAnimationMetaForEntity(
  entity: DisplayEntity,
  sprite: EntitySprite,
  animationState: EntitySpriteAnimationState,
  bodyLibrary?: OriginalSceneSpriteLibraryMeta | null,
  animationAction?: CrystalEntityAnimationAction,
): ViewportSpriteAnimationMeta | null {
  const attackAnimation = attackAnimationForCrystalAction(animationAction) ?? entity.attackAnimation;
  if (entity.kind === "monster" || entity.kind === "npc") {
    const frameSetAnimation = crystalEntityAnimationMeta(
      bodyLibrary?.frameSet,
      animationState,
      sprite.frameBaseOffset,
      attackAnimation,
    );
    if (frameSetAnimation) {
      return { ...frameSetAnimation, weaponFrameOffset: null };
    }
  }

  if (entity.kind === "npc") {
    if (animationState === "harvesting") {
      return {
        frameBaseOffset: sprite.frameBaseOffset + 12,
        weaponFrameOffset: null,
        frameCount: 10,
        directionStride: 10,
        frameIntervalMs: 200,
      };
    }
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

  if (sprite.mountLibrary) {
    switch (animationState) {
      case "walking":
        return playerAnimationMetaForAction(sprite, "mountWalking", false);
      case "running":
        return playerAnimationMetaForAction(sprite, "mountRunning", false);
      case "struck":
        return playerAnimationMetaForAction(sprite, "mountStruck", false);
      case "attackMelee":
      case "attackRange":
        return playerAnimationMetaForAction(sprite, "mountAttack", false);
      default:
        return playerAnimationMetaForAction(sprite, "mountStanding", false);
    }
  }

  const archerAlt =
    Boolean(sprite.bodyLibrary.startsWith("ARArmour/")) &&
    (animationState === "walking" ||
      animationState === "running" ||
      (animationState === "attackRange" && attackAnimation !== "spell"));

  switch (animationState) {
    case "walking":
      return playerAnimationMetaForAction(sprite, archerAlt ? "archerWalking" : "walking");
    case "running":
      return playerAnimationMetaForAction(sprite, archerAlt ? "archerRunning" : "running");
    case "attackMelee":
      return playerAnimationMetaForAction(
        sprite,
        attackAnimation === "melee2"
          ? "attack2"
          : attackAnimation === "melee3"
            ? "attack3"
            : attackAnimation === "melee4"
              ? "attack4"
              : "attack1",
      );
    case "attackRange":
      return playerAnimationMetaForAction(
        sprite,
        attackAnimation === "spell" ? "spell" : "attackRange",
      );
    case "struck":
      return playerAnimationMetaForAction(sprite, "struck");
    case "dying":
      return playerAnimationMetaForAction(sprite, "dying");
    case "dead":
      return playerAnimationMetaForAction(sprite, "dead");
    case "reviving":
      return playerAnimationMetaForAction(sprite, "reviving");
    default:
      return playerAnimationMetaForAction(sprite, "standing");
  }
}

function attackAnimationForCrystalAction(
  action?: CrystalEntityAnimationAction,
): DisplayEntity["attackAnimation"] | undefined {
  switch (action) {
    case "attack2":
      return "melee2";
    case "attack3":
      return "melee3";
    case "attack4":
      return "melee4";
    case "attackRange1":
      return "range";
    case "spell":
      return "spell";
    case "attack1":
      return "melee1";
    default:
      return undefined;
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
  // The DOM node owns Crystal's fixed 48/50px DisplayRectangle. Keeping its
  // left edge on the integer cell origin avoids percentage-centering at .5px.
  return 0;
}

export function entityNameplateTopOffset(entity: DisplayEntity, sprite: ViewportEntitySprite | null) {
  // MirLabel is 12px high in this presentation. These are Crystal's alive
  // DisplayRectangle formulas after substituting half the label height:
  // player: -(31 - 6) + 8 = -17; NPC/monster: -(32 - 6) + 8 = -18.
  const displayRectangleOffset = entity.kind === "npc" || entity.kind === "monster" ? -18 : -17;
  const lineAdjustment =
    (entity.kind === "npc" || entity.kind === "monster") && entity.name.includes("_")
      ? -((entity.name.split("_").filter(Boolean).length - 1) * 10) / 2
      : 0;
  return displayRectangleOffset + lineAdjustment + (entity.dead ? 35 : 0);
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

  const markerPriority: Record<DisplayQuest["stage"], number> = {
    readyToTurnIn: 0,
    available: 1,
    inProgress: 2,
    completed: 3,
  };
  const activeQuest = questLog
    .filter((quest) => quest.stage !== "completed" && entity.questIds?.includes(quest.questId))
    .sort((left, right) => markerPriority[left.stage] - markerPriority[right.stage])[0] ?? null;
  if (!activeQuest) {
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
