#!/usr/bin/env node

import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(webRoot, "..", "..");
const publicRoot = path.join(webRoot, "public");
const args = parseArgs(process.argv.slice(2));
const baseManifestPath = path.resolve(requiredArg("baseManifest"));
const originalAssetManifestPath = path.resolve(
  args.originalAssetManifest ?? path.join(publicRoot, "original-asset-manifest.generated.json"),
);
const version = normalizeVersion(requiredArg("version"));
const objectPrefix = normalizePrefix(args.objectPrefix ?? `mir2/v/${version}`);
const fallbackObjectPrefix = normalizePrefix(requiredArg("fallbackObjectPrefix"));
const assetBaseUrl = normalizeBaseUrl(requiredArg("assetBaseUrl"));
const outputPath = path.resolve(
  args.output ?? path.join(repoRoot, "docs", "generated", "remote-assets", version, "remote-asset-release.json"),
);
const uploadPlanPath = path.resolve(
  args.uploadPlan ?? path.join(path.dirname(outputPath), "overlay-upload-plan.json"),
);
const manifestUploadPlanPath = path.resolve(
  args.manifestUploadPlan ?? path.join(path.dirname(outputPath), "overlay-manifest-upload-plan.json"),
);
const overlayRoots = String(requiredArg("overlayRoots"))
  .split(",")
  .map((value) => value.trim().replace(/^\/+|\/+$/g, ""))
  .filter(Boolean);

if (!objectPrefix.startsWith("mir2/v/") || !fallbackObjectPrefix.startsWith("mir2/v/")) {
  throw new Error("Overlay and fallback prefixes must both start with mir2/v/.");
}
if (objectPrefix === fallbackObjectPrefix) {
  throw new Error("Overlay and fallback prefixes must differ.");
}

const base = JSON.parse(await fs.readFile(baseManifestPath, "utf8"));
if (normalizePrefix(base.objectPrefix) !== fallbackObjectPrefix) {
  throw new Error(
    `Base manifest prefix mismatch: expected ${fallbackObjectPrefix}, got ${base.objectPrefix ?? "<empty>"}`,
  );
}
if (Number(base.stats?.missingCount ?? 0) !== 0) {
  throw new Error(`Base manifest missingCount must be zero, got ${base.stats?.missingCount}`);
}
if (base.fullCrystalPack?.enabled !== true || base.fullCrystalPack?.verified !== true) {
  throw new Error("Overlay base manifest must declare a verified full Crystal pack.");
}

const originalAssets = JSON.parse(await fs.readFile(originalAssetManifestPath, "utf8"));
const baseFiles = Array.isArray(base.files) ? base.files : [];
const fileByPath = new Map(baseFiles.map((file) => [relativePath(file), { ...file }]));
const uploadPaths = new Set();

for (const [publicPath, asset] of Object.entries(originalAssets.assets ?? {})) {
  const relative = publicPath.replace(/^\/+/, "");
  const baseFile = fileByPath.get(relative);
  const sha256 = String(asset.sha256 ?? "").toLowerCase();
  const size = Number(asset.size ?? 0);
  if (!/^[a-f0-9]{64}$/.test(sha256) || !Number.isSafeInteger(size) || size <= 0) {
    throw new Error(`Invalid original asset entry: ${publicPath}`);
  }
  if (baseFile && baseFile.h === sha256 && Number(baseFile.s) === size) continue;

  fileByPath.set(relative, {
    p: relative,
    s: size,
    h: sha256,
    c: contentTypeForPath(relative),
    src: mergeSources(baseFile?.src, "overlay:original-asset-manifest"),
  });
  uploadPaths.add(relative);
}

for (const root of overlayRoots) {
  const resolvedRoot = path.resolve(publicRoot, ...root.split("/"));
  const relativeRoot = path.relative(publicRoot, resolvedRoot);
  if (!relativeRoot || relativeRoot.startsWith("..") || path.isAbsolute(relativeRoot)) {
    throw new Error(`Overlay root escapes public/: ${root}`);
  }
  for (const filePath of await listFilesRecursive(resolvedRoot)) {
    const relative = path.relative(publicRoot, filePath).split(path.sep).join("/");
    const bytes = await fs.readFile(filePath);
    const sha256 = hash(bytes);
    const baseFile = fileByPath.get(relative);
    if (baseFile && baseFile.h === sha256 && Number(baseFile.s) === bytes.length) continue;
    fileByPath.set(relative, {
      p: relative,
      s: bytes.length,
      h: sha256,
      c: contentTypeForPath(relative),
      src: mergeSources(baseFile?.src, "overlay:public-root"),
    });
    uploadPaths.add(relative);
  }
}

const files = [...fileByPath.values()].sort((left, right) => relativePath(left).localeCompare(relativePath(right)));
for (const [publicPath, asset] of Object.entries(originalAssets.assets ?? {})) {
  const file = fileByPath.get(publicPath.replace(/^\/+/, ""));
  if (!file || file.h !== asset.sha256 || Number(file.s) !== Number(asset.size)) {
    throw new Error(`Logical overlay closure mismatch: ${publicPath}`);
  }
}

const uploadFiles = files.filter((file) => uploadPaths.has(relativePath(file))).map((file) => ({
  ...file,
  stagePath: path.join(publicRoot, ...relativePath(file).split("/")),
}));
const originalUiRoot = (base.publicAssetRoots ?? []).find((root) => root.root === "original-ui");
const entityAtlasRoot = (base.publicAssetRoots ?? []).find((root) => root.root === "bevy-entity-atlases");
const monsterRoot = (base.sceneSpriteRoots ?? []).find((root) => root.root === "Monster");
const originalUiFileCount = files.filter((file) => relativePath(file).startsWith("original-ui/")).length;
const entityAtlasFileCount = files.filter((file) => relativePath(file).startsWith("bevy-entity-atlases/")).length;
const monsterFileCount = files.filter((file) => relativePath(file).startsWith("original-ui/Monster/")).length;
const logicalTotalBytes = files.reduce((sum, file) => sum + Number(file.s ?? file.size ?? 0), 0);
const storageBytes = files.reduce((sum, file) => sum + Number(file.es ?? file.encodedSize ?? file.s ?? file.size ?? 0), 0);

const release = {
  ...base,
  version,
  generatedAt: new Date().toISOString(),
  assetBaseUrl,
  objectPrefix,
  fallbackObjectPrefix,
  overlayRoots,
  outputDir: path.dirname(outputPath),
  stageDir: null,
  stats: {
    ...base.stats,
    originalAssetManifestAssetCount: Number(originalAssets.stats?.assetCount ?? 0),
    sceneSpriteFileCount: replaceCount(base.stats?.sceneSpriteFileCount, monsterRoot?.fileCount, monsterFileCount),
    publicAssetFileCount: replaceCount(
      replaceCount(base.stats?.publicAssetFileCount, originalUiRoot?.fileCount, originalUiFileCount),
      entityAtlasRoot?.fileCount,
      entityAtlasFileCount,
    ),
    fileCount: files.length,
    missingCount: 0,
    totalBytes: logicalTotalBytes,
    storageBytes,
    encodedFileCount: files.filter((file) => Boolean(file.e ?? file.contentEncoding)).length,
    storageSavingsBytes: logicalTotalBytes - storageBytes,
  },
  originalAssetManifest: {
    ...base.originalAssetManifest,
    schemaVersion: originalAssets.schemaVersion,
    assetHash: originalAssets.assetHash,
    assetCount: Number(originalAssets.stats?.assetCount ?? 0),
    originalMapPngCount: Number(originalAssets.stats?.originalMapPngCount ?? 0),
    originalUiPngCount: Number(originalAssets.stats?.originalUiPngCount ?? 0),
  },
  sceneSpriteRoots: (base.sceneSpriteRoots ?? []).map((root) =>
    root.root === "Monster" ? { ...root, fileCount: monsterFileCount } : root,
  ),
  publicAssetRoots: (base.publicAssetRoots ?? []).map((root) => {
    if (root.root === "original-ui") return { ...root, fileCount: originalUiFileCount };
    if (root.root === "bevy-entity-atlases") return { ...root, fileCount: entityAtlasFileCount };
    return root;
  }),
  files,
  missing: [],
  missingRequiredManifestPaths: [],
};
const uploadPlan = {
  schemaVersion: 1,
  kind: "mir2-r2-overlay-upload-plan",
  publishReleaseManifest: false,
  version,
  generatedAt: release.generatedAt,
  assetBaseUrl,
  objectPrefix,
  cacheControl: release.cacheControl,
  files: uploadFiles,
};

await fs.mkdir(path.dirname(outputPath), { recursive: true });
await fs.writeFile(outputPath, `${JSON.stringify(release)}\n`, "utf8");
await fs.writeFile(uploadPlanPath, `${JSON.stringify(uploadPlan, null, 2)}\n`, "utf8");
const releaseBytes = await fs.readFile(outputPath);
const manifestUploadPlan = {
  schemaVersion: 1,
  kind: "mir2-r2-overlay-manifest-upload-plan",
  publishReleaseManifest: false,
  version,
  generatedAt: release.generatedAt,
  assetBaseUrl,
  objectPrefix,
  files: [{
    p: "remote-asset-release.json",
    s: releaseBytes.length,
    h: hash(releaseBytes),
    c: "application/json; charset=utf-8",
    src: ["overlay:release-manifest"],
    stagePath: outputPath,
    cacheControl: "public, max-age=60, stale-while-revalidate=300",
  }],
};
await fs.writeFile(manifestUploadPlanPath, `${JSON.stringify(manifestUploadPlan, null, 2)}\n`, "utf8");
console.log(JSON.stringify({
  ok: true,
  outputPath,
  uploadPlanPath,
  manifestUploadPlanPath,
  version,
  objectPrefix,
  fallbackObjectPrefix,
  logicalFileCount: files.length,
  uploadFileCount: uploadFiles.length,
  originalAssetCount: release.originalAssetManifest.assetCount,
  fullCrystalPackFileCount: release.fullCrystalPack.fileCount,
  totalBytes: logicalTotalBytes,
  storageBytes,
}, null, 2));

function requiredArg(name) {
  const value = String(args[name] ?? "").trim();
  if (!value) throw new Error(`Missing required --${name}.`);
  return value;
}

function relativePath(file) {
  return String(file?.p ?? file?.relativePath ?? file?.path ?? "").replace(/^\/+/, "");
}

function replaceCount(totalValue, oldValue, newValue) {
  const total = Number(totalValue ?? 0);
  const oldCount = Number(oldValue ?? 0);
  return total - oldCount + newValue;
}

function mergeSources(sources, source) {
  return [...new Set([...(Array.isArray(sources) ? sources : []), source])];
}

function hash(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

async function listFilesRecursive(root) {
  const stats = await fs.stat(root);
  if (stats.isFile()) return [root];
  const files = [];
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of await fs.readdir(current, { withFileTypes: true })) {
      const entryPath = path.join(current, entry.name);
      if (entry.isDirectory()) stack.push(entryPath);
      else if (entry.isFile()) files.push(entryPath);
    }
  }
  return files.sort();
}

function contentTypeForPath(filePath) {
  const extension = path.extname(filePath).toLowerCase();
  if (extension === ".png") return "image/png";
  if (extension === ".json") return "application/json; charset=utf-8";
  if (extension === ".cur") return "image/x-icon";
  throw new Error(`Unsupported overlay asset extension: ${filePath}`);
}

function normalizeVersion(value) {
  const normalized = String(value).trim().replace(/[^a-zA-Z0-9._-]/g, "-").replace(/-+/g, "-").replace(/^-+|-+$/g, "").slice(0, 80);
  if (!normalized) throw new Error("Invalid overlay version.");
  return normalized;
}

function normalizePrefix(value) {
  return String(value ?? "").trim().replace(/^\/+|\/+$/g, "");
}

function normalizeBaseUrl(value) {
  const url = new URL(String(value));
  if (url.protocol !== "https:") throw new Error("Overlay assetBaseUrl must use HTTPS.");
  return url.href.replace(/\/+$/, "");
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (!value.startsWith("--")) continue;
    const key = value.slice(2);
    const next = values[index + 1];
    parsed[key] = next && !next.startsWith("--") ? values[++index] : true;
  }
  return parsed;
}
