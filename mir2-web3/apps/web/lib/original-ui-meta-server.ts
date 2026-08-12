import "server-only";

export type OriginalUiFrameMeta = {
  index: number;
  width: number;
  height: number;
  x: number;
  y: number;
  shadowX: number;
  shadowY: number;
  hasMask: boolean;
  maskWidth: number | null;
  maskHeight: number | null;
  path: string;
  maskPath: string | null;
};

export type OriginalUiLibraryMeta = {
  version: number;
  count: number;
  frames: OriginalUiFrameMeta[];
};

export class OriginalUiMetaError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly code: string,
  ) {
    super(message);
  }
}

export function normalizeOriginalUiLibraryKey(libraryKey: string) {
  const normalized = libraryKey
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/");

  if (
    normalized.startsWith("/") ||
    normalized.split("/").some((segment) => segment === "." || segment === "..")
  ) {
    throw new OriginalUiMetaError(`Invalid original UI library: ${libraryKey}`, 400, "invalid_library_path");
  }

  if (!normalized || normalized.startsWith("Map/")) {
    throw new OriginalUiMetaError(`Unsupported original UI library: ${normalized}`, 400, "unsupported_library");
  }

  return normalized;
}

export async function readStaticOriginalUiLibraryMeta(
  request: Request,
  normalizedLibrary: string,
): Promise<OriginalUiLibraryMeta | null> {
  const encodedPath = normalizedLibrary
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
  const localUrl = new URL(`/original-ui/${encodedPath}/meta.json`, request.url).toString();

  // Same-origin metadata is regenerated during local asset repair/builds. Do not let
  // Next's server-side fetch cache keep serving the previous truncated (often 80-frame)
  // payload after the JSON on disk has become complete. Remote release metadata remains
  // immutable and can still use the force-cache path below.
  const local = await readOriginalUiMetaUrl(localUrl, "no-store");
  // A complete same-origin meta wins outright — no R2 round-trip.
  if (local && isCompleteOriginalUiMeta(local)) {
    return local;
  }

  // The same-origin meta is missing or TRUNCATED (frames.length < count). The kept actor
  // libraries (CArmour/Monster/...) ship only their movement frames same-origin; the action
  // frames (attack/struck/die/dead) live on the R2 release. Prefer the most complete remote
  // meta — the frame PNGs themselves are backfilled by the asset Service Worker's R2 fallback.
  let best = local;
  for (const url of uniqueUrls(remoteOriginalUiMetaUrls(encodedPath))) {
    const remote = await readOriginalUiMetaUrl(url, "force-cache");
    if (remote && (!best || remote.frames.length > best.frames.length)) {
      best = remote;
      if (isCompleteOriginalUiMeta(best)) break;
    }
  }

  return best;
}

function isCompleteOriginalUiMeta(meta: OriginalUiLibraryMeta) {
  return meta.frames.length >= meta.count;
}

function remoteOriginalUiMetaUrls(encodedPath: string) {
  return [
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL,
    process.env.MIR2_ASSET_BASE_URL,
  ]
    .map(normalizeAssetBaseUrl)
    .filter(Boolean)
    .map((baseUrl) => `${baseUrl}/original-ui/${encodedPath}/meta.json`);
}

function normalizeAssetBaseUrl(value: string | undefined) {
  return value?.trim().replace(/\/+$/, "") ?? "";
}

function uniqueUrls(urls: string[]) {
  return Array.from(new Set(urls.filter(Boolean)));
}

async function readOriginalUiMetaUrl(url: string, cache: RequestCache) {
  try {
    const response = await fetch(url, { cache });
    if (!response.ok) {
      return null;
    }
    const meta = (await response.json()) as OriginalUiLibraryMeta;
    return isOriginalUiLibraryMeta(meta) ? meta : null;
  } catch {
    return null;
  }
}

function isOriginalUiLibraryMeta(value: OriginalUiLibraryMeta) {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof value.version === "number" &&
    typeof value.count === "number" &&
    Array.isArray(value.frames)
  );
}
