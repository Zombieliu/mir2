import { validateBevyRuntimeManifest } from "./bevy-runtime-manifest.mjs";

type BuildRuntimeManifest = Readonly<{ version: string; [key: string]: unknown }>;

export function parseBevyRuntimeBuildManifest(serialized: string | undefined): BuildRuntimeManifest {
  try {
    if (!serialized) throw new Error("Missing compiled runtime metadata");
    const raw = JSON.parse(serialized) as Record<string, unknown>;
    validateBevyRuntimeManifest(raw, "compiled runtime manifest");
    return Object.freeze(raw) as BuildRuntimeManifest;
  } catch {
    // Invalid build metadata leaves the compatibility client available.
    return Object.freeze({ version: "", files: [] });
  }
}

// Next's config.env embeds this one snapshot in both Page and proxy. Neither
// module independently imports a manifest that can change during compilation.
const runtimeManifest = parseBevyRuntimeBuildManifest(process.env.MIR2_BEVY_RUNTIME_BUILD_MANIFEST);
export default runtimeManifest;
