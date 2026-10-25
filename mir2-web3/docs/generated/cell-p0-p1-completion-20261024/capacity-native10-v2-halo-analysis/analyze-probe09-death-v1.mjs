import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import readline from 'node:readline';
const root='C:/mir2-build/p01-completion-20261024',run=path.join(root,'release-native-10-v2-halo-feasibility-09'),out=path.join(root,'probe09-death-analysis');
if(fs.existsSync(out))throw Error('Refuse overwrite analysis');fs.mkdirSync(out);
const actor='L26466205',guardAt=1792891599013,runId='1792891029840-02155527';
const hash=b=>crypto.createHash('sha256').update(b).digest('hex'),pin=file=>{const b=fs.readFileSync(file);return{file,bytes:b.length,sha256:hash(b)}};
const reportFile=path.join(run,'reports',runId+'.report.json'),report=JSON.parse(fs.readFileSync(reportFile)),rawFile=path.join(run,'reports',runId+'.samples.jsonl');
const digest=crypto.createHash('sha256'),input=fs.createReadStream(rawFile);let bytes=0,line=0;input.on('data',b=>{bytes+=b.length;digest.update(b);});
const kinds={},windows=[],controls=[],life=[],transforms=[],actions=[],ownerLife=[],traces=[],injuries=[],actorIds=new Set();
for await(const t of readline.createInterface({input,crlfDelay:Infinity})){
 const r=JSON.parse(t),entry={line:++line,...r};kinds[r.type]=(kinds[r.type]??0)+1;
 if(['phaseStarted','phaseEnded','safetyStop','finished'].includes(r.type))controls.push(entry);
 if(r.type==='window'&&r.phase==='stage-10')windows.push(entry);
 if(r.actor!==actor)continue;
 if(r.type==='playerLifecycle')life.push(entry);
 if(r.type==='selfTransform'&&r.at>=1792891488618)transforms.push(entry);
 if(r.type==='action'&&r.at>=1792891488618)actions.push(entry);
 if(r.type==='pendingAttackEvidence'){
  ownerLife.push({line,at:r.sentAtWallMs,phase:r.phase,requestId:r.requestId,owner:r.owner,target:r.target,outcome:r.outcome});
  for(const p of r.packets)if(p.receiver==='observer'&&p.packet==='ObjectAttack'&&p.payload?.objectId!==r.target?.objectId)actorIds.add(p.payload?.objectId);
  const ids=new Set([r.owner?.objectId,...actorIds]);
  for(const p of r.packets)if(ids.has(p.payload?.objectId)&&['ObjectStruck','ObjectHealth','DamageIndicator','ObjectDied','ObjectRevive'].includes(p.packet))injuries.push({traceLine:line,requestId:r.requestId,...p});
  if(r.at>=1792891587000)traces.push(entry);
 }
}
const rawPin={file:rawFile,bytes,sha256:digest.digest('hex'),rows:line};
const selected={schemaVersion:1,runId,actor,scope:'Bounded public driver projections only; no private/store or complete recordings copied.',pins:{report:pin(reportFile),raw:rawPin,cleanup:pin(path.join(run,'realm-cleanup.json')),scene:pin(path.join(run,'scenario.json')),sourcePins:pin(path.join(run,'source-pins-admitted-v2.json')),helper:pin(import.meta.filename)},
 rawTypeCensus:kinds,controls,stage10Windows:windows,all66AttackOwnerSnapshots:ownerLife,actorObservedRemoteObjectIds:[...actorIds],recordedOwnerOrObserverInjuryPackets:injuries,
 failedWindowAndFinalActions:actions,last12SecondsFullAttackTraces:traces,failedWindowAndFinalOwnerTransforms:transforms,
 actorLifecycleSinceFinalAttack:life.filter(r=>r.at>=1792891597000),actorLifecycleMapOrSelfMembershipChanges:life.filter(r=>r.before?.self?.objectId!==r.after?.self?.objectId||r.before?.mapFileName!==r.after?.mapFileName),
 limits:['OwnerHP in attack traces is the snapshot atsend, not a continuous health log.','PLAYER_LIFECYCLE and selfTransform projections retain identity/coordinates, notHP/dead; pendingAttackEvidence does not contain HealthChanged and exists only during pendingattacks.','Target206534 ObjectDied at1597301 is aDeer, not the player. No death verdict is inferred solely from the merged checkOwner error.']};
const selectedFile=path.join(out,'selected-events-v1.json');fs.writeFileSync(selectedFile,JSON.stringify(selected,null,2)+'\n',{flag:'wx'});
const recording=path.join(run,'recordings/0-498025.jsonl'),recordHash=crypto.createHash('sha256');
let carry=Buffer.alloc(0),offset=0,ordinal=0,recordBytes=0;
const picks=[{name:'last-before-guard',select:(at,best)=>at<=guardAt&&(!best||at>best.at)},{name:'first-after-guard',select:(at,best)=>at>=guardAt&&(!best||at<best.at)},{name:'first-after-guard-plus500ms',select:(at,best)=>at>=guardAt+500&&(!best||at<best.at)}];
for await(const chunk of fs.createReadStream(recording)){
 recordHash.update(chunk);recordBytes+=chunk.length;const b=Buffer.concat([carry,chunk]);let start=0;
 for(let end;(end=b.indexOf(10,start))!==-1;start=end+1){
  const raw=b.subarray(start,end),header=raw.subarray(0,Math.min(2048,raw.length)).toString('utf8');ordinal++;
  const at=Number(header.match(/"capturedAtMs":([0-9]+)/)?.[1]);
  if(Number.isSafeInteger(at)&&Math.abs(at-guardAt)<=2000)for(const p of picks)if(p.select(at,p.best))p.best={at,ordinal,byteOffset:offset+start,bytes:raw.length,line:raw.toString('utf8')};
 }
 offset+=start;carry=b.subarray(start);
}
if(carry.length||recordBytes!==fs.statSync(recording).size||picks.some(p=>!p.best))throw Error('Recording framing or narrow selection incomplete');
const field=(o,keys)=>Object.fromEntries(keys.filter(k=>o[k]!==undefined).map(k=>[k,o[k]]));
const frames=picks.map(p=>{
 const f=JSON.parse(p.best.line),entities=f.world?.entities??[],own=entities.filter(e=>e.name===actor),ids=new Set(own.map(e=>e.objectId)),center=own[0]??{x:264,y:649};
 const near=entities.filter(e=>e.kind==='monster'&&Number.isFinite(e.x)&&Number.isFinite(e.y)&&Math.max(Math.abs(e.x-center.x),Math.abs(e.y-center.y))<=8);
 const ev=(f.events??[]).filter(e=>(e.name===actor||ids.has(e.objectId))&&['health','death','move','spawn','despawn','revive'].includes(e.kind));
 return{selection:p.name,sourceLineOrdinal:p.best.ordinal,sourceByteOffset:p.best.byteOffset,sourceLineBytes:p.best.bytes,sourceLineSha256:hash(Buffer.from(p.best.line)),
  header:field(f,['schema','recordingId','sequence','capturedAtMs','mapFileName','digest']),
  actorEntities:own.map(e=>field(e,['objectId','name','kind','x','y','dead','hp','maxHp','level','ai','disposition'])),actorNameMatches:own.length,
  localCenter:field(center,['x','y']),centerScope:own.length?'Matching actor public spectator entity':'Last real ownerUserLocation264649 fallback, not a manufactured spectator presence',
  nearbyMonsterCount:near.length,nearbyMonsters:near.slice(0,32).map(e=>field(e,['objectId','name','kind','x','y','dead','hp','maxHp','level','ai','disposition'])),nearbyTruncated:near.length>32,
  actorDerivedFrameEvents:ev.map(e=>({kind:e.kind,atMs:e.atMs,objectId:e.objectId,name:e.name,payload:field(e.payload??{},['from','to','dead','x','y','entityKind'])})),
  eventScope:'Spectator world_events differences are derived snapshots, not original protocol packets and do not identify an attacker.'};
});
const projection={schemaVersion:1,runId,actor,guardAtMs:guardAt,scope:'Exactly3 public spectator JSON frame projections within guard±2s; header-only scan+SHA for other lines, no video or privatepayload.',
 recording:{file:recording,bytes:recordBytes,sha256:recordHash.digest('hex'),rows:ordinal},frames,
 limits:['Merged spectator world may retain prior per-object observations; selected snapshots do not prove exact owner wire-packet order.','world.playerHp/playerObjectId/equipment belong a chosen merged base owner and were deliberately not treated as thisactorHP/gear.','Nearby monsters and positions do not by themselves prove who caused any hit; no attacker is inferred from proximity.']};
const projectionFile=path.join(out,'spectator-three-frame-projection-v1.json');fs.writeFileSync(projectionFile,JSON.stringify(projection,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({selected:pin(selectedFile),projection:pin(projectionFile),recording:projection.recording,frames:frames.map(f=>({selection:f.selection,at:f.header.capturedAtMs,sequence:f.header.sequence,own:f.actorEntities,events:f.actorDerivedFrameEvents,near:f.nearbyMonsters.map(m=>({id:m.objectId,name:m.name,x:m.x,y:m.y,hp:m.hp,dead:m.dead}))})),injuryPackets:injuries,ownerHp:ownerLife.filter((x,i,a)=>!i||x.owner?.hp!==a[i-1].owner?.hp).map(x=>({at:x.at,hp:x.owner?.hp,max:x.owner?.maxHp,pos:field(x.owner??{},['x','y']),request:x.requestId}))}));
