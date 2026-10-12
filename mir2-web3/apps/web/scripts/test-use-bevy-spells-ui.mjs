import assert from 'node:assert/strict';import test from 'node:test';import {readFileSync} from 'node:fs';import ts from 'typescript';
function fixture(){let effect;const hosts=[],states=[];class Host{constructor(o){this.options=o;this.events=[];hosts.push(this);}observeSnapshot(...v){this.events.push(v);}tick(){if(!this.options.isCurrent())this.stop();}withdraw(){this.withdrawn=true;this.options.onDiagnostic?.({ready:false,reason:'withdrawn'});}stop(){this.stopped=true;this.options.onDiagnostic?.({ready:false,reason:'stopped'});}pointerContext(){return null;}pointer(){return false;}allows(){return false;}}
 const m={exports:{}};const deps={react:{useRef:v=>({current:v}),useState:v=>{const state={value:v,changes:0};states.push(state);return[v,next=>{const value=typeof next==='function'?next(state.value):next;if(value!==state.value){state.value=value;state.changes++;}}];},useEffect:f=>{effect=f;}},'./bevy-spells-ui':{SpellsHost:Host,parseSpellsIconMetadata:v=>v}};
 new Function('exports','module','require',ts.transpileModule(readFileSync(new URL('../lib/use-bevy-spells-ui.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m,p=>deps[p]);return{run:m.exports.useBevySpellsUi,get effect(){return effect;},hosts,states};}
test('hook captures runtime, complete pre-runtime baseline and aborts metadata on cleanup',async()=>{const saved={window:globalThis.window,document:globalThis.document,fetch:globalThis.fetch};let tick,fetchResolve,signal;const listeners=new Map();
 globalThis.window={setInterval:f=>{tick=f;return 1;},clearInterval:()=>{},addEventListener:(k,f)=>listeners.set(k,f),removeEventListener:k=>listeners.delete(k)};globalThis.document={addEventListener:(k,f)=>listeners.set(k,f),removeEventListener:k=>listeners.delete(k)};globalThis.fetch=(_,o)=>{signal=o.signal;return new Promise(r=>{fetchResolve=r;});};
 try{const h=fixture(),ref={current:{}},baseline={snapshot:{playerObjectId:3,knownSkills:[{spell:'FireBall',need1:null}],skillKeyAck:{accepted:false}},owner:{connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3}};
 h.run({requested:true,runtimeGeneration:1,runtimeRef:ref,read:()=>({}),readBaseline:()=>baseline,onIntent:()=> 'confirmedSend'});const cleanup=h.effect();assert.deepEqual(h.hosts[0].events[0],[baseline.snapshot,baseline.owner]);assert.equal(h.hosts[0].options.isCurrent(),true);
 listeners.get('blur')();assert.equal(h.hosts[0].withdrawn,true);ref.current={};tick();assert.equal(h.hosts[0].stopped,true);cleanup();assert.equal(signal.aborted,true);assert.equal(listeners.size,0);fetchResolve({ok:true,json:async()=>({version:1})});await Promise.resolve();await Promise.resolve();assert.equal(h.hosts[0].options.isCurrent(),false);
 }finally{Object.assign(globalThis,saved);}});
test('unrequested hook does not create controller or install a sink',()=>{const h=fixture();h.run({requested:false,runtimeGeneration:1,runtimeRef:{current:{}},read:()=>({}),onIntent:()=> 'confirmedSend'});assert.equal(h.effect(),undefined);assert.equal(h.hosts.length,0);});
test('hook diagnostic dedupes finite values on existing lifecycle and ignores retired callbacks',async()=>{
 const saved={window:globalThis.window,document:globalThis.document,fetch:globalThis.fetch};let tick,fetchResolve,fetches=0,intervals=0,reads=0;const listeners=new Map();
 globalThis.window={setInterval:f=>{tick=f;intervals++;return 1;},clearInterval:()=>{},addEventListener:(k,f)=>listeners.set(k,f),removeEventListener:k=>listeners.delete(k)};globalThis.document={addEventListener:(k,f)=>listeners.set(k,f),removeEventListener:k=>listeners.delete(k)};
 globalThis.fetch=()=>{fetches++;return new Promise(resolve=>{fetchResolve=resolve;});};
 try{const h=fixture(),ref={current:{}},result=h.run({requested:true,runtimeGeneration:1,runtimeRef:ref,read:()=>{reads++;return {};},onIntent:()=> 'confirmedSend'}),cleanup=h.effect(),host=h.hosts[0];
  assert.deepEqual(result.diagnostic,{ready:false,reason:'stopped'});const report=host.options.onDiagnostic;report({ready:false,reason:'baseline-missing'});const retained=h.states[1].value,changes=h.states[1].changes;
  report({ready:false,reason:'baseline-missing'});assert.equal(h.states[1].value,retained);assert.equal(h.states[1].changes,changes);
  report({ready:false,reason:'ineligible',inputReason:'hud-unready'});report({ready:false,reason:'ineligible',inputReason:'unfocused'});assert.deepEqual(h.states[1].value,{ready:false,reason:'ineligible',inputReason:'unfocused'});
  report({ready:true,reason:'ready'});report({ready:true,reason:'ready'});assert.deepEqual(h.states[1].value,{ready:true,reason:'ready'});host.options.onState(true);assert.equal(h.states[0].value,true);
  assert.equal(reads,0);assert.equal(fetches,1);assert.equal(intervals,1);listeners.get('blur')();assert.deepEqual(h.states[1].value,{ready:false,reason:'withdrawn'});assert.equal(h.states[0].value,false);
  cleanup();const after=h.states.map(s=>({value:s.value,changes:s.changes}));report({ready:false,reason:'retired'});host.options.onState(true);fetchResolve({ok:true,json:async()=>({version:1,frames:[]})});await Promise.resolve();await Promise.resolve();await Promise.resolve();
  assert.deepEqual(h.states.map(s=>({value:s.value,changes:s.changes})),after);assert.equal(reads,0);assert.equal(fetches,1);assert.equal(intervals,1);assert.equal(listeners.size,0);assert.equal(typeof tick,'function');
 }finally{Object.assign(globalThis,saved);}
});
test('unrequested and absent runtime diagnostics require no host or runtime read',()=>{
 for(const [requested,runtime,expected]of [[false,{}, {ready:false,reason:'ineligible',inputReason:'request-disabled'}],[true,null,{ready:false,reason:'unsupported-abi'}]]){
  const h=fixture();h.run({requested,runtimeGeneration:1,runtimeRef:{current:runtime},read:()=>{throw Error('must not read');},onIntent:()=> 'confirmedSend'});assert.equal(h.effect(),undefined);assert.equal(h.hosts.length,0);assert.deepEqual(h.states[1].value,expected);
 }
});
test('hook diagnostic uses the existing callback lifecycle without new polling or runtime access',()=>{
 const source=readFileSync(new URL('../lib/use-bevy-spells-ui.ts',import.meta.url),'utf8');assert.match(source,/onDiagnostic:v=>\{if\(live\)updateDiagnostic\(v\);\}/);assert.match(source,/previous\.ready===next\.ready&&previous\.reason===next\.reason&&previous\.inputReason===next\.inputReason\?previous:next/);
 assert.match(source,/return\{ready,diagnostic,withdraw:/);assert.match(source,/return\(\)=>\{live=false;abort\.abort\(\);/);assert.equal((source.match(/window\.setInterval\(/g)??[]).length,1);assert.equal((source.match(/\bfetch\(/g)??[]).length,1);
 assert.doesNotMatch(source,/getMir2SpellsUi(?:Status|Capabilities)/);assert.doesNotMatch(source,/diagnostic\.(?:ready|reason|inputReason)/);
});
