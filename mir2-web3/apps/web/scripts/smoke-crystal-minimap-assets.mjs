import { readFile } from "node:fs/promises";
import path from "node:path";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const RESPAWN_MANIFEST_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const MMAP_META_PATH = path.join(WORKSPACE_ROOT, "public", "original-ui", "MMap", "meta.json");

const REPRESENTATIVE_MAPS = ["0", "1", "2", "HF1", "HF2", "HF3", "HKR"];
const NO_MINI_MAPS = ["D1801"];

const respawnManifest = JSON.parse(await readFile(RESPAWN_MANIFEST_PATH, "utf8"));
const mmapMeta = JSON.parse(await readFile(MMAP_META_PATH, "utf8"));
const maps = respawnManifest.maps ?? [];
const exportedByIndex = new Map((mmapMeta.frames ?? []).map((frame) => [Number(frame.index), frame]));
const failures = [];

for (const fileName of REPRESENTATIVE_MAPS) {
  const map = mapByFileName(fileName);
  if (!map) {
    failures.push(`${fileName} is missing from crystal_respawn_manifest.json`);
    continue;
  }

  if (!map.mini_map || map.mini_map <= 0) {
    failures.push(`${fileName} should have a Crystal mini_map index`);
    continue;
  }

  const frame = exportedByIndex.get(Number(map.mini_map));
  if (!frame) {
    failures.push(`${fileName} mini_map ${map.mini_map} is not exported`);
    continue;
  }

  if (Number(frame.width) <= 0 || Number(frame.height) <= 0) {
    failures.push(`${fileName} mini_map ${map.mini_map} has invalid dimensions`);
  }
}

for (const fileName of NO_MINI_MAPS) {
  const map = mapByFileName(fileName);
  if (!map) {
    failures.push(`${fileName} is missing from crystal_respawn_manifest.json`);
    continue;
  }
  if (Number(map.mini_map) !== 0) {
    failures.push(`${fileName} should have mini_map 0`);
  }
}

const neededMiniMaps = [
  ...new Set(maps.map((map) => Number(map.mini_map)).filter((index) => Number.isFinite(index) && index > 0)),
].sort((left, right) => left - right);
const missing = neededMiniMaps.filter((index) => !exportedByIndex.has(index));

console.log(
  JSON.stringify(
    {
      representativeMaps: REPRESENTATIVE_MAPS,
      noMiniMaps: NO_MINI_MAPS,
      neededMiniMapCount: neededMiniMaps.length,
      exportedMiniMapCount: mmapMeta.frames?.length ?? 0,
      missingMiniMapIndices: missing,
    },
    null,
    2,
  ),
);

if (missing.length > 0) {
  console.warn(`Warning: ${missing.length} Crystal mini-map indices are not exported: ${missing.join(", ")}`);
}

if (failures.length > 0) {
  console.error("Crystal mini-map asset smoke failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exitCode = 1;
}

function mapByFileName(fileName) {
  const normalized = normalizeMapFileName(fileName);
  return maps.find((map) => normalizeMapFileName(map.map_file_name ?? "") === normalized) ?? null;
}

function normalizeMapFileName(fileName) {
  return String(fileName).trim().replace(/\.map$/i, "").toLowerCase();
}
