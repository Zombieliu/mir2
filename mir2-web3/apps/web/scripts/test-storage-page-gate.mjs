import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// Strictly pure source dependencies; no WASM/runtime loader or filesystem fixture writes.
const sources = { identity: "../lib/world-model/item-identity.ts",
  equipment: "../lib/equipment-gateway-adapter.ts", parcel: "../lib/mail-parcel-gateway-adapter.ts",
  storage: "../lib/storage-gateway-adapter.ts", social: "../lib/social-incoming-replies.ts",
  operations: "../lib/social-window-operations.ts", rental: "../lib/storage-rental-confirmation.ts",
  bag: "../lib/bevy-bag-model.ts", socialItems: "../lib/social-item-window-model.ts",
  tooltip:"../lib/shared-item-tooltip.ts", guildBuff:"../lib/guild-buff-ui.ts", socialActions: "../lib/social-parity-actions.ts", extended: "../lib/extended-server-packets.ts" };
const allow = { identity: {}, equipment: { "./world-model/item-identity": "identity" },
  parcel: { "./equipment-gateway-adapter": "equipment" },
  storage: { "./equipment-gateway-adapter": "equipment", "./world-model/item-identity": "identity",
    "./mail-parcel-gateway-adapter": "parcel" }, social: {}, operations: {},
  rental: { "./social-incoming-replies": "social" }, bag: { "./world-model/item-identity": "identity" },
  socialItems: { "./bevy-bag-model": "bag", "./world-model/item-identity": "identity" }, tooltip:{}, guildBuff:{}, socialActions: {}, extended: {} };
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
const names = ["submitStorageTransfer", "storeItem", "takeBackItem", "send", "sendRaw",
  "itemCommandRequiresOwner", "retireEquipmentSession", "advanceHeroWindowEpochs", "mailParcelItemsIdle", "endStorageService", "retireNpcShopService",
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
  "TradeItem", "NewItemInfo", "NewRecipeInfo"];
let gatewayGuard, snapshotStatements, snapshotRevision, socialSnapshotFriends, socialWindowRender, rosterRenderInitializer, tradeLeaseInitializer;
function visit(node) {
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
    assert.equal(statements[index - 1].getText(pageAst), "observeEquipmentSnapshot(snapshot, connectionGeneration);");
    snapshotStatements = statements.slice(index - 1, index + 2).map(n => n.getText(pageAst)).join("\n");
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
const actualFunctions = names.map(name => declarations.get(name)).join("\n")
  + '\nfunction applyStorageAck(event) { const payload=event.payload??{}; switch(event.packet) { case "StoreItemV2":\n'
  + ackCase.getText(pageAst) + "\n} }\n"
  + "function applySocialEvent(event, connectionGeneration, source) {" + gatewayGuard
  + "\nconst payload=event.payload??{}; switch(event.packet) {\n" + socialPacketNames.map(n => socialCases.get(n)).join("\n") + "\n}}\n"
  + "function applySocialSnapshot(snapshot, connectionGeneration, source) {" + gatewayGuard
  + "\n" + snapshotRevision + "\n" + snapshotStatements + "\n}"
  + "\nfunction projectSocialSnapshotFriends(snapshot, current, socialOwner) {return " + socialSnapshotFriends + ";}"
  + "\nfunction applySocialWindowRender(showGroup, showBonds, showGuild) {" + socialWindowRender + "}"
  + "\nfunction captureSocialRosterRender(showFriends, showGuild) {return " + rosterRenderInitializer + ";}"
  + "\nfunction captureSocialTradeUiLease() {return " + tradeLeaseInitializer + ";}";
assert.equal(socialCases.size, socialPacketNames.length);
assert.equal(gatewayGuard, "if (connectionGeneration !== equipmentConnectionGenerationRef.current || socketRef.current !== source) return;",
  "retain the actual source socket and connection admission guard");
assert.equal(snapshotRevision, "worldSnapshotVersionRef.current += 1;");
assert.ok(socialSnapshotFriends && socialWindowRender && rosterRenderInitializer && tradeLeaseInitializer);
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
  let listener = null, socketThrows = false, attempts = 0, actions = 0;
  const socket = { readyState: 1, send(body) {
    attempts++; if (socketThrows) throw Error("controlled socket.send outcome unknown");
    sent.push(JSON.parse(body));
  } };
  const fixtureOwner = { ...owner, ...identity };
  const liveOwner = { current: { ...fixtureOwner } };
  const scope = {
    guildBuffAuthorityRef:{current:new (loadPure("guildBuff").GuildBuffAuthority)()},
    heroOperationsRef: {current: {pending:null}}, mailCollectBarrierRef: {current:null},
    observePreferenceRef:{current:null},observeBootstrapRef:{current:null},combatModeRawRef:{current:null},
    heroWindowEpochsRef:{current:{inventory:1,character:1,belt:1}},skillBarPointerHeldRef:{current:false},skillBarDocumentCacheRef:{current:null},
    ...adapter, ...pureSocial, ...pureOperations, ...pureSocialItems, ...pureSocialActions, ...pureExtended,
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
    setMailboxOpen: () => {},
    syncMailParcel: () => { throw Error("unexpected live Mail projection in idle fixture"); },
    sameMailOwner: () => false,
    socialRepliesRef: {current: new pureSocial.SocialIncomingReplies()}, socialReplyWindowsRef: {current: {group: false, bonds: false, guild: false}},
    socialSceneRevisionRef: {current: 1}, renderSocialRequests: () => {},
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
  scope.updateWorld = updater => { scope.worldRef.current = updater(scope.worldRef.current); };
  const keys = Object.keys(scope);
  const functions = new Function(...keys, pageJavaScript + "\nreturn {"
    + names.join(",") + ",applyStorageAck,applySocialEvent,applySocialSnapshot,projectSocialSnapshotFriends,applySocialWindowRender,captureSocialRosterRender,captureSocialTradeUiLease};")(...keys.map(key => scope[key]));
  return { ...functions, scope, sent, errors, logs, terminations, npcWithdrawals, npcInventoryRetirements, socket, liveOwner,
    get attempts() { return attempts; }, get actions() { return actions; },
    onAction(value) { listener = value; }, throwAtSocket() { socketThrows = true; },
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
