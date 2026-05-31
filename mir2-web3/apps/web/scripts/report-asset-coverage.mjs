// Aggregates the resource-loading coverage signals into one numeric scoreboard, operating
// purely on committed/cached artifacts so it runs offline (no CRYSTAL_CLIENT_ROOT, no server).
// Each section degrades to "unavailable" if its cached input is missing, rather than failing.
//
// Sources:
//   - public/original-{map,ui}: committed PNG counts
//   - docs/generated/map/latest-crystal-map-coverage.json: sampled sprite-frame coverage
//   - docs/generated/assets/latest-minimap-assets.json: mini-map coverage
//   - lib/generated/crystal-present-sounds.generated.json + public sound index: sound coverage
import { existsSync, readFileSync, readdirSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";

const webRoot = path.resolve(import.meta.dirname, "..");
const repoRoot = path.resolve(webRoot, "..", "..");
const docsDir = firstExistingDir([
  path.join(repoRoot, "docs"),
  path.join(webRoot, "docs"),
]);

const summary = {
  generatedAt: new Date().toISOString(),
  sections: {
    originalAssets: reportOriginalAssets(),
    mapSpriteCoverage: reportMapSpriteCoverage(),
    miniMapCoverage: reportMiniMapCoverage(),
    soundCoverage: reportSoundCoverage(),
  },
};

const overall = computeOverall(summary.sections);
summary.overall = overall;

printTable(summary);

if (docsDir) {
  const outputDir = path.join(docsDir, "generated", "assets");
  mkdirSync(outputDir, { recursive: true });
  const outputPath = path.join(outputDir, "latest-asset-coverage-summary.json");
  writeFileSync(outputPath, `${JSON.stringify(summary, null, 2)}\n`);
  console.log(`\nWrote ${outputPath}`);
}

function reportOriginalAssets() {
  const roots = [
    path.join(webRoot, "public", "original-map"),
    path.join(webRoot, "public", "original-ui"),
  ];
  let pngCount = 0;
  for (const root of roots) {
    if (existsSync(root)) {
      pngCount += countFiles(root, (name) => name.toLowerCase().endsWith(".png"));
    }
  }
  return { available: pngCount > 0, pngCount };
}

function reportMapSpriteCoverage() {
  const coverage = readJsonIfExists(path.join(docsDir ?? "", "generated", "map", "latest-crystal-map-coverage.json"));
  const sampled = coverage?.sampledSpriteCoverage;
  if (!sampled || typeof sampled.sourceRequiredFrameCount !== "number" || sampled.sourceRequiredFrameCount <= 0) {
    return { available: false };
  }
  const required = sampled.sourceRequiredFrameCount;
  const present = sampled.sourcePresentFrameCount ?? 0;
  const empty = sampled.sourceEmptyFrameCount ?? 0;
  const outOfRange = sampled.sourceOutOfRangeFrameCount ?? 0;
  // "renderable" = frames Crystal would actually draw that resolve; "accounted" additionally
  // counts frames Crystal itself skips (empty / out-of-range) so they are not a real gap.
  return {
    available: true,
    requiredFrames: required,
    presentFrames: present,
    renderablePercent: round2((present / required) * 100),
    accountedPercent: round2(((present + empty + outOfRange) / required) * 100),
    mapsWithMissingSourceAssets: Array.isArray(sampled.mapsWithMissingSourceAssets)
      ? sampled.mapsWithMissingSourceAssets.length
      : null,
  };
}

function reportMiniMapCoverage() {
  const minimap = readJsonIfExists(path.join(docsDir ?? "", "generated", "assets", "latest-minimap-assets.json"));
  if (!minimap || typeof minimap.neededMiniMapCount !== "number" || minimap.neededMiniMapCount <= 0) {
    return { available: false };
  }
  const needed = minimap.neededMiniMapCount;
  const exported = minimap.exportedMiniMapCount ?? 0;
  const missing = Array.isArray(minimap.missingMiniMapIndices) ? minimap.missingMiniMapIndices.length : null;
  return {
    available: true,
    neededMiniMaps: needed,
    exportedMiniMaps: exported,
    missingMiniMaps: missing,
    coveragePercent: round2((Math.min(exported, needed) / needed) * 100),
  };
}

function reportSoundCoverage() {
  const present = readJsonIfExists(path.join(webRoot, "lib", "generated", "crystal-present-sounds.generated.json"));
  const index = readJsonIfExists(path.join(webRoot, "public", "original-ui", "sound-index.generated.json"));
  const presentCount = Array.isArray(present?.files) ? present.files.length : 0;
  const indexedCount = index?.sounds ? Object.keys(index.sounds).length : 0;
  if (indexedCount <= 0) {
    return { available: false, presentSounds: presentCount };
  }
  return {
    available: true,
    presentSounds: presentCount,
    indexedSounds: indexedCount,
    coveragePercent: round2((presentCount / indexedCount) * 100),
    note: "remaining sounds are raw-asset-limited (need the Crystal client)",
  };
}

function computeOverall(sections) {
  // Surfaces the two figures that matter most for "can a scene render": map sprite renderable
  // coverage and mini-map coverage. Sound coverage is intentionally excluded from the headline
  // because it is raw-asset-limited and tracked separately.
  const parts = [];
  if (sections.mapSpriteCoverage.available) parts.push(sections.mapSpriteCoverage.renderablePercent);
  if (sections.miniMapCoverage.available) parts.push(sections.miniMapCoverage.coveragePercent);
  if (parts.length === 0) return { available: false };
  return {
    available: true,
    renderCoveragePercent: round2(parts.reduce((sum, value) => sum + value, 0) / parts.length),
    note: "render coverage = mean of map-sprite + mini-map coverage; excludes sound coverage (raw-asset-limited, tracked separately). Not a measure of total asset completeness.",
  };
}

function printTable(report) {
  console.log("Resource asset coverage summary");
  console.log("================================");
  const { originalAssets, mapSpriteCoverage, miniMapCoverage, soundCoverage } = report.sections;
  console.log(`original PNG assets present : ${originalAssets.pngCount}`);
  console.log(
    mapSpriteCoverage.available
      ? `map sprite frames           : ${mapSpriteCoverage.presentFrames}/${mapSpriteCoverage.requiredFrames} renderable=${mapSpriteCoverage.renderablePercent}% accounted=${mapSpriteCoverage.accountedPercent}%`
      : "map sprite frames           : unavailable (cached coverage absent)",
  );
  console.log(
    miniMapCoverage.available
      ? `mini-maps                   : ${miniMapCoverage.exportedMiniMaps}/${miniMapCoverage.neededMiniMaps} (${miniMapCoverage.coveragePercent}%, missing=${miniMapCoverage.missingMiniMaps})`
      : "mini-maps                   : unavailable (cached coverage absent)",
  );
  console.log(
    soundCoverage.available
      ? `sounds (raw-asset-limited)  : ${soundCoverage.presentSounds}/${soundCoverage.indexedSounds} (${soundCoverage.coveragePercent}%)`
      : `sounds                      : ${soundCoverage.presentSounds} present`,
  );
  if (report.overall.available) {
    console.log(`render coverage (headline)  : ${report.overall.renderCoveragePercent}%`);
    console.log("  (= map-sprite + mini-map mean; excludes raw-asset-limited sound coverage above)");
  }
}

function countFiles(root, predicate) {
  let total = 0;
  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      total += countFiles(entryPath, predicate);
    } else if (entry.isFile() && predicate(entry.name)) {
      total += 1;
    }
  }
  return total;
}

function readJsonIfExists(filePath) {
  if (!filePath || !existsSync(filePath)) return null;
  try {
    return JSON.parse(readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function firstExistingDir(candidates) {
  for (const candidate of candidates) {
    if (existsSync(candidate)) return candidate;
  }
  return null;
}

function round2(value) {
  return Math.round(value * 100) / 100;
}
