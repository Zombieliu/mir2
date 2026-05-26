import { createHash } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";

import { loadCrystalSceneBlueprint } from "./crystal-map-loader";
import type { SceneBlueprint } from "./scene-types";

type CrystalSceneBlueprintRequest = {
  mapFileName: string | null;
  centerX: number | null;
  centerY: number | null;
  width: number | null;
  height: number | null;
};

type CachedCrystalSceneBlueprint = {
  blueprint: SceneBlueprint;
  cacheStatus: "hit" | "miss" | "bypass";
  cacheKey: string;
};

const MAX_MEMORY_BLUEPRINTS = 128;
const SCENE_CACHE_SCHEMA_VERSION = "2026-05-21-v2";
const webRoot = process.cwd();
const cacheDir = path.join(webRoot, ".next/cache/mir2-scene-blueprints");
const memoryBlueprints = new Map<string, Promise<SceneBlueprint>>();
const disableRequestFileWrites = process.env.MIR2_DISABLE_REQUEST_FILE_WRITES === "1";

export async function loadCachedCrystalSceneBlueprint(
  request: CrystalSceneBlueprintRequest,
): Promise<CachedCrystalSceneBlueprint> {
  const cacheKey = await createSceneCacheKey(request);

  if (process.env.MIR2_SCENE_BLUEPRINT_CACHE === "0") {
    return {
      blueprint: await loadCrystalSceneBlueprint(request),
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
    pending = loadCrystalSceneBlueprint(request).then(async (blueprint) => {
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
    const raw = await readFile(path.join(cacheDir, `${cacheKey}.json`), "utf8");
    return JSON.parse(raw) as SceneBlueprint;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
      console.warn("[mir2] failed to read scene blueprint cache", error);
    }
    return null;
  }
}

async function writeDiskBlueprint(cacheKey: string, blueprint: SceneBlueprint) {
  if (disableRequestFileWrites) return;

  try {
    await mkdir(cacheDir, { recursive: true });
    const targetPath = path.join(cacheDir, `${cacheKey}.json`);
    const tempPath = `${targetPath}.${process.pid}.${Date.now()}.tmp`;
    await writeFile(tempPath, JSON.stringify(blueprint), "utf8");
    await rename(tempPath, targetPath);
  } catch (error) {
    console.warn("[mir2] failed to write scene blueprint cache", error);
  }
}

async function createSceneCacheKey(request: CrystalSceneBlueprintRequest) {
  const canonicalRequest = {
    mapFileName: request.mapFileName?.trim() || "0",
    centerX: canonicalNumber(request.centerX),
    centerY: canonicalNumber(request.centerY),
    width: canonicalNumber(request.width),
    height: canonicalNumber(request.height),
  };
  return createHash("sha256")
    .update(SCENE_CACHE_SCHEMA_VERSION)
    .update(process.env.MIR2_SCENE_CACHE_BUSTER ?? "")
    .update(JSON.stringify(canonicalRequest))
    .digest("hex")
    .slice(0, 32);
}

function canonicalNumber(value: number | null) {
  return Number.isFinite(value) ? value : null;
}
