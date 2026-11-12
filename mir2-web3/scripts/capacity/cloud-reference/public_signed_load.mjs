// Bounded ordinary-player mixed-transport calibration. No idle CCU qualification.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { pathToFileURL } from 'node:url';
import * as probe from './public_signed_probe.mjs';
import { migrateGuestPool } from './public_sui_wallet_auth.mjs';
import { outsideRepository, observedPlayerId, redact } from '../../../apps/web/scripts/playtest-multiplayer-smoke.mjs';
import { LoginBudget, LOGIN_WINDOW_MS, CapacityMonitor, BoundedEvidence, verifiedCombatHit } from '../../../apps/web/scripts/playtest-capacity-soak-core.mjs';
import { loadProtocolCollisionMap, protocolMapCellIsWalkable } from '../../../apps/web/scripts/quest-agent/protocol-navigation.mjs';
import { equipStarterGear } from '../../../apps/web/scripts/quest-agent/protocol-play.mjs';

const mono = () => performance.now();
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
const utc = () => new Date().toISOString();
const check = (condition, message) => { if (!condition) throw new Error(message); };
const self = c => c.snapshot?.entities?.find(e => e.objectId === c.snapshot.playerObjectId);
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const STEPS = [[0,-1,'Up'],[1,-1,'UpRight'],[1,0,'Right'],[1,1,'DownRight'],[0,1,'Down'],[-1,1,'DownLeft'],[-1,0,'Left'],[-1,-1,'UpLeft']];
const WEAK = new Set(['Hen', 'Deer', 'Scarecrow']);
const percentile = values => { const v = values.slice().sort((a,b)=>a-b); return v.length ? v[Math.ceil(v.length*.95)-1] : null; };

export function fixedDue(next, interval, now) {
  const due = [];
  while (next <= now) { due.push(next); next += interval; check(due.length <= 1000, 'Fixed schedule backlog invalidates driver'); }
  return { due, next };
}
export const movementWitnessLatency = (sentAt, ownerAck, peerAck) => Math.max(ownerAck.monotonicMs, peerAck.monotonicMs) - sentAt;
export function fighterForProfile(profile, index) {
  check(['mixed-combat','movement-chat'].includes(profile),'Unknown declared workload profile');
  return profile === 'mixed-combat' && index % 5 === 0;
}
export async function closeVerification(client, result, safe = value => value) {
  client.draining = true;
  try { await client.close(); result.normalLogout = client.closed && !client.inGame; }
  catch (error) { result.normalLogout = false; result.closeFailure = safe(error.message); }
  result.passed = result.statePreserved === true && result.normalLogout === true;
  return result;
}
export function stageApplicationTraffic(before, after, elapsedMs) {
  check(Number.isFinite(elapsedMs) && elapsedMs > 0, 'Application traffic requires a real positive measurement span');
  const actors = before.map(b => {
    const a=after.find(a=>a.actorOrdinal===b.actorOrdinal);
    check(a && a.sent>=b.sent && a.received>=b.received,'Application byte counter missing or decreased');
    const sent=a.sent-b.sent,received=a.received-b.received;
    return {actorOrdinal:b.actorOrdinal,profile:b.profile,sentBytes:sent,receivedBytes:received,
      sentMbps:sent*8/(elapsedMs*1000),receivedMbps:received*8/(elapsedMs*1000)};
  });
  const profiles=[...new Set(actors.map(a=>a.profile))].map(profile=>({profile,actors:actors.filter(a=>a.profile===profile).length,
    sentBytes:actors.filter(a=>a.profile===profile).reduce((sum,a)=>sum+a.sentBytes,0),
    receivedBytes:actors.filter(a=>a.profile===profile).reduce((sum,a)=>sum+a.receivedBytes,0)}));
  for(const p of profiles){p.sentMbps=p.sentBytes*8/(elapsedMs*1000);p.receivedMbps=p.receivedBytes*8/(elapsedMs*1000);}
  const sentBytes=actors.reduce((sum,a)=>sum+a.sentBytes,0),receivedBytes=actors.reduce((sum,a)=>sum+a.receivedBytes,0);
  return {elapsedMs,sentBytes,receivedBytes,sentMbps:sentBytes*8/(elapsedMs*1000),receivedMbps:receivedBytes*8/(elapsedMs*1000),actors,profiles,
    directionDefinition:'sent=client to Gateway; received=Gateway to client',
    scope:'Game WebSocket application payload including catalog-gzip headers; excludes HTTP authentication, WebSocket/TLS/TCP/IP framing, VPN and other services'};
}
export function assessActorMinute(rows, fighter, seconds = 60) {
  const offered = rows.length, completed = rows.filter(r => r.completed).length;
  const moves = rows.filter(r => r.kind === 'walk' && r.completed).length;
  const chats = rows.filter(r => r.kind === 'chat' && r.completed).length;
  const launched = rows.filter(r => r.kind === 'attack' && r.launched).length;
  const positive = rows.filter(r => r.kind === 'attack' && r.positive).length;
  const timeouts = rows.filter(r => r.timeout).length;
  const p95 = kind => percentile(rows.filter(r => r.kind === kind && r.completed).map(r => r.latencyMs));
  const scheduleP95Ms = percentile(rows.map(r => r.latenessMs));
  const hpP95Ms = percentile(rows.filter(r => r.positive).map(r => r.hpLatencyMs));
  const moveOffered = rows.filter(r => r.kind === 'walk').length;
  const attacksOffered = rows.filter(r => r.kind === 'attack').length;
  const passed = offered > 0 && completed / offered >= .95 && moveOffered >= Math.floor(seconds / 2) &&
    moves >= moveOffered * .95 && chats >= Math.floor(seconds / 30) && timeouts / offered < .01 &&
    p95('walk') !== null && p95('walk') <= 750 && p95('chat') !== null && p95('chat') <= 1000 &&
    scheduleP95Ms !== null && scheduleP95Ms <= 250 && (!fighter || attacksOffered >= Math.floor(seconds / 2) && launched >= attacksOffered * .95 && launched * 60 / seconds >= 18 &&
      positive * 60 / seconds >= 12 && p95('attack') !== null && p95('attack') <= 900 && hpP95Ms !== null && hpP95Ms <= 1000);
  return { passed, offered, completed, completionRatio: offered ? completed/offered : 0, moves, moveOffered, attacksOffered,
    chats, launched, positive, timeouts, movementP95Ms: p95('walk'), chatP95Ms: p95('chat'),
    launchP95Ms: p95('attack'), positiveHpP95Ms: hpP95Ms, scheduleP95Ms };
}
export function campaignMode(mode = 'strict', profile = 'mixed-combat') {
  check(['strict','explore'].includes(mode),'Campaign mode must be strict or explore');
  check(mode !== 'explore' || profile === 'movement-chat','Explore campaign requires the movement-chat profile');
  return mode;
}
export function workloadAdequacy(results, cohort = results) {
  const offered=cohort.reduce((sum,r)=>sum+r.offered,0),completed=cohort.reduce((sum,r)=>sum+r.completed,0);
  return {workloadAdequate:results.length>0&&offered>0&&completed/offered>=.95&&results.every(r=>
    r.offered>=32&&r.moves>=20&&r.chats>=1&&r.completionRatio>=.70&&
    Number.isFinite(r.scheduleP95Ms)&&r.scheduleP95Ms<=250),
    cohortOffered:offered,cohortCompleted:completed,cohortCompletionRatio:offered?completed/offered:0};
}
export function campaignMayContinue(mode, result) {
  return result.failure == null && (mode === 'explore' ? result.workloadAdequate === true : result.passed === true);
}
export function campaignAllSloPassed(completed, stickySloPassed, stages, admissionSloMisses = []) {
  return completed === true && stickySloPassed === true && admissionSloMisses.length === 0 && stages.length > 0 && stages.every(s=>s.passed === true);
}
export function campaignSloFailure(activitySloPassed, stages, admissionSloMisses = []) {
  return admissionSloMisses.length > 0 && activitySloPassed === true && stages.every(s=>s.passed === true)
    ? {phase:'admission-SLO',category:'admissionSLO',message:'Campaign completed after missed original bootstrap admission deadlines; diagnostic observation is not a stable capacity qualification'}
    : {phase:'campaign',category:'activitySLO',message:'Campaign completed with failed strict SLO windows; exploration is not a stable capacity qualification'};
}
export function lateBootstrapObservation(value = 'off', mode = 'strict') {
  check(['on','off'].includes(value),'Observe late bootstrap must be on or off');
  check(value !== 'on' || mode === 'explore','Late bootstrap observation requires campaign-mode explore');
  return value === 'on';
}
export function optionsFrom(args) {
  const o = {};
  const allowed = new Set(['web-base','endpoint','private-directory','output','monitor','stages','stage-seconds','top-seconds','wall-seconds','map-root','profile','max-evidence-rows','campaign-mode','equipment-roundtrip-actors','observe-late-bootstrap']);
  for (let i=0;i<args.length;i+=2) { const k=args[i]?.slice(2); check(args[i]?.startsWith('--') && allowed.has(k) && args[i+1]!==undefined && o[k]===undefined,'Use unique --name value options'); o[k]=args[i+1]; }
  o['web-base'] ??= 'https://numeron.obelisk.build'; o.endpoint ??= 'wss://165.154.65.136.sslip.io/playtest/ws';
  o.profile ??= 'mixed-combat';fighterForProfile(o.profile,0);
  o.campaignMode=campaignMode(o['campaign-mode'],o.profile);
  o.observeLateBootstrap=lateBootstrapObservation(o['observe-late-bootstrap'],o.campaignMode);
  o.stages = String(o.stages ?? '2').split(',').map(Number);
  check(o.stages.length && o.stages.every((n,i)=>[2,10,20,25,50,100].includes(n)&&(!i||n>o.stages[i-1])),'Use ascending stages2/10/20/25/50/100');
  o.equipmentRoundtripActors=probe.equipmentRegressionActors(o['equipment-roundtrip-actors'],o.stages.at(-1));
  o.seconds=Number(o['stage-seconds']??60); o.topSeconds=Number(o['top-seconds']??0); o.wallSeconds=Number(o['wall-seconds']??600);
  o.maxEvidenceRows=Number(o['max-evidence-rows']??2000000);
  check(Number.isSafeInteger(o.maxEvidenceRows)&&o.maxEvidenceRows>=2000000&&o.maxEvidenceRows<=8000000,'Evidence row budget must be an integer2000000..8000000');
  check([60,300].includes(o.seconds)&&[0,300].includes(o.topSeconds)&&Number.isInteger(o.wallSeconds)&&o.wallSeconds>=180&&o.wallSeconds<=14400,'Invalid bounded durations');
  for(const key of ['private-directory','output','monitor'])check(o[key]&&path.isAbsolute(o[key]),'Absolute private/output/monitor paths required');
  o.privateDirectory=outsideRepository(o['private-directory']); o.output=outsideRepository(o.output); return o;
}
function owner(c) {
  const p=self(c); check(c.inGame&&!c.closed&&p&&!p.dead&&c.snapshot.playerHp>0,'Actor not a real living in-world player'); return p;
}
function peersFor(actor, actors) {
  return actors.filter(a=>a!==actor&&a.active&&Math.floor(a.index/5)===Math.floor(actor.index/5)&&a.client.snapshot.mapFileName===actor.client.snapshot.mapFileName&&distance(owner(a.client),owner(actor.client))<=12);
}
function targetFor(actor, actors) {
  const c=actor.client, peers=peersFor(actor,actors);
  const targets=c.snapshot.entities.filter(e=>e.kind==='monster'&&WEAK.has(e.name)&&!e.dead&&!actor.quarantined.has(e.objectId)&&
    !actors.some(other=>other!==actor&&other.fighter&&other.target?.objectId===e.objectId&&other.client.snapshot.entities.some(v=>v.objectId===e.objectId&&!v.dead))&&
    peers.some(p=>p.client.snapshot.entities.some(v=>v.objectId===e.objectId&&!v.dead)))
    .sort((a,b)=>distance(owner(c),a)-distance(owner(c),b));
  return targets.find(e=>e.objectId===actor.provenTargetId)??targets[0]??null;
}
export async function walkOne(actor, actors, context, row) {
  const c=actor.client, from={...owner(c)}, peers=peersFor(actor,actors); check(peers.length,'No real AOI movement witness');
  const groupFighter=actors.find(a=>a.index===Math.floor(actor.index/5)*5&&a.active);
  const target=actor.fighter?targetFor(actor,actors):groupFighter?.target;
  if(actor.fighter)actor.target=target;
  const anchor=target??(groupFighter?owner(groupFighter.client):actor.home), map=await context.map(c.snapshot.mapFileName);
  const occupied=new Set(c.snapshot.entities.filter(e=>e.objectId!==from.objectId&&!e.dead).map(e=>e.x+','+e.y));
  const candidates=STEPS.map(([dx,dy,direction],i)=>({x:from.x+dx,y:from.y+dy,direction,i})).filter(p=>
    protocolMapCellIsWalkable(map,p)&&!occupied.has(p.x+','+p.y)&&!context.reserved.has(c.snapshot.mapFileName+':'+p.x+','+p.y)&&
    !(c.snapshot.mapTransfers??[]).some(t=>t.bounds&&p.x>=t.bounds.minX&&p.x<=t.bounds.maxX&&p.y>=t.bounds.minY&&p.y<=t.bounds.maxY));
  candidates.sort((a,b)=>Math.abs(distance(a,anchor)-(actor.fighter?1:3))-Math.abs(distance(b,anchor)-(actor.fighter?1:3))||
    ((a.i-actor.walkRotation+8)%8)-((b.i-actor.walkRotation+8)%8));
  const to=candidates[0]; check(to,'No legal unoccupied ordinary step');
  const witness=peers[0].client, shared=observedPlayerId(witness,c.label); check(Number.isInteger(shared),'Movement shared identity absent');
  const cursors=[c.sequence,witness.sequence], key=c.snapshot.mapFileName+':'+to.x+','+to.y;
  context.reserved.set(key,actor.index);
  try {
    await c.pace(); row.sentAt=mono(); row.latenessMs=row.sentAt-row.plannedAt;
    c.send({type:'walk',direction:to.direction});
    const ownerEvidence=await c.wait(()=>c.events.find(e=>e.sequence>cursors[0]&&e.packet==='UserLocation'&&e.monotonicMs<=row.plannedAt+2000&&
      (e.payload.location?.x??e.payload.x)===to.x&&(e.payload.location?.y??e.payload.y)===to.y),'actual one-step owner ACK',Math.max(1,row.plannedAt+2000-mono()));
    const evidence=await witness.wait(()=>witness.events.find(e=>e.sequence>cursors[1]&&e.packet==='ObjectWalk'&&e.payload?.objectId===shared&&e.monotonicMs<=row.plannedAt+2000&&
      (e.payload.location?.x??e.payload.x)===to.x&&(e.payload.location?.y??e.payload.y)===to.y),'actual one-step peer witness',Math.max(1,row.plannedAt+2000-mono()));
    row.latencyMs=movementWitnessLatency(row.sentAt,ownerEvidence,evidence); row.completed=true;
    row.ownerReceipt=ownerEvidence.sequence; row.peerReceipt=evidence.sequence;row.peerActorOrdinal=witness.ordinal;
    row.ownerReceiptAtMs=ownerEvidence.monotonicMs;row.peerReceiptAtMs=evidence.monotonicMs;
    actor.walkRotation=(to.i+2)%8;
  } finally { context.reserved.delete(key); }
}
async function attackOnce(actor,actors,context,row) {
  const c=actor.client, target=targetFor(actor,actors); actor.target=target;
  check(target&&distance(owner(c),target)===1,'No adjacent real weak PvE target; offered attack remains incomplete');
  const observer=peersFor(actor,actors).find(a=>a.client.snapshot.entities.some(e=>e.objectId===target.objectId&&!e.dead)&&distance(owner(a.client),target)<=6)?.client;
  check(observer,'No actual same-target PvE observer');
  const views=[c,observer], cursors=views.map(v=>v.sequence), attackerIds=views.map(v=>observedPlayerId(v,c.label));
  check(attackerIds.every(Number.isInteger),'PvE attacker namespaces absent');
  await c.pace(); row.sentAt=mono(); row.latenessMs=row.sentAt-row.plannedAt; row.objectId=target.objectId;
  c.send({type:'attack',objectId:target.objectId});
  let launch;
  try { launch=await observer.wait(()=>observer.events.find(e=>e.sequence>cursors[1]&&e.packet==='ObjectAttack'&&e.payload?.objectId===attackerIds[1]),'actual peer attack launch',900); }
  catch(error){actor.quarantined.add(target.objectId);context.emit({kind:'pveNoLaunchQuarantined',actorOrdinal:actor.index,objectId:target.objectId,atUtc:utc(),monotonicMs:mono()});throw error;}
  row.launched=true; row.completed=true; row.latencyMs=launch.monotonicMs-row.sentAt; row.launchReceipt=launch.sequence;
  check(!actor.evidencePending,'At most one pending PvE damage proof');
  actor.evidencePending=(async()=>{
    try {
      const hit=await c.wait(()=>verifiedCombatHit(...views.map((v,i)=>v.events.filter(e=>e.sequence>cursors[i]&&e.monotonicMs>=row.sentAt&&e.monotonicMs<=row.sentAt+1000)),
        target.objectId,...attackerIds),'ordered dual-observer positive HP',Math.max(1,row.sentAt+1000-mono()));
      row.positive=true; row.damage=hit.damage; row.hpLatencyMs=hit.monotonicMs-row.sentAt; actor.provenTargetId=target.objectId;
      context.emit({kind:'positiveHpProof',actorOrdinal:actor.index,ticketSequence:row.ticket,objectId:target.objectId,
        damage:hit.damage,healthPercent:hit.healthPercent,ownerStruckReceipt:hit.receipts[0],peerStruckReceipt:hit.receipts[1],
        latencyMs:row.hpLatencyMs,atUtc:utc(),monotonicMs:mono()});
    } catch { actor.quarantined.add(target.objectId); context.emit({kind:'pveTargetQuarantined',actorOrdinal:actor.index,objectId:target.objectId,atUtc:utc(),monotonicMs:mono()}); }
    finally { actor.evidencePending=null; }
  })();
}
function scheduler(context) {
  const actors=[], allRows=[]; let stopped=false, timer;
  function activate(client,index,at=mono()) {
    const actor={client,index,fighter:fighterForProfile(context.profile,index),active:true,home:{...owner(client)},pending:null,evidencePending:null,
      quarantined:new Set(context.prepQuarantine.get(index)??[]),provenTargetId:context.prepProven.get(index),walkRotation:index%8,ticket:0,
      nextWalk:at+1000+index%10*15,nextChat:at+15500+index%10*15,nextAttack:at+index%10*15,activeAt:at,windowAt:at,windows:[]};
    actors.push(actor); return actor;
  }
  function launch(actor,kind,plannedAt,now) {
    const row={actorOrdinal:actor.index,kind,ticket:++actor.ticket,plannedAt,latenessMs:now-plannedAt,completed:false,launched:false,positive:false}; allRows.push(row);
    context.emit({kind:'offered',action:kind,actorOrdinal:actor.index,ticketSequence:row.ticket,plannedAtMs:plannedAt,
      atUtc:utc(),monotonicMs:now,latenessMs:row.latenessMs});
    if(actor.pending||kind==='attack'&&actor.evidencePending){row.reason='previousActionPending';context.emit({kind:'actionOutcome',action:kind,actorOrdinal:actor.index,ticketSequence:row.ticket,
      completed:false,reason:row.reason,latenessMs:row.latenessMs,atUtc:utc(),monotonicMs:now});return;}
    actor.pending=(async()=>{
      try {
        owner(actor.client);
        if(kind==='walk')await walkOne(actor,actors,context,row);
        else if(kind==='chat'){
          const peers=peersFor(actor,actors); check(peers.length,'Chat has no actual peer');
          await actor.client.pace();row.sentAt=mono();row.latenessMs=row.sentAt-plannedAt;
          const result=await probe.chatAll([actor.client,...peers.map(p=>p.client)],actor.client,{enforceLatency:false});
          row.sentAt=result.sentAtMs;row.latenessMs=row.sentAt-plannedAt;row.latencyMs=result.latencyMs;row.completed=true;
        }else await attackOnce(actor,actors,context,row);
      } catch(error){row.timeout=/timed out|timeout/i.test(error.message);row.reason=context.safe(error.message);}
      finally{
        context.emit({kind:'actionOutcome',action:kind,actorOrdinal:actor.index,ticketSequence:row.ticket,completed:row.completed,launched:row.launched,
          latencyMs:row.latencyMs??-1,latenessMs:row.latenessMs,ownerReceipt:row.ownerReceipt??0,peerReceipt:row.peerReceipt??0,
          peerActorOrdinal:row.peerActorOrdinal??-1,ownerReceiptAtMs:row.ownerReceiptAtMs??-1,peerReceiptAtMs:row.peerReceiptAtMs??-1,
          timeout:row.timeout??false,reason:row.reason??'',atUtc:utc(),monotonicMs:mono()});
        actor.pending=null;
      }
    })();
  }
  async function tick(){
    if(stopped)return;
    try {
      context.assert(); const now=mono();
      for(const actor of actors){owner(actor.client);
        for(const[kind,key,period]of[['walk','nextWalk',2000],['chat','nextChat',30000],...(actor.fighter?[['attack','nextAttack',2000]]:[])]){
          const due=fixedDue(actor[key],period,now);actor[key]=due.next;
          for(const planned of due.due)launch(actor,kind,planned,now);
        }
        while(now>=actor.windowAt+62100){
          const cohortRows=allRows.filter(r=>r.plannedAt>=actor.windowAt&&r.plannedAt<actor.windowAt+60000);
          const result=assessActorMinute(cohortRows.filter(r=>r.actorOrdinal===actor.index),actor.fighter);
          Object.assign(result,workloadAdequacy([result],[{offered:cohortRows.length,completed:cohortRows.filter(r=>r.completed).length}]));
          if(!result.passed){context.allSloPassed=false;context.activitySloPassed=false;}
          actor.windows.push(result);context.emit({kind:'continuousMinute',actorOrdinal:actor.index,windowStartedAtMs:actor.windowAt,...result,atUtc:utc(),monotonicMs:now});
          actor.windowAt+=60000;check(actor.windows.length<=1000,'Continuous window evidence bound exceeded');
          check(campaignMayContinue(context.campaignMode,result),'Continuous per-actor minute failed '+(context.campaignMode==='explore'?'minimum workload':'strict SLO')+' during '+context.phase+' actorOrdinal='+actor.index);
        }
      }
    }catch(error){context.stop(context.safe(error.message));}
  }
  timer=setInterval(()=>void tick(),20);
  return {actors,allRows,activate,
    async stop(){stopped=true;clearInterval(timer);await Promise.allSettled(actors.flatMap(a=>[a.pending,a.evidencePending]).filter(Boolean));},
    async measure(name,seconds,count){
      const start=mono(), population=[];context.phase=name;let failure;
      const wireBefore=actors.map(a=>({actorOrdinal:a.index,profile:a.client.transportProfile,...a.client.bytes}));
      try{while(mono()-start<seconds*1000){context.assert();check(actors.filter(a=>a.active&&a.client.inGame&&!a.client.closed).length===count,'Controlled active population differs');
        const health=context.guard.latest?.health;check(health?.currentWsConnections===count&&health?.currentActiveSessions===count,'Authoritative server WS/active population differs from measured cohort');
        population.push({atUtc:utc(),elapsedMs:mono()-start,controlled:count,serverWs:health.currentWsConnections,
          serverActive:health.currentActiveSessions});await sleep(1000);}}catch(error){failure=context.safe(error.message);}
      const end=mono(),wireAfter=actors.map(a=>({actorOrdinal:a.index,profile:a.client.transportProfile,...a.client.bytes}));
      await sleep(2100);try{context.assert();}catch(error){failure??=context.safe(error.message);}
      const windows=[];
      for(let minute=0;minute<Math.floor((end-start)/60000);minute++){
        const from=start+minute*60000,to=from+60000;
        const results=actors.map(a=>({actorOrdinal:a.index,fighter:a.fighter,transportProfile:a.client.transportProfile,
          ...assessActorMinute(allRows.filter(r=>r.actorOrdinal===a.index&&r.plannedAt>=from&&r.plannedAt<to),a.fighter)}));
        windows.push({minute:minute+1,actors:results,passed:results.every(r=>r.passed),...workloadAdequacy(results)});
      }
      const passed=!failure&&windows.length>=Math.floor(seconds/60)&&windows.every(w=>w.passed);
      if(!passed){context.allSloPassed=false;context.activitySloPassed=false;}
      return {name,players:count,seconds,startedAtMs:start,endedAtMs:end,elapsedSeconds:(end-start)/1000,population,windows,
        applicationTraffic:stageApplicationTraffic(wireBefore,wireAfter,end-start),wireBefore,wireAfter,
        failure:failure??null,passed,workloadAdequate:!failure&&windows.length>=Math.floor(seconds/60)&&windows.every(w=>w.workloadAdequate)};
    }};
}

export async function runLoad(options) {
  options.campaignMode=campaignMode(options.campaignMode,options.profile);
  options.observeLateBootstrap=lateBootstrapObservation(typeof options.observeLateBootstrap==='boolean'?
    options.observeLateBootstrap?'on':'off':options.observeLateBootstrap,options.campaignMode);
  options.equipmentRoundtripActors??=[];
  options.maxEvidenceRows??=2000000;
  check(Number.isSafeInteger(options.maxEvidenceRows)&&options.maxEvidenceRows>=2000000&&options.maxEvidenceRows<=8000000,'Evidence row budget must be an integer2000000..8000000');
  for(const key of ['SignedClient','login','atomicPrivate','secureDirectory','state','chatAll'])check(typeof probe[key]==='function','Probe helper export not ready: '+key);
  await probe.secureDirectory(options.privateDirectory);await fs.mkdir(options.output,{recursive:true});
  const lockFile=path.join(options.privateDirectory,'.cloud-reference.lock'),lock=await fs.open(lockFile,'wx',0o600);
  const runId=Date.now()+'-'+crypto.randomBytes(3).toString('hex'),eventsFile=path.join(options.output,runId+'.events.jsonl'),reportFile=path.join(options.output,runId+'.report.json');
  const writer=new BoundedEvidence(eventsFile),monitorWriter=new BoundedEvidence(path.join(options.output,runId+'.monitor.jsonl'));
  const guard=new CapacityMonitor(options.monitor,monitorWriter),live=[];
  const report={schema:'mir2.cloud-reference-public-signed-load.v1',startedAtUtc:utc(),capacityEvidence:false,ccuQualified:false,
    maximumEvidenceRows:options.maxEvidenceRows,campaignMode:options.campaignMode,campaignCompleted:false,allSloPassed:false,
    diagnosticObserveEnabled:options.observeLateBootstrap,diagnosticObserveUsed:false,admissionSloMisses:[],
    equipmentRoundtripRequestedActors:options.equipmentRoundtripActors,equipmentRegression:[],admissionTransportRecovery:[],
    profile:options.profile,combatTested:options.profile==='mixed-combat',fullCombatCapacityQualified:false,physicalTriClientTested:false,
    scope:options.profile==='movement-chat'?'mixed-transport ordinary movement/chat only; no combat or physical-client qualification':
      'mixed-plain-Web/native-gzip ordinary movement and at-least20-percent weak-PvE; no client rendering proof',stages:[],preparation:[],cleanup:[],savedVerification:[],ok:false};
  const context={webBase:options['web-base'],endpoint:options.endpoint,profile:options.profile,campaignMode:options.campaignMode,allSloPassed:true,activitySloPassed:true,connection:0,secrets:[],guard,reserved:new Map(),phase:'preflight',prepQuarantine:new Map(),prepProven:new Map(),
    admissionTransportRecovery:report.admissionTransportRecovery,firstAttemptAdmissionOk:true,
    observeLateBootstrap:options.observeLateBootstrap,admissionSloMisses:report.admissionSloMisses,
    transportForActor:index=>index%2===0?'plain-Web':'native-gzip',
    deadline:Date.now()+options.wallSeconds*1000,actorsFile:path.join(options.privateDirectory,'actors.private.json'),ledgerFile:path.join(options.privateDirectory,'auth-ledger.json'),
    assert(){if(writer.failure)throw writer.failure;guard.assert();check(Date.now()<this.deadline,'Finite load wall exhausted');},stop(reason){guard.stop(reason);},
    safe(message){return redact(message,this.secrets.concat((this.pool?.actors??[]).map(a=>a.name)));},
    emit(row){if(writer.rows>=options.maxEvidenceRows){this.evidenceLimitExceeded=true;throw new Error('Driver anonymous evidence hard row bound exceeded: '+options.maxEvidenceRows);}
      if(row.kind==='pveTargetQuarantined'){if(!this.prepQuarantine.has(row.actorOrdinal))this.prepQuarantine.set(row.actorOrdinal,new Set());this.prepQuarantine.get(row.actorOrdinal).add(row.objectId);}
      writer.add(row);if(writer.failure)throw writer.failure;},
    async map(id){if(this.mapId!==id){this.loadedMap=await loadProtocolCollisionMap(id,options['map-root']?{mapRoot:options['map-root'],packagedMapRoot:options['map-root']}:{});this.mapId=id;}return this.loadedMap;}};
  let engine,timer,phase='preflight';const stop=()=>guard.stop('Operator requested load drain');process.on('SIGINT',stop);process.on('SIGTERM',stop);
  try{
    await guard.start();timer=setTimeout(()=>guard.stop('Finite load wall exhausted'),options.wallSeconds*1000);
    context.pool=migrateGuestPool(JSON.parse(await fs.readFile(context.actorsFile,'utf8')));
    check(context.pool.endpoint===context.endpoint&&context.pool.webBase===context.webBase,'Existing private cohort endpoint changed');
    const ledger=JSON.parse(await fs.readFile(context.ledgerFile,'utf8'));
    check(ledger.schemaVersion===1&&ledger.purpose==='cloud-reference-public-auth-budget'&&ledger.endpoint===context.endpoint&&ledger.webBase===context.webBase,'Real persistent auth ledger missing or mismatched');
    context.loginBudget=new LoginBudget(ledger.attempts,0);report.priorAuthAttempts=ledger.attempts.length;
    engine=scheduler(context);
    for(const count of options.stages){phase=context.phase='admission-'+count;
      while(context.pool.actors.length<count)context.pool.actors.push({ordinal:context.pool.actors.length,name:'Cg'+crypto.randomBytes(4).toString('hex'),identityHistory:[]});
      await probe.atomicPrivate(context.actorsFile,context.pool);
      const first=live.length;
      while(live.length<count){context.assert();const i=live.length;
        const c=new probe.SignedClient(context.endpoint,context.pool.actors[i],i,context);live.push(c);await probe.login(c,context,'admission');
        await context.map(c.snapshot.mapFileName);
      }
      for(let i=first;i<live.length;i++)await equipStarterGear(live[i]);
      for(const i of options.equipmentRoundtripActors.filter(i=>i>=first&&i<live.length)){
        phase=context.phase='equipment-roundtrip-'+i;const result={actorOrdinal:i,round:report.equipmentRegression.filter(r=>r.actorOrdinal===i).length+1,passed:false,steps:[]};
        report.equipmentRegression.push(result);await probe.equipmentRoundtrip(live[i],result);
      }
      for(let i=first;i<live.length;i++)if(fighterForProfile(options.profile,i)){const group=live.filter(c=>Math.floor(c.ordinal/5)===Math.floor(i/5));
        const proof=await probe.weakPve(group,live[i],id=>context.map(id),context);context.prepProven.set(i,proof.targetObjectId);report.preparation.push(proof);}
      for(let i=first;i<live.length;i++)engine.activate(live[i],i,mono()+2000);
      await sleep(2200);await guard.afterAdmission(Date.now());
      phase='stage-'+count;const stage=await engine.measure(phase,options.seconds,count);report.stages.push(stage);
      console.log(JSON.stringify({stage:count,passed:stage.passed,workloadAdequate:stage.workloadAdequate,campaignMode:options.campaignMode,actors:count,profile:options.profile,
        scope:options.profile==='movement-chat'?'movement-chat-only':count===2?'protocol-combat-calibration':'measured-mixed-workload'}));
      check(campaignMayContinue(options.campaignMode,stage),'First failed '+(options.campaignMode==='explore'?'minimum workload or safety':'activity/latency')+' window stops escalation; inspect per-actor reasons');
      if(stage.passed)report.highestPassedStage=count;
    }
    if(options.topSeconds){phase='top-observation';const stage=await engine.measure(phase,options.topSeconds,live.length);report.stages.push(stage);check(campaignMayContinue(options.campaignMode,stage),'Top observation failed');}
    report.campaignCompleted=true;
    report.measurementOk=campaignAllSloPassed(report.campaignCompleted,context.allSloPassed,report.stages,report.admissionSloMisses);
    if(!report.measurementOk)report.failure=campaignSloFailure(context.activitySloPassed,report.stages,report.admissionSloMisses);
  }catch(error){report.measurementOk=false;report.failure={phase,message:context.safe(error.message)};}
  finally{
    if(engine)await engine.stop();for(const c of live)c.draining=true;
    clearTimeout(timer);await sleep(live.some(c=>c.inGame&&!c.closed)?11000:0);
    report.allSloPassed=campaignAllSloPassed(report.campaignCompleted,context.allSloPassed,report.stages,report.admissionSloMisses);
    report.measurementOk=report.measurementOk===true&&report.allSloPassed;
    const samples=[];
    for(const c of live){try{if(c.inGame&&!c.closed)samples.push({client:c,expected:probe.state(c)});await c.close();report.cleanup.push({actorOrdinal:c.ordinal,normalLogout:!c.inGame&&c.closed});}
      catch(error){report.cleanup.push({actorOrdinal:c.ordinal,normalLogout:false,message:context.safe(error.message)});}}
    if((report.measurementOk||options.campaignMode==='explore'&&report.campaignCompleted)&&!guard.stopped&&!context.evidenceLimitExceeded&&!writer.failure&&report.cleanup.every(r=>r.normalLogout)){
      for(const sample of samples.slice(0,2)){const c=new probe.SignedClient(context.endpoint,sample.client.actor,sample.client.ordinal,context);live.push(c);
        const result={actorOrdinal:c.ordinal,statePreserved:false,passed:false};
        try{await probe.login(c,context,'verification');check(JSON.stringify(probe.state(c))===JSON.stringify(sample.expected),'Save/relogin state changed');result.statePreserved=true;}
        catch(error){result.message=context.safe(error.message);}
        await closeVerification(c,result,message=>context.safe(message));report.savedVerification.push(result);
        report.cleanup.push({actorOrdinal:c.ordinal,connection:c.connection,phase:'verification',normalLogout:result.normalLogout,closeFailure:result.closeFailure??null});}
    }
    guard.close();process.off('SIGINT',stop);process.off('SIGTERM',stop);for(const c of live){clearInterval(c.timer);c.ws?.terminate();}
    // Verification may also expose a bootstrap deadline miss. Recompute after
    // every login so late diagnostic completion can never restore a green run.
    report.allSloPassed=campaignAllSloPassed(report.campaignCompleted,context.allSloPassed,report.stages,report.admissionSloMisses);
    report.measurementOk=report.measurementOk===true&&report.allSloPassed;
    report.steadyStageSloPassed=campaignAllSloPassed(report.campaignCompleted,context.activitySloPassed,report.stages);
    if(report.campaignCompleted&&!report.measurementOk&&(!report.failure||['campaign','admission-SLO'].includes(report.failure.phase)))
      report.failure=campaignSloFailure(context.activitySloPassed,report.stages,report.admissionSloMisses);
    report.diagnosticObserveUsed=context.diagnosticObserveUsed===true;
    report.ok=report.measurementOk===true&&report.admissionSloMisses.length===0&&!guard.stopped&&!context.evidenceLimitExceeded&&!writer.failure&&report.cleanup.every(r=>r.normalLogout)&&report.savedVerification.length===Math.min(2,options.stages.at(-1))&&report.savedVerification.every(r=>r.passed);
    report.completedAtUtc=utc();report.eventsFile=eventsFile;report.transports=live.filter(c=>c.connection<=options.stages.at(-1)).map(c=>({actorOrdinal:c.ordinal,profile:c.transportProfile,setupBudgetMs:c.transportProfile==='plain-Web'?60000:20000}));
    report.rows=writer.rows;report.evidenceLimitExceeded=context.evidenceLimitExceeded===true;
    report.evidenceComplete=!report.evidenceLimitExceeded&&!writer.failure;
    report.resourceStopReason=guard.reason??null;report.logoutReceipts=live.map(c=>c.logoutReceipt).filter(Boolean);
    report.bootstrapApplicationTraffic=live.filter(c=>c.connection<=options.stages.at(-1)).map(c=>({actorOrdinal:c.ordinal,profile:c.transportProfile,
      setupApplicationBytesSent:c.bootstrapProof?.setupApplicationBytesSent??null,setupApplicationBytesReceived:c.bootstrapProof?.setupApplicationBytesReceived??null,
      strictSetupDeadlinePassed:c.bootstrapProof?.strictSetupDeadlinePassed??false,diagnosticAcceptedLate:c.bootstrapProof?.diagnosticAcceptedLate??false,
      scope:'Cumulative game WS payload through complete bootstrap; separate from steady-stage deltas'}));
    const admissionClients=live.filter(c=>c.connection<=options.stages.at(-1));
    report.attemptedAdmissionActors=admissionClients.length;
    report.completedAdmissionActors=admissionClients.filter(c=>c.bootstrapProof).length;
    report.bootstrapDeadlineSloPassed=report.admissionSloMisses.length===0&&report.completedAdmissionActors===options.stages.at(-1);
    if(report.admissionSloMisses.length)report.scope+='; missed original bootstrap admission SLO; late completion is diagnostic and unqualified';
    report.firstAttemptAdmissionOk=context.firstAttemptAdmissionOk&&admissionClients.length>0&&admissionClients.every(c=>c.bootstrapProof);
    try{await Promise.all([writer.close(),monitorWriter.close()]);}catch(error){report.ok=false;report.evidenceComplete=false;report.evidenceFailure=context.safe(error.message);}
    report.eventsSha256=crypto.createHash('sha256').update(await fs.readFile(eventsFile)).digest('hex');
    report.sourceSha256=crypto.createHash('sha256').update(await fs.readFile(new URL(import.meta.url))).digest('hex');
    report.probeSourceSha256=crypto.createHash('sha256').update(await fs.readFile(new URL('./public_signed_probe.mjs',import.meta.url))).digest('hex');
    report.authSourceSha256=crypto.createHash('sha256').update(await fs.readFile(new URL('./public_sui_wallet_auth.mjs',import.meta.url))).digest('hex');
    await fs.writeFile(reportFile,JSON.stringify(report,null,2)+'\n',{flag:'wx'});await lock.close();await fs.unlink(lockFile);
  }
  return {reportFile,report};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(path.resolve(process.argv[1])).href){try{const r=await runLoad(optionsFrom(process.argv.slice(2)));console.log(JSON.stringify({reportFile:r.reportFile,ok:r.report.ok,failure:r.report.failure??null}));process.exitCode=r.report.ok?0:1;}catch(error){console.error(error.message);process.exitCode=1;}}
