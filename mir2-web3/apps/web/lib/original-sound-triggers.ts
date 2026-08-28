"use client";

// Crystal's client-side sound triggers, ported from Client/MirObjects/MonsterObject.cs and
// PlayerObject.cs. In Crystal the CLIENT (not the server) plays most combat/movement sounds from
// its own animation code; the server only sends a few scripted sounds via the PlaySound packet.
// This module reproduces that: page.tsx calls these helpers from the object-action packet
// handlers (ObjectAttack / ObjectStruck / ObjectDied / ObjectMagic / Struck …) and they compute
// the Crystal sound id and hand it to the presence-aware resolver in original-audio.ts.
//
// Coverage note: generic combat sounds use SoundList constants. Audited arithmetic ids such as
// Scarecrow death (BaseImage 5 * 10 + 3 => 005-3.wav) are exported as explicit direct entries.
// Other per-monster ids remain silent until their exact Crystal files enter that audited set.

import { ORIGINAL_SOUND_IDS } from "./original-sound-events";
import { playOriginalSoundId, playOriginalSoundIdWithFallback } from "./original-audio";
import { spellNumberForName } from "./crystal-magic-effects";

// MonsterObject.cs: BaseSound = (ushort)BaseImage * 10, then a per-action offset.
//   PlayAttackSound -> BaseSound + 1   (the monster's attack roar)
//   PlaySwingSound  -> BaseSound + 4   (the weapon swoosh)
//   PlayDieSound    -> BaseSound + 3   (the death cry)
// Monster *struck* is weapon-based (SoundList.Struck*), not image-based — see below.
const MONSTER_ATTACK_OFFSET = 1;
const MONSTER_SWING_OFFSET = 4;
const MONSTER_DIE_OFFSET = 3;
const FLAMING_SWORD_SPELL_ID = 8;
const FLAMING_SWORD_ATTACK_SOUND_ID = 20081;
type PendingEntitySound = {
  objectId: string | null;
  kind: "attack" | "death";
};
const pendingEntitySoundTimers = new Map<ReturnType<typeof setTimeout>, PendingEntitySound>();

/** Cancel every delayed combat cue owned by an actor/session lifetime. */
export function cancelPendingEntitySounds(objectId?: string | null): void {
  for (const [timer, pending] of pendingEntitySoundTimers) {
    if (objectId !== undefined && pending.objectId !== objectId) continue;
    globalThis.clearTimeout(timer);
    pendingEntitySoundTimers.delete(timer);
  }
}

/** Cancel delayed animation-frame attack sounds when their actor/session lifetime ends. */
export function cancelPendingEntityAttackSounds(objectId?: string | null): void {
  for (const [timer, pending] of pendingEntitySoundTimers) {
    if (pending.kind !== "attack") continue;
    if (objectId !== undefined && pending.objectId !== objectId) continue;
    globalThis.clearTimeout(timer);
    pendingEntitySoundTimers.delete(timer);
  }
}

function scheduleEntityAttackSound(soundId: number, delayMs: number, objectId?: string | null): void {
  const timer = globalThis.setTimeout(() => {
    pendingEntitySoundTimers.delete(timer);
    playOriginalSoundId(soundId);
  }, delayMs);
  pendingEntitySoundTimers.set(timer, { objectId: objectId ?? null, kind: "attack" });
}

export type SoundEntityKind = "selfPlayer" | "player" | "monster" | "npc";

/** The minimal entity shape the sound triggers read (a structural subset of WorldEntity). */
export type SoundEntityRef = {
  kind: SoundEntityKind;
  sprite?: {
    bodyLibrary?: string | null;
    weaponLibrary?: string | null;
    mountLibrary?: string | null;
  } | null;
  genderKey?: string | null;
  classKey?: string | null;
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
  const normalized = bodyLibrary
    .trim()
    .replaceAll("\\", "/")
    .replace(/^\/?original-ui\//i, "")
    .replace(/^\/+/, "");
  const match = /^Monster\/0*(\d+)(?:\/|$)/i.exec(normalized);
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

function playerLibraryIndex(library: string | null | undefined): number | null {
  const match = /\/0*(\d+)(?:\D|$)/.exec(String(library ?? ""));
  if (!match) return null;
  const value = Number(match[1]);
  return Number.isInteger(value) ? value : null;
}

function playerWeaponIndex(entity: SoundEntityRef | null | undefined): number {
  if (!entity || !isPlayer(entity.kind)) return -2;
  const weapon = playerLibraryIndex(entity.sprite?.weaponLibrary) ?? -1;
  return String(entity.classKey ?? "").toLowerCase() === "assassin" && weapon !== -1 ? 1 : weapon;
}

function playerArmourSoundOffset(entity: SoundEntityRef): number {
  if (String(entity.classKey ?? "").toLowerCase() === "assassin") return 0;
  const armour = playerLibraryIndex(entity.sprite?.bodyLibrary);
  return armour === 3 || armour === 6 || armour === 9 ? 10 : 0;
}

function playerStruckBodySoundId(target: SoundEntityRef, attacker: SoundEntityRef | null | undefined): number | null {
  const weapon = playerWeaponIndex(attacker);
  const add = playerArmourSoundOffset(target);
  if ([0, 1, 2, 3, 5, 7, 8, 9, 11, 12, 13, 15, 18, 19, 20, 23, 24, 25, 26, 28, 29, 31, 32, 33, 34, 35, 37, 40, 41].includes(weapon)) {
    return ORIGINAL_SOUND_IDS.struckBodySword + add;
  }
  if ([4, 14, 16, 38].includes(weapon)) return ORIGINAL_SOUND_IDS.struckBodyAxe + add;
  if ([6, 10, 17, 21, 22, 27, 30, 36, 39].includes(weapon)) {
    return ORIGINAL_SOUND_IDS.struckBodyLongStick + add;
  }
  if (weapon === -1) return ORIGINAL_SOUND_IDS.struckBodyFist + add;
  return null;
}

/** Attack swing / roar for an object that just attacked (ObjectAttack / ObjectRangeAttack). */
export function playEntityAttackSound(
  entity: SoundEntityRef | null | undefined,
  spell?: number | string | null,
  objectId?: string | null,
): void {
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
    // Crystal sets the FlamingSword-specific cue when Attack1 begins, then
    // plays the weapon swing on attack frame 1 (100 ms later). SpellToggle
    // itself is silent and never calls this path.
    if (spell === FLAMING_SWORD_SPELL_ID || spell === "FlamingSword") {
      playOriginalSoundId(FLAMING_SWORD_ATTACK_SOUND_ID);
      scheduleEntityAttackSound(ORIGINAL_SOUND_IDS.swingSword, 100, objectId);
      return;
    }
    // Player melee swing — weapon class is not tracked client-side, so use the common sword swing.
    playOriginalSoundId(ORIGINAL_SOUND_IDS.swingSword);
  }
}

/** Hit reaction for an object that was just struck (ObjectStruck / Struck). */
export function playEntityStruckSound(
  target: SoundEntityRef | null | undefined,
  attacker?: SoundEntityRef | null,
): void {
  if (!target) {
    return;
  }
  if (target.kind === "monster") {
    // MonsterObject.PlayStruckSound: weapon clang (default StruckSword) — always present.
    playOriginalSoundId(ORIGINAL_SOUND_IDS.struckSword);
    return;
  }
  if (isPlayer(target.kind)) {
    // RidingMount returns from PlayerObject.PlayStruckSound before the normal
    // weapon/armour body-hit switch. PlayFlinchSound is called separately and
    // still follows the mount cue.
    if (target.sprite?.mountLibrary) {
      const mountType = playerLibraryIndex(target.sprite.mountLibrary);
      if (mountType !== null && mountType < 7) {
        playOriginalSoundId(
          Math.random() < 0.5
            ? ORIGINAL_SOUND_IDS.mountStruckTiger1
            : ORIGINAL_SOUND_IDS.mountStruckTiger2,
        );
      } else if (mountType !== null && mountType < 12) {
        playOriginalSoundId(ORIGINAL_SOUND_IDS.mountStruckWolf);
      }
      playOriginalSoundId(isFemale(target) ? ORIGINAL_SOUND_IDS.femaleFlinch : ORIGINAL_SOUND_IDS.maleFlinch);
      return;
    }
    // PlayerObject.PlayStruckSound resolves the attacking player's weapon and
    // the target's armour class. A non-player/unknown attacker leaves
    // StruckWeapon=-2 and intentionally has no body-hit sample.
    const bodySound = playerStruckBodySoundId(target, attacker);
    if (bodySound !== null) playOriginalSoundId(bodySound);
    // PlayerObject always follows PlayStruckSound with PlayFlinchSound.
    playOriginalSoundId(isFemale(target) ? ORIGINAL_SOUND_IDS.femaleFlinch : ORIGINAL_SOUND_IDS.maleFlinch);
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

/** Crystal MonsterObject plays at Die action start; PlayerObject waits for Die frame 1 (100 ms). */
export function scheduleEntityDieSound(
  entity: SoundEntityRef | null | undefined,
  objectId?: string | null,
): void {
  if (!entity) return;
  if (entity.kind === "monster") {
    playEntityDieSound(entity);
    return;
  }
  const timer = globalThis.setTimeout(() => {
    pendingEntitySoundTimers.delete(timer);
    playEntityDieSound(entity);
  }, 100);
  pendingEntitySoundTimers.set(timer, { objectId: objectId ?? null, kind: "death" });
}

// PlayerObject.cs / MonsterObject.cs magic: SoundManager.PlaySound(20000 + (ushort)Spell * 10 [+ variant]).
// Most 20000-range ids are not in the current SoundList, so these resolve to null and are skipped
// (graceful) — kept for parity completeness and forward-compatibility with a fuller SoundList.
export function playMagicSoundId(
  spell: number | string | null | undefined,
  variantOne = false,
): boolean {
  const spellId = typeof spell === "string" ? spellNumberForName(spell) : Number(spell);
  if (spellId === null || !Number.isFinite(spellId) || spellId <= 0) {
    return false;
  }
  return playOriginalSoundId(20000 + Number(spellId) * 10 + (variantOne ? 1 : 0));
}
