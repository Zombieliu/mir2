#!/usr/bin/env node

// Static pre-implementation region certificate for newcomer-v2. It deliberately
// reads only generated Crystal data and collision packs; it never connects to a
// gateway or controls a character.
import { createHash } from 'node:crypto';
import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  findProtocolWalkPath,
  loadProtocolCollisionMap,
  protocolMapCellIsWalkable,
} from './protocol-navigation.mjs';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, '../../../..');
const respawnManifestPath = path.join(
  repositoryRoot, 'packages/game-data/data/generated/crystal_respawn_manifest.json',
);
const npcManifestPath = path.join(
  repositoryRoot, 'packages/game-data/data/generated/crystal_npc_info_manifest.json',
);
const outputDirectory = path.join(
  repositoryRoot, 'docs/generated/player-qa/newcomer-v2-20260918',
);
const outputJsonPath = path.join(outputDirectory, 'regions.json');
const outputMarkdownPath = path.join(outputDirectory, 'regions.md');

const TARGETS = Object.freeze({
  D001: Object.freeze(['Skeleton']),
  D401: Object.freeze(['Zombie2', 'Zombie3']),
  D022: Object.freeze(['Dung', 'WoomaSoldier', 'WoomaFighter']),
});
const SAMPLE_OFFSETS = Object.freeze([
  Object.freeze([0, 0]),
  Object.freeze([1, 0]), Object.freeze([-1, 0]),
  Object.freeze([0, 1]), Object.freeze([0, -1]),
  Object.freeze([1, 1]), Object.freeze([-1, 1]),
  Object.freeze([1, -1]), Object.freeze([-1, -1]),
]);

const readJson = async sourcePath => JSON.parse(await fs.readFile(sourcePath, 'utf8'));
const digest = async sourcePath => createHash('sha256').update(await fs.readFile(sourcePath)).digest('hex');
const relative = sourcePath => path.relative(repositoryRoot, sourcePath).replaceAll(path.sep, '/');
const point = value => ({ x: Number(value.x), y: Number(value.y) });
const key = value => `${value.x},${value.y}`;
const samePoint = (left, right) => left.x === right.x && left.y === right.y;
const chebyshev = (left, right) => Math.max(Math.abs(left.x - right.x), Math.abs(left.y - right.y));

function mapByFileName(manifest, mapFileName) {
  const result = manifest.maps.find(map => String(map.map_file_name).toUpperCase() === mapFileName.toUpperCase());
  if (!result) throw new Error(`Missing authoritative map ${mapFileName}`);
  return result;
}

function movementBetween(fromMap, toMap) {
  const movement = (fromMap.movements ?? []).find(entry => Number(entry.map_index) === Number(toMap.map_index));
  if (!movement) throw new Error(`Missing MapMovement ${fromMap.map_file_name} -> ${toMap.map_file_name}`);
  return movement;
}

function exitTo(map, destination) {
  const movement = movementBetween(map, destination);
  return {
    source: point(movement.source),
    destination: point(movement.destination),
    destinationMapFileName: String(destination.map_file_name),
    icon: Number(movement.icon),
  };
}

function transferObstacles(map, allowed = []) {
  const permitted = new Set(allowed.map(key));
  return [...new Map((map.movements ?? []).map(entry => {
    const source = point(entry.source);
    return [key(source), source];
  })).values()].filter(source => !permitted.has(key(source)));
}

function walkableAnchorNear(map, nominal, radius, obstacles) {
  const maximumRadius = Math.max(0, Math.min(12, Math.trunc(radius)));
  for (let distance = 0; distance <= maximumRadius; distance += 1) {
    for (let dy = -distance; dy <= distance; dy += 1) {
      for (let dx = -distance; dx <= distance; dx += 1) {
        if (Math.max(Math.abs(dx), Math.abs(dy)) !== distance) continue;
        const candidate = { x: nominal.x + dx, y: nominal.y + dy };
        if (protocolMapCellIsWalkable(map, candidate, obstacles)) return candidate;
      }
    }
  }
  return null;
}

function spawnSamples(map, spawn, obstacles) {
  const center = point(spawn.location);
  const stride = Math.max(4, Math.min(32, Math.floor(Number(spawn.spread) / 3)));
  const samples = [];
  for (const [xFactor, yFactor] of SAMPLE_OFFSETS) {
    const nominal = { x: center.x + xFactor * stride, y: center.y + yFactor * stride };
    const anchor = walkableAnchorNear(map, nominal, 12, obstacles);
    if (anchor && !samples.some(candidate => samePoint(candidate, anchor))) {
      samples.push({ nominal, anchor, centerDistance: chebyshev(center, anchor) });
    }
  }
  return samples;
}

function describePath(pathPoints) {
  if (!pathPoints?.length) return null;
  return {
    steps: pathPoints.length - 1,
    start: point(pathPoints[0]),
    end: point(pathPoints.at(-1)),
    firstSteps: pathPoints.slice(0, 4).map(point),
    lastSteps: pathPoints.slice(-4).map(point),
  };
}

function staticPath(map, start, target, overrides = [], obstacles = []) {
  return describePath(findProtocolWalkPath({
    map,
    start,
    target,
    staticWalkableOverrides: overrides,
    dynamicObstacles: obstacles,
    maxExpanded: map.width * map.height,
  }));
}

async function certifyCombatRegion({ manifest, mapFileName, entryFromMapFileName, targetMonsterNames }) {
  const sourceMap = mapByFileName(manifest, entryFromMapFileName);
  const mapInfo = mapByFileName(manifest, mapFileName);
  const incoming = movementBetween(sourceMap, mapInfo);
  const outgoing = exitTo(mapInfo, sourceMap);
  const map = await loadProtocolCollisionMap(mapFileName);
  const entry = point(incoming.destination);
  const exit = outgoing.source;
  const baseObstacles = transferObstacles(mapInfo, [exit]);
  const relevantSpawns = (mapInfo.respawns ?? []).filter(row => targetMonsterNames.includes(String(row.monster_name)));
  if (relevantSpawns.length === 0) throw new Error(`${mapFileName} has no required target respawns`);

  const groups = [];
  for (const spawn of relevantSpawns) {
    const samples = spawnSamples(map, spawn, baseObstacles);
    const candidatePaths = samples.map(sample => ({
      ...sample,
      entryPath: staticPath(map, entry, sample.anchor, [], baseObstacles),
    })).filter(sample => sample.entryPath);
    const selected = candidatePaths.sort((left, right) => left.entryPath.steps - right.entryPath.steps)[0] ?? null;
    const exitPath = selected
      ? staticPath(map, selected.anchor, exit, [exit], transferObstacles(mapInfo, [exit]))
      : null;
    groups.push({
      monsterName: String(spawn.monster_name),
      respawnIndex: Number(spawn.respawn_index),
      spawnCenter: point(spawn.location),
      count: Number(spawn.count),
      spread: Number(spawn.spread),
      sampleAnchorCount: samples.length,
      reachableSampleCount: candidatePaths.length,
      selectedAnchor: selected ? selected.anchor : null,
      selectedAnchorDistanceFromSpawnCenter: selected?.centerDistance ?? null,
      entryPath: selected?.entryPath ?? null,
      exitPath,
      staticPass: selected != null && exitPath != null,
      samples: candidatePaths.map(sample => ({
        nominal: sample.nominal,
        anchor: sample.anchor,
        anchorDistanceFromSpawnCenter: sample.centerDistance,
        entryPath: sample.entryPath,
      })),
    });
  }
  return {
    mapFileName,
    mapTitle: String(mapInfo.map_title),
    collision: {
      sourcePath: relative(map.sourcePath),
      width: map.width,
      height: map.height,
      mapType: map.type,
      closedDoorCount: map.doors.length,
    },
    entry: {
      fromMapFileName: String(sourceMap.map_file_name),
      source: point(incoming.source),
      destination: entry,
      icon: Number(incoming.icon),
    },
    exit: outgoing,
    blockedUnselectedTransferSources: baseObstacles,
    groups,
    staticPass: groups.every(group => group.staticPass),
  };
}

async function certifyVillage(manifest, npcManifest) {
  const mapInfo = mapByFileName(manifest, '0');
  const jane = (npcManifest.npcs ?? []).find(npc => String(npc.name) === 'Assistant_Jane' && String(npc.map_file_name) === '0');
  if (!jane) throw new Error('Missing authoritative Assistant_Jane on map 0');
  const start = (mapInfo.safe_zones ?? []).find(zone => zone.start_point === true);
  if (!start) throw new Error('Missing authoritative map-0 start safe zone');
  const map = await loadProtocolCollisionMap('0');
  const startPoint = point(start.location);
  const npcPoint = point(jane.location);
  const transferSources = transferObstacles(mapInfo);
  const adjacentCandidates = SAMPLE_OFFSETS.slice(1).map(([dx, dy]) => ({ x: npcPoint.x + dx, y: npcPoint.y + dy }))
    .filter(candidate => protocolMapCellIsWalkable(map, candidate, transferSources))
    .map(candidate => ({ candidate, path: staticPath(map, startPoint, candidate, [], transferSources) }))
    .filter(candidate => candidate.path)
    .sort((left, right) => left.path.steps - right.path.steps);
  const selected = adjacentCandidates[0] ?? null;
  return {
    mapFileName: '0',
    mapTitle: String(mapInfo.map_title),
    collision: { sourcePath: relative(map.sourcePath), width: map.width, height: map.height, mapType: map.type },
    startSafeZone: { center: startPoint, size: Number(start.size) },
    npc: { name: String(jane.name), location: npcPoint, npcIndex: Number(jane.npc_index) },
    reachableAdjacentTiles: adjacentCandidates,
    selectedAdjacentTile: selected?.candidate ?? null,
    entryPath: selected?.path ?? null,
    staticPass: selected != null,
  };
}

async function certifyD021Passage(manifest) {
  const woods = manifest.maps.find(map => String(map.map_title) === 'WoomyonWoods(S)');
  const d021 = mapByFileName(manifest, 'D021');
  const d022 = mapByFileName(manifest, 'D022');
  if (!woods) throw new Error('Missing authoritative WoomyonWoods(S) map');
  const fromWoods = movementBetween(woods, d021);
  const toD022 = movementBetween(d021, d022);
  const fromD022 = movementBetween(d022, d021);
  const toWoods = movementBetween(d021, woods);
  const map = await loadProtocolCollisionMap('D021');
  const forwardEntry = point(fromWoods.destination);
  const forwardExit = point(toD022.source);
  const returnEntry = point(fromD022.destination);
  const returnExit = point(toWoods.source);
  return {
    mapFileName: 'D021',
    mapTitle: String(d021.map_title),
    collision: { sourcePath: relative(map.sourcePath), width: map.width, height: map.height, mapType: map.type },
    forward: {
      fromMapFileName: String(woods.map_file_name), entry: forwardEntry,
      toMapFileName: 'D022', exit: forwardExit,
      path: staticPath(map, forwardEntry, forwardExit, [forwardExit], transferObstacles(d021, [forwardExit])),
    },
    return: {
      fromMapFileName: 'D022', entry: returnEntry,
      toMapFileName: String(woods.map_file_name), exit: returnExit,
      path: staticPath(map, returnEntry, returnExit, [returnExit], transferObstacles(d021, [returnExit])),
    },
    staticPass: false,
  };
}

function markdown(report) {
  const status = report.staticPass ? '静态碰撞认证通过（动态未认证）' : '静态碰撞认证失败';
  const lines = [
    '# Newcomer V2 区域静态认证',
    '',
    `生成时间：${report.generatedAt}。状态：**${status}**。`,
    '',
    '本记录只认证本地 Crystal 碰撞包与权威 `MapMovement` / NPC / respawn 清单。它没有连接网关、控制角色或运行真人路线。路径将未选定的当前地图传送格当作障碍，因此不会把穿门误判成目标区域的正常步行。',
    '',
    '所有“目标刷点”证据均为出生矩形内多个碰撞可走锚点，不把 `location` 中心本身当作充分证据。怪物实际出现位置、AOI、占位、刷新时机和当前门状态仍需后续动态验收。',
    '',
    '## 可用区域与静态路径',
    '',
    `- ` + '`0`' + `：起始安全区到 Assistant_Jane 相邻格 ${formatPath(report.village.entryPath)}。`,
    `- ` + '`D001`' + `：${formatRegion(report.combatRegions.D001)}。`,
    `- ` + '`D401`' + `：${formatRegion(report.combatRegions.D401)}。`,
    `- ` + '`D021`' + `：WoomyonWoods(S)→D022 ${formatPath(report.d021.forward.path)}；D022→WoomyonWoods(S) ${formatPath(report.d021.return.path)}。`,
    `- ` + '`D022`' + `：${formatRegion(report.combatRegions.D022)}。`,
    '',
    '## 不能由本认证宣称',
    '',
    `- D001 Skeleton（80，spread 200）、D401 Zombie2/Zombie3（各75，spread 100）及 D022 Dung/WoomaSoldier/WoomaFighter（50/50/30，spread 250）都是高密度宽区域配置。静态路径不代表可安全穿越。`,
    '- D022 的典型交战是否会同时承受超过三只敌人、是否可合理撤退，以及单目标搜寻是否不超过90秒，均为动态生存/搜寻项目，仍是未认证状态。',
    '- 此结果不能证明两小时预算、AI路线、真人理解、门的实时状态或共享 Zone 占位。',
    '',
    '## 输入',
    '',
    ...Object.entries(report.inputs).map(([name, source]) => `- ${name}: \`${source.path}\`，SHA-256 \`${source.sha256}\``),
    '',
    `机器可读完整路径证据：[regions.json](regions.json)。`,
  ];
  return `${lines.join('\n')}\n`;
}

function formatPath(pathRecord) {
  return pathRecord ? `${pathRecord.steps} 步 (${pathRecord.start.x},${pathRecord.start.y})→(${pathRecord.end.x},${pathRecord.end.y})` : '无路径';
}

function formatRegion(region) {
  const details = region.groups.map(group =>
    `${group.monsterName}: 入口 ${formatPath(group.entryPath)}，返程 ${formatPath(group.exitPath)}`,
  );
  return region.staticPass ? details.join('；') : `${details.join('；')}（失败）`;
}

const manifest = await readJson(respawnManifestPath);
const npcManifest = await readJson(npcManifestPath);
const [village, d001, d401, d021, d022] = await Promise.all([
  certifyVillage(manifest, npcManifest),
  certifyCombatRegion({ manifest, mapFileName: 'D001', entryFromMapFileName: '0', targetMonsterNames: TARGETS.D001 }),
  certifyCombatRegion({ manifest, mapFileName: 'D401', entryFromMapFileName: '0', targetMonsterNames: TARGETS.D401 }),
  certifyD021Passage(manifest),
  certifyCombatRegion({ manifest, mapFileName: 'D022', entryFromMapFileName: 'D021', targetMonsterNames: TARGETS.D022 }),
]);
d021.staticPass = Boolean(d021.forward.path && d021.return.path);
const report = {
  schema: 1,
  type: 'newcomer-v2-static-region-certification',
  generatedAt: new Date().toISOString(),
  contract: 'docs/NEWCOMER-1-30-V2-DESIGN.md',
  method: {
    collision: 'loadProtocolCollisionMap + findProtocolWalkPath (eight-direction, corner-safe)',
    transferPolicy: 'all unselected current-map MapMovement source cells are dynamic obstacles',
    spawnAnchorPolicy: 'nine distributed nominal points per respawn, each snapped to a nearby collision-walkable point; shortest reachable anchor recorded',
    dynamicClaimsExcluded: ['monster occupancy', 'AOI', 'respawn timing', 'door state', 'survivability', '90-second search', 'two-hour timing'],
  },
  inputs: {
    respawnManifest: { path: relative(respawnManifestPath), sha256: await digest(respawnManifestPath) },
    npcManifest: { path: relative(npcManifestPath), sha256: await digest(npcManifestPath) },
  },
  village,
  combatRegions: { D001: d001, D401: d401, D022: d022 },
  d021,
  staticPass: village.staticPass && d001.staticPass && d401.staticPass && d021.staticPass && d022.staticPass,
  dynamicCertification: 'not-attempted',
};
await fs.mkdir(outputDirectory, { recursive: true });
await fs.writeFile(outputJsonPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
await fs.writeFile(outputMarkdownPath, markdown(report), 'utf8');
console.log(JSON.stringify({ outputJsonPath: relative(outputJsonPath), outputMarkdownPath: relative(outputMarkdownPath), staticPass: report.staticPass }, null, 2));
if (!report.staticPass) process.exitCode = 1;
