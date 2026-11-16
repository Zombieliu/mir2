import fs from 'node:fs';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

// Offline geometry only. This script never starts a realm or changes repository files.
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const root = 'C:/mir2-build/p01-completion-20261024';
const candidateFile = root + '/partition-ordinary-two-name-span4-halo-100-v1.json';
const prefixFile = root + '/partition-ordinary-two-name-span4-halo-10-v1.json';
const proposalFile = root + '/partition-span4-halo-static-validation-v1.json';
for (const file of [candidateFile, prefixFile, proposalFile]) if (fs.existsSync(file)) throw new Error('Refusing to overwrite ' + file);
process.chdir(project);
const { loadProtocolCollisionMap, findProtocolWalkPath, protocolMapCellIsWalkable } = await import(pathToFileURL(project + '/apps/web/scripts/quest-agent/protocol-navigation.mjs'));
const { loadStationaryNpcObstacles, insideCombatCoverage, validateNativeExplicitHomes, validateNativeCombatClusters } = await import(pathToFileURL(project + '/apps/web/scripts/playtest-capacity-soak.mjs'));
const oldFile = root + '/partition-ordinary-two-name-span4-100-proposal-v1.json';
const oldProposalFile = root + '/partition-span4-static-proposal-v1.json';
const authorBaseFile = root + '/partition-span4-static-proposal-v1.mjs';
const manifestFile = project + '/apps/web/lib/generated/crystal_respawn_manifest.json';
const templateFile = project + '/packages/game-data/data/generated/crystal_monster_manifest.json';
const npcFile = project + '/packages/game-data/data/generated/crystal_npc_info_manifest.json';
const old = JSON.parse(fs.readFileSync(oldFile));
const manifest = JSON.parse(fs.readFileSync(manifestFile));
const templates = JSON.parse(fs.readFileSync(templateFile)).monsters;
const metadata = manifest.maps.find(m => m.map_file_name === '0');
const map = await loadProtocolCollisionMap('0');
const npcs = await loadStationaryNpcObstacles('0');
const origin = { x: 288, y: 616 }, span = 4, size = map.width * map.height;
const key = p => `${p.x},${p.y}`, index = p => p.y * map.width + p.x;
const dist = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const inMap = p => Number.isInteger(p.x) && Number.isInteger(p.y) && p.x >= 0 && p.y >= 0 && p.x < map.width && p.y < map.height;
const steps = [[0,-1],[1,-1],[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1]];
const transferPoints = metadata.movements.map(m => m.source);
const transfers = transferPoints.map(p => ({ bounds: { minX: p.x, maxX: p.x, minY: p.y, maxY: p.y } }));
const npcMask = new Uint8Array(size), transferMask = new Uint8Array(size);
for (const p of npcs) npcMask[index(p)] = 1;
for (const p of transferPoints) transferMask[index(p)] = 1;
const arrivalPoints = manifest.maps.flatMap(m => m.movements.filter(x => x.map_index === metadata.map_index && (x.destination.x !== 0 || x.destination.y !== 0)).map(x => x.destination));
function canonical(rule) {
  if (!rule.count) return [];
  if (rule.count === 1 && !rule.spread) return [{ slot: 0, ...rule.location }];
  if (!rule.spread) return [];
  const cells = [];
  for (let y = rule.location.y - rule.spread; y <= rule.location.y + rule.spread; y++) for (let x = rule.location.x - rule.spread; x <= rule.location.x + rule.spread; x++) {
    const p = { x, y };
    if (protocolMapCellIsWalkable(map, p) && !arrivalPoints.some(q => dist(p, q) <= 8)) cells.push(p);
  }
  const used = new Set(), out = [];
  for (let slot = 0; slot < rule.count && cells.length; slot++) {
    let n = (rule.respawn_index * 97651 + slot * 12347 + 0x9e3779b9) % cells.length;
    for (let attempt = 0; attempt < cells.length; attempt++) {
      const p = cells[n];
      if (!used.has(key(p))) { used.add(key(p)); out.push({ slot, ...p }); break; }
      n = (n + 1) % cells.length;
    }
  }
  return out;
}
const selectedNames = ['Deer', 'Scarecrow'];
const allInitial = metadata.respawns.filter(r => r.count > 0).flatMap(r => canonical(r).map(p => ({ ...p, name: r.monster_name, ai: r.monster_ai, viewRange: r.monster_view_range,
  objectId: 200000 + r.respawn_index * 100 + p.slot, ruleIndex: r.respawn_index, hp: r.monster_hp, delayMinutes: r.delay_minutes, randomDelayMinutes: r.random_delay_minutes })));
const targets = allInitial.filter(p => selectedNames.includes(p.name));
const neutralAi = new Set([1,2,3,6,34,56,57,58,68,81,113]);
const danger = allInitial.filter(m => !neutralAi.has(m.ai) && (templates.find(t => t.name === m.name)?.max_dc ?? Infinity) > 2);
const dangerMask = new Uint8Array(size), initialMask = new Uint8Array(size);
for (const p of allInitial) if (inMap(p)) initialMask[index(p)] = 1;
for (const m of danger) for (let y = Math.max(0, m.y - 6); y <= Math.min(map.height - 1, m.y + 6); y++) for (let x = Math.max(0, m.x - 8); x <= Math.min(map.width - 1, m.x + 8); x++) dangerMask[y * map.width + x] = 1;
const clear = p => inMap(p) && !map.blocked[index(p)] && !npcMask[index(p)] && !transferMask[index(p)];
const used = new Uint8Array(size);
function footprintIssues(home, reserve = false, occupancy = used) {
  const issues = new Set();
  for (let dy = 0; dy <= span; dy++) for (let dx = 0; dx <= span; dx++) {
    const p = { x: home.x + dx, y: home.y + dy }, n = index(p);
    if (!inMap(p) || map.blocked[n]) issues.add('terrain');
    if (npcMask[n]) issues.add('NPC');
    if (transferMask[n]) issues.add('transfer');
    if (occupancy[n]) issues.add('other-footprint');
    if (initialMask[n]) issues.add('canonical-initial-monster');
    if (dangerMask[n]) issues.add('initial-danger-8x6');
    if (reserve && inMap(p)) occupancy[n] = 1;
  }
  for (let dy = -1; dy <= span + 1; dy++) for (let dx = -1; dx <= span + 1; dx++) {
    const p = {x:home.x+dx,y:home.y+dy};
    if (!clear(p)) issues.add('seven-by-seven-halo-terrain-NPC-or-transfer');
  }
  return [...issues];
}
const oldUsed = new Uint8Array(size);
const oldExpansionFailures = old.anchors.flatMap((home, actorIndex) => {
  const reasons = footprintIssues(home, false, oldUsed);
  footprintIssues(home, true, oldUsed);
  return reasons.length ? [{ actorIndex, home, reasons }] : [];
});
const queue = new Int32Array(size);
function component(start, allowed, occupied = []) {
  const blocked = new Set(occupied.map(key)); blocked.delete(key(start));
  const seen = new Uint8Array(size); let head = 0, tail = 0;
  const walk = p => clear(p) && allowed(p) && !blocked.has(key(p));
  if (walk(start)) { seen[index(start)] = 1; queue[tail++] = index(start); }
  while (head < tail) {
    const n = queue[head++], p = { x: n % map.width, y: Math.floor(n / map.width) };
    for (const [dx, dy] of steps) {
      const q = { x: p.x + dx, y: p.y + dy };
      if (!walk(q) || seen[index(q)] || (dx && dy && (!walk({ x: p.x + dx, y: p.y }) || !walk({ x: p.x, y: p.y + dy })))) continue;
      seen[index(q)] = 1; queue[tail++] = index(q);
    }
  }
  return { count: tail, has: p => inMap(p) && Boolean(seen[index(p)]) };
}
const bootstrapCache = new Map();
function bootstrap(home) {
  const k = key(home); if (bootstrapCache.has(k)) return bootstrapCache.get(k);
  const route = findProtocolWalkPath({ map, start: origin, target: home, dynamicObstacles: npcs, maxExpanded: 4000 });
  const proof = { home, found: Boolean(route), pathSteps: route ? route.length - 1 : null,
    noTransfer: Boolean(route) && route.every(p => !transferMask[index(p)]),
    noInitialDangerRectangle: Boolean(route) && route.every(p => !dangerMask[index(p)]),
    initialMonsterCellsOnPath: route ? route.filter(p => initialMask[index(p)]).length : null };
  bootstrapCache.set(k, proof); return proof;
}
const candidates = [];
// Same bounded forest/village region as the v3 author; every integer home is considered.
for (let y = 512; y <= 682; y++) for (let x = 208; x <= 376; x++) {
  const p = { x, y }; if (footprintIssues(p).length) continue;
  const own = { observerHomes: [p], patrolSpan: span };
  const local = targets.filter(t => insideCombatCoverage(own, t));
  candidates.push({ p, local, densityD10: local.reduce((sum, t) => sum + Math.ceil(t.hp / 10) / t.delayMinutes, 0) });
}
// Reserve original lawful footprints so replacing an earlier home cannot displace
// a later already-lawful home. Halo overlap is permitted; 5x5 footprint overlap is not.
const originalLawful = old.anchors.map(home => !footprintIssues(home).length && (() => {const b=bootstrap(home);return b.found&&b.noTransfer&&b.noInitialDangerRectangle;})());
for(let i=0;i<old.anchors.length;i++)if(originalLawful[i])footprintIssues(old.anchors[i],true);
const anchors = [], selections = [], rejected = new Set();
for (let actorIndex = 0; actorIndex < 100; actorIndex++) {
  const fighter = actorIndex % 10 === 5;
  if(originalLawful[actorIndex])for(let dy=0;dy<=span;dy++)for(let dx=0;dx<=span;dx++)used[index({x:old.anchors[actorIndex].x+dx,y:old.anchors[actorIndex].y+dy})]=0;
  const observers = anchors.filter((_, i) => i % 10 !== 5);
  const cluster = { observerHomes: observers, patrolSpan: span };
  const connected = anchors.length ? component(anchors[0], p => insideCombatCoverage(cluster, p), anchors.slice(1)) : null;
  const covered = new Set(targets.filter(t => insideCombatCoverage(cluster, t)).map(t => t.objectId));
  const ranked = candidates.filter(c => !rejected.has(key(c.p)) && !footprintIssues(c.p).length && (!connected || connected.has(c.p))).map(c => {
    const gain = c.local.filter(t => !covered.has(t.objectId)).reduce((sum, t) => sum + Math.ceil(t.hp / 10) / t.delayMinutes, 0);
    const score = -dist(c.p, old.anchors[actorIndex]) * 1000000 + c.densityD10;
    return { ...c, gainD10: gain, score };
  }).sort((a, b) => b.score - a.score || dist(a.p, origin) - dist(b.p, origin) || a.p.y - b.p.y || a.p.x - b.p.x);
  let chosen;
  for (const c of ranked) {
    const proof = bootstrap(c.p);
    if (!proof.found || !proof.noTransfer || !proof.noInitialDangerRectangle) { rejected.add(key(c.p)); continue; }
    chosen = { ...c, bootstrap: proof }; break;
  }
  if (!chosen) break;
  anchors.push(chosen.p); footprintIssues(chosen.p, true);
  selections.push({ actorIndex, role: fighter ? 'combat' : 'movement', home: chosen.p, distanceFromOldHome: dist(chosen.p, old.anchors[actorIndex]), gainD10: chosen.gainD10, bootstrap: chosen.bootstrap });
}
const combatIndices = new Set(anchors.map((_, i) => i).filter(i => i % 10 === 5));
const definitions = [{ combatActorIndices: [...combatIndices], movementActorIndices: anchors.map((_, i) => i).filter(i => !combatIndices.has(i)) }];
const candidate = { schemaVersion: 1, mapFileName: '0', patrolSpan: span, monsterNames: selectedNames, anchors, combatClusters: definitions };
let validatorResult = { passed: false, error: 'Fewer than 100 lawful homes found' };
if (anchors.length === 100) {
  const explicit = validateNativeExplicitHomes(map, origin, 100, anchors, span, transfers, npcs);
  validateNativeCombatClusters(definitions, anchors, combatIndices, span);
  validatorResult = { passed: true, footprintCells: explicit.footprintCells, visitedCells: explicit.visitedCells, mapCells: explicit.mapCells };
}
if(anchors.length===100){
  fs.writeFileSync(candidateFile,JSON.stringify(candidate,null,2)+'\n',{flag:'wx'});
  const prefix={...candidate,anchors:candidate.anchors.slice(0,10),combatClusters:candidate.combatClusters.map(c=>({combatActorIndices:c.combatActorIndices.filter(i=>i<10),movementActorIndices:c.movementActorIndices.filter(i=>i<10)}))};
  fs.writeFileSync(prefixFile,JSON.stringify(prefix,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify({event:'scene-files-created',candidateFile,prefixFile,selected:anchors.length,validatorResult}));
}
const rate = (list, d) => list.reduce((sum, t) => sum + Math.ceil(t.hp / d) / t.delayMinutes, 0);
const stages = [];
for (const count of [2,10,25,50,100]) {
  if (anchors.length < count) continue;
  const active = anchors.slice(0, count), observerIndices = active.map((_, i) => i).filter(i => i % 10 !== 5), fighterIndices = active.map((_, i) => i).filter(i => i % 10 === 5);
  const cluster = { observerHomes: observerIndices.map(i => active[i]), patrolSpan: span };
  const rootConnected = component(active[0], p => insideCombatCoverage(cluster, p), active.slice(1));
  const observerDisconnected = observerIndices.filter(i => i !== 0 && !steps.some(([dx,dy]) => rootConnected.has({ x: active[i].x + dx, y: active[i].y + dy })));
  const fighters = fighterIndices.map(actorIndex => {
    const home = active[actorIndex], occupied = active.filter((_, i) => i !== actorIndex), connected = component(home, p => insideCombatCoverage(cluster, p), occupied);
    const possible = targets.filter(t => insideCombatCoverage(cluster, t) && steps.some(([dx,dy]) => connected.has({ x: t.x + dx, y: t.y + dy })));
    const paths = [];
    for (const target of possible) {
      const goals = steps.map(([dx,dy]) => ({ x: target.x + dx, y: target.y + dy })).filter(p => connected.has(p)).sort((a,b) => dist(a, home) - dist(b, home));
      for (const goal of goals) {
        const route = findProtocolWalkPath({ map, start: home, target: goal, dynamicObstacles: [...npcs, ...occupied], maxExpanded: 4000 });
        if (route && route.every(p => !transferMask[index(p)] && insideCombatCoverage(cluster, p))) { paths.push({ objectId: target.objectId, goal, steps: route.length - 1 }); break; }
      }
    }
    return { actorIndex, home, homeCoveredByActiveObserver: insideCombatCoverage(cluster, home), connectedCells: connected.count,
      isolatedObserverIndices: observerIndices.filter(i => !steps.some(([dx,dy]) => connected.has({ x: active[i].x + dx, y: active[i].y + dy }))),
      connectedCanonicalTargets: possible.length, exact4000TargetPaths: paths.length, targetPaths: paths };
  });
  const ids = new Set(fighters.flatMap(f => f.targetPaths.map(t => t.objectId))), unique = targets.filter(t => ids.has(t.objectId));
  stages.push({ count, fighterIndices, activeObserverIndices: observerIndices, rootCoverageConnectedCells: rootConnected.count, observerDisconnected,
    fighters, uniqueExact4000CanonicalTargets: unique.length, targetCountsByName: Object.fromEntries(selectedNames.map(name => [name, unique.filter(t => t.name === name).length])),
    conditionalPositiveHitCapacityByFixedDamage: Object.fromEntries([3,5,10,25].map(d => [d, rate(unique, d)])),
    requirementsPerMinute: { attacks: 18 * fighterIndices.length, positiveHits: 12 * fighterIndices.length },
    targetSupplyClaim: 'Conditional optimistic fixed-damage positive-hit estimate only; it proves neither attempted attacks nor live supply, scheduling, cooldown, 60s windows or two-hour survival.' });
}
function pin(file) { const bytes = fs.readFileSync(file); return { path: file, bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') }; }
const proposal = { schemaVersion: 1, createdAt: new Date().toISOString(), scope: 'Offline span4 with full7x7 static halo scene candidate, pending root review; not adopted or run', staticOnly: true, accepted: false, capacityAccepted: false,
  sourceCommit: execFileSync('git', ['rev-parse','HEAD'], { cwd: project, encoding:'utf8', windowsHide:true }).trim(),
  invariants: { mapFileName:'0', mapContentProfile:'platinum_176', monsterNames:selectedNames, span:4, footprint:'5x5', staticHalo:'7x7 terrain/NPC/transfer clear', ordinaryProtocolOnly:true, noTeleport:true, excludedStationaryNpcs:true, excludedTransfers:true,
    maxExpanded:4000, nativeOpportunityMs:750, minimumRunFraction:.5, attacksPerFighterMinute:18, positiveHitsPerFighterMinute:12, existingGateUnchanged:true },
  sourcePins: [oldFile,oldProposalFile,authorBaseFile,manifestFile,templateFile,npcFile,map.sourcePath,project+'/apps/web/scripts/playtest-capacity-soak.mjs',project+'/apps/web/scripts/quest-agent/protocol-navigation.mjs',import.meta.filename].map(pin),
  map: { width:map.width,height:map.height,cells:size,stationaryNpcCount:npcs.length,transfers:transferPoints.length,canonicalInitialMonsters:allInitial.length,canonicalTargetSlots:targets.length,initialDangerousEntities:danger.length },
  priorSpan4HomesFailingHaloOrExistingFootprintExclusions: { failures:oldExpansionFailures.length, details:oldExpansionFailures }, search: { bounds:{minX:208,maxX:376,minY:512,maxY:682},initialLegalCandidates:candidates.length,rejectedBootstrapCandidates:rejected.size,selected:anchors.length,
    selection:'All actors choose closest lawful prior span4 home, density only ties; pre-reserve lawful future footprints. Current prefix observer coverage and original bootstrap constraints still required.', originalLawfulPreReserved:originalLawful.filter(Boolean).length },
  validatorResult,candidateFile:anchors.length===100?candidateFile:null,prefixFile:anchors.length===100?prefixFile:null,selections,stages,
  homeChanges:{unchanged:selections.filter(s=>!s.distanceFromOldHome).length,changed:selections.filter(s=>s.distanceFromOldHome).length,maxChebyshev:Math.max(...selections.map(s=>s.distanceFromOldHome)),sumChebyshev:selections.reduce((n,s)=>n+s.distanceFromOldHome,0),details:selections.filter(s=>s.distanceFromOldHome).map(s=>({actorIndex:s.actorIndex,role:s.role,oldHome:old.anchors[s.actorIndex],newHome:s.home,distance:s.distanceFromOldHome}))},
  densityComparison:[2,10,25,50,100].map(count=>{const measure=hs=>{let homePairsWithin16=0;for(let a=0;a<hs.length;a++)for(let b=a+1;b<hs.length;b++)if(dist(hs[a],hs[b])<=16)homePairsWithin16++;return{homePairsWithin16,meanPairChebyshev:hs.length>1?hs.reduce((sum,a,i)=>sum+hs.slice(i+1).reduce((n,b)=>n+dist(a,b),0),0)/(hs.length*(hs.length-1)/2):0};};return{count,priorSpan4:measure(old.anchors.slice(0,count)),halo:measure(anchors.slice(0,count)),scope:'Static home centre proximity only, not live AOI or visible entity count.'};}),
  limitations: [
    'No game service, client login, movement, combat, recordings, throughput, gate, profile or source code was changed or tested.',
    'Every selected home has a clear7x7 static terrain/NPC/transfer halo; initial-monster and danger exclusions apply to the5x5 body as before. Halo can contain canonical monsters. More than two moving blockers, hints, stale AOI and future wander are not removed by this static condition.',
    'Two ordinary single-cell blockers cannot eliminate all four cardinal first steps from an original5x5 corner when all four7x7 halo cardinal neighbors remain otherwise free. This is not a guarantee of95percent activity, live route availability or combat supply.',
    'This is a different scene and cannot rewrite08 or inherit any prior live scene acceptance; adoption needs root review and a fresh strict probe.',
    'Source patrol can run continuously on an empty span2 square as an existing fixture demonstrates; 03 Run ratio failure is not proven to be caused by span2 geometry.',
    '5x5 instead of3x3 reserves2500 rather than900 footprint cells; span4 observer guaranteed coverage is25x25 rather than27x27 before terrain and union.',
    'Coverage calculations include only prefix-active observer homes. Runtime online/living/ready state and actual witness packets remain required.',
    'Canonical initial spawn positions and8x6 danger rectangles are mirrors of initial static geometry, not a live monster census, future wander bound, non-target aggro proof or natural survival guarantee.',
    'Bootstrap4000 A* excludes NPCs and transfers, but live monsters/players can change occupancy; initial monster crossings are reported and not hidden.',
    'Representative active home occupancy is used for fighter paths. All moving actors and target motion are not simulated; shared target identities are deduplicated.',
    'Fixed-damage positive-hit rates are conditional optimistic supply estimates; they cannot establish18attempts/min,12positivehits/min per actor, timing windows, actual target allocation, damage growth or two-hour acceptance.'
  ] };
fs.writeFileSync(proposalFile, JSON.stringify(proposal,null,2)+'\n', {flag:'wx'});
console.log(JSON.stringify({proposalFile,candidateFile:proposal.candidateFile,map:proposal.map,oldExpansionFailures:oldExpansionFailures.length,search:proposal.search,validatorResult,
  stages:stages.map(s=>({count:s.count,fighters:s.fighterIndices.length,observers:s.activeObserverIndices.length,disconnectedObservers:s.observerDisconnected,
    fighterIsolatedObservers:s.fighters.map(f=>f.isolatedObserverIndices.length),targets:s.uniqueExact4000CanonicalTargets,rates:s.conditionalPositiveHitCapacityByFixedDamage})),
  pins:[proposalFile,...(proposal.candidateFile?[candidateFile,prefixFile]:[])].map(pin)},null,2));
