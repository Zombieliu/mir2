import fs from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const DEFAULT_CRYSTAL_CLIENT_ROOT = "E:\\mir2\\Crystal\\Build\\Client\\Debug";
const CRYSTAL_CLIENT_ROOT = process.env.CRYSTAL_CLIENT_ROOT ?? DEFAULT_CRYSTAL_CLIENT_ROOT;
const MAP_DIR = path.join(CRYSTAL_CLIENT_ROOT, "Map");
const RESPAWN_MANIFEST_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_respawn_manifest.json",
);
const NPC_INFO_MANIFEST_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_npc_info_manifest.json",
);
const NPC_SCRIPT_MANIFEST_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_npc_manifest.json",
);
const NPC_COMMAND_SUMMARY_PATH = path.join(
  REPO_ROOT,
  "packages",
  "game-data",
  "data",
  "generated",
  "crystal_npc_command_summary.json",
);
const OUTPUT_PATH = path.resolve(
  REPO_ROOT,
  process.env.MIR2_MAP_GAMEPLAY_AUDIT_OUT ?? "docs/generated/map/latest-crystal-map-gameplay.json",
);

const respawnManifest = readJson(RESPAWN_MANIFEST_PATH);
const npcInfoManifest = readJson(NPC_INFO_MANIFEST_PATH);
const npcScriptManifest = readJson(NPC_SCRIPT_MANIFEST_PATH);
const npcCommandSummary = readJson(NPC_COMMAND_SUMMARY_PATH);
const maps = Array.isArray(respawnManifest.maps) ? respawnManifest.maps : [];
const npcs = Array.isArray(npcInfoManifest.npcs) ? npcInfoManifest.npcs : [];
const scripts = Array.isArray(npcScriptManifest.scripts) ? npcScriptManifest.scripts : [];
const scriptsByKey = new Map(scripts.map((script) => [normalizeScriptKey(script.script_key), script]));
const mapsByIndex = new Map(maps.map((map) => [Number(map.map_index), map]));
const mapsByFile = new Map(maps.map((map) => [normalizeMapFileName(map.map_file_name), map]));
const mapFilesByName = indexFilesByNormalizedStem(MAP_DIR, ".map", false);
const parsedMapCache = new Map();

const failures = [];
const warnings = [];
const movementResults = [];
const respawnResults = [];
const npcResults = [];
const staticResults = [];

for (const map of maps) {
  auditStaticMapSemantics(map);
  for (const movement of map.movements ?? []) auditMovement(map, movement);
  for (const respawn of map.respawns ?? []) auditRespawn(map, respawn);
}

for (const npc of npcs) auditNpc(npc);

const failedMovements = movementResults.filter((entry) => entry.failures.length > 0);
const failedRespawns = respawnResults.filter((entry) => entry.failures.length > 0);
const failedNpcs = npcResults.filter((entry) => entry.failures.length > 0);
const failedStatic = staticResults.filter((entry) => entry.failures.length > 0);
for (const entry of failedMovements) failures.push(...entry.failures.map((failure) => `movement ${entry.mapFileName}: ${failure}`));
for (const entry of failedRespawns) failures.push(...entry.failures.map((failure) => `respawn ${entry.mapFileName}: ${failure}`));
for (const entry of failedNpcs) failures.push(...entry.failures.map((failure) => `npc ${entry.name}: ${failure}`));
for (const entry of failedStatic) failures.push(...entry.failures.map((failure) => `static ${entry.mapFileName}: ${failure}`));

const summary = {
  generatedAt: new Date().toISOString(),
  crystalClientRoot: CRYSTAL_CLIENT_ROOT,
  manifest: {
    totalMaps: maps.length,
    totalMovements: movementResults.length,
    totalRespawns: respawnResults.length,
    totalNpcs: npcResults.length,
    totalNpcScripts: scripts.length,
    totalNpcCommands: Number(npcCommandSummary.total_commands ?? 0),
    unimplementedNpcCommands: Number(npcCommandSummary.unimplemented_commands ?? 0),
    unimplementedNpcOccurrences: Number(npcCommandSummary.unimplemented_occurrences ?? 0),
  },
  mapGameplayCoverage: {
    mapsWithMovement: uniqueCount(movementResults.map((entry) => entry.mapFileName)),
    movementsChecked: movementResults.length,
    directMovements: movementResults.filter((entry) => entry.status === "direct").length,
    ignoredMovements: movementResults.filter((entry) => entry.status === "ignored").length,
    movementFailures: failedMovements.length,
    mapsWithRespawns: uniqueCount(respawnResults.map((entry) => entry.mapFileName)),
    respawnsChecked: respawnResults.length,
    respawnsWithWalkableCandidates: respawnResults.filter((entry) => entry.walkableCandidateCount > 0).length,
    respawnsWithoutWalkableCandidates: respawnResults.filter((entry) => entry.walkableCandidateCount === 0).length,
    crystalInertRespawns: respawnResults.filter((entry) => entry.crystalInert).length,
    respawnFailures: failedRespawns.length,
    mapsWithNpcs: uniqueCount(npcResults.map((entry) => entry.mapFileName)),
    npcsChecked: npcResults.length,
    npcFailures: failedNpcs.length,
    mapsWithSafeZones: maps.filter((map) => (map.safe_zones ?? []).length > 0).length,
    mapsWithSafeZoneSpells: maps.filter((map) => (map.safe_zone_spells ?? []).length > 0).length,
    mapsWithLight: maps.filter((map) => Number(map.light) !== 0).length,
    mapsWithDropRules: maps.filter((map) => map.no_throw_item || map.no_drop_player || map.no_drop_monster).length,
    mapsWithDoors: staticResults.filter((entry) => entry.doorCells > 0).length,
    mapsWithCellLights: staticResults.filter((entry) => entry.lightCells > 0).length,
    mapsWithFishingCells: staticResults.filter((entry) => entry.fishingCells > 0).length,
    staticFailures: failedStatic.length,
  },
  npcScriptCoverage: {
    scriptsTotal: scripts.length,
    npcInfoRows: npcs.length,
    npcRowsWithScripts: npcResults.filter((entry) => entry.scriptFound).length,
    npcRowsMissingScripts: npcResults.filter((entry) => !entry.scriptFound).length,
    npcRowsWithEmptyPlaceholderScripts: npcResults.filter((entry) => entry.scriptFound && entry.scriptPlaceholder).length,
    implementedCommands: Number(npcCommandSummary.implemented_commands ?? 0),
    unimplementedCommands: Number(npcCommandSummary.unimplemented_commands ?? 0),
    unimplementedOccurrences: Number(npcCommandSummary.unimplemented_occurrences ?? 0),
  },
  warnings,
  failures,
  samples: {
    movementFailures: failedMovements.slice(0, 25),
    respawnFailures: failedRespawns.slice(0, 25),
    npcFailures: failedNpcs.slice(0, 25),
    staticFailures: failedStatic.slice(0, 25),
  },
  movements: movementResults,
  respawns: respawnResults,
  npcs: npcResults,
  staticMaps: staticResults,
};

console.log(JSON.stringify(summaryBrief(summary), null, 2));
await mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
await writeFile(OUTPUT_PATH, `${JSON.stringify(summary, null, 2)}\n`);
console.log(`Wrote ${OUTPUT_PATH}`);

if (process.env.MIR2_MAP_GAMEPLAY_STRICT === "1" && failures.length > 0) {
  process.exitCode = 1;
}

function auditStaticMapSemantics(map) {
  const result = {
    ...mapIdentity(map),
    width: null,
    height: null,
    doorCells: 0,
    closedDoorCells: 0,
    lightCells: 0,
    fishingCells: 0,
    blockedCells: 0,
    failures: [],
  };
  const parsed = loadParsedMapForManifestMap(map);
  if (!parsed) {
    result.failures.push("map file is missing or unparseable");
    staticResults.push(result);
    return;
  }
  result.width = parsed.width;
  result.height = parsed.height;
  for (const cell of parsed.cells ?? []) {
    if (cell.doorIndex > 0) result.doorCells += 1;
    if (cell.doorIndex > 0 && cell.frontImage !== 0) result.closedDoorCells += 1;
    if (cell.light > 0) result.lightCells += 1;
    if (cell.light >= 100 && cell.light <= 119) result.fishingCells += 1;
    if (cellBlocksMovement(cell)) result.blockedCells += 1;
  }
  for (const zone of map.safe_zones ?? []) {
    if (!pointInMap(zone.location, parsed)) {
      result.failures.push(`safe zone out of map bounds at ${pointLabel(zone.location)}`);
    }
  }
  for (const spell of map.safe_zone_spells ?? []) {
    if (!pointInMap(spell.location, parsed)) {
      result.failures.push(`safe-zone spell out of map bounds at ${pointLabel(spell.location)}`);
    }
  }
  staticResults.push(result);
}

function auditMovement(map, movement) {
  const targetMap = mapsByIndex.get(Number(movement.map_index));
  const result = {
    ...mapIdentity(map),
    source: pointSummary(movement.source),
    destination: pointSummary(movement.destination),
    targetMapIndex: Number(movement.map_index),
    targetMapFileName: normalizeMapFileName(targetMap?.map_file_name ?? ""),
    needHole: movement.need_hole === true,
    needMove: movement.need_move === true,
    conquestIndex: Number(movement.conquest_index ?? 0),
    sourceBlocked: null,
    destinationBlocked: null,
    status: "direct",
    ignoredReason: null,
    failures: [],
  };
  const sourceMap = loadParsedMapForManifestMap(map);
  if (!sourceMap) {
    result.failures.push("source map file is missing or unparseable");
  } else if (!pointInMap(movement.source, sourceMap)) {
    ignoreMovement(result, `source out of map bounds at ${pointLabel(movement.source)}`);
  } else {
    result.sourceBlocked = pointBlocksMovement(movement.source, sourceMap);
    if (result.sourceBlocked) ignoreMovement(result, `source is not a Crystal ValidPoint at ${pointLabel(movement.source)}`);
  }
  if (!targetMap) {
    ignoreMovement(result, `target map index ${movement.map_index} is not present in manifest`);
  } else {
    const parsedTarget = loadParsedMapForManifestMap(targetMap);
    if (!parsedTarget) {
      result.failures.push("target map file is missing or unparseable");
    } else if (!pointInMap(movement.destination, parsedTarget)) {
      ignoreMovement(result, `destination out of target map bounds at ${pointLabel(movement.destination)}`);
    } else {
      result.destinationBlocked = pointBlocksMovement(movement.destination, parsedTarget);
      if (result.destinationBlocked) ignoreMovement(result, `destination is not a Crystal ValidPoint at ${pointLabel(movement.destination)}`);
    }
  }
  if (movement.need_hole === true) ignoreMovement(result, "requires DigOut spell object at the source tile");
  if (movement.need_move === true) ignoreMovement(result, "NPC ENTERMAP deferred movement, not direct map movement");
  if (Number(movement.conquest_index ?? 0) > 0) ignoreMovement(result, "requires Crystal conquest ownership gate");
  movementResults.push(result);
}

function auditRespawn(map, respawn) {
  const result = {
    ...mapIdentity(map),
    respawnIndex: Number(respawn.respawn_index ?? 0),
    monsterIndex: Number(respawn.monster_index ?? 0),
    monsterName: respawn.monster_name ?? null,
    location: pointSummary(respawn.location),
    count: Number(respawn.count ?? 0),
    spread: Number(respawn.spread ?? 0),
    routePath: respawn.route_path ?? null,
    routePoints: Array.isArray(respawn.route) ? respawn.route.length : 0,
    walkableCandidateCount: 0,
    crystalInert: false,
    failures: [],
  };
  const parsed = loadParsedMapForManifestMap(map);
  if (!parsed) {
    result.failures.push("map file is missing or unparseable");
  } else {
    result.walkableCandidateCount = countWalkableCandidatesInRespawnRect(parsed, respawn.location, result.spread);
    if (result.walkableCandidateCount === 0 && result.count > 0) {
      result.crystalInert = true;
      warnings.push(
        `respawn ${result.mapFileName} ${result.respawnIndex}: no Crystal walkable cells in location/spread rectangle ${pointLabel(respawn.location)} ± ${result.spread}; Crystal keeps the row but MonsterObject.Spawn(MapRespawn) returns false`,
      );
    }
    for (const routePoint of respawn.route ?? []) {
      if (!pointInMap(routePoint, parsed)) {
        warnings.push(`respawn ${result.mapFileName} ${result.respawnIndex}: route point out of map bounds at ${pointLabel(routePoint)}; Crystal loads routes but movement AI can only step onto valid map cells`);
      }
    }
  }
  if (!Number.isFinite(result.monsterIndex) || result.monsterIndex <= 0) {
    result.failures.push("monster index is missing");
  }
  if (result.count < 0) result.failures.push("respawn count is negative");
  respawnResults.push(result);
}

function auditNpc(npc) {
  const map = mapsByIndex.get(Number(npc.map_index)) ?? mapsByFile.get(normalizeMapFileName(npc.map_file_name));
  const scriptKey = normalizeScriptKey(npc.script_key ?? npc.file_name ?? "");
  const script = scriptsByKey.get(scriptKey);
  const result = {
    npcIndex: Number(npc.npc_index ?? 0),
    name: npc.name ?? "",
    mapIndex: Number(npc.map_index ?? 0),
    mapFileName: normalizeMapFileName(npc.map_file_name),
    location: pointSummary(npc.location),
    scriptKey,
    scriptFound: Boolean(script),
    scriptLineCount: script?.line_count ?? 0,
    scriptNonEmptyLineCount: script?.non_empty_line_count ?? 0,
    scriptPlaceholder: Boolean(script) && Number(script.non_empty_line_count ?? 0) <= 0,
    failures: [],
  };
  if (!map) {
    result.failures.push(`npc map index ${npc.map_index} is not present in manifest`);
  } else {
    const parsed = loadParsedMapForManifestMap(map);
    if (!parsed) {
      result.failures.push("npc map file is missing or unparseable");
    } else if (!pointInMap(npc.location, parsed)) {
      result.failures.push(`npc location out of map bounds at ${pointLabel(npc.location)}`);
    }
  }
  if (!script) {
    result.failures.push(`script ${scriptKey} is missing from NPC script manifest`);
  } else if (Number(script.non_empty_line_count ?? 0) <= 0) {
    warnings.push(`npc ${result.name} ${scriptKey}: script file exists but is empty; Crystal treats this as an inert placeholder dialog`);
  }
  npcResults.push(result);
}

function ignoreMovement(result, reason) {
  if (result.status === "ignored") {
    result.ignoredReason = `${result.ignoredReason}; ${reason}`;
    return;
  }
  result.status = "ignored";
  result.ignoredReason = reason;
}

function loadParsedMapForManifestMap(map) {
  const normalized = normalizeMapFileName(map?.map_file_name ?? "");
  if (parsedMapCache.has(normalized)) return parsedMapCache.get(normalized);
  const mapPath = mapFilesByName.get(normalized) ?? path.join(MAP_DIR, `${normalized}.map`);
  if (!fs.existsSync(mapPath)) {
    parsedMapCache.set(normalized, null);
    return null;
  }
  try {
    const parsed = parseMapBytes(`${normalized}.map`, fs.readFileSync(mapPath));
    if (!Array.isArray(parsed.cells)) {
      parsedMapCache.set(normalized, null);
      return null;
    }
    parsedMapCache.set(normalized, parsed);
    return parsed;
  } catch (error) {
    warnings.push(`${normalized}: ${error instanceof Error ? error.message : String(error)}`);
    parsedMapCache.set(normalized, null);
    return null;
  }
}

function parseMapBytes(fileName, bytes) {
  const type = detectMapType(bytes);
  switch (type) {
    case 100:
      return parseType100Map(fileName, bytes);
    case 0:
      return parseType0Map(fileName, bytes);
    case 1:
      return parseType1Map(fileName, bytes);
    case 2:
      return parseType2Map(fileName, bytes);
    case 3:
      return parseType3Map(fileName, bytes);
    case 4:
      return parseType4Map(fileName, bytes);
    case 5:
      return parseType5Map(fileName, bytes);
    case 6:
      return parseType6Map(fileName, bytes);
    case 7:
      return parseType7Map(fileName, bytes);
    default:
      return { fileName, width: detectMapWidth(bytes, type), height: detectMapHeight(bytes, type), type, cells: null };
  }
}

function parseType0Map(fileName, bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = [];
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 12 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: normalizeBackImage(bytes.readInt16LE(offset)),
        middleIndex: 1,
        middleImage: bytes.readInt16LE(offset + 2),
        frontImage: bytes.readInt16LE(offset + 4),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 2,
        light: bytes.readUInt8(offset + 11),
      });
      offset += 12;
    }
  }
  return { fileName, width, height, type: 0, cells };
}

function parseType1Map(fileName, bytes) {
  const xor = bytes.readInt16LE(23);
  const width = bytes.readInt16LE(21) ^ xor;
  const height = bytes.readInt16LE(25) ^ xor;
  const cells = [];
  let offset = 54;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 15 > bytes.length) break;
      let frontIndex = bytes.readUInt8(offset + 12) + 2;
      if (frontIndex === 102) frontIndex = 90;
      if (frontIndex >= 255) frontIndex = -1;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: (bytes.readInt32LE(offset) ^ 0xaa38aa38) | 0,
        middleIndex: 1,
        middleImage: signed16(bytes.readInt16LE(offset + 4) ^ xor),
        frontImage: signed16(bytes.readInt16LE(offset + 6) ^ xor),
        doorIndex: bytes.readUInt8(offset + 8) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 9),
        frontAnimationFrame: bytes.readUInt8(offset + 10),
        frontAnimationTick: bytes.readUInt8(offset + 11),
        frontIndex,
        light: bytes.readUInt8(offset + 13),
      });
      offset += 15;
    }
  }
  return { fileName, width, height, type: 1, cells };
}

function parseType2Map(fileName, bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = [];
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 14 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backImage: normalizeBackImage(bytes.readInt16LE(offset)),
        middleImage: bytes.readInt16LE(offset + 2),
        frontImage: bytes.readInt16LE(offset + 4),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 120,
        light: bytes.readUInt8(offset + 11),
        backIndex: bytes.readUInt8(offset + 12) + 100,
        middleIndex: bytes.readUInt8(offset + 13) + 110,
      });
      offset += 14;
    }
  }
  return { fileName, width, height, type: 2, cells };
}

function parseType3Map(fileName, bytes) {
  const width = bytes.readInt16LE(0);
  const height = bytes.readInt16LE(2);
  const cells = [];
  let offset = 52;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 36 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backImage: normalizeBackImage(bytes.readInt16LE(offset)),
        middleImage: bytes.readInt16LE(offset + 2),
        frontImage: bytes.readInt16LE(offset + 4),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 120,
        light: bytes.readUInt8(offset + 11),
        backIndex: bytes.readUInt8(offset + 12) + 100,
        middleIndex: bytes.readUInt8(offset + 13) + 110,
        tileAnimationImage: bytes.readInt16LE(offset + 14),
        tileAnimationFrames: bytes.readUInt8(offset + 21),
        tileAnimationOffset: bytes.readInt16LE(offset + 22),
      });
      offset += 36;
    }
  }
  return { fileName, width, height, type: 3, cells };
}

function parseType4Map(fileName, bytes) {
  const xor = bytes.readInt16LE(33);
  const width = bytes.readInt16LE(31) ^ xor;
  const height = bytes.readInt16LE(35) ^ xor;
  const cells = [];
  let offset = 64;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 12 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: normalizeBackImage(signed16(bytes.readInt16LE(offset) ^ xor)),
        middleIndex: 1,
        middleImage: signed16(bytes.readInt16LE(offset + 2) ^ xor),
        frontImage: signed16(bytes.readInt16LE(offset + 4) ^ xor),
        doorIndex: bytes.readUInt8(offset + 6) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 7),
        frontAnimationFrame: bytes.readUInt8(offset + 8),
        frontAnimationTick: bytes.readUInt8(offset + 9),
        frontIndex: bytes.readUInt8(offset + 10) + 2,
        light: bytes.readUInt8(offset + 11),
      });
      offset += 12;
    }
  }
  return { fileName, width, height, type: 4, cells };
}

function parseType5Map(fileName, bytes) {
  const width = bytes.readInt16LE(22);
  const height = bytes.readInt16LE(24);
  const cells = createEmptyCellGrid(width, height);
  let offset = 28;
  for (let x = 0; x < Math.floor(width / 2); x += 1) {
    for (let y = 0; y < Math.floor(height / 2); y += 1) {
      if (offset + 3 > bytes.length) break;
      const backIndex = bytes.readUInt8(offset) !== 255 ? bytes.readUInt8(offset) + 200 : -1;
      const backImage = bytes.readUInt16LE(offset + 1) + 1;
      for (let index = 0; index < 4; index += 1) {
        const cell = cells[(x * 2 + (index % 2)) * height + (y * 2 + Math.floor(index / 2))];
        if (!cell) continue;
        cell.backIndex = backIndex;
        cell.backImage = backImage;
      }
      offset += 3;
    }
  }
  offset = 28 + 3 * (Math.floor(width / 2) + (width % 2)) * Math.floor(height / 2);
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 14 > bytes.length) break;
      const cell = cells[x * height + y];
      const flag = bytes.readUInt8(offset);
      cell.middleAnimationFrame = bytes.readUInt8(offset + 1);
      cell.frontAnimationFrame = bytes.readUInt8(offset + 2) === 255 ? 0 : bytes.readUInt8(offset + 2) & 0x8f;
      cell.middleAnimationTick = 0;
      cell.frontAnimationTick = 0;
      cell.frontIndex = bytes.readUInt8(offset + 3) !== 255 ? bytes.readUInt8(offset + 3) + 200 : -1;
      cell.middleIndex = bytes.readUInt8(offset + 4) !== 255 ? bytes.readUInt8(offset + 4) + 200 : -1;
      cell.middleImage = bytes.readUInt16LE(offset + 5) + 1;
      cell.frontImage = bytes.readUInt16LE(offset + 7) + 1;
      if (cell.frontImage === 1 && cell.frontIndex === 200) cell.frontIndex = -1;
      cell.light = (bytes.readUInt8(offset + 12) & 0x0f) * 2;
      if ((flag & 0x01) !== 1) cell.backImage |= 0x20000000;
      if ((flag & 0x02) !== 2) cell.frontImage |= 0x8000;
      offset += 14;
    }
  }
  return { fileName, width, height, type: 5, cells };
}

function parseType6Map(fileName, bytes) {
  const width = bytes.readInt16LE(16);
  const height = bytes.readInt16LE(18);
  const cells = [];
  let offset = 40;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 20 > bytes.length) break;
      const flag = bytes.readUInt8(offset);
      let frontAnimationFrame = bytes.readUInt8(offset + 11) === 255 ? 0 : bytes.readUInt8(offset + 11);
      if (frontAnimationFrame > 0x0f) frontAnimationFrame &= 0x0f;
      let frontIndex = bytes.readUInt8(offset + 3) !== 255 ? bytes.readUInt8(offset + 3) + 300 : -1;
      const baseFrontImage = bytes.readInt16LE(offset + 8) + 1;
      if (baseFrontImage === 1 && frontIndex === 200) frontIndex = -1;
      const cell = {
        ...emptyParsedMapCell(x, y),
        backIndex: bytes.readUInt8(offset + 1) !== 255 ? bytes.readUInt8(offset + 1) + 300 : -1,
        middleIndex: bytes.readUInt8(offset + 2) !== 255 ? bytes.readUInt8(offset + 2) + 300 : -1,
        frontIndex,
        backImage: bytes.readInt16LE(offset + 4) + 1,
        middleImage: bytes.readInt16LE(offset + 6) + 1,
        frontImage: (flag & 0x02) !== 2 ? baseFrontImage | 0x8000 : baseFrontImage,
        middleAnimationFrame: bytes.readUInt8(offset + 10),
        frontAnimationFrame,
        middleAnimationTick: 1,
        frontAnimationTick: 1,
        light: (bytes.readUInt8(offset + 12) & 0x0f) * 4,
      };
      if ((flag & 0x01) !== 1) cell.backImage |= 0x20000000;
      cells.push(cell);
      offset += 20;
    }
  }
  return { fileName, width, height, type: 6, cells };
}

function parseType7Map(fileName, bytes) {
  const width = bytes.readInt16LE(21);
  const height = bytes.readInt16LE(25);
  const cells = [];
  let offset = 54;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 15 > bytes.length) break;
      cells.push({
        ...emptyParsedMapCell(x, y),
        backIndex: 0,
        backImage: normalizeBackImage(bytes.readInt32LE(offset)),
        middleIndex: 1,
        middleImage: bytes.readInt16LE(offset + 4),
        frontImage: bytes.readInt16LE(offset + 6),
        doorIndex: bytes.readUInt8(offset + 8) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 9),
        frontAnimationFrame: bytes.readUInt8(offset + 10),
        frontAnimationTick: bytes.readUInt8(offset + 11),
        frontIndex: bytes.readUInt8(offset + 12) + 2,
        light: bytes.readUInt8(offset + 13),
      });
      offset += 15;
    }
  }
  return { fileName, width, height, type: 7, cells };
}

function parseType100Map(fileName, bytes) {
  const width = bytes.readInt16LE(4);
  const height = bytes.readInt16LE(6);
  const cells = [];
  let offset = 8;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (offset + 26 > bytes.length) break;
      cells.push({
        x,
        y,
        backIndex: bytes.readInt16LE(offset),
        backImage: bytes.readInt32LE(offset + 2),
        middleIndex: bytes.readInt16LE(offset + 6),
        middleImage: bytes.readInt16LE(offset + 8),
        frontIndex: bytes.readInt16LE(offset + 10),
        frontImage: bytes.readInt16LE(offset + 12),
        doorIndex: bytes.readUInt8(offset + 14) & 0x7f,
        doorOffset: bytes.readUInt8(offset + 15),
        frontAnimationFrame: bytes.readUInt8(offset + 16),
        frontAnimationTick: bytes.readUInt8(offset + 17),
        middleAnimationFrame: bytes.readUInt8(offset + 18),
        middleAnimationTick: bytes.readUInt8(offset + 19),
        tileAnimationImage: bytes.readInt16LE(offset + 20),
        tileAnimationOffset: bytes.readInt16LE(offset + 22),
        tileAnimationFrames: bytes.readUInt8(offset + 24),
        light: bytes.readUInt8(offset + 25),
      });
      offset += 26;
    }
  }
  return { fileName, width, height, type: 100, cells };
}

function detectMapType(bytes) {
  if (bytes[2] === 0x43 && bytes[3] === 0x23) return 100;
  if (bytes[0] === 0) return 5;
  if (bytes[0] === 0x0f && bytes[5] === 0x53 && bytes[14] === 0x33) return 6;
  if (bytes[0] === 0x15 && bytes[4] === 0x32 && bytes[6] === 0x41 && bytes[19] === 0x31) return 4;
  if (bytes[0] === 0x10 && bytes[2] === 0x61 && bytes[7] === 0x31 && bytes[14] === 0x31) return 1;
  if (bytes[4] === 0x0f || (bytes[4] === 0x03 && bytes[18] === 0x0d && bytes[19] === 0x0a)) {
    const width = bytes[0] + (bytes[1] << 8);
    const height = bytes[2] + (bytes[3] << 8);
    return bytes.length > 52 + width * height * 14 ? 3 : 2;
  }
  if (bytes[0] === 0x0d && bytes[1] === 0x4c && bytes[7] === 0x20 && bytes[11] === 0x6d) return 7;
  return 0;
}

function detectMapWidth(bytes, type) {
  if (type === 1) return bytes.readInt16LE(21) ^ bytes.readInt16LE(23);
  if (type === 4) return bytes.readInt16LE(31) ^ bytes.readInt16LE(33);
  if (type === 5) return bytes.readInt16LE(22);
  if (type === 6) return bytes.readInt16LE(16);
  if (type === 7) return bytes.readInt16LE(21);
  return bytes.readInt16LE(0);
}

function detectMapHeight(bytes, type) {
  if (type === 1) return bytes.readInt16LE(25) ^ bytes.readInt16LE(23);
  if (type === 4) return bytes.readInt16LE(35) ^ bytes.readInt16LE(33);
  if (type === 5) return bytes.readInt16LE(24);
  if (type === 6) return bytes.readInt16LE(18);
  if (type === 7) return bytes.readInt16LE(25);
  return bytes.readInt16LE(2);
}

function emptyParsedMapCell(x, y) {
  return {
    x,
    y,
    backIndex: -1,
    backImage: 0,
    middleIndex: -1,
    middleImage: 0,
    frontIndex: -1,
    frontImage: 0,
    doorIndex: 0,
    doorOffset: 0,
    frontAnimationFrame: 0,
    frontAnimationTick: 0,
    middleAnimationFrame: 0,
    middleAnimationTick: 0,
    tileAnimationImage: 0,
    tileAnimationOffset: 0,
    tileAnimationFrames: 0,
    light: 0,
  };
}

function createEmptyCellGrid(width, height) {
  const cells = [];
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) cells.push(emptyParsedMapCell(x, y));
  }
  return cells;
}

function pointBlocksMovement(point, parsedMap) {
  const cell = parsedMap.cells?.[Number(point.x) * parsedMap.height + Number(point.y)];
  return cell ? cellBlocksMovement(cell) : null;
}

function cellBlocksMovement(cell) {
  return (cell.backImage & 0x20000000) !== 0 || (cell.frontImage & 0x8000) !== 0;
}

function countWalkableCandidatesInRespawnRect(parsedMap, origin, spreadValue) {
  const spread = Number(spreadValue ?? 0);
  const minX = Math.max(0, Number(origin?.x ?? 0) - spread);
  const maxX = Math.min(parsedMap.width - 1, Number(origin?.x ?? 0) + spread);
  const minY = Math.max(0, Number(origin?.y ?? 0) - spread);
  const maxY = Math.min(parsedMap.height - 1, Number(origin?.y ?? 0) + spread);
  if (minX > maxX || minY > maxY) return 0;
  let count = 0;
  for (let x = minX; x <= maxX; x += 1) {
    for (let y = minY; y <= maxY; y += 1) {
      const cell = parsedMap.cells?.[x * parsedMap.height + y];
      if (cell && !cellBlocksMovement(cell)) count += 1;
    }
  }
  return count;
}

function pointInMap(point, parsedMap) {
  const x = Number(point?.x);
  const y = Number(point?.y);
  return Number.isInteger(x) && Number.isInteger(y) && x >= 0 && y >= 0 && x < parsedMap.width && y < parsedMap.height;
}

function pointLabel(point) {
  return `${point?.x ?? "?"}:${point?.y ?? "?"}`;
}

function pointSummary(point) {
  return {
    x: Number.isFinite(Number(point?.x)) ? Number(point.x) : null,
    y: Number.isFinite(Number(point?.y)) ? Number(point.y) : null,
  };
}

function mapIdentity(map) {
  return {
    mapIndex: Number(map?.map_index ?? 0),
    mapFileName: normalizeMapFileName(map?.map_file_name ?? ""),
    title: map?.map_title ?? null,
  };
}

function indexFilesByNormalizedStem(dir, extension, recursive) {
  const result = new Map();
  if (!fs.existsSync(dir)) return result;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (recursive) {
        for (const [key, value] of indexFilesByNormalizedStem(entryPath, extension, true)) result.set(key, value);
      }
      continue;
    }
    if (!entry.isFile() || !entry.name.toLowerCase().endsWith(extension.toLowerCase())) continue;
    result.set(normalizeMapFileName(entry.name.slice(0, -extension.length)), entryPath);
  }
  return result;
}

function normalizeMapFileName(mapFileName) {
  const normalized = String(mapFileName || "0").trim().replaceAll("\\", "/").split("/").pop() ?? "0";
  return normalized.replace(/\.map$/i, "") || "0";
}

function normalizeScriptKey(scriptKey) {
  return String(scriptKey ?? "").trim().replaceAll("\\", "/").replace(/\.txt$/i, "");
}

function normalizeBackImage(image) {
  return (image & 0x8000) !== 0 ? (image & 0x7fff) | 0x20000000 : image;
}

function signed16(value) {
  return (value << 16) >> 16;
}

function uniqueCount(values) {
  return new Set(values.filter(Boolean)).size;
}

function summaryBrief(summary) {
  return {
    generatedAt: summary.generatedAt,
    manifest: summary.manifest,
    mapGameplayCoverage: summary.mapGameplayCoverage,
    npcScriptCoverage: summary.npcScriptCoverage,
    failureCount: summary.failures.length,
    warningCount: summary.warnings.length,
  };
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}
