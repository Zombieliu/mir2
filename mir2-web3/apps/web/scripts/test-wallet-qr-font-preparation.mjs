import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const source=readFileSync(new URL('../lib/wallet-qr-font-preparation.ts',import.meta.url),'utf8');
function clockWindow(){
 const target=new EventTarget(),frames=new Map(),timers=new Map();let id=0,clock=0;
 Object.assign(target,{location:{origin:'https://numeron.obelisk.build',search:''},requestAnimationFrame:callback=>{frames.set(++id,callback);return id;},cancelAnimationFrame:key=>frames.delete(key)});
 return{target,frames,timers,performance:{now:()=>clock},
  setTimeout:(callback,ms)=>{timers.set(++id,{callback,at:clock+ms});return id;},clearTimeout:key=>timers.delete(key),
  paint:()=>{const pending=[...frames.values()];frames.clear();pending.forEach(callback=>callback(clock));},
  advance:ms=>{clock+=ms;for(const[key,row]of[...timers])if(row.at<=clock){timers.delete(key);row.callback();}}};
}
function compile(source,globals={}){
 const module={exports:{}};const js=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 new Function('module','exports',...Object.keys(globals),js)(module,module.exports,...Object.values(globals));return module.exports;
}
function harness(){
 const clock=clockWindow(),library=compile(source,{window:clock.target,performance:clock.performance,setTimeout:clock.setTimeout,clearTimeout:clock.clearTimeout});
 const image=new EventTarget();Object.assign(image,{complete:false,naturalWidth:0,src:'data:image/png;private-qr-data'});
 const states=[],reads=[];let eligible=true,worker=true;
 const start=()=>library.prepareFontAfterWalletQrImage({target:clock.target,eligible:()=>eligible,workerReady:()=>worker,
  readFont:options=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});reads.push({options,resolve,reject});return promise;},
  onState:(state,detail)=>states.push({state,detail})});
 const emit=(id,phase)=>clock.target.dispatchEvent(new CustomEvent(library.WALLET_QR_IMAGE_LIFECYCLE,{detail:{id,phase}}));
 return{...clock,...library,image,states,reads,start,emit,setEligible:value=>eligible=value,setWorker:value=>worker=value};
}
const turn=()=>new Promise(setImmediate);
const result={requested:1,completed:['/original-ui/fonts/NotoSansCJKsc-Regular.otf'],failed:[],aborted:false,bytes:16437364};

test('a decoded QR gets a paint opportunity before any preparation and sends no approval URL',async()=>{
 const f=harness(),lifecycle=[];f.target.addEventListener(f.WALLET_QR_IMAGE_LIFECYCLE,e=>lifecycle.push(e.detail));
 const stop=f.start(),retire=f.observeWalletQrImage(f.image);
 try{
  f.paint();await turn();assert.equal(f.reads.length,0);
  Object.assign(f.image,{complete:true,naturalWidth:240});f.image.dispatchEvent(new Event('load'));
  f.paint();await turn();assert.equal(f.reads.length,0,'first frame paints the QR before downloads');
  f.paint();await turn();assert.equal(f.reads.length,1);assert.deepEqual(Object.keys(lifecycle[0]).sort(),['id','phase']);
  assert(!JSON.stringify(lifecycle).includes('private'));assert.equal(lifecycle[0].phase,'ready');
  f.reads[0].resolve(result);await turn();assert.equal(f.states.at(-1).state,'Complete');assert.equal(f.states.at(-1).detail.bytes,16437364);
 }finally{retire();stop();assert.equal(f.frames.size,0);assert.equal(f.timers.size,0);}
});

test('a cached decoded image follows the same two-frame rule, once',async()=>{
 const f=harness();Object.assign(f.image,{complete:true,naturalWidth:240});const stop=f.start(),retire=f.observeWalletQrImage(f.image);
 try{f.image.dispatchEvent(new Event('load'));f.paint();assert.equal(f.frames.size,1);f.paint();f.image.dispatchEvent(new Event('load'));await turn();assert.equal(f.reads.length,1);}
 finally{retire();stop();}
});

test('cancelling before either frame leaves no downloads or late readiness',async()=>{
 for(const paints of[0,1]){
  const f=harness();Object.assign(f.image,{complete:true,naturalWidth:240});const stop=f.start(),retire=f.observeWalletQrImage(f.image);
  try{for(let n=0;n<paints;n++)f.paint();retire();f.paint();f.paint();await turn();assert.equal(f.reads.length,0);assert.equal(f.frames.size,0);}
  finally{retire();stop();}
 }
});

test('a broken or undecoded QR image cannot initiate prefetch',async()=>{
 const f=harness();f.image.complete=true;const stop=f.start(),retire=f.observeWalletQrImage(f.image);
 try{f.image.dispatchEvent(new Event('load'));f.paint();f.paint();await turn();assert.equal(f.reads.length,0);}
 finally{retire();stop();}
});

test('unconfirmed Worker waits briefly without a direct uncached font request, then starts once',async()=>{
 const f=harness();f.setWorker(false);const stop=f.start();
 try{f.emit(1,'ready');await turn();assert.equal(f.reads.length,0);assert.equal(f.timers.size,1);
  f.setWorker(true);f.advance(100);await turn();assert.equal(f.reads.length,1);assert.equal(f.timers.size,0);
  f.emit(1,'ready');await turn();assert.equal(f.reads.length,1);
 }finally{stop();}
});

test('Worker confirmation after the original image retires cannot start a font reader',async()=>{
 const f=harness();f.setWorker(false);const stop=f.start();
 try{f.emit(1,'ready');f.emit(1,'retired');f.setWorker(true);f.advance(8000);await turn();assert.equal(f.reads.length,0);assert.equal(f.timers.size,0);}
 finally{stop();}
});

test('missing Worker confirmation has an eight-second background limit',async()=>{
 const f=harness();f.setWorker(false);const stop=f.start();
 try{f.emit(1,'ready');f.advance(8000);await turn();assert.equal(f.reads.length,0);assert.equal(f.timers.size,0);assert.equal(f.states.at(-1).state,'Skipped');f.setWorker(true);f.advance(100);await turn();assert.equal(f.reads.length,0);}
 finally{stop();}
});

test('retirement aborts the QR foreground reader and suppresses a late result',async()=>{
 const f=harness(),stop=f.start();
 try{f.emit(1,'ready');await turn();assert.equal(f.reads.length,1);f.emit(1,'retired');assert(f.reads[0].options.signal.aborted);
  f.reads[0].resolve(result);await turn();assert(!f.states.some(s=>s.state==='Complete'));
 }finally{stop();}
});

test('an old image retirement or result cannot cancel or complete a newer reader',async()=>{
 const f=harness(),stop=f.start();
 try{f.emit(1,'ready');await turn();f.emit(2,'ready');await turn();assert.equal(f.reads.length,2);assert(f.reads[0].options.signal.aborted);
  f.emit(1,'retired');assert.equal(f.reads[1].options.signal.aborted,false);f.reads[0].resolve(result);await turn();assert(!f.states.some(s=>s.state==='Complete'));
  f.reads[1].resolve(result);await turn();assert.equal(f.states.filter(s=>s.state==='Complete').length,1);
 }finally{stop();}
});

test('invalid, duplicate and ineligible lifecycle events cannot create extra work',async()=>{
 const f=harness(),stop=f.start();
 try{for(const[id,phase]of[[0,'ready'],[-1,'ready'],[1,'unknown'],['1','ready']])f.emit(id,phase);
  f.setEligible(false);f.emit(1,'ready');await turn();assert.equal(f.reads.length,0);f.setEligible(true);f.emit(2,'ready');f.emit(2,'ready');await turn();assert.equal(f.reads.length,1);
 }finally{stop();}
});

test('unmount removes readiness listeners and pending confirmation timers',async()=>{
 const f=harness();f.setWorker(false);const stop=f.start();f.emit(1,'ready');stop();f.setWorker(true);f.advance(8000);f.emit(2,'ready');await turn();assert.equal(f.reads.length,0);assert.equal(f.timers.size,0);
});

const page=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('page.tsx',page,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
let effect;const visit=node=>{if(ts.isCallExpression(node)&&node.expression.getText(ast)==='useEffect'&&node.arguments[0]?.name?.text==='prepareFontDuringWalletConfirmation')effect=node.arguments[0];ts.forEachChild(node,visit);};visit(ast);assert(effect);
function actualPage(options={}){
 const f=harness(),qr={status:'pending',expiresAt:Date.now()+60000,...options.qr};
 f.target.__mir2AssetCache={enabled:true,status:'registered',...options.cache};
 f.target.location.search=options.search??'';
 const env={prepareFontAfterWalletQrImage:f.prepareFontAfterWalletQrImage,window:f.target,
  navigator:{maxTouchPoints:options.touch?1:0,userAgent:'Chrome desktop',serviceWorker:{controller:options.noController?null:{scriptURL:options.workerUrl??'https://numeron.obelisk.build/mir2-asset-worker.js'}}},
  clientProfile:{layout:options.touch?'touch':'desktop',input:options.touch?'touch':'keyboardMouse'},
  bevyRuntimeStartup:{runtimeAllowed:options.allowed!==false},dubheQrStateRef:{current:qr},URLSearchParams,URL,
  resolveBevyRuntimeBootDecision:compile(readFileSync(new URL('../lib/bevy-runtime-policy.ts',import.meta.url),'utf8')).resolveBevyRuntimeBootDecision,
  prewarmBevyRuntimePrimaryFont:options=>{f.reads.push({options});return Promise.resolve(result);},markMir2CacheMilestone:(name,detail)=>f.states.push({name,detail})};
 const js=ts.transpileModule(effect.getText(ast),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const cleanup=new Function(...Object.keys(env),js+'\nreturn prepareFontDuringWalletConfirmation();')(...Object.values(env));
 return{...f,env,cleanup};
}

test('actual Page primes only the original primary font after visible QR and confirmed own Worker',async()=>{
 const f=actualPage();try{await turn();assert.equal(f.reads.length,0);f.emit(1,'ready');await turn();assert.equal(f.reads.length,1);assert(f.states.some(s=>s.name==='walletQrFontPreparationStart'));assert(f.states.some(s=>s.name==='walletQrFontPreparationComplete'));}finally{f.cleanup();}
});

test('actual Page skips expired/nonpending QR, disabled/missing runtime, touch and unconfirmed/foreign Worker',async()=>{
 for(const options of[{qr:{status:'creating'}},{qr:{status:'error'}},{qr:{expiresAt:Date.now()-1}},{allowed:false},{touch:true},
  {search:'?bevyRuntime=0'},{search:'?skipRuntime=1'},{cache:{enabled:false}},{cache:{status:'registered-unconfirmed'}},{noController:true},
  {workerUrl:'https://elsewhere.example.test/mir2-asset-worker.js'},{workerUrl:'https://numeron.obelisk.build/other-worker.js'},{workerUrl:'invalid'}]){
  const f=actualPage(options);try{f.emit(1,'ready');f.advance(8000);await turn();assert.equal(f.reads.length,0,JSON.stringify(options));}finally{f.cleanup();}
 }
});

test('actual Page rechecks the current QR before a late Worker confirmation',async()=>{
 const f=actualPage({cache:{status:'registering'}});try{f.emit(1,'ready');f.env.dubheQrStateRef.current=null;f.target.__mir2AssetCache.status='registered';f.advance(100);await turn();assert.equal(f.reads.length,0);}finally{f.cleanup();}
});
