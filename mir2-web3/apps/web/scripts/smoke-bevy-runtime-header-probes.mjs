// Actual Chrome transport faults; this is not a game or public timing qualification.
import assert from 'node:assert/strict';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {createRequire} from 'node:module';
import {resolve,join} from 'node:path';
import ts from 'typescript';

assert(process.argv.length===3&&process.env.MIR2_CHROME_BIN,'pass a fresh directory and MIR2_CHROME_BIN');
const output=resolve(process.argv[2]);await mkdir(output,{recursive:false});
const {chromium}=createRequire(resolve(process.env.MIR2_PLAYWRIGHT_PACKAGE??'package.json'))('playwright');
const source=await readFile(new URL('../lib/bevy-runtime-initializer.ts',import.meta.url),'utf8');
const compiled=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
const binary=Buffer.from('0061736d010000000105016000017f03020100070a0106616e7377657200000a06010400412a0b','hex');
const requests=[],counts=new Map(),retired=[],timers=new Set();
let browser;
const server=createServer((req,res)=>{
 if(req.url==='/initializer.js'){res.writeHead(200,{'content-type':'application/javascript'});res.end(compiled);return;}
 const match=req.url.match(/^\/(headers|retired|denied|slow)\/(app|mirror)\.wasm$/);
 if(!match){res.writeHead(200,{'content-type':'text/html'});res.end('<!doctype html><title>Runtime headers</title>');return;}
 const[,scenario,lane]=match,count=(counts.get(req.url)??0)+1;counts.set(req.url,count);
 const row={scenario,lane,count,at:Date.now(),headersSent:false};requests.push(row);
 if(scenario==='slow'){
  if(lane==='app'){row.headersSent=true;res.writeHead(503);res.end();return;}
  const timer=setTimeout(()=>{timers.delete(timer);row.headersSent=true;res.writeHead(200,{'content-type':'application/wasm'});res.end(binary);},9000);
  timers.add(timer);res.once('close',()=>{clearTimeout(timer);timers.delete(timer);});return;
 }
 if(scenario==='retired'||scenario==='headers'&&count===1){
  res.once('close',()=>{if(!res.writableEnded)retired.push({scenario,lane,count,at:Date.now()});});return;
 }
 row.headersSent=true;res.writeHead(scenario==='denied'?403:200,{'content-type':'application/wasm','cache-control':'no-store'});
 res.end(scenario==='denied'?undefined:binary);
});
const proof={passed:false,scope:'Actual Chrome, loopback no-header faults and original WASM compilation; no external account/game/public performance claim',
 sourceSha256:createHash('sha256').update(source).digest('hex'),closed:{browser:false,server:false}};
try{
 await new Promise((ok,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',ok);});
 const origin='http://127.0.0.1:'+server.address().port;
 browser=await chromium.launch({executablePath:process.env.MIR2_CHROME_BIN,headless:true});proof.chromeVersion=browser.version();
 const page=await browser.newPage(),origins=new Set(),pageErrors=[];
 page.on('request',r=>origins.add(new URL(r.url()).origin));page.on('pageerror',e=>pageErrors.push(e.message));await page.goto(origin);
 proof.cases=await page.evaluate(async origin=>{
  const{initializeBevyRuntime}=await import(origin+'/initializer.js');const results=[];
  for(const scenario of ['headers','retired','denied','slow']){
   const owner=new AbortController(),signals=[],requests=[],events=[];
   let imports=0,canvases=0,initialized=0,recovery=0,answer=null,errorName=null;
   const started=Date.now();
   try{
    await initializeBevyRuntime({wasmUrl:origin+'/'+scenario+'/app.wasm',wasmFallbackUrls:[origin+'/'+scenario+'/mirror.wasm'],timeoutMs:120000,
     ownerSignal:owner.signal,takeRecovery:()=>{recovery++;return true;},onDownloadAttempt:e=>events.push(e),
     loadModule:async()=>{imports++;return{async default({module_or_path}){initialized++;answer=(await WebAssembly.instantiate(module_or_path)).exports.answer();}};},
     waitForCanvas:async()=>{canvases++;},fetchImpl:request=>{
      requests.push({path:new URL(request.url).pathname,atMs:Date.now()-started,priorAborted:signals.every(s=>s.aborted)});signals.push(request.signal);
      if(scenario==='retired'&&requests.length===2)queueMicrotask(()=>owner.abort());
      return fetch(request);
     }});
   }catch(error){errorName=error.name;}
   results.push({scenario,elapsedMs:Date.now()-started,requests,events,imports,canvases,initialized,recovery,answer,errorName,allRequestsRetired:signals.every(s=>s.aborted)});
  }
  return results;
 },origin);
 assert.deepEqual([...origins],[origin]);assert.deepEqual(pageErrors,[]);
 for(const row of proof.cases){
  assert.equal(row.imports,1);assert.equal(row.recovery,0);assert.equal(row.allRequestsRetired,true);
  if(row.scenario==='headers'){
   assert.deepEqual(row.requests.map(r=>r.path),['/headers/app.wasm','/headers/mirror.wasm','/headers/app.wasm']);
   assert(row.requests.every(r=>r.priorAborted));assert.equal(row.errorName,null);assert.equal(row.answer,42);
   assert.equal(row.initialized,1);assert.equal(row.canvases,1);
   assert(row.elapsedMs>=15900&&row.elapsedMs<30000,'two real 8-second probes remain bounded');
   assert.deepEqual(row.events.filter(e=>e.state==='failed').map(e=>[e.reason,e.bytes,e.retryable]),[['headers-timeout',0,true],['headers-timeout',0,true]]);
   assert.equal(row.events.at(-1).bytes,binary.length);
  }else if(row.scenario==='slow'){
   assert.deepEqual(row.requests.map(r=>r.path),['/slow/app.wasm','/slow/mirror.wasm','/slow/mirror.wasm']);
   assert(row.requests.every(r=>r.priorAborted));assert.equal(row.errorName,null);assert.equal(row.answer,42);
   assert.equal(row.initialized,1);assert.equal(row.canvases,1);
   assert(row.elapsedMs>=16900&&row.elapsedMs<30000,'a viable 9-second mirror retains its final header budget');
  }else{
   assert.equal(row.initialized,0);assert.equal(row.canvases,0);assert.equal(row.requests.length,2);
   assert.equal(row.errorName,row.scenario==='retired'?'AbortError':'TypeError');
  }
 }
 proof.requests=requests;proof.retiredHeaders=retired;proof.pageErrors=pageErrors;proof.passed=true;
}catch(error){proof.failure={name:error.name,message:error.message};process.exitCode=1;}
finally{
 if(browser){await browser.close();proof.closed.browser=true;}
 for(const timer of timers)clearTimeout(timer);
 server.closeAllConnections();await new Promise(ok=>server.close(ok));proof.closed.server=true;
 await writeFile(join(output,'receipt.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({passed:proof.passed,cases:proof.cases?.length??0,closed:proof.closed}));
}
