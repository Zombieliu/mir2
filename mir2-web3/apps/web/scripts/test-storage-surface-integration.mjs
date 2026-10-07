import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

// Compile only the explicit pure TypeScript dependency graph, never a renderer/Core module.
const cache = new Map();
const pureModules = new Set(["world-model/item-identity", "bevy-bag-model", "bevy-storage-model", "bevy-bag-ui", "bevy-storage-ui", "equipment-gateway-adapter", "mail-parcel-gateway-adapter", "storage-gateway-adapter", "social-window-operations", "social-incoming-replies", "storage-rental-confirmation", "bag-belt-gesture", "npc-purchase-client", "bag-belt-move-dispatcher", "npc-repair-service"]);
function load(name, fresh = false) {
  assert.ok(pureModules.has(name), "unexpected non-pure import: " + name);
  if (!fresh && cache.has(name)) return cache.get(name);
  const source = readFileSync(new URL("../lib/" + name + ".ts", import.meta.url), "utf8");
  const javascript = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const module = { exports: {} };
  new Function("exports", "module", "require", javascript)(module.exports, module,
    request => load(request.replace(/^\.\//, "")));
  if (!fresh) cache.set(name, module.exports);
  return module.exports;
}
const storage = load("bevy-storage-ui");
const projection = load("bevy-storage-model");
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false };
const capabilities = { schemaVersion: 1, storageUiAbiVersion: 1, storageIntentAbiVersion: 1, compiled: true, startup: true };
const worldItem = (container, slot, uniqueId) => ({ container, slot, authoritativeUniqueId: uniqueId,
  key: "sword", name: "WoodenSword", quantity: 1, icon: 2, description: "Exact item" });
function world() {
  return { inventoryCapacity: 54, maxBagSlots: 48, gold: 200,
    inventoryItems: [worldItem("bag2", 4, 0)], beltItems: [], equipmentItems: [],
    storageItems: [worldItem("storage", 3, 7)], storageSize: 160,
    hasStoragePassword: false, storageSessionUnlocked: true, hasExpandedStorage: false,
    expandedStorageExpiryTimeBinaryDatetime: 1000 };
}
function fixture(overrides = {}) {
  let now = 100, status = null, sink = null, state = null, host, behavior = () => false;
  const projected = projection.projectBevyStorageModel(world());
  assert.equal(projected.ok, true);
  let input = { connectionGeneration: 3, sessionGeneration: 5, ownerRevision: 0,
    eligible: true, open: true, serviceRevision: 11, presentation,
    inventory: projected.inventory, storage: projected.storage, player: { level: 1 },
    blockedUniqueIds: [], pendingCells: [], language: "en" };
  const snapshots = [], owners = [], intents = [], edges = [], withdrawals = [];
  const runtime = {
    getMir2StorageUiCapabilities: () => JSON.stringify(capabilities),
    setMir2StorageUiSnapshot(json) { snapshots.push(JSON.parse(json)); return true; },
    getMir2StorageUiStatus: () => JSON.stringify(status),
    setMir2StorageUiIntentSink(callback) { sink = callback; },
    clearMir2StorageUiIntentSink() { sink = null; },
    setMir2StorageUiPointerEdge(json) { edges.push(JSON.parse(json)); return true; },
    withdrawMir2StorageUiSnapshot(json) { withdrawals.push(JSON.parse(json)); return true; },
    ...overrides,
  };
  function replace(lib = storage) {
    return host = new lib.BevyStorageHost({ runtime, read: () => input, now: () => now,
      onOwner(owner, runGeneration, ownerRevision) {
        owners.push({ owner, runGeneration, ownerRevision }); input.ownerRevision = ownerRevision;
      },
      onState(next) { state = next; },
      onIntent(intent) { intents.push(intent); return behavior(intent); },
    });
  }
  replace();
  function echo(extra = {}) {
    const s = snapshots.at(-1);
    assert.ok(s, "a memory renderer may only apply an actually published snapshot");
    status = { runGeneration: s.runGeneration, connectionGeneration: s.connectionGeneration,
      sessionGeneration: s.sessionGeneration, ownerRevision: s.ownerRevision,
      frame: (status?.frame ?? 0) + 1, ready: true, inputEnabled: s.inputEnabled,
      appliedRevision: s.revision, appliedModelRevision: s.modelRevision,
      appliedPresentationRevision: s.presentationRevision, appliedServiceRevision: s.serviceRevision,
      inputRegions: [{ left: 100, top: 100, width: 700, height: 400 }], error: null, ...extra };
  }
  function activate() {
    host.tick(); echo(); host.tick(); echo(); host.tick();
    assert.equal(state.active, true); assert.equal(state.transitioning, false);
  }
  function intent(extra = {}) {
    const s = snapshots.at(-1);
    const value = { runGeneration: s.runGeneration, connectionGeneration: s.connectionGeneration,
      sessionGeneration: s.sessionGeneration, ownerRevision: s.ownerRevision,
      modelRevision: s.modelRevision, presentationRevision: s.presentationRevision,
      serviceRevision: s.serviceRevision, intentSequence: 1, type: "storeItem",
      source: { container: 0, slot: 44, uniqueId: 0 }, target: { container: 4, slot: 10 }, ...extra };
    if (["close", "password", "rent"].includes(value.type)) { delete value.source; delete value.target; }
    return value;
  }
  return { runtime, snapshots, owners, intents, edges, withdrawals, echo, activate, intent, replace,
    get host() { return host; }, get input() { return input; }, set input(value) { input = value; },
    get state() { return state; }, get sink() { return sink; },
    set behavior(callback) { behavior = callback; }, time(ms) { now += ms; },
    emit(value) { assert.equal(typeof sink, "function"); return sink(JSON.stringify(value)); },
  };
}

// Actual function bodies are extracted by TS AST, with only in-memory boundary dependencies.
function sourceFile(relative) {
  const text = readFileSync(new URL("../" + relative, import.meta.url), "utf8");
  return ts.createSourceFile(relative, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
}
function functions(relative, names, scope) {
  const ast = sourceFile(relative), found = new Map();
  function visit(n) {
    if (ts.isFunctionDeclaration(n) && names.includes(n.name?.text)) {
      assert.equal(found.has(n.name.text), false, "ambiguous source function " + n.name.text);
      // Execute selected declarations as a script, preserving their original
      // function bodies. Module-only export/default tokens cannot enter Function.
      let declaration = n.getText(ast);
      const start = n.getStart(ast);
      const moduleModifiers = (n.modifiers ?? []).filter(modifier =>
        modifier.kind === ts.SyntaxKind.ExportKeyword || modifier.kind === ts.SyntaxKind.DefaultKeyword);
      for (const modifier of [...moduleModifiers].sort((a, b) => b.getStart(ast) - a.getStart(ast))) {
        declaration = declaration.slice(0, modifier.getStart(ast) - start)
          + declaration.slice(modifier.getEnd() - start);
      }
      found.set(n.name.text, declaration);
    }
    ts.forEachChild(n, visit);
  }
  visit(ast); assert.equal(found.size, names.length, "all actual source functions must exist");
  const js = ts.transpileModule(names.map(n => found.get(n)).join("\n"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), js + ";return {" + names.join(",") + "};")(...Object.values(scope));
}
function effect(relative, hookName, marker, scope) {
  const ast = sourceFile(relative), found = [];
  function visit(n) {
    if (ts.isCallExpression(n) && n.expression.getText(ast) === hookName
      && n.arguments[0]?.getText(ast).includes(marker)) found.push(n.arguments[0]);
    ts.forEachChild(n, visit);
  }
  visit(ast); assert.equal(found.length, 1, "select one actual effect: " + marker);
  const js = ts.transpileModule("const actualEffect = " + found[0].getText(ast) + ";", {
    compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), js + ";return actualEffect;")(...Object.values(scope));
}
const bag = load("bevy-bag-ui"), identity = load("world-model/item-identity");
const storageTransport = load("storage-gateway-adapter"), parcelTransport = load("mail-parcel-gateway-adapter");
function pageFixture() {
  const f = fixture(); f.activate();
  let listener = null, sendError = null; const sent = [], values = {}, logs = [], yields = [];
  const owner = f.owners.at(-1);
  const scope = {
    heroOperationsRef: {current:{pending:null}}, mailCollectBarrierRef: {current:null},
    npcPearlSendEpochRef: { current: 0 },
    bagBeltMovesRef: { current: new (load("bag-belt-move-dispatcher").BagBeltMoveDispatcher)() },
    bagBeltInventoryReadyRef: { current: null },
    npcRepairAuthorityRef: { current: new (load("npc-repair-service").NpcRepairService)() },
    worldFishingActiveGestureRef: { current: null }, worldFishingQueuedRef: { current: null },
    worldFishingGestureRegistryRef: { current: new Map() },
    worldRef: { current: { ...world(), connected: true, playerObjectId: "3", mapFileName: "D001", requireStoragePassword: false } },
    equipmentBagOwnerRef: { current: { ...owner, connectionGeneration: 3, sessionGeneration: 5 } },
    equipmentConnectionGenerationRef: { current: 3 }, equipmentSessionGenerationRef: { current: 5 },
    equipmentHostSuspendReasonRef: { current: null }, equipmentControllerRef: { current: null },
    equipmentStartGameRef: { current: { connectionGeneration: 3, sessionGeneration: 5 } },
    equipmentSnapshotRef: { current: { connectionGeneration: 3, sessionGeneration: 5 } },
    socialSceneRevisionRef: { current: 1 }, screenRef: { current: "game" }, document: { visibilityState: "visible" },
    socialItemOperationsRef: { current: new (load("social-window-operations").SocialWindowOperations)() },
    storageRentalRef: { current: new (load("storage-rental-confirmation").StorageRentalConfirmation)() },
    setStorageRentalPrompt(value) { values.rentalPrompt = typeof value === "function" ? value(values.rentalPrompt ?? null) : value; },
    equipmentRenderOwnerToken: { ...owner, connectionGeneration: 3, sessionGeneration: 5 },
    bevyCharacterSendGateRef: { current: () => false }, bevyBagSendGateRef: { current: () => false },
    storageUiIngressRef: { current: f.host }, storageServiceActiveRef: { current: true },
    storageCompatibilityRef: { current: false }, storageServiceRevisionRef: { current: 11 },
    bagOpenRef: { current: true }, bagCompatibilityModeRef: { current: null },
    bagYieldRef: { current: () => yields.push("yield") },
    storageRequestSequenceRef: { current: 1 }, pendingStorageRequestsRef: { current: new Map() },
    combatIngressRef: { current: null }, mailDispatcherRef: { current: null }, mailIngressRef: { current: null },
    mailParcelRef: { current: null }, bevySpellsSendGateRef: { current: () => false },
    socketRef: { current: { readyState: 1, send(json) { if (sendError) throw sendError; sent.push(JSON.parse(json)); } } },
    WebSocket: { OPEN: 1 }, CustomEvent: class { constructor(type, init) { this.type = type; this.detail = init.detail; } },
    window: { dispatchEvent(event) { listener?.(event); return true; } }, lastCommandRef: { current: null },
    isSpectatorBrowserMode: () => false, isMovementConsoleCommand: () => false,
    recordDebugEvent() {}, appendLog(...args) { logs.push(args); }, t: (key, args, fallback) => fallback ?? key,
    rejectEquipmentCommand(reason) { logs.push(reason); return false; }, console: { error(...args) { logs.push(args); } },
    sameBagOwner: bag.sameBagOwner, ...identity, ...storageTransport, ...parcelTransport,
    setStorageServiceActive: value => { values.active = value; },
    setStorageCompatibility: value => { values.compatibility = value; },
    setStoragePasswordOpenVersion: value => { values.passwordVersion = value; },
    setStorageServiceOpenVersion: value => { values.serviceVersion = value; },
    setBagCompatibilityMode: value => { values.bagCompatibility = value; },
  };
  Object.assign(scope, functions("lib/client-login-runtime.ts", ["preauthCommandKind", "isSensitiveGatewayCommand"], scope));
  const names = ["currentEquipmentOwner", "itemCommandRequiresOwner", "send", "sendRaw", "submitStorageTransfer",
    "endStorageService", "requestBagCompatibility", "storageIntentMatchesCommand", "dispatchBevyStorageIntent",
    "socialItemMutationAllowed", "parityItemMutationAllowed", "currentSpellsOwner", "currentSocialReplyOwner", "currentStorageRentalFacts",
    "closeStorageRentalUi", "cancelStorageRental", "confirmStorageRental",
    "isMovementCommand", "isCombatResolutionCommand", "isMovementPredictionBlockingCommand",
    "retireWorldFishingGesture", "cancelWorldFishingGesture"];
  const api = functions("app/page.tsx", names, scope);
  f.behavior = api.dispatchBevyStorageIntent;
  return { f, scope, api, sent, values, logs, yields,
    set listener(cb) { listener = cb; }, set sendError(error) { sendError = error; },
    reactivate() {
      f.activate(); const current = f.owners.at(-1);
      scope.equipmentBagOwnerRef.current = { ...current, connectionGeneration: 3, sessionGeneration: 5 };
    } };
}

test("actual Page close returns ownership to normal Bag without closing the bag or sending a command", () => {
  const p = pageFixture(), i = p.f.intent({ type: "close" });
  assert.equal(p.f.emit(i), true);
  assert.equal(p.scope.storageServiceActiveRef.current, false);
  assert.equal(p.scope.storageCompatibilityRef.current, false);
  assert.equal(p.scope.bagOpenRef.current, true);
  assert.equal(p.scope.bagCompatibilityModeRef.current, null);
  assert.equal(p.values.bagCompatibility, null);
  assert.equal(p.values.serviceVersion, 0); assert.equal(p.values.passwordVersion, 0);
  assert.equal(p.f.state.active, false); assert.equal(p.f.host.pointerContext(), null);
  assert.equal(p.f.emit(i), false); assert.equal(p.sent.length, 0);
});

test("actual Page password handoff invokes the existing set/change/unlock panel once per service", () => {
  for (const [hasPassword, locked, expected] of [[false, false, "set"], [true, false, "change"], [true, true, "unlock"]]) {
    const p = pageFixture(), i = p.f.intent({ type: "password" });
    assert.equal(p.f.emit(i), true); assert.equal(p.f.emit(i), false);
    assert.equal(p.scope.storageCompatibilityRef.current, true); assert.equal(p.values.passwordVersion, 11);
    assert.equal(p.scope.bagCompatibilityModeRef.current, "fullInventory");
    assert.deepEqual(p.yields, ["yield"]); assert.equal(p.sent.length, 0); assert.equal(p.f.state.active, false);
    const panel = { world: { hasStoragePassword: hasPassword }, storageLocked: locked,
      storagePasswordOpenVersion: p.values.passwordVersion, consumedStoragePasswordVersionRef: { current: 0 } };
    const changes = []; let opens = 0;
    for (const name of ["StoragePassword", "NewStoragePassword", "ConfirmStoragePassword", "StoragePasswordPanelMode"])
      panel["set" + name] = value => changes.push([name, value]);
    panel.setShowStoragePasswordPanel = value => { if (value) opens++; };
    Object.assign(panel, functions("app/components/original-client-inventory-window.tsx", ["openStoragePasswordPanel"], panel));
    const handoff = effect("app/components/original-client-inventory-window.tsx", "useEffect", "consumedStoragePasswordVersionRef.current ===", panel);
    handoff(); handoff();
    assert.equal(opens, 1); assert.equal(panel.consumedStoragePasswordVersionRef.current, 11);
    assert.deepEqual(changes, [["StoragePassword", ""], ["NewStoragePassword", ""],
      ["ConfirmStoragePassword", ""], ["StoragePasswordPanelMode", expected]]);
    const cleared = [];
    const closeScope = { onCloseStorage: jsxHandler("app/page.tsx", "onCloseStorage", { endStorageService: p.api.endStorageService, bagCompatibilityModeRef: p.scope.bagCompatibilityModeRef, setBagCompatibilityMode: p.scope.setBagCompatibilityMode }), activeTab: "bag1", onTabChange() { assert.fail("no unrelated tab change"); } };
    for (const name of ["StorageMode", "PendingMoveItem", "PendingSplitItem", "ShowStoragePasswordPanel"])
      closeScope["set" + name] = value => cleared.push([name, value]);
    functions("app/components/original-client-inventory-window.tsx", ["closeStorageWindow"], closeScope).closeStorageWindow();
    assert.equal(p.scope.storageServiceActiveRef.current, false);
    assert.equal(p.scope.bagOpenRef.current, true);
    assert.equal(p.scope.bagCompatibilityModeRef.current, null);
    assert.deepEqual(cleared, [["StorageMode", null], ["PendingMoveItem", null], ["PendingSplitItem", null], ["ShowStoragePasswordPanel", false]]);
  }
});

test("actual Page rent confirms exactly one existing command after handoff; retired owner sends none", () => {
  const p = pageFixture(), i = p.f.intent({ type: "rent" });
  p.scope.worldRef.current.gold = 1_000_000;
  assert.equal(p.f.emit(i), true); assert.equal(p.f.emit(i), false);
  assert.deepEqual(p.sent, [], "opening the confirmation cannot send the rental command");
  assert.equal(p.scope.storageCompatibilityRef.current, true); assert.equal(p.f.state.active, false);
  assert.equal(p.f.host.claim(i), false);
  const id = p.values.rentalPrompt.id;
  p.api.confirmStorageRental(id); p.api.confirmStorageRental(id);
  assert.deepEqual(p.sent, [{ type: "chat", message: "@ADDSTORAGE" }]);
  assert.equal(p.values.rentalPrompt, null); assert.equal(p.scope.storageCompatibilityRef.current, false);
  assert.equal(p.scope.bagCompatibilityModeRef.current, null);
  const retired = pageFixture(); retired.scope.worldRef.current.gold = 1_000_000;
  assert.equal(retired.f.emit(retired.f.intent({ type: "rent" })), true);
  retired.listener = () => retired.api.endStorageService();
  retired.api.confirmStorageRental(retired.values.rentalPrompt.id);
  assert.equal(retired.sent.length, 0); assert.equal(retired.scope.storageCompatibilityRef.current, false);
  assert.equal(retired.scope.storageServiceActiveRef.current, false); assert.equal(retired.values.rentalPrompt, null);
  assert.equal(retired.f.emit(retired.f.intent({ type: "rent" })), false, "retired storage owner cannot open or send another rental");
  const ownerChanged = pageFixture(); ownerChanged.scope.worldRef.current.gold = 1_000_000;
  assert.equal(ownerChanged.f.emit(ownerChanged.f.intent({ type: "rent" })), true);
  const ownerPrompt = ownerChanged.values.rentalPrompt.id;
  ownerChanged.listener = () => { ownerChanged.scope.equipmentSessionGenerationRef.current++; };
  ownerChanged.api.confirmStorageRental(ownerPrompt);
  assert.equal(ownerChanged.sent.length, 0, "the final rental claim must reject the listener's replacement owner");
  ownerChanged.api.cancelStorageRental(ownerPrompt);
  assert.equal(ownerChanged.values.rentalPrompt, null); assert.equal(ownerChanged.scope.storageCompatibilityRef.current, false);
});

test("actual Page source Bag2 mapping and its own reserved empty target pass the final fence once", () => {
  const p = pageFixture(); let during;
  p.listener = () => {
    during = [...p.scope.pendingStorageRequestsRef.current.values()]; assert.equal(during.length, 1);
    assert.equal(during[0].enteredSocket, false);
    assert.equal(during[0].proof.source.container, "bag2"); assert.equal(during[0].proof.source.slot, 4);
    assert.equal(during[0].proof.target.uniqueId, null);
    p.f.input.blockedUniqueIds = [0]; p.f.input.pendingCells = [{ container: 0, slot: 44 }, { container: 4, slot: 10 }];
  };
  const i = p.f.intent(); assert.equal(p.f.emit(i), true);
  assert.deepEqual(p.sent, [{ type: "storeItemV2", requestId: "st-0000000000000001", from: 44, to: 10 }]);
  assert.equal(during[0].enteredSocket, true);
  assert.equal(p.scope.pendingStorageRequestsRef.current.size, 1);
  assert.equal(p.scope.storageRequestSequenceRef.current, 2);
  assert.equal(p.f.host.claim(i), false); assert.equal(p.f.emit(i), false);
});

test("actual Page final listener owner retirement burns the unsent request ID and clears only its reservation", () => {
  const p = pageFixture(); let id;
  p.listener = () => { id = [...p.scope.pendingStorageRequestsRef.current.keys()][0]; p.f.host.withdraw(); };
  assert.equal(p.f.emit(p.f.intent()), false); assert.equal(id, "st-0000000000000001");
  assert.equal(p.sent.length, 0); assert.equal(p.scope.pendingStorageRequestsRef.current.size, 0);
  assert.equal(p.scope.storageRequestSequenceRef.current, 2);
  p.listener = null; p.reactivate();
  assert.equal(p.f.emit(p.f.intent({ intentSequence: 2 })), true);
  assert.equal(p.sent[0].requestId, "st-0000000000000002");
});

test("actual Page replacement source/target or edited DTO after mir2:action cannot enter socket", () => {
  for (const change of [
    p => { p.scope.worldRef.current.inventoryItems = [worldItem("bag2", 4, 99)]; },
    p => { p.scope.worldRef.current.storageItems.push(worldItem("storage", 10, 99)); },
    p => { p.scope.lastCommandRef.current.to = 11; },
  ]) {
    const p = pageFixture(); p.listener = () => change(p);
    assert.equal(p.f.emit(p.f.intent()), false); assert.equal(p.sent.length, 0);
    assert.equal(p.scope.pendingStorageRequestsRef.current.size, 0);
    assert.equal(p.scope.storageRequestSequenceRef.current, 2);
  }
});

test("actual Page socket throw after irreversible claim retains outcome-unknown reservation", () => {
  const p = pageFixture(), i = p.f.intent(); p.sendError = Error("memory socket outcome unknown");
  assert.equal(p.f.emit(i), false); assert.equal(p.sent.length, 0);
  const reservation = [...p.scope.pendingStorageRequestsRef.current.values()][0];
  assert.ok(reservation); assert.equal(reservation.enteredSocket, true);
  assert.equal(p.scope.storageRequestSequenceRef.current, 2); assert.equal(p.f.host.claim(i), false);
  assert.equal(p.f.emit(i), false);
});

test("actual Storage RPC captured same-clock/same-pointer terminals cannot revoke a newer gesture proof", () => {
  for (const phase of ["cancel", "blur", "up"]) {
    const f = fixture(); f.activate(); const router = new storage.StoragePointerRouter();
    const first = router.down(f.host.pointerContext(), 7, 0, 150, 150);
    assert.equal(f.host.pointer(first), true);
    const oldTerminal = router.cancel(phase === "up" ? "cancel" : phase);
    assert.equal(f.host.pointer(oldTerminal), true);
    const next = router.down(f.host.pointerContext(), 7, 0, 160, 160);
    assert.equal(next.pointerId, first.pointerId); assert.notEqual(next.downSequence, first.downSequence);
    assert.equal(f.host.pointer(next), true);
    const i = f.intent({ intentSequence: 2 }); assert.equal(f.emit(i), false); assert.equal(f.host.allows(i), true);
    assert.equal(f.host.pointer({ ...oldTerminal, phase }), true);
    assert.equal(f.host.allows(i), true, "retired terminal must preserve the newer admitted proof");
    const cancel = router.cancel(); assert.equal(f.host.pointer(cancel), true);
    assert.equal(f.host.allows(i), false, "matching new terminal must still cancel its actual RPC lease");
  }
});

test("actual Storage new Down during final listener revokes the old intent before socket entry", () => {
  const p = pageFixture(), router = new storage.StoragePointerRouter();
  const first = router.down(p.f.host.pointerContext(), 9, 0, 150, 150); assert.equal(p.f.host.pointer(first), true);
  p.listener = () => {
    const old = router.edge("up", 9, 150, 150); assert.equal(p.f.host.pointer(old), true);
    assert.equal(p.f.host.pointer(router.down(p.f.host.pointerContext(), 9, 0, 160, 160)), true);
  };
  assert.equal(p.f.emit(p.f.intent()), false); assert.equal(p.sent.length, 0);
  assert.equal(p.scope.pendingStorageRequestsRef.current.size, 0); assert.equal(p.scope.storageRequestSequenceRef.current, 2);
});

function jsxHandler(relative, name, scope) {
  const ast = sourceFile(relative), found = [];
  function visit(n) {
    if (ts.isJsxAttribute(n) && n.name.getText(ast) === name && ts.isJsxExpression(n.initializer)
      && n.initializer.expression) found.push(n.initializer.expression);
    ts.forEachChild(n, visit);
  }
  visit(ast); assert.equal(found.length, 1, "one actual JSX callback: " + name);
  const js = ts.transpileModule("const handler = " + found[0].getText(ast) + ";", {
    compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), js + ";return handler;")(...Object.values(scope));
}
function hookOption(relative, hook, key, scope) {
  const ast = sourceFile(relative), found = [];
  function visit(n) {
    if (ts.isCallExpression(n) && n.expression.getText(ast) === hook) {
      assert.ok(ts.isObjectLiteralExpression(n.arguments[0]));
      const p = n.arguments[0].properties.find(p => ts.isPropertyAssignment(p) && p.name.getText(ast) === key);
      assert.ok(p); found.push(p.initializer);
    }
    ts.forEachChild(n, visit);
  }
  visit(ast); assert.equal(found.length, 1);
  const js = ts.transpileModule("const value = " + found[0].getText(ast) + ";", {
    compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), js + ";return value;")(...Object.values(scope));
}

test("actual Page service lifecycle requests only Storage until close then requests the normal Bag host", () => {
  const p = pageFixture();
  function requested() {
    const scope = { bevyBagUiRequested: true, bevyQuestUiRequested: true,
      storageServiceActive: p.scope.storageServiceActiveRef.current, storageCompatibility: p.scope.storageCompatibilityRef.current };
    return [hookOption("app/page.tsx", "useBevyBagUi", "requested", scope),
      hookOption("app/page.tsx", "useBevyStorageUi", "requested", scope)];
  }
  assert.deepEqual(requested(), [false, true]);
  assert.equal(p.f.emit(p.f.intent({ type: "close" })), true);
  assert.deepEqual(requested(), [true, false]);
  assert.equal(p.scope.bagCompatibilityModeRef.current, null);
});
function shellStorageFixture() {
  const text = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
  const ast = ts.createSourceFile("shell.tsx", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const names = ["scenePointFromMouseEvent", "beginCombatUiHold", "endCombatUiHold", "cancelSharedStoragePointer", "handleSharedStoragePointer",
    "cancelWorldFishingHeldPointer", "retireWorldFishingPhysical"];
  const declarations = [];
  function visit(node) {
    if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) declarations.push(node.getText(ast));
    ts.forEachChild(node, visit);
  }
  visit(ast); assert.equal(declarations.length, names.length);
  const f = fixture(); f.activate(); let hook = null, stops = 0;
  class Element { constructor() { this.id = "canvas"; } setPointerCapture() {} }
  const holds = new Map(), router = new storage.StoragePointerRouter();
  const scope = { parityUiBlocksGameplay:undefined, onHeroShortcut:undefined, storagePointerRouterRef: { current: router }, combatUiHoldRef: { current: new Map() },
    onCombatUiHeld(channel, token, held) { if (held) holds.set(channel, token); else if (holds.get(channel) === token) holds.delete(channel); },
    storagePointerCallbacksRef: { current: { getBevyStoragePointerContext: () => f.host.pointerContext(),
      onBevyStoragePointer(edge) { const accepted = f.host.pointer(edge); hook?.(edge); return accepted; } } },
    stageFrameRef: { current: { focus() {}, getBoundingClientRect: () => ({ left: 0, top: 0, width: 1024, height: 768 }) } },
    stagePresentation: { virtualWidth: 1024, virtualHeight: 768 }, heldScenePointerRef: { current: null },
    worldFishingPhysicalRef: { current: null }, worldFishingTerminalRef: { current: null },
    onViewportDirectionStop() { stops++; }, stopHeldScenePointer() { scope.heldScenePointerRef.current = null; stops++; },
    sceneInteractionReady: true, questLocalModalOpen: false, mobileMoreOpen: false, bevyQuestUiCapturesPointer: false,
    bevyStorageUiActive: true, bevyStorageUiTransitioning: false,
    bagPointerRouterRef: { current: { held: null } }, characterPointerRouterRef: { current: { held: null } },
    hudPointerRouterRef: { current: { held: null } }, spellsPointerRouterRef: { current: { held: null } },
    HTMLElement: Element, sharedUiCanvasId: () => "canvas", webGl2SharedCanvasPrototype: false,
  };
  const js = ts.transpileModule(declarations.join("\n"), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  const api = new Function(...Object.keys(scope), js + ";return {" + names.join(",") + "};")(...Object.values(scope));
  return { f, scope, api, router, holds, get stops() { return stops; }, set hook(callback) { hook = callback; },
    event(x = 150, y = 150, pointerId = 1) { return { target: new Element(), clientX: x, clientY: y,
      pointerId, pointerType: "mouse", button: 0, preventDefault() {} }; } };
}

test("actual Shell storage routing holds combat only for UI origin and preserves a crossing world gesture", () => {
  const s = shellStorageFixture(); s.api.handleSharedStoragePointer(s.event(), "down");
  assert.equal(s.router.held.origin, "storage"); assert.equal(s.holds.size, 1);
  assert.equal(s.scope.heldScenePointerRef.current, null);
  s.api.handleSharedStoragePointer(s.event(900, 700), "up"); assert.equal(s.holds.size, 0);
  s.api.handleSharedStoragePointer(s.event(900, 700, 2), "down");
  assert.equal(s.router.held.origin, "world"); assert.equal(s.holds.size, 0);
  assert.ok(s.scope.heldScenePointerRef.current);
  s.api.handleSharedStoragePointer(s.event(150, 150, 2), "move");
  assert.equal(s.router.held.origin, "world"); assert.equal(s.scope.heldScenePointerRef.current.sceneX, 150);
  s.api.handleSharedStoragePointer(s.event(150, 150, 2), "up");
  assert.equal(s.router.held, null); assert.equal(s.scope.heldScenePointerRef.current, null);
});

test("actual Shell old cancellation cannot release a synchronously established replacement UI lease/token", () => {
  const s = shellStorageFixture(); s.api.handleSharedStoragePointer(s.event(), "down");
  let freshLease, freshHold;
  s.hook = edge => {
    if (edge.phase !== "cancel") return; s.hook = null; s.f.host.withdraw(); s.f.activate();
    s.api.handleSharedStoragePointer(s.event(160, 160, 2), "down");
    freshLease = s.router.held; freshHold = s.scope.combatUiHoldRef.current.get("storage");
  };
  s.api.cancelSharedStoragePointer();
  assert.equal(s.router.held, freshLease); assert.equal(s.scope.combatUiHoldRef.current.get("storage"), freshHold);
  assert.equal(s.holds.get("storage"), freshHold.token);
});

test("actual Shell old cancel or up cannot clear a synchronously established replacement world hold", () => {
  for (const terminal of ["cancel", "up"]) {
    const s = shellStorageFixture(); s.api.handleSharedStoragePointer(s.event(), "down");
    let freshLease, freshWorld, stopsAfterFresh;
    s.hook = edge => {
      if (edge.phase !== terminal) return; s.hook = null; s.f.host.withdraw(); s.f.activate();
      s.api.handleSharedStoragePointer(s.event(900, 700, 2), "down");
      freshLease = s.router.held; freshWorld = s.scope.heldScenePointerRef.current; stopsAfterFresh = s.stops;
      assert.equal(freshLease.origin, "world"); assert.ok(freshWorld);
    };
    if (terminal === "cancel") s.api.cancelSharedStoragePointer(); else s.api.handleSharedStoragePointer(s.event(), "up");
    assert.equal(s.router.held, freshLease);
    assert.equal(s.scope.heldScenePointerRef.current, freshWorld, "old terminal cannot clear the new world-origin hold");
    assert.equal(s.stops, stopsAfterFresh, "old terminal cannot stop the replacement gesture");
  }
});


test("actual Shell active or arming storage stops held keyboard movement even without a ready pointer context", () => {
  for (const [active, transitioning] of [[true, false], [false, true]]) {
    let stops = 0;
    const scope = { parityUiBlocksGameplay: undefined, bevyStorageUiActive: active, bevyStorageUiTransitioning: transitioning,
      storagePointerCallbacksRef: { current: { getBevyStoragePointerContext: () => null } },
      heldKeyboardMoveKeysRef: { current: new Set(["right"]) }, heldKeyboardRunModeRef: { current: true },
      onViewportDirectionStop: () => { stops++; }, onViewportDirectionIntent() { assert.fail("storage cannot move the world"); } };
    scope.npcShopPointerCallbacksRef = { current: { getBevyNpcShopInputBlocked: () => false } };
    Object.assign(scope, functions("app/original-client-shell.tsx", ["npcShopBlocksWorldInput"], scope));
    functions("app/original-client-shell.tsx", ["dispatchKeyboardMoveInput"], scope).dispatchKeyboardMoveInput("edge");
    assert.equal(scope.heldKeyboardMoveKeysRef.current.size, 0);
    assert.equal(scope.heldKeyboardRunModeRef.current, false); assert.equal(stops, 1);
    scope.screen = "game"; scope.bevyQuestUiCapturesPointer = false;
    effect("app/original-client-shell.tsx", "useEffect", "function handleKeyboardMoveDown", scope)();
    assert.equal(stops, 2, "arming must also avoid installing world keyboard handlers");
  }
});

test("actual Shell keyboard effects include storage transitions in their dependency roster", () => {
  const ast = sourceFile("app/original-client-shell.tsx"), effects = [];
  function visit(n) {
    if (ts.isCallExpression(n) && n.expression.getText(ast) === "useEffect"
      && ["function handleKeyboardMoveDown", "function handleShortcutKey"].some(marker => n.arguments[0]?.getText(ast).includes(marker))) effects.push(n);
    ts.forEachChild(n, visit);
  }
  visit(ast);
  // Movement and combat listeners must be recreated when either storage phase changes.
  assert.equal(effects.length, 2);
  for (const n of effects) {
    assert.ok(ts.isArrayLiteralExpression(n.arguments[1]));
    const deps = n.arguments[1].elements.map(e => e.getText(ast));
    assert.ok(deps.includes("bevyStorageUiActive")); assert.ok(deps.includes("bevyStorageUiTransitioning"));
  }
});

test("actual Shell window blur/resize/unmount withdraw Storage and remove its own terminal listeners", () => {
  const calls = [], listeners = new Map(), documentListeners = new Map();
  const scope = { window: {
    addEventListener(type, fn, capture) { assert.equal(listeners.has(type), false); listeners.set(type, { fn, capture }); },
    removeEventListener(type, fn, capture) { assert.equal(listeners.get(type)?.fn, fn); assert.equal(listeners.get(type)?.capture, capture); listeners.delete(type); },
  }, document: { visibilityState: "visible",
    addEventListener(type, fn, capture) { assert.equal(documentListeners.has(type), false); documentListeners.set(type, { fn, capture }); },
    removeEventListener(type, fn, capture) { assert.equal(documentListeners.get(type)?.fn, fn); assert.equal(documentListeners.get(type)?.capture, capture); documentListeners.delete(type); },
  }, sharedBagPointerHandlerRef: { current: (event, phase) => calls.push(["terminal", phase, event]) },
    cancelSharedStoragePointer: phase => calls.push(["storage", phase ?? "cancel"]),
    heldQuestControlPointersRef: { current: new Set() } };
  scope.npcShopPointerRouterRef = { current: { held: null, cancel: () => null } };
  scope.npcShopPointerCallbacksRef = { current: {} };
  scope.combatUiHoldRef = { current: new Map() };
  scope.heldScenePointerRef = { current: null };
  scope.onCombatUiHeld = undefined;
  scope.bagBeltPointerRef = { current: null }; scope.bagBeltArmedSharedRef = { current: null };
  scope.bagBeltQuarantineRef = { current: new Map() }; scope.bagBeltClickFenceRef = { current: new Map() };
  scope.bagBeltButtonsRef = { current: null }; scope.bagBeltGeometryRef = { current: null };
  scope.bagBeltCallbacksRef = { current: {} };
  scope.npcRepairPointerRef = { current: null }; scope.npcRepairQuarantineRef = { current: new Map() };
  scope.npcRepairClickFenceRef = { current: new Map() };
  scope.worldFishingPhysicalRef = { current: null }; scope.worldFishingTerminalRef = { current: null };
  Object.assign(scope, functions("app/original-client-shell.tsx", ["cancelSharedNpcShopPointer", "cancelBagBeltPointer", "cancelNpcRepairPointer",
    "endCombatUiHold", "cancelWorldFishingHeldPointer", "retireWorldFishingPhysical"], scope));
  for (const name of ["Compose", "Mail", "Spells", "Character", "Bag", "Hud"]) scope["cancelShared" + name + "Pointer"] = () => {};
  const cleanup = effect("app/original-client-shell.tsx", "useEffect", 'cancelSharedStoragePointer("blur")', scope)();
  assert.equal(listeners.size, 8);
  assert.deepEqual([...listeners.keys()], ["pointerdown", "click", "mousedown", "pointerup", "pointercancel", "blur", "resize", "pagehide"]);
  for (const [type, listener] of listeners) assert.equal(listener.capture,
    ["pointerdown", "click", "mousedown", "pointerup", "pointercancel"].includes(type) ? true : undefined, type);
  assert.equal(documentListeners.size, 1); assert.deepEqual([...documentListeners.keys()], ["visibilitychange"]);
  assert.equal(documentListeners.get("visibilitychange").capture, undefined);
  listeners.get("blur").fn(); listeners.get("resize").fn();
  const event = { pointerId: 1 }; listeners.get("pointerup").fn(event);
  assert.deepEqual(calls, [["storage", "blur"], ["storage", "cancel"], ["terminal", "up", event]]);
  cleanup(); assert.equal(listeners.size, 0); assert.equal(documentListeners.size, 0);
  assert.deepEqual(calls.at(-1), ["storage", "cancel"]);
});

test("actual Shell layout owner/geometry replacement cancels Storage before clearing world holds", () => {
  const calls = [], scope = { cancelSharedStoragePointer: () => calls.push("storage"),
    bevyBagOwnerRevision: 0, bevyBagUiActive: false, bevyStorageUiActive: false, bevyStorageUiTransitioning: true,
    heldScenePointerRef: { current: { pointerId: 1 } }, onViewportDirectionStop: () => calls.push("stop") };
  scope.npcShopPointerRouterRef = { current: { held: null, cancel: () => null } };
  scope.npcShopPointerCallbacksRef = { current: {} };
  scope.combatUiHoldRef = { current: new Map() };
  scope.onCombatUiHeld = undefined;
  scope.bagBeltPointerRef = { current: null }; scope.bagBeltArmedSharedRef = { current: null };
  scope.bagBeltQuarantineRef = { current: new Map() };
  scope.npcRepairPointerRef = { current: null }; scope.npcRepairQuarantineRef = { current: new Map() };
  scope.worldFishingPhysicalRef = { current: null }; scope.worldFishingTerminalRef = { current: null };
  Object.assign(scope, functions("app/original-client-shell.tsx", ["cancelSharedNpcShopPointer", "cancelBagBeltPointer", "cancelNpcRepairPointer",
    "endCombatUiHold", "cancelWorldFishingHeldPointer", "retireWorldFishingPhysical"], scope));
  for (const name of ["Compose", "Mail", "Hud", "Character", "Spells", "Bag"]) scope["cancelShared" + name + "Pointer"] = () => calls.push(name);
  effect("app/original-client-shell.tsx", "useLayoutEffect", "if (bevyBagOwnerRevision === 0", scope)();
  assert.equal(calls[0], "storage"); assert.equal(calls.at(-1), "stop"); assert.equal(scope.heldScenePointerRef.current, null);
});

function hookHarness(name) {
  const refs = [], effects = [], state = [], listeners = new Map(); let refCursor = 0, stateCursor = 0, interval = 0;
  const react = {
    useRef(initial) { const i = refCursor++; return refs[i] ??= { current: initial }; },
    useState(initial) { const i = stateCursor++; if (!(i in state)) state[i] = initial;
      return [state[i], value => { state[i] = typeof value === "function" ? value(state[i]) : value; }]; },
    useEffect(callback) { effects.push(callback); },
  };
  const dom = { addEventListener(type, fn) { const set = listeners.get(type) ?? new Set(); set.add(fn); listeners.set(type, set); },
    removeEventListener(type, fn) { listeners.get(type)?.delete(fn); },
    setInterval() { return ++interval; }, clearInterval() {} };
  const text = readFileSync(new URL("../lib/" + name + ".ts", import.meta.url), "utf8");
  const js = ts.transpileModule(text, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const module = { exports: {} };
  new Function("exports", "module", "require", "window", "document", "performance", js)(module.exports, module,
    request => request === "react" ? react : load(request.replace(/^\.\//, "")), dom, dom, { now: () => 100 });
  return { refs, effects, state, listeners,
    render(options) { refCursor = 0; stateCursor = 0; return module.exports[name === "use-bevy-bag-ui" ? "useBevyBagUi" : "useBevyStorageUi"](options); } };
}

test("actual Bag hook first construction publishes its initial owner without a TDZ and retired cleanup cannot rewrite replacement", () => {
  const h = hookHarness("use-bevy-bag-ui"), owners = [], input = {
    connectionGeneration: 3, sessionGeneration: 5, ownerRevision: 0, eligible: false,
    bagOpen: true, page: "bag1", presentation, model: projection.projectBevyStorageModel(world()).inventory,
    player: { level: 1 }, blockedUniqueIds: [],
  };
  const options = { requested: true, runtimeGeneration: 1, runtimeRef: { current: { getMir2BagUiAbiVersion: () => 0 } },
    read: () => input, onOwner(owner, run, revision) { owners.push({ owner, run, revision }); input.ownerRevision = revision; },
    onIntent: () => ({ accepted: false }) };
  h.render(options); let cleanup;
  assert.doesNotThrow(() => { cleanup = h.effects[0](); });
  assert.equal(owners[0].owner, "react"); assert.equal(owners.length, 1); assert.ok(h.refs[1].current);
  h.render({ ...options, runtimeGeneration: 2 }); const cleanupNew = h.effects[1]();
  const replacement = h.refs[1].current, ownerCount = owners.length;
  cleanup(); assert.equal(h.refs[1].current, replacement); assert.equal(owners.length, ownerCount);
  cleanupNew(); assert.equal(h.refs[1].current, null); assert.equal(owners.length, ownerCount);
});

test("actual Storage hook obsolete cleanup leaves the replacement sink/host/owner intact", () => {
  const h = hookHarness("use-bevy-storage-ui"), f = fixture(), owners = [];
  const options = { requested: true, runtimeGeneration: 1, runtimeRef: { current: f.runtime },
    read: () => f.input, onOwner(owner, run, revision) { owners.push({ owner, run, revision }); f.input.ownerRevision = revision; }, onIntent: () => false };
  h.render(options); const cleanup = h.effects[0]();
  h.render({ ...options, runtimeGeneration: 2 }); const cleanupNew = h.effects[1]();
  const replacement = h.refs[1].current, sink = f.sink, ownerCount = owners.length;
  cleanup(); assert.equal(h.refs[1].current, replacement); assert.equal(f.sink, sink); assert.equal(owners.length, ownerCount);
  cleanupNew(); assert.equal(h.refs[1].current, null); assert.equal(f.sink, null); assert.equal(owners.length, ownerCount);
});


test("Source31 storage integration safe expiry preserves custody and wide or unsafe date carriers fail closed", () => {
  const projected = projection.projectBevyStorageModel(world());
  assert.equal(projected.ok, true);
  assert.equal(projected.storage.expiry, 1000);
  assert.equal(storage.validStorageModel(projected.inventory, projected.storage), true);
  const current = fixture(); current.activate();
  assert.equal(current.state.active, true);
  assert.equal(current.snapshots.at(-1).storage.expiry, 1000);
  // The former unsafe Number fixture was invalid exact-source evidence.
  // Preserve the original controller chain with safe input and explicitly
  // reject both exact wide text and numbers whose source may be rounded.
  for (const expiry of ["639028224000000001", "-9223372036854775808", "9223372036854775807",
    639028224000000000, Number("639028224000000001")]) {
    const input = { ...world(), expandedStorageExpiryTimeBinaryDatetime: expiry };
    const rejected = projection.projectBevyStorageModel(input);
    assert.equal(rejected.ok, false, String(expiry));
    assert.deepEqual(rejected.error, { code: "invalidField", field: "storage" });
    assert.equal(input.expandedStorageExpiryTimeBinaryDatetime, expiry);
  }
  assert.equal(current.state.active, true);
  assert.equal(current.snapshots.at(-1).storage.expiry, 1000);
});

test("actual Shell arming shortcut listener swallows combat/item keys before their callbacks", () => {
  for (const [active, transitioning] of [[true, false], [false, true]]) {
    let listener, prevented = 0;
    const scope = { screen: "game", bevyStorageUiActive: active, bevyStorageUiTransitioning: transitioning,
      getKeybindCaptureActive: undefined, parityUiBlocksGameplay: undefined, document: {visibilityState:"visible",hasFocus:()=>true},
      window: { addEventListener(type, callback) { assert.equal(type, "keydown"); listener = callback; },
        removeEventListener(type, callback) { assert.equal(type, "keydown"); assert.equal(callback, listener); listener = null; } },
      onCombatKey() { assert.fail("no combat callback while Storage owns/arms"); },
      onUseItem() { assert.fail("no item callback while Storage owns/arms"); } };
    scope.npcShopPointerCallbacksRef = { current: { getBevyNpcShopInputBlocked: () => false } };
    Object.assign(scope, functions("app/original-client-shell.tsx", ["npcShopBlocksWorldInput"], scope));
    const cleanup = effect("app/original-client-shell.tsx", "useEffect", "function handleShortcutKey", scope)();
    assert.equal(typeof listener, "function");
    for (const key of ["Enter", "F1", "1"]) listener({ key, preventDefault() { prevented++; } });
    assert.equal(prevented, 3); cleanup(); assert.equal(listener, null);
  }
});
