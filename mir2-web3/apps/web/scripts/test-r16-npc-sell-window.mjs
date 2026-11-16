import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const read = path => readFileSync(new URL(path, import.meta.url), "utf8");
const helperModule = { exports: {} };
new Function("exports", "module", ts.transpileModule(read("../lib/r16-npc-shop-layout.ts"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText)(helperModule.exports, helperModule);
const api = helperModule.exports;
const shopSource = read("../app/components/original-client-game-shop.tsx");
const ast = ts.createSourceFile("actual-shop.tsx", shopSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const functionText = name => {
  const found = ast.statements.filter(node => ts.isFunctionDeclaration(node) && node.name?.text === name);
  assert.equal(found.length, 1, "actual function " + name); return found[0].getText(ast).replace(/^export\s+/, "");
};
const element = (tag, props, ...children) => ({ tag, props: props ?? {}, children });
function nodes(tree) {
  return Array.isArray(tree) ? tree.flatMap(nodes) : tree && typeof tree === "object" ? [tree, ...nodes(tree.children)] : [];
}
function strings(tree) {
  return Array.isArray(tree) ? tree.map(strings).join("") : tree && typeof tree === "object" ? strings(tree.children) : typeof tree === "string" || typeof tree === "number" ? String(tree) : "";
}
const translate = (key, _args, fallback) => fallback ?? key;
function fixture(extra = {}) {
  const slots = [], effects = new Map(), layoutEffects = new Map(), cleanups = new Map();
  let cursor = 0, dirty = false, tree;
  const calls = [], tabs = [], document = { activeElement: null }, props = {
    t: translate, npcName: "Merchant", items: [{ id: 917, name: "BaseDress(M)", icon: 100, price: 12, count: 1 }], tabs: ["buy", "sell"],
    blocked: () => false, inputBlocked: false, onSell: (id, quantity) => calls.push([id, quantity]), onBuy: () => tabs.push("buy"), onClose() {}, ...extra,
  };
  const effect = queue => (fn, deps) => {
    const index = cursor++, before = slots[index];
    if (!before || !deps || deps.some((value, at) => !Object.is(value, before[at]))) { slots[index] = deps; queue.set(index, fn); }
  };
  const scope = { ...api, document, element, formatGold: value => Math.max(0, Math.trunc(value)).toLocaleString("en-US"),
    originalAssetPath: path => path, originalItemIconPath: icon => "/item/" + icon + ".png",
    ORIGINAL_UI: { inventory: { closeButton: { base: "/close.png", hover: "/close-hover.png", pressed: "/close-pressed.png" } } },
    useState(initial) { const index = cursor++; if (!(index in slots)) slots[index] = initial;
      return [slots[index], value => { const next = typeof value === "function" ? value(slots[index]) : value; if (!Object.is(next, slots[index])) { slots[index] = next; dirty = true; } }]; },
    useRef(initial) { const index = cursor++; if (!(index in slots)) slots[index] = { current: initial }; return slots[index]; },
    useLayoutEffect: effect(layoutEffects), useEffect: effect(effects),
  };
  const input = functionText("NpcSellServiceWindow");
  const code = ts.transpileModule(input, { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.React, jsxFactory: "element", jsxFragmentFactory: "fragment" } }).outputText;
  const component = new Function(...Object.keys(scope), "fragment", code + "\nreturn NpcSellServiceWindow;")(...Object.values(scope), "fragment");
  const flush = queue => { for (const [index, fn] of [...queue]) { queue.delete(index); cleanups.get(index)?.(); cleanups.set(index, fn()); } };
  function render(changes = {}, { passive = true } = {}) {
    Object.assign(props, changes);
    for (let round = 0; round < 6; round++) {
      cursor = 0; dirty = false; tree = component(props);
      for (const node of nodes(tree)) {
        node.focus = () => { document.activeElement = node; };
        if (node.props.ref) {
          node.querySelectorAll = () => nodes(node).filter(child => child.tag === "button" && !child.props.disabled);
          node.querySelector = () => node.querySelectorAll()[0]; node.props.ref.current = node;
        }
      }
      flush(layoutEffects); if (passive) flush(effects); if (!dirty) return tree;
    }
    assert.fail("unexpected render loop");
  }
  const one = predicate => { const found = nodes(tree).filter(predicate); assert.equal(found.length, 1); return found[0]; };
  const button = name => one(node => node.tag === "button" && node.props.className?.split(" ").includes(name));
  const open = () => { one(node => node.props["data-r16-npc-sell-open"] === "true").props.onClick(); render(); };
  const choose = (id = props.items[0].id) => { one(node => node.props.className === "npc-shop-row" && node.props["data-item-id"] === String(id)).props.onClick(); render(); };
  render();
  return { props, calls, tabs, render, one, button, open, choose, get tree() { return tree; }, get text() { return strings(tree); },
    dispose() { for (const cleanup of cleanups.values()) cleanup?.(); } };
}

test("Sell frame, target and original confirm match Native source; no fake Hold is rendered", () => {
  const native = read("../../game-client/client-bevy/src/crystal_ui/npc_item_service.rs"), layout = api.R16_NPC_SELL_LAYOUT;
  assert.match(native, /CONFIRM: CrystalRect = CrystalRect::new\(114\.0, 62\.0, 48\.0, 25\.0\)/);
  assert.match(native, /ITEM: CrystalRect = CrystalRect::new\(38\.0, 72\.0, 36\.0, 32\.0\)/);
  assert.match(native, /ITEM_CLICK_TARGET: CrystalRect = CrystalRect::new\(20\.0, 55\.0, 75\.0, 75\.0\)/);
  const f = fixture();
  assert.deepEqual(layout.panel, { left: 264, top: 224, width: 176, height: 147 });
  assert.ok(nodes(f.tree).some(node => node.tag === "img" && node.props.src === "/original-ui/Prguse2/351.png"));
  assert.equal(f.button("npc-shop-confirm").props.style.left, layout.confirm.left);
  assert.equal(f.button("npc-shop-confirm").props.style["--r16-sell-art"], 'url("/original-ui/Title/290.png")');
  assert.equal(f.button("npc-shop-confirm").props["data-shop-tab-key"], "sell");
  assert.equal(f.button("npc-shop-confirm").props.disabled, true); assert(f.text.includes("选择背包物品"));
  assert.ok(!nodes(f.tree).some(node => /hold/i.test(node.props["aria-label"] ?? "")));
});

test("choosing a real owned UID shows its quoted price and invokes only existing Sell on confirmation", () => {
  const f = fixture(); f.open();
  assert(f.one(node => node.props["data-r16-npc-sell-picker"] === "true"));
  const row = f.one(node => node.props.className === "npc-shop-row"); assert.equal(row.props["data-unit-price"], 12);
  f.choose(); assert.deepEqual(f.calls, []); assert(!nodes(f.tree).some(node => node.props["data-r16-npc-sell-picker"]));
  assert(f.text.includes("Sell: 12")); assert.equal(f.button("npc-shop-confirm").props.disabled, false);
  f.button("npc-shop-confirm").props.onClick(); assert.deepEqual(f.calls, [[917, 1]]);
  assert.ok(!/sold|success|已出售|出售成功|已到账/i.test(f.text), "a callback is not a receipt");
});

test("stack amount remains explicit, bounded and reflected in the display total", () => {
  const f = fixture({ items: [{ id: "9007199254740993", name: "HPDrug", icon: 1, price: 4, count: 9 }] }); f.open(); f.choose();
  f.one(node => node.tag === "input").props.onChange({ target: { value: "3" } }); f.render();
  assert(f.text.includes("Sell: 12")); f.button("npc-shop-confirm").props.onClick(); assert.deepEqual(f.calls, [["9007199254740993", 3]]);
  f.one(node => node.tag === "input").props.onChange({ target: { value: "bad" } }); f.render();
  assert.equal(f.button("npc-shop-confirm").props.disabled, true); f.button("npc-shop-confirm").props.onClick(); assert.equal(f.calls.length, 1);
  assert.equal(api.r16NpcSellQuantity("100", 9), 9); assert.equal(api.r16NpcSellQuantity("100000", 100000), 65535);
});

test("fresh blocking rejects stale choose, confirm, amount, close and Buy edges", () => {
  let blocked = false, closed = 0;
  const f = fixture({ blocked: () => blocked, onClose: () => closed++ }); f.open();
  const row = f.one(node => node.props.className === "npc-shop-row"); blocked = true; row.props.onClick(); f.render();
  assert.equal(f.button("npc-shop-confirm").props.disabled, true); assert.deepEqual(f.calls, []);
  blocked = false; f.choose(); const confirm = f.button("npc-shop-confirm"), close = f.button("npc-shop-close");
  const buy = f.one(node => node.props["data-shop-tab-key"] === "buy"); blocked = true;
  confirm.props.onClick(); close.props.onClick(); buy.props.onClick(); assert.deepEqual(f.calls, []); assert.deepEqual(f.tabs, []); assert.equal(closed, 0);
  blocked = false;
  const stack = fixture({ blocked: () => blocked, items: [{ id: 8, name: "Stack", icon: 1, price: 4, count: 9 }] }); stack.open(); stack.choose();
  const input = stack.one(node => node.tag === "input"); blocked = true; input.props.onChange({ target: { value: "3" } }); stack.render();
  assert.equal(stack.one(node => node.tag === "input").props.value, "1"); assert.deepEqual(stack.calls, []);
});

test("same UID with a changed price/count/name/source cannot retain or submit an old selection", () => {
  for (const change of [item => ({ ...item, price: 99 }), item => ({ ...item, count: 2 }), item => ({ ...item, name: "Replacement" })]) {
    const f = fixture(); f.open(); f.choose(); const stale = f.button("npc-shop-confirm");
    f.render({ items: [change(f.props.items[0])] }, { passive: false });
    assert.equal(f.button("npc-shop-confirm").props.disabled, true); assert(f.text.includes("选择背包物品"));
    stale.props.onClick(); assert.deepEqual(f.calls, []);
  }
  const f = fixture(); f.open(); f.choose(); const stale = f.button("npc-shop-confirm"); f.render({ npcName: "Other merchant" });
  assert.equal(f.button("npc-shop-confirm").props.disabled, true); stale.props.onClick(); assert.deepEqual(f.calls, []);
  const mutated = fixture(); mutated.open(); mutated.choose(); mutated.props.items[0].price = 99;
  mutated.button("npc-shop-confirm").props.onClick(); assert.deepEqual(mutated.calls, [], "fresh target facts also reject in-place changes before a new render");
});

test("new array objects with identical current facts keep a legitimate selection usable", () => {
  const f = fixture(); f.open(); f.choose(); f.render({ items: f.props.items.map(item => ({ ...item })), tabs: [...f.props.tabs] });
  assert.equal(f.button("npc-shop-confirm").props.disabled, false); f.button("npc-shop-confirm").props.onClick(); assert.deepEqual(f.calls, [[917, 1]]);
});

test("service unmount clears the old render lease and fresh service starts without a selected UID", () => {
  const f = fixture(); f.open(); f.choose(); const stale = f.button("npc-shop-confirm"); f.dispose(); stale.props.onClick(); assert.deepEqual(f.calls, []);
  const fresh = fixture(); assert.equal(fresh.button("npc-shop-confirm").props.disabled, true); assert(fresh.text.includes("选择背包物品"));
});

test("disabled/duplicate/missing owned UID facts never authorize selection", () => {
  const items = [{ id: 7, name: "Own", icon: 1, price: 4, count: 1 }], key = api.r16NpcSellSourceKey("M", ["sell"], items);
  assert.equal(api.r16NpcSellCapture(key, items, "7"), null); assert.equal(api.r16NpcSellCapture(key, [...items, { ...items[0] }], 7), null);
  const selection = api.r16NpcSellCapture(key, items, 7); assert.ok(selection); assert.equal(api.r16NpcSellTarget(selection, key, []), null);
  const f = fixture({ items: [{ ...items[0], disabled: true }] }); f.open(); assert.equal(f.one(node => node.props.className === "npc-shop-row").props.disabled, true);
  f.choose(); assert.equal(f.button("npc-shop-confirm").props.disabled, true); assert.deepEqual(f.calls, []);
});

test("Sell navigation and close retain original callbacks without a sale", () => {
  let closed = 0; const f = fixture({ onClose: () => closed++ });
  f.one(node => node.props["data-shop-tab-key"] === "buy").props.onClick(); f.button("npc-shop-close").props.onClick();
  assert.deepEqual(f.tabs, ["buy"]); assert.equal(closed, 1); assert.deepEqual(f.calls, []);
});

test("actual NpcShopWindow routes Sell to the leaf and Buy back through the existing controlled-tab callback", () => {
  const tabChanges = [], items = [{ id: 917, name: "BaseDress(M)", icon: 100, price: 12, count: 1 }]; let blocked = false;
  const leaf = () => {}, onSell = () => {}, scope = { element, NpcSellServiceWindow: leaf,
    useState: value => [value, () => {}], useMemo: fn => fn(), useEffect() {}, useLayoutEffect() {}, useRef: value => ({ current: value }) };
  const code = ts.transpileModule(functionText("NpcShopWindow"), { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.React, jsxFactory: "element" } }).outputText;
  const component = new Function(...Object.keys(scope), code + "\nreturn NpcShopWindow;")(...Object.values(scope));
  const tree = component({ t: translate, tab: "sell", initialTab: "buy", availableTabs: ["buy", "sell"], sellItems: items, onSell,
    getInputBlocked: () => blocked, onTabChange: tab => tabChanges.push(tab), onClose() {} });
  assert.equal(tree.tag, leaf); assert.equal(tree.props.items, items); assert.equal(tree.props.onSell, onSell);
  tree.props.onBuy(); assert.deepEqual(tabChanges, ["buy"]); blocked = true; tree.props.onBuy(); assert.deepEqual(tabChanges, ["buy"]);
});
