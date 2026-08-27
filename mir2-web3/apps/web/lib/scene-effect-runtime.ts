import {
  effectFrameAt,
  resolveMapEffect,
  resolveMapEffectByNumber,
  resolveSpellAttackOverlayEffect,
  resolveSpellCastEffect,
  resolveSpellEffect,
  spellNameForNumber,
  type EffectAnimation,
  type EffectAssets,
  type EffectFrameMeta,
} from "./crystal-magic-effects";
import type { DisplaySceneEffect } from "../app/components/original-client-types";

// Crystal's DrawBlend path uses SourceAlpha + One. CSS plus-lighter is the
// direct bounded equivalent once additive source texels are alpha-normalized.
export const CRYSTAL_ADDITIVE_MIX_BLEND_MODE = "plus-lighter" as const;

const CRYSTAL_GROUND_EFFECT_LAYER_OFFSET = 48;
const CRYSTAL_TRANSIENT_SPELL_LAYER_OFFSET = 90;
export const FLAMING_SWORD_SPELL_ID = 8;
export const FLAMING_SWORD_ATTACK_DURATION_MS = 600;

const CRYSTAL_ATTACK_DIRECTIONS = [
  "Up",
  "UpRight",
  "Right",
  "DownRight",
  "Down",
  "DownLeft",
  "Left",
  "UpLeft",
] as const;

/**
 * Crystal sorts persistent ObjectSpell instances before actors in each map
 * cell. Keep their optional mask adjacent to the body without lifting either
 * above the entity layer (64).
 */
export function crystalSceneEffectLayerOffset(
  source: DisplaySceneEffect["source"],
  mask = false,
): number {
  const base = source === "spell" || source === "attackOverlay"
    ? CRYSTAL_TRANSIENT_SPELL_LAYER_OFFSET
    : CRYSTAL_GROUND_EFFECT_LAYER_OFFSET;
  return base + (mask ? 1 : 0);
}

export function sceneEffectAnimationAssetUrls(animation: EffectAnimation): string[] {
  return Array.from(
    new Set(
      animation.frames.flatMap((frame) =>
        frame.maskPath ? [frame.path, frame.maskPath] : [frame.path],
      ),
    ),
  );
}

export type ResolvedSceneEffectFrame = {
  effect: DisplaySceneEffect;
  animation: EffectAnimation;
  frame: EffectFrameMeta;
};

function resolveEffectAnimation(
  assets: EffectAssets,
  effect: DisplaySceneEffect,
): EffectAnimation | null {
  if (effect.source === "attackOverlay") {
    const name = typeof effect.spellOrEffect === "number"
      ? spellNameForNumber(effect.spellOrEffect)
      : effect.spellOrEffect;
    return name
      ? resolveSpellAttackOverlayEffect(assets, name, effect.direction)
      : null;
  }
  if (effect.source === "spell" || effect.source === "objectSpell") {
    const name = typeof effect.spellOrEffect === "number"
      ? spellNameForNumber(effect.spellOrEffect)
      : effect.spellOrEffect;
    if (!name) return null;
    // Crystal's ObjectSpell is a tile-anchored world object. Its animation can
    // differ from the caster animation for the same Spell enum value.
    return effect.source === "objectSpell"
      ? resolveMapEffect(assets, name, effect.value) ?? resolveSpellEffect(assets, name, effect.direction)
      : resolveSpellCastEffect(assets, name, effect.direction) ?? resolveMapEffect(assets, name, effect.value);
  }
  return typeof effect.spellOrEffect === "number"
    ? resolveMapEffectByNumber(assets, effect.spellOrEffect, effect.value)
    : resolveMapEffect(assets, effect.spellOrEffect, effect.value);
}

export function isFlamingSwordAttackSpell(spell: unknown): boolean {
  return spell === FLAMING_SWORD_SPELL_ID || spell === "FlamingSword";
}

function packetNumber(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function packetObjectId(value: unknown): string | null {
  if ((typeof value !== "number" && typeof value !== "string") || String(value) === "0") {
    return null;
  }
  const objectId = String(value).trim();
  return objectId ? objectId : null;
}

function packetDirectionIndex(value: unknown): number {
  if (typeof value === "number" && Number.isFinite(value)) {
    return ((Math.trunc(value) % CRYSTAL_ATTACK_DIRECTIONS.length) +
      CRYSTAL_ATTACK_DIRECTIONS.length) % CRYSTAL_ATTACK_DIRECTIONS.length;
  }
  const index = CRYSTAL_ATTACK_DIRECTIONS.indexOf(
    String(value) as (typeof CRYSTAL_ATTACK_DIRECTIONS)[number],
  );
  return index >= 0 ? index : 4;
}

/**
 * Project one authoritative ObjectAttack(spell=FlamingSword) into the stable,
 * attacker-bound scene-effect state consumed by both the web renderer and its
 * deterministic tests. SpellToggle is intentionally outside this function.
 */
export function createFlamingSwordAttackOverlay(
  payload: Record<string, unknown>,
  now: number,
  fallbackAnchor?: { x: number; y: number } | null,
): DisplaySceneEffect | null {
  if (!isFlamingSwordAttackSpell(payload.spell)) return null;
  const objectId = packetObjectId(payload.objectId);
  if (!objectId) return null;
  const location = payload.location as { x?: unknown; y?: unknown } | undefined;
  const x = packetNumber(location?.x) ?? fallbackAnchor?.x ?? null;
  const y = packetNumber(location?.y) ?? fallbackAnchor?.y ?? null;
  if (x === null || y === null || !Number.isFinite(now)) return null;
  return {
    key: `crystal-attack-overlay:FlamingSword:${objectId}`,
    source: "attackOverlay",
    spellOrEffect: FLAMING_SWORD_SPELL_ID,
    objectId,
    x,
    y,
    direction: packetDirectionIndex(payload.direction),
    value: 0,
    startedAt: now,
    expiresAt: now + FLAMING_SWORD_ATTACK_DURATION_MS,
  };
}

/** Replace only the same attacker's overlay while retaining other attackers. */
export function upsertFlamingSwordAttackOverlay(
  effects: readonly DisplaySceneEffect[],
  overlay: DisplaySceneEffect,
  now: number,
  maxEffects = 96,
): DisplaySceneEffect[] {
  return [
    ...effects
      .filter((effect) => effect.expiresAt > now && effect.key !== overlay.key)
      .slice(-(Math.max(1, maxEffects) - 1)),
    overlay,
  ];
}

/**
 * Reducer for the ObjectAttack -> world.effects boundary. Ordinary attacks
 * preserve the current array by reference; FlamingSword atomically inserts or
 * restarts the stable attacker overlay.
 */
export function applyObjectAttackSceneEffects(
  effects: DisplaySceneEffect[],
  payload: Record<string, unknown>,
  now: number,
  fallbackAnchor?: { x: number; y: number } | null,
): DisplaySceneEffect[] {
  const overlay = createFlamingSwordAttackOverlay(payload, now, fallbackAnchor);
  return overlay ? upsertFlamingSwordAttackOverlay(effects, overlay, now) : effects;
}

export type ObjectAttackAnimation = "melee1" | "melee2" | "melee3" | "melee4" | "range";

export type ObjectAttackSceneEntity = {
  objectId: string;
  x: number;
  y: number;
  direction?: string;
  attackAnimation?: ObjectAttackAnimation | "spell";
  attackStartedAt?: number;
  attackUntil?: number;
};

function objectAttackAnimation(payload: Record<string, unknown>): ObjectAttackAnimation {
  if (typeof payload.spell === "string") return "range";
  switch (packetNumber(payload.attackType)) {
    case 1:
      return "melee2";
    case 2:
      return "melee3";
    case 3:
      return "melee4";
    default:
      return "melee1";
  }
}

/**
 * Complete ObjectAttack projection used by page.tsx: actor pose and the
 * attacker-bound effect store are updated in one deterministic state step.
 */
export function applyObjectAttackSceneState<T extends ObjectAttackSceneEntity>(
  entities: T[],
  effects: DisplaySceneEffect[],
  payload: Record<string, unknown>,
  now: number,
  actionDurationMs: (entity: T, animation: ObjectAttackAnimation) => number,
): { entities: T[]; effects: DisplaySceneEffect[] } {
  const objectId = packetObjectId(payload.objectId);
  if (!objectId) return { entities, effects };
  const location = payload.location as { x?: unknown; y?: unknown } | undefined;
  const anchor = entities.find((entity) => entity.objectId === objectId) ?? null;
  const animation = objectAttackAnimation(payload);
  const nextEntities = entities.map((entity) =>
    entity.objectId === objectId
      ? {
          ...entity,
          x: typeof location?.x === "number" ? location.x : entity.x,
          y: typeof location?.y === "number" ? location.y : entity.y,
          direction: typeof payload.direction === "string" ? payload.direction : entity.direction,
          attackAnimation: animation,
          attackStartedAt: now,
          attackUntil: now + actionDurationMs(entity, animation),
        }
      : entity,
  );
  return {
    entities: nextEntities,
    effects: applyObjectAttackSceneEffects(effects, payload, now, anchor),
  };
}

/** Resolve a packet-backed scene effect to its exact source frame for this tick. */
export function resolveSceneEffectFrame(
  assets: EffectAssets | null,
  effect: DisplaySceneEffect,
  now: number,
): ResolvedSceneEffectFrame | null {
  if (!assets || now < effect.startedAt || now >= effect.expiresAt) return null;
  const animation = resolveEffectAnimation(assets, effect);
  if (!animation) return null;
  const frame = effectFrameAt(
    {
      key: effect.key,
      animation,
      tileX: effect.x,
      tileY: effect.y,
      startedAt: effect.startedAt,
      expiresAt: effect.expiresAt,
    },
    now,
  );
  return frame ? { effect, animation, frame } : null;
}

export function collectResolvedSceneEffectFrames(
  assets: EffectAssets | null,
  effects: readonly DisplaySceneEffect[],
  now: number,
): ResolvedSceneEffectFrame[] {
  return effects.flatMap((effect) => {
    const resolved = resolveSceneEffectFrame(assets, effect, now);
    return resolved ? [resolved] : [];
  });
}
