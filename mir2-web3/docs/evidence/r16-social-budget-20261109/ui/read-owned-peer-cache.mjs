// Readonly diagnosis of this task's own retained Node process. No Gateway sends.
import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {setTimeout as delay} from 'node:timers/promises';
const pid=Number(process.argv[2]);assert(Number.isInteger(pid)&&pid>0);process._debugProcess(pid);
let target;for(let i=0;i<30;i++){try{target=(await(await fetch('http://127.0.0.1:9229/json/list')).json())[0];if(target)break;}catch{}await delay(100);}
assert(target?.webSocketDebuggerUrl,'OWNED_INSPECTOR_NOT_READY');
const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise((r,j)=>{ws.addEventListener('open',r,{once:true});ws.addEventListener('error',j,{once:true});});
let seq=0;const pending=new Map(),events=[];
ws.addEventListener('message',event=>{const msg=JSON.parse(event.data);if(msg.id){const p=pending.get(msg.id);pending.delete(msg.id);msg.error?p.reject(Error(msg.error.message)):p.resolve(msg.result);}else events.push(msg);});
function rpc(method,params={}){const id=++seq;return new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});}
await rpc('Debugger.enable');
let parsed;for(let i=0;i<30;i++){parsed=events.find(e=>e.method==='Debugger.scriptParsed'&&e.params.url.endsWith('/social-ui.mjs'));if(parsed)break;await delay(100);}
assert(parsed,'OWNED_RUNNER_SCRIPT_NOT_FOUND');
const source=await rpc('Debugger.getScriptSource',{scriptId:parsed.params.scriptId});const lines=source.scriptSource.split('\n');
const lineNumber=lines.findIndex(line=>line.includes("input.on('line',v=>{if(v.trim()==='close')"));assert(lineNumber>=0);
const columnNumber=lines[lineNumber].indexOf("if(v.trim()==='close')");
const point=await rpc('Debugger.setBreakpoint',{location:{scriptId:parsed.params.scriptId,lineNumber,columnNumber}});
console.log(JSON.stringify({phase:'readonly-peer-cache-breakpoint-ready',ownedPid:pid}));
let paused;for(let i=0;i<600;i++){paused=events.find(e=>e.method==='Debugger.paused');if(paused)break;await delay(100);}
assert(paused,'OWNED_READONLY_PAUSE_TIMEOUT');
const expression=`(()=>{const rows=[];let ordinal=0;const waiter=peer.waitFor(e=>{ordinal++;if(e.type==='packet'&&['TradeRequest','TradeAccept','DepositTradeItem','RetrieveTradeItem','TradeCancel','TradeItem'].includes(e.packet))rows.push({ordinal,type:'packet',packet:e.packet,from:e.payload?.from,to:e.payload?.to,success:e.payload?.success,unlock:e.payload?.unlock});if(e.type==='worldSnapshot'){const t=e.payload.stage5Systems?.trade;rows.push({ordinal,type:'worldSnapshot',tick:e.payload.tick,tradeIsNull:t===null,tradePresent:t!=null,completed:t?.completed??null,nonceHash:t?.settlementNonce?hash(t.settlementNonce):null});}return false;},{after:0,timeoutMs:1});waiter.catch(()=>{});const s=peer.snapshot();return JSON.stringify({readonly:true,noGatewayCommandIssued:true,cachedFramesExamined:ordinal,peerSequence:peer.sequence,rows,projection:{ready:s.ready,tradeActive:s.trade.active,uncertain:s.trade.uncertain,idHash:s.trade.id?hash(s.trade.id):null,lastCancel:s.trade.lastCancel?{unlock:s.trade.lastCancel.unlock,tradeIdHash:s.trade.lastCancel.tradeId?hash(s.trade.lastCancel.tradeId):null}:null,economy:econ(s)},primary:{tradeActive:current().trade.active,uncertain:current().trade.uncertain,economy:econ(current())}});})()`;
let result;
try{const v=await rpc('Debugger.evaluateOnCallFrame',{callFrameId:paused.params.callFrames[0].callFrameId,expression,returnByValue:true});assert(!v.exceptionDetails,'READONLY_SCOPE_EVALUATION_FAILED');result=JSON.parse(v.result.value);}
finally{await rpc('Debugger.removeBreakpoint',{breakpointId:point.breakpointId});await rpc('Debugger.resume');await rpc('Debugger.disable');ws.close();}
const leaf=process.argv[3]??'peer-cached-frames-readonly.json';assert(/^[a-z][a-z0-9-]*\.json$/.test(leaf));result.inspectedAt=new Date().toISOString();
await writeFile('D:/mir2-builds/r16-parity-20261109/social-budget-ui-r1/browser-r3/'+leaf,JSON.stringify(result,null,2),{flag:'wx'});
console.log(JSON.stringify({readOnly:true,peerActive:result.projection.tradeActive,peerUncertain:result.projection.uncertain,cachedFrames:result.cachedFramesExamined,terminalPackets:result.rows.filter(r=>r.packet==='TradeCancel').length,lastSnapshotTradeNull:result.rows.filter(r=>r.type==='worldSnapshot').at(-1)?.tradeIsNull}));
