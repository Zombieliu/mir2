"use client";

// Crystal's client-side sound triggers, ported from Client/MirObjects/MonsterObject.cs and
// PlayerObject.cs. In Crystal the CLIENT (not the server) plays most combat/movement sounds from
// its own animation code; the server only sends a few scripted sounds via the PlaySound packet.
// This module reproduces that: page.tsx calls these helpers from the object-action packet
// handlers (ObjectAttack / ObjectStruck / ObjectDied / ObjectMagic / Struck …) and they compute
// the Crystal sound id and hand it to the presence-aware resolver in original-audio.ts.
//
// Coverage note: generic combat sounds (player swing/struck/die, monster struck clang) use the
// always-present SoundList.cs weapon constants, so they always sound. Per-monster roars
// (image*10 + offset) sound only when that monster has a SoundList entry — exactly as in Crystal,
// where monsters without a sound entry are silent. Unlisted ids resolve to null and are skipped.

import { ORIGINAL_SOUND_IDS } from "./original-sound-events";
import { playOriginalSoundId, playOriginalSoundIdWithFallback } from "./original-audio";

// MonsterObject.cs: BaseSound = (ushort)BaseImage * 10, then a per-action offset.
//   PlayAttackSound -> BaseSound + 1   (the monster's attack roar)
//   PlaySwingSound  -> BaseSound + 4   (the weapon swoosh)
//   PlayDieSound    -> BaseSound + 3   (the death cry)
// Monster *struck* is weapon-based (SoundList.Struck*), not image-based — see below.
const MONSTER_ATTACK_OFFSET = 1;
const MONSTER_SWING_OFFSET = 4;
const MONSTER_DIE_OFFSET = 3;

export type SoundEntityKind = "selfPlayer" | "player" | "monster" | "npc";

/** The minimal entity shape the sound triggers read (a structural subset of WorldEntity). */
export type SoundEntityRef = {
  kind: SoundEntityKind;
  sprite?: { bodyLibrary?: string | null } | null;
  genderKey?: string | null;
};

function isPlayer(kind: SoundEntityKind): boolean {
  return kind === "player" || kind === "selfPlayer";
}

function isFemale(entity: SoundEntityRef): boolean {
  return String(entity.genderKey ?? "").toLowerCase() === "female";
}

/** Parse the Crystal monster image index from a body library key, e.g. "Monster/042" -> 42. */
export function monsterImageFromBodyLibrary(bodyLibrary: string | null | undefined): number | null {
  if (!bodyLibrary) {
    return null;
  }
  const match = /^Monster\/0*(\d+)/i.exec(bodyLibrary);
  if (!match) {
    return null;
  }
  const image = Number(match[1]);
  return Number.isFinite(image) ? image : null;
}

function monsterBaseSound(entity: SoundEntityRef): number | null {
  const image = monsterImageFromBodyLibrary(entity.sprite?.bodyLibrary);
  return image === null ? null : image * 10;
}

/** Attack swing / roar for an object that just attacked (ObjectAttack / ObjectRangeAttack). */
export function playEntityAttackSound(entity: SoundEntityRef | null | undefined): void {
  if (!entity) {
    return;
  }
  if (entity.kind === "monster") {
    const base = monsterBaseSound(entity);
    if (base !== null) {
      // Prefer the attack roar; fall back within the monster's own id block, then the swing.
      playOriginalSoundIdWithFallback([base + MONSTER_ATTACK_OFFSET, base + MONSTER_SWING_OFFSET]);
    }
    return;
  }
  if (isPlayer(entity.kind)) {
    // Player melee swing — weapon class is not tracked client-side, so use the common sword swing.
    playOriginalSoundId(ORIGINAL_SOUND_IDS.swingSword);
  }
}

/** Hit reaction for an object that was just struck (ObjectStruck / Struck). */
export function playEntityStruckSound(entity: SoundEntityRef | null | undefined): void {
  if (!entity) {
    return;
  }
  if (entity.kind === "monster") {
    // MonsterObject.PlayStruckSound: weapon clang (default StruckSword) — always present.
    playOriginalSoundId(ORIGINAL_SOUND_IDS.struckSword);
    return;
  }
  if (isPlayer(entity.kind)) {
    // PlayerObject.PlayStruckSound default: flesh hit on the body (StruckBodySword).
    playOriginalSoundId(ORIGINAL_SOUND_IDS.struckBodySword);
  }
}

/** Death cry for an object that just died (ObjectDied / Death). */
export function playEntityDieSound(entity: SoundEntityRef | null | undefined): void {
  if (!entity) {
    return;
  }
  if (entity.kind === "monster") {
    const base = monsterBaseSound(entity);
    if (base !== null) {
      playOriginalSoundId(base + MONSTER_DIE_OFFSET);
    }
    return;
  }
  if (isPlayer(entity.kind)) {
    playOriginalSoundId(isFemale(entity) ? ORIGINAL_SOUND_IDS.femaleDie : ORIGINAL_SOUND_IDS.maleDie);
  }
}

// PlayerObject.cs / MonsterObject.cs magic: SoundManager.PlaySound(20000 + (ushort)Spell * 10 [+ variant]).
// Most 20000-range ids are not in the current SoundList, so these resolve to null and are skipped
// (graceful) — kept for parity completeness and forward-compatibility with a fuller SoundList.
export function playMagicSoundId(spell: number | null | undefined, genderVariant = false): boolean {
  const spellId = Number(spell);
  if (!Number.isFinite(spellId) || spellId <= 0) {
    return false;
  }
  return playOriginalSoundId(20000 + spellId * 10 + (genderVariant ? 1 : 0));
}
