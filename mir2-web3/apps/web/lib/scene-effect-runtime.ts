import {
  effectFrameAt,
  resolveMapEffect,
  resolveMapEffectByNumber,
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

/**
 * Crystal sorts persistent ObjectSpell instances before actors in each map
 * cell. Keep their optional mask adjacent to the body without lifting either
 * above the entity layer (64).
 */
export function crystalSceneEffectLayerOffset(
  source: DisplaySceneEffect["source"],
  mask = false,
): number {
  const base = source === "spell"
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
