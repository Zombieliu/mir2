// Actual modules + controlled runtime/Page transport dependencies. No browser or server.
import assert from 'node:assert/strict';import test from 'node:test';import {readFileSync,writeFileSync,mkdirSync}from'node:fs';import{resolve,isAbsolute}from'node:path';import ts from'typescript';
import{pathToFileURL}from'node:url';
import{createHash}from'node:crypto';
// Mandatory exact-input compiled port. No JavaScript SendSlot algorithm exists
// in this producer. Root builds the candidate and supplies its binding path.
const sendSlotSessions=[];
const args=process.argv.slice(2),cli=new Map();if(args.length%2)throw Error('Expected named absolute QA paths');
for(let i=0;i<args.length;i+=2){if(!['--fixture-dir','--send-slot-module','--send-slot-wasm'].includes(args[i])||cli.has(args[i])||!isAbsolute(args[i+1]))throw Error('Invalid/duplicate/nonabsolute QA path argument');cli.set(args[i],resolve(args[i+1]));}
const sendSlotModulePath=cli.get('--send-slot-module'),sendSlotWasmPath=cli.get('--send-slot-wasm');
if(!sendSlotModulePath||!sendSlotWasmPath)throw Error('Root must supply --send-slot-module and --send-slot-wasm absolute QA paths for compiled SendSlot evidence');
const fixtureDir=cli.get('--fixture-dir')??null;if(fixtureDir)mkdirSync(fixtureDir,{recursive:true});
const compiledSendSlot=await import(pathToFileURL(sendSlotModulePath).href);
await compiledSendSlot.default({module_or_path:readFileSync(sendSlotWasmPath)});
assert.equal(compiledSendSlot.mail_send_slot_abi_version?.(),1);assert.equal(typeof compiledSendSlot.MailSendSlotBridge,'function');
function recordedSendSlotModule(){return{mail_send_slot_abi_version:()=>compiledSendSlot.mail_send_slot_abi_version(),MailSendSlotBridge:class{
 constructor(){this.bridge=new compiledSendSlot.MailSendSlotBridge();this.rows=[];sendSlotSessions.push(this.rows);}
 transact(inputJson){const outputJson=this.bridge.transact(inputJson);this.rows.push({inputJson,outputJson,input:JSON.parse(inputJson),output:JSON.parse(outputJson)});return outputJson;}
 draft_gold(){return this.bridge.draft_gold?.();}
}};}
function actualSendSlotFactory(module,cached=false){
 const source=ts.createSourceFile('client-core-runtime.ts',readFileSync(new URL('../lib/client-core-runtime.ts',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
 const names=['slotUtf8','slotFields','slotDecimal','slotStream','slotOwner','slotRaw','slotPayload','slotFlight','mailSendSlotRequest','mailSendSlotResult','createMailSendSlotRuntime'],found=new Map();let getter;
 function visit(n){if(ts.isFunctionDeclaration(n)&&names.includes(n.name?.text))found.set(n.name.text,n.getText(source));if(ts.isMethodDeclaration(n)&&n.name?.getText(source)==='getMailSendSlot')getter=n.getText(source);ts.forEachChild(n,visit);}visit(source);assert.equal(found.size,names.length);assert.ok(getter);
 const code=ts.transpileModule(names.map(n=>found.get(n)).join('\n')+`\nlet mailSendSlot=null;const runtime={${getter}};`,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 return new Function('module',code+';return '+(cached?'runtime':'createMailSendSlotRuntime(module)')+';')(module);
}
function mailDispatcher(read,module=recordedSendSlotModule()){
 const slot=actualSendSlotFactory(module),d=new ui.MailDispatcher(read,()=>slot),socket={readyState:1};d.mailTestSocket=socket;d.mailTestSlot=slot;
 d.composer.observeOpen(socket,socket,true);return d;
}
function mailSender(){const slot=actualSendSlotFactory(recordedSendSlotModule()),sender=new ui.MailComposeSender(()=>slot),socket={readyState:1};sender.mailTestSocket=socket;sender.observeOpen(socket,socket,true);return sender;}
function load(url,map={}){const m={exports:{}},code=ts.transpileModule(readFileSync(url,'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 new Function('exports','module','require',code)(m.exports,m,name=>{if(!(name in map))throw Error(`unexpected import ${name}`);return map[name];});return m.exports;}
const packets=load(new URL('../lib/extended-server-packets.ts',import.meta.url)),ui=load(new URL('../lib/bevy-mail-ui.ts',import.meta.url),{'./extended-server-packets':packets});
const textInput=load(new URL('../lib/bevy-mail-text-input.ts',import.meta.url));
const itemIdentity=load(new URL('../lib/world-model/item-identity.ts',import.meta.url)),equipmentAdapter=load(new URL('../lib/equipment-gateway-adapter.ts',import.meta.url),{'./world-model/item-identity':itemIdentity}),parcelAdapter=load(new URL('../lib/mail-parcel-gateway-adapter.ts',import.meta.url),{'./equipment-gateway-adapter':equipmentAdapter});
const wire='{"type":"packet","packet":"ReceiveMail","payload":{"mail":[{"mailId":71,"senderName":"Sender","message":"本文\\r\\nSecond","opened":false,"locked":false,"canReply":true,"collected":false,"gold":100,"dateSentBinaryDatetime":638962902000000001,"items":[{"unique_id":77,"item_index":1,"count":2,"current_dura":3,"max_dura":4,"soul_bound_id":-1,"identified":true,"cursed":false,"gem_count":1}]}]}}';
function actualMail(){return packets.parseMailList(packets.parseGatewayMailDates(wire).payload.mail);}
// Controlled resolver PORT, never a JavaScript catalogue. Root verifies every
// exported raw/returned DTO pair with the actual Rust/WASM resolver.
const resolverCaptures=[];
function resolverPort(json){const rows=JSON.parse(json);for(const row of rows)for(const item of row.items){
 if(item.uniqueId===null)continue;
 const key=item.key?.startsWith('crystal-item-')?Number(item.key.slice(13)):null;
 if(item.key&&key===null||key!==null&&item.itemIndex!==null&&key!==item.itemIndex)return null;
 const index=key??item.itemIndex;if(index!==1&&index!==120)return null;
 item.itemIndex=index;item.key=`crystal-item-${index}`;if(item.identified===null)item.identified=index===1;
 }const output=JSON.stringify(rows);resolverCaptures.push({input:JSON.parse(json),output:JSON.parse(output)});return output;}
function refreshSource(index=1,identified=null){return [{id:71,from:'Sender',subject:actualMail()[0].message,body:'',gold:100,items:[],itemStatesJson:[JSON.stringify({unique_id:77,name:index===1?'SpiritBlade':'MysteryHelmet',key:`crystal-item-${index}`,quantity:2,durability_current:3,durability_max:4,soul_bound_id:null,gem_count:1,identified,cursed:false,user_item_metadata:null})],opened:true,locked:false,claimed:false,deleted:false}];}
function refreshMail(){return packets.mergeMailList(refreshSource(),actualMail(),resolverPort);}
function resolutionFixtures(){const cases=[];
 for(const [name,oldIndex,oldFlag,index,flag]of[['default-spirit',1,false,1,null],['default-mystery',120,true,120,null],['template-change',1,true,120,null],['explicit-spirit',1,true,1,false],['explicit-mystery',120,false,120,true],['cosmetic-same-template',1,true,1,null]]){
  const previous=actualMail();previous[0].items[0].itemIndex=oldIndex;previous[0].items[0].identified=oldFlag;
  const source=refreshSource(index,flag),current=packets.parseMailList(source),resolved=JSON.parse(resolverPort(JSON.stringify(current))),merged=packets.mergeMailList(source,previous,resolverPort);
  cases.push({name,previous,current,resolved,merged});
 }
 return cases;
}

const caps={schemaVersion:1,mailPageAbiVersion:1,mailIntentAbiVersion:1,compiled:true,startup:true};
function fixture(){let sent=null,sink=null,frame=1,now=10,current=true,clearCount=0;const calls=[],captures=[],edges=[];
 const owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3};
 const input={owner,mailboxOwner:{...owner},sceneRevision:4,hudGeneration:5,open:true,rustOwns:false,eligible:true,mail:packets.mergeMailList(packets.parseGatewayMailDates(wire).payload.mail,null,resolverPort),presentation:{logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false}};
 const runtime={resolveMir2MailUiRows:resolverPort,getMir2MailUiCapabilities:()=>JSON.stringify(caps),setMir2MailUiSnapshot:json=>{sent=JSON.parse(json);captures.push(json);return true;},
 getMir2MailUiStatus:()=>JSON.stringify({run:sent?.run??0,...owner,sceneRevision:sent?.sceneRevision??4,hudGeneration:sent?.hudGeneration??5,version:1,frame:frame++,ready:sent?.open===true,inputEnabled:sent?.inputEnabled===true,modal:false,renderRevision:7,appliedRevision:sent?.revision??0,appliedModelRevision:sent?.modelRevision??0,appliedPresentationRevision:sent?.presentationRevision??0,inputRegions:sent?.inputEnabled?[{left:712,top:70,width:312,height:444}]:[],error:null}),
 setMir2MailUiIntentSink:f=>sink=f,clearMir2MailUiIntentSink:()=>{++clearCount;sink=null;},setMir2MailUiPointerEdge:json=>{edges.push(json);return true;}};
 let onIntent=i=>{calls.push(i);return'confirmedSend';};const host=new ui.MailHost({runtime,isCurrent:()=>current,read:()=>input,now:()=>now,onState:()=>{},onIntent:i=>onIntent(i)});
 host.tick();input.rustOwns=true;host.tick();host.tick();
 const intent=(overrides={})=>({run:host.run,...input.owner,sceneRevision:input.sceneRevision,hudGeneration:input.hudGeneration,sequence:1,modelRevision:sent.modelRevision,presentationRevision:sent.presentationRevision,renderRevision:7,action:'read',mailId:71,lock:null,...overrides});
 return{host,input,runtime,calls,captures,edges,intent,emit:(overrides={})=>JSON.parse(sink(JSON.stringify(intent(overrides)))),get sent(){return sent;},get sink(){return sink;},get clearCount(){return clearCount;},set now(v){now=v;},set current(v){current=v;},set onIntent(v){onIntent=v;}};
}
test('ReceiveMail raw date lexeme, real UserItem and legacy Stage5 share one loss-aware mailbox',()=>{
 const mail=actualMail();assert.equal(mail[0].dateSentBinaryDatetime,'638962902000000001');assert.equal(mail[0].metadataKnown,true);assert.equal(mail[0].items[0].uniqueId,77);assert.equal(mail[0].items[0].itemIndex,1);assert.equal(mail[0].items[0].currentDura,3);
 const ordinary=packets.parseMailList(JSON.parse(wire).payload.mail);assert.equal(ordinary[0].dateSentBinaryDatetime,null);assert.equal(ordinary[0].metadataKnown,false);
 const nativeParse=JSON.parse;try{JSON.parse=(text,reviver)=>nativeParse(text,reviver?function(k,v){return reviver.call(this,k,v);}:undefined);const unsupported=packets.parseMailList(packets.parseGatewayMailDates(wire).payload.mail);assert.equal(unsupported[0].dateSentBinaryDatetime,null);assert.equal(unsupported[0].metadataKnown,false);}finally{JSON.parse=nativeParse;}
 assert.equal(packets.parseMailList([{mailId:1,canReply:true,dateSentBinaryDatetime:'9223372036854775808'}])[0].metadataKnown,false);
 const legacy=packets.parseMailList([{id:72,from:'Legacy',body:'Body',read:true,claimed:true,items:['Wooden Sword']}]);assert.equal(legacy[0].opened,true);assert.equal(legacy[0].collected,true);assert.equal(legacy[0].items[0].name,'Wooden Sword');assert.equal(legacy[0].metadataKnown,false);
 const other='{"type":"packet","packet":"Other","payload":{"mailId":1,"dateSentBinaryDatetime":638962902000000001}}';assert.deepEqual(packets.parseGatewayMailDates(other),JSON.parse(other));
 assert.equal(packets.parseMailList([{mailId:1,items:[{unique_id:9007199254740992,item_index:1}]}]),null);assert.equal(packets.parseMailList([{mailId:1},{mailId:1}]),null);
});
test('actual Host setter and router export strict Rust fixtures after hidden prewarm',()=>{const f=fixture();assert.equal(f.captures.length>=2,true);assert.equal(JSON.parse(f.captures[0]).inputEnabled,false);assert.equal(f.sent.inputEnabled,true);const c=f.host.pointerContext();assert.ok(c);const router=new ui.MailPointerRouter();assert.equal(f.host.pointer(router.down(c,1,750,130)),true);assert.equal(f.edges.length,1);
 if(fixtureDir){writeFileSync(resolve(fixtureDir,'mail-wire.json'),wire);writeFileSync(resolve(fixtureDir,'mail-snapshot.json'),f.captures.at(-1));writeFileSync(resolve(fixtureDir,'mail-pointer.json'),f.edges[0]);writeFileSync(resolve(fixtureDir,'mail-legacy-normalized.json'),JSON.stringify({legacy:[...packets.parseMailList([{id:72,from:'Legacy',body:'Body',read:true,claimed:true,items:['Wooden Sword']},{id:73,from:'Sender',body:'Parcel',gold:0,items:[],itemStatesJson:[JSON.stringify({unique_id:77,name:'SpiritBlade',key:'crystal-item-1',quantity:2,durability_current:3,durability_max:4,user_item_metadata:{item_index:1}})]}]),...refreshMail()],resolutionCases:resolutionFixtures(),resolverCaptures}));}
});
test('one-use current intent proof, exact owner/model/scene/render, no wire IDs and no optimistic read',()=>{const f=fixture();f.onIntent=i=>{assert.equal(f.host.allows(i),true);assert.equal(f.host.claim(i),true);assert.equal(f.host.claim(i),false);f.calls.push(i);return'confirmedSend';};assert.equal(f.emit().outcome,'confirmedSend');assert.equal(f.emit().outcome,'definitelyUnsent');assert.equal(f.calls.length,1);assert.equal(f.sent.mail[0].opened,false);assert.equal(f.sent.mail[0].collected,false);assert.equal(Object.hasOwn(f.calls[0],'requestId'),false);
 for(const field of['run','connectionGeneration','sessionGeneration','ownerRevision','playerObjectId','sceneRevision','modelRevision','presentationRevision','renderRevision'])assert.equal(f.emit({sequence:2,[field]:999}).outcome,'definitelyUnsent');
});
test('synchronous cancel rejects proof, fresh edge survives, retained pointer crossing cannot replay',()=>{const f=fixture();f.onIntent=i=>{f.host.withdraw();assert.equal(f.host.allows(i),false);assert.equal(f.host.claim(i),false);return'definitelyUnsent';};assert.equal(f.emit().outcome,'definitelyUnsent');f.host.tick();f.host.tick();f.host.tick();f.onIntent=()=> 'confirmedSend';assert.equal(f.emit({sequence:1}).outcome,'definitelyUnsent');assert.equal(f.emit({sequence:2}).outcome,'confirmedSend');
 const c=f.host.pointerContext(),router=new ui.MailPointerRouter();assert.ok(router.down(c,1,750,130));assert.equal(router.edge('up',2,750,130),null);assert.equal(router.edge('move',1,0,0).phase,'cancel');assert.equal(router.edge('up',1,750,130),null);
});
test('bad/current model withdraw, scene close and old-owner data cannot regain authority through timer',()=>{const f=fixture();f.input.mail=null;f.host.tick();assert.equal(f.host.pointerContext(),null);f.now=60;f.host.tick();assert.equal(f.sent.open,false);f.input.mail=actualMail();f.host.tick();f.host.tick();f.host.tick();assert.ok(f.host.pointerContext());f.input.owner={...f.input.owner,sessionGeneration:9};f.host.tick();assert.equal(f.host.pointerContext(),null);assert.equal(f.sent.open,false);});
test('HMR replacement burns run and old cleanup cannot clear replacement sink',()=>{const f=fixture(),old=f.host,newHost=new ui.MailHost({runtime:f.runtime,isCurrent:()=>true,read:()=>f.input,now:()=>10,onState:()=>{},onIntent:()=> 'confirmedSend'});newHost.tick();newHost.tick();const sink=f.sink;old.stop();assert.equal(f.sink,sink);assert.equal(f.clearCount,0);assert.ok(newHost.run>old.run);assert.equal(JSON.parse(sink(JSON.stringify(f.intent({run:old.run})))).outcome,'definitelyUnsent');});
test('fallback capability objects remain independent and tiny touch layout stays compatible',()=>{const f=fixture();assert.equal(ui.supportsMail({...f.runtime,getMir2MailUiCapabilities:()=>JSON.stringify({...caps,bagUiAbiVersion:1})}),false);assert.equal(ui.supportsMail({}),false);assert.equal(ui.supportsMail({...f.runtime,resolveMir2MailUiRows:undefined}),false);assert.equal(ui.fitsMail({...f.input.presentation,touch:true,stageCssScale:600/1024}),false);});

// Execute the actual Page final dispatcher/sink function with controlled dependencies.
function pageSend(dispatcher,onAction,mailIngress={allows:()=>false,claim:()=>false},overrides={}){
 const source=ts.createSourceFile('page.tsx',readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let fn;
 function visit(n){if(ts.isFunctionDeclaration(n)&&n.name?.text==='sendRaw')fn=n;ts.forEachChild(n,visit);}visit(source);assert.ok(fn);
 const writes=[],socket=dispatcher?.mailTestSocket??{readyState:1};socket.send=body=>writes.push(body);const window={dispatchEvent:onAction},noop=()=>{},no=()=>false;
 const dep={isSpectatorBrowserMode:no,socketRef:{current:socket},WebSocket:{OPEN:1},lastCommandRef:{current:null},isMovementPredictionBlockingCommand:no,movementPredictionBlockedUntilRef:{current:0},MOVEMENT_ACTION_PREDICTION_BLOCK_MS:200,
 window,assignMovementConsoleSequence:x=>x,markMir2CacheMilestone:noop,lastRankingRequestRef:{current:null},numberOrUndefined:x=>typeof x==='number'?x:undefined,isMovementCommand:no,movementDiagnosticsRef:{current:null},recordMovementDiagnostic:noop,captureMovementDiagnosticSample:noop,isMovementConsoleCommand:no,rememberMovementConsoleCommand:noop,movementConsoleLogEnabled:no,recordMovementConsoleEvent:noop,captureMovementConsoleState:noop,recordDebugEvent:noop,CustomEvent:class{constructor(type,init){this.type=type;this.detail=init.detail;}},
 itemCommandRequiresOwner:no,currentEquipmentOwner:()=>true,equipmentRenderOwnerToken:null,equipmentHostSuspendReasonRef:{current:null},bevySpellsSendGateRef:{current:no},combatIngressRef:{current:null},mailDispatcherRef:{current:dispatcher},mailIngressRef:{current:mailIngress},scheduleMovementConfirmTick:noop,isCombatResolutionCommand:no,scheduleCombatConfirmTick:noop,appendLog:noop,t:()=>'',mailParcelRef:{current:null},syncMailParcel:()=>true,mailParcelItemsIdle:()=>true,currentSpellsOwner:()=>null,worldRef:{current:{playerObjectId:3}},sameMailOwner:ui.sameMailOwner,...parcelAdapter,...overrides};
 const module={exports:{}},compiled=ts.transpileModule(`export ${fn.getText(source)}`,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 new Function(...Object.keys(dep),'module','exports',compiled)(...Object.values(dep),module,module.exports);return{send:module.exports.sendRaw,writes,socket,socketRef:dep.socketRef,deps:dep};
}
for(const mutation of['locked','collected','cancel','owner','content'])test(`actual Page after mir2:action ${mutation} rejects collect and keeps local state authoritative`,()=>{
 const f=fixture();const state={owner:{...f.input.owner},mail:actualMail(),enabled:true},d=mailDispatcher(()=>state),p=d.prepare('collectParcel',71);assert.ok(p);
 const page=pageSend(d,()=>{if(mutation==='cancel')d.withdraw();else if(mutation==='owner')state.owner.sessionGeneration++;else if(mutation==='content')state.mail[0].message='changed';else state.mail[0][mutation]=true;});
 assert.equal(page.send({type:'collectParcel',mailId:71},{mailProof:p}),false);assert.equal(page.writes.length,0);assert.equal(d.claim(p),false);
});
test('actual Page nested same proof commits only once, ordinary fresh proof can send after cancellation',()=>{
 const f=fixture(),state={owner:{...f.input.owner},mail:actualMail(),enabled:true},d=mailDispatcher(()=>state),p=d.prepare('readMail',71);let nested=false;const page=pageSend(d,()=>{if(!nested){nested=true;page.send({type:'readMail',mailId:71},{mailProof:p});}});
 assert.equal(page.send({type:'readMail',mailId:71},{mailProof:p}),false);assert.equal(page.writes.length,1);assert.equal(state.mail[0].opened,false);d.withdraw();const next=d.prepare('readMail',71);assert.ok(next);assert.equal(page.send({type:'readMail',mailId:71},{mailProof:next}),true);assert.equal(page.writes.length,2);
});

function pageCapture(resolver=resolverPort){
 const source=ts.createSourceFile('page.tsx',readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let fn;
 function visit(n){if(ts.isFunctionDeclaration(n)&&n.name?.text==='captureMailGatewayEvent')fn=n;ts.forEachChild(n,visit);}visit(source);assert.ok(fn);
 const owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3},raw={current:null};
 const deps={runtimeRef:{current:{resolveMir2MailUiRows:resolver}},equipmentConnectionGenerationRef:{current:1},mailSceneRevisionRef:{current:1},mailOpenRef:{current:false},worldRef:{current:{mapFileName:'map-a',playerObjectId:3}},mailRawRef:raw,
 currentSpellsOwner:()=>owner,normalizeMapFileName:x=>String(x),setMailboxOpen:()=>{},parseMailList:packets.parseMailList,mergeMailList:packets.mergeMailList,sameMailOwner:ui.sameMailOwner,
 validMailCompatibility:()=>true,mailCompatRef:{current:null},mailIngressRef:{current:{withdraw:()=>{}}},mailDispatcherRef:{current:{composer:mailSender(),withdraw:()=>{}}},setMailCompatibility:()=>{},presentMailCompatibility:()=>{},setMailComposeRevision:()=>{},mailParcelRawRef:{current:null},mailParcelRef:{current:null},syncMailParcel:()=>false,sendMailParcelLocks:()=>{},pumpMailParcelQuote:()=>{},queueMicrotask:()=>{},...parcelAdapter};
 const module={exports:{}},code=ts.transpileModule(`export ${fn.getText(source)}`,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 new Function(...Object.keys(deps),'module','exports',code)(...Object.values(deps),module,module.exports);return{capture:module.exports.captureMailGatewayEvent,raw,owner,connection:deps.equipmentConnectionGenerationRef};
}
const persistedItem={unique_id:77,name:'SpiritBlade',key:'crystal-item-1',quantity:2,durability_current:3,durability_max:4,soul_bound_id:null,gem_count:1,identified:null,cursed:false,user_item_metadata:{item_index:1}};
test('actual Page ReceiveMail then Stage5 keeps exact same-owner metadata, item-only parcel and authoritative deletion',()=>{
 const f=pageCapture(),received=packets.parseGatewayMailDates(wire);f.capture(received,1);
 const stage={id:71,from:'Sender',subject:'',body:actualMail()[0].message,gold:100,items:[],itemStatesJson:[JSON.stringify(persistedItem)],opened:true,locked:false,claimed:false,deleted:false};
 f.capture({type:'worldSnapshot',payload:{playerObjectId:3,mapFileName:'map-a',stage5Systems:{mail:[stage]}}},1);
 assert.equal(f.raw.current.mail[0].opened,true);assert.equal(f.raw.current.mail[0].metadataKnown,true);assert.equal(f.raw.current.mail[0].dateSentBinaryDatetime,'638962902000000001');assert.equal(f.raw.current.mail[0].items[0].uniqueId,77);assert.equal(f.raw.current.mail[0].items[0].soulBoundId,-1);assert.equal(f.raw.current.mail[0].items[0].identified,true);assert.equal(ui.mailContentKey(f.raw.current.mail[0]),ui.mailContentKey(packets.mergeMailList(actualMail(),null,resolverPort)[0]));
 const d=mailDispatcher(()=>({owner:f.owner,mail:f.raw.current.mail,enabled:true}));assert.ok(d.prepare('collectParcel',71));
 f.owner.connectionGeneration=2;f.connection.current=2;f.capture({type:'worldSnapshot',payload:{playerObjectId:3,mapFileName:'map-a',stage5Systems:{mail:[stage]}}},2);assert.equal(f.raw.current.mail[0].metadataKnown,false);
 f.capture({type:'worldSnapshot',payload:{playerObjectId:3,mapFileName:'map-a',stage5Systems:{mail:[{...stage,deleted:true}]}}},2);assert.deepEqual(f.raw.current.mail,[]);assert.equal(d.prepare('deleteMail',71),null);
 const itemOnly=packets.parseMailList([{...stage,gold:0}]);assert.equal(itemOnly[0].items.length,1);assert.ok(mailDispatcher(()=>({owner:f.owner,mail:itemOnly,enabled:true})).prepare('collectParcel',71));
 assert.equal(packets.parseMailList([{...stage,itemStatesJson:['{bad']}]),null);assert.equal(packets.parseMailList([{...stage,itemStatesJson:[JSON.stringify({...persistedItem,unique_id:9007199254740992})]}]),null);
});
test('current model/presentation drift blocks pointer before tick; final lock eligibility and pending bound fail closed',()=>{
 const f=fixture();f.input.mail[0].message='replacement';assert.equal(f.host.pointerContext(),null);
 const owner={...f.input.owner},letter=packets.parseMailList([{mailId:71,senderName:'Sender',message:'letter',items:[]}]),state={owner,mail:letter,enabled:true},d=mailDispatcher(()=>state),p=d.prepare('lockMail',71);assert.ok(p);assert.equal(p.lock,true);state.mail[0].locked=true;assert.equal(d.allows(p),false);
 state.mail[0].locked=false;d.withdraw();for(let i=0;i<64;i++)assert.ok(d.prepare('readMail',71));assert.equal(d.prepare('readMail',71),null);d.withdraw();assert.ok(d.prepare('readMail',71));
});

function hookHarness(options){
 const slots=[],effects=[],timers=new Map();let cursor=0,nextTimer=1,queued=[],result;
 const react={useRef:v=>{const i=cursor++;return slots[i]??= {current:v};},useState:v=>{const i=cursor++;if(!(i in slots))slots[i]=v;return[slots[i],value=>{slots[i]=typeof value==='function'?value(slots[i]):value;}];},
 useEffect:(create,deps)=>{const i=cursor++,old=effects[i];if(!old||deps.some((v,n)=>v!==old.deps[n]))queued.push(()=>{old?.cleanup?.();effects[i]={deps,cleanup:create()};});}};
 const window={setInterval:f=>{const id=nextTimer++;timers.set(id,f);return id;},clearInterval:id=>timers.delete(id),addEventListener(){},removeEventListener(){}},document={addEventListener(){},removeEventListener(){}};
 const module={exports:{}},code=ts.transpileModule(readFileSync(new URL('../lib/use-bevy-mail-ui.ts',import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 new Function('exports','module','require','window','document','performance',code)(module.exports,module,name=>name==='react'?react:name==='./bevy-mail-ui'?ui:name==='./bevy-mail-text-input'?textInput:(()=>{throw Error(name);})(),window,document,{now:()=>10});
 const render=()=>{cursor=0;queued=[];result=module.exports.useBevyMailUi(options);for(const f of queued)f();return result;};
 const flush=()=>{for(const f of [...timers.values()])f();return render();};
 render();return{options,render,flush,unmount:()=>{for(const e of effects)e?.cleanup?.();},get timers(){return timers.size;}};
}
test('actual Mail hook retires missing runtime and resumes compatibility; live withdrawal waits for published frame',()=>{
 const f=fixture(),options={requested:true,runtimeGeneration:1,runtimeRef:{current:f.runtime},read:()=>f.input,onIntent:()=> 'definitelyUnsent'},h=hookHarness(options);h.flush();h.flush();assert.equal(h.render().ready,true);assert.equal(h.render().withdrawn,false);
 const published=JSON.parse(f.runtime.getMir2MailUiStatus());f.runtime.getMir2MailUiStatus=()=>JSON.stringify(published);options.requested=false;h.render();assert.equal(h.render().withdrawn,false);h.flush();assert.equal(h.render().withdrawn,false);
 options.runtimeRef.current=null;options.runtimeGeneration++;h.render();assert.equal(h.render().ready,false);assert.equal(h.render().withdrawn,true);h.unmount();assert.equal(h.timers,0);
});
test('actual Mail hook replacement/setup failure preserves live barrier then observes computed withdrawal',()=>{
 const f=fixture(),options={requested:true,runtimeGeneration:1,runtimeRef:{current:f.runtime},read:()=>f.input,onIntent:()=> 'definitelyUnsent'},h=hookHarness(options);h.flush();h.flush();assert.equal(h.render().withdrawn,false);
 const old=JSON.parse(f.runtime.getMir2MailUiStatus());f.runtime.getMir2MailUiStatus=()=>JSON.stringify(old);options.runtimeGeneration++;f.runtime.setMir2MailUiIntentSink=()=>{throw Error('setup');};h.render();assert.equal(h.render().withdrawn,false);h.flush();assert.equal(h.render().withdrawn,false);
 old.frame++;old.ready=false;old.inputEnabled=false;h.flush();assert.equal(h.render().withdrawn,true);h.unmount();assert.equal(h.timers,0);
});
function mailWindowActions(presentation,onOpen,onPresentationChange){
 const source=ts.createSourceFile('mail-window.tsx',readFileSync(new URL('../app/components/original-client-mail-window.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);const decl=[];let view;
 function visit(n){if(ts.isFunctionDeclaration(n)&&['openMessage','isUnread'].includes(n.name?.text))decl.push(n.getText(source));if(ts.isVariableDeclaration(n)&&n.name.getText(source)==='setView')view=n.getText(source);ts.forEachChild(n,visit);}visit(source);assert.equal(decl.length,2);assert.ok(view);
 const code=ts.transpileModule(decl.join('\n')+';const '+view+';',{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const deps={presentation,onOpen,onPresentationChange,composeState:null,setLocalSelectedId:()=>{},setLocalView:()=>{}};return new Function(...Object.keys(deps),code+';return {openMessage,setView};')(...Object.values(deps));
}
test('actual MailWindow handoff suppresses only current reader; failed or silent read Back/reopen retries without ACK',()=>{
 const row={id:71,from:'Sender',body:'body',read:false,contentKey:'same'},initial={key:'initial',view:'read',selectedId:71,contentKey:'same',recipient:'',readDispatched:true};let p=initial,reads=0;
 const mount=()=>mailWindowActions(p,()=>{reads++;return false;},next=>p=next);
 mount().openMessage(row);assert.equal(reads,0);mount().setView('inbox');assert.equal(p.readDispatched,false);mount().openMessage(row);assert.equal(reads,1);assert.equal(p.readDispatched,false);assert.equal(row.read,false);
 mount().setView('inbox');mount().openMessage(row);assert.equal(reads,2);mount();mount();assert.equal(reads,2);
});
test('actual Page controlled Back clears prior local read attempt before explicit reopen',()=>{
 const source=ts.createSourceFile('page.tsx',readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let fn;function visit(n){if(ts.isFunctionDeclaration(n)&&n.name?.text==='presentMailCompatibility')fn=n;ts.forEachChild(n,visit);}visit(source);assert.ok(fn);
 const owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3},row=actualMail()[0],attempt={current:{owner,sceneRevision:1,content:ui.mailContentKey(row)}},compat={current:null};
 const deps={currentSpellsOwner:()=>owner,worldRef:{current:{playerObjectId:3}},mailRawRef:{current:{owner,mail:[row]}},parseMailList:packets.parseMailList,sameMailOwner:ui.sameMailOwner,mailContentKey:ui.mailContentKey,mailOpenRef:{current:true},mailIngressRef:{current:{withdraw(){}}},mailDispatcherRef:{current:{composer:mailSender(),withdraw(){}}},mailReadAttemptRef:attempt,mailPresentationSequenceRef:{current:0},mailSceneRevisionRef:{current:1},mailCompatRef:compat,setMailCompatibility:()=>{},cancelMailParcel:()=>{}};
 const module={exports:{}},code=ts.transpileModule('export '+fn.getText(source),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;new Function(...Object.keys(deps),'module','exports',code)(...Object.values(deps),module,module.exports);
 module.exports.presentMailCompatibility('inbox',71,'',true);assert.equal(attempt.current,null);assert.equal(compat.current.readDispatched,false);module.exports.presentMailCompatibility('read',71,'',false);assert.equal(compat.current.readDispatched,false);
 const refreshed=refreshMail()[0];assert.equal(refreshed.metadataKnown,true);assert.equal(refreshed.items[0].identified,true);assert.equal(refreshed.items[0].soulBoundId,-1);assert.equal(ui.mailContentKey(packets.mergeMailList([row],null,resolverPort)[0]),ui.mailContentKey(refreshed));
 const changed=structuredClone(refreshed);changed.items[0].currentDura++;assert.notEqual(ui.mailContentKey(row),ui.mailContentKey(changed));
});


test('current nullable/template facts never borrow previous flags or indices; resolved metadata and final proof qualify current rows',()=>{
 const previous=actualMail();
 const unknown=packets.mergeMailList(refreshSource(120),previous);assert.equal(unknown[0].items[0].itemIndex,null);assert.equal(unknown[0].items[0].identified,null);assert.equal(unknown[0].metadataKnown,false);
 const unknownSource=refreshSource();const unknownItem=JSON.parse(unknownSource[0].itemStatesJson[0]);unknownItem.key='unknown-current-template';unknownSource[0].itemStatesJson[0]=JSON.stringify(unknownItem);assert.equal(packets.mergeMailList(unknownSource,previous,resolverPort),null);
 assert.equal(packets.mergeMailList(refreshSource(),previous,()=>null),null);assert.equal(packets.mergeMailList(refreshSource(),previous,()=>{throw Error('unavailable');}),null);
 assert.equal(packets.mergeMailList(refreshSource(),previous,json=>{const rows=JSON.parse(json);rows[0].items[0].itemIndex=120;return resolverPort(JSON.stringify(rows));}),null);
 const cases=resolutionFixtures();for(const c of cases){assert.equal(c.current[0].items[0].itemIndex,null);assert.equal(c.merged[0].metadataKnown,c.name==='cosmetic-same-template');}
 assert.equal(cases[0].current[0].items[0].identified,null);assert.equal(cases[1].current[0].items[0].identified,null);
 const owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3};let mail=packets.mergeMailList(previous,null,resolverPort);
 const dispatcher=mailDispatcher(()=>({owner,mail,enabled:true}));const proof=dispatcher.prepare('collectParcel',71);assert.ok(proof);
 mail=cases.find(c=>c.name==='cosmetic-same-template').merged;assert.equal(dispatcher.allows(proof),true);
 const sink=pageSend(dispatcher,()=>{mail=cases.find(c=>c.name==='template-change').merged;});assert.equal(sink.send({type:'collectParcel',mailId:71},{mailProof:proof}),false);assert.deepEqual(sink.writes,[]);assert.equal(dispatcher.claim(proof),false);
 mail=cases.find(c=>c.name==='cosmetic-same-template').merged;const fresh=dispatcher.prepare('collectParcel',71);const positive=pageSend(dispatcher,()=>{});assert.equal(positive.send({type:'collectParcel',mailId:71},{mailProof:fresh}),true);assert.equal(positive.writes.length,1);
});


test('actual Page retains current compatibility mailbox/actions when catalogue is unavailable; legacy aliases never borrow defaults',()=>{
 for(const resolver of[undefined,()=>null,()=>{throw Error('catalogue unavailable');}]){
  // null intentionally disables the default argument for the missing-export case.
  const f=pageCapture(resolver??null),source=refreshSource(),state=JSON.parse(source[0].itemStatesJson[0]);state.key='wooden-sword';source[0].itemStatesJson[0]=JSON.stringify(state);
  f.capture({type:'worldSnapshot',payload:{playerObjectId:3,mapFileName:'map-a',stage5Systems:{mail:source}}},1);
  assert.equal(f.raw.current.catalogResolved,false);assert.equal(f.raw.current.mail.length,1);assert.equal(f.raw.current.mail[0].items[0].identified,null);assert.equal(f.raw.current.mail[0].items[0].itemIndex,null);assert.equal(f.raw.current.mail[0].metadataKnown,false);
  const live={owner:f.owner,mail:f.raw.current.mail,enabled:true},dispatcher=mailDispatcher(()=>live);
  for(const type of['readMail','collectParcel','deleteMail']){const proof=dispatcher.prepare(type,71);assert.ok(proof);const sink=pageSend(dispatcher,()=>{});assert.equal(sink.send({type,mailId:71},{mailProof:proof}),true);assert.equal(sink.writes.length,1);}
  // Current owner/content/cancel changes after the actual mir2:action still fence compatibility sends.
  for(const mutation of['owner','content','cancel']){live.owner={...f.owner};live.mail=structuredClone(f.raw.current.mail);const proof=dispatcher.prepare('collectParcel',71);assert.ok(proof);const sink=pageSend(dispatcher,()=>{if(mutation==='owner')live.owner={...live.owner,connectionGeneration:2};else if(mutation==='content')live.mail[0].items[0].key='short-sword';else dispatcher.withdraw();});assert.equal(sink.send({type:'collectParcel',mailId:71},{mailProof:proof}),false);assert.deepEqual(sink.writes,[]);}
  live.owner={...f.owner};live.mail=[{...f.raw.current.mail[0],gold:0,items:[],itemCount:0}];const lock=dispatcher.prepare('lockMail',71);assert.ok(lock);const sink=pageSend(dispatcher,()=>{});assert.equal(sink.send({type:'lockMail',mailId:71,lock:true},{mailProof:lock}),true);assert.equal(sink.writes.length,1);
 }
 const f=fixture();f.runtime.resolveMir2MailUiRows=undefined;f.host.tick();assert.equal(f.host.pointerContext(),null);assert.equal(f.host.isWithdrawn(),true);
});


// Extract the two actual hook read providers, not a reconstructed eligibility rule.
function pageReadProviders(overrides={}){
 const source=ts.createSourceFile('page.tsx',readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),found=new Map();
 function visit(node){if(ts.isVariableDeclaration(node)&&['bevySpellsUi','bevyMailUi'].includes(node.name.getText(source))){
  const call=node.initializer;assert.ok(call&&ts.isCallExpression(call));assert.equal(call.expression.getText(source),node.name.getText(source)==='bevySpellsUi'?'useBevySpellsUi':'useBevyMailUi');
  const input=call.arguments[0];assert.ok(ts.isObjectLiteralExpression(input));const property=input.properties.find(p=>ts.isPropertyAssignment(p)&&p.name.getText(source)==='read');assert.ok(property&&ts.isArrowFunction(property.initializer));assert.equal(found.has(node.name.getText(source)),false);found.set(node.name.getText(source),property.initializer.getText(source));
 }ts.forEachChild(node,visit);}visit(source);assert.equal(found.size,2);
 const owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3},row=actualMail()[0];
 const deps={worldRef:{current:{connected:true,playerObjectId:3,entities:[{objectId:3}]}},currentSpellsOwner:id=>id===3?owner:null,spellsRawSnapshotRef:{current:{snapshot:{playerObjectId:3}}},
  mailRawRef:{current:{owner,mail:[row],catalogResolved:true}},bevyQuestGenerationRef:{current:5},mailSceneRevisionRef:{current:4},mailOpenRef:{current:true},mailCompatRef:{current:null},mailRustOwnsRef:{current:true},bevyHpLocalOverlayOpenRef:{current:false},
  bevyQuestUiRequested:true,bevyRuntimeBackend:'webgpu',sharedCanvasUsesWebGl2:()=>false,webGl2SharedCanvasPrototype:false,screenRef:{current:'game'},initialSceneAssetsReadyRef:{current:true},equipmentHostSuspendReasonRef:{current:null},bevyHudUi:{readCurrent:()=>({ready:true})},
  document:{visibilityState:'visible',hasFocus:()=>true,querySelector:()=>null},readBevyQuestPresentation:()=>({logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false}),sharedUiCanvasId:()=> 'mir2-quest-ui-canvas',clientProfile:{input:'mouse'},projectAuthoritativeHudPlayer:(_,player)=>player,
  bevyQuestReactModalOpen:false,showCharacter:false,activeCharacterTab:'spells',showInventory:false,showQuestLog:false,showHeroPet:false,showGuild:false,showGroup:false,showFriends:false,showBonds:false,showRanking:false,showMarket:false,showConquest:false,showTrade:false,showBuffs:false,showMail:false,showWorldMap:false,showHelp:false,showHotkeys:false,showChatSettings:false,npcShopService:null,npcRepairService:null,...overrides};
 const code=ts.transpileModule(`const spellsRead=${found.get('bevySpellsUi')};const mailRead=${found.get('bevyMailUi')};`,{compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
 const api=new Function(...Object.keys(deps),code+'\nreturn {spellsRead,mailRead};')(...Object.values(deps));return{...api,deps};
}
test('actual Page Spells provider is independent of Mail resolution while Mail provider and owner/scene fences stay closed',()=>{
 const absent=pageReadProviders({mailRawRef:{current:null}});assert.equal(absent.spellsRead(null).eligible,true);assert.equal(absent.mailRead().eligible,false);
 for(const resolver of[null,()=>null,()=>{throw Error('unavailable');}]){
  const captured=pageCapture(resolver),source=refreshSource(),item=JSON.parse(source[0].itemStatesJson[0]);item.key='wooden-sword';source[0].itemStatesJson[0]=JSON.stringify(item);captured.capture({type:'worldSnapshot',payload:{playerObjectId:3,mapFileName:'map-a',stage5Systems:{mail:source}}},1);
  const providers=pageReadProviders({mailRawRef:captured.raw});assert.equal(captured.raw.current.catalogResolved,false);assert.equal(providers.spellsRead(null).eligible,true);assert.equal(providers.mailRead().eligible,false);
 }
 const unresolved=pageReadProviders({mailRawRef:{current:{mail:actualMail(),owner:{connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3},catalogResolved:false}}});assert.equal(unresolved.spellsRead(null).eligible,true);assert.equal(unresolved.mailRead().eligible,false);
 const valid=pageReadProviders();assert.equal(valid.spellsRead(null).eligible,true);assert.equal(valid.mailRead().eligible,true);
 assert.equal(pageReadProviders({spellsRawSnapshotRef:{current:{snapshot:{playerObjectId:9}}}}).spellsRead(null).eligible,false,'actual mismatched owner remains blocked');
 for(const overrides of[{screenRef:{current:'login'}},{worldRef:{current:{connected:false,playerObjectId:3,entities:[{objectId:3}]}}},{initialSceneAssetsReadyRef:{current:false}},{equipmentHostSuspendReasonRef:{current:'reset'}},{bevyHudUi:{readCurrent:()=>({ready:false})}},{bevyQuestReactModalOpen:true}]){const p=pageReadProviders(overrides);assert.equal(p.spellsRead(null).eligible,false);assert.equal(p.mailRead().eligible,false);}
 const f=fixture(),p=pageReadProviders(),host=new ui.MailHost({runtime:f.runtime,isCurrent:()=>true,read:p.mailRead,now:()=>10,onState:()=>{},onIntent:()=> 'confirmedSend'});host.tick();host.tick();host.tick();assert.ok(host.pointerContext());
 p.deps.mailSceneRevisionRef.current++;assert.equal(host.pointerContext(),null,'old presentation scene cannot keep input authority');host.tick();host.tick();host.tick();assert.ok(host.pointerContext());
 p.deps.mailRawRef.current.owner={...p.deps.mailRawRef.current.owner,playerObjectId:9};host.tick();assert.equal(host.pointerContext(),null,'stale mailbox owner cannot acquire current Mail grant');host.stop();
});

// M14a: execute actual Page/Window consumers with a controlled Rust/WASM port.
// Port inputs/outputs are exported for Root's independent compiled Rust replay.
const composeCaptures=[];
const parcelSessions=[];
const parcelOwner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3};
const emptyParcelSnapshot={bagCapacity:40,items:[]};
const emptyParcelState={attachmentUniqueIds:[],stamped:false,slotLimit:1,blockedUniqueIds:[],postage:null,pendingQuote:false,quoteReady:false,reviewRequired:false,notice:null};
function parcelOutput(state,locks=[],quote=null,token=null){return{ok:true,state:{...emptyParcelState,...state},locks,quote,token};}
function actualParcelRuntime(module){
 const source=ts.createSourceFile('client-core-runtime.ts',readFileSync(new URL('../lib/client-core-runtime.ts',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TS),nodes=[];let method=null;
 function visit(n){if(ts.isFunctionDeclaration(n)&&['mailResult','parcelResult'].includes(n.name?.text))nodes.push(n.getText(source));if(ts.isMethodDeclaration(n)&&n.name?.getText(source)==='createMailParcelLedger')method=n.getText(source);ts.forEachChild(n,visit);}visit(source);assert.equal(nodes.length,2);assert.ok(method);
 const code=ts.transpileModule(`${nodes.join('\n')}\nconst runtime={${method}};`,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 return new Function('module',code+';return runtime.createMailParcelLedger();')(module);
}
// A controlled export exercises only declared fixture transitions. Its full
// transcript must independently agree with compiled Rust before acceptance.
function readyGoldParcel(){
 const captures=[];parcelSessions.push(captures);let state={...emptyParcelState},ownerKey=null,gold=700,serial=0;
 const runtime=actualParcelRuntime({mail_parcel_abi_version:()=>1,MailParcelBridge:class{transact(inputJson){const input=JSON.parse(inputJson);let output;
  if(input.action==='sync'){const key=JSON.stringify(input.owner);if(ownerKey!==key||gold!==input.gold)state={...emptyParcelState};ownerKey=key;gold=input.gold;output=parcelOutput(state);}
  else if(input.action==='quote'){state={...state,pendingQuote:true};output=parcelOutput(state,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},++serial);}
  else if(input.action==='enter'||input.action==='finish')output=parcelOutput(state);
  else if(input.action==='cost'){state={...state,pendingQuote:false,postage:70,quoteReady:true};output=parcelOutput(state);}
  else if(input.action==='complete'||input.action==='cancel'){state={...emptyParcelState};output=parcelOutput(state);}
  else if(input.action==='status')output=parcelOutput(state);else throw Error(`undeclared controlled export action ${input.action}`);
  const outputJson=JSON.stringify(output);captures.push({inputJson,outputJson,input,output});return outputJson;}}});
 const parcel=new ui.MailParcelController(runtime);parcel.sync(parcelOwner,emptyParcelSnapshot,700,1);const q=parcel.quote(parcelOwner,2);assert.ok(q);assert.equal(parcel.enter(q.proof,q.proof.body),true);parcel.finish(q.proof);parcel.call({action:'cost',cost:70});return parcel;
}
// Extract product AST nodes unchanged: schema decisions belong to mailResult
// and the two actual wrapper methods, never a test copy of their rules.
function actualMailWrappers(module){
 const source=ts.createSourceFile('client-core-runtime.ts',readFileSync(new URL('../lib/client-core-runtime.ts',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TS),methods=new Map();let parser=null;
 function visit(n){
  if(ts.isFunctionDeclaration(n)&&n.name?.text==='mailResult'){assert.equal(parser,null);parser=n.getText(source);}
  if(ts.isMethodDeclaration(n)&&['normalizeMailMessage','prepareMailSend'].includes(n.name?.getText(source))){const name=n.name.getText(source);assert.equal(methods.has(name),false);methods.set(name,n.getText(source));}
  ts.forEachChild(n,visit);
 }visit(source);assert.ok(parser);assert.equal(methods.size,2);
 const code=ts.transpileModule(`${parser}\nconst bridge={${[...methods.values()].join(',\n')}};`,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 return new Function('module',code+';return bridge;')(module);
}
const wrapperSendInput={recipient:' R ',message:' hello\r\n世界\t ',gold:700,attachmentUniqueIds:[],stamped:false};
const wrapperPayload={name:'R',message:'hello\n世界',gold:700,itemsIdx:[0,0,0,0,0],stamped:false};
function wrapperModule(overrides={}){return{mail_compose_abi_version:()=>1,normalize_mail_message:()=>JSON.stringify({ok:true,message:'body'}),prepare_mail_send:()=>JSON.stringify({ok:true,payload:wrapperPayload}),...overrides};}
test('actual small-WASM wrappers reject missing/mismatched ABI and absent exports before making a call',()=>{
 for(const abi of[undefined,()=>0,()=>2,()=> '1']){
  let calls=0;const bridge=actualMailWrappers(wrapperModule({mail_compose_abi_version:abi,normalize_mail_message:()=>{calls++;},prepare_mail_send:()=>{calls++;}}));
  assert.deepEqual(bridge.normalizeMailMessage('body'),{ok:false,error:'Shared mail rules are unavailable; reload the client'});
  assert.deepEqual(bridge.prepareMailSend(wrapperSendInput),{ok:false,error:'Shared mail rules are unavailable; reload the client'});assert.equal(calls,0);
 }
 const noMessage=actualMailWrappers(wrapperModule({normalize_mail_message:undefined}));assert.equal(noMessage.normalizeMailMessage('body').ok,false);assert.equal(noMessage.prepareMailSend(wrapperSendInput).ok,true);
 const noSend=actualMailWrappers(wrapperModule({prepare_mail_send:undefined}));assert.equal(noSend.prepareMailSend(wrapperSendInput).ok,false);assert.equal(noSend.normalizeMailMessage('body').ok,true);
});
test('actual small-WASM wrappers serialize exact parameters and accept exact success and Rust rejection schemas',()=>{
 const calls=[],bridge=actualMailWrappers(wrapperModule({normalize_mail_message:json=>{calls.push(['message',json]);return JSON.stringify({ok:true,message:'a\nb'});},prepare_mail_send:json=>{calls.push(['send',json]);return JSON.stringify({ok:true,payload:wrapperPayload});}}));
 assert.deepEqual(bridge.normalizeMailMessage('a\r\nb'),{ok:true,message:'a\nb'});assert.deepEqual(bridge.prepareMailSend(wrapperSendInput),{ok:true,payload:wrapperPayload});
 assert.deepEqual(calls,[['message',JSON.stringify({message:'a\r\nb'})],['send',JSON.stringify(wrapperSendInput)]]);
 const rejection={ok:false,error:'Invalid mail input'},failed=actualMailWrappers(wrapperModule({normalize_mail_message:()=>JSON.stringify(rejection),prepare_mail_send:()=>JSON.stringify(rejection)}));
 assert.deepEqual(failed.normalizeMailMessage('body'),rejection);assert.deepEqual(failed.prepareMailSend(wrapperSendInput),rejection);
});
test('actual mailResult and wrappers throw on malformed decision and success schemas',()=>{
 const badDecisions=[null,[],{}, {ok:1},{ok:'true'},{ok:false},{ok:false,error:3},{ok:false,error:'bad',message:'extra'}];
 for(const value of badDecisions){const bridge=actualMailWrappers(wrapperModule({normalize_mail_message:()=>JSON.stringify(value),prepare_mail_send:()=>JSON.stringify(value)}));assert.throws(()=>bridge.normalizeMailMessage('body'));assert.throws(()=>bridge.prepareMailSend(wrapperSendInput));}
 for(const value of[{ok:true},{ok:true,message:3},{ok:true,message:'body',extra:true}])assert.throws(()=>actualMailWrappers(wrapperModule({normalize_mail_message:()=>JSON.stringify(value)})).normalizeMailMessage('body'));
 const badSends=[{ok:true},{ok:true,payload:null},{ok:true,payload:[]},{ok:true,payload:wrapperPayload,extra:true}];
 for(const [key,bad]of[['name',3],['message',null],['gold',-1],['gold',1.5],['gold',4294967296],['itemsIdx',[0,0,0,0]],['itemsIdx',[0,0,0,0,-1]],['itemsIdx',[0,0,0,0,1.5]],['itemsIdx',[0,0,0,0,9007199254740992]],['stamped',0]])badSends.push({ok:true,payload:{...wrapperPayload,[key]:bad}});
 badSends.push({ok:true,payload:{...wrapperPayload,subject:'extra'}});const missing={...wrapperPayload};delete missing.stamped;badSends.push({ok:true,payload:missing});
 for(const value of badSends)assert.throws(()=>actualMailWrappers(wrapperModule({prepare_mail_send:()=>JSON.stringify(value)})).prepareMailSend(wrapperSendInput));
});
test('actual small-WASM wrappers propagate ABI/export exceptions and JSON parse errors to their Page caller',()=>{
 const failure=Error('controlled WASM export failure');
 for(const overrides of[{mail_compose_abi_version:()=>{throw failure;}},{normalize_mail_message:()=>{throw failure;},prepare_mail_send:()=>{throw failure;}}]){
  const bridge=actualMailWrappers(wrapperModule(overrides));assert.throws(()=>bridge.normalizeMailMessage('body'),e=>e===failure);assert.throws(()=>bridge.prepareMailSend(wrapperSendInput),e=>e===failure);
 }
 const malformed=actualMailWrappers(wrapperModule({normalize_mail_message:()=>'{',prepare_mail_send:()=>'{'}));assert.throws(()=>malformed.normalizeMailMessage('body'),SyntaxError);assert.throws(()=>malformed.prepareMailSend(wrapperSendInput),SyntaxError);
});
function pageFunctions(names,deps){
 const source=ts.createSourceFile('page.tsx',readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),found=new Map();
 function visit(n){if(ts.isFunctionDeclaration(n)&&names.includes(n.name?.text))found.set(n.name.text,n.getText(source));ts.forEachChild(n,visit);}visit(source);
 assert.equal(found.size,names.length);const code=ts.transpileModule(names.map(n=>found.get(n)).join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 const api=new Function(...Object.keys(deps),code+`;return {${names.join(',')}};`)(...Object.values(deps));
 // Explicit fixture source captured once from its producing controlled socket.
 // Production has no default-source fallback and rejects an unbound ACK.
 if(api.captureMailGatewayEvent&&deps.mailSourceSocket){const actual=api.captureMailGatewayEvent,source=deps.mailSourceSocket;api.captureMailGatewayEvent=(event,generation,provided=source)=>actual(event,generation,provided);}
 return api;
}
function composePage(options={}){
 const owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3},live={owner,mail:actualMail(),enabled:true};
 const dispatcher=mailDispatcher(()=>live,options.slotModule),draft={to:' R ',subject:'local only',body:' hello\r\n世界\t ',goldText:'700',items:[]};
 const wirePayload={name:'R',message:'hello\n世界',gold:700,itemsIdx:[0,0,0,0,0],stamped:false};
 const parcel=readyGoldParcel();
 const send=pageSend(dispatcher,()=>options.onAction?.(api),undefined,{mailParcelRef:{current:parcel},currentSpellsOwner:()=>live.owner,syncMailParcel:gold=>{parcel.sync(live.owner,emptyParcelSnapshot,gold??700,10);return true;}});if(options.socketThrow)send.socket.send=()=>{throw Error('controlled socket failure');};
 const wasmCalls=[],wasm=wrapperModule({
  prepare_mail_send:inputJson=>{const input=JSON.parse(inputJson);assert.deepEqual(input,{recipient:draft.to,message:draft.body,gold:700,attachmentUniqueIds:[],stamped:false});
   if(options.coreThrow)throw Error('controlled core failure');const outputJson=options.sendResult??JSON.stringify({ok:true,payload:wirePayload});
   wasmCalls.push({kind:'send',inputJson,outputJson});if(!options.sendResult)composeCaptures.push({kind:'send',input:JSON.parse(inputJson),output:JSON.parse(outputJson),inputJson,outputJson});return outputJson;},
  normalize_mail_message:inputJson=>{const {message}=JSON.parse(inputJson);const output=message==='😀'.repeat(251)?{ok:false,error:'Mail message exceeds 500 UTF-16 units; draft kept'}:{ok:true,message:'😀'.repeat(250)};
   const outputJson=options.messageResult??JSON.stringify(output);wasmCalls.push({kind:'message',inputJson,outputJson});if(!options.messageResult)composeCaptures.push({kind:'message',input:JSON.parse(inputJson),output:JSON.parse(outputJson),inputJson,outputJson});return outputJson;},
  ...(options.wasmOverrides??{})});
 const views=[],deps={currentSpellsOwner:()=>live.owner,worldRef:{current:{playerObjectId:3,mapFileName:'map-a'}},mailRenderOwner:owner,mailRenderRows:live.mail,
  mailSourceSocket:send.socket,mailDispatcherRef:{current:dispatcher},mailCompatRef:{current:{key:'compose-1'}},validMailCompatibility:()=>true,mailPresentation:{key:'compose-1'},
  questCoreRuntimeRef:{current:actualMailWrappers(wasm)},
  setMailComposeRevision:()=>{},parseMailList:packets.parseMailList,sameMailOwner:ui.sameMailOwner,mailIngressRef:{current:{allows:()=>false,withdraw(){}}},sendRaw:send.send,
  equipmentConnectionGenerationRef:{current:1},mailSceneRevisionRef:{current:4},mailOpenRef:{current:true},mailRawRef:{current:{owner,mail:live.mail}},
  setMailboxOpen:()=>{},normalizeMapFileName:x=>String(x),runtimeRef:{current:null},mergeMailList:packets.mergeMailList,setMailCompatibility:()=>{},presentMailCompatibility:view=>views.push(view),
  mailParcelRef:{current:parcel},mailParcelRawRef:{current:null},syncMailParcel:send.deps.syncMailParcel,sendMailParcelLocks:()=>{},mailParcelItemsIdle:()=>true,pumpMailParcelQuote:()=>{},queueMicrotask:()=>{},...parcelAdapter};
 const api=pageFunctions(['rememberMailDraft','normalizeMailDraftMessage','sendMailMessage','dispatchMailCommand','captureMailGatewayEvent'],deps);
 api.rememberMailDraft(draft);
 return{...api,draft,wirePayload,dispatcher,live,owner,send,views,deps,wasmCalls};
}
function submitInput(f){return{to:f.draft.to,subject:f.draft.subject,body:f.draft.body,gold:700,items:[]};}
test('actual Page compatibility send calls small Rust port, preserves gold, exact body and single flight through reentrant listener',()=>{
 let nested;const f=composePage({onAction:api=>{nested=api.sendMailMessage({to:'other',subject:'',body:'duplicate',gold:1});}});
 assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');assert.equal(nested,'definitelyUnsent');
 assert.deepEqual(f.send.writes,[JSON.stringify({...f.wirePayload,type:'sendMail'})]);assert.equal(JSON.parse(f.send.writes[0]).subject,undefined);
 assert.deepEqual(f.wasmCalls,[{kind:'send',inputJson:JSON.stringify(wrapperSendInput),outputJson:JSON.stringify({ok:true,payload:wrapperPayload})}]);
 assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);assert.equal(f.dispatcher.composer.pending(f.owner),true);
 f.dispatcher.withdraw();assert.equal(f.dispatcher.composer.pending(f.owner),true,'handoff cannot grant a second send');
 assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent');assert.equal(f.send.writes.length,1);
});
test('actual Page ACK fails with full draft retained, succeeds only for current flight and ignores old connection ACK',()=>{
 const f=composePage();assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');
 f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:1}},0);
 assert.equal(f.dispatcher.composer.pending(f.owner),true);assert.deepEqual(f.views,[]);
 f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:-1}},1);
 assert.equal(f.dispatcher.composer.pending(f.owner),false);assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);assert.deepEqual(f.views,[]);
 assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');
 f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:1}},1);
 assert.deepEqual(f.views,['inbox']);assert.equal(f.dispatcher.composer.draft(f.owner),null);
 f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:-1}},1);assert.deepEqual(f.views,['inbox']);
});
test('actual Page errors before socket are retryable; socket throw is unknown with no timeout or handoff retry',()=>{
 for(const options of[{coreThrow:true},{onAction(){throw Error('controlled listener failure');}}]){
  const f=composePage(options);assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent');assert.equal(f.dispatcher.composer.pending(f.owner),false);
  assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);assert.equal(f.send.writes.length,0);assert.ok(f.dispatcher.composer.notice);
 }
 const f=composePage({socketThrow:true});assert.equal(f.sendMailMessage(submitInput(f)),'outcomeUnknown');
 f.dispatcher.withdraw();assert.equal(f.dispatcher.composer.pending(f.owner),true);assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent');
 f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:-1}},1);assert.equal(f.dispatcher.composer.pending(f.owner),false);
});
test('same-connection owner change reserves the uncorrelated old ACK slot without touching the new owner draft',()=>{
 const f=composePage();assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');
 f.live.owner={...f.owner,sessionGeneration:8};
 f.captureMailGatewayEvent({type:'packet',packet:'NewMail',payload:{}},1);
 assert.equal(f.dispatcher.composer.draft(f.live.owner),null);assert.equal(f.dispatcher.composer.pending(f.live.owner),true);
 assert.equal(f.dispatcher.composer.remember(f.live.owner,'new',{...f.draft,to:'new owner'}),false);
 f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:1}},1);
 assert.equal(f.dispatcher.composer.pending(f.live.owner),false);assert.deepEqual(f.views,[],'late old success cannot close a new composer');
 assert.equal(f.dispatcher.composer.remember(f.live.owner,'new',{...f.draft,to:'new owner'}),true);
 f.dispatcher.composer.sync({...f.live.owner,connectionGeneration:2});assert.equal(f.dispatcher.composer.draft(f.live.owner),null);
});
test('actual final Page socket rejects payload substitution and owner switch before write; committed false stays unknown',()=>{
 const f=composePage(),sender=f.dispatcher.composer,p=sender.reserve(f.owner,'compose-1',f.wirePayload);
 const proof=f.dispatcher.prepare('sendMail',null,null,p);assert.ok(proof);
 assert.equal(f.send.send({...f.wirePayload,gold:701,type:'sendMail'},{mailProof:proof}),false);
 assert.equal(sender.finish(p,'definitelyUnsent'),'definitelyUnsent');assert.equal(f.send.writes.length,0);
 const switched=composePage({onAction(){switched.live.owner={...switched.owner,sessionGeneration:8};}});
 assert.equal(switched.sendMailMessage(submitInput(switched)),'definitelyUnsent');assert.equal(switched.send.writes.length,0);
 const g=composePage(),q=g.dispatcher.composer.reserve(g.owner,'compose-1',g.wirePayload);
 assert.equal(g.dispatcher.composer.enterSocket(q,q.body,g.send.socket),true);assert.equal(g.dispatcher.composer.finish(q,'definitelyUnsent'),'outcomeUnknown');
 assert.equal(g.dispatcher.composer.pending(g.owner),true);
});
test('actual Page rejects unresolved attachment refs visibly; normalization rejects overflow without replacing saved draft',()=>{
 const f=composePage();assert.equal(f.sendMailMessage({...submitInput(f),items:['Wooden Sword']}),'definitelyUnsent');
 assert.equal(f.send.writes.length,0);assert.match(f.dispatcher.composer.notice,/attachment/i);assert.deepEqual(f.dispatcher.composer.draft(f.owner).items,['Wooden Sword']);
 assert.deepEqual(f.normalizeMailDraftMessage('😀'.repeat(250)),{ok:true,message:'😀'.repeat(250)});
 assert.equal(f.normalizeMailDraftMessage('😀'.repeat(251)).ok,false);assert.equal(f.dispatcher.composer.draft(f.owner).body,f.draft.body);
});
function windowComposeFunctions(names,deps){
 const source=ts.createSourceFile('mail-window.tsx',readFileSync(new URL('../app/components/original-client-mail-window.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),found=new Map();
 function visit(n){if(ts.isFunctionDeclaration(n)&&names.includes(n.name?.text))found.set(n.name.text,n.getText(source));ts.forEachChild(n,visit);}visit(source);assert.equal(found.size,names.length);
 const code=ts.transpileModule(names.map(n=>found.get(n)).join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 return new Function(...Object.keys(deps),code+`;return {${names.join(',')}};`)(...Object.values(deps));
}
test('actual component submit keeps view until ACK, exact raw gold and Page-owned draft survives remount',()=>{
 const f=composePage(),calls=[];
 const deps={onSendMail:d=>{calls.push(d);return'confirmedSend';},composeState:{pending:false,draft:f.draft},composeTo:f.draft.to,composeSubject:f.draft.subject,
  composeBody:f.draft.body,composeGold:'700',composeItems:[],parcelState:null,setView(){throw Error('component closed before ACK');}};
 windowComposeFunctions(['submitCompose'],deps).submitCompose();assert.deepEqual(calls,[{...submitInput(f),items:undefined}]);
 deps.composeState.pending=true;windowComposeFunctions(['submitCompose'],deps).submitCompose();assert.equal(calls.length,1);
 deps.composeState.pending=false;deps.composeGold='1.5';windowComposeFunctions(['submitCompose'],deps).submitCompose();assert.equal(calls[1].gold,1.5,'never floor invalid gold into a different send');
 const persisted=[];windowComposeFunctions(['saveDraft'],{composeState:{pending:false},setLocalDraft:()=>{},onDraftChange:d=>persisted.push(d)}).saveDraft(f.draft);
 assert.deepEqual(persisted,[f.draft]);assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);
});
test('actual Page old compose callbacks cannot replace or submit a newer same-owner draft',()=>{
 const f=composePage();f.deps.mailCompatRef.current={key:'compose-2'};
 const newer={...f.draft,body:'newer draft'};f.dispatcher.composer.remember(f.owner,'compose-2',newer);
 f.rememberMailDraft({...f.draft,body:'old edit'});
 assert.deepEqual(f.dispatcher.composer.draft(f.owner),newer);
 assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent');assert.equal(f.send.writes.length,0);
 assert.deepEqual(f.dispatcher.composer.draft(f.owner),newer);
});
test('actual Page handles unavailable and malformed real wrapper responses before reserving a flight',()=>{
 for(const options of[{wasmOverrides:{mail_compose_abi_version:()=>2}},{sendResult:'{'},{sendResult:JSON.stringify({ok:true,payload:{...wrapperPayload,gold:1.5}})},{sendResult:JSON.stringify({ok:false,error:'Invalid mail input'})}]){
  const f=composePage(options);assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent');assert.equal(f.send.writes.length,0);assert.equal(f.dispatcher.composer.pending(f.owner),false);assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);assert.ok(f.dispatcher.composer.notice);
 }
 for(const options of[{wasmOverrides:{normalize_mail_message:undefined}},{messageResult:'{'},{messageResult:JSON.stringify({ok:true,message:3})}]){
  const f=composePage(options);assert.equal(f.normalizeMailDraftMessage('body').ok,false);assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);assert.ok(f.dispatcher.composer.notice);
 }
});
function scriptedParcel(steps){
 const captures=[];parcelSessions.push(captures);let at=0;
 const runtime=actualParcelRuntime({mail_parcel_abi_version:()=>1,MailParcelBridge:class{transact(inputJson){
  const input=JSON.parse(inputJson),step=steps[at++];assert.ok(step,`unexpected parcel action ${input.action}`);assert.deepEqual(input,step[0]);
  const output=step[1],outputJson=JSON.stringify(output);captures.push({inputJson,outputJson,input,output});return outputJson;
 }}});
 return{parcel:new ui.MailParcelController(runtime),done(){assert.equal(at,steps.length);}};
}
const parcelPricing={templateIndex:1,itemType:1,shape:0,templatePrice:100,templateDurability:100,quantity:1,currentDura:50,maxDura:100,addedStats:[{stat:1,value:1},{stat:2,value:-1}]};
const parcelSnapshot={bagCapacity:80,items:[{uniqueId:41,container:0,slot:40,pricing:parcelPricing,stamp:false},{uniqueId:42,container:0,slot:41,pricing:{...parcelPricing,templateIndex:2},stamp:false},{uniqueId:99,container:0,slot:0,pricing:{...parcelPricing,templateIndex:838,itemType:0,shape:1,templateDurability:0,currentDura:null,maxDura:null},stamp:true}]};
const parcelSync=(snapshot=parcelSnapshot,gold=700,now=1,owner=parcelOwner)=>({action:'sync',owner,snapshot,gold,now});
test('actual parcel wrapper validates ABI, exact JSON, rejection and malformed schemas',()=>{
 for(const module of[{}, {mail_parcel_abi_version:()=>2}, {mail_parcel_abi_version:()=>1}])assert.throws(()=>actualParcelRuntime(module));
 const f=scriptedParcel([[{action:'status'},parcelOutput()],[{action:'attach',uniqueId:0},{ok:false,error:'Invalid parcel input'}]]);
 assert.equal(f.parcel.call({action:'status'}).ok,true);assert.deepEqual(f.parcel.call({action:'attach',uniqueId:0}),{ok:false,error:'Invalid parcel input'});f.done();
 for(const value of[null,{},parcelOutput({...emptyParcelState,slotLimit:2}),parcelOutput({...emptyParcelState,blockedUniqueIds:[41,41]}),parcelOutput({...emptyParcelState,postage:4294967296}),{...parcelOutput(),extra:true}]){
  const runtime=actualParcelRuntime({mail_parcel_abi_version:()=>1,MailParcelBridge:class{transact(){return JSON.stringify(value);}}});assert.throws(()=>runtime.transact({action:'status'}));
 }
 for(const transact of[()=>'{',()=>{throw Error('controlled parcel export failure');}]){
  const parcel=new ui.MailParcelController(actualParcelRuntime({mail_parcel_abi_version:()=>1,MailParcelBridge:class{transact=transact;}}));assert.equal(parcel.call({action:'status'}).ok,false);assert.match(parcel.error,/draft kept/);
 }
});
test('actual raw snapshot adapter maps Bag2 and exact pricing without manufacturing display authority',()=>{
 const raw={playerObjectId:3,inventoryCapacity:86,inventoryItems:[{uniqueId:41,container:'bag2',slot:0,quantity:1,durabilityCurrent:50,durabilityMax:100,tooltipSource:{info:{item_index:1,item_type:1,shape:0,price:100,durability:100},userItem:{unique_id:41,item_index:1,count:1,added_stats:parcelPricing.addedStats}}},{container:'bag1',slot:1,name:'display only',quantity:1}],beltItems:[],equipmentItems:[]};
 const result=parcelAdapter.projectMailParcelSnapshot(raw);assert.ok(result);assert.equal(result.items[0].slot,40);assert.deepEqual(result.items[0].pricing,parcelPricing);assert.equal(result.items[1].uniqueId,null);assert.equal(result.items[1].pricing,null);
 const drift=structuredClone(raw);drift.inventoryItems[0].tooltipSource.userItem.added_stats=[{stat:1,value:2}];assert.notDeepEqual(parcelAdapter.projectMailParcelSnapshot(drift).items[0].pricing,result.items[0].pricing);
 const bad=structuredClone(raw);bad.inventoryItems[0].tooltipSource.userItem.unique_id=42;assert.equal(parcelAdapter.projectMailParcelSnapshot(bad).items[0].pricing,null);
 assert.equal(parcelAdapter.projectMailParcelSnapshot({...raw,inventoryItems:[raw.inventoryItems[0],raw.inventoryItems[0]]}),null);
});
test('actual shared wrapper/controller selects Bag2 UID, keeps false echoes locked, applies stamp and visibly reconciles stamp loss',()=>{
 const selected={...emptyParcelState,attachmentUniqueIds:[41],blockedUniqueIds:[41]},stamped={...selected,stamped:true,slotLimit:5},two={...stamped,attachmentUniqueIds:[41,42],blockedUniqueIds:[41,42]},lost={...selected,reviewRequired:true,notice:'Parcel inventory changed; review attachments and stamp'};
 const withoutStamp={...parcelSnapshot,items:parcelSnapshot.items.slice(0,2)};
 const f=scriptedParcel([[parcelSync(),parcelOutput()],[{action:'attach',uniqueId:41},parcelOutput(selected,[{uniqueId:41,locked:true}])],[{action:'lock',uniqueId:41,locked:false},parcelOutput(selected)],[{action:'attach',uniqueId:42},{ok:false,error:'Attachment is unavailable or parcel slots are full'}],[{action:'stamp'},parcelOutput(stamped)],[{action:'attach',uniqueId:42},parcelOutput(two,[{uniqueId:42,locked:true}])],[parcelSync(withoutStamp,700,2),parcelOutput(lost,[{uniqueId:42,locked:false}])]]);
 const p=f.parcel;p.sync(parcelOwner,parcelSnapshot,700,1);p.call({action:'attach',uniqueId:41});const lock=p.lock(parcelOwner,41,true);assert.ok(lock);assert.equal(p.enterLock(lock.proof,lock.proof.body),true);assert.equal(p.enterLock(lock.proof,lock.proof.body),false);
 p.call({action:'lock',uniqueId:41,locked:false});assert.deepEqual(p.state.blockedUniqueIds,[41]);assert.equal(p.call({action:'attach',uniqueId:42}).ok,false);p.call({action:'stamp'});p.call({action:'attach',uniqueId:42});p.sync(parcelOwner,withoutStamp,700,2);
 assert.equal(p.state.reviewRequired,true);assert.equal(p.allowsSend(parcelOwner,{name:'R',message:'body',gold:700,itemsIdx:[41,0,0,0,0],stamped:false}),false);f.done();
});
test('actual final Page socket checks immutable source and destination UIDs, equipment ordinals and current snapshot barriers',()=>{
 const snapshot={...parcelSnapshot,items:[...parcelSnapshot.items,{uniqueId:43,container:0,slot:2,pricing:parcelPricing,stamp:false},{uniqueId:44,container:2,slot:0,pricing:parcelPricing,stamp:false},{uniqueId:45,container:4,slot:0,pricing:parcelPricing,stamp:false}]};
 const controller={snapshot,state:{blockedUniqueIds:[41,44]},sync(){}};
 for(const command of[{type:'moveItem',grid:'inventory',from:2,to:40},{type:'moveItem',grid:'inventory',from:40,to:2},{type:'takeBackItem',from:0,to:40},{type:'storeItem',from:40,to:0},{type:'retrieveTradeItem',from:0,to:40},{type:'depositTradeItem',from:40,to:0},{type:'equipItem',grid:'inventory',uniqueId:43,to:0},{type:'removeItem',grid:'inventory',uniqueId:44,to:2},{type:'repairItem',uniqueId:0},{type:'specialRepairItem',uniqueId:0},{type:'mergeItem',gridFrom:'inventory',idFrom:43,gridTo:'inventory',idTo:41}]){
  const page=pageSend(null,()=>{},{allows:()=>false},{mailParcelRef:{current:controller}});assert.equal(page.send(command),false,JSON.stringify(command));assert.equal(page.writes.length,0);
 }
 const command={type:'moveItem',grid:'inventory',from:2,to:3},page=pageSend(null,()=>{command.type='getRanking';command.to=40;},{allows:()=>false},{mailParcelRef:{current:controller}});assert.equal(page.send(command),false);assert.equal(page.writes.length,0);
 const current=pageSend(null,()=>{controller.state.blockedUniqueIds=[43];},{allows:()=>false},{mailParcelRef:{current:controller}});assert.equal(current.send({type:'moveItem',grid:'inventory',from:2,to:3}),false);
 controller.state.blockedUniqueIds=[41];controller.snapshot=null;assert.equal(current.send({type:'moveItem',grid:'inventory',from:2,to:3}),false);
 controller.snapshot=snapshot;assert.equal(current.send({type:'moveItem',grid:'inventory',from:41,to:3}),true,'unselected valid carried UID remains usable');
});
test('actual quote socket reserves before nested listener and distinguishes before-socket failure from unknown delivery',()=>{
 for(const mode of['nested','listenerThrow','socketThrow','substitute']){
  const reserved={...emptyParcelState,pendingQuote:true},unsent={...emptyParcelState,notice:'Postage request was not sent'},steps=[[parcelSync(emptyParcelSnapshot),parcelOutput()],[{action:'quote',now:2},parcelOutput(reserved,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},1)]];
  if(mode==='nested')steps.push([{action:'quote',now:3},parcelOutput(reserved)]);
  if(mode!=='listenerThrow')steps.push([parcelSync(emptyParcelSnapshot,mode==='substitute'?701:700,4),parcelOutput(reserved)]);
  if(mode!=='listenerThrow'&&mode!=='substitute')steps.push([{action:'enter',token:1},parcelOutput(reserved)]);
  steps.push([{action:'finish',token:1},parcelOutput(mode==='listenerThrow'||mode==='substitute'?unsent:reserved)]);
  const f=scriptedParcel(steps),p=f.parcel;p.sync(parcelOwner,emptyParcelSnapshot,700,1);const q=p.quote(parcelOwner,2);assert.ok(q);
  const page=pageSend(null,()=>{if(mode==='nested')assert.equal(p.quote(parcelOwner,3),null);if(mode==='listenerThrow')throw Error('before socket');},{allows:()=>false},{mailParcelRef:{current:p},currentSpellsOwner:()=>parcelOwner,syncMailParcel:gold=>{p.sync(parcelOwner,emptyParcelSnapshot,gold,4);return true;}});
  if(mode==='socketThrow')page.socket.send=()=>{throw Error('socket attempted');};
  if(mode==='nested')assert.equal(page.send(q.payload,{mailQuoteProof:q.proof}),true);else if(mode==='substitute')assert.equal(page.send({...q.payload,gold:701},{mailQuoteProof:q.proof}),false);else assert.throws(()=>page.send(q.payload,{mailQuoteProof:q.proof}));p.finish(q.proof);
  assert.equal(p.state.pendingQuote,mode!=='listenerThrow'&&mode!=='substitute');assert.equal(page.writes.length,mode==='nested'?1:0);f.done();
 }
});
test('actual parcel owner/close timeout cannot reuse uncorrelated reply and MAX cost does not grant send',()=>{
 const pending={...emptyParcelState,pendingQuote:true},timed={...pending,notice:'Postage quote unavailable'},max={...emptyParcelState,notice:'Postage quote unavailable'},other={...parcelOwner,sessionGeneration:9};
 const f=scriptedParcel([[parcelSync(emptyParcelSnapshot),parcelOutput()],[{action:'quote',now:2},parcelOutput(pending,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},1)],[{action:'enter',token:1},parcelOutput(pending)],[{action:'finish',token:1},parcelOutput(pending)],[parcelSync(emptyParcelSnapshot,700,6000),parcelOutput(timed)],[{action:'cancel'},parcelOutput(pending)],[parcelSync(emptyParcelSnapshot,700,6001,other),parcelOutput(timed)],[{action:'quote',now:6002},parcelOutput(timed)],[{action:'cost',cost:70},parcelOutput({...emptyParcelState,notice:'Postage quote unavailable'})],[{action:'quote',now:6003},parcelOutput(pending,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},2)],[{action:'enter',token:2},parcelOutput(pending)],[{action:'finish',token:2},parcelOutput(pending)],[{action:'cost',cost:4294967295},parcelOutput(max)],[{action:'quote',now:6004},parcelOutput(max)]]);
 const p=f.parcel;p.sync(parcelOwner,emptyParcelSnapshot,700,1);let q=p.quote(parcelOwner,2);p.enter(q.proof,q.proof.body);p.finish(q.proof);p.sync(parcelOwner,emptyParcelSnapshot,700,6000);p.call({action:'cancel'});p.sync(other,emptyParcelSnapshot,700,6001);assert.equal(p.quote(other,6002),null);p.call({action:'cost',cost:70});assert.equal(p.state.quoteReady,false);q=p.quote(other,6003);assert.ok(q);p.enter(q.proof,q.proof.body);p.finish(q.proof);p.call({action:'cost',cost:4294967295});assert.equal(p.state.postage,null);assert.equal(p.quote(other,6004),null);f.done();
});
function selectedQuoteSteps(){
 const selected={...emptyParcelState,attachmentUniqueIds:[41],blockedUniqueIds:[41]},pending={...selected,pendingQuote:true},ready={...selected,postage:72,quoteReady:true};
 return{selected,pending,ready,steps:[[parcelSync(),parcelOutput()],[{action:'attach',uniqueId:41},parcelOutput(selected,[{uniqueId:41,locked:true}])],[{action:'quote',now:2},parcelOutput(pending,[],{gold:700,itemsIdx:[41,0,0,0,0],stamped:false},1)],[{action:'enter',token:1},parcelOutput(pending)],[{action:'finish',token:1},parcelOutput(pending)],[{action:'cost',cost:72},parcelOutput(ready)]]};
}
function warmSelected(p){p.sync(parcelOwner,parcelSnapshot,700,1);p.call({action:'attach',uniqueId:41});const q=p.quote(parcelOwner,2);assert.ok(q);assert.equal(p.enter(q.proof,q.proof.body),true);p.finish(q.proof);p.call({action:'cost',cost:72});}
test('actual final Page parcel send rechecks live tuple, missing raw authority and equipment pending after listeners',()=>{
 for(const mode of['stats','snapshotBarrier','equipmentBusy']){
  const golden=selectedQuoteSteps(),drift=structuredClone(parcelSnapshot);drift.items[0].pricing.addedStats=[{stat:1,value:2}];
  const snapshot=mode==='stats'?drift:mode==='snapshotBarrier'?null:parcelSnapshot,state=mode==='stats'?{...golden.ready,quoteReady:false}:mode==='snapshotBarrier'?{...golden.selected}:golden.ready;
  const f=scriptedParcel([...golden.steps,[parcelSync(snapshot,700,10),parcelOutput(state)],[parcelSync(snapshot,700,10),parcelOutput(state)]]);warmSelected(f.parcel);
  const owner={...parcelOwner},live={owner,mail:actualMail(),enabled:true},d=mailDispatcher(()=>live),draft={to:'R',subject:'local',body:'body',goldText:'700',items:[],attachmentUniqueIds:[41],stamped:false};d.composer.remember(owner,'item',draft);
  const payload={name:'R',message:'body',gold:700,itemsIdx:[41,0,0,0,0],stamped:false},reservation=d.composer.reserve(owner,'item',payload),proof=d.prepare('sendMail',null,null,reservation);assert.ok(proof);
  let afterListener=false;const page=pageSend(d,()=>{afterListener=true;},undefined,{mailParcelRef:{current:f.parcel},currentSpellsOwner:()=>owner,syncMailParcel:()=>{assert.equal(afterListener,true);f.parcel.sync(owner,snapshot,700,10);return true;},mailParcelItemsIdle:()=>mode!=='equipmentBusy'});
  assert.equal(page.send({...payload,type:'sendMail'},{mailProof:proof}),false);assert.equal(page.writes.length,0);assert.equal(d.composer.finish(reservation,'definitelyUnsent'),'definitelyUnsent');assert.deepEqual(d.composer.draft(owner),draft);f.done();
 }
});
test('actual Page current ACK failure retains selected locks, success unlocks exact UID without changing authoritative gold',()=>{
 const golden=selectedQuoteSteps(),sync=[parcelSync(parcelSnapshot,700,10),parcelOutput(golden.ready)],f=scriptedParcel([...golden.steps,sync,sync,sync,sync,[{action:'complete'},parcelOutput(emptyParcelState,[{uniqueId:41,locked:false}])]]);warmSelected(f.parcel);
 const owner={...parcelOwner},live={owner,mail:actualMail(),enabled:true},d=mailDispatcher(()=>live),draft={to:'R',subject:'local',body:'body',goldText:'700',items:[],attachmentUniqueIds:[41],stamped:false};d.composer.remember(owner,'item',draft);
 const payload={name:'R',message:'body',gold:700,itemsIdx:[41,0,0,0,0],stamped:false},page=pageSend(d,()=>{},undefined,{mailParcelRef:{current:f.parcel},currentSpellsOwner:()=>owner,syncMailParcel:()=>{f.parcel.sync(owner,parcelSnapshot,700,10);return true;}});
 const original=composePage(),unlocks=[],views=[];original.deps.mailSourceSocket=page.socket;original.deps.mailDispatcherRef.current=d;original.deps.mailParcelRef.current=f.parcel;original.deps.worldRef.current.gold=1234;original.deps.sendMailParcelLocks=locks=>unlocks.push(...locks);original.deps.presentMailCompatibility=view=>views.push(view);
 const ack=pageFunctions(['captureMailGatewayEvent'],original.deps).captureMailGatewayEvent;
 function send(){const r=d.composer.reserve(owner,'item',payload),p=d.prepare('sendMail',null,null,r);assert.equal(page.send({...payload,type:'sendMail'},{mailProof:p}),true);d.composer.finish(r,'confirmedSend');}
 send();ack({type:'packet',packet:'MailSent',payload:{result:-1}},1);assert.deepEqual(f.parcel.state.blockedUniqueIds,[41]);assert.deepEqual(d.composer.draft(owner),draft);assert.deepEqual(unlocks,[]);
 send();ack({type:'packet',packet:'MailSent',payload:{result:1}},1);assert.deepEqual(unlocks,[{uniqueId:41,locked:false}]);assert.deepEqual(views,['inbox']);assert.equal(original.deps.worldRef.current.gold,1234);assert.equal(d.composer.draft(owner),null);f.done();
});
test('selected unknown pricing stays visible and blocks parcel while unselected unknown metadata permits gold',()=>{
 const unknown={...parcelSnapshot,items:[{...parcelSnapshot.items[0],pricing:null}]},notice='Attachment pricing or stamp is unavailable; draft kept',selected={...emptyParcelState,attachmentUniqueIds:[41],blockedUniqueIds:[41],notice};
 const f=scriptedParcel([[parcelSync(unknown),parcelOutput()],[{action:'attach',uniqueId:41},parcelOutput(selected,[{uniqueId:41,locked:true}])],[{action:'quote',now:2},parcelOutput(selected)]]);f.parcel.sync(parcelOwner,unknown,700,1);assert.equal(f.parcel.call({action:'attach',uniqueId:41}).ok,true);assert.equal(f.parcel.quote(parcelOwner,2),null);assert.match(f.parcel.state.notice,/pricing/);assert.deepEqual(f.parcel.state.attachmentUniqueIds,[41]);f.done();
});
test('actual Page item mutation invalidates raw authority before next full snapshot, older connection leaves it untouched',()=>{
 const f=composePage();f.deps.mailParcelRawRef.current={owner:f.owner,snapshot:parcelSnapshot};let invalidations=0;f.deps.syncMailParcel=()=>{invalidations++;return true;};
 // Extract again with the new controlled sync dependency; the product function is unchanged.
 const api=pageFunctions(['captureMailGatewayEvent'],f.deps);api.captureMailGatewayEvent({type:'packet',packet:'ItemDurability',payload:{}},0);assert.ok(f.deps.mailParcelRawRef.current);
 api.captureMailGatewayEvent({type:'packet',packet:'ItemDurability',payload:{}},1);assert.equal(f.deps.mailParcelRawRef.current,null);assert.equal(invalidations,1);
});
test('actual Page packetless and nonpacket events leave raw parcel authority intact',()=>{
 const f=composePage(),authority={owner:f.owner,snapshot:parcelSnapshot};f.deps.mailParcelRawRef.current=authority;let invalidations=0;
 f.deps.syncMailParcel=()=>{invalidations++;return false;};const api=pageFunctions(['captureMailGatewayEvent'],f.deps);
 for(const event of[{type:'packet',payload:{}},{type:'packet',packet:null,payload:{}},{type:'packet',packet:42,payload:{}},{type:'error',message:'connection notice'},{type:'realmInfo',packet:'ItemDurability',payload:{}}]){
  assert.doesNotThrow(()=>api.captureMailGatewayEvent(event,1));assert.equal(f.deps.mailParcelRawRef.current,authority);assert.equal(invalidations,0);
 }
 api.captureMailGatewayEvent({type:'packet',packet:'ItemRemoved',payload:{}},1);assert.equal(f.deps.mailParcelRawRef.current,null);assert.equal(invalidations,1,'a real item mutation still creates the barrier');
});
test('actual Page ownerless Cost retires only the entered current-connection slot, then next owner can quote and send',()=>{
 const pending={...emptyParcelState,pendingQuote:true},nextOwner={...parcelOwner,sessionGeneration:9};
 const malformed=[undefined,-1,1.5,4294967296,'70'];
 const prefix=[[parcelSync(emptyParcelSnapshot),parcelOutput()],[{action:'quote',now:2},parcelOutput(pending,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},1)],[{action:'enter',token:1},parcelOutput(pending)],[{action:'finish',token:1},parcelOutput(pending)]];
 const ownerlessSync={action:'sync',owner:null,snapshot:null,gold:0,now:10};
 const steps=[...prefix,...malformed.map(()=>[ownerlessSync,parcelOutput(pending)]),[ownerlessSync,parcelOutput(pending)],[{action:'cost',cost:70},parcelOutput()], [parcelSync(emptyParcelSnapshot,700,11,nextOwner),parcelOutput()],[{action:'quote',now:12},parcelOutput(pending,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},2)],[parcelSync(emptyParcelSnapshot,700,13,nextOwner),parcelOutput(pending)],[{action:'enter',token:2},parcelOutput(pending)],[{action:'finish',token:2},parcelOutput(pending)]];
 const scripted=scriptedParcel(steps),p=scripted.parcel;p.sync(parcelOwner,emptyParcelSnapshot,700,1);let q=p.quote(parcelOwner,2);p.enter(q.proof,q.proof.body);p.finish(q.proof);
 const f=composePage();f.live.owner=null;f.deps.mailParcelRef.current=p;f.deps.syncMailParcel=()=>{p.sync(null,null,0,10);return true;};const raw=f.deps.mailRawRef.current,locks=[],views=[];
 f.deps.sendMailParcelLocks=l=>locks.push(...l);f.deps.presentMailCompatibility=v=>views.push(v);const capture=pageFunctions(['captureMailGatewayEvent'],f.deps).captureMailGatewayEvent;
 capture({type:'packet',packet:'MailCost',payload:{cost:70}},0);assert.equal(p.state.pendingQuote,true);assert.equal(p.retireCost(2,70),false);
 for(const cost of malformed){capture({type:'packet',packet:'MailCost',payload:{cost}},1);assert.equal(p.state.pendingQuote,true);assert.equal(p.state.quoteReady,false);}
 assert.equal(p.retireCost(2,70),false,'ownerless helper still checks the entered connection');assert.equal(p.state.pendingQuote,true);
 capture({type:'packet',packet:'MailCost',payload:{cost:70}},1);assert.equal(p.state.pendingQuote,false);assert.equal(p.state.quoteReady,false);assert.equal(p.state.postage,null);assert.equal(p.retireCost(1,70),false,'no entered flight cannot be retired twice');
 assert.equal(f.deps.mailRawRef.current,raw);assert.deepEqual(locks,[]);assert.deepEqual(views,[]);
 f.live.owner=nextOwner;p.sync(nextOwner,emptyParcelSnapshot,700,11);q=p.quote(nextOwner,12);assert.ok(q);
 const page=pageSend(f.dispatcher,()=>{},undefined,{mailParcelRef:{current:p},currentSpellsOwner:()=>f.live.owner,syncMailParcel:gold=>{p.sync(nextOwner,emptyParcelSnapshot,gold,13);return true;}});
 assert.equal(page.send(q.payload,{mailQuoteProof:q.proof}),true);p.finish(q.proof);
 const draft={...f.draft,body:'body',goldText:'0'};assert.equal(f.dispatcher.composer.remember(nextOwner,'next',draft),true);const payload={name:'R',message:'body',gold:0,itemsIdx:[0,0,0,0,0],stamped:false},reservation=f.dispatcher.composer.reserve(nextOwner,'next',payload),proof=f.dispatcher.prepare('sendMail',null,null,reservation);
 assert.ok(proof);assert.equal(page.send({...payload,type:'sendMail'},{mailProof:proof}),true);assert.equal(page.writes.length,2);scripted.done();
});
test('actual Page ownerless ACK retires unknown entered delivery without completing draft/locks/inbox, then next owner uses same socket',()=>{
 const f=composePage({socketThrow:true});assert.equal(f.sendMailMessage(submitInput(f)),'outcomeUnknown');f.live.owner=null;
 const locks=[],views=[],raw=f.deps.mailRawRef.current;f.deps.sendMailParcelLocks=l=>locks.push(...l);f.deps.presentMailCompatibility=v=>views.push(v);const capture=pageFunctions(['captureMailGatewayEvent'],f.deps).captureMailGatewayEvent;
 const probeOwner={...f.owner,sessionGeneration:9},sender=f.dispatcher.composer;
 // pending(null) is intentionally invisible. Observe the actual public gate
 // through a same-socket owner, then restore the real owner-null boundary.
 function proveBlocked(){assert.equal(sender.remember(probeOwner,'probe',f.draft),false);assert.equal(sender.reserve(probeOwner,'probe',f.wirePayload),null);assert.equal(sender.draft(probeOwner),null);sender.sync(null);}
 function proveAvailable(){assert.equal(sender.remember(probeOwner,'probe',f.draft),true);assert.deepEqual(sender.draft(probeOwner),f.draft);const reservation=sender.reserve(probeOwner,'probe',f.wirePayload);assert.ok(reservation);assert.equal(sender.finish(reservation,'definitelyUnsent'),'definitelyUnsent');sender.sync(null);}
 capture({type:'packet',packet:'MailSent',payload:{result:1}},0);assert.equal(f.dispatcher.composer.pending(f.owner),true,'old connection cannot even sync current owner');
 proveBlocked();for(const result of[undefined,'1',1.5,NaN,2147483648]){capture({type:'packet',packet:'MailSent',payload:{result}},1);proveBlocked();}
 assert.equal(f.dispatcher.composer.retireAcknowledgement(2,1,f.send.socket),null);proveBlocked();
 capture({type:'packet',packet:'MailSent',payload:{result:1}},1);proveAvailable();assert.equal(f.dispatcher.composer.retireAcknowledgement(1,1,f.send.socket),null);assert.deepEqual(locks,[]);assert.deepEqual(views,[]);assert.equal(f.deps.mailRawRef.current,raw);
 capture({type:'packet',packet:'MailSent',payload:{result:1}},1);assert.deepEqual(locks,[]);assert.deepEqual(views,[]);proveAvailable();
 const next={...f.owner,sessionGeneration:9};f.live.owner=next;f.deps.mailRenderOwner=next;const p=f.deps.mailParcelRef.current;
 assert.equal(f.dispatcher.composer.remember(next,'compose-1',f.draft),true);p.sync(next,emptyParcelSnapshot,700,11);const quote=p.quote(next,12);assert.ok(quote);
 f.send.socket.send=body=>f.send.writes.push(body);assert.equal(f.send.send(quote.payload,{mailQuoteProof:quote.proof}),true);p.finish(quote.proof);capture({type:'packet',packet:'MailCost',payload:{cost:70}},1);assert.equal(p.state.quoteReady,true);
 const nextApi=pageFunctions(['sendMailMessage','dispatchMailCommand'],f.deps);assert.equal(nextApi.sendMailMessage(submitInput(f)),'confirmedSend');assert.equal(f.dispatcher.composer.pending(next),true);assert.equal(f.send.writes.length,2);
});
test('retirement APIs reject no-flight, reserved and definitely-unsent requests without granting completion',()=>{
 const sender=mailSender(),draft={to:'R',subject:'',body:'body',goldText:'0',items:[]},payload={name:'R',message:'body',gold:0,itemsIdx:[0,0,0,0,0],stamped:false};
 sender.sync(null);assert.equal(sender.retireAcknowledgement(1,1,sender.mailTestSocket),null);sender.remember(parcelOwner,'one',draft);const proof=sender.reserve(parcelOwner,'one',payload);assert.ok(proof);
 assert.equal(sender.retireAcknowledgement(1,1,sender.mailTestSocket),null);assert.equal(sender.pending(parcelOwner),true);assert.deepEqual(sender.draft(parcelOwner),draft);
 assert.equal(sender.finish(proof,'definitelyUnsent'),'definitelyUnsent');sender.sync(null);assert.equal(sender.retireAcknowledgement(1,1,sender.mailTestSocket),null);
 const reservedPage=composePage();assert.ok(reservedPage.dispatcher.composer.reserve(reservedPage.owner,'compose-1',reservedPage.wirePayload));reservedPage.live.owner=null;
 reservedPage.deps.presentMailCompatibility=()=>{throw Error('reserved ACK cannot open inbox');};reservedPage.deps.sendMailParcelLocks=()=>{throw Error('reserved ACK cannot complete parcel');};
 pageFunctions(['captureMailGatewayEvent'],reservedPage.deps).captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:1}},1);assert.equal(reservedPage.dispatcher.composer.pending(null),false);assert.equal(reservedPage.dispatcher.composer.retireAcknowledgement(1,1,reservedPage.send.socket),null,'sync withdraws unsent reservation; reply grants no completion');
 const pending={...emptyParcelState,pendingQuote:true},unsent={...emptyParcelState,notice:'Postage request was not sent'},nullSync={action:'sync',owner:null,snapshot:null,gold:0,now:3};
 function ownerlessPageCost(parcel){const page=composePage();page.live.owner=null;page.deps.mailParcelRef.current=parcel;page.deps.syncMailParcel=()=>{parcel.sync(null,null,0,3);return true;};page.deps.presentMailCompatibility=()=>{throw Error('retirement cannot open inbox');};page.deps.sendMailParcelLocks=()=>{throw Error('retirement cannot complete locks');};pageFunctions(['captureMailGatewayEvent'],page.deps).captureMailGatewayEvent({type:'packet',packet:'MailCost',payload:{cost:70}},1);}
 const f=scriptedParcel([[parcelSync(emptyParcelSnapshot),parcelOutput()],[{action:'quote',now:2},parcelOutput(pending,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},1)],[nullSync,parcelOutput()]]);f.parcel.sync(parcelOwner,emptyParcelSnapshot,700,1);f.parcel.quote(parcelOwner,2);ownerlessPageCost(f.parcel);assert.equal(f.parcel.retireCost(1,70),false);assert.equal(f.parcel.state.pendingQuote,false);f.done();
 const g=scriptedParcel([[parcelSync(emptyParcelSnapshot),parcelOutput()],[{action:'quote',now:2},parcelOutput(pending,[],{gold:700,itemsIdx:[0,0,0,0,0],stamped:false},1)],[{action:'finish',token:1},parcelOutput(unsent)],[nullSync,parcelOutput()]]);g.parcel.sync(parcelOwner,emptyParcelSnapshot,700,1);const q=g.parcel.quote(parcelOwner,2);g.parcel.finish(q.proof);ownerlessPageCost(g.parcel);assert.equal(g.parcel.retireCost(1,70),false);g.done();
});
test('actual Page rejects malformed MailSent without consuming current entered flight; valid reject then fresh success still settle',()=>{
 const f=composePage();assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');
 const locks=[];f.deps.sendMailParcelLocks=l=>locks.push(...l);
 const capture=pageFunctions(['captureMailGatewayEvent'],f.deps).captureMailGatewayEvent,notice=f.dispatcher.composer.notice;
 for(const result of[0,2,1.5,-1.5,undefined,null,'1',NaN,2147483648]){
  capture({type:'packet',packet:'MailSent',payload:{result}},1);
  assert.equal(f.dispatcher.composer.pending(f.owner),true,`malformed ${String(result)} must retain entered slot`);
  assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);assert.equal(f.dispatcher.composer.notice,notice);
  assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent');assert.equal(f.send.writes.length,1);
  assert.deepEqual(locks,[]);assert.deepEqual(f.views,[]);
 }
 capture({type:'packet',packet:'MailSent',payload:{result:-1}},1);
 assert.equal(f.dispatcher.composer.pending(f.owner),false);assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);
 assert.deepEqual(locks,[]);assert.deepEqual(f.views,[]);
 assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');assert.equal(f.send.writes.length,2);assert.equal(f.dispatcher.composer.pending(f.owner),true);
 capture({type:'packet',packet:'MailSent',payload:{result:1}},1);
 assert.equal(f.dispatcher.composer.pending(f.owner),false);assert.equal(f.dispatcher.composer.draft(f.owner),null);assert.deepEqual(f.views,['inbox']);
});
test('actual Page malformed ownerless MailSent retains tombstone; valid old ACK only retires before fresh same-socket send and ACK',()=>{
 for(const oldResult of[1,-1]){
  const f=composePage({socketThrow:true});assert.equal(f.sendMailMessage(submitInput(f)),'outcomeUnknown');f.live.owner=null;
  const sender=f.dispatcher.composer,next={...f.owner,sessionGeneration:9},locks=[],views=[],raw=f.deps.mailRawRef.current;
  f.deps.sendMailParcelLocks=l=>locks.push(...l);f.deps.presentMailCompatibility=v=>views.push(v);
  const capture=pageFunctions(['captureMailGatewayEvent'],f.deps).captureMailGatewayEvent;
  function proveBlocked(){assert.equal(sender.remember(next,'probe',f.draft),false);assert.equal(sender.reserve(next,'probe',f.wirePayload),null);assert.equal(sender.draft(next),null);sender.sync(null);}
  proveBlocked();
  for(const result of[0,2,1.5,-1.5,undefined,null,'1',NaN,2147483648]){
   capture({type:'packet',packet:'MailSent',payload:{result}},1);proveBlocked();
   assert.deepEqual(locks,[]);assert.deepEqual(views,[]);assert.equal(f.deps.mailRawRef.current,raw);
  }
  capture({type:'packet',packet:'MailSent',payload:{result:oldResult}},0);proveBlocked();
  capture({type:'packet',packet:'MailSent',payload:{result:oldResult}},1);
  assert.deepEqual(locks,[]);assert.deepEqual(views,[]);assert.equal(f.deps.mailRawRef.current,raw);
  assert.equal(sender.retireAcknowledgement(1,oldResult,f.send.socket),null,'old slot may retire only once');
  f.live.owner=next;f.deps.mailRenderOwner=next;
  assert.equal(sender.remember(next,'compose-1',f.draft),true);assert.deepEqual(sender.draft(next),f.draft);
  const p=f.deps.mailParcelRef.current;p.sync(next,emptyParcelSnapshot,700,11);const quote=p.quote(next,12);assert.ok(quote);
  f.send.socket.send=body=>f.send.writes.push(body);
  assert.equal(f.send.send(quote.payload,{mailQuoteProof:quote.proof}),true);p.finish(quote.proof);
  capture({type:'packet',packet:'MailCost',payload:{cost:70}},1);assert.equal(p.state.quoteReady,true);
  const nextApi=pageFunctions(['sendMailMessage','dispatchMailCommand'],f.deps);
  assert.equal(nextApi.sendMailMessage(submitInput(f)),'confirmedSend');assert.equal(sender.pending(next),true);assert.equal(f.send.writes.length,2);
  const nextNotice=sender.notice;
  for(const result of[0,2,1.5]){capture({type:'packet',packet:'MailSent',payload:{result}},1);assert.equal(sender.pending(next),true);assert.deepEqual(sender.draft(next),f.draft);assert.equal(sender.notice,nextNotice);assert.deepEqual(views,[]);}
  capture({type:'packet',packet:'MailSent',payload:{result:1}},1);
  assert.equal(sender.pending(next),false);assert.equal(sender.draft(next),null);assert.deepEqual(views,['inbox']);assert.deepEqual(locks,[]);
 }
});
test('actual Page no-slot MailSent cannot complete normal or ownerless draft; later real send requires its own legal ACK',()=>{
 for(const ownerless of[false,true]){
  const f=composePage(),sender=f.dispatcher.composer,locks=[],views=[];
  f.deps.sendMailParcelLocks=l=>locks.push(...l);f.deps.presentMailCompatibility=v=>views.push(v);
  const capture=pageFunctions(['captureMailGatewayEvent'],f.deps).captureMailGatewayEvent;
  if(ownerless){f.live.owner=null;capture({type:'packet',packet:'NewMail',payload:{}},1);}
  const saved=sender.draft(f.live.owner),notice=sender.notice;
  for(const result of[0,2,1.5,1,-1]){
   capture({type:'packet',packet:'MailSent',payload:{result}},1);assert.equal(sender.pending(f.live.owner),false);
   assert.deepEqual(sender.draft(f.live.owner),saved);assert.equal(sender.notice,notice);assert.deepEqual(locks,[]);assert.deepEqual(views,[]);
  }
  f.live.owner=f.owner;f.rememberMailDraft(f.draft);assert.deepEqual(sender.draft(f.owner),f.draft);
  assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');assert.equal(f.send.writes.length,1);assert.equal(sender.pending(f.owner),true);
  capture({type:'packet',packet:'MailSent',payload:{result:1}},1);assert.equal(sender.pending(f.owner),false);assert.equal(sender.draft(f.owner),null);assert.deepEqual(views,['inbox']);
 }
});
test('actual component emits current numeric Rust-selected UIDs and stamp, not display references',()=>{
 const calls=[],deps={onSendMail:d=>calls.push(d),composeState:{pending:false},composeTo:'R',composeSubject:'local',composeBody:'body',composeGold:'700',composeItems:[],parcelState:{attachmentUniqueIds:[41,42],stamped:true}};
 windowComposeFunctions(['submitCompose'],deps).submitCompose();assert.deepEqual(calls,[{to:'R',subject:'local',body:'body',gold:700,items:undefined,attachmentUniqueIds:[41,42],stamped:true}]);
});
test('parcel actual wrapper transcripts require explicit compiled Rust replay',()=>{
 assert.ok(parcelSessions.length>0);assert.ok(parcelSessions.some(s=>s.some(r=>r.input.action==='enter')));assert.ok(parcelSessions.some(s=>s.some(r=>r.input.action==='attach')));
 for(const session of parcelSessions)for(const row of session){assert.deepEqual(JSON.parse(row.inputJson),row.input);assert.deepEqual(JSON.parse(row.outputJson),row.output);}
 if(fixtureDir)writeFileSync(resolve(fixtureDir,'mail-parcel-port.json'),JSON.stringify(parcelSessions));
});
test('compose port captures are replayable by compiled Rust, no JS policy is accepted as Rust evidence',()=>{
 assert.ok(composeCaptures.some(c=>c.kind==='send'));assert.ok(composeCaptures.some(c=>c.kind==='message'&&!c.output.ok));
 for(const capture of composeCaptures){assert.equal(typeof capture.inputJson,'string');assert.equal(typeof capture.outputJson,'string');assert.deepEqual(JSON.parse(capture.inputJson),capture.input);assert.deepEqual(JSON.parse(capture.outputJson),capture.output);}
 if(fixtureDir)writeFileSync(resolve(fixtureDir,'mail-compose-port.json'),JSON.stringify(composeCaptures));
});

test('mail additive cached getter is the same facade and only actual current OPEN allocates Core stream',()=>{
 const count=sendSlotSessions.length,runtime=actualSendSlotFactory(recordedSendSlotModule(),true),slot=runtime.getMailSendSlot();assert.equal(runtime.getMailSendSlot(),slot);assert.equal(sendSlotSessions.length,count+1);
 const a={},b={},initial=slot.transact({op:'status'}).state;assert.equal(initial.stream,null);
 assert.equal(slot.observeOpen(a,b,true),false);assert.equal(slot.observeOpen(a,a,false),false);assert.equal(slot.transact({op:'status'}).state.stream,null);
 assert.equal(slot.observeOpen(a,a,true),true);const stream=slot.streamFor(a);assert.deepEqual(stream,{run:'1',connection:'1'});
 assert.equal(slot.observeOpen(a,a,true),true);assert.deepEqual(slot.streamFor(a),stream);
 assert.throws(()=>slot.transact({op:'open'}),/current socket OPEN/);assert.equal(slot.observeOpen(b,b,true),true);assert.deepEqual(slot.streamFor(b),{run:'1',connection:'2'});
 assert.equal(slot.observeOpen(a,a,true),false,'an already known old socket never replaces a later stream');assert.deepEqual(slot.streamFor(a),stream);
});
test('actual compiled Core draft gold getter is read only, accepts legacy raw gold and follows full Core draft changes',()=>{
 const slot=actualSendSlotFactory(recordedSendSlotModule()),owner={connectionGeneration:'1',sessionGeneration:'2',ownerRevision:'0',playerObjectId:'3'};
 const raw={to:'R',subject:'s',body:'body',goldText:'0x7',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 assert.equal(slot.draftGold(),null);const incarnation=slot.transact({op:'mount'}).state.incarnation;
 assert.equal(slot.transact({op:'sync',incarnation,owner}).matched,true);
 assert.equal(slot.transact({op:'remember',incarnation,key:'A',draft:raw}).matched,true);
 const before=slot.transact({op:'status'}).state,rows=sendSlotSessions.at(-1).length;
 assert.equal(before.stream,null,'projection needs no socket flight');assert.equal(slot.draftGold(),7);assert.equal(slot.draftGold(),7);
 assert.equal(sendSlotSessions.at(-1).length,rows,'getter cannot add an old transact transcript row');
 assert.deepEqual(slot.transact({op:'status'}).state,before,'getter cannot advance Core draft generation');
 const changed={...raw,body:'changed',attachmentUniqueIds:['41'],attachmentUniqueIdsPresent:true,stamped:true,stampedPresent:true};
 assert.equal(slot.transact({op:'remember',incarnation,key:'A',draft:changed}).matched,true);assert.equal(slot.draftGold(),7);
 assert.equal(slot.transact({op:'remember',incarnation,key:'A',draft:{...changed,goldText:''}}).matched,true);assert.equal(slot.draftGold(),0);
 assert.equal(slot.transact({op:'remember',incarnation,key:'A',draft:{...changed,goldText:'bad'}}).matched,true);assert.equal(slot.draftGold(),null);
 assert.equal(slot.transact({op:'sync',incarnation,owner:null}).matched,true);assert.equal(slot.draftGold(),null);
});
test('old Core bridge and malformed optional gold getter keep old send slot while common projection fails closed',()=>{
 const recorded=recordedSendSlotModule(),owner={connectionGeneration:1,sessionGeneration:2,ownerRevision:0,playerObjectId:3};
 const draft={to:'R',subject:'s',body:'body',goldText:'7',items:[]};
 const absent={mail_send_slot_abi_version:()=>1,MailSendSlotBridge:class{constructor(){this.bridge=new recorded.MailSendSlotBridge();}transact(input){return this.bridge.transact(input);}}};
 const oldSlot=actualSendSlotFactory(absent),sender=new ui.MailComposeSender(()=>oldSlot),socket={};
 assert.equal(sender.observeOpen(socket,socket,true),true);assert.equal(sender.remember(owner,'old',draft),true);
 assert.equal(sender.composeSnapshot(owner)?.gold,null);assert.equal(oldSlot.draftGold(),null);
 assert.deepEqual(sender.draft(owner),draft,'missing additive getter cannot withdraw old Core draft');
 for(const value of[undefined,null,'7',NaN,Infinity,-1,-0,0x100000000]){
  const module={mail_send_slot_abi_version:()=>1,MailSendSlotBridge:class{constructor(){this.bridge=new compiledSendSlot.MailSendSlotBridge();}
   transact(input){return this.bridge.transact(input);}draft_gold(){return value;}}};
  assert.equal(actualSendSlotFactory(module).draftGold(),null);
 }
 const throwing={mail_send_slot_abi_version:()=>1,MailSendSlotBridge:class{constructor(){this.bridge=new compiledSendSlot.MailSendSlotBridge();}
  transact(input){return this.bridge.transact(input);}draft_gold(){throw Error('getter failure');}}};
 assert.equal(actualSendSlotFactory(throwing).draftGold(),null);
 const slot=actualSendSlotFactory(recordedSendSlotModule()),first=new ui.MailComposeSender(()=>slot);
 assert.equal(first.remember(owner,'A',draft),true);assert.equal(first.composeSnapshot(owner)?.gold,7);
 const replacement=new ui.MailComposeSender(()=>slot);replacement.sync(owner);
 assert.equal(first.composeSnapshot(owner),null,'old sender incarnation cannot borrow a new mount');
 assert.equal(replacement.composeSnapshot(owner)?.gold,7);
});
test('mail new ABI missing malformed version exports requests and responses fail closed without legacy send fallback',()=>{
 for(const module of[{}, {mail_send_slot_abi_version:()=>2}, {mail_send_slot_abi_version:()=> '1'}, {mail_send_slot_abi_version:()=>1}]){
  const f=composePage({slotModule:module});assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent');assert.deepEqual(f.send.writes,[]);assert.deepEqual(f.dispatcher.composer.draft(f.owner),f.draft);assert.match(f.dispatcher.composer.notice,/unavailable|authorization/i);
 }
 const unavailable=actualSendSlotFactory({},true),cached=unavailable.getMailSendSlot(),first=new ui.MailComposeSender(()=>cached),draft={to:'R',subject:'kept',body:'body',goldText:'0',items:[]};
 assert.equal(first.remember(parcelOwner,'missing',draft),true);const remounted=new ui.MailComposeSender(()=>unavailable.getMailSendSlot());remounted.sync(parcelOwner);assert.deepEqual(remounted.draft(parcelOwner),draft);assert.equal(remounted.reserve(parcelOwner,'missing',{name:'R',message:'body',gold:0,itemsIdx:[0,0,0,0,0],stamped:false}),null);assert.equal(cached.transact({op:'status'}).ok,false,'passive draft cache never fabricates a Core state or flight');
 const invalid=[null,[],{}, {ok:true}, {ok:false,error:3}, {ok:false,error:'bad',extra:true}];
 const base=new compiledSendSlot.MailSendSlotBridge(),valid=JSON.parse(base.transact(JSON.stringify({op:'status'})));
 for(const value of['0','01','+1','18446744073709551616',9007199254740992])invalid.push({...valid,state:{...valid.state,generation:value}});
 invalid.push({...valid,matched:1},{...valid,completion:'other'},{...valid,state:{...valid.state,extra:true}},{...valid,state:{...valid.state,draft:{}}});
 for(const malformed of invalid){let calls=0;const module={mail_send_slot_abi_version:()=>1,MailSendSlotBridge:class{transact(){return JSON.stringify(++calls===1?valid:malformed);}}},slot=actualSendSlotFactory(module);assert.throws(()=>slot.transact({op:'status'}));}
 {let calls=0;const duplicate=JSON.stringify(valid).replace('{','{"ok":true,');const slot=actualSendSlotFactory({mail_send_slot_abi_version:()=>1,MailSendSlotBridge:class{transact(){return ++calls===1?JSON.stringify(valid):duplicate;}}});assert.throws(()=>slot.transact({op:'status'}),/Noncanonical/);}
 const rawPort=new (recordedSendSlotModule().MailSendSlotBridge)();assert.equal(JSON.parse(rawPort.transact('{"op":"open","op":"status"}')).ok,false);
 const slot=actualSendSlotFactory(recordedSendSlotModule());
 for(const input of[{op:'status',unknown:true},{op:'sync',incarnation:1,owner:null},{op:'sync',incarnation:'01',owner:null},{op:'sync',incarnation:'18446744073709551616',owner:null},{op:'ack',incarnation:null,stream:{run:'1',connection:'1'},result:2},{op:'remember',incarnation:'1',key:'A'}])assert.throws(()=>slot.transact(input),/request/);
 // Byte-boundary fixtures measure UTF-8 independently with Node Buffer. The
 // validator itself is the actual AST-extracted slotUtf8, never a test copy.
 const atBytes=limit=>'中'.repeat(Math.floor(limit/3))+'a'.repeat(limit%3);
 const rawDraft={to:'R',subject:'s',body:'body',goldText:'0',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 const stringOwner={connectionGeneration:'1',sessionGeneration:'2',ownerRevision:'0',playerObjectId:'3'};
 const readySlot=()=>{const slot=actualSendSlotFactory(recordedSendSlotModule()),incarnation=slot.transact({op:'mount'}).state.incarnation,source={};assert.equal(slot.observeOpen(source,source,true),true);assert.equal(slot.transact({op:'sync',incarnation,owner:stringOwner}).matched,true);assert.equal(slot.transact({op:'remember',incarnation,key:'A',draft:rawDraft}).matched,true);return{slot,incarnation};};
 const responseSlot=response=>{let calls=0;return actualSendSlotFactory({mail_send_slot_abi_version:()=>1,MailSendSlotBridge:class{transact(){return JSON.stringify(++calls===1?valid:response);}}});};
 for(const [field,limit]of[['to',1024],['subject',4096],['body',8192],['goldText',256],['items',1024]]){
  const boundary=atBytes(limit),excess=boundary+'a';assert.equal(Buffer.byteLength(boundary,'utf8'),limit);assert.equal(Buffer.byteLength(excess,'utf8'),limit+1);
  const good={...rawDraft,[field]:field==='items'?[boundary]:boundary},bad={...rawDraft,[field]:field==='items'?[excess]:excess},f=readySlot();
  const accepted=f.slot.transact({op:'remember',incarnation:f.incarnation,key:'A',draft:good});assert.equal(accepted.matched,true,'actual compiled bridge accepts exact raw byte boundary');
  const snapshot=f.slot.transact({op:'status'}).state;assert.throws(()=>f.slot.transact({op:'remember',incarnation:f.incarnation,key:'A',draft:bad}),/request/);assert.deepEqual(f.slot.transact({op:'status'}).state,snapshot);
  assert.deepEqual(responseSlot(accepted).transact({op:'status'}).state.draft,good);
  assert.throws(()=>responseSlot({...accepted,state:{...accepted.state,draft:bad}}).transact({op:'status'}),/state/);
  const direct=new (recordedSendSlotModule().MailSendSlotBridge)(),before=JSON.parse(direct.transact(JSON.stringify({op:'status'}))).state;
  assert.equal(JSON.parse(direct.transact(JSON.stringify({op:'remember',incarnation:'1',key:'A',draft:bad}))).ok,false);assert.deepEqual(JSON.parse(direct.transact(JSON.stringify({op:'status'}))).state,before);
 }
 const f=readySlot(),keyBoundary=atBytes(4096),keyExcess=keyBoundary+'a',bodyBoundary=atBytes(32768),bodyExcess=bodyBoundary+'a';
 assert.equal(Buffer.byteLength(keyBoundary,'utf8'),4096);assert.equal(Buffer.byteLength(bodyBoundary,'utf8'),32768);
 const keyed=f.slot.transact({op:'remember',incarnation:f.incarnation,key:keyBoundary,draft:rawDraft});assert.equal(keyed.matched,true);
 assert.throws(()=>f.slot.transact({op:'remember',incarnation:f.incarnation,key:keyExcess,draft:rawDraft}),/request/);
 assert.throws(()=>responseSlot({...keyed,state:{...keyed.state,key:keyExcess}}).transact({op:'status'}),/state/);
 const slotPayload={name:'R',message:'body',gold:0,itemsIdx:['0','0','0','0','0'],stamped:false},wireBody=JSON.stringify({...slotPayload,itemsIdx:[0,0,0,0,0],type:'sendMail'});
 const reserved=f.slot.transact({op:'reserve',incarnation:f.incarnation,key:keyBoundary,payload:slotPayload,body:wireBody});assert.equal(reserved.matched,true,'actual compiled reserve accepts exact key byte boundary');
 const flight=reserved.state.flight;assert.throws(()=>f.slot.transact({op:'reserve',incarnation:f.incarnation,key:keyExcess,payload:slotPayload,body:wireBody}),/request/);
 for(const op of['allows','enter','cancel']){
  assert.equal(f.slot.transact({op,incarnation:f.incarnation,token:flight.token,stream:flight.stream,body:bodyBoundary}).matched,false,'in-bound mismatched body cannot claim an actual Core token');
  assert.throws(()=>f.slot.transact({op,incarnation:f.incarnation,token:flight.token,stream:flight.stream,body:bodyExcess}),/request/);
 }
 assert.equal(f.slot.transact({op:'reserve',incarnation:f.incarnation,key:keyBoundary,payload:slotPayload,body:bodyBoundary}).matched,false);
 assert.throws(()=>f.slot.transact({op:'reserve',incarnation:f.incarnation,key:keyBoundary,payload:slotPayload,body:bodyExcess}),/request/);
 for(const [field,boundary,excess]of[['key',keyBoundary,keyExcess],['body',bodyBoundary,bodyExcess]]){
  assert.equal(responseSlot({...reserved,state:{...reserved.state,flight:{...flight,[field]:boundary}}}).transact({op:'status'}).ok,true);
  assert.throws(()=>responseSlot({...reserved,state:{...reserved.state,flight:{...flight,[field]:excess}}}).transact({op:'status'}),/state/);
 }
 for(const [field,limit]of[['to',1024],['subject',4096],['body',8192],['goldText',256],['items',1024]]){
  const boundary=atBytes(limit),excess=boundary+'a',good={...rawDraft,[field]:field==='items'?[boundary]:boundary},bad={...rawDraft,[field]:field==='items'?[excess]:excess};
  assert.equal(responseSlot({...reserved,state:{...reserved.state,flight:{...flight,draft:good}}}).transact({op:'status'}).ok,true);
  assert.throws(()=>responseSlot({...reserved,state:{...reserved.state,flight:{...flight,draft:bad}}}).transact({op:'status'}),/state/);
 }
 const whole=readySlot(),request={op:'reserve',incarnation:whole.incarnation,key:'A',payload:{...slotPayload,message:''},body:wireBody};
 request.payload.message=atBytes(65536-Buffer.byteLength(JSON.stringify(request),'utf8'));assert.equal(Buffer.byteLength(JSON.stringify(request),'utf8'),65536);
 assert.equal(whole.slot.transact(request).matched,false,'exact serialized request limit reaches actual Rust refusal without creating a flight');
 request.payload.message+='a';assert.equal(Buffer.byteLength(JSON.stringify(request),'utf8'),65537);assert.throws(()=>whole.slot.transact(request),/request/);
 const direct=new (recordedSendSlotModule().MailSendSlotBridge)();assert.equal(JSON.parse(direct.transact(JSON.stringify(request))).ok,false,'actual Rust rejects one byte over the whole request cap');
 assert.throws(()=>whole.slot.transact({op:'remember',incarnation:whole.incarnation,key:'A',draft:{...rawDraft,to:'\ud800'}}),/request/);
 // Actual component ignores callback outcomes: only Page revision notification
 // makes a READY -> reserve-refused transition visible. Initial missing ABI
 // above is retained but is not used as proof of this later refusal boundary.
 const recorded=recordedSendSlotModule();let refuseReserve=false,reserveAttempts=0;
 const refusalModule={mail_send_slot_abi_version:()=>1,MailSendSlotBridge:class{constructor(){this.bridge=new recorded.MailSendSlotBridge();}transact(inputJson){if(refuseReserve&&JSON.parse(inputJson).op==='reserve'){reserveAttempts++;return this.bridge.transact(JSON.stringify({op:'status'}));}return this.bridge.transact(inputJson);}}};
 const ready=composePage({slotModule:refusalModule});assert.deepEqual(ready.dispatcher.composer.draft(ready.owner),ready.draft);assert.equal(ready.dispatcher.composer.pending(ready.owner),false);assert.equal(ready.dispatcher.composer.notice,null);
 const revisions=[],outcomes=[],queued=[];const send=pageFunctions(['sendMailMessage','dispatchMailCommand'],{...ready.deps,setMailComposeRevision:update=>revisions.push(update(0)),queueMicrotask:callback=>queued.push(callback)}).sendMailMessage;
 refuseReserve=true;const submit=windowComposeFunctions(['submitCompose'],{onSendMail:draft=>{const outcome=send(draft);outcomes.push(outcome);return outcome;},composeState:{pending:false,draft:ready.draft},composeTo:ready.draft.to,composeSubject:ready.draft.subject,composeBody:ready.draft.body,composeGold:ready.draft.goldText,composeItems:[],parcelState:null,setView(){throw Error('refused reserve cannot close the component');}}).submitCompose;
 assert.equal(submit(),undefined);assert.deepEqual(outcomes,['definitelyUnsent']);assert.deepEqual(revisions,[1]);assert.equal(reserveAttempts,1);assert.deepEqual(ready.send.writes,[]);assert.deepEqual(queued,[]);assert.deepEqual(ready.views,[]);
 assert.deepEqual(ready.dispatcher.composer.draft(ready.owner),ready.draft);assert.equal(ready.dispatcher.composer.pending(ready.owner),false);assert.match(ready.dispatcher.composer.notice,/authorization unavailable/i);assert.equal(reserveAttempts,1,'reads never retry a refused request');
});
test('mail actual Page OPEN is idempotent through entered flight and new socket rejects old source ACK without touching new binding',()=>{
 const f=composePage({socketThrow:true});assert.equal(f.sendMailMessage(submitInput(f)),'outcomeUnknown');const slot=f.dispatcher.mailTestSlot,before=slot.transact({op:'status'}).state,old=before.flight;
 const deps={socketRef:f.send.socketRef,WebSocket:{OPEN:1},mailDispatcherRef:{current:f.dispatcher}},open=pageFunctions(['observeMailSocketOpen'],deps).observeMailSocketOpen;
 assert.equal(open(f.send.socket),true);assert.deepEqual(slot.transact({op:'status'}).state,before);
 const next={readyState:0,send:body=>f.send.writes.push(body)};f.send.socketRef.current=next;assert.equal(open(f.send.socket),false);assert.equal(open(next),false);assert.deepEqual(slot.transact({op:'status'}).state,before);
 next.readyState=1;assert.equal(open(next),true);const fresh=slot.transact({op:'status'}).state;assert.equal(fresh.flight,null);assert.equal(fresh.generation,before.generation);assert.deepEqual(fresh.stream,{run:'1',connection:'2'});
 const proof=f.dispatcher.composer.reserve(f.owner,'compose-1',f.wirePayload);assert.ok(BigInt(proof.serial)>BigInt(old.token));const command=f.dispatcher.prepare('sendMail',null,null,proof);assert.ok(command);assert.equal(f.send.send({...f.wirePayload,type:'sendMail'},{mailProof:command}),true);f.dispatcher.composer.finish(proof,'confirmedSend');
 const occupied=slot.transact({op:'status'}).state;f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:1}},1,f.send.socket);assert.deepEqual(slot.transact({op:'status'}).state,occupied);assert.deepEqual(f.views,[]);
 f.captureMailGatewayEvent({type:'packet',packet:'MailSent',payload:{result:1}},1,next);assert.equal(f.dispatcher.composer.pending(f.owner),false);assert.deepEqual(f.views,['inbox']);
});
test('mail actual component callbacks advance Core full raw clock synchronously for ABA and every draft field but identical caret read does not',()=>{
 const f=composePage(),slot=f.dispatcher.mailTestSlot,generations=[];
 const save=windowComposeFunctions(['saveDraft'],{composeState:{pending:false},setLocalDraft:()=>{},onDraftChange:d=>{f.rememberMailDraft(d);generations.push(slot.transact({op:'status'}).state.generation);}}).saveDraft;
 let draft={...f.draft};const start=slot.transact({op:'status'}).state.generation;
 for(const [key,value]of[['body','B'],['body',f.draft.body],['to','Other'],['to',f.draft.to],['subject','B'],['subject',f.draft.subject],['goldText','0700'],['goldText',f.draft.goldText],['items',['display reference']],['items',[]],['attachmentUniqueIds',[41,42]],['attachmentUniqueIds',[42,41]],['attachmentUniqueIds',[]],['stamped',true],['stamped',false]]){draft={...draft,[key]:value};save(draft);}
 assert.equal(new Set([start,...generations]).size,generations.length+1);assert.deepEqual(f.dispatcher.composer.draft(f.owner),draft);
 const stable=slot.transact({op:'status'}).state.generation;save(draft);f.dispatcher.composer.draft(f.owner);f.dispatcher.composer.pending(f.owner);assert.equal(slot.transact({op:'status'}).state.generation,stable,'same raw callback/focus/caret reads carry no mutation');
});
test('mail actual presentation uses checked Core incarnation across replacement and remount retains entered tombstone in same cached facade',()=>{
 const f=composePage(),deps={...f.deps,mailPresentationSequenceRef:{current:Number.MAX_SAFE_INTEGER},mailReadAttemptRef:{current:null},mailContentKey:ui.mailContentKey,cancelMailParcel:()=>{}};
 const present=pageFunctions(['presentMailCompatibility'],deps).presentMailCompatibility;
 present('compose',null,f.draft.to,false,f.draft.subject);const a=deps.mailCompatRef.current.key;assert.match(a,/^mail-core-[1-9][0-9]*$/);assert.equal(deps.mailPresentationSequenceRef.current,Number.MAX_SAFE_INTEGER);
 present('compose',null,'Other',false,'B');present('compose',null,f.draft.to,false,f.draft.subject);assert.notEqual(deps.mailCompatRef.current.key,a);assert.equal(f.sendMailMessage(submitInput(f)),'definitelyUnsent','retained old callback cannot target the new Core presentation key');
 const g=composePage();assert.equal(g.sendMailMessage(submitInput(g)),'confirmedSend');const slot=g.dispatcher.mailTestSlot,old=slot.transact({op:'status'}).state.flight;
 const remounted=new ui.MailComposeSender(()=>slot);remounted.sync(g.owner);const state=slot.transact({op:'status'}).state;assert.notEqual(state.incarnation,old.incarnation);assert.equal(state.flight.token,old.token);assert.equal(state.flight.entered,true);assert.equal(remounted.pending(g.owner),true);
 assert.equal(remounted.acknowledge(g.owner,1,g.send.socket),'retired');assert.deepEqual(remounted.draft(g.owner),g.draft);assert.equal(remounted.pending(g.owner),false);
});
test('mail actual Page legal dirty draft ACK retires old Core token independently while preserving draft notice locks and view',()=>{
 const f=composePage();assert.equal(f.sendMailMessage(submitInput(f)),'confirmedSend');const slot=f.dispatcher.mailTestSlot,state=slot.transact({op:'status'}).state,old=state.flight;
 // Direct strict Core mutation is the dirty-draft gate fixture; ordinary
 // component fields remain disabled while pending. No JS slot policy copy.
 const dirty={...state.draft,body:'B'};assert.equal(slot.transact({op:'remember',incarnation:state.incarnation,key:state.key,draft:dirty}).matched,true);
 const notice=f.dispatcher.composer.notice,locks=[];f.deps.sendMailParcelLocks=x=>locks.push(...x);const capture=pageFunctions(['captureMailGatewayEvent'],f.deps).captureMailGatewayEvent;
 capture({type:'packet',packet:'MailSent',payload:{result:1}},1);const after=slot.transact({op:'status'}).state;assert.equal(after.flight,null);assert.deepEqual(after.draft,dirty);assert.equal(f.dispatcher.composer.notice,notice);assert.deepEqual(f.views,[]);assert.deepEqual(locks,[]);
 assert.notEqual(after.generation,old.generation);
});
test('mail actual parcel acceptance and snapshot mutation notify full raw UID order and stamp clock before queued work',()=>{
 const f=composePage(),slot=f.dispatcher.mailTestSlot,initial=slot.transact({op:'status'}).state.generation;let queued=false;
 const selected={...emptyParcelState,attachmentUniqueIds:[41,42],stamped:true,slotLimit:5};
 const parcel={state:selected,call:()=>({ok:true,state:selected,locks:[],quote:null,token:null}),sync:()=>({ok:true,state:selected,locks:[],quote:null,token:null})};
 const deps={...f.deps,mailParcelRef:{current:parcel},queueMicrotask:()=>{queued=true;assert.notEqual(slot.transact({op:'status'}).state.generation,initial);}};
 pageFunctions(['changeMailParcel'],deps).changeMailParcel('stamp');assert.equal(queued,true);assert.deepEqual(f.dispatcher.composer.draft(f.owner).attachmentUniqueIds,[41,42]);assert.equal(f.dispatcher.composer.draft(f.owner).stamped,true);
 const before=slot.transact({op:'status'}).state.generation;selected.attachmentUniqueIds=[42,41];
 pageFunctions(['syncMailParcel'],{...deps,mailParcelRawRef:{current:null},projectMailParcelSnapshot:()=>null}).syncMailParcel();assert.notEqual(slot.transact({op:'status'}).state.generation,before);assert.deepEqual(f.dispatcher.composer.draft(f.owner).attachmentUniqueIds,[42,41]);
 // An accepted authoritative snapshot remains a raw mutation while ordinary
 // pending editor callbacks are refused. Bump before any lock callback or ACK.
 const payload={...f.wirePayload,itemsIdx:[42,41,0,0,0],stamped:true},proof=f.dispatcher.composer.reserve(f.owner,'compose-1',payload);assert.ok(proof);assert.equal(f.dispatcher.composer.enterSocket(proof,proof.body,f.send.socket),true);
 const entered=slot.transact({op:'status'}).state;assert.equal(f.dispatcher.composer.remember(f.owner,'compose-1',f.draft),false);
 selected.attachmentUniqueIds=[41];selected.stamped=false;const locks=[];
 const sync=pageFunctions(['syncMailParcel'],{...deps,mailParcelRawRef:{current:null},projectMailParcelSnapshot:()=>null,sendMailParcelLocks:rows=>{assert.notEqual(slot.transact({op:'status'}).state.generation,entered.generation);locks.push(...rows);}}).syncMailParcel;
 assert.equal(sync(),true);const dirty=slot.transact({op:'status'}).state;assert.equal(dirty.flight.token,entered.flight.token);assert.equal(dirty.flight.entered,true);assert.notEqual(dirty.generation,entered.generation);assert.deepEqual(f.dispatcher.composer.draft(f.owner).attachmentUniqueIds,[41]);assert.equal(f.dispatcher.composer.draft(f.owner).stamped,false);
 const capture=pageFunctions(['captureMailGatewayEvent'],{...deps,sendMailParcelLocks:()=>{throw Error('dirty parcel ACK cannot unlock');}}).captureMailGatewayEvent;
 capture({type:'packet',packet:'MailSent',payload:{result:1}},1);assert.equal(slot.transact({op:'status'}).state.flight,null);assert.deepEqual(f.dispatcher.composer.draft(f.owner).attachmentUniqueIds,[41]);assert.deepEqual(f.views,[]);assert.deepEqual(locks,[]);
});
test('mail complete actual compiled SendSlot transcripts require current Rust replay with byte exact responses',()=>{
 assert.ok(sendSlotSessions.some(rows=>rows.some(r=>r.input.op==='enter'&&r.output.matched)));assert.ok(sendSlotSessions.some(rows=>rows.some(r=>r.input.op==='ack'&&r.output.matched)));
 for(const rows of sendSlotSessions)for(const row of rows){assert.deepEqual(JSON.parse(row.inputJson),row.input);assert.deepEqual(JSON.parse(row.outputJson),row.output);}
 if(fixtureDir){writeFileSync(resolve(fixtureDir,'mail-send-slot-port.json'),JSON.stringify(sendSlotSessions));writeFileSync(resolve(fixtureDir,'mail-send-slot-wasm-input.json'),JSON.stringify({module:sendSlotModulePath,wasm:sendSlotWasmPath,moduleSha256:createHash('sha256').update(readFileSync(sendSlotModulePath)).digest('hex'),wasmSha256:createHash('sha256').update(readFileSync(sendSlotWasmPath)).digest('hex'),abi:1}));}
});
test('common mail text ABI refuses partial, unknown, unsafe, malformed and unpaired-surrogate DTOs independently of old inbox',()=>{
 const methods={setMir2MailComposeUiSnapshot:()=>true,getMir2MailComposeUiStatus:()=>'',setMir2MailTextEdge:()=>true,
  setMir2MailComposeUiActionEdge:()=>true,setMir2MailComposeUiPointerEdge:()=>true,setMir2MailComposeUiIntentSink:()=>{},clearMir2MailComposeUiIntentSink:()=>{}};
 const cap={schemaVersion:1,mailComposeUiAbiVersion:1,textAdapterAbiVersion:1,compiled:true,startup:true};
 assert.equal(textInput.supportsComposeUi({...methods,getMir2MailComposeUiCapabilities:()=>JSON.stringify(cap)}),true);
 for(const broken of [{...cap,unknown:true},{...cap,textAdapterAbiVersion:2},{...cap,startup:false}])
  assert.equal(textInput.supportsComposeUi({...methods,getMir2MailComposeUiCapabilities:()=>JSON.stringify(broken)}),false);
 const raw={to:'甲',subject:'',body:'👨‍👩‍👧é\r\n',goldText:'0',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 assert.equal(textInput.validComposeRaw(raw),true);assert.equal(textInput.validComposeRaw({...raw,body:'\ud800'}),false);
 assert.equal(textInput.sameComposeRaw(raw,{body:raw.body,to:raw.to,items:[],goldText:raw.goldText,subject:raw.subject,
  stampedPresent:false,attachmentUniqueIdsPresent:false,stamped:false,attachmentUniqueIds:[]}),true,'DTO key order is not draft policy');
 assert.equal(textInput.validComposeRaw({...raw,attachmentUniqueIds:['01'],attachmentUniqueIdsPresent:true}),false);
 assert.equal(textInput.validComposeRaw({...raw,attachmentUniqueIds:['42','42'],attachmentUniqueIdsPresent:true}),false);
 assert.equal(textInput.validComposeRaw({...raw,unexpected:1}),false);
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const snapshot={owner,revision:1,incarnation:1,draftEpoch:1,draftGeneration:'1',presentationRevision:1,layoutRevision:1,open:true,inputEnabled:true,
  kind:'parcel',presentation:{logicalWidth:800,logicalHeight:400,stageCssScale:1,touch:false},raw,gold:200,
  parcel:{stamped:false,stampAvailable:true,quoteReady:false,postage:null,quoteError:null,cells:[]},notice:'N'.repeat(8192),recipientPrompt:null,feedback:null,
  goldPrompt:{draft:'50',maxAmount:200,amount:null}};
 assert.equal(textInput.validComposeSnapshot(snapshot),true,'Rust derives an unverified gold amount from the existing amount helper');
 assert.equal(textInput.validComposeSnapshot({...snapshot,recipientPrompt:'A'}),false,'two portable prompts cannot own focus together');
});
test('common mail UTF16↔UTF8 offset map is exact at astral boundaries and refuses surrogate halves',()=>{
 const body='A👨‍👩‍👧é\r\n终';for(let at=0;at<=body.length;at++){
  const bytes=textInput.utf16ToUtf8(body,at);if(bytes!==null)assert.equal(textInput.utf8ToUtf16(body,bytes),at);
 }
 assert.equal(textInput.utf16ToUtf8('A😀B',2),null);
 assert.equal(textInput.utf8ToUtf16('A😀B',2),null);
 assert.equal(textInput.utf16ToUtf8('\ud800',0),null);
});
test('rapid mail typing waits for each accepted Rust sequence and Core generation; stale external edit retires pending operations',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const base={to:'A',subject:'',body:'',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let raw=base,proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const edges=[],runtime={setMir2MailTextEdge:json=>{edges.push(JSON.parse(json));return true;}};
 const status=()=>({version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[],caret:null,scroll:[0,0],error:null,
  textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null});
 const queue=new textInput.MailTextEdgeQueue(runtime,()=>({status:status(),raw}));
 assert.equal(queue.enqueue({op:'insert',text:'A'}),true);assert.equal(queue.enqueue({op:'insert',text:'😀'}),true);
 assert.equal(edges.length,1);queue.tick();assert.equal(edges.length,1,'second input stays buffered while first edge is only admitted');
 raw={...base,body:'A'};queue.ownEdit(raw,'2');proof={...proof,sequence:2,draftGeneration:'2',editorRevision:2};queue.tick();
 assert.equal(edges.length,2);assert.equal(edges[1].operation.text,'😀');assert.equal(edges[1].proof.draftGeneration,'2');
 raw={...base,body:'external'};proof={...proof,sequence:3,draftGeneration:'3',editorRevision:3};queue.tick();
 assert.equal(queue.pendingCount,0,'external replacement cannot rebase a stale interactive edge');
});
test('common mail sink accepts the original admitted proof before status publication, then drains typing only at new Core generation',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const {run:ignoredRun,...localOwner}=owner;void ignoredRun;
 let raw={to:'A',subject:'',body:'',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let generation='1',proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const edges=[];let sink=()=>{};
 const status=()=>({version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[{left:0,top:0,width:800,height:400}],caret:null,
  scroll:[0,0],error:null,textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null});
 const runtime={getMir2MailComposeUiCapabilities:()=>JSON.stringify({schemaVersion:1,mailComposeUiAbiVersion:1,textAdapterAbiVersion:1,compiled:true,startup:true}),
  setMir2MailComposeUiSnapshot:()=>true,getMir2MailComposeUiStatus:()=>JSON.stringify(status()),setMir2MailTextEdge:json=>{edges.push(JSON.parse(json));return true;},
  setMir2MailComposeUiActionEdge:()=>true,setMir2MailComposeUiPointerEdge:()=>true,setMir2MailComposeUiIntentSink:callback=>{sink=callback;},clearMir2MailComposeUiIntentSink:()=>{}};
 const read=()=>({owner:localOwner,incarnation:1,draftEpoch:1,draftGeneration:generation,open:true,kind:'letter',
  presentation:{logicalWidth:800,logicalHeight:400,stageCssScale:1,touch:false},raw,gold:0,
  parcel:{stamped:false,stampAvailable:true,quoteReady:false,postage:null,quoteError:null,cells:[]},notice:null,recipientPrompt:null,feedback:null,goldPrompt:null,eligible:true});
 const host=new textInput.MailComposeHost(1,{runtime,isCurrent:()=>true,read,onIntent:(intent,current)=>{
  assert.equal(intent.proof.sequence,2);assert.equal(proof.sequence,1,'callback is before terminal status publication');
  assert.deepEqual(intent.baseRaw,raw);raw={...raw,body:intent.bodyMutation};generation='2';current.edges.ownEdit(raw,generation);current.tick();
 }});
 host.tick();assert.equal(host.ready,true);
 assert.equal(host.edges.enqueue({op:'insert',text:'A'}),true);assert.equal(host.edges.enqueue({op:'insert',text:'B'}),true);
 assert.equal(edges.length,1);const first=edges[0];
 sink(JSON.stringify({proof:first.proof,action:'edit',baseRaw:raw,bodyMutation:'A',recipientMutation:null,requestId:null,clipboardKind:null,selectedText:null,error:null,promptMutation:null,promptValue:null}));
 assert.equal(edges.length,1,'no second old-generation edge before authoritative re-ingest');
 proof={...proof,sequence:2,draftGeneration:'2',editorRevision:2};host.tick();
 assert.equal(edges.length,2);assert.equal(edges[1].proof.draftGeneration,'2');assert.equal(edges[1].operation.text,'B');
});
test('recipient prompt keeps the latest browser raw input while one Rust prompt replacement is in flight',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const raw={to:'A',subject:'',body:'',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:2,presentationRevision:1,layoutRevision:1,sequence:1};
 const status=()=>({version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[],caret:null,scroll:[0,0],error:null,
  textTarget:'recipient',promptText:'A',promptViewport:{left:10,top:20,width:100,height:20},bodyViewport:null});
 const edges=[],queue=new textInput.MailTextEdgeQueue({setMir2MailTextEdge:json=>{edges.push(JSON.parse(json));return true;}},()=>({status:status(),raw}));
 for(const text of ['Ab','Abc','Abcd'])assert.equal(queue.enqueue({op:'promptReplace',target:'recipient',text}),true);
 assert.equal(edges.length,1);assert.equal(queue.pendingCount,2,'later raw replacements coalesce without owning text policy');
 proof={...proof,sequence:2,editorRevision:2};queue.tick();assert.equal(edges.length,2);assert.equal(edges[1].operation.text,'Abcd');
});
test('mail clipboard gesture uses current published selection; cut waits for matching Rust selectedText and stale request cannot delete',async()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const raw={to:'A',subject:'',body:'A😀B',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const view=()=>({status:{version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[],caret:{left:0,top:0,width:1,height:16,anchorUtf16:1,caretUtf16:3,editorRevision:1,layoutRevision:1},scroll:[0,0],error:null,
  textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null},raw});
 const edges=[],queue=new textInput.MailTextEdgeQueue({setMir2MailTextEdge:json=>{edges.push(JSON.parse(json));return true;}},view);
 const clipboard=new textInput.MailTextClipboard(queue,view,{readText:async()=>{throw Error('paste denied');}}),written=[];
 const gesture={setData:(format,text)=>{assert.equal(format,'text/plain');written.push(text);},getData:()=>'',preventDefault:()=>{}};
 assert.equal(clipboard.gesture('cut',gesture),true);assert.deepEqual(written,['😀']);
 const request=edges[0];proof={...proof,sequence:2};queue.tick();
 const intent={proof:request.proof,action:'clipboardRequest',baseRaw:raw,bodyMutation:null,recipientMutation:null,requestId:1,clipboardKind:'cut',selectedText:'wrong',error:null,promptMutation:null,promptValue:null};
 await clipboard.onRequest(intent);assert.equal(edges.length,2);assert.equal(edges[1].operation.success,false,'mismatched Rust text cannot authorize cut');
});
test('mail asynchronous paste retains its original gesture proof across delayed permission and refuses reordered edits',async()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const raw={to:'A',subject:'',body:'A',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const view=()=>({status:{version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[],caret:{left:0,top:0,width:1,height:16,anchorUtf16:1,caretUtf16:1,editorRevision:proof.editorRevision,layoutRevision:1},scroll:[0,0],error:null,
  textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null},raw});
 const edges=[],queue=new textInput.MailTextEdgeQueue({setMir2MailTextEdge:json=>{edges.push(JSON.parse(json));return true;}},view);
 let resolveRead;const clipboard=new textInput.MailTextClipboard(queue,view,{readText:()=>new Promise(resolve=>{resolveRead=resolve;})});
 assert.equal(clipboard.gesture('paste',{setData:()=>{},getData:()=>{throw Error('no synchronous text');},preventDefault:()=>{}}),true);
 const request={proof:edges[0].proof,action:'clipboardRequest',baseRaw:raw,bodyMutation:null,recipientMutation:null,requestId:1,clipboardKind:'paste',selectedText:null,error:null,promptMutation:null,promptValue:null};
 const waiting=clipboard.onRequest(request);proof={...proof,sequence:3,editorRevision:2};resolveRead('late');await waiting;
 assert.equal(edges.length,1,'a delayed paste cannot recapture a new editor revision or enqueue text');
});
test('common mail pointer uses actual canvas transform and retires held proof after an editor revision',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const raw={to:'A',subject:'',body:'A',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 const presentation={logicalWidth:800,logicalHeight:400,stageCssScale:1,touch:false};
 const {run:ignoredRun,...localOwner}=owner;void ignoredRun;
 assert.deepEqual(textInput.composeStagePoint({left:100,top:40,width:1600,height:800},presentation,900,440),{x:400,y:200});
 let proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const edges=[],status=()=>({version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[{left:0,top:0,width:800,height:400}],caret:null,
  scroll:[0,0],error:null,textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null});
 const runtime={getMir2MailComposeUiCapabilities:()=>JSON.stringify({schemaVersion:1,mailComposeUiAbiVersion:1,textAdapterAbiVersion:1,compiled:true,startup:true}),
  setMir2MailComposeUiSnapshot:()=>true,getMir2MailComposeUiStatus:()=>JSON.stringify(status()),setMir2MailTextEdge:()=>true,
  setMir2MailComposeUiActionEdge:()=>true,setMir2MailComposeUiPointerEdge:json=>{edges.push(JSON.parse(json));return true;},
  setMir2MailComposeUiIntentSink:()=>{},clearMir2MailComposeUiIntentSink:()=>{}};
 const host=new textInput.MailComposeHost(1,{runtime,isCurrent:()=>true,read:()=>({owner:localOwner,incarnation:1,draftEpoch:1,draftGeneration:'1',
  open:true,kind:'letter',presentation,raw,gold:0,parcel:{stamped:false,stampAvailable:true,quoteReady:false,postage:null,quoteError:null,cells:[]},
  notice:null,recipientPrompt:null,feedback:null,goldPrompt:null,eligible:true}),onIntent:()=>{}});
 host.tick();assert.equal(host.ready,true);
 // Host owner input omits run; a real adapter constructs that from the live run.
 const read=host.textContext();assert.equal(read?.status.proof?.editorRevision,1);
 assert.equal(host.pointer(7,'down',400,200,false),true);
 proof={...proof,editorRevision:2,sequence:2};
 assert.equal(host.pointer(7,'up',400,200,false),false);
 assert.equal(edges.length,1,'stale held click cannot dispatch a common action');
});
test('common mail Page and Shell retain single Core sender and stop scene keys while a shared editor is modal',()=>{
 const page=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8');
 const shell=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8');
 assert.match(page,/sender\?\.rememberRaw\(owner,p\.key,next\)/);
 assert.match(page,/host\.edges\.ownEdit\(accepted\.raw,accepted\.generation\)/);
 assert.match(page,/sendMailMessage\(draft,true\)/);
 assert.match(page,/host\.edges\.pendingCount>0.*click Send again/);
 assert.match(page,/bevyMailUi\.composeRequested&&bevyMailUi\.composeWithdrawn/);
 assert.match(shell,/data-mail-text-adapter="shared"/);
 assert.match(shell,/if\(bevyMailComposeReady\|\|bevyMailComposePending\)/);
 assert.match(shell,/onCompositionEnd=/);
 assert.match(shell,/onBevyMailClipboard\?\.\('cut'/);
 assert.match(shell,/getBevyMailComposeBusy\?\.\(\)/);
});

// Evaluate actual Shell handler AST with fake event and clipboard ports only.
function actualShellHandler(name,deps,jsx=false){
 const source=ts.createSourceFile('original-client-shell.tsx',readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 let expression=null;function visit(node){
  if(!jsx&&ts.isFunctionDeclaration(node)&&node.name?.text===name)expression=node.getText(source);
  if(jsx&&ts.isJsxSelfClosingElement(node)&&node.tagName.getText(source)==='textarea'){
   const marker=node.attributes.properties.find(a=>ts.isJsxAttribute(a)&&a.name.text==='data-mail-text-adapter');
   if(marker?.initializer?.getText(source)==='"shared"'){
    const attribute=node.attributes.properties.find(a=>ts.isJsxAttribute(a)&&a.name.text===name);
    if(attribute?.initializer&&ts.isJsxExpression(attribute.initializer)&&attribute.initializer.expression)expression=attribute.initializer.expression.getText(source);
   }
  }ts.forEachChild(node,visit);
 }visit(source);assert.ok(expression,`actual Shell ${name} handler must be present`);
 const code=ts.transpileModule(jsx?`const callback=${expression};`:`${expression}`,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 return new Function(...Object.keys(deps),`${code};return ${jsx?'callback':name};`)(...Object.values(deps));
}
function actualShellComposeLayout(deps){
 const source=ts.createSourceFile('original-client-shell.tsx',readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 let callback=null;function visit(node){if(ts.isCallExpression(node)&&node.expression.getText(source)==='useLayoutEffect'){
  const candidate=node.arguments[0];if(candidate&&candidate.getText(source).includes('setMailTextPosition')&&candidate.getText(source).includes('setSelectionRange'))callback=candidate.getText(source);
 }ts.forEachChild(node,visit);}visit(source);assert.ok(callback,'actual Shell compose layout effect must be present');
 const code=ts.transpileModule(`const callback=${callback};`,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 return new Function(...Object.keys(deps),`${code};return callback;`)(...Object.values(deps));
}
function actualShellNativeListenerEffect(deps){
 const source=ts.createSourceFile('original-client-shell.tsx',readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 let callback=null;function visit(node){if(ts.isCallExpression(node)&&node.expression.getText(source)==='useLayoutEffect'){
  const candidate=node.arguments[0];if(candidate&&candidate.getText(source).includes("addEventListener('beforeinput',listener)"))callback=candidate.getText(source);
 }ts.forEachChild(node,visit);}visit(source);assert.ok(callback,'actual native beforeinput listener effect must be present');
 const code=ts.transpileModule(`const callback=${callback};`,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
 return new Function(...Object.keys(deps),`${code};return callback;`)(...Object.values(deps));
}
test('actual Shell modal guards preserve focused mail textarea native character and clipboard defaults while blocking world',()=>{
 const textarea={};let activeElement=textarea,ready=true,pending=false,stops=0,prevented=0;
 const held={current:new Set(['up'])},run={current:true},deps={
  get bevyMailComposeReady(){return ready;},get bevyMailComposePending(){return pending;},
  mailTextRef:{current:textarea},document:{get activeElement(){return activeElement;}},
  heldKeyboardMoveKeysRef:held,heldKeyboardRunModeRef:run,onViewportDirectionStop:()=>{stops++;}
 };
 const makeHandlers=()=>{const state={...deps,bevyMailComposeReady:ready,bevyMailComposePending:pending};
  const current=actualShellHandler('isCurrentMailTextTarget',state);
  return ['handleShortcutKey','handleKeyboardMoveDown','handleKeyboardMoveUp'].map(name=>actualShellHandler(name,{...state,isCurrentMailTextTarget:current}));};
 let handlers=makeHandlers();
 const event=(target,key)=>({target,key,preventDefault(){prevented++;}});
 for(const key of ['a','7','ArrowLeft','Backspace','c','v','x'])for(const handler of handlers)handler(event(textarea,key));
 assert.equal(prevented,0,'real focused textarea keeps native text and Ctrl+C/V/X gesture defaults');
 assert.equal(held.current.size,0);assert.equal(run.current,false);assert.ok(stops>0,'modal still stops held world movement');
 activeElement=null;for(const handler of handlers)handler(event(textarea,'a'));assert.equal(prevented,3,'unfocused textarea has no editor authority');
 activeElement=textarea;pending=true;handlers=makeHandlers();for(const handler of handlers)handler(event(textarea,'v'));assert.equal(prevented,6,'pending composer blocks even its former textarea');
 pending=false;handlers=makeHandlers();for(const handler of handlers)handler(event({},'a'));assert.equal(prevented,9,'other world target stays blocked');
});
test('actual native beforeinput handler admits body characters once; prompt input and clipboard JSX handlers keep browser gesture',()=>{
 const textarea={},calls=[],notices=[];let target='body',prevented=0;
 const proof={owner:{run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7},incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const context={status:{textTarget:target,proof}},active={current:null},echo={current:null},dom={current:null};
 const deps={mailTextRef:{current:textarea},mailTextInputCallbacksRef:{current:{context,onEdge:edge=>{calls.push(edge);return true;}}},
  mailCompositionActiveRef:active,mailDomCompositionRef:dom,mailCompositionEchoRef:echo,sameComposeAuthority:textInput.sameComposeAuthority,
  mailKeyBeforeInputRef:{current:null},setMailTextInputNotice:v=>notices.push(v)};
 const before=actualShellHandler('handleMailNativeBeforeInput',deps);
 const input=(inputType,data,isComposing=false)=>({target:textarea,inputType,data,isComposing,preventDefault(){prevented++;}});
 before(input('insertText','A'));assert.deepEqual(calls,[{op:'insert',text:'A'}]);assert.equal(prevented,1);
 before(input('insertCompositionText','あ',true));assert.equal(calls.length,1);assert.equal(prevented,1,'IME preview remains native and uncommitted');
 active.current=3;dom.current={target:'body',proof,element:textarea};const end=actualShellHandler('onCompositionEnd',{mailCompositionActiveRef:active,
  mailDomCompositionRef:dom,currentMailDomComposition:()=>true,bevyMailTextContext:context,
  onBevyMailTextEdge:edge=>{calls.push(edge);return true;},mailCompositionEchoRef:echo,setMailTextInputNotice:v=>notices.push(v)},true);
 end({currentTarget:textarea,data:'あ'});assert.deepEqual(calls.at(-1),{op:'imeCommit',compositionId:3,text:'あ'});assert.equal(echo.current.compositionId,3);
 before(input('insertFromComposition','あ'));assert.equal(calls.length,2);assert.equal(prevented,2,'typed composition echo cannot double commit');
 before(input('insertText','あ'));assert.deepEqual(calls.at(-1),{op:'insert',text:'あ'},'same character from a later ordinary input cannot be swallowed');
 deps.mailTextInputCallbacksRef.current.context.status.textTarget='recipient';before(input('insertText','R'));assert.equal(prevented,3,'prompt native input is not cancelled');
 for(const prompt of ['recipient','gold']){
  const promptCalls=[],proof={sequence:1},context={status:{textTarget:prompt,proof}},buffered={current:null},promptText={value:prompt==='recipient'?'Reader':'123'};
  const onInput=actualShellHandler('onInput',{bevyMailTextContext:context,bufferedMailPromptRef:buffered,
    mailTextRef:{current:promptText},mailDomCompositionRef:{current:null},sameComposeScope:textInput.sameComposeScope,
    onBevyMailTextEdge:edge=>{promptCalls.push(edge);return true;}},true);
  onInput({currentTarget:promptText,nativeEvent:{isComposing:false}});
  assert.deepEqual(promptCalls,[{op:'promptReplace',target:prompt,text:prompt==='recipient'?'Reader':'123'}]);assert.deepEqual(buffered.current,{target:prompt,text:prompt==='recipient'?'Reader':'123',proof,element:promptText});
 }
 for(const kind of ['onCopy','onCut','onPaste']){
  const seen=[],handler=actualShellHandler(kind,{bevyMailTextContext:{status:{textTarget:'body'}},onBevyMailClipboard:(action,port)=>{seen.push(action);port.getData?.('text/plain');return true;}},true);
  handler({clipboardData:{types:['text/plain'],setData(){},getData:()=> 'selected'},preventDefault(){prevented++;}});
  assert.deepEqual(seen,[kind.slice(2).toLowerCase()]);
 }
 const shell=readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8');
 assert.match(shell,/addEventListener\('beforeinput',listener\)/);assert.doesNotMatch(shell,/onBeforeInput=/);
 assert.equal(notices.at(-1),null);
});
test('IME preview DTO uses nullable UTF16 ranges with both endpoints on scalar boundaries',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const raw={to:'A',subject:'',body:'',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 const proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const status={version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[],caret:null,scroll:[0,0],error:null,textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null};
 const edges=[],queue=new textInput.MailTextEdgeQueue({setMir2MailTextEdge:json=>{edges.push(JSON.parse(json));return true;}},()=>({status,raw}));
 assert.equal(queue.enqueue({op:'imePreview',compositionId:1,text:'A😀',cursorUtf16:[2,2]}),false);
 assert.equal(queue.enqueue({op:'imePreview',compositionId:1,text:'A😀',cursorUtf16:[1,3]}),true);
 assert.deepEqual(edges[0].operation.cursorUtf16,[1,3]);
});
test('refused text edge retires on external Core replacement and fresh typing cannot inherit it',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 let raw={to:'A',subject:'',body:'',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const status=()=>({version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[],caret:null,scroll:[0,0],error:null,textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null});
 const edges=[],queue=new textInput.MailTextEdgeQueue({setMir2MailTextEdge:json=>{const edge=JSON.parse(json);edges.push(edge);return edge.operation.text==='B';}},()=>({status:status(),raw}));
 assert.equal(queue.enqueue({op:'insert',text:'A'}),true);assert.equal(edges.length,1);
 queue.tick();assert.equal(edges.length,2,'refused edge may retry only against unchanged proof and raw');assert.equal(edges[1].operation.text,'A');
 raw={...raw,body:'external'};proof={...proof,draftGeneration:'2',editorRevision:2,sequence:2};
 assert.equal(queue.enqueue({op:'insert',text:'B'}),true);
 assert.equal(edges.length,3);assert.equal(edges[2].operation.text,'B');assert.equal(edges[2].proof.draftGeneration,'2');
 assert.equal(queue.pendingCount,1,'only the new accepted edge remains in flight');
});
test('admitted nonmutating edge does not authorize a later external Core edit',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 let raw={to:'A',subject:'',body:'',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const status=()=>({version:1,owner,ready:true,inputEnabled:true,modal:true,proof,inputRegions:[],caret:null,scroll:[0,0],error:null,textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null});
 const edges=[],queue=new textInput.MailTextEdgeQueue({setMir2MailTextEdge:json=>{edges.push(JSON.parse(json));return true;}},()=>({status:status(),raw}));
 queue.enqueue({op:'selection',anchorUtf16:0,caretUtf16:0});queue.enqueue({op:'insert',text:'A'});assert.equal(edges.length,1);
 raw={...raw,body:'external'};proof={...proof,sequence:2,draftGeneration:'2',editorRevision:2};queue.tick();
 assert.equal(queue.pendingCount,0);assert.equal(edges.length,1);
});
test('old common composer stop and withdraw leave replacement sink and snapshot alive',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const {run:ignored,...local}=owner;void ignored;const raw={to:'A',subject:'',body:'',goldText:'',items:[],attachmentUniqueIds:[],stamped:false,attachmentUniqueIdsPresent:false,stampedPresent:false};
 let snapshot=null,sink=null,clears=0,seen=0;const proof={owner:{...owner,run:2},incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const runtime={getMir2MailComposeUiCapabilities:()=>JSON.stringify({schemaVersion:1,mailComposeUiAbiVersion:1,textAdapterAbiVersion:1,compiled:true,startup:true}),
  setMir2MailComposeUiSnapshot:json=>{snapshot=JSON.parse(json);return true;},getMir2MailComposeUiStatus:()=>JSON.stringify({version:1,owner:snapshot?.owner??owner,ready:true,inputEnabled:true,modal:true,proof:{...proof,owner:snapshot?.owner??owner},inputRegions:[],caret:null,scroll:[0,0],error:null,textTarget:'body',promptText:null,promptViewport:null,bodyViewport:null}),
  setMir2MailTextEdge:()=>true,setMir2MailComposeUiActionEdge:()=>true,setMir2MailComposeUiPointerEdge:()=>true,
  setMir2MailComposeUiIntentSink:callback=>{sink=callback;},clearMir2MailComposeUiIntentSink:()=>{clears++;sink=null;}};
 const read=()=>({owner:local,incarnation:1,draftEpoch:1,draftGeneration:'1',open:true,kind:'letter',presentation:{logicalWidth:800,logicalHeight:400,stageCssScale:1,touch:false},raw,gold:0,
  parcel:{stamped:false,stampAvailable:true,quoteReady:false,postage:null,quoteError:null,cells:[]},notice:null,recipientPrompt:null,feedback:null,goldPrompt:null,eligible:true});
 const old=new textInput.MailComposeHost(1,{runtime,isCurrent:()=>true,read,onIntent:()=>{throw Error('retired sink');}});old.tick();const prior=sink;
 const replacement=new textInput.MailComposeHost(2,{runtime,isCurrent:()=>true,read,onIntent:()=>{seen++;}});replacement.tick();assert.equal(replacement.ready,true);
 const duplicate=new textInput.MailComposeHost(2,{runtime,isCurrent:()=>true,read,onIntent:()=>{throw Error('duplicate run');}});
 duplicate.tick();duplicate.stop();assert.equal(replacement.ready,true,'same run must not replace live host identity');
 const rollback=new textInput.MailComposeHost(1,{runtime,isCurrent:()=>true,read,onIntent:()=>{throw Error('rollback run');}});
 rollback.tick();rollback.stop();assert.equal(replacement.ready,true,'lower run must not replace live host identity');
 const liveSink=sink,liveSnapshot=snapshot;old.stop();old.withdraw();old.tick();assert.equal(sink,liveSink);assert.deepEqual(snapshot,liveSnapshot);assert.equal(clears,0);
 const edge=replacement.action('cancel');assert.equal(edge,true);const actionProof={...proof,sequence:2};
 prior(JSON.stringify({proof:actionProof,action:'cancel',baseRaw:raw,bodyMutation:null,recipientMutation:null,requestId:null,clipboardKind:null,selectedText:null,error:null,promptMutation:null,promptValue:null}));
 assert.equal(seen,0,'captured old sink cannot act after replacement');
 liveSink(JSON.stringify({proof:actionProof,action:'cancel',baseRaw:raw,bodyMutation:null,recipientMutation:null,requestId:null,clipboardKind:null,selectedText:null,error:null,promptMutation:null,promptValue:null}));
 assert.equal(seen,1);assert.equal(replacement.ready,true);
});
test('actual shared textarea composition blocks body and prompt keys and preedit selection before canonical transport',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 const body={value:'preedit',selectionStart:1,selectionEnd:1,selectionDirection:'forward'},textRef={current:body},dom={current:null},id={current:0},bodyActive={current:null},echo={current:null},buffered={current:null},pending={current:null};
 const edges=[],actions=[],context={status:{textTarget:'body',proof,caret:{anchorUtf16:0,caretUtf16:0},promptText:null},pendingEdges:0};
 const onEdge=edge=>{edges.push(edge);return true;},notice=()=>{};
 const retire=()=>{dom.current=null;bodyActive.current=null;echo.current=null;};
 const deps={mailTextRef:textRef,mailDomCompositionRef:dom,mailCompositionIdRef:id,mailCompositionActiveRef:bodyActive,
  mailCompositionEchoRef:echo,retireMailDomComposition:retire,sameComposeScope:textInput.sameComposeScope,
  pendingMailPromptConfirmRef:pending,mailKeyBeforeInputRef:{current:null},
  bevyMailTextContext:context,onBevyMailTextEdge:onEdge,setMailTextInputNotice:notice};
 const start=actualShellHandler('onCompositionStart',deps,true);start({currentTarget:body});
 assert.equal(dom.current.target,'body');assert.deepEqual(edges,[{op:'imeStart',compositionId:1}]);
 const key=actualShellHandler('onKeyDown',{...deps,onBevyMailComposeAction:action=>actions.push(action),pendingMailPromptConfirmRef:pending,mailKeyBeforeInputRef:{current:null}},true);
 const select=actualShellHandler('onSelect',deps,true);let prevented=0;
 for(const name of ['Enter','Backspace','Delete','ArrowLeft','ArrowRight'])key({currentTarget:body,key:name,nativeEvent:{isComposing:false,keyCode:0},preventDefault(){prevented++;}});
 select({currentTarget:body,nativeEvent:{isComposing:false}});assert.equal(edges.length,1,'preedit key and DOM selection cannot mutate accepted body');assert.equal(prevented,0);
 const prompt={value:'Reader',selectionStart:3,selectionEnd:3,selectionDirection:'forward'};textRef.current=prompt;context.status.textTarget='recipient';
 start({currentTarget:prompt});assert.equal(dom.current.target,'recipient');assert.equal(bodyActive.current,null,'prompt DOM lifecycle is independent from Rust body composition id');
 const promptInput=actualShellHandler('onInput',{...deps,bufferedMailPromptRef:buffered},true);
 promptInput({currentTarget:prompt,nativeEvent:{isComposing:true}});
 for(const name of ['Enter','Escape','Backspace'])key({currentTarget:prompt,key:name,nativeEvent:{isComposing:false,keyCode:0},preventDefault(){prevented++;}});
 select({currentTarget:prompt,nativeEvent:{isComposing:true}});
 assert.equal(edges.length,1);assert.deepEqual(actions,[]);assert.equal(prevented,0,'IME Enter cannot confirm or cancel prompt');
 const end=actualShellHandler('onCompositionEnd',{...deps,bufferedMailPromptRef:buffered,currentMailDomComposition:()=>true},true);
 end({currentTarget:prompt,data:'Reader'});assert.deepEqual(edges.at(-1),{op:'promptReplace',target:'recipient',text:'Reader'});
 assert.equal(dom.current,null);assert.equal(buffered.current.text,'Reader');
 promptInput({currentTarget:prompt,nativeEvent:{isComposing:false}});assert.equal(edges.length,2,'trailing same raw input is not a second prompt replacement');
 key({currentTarget:prompt,key:'Enter',nativeEvent:{isComposing:true,keyCode:0},preventDefault(){prevented++;}});
 key({currentTarget:prompt,key:'Enter',nativeEvent:{isComposing:false,keyCode:229},preventDefault(){prevented++;}});
 assert.deepEqual(actions,[],'native composing and process key stay with the IME even after DOM end');
 const gold={value:'123',selectionStart:3,selectionEnd:3,selectionDirection:'forward'};textRef.current=gold;context.status.textTarget='gold';
 start({currentTarget:gold});promptInput({currentTarget:gold,nativeEvent:{isComposing:true}});
 key({currentTarget:gold,key:'Enter',nativeEvent:{isComposing:false,keyCode:0},preventDefault(){prevented++;}});
 assert.equal(edges.length,2);assert.deepEqual(actions,[]);
 end({currentTarget:gold,data:'123'});assert.deepEqual(edges.at(-1),{op:'promptReplace',target:'gold',text:'123'});
});
test('actual IME explicit completion before or after compositionend commits exactly once and leaves later same text valid',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 for(const order of ['beforeEnd','afterEnd']){
  const textarea={value:'あ'},textRef={current:textarea},dom={current:null},id={current:0},active={current:null},echo={current:null},edges=[],notices=[];
  const context={status:{textTarget:'body',proof}},onEdge=edge=>{edges.push(edge);return true;};
  const start=actualShellHandler('onCompositionStart',{mailTextRef:textRef,mailDomCompositionRef:dom,mailCompositionIdRef:id,
   mailCompositionActiveRef:active,mailCompositionEchoRef:echo,retireMailDomComposition:()=>{},sameComposeScope:textInput.sameComposeScope,
   pendingMailPromptConfirmRef:{current:null},mailKeyBeforeInputRef:{current:null},
   bevyMailTextContext:context,onBevyMailTextEdge:onEdge,setMailTextInputNotice:v=>notices.push(v)},true);
  const end=actualShellHandler('onCompositionEnd',{mailDomCompositionRef:dom,mailCompositionActiveRef:active,
   mailCompositionEchoRef:echo,currentMailDomComposition:()=>true,bevyMailTextContext:context,
   onBevyMailTextEdge:onEdge,setMailTextInputNotice:v=>notices.push(v)},true);
  const before=actualShellHandler('handleMailNativeBeforeInput',{mailTextRef:textRef,mailTextInputCallbacksRef:{current:{context,onEdge}},
   mailDomCompositionRef:dom,mailCompositionEchoRef:echo,sameComposeAuthority:textInput.sameComposeAuthority,
   mailKeyBeforeInputRef:{current:null},setMailTextInputNotice:v=>notices.push(v)});
  const event=(type,data)=>({target:textarea,inputType:type,data,isComposing:false,preventDefault(){}});
  start({currentTarget:textarea});if(order==='beforeEnd')before(event('insertFromComposition','あ'));
  end({currentTarget:textarea,data:'あ'});if(order==='afterEnd')before(event('insertFromComposition','あ'));
  end({currentTarget:textarea,data:'あ'});
  assert.equal(edges.filter(edge=>edge.op==='imeCommit').length,1,`${order} must commit once`);
  before(event('insertText','あ'));assert.deepEqual(edges.at(-1),{op:'insert',text:'あ'},'ordinary same text without keydown remains valid');
 }
});
test('actual compose layout preserves active preedit DOM and stale old element end cannot erase a new target',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 let valueWrites=0,selectionWrites=0,focusWrites=0,positions=0;const oldText={_value:'preedit',get value(){return this._value;},set value(v){this._value=v;valueWrites++;},
  selectionStart:1,selectionEnd:1,selectionDirection:'forward',setSelectionRange(){selectionWrites++;},focus(){focusWrites++;}};
 const textRef={current:oldText},dom={current:{target:'body',proof,element:oldText}},active={current:3},echo={current:null},buffered={current:null},pending={current:null};
 const frame={getBoundingClientRect:()=>({left:0,top:0})},canvas={getBoundingClientRect:()=>({left:0,top:0})};
 const context={status:{textTarget:'body',proof,modal:true,inputEnabled:true,caret:{left:10,top:20,width:2,height:18,anchorUtf16:0,caretUtf16:0},bodyViewport:null,promptViewport:null,promptText:null},
  raw:{body:'canonical'},presentation:{stageCssScale:1},pendingEdges:0};
 const current=actualShellHandler('currentMailDomComposition',{mailDomCompositionRef:dom,bevyMailComposeReady:true,bevyMailComposePending:false,
  mailTextRef:textRef,sameComposeScope:textInput.sameComposeScope});
 const retire=actualShellHandler('retireMailDomComposition',{mailDomCompositionRef:dom,mailCompositionActiveRef:active,mailCompositionEchoRef:echo,
  pendingMailPromptConfirmRef:pending,bufferedMailPromptRef:buffered,mailKeyBeforeInputRef:{current:null},sameComposeScope:textInput.sameComposeScope});
 let retireSwaps=0;const base={bevyMailComposeReady:true,bevyMailTextContext:context,stageFrameRef:{current:frame},document:{getElementById:()=>canvas,activeElement:oldText},
  sharedUiCanvasId:()=> 'shared',webGl2SharedCanvasPrototype:false,mailDomCompositionRef:dom,currentMailDomComposition:current,retireMailDomComposition:retire,
  bufferedMailPromptRef:buffered,pendingMailPromptConfirmRef:pending,setMailDomRetireKey:()=>{retireSwaps++;},setMailTextPosition:()=>{positions++;},mailTextRef:textRef,
  sameComposeScope:textInput.sameComposeScope,onBevyMailComposeAction:()=>{throw Error('preedit confirm');}};
 actualShellComposeLayout(base)();assert.equal(positions,1);assert.equal(valueWrites,0);assert.equal(selectionWrites,0);assert.equal(focusWrites,0);
 const oldEnd=actualShellHandler('onCompositionEnd',{mailDomCompositionRef:dom,mailCompositionActiveRef:active,mailCompositionEchoRef:echo,currentMailDomComposition:current,
  sameComposeScope:textInput.sameComposeScope,bevyMailTextContext:context,onBevyMailTextEdge:()=>{throw Error('old commit');},setMailTextInputNotice:()=>{}},true);
 const nextProof={...proof,draftGeneration:'2'},newText={value:'',focus(){},setSelectionRange(){}},nextContext={...context,status:{...context.status,
  proof:nextProof,textTarget:'recipient',caret:null,promptViewport:{left:10,top:20,width:100,height:20},promptText:'New'}};
 assert.equal(current({...context,status:{...context.status,proof:nextProof}}),false,'opaque Core generation replacement retires preedit without using sequence as a content clock');
 assert.equal(current({...context,status:{...context.status,proof:{...proof,owner:{...owner,run:2}}}}),false,'new owner retires old DOM composition');
 assert.equal(current({...context,status:{...context.status,proof:{...proof,focusGeneration:2}}}),false,'new focus generation retires old DOM composition');
 assert.equal(current({...context,status:{...context.status,modal:false}}),false,'closed modal retires old DOM composition');
 textRef.current=newText;actualShellComposeLayout({...base,bevyMailTextContext:nextContext})();
 assert.equal(retireSwaps,1,'invalidated active preedit requests a fresh textarea element');
 actualShellComposeLayout({...base,bevyMailTextContext:nextContext})();
 assert.equal(dom.current,null,'new draft generation/target retires old preedit');assert.equal(newText.value,'New');
 const edges=[],newStart=actualShellHandler('onCompositionStart',{mailTextRef:textRef,mailDomCompositionRef:dom,mailCompositionIdRef:{current:3},
  mailCompositionActiveRef:active,mailCompositionEchoRef:echo,retireMailDomComposition:retire,sameComposeScope:textInput.sameComposeScope,
  pendingMailPromptConfirmRef:pending,mailKeyBeforeInputRef:{current:null},
  bevyMailTextContext:nextContext,onBevyMailTextEdge:edge=>{edges.push(edge);return true;},setMailTextInputNotice:()=>{}},true);
 newStart({currentTarget:newText});assert.equal(dom.current.element,newText);
 oldEnd({currentTarget:oldText,data:'stale'});assert.equal(dom.current.element,newText,'old element completion cannot clear new active lifecycle');assert.equal(edges.length,0);
 const newEnd=actualShellHandler('onCompositionEnd',{mailDomCompositionRef:dom,mailCompositionActiveRef:active,mailCompositionEchoRef:echo,currentMailDomComposition:current,
  sameComposeScope:textInput.sameComposeScope,bevyMailTextContext:nextContext,onBevyMailTextEdge:edge=>{edges.push(edge);return true;},
  bufferedMailPromptRef:buffered,setMailTextInputNotice:()=>{}},true);
 newText.value='Reader';newEnd({currentTarget:newText,data:'Reader'});assert.deepEqual(edges,[{op:'promptReplace',target:'recipient',text:'Reader'}]);
});
test('actual native listener follows the textarea element across Core generation and DOM replacement',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const proof={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:1,presentationRevision:1,layoutRevision:1,sequence:1};
 function textNode(){const listeners=new Set();return{listeners,addEventListener(type,listener){assert.equal(type,'beforeinput');listeners.add(listener);},
  removeEventListener(type,listener){assert.equal(type,'beforeinput');listeners.delete(listener);},
  input(data){for(const listener of [...listeners])listener({target:this,inputType:'insertText',data,isComposing:false,preventDefault(){}});}};}
 const first=textNode(),second=textNode(),textRef={current:first},context={status:{textTarget:'body',proof}},accepted=[];
 const callbacks={current:{context,onEdge:edge=>{accepted.push({generation:context.status.proof.draftGeneration,edge});return true;}}};
 const before=actualShellHandler('handleMailNativeBeforeInput',{mailTextRef:textRef,mailTextInputCallbacksRef:callbacks,
  mailDomCompositionRef:{current:null},mailCompositionEchoRef:{current:null},sameComposeAuthority:textInput.sameComposeAuthority,
  mailKeyBeforeInputRef:{current:null},setMailTextInputNotice:()=>{}});
 const effect=actualShellNativeListenerEffect({screen:'game',bevyMailComposeReady:true,mailTextPosition:{left:0},mailTextRef:textRef,
  mailNativeBeforeInputRef:{current:before}});
 const cleanup1=effect();assert.equal(first.listeners.size,1);first.input('A');
 context.status.proof={...proof,draftGeneration:'2',editorRevision:2,sequence:2};
 cleanup1();const cleanup2=effect();assert.equal(first.listeners.size,1,'same element retains exactly one native listener after Core generation rerender');
 first.input('B');assert.deepEqual(accepted.map(row=>[row.generation,row.edge.text]),[['1','A'],['2','B']]);
 cleanup2();textRef.current=second;const cleanup3=effect();assert.equal(first.listeners.size,0);assert.equal(second.listeners.size,1);
 first.input('stale');second.input('C');assert.deepEqual(accepted.map(row=>row.edge.text),['A','B','C'],'only the live element dispatches');
 cleanup3();assert.equal(second.listeners.size,0);
 const source=ts.createSourceFile('original-client-shell.tsx',readFileSync(new URL('../app/original-client-shell.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 let key=null;function visit(node){if(ts.isJsxSelfClosingElement(node)&&node.tagName.getText(source)==='textarea'&&
  node.attributes.properties.some(attribute=>ts.isJsxAttribute(attribute)&&attribute.name.text==='data-mail-text-adapter')){
  key=node.attributes.properties.find(attribute=>ts.isJsxAttribute(attribute)&&attribute.name.text==='key')?.getText(source)??null;
 }ts.forEachChild(node,visit);}visit(source);
 assert.ok(key);assert.doesNotMatch(key,/draftGeneration/,'ordinary Core generation must not remount the textarea');assert.match(key,/mailDomRetireKey/);
});
test('actual prompt buffer and confirm fence external Core generation and textarea identity',()=>{
 const owner={run:1,connectionGeneration:1,sessionGeneration:1,ownerRevision:0,sceneRevision:1,hudGeneration:1,playerObjectId:7};
 const proof1={owner,incarnation:1,draftEpoch:1,draftGeneration:'1',editorRevision:1,focusGeneration:2,presentationRevision:1,layoutRevision:1,sequence:1};
 const proof2={...proof1,draftGeneration:'2',editorRevision:2,sequence:2};
 const first={value:'Reader'},second={value:'Reader',focus(){}},textRef={current:first},buffered={current:null},pending={current:null};
 const context={status:{textTarget:'recipient',proof:proof1,modal:true,inputEnabled:true,promptText:'',promptViewport:{left:10,top:20,width:100,height:20},caret:null},
  raw:{body:''},presentation:{stageCssScale:1},pendingEdges:0};
 const edges=[];let submit=edge=>{edges.push(edge);return true;};
 const input=actualShellHandler('onInput',{mailTextRef:textRef,mailDomCompositionRef:{current:null},bevyMailTextContext:context,
  bufferedMailPromptRef:buffered,sameComposeScope:textInput.sameComposeScope,onBevyMailTextEdge:edge=>submit(edge)},true);
 // The thin queue accepted this browser event while its Rust setter refused admission.
 input({currentTarget:first,nativeEvent:{isComposing:false}});assert.equal(edges.length,1);assert.equal(buffered.current.proof.draftGeneration,'1');
 context.status.proof=proof2;
 input({currentTarget:first,nativeEvent:{isComposing:false}});
 assert.deepEqual(edges.at(-1),{op:'promptReplace',target:'recipient',text:'Reader'},'old generation buffer cannot swallow fresh same text');
 assert.equal(edges.length,2);
 assert.equal(buffered.current.proof.draftGeneration,'2');
 textRef.current=second;input({currentTarget:second,nativeEvent:{isComposing:false}});
 assert.equal(edges.length,3,'old element buffer cannot swallow new element same text');assert.equal(buffered.current.element,second);
 second.value='Newest';const latest={target:'recipient',text:'External',proof:proof2,element:second};
 submit=edge=>{edges.push(edge);buffered.current=latest;return false;};input({currentTarget:second,nativeEvent:{isComposing:false}});
 assert.equal(buffered.current,latest,'old refused callback cannot clear a newer prompt buffer');
 const staleDom={current:{target:'recipient',proof:proof1,element:first}},retire=actualShellHandler('retireMailDomComposition',{
  mailDomCompositionRef:staleDom,mailCompositionActiveRef:{current:null},mailCompositionEchoRef:{current:null},
  pendingMailPromptConfirmRef:pending,bufferedMailPromptRef:buffered,mailKeyBeforeInputRef:{current:null},sameComposeScope:textInput.sameComposeScope});
 retire();assert.equal(buffered.current,latest,'retiring old preedit cannot clear new element/Core generation buffer');
 pending.current={target:'recipient',text:'Reader',proof:proof1,element:first};
 const frame={getBoundingClientRect:()=>({left:0,top:0})},canvas={getBoundingClientRect:()=>({left:0,top:0})};
 actualShellComposeLayout({bevyMailComposeReady:true,bevyMailTextContext:context,stageFrameRef:{current:frame},document:{getElementById:()=>canvas,activeElement:second},
  sharedUiCanvasId:()=> 'shared',webGl2SharedCanvasPrototype:false,mailDomCompositionRef:{current:null},currentMailDomComposition:()=>false,
  retireMailDomComposition:()=>{},bufferedMailPromptRef:buffered,pendingMailPromptConfirmRef:pending,setMailDomRetireKey:()=>{},
  setMailTextPosition:()=>{},mailTextRef:textRef,sameComposeScope:textInput.sameComposeScope,onBevyMailComposeAction:()=>{throw Error('stale confirm');}})();
 assert.equal(pending.current,null,'old Core generation confirm cannot fire under new proof');
 let prevented=0;const newer={target:'recipient',text:'Newest',proof:proof2,element:second};
 submit=()=>{pending.current=newer;return false;};
 const key=actualShellHandler('onKeyDown',{mailTextRef:textRef,mailDomCompositionRef:{current:null},bevyMailTextContext:context,
  pendingMailPromptConfirmRef:pending,onBevyMailTextEdge:edge=>submit(edge),onBevyMailComposeAction:()=>{throw Error('unexpected confirm');}},true);
 key({currentTarget:second,key:'Enter',nativeEvent:{isComposing:false,keyCode:0},preventDefault(){prevented++;}});
 assert.equal(prevented,1);assert.equal(pending.current,newer,'retired response cannot clear replacement confirm intent');
});
test('actual Page common painter and Submit use current Core typed gold through prompt, body and parcel changes',()=>{
 const f=composePage(),owner=f.owner,sender=f.dispatcher.composer,plan={ids:[],stamped:false},synced=[],sent=[];
 const state={attachmentUniqueIds:[],stamped:false,quoteReady:false,postage:null,notice:null};
 const parcel={state,error:null,sync(_owner,_snapshot,gold){synced.push(gold);this.state={...state,attachmentUniqueIds:[...plan.ids],stamped:plan.stamped};
   return{ok:true,state:this.state,locks:[]};}};
 const deps={...f.deps,currentSpellsOwner:()=>owner,worldRef:{current:{playerObjectId:owner.playerObjectId,gold:1000,connected:true,
  inventoryItems:[{authoritativeUniqueId:41,icon:1,quantity:1}]}},mailCompatRef:{current:{view:'compose',key:'compose-1',owner}},
  mailRawRef:{current:{owner,mail:actualMail(),catalogResolved:true}},mailComposePromptRef:{current:null},
  mailParcelRef:{current:parcel},mailParcelRawRef:{current:null},sameComposeRaw:textInput.sameComposeRaw,
  readBevyQuestPresentation:()=>({logicalWidth:1024,logicalHeight:768,stageCssScale:1,touch:false}),
  document:{querySelector:()=>({}),visibilityState:'visible',hasFocus:()=>true},sharedUiCanvasId:()=> 'shared',
  webGl2SharedCanvasPrototype:false,clientProfile:{input:'keyboard'},bevyHpLocalOverlayOpenRef:{current:false},
  bevyQuestReactModalOpen:false,showInventory:false,showCharacter:false,showQuestLog:false,showHeroPet:false,
  showGuild:false,showGroup:false,showFriends:false,showBonds:false,showRanking:false,showMarket:false,
  showConquest:false,showTrade:false,showBuffs:false,showWorldMap:false,showHelp:false,showHotkeys:false,
  showChatSettings:false,npcShopService:null,npcRepairService:null,bevyQuestGenerationRef:{current:1},
  mailIngressRef:{current:{withdrawn:true}},bevyQuestUiRequested:true,bevyRuntimeBackend:'webgpu',
  sharedCanvasUsesWebGl2:()=>true,screenRef:{current:'game'},initialSceneAssetsReadyRef:{current:true},
  equipmentHostSuspendReasonRef:{current:null},bevyHudUi:{readCurrent:()=>({ready:true})},
  sendMailMessage:(draft,fromCommon)=>{sent.push({draft,fromCommon});return'confirmedSend';},
  sendMailParcelLocks:()=>{},queueMicrotask:()=>{},setMailComposeRevision:()=>{},pumpMailParcelQuote:()=>{}};
 const api=pageFunctions(['readMailComposeInput','syncMailParcel','dispatchComposeIntent'],deps),host={tick(){},edges:{pendingCount:0,ownEdit(){}}};
 const intent=(action,extra={})=>{const core=sender.composeSnapshot(owner);assert.ok(core);return{action,baseRaw:core.raw,
  proof:{draftGeneration:core.generation,incarnation:Number(core.incarnation)},bodyMutation:null,recipientMutation:null,
  promptMutation:null,promptValue:null,...extra};};
 let core=sender.composeSnapshot(owner);assert.ok(core);assert.equal(sender.rememberRaw(owner,'compose-1',{...core.raw,goldText:''}),true);
 assert.equal(api.readMailComposeInput()?.gold,0,'blank letter paints zero rather than wallet balance');
 api.dispatchComposeIntent(intent('gold'),host);assert.equal(api.readMailComposeInput()?.goldPrompt?.maxAmount,1000);
 api.dispatchComposeIntent(intent('promptEdit',{promptMutation:{target:'gold',text:'7'}}),host);
 api.dispatchComposeIntent(intent('goldConfirm',{promptMutation:{target:'gold',text:'7'},promptValue:7}),host);
 assert.equal(deps.mailComposePromptRef.current,null);assert.equal(synced.at(-1),7);
 assert.equal(api.readMailComposeInput()?.gold,7,'confirmed attachment paints seven, not wallet 1000');
 api.dispatchComposeIntent(intent('edit',{bodyMutation:'typed body'}),host);assert.equal(api.readMailComposeInput()?.gold,7);
 plan.ids.push(41);plan.stamped=true;assert.equal(api.syncMailParcel(),true);
 core=sender.composeSnapshot(owner);assert.deepEqual(core?.raw.attachmentUniqueIds,['41']);assert.equal(core?.raw.stamped,true);
 assert.equal(api.readMailComposeInput()?.gold,7,'parcel generation change cannot withdraw current Rust projection');
 api.dispatchComposeIntent(intent('submit'),host);assert.equal(sent.length,1);assert.equal(sent[0].fromCommon,true);
 assert.equal(sent[0].draft.gold,7);assert.equal(sent[0].draft.body,'typed body');assert.deepEqual(sent[0].draft.attachmentUniqueIds,[41]);
 core=sender.composeSnapshot(owner);assert.ok(core);assert.equal(sender.rememberRaw(owner,'compose-1',{...core.raw,goldText:'0x7'}),true);
 assert.equal(api.readMailComposeInput()?.gold,7,'valid legacy raw uses Core raw_gold, never JavaScript Number');
 api.dispatchComposeIntent(intent('submit'),host);assert.equal(sent.at(-1).draft.gold,7);
 const stale=intent('submit');core=sender.composeSnapshot(owner);assert.ok(core);
 assert.equal(sender.rememberRaw(owner,'compose-1',{...core.raw,body:'external replacement'}),true);
 api.dispatchComposeIntent(stale,host);assert.equal(sent.length,2,'stale full raw cannot send');
 deps.mailCompatRef.current={...deps.mailCompatRef.current,owner:{...owner,sessionGeneration:99}};
 assert.equal(api.readMailComposeInput(),null);api.dispatchComposeIntent(intent('submit'),host);assert.equal(sent.length,2,'wrong owner cannot submit');
 deps.mailCompatRef.current={...deps.mailCompatRef.current,owner};core=sender.composeSnapshot(owner);assert.ok(core);
 const dispatcher=deps.mailDispatcherRef.current;
 deps.mailDispatcherRef.current={composer:{composeSnapshot:()=>({...core,gold:null}),error(){},notice:null}};
 assert.equal(api.readMailComposeInput(),null,'old ABI-1 getter absence cannot paint wallet or guessed zero');
 api.dispatchComposeIntent(intent('submit'),host);assert.equal(sent.length,2,'missing typed projection cannot submit');
 deps.mailDispatcherRef.current=dispatcher;
 assert.equal(sender.rememberRaw(owner,'compose-1',{...core.raw,goldText:'bad'}),true);
 assert.equal(sender.composeSnapshot(owner)?.gold,null);assert.equal(api.readMailComposeInput(),null);
 api.dispatchComposeIntent(intent('submit'),host);assert.equal(sent.length,2,'invalid Core raw gold fails closed');
 const source=ts.createSourceFile('page.tsx',readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 let submit=null;function visit(node){if(ts.isFunctionDeclaration(node)&&node.name?.text==='dispatchComposeIntent')submit=node.getText(source);ts.forEachChild(node,visit);}visit(source);
 assert.ok(submit);assert.doesNotMatch(submit,/Number\(core\.raw\.goldText\)/,'new common Submit does not parse raw gold');
});
