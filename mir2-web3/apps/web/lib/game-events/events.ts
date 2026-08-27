// Typed discriminated-union event vocabulary for the client game event bus.
//
// Each variant carries the minimal payload the existing inline trigger functions
// need — objectId, soundId, spell, damage, etc. — so a future packet handler can
// emit a GameEvent and let subscribers (sound, VFX) react without the handler
// knowing about audio or DOM.
//
// Design notes:
//   • Discriminated union: every variant has a literal `type` field.
//   • No DOM / no React / no third-party deps — plain TS types only.
//   • `SoundEntityRef` is re-exported from original-sound-triggers to keep the
//     subscriber's injected interface consistent with the real implementation.

export type { SoundEntityRef } from "../original-sound-triggers";
export type { OriginalSoundEvent } from "../original-sound-events";

import type { OriginalSoundEvent } from "../original-sound-events";

// ---------------------------------------------------------------------------
// Damage / combat type (mirrors Crystal DamageType enum)
// ---------------------------------------------------------------------------

/** Crystal DamageType: 0 = normal hit, 1 = miss, 2 = critical. */
export type CrystalDamageType = 0 | 1 | 2;

// ---------------------------------------------------------------------------
// Individual event variants
// ---------------------------------------------------------------------------

/** An entity performed a melee/ranged attack. Triggers attack sound + animation. */
export type EntityAttackEvent = {
  type: "entityAttack";
  /** Stringified objectId of the attacker. */
  objectId: string;
  /** Optional Crystal Spell identity carried by ObjectAttack. */
  spell?: number | string | null;
};

/** An entity was struck (took a hit). Triggers struck sound + hit-flash. */
export type EntityStruckEvent = {
  type: "entityStruck";
  /** Stringified objectId of the struck entity. */
  objectId: string;
};

/** An entity died. Triggers death sound. */
export type EntityDiedEvent = {
  type: "entityDied";
  /** Stringified objectId of the dead entity. */
  objectId: string;
};

/** An entity cast a magic spell. Triggers magic sound. */
export type MagicCastEvent = {
  type: "magicCast";
  /** Stringified objectId of the caster. */
  objectId: string;
  /** Crystal Spell enum value (used to derive the 20000+spell*10 sound id). */
  spell: number | null | undefined;
};

/** Play a raw Crystal SoundList id directly (from a PlaySound server packet). */
export type PlaySoundEvent = {
  type: "playSound";
  soundId: number;
};

/**
 * Play a semantically-named, client-initiated UI/feedback sound (character
 * created, fishing pull, level up, teleport). Distinct from `playSound` (a raw
 * server-driven id): this routes through `playOriginalSoundEvent`, which honours
 * each event's fallback chain (e.g. characterCreated -> uiButtonClick).
 */
export type UiSoundEvent = {
  type: "uiSound";
  event: OriginalSoundEvent;
};

/** The player gained gold (triggers the coin clink sound). */
export type GainedGoldEvent = {
  type: "gainedGold";
  amount: number;
};

/** The current map's background music changed (triggers setOriginalMusicId). */
export type MapMusicChangedEvent = {
  type: "mapMusicChanged";
  /** Crystal SoundList music id; null / 0 stops the current track. */
  musicId: number | null | undefined;
};

/**
 * A damage number should be displayed over an entity (Crystal DamageIndicator).
 * Carries the raw delta and Crystal DamageType so the VFX subscriber can derive
 * the text and variant without touching world state.
 */
export type DamageDealtEvent = {
  type: "damageDealt";
  /** Stringified objectId of the target. */
  objectId: string;
  /**
   * Raw HP delta from the Crystal DamageIndicator packet. Negative = damage
   * taken, positive = heal/regen. Crystal sends `armour - damage` here.
   */
  damage: number;
  /** Crystal DamageType: 0 = hit, 1 = miss, 2 = crit. */
  damageType: CrystalDamageType;
};

// ---------------------------------------------------------------------------
// Union
// ---------------------------------------------------------------------------

export type GameEvent =
  | EntityAttackEvent
  | EntityStruckEvent
  | EntityDiedEvent
  | MagicCastEvent
  | PlaySoundEvent
  | UiSoundEvent
  | GainedGoldEvent
  | MapMusicChangedEvent
  | DamageDealtEvent;

/** The `type` discriminant of every GameEvent variant. */
export type GameEventType = GameEvent["type"];

/** Narrow a GameEvent to the variant with the given type literal. */
export type GameEventOf<T extends GameEventType> = Extract<GameEvent, { type: T }>;
