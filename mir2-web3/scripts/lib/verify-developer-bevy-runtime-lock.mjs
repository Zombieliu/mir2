import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import path from "node:path";
import { verifyBevyRuntimeVersion } from "../../apps/web/scripts/lib/bevy-runtime-version.mjs";

const WASM_HEADER = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);

export function verifyDeveloperBevyRuntimeLock(manifest, webRoot) {
  // Share the builder's schema validation and immutable identity, including
  // schema2 capabilities and the original file ordering of legacy releases.
  const verified = verifyBevyRuntimeVersion(manifest, "Bevy runtime version manifest");
  for (const file of verified.files) {
    // The shared schema validator permits only canonical package artifact paths.
    const absolutePath = path.resolve(webRoot, ...file.path.split("/"));
    const info = statSync(absolutePath);
    if (!info.isFile() || info.size === 0) {
      throw new Error(`Bevy runtime file is missing or empty: ${file.path}`);
    }
    const bytes = readFileSync(absolutePath);
    const hash = createHash("sha256").update(bytes).digest("hex");
    if (hash !== file.sha256) {
      throw new Error(`Bevy runtime hash mismatch: ${file.path}`);
    }
    if (file.path.endsWith(".wasm") && !bytes.subarray(0, 8).equals(WASM_HEADER)) {
      throw new Error(`Bevy runtime has an invalid WebAssembly header: ${file.path}`);
    }
  }
  return verified;
}
