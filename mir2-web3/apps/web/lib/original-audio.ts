"use client";

import { crystalSoundPath } from "./original-sound-index";
import { ORIGINAL_SOUND_EVENT_FALLBACKS, type OriginalSoundEvent } from "./original-sound-events";

const DEFAULT_MUSIC_VOLUME = 0.72;
const DEFAULT_EFFECT_VOLUME = 0.86;
const MAX_SIMULTANEOUS_EFFECTS = 8;
const ORIGINAL_AUDIO_SETTINGS_STORAGE_KEY = "mir2.originalAudioSettings";
const AUDIO_DEBUG_STORAGE_KEY = "mir2-audio-debug";

export type OriginalAudioSettings = {
  musicEnabled: boolean;
  effectsEnabled: boolean;
  /** Background music gain, 0..1. */
  musicVolume: number;
  /** Sound-effect gain, 0..1. */
  effectsVolume: number;
};

const DEFAULT_ORIGINAL_AUDIO_SETTINGS: OriginalAudioSettings = {
  musicEnabled: false,
  effectsEnabled: false,
  musicVolume: DEFAULT_MUSIC_VOLUME,
  effectsVolume: DEFAULT_EFFECT_VOLUME,
};

function clampVolume(value: unknown, fallback: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    return fallback;
  }
  return Math.min(1, Math.max(0, value));
}

let musicAudio: HTMLAudioElement | null = null;
let activeMusicSrc: string | null = null;
let pendingMusicSrc: string | null = null;
const activeEffects = new Set<HTMLAudioElement>();
const audioSettingsListeners = new Set<(settings: OriginalAudioSettings) => void>();
let audioSettings = { ...DEFAULT_ORIGINAL_AUDIO_SETTINGS };
let audioSettingsLoaded = false;

export function setOriginalMusic(src: string | null) {
  pendingMusicSrc = src;
  if (typeof window === "undefined") {
    return;
  }

  if (!src || !loadOriginalAudioSettings().musicEnabled) {
    musicAudio?.pause();
    if (!src) {
      activeMusicSrc = null;
    }
    return;
  }

  const audio = musicAudio ?? new Audio();
  musicAudio = audio;
  if (activeMusicSrc !== src) {
    audio.pause();
    audio.src = src;
    audio.currentTime = 0;
    activeMusicSrc = src;
  }
  audio.loop = true;
  audio.volume = loadOriginalAudioSettings().musicVolume;
  void audio.play().catch(() => undefined);
}

export function unlockOriginalAudio() {
  setOriginalMusic(pendingMusicSrc);
}

// Loop the background music for a Crystal SoundList music id (e.g. a map's `music` field). A null
// / 0 / unresolved id stops the current track, matching maps that declare no music.
export function setOriginalMusicId(sound: number | string | null | undefined) {
  const soundId = Number(sound);
  const path = Number.isFinite(soundId) && soundId > 0 ? crystalSoundPath(soundId) : null;
  setOriginalMusic(path);
}

export function stopOriginalAudio() {
  musicAudio?.pause();
  musicAudio = null;
  activeMusicSrc = null;
  pendingMusicSrc = null;
  for (const effect of activeEffects) {
    effect.pause();
  }
  activeEffects.clear();
}

export function playOriginalSoundPath(src: string | null | undefined, volume?: number) {
  const settings = loadOriginalAudioSettings();
  if (!src || typeof window === "undefined" || !settings.effectsEnabled) {
    return false;
  }

  if (activeEffects.size >= MAX_SIMULTANEOUS_EFFECTS) {
    const oldest = activeEffects.values().next().value as HTMLAudioElement | undefined;
    oldest?.pause();
    if (oldest) {
      activeEffects.delete(oldest);
    }
  }

  const audio = new Audio(src);
  audio.volume = clampVolume(volume, settings.effectsVolume);
  activeEffects.add(audio);
  const cleanup = () => activeEffects.delete(audio);
  audio.addEventListener("ended", cleanup, { once: true });
  audio.addEventListener("error", cleanup, { once: true });
  void audio.play().catch(cleanup);
  return true;
}

export function playOriginalSoundId(sound: number | string | null | undefined, volume?: number) {
  const path = crystalSoundPath(sound);
  if (!path) {
    recordMissingSound(sound);
    return false;
  }
  return playOriginalSoundPath(path, volume);
}

// Play the first sound in `ids` whose audio is actually present, so a missing preferred sound
// degrades to a fallback instead of silence. Returns false (and records a miss) if none resolve.
export function playOriginalSoundIdWithFallback(ids: Array<number | string>, volume?: number) {
  for (const id of ids) {
    const path = crystalSoundPath(id);
    if (path) {
      return playOriginalSoundPath(path, volume);
    }
  }
  if (ids.length > 0) {
    recordMissingSound(ids[0]);
  }
  return false;
}

// Play a semantically-named sound event, walking its configured fallback chain.
export function playOriginalSoundEvent(event: OriginalSoundEvent, volume?: number) {
  return playOriginalSoundIdWithFallback(ORIGINAL_SOUND_EVENT_FALLBACKS[event], volume);
}

const missingSoundRequests = new Map<number, number>();
let missingSoundTotal = 0;

export type OriginalAudioDiagnostics = {
  missingSoundTotal: number;
  uniqueMissingSoundIds: number;
  missingSoundIds: number[];
};

export function getOriginalAudioDiagnostics(): OriginalAudioDiagnostics {
  return {
    missingSoundTotal,
    uniqueMissingSoundIds: missingSoundRequests.size,
    missingSoundIds: [...missingSoundRequests.keys()].sort((left, right) => left - right),
  };
}

export function resetOriginalAudioDiagnosticsForTests() {
  missingSoundRequests.clear();
  missingSoundTotal = 0;
}

function recordMissingSound(sound: number | string | null | undefined) {
  const soundId = Number(sound);
  if (!Number.isFinite(soundId)) {
    return;
  }
  missingSoundTotal += 1;
  const nextCount = (missingSoundRequests.get(soundId) ?? 0) + 1;
  missingSoundRequests.set(soundId, nextCount);
  if (typeof window !== "undefined") {
    (window as typeof window & { __mir2AudioDiagnostics?: OriginalAudioDiagnostics }).__mir2AudioDiagnostics =
      getOriginalAudioDiagnostics();
    if (nextCount === 1 && audioDebugEnabled()) {
      console.warn(`[mir2] sound asset missing for id ${soundId} (bytes not committed); skipped playback`);
    }
  }
}

function audioDebugEnabled() {
  if (typeof window === "undefined") {
    return false;
  }
  try {
    return window.localStorage.getItem(AUDIO_DEBUG_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function getOriginalAudioSettings() {
  return loadOriginalAudioSettings();
}

export function setOriginalAudioSettings(settings: Partial<OriginalAudioSettings>) {
  const currentSettings = loadOriginalAudioSettings();
  const nextSettings: OriginalAudioSettings = {
    musicEnabled:
      typeof settings.musicEnabled === "boolean" ? settings.musicEnabled : currentSettings.musicEnabled,
    effectsEnabled:
      typeof settings.effectsEnabled === "boolean" ? settings.effectsEnabled : currentSettings.effectsEnabled,
    musicVolume:
      settings.musicVolume === undefined
        ? currentSettings.musicVolume
        : clampVolume(settings.musicVolume, currentSettings.musicVolume),
    effectsVolume:
      settings.effectsVolume === undefined
        ? currentSettings.effectsVolume
        : clampVolume(settings.effectsVolume, currentSettings.effectsVolume),
  };

  audioSettings = nextSettings;
  audioSettingsLoaded = true;
  persistOriginalAudioSettings(nextSettings);

  if (!nextSettings.musicEnabled) {
    musicAudio?.pause();
  } else {
    if (musicAudio) {
      musicAudio.volume = nextSettings.musicVolume;
    }
    if (pendingMusicSrc) {
      setOriginalMusic(pendingMusicSrc);
    }
  }

  if (!nextSettings.effectsEnabled) {
    for (const effect of activeEffects) {
      effect.pause();
    }
    activeEffects.clear();
  }

  for (const listener of audioSettingsListeners) {
    listener(nextSettings);
  }

  return nextSettings;
}

export function subscribeOriginalAudioSettings(listener: (settings: OriginalAudioSettings) => void) {
  audioSettingsListeners.add(listener);
  listener(loadOriginalAudioSettings());
  return () => {
    audioSettingsListeners.delete(listener);
  };
}

function loadOriginalAudioSettings() {
  if (audioSettingsLoaded || typeof window === "undefined") {
    return audioSettings;
  }

  audioSettingsLoaded = true;
  try {
    const rawSettings = window.localStorage.getItem(ORIGINAL_AUDIO_SETTINGS_STORAGE_KEY);
    if (!rawSettings) {
      return audioSettings;
    }

    const parsed = JSON.parse(rawSettings) as unknown;
    if (!parsed || typeof parsed !== "object") {
      return audioSettings;
    }

    const storedSettings = parsed as Partial<Record<keyof OriginalAudioSettings, unknown>>;
    audioSettings = {
      musicEnabled:
        typeof storedSettings.musicEnabled === "boolean"
          ? storedSettings.musicEnabled
          : DEFAULT_ORIGINAL_AUDIO_SETTINGS.musicEnabled,
      effectsEnabled:
        typeof storedSettings.effectsEnabled === "boolean"
          ? storedSettings.effectsEnabled
          : DEFAULT_ORIGINAL_AUDIO_SETTINGS.effectsEnabled,
      // Volume fields were added later; older persisted settings omit them and fall back to the
      // defaults (settings migration).
      musicVolume: clampVolume(storedSettings.musicVolume, DEFAULT_ORIGINAL_AUDIO_SETTINGS.musicVolume),
      effectsVolume: clampVolume(storedSettings.effectsVolume, DEFAULT_ORIGINAL_AUDIO_SETTINGS.effectsVolume),
    };
  } catch {
    audioSettings = { ...DEFAULT_ORIGINAL_AUDIO_SETTINGS };
  }

  return audioSettings;
}

function persistOriginalAudioSettings(settings: OriginalAudioSettings) {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(ORIGINAL_AUDIO_SETTINGS_STORAGE_KEY, JSON.stringify(settings));
  } catch {
    // Audio still follows the in-memory settings when storage is unavailable.
  }
}
