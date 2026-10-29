import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';

export const PROJECT = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
export const BASE = 'C:/mir2-build/p01-completion-20261024';
export const ROOT = import.meta.dirname;
export const PARAMETERS = Object.freeze({ schemaVersion: 1, mapFileName: '0', patrolSpan: 2, bodySide: 3, haloSide: 5,
  bounds: { minX: 208, maxX: 376, minY: 512, maxY: 682 }, origin: { x: 288, y: 616 },
  monsterNames: ['Deer', 'Scarecrow'], actors: 100, prefixes: [2, 10, 25, 50, 100], fighterIndices: Array.from({ length: 10 }, (_, i) => i * 10 + 5),
  maxExpanded: 4000, stableWitnessRange: 6, bodyExcludesCanonicalInitialMonsters: true, initialDangerHalfWidth: 8, initialDangerHalfHeight: 6,
  ordinaryProtocolOnly: true, freshWarrior: true, grants: false, newSpawnRules: false, productChanges: false,
  attacksPerFighterMinute: 18, positiveHitsPerFighterMinute: 12, minimumNativeCompletion: .95,
  minimumRunFraction: .5, minimumNativeCellsPerMinute: 80, opportunityMs: 750, commandsPerSecond: 3,
  actualRuntimeLimitsUnchanged: { pathTargets: 4, adjacentGoals: 8, searchNodesPerPath: 4000, attackDeadlineMs: 900, chaseMoves: 40, chaseMs: 30000, visits: 3 },
  accepted: false, capacityAccepted: false, staticOnly: true });
export const STEPS = [[0, -1], [1, -1], [1, 0], [1, 1], [0, 1], [-1, 1], [-1, 0], [-1, -1]];
export const key = p => `${p.x},${p.y}`;
export const dist = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
export const pin = file => { const bytes = fs.readFileSync(file); return { path: path.resolve(file).replaceAll('\\', '/'), bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') }; };
export const writeNew = (file, value) => { if (path.dirname(path.resolve(file)) !== path.resolve(ROOT)) throw new Error('Output must be an immediate child of the dedicated external directory'); fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' }); return pin(file); };

export async function loadGeometry() {
  const navFile = PROJECT + '/apps/web/scripts/quest-agent/protocol-navigation.mjs';
  const driverFile = PROJECT + '/apps/web/scripts/playtest-capacity-soak.mjs';
  const nav = await import(pathToFileURL(navFile));
  const driver = await import(pathToFileURL(driverFile));
  const manifestFile = PROJECT + '/apps/web/lib/generated/crystal_respawn_manifest.json';
  const templateFile = PROJECT + '/packages/game-data/data/generated/crystal_monster_manifest.json';
  const manifest = JSON.parse(fs.readFileSync(manifestFile));
  const templates = JSON.parse(fs.readFileSync(templateFile)).monsters;
  const metadata = manifest.maps.find(m => m.map_file_name === '0');
  const map = await nav.loadProtocolCollisionMap('0');
  const npcs = await driver.loadStationaryNpcObstacles('0');
  const size = map.width * map.height, index = p => p.y * map.width + p.x;
  const inMap = p => Number.isInteger(p?.x) && Number.isInteger(p?.y) && p.x >= 0 && p.y >= 0 && p.x < map.width && p.y < map.height;
  const npcMask = new Uint8Array(size), transferMask = new Uint8Array(size), initialMask = new Uint8Array(size), dangerMask = new Uint8Array(size);
  for (const p of npcs) if (inMap(p)) npcMask[index(p)] = 1;
  for (const p of metadata.movements.map(m => m.source)) if (inMap(p)) transferMask[index(p)] = 1;
  const transfers = metadata.movements.map(m => ({ bounds: { minX: m.source.x, maxX: m.source.x, minY: m.source.y, maxY: m.source.y } }));
  const arrivals = manifest.maps.flatMap(m => m.movements.filter(x => x.map_index === metadata.map_index && (x.destination.x !== 0 || x.destination.y !== 0)).map(x => x.destination));
  // Mirrors monsters.rs crystal_world_respawn_spawns / deterministic_roll tick=0.
  // This is initial canonical placement, not a future/live occupancy claim.
  function canonical(rule) {
    if (!rule.count) return [];
    if (rule.count === 1 && !rule.spread) return [{ slot: 0, ...rule.location }];
    if (!rule.spread) return [];
    const cells = [];
    for (let y = Math.max(0, rule.location.y - rule.spread); y <= Math.min(map.height - 1, rule.location.y + rule.spread); y++) {
      for (let x = Math.max(0, rule.location.x - rule.spread); x <= Math.min(map.width - 1, rule.location.x + rule.spread); x++) {
        const p = { x, y }; if (!map.blocked[index(p)] && !arrivals.some(q => dist(p, q) <= 8)) cells.push(p);
      }
    }
    const used = new Set(), out = [];
    for (let slot = 0; slot < rule.count && cells.length; slot++) {
      let n = (rule.respawn_index * 97651 + slot * 12347 + 0x9e3779b9) % cells.length;
      for (let attempt = 0; attempt < cells.length; attempt++, n = (n + 1) % cells.length) {
        const p = cells[n]; if (!used.has(key(p))) { used.add(key(p)); out.push({ slot, ...p }); break; }
      }
    }
    return out;
  }
  const allInitial = metadata.respawns.flatMap(r => canonical(r).map(p => ({ ...p, name: r.monster_name, ai: r.monster_ai,
    objectId: 200000 + (r.count === 1 && r.spread === 0 ? r.respawn_index : r.respawn_index * 100 + p.slot),
    ruleIndex: r.respawn_index, hp: r.monster_hp, delayMinutes: r.delay_minutes, randomDelayMinutes: r.random_delay_minutes,
    spawnCenter: r.location, spawnSpread: r.spread })));
  const targets = allInitial.filter(p => PARAMETERS.monsterNames.includes(p.name));
  const neutralAi = new Set([1, 2, 3, 6, 34, 56, 57, 58, 68, 81, 113]);
  const danger = allInitial.filter(m => !neutralAi.has(m.ai) && (templates.find(t => t.name === m.name)?.max_dc ?? Infinity) > 2);
  for (const p of allInitial) if (inMap(p)) initialMask[index(p)] = 1;
  for (const m of danger) for (let y = Math.max(0, m.y - 6); y <= Math.min(map.height - 1, m.y + 6); y++) {
    for (let x = Math.max(0, m.x - 8); x <= Math.min(map.width - 1, m.x + 8); x++) dangerMask[y * map.width + x] = 1;
  }
  const clear = p => inMap(p) && !map.blocked[index(p)] && !npcMask[index(p)] && !transferMask[index(p)];
  const body = home => Array.from({ length: 9 }, (_, i) => ({ x: home.x + i % 3, y: home.y + Math.floor(i / 3) }));
  const halo = home => Array.from({ length: 25 }, (_, i) => ({ x: home.x - 1 + i % 5, y: home.y - 1 + Math.floor(i / 5) }));
  function issues(home, used = null) {
    const reasons = new Set();
    const b = PARAMETERS.bounds;
    if (!inMap(home) || home.x < b.minX || home.x > b.maxX || home.y < b.minY || home.y > b.maxY) reasons.add('home-outside-original-bounds');
    for (const p of halo(home)) if (!clear(p)) reasons.add('halo-static-terrain-NPC-or-transfer');
    for (const p of body(home)) {
      if (initialMask[index(p)]) reasons.add('body-canonical-initial-monster');
      if (dangerMask[index(p)]) reasons.add('body-initial-danger-8x6');
      if (used?.has(key(p))) reasons.add('body-overlap');
    }
    return [...reasons];
  }
  const bootstrapCache = new Map();
  function bootstrap(home) {
    const k = key(home); if (bootstrapCache.has(k)) return bootstrapCache.get(k);
    const route = nav.findProtocolWalkPath({ map, start: PARAMETERS.origin, target: home, dynamicObstacles: npcs, maxExpanded: 4000 });
    const proof = { found: Boolean(route), steps: route ? route.length - 1 : null, noTransfer: Boolean(route) && route.every(p => !transferMask[index(p)]),
      noInitialDanger: Boolean(route) && route.every(p => !dangerMask[index(p)]), initialMonsterCrossings: route ? route.filter(p => initialMask[index(p)]).length : null };
    bootstrapCache.set(k, proof); return proof;
  }
  const staticGoals = new Map(targets.map(t => [t.objectId, STEPS.map(([dx, dy]) => ({ x: t.x + dx, y: t.y + dy })).filter(clear)]));
  function witnessGoals(home, target, mode = 'all') {
    // All requires the SAME observer and SAME attack tile to cover all nine
    // legal peer body positions. Some is explicitly weaker potential geometry.
    const positions = body(home), range = PARAMETERS.stableWitnessRange;
    return staticGoals.get(target.objectId).filter(goal => mode === 'all'
      ? positions.every(p => dist(p, target) <= range && dist(p, goal) <= range)
      : positions.some(p => dist(p, target) <= range && dist(p, goal) <= range));
  }
  function localTargets(home, mode = 'all') { return targets.filter(t => witnessGoals(home, t, mode).length); }
  function cluster(homes, count = homes.length) { return { observerHomes: homes.slice(0, count).filter((_, i) => i % 10 !== 5), patrolSpan: 2 }; }
  function component(start, allowed = () => true, occupied = []) {
    const blocked = new Set(occupied.map(key)); blocked.delete(key(start));
    const seen = new Uint8Array(size), queue = new Int32Array(size); let head = 0, tail = 0;
    const walk = p => clear(p) && allowed(p) && !blocked.has(key(p));
    if (walk(start)) { seen[index(start)] = 1; queue[tail++] = index(start); }
    while (head < tail) {
      const n = queue[head++], p = { x: n % map.width, y: Math.floor(n / map.width) };
      for (const [dx, dy] of STEPS) {
        const q = { x: p.x + dx, y: p.y + dy };
        if (!walk(q) || seen[index(q)] || (dx && dy && (!walk({ x: p.x + dx, y: p.y }) || !walk({ x: p.x, y: p.y + dy })))) continue;
        seen[index(q)] = 1; queue[tail++] = index(q);
      }
    }
    return { count: tail, has: p => inMap(p) && Boolean(seen[index(p)]) };
  }
  let pathSearches = 0;
  function route(start, goal, homes, actorIndex = -1, allowed = () => true) {
    pathSearches++;
    const obstacles = [...npcs, ...homes.filter((p, i) => i !== actorIndex && key(p) !== key(start) && key(p) !== key(goal))];
    const points = nav.findProtocolWalkPath({ map, start, target: goal, dynamicObstacles: obstacles, maxExpanded: 4000 });
    return points && points.every(p => !transferMask[index(p)] && allowed(p)) ? points : null;
  }
  const sourcePins = [navFile, driverFile, manifestFile, templateFile, PROJECT + '/packages/game-data/data/generated/crystal_npc_info_manifest.json', map.sourcePath,
    PROJECT + '/apps/simulation/src/runtime/monsters.rs', PROJECT + '/apps/simulation/src/runtime/session.rs', import.meta.filename].map(pin);
  return { nav, driver, map, npcs, metadata, templates, targets, allInitial, danger, sourcePins, size, index, inMap, clear, initialMask, dangerMask,
    npcMask, transferMask, transfers, body, halo, issues, bootstrap, staticGoals, witnessGoals, localTargets, cluster, component, route,
    get pathSearches() { return pathSearches; } };
}

export function fixedDamageSupply(targets, damages = [1, 3, 5, 10, 15, 25]) {
  return Object.fromEntries(damages.map(damage => [damage, targets.reduce((sum, t) => sum + Math.ceil(t.hp / damage) / Math.max(3, t.delayMinutes), 0)]));
}

export function geometrySelfChecks() {
  const all = (h, target, goal) => Array.from({ length: 9 }, (_, i) => ({ x: h.x + i % 3, y: h.y + Math.floor(i / 3) }))
    .every(p => dist(p, target) <= 6 && dist(p, goal) <= 6);
  if (!all({ x: 0, y: 0 }, { x: 6, y: 0 }, { x: 5, y: 0 }) || all({ x: 0, y: 0 }, { x: 7, y: 0 }, { x: 6, y: 0 })) throw new Error('All-nine-position witness boundary failed');
  if (all({ x: 0, y: 0 }, { x: 6, y: 0 }, { x: 7, y: 0 })) throw new Error('Prospective attack tile range was not checked');
  return { passed: true, boundaryChecks: 3, scope: 'Pure integer geometry self-checks; no runtime or acceptance claim' };
}
