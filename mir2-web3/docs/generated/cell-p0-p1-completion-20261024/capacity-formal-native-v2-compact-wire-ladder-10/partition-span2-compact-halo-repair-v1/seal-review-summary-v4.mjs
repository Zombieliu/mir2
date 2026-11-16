import fs from 'node:fs';
import { execFileSync, spawnSync } from 'node:child_process';
import path from 'node:path';
import { ROOT as PRIOR, BASE, PROJECT, PARAMETERS, key, dist, pin } from '../partition-span2-compact-halo-scene-v1/static-lib.mjs';
const ROOT=import.meta.dirname,read=name=>JSON.parse(fs.readFileSync(ROOT+'/'+name));
const inventory=JSON.parse(fs.readFileSync(PRIOR+'/collection-manifest-v1.json'));for(const f of inventory.files){const actual=pin(PRIOR+'/'+f.file);if(actual.bytes!==f.bytes||actual.sha256!==f.sha256)throw new Error('Sealed80changed '+f.file);}
const driver=JSON.parse(fs.readFileSync(BASE+'/capacity-native-witness-approach-driver-frozen-v1.json'));for(const f of driver.files){const actual=pin(PROJECT+'/'+f.file);if(actual.bytes!==f.bytes||actual.sha256!==f.sha256)throw new Error('Product9sourcechanged '+f.file);}
const head=execFileSync('git',['rev-parse','HEAD'],{cwd:PROJECT,encoding:'utf8',windowsHide:true}).trim(),tracked=execFileSync('git',['status','--porcelain','--untracked-files=no'],{cwd:PROJECT,encoding:'utf8',windowsHide:true}).trim();
if(head!=='6d70d02d0bc390156b77a3e48acee714fc87cda3'||tracked)throw new Error('Immutable driverHEAD/tracked state differs');
const original=JSON.parse(fs.readFileSync(PRIOR+'/candidate-100-nearest-ordered-final-v3.json')),scene=read('candidate-100-repaired-v4.json'),result=read('repair-result-v1.json'),validation=read('validation-repaired-v4.json'),scout=read('prefix-scout-cost-repaired-v4.json'),model=read('conditional-level-supply-repaired-v4.json');
const changed=scene.anchors.flatMap((p,i)=>key(p)===key(original.anchors[i])?[]:[{actorIndex:i,before:original.anchors[i],after:p,displacement:dist(p,original.anchors[i])}]);
if(changed.length!==1||changed[0].actorIndex!==92||changed[0].displacement!==3)throw new Error('Unexpected localrepair delta');
if(!fs.readFileSync(ROOT+'/candidate-10-repaired-v4.json').equals(fs.readFileSync(PRIOR+'/candidate-10-compact-v2.json')))throw new Error('First10raw changed');
if(!scout.stages.every(s=>s.passed)||!result.allRepresentativePrefixScoutsPassed)throw new Error('A representative scout remains failed');
if(result.attempts.length!==2)throw new Error('Unexpected attempt count');
const correction={schemaVersion:1,scope:'Corrects an earlier assistant status message only; no rawtrial/result/source changed',earlierMessageClaimedActualTrials:1,
  actualRecordedTrials:2,first:{home:{x:229,y:533},result:'failed, stilloutsidecoverage at244538',receipt:result.attempts[0]},
  second:{home:{x:229,y:534},result:'passed representative100scout',receipt:result.attempts[1]},rankedCandidatePoints:16,changedActors:1};
const correctionFile=ROOT+'/attempt-count-correction-v1.json';fs.writeFileSync(correctionFile,JSON.stringify(correction,null,2)+'\n',{flag:'wx'});
let checkedPaths=0,bad=[];
for(const s of validation.stages)for(const f of s.perFighter)for(const p of f.paths){checkedPaths++;const index=scene.anchors.slice(0,s.count).findIndex((q,i)=>i!==f.actorIndex&&key(q)===key(p.goal));if(index>=0)bad.push({count:s.count,fighter:f.actorIndex,target:p.objectId,otherActor:index});}
if(bad.length)throw new Error('Saved targetpath goal occupies another representative home');
const syntax=[];for(const file of fs.readdirSync(ROOT).filter(x=>x.endsWith('.mjs')).sort()){const run=spawnSync(process.execPath,['--check',ROOT+'/'+file],{encoding:'utf8',windowsHide:true});syntax.push({source:pin(ROOT+'/'+file),exitCode:run.status,diagnostic:run.stderr||null});if(run.status!==0)throw new Error('ExternalJS syntaxfailed '+file);}
const syntaxFile=ROOT+'/syntax-verification-v4.json';fs.writeFileSync(syntaxFile,JSON.stringify({schemaVersion:1,scope:'Parseronly externaltools, no game test/build/service',count:syntax.length,files:syntax,passed:true},null,2)+'\n',{flag:'wx'});
const summary={schemaVersion:1,createdAt:new Date().toISOString(),scope:'Bounded external layoutrepair staticreview package. No product edits, threshold change, scene-name addition, grant, realm or capacity run.',
  source:{driverCommit:head,backendCommit:driver.backendSourceCommit,driver9Aggregate:driver.sourceSha256,actualNineRawFilesVerified:true,trackedClean:true,
    sealedPriorFilesUnchanged:inventory.fileCount,priorCollection:pin(PRIOR+'/collection-manifest-v1.json')},
  scene:{full:pin(ROOT+'/candidate-100-repaired-v4.json'),ten:pin(ROOT+'/candidate-10-repaired-v4.json'),originalNearest:pin(PRIOR+'/candidate-100-nearest-ordered-final-v3.json'),
    firstTenRawByteExact:true,monsterNames:scene.monsterNames,patrolSpan:scene.patrolSpan,changedCoordinates:changed,combatIndices:PARAMETERS.fighterIndices},
  boundedWork:{limits:{actors:4,rankedCandidatesPerActor:16,maximumOriginalDisplacement:6},actual:{changedActors:1,rankedCandidatesForActor92:16,actualTrialResults:2},
    correction:pin(correctionFile),allTrialPins:result.attempts,failedFirstTrialPreserved:true,
    initialDiagnosis:pin(ROOT+'/original-failure-diagnosis-v1.json'),repairedLegProof:pin(ROOT+'/repaired-leg-proof-v4.json'),
    explanation:'Original fourpaths were found in453..528nodes but crossed3uncovered cells; after home92shift, firstgoal230535 takes332nodes/53steps, allcells also lie withinOLDcoverage. The route destination changed; no coverage/path algorithm or node budget was widened.'},
  staticContracts:{validation:pin(ROOT+'/validation-repaired-v4.json'),actualOriginalPrefixValidators:pin(ROOT+'/original-prefix-contracts-v4.json'),
    bodyChecked:900,haloChecks:2500,wholeBfs:true,originalClustersPassedFor:[10,25,50,100],prefix2ClusterNotApplicableBecauseNoFighter:true,
    activeOnlyCoverageAndOriginal4000TargetPaths:true,acceptedPathCount:checkedPaths,acceptedGoalEqualsOtherRepresentativeHome:0,
    fixedOrderScout:pin(ROOT+'/prefix-scout-cost-repaired-v4.json'),allRepresentativeScoutsPassed:true,
    scoutRows:scout.stages.map(s=>({count:s.count,passed:s.passed,firstCircuit:s.firstCircuitUnitWalkSteps??s.firstCircuitSteps??null,secondCircuit:s.secondCircuitUnitWalkSteps,
      maxLegUnitWalkSteps:Math.max(...s.legs.map(l=>l.steps)),legsOver48UnitSteps:s.legs.filter(l=>l.steps>48).map(({observerIndex,from,goal,steps})=>({observerIndex,from,goal,steps}))})),
    scoutBudgetBoundary:'Static representative originalfirst4/4000A*pass is not an online24ACK/20s waypoint simulation. At50/100some53..65unitstep legs can be advanced before reachingthegoal, even with fastRun. No actual complete886step tour,50%Run,95%completion or supply guarantee follows.'},
  conditionalSupply:{model:pin(ROOT+'/conditional-level-supply-repaired-v4.json'),actualLaunchModelPerformed:false,
    stages:model.stages.map(s=>({actors:s.actors,fighters:s.fighterCount,canonicalStableTargets:s.targets,counts:s.targetCounts,
      requiredLaunchedPerMinute:s.requiredAttacksPerMinute,requiredPositiveHitsPerMinute:s.requiredPositiveHitsPerMinute,
      levels:s.levels.filter(l=>[1,5,10,15].includes(l.level)).map(l=>({level:l.level,starterDc:[l.ordinaryStarterMinDc,l.ordinaryStarterMaxDc],
        idealExpectedPositiveHitsPerMinute:l.idealUniformExpectedPositiveHitsPerMinute,conditionalAllLaunchesPositiveMarginOverLaunchedRequirement:l.idealUniformExpectedPositiveHitsPerMinute-s.requiredAttacksPerMinute,
        proofOfLaunchedRate:false,proofOfAny60sWindow:false}))})),
    closedPoolProgression:model.closedPoolProgression,
    distinction:'DP estimates positivehits to consumeHP. It does not estimate legitimate launched attacks/misses or actualallocation. N100requires180launched AND120positive/min;130.474ideal level15positive/min alone does not prove180launched. No syntheticmiss/padding/quarantine/denominator change is allowed. These are conditional sensitivities, not an unconditionalfresh2hfailure.'},
  independentFirstTenReview:pin(BASE+'/compact-halo-scene-independent-review-v1.json'),independent100ReviewPending:true,
  proofAdapters:pin(ROOT+'/validation-adapter-provenance-v2.json'),syntaxVerification:pin(syntaxFile),
  externalSetupFailuresPreserved:['diagnose-failure-setup-preimage-v1.mjs.txt','diagnose-failure-setup-preimage-v2.mjs.txt','diagnostic-adapter-setup-failure-v1.json','diagnose-original-run-v2.log','full-static-validation-v4.log'],
  rootDecision:{pairedStaticInputsHavePassedSpecifiedSafetyAndRepresentativePathCriteria:true,newProbeOrLauncherStarted:false,rootRealRunApproval:false,
    formalCapacityReady:false,P0Accepted:false,P2Started:false,next:'Independent100review then root decision; retain original2/10/25/50/100300s,7200ssoak,5resume5savepost60 and everyoriginalthreshold.'},
  reproduction:{validator:`node "${ROOT}/validate-repaired.mjs" candidate-100-repaired-v4.json validation-independent-repaired-v4.json`,
    caution:'wxoutputs require new outputnames; any full reauthoring uses a new externaldirectory. Do not modify source, sealed80items or launchservices.'},
  accepted:false,capacityAccepted:false};
const out=ROOT+'/review-summary-v4.json';fs.writeFileSync(out,JSON.stringify(summary,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({summary:pin(out),scene:summary.scene.full,ten:summary.scene.ten,syntax:syntax.length,source9Verified:true,priorSealed:inventory.fileCount,accepted:false,capacityAccepted:false}));
