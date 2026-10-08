import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

// These tests consume actual source declarations. No React, Core loader, WASM,
// renderer, DOM, real socket or filesystem-output fixture is initialized.
const source = name => readFileSync(new URL(name, import.meta.url), "utf8");
const pageText = source("../app/page.tsx"), shellText = source("../app/original-client-shell.tsx");
function tree(text, filename) { return ts.createSourceFile(filename, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX); }
const pageTree = tree(pageText, "actual-page.tsx"), shellTree = tree(shellText, "actual-shell.tsx");
function nodes(ast, predicate) {
  const found = []; const visit = n => { if (predicate(n)) found.push(n); ts.forEachChild(n, visit); }; visit(ast); return found;
}
function declaration(ast, name) {
  const found = nodes(ast, n => ts.isFunctionDeclaration(n) && n.name?.text === name);
  assert.equal(found.length, 1, "sole actual function: " + name); return found[0];
}
function actualFunctions(ast, names, scope) {
  const input = names.map(n => declaration(ast, n).getText(ast).replace(/^export\s+/, "")).join("\n");
  const code = ts.transpileModule(input, { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), code + "\nreturn {" + names.join(",") + "};")(...Object.values(scope));
}
const modules = new Map(), pureFiles = { spells:"../lib/bevy-spells-ui.ts", combat:"../lib/bevy-combat-input.ts", npcBuy:"../lib/bevy-npc-shop-buy.ts",
  modeKeys:"../lib/shared-combat-mode-keys.ts", hero:"../lib/hero-player-ui.ts", operations:"../lib/social-window-operations.ts", equipment:"../lib/equipment-gateway-adapter.ts",
  parcel:"../lib/mail-parcel-gateway-adapter.ts", storage:"../lib/storage-gateway-adapter.ts",
  questWorld:"../lib/bevy-quest-world-context.ts", questControls:"../lib/bevy-quest-world-controls.ts", bagModel:"../lib/bevy-bag-model.ts",
  npcPearl:"../lib/npc-pearl-buy.ts",npcPearlSource:"../lib/npc-pearl-buy-source.ts",crystalSource:"../lib/crystal-item-source.ts" };
const pureRequires = { modeKeys:{}, hero:{}, spells:{}, combat:{"./bevy-spells-ui":"spells"}, npcBuy:{"./npc-pearl-buy":"npcPearl"}, operations:{},
  npcPearl:{"./npc-pearl-buy-source":"npcPearlSource"},npcPearlSource:{"./crystal-item-source":"crystalSource"},crystalSource:{},
  equipment:{"./world-model/item-identity":"identity"}, parcel:{"./equipment-gateway-adapter":"equipment"},
  storage:{"./equipment-gateway-adapter":"equipment","./world-model/item-identity":"identity","./mail-parcel-gateway-adapter":"parcel"},
  questWorld:{"./bevy-bag-model":"bagModel"}, questControls:{"./bevy-quest-world-context":"questWorld"},
  bagModel:{"./world-model/item-identity":"identity"} };
function loadPure(name) {
  assert(Object.hasOwn(pureFiles,name), "source outside finite pure allowlist");
  if (modules.has(name)) return modules.get(name);
  const code = ts.transpileModule(source(pureFiles[name]), {compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  const module={exports:{}}; new Function("require","exports","module",code)(id=>{
    assert(Object.hasOwn(pureRequires[name],id), "unexpected runtime import: " + id);
    return loadPure(pureRequires[name][id]);
  },module.exports,module); modules.set(name,module.exports); return module.exports;
}
const { CombatHost } = loadPure("combat");
const { NpcGoldBuyInventoryReadiness, npcGoldBuyInventoryMutationPacket } = loadPure("npcBuy");
function combatFixture() {
  const withdrawals=[], snapshots=[], wires=[], ready=[]; let owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:7};
  const runtime={getMir2CombatInputCapabilities:()=>JSON.stringify({schemaVersion:1,combatInputAbiVersion:1,combatIntentAbiVersion:1,compiled:true,startup:true}),
    setMir2CombatInputSnapshot:json=>{snapshots.push(json);return true;},setMir2CombatInputEdge:()=>'{"handled":true}',
    getMir2CombatInputStatus:()=>JSON.stringify({version:1,frame:0,ready:false,identity:null,modelRevision:null,revision:null,map:null}),
    setMir2CombatInputIntentSink:()=>{},clearMir2CombatInputIntentSink:()=>{},withdrawMir2CombatInputSnapshot:json=>withdrawals.push(json)};
  const host=new CombatHost({runtime,isCurrent:()=>true,read:()=>({owner,facts:null}),now:()=>100,
    onReady:v=>ready.push(v),onWire:(...args)=>{wires.push(args);return "definitelyUnsent";},onApproach:()=>{},onClear:()=>{}});
  return {host,withdrawals,snapshots,wires,ready,get owner(){return owner;},set owner(v){owner=v;}};
}

test("actual combat NPC channel holds only the current opaque token and leaves other channels intact",()=>{
  const f=combatFixture(),old={},fresh={},bag={};
  assert(f.host.setUiHeld("bag",bag,true)); assert(f.host.setUiHeld("npcShop",old,true));
  assert(f.host.setUiHeld("npcShop",fresh,true)); const before=f.withdrawals.length;
  assert.equal(f.host.setUiHeld("npcShop",old,false),false); assert(f.host.hasUiHeld());
  assert.equal(f.withdrawals.length,before); assert(f.host.setUiHeld("npcShop",fresh,false));
  assert(f.host.hasUiHeld(),"releasing NPC cannot remove Bag hold"); assert(f.host.setUiHeld("bag",bag,false));
  assert.equal(f.host.hasUiHeld(),false); assert.equal(f.wires.length,0);
});

test("actual combat channel rejects unknown labels and retires old-owner NPC hold before publishing",()=>{
  const f=combatFixture(),token={}; assert.equal(f.host.setUiHeld("npc",token,true),false);
  assert.equal(f.host.setUiHeld("npcShop",null,true),false); assert(f.host.setUiHeld("npcShop",token,true));
  f.owner={...f.owner,sessionGeneration:3}; f.host.tick(); assert.equal(f.host.hasUiHeld(),false);
  assert.equal(f.host.setUiHeld("npcShop",token,false),false); assert.equal(f.snapshots.length,0); assert.equal(f.wires.length,0);
});

const identityModule = (() => {
  const module={exports:{}}; const code=ts.transpileModule(source("../lib/world-model/item-identity.ts"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  new Function("exports","module","require",code)(module.exports,module,id=>assert.fail("unexpected identity import: "+id)); return module.exports;
})();
const bagProjection = (() => {
  const module={exports:{}}; const code=ts.transpileModule(source("../lib/bevy-bag-model.ts"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
  new Function("exports","module","require",code)(module.exports,module,id=>{
    assert.equal(id,"./world-model/item-identity");return identityModule;
  });return module.exports.projectBevyBagModel;
})();
pureFiles.identity="../lib/world-model/item-identity.ts";pureRequires.identity={};modules.set("identity",identityModule);
pureFiles.bag="../lib/bevy-bag-ui.ts";pureRequires.bag={"./world-model/item-identity":"identity","./bag-belt-gesture":"bagBelt"};
pureFiles.bagBelt="../lib/bag-belt-gesture.ts";pureRequires.bagBelt={};
pureFiles.bagBeltMove="../lib/bag-belt-move-dispatcher.ts";pureRequires.bagBeltMove={"./mail-parcel-gateway-adapter":"parcel"};
const npcAuthSelectors=actualFunctions(tree(source("../lib/client-login-runtime.ts"),"actual-login-runtime.ts"),
  ["preauthCommandKind","isSensitiveGatewayCommand"],{});
const pageNames=["npcGoldBuyCurrent","readNpcGoldBuyCurrent","currentNpcShopTab","setNpcShopCompatibilityTab",
  "readNpcShopUiInput","npcShopIntentMatchesStatus","npcShopIntentMatchesCommand","npcShopUiSendCurrent",
  "dispatchBevyNpcShopIntent","retireNpcShopService","sendRaw","itemCommandRequiresOwner",
  "invalidateNpcGoldBuyGatewayPacket","applyNpcGoldBuyGatewaySnapshot","npcPurchaseSessionKey","applyNpcPurchaseOrdinarySnapshot","socialItemMutationAllowed", "parityItemMutationAllowed",
  "currentSpellsOwner","currentEquipmentOwner","currentSocialReplyOwner","currentSocialReceiveOwner","sameSocialPhysicalOwner","captureParitySnapshot",
  "referenceWindowsBlockGameplay","syncObservePreference","currentQuestWorldIdentity","currentCombatModeOwner",
  "nextCombatModeRevision","captureCombatModeSnapshot"];
const clone=v=>JSON.parse(JSON.stringify(v));
function pageFixture() {
  const trace=[],sent=[],receipts=[],prepared=[],tabs=[],closed=[],errors=[];let listener=null,throws=false,allow=true,claimed=false,withdrawResult=true,finalUiCurrent=true,beforeLast=null;
  let presentation={logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false};
  const world={connected:true,playerObjectId:"1",playerHp:100,playerMaxHp:100,playerMp:50,playerMaxMp:50,
    mapFileName:"D000",mapTitle:"Home",inSafeZone:false,gold:100,credit:0,playerCrystalStats:null,
    playerExperience:0,playerMaxExperience:0,currentWeight:0,maxWeight:100,playerWeights:null,activeNpcDialog:null,
    entities:[{objectId:"1",x:4,y:5,dead:false,level:1,name:"Player",classKey:null,genderKey:null,hair:null,wingEffect:null}],
    inventoryCapacity:46,maxBagSlots:40,inventoryItems:[],beltItems:[],equipmentItems:[],storageItems:[]};
  const service={supportsBuy:true,supportsSell:true,serviceRevision:1,catalogRevision:1,panelType:0,hideAddedStats:false,
    buyItems:[{id:0,name:"Potion",price:17,purchaseRate:0.9,requiresGoldBuyPlan:true,count:1,stock:-1,icon:10,
      description:"raw",tooltipSource:{info:{index:5,stack_size:20},userItem:{unique_id:0,count:1,is_shop_item:true}}}]};
  const proof={runGeneration:10,connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:1,
    revision:3,modelRevision:2,presentationRevision:1,serviceRevision:1,catalogRevision:1,
    coreAuthorityRevision:"9007199254740993",controlRevision:"9007199254740995"};
  const status={runGeneration:proof.runGeneration,connectionGeneration:proof.connectionGeneration,sessionGeneration:proof.sessionGeneration,
    ownerRevision:proof.ownerRevision,playerObjectId:proof.playerObjectId,frame:1,ready:true,inputEnabled:true,error:null,
    appliedRevision:proof.revision,appliedModelRevision:proof.modelRevision,appliedPresentationRevision:proof.presentationRevision,
    appliedServiceRevision:proof.serviceRevision,appliedCatalogRevision:proof.catalogRevision,
    coreAuthorityRevision:proof.coreAuthorityRevision,controlRevision:proof.controlRevision,selectedId:0,quantity:2,startIndex:0,
    feedback:{phase:null,pending:false,canReserve:true,previousUnknown:0},
    inputRegions:[{left:220,top:217,width:244,height:334},{left:474,top:251,width:330,height:80}]};
  const intent={proof,gesture:{pointerId:1,downSequence:1,sequence:2,origin:"shop",button:0},intentSequence:1,
    action:{type:"buy"},selectedId:0,quantity:2,startIndex:0,preentryWithdraw:false,
    command:{type:"buyItem",itemIndex:0,count:2,panelType:0}};
  const socket={readyState:1,send(body){trace.push("socket");if(throws)throw Error("controlled send exception");sent.push(JSON.parse(body));}};
  const ui={blocksInput:()=>true,readStatus:()=>status,allows:i=>allow&&i===intent,
    claimCurrent(i){return finalUiCurrent&&i===intent;},
    stop(){finalUiCurrent=false;trace.push("uiStop");},
    claim(i){trace.push("uiClaim");if(!allow||i!==intent||claimed)return false;claimed=true;return true;},
    deferRetirement(i){trace.push("defer");return allow&&i===intent;},withdraw(){finalUiCurrent=false;trace.push("uiWithdraw");}};
  // Fixed protocol responses test downstream wiring only. No TS pricing, capacity,
  // Core attempt transitions, token allocation or renderer readiness is modeled.
  const dispatcher={observe(){trace.push("observe");},status(){trace.push("status");return{authorityRevision:proof.coreAuthorityRevision,canReserve:true,flight:null,lastPhase:null};},
    prepare(current,quantity){trace.push("prepare");const result={proof:Object.freeze({}),wire:{type:"buyItem",itemIndex:0,count:2,panelType:0}};
      prepared.push({current:clone(current),quantity,result});return result;},
    allows(p){trace.push("coreAllows");return prepared.some(x=>x.result.proof===p);},
    claim(p,wire,body,target,beforeEntry){trace.push("coreClaim");assert.equal(target,socket);assert.equal(body,JSON.stringify(wire));
      beforeLast?.();
      if(!prepared.some(x=>x.result.proof===p)||typeof beforeEntry!=="function"||!beforeEntry())return false;
      trace.push("entry");return true;},rejectIfUnentered(p){receipts.push(["definitelyUnsent",p]);},
    transportResult(p,outcome){receipts.push([outcome,p]);},withdrawAt(revision){trace.push("withdrawAt");assert.equal(revision,proof.coreAuthorityRevision);return withdrawResult;},
    withdraw(){trace.push("withdraw");}};
  const inventoryReadiness = new NpcGoldBuyInventoryReadiness();
  const inventoryOwner = { connectionGeneration:1, sessionGeneration:2, playerObjectId:1 };
  inventoryReadiness.finish(inventoryReadiness.begin(inventoryOwner), inventoryOwner, true, bagProjection(world).model);
  const scope={projectBevyBagModel:bagProjection,npcGoldBuyInventoryMutationPacket,
    playerReferenceWindowsRef:{current:{help:false,hotkeys:false,options:false,capture:false}},rankingInspectOpenRef:{current:false},
    npcPurchaseUnavailableRef:{current:false},npcPurchaseOptInSocketsRef:{current:new WeakSet()},
    npcPurchaseApplyingEconomyRef:{current:false},npcPurchaseEconomicSourceRef:{current:null},npcPurchaseDisplaySourceRef:{current:null},
    accountIdRef:{current:"finite-fixture-account"},socialCharacterIndexRef:{current:{requested:null,current:0}},
    npcPearlSendEpochRef:{current:0},npcPearlShopSourceRef:{current:new (loadPure("npcPearlSource").NpcPearlShopSource)()},
    bagBeltMovesRef:{current:new (loadPure("bagBeltMove").BagBeltMoveDispatcher)()},bagBeltInventoryReadyRef:{current:null},
    preauthCommandKind:npcAuthSelectors.preauthCommandKind,isSensitiveGatewayCommand:npcAuthSelectors.isSensitiveGatewayCommand,
    // These inactive repair, wallet, fishing and Bag notifications are external
    // ports. The actual NPC readiness, owner, claim and socket gates remain below.
    retireWorldFishingGesture:()=>{},closeNpcRepairService:()=>{},
    invalidateBagBeltInventory:()=>{},observeBagBeltInventorySnapshot:()=>{},observeNpcPearlWallet:()=>{},
    readNpcRepairOwner:()=>null,captureNpcRepairDialog:()=>{},npcRepairDialogBindingRef:{current:null},
    npcRepairAuthorityRef:{current:{prepareSnapshot:()=>{},invalidateInventory:()=>{},observeSnapshot:()=>{}}},
    observeBootstrapRef:{current:null},observePreferenceRef:{current:null},
    combatModePhysicalRef:{current:null},combatModeRawRef:{current:null},combatModeRevisionRef:{current:0},
    nextCombatModePhysicalGeneration:loadPure("modeKeys").nextCombatModePhysicalGeneration,
    npcGoldBuyInventoryRef:{current:inventoryReadiness},
    equipmentSnapshotRef:{current:{connectionGeneration:1,sessionGeneration:2}},
    normalizeQuestMapFileName:loadPure("questWorld").normalizeQuestMapFileName,
    questCapturedWorldRef:{current:null},questSceneRevisionRef:{current:1},questSkillsBaselineCurrentRef:{current:false},
    socialItemOperationsRef:{current:new (loadPure("operations").SocialWindowOperations)()},
    applyGatewayWorldSnapshot(snapshot, connectionGeneration) {
      scope.worldRef.current = { ...scope.worldRef.current, ...snapshot, playerObjectId:String(snapshot.playerObjectId) };
      scope.equipmentSnapshotRef.current = { connectionGeneration, sessionGeneration:scope.equipmentSessionGenerationRef.current };
    },
    heroAuthorityRef:{current:new (loadPure("hero").HeroPlayerAuthority)()}, heroOperationsRef:{current:new (loadPure("hero").HeroPlayerOperations)()},
    heroManagementOpenRef:{current:false}, heroManagementPage:null, cashShopOpenRef:{current:false}, mailCollectBarrierRef:{current:null},
    socialSceneRevisionRef:{current:1}, worldSnapshotVersionRef:{current:1}, renderParityServices:()=>{},
    worldRef:{current:world},npcShopServiceRef:{current:service},npcShopTabBindingRef:{current:null},
    npcBuySelectedRef:{current:null},npcBuyDispatcherRef:{current:dispatcher},npcShopUiIngressRef:{current:ui},
    npcShopClockRef:{current:{service:1,catalog:1}},setNpcShopService:v=>closed.push(v),setNpcShopTabBinding:v=>tabs.push(v),
    equipmentConnectionGenerationRef:{current:1},equipmentSessionGenerationRef:{current:2},equipmentBagOwnerRef:{current:{ownerRevision:0}},
    equipmentControllerRef:{current:{status:()=>({ready:true,pending:0})}},equipmentHostSuspendReasonRef:{current:null},equipmentStartGameRef:{current:{connectionGeneration:1,sessionGeneration:2}},
    sameBagOwner:loadPure("bag").sameBagOwner,bevyCharacterSendGateRef:{current:()=>false},bevyBagSendGateRef:{current:()=>false},
    pendingStorageRequestsRef:{current:new Map()},mailParcelRef:{current:null},screenRef:{current:"game"},socketRef:{current:socket},WebSocket:{OPEN:1},
    bevyQuestReactModalOpen:false,showQuestLog:false,showCharacter:false,showHeroPet:false,showGuild:false,showGroup:false,showFriends:false,
    showBonds:false,showRanking:false,showMarket:false,showConquest:false,showTrade:false,showBuffs:false,showMail:false,showWorldMap:false,
    showHelp:false,showHotkeys:false,showChatSettings:false,npcRepairService:null,bevyQuestUiRequested:true,bevyRuntimeBackend:"webgpu",
    webGl2SharedCanvasPrototype:false,initialSceneAssetsReadyRef:{current:true},language:"en",clientProfile:{input:"mouse"},
    document:{visibilityState:"visible",hasFocus:()=>true,querySelector:()=>null},
    readBevyQuestPresentation:()=>presentation,
    sharedCanvasUsesWebGl2:()=>false,sharedUiCanvasId:()=>"shared-ui",isSpectatorBrowserMode:()=>false,
    lastCommandRef:{current:null},isMovementPredictionBlockingCommand:()=>false,isMovementConsoleCommand:()=>false,
    isMovementCommand:()=>false,isCombatResolutionCommand:()=>false,recordDebugEvent:()=>{},appendLog:()=>{},t:key=>key,
    window:{dispatchEvent(event){assert.equal(event.type,"mir2:action");trace.push("action");listener?.(event);}},
    CustomEvent:class{constructor(type,options){this.type=type;this.detail=options.detail;}},
    combatIngressRef:{current:null},mailDispatcherRef:{current:null},mailIngressRef:{current:null},storageUiIngressRef:{current:null},
    mailMutationAllowed:loadPure("parcel").mailMutationAllowed,isMailItemMutation:loadPure("parcel").isMailItemMutation,
    storageMutationAllowed:loadPure("storage").storageMutationAllowed,
    console:{error:(...args)=>errors.push(args)},equipmentRenderOwnerToken:{connectionGeneration:1,sessionGeneration:2,ownerRevision:0},
    legacyNpcBuyAllowed:()=>assert.fail("ordinary shared UI cannot use legacy proof")};
  const api=actualFunctions(pageTree,pageNames,{...scope,
    applyGatewayWorldSnapshot:(...args)=>scope.applyGatewayWorldSnapshot(...args)});
  return{api,scope,world,service,proof,status,intent,socket,ui,dispatcher,trace,sent,receipts,prepared,tabs,closed,errors,
    onAction(fn){listener=fn;},get allows(){return allow;},set allows(v){allow=v;},set withdrawResult(v){withdrawResult=v;},set beforeLast(v){beforeLast=v;},set presentation(v){presentation=v;},throwSocket(){throws=true;}};
}

test("actual Page read model carries full player raw inventory and readonly canonical Core without observing in final checks",()=>{
  const f=pageFixture(),input=f.api.readNpcShopUiInput();assert.equal(input.eligible,true);assert.equal(input.showBuy,true);
  assert.equal(Object.keys(input.player).length,23);assert.equal(input.player.currentWeightKnown,false);
  assert.equal(input.shop.goods[0].unique_id,0);assert.equal(input.shop.goods[0].price,17);assert.equal(input.shop.goods[0].purchase_rate,0.9);
  assert.deepEqual(input.shop.goods[0].tooltip_source,f.service.buyItems[0].tooltipSource);assert.deepEqual(input.inventory,bagProjection(f.world).model);
  assert.equal(input.coreStatus.authorityRevision,"9007199254740993");assert.deepEqual(f.trace,["observe","status"]);
  f.trace.length=0;assert.equal(f.api.readNpcShopUiInput(false).coreStatus,null);assert.deepEqual(f.trace,[]);
  for(const id of [null,"0","01","9007199254740993","4294967296"]){f.world.playerObjectId=id;assert.equal(f.api.readNpcShopUiInput(false).playerObjectId,0);}
});

test("actual Page shared Buy uses common selected quantity and claims UI then exact Core body immediately before socket",()=>{
  const f=pageFixture();f.onAction(()=>assert.equal(f.trace.includes("entry"),false));
  assert.equal(f.api.dispatchBevyNpcShopIntent(f.intent),true);
  assert.equal(f.prepared.length,1);assert.equal(f.prepared[0].quantity,2);assert.equal(f.prepared[0].current.shop.selected_id,0);
  assert.deepEqual(f.sent,[f.intent.command]);assert.deepEqual(f.trace,["prepare","coreAllows","action","uiClaim","coreClaim","entry","socket"]);
  assert.deepEqual(f.receipts.map(x=>x[0]),["flushed","definitelyUnsent"]);assert.equal(f.api.dispatchBevyNpcShopIntent(f.intent),false);
});

test("actual Page final synchronous listener rejects changed service raw inventory owner layout status and socket before entry",()=>{
  for(const change of [f=>{f.scope.npcShopServiceRef.current={...f.service,catalogRevision:2};},f=>f.api.retireNpcShopService(),
    f=>{f.world.gold++;},f=>{f.service.buyItems[0].tooltipSource.info.index++;},f=>{f.scope.equipmentBagOwnerRef.current.ownerRevision++;},
    f=>{f.presentation=null;},f=>{f.status.controlRevision="9007199254740997";},
    f=>{f.scope.socketRef.current={readyState:1,send:()=>assert.fail("replacement socket")};},f=>{f.allows=false;}]){
    const f=pageFixture();
    f.onAction(()=>change(f));
    assert.equal(f.api.dispatchBevyNpcShopIntent(f.intent),false);assert.equal(f.sent.length,0);assert.equal(f.trace.includes("entry"),false);
    assert.equal(f.receipts.at(-1)[0],"definitelyUnsent");
  }
});

test("actual Page nonBuy two-phase local actions retire exact preentry before claim and never send a wire",()=>{
  for(const type of ["select","quantityInc","quantityDec","pageUp","pageDown"]){
    const f=pageFixture();f.intent.action=type==="select"?{type,uniqueId:0}:{type};f.intent.preentryWithdraw=true;f.intent.command=null;
    assert.equal(f.api.dispatchBevyNpcShopIntent(f.intent),true);assert.deepEqual(f.trace,["withdrawAt","uiClaim"]);
    assert.equal(f.prepared.length,0);assert.equal(f.sent.length,0);assert.equal(f.status.quantity,2);
  }
  const refused=pageFixture();refused.intent.action={type:"quantityInc"};refused.intent.preentryWithdraw=true;refused.intent.command=null;refused.withdrawResult=false;
  assert.equal(refused.api.dispatchBevyNpcShopIntent(refused.intent),false);assert.deepEqual(refused.trace,["withdrawAt"]);assert.equal(refused.sent.length,0);
});

test("actual Page Close and Sell defer retirement before source or compatibility tab mutation",()=>{
  const close=pageFixture();close.intent.action={type:"close"};close.intent.preentryWithdraw=true;close.intent.command=null;
  assert.equal(close.api.dispatchBevyNpcShopIntent(close.intent),true);assert.deepEqual(close.trace,["withdrawAt","defer","withdraw"]);
  assert.equal(close.scope.npcShopServiceRef.current,null);assert.deepEqual(close.closed,[null]);
  const sell=pageFixture();sell.intent.action={type:"handoffSell"};sell.intent.preentryWithdraw=true;sell.intent.command=null;
  assert.equal(sell.api.dispatchBevyNpcShopIntent(sell.intent),true);assert.deepEqual(sell.trace,["withdrawAt","defer"]);
  assert.equal(sell.api.currentNpcShopTab(),"sell");assert.equal(sell.tabs[0].service,sell.service);assert.equal(sell.sent.length,0);
});

test("actual Page entered socket exception reports Unknown plus exact unentered cleanup without inventing an ACK",()=>{
  const f=pageFixture();f.throwSocket();assert.equal(f.api.dispatchBevyNpcShopIntent(f.intent),false);
  assert.equal(f.trace.filter(x=>x==="entry").length,1);assert.equal(f.trace.filter(x=>x==="socket").length,1);assert.equal(f.sent.length,0);
  assert.deepEqual(f.receipts.map(x=>x[0]),["unknown","definitelyUnsent"]);assert.equal(f.errors.length,1);
  assert.equal(f.api.dispatchBevyNpcShopIntent(f.intent),false);
});


test("actual final Page checkpoint rejects deferred UI withdraw or stop after all Core callbacks despite unchanged status",()=>{
  for(const method of ["withdraw","stop"]){
    const f=pageFixture(),before=clone(f.status);f.beforeLast=()=>f.ui[method]();
    assert.equal(f.api.dispatchBevyNpcShopIntent(f.intent),false);assert.deepEqual(f.status,before);
    assert(f.trace.includes("uiClaim"));assert(f.trace.includes("coreClaim"));assert.equal(f.trace.includes("entry"),false);
    assert.equal(f.sent.length,0);assert.equal(f.receipts.at(-1)[0],"definitelyUnsent");
  }
});


test("actual Page hook and Shell props carry the same host without acquiring Bag ownership",()=>{
  const hooks=nodes(pageTree,n=>ts.isCallExpression(n)&&ts.isIdentifier(n.expression)&&n.expression.text==="useBevyNpcShopUi");
  assert.equal(hooks.length,1);assert.equal(hooks[0].arguments.length,1);const options=hooks[0].arguments[0];assert(ts.isObjectLiteralExpression(options));
  const properties=new Map(options.properties.filter(ts.isPropertyAssignment).map(p=>[p.name.getText(pageTree),p.initializer]));
  assert.equal(properties.get("read").getText(pageTree),"readNpcShopUiInput");
  assert.equal(properties.get("onIntent").getText(pageTree),"dispatchBevyNpcShopIntent");
  const owner=properties.get("onOwner");assert(ts.isArrowFunction(owner));assert.equal(owner.body.getText(pageTree),"undefined");
  const shells=nodes(pageTree,n=>(ts.isJsxSelfClosingElement(n)||ts.isJsxOpeningElement(n))&&n.tagName.getText(pageTree)==="OriginalClientShell");
  assert.equal(shells.length,1);const props=new Map(shells[0].attributes.properties.filter(ts.isJsxAttribute).map(p=>[p.name.text,p.initializer]));
  for(const [key,member] of [["bevyNpcShopUiActive","active"],["bevyNpcShopUiTransitioning","transitioning"],
    ["getBevyNpcShopInputBlocked","blocksInput"],["getBevyNpcShopPointerContext","pointerContext"],["onBevyNpcShopPointer","pointer"]]){
    const p=props.get(key);assert(ts.isJsxExpression(p)&&p.expression);assert.equal(p.expression.getText(pageTree),"bevyNpcShopUi."+member);
  }
  const change=props.get("onNpcShopTabChange").expression;assert(ts.isArrowFunction(change));
  const code=ts.transpileModule("const actualTab="+change.getText(pageTree)+";",{compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
  const f=pageFixture(),events=[];
  const tab=new Function("npcShopService","npcShopServiceRef","npcShopUiIngressRef","npcBuyDispatcherRef","setNpcShopCompatibilityTab",code+"\nreturn actualTab;")
    (f.service,f.scope.npcShopServiceRef,f.scope.npcShopUiIngressRef,{current:{withdraw:()=>events.push("withdraw")}},(service,value)=>events.push([service,value]));
  tab("sell");assert.deepEqual(events,[],"arming/active compatibility click cannot bypass shared host");
  f.scope.npcShopUiIngressRef.current=null;tab("sell");assert.deepEqual(events,["withdraw",[f.service,"sell"]]);
  f.scope.npcShopServiceRef.current={...f.service};tab("sell");
  assert.deepEqual(events,["withdraw",[f.service,"sell"]],"old render callback cannot act on a replacement service with identical contents");
});


test("actual shared Page denies naked legacy mixed DTO and stale current status before reservation",()=>{
  const f=pageFixture();assert.equal(f.api.sendRaw(f.intent.command),false);
  assert.equal(f.api.sendRaw(f.intent.command,{npcUi:{intent:f.intent}}),false);
  for(const mutate of [i=>{i.command.requestId="extra";},i=>{i.preentryWithdraw=true;},
    i=>{i.proof.catalogRevision++;},i=>{i.proof.controlRevision="18446744073709551615";}]){
    const g=pageFixture();mutate(g.intent);assert.equal(g.api.dispatchBevyNpcShopIntent(g.intent),false);
    assert.equal(g.prepared.length,0);assert.equal(g.trace.includes("action"),false);assert.equal(g.sent.length,0);
  }
  assert.deepEqual(f.trace,[]);assert.equal(f.prepared.length,0);assert.equal(f.sent.length,0);
  const mismatch=pageFixture();mismatch.intent.command.count=3;
  assert.equal(mismatch.api.dispatchBevyNpcShopIntent(mismatch.intent),false);assert.equal(mismatch.prepared.length,1);
  assert.equal(mismatch.sent.length,0);assert.equal(mismatch.receipts.at(-1)[0],"definitelyUnsent");
});


pureFiles.npc="../lib/bevy-npc-shop-ui.ts";pureRequires.npc={"./bevy-bag-ui":"bag"};
const {NpcShopPointerRouter}=loadPure("npc");
const shellNames=["npcShopBlocksWorldInput","stopNpcShopWorldInput","guardNpcShopGameplay","dispatchKeyboardMoveInput",
  "cancelWorldFishingHeldPointer","beginCombatUiHold","endCombatUiHold","cancelSharedNpcShopPointer","handleSharedNpcShopPointer","handleSharedQuestWorldPointer","handleSharedUiPointer","updateSceneCombatPointer"];
function shellFixture() {
  let blocked=true,context=null,behavior=()=>true,stops=0,focus=0,downstream=0;const edges=[],holds=[],combatPointers=[];
  const router=new NpcShopPointerRouter();
  const proof={runGeneration:1,connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:1,
    revision:1,modelRevision:1,presentationRevision:1,serviceRevision:1,catalogRevision:1,
    coreAuthorityRevision:"9007199254740993",controlRevision:"9007199254740995"};
  context={proof,presentation:{logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false},
    inputRegions:[{left:220,top:217,width:244,height:334},{left:474,top:251,width:330,height:80}]};
  class Element {constructor(id){this.id=id;}setPointerCapture(id){this.capture=id;}closest(){return null;}}
  const callbacks={getBevyNpcShopInputBlocked:()=>blocked,getBevyNpcShopPointerContext:()=>context,
    onBevyNpcShopPointer:edge=>{edges.push(edge);return behavior(edge);}};
  const scope={parityUiBlocksGameplay:undefined,onHeroShortcut:undefined,
    handleNpcRepairPointer:()=>false,handleBagBeltPointer:()=>false,worldFishingPhysicalRef:{current:null},
    npcShopPointerRouterRef:{current:router},npcShopPointerCallbacksRef:{current:callbacks},
    sceneHoveredObjectIdRef:{current:null},setSceneHoveredObjectId:()=>{},
    heldQuestControlPointersRef:{current:new Set()},screen:"game",bevyBagUiActive:false,
    bagPointerCallbacksRef:{current:{getBevyBagPointerContext:()=>null}},readBevyHudStatus:()=>null,
    readBevyQuestWorldControlBlockers:()=>null,readBevyQuestWorldControls:()=>null,
    questWorldControlAt:loadPure("questControls").questWorldControlAt,
    heldKeyboardMoveKeysRef:{current:new Set(["right"])},heldKeyboardRunModeRef:{current:true},heldScenePointerRef:{current:{button:0}},
    onViewportDirectionStop:()=>stops++,onCombatPointer:(...args)=>combatPointers.push(args),
    combatUiHoldRef:{current:new Map()},onCombatUiHeld:(...args)=>holds.push(args),
    stageFrameRef:{current:{focus:()=>focus++,dataset:{viewportSceneWidth:"1024",viewportSceneHeight:"768"}}},sceneInteractionReady:true,questLocalModalOpen:false,mobileMoreOpen:false,bevyQuestUiCapturesPointer:false,
    HTMLElement:Element,sharedUiCanvasId:()=>"shared-ui",webGl2SharedCanvasPrototype:false,
    scenePointFromMouseEvent:e=>({sceneX:e.clientX,sceneY:e.clientY}),
    bagPointerRouterRef:{current:{held:null}},storagePointerRouterRef:{current:{held:null}},characterPointerRouterRef:{current:{held:null}},
    hudPointerRouterRef:{current:{held:null}},spellsPointerRouterRef:{current:{held:null}},
    handleSharedComposePointer:()=>{downstream++;return false;},handleSharedMailPointer:()=>{downstream++;return false;},
    handleSharedStoragePointer:()=>{downstream++;return false;},handleSharedSpellsPointer:()=>{downstream++;return false;},handleSharedBagPointer:()=>{downstream++;},
    bevyStorageUiActive:false,bevyStorageUiTransitioning:false,storagePointerCallbacksRef:{current:{getBevyStoragePointerContext:()=>null}},
    bevyMailComposeReady:false,bevyMailComposePending:false,spellsPointerCallbacksRef:{current:{getBevySpellsPointerContext:()=>null}},
    latestMoveInputRef:{current:{screen:"login",renderPlayer:null,player:null}}};
  const api=actualFunctions(shellTree,shellNames,scope);
  function event(x=240,y=260,id=1){const e={target:new Element("shared-ui"),pointerId:id,pointerType:"mouse",button:0,
    clientX:x,clientY:y,prevented:0,stopped:0,preventDefault(){this.prevented++;},stopPropagation(){this.stopped++;}};return e;}
  return{api,scope,router,edges,holds,combatPointers,event,get blocked(){return blocked;},set blocked(v){blocked=v;},
    get context(){return context;},set context(v){context=v;},set behavior(v){behavior=v;},
    get stops(){return stops;},get focus(){return focus;},get downstream(){return downstream;}};
}

test("actual Shell arming blocks synchronously before pointer context and clears keyboard direction combat intent",()=>{
  const f=shellFixture();f.context=null;const e=f.event();f.api.handleSharedUiPointer(e,"down");
  assert(e.prevented>0&&e.stopped>0);assert.equal(f.downstream,0);assert.equal(f.edges.length,0);
  assert.equal(f.scope.heldKeyboardMoveKeysRef.current.size,0);assert.equal(f.scope.heldKeyboardRunModeRef.current,false);
  assert.equal(f.scope.heldScenePointerRef.current,null);assert.deepEqual(f.combatPointers.at(-1),[null,null]);
  f.scope.heldKeyboardMoveKeysRef.current.add("up");f.api.dispatchKeyboardMoveInput("edge");assert.equal(f.scope.heldKeyboardMoveKeysRef.current.size,0);
  const called=[];const guarded=f.api.guardNpcShopGameplay((...args)=>called.push(args));guarded("attack",3);assert.deepEqual(called,[]);
  f.blocked=false;guarded("attack",3);assert.deepEqual(called,[["attack",3]]);const pass=f.event();f.api.handleSharedUiPointer(pass,"down");assert.equal(f.downstream,5);
});

test("actual Shell routes shop origin with an exact combat hold and consumes world origin without a shop hold",()=>{
  const shop=shellFixture(),e=shop.event();shop.api.handleSharedUiPointer(e,"down");
  assert.equal(shop.edges[0].origin,"shop");assert.equal(shop.holds[0][0],"npcShop");assert.equal(shop.holds[0][2],true);
  const token=shop.holds[0][1];shop.api.handleSharedUiPointer(e,"up");
  assert.equal(shop.edges.at(-1).phase,"up");assert.equal(shop.router.held,null);assert.deepEqual(shop.holds.at(-1),["npcShop",token,false]);
  assert.equal(shop.downstream,0);assert.equal(shop.scope.heldScenePointerRef.current,null);
  const world=shellFixture(),outside=world.event(900,700);world.api.handleSharedUiPointer(outside,"down");
  world.api.handleSharedUiPointer(world.event(240,260),"move");world.api.handleSharedUiPointer(world.event(240,260),"up");
  assert.deepEqual(world.edges.map(x=>x.origin),["world","world","world"]);assert.equal(world.holds.length,0);assert.equal(world.downstream,0);
  assert.equal(world.scope.heldScenePointerRef.current,null);
});

test("actual Shell rejects wrong canvas invalid pointer and a competing panel without leaking to legacy gameplay",()=>{
  for(const alter of [f=>{f.scope.bagPointerRouterRef.current.held={};},f=>{f.scope.storagePointerRouterRef.current.held={};},
    (_f,e)=>{e.target.id="other";},(_f,e)=>{e.button=1;},(_f,e)=>{e.pointerType="touch";}]){
    const f=shellFixture(),e=f.event();alter(f,e);assert.equal(f.api.handleSharedNpcShopPointer(e,"down"),true);
    assert.equal(f.edges.length,0);assert.equal(f.holds.length,0);assert.equal(f.router.held,null);assert(e.prevented&&e.stopped);
  }
  const f=shellFixture(),e=f.event();f.api.handleSharedNpcShopPointer(e,"down");const lease=f.router.held;
  const wrong=f.event(240,260,2);f.api.handleSharedNpcShopPointer(wrong,"up");assert.equal(f.router.held,lease);
  assert.equal(f.edges.length,1);assert(wrong.prevented&&wrong.stopped);
});

test("actual Shell old nonce terminal cannot clear the new same-pointer lease or its combat hold",()=>{
  const f=shellFixture(),e=f.event();f.api.handleSharedNpcShopPointer(e,"down");const old=f.router.held;
  f.api.cancelSharedNpcShopPointer();f.api.handleSharedNpcShopPointer(e,"down");const fresh=f.router.held,hold=f.scope.combatUiHoldRef.current.get("npcShop");
  assert.notEqual(fresh.downSequence,old.downSequence);const before=f.holds.length;
  f.api.handleSharedNpcShopPointer(e,"up");assert.equal(f.edges.at(-1).downSequence,old.downSequence);
  assert.equal(f.router.held,fresh);assert.equal(f.scope.combatUiHoldRef.current.get("npcShop"),hold);assert.equal(f.holds.length,before);
  f.api.handleSharedNpcShopPointer(e,"up");assert.equal(f.router.held,null);assert.equal(f.scope.combatUiHoldRef.current.has("npcShop"),false);
});

test("actual Shell refused RPC cleans its own lease but synchronous replacement cannot be canceled by the old finally",()=>{
  const refused=shellFixture();refused.behavior=()=>false;refused.api.handleSharedNpcShopPointer(refused.event(),"down");
  assert.equal(refused.router.held,null);assert.equal(refused.scope.combatUiHoldRef.current.has("npcShop"),false);
  assert.deepEqual(refused.edges.map(x=>x.phase),["down","cancel"]);
  const f=shellFixture(),e=f.event();let once=false;
  f.behavior=edge=>{if(edge.phase==="down"&&!once){once=true;f.api.cancelSharedNpcShopPointer();f.api.handleSharedNpcShopPointer(e,"down");return false;}return true;};
  f.api.handleSharedNpcShopPointer(e,"down");assert(f.router.held);const fresh=f.router.held;
  assert.equal(f.scope.combatUiHoldRef.current.get("npcShop").token,f.holds.at(-1)[1]);
  assert.equal(f.edges.at(-1).downSequence,fresh.downSequence);assert.equal(f.edges.at(-1).phase,"down");
});


test("actual gamepad and touch props evaluate arming active and synchronous blocker without relying on a React update",()=>{
  const controls=nodes(shellTree,n=>(ts.isJsxSelfClosingElement(n)||ts.isJsxOpeningElement(n))
    &&["OriginalClientGamepadControls","OriginalClientMobileControls"].includes(n.tagName.getText(shellTree)));
  assert.equal(controls.length,2);
  for(const element of controls){
    const props=new Map(element.attributes.properties.filter(ts.isJsxAttribute).map(p=>[p.name.text,p.initializer?.expression]));
    for(const key of ["onDirectionIntent","onPrimaryTargetAction","onApproachTarget","onPickGroundDrop","onToggleInventory","onToggleCharacter","onCastSkill","onUseItem"]){
      const value=props.get(key);assert(ts.isCallExpression(value),key);assert.equal(value.expression.getText(shellTree),"guardNpcShopGameplay");assert.equal(value.arguments.length,1);
    }
    for(const key of ["enabled","gameplayReady"].filter(k=>props.has(k))){
      const expression=props.get(key).getText(shellTree);const code=ts.transpileModule("const result="+expression+";",{compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
      const evaluate=(active,arming,blocked)=>new Function("clientProfile","screen","sceneInteractionReady","bevyQuestUiCapturesPointer",
        "bevyBagUiActive","bevyStorageUiActive","bevyStorageUiTransitioning","bevyNpcShopUiActive","bevyNpcShopUiTransitioning","npcShopBlocksWorldInput",
        code+"\nreturn result;")({input:element.tagName.getText(shellTree)==="OriginalClientGamepadControls"?"gamepad":"touch",layout:"touch"},
          "game",true,false,false,false,false,active,arming,()=>blocked);
      assert.equal(evaluate(false,false,false),true,key+" preserves ordinary eligible input");
      assert.equal(evaluate(true,false,false),false,key+" active");assert.equal(evaluate(false,true,false),false,key+" arming");
      assert.equal(evaluate(false,false,true),false,key+" synchronous host gate before React state commits");
    }
  }
});


test("actual mobile menu close feedback always cleans up while opening remains blocked by the current NPC host",()=>{
  const mobile=nodes(shellTree,n=>(ts.isJsxSelfClosingElement(n)||ts.isJsxOpeningElement(n))&&n.tagName.getText(shellTree)==="OriginalClientMobileControls");
  assert.equal(mobile.length,1);const property=mobile[0].attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.text==="onSecondaryOpenChange");
  assert(property&&ts.isJsxExpression(property.initializer));const expression=property.initializer.expression;assert(ts.isArrowFunction(expression));
  const code=ts.transpileModule("const actual="+expression.getText(shellTree)+";",{compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
  let blocked=true;const writes=[];const callback=new Function("npcShopBlocksWorldInput","setMobileMoreOpen",code+"\nreturn actual;")(()=>blocked,v=>writes.push(v));
  callback(false);assert.deepEqual(writes,[false],"arming cannot trap an already open mobile menu");
  callback(true);assert.deepEqual(writes,[false]);blocked=false;callback(true);assert.deepEqual(writes,[false,true]);
  blocked=true;callback(false);assert.deepEqual(writes,[false,true,false]);
});


test("actual Shell refused current Up sends one exact cancel and the next manual Down is admitted",()=>{
  const f=shellFixture(),e=f.event();f.behavior=edge=>edge.phase!=="up";
  assert.equal(f.api.handleSharedNpcShopPointer(e,"down"),true);const down=f.edges[0];
  assert.equal(f.api.handleSharedNpcShopPointer(e,"up"),true);assert.deepEqual(f.edges.map(x=>x.phase),["down","up","cancel"]);
  const up=f.edges[1],cancel=f.edges[2];assert.equal(cancel.downSequence,down.downSequence);assert.equal(cancel.pointerId,down.pointerId);
  assert.equal(cancel.sequence,up.sequence+1);assert.equal(cancel.origin,"shop");assert.equal(cancel.button,down.button);
  assert.equal(f.router.held,null);assert.equal(f.scope.combatUiHoldRef.current.has("npcShop"),false);
  assert.equal(f.router.cancelTerminal(up),null,"the recovery was one time");
  f.api.handleSharedNpcShopPointer(e,"down");assert(f.router.held);assert.equal(f.edges.at(-1).phase,"down");
  assert.equal(f.scope.combatUiHoldRef.current.has("npcShop"),true);assert.equal(f.downstream,0);
});

test("actual Shell failed old terminal cannot cancel a synchronous new Down quarantine or new scene state",()=>{
  const f=shellFixture(),e=f.event();let nested=false;const scene={button:0,sceneX:900,sceneY:700,localMarker:"new RPC state"};
  f.api.handleSharedNpcShopPointer(e,"down");
  f.behavior=edge=>{if(edge.phase==="up"&&!nested){nested=true;f.api.handleSharedNpcShopPointer(e,"down");f.scope.heldScenePointerRef.current=scene;return false;}return true;};
  f.api.handleSharedNpcShopPointer(e,"up");const fresh=f.router.held,hold=f.scope.combatUiHoldRef.current.get("npcShop");assert(fresh&&hold);
  assert.deepEqual(f.edges.map(x=>x.phase),["down","up","down"]);assert.equal(f.scope.heldScenePointerRef.current,scene);
  assert.equal(f.router.cancelTerminal(f.edges[1]),null);assert.equal(f.router.held,fresh);assert.equal(f.scope.combatUiHoldRef.current.get("npcShop"),hold);
  const old=shellFixture();old.api.handleSharedNpcShopPointer(old.event(),"down");const retired=old.router.held;
  old.api.cancelSharedNpcShopPointer();old.api.handleSharedNpcShopPointer(old.event(),"down");const current=old.router.held,currentHold=old.scope.combatUiHoldRef.current.get("npcShop");
  old.behavior=edge=>edge.phase!=="up";const at=old.edges.length;old.api.handleSharedNpcShopPointer(old.event(),"up");
  assert.equal(old.edges.length,at+1,"failed quarantined terminal creates no recovery cancel");assert.equal(old.edges.at(-1).downSequence,retired.downSequence);
  assert.equal(old.router.held,current);assert.equal(old.scope.combatUiHoldRef.current.get("npcShop"),currentHold);
  assert.equal(old.router.cancelTerminal(old.edges.at(-1)),null);
});


test("actual compatibility Buy final Core callback refuses synchronous shared arming or changed closed socket before entry",()=>{
  for(const change of [f=>{f.armSharedHost();},f=>{f.scope.socketRef.current={readyState:1,send:()=>assert.fail("replacement socket cannot send")};},
    f=>{f.socket.readyState=0;}]){
    const f=pageFixture();let blocked=false;f.ui.blocksInput=()=>blocked;f.armSharedHost=()=>{blocked=true;};
    const prepared=f.dispatcher.prepare(f.api.readNpcGoldBuyCurrent(),2);f.trace.length=0;
    f.beforeLast=()=>change(f);
    assert.equal(f.api.sendRaw(prepared.wire,{npcBuyProof:prepared.proof}),false);
    assert.deepEqual(f.trace,["coreAllows","action","coreClaim"]);
    assert.equal(f.trace.includes("entry"),false);assert.equal(f.sent.length,0);assert.equal(f.receipts.length,0);
  }
  const positive=pageFixture();positive.ui.blocksInput=()=>false;
  const prepared=positive.dispatcher.prepare(positive.api.readNpcGoldBuyCurrent(),2);positive.trace.length=0;
  assert.equal(positive.api.sendRaw(prepared.wire,{npcBuyProof:prepared.proof}),true);
  assert.deepEqual(positive.trace,["coreAllows","action","coreClaim","entry","socket"]);
  assert.deepEqual(positive.sent,[{type:"buyItem",itemIndex:0,count:2,panelType:0}]);
  assert.deepEqual(positive.receipts.map(x=>x[0]),["flushed"]);
});

test("actual Page retires inventory availability before log-only gain, merge and extended item patch handlers", () => {
  for (const packet of ["GainedItem","MoveItem","MergeItem","RefreshItem","ItemUpgraded","StoreItemV2","MailLockedItem","LoseGold"]) {
    const f = pageFixture(); assert.equal(f.api.readNpcGoldBuyCurrent().blocked, false);
    const before = clone(f.world);
    f.api.invalidateNpcGoldBuyGatewayPacket({type:"packet",packet,payload:{result:0}});
    assert.equal(f.api.readNpcGoldBuyCurrent().blocked, true);
    assert.deepEqual(f.world, before, "retirement must not manufacture an inventory authority change");
    assert.deepEqual(f.trace, ["withdraw"]);
  }
  const f = pageFixture(); f.api.invalidateNpcGoldBuyGatewayPacket({type:"packet",packet:"ObjectWalk",payload:{}});
  assert.equal(f.api.readNpcGoldBuyCurrent().blocked, false); assert.deepEqual(f.trace, []);
});

test("actual Page restores only a successful full current-owner snapshot and refuses reentrant invalidation", () => {
  const snapshot = f => ({ ...clone(f.world), playerObjectId:1,
    npcGoldTradeCapacity:{rosterValid:true,freshCompatibleUniqueIds:[]} });
  const f=pageFixture(); f.api.invalidateNpcGoldBuyGatewayPacket({type:"packet",packet:"GainedItem"});
  f.api.applyNpcGoldBuyGatewaySnapshot(snapshot(f),1);
  assert.equal(f.api.readNpcGoldBuyCurrent().blocked,false);
  assert.deepEqual(f.api.readNpcGoldBuyCurrent().inventory.npcGoldTradeCapacity,snapshot(f).npcGoldTradeCapacity);
  for (const alter of [
    f=>{f.scope.applyGatewayWorldSnapshot=()=>{throw Error("incomplete snapshot");};},
    f=>{const apply=f.scope.applyGatewayWorldSnapshot;f.scope.applyGatewayWorldSnapshot=(...args)=>{apply(...args);f.api.invalidateNpcGoldBuyGatewayPacket({type:"packet",packet:"RefreshItem"});};},
    f=>{const apply=f.scope.applyGatewayWorldSnapshot;f.scope.applyGatewayWorldSnapshot=(...args)=>{apply(...args);f.scope.equipmentSessionGenerationRef.current=3;};},
    f=>{f.scope.applyGatewayWorldSnapshot=()=>{};},
    f=>{const apply=f.scope.applyGatewayWorldSnapshot;f.scope.applyGatewayWorldSnapshot=(...args)=>{apply(...args);f.scope.worldRef.current.npcGoldTradeCapacity={rosterValid:true,freshCompatibleUniqueIds:[999]};};}
  ]) {
    const x=pageFixture(); alter(x);
    try{x.api.applyNpcGoldBuyGatewaySnapshot(snapshot(x),1);}catch(error){assert.equal(error.message,"incomplete snapshot");}
    assert.equal(x.api.readNpcGoldBuyCurrent()?.blocked??true,true);
  }
});

test("actual Page full snapshot and every Native inventory invalidator revoke Web availability at ingress", () => {
  const native=source("../../game-client/platform-windows/src/gateway.rs");
  const start=native.indexOf('                "GainedGold" | "LoseGold" | "UserInformation"');
  assert(start>=0); const end=native.indexOf("=> {",start); assert(end>start);
  const packets=[...native.slice(start,end).matchAll(/"([A-Za-z0-9]+)"/g)].map(m=>m[1]);
  assert(packets.length>=45); for(const packet of packets)assert.equal(npcGoldBuyInventoryMutationPacket(packet),true,packet);
  const body=declaration(pageTree,"handleGatewayEvent").getText(pageTree);
  assert(body.indexOf("invalidateNpcGoldBuyGatewayPacket(event)")<body.indexOf("captureSpellsGatewayEvent(event,connectionGeneration)"));
  assert(body.includes("applyNpcPurchaseOrdinarySnapshot(event.payload as GatewayWorldSnapshot, rawWorldFrame, connectionGeneration, source)"));
  const ordinary=declaration(pageTree,"applyNpcPurchaseOrdinarySnapshot").getText(pageTree);
  const unqualified=ordinary.slice(ordinary.indexOf("if (!qualified) {"),ordinary.indexOf("const prior ="));
  assert(unqualified.includes("applyNpcGoldBuyGatewaySnapshot(snapshot, connectionGeneration);"));
  assert(unqualified.includes("return;"),"unqualified snapshot returns with original full-economy/display defaults");
  const f=pageFixture();f.api.invalidateNpcGoldBuyGatewayPacket({type:"worldSnapshot",payload:{}});
  assert.equal(f.api.readNpcGoldBuyCurrent().blocked,true);
});

test("NPC-only bag projection accepts attested cross-grid aliases and validates listed identity without weakening ordinary Bag", () => {
  const row=(id,container,slot)=>({key:"potion",name:"Potion",icon:10,description:"raw",
    authoritativeUniqueId:id,slot,container,quantity:18,tooltipSource:{info:{item_index:658,stack_size:20},userItem:{unique_id:id,count:18}}});
  const world={inventoryCapacity:46,maxBagSlots:40,gold:100,
    inventoryItems:[row(0,"bag1",0)],beltItems:[row(0,"belt",0)],equipmentItems:[],
    npcGoldTradeCapacity:{rosterValid:true,freshCompatibleUniqueIds:[]}};
  assert.equal(bagProjection(world).ok,false);
  const accepted=bagProjection(world,{npcGoldTrade:true});assert(accepted.ok);
  assert.deepEqual(accepted.model.npcGoldTradeCapacity,world.npcGoldTradeCapacity);
  for(const evidence of [
    {rosterValid:true,freshCompatibleUniqueIds:[0]},
    {rosterValid:true,freshCompatibleUniqueIds:[999]},
    {rosterValid:true,freshCompatibleUniqueIds:[1,1]},
    {rosterValid:false,freshCompatibleUniqueIds:[1]},
    {rosterValid:1,freshCompatibleUniqueIds:[]},
    {rosterValid:true}, {freshCompatibleUniqueIds:[]},
    {rosterValid:true,freshCompatibleUniqueIds:[],extra:1},
    {rosterValid:true,freshCompatibleUniqueIds:[9007199254740992]}
  ])assert.equal(bagProjection({...world,npcGoldTradeCapacity:evidence},{npcGoldTrade:true}).ok,false);
  assert.equal(bagProjection({...world,inventoryItems:[row(0,"bag1",0),row(0,"bag1",1)]},{npcGoldTrade:true}).ok,false);
  assert.equal(bagProjection({...world,npcGoldTradeCapacity:null},{npcGoldTrade:true}).ok,false);
  const unambiguous={...world,inventoryItems:[row(10,"bag1",0)],beltItems:[],
    npcGoldTradeCapacity:{rosterValid:true,freshCompatibleUniqueIds:[10]}};
  assert(bagProjection(unambiguous,{npcGoldTrade:true}).ok);
  assert(bagProjection({...unambiguous,npcGoldTradeCapacity:null},{npcGoldTrade:true}).ok);
  assert(bagProjection({...unambiguous,npcGoldTradeCapacity:{rosterValid:false,freshCompatibleUniqueIds:[]}},{npcGoldTrade:true}).ok);
});

test("a successful nested full snapshot cannot authorize an older outer inventory during synchronous publication", () => {
  const f=pageFixture(), apply=f.scope.applyGatewayWorldSnapshot;
  const outer={...clone(f.world),playerObjectId:1,
    npcGoldTradeCapacity:{rosterValid:true,freshCompatibleUniqueIds:[]}};
  const newer={...clone(outer),gold:200};
  let nested=false, checkedNew=false, checkedOld=false;
  f.scope.applyGatewayWorldSnapshot=(snapshot,connectionGeneration)=>{
    if(!nested){
      nested=true; f.api.applyNpcGoldBuyGatewaySnapshot(newer,connectionGeneration);
      assert.equal(f.api.readNpcGoldBuyCurrent().blocked,false,"new completed snapshot is ready");
      checkedNew=true; nested=false;
      apply(snapshot,connectionGeneration);
      assert.equal(f.api.readNpcGoldBuyCurrent().blocked,true,"old inventory fails the ready fingerprint before outer finally");
      checkedOld=true;
    }else apply(snapshot,connectionGeneration);
  };
  f.api.applyNpcGoldBuyGatewaySnapshot(outer,1);
  assert(checkedNew&&checkedOld);assert.equal(f.scope.worldRef.current.gold,100);
  assert.equal(f.api.readNpcGoldBuyCurrent().blocked,true);assert.equal(f.sent.length,0);
});


test("actual Page collected mail barrier needs same physical receiver and a strictly later valid full snapshot",()=>{
  const f=pageFixture(),owner=f.api.currentSocialReceiveOwner();assert.ok(owner);
  const snapshot={...f.scope.worldRef.current,playerObjectId:owner.playerObjectId,mapFileName:owner.mapFileName};
  const barrier={owner,mailId:1,observedSnapshot:1};f.scope.mailCollectBarrierRef.current=barrier;
  f.api.captureParitySnapshot(snapshot);assert.strictEqual(f.scope.mailCollectBarrierRef.current,barrier,"same version is not post-receipt inventory authority");
  f.scope.worldSnapshotVersionRef.current=2;f.scope.equipmentSnapshotRef.current=null;
  f.api.captureParitySnapshot(snapshot);assert.strictEqual(f.scope.mailCollectBarrierRef.current,barrier,"incomplete equipment layout cannot release custody");
  f.scope.equipmentSnapshotRef.current={connectionGeneration:owner.connectionGeneration,sessionGeneration:owner.sessionGeneration};
  f.scope.mailCollectBarrierRef.current={...barrier,owner:{...owner,socket:{}}};const oldPhysical=f.scope.mailCollectBarrierRef.current;
  f.api.captureParitySnapshot(snapshot);assert.strictEqual(f.scope.mailCollectBarrierRef.current,oldPhysical,"another physical sender cannot retire old custody");
  f.scope.mailCollectBarrierRef.current=barrier;f.api.captureParitySnapshot(snapshot);
  assert.equal(f.scope.mailCollectBarrierRef.current,null,"only the actual same-owner later full snapshot releases the confirmed collection");
});


test("actual ordinary unqualified snapshot delegates readiness while stale physical owners stay blocked",()=>{
  const snapshot=f=>({...clone(f.world),playerObjectId:1,npcGoldTradeCapacity:{rosterValid:true,freshCompatibleUniqueIds:[]}});
  const f=pageFixture(); f.api.invalidateNpcGoldBuyGatewayPacket({type:"worldSnapshot",payload:{}});
  assert.equal(f.api.readNpcGoldBuyCurrent().blocked,true);
  f.api.applyNpcPurchaseOrdinarySnapshot(snapshot(f),undefined,1,f.socket);
  assert.equal(f.api.readNpcGoldBuyCurrent().blocked,false);
  assert.deepEqual(f.api.readNpcGoldBuyCurrent().inventory.npcGoldTradeCapacity,snapshot(f).npcGoldTradeCapacity);
  assert.equal(f.scope.npcPurchaseDisplaySourceRef.current,null);
  for(const boundary of ["generation","socket","economy"]){
    const x=pageFixture(); x.api.invalidateNpcGoldBuyGatewayPacket({type:"worldSnapshot",payload:{}});
    const before=clone(x.scope.worldRef.current),trace=[...x.trace];
    if(boundary==="economy")x.scope.npcPurchaseApplyingEconomyRef.current=true;
    x.api.applyNpcPurchaseOrdinarySnapshot({...snapshot(x),gold:999},undefined,boundary==="generation"?2:1,boundary==="socket"?{}:x.socket);
    assert.equal(x.api.readNpcGoldBuyCurrent().blocked,true,boundary);
    assert.deepEqual(x.scope.worldRef.current,before,boundary);
    assert.deepEqual(x.trace,trace,boundary);
  }
});
