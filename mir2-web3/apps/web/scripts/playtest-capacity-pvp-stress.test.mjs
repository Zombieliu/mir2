import test from 'node:test';
import assert from 'node:assert/strict';
import { pvpStressOptions, DevelopmentPvpLoginBudget, pvpStressRaw, pvpWindowResult, createPvpStressEngine, pvpSlots, validatePvpAuthHealth, pvpStressClientPolicy, PvpStressClient, applyPvpStageResult, preparePvpGroup, createPvpFirstFailureRecorder, pvpPublicFailureMessage, recordPvpHealObservation, pvpHealComplete, waitPvpAttackOutcome } from './playtest-capacity-pvp-stress.mjs';
import { Histogram, LOGIN_WINDOW_MS } from './playtest-capacity-soak-core.mjs';
import { CapacityClient } from './playtest-capacity-soak.mjs';

function fixture(count=500){
  const actors=[],groups=[];
  for(let i=0;i<count;){const size=i===22||i===25?3:2,groupIndex=groups.length,indices=Array.from({length:size},(_,j)=>i+j),origin={x:10+(groupIndex%20)*10,y:10+Math.floor(groupIndex/20)*10};
    assert(i+size<=count);for(let j=0;j<size;j++)actors.push({poolIndex:i+j,groupIndex,targetIndex:indices[(j+1)%size],name:'ltpvp'+String(i+j).padStart(4,'0'),characterIndex:0,position:{x:origin.x+(j===1?1:0),y:origin.y+(j===2?1:0)}});
    groups.push({groupIndex,type:size===3?'triangle':'pair',actorIndices:indices});i+=size;
  }
  return {schemaVersion:1,scope:'server-only-preconfigured-pvp',preseeded:true,ordinaryEvidence:false,count,mapFileName:'0',actors,groups,
    hpItem:{templateIndex:662,name:'(HP)DrugLarge',grid:'Inventory',perActor:194},requiresGatewayProfile:{developmentLoopbackAuthBudget:600,authWindowSeconds:900}};
}
function options(extra={}){return {realm:'isolated',endpoint:'ws://127.0.0.1:17710/ws',count:500,finiteWallSeconds:18000,...extra};}
function raw(f,index=0){return {mapFileName:'0',playerObjectId:1000,playerHp:674,playerMaxHp:674,inSafeZone:false,inventoryItems:[{name:'(HP)DrugLarge',uniqueId:2,quantity:20,container:'bag1',slot:0,tooltipSource:{info:{item_index:662},realInfo:null}}],beltItems:[],entities:f.actors.slice(0,2).map((a,i)=>({kind:i===index?'selfPlayer':'player',objectId:i===index?1000:50000+i,name:a.name,dead:false,...a.position}))};}

test('PvP500 has247 pairs/two triangles and exact closed25 prefix',()=>{
  const f=fixture(),c=pvpStressOptions(options(),f);
  assert.deepEqual(c.stages,[2,10,25,50,100,250,500]);assert.equal(f.groups.filter(g=>g.type==='pair').length,247);assert.equal(f.groups.filter(g=>g.type==='triangle').length,2);
  assert.deepEqual(f.actors.slice(22,25).map(a=>a.targetIndex),[23,24,22]);assert.equal(c.stageSeconds,300);assert.equal(c.soakSeconds,0);
});
test('PvP strict stages/pilot/soak/wall and auth600 are explicit',()=>{
  const f=fixture();assert.equal(pvpStressOptions(options({stageSeconds:60,pilot:true}),f).stageSeconds,60);
  assert.throws(()=>pvpStressOptions(options({stageSeconds:60}),f));assert.throws(()=>pvpStressOptions(options({soakSeconds:3600}),f));
  assert.throws(()=>pvpStressOptions(options({finiteWallSeconds:2000}),f));assert.throws(()=>pvpStressOptions(options({stages:'2,9'}),f));
  assert.equal(pvpStressOptions(options({authBudget:600,developmentAuthProfile:'loopback-600-v1'}),f).authBudget,600);
  assert.throws(()=>pvpStressOptions(options({authBudget:600}),f));
  assert.throws(()=>pvpStressOptions(options({authBudget:600,developmentAuthProfile:'loopback-600-v1',endpoint:'wss://example.org/ws'}),f));
});
test('PvP missing/duplicate/wrong group targets, occupied cells and open stage prefix reject',()=>{
  for(const mutate of[f=>f.actors[24].targetIndex=25,f=>f.actors[0].targetIndex=0,f=>f.actors[1].position={...f.actors[0].position},f=>f.groups[0].actorIndices=[0,0],f=>f.actors[1].groupIndex=99]){
    const f=fixture();mutate(f);assert.throws(()=>pvpStressOptions(options(),f));
  }
});
test('Development auth600 preserves existing failed attempts and exact901s expiry',()=>{
  let at=10000000;const attempts=Array.from({length:599},(_,i)=>({kind:'admission',at:at-1000+i})),b=new DevelopmentPvpLoginBudget(attempts,0,()=>at);
  b.reserve();assert.equal(b.attempts.length,600);assert(b.delay()>0);assert.throws(()=>b.reserve());at+=LOGIN_WINDOW_MS;assert.equal(b.delay(),0);b.reserve();assert.equal(b.attempts.length,1);
  const future=new DevelopmentPvpLoginBudget([],at+1000,()=>at);assert.equal(future.delay(),1000);
});
test('PvP LargeHP canonical nullable tooltip identity and inventory grid remain exact',()=>{
  const f=fixture(2),r=raw(f);const s=pvpStressRaw(r,f.actors[0].name,1,42);
  assert.equal(s.hpStock,20);assert.equal(s.items[0].grid,'Inventory');assert.equal(s.hp,674);assert.equal(s.at,42);
  r.inventoryItems[0].quantity=1;assert.equal(s.items[0].quantity,20);
  for(const mutate of[r=>r.inventoryItems[0].tooltipSource.realInfo={item_index:658},r=>r.inventoryItems.push(structuredClone(r.inventoryItems[0])),r=>delete r.playerHp,r=>r.inSafeZone=true,r=>r.entities[0].kind='player',r=>r.inventoryItems[0].quantity=Infinity]){
    const r=raw(f);mutate(r);assert.throws(()=>pvpStressRaw(r,f.actors[0].name,2));
  }
});
function stats(){const ack=new Histogram(),hit=new Histogram();ack.add(5);hit.add(20);return{requests:20,launched:19,notLaunched:1,positive:13,damage:130,timeouts:0,heals:1,consumed:1,witnessFailures:0,ack,hit,hpMinimum:550,hpMaximum:674,latest:{at:60000,hp:674}};}
test('PvP actual minute rates cannot substitute requests or rounded60s for launched/positive',()=>{
  assert(pvpWindowResult(stats(),60.018,0).passed);
  const a=stats();a.launched=18;a.positive=12;assert.equal(pvpWindowResult(a,60.018,0).passed,false);
  const b=stats();b.requests=1000;b.launched=0;b.positive=0;assert.equal(pvpWindowResult(b,60,0).passed,false);
  const c=stats();c.witnessFailures=1;assert.equal(pvpWindowResult(c,60,0).passed,false);
  const d=stats();d.latest=null;assert.equal(pvpWindowResult(d,60,0).passed,false);
});

function engineFixture(edit={}){
  const f=fixture(2),rows=[],commands=[],clients=[];let at=10000,stopped=null;
  const context={fixture:f,writer:{add:r=>rows.push(r)},guard:{assert(){if(stopped)throw stopped},stop(reason){stopped??=new Error(reason)}}};
  const packet=(c,packet,payload)=>{const e={type:'packet',direction:'received',packet,payload,sequence:++c.sequence,monotonicMs:at+(edit.lateWitnessReceipts&&['ObjectStruck','ObjectHealth','DamageIndicator'].includes(packet)?2001:0)};c.events.push(e);if(packet==='HealthChanged'){c.pvpLastExactHpWall=at;context.onPvpExactHp?.(c,payload.hp,e);}recordPvpHealObservation(c,{type:'packet',packet,payload},e);return e;};
  for(let i=0;i<2;i++){
    let requests=0;
    const c={label:f.actors[i].name,context,stressIndex:i,stressOwnerId:1000,snapshot:raw(f,i),sequence:0,events:[],closed:false,
      async pace(){if(edit.onPace)edit.onPace(c);},
      nextAttackV2(objectId){return{type:'attackV2',protocolVersion:2,requestId:String(++requests),objectId}},
      async wait(predicate,label){const value=predicate();if(value){if(edit.lateAckDiscovery&&label==='correlated PvP V2 outcome')at+=2001;return value;}const error=Object.assign(new Error('fixture timeout'),{code:'CAPACITY_WAIT_TIMEOUT'});throw error;},
      send(command){commands.push({actorIndex:i,...command});if(command.type==='keepAlive')return;
        if(command.type==='clientVersion'){c.stressRaw=pvpStressRaw(c.snapshot,c.label,++c.sequence,at);c.pvpLastExactHpWall=at;context.onRaw?.(c);recordPvpHealObservation(c,{type:'worldSnapshot',payload:c.snapshot},{direction:'received',sequence:c.sequence,monotonicMs:at});return;}
        if(command.type==='useItem'){
          if(!edit.noConsumption)c.snapshot.inventoryItems[0].quantity--;if(!edit.noHealing)c.snapshot.playerHp=Math.min(674,c.snapshot.playerHp+110);
          packet(c,'UseItem',{uniqueId:2,grid:'Inventory',success:true});c.stressRaw=pvpStressRaw(c.snapshot,c.label,++c.sequence,at);c.pvpLastExactHpWall=at;context.onRaw?.(c);recordPvpHealObservation(c,{type:'worldSnapshot',payload:c.snapshot},{direction:'received',sequence:c.sequence,monotonicMs:at});return;
        }
        assert.equal(command.type,'attackV2');const peer=clients[1-i];
        if(!edit.noOutcome)c.events.push({type:'attackResultV2',direction:'received',sequence:++c.sequence,monotonicMs:at,protocolVersion:2,requestId:command.requestId,objectId:command.objectId,result:edit.unknown?'unknown':edit.notLaunched?'notLaunched':'launched',...(edit.unknown?{code:'outcomeUnknown'}:{})});
        if(edit.notLaunched||edit.unknown||edit.noOutcome)return;
        const damage=edit.damage??1;peer.snapshot.playerHp-=damage;packet(peer,'HealthChanged',{hp:peer.snapshot.playerHp,maxHp:674});
        if(edit.death)peer.snapshot.entities[1-i].dead=true;
        for(const receiver of edit.oneSided?[c]:clients){const target=receiver.snapshot.entities[1-i].objectId,attacker=receiver.snapshot.entities[i].objectId;
          packet(receiver,'ObjectStruck',{objectId:target,attackerId:attacker});packet(receiver,'ObjectHealth',{objectId:target,percent:Math.floor(peer.snapshot.playerHp*100/674)});packet(receiver,'DamageIndicator',{objectId:target,damage});}
      },
    };clients.push(c);
  }
  const services={now:()=>at,mono:()=>at,sleep:async ms=>{at+=ms}};
  return{f,context,clients,rows,commands,services,get stopped(){return stopped}};
}
test('PvP engine exact two-fighter minute uses receiver-local V2 results and real dual packet damage',async()=>{
  const f=engineFixture(),engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
  try{const stage=await engine.measure('stage-2',60,2);assert.equal(stage.controlledActors,2);assert.equal(stage.windows.length,1);
    for(const a of stage.windows[0].actors){assert(a.launched>0);assert(a.positive>0);assert(a.latestRaw?.hp>0);assert.equal(a.witnessFailures,0);}
    assert(f.commands.filter(c=>c.type==='attackV2').every(c=>c.objectId>=50000));
    assert(!f.commands.some(c=>c.type==='attack'||c.type.startsWith('qa')));
  }finally{await engine.stop();}
});
test('PvP notLaunched remains honest zero activity rather than launched from request count',async()=>{
  const f=engineFixture({notLaunched:true}),engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
  try{const stage=await engine.measure('stage-2',60,2);assert.equal(stage.passed,false);for(const row of stage.windows[0].actors){assert(row.requests>0);assert.equal(row.launched,0);assert.equal(row.positive,0);}}
  finally{await engine.stop();}
});
test('PvP missing outcome and unknown stop without legacy fallback or retry',async()=>{
  for(const edit of[{noOutcome:true},{unknown:true}]){const f=engineFixture(edit),engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
    await assert.rejects(engine.measure('stage-2',60,2));await engine.stop();assert(f.stopped);assert(f.commands.filter(c=>c.type==='attackV2').length<=2);assert(!f.commands.some(c=>c.type==='attack'));}
});
test('PvP death is terminal and never auto-revives',async()=>{
  const f=engineFixture({death:true}),engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
  await assert.rejects(engine.measure('stage-2',60,2));await engine.stop();assert(f.stopped);assert(!f.commands.some(c=>c.type==='townRevive'));
});
test('PvP held LargeHP recovery requires ACK plus an independent native effect and keeps scalar samples',async()=>{
  const f=engineFixture({damage:20}),engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
  try{await engine.measure('stage-2',60,2);assert(f.commands.some(c=>c.type==='useItem'));assert(f.rows.some(r=>r.type==='pvpHeal'&&r.before.hp<r.after.hp&&r.evidenceMs<=2000));assert(!JSON.stringify(f.rows).includes('password'));}
  finally{await engine.stop();}
});
test('PvP slot gates serialize persistence and bound startGame without losing releases',async()=>{
  const slots=pvpSlots(1),events=[];const release=await slots.acquire();events.push(1);
  const next=slots.acquire().then(r=>{events.push(2);r();});await Promise.resolve();assert.deepEqual(events,[1]);release();release();await next;assert.deepEqual(events,[1,2]);
  const r=await slots.acquire();r();
});
test('PvP auth600 is checked against actual public health not fixture declaration alone',()=>{
  const capacity=Object.fromEntries(['currentWsConnections','currentActiveSessions','currentReconnectLeases','currentLoginInFlight','currentNewCharacterInFlight','currentStartGameInFlight'].map(k=>[k,0]));
  const health={capacity,loadTestLogin:{testOnly:true,loginPeerLimit:600,loginDeviceLimit:600,windowSeconds:900}};
  assert.equal(validatePvpAuthHealth(health,600).loadTestLogin.loginPeerLimit,600);
  assert.throws(()=>validatePvpAuthHealth({capacity},600));assert.throws(()=>validatePvpAuthHealth({...health,loadTestLogin:{...health.loadTestLogin,testOnly:false}},600));
  assert.throws(()=>validatePvpAuthHealth({...health,capacity:{...capacity,currentActiveSessions:1}},600));
});

test('PvP pilot uses legacy omitted ceiling while500 explicitly opts in at real client construction',()=>{
  for(const count of[2,25,100,250,500]){
    const account={accountId:'ltpvp12345678',name:'ltpvp0000',password:'fixture-password-12'},policy=pvpStressClientPolicy(count);
    const context={policy,attackResults:'v2',pool:{accounts:[account]}};
    const client=new PvpStressClient('ws://127.0.0.1:17710/ws',account,context);
    assert(client.lifecycleNames.has(account.name));assert.equal(Object.hasOwn(policy,'targetActors'),count>100);
  }
});

test('PvP soft SLO remains terminal by default and diagnostic flag must be a literal boolean',()=>{
  const f=fixture(),config=pvpStressOptions(options(),f);assert.equal(config.diagnosticContinueOnSloFailure,false);
  for(const value of[1,'true',{},null]){assert.throws(()=>pvpStressOptions(options({diagnosticContinueOnSloFailure:value}),f));}
  const report={stages:[]},stage={name:'stage-10',completed:true,passed:false};
  assert.throws(()=>applyPvpStageResult(report,stage),/stage failed/);assert.equal(report.stages[0],stage);assert.equal(report.anyFailedStage,true);assert.equal(report.ok,false);
});
test('PvP explicit diagnostic continuation preserves failed10 and can collect25 without claiming success',()=>{
  const config=pvpStressOptions(options({stages:'10,25,50,100,250,500',diagnosticContinueOnSloFailure:true}),fixture());assert.equal(config.diagnosticContinueOnSloFailure,true);
  const report={stages:[],capacityAccepted:false,p0Accepted:false},failed={name:'stage-10',completed:true,passed:false},higher={name:'stage-25',completed:true,passed:true};
  applyPvpStageResult(report,failed,true);applyPvpStageResult(report,higher,true);
  assert.deepEqual(report.stages,[failed,higher]);assert.equal(failed.passed,false);assert.equal(report.anyFailedStage,true);assert.equal(report.ok,false);assert.equal(report.serverOnlyMeasuredProfilePassed,false);assert.equal(report.capacityAccepted,false);assert.equal(report.p0Accepted,false);
});

function displacedPlacement(blockAll=false){
  const f=fixture(2);f.actors[0].position={x:104,y:80};f.actors[1].position={x:105,y:80};
  const clients=[],sent=[];let at=10000;
  const context={fixture:f,guard:{assert(){}},writer:{add(){}}};
  for(let i=0;i<2;i++){
    const snapshot=raw(f,i);snapshot.entities[1].x=104;snapshot.entities[1].y=79;
    snapshot.entities.push({kind:'monster',objectId:206302,name:'Deer',disposition:'neutral',dead:false,hp:5,x:105,y:80});
    const c={context,label:f.actors[i].name,stressIndex:i,stressOwnerId:1000,snapshot,sequence:0,events:[],closed:false,
      record(){},async pace(){},async refresh(){this.send({type:'clientVersion'});return this.snapshot;},
      async wait(predicate){const value=predicate();if(!value)throw new Error('placement fixture missing response');return value;},
      send(command){sent.push({actorIndex:i,...command});
        if(['walk','run'].includes(command.type)){
          const d={Up:[0,-1],UpRight:[1,-1],Right:[1,0],DownRight:[1,1],Down:[0,1],DownLeft:[-1,1],Left:[-1,0],UpLeft:[-1,-1]}[command.direction],from=this.snapshot.entities[i],n=command.type==='run'?2:1,x=from.x+d[0]*n,y=from.y+d[1]*n;
          for(const other of clients)Object.assign(other.snapshot.entities[i],{x,y});this.events.push({type:'packet',direction:'received',packet:'UserLocation',payload:{x,y},sequence:++this.sequence,monotonicMs:at});
        }else {assert.equal(command.type,'clientVersion');this.events.push({type:'worldSnapshot',direction:'received',sequence:++this.sequence,monotonicMs:at});this.stressRaw=pvpStressRaw(this.snapshot,this.label,this.sequence,at);}
      },
    };clients.push(c);
  }
  return {f,clients,context,sent,resources:{map:{mapFileName:'0',width:128,height:128,type:100,blocked:new Uint8Array(128*128).fill(blockAll?1:0)},safeZones:[],npcs:[]},services:{now:()=>at,sleep:async ms=>{at+=ms}}};
}
test('PvP actual displaced10479 spawn and Deer-occupied10580 get one translated adjacent group by real navigator',async()=>{
  const f=displacedPlacement(),original=JSON.stringify(f.f);
  const row=await preparePvpGroup(f.clients,f.context,f.resources,f.services);
  assert(row.densityGeometryChanged);assert.equal(row.sourceFixtureRewritten,false);assert(row.physicalSteps>0&&row.physicalSteps<=32);assert(row.elapsedMs<30000);
  const a=f.context.pvpAssignments.get(0),b=f.context.pvpAssignments.get(1);assert.equal(Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y)),1);
  assert.notDeepEqual(b,{x:105,y:80});assert.equal(JSON.stringify(f.f),original);assert(f.sent.some(c=>['walk','run'].includes(c.type)));
  assert(!f.sent.some(c=>['moveTo','townRevive','qa.giveItem'].includes(c.type)));
});
test('PvP no legal local group placement fails without reservation, assignment or dangerous movement',async()=>{
  const f=displacedPlacement(true);await assert.rejects(preparePvpGroup(f.clients,f.context,f.resources,f.services),/No fresh walkable/);
  assert.equal(f.context.pvpAssignments,undefined);assert.equal(f.context.pvpReservedCells.size,0);assert(!f.sent.some(c=>['walk','run'].includes(c.type)));
});

test('PvP private first failure retains exact Error identity and only bounded error scalars, public stays redacted',async()=>{
  const writes=[],error=Object.assign(new Error('token=private-original-detail'),{code:'FIRST_REAL_CODE',packet:{password:'must-not-copy'},env:{secret:'must-not-copy'}});
  error.stack='stack '+('x'.repeat(20000));
  const recorder=createPvpFirstFailureRecorder(import.meta.dirname,{writeFile:async(file,body,options)=>writes.push({file,body,options})});
  assert.equal(await recorder(error,{phase:'stage-25',actorIndex:13}),error);
  assert.equal(await recorder(new Error('later monitor summary'),{phase:'main'}),error);assert.equal(recorder.firstError,error);assert.equal(writes.length,1);assert.equal(recorder.written,true);
  const data=JSON.parse(writes[0].body);assert.equal(data.actorIndex,13);assert.equal(data.error.code,'FIRST_REAL_CODE');assert.equal(data.error.message,error.message);
  assert(Buffer.byteLength(data.error.stack)<=8192);assert(!writes[0].body.includes('must-not-copy'));assert.equal(writes[0].options.flag,'wx');assert.equal(writes[0].options.mode,0o600);
  const publicReport={error:pvpPublicFailureMessage(error)};assert(!JSON.stringify(publicReport).includes('private-original-detail'));assert(!JSON.stringify(publicReport).includes('FIRST_REAL_CODE'));
});
test('PvP first failure I/O error never replaces original failure or permits overwrite',async()=>{
  const original=new Error('original'),io=new Error('private disk unavailable');let calls=0;
  const recorder=createPvpFirstFailureRecorder(import.meta.dirname,{writeFile:async()=>{calls++;throw io;}});
  assert.equal(await recorder(original,{phase:'worker',actorIndex:5}),original);assert.equal(recorder.firstError,original);assert.equal(recorder.writeFailed,true);assert.equal(recorder.written,false);
  assert.equal(await recorder(new Error('second')),original);assert.equal(calls,1);
});

function doseObservation(){
  const f=fixture(2),snapshot=raw(f),client={label:f.actors[0].name,snapshot,closed:false};snapshot.playerHp=500;
  client.pvpHealPending={cursor:10,sentAtMs:1000,deadlineMs:3000,ownerId:1000,ownerName:client.label,map:'0',item:{uniqueId:2,quantity:20,grid:'Inventory'},hpBefore:500};
  let sequence=10;const receive=(message,at=1100)=>recordPvpHealObservation(client,message,{direction:'received',sequence:++sequence,monotonicMs:at});
  const ack=()=>receive({type:'packet',packet:'UseItem',payload:{uniqueId:2,grid:'Inventory',success:true}});
  return {client,pending:client.pvpHealPending,receive,ack};
}
test('PvP native heal accepts actual consumption despite ongoing damage and no net HP growth',()=>{
  const f=doseObservation();f.ack();f.client.snapshot.inventoryItems[0].quantity=19;f.client.snapshot.playerHp=490;
  f.receive({type:'worldSnapshot',payload:f.client.snapshot});assert(pvpHealComplete(f.pending));assert.equal(f.pending.consumption.provenDelta,1);assert.equal(f.pending.hpIncrease,undefined);
});
test('PvP independent exact HP and ACK latch in either order without requiring same raw consumption',()=>{
  for(const hpFirst of[true,false]){const f=doseObservation(),hp=()=>f.receive({type:'packet',packet:'HealthChanged',payload:{hp:501,maxHp:674}});
    if(hpFirst){hp();assert.equal(pvpHealComplete(f.pending),false);f.ack();}else{f.ack();assert.equal(pvpHealComplete(f.pending),false);hp();}
    assert(pvpHealComplete(f.pending));assert.equal(f.pending.hpIncrease.firstExactHp,501);assert.equal(f.pending.consumption,undefined);
    f.receive({type:'packet',packet:'HealthChanged',payload:{hp:480,maxHp:674}});assert.equal(f.pending.hpIncrease.firstExactHp,501);
  }
});
test('PvP no ACK, expired evidence, percent-only or wrong UID cannot verify healing',()=>{
  const noAck=doseObservation();noAck.receive({type:'packet',packet:'HealthChanged',payload:{hp:501}});assert.equal(pvpHealComplete(noAck.pending),false);
  const expired=doseObservation();expired.ack();expired.receive({type:'packet',packet:'HealthChanged',payload:{hp:501}},3001);assert.equal(pvpHealComplete(expired.pending),false);
  const percent=doseObservation();percent.ack();percent.receive({type:'packet',packet:'ObjectHealth',payload:{objectId:1000,percent:99}});assert.equal(pvpHealComplete(percent.pending),false);assert.equal(percent.pending.percentCue,true);
  const wrong=doseObservation();wrong.receive({type:'packet',packet:'UseItem',payload:{uniqueId:3,grid:'Inventory',success:true}});wrong.receive({type:'packet',packet:'HealthChanged',payload:{hp:501}});assert.equal(pvpHealComplete(wrong.pending),false);
});
test('PvP normal native combat does not poll full snapshots every1.5s',async()=>{
  const f=engineFixture(),engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
  try{await engine.measure('stage-2',60,2);assert(f.commands.filter(c=>c.type==='clientVersion').length<=2,'native exact HealthChanged removes periodic full snapshot polling');}
  finally{await engine.stop();}
});

test('PvP actual wait final deadline read accepts only predeadline receipt despite late polling discovery',async t=>{
  let wall=10000;t.mock.method(Date,'now',()=>wall);
  const request={type:'attackV2',protocolVersion:2,requestId:'12',objectId:50001};
  const client={events:[],context:{guard:{assert(){}}},closed:false,wait:CapacityClient.prototype.wait};
  const waiting=waitPvpAttackOutcome(client,request,10,10000);
  const receipt={type:'attackResultV2',direction:'received',sequence:11,monotonicMs:11999,protocolVersion:2,requestId:'12',objectId:50001,result:'launched'};
  client.events.push(receipt);wall=12001;
  assert.equal(await waiting,receipt);assert.equal(receipt.monotonicMs-10000,1999);assert(wall-10000>2000);
});
test('PvP real engine credits predeadline dual witnesses even when ACK discovery resumes after deadline',async()=>{
  for(const lateWitnessReceipts of[false,true]){
    const f=engineFixture({lateAckDiscovery:true,lateWitnessReceipts}),engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
    await engine.stop();assert.equal(f.stopped,null);
    const totals=engine.totals();assert.equal(totals.length,2);
    for(const row of totals){assert.equal(row.requests,1);assert.equal(row.launched,1);assert.equal(row.positive,lateWitnessReceipts?0:1);assert.equal(row.attackAckLatency.maxMs,0);if(!lateWitnessReceipts)assert.equal(row.positiveHitLatency.maxMs,0);}
    assert(f.clients.every(c=>c.events.some(e=>e.packet==='ObjectStruck')));
  }
});
test('PvP final deadline read rejects late ACK, wrong request and receiver-local object',async t=>{
  let wall=10000;t.mock.method(Date,'now',()=>wall);
  const request={type:'attackV2',protocolVersion:2,requestId:'12',objectId:50001};
  for(const mismatch of[{monotonicMs:12001},{requestId:'13'},{objectId:1000}]){
    wall=10000;const client={events:[],context:{guard:{assert(){}}},closed:false,wait:CapacityClient.prototype.wait};
    const waiting=waitPvpAttackOutcome(client,request,10,10000);
    client.events.push({type:'attackResultV2',direction:'received',sequence:11,monotonicMs:11999,protocolVersion:2,requestId:'12',objectId:50001,result:'launched',...mismatch});wall=12001;
    await assert.rejects(waiting,e=>e.code==='CAPACITY_WAIT_TIMEOUT');
  }
});
test('PvP timeout alone emits at most eight anonymous outcome scalars without raw payload',async()=>{
  const f=engineFixture({noOutcome:true});
  for(const c of f.clients)for(let i=0;i<12;i++)c.events.push({type:'attackResultV2',direction:'received',sequence:++c.sequence,monotonicMs:9000+i,protocolVersion:2,requestId:String(i+1),objectId:50001,result:'notLaunched',payload:{password:'secret-token-do-not-copy'},name:'secret-token-do-not-copy'});
  const engine=createPvpStressEngine(f.context,f.services);engine.addGroup(f.clients);
  await assert.rejects(engine.measure('stage-2',60,2));await engine.stop();
  const diagnostics=f.rows.filter(r=>r.type==='pvpAttackOutcomeTimeout');assert.equal(diagnostics.length,2);
  for(const row of diagnostics){assert.equal(row.received.length,8);assert.equal(row.deadlineMonotonicMs-row.sendMonotonicMs,2000);assert(row.received.every(e=>e.sequence<=row.sendCursor));assert(!JSON.stringify(row).includes('secret-token'));assert(!JSON.stringify(row).includes('payload'));}
});
