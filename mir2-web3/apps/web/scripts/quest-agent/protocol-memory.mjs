import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createInterface } from "node:readline";

const MONSTER_PACKETS = new Set(["ObjectMonster", "NewMonsterInfo"]);
const MAP_PACKETS = new Set(["MapInformation", "MapInfo", "MapChanged"]);

/**
 * Load only remembered monster locations from one already-redacted character
 * trace. Returned records are location hints, not live entities: object ids,
 * health and every unrelated trace field are deliberately discarded.
 */
export async function loadObservedMonsterLocations(tracePath, options = {}) {
  const maxPerMonsterMap = positiveInteger(options.maxPerMonsterMap, 256);
  const maxReadBytes = positiveInteger(options.maxReadBytes, 64 * 1024 * 1024);
  const buckets = new Map();
  let currentMap = null;
  let ordinal = 0;
  let size;
  try {
    size = (await stat(tracePath)).size;
  } catch (error) {
    if (error?.code === 'ENOENT') return [];
    throw error;
  }
  if (size === 0) return [];
  const start = Math.max(0, size - maxReadBytes);
  const lines = createInterface({
    input: createReadStream(tracePath, { encoding: "utf8", start }),
    crlfDelay: Infinity,
  });

  let discardPartialFirstLine = start > 0;
  for await (const line of lines) {
    if (discardPartialFirstLine) {
      discardPartialFirstLine = false;
      continue;
    }
    let entry;
    try { entry = JSON.parse(line); } catch { continue; }
    if (entry?.direction && entry.direction !== "received") continue;

    if (MAP_PACKETS.has(entry?.packet)) {
      currentMap = mapFileName(entry.payload) ?? currentMap;
      continue;
    }

    if (entry?.type === "worldSnapshot" || entry?.packet === "WorldSnapshot") {
      const snapshot = entry.payload ?? {};
      currentMap = mapFileName(snapshot) ?? currentMap;
      if (!currentMap) continue;
      for (const entity of snapshot.entities ?? []) {
        if (String(entity?.kind ?? "").toLowerCase() !== "monster") continue;
        remember(buckets, observation(entity, currentMap, ++ordinal), maxPerMonsterMap);
      }
      continue;
    }

    if (MONSTER_PACKETS.has(entry?.packet) && currentMap) {
      remember(buckets, observation(entry.payload, currentMap, ++ordinal), maxPerMonsterMap);
    }
  }

  return [...buckets.values()]
    .flatMap(bucket => [...bucket.values()])
    .sort((left, right) => right.ordinal - left.ordinal)
    .map(({ monsterName, mapFileName, x, y }) => ({ monsterName, mapFileName, x, y }));
}

function observation(source, map, ordinal) {
  const monsterName = String(source?.name ?? source?.monsterName ?? "").trim();
  const x = finiteNumber(source?.x ?? source?.location?.x);
  const y = finiteNumber(source?.y ?? source?.location?.y);
  if (!monsterName || x == null || y == null) return null;
  return { monsterName, mapFileName: String(map), x, y, ordinal };
}

function remember(buckets, value, limit) {
  if (!value) return;
  const bucketKey = `${value.monsterName.toLowerCase()}\u0000${value.mapFileName}`;
  let bucket = buckets.get(bucketKey);
  if (!bucket) {
    bucket = new Map();
    buckets.set(bucketKey, bucket);
  }
  const coordinateKey = `${value.x},${value.y}`;
  bucket.delete(coordinateKey);
  bucket.set(coordinateKey, value);
  while (bucket.size > limit) bucket.delete(bucket.keys().next().value);
}

function mapFileName(source) {
  const value = source?.mapFileName ?? source?.fileName ?? source?.map?.fileName;
  return value == null || String(value) === "" ? null : String(value);
}

function finiteNumber(value) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

function positiveInteger(value, fallback) {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : fallback;
}
