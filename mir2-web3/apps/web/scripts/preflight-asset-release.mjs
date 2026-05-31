// Offline "release readiness" preflight for the resource-loading assets. Validates the things
// that can be checked without a running server or the raw Crystal client, so regressions are
// caught before an asset release/upload instead of in production:
//   1. present-sound manifest <-> committed .wav files are consistent
//   2. the Bevy entity atlas manifest is internally consistent (image exists, rects in bounds)
//   3. a sample of the original-asset manifest's sha256 hashes match the bytes on disk
//   4. render-coverage thresholds (map sprite + mini-map) hold
// Exits non-zero on any failure. Each section degrades to a warning if its input is absent.
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";

const webRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(webRoot, "..", "..");
const docsDir = [path.join(repoRoot, "docs"), path.join(webRoot, "docs")].find((dir) => existsSync(dir));

const MIN_RENDER_COVERAGE_PERCENT = Number.parseFloat(process.env.MIR2_MIN_RENDER_COVERAGE_PERCENT ?? "95");
const ORIGINAL_ASSET_HASH_SAMPLE = Number.parseInt(process.env.MIR2_ASSET_HASH_SAMPLE ?? "48", 10);

const failures = [];
const warnings = [];
const checks = {};

checks.presentSounds = checkPresentSounds();
checks.entityAtlas = checkEntityAtlas();
checks.originalAssetIntegrity = checkOriginalAssetIntegrity();
checks.renderCoverage = checkRenderCoverage();

console.log("Asset release preflight");
console.log("=======================");
for (const [name, result] of Object.entries(checks)) {
  console.log(`- ${name}: ${result.status}${result.detail ? ` (${result.detail})` : ""}`);
}
for (const warning of warnings) {
  console.log(`warning: ${warning}`);
}
if (failures.length > 0) {
  console.error("\nPreflight FAILED:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exitCode = 1;
} else {
  console.log("\nPreflight passed.");
}

function checkPresentSounds() {
  const manifestPath = path.join(webRoot, "lib", "generated", "crystal-present-sounds.generated.json");
  const soundDir = path.join(webRoot, "public", "original-ui", "Sound");
  const manifest = readJsonIfExists(manifestPath);
  if (!manifest || !Array.isArray(manifest.files)) {
    return warn("present-sound manifest absent", "unavailable");
  }
  if (!existsSync(soundDir)) {
    return warn("sound directory absent", "unavailable");
  }
  const onDisk = new Set(
    readdirSync(soundDir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith(".wav"))
      .map((entry) => entry.name),
  );
  const missing = manifest.files.filter((file) => !onDisk.has(file));
  const untracked = [...onDisk].filter((file) => !manifest.files.includes(file));
  if (missing.length > 0) {
    failures.push(`present-sound manifest lists ${missing.length} file(s) not on disk: ${missing.slice(0, 5).join(", ")}`);
  }
  if (untracked.length > 0) {
    failures.push(`sound dir has ${untracked.length} .wav not in manifest (run generate:present-sounds): ${untracked.slice(0, 5).join(", ")}`);
  }
  return pass(`${manifest.files.length} present sound(s) consistent`);
}

function checkEntityAtlas() {
  const manifestPath = path.join(webRoot, "public", "bevy-entity-atlases", "manifest.json");
  const manifest = readJsonIfExists(manifestPath);
  if (!manifest || !Array.isArray(manifest.atlases) || manifest.atlases.length === 0) {
    return warn("entity atlas manifest absent", "unavailable");
  }
  let totalSources = 0;
  for (const atlas of manifest.atlases) {
    const imageRel = String(atlas.imageUrl ?? "").replace(/^\//, "");
    const imagePath = path.join(webRoot, "public", ...imageRel.split("/").map((part) => decodeURIComponent(part)));
    if (!imageRel || !existsSync(imagePath)) {
      failures.push(`atlas '${atlas.key}' image missing on disk: ${atlas.imageUrl}`);
      continue;
    }
    const rects = Array.isArray(atlas.rects) ? atlas.rects : [];
    if (typeof atlas.sourceCount === "number" && atlas.sourceCount !== rects.length) {
      failures.push(`atlas '${atlas.key}' sourceCount ${atlas.sourceCount} != rects ${rects.length}`);
    }
    for (const rect of rects) {
      if (rect.x < 0 || rect.y < 0 || rect.x + rect.width > atlas.width || rect.y + rect.height > atlas.height) {
        failures.push(`atlas '${atlas.key}' rect '${rect.key}' is out of bounds`);
        break;
      }
    }
    totalSources += rects.length;
  }
  return pass(`${manifest.atlases.length} atlas(es), ${totalSources} packed sprite(s)`);
}

function checkOriginalAssetIntegrity() {
  const manifestPath = path.join(webRoot, "public", "original-asset-manifest.generated.json");
  const manifest = readJsonIfExists(manifestPath);
  if (!manifest || !manifest.assets) {
    return warn("original-asset manifest absent (run generate:original-asset-manifest)", "unavailable");
  }
  const entries = Object.entries(manifest.assets);
  if (entries.length === 0) {
    return warn("original-asset manifest empty", "unavailable");
  }
  const sampleSize = Math.min(ORIGINAL_ASSET_HASH_SAMPLE, entries.length);
  const step = Math.max(1, Math.floor(entries.length / sampleSize));
  let verified = 0;
  for (let index = 0; index < entries.length && verified < sampleSize; index += step) {
    const [publicPath, meta] = entries[index];
    const filePath = path.join(webRoot, "public", ...publicPath.replace(/^\//, "").split("/"));
    if (!existsSync(filePath)) {
      failures.push(`manifest asset missing on disk: ${publicPath}`);
      continue;
    }
    if (typeof meta?.sha256 === "string") {
      const actual = createHash("sha256").update(readFileSync(filePath)).digest("hex");
      if (actual !== meta.sha256) {
        failures.push(`manifest sha256 mismatch for ${publicPath}`);
      }
    }
    verified += 1;
  }
  return pass(`verified ${verified}/${entries.length} sampled asset hashes`);
}

function checkRenderCoverage() {
  const summary = readJsonIfExists(path.join(docsDir ?? "", "generated", "assets", "latest-asset-coverage-summary.json"));
  const map = summary?.sections?.mapSpriteCoverage;
  const minimap = summary?.sections?.miniMapCoverage;
  if (!summary || !map?.available) {
    return warn("coverage summary absent (run report:asset-coverage)", "unavailable");
  }
  if (typeof map.renderablePercent === "number" && map.renderablePercent < MIN_RENDER_COVERAGE_PERCENT) {
    failures.push(`map sprite renderable coverage ${map.renderablePercent}% < ${MIN_RENDER_COVERAGE_PERCENT}%`);
  }
  if (minimap?.available && minimap.missingMiniMaps && minimap.missingMiniMaps > 0) {
    failures.push(`${minimap.missingMiniMaps} mini-map(s) missing`);
  }
  return pass(`map renderable ${map.renderablePercent}%, mini-map missing ${minimap?.missingMiniMaps ?? "n/a"}`);
}

function readJsonIfExists(filePath) {
  if (!filePath || !existsSync(filePath)) return null;
  try {
    return JSON.parse(readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function pass(detail) {
  return { status: "ok", detail };
}

function warn(message, status) {
  warnings.push(message);
  return { status: status ?? "warn", detail: message };
}
