// Semantic names for the Crystal sound IDs the client triggers itself (server-driven sounds
// arrive by raw id via the PlaySound packet). IDs follow the Crystal SoundList.lst numbering.
//
// Only four sound .wav files are currently committed (ids 10100 / 10146 / 10147 / 10168); the
// rest of the library is raw-asset-limited. This registry documents intent in one place and
// gives callers a typed handle plus an ordered fallback chain, so a missing preferred sound can
// degrade to a present one (or be skipped cleanly) instead of being hard-coded at each call
// site. Resolution and graceful "missing" handling live in original-audio.ts.
export const ORIGINAL_SOUND_IDS = {
  /** UI button press — the canonical interface click (100.wav). */
  uiButtonClick: 10100,
  /** Looping login-screen background music (Login2.wav). */
  loginScreenMusic: 10146,
  /** Looping character-select background music (Select2.wav). */
  characterSelectMusic: 10147,
  /** Played once when a new character is created (NewChar.wav). */
  characterCreated: 10168,
} as const;

export type OriginalSoundEvent = keyof typeof ORIGINAL_SOUND_IDS;

// Ordered fallback chains: the first id whose audio is present is played. Kept conservative so
// no event silently borrows an unrelated clip; today most chains have a single entry because
// only four sounds are committed.
export const ORIGINAL_SOUND_EVENT_FALLBACKS: Record<OriginalSoundEvent, number[]> = {
  uiButtonClick: [ORIGINAL_SOUND_IDS.uiButtonClick],
  loginScreenMusic: [ORIGINAL_SOUND_IDS.loginScreenMusic],
  characterSelectMusic: [ORIGINAL_SOUND_IDS.characterSelectMusic],
  characterCreated: [ORIGINAL_SOUND_IDS.characterCreated, ORIGINAL_SOUND_IDS.uiButtonClick],
};
