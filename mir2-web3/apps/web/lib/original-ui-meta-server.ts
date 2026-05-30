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
  const candidates = [
    new URL(`/original-ui/${encodedPath}/meta.json`, request.url).toString(),
    ...remoteOriginalUiMetaUrls(encodedPath),
  ];

  for (const url of uniqueUrls(candidates)) {
    const meta = await readOriginalUiMetaUrl(url);
    if (meta) {
      return meta;
    }
  }

  return null;
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

async function readOriginalUiMetaUrl(url: string) {
  try {
    const response = await fetch(url, { cache: "force-cache" });
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
