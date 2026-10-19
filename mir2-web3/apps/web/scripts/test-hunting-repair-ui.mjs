import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
const ts = createRequire(import.meta.url)(process.env.MIR2_TEST_TYPESCRIPT_PATH ?? "typescript");
const read = name => readFileSync(new URL(name, import.meta.url), "utf8");
const options = { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.React, jsxFactory: "element" };
const files = { service: "../lib/npc-repair-service.ts", equipment: "../lib/equipment-gateway-adapter.ts",
  identity: "../lib/world-model/item-identity.ts", client: "../lib/hunting-repair-client.ts", purchase: "../lib/npc-purchase-client.ts" };
const imports = { service: { "./equipment-gateway-adapter": "equipment" }, equipment: { "./world-model/item-identity": "identity" },
  identity: {}, client: { "./npc-purchase-client": "purchase" }, purchase: {} };
const cache = new Map();
function load(name) {
  if (cache.has(name)) return cache.get(name);
  assert(Object.hasOwn(files, name)); const module = { exports: {} }; cache.set(name, module.exports);
  const compiled = ts.transpileModule(read(files[name]), { compilerOptions: options, reportDiagnostics: true });
  assert.equal((compiled.diagnostics ?? []).filter(v => v.category === ts.DiagnosticCategory.Error).length, 0);
  new Function("require", "module", "exports", compiled.outputText)(id => {
    assert(Object.hasOwn(imports[name], id), "Unexpected runtime dependency " + id); return load(imports[name][id]);
  }, module, module.exports); return module.exports;
}
const { NpcRepairService, projectNpcRepairBag } = load("service");
const { HuntingRepairClient } = load("client");
const quoteId = "a".repeat(64);
function item(id = 900, slot = 7, container = "bag1") {
  return { uniqueId: id, slot, container, name: "Sword", icon: 1, quantity: 1, durabilityCurrent: 40, durabilityMax: 100,
    tooltipSource: { info: { item_index: 77, price: 1000, durability: 100 }, userItem: { unique_id: id, item_index: 77,
      count: 1, current_dura: 40, max_dura: 100, added_stats: [{ stat: 5, value: -3 }], rental_information: null } } };
}
function snapshot(patch = {}) { return { playerObjectId: 17, mapFileName: "M001.map", gold: 500, inventoryCapacity: 86,
  inventoryItems: [item(), item(901, 3, "bag2")], beltItems: [], equipmentItems: [], storageItems: [], storageSize: 80, ...patch }; }
function owner(patch = {}) { return { socket: {}, connectionGeneration: 4, sessionGeneration: 8, ownerRevision: 3,
  playerObjectId: 17, sceneRevision: 12, mapFileName: "M001.map", ...patch }; }
function state(patch = {}) { return { phase: "idle", visible: true, quote: null, quoteId: null, receipt: null, reason: null,
  reloadRequired: false, ...patch }; }
const serverQuote = (patch = {}) => ({ quoteId, uniqueId: "900", special: false, costBoundGold: "18446744073709551615",
  createdAtMs: 1000, expiresAtMs: 31000, ...patch });
function fixture() {
  const service = new NpcRepairService(), who = owner(), source = Object.freeze({ npcObjectId: 55 });
  assert(service.openHunting(who, "repair", source, "Blacksmith")); assert(service.observeSnapshot(who, snapshot()));
  let status = state(), allowed = true;
  return { service, owner: who, source, view: wallet => service.viewHunting(who, status, () => allowed, wallet),
    get state() { return status; }, set state(value) { status = value; }, set allowed(value) { allowed = value; } };
}
const legacyQuote = () => ({ repairPrice: 21, totalPrice: 21, displayedTotal: 21, affordable: true });
function geometry(patch = {}) { return { pointerId: 1, pointerType: "mouse", page: "bag1", stage: {}, target: {}, item: {},
  stageRect: [0, 0, 1024, 768], targetRect: [100, 100, 75, 75], itemRect: [1, 1, 36, 32],
  virtualWidth: 1024, virtualHeight: 768, scale: 1, devicePixelRatio: 1, ...patch }; }

test("bound view does not invoke legacy pricing or expose legacy gold/affordability", () => {
  const f = fixture(); let readQuotes = 0;
  assert.equal(f.service.view(f.owner, () => { ++readQuotes; return legacyQuote(); }, () => true), null);
  assert.equal(readQuotes, 0); const view = f.view();
  assert.equal(view.paymentMode, "hunting-v1"); assert.equal(view.gold, null); assert.equal(view.hold, false);
  assert(view.rows.every(row => row.quote === null && !Object.hasOwn(row, "affordable")));
  assert.equal(f.service.toggleHold(view, view), null);
  const selection = f.service.select(view, 900, view); assert(selection);
  assert.equal(f.service.reserve(selection, view), null);
  assert.deepEqual(f.service.huntingIntent(selection, view), { uniqueId: "900", special: false });
});
test("bound source accepts missing legacy gold and displays only observed u64 wallets", () => {
  const f = fixture(), raw = snapshot(); delete raw.gold;
  assert(f.service.observeSnapshot(f.owner, raw));
  const wallet = { boundGold: "18446744073709551615", unboundGold: "0" }, view = f.view(wallet);
  assert(view); assert.equal(view.gold, null); assert.deepEqual(view.wallet, wallet); assert(Object.isFrozen(view.wallet));
  assert.equal(f.view({ boundGold: "01", unboundGold: "0" }).wallet, null);
  assert.equal(f.view({ boundGold: "18446744073709551616", unboundGold: "0" }).wallet, null);
  assert.equal(projectNpcRepairBag(raw), null, "legacy full source keeps its gold requirement");
});
test("quote, receipt and wallet updates preserve carried-source stamp and existing selection", () => {
  const f = fixture(), first = f.view(), selection = f.service.select(first, 900, first); assert(selection);
  for (const phase of ["quoting", "quoted", "pending", "committed"]) {
    f.state = state({ phase, quoteId, quote: serverQuote() }); const next = f.view({ boundGold: "100", unboundGold: "50" });
    assert.equal(next.stamp, first.stamp); assert.deepEqual(f.service.huntingIntent(selection, next), { uniqueId: "900", special: false });
    if (phase === "pending") assert.equal(f.service.select(next, 901, next), null);
  }
});
test("real source mutation, unknown interval and actor/NPC changes retire original proof", () => {
  for (const change of [f => { const raw = snapshot(); raw.inventoryItems[0].tooltipSource.userItem.added_stats[0].value = -2;
    f.service.prepareSnapshot(f.owner, raw); f.service.observeSnapshot(f.owner, raw); },
  f => { f.service.invalidateInventory(); f.service.observeSnapshot(f.owner, snapshot()); },
  f => f.service.openHunting(f.owner, "repair", {}),
  f => f.service.viewHunting({ ...f.owner, ownerRevision: 4 }, state(), () => true)]) {
    const f = fixture(), before = f.view(), selected = f.service.select(before, 900, before); assert(selected);
    change(f); assert.equal(f.service.huntingIntent(selected, f.view()), null);
  }
});
test("current locks are checked without conflating them with transport stamp", () => {
  const f = fixture(), before = f.view(), selected = f.service.select(before, 900, before); f.allowed = false;
  const after = f.view(); assert.equal(before.stamp, after.stamp); assert.equal(f.service.huntingIntent(selected, after), null);
  assert.equal(f.service.select(before, 900, after), null);
});
test("bound drag preserves exact geometry and only returns an unsubmitted source selection", () => {
  const f = fixture(), view = f.view(), g = geometry(), drag = f.service.beginDrag(view, 900, 7, g, view); assert(drag);
  const selection = f.service.drop(drag, g, 110, 110, view); assert(selection);
  assert.deepEqual(f.service.huntingIntent(selection, view), { uniqueId: "900", special: false });
  assert.equal(f.service.drop(drag, g, 110, 110, view), null);
  for (const mutate of [g => ({ ...g, target: {} }), g => ({ ...g, targetRect: [101, 100, 75, 75] }), g => ({ ...g, page: "bag2" })]) {
    const g2 = geometry(), token = f.service.beginDrag(view, 900, 7, g2, view); assert(token);
    assert.equal(f.service.drop(token, mutate(g2), 110, 110, view), null);
  }
});
test("pending forbids new selections and drags but keeps non-consuming original proof", () => {
  const f = fixture(), before = f.view(), selected = f.service.select(before, 900, before);
  f.state = state({ phase: "pending", quoteId, quote: serverQuote() }); const pending = f.view();
  assert.equal(f.service.select(pending, 901, pending), null); assert.equal(f.service.beginDrag(pending, 901, 43, geometry({ page: "bag2" }), pending), null);
  assert(f.service.huntingIntent(selected, pending)); assert(f.service.huntingIntent(selected, pending));
});
test("close and missing target preserve a recovery-only view incapable of issuing selection/proof", () => {
  const f = fixture(); f.state = state({ phase: "pending", quoteId, quote: serverQuote() }); f.service.close(); f.service.invalidateInventory();
  const view = f.view(); assert(view.recoveryOnly); assert.deepEqual(view.rows, []); assert.equal(view.hunting.quoteId, quoteId);
  assert.equal(f.service.select(view, 900, view), null); assert.equal(f.service.beginDrag(view, 900, 7, geometry(), view), null);
  f.state = state(); assert.equal(f.view(), null);
});
test("unsafe u64 item identities cannot become bound repair requests", () => {
  for (const id of [Number.MAX_SAFE_INTEGER + 1, "9007199254740993"]) {
    const f = fixture(); f.service.observeSnapshot(f.owner, snapshot({ inventoryItems: [item(id)] }));
    const view = f.view(); assert.equal(view, null); // Whole raw identity source remains unproven.
  }
  const f = fixture(); assert.equal(f.service.huntingIntent({ stamp: f.view().stamp, uniqueId: 900 }, f.view()), null);
});
test("legacy quote, affordability, entered barrier and Hold semantics remain intact", () => {
  const service = new NpcRepairService(), who = owner(); assert(service.open(who, "repair", 0.5, {})); assert(service.observeSnapshot(who, snapshot()));
  const poor = service.view(who, () => ({ ...legacyQuote(), affordable: false }), () => true);
  assert.equal(poor.gold, 500); assert.equal(poor.rows[0].reason, "gold"); const selection = service.select(poor, 900, poor); assert(selection);
  assert.equal(service.reserve(selection, poor), null);
  const current = service.view(who, legacyQuote, () => true); assert.equal(service.toggleHold(current, current), true);
  const proof = service.reserve(service.select(current, 900, current), current); assert(proof); assert(service.enter(proof, current, proof.command));
  const busy = service.view(who, legacyQuote, () => true); assert.equal(busy.rows[0].reason, "busy");
  service.close(); assert(service.open(who, "repair", 0.5, {})); assert.equal(service.view(who, legacyQuote, () => true).rows[0].reason, "busy");
});
test("real owner adapter Confirm can recheck existing source after changing phase to pending", () => {
  const f = fixture(), sent = [], initial = f.view(), selection = f.service.select(initial, 900, initial); let live = initial;
  const client = new HuntingRepairClient({ owner: f.owner.socket, actorScope: Object.freeze({ actor: 17 }), mode: "hunting-v1", now: () => 1000,
    isCurrent: socket => socket === f.owner.socket, selectionMatches: (_socket, requested) => {
      const proof = f.service.huntingIntent(selection, live); return !!proof && proof.uniqueId === requested.uniqueId && proof.special === requested.special;
    }, send: (_socket, raw) => sent.push(JSON.parse(raw)), applySnapshot: () => false,
    onChange: next => { f.state = next; live = f.view(); } });
  assert(client.quote({ uniqueId: "900", special: false }));
  assert(client.receive(JSON.stringify({ type: "huntingRepairOwner", protocolVersion: 1, requestId: sent[0].requestId,
    reply: { kind: "quoted", quote: serverQuote() }, snapshot: null })));
  assert(client.confirm()); assert.equal(sent[1].action.kind, "confirm"); assert.equal(client.confirm(), false);
  assert.equal(sent.filter(v => v.action.kind === "confirm").length, 1);
});

function tree(text, filename) { return ts.createSourceFile(filename, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX); }
function functions(ast, names) { const found = new Map(); const visit = node => {
  if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) found.set(node.name.text, node.getText(ast).replace(/^export\s+/, ""));
  ts.forEachChild(node, visit); }; visit(ast); return names.map(name => { assert(found.has(name), name); return found.get(name); }).join("\n"); }
const shopAst = tree(read("../app/components/original-client-game-shop.tsx"), "shop.tsx");
function shopFixture(view, selection = null, blocked = false) {
  const calls = { confirm: [], query: 0, hold: 0, close: 0 }, states = [], scope = {
    useState(initial) { const i = hook++; if (!(i in states)) states[i] = initial; return [states[i], value => { states[i] = typeof value === "function" ? value(states[i]) : value; }]; },
    useMemo: fn => fn(), useEffect: () => {}, useLayoutEffect: () => {}, useRef: () => ({ current: null }),
    element: (tag, props, ...children) => ({ tag, props: props ?? {}, children }),
    ORIGINAL_UI: { inventory: { closeButton: {} } }, shopStyle: {}, NPC_SHOP_GRADE_COLOUR: {}, SpriteButton: "SpriteButton",
    originalAssetPath: path => path, originalItemIconPath: () => "/item" };
  const code = ts.transpileModule(functions(shopAst, ["NpcShopWindow", "npcShopBuyBlockedTitle", "npcShopTabLabel", "formatGold", "confirmEnabledFor"]),
    { compilerOptions: options }).outputText;
  const component = new Function(...Object.keys(scope), code + "\nreturn NpcShopWindow;")(...Object.values(scope));
  const props = { t: (_key, _args, fallback) => fallback, onClose: () => ++calls.close,
    repairView: view, repairTargetSelection: selection, initialTab: "repair", availableTabs: ["repair"], inputBlocked: blocked,
    onConfirmRepair: selected => { calls.confirm.push(selected); return true; }, onQueryRepair: () => { ++calls.query; return true; }, onToggleRepairHold: () => ++calls.hold };
  let hook = 0, rendered;
  const render = () => { hook = 0; rendered = component(props); return rendered; }; render();
  const findAll = predicate => { const found = []; const visit = node => { if (Array.isArray(node)) { node.forEach(visit); return; }
    if (!node || typeof node !== "object") return; if (predicate(node)) found.push(node); node.children?.forEach(visit); }; visit(rendered); return found; };
  const control = name => { const parent = findAll(node => node.props["data-npc-repair-control"] === name)[0]; assert(parent, name);
    return parent.tag === "button" ? parent : parent.children[0]; };
  const text = () => { const out = []; const visit = node => { if (Array.isArray(node)) node.forEach(visit); else if (typeof node === "string" || typeof node === "number") out.push(String(node));
    else node?.children?.forEach(visit); }; visit(rendered); return out.join(" "); };
  return { props, calls, render, control, text, findAll };
}
test("actual Shop displays server u64 fee and observed wallets, disables Hold, and only confirms matching quote", () => {
  const f = fixture(); f.state = state({ phase: "quoted", quoteId, quote: serverQuote() }); const view = f.view({ boundGold: "0", unboundGold: "123" });
  const selected = f.service.select(view, 900, view), ui = shopFixture(view, selected);
  assert(ui.text().includes("18,446,744,073,709,551,615")); assert(ui.text().includes("Bound"));
  assert.equal(ui.control("hold").props.disabled, true); ui.control("hold").props.onClick(); assert.equal(ui.calls.hold, 0);
  assert.equal(ui.control("confirm").props.disabled, false); ui.control("confirm").props.onClick(); assert.deepEqual(ui.calls.confirm, [selected]);
  ui.props.repairView = { ...view, hunting: state({ phase: "quoted", quoteId, quote: serverQuote({ uniqueId: "901" }) }) }; ui.render();
  assert.equal(ui.control("confirm").props.disabled, true); ui.control("confirm").props.onClick(); assert.equal(ui.calls.confirm.length, 1);
});
test("actual Shop pending Query works without target, NPC source, funds or mutation readiness", () => {
  const f = fixture(); f.state = state({ phase: "pending", quoteId, quote: serverQuote() }); f.service.close(); f.service.invalidateInventory();
  const ui = shopFixture(f.view(), null, true); assert.equal(ui.control("confirm").props.disabled, true);
  assert.equal(ui.control("query").props.disabled, false); ui.control("query").props.onClick(); assert.equal(ui.calls.query, 1);
  assert.equal(ui.calls.confirm.length, 0); assert(ui.text().includes("Pending"));
});
test("actual Shop fresh selection or quoting cannot trigger Confirm", () => {
  for (const phase of ["idle", "quoting", "rejected", "pending", "committed"]) {
    const f = fixture(), first = f.view(), selected = f.service.select(first, 900, first);
    f.state = state({ phase, quoteId, quote: serverQuote() }); const ui = shopFixture(f.view(), selected);
    assert.equal(ui.control("confirm").props.disabled, true, phase); ui.control("confirm").props.onClick(); assert.equal(ui.calls.confirm.length, 0, phase);
  }
});
test("actual Shell preserves bound target across accepted or rejected Confirm and retains legacy clearing", () => {
  const ast = tree(read("../app/original-client-shell.tsx"), "shell.tsx");
  for (const paymentMode of ["legacy", "hunting-v1"]) for (const accepted of [true, false]) {
    const selected = {}, ref = { current: selected }, cleared = [];
    const scope = { npcRepairView: { paymentMode }, onConfirmNpcRepair: () => accepted, npcRepairVisibleTargetRef: ref,
      changeNpcRepairTarget: value => cleared.push(value) };
    const code = ts.transpileModule(functions(ast, ["confirmNpcRepairTarget"]), { compilerOptions: options }).outputText;
    const fn = new Function(...Object.keys(scope), code + "\nreturn confirmNpcRepairTarget;")(...Object.values(scope));
    assert.equal(fn(selected), accepted); assert.deepEqual(cleared, paymentMode === "legacy" ? [null] : []);
  }
});
test("Query contract is forwarded independently through actual Shell/Scene props", () => {
  for (const name of ["../app/components/original-client-shell-types.ts", "../app/components/original-client-game-ui-scene.tsx", "../app/original-client-shell.tsx"])
    assert(read(name).includes("onQueryNpcRepair"), name);
  assert(read("../app/components/original-client-game-ui-scene.tsx").includes("onQueryRepair={onQueryNpcRepair}"));
});
