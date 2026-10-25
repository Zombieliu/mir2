import assert from 'node:assert/strict';
import test from 'node:test';
import {readFileSync} from 'node:fs';
import ts from 'typescript';

const source=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8');
const ast=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const functions=new Map();let shiftEffect,combatOptions,modeHostInitializer,gatewayGuard,attackModeCase;
const attributes=new Map();
function visit(node){
 if(ts.isFunctionDeclaration(node)&&node.name)functions.set(node.name.text,node);
 if(ts.isCallExpression(node)&&node.expression.getText(ast)==='useEffect'&&node.arguments[0]?.getText(ast).includes('observeCombatShiftDown'))shiftEffect=node.arguments[0];
 if(ts.isCallExpression(node)&&node.expression.getText(ast)==='useBevyCombatInput')combatOptions=node.arguments[0];
 if(ts.isNewExpression(node)&&node.expression.getText(ast)==='SharedCombatModeKeys')modeHostInitializer=node;
 if(ts.isCaseClause(node)&&node.expression.getText(ast)==='"ChangeAMode"')attackModeCase=node;
 if(ts.isJsxAttribute(node)&&['combatModes','onCombatKey','onCombatCancel'].includes(node.name.getText(ast)))attributes.set(node.name.getText(ast),node.initializer.expression);
 ts.forEachChild(node,visit);
}
visit(ast);
gatewayGuard=functions.get('handleGatewayEvent').body.statements[0];
assert.ok(shiftEffect&&combatOptions&&modeHostInitializer&&attackModeCase);
const option=name=>combatOptions.properties.find(property=>property.name?.getText(ast)===name).initializer.getText(ast);
const nativeKeyboard=JSON.parse(readFileSync(new URL('../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json',import.meta.url),'utf8'));
const cache=new Map();
function load(name){
 if(cache.has(name))return cache.get(name);
 const module={exports:{}};cache.set(name,module.exports);
 new Function('exports','module','require',ts.transpileModule(readFileSync(new URL(`../lib/${name}.ts`,import.meta.url),'utf8'),
  {compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(module.exports,module,
  path=>path==='../../game-client/client-bevy/src/crystal_ui/keyboard_defaults.json'?{default:nativeKeyboard}:load(path.replace('./','')));
 return module.exports;
}
const {supportsForcedPlayerAttack}=load('bevy-combat-input');
const {SharedCombatModeKeys,nextCombatModePhysicalGeneration}=load('shared-combat-mode-keys');
const {defaultCrystalKeyBindings,displayCrystalKeyBinding}=load('player-ui-preferences');
const names=['setCombatForceAttack','cancelCombatInput','cancelLockedMonsterAttack','lockMonsterAttack','activateEntity',
 'selectAttackMode','readCombatModeControls','currentCombatModeOwner','sameCombatModeOwner','readCombatModeSource',
 'nextCombatModeRevision','captureCombatModeSnapshot','captureCombatModeReceipt'];
const code=ts.transpileModule(names.map(name=>functions.get(name).getText(ast)).join('\n')+`
 const mountShift = ${shiftEffect.getText(ast)};
 const readFacts = ${option('read')};
 const clearApproach = ${option('onClear')};
 const combatKey = ${attributes.get('onCombatKey').getText(ast)};
 const combatCancel = ${attributes.get('onCombatCancel').getText(ast)};
 const controls = () => (${attributes.get('combatModes').getText(ast)});
 const makeModeHost = () => (${modeHostInitializer.getText(ast)});
 function receiveMode(payload) { switch ("ChangeAMode") { ${attackModeCase.getText(ast)} } }
 function receiveGateway(event,connectionGeneration,source) { ${gatewayGuard.getText(ast)} if(event.type==="packet"&&event.packet==="ChangeAMode")receiveMode(event.payload); }
 return {${names.join(',')},mountShift,readFacts,clearApproach,combatKey,combatCancel,controls,makeModeHost,receiveGateway};`,
 {compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
const create=new Function('scope','with(scope){'+code+'}');

function eventSurface(){
 const listeners=[];
 return {listeners,addEventListener(type,fn,capture=false){listeners.push({type,fn,capture});},
  removeEventListener(type,fn,capture=false){const index=listeners.findIndex(listener=>listener.type===type&&listener.fn===fn&&listener.capture===capture);assert.ok(index>=0);listeners.splice(index,1);},
  emit(type,event={}){for(const listener of listeners.filter(listener=>listener.type===type))listener.fn(event);}};
}
function fixture(){
 const ref=current=>({current}),edges=[],logs=[],wire=[],fresh=[];
 const flags={abi:2,combatSupported:true,modeSupported:true,modal:false,focus:true,spectator:false};
 const actor=(objectId,kind,x)=>({objectId:String(objectId),kind,x,y:10,direction:'up',dead:false,hp:10,maxHp:10,classKey:'Warrior',level:1});
 const self=actor(3,'selfPlayer',10),player=actor(4,'player',12),monster=actor(5,'monster',11),npc=actor(6,'npc',13);
 const current={playerObjectId:'3',mapFileName:'D001',connected:true,selectedObjectId:null,entities:[self,player,monster,npc],
  playerHp:10,playerMaxHp:10,playerMp:10,stage5Systems:{attackMode:0},activeNpcDialog:null,originalMapRegion:null};
 const window=eventSurface(),document={...eventSurface(),visibilityState:'visible',hasFocus:()=>flags.focus,activeElement:null,
  querySelectorAll:selector=>selector.includes('img.')?[]:[{parentElement:null,getBoundingClientRect:()=>({width:1024,height:768})}]};
 let requestId=0;
 const runtime={getMir2CombatInputCapabilities:()=>JSON.stringify({schemaVersion:1,combatInputAbiVersion:flags.abi,combatIntentAbiVersion:1,compiled:true,startup:true}),
  setMir2CombatInputSnapshot:()=>true,setMir2CombatInputEdge:()=>'{"handled":true}',getMir2CombatInputStatus:()=>'{}',
  setMir2CombatInputIntentSink:()=>{},clearMir2CombatInputIntentSink:()=>{},withdrawMir2CombatInputSnapshot:()=>{},
  getMir2CombatModeKeysVersion:()=>flags.modeSupported?1:0,processMir2CombatModeKeys:json=>{
   const input=JSON.parse(json),result={version:1,ok:true};
   if(input.operation==='request'){
    const functions=['AttackmodePeace','AttackmodeGroup','AttackmodeGuild','AttackmodeEnemyguild','AttackmodeRedbrown','AttackmodeAll'];
    const command={type:'changeAMode',mode:functions.indexOf(input.function)};
    return JSON.stringify({...result,command,proof:{requestId:++requestId,owner:input.owner,revision:input.revision,function:input.function,command}});
   }
   return JSON.stringify(input.operation==='claim'?{...result,entered:true}:result);
  }};
 const scope={combatForceAttackRef:ref(false),worldRef:ref(current),world:current,runtimeRef:ref(runtime),
  combatIngressRef:ref(null),combatApproachIntentRef:ref(null),queuedMoveIntentRef:ref(null),movementPlanRef:ref(null),
  lockedMonsterAttackRef:ref(null),pendingNpcInteractRef:ref(null),pendingPickupRef:ref(null),pendingTransferRef:ref(null),pendingOnchainMineRef:ref(false),
  stopOnchainMining:()=>{},appendLog:(...args)=>logs.push(args),npcCallGuardRef:ref(null),interactTarget:id=>edges.push({type:'npc',objectId:id}),
  updateWorld:update=>{scope.worldRef.current=update(scope.worldRef.current);},supportsForcedPlayerAttack,
  window,document,HTMLElement:class HTMLElement{},getComputedStyle:()=>({display:'block',visibility:'visible',opacity:'1'}),
  combatPointerRef:ref({cursor:[12,10],hovered:'4',lock:false}),combatMastersRef:ref(new Map()),combatRouteCacheRef:ref(null),
  combatRawRef:ref({snapshot:{playerObjectId:3,mapFileName:'D001',knownSkills:[],entities:current.entities.map(entity=>({...entity,hasClassWeapon:true,ridingMount:false,fishing:false,dazed:false}))}}),
  readCurrentCombatBaseline:()=>true,currentSpellsOwner:()=>({connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3}),
  currentAuthoritativeSelf:world=>world.entities.find(entity=>entity.objectId===world.playerObjectId),heroUiBlocksGameplay:()=>false,
  skillBarPointerHeldRef:ref(false),otherPlayerUiBlocksInput:()=>flags.modal,bevyHpLocalOverlayOpenRef:ref(false),
  cashShopOpenRef:ref(false),referenceWindowsBlockGameplay:()=>false,spellsIngressRef:ref(null),webGl2SharedCanvasPrototype:false,
  movementBlockedStepsRef:ref([]),crystalMovementWalkRoute:(_self,point)=>[point],screenRef:ref('game'),initialSceneAssetsReadyRef:ref(true),
  equipmentHostSuspendReasonRef:ref(null),isMovementBusy:()=>false,nextMoveSendAtRef:ref(0),crystalKeyBindingsRef:ref(defaultCrystalKeyBindings()),
  displayCrystalKeyBinding,combatModePhysicalRef:ref(null),combatModeRawRef:ref(null),combatModeRevisionRef:ref(0),combatModeHostRef:ref(null),
  identity:{connectionGeneration:1,sessionGeneration:2,sceneRevision:1,playerObjectId:3,mapFileName:'D001'},
  currentQuestWorldIdentity:()=>scope.identity,nextCombatModePhysicalGeneration,WebSocket:{OPEN:1},socketRef:ref({readyState:1}),
  SharedCombatModeKeys,performance:{now:()=>350},isSpectatorBrowserMode:()=>flags.spectator,
  normalizeQuestMapFileName:value=>value,numberOrUndefined:value=>typeof value==='number'?value:undefined,
  attackModeChatMessage:mode=>Number.isInteger(mode)&&mode>=0&&mode<=5?{localizationKey:'mode',fallback:'mode '+mode}:null,
  gameEntryChatOverrideActiveRef:ref(false),t:(_key,_args,fallback)=>fallback,equipmentConnectionGenerationRef:ref(1),
  sendRaw:(command,options)=>{if(!scope.combatModeHostRef.current.claim(options.modeProof,command))return false;wire.push(command);return true;}};
 for(const name of ['bevyQuestReactModalOpen','showQuestLog','showCharacter','showHeroPet','showGuild','showGroup','showFriends','showBonds','showRanking',
  'showMarket','showConquest','showTrade','showBuffs','showMail','showWorldMap','showChatSettings','npcShopService','npcRepairService'])scope[name]=false;
 const api=create(scope);
 scope.bevyCombatInput=scope.combatIngressRef.current={supported:()=>flags.combatSupported,hasUiHeld:()=>false,
  edge:edge=>{edges.push(edge);if(edge.type==='cancel')api.clearApproach();return true;},fresh:()=>{fresh.push(scope.combatForceAttackRef.current);return true;}};
 scope.combatModeHostRef.current=api.makeModeHost();
 api.captureCombatModeSnapshot({playerObjectId:3,mapFileName:'D001',stage5Systems:{attackMode:0,petMode:0}});
 return {api,scope,flags,edges,logs,wire,fresh,window,document,player,monster,self};
}

test('actual Page selects a player with guidance and requires held Shift plus v2 to request attack',()=>{
 const f=fixture();f.api.activateEntity('4');assert.equal(f.scope.worldRef.current.selectedObjectId,'4');
 assert.equal(f.edges.filter(edge=>edge.type==='select').length,0);assert.match(f.logs.at(-1)[0],/Shift.*左键/);
 f.api.setCombatForceAttack(true);f.api.activateEntity('4');
 assert.deepEqual(f.edges.at(-1),{type:'select',objectId:4});assert.equal(f.api.readFacts().facts.forceAttack,true);
 f.api.setCombatForceAttack(false);assert.equal(f.edges.at(-1).type,'cancel');assert.equal(f.api.readFacts().facts.forceAttack,false);
 const before=f.edges.length;f.api.lockMonsterAttack('4');assert.equal(f.edges.length,before,'direct locked-player entry cannot bypass Shift');
 f.flags.abi=1;f.api.setCombatForceAttack(true);f.api.activateEntity('4');assert.match(f.logs.at(-1)[0],/更新客户端/);
 assert.equal(f.edges.filter(edge=>edge.type==='select').length,1,'legacy runtimes retain selection without requesting PvP');
 f.api.activateEntity('5');assert.deepEqual(f.edges.at(-1),{type:'select',objectId:5},'v1 retains ordinary monster combat');
});

test('actual Shift lifecycle stops only player pursuit on release and retires on blur, pagehide, hidden and cleanup',()=>{
 const f=fixture(),cleanup=f.api.mountShift();assert.equal(f.window.listeners.filter(listener=>listener.type==='keyup')[0].capture,true);
 f.window.emit('keydown',{key:'Shift',code:'ShiftLeft',shiftKey:true});f.api.activateEntity('4');
 const own={kind:'target'};f.scope.combatApproachIntentRef.current=own;f.scope.queuedMoveIntentRef.current=own;f.scope.movementPlanRef.current={targetX:11};
 f.window.emit('keyup',{key:'Shift',code:'ShiftLeft',shiftKey:false});
 assert.equal(f.scope.combatForceAttackRef.current,false);assert.equal(f.edges.at(-1).type,'cancel');
 assert.equal(f.scope.queuedMoveIntentRef.current,null);assert.equal(f.scope.movementPlanRef.current,null);
 f.window.emit('keydown',{key:'Shift',code:'ShiftRight',shiftKey:true});f.api.activateEntity('5');const count=f.edges.length;
 f.window.emit('keyup',{key:'Shift',code:'ShiftRight',shiftKey:false});assert.equal(f.edges.length,count,'monster pursuit remains active after Shift release');
 for(const event of ['blur','pagehide','hidden']){
  f.window.emit('keydown',{key:'Shift',code:'ShiftLeft',shiftKey:true});
  if(event==='hidden'){f.document.visibilityState='hidden';f.document.emit('visibilitychange');f.document.visibilityState='visible';}
  else f.window.emit(event);
  assert.equal(f.scope.combatForceAttackRef.current,false,event);assert.equal(f.edges.at(-1).type,'cancel',event);
 }
 f.window.emit('keydown',{key:'Shift',code:'ShiftLeft',shiftKey:true});cleanup();
 assert.equal(f.scope.combatForceAttackRef.current,false);assert.equal(f.window.listeners.length,0);assert.equal(f.document.listeners.length,0);
});

test('actual Page key and cancel props retain skill forwarding while synchronizing held Shift',()=>{
 const f=fixture(),action={type:'key',key:'F1',modifiers:{shift:true,ctrl:false,alt:false,tilde:false}};
 assert.deepEqual(f.api.combatKey(action,true),{supported:true,handled:true});assert.equal(f.scope.combatForceAttackRef.current,true);
 assert.deepEqual(f.edges.at(-1),{...action,repeat:true});f.api.combatCancel();assert.equal(f.scope.combatForceAttackRef.current,false);
 assert.equal(f.edges.at(-1).type,'cancel');f.flags.modal=true;assert.equal(f.api.readFacts().facts.enabled,false,'existing modal gate still controls the shared controller');
});

test('actual Page preserves manual movement when retiring a different combat approach',()=>{
 const f=fixture(),manual={kind:'direction'},plan={direction:'up'};
 f.scope.combatApproachIntentRef.current={kind:'target'};f.scope.queuedMoveIntentRef.current=manual;f.scope.movementPlanRef.current=plan;
 f.api.clearApproach();assert.equal(f.scope.queuedMoveIntentRef.current,manual);assert.equal(f.scope.movementPlanRef.current,plan);
});

test('actual Page mode controls use custom shortcut and native request proofs with no optimistic mode update',()=>{
 const f=fixture();f.scope.crystalKeyBindingsRef.current=f.scope.crystalKeyBindingsRef.current.map(binding=>binding.function==='ChangeAttackmode'?{...binding,key:'K',ctrl:0,alt:1,shift:1}:binding);
 const controls=f.api.controls();assert.equal(controls.shortcut,'Alt + Shift + K');assert.equal(controls.mode,0);assert.equal(controls.enabled,true);
 controls.onSelectMode(5);assert.deepEqual(f.wire,[{type:'changeAMode',mode:5}]);assert.equal(f.api.controls().mode,0);assert.equal(f.scope.worldRef.current.stage5Systems.attackMode,0);
 for(const mode of [-1,6,NaN,2.5])controls.onSelectMode(mode);assert.equal(f.wire.length,1);
 for(let mode=0;mode<=5;mode++)controls.onSelectMode(mode);assert.deepEqual(f.wire.slice(1).map(command=>command.mode),[0,1,2,3,4,5]);
 f.flags.modal=true;controls.onSelectMode(1);assert.equal(f.wire.length,7,'a stale visible callback rechecks the current native request source');
 f.flags.modal=false;f.flags.modeSupported=false;assert.equal(f.api.controls().enabled,false);controls.onSelectMode(1);assert.equal(f.wire.length,7);
});

test('actual ChangeAMode updates raw authority and visible world only for a valid current receipt',()=>{
 const f=fixture(),socket=f.scope.socketRef.current,event=mode=>({type:'packet',packet:'ChangeAMode',payload:{mode}});
 f.api.receiveGateway(event(5),1,socket);assert.equal(f.api.controls().mode,5);assert.equal(f.scope.worldRef.current.stage5Systems.attackMode,5);
 for(const mode of [-1,6,'2',NaN])f.api.receiveGateway(event(mode),1,socket);
 assert.equal(f.api.controls().mode,5);assert.equal(f.scope.worldRef.current.stage5Systems.attackMode,5);
 f.api.receiveGateway(event(1),2,socket);f.api.receiveGateway(event(1),1,{readyState:1});
 assert.equal(f.api.controls().mode,5);assert.equal(f.scope.worldRef.current.stage5Systems.attackMode,5,'old connection and foreign socket receipts cannot repaint mode');
 f.scope.identity={...f.scope.identity,sessionGeneration:3};assert.equal(f.api.controls().mode,null);
 f.api.controls().onSelectMode(1);assert.equal(f.wire.length,0,'old raw modes cannot be relabeled as a new session');
});

test('actual session retirement clears held Shift and its raw combat source before another player can be targeted',()=>{
 const f=fixture(),ref=current=>({current});let withdrawn=0;
 Object.assign(f.scope,{npcPurchaseClientRef:ref(null),npcPurchaseEconomicSourceRef:ref(null),npcPurchaseDisplaySourceRef:ref(null),
  npcPearlShopSourceRef:ref({retire(){}}),retireWorldFishingGesture(){},worldFishingSourceRef:ref({retire(){}}),worldFishingCommitRef:ref(null),
  retireSocialRequests(){},observePreferenceRef:ref(null),observeBootstrapRef:ref(null),advanceHeroWindowEpochs(){},skillBarDocumentCacheRef:ref(null),
  retireSocialItemConnection(){},npcGoldBuyInventoryRef:ref({invalidate(){}}),npcRepairAuthorityRef:ref({invalidateInventory(){}}),
  retireNpcShopService(){},endStorageService(){},pendingStorageRequestsRef:ref(new Map()),setMailboxOpen(){},mailRawRef:ref(null),
  combatBaselineSourceRef:ref({old:true}),spellsRawSnapshotRef:ref(null),equipmentControllerRef:ref(null),equipmentStartGameRef:ref({}),equipmentSnapshotRef:ref({})});
 f.scope.combatIngressRef.current.withdraw=()=>{withdrawn++;};
 const retireCode=ts.transpileModule(functions.get('retireEquipmentSession').getText(ast)+'\nreturn retireEquipmentSession;',{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const retire=new Function('scope','with(scope){'+retireCode+'}')(f.scope);
 f.api.setCombatForceAttack(true);retire();assert.equal(f.scope.combatForceAttackRef.current,false);assert.equal(withdrawn,1);
 assert.equal(f.scope.combatRawRef.current,null);assert.equal(f.scope.combatBaselineSourceRef.current,null);assert.equal(f.scope.combatModeRawRef.current,null);
 f.api.activateEntity('4');assert.equal(f.edges.filter(edge=>edge.type==='select').length,0,'new session requires a fresh held gesture');
});
