import assert from 'node:assert/strict';import test from 'node:test';import {readFileSync,writeFileSync} from 'node:fs';import ts from 'typescript';
const nativeKeyboardDefaults=JSON.parse(readFileSync(new URL('../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json',import.meta.url),'utf8'));
const cache=new Map();function load(name){if(cache.has(name))return cache.get(name);const m={exports:{}};cache.set(name,m.exports);new Function('exports','module','require',ts.transpileModule(readFileSync(new URL(`../lib/${name}.ts`,import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m,n=>n==='../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json'?{default:nativeKeyboardDefaults}:load(n.replace('./','')));return m.exports;}
const {CombatHost,supportsCombat,supportsForcedPlayerAttack,parseCombatProof}=load('bevy-combat-input');
const {defaultCrystalKeyBindings}=load('player-ui-preferences');
test('actual Shell shortcuts precede Winit canvas cancellation and preserve capture, modal, focus and editable gates',()=>{
 const source=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8');
 const ast=ts.createSourceFile('shell.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let effect,editable;
 function visit(n){
  if(ts.isCallExpression(n)&&n.expression.getText(ast)==='useEffect'&&n.arguments[0]?.getText(ast).includes('function handleShortcutKey'))effect=n.arguments[0];
  if(ts.isFunctionDeclaration(n)&&n.name?.text==='keyboardInputTargetIsEditable')editable=n;
  ts.forEachChild(n,visit);
 }visit(ast);assert.ok(effect);assert.ok(editable);
 const listeners=[],calls=[];
 class HTMLElement{isContentEditable=false;closest(){return null;}}class HTMLInputElement extends HTMLElement{}class HTMLTextAreaElement extends HTMLElement{}class HTMLSelectElement extends HTMLElement{}
 const flags={capture:false,focus:true,modal:false};
 const scope={HTMLElement,HTMLInputElement,HTMLTextAreaElement,HTMLSelectElement,screen:'game',selectedEntity:null,
  world:{beltItems:[],knownSkills:[]},window:{addEventListener:(type,fn,capture)=>listeners.push({type,fn,capture:capture===true}),
   removeEventListener:(type,fn,capture)=>{const i=listeners.findIndex(x=>x.type===type&&x.fn===fn&&x.capture===(capture===true));assert.ok(i>=0);listeners.splice(i,1);}},
  document:{visibilityState:'visible',hasFocus:()=>flags.focus},getKeybindCaptureActive:()=>flags.capture,
  npcShopBlocksWorldInput:()=>false,stopNpcShopWorldInput:()=>{},bevyStorageUiActive:false,bevyStorageUiTransitioning:false,
  bevyMailComposeReady:false,bevyMailComposePending:false,spellsPointerCallbacksRef:{current:{getBevySpellsPointerContext:()=>({modal:flags.modal})}},
  bevyQuestUiCapturesPointer:false,heroPointerCallbacksRef:{current:{onBevyHeroKey:()=>false}},heroBlocksWorldInput:()=>false,
  crystalKeyBindings:defaultCrystalKeyBindings(),combatTildeRef:{current:false},
  ...load('crystal-shortcut-actions'),...load('player-ui-preferences'),...load('shared-combat-mode-keys'),
  onCrystalWindowShortcut:action=>calls.push(action),onCrystalGameplayShortcut:()=>false,onCombatKey:()=>({supported:true,handled:false}),
  isKeyboardMoveKey:()=>false,onPrimaryTargetAction:()=>{},revealDropView:()=>{}};
 const js=ts.transpileModule(editable.getText(ast)+'\nreturn ('+effect.getText(ast)+')();',{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const cleanup=new Function('scope','with(scope){'+js+'}')(scope);
 function key(target=new HTMLElement(),options={}){const e={target,key:'i',code:'KeyI',altKey:false,ctrlKey:false,shiftKey:false,metaKey:false,repeat:false,
   defaultPrevented:false,preventDefault(){this.defaultPrevented=true;},...options};
  for(const l of listeners.filter(l=>l.type==='keydown'&&l.capture))l.fn(e);
  e.preventDefault(); // Original Winit canvas target listener, after window capture.
  for(const l of listeners.filter(l=>l.type==='keydown'&&!l.capture))l.fn(e);return e;}
 key();assert.equal(calls.length,1,'canvas focus must retain the original Inventory2 shortcut');calls.length=0;
 key(new HTMLElement(),{key:'q',code:'KeyQ'});assert.equal(calls.length,1);calls.length=0;
 for(const gate of ['capture','focus','modal']){flags[gate]=gate!=='focus';key();assert.equal(calls.length,0,gate+' gate remains');flags[gate]=gate==='focus';}
 key(new HTMLInputElement());key(new HTMLTextAreaElement());key(new HTMLSelectElement());assert.equal(calls.length,0);
 key(new HTMLElement(),{defaultPrevented:true});key(new HTMLElement(),{repeat:true});assert.equal(calls.length,0);
 scope.document.visibilityState='hidden';key();assert.equal(calls.length,0);
 cleanup();assert.equal(listeners.length,0,'capture listener is removed with the same phase');
});
const caps={schemaVersion:1,combatInputAbiVersion:1,combatIntentAbiVersion:1,compiled:true,startup:true};
function mountActualCombatHook(options){
 const source=readFileSync(new URL('../lib/use-bevy-combat-input.ts',import.meta.url),'utf8');
 const ast=ts.createSourceFile('hook.ts',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
 const fn=ast.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==='useBevyCombatInput');assert.ok(fn);
 const js=ts.transpileModule(fn.getText(ast),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 let interval=null,cleanup=null;const effects=[],exports={};
 const window={setInterval:fn=>{interval=fn;return 1;},clearInterval:()=>{interval=null;},addEventListener(){},removeEventListener(){}};
 const hook=new Function('exports','useRef','useState','useEffect','CombatHost','supportsCombat','window','document','performance',js+';return exports.useBevyCombatInput;')(
  exports,value=>({current:value}),()=>[false,()=>{}],fn=>effects.push(fn),CombatHost,supportsCombat,window,{addEventListener(){},removeEventListener(){}},{now:()=>10});
 const api=hook(options);for(const effect of effects)cleanup=effect();
 return{api,tick:()=>{assert.ok(interval);interval();},stop:()=>cleanup(),get interval(){return interval;}};
}
test('actual hook ingests a late current baseline once, retains proof gates and never replays a select',()=>{
 const f=fixture();let baseline=null;
 const h=mountActualCombatHook({runtimeGeneration:1,runtimeRef:{current:f.runtime},read:()=>f.input,readBaseline:()=>baseline,onWire:()=>{throw Error('unexpected wire');},onApproach:()=>{},onClear:()=>{}});
 assert.equal(h.api.diagnostics().rawPresent,false);const initial=f.captures.length;
 baseline={snapshot:f.raw,owner:{...f.input.owner,sessionGeneration:9}};h.tick();assert.equal(h.api.diagnostics().rawPresent,false);
 baseline={snapshot:f.raw,owner:{...f.input.owner}};h.tick();assert.equal(h.api.diagnostics().rawPresent,true);assert.equal(h.api.diagnostics().snapshotPresent,true);assert.equal(f.captures.length,initial+1);
 h.tick();assert.equal(f.captures.length,initial+1);assert.equal(f.edges.length,0);assert.equal(f.calls.length,0);
 const run=f.sent.identity.controllerRun;f.input.owner={...f.input.owner,sessionGeneration:10};h.tick();assert.equal(h.api.diagnostics().snapshotPresent,false);
 baseline={snapshot:f.raw,owner:{...f.input.owner}};h.tick();assert.equal(h.api.diagnostics().snapshotPresent,true);assert.ok(f.sent.identity.controllerRun>run);assert.equal(f.edges.length,0);
 h.stop();assert.equal(h.interval,null);
});
function combatSourceFixture(){
 const source=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 const names=['readCurrentCombatBaseline','applyNpcGoldBuyGatewaySnapshot','captureSpellsGatewayEvent'],decl=[];let provider;
 function visit(n){if(ts.isFunctionDeclaration(n)&&n.name&&names.includes(n.name.text))decl.push(n.getText(ast));
  if(ts.isCallExpression(n)&&n.expression.getText(ast)==='useBevyCombatInput')provider=n.arguments[0].properties.find(p=>p.name?.getText(ast)==='readBaseline').initializer.getText(ast);
  ts.forEachChild(n,visit);}
 visit(ast);assert.equal(decl.length,names.length);assert.equal(provider,'readCurrentCombatBaseline');
 const f=fixture(),session={connectionGeneration:1,sessionGeneration:2,afterSnapshotVersion:0},socket={readyState:1};
 const scope={combatBaselineSourceRef:{current:null},combatRawRef:{current:null},worldRef:{current:{connected:true,playerObjectId:'3',mapFileName:'D001'}},
  equipmentStartGameRef:{current:session},socketRef:{current:socket},equipmentSnapshotRef:{current:null},
  equipmentConnectionGenerationRef:{current:1},equipmentSessionGenerationRef:{current:2},questSceneRevisionRef:{current:1},worldSnapshotVersionRef:{current:1},
  questCapturedWorldRef:{current:null},questSkillsBaselineCurrentRef:{current:false},combatIngressRef:{current:f.host},
  spellsRawSnapshotRef:{current:null},spellsIngressRef:{current:null},combatMastersRef:{current:new Map()},
  currentSpellsOwner:()=>f.input.owner,WebSocket:{OPEN:1},normalizeQuestMapFileName:x=>x,
  npcGoldBuyInventoryRef:{current:{begin:()=>({}),finish:()=>{}}},projectBevyBagModel:()=>({ok:true,model:{}}),captureParitySnapshot:()=>{},
  observeBagBeltInventorySnapshot:()=>{},observeNpcPearlWallet:()=>{},readNpcRepairOwner:()=>null,
  npcRepairAuthorityRef:{current:{observeSnapshot:()=>{},invalidateInventory:()=>{}}},captureNpcRepairDialog:()=>{},npcBuyDispatcherRef:{current:null},
  applyGatewayWorldSnapshot:snapshot=>{scope.equipmentSnapshotRef.current={connectionGeneration:1,sessionGeneration:2};
   scope.worldRef.current={...scope.worldRef.current,playerObjectId:String(snapshot.playerObjectId),mapFileName:snapshot.mapFileName};scope.duringProjection?.();}};
 const js=ts.transpileModule(decl.join('\n')+'\nreturn {'+names.join(',')+'};',{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const api=new Function('scope','with(scope){'+js+'}')(scope);
 const snapshot={playerObjectId:3,mapFileName:'D001',knownSkills:f.raw.knownSkills,entities:[]};
 return{f,scope,api,session,socket,snapshot,commit:()=>api.applyNpcGoldBuyGatewaySnapshot(snapshot,1),read:()=>api.readCurrentCombatBaseline()};
}
test('actual Page provider authenticates physical source across a late UI owner change without renaming old cache or replaying edges',()=>{
 const x=combatSourceFixture();assert.equal(x.read(),null);x.commit();assert.ok(x.read());
 assert.notEqual(x.read().snapshot.knownSkills,x.snapshot.knownSkills,'original learned data is detached at actual projection commit');
 const cachedOwner=x.scope.combatRawRef.current.owner,source=x.scope.combatBaselineSourceRef.current,run=x.f.sent.identity.controllerRun;
 x.f.input.owner={...x.f.input.owner,ownerRevision:1};assert.equal(x.read().owner.ownerRevision,1);assert.equal(cachedOwner.ownerRevision,0);
 const h=mountActualCombatHook({runtimeGeneration:1,runtimeRef:{current:x.f.runtime},read:()=>x.f.input,readBaseline:x.read,onWire:()=>{throw Error('unexpected wire');},onApproach:()=>{},onClear:()=>{}});
 assert.ok(h.api.diagnostics().snapshotPresent);assert.ok(x.f.sent.identity.controllerRun>run);assert.equal(x.f.edges.length,0);assert.equal(x.f.calls.length,0);
 assert.equal(x.scope.combatBaselineSourceRef.current,source);assert.equal(x.scope.combatRawRef.current.owner,cachedOwner);h.stop();
 for(const key of ['connectionGeneration','sessionGeneration','playerObjectId']){const old=x.f.input.owner;x.f.input.owner={...old,[key]:old[key]+1};assert.equal(x.read(),null,key);x.f.input.owner=old;}
 x.scope.worldRef.current.mapFileName='D002';assert.equal(x.read(),null);x.scope.worldRef.current.mapFileName='D001';
 x.scope.questSceneRevisionRef.current++;assert.equal(x.read(),null,'map round trip cannot recover an old scene source');x.scope.questSceneRevisionRef.current--;
 x.scope.equipmentStartGameRef.current={...x.session};assert.equal(x.read(),null,'same numeric epochs do not authenticate a new session object');x.scope.equipmentStartGameRef.current=x.session;
 x.scope.socketRef.current={readyState:1};assert.equal(x.read(),null,'same numeric epochs do not authenticate a replacement socket');x.scope.socketRef.current=x.socket;
 x.socket.readyState=3;assert.equal(x.read(),null);x.socket.readyState=1;x.scope.worldSnapshotVersionRef.current++;assert.equal(x.read(),null,'superseded source is never reused');
});
test('actual Page commit rejects changed physical custody during projection and invalid latest skills cannot restore older valid data',()=>{
 for(const change of ['session','socket','scene','version','map','inventory']){
  const x=combatSourceFixture();x.commit();const prior=x.scope.combatBaselineSourceRef.current;
  x.scope.duringProjection=()=>{if(change==='session')x.scope.equipmentStartGameRef.current={...x.session};if(change==='socket')x.scope.socketRef.current={readyState:1};
   if(change==='scene')x.scope.questSceneRevisionRef.current++;if(change==='version')x.scope.worldSnapshotVersionRef.current++;
   if(change==='map')x.scope.worldRef.current.mapFileName='D002';if(change==='inventory')x.scope.projectBevyBagModel=()=>({ok:false});};
  x.commit();assert.equal(x.scope.combatBaselineSourceRef.current,null,change);assert.equal(x.read(),null,change);assert.notEqual(x.scope.combatBaselineSourceRef.current,prior);
 }
 for(const skills of [null,Array.from({length:513},()=>({})),[null]]){
  const x=combatSourceFixture();x.commit();x.snapshot.knownSkills=skills;x.scope.worldSnapshotVersionRef.current++;x.commit();
  assert.deepEqual(x.read().snapshot.knownSkills,skills,'invalid authoritative rows retain their real value');assert.equal(x.f.host.diagnostics().snapshotPresent,false);assert.equal(x.f.host.diagnostics().rawPresent,false);
 }
 const x=combatSourceFixture();x.commit();x.snapshot.knownSkills=[{id:44,spell:'FireBall',hotkey:2}];x.scope.worldSnapshotVersionRef.current++;x.commit();
 assert.deepEqual(x.f.sent.learned,x.snapshot.knownSkills,'a fresh snapshot updates learned slots even under the same UI owner');
 x.snapshot.knownSkills=[{invalid:1n}];x.scope.worldSnapshotVersionRef.current++;assert.throws(x.commit);assert.equal(x.read(),null,'serialization failure never restores the prior valid source');
});
test('actual learned-skill packets retire the physical combat source while cast timing preserves it',()=>{
 for(const packet of ['NewMagic','RemoveMagic','MagicLeveled']){const x=combatSourceFixture();x.commit();
  x.api.captureSpellsGatewayEvent({type:'packet',packet,payload:{hero:true}},1);assert.ok(x.read(),'Hero skills do not retire player learned data');
  x.api.captureSpellsGatewayEvent({type:'packet',packet,payload:{}},1);assert.equal(x.read(),null);assert.equal(x.f.host.diagnostics().snapshotPresent,false);}
 const x=combatSourceFixture();x.commit();const physical=x.scope.combatBaselineSourceRef.current;
 x.api.captureSpellsGatewayEvent({type:'packet',packet:'MagicCast',payload:{spell:'FireBall'}},1);assert.equal(x.scope.combatBaselineSourceRef.current,physical);
});
test('actual Page delivers current HalfMoon ACK to combat and preserves it across later controller owners',()=>{
 const x=combatSourceFixture();x.snapshot.knownSkills=[{id:90,key:'halfMoon',spell:'HalfMoon',hotkey:16,castKind:'toggle',canUse:false}];x.commit();
 const physical=x.scope.combatBaselineSourceRef.current,cachedOwner=x.scope.combatRawRef.current.owner;
 x.scope.questSkillsBaselineCurrentRef.current=true;
 for(const [payload,connection] of [[{objectId:3,spell:'HalfMoon',canUse:true},2],[{objectId:4,spell:'HalfMoon',canUse:true},1],[{objectId:'3',spell:'HalfMoon',canUse:true},1],[{objectId:3,spell:'HalfMoon',canUse:1},1],[{objectId:3,spell:4,canUse:true},1],[{objectId:3,spell:'Unknown',canUse:true},1]]){
  x.api.captureSpellsGatewayEvent({type:'packet',packet:'SpellToggle',payload},connection);assert.equal(x.f.sent.learned[0].canUse,false);assert.equal(x.read().snapshot.knownSkills[0].canUse,false);
 }
 x.api.captureSpellsGatewayEvent({type:'packet',packet:'SpellToggle',payload:{objectId:3,spell:'HalfMoon',canUse:true}},1);
 assert.equal(x.f.sent.learned[0].canUse,true);assert.equal(x.f.sent.learned[0].castKind,'toggle');assert.equal(x.read().snapshot.knownSkills[0].canUse,true);
 assert.equal(x.scope.questSkillsBaselineCurrentRef.current,true);assert.equal(x.scope.combatBaselineSourceRef.current,physical);
 x.f.input.owner={...x.f.input.owner,ownerRevision:1};const baseline=x.read();assert.ok(x.f.host.observeSnapshot(baseline.snapshot,baseline.owner));
 assert.equal(x.f.sent.learned[0].canUse,true);assert.equal(x.scope.combatRawRef.current.owner,cachedOwner);
 x.api.captureSpellsGatewayEvent({type:'packet',packet:'SpellToggle',payload:{objectId:0,spell:'HalfMoon',canUse:false}},1);
 assert.equal(x.f.sent.learned[0].canUse,false);assert.equal(x.f.sent.learned[0].castKind,'toggle');
});
test('cached combat diagnostics never poll, send, withdraw or consume a proof',()=>{
 const f=fixture();let reads=0,writes=0,withdrawals=0;
 for(const key of ['getMir2CombatInputStatus','setMir2CombatInputSnapshot','setMir2CombatInputEdge','withdrawMir2CombatInputSnapshot']){
  const original=f.runtime[key];f.runtime[key]=(...args)=>{if(key==='getMir2CombatInputStatus')reads++;else if(key==='withdrawMir2CombatInputSnapshot')withdrawals++;else writes++;return original(...args);};
 }
 const sent=f.sent,first=f.host.diagnostics();assert.equal(first.snapshotAccepted,true);assert.equal(first.statusReady,true);
 for(let i=0;i<10;i++)assert.deepEqual(f.host.diagnostics(),first);
 assert.deepEqual([reads,writes,withdrawals],[0,0,0]);assert.equal(f.sent,sent);assert.equal(f.calls.length,0);
 assert.equal(f.emit().outcome,'confirmedSend');assert.equal(f.calls.length,1);
 assert.ok(f.host.edge({type:'select',objectId:4}));assert.equal(f.host.diagnostics().edgeAccepted,true);
});
test('diagnostics record disabled facts and a real rejected snapshot without bypassing either gate',()=>{
 const f=fixture();f.input.facts.enabled=false;f.host.tick();assert.equal(f.ready,false);
 assert.deepEqual(f.host.diagnostics().input,{factsPresent:true,enabled:false,ownerMatched:true});assert.equal(f.host.diagnostics().snapshotPresent,false);
 f.input.facts.enabled=true;f.runtime.setMir2CombatInputSnapshot=()=>false;f.host.tick();
 assert.equal(f.host.diagnostics().snapshotAccepted,false);assert.equal(f.host.diagnostics().snapshotPresent,false);assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.calls.length,0);
});
function fixture(existing){let sent=null,sink=null,now=10,frame=1,current=true,ready=false,clear=0,hook=null,clearWork=0;const calls=[],captures=[],edges=[];
 const runtime=existing??{getMir2CombatInputCapabilities:()=>JSON.stringify(caps),setMir2CombatInputSnapshot:json=>{captures.push(json);sent=JSON.parse(json);return true;},setMir2CombatInputEdge:json=>{edges.push(json);return '{"handled":true}';},getMir2CombatInputStatus:()=>JSON.stringify({version:1,frame:frame++,ready:Boolean(sent?.enabled),identity:sent?.identity??null,modelRevision:sent?.modelRevision??null,revision:sent?.revision??null,map:sent?.map??null}),setMir2CombatInputIntentSink:fn=>{sink=fn;},clearMir2CombatInputIntentSink:()=>{clear++;sink=null;},withdrawMir2CombatInputSnapshot:()=>{sent=null;}};
 const actor=(id,kind,x)=>({objectId:String(id),kind,x,y:10,direction:'up',dead:false,masterObjectId:0,ai:0});
 const input={owner:{connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3},facts:{map:'D001',enabled:true,player:actor(3,'selfPlayer',10),hp:10,maxHp:10,mp:10,level:1,attackSpeed:0,actors:[actor(3,'selfPlayer',10),actor(4,'monster',11)],clickTargets:[{kind:'monster',objectId:4,x:11,y:10,dead:false,ai:0,harvestable:false}],selectedObjectId:4,hoveredObjectId:'4',cursor:[12,10],class:'Warrior',hasClassWeapon:true,ridingMount:false,fishing:false,dazed:false,motionRemainingMs:0,movementReady:true,planningPosition:null,neighbours:[{x:11,y:9,cost:1}],spellLockKey:'None',pointerSpellLock:false,bindings:null}};
 const host=new CombatHost({runtime,isCurrent:()=>current,read:()=>input,now:()=>now,onReady:v=>{ready=v;},onWire:(p,b)=>{calls.push([p,b]);if(hook)return hook(p,b);assert.ok(host.allows(p));assert.ok(host.allows(p));assert.ok(host.claim(p,b));assert.equal(host.claim(p,b),false);return 'confirmedSend';},onApproach:()=>{},onClear:()=>{clearWork++;}});
 const raw={playerObjectId:3,knownSkills:[{key:'fireball',spell:'FireBall',magicName:'火球',hotkey:1,castKind:'target',mpCost:5,icon:0,need2:null}]};host.observeSnapshot(raw,input.owner);
 const proof=(changes={})=>({identity:{...input.owner,controllerRun:host.run},modelRevision:sent?.modelRevision??0,map:'D001',sequence:host.run*2**22+1,edgeSequence:1,movementBlockMs:0,slot:1,skillId:0,spell:'FireBall',targetId:4,command:{type:'magic',objectId:3,spell:'FireBall',direction:'right',targetId:4,x:11,y:10,spellTargetLock:false},...changes});
 return{host,input,runtime,raw,calls,captures,edges,proof,emit:p=>JSON.parse(sink(JSON.stringify({type:'wire',proof:p??proof()}))),get sent(){return sent;},get sink(){return sink;},get ready(){return ready;},get clear(){return clear;},get clearWork(){return clearWork;},set hook(v){hook=v;},set now(v){now=v;},set current(v){current=v;},set frame(v){frame=v;}};
}
test('independent exact combat caps preserve old fallback and recognize compiled startup waiting',()=>{const f=fixture();assert.ok(supportsCombat(f.runtime));assert.equal(supportsCombat({}),false);assert.equal(supportsCombat({...f.runtime,getMir2CombatInputCapabilities:()=>JSON.stringify({...caps,questUiAbiVersion:1})}),false);assert.ok(supportsCombat({...f.runtime,getMir2CombatInputCapabilities:()=>JSON.stringify({...caps,startup:false})}));});
test('combat capabilities explicitly distinguish forced-player input v2 from ordinary v1',()=>{
 const f=fixture();assert.equal(supportsForcedPlayerAttack(f.runtime),false);
 const v2={...f.runtime,getMir2CombatInputCapabilities:()=>JSON.stringify({...caps,combatInputAbiVersion:2})};
 assert.ok(supportsCombat(v2));assert.ok(supportsForcedPlayerAttack(v2));
 for(const invalid of [{combatInputAbiVersion:0},{combatInputAbiVersion:3},{combatInputAbiVersion:'2'},{combatIntentAbiVersion:2},{schemaVersion:2}]){
  const runtime={...f.runtime,getMir2CombatInputCapabilities:()=>JSON.stringify({...caps,...invalid})};
  assert.equal(supportsCombat(runtime),false);assert.equal(supportsForcedPlayerAttack(runtime),false);
 }
 const runtimeSource=readFileSync(new URL('../../game-client/runtime/src/combat_input_host.rs',import.meta.url),'utf8');
 assert.match(runtimeSource,/"combatInputAbiVersion":2,"combatIntentAbiVersion":1/);
});
test('new host omits forced input for strict v1 runtimes and preserves monster dispatch',()=>{
 const f=fixture(),legacyKeys=Object.keys(f.sent).sort(),set=f.runtime.setMir2CombatInputSnapshot;
 f.runtime.setMir2CombatInputSnapshot=json=>{
  assert.deepEqual(Object.keys(JSON.parse(json)).sort(),legacyKeys,'legacy deny_unknown_fields schema');return set(json);
 };
 f.input.facts.forceAttack=true;f.input.facts.hp--;
 f.host.tick();assert.ok(f.ready);assert.equal(Object.hasOwn(f.sent,'forceAttack'),false);
 const sent=f.sent,captures=f.captures.length;
 f.input.facts.forceAttack=false;f.host.tick();
 assert.equal(f.sent,sent);assert.equal(f.captures.length,captures,'Shift release does not change the v1 model');
 const proof=f.proof({slot:null,skillId:null,spell:null,command:{type:'attack',objectId:4}});
 assert.equal(f.emit(proof).outcome,'confirmedSend');assert.equal(f.calls.length,1);
});
test('v2 receives held and released forced input while downgrade removes the field',()=>{
 const f=fixture();let abi=2;f.runtime.getMir2CombatInputCapabilities=()=>JSON.stringify({...caps,combatInputAbiVersion:abi});
 f.host.tick();assert.equal(f.sent.forceAttack,false);
 f.input.facts.forceAttack=true;f.host.tick();assert.equal(f.sent.forceAttack,true);
 const held=f.sent.modelRevision;f.input.facts.forceAttack=false;f.host.tick();
 assert.equal(f.sent.forceAttack,false);assert.ok(f.sent.modelRevision>held);
 f.input.facts.forceAttack=true;abi=1;f.host.tick();assert.ok(f.ready);assert.equal(Object.hasOwn(f.sent,'forceAttack'),false);
 abi=3;f.host.tick();assert.equal(f.sent,null);assert.equal(f.ready,false);
});
test('actual setter and event capture preserve raw null/zero and explicit slot16',()=>{const f=fixture();assert.equal(f.sent.learned[0].icon,0);assert.equal(f.sent.learned[0].need2,null);assert.equal(f.sent.learned[0].magicName,'火球');assert.ok(f.host.edge({type:'key',key:'F1',modifiers:{ctrl:false,alt:false,shift:false,tilde:false}}));const edge=JSON.parse(f.edges.at(-1));assert.deepEqual(edge.identity,f.sent.identity);assert.equal(edge.sequence,1);assert.equal(edge.action.type,'key');
 if(process.env.MIR2_M11_COMBAT_SNAPSHOT_FIXTURE)writeFileSync(process.env.MIR2_M11_COMBAT_SNAPSHOT_FIXTURE,f.captures.at(-1),{flag:'wx'});
 if(process.env.MIR2_M11_COMBAT_EDGE_FIXTURE)writeFileSync(process.env.MIR2_M11_COMBAT_EDGE_FIXTURE,f.edges.at(-1),{flag:'wx'});
 f.host.edge({type:'slot',slot:16,cursor:null});assert.equal(JSON.parse(f.edges.at(-1)).action.slot,16);
});
test('pure repeated peek then one claim allows exactly one actual dispatch',()=>{const f=fixture();assert.equal(f.emit().outcome,'confirmedSend');assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.calls.length,1);assert.equal(f.host.allows(f.proof()),false);});
for(const outcome of ['definitelyUnsent','outcomeUnknown'])test(`${outcome} burns sequence without automatic replay`,()=>{const f=fixture();f.hook=()=>outcome;assert.equal(f.emit().outcome,outcome);f.now=60;f.host.tick();assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.calls.length,1);});
test('synchronous owner/runtime/skill/target/map mutations prevent final claim',()=>{for(const mutation of [f=>f.input.owner={...f.input.owner,sessionGeneration:9},f=>{f.current=false;},f=>f.host.observeSnapshot({...f.raw,knownSkills:[]},f.input.owner),f=>f.input.facts.actors.pop(),f=>{f.input.facts.map='D002';}]){const f=fixture();f.hook=(p,b)=>{assert.ok(f.host.allows(p));mutation(f);return f.host.claim(p,b)?'confirmedSend':'definitelyUnsent';};assert.equal(f.emit().outcome,'definitelyUnsent');}});
test('nested synchronous same-proof listener cannot claim or resend twice',()=>{const f=fixture();f.hook=(p,b)=>{assert.equal(f.emit(p).outcome,'definitelyUnsent');assert.ok(f.host.claim(p,b));assert.equal(f.host.claim(p,b),false);return 'confirmedSend';};assert.equal(f.emit().outcome,'confirmedSend');assert.equal(f.calls.length,1);});
test('serialization binds plain exact command and rejects extra or malformed wire',()=>{const f=fixture();assert.equal(parseCombatProof(f.proof({command:{...f.proof().command,debug:true}})),null);assert.equal(parseCombatProof(f.proof({command:{...f.proof().command,targetId:-1}})),null);assert.equal(parseCombatProof(f.proof({command:{...f.proof().command,direction:'unknown'}})),null);assert.equal(f.emit(f.proof({sequence:(f.host.run+1)*2**22+1})).outcome,'definitelyUnsent');f.hook=(p,b)=>f.host.claim(p,b+' ')?'confirmedSend':'definitelyUnsent';assert.equal(f.emit().outcome,'definitelyUnsent');});
test('raw exact local row/spell survives only actual unique current binding',()=>{for(const rows of [[{...fixture().raw.knownSkills[0],spell:'Healing'}],[{spell:'Healing'},...fixture().raw.knownSkills],[...fixture().raw.knownSkills,...fixture().raw.knownSkills],[{...fixture().raw.knownSkills[0],hotkey:16}]]){const f=fixture();f.host.observeSnapshot({...f.raw,knownSkills:rows},f.input.owner);assert.equal(f.emit().outcome,'definitelyUnsent');}});
for(const [name,rows] of [['nonarray',null],['oversize',Array.from({length:513},()=>({spell:'FireBall'}))]])test(`bad current ${name} stays withdrawn after 50ms; valid recover and stale bad owner cannot revoke`,()=>{const f=fixture(),old=f.input.owner;assert.ok(f.ready);assert.equal(f.host.observeSnapshot({...f.raw,knownSkills:rows},old),false);f.now=60;f.host.tick();assert.equal(f.ready,false);assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.calls.length,0);assert.ok(f.host.observeSnapshot(f.raw,old));assert.ok(f.ready);f.input.owner={...old,sessionGeneration:3,playerObjectId:5};f.input.facts.player.objectId='5';assert.ok(f.host.observeSnapshot({...f.raw,playerObjectId:5},f.input.owner));const before=f.sent;assert.equal(f.host.observeSnapshot({...f.raw,knownSkills:rows},old),false);f.host.tick();assert.equal(f.sent,before);assert.ok(f.ready);});
test('actual cast clock uses current connection observation and exact nonzero actor delay',()=>{const f=fixture();assert.equal(f.host.observePacket('MagicDelay',{spell:'FireBall',delay:3400},f.input.owner),false);assert.equal(f.host.observePacket('MagicDelay',{spell:'FireBall',objectId:0,delay:3400},f.input.owner),false);f.now=100;assert.ok(f.host.observePacket('MagicCast',{spell:'FireBall'},f.input.owner));f.now=200;assert.ok(f.host.observePacket('MagicDelay',{spell:'FireBall',objectId:3,delay:3400},f.input.owner));assert.deepEqual(f.sent.timing,[{spell:'FireBall',sequence:1,observedAtMs:100,delayMs:3400}]);assert.equal(f.host.observePacket('MagicCast',{spell:'FireBall'},{...f.input.owner,sessionGeneration:9}),false);assert.equal(f.host.observePacket('Magic',{spell:'FireBall'},f.input.owner),false);assert.equal(f.host.observePacket('MagicDelay',{spell:'FireBall',objectId:4,delay:10},f.input.owner),false);f.host.observeSnapshot({...f.raw,snapshotSerial:999},f.input.owner);assert.equal(f.sent.timing[0].sequence,1);});
test('combat HalfMoon ACK updates enabled state without changing its definition or trusting another actor/session',()=>{
 const f=fixture();f.raw.knownSkills=[{id:90,key:'halfMoon',spell:'HalfMoon',hotkey:16,castKind:'toggle',canUse:false},{id:91,spell:'Slaying',castKind:'passive',canUse:false}];assert.ok(f.host.observeSnapshot(f.raw,f.input.owner));
 for(const [payload,owner] of [[{objectId:4,spell:'HalfMoon',canUse:true},f.input.owner],[{objectId:3,spell:'HalfMoon',canUse:true},{...f.input.owner,sessionGeneration:9}],[{objectId:3,spell:'HalfMoon',canUse:'true'},f.input.owner],[{objectId:'3',spell:'HalfMoon',canUse:true},f.input.owner],[{objectId:3,spell:4,canUse:true},f.input.owner],[{objectId:3,spell:'Unknown',canUse:true},f.input.owner]]){
  assert.equal(f.host.observePacket('SpellToggle',payload,owner),false);assert.equal(f.sent.learned[0].canUse,false);
 }
 assert.ok(f.host.observePacket('SpellToggle',{objectId:3,spell:'HalfMoon',canUse:true},f.input.owner));assert.equal(f.sent.learned[0].canUse,true);assert.equal(f.sent.learned[0].castKind,'toggle');
 assert.equal(f.raw.knownSkills[0].canUse,false,'host retains detached authoritative rows');
 assert.ok(f.host.observePacket('SpellToggle',{objectId:0,spell:'HalfMoon',canUse:false},f.input.owner));assert.equal(f.sent.learned[0].castKind,'toggle');
 assert.ok(f.host.observePacket('SpellToggle',{objectId:3,spell:'Slaying',canUse:true},f.input.owner));assert.equal(f.sent.learned[1].castKind,'passive');
});
test('delay before first cast is metadata; repeated delay never resets real start',()=>{const f=fixture();f.raw.knownSkills[0].delayMs=200;f.host.observeSnapshot(f.raw,f.input.owner);assert.ok(f.host.observePacket('MagicDelay',{spell:'FireBall',objectId:3,delay:2200},f.input.owner));assert.deepEqual(f.sent.timing,[{spell:'FireBall',sequence:0,observedAtMs:10,delayMs:2200}]);f.now=100;assert.ok(f.host.observePacket('MagicCast',{spell:'FireBall'},f.input.owner));f.now=500;assert.ok(f.host.observePacket('MagicDelay',{spell:'FireBall',objectId:3,delay:2200},f.input.owner));assert.deepEqual(f.sent.timing,[{spell:'FireBall',sequence:1,observedAtMs:100,delayMs:2200}]);assert.equal(f.host.observePacket('MagicDelay',{spell:'FireBall',objectId:3,delay:9999},{...f.input.owner,sessionGeneration:9}),false);});
test('confirmed owner packet metadata survives before the first complete raw model',()=>{const f=fixture();const host=new CombatHost({runtime:f.runtime,isCurrent:()=>true,read:()=>f.input,now:()=>10,onReady:()=>{},onWire:()=> 'definitelyUnsent',onApproach:()=>{},onClear:()=>{}});assert.ok(host.observePacket('MagicDelay',{spell:'FireBall',objectId:3,delay:2200},f.input.owner));assert.ok(host.observePacket('MagicCast',{spell:'FireBall'},f.input.owner));assert.ok(host.observeSnapshot(f.raw,f.input.owner));assert.deepEqual(f.sent.timing,[{spell:'FireBall',sequence:1,observedAtMs:10,delayMs:2200}]);});
test('same-runtime HMR/late cleanup/owner replacement burn new runs without old takeover',()=>{const f=fixture(),old=f.host;let next;next=new CombatHost({runtime:f.runtime,isCurrent:()=>true,read:()=>f.input,now:()=>10,onReady:()=>{},onWire:()=> 'definitelyUnsent',onApproach:()=>{},onClear:()=>{}});next.observeSnapshot(f.raw,f.input.owner);const sink=f.sink,before=f.sent;f.input.owner={...f.input.owner,ownerRevision:1};assert.equal(old.observeSnapshot(f.raw,f.input.owner),false);old.stop();assert.equal(f.sink,sink);assert.equal(f.clear,0);assert.ok(next.run>old.run);next.observeSnapshot(f.raw,f.input.owner);assert.ok(next.run>before.identity.controllerRun);});
test('failed sink setup burns global namespace; clock rollback and throwing providers withdraw',()=>{const f=fixture(),failed=new CombatHost({runtime:{...f.runtime,setMir2CombatInputIntentSink:()=>{throw Error('setup');},clearMir2CombatInputIntentSink:()=>{}},isCurrent:()=>true,read:()=>f.input,now:()=>10,onReady:()=>{},onWire:()=> 'confirmedSend',onApproach:()=>{},onClear:()=>{}});assert.ok(failed.run>f.host.run);f.now=9;f.host.tick();assert.equal(f.ready,false);assert.equal(f.emit().outcome,'definitelyUnsent');f.now=10;assert.ok(f.host.observeSnapshot(f.raw,f.input.owner));assert.ok(f.ready);Object.defineProperty(f.input,'facts',{get(){throw Error('provider');}});f.now=60;f.host.tick();assert.equal(f.ready,false);assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.calls.length,0);});
test('clear and same owner reopen preserve edge highwater; frozen frame withdraws ready',()=>{const f=fixture();f.host.edge({type:'slot',slot:1,cursor:null});const first=JSON.parse(f.edges.at(-1));f.host.withdraw();f.host.observeSnapshot(f.raw,f.input.owner);f.host.edge({type:'slot',slot:1,cursor:null});const next=JSON.parse(f.edges.at(-1));assert.equal(next.identity.controllerRun,first.identity.controllerRun);assert.ok(next.sequence>first.sequence);f.runtime.getMir2CombatInputStatus=()=>JSON.stringify({version:1,frame:1,ready:true,identity:f.sent.identity,modelRevision:f.sent.modelRevision,revision:f.sent.revision,map:f.sent.map});f.host.tick();f.now=1000;f.host.tick();assert.equal(f.ready,false);});
test('production integration captures raw before coalescing and final event proof claim is adjacent to socket send',()=>{
 const p=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),s=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8'),r=readFileSync(new URL('../../game-client/runtime/src/lib.rs',import.meta.url),'utf8');
 assert.ok(p.indexOf('captureSpellsGatewayEvent(event,connectionGeneration)')<p.indexOf('const debugEvent ='));
 const send=p.slice(p.indexOf('function sendRaw('),p.indexOf('function sendSpectatorControl'));
 assert.ok(send.indexOf('window.dispatchEvent')<send.indexOf('combatIngressRef.current.claim'));
 assert.match(send,/claim\(options.combatProof,serialized\)\)\)return false;\s*if\(options\?\.mailProof\)\{[\s\S]*?mailDispatcherRef\.current\.claim\(p\)/);
 const sendAst=ts.createSourceFile('send.tsx',send,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 const statements=sendAst.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==='sendRaw').body.statements;
 const normalized=node=>node.getText(sendAst).replace(/\s+/g,' ');
 const claimIndex=statements.findIndex(n=>n.getText(sendAst).includes('combatIngressRef.current.claim'));
 const socketTries=statements.filter(n=>ts.isTryStatement(n)&&n.tryBlock.statements.some(statement=>statement.getText(sendAst)==='socket.send(serialized);'));
 assert.equal(socketTries.length,1,'sole actual socket send try boundary');
 const socketTry=socketTries[0],socketIndex=statements.indexOf(socketTry);
 assert.ok(claimIndex>=0&&socketIndex>claimIndex);
 assert.equal(socketTry.tryBlock.statements.length,1,'no callback before or after socket send inside try');
 const actualSocketSend=socketTry.tryBlock.statements[0];
 assert.ok(ts.isExpressionStatement(actualSocketSend)&&ts.isCallExpression(actualSocketSend.expression));
 assert.equal(actualSocketSend.getText(sendAst),'socket.send(serialized);');
 assert.equal(socketTry.finallyBlock,undefined);
 const rankingInspectCondition='wireCommand.type === "inspect" && (!options?.rankingInspectProof ||socketRef.current!==socket||socket.readyState!==WebSocket.OPEN||options.rankingInspectProof.owner.socket!==socket ||!rankingInspectRequestsRef.current.claim(options.rankingInspectProof,currentRankingInspectSource(),wireCommand,Math.floor(performance.now())))';
 assert.deepEqual(statements.slice(claimIndex+1,socketIndex).map(n=>ts.isIfStatement(n)?normalized(n.expression):null),[
  'options?.mailProof','options?.mailQuoteProof','options?.mailLockProof',
  "options?.mailProof?.commandType==='sendMail'&&(!options.mailProof.compose||!mailDispatcherRef.current?.composer.enterSocket(options.mailProof.compose,serialized,socket))",
  'options?.ownerToken && "surface" in options.ownerToken && options.ownerToken.surface === "storage" && (!storageIntentMatchesCommand(options.ownerToken.intent, wireCommand) || storageUiIngressRef.current?.claim(options.ownerToken.intent) !== true)',
  'storageReservation',
  'wireCommand.type === "buyItem" && options?.npcLegacyBuyProof && !legacyNpcBuyAllowed(options.npcLegacyBuyProof, wireCommand, true)',
  'wireCommand.type === "buyItem" && !options?.npcUi && npcShopUiIngressRef.current?.blocksInput()',
  'options?.npcUi && (!npcShopIntentMatchesCommand(options.npcUi.intent, wireCommand) || npcShopUiIngressRef.current?.claim(options.npcUi.intent) !== true)',
  'options?.npcBuyProof && !npcBuyDispatcher?.claim(options.npcBuyProof, wireCommand, serialized, socket, options.npcUi ? () => npcShopUiSendCurrent(options.npcUi!, socket) : () => !npcShopUiIngressRef.current?.blocksInput() && socketRef.current === socket && socket.readyState === WebSocket.OPEN)',
  'options?.socialReplyProof && (!socialReplyWindowOpen(options.socialReplyProof.request.kind) || !socialRepliesRef.current.claim(options.socialReplyProof, currentSocialReplyOwner(), wireCommand))',
  'options?.storageRentalProof && !storageRentalRef.current.claim(options.storageRentalProof, currentStorageRentalFacts(), wireCommand)',
  'wireCommand.type === "guildStorageItemChange" && wireCommand.changeType === 3',
  '!socialItemMutationAllowed(wireCommand, options?.socialItemProof)',
  'options?.socialItemProof && (!socialItemProofCurrent(options.socialItemProof) || !socialItemOperationsRef.current.claim(options.socialItemProof, currentSocialReplyOwner(), readSocialItemSurface(options.socialItemProof.surface)?.sourceKey ?? "", wireCommand))',
  'options?.socialTradeProof && !socialTradeProofCurrent(options.socialTradeProof, wireCommand)',
  'options?.socialTradeProof && wireCommand.type === "tradeReply"',
  'options?.socialRosterProof && !socialRosterProofCurrent(options.socialRosterProof, wireCommand)',
  'options?.rankingProof && !rankingQueriesRef.current.claim(options.rankingProof, currentSocialReplyOwner(), wireCommand)',
  'options?.rankingProof',
  'wireCommand.type === "startGame"',
  'options?.guildBuffProof',
  'options?.heroProof',
  'options?.cashProof && (!cashShopOpenRef.current || !cashPurchasesRef.current.claim(options.cashProof, currentCashGameShopSource(), wireCommand))',
  'options?.creatureProof',
  'options?.mailProof?.commandType === "collectParcel"',
  'options?.modeProof && !combatModeHostRef.current?.claim(options.modeProof, wireCommand)',
  'socketRef.current !== socket || socket.readyState !== WebSocket.OPEN',
  null, 'authKind', 'options?.npcRepairProof',
  'isMailItemMutation(wireCommand) || ["dropGold", "tradeGold", "gameShopBuy", "sendMail", "collectParcel"].includes(String(wireCommand.type))',
  'options?.bagBeltProof',
  'options?.worldFishingProof && !enterWorldFishingSend(options.worldFishingProof, wireCommand, socket)',
  'options?.npcPearlBuyProof && !npcBuyDispatcher?.claimPearl(options.npcPearlBuyProof,wireCommand,serialized,socket, () => npcPearlSendCurrent(options.npcPearlBuyProof!,wireCommand,socket))',
  rankingInspectCondition,
 ]);

 // The original ordered parity suffix ends at the two additive Auth nodes.
 // Repair adds a command-scoped proof gate and a local inventory retire.
 // Bag/Belt entry follows both nodes; the additive WorldFishing entry is last before transport.
 const authClaimIndex=statements.findIndex(n=>ts.isIfStatement(n)&&normalized(n.expression)==='authKind');
 const authDeclaration=statements[authClaimIndex-1],authClaim=statements[authClaimIndex];
 assert.ok(ts.isVariableStatement(authDeclaration));
 assert.equal(authDeclaration.declarationList.flags,ts.NodeFlags.Const);
 assert.equal(authDeclaration.declarationList.declarations.length,1);
 assert.equal(authDeclaration.modifiers,undefined);
 assert.equal(normalized(authDeclaration),'const authKind = preauthCommandKind(command);');
 const authVariable=authDeclaration.declarationList.declarations[0];
 assert.equal(authVariable.name.getText(sendAst),'authKind');assert.equal(authVariable.type,undefined);
 assert.ok(ts.isCallExpression(authVariable.initializer));
 assert.equal(authVariable.initializer.expression.getText(sendAst),'preauthCommandKind');
 assert.deepEqual(authVariable.initializer.arguments.map(n=>n.getText(sendAst)),['command']);
 assert.ok(ts.isIfStatement(authClaim));assert.equal(normalized(authClaim.expression),'authKind');
 assert.equal(authClaim.elseStatement,undefined);assert.ok(ts.isBlock(authClaim.thenStatement));
 const authStatements=authClaim.thenStatement.statements;
 assert.equal(authStatements.length,2);
 assert.equal(normalized(authStatements[0]),'const active = authSendRef.current;');
 const authRefusal=authStatements[1];
 assert.ok(ts.isIfStatement(authRefusal));assert.equal(authRefusal.elseStatement,undefined);
 assert.equal(normalized(authRefusal.expression),'!active || active.attempt.command !== command || active.proof !== options?.authProof || (!active.attempt.reconnect && !authSurfaceCurrent(active.attempt.epoch)) || !preauthGate().enter(active.proof, socket, equipmentConnectionGenerationRef.current, authKind)');
 assert.ok(ts.isReturnStatement(authRefusal.thenStatement));
 assert.equal(authRefusal.thenStatement.expression.kind,ts.SyntaxKind.FalseKeyword);
 assert.equal(statements[authClaimIndex-2].getText(sendAst).startsWith('if (socketRef.current !== socket || socket.readyState !== WebSocket.OPEN)'),true);
 const repairGate=statements[authClaimIndex+1],repairRetire=statements[authClaimIndex+2];
 assert.ok(ts.isIfStatement(repairGate));assert.equal(normalized(repairGate.expression),'options?.npcRepairProof');
 assert.equal(repairGate.elseStatement,undefined);assert.ok(ts.isBlock(repairGate.thenStatement));
 assert.deepEqual(repairGate.thenStatement.statements.map(normalized),[
  'const repairView = readNpcRepairView();',
  'if (socketRef.current !== socket || socket.readyState !== WebSocket.OPEN || !npcRepairAuthorityRef.current.enter(options.npcRepairProof, repairView, wireCommand)) return false;'
 ]);
 assert.ok(ts.isIfStatement(repairRetire));
 assert.equal(normalized(repairRetire.expression),'isMailItemMutation(wireCommand) || ["dropGold", "tradeGold", "gameShopBuy", "sendMail", "collectParcel"].includes(String(wireCommand.type))');
 assert.ok(ts.isBlock(repairRetire.thenStatement));assert.equal(repairRetire.thenStatement.statements.length,1);
 assert.equal(normalized(repairRetire.thenStatement.statements[0]),'npcRepairAuthorityRef.current.invalidateInventory();');
 const bagProofGate=statements[authClaimIndex+3];
 assert.ok(ts.isIfStatement(bagProofGate));assert.equal(normalized(bagProofGate.expression),'options?.bagBeltProof');
 assert.equal(bagProofGate.elseStatement,undefined);assert.ok(ts.isBlock(bagProofGate.thenStatement));
 assert.deepEqual(bagProofGate.thenStatement.statements.map(normalized),[
  'const proof = options.bagBeltProof, owner = bagBeltPhysicalOwner();',
  'if (!owner || socketRef.current !== socket || socket.readyState !== WebSocket.OPEN || !bagBeltFinalCurrent(proof, wireCommand) || !bagBeltMovesRef.current.enter(proof.reservation, owner, wireCommand)) return false;'
 ]);
 const worldFishingProofGate=statements[authClaimIndex+4];
 assert.ok(ts.isIfStatement(worldFishingProofGate));
 assert.equal(normalized(worldFishingProofGate.expression),'options?.worldFishingProof && !enterWorldFishingSend(options.worldFishingProof, wireCommand, socket)');
 assert.equal(worldFishingProofGate.elseStatement,undefined);
 assert.ok(ts.isReturnStatement(worldFishingProofGate.thenStatement));
 assert.equal(worldFishingProofGate.thenStatement.expression.kind,ts.SyntaxKind.FalseKeyword);
 const pearlGate=statements[authClaimIndex+5];
 assert.ok(ts.isIfStatement(pearlGate));assert.equal(normalized(pearlGate.expression),'options?.npcPearlBuyProof && !npcBuyDispatcher?.claimPearl(options.npcPearlBuyProof,wireCommand,serialized,socket, () => npcPearlSendCurrent(options.npcPearlBuyProof!,wireCommand,socket))');
 assert.equal(pearlGate.elseStatement,undefined);assert.ok(ts.isReturnStatement(pearlGate.thenStatement));
 assert.equal(normalized(pearlGate.thenStatement.expression),'rejectNpcPurchaseSendGate("pearlClaimRejected")');
 const rejectionDeclaration=statements.find(n=>ts.isVariableStatement(n)&&n.declarationList.declarations.some(d=>d.name.getText(sendAst)==='rejectNpcPurchaseSendGate'));
 assert.ok(rejectionDeclaration,'Pearl rejection helper is a local pre-send declaration');
 const rejectionBody=rejectionDeclaration.declarationList.declarations.find(d=>d.name.getText(sendAst)==='rejectNpcPurchaseSendGate').initializer.body;
 assert.ok(ts.isBlock(rejectionBody));
 const rejectReturn=rejectionBody.statements.at(-1);assert.ok(ts.isReturnStatement(rejectReturn));assert.equal(rejectReturn.expression.kind,ts.SyntaxKind.FalseKeyword);
 const rankingInspectGate=statements[authClaimIndex+6];
 assert.ok(ts.isIfStatement(rankingInspectGate));assert.equal(normalized(rankingInspectGate.expression),rankingInspectCondition);
 assert.equal(rankingInspectGate.elseStatement,undefined);assert.ok(ts.isReturnStatement(rankingInspectGate.thenStatement));
 assert.equal(rankingInspectGate.thenStatement.expression.kind,ts.SyntaxKind.FalseKeyword);
 assert.equal(statements[authClaimIndex+7],socketTry,'no executable external callback between final Pearl Core entry and sole socket send');
 assert.equal(socketIndex,authClaimIndex+7);
 // Every pre-send statement after the combat claim is covered by the exact
 // ordered list above, including Repair/retire, Bag/Belt and final WorldFishing entry.
 assert.deepEqual(statements.slice(claimIndex+1,socketIndex).filter(n=>ts.isIfStatement(n)).map(n=>normalized(n.expression)).slice(-6),[
  'options?.npcRepairProof',
  'isMailItemMutation(wireCommand) || ["dropGold", "tradeGold", "gameShopBuy", "sendMail", "collectParcel"].includes(String(wireCommand.type))',
  'options?.bagBeltProof',
  'options?.worldFishingProof && !enterWorldFishingSend(options.worldFishingProof, wireCommand, socket)',
  'options?.npcPearlBuyProof && !npcBuyDispatcher?.claimPearl(options.npcPearlBuyProof,wireCommand,serialized,socket, () => npcPearlSendCurrent(options.npcPearlBuyProof!,wireCommand,socket))',
  rankingInspectCondition
 ]);
 // Execute the actual final Auth fragment with its actual durable gate and
 // Page surface check. Only socket writes and document facts are memory data.
 const authSource=readFileSync(new URL('../lib/client-login-runtime.ts',import.meta.url),'utf8');
 const authAst=ts.createSourceFile('pure-auth.ts',authSource,ts.ScriptTarget.Latest,true);
 const authNames=['PreauthFlightGate','preauthCommandKind','preauthGateKey','persistentPreauthGate'];
 const authNodes=authAst.statements.filter(n=>(ts.isClassDeclaration(n)||ts.isFunctionDeclaration(n))&&authNames.includes(n.name?.text)
  ||ts.isVariableStatement(n)&&n.declarationList.declarations.some(d=>authNames.includes(d.name.getText(authAst))));
 assert.equal(authNodes.length,authNames.length);
 const pageAst=ts.createSourceFile('auth-page.tsx',p,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),surfaceNodes=[];
 function findAuth(node){if(ts.isFunctionDeclaration(node)&&['authSurfaceCurrent','preauthGate'].includes(node.name?.text))surfaceNodes.push(node);ts.forEachChild(node,findAuth);}
 findAuth(pageAst);assert.equal(surfaceNodes.length,2);
 const authCode=ts.transpileModule(authNodes.map(n=>n.getText(authAst).replace(/^export /,'')).join('\n')+'\n'
  +surfaceNodes.map(n=>n.getText(pageAst)).join('\n')+'\n'
  +'return {PreauthFlightGate,run(command,options,socket){'
  +[authDeclaration,authClaim,actualSocketSend].map(n=>n.getText(sendAst)).join('\n')+'\nreturn true;}};',
  {compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const authFactory=new Function('authSendRef','preauthGateRef','equipmentConnectionGenerationRef','loginAuthRef','screenRef','document','serialized',authCode);
 for(const change of ['none','no-active','command','active-proof','option-proof','epoch','generation','socket','hidden','screen']){
  const calls=[],socket={send:body=>calls.push(body)},authSendRef={current:null},preauthGateRef={current:null},
   generation={current:3},ui={current:{epoch:7,surface:'login'}},screen={current:'login'},document={visibilityState:'visible'};
  const auth=authFactory(authSendRef,preauthGateRef,generation,ui,screen,document,'wire');
  const gate=new auth.PreauthFlightGate();preauthGateRef.current=gate;
  const command=Object.freeze({type:'login',accountId:'User',password:'Secret'}),proof=gate.reserve(socket,3,7,'login');
  authSendRef.current={attempt:{epoch:7,command},proof};let options={authProof:proof},actualSocket=socket;
  if(change==='no-active')authSendRef.current=null;
  if(change==='command')authSendRef.current.attempt.command={...command};
  if(change==='active-proof')authSendRef.current.proof={...proof};
  if(change==='option-proof')options={authProof:{...proof}};
  if(change==='epoch')ui.current={epoch:8,surface:'login'};
  if(change==='generation')generation.current=4;
  if(change==='socket')actualSocket={send:()=>assert.fail('foreign Auth socket')};
  if(change==='hidden')document.visibilityState='hidden';
  if(change==='screen')screen.current='game';
  assert.equal(auth.run(command,options,actualSocket),change==='none',change);
  assert.deepEqual(calls,change==='none'?['wire']:[]);
  assert.equal(gate.pending(socket),change==='none');
  if(change==='none'){assert.equal(auth.run(command,options,socket),false);assert.deepEqual(calls,['wire']);}
 }
 const paritySocketIndex=authClaimIndex-1;
 const appendedClaims=statements.slice(paritySocketIndex-7,paritySocketIndex).filter(n=>normalized(n.expression)!=='options?.modeProof && !combatModeHostRef.current?.claim(options.modeProof, wireCommand)');
 assert.equal(appendedClaims.length,6,'the original six parity/socket guards remain ordered around the additive mode claim');
 const modeClaim=statements[paritySocketIndex-2];
 assert.equal(normalized(modeClaim.expression),'options?.modeProof && !combatModeHostRef.current?.claim(options.modeProof, wireCommand)');
 assert.ok(ts.isReturnStatement(modeClaim.thenStatement));assert.equal(modeClaim.thenStatement.expression.kind,ts.SyntaxKind.FalseKeyword);
 assert.equal(modeClaim.elseStatement,undefined);
 assert.equal(normalized(appendedClaims[0].thenStatement),'{const source = currentGuildBuffSource(); if (!socialItemWindowsRef.current.guild || !source || !guildBuffOperationsRef.current.claim(options.guildBuffProof,source,wireCommand)) return false;}');
 assert.equal(normalized(appendedClaims[1].thenStatement),'{ if (!heroProofCurrent(options.heroProof, wireCommand)) return false; const model = currentHeroModel(); if (!model || !heroOperationsRef.current.claim(options.heroProof, model, wireCommand)) return false; }');
 assert.match(normalized(appendedClaims[3].thenStatement),/!creatureOperationsRef\.current\.claim\(options\.creatureProof, currentCreatureSource\(\), wireCommand\)/);
 assert.match(normalized(appendedClaims[3].thenStatement),/creatureRenameRef\.current = \{owner:options\.creatureProof\.source\.owner, enabled:false\}/);
 assert.match(normalized(appendedClaims[4].thenStatement),/mailCollectBarrierRef\.current = \{owner, mailId:wireCommand\.mailId as number, observedSnapshot:null\}/);
 for(const proof of ['heroProof','guildBuffProof','cashProof','creatureProof']) assert.match(normalized(appendedClaims[5].thenStatement),new RegExp('options\\?\\.'+proof));
 const npcFinalGuards=statements.slice(claimIndex+7,claimIndex+11);
 assert.equal(npcFinalGuards.length,4);
 const npcFinalRejections=['rejectNpcPurchaseSendGate("legacyClaimRejected")','rejectNpcPurchaseSendGate("finalInputBlocked")','false','rejectNpcPurchaseSendGate("goldClaimRejected")'];
 for(const [index,guard] of npcFinalGuards.entries()){assert.ok(ts.isIfStatement(guard));assert.equal(guard.elseStatement,undefined);assert.ok(ts.isReturnStatement(guard.thenStatement));assert.equal(normalized(guard.thenStatement.expression),npcFinalRejections[index]);}
 // The whole actual early admission block is retained. Mixed NPC proofs on a
 // combat command and a Buy carrying both proof kinds must never reach entry.
 const npcEarlyGuards=statements.filter(n=>ts.isIfStatement(n)&&[
  '(options?.npcBuyProof || options?.npcPearlBuyProof || options?.npcLegacyBuyProof) && command.type !== "buyItem"',
  'command.type === "buyItem"',
 ].includes(normalized(n.expression)));
 assert.equal(npcEarlyGuards.length,2);
 assert.deepEqual(npcEarlyGuards.map(n=>normalized(n.expression)),[
  '(options?.npcBuyProof || options?.npcPearlBuyProof || options?.npcLegacyBuyProof) && command.type !== "buyItem"','command.type === "buyItem"']);
 const earlyBuy=npcEarlyGuards[1];assert.ok(ts.isBlock(earlyBuy.thenStatement));
 assert.deepEqual(earlyBuy.thenStatement.statements.map(n=>ts.isIfStatement(n)?normalized(n.expression):null),[
  '[options?.npcBuyProof,options?.npcPearlBuyProof,options?.npcLegacyBuyProof].filter(Boolean).length !== 1',
  'options?.npcBuyProof && !npcBuyDispatcher?.allows(options.npcBuyProof, command)',
  'options?.npcPearlBuyProof && !npcBuyDispatcher?.allowsPearl(options.npcPearlBuyProof, command)',
  'options?.npcLegacyBuyProof && !legacyNpcBuyAllowed(options.npcLegacyBuyProof, command)',
 ]);
 const receipt=statements[socketIndex+1];
 assert.ok(ts.isIfStatement(receipt));assert.equal(normalized(receipt.expression),'options?.npcBuyProof || options?.npcPearlBuyProof');
 assert.equal(receipt.thenStatement.getText(sendAst),'npcBuyDispatcher?.transportResult((options.npcPearlBuyProof ?? options.npcBuyProof)!, "flushed");');
 assert.equal(normalized(socketTry.catchClause.block),'{ if (options?.modeProof) combatModeHostRef.current?.outcomeUnknown(options.modeProof); if (options?.guildBuffProof) guildBuffOperationsRef.current.outcomeUnknown(options.guildBuffProof); if (options?.heroProof) heroOperationsRef.current.outcomeUnknown(options.heroProof); if (options?.cashProof) cashPurchasesRef.current.markUnknown(options.cashProof); if (options?.creatureProof) creatureOperationsRef.current.markUnknown(options.creatureProof); if (!options?.npcBuyProof && !options?.npcPearlBuyProof) throw error; npcBuyDispatcher?.transportResult((options.npcPearlBuyProof ?? options.npcBuyProof)!, "unknown"); console.error("[mir2] NPC purchase send outcome is unknown", error); return rejectNpcPurchaseSendGate("sendUnknown"); }');
 const npcGateCode=ts.transpileModule([rejectionDeclaration,...npcEarlyGuards,...npcFinalGuards,socketTry,receipt].map(n=>n.getText(sendAst)).join('\n')+'\nreturn true;',
  {compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const actualNpcGate=new Function('command','wireCommand','options','npcBuyDispatcher','legacyNpcBuyAllowed','socket','serialized','console',
  'npcShopUiIngressRef','npcShopIntentMatchesCommand','npcShopUiSendCurrent','socketRef','WebSocket','recordNpcPurchaseAction','npcPurchaseSendActionRef',npcGateCode);
 const runNpcGate=(command,wireCommand,options,dispatcher,legacy,socket,serialized,console)=>actualNpcGate(
  command,wireCommand,options,dispatcher,legacy,socket,serialized,console,{current:null},
  ()=>assert.fail('idle NPC boundary cannot receive a UI intent'),()=>assert.fail('idle NPC boundary cannot run shared UI entry'),
  {current:socket},{OPEN:1},()=>{},{current:null});
 for(const type of ['magic','attack','rangeAttack','harvest','magicKey']){
  const calls=[],socket={send:value=>calls.push(value)},dispatcher={allows:()=>{calls.push('allows');return false;},claim:()=>{calls.push('claim');return false;},transportResult:()=>calls.push('receipt')},legacy=()=>{calls.push('legacy');return false;};
  assert.equal(runNpcGate({type},{type},{combatProof:{}},dispatcher,legacy,socket,'wire',console),true);
  assert.deepEqual(calls,['wire'],'valid combat commands bypass both NPC proof callbacks');
  for(const options of [{npcBuyProof:{}},{npcLegacyBuyProof:{}},{npcBuyProof:{},npcLegacyBuyProof:{}},
    {npcPearlBuyProof:{}},{npcBuyProof:{},npcPearlBuyProof:{}},{npcPearlBuyProof:{},npcLegacyBuyProof:{}}]){
   calls.length=0;assert.equal(runNpcGate({type},{type},options,dispatcher,legacy,socket,'wire',console),false);
   assert.deepEqual(calls,[],'actual early guard refuses foreign NPC proofs before callbacks or socket');
  }
  calls.length=0;socket.send=()=>{throw Error('controlled combat socket failure');};
  assert.throws(()=>runNpcGate({type},{type},{combatProof:{}},dispatcher,legacy,socket,'wire',console),/controlled combat socket failure/);
  assert.deepEqual(calls,[],'combat socket failure is rethrown without an NPC receipt');
 }
 for(const kind of ['ordinary','legacy'])for(const allowed of [true,false]){
  const calls=[],proof={},wire={type:'buyItem'},socket={readyState:1,send:value=>calls.push(value)},options=kind==='ordinary'?{npcBuyProof:proof}:{npcLegacyBuyProof:proof};
  const dispatcher={allows:()=>true,claim:(...args)=>{assert.equal(args.length,5);assert.deepEqual(args.slice(0,4),[proof,wire,'wire',socket]);assert.equal(typeof args[4],'function');assert.equal(args[4](),true,'actual fifth-argument entry guard accepts only this idle current open socket');calls.push('claim');return allowed;},
   transportResult:(...args)=>{assert.deepEqual(args,[proof,'flushed']);calls.push('flushed');}};
  const legacy=(...args)=>{assert.equal(args[0],proof);assert.strictEqual(args[1],wire);if(args[2]===true){calls.push('legacy');return allowed;}assert.equal(args.length,2);return true;};
  assert.equal(runNpcGate(wire,wire,options,dispatcher,legacy,socket,'wire',console),allowed);
  assert.deepEqual(calls,kind==='ordinary'?(allowed?['claim','wire','flushed']:['claim']):(allowed?['legacy','wire']:['legacy']),
   'each actual Buy final proof refusal precedes the exact socket send');
 }
 const mixedCalls=[];
 assert.equal(runNpcGate({type:'buyItem'},{type:'buyItem'},{npcBuyProof:{},npcLegacyBuyProof:{}},
  {allows:()=>{mixedCalls.push('allows');return true;},claim:()=>{mixedCalls.push('claim');return true;}},
  ()=>{mixedCalls.push('legacy');return true;},{send:()=>mixedCalls.push('socket')},'wire',console),false);
 assert.deepEqual(mixedCalls,[],'the actual early Buy guard rejects both proof kinds without claiming either');
 const awaits=[],events=[];function findAwait(node){if(ts.isAwaitExpression(node))awaits.push(node);if(ts.isCallExpression(node)&&node.expression.getText(sendAst).endsWith('.dispatchEvent')||ts.isNewExpression(node))events.push(node);ts.forEachChild(node,findAwait);}
 statements.slice(claimIndex,socketIndex+1).forEach(findAwait);
 assert.equal(awaits.length,0,'no AwaitExpression between final combat claim and actual socket send');
 assert.equal(events.length,0,'no new constructor or synchronous event between final combat claim and actual socket send');
 // Keep the original ten-condition entry text assertion, independently of the
 // exact social/ranking AST guards above, and append only the asserted sole socket boundary.
 const entry=send.slice(send.indexOf("if(options?.mailProof?.commandType==='sendMail'"),send.indexOf('if (options?.socialReplyProof &&'))+'try { socket.send(serialized);';
 assert.match(entry.replace(/\s+/g,' ').trim(),/^if\(options\?\.mailProof\?\.commandType==='sendMail'&&\(!options\.mailProof\.compose\|\|!mailDispatcherRef\.current\?\.composer\.enterSocket\(options\.mailProof\.compose,serialized,socket\)\)\)return false; if \(options\?\.ownerToken && "surface" in options\.ownerToken && options\.ownerToken\.surface === "storage" && \(!storageIntentMatchesCommand\(options\.ownerToken\.intent, wireCommand\) \|\| storageUiIngressRef\.current\?\.claim\(options\.ownerToken\.intent\) !== true\)\) return false; if \(storageReservation\) storageReservation\.enteredSocket = true; if \(wireCommand\.type === "buyItem" && options\?\.npcLegacyBuyProof && !legacyNpcBuyAllowed\(options\.npcLegacyBuyProof, wireCommand, true\)\) return rejectNpcPurchaseSendGate\("legacyClaimRejected"\); if \(wireCommand\.type === "buyItem" && !options\?\.npcUi && npcShopUiIngressRef\.current\?\.blocksInput\(\)\) return rejectNpcPurchaseSendGate\("finalInputBlocked"\); if \(options\?\.npcUi && \(!npcShopIntentMatchesCommand\(options\.npcUi\.intent, wireCommand\) \|\| npcShopUiIngressRef\.current\?\.claim\(options\.npcUi\.intent\) !== true\)\) return false; if \(options\?\.npcBuyProof && !npcBuyDispatcher\?\.claim\(options\.npcBuyProof, wireCommand, serialized, socket, options\.npcUi \? \(\) => npcShopUiSendCurrent\(options\.npcUi!, socket\) : \(\) => !npcShopUiIngressRef\.current\?\.blocksInput\(\) && socketRef\.current === socket && socket\.readyState === WebSocket\.OPEN\)\) return rejectNpcPurchaseSendGate\("goldClaimRejected"\); try \{ socket\.send\(serialized\);$/);
 assert.match(s,/skillBarMatch && !combat\?\.supported/);assert.match(r,/combat_input_host::install\(&mut app\)/);
});

for(const kind of ['cancel','clear'])test('synchronous '+kind+' retires in-flight proof; replay fails and fresh tick output works',()=>{const f=fixture();let after=null;
 f.hook=(p,b)=>{assert.ok(f.host.allows(p));if(kind==='cancel')f.host.edge({type:'cancel'});else f.sink(JSON.stringify({type:'clear'}));after={allows:f.host.allows(p),claimed:f.host.claim(p,b)};return after.claimed?'confirmedSend':'definitelyUnsent';};
 assert.equal(f.emit().outcome,'definitelyUnsent');assert.deepEqual(after,{allows:false,claimed:false});assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.calls.length,1);
 f.hook=null;assert.ok(f.host.edge({type:'slot',slot:1,cursor:null}));assert.equal(f.emit(f.proof({sequence:f.host.run*2**22+2,edgeSequence:0})).outcome,'confirmedSend');assert.equal(f.calls.length,2);
});
test('enabled false synchronously withdraws proof and held work, with valid re-enable and no replay',()=>{const f=fixture();f.hook=(p,b)=>{f.input.facts.enabled=false;f.host.tick();assert.equal(f.ready,false);assert.ok(f.clearWork>0);return f.host.claim(p,b)?'confirmedSend':'definitelyUnsent';};assert.equal(f.emit().outcome,'definitelyUnsent');f.now=60;f.host.tick();assert.equal(f.ready,false);assert.equal(f.emit().outcome,'definitelyUnsent');f.input.facts.enabled=true;f.host.tick();f.hook=null;assert.equal(f.emit(f.proof({sequence:f.host.run*2**22+2,edgeSequence:0})).outcome,'confirmedSend');});
test('actual Page HP overlay callback and combat producer block immediately before React render',()=>{
 const source=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let callback=null,read=null,wired=false;const guardNames=['otherPlayerUiBlocksInput','referenceWindowsBlockGameplay','heroUiBlocksGameplay'],guardDeclarations=new Map();
 const heroPageRefNames=['heroSharedUiIngressRef','heroManagementOpenRef'],heroPageRefInitializers=new Map();
 function visit(n){if(ts.isVariableDeclaration(n)&&ts.isIdentifier(n.name)&&heroPageRefNames.includes(n.name.text)){assert.ok(n.initializer&&ts.isCallExpression(n.initializer)&&n.initializer.expression.getText(ast)==='useRef');heroPageRefInitializers.set(n.name.text,n.initializer.arguments[0].getText(ast));}if(ts.isFunctionDeclaration(n)&&n.name&&guardNames.includes(n.name.text)){assert.equal(guardDeclarations.has(n.name.text),false);guardDeclarations.set(n.name.text,n.getText(ast));}
  if(ts.isVariableDeclaration(n)&&n.name.getText(ast)==='handleHpOrbLocalOverlayChange')callback=n.initializer;
  if(ts.isCallExpression(n)&&n.expression.getText(ast)==='useBevyCombatInput'){const props=n.arguments[0].properties;read=props.find(p=>p.name?.getText(ast)==='read').initializer;}
  if(ts.isJsxAttribute(n)&&n.name.getText(ast)==='onHpOrbLocalOverlayChange')wired=n.initializer?.expression?.getText(ast)==='handleHpOrbLocalOverlayChange';ts.forEachChild(n,visit);}
 visit(ast);assert.ok(callback&&read&&wired);for(const name of guardNames)assert.ok(guardDeclarations.has(name),'actual live modal producer dependency '+name);
 function extracted(node,deps){const js=ts.transpileModule([...guardDeclarations.values()].join('\n')+'\nconst extracted = '+node.getText(ast)+';',{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;return new Function(...Object.keys(deps),js+';return extracted;')(...Object.values(deps));}
 const f=fixture(),overlay={current:false},state=[],events=[],ingress={current:{withdraw:()=>{events.push('withdraw');f.host.withdraw();}}};
 const onOverlay=extracted(callback,{bevyHpLocalOverlayOpenRef:overlay,combatIngressRef:ingress,mailIngressRef:{current:null},mailDispatcherRef:{current:null},setBevyHpLocalOverlayOpen:v=>{events.push('state');state.push(v);}});
 const current={...f.input.facts,playerObjectId:3,mapFileName:'D001',connected:true,entities:f.input.facts.actors,selectedObjectId:null,playerHp:10,playerMaxHp:10,playerMp:10};
 let worldVisible=true,canvasVisible=true,domPresent=false,domLoaded=true;const selectors=[];
 const stage={display:'block',opacity:'1',parentElement:null};
 const composite={get hidden(){return !worldVisible;},parentElement:stage};
 const mapImage={get complete(){return domLoaded;},get naturalWidth(){return domLoaded?48:0;},get naturalHeight(){return domLoaded?32:0;},parentElement:composite,getBoundingClientRect:()=>({width:48,height:32})};
 const deps={worldRef:{current},combatRawRef:{current:{snapshot:{entities:[],mapFileName:'D001'},owner:f.input.owner}},readCurrentCombatBaseline:()=>({snapshot:f.raw,owner:f.input.owner}),currentSpellsOwner:()=>f.input.owner,currentAuthoritativeSelf:()=>current.entities[0],combatMastersRef:{current:new Map()},document:{activeElement:null,visibilityState:'visible',hasFocus:()=>true,querySelectorAll:selector=>{selectors.push(selector);if(selector.includes('img.scene-backdrop-sprite'))return domPresent?[mapImage]:[];return [{hidden:selector.includes('quest-ui-canvas')||(selector==='.game-world-composite'?!worldVisible:!canvasVisible),parentElement:stage,getBoundingClientRect:()=>({width:10,height:10})}];}},HTMLElement:class{},getComputedStyle:surface=>({display:surface.display??'block',visibility:surface.hidden?'hidden':'visible',opacity:surface.opacity??'1'}),webGl2SharedCanvasPrototype:false,bevyHpLocalOverlayOpenRef:overlay,combatIngressRef:{current:{hasUiHeld:()=>f.host.hasUiHeld()}},npcShopService:null,npcRepairService:null,spellsIngressRef:{current:null},combatRouteCacheRef:{current:null},movementBlockedStepsRef:{current:[]},movementPlanRef:{current:null},nextMoveSendAtRef:{current:0},screenRef:{current:'game'},initialSceneAssetsReadyRef:{current:true},equipmentHostSuspendReasonRef:{current:null},combatPointerRef:{current:{hovered:null,cursor:null,lock:false}},crystalKeyBindingsRef:{current:defaultCrystalKeyBindings()},isMovementBusy:()=>false,crystalMovementWalkRoute:()=>{throw Error('unexpected route');}};
 deps.combatForceAttackRef={current:false};
 for(const key of ['bevyQuestReactModalOpen','showQuestLog','showCharacter','showHeroPet','showGuild','showGroup','showFriends','showBonds','showRanking','showMarket','showConquest','showTrade','showBuffs','showMail','showWorldMap','showHelp','showHotkeys','showChatSettings'])deps[key]=false;
 Object.assign(deps,{heroManagementPage:null,cashShopOpenRef:{current:false},heroManagementOpenRef:{current:false},skillBarPointerHeldRef:{current:false},
  playerReferenceWindowsRef:{current:{help:false,hotkeys:false,options:false,capture:false}},
  socialItemWindowsRef:{current:{guild:false,trade:false}},socialReplyWindowsRef:{current:{group:false,bonds:false}},socialRosterWindowsRef:{current:{friends:false}},
  npcShopServiceRef:{current:null},npcRepairServiceRef:{current:null},npcShopUiIngressRef:{current:null},storageUiIngressRef:{current:null}});
 for(const key of ['questReactModalOpenRef','bagOpenRef','characterOpenRef','questLogOpenRef','heroPetOpenRef','rankingWindowRef','marketOpenRef','conquestOpenRef',
  'buffsOpenRef','mailUiOpenRef','worldMapOpenRef','chatSettingsOpenRef','tutorialOpenRef','storageServiceActiveRef','rankingInspectOpenRef'])deps[key]={current:false};
 assert.equal(deps.crystalKeyBindingsRef.current.length,96);assert.deepEqual(deps.crystalKeyBindingsRef.current,nativeKeyboardDefaults);
 for(const name of heroPageRefNames)assert.ok(heroPageRefInitializers.has(name),'actual Page Hero ref initializer '+name);
 const heroPageRefsJs=ts.transpileModule([...heroPageRefInitializers].map(([name,initializer])=>'const '+name+'={current:'+initializer+'};').join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 Object.assign(deps,new Function(heroPageRefsJs+'\nreturn {'+heroPageRefNames.join(',')+'};')());
  const readFacts=extracted(read,deps);assert.equal(readFacts().facts.enabled,true);
  assert.equal(readFacts().facts.spellLockKey,nativeKeyboardDefaults.find(binding=>binding.function==='TargetSpellLockOn').key);
  assert.deepEqual(readFacts().facts.bindings,nativeKeyboardDefaults.filter(binding=>/^Bar[12]Skill[1-8]$/.test(binding.function)));
 for(const key of ['heroManagementOpenRef','skillBarPointerHeldRef','questReactModalOpenRef','bagOpenRef','rankingInspectOpenRef']){deps[key].current=true;assert.equal(readFacts().facts.enabled,false,'actual synchronous live fence '+key);deps[key].current=false;assert.equal(readFacts().facts.enabled,true);}
 let heroSharedOwner=true,heroWorldBlocked=true;
 deps.heroSharedUiIngressRef.current={isSharedOwner:()=>heroSharedOwner,blocksWorld:()=>heroWorldBlocked};
 assert.equal(readFacts().facts.enabled,false,'actual shared Hero world fence blocks combat');
 heroWorldBlocked=false;assert.equal(readFacts().facts.enabled,true,'nonblocking shared Hero owner permits combat');
 deps.heroManagementOpenRef.current=true;assert.equal(readFacts().facts.enabled,true,'shared Hero owns the live world fence instead of the React window');
 heroSharedOwner=false;assert.equal(readFacts().facts.enabled,false,'React Hero window resumes its fence when shared ownership ends');
 deps.heroManagementOpenRef.current=false;deps.heroSharedUiIngressRef.current=null;
 const readSharedFacts=extracted(read,{...deps,webGl2SharedCanvasPrototype:true});
 assert.equal(readSharedFacts().facts.enabled,true,'hidden stage UI cannot hide a visible DOM world');
 assert.equal(selectors.at(-1),'.game-world-composite');
 worldVisible=false;assert.equal(readSharedFacts().facts.enabled,false,'hidden DOM world suspends combat');worldVisible=true;
 canvasVisible=false;domPresent=true;
 assert.equal(readFacts().facts.enabled,true,'loaded DOM map allows combat while all GPU canvases are hidden');
 domPresent=false;assert.equal(readFacts().facts.enabled,false,'an empty visible composite cannot replace a painted world');domPresent=true;
 domLoaded=false;assert.equal(readFacts().facts.enabled,false,'unloaded map images cannot enable combat');domLoaded=true;
 worldVisible=false;assert.equal(readFacts().facts.enabled,false,'hidden composite ancestor blocks loaded DOM map');worldVisible=true;
 stage.display='none';assert.equal(readFacts().facts.enabled,false,'hidden stage ancestor blocks loaded DOM map');stage.display='block';
 stage.opacity='0';assert.equal(readFacts().facts.enabled,false,'transparent stage ancestor blocks loaded DOM map');stage.opacity='1';
 deps.initialSceneAssetsReadyRef.current=false;assert.equal(readFacts().facts.enabled,false,'DOM map still requires scene interaction readiness');deps.initialSceneAssetsReadyRef.current=true;
 deps.document.hasFocus=()=>false;assert.equal(readFacts().facts.enabled,false,'DOM compatibility does not bypass page focus');deps.document.hasFocus=()=>true;
 deps.document.visibilityState='hidden';assert.equal(readFacts().facts.enabled,false,'DOM compatibility does not bypass page visibility');deps.document.visibilityState='visible';
 deps.document.activeElement=new deps.HTMLElement();deps.document.activeElement.tagName='INPUT';assert.equal(readFacts().facts.enabled,false,'editing still blocks combat on a DOM map');deps.document.activeElement=null;
 current.mapFileName='D002';assert.equal(readFacts().facts,null,'DOM map still requires the current authoritative map');current.mapFileName='D001';
 assert.equal(readFacts().facts.enabled,true,'DOM map recovers when every existing input fence is valid');
 canvasVisible=true;domPresent=false;
 f.hook=(p,b)=>{assert.ok(f.host.allows(p));onOverlay(true);assert.equal(overlay.current,true);assert.deepEqual(events,['withdraw','state']);assert.equal(readFacts().facts.enabled,false);assert.equal(f.host.allows(p),false);return f.host.claim(p,b)?'confirmedSend':'definitelyUnsent';};
 assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.emit().outcome,'definitelyUnsent');f.now=60;f.input.facts.enabled=readFacts().facts.enabled;f.host.tick();assert.equal(f.ready,false);
 onOverlay(false);assert.equal(readFacts().facts.enabled,true);f.input.facts.enabled=true;f.host.tick();f.hook=null;assert.equal(f.emit(f.proof({sequence:f.host.run*2**22+2,edgeSequence:0})).outcome,'confirmedSend');assert.deepEqual(state,[true,false]);const token={};f.host.setUiHeld('bag',token,true);assert.equal(readFacts().facts.enabled,false);f.host.setUiHeld('bag',token,false);assert.equal(readFacts().facts.enabled,true);
});

test('UI hold withdraws synchronous proof and blocks every 50ms publish; channels and tokens release independently',()=>{const f=fixture(),bag={},hud={};const replay=f.proof();f.hook=(p,b)=>{assert.ok(f.host.setUiHeld('bag',bag,true));assert.ok(f.host.setUiHeld('hud',hud,true));return f.host.claim(p,b)?'confirmedSend':'definitelyUnsent';};assert.equal(f.emit().outcome,'definitelyUnsent');const captures=f.captures.length;f.now=60;f.host.tick();assert.equal(f.ready,false);assert.equal(f.captures.length,captures);assert.equal(f.emit(replay).outcome,'definitelyUnsent');assert.equal(f.host.setUiHeld('bag',{},false),false);assert.ok(f.host.setUiHeld('bag',bag,false));f.now=110;f.host.tick();assert.equal(f.ready,false);assert.ok(f.host.hasUiHeld());assert.ok(f.host.setUiHeld('hud',hud,false));f.host.tick();assert.ok(f.ready);f.hook=null;assert.equal(f.emit(f.proof({sequence:f.host.run*2**22+2,edgeSequence:0})).outcome,'confirmedSend');});
test('UI hold retired owner/runtime and stale release cannot unblock a new token',()=>{const f=fixture(),old={},next={};f.host.setUiHeld('bag',old,true);f.input.owner={...f.input.owner,ownerRevision:1};f.host.observeSnapshot(f.raw,f.input.owner);assert.equal(f.host.hasUiHeld(),false);assert.ok(f.host.setUiHeld('bag',next,true));assert.equal(f.host.setUiHeld('bag',old,false),false);assert.ok(f.host.hasUiHeld());f.host.clearUiHeld();assert.equal(f.host.hasUiHeld(),false);f.host.tick();assert.ok(f.ready);f.host.setUiHeld('bag',next,true);f.host.stop();assert.equal(f.host.hasUiHeld(),false);assert.equal(f.host.setUiHeld('bag',old,true),false);});
function shellRoutingFixture(channel,configure=()=>{}){
 const source=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('shell.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),names=['retireWorldFishingPhysical','cancelWorldFishingHeldPointer','worldFishingStageGeometry','worldFishingPointCurrent','worldFishingPhysicalCurrent','attachWorldFishingPhysical','finishWorldFishingPhysical','npcShopBlocksWorldInput','stopNpcShopWorldInput','cancelSharedNpcShopPointer','handleSharedNpcShopPointer','cancelSharedComposePointer','handleSharedComposePointer','cancelSharedMailPointer','handleSharedMailPointer','beginCombatUiHold','endCombatUiHold','cancelSharedBagPointer','cancelSharedCharacterPointer','cancelSharedHudPointer','cancelSharedSpellsPointer','handleSharedBagPointer','handleSharedCharacterPointer','handleSharedHudPointer','handleSharedSpellsPointer','cancelSharedStoragePointer','handleSharedStoragePointer','handleSharedQuestWorldPointer','handleSharedUiPointer','isSharedBagCompatibilityMouse','handleScenePointerAction','stopHeldScenePointer','repairGeometry','cancelNpcRepairPointer','beginNpcRepairPointer','handleNpcRepairPointer','npcRepairControlTarget','rememberNpcRepairControlClick','fenceNpcRepairClick','changeNpcRepairTarget','confirmNpcRepairTarget','openNpcRepairBagPage','rememberBagBeltClick','fenceBagBeltMouse','cancelBagBeltPointer','refreshBagBeltGeometry','bagBeltCallbacksMatch','beginBagBeltPointer','beginCompatBagBeltPointer','finishBagBeltPointer','handleBagBeltPointer','heroBlocksWorldInput','cancelSharedHeroPointer','handleSharedHeroPointer'],decl=[];
 function visit(n){if(ts.isFunctionDeclaration(n)&&n.name&&names.includes(n.name.text))decl.push(n.getText(ast));ts.forEachChild(n,visit);}visit(ast);assert.equal(decl.length,names.length);
 const heroRefNames=['heroPointerLeaseRef','heroHoverContextRef','heroPointerCallbacksRef'],heroRefInitializers=new Map();
 const repairRefNames=['npcRepairTargetRef','npcRepairQuarantineRef','npcRepairClickFenceRef','npcRepairPointerRef','npcRepairVisibleTargetRef'],repairRefInitializers=new Map();let repairRegistration=null;
 // The current Shell world-input paths call real Fishing cancellation. Extract
 // its closed physical-pointer declarations and actual dormant useRef initializers.
 const fishingRefNames=['worldFishingPhysicalRef','worldFishingTerminalRef','worldFishingPointersRef','worldFishingQuarantineRef','worldFishingShellRef'],fishingRefInitializers=new Map();
 const bagBeltRefNames=['bagBeltButtonsRef','bagBeltGeometryRef','bagBeltGeometryRevisionRef','bagBeltCallbacksRef','bagBeltContextRef','bagBeltPointerRef','bagBeltArmedSharedRef','bagBeltQuarantineRef','bagBeltRejectedTerminalRef','bagBeltClickFenceRef'],bagBeltRefInitializers=new Map();let bagBeltRectInitializer;
 function visitRepairBindings(n){
  if(ts.isVariableDeclaration(n)&&ts.isIdentifier(n.name)){
   if(heroRefNames.includes(n.name.text)){assert.ok(n.initializer&&ts.isCallExpression(n.initializer)&&n.initializer.expression.getText(ast)==='useRef');heroRefInitializers.set(n.name.text,n.initializer.arguments[0].getText(ast));}
   if(fishingRefNames.includes(n.name.text)){assert.ok(n.initializer&&ts.isCallExpression(n.initializer)&&n.initializer.expression.getText(ast)==='useRef');fishingRefInitializers.set(n.name.text,n.initializer.arguments[0].getText(ast));}
   if(repairRefNames.includes(n.name.text)){assert.ok(n.initializer&&ts.isCallExpression(n.initializer)&&n.initializer.expression.getText(ast)==='useRef');repairRefInitializers.set(n.name.text,n.initializer.arguments[0].getText(ast));}
   if(bagBeltRefNames.includes(n.name.text)){assert.ok(n.initializer&&ts.isCallExpression(n.initializer)&&n.initializer.expression.getText(ast)==='useRef');bagBeltRefInitializers.set(n.name.text,n.initializer.arguments[0].getText(ast));}
   if(n.name.text==='bagBeltRect'){assert.ok(n.initializer&&ts.isArrowFunction(n.initializer));bagBeltRectInitializer=n.initializer.getText(ast);}
   if(n.name.text==='registerNpcRepairTarget'){assert.ok(n.initializer&&ts.isCallExpression(n.initializer)&&ts.isArrowFunction(n.initializer.arguments[0]));repairRegistration=n.initializer.arguments[0].getText(ast);}
  }
  ts.forEachChild(n,visitRepairBindings);
 }
 visitRepairBindings(ast);for(const name of heroRefNames)assert.ok(heroRefInitializers.has(name),'actual Shell Hero ref initializer '+name);for(const name of repairRefNames)assert.ok(repairRefInitializers.has(name),'actual Shell repair ref initializer '+name);assert.ok(repairRegistration,'actual Shell repair target registration');
 for(const name of fishingRefNames)assert.ok(fishingRefInitializers.has(name),'actual Shell Fishing inactive ref initializer '+name);
 for(const name of bagBeltRefNames)assert.ok(bagBeltRefInitializers.has(name),'actual Shell Bag/Belt inactive ref initializer '+name);assert.ok(bagBeltRectInitializer,'actual Shell Bag/Belt rectangle reader');

 const f=fixture(),context={runGeneration:1,connectionGeneration:1,sessionGeneration:2,ownerRevision:0,requestRun:1,playerObjectId:3,ledgerRunGeneration:1,hudGeneration:1,modelRevision:1,presentationRevision:1,renderRevision:1,presentation:{touch:false},modal:false,inputRegions:[{left:100,top:100,width:30,height:30}]};
 class Element{constructor(){this.id='canvas';}setPointerCapture(){}closest(){return null;}}
 const scope={parityUiBlocksGameplay:undefined,onHeroShortcut:undefined,npcShopPointerRouterRef:{current:new(load('bevy-npc-shop-ui').NpcShopPointerRouter)()},npcShopPointerCallbacksRef:{current:{getBevyNpcShopInputBlocked:()=>false,getBevyNpcShopPointerContext:()=>null}},heldKeyboardMoveKeysRef:{current:new Set()},heldKeyboardRunModeRef:{current:false},onCombatPointer:()=>{},bevyStorageUiActive:false,bevyStorageUiTransitioning:false,storagePointerRouterRef:{current:new(load('bevy-storage-ui').StoragePointerRouter)()},storagePointerCallbacksRef:{current:{getBevyStoragePointerContext:()=>null}},bevyMailPageReady:false,bevyMailTextContext:null,bevyMailComposeReady:false,bevyMailComposePending:false,heldComposePointerRef:{current:null},mailComposePointerCallbacksRef:{current:{}},document:{getElementById:()=>null},mailPointerRouterRef:{current:new(load('bevy-mail-ui').MailPointerRouter)()},mailPointerCallbacksRef:{current:{getBevyMailPointerContext:()=>null}},onCombatUiHeld:(c,t,h)=>f.host.setUiHeld(c,t,h),combatUiHoldRef:{current:new Map()},bagPointerRouterRef:{current:new(load('bevy-bag-ui').BagPointerRouter)()},characterPointerRouterRef:{current:new(load('bevy-character-ui').CharacterPointerRouter)()},hudPointerRouterRef:{current:new(load('bevy-hud-ui').HudPointerRouter)()},spellsPointerRouterRef:{current:new(load('bevy-spells-ui').SpellsPointerRouter)()},
 bagPointerCallbacksRef:{current:{getBevyBagPointerContext:()=>channel==='bag'?context:null,onBevyBagPointer:()=>true}},characterPointerCallbacksRef:{current:{getBevyCharacterPointerContext:()=>channel==='character'?context:null,onBevyCharacterPointer:()=>true}},spellsPointerCallbacksRef:{current:{getBevySpellsPointerContext:()=>channel==='spells'?context:null,onBevySpellsPointer:()=>true}},
 hudStatus:channel==='hud'?{ready:true,generation:1,revision:1,characterStatsReady:false,modal:false,foregroundRects:[],plan:{main:{left:100,top:100,width:30,height:30},characterRect:{left:500,top:500,width:30,height:30},characterHits:[],buttons:[]}}:null,readBevyHudStatus:()=>scope.hudStatus,
 stageFrameRef:{current:{dataset:{viewportSceneWidth:'1024',viewportSceneHeight:'768'},focus(){}}},heldScenePointerRef:{current:null},heldQuestControlPointersRef:{current:new Set()},screen:'game',player:{objectId:'3'},questControls:null,questBlockers:null,readBevyQuestWorldControls:()=>scope.questControls,readBevyQuestWorldControlBlockers:()=>scope.questBlockers,questWorldControlAt:load('bevy-quest-world-controls').questWorldControlAt,viewportStops:0,worldDispatches:0,onViewportDirectionStop:()=>{scope.viewportStops++;},dispatchSceneClickInput:()=>{scope.worldDispatches++;},sceneInteractionReady:true,questLocalModalOpen:false,mobileMoreOpen:false,bevyQuestUiCapturesPointer:false,bevyBagUiActive:channel==='bag',bevyCharacterPageReady:channel==='character',bevySpellsPageReady:channel==='spells',bevyHudUiReady:channel==='hud',HTMLElement:Element,sharedUiCanvasId:()=> 'canvas',webGl2SharedCanvasPrototype:false,scenePointFromMouseEvent:e=>({sceneX:e.clientX,sceneY:e.clientY}),dispatchBevyHudNavigation:()=>{},window:{dispatchEvent(){}},CustomEvent:class{}};

 scope.npcRepairService=null;scope.npcRepairView=null;scope.activeInventoryTab='bag1';scope.showInventory=channel==='bag';
 scope.stagePresentation={virtualWidth:1024,virtualHeight:768,scale:1};scope.Element=Element;scope.Node=Element;
 scope.onBeginNpcRepairDrag=undefined;scope.onCancelNpcRepairDrag=undefined;scope.onDropNpcRepairDrag=undefined;scope.onConfirmNpcRepair=undefined;
 scope.setNpcRepairTargetSelection=selection=>{scope.npcRepairTargetSelection=selection;};scope.onOpenInventoryTab=tab=>{scope.activeInventoryTab=tab;};
 scope.getBevyHeroInputBlocked=undefined;scope.getBevyHeroPointerContext=undefined;scope.onBevyHeroPointer=undefined;scope.onBevyHeroKey=undefined;
 const heroRefsJs=ts.transpileModule([...heroRefInitializers].map(([name,initializer])=>'const '+name+'={current:'+initializer+'};').join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 Object.assign(scope,new Function(...['getBevyHeroInputBlocked','getBevyHeroPointerContext','onBevyHeroPointer','onBevyHeroKey'],heroRefsJs+'\nreturn {'+heroRefNames.join(',')+'};')(...['getBevyHeroInputBlocked','getBevyHeroPointerContext','onBevyHeroPointer','onBevyHeroKey'].map(name=>scope[name])));
 assert.equal(scope.heroPointerLeaseRef.current,null);assert.equal(scope.heroHoverContextRef.current,null);
 const fishingRefsJs=ts.transpileModule([...fishingRefInitializers].map(([name,initializer])=>'const '+name+'={current:'+initializer+'};').join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 Object.assign(scope,new Function(fishingRefsJs+'\nreturn {'+fishingRefNames.join(',')+'};')());
 assert.equal(scope.worldFishingPhysicalRef.current,null);assert.equal(scope.worldFishingTerminalRef.current,null);
 assert.equal(scope.worldFishingPointersRef.current.size,0);assert.equal(scope.worldFishingQuarantineRef.current.size,0);
 assert.equal(scope.worldFishingShellRef.current,null);
 const repairRefsJs=ts.transpileModule([...repairRefInitializers].map(([name,initializer])=>'const '+name+'={current:'+initializer+'};').join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 Object.assign(scope,new Function(repairRefsJs+'\nreturn {'+repairRefNames.join(',')+'};')());
 const bagBeltRefsJs=ts.transpileModule([...bagBeltRefInitializers].map(([name,initializer])=>'const '+name+'={current:'+initializer+'};').join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 Object.assign(scope,new Function(bagBeltRefsJs+'\nreturn {'+bagBeltRefNames.join(',')+'};')(),load('bag-belt-gesture'));
 assert.equal(scope.bagBeltPointerRef.current,null);assert.equal(scope.bagBeltArmedSharedRef.current,null);
 assert.equal(scope.bagBeltGeometryRef.current,null);assert.equal(scope.bagBeltContextRef.current,null);
 let lifecycle=null;
 function findLifecycle(n){if(ts.isCallExpression(n)&&n.expression.getText(ast)==='useEffect'&&n.arguments[0]?.getText(ast).includes('heldQuestControlPointersRef.current.clear()')&&n.arguments[0]?.getText(ast).includes('window.addEventListener("pointerup"'))lifecycle=n.arguments[0];ts.forEachChild(n,findLifecycle);}findLifecycle(ast);assert.ok(lifecycle,'extract the actual Shell window terminal/cleanup effect');
 const listeners=new Map(),documentListeners=new Map();scope.window={...scope.window,devicePixelRatio:1,addEventListener:(name,fn)=>listeners.set(name,fn),removeEventListener:(name,fn)=>{assert.equal(listeners.get(name),fn);listeners.delete(name);}};
 scope.document={...scope.document,visibilityState:"visible",hasFocus:()=>true,addEventListener:(name,fn)=>documentListeners.set(name,fn),removeEventListener:(name,fn)=>{assert.equal(documentListeners.get(name),fn);documentListeners.delete(name);}};
 scope.sharedBagPointerHandlerRef={current:null};configure(scope,context);
 const code=ts.transpileModule(decl.join('\n')+'\nconst bagBeltRect = '+bagBeltRectInitializer+';\nconst registerNpcRepairTarget = '+repairRegistration+';\nconst mountPointerLifecycle = '+lifecycle.getText(ast)+';',{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText,api=new Function(...Object.keys(scope),code+';return {'+names.join(',')+',mountPointerLifecycle};')(...Object.values(scope));
 scope.sharedBagPointerHandlerRef.current=api.handleSharedUiPointer;
 return{f,scope,api,listeners,event:(x=110,y=110,id=7)=>({target:new Element(),currentTarget:scope.stageFrameRef.current,pointerId:id,pointerType:'mouse',isPrimary:true,button:0,timeStamp:10,detail:1,clientX:x,clientY:y,prevented:false,preventDefault(){this.prevented=true;},stopPropagation(){},stopImmediatePropagation(){}})};
}
function bagBeltRoutingFixture(){
 const wire=[],begun=[],arms=[],canceled=[],captured=new Set(),flags={arm:true,occluded:false,rejectUp:false};
 const stageRect={left:160,top:20,width:512,height:384},scale=0.5;let canvas,buttons,context;
 const routing=shellRoutingFixture('bag',(scope,bagContext)=>{
  context=bagContext;context.inputRegions=[{left:100,top:100,width:140,height:60}];
  scope.stagePresentation.scale=scale;
  canvas=new scope.HTMLElement();Object.assign(canvas,{isConnected:true,getBoundingClientRect:()=>({...stageRect}),
   setPointerCapture:id=>captured.add(id),hasPointerCapture:id=>captured.has(id),releasePointerCapture:id=>captured.delete(id)});
  buttons=Array.from({length:6},(_,slot)=>{const button=new scope.HTMLElement();
   button.rect={left:stageRect.left+(50+slot*40)*scale,top:stageRect.top+650*scale,width:32*scale,height:32*scale};
   Object.assign(button,{isConnected:true,disabled:false,hidden:false,getBoundingClientRect:()=>({...button.rect}),contains:node=>node===button});return button;});
  Object.assign(scope.stageFrameRef.current,{isConnected:true,getBoundingClientRect:()=>({...stageRect}),contains:node=>node===canvas||buttons.includes(node)});
  scope.scenePointFromMouseEvent=e=>({sceneX:(e.clientX-stageRect.left)/scale,sceneY:(e.clientY-stageRect.top)/scale});
  scope.bagBeltContextRef.current={page:'bag1',open:true,screen:'game',repair:false,virtualWidth:1024,virtualHeight:768,scale};
  scope.bagBeltButtonsRef.current=buttons.map((node,slot)=>({node,slot,item:null}));
  scope.bagBeltCallbacksRef.current={onBagBeltDropGeometry:geometry=>{context.beltDropGeometry=geometry;},
   onBeginBagToBeltGesture:(source,geometry,owner)=>{assert.equal(source,null);assert.equal(owner,'bevy');assert.equal(geometry,context.beltDropGeometry);
    const proof=Object.freeze({token:Object.freeze({})});begun.push(proof);return proof;},
   onArmBagToBeltGesture:(proof,geometry,target,point)=>{arms.push({proof,geometry,target,point});return flags.arm;},
   onCancelBagToBeltGesture:proof=>canceled.push(proof)};
  scope.document.elementFromPoint=(x,y)=>flags.occluded?canvas:buttons.find(b=>x>=b.rect.left&&x<b.rect.left+b.rect.width&&y>=b.rect.top&&y<b.rect.top+b.rect.height)??canvas;
  scope.window.getComputedStyle=()=>({display:'block',visibility:'visible',pointerEvents:'auto',opacity:'1'});
  // Bind the real DOM-aware endpoint functions to this fixture's browser;
  // cached module functions otherwise close over Node's missing DOM globals.
  const module={exports:{}};
  new Function('exports','module','document','window',ts.transpileModule(readFileSync(new URL('../lib/bag-belt-gesture.ts',import.meta.url),'utf8'),
   {compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(module.exports,module,scope.document,scope.window);
  Object.assign(scope,module.exports);
  scope.bagPointerCallbacksRef.current.onBevyBagPointer=edge=>{
   assert.deepEqual(Object.keys(edge).sort(),['runGeneration','connectionGeneration','sessionGeneration','ownerRevision','presentationRevision','sequence','pointerId','phase','origin','x','y','button'].sort());
   wire.push({...edge});return edge.phase!=='up'||!flags.rejectUp;};
 });
 const geometry=routing.api.refreshBagBeltGeometry();assert.ok(geometry);assert.equal(context.beltDropGeometry,geometry);
 return{...routing,wire,begun,arms,canceled,captured,flags,buttons,context,
  event:(x=110,y=110,id=7,timeStamp=10)=>({...routing.event(stageRect.left+x*scale,stageRect.top+y*scale,id),target:canvas,timeStamp})};
}
test('actual Shell Belt-capable same-Bag drag retains original Up and retires only its unused Belt proof',()=>{
 const x=bagBeltRoutingFixture(),cleanup=x.api.mountPointerLifecycle();x.api.handleSharedUiPointer(x.event(),'down');
 assert.equal(x.scope.bagBeltPointerRef.current.proof,x.begun[0],'real Bag down acquired the Belt lease');assert.ok(x.f.host.hasUiHeld());assert.ok(x.captured.has(7));
 x.api.handleSharedUiPointer(x.event(200,110,7,15),'move');x.listeners.get('pointerup')(x.event(200,110,7,20));
 assert.deepEqual(x.wire.map(e=>e.phase),['down','move','up']);assert.deepEqual([x.wire[2].x,x.wire[2].y],[200,110]);
 assert.ok(x.wire.every(e=>e.origin==='bag'&&e.ownerRevision===x.context.ownerRevision));assert.ok(x.wire[0].sequence<x.wire[1].sequence&&x.wire[1].sequence<x.wire[2].sequence);
 assert.equal(x.arms.length,0);assert.deepEqual(x.canceled,[x.begun[0]]);assert.equal(x.scope.bagBeltArmedSharedRef.current,null);
 assert.equal(x.scope.bagBeltRejectedTerminalRef.current,null);assert.equal(x.scope.bagPointerRouterRef.current.held,null);assert.equal(x.f.host.hasUiHeld(),false);assert.equal(x.captured.size,0);
 x.api.handleSharedUiPointer(x.event(200,110,7,20),'up');assert.equal(x.wire.length,3);assert.equal(x.canceled.length,1);
 x.api.handleSharedUiPointer(x.event(110,110,7,30),'down');const held=x.scope.bagPointerRouterRef.current.held,hold=x.scope.combatUiHoldRef.current.get('bag');
 x.api.handleSharedUiPointer(x.event(200,110,7,20),'up');assert.equal(x.scope.bagPointerRouterRef.current.held,held);assert.equal(x.scope.combatUiHoldRef.current.get('bag'),hold);
 x.api.handleSharedUiPointer(x.event(200,110,7,40),'up');assert.equal(x.wire.at(-1).phase,'up');assert.equal(x.f.host.hasUiHeld(),false);assert.deepEqual(x.canceled,x.begun);
 cleanup();assert.equal(x.listeners.size,0);
});
test('actual Shell Belt release preserves an accepted proof and cancels rejected, occluded or disabled endpoints',()=>{
 for(const mode of ['accepted','armRejected','occluded','disabled','wireRejected']){
  const x=bagBeltRoutingFixture();x.api.handleSharedUiPointer(x.event(),'down');assert.equal(x.begun.length,1);
  if(mode==='armRejected')x.flags.arm=false;if(mode==='occluded')x.flags.occluded=true;if(mode==='disabled')x.buttons[2].disabled=true;if(mode==='wireRejected')x.flags.rejectUp=true;
  x.api.handleSharedUiPointer(x.event(146,666,7,15),'move');x.api.handleSharedUiPointer(x.event(146,666,7,20),'up');
  assert.deepEqual(x.wire.map(e=>e.phase),['down','move',['accepted','wireRejected'].includes(mode)?'up':'cancel'],mode);
  assert.equal(x.arms.length,['occluded','disabled'].includes(mode)?0:1,mode);assert.equal(x.scope.bagPointerRouterRef.current.held,null);assert.equal(x.f.host.hasUiHeld(),false);assert.equal(x.captured.size,0);
  if(mode==='accepted'){
   assert.equal(x.scope.bagBeltArmedSharedRef.current.proof,x.begun[0]);assert.equal(x.canceled.length,0);
   assert.deepEqual(x.arms[0].target,{slot:2,uniqueId:null});assert.equal(x.arms[0].geometry,x.context.beltDropGeometry);
   x.api.handleSharedUiPointer(x.event(146,666,7,20),'up');assert.equal(x.arms.length,1);assert.equal(x.wire.length,3);
   x.api.cancelBagBeltPointer(x.scope.bagBeltArmedSharedRef.current);
  }
  assert.equal(x.scope.bagBeltArmedSharedRef.current,null);assert.deepEqual(x.canceled,[x.begun[0]],mode+' never reuses or replaces the proof');
 }
});
test('actual Shell changed Bag owner, Belt geometry, callbacks or physical source cancel an old drag',()=>{
 for(const change of ['owner','geometry','callback','source','pointerType']){
  const x=bagBeltRoutingFixture();x.api.handleSharedUiPointer(x.event(),'down');
  if(change==='owner')x.context.ownerRevision++;if(change==='geometry')x.buttons[0].rect.left++;
  if(change==='callback')x.scope.bagBeltCallbacksRef.current={...x.scope.bagBeltCallbacksRef.current,onArmBagToBeltGesture:()=>{throw Error('replacement callback must not acquire the old proof');}};
  if(change==='source')x.scope.bagBeltPointerRef.current.node.isConnected=false;
  const release=x.event(200,110,7,20);if(change==='pointerType')release.pointerType='touch';x.api.handleSharedUiPointer(release,'up');
  assert.ok(x.wire.some(e=>e.phase==='cancel'),change);assert.equal(x.arms.length,0);assert.deepEqual(x.canceled,[x.begun[0]]);
  assert.equal(x.scope.bagBeltPointerRef.current,null);assert.equal(x.scope.bagBeltArmedSharedRef.current,null);assert.equal(x.scope.bagPointerRouterRef.current.held,null);assert.equal(x.f.host.hasUiHeld(),false);assert.equal(x.captured.size,0);
  if(change==='owner')assert.ok(x.wire.every(e=>e.ownerRevision===0),'retirement cannot rename old edges into the new owner');
  x.api.handleSharedUiPointer(release,'up');assert.equal(x.arms.length,0);assert.equal(x.canceled.length,1);
 }
});
test('actual Shell Belt lease cancel, blur and cleanup do not leak old terminals into a fresh gesture',()=>{
 for(const terminal of ['cancel','blur','cleanup']){
  const x=bagBeltRoutingFixture(),cleanup=x.api.mountPointerLifecycle();x.api.handleSharedUiPointer(x.event(),'down');
  if(terminal==='cancel')x.api.handleSharedUiPointer(x.event(200,110,7,20),'cancel');else if(terminal==='blur')x.listeners.get('blur')();else cleanup();
  assert.ok(x.wire.some(e=>e.phase===(terminal==='cleanup'?'cancel':terminal)));
  assert.equal(x.arms.length,0);assert.deepEqual(x.canceled,[x.begun[0]]);assert.equal(x.scope.bagBeltPointerRef.current,null);assert.equal(x.scope.bagBeltArmedSharedRef.current,null);
  assert.equal(x.scope.bagPointerRouterRef.current.held,null);assert.equal(x.scope.bagBeltRejectedTerminalRef.current,null);assert.equal(x.f.host.hasUiHeld(),false);assert.equal(x.captured.size,0);
  if(terminal==='cleanup'){assert.equal(x.listeners.size,0);continue;}
  const id=terminal==='cancel'?7:8;x.api.handleSharedUiPointer(x.event(110,110,id,30),'down');const held=x.scope.bagPointerRouterRef.current.held,hold=x.scope.combatUiHoldRef.current.get('bag');assert.ok(held);assert.ok(hold);
  x.api.handleSharedUiPointer(x.event(200,110,7,20),'up');assert.equal(x.scope.bagPointerRouterRef.current.held,held);assert.equal(x.scope.combatUiHoldRef.current.get('bag'),hold);
  x.api.handleSharedUiPointer(x.event(200,110,id,40),'up');assert.equal(x.wire.at(-1).phase,'up');assert.equal(x.wire.at(-1).pointerId,id);assert.equal(x.f.host.hasUiHeld(),false);assert.deepEqual(x.canceled,x.begun);
  cleanup();assert.equal(x.listeners.size,0);
 }
});
for(const channel of ['bag','character','hud','spells'])test('actual Shell '+channel+' UI-origin lease blocks timer; terminal and cleanup restore fresh input',()=>{for(const terminal of ['up','cancel','blur']){const {f,api,event}=shellRoutingFixture(channel);api.handleSharedUiPointer(event(),'move');assert.equal(f.host.hasUiHeld(),false,'passive hover never blocks');api.handleSharedUiPointer(event(),'down');assert.ok(f.host.hasUiHeld());const captures=f.captures.length;f.now=60;f.host.tick();assert.equal(f.ready,false);assert.equal(f.captures.length,captures);if(terminal==='blur')api['cancelShared'+channel[0].toUpperCase()+channel.slice(1)+'Pointer']();else api.handleSharedUiPointer(event(),terminal);assert.equal(f.host.hasUiHeld(),false);f.host.tick();assert.ok(f.ready);assert.equal(f.emit(f.proof({edgeSequence:0})).outcome,'confirmedSend');}});
test('actual Shell world-origin Bag/HUD crossing stays world; old token cleanup cannot release new hold',()=>{for(const channel of ['bag','hud']){const {f,api,event,scope}=shellRoutingFixture(channel);api.handleSharedUiPointer(event(50,50),'down');assert.equal(f.host.hasUiHeld(),false);api.handleSharedUiPointer(event(),'move');assert.equal(f.host.hasUiHeld(),false);api.handleSharedUiPointer(event(),'up');assert.equal(f.host.hasUiHeld(),false);api.beginCombatUiHold(channel,7);const old=scope.combatUiHoldRef.current.get(channel);api.beginCombatUiHold(channel,7);api.endCombatUiHold(channel,old);assert.ok(f.host.hasUiHeld());api.endCombatUiHold(channel,scope.combatUiHoldRef.current.get(channel));assert.equal(f.host.hasUiHeld(),false);}});

for(const channel of ['bag','character','spells'])test('actual '+channel+' rejected or throwing terminal releases captured hold after router lease cleared',()=>{for(const terminal of ['up','cancel'])for(const throws of [false,true]){const {f,api,event,scope}=shellRoutingFixture(channel),callbacks=scope[channel+'PointerCallbacksRef'].current;callbacks['onBevy'+channel[0].toUpperCase()+channel.slice(1)+'Pointer']=edge=>{if(edge.phase==='down')return true;if(throws)throw Error('terminal rejected');return false;};api.handleSharedUiPointer(event(),'down');assert.ok(f.host.hasUiHeld());if(throws)assert.throws(()=>api.handleSharedUiPointer(event(),terminal),/terminal rejected/);else api.handleSharedUiPointer(event(),terminal);assert.equal(scope[channel+'PointerRouterRef'].current.held,null);assert.equal(f.host.hasUiHeld(),false);f.now=60;f.host.tick();assert.ok(f.ready);assert.equal(f.emit(f.proof({edgeSequence:0})).outcome,'confirmedSend');}});
for(const channel of ['bag','character','hud','spells'])test('actual '+channel+' no-lease cleanup releases orphan captured token',()=>{const {f,api,scope}=shellRoutingFixture(channel);api.beginCombatUiHold(channel,7);assert.equal(scope[channel+'PointerRouterRef'].current.held,null);assert.ok(f.host.hasUiHeld());api['cancelShared'+channel[0].toUpperCase()+channel.slice(1)+'Pointer']();assert.equal(f.host.hasUiHeld(),false);f.host.tick();assert.ok(f.ready);});
for(const channel of ['bag','character','spells'])test('actual '+channel+' old rejected terminal cannot cancel synchronously new lease or token',()=>{const {f,api,event,scope}=shellRoutingFixture(channel),callbacks=scope[channel+'PointerCallbacksRef'].current;let replaced=false;callbacks['onBevy'+channel[0].toUpperCase()+channel.slice(1)+'Pointer']=edge=>{if(edge.phase==='down')return true;if(edge.phase==='up'&&!replaced){replaced=true;api.handleSharedUiPointer(event(),'down');return false;}return true;};api.handleSharedUiPointer(event(),'down');const old=scope.combatUiHoldRef.current.get(channel);api.handleSharedUiPointer(event(),'up');assert.ok(replaced);assert.ok(f.host.hasUiHeld());assert.ok(scope[channel+'PointerRouterRef'].current.held);assert.notEqual(scope.combatUiHoldRef.current.get(channel).token,old.token);f.now=60;f.host.tick();assert.equal(f.ready,false);api.handleSharedUiPointer(event(),'up');assert.equal(f.host.hasUiHeld(),false);f.host.tick();assert.ok(f.ready);});

test('actual Shell stale painted Quest button swallows one gesture before Bag/HUD world leases and compatibility mouse dispatch',()=>{
 for(const channel of ['bag','hud'])for(const terminal of ['up','cancel']){
  const {f,api,event,scope}=shellRoutingFixture(channel);
  scope.questControls=null;
  scope.questBlockers={version:1,logicalWidth:512,logicalHeight:384,rects:[{type:'attackQuestTarget',left:150,top:150,width:15,height:15}]};
  const down=event(310,310);api.handleSharedUiPointer(down,'down');
  assert.equal(down.prevented,true);assert.ok(scope.heldQuestControlPointersRef.current.has(7));
  assert.equal(scope.bagPointerRouterRef.current.held,null);assert.equal(scope.hudPointerRouterRef.current.held,null);
  assert.equal(scope.heldScenePointerRef.current,null);assert.equal(f.host.hasUiHeld(),false);
  assert.equal(scope.viewportStops,1,'the button stops prior world movement once');
  api.handleScenePointerAction(down);api.handleSharedUiPointer(event(500,500),'move');
  api.handleSharedUiPointer(event(500,500),terminal);api.handleScenePointerAction(event());api.stopHeldScenePointer();
  assert.equal(scope.heldQuestControlPointersRef.current.size,0);assert.equal(scope.viewportStops,1);
  assert.equal(scope.worldDispatches,0,'neither a later mouse compatibility event nor the terminal may dispatch world input');
  api.handleSharedUiPointer(event(40,40,8),'down');
  assert.ok(scope.heldScenePointerRef.current,'blank canvas keeps the existing world-origin route');
  assert.equal(scope.heldQuestControlPointersRef.current.size,0);assert.equal(f.host.hasUiHeld(),false);
  api.handleSharedUiPointer(event(40,40,8),'up');assert.equal(scope.worldDispatches,1);
 }
});

test('actual Shell painted Bag and Character own overlapping stale Quest paint while HUD foreground and modal swallow world gestures',()=>{
 for(const owner of ['bag','character','hudForeground','hudModal','questModal','mobileMore']){
  const {api,event,scope}=shellRoutingFixture(owner==='bag'?'bag':owner==='character'?'character':'hud',(scope)=>{
   scope.questControls=null;
   scope.questBlockers={version:1,logicalWidth:1024,logicalHeight:768,rects:[{type:'attackQuestTarget',left:100,top:100,width:30,height:30}]};
   if(owner==='hudForeground')scope.hudStatus.foregroundRects=[{left:100,top:100,width:30,height:30}];
   if(owner==='hudModal')scope.hudStatus.modal=true;
   if(owner==='questModal')scope.questLocalModalOpen=true;
   if(owner==='mobileMore')scope.mobileMoreOpen=true;
  });
  const down=event();api.handleSharedUiPointer(down,'down');
  assert.equal(scope.heldQuestControlPointersRef.current.size,['hudForeground','hudModal'].includes(owner)?1:0,
   owner+' keeps a world-blocking gesture separate from actual Quest action authority');
  assert.equal(scope.questControls,null,'stale blockers never produce a live Quest controls proof');
  assert.equal(scope.heldScenePointerRef.current,null,owner+' cannot dispatch an overlapping stale button as world input');
  assert.equal(scope.worldDispatches,0);
  if(owner==='bag')assert.equal(scope.bagPointerRouterRef.current.held?.origin,'bag','the actual painted Bag router owns its down edge');
  if(owner==='character')assert.ok(scope.characterPointerRouterRef.current.held,'the actual Character router owns its down edge');
  if(['hudForeground','hudModal'].includes(owner)){
   api.handleSharedUiPointer(event(500,500),'up');
   assert.equal(scope.heldQuestControlPointersRef.current.size,0,'the foreground blocker retains and consumes its old terminal');
   assert.equal(scope.worldDispatches,0);
  }
 }
 const {api,event,scope}=shellRoutingFixture('hud',(scope)=>{
  scope.bevyQuestUiCapturesPointer=true;
  scope.questBlockers={version:1,logicalWidth:1024,logicalHeight:768,rects:[{type:'attackQuestTarget',left:100,top:100,width:30,height:30}]};
  scope.hudStatus.foregroundRects=[{left:100,top:100,width:30,height:30}];
 });
 api.handleSharedUiPointer(event(),'down');
 assert.ok(scope.heldQuestControlPointersRef.current.has(7),'foregroundRects produced by a capturing Quest panel still permit its button gesture');
 assert.equal(scope.hudPointerRouterRef.current.held,null);assert.equal(scope.heldScenePointerRef.current,null);
 api.handleSharedUiPointer(event(),'up');assert.equal(scope.heldQuestControlPointersRef.current.size,0);
});

test('actual Shell captured Quest terminal survives newly appearing NPC owner without cancelling that new owner',()=>{
 for(const terminal of ['up','cancel']){
  let npcContext;
  const {api,event,scope}=shellRoutingFixture('bag',(scope,context)=>{
   npcContext={proof:{runGeneration:context.runGeneration,connectionGeneration:context.connectionGeneration,
    sessionGeneration:context.sessionGeneration,ownerRevision:context.ownerRevision,playerObjectId:context.playerObjectId,
    revision:context.renderRevision,modelRevision:context.modelRevision,presentationRevision:context.presentationRevision,
    serviceRevision:1,catalogRevision:1,coreAuthorityRevision:'1',controlRevision:'1'},
    presentation:context.presentation,inputRegions:context.inputRegions};
   scope.npcShopPointerCallbacksRef.current.getBevyNpcShopPointerContext=()=>npcContext;
  });
  scope.questBlockers={version:1,logicalWidth:1024,logicalHeight:768,rects:[{type:'attackQuestTarget',left:300,top:300,width:30,height:30}]};
  api.handleSharedUiPointer(event(310,310),'down');assert.ok(scope.heldQuestControlPointersRef.current.has(7));
  let npcReads=0;scope.npcShopPointerCallbacksRef.current.getBevyNpcShopInputBlocked=()=>{npcReads++;return true;};
  const newOwner=scope.npcShopPointerRouterRef.current.down(npcContext,9,0,110,110);
  assert.ok(newOwner,'the actual NPC router establishes the new lease');assert.equal(newOwner.origin,'shop');
  assert.equal(scope.npcShopPointerRouterRef.current.held,newOwner);
  scope.questBlockers=null;
  const release=event(500,500);api.handleSharedUiPointer(release,terminal);
  assert.equal(release.prevented,true);assert.equal(scope.heldQuestControlPointersRef.current.size,0);
  assert.equal(npcReads,0,'the old captured terminal is routed before consulting a newly appearing owner');
  assert.equal(scope.npcShopPointerRouterRef.current.held,newOwner,'the old terminal cannot cancel a new NPC lease');
  assert.equal(scope.viewportStops,1);assert.equal(scope.worldDispatches,0);assert.equal(scope.heldScenePointerRef.current,null);
 }
});

test('actual Shell window blur, resize and cleanup clear captured Quest gestures through the real effect',()=>{
 for(const terminal of ['blur','resize','cleanup']){
  const {api,event,scope,listeners}=shellRoutingFixture('bag');
  scope.questBlockers={version:1,logicalWidth:1024,logicalHeight:768,rects:[{type:'attackQuestTarget',left:300,top:300,width:30,height:30}]};
  const cleanup=api.mountPointerLifecycle();
  api.handleSharedUiPointer(event(310,310),'down');assert.ok(scope.heldQuestControlPointersRef.current.has(7));
  if(terminal==='cleanup')cleanup();else listeners.get(terminal)();
  assert.equal(scope.heldQuestControlPointersRef.current.size,0,terminal+' releases the captured Quest pointer');
  assert.equal(scope.worldDispatches,0);assert.equal(scope.heldScenePointerRef.current,null);
  if(terminal!=='cleanup')cleanup();
  assert.equal(listeners.size,0,'the real cleanup removes the registered window listeners');
 }
});

function sharedCanvasWorldTargetFixture(){
 const source=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8');
 const ast=ts.createSourceFile('shell.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 const names=['sceneWorldObjectTarget','retainSceneWorldObjectTarget','scenePointFromMouseEvent','handleSharedHudPointer','cancelSharedHudPointer','beginCombatUiHold','endCombatUiHold','dispatchSceneClickInput','dispatchSceneMoveInput','stopHeldScenePointer','handleSceneWorldPointerDown'];
 const declarations=[];let capture,tick;
 function visit(n){
  if(ts.isFunctionDeclaration(n)&&n.name&&names.includes(n.name.text))declarations.push(n.getText(ast));
  if(ts.isJsxAttribute(n)&&n.name.getText(ast)==='onPointerDownCapture'&&n.initializer&&ts.isJsxExpression(n.initializer)&&n.initializer.expression?.getText(ast).includes('retainSceneWorldObjectTarget'))capture=n.initializer.expression.getText(ast);
  if(ts.isCallExpression(n)&&n.expression.getText(ast)==='window.setInterval'&&n.arguments[0]?.getText(ast).includes('dispatchSceneMoveInput(held)'))tick=n.arguments[0].getText(ast);
  ts.forEachChild(n,visit);
 }
 visit(ast);assert.equal(declarations.length,names.length);assert.ok(capture);assert.ok(tick);
 class DomElement{
  constructor(classes='',dataset={},parent=null,id=''){this.classes=new Set(classes.split(' ').filter(Boolean));this.dataset=dataset;this.parent=parent;this.id=id;this.classList={contains:x=>this.classes.has(x)};}
  closest(selector){for(let node=this;node;node=node.parent){if(selector.split(',').some(part=>[...part.matchAll(/\.([\w-]+)/g)].every(m=>node.classes.has(m[1]))&&[...part.matchAll(/\[data-([\w-]+)(?:=['"]([^'"]*)['"])?\]/g)].every(m=>{const key=m[1].replace(/-([a-z])/g,(_,c)=>c.toUpperCase());return Object.hasOwn(node.dataset,key)&&(m[2]===undefined||node.dataset[key]===m[2]);})))return node;}return null;}
  contains(target){for(let node=target;node;node=node.parent)if(node===this)return true;return false;}
  focus(){}setPointerCapture(){}
  getBoundingClientRect(){return{left:160,top:0,width:960,height:720};}
 }
 const frame=new DomElement('client-stage-frame'),canvas=new DomElement('',{},frame,'mir2-quest-ui-canvas');
 const name=new DomElement('entity-nameplate',{objectId:'206644',uiInteractive:'true'},frame);
 const label=new DomElement('',{},name),sprite=new DomElement('entity-sprite-stack',{objectId:'206644',uiInteractive:'true'},frame),body=new DomElement('entity-sprite-hit',{},sprite);
 const drop=new DomElement('ground-drop-marker',{objectId:'300001'},frame),calls=[],hitReads=[];
 const scope={stageFrameRef:{current:frame},stagePresentation:{virtualWidth:1024,virtualHeight:768},webGl2SharedCanvasPrototype:false,sharedUiCanvasId:()=>canvas.id,
  document:{elementsFromPoint:(x,y)=>{hitReads.push([x,y]);return scope.layers;}},layers:[canvas,label,name,body,frame],HTMLElement:DomElement,
  hudPointerRouterRef:{current:new (load('bevy-hud-ui').HudPointerRouter)()},bevyHudUiReady:true,sceneInteractionReady:true,bagPointerRouterRef:{current:{held:null}},
  characterPointerCallbacksRef:{current:{}},spellsPointerCallbacksRef:{current:{}},bagPointerCallbacksRef:{current:{}},questLocalModalOpen:false,mobileMoreOpen:false,bevyQuestUiCapturesPointer:false,
  status:{ready:true,generation:1,revision:1,characterStatsReady:false,modal:false,foregroundRects:[],plan:{main:{left:0,top:672,width:1024,height:96},buttons:[]}},
  readBevyHudStatus:()=>scope.status,heldScenePointerRef:{current:null},combatUiHoldRef:{current:new Map()},onCombatUiHeld:undefined,
  cancelWorldFishingHeldPointer:()=>{},onViewportDirectionStop:()=>{},dispatchBevyHudNavigation:action=>calls.push(['hud',action]),
  latestMoveInputRef:{current:{screen:'game'}},tileFromScenePoint:()=>({x:99,y:88}),npcShopBlocksWorldInput:()=>scope.shopBlocked??false,
  stopNpcShopWorldInput:()=>{scope.heldScenePointerRef.current=null;},playerUiPreferences:{newMove:false},
  onViewportTileClick:(x,y)=>calls.push(['tile',x,y]),onViewportTileSecondaryAction:(x,y)=>calls.push(['secondary',x,y]),onViewportDirectionStep:(x,y,mode)=>calls.push(['step',x,y,mode]),
  onActivateEntity:id=>calls.push(['entity',id]),onPickGroundDrop:id=>calls.push(['drop',id]),worldFishingPhysicalRef:{current:null},worldFishingPointersRef:{current:new Map()},
  handleSharedUiPointer:(event,phase)=>api.handleSharedHudPointer(event,phase)};
 const js=ts.transpileModule(declarations.join('\n')+'\nconst capture='+capture+';const tick='+tick+';\nreturn {'+names.join(',')+',capture,tick};',{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const api=new Function('scope','with(scope){'+js+'}')(scope);
 const event=(x=632.5,y=348.75)=>({target:canvas,clientX:x,clientY:y,pointerId:7,pointerType:'mouse',button:0,timeStamp:1,preventDefault(){}});
 return{api,event,scope,calls,hitReads,frame,canvas,name,label,body,drop,DomElement};
}

test('actual shared HUD canvas preserves nameplate and body identity without walking to their offset tile',()=>{
 for(const kind of ['name','body']){
  const f=sharedCanvasWorldTargetFixture();if(kind==='body')f.scope.layers=[f.canvas,f.body,f.frame];
  const down=f.event();f.api.capture(down);const held=f.scope.heldScenePointerRef.current;
  assert.deepEqual(held.worldTarget,{kind:'entity',objectId:'206644'});assert.deepEqual(f.hitReads,[[632.5,348.75]]);
  f.api.tick();f.api.dispatchSceneMoveInput(held);assert.equal(held.dispatched,false);assert.deepEqual(f.calls,[],'holding an entity never queues offset-tile movement');
  f.scope.layers=[f.canvas,f.drop,f.frame];f.api.handleSharedHudPointer(f.event(),'up');
  assert.deepEqual(f.calls,[['entity','206644']],'the down target survives crossing to another object before release');
  assert.equal(f.scope.heldScenePointerRef.current,null);assert.equal(f.scope.hudPointerRouterRef.current.held,null);
 }
});

test('actual shared canvas routes drop labels to pickup and empty world to original tile action',()=>{
 const f=sharedCanvasWorldTargetFixture();f.scope.layers=[f.canvas,f.drop,f.frame];f.api.capture(f.event());f.api.handleSharedHudPointer(f.event(),'up');
 assert.deepEqual(f.calls,[['drop','300001']]);f.calls.length=0;f.scope.layers=[f.canvas,f.frame];f.api.capture(f.event());f.api.handleSharedHudPointer(f.event(),'up');
 assert.deepEqual(f.calls,[['tile',99,88]]);
});

test('actual surface router refuses world-target lookup under HUD, modal and foreground owners',()=>{
 for(const blocked of ['hud','modal','foreground']){
  const f=sharedCanvasWorldTargetFixture();let down=f.event();
  if(blocked==='hud'){down=f.event(632.5,680);f.scope.status.plan.buttons=[{action:'character',rect:{left:480,top:672,width:100,height:96}}];}
  if(blocked==='modal')f.scope.status.modal=true;
  if(blocked==='foreground')f.scope.status.foregroundRects=[{left:0,top:0,width:1024,height:672}];
  f.api.capture(down);assert.equal(f.scope.heldScenePointerRef.current,null);assert.deepEqual(f.hitReads,[]);f.api.handleSharedHudPointer(down,'up');
  assert.equal(f.calls.some(([kind])=>kind==='entity'||kind==='drop'||kind==='tile'),false);
 }
});

test('actual target resolver rejects foreign stage, DOM UI and noninteractive objects; cancellation retires the target',()=>{
 const f=sharedCanvasWorldTargetFixture();
 f.scope.layers=[f.canvas,new f.DomElement('entity-nameplate',{objectId:'other',uiInteractive:'true'},new f.DomElement()),f.frame];assert.equal(f.api.sceneWorldObjectTarget(10,20),undefined);
 f.scope.layers=[f.canvas,new f.DomElement('game-ui-scene',{},f.frame),f.name,f.frame];assert.equal(f.api.sceneWorldObjectTarget(10,20),undefined);
 f.name.dataset.uiInteractive='false';f.scope.layers=[f.canvas,f.name,f.frame];assert.equal(f.api.sceneWorldObjectTarget(10,20),undefined);f.name.dataset.uiInteractive='true';
 f.scope.layers=[f.label,f.name,f.frame];assert.equal(f.api.sceneWorldObjectTarget(10,20),undefined,'ordinary DOM target uses its existing handler');
 f.scope.layers=[f.canvas,f.name,f.frame];f.api.capture(f.event());f.api.handleSharedHudPointer(f.event(),'cancel');f.api.handleSharedHudPointer(f.event(),'up');
 assert.deepEqual(f.calls,[]);assert.equal(f.scope.heldScenePointerRef.current,null);
});
