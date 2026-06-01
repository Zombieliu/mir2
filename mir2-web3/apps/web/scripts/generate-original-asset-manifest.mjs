import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const DEFAULT_OUTPUT_PATH = path.join(PUBLIC_ROOT, "original-asset-manifest.generated.json");
const SCHEMA_VERSION = 1;
const TRACKED_ASSET_EXTENSIONS = new Set([".png", ".cur"]);
const MANIFEST_ROOTS = [
  { publicRoot: "original-map", source: "original-map" },
  { publicRoot: "original-ui", source: "original-ui" },
];

const args = parseArgs(process.argv.slice(2));
const outputPath = path.resolve(args.output ?? process.env.MIR2_ORIGINAL_ASSET_MANIFEST_PATH ?? DEFAULT_OUTPUT_PATH);
const collectionMode = String(args.mode ?? process.env.MIR2_ORIGINAL_ASSET_MANIFEST_MODE ?? "filesystem");
const remoteReleaseManifestSource = normalizeOptionalString(
  args.remoteRelease ??
    args.remoteReleaseManifest ??
    process.env.MIR2_ORIGINAL_ASSET_REMOTE_RELEASE ??
    process.env.MIR2_ORIGINAL_ASSET_REMOTE_RELEASE_MANIFEST ??
    process.env.MIR2_REMOTE_ASSET_RELEASE_MANIFEST_URL ??
    "",
);
const manifestConcurrency = positiveIntegerArg(
  args.concurrency ?? process.env.MIR2_ORIGINAL_ASSET_MANIFEST_CONCURRENCY,
  64,
);

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  if (!["filesystem", "git", "remote-release"].includes(collectionMode)) {
    throw new Error(`Unsupported --mode ${collectionMode}; expected "filesystem", "git", or "remote-release".`);
  }

  const { records, remoteRelease } =
    collectionMode === "remote-release"
      ? await collectRemoteReleaseAssetRecords()
      : { records: await collectLocalAssetRecords(), remoteRelease: null };

  const entries = [];
  const assetHash = createHash("sha256");
  let totalBytes = 0;
  const stats = {
    originalMapPngCount: 0,
    originalUiPngCount: 0,
  };

  for (const record of records.filter(Boolean).sort((left, right) => left.publicPath.localeCompare(right.publicPath))) {
    totalBytes += record.size;
    assetHash.update(record.publicPath);
    assetHash.update(String(record.size));
    assetHash.update(record.sha256 || record.contentType || "");

    if (record.source === "original-map") stats.originalMapPngCount += 1;
    if (record.source === "original-ui") stats.originalUiPngCount += 1;
    const entry = {
      size: record.size,
      source: record.source,
    };
    if (record.sha256) entry.sha256 = record.sha256;
    if (record.contentType) entry.contentType = record.contentType;
    entries.push([record.publicPath, entry]);
  }

  const assets = Object.fromEntries(entries.sort(([left], [right]) => left.localeCompare(right)));
  const manifest = {
    schemaVersion: SCHEMA_VERSION,
    kind: "mir2-original-asset-manifest",
    generatedAt: new Date().toISOString(),
    collectionMode,
    assetHash: assetHash.digest("hex"),
    stats: {
      assetCount: entries.length,
      ...stats,
      totalBytes,
    },
    ...(remoteRelease ? { remoteRelease } : {}),
    assets,
  };

  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  console.log(
    JSON.stringify(
      {
        ok: true,
        outputPath,
        collectionMode,
        assetCount: manifest.stats.assetCount,
        originalMapPngCount: manifest.stats.originalMapPngCount,
        originalUiPngCount: manifest.stats.originalUiPngCount,
        totalBytes,
        assetHash: manifest.assetHash,
        remoteRelease,
      },
      null,
      2,
    ),
  );
}

async function collectLocalAssetRecords() {
  const files = collectionMode === "git" ? await collectGitTrackedPngs() : await collectFilesystemPngs();
  let completed = 0;
  const hashedFiles = await mapWithConcurrency(files, manifestConcurrency, async (filePath) => {
    const relativePath = path.relative(PUBLIC_ROOT, filePath).split(path.sep).join("/");
    const publicPath = `/${relativePath}`;
    const root = MANIFEST_ROOTS.find((candidate) => relativePath.startsWith(`${candidate.publicRoot}/`));
    if (!root) return null;

    const file = await fs.readFile(filePath);
    const sha256 = createHash("sha256").update(file).digest("hex");
    completed += 1;
    if (completed % 25000 === 0 || completed === files.length) {
      console.error(`[original-asset-manifest] hashed ${completed}/${files.length}`);
    }
    return {
      publicPath,
      source: root.source,
      size: file.length,
      sha256,
    };
  });

  return hashedFiles.filter(Boolean);
}

async function collectRemoteReleaseAssetRecords() {
  const source = resolveRemoteReleaseManifestSource();
  const release = await readRemoteReleaseManifest(source);
  const files = Array.isArray(release.files) ? release.files : [];
  if (files.length === 0) {
    throw new Error(`Remote asset release manifest has no files: ${source}`);
  }

  const records = [];
  for (const file of files) {
    const relativePath = normalizeRemoteReleaseRelativePath(file, release);
    if (!relativePath || !TRACKED_ASSET_EXTENSIONS.has(path.extname(relativePath).toLowerCase())) {
      continue;
    }
    const root = MANIFEST_ROOTS.find((candidate) => relativePath.startsWith(`${candidate.publicRoot}/`));
    if (!root) continue;
    const size = normalizeRemoteReleaseFileSize(file);
    const sha256 = normalizeOptionalString(file.sha256 ?? file.h ?? "");
    const contentType = normalizeOptionalString(file.contentType ?? file.c ?? "");
    records.push({
      publicPath: `/${relativePath}`,
      source: root.source,
      size,
      sha256,
      contentType,
    });
  }

  if (records.length === 0) {
    throw new Error(`Remote asset release manifest has no original-map/original-ui PNG/CUR entries: ${source}`);
  }

  return {
    records,
    remoteRelease: {
      source,
      version: normalizeOptionalString(release.version ?? ""),
      assetBaseUrl: normalizeOptionalString(release.assetBaseUrl ?? ""),
      objectPrefix: normalizeOptionalString(release.objectPrefix ?? ""),
      fileCount: files.length,
      missingCount: Number.isFinite(Number(release.stats?.missingCount)) ? Number(release.stats.missingCount) : null,
    },
  };
}

function resolveRemoteReleaseManifestSource() {
  if (remoteReleaseManifestSource) return remoteReleaseManifestSource;
  const assetBaseUrl = normalizeOptionalString(
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? process.env.MIR2_ASSET_BASE_URL ?? "",
  ).replace(/\/+$/, "");
  if (assetBaseUrl) return `${assetBaseUrl}/remote-asset-release.json`;
  throw new Error(
    "remote-release mode requires --remoteRelease, MIR2_ORIGINAL_ASSET_REMOTE_RELEASE, or NEXT_PUBLIC_MIR2_ASSET_BASE_URL.",
  );
}

async function readRemoteReleaseManifest(source) {
  if (isHttpUrl(source)) {
    const response = await fetch(source, { cache: "no-store" });
    if (!response.ok) {
      throw new Error(`Remote asset release fetch failed: HTTP ${response.status} ${source}`);
    }
    return response.json();
  }
  return JSON.parse(await fs.readFile(path.resolve(source), "utf8"));
}

function normalizeRemoteReleaseRelativePath(file, release) {
  const value = normalizeOptionalString(file.relativePath ?? file.path ?? file.p ?? file.objectKey ?? file.k ?? "");
  if (!value) return "";
  let relativePath = value;
  if (isHttpUrl(relativePath)) {
    try {
      relativePath = new URL(relativePath).pathname;
    } catch {
      return "";
    }
  }
  relativePath = relativePath.replace(/^\/+/, "");
  const objectPrefix = normalizeOptionalString(release.objectPrefix ?? "").replace(/^\/+|\/+$/g, "");
  if (objectPrefix && relativePath.startsWith(`${objectPrefix}/`)) {
    relativePath = relativePath.slice(objectPrefix.length + 1);
  }
  return relativePath;
}

function normalizeRemoteReleaseFileSize(file) {
  const size = Number(file.size ?? file.s ?? 0);
  return Number.isFinite(size) && size > 0 ? size : 0;
}

async function collectFilesystemPngs() {
  const files = [];
  for (const root of MANIFEST_ROOTS) {
    const rootPath = path.join(PUBLIC_ROOT, root.publicRoot);
    if (!(await directoryExists(rootPath))) {
      continue;
    }
    for (const filePath of await listPngFilesRecursive(rootPath)) {
      files.push(filePath);
    }
  }
  return files.sort((left, right) => left.localeCompare(right));
}

async function collectGitTrackedPngs() {
  const result = spawnSync(
    "git",
    [
      "ls-files",
      "-z",
      "--cached",
      "--",
      "apps/web/public/original-map",
      "apps/web/public/original-ui",
    ],
    {
      cwd: REPO_ROOT,
      encoding: "utf8",
      maxBuffer: 128 * 1024 * 1024,
    },
  );
  if (result.status !== 0) {
    throw new Error(result.stderr || `git ls-files failed with exit ${result.status}`);
  }

  return result.stdout
    .split("\0")
    .filter(Boolean)
    .filter((filePath) => TRACKED_ASSET_EXTENSIONS.has(path.extname(filePath).toLowerCase()))
    .map((filePath) => path.join(REPO_ROOT, filePath))
    .sort((left, right) => left.localeCompare(right));
}

async function listPngFilesRecursive(root) {
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
      if (entry.isFile() && TRACKED_ASSET_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) {
        files.push(entryPath);
      }
    }
  }

  return files;
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

async function directoryExists(directoryPath) {
  try {
    const stats = await fs.stat(directoryPath);
    return stats.isDirectory();
  } catch (error) {
    if (error?.code === "ENOENT") return false;
    throw error;
  }
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) continue;
    const equals = token.indexOf("=");
    if (equals !== -1) {
      parsed[token.slice(2, equals)] = token.slice(equals + 1);
      continue;
    }

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

function positiveIntegerArg(value, fallback) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 1) return fallback;
  return Math.floor(parsed);
}

function normalizeOptionalString(value) {
  return String(value ?? "").trim();
}

function isHttpUrl(value) {
  return /^https?:\/\//i.test(String(value || ""));
}
