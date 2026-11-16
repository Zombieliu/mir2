import { performance } from 'node:perf_hooks';

export const LIMITS=Object.freeze({minimumConnection:1,maximumConnection:2,minX:241,maxX:244,minY:594,maxY:599,rememberedIds:64,localEntities:32,pendingEvents:64,rowsPerConnection:12000,hintCells:7});
const INSTALLED=new WeakMap();
const MOVEMENT=new Set(['ObjectWalk','ObjectRun','ObjectTurn','ObjectBackStep','ObjectPushed','ObjectDash','ObjectDashFail']);
const SOURCES=new Set([...MOVEMENT,'ObjectMonster','NewMonsterInfo','ObjectNpc','NewNpcInfo','ObjectPlayer','ObjectRemove','ObjectTeleportOut','ObjectDied','ObjectRevived','ObjectTeleportIn','ObjectAttack','ObjectRangeAttack','ObjectMagic','ObjectStruck','UserLocation','Pushed','UserDash','UserDashFail','MapChanged','MapInformation']);
const DIRECTIONS=new Set(['Up','UpRight','Right','DownRight','Down','DownLeft','Left','UpLeft']);
const PHASES=new Set(['bootstrap-2','bootstrap-10','stage-2','stage-10','settle-2','settle-10','soak','drain','setup']);
const point=value=>value&&Number.isSafeInteger(value.x)&&value.x>=0&&value.x<=65535&&Number.isSafeInteger(value.y)&&value.y>=0&&value.y<=65535?{x:value.x,y:value.y}:null;
const id=value=>Number.isSafeInteger(value)&&value>0&&value<=4294967295?value:null;
const integer=value=>Number.isSafeInteger(value)&&value>=0?value:null;
const clock=value=>Number.isFinite(value)&&value>=0?value:null;
const map=value=>value==='0'?'0':value==null?'missing':'other';
const inside=p=>p&&p.x>=LIMITS.minX&&p.x<=LIMITS.maxX&&p.y>=LIMITS.minY&&p.y<=LIMITS.maxY;
const kind=value=>value==='monster'||value==='npc'?value:'unknown';
function entity(value){const objectId=id(value?.objectId),p=point(value);return objectId&&p?{objectId,...p,kind:kind(value.kind),...(typeof value.dead==='boolean'?{dead:value.dead}:{})}:null;}
function isNonPlayer(value,snapshot){return value&&id(value.objectId)!==id(snapshot?.playerObjectId)&&(value.kind==='monster'||value.kind==='npc');}
function find(snapshot,objectId){return objectId&&Array.isArray(snapshot?.entities)?snapshot.entities.find(e=>id(e?.objectId)===objectId):null;}
function snapshotView(client){
 const snapshot=client.snapshot,regionMap=map(snapshot?.mapFileName),owner=entity(find(snapshot,id(snapshot?.playerObjectId))),entities=[];let totalCount=0;
 const projectionLimit=LIMITS.localEntities/2; // Two views together never exceed32 non-player entities per row.
 if(regionMap==='0'&&Array.isArray(snapshot?.entities))for(const value of snapshot.entities){if(!isNonPlayer(value,snapshot)||!inside(point(value)))continue;totalCount++;if(entities.length<projectionLimit){const p=entity(value);if(p)entities.push(p);}}
 return{map:regionMap,owner:owner?{objectId:owner.objectId,x:owner.x,y:owner.y,...(typeof owner.dead==='boolean'?{dead:owner.dead}:{})}:null,entities,totalCount,projectionLimit,truncated:totalCount>projectionLimit};
}
function hintView(client){
 const hint=client.navigationHint;if(!hint||hint.connection!==client.lifecycleConnection||hint.mapFileName!=='0'||!['walk','run'].includes(hint.kind))return null;
 const origin=point(hint.origin),first=point(hint.first),target=point(hint.target);
 if(!origin||!first||!target||!Array.isArray(hint.cells)||hint.cells.length>LIMITS.hintCells)return null;
 const cells=hint.cells.map(point);if(cells.some(p=>!p))return null;
 return{kind:hint.kind,origin,first,target,cells,sendCursor:integer(hint.sendCursor),sentAtMs:clock(hint.sentAtMs)};
}
function hintIntersects(hint){return hint&&hint.cells.some(inside);}
function wireView(message){
 const value=message?.payload,objectId=id(value?.objectId),p=point(value?.location)??point(value);
 const direction=DIRECTIONS.has(value?.direction)?value.direction:undefined;
 return{...(objectId?{objectId}:{}),...(p??{}),...(direction?{direction}:{})};
}
function sourceOf(message){if(message?.type==='worldSnapshot')return'worldSnapshot';return message?.type==='packet'&&SOURCES.has(message.packet)?message.packet:null;}
function commandOf(command){return command&&['walk','run'].includes(command.type)?{kind:command.type,...(DIRECTIONS.has(command.direction)?{direction:command.direction}:{})}:null;}
function receiptOf(client){const r=client.events?.at(-1);return r?{sequence:integer(r.sequence),monotonicMs:clock(r.monotonicMs)}:null;}

// The only mutations are this module's prototype wrappers and private bounded
// diagnostic state. No client state, event history, planner or metric is edited.
export function installCompactNavigationDiagnostics(Client,{emit,wallNow=Date.now,monotonicNow=()=>performance.now()}={}){
 if(typeof emit!=='function'||typeof wallNow!=='function'||typeof monotonicNow!=='function')throw TypeError('Synchronous diagnostic callback and clocks are required');
 const prototype=Client?.prototype,descriptors=Object.fromEntries(['observe','send'].map(k=>[k,prototype&&Object.getOwnPropertyDescriptor(prototype,k)]));
 if(!prototype||INSTALLED.has(prototype)||Object.values(descriptors).some(d=>typeof d?.value!=='function'||!d.writable))throw Error('Expected an unwrapped writable own observe/send prototype');
 const states=new Map(),byClient=new WeakMap();let installed=true,duplicateConnectionInstances=0,selectionFailures=0;
 function selected(client){
  let state=byClient.get(client);if(state)return state;
  const connection=client?.lifecycleConnection;if(connection!==1&&connection!==2)return null;
  if(states.has(connection)){duplicateConnectionInstances++;return null;}
  state={connection,remembered:new Map(),rememberedEvictions:0,rows:0,rowAttempts:0,rowsTruncated:false,traceTruncated:false,entityRowsTruncated:0,pendingTruncated:0,pendingWindows:0,pending:null,sequence:0,sinkFailures:0,projectionFailures:0,diagnosticFailures:0,observations:0,sends:0};states.set(connection,state);byClient.set(client,state);return state;
 }
 function remember(state,view){for(const e of view.entities){state.remembered.delete(e.objectId);if(state.remembered.size>=LIMITS.rememberedIds){state.remembered.delete(state.remembered.keys().next().value);state.rememberedEvictions++;}state.remembered.set(e.objectId,{...e});}if(view.truncated)state.entityRowsTruncated++;}
 function base(client,state){return{type:'compactNavigationWire',schemaVersion:1,connection:state.connection,diagnosticSequence:++state.sequence,observedAtMs:integer(wallNow()),monotonicMs:clock(monotonicNow()),phase:PHASES.has(client.context?.phase)?client.context.phase:'other'};}
 function write(client,state,row){
  state.rowAttempts++;if(state.rowsTruncated)return;
  if(state.rows===LIMITS.rowsPerConnection-1){state.rowsTruncated=true;row={...base(client,state),event:'row-limit',truncated:true,reason:'rows-per-connection-limit'};}
  if(row.truncated===true)state.traceTruncated=true;
  try{emit(row,client.context?.writer);state.rows++;}catch(error){state.sinkFailures++;throw error;}
 }
 function pendingView(state){const p=state.pending;return p?{diagnosticWindow:p.number,sendCursor:p.sendCursor,recordedSendSequence:p.recordedSendSequence,eventsRecorded:p.eventsRecorded,truncated:p.truncated}:null;}
 function observeBefore(client,state,args){
  const source=sourceOf(args[0]);if(!source)return null;
  const view=snapshotView(client),wire=source==='worldSnapshot'?{}:wireView(args[0]),objectId=wire.objectId??null,existing=find(client.snapshot,objectId),known=existing?entity(existing):state.remembered.get(objectId)??null;
  remember(state,view);
  return{source,view,wire,objectId,beforePresent:Boolean(existing),beforeKnown:known,knownPlayer:existing&&(existing.kind==='player'||id(existing.objectId)===id(client.snapshot?.playerObjectId)),sequence:integer(client.sequence),hint:hintView(client),hintToken:client.navigationHint};
 }
 function observeAfter(client,state,before,originalThrew){
  if(!before)return;
  const after=snapshotView(client),afterExisting=find(client.snapshot,before.objectId),knownPlayer=before.knownPlayer||afterExisting&&(afterExisting.kind==='player'||id(afterExisting.objectId)===id(client.snapshot?.playerObjectId));
  const wasRemembered=state.remembered.has(before.objectId),wirePoint=point(before.wire),knownPoint=point(before.beforeKnown),afterKnown=entity(afterExisting);
  const beforeMap=before.view.map,afterMap=after.map,regionInterest=before.view.totalCount>0||after.totalCount>0||inside(before.view.owner)||inside(after.owner);
  const mapRelevant=beforeMap==='0'||afterMap==='0';
  const entityRelevant=!knownPlayer&&(inside(wirePoint)||inside(knownPoint)||inside(afterKnown)||wasRemembered);
  const scope=mapRelevant&&(before.source==='worldSnapshot'||['MapChanged','MapInformation'].includes(before.source)?regionInterest||Boolean(state.pending):entityRelevant||regionInterest&&['UserLocation','Pushed','UserDash','UserDashFail'].includes(before.source)||Boolean(state.pending));
  remember(state,after);if(knownPlayer)state.remembered.delete(before.objectId);
  if(after.map==='0'&&!knownPlayer&&inside(wirePoint)&&['ObjectMonster','NewMonsterInfo','ObjectNpc','NewNpcInfo'].includes(before.source)&&before.objectId){
   const e={objectId:before.objectId,...wirePoint,kind:['ObjectNpc','NewNpcInfo'].includes(before.source)?'npc':'monster'};remember(state,{entities:[e],truncated:false});
  }
  const hintAfter=hintView(client),hintCleared=Boolean(before.hintToken)&&client.navigationHint!==before.hintToken;
  try{
   if(!scope||knownPlayer)return;
   const p=state.pending;if(p&&p.truncated)return;
   if(p&&p.eventsRecorded===LIMITS.pendingEvents-1){p.truncated=true;state.pendingTruncated++;p.eventsRecorded++;write(client,state,{...base(client,state),event:'pending-limit',source:before.source,pending:pendingView(state),truncated:true,reason:'pending-event-limit'});return;}
   if(p)p.eventsRecorded++;
   const unknownMotion=MOVEMENT.has(before.source)&&!before.beforePresent;
   write(client,state,{...base(client,state),event:'observe',source:before.source,sequenceBefore:before.sequence,sequenceAfter:integer(client.sequence),receipt:receiptOf(client),originalOutcome:originalThrew?'threw':'returned',wire:before.wire,beforePresent:before.beforePresent,afterPresent:Boolean(afterExisting),wireClassification:knownPlayer?'player-excluded':before.beforeKnown?.kind==='monster'||before.beforeKnown?.kind==='npc'?before.beforeKnown.kind:afterKnown?.kind==='monster'||afterKnown?.kind==='npc'?afterKnown.kind:'unclassified',unknownMotion,before:before.view,after,hintBefore:before.hint,hintAfter,hintCleared,pending:pendingView(state),rememberedIds:state.remembered.size,rememberedEvictions:state.rememberedEvictions,rememberedTruncated:state.rememberedEvictions>0,truncated:before.view.truncated||after.truncated});
   state.observations++;
  }finally{if(state.pending&&hintCleared)state.pending=null;}
 }
 function sendBefore(client,state,args){const command=commandOf(args[0]),hint=hintView(client);if(!command||!hintIntersects(hint))return null;return{command,hint,sequence:integer(client.sequence),before:snapshotView(client)};}
 function sendAfter(client,state,before,originalThrew){
  if(!before)return;state.sends++;
  const priorPendingSuperseded=!originalThrew&&Boolean(state.pending);
  if(!originalThrew){if(state.pending){state.pendingTruncated++;state.pending=null;}state.pending={number:++state.pendingWindows,sendCursor:before.hint.sendCursor,recordedSendSequence:integer(client.sequence),eventsRecorded:0,truncated:false};}
  const after=snapshotView(client);
  write(client,state,{...base(client,state),event:'send',source:before.command.kind,command:before.command,sequenceBefore:before.sequence,sequenceAfter:integer(client.sequence),originalOutcome:originalThrew?'threw':'returned',before:before.before,after,hint:before.hint,pending:pendingView(state),priorPendingSuperseded,truncated:priorPendingSuperseded||before.before.truncated||after.truncated});
 }
 const wrappers={};
 for(const method of ['observe','send']){
  const original=descriptors[method].value;
  wrappers[method]=function(...args){
   let state,selectionError;try{state=selected(this);}catch(error){selectionFailures++;selectionError=error;}
   if(!state){const result=Reflect.apply(original,this,args);if(selectionError)throw selectionError;return result;}
   if(state.rowsTruncated)return Reflect.apply(original,this,args);
   let before,projectionError;try{before=method==='observe'?observeBefore(this,state,args):sendBefore(this,state,args);}catch(error){state.projectionFailures++;state.diagnosticFailures++;projectionError=error;}
   let result,originalError,originalThrew=false;
   try{result=Reflect.apply(original,this,args);}catch(error){originalThrew=true;originalError=error;}
   let diagnosticError=projectionError;
   if(!diagnosticError)try{if(method==='observe')observeAfter(this,state,before,originalThrew);else sendAfter(this,state,before,originalThrew);}catch(error){state.diagnosticFailures++;diagnosticError=error;}
   if(originalThrew)throw originalError; // Preserve the identical original error.
   if(diagnosticError)throw diagnosticError; // Existing writer safety-stop can propagate.
   return result;
  };
 }
 for(const method of ['observe','send'])Object.defineProperty(prototype,method,{...descriptors[method],value:wrappers[method]});
 const token={};INSTALLED.set(prototype,token);
 return Object.freeze({
  status(){return{schemaVersion:1,installed,scope:'Additional bounded diagnostics only; no ACK classification or metric/acceptance mutation. Callback exceptions can stop the original driver; allocation and projection overhead remain subject to unchanged driver-lag/safety gates.',limits:{...LIMITS},duplicateConnectionInstances,selectionFailures,connections:[...states.values()].map(s=>({connection:s.connection,rows:s.rows,rowAttempts:s.rowAttempts,rememberedIds:s.remembered.size,rememberedEvictions:s.rememberedEvictions,entityRowsTruncated:s.entityRowsTruncated,pendingTruncated:s.pendingTruncated,pendingWindows:s.pendingWindows,pendingOpen:Boolean(s.pending),rowsTruncated:s.rowsTruncated,traceTruncated:s.traceTruncated,sinkFailures:s.sinkFailures,projectionFailures:s.projectionFailures,diagnosticFailures:s.diagnosticFailures,observations:s.observations,sends:s.sends,complete:!s.traceTruncated&&!s.rowsTruncated&&!s.rememberedEvictions&&!s.entityRowsTruncated&&!s.pendingTruncated&&!s.pending&&!s.sinkFailures&&!s.diagnosticFailures}))};},
  uninstall(){if(!installed)return;if(INSTALLED.get(prototype)!==token||['observe','send'].some(k=>prototype[k]!==wrappers[k]))throw Error('Refusing to overwrite a newer prototype patch');for(const method of ['observe','send'])Object.defineProperty(prototype,method,descriptors[method]);INSTALLED.delete(prototype);installed=false;}
 });
}
