import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// Strictly pure source dependencies; no WASM/runtime loader or filesystem fixture writes.
const sources = { fishingSource: "../lib/world-fishing-source.ts", bagBeltMove: "../lib/bag-belt-move-dispatcher.ts", identity: "../lib/world-model/item-identity.ts",
  equipment: "../lib/equipment-gateway-adapter.ts", parcel: "../lib/mail-parcel-gateway-adapter.ts",
  storage: "../lib/storage-gateway-adapter.ts", social: "../lib/social-incoming-replies.ts",
  operations: "../lib/social-window-operations.ts", rental: "../lib/storage-rental-confirmation.ts",
  bag: "../lib/bevy-bag-model.ts", socialItems: "../lib/social-item-window-model.ts",
  stage5:"../lib/stage5-window-adapters.ts", tooltip:"../lib/shared-item-tooltip.ts", guildBuff:"../lib/guild-buff-ui.ts", socialActions: "../lib/social-parity-actions.ts", extended: "../lib/extended-server-packets.ts" };
const allow = { fishingSource: {}, bagBeltMove: { "./mail-parcel-gateway-adapter": "parcel" }, identity: {}, equipment: { "./world-model/item-identity": "identity" },
  parcel: { "./equipment-gateway-adapter": "equipment" },
  storage: { "./equipment-gateway-adapter": "equipment", "./world-model/item-identity": "identity",
    "./mail-parcel-gateway-adapter": "parcel" }, social: {}, operations: {},
  rental: { "./social-incoming-replies": "social" }, bag: { "./world-model/item-identity": "identity" },
  socialItems: { "./bevy-bag-model": "bag", "./world-model/item-identity": "identity" }, stage5:{}, tooltip:{}, guildBuff:{}, socialActions: {}, extended: {} };
const modules = new Map();
function loadPure(name) {
  assert(Object.hasOwn(sources, name), "module outside pure allowlist");
  if (modules.has(name)) return modules.get(name);
  const url = new URL(sources[name], import.meta.url);
  const code = ts.transpileModule(readFileSync(url, "utf8"), { fileName: fileURLToPath(url),
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const module = { exports: {} };
  new Function("require", "exports", "module", code)((id) => {
    assert(Object.hasOwn(allow[name], id), "unexpected " + name + " dependency: " + id);
    return loadPure(allow[name][id]);
  }, module.exports, module);
  modules.set(name, module.exports);
  return module.exports;
}

function extractPureDeclarations(relative, requested) {
  const url = new URL(relative, import.meta.url), source = readFileSync(url, "utf8");
  const ast = ts.createSourceFile(fileURLToPath(url), source, ts.ScriptTarget.Latest, true);
  const selected = new Map();
  for (const node of ast.statements) {
    const key = (ts.isFunctionDeclaration(node) || ts.isClassDeclaration(node)) ? node.name?.text
      : ts.isVariableStatement(node) && node.declarationList.declarations.length === 1
        ? node.declarationList.declarations[0].name.getText(ast) : null;
    if (requested.includes(key)) { assert(!selected.has(key)); selected.set(key,node.getText(ast).replace(/^export /,"")); }
  }
  assert.deepEqual([...selected.keys()].sort(),requested.slice().sort(),"complete actual pure declarations");
  const js = ts.transpileModule([...selected.values()].join("\n"),{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
  return new Function(js+"\nreturn {"+requested.join(",")+"};")();
}
// Select data-only declarations; neither passkey imports nor the Core loader/manifest run.
const pureAuth = extractPureDeclarations("../lib/client-login-runtime.ts",["PreauthFlightGate","preauthGateKey",
  "persistentPreauthGate","preauthCommandKind","isSensitiveGatewayCommand","newAccountCommand",
  "sendNewAccountCommand","sendChangePasswordCommand","sendPasswordLoginCommand"]);
const pureAuthCore = extractPureDeclarations("../lib/client-core-runtime.ts",["authErrors","authValidationError","createAuthUiRuntime","slotUtf8"]);

const adapter = loadPure("storage");
const item = (id, container, slot) => ({ uniqueId: id, authoritativeUniqueId: id, container, slot,
  key: "item-" + id, icon: 1, description: "fixture item", name: "same display name", quantity: 1,
  tooltipSource: { info: { item_type: 3 } } });
function world() {
  return { inventoryCapacity: 54, maxBagSlots: 48, storageSize: 160, hasExpandedStorage: true,
    gold: 2_000_000, expandedStorageExpiryTimeBinaryDatetime: 0, connected: true,
    playerObjectId: "3", mapFileName: "D001", stage5Systems: { guild: {name: "Guild"}, trade: null },
    entities: [{objectId: "3", name: "Self"}], rankings: {}, rankingCurrentKey: null,
    hasStoragePassword: false, requireStoragePassword: false, storageSessionUnlocked: true,
    inventoryItems: [item(11, "bag1", 3), item(22, "bag2", 3), item(33, "bag1", 7)],
    storageItems: [item(44, "storage", 83)], beltItems: [], equipmentItems: [] };
}
const selection = row => ({ container: row.container, slot: row.slot, uniqueId: row.uniqueId,
  authoritativeUniqueId: row.authoritativeUniqueId });
const requestId = seq => "st-" + String(seq).padStart(16, "0");


// Extract complete, current nested Page declarations. No React, client-core
// runtime, WASM, socket implementation, or renderer is imported/initialized.
const pageUrl = new URL("../app/page.tsx", import.meta.url);
const pageSource = readFileSync(pageUrl, "utf8");
const pageAst = ts.createSourceFile(fileURLToPath(pageUrl), pageSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const names = ["worldFishingOwner", "cancelWorldFishingGesture", "retireWorldFishingGesture", "publishWorldFishingSource", "captureWorldFishingSnapshot", "emptyLoginAuthState", "setLoginBusy", "preauthGate", "updateLoginAuth", "authSurfaceCurrent",
  "openLoginAuth", "closeLoginAuth", "changeRegistrationField", "changePasswordField", "submitRegistration", "submitChangePassword",
  "setSafeKeyFocus", "editSafeKey", "randomSafeKeys", "queuePreauthAttempt", "sendPreauthAttempt", "consumePreauthReply",
  "submitPasswordLoginWithCredentials", "submitLogin", "submitSuiLogin", "submitIdentitySession",
  "quickEnterWorld", "captureIdentityIntent", "identityIntentCurrent", "linkCurrentSuiIdentity", "setIdentityLinkBusy",
  "clearGatewayReconnectTimer", "setGatewayReconnectStatus", "resetGatewayReconnectState", "createIdleReconnectStatus", "flushGatewayProtocolQueue", "captureSpouseUiSource", "spouseUiSourceCurrent", "mailSpouse", "whisperSpouse", "openMailWindow",
  "submitStorageTransfer", "storeItem", "takeBackItem", "send", "sendRaw",
  "itemCommandRequiresOwner", "retireEquipmentSession", "advanceHeroWindowEpochs", "mailParcelItemsIdle", "endStorageService", "closeNpcRepairService", "retireNpcShopService",
  "currentSpellsOwner", "currentSocialReplyOwner", "currentSocialReceiveOwner", "sameSocialPhysicalOwner",
  "socialReplyWindowOpen", "captureSocialRequest", "closeSocialReplyWindow", "replySocialRequest", "retireSocialRequests",
  "retireSocialItemConnection", "socialItemMutationAllowed", "parityItemMutationAllowed", "readSocialItemSurface", "socialItemProofCurrent",
  "submitSocialItem", "socialItemReceiptOwner", "observeSocialItemReceipt", "observeGuildPermissions", "observeGuildStorageChange",
  "socialTradeProofCurrent", "rentExpandedStorage", "currentStorageRentalFacts", "closeStorageRentalUi", "cancelStorageRental",
  "confirmStorageRental", "observeEquipmentSnapshot", "syncEquipmentSnapshot", "suspendEquipmentConnection",
  "stringOrFallback", "numberOrUndefined", "numberOrZero", "mapClassKey", "rankingPageKey",
  "setShowGuild", "setShowGroup", "setShowBonds", "setShowRanking", "friendCharacterIndex", "removeFriendEntry", "editFriendMemo",
  "changeGuildMemberRank", "saveGuildRank", "rankingRequestForTab", "issueRankingRequest", "requestRanking", "refreshRanking",
  "moveRankingRows", "setRankingOnlineOnly", "applyRankingPacket", "refreshFriends", "setShowFriends",
  "readSocialRosterSource", "socialRosterProofCurrent", "sendSocialRosterCommand", "projectPartnerTradePacket", "sendTradeUiCommand", "confirmTrade"];
const declarations = new Map();
const ackCases = [];
const socialCases = new Map();
const socialPacketNames = ["GroupInvite", "MarriageRequest", "DivorceRequest", "MentorRequest", "GuildStatus", "GuildStorageList",
  "GuildStorageItemChange", "DepositTradeItem", "RetrieveTradeItem", "LogOutFailed", "FriendUpdate", "GuildInvite", "GuildMemberChange", "Rankings",
  "TradeItem", "NewItemInfo", "NewRecipeInfo", "Connected", "NewAccount", "Login", "LoginBanned", "ChangePassword"];
let authControlsInitializer;
let spouseRenderInitializer, spouseRenderCallbacks;
let gatewayGuard, snapshotStatements, snapshotRevision, socialSnapshotFriends, socialWindowRender, rosterRenderInitializer, tradeLeaseInitializer;
function visit(node) {
  if (ts.isVariableDeclaration(node) && node.name.getText(pageAst) === "loginAuthControls") {
    assert.equal(authControlsInitializer, undefined, "sole actual Auth controls lease");
    authControlsInitializer = node.initializer.getText(pageAst);
  }
  if (ts.isVariableDeclaration(node) && node.name.getText(pageAst) === "spouseUiSource") {
    assert.equal(spouseRenderInitializer, undefined, "sole actual spouse render capture");
    spouseRenderInitializer = node.initializer.getText(pageAst);
  }
  if (ts.isJsxAttribute(node) && node.name.text === "bonds" && node.initializer
    && ts.isJsxExpression(node.initializer) && ts.isObjectLiteralExpression(node.initializer.expression)) {
    assert.equal(spouseRenderCallbacks, undefined, "sole actual Bonds props binding");
    spouseRenderCallbacks = node.initializer.expression.properties.filter(p => ts.isPropertyAssignment(p)
      && ["onMailPartner", "onWhisperPartner"].includes(p.name.getText(pageAst))).map(p => p.getText(pageAst));
    assert.equal(spouseRenderCallbacks.length, 2, "actual Page binds both spouse callbacks");
  }
  if (ts.isFunctionDeclaration(node) && node.name && names.includes(node.name.text)) {
    assert(!declarations.has(node.name.text), "ambiguous Page declaration: " + node.name.text);
    declarations.set(node.name.text, node.getText(pageAst));
  }
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression)
    && node.expression.text === "TakeBackItemV2") ackCases.push(node);
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression) && socialPacketNames.includes(node.expression.text)) {
    assert(!socialCases.has(node.expression.text), "sole actual social receive case");
    socialCases.set(node.expression.text, node.getText(pageAst));
  }
  if (ts.isFunctionDeclaration(node) && node.name?.text === "handleGatewayEvent") {
    gatewayGuard = node.body.statements[0].getText(pageAst);
    const snapshots = node.body.statements.find(n => ts.isIfStatement(n) && n.expression.getText(pageAst) === 'event.type === "worldSnapshot"');
    snapshotRevision = snapshots.thenStatement.statements[0].getText(pageAst);
  }
  if (ts.isFunctionDeclaration(node) && node.name?.text === "applyGatewayWorldSnapshot") {
    const statements = node.body.statements;
    const index = statements.findIndex(n => n.getText(pageAst).startsWith("const socialOwner ="));
    assert(index > 0);
    // Source23 inserts typed Fishing capture between the existing raw Equipment
    // barrier and Social observation. Keep both original statements in this fragment.
    assert.equal(statements[index - 2].getText(pageAst), "observeEquipmentSnapshot(snapshot, connectionGeneration);");
    assert.equal(statements[index - 1].getText(pageAst), "captureWorldFishingSnapshot(snapshot, connectionGeneration);");
    snapshotStatements = statements.slice(index - 2, index + 2).map(n => n.getText(pageAst)).join("\n");
  }
  if (ts.isPropertyAssignment(node) && node.name.getText(pageAst) === "social" && node.initializer.getText(pageAst).includes("socialFriendsRef.current")) {
    assert.equal(socialSnapshotFriends, undefined, "sole actual full snapshot FriendUpdate preservation expression");
    socialSnapshotFriends = node.initializer.getText(pageAst);
  }
  if (ts.isExpressionStatement(node) && node.getText(pageAst).startsWith("socialReplyWindowsRef.current = { group:")) {
    assert.equal(socialWindowRender, undefined, "sole actual rendered social window ownership assignment");
    socialWindowRender = node.getText(pageAst);
  }
  if (ts.isVariableDeclaration(node) && node.name.getText(pageAst) === "socialRosterRenderSources") {
    assert.equal(rosterRenderInitializer, undefined, "sole actual render-captured roster source");
    rosterRenderInitializer = node.initializer.getText(pageAst);
  }
  if (ts.isVariableDeclaration(node) && node.name.getText(pageAst) === "socialTradeUiLease") {
    assert.equal(tradeLeaseInitializer, undefined, "sole actual Trade UI render lease");
    tradeLeaseInitializer = node.initializer.getText(pageAst);
  }
  ts.forEachChild(node, visit);
}
visit(pageAst);
assert.deepEqual([...declarations.keys()].sort(), [...names].sort(), "actual Page declarations must remain available");
assert.equal(ackCases.length, 1, "use the sole actual storage receipt case");
const ackCase = ackCases[0];
assert.equal(ackCase.parent.clauses[ackCase.parent.clauses.indexOf(ackCase) - 1].expression.text,
  "StoreItemV2", "both real receipt names share this actual case");

let authCloseStatements;
function visitAuthClose(node) {
  if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression)
    && node.expression.expression.getText(pageAst)==="socket" && node.expression.name.text==="addEventListener"
    && ts.isStringLiteral(node.arguments[0]) && node.arguments[0].text==="close") {
    assert.equal(authCloseStatements,undefined,"sole actual physical-close listener");
    const statements=node.arguments[1].body.statements;
    assert.equal(statements[0].getText(pageAst),"if (socketRef.current !== socket) return;");
    const start=statements.findIndex(n=>n.getText(pageAst)==="pendingGatewayProtocolActionRef.current = null;");
    const end=statements.findIndex((n,i)=>i>start&&n.getText(pageAst)==="setLoginBusy(false);");
    assert(start>0&&end>start);
    const socketClear=statements.find(n=>n.getText(pageAst)==="socketRef.current = null;");
    assert.ok(socketClear);
    authCloseStatements=[statements[0],socketClear,...statements.slice(start,end+1)].map(n=>n.getText(pageAst)).join("\n");
  }
  ts.forEachChild(node,visitAuthClose);
}
visitAuthClose(pageAst);assert.ok(authCloseStatements);

const actualFunctions = names.map(name => declarations.get(name)).join("\n")
  + "\nfunction applyAuthSocketClose(socket) {" + authCloseStatements + "\n}"
  + '\nfunction applyStorageAck(event) { const payload=event.payload??{}; switch(event.packet) { case "StoreItemV2":\n'
  + ackCase.getText(pageAst) + "\n} }\n"
  + "function applySocialEvent(event, connectionGeneration, source) {" + gatewayGuard
  + "\nconst payload=event.payload??{}; switch(event.packet) {\n" + socialPacketNames.map(n => socialCases.get(n)).join("\n") + "\n}}\n"
  + "function applySocialSnapshot(snapshot, connectionGeneration, source) {" + gatewayGuard
  + "\n" + snapshotRevision + "\n" + snapshotStatements + "\n}"
  + "\nfunction projectSocialSnapshotFriends(snapshot, current, socialOwner) {return " + socialSnapshotFriends + ";}"
  + "\nfunction applySocialWindowRender(showGroup, showBonds, showGuild) {" + socialWindowRender + "}"
  + "\nfunction captureSocialRosterRender(showFriends, showGuild) {return " + rosterRenderInitializer + ";}"
  + "\nfunction captureSocialTradeUiLease() {return " + tradeLeaseInitializer + ";}"
  + "\nfunction captureSpouseRender(showBonds, extraWindowData) { const spouseUiSource = " + spouseRenderInitializer
  + ";return {" + spouseRenderCallbacks.join(",") + "};}"
  + "\nfunction captureLoginAuthControls(loginAuthState, authCoreReady, loginBusy) { const authRenderEpoch = loginAuthState.epoch; return " + authControlsInitializer + ";}";
assert.equal(socialCases.size, socialPacketNames.length);
assert.equal(gatewayGuard, "if (connectionGeneration !== equipmentConnectionGenerationRef.current || socketRef.current !== source) return;",
  "retain the actual source socket and connection admission guard");
assert.equal(snapshotRevision, "worldSnapshotVersionRef.current += 1;");
assert.ok(socialSnapshotFriends && socialWindowRender && rosterRenderInitializer && tradeLeaseInitializer);
assert.ok(spouseRenderInitializer && spouseRenderCallbacks);
assert.ok(authControlsInitializer);
// Extract only the current pure normalizer; its type-only world import is not run.
const pureQuestMap = extractPureDeclarations("../lib/bevy-quest-world-context.ts", ["normalizeQuestMapFileName"]);
const fishingRefNames = ["worldFishingSourceRef", "worldFishingCommitRef", "worldFishingGestureRegistryRef",
  "worldFishingActiveGestureRef", "worldFishingQueuedRef", "worldFishingCastClockRef", "questSceneRevisionRef"];
const fishingRefInitializers = new Map();
(function visitFishingRefs(node) {
  if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && fishingRefNames.includes(node.name.text)) {
    assert(!fishingRefInitializers.has(node.name.text), "sole actual Page Fishing ref " + node.name.text);
    assert.ok(node.initializer && ts.isCallExpression(node.initializer) && node.initializer.expression.getText(pageAst) === "useRef");
    fishingRefInitializers.set(node.name.text, node.initializer.arguments[0].getText(pageAst));
  }
  ts.forEachChild(node, visitFishingRefs);
})(pageAst);
assert.equal(fishingRefInitializers.size, fishingRefNames.length, "actual Page dormant Fishing custody initializers");
const fishingRefsJs = ts.transpileModule([...fishingRefInitializers].map(([name, initializer]) =>
  "const " + name + " = {current:" + initializer + "};").join("\n"), {compilerOptions: {target: ts.ScriptTarget.ES2022}}).outputText;
const pureFishingSource = loadPure("fishingSource");
const pageJavaScript = ts.transpileModule(actualFunctions, {
  fileName: "actual-storage-page-gates.ts",
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;
const pureIdentity = loadPure("identity");
const pureEquipment = loadPure("equipment");
const pureParcel = loadPure("parcel");
const pureSocial = loadPure("social"), pureOperations = loadPure("operations"), pureRental = loadPure("rental"), pureSocialItems = loadPure("socialItems");
const pureSocialActions = loadPure("socialActions"), pureExtended = loadPure("extended");
const mailOwnerSource = readFileSync(new URL("../lib/bevy-mail-ui.ts", import.meta.url), "utf8");
const mailOwnerAst = ts.createSourceFile("mail-owner.ts", mailOwnerSource, ts.ScriptTarget.Latest, true);
const mailOwnerNodes = mailOwnerAst.statements.filter(n => ts.isFunctionDeclaration(n) && n.name?.text === "sameMailOwner"
  || ts.isVariableStatement(n) && n.declarationList.declarations.some(d => d.name.getText(mailOwnerAst) === "ownerKeys"));
assert.equal(mailOwnerNodes.length, 2);
const actualSameMailOwner = new Function(ts.transpileModule(mailOwnerNodes.map(n => n.getText(mailOwnerAst).replace(/^export /, "")).join("\n"),
  {compilerOptions: {target: ts.ScriptTarget.ES2022}}).outputText + "\nreturn sameMailOwner;")();
const session = { connectionGeneration: 3, sessionGeneration: 5 };
const owner = { ...session, runGeneration: 0, ownerRevision: 1, owner: "react" };

function harness({ identity = session, sequenceRef = { current: 1 }, pendingRef = { current: new Map() } } = {}) {
  const sent = [], errors = [], logs = [], terminations = [], npcWithdrawals = [], npcInventoryRetirements = [];
  let listener = null, socketThrows = false, socketThrowType = null, attempts = 0, actions = 0;
  const socket = { readyState: 1, send(body) {
    attempts++; if (socketThrows && (!socketThrowType || JSON.parse(body).type === socketThrowType)) throw Error("controlled socket.send outcome unknown");
    sent.push(JSON.parse(body));
  } };
  const fixtureOwner = { ...owner, ...identity };
  const liveOwner = { current: { ...fixtureOwner } };
  const scope = {
    authStateEvents: [], authBusyEvents: [], authErrorEvents: [], authConnections: [], authMilestones: [],
    loginAuthRef: {current: null}, authCoreRef:{current:null}, preauthGateRef:{current:null}, authSendRef:{current:null},
    loginBusyRef:{current:false}, gatewayProtocolReadyRef:{current:true}, pendingGatewayProtocolActionRef:{current:null},
    oauthIntentRef:{current:0}, accountIdRef:{current:"Existing"}, passwordRef:{current:"ExistingSecret"}, activeReconnectAuthRef:{current:null},
    reconnectTimerRef:{current:null}, reconnectAttemptRef:{current:0}, reconnectSnapshotRef:{current:null}, reconnectStatusRef:{current:{mode:"idle",attempt:0,nextAttemptAt:null}},
    setLoginAuthState: value => scope.authStateEvents.push(value), setLoginBusyState: value => scope.authBusyEvents.push(value),
    setLoginErrorKey: value => scope.authErrorEvents.push(value), setAuthCoreReady: value => {scope.authReady=value;},
    setAccountId: value => {scope.accountDisplay=value;}, setPassword: value => {scope.passwordDisplay=value;}, setIdentityProvider: value => {scope.identityProvider=value;},
    setWalletPickerOpen: value => {scope.walletPicker=value;}, setReconnectStatus: value => {scope.reconnectStatus=value;},
    connectGateway: (bootstrap, fresh) => scope.authConnections.push({bootstrap,fresh}),
    requestSuiLoginToken: (kind, walletId) => scope.oauthProvider(kind,walletId),
    markMir2CacheMilestone: (...args) => scope.authMilestones.push(args),
    language:"en", failGatewayReconnect:()=>{scope.reconnectFailed=true;},
    setScreen:value=>{scope.screenRef.current=value;},questCoreRuntimeRef:{current:null},
    identityLinkBusyRef:{current:false},identityLinkIntentRef:{current:0},identityLinkEvents:[],
    setIdentityLinkBusyState:value=>{scope.linkBusy=value;},setIdentityLinkStatus:value=>scope.identityLinkEvents.push(value),
    requestPasskeyIdentityCredential:()=>scope.credentialProvider(),requestWalletIdentityCredential:()=>scope.credentialProvider(),
    linkSuiIdentity:(...args)=>scope.linkProvider(...args),
    guildBuffAuthorityRef:{current:new (loadPure("guildBuff").GuildBuffAuthority)()},
    heroOperationsRef: {current: {pending:null}}, mailCollectBarrierRef: {current:null},
    // Actual empty transport custody; no Bag/Belt surface or readiness exists in this fixture.
    bagBeltMovesRef: {current: new (loadPure("bagBeltMove").BagBeltMoveDispatcher)()},
    bagBeltInventoryReadyRef: {current: null}, bagBeltGeometryRef: {current: null},
    observePreferenceRef:{current:null},observeBootstrapRef:{current:null},combatModeRawRef:{current:null},
    heroWindowEpochsRef:{current:{inventory:1,character:1,belt:1}},skillBarPointerHeldRef:{current:false},skillBarDocumentCacheRef:{current:null},
    ...pureAuth, ...loadPure("stage5"), ...adapter, ...pureSocial, ...pureOperations, ...pureSocialItems, ...pureSocialActions, ...pureExtended,
    projectEquipmentGatewaySnapshot: pureEquipment.projectEquipmentGatewaySnapshot,
    authoritativeItemUniqueId: pureIdentity.authoritativeItemUniqueId,
    currentEquipmentCommandItem: pureEquipment.currentEquipmentCommandItem,
    classifyEquipmentUse: pureEquipment.classifyEquipmentUse,
    equipmentGatewayOperation: pureEquipment.equipmentGatewayOperation,
    mailMutationAllowed: pureParcel.mailMutationAllowed,
    isMailItemMutation: pureParcel.isMailItemMutation,
    readSharedItemCatalogInfo:loadPure("tooltip").readSharedItemCatalogInfo, runtimeRef:{current:null},
    worldRef: { current: world() }, socketRef: { current: socket },
    pendingStorageRequestsRef: pendingRef, storageRequestSequenceRef: sequenceRef,
    storageUiIngressRef: { current: null }, storageServiceActiveRef: { current: false }, storageCompatibilityRef: { current: false },
    setStorageServiceActive: () => {}, setStorageCompatibility: () => {}, setStoragePasswordOpenVersion: () => {}, setStorageServiceOpenVersion: () => {},
    npcBuyDispatcherRef: { current: { withdraw: () => npcWithdrawals.push("withdraw"), status:()=>({flight:null}) } },
    npcRepairAuthorityRef: { current: { invalidateInventory: () => {}, close: () => {} } },
    npcRepairDialogBindingRef: { current: null }, npcRepairServiceRef: { current: null }, setNpcRepairService: () => {},
    npcGoldBuyInventoryRef: { current: { invalidate: () => npcInventoryRetirements.push("invalidate") } },
    npcShopClockRef: { current: { service: 3, catalog: 4 } }, npcBuySelectedRef: { current: 77 },
    npcShopServiceRef: { current: { supportsBuy: true } }, setNpcShopService: () => {},
    equipmentRenderOwnerToken: { ...fixtureOwner }, equipmentHostSuspendReasonRef: { current: null },
    equipmentConnectionGenerationRef: { current: identity.connectionGeneration }, equipmentSessionGenerationRef: { current: identity.sessionGeneration },
    equipmentStartGameRef: { current: { ...identity, afterSnapshotVersion: 0 } }, equipmentBagOwnerRef: liveOwner,
    equipmentSnapshotRef: { current: { ...identity, snapshot: {} } },
    equipmentControllerRef: { current: {
      status: () => ({ ...identity, pending: 0 }),
      hasPendingInstance: () => ({ ok: true, reserved: false }),
      terminateSession: identity => terminations.push(identity),
      invalidateSnapshot: () => {}, observeSnapshot: () => {}, establishBaseline: () => {},
      suspendConnection: () => {}, resumeSameSession: () => {},
      reserve: () => { throw Error("equipment reservation outside this storage fixture"); },
    } },
    currentEquipmentOwner: token => token != null && Object.keys(owner).every(key => token[key] === liveOwner.current[key]),
    rejectEquipmentCommand: reason => { logs.push(reason); return false; },
    combatIngressRef: { current: null }, combatRawRef: { current: null },
    combatMastersRef: { current: new Map() }, combatPointerRef: { current: null },
    spellsIngressRef: { current: null }, spellsRawSnapshotRef: { current: null },
    mailParcelRef: { current: null }, mailIngressRef: { current: null },
    mailDispatcherRef: { current: null }, mailRawRef: { current: null },
    mailOpenEvents: [], mailComposeEvents: [], chatComposeEvents: [],
    setMailboxOpen: value => { scope.mailOpenEvents.push(value); scope.onMailOpen?.(); },
    presentMailCompatibility: (...args) => scope.mailComposeEvents.push(args),
    setChatMessage: value => scope.chatComposeEvents.push(value),
    syncMailParcel: () => { throw Error("unexpected live Mail projection in idle fixture"); },
    sameMailOwner: () => false,
    socialRepliesRef: {current: new pureSocial.SocialIncomingReplies()}, socialReplyWindowsRef: {current: {group: false, bonds: false, guild: false}},
    socialSceneRevisionRef: {current: 1}, renderSocialRequests: () => {},
    bondsWindowLeaseRef: {current: {open: false, epoch: 0}},
    setShowGroupState: value => { scope.groupOpen = value; }, setShowBondsState: value => { scope.bondsOpen = value; },
    setShowGuildState: value => { scope.guildOpen = value; }, setShowRankingState: value => { scope.rankingOpen = value; },
    rankingWindowRef: {current: false}, rankingQueriesRef: {current: new pureSocialActions.RankingQueries()}, renderRankingRequests: () => {},
    guildRanksRef: {current: null}, socialFriendsRef: {current: null},
    socialRosterWindowsRef: {current: {friends: false}}, socialRosterRenderSources: {friend: null, guild: null},
    setShowFriendsState: value => { scope.friendsOpen = value; },
    socialItemOperationsRef: {current: new pureOperations.SocialWindowOperations()}, socialOwnTradeRef: {current: null},
    socialItemWindowsRef: {current: {trade: true, guild: true}}, socialCharacterIndexRef: {current: {requested: 7, current: 7}},
    tradeLifecycleRef: {current: {state: "closed", partner: ""}}, tradePartnerRef: {current: null}, tradeIncarnationRef: {current: 1},
    socialTradeSendingRef: {current: null}, socialTradeReplySpentRef: {current: null}, renderSocialItems: () => {},
    socialTradeUiLease: {owner: null, incarnation: 0, partner: "", state: "closed"},
    guildStorageRawRef: {current: null}, guildPermissionsRef: {current: null}, guildStorageRevisionRef: {current: 0},
    socialCatalogRef: {current: new Map([[6, {name: "Guild item", icon: 1}]])}, worldSnapshotVersionRef: {current: 1},
    storageRentalRef: {current: new pureRental.StorageRentalConfirmation()}, storageServiceRevisionRef: {current: 1},
    bagOpenRef: {current: false}, bagCompatibilityModeRef: {current: null}, setBagCompatibilityMode: () => {},
    setStorageRentalPrompt: value => { scope.rentalPrompt = typeof value === "function" ? value(scope.rentalPrompt) : value; },
    screenRef: {current: "game"}, document: {visibilityState: "visible"},
    lastCommandRef: { current: null }, lastRankingRequestRef: { current: null },
    movementPredictionBlockedUntilRef: { current: 0 }, movementDiagnosticsRef: { current: null },
    MOVEMENT_ACTION_PREDICTION_BLOCK_MS: 0, WebSocket: { OPEN: 1 },
    window: { __mir2CommandHistory: [], dispatchEvent(event) {
      assert.equal(event.type, "mir2:action"); actions++; listener?.(event);
    } },
    CustomEvent: class { constructor(type, options) { this.type = type; this.detail = options.detail; } },
    console: { error: (...args) => errors.push(args) }, Date, Math, JSON,
    isSpectatorBrowserMode: () => false,
    isMovementPredictionBlockingCommand: () => false, isMovementCommand: () => false,
    isMovementConsoleCommand: () => false, isCombatResolutionCommand: () => false,
    recordDebugEvent: () => {}, appendLog: message => logs.push(message),
    t: (key, _args, fallback) => fallback ?? key,
  };
  // These are real source/cancel closures with inactive gesture custody. This
  // storage/social harness has no committed Fishing runtime, proof or physical edge.
  Object.assign(scope, pureQuestMap, {sameWorldFishingOwner: pureFishingSource.sameOwner},
    new Function("WorldFishingSource", fishingRefsJs + "\nreturn {" + fishingRefNames.join(",") + "};")(pureFishingSource.WorldFishingSource));
  scope.setWorldFishingRecord = value => { scope.fishingRecord = typeof value === "function" ? value(scope.fishingRecord ?? null) : value; };
  assert.equal(scope.worldFishingActiveGestureRef.current, null);
  assert.equal(scope.worldFishingQueuedRef.current, null);
  assert.equal(scope.worldFishingCommitRef.current, null);
  scope.updateWorld = updater => { scope.worldRef.current = updater(scope.worldRef.current); };
  const keys = Object.keys(scope);
  const functions = new Function(...keys, pageJavaScript + "\nreturn {"
    + names.join(",") + ",applyAuthSocketClose,applyStorageAck,applySocialEvent,applySocialSnapshot,projectSocialSnapshotFriends,applySocialWindowRender,captureSocialRosterRender,captureSocialTradeUiLease,captureSpouseRender,captureLoginAuthControls};")(...keys.map(key => scope[key]));
  scope.loginAuthRef.current = functions.emptyLoginAuthState(1);
  return { ...functions, scope, sent, errors, logs, terminations, npcWithdrawals, npcInventoryRetirements, socket, liveOwner,
    get attempts() { return attempts; }, get actions() { return actions; },
    onAction(value) { listener = value; }, throwAtSocket(type = null) { socketThrows = true; socketThrowType = type; },
    renderRoster() { Object.assign(scope.socialRosterRenderSources, functions.captureSocialRosterRender(Boolean(scope.friendsOpen), Boolean(scope.guildOpen))); },
    renderTrade() { Object.assign(scope.socialTradeUiLease, functions.captureSocialTradeUiLease()); },
    get pending() { return scope.pendingStorageRequestsRef.current; },
    receive(packet, payload, connection = scope.equipmentConnectionGenerationRef.current, source = scope.socketRef.current) {
      return functions.applySocialEvent({packet, payload}, connection, source);
    },
    snapshot(snapshot = scope.worldRef.current, connection = scope.equipmentConnectionGenerationRef.current, source = scope.socketRef.current) {
      return functions.applySocialSnapshot({...snapshot, playerObjectId: Number(snapshot.playerObjectId)}, connection, source);
    },
    deposit(slot = 4, index = 1) { return functions.storeItem(selection(scope.worldRef.current.inventoryItems[index]), slot); },
    withdraw(slot = 7, container = "bag2") { return functions.takeBackItem(selection(scope.worldRef.current.storageItems[0]), slot, container); },
  };
}
function ack(command, patch = {}, packet = command.type === "storeItemV2" ? "StoreItemV2" : "TakeBackItemV2") {
  return { packet, payload: { requestId: command.requestId, from: command.from, to: command.to, success: true, ...patch } };
}

// Memory observations only. All projection, admission, claims, receive reducers
// and snapshot barriers below are the real extracted Page/pure-module code.
const socialRequests = () => [
  ["GroupInvite", "group", "groupInvite"], ["MarriageRequest", "marriage", "marriageReply"],
  ["DivorceRequest", "divorce", "divorceReply"], ["MentorRequest", "mentor", "mentorReply"],
];
const requests = page => page.scope.socialRepliesRef.current.list(page.currentSocialReceiveOwner());
function openRental(page, renewed = true) {
  page.scope.storageServiceActiveRef.current = true; page.scope.bagOpenRef.current = true;
  page.scope.worldRef.current.hasExpandedStorage = renewed;
  page.rentExpandedStorage();
  const proof = page.scope.storageRentalRef.current.pending;
  assert.ok(proof, "actual Page opening captures current service and owner");
  assert.deepEqual(page.scope.rentalPrompt, {id: proof.id, renewing: renewed});
  return proof;
}
function openTrade(page, offers = false) {
  const trade = {partner: "Partner", settlementNonce: "nonce", offeredGold: 0, accepted: true,
    locked: false, escrowPrepared: false, completed: false,
    offeredSlots: offers ? {3: 43} : {}, offeredUniqueIds: offers ? {3: 22} : {}};
  page.scope.worldRef.current.stage5Systems.trade = trade;
  page.scope.tradeLifecycleRef.current = {state: "open", partner: "Partner"};
  page.snapshot();
  const source = page.readSocialItemSurface("trade"); assert.ok(source);
  return source;
}
const guildRow = (id = 66) => ({item: {unique_id: id, item_index: 6, count: 1}, userId: 7});
function openGuild(page, occupied = true) {
  page.receive("GuildStatus", {guildName: "Guild", guildRankName: "Member", level: 1, experience: 0,
    maxExperience: 100, gold: 0, sparePoints: 0, memberCount: 1, maxMembers: 10, voting: false,
    itemCount: 0, buffCount: 0, myOptions: 24, myRankId: 1, typed: true});
  const items = Array(112).fill(null); if (occupied) items[3] = guildRow();
  page.receive("GuildStorageList", {items, typed: true});
  const source = page.readSocialItemSurface("guild"); assert.ok(source); return source;
}
function openGuildRoster(page) {
  openGuild(page);
  page.receive("GuildStatus", {guildName: "Guild", myOptions: 25, typed: true});
  page.receive("GuildMemberChange", {name: "Self", rankIndex: 4, status: 2, typed: true,
    ranks: [{index: 4, name: "Officer", options: 25, members: [{name: "Self", id: 7, online: true},
      {name: "Member", id: 8, online: false}]}]});
  page.setShowGuild(true); page.renderRoster();
}
const guildChange = (changeType = 2, from = 3, to = 5, user = 7) => ({changeType, from, to, user,
  item: changeType >= 3 || changeType === 1 ? null : guildRow(), typed: true});

test("actual Page receives all four incoming packets and sends only fresh epoch boolean replies once", () => {
  for (const [packet, kind, wire] of socialRequests()) for (const accept of [false, true]) {
    const page = harness();
    assert.equal(page.sendRaw({type: wire, acceptInvite: accept}), false, "raw reply cannot obtain local authority");
    page.receive(packet, {name: "Same name", level: 30, typed: true});
    const old = requests(page)[0]; page.receive(packet, {name: "Same name", level: 31, typed: true});
    const current = requests(page)[0]; assert.ok(current.epoch > old.epoch);
    page.replySocialRequest(kind, old.epoch, accept); assert.equal(page.attempts, 0);
    page.replySocialRequest(kind, current.epoch, accept);
    assert.deepEqual(page.sent, [{type: wire, acceptInvite: accept}]);
    assert.equal(typeof page.sent[0].acceptInvite, "boolean"); assert.equal(requests(page).length, 0);
    page.replySocialRequest(kind, current.epoch, !accept); assert.equal(page.attempts, 1);
  }
});

test("actual Page incoming source guard and final social owner fence reject socket session scene window and suspension changes", () => {
  for (const mode of ["socket", "connection", "session", "scene", "map", "player", "closed", "hidden", "window", "logout"]) {
    const page = harness();
    page.receive("MentorRequest", {name: "Wrong source"}, session.connectionGeneration, {});
    page.receive("MentorRequest", {name: "Wrong connection"}, session.connectionGeneration + 1);
    assert.equal(requests(page).length, 0);
    page.receive("MentorRequest", {name: "Mentor", level: 30}); const request = requests(page)[0];
    page.onAction(() => {
      if (mode === "socket") page.scope.socketRef.current = {readyState: 1};
      if (mode === "connection") page.scope.equipmentConnectionGenerationRef.current++;
      if (mode === "session") page.scope.equipmentSessionGenerationRef.current++;
      if (mode === "scene") page.scope.socialSceneRevisionRef.current++;
      if (mode === "map") page.scope.worldRef.current.mapFileName = "D002";
      if (mode === "player") page.scope.worldRef.current.playerObjectId = "4";
      if (mode === "closed") page.socket.readyState = 3;
      if (mode === "hidden") page.scope.document.visibilityState = "hidden";
      if (mode === "window") page.closeSocialReplyWindow("bonds");
      if (mode === "logout") page.suspendEquipmentConnection("logoutPending");
    });
    page.replySocialRequest("mentor", request.epoch, true); assert.equal(page.attempts, 0, mode);
    assert.equal(page.sent.length, 0, mode);
  }
});

test("actual Page reentrant same-kind incoming replaces the old send while proven-unsent retains only the current request", () => {
  const page = harness(); page.receive("GroupInvite", {name: "Group"}); const old = requests(page)[0];
  page.onAction(() => page.receive("GroupInvite", {name: "Group"}));
  page.replySocialRequest("group", old.epoch, true); assert.equal(page.attempts, 0);
  const next = requests(page)[0]; assert.ok(next.epoch > old.epoch);
  page.onAction(null); page.socket.readyState = 3;
  page.replySocialRequest("group", next.epoch, false); assert.equal(page.attempts, 0);
  page.socket.readyState = 1; assert.strictEqual(requests(page)[0], next);
  page.replySocialRequest("group", old.epoch, true); assert.strictEqual(requests(page)[0], next);
  page.replySocialRequest("group", next.epoch, false); assert.deepEqual(page.sent, [{type: "groupInvite", acceptInvite: false}]);
});

test("actual Page social reply socket exception burns the entered epoch without automatic replay", () => {
  const page = harness(); page.receive("MarriageRequest", {name: "Partner"}); const request = requests(page)[0];
  page.throwAtSocket(); page.replySocialRequest("marriage", request.epoch, true);
  assert.equal(page.attempts, 1); assert.equal(page.errors.length, 1); assert.equal(requests(page).length, 0);
  page.replySocialRequest("marriage", request.epoch, true); assert.equal(page.attempts, 1);
});

test("actual Page rental opening sends no command and explicit new or renewal confirmation claims once", () => {
  for (const renewed of [false, true]) {
    const page = harness(), proof = openRental(page, renewed), before = {...page.scope.worldRef.current};
    assert.equal(page.attempts, 0); assert.equal(page.sendRaw({type: "chat", message: "@ADDSTORAGE"}), false);
    page.confirmStorageRental(proof.id); page.confirmStorageRental(proof.id);
    assert.deepEqual(page.sent, [{type: "chat", message: "@ADDSTORAGE"}]);
    assert.equal(page.scope.storageRentalRef.current.pending, null); assert.equal(page.scope.rentalPrompt, null);
    for (const key of ["gold", "storageSize", "hasExpandedStorage", "expandedStorageExpiryTimeBinaryDatetime"])
      assert.equal(page.scope.worldRef.current[key], before[key], "rental success is server authority: " + key);
  }
});

test("actual Page rental cancellation and service closure cannot cancel or confirm a successor prompt", () => {
  const page = harness(), old = openRental(page);
  page.cancelStorageRental(old.id); assert.equal(page.scope.storageRentalRef.current.pending, null);
  const next = openRental(page); page.cancelStorageRental(old.id); page.confirmStorageRental(old.id);
  assert.strictEqual(page.scope.storageRentalRef.current.pending, next); assert.equal(page.attempts, 0);
  page.endStorageService(); assert.equal(page.scope.storageRentalRef.current.pending, null);
  page.confirmStorageRental(next.id); assert.equal(page.attempts, 0);
});

test("actual Page rental final action fence rechecks owner service size expiry capacity and live balance", () => {
  for (const mode of ["connection", "session", "scene", "map", "player", "service", "size", "expiry", "expanded", "gold", "bag", "close"]) {
    const page = harness(), proof = openRental(page);
    page.onAction(() => {
      if (mode === "connection") page.scope.equipmentConnectionGenerationRef.current++;
      if (mode === "session") page.scope.equipmentSessionGenerationRef.current++;
      if (mode === "scene") page.scope.socialSceneRevisionRef.current++;
      if (mode === "map") page.scope.worldRef.current.mapFileName = "D002";
      if (mode === "player") page.scope.worldRef.current.playerObjectId = "4";
      if (mode === "service") page.scope.storageServiceRevisionRef.current++;
      if (mode === "size") page.scope.worldRef.current.storageSize = 80;
      if (mode === "expiry") page.scope.worldRef.current.expandedStorageExpiryTimeBinaryDatetime++;
      if (mode === "expanded") page.scope.worldRef.current.hasExpandedStorage = false;
      if (mode === "gold") page.scope.worldRef.current.gold = 999_999;
      if (mode === "bag") page.scope.bagOpenRef.current = false;
      if (mode === "close") page.endStorageService();
    });
    page.confirmStorageRental(proof.id); assert.equal(page.attempts, 0, mode);
    if (mode !== "close") assert.strictEqual(page.scope.storageRentalRef.current.pending, proof, "proven-unsent has not claimed " + mode);
  }
  const page = harness(), proof = openRental(page);
  page.onAction(() => { page.scope.worldRef.current.gold = 1_500_000; });
  page.confirmStorageRental(proof.id); assert.equal(page.attempts, 1, "a different sufficient balance remains valid");
});

test("actual Page rental unknown send consumes proof and cannot replay or close a reentrant newer prompt", () => {
  const page = harness(), proof = openRental(page); page.throwAtSocket();
  page.confirmStorageRental(proof.id); assert.equal(page.attempts, 1); assert.equal(page.errors.length, 1);
  assert.equal(page.scope.storageRentalRef.current.pending, null); page.confirmStorageRental(proof.id); assert.equal(page.attempts, 1);
  const replacement = harness(), old = openRental(replacement); let next;
  replacement.onAction(() => { replacement.cancelStorageRental(old.id); next = openRental(replacement); });
  replacement.confirmStorageRental(old.id); assert.equal(replacement.attempts, 0);
  assert.strictEqual(replacement.scope.storageRentalRef.current.pending, next);
  assert.equal(replacement.scope.rentalPrompt.id, next.id);
});

test("actual Page Trade deposit and retrieve use physical Bag2 cells and complete ACK then snapshot barriers", () => {
  for (const offered of [false, true]) {
    const page = harness(), source = openTrade(page, offered);
    const kind = offered ? "retrieveTrade" : "depositTrade", from = offered ? 3 : 43, to = offered ? 47 : 5;
    page.submitSocialItem(kind, from, to, 22, source.sourceKey);
    assert.deepEqual(page.sent, [{type: offered ? "retrieveTradeItem" : "depositTradeItem", from, to}]);
    const operation = page.scope.socialItemOperationsRef.current.pending; assert.equal(operation.entered, true);
    page.snapshot(); assert.strictEqual(page.scope.socialItemOperationsRef.current.pending.proof, operation.proof, "snapshot before ACK cannot release");
    page.receive(offered ? "RetrieveTradeItem" : "DepositTradeItem", {from, to, success: true});
    assert.equal(page.scope.socialItemOperationsRef.current.pending.acked, true); assert.equal(page.scope.socialItemOperationsRef.current.pending !== null, true);
    page.snapshot(); assert.equal(page.scope.socialItemOperationsRef.current.pending, null);
  }
});

test("actual Page social item source and final event fence reject replacements permissions windows and concurrent item owners", () => {
  for (const mode of ["source", "window", "connection", "scene", "socket", "equipment", "storage", "mail", "rental", "locked"]) {
    const page = harness(), source = openTrade(page);
    page.submitSocialItem("depositTrade", 43, 5, 22, "stale source"); assert.equal(page.attempts, 0);
    page.onAction(() => {
      if (mode === "source") page.scope.worldRef.current.inventoryItems[1] = item(99, "bag2", 3);
      if (mode === "window") page.scope.socialItemWindowsRef.current.trade = false;
      if (mode === "connection") page.scope.equipmentConnectionGenerationRef.current++;
      if (mode === "scene") page.scope.socialSceneRevisionRef.current++;
      if (mode === "socket") page.scope.socketRef.current = {readyState: 1};
      if (mode === "equipment") page.scope.equipmentControllerRef.current.status = () => ({pending: 1});
      if (mode === "storage") {
        const proof = {type: "storeItemV2", requestId: requestId(9), from: 7, to: 8,
          source: {uniqueId: 33}, target: {uniqueId: null}};
        page.pending.set(proof.requestId, {proof, enteredSocket: true});
      }
      if (mode === "mail") page.scope.mailParcelRef.current = {state: {blockedUniqueIds: [33]}};
      if (mode === "rental") openRental(page);
      if (mode === "locked") page.scope.socialOwnTradeRef.current.trade.locked = true;
    });
    page.submitSocialItem("depositTrade", 43, 5, 22, source.sourceKey);
    assert.equal(page.attempts, 0, mode); assert.equal(page.scope.socialItemOperationsRef.current.pending, null, mode);
  }
});

test("actual Page social item unknown outcome remains pending across closed UI and unrelated snapshots", () => {
  const page = harness(), source = openTrade(page); page.throwAtSocket();
  page.submitSocialItem("depositTrade", 43, 5, 22, source.sourceKey);
  const pending = page.scope.socialItemOperationsRef.current.pending; assert.ok(pending?.entered);
  assert.equal(page.attempts, 1); page.scope.socialItemWindowsRef.current.trade = false;
  page.snapshot(); assert.strictEqual(page.scope.socialItemOperationsRef.current.pending.proof, pending.proof);
  page.submitSocialItem("depositTrade", 43, 5, 22, source.sourceKey); assert.equal(page.attempts, 1);
  page.retireSocialItemConnection(); assert.equal(page.scope.socialItemOperationsRef.current.pending, null);
});

test("actual Page Guild typed change correlates user tuple UID and source before releasing a full snapshot barrier", () => {
  const page = harness(), source = openGuild(page);
  page.submitSocialItem("guild2", 3, 5, 66, source.sourceKey);
  assert.deepEqual(page.sent, [{type: "guildStorageItemChange", changeType: 2, from: 3, to: 5}]);
  const pending = page.scope.socialItemOperationsRef.current.pending, bank = page.scope.guildStorageRawRef.current;
  assert.equal(bank.items[3].item.unique_id, 66, "send does not optimistically move the bank");
  for (const [packet, connection, socket] of [[guildChange(), 4, page.socket], [guildChange(), 3, {}]]) {
    page.receive("GuildStorageItemChange", packet, connection, socket);
    assert.equal(page.scope.socialItemOperationsRef.current.pending.acked, false); assert.strictEqual(page.scope.guildStorageRawRef.current, bank);
  }
  page.receive("GuildStorageItemChange", guildChange()); assert.equal(page.scope.socialItemOperationsRef.current.pending.acked, true);
  assert.equal(page.scope.guildStorageRawRef.current.items[3], null);
  assert.equal(page.scope.guildStorageRawRef.current.items[5].item.unique_id, 66);
  page.snapshot(); assert.equal(page.scope.socialItemOperationsRef.current.pending, null);
  page.submitSocialItem("guild2", 3, 7, 66, source.sourceKey); assert.equal(page.attempts, 1, "old slot/source cannot grant UID authority");
});

test("actual Page Guild failures 3 to 5 preserve raw slots and other-user broadcasts never acknowledge own operation", () => {
  for (const requested of [0, 1, 2]) {
    const page = harness(), source = openGuild(page), from = requested === 0 ? 43 : 3, to = requested === 1 ? 47 : 5;
    page.submitSocialItem("guild" + requested, from, to, requested === 0 ? 22 : 66, source.sourceKey);
    const pending = page.scope.socialItemOperationsRef.current.pending; assert.ok(pending?.entered);
    const before = JSON.stringify(page.scope.guildStorageRawRef.current.items);
    page.receive("GuildStorageItemChange", guildChange(requested + 3, from, to, 0));
    assert.equal(page.scope.socialItemOperationsRef.current.pending.acked, true); assert.equal(page.scope.socialItemOperationsRef.current.pending.success, false);
    assert.equal(JSON.stringify(page.scope.guildStorageRawRef.current.items), before, "failure has no optimistic slot effect");
    page.snapshot(); assert.equal(page.scope.socialItemOperationsRef.current.pending, null);
  }
  const page = harness(), source = openGuild(page); page.submitSocialItem("guild2", 3, 5, 66, source.sourceKey);
  const pending = page.scope.socialItemOperationsRef.current.pending;
  page.receive("GuildStorageItemChange", guildChange(2, 3, 5, 8)); assert.equal(page.scope.socialItemOperationsRef.current.pending.acked, false);
  assert.equal(page.scope.guildStorageRawRef.current.items[5].item.unique_id, 66, "shared broadcast updates bank only");
  page.snapshot(); assert.strictEqual(page.scope.socialItemOperationsRef.current.pending.proof, pending.proof);
});

test("actual Page logoutPending accepts authenticated Guild updates and post-ACK full snapshot but grants no new send", () => {
  const page = harness(), source = openGuild(page); page.submitSocialItem("guild2", 3, 5, 66, source.sourceKey);
  const pending = page.scope.socialItemOperationsRef.current.pending;
  page.suspendEquipmentConnection("logoutPending"); assert.equal(page.readSocialItemSurface("guild"), null);
  const before = page.attempts; page.submitSocialItem("guild2", 3, 7, 66, source.sourceKey); assert.equal(page.attempts, before);
  page.receive("MentorRequest", {name: "Received while paused", level: 30, typed: true}); assert.equal(requests(page).length, 1);
  page.receive("GuildStorageItemChange", guildChange()); assert.equal(page.scope.socialItemOperationsRef.current.pending.acked, true);
  assert.equal(page.scope.guildStorageRawRef.current.items[3], null); assert.equal(page.scope.guildStorageRawRef.current.items[5].item.unique_id, 66);
  page.snapshot(); assert.equal(page.scope.socialItemOperationsRef.current.pending, null);
  page.receive("LogOutFailed", {}); assert.equal(page.scope.equipmentHostSuspendReasonRef.current, null);
  const fresh = page.readSocialItemSurface("guild"); assert.ok(fresh); assert.equal(fresh.slots[3], null);
  page.submitSocialItem("guild2", 3, 7, 66, source.sourceKey); page.submitSocialItem("guild2", 3, 7, 66, fresh.sourceKey);
  assert.equal(page.attempts, before, "restoring the session cannot restore the retired slot3 selection");
  page.submitSocialItem("guild2", 5, 7, 66, fresh.sourceKey); assert.equal(page.attempts, before + 1, "fresh slot5 selection remains usable");
});

test("actual Page partial stale session and foreign socket snapshots cannot clear a social ACK barrier", () => {
  const page = harness(), source = openGuild(page); page.submitSocialItem("guild2", 3, 5, 66, source.sourceKey);
  const pending = page.scope.socialItemOperationsRef.current.pending; page.receive("GuildStorageItemChange", guildChange());
  const revision = page.scope.worldSnapshotVersionRef.current;
  page.snapshot(page.scope.worldRef.current, 4); page.snapshot(page.scope.worldRef.current, 3, {});
  assert.equal(page.scope.worldSnapshotVersionRef.current, revision); assert.strictEqual(page.scope.socialItemOperationsRef.current.pending.proof, pending.proof);
  page.snapshot({...page.scope.worldRef.current, inventoryItems: undefined});
  assert.equal(page.scope.equipmentSnapshotRef.current, null); assert.strictEqual(page.scope.socialItemOperationsRef.current.pending.proof, pending.proof);
  page.scope.equipmentSessionGenerationRef.current++;
  page.snapshot(); assert.strictEqual(page.scope.socialItemOperationsRef.current.pending.proof, pending.proof);
  page.scope.equipmentSessionGenerationRef.current--; page.snapshot(); assert.equal(page.scope.socialItemOperationsRef.current.pending, null);
});

test("actual Page GuildInvite opens the real Guild owner across render and replies with the captured epoch once", () => {
  for (const acceptInvite of [false, true]) {
    const page = harness(); page.receive("GuildInvite", {name: "Incoming Guild", typed: true});
    assert.equal(page.scope.guildOpen, true); assert.equal(page.socialReplyWindowOpen("guild"), true);
    page.applySocialWindowRender(Boolean(page.scope.groupOpen), Boolean(page.scope.bondsOpen), Boolean(page.scope.guildOpen));
    assert.equal(page.socialReplyWindowOpen("guild"), true, "actual render cannot erase the Guild setter's owner");
    const old = requests(page).find(row => row.kind === "guild");
    page.receive("GuildInvite", {name: "Incoming Guild", typed: true}); const current = requests(page).find(row => row.kind === "guild");
    assert.ok(current.epoch > old.epoch); page.replySocialRequest("guild", old.epoch, acceptInvite); assert.equal(page.attempts, 0);
    page.replySocialRequest("guild", current.epoch, acceptInvite); page.replySocialRequest("guild", current.epoch, !acceptInvite);
    assert.deepEqual(page.sent, [{type: "guildInvite", acceptInvite}]); assert.equal(page.attempts, 1);
  }
});

test("actual Page Guild reentrant invite and actual close setter retire only their own captured epoch", () => {
  const page = harness(); page.receive("MarriageRequest", {name: "Partner"}); page.receive("GuildInvite", {name: "Guild"});
  const old = requests(page).find(row => row.kind === "guild"), marriage = requests(page).find(row => row.kind === "marriage");
  page.onAction(() => page.receive("GuildInvite", {name: "Guild"})); page.replySocialRequest("guild", old.epoch, true);
  assert.equal(page.attempts, 0); const next = requests(page).find(row => row.kind === "guild"); assert.ok(next.epoch > old.epoch);
  assert.strictEqual(requests(page).find(row => row.kind === "marriage"), marriage);
  page.onAction(() => page.setShowGuild(false)); page.replySocialRequest("guild", next.epoch, false); assert.equal(page.attempts, 0);
  assert.equal(page.scope.guildOpen, false); assert.equal(page.socialReplyWindowOpen("guild"), false);
  assert.equal(requests(page).some(row => row.kind === "guild"), false);
  assert.strictEqual(requests(page).find(row => row.kind === "marriage"), marriage);
});

test("actual Page FriendUpdate uses friendRecords identity for remove and memo without guessing display list positions", () => {
  const page = harness();
  page.receive("FriendUpdate", {friends: ["Zero", "Pat", "Legacy"], blocked: ["Blocked"], typed: true,
    friendRecords: [{name: "Zero", index: 0, memo: "zero", online: false, blocked: false},
      {name: "Pat", index: 42, memo: "memo", online: true, blocked: false},
      {name: "Blocked", index: 7, memo: "blocked", online: false, blocked: true}, {name: "Legacy", online: false}]});
  assert.equal(page.friendCharacterIndex("pAT"), 42); assert.equal(page.friendCharacterIndex("Zero"), 0);
  assert.equal(page.friendCharacterIndex("Legacy"), null);
  page.setShowFriends(true); page.renderRoster();
  page.removeFriendEntry("Zero"); page.editFriendMemo("Pat", "new memo"); page.removeFriendEntry("Legacy");
  assert.deepEqual(page.sent, [{type: "removeFriend", characterIndex: 0}, {type: "addMemo", characterIndex: 42, memo: "new memo"}]);
  assert.deepEqual(page.scope.worldRef.current.stage5Systems.social.friends, ["Zero", "Pat", "Legacy"], "sends never optimistically remove the roster");
  assert.equal(page.scope.socialFriendsRef.current.entries.find(row => row.name === "Pat").memo, "memo", "memo success remains server authority");
  page.receive("FriendUpdate", {friends: [{name: "Pat", index: 42}], blocked: [], typed: true});
  assert.equal(page.friendCharacterIndex("Pat"), null, "only actual friendRecords can authorize mutations");
  page.removeFriendEntry("Pat"); page.editFriendMemo("Pat", "blocked"); assert.equal(page.attempts, 2);
});

test("actual Page FriendUpdate rejects stale source and ambiguous target while complete snapshots preserve newer received identities", () => {
  const page = harness(), payload = {friendRecords: [{name: "Pat", index: 42}, {name: "Other", index: 7},
    {name: "oTHER", index: 8}, {name: "Bad", index: -1}], typed: true};
  page.receive("FriendUpdate", payload, 4); page.receive("FriendUpdate", payload, 3, {});
  assert.equal(page.scope.socialFriendsRef.current, null);
  page.receive("FriendUpdate", payload); assert.equal(page.friendCharacterIndex("Pat"), 42);
  assert.equal(page.friendCharacterIndex("Other"), null); assert.equal(page.friendCharacterIndex("Bad"), null);
  const snapshot = {...page.scope.worldRef.current, stage5Systems: {social: {friends: [], blocked: [], friendInfos: []}}};
  const preserved = page.projectSocialSnapshotFriends(snapshot, page.scope.worldRef.current, page.currentSocialReceiveOwner());
  assert.equal(preserved.friendInfos.find(row => row.name === "Pat").index, 42);
  assert.deepEqual(preserved.friends, ["Pat", "Other", "oTHER", "Bad"]);
  assert.strictEqual(page.projectSocialSnapshotFriends(snapshot, page.scope.worldRef.current,
    {...page.currentSocialReceiveOwner(), sessionGeneration: 6}), snapshot.stage5Systems.social);
  page.scope.equipmentSessionGenerationRef.current++; assert.equal(page.friendCharacterIndex("Pat"), null);
  page.removeFriendEntry("Pat"); assert.equal(page.attempts, 0);
});

test("actual Page GuildMemberChange projects actual ranks and updates permissions only for the current self name and character ID", () => {
  const page = harness(); openGuild(page);
  const payload = {name: "Self", rankIndex: 4, status: 2, typed: true, ranks: [
    {index: 4, name: "Store only", options: 8, members: [{name: "Self", id: 7, online: true, hasVoted: false, lastLoginBinaryDatetime: "0"}]},
    {index: 9, name: "Leader", options: 255, members: [{name: "Other", id: 8, online: false, hasVoted: false, lastLoginBinaryDatetime: "0"}]},
  ]};
  page.receive("GuildMemberChange", payload, 4); assert.equal(page.scope.guildRanksRef.current, null);
  page.receive("GuildMemberChange", payload); assert.equal(page.scope.guildRanksRef.current.value.ranks[0].index, 4);
  assert.deepEqual(page.scope.guildRanksRef.current.value.members[0], {name: "Self", id: 7, rank: "Store only", rankIndex: 4, online: true});
  assert.deepEqual(page.scope.guildPermissionsRef.current.permissions, ["CanStoreItem"]);
  assert.deepEqual(page.scope.worldRef.current.stage5Systems.guild.members, ["Self", "Other"]);
  const source = page.readSocialItemSurface("guild"); page.submitSocialItem("guild1", 3, 47, 66, source.sourceKey);
  assert.equal(page.attempts, 0, "a rank without retrieve permission cannot authorize the existing item");
  page.receive("GuildMemberChange", {...payload, ranks: [{index: 4, name: "Bad", options: 256, members: []}]});
  assert.equal(page.scope.guildRanksRef.current, null, "malformed rank options cannot become a projected rank");
});

test("actual Page Guild rank assignment saves type2 separately from type3 rename and exact eight type5 options", () => {
  const page = harness(); openGuildRoster(page); page.changeGuildMemberRank("  Member  ", 4);
  assert.deepEqual(page.sent, [{type: "editGuildMember", changeType: 2, rankIndex: 4, name: "Member", rankName: "Officer"}]);
  page.saveGuildRank(4, "Officer", ["CanStoreItem"]);
  assert.deepEqual(page.sent.slice(1), pureSocialActions.guildRankSaveCommands(4, "Officer", ["CanStoreItem"]));
  for (const index of [-1, 1.5, 2147483648]) page.changeGuildMemberRank("Member", index);
  page.saveGuildRank(4, "Officer", ["canStoreItem"]); assert.equal(page.attempts, 10);
  page.changeGuildMemberRank("Unknown member", 4); page.changeGuildMemberRank("Member", 99);
  assert.equal(page.attempts, 10, "actual rank and member rows are required before type2 can enter");
  page.setShowGuild(false); page.renderRoster();
  assert.deepEqual(page.scope.socialRosterRenderSources, {friend: null, guild: null}, "actual closed render captures no roster projection");
  page.saveGuildRank(4, "Officer", ["CanStoreItem"]); assert.equal(page.attempts, 10);
});

const rankingPacket = (rankType = 3, count = 100) => ({rankType, count, myRank: 8, listings: [22, 33],
  listingDetails: [{playerId: 22, name: "First", level: 30, class: "Taoist"}, {name: "Second", level: 29, class: "Wizard"}]});

test("actual Page friend final roster proof rejects same-socket logout scene replacement and closed window before immutable DTO entry", () => {
  for (const command of [{type: "removeFriend", characterIndex: 42}, {type: "addMemo", characterIndex: 42, memo: "memo"},
    {type: "refreshFriends"}, {type: "editGuildMember", changeType: 2, rankIndex: 4, name: "Member", rankName: ""}])
    assert.equal(harness().sendRaw(command), false, "ordinary roster wire cannot bypass the actual proof gate");
  for (const mode of ["logout", "scene", "map", "roster", "window", "session", "socket", "wire"]) {
    const page = harness(); page.receive("FriendUpdate", {friendRecords: [{name: "Pat", index: 42}], typed: true});
    page.setShowFriends(true); page.renderRoster();
    page.onAction(() => {
      if (mode === "logout") page.suspendEquipmentConnection("logoutPending");
      if (mode === "scene") page.scope.socialSceneRevisionRef.current++;
      if (mode === "map") page.scope.worldRef.current.mapFileName = "D002";
      if (mode === "roster") page.receive("FriendUpdate", {friendRecords: [{name: "Pat", index: 99}], typed: true});
      if (mode === "window") page.setShowFriends(false);
      if (mode === "session") page.scope.equipmentSessionGenerationRef.current++;
      if (mode === "socket") page.scope.socketRef.current = {readyState: 1};
      if (mode === "wire") page.scope.lastCommandRef.current.characterIndex = 99;
    });
    page.editFriendMemo("Pat", "memo"); assert.equal(page.attempts, 0, mode);
    assert.equal(page.sent.length, 0, mode);
  }
  const page = harness(); page.receive("FriendUpdate", {friendRecords: [{name: "Pat", index: 42}]});
  page.setShowFriends(true); page.renderRoster();
  page.receive("FriendUpdate", {friendRecords: [{name: "Pat", index: 99}]});
  page.removeFriendEntry("Pat"); assert.equal(page.attempts, 0, "old render cannot use a newer index under its old source");
  page.renderRoster(); page.removeFriendEntry("Pat"); assert.deepEqual(page.sent, [{type: "removeFriend", characterIndex: 99}]);
});

test("actual Page Guild roster final proof rejects synchronous owner guild rank permission and window replacement", () => {
  for (const mode of ["logout", "scene", "guild", "rank", "permission", "window", "session", "socket"]) {
    const page = harness(); openGuildRoster(page);
    page.onAction(() => {
      if (mode === "logout") page.suspendEquipmentConnection("logoutPending");
      if (mode === "scene") page.scope.socialSceneRevisionRef.current++;
      if (mode === "guild") page.scope.worldRef.current.stage5Systems.guild.name = "Another";
      if (mode === "rank") page.receive("GuildMemberChange", {name: "Self", status: 2, rankIndex: 4, typed: true,
        ranks: [{index: 4, name: "Changed", options: 25, members: [{name: "Self", id: 7, online: true}]}]});
      if (mode === "permission") page.receive("GuildStatus", {guildName: "Guild", myOptions: 8});
      if (mode === "window") page.setShowGuild(false);
      if (mode === "session") page.scope.equipmentSessionGenerationRef.current++;
      if (mode === "socket") page.scope.socketRef.current = {readyState: 1};
    });
    page.changeGuildMemberRank("Member", 4); assert.equal(page.attempts, 0, mode);
  }
});

test("actual Page quiet nine-command Guild rank save rechecks the actual captured source at each socket boundary and stops on replacement", () => {
  for (const stoppedAt of [1, 4, 9]) {
    const page = harness(); openGuildRoster(page); let actions = 0;
    page.onAction(() => { if (++actions === stoppedAt) page.receive("GuildStatus", {guildName: "Guild", myOptions: 8}); });
    page.saveGuildRank(4, "Officer", ["CanStoreItem"]);
    const expected = pureSocialActions.guildRankSaveCommands(4, "Officer", ["CanStoreItem"]);
    assert.deepEqual(page.sent, expected.slice(0, stoppedAt - 1)); assert.equal(page.attempts, stoppedAt - 1);
    assert.equal(actions, stoppedAt, "failed final proof stops the next option immediately");
    assert.equal(page.logs.includes("log.sent"), false, "every actual command preserves quiet sending");
  }
});

test("actual Page partner TradeItems parser preserves holes raw safe UID and complete readonly tooltip metadata", () => {
  const page = harness(), raw = Array(10).fill(null);
  raw[3] = {unique_id: 0, item_index: 77, count: 2, current_dura: 9, max_dura: 10, gem_count: 1, identified: true};
  raw[9] = {unique_id: 9001, item_index: 78, count: 1};
  const before = JSON.stringify(raw), items = page.projectPartnerTradePacket(raw);
  assert.equal(items.length, 10); assert.equal(items[0], null); assert.equal(items[8], null);
  assert.equal(items[3].id, 0); assert.equal(items[3].uniqueId, 0); assert.equal(items[3].slot, 3); assert.equal(items[3].count, 2);
  assert.equal(items[9].uniqueId, 9001); assert.equal(items[9].slot, 9);
  assert.deepEqual(items[3].tooltipSource.item, raw[3]);
  assert.equal(Object.isFrozen(items[3].tooltipSource), true); assert.equal(Object.isFrozen(items[3].tooltipSource.item), true);
  assert.equal(items[3].tooltipSource.info, undefined, "missing catalog cannot invent template authority");
  assert.equal(JSON.stringify(raw), before);
  assert.equal(items[3].name, "Unknown item");
});

test("actual Page partner TradeItems aggregate rejects unsafe duplicate and malformed UID rows as an unknown whole packet", () => {
  const page = harness(), good = {unique_id: 9001, item_index: 77, count: 1};
  for (const bad of [{...good, unique_id: Number.MAX_SAFE_INTEGER + 1}, {...good, unique_id: -1},
    {...good, unique_id: "9001"}, {item_index: 77, count: 1}, {...good, item_index: -1},
    {...good, item_index: 2147483648}, {...good, count: 0}, {...good, count: 65536}, [], "row"])
    assert.equal(page.projectPartnerTradePacket([good, null, bad]), undefined);
  assert.equal(page.projectPartnerTradePacket([good, null, {...good}]), undefined, "duplicate instance UID invalidates all slots");
  assert.equal(page.projectPartnerTradePacket(Array(11).fill(null)), undefined);
  assert.equal(page.projectPartnerTradePacket(null), undefined);
  assert.deepEqual(page.projectPartnerTradePacket([]), Array(10).fill(null), "measured empty packet has ten known empty cells");
});

test("actual Page late NewItemInfo reprojects cached partner raw items and bad TradeItem forbids confirmation while allowing explicit unlock", () => {
  const page = harness(); openTrade(page); page.renderTrade();
  page.scope.tradePartnerRef.current = {owner: page.currentSocialReplyOwner(), incarnation: page.scope.tradeIncarnationRef.current};
  const raw = Array(10).fill(null); raw[3] = {unique_id: 9001, item_index: 77, count: 2, current_dura: 9, identified: true};
  page.receive("TradeItem", {tradeItems: raw}); const first = page.scope.tradePartnerRef.current.items;
  assert.equal(page.scope.tradePartnerRef.current.itemsKnown, true); assert.equal(first[3].name, "Unknown item");
  const info = {index: 77, name: "Known item", image: 12, item_type: 3, shape: 7};
  page.receive("NewItemInfo", {info}); const current = page.scope.tradePartnerRef.current.items;
  assert.notStrictEqual(current, first); assert.equal(current[3].name, "Known item"); assert.equal(current[3].icon, 12);
  assert.equal(current[3].uniqueId, 9001); assert.equal(current[3].slot, 3); assert.equal(current[4], null);
  assert.deepEqual(current[3].tooltipSource.info, info); assert.equal(Object.isFrozen(current[3].tooltipSource.info), true);
  assert.deepEqual(current[3].tooltipSource.item, raw[3]);
  page.confirmTrade(); assert.deepEqual(page.sent, [{type: "tradeConfirm", locked: true}]);
  page.receive("TradeItem", {tradeItems: [{unique_id: Number.MAX_SAFE_INTEGER + 1, item_index: 77, count: 1}]});
  assert.equal(page.scope.tradePartnerRef.current.itemsKnown, false); assert.equal(page.scope.tradePartnerRef.current.items, undefined);
  page.confirmTrade(); assert.equal(page.attempts, 1, "unknown partner aggregate cannot authorize locked true");
  page.scope.worldRef.current.stage5Systems.trade.locked = true; page.snapshot(); page.renderTrade();
  page.confirmTrade(); assert.deepEqual(page.sent[1], {type: "tradeConfirm", locked: false});
  assert.equal(page.sendTradeUiCommand({type: "tradeConfirm", locked: true}), false, "current locked state requires the opposite boolean");
  assert.equal(page.attempts, 2);
});

test("actual Page Ranking queues selected online filter and settles old rows under their own type and offset before issuing the desired query", () => {
  const page = harness(); assert.equal(page.sendRaw({type: "getRanking", rankType: 3, rankIndex: 20, onlineOnly: false}), false);
  page.issueRankingRequest({rankType: 3, rankIndex: 20, onlineOnly: false}); const old = page.scope.rankingQueriesRef.current.pending;
  page.setRankingOnlineOnly(true); assert.strictEqual(page.scope.rankingQueriesRef.current.pending, old); assert.equal(page.attempts, 1);
  page.receive("Rankings", rankingPacket());
  const previous = page.scope.worldRef.current.rankings["3:all"];
  assert.equal(previous.rankIndex, 20); assert.equal(previous.onlineOnly, false); assert.deepEqual(previous.entries.map(row => row.rank), [21, 22]);
  assert.equal(previous.entries[0].classKey, "taoist"); assert.equal(previous.entries[1].playerId, 33);
  assert.deepEqual(page.sent, [{type: "getRanking", rankType: 3, rankIndex: 20, onlineOnly: false},
    {type: "getRanking", rankType: 3, rankIndex: 0, onlineOnly: true}]);
  const next = page.scope.rankingQueriesRef.current.pending; assert.notStrictEqual(next, old);
  page.receive("Rankings", rankingPacket()); assert.equal(page.scope.rankingQueriesRef.current.pending, null);
  assert.equal(page.scope.worldRef.current.rankings["3:online"].rankIndex, 0);
  assert.equal(page.scope.worldRef.current.rankings["3:online"].onlineOnly, true);
});

test("actual Page Ranking source session malformed response and final scene fences reject foreign authority", () => {
  const page = harness(); page.issueRankingRequest({rankType: 3, rankIndex: 20, onlineOnly: false});
  const proof = page.scope.rankingQueriesRef.current.pending;
  page.receive("Rankings", rankingPacket(), 4); page.receive("Rankings", rankingPacket(), 3, {});
  page.receive("Rankings", rankingPacket(2)); page.receive("Rankings", {...rankingPacket(), count: -1});
  page.receive("Rankings", {...rankingPacket(), listingDetails: Array(21).fill({name: "Bad"})});
  page.scope.equipmentSessionGenerationRef.current++; page.receive("Rankings", rankingPacket());
  assert.strictEqual(page.scope.rankingQueriesRef.current.pending, proof); assert.deepEqual(page.scope.worldRef.current.rankings, {});
  page.scope.equipmentSessionGenerationRef.current--; page.scope.socialSceneRevisionRef.current++; page.scope.worldRef.current.mapFileName = "D002";
  page.receive("Rankings", rankingPacket()); assert.equal(page.scope.rankingQueriesRef.current.pending, null, "same physical owner can receive after a scene change");
  for (const mode of ["scene", "session", "socket", "wire"]) {
    const fenced = harness(); fenced.onAction(() => {
      if (mode === "scene") fenced.scope.socialSceneRevisionRef.current++;
      if (mode === "session") fenced.scope.equipmentSessionGenerationRef.current++;
      if (mode === "socket") fenced.scope.socketRef.current = {readyState: 1};
      if (mode === "wire") fenced.scope.lastCommandRef.current.rankIndex = 40;
    });
    fenced.issueRankingRequest({rankType: 3, rankIndex: 20, onlineOnly: true});
    assert.equal(fenced.attempts, 0, mode); assert.equal(fenced.scope.rankingQueriesRef.current.pending, null, "only proven-unsent request is released: " + mode);
  }
});

test("actual Page Ranking unknown socket entry is retained without retry and actual connection retirement clears it", () => {
  const page = harness(); page.throwAtSocket(); page.issueRankingRequest({rankType: 3, rankIndex: 20, onlineOnly: false});
  const proof = page.scope.rankingQueriesRef.current.pending; assert.ok(proof); assert.equal(page.attempts, 1); assert.equal(page.errors.length, 1);
  page.refreshRanking(); page.requestRanking("online"); assert.equal(page.attempts, 1); assert.strictEqual(page.scope.rankingQueriesRef.current.pending, proof);
  page.receive("Rankings", rankingPacket(2)); assert.strictEqual(page.scope.rankingQueriesRef.current.pending, proof);
  page.retireSocialItemConnection(); assert.equal(page.scope.rankingQueriesRef.current.pending, null);
  assert.equal(page.scope.guildRanksRef.current, null); assert.equal(page.scope.socialFriendsRef.current, null);
  page.receive("Rankings", rankingPacket()); assert.deepEqual(page.scope.worldRef.current.rankings, {}); assert.equal(page.attempts, 1);
});

test("actual Page Ranking window opening and row movement retain online class with a single outstanding bounded offset", () => {
  const page = harness(); page.setShowRanking(true); assert.equal(page.attempts, 1);
  page.setShowRanking(true); assert.equal(page.attempts, 1, "already-open window cannot request twice");
  page.receive("Rankings", rankingPacket(0)); page.issueRankingRequest({rankType: 3, rankIndex: 20, onlineOnly: true});
  page.receive("Rankings", rankingPacket()); page.moveRankingRows(1);
  assert.deepEqual(page.sent.at(-1), {type: "getRanking", rankType: 3, rankIndex: 21, onlineOnly: true});
  const attempts = page.attempts; page.moveRankingRows(1); assert.equal(page.attempts, attempts, "pending rows cannot emit another wire");
  page.receive("Rankings", rankingPacket()); page.moveRankingRows(-1);
  assert.deepEqual(page.sent.at(-1), {type: "getRanking", rankType: 3, rankIndex: 20, onlineOnly: true});
});

test("actual Page store and take-back serialize Bag2 physical cells with one reserved proof", () => {
  const deposit = harness();
  assert.equal(deposit.deposit(), true);
  assert.deepEqual(deposit.sent, [{ type: "storeItemV2", requestId: requestId(1), from: 43, to: 4 }]);
  assert.equal(deposit.pending.get(requestId(1)).enteredSocket, true);
  assert.equal(Object.keys(deposit.sent[0]).length, 4, "UID proof stays local");
  const withdraw = harness();
  assert.equal(withdraw.withdraw(), true);
  assert.deepEqual(withdraw.sent, [{ type: "takeBackItemV2", requestId: requestId(1), from: 83, to: 47 }]);
  assert.equal(withdraw.withdraw(8), false, "48 bag slots include only eight Bag2 cells");
});

test("actual Page refuses stale display-only duplicate and revoked selections before emitting action", () => {
  for (const mode of ["replacement", "display", "duplicate", "capacity", "password"]) {
    const page = harness(), w = page.scope.worldRef.current, selected = selection(w.inventoryItems[1]);
    if (mode === "replacement") w.inventoryItems[1] = item(99, "bag2", 3);
    if (mode === "display") delete selected.authoritativeUniqueId;
    if (mode === "duplicate") w.storageItems.push(item(22, "storage", 9));
    if (mode === "capacity") { w.inventoryCapacity = 46; w.maxBagSlots = 40; }
    if (mode === "password") { w.hasStoragePassword = true; w.storageSessionUnlocked = false; }
    assert.equal(page.storeItem(selected, 4), false, mode);
    assert.equal(page.actions, 0, mode); assert.equal(page.attempts, 0, mode);
    assert.equal(page.pending.size, 0); assert.equal(page.scope.storageRequestSequenceRef.current, 1);
  }
  const page = harness(); page.scope.worldRef.current.hasExpandedStorage = false;
  assert.equal(page.withdraw(), false, "storage page two was revoked");
  assert.equal(page.attempts, 0);
});

test("actual Page final event fence rejects replacement target capacity password owner and socket changes", () => {
  for (const mode of ["source", "target", "capacity", "password", "owner", "connection", "session", "socket", "closed", "pending-equipment"]) {
    const page = harness();
    page.onAction(() => {
      const w = page.scope.worldRef.current;
      if (mode === "source") w.inventoryItems[1] = item(99, "bag2", 3);
      if (mode === "target") w.storageItems.push(item(55, "storage", 4));
      if (mode === "capacity") { w.inventoryCapacity = 46; w.maxBagSlots = 40; }
      if (mode === "password") { w.hasStoragePassword = true; w.storageSessionUnlocked = false; }
      if (mode === "owner") page.liveOwner.current.ownerRevision++;
      if (mode === "connection") { page.liveOwner.current.connectionGeneration++; page.scope.equipmentConnectionGenerationRef.current++; }
      if (mode === "session") { page.liveOwner.current.sessionGeneration++; page.scope.equipmentSessionGenerationRef.current++; }
      if (mode === "socket") page.scope.socketRef.current = { readyState: 1, send() { throw Error("wrong socket"); } };
      if (mode === "closed") page.socket.readyState = 3;
      if (mode === "pending-equipment") page.scope.equipmentControllerRef.current.status = () => ({ pending: 1 });
    });
    assert.equal(page.deposit(), false, mode);
    assert.equal(page.attempts, 0, mode); assert.equal(page.pending.size, 0, mode);
    assert.equal(page.scope.storageRequestSequenceRef.current, 2, "proven-unsent request ID stays spent");
  }
  const page = harness();
  page.onAction(() => { page.scope.worldRef.current.hasExpandedStorage = false; });
  assert.equal(page.withdraw(), false, "withdraw rechecks source storage page");
  assert.equal(page.attempts, 0); assert.equal(page.pending.size, 0);
});

test("actual immutable storage wire rejects action-time mutation of the caller DTO", () => {
  const page = harness();
  page.onAction(() => { page.scope.lastCommandRef.current.to = 5; });
  assert.equal(page.deposit(), false); assert.equal(page.attempts, 0);
  assert.equal(page.pending.size, 0);
});

test("actual synchronous reentry reserves both source and empty target and never reuses request IDs", () => {
  const page = harness(), reentry = [];
  page.onAction(() => {
    reentry.push(page.deposit(5)); // same source, different empty target
    reentry.push(page.deposit(4, 2)); // another UID, same empty destination
  });
  assert.equal(page.deposit(), true);
  assert.deepEqual(reentry, [false, false]); assert.equal(page.sent.length, 1);
  assert.equal(page.pending.size, 1); assert.equal(page.scope.storageRequestSequenceRef.current, 2);
  page.onAction(null); page.applyStorageAck(ack(page.sent[0]));
  assert.equal(page.pending.size, 0); assert.equal(page.deposit(), true);
  assert.equal(page.sent[1].requestId, requestId(2));
  page.applyStorageAck(ack(page.sent[0]));
  assert.equal(page.pending.has(requestId(2)), true, "duplicate old ACK cannot retire new flight");
});

test("actual synchronous reentry on independent cells keeps two distinct requests", () => {
  const page = harness(); let reentered = false;
  page.onAction(() => {
    if (!reentered) { reentered = true; assert.equal(page.deposit(5, 2), true); }
  });
  assert.equal(page.deposit(), true); assert.equal(page.pending.size, 2);
  assert.deepEqual(page.sent.map(command => command.requestId), [requestId(2), requestId(1)]);
  assert.equal(page.scope.storageRequestSequenceRef.current, 3);
});

test("actual definitely-unsent cleanup and socket.send unknown outcome have different ownership", () => {
  const closed = harness(); closed.socket.readyState = 3;
  assert.equal(closed.deposit(), false); assert.equal(closed.pending.size, 0);
  assert.equal(closed.scope.storageRequestSequenceRef.current, 2);
  const unknown = harness(); unknown.throwAtSocket();
  assert.equal(unknown.deposit(), false); assert.equal(unknown.attempts, 1);
  assert.equal(unknown.pending.size, 1); assert.equal(unknown.errors.length, 1);
  const proof = unknown.pending.get(requestId(1)).proof;
  assert.equal(unknown.pending.get(requestId(1)).enteredSocket, true);
  assert.equal(unknown.deposit(), false, "unknown outcome cannot be retried on the same cells");
  unknown.applyStorageAck(ack(proof, { success: false }));
  assert.equal(unknown.pending.size, 0, "exact failed ACK retires the entered old flight");
});

test("actual storage receipt case refuses wrong tuples malformed status and unentered flight", () => {
  for (const deposit of [true, false]) {
    const page = harness(); assert.equal(deposit ? page.deposit() : page.withdraw(), true);
    const command = page.sent[0];
    for (const event of [
      ack(command, { requestId: requestId(9) }), ack(command, { from: command.from + 1 }),
      ack(command, { to: command.to + 1 }), ack(command, { success: 0 }),
      ack(command, { success: undefined }), ack(command, {}, deposit ? "TakeBackItemV2" : "StoreItemV2"),
    ]) { page.applyStorageAck(event); assert.equal(page.pending.size, 1); }
    page.pending.get(command.requestId).enteredSocket = false;
    page.applyStorageAck(ack(command)); assert.equal(page.pending.size, 1);
    page.pending.get(command.requestId).enteredSocket = true;
    page.applyStorageAck(ack(command, { success: false }));
    assert.equal(page.pending.size, 0);
    page.applyStorageAck(ack(command)); assert.equal(page.pending.size, 0);
  }
});

test("actual Page cross-item gates lock reserved source and empty destination but allow unrelated items", () => {
  const page = harness(); assert.equal(page.deposit(), true);
  const before = page.sent.length;
  for (const command of [
    { type: "dropItem", uniqueId: 22 },
    { type: "useItem", uniqueId: 22, grid: "inventory" },
    { type: "splitItem", uniqueId: 22, grid: "inventory", count: 1 },
    { type: "mergeItem", idFrom: 33, idTo: 22, gridFrom: "inventory", gridTo: "inventory" },
    { type: "moveItem", grid: "inventory", from: 7, to: 43 },
    { type: "moveItem", grid: "storage", from: 83, to: 4 },
  ]) assert.equal(page.send(command), false, JSON.stringify(command));
  assert.equal(page.sent.length, before);
  assert.equal(page.send({ type: "useItem", grid: "inventory", uniqueId: 33 }), true);
  assert.equal(page.send({ type: "dropItem", uniqueId: 33 }), true);
  assert.equal(page.sent.length, before + 2);
});

test("actual Mail attach quote and Send final gates respect storage UID and newly occupied target cell", () => {
  const page = harness(); assert.equal(page.deposit(), true);
  assert.equal(page.mailParcelItemsIdle([0, 22]), false);
  assert.equal(page.mailParcelItemsIdle([0, 33]), true);
  page.scope.worldRef.current.storageItems.push(item(55, "storage", 4));
  assert.equal(page.mailParcelItemsIdle([55]), false, "UID not captured before event still occupies reserved target");
  // Mail ownership is controlled, while the real Page storage/idle gate and
  // exact immutable wire execute. No mock sends and no Mail WASM initialization.
  const mailOwner = page.currentSpellsOwner(Number(page.scope.worldRef.current.playerObjectId));
  let mailEntries = 0;
  page.scope.mailDispatcherRef.current = {
    allows: () => true, claim: () => true,
    composer: { enterSocket: () => { mailEntries++; return true; } },
  };
  // The functions capture references, so update the existing ref's current.
  page.scope.mailParcelRef.current = { state: { blockedUniqueIds: [55] }, snapshot: null,
    allowsSend: () => true, enter: () => true, enterLock: () => true };
  // Idle fixture sync intentionally throws: use an actual named Page function
  // extraction with fresh dependencies below to exercise the final stamped case.
  const mailScope = { ...page.scope, syncMailParcel: () => true,
    sameMailOwner: actualSameMailOwner,
    mailMutationAllowed: pureParcel.mailMutationAllowed,
  };
  mailScope.mailParcelRef.current.snapshot = { bagCapacity: 48, items: [
    { container: 4, slot: 4, uniqueId: 55, pricing: null, stamp: false },
    { container: 0, slot: 7, uniqueId: 33, pricing: null, stamp: false },
  ] };
  const keys = Object.keys(mailScope);
  const api = new Function(...keys, pageJavaScript + "\nreturn {sendRaw,mailParcelItemsIdle};")(...keys.map(key => mailScope[key]));
  const command = { type: "sendMail", gold: 0, itemsIdx: [0, 0, 0, 0, 0], stamped: true,
    recipient: "receiver", message: "body" };
  const proof = { commandType: "sendMail", mailId: null, owner: mailOwner, compose: {} };
  assert.equal(api.sendRaw(command, { mailProof: proof }), false,
    "reserved stamp UID participates via blockedUniqueIds, even when itemsIdx is empty");
  assert.equal(page.sent.length, 1);
  assert.equal(mailEntries, 0, "blocked stamp must not enter socket");
  for (const dto of [{ type: "mailLockedItem", uniqueId: 22, locked: true },
    { type: "mailCost", gold: 0, itemsIdx: [22, 0, 0, 0, 0] }]) {
    assert.equal(api.sendRaw(dto, dto.type === "mailCost" ? { mailQuoteProof: { owner: mailOwner } }
      : { mailLockProof: { owner: mailOwner } }), false);
  }
  assert.equal(api.sendRaw({ type: "mailLockedItem", uniqueId: 22, locked: false },
    { mailLockProof: { owner: mailOwner } }), true, "cleanup lock release remains available");
  page.scope.mailParcelRef.current.state.blockedUniqueIds = [33];
  assert.equal(api.sendRaw(command, { mailProof: proof }), true, "unrelated stamp is a live positive control");
  assert.equal(mailEntries, 1); assert.equal(page.sent.at(-1).type, "sendMail");
});

test("actual confirmed session retirement releases storage cells and retains monotonic request IDs", () => {
  const page = harness(); assert.equal(page.deposit(), true);
  const old = page.sent[0];
  assert.deepEqual(page.npcWithdrawals, []);
  page.retireEquipmentSession();
  assert.deepEqual(page.npcInventoryRetirements, ["invalidate"]);
  assert.deepEqual(page.npcWithdrawals, ["withdraw"]);
  assert.equal(page.pending.size, 0); assert.equal(page.terminations.length, 1);
  assert.equal(page.scope.storageRequestSequenceRef.current, 2);
  assert.equal(page.scope.equipmentHostSuspendReasonRef.current, "connectionUnavailable");
  assert.equal(page.scope.npcShopServiceRef.current, null);
  assert.equal(page.scope.npcBuySelectedRef.current, null);
  assert.equal(page.scope.npcShopClockRef.current.service, 4);
  page.applyStorageAck(ack(old)); assert.equal(page.pending.size, 0);
  const next = harness({ identity: { ...session, sessionGeneration: 6 },
    sequenceRef: page.scope.storageRequestSequenceRef, pendingRef: page.scope.pendingStorageRequestsRef });
  assert.equal(next.deposit(), true);
  assert.equal(next.sent[0].requestId, requestId(2));
  next.applyStorageAck(ack(old)); assert.equal(next.pending.has(requestId(2)), true);
});

test("actual request sequence exhaustion permanently refuses another reservation", () => {
  const page = harness(); page.scope.storageRequestSequenceRef.current = Number.MAX_SAFE_INTEGER;
  assert.equal(page.deposit(), true);
  assert.equal(page.sent[0].requestId, requestId(Number.MAX_SAFE_INTEGER));
  assert.equal(page.scope.storageRequestSequenceRef.current, 0);
  page.applyStorageAck(ack(page.sent[0]));
  assert.equal(page.deposit(), false); assert.equal(page.sent.length, 1);
});

// Execute only the real callback block, rather than reproducing its refusal
// branch. This covers compatibility selection retention without React/DOM.
const uiUrl = new URL("../app/components/original-client-inventory-window.tsx", import.meta.url);
const uiSource = readFileSync(uiUrl, "utf8");
const uiAst = ts.createSourceFile(fileURLToPath(uiUrl), uiSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function actualSelectionBlock(callback) {
  const calls = [];
  function find(node) {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === callback) calls.push(node);
    ts.forEachChild(node, find);
  }
  find(uiAst); assert.equal(calls.length, 1, "sole actual selection callback: " + callback);
  let block = calls[0].parent;
  while (block && !ts.isBlock(block)) block = block.parent;
  assert(block, "actual callback block");
  const compiled = ts.transpileModule("function invoke() " + block.getText(uiAst), {
    fileName: "actual-selection-block.ts",
    compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  return scope => {
    const keys = Object.keys(scope);
    return new Function(...keys, compiled + "\nreturn invoke;")(...keys.map(key => scope[key]))();
  };
}
for (const callback of ["onStoreItem", "onTakeBackItem"]) {
  test("actual inventory " + callback + " keeps selection on refusal and passes explicit authority/page on success", () => {
    const invoke = actualSelectionBlock(callback);
    for (const accepted of [false, true]) {
      const calls = [], clear = [], feedback = [], chosen = item(22, callback === "onStoreItem" ? "bag2" : "storage", 3);
      const scope = { pendingMoveItem: chosen, takeBackItem: chosen, absoluteSlot: 83,
        slotIndex: 7, activeTab: "bag2", slot: { key: "bag2:7" },
        [callback]: (...args) => { calls.push(args); return accepted; },
        setPendingMoveItem: value => clear.push(value), setDeleteFeedback: value => feedback.push(value),
        t: (key, _args, fallback) => fallback ?? key };
      invoke(scope);
      assert.equal(calls.length, 1); assert.equal(calls[0][0].authoritativeUniqueId, 22);
      assert.deepEqual(calls[0].slice(1), callback === "onStoreItem" ? [83] : [7, "bag2"]);
      assert.deepEqual(clear, accepted ? [null] : []);
      assert.equal(chosen.authoritativeUniqueId, 22); assert.equal(feedback.length, 1);
    }
  });
}


test("actual new-connection and successful StartGame branches clear storage before publishing identity without resetting IDs", () => {
  const connects = [], starts = [];
  function find(node) {
    if (ts.isFunctionDeclaration(node) && node.name?.text === "connectGateway") connects.push(node);
    if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression) && node.expression.text === "StartGame"
      && node.getText(pageAst).includes("++equipmentSessionGenerationRef.current")) starts.push(node);
    ts.forEachChild(node, find);
  }
  find(pageAst);
  assert.equal(connects.length, 1); assert.equal(starts.length, 1);
  const connection = connects[0].body.statements.map(statement => statement.getText(pageAst));
  const clear = connection.findIndex(text => text === "pendingStorageRequestsRef.current.clear();");
  const generation = connection.findIndex(text => text.includes("++equipmentConnectionGenerationRef.current"));
  const publish = connection.findIndex(text => text === "socketRef.current = socket;");
  assert(clear >= 0 && clear < generation && generation < publish,
    "new socket identity is published only after old transfer cells are released");
  const start = starts[0].getText(pageAst);
  const startClear = start.indexOf("pendingStorageRequestsRef.current.clear();");
  const startPublish = start.indexOf("equipmentStartGameRef.current =");
  assert(startClear >= 0 && startClear < startPublish,
    "successful character session retires old transfers before its new identity");
  for (const text of [connects[0].getText(pageAst), start]) {
    assert(!/storageRequestSequenceRef\.current\s*=(?!=)/.test(text),
      "connection/session reset must not recycle request IDs");
  }
});


test("actual Page Hero and parcel collection barriers preserve reserved and unknown custody", () => {
  const page = harness(), proof = Object.freeze({});
  const commands = [{type:"collectParcel",mailId:1},{type:"moveItem",grid:"inventory",from:3,to:4},
    {type:"transferHeroItem",from:3,to:2},{type:"retrieveTradeItem",from:0,to:3},{type:"guildStorageItemChange",changeType:1,from:0,to:3}];
  for (const state of ["reserved","entered","unknown","acknowledged"]) {
    page.scope.heroOperationsRef.current.pending = {proof,state};
    for (const command of commands) assert.equal(page.parityItemMutationAllowed(command),false,state+" retains "+command.type);
    assert.equal(page.parityItemMutationAllowed(commands[2],proof),state === "reserved","only exact unentered proof can pass its own reservation");
    assert.equal(page.parityItemMutationAllowed({type:"chat",message:"ordinary chat"}),true);
  }
  page.scope.heroOperationsRef.current.pending = null;
  page.scope.mailCollectBarrierRef.current = {owner:{},mailId:1,observedSnapshot:null};
  for (const command of commands) assert.equal(page.parityItemMutationAllowed(command),false,"unresolved collect retains "+command.type);
  page.scope.mailCollectBarrierRef.current = null;
  assert.equal(page.parityItemMutationAllowed(commands[1]),true,"a retired barrier restores the existing item path");
});

function exactGuildCatalogFixture(index=6) {
  return {item_index:index,name:"Runtime guild template",item_type:13,grade:0,required_type:0,
    required_class:31,required_gender:3,item_set:0,shape:0,weight:1,light:0,required_amount:1,
    image:77,durability:10,stack_size:20,price:1,start_item:false,effect:0,need_identify:false,
    show_group_pickup:false,class_based:false,level_based:false,can_mine:false,global_drop_notify:false,
    bind:0,unique:0,random_stats_id:0,can_fast_run:false,can_awakening:false,slots:0,stats:[],tooltip:null};
}
test("actual Page Guild runtime catalog projects an unobserved template while final UID slot and permission fences still apply", () => {
  const page=harness(); page.scope.socialCatalogRef.current.clear(); let queries=0;
  page.scope.runtimeRef.current={getMir2ItemCatalogInfo(json){queries++; assert.deepEqual(JSON.parse(json),{version:1,itemIndex:6});
    return JSON.stringify({version:1,ok:true,itemInfo:exactGuildCatalogFixture()});}};
  const source=openGuild(page); assert.equal(source.slots.length,112);
  assert.deepEqual(source.slots[3],{slot:3,uniqueId:66,name:"Runtime guild template",icon:77,count:1});
  assert.equal(source.slots[2],null); assert.equal(page.scope.socialCatalogRef.current.size,0); assert.equal(queries,1);
  page.submitSocialItem("guild2",3,5,66,source.sourceKey);
  assert.deepEqual(page.sent,[{type:"guildStorageItemChange",changeType:2,from:3,to:5}]);
  assert.equal(page.scope.socialItemOperationsRef.current.pending.entered,true); assert.equal(queries,1);
  for (const mutation of [p=>{p.scope.guildStorageRawRef.current.items[3]=guildRow(67);},
    p=>{p.scope.guildStorageRawRef.current.items[5]=p.scope.guildStorageRawRef.current.items[3];p.scope.guildStorageRawRef.current.items[3]=null;},
    p=>{p.scope.guildPermissionsRef.current.permissions=[];},
    p=>{p.scope.guildStorageRawRef.current.items[3].item.unique_id=Number.MAX_SAFE_INTEGER+1;},
    p=>{p.scope.guildStorageRawRef.current.items[5]=guildRow(66);}]) {
    const current=harness(); current.scope.socialCatalogRef.current.clear();
    current.scope.runtimeRef.current={getMir2ItemCatalogInfo:()=>JSON.stringify({version:1,ok:true,itemInfo:exactGuildCatalogFixture()})};
    const before=openGuild(current); current.onAction(()=>mutation(current));
    current.submitSocialItem("guild2",3,5,66,before.sourceKey);
    assert.equal(current.attempts,0); assert.equal(current.scope.socialItemOperationsRef.current.pending,null);
  }
  const fallback=harness(); fallback.scope.runtimeRef.current={getMir2ItemCatalogInfo:()=>"malformed"};
  assert.equal(openGuild(fallback).slots[3].name,"Guild item","older observed catalog remains the existing fallback");
  const malformed=harness(); malformed.scope.socialCatalogRef.current.clear();
  malformed.scope.runtimeRef.current={getMir2ItemCatalogInfo:()=>JSON.stringify({version:1,ok:true,itemInfo:exactGuildCatalogFixture(7)})};
  malformed.receive("GuildStatus",{guildName:"Guild",myOptions:24,typed:true});
  const items=Array(112).fill(null); items[3]=guildRow(); malformed.receive("GuildStorageList",{items,typed:true});
  assert.equal(malformed.readSocialItemSurface("guild"),null); assert.equal(malformed.attempts,0);
});


test("actual Page spouse Mail opens only a named local draft and Whisper uses exact Native text without sending", () => {
  const page = harness();
  page.scope.worldRef.current.stage5Systems.relationship = {name: "Spouse", mapName: "D002", marriedDays: 7};
  page.setShowBonds(true);
  const relationship = page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship);
  const callbacks = page.captureSpouseRender(true, {relationship});
  assert.equal(typeof callbacks.onMailPartner, "function");
  assert.equal(typeof callbacks.onWhisperPartner, "function");
  callbacks.onMailPartner(); callbacks.onWhisperPartner();
  assert.deepEqual(page.scope.mailOpenEvents, [true]);
  assert.deepEqual(page.scope.mailComposeEvents, [["compose", null, "Spouse"]]);
  assert.deepEqual(page.scope.chatComposeEvents, [":)"], "spouse flow is not the friend's /name prefix");
  assert.deepEqual(page.sent, []); assert.equal(page.attempts, 0); assert.equal(page.actions, 0);
  page.openMailWindow("Friend");
  assert.deepEqual(page.scope.mailComposeEvents.at(-1), ["compose", null, "Friend"], "existing friend/local compose entry is preserved");
});

test("actual Page spouse availability uses received mapName while unmarried and stale displayed relationships are rejected", () => {
  const page = harness(); page.setShowBonds(true);
  page.scope.worldRef.current.stage5Systems.relationship = {name: "Spouse", mapName: "", partnerOnline: true};
  const offline = page.captureSpouseRender(true, {relationship: page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship)});
  assert.equal(typeof offline.onMailPartner, "function"); assert.equal(offline.onWhisperPartner, undefined);
  offline.onMailPartner();
  assert.deepEqual(page.scope.mailComposeEvents, [["compose", null, "Spouse"]]);
  const source = page.captureSpouseUiSource(page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship));
  page.whisperSpouse(source); assert.deepEqual(page.scope.chatComposeEvents, []);
  assert.deepEqual(page.captureSpouseRender(false, {relationship: page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship)}),
    {onMailPartner: undefined, onWhisperPartner: undefined});
  assert.equal(page.captureSpouseUiSource({partnerName: "Old spouse", partnerMap: ""}), null, "displayed source must equal the live adapter projection");
  for (const relationship of [null, {}, {name: "", mapName: "D002"}]) {
    page.scope.worldRef.current.stage5Systems.relationship = relationship;
    const shown = page.scope.adaptRelationship(relationship);
    assert.equal(page.captureSpouseUiSource(shown), null);
    assert.deepEqual(page.captureSpouseRender(true, {relationship: shown}), {onMailPartner: undefined, onWhisperPartner: undefined});
  }
  page.mailSpouse(source); page.whisperSpouse(source);
  assert.equal(page.scope.mailComposeEvents.length, 1); assert.equal(page.attempts, 0);
});

test("actual Page retained spouse callbacks reject physical owner scene visibility window epoch and relationship changes", () => {
  const mutations = [
    ["other socket", p => { p.scope.socketRef.current = {readyState: 1}; }],
    ["closed socket", p => { p.socket.readyState = 0; }],
    ["connection", p => { p.scope.equipmentConnectionGenerationRef.current++; }],
    ["session", p => { p.scope.equipmentSessionGenerationRef.current++; }],
    ["new coherent session", p => { p.scope.equipmentSessionGenerationRef.current++; p.scope.equipmentStartGameRef.current.sessionGeneration++; }],
    ["character", p => { p.scope.worldRef.current.playerObjectId = "4"; }],
    ["scene revision", p => { p.scope.socialSceneRevisionRef.current++; }],
    ["map", p => { p.scope.worldRef.current.mapFileName = "D003"; }],
    ["disconnected", p => { p.scope.worldRef.current.connected = false; }],
    ["not game", p => { p.scope.screenRef.current = "login"; }],
    ["hidden", p => { p.scope.document.visibilityState = "hidden"; }],
    ["paused owner", p => { p.scope.equipmentHostSuspendReasonRef.current = "logoutPending"; }],
    ["window closed", p => { p.setShowBonds(false); }],
    ["window close/reopen", p => { p.setShowBonds(false); p.setShowBonds(true); }],
    ["reply close/reopen", p => { p.closeSocialReplyWindow("bonds"); p.setShowBonds(true); }],
    ["retired requests", p => { p.retireSocialRequests(); }],
    ["partner", p => { p.scope.worldRef.current.stage5Systems.relationship.name = "Other spouse"; }],
    ["partner map", p => { p.scope.worldRef.current.stage5Systems.relationship.mapName = "D004"; }],
    ["projection source", p => { p.scope.worldRef.current.stage5Systems.relationship.marriedDays++; }],
    ["unmarried", p => { p.scope.worldRef.current.stage5Systems.relationship = null; }],
  ];
  for (const [label, mutate] of mutations) {
    const page = harness();
    page.scope.worldRef.current.stage5Systems.relationship = {name: "Spouse", mapName: "D002", marriedDays: 7};
    page.setShowBonds(true);
    const callbacks = page.captureSpouseRender(true, {relationship: page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship)});
    assert.equal(typeof callbacks.onMailPartner, "function", label); assert.equal(typeof callbacks.onWhisperPartner, "function", label);
    mutate(page); callbacks.onMailPartner(); callbacks.onWhisperPartner();
    assert.deepEqual(page.scope.mailOpenEvents, [], label); assert.deepEqual(page.scope.mailComposeEvents, [], label);
    assert.deepEqual(page.scope.chatComposeEvents, [], label); assert.equal(page.attempts, 0, label);
  }
});

test("actual Page spouse mail rechecks after opening and old window callbacks cannot target a new compose owner", () => {
  const page = harness();
  page.scope.worldRef.current.stage5Systems.relationship = {name: "Spouse", mapName: "D002"};
  page.setShowBonds(true);
  const callbacks = page.captureSpouseRender(true, {relationship: page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship)});
  const capturedEpoch = page.scope.bondsWindowLeaseRef.current.epoch;
  page.scope.onMailOpen = () => { page.setShowBonds(false); page.setShowBonds(true); };
  callbacks.onMailPartner();
  assert.ok(page.scope.bondsWindowLeaseRef.current.epoch > capturedEpoch);
  assert.deepEqual(page.scope.mailOpenEvents, [true]); assert.deepEqual(page.scope.mailComposeEvents, []);
  callbacks.onMailPartner(); callbacks.onWhisperPartner();
  assert.deepEqual(page.scope.mailOpenEvents, [true]); assert.deepEqual(page.scope.chatComposeEvents, []);
  delete page.scope.onMailOpen;
  const fresh = page.captureSpouseRender(true, {relationship: page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship)});
  fresh.onMailPartner(); fresh.onWhisperPartner();
  assert.deepEqual(page.scope.mailComposeEvents, [["compose", null, "Spouse"]]);
  assert.deepEqual(page.scope.chatComposeEvents, [":)"]); assert.equal(page.attempts, 0);
});


test("actual Bonds spouse buttons derive disabled state from name and map and ExtraWindows forwards their exact callbacks", () => {
  const url = new URL("../app/components/original-client-bonds-window.tsx", import.meta.url);
  const ast = ts.createSourceFile(fileURLToPath(url), readFileSync(url, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const component = ast.statements.find(n => ts.isFunctionDeclaration(n) && n.name?.text === "BondsWindow");
  assert.ok(component);
  const selected = new Set(["partner", "married", "partnerOnline", "canMailPartner", "canWhisperPartner"]);
  const locals = component.body.statements.filter(n => ts.isVariableStatement(n)
    && n.declarationList.declarations.some(d => selected.has(d.name.getText(ast))));
  assert.equal(locals.length, 5, "use actual derived component availability and online status");
  const buttons = [];
  function visitButton(node) {
    if (ts.isJsxOpeningElement(node) && node.tagName.getText(ast) === "button") {
      const fields = new Map(node.attributes.properties.filter(ts.isJsxAttribute).map(a => [a.name.text, a.initializer]));
      const action = fields.get("data-bonds-action");
      if (action && ts.isStringLiteral(action) && ["mail-spouse", "whisper-spouse"].includes(action.text)) {
        const expression = name => { const value = fields.get(name); assert.ok(value, name);
          if (ts.isStringLiteral(value)) return JSON.stringify(value.text);
          assert.ok(ts.isJsxExpression(value) && value.expression, name); return value.expression.getText(ast); };
        buttons.push('{action:'+JSON.stringify(action.text)+',type:'+expression("type")+',disabled:'+expression("disabled")
          +',label:'+expression("aria-label")+',click:'+expression("onClick")+'}');
      }
    }
    ts.forEachChild(node, visitButton);
  }
  visitButton(component); assert.equal(buttons.length, 2);
  const componentCode = ts.transpileModule(locals.map(n => n.getText(ast)).join("\n")
    + "\nreturn Object.assign([" + buttons.join(",") + "], {partnerOnline});", {compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText;
  const controls = new Function("relationship", "onMailPartner", "onWhisperPartner", "t", componentCode);
  const bridgeUrl = new URL("../app/components/original-client-extra-windows.tsx", import.meta.url);
  const bridgeAst = ts.createSourceFile(fileURLToPath(bridgeUrl), readFileSync(bridgeUrl, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let bridge;
  function visitBridge(node) {
    if (ts.isJsxSelfClosingElement(node) && node.tagName.getText(bridgeAst) === "BondsWindow") {
      assert.equal(bridge, undefined, "sole actual Bonds component bridge");
      bridge = node.attributes.properties.filter(a => ts.isJsxAttribute(a)
        && ["onMailPartner", "onWhisperPartner"].includes(a.name.text)).map(a => {
          assert.ok(ts.isJsxExpression(a.initializer) && a.initializer.expression);
          return a.name.text + ":" + a.initializer.expression.getText(bridgeAst);
        });
    }
    ts.forEachChild(node, visitBridge);
  }
  visitBridge(bridgeAst); assert.equal(bridge?.length, 2);
  const forward = new Function("bonds", "return {"+bridge.join(",")+"};");
  const page = harness(); page.setShowBonds(true);
  page.scope.worldRef.current.stage5Systems.relationship = {name:"Spouse",mapName:"D002"};
  const relationship = page.scope.adaptRelationship(page.scope.worldRef.current.stage5Systems.relationship);
  const callbacks = page.captureSpouseRender(true, {relationship}), props = forward(callbacks);
  assert.equal(props.onMailPartner, callbacks.onMailPartner); assert.equal(props.onWhisperPartner, callbacks.onWhisperPartner);
  const t = (_key,_args,fallback) => fallback;
  const active = controls(relationship,props.onMailPartner,props.onWhisperPartner,t);
  assert.equal(active.partnerOnline, true, "actual adapter map enables the online label without a separate flag");
  assert.deepEqual(active.map(b => ({action:b.action,type:b.type,disabled:b.disabled,label:b.label})), [
    {action:"mail-spouse",type:"button",disabled:false,label:"Mail Spouse"},
    {action:"whisper-spouse",type:"button",disabled:false,label:"Whisper Spouse"}]);
  active[0].click(); active[1].click();
  assert.deepEqual(page.scope.mailComposeEvents, [["compose",null,"Spouse"]]); assert.deepEqual(page.scope.chatComposeEvents,[":)"]);
  for (const [source, mail, whisper, expected] of [
    [null,props.onMailPartner,props.onWhisperPartner,[true,true]],
    [{partnerName:"",partnerMap:"D002"},props.onMailPartner,props.onWhisperPartner,[true,true]],
    [{partnerName:"Spouse",partnerMap:"",partnerOnline:true},props.onMailPartner,props.onWhisperPartner,[false,true]],
    [{partnerName:"Spouse",partnerMap:"D002",partnerOnline:false},props.onMailPartner,props.onWhisperPartner,[false,false]],
    [relationship,undefined,undefined,[true,true]],
  ]) {
    const derived = controls(source,mail,whisper,t);
    assert.deepEqual(derived.map(b => b.disabled),expected);
    assert.equal(derived.partnerOnline, Boolean(source?.partnerName?.trim() && source?.partnerMap?.length),
      "status and Whisper use the same authoritative spouse map");
  }
  assert.equal(page.attempts,0);
});

// Auth seams are data-only fake optional ABI responses and transport observations.
// The validation/admission/focus/error/lease logic remains production extraction.
const registrationDraft = () => ({accountId:"NewUser",password:"Secret9",confirmPassword:"Secret9",
  userName:"Player",birthDate:"1970-01-01",secretQuestion:"Question",secretAnswer:"Answer",emailAddress:"u@example.com"});
function authAbi() {
  const calls=[], values={registration:"621355968000000000",password:0,keys:"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",edited:"SharedEdit",freed:0};
  class AuthUiBridge {
    constructor(seed) { calls.push(["construct",seed]); }
    keys() {calls.push(["keys"]);return values.keys;}
    reshuffle() {calls.push(["reshuffle"]);return values.keys.split("").reverse().join("");}
    edit(...args) {calls.push(["edit",...args]);return values.edited;}
    free() {values.freed++;}
  }
  const module={AuthUiBridge,auth_ui_abi_version:()=>1,
    auth_ui_validate_registration:(...args)=>{calls.push(["registration",...args]);return values.registration;},
    auth_ui_validate_password:(...args)=>{calls.push(["password",...args]);return values.password;}};
  return {module,calls,values};
}
function authPage(surface="login",ready=true) {
  const page=harness(), abi=authAbi();
  page.scope.screenRef.current="login";
  if(ready) page.scope.authCoreRef.current=pureAuthCore.createAuthUiRuntime(abi.module,1n);
  page.scope.questCoreRuntimeRef.current={createAuthUi:seed=>pureAuthCore.createAuthUiRuntime(abi.module,seed)};
  if(surface!=="login") page.openLoginAuth(surface,page.scope.loginAuthRef.current.epoch);
  const controls=()=>page.captureLoginAuthControls(page.scope.loginAuthRef.current,ready,page.scope.loginBusyRef.current);
  return Object.assign(page,{abi,controls});
}
function fillRegistration(page, draft=registrationDraft()) {
  const controls=page.controls();
  for(const [key,value] of Object.entries(draft)) controls.registrationChange(key,value);
}
function fillPassword(page) {
  const controls=page.controls();
  for(const [key,value] of Object.entries({accountId:"Existing",oldPassword:"Old9Secret",newPassword:"New9Secret",confirmPassword:"New9Secret"}))
    controls.passwordChange(key,value);
}
function deferred() {let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return {promise,resolve,reject};}

test("actual auth persistent physical lane claims once and terminal never reopens the same socket",()=>{
  const host={},gate=pureAuth.persistentPreauthGate(host),socket={},foreign={};
  assert.equal(pureAuth.persistentPreauthGate(host),gate);
  const proof=gate.reserve(socket,3,1,"newAccount"); assert(Object.isFrozen(proof));
  assert.equal(gate.allows({...proof},socket,3,"newAccount"),false);
  assert.equal(gate.allows(proof,foreign,3,"newAccount"),false);
  assert.equal(gate.enter(proof,socket,4,"newAccount"),false);
  assert.equal(gate.enter(proof,socket,3,"changePassword"),false);
  assert.equal(gate.enter(proof,socket,3,"newAccount"),true);
  assert.equal(gate.enter(proof,socket,3,"newAccount"),false);
  assert.equal(gate.cancelUnsent(proof),false);
  assert.equal(gate.complete(socket,4,"newAccount"),null);
  assert.equal(gate.complete(socket,3,"changePassword"),null);
  assert.equal(gate.complete(socket,3,"newAccount"),proof);
  assert.equal(gate.complete(socket,3,"newAccount"),null);
  assert.equal(gate.used(socket),true); assert.equal(gate.pending(socket),false);
  assert.equal(gate.reserve(socket,4,2,"login"),null);
  assert.ok(gate.reserve(foreign,4,2,"login"));
});

test("actual registration command keeps an exact DOB number token and omits confirmation and ISO draft",()=>{
  const command=pureAuth.newAccountCommand(registrationDraft(),"621355968000000000");
  assert(Object.isFrozen(command)); const text=JSON.stringify(command);
  assert.equal(text.match(/"birthDateBinary":([0-9]+)/)[1],"621355968000000000");
  assert.deepEqual(Object.keys(command).sort(),["type","accountId","password","birthDateBinary","userName","secretQuestion","secretAnswer","emailAddress"].sort());
  assert.equal(Object.hasOwn(command,"confirmPassword"),false); assert.equal(Object.hasOwn(command,"birthDate"),false);
  for(const binary of ["-1","01","621355968000000001","621355968000000512","3155378112000000001","9223372036854775807","NaN"])
    assert.throws(()=>pureAuth.newAccountCommand(registrationDraft(),binary),undefined,binary);
});

test("actual Page registration queues immutable eight fields before Ready and double click sends once",()=>{
  const page=authPage("registration");fillRegistration(page);
  page.scope.gatewayProtocolReadyRef.current=false;
  const old=page.controls(); old.submitRegistration(); old.submitRegistration();
  const pending=page.scope.pendingGatewayProtocolActionRef.current;
  assert.equal(pending.kind,"auth"); assert.equal(pending.attempt.kind,"newAccount");
  assert(Object.isFrozen(pending.attempt.command));
  assert.deepEqual(page.abi.calls.find(c=>c[0]==="registration").slice(1),Object.values(registrationDraft()));
  old.registrationChange("password","Changed9"); // busy draft is immutable
  assert.equal(page.scope.loginAuthRef.current.registration.password,"Secret9");
  page.scope.loginAuthRef.current={...page.scope.loginAuthRef.current,registration:{...registrationDraft(),password:"Later9"}};
  assert.equal(pending.attempt.command.password,"Secret9");
  assert.equal(page.sent.length,0);assert.equal(page.scope.authConnections.length,1);
  page.receive("Connected",{});
  assert.deepEqual(page.sent.map(c=>c.type),["setLanguage","clientVersion","newAccount"]);
  assert.equal(page.sent[2].password,"Secret9");assert.equal(page.sent[2].birthDateBinary,621355968000000000);
  assert.equal(Object.hasOwn(page.sent[2],"confirmPassword"),false);
  page.receive("Connected",{}); assert.equal(page.sent.filter(c=>c.type==="newAccount").length,1);
});

test("actual Page password change never carries confirmation or saves replacement secrets as reconnect credentials",()=>{
  const page=authPage("changePassword");fillPassword(page);
  const retained={kind:"password",accountId:"OldActor",password:"OriginalReconnect9"};
  page.scope.activeReconnectAuthRef.current=retained;
  page.controls().submitChangePassword();page.controls().submitChangePassword();
  assert.deepEqual(page.sent.map(c=>c.type),["clientVersion","changePassword"]);
  assert.deepEqual(page.sent[1],{type:"changePassword",accountId:"Existing",currentPassword:"Old9Secret",newPassword:"New9Secret"});
  assert.equal(page.scope.activeReconnectAuthRef.current,retained);
  assert.equal(page.scope.passwordRef.current,"ExistingSecret");
  page.receive("ChangePassword",{result:0});
  assert.equal(page.scope.loginAuthRef.current.surface,"login");assert.ok(page.scope.loginAuthRef.current.notice);
  assert.equal(page.preauthGate().used(page.socket),true);
  const registration=authPage("registration");fillRegistration(registration);
  registration.scope.activeReconnectAuthRef.current=retained;registration.controls().submitRegistration();
  registration.receive("NewAccount",{result:8});
  assert.equal(registration.scope.activeReconnectAuthRef.current,retained);
  assert.equal(registration.scope.passwordRef.current,"ExistingSecret");
});

test("actual Page closing form retains entered lane and late or foreign replies cannot display retired UI",()=>{
  const page=authPage("registration");fillRegistration(page);
  const controls=page.controls();controls.submitRegistration();
  assert.equal(page.preauthGate().pending(page.socket),true);
  controls.close();const retired=page.scope.loginAuthRef.current;
  assert.equal(page.preauthGate().pending(page.socket),true);assert.equal(retired.surface,"login");
  page.receive("NewAccount",{result:8},page.scope.equipmentConnectionGenerationRef.current,{});
  assert.equal(page.scope.loginAuthRef.current,retired);
  page.receive("NewAccount",{result:8});
  assert.equal(page.scope.loginAuthRef.current,retired);assert.equal(page.preauthGate().used(page.socket),true);
  assert.equal(page.preauthGate().pending(page.socket),false);
  controls.submitRegistration();assert.equal(page.sent.filter(c=>c.type==="newAccount").length,1);
  page.controls().open("registration");fillRegistration(page);page.controls().submitRegistration();
  assert.deepEqual(page.scope.authConnections.at(-1),{bootstrap:false,fresh:true});
  const oldSocket=page.socket,newSocket={readyState:1,send:body=>page.sent.push(JSON.parse(body))};
  page.scope.socketRef.current=newSocket;page.scope.equipmentConnectionGenerationRef.current++;
  page.scope.gatewayProtocolReadyRef.current=false;page.receive("Connected",{});
  assert.equal(page.sent.filter(c=>c.type==="newAccount").length,2);assert.equal(page.preauthGate().used(oldSocket),true);
});

test("actual auth send final lease rejects synchronous UI and physical generation changes plus legacy proofless sends",()=>{
  for(const mutate of [
    p=>p.scope.document.visibilityState="hidden",
    p=>p.scope.screenRef.current="game",
    p=>p.controls().close(),
    p=>p.scope.equipmentConnectionGenerationRef.current++,
    p=>p.scope.socketRef.current={readyState:1,send:()=>assert.fail("foreign socket")},
  ]) {
    const page=authPage("registration");fillRegistration(page);
    page.onAction(event=>{if(event.detail.type==="newAccount")mutate(page);});
    page.controls().submitRegistration();
    assert.equal(page.sent.some(c=>c.type==="newAccount"),false);
    assert.equal(page.preauthGate().used(page.socket),false,"definitely-unsent lane can retire");
  }
  const page=authPage();
  for(const command of [{type:"login",accountId:"User",password:"Secret"},{type:"passkeyLogin",accountId:"User",token:"Token"},
    pureAuth.newAccountCommand(registrationDraft(),"621355968000000000"),{type:"changePassword",accountId:"User",currentPassword:"Old",newPassword:"New"}])
    assert.equal(page.send(command),false);
  assert.equal(page.attempts,0);
});

test("actual auth entered socket exception stays unknown and cannot replay on the same physical connection",()=>{
  const page=authPage("registration");fillRegistration(page);page.throwAtSocket("newAccount");
  page.controls().submitRegistration();
  assert.equal(page.preauthGate().pending(page.socket),true);assert.equal(page.preauthGate().used(page.socket),true);
  assert.equal(page.scope.authSendRef.current,null);assert.equal(page.scope.loginBusyRef.current,false);
  assert.match(page.scope.authErrorEvents.at(-1),/outcome is unknown/);
  const actions=page.actions;page.controls().submitRegistration();
  assert.equal(page.actions,actions);assert.deepEqual(page.scope.authConnections.at(-1),{bootstrap:false,fresh:true});
  assert.equal(page.sent.some(c=>c.type==="newAccount"),false);
  assert.deepEqual(page.scope.window.__mir2CommandHistory.filter(c=>c.type==="newAccount").map(c=>Object.keys(c).sort()),[["at","type"]]);
});

test("actual optional Core wrapper delegates ordered fields errors focus and SafeKey edits without JS validation",()=>{
  const abi=authAbi(),core=pureAuthCore.createAuthUiRuntime(abi.module,7n),draft=registrationDraft();
  assert.deepEqual(core.validateRegistration(draft),{ok:true,birthDateBinary:"621355968000000000"});
  assert.deepEqual(abi.calls.find(c=>c[0]==="registration"),["registration",...Object.values(draft)]);
  abi.values.registration="e9";
  const failure=core.validateRegistration(draft);
  assert.deepEqual(failure,pureAuthCore.authValidationError(9));assert.equal(failure.field,"birthDate");
  abi.values.password=12;
  assert.deepEqual(core.validatePassword({accountId:"User",oldPassword:"Old",newPassword:"New",confirmPassword:"Mismatch"}),
    pureAuthCore.authValidationError(12));
  assert.equal(core.keys(),"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789");
  assert.equal(core.reshuffle(),"9876543210ZYXWVUTSRQPONMLKJIHGFEDCBA");
  assert.equal(core.edit("Existing","Q",true,false),"SharedEdit");
  assert.deepEqual(abi.calls.at(-1),["edit","Existing","Q",true,false]);
  assert.throws(()=>core.validateRegistration({...draft,userName:"\ud800"}),/encoding/);
  abi.values.registration="e14";assert.throws(()=>core.validateRegistration(draft),/birth date/);
  abi.values.registration="3155378976000000000";assert.throws(()=>core.validateRegistration(draft),/birth date/);
  abi.values.keys="A".repeat(36);assert.throws(()=>core.keys(),/keyboard/);
  core.dispose();core.dispose();assert.equal(abi.values.freed,1);assert.throws(()=>core.edit("x","Q",false,false),/retired/);
  const page=authPage("registration");fillRegistration(page);page.abi.values.registration="e9";page.controls().submitRegistration();
  assert.equal(page.scope.loginAuthRef.current.focusField,"birthDate");assert.equal(page.sent.length,0);
  const dom=actualAuthDom(),view=dom.render(page);view.focus();assert.equal(dom.doc.activeElement,view.fields.find(n=>n.props.name==="birthDate"));
  let repeated=0;dom.doc.activeElement.focus=()=>repeated++;view.focus();assert.equal(repeated,0,"actual focus effect does not refocus the already active field");
  for(const module of [{},{...abi.module,auth_ui_abi_version:()=>0},{...abi.module,auth_ui_validate_registration:undefined}])
    assert.throws(()=>pureAuthCore.createAuthUiRuntime(module,1n),/unavailable/);
});

test("actual missing optional auth Core preserves ordinary login while disabling new form submission",()=>{
  const page=authPage("registration",false);fillRegistration(page);page.controls().submitRegistration();assert.equal(page.sent.length,0);
  page.controls().close();page.controls().safeEnter();assert.equal(page.sent.length,0);
  page.submitLogin();assert.deepEqual(page.sent.map(c=>c.type),["clientVersion","login"]);
  assert.equal(page.sent[1].accountId,"Existing");assert.equal(page.sent[1].password,"ExistingSecret");
});

test("actual quick-enter captures one immutable pre-Ready attempt and complete identity leases reject stale sources",()=>{
  const page=authPage();page.scope.gatewayProtocolReadyRef.current=false;
  page.quickEnterWorld();page.quickEnterWorld();
  const attempt=page.scope.pendingGatewayProtocolActionRef.current.attempt;
  assert.equal(attempt.bootstrap,true);assert.equal(attempt.kind,"login");assert(Object.isFrozen(attempt.command));
  page.scope.accountIdRef.current="Changed";page.scope.passwordRef.current="ChangedSecret";
  page.receive("Connected",{});
  assert.deepEqual(page.sent.map(c=>c.type),["setLanguage","clientVersion","login","startGame"]);
  assert.deepEqual(page.sent[2],{type:"login",accountId:"Existing",password:"ExistingSecret"});
  for(const mutate of [
    p=>p.scope.loginAuthRef.current={...p.scope.loginAuthRef.current,epoch:2},
    p=>p.scope.oauthIntentRef.current++,
    p=>p.scope.socketRef.current={},
    p=>p.scope.equipmentConnectionGenerationRef.current++,
    p=>p.scope.activeReconnectAuthRef.current={kind:"sui"},
    p=>p.scope.screenRef.current="game",
  ]) {
    const next=authPage(),lease=next.captureIdentityIntent();
    assert.equal(next.identityIntentCurrent({...lease}),true);mutate(next);assert.equal(next.identityIntentCurrent(lease),false);
  }
});

test("actual OAuth and identity linking ignore retired owner results at both awaited boundaries",async()=>{
  for(const mutate of [p=>p.controls().close(),p=>p.scope.socketRef.current={},p=>p.scope.equipmentConnectionGenerationRef.current++]) {
    const page=authPage(),response=deferred();page.scope.oauthProvider=()=>response.promise;
    const work=page.submitSuiLogin("passkey");mutate(page);
    response.resolve({accountId:"obl_old",token:"OldToken",expiresAt:Date.now()+10000});await work;
    assert.equal(page.sent.length,0);assert.equal(page.scope.activeReconnectAuthRef.current,null);
  }
  for(const boundary of ["credential","link"]) {
    const page=authPage(),credential=deferred(),linked=deferred(),calls=[];
    page.scope.activeReconnectAuthRef.current={kind:"sui",accountId:"obl_old",token:"OldToken",expiresAt:Date.now()+10000};
    page.scope.credentialProvider=()=>credential.promise;
    page.scope.linkProvider=(...args)=>{calls.push(args);return linked.promise;};
    const work=page.linkCurrentSuiIdentity("passkey");
    page.linkCurrentSuiIdentity("passkey");assert.equal(page.scope.identityLinkIntentRef.current,1);
    if(boundary==="link"){credential.resolve({provider:"suiPasskey"});await Promise.resolve();assert.equal(calls.length,1);}
    page.scope.activeReconnectAuthRef.current={kind:"sui",accountId:"obl_new",token:"NewToken",expiresAt:Date.now()+10000};
    page.scope.identityLinkIntentRef.current++;page.scope.identityLinkEvents.push("NewActorStatus");
    credential.resolve({provider:"suiPasskey"});linked.resolve({identityCount:2});await work;
    assert.equal(calls.length,boundary==="credential"?0:1);
    if(calls.length)assert.deepEqual(calls[0],["obl_old","OldToken",{provider:"suiPasskey"}]);
    assert.equal(page.scope.identityLinkEvents.at(-1),"NewActorStatus");
    assert.equal(page.scope.identityProvider,undefined);
  }
});

function actualAuthDom() {
  const source=readFileSync(new URL("../app/components/original-client-overlays.tsx",import.meta.url),"utf8");
  const ast=ts.createSourceFile("auth-overlays.tsx",source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const selected=ast.statements.filter(n=>ts.isFunctionDeclaration(n)&&n.name?.text==="LoginAuthDialog"
    ||ts.isVariableStatement(n)&&n.declarationList.declarations.some(d=>["LOGIN_REGISTRATION_FIELDS","LOGIN_PASSWORD_FIELDS"].includes(d.name.getText(ast))));
  assert.equal(selected.length,3);
  const Login=ast.statements.find(n=>ts.isFunctionDeclaration(n)&&n.name?.text==="LoginOverlay");assert.ok(Login);
  const gates=Login.body.statements.filter(n=>ts.isVariableStatement(n)&&n.declarationList.declarations.some(d=>["authBlocked","loginBlocked"].includes(d.name.getText(ast))));
  assert.equal(gates.length,2);
  const focusEffects=Login.body.statements.filter(n=>ts.isExpressionStatement(n)&&ts.isCallExpression(n.expression)&&n.expression.expression.getText(ast)==="useEffect");
  assert.equal(focusEffects.length,1);const focusBody=focusEffects[0].expression.arguments[0].body.getText(ast);
  const nodes=[],normalEnter=[];
  function visit(n) {
    if(ts.isJsxSelfClosingElement(n)&&n.tagName.getText(ast)==="input") {
      const cls=n.attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.text==="className");
      if(cls?.initializer&&ts.isStringLiteral(cls.initializer)&&["login-input account","login-input password"].includes(cls.initializer.text)) {
        const handler=n.attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.text==="onKeyDown");
        normalEnter.push(handler.initializer.expression.getText(ast));
      }
    }
    if(ts.isJsxElement(n)&&n.openingElement.tagName.getText(ast)==="div") {
      const cls=n.openingElement.attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.text==="className");
      if(cls?.initializer&&ts.isStringLiteral(cls.initializer)&&["login-button account","login-button password","login-button view"].includes(cls.initializer.text))
        nodes.push(n);
    }
    if(ts.isJsxElement(n)&&n.openingElement.tagName.getText(ast)==="form") {
      const cls=n.openingElement.attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.text==="className");
      if(cls?.initializer&&ts.isStringLiteral(cls.initializer)&&cls.initializer.text==="login-dialog") {
        const onSubmit=n.openingElement.attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.text==="onSubmit");
        nodes.normalSubmit=onSubmit.initializer.expression.getText(ast);
      }
    }
    ts.forEachChild(n,visit);
  }
  visit(Login);assert.equal(nodes.length,3);assert.equal(normalEnter.length,2);assert.ok(nodes.normalSubmit);
  const doc={activeElement:null};
  const React={createElement:(type,props,...children)=>({type,props:props??{},children:children.flat(Infinity).filter(c=>c!=null&&c!==false)})};
  const js=ts.transpileModule(selected.map(n=>n.getText(ast)).join("\n")
    +"\nfunction actualFocus(authSurface,authEpoch,authFocusField,authSafeFocusRef,overlayRef)"+focusBody
    +"\nfunction actualOpenButtons(loginAuth,loginBusy,t){"+gates.map(n=>n.getText(ast)).join("\n")+";return ["
    +nodes.map(n=>n.getText(ast)).join(",")+"];}"
    +"\nfunction actualNormalGate(loginAuth,loginBusy,onSubmitLogin){"+gates.map(n=>n.getText(ast)).join("\n")
    +";return {loginBlocked,authBlocked,onSubmit:"+nodes.normalSubmit+",onEnter:["+normalEnter.join(",")+"]};}",
    {compilerOptions:{target:ts.ScriptTarget.ES2022,jsx:ts.JsxEmit.React}}).outputText;
  const compiled=new Function("React","document","ORIGINAL_UI","SpriteButton",js+"\nreturn {LoginAuthDialog,actualOpenButtons,actualNormalGate,actualFocus};")(
    React,doc,{login:{dialog:"fixture-asset",buttons:{}}},"SpriteButton");
  const flatten=node=>node&&typeof node==="object"?[node,...node.children.flatMap(flatten)]:[];
  function render(page) {
    const controls=page.controls(),gates=compiled.actualNormalGate(controls,page.scope.loginBusyRef.current,()=>page.submitLogin());
    const root=compiled.LoginAuthDialog({t:(_k,_a,f)=>f,loginAuth:controls,blocked:gates.authBlocked,
      accountId:page.scope.accountIdRef.current,password:page.scope.passwordRef.current,
      onAccountIdChange:v=>{page.scope.accountIdRef.current=v;},onPasswordChange:v=>{page.scope.passwordRef.current=v;}});
    const all=flatten(root);
    for(const node of all) {
      node.focus=()=>{doc.activeElement=node;node.props.onFocus?.();};
      node.disabled=!!node.props.disabled;node.dataset={loginAuthField:node.props["data-login-auth-field"]};
    }
    const form=all.find(n=>n.type==="form"),fields=all.filter(n=>n.type==="input"),submit=all.find(n=>n.props["data-login-auth-submit"]);
    form.querySelectorAll=selector=>selector.startsWith("[data-login-auth-field]")
      ?fields.filter(n=>!n.disabled):all.filter(n=>["input","button"].includes(n.type)&&!n.disabled);
    form.querySelector=selector=>selector.includes("data-login-auth-close")?all.find(n=>n.props["data-login-auth-close"]):submit.disabled?null:submit;
    form.contains=node=>all.includes(node);
    function focus(){compiled.actualFocus(controls.state.surface,controls.state.epoch,controls.state.focusField,
      {current:controls.state.safeFocus},{current:{querySelector:()=>form}});}
    return {all,form,fields,submit,controls,gates,focus};
  }
  return {...compiled,doc,flatten,render};
}
function actualAuthBridgeValue(source,name,tag) {
  const ast=ts.createSourceFile("auth-props.tsx",source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),found=[];
  function visit(n) {
    if((ts.isJsxOpeningElement(n)||ts.isJsxSelfClosingElement(n))&&n.tagName.getText(ast)===tag) {
      const attr=n.attributes.properties.find(p=>ts.isJsxAttribute(p)&&p.name.text==="loginAuth");
      if(attr?.initializer&&ts.isJsxExpression(attr.initializer))found.push(attr.initializer.expression.getText(ast));
    }
    ts.forEachChild(n,visit);
  }
  visit(ast);assert.equal(found.length,1,"one actual Auth prop bridge: "+tag);
  return new Function(name,"return "+ts.transpileModule("("+found[0]+")",{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText.replace(/;\s*$/,"")+";");
}
test("actual Page Shell Overlay bridge opens registration password and SafeKey controls with 8 4 and 36 source fields",()=>{
  const dom=actualAuthDom(),page=authPage();
  const pageBridge=actualAuthBridgeValue(pageSource,"loginAuthControls","OriginalClientShell");
  const shellSource=readFileSync(new URL("../app/original-client-shell.tsx",import.meta.url),"utf8");
  const shellBridge=actualAuthBridgeValue(shellSource,"loginAuth","LoginOverlay");
  const controls=page.controls();assert.equal(shellBridge(pageBridge(controls)),controls);
  for(const surface of ["registration","changePassword","safeKey"]) {
    const next=authPage(),buttons=dom.flatten({type:"root",props:{},children:dom.actualOpenButtons(next.controls(),false,(_k,_a,f)=>f)});
    const button=buttons.filter(n=>n.type==="SpriteButton")[["registration","changePassword","safeKey"].indexOf(surface)];
    assert.equal(button.props.disabled,false);button.props.onClick();
    assert.equal(next.scope.loginAuthRef.current.surface,surface);
    const view=dom.render(next);assert.equal(view.form.props.role,"dialog");assert.equal(view.form.props.autoComplete,"off");
    assert.equal(view.fields.length,surface==="registration"?8:surface==="changePassword"?4:2);
    if(surface==="safeKey") {
      const keys=view.all.filter(n=>n.props["data-login-safe-key"]);
      assert.deepEqual(keys.map(n=>n.props["data-login-safe-key"]),Array.from(next.scope.loginAuthRef.current.safeKeys));
      assert.equal(keys.length,36);keys[0].props.onClick();assert.equal(next.scope.accountIdRef.current,"SharedEdit");
      assert.deepEqual(next.abi.calls.at(-1),["edit","Existing","A",true,false]);
      const deleteButton=view.all.find(n=>n.type==="button"&&n.children.includes("Delete"));deleteButton.props.onClick();
      assert.deepEqual(next.abi.calls.at(-1),["edit","SharedEdit","",true,true]);
      const randomButton=view.all.find(n=>n.type==="button"&&n.children.includes("Random"));randomButton.props.onClick();
      assert.equal(next.scope.loginAuthRef.current.safeKeys,"9876543210ZYXWVUTSRQPONMLKJIHGFEDCBA");
      view.fields[1].focus();assert.equal(next.scope.loginAuthRef.current.safeFocus,"password");
      const fresh=dom.render(next);fresh.form.props.onSubmit({preventDefault(){}});
      assert.equal(next.sent.at(-1).type,"login");
    } else {
      const draft=surface==="registration"?registrationDraft():{accountId:"User",oldPassword:"Old",newPassword:"New",confirmPassword:"New"};
      for(const field of view.fields)field.props.onChange({target:{value:draft[field.props.name]}});
      assert.deepEqual(next.scope.loginAuthRef.current[surface==="registration"?"registration":"changePassword"],draft);
    }
    const account=view.fields.find(n=>n.props.name==="accountId");assert.equal(account.props.maxLength,24);
    for(const field of view.fields.filter(n=>n.props.type==="password"))assert.equal(field.props.maxLength,32);
  }
});
test("actual auth DOM field Enter advances to Submit without sending and pending normal login ignores optional ready",()=>{
  const dom=actualAuthDom();
  for(const surface of ["registration","changePassword"]) {
    const page=authPage(surface);if(surface==="registration")fillRegistration(page);else fillPassword(page);
    const view=dom.render(page);
    for(let i=0;i<view.fields.length;i++) {
      let prevented=0;
      view.form.props.onKeyDown({key:"Enter",nativeEvent:{isComposing:false},currentTarget:view.form,target:view.fields[i],
        preventDefault(){prevented++;},stopPropagation(){}});
      assert.equal(prevented,1);assert.equal(dom.doc.activeElement,view.fields[i+1]??view.submit);assert.equal(page.sent.length,0);
    }
    view.form.props.onSubmit({preventDefault(){}});
    assert.equal(page.sent.at(-1).type,surface==="registration"?"newAccount":"changePassword");
    const pending=dom.render(page);assert(pending.fields.every(n=>n.disabled));assert.equal(pending.submit.disabled,true);
    pending.form.props.onSubmit({preventDefault(){}});assert.equal(page.sent.length,2);
    pending.form.props.onKeyDown({key:"Escape",preventDefault(){},stopPropagation(){}});
    assert.equal(page.scope.loginAuthRef.current.surface,"login");
  }
  const page=authPage("login",false),gate=dom.actualNormalGate(page.controls(),false,()=>page.submitLogin());
  assert.equal(gate.authBlocked,true);assert.equal(gate.loginBlocked,false);
  gate.onSubmit({preventDefault(){}});assert.equal(page.sent.at(-1).type,"login");
  const blocked=dom.actualNormalGate(page.controls(),true,()=>assert.fail("duplicate normal login"));
  assert.equal(blocked.loginBlocked,true);blocked.onSubmit({preventDefault(){}});
  for(const onEnter of blocked.onEnter)onEnter({key:"Enter",preventDefault(){}});
  assert.equal(page.sent.length,2);
});

test("actual physical socket-close Auth retirement clears queued secrets and epochs without clearing entered lane",()=>{
  const page=authPage("registration");fillRegistration(page);page.controls().submitRegistration();
  const oldEpoch=page.scope.loginAuthRef.current.epoch,oldIntent=page.scope.oauthIntentRef.current;
  page.scope.pendingGatewayProtocolActionRef.current={kind:"auth",attempt:{command:{type:"login",password:"QueuedSecret"}}};
  const oldLink=page.scope.identityLinkIntentRef.current;
  page.applyAuthSocketClose({});assert.equal(page.scope.socketRef.current,page.socket);
  page.applyAuthSocketClose(page.socket);
  assert.equal(page.scope.socketRef.current,null);
  assert.equal(page.scope.pendingGatewayProtocolActionRef.current,null);
  assert.equal(page.scope.loginAuthRef.current.epoch,oldEpoch+1);
  assert.equal(page.scope.loginAuthRef.current.surface,"login");
  assert.equal(page.scope.oauthIntentRef.current,oldIntent+1);
  assert.equal(page.scope.identityLinkIntentRef.current,oldLink+1);
  assert.equal(page.scope.gatewayProtocolReadyRef.current,false);
  assert.equal(page.scope.loginBusyRef.current,false);
  assert.equal(page.preauthGate().pending(page.socket),true);assert.equal(page.preauthGate().used(page.socket),true);
  const retired=page.scope.loginAuthRef.current;
  page.receive("NewAccount",{result:8},page.scope.equipmentConnectionGenerationRef.current,page.socket);
  assert.equal(page.scope.loginAuthRef.current,retired);
});
