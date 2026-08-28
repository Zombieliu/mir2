// Sound subscriber: maps GameEvents to the Crystal audio trigger functions.
//
// `registerSoundSubscriber(bus, audio)` wires every combat/UI sound event to the
// existing original-audio / original-sound-triggers helpers.  The `audio`
// parameter is an injected interface so the subscriber is fully testable without
// a real browser audio context.
//
// Default wiring: call `registerDefaultSoundSubscriber(bus)` to connect the real
// implementations from original-audio.ts and original-sound-triggers.ts.
//
// Design:
//   • Injected sink — no direct import of browser-only modules in the core
//     registration function.
//   • Matches the existing inline triggers in page.tsx 1:1 so the eventual
//     migration is a mechanical substitution.

import type { GameEventBus, Unsubscribe } from "./bus";
import type { OriginalSoundEvent, SoundEntityRef } from "./events";

// ---------------------------------------------------------------------------
// Injected audio interface
// ---------------------------------------------------------------------------

/**
 * Minimal interface for sound playback that the subscriber drives.
 * Matches the signatures of the existing helpers in original-audio.ts and
 * original-sound-triggers.ts, so the real implementations satisfy it
 * directly.
 */
export type AudioSink = {
  /** Play by raw Crystal SoundList id (original-audio.ts `playOriginalSoundId`). */
  playOriginalSoundId(soundId: number): void;
  /**
   * Play a semantically-named sound event with its fallback chain
   * (original-audio.ts `playOriginalSoundEvent`).
   */
  playOriginalSoundEvent(event: OriginalSoundEvent): void;
  /** Loop background music by Crystal SoundList id (original-audio.ts `setOriginalMusicId`). */
  setOriginalMusicId(musicId: number | string | null | undefined): void;
  /** Entity attack sound (original-sound-triggers.ts `playEntityAttackSound`). */
  playEntityAttackSound(
    entity: SoundEntityRef | null | undefined,
    spell?: number | string | null,
    objectId?: string | null,
  ): void;
  /** Entity struck sound (original-sound-triggers.ts `playEntityStruckSound`). */
  playEntityStruckSound(
    target: SoundEntityRef | null | undefined,
    attacker?: SoundEntityRef | null,
  ): void;
  /** Schedule the death cry at Crystal PlayerObject Die frame 1 (100 ms). */
  scheduleEntityDieSound(entity: SoundEntityRef | null | undefined, objectId?: string | null): void;
  /** Revive cue (SoundList.Revive). */
  playReviveSound(): void;
  /** Magic phase sound by spell id (variant 0 = cast, variant 1 = target/effect). */
  playMagicSoundId(spell: number | string | null | undefined, variantOne?: boolean): void;
  /** Look up a SoundEntityRef for a given objectId (from world state). */
  soundEntityRefFor(objectId: string | null | undefined): SoundEntityRef | null;
  /** Play the gold-gained sound (ORIGINAL_SOUND_IDS.gold). */
  playGoldSound(): void;
};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/**
 * Subscribe the sound-effect handlers to the bus.
 * Returns an array of unsubscribe functions so all handlers can be torn down
 * together (useful in tests and HMR).
 */
export function registerSoundSubscriber(bus: GameEventBus, audio: AudioSink): Unsubscribe {
  const unsubs: Unsubscribe[] = [];

  // entityAttack -> attack swing / roar
  unsubs.push(
    bus.on("entityAttack", (event) => {
      audio.playEntityAttackSound(audio.soundEntityRefFor(event.objectId), event.spell, event.objectId);
    }),
  );

  // entityStruck -> hit clang / body thud
  unsubs.push(
    bus.on("entityStruck", (event) => {
      audio.playEntityStruckSound(
        audio.soundEntityRefFor(event.objectId),
        audio.soundEntityRefFor(event.attackerId),
      );
    }),
  );

  // entityDied -> death cry
  unsubs.push(
    bus.on("entityDied", (event) => {
      audio.scheduleEntityDieSound(audio.soundEntityRefFor(event.objectId), event.objectId);
    }),
  );

  unsubs.push(
    bus.on("entityRevived", () => {
      audio.playReviveSound();
    }),
  );

  // magicCast -> 20000 + spell*10 sound
  unsubs.push(
    bus.on("magicCast", (event) => {
      audio.playMagicSoundId(event.spell);
    }),
  );

  // Crystal ObjectEffect.Healing -> 20000 + Spell.Healing*10 + 1.
  // Other ObjectEffect families remain silent until their source branch has
  // an independently-audited audio contract.
  unsubs.push(
    bus.on("magicEffect", (event) => {
      if (event.effect === 3) {
        audio.playMagicSoundId(61, true);
      }
    }),
  );

  // playSound -> raw SoundList id (server-driven via PlaySound packet)
  unsubs.push(
    bus.on("playSound", (event) => {
      audio.playOriginalSoundId(event.soundId);
    }),
  );

  // uiSound -> client-initiated, fallback-aware feedback sound
  unsubs.push(
    bus.on("uiSound", (event) => {
      audio.playOriginalSoundEvent(event.event);
    }),
  );

  // gainedGold -> coin clink
  unsubs.push(
    bus.on("gainedGold", (event) => {
      if (event.amount > 0) {
        audio.playGoldSound();
      }
    }),
  );

  // mapMusicChanged -> loop/stop background track
  unsubs.push(
    bus.on("mapMusicChanged", (event) => {
      audio.setOriginalMusicId(event.musicId);
    }),
  );

  return () => {
    for (const unsub of unsubs) {
      unsub();
    }
  };
}

// ---------------------------------------------------------------------------
// Default wiring (real audio modules)
// ---------------------------------------------------------------------------

/**
 * Build an AudioSink backed by the real original-audio.ts and
 * original-sound-triggers.ts implementations. The `soundEntityRefFor` lookup is
 * provided by the caller (it needs access to live world state, so it can't be
 * pre-wired here).
 */
export function makeRealAudioSink(
  soundEntityRefFor: (objectId: string | null | undefined) => SoundEntityRef | null,
): AudioSink {
  // Import lazily so this file is safe to import in test environments that stub
  // these modules (node scripts without a browser).
  const {
    playOriginalSoundId,
    playOriginalSoundEvent,
    setOriginalMusicId,
  } = require("../original-audio") as typeof import("../original-audio");
  const {
    playEntityAttackSound,
    playEntityStruckSound,
    scheduleEntityDieSound,
    playMagicSoundId,
  } = require("../original-sound-triggers") as typeof import("../original-sound-triggers");
  const { ORIGINAL_SOUND_IDS } = require("../original-sound-events") as typeof import("../original-sound-events");

  return {
    playOriginalSoundId,
    playOriginalSoundEvent,
    setOriginalMusicId,
    playEntityAttackSound,
    playEntityStruckSound,
    scheduleEntityDieSound,
    playReviveSound() {
      playOriginalSoundId(ORIGINAL_SOUND_IDS.revive);
    },
    playMagicSoundId,
    soundEntityRefFor,
    playGoldSound() {
      playOriginalSoundId(ORIGINAL_SOUND_IDS.gold);
    },
  };
}

/**
 * Convenience: wire the real audio implementations to `bus`, with `soundEntityRefFor`
 * provided by the caller. Returns the aggregate unsubscribe.
 */
export function registerDefaultSoundSubscriber(
  bus: GameEventBus,
  soundEntityRefFor: (objectId: string | null | undefined) => SoundEntityRef | null,
): Unsubscribe {
  return registerSoundSubscriber(bus, makeRealAudioSink(soundEntityRefFor));
}
