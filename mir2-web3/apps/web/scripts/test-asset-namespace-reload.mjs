import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

function compile(source){return ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;}
const helper={};new Function('exports',compile(readFileSync(new URL('../lib/asset-namespace-reload.ts',import.meta.url),'utf8')))(helper);
const {canReloadAssetNamespace}=helper;
const registrar=readFileSync(process.env.R16_ASSET_NAMESPACE_SOURCE??new URL('../app/components/asset-cache-registrar.tsx',import.meta.url),'utf8');
const ast=ts.createSourceFile('registrar.tsx',registrar,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const fn=ast.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==='maybeReloadAfterCacheNamespaceChange');assert(fn);
function fixture(){
 const state={screen:'login',visible:true,connected:true,disabled:false,value:'',modal:false},events=[],local=new Map([['namespace','old']]),session=new Map();
 const control=()=>({disabled:state.disabled}),input=()=>({type:'text',value:state.value});
 const form={get isConnected(){return state.connected;},querySelectorAll:()=>[control()]};
 const root={querySelector:selector=>selector==="form[data-login-auth-surface='login']"?form:null,querySelectorAll:()=>[input(),{type:'range',value:'80'}]};
 const document={get visibilityState(){return state.visible?'visible':'hidden';},querySelector:selector=>selector==="main[data-client-screen='login']"&&state.screen==='login'?root:null,querySelectorAll:()=>state.modal?[{getClientRects:()=>[{}]}]:[]};
 const storage=map=>({getItem:key=>map.get(key)??null,setItem:(key,value)=>map.set(key,value)});
 const window={localStorage:storage(local),sessionStorage:storage(session),setTimeout,location:{reload:()=>events.push('reload')}};
 const run=new Function('window','document','CACHE_NAMESPACE_STORAGE_KEY','readStorageValue','writeStorageValue','canReloadAssetNamespace',compile(fn.getText(ast))+'\nreturn maybeReloadAfterCacheNamespaceChange;')(
  window,document,'namespace',(store,key)=>store.getItem(key),(store,key,value)=>store.setItem(key,value),canReloadAssetNamespace);
 return{state,document,events,local,session,run};
}

test('only a visible, connected, empty ordinary login form is eligible',()=>{
 const f=fixture();assert(canReloadAssetNamespace(f.document));
 for(const [field,value]of [['screen','game'],['screen','select'],['visible',false],['connected',false],['disabled',true],['value','typing'],['value',' '],['modal',true]]){
  const fresh=fixture();fresh.state[field]=value;assert.equal(canReloadAssetNamespace(fresh.document),false,field);
 }
});
test('an empty idle login reloads once at the original delay and marks only its actual reload',t=>{
 t.mock.timers.enable({apis:['setTimeout']});const f=fixture();f.run('new',['old-cache']);
 assert.equal(f.local.get('namespace'),'new');assert.equal(f.session.get('namespace.reloaded.new'),undefined);
 t.mock.timers.tick(249);assert.equal(f.events.length,0);t.mock.timers.tick(1);assert.deepEqual(f.events,['reload']);
 assert.equal(f.session.get('namespace.reloaded.new'),'1');f.run('new',['older']);t.mock.timers.tick(250);assert.equal(f.events.length,1);
});
for(const [field,value]of [['screen','game'],['screen','select'],['visible',false],['disabled',true],['value','typed account'],['modal',true]])test(`a namespace change preserves active ${field}=${value}`,t=>{
 t.mock.timers.enable({apis:['setTimeout']});const f=fixture();f.state[field]=value;f.run('new',['old-cache']);t.mock.timers.tick(1000);
 assert.equal(f.events.length,0);assert.equal(f.session.get('namespace.reloaded.new'),undefined);assert.equal(f.state[field],value);
});
for(const [field,value]of [['value','typed before timer'],['disabled',true],['screen','game'],['visible',false],['modal',true]])test(`a pending reload rechecks ${field} at execution`,t=>{
 t.mock.timers.enable({apis:['setTimeout']});const f=fixture();f.run('new',['old-cache']);f.state[field]=value;t.mock.timers.tick(250);
 assert.equal(f.events.length,0);assert.equal(f.session.get('namespace.reloaded.new'),undefined);
});
test('an older namespace callback cannot reload or mark a later configuration',t=>{
 t.mock.timers.enable({apis:['setTimeout']});const f=fixture();f.run('a',['old-cache']);f.run('b',['old-cache']);t.mock.timers.tick(250);
 assert.deepEqual(f.events,['reload']);assert.equal(f.session.get('namespace.reloaded.a'),undefined);assert.equal(f.session.get('namespace.reloaded.b'),'1');
});
test('no removed cache or an unchanged namespace needs no reload',t=>{
 t.mock.timers.enable({apis:['setTimeout']});const f=fixture();f.run('new',[]);f.run('old',['removed']);t.mock.timers.tick(1000);assert.equal(f.events.length,0);
});
