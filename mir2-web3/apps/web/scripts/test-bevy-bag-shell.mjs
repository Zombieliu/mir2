import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

function load(path, dependency) {
  const module = { exports: {} };
  new Function("exports", "module", "require", ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText)(module.exports, module, () => dependency);
  return module.exports;
}
const bagModule = load("../lib/bevy-bag-ui.ts", load("../lib/world-model/item-identity.ts"));
const { BagPointerRouter } = bagModule;
const { CharacterPointerRouter } = load("../lib/bevy-character-ui.ts",bagModule);
const { SpellsPointerRouter } = load("../lib/bevy-spells-ui.ts");
const { MailPointerRouter } = load("../lib/bevy-mail-ui.ts",load("../lib/extended-server-packets.ts"));
const { sharedUiCanvasId } = load("../lib/bevy-shared-canvas-mode.ts");
const shell = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
const ast = ts.createSourceFile("shell.tsx", shell, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const names = ["handleSharedUiPointer","handleSharedMailPointer","cancelSharedMailPointer","handleSharedSpellsPointer","cancelSharedSpellsPointer","handleSharedCharacterPointer","cancelSharedCharacterPointer","handleSharedHudPointer","cancelSharedHudPointer","beginCombatUiHold","endCombatUiHold","scenePointFromMouseEvent", "cancelSharedBagPointer", "handleSharedBagPointer",
  "isSharedBagCompatibilityMouse", "handleScenePointerAction", "handleScenePointerMove", "stopHeldScenePointer",
  "dispatchSceneMoveInput", "dispatchSceneClickInput"];
const found = new Map();
function visit(node) {
  if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) found.set(node.name.text, node.getText(ast));
  ts.forEachChild(node, visit);
}
visit(ast);
assert.equal(found.size, names.length);
const lostCaptureBindings=[];
function findLostCapture(node){if(ts.isJsxAttribute(node)&&node.name.getText(ast)==="onLostPointerCapture")lostCaptureBindings.push(node);ts.forEachChild(node,findLostCapture);}findLostCapture(ast);
assert.equal(lostCaptureBindings.length,1);
const lostArrow=lostCaptureBindings[0].initializer?.expression;
assert.ok(lostArrow&&ts.isArrowFunction(lostArrow)&&ts.isCallExpression(lostArrow.body));
assert.equal(lostArrow.body.expression.getText(ast),"handleSharedUiPointer");
assert.equal(lostArrow.body.arguments.length,2);assert.equal(lostArrow.body.arguments[0].getText(ast),lostArrow.parameters[0].name.getText(ast));assert.equal(lostArrow.body.arguments[1].text,"cancel");
const lostCaptureCode=ts.transpileModule(`const lostCapture = ${lostArrow.getText(ast)};`,{compilerOptions:{module:ts.ModuleKind.None,target:ts.ScriptTarget.ES2022}}).outputText;
const code = ts.transpileModule([...found.values()].join("\n"), {
  compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 },
}).outputText;

function fixture(webGl2SharedCanvasPrototype = false,channel="bag") {
  const clicks = [], moves = [], edges = [];
  let stops = 0, fallback = 0;
  let context = { runGeneration: 1, connectionGeneration: 2, sessionGeneration: 3, ownerRevision: 4,
    presentationRevision: 5, presentation: { logicalWidth: 1024, logicalHeight: 768, stageCssScale: .5, touch: false },
    inputRegions: [{ left: 700, top: 40, width: 316, height: 236 }] };
  class Element {
    constructor(id = "") { this.id = id; }
    focus() {}
    setPointerCapture() {}
    closest() { return null; }
    getBoundingClientRect() { return { left: 30, top: 20, width: 512, height: 384 }; }
  }
  const frame = new Element(), canvas = new Element(sharedUiCanvasId(webGl2SharedCanvasPrototype));
  const scope = {
    HTMLElement: Element, Element, onCombatPointer:undefined, onCombatUiHeld:undefined, combatUiHoldRef:{current:new Map()},
    characterPointerRouterRef:{current:new CharacterPointerRouter()},characterPointerCallbacksRef:{current:{getBevyCharacterPointerContext:()=>channel==="character"?{...context,ledgerRunGeneration:1,hudGeneration:1,modelRevision:1}:null,onBevyCharacterPointer:edge=>{edges.push({...edge,channel:"character"});return true;}}},
    hudPointerRouterRef:{current:{held:null,cancel:()=>null}},mailPointerRouterRef:{current:new MailPointerRouter()},
    mailPointerCallbacksRef:{current:{getBevyMailPointerContext:()=>channel==="mail"?{...context,run:1,sceneRevision:1,hudGeneration:1,playerObjectId:6,modelRevision:1,renderRevision:1,modal:false}:null,onBevyMailPointer:edge=>{edges.push({...edge,channel:"mail"});return true;}}},
    spellsPointerRouterRef:{current:new SpellsPointerRouter()},spellsPointerCallbacksRef:{current:{getBevySpellsPointerContext:()=>channel==="spells"?{...context,requestRun:1,hudGeneration:1,playerObjectId:6,modelRevision:1,renderRevision:1,modal:false}:null,onBevySpellsPointer:edge=>{edges.push({...edge,channel:"spells"});return true;}}},
    readBevyHudStatus:()=>null,bevyCharacterPageReady:channel==="character",bevyMailPageReady:channel==="mail",bevySpellsPageReady:channel==="spells",bevyHudUiReady:false,questLocalModalOpen:false,mobileMoreOpen:false,
    sharedUiCanvasId, webGl2SharedCanvasPrototype,
    stageFrameRef: { current: frame }, stagePresentation: { virtualWidth: 1024, virtualHeight: 768 },
    bagPointerRouterRef: { current: new BagPointerRouter() }, heldScenePointerRef: { current: null },
    bagPointerCallbacksRef: { current: { getBevyBagPointerContext: () => channel==="bag"?context:null,
      onBevyBagPointer: (edge) => { edges.push(edge); return true; } } },
    onViewportDirectionStop: () => stops++, onBevyBagTouchFallback: () => fallback++,
    bevyBagUiActive: channel==="bag", bevyQuestUiCapturesPointer: false, sceneInteractionReady: true,
    screen: "game", player: {}, latestMoveInputRef: { current: { screen: "game" } },
    tileFromScenePoint: (x, y) => ({ x, y }),
    onViewportTileClick: (x, y) => clicks.push({ x, y, mode: "walk" }),
    onViewportTileSecondaryAction: (x, y) => clicks.push({ x, y, mode: "run" }),
    onViewportDirectionStep: (x, y, mode) => moves.push({ x, y, mode }), Date,
  };
  const api = new Function(...Object.keys(scope), `${code}\n${lostCaptureCode}\nreturn {${names.join(",")},lostCapture};`)(...Object.values(scope));
  function event(x, y, button = 0, pointerType = "mouse", pointerId = 1) {
    // Browser PointerEvent coordinates are getters on the prototype, not
    // enumerable object fields. This catches object-spread coordinate loss.
    const proto = { get clientX() { return x * .5 + 30; }, get clientY() { return y * .5 + 20; } };
    return Object.assign(Object.create(proto), { target: canvas, currentTarget: frame, pointerId, pointerType, button,
      preventDefault() { this.defaultPrevented = true; } });
  }
  return { ...api, scope, canvas, clicks, moves, edges, event,
    setTouch() { context = { ...context, presentation: { logicalWidth: 1368, logicalHeight: 768, stageCssScale: 640 / 1368, touch: true } }; },
    loseLayout() { context = null; }, get stops() { return stops; }, get fallback() { return fallback; } };
}

test("actual Shell UI-down dragged out and released never reaches the world helper", () => {
  const f = fixture();
  f.handleSharedBagPointer(f.event(720, 100), "down");
  f.handleSharedBagPointer(f.event(400, 450), "move");
  f.handleSharedBagPointer(f.event(400, 450), "up");
  assert.deepEqual(f.edges.map(({ phase, origin }) => ({ phase, origin })), [
    { phase: "down", origin: "bag" }, { phase: "move", origin: "bag" }, { phase: "up", origin: "bag" },
  ]);
  assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
});

test("actual Shell outside-world down/click and hold work while compatibility mouse cannot duplicate", () => {
  const f = fixture();
  f.handleSharedBagPointer(f.event(400, 450), "down");
  f.handleScenePointerAction(f.event(400, 450));
  f.handleSharedBagPointer(f.event(400, 450), "up");
  assert.deepEqual(f.clicks, [{ x: 400, y: 450, mode: "walk" }]);
  assert.equal(f.isSharedBagCompatibilityMouse(f.event(400, 450)), true);
  f.handleSharedBagPointer(f.event(400, 450, 2), "down");
  f.handleSharedBagPointer(f.event(730, 90, 2), "move");
  const held = f.scope.heldScenePointerRef.current;
  held.dispatched = true; f.dispatchSceneMoveInput(held);
  f.handleSharedBagPointer(f.event(730, 90, 2), "up");
  assert.deepEqual(f.moves, [{ x: 730, y: 90, mode: "run" }]);
  assert.equal(f.clicks.length, 1);
  assert.equal(f.edges.at(-1).origin, "world");
});

test("actual Shell lost layout cancels an outstanding world pulse and never releases it as a click", () => {
  const f = fixture();
  f.handleSharedBagPointer(f.event(400, 450), "down");
  f.loseLayout();
  f.handleSharedBagPointer(f.event(400, 450), "up");
  assert.equal(f.scope.heldScenePointerRef.current, null);
  assert.deepEqual(f.clicks, []);
  assert.ok(f.edges.some((edge) => edge.phase === "cancel"));
});

test("actual Shell touch down requests compatibility without granting a world or Bevy gesture", () => {
  const f = fixture(); f.handleSharedBagPointer(f.event(400, 450, 0, "touch"), "down");
  assert.equal(f.fallback, 1); assert.deepEqual(f.edges, []); assert.deepEqual(f.clicks, []);
});

test("actual Shell wide touch uses the Bag lease, swallows second finger and never emits world input", () => {
  const f = fixture(); f.setTouch();
  const first = f.event(720, 90, 0, "touch", 1);
  f.handleSharedBagPointer(first, "down");
  assert.equal(first.defaultPrevented, true);
  const second = f.event(730, 100, 0, "touch", 2);
  f.handleSharedBagPointer(second, "down");
  assert.equal(second.defaultPrevented, true);
  f.handleSharedBagPointer(f.event(730, 100, 0, "touch", 2), "up");
  f.handleSharedBagPointer(f.event(400, 450, 0, "touch", 1), "move");
  f.handleSharedBagPointer(f.event(400, 450, 0, "touch", 1), "up");
  assert.deepEqual(f.edges.map(({ phase, origin, pointerId }) => ({ phase, origin, pointerId })), [
    { phase: "down", origin: "bag", pointerId: 1 }, { phase: "move", origin: "bag", pointerId: 1 },
    { phase: "up", origin: "bag", pointerId: 1 },
  ]);
  assert.equal(f.fallback, 0); assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
  f.handleSharedBagPointer(f.event(400, 450, 0, "touch", 3), "down");
  f.handleSharedBagPointer(f.event(400, 450, 0, "touch", 3), "up");
  assert.equal(f.edges.at(-1).origin, "world");
  assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
});

test("opt-in primary canvas keeps Bag touch lease, second-finger exclusion, and ordinary routing isolation", () => {
  const f = fixture(true); f.setTouch();
  assert.equal(f.canvas.id, "mir2-web3-canvas");
  const first = f.event(720, 90, 0, "touch", 1);
  f.handleSharedBagPointer(first, "down");
  const second = f.event(730, 100, 0, "touch", 2);
  f.handleSharedBagPointer(second, "down");
  f.handleSharedBagPointer(f.event(730, 100, 0, "touch", 2), "up");
  f.handleSharedBagPointer(f.event(720, 90, 0, "touch", 1), "up");
  assert.equal(first.defaultPrevented, true);
  assert.equal(second.defaultPrevented, true);
  assert.deepEqual(f.edges.map(({ phase, origin, pointerId }) => ({ phase, origin, pointerId })), [
    { phase: "down", origin: "bag", pointerId: 1 }, { phase: "up", origin: "bag", pointerId: 1 },
  ]);
  assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
  const ordinaryCanvas = new f.scope.HTMLElement("mir2-quest-ui-canvas");
  const unrelated = f.event(720, 90, 0, "touch", 3); unrelated.target = ordinaryCanvas;
  f.handleSharedBagPointer(unrelated, "down");
  assert.equal(unrelated.defaultPrevented, undefined);
  assert.equal(f.edges.length, 2);
});

test("opt-in primary canvas retains mouse world origin and blur quarantine without stale click", () => {
  const f = fixture(true);
  f.handleSharedBagPointer(f.event(400, 450), "down");
  f.handleSharedBagPointer(f.event(720, 90), "move");
  f.cancelSharedBagPointer("blur");
  f.handleSharedBagPointer(f.event(720, 90), "up");
  assert.deepEqual(f.clicks, []);
  assert.deepEqual(f.edges.map(({ phase, origin }) => ({ phase, origin })), [
    { phase: "down", origin: "world" }, { phase: "move", origin: "world" },
    { phase: "blur", origin: "world" }, { phase: "up", origin: "world" },
  ]);
  f.handleSharedBagPointer(f.event(400, 450), "down");
  f.handleSharedBagPointer(f.event(400, 450), "up");
  assert.deepEqual(f.clicks, [{ x: 400, y: 450, mode: "walk" }]);
});

test("actual Shell terminal pointercancel immediately frees its pointerId for the next real gesture", () => {
  const f = fixture(); f.setTouch();
  f.handleSharedBagPointer(f.event(720, 90, 0, "touch", 1), "down");
  f.handleSharedBagPointer(f.event(720, 90, 0, "touch", 1), "cancel");
  f.handleSharedBagPointer(f.event(720, 90, 0, "touch", 1), "cancel"); // lost capture follows cancel
  f.handleSharedBagPointer(f.event(730, 90, 0, "touch", 1), "down");
  f.handleSharedBagPointer(f.event(730, 90, 0, "touch", 1), "up");
  assert.deepEqual(f.edges.map(({ phase, pointerId }) => ({ phase, pointerId })), [
    { phase: "down", pointerId: 1 }, { phase: "cancel", pointerId: 1 },
    { phase: "down", pointerId: 1 }, { phase: "up", pointerId: 1 },
  ]);
  assert.equal(lostArrow.body.expression.getText(ast), "handleSharedUiPointer");
  f.lostCapture(f.event(730,90,0,"touch",1));
  assert.equal(f.scope.bagPointerRouterRef.current.held,null);assert.equal(f.edges.filter(e=>e.phase==="cancel").length,1);
});

test("actual Shell retired proactive cancel cannot clear the next pointer lease", () => {
  const f = fixture(); f.setTouch();
  f.handleSharedBagPointer(f.event(720, 90, 0, "touch", 1), "down");
  f.cancelSharedBagPointer(); // owner, blur or resize; old finger may still be held
  f.handleSharedBagPointer(f.event(730, 90, 0, "touch", 2), "down");
  f.handleSharedBagPointer(f.event(720, 90, 0, "touch", 1), "cancel");
  assert.equal(f.scope.bagPointerRouterRef.current.held?.pointerId, 2);
  f.handleSharedBagPointer(f.event(730, 90, 0, "touch", 2), "up");
  assert.deepEqual(f.edges.map(({ phase, pointerId }) => ({ phase, pointerId })), [
    { phase: "down", pointerId: 1 }, { phase: "cancel", pointerId: 1 }, { phase: "down", pointerId: 2 },
    { phase: "cancel", pointerId: 1 }, { phase: "up", pointerId: 2 },
  ]);
});

test("actual Shell blur without old up accepts a new down with the reused touch pointerId", () => {
  const f = fixture(); f.setTouch();
  f.handleSharedBagPointer(f.event(720, 90, 0, "touch", 1), "down");
  f.cancelSharedBagPointer("blur");
  f.handleSharedBagPointer(f.event(400, 450, 0, "touch", 1), "move");
  assert.equal(f.scope.bagPointerRouterRef.current.held, null);
  f.handleSharedBagPointer(f.event(730, 90, 0, "touch", 1), "down");
  f.handleSharedBagPointer(f.event(730, 90, 0, "touch", 1), "up");
  assert.deepEqual(f.edges.map(({ phase, pointerId }) => ({ phase, pointerId })), [
    { phase: "down", pointerId: 1 }, { phase: "blur", pointerId: 1 },
    { phase: "down", pointerId: 1 }, { phase: "up", pointerId: 1 },
  ]);
  assert.deepEqual(f.clicks, []); assert.deepEqual(f.moves, []);
});

test("actual Page compatibility interaction stays latched across tabs, resets on close, and is opt-in only", () => {
  const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const selected = ["setShowInventory", "openInventory", "requestBagCompatibility", "retainReactBagInteraction"];
  const declarations = [];
  function find(node) {
    if (ts.isFunctionDeclaration(node) && selected.includes(node.name?.text)) declarations.push(node.getText(ast));
    ts.forEachChild(node, find);
  }
  find(ast); assert.equal(declarations.length, 4);
  const js = ts.transpileModule(declarations.join("\n"), { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 } }).outputText;
  for (const requested of [false, true]) {
    const state = { open: false, mode: null, storageVersion: 7, tab: "bag1", yields: 0 };
    const scope = {
      sharedHudNavigationRef:{current:{ready:false}},hudProjectionRef:{current:false},fallbackHudNavigationRevisionRef:{current:0},nextHudNavigationRevision:()=>1,navigateSharedHud:()=>false,
      bevyBagUiRequested: requested, bagOpenRef: { current: false }, bagCompatibilityModeRef: { current: null },
      bagYieldRef: { current: () => state.yields++ },
      setShowInventoryState: (value) => { state.open = value; },
      setBagCompatibilityMode: (value) => { state.mode = value; },
      setStorageServiceOpenVersion: (value) => { state.storageVersion = value; },
      setActiveInventoryTab: (value) => { state.tab = value; }, closeTouchSecondaryWindows: () => {},
    };
    const api = new Function(...Object.keys(scope), `${js}\nreturn {${selected.join(",")}};`)(...Object.values(scope));
    api.openInventory("bag1");
    assert.equal(state.mode, null, "opening/hover itself does not pin compatibility");
    api.retainReactBagInteraction();
    assert.equal(state.mode, requested ? "fullInventory" : null);
    api.openInventory("bag2");
    assert.equal(state.mode, requested ? "fullInventory" : null, "tab change retains this open's owner");
    api.setShowInventory(false);
    assert.equal(state.open, false); assert.equal(state.mode, null);
    assert.equal(state.storageVersion, requested ? 0 : 7, "legacy UI signal unchanged without B2");
    api.openInventory("bag1");
    assert.equal(state.open, true); assert.equal(state.mode, null, "new open may handshake again");
  }
  const inventory = readFileSync(new URL("../app/components/original-client-inventory-window.tsx", import.meta.url), "utf8");
  assert.match(inventory, /onPointerDownCapture=\{onCompatibilityInteraction\}/);
  assert.match(inventory, /onKeyDownCapture=\{onCompatibilityInteraction\}/);
  assert.doesNotMatch(inventory, /on(?:MouseEnter|PointerEnter|MouseMove|PointerMove)Capture=\{onCompatibilityInteraction\}/);
});

test("actual Page touch Quest switch closes foreground Bag before opening the retained Quest view", () => {
  const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let declaration;
  function find(node) {
    if (ts.isFunctionDeclaration(node) && node.name?.text === "toggleQuestLogWindow") declaration = node.getText(ast);
    ts.forEachChild(node, find);
  }
  find(ast); assert.ok(declaration);
  const js = ts.transpileModule(declaration, { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022 } }).outputText;
  for (const layout of ["touch", "desktop"]) {
    const calls = [];
    const scope = { showQuestLog: false, clientProfile: { layout },navigateSharedHud:()=>false,
      closeTouchSecondaryWindows: () => calls.push("close bag"),
      setShowQuestLog: (open) => calls.push(open ? "open quest" : "close quest") };
    const toggle = new Function(...Object.keys(scope), `${js}\nreturn toggleQuestLogWindow;`)(...Object.values(scope));
    toggle();
    assert.deepEqual(calls, layout === "touch" ? ["close bag", "open quest"] : ["open quest"]);
  }
});


test("actual root JSX lost capture traverses Mail Spells Bag Character owners exactly once and preserves successor lease",()=>{
 for(const channel of["bag","mail","spells","character"]){const f=fixture(false,channel);f.setTouch();const ref=f.scope[`${channel}PointerRouterRef`];
  f.handleSharedUiPointer(f.event(720,90,0,"touch",1),"down");assert.equal(ref.current.held?.pointerId,1,channel);
  f.lostCapture(f.event(720,90,0,"touch",1));f.lostCapture(f.event(720,90,0,"touch",1));assert.equal(ref.current.held,null);assert.equal(f.edges.filter(e=>e.phase==="cancel").length,1,channel);assert.equal(f.edges.some(e=>e.phase==="up"),false);
  f.handleSharedUiPointer(f.event(730,90,0,"touch",1),"down");assert.equal(ref.current.held?.pointerId,1,"same pointer ID is immediately reusable");f.handleSharedUiPointer(f.event(730,90,0,"touch",1),"up");assert.equal(ref.current.held,null);
  f.handleSharedUiPointer(f.event(720,90,0,"touch",2),"down");const successor=ref.current.held;assert.ok(successor);
  f.lostCapture(f.event(720,90,0,"touch",1));assert.equal(ref.current.held,successor,"old terminal cannot clear another pointer/owner lease");f.lostCapture(f.event(720,90,0,"touch",2));assert.equal(ref.current.held,null);
  assert.deepEqual(f.clicks,[]);assert.deepEqual(f.moves,[]);assert.equal(f.edges.filter(e=>e.phase==="up").length,1);assert.equal(f.edges.filter(e=>e.phase==="cancel"&&e.pointerId===2).length,1);
 }
});
