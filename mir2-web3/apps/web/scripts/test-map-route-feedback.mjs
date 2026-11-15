import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import test from "node:test";
import ts from "typescript";

const pageSource=readFileSync(new URL("../app/page.tsx",import.meta.url),"utf8");
const ast=ts.createSourceFile("page.tsx",pageSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const names=["cancelMapImageRoute","finishMapImageRoute","mapImageRouteCurrent","refreshMapImageRoute","beginMapImageRoute","mapImageRouteMovement"];
const declarations=[];
function visit(node){
  if(ts.isFunctionDeclaration(node)&&names.includes(node.name?.text)) declarations.push(node.getText(ast));
  ts.forEachChild(node,visit);
}
visit(ast);
// The old Page has no finish helper. Keep the baseline runnable so failures
// demonstrate player behavior, rather than failing to compile the harness.
const code=ts.transpileModule(declarations.join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
const executionSource=readFileSync(new URL("../lib/quest-route-execution.ts",import.meta.url),"utf8");
const execution={exports:{}};
new Function("exports",ts.transpileModule(executionSource,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText)(execution.exports);
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return{promise,resolve,reject};};
const flush=async()=>{for(let i=0;i<10;i++) await Promise.resolve();};

function harness(){
  const ref=current=>({current}),source={owner:{},mapIndex:1,mapFileName:"0",mapWidth:32,mapHeight:32,miniMapIndex:2,bigMapIndex:3};
  const geometry={mapFileName:"0",width:32,height:32,blockedBits:new Uint8Array(128)};
  const state={source,blocked:false,self:{x:10,y:10},now:1000,holdEdges:false,edgeTask:null,result:{status:"ok",steps:[{x:11,y:10},{x:12,y:10}]}};
  const routeRef=ref(null),queuedRef=ref(null),timers=new Map(),loading=[],logs=[],queued=[],queries=[];
  let timerId=0;
  const deps={
    window:{setTimeout(fn,ms){const id=++timerId;timers.set(id,{fn,at:state.now+ms});return id;},clearTimeout:id=>timers.delete(id)},
    Date:{now:()=>state.now},mapImageRouteRef:routeRef,queuedMoveIntentRef:queuedRef,movementPlanRef:ref(null),
    currentMapImageSource:()=>state.source,mapRouteInputBlocked:()=>state.blocked,
    mapImageSourceRef:ref({source,core:{searchMapRoute(query){queries.push(query);return state.result;}}}),
    mapRouteRasterMeta:{frames:[{index:2,path:"/map/2.png",width:100,height:100},{index:3,path:"/map/3.png",width:100,height:100}]},
    retireQuestWorldActions:()=>api.cancelMapImageRoute(),cancelLockedMonsterAttack(){},stopOnchainMining(){},
    questCollisionCacheRef:ref({load(){const task=deferred();loading.push(task);return task.promise;}}),
    buildMapRouteEdges:async()=>{if(state.holdEdges){state.edgeTask=deferred();await state.edgeTask.promise;}return new Uint8Array(1024);},
    currentAuthoritativeSelf:()=>state.self,questRouteOccupancy:()=>[],questRouteRejectedEdges:()=>[],
    queueCrystalMoveIntent:intent=>{queuedRef.current=intent;queued.push(intent);},trySendQueuedCrystalMove(){},
    t:(key,args=[])=>({key,args}),appendLog:(message,tone)=>logs.push({...message,tone}),
    worldRef:ref({}),crystalSelfMovementProfile:()=>({distance:2}),nextQuestRouteStep:execution.exports.nextQuestRouteStep,
    movementBlockedStepsRef:ref([]),recentMovementBlockedSteps:()=>[],movementStepBlockedByRecentCorrection:()=>false,
  };
  const exported=names.filter(name=>declarations.some(text=>text.startsWith(`function ${name}(`)||text.startsWith(`async function ${name}(`)));
  const api=new Function(...Object.keys(deps),`${code};return {${exported.join(",")}};`)(...Object.values(deps));
  const intent={kind:"mini",source,x:12,y:10,imageSrc:"/map/2.png",imageWidth:100,imageHeight:100};
  const begin=()=>{api.beginMapImageRoute(intent);return routeRef.current;};
  const elapse=(ms,fire=true)=>{state.now+=ms;if(fire)for(const[id,row]of[...timers])if(row.at<=state.now){timers.delete(id);row.fn();}};
  return{...api,state,source,geometry,routeRef,queuedRef,loading,logs,queued,queries,timers,begin,elapse,intent};
}
const terminal=h=>h.logs.filter(row=>row.key!=="ui.questRoutePlanning");

test("actual Page map click shows the selected coordinates while waiting for complete collision",()=>{
  const h=harness();h.begin();
  assert.deepEqual(h.logs,[{key:"ui.questRoutePlanning",args:[12,10],tone:"system"}]);
  assert.equal(h.loading.length,1);assert.equal(h.queued.length,0);
});

test("actual Page hung map collision stops at30s and a late body cannot start walking",async()=>{
  const h=harness();h.begin();h.elapse(29_999);assert(h.routeRef.current);
  h.elapse(1);assert.equal(h.routeRef.current,null);
  assert.deepEqual(terminal(h),[{key:"ui.questRouteLoadingTimeout",args:[],tone:"system"}]);
  h.loading[0].resolve(h.geometry);await flush();
  assert.equal(h.queries.length,0);assert.equal(h.queued.length,0);assert.equal(terminal(h).length,1);assert.equal(h.timers.size,0);
});

test("actual Page delayed timer delivery cannot accept geometry after the same30s deadline",async()=>{
  const h=harness();h.begin();h.elapse(30_000,false);h.loading[0].resolve(h.geometry);await flush();
  assert.equal(h.routeRef.current,null);assert.equal(h.queries.length,0);assert.equal(h.queued.length,0);
  assert.equal(terminal(h)[0]?.key,"ui.questRouteLoadingTimeout");assert.equal(h.timers.size,0);
});

test("actual Page initial edge construction shares the deadline with collision download",async()=>{
  const h=harness();h.state.holdEdges=true;h.begin();h.loading[0].resolve(h.geometry);await flush();
  assert(h.state.edgeTask);h.elapse(30_000);assert.equal(h.routeRef.current,null);
  h.state.edgeTask.resolve();await flush();
  assert.equal(h.queries.length,0);assert.equal(h.queued.length,0);assert.equal(terminal(h)[0]?.key,"ui.questRouteLoadingTimeout");
});

test("actual Page complete map route before the deadline cancels its timer and moves to its exact endpoint",async()=>{
  const h=harness(),route=h.begin();h.elapse(29_999);h.loading[0].resolve(h.geometry);await flush();
  assert.equal(h.timers.size,0);assert.equal(h.queued.length,1);assert.equal(h.routeRef.current,route);assert.deepEqual(terminal(h),[]);
  assert.deepEqual(h.mapImageRouteMovement(route,h.state.self,"run",h.state.now),{point:{x:12,y:10},direction:"Right",mode:"run"});
  h.elapse(10_000);assert.equal(h.routeRef.current,route,"a completed planning deadline cannot stop normal travel");
  h.state.self={x:12,y:10};assert.equal(h.mapImageRouteMovement(route,h.state.self,"run",h.state.now),null);
  assert.equal(h.routeRef.current,null);assert.equal(h.queuedRef.current,null);
  assert.deepEqual(terminal(h),[{key:"ui.questRouteArrived",args:[12,10],tone:"system"}]);
});

for(const outcome of ["resolve","reject"])test(`actual Page replaced map ${outcome} and old timer cannot affect a newer click`,async()=>{
  const h=harness();h.begin();h.elapse(200);const next=h.begin();
  assert.equal(h.timers.size,1);
  h.loading[0][outcome](outcome==="resolve"?h.geometry:Error("old failure"));await flush();
  h.elapse(29_800);assert.equal(h.routeRef.current,next);assert.deepEqual(terminal(h),[]);
  h.loading[1].resolve(h.geometry);await flush();assert.equal(h.queued.length,1);assert.equal(h.timers.size,0);
});

for(const change of ["map","blocked","cancel"])test(`actual Page ${change} retires map work without stale timeout or movement`,async()=>{
  const h=harness();h.begin();
  if(change==="map")h.state.source={...h.source,owner:{}};
  else if(change==="blocked")h.state.blocked=true;
  else h.cancelMapImageRoute();
  h.elapse(30_000);h.loading[0].resolve(h.geometry);await flush();
  assert.equal(h.routeRef.current,null);assert.equal(h.queued.length,0);assert.deepEqual(terminal(h),[]);assert.equal(h.timers.size,0);
});

test("actual Page zero-step map click reports only authoritative arrival without movement",async()=>{
  const h=harness();h.state.self={x:12,y:10};h.state.result={status:"ok",steps:[]};h.begin();h.loading[0].resolve(h.geometry);await flush();
  assert.equal(h.queued.length,0);assert.equal(h.routeRef.current,null);assert.equal(h.timers.size,0);
  assert.deepEqual(terminal(h),[{key:"ui.questRouteArrived",args:[12,10],tone:"system"}]);
});

test("actual Page complete unreachable map search reports once and never queues partial progress",async()=>{
  const h=harness();h.state.result={status:"unreachable"};h.begin();h.loading[0].resolve(h.geometry);await flush();
  assert.equal(h.queued.length,0);assert.equal(h.routeRef.current,null);assert.equal(h.timers.size,0);
  assert.equal(terminal(h)[0]?.key,"ui.mapRouteUnavailable");assert.equal(terminal(h).length,1);
});
