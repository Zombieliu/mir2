import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const DEFAULT_ATLAS_ROOT = path.join(PUBLIC_ROOT, "generated", "map-atlas");
const DEFAULT_OBJECT_PREFIX = "mir2/v/20260730-fullcrystal-f71b89aa-gzip1";
const IMMUTABLE_CACHE_CONTROL = "public, max-age=31536000, immutable";

const args = parseArgs(process.argv.slice(2));
const atlasRoot = path.resolve(args.atlasRoot ?? DEFAULT_ATLAS_ROOT);
const objectPrefix = normalizeObjectPrefix(args.objectPrefix ?? DEFAULT_OBJECT_PREFIX);
const outputPath = path.resolve(
  args.output ?? path.join(atlasRoot, "map-atlas-release.generated.json"),
);

await main();

async function main() {
  const manifestPath = args.manifest
    ? path.resolve(args.manifest)
    : await findContentAddressedManifest(atlasRoot);
  const manifestBytes = await fs.readFile(manifestPath);
  const manifestHash = sha256(manifestBytes);
  const expectedManifestName = `manifest.${manifestHash}.json`;
  if (path.basename(manifestPath) !== expectedManifestName) {
    throw new Error(
      `Map-atlas manifest is not content addressed: expected ${expectedManifestName}, found ${path.basename(manifestPath)}`,
    );
  }

  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  if (manifest.schemaVersion !== 2 || !Array.isArray(manifest.pages) || manifest.pages.length === 0) {
    throw new Error("Expected a non-empty compact map-atlas schemaVersion 2 manifest.");
  }

  const pagePaths = [...new Set(manifest.pages.map((page) => normalizePublicPath(page.u)))].sort();
  if (pagePaths.length !== manifest.pages.length) {
    throw new Error("Map-atlas manifest contains duplicate or invalid page URLs.");
  }

  const files = [];
  for (const publicPath of pagePaths) {
    if (!/^generated\/map-atlas\/.+\/p\d+\.[0-9a-f]{16}\.png$/.test(publicPath)) {
      throw new Error(`Map-atlas page is not content addressed: /${publicPath}`);
    }
    files.push(await releaseFile(publicPath, "image/png"));
  }

  const manifestRelativePath = normalizePublicPath(
    `generated/map-atlas/${path.basename(manifestPath)}`,
  );
  files.push(await releaseFile(manifestRelativePath, "application/json; charset=utf-8"));

  const release = {
    schemaVersion: 1,
    kind: "mir2-map-atlas-immutable-release",
    generatedAt: new Date().toISOString(),
    objectPrefix,
    cacheControl: IMMUTABLE_CACHE_CONTROL,
    mapAtlas: {
      schemaVersion: 2,
      contentHash: manifestHash,
      manifestPath: `/${manifestRelativePath}`,
      pageCount: pagePaths.length,
      maxPageBytes: Math.max(...files.slice(0, -1).map((file) => file.size)),
      totalBytes: files.reduce((sum, file) => sum + file.size, 0),
    },
    files,
  };

  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(release, null, 2)}\n`);
  console.log(JSON.stringify({ ok: true, outputPath, ...release.mapAtlas }, null, 2));
}

async function releaseFile(relativePath, contentType) {
  const stagePath = resolveInside(PUBLIC_ROOT, relativePath);
  const bytes = await fs.readFile(stagePath);
  const fileHash = sha256(bytes);
  const basename = path.basename(relativePath);
  const embeddedHash = basename.match(/\.([0-9a-f]{16,64})\.(?:png|json)$/)?.[1] ?? "";
  if (!fileHash.startsWith(embeddedHash)) {
    throw new Error(`Content hash mismatch for /${relativePath}: ${fileHash}`);
  }
  return {
    path: `/${relativePath}`,
    relativePath,
    stagePath,
    objectKey: `${objectPrefix}/${relativePath}`,
    size: bytes.length,
    contentType,
    cacheControl: IMMUTABLE_CACHE_CONTROL,
    sha256: fileHash,
    sources: ["map-atlas-v2"],
  };
}

async function findContentAddressedManifest(root) {
  const names = (await fs.readdir(root))
    .filter((name) => /^manifest\.[0-9a-f]{64}\.json$/.test(name))
    .sort();
  if (names.length !== 1) {
    throw new Error(
      `Expected exactly one content-addressed map-atlas manifest in ${root}; found ${names.length}.`,
    );
  }
  return path.join(root, names[0]);
}

function normalizePublicPath(value) {
  const relativePath = String(value ?? "").trim().replace(/^\/+/, "");
  if (!relativePath || relativePath.includes("..") || relativePath.includes("\\")) {
    throw new Error(`Unsafe public asset path: ${value}`);
  }
  return relativePath;
}

function resolveInside(root, relativePath) {
  const resolved = path.resolve(root, relativePath);
  const rootPrefix = `${path.resolve(root)}${path.sep}`;
  if (!resolved.startsWith(rootPrefix)) {
    throw new Error(`Asset path escapes public root: ${relativePath}`);
  }
  return resolved;
}

function normalizeObjectPrefix(value) {
  const prefix = String(value ?? "").trim().replace(/^\/+|\/+$/g, "");
  if (!prefix.startsWith("mir2/v/") || prefix.includes("..") || prefix.includes("\\")) {
    throw new Error(`Unsafe immutable object prefix: ${value}`);
  }
  return prefix;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (!value.startsWith("--")) continue;
    const [rawKey, inlineValue] = value.slice(2).split("=", 2);
    parsed[rawKey] = inlineValue ?? values[index + 1];
    if (inlineValue == null) index += 1;
  }
  return parsed;
}
