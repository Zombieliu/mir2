// Semantic names for the Crystal sound IDs the client triggers itself (server-driven sounds
// arrive by raw id via the PlaySound packet). IDs follow the Crystal SoundList.lst numbering.
//
// Per Crystal's SoundList, the generic UI click is ButtonB (10104 / 104.wav): MirButton sets
// `Sound = SoundList.ButtonB` and MirControl.OnMouseClick plays it on every click. 100.wav is a
// different clip entirely — LoginEffect (10100), played by the login flow (ORIGINAL_AUDIO
// .loginEffect). The two must not be conflated; reusing 100.wav for clicks is what made every
// button replay the login sound.
//
// The ButtonA/B/C clips (103/104/105.wav) are raw-asset-limited (not committed), while the
// login/select/newchar wavs are present (100.wav / Login2.wav / Select2.wav / NewChar.wav). So
// uiButtonClick currently resolves to nothing and clicks stay silent until 104.wav ships — the
// faithful behavior (correct clip or silence, never the wrong clip). This registry documents
// intent in one place and gives callers a typed handle plus an ordered fallback chain, so a
// missing preferred sound degrades to a present one (or is skipped cleanly) instead of being
// hard-coded at each call site. Resolution and graceful "missing" handling live in
// original-audio.ts.
export const ORIGINAL_SOUND_IDS = {
  /** Generic UI button press — Crystal's ButtonB / the MirButton click default (104.wav). */
  uiButtonClick: 10104,
  /** Looping login-screen background music (Login2.wav). */
  loginScreenMusic: 10146,
  /** Looping character-select background music (Select2.wav). */
  characterSelectMusic: 10147,
  /** Played once when a new character is created (NewChar.wav). */
  characterCreated: 10168,
} as const;

export type OriginalSoundEvent = keyof typeof ORIGINAL_SOUND_IDS;

// Ordered fallback chains: the first id whose audio is present is played. Kept conservative so
// no event silently borrows an unrelated clip — notably uiButtonClick is ButtonB-only and must
// never fall back to the LoginEffect (10100), or every click would replay the login sound again.
export const ORIGINAL_SOUND_EVENT_FALLBACKS: Record<OriginalSoundEvent, number[]> = {
  uiButtonClick: [ORIGINAL_SOUND_IDS.uiButtonClick],
  loginScreenMusic: [ORIGINAL_SOUND_IDS.loginScreenMusic],
  characterSelectMusic: [ORIGINAL_SOUND_IDS.characterSelectMusic],
  characterCreated: [ORIGINAL_SOUND_IDS.characterCreated, ORIGINAL_SOUND_IDS.uiButtonClick],
};
