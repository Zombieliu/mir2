import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";
function load(relative, deps = {}) {
  const source = readFileSync(new URL(relative, import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const m = { exports: {} }; new Function("exports", "module", "require", code)(m.exports, m, key => {
    if (!(key in deps)) throw Error(`Unexpected dependency ${key}`); return deps[key];
  }); return m.exports;
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

test("actual shell handler keeps uncovered HUD available with Bag/Quest open and preserves foreground/world routes", () => {
  const file = ts.createSourceFile("shell.tsx", readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const functions = [];
  const visit = node => { if (ts.isFunctionDeclaration(node) && ["handleSharedHudPointer", "cancelSharedHudPointer"].includes(node.name?.text)) functions.push(node.getText(file)); ts.forEachChild(node, visit); };
  visit(file); assert.equal(functions.length, 2);
  const compiled = ts.transpileModule(functions.join("\n"), { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  class Element { constructor(id) { this.id = id; } setPointerCapture() {} }
  const router = new hud.HudPointerRouter(), held = { current: null }, bag = { current: { held: null } }, actions = [], moves = [];
  let foreground = [], current = status({ navigation: { ...navigation, bagOpen: true } });
  const scope = { HTMLElement: Element, sharedUiCanvasId: () => "shared", webGl2SharedCanvasPrototype: true, bevyHudUiReady: true,
    hudPointerRouterRef: { current: router }, bagPointerRouterRef: bag, heldScenePointerRef: held,
    characterPointerCallbacksRef: { current: { getBevyCharacterPointerContext: () => null } },
    spellsPointerCallbacksRef: { current: { getBevySpellsPointerContext: () => null } },
    readBevyHudStatus: () => current, stageFrameRef: { current: { focus() {} } }, sceneInteractionReady: true,
    scenePointFromMouseEvent: event => ({ sceneX: event.clientX, sceneY: event.clientY }),
    bagPointerCallbacksRef: { current: { getBevyBagPointerContext: () => ({ inputRegions: foreground }) } }, questLocalModalOpen: false, mobileMoreOpen: false,
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
  foreground = []; bag.current.held = { origin: "ui" }; assert.equal(handler(edge(930, 694), "down"), false, "existing Bag drag cannot pierce HUD"); bag.current.held = null;
  handler(edge(400, 400), "down"); handler(edge(930, 694), "move"); handler(edge(930, 694), "up");
  assert.equal(moves.length, 1); assert.equal(moves[0].sceneX, 930, "world origin retains the existing world release path");
  handler(edge(400, 400), "down"); current = status({ generation: 8 }); handler(edge(930, 694), "up"); assert.equal(moves.length, 1);
  assert.match(file.text, /\[bevySpellsPageReady, showCharacter, activeCharacterTab, bevyCharacterPageReady, bevyHudUiReady, bevyMapRuntimeGeneration, screen, player\?\.objectId/);
  assert.match(file.text, /const blur = \(\) => \{ cancelSharedSpellsPointer\(\);\s*cancelSharedCharacterPointer\(\); cancelSharedBagPointer\("blur"\); cancelSharedHudPointer\(\); \}/);
});
