import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
const root = 'C:/mir2-build/p01-completion-20261024';
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const file = name => path.join(root, name);
const load = name => JSON.parse(fs.readFileSync(file(name), 'utf8'));
const pin = name => { const bytes = fs.readFileSync(name); return { file: name, bytes: bytes.length, sha256: crypto.createHash('sha256').update(bytes).digest('hex') }; };
const scenarioFile = file('formal-native-v2-ladder-04/scenario.json'), candidateFile = file('partition-ordinary-two-name-safe-home-100-candidate-v3.json');
const scenario = JSON.parse(fs.readFileSync(scenarioFile)), candidate = JSON.parse(fs.readFileSync(candidateFile));
if (!fs.readFileSync(scenarioFile).equals(fs.readFileSync(candidateFile))) throw new Error('Current formal04 scenario differs from the original two-name100 candidate');
const policy = load('formal-native-v2-ladder-04/run-options.json').policy;
const old = load('partition-stage-two-name-safe-home-supply-plan-v3.json'), pack = load('partition-stage-two-name-package-v3.json');
const manifestFile = path.join(project, 'apps/web/lib/generated/crystal_respawn_manifest.json');
const map = JSON.parse(fs.readFileSync(manifestFile)).maps.find(m => m.map_file_name === scenario.mapFileName);
const targetRules = map.respawns.filter(r => scenario.monsterNames.includes(r.monster_name));
const conditional = (targets, damage) => targets.reduce((sum, t) => sum + Math.ceil(t.hp / damage) / t.delayMinutes, 0);
const prefixes = policy.stages.map(count => {
  const combat = scenario.combatClusters.flatMap(c => c.combatActorIndices).filter(i => i < count);
  const movement = scenario.combatClusters.flatMap(c => c.movementActorIndices).filter(i => i < count);
  const previous = old.stages.find(s => s.count === count), packaged = pack.stages.find(s => s.count === count)?.models.find(m => m.model === 'twoName');
  if (JSON.stringify(combat) !== JSON.stringify(previous.fighterIndices) || movement.length !== previous.activeObserverCount) throw new Error('Prefix roles differ from prior static analysis');
  if (packaged) for (const t of packaged.targets) {
    const native = targetRules.find(r => r.respawn_index === t.ruleIndex);
    if (!native || native.monster_name !== t.name || native.monster_hp !== t.hp || native.delay_minutes !== t.delayMinutes || native.random_delay_minutes !== 0) throw new Error('Target HP/respawn changed');
  }
  const rates = packaged ? Object.fromEntries([3,5,10,25].map(d => [d, conditional(packaged.targets, d)])) : previous.conditionalPositiveHitCapacityByFixedDamage;
  return { count, combatIndices: combat, combatActors: combat.length, movementObservers: movement.length,
    combatFraction: combat.length / count, requiredLaunchedAttacksPerMinute: combat.length * policy.minimumCombatAttacksPerMinute,
    requiredDualEndPositiveHitsPerMinute: combat.length * policy.minimumCombatHitsPerMinute,
    canonicalReachableTargets: previous.uniqueTargetCount, canonicalTargetNames: packaged?.namesCount ?? null,
    conditionalIdealPositiveHitRatesByFixedDamage: rates,
    conditionalRateDefinition: 'Sum ceil(nativeHP/fixedPositiveDamage)/nativeRespawnMinutes over distinct static canonical reachable IDs; ignores kill duration, AI/travel, contention, real accuracy, quarantine and per-window timing. This is not an unconditional attempt ceiling or an acceptance result.',
    conditionalAllocation: previous.conditionalAllocatedPositiveHits.filter(v => v.perFighterDemand === 12),
    staticFighterSummaries: previous.fighters,
    navigationRecomputedThisReview: false };
});
const refs = [
  ['apps/web/scripts/playtest-capacity-soak.mjs', [541,804,814,1018,1247,1252,1254,1367,1699]],
  ['apps/web/scripts/playtest-capacity-soak-core.mjs', [53,80,82,166,175,185,220,332]],
  ['apps/simulation/src/runtime/monsters.rs', [315,389,404,412,975,1664]],
  ['apps/simulation/src/runtime/zone/runtime.rs', [2727,2762,2776,14335,14340,14354,16651,16678,16748,16771]],
  ['apps/simulation/src/runtime/zone/types.rs', [372,390,1382]],
  ['apps/simulation/src/runtime/stats.rs', [183,208,225,232,233,235]],
  ['apps/simulation/src/runtime/combat.rs', [736]]
].map(([name, lines]) => ({ ...pin(path.join(project,name)), lines }));
const result = { schemaVersion: 1, reviewedAt: new Date().toISOString(),
  repositoryCommit: execFileSync('git', ['rev-parse','HEAD'], { cwd: project, encoding: 'utf8' }).trim(),
  scope: 'Bounded read-only review of the original formal04 two-name map0/span2 scenario. JSON parsing, source reading and small arithmetic only; no game/test imports, pathfinding, realm, build, load, recording or private-account reads.',
  conclusion: { unconditionalStaticFailureProven: false, staticAcceptanceProven: false,
    decision: 'No unconditional supply contradiction was established for the unchanged formal04 profile. A genuine qualifying07 gate can authorize fresh formal04 as originally frozen; every prefix and the two-hour workload still require the original runtime gates.',
    conditionalWarning: 'At100, the same154 static targets serve10 rather than5 fighters. The fixed-positive-damage10 ideal stock model gives100.2 hits/min against120 required; that is a conditional shortage, not proof that all real successful attacks stay at least10 damage or that live supply equals the initial canonical set.' },
  scenario: { ...pin(scenarioFile), equalOriginalCandidateBytes: true, originalCandidate: pin(candidateFile),
    mapFileName: scenario.mapFileName, patrolSpan: scenario.patrolSpan, monsterNames: scenario.monsterNames,
    anchors: scenario.anchors.length, combatClusters: scenario.combatClusters.length,
    activeCoverageRule: 'Current actual-ready, in-game, living, same-map movement clients activate their declared home footprints. Future, paused, closed and absent clients cannot lend observer coverage; actual attacks additionally require real nearby observers. This review does not substitute static homes for online witness evidence.' },
  requirements: { stageSeconds: policy.stageSeconds, soakSeconds: policy.soakSeconds, movementIntervalMs: policy.movementIntervalMs,
    attacksPerFighterMinute: policy.minimumCombatAttacksPerMinute, positiveHitsPerFighterMinute: policy.minimumCombatHitsPerMinute,
    minimumNativeRunRatio: policy.minimumNativeRunRatio, timeoutExclusive: policy.timeoutExclusive, maximumCorrectionRatio: policy.maximumCorrectionRatio },
  prefixes,
  nativeContent: { manifest: pin(manifestFile), allMapSelectedRules: targetRules.length,
    allMapSelectedStockSlots: targetRules.reduce((n,r) => n+r.count,0),
    rules: targetRules.map(r => ({ respawnIndex:r.respawn_index, name:r.monster_name, count:r.count, hp:r.monster_hp,
      ai:r.monster_ai, agility:r.monster_agility, respawnMinutes:r.delay_minutes, randomRespawnMinutes:r.random_delay_minutes,
      center:r.location, spread:r.spread })),
    hp: { Deer:25, Scarecrow:20 },
    respawn: 'Selected native rules have zero jitter and3–6 minute delays from death. Deer AI2 requires harvested or removed corpse before due respawn can proceed; ordinary unharvested corpse expires after180000ms, so it is not a permanent blocked slot. Respawn still requires its own due time; harvest does not shorten it.',
    damage: 'All ordinary capacity roles are Warrior/Male. Authoritative physical melee first rolls accuracy against native agility, then real min/max DC/luck and monster armour. Warrior class base DC rises by trunc(level/5), before equipment and other stats. Neither fixed3 nor fixed10 damage is established for every real future swing.',
    attacksVersusHits: 'V2 activity counts correlated launched attacks; notLaunched does not count as launched attack or hit. A positive hit still requires original dual-end damage/shared-health evidence.18 launched attacks/min is not identical to18 positive hits/min;12 hits/min is a separate required minimum.' },
  prospectiveRisks: [
    'The100 prefix adds no canonical reachable targets beyond50 while doubling fighter demand. Real higher damage can kill stock faster; low damage can improve hit stock but consume more time and expose movement/cooldown costs.',
    'Static mean replenishment never proves each fighter in each60s window. Native AI wandering, chase/coverage geometry, target-claim contention, corpse/death phases and unequal nearest-target scheduling can cause a local empty window despite sufficient aggregate stock.',
    'Every unsuccessful/unverified native attack exception permanently quarantines that actor/target objectId for the run;65 quarantined IDs stops it. Respawn of the same ID does not wash quarantine. Real agility misses and late/unknown evidence can progressively reduce usable per-actor stock; a finite2h must-fail bound has not been proven from unknown live swing times.',
    'Damage and effective supply are not frozen at initial starter level: earlier fighters continue ordinary combat while later accounts await normal admission windows, and during two-hour soak. Never extrapolate the initial candidate damage or six-minute result across the whole ladder.',
    'Original span2 safe homes and existing4000 navigation proof concern geometry, not live occupancy or Run ratio. The native primed fallback preference preserves true obstacles and may still return Walk when no legal Run exists;50%Run and other activity gates may correctly fail.',
    'No alternate span4 scene, optional Hen target, resource-threshold change or failed-run reclassification was used.'
  ],
  monitorWithoutNewInstrumentation: [
    'Original per-actor continuous60s launched attacks, positive hits, native completion, movement Run ratio, cells/min and correction/timeout/fanout gates.',
    'Existing V2 action outcomes, actual positive damage/health evidence and available owner level/stat-derived timing; do not infer a successful attack from owner position alone.',
    'Existing ambiguousTargetQuarantined records and repeated target IDs, scout/chase/fallback activity and failures; count isolation by actor and keep original failure verdicts.',
    'Existing public zone histogram phase deltas, slow-stage receive-time records, driver lag and resource monitors. None of these static supply calculations establish server throughput or a physical-host maximum.'
  ],
  inputPins: [pin(file('partition-stage-two-name-safe-home-supply-plan-v3.json')),pin(file('partition-stage-two-name-package-v3.json')),
    pin(file('formal-native-v2-ladder-04/run-options.json')),pin(file('capacity-patrol-run-preference-driver-frozen-v1.json'))], sourcePins: refs,
  oldGeometryScope: 'Previous canonical reachable-target analysis is retained by raw SHA, and HP/respawn fields were cross-checked against current native manifest. No navigation, live creature census, build or performance measurement was rerun. Earlier historical warning about future homes predates the current actual-active coverage fix and does not describe current code.',
  acceptance: { capacityAccepted:false, profileOrThresholdChanged:false, sourceChanged:false, serviceOrTestStarted:false } };
const out=file('formal04-supply-static-review-v1.json');fs.writeFileSync(out,JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({report:pin(out),sourceFiles:refs.length,prefixes:prefixes.map(p=>({count:p.count,fighters:p.combatActors,observers:p.movementObservers,targets:p.canonicalReachableTargets,hitsRequired:p.requiredDualEndPositiveHitsPerMinute,rates:p.conditionalIdealPositiveHitRatesByFixedDamage})),conclusion:result.conclusion}));
