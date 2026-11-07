import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const file = ts.createSourceFile("shell.tsx", readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8"),
  ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
function nodes(root, predicate) {
  const found = [];
  const visit = node => { if (predicate(node)) found.push(node); ts.forEachChild(node, visit); };
  visit(root); return found;
}
function compile(text, scope, result) {
  const code = ts.transpileModule(text, { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), `${code}\n${result}`)(...Object.values(scope));
}
function load(relative) {
  const module = { exports: {} }, text = readFileSync(new URL(relative, import.meta.url), "utf8");
  const code = ts.transpileModule(text, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  new Function("exports", "module", "require", code)(module.exports, module, name => { throw Error(`Unexpected dependency ${name}`); });
  return module.exports;
}
const hud = load("../lib/bevy-hud-ui.ts"), { sharedUiCanvasId } = load("../lib/bevy-shared-canvas-mode.ts");
const names = ["sceneWorldObjectTarget", "retainSceneWorldObjectTarget", "scenePointFromMouseEvent", "committedViewportPlayerPosition",
  "tileFromScenePoint", "handleScenePointerAction", "stopHeldScenePointer", "dispatchSceneClickInput", "dispatchSceneMoveInput",
  "isSharedBagCompatibilityMouse", "npcShopBlocksWorldInput", "stopNpcShopWorldInput", "heroBlocksWorldInput",
  "handleSharedUiPointer", "handleSharedBagPointer", "handleSharedHudPointer", "cancelSharedHudPointer", "handleSharedQuestWorldPointer"];
const declarations = nodes(file, node => ts.isFunctionDeclaration(node) && names.includes(node.name?.text));
assert.equal(declarations.length, names.length);
const stage = nodes(file, node => (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node))
  && node.attributes.properties.some(attribute => ts.isJsxAttribute(attribute) && attribute.name.getText(file) === "data-viewport-player-x"));
assert.equal(stage.length, 1, "use the actual committed game-stage event wiring");
function stageBinding(name) {
  const attribute = stage[0].attributes.properties.find(node => ts.isJsxAttribute(node) && node.name.getText(file) === name);
  assert(attribute && ts.isJsxExpression(attribute.initializer) && attribute.initializer.expression);
  return attribute.initializer.expression.getText(file);
}
const effects = nodes(file, node => ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === "useEffect"
  && node.arguments[0] && node.arguments[0].getText(file).includes("if (held.worldTarget) return;"));
assert.equal(effects.length, 1, "retain the actual world hold/blur lifecycle");
const terminal = nodes(file, node => ts.isVariableDeclaration(node) && node.name.getText(file) === "up" && node.initializer
  && node.initializer.getText(file).includes('sharedBagPointerHandlerRef.current(event, "up")'));
assert.equal(terminal.length, 1, "use the actual window pointerup capture forwarder");

class ClassList extends Set { contains(value) { return this.has(value); } }
class Element {
  constructor({ id = "", classes = [], dataset = {}, parent = null } = {}) {
    this.id = id; this.classList = new ClassList(classes); this.dataset = { ...dataset }; this.parent = parent;
    this.tagName = id.includes("canvas") ? "CANVAS" : "DIV"; this.focuses = 0;
  }
  contains(element) { for (let current = element; current; current = current.parent) if (current === this) return true; return false; }
  matches(selector) {
    if ([...selector.matchAll(/\.([\w-]+)/g)].some(([, name]) => !this.classList.has(name))) return false;
    if (selector.includes("[data-ui-interactive='true']") && this.dataset.uiInteractive !== "true") return false;
    if (selector.includes("[data-object-id]") && !this.dataset.objectId) return false;
    return selector.startsWith(".") || selector.startsWith("[");
  }
  closest(selector) {
    const options = selector.split(",").map(value => value.trim());
    for (let current = this; current; current = current.parent) if (options.some(option => current.matches(option))) return current;
    return null;
  }
  getAttribute(name) { return name === "data-object-id" ? this.dataset.objectId ?? null : null; }
  focus() { this.focuses++; }
  setPointerCapture() {}
  getBoundingClientRect() { return { left: 30, top: 20, width: 512, height: 384 }; }
}
function fixture({ native = true, oldMouseHold = false, marker = "drop", sceneReady = true, layers: initialLayers } = {}) {
  let now = 1000, serial = 0, pickupAllowed = true;
  const listeners = new Map(), timers = new Map(), picks = [], activations = [], clicks = [], moves = [];
  const frame = new Element({ classes: ["client-stage-frame"], dataset: { viewportPlayerX: "299", viewportPlayerY: "618",
    viewportSceneWidth: "1024", viewportSceneHeight: "768" } });
  const canvas = new Element({ id: sharedUiCanvasId(false), parent: frame });
  const worldTile = { x: 299, y: 619 }, labelPoint = { x: 556, y: 372 };
  const markerNode = new Element({ classes: [marker === "drop" ? "ground-drop-marker" : "entity-nameplate"],
    dataset: { objectId: marker === "drop" ? "gold-drop-7" : "npc-15", uiInteractive: "true" }, parent: frame });
  const label = new Element({ classes: ["drop-label"], parent: markerNode });
  let layers = initialLayers ?? [canvas, label, markerNode, frame];
  const held = { current: null }, idleRouter = () => ({ held: null, edge: () => null, cancel: () => null, matches: () => false });
  const status = { version: 1, generation: 7, revision: 1, frame: 1, ready: true, characterStatsReady: false,
    navigation: { characterOpen: false, characterPage: "character", bagOpen: false, questOpen: false }, modal: false, foregroundRects: [], error: null,
    plan: { main: { left: 0, top: 616, width: 1024, height: 152 }, orb: { left: 0, top: 646, width: 104, height: 80 },
      experienceBar: { left: 9, top: 759, width: 1004, height: 8 }, weightBar: { left: 919, top: 719, width: 76, height: 12 },
      characterRect: { left: 760, top: 0, width: 264, height: 380 }, characterHits: [],
      buttons: ["character", "bag", "skill", "quest", "option", "menu", "gameShop"].map((action, i) => ({ action,
        rect: { left: 905 + i * 23, top: 692, width: 20, height: 20 } })) } };
  const flags = { parity: false, shop: false };
  const window = {
    addEventListener(type, fn) { const group = listeners.get(type) ?? new Set(); group.add(fn); listeners.set(type, group); },
    removeEventListener(type, fn) { listeners.get(type)?.delete(fn); },
    setInterval(fn, ms) { const id = ++serial; timers.set(id, { fn, ms, at: now + ms }); return id; },
    clearInterval(id) { timers.delete(id); },
    emit(type) { for (const fn of [...(listeners.get(type) ?? [])]) fn({ type, timeStamp: now }); }
  };
  const scope = {
    HTMLElement: Element, Element, window, Date: { now: () => now },
    document: { elementsFromPoint: () => layers, visibilityState: "visible", hasFocus: () => true },
    sharedUiCanvasId, webGl2SharedCanvasPrototype: false, stageFrameRef: { current: frame },
    stagePresentation: { virtualWidth: 1024, virtualHeight: 768 }, viewportLayout: { offsetX: 10, offsetY: 11 },
    VIEWPORT_CELL_WIDTH: 48, VIEWPORT_CELL_HEIGHT: 32, CRYSTAL_MOVE_INPUT_INTERVAL_MS: 100,
    heldScenePointerRef: held, scenePointerDiagnosticsRef: { current: null }, latestMoveInputRef: { current: { screen: "game", player: { x: 299, y: 618 } } },
    screen: "game", player: { x: 299, y: 618 }, sceneInteractionReady: sceneReady, bevyQuestUiCapturesPointer: false,
    bevyMailComposeReady: false, bevyMailComposePending: false, bevyBagUiActive: false, bevyHudUiReady: native,
    questLocalModalOpen: false, mobileMoreOpen: false, playerUiPreferences: undefined,
    parityUiBlocksGameplay: () => flags.parity, onCombatUiHeld: undefined, combatUiHoldRef: { current: new Map() },
    heroPointerLeaseRef: { current: null }, heroPointerCallbacksRef: { current: { getBevyHeroInputBlocked: () => false } },
    npcShopPointerCallbacksRef: { current: { getBevyNpcShopInputBlocked: () => flags.shop } },
    npcShopPointerRouterRef: { current: idleRouter() }, storagePointerRouterRef: { current: idleRouter() },
    mailPointerRouterRef: { current: idleRouter() }, characterPointerRouterRef: { current: idleRouter() },
    bagPointerRouterRef: { current: idleRouter() }, hudPointerRouterRef: { current: new hud.HudPointerRouter() },
    characterPointerCallbacksRef: { current: { getBevyCharacterPointerContext: () => null } },
    spellsPointerCallbacksRef: { current: { getBevySpellsPointerContext: () => null } },
    bagPointerCallbacksRef: { current: { getBevyBagPointerContext: () => null } },
    heldQuestControlPointersRef: { current: new Set() }, bagBeltArmedSharedRef: { current: null }, bagBeltRejectedTerminalRef: { current: null },
    readBevyHudStatus: () => hud.readHudStatus({ getMir2HudUiStatus: () => JSON.stringify(status) }, status.generation),
    readBevyQuestWorldControlBlockers: () => null, readBevyQuestWorldControls: () => null,
    heldKeyboardMoveKeysRef: { current: new Set() }, heldKeyboardRunModeRef: { current: false },
    worldFishingPhysicalRef: { current: null }, worldFishingPointersRef: { current: new Map() },
    sharedBagPointerHandlerRef: { current: null },
    onPickGroundDrop: id => { picks.push(id); return pickupAllowed; }, onActivateEntity: id => activations.push(id),
    onViewportTileClick: (x, y) => clicks.push({ x, y, mode: "walk" }),
    onViewportTileSecondaryAction: (x, y) => clicks.push({ x, y, mode: "run" }),
    onViewportDirectionStep: (x, y, mode) => moves.push({ x, y, mode }), onViewportDirectionStop() {}, updateSceneCombatPointer() {},
    // Independent surfaces and fishing have no admitted lease in this fixture.
    // Their real arbitration entry points still execute before the real HUD owner.
    handleSharedHeroPointer: () => false, handleNpcRepairPointer: () => false, handleBagBeltPointer: () => false,
    handleSharedNpcShopPointer: () => false, handleSharedComposePointer: () => false, handleSharedMailPointer: () => false,
    handleSharedStoragePointer: () => false, handleSharedSpellsPointer: () => false, handleSharedCharacterPointer: () => false,
    cancelBagBeltPointer() {}, cancelSharedBagPointer() {}, cancelWorldFishingHeldPointer() {},
    attachWorldFishingPhysical() {}, finishWorldFishingPhysical() {}, handleSceneWorldPointerDown() {}
  };
  let removed = 0;
  const printer = ts.createPrinter();
  const source = declarations.map(node => {
    if (!oldMouseHold || node.name.text !== "handleScenePointerAction") return node.getText(file);
    const result = ts.transform(node, [context => root => {
      const visit = current => {
        if (ts.isObjectLiteralExpression(current) && current.properties.some(property => ts.isPropertyAssignment(property) && property.name.getText(file) === "worldTarget")) {
          removed++;
          return ts.factory.updateObjectLiteralExpression(current, current.properties.filter(property => !ts.isPropertyAssignment(property) || property.name.getText(file) !== "worldTarget"));
        }
        return ts.visitEachChild(current, visit, context);
      };
      return ts.visitNode(root, visit);
    }]);
    const text = printer.printNode(ts.EmitHint.Unspecified, result.transformed[0], file); result.dispose(); return text;
  }).join("\n");
  if (oldMouseHold) assert.equal(removed, 1, "the negative control removes only the new mouse-hold target property");
  const bindings = `const stageDown = ${stageBinding("onPointerDownCapture")};
    const stageMouseDownCapture = ${stageBinding("onMouseDownCapture")};
    const stageMouseDown = ${stageBinding("onMouseDown")};
    const stageMouseUp = ${stageBinding("onMouseUp")};
    const capturedPointerUp = ${terminal[0].initializer.getText(file)};
    const mountWorldHold = ${effects[0].arguments[0].getText(file)};`;
  const api = compile(`${source}\n${bindings}`, scope,
    `return { ${names.join(",")}, stageDown, stageMouseDownCapture, stageMouseDown, stageMouseUp, capturedPointerUp, mountWorldHold };`);
  scope.sharedBagPointerHandlerRef.current = api.handleSharedUiPointer;
  const cleanup = api.mountWorldHold();
  function event(target = canvas, changes = {}) {
    const prototype = { get clientX() { return 30 + labelPoint.x / 2; }, get clientY() { return 20 + labelPoint.y / 2; } };
    return Object.assign(Object.create(prototype), { target, currentTarget: frame, pointerId: 1, pointerType: "mouse", button: 0,
      isPrimary: true, timeStamp: now, preventDefault() { this.defaultPrevented = true; }, ...changes });
  }
  return { api, scope, flags, frame, canvas, markerNode, label, held, status, picks, activations, clicks, moves, worldTile, labelPoint, event,
    set layers(value) { layers = value; }, set pickupAllowed(value) { pickupAllowed = value; },
    down(target = native ? canvas : frame) { const e = event(target); api.stageDown(e); return e; },
    mouseDown(target = native ? canvas : frame) { const e = event(target); api.stageMouseDownCapture(e); api.stageMouseDown(e); return e; },
    up(target = native ? canvas : frame) { const e = event(target); api.capturedPointerUp(e); api.stageMouseUp(e); window.emit("mouseup"); },
    blur() { window.emit("blur"); },
    advance(ms) {
      const until = now + ms;
      for (;;) { const next = [...timers.values()].filter(timer => timer.at <= until).sort((a, b) => a.at - b.at)[0];
        if (!next) break; now = next.at; next.at += next.ms; next.fn(); }
      now = until;
    }, cleanup() { cleanup?.(); }
  };
}

test("real stage capture retains a Canvas-underlay Gold marker and compatibility mouse/up cannot duplicate it", () => {
  const f = fixture();
  try {
    assert.notDeepEqual(f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y), f.worldTile, "fanned Gold label pixels belong to a different tile");
    f.down(); assert.deepEqual(f.held.current.worldTarget, { kind: "drop", objectId: "gold-drop-7" });
    const admitted = f.held.current; f.mouseDown(); assert.equal(f.held.current, admitted, "same-Canvas compatibility mouse yields to the admitted lease");
    f.mouseDown(f.frame); assert.equal(f.held.current, admitted, "retargeting does not bypass an existing Native world lease either");
    f.advance(250); assert.deepEqual(f.moves, [], "holding an object marker is never a tile movement hold");
    f.up(); f.up(); assert.deepEqual(f.picks, ["gold-drop-7"]); assert.deepEqual(f.activations, []);
    assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []); assert.equal(f.held.current, null);
  } finally { f.cleanup(); }
});

test("an unadmitted second pointer cannot replace the first retained world marker", () => {
  const f = fixture();
  try {
    f.down(); const admitted = f.held.current;
    const other = new Element({ classes: ["ground-drop-marker"], dataset: { objectId: "other-drop" }, parent: f.frame });
    f.layers = [f.canvas, other, f.frame];
    f.api.stageDown(f.event(f.canvas, { pointerId: 2 }));
    assert.equal(f.held.current, admitted); assert.deepEqual(admitted.worldTarget, { kind: "drop", objectId: "gold-drop-7" });
    f.api.capturedPointerUp(f.event(f.canvas, { pointerId: 2 })); assert.deepEqual(f.picks, []);
    f.up(); assert.deepEqual(f.picks, ["gold-drop-7"]); assert.deepEqual(f.clicks, []);
  } finally { f.cleanup(); }
});

test("the actual legacy stage mouse path preserves the visible Gold target instead of the offset tile", () => {
  const f = fixture({ native: false });
  try {
    f.down(); assert.equal(f.held.current, null, "the inactive Native owner did not fabricate a shared lease");
    f.mouseDown(); assert.deepEqual(f.held.current.worldTarget, { kind: "drop", objectId: "gold-drop-7" });
    f.advance(250); f.up(); f.up(); assert.deepEqual(f.picks, ["gold-drop-7"]);
    assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
  } finally { f.cleanup(); }
});

test("removing only the new legacy mouse property in memory reproduces zero pickup and wrong-tile fallback", () => {
  const f = fixture({ native: false, oldMouseHold: true });
  try {
    f.down(); f.mouseDown(); assert.equal(f.held.current.worldTarget, undefined); f.up();
    assert.deepEqual(f.picks, []); assert.equal(f.clicks.length, 1);
    assert.notDeepEqual({ x: f.clicks[0].x, y: f.clicks[0].y }, f.worldTile);
  } finally { f.cleanup(); }
});

test("NPC nameplate and interactive sprite-hit underlays activate exactly once through either real owner path", () => {
  for (const native of [true, false]) for (const sprite of [false, true]) {
    const f = fixture({ native, marker: "entity" });
    try {
      if (sprite) {
        const stack = new Element({ classes: ["entity-sprite-stack"], dataset: { objectId: "npc-sprite-15", uiInteractive: "true" }, parent: f.frame });
        const hit = new Element({ classes: ["entity-sprite-hit"], parent: stack }); f.layers = [f.canvas, hit, stack, f.frame];
      }
      f.down(); f.mouseDown(); f.up(); f.up();
      assert.deepEqual(f.activations, [sprite ? "npc-sprite-15" : "npc-15"]); assert.deepEqual(f.picks, []);
      assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
    } finally { f.cleanup(); }
  }
});

test("foreign frames, non-Canvas top layers, missing markers and noninteractive entity markers never acquire object authority", () => {
  for (const scenario of ["foreign", "noncanvas", "missing", "noninteractive"]) {
    const f = fixture({ native: false });
    try {
      if (scenario === "foreign") {
        const foreign = new Element(), target = new Element({ classes: ["ground-drop-marker"], dataset: { objectId: "foreign-id" }, parent: foreign });
        f.layers = [f.canvas, target, f.frame];
      } else if (scenario === "noncanvas") f.layers = [f.frame, f.label, f.markerNode];
      else if (scenario === "missing") f.layers = [f.canvas, f.frame];
      else { f.markerNode.classList = new ClassList(["entity-nameplate"]); f.markerNode.dataset.uiInteractive = "false"; }
      assert.equal(f.api.sceneWorldObjectTarget(308, 206), undefined, scenario);
      f.down(); f.mouseDown(); f.up(); assert.deepEqual(f.picks, [], scenario); assert.deepEqual(f.activations, [], scenario);
      assert.equal(f.clicks.length, 1, "the ordinary admitted world click keeps its existing tile fallback");
    } finally { f.cleanup(); }
  }
});

test("foreground UI, Native modal/foreground ownership and unavailable scene keep world object callbacks blocked", () => {
  for (const scenario of ["dom-ui", "native-modal", "native-foreground", "scene", "quest", "parity", "shop"]) {
    const f = fixture({ sceneReady: scenario !== "scene" });
    try {
      let target = f.canvas;
      if (scenario === "dom-ui") {
        target = new Element({ classes: ["game-ui-scene"], dataset: { uiInteractive: "true" }, parent: f.frame });
        f.layers = [target, f.canvas, f.label];
      } else if (scenario === "native-modal") f.status.modal = true;
      else if (scenario === "native-foreground") f.status.foregroundRects = [{ left: 500, top: 350, width: 200, height: 100 }];
      else if (scenario === "quest") f.scope.heldQuestControlPointersRef.current.add(1);
      else if (scenario !== "scene") f.flags[scenario] = true;
      f.down(target); f.mouseDown(target); f.up(target);
      assert.deepEqual(f.picks, [], scenario); assert.deepEqual(f.activations, [], scenario); assert.deepEqual(f.clicks, [], scenario);
    } finally { f.cleanup(); }
  }
});

test("focus blur, owner generation change and Page rejection never replay or turn a retained marker into a tile click", () => {
  for (const scenario of ["blur", "owner", "page-rejected"]) {
    const f = fixture();
    try {
      f.down(); f.mouseDown();
      if (scenario === "blur") f.blur();
      else if (scenario === "owner") f.status.generation++;
      else f.pickupAllowed = false;
      f.up(); f.advance(400); f.up();
      assert.deepEqual(f.picks, scenario === "page-rejected" ? ["gold-drop-7"] : [], scenario);
      assert.deepEqual(f.clicks, [], scenario); assert.deepEqual(f.moves, [], scenario);
    } finally { f.cleanup(); }
  }
});

test("the Mini Map frame stops an ordinary route for both buttons without taking image or HUD actions", () => {
  const mapFile = ts.createSourceFile("map-panels.tsx",
    readFileSync(new URL("../app/components/original-client-map-panels.tsx", import.meta.url), "utf8"),
    ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const [frame] = nodes(mapFile, node => ts.isJsxOpeningElement(node) && node.tagName.getText(mapFile) === "section"
    && node.attributes.properties.some(attr => ts.isJsxAttribute(attr) && attr.name.getText(mapFile) === "className"
      && attr.initializer?.getText(mapFile).includes("mini-map-panel")));
  assert(frame, "use the real Mini Map frame");
  const attr = frame.attributes.properties.find(node => ts.isJsxAttribute(node) && node.name.getText(mapFile) === "onMouseDown");
  assert(attr && ts.isJsxExpression(attr.initializer) && attr.initializer.expression, "frame owns mouse down");
  const [consume] = nodes(mapFile, node => ts.isFunctionDeclaration(node) && node.name?.text === "consumeMapImageClick");
  const source = {}, stopped = [];
  const handle = compile(consume.getText(mapFile), {
    Element, routeSource: source, onImageRoutePress: owner => stopped.push(owner),
  }, `return ${attr.initializer.expression.getText(mapFile)};`);
  const press = (target, button) => {
    const event = { target, button, preventDefault() { this.prevented = true; }, stopPropagation() { this.stopped = true; } };
    handle(event);
    return Boolean(event.prevented && event.stopped);
  };
  const header = new Element({ classes: ["mini-map-name"] });
  assert.equal(press(header, 0), true);
  assert.equal(press(header, 2), true);
  assert.equal(press(header, 1), false);
  const raster = new Element({ parent: new Element({ classes: ["mini-map-scene"] }) });
  const button = new Element();
  button.matches = selector => selector === "button";
  for (const action of [raster, new Element({ parent: button })]) {
    assert.equal(press(action, 0), false);
    assert.equal(press(action, 2), false);
  }
  assert.deepEqual(stopped, [source, source]);
});
