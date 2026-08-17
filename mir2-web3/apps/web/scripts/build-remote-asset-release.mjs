import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { Writable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { constants as zlibConstants, createGzip } from "node:zlib";

import { createCasRelease, writeCasReleaseArtifacts } from "./asset-pipeline/cas-release.mjs";
import { inspectFullPackClosure, sha256File } from "./asset-pipeline/full-pack-closure.mjs";
import { assertQuestItemIconClosure } from "./asset-pipeline/quest-item-icon-closure.mjs";
import { verifyMonsterFrameClosure } from "./verify-monster-frame-closure.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const ORIGINAL_ASSET_MANIFEST_PATH = path.join(WEB_ROOT, "public", "original-asset-manifest.generated.json");
const BEVY_RUNTIME_MANIFEST_PATH = path.join(WEB_ROOT, "lib", "generated", "bevy_runtime_version.json");
const FULL_CRYSTAL_PACK_ROOT = path.join(WEB_ROOT, "public", "generated", "crystal-packs", "full");
const FULL_CRYSTAL_PACK_INDEX_PATH = "/generated/crystal-packs/full/index.json";
const FULL_CRYSTAL_PACK_COVERAGE_PATH = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "assets",
  "crystal-full-pack-coverage.generated.json",
);
const DEFAULT_BASE_URL = "http://127.0.0.1:13010";
const DEFAULT_OUTPUT_ROOT = path.resolve(REPO_ROOT, "docs", "generated", "remote-assets");
const DEFAULT_CACHE_CONTROL = "public, max-age=31536000, immutable";
const DEFAULT_SCENE_SPRITE_REMOTE_ROOTS = [
  "CArmour",
  "CHair",
  "CWeapon",
  "AArmour",
  "AHair",
  "AWeapon",
  "ARArmour",
  "ARHair",
  "ARWeapon",
  "NPC",
  "Monster",
];
const DEFAULT_PUBLIC_ASSET_ROOTS = [
  "original-ui",
  "original-map",
  "generated/original-map-blend",
  "generated/map-atlas",
  "bevy-entity-atlases",
];
const FULL_PACK_GZIP_OPTIONS = Object.freeze({
  level: zlibConstants.Z_BEST_COMPRESSION,
  mtime: 0,
});
const LOGIN_TITLE_PATHS = [
  ...makeRange(30, 32),
  ...makeRange(320, 334),
].map((value) => `/original-ui/Title/${value}.png`);
const LOGIN_CHRSEL_PATHS = Array.from({ length: 19 }, (_, index) => `/original-ui/ChrSel/${index}.png`);
const EXTRA_ORIGINAL_ASSET_PATHS = [
  "/original-ui/Sound/Login2.wav",
  "/original-ui/Sound/100.wav",
  "/original-ui/Prguse/44.png",
  "/original-ui/Prguse/65.png",
  "/original-ui/Prguse/940.png",
  "/original-ui/Title/40.png",
  ...makeRange(340, 354).map((value) => `/original-ui/Title/${value}.png`),
  ...makeRange(360, 362).map((value) => `/original-ui/Title/${value}.png`),
];
const QUEST_ITEM_ICON_CLOSURE = assertQuestItemIconClosure();
const PUBLIC_ASSET_EXTENSIONS = new Set([
  ".cur",
  ".gif",
  ".jpeg",
  ".jpg",
  ".json",
  ".js",
  ".mp3",
  ".ogg",
  ".png",
  ".wav",
  ".webp",
  ".wasm",
]);
const REQUIRED_MANIFEST_PATHS = [
  ...LOGIN_TITLE_PATHS,
  ...LOGIN_CHRSEL_PATHS,
  "/original-ui/Prguse/1084.png",
  "/original-ui/Cursors/Cursor_Default.CUR",
  "/original-ui/Cursors/Cursor_TextPrompt.CUR",
  ...EXTRA_ORIGINAL_ASSET_PATHS,
  ...QUEST_ITEM_ICON_CLOSURE.requiredPaths,
];

const args = parseArgs(process.argv.slice(2));
const baseUrl = normalizeUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL);
const runId = String(args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14));
const outputDir = path.resolve(args.outDir ?? process.env.MIR2_REMOTE_ASSET_OUTPUT_DIR ?? path.join(DEFAULT_OUTPUT_ROOT, runId));
const allowMissing = booleanArg(args.allowMissing ?? process.env.MIR2_REMOTE_ASSET_ALLOW_MISSING, false);
const offlineManifest = booleanArg(args.offlineManifest ?? process.env.MIR2_REMOTE_ASSET_OFFLINE_MANIFEST, false);
const includeSceneSprites = booleanArg(
  args.includeSceneSprites ?? process.env.MIR2_REMOTE_ASSET_INCLUDE_SCENE_SPRITES,
  true,
);
const sceneSpriteRoots = parseListArg(
  args.sceneSpriteRoots ?? process.env.MIR2_REMOTE_ASSET_SCENE_SPRITE_ROOTS,
  DEFAULT_SCENE_SPRITE_REMOTE_ROOTS,
);
const includePublicAssetRoots = booleanArg(
  args.includePublicAssetRoots ?? process.env.MIR2_REMOTE_ASSET_INCLUDE_PUBLIC_ROOTS,
  true,
);
const includeFullCrystalPack = booleanArg(
  args.includeFullCrystalPack ?? process.env.MIR2_REMOTE_ASSET_INCLUDE_FULL_CRYSTAL_PACK,
  false,
);
const includeBevyRuntime = booleanArg(
  args.includeBevyRuntime ?? process.env.MIR2_REMOTE_ASSET_INCLUDE_BEVY_RUNTIME,
  false,
);
const publicAssetRoots = parseListArg(
  args.publicAssetRoots ?? process.env.MIR2_REMOTE_ASSET_PUBLIC_ROOTS,
  DEFAULT_PUBLIC_ASSET_ROOTS,
);
const stageConcurrency = positiveIntegerArg(
  args.stageConcurrency ?? process.env.MIR2_REMOTE_ASSET_STAGE_CONCURRENCY,
  32,
);
const stageFileMode = String(args.stageFileMode ?? process.env.MIR2_REMOTE_ASSET_STAGE_FILE_MODE ?? "copy").toLowerCase();
const hashMode = String(args.hashMode ?? process.env.MIR2_REMOTE_ASSET_HASH_MODE ?? "sha256").toLowerCase();
const compactFiles = booleanArg(args.compactFiles ?? process.env.MIR2_REMOTE_ASSET_COMPACT_FILES, false);
const casEnabled = booleanArg(args.cas ?? process.env.MIR2_REMOTE_ASSET_CAS, true);
const casPrefix = args.casPrefix ?? process.env.MIR2_REMOTE_ASSET_CAS_PREFIX ?? "mir2/cas";
const releaseChannel = args.channel ?? process.env.MIR2_REMOTE_ASSET_CHANNEL ?? "production";
const gzipFullCrystalPackJson = booleanArg(
  args.gzipFullCrystalPackJson ?? process.env.MIR2_REMOTE_ASSET_GZIP_FULL_PACK_JSON,
  includeFullCrystalPack,
);
const gzipBevyRuntimeWasm = booleanArg(
  args.gzipBevyRuntimeWasm ?? process.env.MIR2_REMOTE_ASSET_GZIP_BEVY_RUNTIME_WASM,
  includeBevyRuntime,
);
const gzipConcurrency = positiveIntegerArg(
  args.gzipConcurrency ?? process.env.MIR2_REMOTE_ASSET_GZIP_CONCURRENCY,
  4,
);

async function main() {
  if (gzipFullCrystalPackJson && !includeFullCrystalPack) {
    throw new Error("Full-pack JSON compression requires --includeFullCrystalPack true.");
  }
  if (gzipFullCrystalPackJson && casEnabled) {
    throw new Error("Compressed full-pack releases require --cas false to avoid uploading duplicate raw CAS objects.");
  }
  if (gzipBevyRuntimeWasm && !includeBevyRuntime) {
    throw new Error("Bevy runtime compression requires --includeBevyRuntime true.");
  }
  if (gzipBevyRuntimeWasm && casEnabled) {
    throw new Error("Compressed Bevy runtime releases require --cas false.");
  }
  if (includeSceneSprites && sceneSpriteRoots.includes("Monster")) {
    const monsterLibraries = await verifyMonsterFrameClosure();
    console.log(
      `[remote-assets] monster frame closure verified (${monsterLibraries.length} libraries)`,
    );
  }

  const manifestUrl = new URL("/api/asset-manifest", baseUrl);
  const assetManifest = offlineManifest ? createOfflineAssetManifest() : await fetchJson(manifestUrl);
  const version = String(assetManifest.version || "unknown");
  const objectPrefix = normalizeObjectPrefix(
    resolveTemplate(
      args.objectPrefix ??
        process.env.MIR2_ASSET_OBJECT_PREFIX ??
        assetManifest.remoteAssets?.objectPrefix ??
        "mir2/v/{version}",
      version,
    ),
  );
  const assetBaseUrl = normalizeAssetBaseUrl(
    resolveTemplate(
      args.assetBaseUrl ??
        process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ??
        process.env.MIR2_ASSET_BASE_URL ??
        assetManifest.remoteAssets?.assetBaseUrl ??
        "",
      version,
    ),
  );
  const stageDir = path.resolve(
    args.stageDir ?? process.env.MIR2_REMOTE_ASSET_STAGE_DIR ?? path.join(REPO_ROOT, ".mir2-remote-assets", version),
  );

  const collected = await collectReleaseUrls(assetManifest, manifestUrl);
  const staged = await stageStaticFiles({
    staticUrls: collected.staticUrls,
    stageDir,
    objectPrefix,
    allowMissing,
    concurrency: stageConcurrency,
  });
  await annotateStoredRepresentations(staged.files);
  const bevyRuntime = includeBevyRuntime
    ? await readBevyRuntimeReleaseRecord()
    : { enabled: false, version: null, contentEncoding: null, files: [] };
  const requiredManifestPaths = [
    ...REQUIRED_MANIFEST_PATHS,
    ...(includeFullCrystalPack ? [FULL_CRYSTAL_PACK_INDEX_PATH] : []),
    ...bevyRuntime.files.map((file) => file.path),
  ];
  const requiredReleasePaths = requiredManifestPaths.filter((requiredPath) =>
    staged.files.some((file) => file.path === requiredPath),
  );
  const missingRequiredManifestPaths = requiredManifestPaths.filter((requiredPath) =>
    !requiredReleasePaths.includes(requiredPath),
  );

  if (missingRequiredManifestPaths.length > 0 && !allowMissing) {
    throw new Error(
      `Remote asset release is missing required paths: ${missingRequiredManifestPaths.join(", ")}`,
    );
  }

  const release = {
    schemaVersion: 1,
    kind: "mir2-remote-asset-release",
    version,
    generatedAt: new Date().toISOString(),
    baseUrl,
    assetManifestUrl: manifestUrl.href,
    assetBaseUrl: assetBaseUrl || null,
    objectPrefix,
    pathMode: "mirror-local-public-path",
    cacheControl: DEFAULT_CACHE_CONTROL,
    outputDir,
    stageDir,
    stats: {
      packCount: collected.packs.length,
      sceneCount: collected.scenes.length,
      originalAssetManifestAssetCount: collected.originalAssetManifest.assetCount,
      sceneSpriteRootCount: collected.sceneSpriteRoots.length,
      sceneSpriteFileCount: collected.sceneSpriteRoots.reduce((sum, root) => sum + root.fileCount, 0),
      publicAssetRootCount: collected.publicAssetRoots.length,
      publicAssetFileCount: collected.publicAssetRoots.reduce((sum, root) => sum + root.fileCount, 0),
      fullCrystalPackFileCount: collected.fullCrystalPack.fileCount,
      fullCrystalPackLibraryCount: collected.fullCrystalPack.libraryCount,
      fullCrystalPackPageCount: collected.fullCrystalPack.pageCount,
      fileCount: staged.files.length,
      missingCount: staged.missing.length,
      totalBytes: staged.files.reduce((sum, file) => sum + file.size, 0),
      storageBytes: staged.files.reduce((sum, file) => sum + (file.encodedSize ?? file.size), 0),
      encodedFileCount: staged.files.filter((file) => file.contentEncoding).length,
      storageSavingsBytes: staged.files.reduce(
        (sum, file) => sum + file.size - (file.encodedSize ?? file.size),
        0,
      ),
      stageConcurrency,
      gzipConcurrency,
    },
    packs: collected.packs,
    scenes: collected.scenes,
    originalAssetManifest: collected.originalAssetManifest,
    sceneSpriteRoots: collected.sceneSpriteRoots,
    publicAssetRoots: collected.publicAssetRoots,
    fullCrystalPack: collected.fullCrystalPack,
    bevyRuntime,
    files: compactFiles ? staged.files.map(compactReleaseFile) : staged.files,
    missing: staged.missing,
    requiredManifestPaths,
    missingRequiredManifestPaths,
  };

  if (release.missing.length > 0 && !allowMissing) {
    throw new Error(`Remote asset release has ${release.missing.length} missing files`);
  }

  await fs.mkdir(outputDir, { recursive: true });
  if (casEnabled) {
    if (hashMode !== "sha256") throw new Error("CAS releases require MIR2_REMOTE_ASSET_HASH_MODE=sha256.");
    release.cas = await writeCasReleaseArtifacts(
      createCasRelease(staged.files, { prefix: casPrefix, channel: releaseChannel }),
      outputDir,
    );
  }
  const releasePath = path.join(outputDir, "remote-asset-release.json");
  const latestPath = path.join(DEFAULT_OUTPUT_ROOT, "latest-remote-asset-release.json");
  await fs.writeFile(releasePath, `${stringifyRelease(release)}\n`, "utf8");
  await fs.mkdir(DEFAULT_OUTPUT_ROOT, { recursive: true });
  await fs.copyFile(releasePath, latestPath);

  console.log(
    JSON.stringify(
      {
        ok: release.missing.length === 0 || allowMissing,
        releasePath,
        latestPath,
        version,
        assetBaseUrl: release.assetBaseUrl,
        objectPrefix,
        fileCount: release.stats.fileCount,
        totalBytes: release.stats.totalBytes,
        missingCount: release.stats.missingCount,
        stageDir,
        casManifestHash: release.cas?.manifest.sha256 ?? null,
        casManifestObjectKey: release.cas?.manifest.objectKey ?? null,
        channelObjectKey: release.cas?.channel.objectKey ?? null,
      },
      null,
      2,
    ),
  );
}

function compactReleaseFile(file) {
  const compact = {
    p: file.relativePath,
    s: file.size,
    h: file.sha256,
    c: file.contentType,
    src: file.sources,
  };
  if (file.contentEncoding) {
    compact.e = file.contentEncoding;
    compact.es = file.encodedSize;
    compact.eh = file.encodedSha256;
  }
  return compact;
}

function stringifyRelease(release) {
  return compactFiles ? JSON.stringify(release) : JSON.stringify(release, null, 2);
}

function createOfflineAssetManifest() {
  const version = normalizeAssetVersion(args.assetVersion ?? process.env.MIR2_ASSET_VERSION ?? runId);
  if (!version) {
    throw new Error("Offline asset manifest mode requires --assetVersion, MIR2_ASSET_VERSION, or --runId.");
  }

  return {
    schemaVersion: 1,
    version,
    versionSource: "offline-build-remote-asset-release",
    remoteAssets: {
      enabled: Boolean(args.assetBaseUrl ?? process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? process.env.MIR2_ASSET_BASE_URL),
      assetBaseUrl: args.assetBaseUrl ?? process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? process.env.MIR2_ASSET_BASE_URL ?? null,
      objectPrefix: args.objectPrefix ?? process.env.MIR2_ASSET_OBJECT_PREFIX ?? "mir2/v/{version}",
    },
    resourcePacks: [],
  };
}

async function collectReleaseUrls(assetManifest, manifestUrl) {
  const staticUrls = new Map();
  const packs = [];
  const scenes = [];
  const sceneSpriteRootRecords = [];
  const publicAssetRootRecords = [];
  let fullCrystalPackRecord = disabledFullCrystalPackRecord();
  const originalAssetManifestRecord = await collectOriginalAssetManifestStaticUrls(staticUrls);
  const resourcePacks = Array.isArray(assetManifest.resourcePacks) ? assetManifest.resourcePacks : [];

  for (const pack of [...resourcePacks].sort((a, b) => Number(a.priority ?? 0) - Number(b.priority ?? 0))) {
    const packName = String(pack.name ?? "pack");
    const packRecord = {
      name: packName,
      label: String(pack.label ?? packName),
      priority: Number(pack.priority ?? 0),
      urlCount: 0,
      sceneCount: Array.isArray(pack.scenes) ? pack.scenes.length : 0,
    };

    for (const url of Array.isArray(pack.urls) ? pack.urls : []) {
      if (addStaticUrl(staticUrls, url, packName)) packRecord.urlCount += 1;
    }

    for (const scene of Array.isArray(pack.scenes) ? pack.scenes : []) {
      const sceneUrl = new URL(String(scene.url), manifestUrl);
      const response = await fetch(sceneUrl);
      if (!response.ok) {
        scenes.push({
          pack: packName,
          label: String(scene.label ?? scene.url),
          url: sceneUrl.href,
          ok: false,
          status: response.status,
          frameCount: 0,
        });
        continue;
      }

      const blueprint = await response.json();
      const limit = Number(scene.spriteFrameLimit ?? 0);
      const frameUrls = extractSceneFrameUrls(blueprint, limit);
      for (const frameUrl of frameUrls) addStaticUrl(staticUrls, frameUrl, `${packName}:scene`);
      scenes.push({
        pack: packName,
        label: String(scene.label ?? scene.url),
        url: sceneUrl.href,
        ok: true,
        status: response.status,
        frameCount: frameUrls.length,
        spriteFrameLimit: limit,
      });
    }

    packs.push(packRecord);
  }

  if (includeSceneSprites) {
    sceneSpriteRootRecords.push(...(await collectSceneSpriteStaticUrls(staticUrls, sceneSpriteRoots)));
  }

  if (includePublicAssetRoots || includeBevyRuntime) {
    const roots = [...new Set([
      ...(includePublicAssetRoots ? publicAssetRoots : []),
      ...(includeBevyRuntime ? ["bevy-runtime"] : []),
    ])];
    publicAssetRootRecords.push(...(await collectPublicAssetRootStaticUrls(staticUrls, roots)));
  }

  if (includeFullCrystalPack) {
    fullCrystalPackRecord = await collectFullCrystalPackStaticUrls(staticUrls);
  }

  await collectMapAtlasPageStaticUrls(staticUrls);

  return {
    staticUrls,
    packs,
    scenes,
    originalAssetManifest: originalAssetManifestRecord,
    sceneSpriteRoots: sceneSpriteRootRecords,
    publicAssetRoots: publicAssetRootRecords,
    fullCrystalPack: fullCrystalPackRecord,
  };
}

function disabledFullCrystalPackRecord() {
  return {
    enabled: false,
    verified: false,
    path: FULL_CRYSTAL_PACK_INDEX_PATH,
    contentHash: null,
    libraryCount: 0,
    pageCount: 0,
    fileCount: 0,
  };
}

async function collectFullCrystalPackStaticUrls(staticUrls) {
  let coverage;
  try {
    coverage = await readJsonFile(FULL_CRYSTAL_PACK_COVERAGE_PATH);
  } catch (error) {
    if (error?.code === "ENOENT") {
      throw new Error(
        "Full Crystal pack publication requires a built and verified pack. Run assets:full-pack:build and assets:full-pack:verify first.",
      );
    }
    throw error;
  }

  const closure = await inspectFullPackClosure({
    fullPackRoot: FULL_CRYSTAL_PACK_ROOT,
    publicRoot: path.posix.dirname(FULL_CRYSTAL_PACK_INDEX_PATH),
    expectedContentHash: coverage?.evidence?.contentHash ?? "",
    verifyPageHashes: false,
    rejectOrphans: true,
  });
  if (
    coverage?.kind !== "mir2-crystal-full-pack-coverage" ||
    coverage?.mode !== "verify" ||
    coverage?.evidence?.pageHashesVerified !== true ||
    coverage?.evidence?.contentHash !== closure.contentHash ||
    Number(coverage?.evidence?.verifiedLibraryCount ?? 0) !== closure.libraryCount ||
    Number(coverage?.evidence?.verifiedUniquePageCount ?? 0) !== closure.pageCount
  ) {
    throw new Error(
      "Full Crystal pack coverage evidence is missing, stale, or not hash-verified. Run assets:full-pack:verify.",
    );
  }

  addStaticUrl(staticUrls, closure.indexFile.publicPath, "full-crystal-pack:index", {
    expectedSize: closure.indexFile.size,
  });
  for (const file of closure.libraryFiles) {
    addStaticUrl(staticUrls, file.publicPath, "full-crystal-pack:library", {
      expectedSha256: file.sha256,
      expectedSize: file.size,
    });
  }
  for (const file of closure.pageFiles) {
    addStaticUrl(staticUrls, file.publicPath, "full-crystal-pack:page", {
      expectedSha256: file.sha256,
      expectedSize: file.size,
    });
  }

  return {
    enabled: true,
    verified: true,
    path: FULL_CRYSTAL_PACK_INDEX_PATH,
    contentHash: closure.contentHash,
    sourceContentHash: closure.sourceContentHash,
    libraryCount: closure.libraryCount,
    pageCount: closure.pageCount,
    fileCount: closure.fileCount,
    jsonContentEncoding: gzipFullCrystalPackJson ? "gzip" : null,
  };
}

async function readJsonFile(filePath) {
  return JSON.parse(await fs.readFile(filePath, "utf8"));
}

async function collectMapAtlasPageStaticUrls(staticUrls) {
  const manifestPath = "/generated/map-atlas/manifest.json";
  if (!staticUrls.has(manifestPath)) return;

  const localManifestPath = path.join(WEB_ROOT, "public", "generated", "map-atlas", "manifest.json");
  let manifest;
  try {
    manifest = JSON.parse(await fs.readFile(localManifestPath, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT") return;
    throw error;
  }

  for (const atlas of Array.isArray(manifest.atlases) ? manifest.atlases : []) {
    addStaticUrl(staticUrls, atlas?.imageUrl, "map-atlas-manifest");
  }
}

async function collectOriginalAssetManifestStaticUrls(staticUrls) {
  const manifest = await readOriginalAssetManifest();
  const assetPaths = originalAssetManifestAssetPaths(manifest);
  for (const assetPath of assetPaths) {
    addStaticUrl(staticUrls, assetPath, "original-asset-manifest");
  }

  return {
    path: path.relative(REPO_ROOT, ORIGINAL_ASSET_MANIFEST_PATH).split(path.sep).join("/"),
    schemaVersion: manifest.schemaVersion ?? null,
    assetHash: manifest.assetHash ?? null,
    assetCount: assetPaths.length,
    originalMapPngCount: manifest.stats?.originalMapPngCount ?? null,
    originalUiPngCount: manifest.stats?.originalUiPngCount ?? null,
  };
}

async function readOriginalAssetManifest() {
  try {
    return JSON.parse(await fs.readFile(ORIGINAL_ASSET_MANIFEST_PATH, "utf8"));
  } catch (error) {
    if (error?.code === "ENOENT") {
      throw new Error(`Original asset manifest is missing: ${ORIGINAL_ASSET_MANIFEST_PATH}`);
    }
    throw error;
  }
}

function originalAssetManifestAssetPaths(manifest) {
  const paths = Object.keys(manifest?.assets ?? {})
    .map(normalizeOriginalAssetManifestPath)
    .filter(Boolean)
    .sort((left, right) => left.localeCompare(right));
  if (!paths.length) {
    throw new Error(`Original asset manifest has no assets: ${ORIGINAL_ASSET_MANIFEST_PATH}`);
  }
  return paths;
}

function normalizeOriginalAssetManifestPath(value) {
  const pathname = String(value || "").trim().replace(/^\/+/, "");
  if (!pathname) return "";
  const assetPath = `/${pathname}`;
  if (!assetPath.startsWith("/original-map/") && !assetPath.startsWith("/original-ui/")) return "";
  const ext = path.extname(assetPath).toLowerCase();
  if (ext !== ".png" && ext !== ".cur") return "";
  return assetPath;
}

function addStaticUrl(staticUrls, value, source, integrity = {}) {
  const assetPath = normalizeStaticAssetPath(value);
  if (!assetPath) return false;
  const existing = staticUrls.get(assetPath);
  if (existing) {
    assertCompatibleIntegrity(existing, integrity, assetPath);
    existing.sources.add(source);
    return false;
  }
  staticUrls.set(assetPath, {
    path: assetPath,
    sources: new Set([source]),
    expectedSha256: integrity.expectedSha256 ?? null,
    expectedSize: integrity.expectedSize ?? null,
  });
  return true;
}

function assertCompatibleIntegrity(existing, integrity, assetPath) {
  for (const key of ["expectedSha256", "expectedSize"]) {
    const next = integrity[key] ?? null;
    if (next !== null && existing[key] !== null && existing[key] !== next) {
      throw new Error(`Conflicting ${key} for release asset ${assetPath}`);
    }
    if (existing[key] === null && next !== null) existing[key] = next;
  }
}

async function stageStaticFiles({ staticUrls, stageDir, objectPrefix, allowMissing, concurrency }) {
  await fs.mkdir(stageDir, { recursive: true });
  const entries = [...staticUrls.values()].sort((a, b) => a.path.localeCompare(b.path));
  let completed = 0;
  const results = await mapWithConcurrency(entries, concurrency, async (entry) => {
    const result = await stageOneStaticFile({ entry, stageDir, objectPrefix });
    completed += 1;
    if (completed % 25000 === 0 || completed === entries.length) {
      console.error(`[remote-asset-release] staged ${completed}/${entries.length}`);
    }
    return result;
  });

  const files = [];
  const missing = [];
  for (const result of results) {
    if (result.file) files.push(result.file);
    if (result.missing) missing.push(result.missing);
  }

  if (missing.length > 0 && !allowMissing) {
    console.error(JSON.stringify({ missing: missing.slice(0, 20), missingCount: missing.length }, null, 2));
  }

  return { files, missing };
}

async function annotateStoredRepresentations(files) {
  const encodedFiles = files.filter((file) => {
    const relativePath = file.relativePath.toLowerCase();
    return (
      (gzipFullCrystalPackJson &&
        relativePath.startsWith("generated/crystal-packs/full/") &&
        relativePath.endsWith(".json")) ||
      (gzipBevyRuntimeWasm &&
        relativePath.startsWith("bevy-runtime/pkg-") &&
        relativePath.endsWith("_bg.wasm"))
    );
  });
  if (!encodedFiles.length) return;
  let completed = 0;
  await mapWithConcurrency(encodedFiles, gzipConcurrency, async (file) => {
    const encoded = await gzipFileMetadata(file.stagePath);
    file.contentEncoding = "gzip";
    file.encodedSize = encoded.size;
    file.encodedSha256 = encoded.sha256;
    completed += 1;
    if (completed % 250 === 0 || completed === encodedFiles.length) {
      console.error(`[remote-asset-release] measured gzip ${completed}/${encodedFiles.length}`);
    }
  });
}

async function readBevyRuntimeReleaseRecord() {
  const manifest = await readJsonFile(BEVY_RUNTIME_MANIFEST_PATH);
  const version = String(manifest?.version ?? "").trim();
  const files = Array.isArray(manifest?.files)
    ? manifest.files.map((file) => ({
        path: `/${String(file.path ?? "").replace(/^public\//, "").replace(/^\/+/, "")}`,
        sha256: String(file.sha256 ?? ""),
      }))
    : [];
  if (!/^bevy-[a-f0-9]{16}$/i.test(version) || files.length !== 4) {
    throw new Error(`Invalid Bevy runtime manifest: ${BEVY_RUNTIME_MANIFEST_PATH}`);
  }
  for (const file of files) {
    if (
      !/^\/bevy-runtime\/pkg-(?:webgpu|webgl2)\/mir2_bevy_runtime(?:_bg\.wasm|\.js)$/.test(file.path) ||
      !/^[a-f0-9]{64}$/i.test(file.sha256)
    ) {
      throw new Error(`Invalid Bevy runtime file entry: ${JSON.stringify(file)}`);
    }
  }
  return {
    enabled: true,
    version,
    contentEncoding: gzipBevyRuntimeWasm ? "gzip" : null,
    files,
  };
}

async function gzipFileMetadata(filePath) {
  const hash = createHash("sha256");
  let size = 0;
  const sink = new Writable({
    write(chunk, _encoding, callback) {
      size += chunk.length;
      hash.update(chunk);
      callback();
    },
  });
  await pipeline(createReadStream(filePath), createGzip(FULL_PACK_GZIP_OPTIONS), sink);
  return { size, sha256: hash.digest("hex") };
}

async function stageOneStaticFile({ entry, stageDir, objectPrefix }) {
  const relativePath = decodeAssetRelativePath(entry.path.replace(/^\/+/, ""));
  const localPath = path.join(WEB_ROOT, "public", relativePath);
  const stagePath = stageFileMode === "reference" ? localPath : path.join(stageDir, relativePath);

  let stats;
  try {
    stats = await fs.stat(localPath);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
    return {
      missing: {
        path: entry.path,
        localPath,
        objectKey: joinObjectKey(objectPrefix, relativePath),
        sources: [...entry.sources].sort(),
      },
    };
  }

  if (!stats.isFile()) {
    return {
      missing: {
        path: entry.path,
        localPath,
        objectKey: joinObjectKey(objectPrefix, relativePath),
        sources: [...entry.sources].sort(),
        reason: "not-a-file",
      },
    };
  }

  if (hashMode !== "sha256" && hashMode !== "skip") {
    throw new Error(`Unsupported MIR2_REMOTE_ASSET_HASH_MODE: ${hashMode}; expected "sha256" or "skip".`);
  }

  if (entry.expectedSize !== null && stats.size !== entry.expectedSize) {
    throw new Error(`Release asset size mismatch: ${entry.path} expected ${entry.expectedSize}, found ${stats.size}`);
  }
  if (entry.expectedSha256 && hashMode !== "sha256") {
    throw new Error(`MIR2_REMOTE_ASSET_HASH_MODE=sha256 is required for integrity-bound asset ${entry.path}`);
  }
  const digest = hashMode === "sha256" ? await sha256File(localPath) : null;
  if (entry.expectedSha256 && digest !== entry.expectedSha256) {
    throw new Error(`Release asset hash mismatch: ${entry.path} expected ${entry.expectedSha256}, found ${digest}`);
  }
  if (stageFileMode !== "reference") {
    await fs.mkdir(path.dirname(stagePath), { recursive: true });
    await stageFile(localPath, stagePath);
  }

  return {
    file: {
      path: entry.path,
      relativePath,
      localPath,
      stagePath,
      objectKey: joinObjectKey(objectPrefix, relativePath),
      size: stats.size,
      sha256: digest,
      contentType: contentTypeForPath(relativePath),
      cacheControl: DEFAULT_CACHE_CONTROL,
      sources: [...entry.sources].sort(),
    },
  };
}

async function stageFile(localPath, stagePath) {
  if (stageFileMode === "link") {
    try {
      await fs.rm(stagePath, { force: true });
      await fs.link(localPath, stagePath);
      return;
    } catch (error) {
      if (error?.code !== "EXDEV" && error?.code !== "EPERM" && error?.code !== "EOPNOTSUPP") {
        throw error;
      }
    }
  }

  if (stageFileMode === "copy") {
    await fs.copyFile(localPath, stagePath);
    return;
  }

  throw new Error(`Unsupported MIR2_REMOTE_ASSET_STAGE_FILE_MODE: ${stageFileMode}; expected "copy", "link", or "reference".`);
}

async function mapWithConcurrency(items, concurrency, worker) {
  const results = new Array(items.length);
  let nextIndex = 0;
  const workerCount = Math.min(Math.max(1, concurrency), items.length || 1);

  await Promise.all(
    Array.from({ length: workerCount }, async () => {
      while (nextIndex < items.length) {
        const index = nextIndex;
        nextIndex += 1;
        results[index] = await worker(items[index], index);
      }
    }),
  );

  return results;
}

function extractSceneFrameUrls(blueprint, limit) {
  const sprites = blueprint?.originalMapRegion?.sprites;
  if (!sprites || typeof sprites !== "object") return [];

  const urls = [];
  let sourceFrameCount = 0;
  for (const spriteKey of rankSceneSpriteKeys(blueprint)) {
    const sprite = sprites[spriteKey];
    for (const frame of sprite?.frames ?? []) {
      if (typeof frame?.path === "string" && frame.path.startsWith("/original-map/")) {
        sourceFrameCount += 1;
        urls.push(frame.path);
        const renderPath = mapSpriteRenderPrewarmPath(frame.path);
        if (renderPath !== frame.path) urls.push(renderPath);
      }
      if (limit > 0 && sourceFrameCount >= limit) return [...new Set(urls)];
    }
  }

  return [...new Set(urls)];
}

function rankSceneSpriteKeys(blueprint) {
  const region = blueprint?.originalMapRegion;
  const sprites = region?.sprites ?? {};
  const center = scenePrewarmCenter(blueprint);
  const seen = new Map();
  let order = 0;

  for (const cell of region?.cells ?? []) {
    const x = typeof cell?.x === "number" ? cell.x : center.x;
    const y = typeof cell?.y === "number" ? cell.y : center.y;
    const distance = Math.max(Math.abs(x - center.x), Math.abs(y - center.y));
    for (const [layer, key] of [cell.front, cell.middle, cell.tileAnimation, cell.back].entries()) {
      if (!key || !(key in sprites)) continue;
      const priority = sceneSpritePrewarmPriority(sprites[key], layer);
      const previous = seen.get(key);
      if (
        !previous ||
        priority < previous.priority ||
        (priority === previous.priority && distance < previous.distance) ||
        (priority === previous.priority && distance === previous.distance && layer < previous.layer)
      ) {
        seen.set(key, { distance, priority, layer, order });
      }
    }
    order += 1;
  }

  const ranked = Array.from(seen.entries())
    .sort(
      (a, b) =>
        a[1].priority - b[1].priority ||
        a[1].distance - b[1].distance ||
        a[1].layer - b[1].layer ||
        a[1].order - b[1].order,
    )
    .map(([key]) => key);
  for (const key of Object.keys(sprites)) {
    if (!seen.has(key)) ranked.push(key);
  }
  return ranked;
}

function sceneSpritePrewarmPriority(sprite, layer) {
  if (sprite?.drawMode === "object") return 0;
  if (sprite?.kind === "front") return 1;
  if (sprite?.kind === "middle" || sprite?.kind === "tileAnimation") return 2;
  if (sprite?.kind === "back") return 4;
  return 3 + layer;
}

function scenePrewarmCenter(blueprint) {
  const center = blueprint?.sceneView?.center;
  if (typeof center?.x === "number" && typeof center?.y === "number") return center;
  const bounds = blueprint?.originalMapRegion?.playBounds;
  if (bounds) {
    return {
      x: Math.round((bounds.minX + bounds.maxX) / 2),
      y: Math.round((bounds.minY + bounds.maxY) / 2),
    };
  }
  return { x: 0, y: 0 };
}

function normalizeStaticAssetPath(value) {
  if (typeof value !== "string" || !value) return "";
  let url;
  try {
    url = new URL(value, baseUrl);
  } catch {
    return "";
  }
  if (
    !url.pathname.startsWith("/original-ui/") &&
    !url.pathname.startsWith("/original-map/") &&
    !url.pathname.startsWith("/generated/original-map-blend/") &&
    !url.pathname.startsWith("/generated/map-atlas/") &&
    !url.pathname.startsWith("/generated/crystal-packs/full/") &&
    !url.pathname.startsWith("/bevy-entity-atlases/")
  ) {
    return "";
  }
  return url.pathname;
}

function mapSpriteRenderPrewarmPath(value) {
  const frame = value.match(/\/original-map\/WemadeMir2\/Objects\/(27(?:2[3-9]|3[0-2]))\.png$/i)?.[1];
  return frame ? `/generated/original-map-blend/WemadeMir2/Objects/${frame}.png` : value;
}

function decodeAssetRelativePath(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

async function collectSceneSpriteStaticUrls(staticUrls, roots) {
  const records = [];
  const publicRoot = path.join(WEB_ROOT, "public");
  const originalUiRoot = path.join(publicRoot, "original-ui");

  for (const root of roots) {
    const cleanRoot = normalizeSceneSpriteRoot(root);
    if (!cleanRoot) continue;

    const localRoot = path.resolve(originalUiRoot, cleanRoot);
    if (!isPathInside(localRoot, originalUiRoot)) {
      records.push({ root: cleanRoot, ok: false, fileCount: 0, reason: "outside-original-ui-root" });
      continue;
    }

    let stats;
    try {
      stats = await fs.stat(localRoot);
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      records.push({ root: cleanRoot, ok: false, fileCount: 0, reason: "missing-root" });
      continue;
    }

    if (!stats.isDirectory()) {
      records.push({ root: cleanRoot, ok: false, fileCount: 0, reason: "not-a-directory" });
      continue;
    }

    const files = await listFilesRecursive(localRoot);
    for (const filePath of files) {
      const relativePath = path.relative(publicRoot, filePath).split(path.sep).join("/");
      addStaticUrl(staticUrls, `/${relativePath}`, `scene-sprite:${cleanRoot}`);
    }

    records.push({
      root: cleanRoot,
      ok: true,
      fileCount: files.length,
      path: `/original-ui/${cleanRoot}`,
    });
  }

  return records;
}

async function collectPublicAssetRootStaticUrls(staticUrls, roots) {
  const records = [];
  const publicRoot = path.join(WEB_ROOT, "public");

  for (const root of roots) {
    const cleanRoot = normalizePublicAssetRoot(root);
    if (!cleanRoot) continue;

    const localRoot = path.resolve(publicRoot, cleanRoot);
    if (!isPathInside(localRoot, publicRoot)) {
      records.push({ root: cleanRoot, ok: false, fileCount: 0, reason: "outside-public-root" });
      continue;
    }

    let stats;
    try {
      stats = await fs.stat(localRoot);
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      records.push({ root: cleanRoot, ok: false, fileCount: 0, reason: "missing-root" });
      continue;
    }

    if (!stats.isDirectory()) {
      records.push({ root: cleanRoot, ok: false, fileCount: 0, reason: "not-a-directory" });
      continue;
    }

    const files = (await listFilesRecursive(localRoot)).filter((filePath) =>
      PUBLIC_ASSET_EXTENSIONS.has(path.extname(filePath).toLowerCase()),
    );
    for (const filePath of files) {
      const relativePath = path.relative(publicRoot, filePath).split(path.sep).join("/");
      addStaticUrl(staticUrls, `/${relativePath}`, `public-root:${cleanRoot}`);
    }

    records.push({
      root: cleanRoot,
      ok: true,
      fileCount: files.length,
      path: `/${cleanRoot}`,
    });
  }

  return records;
}

async function listFilesRecursive(root) {
  const files = [];
  const stack = [root];

  while (stack.length > 0) {
    const current = stack.pop();
    const entries = await fs.readdir(current, { withFileTypes: true });

    for (const entry of entries) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(entryPath);
        continue;
      }
      if (entry.isFile()) {
        files.push(entryPath);
      }
    }
  }

  return files;
}

function normalizeSceneSpriteRoot(value) {
  const root = String(value || "").trim().replace(/^\/+|\/+$/g, "");
  if (!root || root.includes("..") || root.includes("\\") || path.isAbsolute(root)) return "";
  return root;
}

function normalizePublicAssetRoot(value) {
  const root = String(value || "").trim().replace(/^\/+|\/+$/g, "");
  if (!root || root.includes("..") || root.includes("\\") || path.isAbsolute(root)) return "";
  if (
    root !== "original-ui" &&
    root !== "original-map" &&
    root !== "generated/original-map-blend" &&
    root !== "generated/map-atlas" &&
    root !== "bevy-entity-atlases" &&
    root !== "bevy-runtime"
  ) {
    return "";
  }
  return root;
}

function isPathInside(candidate, root) {
  const relative = path.relative(root, candidate);
  return relative === "" || (!!relative && !relative.startsWith("..") && !path.isAbsolute(relative));
}

async function fetchJson(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url.href}: HTTP ${response.status}`);
  }
  return response.json();
}

function contentTypeForPath(filePath) {
  const ext = path.extname(filePath).toLowerCase();
  if (ext === ".png") return "image/png";
  if (ext === ".jpg" || ext === ".jpeg") return "image/jpeg";
  if (ext === ".gif") return "image/gif";
  if (ext === ".webp") return "image/webp";
  if (ext === ".cur") return "image/x-icon";
  if (ext === ".wav") return "audio/wav";
  if (ext === ".mp3") return "audio/mpeg";
  if (ext === ".ogg") return "audio/ogg";
  if (ext === ".wasm") return "application/wasm";
  if (ext === ".js") return "text/javascript; charset=utf-8";
  if (ext === ".json") return "application/json; charset=utf-8";
  if (ext === ".txt") return "text/plain; charset=utf-8";
  return "application/octet-stream";
}

function joinObjectKey(prefix, relativePath) {
  const cleanPrefix = normalizeObjectPrefix(prefix);
  const cleanPath = relativePath.replace(/^\/+/, "");
  return cleanPrefix ? `${cleanPrefix}/${cleanPath}` : cleanPath;
}

function resolveTemplate(value, version) {
  return String(value).replaceAll("{version}", version);
}

function normalizeObjectPrefix(value) {
  return String(value || "").trim().replace(/^\/+|\/+$/g, "");
}

function makeRange(start, end) {
  const values = [];
  for (let value = start; value <= end; value += 1) {
    values.push(value);
  }
  return values;
}

function normalizeAssetBaseUrl(value) {
  return String(value || "").trim().replace(/\/+$/, "");
}

function normalizeAssetVersion(value) {
  const trimmed = String(value || "").trim();
  if (!trimmed) return "";
  return trimmed.replace(/[^a-zA-Z0-9._-]/g, "-").replace(/-+/g, "-").replace(/^-+|-+$/g, "").slice(0, 80);
}

function normalizeUrl(value) {
  return new URL(String(value)).href.replace(/\/+$/, "");
}

function booleanArg(value, fallback) {
  if (value == null) return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function positiveIntegerArg(value, fallback) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 1) return fallback;
  return Math.floor(parsed);
}

function parseListArg(value, fallback) {
  if (value == null) return fallback;
  const items = Array.isArray(value) ? value : String(value).split(",");
  const parsed = items.map((item) => String(item).trim()).filter(Boolean);
  return parsed.length > 0 ? parsed : fallback;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) continue;
    const key = token.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
