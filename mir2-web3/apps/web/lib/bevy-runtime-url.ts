export type BevyRuntimeUrlBackend = "webgpu" | "webgl2";

export type BevyRuntimeUrls = {
  moduleUrl: string;
  wasmUrl: string;
};

/**
 * Runtime bytes live behind a content-versioned public URL. The Next rewrite
 * maps this immutable URL onto the currently built public directory, while the
 * version segment keeps browser/CDN cache identity tied to the runtime hash.
 */
export function createBevyRuntimeUrls(
  version: string,
  backend: BevyRuntimeUrlBackend,
): BevyRuntimeUrls {
  const encodedVersion = encodeURIComponent(version.trim() || "local");
  const packageDir = backend === "webgpu" ? "pkg-webgpu" : "pkg-webgl2";
  const base = `/bevy-runtime/v/${encodedVersion}/${packageDir}`;
  return {
    moduleUrl: `${base}/mir2_bevy_runtime.js`,
    wasmUrl: `${base}/mir2_bevy_runtime_bg.wasm`,
  };
}
