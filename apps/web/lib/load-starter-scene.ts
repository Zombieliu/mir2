import "server-only";

import { readFile } from "node:fs/promises";
import path from "node:path";

import type { OriginalMapRegion, SceneBlueprint } from "./scene-types";

type StarterSceneFile = {
  map?: {
    title?: string;
    mini_map?: number;
    big_map?: number;
  };
  scene_view?: {
    center?: {
      x?: number;
      y?: number;
    };
    width?: number;
    height?: number;
  };
  terrain_patches?: Array<{
    x?: number;
    y?: number;
    width?: number;
    height?: number;
    kind?: SceneBlueprint["terrainPatches"][number]["kind"];
  }>;
  decor_objects?: Array<{
    id?: string;
    x?: number;
    y?: number;
    kind?: SceneBlueprint["decorObjects"][number]["kind"];
  }>;
};

export async function loadStarterSceneBlueprint(): Promise<SceneBlueprint> {
  const [source, originalMapRegion] = await Promise.all([
    readFile(starterScenePath(), "utf8"),
    loadOriginalMapRegion(),
  ]);
  const parsed = JSON.parse(source) as StarterSceneFile;

  return {
    mapTitle: typeof parsed.map?.title === "string" ? parsed.map.title : null,
    miniMapIndex: typeof parsed.map?.mini_map === "number" ? parsed.map.mini_map : null,
    bigMapIndex: typeof parsed.map?.big_map === "number" ? parsed.map.big_map : null,
    sceneView: parsed.scene_view?.center
      ? {
          center: {
            x: numberOrZero(parsed.scene_view.center.x),
            y: numberOrZero(parsed.scene_view.center.y),
          },
          width: numberOrZero(parsed.scene_view.width),
          height: numberOrZero(parsed.scene_view.height),
        }
      : null,
    terrainPatches: (parsed.terrain_patches ?? []).map((patch) => ({
      x: numberOrZero(patch.x),
      y: numberOrZero(patch.y),
      width: numberOrZero(patch.width),
      height: numberOrZero(patch.height),
      kind: patch.kind ?? "grass",
    })),
    decorObjects: (parsed.decor_objects ?? []).map((decor, index) => ({
      id: typeof decor.id === "string" ? decor.id : `decor-${index}`,
      x: numberOrZero(decor.x),
      y: numberOrZero(decor.y),
      kind: decor.kind ?? "rock",
    })),
    originalMapRegion,
  };
}

function starterScenePath() {
  return path.resolve(
    process.cwd(),
    "..",
    "..",
    "packages",
    "game-data",
    "data",
    "starter_scene.json",
  );
}

async function loadOriginalMapRegion(): Promise<OriginalMapRegion | null> {
  try {
    const source = await readFile(originalMapRegionPath(), "utf8");
    return JSON.parse(source) as OriginalMapRegion;
  } catch {
    return null;
  }
}

function originalMapRegionPath() {
  return path.resolve(
    process.cwd(),
    "..",
    "..",
    "packages",
    "game-data",
    "data",
    "generated",
    "crystal_starter_map_region.json",
  );
}

function numberOrZero(value: unknown) {
  return typeof value === "number" ? value : 0;
}
