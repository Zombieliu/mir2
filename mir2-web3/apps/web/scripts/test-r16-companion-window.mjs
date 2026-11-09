import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
const read = p => readFileSync(new URL(p, import.meta.url), "utf8");
function evaluate(path, deps = {}, globals = {}) {
  const module = { exports: {} };
  new Function("require", "exports", "module", ...Object.keys(globals), ts.transpileModule(read(path), {
    fileName: path,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, jsx: ts.JsxEmit.ReactJSX },
  }).outputText)(id => { assert(Object.hasOwn(deps, id), id); return deps[id]; }, module.exports, module, ...Object.values(globals));
  return module.exports;
}
const api = evaluate("../lib/r16-companion-client.ts");
const jsx = (type, props) => ({ type, props: props ?? {} });
function expand(node) {
  if (Array.isArray(node)) return node.flatMap(expand);
  if (!node || typeof node !== "object") return node;
  if (typeof node.type === "function") return expand(node.type(node.props));
  return { ...node, props: { ...node.props, children: expand(node.props.children) } };
}
function all(node) { return Array.isArray(node) ? node.flatMap(all) : node && typeof node === "object" ? [node, ...all(node.props.children)] : []; }
function strings(node) { return Array.isArray(node) ? node.flatMap(strings).join("") : node && typeof node === "object" ? strings(node.props.children) : typeof node === "string" ? node : ""; }
const event = (key, extra = {}) => ({ key, nativeEvent: { isComposing: false }, prevented: false, stopped: false, preventDefault() { this.prevented = true; }, stopPropagation() { this.stopped = true; }, ...extra });
function host() {
  const slots = [], effects = [], cleanups = []; let cursor = 0;
  const document = { activeElement: null }, intervals = new Map(); let intervalId = 0;
  const react = { useSyncExternalStore(_subscribe, snapshot) { return snapshot(); },
    useState(initial) { const i = cursor++; if (!(i in slots)) slots[i] = typeof initial === "function" ? initial() : initial;
      return [slots[i], next => { slots[i] = typeof next === "function" ? next(slots[i]) : next; }]; },
    useRef(initial) { const i = cursor++; if (!(i in slots)) slots[i] = { current: initial }; return slots[i]; },
    useEffect(effect, deps) { const i = cursor++, before = slots[i]; if (!before || deps.some((d, n) => !Object.is(d, before[n]))) { slots[i] = deps; effects.push({ i, effect }); } },
  };
  const exports = evaluate("../app/components/r16-companion-window.tsx", { react, "react/jsx-runtime": { jsx, jsxs: jsx, Fragment: "fragment" }, "../../lib/r16-companion-client": api },
    { document, setInterval(fn, delay) { const id = ++intervalId; intervals.set(id, { fn, delay }); return id; }, clearInterval(id) { intervals.delete(id); } });
  let source = { socket: {}, socketGeneration: 1, sessionId: "s", roleId: "account:role-2", realmId: "trial", worldEpoch: "w", contextEpoch: 1 };
  const sent = [], client = new api.R16CompanionClient({ captureSource: () => source, sendPrivate(_s, f) { sent.push(f); return "sent"; }, now: () => 1000, randomId: () => "window-test" });
  let open = true, invited = 0, settings = 0;
  const props = { client, open, onOpenChange(value) { open = value; }, onInviteParty(lease, name) { assert.equal(client.invitation(lease)?.characterName, name); invited++; return true; }, onOpenModelSettings() { settings++; } };
  function reply(result, status = "ok") { const request = sent.at(-1); return client.receive(source, { type: "companionResult", payload: { requestId: request.requestId, residentId: request.residentId ?? null, operation: request.operation, status, result } }); }
  const value = (version = 1, runtime = {}) => ({ archive: { residentId: "alo-1", archiveId: "a-1", displayName: "阿洛", characterName: "AloAI", realmId: "trial", worldEpoch: "w", baseSetting: "私人背景" },
    profile: { policyVersion: version, style: "collection", party: { assistBeforeLoot: version > 1, lootOnlyAfterCombat: version > 1, askBeforeLeaving: version > 1 } }, growthRevision: version,
    feedback: [{ feedbackId: "f-1", status: "active", summary: "PRIVATE MEMORY", recordedAt: 100 }], episodes: [{ episodeId: "e-1", summary: "PRIVATE EPISODE", kind: "decision", action: "follow", outcome: "selected" }], nextCursor: null,
    runtime: { provider: "rules", modelEnabled: false, configured: false, ...runtime } });
  function ready(runtime = {}) { client.list(client.snapshot().renderLease); reply({ companions: [{ residentId: "alo-1", archiveId: "a-1", displayName: "阿洛", online: true }], worldEpoch: "w" }); reply(value(1, runtime)); }
  let tree;
  function render(overrides = {}) {
    cursor = 0; tree = expand(exports.R16CompanionWindow({ ...props, open, ...overrides }));
    const nodes = all(tree), panel = nodes.find(n => n.props.role === "dialog");
    for (const node of nodes) node.focus = () => { document.activeElement = node; node.props.onFocus?.(); };
    if (panel) {
      const enabled = root => all(root).filter(n => n.type === "button" && !n.props.disabled || n.type === "textarea" && !n.props.disabled || n.type === "a" && n.props.href);
      panel.querySelectorAll = () => enabled(panel);
      panel.querySelector = selector => {
        if (!selector.includes("data-r16-companion-confirm")) return enabled(panel)[0];
        const buttons = enabled(nodes.find(n => n.props["data-r16-companion-confirm"] !== undefined));
        return selector.includes("button") ? buttons[0] : { querySelectorAll: () => buttons };
      };
      panel.props.ref.current = panel;
    }
    return tree;
  }
  const button = id => all(tree).find(n => n.props["data-r16-companion-action"] === id);
  function click(id) { const b = button(id); assert(b, id); b.props.onClick(event("click")); render(); return b; }
  return { client, sent, props, value, ready, reply, render, button, click, document, intervals, exports,
    fire(delay) { for (const timer of [...intervals.values()]) if (timer.delay === delay) timer.fn(); },
    effects() { const pending = effects.splice(0); for (const { i, effect } of pending) { cleanups[i]?.(); cleanups[i] = effect(); } },
    dispose() { for (const cleanup of cleanups) cleanup?.(); },
    get tree() { return tree; }, get open() { return open; }, get invited() { return invited; }, get settings() { return settings; },
    retire() { source = null; client.retire(); } };
}
test("entry/panel/six tabs and fixed-teaching hit boxes match actual R16 source", () => {
  const native = read("../../game-client/client-bevy/src/crystal_ui/companion.rs");
  const h = host(); h.ready(); h.render({ open: false });
  assert.match(native, /"伙伴 Companion",872\.,174\.,140\./);
  assert.deepEqual(h.button("open").props.style, { left: 872, top: 174, width: 140, height: 29, pointerEvents: "auto" });
  h.render(); const panel = all(h.tree).find(n => n.props.role === "dialog");
  assert.match(native, /left:px\(140\.\),top:px\(44\.\),width:px\(744\.\),height:px\(644\.\)/);
  assert.deepEqual(panel.props.style, { left: 140, top: 44, width: 744, height: 644 });
  assert.equal(panel.props["aria-modal"], "true");
  const tabs = all(h.tree).filter(n => n.props.role === "tab"); assert.equal(tabs.length, 6);
  assert.deepEqual(tabs.map(n => n.props.children), ["培养与体验", "记忆管理", "行动与成本", "私人委托", "独立补给", "自主生活"]);
  for (const [i, tab] of tabs.entries()) assert.deepEqual(tab.props.style, { left: 18 + i * 120, top: 132, width: 110 });
  assert.deepEqual(h.button("teach").props.style, { left: 22, top: 326, width: 690 });
});
test("actual rendered controls perform teaching, memory read, explicit supply confirmation and revocation", () => {
  const h = host(); h.ready(); h.render(); h.click("teach");
  assert.equal(h.sent.at(-1).operation, "teach"); assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 1 });
  h.reply(h.value(2)); h.render(); h.click("tab-1"); assert(strings(h.tree).includes("PRIVATE MEMORY"));
  h.click("tab-4"); const count = h.sent.length; h.click("prepare-supply"); assert.equal(h.sent.length, count);
  assert(strings(h.tree).includes("最多花费 50 游戏金币")); assert(strings(h.tree).includes("权限 5 分钟有效"));
  assert.deepEqual(h.button("confirm").props.style, { left: 370, top: 390, width: 210 });
  h.click("confirm"); assert.equal(h.sent.at(-1).operation, "authorizeSupply");
  assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 2, budgetGold: 50, quantity: 1, expiresInSeconds: 300 });
  h.reply(h.value(2, { supplyTask: { status: "authorized", budgetGold: 50, chargedGold: null, acquiredQuantity: 0, receiptKnown: false, returnedToAnchor: false } }));
  h.render(); assert.equal(h.button("prepare-supply").props.disabled, true); assert(strings(h.tree).includes("尚未确认"));
  h.click("cancel-supply"); assert.equal(h.sent.at(-1).operation, "cancelSupply"); assert.deepEqual(h.sent.at(-1).payload, {});
});
test("trial controls use the current visit and offer and keep unmeasured costs distinct from zero", () => {
  const h = host(); h.ready(); h.render(); h.click("open-trial");
  assert.equal(h.button("trial-start").props.disabled, true);
  const trial = { experimentId: "trial-1", activeVisitId: null,
    usage: { runtimeSeconds: 10, modelCalls: 0, confirmedAdventureCount: 0 },
    costs: { currency: "CNY", modelMicros: "0", hostingMicros: null, complete: false } };
  const serverView = extra => ({ ...h.value(), trial: { ...trial, ...extra } });
  h.client.refresh(h.client.snapshot().renderLease); h.reply(serverView()); h.render();
  h.click("trial-start");
  const request = h.sent.at(-1);
  assert.equal(request.operation, "trialEvent");
  const { visitId } = request.payload;
  assert.match(visitId, /^wvisit-/);
  assert.deepEqual(request.payload, { kind: "visit", visitId, entrySource: "unknown", reminderCount: 0 });
  h.reply(serverView({ activeVisitId: visitId })); h.render();
  h.click("trial-reason-shared-history"); h.click("trial-save-choice");
  assert.deepEqual(h.sent.at(-1).payload, { kind: "choice", visitId, reason: "shared-history" });
  h.reply(serverView({ activeVisitId: visitId })); h.render();
  h.click("tab-2"); h.click("open-costs");
  assert(strings(h.tree).includes("模型金额记录：0 CNY　托管：未计量"));
  assert(strings(h.tree).includes("成本未齐，暂不可计算"));
  assert.equal(h.button("trial-purchase-intent").props.disabled, true);
  const offer = { offerId: "offer-1", priceMicros: "1234567", currency: "CNY", durationDays: 7, adventureCount: 3 };
  h.client.refresh(h.client.snapshot().renderLease); h.reply(serverView({ activeVisitId: visitId, offer })); h.render();
  assert(strings(h.tree).includes("1.234567 CNY"));
  h.click("trial-purchase-intent");
  assert.deepEqual(h.sent.at(-1).payload, { kind: "purchaseIntent", visitId, offerId: "offer-1" });
  h.reply(serverView({ offer })); h.render();
  assert.equal(h.button("trial-purchase-intent").props.disabled, true);
});
test("confirmation uses an opaque rendered lease and cannot authorize a changed view", () => {
  const h = host(); h.ready(); h.render(); h.click("tab-4"); h.click("prepare-supply");
  const stale = h.button("confirm");
  h.client.refresh(h.client.snapshot().renderLease); h.reply(h.value(2)); h.render();
  assert.equal(h.button("confirm").props.disabled, true);
  const count = h.sent.length; stale.props.onClick(event("click")); assert.equal(h.sent.length, count);
});
test("memory revoke/delete requires confirmation and pending/unknown buttons cannot send", () => {
  const h = host(); h.ready(); h.render(); const staleTeach = h.button("teach"); h.click("tab-1"); const before = h.sent.length;
  h.click("revoke-f-1"); assert.equal(h.sent.length, before); h.click("confirm");
  assert.equal(h.sent.at(-1).operation, "revokeFeedback"); assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 1, feedbackId: "f-1" });
  h.reply({}, "unknown"); h.render(); h.click("tab-0"); assert.equal(h.button("teach"), undefined); assert.equal(h.button("recall").props.disabled, true);
  const count = h.sent.length; staleTeach.props.onClick(event("click")); assert.equal(h.sent.length, count); assert(strings(h.tree).includes("不会自动重发"));
});
test("party invitation uses current lease and server character name, while absent hosts disable real actions", () => {
  const h = host(); h.ready(); h.render(); const stale = h.button("invite"); h.click("invite"); assert.equal(h.invited, 1);
  h.render({ onInviteParty: undefined, onOpenModelSettings: undefined }); assert.equal(h.button("invite").props.disabled, true); assert.equal(h.button("model-settings").props.disabled, true);
  h.retire(); stale.props.onClick(event("click")); assert.equal(h.invited, 1);
  h.render(); assert.equal(h.button("teach"), undefined); assert(!strings(h.tree).includes("PRIVATE MEMORY"));
});
test("model text Enter respects IME and uses existing propose/confirm/cancel operations", () => {
  const h = host(); h.ready({ provider: "openai", configured: true, modelEnabled: true }); h.render(); h.click("open-teaching");
  let input = all(h.tree).find(n => n.type === "textarea"); input.props.onChange({ target: { value: "离队前不用再问" } }); h.render(); input = all(h.tree).find(n => n.type === "textarea");
  const before = h.sent.length; input.props.onCompositionStart(); input.props.onKeyDown(event("Enter")); assert.equal(h.sent.length, before);
  input.props.onCompositionEnd(); input.props.onKeyDown(event("Enter", { nativeEvent: { isComposing: true } })); assert.equal(h.sent.length, before);
  input.props.onKeyDown(event("Enter")); assert.equal(h.sent.at(-1).operation, "proposeTeaching");
  assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 1, text: "离队前不用再问" });
  h.reply(h.value(1, { provider: "openai", configured: true, modelEnabled: true, teachingProposal: { proposalId: "p-1", status: "proposed", sourceVersion: 1, changes: { askBeforeLeaving: false } } }));
  h.render(); assert.equal(h.button("confirm-teaching").props.disabled, false); assert(strings(h.tree).includes("离队先询问：否 → 否"));
  h.click("confirm-teaching"); assert.equal(h.sent.at(-1).operation, "confirmTeaching"); assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 1, proposalId: "p-1" });
  h.reply(h.value(2, { provider: "openai", configured: true, modelEnabled: true, teachingProposal: { proposalId: "p-2", status: "proposed", sourceVersion: 2, changes: { askBeforeLeaving: false } } }));
  h.render(); h.click("cancel-teaching"); assert.equal(h.sent.at(-1).operation, "cancelTeaching"); assert.deepEqual(h.sent.at(-1).payload, { proposalId: "p-2" });
});
test("new content never paints an old private draft, even before passive cleanup", () => {
  const h = host(); h.ready({ provider: "openai", configured: true, modelEnabled: true }); h.render(); h.click("tab-3");
  all(h.tree).find(n => n.type === "textarea").props.onChange({ target: { value: "PRIVATE DRAFT" } }); h.render();
  assert.equal(all(h.tree).find(n => n.type === "textarea").props.value, "PRIVATE DRAFT");
  h.client.refresh(h.client.snapshot().renderLease); h.reply(h.value(2, { provider: "openai", configured: true, modelEnabled: true })); h.render();
  assert.equal(all(h.tree).find(n => n.type === "textarea").props.value, "");
  h.retire(); h.render(); assert(!strings(h.tree).includes("PRIVATE"));
});
test("Escape cancels confirmation first, focus wraps inside modal, and timers clean up", () => {
  const h = host(); h.render({ open: false }); h.effects(); assert.equal(h.intervals.size, 1);
  h.ready(); h.render(); h.click("tab-4"); h.click("prepare-supply");
  const panel = all(h.tree).find(n => n.props.role === "dialog"), cancel = h.button("cancel-confirm"), confirm = h.button("confirm");
  h.document.activeElement = cancel; const backwards = event("Tab", { shiftKey: true }); panel.props.onKeyDown(backwards);
  assert.equal(h.document.activeElement, confirm); assert.equal(backwards.prevented, true);
  const escape = event("Escape"); panel.props.onKeyDown(escape); h.render(); assert.equal(h.button("confirm"), undefined); assert.equal(h.open, true);
  all(h.tree).find(n => n.props.role === "dialog").props.onKeyDown(event("Escape")); assert.equal(h.open, false);
  h.dispose(); assert.equal(h.intervals.size, 0);
});
test("life duration selection and confirmation send the real fixed-budget permission", () => {
  const h = host(); h.ready(); h.render(); h.click("tab-5"); h.click("duration-30");
  const count = h.sent.length; h.click("prepare-life"); assert.equal(h.sent.length, count); h.click("confirm");
  assert.equal(h.sent.at(-1).operation, "authorizeLife"); assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 1, budgetGold: 50, durationSeconds: 1800 });
});
function mountedReady(h, runtime = {}) {
  h.ready(runtime); h.render(); h.effects(); h.render();
  assert.equal(h.client.snapshot().pendingOperation, "view");
  h.reply(h.value(1, runtime)); h.render(); h.effects(); h.render();
}
test("open window polls planning to a current proposal every 2.5s using reads only, and close removes that timer", () => {
  const native = read("../../game-client/client-bevy/src/crystal_ui/companion.rs"); assert.match(native, /last_task_refresh_ms\) >= 2500/);
  const h = host(), model = { provider: "openai", configured: true, modelEnabled: true }; mountedReady(h, model);
  h.click("open-teaching"); all(h.tree).find(n => n.type === "textarea").props.onChange({ target: { value: "离队前不用再问" } }); h.render(); h.click("propose-teaching");
  h.reply(h.value(1, { ...model, teachingProposal: { proposalId: "p-1", status: "planning", sourceVersion: 1 } })); h.render(); h.effects(); h.render();
  assert.equal(all(h.tree).find(n => n.type === "textarea").props.value, "");
  const count = h.sent.length; h.fire(2500); assert.equal(h.sent.length, count + 1); assert.equal(h.sent.at(-1).operation, "view");
  h.fire(2500); assert.equal(h.sent.length, count + 1, "pending read cannot overlap another read");
  h.reply(h.value(1, { ...model, teachingProposal: { proposalId: "p-1", status: "proposed", sourceVersion: 1, changes: { askBeforeLeaving: false } } })); h.render();
  assert.equal(h.button("confirm-teaching").props.disabled, false);
  assert.equal(h.sent.filter(f => f.operation === "proposeTeaching").length, 1);
  h.click("close"); h.effects(); assert.equal([...h.intervals.values()].filter(t => t.delay === 2500).length, 0);
  const closedCount = h.sent.length; h.fire(2500); h.fire(1000); assert.equal(h.sent.length, closedCount); h.dispose();
});
test("editing/composition and nonempty drafts pause polling; unchanged explicit views preserve the draft", () => {
  const h = host(), model = { provider: "openai", configured: true, modelEnabled: true }; mountedReady(h, model); h.click("tab-3");
  let input = all(h.tree).find(n => n.type === "textarea"), count = h.sent.length;
  input.props.onFocus(); h.fire(2500); assert.equal(h.sent.length, count);
  input.props.onBlur(); input.props.onCompositionStart(); h.fire(2500); assert.equal(h.sent.length, count); input.props.onCompositionEnd();
  input.props.onChange({ target: { value: "PRIVATE EDITING DRAFT" } }); h.fire(2500); assert.equal(h.sent.length, count, "draft ref pauses even before rerender"); h.render();
  const epoch = h.client.snapshot().contentEpoch; h.client.refresh(h.client.snapshot().renderLease); h.reply(h.value(1, model)); h.render(); h.effects(); h.render();
  assert.equal(h.client.snapshot().contentEpoch, epoch); input = all(h.tree).find(n => n.type === "textarea"); assert.equal(input.props.value, "PRIVATE EDITING DRAFT");
  count = h.sent.length; h.fire(2500); assert.equal(h.sent.length, count);
  input.props.onChange({ target: { value: "" } }); h.render(); h.fire(2500); assert.equal(h.sent.length, count + 1); assert.equal(h.sent.at(-1).operation, "view"); h.dispose();
});
test("a confirmation keeps its original live lease because polling pauses until dismissal", () => {
  const h = host(); mountedReady(h); h.click("tab-4"); h.click("prepare-supply"); h.effects();
  const before = h.sent.length, lease = h.client.snapshot().renderLease;
  h.fire(2500); h.fire(2500); h.render();
  assert.equal(h.sent.length, before); assert.equal(h.client.snapshot().renderLease, lease); assert.equal(h.button("confirm").props.disabled, false);
  h.click("cancel-confirm"); h.fire(2500); assert.equal(h.sent.at(-1).operation, "view"); h.dispose();
});
test("invisible format input cannot enable or dispatch a model action", () => {
  const h = host(); h.ready({ provider: "openai", configured: true, modelEnabled: true }); h.render(); h.click("open-teaching");
  for (const character of ["\u0085", "\u200B"]) {
    all(h.tree).find(n => n.type === "textarea").props.onChange({ target: { value: `跟我${character}回城` } }); h.render();
    assert.equal(h.button("propose-teaching").props.disabled, true);
    const count = h.sent.length; h.click("propose-teaching"); all(h.tree).find(n => n.type === "textarea").props.onKeyDown(event("Enter")); assert.equal(h.sent.length, count);
  }
});
