import { createHash } from "node:crypto";
import {
  access,
  mkdir,
  readFile,
  readdir,
  rename,
  stat,
  statfs,
  unlink,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

import {
  decodeFrameRgba,
  decodeMaskFrameRgba,
  normalizeLibraryName,
  parseLibrary,
} from "../crystal-library.mjs";

export const FULL_PACK_SCHEMA_VERSION = 1;
export const FULL_PACK_INDEX_KIND = "mir2-crystal-full-pack-index";
export const LIBRARY_PACK_KIND = "mir2-crystal-library-pack";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const WEB_ROOT = path.resolve(import.meta.dirname, "..", "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const DEFAULT_DATA_DIR = path.join(MIR2_ROOT, "Crystal", "Build", "Client", "Debug", "Data");
const DEFAULT_CATALOG_PATH = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "assets",
  "crystal-pack-catalog.generated.json",
);
const DEFAULT_OUTPUT_DIR = path.join(WEB_ROOT, "public", "generated", "crystal-packs", "full");
const DEFAULT_REPORT_PATH = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "assets",
  "crystal-full-pack-coverage.generated.json",
);
const DEFAULT_URL_ROOT = "/generated/crystal-packs/full";
const DEFAULT_PAGE_SIZE = 4096;
const PREFERRED_PAGE_WIDTH = 2048;
const DEFAULT_PADDING = 1;
const DEFAULT_COMPRESSION_LEVEL = 6;
const DEFAULT_JOBS = 1;
const MIN_PAGE_WIDTH = 64;

// Full conversion runs for thousands of pages. libvips' default process-wide
// cache retains decoded 4K surfaces long after each page has been written.
sharp.cache(false);
sharp.concurrency(1);

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  runFullAssetPackCompiler().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

export async function runFullAssetPackCompiler(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  const mode = String(args.mode ?? "plan").toLowerCase();
  const options = {
    dataDir: path.resolve(args.dataDir ?? DEFAULT_DATA_DIR),
    catalogPath: path.resolve(args.catalog ?? DEFAULT_CATALOG_PATH),
    outputDir: path.resolve(args.output ?? DEFAULT_OUTPUT_DIR),
    reportPath: path.resolve(args.report ?? DEFAULT_REPORT_PATH),
    urlRoot: String(args.urlRoot ?? DEFAULT_URL_ROOT).replace(/\/$/, ""),
    pageSize: positiveInteger(args.pageSize, DEFAULT_PAGE_SIZE),
    padding: nonNegativeInteger(args.padding, DEFAULT_PADDING),
    compressionLevel: boundedInteger(args.compressionLevel, DEFAULT_COMPRESSION_LEVEL, 0, 9),
    jobs: positiveInteger(args.jobs, DEFAULT_JOBS),
    resume: parseBoolean(args.resume, true),
    verifyPages: parseBoolean(args.verifyPages, mode === "verify"),
    categories: parseFilter(args.categories),
    libraries: parseFilter(args.libraries),
    maxLibraries: optionalPositiveInteger(args.maxLibraries),
    apply: parseBoolean(args.apply, false),
  };

  if (options.padding * 2 >= options.pageSize) {
    throw new Error("Crystal full-pack padding must be smaller than half the page size");
  }

  let result;
  if (mode === "plan") {
    result = await planFullAssetPacks(options);
  } else if (mode === "build") {
    result = await buildFullAssetPacks(options);
  } else if (mode === "verify") {
    result = await verifyFullAssetPacks(options);
  } else if (mode === "prune") {
    result = await pruneFullAssetPacks(options);
  } else {
    throw new Error(`Unsupported full-pack mode ${mode}; expected plan, build, verify, or prune`);
  }

  console.log(JSON.stringify(result.consoleSummary, null, 2));
  return result;
}

export async function planFullAssetPacks(options) {
  const resolved = await resolveInputs(options);
  const config = packConfig(resolved);
  const libraries = [];
  const startedAt = Date.now();

  for (let index = 0; index < resolved.entries.length; index += 1) {
    const entry = resolved.entries[index];
    const inspected = await inspectLibraryFile(resolved.dataDir, entry, config);
    libraries.push(planRecord(inspected));
    writeProgress("plan", index + 1, resolved.entries.length, entry.key, inspected.layout.pages.length);
  }

  const body = {
    schemaVersion: FULL_PACK_SCHEMA_VERSION,
    kind: "mir2-crystal-full-pack-plan",
    catalogContentHash: resolved.catalogContentHash,
    config,
    libraries: libraries.sort((left, right) => compareCodePoints(left.key, right.key)),
    summary: summarizePlan(libraries),
  };
  const plan = { ...body, contentHash: semanticHash(body) };
  await mkdir(resolved.outputDir, { recursive: true });
  await atomicWriteJson(path.join(resolved.outputDir, "plan.json"), plan);
  const disk = await diskBudget(resolved.outputDir, plan.summary.conservativeBuildBytes);
  const report = coverageReport("plan", plan.summary, {
    contentHash: plan.contentHash,
    catalogContentHash: resolved.catalogContentHash,
    elapsedMs: Date.now() - startedAt,
    disk,
  });
  await atomicWriteJson(resolved.reportPath, report);
  return {
    plan,
    consoleSummary: {
      ok: true,
      mode: "plan",
      outputDir: resolved.outputDir,
      reportPath: resolved.reportPath,
      contentHash: plan.contentHash,
      ...plan.summary,
      disk,
      elapsedMs: Date.now() - startedAt,
    },
  };
}

export async function buildFullAssetPacks(options) {
  const resolved = await resolveInputs(options);
  const config = packConfig(resolved);
  const startedAt = Date.now();
  await mkdir(path.join(resolved.outputDir, "pages"), { recursive: true });
  await mkdir(path.join(resolved.outputDir, "libraries"), { recursive: true });

  const planPath = path.join(resolved.outputDir, "plan.json");
  let conservativeBuildBytes = resolved.entries.reduce((sum, entry) => sum + entry.sourceBytes * 2, 0);
  try {
    const priorPlan = JSON.parse(await readFile(planPath, "utf8"));
    if (priorPlan?.config?.contentHash === config.contentHash) {
      conservativeBuildBytes = priorPlan.summary?.conservativeBuildBytes ?? conservativeBuildBytes;
    }
  } catch {
    // A plan is recommended but not required; build remains self-contained.
  }
  const disk = await diskBudget(resolved.outputDir, conservativeBuildBytes);
  if (disk.freeBytes !== null && disk.freeBytes < disk.requiredBytes) {
    throw new Error(
      `Insufficient disk for Crystal full pack: need ${disk.requiredBytes} bytes, have ${disk.freeBytes} bytes`,
    );
  }

  let completed = 0;
  let resumed = 0;
  const records = await mapWithConcurrency(resolved.entries, resolved.jobs, async (entry) => {
    const built = await buildLibraryPack(resolved, entry, config);
    completed += 1;
    if (built.resumed) resumed += 1;
    writeProgress(built.resumed ? "resume" : "build", completed, resolved.entries.length, entry.key, built.record.pageCount);
    return built.record;
  });

  const sortedRecords = records.sort((left, right) => compareCodePoints(left.key, right.key));
  const librariesByKey = Object.fromEntries(sortedRecords.map((record) => [record.key, record]));
  const summary = summarizeIndex(sortedRecords);
  const body = {
    schemaVersion: FULL_PACK_SCHEMA_VERSION,
    kind: FULL_PACK_INDEX_KIND,
    sourceContentHash: resolved.catalogContentHash,
    catalogContentHash: resolved.catalogContentHash,
    textureFormat: "png-rgba8-srgb",
    sampler: "nearest",
    config,
    libraries: sortedRecords,
    librariesByKey,
    summary,
  };
  const index = { ...body, contentHash: semanticHash(body) };
  validateFullPackIndex(index);
  await atomicWriteJson(path.join(resolved.outputDir, "index.json"), index);

  const report = coverageReport("build", summary, {
    contentHash: index.contentHash,
    catalogContentHash: resolved.catalogContentHash,
    resumedLibraryCount: resumed,
    builtLibraryCount: sortedRecords.length - resumed,
    elapsedMs: Date.now() - startedAt,
    disk,
  });
  await atomicWriteJson(resolved.reportPath, report);
  return {
    index,
    consoleSummary: {
      ok: true,
      mode: "build",
      outputDir: resolved.outputDir,
      reportPath: resolved.reportPath,
      contentHash: index.contentHash,
      resumedLibraryCount: resumed,
      builtLibraryCount: sortedRecords.length - resumed,
      ...summary,
      elapsedMs: Date.now() - startedAt,
    },
  };
}

export async function verifyFullAssetPacks(options) {
  const resolved = await resolveInputs(options);
  const startedAt = Date.now();
  const indexPath = path.join(resolved.outputDir, "index.json");
  const index = JSON.parse(await readFile(indexPath, "utf8"));
  validateFullPackIndex(index);

  const expectedByKey = new Map(resolved.entries.map((entry) => [entry.key, entry]));
  if (index.libraries.length !== expectedByKey.size) {
    throw new Error(
      `Full-pack index library count ${index.libraries.length} does not match selected source count ${expectedByKey.size}`,
    );
  }

  const checkedPages = new Set();
  let verifiedLibraries = 0;
  for (const record of [...index.libraries].sort((a, b) => compareCodePoints(a.key, b.key))) {
    const expected = expectedByKey.get(record.key);
    if (!expected) throw new Error(`Full-pack index contains unexpected library ${record.key}`);
    if (record.sourceSha256 !== expected.sourceSha256) {
      throw new Error(`Source hash drift for ${record.key}`);
    }
    const manifestPath = publicFileForUrl(resolved.outputDir, resolved.urlRoot, record.manifestUrl);
    const manifestBytes = await readFile(manifestPath);
    if (sha256(manifestBytes) !== record.manifestSha256) {
      throw new Error(`Manifest file hash mismatch for ${record.key}`);
    }
    const manifest = JSON.parse(manifestBytes.toString("utf8"));
    validateLibraryPack(manifest);
    crossValidateIndexRecord(record, manifest);
    if (resolved.verifyPages) {
      for (const page of manifest.pages) {
        if (checkedPages.has(page.sha256)) continue;
        const pagePath = publicFileForUrl(resolved.outputDir, resolved.urlRoot, page.imageUrl);
        const bytes = await readFile(pagePath);
        if (bytes.byteLength !== page.networkBytes || sha256(bytes) !== page.sha256) {
          throw new Error(`CAS page hash mismatch for ${page.imageUrl}`);
        }
        checkedPages.add(page.sha256);
      }
    }
    verifiedLibraries += 1;
    writeProgress("verify", verifiedLibraries, expectedByKey.size, record.key, record.pageCount);
  }

  const report = coverageReport("verify", index.summary, {
    contentHash: index.contentHash,
    catalogContentHash: index.catalogContentHash,
    verifiedLibraryCount: verifiedLibraries,
    verifiedUniquePageCount: checkedPages.size,
    pageHashesVerified: resolved.verifyPages,
    elapsedMs: Date.now() - startedAt,
  });
  await atomicWriteJson(resolved.reportPath, report);
  return {
    index,
    consoleSummary: {
      ok: true,
      mode: "verify",
      contentHash: index.contentHash,
      verifiedLibraryCount: verifiedLibraries,
      verifiedUniquePageCount: checkedPages.size,
      pageHashesVerified: resolved.verifyPages,
      ...index.summary,
      elapsedMs: Date.now() - startedAt,
    },
  };
}

export async function pruneFullAssetPacks(options) {
  const resolved = await resolveInputs(options);
  if (resolved.categories || resolved.libraries || resolved.maxLibraries) {
    throw new Error("Full-pack prune requires the complete unfiltered index");
  }

  const startedAt = Date.now();
  const indexPath = path.join(resolved.outputDir, "index.json");
  const index = JSON.parse(await readFile(indexPath, "utf8"));
  validateFullPackIndex(index);
  if (index.catalogContentHash !== resolved.catalogContentHash) {
    throw new Error("Full-pack prune refused because index and source catalog hashes differ");
  }

  const retainedManifests = new Set();
  const retainedPages = new Set();
  for (const record of index.libraries) {
    const manifestPath = publicFileForUrl(resolved.outputDir, resolved.urlRoot, record.manifestUrl);
    const manifestBytes = await readFile(manifestPath);
    if (sha256(manifestBytes) !== record.manifestSha256) {
      throw new Error(`Full-pack prune refused because manifest hash drifted for ${record.key}`);
    }
    const manifest = JSON.parse(manifestBytes.toString("utf8"));
    validateLibraryPack(manifest);
    crossValidateIndexRecord(record, manifest);
    retainedManifests.add(manifestPath);
    for (const page of manifest.pages) {
      retainedPages.add(publicFileForUrl(resolved.outputDir, resolved.urlRoot, page.imageUrl));
    }
  }

  const manifestRoot = path.join(resolved.outputDir, "libraries");
  const pageRoot = path.join(resolved.outputDir, "pages");
  const manifestFiles = await listRegularFiles(manifestRoot);
  const pageFiles = await listRegularFiles(pageRoot);
  const orphanManifests = manifestFiles.filter((filePath) => !retainedManifests.has(filePath));
  const orphanPages = pageFiles.filter((filePath) => !retainedPages.has(filePath));
  const [orphanManifestBytes, orphanPageBytes] = await Promise.all([
    totalFileBytes(orphanManifests),
    totalFileBytes(orphanPages),
  ]);

  if (resolved.apply) {
    for (const filePath of [...orphanManifests, ...orphanPages]) {
      assertInsideDirectory(resolved.outputDir, filePath);
      await unlink(filePath);
    }
  }

  const consoleSummary = {
    ok: true,
    mode: "prune",
    applied: resolved.apply,
    outputDir: resolved.outputDir,
    retainedManifestCount: retainedManifests.size,
    retainedPageCount: retainedPages.size,
    orphanManifestCount: orphanManifests.length,
    orphanManifestBytes,
    orphanPageCount: orphanPages.length,
    orphanPageBytes,
    reclaimedBytes: resolved.apply ? orphanManifestBytes + orphanPageBytes : 0,
    elapsedMs: Date.now() - startedAt,
  };
  return { index, consoleSummary };
}

export async function buildLibraryPack(resolved, entry, config) {
  const manifestRelativePath = manifestRelativePathFor(entry, config);
  const manifestPath = path.join(resolved.outputDir, ...manifestRelativePath.split("/"));
  const manifestUrl = `${resolved.urlRoot}/${manifestRelativePath}`;

  if (resolved.resume && await exists(manifestPath)) {
    try {
      const bytes = await readFile(manifestPath);
      const manifest = JSON.parse(bytes.toString("utf8"));
      validateLibraryPack(manifest);
      if (
        manifest.id === entry.key &&
        manifest.category === entry.category &&
        manifest.source.sha256 === entry.sourceSha256 &&
        manifest.config.contentHash === config.contentHash &&
        await allManifestPagesExist(resolved, manifest)
      ) {
        return {
          resumed: true,
          record: indexRecordFor(entry, manifest, manifestUrl, sha256(bytes)),
        };
      }
    } catch {
      // Immutable manifest names include source/config hashes. Invalid content is
      // rebuilt and atomically replaced below rather than trusted as a resume hit.
    }
  }

  const inspected = await inspectLibraryFile(resolved.dataDir, entry, config);
  const pageRecords = [];
  const placements = new Map();
  for (let pageIndex = 0; pageIndex < inspected.layout.pages.length; pageIndex += 1) {
    const page = inspected.layout.pages[pageIndex];
    const pageId = `p${pageIndex}`;
    const png = await renderPage(inspected.library, page, config.padding, resolved.compressionLevel);
    const pageHash = sha256(png);
    const imageUrl = await writeCasPage(resolved.outputDir, resolved.urlRoot, pageHash, png);
    const pageKey = `sha256:${pageHash}`;
    const rects = page.sources.map((source) => ({
      key: source.key,
      x: source.x + config.padding,
      y: source.y + config.padding,
      width: source.width,
      height: source.height,
      sourceKind: source.role,
    }));
    for (const source of page.sources) {
      placements.set(source.key, {
        page: pageId,
        pageKey,
        rectKey: source.key,
        imageUrl,
        x: source.x + config.padding,
        y: source.y + config.padding,
        width: source.width,
        height: source.height,
      });
    }
    pageRecords.push({
      id: pageId,
      key: pageKey,
      sha256: pageHash,
      width: page.width,
      height: page.height,
      networkBytes: png.byteLength,
      gpuBytes: page.width * page.height * 4,
      imageUrl,
      rectCount: page.sources.length,
      rects,
    });
  }

  const frames = inspected.framePlans.map((framePlan) => frameRecord(framePlan, placements));
  const summary = {
    frameSlotCount: inspected.library.count,
    packedFrameCount: frames.filter((frame) => frame.status === "packed").length,
    noDrawFrameCount: frames.filter((frame) => frame.status === "noDraw").length,
    packedMaskCount: frames.filter((frame) => frame.mask?.status === "packed").length,
    noDrawMaskCount: frames.filter((frame) => frame.mask?.status === "noDraw").length,
    actionCount: inspected.library.frameSet.count,
    pageCount: pageRecords.length,
    networkBytes: pageRecords.reduce((sum, page) => sum + page.networkBytes, 0),
    gpuBytes: pageRecords.reduce((sum, page) => sum + page.gpuBytes, 0),
    rawRgbaBytes: inspected.rawRgbaBytes,
    maxSourceWidth: inspected.maxSourceWidth,
    maxSourceHeight: inspected.maxSourceHeight,
  };
  summary.drawableFrameCount = summary.packedFrameCount;
  summary.maskFrameCount = summary.packedMaskCount;
  summary.rectCount = summary.drawableFrameCount + summary.maskFrameCount;
  const body = {
    schemaVersion: FULL_PACK_SCHEMA_VERSION,
    kind: LIBRARY_PACK_KIND,
    sourceContentHash: resolved.catalogContentHash,
    libraryKey: entry.key,
    sourceSha256: entry.sourceSha256,
    shardUrl: manifestUrl,
    frameSlotCount: inspected.library.count,
    id: entry.key,
    category: entry.category,
    source: {
      path: entry.sourcePath,
      sha256: entry.sourceSha256,
      bytes: entry.sourceBytes,
      version: inspected.library.version,
    },
    textureFormat: "png-rgba8-srgb",
    sampler: "nearest",
    config,
    frameSet: {
      count: inspected.library.frameSet.count,
      actions: inspected.library.frameSet.actions,
    },
    pages: pageRecords,
    frames,
    summary,
  };
  const manifest = { ...body, contentHash: semanticHash(body) };
  validateLibraryPack(manifest);
  const manifestBytes = Buffer.from(`${canonicalJson(manifest)}\n`, "utf8");
  await mkdir(path.dirname(manifestPath), { recursive: true });
  await atomicWriteFile(manifestPath, manifestBytes);
  return {
    resumed: false,
    record: indexRecordFor(entry, manifest, manifestUrl, sha256(manifestBytes)),
  };
}

export async function inspectLibraryFile(dataDir, entry, config) {
  const filePath = path.join(dataDir, ...entry.sourcePath.split("/"));
  const buffer = await readFile(filePath);
  const sourceSha256 = sha256(buffer);
  if (sourceSha256 !== entry.sourceSha256) {
    throw new Error(`Crystal source hash mismatch for ${entry.key}: expected ${entry.sourceSha256}, got ${sourceSha256}`);
  }
  if (buffer.byteLength !== entry.sourceBytes) {
    throw new Error(`Crystal source byte count mismatch for ${entry.key}`);
  }
  const library = parseLibrary(buffer);
  if (library.count !== entry.frameSlotCount || library.version !== entry.version) {
    throw new Error(`Crystal catalog metadata drift for ${entry.key}`);
  }

  const sources = [];
  const framePlans = [];
  let rawRgbaBytes = 0;
  for (let index = 0; index < library.count; index += 1) {
    const frame = library.frames[index];
    if (!frame) {
      framePlans.push({ index, status: "noDraw", reason: "empty-offset", frame: null, maskPlan: null });
      continue;
    }
    const base = { index, frame };
    if (frame.width <= 0 || frame.height <= 0) {
      framePlans.push({ ...base, status: "noDraw", reason: "non-positive-dimensions", maskPlan: maskPlanFor(frame, null) });
      continue;
    }
    if (frame.dataLength <= 0) {
      framePlans.push({ ...base, status: "noDraw", reason: "empty-image-data", maskPlan: maskPlanFor(frame, null) });
      continue;
    }
    const imageKey = `${entry.key}#${index}`;
    sources.push({
      key: imageKey,
      frameIndex: index,
      role: "image",
      width: frame.width,
      height: frame.height,
    });
    rawRgbaBytes += checkedRgbaBytes(frame.width, frame.height, imageKey);

    let maskPlan = null;
    if (frame.maskRgba) {
      if (frame.maskWidth > 0 && frame.maskHeight > 0 && frame.maskLength > 0) {
        const maskKey = `${imageKey}:mask`;
        sources.push({
          key: maskKey,
          frameIndex: index,
          role: "mask",
          width: frame.maskWidth,
          height: frame.maskHeight,
        });
        rawRgbaBytes += checkedRgbaBytes(frame.maskWidth, frame.maskHeight, maskKey);
        maskPlan = { status: "packed", key: maskKey };
      } else {
        maskPlan = { status: "noDraw", reason: "invalid-mask-payload" };
      }
    }
    framePlans.push({ ...base, status: "packed", imageKey, maskPlan });
  }
  const layout = packSourceDescriptors(sources, config);
  const maxSourceWidth = sources.reduce((max, source) => Math.max(max, source.width), 0);
  const maxSourceHeight = sources.reduce((max, source) => Math.max(max, source.height), 0);
  return { entry, library, framePlans, layout, rawRgbaBytes, maxSourceWidth, maxSourceHeight };
}

export function packSourceDescriptors(sources, { pageSize, padding }) {
  if (sources.length === 0) return { width: 0, pages: [], gpuBytes: 0 };
  const sorted = [...sources].sort(compareSources);
  const maxOuterWidth = sorted.reduce((max, source) => Math.max(max, source.width + padding * 2), 0);
  const maxOuterHeight = sorted.reduce((max, source) => Math.max(max, source.height + padding * 2), 0);
  if (maxOuterWidth > pageSize || maxOuterHeight > pageSize) {
    const source = sorted.find(
      (candidate) => candidate.width + padding * 2 > pageSize || candidate.height + padding * 2 > pageSize,
    );
    throw new Error(
      `Crystal source ${source.key} (${source.width}x${source.height}) exceeds ${pageSize}px page budget`,
    );
  }

  const firstWidth = Math.max(MIN_PAGE_WIDTH, nextPowerOfTwo(maxOuterWidth));
  let best = null;
  const lastWidth = Math.min(pageSize, Math.max(PREFERRED_PAGE_WIDTH, firstWidth));
  for (let width = firstWidth; width <= lastWidth; width *= 2) {
    const candidate = shelfPack(sorted, width, pageSize, padding);
    const gpuBytes = candidate.pages.reduce((sum, page) => sum + page.width * page.height * 4, 0);
    const score = [gpuBytes, candidate.pages.length, -width];
    if (!best || compareScore(score, best.score) < 0) {
      best = { width, pages: candidate.pages, gpuBytes, score };
    }
  }
  return { width: best.width, pages: best.pages, gpuBytes: best.gpuBytes };
}

export function validateLibraryPack(manifest) {
  if (
    manifest?.schemaVersion !== FULL_PACK_SCHEMA_VERSION ||
    manifest.kind !== LIBRARY_PACK_KIND ||
    typeof manifest.id !== "string" ||
    !manifest.id
  ) {
    throw new Error("Invalid Crystal library-pack schema");
  }
  validateContentHash(manifest, "Crystal library pack");
  if (!manifest.source || !/^[a-f0-9]{64}$/.test(manifest.source.sha256 ?? "")) {
    throw new Error(`Invalid source metadata for ${manifest.id}`);
  }
  if (manifest.frameSet?.count !== manifest.frameSet?.actions?.length) {
    throw new Error(`FrameSet count mismatch for ${manifest.id}`);
  }

  const pages = new Map();
  for (const page of manifest.pages ?? []) {
    if (pages.has(page.id)) throw new Error(`Duplicate page id ${page.id} in ${manifest.id}`);
    if (page.key !== `sha256:${page.sha256}` || !/^[a-f0-9]{64}$/.test(page.sha256 ?? "")) {
      throw new Error(`Invalid page hash ${page.id} in ${manifest.id}`);
    }
    if (page.width <= 0 || page.height <= 0 || page.networkBytes <= 0) {
      throw new Error(`Invalid page dimensions/bytes ${page.id} in ${manifest.id}`);
    }
    pages.set(page.id, page);
  }

  const frames = manifest.frames ?? [];
  if (frames.length !== manifest.summary?.frameSlotCount) {
    throw new Error(`Frame completeness mismatch for ${manifest.id}`);
  }
  let packedFrameCount = 0;
  let noDrawFrameCount = 0;
  let packedMaskCount = 0;
  let noDrawMaskCount = 0;
  for (let index = 0; index < frames.length; index += 1) {
    const frame = frames[index];
    if (frame?.index !== index) throw new Error(`Non-contiguous frame ${index} in ${manifest.id}`);
    if (frame.status === "packed") {
      packedFrameCount += 1;
      validatePlacement(frame.image, pages, manifest.config.padding, `${manifest.id}#${index}`);
    } else if (frame.status === "noDraw" && typeof frame.reason === "string") {
      noDrawFrameCount += 1;
    } else {
      throw new Error(`Unclassified frame ${manifest.id}#${index}`);
    }
    if (frame.mask?.status === "packed") {
      packedMaskCount += 1;
      validatePlacement(frame.mask.image, pages, manifest.config.padding, `${manifest.id}#${index}:mask`);
    } else if (frame.mask?.status === "noDraw") {
      noDrawMaskCount += 1;
    }
  }
  const expected = manifest.summary;
  if (
    packedFrameCount !== expected.packedFrameCount ||
    noDrawFrameCount !== expected.noDrawFrameCount ||
    packedMaskCount !== expected.packedMaskCount ||
    noDrawMaskCount !== expected.noDrawMaskCount ||
    packedFrameCount + noDrawFrameCount !== expected.frameSlotCount ||
    pages.size !== expected.pageCount
  ) {
    throw new Error(`Summary completeness mismatch for ${manifest.id}`);
  }
  return true;
}

export function validateFullPackIndex(index) {
  if (index?.schemaVersion !== FULL_PACK_SCHEMA_VERSION || index.kind !== FULL_PACK_INDEX_KIND) {
    throw new Error("Invalid Crystal full-pack index schema");
  }
  validateContentHash(index, "Crystal full-pack index");
  if (!Array.isArray(index.libraries)) throw new Error("Crystal full-pack index libraries must be an array");
  const records = index.libraries;
  const seenKeys = new Set();
  for (const record of records) {
    if (record.libraryKey !== record.key || seenKeys.has(record.key)) {
      throw new Error(`Full-pack key mismatch for ${record.key}`);
    }
    seenKeys.add(record.key);
    if (index.librariesByKey?.[record.key]?.manifestSha256 !== record.manifestSha256) {
      throw new Error(`Full-pack librariesByKey mismatch for ${record.key}`);
    }
    if (!/^[a-f0-9]{64}$/.test(record.sourceSha256 ?? "")) {
      throw new Error(`Invalid source hash for ${record.key}`);
    }
    if (!/^[a-f0-9]{64}$/.test(record.manifestSha256 ?? "") || !record.manifestUrl) {
      throw new Error(`Invalid manifest reference for ${record.key}`);
    }
    if (record.packedFrameCount + record.noDrawFrameCount !== record.frameSlotCount) {
      throw new Error(`Index frame completeness mismatch for ${record.key}`);
    }
  }
  const summary = summarizeIndex(records);
  for (const [key, value] of Object.entries(summary)) {
    if (index.summary?.[key] !== value) throw new Error(`Full-pack summary mismatch for ${key}`);
  }
  return true;
}

function frameRecord(plan, placements) {
  const frame = plan.frame;
  const metadata = frame
    ? {
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
        shadowX: frame.shadowX,
        shadowY: frame.shadowY,
        shadow: frame.shadow,
      }
    : {};
  if (plan.status === "noDraw") {
    return {
      index: plan.index,
      status: "noDraw",
      noDraw: true,
      reason: plan.reason,
      ...metadata,
      mask: noDrawMaskRecord(frame, plan.maskPlan),
    };
  }
  const image = placements.get(plan.imageKey);
  if (!image) throw new Error(`Missing packed placement for ${plan.imageKey}`);
  let mask = null;
  if (plan.maskPlan?.status === "packed") {
    const maskImage = placements.get(plan.maskPlan.key);
    if (!maskImage) throw new Error(`Missing packed mask placement for ${plan.maskPlan.key}`);
    mask = {
      status: "packed",
      width: frame.maskWidth,
      height: frame.maskHeight,
      x: frame.maskX,
      y: frame.maskY,
      image: maskImage,
    };
  } else if (plan.maskPlan?.status === "noDraw") {
    mask = {
      status: "noDraw",
      reason: plan.maskPlan.reason,
      width: frame.maskWidth ?? null,
      height: frame.maskHeight ?? null,
      x: frame.maskX ?? null,
      y: frame.maskY ?? null,
    };
  }
  return {
    index: plan.index,
    status: "packed",
    noDraw: false,
    ...metadata,
    image,
    pageKey: image.pageKey,
    rectKey: image.rectKey,
    imageUrl: image.imageUrl,
    mask,
    maskPageKey: mask?.status === "packed" ? mask.image.pageKey : null,
    maskRectKey: mask?.status === "packed" ? mask.image.rectKey : null,
    maskImageUrl: mask?.status === "packed" ? mask.image.imageUrl : null,
    maskWidth: frame.maskWidth ?? null,
    maskHeight: frame.maskHeight ?? null,
    maskX: frame.maskX ?? null,
    maskY: frame.maskY ?? null,
  };
}

function noDrawMaskRecord(frame, maskPlan) {
  if (!frame?.maskRgba) return null;
  return {
    status: "noDraw",
    reason: maskPlan?.reason ?? "parent-frame-no-draw",
    width: frame.maskWidth ?? null,
    height: frame.maskHeight ?? null,
    x: frame.maskX ?? null,
    y: frame.maskY ?? null,
  };
}

function maskPlanFor(frame, packedPlan) {
  if (!frame?.maskRgba) return null;
  return packedPlan ?? { status: "noDraw", reason: "parent-frame-no-draw" };
}

async function renderPage(library, page, padding, compressionLevel) {
  const pixels = Buffer.alloc(page.width * page.height * 4);
  for (const source of page.sources) {
    const frame = library.frames[source.frameIndex];
    const rgba = source.role === "mask" ? decodeMaskFrameRgba(library, frame) : decodeFrameRgba(library, frame);
    const expectedBytes = checkedRgbaBytes(source.width, source.height, source.key);
    if (rgba.byteLength !== expectedBytes) {
      throw new Error(`Decoded RGBA byte count mismatch for ${source.key}`);
    }
    blitExtrudedRgba(
      pixels,
      page.width,
      page.height,
      rgba,
      source.width,
      source.height,
      source.x,
      source.y,
      padding,
    );
  }
  return sharp(pixels, { raw: { width: page.width, height: page.height, channels: 4 } })
    .png({ compressionLevel, adaptiveFiltering: false, palette: false })
    .toBuffer();
}

export function blitExtrudedRgba(
  target,
  targetWidth,
  targetHeight,
  source,
  sourceWidth,
  sourceHeight,
  outerX,
  outerY,
  padding,
) {
  const outerWidth = sourceWidth + padding * 2;
  const outerHeight = sourceHeight + padding * 2;
  if (outerX < 0 || outerY < 0 || outerX + outerWidth > targetWidth || outerY + outerHeight > targetHeight) {
    throw new Error("Atlas blit exceeds page bounds");
  }
  const rowBytes = sourceWidth * 4;
  for (let y = 0; y < sourceHeight; y += 1) {
    const sourceOffset = y * rowBytes;
    const targetOffset = ((outerY + padding + y) * targetWidth + outerX + padding) * 4;
    source.copy(target, targetOffset, sourceOffset, sourceOffset + rowBytes);
    for (let x = 0; x < padding; x += 1) {
      source.copy(target, targetOffset - (x + 1) * 4, sourceOffset, sourceOffset + 4);
      source.copy(
        target,
        targetOffset + rowBytes + x * 4,
        sourceOffset + rowBytes - 4,
        sourceOffset + rowBytes,
      );
    }
  }
  const outerRowBytes = outerWidth * 4;
  const firstRowOffset = ((outerY + padding) * targetWidth + outerX) * 4;
  const lastRowOffset = ((outerY + padding + sourceHeight - 1) * targetWidth + outerX) * 4;
  for (let y = 0; y < padding; y += 1) {
    target.copy(target, ((outerY + y) * targetWidth + outerX) * 4, firstRowOffset, firstRowOffset + outerRowBytes);
    target.copy(
      target,
      ((outerY + padding + sourceHeight + y) * targetWidth + outerX) * 4,
      lastRowOffset,
      lastRowOffset + outerRowBytes,
    );
  }
}

function shelfPack(sorted, width, pageSize, padding) {
  const pages = [];
  let page = newShelfPage(width);
  const flush = () => {
    if (page.sources.length === 0) return;
    page.height = nextPowerOfTwo(page.cursorY + page.rowHeight);
    pages.push(page);
  };
  for (const source of sorted) {
    const outerWidth = source.width + padding * 2;
    const outerHeight = source.height + padding * 2;
    if (page.cursorX + outerWidth > width) {
      page.cursorX = 0;
      page.cursorY += page.rowHeight;
      page.rowHeight = 0;
    }
    if (page.cursorY + outerHeight > pageSize) {
      flush();
      page = newShelfPage(width);
    }
    page.sources.push({ ...source, x: page.cursorX, y: page.cursorY });
    page.cursorX += outerWidth;
    page.rowHeight = Math.max(page.rowHeight, outerHeight);
  }
  flush();
  return { pages };
}

function newShelfPage(width) {
  return { width, height: 0, cursorX: 0, cursorY: 0, rowHeight: 0, sources: [] };
}

async function resolveInputs(options) {
  const catalogPath = path.resolve(options.catalogPath ?? DEFAULT_CATALOG_PATH);
  const catalog = JSON.parse(await readFile(catalogPath, "utf8"));
  const entries = flattenCatalog(catalog, options.categories, options.libraries, options.maxLibraries);
  if (entries.length === 0) throw new Error("Crystal full-pack selection contains no libraries");
  return {
    ...options,
    dataDir: path.resolve(options.dataDir ?? DEFAULT_DATA_DIR),
    outputDir: path.resolve(options.outputDir ?? DEFAULT_OUTPUT_DIR),
    reportPath: path.resolve(options.reportPath ?? DEFAULT_REPORT_PATH),
    urlRoot: String(options.urlRoot ?? DEFAULT_URL_ROOT).replace(/\/$/, ""),
    pageSize: options.pageSize ?? DEFAULT_PAGE_SIZE,
    padding: options.padding ?? DEFAULT_PADDING,
    compressionLevel: options.compressionLevel ?? DEFAULT_COMPRESSION_LEVEL,
    jobs: options.jobs ?? DEFAULT_JOBS,
    resume: options.resume ?? true,
    verifyPages: options.verifyPages ?? false,
    catalogContentHash: catalog.catalog?.contentHash ?? catalog.release?.contentHash ?? catalog.contentHash,
    entries,
  };
}

function flattenCatalog(catalog, categoryFilter, libraryFilter, maxLibraries) {
  if (!Array.isArray(catalog?.packs)) throw new Error("Crystal pack catalog has no packs");
  const categories = categoryFilter?.length ? new Set(categoryFilter) : null;
  const libraries = libraryFilter?.length
    ? new Set(libraryFilter.map((value) => normalizeLibraryName(value).replace(/\.Lib$/i, "")))
    : null;
  const entries = [];
  const foldedKeys = new Set();
  for (const pack of catalog.packs) {
    if (categories && !categories.has(pack.category)) continue;
    for (const library of pack.libraries ?? []) {
      if (library.status !== "ok") throw new Error(`Catalog source is not parseable: ${library.path}`);
      const sourcePath = normalizeLibraryName(library.path);
      const key = sourcePath.replace(/\.Lib$/i, "");
      if (libraries && !libraries.has(key)) continue;
      const folded = key.toLowerCase();
      if (foldedKeys.has(folded)) throw new Error(`Case-insensitive duplicate Crystal library key ${key}`);
      foldedKeys.add(folded);
      entries.push({
        key,
        category: pack.category,
        sourcePath,
        sourceSha256: library.sha256,
        sourceBytes: library.byteLength,
        version: library.version,
        frameSlotCount: library.frameSlotCount,
        presentFrameCount: library.presentFrameCount,
      });
    }
  }
  entries.sort((left, right) => compareCodePoints(left.key, right.key));
  if (libraries) {
    const missing = [...libraries].filter((key) => !entries.some((entry) => entry.key === key));
    if (missing.length) throw new Error(`Unknown Crystal libraries: ${missing.join(", ")}`);
  }
  return maxLibraries ? entries.slice(0, maxLibraries) : entries;
}

function packConfig(options) {
  const body = {
    schemaVersion: FULL_PACK_SCHEMA_VERSION,
    pageSize: options.pageSize,
    padding: options.padding,
    pngCompressionLevel: options.compressionLevel,
    textureFormat: "png-rgba8-srgb",
    sampler: "nearest",
    decoder: "crystal-bgra-direct3d-row-pitch-v1",
    packer: "deterministic-best-width-shelf-v1",
    preferredPageWidth: PREFERRED_PAGE_WIDTH,
  };
  return { ...body, contentHash: semanticHash(body) };
}

function planRecord(inspected) {
  const packedFrameCount = inspected.framePlans.filter((frame) => frame.status === "packed").length;
  const noDrawFrameCount = inspected.framePlans.length - packedFrameCount;
  const packedMaskCount = inspected.framePlans.filter((frame) => frame.maskPlan?.status === "packed").length;
  const noDrawMaskCount = inspected.framePlans.filter((frame) => frame.maskPlan?.status === "noDraw").length;
  return {
    key: inspected.entry?.key,
    category: inspected.entry?.category,
    sourceBytes: inspected.entry?.sourceBytes,
    frameSlotCount: inspected.library.count,
    packedFrameCount,
    noDrawFrameCount,
    packedMaskCount,
    noDrawMaskCount,
    actionCount: inspected.library.frameSet.count,
    pageCount: inspected.layout.pages.length,
    gpuBytes: inspected.layout.gpuBytes,
    rawRgbaBytes: inspected.rawRgbaBytes,
    maxSourceWidth: inspected.maxSourceWidth,
    maxSourceHeight: inspected.maxSourceHeight,
  };
}

function summarizePlan(libraries) {
  const summary = libraries.reduce(
    (total, library) => {
      for (const key of [
        "frameSlotCount",
        "packedFrameCount",
        "noDrawFrameCount",
        "packedMaskCount",
        "noDrawMaskCount",
        "actionCount",
        "pageCount",
        "gpuBytes",
        "rawRgbaBytes",
      ]) total[key] += library[key];
      total.totalSourceBytes += library.sourceBytes;
      total.maxSourceWidth = Math.max(total.maxSourceWidth, library.maxSourceWidth);
      total.maxSourceHeight = Math.max(total.maxSourceHeight, library.maxSourceHeight);
      return total;
    },
    {
      libraryCount: libraries.length,
      frameSlotCount: 0,
      packedFrameCount: 0,
      noDrawFrameCount: 0,
      packedMaskCount: 0,
      noDrawMaskCount: 0,
      actionCount: 0,
      pageCount: 0,
      gpuBytes: 0,
      rawRgbaBytes: 0,
      totalSourceBytes: 0,
      maxSourceWidth: 0,
      maxSourceHeight: 0,
    },
  );
  summary.estimatedManifestBytes = summary.frameSlotCount * 280 + summary.pageCount * 240;
  summary.estimatedNetworkBytes = Math.ceil(summary.totalSourceBytes * 1.25);
  summary.conservativeBuildBytes = Math.ceil(
    summary.estimatedManifestBytes * 1.25 + summary.totalSourceBytes * 2,
  );
  summary.drawableFrameCount = summary.packedFrameCount;
  summary.maskFrameCount = summary.packedMaskCount;
  summary.rectCount = summary.drawableFrameCount + summary.maskFrameCount;
  return summary;
}

function summarizeIndex(records) {
  const summary = records.reduce(
    (total, record) => {
      total.libraryCount += 1;
      for (const key of [
        "frameSlotCount",
        "packedFrameCount",
        "noDrawFrameCount",
        "packedMaskCount",
        "noDrawMaskCount",
        "actionCount",
        "pageCount",
        "networkBytes",
        "gpuBytes",
        "rawRgbaBytes",
      ]) total[key] += record[key];
      total.totalSourceBytes += record.sourceBytes;
      total.maxSourceWidth = Math.max(total.maxSourceWidth, record.maxSourceWidth);
      total.maxSourceHeight = Math.max(total.maxSourceHeight, record.maxSourceHeight);
      return total;
    },
    {
      libraryCount: 0,
      frameSlotCount: 0,
      packedFrameCount: 0,
      noDrawFrameCount: 0,
      packedMaskCount: 0,
      noDrawMaskCount: 0,
      actionCount: 0,
      pageCount: 0,
      networkBytes: 0,
      gpuBytes: 0,
      rawRgbaBytes: 0,
      totalSourceBytes: 0,
      maxSourceWidth: 0,
      maxSourceHeight: 0,
    },
  );
  summary.drawableFrameCount = summary.packedFrameCount;
  summary.maskFrameCount = summary.packedMaskCount;
  summary.rectCount = summary.drawableFrameCount + summary.maskFrameCount;
  return summary;
}

function indexRecordFor(entry, manifest, manifestUrl, manifestSha256) {
  return {
    key: entry.key,
    libraryKey: entry.key,
    category: entry.category,
    sourcePath: entry.sourcePath,
    sourceSha256: entry.sourceSha256,
    sourceBytes: entry.sourceBytes,
    version: entry.version,
    manifestUrl,
    shardUrl: manifestUrl,
    manifestSha256,
    contentHash: manifest.contentHash,
    ...manifest.summary,
  };
}

function crossValidateIndexRecord(record, manifest) {
  if (
    record.key !== manifest.id ||
    record.category !== manifest.category ||
    record.sourceSha256 !== manifest.source.sha256 ||
    record.contentHash !== manifest.contentHash
  ) {
    throw new Error(`Index/manifest identity mismatch for ${record.key}`);
  }
  for (const [key, value] of Object.entries(manifest.summary)) {
    if (record[key] !== value) throw new Error(`Index/manifest summary mismatch for ${record.key}.${key}`);
  }
}

function validatePlacement(placement, pages, padding, label) {
  if (!placement || placement.rectKey !== label) throw new Error(`Missing placement for ${label}`);
  const page = pages.get(placement.page);
  if (!page || placement.pageKey !== page.key) throw new Error(`Missing page for ${label}`);
  if (
    placement.x < padding ||
    placement.y < padding ||
    placement.width <= 0 ||
    placement.height <= 0 ||
    placement.x + placement.width + padding > page.width ||
    placement.y + placement.height + padding > page.height
  ) {
    throw new Error(`Placement ${label} lacks a complete extruded gutter`);
  }
}

async function allManifestPagesExist(resolved, manifest) {
  for (const page of manifest.pages) {
    const pagePath = publicFileForUrl(resolved.outputDir, resolved.urlRoot, page.imageUrl);
    try {
      const metadata = await stat(pagePath);
      if (metadata.size !== page.networkBytes) return false;
    } catch {
      return false;
    }
  }
  return true;
}

async function writeCasPage(outputDir, urlRoot, hash, bytes) {
  const relative = `pages/${hash.slice(0, 2)}/${hash}.png`;
  const filePath = path.join(outputDir, ...relative.split("/"));
  await mkdir(path.dirname(filePath), { recursive: true });
  try {
    await writeFile(filePath, bytes, { flag: "wx" });
  } catch (error) {
    if (error?.code !== "EEXIST") throw error;
    const existing = await readFile(filePath);
    if (existing.byteLength !== bytes.byteLength || sha256(existing) !== hash) {
      throw new Error(`Existing CAS page is corrupt: ${filePath}`);
    }
  }
  return `${urlRoot}/${relative}`;
}

function publicFileForUrl(outputDir, urlRoot, url) {
  const prefix = `${urlRoot}/`;
  if (!url.startsWith(prefix)) throw new Error(`Asset URL escapes full-pack root: ${url}`);
  const relative = decodeURIComponent(url.slice(prefix.length));
  const resolved = path.resolve(outputDir, ...relative.split("/"));
  const root = `${path.resolve(outputDir)}${path.sep}`;
  if (!resolved.startsWith(root)) throw new Error(`Asset URL escapes output directory: ${url}`);
  return resolved;
}

function manifestRelativePathFor(entry, config) {
  const libraryId = sha256(Buffer.from(entry.key, "utf8")).slice(0, 24);
  return `libraries/${encodeURIComponent(entry.category)}/${libraryId}-${entry.sourceSha256.slice(0, 20)}-${config.contentHash.slice(0, 12)}.json`;
}

function coverageReport(mode, summary, evidence) {
  return {
    schemaVersion: 1,
    kind: "mir2-crystal-full-pack-coverage",
    mode,
    completeness: {
      sourceLibraryCount: summary.libraryCount,
      frameSlotCount: summary.frameSlotCount,
      classifiedFrameCount: summary.packedFrameCount + summary.noDrawFrameCount,
      packedFrameCount: summary.packedFrameCount,
      noDrawFrameCount: summary.noDrawFrameCount,
      packedMaskCount: summary.packedMaskCount,
      noDrawMaskCount: summary.noDrawMaskCount,
      allFrameSlotsClassified:
        summary.frameSlotCount === summary.packedFrameCount + summary.noDrawFrameCount,
    },
    storage: {
      pageCount: summary.pageCount,
      networkBytes: summary.networkBytes ?? null,
      gpuBytes: summary.gpuBytes,
      rawRgbaBytes: summary.rawRgbaBytes,
      estimatedNetworkBytes: summary.estimatedNetworkBytes ?? null,
      estimatedManifestBytes: summary.estimatedManifestBytes ?? null,
      conservativeBuildBytes: summary.conservativeBuildBytes ?? null,
    },
    evidence,
  };
}

async function diskBudget(outputDir, conservativeBuildBytes) {
  try {
    const stats = await statfs(path.dirname(outputDir));
    const freeBytes = Number(stats.bavail) * Number(stats.bsize);
    return {
      freeBytes,
      requiredBytes: conservativeBuildBytes,
      headroomBytes: freeBytes - conservativeBuildBytes,
    };
  } catch {
    return { freeBytes: null, requiredBytes: conservativeBuildBytes, headroomBytes: null };
  }
}

async function mapWithConcurrency(items, concurrency, worker) {
  const output = new Array(items.length);
  let nextIndex = 0;
  const runners = Array.from({ length: Math.min(concurrency, items.length) }, async () => {
    while (true) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) return;
      output[index] = await worker(items[index], index);
    }
  });
  await Promise.all(runners);
  return output;
}

async function atomicWriteJson(filePath, value) {
  await atomicWriteFile(filePath, Buffer.from(`${canonicalJson(value)}\n`, "utf8"));
}

async function atomicWriteFile(filePath, bytes) {
  await mkdir(path.dirname(filePath), { recursive: true });
  const temporaryPath = `${filePath}.tmp-${process.pid}-${Date.now()}-${Math.random().toString(16).slice(2)}`;
  await writeFile(temporaryPath, bytes);
  try {
    await rename(temporaryPath, filePath);
  } catch (error) {
    if (error?.code !== "EEXIST" && error?.code !== "EPERM") throw error;
    await writeFile(filePath, bytes);
  }
}

function compareSources(left, right) {
  return (
    right.height - left.height ||
    right.width - left.width ||
    left.frameIndex - right.frameIndex ||
    compareCodePoints(left.role, right.role) ||
    compareCodePoints(left.key, right.key)
  );
}

function compareScore(left, right) {
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

function checkedRgbaBytes(width, height, label) {
  const bytes = width * height * 4;
  if (!Number.isSafeInteger(bytes) || bytes <= 0) throw new Error(`Invalid RGBA dimensions for ${label}`);
  return bytes;
}

function validateContentHash(value, label) {
  const body = { ...value };
  delete body.contentHash;
  if (value.contentHash !== semanticHash(body)) throw new Error(`${label} contentHash mismatch`);
}

function semanticHash(value) {
  return sha256(Buffer.from(canonicalJson(value), "utf8"));
}

function canonicalJson(value) {
  return JSON.stringify(canonicalize(value), null, 2);
}

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort(compareCodePoints).map((key) => [key, canonicalize(value[key])]));
  }
  return value;
}

function sha256(value) {
  return createHash("sha256").update(value).digest("hex");
}

function nextPowerOfTwo(value) {
  return 2 ** Math.ceil(Math.log2(Math.max(1, value)));
}

function compareCodePoints(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function writeProgress(mode, current, total, key, pageCount) {
  process.stderr.write(`[crystal-full-pack] ${mode} ${current}/${total} ${key} pages=${pageCount}\n`);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) continue;
    const equals = argument.indexOf("=");
    if (equals >= 0) parsed[argument.slice(2, equals)] = argument.slice(equals + 1);
    else if (argv[index + 1] && !argv[index + 1].startsWith("--")) parsed[argument.slice(2)] = argv[++index];
    else parsed[argument.slice(2)] = "true";
  }
  return parsed;
}

function parseFilter(value) {
  if (value === undefined || value === null || value === "") return null;
  return String(value).split(",").map((entry) => entry.trim()).filter(Boolean);
}

function parseBoolean(value, fallback) {
  if (value === undefined) return fallback;
  if (["1", "true", "yes", "on"].includes(String(value).toLowerCase())) return true;
  if (["0", "false", "no", "off"].includes(String(value).toLowerCase())) return false;
  throw new Error(`Invalid boolean ${value}`);
}

function positiveInteger(value, fallback) {
  const parsed = value === undefined ? fallback : Number(value);
  if (!Number.isInteger(parsed) || parsed <= 0) throw new Error(`Expected a positive integer, got ${value}`);
  return parsed;
}

function optionalPositiveInteger(value) {
  return value === undefined ? null : positiveInteger(value, null);
}

function nonNegativeInteger(value, fallback) {
  const parsed = value === undefined ? fallback : Number(value);
  if (!Number.isInteger(parsed) || parsed < 0) throw new Error(`Expected a non-negative integer, got ${value}`);
  return parsed;
}

function boundedInteger(value, fallback, min, max) {
  const parsed = value === undefined ? fallback : Number(value);
  if (!Number.isInteger(parsed) || parsed < min || parsed > max) {
    throw new Error(`Expected an integer in [${min}, ${max}], got ${value}`);
  }
  return parsed;
}

async function exists(filePath) {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function listRegularFiles(root) {
  if (!await exists(root)) return [];
  const files = [];
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const entryPath = path.join(root, entry.name);
    if (entry.isSymbolicLink()) {
      throw new Error(`Full-pack prune refuses symbolic links: ${entryPath}`);
    }
    if (entry.isDirectory()) files.push(...await listRegularFiles(entryPath));
    else if (entry.isFile()) files.push(path.resolve(entryPath));
  }
  return files.sort(compareCodePoints);
}

async function totalFileBytes(filePaths) {
  let total = 0;
  for (const filePath of filePaths) {
    total += (await stat(filePath)).size;
  }
  return total;
}

function assertInsideDirectory(root, candidate) {
  const relative = path.relative(path.resolve(root), path.resolve(candidate));
  if (!relative || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error(`Full-pack prune path escapes output directory: ${candidate}`);
  }
}
