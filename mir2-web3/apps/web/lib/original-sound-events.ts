// Semantic names for the Crystal sound IDs the client triggers itself (server-driven sounds
// arrive by raw id via the PlaySound packet). IDs follow the Crystal SoundList.lst numbering and
// the named constants in Client/MirSounds/SoundList.cs.
//
// The full Crystal sound library (the 320 distinct .wav files the 450 SoundList ids reference) is
// published to R2 and served at /original-ui/Sound/..., so every id below resolves in production.
// This registry gives callers a typed handle plus an ordered fallback chain, so a preferred sound
// that has no SoundList entry degrades to a present one (or is skipped cleanly) instead of being
// hard-coded at each call site. Resolution and graceful "missing" handling live in
// original-audio.ts; the per-action id formulas (monster/player combat) live in
// original-sound-triggers.ts.
export const ORIGINAL_SOUND_IDS = {
  // --- UI / system (SoundList.cs) ---
  /** UI button press — the canonical interface click (100.wav / LoginEffect). */
  uiButtonClick: 10100,
  buttonA: 10103,
  buttonB: 10104,
  buttonC: 10105,
  /** Gold gained / coin sound (106.wav). */
  gold: 10106,
  eatDrug: 10107,
  clickDrug: 10108,
  /** Teleport whoosh (110.wav). */
  teleport: 10110,
  clickWeapon: 10111,
  clickArmour: 10112,
  clickRing: 10113,
  clickBracelet: 10114,
  clickNecklace: 10115,
  clickHelmet: 10116,
  clickBoots: 10117,
  clickItem: 10118,
  /** Played once when the player gains a level (156.wav). */
  levelUp: 10156,

  // --- Music (looped via setOriginalMusic, not as one-shot effects) ---
  /** Looping login-screen background music (Login2.wav). */
  loginScreenMusic: 10146,
  /** Looping character-select background music (Select2.wav). */
  characterSelectMusic: 10147,

  // --- Character lifecycle ---
  /** Played once when a new character is created (NewChar.wav). */
  characterCreated: 10168,
  maleFlinch: 10138,
  femaleFlinch: 10139,
  maleDie: 10144,
  femaleDie: 10145,
  revive: 20791,

  // --- Player melee swing (weapon-class; default sword) ---
  swingShort: 10050,
  swingWood: 10051,
  swingSword: 10052,
  swingSword2: 10053,
  swingAxe: 10054,
  swingClub: 10055,
  swingLong: 10056,

  // --- Struck-by-weapon clang (monster-struck uses these) ---
  struckShort: 10060,
  struckWooden: 10061,
  struckSword: 10062,
  struckSword2: 10063,
  struckAxe: 10064,
  struckClub: 10065,
  /** Player-body flesh hit (default for a struck player). */
  struckBodySword: 10070,
  struckBodyAxe: 10071,
  struckBodyLongStick: 10072,
  struckBodyFist: 10073,

  // --- Mounts ---
  mountWalkL: 10176,
  mountWalkR: 10177,
  mountRun: 10178,

  // --- Fishing ---
  fishingThrow: 10184,
  fishingPull: 10185,
  fishing: 10186,
} as const;

export type OriginalSoundEvent = keyof typeof ORIGINAL_SOUND_IDS;

// Ordered fallback chains: the first id whose audio resolves is played. Most events are a single
// entry; a few graft a related clip so a preferred-but-unlisted sound still produces something.
const EXPLICIT_FALLBACKS: Partial<Record<OriginalSoundEvent, number[]>> = {
  characterCreated: [ORIGINAL_SOUND_IDS.characterCreated, ORIGINAL_SOUND_IDS.uiButtonClick],
};

export const ORIGINAL_SOUND_EVENT_FALLBACKS: Record<OriginalSoundEvent, number[]> = Object.fromEntries(
  (Object.keys(ORIGINAL_SOUND_IDS) as OriginalSoundEvent[]).map((event) => [
    event,
    EXPLICIT_FALLBACKS[event] ?? [ORIGINAL_SOUND_IDS[event]],
  ]),
) as Record<OriginalSoundEvent, number[]>;
