// Independent server-only preconfigured PvP pressure. Never ordinary/P0 acceptance.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { performance, monitorEventLoopDelay } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';
import { ordinaryLogin, drain, matchAttackResultV2, loadStationaryNpcObstacles } from './playtest-capacity-soak.mjs';
import { PvpProbeClient, pvpHitEvidence } from './playtest-capacity-pvp-probe.mjs';
import { createNavigator, isTerminalNavigationNoPath } from './quest-agent/protocol-play.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable } from './quest-agent/protocol-navigation.mjs';
import { loadOrdinaryProtection } from './capacity-ordinary-protection.mjs';
import { LoginBudget, LOGIN_WINDOW_MS, CapacityMonitor, BoundedEvidence, Histogram } from './playtest-capacity-soak-core.mjs';
import { fetchResumeHealth, resumeReadinessState } from './playtest-native-resume-smoke.mjs';
import { outsideRepository, validateEndpoint, observedPlayerId } from './playtest-multiplayer-smoke.mjs';

const ownedErrors=new WeakSet();
const check=(ok,message)=>{if(!ok){const e=new Error(message);ownedErrors.add(e);throw e;}};
const integer=(n,a,b)=>Number.isSafeInteger(n)&&n>=a&&n<=b;
const pause=ms=>new Promise(r=>setTimeout(r,ms));
const distance=(a,b)=>Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y));
const self=c=>c.snapshot?.entities?.find(e=>e.objectId===c.snapshot.playerObjectId);
const publicVitals=r=>r?{sequence:r.sequence,at:r.at,map:r.map,x:r.x,y:r.y,hp:r.hp,maxHp:r.maxHp,hpStock:r.hpStock}:null;
const safeError=e=>ownedErrors.has(e)?e.message:'Underlying PvP transport, monitor or operation failed';

export { safeError as pvpPublicFailureMessage };
// One private receipt only. Never serialize packets, client/account objects or options.
export function createPvpFirstFailureRecorder(privateDirectory,{writeFile=fs.writeFile}={}){
  const file=path.join(privateDirectory,'pvp-stress-first-failure-v1.json');let claimed=false,original,pending,written=false,writeFailed=false;
  const text=(error,key,limit)=>{try{const value=error?.[key];if(typeof value!=='string'&&!(key==='code'&&typeof value==='number'&&Number.isFinite(value)))return null;const bytes=Buffer.from(String(value));return bytes.length<=limit?String(value):bytes.subarray(0,limit-3).toString('utf8');}catch{return '[unreadable Error field]';}};
  const capture=async(error,{phase,actorIndex}={})=>{
    if(!claimed){claimed=true;original=error;
      const row={schemaVersion:1,privateOnly:true,at:new Date().toISOString(),phase:typeof phase==='string'?phase.slice(0,80):null,actorIndex:integer(actorIndex,0,499)?actorIndex:null,
        error:{name:text(error,'name',80),code:text(error,'code',80),message:text(error,'message',2048),stack:text(error,'stack',8192)}};
      pending=(async()=>{const stat=await fs.lstat(privateDirectory);check(stat.isDirectory()&&!stat.isSymbolicLink()&&path.resolve(await fs.realpath(privateDirectory)).toLowerCase()===path.resolve(privateDirectory).toLowerCase(),'Private failure directory must already be canonical');
        await writeFile(file,JSON.stringify(row,null,2)+'\n',{flag:'wx',mode:0o600});written=true;
      })().catch(()=>{writeFailed=true;});
    }
    await pending;return original;
  };
  Object.defineProperties(capture,{firstError:{get:()=>original},written:{get:()=>written},writeFailed:{get:()=>writeFailed}});return capture;
}

export function pvpStressOptions(options,fixture) {
  check(options?.realm==='isolated','PvP stress requires explicit isolated realm');
  const endpoint=validateEndpoint(options.endpoint),count=Number(options.count??fixture?.count);
  check(integer(count,2,500),'PvP count must be2..500');
  const allowed=[2,10,25,50,100,250,500];
  const stages=String(options.stages??'2,10,25,50,100,250,500').split(',').map(Number);
  check(stages.length>0&&stages.every((n,i)=>allowed.includes(n)&&n<=count&&(!i||n>stages[i-1])),'Invalid ascending PvP ladder');
  const stageSeconds=Number(options.stageSeconds??300),soakSeconds=Number(options.soakSeconds??0),finiteWallSeconds=Number(options.finiteWallSeconds);
  check(stageSeconds===300||(stageSeconds===60&&options.pilot===true||stageSeconds===60&&options.pilot==='true'),'Only300s standard or explicit60s pilot');
  check(soakSeconds===0||soakSeconds===7200,'Two-hour soak must explicitly be7200');
  check(integer(finiteWallSeconds,60,86400)&&finiteWallSeconds>=stages.length*stageSeconds+soakSeconds+60,'Finite wall must include admission, measurement and drain');
  const diagnosticContinueOnSloFailure=options.diagnosticContinueOnSloFailure===undefined?false:options.diagnosticContinueOnSloFailure;
  check(typeof diagnosticContinueOnSloFailure==='boolean','Diagnostic continuation must be a literal boolean');
  const authBudget=Number(options.authBudget??30);
  check(authBudget===30||authBudget===600,'Only ordinary30 or explicit development600 authentication profile');
  if(authBudget===600)check(['127.0.0.1','localhost','[::1]'].includes(new URL(endpoint).hostname)&&options.developmentAuthProfile==='loopback-600-v1'&&
    fixture?.requiresGatewayProfile?.developmentLoopbackAuthBudget===600&&fixture?.requiresGatewayProfile?.authWindowSeconds===900,'600 auth budget requires explicit loopback development fixture/profile');
  check(fixture?.schemaVersion===1&&fixture.scope==='server-only-preconfigured-pvp'&&fixture.preseeded===true&&fixture.ordinaryEvidence===false&&
    fixture.mapFileName==='0'&&integer(fixture.count,count,500)&&Array.isArray(fixture.actors)&&fixture.actors.length===fixture.count&&Array.isArray(fixture.groups),'Exact server-only fixture required');
  check(fixture.hpItem?.templateIndex===662&&fixture.hpItem.name==='(HP)DrugLarge','Only declared ordinary LargeHP662 is supported');
  const points=new Set();
  for(let i=0;i<fixture.count;i++){
    const a=fixture.actors[i];check(a.poolIndex===i&&integer(a.groupIndex,0,499)&&integer(a.targetIndex,0,fixture.count-1)&&a.targetIndex!==i&&typeof a.name==='string'&&integer(a.characterIndex,0,0x7fffffff)&&
      integer(a.position?.x,0,65535)&&integer(a.position?.y,0,65535),'Invalid fixture actor tuple');
    const key=a.position.x+','+a.position.y;check(!points.has(key),'Duplicate fixture cell');points.add(key);
  }
  const grouped=new Set(),incoming=new Map();
  fixture.groups.forEach((g,i)=>{check(g.groupIndex===i&&['pair','triangle'].includes(g.type)&&Array.isArray(g.actorIndices)&&g.actorIndices.length===(g.type==='pair'?2:3),'Fixture group shape differs');
    for(const index of g.actorIndices){check(integer(index,0,fixture.count-1)&&!grouped.has(index),'Duplicate/missing group actor');grouped.add(index);const a=fixture.actors[index];
      check(a.groupIndex===g.groupIndex&&g.actorIndices.includes(a.targetIndex)&&distance(a.position,fixture.actors[a.targetIndex].position)===1,'Target must be adjacent inside its explicit group');
      incoming.set(a.targetIndex,(incoming.get(a.targetIndex)??0)+1);
    }
  });
  check(grouped.size===fixture.count&&fixture.actors.every((_,i)=>incoming.get(i)===1),'Every fighter requires exactly one incoming target edge');
  check(stages.every(n=>fixture.actors.slice(0,n).every(a=>a.targetIndex<n)&&fixture.groups.every(g=>g.actorIndices.every(i=>i<n)||g.actorIndices.every(i=>i>=n))),'Every measurement prefix must contain complete groups');
  const loginConcurrency=Number(options.loginConcurrency??4);check(integer(loginConcurrency,1,4),'Login concurrency must be1..4');
  return {...options,endpoint,count,stages,stageSeconds,soakSeconds,finiteWallSeconds,authBudget,loginConcurrency,diagnosticContinueOnSloFailure,fixture};
}

export function applyPvpStageResult(report,stage,diagnosticContinueOnSloFailure=false){
  report.stages.push(stage);
  if(!stage.passed){
    report.anyFailedStage=true;report.ok=false;report.serverOnlyMeasuredProfilePassed=false;
    check(diagnosticContinueOnSloFailure===true,'PvP stage failed actual per-fighter rolling rate/latency/witness gates');
  }
}

export function pvpStressClientPolicy(count){return {activityMode:'native',...(count>100?{targetActors:500}:{}),actionTimeoutMs:2000};}

// This opt-in budget is independent from the production30 and ordinary25/5.
// It never rewrites/prunes early, rotates addresses, or asserts server config.
export class DevelopmentPvpLoginBudget extends LoginBudget {
  delay(){this.prune();return Math.max(0,this.notBefore-this.clock(),this.attempts.length>=600?this.attempts[0].at+LOGIN_WINDOW_MS-this.clock():0);}
}
export function pvpStressRaw(raw,name,sequence,at=Date.now()) {
  const owner=raw?.entities?.find(e=>e.objectId===raw.playerObjectId&&e.name===name&&e.kind==='selfPlayer');
  check(owner&&owner.dead===false&&integer(owner.objectId,1,0xffffffff)&&integer(owner.x,0,65535)&&integer(owner.y,0,65535),'PvP exact live owner required');
  check(integer(raw.playerHp,1,0x7fffffff)&&integer(raw.playerMaxHp,1,0x7fffffff)&&raw.playerHp<=raw.playerMaxHp&&raw.inSafeZone===false,'PvP exact positive HP outside safe zone required');
  check(Array.isArray(raw.inventoryItems)&&Array.isArray(raw.beltItems),'PvP complete raw inventory and belt required');
  const ids=new Set(),items=[];
  for(const [grid,rows]of[['Inventory',raw.inventoryItems],['Belt',raw.beltItems]])for(const item of rows){
    const uid=typeof item?.uniqueId==='string'&&/^\d+$/.test(item.uniqueId)?Number(item.uniqueId):item?.uniqueId;
    check(integer(uid,0,Number.MAX_SAFE_INTEGER)&&!ids.has(uid)&&integer(item.quantity,1,0xffffffff),'PvP malformed/duplicate inventory item');ids.add(uid);
    const source=item.tooltipSource,values=[item.itemIndex,item.item_index,source?.info?.item_index,source?.info?.itemIndex,source?.realInfo?.item_index,source?.realInfo?.itemIndex].filter(x=>x!=null);
    if(item.name==='(HP)DrugLarge'||values.includes(662)){
      check(item.name==='(HP)DrugLarge'&&values.length>0&&values.every(x=>x===662)&&
        (grid==='Belt'?item.container==='belt':['bag1','bag2'].includes(item.container)),'PvP LargeHP identity/container conflict');
      items.push({uniqueId:uid,quantity:item.quantity,grid});
    }
  }
  const hpStock=items.reduce((n,i)=>n+i.quantity,0);check(Number.isSafeInteger(hpStock),'PvP HP stock overflow');
  return {sequence,at,ownerId:owner.objectId,map:raw.mapFileName,x:owner.x,y:owner.y,hp:raw.playerHp,maxHp:raw.playerMaxHp,hpStock,items:items.sort((a,b)=>a.uniqueId-b.uniqueId)};
}
export function pvpSlots(maximum) {
  let active=0;const waiting=[];
  return {async acquire(){if(active>=maximum)await new Promise(resolve=>waiting.push(resolve));else active++;
    let released=false;return()=>{if(released)return;released=true;if(waiting.length)waiting.shift()();else active--;};}};
}
export function validatePvpAuthHealth(health,authBudget) {
  const counters=resumeReadinessState(health);check(Object.values(counters).every(v=>v===0),'PvP starts only on an actually drained realm');
  if(authBudget===600){const p=health.loadTestLogin;check(p?.testOnly===true&&p.loginPeerLimit===600&&p.loginDeviceLimit===600&&p.windowSeconds===900,'Actual public health lacks explicit development600 auth profile');}
  return {capacity:counters,loadTestLogin:health.loadTestLogin??null};
}
// Independent received receipts implement the original native ACK && (consume || exact HP increase).
export function recordPvpHealObservation(client,message,receipt){
  const pending=client.pvpHealPending;
  if(!pending||receipt?.direction!=='received'||receipt.sequence<=pending.cursor||receipt.monotonicMs<pending.sentAtMs||receipt.monotonicMs>pending.deadlineMs)return;
  try{
    const own=self(client);check(own?.objectId===pending.ownerId&&own.name===pending.ownerName&&own.dead===false&&client.snapshot.mapFileName===pending.map&&!client.closed,'PvP healing owner/map/life changed');
    const proof={sequence:receipt.sequence,monotonicMs:receipt.monotonicMs,source:message.type==='worldSnapshot'?'worldSnapshot':message.packet};
    if(message.type==='error')check(false,'PvP healing received public operation error');
    if(message.type==='packet'&&message.packet==='UseItem'&&Number(message.payload?.uniqueId)===pending.item.uniqueId){
      check(message.payload.grid===pending.item.grid&&message.payload.success===true,'PvP held dose rejected or UID/grid differs');pending.ack??={...proof,uniqueId:pending.item.uniqueId,grid:pending.item.grid};
    }
    if(message.type==='worldSnapshot'){
      const raw=pvpStressRaw(message.payload,pending.ownerName,receipt.sequence);
      check(raw.ownerId===pending.ownerId&&raw.map===pending.map,'PvP healing raw owner changed');
      const selected=raw.items.find(i=>i.uniqueId===pending.item.uniqueId),present=[...message.payload.inventoryItems,...message.payload.beltItems].find(i=>Number(i.uniqueId)===pending.item.uniqueId);
      check(!present||selected,'PvP held UID template changed');
      const quantity=selected?.quantity??0;
      check(quantity===pending.item.quantity||quantity===pending.item.quantity-1,'PvP held dose UID quantity conflict');
      if(quantity===pending.item.quantity-1)pending.consumption??={...proof,uniqueId:pending.item.uniqueId,quantityBefore:pending.item.quantity,quantityAfter:quantity,provenDelta:1};
      if(raw.hp>pending.hpBefore)pending.hpIncrease??={...proof,hpBefore:pending.hpBefore,firstExactHp:raw.hp};
    }
    if(message.type==='packet'&&message.packet==='HealthChanged'&&(message.payload?.objectId==null||message.payload.objectId===pending.ownerId)&&integer(message.payload.hp,1,0x7fffffff)&&message.payload.hp>pending.hpBefore)
      pending.hpIncrease??={...proof,hpBefore:pending.hpBefore,firstExactHp:message.payload.hp};
    if(message.type==='packet'&&message.packet==='ObjectHealth'&&message.payload?.objectId===pending.ownerId&&integer(message.payload.percent,0,100))pending.percentCue=true;
  }catch(error){pending.failure??=error;}
}
export function pvpHealComplete(pending){if(pending.failure)throw pending.failure;return Boolean(pending.ack&&(pending.consumption||pending.hpIncrease));}

export function waitPvpAttackOutcome(client,request,cursor,sentAtMs){
  const matching=()=>matchAttackResultV2(client.events.filter(e=>e.direction==='received'&&e.sequence>cursor&&
    Number.isFinite(e.monotonicMs)&&e.monotonicMs>=sentAtMs&&e.monotonicMs<=sentAtMs+2000),request);
  return client.wait(matching,'correlated PvP V2 outcome',2000,matching);
}
function attackTimeoutDiagnostic(client,request,cursor,sentAtMs){
  const numeric=n=>Number.isFinite(n)?n:null,requestId=v=>typeof v==='string'&&/^[1-9][0-9]{0,19}$/.test(v)?v:null;
  const codes=new Set(['unsupported','executionUnavailable','authenticationRequired','requestIdNotFresh','outcomeUnknown','previousOutcomeUnknown']);
  return {type:'pvpAttackOutcomeTimeout',actorIndex:client.stressIndex,requestId:requestId(request.requestId),objectId:numeric(request.objectId),
    sendMonotonicMs:sentAtMs,deadlineMonotonicMs:sentAtMs+2000,sendCursor:cursor,
    received:client.events.filter(e=>e.direction==='received'&&e.type==='attackResultV2').slice(-8).map(e=>({
      sequence:numeric(e.sequence),monotonicMs:numeric(e.monotonicMs),protocolVersion:numeric(e.protocolVersion),requestId:requestId(e.requestId),objectId:numeric(e.objectId),
      result:['launched','notLaunched','unavailable','unknown'].includes(e.result)?e.result:null,code:e.code===undefined?null:codes.has(e.code)?e.code:'unrecognized'}))};
}

export class PvpStressClient extends PvpProbeClient {
  send(command){const count=command?.type==='clientVersion'&&this.inGame,reason=this.pvpSnapshotQueryReason??(this.draining?'drain':this.context.phase?.startsWith('admission-')?'preparation':'unclassified');
    const result=super.send(command);if(count){this.pvpSnapshotQueries??={};this.pvpSnapshotQueries[reason]=(this.pvpSnapshotQueries[reason]??0)+1;}return result;
  }
  async request(command,...rest){
    if(command?.type!=='startGame')return super.request(command,...rest);
    this.releasePvpAuthPersistence?.();this.releasePvpAuthPersistence=null;
    const release=await this.context.pvpStartSlots.acquire();
    try{return await super.request(command,...rest);}finally{release();}
  }
  observe(message){
    super.observe(message);
    if(message.type==='worldSnapshot'&&message.payload?.playerObjectId!=null){
      try{this.stressRaw=pvpStressRaw(message.payload,this.label,this.sequence);this.pvpLastExactHpWall=this.stressRaw.at;this.context.onRaw?.(this);}
      catch(error){this.failure??=error;}
    }
    if(message.type==='packet'&&message.packet==='HealthChanged'&&integer(message.payload?.hp,0,0x7fffffff)){this.pvpLastExactHpWall=Date.now();this.context.onPvpExactHp?.(this,message.payload.hp,this.events.at(-1));}
    recordPvpHealObservation(this,message,this.events.at(-1));
  }
}
// Only ephemeral runtime assignments move. The immutable fixture remains original.
export async function preparePvpGroup(group,context,resources,{now=Date.now,sleep=pause}={}){
  const began=now(),deadline=began+30000,reserved=context.pvpReservedCells??=new Set(),owned=[];
  const groupNames=new Set(group.map(c=>c.label)),origins=group.map(c=>({...context.fixture.actors[c.stressIndex].position}));
  let sentCells=0,fault=null,queue=Promise.resolve();
  const safeCell=p=>protocolMapCellIsWalkable(resources.map,p)&&!resources.safeZones.some(z=>Math.max(Math.abs(p.x-z.location.x),Math.abs(p.y-z.location.y))<=z.size)&&!resources.npcs.some(n=>distance(p,n)===0);
  function guard(){context.guard.assert();if(fault)throw fault;check(now()<deadline,'PvP group placement30s exhausted');
    for(const c of group){if(c.failure)throw c.failure;const own=self(c);check(!c.closed&&own?.dead===false&&own.objectId===c.stressOwnerId&&own.name===c.label&&c.snapshot.mapFileName===context.fixture.mapFileName&&c.snapshot.inSafeZone===false&&integer(c.snapshot.playerHp,1,0x7fffffff),'PvP group owner/map/life changed');}}
  const forbidden=p=>!safeCell(p)||group.some(c=>(c.snapshot.mapTransfers??[]).some(t=>{const b=t.bounds;return b&&p.x>=b.minX&&p.x<=b.maxX&&p.y>=b.minY&&p.y<=b.maxY;}));
  async function freshAll(){for(const c of group){guard();const after=c.sequence;await c.refresh();guard();check(c.stressRaw?.sequence>after,'PvP group needs fresh authoritative AOI');}}
  function candidate(offset){const points=origins.map(p=>({x:p.x+offset.x,y:p.y+offset.y}));
    return points.every((p,i)=>!forbidden(p)&&!reserved.has(p.x+','+p.y)&&group.every(c=>distance(self(c),p)<=16)&&
      group.every(c=>!c.snapshot.entities.some(e=>e.dead!==true&&distance(e,p)===0&&e.name!==group[i].label)));
  }
  const offsets=[];for(let y=-3;y<=3;y++)for(let x=-3;x<=3;x++)offsets.push({x,y});offsets.sort((a,b)=>Math.max(Math.abs(a.x),Math.abs(a.y))-Math.max(Math.abs(b.x),Math.abs(b.y))||a.y-b.y||a.x-b.x);
  const before=group.map(c=>({actorIndex:c.stressIndex,...publicVitals(c.stressRaw)}));
  try{
    await freshAll();
    for(const offset of offsets){
      guard();if(!candidate(offset))continue;
      const points=origins.map(p=>({x:p.x+offset.x,y:p.y+offset.y}));for(const point of points){const key=point.x+','+point.y;reserved.add(key);owned.push(key);}
      try{
        for(let index=0;index<group.length;index++){
          const c=group[index],target=points[index];if(distance(self(c),target)===0)continue;
          const verifyCells=cells=>{guard();check(cells.every(p=>!forbidden(p)&&!c.snapshot.entities.some(e=>e.objectId!==self(c).objectId&&e.dead!==true&&distance(e,p)===0)),'PvP group movement cell became occupied/forbidden');};
          const facade=new Proxy(c,{get(object,key){
            if(key==='send')return command=>{const from={x:self(c).x,y:self(c).y};queue=queue.then(async()=>{
              if(fault)throw fault;await c.pace();guard();
              if(['walk','run'].includes(command.type)){
                check(distance(self(c),from)===0,'PvP group movement origin changed during cadence');
                const dirs={Up:[0,-1],UpRight:[1,-1],Right:[1,0],DownRight:[1,1],Down:[0,1],DownLeft:[-1,1],Left:[-1,0],UpLeft:[-1,-1]},delta=dirs[command.direction],length=command.type==='run'?2:1;
                check(delta&&sentCells+length<=32,'PvP group32 physical step budget exhausted');
                verifyCells(Array.from({length},(_,i)=>({x:from.x+delta[0]*(i+1),y:from.y+delta[1]*(i+1)})));sentCells+=length;
              }else check(['clientVersion','keepAlive'].includes(command.type),'Unexpected preparation navigation command');
              c.send(command);
            }).catch(error=>{fault??=error;});};
            if(key==='wait')return async(predicate,label,timeout=2000)=>{await queue;guard();return c.wait(predicate,label,Math.max(1,Math.min(timeout,deadline-now())));};
            const value=Reflect.get(object,key);return typeof value==='function'?value.bind(object):value;
          }});
          const navigate=createNavigator(facade,{loadCollisionMap:async name=>{check(String(name)===context.fixture.mapFileName,'PvP group cannot change map');return resources.map;},movementResponseTimeoutMs:2000,now,delay:sleep});
          const result=await navigate(target,0,()=>{guard();return false;},{maxSuccessfulSteps:32-sentCells,maxAttempts:64,maxNoPathRefreshes:0,detectPositionCycles:true,beforeMovement:({physicalCells})=>{verifyCells(physicalCells);return null;}});
          await queue;guard();check(result.reached&&distance(self(c),target)===0,'PvP group normal path did not reach assignment');
        }
        await freshAll();
        for(let i=0;i<group.length;i++){const c=group[i],raw=c.stressRaw;check(raw&&distance(raw,points[i])===0&&raw.hp/raw.maxHp>=.85&&raw.hpStock>0,'PvP placed group final exact health/stock/position differs');}
        for(const c of group){const peer=group.find(p=>p.stressIndex===context.fixture.actors[c.stressIndex].targetIndex),id=observedPlayerId(c,peer.label),seen=c.snapshot.entities.find(e=>e.objectId===id&&e.name===peer.label&&e.kind==='player'&&e.dead===false);check(seen&&distance(self(c),seen)===1,'Placed PvP group lost adjacent witness');}
        context.pvpAssignments??=new Map();const assignments=group.map((c,i)=>({actorIndex:c.stressIndex,original:origins[i],actual:{...points[i]},rawSequence:c.stressRaw.sequence}));for(const row of assignments)context.pvpAssignments.set(row.actorIndex,row.actual);
        const evidence={groupIndex:context.fixture.actors[group[0].stressIndex].groupIndex,translation:offset,assignments,before,physicalSteps:sentCells,elapsedMs:now()-began,sourceFixtureRewritten:false,densityGeometryChanged:offset.x!==0||offset.y!==0};
        context.pvpPlacements??=[];context.pvpPlacements.push(evidence);context.writer.add({type:'pvpGroupPlacement',...evidence});return evidence;
      }catch(error){
        for(const key of owned.splice(0))reserved.delete(key);await queue;guard();
        if(!isTerminalNavigationNoPath(error))throw error;
        await freshAll();
      }
    }
    check(false,'No fresh walkable unoccupied non-safe PvP group translation within3 cells');
  }catch(error){for(const key of owned)reserved.delete(key);throw error;}
}

const newCounts=()=>({requests:0,launched:0,notLaunched:0,positive:0,damage:0,timeouts:0,heals:0,consumed:0,witnessFailures:0,ack:new Histogram(),hit:new Histogram(),hpMinimum:null,hpMaximum:null,latest:null});
export function pvpWindowResult(stats,seconds,actorIndex) {
  const launchedPerMinute=stats.launched*60/seconds,positivePerMinute=stats.positive*60/seconds;
  const timeoutRatio=stats.timeouts/Math.max(1,stats.requests);
  return {actorIndex,role:'fighter',seconds,requests:stats.requests,launched:stats.launched,notLaunched:stats.notLaunched,positive:stats.positive,positiveDamage:stats.damage,
    launchedPerMinute,positivePerMinute,heals:stats.heals,heldUnitsConsumed:stats.consumed,timeouts:stats.timeouts,timeoutRatio,witnessFailures:stats.witnessFailures,
    attackAckLatency:stats.ack.summary(),positiveHitLatency:stats.hit.summary(),hpMinimum:stats.hpMinimum,hpMaximum:stats.hpMaximum,latestRaw:stats.latest,
    passed:stats.hpMinimum>0&&stats.latest!==null&&launchedPerMinute>=18&&positivePerMinute>=12&&timeoutRatio<.01&&stats.witnessFailures===0&&stats.ack.count>0&&stats.ack.quantile(.95)<=900&&stats.hit.count>0&&stats.hit.quantile(.95)<=1000};
}

export function createPvpStressEngine(context,{now=Date.now,mono=()=>performance.now(),sleep=pause}={}) {
  const states=[],tasks=[],windowGraceMs=2100;let running=true,measurement=null,stoppingError=null;
  function guard(){context.guard.assert();if(stoppingError)throw stoppingError;}
  function bound(c){
    guard();if(c.failure)throw c.failure;const p=self(c),f=context.fixture.actors[c.stressIndex],assigned=context.pvpAssignments?.get(c.stressIndex)??f.position;
    check(!c.closed&&p?.dead===false&&p.name===c.label&&p.objectId===c.stressOwnerId&&c.snapshot.mapFileName===context.fixture.mapFileName&&c.snapshot.inSafeZone===false,'PvP owner/map/life/transport changed');
    check(p.x===assigned.x&&p.y===assigned.y,'Stationary pair position changed');
    check(integer(c.snapshot.playerHp,1,0x7fffffff)&&integer(c.snapshot.playerMaxHp,1,0x7fffffff)&&c.snapshot.playerHp/c.snapshot.playerMaxHp>.3,'PvP critical/unknown exact owner HP');
  }
  function counts(token){
    if(!token)return null;const {phase,at,index}=token,i=Math.min(Math.floor((at-phase.startedAt)/60000),phase.windowCount-1);
    if(i<0)return null;check(!phase.completed.some(w=>w.index===i),'Evidence completed after the immutable measurement window');if(!phase.windows.has(i))phase.windows.set(i,new Map());const rows=phase.windows.get(i);
    if(!rows.has(index))rows.set(index,newCounts());return rows.get(index);
  }
  function token(c,at=now()){return measurement?{phase:measurement,at,index:c.stressIndex}:null;}
  function record(c,action,t){
    c.stressTotals??=newCounts();
    for(const s of[c.stressTotals,counts(t)].filter(Boolean)){
      for(const k of['requests','launched','notLaunched','positive','damage','timeouts','heals','consumed','witnessFailures'])s[k]+=action[k]??0;
      if(action.ackMs!==undefined)s.ack.add(action.ackMs);if(action.hitMs!==undefined)s.hit.add(action.hitMs);
    }
  }
  context.onRaw=c=>{const row=counts(token(c));if(row){row.hpMinimum=Math.min(row.hpMinimum??Infinity,c.stressRaw.hp);row.hpMaximum=Math.max(row.hpMaximum??0,c.stressRaw.hp);row.latest=publicVitals(c.stressRaw);}};
  context.onPvpExactHp=(c,hp,receipt)=>{const row=counts(token(c));if(row){row.hpMinimum=Math.min(row.hpMinimum??Infinity,hp);row.hpMaximum=Math.max(row.hpMaximum??0,hp);row.latest={...publicVitals(c.stressRaw),sequence:receipt.sequence,at:now(),hp,hpSource:'HealthChanged',inventoryAsOfAt:c.stressRaw?.at??null};}};
  async function fresh(c,deadline=mono()+2000,reason='stale-watchdog'){
    await c.pace();bound(c);const cursor=c.sequence,remaining=Math.floor(deadline-mono());check(remaining>0,'PvP raw action evidence exceeded2s');
    c.pvpSnapshotQueryReason=reason;try{c.send({type:'clientVersion'});}finally{c.pvpSnapshotQueryReason=null;}
    await c.wait(()=>c.stressRaw?.sequence>cursor,'PvP fresh exact raw',remaining);bound(c);return structuredClone(c.stressRaw);
  }
  async function keepAlive(c){if(now()-(c.lastPvpKeepAlive??0)>=2000){await c.pace();bound(c);c.send({type:'keepAlive',time:now()});c.lastPvpKeepAlive=now();}}
  async function heal(c){
    // Selection may use the normal public cache. Only newly received raw/HP packets prove effect.
    const before=pvpStressRaw(c.snapshot,c.label,c.sequence,now());if(before.hp/before.maxHp>=.85)return false;
    if(now()-(c.lastPvpDose??-Infinity)<6000)return true;
    const item=before.items[0];check(item,'PvP held LargeHP supply exhausted');await c.pace();bound(c);
    const live=pvpStressRaw(c.snapshot,c.label,c.sequence,now()),selected=live.items.find(i=>i.uniqueId===item.uniqueId);
    check(selected&&selected.quantity===item.quantity&&selected.grid===item.grid,'PvP held dose selection changed during cadence');
    if(live.hp/live.maxHp>=.85)return false;
    const started=mono(),cursor=c.sequence,t=token(c),pending={cursor,sentAtMs:started,deadlineMs:started+2000,ownerId:live.ownerId,ownerName:c.label,map:live.map,item:{...item},hpBefore:live.hp};
    check(!c.pvpHealPending,'Only one PvP held dose may be pending');c.pvpHealPending=pending;
    try{
      c.send({type:'useItem',uniqueId:item.uniqueId,grid:item.grid});c.lastPvpDose=now();
      const ready=()=>{bound(c);return pvpHealComplete(pending);};
      await c.wait(()=>ready()||(pending.ack&&pending.percentCue&&!pending.cueQuerySent),'PvP native dose acknowledgement and independent effect',Math.max(1,pending.deadlineMs-mono()));
      if(!ready()){
        check(pending.ack&&pending.percentCue&&!pending.cueQuerySent,'PvP dose needs actual effect evidence');pending.cueQuerySent=true;
        await fresh(c,pending.deadlineMs,'heal-percent-cue');
        await c.wait(ready,'PvP dose after one percent cue',Math.max(1,pending.deadlineMs-mono()));
      }
      check(ready(),'PvP native healing not proved within2s');
      record(c,{heals:1,consumed:pending.consumption?.provenDelta??0},t);
      context.writer.add({type:'pvpHeal',at:now(),actorIndex:c.stressIndex,uniqueId:item.uniqueId,grid:item.grid,beforeSource:'current-public-cache-selection',before:publicVitals(live),after:publicVitals(c.stressRaw),
        acknowledgement:pending.ack,consumed:Boolean(pending.consumption),consumption:pending.consumption??null,hpIncreaseObserved:Boolean(pending.hpIncrease),hpIncrease:pending.hpIncrease??null,
        criterion:'ACK && (complete-raw UID consumption || first exact HP increase)',extraSnapshotQueries:pending.cueQuerySent?1:0,evidenceMs:mono()-started});return true;
    }finally{c.pvpHealPending=null;}
  }
  function peerView(c,peer){const id=observedPlayerId(c,peer.label),p=c.snapshot.entities.find(e=>e.objectId===id&&e.name===peer.label&&e.kind==='player'&&e.dead===false);check(p&&distance(self(c),p)===1,'PvP pair has no adjacent real witness');return id;}
  async function attack(c,peer){
    bound(c);bound(peer);const target=peerView(c,peer);peerView(peer,c);
    if(peer.snapshot.playerHp/peer.snapshot.playerMaxHp<.65)return;
    await c.pace();bound(c);bound(peer);check(peerView(c,peer)===target,'PvP target identity changed during cadence');
    const request=c.nextAttackV2(target),started=mono(),cursors=[c.sequence,peer.sequence],t=token(c);
    const targetIds=[target,self(peer).objectId],attackerIds=[self(c).objectId,peerView(peer,c)];
    c.send(request);record(c,{requests:1},t);
    try{
      const result=await waitPvpAttackOutcome(c,request,cursors[0],started);
      check(result.result==='launched'||result.result==='notLaunched','PvP V2 unavailable/unknown; no fallback or retry');
      check(result.monotonicMs>=started&&result.monotonicMs<=started+2000,'PvP correlated outcome arrived after2s');record(c,{[result.result]:1,ackMs:result.monotonicMs-started},t);
      if(result.result==='notLaunched')return;
      let hit=null;
      while(true){
        bound(c);bound(peer);
        const views=[c,peer].map((client,i)=>({events:client.events.filter(e=>e.sequence>cursors[i]&&e.monotonicMs>=started&&e.monotonicMs<=started+2000)}));
        hit=pvpHitEvidence(views,cursors,targetIds,attackerIds);if(hit||mono()>=started+2000)break;await sleep(20);
      }
      if(hit){
        const damageEvents=[c,peer].map((client,i)=>client.events.find(e=>e.sequence===hit.receipts[i].damage)),damage=Math.max(...damageEvents.map(e=>e?.payload?.damage??0));
        const at=Math.max(...[c,peer].flatMap((client,i)=>Object.values(hit.receipts[i]).map(seq=>client.events.find(e=>e.sequence===seq)?.monotonicMs??started)));
        const key=hit.receipts[0].struck;check(key!==(c.lastCreditedPvpStrike??-1),'Duplicate PvP positive strike');c.lastCreditedPvpStrike=key;
        record(c,{positive:1,damage,hitMs:at-started},t);
      }
    }catch(error){if(error.code==='CAPACITY_WAIT_TIMEOUT'){try{context.writer.add(attackTimeoutDiagnostic(c,request,cursors[0],started));}catch{}record(c,{timeouts:1},t);check(false,'PvP correlated outcome timeout; unknown execution is terminal');}throw error;}
  }
  async function worker(c,peer){
    try{while(running){bound(c);bound(peer);await keepAlive(c);
      if(now()-(c.pvpLastExactHpWall??c.stressRaw?.at??0)>30000){
        const maximum=context.maximumStaleWatchdogQueries??120;check((c.pvpWatchdogCount??0)<maximum&&now()-(c.pvpLastWatchdogAt??-Infinity)>=30000,'PvP stale exact-HP watchdog budget exhausted');
        c.pvpWatchdogCount=(c.pvpWatchdogCount??0)+1;c.pvpLastWatchdogAt=now();await fresh(c,mono()+2000,'stale-watchdog');
      }
      const exact=c.snapshot.playerHp/c.snapshot.playerMaxHp;
      if(exact<.85){await heal(c);await sleep(100);continue;}
      if(now()<(c.nextPvpAttackAt??0)){await sleep(Math.min(100,c.nextPvpAttackAt-now()));continue;}
      c.nextPvpAttackAt=now()+900;await attack(c,peer);
    }}catch(error){stoppingError??=error;try{await context.recordFirstFailure?.(error,{phase:measurement?.name??context.phase,actorIndex:c.stressIndex});}catch{}context.guard.stop(safeError(error));}}
  function addGroup(group){for(const c of group){c.pvpActiveStartedAt=now();states.push(c);}for(const c of group){const peer=group.find(p=>p.stressIndex===context.fixture.actors[c.stressIndex].targetIndex);check(peer,'Target missing from admitted complete group');tasks.push(worker(c,peer));}}
  function closeWindow(phase,index,endAt){
    const rows=phase.windows.get(index)??new Map(),seconds=(endAt-(phase.startedAt+index*60000))/1000;
    const actors=Array.from({length:phase.count},(_,i)=>pvpWindowResult(rows.get(i)??newCounts(),seconds,i));
    const row={type:'pvpWindow',phase:phase.name,index,startedAt:phase.startedAt+index*60000,endedAt:endAt,seconds,actors,passed:actors.every(a=>a.passed)};
    phase.completed.push(row);phase.windows.delete(index);context.writer.add(row);
  }
  async function measure(name,seconds,count){
    guard();const phase={name,count,startedAt:now(),windowCount:Math.ceil(seconds/60),windows:new Map(),completed:[]};measurement=phase;
    try{while(now()-phase.startedAt<seconds*1000){guard();await sleep(100);const elapsed=now()-phase.startedAt;
      while(phase.completed.length<phase.windowCount-1&&elapsed>=(phase.completed.length+1)*60000+windowGraceMs)closeWindow(phase,phase.completed.length,phase.startedAt+(phase.completed.length+1)*60000);
    }
      const endedAt=now();measurement=null;await sleep(windowGraceMs);guard();
      while(phase.completed.length<phase.windowCount){const i=phase.completed.length;closeWindow(phase,i,i===phase.windowCount-1?endedAt:phase.startedAt+(i+1)*60000);}
      return {name,controlledActors:count,fighters:count,completed:true,seconds,measuredSeconds:(endedAt-phase.startedAt)/1000,windows:phase.completed,passed:phase.completed.every(w=>w.passed)};
    }finally{measurement=null;}
  }
  return {addGroup,measure,guard,states,async stop(){running=false;await Promise.allSettled(tasks);},totals:()=>states.map(c=>({actorIndex:c.stressIndex,activeElapsedMs:now()-c.pvpActiveStartedAt,...pvpWindowResult(c.stressTotals??newCounts(),Math.max(.001,(now()-c.pvpActiveStartedAt)/1000),c.stressIndex)}))};
}

async function readJson(file){check(typeof file==='string'&&path.isAbsolute(file),'Absolute input path required');const st=await fs.lstat(file);check(st.isFile()&&!st.isSymbolicLink(),'Regular canonical input required');return JSON.parse(await fs.readFile(file,'utf8'));}
function stressPool(pool,config){
  check(pool?.schemaVersion===1&&pool.purpose==='isolated-load-actors'&&pool.realm==='isolated'&&pool.endpoint===config.endpoint&&Array.isArray(pool.accounts)&&pool.accounts.length===config.fixture.count,'Dedicated server-only pool differs');
  const names=new Set(),ids=new Set();pool.accounts.forEach((a,i)=>{const f=config.fixture.actors[i];check(/^lt[a-z0-9]{5,18}$/i.test(a.accountId??'')&&typeof a.password==='string'&&a.password.length>=12&&a.password.length<=20&&a.state==='ready'&&a.provisioning&&a.name===f.name&&a.characterIndex===f.characterIndex,'Only dedicated saved fixture actors may login');check(!names.has(a.name)&&!ids.has(a.accountId),'Duplicate fixture identity');names.add(a.name);ids.add(a.accountId);});return pool;
}
export async function runPvpStress(options){
  const fixture=options.fixture??await readJson(options.fixtureFile),config=pvpStressOptions(options,fixture),accountFile=outsideRepository(config.poolFile),output=outsideRepository(config.output);
  check(path.basename(path.dirname(accountFile))==='private','Outside-repository private pool required');await fs.mkdir(output,{recursive:true});
  const runId=Date.now()+'-'+crypto.randomBytes(4).toString('hex'),reportPath=path.join(output,runId+'.pvp-stress-report.json'),samplesPath=path.join(output,runId+'.pvp-stress-samples.jsonl');
  const writer=new BoundedEvidence(samplesPath),guard=new CapacityMonitor(config.monitor,writer),report={schemaVersion:1,runId,reportPath,samplesPath,startedAt:new Date().toISOString(),
    scope:'server-only-preconfigured-all-fighter-pvp',preseeded:true,ordinaryEvidence:false,ordinaryP0Qualification:false,capacityAccepted:false,p0Accepted:false,
    attackResults:'v2',legacyFallback:false,diagnosticContinuation:config.diagnosticContinueOnSloFailure,anyFailedStage:false,count:config.count,stagesRequested:config.stages,stageSeconds:config.stageSeconds,soakSeconds:config.soakSeconds,finiteWallSeconds:config.finiteWallSeconds,
    loginConcurrency:config.loginConcurrency,startGameConcurrency:2,authAndPoolWritesSerialized:true,stationaryGroups:true,movingPvpTested:false,hotspotTested:false,clientAuthBudget:config.authBudget,serverAuthProfileVerification:'Root owned wrapper responsibility; fixture declaration alone is not runtime proof',stages:[],cleanupErrors:[]};
  const context={endpoint:config.endpoint,policy:pvpStressClientPolicy(config.count),attackResults:'v2',phase:'server-only-pvp',fixture,writer:{add(){}},guard,accountFile,
    ledgerFile:path.join(path.dirname(accountFile),'capacity-auth-budget.json'),pvpStartSlots:pvpSlots(2),clients:[],allClients:[],cleanupErrors:report.cleanupErrors,add(){}};
  // Driver-owned output uses only explicit anonymous scalar rows.
  context.recordFirstFailure=createPvpFirstFailureRecorder(path.dirname(accountFile));
  const engineContext={...context,writer,maximumStaleWatchdogQueries:Math.ceil(config.finiteWallSeconds/30)};context.onRaw=c=>engineContext.onRaw?.(c);context.onPvpExactHp=(c,hp,receipt)=>engineContext.onPvpExactHp?.(c,hp,receipt);let engine,lock,wallTimer,lagTimer;const lag=monitorEventLoopDelay({resolution:20});let lagConsecutive=0;
  const stop=()=>guard.stop('Operator requested PvP drain'),deadline=Date.now()+config.finiteWallSeconds*1000;
  process.on('SIGINT',stop);process.on('SIGTERM',stop);
  async function drainOne(c){if(!c.ws){c.pvpNotAdmitted=true;return {actorIndex:c.stressIndex,notAdmitted:true,normalLogout:false};}const errors=report.cleanupErrors.length,cursor=c.sequence,saved=await drain(c,context);check(saved&&c.closed&&!c.inGame&&report.cleanupErrors.length===errors&&c.events.some(e=>e.direction==='received'&&e.sequence>cursor&&e.packet==='LogOutSuccess'),'PvP normal logout/save not acknowledged');return {actorIndex:c.stressIndex,normalLogout:true,final:publicVitals(c.stressRaw)};}
  try{
    lock=await fs.open(path.join(path.dirname(accountFile),'capacity-run.lock'),'wx',0o600);await lock.writeFile(JSON.stringify({runId,pid:process.pid,scope:report.scope}));
    context.pool=stressPool(await readJson(accountFile),config);engineContext.pool=context.pool;
    let ledger;try{ledger=await readJson(context.ledgerFile)}catch(e){if(e.code!=='ENOENT')throw e;}
    if(ledger){check(ledger.schemaVersion===1&&ledger.purpose==='capacity-auth-budget'&&ledger.endpoint===config.endpoint,'Existing auth ledger endpoint/schema differs');check(options.loginNotBefore===undefined,'Existing ledger cannot be overridden');}
    const notBefore=ledger?0:options.loginNotBefore===undefined?Date.now()+LOGIN_WINDOW_MS:Date.parse(options.loginNotBefore);check(Number.isFinite(notBefore),'Invalid explicit auth origin');
    context.loginBudget=config.authBudget===600?new DevelopmentPvpLoginBudget(ledger?.attempts??[],notBefore):new LoginBudget(ledger?.attempts??[],notBefore);
    wallTimer=setTimeout(()=>guard.stop('Finite PvP wall exhausted including admission and drain'),config.finiteWallSeconds*1000);
    report.healthBefore=validatePvpAuthHealth(await fetchResumeHealth(config.endpoint),config.authBudget);
    await guard.start();lag.enable();lagTimer=setInterval(()=>{const p99=lag.percentile(99)/1e6;lag.reset();lagConsecutive=p99>250?lagConsecutive+1:0;writer.add({type:'driverLag',at:Date.now(),p99Ms:p99,consecutiveOver250:lagConsecutive});if(lagConsecutive>=3)guard.stop('Driver lag exceeded250ms for3 samples');},1000);
    const [placementMap,protection,placementNpcs]=await Promise.all([loadProtocolCollisionMap(fixture.mapFileName),loadOrdinaryProtection({source:{runtimeProfileId:'platinum_176'}},fixture.mapFileName),loadStationaryNpcObstacles(fixture.mapFileName)]);
    const placementResources={map:placementMap,safeZones:protection.safeZones,npcs:placementNpcs};
    engine=createPvpStressEngine(engineContext);
    for(const admissionTarget of config.stages){
      const actualCount=admissionTarget;context.phase=engineContext.phase='admission-'+actualCount;
      const admissionStartedAt=Date.now(),first=context.clients.length,admissionSlots=pvpSlots(config.loginConcurrency),authPersistence=pvpSlots(1);
      const groups=fixture.groups.filter(g=>g.actorIndices[0]>=first&&g.actorIndices[0]<actualCount);
      for(let i=first;i<actualCount;i++){const c=new PvpStressClient(config.endpoint,context.pool.accounts[i],context);c.stressIndex=i;context.clients.push(c);context.allClients.push(c);}
      const results=await Promise.allSettled(groups.map(async group=>{
        const members=await Promise.allSettled(group.actorIndices.map(async i=>{
          const releaseAdmission=await admissionSlots.acquire();let releaseAuth;
          try{
            engine.guard();const c=context.clients[i];releaseAuth=await authPersistence.acquire();engine.guard();c.releasePvpAuthPersistence=releaseAuth;
            await ordinaryLogin(c,context);engine.guard();
            c.stressOwnerId=self(c)?.objectId;await c.refresh();const raw=c.stressRaw,f=fixture.actors[i];check(raw&&raw.map===fixture.mapFileName&&distance(raw,f.position)<=4&&raw.hp/raw.maxHp>=.85&&raw.hpStock>0,'Fresh fixture position/health/held inventory differs');
            const mode=await c.request({type:'changeAMode',mode:5},'ChangeAMode',2000);check(mode.payload?.mode===5,'PvP attack mode not acknowledged');
          }catch(error){await context.recordFirstFailure(error,{phase:context.phase,actorIndex:i});guard.stop(safeError(error));throw error;}finally{releaseAuth?.();releaseAdmission();}
        }));
        const memberFailure=members.find(r=>r.status==='rejected');if(memberFailure)throw memberFailure.reason;
        engine.guard();await preparePvpGroup(group.actorIndices.map(i=>context.clients[i]),engineContext,placementResources);
        engine.addGroup(group.actorIndices.map(i=>context.clients[i]));
      }));
      const failure=results.find(r=>r.status==='rejected');if(failure)throw failure.reason;
      report.admissions??=[];report.admissions.push({target:actualCount,newActors:actualCount-first,elapsedMs:Date.now()-admissionStartedAt});
      context.phase=engineContext.phase='stage-'+admissionTarget;
      const stage=await engine.measure('stage-'+admissionTarget,config.stageSeconds,actualCount);stage.requestedAdmissionTarget=admissionTarget;stage.actualControlledActors=actualCount;applyPvpStageResult(report,stage,config.diagnosticContinueOnSloFailure);
    }
    if(config.soakSeconds)report.soak=await engine.measure('explicit-two-hour-soak',7200,context.clients.length);
    check(!report.soak||report.soak.passed,'PvP explicit soak failed');report.measurementsCompleted=true;report.ok=!report.anyFailedStage;
  }catch(error){await context.recordFirstFailure(error,{phase:context.phase});report.ok=false;report.error=safeError(error);}
  finally{
    await engine?.stop();clearInterval(lagTimer);lag.disable();report.totals=engine?.totals()??[];report.groupPlacements=engineContext.pvpPlacements??[];report.extraSnapshotQueries=context.allClients.map(c=>({actorIndex:c.stressIndex,byReason:c.pvpSnapshotQueries??{},staleWatchdogs:c.pvpWatchdogCount??0}));report.normalDrains=[];
    const clients=context.allClients.filter(c=>!c.closed);let cursor=0;
    await Promise.all(Array.from({length:Math.min(8,clients.length)},async()=>{for(;;){const i=cursor++;if(i>=clients.length)return;try{report.normalDrains.push(await drainOne(clients[i]));}catch{report.cleanupErrors.push({actorIndex:clients[i].stressIndex,step:'normal-drain-incomplete'});}}}));
    clearTimeout(wallTimer);guard.close();process.off('SIGINT',stop);process.off('SIGTERM',stop);
    if(lock)try{await lock.close();await fs.unlink(path.join(path.dirname(accountFile),'capacity-run.lock'));}catch{report.cleanupErrors.push({step:'lock-release'});}
    try{await writer.close();}catch{report.cleanupErrors.push({step:'evidence-close'});}
  }
  report.privateFirstFailureRecorded=context.recordFirstFailure.written;report.privateFirstFailureWriteFailed=context.recordFirstFailure.writeFailed;
  report.finishedAt=new Date().toISOString();report.totalElapsedMs=Date.parse(report.finishedAt)-Date.parse(report.startedAt);report.finiteWallExceeded=Date.now()>deadline;
  report.cleanupComplete=report.cleanupErrors.length===0&&context.allClients.every(c=>c.pvpNotAdmitted===true||c.closed&&!c.inGame);report.resourceMonitorStopped=guard.stopped;report.resourceStopReason=guard.reason??null;
  report.ok=report.ok===true&&!report.anyFailedStage&&report.cleanupComplete&&!report.finiteWallExceeded;report.serverOnlyMeasuredProfilePassed=report.ok;
  await fs.writeFile(reportPath,JSON.stringify(report,null,2)+'\n',{flag:'wx'});return report;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){const args=process.argv.slice(2),options={};for(let i=0;i<args.length;i+=2){check(/^--[A-Za-z]+$/.test(args[i])&&args[i+1]!==undefined,'Named CLI pairs required');const key=args[i].slice(2);if(key==='diagnosticContinueOnSloFailure'){check(['true','false'].includes(args[i+1]),'Diagnostic CLI flag must be true or false');options[key]=args[i+1]==='true';}else options[key]=args[i+1];}const r=await runPvpStress(options);console.log(JSON.stringify({ok:r.ok,reportPath:r.reportPath,samplesPath:r.samplesPath,capacityAccepted:false,p0Accepted:false}));process.exitCode=r.ok?0:1;}
