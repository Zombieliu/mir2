"use client";

import { crystalSoundPath } from "./original-sound-index";

const ORIGINAL_MUSIC_VOLUME = 0.72;
const ORIGINAL_EFFECT_VOLUME = 0.86;
const MAX_SIMULTANEOUS_EFFECTS = 8;
const ORIGINAL_AUDIO_SETTINGS_STORAGE_KEY = "mir2.originalAudioSettings";

export type OriginalAudioSettings = {
  musicEnabled: boolean;
  effectsEnabled: boolean;
};

const DEFAULT_ORIGINAL_AUDIO_SETTINGS: OriginalAudioSettings = {
  musicEnabled: true,
  effectsEnabled: true,
};

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
  audio.volume = ORIGINAL_MUSIC_VOLUME;
  void audio.play().catch(() => undefined);
}

export function unlockOriginalAudio() {
  setOriginalMusic(pendingMusicSrc);
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

export function playOriginalSoundPath(src: string | null | undefined, volume = ORIGINAL_EFFECT_VOLUME) {
  if (!src || typeof window === "undefined" || !loadOriginalAudioSettings().effectsEnabled) {
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
  audio.volume = volume;
  activeEffects.add(audio);
  const cleanup = () => activeEffects.delete(audio);
  audio.addEventListener("ended", cleanup, { once: true });
  audio.addEventListener("error", cleanup, { once: true });
  void audio.play().catch(cleanup);
  return true;
}

export function playOriginalSoundId(sound: number | string | null | undefined, volume = ORIGINAL_EFFECT_VOLUME) {
  return playOriginalSoundPath(crystalSoundPath(sound), volume);
}

export function getOriginalAudioSettings() {
  return loadOriginalAudioSettings();
}

export function setOriginalAudioSettings(settings: Partial<OriginalAudioSettings>) {
  const currentSettings = loadOriginalAudioSettings();
  const nextSettings = {
    musicEnabled:
      typeof settings.musicEnabled === "boolean" ? settings.musicEnabled : currentSettings.musicEnabled,
    effectsEnabled:
      typeof settings.effectsEnabled === "boolean" ? settings.effectsEnabled : currentSettings.effectsEnabled,
  };

  audioSettings = nextSettings;
  audioSettingsLoaded = true;
  persistOriginalAudioSettings(nextSettings);

  if (!nextSettings.musicEnabled) {
    musicAudio?.pause();
  } else if (pendingMusicSrc) {
    setOriginalMusic(pendingMusicSrc);
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
