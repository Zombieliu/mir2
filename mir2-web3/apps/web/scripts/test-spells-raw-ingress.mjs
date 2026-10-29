import assert from 'node:assert/strict';import test from 'node:test';import {readFileSync} from 'node:fs';import ts from 'typescript';
const source=readFileSync(new URL('../app/page.tsx',import.meta.url),'utf8'),ast=ts.createSourceFile('page.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
const pureAdapterCache=new Map();
function pureAdapter(name){assert.ok(['storage-gateway-adapter','mail-parcel-gateway-adapter','equipment-gateway-adapter','world-model/item-identity','bag-belt-move-dispatcher'].includes(name),'pure fixture module only');if(pureAdapterCache.has(name))return pureAdapterCache.get(name);const module={exports:{}};const javascript=ts.transpileModule(readFileSync(new URL(`../lib/${name}.ts`,import.meta.url),'utf8'),{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;new Function('exports','module','require',javascript)(module.exports,module,request=>pureAdapter(request.replace(/^\.\//,'')));pureAdapterCache.set(name,module.exports);return module.exports;}
const mailAdapter=pureAdapter('mail-parcel-gateway-adapter'),storageAdapter=pureAdapter('storage-gateway-adapter'),{BagBeltMoveDispatcher}=pureAdapter('bag-belt-move-dispatcher');
function pureAuthCommandHelpers(){const text=readFileSync(new URL('../lib/client-login-runtime.ts',import.meta.url),'utf8'),tree=ts.createSourceFile('login.ts',text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS),names=['preauthCommandKind','isSensitiveGatewayCommand'];const declarations=names.map(name=>{const n=tree.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text===name);assert.ok(n,name);return n.getText(tree).replace(/^export\s+/,'');});const javascript=ts.transpileModule(declarations.join('\n'),{compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;return new Function(`${javascript};return {${names.join(',')}};`)();}const authCommands=pureAuthCommandHelpers();
function declaration(name){let found;function visit(n){if(ts.isFunctionDeclaration(n)&&n.name?.text===name)found=n;ts.forEachChild(n,visit);}visit(ast);assert.ok(found,name);return found;}
function load(names,scope){const code=ts.transpileModule(names.map(n=>declaration(n).getText(ast)).join('\n'),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;return new Function(...Object.keys(scope),`${code};return {${names.join(',')}};`)(...Object.values(scope));}
test('actual Page detaches complete raw snapshot after connection/session guard, before coalescing',()=>{let ownerRevision=0;const deliveries=[];const owner={connectionGeneration:5,sessionGeneration:6,playerObjectId:7};
 const scope={equipmentStartGameRef:{current:owner},equipmentHostSuspendReasonRef:{current:null},equipmentConnectionGenerationRef:{current:5},equipmentSessionGenerationRef:{current:6},equipmentBagOwnerRef:{current:{ownerRevision}},spellsRawSnapshotRef:{current:null},combatRawRef:{current:null},combatBaselineSourceRef:{current:null},questCapturedWorldRef:{current:null},questSkillsBaselineCurrentRef:{current:false},combatMastersRef:{current:new Map()},combatIngressRef:{current:null},spellsIngressRef:{current:{observeSnapshot:(s,o)=>deliveries.push([s,o]),observePacket:(...v)=>deliveries.push(v)}},worldRef:{current:{playerObjectId:7}}};
 const api=load(['currentSpellsOwner','captureSpellsGatewayEvent'],scope);const first={type:'worldSnapshot',payload:{playerObjectId:7,knownSkills:[{spell:'FireBall',icon:0,need1:null,magicName:'火球'}],skillKeyAck:{requestId:4194305,spell:'FireBall',key:16,oldKey:1,accepted:false}}};
 api.captureSpellsGatewayEvent(first,4);assert.equal(deliveries.length,0);api.captureSpellsGatewayEvent(first,5);first.payload.knownSkills[0].spell='Healing';api.captureSpellsGatewayEvent({type:'worldSnapshot',payload:{playerObjectId:7,knownSkills:[]}},5);
 assert.equal(deliveries.length,2);assert.equal(deliveries[0][0].knownSkills[0].spell,'FireBall');assert.equal(deliveries[0][0].knownSkills[0].need1,null);assert.equal(deliveries[0][0].skillKeyAck.accepted,false);assert.equal(deliveries[1][0].skillKeyAck,undefined);assert.equal(deliveries[0][1].sessionGeneration,6);
 scope.worldRef.current.playerObjectId=99;api.captureSpellsGatewayEvent({type:'packet',packet:'MagicCast',payload:{spell:'FireBall'}},5);assert.equal(deliveries[2][2].playerObjectId,7,'raw owner remains exact while React player is stale');
 scope.equipmentSessionGenerationRef.current=8;api.captureSpellsGatewayEvent(first,5);assert.equal(deliveries.length,3);
 // Execute the actual handler until its capture boundary, before any later references run.
 const sentinel={},socket={};let captured=0,inventoryRetired=0;const head=load(['handleGatewayEvent'],{equipmentConnectionGenerationRef:{current:5},socketRef:{current:socket},invalidateNpcGoldBuyGatewayPacket:()=>{inventoryRetired++;},captureQuestMapGatewayEvent:()=>{},captureSpellsGatewayEvent:()=>{captured++;throw sentinel;}});
 assert.doesNotThrow(()=>head.handleGatewayEvent(first,4,socket));assert.throws(()=>head.handleGatewayEvent(first,5,socket),e=>e===sentinel);assert.equal(captured,1);assert.equal(inventoryRetired,1);
});
function sender(){const sends=[];let proofValid=true,listener=()=>{};const socket={readyState:1,send:j=>sends.push(JSON.parse(j))};const scope={isSpectatorBrowserMode:()=>false,socketRef:{current:socket},WebSocket:{OPEN:1},lastCommandRef:{current:null},npcPearlSendEpochRef:{current:0},bagBeltMovesRef:{current:new BagBeltMoveDispatcher()},bagBeltInventoryReadyRef:{current:null},equipmentSnapshotRef:{current:null},heroOperationsRef:{current:{pending:null}},mailCollectBarrierRef:{current:null},socialItemOperationsRef:{current:{pending:null}},preauthCommandKind:authCommands.preauthCommandKind,isSensitiveGatewayCommand:authCommands.isSensitiveGatewayCommand,isMovementPredictionBlockingCommand:()=>false,isMovementConsoleCommand:()=>false,isMovementCommand:()=>false,isCombatResolutionCommand:()=>false,recordDebugEvent:()=>{},window:{dispatchEvent:()=>listener()},CustomEvent:class{constructor(type,options){this.type=type;this.detail=options.detail;}},itemCommandRequiresOwner:()=>false,mailParcelRef:{current:null},mailDispatcherRef:{current:null},combatIngressRef:{current:null},mailMutationAllowed:mailAdapter.mailMutationAllowed,isMailItemMutation:mailAdapter.isMailItemMutation,storageMutationAllowed:storageAdapter.storageMutationAllowed,pendingStorageRequestsRef:{current:new Map()},worldRef:{current:{}},bevySpellsSendGateRef:{current:()=>proofValid},appendLog:()=>{},t:()=>''};const api=load(['parityItemMutationAllowed','socialItemMutationAllowed','sendRaw','dispatchBevySpellsIntent'],scope);return{api,sends,scope,set listener(f){listener=f;},set proofValid(v){proofValid=v;}};}
const intent={requestRun:1,connectionGeneration:2,sessionGeneration:3,ownerRevision:0,hudGeneration:4,playerObjectId:5,modelRevision:6,presentationRevision:7,renderRevision:8,skillId:0,requestId:4194305,spell:'FireBall',key:16,oldKey:1};
test('actual sendRaw checks proof after synchronous mir2:action and directly before socket send',()=>{const f=sender();f.listener=()=>{f.proofValid=false;};assert.equal(f.api.dispatchBevySpellsIntent(intent,intent),'definitelyUnsent');assert.equal(f.sends.length,0);
 f.proofValid=true;f.listener=()=>{f.scope.socketRef.current={readyState:1};};assert.equal(f.api.dispatchBevySpellsIntent(intent,intent),'definitelyUnsent');assert.equal(f.sends.length,0);
 const success=sender();assert.equal(success.api.dispatchBevySpellsIntent(intent,intent),'confirmedSend');assert.deepEqual(success.sends,[{type:'magicKey',requestId:4194305,spell:'FireBall',key:16,oldKey:1}]);
 assert.equal(success.api.sendRaw({type:'magicKey',requestId:4194305,spell:'Healing',key:16,oldKey:1},{skillProof:intent}),false);assert.equal(success.sends.length,1);
 const unknown=sender();unknown.scope.socketRef.current.send=()=>{throw Error('possibly sent');};assert.equal(unknown.api.dispatchBevySpellsIntent(intent,intent),'outcomeUnknown');assert.equal(unknown.sends.length,0);
});


function authenticatedSpellsBootstrapFixture(knownSkills = []) {
 const socket = {readyState: 1}, session = {connectionGeneration: 5, sessionGeneration: 6, afterSnapshotVersion: 3};
 const oldOwner = {connectionGeneration: 5, sessionGeneration: 6, ownerRevision: 0, playerObjectId: 7};
 const cached = {snapshot: {playerObjectId: 7, knownSkills: [], skillKeyAck: {requestId: 4194305, spell: 'FireBall', key: 16, oldKey: 1, accepted: false}}, owner: {...oldOwner}};
 const physical = {socket, session, connectionGeneration: 5, sessionGeneration: 6, sceneRevision: 2, snapshotVersion: 4,
  snapshot: {playerObjectId: 7, knownSkills, mapFileName: 'Bichon'}};
 const scope = {socketRef: {current: socket}, WebSocket: {OPEN: 1}, equipmentStartGameRef: {current: session},
  equipmentHostSuspendReasonRef: {current: null}, equipmentConnectionGenerationRef: {current: 5}, equipmentSessionGenerationRef: {current: 6},
  equipmentBagOwnerRef: {current: {ownerRevision: 1}}, questSceneRevisionRef: {current: 2}, worldSnapshotVersionRef: {current: 4},
  worldRef: {current: {connected: true, playerObjectId: '7', mapFileName: 'Bichon'}}, combatBaselineSourceRef: {current: physical},
  spellsRawSnapshotRef: {current: cached}};
 const api = load(['currentSpellsOwner', 'readCurrentCombatBaseline', 'readCurrentSpellsBaseline'], scope);
 let options;
 function visit(n) {if (ts.isVariableDeclaration(n) && n.name.getText(ast) === 'bevySpellsUi') options = n.initializer.arguments[0]; ts.forEachChild(n, visit);}
 visit(ast); assert.ok(options, 'actual Page Spells hook options');
 const getter = options.properties.find(n => n.name?.getText(ast) === 'readBaseline'); assert.ok(getter);
 const code = ts.transpileModule('const getBaseline = ' + getter.initializer.getText(ast) + ';', {compilerOptions: {target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None}}).outputText;
 const getBaseline = new Function('readCurrentSpellsBaseline', 'spellsRawSnapshotRef', code + ';return getBaseline;')(api.readCurrentSpellsBaseline, scope.spellsRawSnapshotRef);
 return {scope, physical, cached, api, getBaseline};
}
function bootstrapSpellsHost(f) {
 const module = {exports: {}};
 new Function('exports', 'module', ts.transpileModule(readFileSync(new URL('../lib/bevy-spells-ui.ts', import.meta.url), 'utf8'), {compilerOptions: {module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022}}).outputText)(module.exports, module);
 let sent, frame = 1; const diagnostics = [];
 const runtime = {getMir2SpellsUiCapabilities: () => JSON.stringify({schemaVersion: 1, spellsPageAbiVersion: 1, spellsIntentAbiVersion: 1, compiled: true, startup: true}),
  setMir2SpellsUiSnapshot: json => {sent = JSON.parse(json); return true;},
  getMir2SpellsUiStatus: () => JSON.stringify({...sent, version: 1, frame: frame++, ready: sent?.open === true, inputEnabled: sent?.inputEnabled === true,
   modal: false, pending: false, renderRevision: 7, appliedRevision: sent?.revision ?? 0, appliedModelRevision: sent?.modelRevision ?? 0,
   appliedPresentationRevision: sent?.presentationRevision ?? 0, inputRegions: [], error: null}),
  setMir2SpellsUiIntentSink: () => {}, clearMir2SpellsUiIntentSink: () => {}, setMir2SpellsUiPointerEdge: () => true};
 const input = {owner: f.api.currentSpellsOwner(7), hudGeneration: 4, eligible: true, open: true,
  player: {name: 'Hero', hp: 1, maxHp: 1, mp: 1, maxMp: 1, level: 1, gold: 0, credit: 0, experience: 0, maxExperience: 1, currentWeight: 0, currentWeightKnown: false, maxWeight: 1},
  presentation: {logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false}, iconMetadata: {version: 3, frames: []}};
 const host = new module.exports.SpellsHost({runtime, isCurrent: () => true, read: () => input, now: () => 10, onState: () => {},
  onDiagnostic: d => diagnostics.push(d), onIntent: () => {throw Error('bootstrap cannot send input');}});
 return {host, diagnostics, get sent() {return sent;}};
}
test('actual Page bootstrap accepts authenticated empty learned after Bag owner initialization without retagging old cache or replaying ACK', () => {
 const f = authenticatedSpellsBootstrapFixture(), cacheBefore = JSON.stringify(f.cached), sourceBefore = JSON.stringify(f.physical), h = bootstrapSpellsHost(f);
 try {
  assert.equal(h.host.observeSnapshot(f.cached.snapshot, f.cached.owner), false, 'old bootstrap cache is rejected by the real Host owner gate');
  h.host.tick(); assert.equal(h.diagnostics.at(-1).reason, 'baseline-missing');
  const baseline = f.getBaseline(); assert.equal(baseline.owner.ownerRevision, 1); assert.deepEqual(baseline.snapshot.knownSkills, []);
  assert.equal(Object.hasOwn(baseline.snapshot, 'skillKeyAck'), false, 'new controller bootstrap cannot replay an old receipt');
  assert.equal(h.host.observeSnapshot(baseline.snapshot, baseline.owner), true); assert.equal(h.diagnostics.at(-1).reason, 'ready');
  assert.deepEqual(h.sent.learned, []); assert.deepEqual(h.sent.receipts, []);
  assert.equal(JSON.stringify(f.cached), cacheBefore); assert.equal(JSON.stringify(f.physical), sourceBefore);
 } finally {h.host.stop();}
});
test('actual Spells bootstrap rejects stale socket/session/scene/version/self/map and suspended sources', () => {
 const rejects = [
  ['missing source', f => {f.scope.combatBaselineSourceRef.current = null;}],
  ['replaced socket', f => {f.scope.socketRef.current = {readyState: 1};}],
  ['closed socket', f => {f.physical.socket.readyState = 3;}],
  ['missing session', f => {f.scope.equipmentStartGameRef.current = null;}],
  ['replaced session object', f => {f.scope.equipmentStartGameRef.current = {...f.physical.session};}],
  ['connection generation', f => {f.physical.connectionGeneration++;}],
  ['session generation', f => {f.physical.sessionGeneration++;}],
  ['current connection generation', f => {f.scope.equipmentConnectionGenerationRef.current++;}],
  ['current session generation', f => {f.scope.equipmentSessionGenerationRef.current++;}],
  ['suspended host', f => {f.scope.equipmentHostSuspendReasonRef.current = 'disconnected';}],
  ['scene revision', f => {f.scope.questSceneRevisionRef.current++;}],
  ['snapshot version', f => {f.scope.worldSnapshotVersionRef.current++;}],
  ['pre StartGame version', f => {f.physical.session.afterSnapshotVersion = 4;}],
  ['disconnected projection', f => {f.scope.worldRef.current.connected = false;}],
  ['different player', f => {f.scope.worldRef.current.playerObjectId = '8';}],
  ['different raw self', f => {f.physical.snapshot.playerObjectId = 8;}],
  ['invalid player', f => {f.scope.worldRef.current.playerObjectId = '0';}],
  ['empty map', f => {f.scope.worldRef.current.mapFileName = '';}],
  ['different map', f => {f.scope.worldRef.current.mapFileName = 'SnakeCave';}],
  ['different source map', f => {f.physical.snapshot.mapFileName = 'SnakeCave';}]
 ];
 for (const [label, mutate] of rejects) {const f = authenticatedSpellsBootstrapFixture(); mutate(f); const before = JSON.stringify(f.cached); assert.equal(f.getBaseline(), null, label); assert.equal(JSON.stringify(f.cached), before, label + ' cannot revive cached authority');}
});
test('actual Spells bootstrap distinguishes genuine empty learned from missing null nonarray and overflow without normalizing raw fields', () => {
 for (const [label, learned] of [['missing', undefined], ['null', null], ['object', {}], ['string', '[]'], ['overflow', Array.from({length: 513}, () => ({spell: 'FireBall'}))]]) {
  const f = authenticatedSpellsBootstrapFixture(); if (label === 'missing') delete f.physical.snapshot.knownSkills; else f.physical.snapshot.knownSkills = learned;
  const before = JSON.stringify(f.physical); assert.equal(f.getBaseline(), null, label); assert.equal(JSON.stringify(f.physical), before);
 }
 const learned = [{spell: 'FireBall', icon: 0, need1: null, hotkey: 0}], f = authenticatedSpellsBootstrapFixture(learned);
 assert.deepEqual(f.getBaseline().snapshot.knownSkills, learned); assert.equal(f.getBaseline().snapshot.knownSkills, f.physical.snapshot.knownSkills);
});
