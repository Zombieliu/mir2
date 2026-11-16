import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
/** Passive QA logpoint on the original glue return. Never pauses or replaces a function. */
export async function installRoutePlanProbe({page,gluePath,glueUrlPath}) {
 const bytes=await readFile(gluePath),lines=bytes.toString().split(/\r?\n/);
 const start=lines.findIndex(l=>l.startsWith('export function getMir2QuestRoutePlan('));
 const offset=lines.slice(start,start+25).findIndex(l=>l.includes('return getStringFromWasm0(ret[0], ret[1])'));
 if(start<0||offset<0||!glueUrlPath.startsWith('/bevy-runtime/v/')||!glueUrlPath.endsWith('/mir2_bevy_runtime.js'))throw Error('unrecognized pinned planner glue');
 await page.evaluate(()=>{window.__r16PlannerObservation={rows:[],errors:0,dropped:0};});
 const cdp=await page.context().newCDPSession(page);let paused=0;
 cdp.on('Debugger.paused',()=>{paused++;void cdp.send('Debugger.resume').catch(()=>{});});
 await cdp.send('Debugger.enable');
 const escaped=glueUrlPath.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 const condition="(()=>{try{const o=window.__r16PlannerObservation;if(!o)return false;if(o.rows.length>=256){o.dropped++;return false;}const q=JSON.parse(query_json),r=JSON.parse(getStringFromWasm0(ret[0],ret[1]));o.rows.push({at:Date.now(),phase:window.__r16PlannerPhase??'unmarked',query:q,result:r});}catch{if(window.__r16PlannerObservation)window.__r16PlannerObservation.errors++;}return false;})()";
 const bp=await cdp.send('Debugger.setBreakpointByUrl',{urlRegex:escaped+'(?:\\?.*)?$',lineNumber:start+offset,condition});
 if(!bp.locations.length)throw Error('loaded planner return logpoint not bound');
 return {source:{path:gluePath,sha256:createHash('sha256').update(bytes).digest('hex'),glueUrlPath,lineNumber:start+offset,locations:bp.locations.length},
  async snapshot(){return{...await page.evaluate(()=>window.__r16PlannerObservation),paused};},
  async stop(){await cdp.send('Debugger.removeBreakpoint',{breakpointId:bp.breakpointId});await cdp.send('Debugger.disable');await cdp.detach();return{...await page.evaluate(()=>window.__r16PlannerObservation),paused,closed:true};}
 };
}
