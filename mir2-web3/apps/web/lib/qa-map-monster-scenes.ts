import "server-only";

import respawnManifest from "./generated/crystal_respawn_manifest.json";

export type QaMapMonsterRespawn = {
  respawnIndex: number;
  monsterIndex: number | null;
  monsterName: string;
  monsterImage: number | null;
  location: { x: number; y: number };
  count: number;
  spread: number;
  delayMinutes: number;
  direction: string | null;
  routePath: string | null;
  randomDelayMinutes: number;
};

export type QaMapMonsterScene = {
  id: string;
  sceneIndex: number;
  mapIndex: number | null;
  mapFileName: string;
  mapTitle: string | null;
  center: { x: number; y: number };
  selectedRespawn: QaMapMonsterRespawn;
  visibleRespawns: QaMapMonsterRespawn[];
  dominantMonsters: string[];
};

export type QaMapMonsterSceneList = {
  generatedAt: string | null;
  limitPerMap: number;
  sourceMapCount: number;
  respawnMapCount: number;
  positiveRespawnRowCount: number;
  sceneCount: number;
  scenes: QaMapMonsterScene[];
};

type CrystalRespawnManifest = {
  generated_at?: string | null;
  total_maps?: number;
  maps?: CrystalRespawnMap[];
};

type CrystalRespawnMap = {
  map_index?: number | null;
  map_file_name?: string | null;
  map_title?: string | null;
  respawns?: CrystalRespawnRow[];
};

type CrystalRespawnRow = {
  monster_index?: number | null;
  location?: { x?: number | null; y?: number | null } | null;
  count?: number | null;
  spread?: number | null;
  delay_minutes?: number | null;
  direction?: string | null;
  route_path?: string | null;
  random_delay_minutes?: number | null;
  respawn_index?: number | null;
  monster_name?: string | null;
  monster_image?: number | null;
};

const DEFAULT_LIMIT_PER_MAP = 3;
const MAX_LIMIT_PER_MAP = 10;
const VIEWPORT_HALF_WIDTH = 18;
const VIEWPORT_HALF_HEIGHT = 14;
const MAX_VISIBLE_RESPAWNS = 36;

const manifest = respawnManifest as CrystalRespawnManifest;

export function normalizeQaMapMonsterLimit(value: string | number | null | undefined): number {
  const parsed = typeof value === "number" ? value : Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return DEFAULT_LIMIT_PER_MAP;
  return Math.max(1, Math.min(Math.trunc(parsed), MAX_LIMIT_PER_MAP));
}

export function buildQaMapMonsterScenes(limitPerMapInput?: string | number | null): QaMapMonsterSceneList {
  const limitPerMap = normalizeQaMapMonsterLimit(limitPerMapInput);
  const maps = Array.isArray(manifest.maps) ? manifest.maps : [];
  const scenes: QaMapMonsterScene[] = [];
  let positiveRespawnRowCount = 0;
  let respawnMapCount = 0;

  for (const map of maps) {
    const respawns = positiveRespawnsForMap(map);
    if (respawns.length === 0) continue;

    positiveRespawnRowCount += respawns.length;
    respawnMapCount += 1;

    const representatives = selectRepresentativeRespawns(respawns, limitPerMap);
    for (const selectedRespawn of representatives) {
      const sceneIndex = scenes.length;
      const visibleRespawns = visibleRespawnsForScene(respawns, selectedRespawn);
      scenes.push({
        id: buildSceneId(sceneIndex, map.map_file_name, selectedRespawn),
        sceneIndex,
        mapIndex: integerOrNull(map.map_index),
        mapFileName: normalizeMapFileName(map.map_file_name),
        mapTitle: typeof map.map_title === "string" && map.map_title.length > 0 ? map.map_title : null,
        center: selectedRespawn.location,
        selectedRespawn,
        visibleRespawns,
        dominantMonsters: dominantMonsters(visibleRespawns),
      });
    }
  }

  return {
    generatedAt: typeof manifest.generated_at === "string" ? manifest.generated_at : null,
    limitPerMap,
    sourceMapCount: typeof manifest.total_maps === "number" ? manifest.total_maps : maps.length,
    respawnMapCount,
    positiveRespawnRowCount,
    sceneCount: scenes.length,
    scenes,
  };
}

export function getQaMapMonsterScene(
  sceneIdOrIndex: string | number | null | undefined,
  limitPerMapInput?: string | number | null,
): { list: QaMapMonsterSceneList; scene: QaMapMonsterScene | null } {
  const list = buildQaMapMonsterScenes(limitPerMapInput);
  if (sceneIdOrIndex === null || sceneIdOrIndex === undefined || sceneIdOrIndex === "") {
    return { list, scene: list.scenes[0] ?? null };
  }

  const sceneKey = String(sceneIdOrIndex);
  const parsedIndex = Number.parseInt(sceneKey, 10);
  const scene = Number.isFinite(parsedIndex) && String(parsedIndex) === sceneKey
    ? list.scenes[parsedIndex] ?? null
    : list.scenes.find((candidate) => candidate.id === sceneKey) ?? null;

  return { list, scene };
}

function positiveRespawnsForMap(map: CrystalRespawnMap): QaMapMonsterRespawn[] {
  const respawns = Array.isArray(map.respawns) ? map.respawns : [];
  return respawns
    .map((respawn) => normalizeRespawn(respawn))
    .filter((respawn): respawn is QaMapMonsterRespawn => respawn !== null)
    .sort((a, b) => {
      if (b.count !== a.count) return b.count - a.count;
      if (b.spread !== a.spread) return b.spread - a.spread;
      if (a.monsterName !== b.monsterName) return a.monsterName.localeCompare(b.monsterName);
      return a.respawnIndex - b.respawnIndex;
    });
}

function normalizeRespawn(respawn: CrystalRespawnRow): QaMapMonsterRespawn | null {
  const x = integerOrNull(respawn.location?.x);
  const y = integerOrNull(respawn.location?.y);
  const count = integerOrNull(respawn.count) ?? 0;
  if (x === null || y === null || count <= 0) return null;

  const monsterIndex = integerOrNull(respawn.monster_index);
  const monsterName = typeof respawn.monster_name === "string" && respawn.monster_name.trim().length > 0
    ? respawn.monster_name.trim()
    : monsterIndex === null
      ? "Unknown"
      : `Monster ${monsterIndex}`;

  return {
    respawnIndex: integerOrNull(respawn.respawn_index) ?? 0,
    monsterIndex,
    monsterName,
    monsterImage: integerOrNull(respawn.monster_image),
    location: { x, y },
    count,
    spread: integerOrNull(respawn.spread) ?? 0,
    delayMinutes: integerOrNull(respawn.delay_minutes) ?? 0,
    direction: typeof respawn.direction === "string" ? respawn.direction : null,
    routePath: typeof respawn.route_path === "string" ? respawn.route_path : null,
    randomDelayMinutes: integerOrNull(respawn.random_delay_minutes) ?? 0,
  };
}

function selectRepresentativeRespawns(
  respawns: QaMapMonsterRespawn[],
  limitPerMap: number,
): QaMapMonsterRespawn[] {
  const selected: QaMapMonsterRespawn[] = [];
  const pickPass = (minDistance: number, uniqueMonster: boolean) => {
    for (const respawn of respawns) {
      if (selected.length >= limitPerMap) break;
      if (selected.includes(respawn)) continue;
      if (uniqueMonster && selected.some((candidate) => sameMonster(candidate, respawn))) continue;
      if (selected.some((candidate) => tileDistance(candidate.location, respawn.location) < minDistance)) continue;
      selected.push(respawn);
    }
  };

  pickPass(48, true);
  pickPass(20, true);
  pickPass(48, false);
  pickPass(0, false);

  return selected.slice(0, limitPerMap);
}

function visibleRespawnsForScene(
  respawns: QaMapMonsterRespawn[],
  selectedRespawn: QaMapMonsterRespawn,
): QaMapMonsterRespawn[] {
  return respawns
    .filter((respawn) => {
      if (respawn === selectedRespawn) return true;
      const spreadCells = Math.ceil(Math.max(0, respawn.spread) / 48);
      return (
        Math.abs(respawn.location.x - selectedRespawn.location.x) <= VIEWPORT_HALF_WIDTH + spreadCells &&
        Math.abs(respawn.location.y - selectedRespawn.location.y) <= VIEWPORT_HALF_HEIGHT + spreadCells
      );
    })
    .sort((a, b) => {
      if (a === selectedRespawn) return -1;
      if (b === selectedRespawn) return 1;
      const distanceDelta =
        tileDistance(a.location, selectedRespawn.location) -
        tileDistance(b.location, selectedRespawn.location);
      if (distanceDelta !== 0) return distanceDelta;
      if (b.count !== a.count) return b.count - a.count;
      return a.respawnIndex - b.respawnIndex;
    })
    .slice(0, MAX_VISIBLE_RESPAWNS);
}

function dominantMonsters(respawns: QaMapMonsterRespawn[]): string[] {
  const names: string[] = [];
  for (const respawn of respawns) {
    if (!names.includes(respawn.monsterName)) names.push(respawn.monsterName);
    if (names.length >= 8) break;
  }
  return names;
}

function buildSceneId(
  sceneIndex: number,
  mapFileName: string | null | undefined,
  respawn: QaMapMonsterRespawn,
): string {
  const index = String(sceneIndex).padStart(4, "0");
  return [
    index,
    safeIdPart(normalizeMapFileName(mapFileName)),
    safeIdPart(respawn.monsterName),
    respawn.location.x,
    respawn.location.y,
    respawn.respawnIndex,
  ].join("-");
}

function sameMonster(a: QaMapMonsterRespawn, b: QaMapMonsterRespawn): boolean {
  return a.monsterName.toLowerCase() === b.monsterName.toLowerCase();
}

function tileDistance(a: { x: number; y: number }, b: { x: number; y: number }): number {
  return Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
}

function normalizeMapFileName(value: string | null | undefined): string {
  const trimmed = typeof value === "string" ? value.trim() : "";
  return trimmed.length > 0 ? trimmed.replace(/\.map$/i, "") : "0";
}

function safeIdPart(value: string): string {
  return value
    .trim()
    .replace(/[^a-z0-9]+/gi, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40) || "item";
}

function integerOrNull(value: unknown): number | null {
  if (typeof value !== "number" || !Number.isFinite(value)) return null;
  return Math.trunc(value);
}
