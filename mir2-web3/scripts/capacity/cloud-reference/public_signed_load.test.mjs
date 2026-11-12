import test from 'node:test';
import assert from 'node:assert/strict';
import os from 'node:os';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { fixedDue, assessActorMinute, movementWitnessLatency, closeVerification, fighterForProfile, stageApplicationTraffic,
  campaignMode, workloadAdequacy, campaignMayContinue, campaignAllSloPassed,walkOne,
  lateBootstrapObservation, campaignSloFailure, optionsFrom } from './public_signed_load.mjs';
import { SignedClient, chatAll, optionsFrom as probeOptions } from './public_signed_probe.mjs';
function healthy(fighter=true){return [
  ...Array.from({length:30},()=>({kind:'walk',completed:true,latencyMs:60,latenessMs:10})),
  ...Array.from({length:2},()=>({kind:'chat',completed:true,latencyMs:60,latenessMs:10})),
  ...(fighter?Array.from({length:30},(_,i)=>({kind:'attack',completed:true,launched:true,positive:i<12,hpLatencyMs:150,latencyMs:30,latenessMs:10})):[])];}
test('missed fixed opportunities remain due and next clock is not shifted after work completion',()=>{
  assert.deepEqual(fixedDue(1000,2000,7500),{due:[1000,3000,5000,7000],next:9000});
});
test('real complete mover and fixed fighter windows pass declared gates',()=>{
  assert.equal(assessActorMinute(healthy(),true).passed,true);
  assert.equal(assessActorMinute(healthy(false),false).passed,true);
});
test('idle sockets and movement-only actor cannot substitute for the declared fighter',()=>{
  assert.equal(assessActorMinute([],false).passed,false);
  assert.equal(assessActorMinute(healthy(false),true).passed,false);
});
test('skipped fixed plans, driver lateness, timeouts and insufficient HP each invalidate a minute',()=>{
  const cases=[rows=>rows.slice(0,4).forEach(r=>{r.completed=false;r.latenessMs=500;}),
    rows=>{rows[0].timeout=true;},rows=>rows.filter(r=>r.kind==='attack').forEach((r,i)=>{r.positive=i<11;}),
    rows=>rows.filter(r=>r.positive).forEach(r=>{r.hpLatencyMs=1001;})];
  for(const mutate of cases){const rows=healthy();mutate(rows);assert.equal(assessActorMinute(rows,true).passed,false);}
});
test('aggregate movement cannot hide excessive unlaunched PvE inputs or missing attack offers',()=>{
  const rows=healthy();rows.filter(r=>r.kind==='attack').slice(0,2).forEach(r=>{r.launched=false;r.completed=false;});
  assert.equal(assessActorMinute(rows,true).passed,false);
  const short=healthy().filter((r,i)=>r.kind!=='attack'||i<50);
  assert.equal(assessActorMinute(short,true).passed,false);
});
test('one bad actor minute cannot be covered by another healthy actor',()=>{
  const bad=healthy(false);bad.filter(r=>r.kind==='chat')[0].completed=false;
  assert.equal([assessActorMinute(healthy(),true),assessActorMinute(bad,false)].every(r=>r.passed),false);
});
test('mixed setup preserves real Web plain60s and native gzip20s with separate connections',()=>{
  const context={connection:0,transportForActor:i=>i%2===0?'plain-Web':'native-gzip'};
  const web=new SignedClient('wss://example.invalid/ws',{name:'SyntheticWeb'},0,context);
  const native=new SignedClient('wss://example.invalid/ws',{name:'SyntheticNative'},1,context);
  assert.equal(web.transportProfile,'plain-Web');assert.equal(web.setupBudgetMs,60000);
  assert.equal(native.transportProfile,'native-gzip');assert.equal(native.setupBudgetMs,20000);
  assert.equal(web.connection,1);assert.equal(native.connection,2);
});
test('late owner ACK cannot hide behind a fast peer movement witness',()=>{
  const latency=movementWitnessLatency(100,{monotonicMs:1800},{monotonicMs:160});assert.equal(latency,1700);
  const rows=healthy(false);rows.filter(r=>r.kind==='walk').forEach(r=>{r.latencyMs=latency;});
  assert.equal(assessActorMinute(rows,false).passed,false);
});
test('verification close failure cannot be reported as successful save verification',async()=>{
  const result=await closeVerification({closed:true,inGame:false,async close(){throw new Error('late logout');}},{statePreserved:true});
  assert.equal(result.passed,false);assert.equal(result.normalLogout,false);assert.equal(result.closeFailure,'late logout');
});
test('movement-chat explicitly has zero fighters and default mixed profile keeps its declared combat role',()=>{
  assert.equal(Array.from({length:100},(_,i)=>fighterForProfile('movement-chat',i)).some(Boolean),false);
  assert.equal(Array.from({length:10},(_,i)=>fighterForProfile('mixed-combat',i)).filter(Boolean).length,2);
  assert.equal(assessActorMinute(healthy(false),fighterForProfile('movement-chat',0)).passed,true);
  assert.equal(assessActorMinute(healthy(false),fighterForProfile('mixed-combat',0)).passed,false);
});
test('stage byte deltas exclude startup totals and retain per-actor/profile direction and Mbps',()=>{
  const before=[{actorOrdinal:0,profile:'plain-Web',sent:100,received:200},{actorOrdinal:1,profile:'native-gzip',sent:1000,received:2000}];
  const after=[{actorOrdinal:0,sent:125100,received:250200},{actorOrdinal:1,sent:251000,received:377000}];
  const r=stageApplicationTraffic(before,after,1000);
  assert.equal(r.sentBytes,375000);assert.equal(r.receivedBytes,625000);assert.equal(r.sentMbps,3);assert.equal(r.receivedMbps,5);
  assert.equal(r.actors[0].sentMbps,1);assert.equal(r.profiles.find(p=>p.profile==='native-gzip').receivedMbps,3);
  assert.throws(()=>stageApplicationTraffic(before,[{actorOrdinal:0,sent:0,received:0}],1000),/counter/);
});

test('strict remains the default and explore explicitly rejects combat profiles',()=>{
  assert.equal(campaignMode(),'strict');assert.equal(campaignMode(undefined,'movement-chat'),'strict');
  assert.equal(campaignMode('explore','movement-chat'),'explore');
  assert.throws(()=>campaignMode('explore','mixed-combat'),/movement-chat/);
  assert.throws(()=>campaignMode('relaxed','movement-chat'),/strict or explore/);
});

test('exploration can continue an adequately active failed experience window without changing strict SLO results',()=>{
  const results=Array.from({length:25},()=>assessActorMinute(healthy(false),false));
  const bad14=healthy(false),bad18=healthy(false);
  bad14.slice(0,4).forEach(r=>{r.completed=false;});
  bad18.slice(0,7).forEach(r=>{r.completed=false;});
  results[14]=assessActorMinute(bad14,false);results[18]=assessActorMinute(bad18,false);
  assert.equal(results[14].passed,false);assert.equal(results[18].passed,false);
  const stage={passed:results.every(r=>r.passed),failure:null,...workloadAdequacy(results)};
  assert.equal(stage.cohortOffered,800);assert.equal(stage.cohortCompleted,789);
  assert.equal(stage.workloadAdequate,true);assert.equal(campaignMayContinue('strict',stage),false);
  assert.equal(campaignMayContinue('explore',stage),true);assert.equal(stage.passed,false);
  assert.equal(results[14].passed,false);assert.equal(results[18].passed,false);
});

test('explore keeps every per-actor activity and driver schedule lower bound',()=>{
  const healthyResult=assessActorMinute(healthy(false),false);
  for(const changes of [{offered:31},{moves:19},{chats:0},{completionRatio:.6999},{scheduleP95Ms:251},{scheduleP95Ms:null}]){
    const r=workloadAdequacy([{...healthyResult,...changes}],[healthyResult]);
    assert.equal(r.workloadAdequate,false,JSON.stringify(changes));
    assert.equal(campaignMayContinue('explore',{passed:false,failure:null,...r}),false);
  }
  assert.equal(workloadAdequacy([]).workloadAdequate,false);
});

test('continuous coverage uses the same absolute window actual cohort offers, including partial entrants',()=>{
  const actor=assessActorMinute(healthy(false),false);
  const cohort=[{offered:32,completed:32},{offered:8,completed:6}];
  assert.equal(workloadAdequacy([actor],cohort).workloadAdequate,true);
  assert.equal(workloadAdequacy([actor],[{offered:100,completed:94}]).workloadAdequate,false);
  assert.equal(workloadAdequacy([actor],[{offered:0,completed:0}]).workloadAdequate,false);
  assert.equal(workloadAdequacy([actor],cohort).cohortCompletionRatio,.95);
});

test('explore never continues a safety failure or an inadequate workload even if a caller supplied passed true',()=>{
  for(const mode of ['strict','explore'])assert.equal(campaignMayContinue(mode,{passed:true,workloadAdequate:true,failure:'stale monitor'}),false);
  assert.equal(campaignMayContinue('explore',{passed:true,workloadAdequate:false,failure:null}),false);
  const slow=healthy(false);slow.filter(r=>r.kind==='walk').forEach(r=>{r.latencyMs=2000;});
  const result=assessActorMinute(slow,false);assert.equal(result.passed,false);
  assert.equal(campaignMayContinue('explore',{...result,...workloadAdequacy([result]),failure:null}),true);
  assert.equal(result.movementP95Ms,2000);assert.equal(result.passed,false);
});

test('later passing stages cannot erase an admission-wait continuous SLO failure or an earlier failed stage',()=>{
  assert.equal(campaignAllSloPassed(true,false,[{passed:true},{passed:true}]),false);
  assert.equal(campaignAllSloPassed(true,true,[{passed:false},{passed:true}]),false);
  assert.equal(campaignAllSloPassed(false,true,[{passed:true}]),false);
  assert.equal(campaignAllSloPassed(true,true,[]),false);
  assert.equal(campaignAllSloPassed(true,true,[{passed:true},{passed:true}]),true);
});

test('a chat delivered after1000ms within its existing budget is completed activity but remains a strict SLO failure',async()=>{
  function clients(){
    const speaker={ordinal:0,label:'SyntheticChat',sequence:0,events:[],snapshot:{playerObjectId:1000,entities:[{objectId:1000,name:'SyntheticChat',kind:'selfPlayer'}]},async pace(){}};
    const peer={sequence:0,events:[],snapshot:{entities:[{objectId:50050,name:'SyntheticChat',kind:'player'}]}};
    const all=[speaker,peer];
    speaker.send=command=>{for(const c of all)c.events.push({sequence:++c.sequence,packet:'ObjectChat',payload:{objectId:50050,text:'SyntheticChat : '+command.message},monotonicMs:performance.now()+1250});};
    for(const c of all)c.wait=async(predicate,label,budget)=>{assert.equal(budget,2500);const result=predicate();assert.ok(result);return result;};
    return all;
  }
  let pair=clients();await assert.rejects(chatAll(pair,pair[0]),/exceeded1000ms/);
  pair=clients();const chat=await chatAll(pair,pair[0],{enforceLatency:false});
  assert.equal(chat.passed,false);assert.ok(chat.latencyMs>=1250&&chat.latencyMs<2500);assert.equal(chat.observers,2);
  const rows=healthy(false);for(const row of rows.filter(r=>r.kind==='chat'))row.latencyMs=chat.latencyMs;
  const result=assessActorMinute(rows,false);
  assert.equal(result.chats,2);assert.equal(result.completed,32);assert.equal(result.timeouts,0);assert.equal(result.passed,false);
  assert.equal(workloadAdequacy([result]).workloadAdequate,true);
  assert.equal(campaignAllSloPassed(true,false,[{passed:false}]),false);
});

test('an on-time owner ACK observed after the absolute walk deadline cannot give a late peer fresh wait time',async t=>{
  const context={connection:0,secrets:[],reserved:new Map(),assert(){},emit(){},async map(){return {width:8,height:8,blocked:new Uint8Array(64)};}};
  const c=new SignedClient('wss://example.invalid/ws',{name:'SyntheticWalk'},0,context);
  const peer=new SignedClient('wss://example.invalid/ws',{name:'SyntheticWitness'},1,context);
  c.inGame=peer.inGame=true;
  c.snapshot={mapFileName:'0',playerObjectId:1000,playerHp:18,entities:[{objectId:1000,name:c.label,kind:'selfPlayer',x:4,y:4},{objectId:50051,name:peer.label,kind:'player',x:6,y:4}]};
  peer.snapshot={mapFileName:'0',playerObjectId:1000,playerHp:18,entities:[{objectId:1000,name:peer.label,kind:'selfPlayer',x:6,y:4},{objectId:50050,name:c.label,kind:'player',x:4,y:4}]};
  const actor={client:c,index:0,active:true,fighter:false,home:{x:4,y:4},walkRotation:0};
  const witness={client:peer,index:1,active:true,fighter:false};let timer;
  t.after(()=>clearTimeout(timer));
  c.ws={readyState:1,send(){timer=setTimeout(()=>{
    c.observe({packet:'UserLocation',payload:{location:{x:4,y:3}}});
    const end=performance.now()+120;while(performance.now()<end){}
    peer.observe({packet:'ObjectWalk',payload:{objectId:50050,location:{x:4,y:3}}});
  },10);}};
  const row={plannedAt:performance.now()-1900};
  await assert.rejects(walkOne(actor,[actor,witness],context,row),/timed out waiting for actual one-step peer witness/);
  assert.ok(c.events.find(e=>e.packet==='UserLocation').monotonicMs<=row.plannedAt+2000);
  assert.ok(peer.events.find(e=>e.packet==='ObjectWalk').monotonicMs>row.plannedAt+2000);
  assert.notEqual(row.completed,true);assert.equal(context.reserved.size,0);
});

test('late-bootstrap observation is LOAD-only, defaults off and requires explicit explore mode',()=>{
  const args=['--private-directory',path.join(os.tmpdir(),'mir2-offline-late-private'),
    '--output',path.join(os.tmpdir(),'mir2-offline-late-public'),'--monitor',path.join(os.tmpdir(),'mir2-offline-late-monitor.json')];
  assert.equal(lateBootstrapObservation(),false);assert.equal(optionsFrom(args).observeLateBootstrap,false);
  assert.equal(optionsFrom([...args,'--profile','movement-chat','--campaign-mode','explore']).observeLateBootstrap,false);
  const on=[...args,'--profile','movement-chat','--campaign-mode','explore','--observe-late-bootstrap','on'];
  assert.equal(optionsFrom(on).observeLateBootstrap,true);
  assert.throws(()=>optionsFrom([...args,'--observe-late-bootstrap','on']),/requires campaign-mode explore/);
  assert.throws(()=>lateBootstrapObservation('on','strict'),/requires campaign-mode explore/);
  for(const value of ['yes','true','',null])assert.throws(()=>lateBootstrapObservation(value,'explore'),/on or off/);
  assert.throws(()=>probeOptions([...args,'--observe-late-bootstrap','on']),/Unknown\/repeated option/);
});

test('passing steady stages cannot erase an admission deadline miss or mislabel it as an activity failure',()=>{
  const stages=[{passed:true},{passed:true}],misses=[{phase:'postAckCompleteBootstrap',originalBudgetMs:20000}];
  assert.equal(campaignAllSloPassed(true,false,stages,misses),false);
  assert.equal(campaignAllSloPassed(true,true,stages,misses),false);
  assert.equal(campaignAllSloPassed(true,true,stages,[]),true);
  const failure=campaignSloFailure(true,stages,misses);
  assert.equal(failure.category,'admissionSLO');assert.equal(failure.phase,'admission-SLO');assert.match(failure.message,/original bootstrap admission deadlines/);
  const activityMessage='Campaign completed with failed strict SLO windows; exploration is not a stable capacity qualification';
  for(const activity of [campaignSloFailure(false,stages,misses),campaignSloFailure(true,[{passed:false}],misses),campaignSloFailure(false,stages,[])]){
    assert.equal(activity.category,'activitySLO');assert.equal(activity.message,activityMessage);
  }
});
