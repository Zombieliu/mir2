"use client";

import { crystalSoundPath } from "./original-sound-index";

const ORIGINAL_MUSIC_VOLUME = 0.72;
const ORIGINAL_EFFECT_VOLUME = 0.86;
const MAX_SIMULTANEOUS_EFFECTS = 8;

let musicAudio: HTMLAudioElement | null = null;
let activeMusicSrc: string | null = null;
let pendingMusicSrc: string | null = null;
const activeEffects = new Set<HTMLAudioElement>();

export function setOriginalMusic(src: string | null) {
  pendingMusicSrc = src;
  if (typeof window === "undefined") {
    return;
  }

  if (!src) {
    musicAudio?.pause();
    activeMusicSrc = null;
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
  if (!src || typeof window === "undefined") {
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
