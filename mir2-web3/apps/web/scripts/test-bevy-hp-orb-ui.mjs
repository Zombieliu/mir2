import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

function load(path, dependencies = {}) {
  const module = { exports: {} };
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
  } }).outputText;
  new Function("exports", "module", "require", code)(module.exports, module, (name) => {
    if (name in dependencies) return dependencies[name];
    throw new Error(`Unexpected dependency ${name}`);
  });
  return module.exports;
}
const hp = load("../lib/bevy-hp-orb.ts");
const experienceBar = load("../lib/bevy-experience-bar.ts");
const host = load("../lib/bevy-quest-ui.ts", { "./quest-action-policy": { questEndpoint: (n) => n } });
const presentation = { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 0.5, touch: true };
const snapshot = { generation: 5, revision: 9, inGame: true, hostVisible: false, presentation,
  hpOrbSlot: { left: 0, top: 648 }, player: { hp: 50, maxHp: 100, level: 26, className: "warrior" } };
const orbStatus = { supported: true, ready: true, generation: 5, revision: 9, hp: 50, maxHp: 100,
  hpOnly: false, slot: { left: 0, top: 648 }, image: "original-ui/Prguse/4.png",
  source: { left: 0, top: 40, width: 50, height: 40 },
  destination: { left: 0, top: 688, width: 50, height: 40 },
  layout: { left: 0, top: 688, width: 50, height: 40 } };
const status = { frame: 3, ready: false, capturesPointer: false, questLogOpen: false,
  generation: 5, revision: 9, openRevision: 0, hpOrb: orbStatus };
const mpSnapshot = { ...snapshot, player: { ...snapshot.player, mp: 25, maxMp: 100 } };
const mpOrbStatus = { supported: true, ready: true, generation: 5, revision: 9, mp: 25, maxMp: 100,
  hpOnly: false, slot: { left: 0, top: 648 }, image: "original-ui/Prguse/4.png",
  source: { left: 51, top: 60, width: 50, height: 20 },
  destination: { left: 51, top: 708, width: 50, height: 20 },
  layout: { left: 51, top: 708, width: 50, height: 20 } };
const mpStatus = { ...status, mpOrb: mpOrbStatus };

test("full-orb origin survives partial/empty fill and rejects detached or stale stage geometry", () => {
  const frame = { isConnected: true, contains: (node) => node === image,
    getBoundingClientRect: () => ({ left: 10, top: 20, width: 512, height: 384 }) };
  let clippedHeight = 0;
  const image = { isConnected: true,
    getBoundingClientRect: () => ({ left: 10, top: 344, width: 52, height: 40, clippedHeight }) };
  assert.deepEqual(hp.readBevyHpOrbSlot(frame, image, presentation), { left: 0, top: 648 });
  clippedHeight = 20;
  assert.deepEqual(hp.readBevyHpOrbSlot(frame, image, presentation), { left: 0, top: 648 });
  image.isConnected = false;
  assert.equal(hp.readBevyHpOrbSlot(frame, image, presentation), null);
  image.isConnected = true;
  frame.getBoundingClientRect = () => ({ left: 10, top: 20, width: 640, height: 384 });
  assert.equal(hp.readBevyHpOrbSlot(frame, image, presentation), null);
  frame.getBoundingClientRect = () => ({ left: 10, top: 20, width: 512, height: 384 });
  image.getBoundingClientRect = () => ({ left: 10, top: 344, width: 52, height: 0 });
  assert.equal(hp.readBevyHpOrbSlot(frame, image, presentation), null, "missing full image cannot supply a slot");
});

test("support probe preserves old runtime compatibility and new status is independent of Quest panel readiness", () => {
  assert.equal(hp.supportsBevyHpOrb(null), false);
  assert.equal(hp.supportsBevyHpOrb({ getMir2QuestUiStatus: () => JSON.stringify({ ready: true }) }), false);
  assert.equal(hp.supportsBevyHpOrb({ getMir2QuestUiStatus: () => "broken" }), false);
  assert.equal(hp.supportsBevyHpOrb({ getMir2QuestUiStatus: () => JSON.stringify(status) }), true);
  assert.deepEqual(host.readBevyQuestUiStatus({ getMir2QuestUiStatus: () => JSON.stringify(status) }, 5)?.hpOrb, orbStatus);
  assert.deepEqual(hp.currentBevyHpOrb(status, snapshot, true), orbStatus);
});

test("ownership yields on stale identity, model, slot, image, measured node, frame, logout and asset readiness", () => {
  const reject = (s = status, p = snapshot, fresh = true) => assert.equal(hp.currentBevyHpOrb(s, p, fresh), null);
  reject(status, snapshot, false);
  reject({ ...status, hpOrb: { ...orbStatus, ready: false } });
  reject({ ...status, hpOrb: { ...orbStatus, generation: 4 } });
  reject({ ...status, hpOrb: { ...orbStatus, revision: 8 } });
  reject(status, { ...snapshot, revision: 10 });
  reject(status, { ...snapshot, inGame: false });
  reject(status, { ...snapshot, hpOrbSlot: null });
  reject(status, { ...snapshot, hpOrbSlot: { left: 1, top: 648 } });
  reject(status, { ...snapshot, player: { ...snapshot.player, hp: 49 } });
  reject({ ...status, hpOrb: { ...orbStatus, image: "original-ui/Prguse/6.png" } });
  reject({ ...status, hpOrb: { ...orbStatus, layout: { ...orbStatus.layout, top: 689 } } });
  assert.equal(hp.matchesBevyHpOrbView(orbStatus, snapshot.hpOrbSlot, 50, 100, false), true);
  assert.equal(hp.matchesBevyHpOrbView(orbStatus, snapshot.hpOrbSlot, 49, 100, false), false);
  assert.equal(hp.matchesBevyHpOrbView(orbStatus, { left: 1, top: 648 }, 50, 100, false), false);
});

test("genuine zero HP and below-26 warrior retain measured image authority", () => {
  const low = { ...snapshot, player: { ...snapshot.player, hp: 0, level: 25 } };
  const empty = { ...orbStatus, hp: 0, hpOnly: true, image: "original-ui/Prguse/6.png",
    source: { left: 0, top: 80, width: 100, height: 0 },
    destination: { left: 0, top: 728, width: 100, height: 0 },
    layout: { left: 0, top: 728, width: 100, height: 0 } };
  assert.deepEqual(hp.currentBevyHpOrb({ ...status, hpOrb: empty }, low, true), empty);
  assert.equal(hp.currentBevyHpOrb({ ...status, hpOrb: empty }, { ...low, player: { ...low.player, maxHp: 0 } }, true), null);
});

test("MP image ownership is independent, bottom-anchored, and rejects unknown or stale model/layout", () => {
  assert.equal(hp.supportsBevyMpOrb(null), false);
  assert.equal(hp.supportsBevyMpOrb({ getMir2QuestUiStatus: () => JSON.stringify(status) }), false);
  assert.equal(hp.supportsBevyMpOrb({ getMir2QuestUiStatus: () => "broken" }), false);
  assert.equal(hp.supportsBevyMpOrb({ getMir2QuestUiStatus: () => JSON.stringify(mpStatus) }), true);
  assert.deepEqual(hp.currentBevyMpOrb({ ...mpStatus, hpOrb: undefined }, mpSnapshot, true), mpOrbStatus,
    "MP needs its own current image, not HP or an open Quest panel");
  assert.deepEqual(hp.currentBevyMpOrb(mpStatus,
    { ...mpSnapshot, player: { ...mpSnapshot.player, className: "Wizard" } }, true), mpOrbStatus);
  assert.deepEqual(hp.currentBevyMpOrb(mpStatus,
    { ...mpSnapshot, player: { ...mpSnapshot.player, className: "taoist", level: 1 } }, true), mpOrbStatus);
  assert.equal(hp.matchesBevyMpOrbView(mpOrbStatus, mpSnapshot.hpOrbSlot, 25, 100, false), true);
  assert.equal(hp.matchesBevyMpOrbView(mpOrbStatus, mpSnapshot.hpOrbSlot, undefined, 100, false), false);
  const reject = (s = mpStatus, p = mpSnapshot, fresh = true) => assert.equal(hp.currentBevyMpOrb(s, p, fresh), null);
  reject(mpStatus, mpSnapshot, false);
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, ready: false } });
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, revision: 8 } });
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, generation: 4 } });
  reject(mpStatus, { ...mpSnapshot, revision: 10 });
  reject(mpStatus, { ...mpSnapshot, inGame: false });
  reject(mpStatus, { ...mpSnapshot, hpOrbSlot: null });
  reject(mpStatus, { ...mpSnapshot, hpOrbSlot: { left: 1, top: 648 } });
  reject(mpStatus, { ...mpSnapshot, player: { ...mpSnapshot.player, mp: undefined } });
  reject(mpStatus, { ...mpSnapshot, player: { ...mpSnapshot.player, maxMp: null } });
  reject(mpStatus, { ...mpSnapshot, player: { ...mpSnapshot.player, mp: -1 } });
  reject(mpStatus, { ...mpSnapshot, player: { ...mpSnapshot.player, maxMp: 0 } });
  reject(mpStatus, { ...mpSnapshot, player: { ...mpSnapshot.player, level: 25 } });
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, image: "original-ui/Prguse/6.png" } });
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, source: { ...mpOrbStatus.source, left: 0 } } });
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, source: { ...mpOrbStatus.source, top: 59 } } });
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, destination: { ...mpOrbStatus.destination, top: 709 } } });
  reject({ ...mpStatus, mpOrb: { ...mpOrbStatus, layout: { ...mpOrbStatus.layout, width: 49 } } });
  reject(mpStatus, { ...mpSnapshot, presentation: { ...presentation, logicalHeight: 700 } });
  assert.equal(hp.matchesBevyMpOrbView(mpOrbStatus, mpSnapshot.hpOrbSlot, 0, 100, false), false);
  const empty = { ...mpOrbStatus, mp: 0,
    source: { left: 51, top: 80, width: 50, height: 0 },
    destination: { left: 51, top: 728, width: 50, height: 0 },
    layout: { left: 51, top: 728, width: 50, height: 0 } };
  assert.deepEqual(hp.currentBevyMpOrb({ ...mpStatus, mpOrb: empty },
    { ...mpSnapshot, player: { ...mpSnapshot.player, mp: 0 } }, true), empty);
  const full = { ...mpOrbStatus, mp: 100,
    source: { left: 51, top: 0, width: 50, height: 80 },
    destination: { left: 51, top: 648, width: 50, height: 80 },
    layout: { left: 51, top: 648, width: 50, height: 80 } };
  assert.deepEqual(hp.currentBevyMpOrb({ ...mpStatus, mpOrb: full },
    { ...mpSnapshot, player: { ...mpSnapshot.player, mp: 100 } }, true), full);
});

test("hook sends no HP field to old runtime and hands off on fresh model/layout/status", () => {
  const previousWindow = globalThis.window;
  const previousPerformance = globalThis.performance;
  let now = 100, effect, state, timer;
  globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } };
  globalThis.performance = { now: () => now };
  try {
    const refs = []; let index = 0;
    const react = { useRef(value) { return refs[index++] ??= { current: value }; },
      useCallback(fn) { return fn; },
      useEffect(fn) { effect = fn; },
      useState(value) { state ??= value; return [state, (update) => { state = typeof update === "function" ? update(state) : update; }]; } };
    const hook = load("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
      "./bevy-hp-orb": hp, "./bevy-experience-bar": experienceBar,
      "./bevy-weight-bar": load("../lib/bevy-weight-bar.ts"),
      "./bevy-hud-bar-draw-plan": load("../lib/bevy-hud-bar-draw-plan.ts") });
    let sent, supported = false, live = { ...snapshot, dialog: { isOpen: false, hasInput: false },
      questLogOpen: false, openRevision: 0 };
    let currentStatus = { ...status, hpOrb: undefined, frame: 1 };
    const runtime = {
      setMir2QuestUiSnapshot(json) { sent = JSON.parse(json); return true; },
      getMir2QuestUiStatus() { return JSON.stringify(supported ? currentStatus : { ...currentStatus, hpOrb: undefined }); },
      setMir2QuestUiIntentSink() {}, clearMir2QuestUiIntentSink() {},
    };
    const owner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 1, runtimeRef: { current: runtime },
      snapshot: () => live, onIntent: () => ({ accepted: false }), onOpenChange() {} });
    const cleanup = effect();
    assert.equal(Object.hasOwn(sent, "hpOrbSlot"), false, "old deny_unknown_fields runtime sees no new key");
    supported = true;
    currentStatus = { ...status, hpOrb: { ...orbStatus, revision: sent.revision + 1 }, frame: 2,
      revision: sent.revision + 1 };
    timer();
    assert.deepEqual(sent.hpOrbSlot, snapshot.hpOrbSlot);
    currentStatus = { ...currentStatus, hpOrb: { ...orbStatus, revision: sent.revision }, revision: sent.revision, frame: 3 };
    timer();
    assert.equal(state.hpOrb?.hp, 50);
    live = { ...live, player: { ...live.player, hp: 49 } };
    timer();
    assert.equal(state.hpOrb, null, "new model cannot retain previous image owner");
    currentStatus = { ...currentStatus, hpOrb: { ...orbStatus, hp: 49, revision: sent.revision },
      revision: sent.revision, frame: 4 };
    timer();
    assert.equal(state.hpOrb?.hp, 49);
    live = { ...live, hpOrbSlot: null };
    owner.refresh();
    assert.equal(sent.hpOrbSlot, null, "opening a window sends slot=null before the next interval");
    assert.equal(state.hpOrb, null);
    live = { ...live, hpOrbSlot: snapshot.hpOrbSlot };
    owner.refresh();
    currentStatus = { ...currentStatus, hpOrb: { ...orbStatus, hp: 49, revision: sent.revision },
      revision: sent.revision, frame: 5 };
    owner.refresh();
    assert.equal(state.hpOrb?.hp, 49, "closed window reacquires only after a new acknowledgement");
    now += 2001;
    owner.refresh();
    assert.equal(state.hpOrb, null, "stalled renderer yields without a new frame");
    live = { ...live, inGame: false, hpOrbSlot: null };
    owner.refresh();
    assert.equal(sent.inGame, false);
    assert.equal(sent.hpOrbSlot, null, "logout retires the shared image geometry");
    cleanup();
    const retiredRevision = sent.revision;
    owner.refresh();
    assert.equal(sent.revision, retiredRevision, "a retired owner cannot republish after cleanup");
  } finally {
    if (previousWindow === undefined) delete globalThis.window; else globalThis.window = previousWindow;
    if (previousPerformance === undefined) delete globalThis.performance; else globalThis.performance = previousPerformance;
  }
});

test("hook removes nested MP keys for old and HP-only runtimes, then owns MP from a fresh MP-only acknowledgement", () => {
  const previousWindow = globalThis.window;
  const previousPerformance = globalThis.performance;
  let now = 100, effect, state, timer;
  globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } };
  globalThis.performance = { now: () => now };
  try {
    const refs = []; let index = 0;
    const react = { useRef(value) { return refs[index++] ??= { current: value }; },
      useCallback(fn) { return fn; }, useEffect(fn) { effect = fn; },
      useState(value) { state ??= value; return [state, (update) => { state = typeof update === "function" ? update(state) : update; }]; } };
    const hook = load("../lib/use-bevy-quest-ui.ts", { react, "./bevy-quest-ui": host,
      "./bevy-hp-orb": hp, "./bevy-experience-bar": experienceBar,
      "./bevy-weight-bar": load("../lib/bevy-weight-bar.ts"),
      "./bevy-hud-bar-draw-plan": load("../lib/bevy-hud-bar-draw-plan.ts") });
    let sent, capability = "old";
    let live = { ...mpSnapshot, player: { ...mpSnapshot.player, mp: null, maxMp: null },
      dialog: { isOpen: false, hasInput: false }, questLogOpen: false, openRevision: 0 };
    let currentStatus = { ...status, hpOrb: undefined, frame: 1 };
    const runtime = { setMir2QuestUiSnapshot(json) { sent = JSON.parse(json); return true; },
      getMir2QuestUiStatus() { return JSON.stringify({ ...currentStatus,
        hpOrb: capability === "hp" ? orbStatus : undefined,
        mpOrb: capability === "mp" ? { ...mpOrbStatus, revision: sent?.revision ?? 0 } : undefined }); },
      setMir2QuestUiIntentSink() {}, clearMir2QuestUiIntentSink() {} };
    const owner = hook.useBevyQuestUi({ requested: true, runtimeGeneration: 1, runtimeRef: { current: runtime },
      snapshot: () => live, onIntent: () => ({ accepted: false }), onOpenChange() {} });
    const cleanup = effect();
    assert.equal(Object.hasOwn(sent.player, "mp"), false);
    assert.equal(Object.hasOwn(sent.player, "maxMp"), false);
    assert.equal(Object.hasOwn(sent, "hpOrbSlot"), false);
    capability = "hp";
    currentStatus = { ...currentStatus, frame: 2 };
    timer();
    assert.deepEqual(sent.hpOrbSlot, mpSnapshot.hpOrbSlot);
    assert.equal(Object.hasOwn(sent.player, "mp"), false, "HP support alone cannot add MP keys");
    capability = "mp";
    live = { ...live, player: { ...live.player, mp: 25, maxMp: 100 } };
    currentStatus = { ...currentStatus, frame: 3 };
    timer();
    assert.deepEqual([sent.player.mp, sent.player.maxMp], [25, 100]);
    assert.deepEqual(sent.hpOrbSlot, mpSnapshot.hpOrbSlot, "MP-only support retains the shared full-orb slot");
    currentStatus = { ...currentStatus, frame: 4, revision: sent.revision };
    timer();
    assert.equal(state.mpOrb?.mp, 25);
    assert.equal(state.hpOrb, null, "one image side cannot manufacture the other side's ownership");
    live = { ...live, player: { ...live.player, mp: 0 } };
    owner.refresh();
    assert.equal(state.mpOrb, null, "a changed live model immediately yields MP");
    currentStatus = { ...currentStatus, frame: 5, revision: sent.revision,
      mpOrb: { ...mpOrbStatus, mp: 0, revision: sent.revision,
        source: { left: 51, top: 80, width: 50, height: 0 },
        destination: { left: 51, top: 728, width: 50, height: 0 },
        layout: { left: 51, top: 728, width: 50, height: 0 } } };
    runtime.getMir2QuestUiStatus = () => JSON.stringify(currentStatus);
    owner.refresh();
    assert.equal(state.mpOrb?.mp, 0, "known zero MP can own a measured empty image");
    live = { ...live, hpOrbSlot: null };
    owner.refresh();
    assert.equal(sent.hpOrbSlot, null);
    assert.equal(state.mpOrb, null, "modal/geometry handoff retires MP before the next interval");
    live = { ...live, presentation: null };
    owner.refresh();
    assert.equal(sent.presentation, null, "resize loss sends a hidden snapshot without stale MP ownership");
    assert.equal(state.mpOrb, null);
    now += 2001;
    owner.refresh();
    assert.equal(state.mpOrb, null);
    cleanup();
  } finally {
    if (previousWindow === undefined) delete globalThis.window; else globalThis.window = previousWindow;
    if (previousPerformance === undefined) delete globalThis.performance; else globalThis.performance = previousPerformance;
  }
});

test("real GameUiScene reports local menu, shop and durability windows in the layout phase", () => {
  const source = readFileSync(new URL("../app/components/original-client-game-ui-scene.tsx", import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX,
  } }).outputText;
  const values = [], callbacks = [];
  let cursor = 0, layoutEffects = [], cleanups = [];
  const react = { memo: (component) => component,
    useEffect: () => {},
    useLayoutEffect: (effect) => layoutEffects.push(effect),
    useRef: (initial) => ({ current: initial }),
    useState(initial) {
      const index = cursor++;
      if (!(index in values)) values[index] = initial;
      return [values[index], (next) => { values[index] = typeof next === "function" ? next(values[index]) : next; }];
    } };
  const element = (type, props) => ({ type, props });
  function MainHud() {}
  function DuraPanel() {}
  const runtime = { jsx: element, jsxs: element, Fragment: Symbol("fragment") };
  const module = { exports: {} };
  new Function("exports", "module", "require", code)(module.exports, module, (id) => {
    if (id === "react") return react;
    if (id === "react/jsx-runtime") return runtime;
    if (id === "./original-client-overlays") return { MainHud };
    if (id === "./original-client-panels") return { DuraPanel };
    if (id === "./original-client-map-panels") return { hasOriginalMiniMapAsset: () => false };
    if (id === "../../lib/content-profile") return { IS_PLATINUM_176_PROFILE: false };
    return {};
  });
  const props = { world: { activeNpcDialog: null, questLog: [], equipmentItems: [], beltItems: [],
      stage5Systems: { mail: [] }, inventoryItems: [], inSafeZone: false },
    player: null, logs: [], showInventory: false, showCharacter: false, showQuestLog: false,
    onHpOrbModalChange: (blocked) => callbacks.push(blocked) };
  function render() {
    for (const cleanup of cleanups) cleanup();
    cursor = 0;
    layoutEffects = [];
    const tree = module.exports.GameUiScene(props);
    cleanups = layoutEffects.map((effect) => effect()).filter((cleanup) => typeof cleanup === "function");
    return tree;
  }
  function find(node, type) {
    if (!node || typeof node !== "object") return null;
    if (node.type === type) return node;
    const children = node.props?.children;
    for (const child of Array.isArray(children) ? children : [children]) {
      const found = find(child, type);
      if (found) return found;
    }
    return null;
  }
  let tree = render();
  assert.equal(callbacks.at(-1), false);
  find(tree, MainHud).props.onToggleMenu();
  tree = render();
  assert.equal(callbacks.at(-1), true, "system menu blocks shared HP before paint");
  find(tree, MainHud).props.onToggleMenu();
  tree = render();
  assert.equal(callbacks.at(-1), false);
  find(tree, MainHud).props.onToggleGameShop();
  tree = render();
  assert.equal(callbacks.at(-1), true, "local shop also blocks shared HP");
  find(tree, MainHud).props.onToggleGameShop();
  tree = render();
  assert.equal(callbacks.at(-1), false);
  find(tree, DuraPanel).props.onToggle();
  render();
  assert.equal(callbacks.at(-1), true, "durability window is included without changing Quest modal timing");
  for (const cleanup of cleanups) cleanup();
  assert.equal(callbacks.at(-1), false, "unmount releases the local HP gate");
});
