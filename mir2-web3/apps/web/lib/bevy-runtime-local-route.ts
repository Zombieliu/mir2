import { validateBevyRuntimeManifest } from "./bevy-runtime-manifest.mjs";

/** Only the complete current bundle may be served at its immutable local URL. */
export function bevyRuntimeLocalRewrite(manifest: unknown, pathname: string): string | null {
  try {
    const bundle = validateBevyRuntimeManifest(manifest);
    const match = /^\/bevy-runtime\/v\/([^/]+)\/([^/]+)\/(mir2_bevy_runtime(?:\.js|_bg\.wasm))$/.exec(pathname);
    if (!match || match[1] !== bundle.version) return null;
    const path = `public/bevy-runtime/${match[2]}/${match[3]}`;
    return bundle.files.some((entry) => entry.path === path)
      ? `/bevy-runtime-releases/${bundle.version}/${match[2]}/${match[3]}` : null;
  } catch {
    return null;
  }
}
