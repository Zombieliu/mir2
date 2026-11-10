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
function load(relative, globals = {}) {
  const module = { exports: {} }, text = readFileSync(new URL(relative, import.meta.url), "utf8");
  const code = ts.transpileModule(text, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  new Function("exports", "module", "require", ...Object.keys(globals), code)(module.exports, module,
    name => { throw Error(`Unexpected dependency ${name}`); }, ...Object.values(globals));
  return module.exports;
}
const hud = load("../lib/bevy-hud-ui.ts"), { sharedUiCanvasId } = load("../lib/bevy-shared-canvas-mode.ts");
const names = ["sceneWorldObjectTarget", "retainSceneWorldObjectTarget", "scenePointFromMouseEvent", "committedViewportPlayerPosition",
  "tileFromScenePoint", "handleScenePointerAction", "stopHeldScenePointer", "dispatchSceneClickInput", "dispatchSceneMoveInput",
  "isSharedBagCompatibilityMouse", "npcShopBlocksWorldInput", "stopNpcShopWorldInput", "heroBlocksWorldInput",
  "handleSharedUiPointer", "handleSharedBagPointer", "handleSharedHudPointer", "cancelSharedHudPointer", "handleSharedQuestWorldPointer",
  "handleSharedNpcShopPointer", "cancelSharedNpcShopPointer", "beginCombatUiHold", "endCombatUiHold",
  "entityBodyAtlasFrame", "bevyEntityAtlasRectKey"];
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
    if (selector.includes(",")) return selector.split(",").some(option => this.matches(option.trim()));
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
  querySelector() { return this.bodyImage ?? null; }
  focus() { this.focuses++; }
  setPointerCapture() {}
  getBoundingClientRect() { return { left: 30, top: 20, width: 512, height: 384 }; }
}
function fixture({ native = true, oldMouseHold = false, marker = "drop", sceneReady = true, newMove = false, modalOnHud = false,
  packed = false,
  layers: initialLayers } = {}) {
  let now = 1000, serial = 0, pickupAllowed = true;
  const listeners = new Map(), timers = new Map(), picks = [], activations = [], clicks = [], moves = [], combatHolds = [];
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
  const document = { elementsFromPoint: () => layers, visibilityState: "visible", hasFocus: () => true,
    addEventListener: window.addEventListener, removeEventListener: window.removeEventListener,
    createElement() {
      let image;
      return { getContext: () => ({ drawImage(source) { image = source; }, getImageData: () => ({ data: image.rgba }) }) };
    },
  };
  const { pickCrystalActor, sampleCrystalBodyPixel } = load("../lib/crystal-actor-hit.ts", { document });
  const actors = [], viewportActorBodies = new Map(), activeBevyEntityAtlas = { rects: {}, pages: [] };
  const scope = {
    HTMLElement: Element, Element, window, Date: { now: () => now },
    document, world: { entities: actors }, viewportActorBodies, pickCrystalActor, sampleCrystalBodyPixel,
    hideDomEntitySpritesForBevy: packed, activeBevyEntityAtlas,
    warmCrystalBodyPixels: () => assert.fail("the matching live atlas supplies the body pixels"),
    sharedUiCanvasId, webGl2SharedCanvasPrototype: false, stageFrameRef: { current: frame },
    stagePresentation: { virtualWidth: 1024, virtualHeight: 768 }, viewportLayout: { offsetX: 10, offsetY: 11 },
    VIEWPORT_CELL_WIDTH: 48, VIEWPORT_CELL_HEIGHT: 32, CRYSTAL_MOVE_INPUT_INTERVAL_MS: 100,
    heldScenePointerRef: held, scenePointerDiagnosticsRef: { current: null },
    latestMoveInputRef: { current: { screen: "game", player: { x: 299, y: 618 }, newMove } },
    screen: "game", player: { x: 299, y: 618 }, sceneInteractionReady: sceneReady, bevyQuestUiCapturesPointer: false,
    bevyMailComposeReady: false, bevyMailComposePending: false, bevyBagUiActive: false, bevyHudUiReady: native,
    questLocalModalOpen: false, mobileMoreOpen: false, playerUiPreferences: { newMove },
    parityUiBlocksGameplay: () => flags.parity, combatUiHoldRef: { current: new Map() },
    onCombatUiHeld: modalOnHud ? (...args) => combatHolds.push(args) : undefined,
    onToggleOptions: modalOnHud ? () => { flags.parity = true; } : undefined,
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
    containsHudRect: hud.containsHudRect,
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
    handleSharedComposePointer: () => false, handleSharedMailPointer: () => false,
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
    const stageLostCapture = ${stageBinding("onLostPointerCapture")};
    const capturedPointerUp = ${terminal[0].initializer.getText(file)};
    const mountWorldHold = ${effects[0].arguments[0].getText(file)};`;
  const api = compile(`${source}\n${bindings}`, scope,
    `return { ${names.join(",")}, stageDown, stageMouseDownCapture, stageMouseDown, stageMouseUp, stageLostCapture, capturedPointerUp, mountWorldHold };`);
  scope.sharedBagPointerHandlerRef.current = api.handleSharedUiPointer;
  const cleanup = api.mountWorldHold();
  function event(target = canvas, { clientX = 30 + labelPoint.x / 2, clientY = 20 + labelPoint.y / 2, ...changes } = {}) {
    const prototype = { get clientX() { return clientX; }, get clientY() { return clientY; } };
    return Object.assign(Object.create(prototype), { target, currentTarget: frame, pointerId: 1, pointerType: "mouse", button: 0,
      isPrimary: true, timeStamp: now, preventDefault() { this.defaultPrevented = true; }, ...changes });
  }
  function addActor(objectId, kind, cell, { alpha = 255, missing = false, dead = false, ai } = {}) {
    const stack = new Element({ classes: ["entity-sprite-stack"], dataset: { objectId, uiInteractive: "true" }, parent: frame });
    const hit = new Element({ classes: ["entity-sprite-hit"], parent: stack });
    const body = { path: "/" + objectId + ".png", x: labelPoint.x - 2, y: labelPoint.y - 2, width: 4, height: 4 };
    const rgba = new Uint8Array(4 * body.width * body.height);
    rgba[(2 * body.width + 2) * 4 + 3] = alpha;
    if (!missing) stack.bodyImage = { complete: true, naturalWidth: body.width, naturalHeight: body.height, rgba,
      dataset: { mir2OriginalSrc: body.path } };
    viewportActorBodies.set(objectId, body);
    actors.push({ objectId, kind, ...cell, dead, ai });
    return hit;
  }
  return { api, scope, flags, frame, canvas, markerNode, label, held, status, picks, activations, clicks, moves, combatHolds, worldTile, labelPoint, event, addActor,
    set layers(value) { layers = value; }, set pickupAllowed(value) { pickupAllowed = value; },
    down(target = native ? canvas : frame, changes) { const e = event(target, changes); api.stageDown(e); return e; },
    mouseDown(target = native ? canvas : frame, changes) {
      const e = event(target, changes); api.stageMouseDownCapture(e); api.stageMouseDown(e); return e;
    },
    up(target = native ? canvas : frame, changes) {
      const e = event(target, changes); api.capturedPointerUp(e); api.stageMouseUp(e); window.emit("mouseup");
    },
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

test("a HUD Options press releases its original combat hold while the new modal blocks world input", () => {
  for (const phase of ["up", "lostCapture"]) {
    const f = fixture({ marker: "entity", newMove: true, modalOnHud: true });
    try {
      const option = f.status.plan.buttons.find(button => button.action === "option").rect;
      const press = { clientX: 30 + (option.left + option.width / 2) / 2,
        clientY: 20 + (option.top + option.height / 2) / 2 };
      f.down(f.canvas, press);
      assert.equal(f.flags.parity, true, "Options opened on the physical HUD down");
      const [channel, token, held] = f.combatHolds[0];
      assert.equal(channel, "hud"); assert.equal(held, true);
      f.api.capturedPointerUp(f.event(f.canvas, { ...press, pointerId: 2 }));
      assert.equal(f.combatHolds.length, 1, "a different pointer cannot release the HUD token");
      const release = f.event(f.canvas, press);
      if (phase === "up") f.api.capturedPointerUp(release);
      else f.api.stageLostCapture(release);
      assert.deepEqual(f.combatHolds, [["hud", token, true], ["hud", token, false]]);
      assert.equal(f.scope.hudPointerRouterRef.current.held, null);
      assert.deepEqual(f.clicks, [], "the Options gesture stays in the UI");
      f.down(f.canvas, { button: 2 }); f.up(f.canvas, { button: 2 });
      assert.deepEqual(f.clicks, [], "the still-open modal denies a fresh world action");
      f.flags.parity = false;
      f.down(f.canvas, { button: 2 }); f.up(f.canvas, { button: 2 });
      assert.deepEqual(f.clicks, [{ ...f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y), mode: "run" }]);
    } finally { f.cleanup(); }
  }
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
        const tile = f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y);
        const hit = f.addActor("npc-sprite-15", "npc", { x: tile.x + 1, y: tile.y });
        f.layers = [f.canvas, hit, f.frame];
      }
      f.down(); f.mouseDown(); f.up(); f.up();
      assert.deepEqual(f.activations, [sprite ? "npc-sprite-15" : "npc-15"]); assert.deepEqual(f.picks, []);
      assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
    } finally { f.cleanup(); }
  }
});

test("both real stage routes pick a solid body through a tree hole using native tile order", () => {
  for (const native of [true, false]) for (const alpha of [0, 1]) {
    const f = fixture({ native });
    try {
      const tile = f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y);
      const tree = f.addActor("tree", "monster", { x: tile.x, y: tile.y + 2 }, { alpha });
      const oma = f.addActor("oma", "monster", { x: tile.x + 1, y: tile.y + 1 });
      const foregroundOutsideScan = f.addActor("far", "monster", { x: tile.x, y: tile.y + 3 });
      f.layers = [...(native ? [f.canvas] : []), foregroundOutsideScan, oma, tree, f.frame];
      const target = native ? f.canvas : foregroundOutsideScan;
      f.down(target); f.mouseDown(target); f.up(target); f.up(target);
      assert.deepEqual(f.activations, [alpha ? "tree" : "oma"]);
      assert.deepEqual(f.clicks, []);
    } finally { f.cleanup(); }
  }
});

test("actor hit resolution keeps exact cells, living priority, ties and unavailable NPC bounds", () => {
  const f = fixture();
  try {
    const tile = f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y);
    const near = { x: tile.x + 1, y: tile.y };
    const ordinary = f.addActor("ordinary-corpse", "monster", near, { dead: true, ai: 0 });
    const harvest = f.addActor("harvestable-corpse", "monster", near, { dead: true, ai: 1 });
    const absent = f.addActor("absent-monster", "monster", near, { missing: true });
    const npc = f.addActor("absent-npc", "npc", near, { missing: true });
    const solid = f.addActor("second", "monster", near);
    f.layers = [f.canvas, ordinary, harvest, absent, npc, f.frame];
    const client = f.event();
    const target = () => f.api.sceneWorldObjectTarget(client.clientX, client.clientY)?.objectId;
    assert.equal(target(), "absent-npc");
    f.layers = [f.canvas, ordinary, absent, f.frame];
    assert.equal(target(), undefined);
    f.layers = [f.canvas, harvest, f.frame];
    assert.equal(target(), "harvestable-corpse");
    f.layers = [f.canvas, npc, solid, f.frame];
    assert.equal(target(), "second", "the last published actor in the cell wins even if below the other DOM element");
    f.addActor("exact", "monster", tile, { missing: true });
    f.layers = [f.canvas, f.frame];
    assert.equal(target(), "exact");
  } finally { f.cleanup(); }
});

test("actor picking reads the current packed body rectangle on its own atlas page", () => {
  const f = fixture({ packed: true });
  try {
    const tile = f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y);
    const tree = f.addActor("tree", "monster", { x: tile.x, y: tile.y + 2 }, { missing: true });
    const oma = f.addActor("oma", "monster", { x: tile.x, y: tile.y + 1 }, { missing: true });
    const page = () => ({ width: 16, height: 16, pixels: new Uint8Array(16 * 16 * 4) });
    const atlas = f.scope.activeBevyEntityAtlas;
    atlas.pages.push(page(), page());
    atlas.rects["/tree.png|4x4"] = { pageIndex: 1, x: 5, y: 5, width: 4, height: 4 };
    atlas.rects["/oma.png|4x4"] = { pageIndex: 1, x: 10, y: 5, width: 4, height: 4 };
    const offset = (x, y) => 4 * (y * 16 + x) + 3;
    atlas.pages[0].pixels[offset(7, 7)] = 255;
    atlas.pages[1].pixels[offset(12, 7)] = 255;
    f.layers = [f.canvas, tree, oma, f.frame];
    const { clientX, clientY } = f.event();
    assert.equal(f.api.sceneWorldObjectTarget(clientX, clientY)?.objectId, "oma");
    atlas.pages[1].pixels[offset(7, 7)] = 1;
    assert.equal(f.api.sceneWorldObjectTarget(clientX, clientY)?.objectId, "tree");
  } finally { f.cleanup(); }
});

test("a prebuilt atlas mask reads only the frame crop and shares one pending decode", async () => {
  const args = [], rgba = new Uint8Array(16);
  rgba[15] = 1;
  const document = { createElement() {
    return { getContext: () => ({ drawImage(...values) { args.push(values); }, getImageData: () => ({ data: rgba }) }) };
  } };
  const { warmCrystalBodyPixels, sampleCrystalBodyPixel } = load("../lib/crystal-actor-hit.ts", { document });
  const body = { path: "/body.png", width: 2, height: 2, x: -5, y: -5 };
  const image = { complete: true, naturalWidth: 1024, naturalHeight: 1024 };
  let loads = 0;
  const loadImage = async () => { loads++; return image; };
  warmCrystalBodyPixels(body, loadImage, { x: 12, y: 36 });
  warmCrystalBodyPixels(body, loadImage, { x: 12, y: 36 });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(loads, 1);
  assert.deepEqual(args, [[image, 12, 36, 2, 2, 0, 0, 2, 2]]);
  assert.equal(sampleCrystalBodyPixel(body, 1, 1, null), true);
  assert.equal(sampleCrystalBodyPixel(body, 0, 0, null), false);
  assert.equal(sampleCrystalBodyPixel(body, 2, 0, null), false);
});

test("secondary taps on a canvas-underlay actor or drop reach only the terrain callback", () => {
  const secondary = { button: 2 };
  for (const native of [true, false]) for (const marker of ["drop", "entity"]) {
    const f = fixture({ native, marker });
    try {
      const tile = f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y);
      f.down(undefined, secondary);
      f.mouseDown(undefined, secondary);
      assert.equal(f.held.current?.button, 2);
      assert.equal(f.held.current?.worldTarget, undefined);
      f.up(undefined, secondary);
      f.up(undefined, secondary);
      assert.deepEqual(f.clicks, [{ ...tile, mode: "run" }]);
      assert.deepEqual(f.moves, []);
      assert.deepEqual(f.picks, []);
      assert.deepEqual(f.activations, []);
    } finally { f.cleanup(); }
  }
});

test("holding secondary over an actor keeps Classic steering and New Move terrain routing", () => {
  const secondary = { button: 2 };
  for (const newMove of [false, true]) {
    const f = fixture({ marker: "entity", newMove });
    try {
      const tile = f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y);
      f.down(undefined, secondary);
      f.advance(210);
      f.up(undefined, secondary);
      f.advance(250);
      const expected = [{ ...tile, mode: "run" }, { ...tile, mode: "run" }];
      assert.deepEqual(f.moves, newMove ? [] : expected);
      assert.deepEqual(f.clicks, newMove ? expected : []);
      assert.deepEqual(f.activations, []);
    } finally { f.cleanup(); }
  }
});

test("direct DOM world markers pass secondary to the stage without opening an actor or picking up a drop", () => {
  const visual = ts.createSourceFile("scene-visuals.tsx",
    readFileSync(new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url), "utf8"),
    ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const spriteHandlers = nodes(visual, node => ts.isVariableDeclaration(node) &&
    ["handleEntityPointerActivate", "handleEntityContextActivate"].includes(node.name.getText(visual)))
    .map(node => "const " + node.getText(visual) + ";").join("\n");
  const attribute = (node, name) => node.attributes.properties.find(property => ts.isJsxAttribute(property) && property.name.getText(visual) === name);
  for (const className of ["entity-sprite-hit", "entity-nameplate", "entity-quest-icon", "ground-drop-marker"]) {
    const [node] = nodes(visual, node => {
      if (!ts.isJsxOpeningElement(node) && !ts.isJsxSelfClosingElement(node)) return false;
      const value = attribute(node, "className")?.initializer.getText(visual);
      return value === '"' + className + '"' || value?.startsWith("{`" + className + " ");
    });
    assert(node, className);
    const binding = name => attribute(node, name)?.initializer.expression?.getText(visual) ?? "undefined";
    const f = fixture({ native: false, marker: "entity" });
    try {
      const stack = new Element({ classes: ["entity-sprite-stack"], dataset: { objectId: "npc-15", uiInteractive: "true" }, parent: f.frame });
      const hit = new Element({ classes: [className], dataset: { objectId: "npc-15",
        uiInteractive: className === "entity-sprite-hit" || className === "entity-quest-icon" ? undefined : "true" },
        parent: className === "entity-sprite-hit" ? stack : f.frame });
      f.layers = [hit, f.frame];
      const handlers = compile(spriteHandlers, { isInteractiveEntity: true, entity: { objectId: "npc-15" },
        onActivateEntity: f.scope.onActivateEntity },
      "return { down: " + binding("onMouseDown") + ", context: " + binding("onContextMenu") + " };");
      const e = f.event(hit, { button: 2, stopPropagation() { this.stopped = true; } });
      f.api.stageMouseDownCapture(e);
      handlers.down?.(e);
      assert.equal(e.stopped, undefined, className);
      f.api.stageMouseDown(e);
      handlers.context?.(e);
      f.up(hit, { button: 2 });
      assert.deepEqual(f.activations, [], className);
      assert.deepEqual(f.picks, [], className);
      assert.deepEqual(f.clicks, [{ ...f.api.tileFromScenePoint(f.labelPoint.x, f.labelPoint.y), mode: "run" }], className);
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
  for (const scenario of ["dom-ui", "native-modal", "native-foreground", "scene", "quest", "parity", "shop"]) for (const button of [0, 2]) {
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
      f.down(target, { button }); f.mouseDown(target, { button }); f.up(target, { button });
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
