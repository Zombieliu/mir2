import assert from 'node:assert/strict';
import {readFileSync,existsSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const pagePath=new URL('../app/page.tsx',import.meta.url);
const libraryPath=new URL('../lib/bevy-runtime-font-prewarm.ts',import.meta.url);
const turn=()=>new Promise(resolve=>setImmediate(resolve));
function compile(source,dependencies={},globals={}){
 const module={exports:{}};
 const js=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 new Function('module','exports','require',...Object.keys(globals),js)(module,module.exports,name=>{assert(name in dependencies,name);return dependencies[name];},...Object.values(globals));
 return module.exports;
}
function actualBoot({playable=true,mode='eager',allowed=true,criticalPending=false,support={webgpu:true,webgl2:true},screen='game',recoveryBudget=new Set(),retryWithWebGl2=false,reused=false}={}){
 const source=readFileSync(pagePath,'utf8'),ast=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 let callback;
 const visit=node=>{if(ts.isCallExpression(node)&&node.expression.getText(ast)==='useEffect'){
  const candidate=node.arguments[0];if(candidate&&candidate.getText(ast).includes('async function bootRuntime()'))callback=candidate;
 }ts.forEachChild(node,visit);};visit(ast);assert(callback);
 const events=[];let finishModule,failModule,finishFonts,finishPrimary,failPrimary,finishCritical,failCritical,observed=Promise.resolve();
 const fontSignals=[],primarySignals=[],runtimeOptions=[],runtimeBackends=[];
 const noop=()=>{},ref=()=>({current:null});
 const env={
  AbortController,window:{location:{search:''},matchMedia:()=>({matches:false})},navigator:{maxTouchPoints:0,userAgent:'Chrome desktop'},
  URLSearchParams,console:{warn:noop,info:noop},shouldBootBevyRuntime:playable,screen,
  clientProfile:{input:'keyboardMouse',layout:'desktop'},bevyRuntimeStartup:{runtimeAllowed:allowed},
  BEVY_RUNTIME_VERSION:'pinned',BEVY_RUNTIME_ASSET_BASE_URL:'https://assets.example.test/release',
  resolveBevyRuntimeBootDecision:()=>({mode,reason:'test'}),detectBevyRuntimeSupport:async()=>support,
  selectBevyRuntimeBackend:(_params,support)=>support.webgpu?'webgpu':support.webgl2?'webgl2':null,
  appendLog:noop,t:key=>key,markMir2CacheMilestone:(name,detail)=>events.push({name,detail}),
  handleBevyRuntimeStatus:noop,DEFAULT_WORLD_STATE:{},clearBevyMovementShadow:noop,seedBevyMovementShadowFromWorld:noop,
  getBevyRuntimePackageForBackend:()=>({id:'webgpu'}),assertBevyRuntimeStartupAgreement:noop,
  createBevyRuntimeBootGate:()=>({}),runBevyRuntimeBootOnce:(_gate,boot)=>boot(),
  requestAssetPrewarmStage:stage=>{events.push({name:'critical-assets',stage});return criticalPending?new Promise((resolve,reject)=>{finishCritical=()=>{events.push({name:'critical-settled'});resolve();};failCritical=()=>{events.push({name:'critical-rejected'});reject(Error('HUD prewarm failed'));};}):Promise.resolve();},
  setBevyRuntimeStarted:noop,setRuntimePhase:noop,setRuntimeMessage:noop,setBevyEntityRendererReady:noop,setBevyRuntimeBackend:noop,
  resetBevyMapImageResidency:noop,withdrawHudBarDrawPlanOutputs:noop,setBevyRuntimeGeneration:noop,
  shouldRetryBevyRuntimeWithWebGl2:backend=>retryWithWebGl2&&backend==='webgpu',canRetryBevyRuntimeBeforeBoot:()=>true,
  safeLocalStorageRemove:noop,
  isBevyRuntimeNetworkFailure:()=>true,scheduleBevyRuntimeCacheRecovery:()=>false,
  runtimeRef:ref(),runtimeDownloadRecoveryRef:{current:recoveryBudget},lastBevyEntityRenderStateJsonRef:ref(),lastBevyMapRenderStateJsonRef:ref(),lastBevyMapCameraOffsetRef:ref(),
  prewarmBevyRuntimeFonts:options=>{events.push({name:'fonts'});fontSignals.push(options.signal);return new Promise(resolve=>{finishFonts=resolve;});},
  prewarmBevyRuntimePrimaryFont:options=>{events.push({name:'primary-font'});primarySignals.push(options.signal);return new Promise((resolve,reject)=>{finishPrimary=resolve;failPrimary=reject;});},
  loadBevyRuntimeModule:(backend,_startup,options)=>{runtimeOptions.push(options);runtimeBackends.push(backend);events.push({name:'module',backend});return new Promise((resolve,reject)=>{finishModule=resolve;failModule=reject;});},
  testObserve:value=>{observed=value;},
 };
 // Page now captures the document's header-admission coordinator. Load its
 // real implementation in this browser scope rather than leaving an imported
 // dependency undefined or substituting an unconditional release stub.
 env.getRuntimeHeaderPriority=compile(readFileSync(new URL('../lib/runtime-header-priority.ts',import.meta.url),'utf8'),{},{window:env.window}).getRuntimeHeaderPriority;
 const headerPriority=env.getRuntimeHeaderPriority();headerPriority.beginCycle();
 if(reused)Object.assign(env.window,{__mir2BevyRuntimeBooted:true,__mir2BevyRuntimeBackend:'webgpu',__mir2BevyRuntime:{}});
 const body=callback.body.getText(ast).replace('void bootRuntime();','testObserve(bootRuntime());');
 assert(body.includes('testObserve(bootRuntime());'));
 const js=ts.transpileModule('function effect()'+body,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 const cleanup=new Function(...Object.keys(env),js+'\nreturn effect();')(...Object.values(env));
 return{events,fontSignals,primarySignals,runtimeOptions,runtimeBackends,cleanup,headerPriority,window:env.window,failModule:error=>failModule(error),
  finishPrimary:result=>finishPrimary(result??{requested:1,completed:['/original-ui/fonts/NotoSansCJKsc-Regular.otf'],failed:[],aborted:false,bytes:3}),failPrimary:error=>failPrimary(error),
  finishCritical:()=>finishCritical(),failCritical:()=>failCritical(),finishModule:()=>{const backend=runtimeBackends.at(-1);events.push({name:'module-resolved',backend});finishModule({getMir2RendererBackend:()=>backend,bootMir2Runtime:noop});},waitForBoot:()=>observed,
  settle:async()=>{cleanup();finishCritical?.();await turn();finishModule?.({});finishPrimary?.({requested:1,completed:[],failed:[],aborted:true,bytes:0});finishFonts?.({requested:6,completed:[],failed:[],aborted:true,bytes:0});await observed;await turn();}};
}

test('Page prewarm delegation returns the real shared stage promise',()=>{
 const page=readFileSync(pagePath,'utf8'),ast=ts.createSourceFile('page.tsx',page,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 const fn=ast.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==='requestAssetPrewarmStage');assert(fn);
 const pending=Promise.resolve(),cacheWindow={__mir2AssetCachePrewarmStage:stage=>{assert.equal(stage,'game');return pending;}};
 const js=ts.transpileModule(fn.getText(ast),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 const actual=new Function('window',js+'\nreturn requestAssetPrewarmStage;')(cacheWindow);
 assert.equal(actual('game'),pending);assert.equal(cacheWindow.__mir2PendingAssetPrewarmStage,'game');
});

test('actual Page downloads the runtime without speculative fonts while the HUD stage is pending',async()=>{
 const run=actualBoot({criticalPending:true});try{await turn();assert(run.events.some(e=>e.name==='critical-assets'&&e.stage==='game'));
  assert(run.events.some(e=>e.name==='module'));assert.equal(run.fontSignals.length,0);
  assert(!run.events.some(e=>e.name==='bevyRuntimeForegroundWarmSettled'));
  run.finishModule();await run.waitForBoot();assert(run.events.some(e=>e.name==='bevyRuntimeReady'));
  assert.equal(run.fontSignals.length,1,'all-family prewarm starts without waiting for the independent HUD stage');
  run.finishCritical();await turn();assert.equal(run.events.filter(e=>e.name==='bevyRuntimeForegroundWarmSettled').length,1);
  assert.equal(run.events.filter(e=>e.name==='module').length,1);assert.equal(run.events.filter(e=>e.name==='fonts').length,1);
 }finally{await run.settle();}
});

test('actual Page starts fonts only after the runtime resolves and boots without waiting for fonts',async()=>{
 const run=actualBoot();
 try{await turn();assert(run.events.some(e=>e.name==='module'));
  assert.equal(run.fontSignals.length,0,'pending runtime must not start the six-family prewarm');
  assert(run.events.findIndex(e=>e.name==='critical-assets')<run.events.findIndex(e=>e.name==='module'));
  run.finishModule();await run.waitForBoot();
  assert(run.events.some(e=>e.name==='fonts'));
  assert(run.events.findIndex(e=>e.name==='module-resolved')<run.events.findIndex(e=>e.name==='fonts'));
  assert(run.events.findIndex(e=>e.name==='fonts')<run.events.findIndex(e=>e.name==='bevyRuntimeReady'));
  assert.equal(run.fontSignals[0].aborted,false);run.cleanup();assert(run.fontSignals[0].aborted);
 }finally{await run.settle();}
});

test('the actual Page header callback releases its real document coordinator without starting the full-family prewarm early',async()=>{
 const run=actualBoot({criticalPending:true});try{
  await turn();let released=false;const waiting=run.headerPriority.wait().then(outcome=>{released=true;return outcome;});
  await turn();assert.equal(released,false);assert.equal(run.fontSignals.length,0);
  run.runtimeOptions[0].onResponseHeaders();assert.equal((await waiting).reason,'headers');
  assert.equal(run.fontSignals.length,0);run.finishModule();await run.waitForBoot();assert.equal(run.fontSignals.length,1);
 }finally{await run.settle();}
});

test('the actual Page cleanup retires its real pending document coordinator',async()=>{
 const run=actualBoot();try{
  await turn();const waiting=run.headerPriority.wait();run.cleanup();assert.equal((await waiting).reason,'retired');
  assert.equal(run.fontSignals.length,0);
 }finally{await run.settle();}
});

test('a failed independent HUD stage cannot withdraw an already ready runtime',async()=>{
 const run=actualBoot({criticalPending:true});try{await turn();assert(run.events.some(e=>e.name==='module'));
  run.finishModule();await run.waitForBoot();run.failCritical();await turn();
  assert.equal(run.events.filter(e=>e.name==='bevyRuntimeReady').length,1);
  assert.equal(run.events.filter(e=>e.name==='bevyRuntimeForegroundWarmSettled').length,1);
  assert.equal(run.fontSignals[0].aborted,false);assert.notEqual(run.window.__mir2BevyRuntimeFailed,true);
 }finally{await run.settle();}
});

test('leaving before the runtime or HUD settles prevents late fonts, diagnostics and boot',async()=>{
 const run=actualBoot({criticalPending:true});try{await turn();assert.equal(run.fontSignals.length,0);
  run.cleanup();run.finishCritical();run.finishModule();await run.waitForBoot();await turn();
  assert.equal(run.fontSignals.length,0);
  assert(!run.events.some(e=>e.name==='bevyRuntimeReady'));
  assert(!run.events.some(e=>e.name==='bevyRuntimeForegroundWarmSettled'));
 }finally{await run.settle();}
});

test('actual Page leaves fonts deferred before first paint and for disabled, touch, missing metadata or GPU',async()=>{
 for(const options of [{playable:false},{mode:'disabled'},{mode:'compatibility'},{allowed:false},{support:{webgpu:false,webgl2:false}}]){
  const run=actualBoot(options);try{await turn();assert(!run.events.some(e=>e.name==='fonts'));assert(!run.events.some(e=>e.name==='module'));}
  finally{await run.settle();}
 }
});

test('actual retryable module failure never starts fonts and keeps the retry boundary',async()=>{
 const run=actualBoot();try{await turn();run.failModule(new TypeError('Failed to fetch'));
  await run.waitForBoot();assert.equal(run.fontSignals.length,0);
  assert.equal(run.window.__mir2BevyRuntimeRetryable,true);assert.equal(run.window.__mir2BevyRuntimeBooted,false);
 }finally{await run.settle();}
});

test('leaving while the runtime downloads prevents late fonts and boot',async()=>{
 const run=actualBoot();try{await turn();run.cleanup();run.finishModule();await run.waitForBoot();
  assert.equal(run.fontSignals.length,0);assert(!run.events.some(e=>e.name==='bevyRuntimeReady'));
 }finally{await run.settle();}
});

test('Page defers fonts through finite binary recovery and cancels both owners after successful boot',async()=>{
 const run=actualBoot();try{await turn();const options=run.runtimeOptions[0];assert(options);
  assert.equal(options.ownerSignal.aborted,false);assert.equal(options.takeRecovery('fixed-source'),true);
  await turn();assert.equal(run.fontSignals.length,0);assert.equal(run.events.filter(e=>e.name==='module').length,1);
  run.finishModule();await run.waitForBoot();assert.equal(run.events.filter(e=>e.name==='bevyRuntimeReady').length,1);
  run.cleanup();assert(options.ownerSignal.aborted);assert(run.fontSignals[0].aborted);
 }finally{await run.settle();}
});

test('only the final fallback backend starts fonts, once, before its single boot',async()=>{
 const run=actualBoot({retryWithWebGl2:true});try{
  await turn();run.failModule(new TypeError('Failed to fetch'));await turn();
  assert.deepEqual(run.runtimeBackends,['webgpu','webgl2']);assert.equal(run.fontSignals.length,0);
  run.finishModule();await run.waitForBoot();assert.equal(run.fontSignals.length,1);
  assert.equal(run.events.filter(e=>e.name==='bevyRuntimeReady').length,1);
  assert.equal(run.events.find(e=>e.name==='bevyRuntimeReady').detail.backend,'webgl2');
 }finally{await run.settle();}
});

test('leaving during fallback cannot start fonts from a late module',async()=>{
 const run=actualBoot({retryWithWebGl2:true});try{
  await turn();run.failModule(new TypeError('Failed to fetch'));await turn();
  assert.deepEqual(run.runtimeBackends,['webgpu','webgl2']);run.cleanup();run.finishModule();await run.waitForBoot();
  assert.equal(run.fontSignals.length,0);assert(!run.events.some(e=>e.name==='bevyRuntimeReady'));
 }finally{await run.settle();}
});

test('an already booted runtime is reused without another module or font prewarm',async()=>{
 const run=actualBoot({reused:true});try{
  await run.waitForBoot();assert.equal(run.runtimeOptions.length,0);assert.equal(run.fontSignals.length,0);
  assert.equal(run.events.filter(e=>e.name==='bevyRuntimeReady').length,1);
  assert.equal(run.events.find(e=>e.name==='bevyRuntimeReady').detail.reused,true);
 }finally{await run.settle();}
});

test('Page spends recovery once per entry/source and preserves it across same-game effect reruns',async()=>{
 const budget=new Set(),source='immutable/version/backend.js';
 const first=actualBoot({recoveryBudget:budget});try{await turn();const take=first.runtimeOptions[0].takeRecovery;
  assert.equal(take(source),true);assert.equal(take(source),false);first.cleanup();assert.equal(take('other-source'),false);
 }finally{await first.settle();}
 const sameGame=actualBoot({recoveryBudget:budget});try{await turn();assert.equal(sameGame.runtimeOptions[0].takeRecovery(source),false);}
 finally{await sameGame.settle();}
 const left=actualBoot({screen:'select',playable:false,recoveryBudget:budget});await left.settle();assert.equal(budget.size,0);
 const nextGame=actualBoot({recoveryBudget:budget});try{await turn();assert.equal(nextGame.runtimeOptions[0].takeRecovery(source),true);}
 finally{await nextGame.settle();}
});

const request=compile(readFileSync(new URL('../lib/asset-prewarm-request.ts',import.meta.url),'utf8'));
const library=existsSync(libraryPath)?compile(readFileSync(libraryPath,'utf8'),{'./asset-prewarm-request':request}):{};

test('actual Page starts one primary font on valid headers before the pending runtime body, once',async()=>{
 const run=actualBoot({criticalPending:true});try{
  await turn();assert.equal(run.primarySignals.length,0);assert.equal(run.fontSignals.length,0);
  run.runtimeOptions[0].onResponseHeaders();await turn();assert.equal(run.primarySignals.length,1);assert.equal(run.fontSignals.length,0);
  run.runtimeOptions[0].onResponseHeaders();assert.equal(run.primarySignals.length,1);
  assert(!run.events.some(e=>e.name==='bevyRuntimeReady'));
  run.finishModule();await run.waitForBoot();assert(run.events.some(e=>e.name==='bevyRuntimeReady'));assert.equal(run.fontSignals.length,1);
  assert.equal(run.primarySignals.length,1,'runtime boot does not await or duplicate the pending single-font read');
 }finally{await run.settle();}
});

test('one early primary font survives a backend fallback without spending another font admission',async()=>{
 const run=actualBoot({retryWithWebGl2:true});try{
  await turn();run.runtimeOptions[0].onResponseHeaders();await turn();assert.equal(run.primarySignals.length,1);
  run.failModule(new TypeError('Failed to fetch'));await turn();assert.deepEqual(run.runtimeBackends,['webgpu','webgl2']);
  run.runtimeOptions[1].onResponseHeaders();assert.equal(run.primarySignals.length,1);assert.equal(run.fontSignals.length,0);
  run.finishModule();await run.waitForBoot();assert.equal(run.fontSignals.length,1);assert.equal(run.events.filter(e=>e.name==='bevyRuntimeReady').length,1);
 }finally{await run.settle();}
});

test('leaving before or after headers cancels the primary owner and ignores late callbacks',async()=>{
 for(const beforeHeaders of[true,false]){
  const run=actualBoot();try{await turn();if(!beforeHeaders)run.runtimeOptions[0].onResponseHeaders();await turn();
   assert.equal(run.primarySignals.length,beforeHeaders?0:1);run.cleanup();run.runtimeOptions[0].onResponseHeaders();
   assert.equal(run.primarySignals.length,beforeHeaders?0:1);assert(run.primarySignals.every(s=>s.aborted));
   if(!beforeHeaders)run.finishPrimary();run.finishModule();await run.waitForBoot();await turn();
   assert.equal(run.fontSignals.length,0);assert(!run.events.some(e=>e.name==='bevyRuntimePrimaryFontPrewarmComplete'));assert(!run.events.some(e=>e.name==='bevyRuntimeReady'));
  }finally{await run.settle();}
 }
});

test('primary failure cannot fail the runtime, and runtime failure cannot publish late primary completion',async()=>{
 const run=actualBoot();try{await turn();run.runtimeOptions[0].onResponseHeaders();await turn();run.failPrimary(new TypeError('font source unavailable'));await turn();
  assert.notEqual(run.window.__mir2BevyRuntimeFailed,true);run.finishModule();await run.waitForBoot();assert.equal(run.fontSignals.length,1);assert(run.events.some(e=>e.name==='bevyRuntimeReady'));
 }finally{await run.settle();}
 const failed=actualBoot();try{await turn();failed.runtimeOptions[0].onResponseHeaders();await turn();assert.equal(failed.primarySignals.length,1);
  failed.failModule(new TypeError('Failed to fetch'));await failed.waitForBoot();assert(failed.primarySignals[0].aborted);failed.finishPrimary();await turn();
  assert(!failed.events.some(e=>e.name==='bevyRuntimePrimaryFontPrewarmComplete'));assert.equal(failed.fontSignals.length,0);
 }finally{await failed.settle();}
});

test('primary prewarm reads only the original SC body and waits for all bytes',async()=>{
 assert.equal(typeof library.prewarmBevyRuntimePrimaryFont,'function');
 const calls=[];let body,done=false;
 const pending=library.prewarmBevyRuntimePrimaryFont({fetchImpl:async path=>{calls.push(path);return new Response(new ReadableStream({start(c){body=c;c.enqueue(new Uint8Array([1,2]));}}));}}).then(r=>{done=true;return r;});
 await turn();assert.deepEqual(calls,['/original-ui/fonts/NotoSansCJKsc-Regular.otf']);assert.equal(done,false);
 body.enqueue(new Uint8Array([3,4]));body.close();const result=await pending;assert.equal(result.requested,1);assert.equal(result.bytes,4);assert.equal(result.completed.length,1);assert.equal(result.failed.length,0);
});

test('failed early SC read does not poison the later full six-family pass',async()=>{
 assert.equal(typeof library.prewarmBevyRuntimePrimaryFont,'function');
 const failed=await library.prewarmBevyRuntimePrimaryFont({fetchImpl:async()=>new Response(null,{status:503})});assert.equal(failed.failed.length,1);assert.equal(failed.completed.length,0);
 const paths=[];const full=await library.prewarmBevyRuntimeFonts({fetchImpl:async path=>{paths.push(path);return new Response(new Uint8Array([1,2,3]));}});
 assert.equal(paths.length,6);assert.equal(full.completed.length,6);assert.equal(full.failed.length,0);
});

test('cancelling a primary read cannot start any other family',async()=>{
 assert.equal(typeof library.prewarmBevyRuntimePrimaryFont,'function');
 const owner=new AbortController(),calls=[];
 const pending=library.prewarmBevyRuntimePrimaryFont({signal:owner.signal,fetchImpl:async path=>{calls.push(path);return new Response(new ReadableStream({start(c){c.enqueue(new Uint8Array([1]));}}));}});
 await turn();owner.abort();const result=await pending;assert(result.aborted);assert.equal(calls.length,1);assert.equal(result.completed.length,0);
});

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
