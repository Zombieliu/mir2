import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
function load(relative, deps = {}, globals = {}) {
  const source = readFileSync(new URL(relative, import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const m = { exports: {} }; new Function("exports", "module", "require", ...Object.keys(globals), code)(m.exports, m, key => {
    if (!(key in deps)) throw Error(`Unexpected dependency ${key}`); return deps[key];
  }, ...Object.values(globals)); return m.exports;
}
const hud = load("../lib/bevy-hud-ui.ts");
const caps = { schemaVersion: 1, hudUiAbiVersion: 1, characterStatsAbiVersion: 1, compiled: true, startup: true };
const navigation = { characterOpen: false, characterPage: "character", bagOpen: false, questOpen: false };
const rect = (left, top, width, height) => ({ left, top, width, height });
const plan = { hp: "HP 10/20", mp: "MP 4/8 ", name: "Live", level: "12", gold: "1", experience: "25%", weight: "2", hpOnly: false,
  main: rect(0, 616, 1024, 152), orb: rect(0, 646, 104, 80), experienceBar: rect(9, 759, 1004, 8), weightBar: rect(919, 719, 76, 12),
  buttons: ["character", "bag", "skill", "quest", "option", "menu", "gameShop"].map((action, i) => ({ action, rect: rect(905 + i * 23, 692, 20, 20), normal: "skin", hover: "skin", pressed: "skin" })),
  characterRect: rect(760, 0, 264, 380), characterHits: [{ action: { type: "closeCharacter" }, rect: rect(1001, 3, 24, 21) },
    { action: { type: "selectCharacterPage", page: "stats2" }, rect: rect(892, 70, 64, 20) }] };
const status = (changes = {}) => ({ version: 1, frame: 1, ready: true, characterStatsReady: false, generation: 7, revision: 1, navigation,
  plan, error: null, foregroundRects: [], modal: false, ...changes });
test("independent capability is exact, old bundle and lean retain compatibility", () => {
  assert.equal(hud.supportsHud({}), false);
  assert.equal(hud.readHudCapabilities({ getMir2HudUiCapabilities: () => JSON.stringify({ ...caps, unknown: true }) }), null);
  const runtime = { getMir2HudUiCapabilities: () => JSON.stringify(caps), getMir2HudSourceGeometry: () => JSON.stringify(plan), setMir2HudUiSnapshot() {}, getMir2HudUiStatus() {}, dispatchMir2HudNavigation() {} };
  assert.equal(hud.supportsHud(runtime), true);
  runtime.getMir2HudUiCapabilities = () => JSON.stringify({ ...caps, compiled: false, startup: false, hudUiAbiVersion: 0, characterStatsAbiVersion: 0 });
  assert.equal(hud.supportsHud(runtime), false);
  const firstRevision = hud.nextHudNavigationRevision();
  assert.equal(load("../lib/bevy-hud-ui.ts").nextHudNavigationRevision(), firstRevision + 1, "a remounted/reloaded module cannot regress the navigation clock");
});
test("HUD geometry chooses owner before movement, foreground priority and crossing drags", () => {
  const router = new hud.HudPointerRouter();
  let lease = router.down(status({ navigation: { ...navigation, bagOpen: true } }), 1, 930, 694, false);
  assert.equal(lease.origin, "ui"); assert.equal(lease.action, "bag");
  assert.equal(router.held.origin, "ui", "drag out retains the UI origin");
  assert.equal(router.down(status(), 2, 40, 40, false), null, "second pointer cannot replace a lease");
  router.cancel(); lease = router.down(status({ navigation: { ...navigation, questOpen: true } }), 1, 976, 694, false);
  assert.equal(lease.action, "quest", "uncovered HUD Quest closes an open quest");
  router.cancel(); assert.equal(router.down(status({ foregroundRects: [rect(900, 680, 90, 90)] }), 1, 930, 694, false), null);
  assert.equal(router.down(status({ modal: true }), 1, 905, 692, false), null);
  lease = router.down(status(), 1, 400, 400, false); assert.equal(lease.origin, "world");
  assert.equal(router.held.origin, "world", "world drag entering HUD remains world");
  assert.equal(router.matches(status({ generation: 8 })), false); router.cancel();
  lease = router.down(status({ characterStatsReady: true }), 1, 1007, 12, false);
  assert.deepEqual(lease.action, { type: "closeCharacter" }); router.cancel();
  lease = router.down(status({ characterStatsReady: true }), 1, 900, 78, false);
  assert.deepEqual(lease.action, { type: "selectCharacterPage", page: "stats2" });
  router.cancel(); assert.equal(router.held, null, "same-frame up/cancel leaves no world hold");
});
test("hook ready loss/null/throw/HMR withdraws captured runtime using last valid snapshot", () => {
  let effect, timer, cleanup, state = null;
  const priorWindow = globalThis.window;
  globalThis.window = { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } };
  const react = { useRef: value => ({ current: value }), useCallback: fn => fn,
    useState: value => [value, setter => { state = typeof setter === "function" ? setter(state) : setter; }],
    useEffect: fn => { effect = fn; } };
  const hook = load("../lib/use-bevy-hud-ui.ts", { react, "./bevy-hud-ui": hud });
  let live = { generation: 7, inGame: true, hostVisible: true, logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: true,
    player: { hp: 10, maxHp: 20, mp: 4, maxMp: 8, name: "Live", gold: 1 }, navigation };
  let applied = status(); const sent = [], actions = []; let asynchronous = false, pending = null;
  const runtime = { getMir2HudUiCapabilities: () => JSON.stringify(caps), getMir2HudSourceGeometry: () => JSON.stringify(plan),
    setMir2HudUiSnapshot(json) { const snapshot = JSON.parse(json); sent.push(snapshot);
      if (asynchronous) pending = snapshot;
      else applied = status({ frame: applied.frame + 1, generation: snapshot.generation, revision: snapshot.revision, ready: snapshot.hostVisible && snapshot.inGame }); return true; },
    getMir2HudUiStatus: () => JSON.stringify(applied), dispatchMir2HudNavigation: json => { actions.push(JSON.parse(json)); return true; } };
  try {
    const owner = hook.useBevyHudUi({ requested: true, runtimeGeneration: 1, runtimeRef: { current: runtime },
      navigationSource: () => "session:1/role:0/object:7/realm:1/world:1/scene:1",
      snapshot: () => { if (live === "throw") throw Error("geometry failed"); return live; }, onNavigation() {} });
    cleanup = effect(); assert.equal(state.ready, true); assert.equal(owner.dispatch({ type: "bag" }), true); assert.equal(actions.length, 1);
    const valid = live; live = null; timer(); assert.equal(sent.at(-1).hostVisible, false);
    assert.equal(sent.at(-1).player.name, "Live"); assert.equal(sent.at(-1).inGame, true, "withdrawal is not logout");
    live = valid; timer(); assert.equal(state.ready, true);
    asynchronous = true; live = { ...valid, player: { ...valid.player, hp: 11 } }; timer();
    assert.equal(sent.at(-1).hostVisible, true, "a pending visible full must not be overwritten by withdrawal");
    assert.equal(state.ready, true, "existing painter remains during the bounded frame wait");
    applied = status({ frame: applied.frame + 1, revision: pending.revision }); pending = null; timer();
    assert.equal(state.ready, true); asynchronous = false;
    applied = { ...applied, frame: applied.frame + 1, ready: false }; timer();
    assert.equal(sent.at(-1).hostVisible, false, "actual applied ready loss still withdraws the root");
    live = valid; timer(); assert.equal(state.ready, true);
    live = "throw"; timer(); assert.equal(sent.at(-1).hostVisible, false); assert.equal(state, null);
    live = valid; timer(); live = null; cleanup(); assert.equal(sent.at(-1).hostVisible, false); assert.equal(timer, null);
  } finally { globalThis.window = priorWindow; }
});

// Run the actual hook with persistent React slots, a controllable native
// mailbox and a clock. The mock Native receiver enforces its real exact token
// contract; it does not reproduce the hook's source/baseline eligibility.
function navigationFixture({ asynchronous = false, initialReady = true } = {}) {
  let now = 100, timer = null, cleanup = null, pendingEffect = null, cursor = 0;
  const slots = [], sent = [], actions = [], projected = [];
  let applied = status({ revision: 0, ready: initialReady });
  let appliedNavigationRevision = 0;
  let pending = null, result = "accept", capability = caps, onStatusRead = null, onSourceRead = null;
  let source = "session:1/role:0/object:7/realm:1/world:1/context:1/scene:1";
  let live = { generation: 7, inGame: true, hostVisible: true, logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1,
    touch: false, hpView: true, navigationRevision: 1, navigation,
    player: { hp: 10, maxHp: 20, mp: 4, maxMp: 8, name: "Live", className: "Warrior", gender: "Male", level: 12,
      gold: 1, credit: 0, experience: 25, maxExperience: 100, currentWeight: 2, currentWeightKnown: true, maxWeight: 50 } };
  const ingest = () => {
    if (!pending) return;
    const snapshot = pending; pending = null;
    applied = status({ frame: applied.frame + 1, generation: snapshot.generation, revision: snapshot.revision,
      ready: snapshot.inGame && snapshot.hostVisible && !!snapshot.player,
      navigation: snapshot.generation !== applied.generation || snapshot.navigationRevision > appliedNavigationRevision
        ? snapshot.navigation : applied.navigation });
    appliedNavigationRevision = Math.max(appliedNavigationRevision, snapshot.navigationRevision ?? 0);
  };
  const runtime = { getMir2HudUiCapabilities: () => JSON.stringify(capability), getMir2HudSourceGeometry: () => JSON.stringify(plan),
    setMir2HudUiSnapshot(json) { pending = JSON.parse(json); sent.push(pending); if (!asynchronous) ingest(); return true; },
    getMir2HudUiStatus() { onStatusRead?.(); return JSON.stringify(applied); },
    dispatchMir2HudNavigation(json) {
      const edge = JSON.parse(json); actions.push(edge);
      if (result === "throw") throw Error("native dispatch failed");
      if (result === "reject" || !applied.ready || edge.generation !== applied.generation || edge.revision !== applied.revision) return false;
      if (edge.action.type === "bag") applied = { ...applied, navigation: { ...applied.navigation, bagOpen: !applied.navigation.bagOpen } };
      return true;
    } };
  const options = { requested: true, runtimeGeneration: 1, runtimeRef: { current: runtime },
    snapshot: () => live, navigationSource() { onSourceRead?.(); return source; }, onNavigation: value => projected.push(value) };
  const react = {
    useRef(value) { const index = cursor++; return slots[index] ??= { current: value }; },
    useCallback(fn, deps) { const index = cursor++, old = slots[index];
      if (!old || deps.some((dep, i) => !Object.is(dep, old.deps[i]))) slots[index] = { deps, value: fn };
      return slots[index].value;
    },
    useState(value) { const index = cursor++; slots[index] ??= { value }; return [slots[index].value, update => {
      slots[index].value = typeof update === "function" ? update(slots[index].value) : update;
    }]; },
    useEffect(effect, deps) { const index = cursor++, old = slots[index];
      if (!old || deps.some((dep, i) => !Object.is(dep, old.deps[i]))) pendingEffect = effect;
      slots[index] = { deps };
    } };
  const hook = load("../lib/use-bevy-hud-ui.ts", { react, "./bevy-hud-ui": hud }, {
    performance: { now: () => now }, window: { setInterval(fn) { timer = fn; return 1; }, clearInterval() { timer = null; } } });
  let owner;
  const render = (changes = {}) => { Object.assign(options, changes); cursor = 0; owner = hook.useBevyHudUi(options); return owner; };
  const effects = () => { if (pendingEffect) { cleanup?.(); cleanup = pendingEffect(); pendingEffect = null; } };
  render(); effects();
  return {
    get owner() { return owner; }, get live() { return live; }, set live(value) { live = value; },
    get source() { return source; }, set source(value) { source = value; },
    get native() { return applied; }, set native(value) { applied = value; },
    get pending() { return pending; }, get timer() { return timer; }, get asynchronous() { return asynchronous; },
    set asynchronous(value) { asynchronous = value; }, set result(value) { result = value; }, set capability(value) { capability = value; },
    set onStatusRead(value) { onStatusRead = value; }, set onSourceRead(value) { onSourceRead = value; },
    options, runtime, sent, actions, projected, render, effects, ingest,
    tick() { timer?.(); }, advance(ms) { now += ms; }, cleanup() { cleanup?.(); cleanup = null; }
  };
}

function pageHudBootstrapFixture() {
  const file = ts.createSourceFile("page.tsx", readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8"),
    ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const setterNames = ["setShowQuestLog", "setShowInventory", "setShowCharacter", "setActiveCharacterTab"];
  const setters = new Map();
  let snapshotSource, projectionSource;
  const visit = node => {
    if (ts.isFunctionDeclaration(node) && setterNames.includes(node.name?.text)) setters.set(node.name.text, node.getText(file));
    if (ts.isVariableDeclaration(node) && node.name.getText(file) === "bevyHudUi") {
      assert(ts.isCallExpression(node.initializer));
      const options = node.initializer.arguments[0];
      assert(ts.isObjectLiteralExpression(options));
      snapshotSource = options.properties.find(property => property.name?.getText(file) === "snapshot")?.initializer.getText(file);
      projectionSource = options.properties.find(property => property.name?.getText(file) === "onNavigation")?.initializer.getText(file);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  assert.equal(setters.size, setterNames.length);
  assert.ok(snapshotSource && projectionSource, "use the actual Page snapshot and Native navigation projection");
  const code = ts.transpileModule([...setters.values(), `const snapshot = ${snapshotSource};`,
    `const onNavigation = ${projectionSource};`].join("\n"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const hook = navigationFixture({ asynchronous: true, initialReady: false });
  let rendered = { quest: false, bag: false, character: false, tab: "char" };
  const queued = { ...rendered };
  const scope = {
    questLogOpenRef: { current: false }, bagOpenRef: { current: false }, characterOpenRef: { current: false },
    activeCharacterTabRef: { current: "char" }, fallbackHudNavigationRevisionRef: { current: hud.nextHudNavigationRevision() },
    sharedHudNavigationRef: { current: { ready: false, dispatch: hook.owner.dispatch } }, hudProjectionRef: { current: false },
    ownedTooltipEpochRef: { current: { bag: {}, character: {} } },
    setShowQuestLogState(value) { queued.quest = value; },
    setShowInventoryState(value) { queued.bag = value; },
    setShowCharacterState(value) { queued.character = value; },
    // useImmediateUiState publishes its ref before React commits the value.
    setActiveCharacterTabState(value) { scope.activeCharacterTabRef.current = value; queued.tab = value; },
    nextHudNavigationRevision: hud.nextHudNavigationRevision,
    navigateSharedHud() { assert.fail("cold bootstrap cannot dispatch through an unready Native owner"); },
    endStorageService() {}, bagYieldRef: { current() {} }, bagCompatibilityModeRef: { current: null },
    setBagCompatibilityMode() {}, bevyBagUiRequested: false, setStorageServiceOpenVersion() {},
    worldRef: { current: { entities: [{ objectId: 7, kind: "selfPlayer" }], playerObjectId: 7, connected: true } },
    screenRef: { current: "game" }, document: { visibilityState: "visible", querySelector() { return {}; } },
    readBevyQuestPresentation() { return { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false }; },
    sharedUiCanvasId() { return "shared"; }, webGl2SharedCanvasPrototype: true, clientProfile: { input: "mouse" },
    bevyQuestGenerationRef: { current: 7 }, bevyQuestUiRequested: true, initialSceneAssetsReadyRef: { current: true },
    runtimeRef: { current: hook.runtime }, projectAuthoritativeHudPlayer() { return hook.live.player; },
    playerUiPreferencesRef: { current: { hpView: true } },
  };
  let page;
  const render = () => {
    const bindings = { ...scope, showQuestLog: rendered.quest, showInventory: rendered.bag,
      showCharacter: rendered.character, activeCharacterTab: rendered.tab };
    page = new Function(...Object.keys(bindings), `${code}\nreturn { snapshot, onNavigation, ${setterNames.join(", ")} };`)(...Object.values(bindings));
    hook.render({ snapshot: page.snapshot, onNavigation: page.onNavigation });
  };
  render(); hook.tick();
  return {
    hook, scope, get page() { return page; }, get rendered() { return rendered; },
    flushReact() { rendered = { ...queued }; render(); },
    cleanup() { hook.cleanup(); },
  };
}

for (const scenario of [
  { name: "Q diary", edit(page) { page.setShowQuestLog(true); }, expected: { ...navigation, questOpen: true } },
  { name: "Bag", edit(page) { page.setShowInventory(true); }, expected: { ...navigation, bagOpen: true } },
  { name: "Character", edit(page) { page.setShowCharacter(true); }, expected: { ...navigation, characterOpen: true } },
  { name: "Character Skills tab", edit(page) { page.setActiveCharacterTab("spells"); page.setShowCharacter(true); },
    expected: { ...navigation, characterOpen: true, characterPage: "spells" } },
]) {
  test(`cold Page ${scenario.name} survives a HUD tick before React commits and the first Native projection`, () => {
    const f = pageHudBootstrapFixture();
    try {
      assert.equal(f.hook.owner.ready, false);
      const initialNavigationRevision = f.hook.pending.navigationRevision;
      scenario.edit(f.page);
      f.hook.tick();
      const submitted = f.hook.pending;
      assert.ok(submitted.navigationRevision > initialNavigationRevision, "the real Page setter advances the explicit navigation clock");
      assert.deepEqual(submitted.navigation, scenario.expected,
        "the new navigation revision must carry the immediate requested values before React re-renders");
      const submissions = f.hook.sent.length;
      f.flushReact(); f.hook.tick();
      assert.equal(f.hook.sent.length, submissions, "committing the same request needs no second snapshot or replay");
      assert.equal(f.hook.pending, submitted);
      f.hook.ingest(); f.hook.tick();
      assert.deepEqual(f.hook.native.navigation, scenario.expected, "Native accepts the exact requested bootstrap navigation");
      assert.equal(f.scope.hudProjectionRef.current, false);
      f.flushReact();
      assert.deepEqual(f.page.snapshot().navigation, scenario.expected, "Native projection must retain the user's opening and selected tab");
      assert.deepEqual(f.rendered, { quest: scenario.expected.questOpen, bag: scenario.expected.bagOpen,
        character: scenario.expected.characterOpen, tab: scenario.expected.characterPage === "character" ? "char" : scenario.expected.characterPage });
      f.hook.tick();
      assert.equal(f.hook.sent.length, submissions, "the Native echo cannot invent another compatibility edit");
      assert.equal(f.hook.actions.length, 0, "cold bootstrap never queues or retries a Native navigation edge");
    } finally { f.cleanup(); }
  });
}

test("actual hook opens Bag once during dirty HP and again during a pending cosmetic ingest without relaxing readCurrent", () => {
  const f = navigationFixture();
  try {
    assert.ok(f.owner.readCurrent());
    f.live = { ...f.live, player: { ...f.live.player, hp: 9 } };
    assert.equal(f.owner.readCurrent(), null, "full freshness rejects a stat projection not yet submitted");
    const dirtyToken = { generation: f.native.generation, revision: f.native.revision };
    assert.equal(f.owner.dispatch({ type: "bag" }), true);
    assert.equal(f.native.navigation.bagOpen, true); assert.equal(f.actions.length, 1);
    assert.deepEqual(f.actions[0], { ...dirtyToken, action: { type: "bag" } });
    f.tick(); assert.equal(f.actions.length, 1, "refresh cannot replay an accepted edge");
    assert.equal(f.projected.at(-1).bagOpen, true, "the accepted native navigation is projected on the next frame");
    f.asynchronous = true; f.live = { ...f.live, player: { ...f.live.player, hp: 8, gold: 2 } }; f.tick();
    assert.ok(f.pending.revision > f.native.revision);
    assert.equal(f.owner.readCurrent(), null, "economic/geometry reads still await the latest submission");
    const pendingToken = { generation: f.native.generation, revision: f.native.revision };
    assert.equal(f.owner.dispatch({ type: "bag" }), true);
    assert.equal(f.native.navigation.bagOpen, false); assert.equal(f.actions.length, 2);
    assert.deepEqual(f.actions[1], { ...pendingToken, action: { type: "bag" } }, "dispatch uses current Native, never the queued revision");
    f.ingest(); f.tick(); assert.ok(f.owner.readCurrent()); assert.equal(f.actions.length, 2);
    assert.equal(f.native.navigation.bagOpen, false);
    const diagnostic = f.owner.diagnostic().at(-1);
    assert.equal(diagnostic.accepted, true); assert.equal(diagnostic.fresh, false);
    assert.equal(diagnostic.appliedPending, true); assert.equal(diagnostic.navigationSourceMatches, true);
    assert.equal(diagnostic.nativeBaseline, true); assert.equal(diagnostic.navigationFresh, true);
  } finally { f.cleanup(); }
});

test("a native ready baseline must first be fully ingested for this hook owner", () => {
  const f = navigationFixture({ asynchronous: true });
  try {
    assert.equal(f.native.ready, true, "an earlier native painter may already be ready");
    assert.equal(f.owner.readCurrent(), null); assert.equal(f.owner.dispatch({ type: "bag" }), false);
    assert.equal(f.actions.length, 0);
    f.ingest(); f.tick(); assert.equal(f.owner.dispatch({ type: "bag" }), true); assert.equal(f.actions.length, 1);
  } finally { f.cleanup(); }
});

test("a new runtime generation cannot inherit its predecessor's ready baseline before its fresh ingest", () => {
  const f = navigationFixture();
  try {
    f.asynchronous = true; f.render({ runtimeGeneration: 2 });
    assert.equal(f.owner.dispatch({ type: "bag" }), false, "render retires the old runtime-generation proof immediately");
    f.effects(); assert.equal(f.native.ready, true, "the previous native painter may still be visible");
    assert.equal(f.owner.readCurrent(), null); assert.equal(f.owner.dispatch({ type: "bag" }), false);
    assert.equal(f.actions.length, 0); f.ingest(); f.tick();
    assert.equal(f.owner.dispatch({ type: "bag" }), true); assert.equal(f.actions.length, 1);
  } finally { f.cleanup(); }
});

test("pending source identity/realm/world/context/scene must ingest a new revision even with the same HUD projection", () => {
  for (const changedSource of ["session:2", "role:1/object:8", "realm:2", "world:2", "context:2", "scene:2"]) {
    const f = navigationFixture();
    try {
      const priorRevision = f.native.revision, priorProjection = JSON.stringify(f.live);
      f.asynchronous = true; f.source += `/${changedSource}`;
      assert.equal(f.owner.dispatch({ type: "bag" }), false, changedSource);
      assert.equal(f.actions.length, 0); f.tick();
      assert.equal(JSON.stringify(f.live), priorProjection, "the identity fence is outside the Native ABI");
      assert.ok(f.pending.revision > priorRevision, `${changedSource} requires a new native acknowledgement`);
      assert.equal(f.owner.readCurrent(), null); assert.equal(f.owner.dispatch({ type: "bag" }), false);
      assert.equal(f.actions.length, 0);
      f.ingest(); f.tick(); assert.equal(f.owner.dispatch({ type: "bag" }), true); assert.equal(f.actions.length, 1);
    } finally { f.cleanup(); }
  }
});

test("pending layout, explicit navigation and player identity changes cannot borrow the cosmetic baseline", () => {
  for (const [label, change] of [
    ["width", live => ({ ...live, logicalWidth: 1200 })], ["height", live => ({ ...live, logicalHeight: 800 })],
    ["scale", live => ({ ...live, stageCssScale: 0.8 })], ["touch", live => ({ ...live, touch: true })],
    ["presentation", live => ({ ...live, hpView: false })], ["explicit navigation", live => ({ ...live, navigationRevision: 2 })],
    ["generation", live => ({ ...live, generation: 8 })], ["name", live => ({ ...live, player: { ...live.player, name: "Other" } })],
    ["class", live => ({ ...live, player: { ...live.player, className: "Wizard" } })],
    ["gender", live => ({ ...live, player: { ...live.player, gender: "Female" } })]
  ]) {
    const f = navigationFixture();
    try {
      f.asynchronous = true; f.live = change(f.live);
      assert.equal(f.owner.dispatch({ type: "bag" }), false, label); f.tick();
      assert.equal(f.owner.readCurrent(), null); assert.equal(f.owner.dispatch({ type: "bag" }), false, `pending ${label}`);
      assert.equal(f.actions.length, 0); f.ingest(); f.tick();
      assert.equal(f.owner.dispatch({ type: "bag" }), true, `fresh ${label}`); assert.equal(f.actions.length, 1);
    } finally { f.cleanup(); }
  }
});

test("hidden, inactive, absent player/source, unknown capability, native loss/error and expired/regressed frame reject", () => {
  for (const [label, invalidate] of [
    ["hidden", f => { f.live = { ...f.live, hostVisible: false }; }],
    ["inactive", f => { f.live = { ...f.live, inGame: false }; }],
    ["absent player", f => { f.live = { ...f.live, player: null }; }],
    ["missing source", f => { f.source = null; }], ["empty source", f => { f.source = ""; }],
    ["unknown capability", f => { f.capability = { ...caps, unknown: true }; }],
    ["native not ready", f => { f.native = { ...f.native, ready: false }; }],
    ["native error", f => { f.native = { ...f.native, error: "renderer failed" }; }],
    ["expired frame", f => { f.advance(2001); }],
    ["regressed revision", f => { f.native = { ...f.native, revision: f.native.revision - 1 }; }],
    ["regressed frame", f => { f.native = { ...f.native, frame: f.native.frame - 1 }; }],
    ["not requested", f => { f.render({ requested: false }); }],
    ["runtime object", f => { f.options.runtimeRef.current = { ...f.runtime }; }],
    ["runtime generation", f => { f.render({ runtimeGeneration: 2 }); }]
  ]) {
    const f = navigationFixture();
    try { invalidate(f); assert.equal(f.owner.dispatch({ type: "bag" }), false, label); assert.equal(f.actions.length, 0, label); }
    finally { f.cleanup(); }
  }
});

test("Native false/throw is final, diagnostics stay bounded and cleanup cannot authorize or replay", () => {
  for (const result of ["reject", "throw"]) {
    const f = navigationFixture();
    try {
      f.result = result;
      assert.equal(f.owner.dispatch({ type: "bag" }), false); assert.equal(f.actions.length, 1);
      f.result = "accept"; f.tick(); f.ingest(); f.tick();
      assert.equal(f.actions.length, 1, `${result} cannot queue/retry later`); assert.equal(f.native.navigation.bagOpen, false);
      for (let i = 0; i < 10; i++) { f.source = null; assert.equal(f.owner.dispatch({ type: "bag" }), false); }
      assert.equal(f.owner.diagnostic().length, 8);
      const copy = f.owner.diagnostic(); copy.length = 0; assert.equal(f.owner.diagnostic().length, 8);
      f.asynchronous = true; f.cleanup(); assert.equal(f.timer, null);
      assert.equal(f.sent.at(-1).hostVisible, false); assert.equal(f.native.ready, true, "cleanup may await Native ingest");
      assert.equal(f.owner.dispatch({ type: "bag" }), false); f.owner.refresh(); assert.equal(f.actions.length, 1);
    } finally { f.cleanup(); }
  }
});

test("source/runtime/visibility/capability or cleanup changes while reading Native are rechecked before the single dispatch", () => {
  for (const [label, retire] of [
    ["source", f => { f.source += "/realm:2"; }],
    ["runtime", f => { f.options.runtimeRef.current = { ...f.runtime }; }],
    ["runtime generation", f => { f.options.runtimeGeneration++; }],
    ["visibility", f => { f.live = { ...f.live, hostVisible: false }; }],
    ["capability", f => { f.capability = { ...caps, unknown: true }; }],
    ["cleanup", f => f.cleanup()]
  ]) {
    const f = navigationFixture();
    try {
      f.asynchronous = true;
      f.onStatusRead = () => { f.onStatusRead = null; retire(f); };
      assert.equal(f.owner.dispatch({ type: "bag" }), false, label); assert.equal(f.actions.length, 0, label);
    } finally { f.cleanup(); }
  }
  const f = navigationFixture();
  try {
    let reads = 0;
    f.asynchronous = true;
    f.onSourceRead = () => { if (++reads === 2) { f.onSourceRead = null; f.cleanup(); } };
    assert.equal(f.owner.dispatch({ type: "bag" }), false, "cleanup inside the final host source read also retires the owner");
    assert.equal(f.actions.length, 0);
  } finally { f.cleanup(); }
});

test("actual shell handler keeps uncovered HUD available with Bag/Quest open and preserves foreground/world routes", () => {
  const file = ts.createSourceFile("shell.tsx", readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const functions = [];
  const fishingFunctions = [];
  const visit = node => { if (ts.isFunctionDeclaration(node) && ["handleSharedHudPointer", "cancelSharedHudPointer"].includes(node.name?.text)) functions.push(node.getText(file)); ts.forEachChild(node, visit); };
  visit(file); assert.equal(functions.length, 2);
  // Keep the real held-pointer cancellation path; this HUD fixture has no fishing lease.
  const visitFishing = node => {
    if (ts.isFunctionDeclaration(node) && ["cancelWorldFishingHeldPointer", "retireWorldFishingPhysical"].includes(node.name?.text))
      fishingFunctions.push(node.getText(file));
    ts.forEachChild(node, visitFishing);
  };
  visitFishing(file); assert.equal(fishingFunctions.length, 2);
  const compiled = ts.transpileModule([...functions, ...fishingFunctions].join("\n"), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  class Element { constructor(id) { this.id = id; } setPointerCapture() {} }
  const router = new hud.HudPointerRouter(), held = { current: null }, bag = { current: { held: null } }, actions = [], moves = [];
  let foreground = [], current = status({ navigation: { ...navigation, bagOpen: true } });
  const scope = { parityUiBlocksGameplay:undefined, onHeroShortcut:undefined, HTMLElement: Element, sharedUiCanvasId: () => "shared", webGl2SharedCanvasPrototype: true, bevyHudUiReady: true, bevyBagUiActive: true,
    hudPointerRouterRef: { current: router }, bagPointerRouterRef: bag, heldScenePointerRef: held,
    worldFishingPhysicalRef: { current: null }, worldFishingTerminalRef: { current: null },
    characterPointerCallbacksRef: { current: { getBevyCharacterPointerContext: () => null } },
    spellsPointerCallbacksRef: { current: { getBevySpellsPointerContext: () => null } },
    readBevyHudStatus: () => current, stageFrameRef: { current: { focus() {} } }, sceneInteractionReady: true,
    scenePointFromMouseEvent: event => ({ sceneX: event.clientX, sceneY: event.clientY }),
    bagPointerCallbacksRef: { current: { getBevyBagPointerContext: () => foreground && { inputRegions: foreground } } }, questLocalModalOpen: false, mobileMoreOpen: false,
    dispatchBevyHudNavigation: action => actions.push(action), onViewportDirectionStop() {},
    stopHeldScenePointer() { moves.push(held.current); }, window: { dispatchEvent() {} }, CustomEvent: class {} };
  const handler = new Function(...Object.keys(scope), `${compiled}\nreturn handleSharedHudPointer;`)(...Object.values(scope));
  const edge = (x, y) => ({ target: new Element("shared"), pointerId: 1, pointerType: "touch", button: 0, clientX: x, clientY: y, preventDefault() {} });
  assert.equal(handler(edge(930, 694), "down"), true); assert.deepEqual(actions.at(-1), { type: "bag" });
  handler(edge(400, 400), "move"); handler(edge(400, 400), "up"); assert.equal(moves.length, 0);
  handler(edge(907, 694), "down"); assert.deepEqual(actions.at(-1), { type: "character" }); handler(edge(907, 694), "up");
  current = status({ navigation: { ...navigation, questOpen: true } }); handler(edge(976, 694), "down");
  assert.deepEqual(actions.at(-1), { type: "quest" }); handler(edge(976, 694), "up");
  foreground = [rect(900, 680, 100, 70)]; assert.equal(handler(edge(930, 694), "down"), false, "actual Bag rectangle receives overlapping point");
  foreground = null;
  const actionsBeforeMissingLayout = actions.length;
  for (const point of [edge(930, 694), edge(400, 400)]) {
    assert.equal(handler(point, "down"), true, "missing live Bag geometry consumes the new press");
    assert.equal(router.held, null);
    assert.equal(held.current, null);
    assert.equal(actions.length, actionsBeforeMissingLayout);
  }
  foreground = []; bag.current.held = { origin: "ui" }; assert.equal(handler(edge(930, 694), "down"), false, "existing Bag drag cannot pierce HUD"); bag.current.held = null;
  handler(edge(400, 400), "down"); handler(edge(930, 694), "move"); handler(edge(930, 694), "up");
  assert.equal(moves.length, 1); assert.equal(moves[0].sceneX, 930, "world origin retains the existing world release path");
  handler(edge(400, 400), "down"); current = status({ generation: 8 }); handler(edge(930, 694), "up"); assert.equal(moves.length, 1);
  assert.match(file.text, /\[bevyHeroUiToken, npcRepairView\?\.stamp, npcRepairService, showInventory, activeInventoryTab, bevyMailPageReady,bevyMailComposeReady,bevyMailComposePending,mailOpen,bevySpellsPageReady, showCharacter, activeCharacterTab, bevyCharacterPageReady, bevyHudUiReady, bevyMapRuntimeGeneration, screen, player\?\.objectId, bevyBagOwnerRevision, bevyBagUiActive, bevyStorageOwnerRevision, bevyStorageUiActive, bevyStorageUiTransitioning, bevyNpcShopUiActive, bevyNpcShopUiTransitioning,/);
  assert.match(file.text, /const blur = \(\) => \{ cancelSharedHeroPointer\(\); cancelBagBeltPointer\(\); cancelBagBeltPointer\(bagBeltArmedSharedRef\.current\); cancelNpcRepairPointer\(\); heldQuestControlPointersRef\.current\.clear\(\); cancelSharedNpcShopPointer\("blur"\); cancelSharedStoragePointer\("blur"\); cancelSharedComposePointer\(\);cancelSharedMailPointer\(\);cancelSharedSpellsPointer\(\);cancelSharedCharacterPointer\(\); cancelSharedBagPointer\("blur"\); cancelSharedHudPointer\(\); \}/);
});
