import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import test from 'node:test';
import ts from 'typescript';

const compile=source=>ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
function load(file){
 const url=new URL(file,import.meta.url),module={exports:{}};
 if(url.pathname.endsWith('.json'))return{default:JSON.parse(readFileSync(url,'utf8'))};
 const require=name=>{assert(name.startsWith('.'));return load(new URL(name+(name.endsWith('.json')?'':'.ts'),url));};
 new Function('require','module','exports',compile(readFileSync(url,'utf8')))(require,module,module.exports);return module.exports;
}
const {MapWorldBaselineRefresh}=load('../lib/map-world-baseline-refresh.ts');
const {observeQuestMapPacket,currentQuestMapIndex}=load('../lib/quest-world-lifecycle.ts');
const {sameQuestWorldIdentity}=load('../lib/bevy-quest-world-context.ts');
function clock(){let now=0,next=0;const rows=new Map();return{rows,setTimer(fn,ms){const id=++next;rows.set(id,{fn,at:now+ms});return id;},clearTimer(id){rows.delete(id);},advance(ms){const until=now+ms;for(;;){const next=[...rows].filter(([,r])=>r.at<=until).sort((a,b)=>a[1].at-b[1].at)[0];if(!next)break;now=next[1].at;rows.delete(next[0]);next[1].fn();}now=until;}};}
const source=()=>({socket:{},connectionGeneration:1,sessionGeneration:1,sceneRevision:2,playerObjectId:1000,mapFileName:'0108'});
function harness(){const time=clock(),owner=source(),state={current:owner,complete:false},requests=[];
 const refresh=new MapWorldBaselineRefresh({current:()=>state.current,complete:()=>state.complete,request:s=>{requests.push(s);return true;},setTimer:time.setTimer,clearTimer:time.clearTimer});
 return{time,owner,state,requests,refresh};}

test('map read waits for projection, then stops when a complete new-scene baseline arrives',()=>{
 const h=harness();h.refresh.schedule(h.owner);assert.equal(h.requests.length,0);h.time.advance(99);assert.equal(h.requests.length,0);h.time.advance(1);assert.equal(h.requests.length,1);
 h.state.complete=true;h.time.advance(750);assert.equal(h.requests.length,1);assert.equal(h.time.rows.size,0);assert(!h.refresh.hasPending());
});
test('an already complete new-scene baseline requires no duplicate request',()=>{const h=harness();h.state.complete=true;h.refresh.schedule(h.owner);h.time.advance(100);assert.deepEqual(h.requests,[]);assert(!h.refresh.hasPending());});
for(const changed of ['socket','connection','session','scene','map','player','logout'])test('pending map read is retired after '+changed,()=>{
 const h=harness();h.refresh.schedule(h.owner);h.state.current=changed==='logout'?null:{...h.owner,...({socket:{socket:{}},connection:{connectionGeneration:2},session:{sessionGeneration:2},scene:{sceneRevision:3},map:{mapFileName:'0'},player:{playerObjectId:2000}}[changed])};h.time.advance(10000);assert.deepEqual(h.requests,[]);assert(!h.refresh.hasPending());assert.equal(h.time.rows.size,0);
});
test('missing or incomplete snapshots have at most three read-only attempts',()=>{const h=harness();h.refresh.schedule(h.owner);h.time.advance(10000);assert.equal(h.requests.length,3);assert(h.requests.every(s=>s.mapFileName==='0108'&&s.socket===h.owner.socket));assert(!h.refresh.hasPending());assert.equal(h.time.rows.size,0);});
test('transport throws are bounded and disposing a pending read cancels its timer',()=>{
 const time=clock(),owner=source();let count=0;const refresh=new MapWorldBaselineRefresh({current:()=>owner,complete:()=>false,request:()=>{count++;throw Error('unknown read outcome');},setTimer:time.setTimer,clearTimer:time.clearTimer});refresh.schedule(owner);time.advance(10000);assert.equal(count,3);refresh.schedule(owner);refresh.cancel();time.advance(10000);assert.equal(count,3);assert.equal(time.rows.size,0);
});
test('a newer map replaces every pending read of the old scene',()=>{const h=harness();h.refresh.schedule(h.owner);const next={...h.owner,mapFileName:'0',sceneRevision:3};h.state.current=next;h.refresh.schedule(next);h.time.advance(100);assert.deepEqual(h.requests,[next]);h.state.complete=true;h.time.advance(10000);assert.equal(h.requests.length,1);});

const pageSource=readFileSync(process.env.R16_MAP_BASELINE_PAGE_SOURCE??new URL('../app/page.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('page.tsx',pageSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const names=['currentMapWorldBaselineSource','refreshMapWorldBaseline','captureQuestMapGatewayEvent'],declarations=[];
function visit(node){if(ts.isFunctionDeclaration(node)&&names.includes(node.name?.text))declarations.push(node.getText(ast));ts.forEachChild(node,visit);}visit(ast);assert.equal(declarations.length,3,'exercise the actual Page map ingress');
function pageHarness(){
 const ref=current=>({current}),time=clock(),socket={readyState:1};const state={queued:{targetX:10,targetY:13},retired:0,sent:[]};
 const refs={equipmentConnectionGenerationRef:ref(1),equipmentSessionGenerationRef:ref(1),questSceneRevisionRef:ref(1),questMapAuthorityRef:ref({connectionGeneration:1,sessionGeneration:1,sceneRevision:1,mapFileName:'0',mapIndex:1}),worldRef:ref({connected:true,mapFileName:'0',playerObjectId:'1000'}),socketRef:ref(socket),screenRef:ref('game'),questCapturedWorldRef:ref(null),questSkillsBaselineCurrentRef:ref(true),mapWorldBaselineRefreshRef:ref(null)};
 const identity=()=>({connectionGeneration:refs.equipmentConnectionGenerationRef.current,sessionGeneration:refs.equipmentSessionGenerationRef.current,sceneRevision:refs.questSceneRevisionRef.current,playerObjectId:Number(refs.worldRef.current.playerObjectId),mapFileName:refs.worldRef.current.mapFileName});refs.questCapturedWorldRef.current=identity();
 class ClockedRefresh extends MapWorldBaselineRefresh{constructor(options){super({...options,setTimer:time.setTimer,clearTimer:time.clearTimer});}}
 const deps={...refs,WebSocket:{OPEN:1},MapWorldBaselineRefresh:ClockedRefresh,currentQuestWorldIdentity:identity,currentQuestMapIndex,observeQuestMapPacket,sameQuestWorldIdentity,sameMapWorldBaselineSource:(a,b)=>b!==null&&a.socket===b.socket&&sameQuestWorldIdentity(a,b),normalizeQuestMapFileName:s=>s?.toLowerCase()??null,
  retireQuestWorldActions:()=>{state.queued=null;state.retired++;},send:(command,options)=>{assert.deepEqual(command,{type:'clientVersion'});assert.deepEqual(options,{quiet:true});state.sent.push({...command});return true;}};
 const api=new Function(...Object.keys(deps),compile(declarations.join('\n'))+';return{'+names.join(',')+'};')(...Object.values(deps));
 const map=(file='0108',index=4,packet='MapInformation',generation=1)=>{api.captureQuestMapGatewayEvent({type:'packet',packet,payload:{fileName:file,mapIndex:index}},generation);refs.worldRef.current.mapFileName=file;};
 return{refs,state,time,identity,api,map};
}
test('actual Page: ordinary map transfer retires navigation and requests its missing personal baseline',()=>{
 const h=pageHarness();h.map();assert.equal(h.state.queued,null);assert.equal(h.state.retired,1);assert.equal(h.refs.questCapturedWorldRef.current,null);h.time.advance(100);assert.deepEqual(h.state.sent,[{type:'clientVersion'}]);
 h.refs.questCapturedWorldRef.current=h.identity();h.refs.questSkillsBaselineCurrentRef.current=true;h.time.advance(10000);assert.equal(h.state.sent.length,1);assert.equal(h.state.queued,null,'reading state never restores the old objective');
});
test('actual Page: initial StartGame map does not request another bootstrap',()=>{const h=pageHarness();h.refs.questCapturedWorldRef.current=null;h.map();h.time.advance(10000);assert.deepEqual(h.state.sent,[]);});
test('actual Page: unchanged MapInformation preserves the current goal without a refresh',()=>{const h=pageHarness();h.map('0',1);h.time.advance(10000);assert.equal(h.state.retired,0);assert(h.state.queued);assert.deepEqual(h.state.sent,[]);});
test('actual Page: same-map MapChanged still retires the scene and reads a fresh baseline',()=>{const h=pageHarness();h.map('0',1,'MapChanged');h.time.advance(100);assert.equal(h.state.queued,null);assert.equal(h.state.sent.length,1);});
test('actual Page: a second map replaces pending refresh before the first read',()=>{const h=pageHarness();h.map();h.map('0',1);h.time.advance(100);assert.equal(h.state.sent.length,1);assert.equal(h.api.currentMapWorldBaselineSource().mapFileName,'0');});
test('actual Page: an old connection map packet cannot request a new baseline',()=>{const h=pageHarness();h.api.captureQuestMapGatewayEvent({type:'packet',packet:'MapInformation',payload:{fileName:'0108',mapIndex:4}},2);h.time.advance(10000);assert.equal(h.state.retired,0);assert.deepEqual(h.state.sent,[]);});
test('actual Page: logout before dispatch sends no read on the selection screen',()=>{const h=pageHarness();h.map();h.refs.screenRef.current='select';h.time.advance(10000);assert.deepEqual(h.state.sent,[]);assert.equal(h.time.rows.size,0);});
for(const packet of ['StartGame','LogOutSuccess','ReturnToLogin'])test('actual Page: '+packet+' disposes the previous map read immediately',()=>{const h=pageHarness();h.map();assert.equal(h.time.rows.size,1);h.api.captureQuestMapGatewayEvent({type:'packet',packet,payload:{}},1);assert.equal(h.time.rows.size,0);h.time.advance(10000);assert.deepEqual(h.state.sent,[]);});
test('actual Page: another character starts without inheriting a pending map refresh',()=>{const h=pageHarness();h.map();h.api.captureQuestMapGatewayEvent({type:'packet',packet:'StartGame',payload:{result:4}},1);h.refs.equipmentSessionGenerationRef.current=2;h.refs.worldRef.current.mapFileName='0108';h.refs.questCapturedWorldRef.current=null;h.map('0',1);h.time.advance(10000);assert.deepEqual(h.state.sent,[]);});
