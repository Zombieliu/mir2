import fs from 'node:fs';
import { execFileSync } from 'node:child_process';
import { ROOT, BASE, PROJECT, PARAMETERS, pin, writeNew, key, dist } from './static-lib.mjs';
const read=name=>JSON.parse(fs.readFileSync(ROOT+'/'+name));
const expected=JSON.parse(fs.readFileSync(BASE+'/capacity-native-witness-approach-driver-frozen-v1.json'));
for(const f of expected.files){const actual=pin(PROJECT+'/'+f.file);if(actual.bytes!==f.bytes||actual.sha256!==f.sha256)throw new Error('Frozen9source differs: '+f.file);}
const head=execFileSync('git',['rev-parse','HEAD'],{cwd:PROJECT,encoding:'utf8',windowsHide:true}).trim();
const tracked=execFileSync('git',['status','--porcelain','--untracked-files=no'],{cwd:PROJECT,encoding:'utf8',windowsHide:true});
if(head!=='6d70d02d0bc390156b77a3e48acee714fc87cda3'||tracked.trim())throw new Error('Expected immutable driverHEAD/trackedclean');
const original=read('candidate-100-compact-v2.json'),tenBytes=fs.readFileSync(ROOT+'/candidate-10-compact-v2.json'),nearest=read('candidate-100-nearest-ordered-final-v3.json');
const coordinateSetsEqual=JSON.stringify(original.anchors.map(key).sort())===JSON.stringify(nearest.anchors.map(key).sort());
if(!coordinateSetsEqual||!tenBytes.equals(fs.readFileSync(ROOT+'/candidate-10-nearest-ordered-final-v3.json')))throw new Error('Suffix permutation identity differs');
const before=read('validation-compact-v2.json'),after=read('validation-compact-v2-final-v2.json');
for(const item of[before,after]){delete item.createdAt;delete item.parameters;delete item.sourcePins;}
if(JSON.stringify(before)!==JSON.stringify(after))throw new Error('External80field correction changed geometric result');
const oldLib=fs.readFileSync(ROOT+'/static-lib-preimage-v1.mjs.txt','utf8'),newLib=fs.readFileSync(ROOT+'/static-lib.mjs','utf8');
if(oldLib.replace('minimumDistinctCells: 80','minimumNativeCellsPerMinute: 80')!==newLib)throw new Error('Metadata correction changed more than one field name');
const oldModel=read('conditional-level-supply-v1.json'),newModel=read('conditional-level-supply-final-v2.json');
if(JSON.stringify(oldModel.stages)!==JSON.stringify(newModel.stages)||JSON.stringify(oldModel.closedPoolProgression)!==JSON.stringify(newModel.closedPoolProgression))throw new Error('Metadata correction changed numeric model');
const validation=read('validation-compact-v2-final-v2.json'),reordered=read('validation-nearest-ordered-final-v3.json'),model=read('conditional-level-supply-final-v2.json'),orderedModel=read('conditional-level-supply-nearest-final-v3.json');
const scout=read('prefix-scout-cost-compact-final-v2.json'),orderedScout=read('prefix-scout-cost-nearest-final-v3.json'),comparison=read('scene-comparison-v1.json');
const densities=PARAMETERS.prefixes.map(count=>{
  const homes=original.anchors.slice(0,count);let within16=0,sum=0,pairs=0;
  for(let i=0;i<homes.length;i++)for(let j=i+1;j<homes.length;j++){const d=dist(homes[i],homes[j]);pairs++;sum+=d;if(d<=16)within16++;}
  return{count,homePairsWithin16:within16,meanHomePairChebyshev:pairs?sum/pairs:null,scope:'Home-point geometry only, not AOI membership/current density/peak or a live monster census'};
});
const stageTable=v=>v.stages.map(s=>({count:s.count,fighterIndices:s.fighterIndices,stableTargets:s.uniqueAllNineStableExact4000Targets,
  counts:s.uniqueCounts,minimumIndividualFighterTargetPaths:s.perFighter.length?Math.min(...s.perFighter.map(f=>f.allNineStableExact4000TargetPaths)):0,
  fixedDamageSensitivity:s.conditionalPositiveHitsPerMinuteByFixedDamage,requiredPerMinute:s.requiredPerMinute}));
const keyPins=['candidate-100-compact-v2.json','candidate-10-compact-v2.json','validation-compact-v2-final-v2.json','conditional-level-supply-final-v2.json',
  'prefix-scout-cost-compact-final-v2.json','candidate-100-nearest-ordered-final-v3.json','candidate-10-nearest-ordered-final-v3.json',
  'validation-nearest-ordered-final-v3.json','conditional-level-supply-nearest-final-v3.json','prefix-scout-cost-nearest-final-v3.json',
  'candidate-100-continuous-ordered-final-v3.json','candidate-10-continuous-ordered-final-v3.json','validation-continuous-ordered-final-v3.log',
  'scene-comparison-v1.json','candidate-inventory-v1.json','author-result-v1.json','compact-author-result-v2.json','compact-author-result-v3.json',
  'suffix-nearest-order-v3.json','suffix-continuous-order-v3.json','suffix-nearest-completion-v3.json','suffix-continuous-completion-v3.json'];
const proposal={schemaVersion:1,createdAt:new Date().toISOString(),scope:'Reviewable external static scene candidates and bounded failed suffix experiments. No live run, launcher, product edit, policy relaxation or acceptance.',
  parameters:PARAMETERS,productFrozen:{driverCommit:head,backendCommit:expected.backendSourceCommit,driver9DeclaredAggregate:expected.sourceSha256,
    actualNineRawFilesVerified:expected.files,trackedClean:true,driverFreezePin:pin(BASE+'/capacity-native-witness-approach-driver-frozen-v1.json')},
  selectedForRootReview:{tenScene:'candidate-10-compact-v2.json',tenScenePin:pin(ROOT+'/candidate-10-compact-v2.json'),firstTen:original.anchors.slice(0,10),
    sceneAdopted:false,realProbeAuthorized:false,pairedFull100Ready:false,formalReady:false,
    reason:'Ten-home candidate is statically safe with better conservative6supply. Both bounded100suffix orderings still have material traversal or coverage failures; root decision is required before any new load.'},
  staticSafety:{mainScenePin:pin(ROOT+'/candidate-100-compact-v2.json'),bodyCellsChecked:900,haloChecks:2500,originalFullMapBfs:true,
    prefixesChecked:[2,10,25,50,100],prefixOnlyObservers:true,fighterIndices:PARAMETERS.fighterIndices,initialDangerAndMonsterExclusion:'Body only; full5x5halo excludes static terrain/NPC/transfer but may contain ordinary canonical monsters.',
    meaning:'Safety checks and prefix-selected-home connectivity pass; this is separate from full fixed-order scout traversal and natural supply.'},
  mainPrefixSupply:stageTable(validation),mainConditionalLevelSupply:model.stages.map(s=>({actors:s.actors,targets:s.targets,fighterCount:s.fighterCount,
    requiredPositiveHitsPerMinute:s.requiredPositiveHitsPerMinute,levels:s.levels.filter(l=>[1,5,10,15].includes(l.level))})),
  progression:model.closedPoolProgression,
  comparison:{sameCurrentCanonicalAllBodyStable6:true,existingSpan4:comparison.geometry.find(s=>s.name==='probe09-10-11-span4-halo'),
    newCompact:comparison.geometry.find(s=>s.name==='new-compact-draft02'),historicalReports:comparison.historicalReports,
    statement:'Current span4 first10:8conservative targets5Deer/3Scarecrow,92representative scout steps. Newcompact-v2:28targets21/7,97steps. Supply geometry improves; tour is five steps longer, so do not claim it shortened.',
    old31PerMinuteNotInherited:true},
  boundedSuffixReordering:{maxOrderingHeuristics:2,heuristics:['nearest-neighbour with original A* edge ranking','continuous12-row alternating-x frontier order'],
    firstTenRawByteExact:true,entire100CoordinateSetExact:true,spanAndHaloUnchanged:true,
    originalScoutResults:scout.stages,nearestScoutResults:orderedScout.stages,nearestSafetyAndPrefixChecksPassed:true,
    nearestSupply:stageTable(reordered),nearestLevelModelPin:pin(ROOT+'/conditional-level-supply-nearest-final-v3.json'),nearestClosedPoolXp:orderedModel.closedPoolProgression,
    continuousRejected:{validator:'validateNativeCombatClusters',error:'Combat home lacks declared observer coverage',logPin:pin(ROOT+'/validation-continuous-ordered-final-v3.log')},
    conclusion:'Nearest reduces25/50tour640->172 and1326->372, but100stillfails representative scout atobserver92. Continuous fails original full cluster coverage. Neither is approved as a complete100scene; no further optimization was attempted.'},
  otherCandidates:{supplyRanked30Targets123Steps:{scene:'candidate-100-v1.json',status:'retained unadopted comparison'},
    bottomRegion17Targets59Steps:{scene:'candidate-100-compact-v3.json',status:'retained unadopted comparison, created before root selectedcompact-v2'},
    drafts:'All ten bounded draft inputs/results, including every nonselected draft and incomplete reorder attempt, are retained in this dedicated directory.'},
  metadataCorrection:{fieldBefore:'minimumDistinctCells:80',fieldAfter:'minimumNativeCellsPerMinute:80',reason:'Original core increments cells by receipt-backed movedCells and divides by actual seconds. It does not count unique coordinates.',
    productChanged:false,sceneChanged:false,geometryNumericResultsExactAfterRemovingMetadata:true,levelNumericResultsExact:true,
    historicalSourceAlias:{originalPath:ROOT+'/static-lib.mjs',preservedOriginalPin:pin(ROOT+'/static-lib-preimage-v1.mjs.txt'),finalSourcePin:pin(ROOT+'/static-lib.mjs')},
    originalReportsPreserved:true,finalPrimaryValidation:'validation-compact-v2-final-v2.json',finalLevelModel:'conditional-level-supply-final-v2.json'},
  requiredFutureObservations:['No old06/07/08/09/10/11 report transfers to the new scene. Old31hit/min observations also used older attack-witness/timer contracts and include sub60second windows.',
    'Before or during a root-approved fresh attempt, use existing ordinary public snapshots/trace and density output to count actually alive/dead/removed ordinary targetIDs, actual peer owner positions and real dual-object membership; staticcanonical28is not an early live census.',
    'Keep actual per60s attack/hit, offered/completed denominator, actual cadence.level/effective public stats, clipped positive damage, scarce heldHP recovery and per-ID kill/nextpositive-hit timing. Do not infer exact respawn without wire lifecycle.',
    'Record actual scout/chase moves and gaps, witness-ready target counts if available from existing evidence, same-map active observers, moving occupancy corrections and original resource guards. No new synthetic objects/AOI or additional driver commands are authorized here.',
    'Original freshprobe2/10each300+360soak/5-5-6actual>=60windows must pass before fullformal; formal remains2/10/25/50/100each300+7200soak+5resume+5save+post60. P0pending, P2notstarted.'],
  sourcePins:[...validation.sourcePins,pin(import.meta.filename)],keyArtifacts:keyPins.map(name=>pin(ROOT+'/'+name)),
  reproduction:{requires:'Same pinned repository sources/data; copy source tools into a NEW external directory before reauthoring because every output iswx/no-overwrite. Running validator alone may use a new outputbasename in this directory.',
    validationCommand:`node "${ROOT}/validate-candidate.mjs" candidate-100-compact-v2.json validation-independent-compact-v2.json`,
    nearestValidationCommand:`node "${ROOT}/validate-candidate.mjs" candidate-100-nearest-ordered-final-v3.json validation-independent-nearest-v3.json`,
    caution:'Only static map/source reads and new external JSON writes. Never invoke a service, seeder, load runner or launcher. Continuous candidate validation is expected to failclosed.'},
  independentReviewPending:true,capacityAccepted:false,accepted:false};
const output=writeNew(ROOT+'/review-proposal-v1.json',proposal);
console.log(JSON.stringify({output,tenScene:proposal.selectedForRootReview.tenScenePin,pairedFull100Ready:false,source9Verified:true,head}));
