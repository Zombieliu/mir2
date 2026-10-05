import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

// Strictly pure source dependencies; no WASM/runtime loader or filesystem fixture writes.
const sources = { identity: "../lib/world-model/item-identity.ts",
  equipment: "../lib/equipment-gateway-adapter.ts", parcel: "../lib/mail-parcel-gateway-adapter.ts",
  storage: "../lib/storage-gateway-adapter.ts" };
const allow = { identity: {}, equipment: { "./world-model/item-identity": "identity" },
  parcel: { "./equipment-gateway-adapter": "equipment" },
  storage: { "./equipment-gateway-adapter": "equipment", "./world-model/item-identity": "identity",
    "./mail-parcel-gateway-adapter": "parcel" } };
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
  name: "same display name", quantity: 1, tooltipSource: { info: { item_type: 3 } } });
function world() {
  return { inventoryCapacity: 54, maxBagSlots: 48, storageSize: 160, hasExpandedStorage: true,
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
  "itemCommandRequiresOwner", "retireEquipmentSession", "mailParcelItemsIdle", "endStorageService", "retireNpcShopService"];
const declarations = new Map();
const ackCases = [];
function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name && names.includes(node.name.text)) {
    assert(!declarations.has(node.name.text), "ambiguous Page declaration: " + node.name.text);
    declarations.set(node.name.text, node.getText(pageAst));
  }
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression)
    && node.expression.text === "TakeBackItemV2") ackCases.push(node);
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
  + ackCase.getText(pageAst) + "\n} }\n";
const pageJavaScript = ts.transpileModule(actualFunctions, {
  fileName: "actual-storage-page-gates.ts",
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;
const pureIdentity = loadPure("identity");
const pureEquipment = loadPure("equipment");
const pureParcel = loadPure("parcel");
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
    ...adapter,
    authoritativeItemUniqueId: pureIdentity.authoritativeItemUniqueId,
    currentEquipmentCommandItem: pureEquipment.currentEquipmentCommandItem,
    classifyEquipmentUse: pureEquipment.classifyEquipmentUse,
    equipmentGatewayOperation: pureEquipment.equipmentGatewayOperation,
    mailMutationAllowed: pureParcel.mailMutationAllowed,
    isMailItemMutation: pureParcel.isMailItemMutation,
    worldRef: { current: world() }, socketRef: { current: socket },
    pendingStorageRequestsRef: pendingRef, storageRequestSequenceRef: sequenceRef,
    storageUiIngressRef: { current: null }, storageServiceActiveRef: { current: false }, storageCompatibilityRef: { current: false },
    setStorageServiceActive: () => {}, setStorageCompatibility: () => {}, setStoragePasswordOpenVersion: () => {}, setStorageServiceOpenVersion: () => {},
    npcBuyDispatcherRef: { current: { withdraw: () => npcWithdrawals.push("withdraw") } },
    npcGoldBuyInventoryRef: { current: { invalidate: () => npcInventoryRetirements.push("invalidate") } },
    npcShopClockRef: { current: { service: 3, catalog: 4 } }, npcBuySelectedRef: { current: 77 },
    npcShopServiceRef: { current: { supportsBuy: true } }, setNpcShopService: () => {},
    equipmentRenderOwnerToken: { ...fixtureOwner }, equipmentHostSuspendReasonRef: { current: null },
    equipmentConnectionGenerationRef: { current: identity.connectionGeneration }, equipmentSessionGenerationRef: { current: identity.sessionGeneration },
    equipmentStartGameRef: { current: { ...identity } },
    equipmentSnapshotRef: { current: { ...identity, snapshot: {} } },
    equipmentControllerRef: { current: {
      status: () => ({ ...identity, pending: 0 }),
      hasPendingInstance: () => ({ ok: true, reserved: false }),
      terminateSession: identity => terminations.push(identity),
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
    sameMailOwner: () => false, currentSpellsOwner: () => null,
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
  const keys = Object.keys(scope);
  const functions = new Function(...keys, pageJavaScript + "\nreturn {"
    + names.join(",") + ",applyStorageAck};")(...keys.map(key => scope[key]));
  return { ...functions, scope, sent, errors, logs, terminations, npcWithdrawals, npcInventoryRetirements, socket, liveOwner,
    get attempts() { return attempts; }, get actions() { return actions; },
    onAction(value) { listener = value; }, throwAtSocket() { socketThrows = true; },
    get pending() { return scope.pendingStorageRequestsRef.current; },
    deposit(slot = 4, index = 1) { return functions.storeItem(selection(scope.worldRef.current.inventoryItems[index]), slot); },
    withdraw(slot = 7, container = "bag2") { return functions.takeBackItem(selection(scope.worldRef.current.storageItems[0]), slot, container); },
  };
}
function ack(command, patch = {}, packet = command.type === "storeItemV2" ? "StoreItemV2" : "TakeBackItemV2") {
  return { packet, payload: { requestId: command.requestId, from: command.from, to: command.to, success: true, ...patch } };
}

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
  const mailOwner = { marker: "controlled-current-mail-owner" };
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
    sameMailOwner: (left, right) => left === right, currentSpellsOwner: () => mailOwner,
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
