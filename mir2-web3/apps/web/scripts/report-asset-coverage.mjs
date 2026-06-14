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
  const sourceUnavailable = Array.isArray(minimap.sourceUnavailableMiniMapIndices)
    ? minimap.sourceUnavailableMiniMapIndices.length
    : 0;
  const sourceAvailableNeeded = Math.max(needed - sourceUnavailable, 1);
  return {
    available: true,
    neededMiniMaps: needed,
    exportedMiniMaps: exported,
    missingMiniMaps: missing,
    sourceUnavailableMiniMaps: sourceUnavailable,
    coveragePercent: round2((Math.min(exported, sourceAvailableNeeded) / sourceAvailableNeeded) * 100),
  };
}

function reportSoundCoverage() {
  const present = readJsonIfExists(path.join(webRoot, "lib", "generated", "crystal-present-sounds.generated.json"));
  const index = readJsonIfExists(path.join(webRoot, "public", "original-ui", "sound-index.generated.json"));
  const presentFiles = new Set((Array.isArray(present?.files) ? present.files : []).map((name) => String(name).toLowerCase()));
  const presentCount = presentFiles.size;
  const sounds = index?.sounds ?? {};
  const indexedCount = Object.keys(sounds).length;
  if (indexedCount <= 0) {
    return { available: false, presentSounds: presentCount };
  }
  // The 450 SoundList ids reference only ~320 DISTINCT .wav files (130 ids alias a shared file).
  // Coverage must be measured file-vs-file (present / distinct-required), not file-vs-id — the
  // old presentCount/indexedCount ratio understated a complete library as ~71%. We also report
  // how many of the 450 ids actually resolve to a present file (the player-facing figure).
  const requiredFiles = new Set();
  let resolvableIds = 0;
  for (const entry of Object.values(sounds)) {
    const fileName = entry?.path ? String(entry.path).split("/").pop()?.toLowerCase() : null;
    if (!fileName) continue;
    requiredFiles.add(fileName);
    if (presentFiles.has(fileName)) resolvableIds += 1;
  }
  const requiredFileCount = requiredFiles.size;
  return {
    available: true,
    presentSounds: presentCount,
    indexedSounds: indexedCount,
    requiredFiles: requiredFileCount,
    resolvableIds,
    idResolutionPercent: round2((resolvableIds / indexedCount) * 100),
    coveragePercent: requiredFileCount > 0 ? round2((presentCount / requiredFileCount) * 100) : 0,
    note: "coverage is distinct present .wav files / distinct required files; the 450 SoundList ids resolve via 320 shared files.",
  };
}

function computeOverall(sections) {
  // Surfaces the two figures that matter most for "can a scene render": map sprite renderable
  // coverage and mini-map coverage. Sound coverage (now complete) is tracked separately so the
  // headline stays a pure "can pixels render" figure.
  const parts = [];
  if (sections.mapSpriteCoverage.available) parts.push(sections.mapSpriteCoverage.renderablePercent);
  if (sections.miniMapCoverage.available) parts.push(sections.miniMapCoverage.coveragePercent);
  if (parts.length === 0) return { available: false };
  return {
    available: true,
    renderCoveragePercent: round2(parts.reduce((sum, value) => sum + value, 0) / parts.length),
    note: "render coverage = mean of map-sprite + mini-map coverage; sound coverage tracked separately (now complete). Not a measure of total asset completeness.",
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
      ? `mini-maps                   : ${miniMapCoverage.exportedMiniMaps}/${miniMapCoverage.neededMiniMaps} (${miniMapCoverage.coveragePercent}%, missing=${miniMapCoverage.missingMiniMaps}, source-unavailable=${miniMapCoverage.sourceUnavailableMiniMaps})`
      : "mini-maps                   : unavailable (cached coverage absent)",
  );
  console.log(
    soundCoverage.available
      ? `sounds (distinct files)     : ${soundCoverage.presentSounds}/${soundCoverage.requiredFiles} (${soundCoverage.coveragePercent}%); SoundList ids resolved ${soundCoverage.resolvableIds}/${soundCoverage.indexedSounds} (${soundCoverage.idResolutionPercent}%)`
      : `sounds                      : ${soundCoverage.presentSounds} present`,
  );
  if (report.overall.available) {
    console.log(`render coverage (headline)  : ${report.overall.renderCoveragePercent}%`);
    console.log("  (= map-sprite + mini-map mean; sound coverage tracked separately above)");
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
