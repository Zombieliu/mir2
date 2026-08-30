import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const webRoot = resolve(import.meta.dirname, "..");
const repoRoot = resolve(webRoot, "..", "..");
const packageManifestPath = resolve(
  repoRoot,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const webManifestPath = resolve(
  webRoot,
  "lib",
  "generated",
  "crystal_respawn_manifest.json",
);

const packageManifest = readFileSync(packageManifestPath, "utf8");
const webManifest = readFileSync(webManifestPath, "utf8");

if (packageManifest !== webManifest) {
  throw new Error(
    "Crystal respawn manifests diverged; regenerate with generate-crystal-respawn-manifest.mjs",
  );
}

const parsed = JSON.parse(packageManifest);
if (parsed.total_maps !== parsed.maps?.length) {
  throw new Error(
    `Crystal respawn manifest map count mismatch: ${parsed.total_maps} != ${parsed.maps?.length}`,
  );
}

console.log(
  `Crystal respawn manifests match (${parsed.total_maps} source records, ${parsed.maps.filter((map) => map.map_file_name).length} named maps)`,
);
