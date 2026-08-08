import presentSoundManifest from "./generated/crystal-present-sounds.generated.json";
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

type PresentSoundManifest = {
  files?: string[];
};

const sounds = (soundIndex as SoundIndexPayload).sounds ?? {};

// The committed Crystal sound index marks ~450 sounds as existing because it was generated
// against a full client export, but only a few .wav files are committed to the repo (the full
// set is large and gated on the raw Crystal client). Resolve paths against the manifest of
// sounds actually present so the client never tries to play bytes that would 404, and so the
// audio diagnostics report truthful coverage. Lower-cased for case-robust matching.
const presentSoundFiles = new Set(
  ((presentSoundManifest as PresentSoundManifest).files ?? []).map((name) => name.toLowerCase()),
);

function remoteAssetBaseUrl(): string {
  return (process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? "").trim().replace(/\/+$/, "");
}

function remoteBackedSoundPath(publicPath: string): string {
  const baseUrl = remoteAssetBaseUrl();
  if (!baseUrl) {
    return publicPath;
  }

  try {
    return new URL(publicPath.replace(/^\/+/, ""), `${baseUrl}/`).href;
  } catch {
    return publicPath;
  }
}

function publicPathIsPresent(publicPath: string | null | undefined): boolean {
  if (!publicPath) {
    return false;
  }
  const base = publicPath.split("/").pop()?.toLowerCase();
  return base ? presentSoundFiles.has(base) : false;
}

export function crystalSoundIndexEntry(sound: number | string | null | undefined): SoundIndexEntry | null {
  const soundId = Number(sound);
  if (!Number.isFinite(soundId)) {
    return null;
  }
  return sounds[String(soundId)] ?? null;
}

export function crystalSoundPath(sound: number | string | null | undefined): string | null {
  const entry = crystalSoundIndexEntry(sound);
  if (entry?.sourceExists && entry.path && publicPathIsPresent(entry.path)) {
    return remoteBackedSoundPath(entry.path);
  }
  return null;
}

export function crystalSoundExists(sound: number | string | null | undefined) {
  return crystalSoundPath(sound) !== null;
}

// True when the index references this sound (so it is a real, known sound) but its audio bytes
// are not committed to the repo — i.e. it is blocked on the raw Crystal client, not a bug.
export function crystalSoundIsMissingAsset(sound: number | string | null | undefined): boolean {
  const entry = crystalSoundIndexEntry(sound);
  if (!entry || !entry.path) {
    return false;
  }
  return !publicPathIsPresent(entry.path);
}

export function crystalPresentSoundCount(): number {
  return presentSoundFiles.size;
}

export function crystalIndexedSoundCount(): number {
  return Object.keys(sounds).length;
}
