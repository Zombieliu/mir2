import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
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
];
const PUBLIC_ASSET_EXTENSIONS = new Set([
  ".cur",
  ".gif",
  ".jpeg",
  ".jpg",
  ".mp3",
  ".ogg",
  ".png",
  ".wav",
  ".webp",
]);

const args = parseArgs(process.argv.slice(2));
const baseUrl = normalizeUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL);
const runId = String(args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14));
const outputDir = path.resolve(args.outDir ?? process.env.MIR2_REMOTE_ASSET_OUTPUT_DIR ?? path.join(DEFAULT_OUTPUT_ROOT, runId));
const allowMissing = booleanArg(args.allowMissing ?? process.env.MIR2_REMOTE_ASSET_ALLOW_MISSING, false);
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
const publicAssetRoots = parseListArg(
  args.publicAssetRoots ?? process.env.MIR2_REMOTE_ASSET_PUBLIC_ROOTS,
  DEFAULT_PUBLIC_ASSET_ROOTS,
);
const stageConcurrency = positiveIntegerArg(
  args.stageConcurrency ?? process.env.MIR2_REMOTE_ASSET_STAGE_CONCURRENCY,
  32,
);

async function main() {
  const manifestUrl = new URL("/api/asset-manifest", baseUrl);
  const assetManifest = await fetchJson(manifestUrl);
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
      sceneSpriteRootCount: collected.sceneSpriteRoots.length,
      sceneSpriteFileCount: collected.sceneSpriteRoots.reduce((sum, root) => sum + root.fileCount, 0),
      publicAssetRootCount: collected.publicAssetRoots.length,
      publicAssetFileCount: collected.publicAssetRoots.reduce((sum, root) => sum + root.fileCount, 0),
      fileCount: staged.files.length,
      missingCount: staged.missing.length,
      totalBytes: staged.files.reduce((sum, file) => sum + file.size, 0),
      stageConcurrency,
    },
    packs: collected.packs,
    scenes: collected.scenes,
    sceneSpriteRoots: collected.sceneSpriteRoots,
    publicAssetRoots: collected.publicAssetRoots,
    files: staged.files,
    missing: staged.missing,
  };

  if (release.missing.length > 0 && !allowMissing) {
    throw new Error(`Remote asset release has ${release.missing.length} missing files`);
  }

  await fs.mkdir(outputDir, { recursive: true });
  const releasePath = path.join(outputDir, "remote-asset-release.json");
  const latestPath = path.join(DEFAULT_OUTPUT_ROOT, "latest-remote-asset-release.json");
  await fs.writeFile(releasePath, `${JSON.stringify(release, null, 2)}\n`, "utf8");
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
      },
      null,
      2,
    ),
  );
}

async function collectReleaseUrls(assetManifest, manifestUrl) {
  const staticUrls = new Map();
  const packs = [];
  const scenes = [];
  const sceneSpriteRootRecords = [];
  const publicAssetRootRecords = [];
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

  if (includePublicAssetRoots) {
    publicAssetRootRecords.push(...(await collectPublicAssetRootStaticUrls(staticUrls, publicAssetRoots)));
  }

  return {
    staticUrls,
    packs,
    scenes,
    sceneSpriteRoots: sceneSpriteRootRecords,
    publicAssetRoots: publicAssetRootRecords,
  };
}

function addStaticUrl(staticUrls, value, source) {
  const assetPath = normalizeStaticAssetPath(value);
  if (!assetPath) return false;
  const existing = staticUrls.get(assetPath);
  if (existing) {
    existing.sources.add(source);
    return false;
  }
  staticUrls.set(assetPath, { path: assetPath, sources: new Set([source]) });
  return true;
}

async function stageStaticFiles({ staticUrls, stageDir, objectPrefix, allowMissing, concurrency }) {
  await fs.mkdir(stageDir, { recursive: true });
  const entries = [...staticUrls.values()].sort((a, b) => a.path.localeCompare(b.path));
  const results = await mapWithConcurrency(entries, concurrency, (entry) =>
    stageOneStaticFile({ entry, stageDir, objectPrefix }),
  );

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

async function stageOneStaticFile({ entry, stageDir, objectPrefix }) {
  const relativePath = decodeAssetRelativePath(entry.path.replace(/^\/+/, ""));
  const localPath = path.join(WEB_ROOT, "public", relativePath);
  const stagePath = path.join(stageDir, relativePath);

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

  const contents = await fs.readFile(localPath);
  await fs.mkdir(path.dirname(stagePath), { recursive: true });
  await fs.writeFile(stagePath, contents);

  return {
    file: {
      path: entry.path,
      relativePath,
      localPath,
      stagePath,
      objectKey: joinObjectKey(objectPrefix, relativePath),
      size: stats.size,
      sha256: createHash("sha256").update(contents).digest("hex"),
      contentType: contentTypeForPath(relativePath),
      cacheControl: DEFAULT_CACHE_CONTROL,
      sources: [...entry.sources].sort(),
    },
  };
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
  for (const sprite of Object.values(sprites)) {
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
    !url.pathname.startsWith("/generated/original-map-blend/")
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
  const entries = await fs.readdir(root, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFilesRecursive(entryPath)));
      continue;
    }
    if (entry.isFile()) files.push(entryPath);
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
    root !== "generated/original-map-blend"
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

function normalizeAssetBaseUrl(value) {
  return String(value || "").trim().replace(/\/+$/, "");
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
