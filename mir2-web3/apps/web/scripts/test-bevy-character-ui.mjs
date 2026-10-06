import assert from "node:assert/strict";import test from "node:test";import {readFileSync,writeFileSync} from "node:fs";import ts from "typescript";
function load(url){const m={exports:{}};new Function("exports","module","require",ts.transpileModule(readFileSync(url,"utf8"),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText)(m.exports,m,p=>load(new URL(`${p}.ts`,url)));return m.exports;}
const ui=load(new URL("../lib/bevy-character-ui.ts",import.meta.url));
const {questWorldControlAt}=load(new URL("../lib/bevy-quest-world-controls.ts",import.meta.url));
const caps={schemaVersion:1,characterPageAbiVersion:1,characterEquipmentIntentAbiVersion:1,compiled:true,startup:true};
const item={uniqueId:0,key:"a",name:"a",quantity:1,container:2,slot:0,icon:1,description:"",stateImageX:0,stateImageY:0,stateImageWidth:0,stateImageHeight:0};
function fixture(){let sent=null,sink=null,frame=1,now=0,current=true,cleared=0;const outcomes=[];
 const runtime={getMir2CharacterUiCapabilities:()=>JSON.stringify(caps),setMir2CharacterUiSnapshot:json=>{sent=JSON.parse(json);return true;},getMir2CharacterUiStatus:()=>JSON.stringify({...sent,version:1,frame:frame++,ready:sent?.open===true,inputEnabled:sent?.inputEnabled===true,appliedRevision:sent?.revision??0,appliedModelRevision:sent?.modelRevision??0,appliedPresentationRevision:sent?.presentationRevision??0,inputRegions:[{left:760,top:94,width:264,height:286}],error:null}),setMir2CharacterUiIntentSink:s=>{sink=s;},clearMir2CharacterUiIntentSink:()=>{cleared++;sink=null;},setMir2CharacterUiPointerEdge:()=>true};
 const input={owner:{runGeneration:0,connectionGeneration:1,sessionGeneration:2,ownerRevision:0,owner:"react"},hudGeneration:3,open:true,eligible:true,model:{capacity:46,gold:0,items:[item]},player:{hp:1,maxHp:1,mp:1,maxMp:1,level:1,gold:0,credit:0,experience:0,maxExperience:1,currentWeight:0,currentWeightKnown:false,maxWeight:1},blockedUniqueIds:[],metadataVersion:null,presentation:{logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false}};
 const host=new ui.CharacterHost({runtime,isCurrent:()=>current,read:()=>input,now:()=>now,onState:()=>{},onIntent:(intent,proof)=>{outcomes.push([intent,proof]);return true;}});
 host.tick();host.tick();
 const intent=(changes={})=>({...sent,intentSequence:1,type:"removeEquipment",source:{container:2,slot:0,uniqueId:0},...changes});
 const emit=changes=>{const i=intent(changes);for(const k of ["revision","open","inputEnabled","presentation","player","model","blockedUniqueIds","metadataVersion"])delete i[k];return JSON.parse(sink(JSON.stringify(i)));};
 return {runtime,input,host,emit,outcomes,get sent(){return sent;},set current(v){current=v;},get cleared(){return cleared;}};
}
test("separate getter rejects missing/extra/old capability; touch fit rejects nonfinite and 600 screen",()=>{
 const f=fixture();assert.equal(ui.supportsCharacter(f.runtime),true);assert.equal(ui.supportsCharacter({}),false);
 assert.equal(ui.supportsCharacter({...f.runtime,getMir2CharacterUiCapabilities:()=>JSON.stringify({...caps,bagUiAbiVersion:1})}),false);
 for(const p of [{logicalWidth:NaN,logicalHeight:768,stageCssScale:1,touch:false},{logicalWidth:1024,logicalHeight:768,stageCssScale:600/1024,touch:true}])assert.equal(ui.fitsCharacter(p),false);
});
test("hidden Bag permits UID zero equipped intent and replay/old tuple is rejected",()=>{
 const f=fixture();assert.equal(f.emit().accepted,true);assert.equal(f.outcomes[0][1].owner,"react");assert.equal(f.emit().accepted,false);
 assert.equal(f.emit({intentSequence:2,source:{container:0,slot:0,uniqueId:0}}).accepted,false);
 assert.equal(f.emit({intentSequence:3,modelRevision:0}).accepted,false);
});
test("replacement UID/pending/model/owner invalidate send; own successful reserve permits exact final proof only",()=>{
 const f=fixture();f.emit();const proof=f.outcomes[0][1];assert.equal(f.host.allows(proof),true);
 f.input.blockedUniqueIds=[0];assert.equal(f.host.allows(proof),false);assert.equal(f.host.allows(proof,0),true);
 f.input.blockedUniqueIds=[0,9];assert.equal(f.host.allows(proof,0),false);
 f.input.blockedUniqueIds=[];f.input.model={...f.input.model,items:[{...item,uniqueId:1}]};assert.equal(f.host.allows(proof),false);
 f.input.model={...f.input.model,items:[item]};f.input.owner={...f.input.owner,sessionGeneration:4};assert.equal(f.host.allows(proof),false);
});
test("runtimeRef retirement withdraws captured runtime and rejects old sink synchronously",()=>{
 const f=fixture();f.current=false;assert.equal(f.emit().accepted,false);assert.equal(f.sent.open,false);f.host.tick();assert.equal(f.cleared,1);
});
test("constructor sink throw retains compatibility and clears captured surface",()=>{
 let cleared=0;const f=fixture();assert.doesNotThrow(()=>new ui.CharacterHost({runtime:{...f.runtime,setMir2CharacterUiIntentSink:()=>{throw Error("sink");},clearMir2CharacterUiIntentSink:()=>{cleared++;}},read:()=>f.input,now:()=>0,onState:()=>{},onIntent:()=>true}));assert.equal(cleared,1);
});
test("pointer capture requires matching revision; crossing cancels and another pointer cannot release",()=>{
 const f=fixture(),router=new ui.CharacterPointerRouter(),c=f.host.pointerContext();assert.ok(c);
 assert.equal(router.down(c,7,0,900,110).phase,"down");assert.equal(router.edge("up",8,900,110),null);
 assert.equal(router.matches({...c,modelRevision:c.modelRevision+1}),false);assert.equal(router.edge("move",7,700,110).phase,"cancel");assert.equal(router.held,null);
 assert.equal(router.down(c,7,0,900,110).phase,"down");assert.equal(router.edge("up",7,900,110).phase,"up");assert.equal(router.held,null);
});
test("actual Host context to Router to runtime setter emits only the strict Rust pointer DTO",()=>{
 const f=fixture(),router=new ui.CharacterPointerRouter(),context=f.host.pointerContext();assert.ok(context);
 const identity=["runGeneration","connectionGeneration","sessionGeneration","ownerRevision","ledgerRunGeneration","hudGeneration","modelRevision","presentationRevision"];
 assert.deepEqual(Object.keys(context).sort(),[...identity,"presentation","inputRegions"].sort());
 let captured=null;f.runtime.setMir2CharacterUiPointerEdge=json=>{captured=json;return true;};
 assert.equal(f.host.pointer(router.down(context,7,0,900,110)),true);
 const wire=JSON.parse(captured);assert.deepEqual(Object.keys(wire).sort(),[...identity,"sequence","pointerId","phase","x","y","button"].sort());
 assert.equal(wire.phase,"down");assert.equal(wire.pointerId,7);assert.equal(wire.x,900);assert.equal(wire.y,110);
 if(process.env.MIR2_M9_POINTER_FIXTURE)writeFileSync(process.env.MIR2_M9_POINTER_FIXTURE,captured,"utf8");
});


test("HMR late old cleanup cannot replace new pending snapshot or clear new sink",()=>{
 const f=fixture();const old=f.host;let acceptedSink=null;let pending=null;
 f.runtime.setMir2CharacterUiIntentSink=s=>{acceptedSink=s;};
 f.runtime.setMir2CharacterUiSnapshot=json=>{pending=JSON.parse(json);return true;};
 const next=new ui.CharacterHost({runtime:f.runtime,read:()=>f.input,now:()=>0,onState:()=>{},onIntent:()=>true});
 next.tick();const newSink=acceptedSink,newRun=pending.runGeneration;
 assert.equal(next.pointerContext(),null,"getter still reports old applied frame until actual ingest");
 old.stop();assert.equal(acceptedSink,newSink);assert.equal(pending.runGeneration,newRun);assert.equal(pending.open,true);assert.equal(f.cleared,0);
});
test("actual shell gives foreground Bag/modal priority and consumes Character clicks before HUD/world",()=>{
 const source=readFileSync(new URL("../app/original-client-shell.tsx",import.meta.url),"utf8");
 const ast=ts.createSourceFile("shell.tsx",source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),names=["cancelSharedCharacterPointer","handleSharedCharacterPointer","handleSharedBagPointer","handleSharedQuestWorldPointer"],decl=[];
 function visit(node){if(ts.isFunctionDeclaration(node)&&node.name&&names.includes(node.name.text))decl.push(node.getText(ast));ts.forEachChild(node,visit);}visit(ast);assert.equal(decl.length,names.length);
 const f=fixture(),context=f.host.pointerContext(),edges=[];let hudCalls=0,stops=0,bag=null;
 class Element {constructor(){this.id="canvas";}setPointerCapture(){}}
 const scope={parityUiBlocksGameplay:undefined,onHeroShortcut:undefined,characterPointerRouterRef:{current:new ui.CharacterPointerRouter()},characterPointerCallbacksRef:{current:{getBevyCharacterPointerContext:()=>context,onBevyCharacterPointer:e=>{edges.push(e);return true;}}},
  bagPointerCallbacksRef:{current:{getBevyBagPointerContext:()=>bag}},bagPointerRouterRef:{current:{held:null}},hudPointerRouterRef:{current:{held:null}},
  stageFrameRef:{current:{dataset:{viewportSceneWidth:"1024",viewportSceneHeight:"768"},focus(){}}},heldScenePointerRef:{current:null},onViewportDirectionStop:()=>{stops++;},
  heldQuestControlPointersRef:{current:new Set()},screen:"game",bevyBagUiActive:true,
  readBevyQuestWorldControls:()=>null,readBevyQuestWorldControlBlockers:()=>null,questWorldControlAt,readBevyHudStatus:()=>null,
  sceneInteractionReady:true,questLocalModalOpen:false,mobileMoreOpen:false,bevyQuestUiCapturesPointer:false,bevyCharacterPageReady:true,
  HTMLElement:Element,sharedUiCanvasId:()=>"canvas",webGl2SharedCanvasPrototype:false,
  scenePointFromMouseEvent:e=>({sceneX:e.clientX,sceneY:e.clientY}),handleSharedHudPointer:()=>{hudCalls++;return true;}};
 const keys=Object.keys(scope),code=ts.transpileModule(decl.join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
 const api=new Function(...keys,`${code};return {handleSharedBagPointer,cancelSharedCharacterPointer};`)(...keys.map(k=>scope[k]));
 const event={target:new Element(),pointerId:7,pointerType:"mouse",button:0,clientX:900,clientY:110,preventDefault(){}};
 api.handleSharedBagPointer(event,"down");api.handleSharedBagPointer(event,"up");assert.equal(edges.length,2);assert.equal(hudCalls,0);assert.ok(stops>=2);
 bag={inputRegions:context.inputRegions};api.handleSharedBagPointer(event,"down");assert.equal(hudCalls,1);assert.equal(edges.length,2);
 bag=null;api.handleSharedBagPointer(event,"down");api.handleSharedBagPointer({...event,pointerId:8},"down");assert.equal(edges.at(-1).phase,"cancel");assert.equal(hudCalls,1);
});
test("fixed stage UI canvas keeps Character visible and interactive for GPU and shared GL2",()=>{
 const text=readFileSync(new URL("../app/original-client-shell.tsx",import.meta.url),"utf8"),ast=ts.createSourceFile("shell.tsx",text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),canvases=[];
 function visit(n){if(ts.isJsxSelfClosingElement(n)&&n.tagName.getText(ast)==="canvas"){
   const id=n.attributes.properties.find(a=>ts.isJsxAttribute(a)&&a.name.getText(ast)==="id");
   if(id?.initializer&&["mir2-web3-canvas","mir2-quest-ui-canvas"].includes(id.initializer.text))canvases.push(n);
 }ts.forEachChild(n,visit);}visit(ast);assert.equal(canvases.length,2);
 const world=canvases.find(n=>n.attributes.properties.some(a=>a.name?.getText(ast)==="id"&&a.initializer?.text==="mir2-web3-canvas"));
 const stage=canvases.find(n=>n.attributes.properties.some(a=>a.name?.getText(ast)==="id"&&a.initializer?.text==="mir2-quest-ui-canvas"));
 assert.ok(world&&stage);assert.ok(ts.isJsxElement(world.parent));
 assert.match(world.parent.openingElement.getText(ast),/game-world-composite/);
 assert.equal(world.attributes.properties.some(a=>a.name?.getText(ast)==="data-ui-interactive"),false);
 assert.equal(stage.attributes.properties.some(a=>a.name?.getText(ast)==="data-ui-interactive"),true);
 for(const webGl2SharedCanvasPrototype of [false,true]){
   const scope={screen:"game",webGl2SharedCanvasPrototype,bevyCharacterPageReady:true,bevyMailPageReady:false,bevySpellsPageReady:false,bevyQuestUiCapturesPointer:false,bevyQuestWorldUiReady:false,bevyQuestUiReady:false,bevyBagUiActive:false,bevyHudUiReady:false,bevyNpcShopUiActive:false,bevyNpcShopUiTransitioning:false,bevyStorageUiActive:false,bevyStorageUiTransitioning:false,hpOrbOwner:false,mpOrbOwner:false,experienceBarOwner:false,weightBarOwner:false};
   const value=name=>{const attribute=stage.attributes.properties.find(a=>a.name?.getText(ast)===name);return new Function(...Object.keys(scope),`return (${attribute.initializer.expression.getText(ast)});`)(...Object.values(scope));};
   assert.match(value("className"),/shared-quest-ui-visible/);assert.equal(value("data-ui-interactive"),"true");assert.equal(value("style").pointerEvents,"auto");
 }
 const page=readFileSync(new URL("../app/page.tsx",import.meta.url),"utf8");
 assert.match(page,/eligible:[\s\S]*?&& bevyHudUi\.readCurrent\(\)\?\.ready === true/,"Character chrome navigation requires actual current HUD qualification");
});
