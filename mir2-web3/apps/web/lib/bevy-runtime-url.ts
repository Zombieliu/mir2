export type BevyRuntimeUrlBackend = "webgpu" | "webgl2";

export type BevyRuntimeUrls = {
  moduleUrl: string;
  wasmUrl: string;
};

function normalizeAssetBaseUrl(assetBaseUrl: string | null | undefined): string {
  const candidate = assetBaseUrl?.trim();
  if (!candidate) return "";

  try {
    const url = new URL(candidate);
    if (url.protocol !== "https:" && url.protocol !== "http:") return "";
    url.search = "";
    url.hash = "";
    return url.toString().replace(/\/+$/, "");
  } catch {
    return "";
  }
}

/**
 * Runtime bytes live behind a content-versioned public URL. The Next rewrite
 * maps the local-development URL onto the currently built public directory.
 * Hosted builds instead use the immutable R2 release root directly so a stale
 * main-domain cache cannot alter the JS/WASM transport representation.
 */
export function createBevyRuntimeUrls(
  version: string,
  backend: BevyRuntimeUrlBackend,
  assetBaseUrl?: string | null,
): BevyRuntimeUrls {
  const encodedVersion = encodeURIComponent(version.trim() || "local");
  const packageDir = backend === "webgpu" ? "pkg-webgpu" : "pkg-webgl2";
  const releaseBase = normalizeAssetBaseUrl(assetBaseUrl);
  const base = `${releaseBase}/bevy-runtime/v/${encodedVersion}/${packageDir}`;
  return {
    moduleUrl: `${base}/mir2_bevy_runtime.js`,
    wasmUrl: `${base}/mir2_bevy_runtime_bg.wasm`,
  };
}
