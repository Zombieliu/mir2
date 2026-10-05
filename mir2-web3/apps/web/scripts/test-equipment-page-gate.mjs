import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function transpileModule(url, requireLocal = () => { throw new Error("unexpected runtime import"); }) {
  const compiled = ts.transpileModule(readFileSync(url, "utf8"), {
    fileName: fileURLToPath(url),
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  const module = { exports: {} };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, requireLocal);
  return module.exports;
}

const identity = transpileModule(new URL("../lib/world-model/item-identity.ts", import.meta.url));
const bagUi = transpileModule(new URL("../lib/bevy-bag-ui.ts", import.meta.url), () => identity);
const bagModel = transpileModule(new URL("../lib/bevy-bag-model.ts", import.meta.url), () => identity);
const adapter = transpileModule(new URL("../lib/equipment-gateway-adapter.ts", import.meta.url), (id) => {
  assert.equal(id, "./world-model/item-identity");
  return identity;
});
const parcelAdapter = transpileModule(new URL("../lib/mail-parcel-gateway-adapter.ts", import.meta.url), (id) => {
  assert.equal(id, "./equipment-gateway-adapter");
  return adapter;
});
const storageAdapter = transpileModule(new URL("../lib/storage-gateway-adapter.ts", import.meta.url), (id) => {
  if (id === "./world-model/item-identity") return identity;
  if (id === "./equipment-gateway-adapter") return adapter;
  if (id === "./mail-parcel-gateway-adapter") return parcelAdapter;
  throw new Error(`Unexpected storage dependency ${id}`);
});
const mailPackets = transpileModule(new URL("../lib/extended-server-packets.ts", import.meta.url));
const mailUi = transpileModule(new URL("../lib/bevy-mail-ui.ts", import.meta.url), (id) => {
  assert.equal(id, "./extended-server-packets");
  return mailPackets;
});
const { EquipmentSessionController } = transpileModule(new URL("../lib/equipment-session-controller.ts", import.meta.url));

const manifest = JSON.parse(readFileSync(new URL("../lib/generated/client_core_runtime.json", import.meta.url), "utf8"));
const packageRoot = new URL(`../public/client-core/${manifest.version}/`, import.meta.url);
const glue = readFileSync(new URL("mir2_platform_web.js", packageRoot), "utf8");
const binary = readFileSync(new URL("mir2_platform_web_bg.wasm", packageRoot));
const wasm = await import(`data:text/javascript;base64,${Buffer.from(glue).toString("base64")}`);
await wasm.default({ module_or_path: binary });
function actualRuntime() {
  return {
    createEquipmentPendingLedger() {
      const bridge = new wasm.EquipmentPendingBridge();
      const parse = (value) => JSON.parse(value);
      const encode = (value) => JSON.stringify(value);
      return {
        replaceSnapshot: (snapshot) => parse(bridge.replace_snapshot(encode(snapshot))),
        reserve: (operation) => parse(bridge.reserve(encode(operation))),
        acknowledge: (operation, success) => parse(bridge.acknowledge(encode({ operation, success }))),
        releaseUnsent: (operation) => parse(bridge.release_unsent(encode(operation))),
        contains: (operation) => parse(bridge.contains(encode(operation))),
        hasInstance: (uniqueId) => parse(bridge.has_instance(encode({ uniqueId }))),
        status: () => parse(bridge.status()),
      };
    },
  };
}

// Compile the named nested declarations from today's page, so test behavior
// follows the real send gates instead of copying their control flow.
const pageUrl = new URL("../app/page.tsx", import.meta.url);
const pageSource = readFileSync(pageUrl, "utf8");
const syntax = ts.createSourceFile(fileURLToPath(pageUrl), pageSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const names = ["cancelMailParcel","setMailboxOpen","syncEquipmentSnapshot", "suspendEquipmentConnection", "rejectEquipmentCommand", "currentEquipmentOwner", "itemCommandRequiresOwner", "send", "sendRaw", "moveItem", "useItem", "dispatchBevyBagIntent", "dispatchBevyCharacterIntent", "setShowInventory"];
const declarations = new Map();
function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name && names.includes(node.name.text)) {
    declarations.set(node.name.text, node.getText(syntax));
  }
  ts.forEachChild(node, visit);
}
visit(syntax);
assert.deepEqual([...declarations.keys()].sort(), [...names].sort(), "page gate declarations changed");
const pageFunctions = names.map((name) => declarations.get(name)).join("\n");
const pageJavaScript = ts.transpileModule(pageFunctions, {
  fileName: "page-gates.ts",
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;

const session = { connectionGeneration: 3, sessionGeneration: 5 };
const bagItem = (uid, source = { info: { item_type: 0 } }) => ({
  container: "bag1", slot: 4, uniqueId: uid, authoritativeUniqueId: uid, tooltipSource: source,
});
const world = (item) => ({ inventoryItems: [item], beltItems: [], storageItems: [], equipmentItems: [], maxBagSlots: 40 });
const snapshot = (uid) => ({ capacity: 46, placements: [{ uniqueId: uid, container: 0, slot: 4 }] });

function harness({ item = bagItem(4), worldState = world(item), cachedSnapshot = snapshot(item.authoritativeUniqueId),
  controller = null, renderConnection = 3, renderSession = 5,
  renderOwner = { runGeneration: 0, ownerRevision: 0, owner: "react" } } = {}) {
  const sent = [];
  const logs = [];
  const socket = { readyState: 1, send: (payload) => sent.push(JSON.parse(payload)) };
  const socketRef = { current: socket };
  const equipmentControllerRef = { current: controller };
  const equipmentConnectionGenerationRef = { current: 3 };
  const equipmentSessionGenerationRef = { current: 5 };
  const equipmentStartGameRef = { current: session };
  const equipmentSnapshotRef = { current: { ...session, snapshot: cachedSnapshot } };
  const equipmentHostSuspendReasonRef = { current: null };
  const worldRef = { current: worldState };
  let actionListener = null;
  const window = { dispatchEvent(event) { actionListener?.(event); }, __mir2CommandHistory: [] };
  class CustomEvent { constructor(type, options) { this.type = type; this.detail = options.detail; } }
  const spellsWithdrawalsRef = { current: 0 };
  const scope = {
    combatIngressRef:{current:null},mailIngressRef:{current:null},mailDispatcherRef:{current:null},mailRawRef:{current:null},mailOpenRef:{current:false},mailReadAttemptRef:{current:null},mailCompatRef:{current:null},setMailCompatibility:()=>{},setShowMail:()=>{},
    // This adjacent fixture initializes the real existing equipment bridge.
    // Mail is legitimately idle/unavailable here; its adapter gate is actual
    // source, and no controlled response pretends to initialize Mail WASM.
    mailParcelRef:{current:null},currentSpellsOwner:()=>null,sameMailOwner:mailUi.sameMailOwner,
    syncMailParcel:()=>false,mailParcelItemsIdle:()=>{throw new Error("Mail send is outside the idle equipment fixture");},
    mailMutationAllowed:parcelAdapter.mailMutationAllowed,isMailItemMutation:parcelAdapter.isMailItemMutation,
    pendingStorageRequestsRef:{current:new Map()},storageMutationAllowed:storageAdapter.storageMutationAllowed,
    storageTransferStillCurrent:storageAdapter.storageTransferStillCurrent,
    spellsIngressRef: { current: { withdraw: () => { spellsWithdrawalsRef.current++; } } },
    spellsRawSnapshotRef: { current: null }, spellsWithdrawalsRef,
    equipmentControllerRef, equipmentConnectionGenerationRef, equipmentSessionGenerationRef,
    equipmentStartGameRef, equipmentSnapshotRef, equipmentHostSuspendReasonRef,
    equipmentRenderConnectionGeneration: renderConnection,
    equipmentRenderSessionGeneration: renderSession,
    equipmentRenderOwnerToken: { ...renderOwner, connectionGeneration: renderConnection, sessionGeneration: renderSession },
    equipmentBagOwnerRef: { current: { ...renderOwner, ...session } },
    bevyBagSendGateRef: { current: () => true },
    bevyCharacterSendGateRef: { current: () => true },
    navigateSharedHud: () => false,
    sharedHudNavigationRef: {current:{ready:false}}, hudProjectionRef:{current:false},
    fallbackHudNavigationRevisionRef:{current:0}, nextHudNavigationRevision:()=>1,
    sameBagOwner: bagUi.sameBagOwner,
    bagOpenRef: { current: true },
    bagYieldRef: { current: () => {} },
    bagCompatibilityModeRef: { current: null }, bevyBagUiRequested: true,
    setBagCompatibilityMode: () => {}, setStorageServiceOpenVersion: () => {}, setShowInventoryState: () => {},
    setActiveInventoryTab: () => {}, requestBagCompatibility: () => {},
    worldRef, socketRef, lastCommandRef: { current: null },
    movementPredictionBlockedUntilRef: { current: 0 }, MOVEMENT_ACTION_PREDICTION_BLOCK_MS: 0,
    lastRankingRequestRef: { current: null }, movementDiagnosticsRef: { current: null },
    WebSocket: { OPEN: 1 }, window, CustomEvent, console, Date, Math, JSON,
    isSpectatorBrowserMode: () => false,
    isMovementPredictionBlockingCommand: () => false,
    isMovementCommand: () => false,
    isMovementConsoleCommand: () => false,
    isCombatResolutionCommand: () => false,
    recordDebugEvent: () => {},
    appendLog: (message) => logs.push(message),
    t: (_key, _args, fallback) => fallback ?? _key,
    authoritativeItemUniqueId: identity.authoritativeItemUniqueId,
    planBagMove: identity.planBagMove,
    planEquipmentRemoval: identity.planEquipmentRemoval,
    equipmentSlotIndex: slot => ({weapon:0,armour:1,helmet:2,mount:13})[slot],
    currentAuthoritativeItem: identity.currentAuthoritativeItem,
    projectBevyBagModel: bagModel.projectBevyBagModel,
    currentEquipmentCommandItem: adapter.currentEquipmentCommandItem,
    classifyEquipmentUse: adapter.classifyEquipmentUse,
    equipmentGatewayOperation: adapter.equipmentGatewayOperation,
  };
  const keys = Object.keys(scope);
  const functions = new Function(...keys, `${pageJavaScript}\nreturn {syncEquipmentSnapshot,suspendEquipmentConnection,send,sendRaw,moveItem,dispatchBevyBagIntent,dispatchBevyCharacterIntent};`)(...keys.map((key) => scope[key]));
  return {
    ...functions, sent, logs, scope,
    onAction(listener) { actionListener = listener; },
  };
}

function readyController(uid = 4) {
  const controller = new EquipmentSessionController(actualRuntime());
  assert.equal(controller.establishBaseline({ ...session, startGameConfirmed: true, complete: true, snapshot: snapshot(uid) }).ok, true);
  return controller;
}

test("ordinary medicine Use reaches socket with a complete session even if equipment WASM is unavailable", () => {
  const page = harness({ controller: null });
  assert.equal(page.send({ type: "useItem", grid: "inventory", uniqueId: 4 }), true);
  assert.deepEqual(page.sent, [{ type: "useItem", grid: "inventory", uniqueId: 4 }]);
});

test("unknown bag metadata refuses Use, and Equip refuses a missing ledger", () => {
  const unknown = harness({ item: bagItem(4, null) });
  assert.equal(unknown.send({ type: "useItem", grid: "inventory", uniqueId: 4 }), false);
  assert.equal(unknown.sent.length, 0);
  const missing = harness({ controller: null });
  assert.equal(missing.send({ type: "equipItem", grid: "inventory", uniqueId: 4, to: 0 }), false);
  assert.equal(missing.sent.length, 0);
});

test("mount Use proves the current instance but emits only Crystal wire slot 13", () => {
  for (const uid of [0, 27]) {
    const mount = { slot: "mount", uniqueId: uid, authoritativeUniqueId: uid };
    const page = harness({ item: mount,
      worldState: { inventoryItems: [], beltItems: [], storageItems: [], equipmentItems: [mount] },
      cachedSnapshot: { capacity: 46, placements: [{ uniqueId: uid, container: 2, slot: 13 }] },
    });
    assert.equal(page.send({ type: "useItem", grid: "equipment", equipmentInstanceId: uid, slot: 13 }), true);
    assert.deepEqual(page.sent, [{ type: "useItem", grid: "equipment", slot: 13 }]);
  }
});

test("a replaced mount, non-mount item, or missing instance proof cannot issue mount Use", () => {
  const current = { slot: "mount", uniqueId: 28, authoritativeUniqueId: 28 };
  const mountPage = harness({ item: current,
    worldState: { inventoryItems: [], beltItems: [], storageItems: [], equipmentItems: [current] },
  });
  assert.equal(mountPage.send({ type: "useItem", grid: "equipment", equipmentInstanceId: 27, slot: 13 }), false);
  assert.equal(mountPage.send({ type: "useItem", grid: "equipment", slot: 13 }), false);
  assert.equal(mountPage.sent.length, 0);
  const weapon = { slot: "weapon", uniqueId: 29, authoritativeUniqueId: 29 };
  const weaponPage = harness({ item: weapon,
    worldState: { inventoryItems: [], beltItems: [], storageItems: [], equipmentItems: [weapon] },
  });
  assert.equal(weaponPage.send({ type: "useItem", grid: "equipment", equipmentInstanceId: 29, slot: 13 }), false);
  assert.equal(weaponPage.sent.length, 0);
});

test("synchronous action listener logout prevents outer Equip bytes and releases its proven-unsent reservation", () => {
  const controller = readyController();
  const page = harness({ controller });
  page.onAction((event) => {
    if (event.detail.type === "equipItem") page.send({ type: "logOut" });
  });
  assert.equal(page.send({ type: "equipItem", grid: "inventory", uniqueId: 4, to: 0 }), false);
  assert.deepEqual(page.sent, [{ type: "logOut" }]);
  assert.equal(page.scope.equipmentHostSuspendReasonRef.current, "logoutPending");
  assert.equal(page.scope.spellsWithdrawalsRef.current, 1);
  assert.equal(controller.status().pending, 0);
});

test("synchronous action listener logout also prevents outer Move bytes", () => {
  const page = harness();
  page.onAction((event) => {
    if (event.detail.type === "moveItem") page.send({ type: "logOut" });
  });
  assert.equal(page.send({ type: "moveItem", grid: "inventory", from: 0, to: 4 }), false);
  assert.deepEqual(page.sent, [{ type: "logOut" }]);
  assert.equal(page.scope.equipmentHostSuspendReasonRef.current, "logoutPending");
});

test("cross-page move keeps the actual WASM ledger target reservation and sends only explicit destination cells", () => {
  const source = { ...bagItem(4), slot: 0 };
  const target = { ...bagItem(0), container: "bag2", slot: 4 };
  const sourcePageOther = { ...bagItem(8), slot: 4 };
  const worldState = { ...world(source), inventoryItems: [source, target, sourcePageOther], maxBagSlots: 48 };
  const cachedSnapshot = { capacity: 54, placements: [
    { uniqueId: 4, container: 0, slot: 0 },
    { uniqueId: 0, container: 0, slot: 44 },
    { uniqueId: 8, container: 0, slot: 4 },
  ] };
  const controller = new EquipmentSessionController(actualRuntime());
  assert.equal(controller.establishBaseline({ ...session, startGameConfirmed: true, complete: true, snapshot: cachedSnapshot }).ok, true);
  const operation = { kind: "equip", uniqueId: 0, grid: "inventory", to: 0 };
  const reservation = controller.reserve({ ...session, ownerRevision: 0, operation });
  assert.equal(reservation.ok, true);
  const page = harness({ worldState, cachedSnapshot, controller });
  assert.equal(page.moveItem(source, 4, "bag2"), false);
  assert.deepEqual(page.sent, []);
  assert.equal(controller.hasPendingInstance(0).reserved, true);
  assert.equal(controller.cancelDefinitelyUnsent(reservation.ticket).ok, true);
  assert.equal(controller.reserve({ ...session, ownerRevision: 0, operation: { ...operation, uniqueId: 8 } }).ok, true);
  assert.equal(page.moveItem(source, 4, "bag2"), true);
  assert.deepEqual(page.sent, [{ type: "moveItem", grid: "inventory", from: 0, to: 44 }]);
  assert.equal(page.moveItem(target, 0, "bag1"), true);
  assert.deepEqual(page.sent.at(-1), { type: "moveItem", grid: "inventory", from: 44, to: 0 });
  const count = page.sent.length;
  for (const destination of [undefined, "quest", "belt", "storage"]) {
    assert.equal(page.moveItem(source, 4, destination), false);
  }
  assert.equal(page.sent.length, count, "bag actions cannot infer a missing page or change grids");
});

test("cross-page move rechecks the socket, connection, session, and logout gate after the synchronous action event", () => {
  const source = { ...bagItem(4), slot: 0 };
  for (const change of ["socket", "connection", "session", "logout"]) {
    const page = harness({ worldState: { ...world(source), maxBagSlots: 48 },
      cachedSnapshot: { capacity: 54, placements: [{ uniqueId: 4, container: 0, slot: 0 }] } });
    page.onAction((event) => {
      if (event.detail.type !== "moveItem") return;
      if (change === "socket") page.scope.socketRef.current = { readyState: 1, send() { throw new Error("wrong socket"); } };
      if (change === "connection") page.scope.equipmentConnectionGenerationRef.current++;
      if (change === "session") page.scope.equipmentSessionGenerationRef.current++;
      if (change === "logout") page.send({ type: "logOut" });
    });
    assert.equal(page.moveItem(source, 4, "bag2"), false, change);
    assert.deepEqual(page.sent, change === "logout" ? [{ type: "logOut" }] : [], change);
  }
});

test("a captured render identity from another connection or session cannot send item commands", () => {
  for (const stale of [{ renderConnection: 2 }, { renderSession: 4 }]) {
    const page = harness({ controller: readyController(), ...stale });
    assert.equal(page.send({ type: "equipItem", grid: "inventory", uniqueId: 4, to: 0 }), false);
    assert.equal(page.send({ type: "moveItem", grid: "inventory", from: 0, to: 4 }), false);
    assert.equal(page.sent.length, 0);
  }
});

test("late WASM installation cannot establish a baseline across a pre-existing logout gate", () => {
  const page = harness({ controller: null });
  page.suspendEquipmentConnection("logoutPending");
  const controller = new EquipmentSessionController(actualRuntime());
  page.scope.equipmentControllerRef.current = controller;
  page.syncEquipmentSnapshot();
  assert.equal(controller.status().ready, false);
  assert.equal(page.scope.equipmentHostSuspendReasonRef.current, "logoutPending");
});

test("actual page rejects old rendered owners and keeps the real WASM pending across handoff and ACK", () => {
  const controller = readyController();
  const page = harness({ controller });
  const equip = { type: "equipItem", grid: "inventory", uniqueId: 4, to: 0 };
  assert.equal(page.send(equip), true);
  const token = { ...session, runGeneration: 1, ownerRevision: 1, owner: "bevy", modelRevision: 7, presentationRevision: 4 };
  page.scope.equipmentBagOwnerRef.current = { ...token };
  controller.setOwnerRevision(1);
  assert.equal(page.send(equip), false, "old React callback cannot borrow new owner");
  assert.equal(page.send(equip, { ownerToken: token }), false, "new renderer shares pending instance");
  assert.equal(page.sent.length, 1);
  assert.equal(controller.status().pending, 1);
  assert.equal(controller.applyAck({ ...session, operation: { kind: "equip", uniqueId: 4, grid: "inventory", to: 0 }, success: true }).ok, true);
  assert.equal(controller.status().pending, 1, "success ACK still awaits authoritative placement");
  controller.observeSnapshot({ ...session, complete: true, snapshot: { capacity: 46, placements: [{ uniqueId: 4, container: 2, slot: 0 }] } });
  assert.equal(controller.status().pending, 0, "ACK is not rejected merely because the renderer changed");
  page.scope.equipmentBagOwnerRef.current = { ...token, owner: "react", ownerRevision: 2 };
  controller.setOwnerRevision(2);
  assert.equal(page.send({ type: "moveItem", grid: "inventory", from: 0, to: 4 }, { ownerToken: token }), false);
});

test("actual raw send rechecks the original Bevy token and live host after synchronous action listeners", () => {
  for (const change of ["owner", "health"]) {
    const controller = readyController();
    const token = { ...session, runGeneration: 8, ownerRevision: 2, owner: "bevy", modelRevision: 9, presentationRevision: 1 };
    controller.setOwnerRevision(2);
    const page = harness({ controller, renderOwner: token });
    page.onAction(() => {
      if (change === "owner") {
        page.scope.equipmentBagOwnerRef.current = { ...token, ownerRevision: 3, owner: "react" };
        controller.setOwnerRevision(3);
      } else page.scope.bevyBagSendGateRef.current = () => false;
    });
    assert.equal(page.send({ type: "equipItem", grid: "inventory", uniqueId: 4, to: 0 }, { ownerToken: token }), false);
    assert.equal(page.sent.length, 0);
    assert.equal(controller.status().pending, 0, "only this definitely-unsent reservation is canceled");
  }
});

test("retired compatibility bag callbacks cannot submit Drop, Merge, Split, Sell or storage operations", () => {
  for (const type of ["dropItem", "mergeItem", "splitItem", "sellItem", "dropGold", "storeItemV2", "takeBackItemV2"]) {
    const command = { type, uniqueId: 4, from: 0, to: 4, count: 1 };
    const allowed = harness();
    assert.equal(allowed.send(command), true, `current owner: ${type}`);
    assert.deepEqual(allowed.sent, [command], "existing wire is preserved");
    const page = harness();
    page.scope.equipmentBagOwnerRef.current = { ...session, runGeneration: 1, ownerRevision: 1, owner: "bevy" };
    assert.equal(page.send({ type }), false, type);
    assert.deepEqual(page.sent, []);
    const current = harness();
    current.onAction(() => { current.scope.equipmentBagOwnerRef.current.ownerRevision++; });
    assert.equal(current.send({ type }), false, `final gate: ${type}`);
    assert.deepEqual(current.sent, []);
  }
});

test("actual Bevy intent adapter preserves UID0 and global bag cells through shared Equip/Use/Move gates", () => {
  const owner = { ...session, owner: "bevy", runGeneration: 1, ownerRevision: 1, modelRevision: 2, presentationRevision: 3 };
  const item = { ...bagItem(0), slot: 0, key: "0", name: "Wooden Sword", icon: 1, quantity: 1, description: "", equipSlot: "weapon" };
  const worldState = { ...world(item), inventoryCapacity: 54, maxBagSlots: 48, gold: 200 };
  const cachedSnapshot = { capacity: 54, placements: [{ uniqueId: 0, container: 0, slot: 0 }] };
  const create = () => {
    const controller = new EquipmentSessionController(actualRuntime());
    controller.establishBaseline({ ...session, startGameConfirmed: true, complete: true, snapshot: cachedSnapshot });
    controller.setOwnerRevision(1);
    return harness({ item, worldState, cachedSnapshot, controller, renderOwner: owner });
  };
  const intent = { ...owner, intentSequence: 1, type: "useItem", source: { container: 0, slot: 0, uniqueId: 0 } };
  const use = create();
  assert.equal(use.dispatchBevyBagIntent(intent).accepted, true);
  assert.deepEqual(use.sent, [{ type: "equipItem", uniqueId: 0, grid: "inventory", to: 0 }]);
  assert.equal(use.dispatchBevyBagIntent({ ...intent, type: "moveItem", target: { container: 0, slot: 44 } }).accepted, false);
  const move = create();
  assert.equal(move.dispatchBevyBagIntent({ ...intent, type: "moveItem", target: { container: 0, slot: 44 } }).accepted, true);
  assert.deepEqual(move.sent, [{ type: "moveItem", grid: "inventory", from: 0, to: 44 }]);
  const wrong = create();
  assert.equal(wrong.dispatchBevyBagIntent({ ...intent, source: { ...intent.source, uniqueId: 4 } }).accepted, false);
  wrong.scope.worldRef.current = { ...worldState, inventoryItems: [item, { ...item, slot: 1 }] };
  assert.equal(wrong.dispatchBevyBagIntent(intent).accepted, false);
  assert.deepEqual(wrong.sent, []);
});

test("actual close wrapper revokes ownership before a same-frame queued Use can submit", () => {
  const controller = readyController();
  const token = { ...session, owner: "bevy", runGeneration: 1, ownerRevision: 1, modelRevision: 2, presentationRevision: 3 };
  controller.setOwnerRevision(1);
  const page = harness({ controller, renderOwner: token });
  let yielded = 0;
  page.scope.bagYieldRef.current = () => {
    yielded++;
    page.scope.equipmentBagOwnerRef.current = { ...token, owner: "react", ownerRevision: 2 };
    controller.setOwnerRevision(2);
  };
  assert.equal(page.dispatchBevyBagIntent({ ...token, intentSequence: 1, type: "close" }).accepted, true);
  assert.equal(yielded, 1);
  assert.equal(page.scope.bagOpenRef.current, false);
  assert.equal(page.dispatchBevyBagIntent({ ...token, intentSequence: 2, type: "useItem", source: { container: 0, slot: 4, uniqueId: 4 } }).accepted, false);
  assert.deepEqual(page.sent, []);
});


function characterPage() {
  const equipment = {slot:"weapon",uniqueId:0,authoritativeUniqueId:0};
  const inventoryItems = Array.from({length:40},(_,slot)=>({...bagItem(slot+100),container:"bag1",slot}));
  const state={inventoryItems,beltItems:[{...bagItem(999),container:"belt",slot:0}],storageItems:[],equipmentItems:[equipment],maxBagSlots:80};
  const baseline={capacity:86,placements:[...inventoryItems.map(i=>({uniqueId:i.authoritativeUniqueId,container:0,slot:i.slot})),{uniqueId:999,container:1,slot:0},{uniqueId:0,container:2,slot:0}]};
  const controller=new EquipmentSessionController(actualRuntime());
  assert.equal(controller.establishBaseline({...session,startGameConfirmed:true,complete:true,snapshot:baseline}).ok,true);
  const page=harness({item:equipment,worldState:state,cachedSnapshot:baseline,controller});
  page.scope.bagOpenRef.current=false;
  page.scope.bevyCharacterSendGateRef.current=(proof,own)=>proof.source.uniqueId===0&&(!controller.hasPendingInstance(0).reserved||own===0);
  const proof={...page.scope.equipmentBagOwnerRef.current,surface:"character",characterRunGeneration:7,hudGeneration:8,modelRevision:2,presentationRevision:3,source:{container:2,slot:0,uniqueId:0}};
  const intent={type:"removeEquipment",source:proof.source};
  return {page,controller,proof,intent};
}
test("actual Character dispatch rechecks UID0, hidden Bag and first empty second-page slot through one real ledger",()=>{
  const {page,controller,proof,intent}=characterPage();
  assert.equal(page.dispatchBevyCharacterIntent(intent,proof),true);
  assert.deepEqual(page.sent,[{type:"removeItem",uniqueId:0,grid:"inventory",to:40}]);
  assert.equal(controller.hasPendingInstance(0).reserved,true);
  assert.equal(page.dispatchBevyCharacterIntent(intent,proof),false);
  assert.equal(controller.applyAck({...session,operation:{kind:"remove",uniqueId:0,grid:"inventory",to:41},success:true}).matched,false);
  assert.equal(controller.hasPendingInstance(0).reserved,true);
});
test("actual Character source replacement, full bag and final synchronous logout cannot send",()=>{
  for(const mode of ["replacement","full","logout"]){const {page,controller,proof,intent}=characterPage();
    if(mode==="replacement")page.scope.worldRef.current.equipmentItems=[{slot:"weapon",uniqueId:9,authoritativeUniqueId:9}];
    if(mode==="full")page.scope.worldRef.current.inventoryItems.push(...Array.from({length:40},(_,slot)=>({...bagItem(slot+200),container:"bag2",slot})));
    if(mode==="logout")page.onAction(e=>{if(e.detail.type==="removeItem")page.suspendEquipmentConnection("logoutPending");});
    assert.equal(page.dispatchBevyCharacterIntent(intent,proof),false);assert.equal(page.sent.length,0);
    assert.equal(controller.hasPendingInstance(0).reserved,false);
  }
});
test("actual Character unknown send keeps the existing pending reservation",()=>{
  const {page,controller,proof,intent}=characterPage();page.scope.socketRef.current.send=()=>{throw Error("transport outcome unknown");};
  assert.equal(page.dispatchBevyCharacterIntent(intent,proof),false);assert.equal(controller.hasPendingInstance(0).reserved,true);
});
