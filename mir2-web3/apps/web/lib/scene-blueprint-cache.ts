import { createHash } from "node:crypto";
import { mkdir, readFile, readdir, rename, stat, unlink, writeFile } from "node:fs/promises";
import path from "node:path";

import { loadCrystalSceneBlueprint } from "./crystal-map-loader";
import {
  createCrystalSceneBlueprintRequestKey,
  normalizeCrystalSceneBlueprintRequest,
  type CrystalSceneBlueprintRequest,
} from "./scene-blueprint-request";
import type { SceneBlueprint } from "./scene-types";

type CachedCrystalSceneBlueprint = {
  blueprint: SceneBlueprint;
  cacheStatus: "hit" | "miss" | "bypass";
  cacheKey: string;
};

const MAX_MEMORY_BLUEPRINTS = 128;
const SCENE_CACHE_DISK_TTL_MS = positiveNumberEnv("MIR2_SCENE_BLUEPRINT_CACHE_TTL_MS", 7 * 24 * 60 * 60 * 1000);
const SCENE_CACHE_DISK_MAX_ENTRIES = positiveNumberEnv("MIR2_SCENE_BLUEPRINT_CACHE_MAX_ENTRIES", 512);
const SCENE_CACHE_DISK_MAX_BYTES = positiveNumberEnv("MIR2_SCENE_BLUEPRINT_CACHE_MAX_BYTES", 64 * 1024 * 1024);
const webRoot = process.cwd();
const cacheDir = path.join(webRoot, ".next/cache/mir2-scene-blueprints");
const memoryBlueprints = new Map<string, Promise<SceneBlueprint>>();
const requestFileWritesEnabled =
  process.env.MIR2_ENABLE_REQUEST_FILE_WRITES === "1" ||
  (process.env.NODE_ENV === "development" && process.env.MIR2_DISABLE_REQUEST_FILE_WRITES !== "1");
let lastDiskTrimAt = 0;

export async function loadCachedCrystalSceneBlueprint(
  request: CrystalSceneBlueprintRequest,
): Promise<CachedCrystalSceneBlueprint> {
  const normalizedRequest = normalizeCrystalSceneBlueprintRequest(request);
  const cacheKey = createSceneCacheKey(normalizedRequest);

  if (process.env.MIR2_SCENE_BLUEPRINT_CACHE === "0") {
    return {
      blueprint: await loadCrystalSceneBlueprint(normalizedRequest),
      cacheStatus: "bypass",
      cacheKey,
    };
  }

  const cached = await readDiskBlueprint(cacheKey);
  if (cached) {
    return { blueprint: cached, cacheStatus: "hit", cacheKey };
  }

  let pending = memoryBlueprints.get(cacheKey);
  if (!pending) {
    pending = loadCrystalSceneBlueprint(normalizedRequest).then(async (blueprint) => {
      await writeDiskBlueprint(cacheKey, blueprint);
      return blueprint;
    });
    rememberBlueprint(cacheKey, pending);
  }

  return {
    blueprint: await pending,
    cacheStatus: "miss",
    cacheKey,
  };
}

function rememberBlueprint(cacheKey: string, pending: Promise<SceneBlueprint>) {
  memoryBlueprints.set(cacheKey, pending);
  if (memoryBlueprints.size <= MAX_MEMORY_BLUEPRINTS) return;
  const oldestKey = memoryBlueprints.keys().next().value as string | undefined;
  if (oldestKey) memoryBlueprints.delete(oldestKey);
}

async function readDiskBlueprint(cacheKey: string) {
  try {
    const filePath = path.join(cacheDir, `${cacheKey}.json`);
    const fileStat = await stat(filePath);
    if (Date.now() - fileStat.mtimeMs > SCENE_CACHE_DISK_TTL_MS) {
      await unlink(filePath).catch(() => undefined);
      return null;
    }
    const raw = await readFile(filePath, "utf8");
    return JSON.parse(raw) as SceneBlueprint;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      console.warn("[mir2] failed to read scene blueprint cache", error);
    }
    return null;
  }
}

async function writeDiskBlueprint(cacheKey: string, blueprint: SceneBlueprint) {
  if (!requestFileWritesEnabled) return;

  try {
    await mkdir(cacheDir, { recursive: true });
    const targetPath = path.join(cacheDir, `${cacheKey}.json`);
    const tempPath = `${targetPath}.${process.pid}.${Date.now()}.tmp`;
    await writeFile(tempPath, JSON.stringify(blueprint), "utf8");
    await rename(tempPath, targetPath);
    await trimDiskBlueprintCache();
  } catch (error) {
    console.warn("[mir2] failed to write scene blueprint cache", error);
  }
}

function createSceneCacheKey(request: CrystalSceneBlueprintRequest) {
  const buster = process.env.MIR2_SCENE_CACHE_BUSTER
    ? createHash("sha1").update(process.env.MIR2_SCENE_CACHE_BUSTER).digest("hex").slice(0, 8)
    : "default";
  return createCrystalSceneBlueprintRequestKey(request, buster);
}

async function trimDiskBlueprintCache() {
  if (!requestFileWritesEnabled) return;
  const now = Date.now();
  if (now - lastDiskTrimAt < 60_000) return;
  lastDiskTrimAt = now;

  try {
    const entries = await Promise.all(
      (await readdir(cacheDir))
        .filter((name) => name.endsWith(".json"))
        .map(async (name) => {
          const filePath = path.join(cacheDir, name);
          const fileStat = await stat(filePath);
          return { name, filePath, mtimeMs: fileStat.mtimeMs, size: fileStat.size };
        }),
    );
    const fresh: typeof entries = [];
    for (const entry of entries) {
      if (now - entry.mtimeMs > SCENE_CACHE_DISK_TTL_MS) {
        await unlink(entry.filePath).catch(() => undefined);
      } else {
        fresh.push(entry);
      }
    }

    fresh.sort((a, b) => a.mtimeMs - b.mtimeMs);
    let totalBytes = fresh.reduce((sum, entry) => sum + entry.size, 0);
    while (
      fresh.length > SCENE_CACHE_DISK_MAX_ENTRIES ||
      (totalBytes > SCENE_CACHE_DISK_MAX_BYTES && fresh.length > 1)
    ) {
      const entry = fresh.shift();
      if (!entry) break;
      totalBytes -= entry.size;
      await unlink(entry.filePath).catch(() => undefined);
    }
  } catch (error) {
    console.warn("[mir2] failed to trim scene blueprint cache", error);
  }
}

function positiveNumberEnv(name: string, fallback: number) {
  const parsed = Number.parseInt(process.env[name] ?? "", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

export function createSceneCacheKeyForTests(request: CrystalSceneBlueprintRequest) {
  return createSceneCacheKey(normalizeCrystalSceneBlueprintRequest(request));
}

export function normalizeSceneBlueprintRequestForTests(request: CrystalSceneBlueprintRequest) {
  return normalizeCrystalSceneBlueprintRequest(request);
}
