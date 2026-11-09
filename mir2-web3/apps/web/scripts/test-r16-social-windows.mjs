import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const read = path => readFileSync(new URL(path, import.meta.url), "utf8");
function evaluate(path, deps = {}) {
  const output = ts.transpileModule(read(path), { fileName: path,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX } });
  const module = { exports: {} };
  new Function("require", "module", "exports", output.outputText)(id => {
    assert.ok(Object.hasOwn(deps, id), "Unexpected test dependency " + id); return deps[id];
  }, module, module.exports);
  return module.exports;
}

let activeHost;
const react = {
  useState(initial) {
    const owner = activeHost, index = owner.cursor++;
    if (!(index in owner.states)) owner.states[index] = typeof initial === "function" ? initial() : initial;
    return [owner.states[index], next => {
      const value = typeof next === "function" ? next(owner.states[index]) : next;
      if (!Object.is(owner.states[index], value)) { owner.states[index] = value; owner.dirty = true; }
    }];
  },
  useRef(initial) { const owner = activeHost, index = owner.cursor++; return owner.states[index] ??= { current: initial }; },
  useMemo(fn) { activeHost.cursor++; return fn(); },
  useEffect(fn, deps) {
    const owner = activeHost, index = owner.cursor++, prior = owner.effectDeps[index];
    if (!prior || deps.some((value, i) => !Object.is(value, prior[i]))) {
      owner.effectDeps[index] = deps; owner.pending.push(() => {
        owner.cleanups[index]?.(); owner.cleanups[index] = fn();
      });
    }
  },
};
const jsx = { Fragment: Symbol("Fragment"), jsx: (type, props) => ({ type, props }), jsxs: (type, props) => ({ type, props }) };
function mount(component, initialProps, effects = true) {
  const host = { cursor: 0, states: [], effectDeps: [], pending: [], cleanups: [], dirty: false };
  let props = initialProps, tree;
  const flush = () => { for (const effect of host.pending.splice(0)) effect(); };
  const render = next => {
    if (next) props = next;
    for (let round = 0; round < 8; round++) {
      host.cursor = 0; host.dirty = false; activeHost = host; tree = component(props);
      if (effects) flush();
      if (!host.dirty) return tree;
    }
    throw new Error("Unstable test render");
  };
  render();
  return { render, flush, get tree() { return tree; }, cleanup() { for (const cleanup of host.cleanups) cleanup?.(); } };
}
function nodes(tree, predicate) {
  const found = [];
  const visit = value => {
    if (Array.isArray(value)) { value.forEach(visit); return; }
    if (!value || typeof value !== "object" || !value.props) return;
    if (predicate(value)) found.push(value); visit(value.props.children);
  };
  visit(tree); return found;
}
const one = (tree, key) => {
  const found = nodes(tree, node => node.props[key] !== undefined);
  assert.equal(found.length, 1, "Exactly one " + key); return found[0];
};
const t = (_key, _args, fallback) => fallback;
const layout = evaluate("../lib/r16-social-layout.ts");
const controls = evaluate("../app/components/r16-social-controls.tsx", {
  react, "react/jsx-runtime": jsx, "../../lib/r16-social-layout": layout,
});
const stableControls = { ...controls, useR16SocialWindow: rect => ({ position: { left: rect.left, top: rect.top },
  pointerDown() {}, pointerMove() {}, pointerUp() {} }) };
const social = evaluate("../lib/social-item-window-model.ts", { "./bevy-bag-model": {}, "./world-model/item-identity": {} });
const dependencies = {
  react, "react/jsx-runtime": jsx, "../../lib/r16-social-layout": layout,
  "./r16-social-controls": stableControls,
};
const { GroupWindow } = evaluate("../app/components/original-client-group-window.tsx", dependencies);
const { TradeWindow } = evaluate("../app/components/original-client-trade-window.tsx", { ...dependencies,
  "../../lib/social-item-window-model": social,
  "./original-client-inventory-utils": { originalItemIconPath: icon => "/original-ui/Items/" + icon + ".png" },
  "./original-client-item-tooltip": { OriginalItemTooltip() {} },
  "./original-client-crystal-item-tooltip": { OriginalCrystalItemTooltip() {} },
});

class ElementStub {
  constructor(name = "element") { this.name = name; this.isConnected = true; }
  closest() { return null; }
  focus() { globalThis.document.activeElement = this; }
}
const globals = { window: globalThis.window, document: globalThis.document, Element: globalThis.Element, HTMLElement: globalThis.HTMLElement };
test.before(() => {
  globalThis.window = { addEventListener() {}, removeEventListener() {} };
  globalThis.document = { activeElement: null };
  globalThis.Element = globalThis.HTMLElement = ElementStub;
});
test.after(() => { for (const [key, value] of Object.entries(globals)) value === undefined ? delete globalThis[key] : globalThis[key] = value; });

function nativeRect(source, name) {
  const match = source.match(new RegExp("const " + name + ":[^=]*=\\s*CrystalRect::new\\(([\\d.]+),\\s*([\\d.]+),\\s*([\\d.]+),\\s*([\\d.]+)\\)"));
  assert.ok(match, "Native rect " + name);
  const [left, top, width, height] = match.slice(1).map(Number); return { left, top, width, height };
}

test("actual window and control rectangles match independent native R16 definitions", () => {
  const native = read("../../game-client/client-bevy/src/crystal_ui/trade_dialog.rs");
  const overlay = read("../../game-client/client-bevy/src/crystal_ui/overlays.rs");
  for (const [web, rust] of Object.entries({ own: "OWN_RECT", guest: "GUEST_RECT", ownName: "OWN_NAME", guestName: "GUEST_NAME", gold: "GOLD", confirm: "CONFIRM", close: "CLOSE" })) {
    assert.deepEqual(layout.R16_SOCIAL_LAYOUT.trade[web], nativeRect(native, rust));
  }
  assert.deepEqual(layout.R16_SOCIAL_LAYOUT.group.panel, nativeRect(overlay, "CRYSTAL_GROUP_PANEL_RECT"));
  assert.deepEqual(layout.R16_SOCIAL_LAYOUT.trade.amount, nativeRect(overlay, "CRYSTAL_DELETE_AMOUNT_RECT"));
});

test("trade slots use column-major locations without changing their protocol IDs", () => {
  assert.deepEqual(layout.r16TradeCell(0), { left: 10, top: 39, width: 36, height: 32 });
  assert.deepEqual(layout.r16TradeCell(1), { left: 10, top: 72, width: 36, height: 32 });
  assert.deepEqual(layout.r16TradeCell(8), { left: 158, top: 39, width: 36, height: 32 });
  assert.deepEqual(layout.r16TradeCell(9), { left: 158, top: 72, width: 36, height: 32 });
  for (const value of [-1, 10, 1.5, NaN]) assert.equal(layout.r16TradeCell(value), null);
  assert.deepEqual(layout.r16GroupMemberRect(0), { left: 16, top: 33, width: 96, height: 16 });
  assert.deepEqual(layout.r16GroupMemberRect(14), { left: 116, top: 175, width: 96, height: 16 });
  assert.equal(layout.r16GroupMemberRect(15), null);
});

test("source window/button art is present, including both independent trade backgrounds", () => {
  for (const [library, frames] of [["Prguse", [114, 115, 116, 117, 118, 119, 120, 238, 360, 389, 390, 660]],
    ["Title", [5, 130, 131, 132, 133, 134, 135, 136, 137, 138, 200, 201, 202, 203, 204, 205, 206, 207, 208, 210, 211, 212, 520, 521, 522]],
    ["Prguse2", [360, 361, 362]]]) {
    for (const frame of frames) assert.ok(readFileSync(new URL("../public/original-ui/" + library + "/" + frame + ".png", import.meta.url)).length > 0);
  }
});

test("group add/remove open real name dialogs and submit only the entered name", () => {
  const calls = [], props = { t, group: { members: [{ name: "Owner", leader: true }] }, playerName: "Owner",
    onInviteMember: name => calls.push(["add", name]), onKickMember: name => calls.push(["remove", name]), onClose() {} };
  const view = mount(GroupWindow, props);
  one(view.tree, "data-r16-group-add").props.onClick(); view.render();
  one(view.tree, "data-r16-group-name").props.onChange({ target: { value: "  NewFriend  " } }); view.render();
  one(view.tree, "data-r16-group-name-submit").props.onClick(); view.render();
  assert.deepEqual(calls, [["add", "NewFriend"]]);
  one(view.tree, "data-r16-group-remove").props.onClick(); view.render();
  one(view.tree, "data-r16-group-name").props.onChange({ target: { value: "NewFriend" } }); view.render();
  nodes(view.tree, node => node.type === controls.R16SocialModal)[0].props.onCancel(); view.render();
  assert.deepEqual(calls, [["add", "NewFriend"]], "cancel never submits removal");
});

test("group name editor retains the entered draft after a definite local send rejection", () => {
  for (const remove of [false, true]) {
    const calls = [], props = { t, group: {members: ["Owner"]}, playerName: "Owner", onClose() {},
      onInviteMember: name => {calls.push(name); return false;}, onKickMember: name => {calls.push(name); return false;} };
    const view = mount(GroupWindow, props);
    one(view.tree, remove ? "data-r16-group-remove" : "data-r16-group-add").props.onClick(); view.render();
    one(view.tree, "data-r16-group-name").props.onChange({target: {value: "  StillHere  "}}); view.render();
    one(view.tree, "data-r16-group-name-submit").props.onClick(); view.render();
    assert.deepEqual(calls, ["StillHere"]);
    assert.equal(one(view.tree, "data-r16-group-name").props.value, "  StillHere  ");
    view.cleanup();
  }
});

test("non-leaders and a full party cannot dispatch a new add through the visible UI", () => {
  const props = { t, group: { members: ["Owner", "Member"] }, playerName: "Member", onInviteMember() {}, onKickMember() {}, onClose() {} };
  const view = mount(GroupWindow, props);
  assert.equal(nodes(view.tree, node => node.props["data-r16-group-add"]).length, 0);
  const full = Array.from({ length: 15 }, (_, i) => ({ name: i === 0 ? "Owner" : "Member" + i, leader: i === 0 }));
  view.render({ ...props, group: { members: full }, playerName: "Owner" });
  assert.equal(one(view.tree, "data-r16-group-add").props.disabled, true);
  one(view.tree, "data-r16-group-add").props.onClick(); view.render();
  assert.equal(nodes(view.tree, node => node.props["data-r16-group-name"]).length, 0);
});

test("group replies keep the captured invitation epoch and preserve the real allow-invites callback", () => {
  const calls = [], props = { t, group: { members: [], allowInvites: true }, incomingInvite: { name: "Leader", epoch: 7 },
    onReplyInvite: (epoch, accept) => calls.push([epoch, accept]), onToggleAllowInvites: value => calls.push(["allow", value]), onClose() {} };
  const view = mount(GroupWindow, props);
  const old = one(view.tree, "data-r16-group-invite-accept");
  view.render({ ...props, incomingInvite: { name: "NewLeader", epoch: 8 } });
  old.props.onClick(); one(view.tree, "data-r16-group-invite-decline").props.onClick();
  const allow = nodes(view.tree, node => node.type === controls.R16SocialButton && node.props.library === "Prguse")[0];
  allow.props.onClick();
  assert.deepEqual(calls, [[7, true], [8, false], ["allow", false]]);
});

function tradeProps(overrides = {}) {
  return { t, trade: { partner: "Partner", state: "open", partnerItemsKnown: true }, playerName: "Owner",
    myItems: Array(10).fill(null), myGold: 0, myConfirmed: false, availableGold: 100,
    inventoryItems: [{ slot: 2, uniqueId: 90, name: "Sword", count: 1, icon: 3 }], emptyInventorySlots: [0, 1, 3],
    itemsReady: true, itemActionPending: false, itemSourceKey: "source-1", onConfirm() {}, onCancel() {}, onSetGold() {}, onClose() {},
    ...overrides };
}
const grid = tree => nodes(tree, node => node.type?.name === "TradeGrid" && node.props.onItemClick)[0];

test("real bag selection deposits the same UID into a selected column-major protocol slot", () => {
  const calls = [], props = tradeProps({ onDepositItem: (...args) => calls.push(args) });
  const before = JSON.stringify(props);
  const view = mount(TradeWindow, props);
  const picker = nodes(view.tree, node => node.type === "button" && node.props.title === "Sword ×1 (3)")[0];
  assert.equal(picker.props.disabled, false);
  picker.props.onClick(); view.render(); grid(view.tree).props.onItemClick(1); view.render();
  assert.deepEqual(calls, [[2, 1, 90]]);
  assert.equal(JSON.stringify(props), before, "UI selection never mutates inventory or offers");
  assert.equal(one(view.tree, "data-r16-trade-bag-picker").props.style.top, 250);
  const sides = nodes(view.tree, node => node.props["data-r16-trade-side"]);
  assert.deepEqual(sides.map(node => [node.props.style.width, node.props.style.height]), [[204, 152], [204, 152]]);
});

test("offer retrieval keeps both original slot identities and stale/pending sources cannot edit", () => {
  const calls = [], offered = { id: "offered", slot: 1, uniqueId: 90, name: "Sword", count: 1, icon: 3 };
  const slots = Array(10).fill(null); slots[1] = offered;
  const props = tradeProps({ myItems: slots, onRetrieveItem: (...args) => calls.push(args) });
  const view = mount(TradeWindow, props);
  grid(view.tree).props.onItemClick(1); view.render();
  nodes(view.tree, node => node.type === "button" && node.props.children === "Empty bag 4")[0].props.onClick(); view.render();
  assert.deepEqual(calls, [[1, 3, 90]]);
  view.render({ ...props, itemActionPending: true }); grid(view.tree).props.onItemClick(1); view.render();
  assert.equal(grid(view.tree).props.disabled, true);
  assert.equal(calls.length, 1);
});

test("an own-confirmed quote can add gold without unlocking items and drops a stale amount prompt", () => {
  const calls = [], props = tradeProps({ myConfirmed: true, itemsReady: false, onSetGold: value => calls.push(value) });
  const view = mount(TradeWindow, props);
  assert.equal(grid(view.tree).props.disabled, true);
  assert.equal(one(view.tree, "data-r16-trade-gold").props.disabled, false);
  one(view.tree, "data-r16-trade-gold").props.onClick(); view.render();
  assert.equal(one(view.tree, "data-r16-trade-gold-input").props.value, "100");
  one(view.tree, "data-r16-trade-gold-input").props.onChange({ target: { value: "999" } }); view.render();
  assert.equal(one(view.tree, "data-r16-trade-gold-input").props.value, "100");
  one(view.tree, "data-r16-trade-gold-input").props.onChange({ target: { value: "17" } }); view.render();
  assert.deepEqual(calls, []);
  view.render({ ...props, availableGold: 10 });
  assert.equal(one(view.tree, "data-r16-trade-gold-input").props.value, "17");
  assert.equal(nodes(view.tree, node => node.props["data-r16-trade-gold-submit"]).length, 0, "wallet decrease never substitutes a smaller delta");
  view.render(props);
  one(view.tree, "data-r16-trade-gold-submit").props.onClick(); view.render(); assert.deepEqual(calls, [17]);
  one(view.tree, "data-r16-trade-gold").props.onClick(); view.render();
  view.render({ ...props, itemSourceKey: "new-source" });
  assert.equal(nodes(view.tree, node => node.props["data-r16-trade-gold-input"]).length, 0);
  assert.deepEqual(calls, [17]);
  assert.equal(layout.r16TradeGoldAmount("0", 100), 0);
  assert.equal(layout.r16TradeGoldAmount("999", 20), 20, "the existing wallet cap is retained");
  for (const value of ["-1", "1.5", "1e3", "4294967296", ""]) assert.equal(layout.r16TradeGoldAmount(value, 100), null);
});

test("locked/pending/unknown-partner controls preserve the original confirm/unlock/cancel gates", () => {
  let confirm = 0, cancel = 0;
  const props = tradeProps({ myConfirmed: true, onConfirm: () => confirm++, onCancel: () => cancel++ });
  const view = mount(TradeWindow, props);
  assert.equal(grid(view.tree).props.disabled, true);
  assert.equal(one(view.tree, "data-r16-trade-gold").props.disabled, false);
  assert.equal(one(view.tree, "data-r16-trade-confirm").props.disabled, false);
  one(view.tree, "data-r16-trade-confirm").props.onClick(); one(view.tree, "data-r16-trade-cancel").props.onClick();
  assert.deepEqual([confirm, cancel], [1, 1]);
  view.render({ ...props, myConfirmed: false, trade: { ...props.trade, confirmed: true, partnerItemsKnown: false } });
  assert.equal(one(view.tree, "data-r16-trade-confirm").props.disabled, true);
  assert.equal(one(view.tree, "data-r16-trade-gold").props.disabled, false);
  view.render({ ...props, itemActionPending: true });
  assert.equal(one(view.tree, "data-r16-trade-confirm").props.disabled, true);
  assert.equal(one(view.tree, "data-r16-trade-gold").props.disabled, true);
  for (const override of [{itemSourceKey: undefined}, {availableGold: 0}]) {
    view.render({ ...props, ...override });
    assert.equal(one(view.tree, "data-r16-trade-gold").props.disabled, true);
  }
});

test("incoming trade uses a separate modal and waits for the real host state to open windows", () => {
  const calls = [], props = tradeProps({ trade: { partner: "Partner", state: "requested" },
    onAccept: () => calls.push("accept"), onCancel: () => calls.push("cancel") });
  const view = mount(TradeWindow, props);
  assert.equal(nodes(view.tree, node => node.props["data-r16-trade-side"]).length, 0);
  one(view.tree, "data-r16-trade-accept").props.onClick(); view.render();
  assert.equal(nodes(view.tree, node => node.props["data-r16-trade-side"]).length, 0);
  one(view.tree, "data-r16-trade-decline").props.onClick(); assert.deepEqual(calls, ["accept", "cancel"]);
});

test("actual native button uses 520/521/522 for unlocked, locked and pressed confirm states", () => {
  const props = { frame: 520, rect: layout.R16_SOCIAL_LAYOUT.trade.confirm, label: "Confirm" };
  const view = mount(controls.R16SocialButton, props);
  assert.equal(view.tree.props.children.props.src, "/original-ui/Title/520.png");
  view.render({ ...props, locked: true }); assert.equal(view.tree.props.children.props.src, "/original-ui/Title/521.png");
  view.tree.props.onMouseDown(); view.render({ ...props, locked: true });
  assert.equal(view.tree.props.children.props.src, "/original-ui/Title/522.png");
});

test("modal consumes Enter/Escape, ignores IME/repeat and cycles Tab inside the dialog", () => {
  let confirm = 0, cancel = 0;
  const view = mount(controls.R16SocialModal, { label: "Amount", rect: layout.R16_SOCIAL_LAYOUT.trade.amount, background: "frame",
    onConfirm: () => confirm++, onCancel: () => cancel++, children: null }, false);
  const section = nodes(view.tree, node => node.props.role === "dialog")[0];
  const fields = [new ElementStub("input"), new ElementStub("ok"), new ElementStub("cancel")];
  document.activeElement = fields[0];
  const key = (name, extra = {}) => { let stopped = false, prevented = false; section.props.onKeyDownCapture({
    key: name, repeat: false, shiftKey: false, nativeEvent: { isComposing: false },
    stopPropagation() { stopped = true; }, preventDefault() { prevented = true; },
    currentTarget: { querySelectorAll: () => fields }, ...extra,
  }); assert.equal(stopped, true); return prevented; };
  assert.equal(key("Enter"), true); key("Enter", { repeat: true }); key("Enter", { nativeEvent: { isComposing: true } });
  key("Escape"); assert.deepEqual([confirm, cancel], [1, 1]);
  key("Tab", { shiftKey: true }); assert.equal(document.activeElement, fields[2]);
  key("Tab"); assert.equal(document.activeElement, fields[0]);
  assert.equal(view.tree.props["data-ui-interactive"], "true");
});

test("modal focuses its real field and restores the prior connected element on close", () => {
  const previous = new ElementStub("previous"), input = new ElementStub("input"); document.activeElement = previous;
  const view = mount(controls.R16SocialModal, { label: "Name", rect: layout.R16_SOCIAL_LAYOUT.group.input,
    background: "frame", onCancel() {}, children: null }, false);
  nodes(view.tree, node => node.props.role === "dialog")[0].props.ref.current = { querySelector: () => input };
  view.flush(); assert.equal(document.activeElement, input); view.cleanup(); assert.equal(document.activeElement, previous);
});

test("independent window drag respects stage scaling and clamps the complete panel", () => {
  const view = mount(() => controls.useR16SocialWindow(layout.R16_SOCIAL_LAYOUT.trade.own), {});
  const currentTarget = { getBoundingClientRect: () => ({ width: 102 }), closest: () => ({ offsetWidth: 1024, offsetHeight: 768 }), setPointerCapture() {} };
  view.tree.pointerDown({ button: 0, target: new ElementStub(), currentTarget, pointerId: 7, clientX: 400, clientY: 500, preventDefault() {} });
  view.tree.pointerMove({ pointerId: 7, clientX: 550, clientY: 550, preventDefault() {} }); view.render();
  assert.deepEqual(view.tree.position, { left: 598, top: 518 });
  view.tree.pointerMove({ pointerId: 7, clientX: 5000, clientY: 5000, preventDefault() {} }); view.render();
  assert.deepEqual(view.tree.position, { left: 820, top: 616 });
  view.tree.pointerUp();
});
