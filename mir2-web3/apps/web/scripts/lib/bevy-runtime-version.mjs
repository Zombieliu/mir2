import crypto from "node:crypto";
import { validateBevyRuntimeManifest } from "../../lib/bevy-runtime-manifest.mjs";

// Keep the original file-order digest for legacy immutable URLs. Schema2 binds
// compiled capabilities as well as every JS/WASM hash; ordering is canonical.
export function computeBevyRuntimeVersion(manifest) {
  const combined = crypto.createHash("sha256");
  let files = manifest.files;
  if (manifest.schemaVersion === 2) {
    const packages = [...manifest.packages].sort((a, b) => compareCodeUnits(a.id, b.id)).map((entry) => ({
      id: entry.id,
      backend: entry.backend,
      packageDir: entry.packageDir,
      questUiAbiVersion: entry.questUiAbiVersion,
      bagUiAbiVersion: entry.bagUiAbiVersion,
      primarySharedUiCompiled: entry.primarySharedUiCompiled,
    }));
    combined.update("mir2-bevy-runtime:2\0");
    combined.update(JSON.stringify(packages));
    combined.update("\0");
    files = [...files].sort((a, b) => compareCodeUnits(a.path, b.path));
  }
  for (const file of files) {
    combined.update(file.path);
    combined.update("\0");
    combined.update(file.sha256);
    combined.update("\0");
  }
  return `bevy-${combined.digest("hex").slice(0, 16)}`;
}

function compareCodeUnits(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

export function verifyBevyRuntimeVersion(manifest, label = "runtime manifest") {
  const normalized = validateBevyRuntimeManifest(manifest, label);
  const expected = computeBevyRuntimeVersion(normalized);
  if (normalized.version !== expected) {
    throw new Error(`Bevy runtime version does not match files/capabilities in ${label}: expected ${expected}`);
  }
  return normalized;
}
