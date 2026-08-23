#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const ASSET_MANIFEST_SCHEMA = "mir2-crystal-original-asset-manifest-v1";
export const ASSET_MANIFEST_PRODUCER = "crystal-original-asset-manifest-builder";
const SHA256_PATTERN = /^[0-9a-f]{64}$/;

const MANIFEST_FIELDS = [
  "schemaVersion",
  "producer",
  "generatedAt",
  "rootName",
  "fileCount",
  "totalBytes",
  "rootSha256",
  "files",
];
const FILE_FIELDS = ["path", "bytes", "sha256"];

export function parseAssetManifestArgs(argv) {
  const result = { includes: [] };
  const allowed = new Set(["asset-root", "output", "include"]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "-h" || token === "--help") {
      result.help = true;
      continue;
    }
    if (!token.startsWith("--")) throw new Error(`Unexpected positional argument: ${token}`);
    const equals = token.indexOf("=");
    const key = token.slice(2, equals > 2 ? equals : undefined);
    if (!allowed.has(key)) throw new Error(`Unknown argument: --${key}`);
    const value = equals > 2 ? token.slice(equals + 1) : argv[++index];
    if (!value || value.startsWith("--")) throw new Error(`--${key} requires a value.`);
    if (key === "include") result.includes.push(value);
    else result[key] = value;
  }
  if (result.help) return result;
  for (const key of ["asset-root", "output"]) {
    if (!result[key]) throw new Error(`--${key} is required.`);
  }
  return result;
}

export async function buildOriginalAssetManifest({ assetRoot, output, includes = [], generatedAt = new Date().toISOString() }) {
  if (typeof assetRoot !== "string" || !assetRoot) throw new Error("assetRoot is required");
  if (typeof output !== "string" || !output) throw new Error("output is required");
  assertIsoTimestamp(generatedAt, "generatedAt");

  const rootPath = path.resolve(assetRoot);
  const outputPath = path.resolve(output);
  await assertNoSymlinkAncestors(rootPath, "asset root path");
  await assertNoSymlinkAncestors(outputPath, "output path");
  const rootStats = await lstatNoFollow(rootPath, "asset root");
  assertNotReparse(rootStats, rootPath, "asset root");
  if (!rootStats.isDirectory()) throw new Error("asset root must be a directory");

  if (isWithinPath(rootPath, outputPath)) {
    throw new Error("output must be outside the asset root");
  }
  await assertAbsent(outputPath, "output");

  const rootName = path.basename(rootPath);
  assertSafeRootName(rootName);
  const normalizedIncludes = normalizeIncludes(includes);
  const files = await enumerateFiles(rootPath, normalizedIncludes);
  const totalBytes = files.reduce((sum, file) => sum + file.bytes, 0);
  if (!Number.isSafeInteger(totalBytes)) throw new Error("totalBytes exceeds JavaScript safe integer range");
  const manifest = {
    schemaVersion: ASSET_MANIFEST_SCHEMA,
    producer: ASSET_MANIFEST_PRODUCER,
    generatedAt,
    rootName,
    fileCount: files.length,
    totalBytes,
    rootSha256: computeRootSha256(files),
    files,
  };
  validateOriginalAssetManifest(manifest);
  await writeJsonCreateOnly(outputPath, manifest);
  return { outputPath, manifest };
}

export function canonicalManifestEntries(files) {
  return files.map(({ path: relativePath, bytes, sha256 }) => `${relativePath}\t${bytes}\t${sha256}\n`).join("");
}

export function computeRootSha256(files) {
  return sha256(Buffer.from(canonicalManifestEntries(files), "utf8"));
}

export function validateOriginalAssetManifest(value) {
  assertClosedObject(value, "asset manifest", MANIFEST_FIELDS);
  assertEqual(value.schemaVersion, ASSET_MANIFEST_SCHEMA, "asset manifest schemaVersion");
  assertEqual(value.producer, ASSET_MANIFEST_PRODUCER, "asset manifest producer");
  assertIsoTimestamp(value.generatedAt, "asset manifest generatedAt");
  assertSafeRootName(value.rootName);
  assertSafeInteger(value.fileCount, "asset manifest fileCount", 0);
  assertSafeInteger(value.totalBytes, "asset manifest totalBytes", 0);
  assertSha256(value.rootSha256, "asset manifest rootSha256");
  if (!Array.isArray(value.files)) throw new Error("asset manifest files must be an array");
  if (value.fileCount !== value.files.length) throw new Error("asset manifest fileCount does not match files");

  let previousPath = null;
  let totalBytes = 0;
  for (const file of value.files) {
    assertClosedObject(file, "asset manifest file", FILE_FIELDS);
    assertSafeRelativePath(file.path, "asset manifest file path");
    assertSafeInteger(file.bytes, `asset manifest bytes for ${file.path}`, 0);
    assertSha256(file.sha256, `asset manifest SHA-256 for ${file.path}`);
    if (previousPath !== null && compareCodePoints(previousPath, file.path) >= 0) {
      throw new Error("asset manifest files must be strictly sorted and unique");
    }
    previousPath = file.path;
    totalBytes += file.bytes;
    if (!Number.isSafeInteger(totalBytes)) throw new Error("asset manifest totalBytes exceeds safe integer range");
  }
  if (totalBytes !== value.totalBytes) throw new Error("asset manifest totalBytes does not match files");
  const expectedRootSha256 = computeRootSha256(value.files);
  if (value.rootSha256 !== expectedRootSha256) throw new Error("asset manifest rootSha256 does not match canonical entries");
  return value;
}

export async function writeJsonCreateOnly(target, value) {
  const targetPath = path.resolve(target);
  await assertNoSymlinkAncestors(targetPath, "output path");
  await assertAbsent(targetPath, "output");
  await fs.mkdir(path.dirname(targetPath), { recursive: true });
  await assertNoSymlinkAncestors(targetPath, "output path");
  const temporaryPath = `${targetPath}.${process.pid}.${crypto.randomUUID()}.tmp`;
  const contents = `${JSON.stringify(value, null, 2)}\n`;
  let handle;
  try {
    handle = await fs.open(temporaryPath, "wx");
    await handle.writeFile(contents, "utf8");
    await handle.sync();
    await handle.close();
    handle = undefined;
    // Linking the completed temporary file is atomic and refuses an existing target.
    await fs.link(temporaryPath, targetPath);
  } catch (error) {
    if (handle) await handle.close().catch(() => {});
    await fs.rm(temporaryPath, { force: true }).catch(() => {});
    throw new Error(`refusing to overwrite or atomically create ${targetPath}: ${error.message}`);
  }
  await fs.rm(temporaryPath, { force: true });
}

export async function assertNoSymlinkAncestors(target, label) {
  let current = path.resolve(target);
  while (true) {
    try {
      const stats = await fs.lstat(current);
      if (stats.isSymbolicLink()) {
        throw new Error(`${label} contains a symlink/reparse ancestor: ${current}`);
      }
    } catch (error) {
      if (error?.code !== "ENOENT") {
        if (error?.message?.includes("symlink/reparse ancestor")) throw error;
        throw new Error(`${label} ancestor cannot be checked: ${error.message}`);
      }
    }
    const parent = path.dirname(current);
    if (parent === current) return;
    current = parent;
  }
}

function normalizeIncludes(includes) {
  if (!Array.isArray(includes)) throw new Error("includes must be an array");
  const normalized = includes.map((value) => {
    assertSafeRelativePath(value, "include");
    return value;
  });
  const comparison = (value) => process.platform === "win32" ? value.toLowerCase() : value;
  const sorted = [...normalized].sort((left, right) => compareCodePoints(comparison(left), comparison(right)));
  for (let index = 0; index < sorted.length; index += 1) {
    const currentSegments = comparison(sorted[index]).split("/");
    for (let priorIndex = 0; priorIndex < index; priorIndex += 1) {
      const priorSegments = comparison(sorted[priorIndex]).split("/");
      if (priorSegments.length <= currentSegments.length && priorSegments.every((segment, segmentIndex) => segment === currentSegments[segmentIndex])) {
        throw new Error("include entries must be unique and must not overlap by ancestor/descendant path");
      }
    }
  }
  return normalized;
}

async function enumerateFiles(rootPath, includes) {
  const records = [];
  const seen = new Set();

  async function visit(directoryPath) {
    const entries = await fs.readdir(directoryPath, { withFileTypes: true });
    entries.sort((left, right) => compareCodePoints(left.name, right.name));
    for (const entry of entries) {
      const entryPath = path.join(directoryPath, entry.name);
      const stats = await lstatNoFollow(entryPath, "asset entry");
      assertNotReparse(stats, entryPath, "asset entry");
      if (stats.isDirectory()) {
        await visit(entryPath);
        continue;
      }
      if (!stats.isFile()) throw new Error(`asset root contains a non-ordinary file: ${entryPath}`);
      const relativePath = path.relative(rootPath, entryPath).split(path.sep).join("/");
      assertSafeRelativePath(relativePath, "asset file path");
      if (seen.has(relativePath)) throw new Error(`duplicate asset path: ${relativePath}`);
      seen.add(relativePath);
      const contents = await fs.readFile(entryPath);
      const afterRead = await lstatNoFollow(entryPath, "asset entry");
      assertNotReparse(afterRead, entryPath, "asset entry");
      if (!afterRead.isFile()) throw new Error(`asset entry changed while reading: ${entryPath}`);
      records.push({ path: relativePath, bytes: contents.length, sha256: sha256(contents) });
    }
  }

  if (includes.length === 0) {
    await visit(rootPath);
  } else {
    for (const include of includes) {
      const includePath = path.join(rootPath, ...include.split("/"));
      const stats = await lstatNoFollow(includePath, "include directory");
      assertNotReparse(stats, includePath, "include directory");
      if (!stats.isDirectory()) throw new Error(`include must name a directory: ${include}`);
      await visit(includePath);
    }
  }
  records.sort((left, right) => compareCodePoints(left.path, right.path));
  return records;
}

async function lstatNoFollow(target, label) {
  try {
    return await fs.lstat(target);
  } catch (error) {
    throw new Error(`${label} cannot be read: ${error.message}`);
  }
}

function assertNotReparse(stats, target, label) {
  // Node exposes Windows symbolic links and junctions as symbolic links through lstat.
  // Checking lstat before every traversal/read prevents following a reparse point.
  if (stats.isSymbolicLink()) throw new Error(`${label} is a symlink/reparse point: ${target}`);
}

async function assertAbsent(target, label) {
  try {
    await fs.lstat(target);
  } catch (error) {
    if (error?.code === "ENOENT") return;
    throw new Error(`${label} cannot be checked: ${error.message}`);
  }
  throw new Error(`refusing to overwrite existing ${label}: ${target}`);
}

function assertSafeRootName(value) {
  if (typeof value !== "string" || !value || value === "." || value === ".." || value.includes("\\") || value.includes("..") || value.includes("/") || path.isAbsolute(value) || /^[A-Za-z]:/.test(value) || /[\u0000-\u001f\u007f]/.test(value)) {
    throw new Error("rootName must be a safe basename");
  }
}

function assertSafeRelativePath(value, label) {
  if (typeof value !== "string" || !value || value.includes("\\") || value.startsWith("/") || path.isAbsolute(value) || /^[A-Za-z]:[\\/]/.test(value) || /[\u0000-\u001f\u007f]/.test(value)) {
    throw new Error(`${label} must be a safe relative path using /`);
  }
  const segments = value.split("/");
  if (segments.some((segment) => !segment || segment === "." || segment === "..")) {
    throw new Error(`${label} contains an unsafe path segment`);
  }
}

function assertClosedObject(value, label, fields) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} must be an object`);
  const allowed = new Set(fields);
  const unknown = Object.keys(value).filter((field) => !allowed.has(field));
  if (unknown.length > 0) throw new Error(`${label} contains unknown field(s): ${unknown.join(", ")}`);
  const missing = fields.filter((field) => !Object.hasOwn(value, field));
  if (missing.length > 0) throw new Error(`${label} is missing field(s): ${missing.join(", ")}`);
}

function assertIsoTimestamp(value, label) {
  if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) throw new Error(`${label} must be an ISO timestamp`);
}

function assertSafeInteger(value, label, minimum) {
  if (!Number.isSafeInteger(value) || value < minimum) throw new Error(`${label} must be a safe integer >= ${minimum}`);
}

function assertSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value)) throw new Error(`${label} must be lowercase SHA-256`);
}

function assertEqual(left, right, label) {
  if (left !== right) throw new Error(`${label} does not match`);
}

function sha256(value) {
  return crypto.createHash("sha256").update(value).digest("hex");
}

function compareCodePoints(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function isWithinPath(rootPath, candidatePath) {
  const root = comparisonPath(rootPath);
  const candidate = comparisonPath(candidatePath);
  const relative = path.relative(root, candidate);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative));
}

function comparisonPath(value) {
  const resolved = path.resolve(value);
  return process.platform === "win32" ? resolved.toLowerCase() : resolved;
}

function printHelp() {
  console.log("Usage: node build-crystal-original-asset-manifest.mjs --asset-root <dir> --output <json> [--include <safe-relative-subdir> ...]");
}

const currentFile = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === currentFile) {
  try {
    const options = parseAssetManifestArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
    } else {
      const result = await buildOriginalAssetManifest({ assetRoot: options["asset-root"], output: options.output, includes: options.includes });
      console.log(JSON.stringify({ ok: true, fileCount: result.manifest.fileCount, totalBytes: result.manifest.totalBytes, rootSha256: result.manifest.rootSha256 }));
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
