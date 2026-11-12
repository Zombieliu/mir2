import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const pagePath=new URL('../app/page.tsx',import.meta.url);
const libraryPath=new URL('../lib/bevy-runtime-font-prewarm.ts',import.meta.url);
const turn=()=>new Promise(resolve=>setImmediate(resolve));
function compile(source,dependencies={}){
 const module={exports:{}};
 const js=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 new Function('module','exports','require',js)(module,module.exports,name=>{assert(name in dependencies,name);return dependencies[name];});
 return module.exports;
}
function actualBoot({playable=true,mode='eager',allowed=true,support={webgpu:true,webgl2:true}}={}){
 const source=readFileSync(pagePath,'utf8'),ast=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 let callback;
 const visit=node=>{if(ts.isCallExpression(node)&&node.expression.getText(ast)==='useEffect'){
  const candidate=node.arguments[0];if(candidate&&candidate.getText(ast).includes('async function bootRuntime()'))callback=candidate;
 }ts.forEachChild(node,visit);};visit(ast);assert(callback);
 const events=[];let finishModule,failModule,finishFonts,observed=Promise.resolve();
 const fontSignals=[];
 const noop=()=>{},ref=()=>({current:null});
 const env={
  AbortController,window:{location:{search:''},matchMedia:()=>({matches:false})},navigator:{maxTouchPoints:0,userAgent:'Chrome desktop'},
  URLSearchParams,console:{warn:noop,info:noop},shouldBootBevyRuntime:playable,screen:'game',
  clientProfile:{input:'keyboardMouse',layout:'desktop'},bevyRuntimeStartup:{runtimeAllowed:allowed},
  BEVY_RUNTIME_VERSION:'pinned',BEVY_RUNTIME_ASSET_BASE_URL:'https://assets.example.test/release',
  resolveBevyRuntimeBootDecision:()=>({mode,reason:'test'}),detectBevyRuntimeSupport:async()=>support,
  selectBevyRuntimeBackend:(_params,support)=>support.webgpu?'webgpu':support.webgl2?'webgl2':null,
  appendLog:noop,t:key=>key,markMir2CacheMilestone:(name,detail)=>events.push({name,detail}),
  setBevyRuntimeStarted:noop,setRuntimePhase:noop,setRuntimeMessage:noop,setBevyEntityRendererReady:noop,setBevyRuntimeBackend:noop,
  resetBevyMapImageResidency:noop,withdrawHudBarDrawPlanOutputs:noop,setBevyRuntimeGeneration:noop,
  shouldRetryBevyRuntimeWithWebGl2:()=>false,canRetryBevyRuntimeBeforeBoot:()=>true,
  isBevyRuntimeNetworkFailure:()=>true,scheduleBevyRuntimeCacheRecovery:()=>false,
  runtimeRef:ref(),lastBevyEntityRenderStateJsonRef:ref(),lastBevyMapRenderStateJsonRef:ref(),lastBevyMapCameraOffsetRef:ref(),
  prewarmBevyRuntimeFonts:options=>{events.push({name:'fonts'});fontSignals.push(options.signal);return new Promise(resolve=>{finishFonts=resolve;});},
  loadBevyRuntimeModule:()=>{events.push({name:'module'});return new Promise((resolve,reject)=>{finishModule=resolve;failModule=reject;});},
  testObserve:value=>{observed=value;},
 };
 const body=callback.body.getText(ast).replace('void bootRuntime();','testObserve(bootRuntime());');
 assert(body.includes('testObserve(bootRuntime());'));
 const js=ts.transpileModule('function effect()'+body,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 const cleanup=new Function(...Object.keys(env),js+'\nreturn effect();')(...Object.values(env));
 return{events,fontSignals,cleanup,window:env.window,failModule:error=>failModule(error),waitForBoot:()=>observed,
  settle:async()=>{cleanup();finishModule?.({});finishFonts?.({requested:6,completed:[],failed:[],aborted:true,bytes:0});await observed;await turn();}};
}

test('actual Page starts fonts while its renderer module is still pending, and cancels on leaving',async()=>{
 const run=actualBoot();
 try{await turn();assert(run.events.some(e=>e.name==='module'));
  assert(run.events.some(e=>e.name==='fonts'),'fonts must start before the pending renderer download finishes');
  assert(run.events.findIndex(e=>e.name==='fonts')<run.events.findIndex(e=>e.name==='module'));
  assert.equal(run.fontSignals[0].aborted,false);run.cleanup();assert(run.fontSignals[0].aborted);
 }finally{await run.settle();}
});

test('actual Page leaves fonts deferred before first paint and for disabled, touch, missing metadata or GPU',async()=>{
 for(const options of [{playable:false},{mode:'disabled'},{mode:'compatibility'},{allowed:false},{support:{webgpu:false,webgl2:false}}]){
  const run=actualBoot(options);try{await turn();assert(!run.events.some(e=>e.name==='fonts'));assert(!run.events.some(e=>e.name==='module'));}
  finally{await run.settle();}
 }
});

test('actual retryable module failure cancels font work without losing the retry boundary',async()=>{
 const run=actualBoot();try{await turn();run.failModule(new TypeError('Failed to fetch'));
  await run.waitForBoot();assert(run.fontSignals[0].aborted);
  assert.equal(run.window.__mir2BevyRuntimeRetryable,true);assert.equal(run.window.__mir2BevyRuntimeBooted,false);
 }finally{await run.settle();}
});

const request=compile(readFileSync(new URL('../lib/asset-prewarm-request.ts',import.meta.url),'utf8'));
const library=existsSync(libraryPath)?compile(readFileSync(libraryPath,'utf8'),{'./asset-prewarm-request':request}):{};

test('prewarm paths exactly cover the Native display font families',()=>{
 const rust=readFileSync(new URL('../../game-client/client-bevy/src/quest_presentation_text.rs',import.meta.url),'utf8');
 const fonts=[...rust.matchAll(/"(original-ui\/fonts\/[^"\n]+\.(?:ttf|otf))"/g)].map(m=>'/'+m[1]);
 assert.equal(new Set(fonts).size,6);assert.deepEqual([...library.BEVY_RUNTIME_FONT_PATHS].sort(),[...new Set(fonts)].sort());
});

test('two pending font bodies bound concurrency and completion waits for full bytes',async()=>{
 const streams=[],requests=[];let active=0,maxActive=0,done=false;
 const pending=library.prewarmBevyRuntimeFonts({fetchImpl:async path=>{
  requests.push(path);active++;maxActive=Math.max(maxActive,active);
  return new Response(new ReadableStream({start(controller){streams.push({finish(){controller.enqueue(new Uint8Array([1,2,3]));controller.close();active--;}});}}));
 }}).then(value=>{done=true;return value;});
 await turn();assert.equal(requests.length,2);assert.equal(done,false);
 for(let n=0;n<6;n++){assert(streams[n]);streams[n].finish();await turn();}
 const result=await pending;assert.equal(maxActive,2);assert.equal(new Set(requests).size,6);
 assert.equal(result.completed.length,6);assert.equal(result.failed.length,0);assert.equal(result.bytes,18);
});

test('leaving cancels active font bodies and never starts the queued fonts',async()=>{
 const owner=new AbortController();let calls=0,cancelled=0;
 const pending=library.prewarmBevyRuntimeFonts({signal:owner.signal,fetchImpl:async()=>{
  calls++;return new Response(new ReadableStream({start(controller){controller.enqueue(new Uint8Array([1]));},cancel(){cancelled++;}}));
 }});
 await turn();assert.equal(calls,2);owner.abort();const result=await pending;
 assert(result.aborted);assert.equal(calls,2);assert.equal(cancelled,2);assert.equal(result.completed.length,0);
});

test('a failed font does not withdraw the runtime or prevent warming the other families',async()=>{
 const failed=library.BEVY_RUNTIME_FONT_PATHS?.[0];
 const result=await library.prewarmBevyRuntimeFonts({fetchImpl:async path=>new Response(new Uint8Array([1,2]),{status:path===failed?503:200})});
 assert.equal(result.completed.length,5);assert.equal(result.failed.length,1);assert.equal(result.failed[0].path,failed);
 assert.equal(result.aborted,false);
});

test('the slow CJK families do not wait behind short fallback requests',async t=>{
 t.mock.timers.enable({apis:['setTimeout']});
 let done=false;
 const owner=new AbortController();
 const pending=library.prewarmBevyRuntimeFonts({signal:owner.signal,fetchImpl:async path=>{
  if(!/NotoSans(?:CJKsc-Regular\.otf|TC\.ttf)$/.test(path)){
   await new Promise(resolve=>setTimeout(resolve,4000));return new Response(new Uint8Array([1]));
  }
  const chunks=path.endsWith('.otf')?6:2;let count=0,timer,cancelled=false;
  return new Response(new ReadableStream({start(stream){const emit=()=>{
   if(cancelled)return;stream.enqueue(new Uint8Array([1]));
   if(++count===chunks)stream.close();else timer=setTimeout(emit,10000);
  };timer=setTimeout(emit,10000);},cancel(){cancelled=true;clearTimeout(timer);}}));
 }}).then(result=>{done=true;return result;});
 try{
  await turn();for(let second=0;second<60;second++){t.mock.timers.tick(1000);await turn();}
  assert(done,'all families should finish by the slowest CJK body; small headers must not delay its start');
  assert.equal((await pending).completed.length,6);
 }finally{owner.abort();await pending;await turn();}
});
