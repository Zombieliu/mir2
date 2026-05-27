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

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  if (!["filesystem", "git"].includes(collectionMode)) {
    throw new Error(`Unsupported --mode ${collectionMode}; expected "filesystem" or "git".`);
  }

  const files = collectionMode === "git" ? await collectGitTrackedPngs() : await collectFilesystemPngs();
  const entries = [];
  const assetHash = createHash("sha256");
  let totalBytes = 0;
  const stats = {
    originalMapPngCount: 0,
    originalUiPngCount: 0,
  };

  for (const filePath of files) {
    const relativePath = path.relative(PUBLIC_ROOT, filePath).split(path.sep).join("/");
    const publicPath = `/${relativePath}`;
    const root = MANIFEST_ROOTS.find((candidate) => relativePath.startsWith(`${candidate.publicRoot}/`));
    if (!root) continue;

    const file = await fs.readFile(filePath);
    const sha256 = createHash("sha256").update(file).digest("hex");
    totalBytes += file.length;
    assetHash.update(publicPath);
    assetHash.update(String(file.length));
    assetHash.update(sha256);

    if (root.source === "original-map") stats.originalMapPngCount += 1;
    if (root.source === "original-ui") stats.originalUiPngCount += 1;

    entries.push([
      publicPath,
      {
        size: file.length,
        sha256,
        source: root.source,
      },
    ]);
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
      },
      null,
      2,
    ),
  );
}

async function collectFilesystemPngs() {
  const files = [];
  for (const root of MANIFEST_ROOTS) {
    const rootPath = path.join(PUBLIC_ROOT, root.publicRoot);
    if (!(await directoryExists(rootPath))) {
      continue;
    }
    files.push(...(await listPngFilesRecursive(rootPath)));
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
  const entries = await fs.readdir(root, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listPngFilesRecursive(entryPath)));
      continue;
    }
    if (entry.isFile() && TRACKED_ASSET_EXTENSIONS.has(path.extname(entry.name).toLowerCase())) {
      files.push(entryPath);
    }
  }

  return files;
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
