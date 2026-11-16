import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import readline from 'node:readline';
const root='C:/mir2-build/p01-completion-20261024',dir=path.join(root,'probe09-death-analysis'),repo='C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const hash=b=>crypto.createHash('sha256').update(b).digest('hex'),pin=file=>{const b=fs.readFileSync(file);return{file,bytes:b.length,sha256:hash(b)}},read=file=>JSON.parse(fs.readFileSync(file));
const selectedFile=path.join(dir,'selected-events-v1.json'),selected=read(selectedFile),firstProjectionFile=path.join(dir,'spectator-three-frame-projection-v1.json'),projection=read(firstProjectionFile);
const output=path.join(dir,'summary-v1.json'),enriched=path.join(dir,'spectator-three-frame-projection-v2.json'),witnessFile=path.join(dir,'request49-witness-loss-v1.json');
for(const f of[output,enriched,witnessFile])if(fs.existsSync(f))throw Error('Refuse overwrite derived evidence');
const fields=(o,keys)=>Object.fromEntries(keys.filter(k=>o?.[k]!==undefined).map(k=>[k,o[k]]));
const fd=fs.openSync(projection.recording.file,'r');
try{for(const f of projection.frames){const b=Buffer.alloc(f.sourceLineBytes);if(fs.readSync(fd,b,0,b.length,f.sourceByteOffset)!==b.length||hash(b)!==f.sourceLineSha256)throw Error('Selectedframe source bytes mismatch');
 const raw=JSON.parse(b),slot=raw.world?.entities?.find(e=>e.objectId===1000);
 f.mergedWorldSharedSelfSlot1000=fields(slot,['objectId','name','kind','x','y','dead','hp','maxHp','level']);
 f.sharedSelfSlotMatchesActor=slot?.name===selected.actor;
 f.matchingActorOwnerStateDeltas=f.actorDerivedFrameEvents.filter(e=>e.objectId===1000&&e.name===selected.actor&&['health','death'].includes(e.kind));
}}finally{fs.closeSync(fd);}
projection.originalProjection=pin(firstProjectionFile);projection.sameThreeFrameReReadOnly=true;
projection.limits.push('ObjectId1000 is a shared personal-self slot across capturedclients. Its previoushealth fromvalue can belong to a different owner; matchingcurrent eventname and to/dead fields are required. Repeated death deltas after interleaved selfsnapshots do not mean repeated deaths or revival. Remote50005 hpnull/deadfalse/level1 is incomplete and cannot refute ownHP0/deadtrue updates.');
fs.writeFileSync(enriched,JSON.stringify(projection,null,2)+'\n',{flag:'wx'});
let ordinal=0,request49;
for await(const t of readline.createInterface({input:fs.createReadStream(selected.pins.raw.file),crlfDelay:Infinity})){
 const r=JSON.parse(t);ordinal++;if(r.type==='pendingAttackEvidence'&&r.actor===selected.actor&&r.requestId==='49'){request49={line:ordinal,...r};break;}
}
if(!request49||request49.line!==12457||request49.truncated||request49.packets.length!==16)throw Error('Expected request49 evidence missing');
const witness={schemaVersion:1,scope:'Original matchedlaunched request49 lacks mandatory observerdamage, so its originalmiss/quarantine remains. Ownerpositive damage is not retroactively promoted to dual-endhit.',raw:selected.pins.raw,trace:request49,
 action:selected.failedWindowAndFinalActions.find(a=>a.requestId==='49'),
 facts:{sentAtMs:1792891566304,typedLaunchedAtMs:1792891566317,observerActorAndTargetRemovedAtMs:1792891566340,ownerTargetStruckDamage2Health8AtMs:1792891566402,ownerRetaliatoryDeerDamage2AtMs:1792891566899,originalMissFinalizedAtMs:1792891567213,
  observerTargetDamageIndicatorCaptured:false,ownerTargetPositiveDamageCaptured:true,receiptTimeout:false,physicalRandomMissProven:false,fatalAttackerProven:false}};
fs.writeFileSync(witnessFile,JSON.stringify(witness,null,2)+'\n',{flag:'wx'});
const sourceFiles={soak:'apps/web/scripts/playtest-capacity-soak.mjs',observation:'apps/web/scripts/quest-agent/protocol-observation.mjs',gear:'apps/web/scripts/quest-agent/protocol-play.mjs',passiveAi:'apps/simulation/src/runtime/zone/runtime/passive_ai.rs',zone:'apps/simulation/src/runtime/zone/runtime.rs',spectator:'apps/gateway/src/spectator.rs',staticAuthor:'C:/mir2-build/p01-completion-20261024/partition-span4-halo-static-author-v1.mjs'};
const sources=Object.fromEntries(Object.entries(sourceFiles).map(([k,f])=>[k,path.isAbsolute(f)?f:path.join(repo,f)]));
const ref=(name,symbol)=>{const text=fs.readFileSync(sources[name],'utf8'),i=text.indexOf(symbol);if(i<0)throw Error('Source symbol missing');return{file:sourceFiles[name],line:text.slice(0,i).split(/\r?\n/).length};};
const failed=selected.stage10Windows.find(w=>!w.assessment.passed),a=failed.assessment.activity.find(a=>a.actor===selected.actor),windowActions=selected.failedWindowAndFinalActions.filter(r=>r.at>=failed.startedAtMs&&r.at<=failed.endedAtMs),chaseCounts=windowActions.filter(r=>r.chaseTargetId).reduce((v,r)=>{v[r.chaseTargetId]=(v[r.chaseTargetId]??0)+1;return v;},{});
const deathDeltas=projection.frames.flatMap(f=>f.matchingActorOwnerStateDeltas.filter(e=>e.kind==='death'&&e.payload.dead===true).map(e=>({frameSequence:f.header.sequence,capturedAtMs:f.header.capturedAtMs,...e})));
if(!deathDeltas.length)throw Error('No matching public deathstate');
const hpChanges=selected.all66AttackOwnerSnapshots.filter((s,i,all)=>!i||s.owner?.hp!==all[i-1].owner?.hp).map(s=>({rawLine:s.line,at:s.at,requestId:s.requestId,owner:s.owner}));
const summary={schemaVersion:1,runId:selected.runId,actor:selected.actor,status:'FAILED unchanged',capacityAccepted:false,
 evidence:{selected:pin(selectedFile),sameThreePublicFrames:pin(enriched),witnessLoss:pin(witnessFile),raw:selected.pins.raw,report:selected.pins.report,cleanup:selected.pins.cleanup,sourcePins:Object.fromEntries(Object.entries(sources).map(([k,f])=>[k,pin(f)])),helper:pin(import.meta.filename)},
 verdict:{actualDeathStateObserved:true,wireFatalPacketRetained:false,fatalAttackerKnown:false,presenceOnlyTransientProven:false,
  explanation:'Matchingactor public server-snapshot-derived self1000 events at1792891599088 showHPto0/deadtrue, corroborating genuine observeddeath state aroundguard1792891599013. This is stronger than the mergedcheckOwner error, but not an originalfatalwirepacket or proof of exactexecutionorder. Lateridentity presence cannot establishalive.',
  originalGate:'Only stage2 completed300.301s/five truewindows andpassed. Stage10 incomplete170.484s; no completedstage10 orsoak. ContinuousfirstFailure null does not cancel actualdeath or the independently failed completewhole-cohortwindow.'},
 deathTimeline:{guardAtMs:1792891599013,lastAttack:{requestId:'66',sentAtMs:1792891597237,ownerAtSend:{objectId:1000,x:265,y:649,hp:3,maxHp:24,dead:false},targetDied:{objectId:206534,name:'Deer',atMs:1792891597301}},
  lastActualOwnerMovement:{atMs:1792891598671,source:'UserLocation',x:264,y:649},
  ownerHealthBeforeGuard:hpChanges,publicDeathStateDeltas:deathDeltas,
  identityAfterGuard:'Drainlifecycle at1599451/1599512 retains ownself1000 at264649; this carries identity/position only, notaliveHP. The firstself=null fullsnapshot at1599885 accompaniesordinarylogout, afterstop.',
  remoteProjectionCaveat:'Sameactor remote50005 hasnullHP/deadfalse/level1 inselectedframes; those incomplete/olderremote fields cannot override matchingownstate eventHP0/deadtrue. ID1000 is reused betweenpersonal snapshots, so eventfrom18 is not asserted to be thisactorpreviousHP.'},
 earlierIndependentWindowFailure:{startedAtMs:failed.startedAtMs,endedAtMs:failed.endedAtMs,actualSeconds:failed.assessment.seconds,passed:false,
  nativeOpportunities:a.nativeOpportunities,nativeCompleted:a.nativeCompleted,moves:a.moves,runs:a.runs,cells:a.cells,attacks:a.attacks,positiveHits:a.hits,positiveDamage:a.damage,
  chaseMovementByTarget:chaseCounts,combatCoverage:failed.assessment.combatCoverage,movementPassed:failed.assessment.movementPassed,chatPassed:failed.assessment.chatPassed,timeoutRatio:failed.assessment.timeoutRatio,correctionRatio:failed.assessment.correctionRatio,
  interpretation:'68 successfulchase moves toDeer206715 andonly1attack/positivehit in60.036s fail18/12. Movingtarget/timid behavior is compatible, but its exacttimid flag is not present inraw and is not claimed. This failure precedesdeathguard by50.359s.'},
 injuryEvidence:{knownEarlierAttacker:{objectId:206528,name:'Deer',ownerObjectStruckAtMs:1792891566899,ownerDamage:2,pairedOwnerObserverObjectStruckAtMs:1792891577996,pairedDamage:1},
  request49WitnessFailure:witness.facts,
  laterPublicNearby:'Threeframes show liveDeer206528 at266649HP2,Deer206704 at266650HP10, otherDeer/Hen andScarecrow206608 corpse. Proximity does not identify thefatalattacker. No unseenCat/extraenemy is inferred.',
  mechanismLimit:'Request49 positiveownerhit withoutobserverdamage remainsmiss/quarantined byoriginalstrictproof. A survivingprovokedDeer can continue ordinaryretaliation afterthetool stops selectingit; this is a workloadsurvival risk, not proof thatitdeliveredthefatalhit.'},
 sourceFindings:{checkOwner:{reference:ref('soak','function checkOwner('),meaning:'Missingstrictself OR authoritativeexactHP<=0/self.dead; error alone cannot distinguish.'},
  deathPredicate:{reference:ref('observation','export function hasAuthoritativePlayerDeath'),meaning:'self.dead or exactsnapshot.playerHp/exactself.hp<=0; percent0alone is notdeath.'},
  deerRetaliation:{reference:ref('passiveAi','pub(super) fn passive_monster_on_damage'),attackReference:ref('passiveAi','if !state.timid && native_monster_adjacent_to'),meaning:'AI2 normalDeer records apositive directattacker, pursues thatliveeligibleplayer andattacks whenadjacent. Timid1/7 subtypeflees. Neutral doesnotmean immune fromretaliation.'},
  scarecrowThreat:{reference:ref('zone','if !monster.hostile_to_player'),aggroReference:ref('zone','fn nearest_native_monster_target('),meaning:'ScarecrowAI0withHostile disposition follows ordinarygenericnearesteligibleplayer aggro8x6 andadjacentattack. Notshown asthefatalattackerhere.'},
  runtimeChaseScope:{reference:ref('soak','const plan = movement(client, destination, context.map, point => insideCombatCoverage'),meaning:'Currentruntimechase enforcescollision,activeobservercoverage andnotransfer. ItdoesnotapplytheauthorinitialdangerMask; astaticclearhome/bootstrap isnotachasecorridorsafetyguarantee.'},
  initialDangerScope:{reference:ref('staticAuthor','const dangerMask'),meaning:'Originalinitial8x6dangerexclusion andinitialmonster exclusions applytoauthorbody/bootstrap only. DeerAI2isnotmarked hostilehighDCdanger bythismask.'},
  ordinaryGear:{reference:ref('gear','export async function equipStarterGear'),meaning:'Singleordinaryattempttoequip availableweapon/armour/torch/necklace/ringLeftbefore ready; norun-levelrepair,additionalgrants orguaranteeofallgear.'},
  healing:{reference:ref('zone','fn tick_player_vital_regen('),meaning:'Passive3%+1 HPpulse each3s, gated3safterdamage; driver offersnouseItem/heal/supply/repair action. Level2maxHP24 cannotbeassumed sustainableagainstseveralretaliatingenemies.'},
  spectator:{reference:ref('spectator','fn world_events('),meaning:'Eventhealth/death arederivedfromsnapshots, notprotocolreceipts; source1000name interleavinglimitsfromvalueinterpretation.'}},
 comparison08:{observed:'08had349positivehits andattackplanninglevels1to5 over11.318minwithoutrecordeddeath;09observedlevel2,maxHP24 andprolongedmovingprey/retaliation. Scenegeometry/AIclocks/target selectiondiffer;08survivalisnot09safetyproof.',sourceRiskReport:pin(path.join(root,'halo-natural-progression-risk-summary-v2.json'))},
 limits:['Only3 authorizedpublicJSONframes wereprojected, withcompletefileSHA/sequence/byteoffset; novideo/private payloadread.','NooriginalownerfatalHealthChanged/ObjectDiedpacketwaspersistedinpendingtrace; fatalattacker/preciseexecutiontimeunknown.','The3sameframeswerere-readonlytoexplainselfID1000aliasing; noplannedextra snapshotsselected.','Noautomaticresurrection,healinggrant,scenechange,gatechange,sourceedit,tests,realm orcapacitypass.']};
fs.writeFileSync(output,JSON.stringify(summary,null,2)+'\n',{flag:'wx'});
const publicFiles=[selectedFile,firstProjectionFile,enriched,witnessFile,output,path.join(root,'analyze-probe09-death-v1.mjs'),import.meta.filename].map(pin),whitelist=path.join(dir,'public-whitelist-v1.json');
fs.writeFileSync(whitelist,JSON.stringify({schemaVersion:1,scope:'Explicitpublic09analysisderivativesonly; originaldriverraw/recordingremainexternal. Noaccount/private/worlddump.',files:publicFiles},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({summary:pin(output),projection:pin(enriched),witness:pin(witnessFile),whitelist:pin(whitelist),actualDeathStateObserved:summary.verdict.actualDeathStateObserved,fatalAttackerKnown:false,independentWindow:summary.earlierIndependentWindowFailure}));
