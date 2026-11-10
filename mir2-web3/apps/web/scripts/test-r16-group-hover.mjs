import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
const ast = ts.createSourceFile("original-client-shell.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function declaration(name, tree = ast) {
  const found = [];
  function visit(node) { if (ts.isFunctionDeclaration(node) && node.name?.text === name) found.push(node); ts.forEachChild(node, visit); }
  visit(tree); assert.equal(found.length, 1, `sole actual source function ${name}`); return found[0].getText(tree).replace(/^export\s+/, "");
}
function functions(tree, names, scope) {
  const output = ts.transpileModule(names.map(name => declaration(name, tree)).join("\n"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
  }).outputText;
  return new Function("scope", `with(scope){${output};return {${names.join(",")}};}`)(scope);
}
function pureFunctions(relative, names, scope = {}) {
  const text = readFileSync(new URL(relative, import.meta.url), "utf8");
  return functions(ts.createSourceFile(relative, text, ts.ScriptTarget.Latest, true), names, scope);
}
const { containsHudRect } = pureFunctions("../lib/bevy-hud-ui.ts", ["validRect", "containsHudRect"]);
const { questWorldControlAt } = pureFunctions("../lib/bevy-quest-world-controls.ts", ["questWorldControlAt"]);

// Small deterministic DOM surfaces. Geometry and the handlers themselves are the
// production functions; no React app, runtime, socket, credentials or service is initialized.
class Element {
  constructor({ id = "", classes = [], data = {}, parent = null, tag = "DIV" } = {}) {
    Object.assign(this, { id, classes: new Set(classes), dataset: data, parent, tagName: tag,
      isConnected: true, isContentEditable: false });
    this.classList = { contains: name => this.classes.has(name) };
  }
  matches(selector) {
    const tag = selector.match(/^[a-z]+/i)?.[0];
    if (tag && tag.toUpperCase() !== this.tagName) return false;
    if ([...selector.matchAll(/\.([\w-]+)/g)].some(([, name]) => !this.classes.has(name))) return false;
    for (const [, name, value] of selector.matchAll(/\[([\w-]+)(?:=['"]([^'"]+)['"])?\]/g)) {
      const actual = name.startsWith("data-") ? this.dataset[name.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase())]
        : name === "contenteditable" ? String(this.isContentEditable) : this[name];
      if (actual === undefined || value !== undefined && actual !== value) return false;
    }
    return true;
  }
  closest(selector) { for (let e = this; e; e = e.parent) if (selector.split(/,\s*/).some(s => e.matches(s))) return e; return null; }
  contains(other) { for (let e = other; e; e = e.parent) if (e === this) return true; return false; }
  getBoundingClientRect() { return this.rect ?? { left: 0, top: 0, right: 1024, bottom: 768, width: 1024, height: 768 }; }
}
class Input extends Element {}
class TextArea extends Element {}
class Select extends Element {}
const { guildKeyboardOwnsEvent } = pureFunctions("../lib/r16-guild-keyboard.ts", ["guildKeyboardOwnsEvent"], { Element });
const { pickCrystalActor } = pureFunctions("../lib/crystal-actor-hit.ts", ["pickCrystalActor"], {});
const rect = (left = 150, top = 100, width = 100, height = 100) => ({ left, top, width, height });
function fixture() {
  const pointCalls = [], pageCalls = [], listeners = new Map(), contexts = {}, flags = {}, queryCalls = [];
  const frame = new Element({ data: { viewportSceneWidth: "1024", viewportSceneHeight: "768" } });
  const canvas = new Element({ id: "shared-ui", parent: frame });
  const peer = new Element({ classes: ["entity-sprite-stack"], data: { objectId: "peer", uiInteractive: "true" }, parent: frame });
  peer.querySelector = () => null;
  const hit = new Element({ classes: ["entity-sprite-hit"], parent: peer });
  const body = { path: "/peer.png", x: 150, y: 100, width: 100, height: 100 };
  const actor = { get objectId() { return peer.dataset.objectId; }, kind: "player", x: 5, y: 5 };
  let layers = [canvas, hit, peer, frame], functionsForKey = ["AddGroupMember"];
  const add = (target, name, fn, options) => { const key = target + name; assert(!listeners.has(key), `single ${key} listener`); listeners.set(key, { fn, options }); };
  const remove = (target, name, fn) => { const key = target + name; assert.equal(listeners.get(key)?.fn, fn); listeners.delete(key); };
  const noAction = () => { throw Error("hover admission must never call an input edge/action"); };
  const router = () => ({ held: null, down: noAction, edge: noAction, cancel: noAction });
  const hud = { ready: true, characterStatsReady: false, modal: false, foregroundRects: [], plan: {
    main: rect(0, 700, 1024, 68), orb: rect(0, 600, 120, 100), experienceBar: rect(120, 690, 500, 10),
    weightBar: rect(620, 690, 100, 10), characterRect: rect(500, 0, 400, 500),
  } };
  const scope = {
    Element, HTMLElement: Element, HTMLInputElement: Input, HTMLTextAreaElement: TextArea, HTMLSelectElement: Select,
    window: { addEventListener: (...a) => add("window:", ...a), removeEventListener: (...a) => remove("window:", ...a) },
    document: { visibilityState: "visible", hasFocus: () => !flags.blur,
      elementFromPoint: (x, y) => { queryCalls.push([x, y, "top"]); return layers[0] ?? null; },
      elementsFromPoint: (x, y) => { queryCalls.push([x, y, "underlay"]); return layers; },
      addEventListener: (...a) => add("document:", ...a), removeEventListener: (...a) => remove("document:", ...a) },
    screen: "game", world: { mapFileName: "0", entities: [actor] }, player: { objectId: "self" }, bevyMapRuntimeGeneration: 7,
    viewportActorBodies: { get: id => id === actor.objectId ? body : undefined },
    pickCrystalActor, sampleCrystalBodyPixel: () => true, hideDomEntitySpritesForBevy: false,
    stageFrameRef: { current: frame }, stagePresentation: { virtualWidth: 1024, virtualHeight: 768 },
    sceneGroupHoverScopeRef: { current: null }, sceneGroupHoverPointRef: { current: null }, sceneGroupHoverCallbacksRef: { current: null },
    sceneHoveredObjectIdRef: { current: null }, setSceneHoveredObjectId: () => {},
    onCombatPointer: (cursor, hovered) => pointCalls.push({ cursor, hovered }),
    sceneInteractionReady: true, questLocalModalRef: { current: false }, mobileMoreOpenRef: { current: false },
    bevyQuestUiCapturesPointer: false, bevyMailComposeReady: false, bevyMailComposePending: false,
    bevyStorageUiActive: false, bevyStorageUiTransitioning: false, bevyNpcShopUiActive: false, bevyNpcShopUiTransitioning: false,
    bevyBagUiActive: false, bevyCharacterPageReady: false, bevySpellsPageReady: false, bevyMailPageReady: false,
    bevyHeroUiTransitioning: false, bevyHudUiReady: true, bevyHeroUiActive: false, bevyHeroUiWorldBlocked: false,
    npcShopBlocksWorldInput: () => !!flags.shopBlocked, heroBlocksWorldInput: () => !!flags.heroBlocked,
    stopNpcShopWorldInput: noAction, sharedUiCanvasId: () => "shared-ui", webGl2SharedCanvasPrototype: true,
    tileFromScenePoint: (x, y) => flags.noTile ? null : { x: Math.floor(x / 48), y: Math.floor(y / 32) },
    readBevyHudStatus: () => hud, readBevyQuestWorldControlBlockers: () => contexts.quest ?? null,
    readBevyQuestWorldControls: () => null, containsHudRect, questWorldControlAt,
    bagPointerCallbacksRef: { current: { getBevyBagPointerContext: () => contexts.bag ?? null } },
    characterPointerCallbacksRef: { current: { getBevyCharacterPointerContext: () => contexts.character ?? null } },
    spellsPointerCallbacksRef: { current: { getBevySpellsPointerContext: () => contexts.spells ?? null } },
    mailPointerCallbacksRef: { current: { getBevyMailPointerContext: () => contexts.mail ?? null } },
    storagePointerCallbacksRef: { current: { getBevyStoragePointerContext: () => contexts.storage ?? null } },
    npcShopPointerCallbacksRef: { current: { getBevyNpcShopPointerContext: () => contexts.npc ?? null } },
    heroPointerCallbacksRef: { current: { getBevyHeroPointerContext: () => contexts.hero ?? null, onBevyHeroKey: () => false } },
    ...Object.fromEntries(["bag", "character", "spells", "hud", "mail", "storage", "npcShop"].map(name => [name + "PointerRouterRef", { current: router() }])),
    heroPointerLeaseRef: { current: null }, heldQuestControlPointersRef: { current: new Set() }, heldComposePointerRef: { current: null },
    bagBeltPointerRef: { current: null }, bagBeltArmedSharedRef: { current: null }, npcRepairPointerRef: { current: null }, combatUiHoldRef: { current: new Map() },
    isSharedBagCompatibilityMouse: () => true, heldScenePointerRef: { current: null },
    getKeybindCaptureActive: () => false, crystalKeyBindings: [{}], combatTildeRef: { current: false },
    crystalKeyNameFromBrowserEvent: () => "G", matchingCrystalKeyFunctions: () => functionsForKey,
    crystalWindowShortcut: () => null, crystalSkillShortcutSlot: () => null, crystalCombatModeStopsDispatch: () => false,
    keyboardMovementDiagnosticsRef: { current: {} }, onCombatKey: () => { flags.combatCalls = (flags.combatCalls ?? 0) + 1; return { handled: false }; },
    onCrystalGameplayShortcut: (id, repeat) => { pageCalls.push({ id, repeat, hovered: scope.sceneHoveredObjectIdRef.current }); return true; },
    onCrystalWindowShortcut: noAction, isCurrentMailTextTarget: () => false, guildKeyboardOwnsEvent,
  };
  const api = functions(ast, ["sceneGroupHoverScope", "retireSceneGroupHoverPoint", "commitSceneGroupHoverScope",
    "rememberSceneGroupHoverPoint", "installSceneGroupHoverObserver", "sceneGroupHoverWorldAllowed", "refreshSceneGroupHover",
    "sceneWorldObjectTarget", "scenePointFromMouseEvent", "updateSceneCombatPointer", "handleScenePointerMove",
    "handleShortcutKey", "keyboardInputTargetIsEditable"], scope);
  scope.sceneGroupHoverCallbacksRef.current = { remember: api.rememberSceneGroupHoverPoint, retire: api.retireSceneGroupHoverPoint };
  api.commitSceneGroupHoverScope(); pointCalls.length = 0;
  const mouse = (x = 200, y = 150) => ({ target: layers[0], currentTarget: frame, clientX: x, clientY: y, preventDefault() {} });
  const key = (repeat = false, target = frame) => ({ key: "g", code: "KeyG", ctrlKey: true, altKey: false, shiftKey: false, metaKey: false,
    repeat, target, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } });
  return { scope, api, contexts, flags, hud, frame, canvas, peer, hit, pointCalls, pageCalls, queryCalls, listeners, mouse, key,
    layers: next => { layers = next; }, functionsForKey: next => { functionsForKey = next; } };
}

test("actual scene move resolves the entity underneath the shared canvas", () => {
  const f = fixture(); f.api.handleScenePointerMove(f.mouse());
  assert.equal(f.pointCalls.at(-1)?.hovered, "peer");
  assert.deepEqual(f.pointCalls.at(-1)?.cursor, [4, 4]);
});
test("ordinary DOM entity hover is synchronized and generic NPC hover stays distinct from a selected player", () => {
  const f = fixture(); f.layers([f.hit, f.peer, f.frame]); f.api.handleScenePointerMove(f.mouse());
  assert.equal(f.pointCalls.at(-1)?.hovered, "peer");
  f.peer.dataset.objectId = "npc"; f.layers([f.canvas, f.hit, f.peer, f.frame]); f.api.handleScenePointerMove(f.mouse());
  assert.equal(f.pointCalls.at(-1)?.hovered, "npc", "Page owns the final live player-kind/identity validation");
  assert.equal(f.pageCalls.length, 0, "hover never dispatches the shortcut");
});
test("actual key handler refreshes the stationary pointer against the current DOM before entering Page", () => {
  const f = fixture(); f.api.handleScenePointerMove(f.mouse()); f.peer.dataset.objectId = "new-peer";
  f.api.handleShortcutKey(f.key());
  assert.deepEqual(f.pageCalls, [{ id: "AddGroupMember", repeat: false, hovered: "new-peer" }]);
});
test("a missing physical point or an empty world never borrows an earlier hover or selection", () => {
  const f = fixture(); f.scope.sceneHoveredObjectIdRef.current = "previous";
  f.api.handleShortcutKey(f.key()); assert.equal(f.pageCalls.length, 0); assert.equal(f.scope.sceneHoveredObjectIdRef.current, null);
  f.layers([f.canvas, f.frame]); f.api.handleScenePointerMove(f.mouse()); f.api.handleShortcutKey(f.key());
  assert.deepEqual(f.pageCalls, [{ id: "AddGroupMember", repeat: false, hovered: null }]);
});
for (const surface of ["bag", "character", "spells", "mail", "storage", "npc", "hero"]) {
  test(`current ${surface} regions block hover and nonmodal world outside remains usable`, () => {
    const f = fixture(); f.contexts[surface] = { inputRegions: [rect()] };
    f.api.handleScenePointerMove(f.mouse()); assert.equal(f.pointCalls.at(-1)?.hovered, null);
    assert(!f.queryCalls.some(x => x[2] === "underlay"), "blocked geometry never queries canvas underlay");
    f.contexts[surface].inputRegions = [rect(600, 100)];
    if (surface === "storage") f.scope.bevyStorageUiActive = true;
    f.api.handleShortcutKey(f.key()); assert.equal(f.pageCalls.at(-1)?.hovered, "peer");
  });
}
test("HUD surfaces, native foreground windows and scaled Quest controls block the underlay", () => {
  for (const setup of [f => { f.hud.plan.main = rect(); }, f => { f.hud.plan.orb = rect(); },
    f => { f.hud.plan.experienceBar = rect(); }, f => { f.hud.plan.weightBar = rect(); },
    f => { f.hud.characterStatsReady = true; f.hud.plan.characterRect = rect(); },
    f => { f.hud.foregroundRects = [rect()]; },
    f => { f.contexts.quest = { logicalWidth: 512, logicalHeight: 384, rects: [rect(75, 50, 50, 50)] }; }]) {
    const f = fixture(); setup(f); f.api.handleScenePointerMove(f.mouse()); f.api.handleShortcutKey(f.key());
    assert.equal(f.pointCalls.at(-1)?.hovered, null); assert.equal(f.pageCalls.length, 0);
    assert(!f.queryCalls.some(x => x[2] === "underlay"));
  }
});
test("all modal states and pending surfaces reject even outside their small regions", () => {
  for (const setup of [f => { f.contexts.spells = { modal: true, inputRegions: [] }; },
    f => { f.contexts.mail = { modal: true, inputRegions: [] }; }, f => { f.hud.modal = true; },
    f => { f.scope.questLocalModalRef.current = true; }, f => { f.scope.mobileMoreOpenRef.current = true; },
    f => { f.scope.bevyQuestUiCapturesPointer = true; }, f => { f.scope.bevyMailComposeReady = true; },
    f => { f.scope.bevyMailComposePending = true; }, f => { f.scope.bevyStorageUiTransitioning = true; },
    f => { f.scope.bevyNpcShopUiTransitioning = true; }, f => { f.flags.shopBlocked = true; },
    f => { f.flags.heroBlocked = true; }, f => { f.scope.sceneInteractionReady = false; }]) {
    const f = fixture(); f.api.rememberSceneGroupHoverPoint(f.mouse()); setup(f);
    assert.equal(f.api.refreshSceneGroupHover(), false); assert.equal(f.pointCalls.at(-1)?.hovered, null);
  }
});
test("active shared surfaces without current geometry fail closed instead of querying the world underlay", () => {
  for (const flag of ["bevyBagUiActive", "bevyCharacterPageReady", "bevySpellsPageReady", "bevyMailPageReady",
    "bevyStorageUiActive", "bevyNpcShopUiActive", "bevyHeroUiActive", "bevyHeroUiTransitioning"]) {
    const f = fixture(); f.scope[flag] = true; f.api.handleScenePointerMove(f.mouse());
    assert.equal(f.scope.sceneHoveredObjectIdRef.current, null); assert(!f.queryCalls.some(x => x[2] === "underlay"));
  }
  const f = fixture(); f.hud.ready = false; f.api.handleScenePointerMove(f.mouse());
  assert.equal(f.scope.sceneHoveredObjectIdRef.current, null);
});
test("held surface leases never become hover probes or new world input", () => {
  for (const name of ["bag", "character", "spells", "hud", "mail", "storage", "npcShop"]) {
    const f = fixture(); f.scope[name + "PointerRouterRef"].current.held = {};
    f.api.handleScenePointerMove(f.mouse()); assert.equal(f.pointCalls.at(-1)?.hovered, null);
  }
  for (const name of ["heroPointerLeaseRef", "heldComposePointerRef", "bagBeltPointerRef", "bagBeltArmedSharedRef", "npcRepairPointerRef"]) {
    const f = fixture(); f.scope[name].current = {}; f.api.handleScenePointerMove(f.mouse()); assert.equal(f.pointCalls.at(-1)?.hovered, null);
  }
  for (const name of ["heldQuestControlPointersRef", "combatUiHoldRef"]) {
    const f = fixture(); f.scope[name].current = new Set([1]); f.api.handleScenePointerMove(f.mouse()); assert.equal(f.pointCalls.at(-1)?.hovered, null);
  }
});
test("a foreground window appearing under a stationary pointer removes the previous target at key time", () => {
  for (const canvasRegion of [false, true]) {
    const f = fixture(); f.api.handleScenePointerMove(f.mouse()); assert.equal(f.scope.sceneHoveredObjectIdRef.current, "peer");
    if (canvasRegion) f.hud.foregroundRects = [rect()];
    else f.layers([new Element({ classes: ["game-ui-scene"], parent: f.frame }), f.canvas, f.hit, f.peer, f.frame]);
    f.api.handleShortcutKey(f.key()); assert.equal(f.pageCalls.length, 0); assert.equal(f.scope.sceneHoveredObjectIdRef.current, null);
  }
});
test("passive capture observes moves swallowed by ExtraWindows and never reuses the old position", () => {
  const f = fixture(), cleanup = f.api.installSceneGroupHoverObserver();
  assert.deepEqual(f.listeners.get("window:pointermove").options, { capture: true, passive: true });
  f.api.handleScenePointerMove(f.mouse());
  const outside = new Element(); f.layers([outside]);
  f.listeners.get("window:pointermove").fn({ clientX: 400, clientY: 300, pointerType: "mouse" });
  f.api.handleShortcutKey(f.key()); assert.equal(f.pageCalls.length, 0); assert.equal(f.scope.sceneHoveredObjectIdRef.current, null);
  assert(f.queryCalls.some(([x, y]) => x === 400 && y === 300));
  cleanup(); assert.equal(f.listeners.size, 0); assert.equal(f.scope.sceneGroupHoverPointRef.current, null);
});
test("blur, hidden document, leaving browser and leaving frame retire the physical point", () => {
  for (const edge of ["blur", "visibility", "pointerout", "frame"]) {
    const f = fixture(), cleanup = f.api.installSceneGroupHoverObserver(); f.api.handleScenePointerMove(f.mouse());
    if (edge === "blur") f.listeners.get("window:blur").fn();
    if (edge === "visibility") { f.scope.document.visibilityState = "hidden"; f.listeners.get("document:visibilitychange").fn(); f.scope.document.visibilityState = "visible"; }
    if (edge === "pointerout") f.listeners.get("window:pointerout").fn({ relatedTarget: null });
    if (edge === "frame") f.api.retireSceneGroupHoverPoint();
    assert.equal(f.scope.sceneGroupHoverPointRef.current, null); f.api.handleShortcutKey(f.key()); assert.equal(f.pageCalls.length, 0); cleanup();
  }
  assert.match(source, /onMouseLeave=\{retireSceneGroupHoverPoint\}/);
});
test("map, player, screen and runtime generation changes retire previous coordinates", () => {
  for (const update of [f => { f.scope.world.mapFileName = "D001"; }, f => { f.scope.player = { objectId: "other" }; },
    f => { f.scope.screen = "select"; }, f => { f.scope.bevyMapRuntimeGeneration++; }]) {
    const f = fixture(); f.api.handleScenePointerMove(f.mouse()); update(f);
    assert.equal(f.api.refreshSceneGroupHover(), false, "even before a layout commit the previous scope is rejected");
    f.api.commitSceneGroupHoverScope(); assert.equal(f.scope.sceneGroupHoverPointRef.current, null);
  }
});
test("unchanged render scope retains the actual point, while invalid coordinates, touch and detached frame retire it", () => {
  const f = fixture(); f.api.handleScenePointerMove(f.mouse()); f.api.commitSceneGroupHoverScope(); assert(f.api.refreshSceneGroupHover());
  for (const event of [{ clientX: NaN, clientY: 100 }, { clientX: 1100, clientY: 100 }, { clientX: 200, clientY: 100, pointerType: "touch" }]) {
    f.api.rememberSceneGroupHoverPoint(event); assert.equal(f.scope.sceneGroupHoverPointRef.current, null);
  }
  f.frame.isConnected = false; f.api.rememberSceneGroupHoverPoint(f.mouse()); assert.equal(f.scope.sceneGroupHoverPointRef.current, null);
});
test("scaled DOM bounds use current geometry rather than a cached virtual position", () => {
  const f = fixture(); f.frame.rect = { left: 0, top: 0, right: 512, bottom: 384, width: 512, height: 384 };
  f.api.handleScenePointerMove(f.mouse(100, 75)); assert.equal(f.scope.sceneHoveredObjectIdRef.current, "peer");
  f.contexts.bag = { inputRegions: [rect()] }; f.api.handleShortcutKey(f.key()); assert.equal(f.pageCalls.length, 0);
});
test("invalid stage geometry cannot create or reuse a hover point", () => {
  for (const bad of [{ width: 0, right: 0 }, { height: NaN }, { left: NaN }]) {
    const f = fixture(); f.api.handleScenePointerMove(f.mouse());
    f.frame.rect = { ...f.frame.getBoundingClientRect(), ...bad };
    assert.equal(f.api.refreshSceneGroupHover(), false); assert.equal(f.scope.sceneHoveredObjectIdRef.current, null);
    f.api.rememberSceneGroupHoverPoint(f.mouse()); assert.equal(f.scope.sceneGroupHoverPointRef.current, null);
  }
});
test("repeat and editable keys cannot invoke Page, and storage's Group exception never dispatches other actions", () => {
  const f = fixture(); f.api.handleScenePointerMove(f.mouse()); const count = f.pointCalls.length;
  f.api.handleShortcutKey(f.key(true)); assert.equal(f.pageCalls.length, 0); assert.equal(f.pointCalls.length, count);
  f.api.handleShortcutKey(f.key(false, new Input())); assert.equal(f.pageCalls.length, 0);
  f.contexts.storage = { inputRegions: [rect(600, 100)] }; f.scope.bevyStorageUiActive = true;
  f.functionsForKey(["AddGroupMember", "Trade"]); f.flags.combatCalls = 0; f.api.handleShortcutKey(f.key());
  assert.deepEqual(f.pageCalls.map(x => x.id), ["AddGroupMember"]); assert.equal(f.flags.combatCalls, 0);
});
test("a shared world pointer may recheck its own HUD lease against live geometry", () => {
  const f = fixture(), router = f.scope.hudPointerRouterRef.current;
  const owner = { pointerId: 1, origin: "world" };
  router.held = owner;
  router.matches = status => status === f.hud;
  assert.equal(f.api.sceneGroupHoverWorldAllowed(200, 150), false, "ordinary hover remains blocked");
  assert.equal(f.api.sceneGroupHoverWorldAllowed(200, 150, { ...owner }), false, "only the same lease is admitted");
  assert.equal(f.api.sceneGroupHoverWorldAllowed(200, 150, owner), true);
  f.hud.foregroundRects = [rect()];
  assert.equal(f.api.sceneGroupHoverWorldAllowed(200, 150, owner), false, "new foreground blocks the world");
  f.hud.foregroundRects = [];
  router.matches = () => false;
  assert.equal(f.api.sceneGroupHoverWorldAllowed(200, 150, owner), false, "a retired HUD lease is rejected");
});
test("the component retains stable keyboard hookup and installs the read-only observer with cleanup", () => {
  assert.match(source, /useWindowKeydown\(screen === "game" \? handleShortcutKey : null\)/);
  assert.match(source, /useEffect\(installSceneGroupHoverObserver, \[\]\)/);
  const body = declaration("sceneGroupHoverWorldAllowed");
  assert(!/\.down\(|\.edge\(|\.cancel\(|onBevy\w+Pointer\(/.test(body));
});
