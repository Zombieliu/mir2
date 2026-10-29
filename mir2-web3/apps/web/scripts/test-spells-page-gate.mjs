import assert from 'node:assert/strict';import test from 'node:test';import {readFileSync} from 'node:fs';import ts from 'typescript';
const text=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('shell.tsx',text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
function loadUi(){const m={exports:{}};new Function('exports','module',ts.transpileModule(readFileSync(new URL('../lib/bevy-spells-ui.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m);return m.exports;}const ui=loadUi();
function loadQuest(url){const m={exports:{}};new Function('exports','module','require',ts.transpileModule(readFileSync(url,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m,key=>loadQuest(new URL(`${key}.ts`,url)));return m.exports;}
const {questWorldControlAt}=loadQuest(new URL('../lib/bevy-quest-world-controls.ts',import.meta.url));
const {storagePrepareCanvasStyle}=loadQuest(new URL('../lib/bevy-storage-presentation.ts',import.meta.url));
function loadMailUi(){const compile=(url,require)=>{const m={exports:{}};new Function('exports','module','require',ts.transpileModule(readFileSync(url,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m,require);return m.exports;};const purchase=compile(new URL('../lib/npc-purchase-client.ts',import.meta.url),id=>{throw Error(`unexpected pure purchase import: ${id}`);});const packets=compile(new URL('../lib/extended-server-packets.ts',import.meta.url),id=>{assert.equal(id,'./npc-purchase-client');return purchase;});return compile(new URL('../lib/bevy-mail-ui.ts',import.meta.url),id=>{assert.equal(id,'./extended-server-packets');return packets;});}const mailUi=loadMailUi();
function node(name){let found;function visit(n){if(ts.isFunctionDeclaration(n)&&n.name?.text===name)found=n;ts.forEachChild(n,visit);}visit(ast);assert.ok(found,name);return found;}
function load(names,scope){const code=ts.transpileModule(names.map(n=>node(n).getText(ast)).join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;return new Function(...Object.keys(scope),`${code};return {${names.join(',')}};`)(...Object.values(scope));}
function fixture(){let context={requestRun:1,connectionGeneration:2,sessionGeneration:3,ownerRevision:0,hudGeneration:4,playerObjectId:5,modelRevision:6,presentationRevision:7,renderRevision:8,modal:true,inputRegions:[{left:322,top:312,width:380,height:144}],presentation:{logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false}},bag=null,hud=0,stops=0;const edges=[],retired=[];class Element{constructor(){this.id='canvas';}setPointerCapture(){}closest(){return null;}}
 const scope={parityUiBlocksGameplay:undefined,onHeroShortcut:undefined,npcShopPointerRouterRef:{current:{held:null}},npcShopBlocksWorldInput:()=>false,handleSharedNpcShopPointer:()=>false,handleSharedComposePointer:()=>false,handleSharedStoragePointer:()=>false,storagePointerRouterRef:{current:{held:null}},bevyNpcShopUiActive:false,bevyNpcShopUiTransitioning:false,bevyStorageUiActive:false,bevyStorageUiTransitioning:false,bevyMailComposeReady:false,bevyMailComposePending:false,mailPointerRouterRef:{current:new mailUi.MailPointerRouter()},mailPointerCallbacksRef:{current:{getBevyMailPointerContext:()=>null,onBevyMailPointer:()=>{throw Error('idle Mail cannot capture a Spells pointer');}}},bevyMailPageReady:false,spellsPointerRouterRef:{current:new ui.SpellsPointerRouter()},spellsPointerCallbacksRef:{current:{getBevySpellsPointerContext:()=>context,onBevySpellsPointer:e=>{edges.push(e);return true;}}},bagPointerCallbacksRef:{current:{getBevyBagPointerContext:()=>bag}},bagPointerRouterRef:{current:{held:null}},hudPointerRouterRef:{current:{held:null}},characterPointerRouterRef:{current:{held:null}},stageFrameRef:{current:{focus(){}}},heldScenePointerRef:{current:null},onViewportDirectionStop:()=>{stops++;},sceneInteractionReady:true,questLocalModalOpen:false,mobileMoreOpen:false,bevyQuestUiCapturesPointer:false,bevySpellsPageReady:true,HTMLElement:Element,sharedUiCanvasId:()=> 'canvas',webGl2SharedCanvasPrototype:false,scenePointFromMouseEvent:e=>({sceneX:e.clientX,sceneY:e.clientY}),cancelSharedCharacterPointer:()=>retired.push('character'),cancelSharedBagPointer:()=>retired.push('bag'),cancelSharedHudPointer:()=>retired.push('hud'),handleSharedBagPointer:()=>{hud++;}};
 Object.assign(scope,{heroPointerLeaseRef:{current:null},handleNpcRepairPointer:()=>false,handleBagBeltPointer:()=>false,handleSharedHeroPointer:()=>false,cancelWorldFishingHeldPointer:()=>{},heldQuestControlPointersRef:{current:new Set()},readBevyQuestWorldControls:()=>null,readBevyQuestWorldControlBlockers:()=>null,questWorldControlAt,readBevyHudStatus:()=>null,screen:'game',bevyBagUiActive:false});
 scope.stageFrameRef.current.dataset={viewportSceneWidth:'1024',viewportSceneHeight:'768'};
 const api=load(['cancelSharedMailPointer','handleSharedMailPointer','cancelSharedSpellsPointer','handleSharedSpellsPointer','handleSharedQuestWorldPointer','handleSharedUiPointer'],scope);const event={target:new Element(),pointerId:1,pointerType:'mouse',button:0,clientX:350,clientY:350,preventDefault(){this.prevented=true;}};
 return{api,event,scope,edges,retired,get hud(){return hud;},get stops(){return stops;},get context(){return context;},set context(v){context=v;},set bag(v){bag=v;}};}
test('actual modal captures before Character/Bag/HUD/world, outside clicks and second pointer cannot save',()=>{const f=fixture();f.api.handleSharedUiPointer(f.event,'down');assert.deepEqual(f.retired,['character','bag','hud']);assert.equal(f.hud,0);assert.equal(f.edges[0].phase,'down');f.api.handleSharedUiPointer({...f.event,pointerId:2},'down');assert.equal(f.edges.at(-1).phase,'cancel');f.api.handleSharedUiPointer(f.event,'up');assert.equal(f.edges.length,2);assert.equal(f.hud,0);
 f.api.handleSharedUiPointer({...f.event,clientX:20},'down');assert.equal(f.edges.length,2);assert.equal(f.hud,0);assert.ok(f.stops>0);
});
test('actual page yields foreground Bag, crossing or revision retires capture',()=>{const f=fixture();f.context={...f.context,modal:false};f.bag={inputRegions:f.context.inputRegions};f.api.handleSharedUiPointer(f.event,'down');assert.equal(f.hud,1);assert.equal(f.edges.length,0);f.bag=null;f.api.handleSharedUiPointer(f.event,'down');f.api.handleSharedUiPointer({...f.event,clientX:900},'move');assert.equal(f.edges.at(-1).phase,'cancel');f.api.handleSharedUiPointer(f.event,'down');f.context={...f.context,modelRevision:9};f.api.handleSharedUiPointer(f.event,'up');assert.equal(f.edges.at(-1).phase,'cancel');});
test('fixed stage UI canvas is interactive for Spells in GPU and shared GL2 while world canvas stays in its composite',()=>{const canvases=[];function visit(n){if(ts.isJsxSelfClosingElement(n)&&n.tagName.getText(ast)==='canvas'){const id=n.attributes.properties.find(a=>a.name?.getText(ast)==='id');if(id?.initializer&&['mir2-web3-canvas','mir2-quest-ui-canvas'].includes(id.initializer.text))canvases.push(n);}ts.forEachChild(n,visit);}visit(ast);assert.equal(canvases.length,2);
 const world=canvases.find(n=>n.attributes.properties.some(a=>a.name?.getText(ast)==='id'&&a.initializer?.text==='mir2-web3-canvas'));
 const stage=canvases.find(n=>n.attributes.properties.some(a=>a.name?.getText(ast)==='id'&&a.initializer?.text==='mir2-quest-ui-canvas'));
 assert.ok(world&&stage);assert.ok(ts.isJsxElement(world.parent));assert.match(world.parent.openingElement.getText(ast),/game-world-composite/);
 assert.equal(world.attributes.properties.some(a=>a.name?.getText(ast)==='data-ui-interactive'),false);
 assert.equal(stage.attributes.properties.some(a=>a.name?.getText(ast)==='data-ui-interactive'),true);
 for(const webGl2SharedCanvasPrototype of [false,true]){const scope={screen:'game',webGl2SharedCanvasPrototype,bevyHeroUiActive:false,bevyHeroUiTransitioning:false,bevyStoragePreparePresentation:null,storagePrepareCanvasStyle,sharedUiCanvasId:()=> 'mir2-quest-ui-canvas',bevyMailPageReady:false,bevyCharacterPageReady:false,bevySpellsPageReady:true,bevyQuestUiCapturesPointer:false,bevyQuestWorldUiReady:false,bevyQuestUiReady:false,bevyBagUiActive:false,bevyHudUiReady:false,bevyNpcShopUiActive:false,bevyNpcShopUiTransitioning:false,bevyStorageUiActive:false,bevyStorageUiTransitioning:false,hpOrbOwner:false,mpOrbOwner:false,experienceBarOwner:false,weightBarOwner:false};const value=name=>new Function(...Object.keys(scope),`return (${stage.attributes.properties.find(a=>a.name?.getText(ast)===name).initializer.expression.getText(ast)});`)(...Object.values(scope));assert.match(value('className'),/shared-quest-ui-visible/);assert.equal(value('data-ui-interactive'),'true');assert.equal(value('style').pointerEvents,'auto');scope.bevySpellsPageReady=false;assert.equal(value('data-ui-interactive'),undefined);assert.equal(value('style').pointerEvents,'none');}
});
test('actual modal fences existing shortcuts and held keyboard movement before dispatch',()=>{let prevented=0,stops=0;const keys=new Set(['left']);const scope={getKeybindCaptureActive:undefined,document:{visibilityState:'visible',hasFocus:()=>true},npcShopBlocksWorldInput:()=>false,stopNpcShopWorldInput:()=>{throw Error('inactive NPC shop cannot own keyboard input');},bevyStorageUiActive:false,bevyStorageUiTransitioning:false,storagePointerCallbacksRef:{current:{getBevyStoragePointerContext:()=>null}},bevyMailComposeReady:false,bevyMailComposePending:false,spellsPointerCallbacksRef:{current:{getBevySpellsPointerContext:()=>({modal:true})}},heldKeyboardMoveKeysRef:{current:keys},heldKeyboardRunModeRef:{current:true},onViewportDirectionStop:()=>{stops++;}};const api=load(['handleShortcutKey','handleKeyboardMoveDown','dispatchKeyboardMoveInput'],scope);const event={key:'F1',preventDefault:()=>{prevented++;}};api.handleShortcutKey(event);api.handleKeyboardMoveDown(event);api.dispatchKeyboardMoveInput();assert.equal(prevented,2);assert.equal(stops,1);assert.equal(keys.size,0);assert.equal(scope.heldKeyboardRunModeRef.current,false);});

function pageSpellsNodes(){
  const source=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8');
  const pageAst=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  let read,props;
  function visit(n){
    if(ts.isCallExpression(n)&&n.expression.getText(pageAst)==='useBevySpellsUi'){
      const options=n.arguments[0];
      read=options.properties.find(p=>p.name?.getText(pageAst)==='read')?.initializer;
    }
    if((ts.isJsxSelfClosingElement(n)||ts.isJsxOpeningElement(n))&&n.tagName.getText(pageAst)==='OriginalClientShell')props=n.attributes.properties;
    ts.forEachChild(n,visit);
  }
  visit(pageAst);assert.ok(read&&props);return{pageAst,read,props};
}
function pageSpellsFixture(){
  const owner={connectionGeneration:2,sessionGeneration:3,ownerRevision:4,playerObjectId:5};
  const state={hudReady:true,focused:true,hudReads:0,focusReads:0};
  const scope={worldRef:{current:{connected:true,playerObjectId:5,entities:[{objectId:5}],activeNpcDialog:null}},
    currentSpellsOwner:()=>owner,spellsRawSnapshotRef:{current:{snapshot:{playerObjectId:5,knownSkills:[]},owner}},
    bevyQuestReactModalOpen:false,heroManagementPage:null,cashShopOpenRef:{current:false},referenceWindowsBlockGameplay:()=>false,npcShopService:null,npcRepairService:null,
    bevyQuestUiRequested:true,bevyRuntimeBackend:'webgpu',sharedCanvasUsesWebGl2:()=>false,webGl2SharedCanvasPrototype:false,
    screenRef:{current:'game'},initialSceneAssetsReadyRef:{current:true},equipmentHostSuspendReasonRef:{current:null},
    bevyHudUi:{readCurrent(){state.hudReads++;return{ready:state.hudReady};}},
    document:{visibilityState:'visible',hasFocus(){state.focusReads++;return state.focused;},querySelector:()=>null},
    bevyQuestGenerationRef:{current:1},showCharacter:true,activeCharacterTab:'spells',
    projectAuthoritativeHudPlayer:()=>({hp:24,maxHp:24}),readBevyQuestPresentation:()=>({logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false}),
    sharedUiCanvasId:()=> 'mir2-quest-ui-canvas',clientProfile:{input:'pointer'}};
  for(const key of ['showQuestLog','showHeroPet','showGuild','showGroup','showFriends','showBonds','showRanking','showMarket','showConquest','showTrade','showBuffs','showMail','showWorldMap','showChatSettings'])scope[key]=false;
  const n=pageSpellsNodes(),output=ts.transpileModule('const actualRead=('+n.read.getText(n.pageAst)+');',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  const factory=new Function(...Object.keys(scope),output+';return actualRead;');
  return{scope,state,read:metadata=>factory(...Object.values(scope))(metadata)};
}
test('actual Page empty-Spells eligibility preserves every refusal and its short-circuit HUD/focus reads',()=>{
  const ok=pageSpellsFixture(),input=ok.read(null);
  assert.equal(input.eligible,true);assert.equal(input.ineligibleReason,undefined);assert.equal(input.open,true);assert.equal(input.iconMetadata,null);
  assert.equal(ok.state.hudReads,1);assert.equal(ok.state.focusReads,1);
  const cases=[
    ['request-disabled',f=>f.scope.bevyQuestUiRequested=false,0,0],
    ['backend-unsupported',f=>f.scope.bevyRuntimeBackend='canvas',0,0],
    ['not-in-game',f=>f.scope.screenRef.current='login',0,0],
    ['disconnected',f=>f.scope.worldRef.current.connected=false,0,0],
    ['player-missing',f=>f.scope.worldRef.current.entities=[],0,0],
    ['owner-not-current',f=>{f.scope.worldRef.current.playerObjectId=9;f.scope.worldRef.current.entities=[{objectId:9}];},0,0],
    ['scene-assets-unready',f=>f.scope.initialSceneAssetsReadyRef.current=false,0,0],
    ['equipment-suspended',f=>f.scope.equipmentHostSuspendReasonRef.current='handoff',0,0],
    ['modal-open',f=>f.scope.showQuestLog=true,0,0],
    ['hud-unready',f=>f.state.hudReady=false,1,0],
    ['hidden',f=>f.scope.document.visibilityState='hidden',1,0],
    ['unfocused',f=>f.state.focused=false,1,1],
  ];
  for(const [reason,change,hudReads,focusReads] of cases){
    const f=pageSpellsFixture();change(f);const refused=f.read(null);
    assert.equal(refused.eligible,false,reason);assert.equal(refused.ineligibleReason,reason);
    assert.equal(f.state.hudReads,hudReads,reason);assert.equal(f.state.focusReads,focusReads,reason);
  }
});
test('public Spells diagnostic transports only the finite host report and never substitutes for readiness',()=>{
  const n=pageSpellsNodes(),attribute=name=>n.props.find(p=>p.name?.getText(n.pageAst)===name);
  const diagnostic=attribute('spellsUiDiagnostics'),ready=attribute('bevySpellsPageReady');assert.ok(diagnostic&&ready);
  function evaluate(expression,scope){const output=ts.transpileModule('const value=('+expression+');',{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;return new Function(...Object.keys(scope),output+';return value;')(...Object.values(scope));}
  const finite={ready:true,reason:'ready'},scope={bevySpellsUi:{ready:false,diagnostic:finite,privateRuntimeStatus:{playerObjectId:999,learned:['not public']}}};
  assert.equal(evaluate(diagnostic.initializer.expression.getText(n.pageAst),scope),JSON.stringify(finite));
  assert.equal(evaluate(ready.initializer.expression.getText(n.pageAst),scope),false);
  let publicAttribute;
  function visit(node){if(ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node))for(const p of node.attributes.properties)if(p.name?.getText(ast)==='data-spells-ui-status')publicAttribute=p;ts.forEachChild(node,visit);}
  visit(ast);assert.ok(publicAttribute);assert.equal(publicAttribute.initializer.expression.getText(ast),'spellsUiDiagnostics');
});
