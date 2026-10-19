import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import test from "node:test";

// This harness executes declarations from the actual Page with the actual
// repair transport and carried-item service. React/Core/WASM are not loaded.
const ts = createRequire(import.meta.url)(process.env.MIR2_TEST_TYPESCRIPT_PATH ?? "typescript");
const compilerOptions = { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS };
const sources = {
  repairClient: "../lib/hunting-repair-client.ts",
  repairWallet: "../lib/hunting-repair-wallet.ts",
  purchaseClient: "../lib/npc-purchase-client.ts",
  repairService: "../lib/npc-repair-service.ts",
  equipment: "../lib/equipment-gateway-adapter.ts",
  bag: "../lib/bevy-bag-model.ts",
  identity: "../lib/world-model/item-identity.ts",
};
const allow = {
  repairClient: { "./npc-purchase-client": "purchaseClient" },
  repairWallet: { "./npc-purchase-client": "purchaseClient" },
  purchaseClient: {},
  repairService: { "./equipment-gateway-adapter": "equipment" },
  equipment: { "./world-model/item-identity": "identity" },
  bag: { "./world-model/item-identity": "identity" },
  identity: {},
};
const clock = { now: 1000 };
class FixtureDate extends Date { static now() { return clock.now; } }
let attemptedFetches = 0;
const modules = new Map();
function load(name) {
  assert(Object.hasOwn(sources, name), "unexpected repair module " + name);
  if (modules.has(name)) return modules.get(name);
  const url = new URL(sources[name], import.meta.url);
  const module = { exports: {} };
  const compiled = ts.transpileModule(readFileSync(url, "utf8"), {
    fileName: fileURLToPath(url), compilerOptions, reportDiagnostics: true,
  });
  assert.equal((compiled.diagnostics ?? []).filter(value => value.category === ts.DiagnosticCategory.Error).length, 0);
  new Function("require", "module", "exports", "Date", "fetch", compiled.outputText)((id) => {
    assert(Object.hasOwn(allow[name], id), "unexpected " + name + " dependency " + id);
    return load(allow[name][id]);
  }, module, module.exports, FixtureDate, () => { attemptedFetches++; throw Error("Page harness must not open a network request"); });
  modules.set(name, module.exports);
  return module.exports;
}
const repairClient = load("repairClient");
const { HuntingRepairWalletClient } = load("repairWallet");
const purchaseClient = load("purchaseClient");
const { NpcRepairService } = load("repairService");
const { projectBevyBagModel } = load("bag");

// Reuse only the complete data fixture from the existing strict client suite;
// its test registration and implementation are never imported or executed.
const fixtureUrl = new URL("./test-hunting-repair-client.mjs", import.meta.url);
const fixtureAst = ts.createSourceFile(fileURLToPath(fixtureUrl), readFileSync(fixtureUrl, "utf8"),
  ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
const snapshots = fixtureAst.statements.filter(node => ts.isFunctionDeclaration(node) && node.name?.text === "snapshot");
assert.equal(snapshots.length, 1, "reuse the sole complete snapshot fixture");
const fullSnapshot = new Function(snapshots[0].getText(fixtureAst) + "\nreturn snapshot;")();

const pageUrl = new URL("../app/page.tsx", import.meta.url);
const pageSource = readFileSync(pageUrl, "utf8");
const pageAst = ts.createSourceFile(fileURLToPath(pageUrl), pageSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function actualPageDeclarations(names) {
  const declarations = new Map();
  const visit = (node) => {
    if (ts.isFunctionDeclaration(node) && node.name && names.includes(node.name.text)) {
      assert(!declarations.has(node.name.text), "ambiguous actual Page declaration " + node.name.text);
      declarations.set(node.name.text, node.getText(pageAst));
    }
    ts.forEachChild(node, visit);
  };
  visit(pageAst);
  assert.deepEqual([...declarations.keys()].sort(), names.slice().sort(), "all actual Page repair declarations must exist");
  return names.map(name => declarations.get(name)).join("\n");
}

const pageNames = [
  "clearIdentitySessionToken", "captureIdentitySessionBinding",
  "currentNpcRepairPaymentMode", "captureHuntingRepairMode", "currentHuntingRepairBinding",
  "retireHuntingRepairActor", "observeHuntingRepairActor", "applyHuntingRepairSnapshot", "npcRepairCanMutate",
  "invalidateHuntingRepairWallet", "refreshHuntingRepairWallet",
  "readNpcRepairOwner", "captureNpcRepairDialog", "currentNpcRepairDialogSource", "closeNpcRepairService",
  "readNpcRepairView", "selectNpcRepair", "toggleNpcRepairHold", "beginNpcRepairDrag", "cancelNpcRepairDrag",
  "dropNpcRepairDrag", "quoteHuntingRepairSelection", "confirmNpcRepair", "queryNpcRepair", "receiveNpcPurchaseGatewayFrame",
];
const physicalMessageListeners = [];
const npcRepairCases = new Map();
let gatewayAdmissionGuard;
function visitPhysicalListeners(node) {
  if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression)
    && node.expression.expression.getText(pageAst) === "socket" && node.expression.name.text === "addEventListener"
    && ts.isStringLiteral(node.arguments[0]) && node.arguments[0].text === "message") {
    physicalMessageListeners.push(node.arguments[1].getText(pageAst));
  }
  if (ts.isFunctionDeclaration(node) && node.name?.text === "handleGatewayEvent") {
    assert.equal(gatewayAdmissionGuard, undefined, "use the sole actual Gateway admission guard");
    gatewayAdmissionGuard = node.body.statements[0].getText(pageAst);
  }
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression)
    && ["NPCRepair", "NPCSRepair"].includes(node.expression.text)) {
    assert(!npcRepairCases.has(node.expression.text), "use the sole actual repair packet case");
    npcRepairCases.set(node.expression.text, node.getText(pageAst));
  }
  ts.forEachChild(node, visitPhysicalListeners);
}
visitPhysicalListeners(pageAst);
assert.equal(physicalMessageListeners.length, 1, "execute the sole actual physical message listener");
assert.equal(gatewayAdmissionGuard,
  "if (connectionGeneration !== equipmentConnectionGenerationRef.current || socketRef.current !== source) return;",
  "retain the actual socket/generation admission before the extracted repair packet cases");
assert.deepEqual([...npcRepairCases.keys()].sort(), ["NPCRepair", "NPCSRepair"]);
const pageCode = ts.transpileModule(actualPageDeclarations(pageNames)
  + "\nfunction physicalMessageListener(socket, connectionGeneration) { return " + physicalMessageListeners[0] + "; }"
  + "\nfunction actualNpcRepairPacket(event, connectionGeneration, source) {" + gatewayAdmissionGuard
  + "\nconst payload = event.payload ?? {}; switch(event.packet) {\n"
  + npcRepairCases.get("NPCRepair") + "\n" + npcRepairCases.get("NPCSRepair") + "\n}}", {
  fileName: "actual-hunting-repair-page.ts", compilerOptions, reportDiagnostics: true,
});
assert.equal((pageCode.diagnostics ?? []).filter(value => value.category === ts.DiagnosticCategory.Error).length, 0);

const uid = 42;
const quoteId = "ab".repeat(32);
function rawSnapshot() {
  const raw = fullSnapshot();
  raw.activeNpcDialog = { npcObjectId: 101, npcName: "Repairer", pages: [], buttons: [] };
  raw.inventoryItems = [{ key: "damaged-sword", name: "Damaged sword", description: "Source fixture", icon: 1,
    uniqueId: uid, quantity: 1, container: "bag1", slot: 0, durabilityCurrent: 500, durabilityMax: 1000,
    tooltipSource: { info: { item_index: 1, item_type: 1, price: 100, durability: 1000 },
      userItem: { unique_id: uid, item_index: 1, count: 1, current_dura: 500, max_dura: 1000,
        added_stats: [], rental_information: null } } }];
  assert(purchaseClient.completeNpcPurchaseSnapshot(raw), "the actual strict parser accepts the full fixture");
  return raw;
}
function displayWorld(raw) {
  return { ...raw, connected: true, playerObjectId: String(raw.playerObjectId),
    entities: raw.entities.map(entity => ({ ...entity, objectId: String(entity.objectId) })),
    inventoryItems: raw.inventoryItems.map(item => ({ ...item, authoritativeUniqueId: item.uniqueId })) };
}
const quote = (patch = {}) => ({ quoteId, uniqueId: String(uid), special: false,
  costBoundGold: "123", createdAtMs: 1000, expiresAtMs: 31_000, ...patch });
const receipt = (patch = {}) => ({ quoteId, costBoundGold: "123", free: false,
  operationKey: "original-repair-operation", requestDigest: "original-repair-digest",
  businessDay: 20_000, committedAtMs: 2000, ...patch });
const frame = (requestId, reply, snapshot = null) => JSON.stringify({ type: "huntingRepairOwner",
  protocolVersion: 1, requestId, reply, snapshot });
const ref = current => ({ current });

function fixture() {
  clock.now = 1000;
  const state = { duringHero: null, duringApply: null, synchronize: true, bagMatches: true, bagReady: true, spectator: false };
  const sent = [], legacy = [], generic = [], handled = [], applied = [], logs = [], tokenUpdates = [], paneChanges = [];
  const socket = { readyState: 1, send(raw) { sent.push({ raw, request: JSON.parse(raw) }); } };
  const raw = rawSnapshot();
  const startGame = Object.freeze({ connectionGeneration: 3, sessionGeneration: 5 });
  const scope = {
    ...repairClient, ...purchaseClient, HuntingRepairWalletClient, projectBevyBagModel,
    huntingRepairHandshakeRef: ref(null), huntingRepairBindingRef: ref(null), huntingRepairApplyingRef: ref(false),
    equipmentStartGameRef: ref(startGame), equipmentSnapshotRef: ref({ ...startGame, snapshot: raw }),
    equipmentConnectionGenerationRef: ref(3), equipmentSessionGenerationRef: ref(5),
    equipmentBagOwnerRef: ref({ ownerRevision: 9 }), equipmentHostSuspendReasonRef: ref(null),
    equipmentControllerRef: ref({ status: () => ({ ready: state.bagReady, pending: 0 }) }),
    accountIdRef: ref("owner"), socialCharacterIndexRef: ref(ref(0)), socketRef: ref(socket),
    identitySessionBindingRef: ref(null),
    worldRef: ref(displayWorld(raw)), screenRef: ref("game"),
    npcRepairAuthorityRef: ref(new NpcRepairService()), npcRepairDialogSourceRef: ref(null),
    npcRepairDialogBindingRef: ref(null), npcRepairServiceRef: ref(null),
    worldSnapshotVersionRef: ref(1), mailRawRef: ref(null), socialFriendsRef: ref(null), npcPurchaseDisplaySourceRef: ref(null),
    questCoreRuntimeRef: ref({ readNpcRepairQuote() { throw Error("hunting must not call the legacy Rust quote reader"); } }),
    pendingStorageRequestsRef: ref(new Map()), socialItemOperationsRef: ref({ pending: false }),
    storageRentalRef: ref({ pending: false }), npcBuyDispatcherRef: ref({ status: () => ({ flight: false }) }),
    mailParcelRef: ref(null), bagOpenRef: ref(true), activeInventoryTabRef: ref("bag1"),
    npcGoldBuyInventoryRef: ref({ matches: () => state.bagMatches }),
    npcPurchaseClientRef: ref(null), npcPurchaseUnavailableRef: ref(false), heroVerifiedDeliveryRef: ref(null),
    heroRawIngressRef: ref(null), heroSharedUiIngressRef: ref(null),
    WebSocket: { OPEN: 1 }, Date: FixtureDate, performance: { now: () => clock.now }, console,
    isSpectatorBrowserMode: () => state.spectator,
    renderParityServices: () => {}, setNpcRepairService: value => { scope.npcRepairServiceRef.current = value; },
    setIdentitySessionToken: value => { tokenUpdates.push({ cleared: value === null,
      refUpdatedFirst: (scope.identitySessionBindingRef.current?.token ?? null) === value }); },
    updateWorld: update => { scope.worldRef.current = update(scope.worldRef.current); },
    retireNpcShopService: () => {}, endStorageService: () => {},
    requestBagCompatibility: mode => { assert.equal(mode, "fullInventory"); scope.equipmentBagOwnerRef.current.ownerRevision++; },
    setShowInventory: open => { scope.bagOpenRef.current = open; paneChanges.push({ inventory: open }); },
    setActiveInventoryTab: page => { scope.activeInventoryTabRef.current = page; },
    setShowCharacter: open => { paneChanges.push({ character: open }); },
    currentSocialReplyOwner: () => ({ socket: scope.socketRef.current,
      connectionGeneration: scope.equipmentConnectionGenerationRef.current,
      sessionGeneration: scope.equipmentSessionGenerationRef.current, sceneRevision: 1,
      playerObjectId: Number(scope.worldRef.current.playerObjectId), mapFileName: scope.worldRef.current.mapFileName }),
    currentSpellsOwner: playerObjectId => ({ connectionGeneration: scope.equipmentConnectionGenerationRef.current,
      sessionGeneration: scope.equipmentSessionGenerationRef.current, playerObjectId }),
    mergeMailList: value => Array.isArray(value) ? value : null,
    parityItemMutationAllowed: () => true, mailMutationAllowed: () => true,
    invalidateNpcGoldBuyGatewayPacket: () => {}, captureQuestMapGatewayEvent: () => {}, captureSpellsGatewayEvent: () => {},
    applyNpcGoldBuyGatewaySnapshot: (snapshot, generation, full) => {
      assert.equal(generation, scope.equipmentConnectionGenerationRef.current);
      assert.equal(full, true, "repair uses the full owner snapshot path");
      applied.push(snapshot);
      if (state.synchronize) {
        scope.worldRef.current = displayWorld(snapshot);
        scope.equipmentSnapshotRef.current = { connectionGeneration: generation,
          sessionGeneration: scope.equipmentSessionGenerationRef.current, snapshot };
      }
      state.duringApply?.();
    },
    applyNpcPurchaseShopSnapshot: () => {},
    parseGatewayMailDates: raw => { generic.push(raw); return JSON.parse(raw); },
    handleGatewayEvent: (...args) => { handled.push(args); },
    observeHeroRawOwner: () => {}, syncHeroRawIngress: () => { state.duringHero?.(); }, currentHeroRawPhysical: () => null,
    sendRaw: (...args) => { legacy.push(args); return true; },
    appendLog: (...args) => { logs.push(args); }, t: (key, _args, fallback) => fallback ?? key,
  };
  const page = new Function(...Object.keys(scope), pageCode.outputText
    + "\nreturn {" + [...pageNames, "physicalMessageListener", "actualNpcRepairPacket"].join(",") + "};")(...Object.values(scope));
  const open = (mode = "repair") => {
    const binding = scope.huntingRepairBindingRef.current;
    assert(page.currentHuntingRepairBinding(binding));
    const owner = page.readNpcRepairOwner();
    page.captureNpcRepairDialog(raw, owner);
    const source = scope.npcRepairDialogSourceRef.current;
    assert(scope.npcRepairAuthorityRef.current.openHunting(owner, mode, source, "Repairer"));
    scope.npcRepairDialogBindingRef.current = source;
    scope.npcRepairServiceRef.current = mode;
    assert(scope.npcRepairAuthorityRef.current.observeSnapshot(owner, raw));
    binding.client.showView();
    const view = page.readNpcRepairView();
    assert(view && view.paymentMode === "hunting-v1" && view.rows.length === 1);
    return view;
  };
  const bind = () => {
    page.captureHuntingRepairMode({ npcRepairMode: "hunting-v1" }, scope.socketRef.current,
      scope.equipmentConnectionGenerationRef.current);
    const binding = scope.huntingRepairBindingRef.current;
    assert(page.currentHuntingRepairBinding(binding));
    return binding;
  };
  const receive = (reply, snapshot = null, requestId = sent.at(-1)?.request.requestId, binding = scope.huntingRepairBindingRef.current) => {
    const raw = frame(requestId, reply, snapshot);
    page.receiveNpcPurchaseGatewayFrame({ data: raw }, scope.equipmentConnectionGenerationRef.current,
      scope.socketRef.current, null, binding);
    return raw;
  };
  return { page, scope, state, socket, raw, startGame, sent, legacy, generic, handled, applied, logs,
    tokenUpdates, paneChanges, open, bind, receive };
}

test("actual Page requires a physical mode and permanently blocks an in-place mode change", () => {
  const f = fixture();
  assert.equal(f.page.currentNpcRepairPaymentMode(), null);
  assert.equal(f.page.observeHuntingRepairActor(f.socket), null);
  assert.equal(f.page.readNpcRepairView(), null);
  f.page.captureHuntingRepairMode({}, f.socket, 3);
  assert.equal(f.page.currentNpcRepairPaymentMode(), "legacy");
  f.page.captureHuntingRepairMode({ npcRepairMode: "hunting-v1" }, f.socket, 3);
  assert.equal(f.page.currentNpcRepairPaymentMode(), "blocked");
  assert.equal(f.scope.huntingRepairBindingRef.current, null);

  const hunting = fixture();
  const binding = hunting.bind();
  hunting.open();
  hunting.page.captureHuntingRepairMode({ npcRepairMode: "hunting-v1" }, hunting.socket, 3);
  assert.equal(hunting.scope.huntingRepairBindingRef.current, binding, "an identical handshake preserves the actual adapter");
  hunting.page.captureHuntingRepairMode({ npcRepairMode: "legacy" }, hunting.socket, 3);
  assert.equal(binding.client.view().phase, "retired");
  assert.equal(hunting.scope.huntingRepairBindingRef.current, null);
  assert.equal(hunting.page.currentNpcRepairPaymentMode(), "blocked");
  assert.equal(hunting.scope.huntingRepairHandshakeRef.current.huntingDeclared, true);
  hunting.page.captureHuntingRepairMode({ npcRepairMode: "hunting-v1" }, hunting.socket, 3);
  assert.equal(hunting.page.currentNpcRepairPaymentMode(), "blocked");
  assert.equal(hunting.page.readNpcRepairView(), null);
  assert.equal(hunting.sent.length + hunting.legacy.length, 0);
});

test("actual Page binds mode and actor to the physical socket and exact StartGame witness", () => {
  const f = fixture(), first = f.bind();
  const replacement = Object.freeze({ ...f.startGame });
  f.scope.equipmentStartGameRef.current = replacement;
  assert.equal(f.page.currentHuntingRepairBinding(first), false);
  const second = f.page.observeHuntingRepairActor(f.socket);
  assert(second && second !== first);
  assert.equal(first.client.view().phase, "retired");
  assert.equal(second.actor.startGame, replacement);

  f.scope.accountIdRef.current = "second-owner";
  const third = f.page.observeHuntingRepairActor(f.socket);
  assert(third && third !== second);
  assert.equal(second.client.view().phase, "retired");
  assert.equal(third.actor.accountId, "second-owner");
  f.scope.socialCharacterIndexRef.current.current = 1;
  const fourth = f.page.observeHuntingRepairActor(f.socket);
  assert(fourth && fourth.actor.characterIndex === 1);
  assert.equal(third.client.view().phase, "retired");

  const nextSocket = { readyState: 1, send: f.socket.send };
  f.scope.socketRef.current = nextSocket;
  f.scope.equipmentConnectionGenerationRef.current = 4;
  f.scope.equipmentStartGameRef.current = Object.freeze({ connectionGeneration: 4, sessionGeneration: 6 });
  f.scope.equipmentSessionGenerationRef.current = 6;
  f.scope.equipmentSnapshotRef.current = { connectionGeneration: 4, sessionGeneration: 6, snapshot: f.raw };
  assert.equal(f.page.currentNpcRepairPaymentMode(), null);
  f.page.captureHuntingRepairMode({ npcRepairMode: "hunting-v1" }, f.socket, 3);
  assert.equal(f.page.currentNpcRepairPaymentMode(), null, "old physical handshake is ignored");
  f.page.captureHuntingRepairMode({ npcRepairMode: "hunting-v1" }, nextSocket, 4);
  assert.equal(fourth.client.view().phase, "retired");
  assert(f.page.currentHuntingRepairBinding(f.scope.huntingRepairBindingRef.current));
  assert.equal(f.scope.huntingRepairBindingRef.current.socket, nextSocket);
});

test("actual Page accepts only a complete current owner snapshot through its full application path", () => {
  const f = fixture(), binding = f.bind();
  for (const key of ["inventoryItems", "stage5Systems", "entities", "gold"]) {
    const partial = structuredClone(f.raw);
    delete partial[key];
    assert.equal(f.page.applyHuntingRepairSnapshot(binding, partial), false, key);
  }
  assert.equal(f.page.applyHuntingRepairSnapshot(binding, { ...f.raw, playerObjectId: 9 }), false);
  assert.equal(f.applied.length, 0);
  const repaired = structuredClone(f.raw);
  repaired.inventoryItems[0].durabilityCurrent = 1000;
  repaired.inventoryItems[0].tooltipSource.userItem.current_dura = 1000;
  repaired.gold = 101;
  assert.equal(f.page.applyHuntingRepairSnapshot(binding, repaired), true);
  assert.equal(f.applied.length, 1);
  assert.equal(f.scope.worldRef.current.inventoryItems[0].durabilityCurrent, 1000);
  assert.equal(f.scope.huntingRepairApplyingRef.current, false);
  assert.equal(f.scope.npcPurchaseDisplaySourceRef.current, null, "repair does not forge purchase/Core Applied proof");
});

test("actual Page refuses nested snapshot publication and rechecks actor and revision after callbacks", () => {
  const nested = fixture(), binding = nested.bind();
  let nestedAccepted;
  nested.state.duringApply = () => {
    nestedAccepted = nested.page.applyHuntingRepairSnapshot(binding, nested.raw);
  };
  assert.equal(nested.page.applyHuntingRepairSnapshot(binding, nested.raw), true);
  assert.equal(nestedAccepted, false);
  assert.equal(nested.applied.length, 1);

  for (const mutation of [
    f => { f.scope.accountIdRef.current = "another-owner"; },
    f => { f.scope.equipmentStartGameRef.current = Object.freeze({ ...f.startGame }); },
    f => { f.scope.worldSnapshotVersionRef.current++; },
    f => { f.page.retireHuntingRepairActor(); },
  ]) {
    const f = fixture(), current = f.bind();
    f.state.duringApply = () => mutation(f);
    assert.equal(f.page.applyHuntingRepairSnapshot(current, f.raw), false);
    assert.equal(f.scope.huntingRepairApplyingRef.current, false, "all failed callbacks release the local applying barrier");
  }
});

test("actual Page does not acknowledge a displayed world or inventory that failed synchronization", () => {
  for (const failure of ["world", "bag", "equipment"]) {
    const f = fixture(), binding = f.bind();
    const changed = structuredClone(f.raw);
    changed.gold = 102;
    if (failure === "world") f.state.synchronize = false;
    if (failure === "bag") f.state.bagMatches = false;
    if (failure === "equipment") f.state.duringApply = () => {
      f.scope.equipmentSnapshotRef.current = { connectionGeneration: 3, sessionGeneration: 4, snapshot: changed };
    };
    assert.equal(f.page.applyHuntingRepairSnapshot(binding, changed), false, failure);
    assert.equal(f.applied.length, 1);
  }
});

test("actual Page consumes the original repair namespace without generic decoding or legacy fallback", () => {
  const f = fixture(), binding = f.bind();
  assert(binding.client.query(quoteId));
  const observed = [], realReceive = binding.client.receive.bind(binding.client);
  binding.client.receive = raw => { observed.push(raw); return realReceive(raw); };
  const raw = frame(f.sent.at(-1).request.requestId, { kind: "pending", quoteId });
  f.page.receiveNpcPurchaseGatewayFrame({ data: raw }, 3, f.socket, null, binding);
  assert.deepEqual(observed, [raw], "the actual adapter receives the exact original source bytes");
  for (const invalid of [
    raw.replace('"type":"huntingRepairOwner"', '"type":"huntingRepairOwner","type":"worldSnapshot"'),
    raw.replace('"type"', '"ty\\u0070e"').replace('"snapshot":null', '"snapshot":null,"snapshot":null'),
    raw.replace('"snapshot":null', '"snapshot":{"partial":true}'),
    '{"type":"huntingRepairOwner","reply":',
  ]) {
    assert(repairClient.isHuntingRepairOwnerFrame(invalid), "invalid input still owns the raw repair namespace");
    f.page.receiveNpcPurchaseGatewayFrame({ data: invalid }, 3, f.socket, null, binding);
    assert.equal(observed.at(-1), invalid);
  }
  assert.equal(f.generic.length, 0);
  assert.equal(f.handled.length, 0);
  assert.equal(f.legacy.length, 0);
});

test("actual physical listener captures the old adapter before Hero callbacks can bind a new actor", () => {
  const f = fixture(), original = f.bind();
  assert(original.client.query(quoteId));
  const nextRequestId = String(BigInt(f.sent.at(-1).request.requestId) + 1n);
  let replacement, deliveries = 0;
  f.state.duringHero = () => {
    f.state.duringHero = null;
    f.scope.socialCharacterIndexRef.current.current = 1;
    replacement = f.page.observeHuntingRepairActor(f.socket);
    assert(replacement && replacement !== original);
    assert(replacement.client.query(quoteId));
    assert.equal(f.sent.at(-1).request.requestId, nextRequestId);
    const receive = replacement.client.receive.bind(replacement.client);
    replacement.client.receive = raw => { deliveries++; return receive(raw); };
  };
  const listener = f.page.physicalMessageListener(f.socket, 3);
  listener({ data: frame(nextRequestId, { kind: "pending", quoteId }) });
  assert(replacement);
  assert.equal(original.client.view().phase, "retired");
  assert.equal(deliveries, 0, "a new ref cannot inherit a delivery captured for the old adapter");
  assert.equal(f.generic.length + f.handled.length + f.legacy.length, 0);
  assert.equal(replacement.client.query(quoteId), false, "the original new-actor query remains outstanding");
});

test("actual hunting drag selects a carried source without auto-confirm or legacy hold behavior", () => {
  const f = fixture(); f.bind();
  const view = f.open();
  assert.equal(view.hold, false);
  f.page.toggleNpcRepairHold(view);
  assert.equal(f.page.readNpcRepairView().hold, false);
  const geometry = { pointerId: 1, pointerType: "mouse", page: "bag1", stage: {}, target: {}, item: {},
    stageRect: [0, 0, 800, 600], targetRect: [100, 100, 100, 100], itemRect: [0, 0, 40, 40],
    virtualWidth: 800, virtualHeight: 600, scale: 1, devicePixelRatio: 1 };
  const item = f.scope.worldRef.current.inventoryItems[0];
  const drag = f.page.beginNpcRepairDrag(view, item, geometry);
  assert(drag);
  const dropped = f.page.dropNpcRepairDrag(drag, geometry, 150, 150);
  assert(dropped && dropped.selection.uniqueId === uid);
  assert.equal(dropped.submitted, false);
  assert.equal(f.sent.length, 1, "dropping starts only the owner's quote request");
  assert.equal(f.sent[0].request.action.kind, "quote");
  assert.equal(f.legacy.length, 0);
  assert.equal(f.page.dropNpcRepairDrag(drag, geometry, 150, 150), null, "an original drag is consumed once");
});

test("actual Page preserves a pending repair across close and queries only the original quote", () => {
  const f = fixture(), binding = f.bind(), view = f.open();
  const selection = f.page.selectNpcRepair(view, uid);
  assert(selection);
  assert.equal(f.sent.at(-1).request.action.kind, "quote");
  assert.deepEqual(f.sent.at(-1).request.action, { kind: "quote", uniqueId: String(uid), special: false });
  f.receive({ kind: "quoted", quote: quote() });
  assert.equal(binding.client.view().phase, "quoted");
  assert.equal(f.page.confirmNpcRepair(selection), true);
  assert.deepEqual(f.sent.at(-1).request.action, { kind: "confirm", quoteId });
  f.receive({ kind: "pending", quoteId });
  f.page.closeNpcRepairService();
  assert.equal(f.scope.huntingRepairBindingRef.current, binding);
  assert.equal(binding.client.view().phase, "pending");
  assert.equal(binding.client.view().quoteId, quoteId);
  assert.equal(f.scope.npcRepairServiceRef.current, "repair", "closing Pending preserves its read-only recovery panel");
  const recovery = f.page.readNpcRepairView();
  assert(recovery && recovery.recoveryOnly && recovery.rows.length === 0);
  assert.equal(f.page.confirmNpcRepair(selection), false);
  assert.equal(f.page.queryNpcRepair(), true);
  assert.deepEqual(f.sent.at(-1).request.action, { kind: "query", quoteId });

  // A known receipt with a null snapshot remains pending, preserving both its
  // original identity and the need for a current complete owner projection.
  f.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false });
  assert.equal(binding.client.view().phase, "pending");
  assert.equal(binding.client.view().reason, "snapshotRequired");
  assert.equal(Object.getPrototypeOf(binding.client.view().receipt), null, "the actual strict decoder retains prototype isolation");
  assert(Object.isFrozen(binding.client.view().receipt));
  assert.deepEqual({ ...binding.client.view().receipt }, receipt());
  assert.equal(f.page.queryNpcRepair(), true);
  const repaired = structuredClone(f.raw);
  repaired.inventoryItems[0].durabilityCurrent = 1000;
  repaired.inventoryItems[0].tooltipSource.userItem.current_dura = 1000;
  f.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false }, repaired);
  assert.equal(binding.client.view().phase, "committed");
  assert.equal(f.applied.length, 1);
  assert.deepEqual(f.sent.map(entry => entry.request.action.kind), ["quote", "confirm", "query", "query"]);
  assert.equal(f.legacy.length, 0);
  assert.equal(attemptedFetches, 0, "no identity grant is fabricated to observe a wallet");
});

test("actual Page manual Close keeps original Query reachable after NPC and item readiness disappear", () => {
  for (const mode of ["repair", "special"]) {
    const f = fixture(), binding = f.bind(), selected = f.page.selectNpcRepair(f.open(mode), uid);
    assert(selected);
    f.receive({ kind: "quoted", quote: quote({ special: mode === "special" }) });
    assert(f.page.confirmNpcRepair(selected));
    f.receive({ kind: "unknown", quoteId });
    f.page.closeNpcRepairService(); // The actual user Close callback, with its default hide=true.

    f.scope.npcRepairDialogSourceRef.current = null;
    f.scope.npcRepairDialogBindingRef.current = null;
    f.scope.worldRef.current.activeNpcDialog = null;
    f.scope.worldRef.current.inventoryItems = [];
    f.scope.npcRepairAuthorityRef.current.invalidateInventory();
    f.scope.bagOpenRef.current = false;
    f.state.bagReady = false;

    assert.equal(f.scope.npcRepairServiceRef.current, mode, "the Scene repair-window condition remains enabled without a new NPC packet");
    assert.equal(binding.client.view().visible, true);
    const recovery = f.page.readNpcRepairView();
    assert(recovery && recovery.recoveryOnly && recovery.rows.length === 0);
    assert.equal(recovery.hunting.quoteId, quoteId);
    assert.equal(f.page.selectNpcRepair(recovery, uid), null);
    assert.equal(f.page.confirmNpcRepair(selected), false, "Close irreversibly retires the old item selection authority");
    assert.equal(f.page.queryNpcRepair(), true, "Query needs only the original current actor and operation");
    assert.deepEqual(f.sent.at(-1).request.action, { kind: "query", quoteId });
    assert.equal(f.page.queryNpcRepair(), false, "double-click does not create overlapping queries");
    f.receive({ kind: "pending", quoteId });
    f.page.closeNpcRepairService();
    assert.equal(f.scope.npcRepairServiceRef.current, mode, "repeated Close remains an idempotent read-only recovery handoff");
    assert(f.page.queryNpcRepair());
    assert.deepEqual(f.sent.map(entry => entry.request.action.kind), ["quote", "confirm", "query", "query"]);
    assert(f.sent.slice(2).every(entry => entry.request.action.quoteId === quoteId));
    assert.equal(f.legacy.length, 0);
  }
});

test("actual Page non-pending Close still hides repair and rejects the old quote selection", () => {
  const f = fixture(), binding = f.bind(), selected = f.page.selectNpcRepair(f.open(), uid);
  f.receive({ kind: "quoted", quote: quote() });
  f.page.closeNpcRepairService();
  assert.equal(f.scope.npcRepairServiceRef.current, null);
  assert.equal(binding.client.view().phase, "idle");
  assert.equal(binding.client.view().visible, false);
  assert.equal(f.page.readNpcRepairView(), null);
  assert.equal(f.page.confirmNpcRepair(selected), false);
  assert.equal(f.sent.length, 1);
});

test("actual Page refuses a quoted selection after the full carried-item source changes", () => {
  const f = fixture(); f.bind();
  const selection = f.page.selectNpcRepair(f.open(), uid);
  assert(selection);
  f.receive({ kind: "quoted", quote: quote() });
  const changed = structuredClone(f.raw);
  changed.inventoryItems[0].durabilityCurrent = 400;
  changed.inventoryItems[0].tooltipSource.userItem.current_dura = 400;
  assert(f.scope.npcRepairAuthorityRef.current.observeSnapshot(f.page.readNpcRepairOwner(), changed));
  assert.equal(f.page.confirmNpcRepair(selection), false);
  assert.deepEqual(f.sent.map(entry => entry.request.action.kind), ["quote"]);
  assert.equal(f.legacy.length, 0);
});

test("actual Page updates private identity custody before React and rejects stale or mismatched grants", () => {
  const f = fixture();
  const event = { token: "synthetic_fixture_token", session: {
    accountId: "owner", sessionId: "fixture-session", expiresAtMs: 10_000, revokedAtMs: null,
  } };
  assert(f.page.captureIdentitySessionBinding(event, f.socket, 3));
  const captured = f.scope.identitySessionBindingRef.current;
  assert(Object.isFrozen(captured));
  assert(captured.socket === f.socket && captured.generation === 3 && captured.accountId === "owner");
  assert(captured.token === event.token && captured.sessionId === event.session.sessionId);
  assert.deepEqual(f.tokenUpdates, [{ cleared: false, refUpdatedFirst: true }]);
  for (const [incoming, socket, generation] of [
    [event, {}, 3], [event, f.socket, 2],
    [{ ...event, session: { ...event.session, accountId: "different-owner" } }, f.socket, 3],
    [{ ...event, session: { ...event.session, expiresAtMs: 1000 } }, f.socket, 3],
    [{ ...event, session: { ...event.session, revokedAtMs: 1 } }, f.socket, 3],
    [{ ...event, token: "invalid\nfixture" }, f.socket, 3],
  ]) {
    assert.equal(f.page.captureIdentitySessionBinding(incoming, socket, generation), false);
    assert.equal(f.scope.identitySessionBindingRef.current, captured);
  }
  assert.equal(f.tokenUpdates.length, 1, "rejected grants never reach the state setter");
  f.page.clearIdentitySessionToken();
  assert.equal(f.scope.identitySessionBindingRef.current, null);
  assert.deepEqual(f.tokenUpdates.at(-1), { cleared: true, refUpdatedFirst: true });
  assert.equal(attemptedFetches, 0);
});

test("actual Page clear retires the existing real wallet without retiring the repair actor", async () => {
  const f = fixture(), binding = f.bind();
  const identity = Object.freeze({ token: "synthetic_fixture_token", sessionId: "fixture-session",
    accountId: "owner", expiresAtMs: 10_000, socket: f.socket, generation: 3 });
  f.scope.identitySessionBindingRef.current = identity;
  const wallet = new HuntingRepairWalletClient({ owner: f.socket, actorScope: binding.actor, identity,
    characterIndex: 0, isCurrent: () => true });
  let retired = 0;
  const retire = wallet.retire.bind(wallet);
  wallet.retire = () => { retired++; retire(); };
  binding.walletClient = wallet;
  binding.walletIdentity = identity;
  f.page.clearIdentitySessionToken();
  assert.equal(retired, 1);
  assert.equal(binding.walletClient, null);
  assert.equal(binding.walletIdentity, null);
  assert.equal(f.scope.identitySessionBindingRef.current, null);
  assert(f.page.currentHuntingRepairBinding(binding), "observational wallet custody does not replace the repair actor");
  assert.equal(await wallet.refresh(), false, "a retired real wallet cannot enter a fetch even with a true owner predicate");
  assert.equal(attemptedFetches, 0);
});

test("actual NPC repair cases select hunting or legacy service from this physical handshake only", () => {
  for (const paymentMode of ["hunting-v1", "legacy"]) for (const packet of ["NPCRepair", "NPCSRepair"]) {
    const f = fixture();
    const binding = paymentMode === "hunting-v1" ? f.bind() : null;
    if (!binding) f.page.captureHuntingRepairMode({ npcRepairMode: "legacy" }, f.socket, 3);
    const owner = f.page.readNpcRepairOwner();
    f.page.captureNpcRepairDialog(f.raw, owner);
    const service = f.scope.npcRepairAuthorityRef.current, opened = [], original = [];
    const openHunting = service.openHunting.bind(service), openLegacy = service.open.bind(service);
    service.openHunting = (...args) => { opened.push(args); return openHunting(...args); };
    service.open = (...args) => { original.push(args); return openLegacy(...args); };
    let identityReads = 0;
    Object.defineProperty(f.scope.identitySessionBindingRef, "current", {
      get() { identityReads++; return null; }, set(value) { assert.equal(value, null); }, configurable: true,
    });
    const event = { type: "packet", packet, payload: { rate: paymentMode === "legacy" ? 1 : "legacy-rate-is-not-authority" } };
    // Both stale socket and stale generation fail before opening a pane.
    f.page.actualNpcRepairPacket(event, 3, {});
    f.page.actualNpcRepairPacket(event, 2, f.socket);
    assert.equal(opened.length + original.length, 0);
    f.page.actualNpcRepairPacket(event, 3, f.socket);
    const mode = packet === "NPCSRepair" ? "special" : "repair";
    assert.equal(f.scope.npcRepairServiceRef.current, mode);
    assert.equal(f.scope.npcRepairDialogBindingRef.current, f.scope.npcRepairDialogSourceRef.current);
    assert(f.paneChanges.some(value => value.inventory === true));
    assert(f.paneChanges.some(value => value.character === false));
    if (binding) {
      assert.equal(opened.length, 1);
      assert.equal(original.length, 0, "hunting never opens the legacy rate service");
      assert.equal(opened[0][1], mode);
      assert.equal(binding.client.view().visible, true);
      assert(identityReads > 0, "the actual opening invokes the read-only wallet refresh gate");
      assert.equal(binding.walletClient, null, "a missing identity grant remains unknown");
    } else {
      assert.equal(opened.length, 0);
      assert.equal(original.length, 1);
      assert.equal(original[0][1], mode);
      assert.equal(original[0][2], 1);
      assert.equal(identityReads, 0);
    }
    assert.equal(f.sent.length + f.legacy.length, 0, "opening never confirms a repair or submits a wallet mutation");
  }
  assert.equal(attemptedFetches, 0);
});
