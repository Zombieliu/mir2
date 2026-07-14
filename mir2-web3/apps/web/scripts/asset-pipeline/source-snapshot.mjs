import { createHash } from "node:crypto";
import { createReadStream, existsSync } from "node:fs";
import { mkdir, open, readdir, stat, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  CRYSTAL_FRAME_SET_RECORD_BYTES,
  parseFrameSetRecords,
} from "../crystal-library.mjs";

export const CRYSTAL_SOURCE_SNAPSHOT_SCHEMA_VERSION = 1;

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const WEB_ROOT = path.resolve(import.meta.dirname, "..", "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const LOCAL_CRYSTAL_DATA_DIR = path.join(MIR2_ROOT, "Crystal", "Build", "Client", "Debug", "Data");
const DOWNLOADED_CRYSTAL_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const DEFAULT_OUTPUT_PATH = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "assets",
  "crystal-source-snapshot.generated.json",
);
const DEFAULT_RUNTIME_FRAME_SET_PATH = path.join(
  WEB_ROOT,
  "public",
  "original-ui",
  "frame-sets.generated.json",
);

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  runSourceSnapshotCli().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

export async function runSourceSnapshotCli(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  const dataDir = resolveDataDir(
    args.dataDir ??
      args._[0] ??
      process.env.CRYSTAL_CLIENT_DATA_DIR ??
      process.env.CRYSTAL_CLIENT_ROOT ??
      defaultCrystalSource(),
  );
  const outputPath = path.resolve(args.output ?? process.env.MIR2_CRYSTAL_SOURCE_SNAPSHOT ?? DEFAULT_OUTPUT_PATH);
  const runtimeFrameSetPath = path.resolve(
    args.runtimeOutput ?? process.env.MIR2_CRYSTAL_FRAME_SET_CATALOG ?? DEFAULT_RUNTIME_FRAME_SET_PATH,
  );
  const includeHashes = parseBoolean(args.hash ?? process.env.MIR2_CRYSTAL_SOURCE_HASH, true);
  const allowPartial = parseBoolean(
    args.allowPartial ?? process.env.MIR2_ALLOW_PARTIAL_CRYSTAL_ASSETS,
    false,
  );
  const minimumLibraryCount = parseInteger(
    args.minimumLibraryCount ?? process.env.MIR2_MIN_CRYSTAL_LIBRARY_COUNT,
    1000,
  );

  console.log(`[crystal-source] scanning ${dataDir}`);
  const snapshot = await buildCrystalSourceSnapshot({
    dataDir,
    includeHashes,
    onProgress: ({ index, total, relativePath }) => {
      if (index === 1 || index === total || index % 50 === 0) {
        console.log(`[crystal-source] ${index}/${total} ${relativePath}`);
      }
    },
  });
  await writeCrystalSourceSnapshot(outputPath, snapshot);
  const frameSetCatalog = buildCrystalFrameSetCatalog(snapshot);
  await writeCrystalSourceSnapshot(runtimeFrameSetPath, frameSetCatalog);

  const failures = validateCrystalSourceSnapshot(snapshot, { minimumLibraryCount });
  console.log(JSON.stringify({
    outputPath,
    runtimeFrameSetPath,
    contentHash: snapshot.contentHash,
    frameSetContentHash: frameSetCatalog.contentHash,
    ...snapshot.summary,
  }, null, 2));
  if (failures.length > 0) {
    const message = `Crystal source snapshot validation failed:\n- ${failures.join("\n- ")}`;
    if (!allowPartial) {
      throw new Error(message);
    }
    console.warn(message);
  }

  return snapshot;
}

export async function buildCrystalSourceSnapshot({ dataDir, includeHashes = true, onProgress } = {}) {
  if (!dataDir) {
    throw new Error("Crystal source snapshot requires a dataDir");
  }

  const resolvedDataDir = path.resolve(dataDir);
  const files = await collectLibraryFiles(resolvedDataDir);
  if (files.length === 0) {
    throw new Error(`No Crystal .Lib files found under ${resolvedDataDir}`);
  }

  const libraries = [];
  for (let index = 0; index < files.length; index += 1) {
    const filePath = files[index];
    const relativePath = normalizeRelativePath(path.relative(resolvedDataDir, filePath));
    onProgress?.({ index: index + 1, total: files.length, relativePath });

    try {
      const inspected = await inspectCrystalLibraryFile(filePath, { includeHash: includeHashes });
      libraries.push({ path: relativePath, status: "ok", ...inspected });
    } catch (error) {
      const fileStat = await stat(filePath);
      libraries.push({
        path: relativePath,
        status: "error",
        byteLength: fileStat.size,
        sha256: includeHashes ? await hashFile(filePath) : null,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  const body = {
    schemaVersion: CRYSTAL_SOURCE_SNAPSHOT_SCHEMA_VERSION,
    sourceKind: "crystal-client-data",
    sourceLayout: "Crystal/Build/Client/Debug/Data",
    hashAlgorithm: includeHashes ? "sha256" : null,
    summary: summarizeLibraries(libraries),
    libraries,
  };
  const contentHash = sha256(Buffer.from(canonicalJson(body), "utf8"));
  return { ...body, contentHash };
}

export async function inspectCrystalLibraryFile(filePath, { includeHash = true } = {}) {
  const handle = await open(filePath, "r");
  let result;
  try {
    const fileStat = await handle.stat();
    const header = await readExact(handle, 0, 8, "library header");
    const version = header.readInt32LE(0);
    const frameSlotCount = header.readInt32LE(4);
    if (version < 2) {
      throw new Error(`Unsupported lib version ${version}`);
    }
    if (frameSlotCount < 0) {
      throw new Error(`Invalid Crystal library frame count ${frameSlotCount}`);
    }

    const fixedHeaderBytes = version >= 3 ? 12 : 8;
    const headerBytes = fixedHeaderBytes + frameSlotCount * 4;
    if (!Number.isSafeInteger(headerBytes) || headerBytes > fileStat.size) {
      throw new Error(
        `Truncated library frame-offset table: need ${headerBytes} bytes, file length is ${fileStat.size}`,
      );
    }

    let frameSeek = null;
    if (version >= 3) {
      const frameSeekBuffer = await readExact(handle, 8, 4, "FrameSet seek");
      frameSeek = frameSeekBuffer.readInt32LE(0);
    }

    const offsetSummary = await inspectFrameOffsets(
      handle,
      fixedHeaderBytes,
      frameSlotCount,
      headerBytes,
      fileStat.size,
    );
    const frameSet = version >= 3
      ? await readFrameSet(handle, frameSeek, headerBytes, fileStat.size)
      : { seek: null, count: 0, actions: [] };
    const actionIds = new Set();
    const duplicateActionIds = [];
    let unknownActionCount = 0;
    for (const action of frameSet.actions) {
      if (action.actionName === null) unknownActionCount += 1;
      if (actionIds.has(action.actionId)) duplicateActionIds.push(action.actionId);
      actionIds.add(action.actionId);
    }

    const issues = [];
    if (offsetSummary.invalidFrameOffsetCount > 0) {
      issues.push(`${offsetSummary.invalidFrameOffsetCount} frame offset(s) are outside image storage`);
    }
    if (unknownActionCount > 0) {
      issues.push(`${unknownActionCount} FrameSet action id(s) are unknown`);
    }
    if (duplicateActionIds.length > 0) {
      issues.push(`duplicate FrameSet action id(s): ${duplicateActionIds.join(",")}`);
    }

    result = {
      byteLength: fileStat.size,
      sha256: null,
      version,
      frameSlotCount,
      presentFrameCount: offsetSummary.presentFrameCount,
      emptyFrameCount: offsetSummary.emptyFrameCount,
      invalidFrameOffsetCount: offsetSummary.invalidFrameOffsetCount,
      frameSeek,
      frameSet,
      issues,
    };
  } finally {
    await handle.close();
  }

  if (includeHash) {
    result.sha256 = await hashFile(filePath);
  }
  return result;
}

export function buildCrystalFrameSetCatalog(snapshot) {
  if (snapshot?.schemaVersion !== CRYSTAL_SOURCE_SNAPSHOT_SCHEMA_VERSION || !Array.isArray(snapshot.libraries)) {
    throw new Error("Crystal FrameSet catalog requires a valid source snapshot");
  }

  const libraries = {};
  let actionCount = 0;
  const sortedLibraries = [...snapshot.libraries].sort((left, right) => compareCodePoints(left.path, right.path));
  for (const library of sortedLibraries) {
    if (library.status !== "ok" || !library.frameSet || library.frameSet.count <= 0) continue;
    const libraryKey = library.path.replace(/\.lib$/i, "");
    const actions = library.frameSet.actions.map((action) => ({ ...action }));
    libraries[libraryKey] = {
      version: library.version,
      sourceSha256: library.sha256,
      actionCount: actions.length,
      actions,
    };
    actionCount += actions.length;
  }

  const body = {
    schemaVersion: 1,
    sourceContentHash: snapshot.contentHash,
    libraryCount: Object.keys(libraries).length,
    actionCount,
    libraries,
  };
  return { ...body, contentHash: sha256(Buffer.from(canonicalJson(body), "utf8")) };
}

export async function writeCrystalSourceSnapshot(outputPath, snapshot) {
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${canonicalJson(snapshot)}\n`, "utf8");
}

export function validateCrystalSourceSnapshot(snapshot, { minimumLibraryCount = 1000 } = {}) {
  const failures = [];
  if (snapshot.summary.libraryCount < minimumLibraryCount) {
    failures.push(`library count ${snapshot.summary.libraryCount} < ${minimumLibraryCount}`);
  }
  if (snapshot.summary.failedLibraryCount > 0) {
    failures.push(`${snapshot.summary.failedLibraryCount} library parse failure(s)`);
  }
  if (snapshot.summary.invalidFrameOffsetCount > 0) {
    failures.push(`${snapshot.summary.invalidFrameOffsetCount} invalid frame offset(s)`);
  }
  if (snapshot.summary.unknownActionCount > 0) {
    failures.push(`${snapshot.summary.unknownActionCount} unknown FrameSet action id(s)`);
  }
  if (snapshot.summary.duplicateActionCount > 0) {
    failures.push(`${snapshot.summary.duplicateActionCount} duplicate FrameSet action id(s)`);
  }
  return failures;
}

async function inspectFrameOffsets(handle, tableOffset, count, headerBytes, fileSize) {
  const offsetsPerChunk = 16_384;
  let presentFrameCount = 0;
  let emptyFrameCount = 0;
  let invalidFrameOffsetCount = 0;

  for (let start = 0; start < count; start += offsetsPerChunk) {
    const chunkCount = Math.min(offsetsPerChunk, count - start);
    const chunk = await readExact(
      handle,
      tableOffset + start * 4,
      chunkCount * 4,
      "frame-offset table chunk",
    );
    for (let index = 0; index < chunkCount; index += 1) {
      const frameOffset = chunk.readInt32LE(index * 4);
      if (frameOffset === 0) {
        emptyFrameCount += 1;
      } else if (frameOffset < headerBytes || frameOffset >= fileSize) {
        invalidFrameOffsetCount += 1;
      } else {
        presentFrameCount += 1;
      }
    }
  }

  return { presentFrameCount, emptyFrameCount, invalidFrameOffsetCount };
}

async function readFrameSet(handle, frameSeek, minimumSeek, fileSize) {
  if (frameSeek === 0) {
    return { seek: 0, count: 0, actions: [] };
  }
  if (
    !Number.isInteger(frameSeek) ||
    frameSeek < minimumSeek ||
    frameSeek > fileSize - 4
  ) {
    throw new Error(`Invalid Crystal FrameSet seek ${frameSeek} for file length ${fileSize}`);
  }

  const countBuffer = await readExact(handle, frameSeek, 4, "FrameSet count");
  const count = countBuffer.readInt32LE(0);
  if (count < 0) {
    throw new Error(`Invalid Crystal FrameSet action count ${count}`);
  }
  const byteLength = count * CRYSTAL_FRAME_SET_RECORD_BYTES;
  if (!Number.isSafeInteger(byteLength) || byteLength > fileSize - frameSeek - 4) {
    throw new Error(
      `Truncated FrameSet records: need ${byteLength} bytes after ${frameSeek + 4}, file length is ${fileSize}`,
    );
  }

  const records = await readExact(handle, frameSeek + 4, byteLength, "FrameSet action records");
  return { seek: frameSeek, count, actions: parseFrameSetRecords(records, 0, count) };
}

async function readExact(handle, position, length, label) {
  const buffer = Buffer.allocUnsafe(length);
  let total = 0;
  while (total < length) {
    const { bytesRead } = await handle.read(buffer, total, length - total, position + total);
    if (bytesRead === 0) {
      throw new Error(`Truncated ${label}: expected ${length} bytes at ${position}, read ${total}`);
    }
    total += bytesRead;
  }
  return buffer;
}

async function collectLibraryFiles(root) {
  if (!existsSync(root)) return [];
  const files = [];
  const visit = async (directory) => {
    const entries = await readdir(directory, { withFileTypes: true });
    entries.sort((left, right) => compareCodePoints(left.name, right.name));
    for (const entry of entries) {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        await visit(entryPath);
      } else if (entry.isFile() && /\.lib$/i.test(entry.name)) {
        files.push(entryPath);
      }
    }
  };
  await visit(root);
  return files;
}

function summarizeLibraries(libraries) {
  const parsed = libraries.filter((library) => library.status === "ok");
  const versionCounts = {};
  for (const version of [...new Set(parsed.map((library) => library.version))].sort((a, b) => a - b)) {
    versionCounts[String(version)] = parsed.filter((library) => library.version === version).length;
  }

  return {
    libraryCount: libraries.length,
    parsedLibraryCount: parsed.length,
    failedLibraryCount: libraries.length - parsed.length,
    sourceBytes: libraries.reduce((sum, library) => sum + library.byteLength, 0),
    frameSlotCount: parsed.reduce((sum, library) => sum + library.frameSlotCount, 0),
    presentFrameCount: parsed.reduce((sum, library) => sum + library.presentFrameCount, 0),
    emptyFrameCount: parsed.reduce((sum, library) => sum + library.emptyFrameCount, 0),
    invalidFrameOffsetCount: parsed.reduce((sum, library) => sum + library.invalidFrameOffsetCount, 0),
    versionCounts,
    frameSetLibraryCount: parsed.filter((library) => library.frameSet.count > 0).length,
    actionCount: parsed.reduce((sum, library) => sum + library.frameSet.count, 0),
    unknownActionCount: parsed.reduce(
      (sum, library) => sum + library.frameSet.actions.filter((action) => action.actionName === null).length,
      0,
    ),
    duplicateActionCount: parsed.reduce((sum, library) => {
      const ids = new Set();
      let duplicates = 0;
      for (const action of library.frameSet.actions) {
        if (ids.has(action.actionId)) duplicates += 1;
        ids.add(action.actionId);
      }
      return sum + duplicates;
    }, 0),
    issueCount: parsed.reduce((sum, library) => sum + library.issues.length, 0),
  };
}

async function hashFile(filePath) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(filePath)) {
    hash.update(chunk);
  }
  return hash.digest("hex");
}

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function canonicalJson(value) {
  return JSON.stringify(value, null, 2);
}

function resolveDataDir(inputPath) {
  const resolved = path.resolve(inputPath);
  return path.basename(resolved).toLowerCase() === "data" ? resolved : path.join(resolved, "Data");
}

function defaultCrystalSource() {
  if (existsSync(LOCAL_CRYSTAL_DATA_DIR)) return LOCAL_CRYSTAL_DATA_DIR;
  return DOWNLOADED_CRYSTAL_ROOT;
}

function normalizeRelativePath(value) {
  return value.split(path.sep).join("/");
}

function compareCodePoints(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function parseArgs(argv) {
  const parsed = { _: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      parsed._.push(argument);
      continue;
    }
    const equals = argument.indexOf("=");
    if (equals >= 0) {
      parsed[argument.slice(2, equals)] = argument.slice(equals + 1);
    } else if (argv[index + 1] && !argv[index + 1].startsWith("--")) {
      parsed[argument.slice(2)] = argv[index + 1];
      index += 1;
    } else {
      parsed[argument.slice(2)] = "true";
    }
  }
  return parsed;
}

function parseBoolean(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function parseInteger(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  const parsed = Number.parseInt(String(value), 10);
  if (!Number.isInteger(parsed) || parsed < 0) {
    throw new Error(`Expected a non-negative integer, received ${value}`);
  }
  return parsed;
}
