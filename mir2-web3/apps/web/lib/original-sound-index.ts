import soundIndex from "../public/original-ui/sound-index.generated.json";

type SoundIndexEntry = {
  fileName: string;
  path: string | null;
  sourceExists: boolean;
  sourceBytes: number;
};

type SoundIndexPayload = {
  sounds?: Record<string, SoundIndexEntry>;
};

const sounds = (soundIndex as SoundIndexPayload).sounds ?? {};

export function crystalSoundPath(sound: number | string | null | undefined): string | null {
  const soundId = Number(sound);
  if (!Number.isFinite(soundId)) {
    return null;
  }

  const entry = sounds[String(soundId)];
  if (entry?.sourceExists && entry.path) {
    return entry.path;
  }

  return null;
}

export function crystalSoundExists(sound: number | string | null | undefined) {
  return crystalSoundPath(sound) !== null;
}
