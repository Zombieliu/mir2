// Same-source Chrome ordering control. Its deliberate three-slot server is not a WAN model.
import assert from'node:assert/strict';
import{readFile,writeFile,mkdir}from'node:fs/promises';
import{createHash}from'node:crypto';
import{createRequire}from'node:module';
import{createServer}from'node:http';
import{resolve,join}from'node:path';
import ts from'typescript';
import{startStaticTransportProbe}from'./lib/r16-static-transport.mjs';
assert(process.argv.length===3&&process.env.MIR2_CHROME_BIN,'pass fresh evidence directory and MIR2_CHROME_BIN');
const out=resolve(process.argv[2]);await mkdir(out,{recursive:false});
const{chromium}=createRequire(resolve(process.env.MIR2_PLAYWRIGHT_PACKAGE??'package.json'))('playwright');
const modules=new Map(),sourceHashes={};const sha=b=>createHash('sha256').update(b).digest('hex');
const compile=s=>ts.transpileModule(s,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
for(const[name,path]of[['priority','runtime-header-priority'],['initializer','bevy-runtime-initializer'],['orchestrator','asset-orchestrator'],['asset-request','asset-prewarm-request']]){
 const source=await readFile(new URL('../lib/'+path+'.ts',import.meta.url),'utf8');modules.set('/'+name+'.js',compile(source));sourceHashes[path]=sha(source);
}
const registrar=await readFile(new URL('../app/components/asset-cache-registrar.tsx',import.meta.url),'utf8');sourceHashes.registrar=sha(registrar);
const ast=ts.createSourceFile('registrar.tsx',registrar,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let callback;
const visit=n=>{if(ts.isNewExpression(n)&&n.expression.getText(ast)==='AssetPrewarmOrchestrator')callback=n.arguments[0].properties.find(p=>p.name?.getText(ast)==='run').initializer;ts.forEachChild(n,visit);};visit(ast);assert(callback);
modules.set('/runner.js',compile("import{getRuntimeHeaderPriority}from'/priority.js';\nexport function createRunner({metrics,prewarmAssetPacks}){const manifest={},prewarmPolicy={},reconfigureWorker=undefined,disposed=false,setSnapshot=()=>{};return "+callback.getText(ast)+";}"));
const wasm=Buffer.from('0061736d010000000105016000017f03020100070a0106616e7377657200000a06010400412a0b','hex');
const png=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jB3sAAAAASUVORK5CYII=','base64');
const groups=new Map(),requests=[],timers=new Set();let browser;
function group(name){if(!groups.has(name))groups.set(name,{active:0,queue:[]});return groups.get(name);}
function admit(g){while(g.active<3&&g.queue.length){const run=g.queue.shift();run();}}
const server=createServer((req,res)=>{
 if(modules.has(req.url)){res.writeHead(200,{'content-type':'application/javascript','cache-control':'no-store'});res.end(modules.get(req.url));return;}
 const status=/^\/(control|priority|timeout|retired)\/status$/.exec(req.url);
 if(status){res.writeHead(200,{'content-type':'application/json'});res.end(JSON.stringify({active:group(status[1]).active}));return;}
 const match=/^\/(control|priority|timeout|retired)\/(?:original-ui\/Prguse\/(\d+)\.png|bevy-runtime\/v\/fixed\/pkg\/runtime\.wasm)$/.exec(req.url);
 if(!match){res.writeHead(200,{'content-type':'text/html'});res.end('<!doctype html><title>Runtime header ordering control</title>');return;}
 const[,scenario,index]=match,isWasm=index===undefined,g=group(scenario),row={scenario,kind:isWasm?'runtime':'prewarm',index:isWasm?null:Number(index),queuedAt:Date.now()};requests.push(row);
 let started=false,ended=false,timer;
 const finish=()=>{if(ended)return;ended=true;clearTimeout(timer);timers.delete(timer);row.finishedAt=Date.now();row.complete=res.writableEnded;if(started){g.active--;admit(g);}};
 res.once('close',finish);
 const run=()=>{
  if(ended)return;started=true;g.active++;row.admittedAt=Date.now();
  if(scenario==='timeout'&&isWasm)return;
  res.writeHead(200,{'content-type':isWasm?'application/wasm':'image/png','cache-control':'no-store','age':'0','x-vercel-cache':'MISS','cf-ray':'abcdef0123456789-NRT','set-cookie':'synthetic-private-cookie=test'});res.flushHeaders();row.headersAt=Date.now();
  if(isWasm)res.write(wasm.subarray(0,4));
  const delay=isWasm?2000:Number(index)<3?4000:1;
  timer=setTimeout(()=>{timers.delete(timer);res.end(isWasm?wasm.subarray(4):png);},delay);timers.add(timer);
 };
 g.queue.push(run);admit(g);
});
const proof={schema:1,passed:false,scope:'Same compiled code, actual Chrome/HTTP/WASM and actual registrar callback. A deliberately bounded three-response server demonstrates ordering only; not a public root-cause, game or Native120 qualification.',sourceHashes,expectedRuntimeSha256:sha(wasm),expectedPngSha256:sha(png),cases:[],closed:{browser:false,server:false}};
try{
 await new Promise((ok,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',ok);});const origin='http://127.0.0.1:'+server.address().port;
 browser=await chromium.launch({executablePath:process.env.MIR2_CHROME_BIN,headless:true});proof.browserVersion=browser.version();
 for(const scenario of['control','priority','timeout','retired']){
  const context=await browser.newContext(),page=await context.newPage(),began=Date.now();let pageErrors=0;page.on('pageerror',()=>pageErrors++);
  const probe=await startStaticTransportProbe({context,elapsed:()=>Date.now()-began,sourceBases:[origin+'/'+scenario]});
  try{
   await page.goto(origin);
   const result=await page.evaluate(async({origin,scenario,pngSha})=>{
    const[{getRuntimeHeaderPriority},{initializeBevyRuntime},{AssetPrewarmOrchestrator},{fetchPrewarmAsset},{createRunner}]=await Promise.all([import('/priority.js'),import('/initializer.js'),import('/orchestrator.js'),import('/asset-request.js'),import('/runner.js')]);
    const gate=getRuntimeHeaderPriority(),owner=new AbortController(),events=[],started=performance.now();let next=0,completed=0,initialized=0,answer=null,headersMs=null,runtimeMs=null;
    if(scenario!=='control')gate.beginCycle();const release=gate.captureRelease();
    const run=createRunner({metrics:{markMilestone:(name,data)=>events.push({name,...data}),snapshot:()=>({})},prewarmAssetPacks:async()=>{
     const consume=async()=>{for(;;){const i=next++;if(i>=113)return;const r=await fetchPrewarmAsset(origin+'/'+scenario+'/original-ui/Prguse/'+i+'.png',{signal:owner.signal});
      const hash=[...new Uint8Array(await crypto.subtle.digest('SHA-256',r.body))].map(x=>x.toString(16).padStart(2,'0')).join('');if(hash!==pngSha)throw Error('prewarm body mismatch');completed++;}};
     await Promise.all([consume(),consume(),consume()]);
    }});
    const orchestrator=new AssetPrewarmOrchestrator({backgroundMode:'afterPlayable',hasWork:(_s,lane)=>lane==='background',run});
    const warm=orchestrator.requestStage('game');void orchestrator.markFirstPlayable();
    if(scenario==='retired'){gate.cancelCycle();await warm;orchestrator.dispose();return{scenario,completed,initialized,events,elapsedMs:performance.now()-started};}
    if(scenario==='control'){
     let ready=false;for(const until=performance.now()+3000;performance.now()<until&&!ready;){ready=(await(await fetch(origin+'/'+scenario+'/status')).json()).active===3;if(!ready)await new Promise(r=>setTimeout(r,10));}if(!ready)throw Error('control server slots not occupied');
    }
    const runtime=initializeBevyRuntime({loadModule:async()=>({async default({module_or_path}){initialized++;answer=(await WebAssembly.instantiate(module_or_path)).exports.answer();}}),
     wasmUrl:origin+'/'+scenario+'/bevy-runtime/v/fixed/pkg/runtime.wasm',timeoutMs:120000,ownerSignal:owner.signal,waitForCanvas:async()=>{},onResponseHeaders:()=>{headersMs=performance.now()-started;release('headers');}})
     .then(()=>{runtimeMs=performance.now()-started;return null;},e=>e.name);
    await warm;const completedAtMs=performance.now()-started;if(scenario==='timeout')owner.abort();const runtimeError=await runtime;release('finished');orchestrator.dispose();
    return{scenario,completed,completedAtMs,headersMs,runtimeMs,runtimeError,initialized,answer,events,elapsedMs:performance.now()-started};
   },{origin,scenario,pngSha:sha(png)});
   result.transport=await probe.stop();result.pageErrors=pageErrors;proof.cases.push(result);
  }finally{await probe.stop();await context.close();}
 }
 const[control,priority,timeout,retired]=proof.cases;
 for(const row of[control,priority,timeout]){assert.equal(row.completed,113);assert.equal(row.pageErrors,0);assert(row.transport.cleanupComplete);assert(!JSON.stringify(row.transport).includes('synthetic-private-cookie'));}
 assert(control.headersMs>=3900);assert(priority.headersMs<2000);assert(control.runtimeMs-priority.runtimeMs>2500);
 for(const row of[control,priority]){assert.equal(row.answer,42);assert.equal(row.initialized,1);assert.equal(row.runtimeError,null);assert.equal(row.transport.capturedRows,114);}
 assert.equal(priority.events[0].reason,'headers');assert.equal(timeout.events[0].reason,'timeout');assert(timeout.events[0].waitedMs>=7900&&timeout.events[0].waitedMs<11000);
 assert.equal(timeout.initialized,0);assert.equal(timeout.runtimeError,'AbortError');assert.equal(retired.completed,0);assert.equal(retired.transport.capturedRows,0);
 proof.requests=requests;proof.passed=true;
}catch(e){proof.failure={name:e.name,message:e.message};process.exitCode=1;}
finally{
 if(browser){await browser.close();proof.closed.browser=true;}for(const timer of timers)clearTimeout(timer);
 server.closeAllConnections();await new Promise(ok=>server.close(ok));proof.closed.server=true;
 await writeFile(join(out,'receipt.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({passed:proof.passed,cases:proof.cases.map(({scenario,completed,headersMs,runtimeMs})=>({scenario,completed,headersMs,runtimeMs})),closed:proof.closed}));
}
