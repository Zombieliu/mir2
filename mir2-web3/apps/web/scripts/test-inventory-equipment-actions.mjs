import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import { fileURLToPath } from "node:url";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import ts from "typescript";

const nativeRequire = createRequire(import.meta.url);
const cache = new Map();
function load(url) {
  if (cache.has(url.href)) return cache.get(url.href);
  if (url.pathname.endsWith(".json")) return JSON.parse(readFileSync(url, "utf8"));
  const javascript = ts.transpileModule(readFileSync(url, "utf8"), {
    fileName: fileURLToPath(url),
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
      jsx: ts.JsxEmit.ReactJSX, esModuleInterop: true },
  }).outputText;
  const module = { exports: {} };
  new Function("require", "exports", "module", javascript)(request => {
    if (!request.startsWith(".")) return nativeRequire(request);
    const dependency = new URL(request, url);
    if (!/\.(?:ts|tsx|json)$/.test(dependency.pathname)) dependency.pathname += ".ts";
    return load(dependency);
  }, module.exports, module);
  cache.set(url.href, module.exports);
  return module.exports;
}
const utils = load(new URL("../app/components/original-client-inventory-utils.ts", import.meta.url));
const panels = load(new URL("../app/components/original-client-inventory-action-panels.tsx", import.meta.url));

// Execute today's actual component handlers, including the shared metadata and
// slot resolver. Callbacks record the requested operation; no server is mocked.
const sourceUrl = new URL("../app/components/original-client-inventory-window.tsx", import.meta.url);
const source = ts.createSourceFile(fileURLToPath(sourceUrl), readFileSync(sourceUrl, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const handlers = new Map();
const cardHandlers = new Map();
const names = ["activateInventoryItem", "sameInventoryActionItem",
  "activateInventoryItemFromPointer", "activateInventoryItemDoubleClick", "runContextAction", "confirmSplitItem"];
function visit(node) {
  if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) handlers.set(node.name.text, node.getText(source));
  if (ts.isJsxOpeningElement(node) && node.attributes.properties.some(attribute =>
    ts.isJsxAttribute(attribute) && attribute.name.getText(source) === "className"
      && attribute.initializer && ts.isStringLiteral(attribute.initializer)
      && attribute.initializer.text === "inventory-item-card")) {
    for (const attribute of node.attributes.properties) {
      if (ts.isJsxAttribute(attribute) && ["onPointerDown", "onMouseDown", "onClick", "onDoubleClick"].includes(attribute.name.getText(source))) {
        assert.ok(attribute.initializer && ts.isJsxExpression(attribute.initializer) && attribute.initializer.expression);
        cardHandlers.set(attribute.name.getText(source), attribute.initializer.expression.getText(source));
      }
    }
  }
  ts.forEachChild(node, visit);
}
visit(source);
assert.equal(handlers.size, names.length);
assert.equal(cardHandlers.size, 4);
const javascript = ts.transpileModule([...handlers.values()].join("\n")
  + `\nfunction itemCardHandlers(item) { return { ${[...cardHandlers].map(([name, expression]) => `${name}: ${expression}`).join(",")} }; }`, {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
}).outputText;

const item = (type, overrides = {}) => ({ key: `crystal-item-${type}`, name: "本地装备", icon: 0,
  uniqueId: 40, authoritativeUniqueId: 0, container: "bag1", slot: 3, quantity: 1,
  description: "", tooltipSource: { info: { item_type: type, stack_size: 1 } }, ...overrides });
const t = (key, _args, fallback) => fallback ?? key;
function fixture(equipmentItems = []) {
  const calls = [];
  const scope = {
    ...utils, window: {}, world: { equipmentItems, inventoryItems: [] }, t, repairMode: false,
    mailItemLocked: () => false, storageMode: null, deleteMode: false, sellMode: false,
    activeTab: "bag1", pendingMoveItem: null, pendingSplitItem: null, pendingGoldDrop: false, contextMenu: null, splitCount: "1",
    inventoryPointerActivationRef: { current: null }, bagBeltActivationGenerationRef: { current: 0 },
    consumedRepairPointerRef: { current: new WeakSet() }, tooltip: { activate() {} },
    onBagBeltPointerDown: undefined, onRepairPointerDown: undefined,
    onEquipItem: (ref, slot) => calls.push({ action: "equip", ref, slot }),
    onUseItem: ref => calls.push({ action: "use", ref }),
    onMoveItem: (ref, slot, container) => { calls.push({ action: "move", ref, slot, container }); return true; },
    onMergeItem: (from, to) => calls.push({ action: "merge", from, to }),
    onSplitItem: (ref, count) => calls.push({ action: "split", ref, count }),
    setPendingMoveItem: value => { scope.pendingMoveItem = value; },
    setPendingSplitItem: value => { scope.pendingSplitItem = value; },
    setPendingDeleteItem: value => { scope.pendingDeleteItem = value; },
    setPendingSellItem: value => { scope.pendingSellItem = value; },
    setSplitCount: value => { scope.splitCount = value; },
    setDeleteFeedback: () => {},
  };
  return { scope, calls, ...new Function("scope", `with (scope) { ${javascript}; return { ${names.join(",")}, itemCardHandlers }; }`)(scope) };
}

function pointerEvent(currentTarget, overrides = {}) {
  return { currentTarget, button: 0, detail: 1, pointerType: "mouse", isPrimary: true,
    defaultPrevented: false, preventDefault() { this.defaultPrevented = true; }, stopPropagation() {}, ...overrides };
}

test("actual primary and double-click handlers equip once and keep zero authoritative identity", () => {
  const f = fixture([{ slot: "ringLeft" }]);
  const ring = item(7);
  f.scope.world.inventoryItems = [ring];
  const card = f.itemCardHandlers(ring), node = {};
  card.onPointerDown(pointerEvent(node));
  card.onMouseDown(pointerEvent(node));
  card.onClick(pointerEvent(node));
  card.onPointerDown(pointerEvent(node));
  card.onMouseDown(pointerEvent(node, { detail: 2 }));
  card.onClick(pointerEvent(node, { detail: 2 }));
  card.onDoubleClick(pointerEvent(node, { detail: 2 }));
  assert.deepEqual(f.calls.map(call => [call.action, call.slot, call.ref.authoritativeUniqueId]), [["equip", "ringRight", 0]]);
});

test("Bag-to-Belt consumed mouse events still activate a double-click once", () => {
  const f = fixture();
  const sword = item(1);
  f.scope.world.inventoryItems = [sword];
  const card = f.itemCardHandlers(sword), node = {};
  let shortPress;
  f.scope.onBagBeltPointerDown = (_event, _item, activate) => { shortPress = activate; return true; };
  for (const detail of [1, 2]) {
    card.onPointerDown(pointerEvent(node));
    card.onMouseDown(pointerEvent(node, { detail }));
    shortPress();
    card.onClick(pointerEvent(node, { detail }));
  }
  card.onDoubleClick(pointerEvent(node, { detail: 2 }));
  assert.deepEqual(f.calls.map(call => call.action), ["equip"]);
});

test("double-click picked-up source clears Move and directly equips without a move or merge", () => {
  const f = fixture();
  const sword = item(1);
  f.scope.world.inventoryItems = [sword];
  f.scope.pendingMoveItem = sword;
  const card = f.itemCardHandlers(sword), node = {};
  card.onMouseDown(pointerEvent(node));
  assert.equal(f.scope.pendingMoveItem, null);
  assert.equal(f.calls.length, 0);
  card.onMouseDown(pointerEvent(node, { detail: 2 }));
  card.onDoubleClick(pointerEvent(node, { detail: 2 }));
  card.onDoubleClick(pointerEvent(node, { detail: 2 }));
  assert.deepEqual(f.calls.map(call => call.action), ["equip"]);
  assert.equal(f.scope.pendingMoveItem, null);
});

test("double-click cannot use a replaced instance or turn a swap into a second item action", () => {
  for (const mode of ["replacement", "moveOther", "dragWithoutActivation", "expired"]) {
    const f = fixture();
    const ring = item(7);
    f.scope.world.inventoryItems = [ring];
    const card = f.itemCardHandlers(ring), node = {};
    if (mode !== "dragWithoutActivation") {
      f.scope.pendingMoveItem = mode === "moveOther" ? item(1, { slot: 5, uniqueId: 90, authoritativeUniqueId: 90 }) : ring;
      card.onMouseDown(pointerEvent(node));
    }
    if (mode === "replacement") f.scope.world.inventoryItems = [item(7, { uniqueId: 91, authoritativeUniqueId: 91 })];
    if (mode === "expired") f.scope.inventoryPointerActivationRef.current.at -= 501;
    card.onMouseDown(pointerEvent(node, { detail: 2 }));
    card.onDoubleClick(pointerEvent(node, { detail: 2 }));
    assert.deepEqual(f.calls.map(call => call.action), mode === "moveOther" ? ["move"] : [], mode);
  }
});

test("double-click keeps repair, storage, sell, delete, quest, mail and split modal behavior", () => {
  for (const mode of ["repair", "store", "takeBack", "sell", "delete", "quest", "mail", "split", "gold", "menu"]) {
    const f = fixture();
    const ring = item(7);
    f.scope.world.inventoryItems = [ring];
    f.scope.inventoryPointerActivationRef.current = { item: ring, at: Date.now(), used: false, moved: false };
    if (mode === "repair") f.scope.repairMode = true;
    if (mode === "store" || mode === "takeBack") f.scope.storageMode = mode;
    if (mode === "sell") f.scope.sellMode = true;
    if (mode === "delete") f.scope.deleteMode = true;
    if (mode === "quest") f.scope.activeTab = "quest";
    if (mode === "mail") f.scope.mailItemLocked = () => true;
    if (mode === "split") f.scope.pendingSplitItem = ring;
    if (mode === "gold") f.scope.pendingGoldDrop = true;
    if (mode === "menu") f.scope.contextMenu = { item: ring };
    f.itemCardHandlers(ring).onDoubleClick(pointerEvent({}));
    assert.equal(f.calls.length, 0, mode);
  }
});

test("double-click consumables use once and keyboard activation remains available", () => {
  const f = fixture();
  const potion = item(3);
  f.scope.world.inventoryItems = [potion];
  const card = f.itemCardHandlers(potion), node = {};
  card.onMouseDown(pointerEvent(node));
  card.onMouseDown(pointerEvent(node, { detail: 2 }));
  card.onDoubleClick(pointerEvent(node, { detail: 2 }));
  assert.deepEqual(f.calls.map(call => call.action), ["use"]);
  card.onClick(pointerEvent(node, { detail: 0 }));
  assert.deepEqual(f.calls.map(call => call.action), ["use", "use"]);
});

test("pointer de-duplication releases after the click window and never blocks a new special mode", () => {
  const f = fixture();
  const ring = item(7);
  f.activateInventoryItemFromPointer(ring);
  f.activateInventoryItemFromPointer(ring);
  assert.equal(f.calls.length, 1);
  f.scope.inventoryPointerActivationRef.current.at -= 501;
  f.activateInventoryItemFromPointer(ring);
  assert.equal(f.calls.length, 2);
  f.scope.storageMode = "store";
  f.activateInventoryItemFromPointer(ring);
  assert.equal(f.scope.pendingMoveItem, ring);
  assert.equal(f.calls.length, 2);
  f.scope.storageMode = null;
  f.scope.sellMode = true;
  f.activateInventoryItemFromPointer(ring);
  assert.equal(f.scope.pendingSellItem, ring);
  f.scope.sellMode = false;
  f.scope.deleteMode = true;
  f.activateInventoryItemFromPointer(ring);
  assert.equal(f.scope.pendingDeleteItem, ring);
  assert.equal(f.calls.length, 2);
});

test("localized metadata-only weapons request replacement of an occupied weapon slot", () => {
  const f = fixture([{ slot: "weapon" }]);
  f.activateInventoryItem(item(1, { name: "木剑" }));
  assert.equal(f.calls.length, 1);
  assert.equal(f.calls[0].action, "equip");
  assert.equal(f.calls[0].slot, "weapon");
  assert.equal(f.calls[0].ref.authoritativeUniqueId, 0);
});

test("primary and context-menu ring actions choose the empty second hand", () => {
  const f = fixture([{ slot: "ringLeft" }]);
  const ring = item(7, { name: "铜戒指" });
  f.activateInventoryItem(ring);
  f.runContextAction(ring, "equip");
  assert.deepEqual(f.calls.map(call => [call.action, call.slot]), [["equip", "ringRight"], ["equip", "ringRight"]]);
  assert.equal(panels.inventoryItemFilterFor(ring), "equip");
});

test("ring menus omit Split and quantities cannot make equipment splittable", () => {
  for (const quantity of [1, 2]) {
    const ring = item(7, { name: "铜戒指", quantity });
    assert.equal(utils.inventoryItemCanSplit(ring), false);
    const html = renderToStaticMarkup(React.createElement(panels.InventoryContextMenu, {
      t, item: ring, x: 0, y: 0, canEquip: Boolean(utils.equipmentSlotForItem(ring)),
      canSplit: utils.inventoryItemCanSplit(ring), onAction() {}, onClose() {},
    }));
    assert.match(html, />Equip</);
    assert.doesNotMatch(html, /Split Item/);
    const f = fixture();
    f.runContextAction(ring, "split");
    f.confirmSplitItem(ring);
    assert.equal(f.scope.pendingSplitItem, null);
    assert.equal(f.calls.length, 0);
  }
});

test("only splittable consumable stacks show Split and counts leave a remainder", () => {
  for (const quantity of [1, 3]) {
    const potion = item(3, { name: "红药水", quantity, tooltipSource: { info: { item_type: 3, stack_size: 20 } } });
    const html = renderToStaticMarkup(React.createElement(panels.InventoryContextMenu, {
      t, item: potion, x: 0, y: 0, canEquip: false, canSplit: utils.inventoryItemCanSplit(potion), onAction() {}, onClose() {},
    }));
    assert.equal(html.includes("Split Item"), quantity > 1);
    if (quantity > 1) {
      const f = fixture();
      f.scope.splitCount = "3";
      f.confirmSplitItem(potion);
      assert.equal(f.calls.length, 0);
      f.scope.splitCount = "2";
      f.confirmSplitItem(potion);
      assert.deepEqual(f.calls.map(call => [call.action, call.count]), [["split", 2]]);
    }
  }
  assert.equal(utils.inventoryItemCanSplit(item(3, { quantity: 2 })), false);
});

test("moving identical rings requests a cell swap instead of a stack merge", () => {
  const f = fixture();
  f.scope.pendingMoveItem = item(7);
  f.activateInventoryItem(item(7, { uniqueId: 41, authoritativeUniqueId: 1, slot: 4 }));
  assert.deepEqual(f.calls.map(call => call.action), ["move"]);
});

test("authoritative ordinary item types override misleading equipment names", () => {
  assert.equal(utils.equipmentSlotForItem(item(3, { key: "SwordPotion", name: "Ring Potion" })), null);
  assert.equal(utils.equipmentSlotForItem({ key: "CopperRing", name: "铜戒指" }, [{ slot: "ringLeft" }]), "ringRight");
});
