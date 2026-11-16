// Bounded orchestration of the retained QA helper's existing ordinary peer API.
// Primary player actions remain real browser UI; no game Runtime is invoked.
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {setTimeout as delay} from 'node:timers/promises';
const target=(await(await fetch('http://127.0.0.1:9229/json/list')).json())[0];
const ws=new WebSocket(target.webSocketDebuggerUrl);await new Promise((r,j)=>{ws.addEventListener('open',r,{once:true});ws.addEventListener('error',j,{once:true});});
let seq=0;const pending=new Map(),events=[];
ws.addEventListener('message',event=>{const msg=JSON.parse(event.data);if(msg.id){const p=pending.get(msg.id);pending.delete(msg.id);msg.error?p.reject(Error(msg.error.message)):p.resolve(msg.result);}else events.push(msg);});
function rpc(method,params={}){const id=++seq;return new Promise((resolve,reject)=>{pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});}
await rpc('Debugger.enable');let parsed;for(let i=0;i<30;i++){parsed=events.find(e=>e.method==='Debugger.scriptParsed'&&e.params.url.endsWith('/social-ui.mjs'));if(parsed)break;await delay(100);}assert(parsed);
const source=await rpc('Debugger.getScriptSource',{scriptId:parsed.params.scriptId});const lines=source.scriptSource.split('\n'),lineNumber=lines.findIndex(line=>line.includes("input.on('line',v=>{if(v.trim()==='close')"));assert(lineNumber>=0);
const point=await rpc('Debugger.setBreakpoint',{location:{scriptId:parsed.params.scriptId,lineNumber,columnNumber:lines[lineNumber].indexOf("if(v.trim()==='close')")}});
console.log(JSON.stringify({phase:'ordinary-peer-readback-ready'}));let paused;for(let i=0;i<600;i++){paused=events.find(e=>e.method==='Debugger.paused');if(paused)break;await delay(100);}assert(paused);
const expression=`(async()=>{const packets=[];const capture=frame=>{if(frame.event.type==='packet'&&['LogOutSuccess','StartGame'].includes(frame.event.packet))packets.push({packet:frame.event.packet,result:frame.event.payload?.result});};peer.on('frame',capture);let fresh;try{const firstLogout=await peer.logout();await peer.close();class OriginWs extends Ws{constructor(url){super(url,{origin:config.origin});}}fresh=new GatewayClient({url:config.gatewayUrl,realmId:config.realmId,WebSocketImpl:OriginWs,timeoutMs:20000});fresh.on('frame',capture);await fresh.connect();await fresh.login(state.actor.accountId,state.actor.password);await fresh.startGame(state.actor.characterIndex);const observed=fresh.snapshot(),assets=econ(observed);const assetsMatch=JSON.stringify(assets)===JSON.stringify(proof.assetsBefore.peer);const result={schema:1,passed:false,scope:'Auxiliary ordinary GatewayClient normal logout/password login/same character/StartGame/snapshot/final logout; no trade replay.',firstLogoutAcknowledged:firstLogout,sameCharacterIndex:fresh.activeCharacterIndex===state.actor.characterIndex,ready:observed.ready,tradeActive:observed.trade.active,tradeUncertain:observed.trade.uncertain,exactAssetsMatch:assetsMatch,economy:assets};result.finalLogoutAcknowledged=await fresh.logout();await fresh.close();result.packets=packets;result.passed=firstLogout&&result.finalLogoutAcknowledged&&result.sameCharacterIndex&&result.ready&&!result.tradeActive&&!result.tradeUncertain&&assetsMatch;return result;}finally{peer.off('frame',capture);await fresh?.close().catch(()=>{});}})()`;
let result;
try{
 const v=await rpc('Debugger.evaluateOnCallFrame',{callFrameId:paused.params.callFrames[0].callFrameId,expression,returnByValue:false});assert(!v.exceptionDetails&&v.result.objectId);
 await rpc('Debugger.removeBreakpoint',{breakpointId:point.breakpointId});await rpc('Debugger.resume');
 const awaited=await rpc('Runtime.awaitPromise',{promiseObjectId:v.result.objectId,returnByValue:true});assert(!awaited.exceptionDetails);result=awaited.result.value;
}finally{try{await rpc('Debugger.disable');}catch{}ws.close();}
await writeFile('D:/mir2-builds/r16-parity-20261109/social-budget-ui-r1/browser-r3/peer-ordinary-readback.json',JSON.stringify(result,null,2),{flag:'wx'});
console.log(JSON.stringify({passed:result.passed,normalLogouts:result.packets.filter(p=>p.packet==='LogOutSuccess').length,exactAssets:result.exactAssetsMatch,tradeClosed:!result.tradeActive&&!result.tradeUncertain}));assert(result.passed);
