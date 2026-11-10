import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function source(relative) {
  const text = readFileSync(new URL(relative, import.meta.url), "utf8");
  return ts.createSourceFile(relative, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
}
function nodes(file, predicate) {
  const found = [];
  const visit = node => { if (predicate(node)) found.push(node); ts.forEachChild(node, visit); };
  visit(file); return found;
}
function compile(text, scope, result = "") {
  const code = ts.transpileModule(text, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } }).outputText;
  return new Function(...Object.keys(scope), `${code}\n${result}`)(...Object.values(scope));
}
const shell = source("../app/original-client-shell.tsx");
const effect = nodes(shell, node => ts.isCallExpression(node) && ts.isIdentifier(node.expression)
  && node.expression.text === "useEffect" && ts.isArrowFunction(node.arguments[0])
  && ts.isBlock(node.arguments[0].body) && node.arguments[0].body.statements.some(statement =>
    ts.isFunctionDeclaration(statement) && statement.name?.text === "handleKeyboardMoveDown"));
assert.equal(effect.length, 1, "extract the actual movement keyboard effect");
const handlers = effect[0].arguments[0].body.statements.filter(node => ts.isFunctionDeclaration(node)
  && ["handleKeyboardMoveDown", "handleKeyboardMoveUp"].includes(node.name?.text));
assert.equal(handlers.length, 2);
const helperNames = ["keyboardMoveDirectionForKey", "crystalDirectionFromKeyboardVector", "keyboardInputTargetIsEditable",
  "npcShopBlocksWorldInput", "stopNpcShopWorldInput", "dispatchKeyboardMoveInput", "heroBlocksWorldInput"];
const helpers = nodes(shell, node => ts.isFunctionDeclaration(node) && helperNames.includes(node.name?.text));
assert.equal(helpers.length, helperNames.length);
const guildKeyboard = source("../lib/r16-guild-keyboard.ts");
const helperSource = [...helpers.map(node => node.getText(shell)),
  guildKeyboard.statements[0].getText(guildKeyboard).replace("export ", "")].join("\n");
const layout = source("../app/components/original-client-scene-layout.ts");
const interval = nodes(layout, node => ts.isVariableDeclaration(node) && ts.isIdentifier(node.name)
  && node.name.text === "CRYSTAL_MOVE_INPUT_INTERVAL_MS");
assert.equal(interval.length, 1); assert(ts.isNumericLiteral(interval[0].initializer));
const intervalMs = Number(interval[0].initializer.text);

class Element {
  constructor(editable = false) { this.isContentEditable = editable; }
  closest() { return null; }
}
class Input extends Element {}
class Textarea extends Element {}
class Select extends Element {}
function key(key, changes = {}) {
  return { key, repeat: false, shiftKey: false, altKey: false, ctrlKey: false, metaKey: false,
    defaultPrevented: false, preventions: 0, propagations: 0, target: new Element(),
    preventDefault() { this.defaultPrevented = true; this.preventions++; },
    stopPropagation() { this.propagations++; }, stopImmediatePropagation() { this.propagations++; }, ...changes };
}
function clock() {
  let now = 0, serial = 0;
  const timers = new Map(), listeners = new Map();
  return {
    setInterval(fn, ms) { assert(ms > 0); const id = ++serial; timers.set(id, { fn, ms, at: now + ms }); return id; },
    clearInterval(id) { timers.delete(id); },
    addEventListener(type, fn, capture = false) {
      if (type === "keydown" || type === "keyup") assert.equal(capture, true, "movement precedes the Winit target listener");
      const group = listeners.get(type) ?? new Set(); group.add(fn); listeners.set(type, group);
    },
    removeEventListener(type, fn) { listeners.get(type)?.delete(fn); },
    emit(type, event) { for (const fn of [...(listeners.get(type) ?? [])]) fn(event); },
    advance(ms) {
      const end = now + ms;
      for (;;) {
        const next = [...timers.values()].filter(timer => timer.at <= end).sort((a, b) => a.at - b.at)[0];
        if (!next) break;
        now = next.at; next.at += next.ms; next.fn();
      }
      now = end;
    },
    get now() { return now; },
    get timers() { return timers.size; }, get listeners() { return [...listeners.values()].reduce((n, group) => n + group.size, 0); }
  };
}
function shellFixture(changes = {}) {
  const flags = { screen: "game", sceneReady: true, ...changes }, window = clock();
  const moves = [], stops = [], cancellations = [], combatPointers = [];
  const held = { current: new Set() }, running = { current: false }, scenePointer = { current: null };
  const diagnostic = { current: { lastKey: null, lastDispatch: null, lastShortcut: null } };
  let cleanup = null, previousDeps = null, installs = 0;
  const scope = {
    window, Element, HTMLElement: Element, HTMLInputElement: Input, HTMLTextAreaElement: Textarea, HTMLSelectElement: Select,
    CRYSTAL_MOVE_INPUT_INTERVAL_MS: intervalMs, screen: flags.screen, sceneInteractionReady: flags.sceneReady,
    bevyQuestUiCapturesPointer: !!flags.quest, bevyStorageUiActive: !!flags.storageActive,
    bevyStorageUiTransitioning: !!flags.storageTransition, bevyNpcShopUiActive: !!flags.shopActive,
    bevyNpcShopUiTransitioning: !!flags.shopTransition, bevyMailComposeReady: !!flags.mailReady,
    bevyMailComposePending: !!flags.mailPending, parityUiBlocksGameplay: () => !!flags.parity,
    heroPointerLeaseRef: { current: flags.heroPointer ? {} : null },
    heroPointerCallbacksRef: { current: { getBevyHeroInputBlocked: () => !!flags.hero } },
    npcShopPointerCallbacksRef: { current: { getBevyNpcShopInputBlocked: () => !!flags.shop } },
    storagePointerCallbacksRef: { current: { getBevyStoragePointerContext: () => flags.storageContext ? {} : null } },
    spellsPointerCallbacksRef: { current: { getBevySpellsPointerContext: () => ({ modal: !!flags.spells }) } },
    latestMoveInputRef: { current: { screen: flags.screen, player: flags.noPlayer ? null : { objectId: "1" }, renderPlayer: null } },
    heldKeyboardMoveKeysRef: held, heldKeyboardRunModeRef: running, heldScenePointerRef: scenePointer,
    keyboardMovementDiagnosticsRef: diagnostic, getKeybindCaptureActive: () => !!flags.capture,
    isCurrentMailTextTarget: () => !!flags.mailText,
    cancelWorldFishingHeldPointer: () => cancellations.push("fishing"),
    updateSceneCombatPointer: (...args) => combatPointers.push(args),
    onViewportDirectionIntent: (...args) => moves.push(args), onViewportDirectionStop: () => stops.push("stop"),
    useEffect(setup, deps) {
      if (previousDeps && deps.length === previousDeps.length && deps.every((value, i) => Object.is(value, previousDeps[i]))) return;
      cleanup?.(); previousDeps = [...deps]; installs++; cleanup = setup() ?? null;
    }
  };
  // Direct handlers cover even a captured old listener during a modal's passive
  // effect cleanup. The second entry point runs the complete real effect.
  const direct = compile(`${helperSource}\n${handlers.map(node => node.getText(shell)).join("\n")}`, scope,
    "return { down: handleKeyboardMoveDown, up: handleKeyboardMoveUp };");
  return {
    flags, window, moves, stops, cancellations, combatPointers, held, running, scenePointer, diagnostic, direct,
    render(callbacks = {}) { Object.assign(scope, callbacks); compile(`${helperSource}\n${effect[0].getText(shell)};`, scope); },
    cleanup() { cleanup?.(); cleanup = null; }, get installs() { return installs; }
  };
}

const blockers = {
  parity: { parity: true }, hero: { hero: true }, "held Hero pointer": { heroPointer: true }, shop: { shop: true },
  "active storage": { storageActive: true }, "transitioning storage": { storageTransition: true },
  "storage context": { storageContext: true }, "mail compose": { mailReady: true },
  "pending mail": { mailPending: true }, "spells modal": { spells: true }, quest: { quest: true },
  "scene not ready": { sceneReady: false }, "keybind capture": { capture: true }
};

test("actual movement handlers leave panel/nonmovement keys untouched before every world blocker", () => {
  for (const [label, flags] of Object.entries(blockers)) for (const phase of ["down", "up"])
    for (const value of ["F9", "Escape", "i", "Enter", "1"]) {
      const f = shellFixture(flags), event = key(value), pointer = {};
      f.held.current.add("down"); f.running.current = true; f.scenePointer.current = pointer;
      f.direct[phase](event);
      const context = `${label}/${phase}/${value}`;
      assert.equal(event.preventions, 0, context); assert.equal(event.propagations, 0, context);
      assert.equal(f.stops.length, 0, context); assert.equal(f.cancellations.length, 0, context);
      assert.equal(f.combatPointers.length, 0, context); assert.equal(f.moves.length, 0, context);
      assert.deepEqual([...f.held.current], ["down"], context); assert.equal(f.running.current, true, context);
      assert.equal(f.scenePointer.current, pointer, context);
    }
});

test("actual Arrow handlers retain every modal, capture, scene, identity and modifier guard", () => {
  for (const [label, flags] of Object.entries({ ...blockers, "not game": { screen: "select" }, "no player": { noPlayer: true } })) {
    const f = shellFixture(flags), down = key("ArrowDown");
    f.direct.down(down); f.direct.up(key("ArrowDown"));
    assert.equal(f.moves.length, 0, label);
  }
  for (const [label, changes] of Object.entries({ prevented: { defaultPrevented: true }, alt: { altKey: true }, ctrl: { ctrlKey: true },
    meta: { metaKey: true }, input: { target: new Input() }, textarea: { target: new Textarea() },
    select: { target: new Select() }, editable: { target: new Element(true) } })) {
    const f = shellFixture(); f.direct.down(key("ArrowDown", changes));
    assert.equal(f.moves.length, 0, label); assert.equal(f.held.current.size, 0, label);
  }
});

test("the installed capture effect preserves Arrow edges, held cadence, Shift, diagonal and release semantics", () => {
  const f = shellFixture(); f.render();
  try {
    assert.equal(intervalMs, 100); assert.equal(f.window.timers, 1);
    const down = key("ArrowDown"); f.window.emit("keydown", down);
    assert.equal(down.defaultPrevented, true);
    assert.deepEqual(f.moves.at(-1), ["Down", "walk", { discrete: true }]);
    const count = f.moves.length; f.window.emit("keydown", key("ArrowDown", { repeat: true }));
    assert.equal(f.moves.length, count, "browser repeats cannot create additional discrete edges");
    f.window.advance(intervalMs); assert.deepEqual(f.moves.at(-1), ["Down", "walk", { discrete: false }]);
    f.window.emit("keydown", key("Shift")); assert.deepEqual(f.moves.at(-1), ["Down", "run", { discrete: false }]);
    f.window.emit("keyup", key("Shift")); assert.deepEqual(f.moves.at(-1), ["Down", "walk", { discrete: false }]);
    f.window.emit("keydown", key("ArrowRight")); assert.deepEqual(f.moves.at(-1), ["DownRight", "walk", { discrete: true }]);
    f.window.emit("keyup", key("ArrowRight")); assert.deepEqual(f.moves.at(-1), ["Down", "walk", { discrete: true }]);
    f.window.emit("keyup", key("ArrowDown")); assert.equal(f.held.current.size, 0); assert.equal(f.stops.length, 1);
    const released = f.moves.length; f.window.advance(300); assert.equal(f.moves.length, released);
    f.window.emit("keydown", key("W")); assert.deepEqual(f.moves.at(-1), ["Up", "walk", { discrete: true }]);
    f.window.emit("blur", {}); assert.equal(f.held.current.size, 0); assert.equal(f.running.current, false);
    const blurred = f.moves.length; f.window.advance(300); assert.equal(f.moves.length, blurred);
  } finally { f.cleanup(); }
  assert.equal(f.window.timers, 0); assert.equal(f.window.listeners, 0);
});

test("the installed effect withdraws on modal mount and dynamic parity still denies Arrow while yielding F9", () => {
  for (const flags of [{ quest: true }, { storageActive: true }, { storageTransition: true }, { shopActive: true }, { shopTransition: true }, { screen: "select" }]) {
    const f = shellFixture(flags); f.held.current.add("down"); f.running.current = true; f.render();
    assert.equal(f.window.listeners, 0); assert.equal(f.window.timers, 0); assert.equal(f.stops.length, 1);
    assert.equal(f.held.current.size, 0); assert.equal(f.running.current, false);
  }
  const f = shellFixture(); f.render();
  try {
    f.flags.parity = true;
    const panel = key("F9"); f.window.emit("keydown", panel); f.window.emit("keyup", panel);
    assert.equal(panel.defaultPrevented, false); assert.equal(f.stops.length, 0); assert.equal(f.moves.length, 0);
    const arrow = key("ArrowDown"); f.window.emit("keydown", arrow);
    assert.equal(arrow.defaultPrevented, true); assert.equal(f.moves.length, 0); assert.equal(f.held.current.size, 0);
  } finally { f.cleanup(); }
});

test("a transient live guard at mount cannot prevent listener installation or require a prop rerender to accept a later gesture", () => {
  for (const guard of ["parity", "hero", "shop"]) {
    const f = shellFixture({ [guard]: true }); f.render();
    try {
      assert.equal(f.window.timers, 1, guard); assert.equal(f.window.listeners, 3, guard);
      const blocked = key("ArrowDown"); f.window.emit("keydown", blocked);
      assert.equal(blocked.defaultPrevented, true, guard); assert.equal(f.moves.length, 0, guard);
      assert.equal(f.held.current.size, 0, "the blocked physical hold is cancelled");
      f.flags[guard] = false;
      f.window.advance(300); assert.equal(f.moves.length, 0, "lifting a guard never replays the cancelled hold");
      f.window.emit("keydown", key("ArrowDown"));
      assert.deepEqual(f.moves.at(-1), ["Down", "walk", { discrete: true }], guard);
      assert.equal(f.installs, 1, "no prop rerender or listener reinstall was necessary");
      f.window.emit("keyup", key("ArrowDown"));
    } finally { f.cleanup(); }
  }
});

function pageFixture() {
  const page = source("../app/page.tsx"), names = ["viewportDirectionCallbacksRef", "onViewportDirectionIntentStable",
    "onViewportDirectionStepStable", "onViewportDirectionStopStable"];
  const declarations = nodes(page, node => ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && names.includes(node.name.text));
  assert.equal(declarations.length, names.length, "extract the real Page forwarder declarations");
  const assignments = nodes(page, node => ts.isExpressionStatement(node) && ts.isBinaryExpression(node.expression)
    && node.expression.operatorToken.kind === ts.SyntaxKind.EqualsToken && ts.isPropertyAccessExpression(node.expression.left)
    && ts.isIdentifier(node.expression.left.expression) && node.expression.left.expression.text === "viewportDirectionCallbacksRef"
    && node.expression.left.name.text === "current");
  assert.equal(assignments.length, 1);
  const attributes = {};
  for (const node of nodes(page, node => ts.isJsxAttribute(node)
    && ["onViewportDirectionIntent", "onViewportDirectionStep", "onViewportDirectionStop"].includes(node.name.getText(page)))) {
    assert(ts.isJsxExpression(node.initializer) && node.initializer.expression);
    assert.equal(attributes[node.name.getText(page)], undefined, "one Shell wiring per direction prop");
    attributes[node.name.getText(page)] = node.initializer.expression.getText(page);
  }
  assert.equal(Object.keys(attributes).length, 3);
  const statements = [...declarations.map(node => node.parent.parent), ...assignments].sort((a, b) => a.pos - b.pos);
  const text = [...new Set(statements)].map(node => node.getText(page)).join("\n");
  const slots = []; let cursor = 0;
  const hooks = {
    useRef(initial) { const index = cursor++; return slots[index] ??= { current: initial }; },
    useCallback(callback, deps) {
      const index = cursor++, old = slots[index];
      if (!old || deps.length !== old.deps.length || deps.some((dep, i) => !Object.is(dep, old.deps[i]))) slots[index] = { callback, deps: [...deps] };
      return slots[index].callback;
    }
  };
  return {
    render(intent, stop, step = () => undefined) {
      cursor = 0;
      return compile(text, { ...hooks, handleViewportDirectionIntent: intent, handleViewportDirectionStep: step, handleViewportDirectionStop: stop },
        `return { onViewportDirectionIntent: ${attributes.onViewportDirectionIntent}, onViewportDirectionStep: ${attributes.onViewportDirectionStep},
          onViewportDirectionStop: ${attributes.onViewportDirectionStop} };`);
    }
  };
}

test("actual Page JSX forwarders keep identity across rerenders and invoke only the latest intent/stop closures", () => {
  const page = pageFixture(), calls = [];
  const render = label => page.render((...args) => calls.push([label, "intent", ...args]), () => calls.push([label, "stop"]));
  const first = render("first"), second = render("second");
  assert.equal(first.onViewportDirectionIntent, second.onViewportDirectionIntent);
  assert.equal(first.onViewportDirectionStop, second.onViewportDirectionStop);
  const options = { discrete: true };
  first.onViewportDirectionIntent("Down", "walk", options); first.onViewportDirectionStop();
  assert.deepEqual(calls, [["second", "intent", "Down", "walk", options], ["second", "stop"]]);
  assert.equal(calls[0][4], options, "forwarding preserves the exact physical-edge options object");
  render("third"); second.onViewportDirectionIntent("Up", "run"); second.onViewportDirectionStop();
  assert.deepEqual(calls.slice(2), [["third", "intent", "Up", "run"], ["third", "stop"]]);
});

test("50ms Page rerenders cannot starve the real 100ms held interval and each tick reaches the latest Page closure", () => {
  const page = pageFixture(), calls = [], f = shellFixture();
  let rendered = 0;
  const render = () => {
    const revision = ++rendered;
    f.render(page.render((...args) => calls.push({ revision, args }), () => calls.push({ revision, stop: true })));
  };
  render(); f.window.emit("keydown", key("ArrowDown"));
  try {
    assert.deepEqual(calls[0], { revision: 1, args: ["Down", "walk", { discrete: true }] });
    for (let i = 0; i < 40; i++) { render(); f.window.advance(50); }
    const held = calls.filter(call => call.args?.[2].discrete === false);
    assert.equal(held.length, 20, "the original interval remains alive across forty unrelated Page renders");
    assert.equal(f.installs, 1); assert.equal(f.window.timers, 1);
    assert.equal(held.at(-1).revision, rendered, "a retained Shell closure still invokes current Page logic");
    f.window.emit("keyup", key("ArrowDown")); assert.deepEqual(calls.at(-1), { revision: rendered, stop: true });
    const released = calls.length; f.window.advance(200); assert.equal(calls.length, released);
  } finally { f.cleanup(); }
  assert.equal(f.window.timers, 0); assert.equal(f.window.listeners, 0);
});

test("actual Page JSX Step forwarder keeps identity and preserves arguments and return values from the latest closure", () => {
  const page = pageFixture(), calls = [], results = [];
  const render = label => {
    const result = { label }; results.push(result);
    return page.render(() => assert.fail("Step must not dispatch a keyboard intent"), () => undefined,
      (...args) => { calls.push({ label, args }); return result; });
  };
  const first = render("first"), second = render("second"), gesture = Object.freeze({ physical: "same-gesture" });
  const pointerHeld = () => true;
  assert.equal(first.onViewportDirectionStep, second.onViewportDirectionStep);
  assert.equal(first.onViewportDirectionStep(299, 619, "walk", gesture, pointerHeld), results[1]);
  assert.deepEqual(calls, [{ label: "second", args: [299, 619, "walk", gesture, pointerHeld] }]);
  assert.equal(calls[0].args[3], gesture, "the exact physical fishing-gesture proof is forwarded without reconstruction");
  const third = render("third");
  assert.equal(second.onViewportDirectionStep, third.onViewportDirectionStep);
  assert.equal(second.onViewportDirectionStep(300, 618, "run"), results[2]);
  assert.deepEqual(calls[1], { label: "third", args: [300, 618, "run"] });
});

function mouseHoldFixture() {
  const mouseEffect = nodes(shell, node => ts.isCallExpression(node) && ts.isIdentifier(node.expression)
    && node.expression.text === "useEffect" && ts.isArrayLiteralExpression(node.arguments[1])
    && node.arguments[1].elements.some(dep => ts.isIdentifier(dep) && dep.text === "onViewportDirectionStep"));
  assert.equal(mouseEffect.length, 1, "extract the actual mouse-hold interval and its Step/Stop dependencies");
  const names = ["dispatchSceneMoveInput", "tileFromScenePoint", "committedViewportPlayerPosition",
    "npcShopBlocksWorldInput", "stopNpcShopWorldInput", "heroBlocksWorldInput"];
  const declarations = nodes(shell, node => ts.isFunctionDeclaration(node) && names.includes(node.name?.text));
  assert.equal(declarations.length, names.length);
  const helpers = declarations.map(node => node.getText(shell)).join("\n");
  const window = clock(), document = Object.assign(clock(), { visibilityState: "visible" });
  const held = { current: null }, flags = { parity: false, shop: false }, secondary = [];
  let cleanup = null, previousDeps = null, installs = 0;
  const scope = {
    window, document, Date: { now: () => window.now }, CRYSTAL_MOVE_INPUT_INTERVAL_MS: intervalMs,
    screen: "game", sceneInteractionReady: true, bevyQuestUiCapturesPointer: false,
    heldScenePointerRef: held, worldFishingPhysicalRef: { current: null }, cancelWorldFishingHeldPointer() {},
    heldKeyboardMoveKeysRef: { current: new Set() }, heldKeyboardRunModeRef: { current: false },
    parityUiBlocksGameplay: () => flags.parity,
    heroPointerLeaseRef: { current: null }, heroPointerCallbacksRef: { current: { getBevyHeroInputBlocked: () => false } },
    npcShopPointerCallbacksRef: { current: { getBevyNpcShopInputBlocked: () => flags.shop } },
    latestMoveInputRef: { current: { screen: "game", player: { x: 299, y: 618 }, newMove: false } },
    stageFrameRef: { current: { dataset: { viewportPlayerX: "299", viewportPlayerY: "618" } } },
    VIEWPORT_CELL_WIDTH: 48, VIEWPORT_CELL_HEIGHT: 32, viewportLayout: { offsetX: 10, offsetY: 11 },
    updateSceneCombatPointer() {},
    onViewportTileSecondaryAction: (...args) => secondary.push(args),
    useEffect(setup, deps) {
      if (previousDeps && deps.length === previousDeps.length && deps.every((value, i) => Object.is(value, previousDeps[i]))) return;
      cleanup?.(); previousDeps = [...deps]; installs++; cleanup = setup() ?? null;
    }
  };
  return {
    window, document, held, flags, secondary,
    setNewMove(newMove) { scope.latestMoveInputRef.current.newMove = newMove; },
    render(callbacks) { Object.assign(scope, callbacks); compile(`${helpers}\n${mouseEffect[0].getText(shell)};`, scope); },
    cleanup() { cleanup?.(); cleanup = null; }, get installs() { return installs; }
  };
}

test("50ms Page renders retain the real mouse-hold interval and every 100ms Step calls the latest Page closure", () => {
  const page = pageFixture(), calls = [], f = mouseHoldFixture();
  let rendered = 0;
  const render = () => {
    const revision = ++rendered;
    f.render(page.render(() => assert.fail("mouse hold must not become a keyboard intent"),
      () => calls.push({ revision, stop: true }), (...args) => calls.push({ revision, args })));
  };
  render();
  const gesture = Object.freeze({ physical: "admitted-mouse-hold" });
  // Admission and world-marker ownership have their own real-handler suite;
  // retain an admitted ordinary world hold here to exercise interval lifetime.
  const pointer = { button: 0, sceneX: 480, sceneY: 384, startedAt: f.window.now, dispatched: false, fishingGesture: gesture };
  f.held.current = pointer;
  try {
    assert.equal(f.window.timers, 1); assert.equal(f.window.listeners, 3); assert.equal(f.document.listeners, 1);
    for (let i = 0; i < 40; i++) {
      render(); f.window.advance(50);
      assert.equal(f.installs, 1, "the actual Step/Stop dependencies never reinstall the held timer");
    }
    assert.equal(calls.length, 20, "a two-second hold keeps all twenty 100ms movement opportunities");
    assert.equal(calls[0].revision, 3);
    assert.equal(calls.at(-1).revision, rendered);
    assert(calls.every(call => call.args[3] === gesture && call.args[4]()));
    assert.deepEqual(calls.at(-1).args.slice(0, 4), [299, 619, "walk", gesture]);
    const firstHold = calls.at(-1).args[4];
    assert.equal(f.held.current, pointer); assert.equal(pointer.dispatched, true);
    assert.equal(f.window.timers, 1);
    f.window.emit("mouseup", { type: "mouseup", timeStamp: f.window.now });
    assert.equal(f.held.current, null); assert.deepEqual(calls.at(-1), { revision: rendered, stop: true });
    assert.equal(firstHold(), false);
    const released = calls.length; f.window.advance(200); assert.equal(calls.length, released);
    render();
    f.held.current = { ...pointer, button: 2, startedAt: f.window.now, dispatched: false };
    f.window.advance(100);
    assert.equal(calls.at(-1).revision, rendered);
    assert.deepEqual(calls.at(-1).args.slice(0, 4), [299, 619, "run", undefined]);
    const secondHold = calls.at(-1).args[4];
    assert.equal(secondHold(), true); assert.equal(firstHold(), false, "a new press cannot revive an old one");
    f.flags.parity = true; f.window.advance(100);
    assert.equal(f.held.current, null); assert.deepEqual(calls.at(-1), { revision: rendered, stop: true });
    assert.equal(secondHold(), false);
    f.flags.parity = false;
    const cancelled = calls.length; f.window.advance(200); assert.equal(calls.length, cancelled, "lifting a modal cannot replay a cancelled hold");
    f.held.current = { ...pointer, startedAt: f.window.now, dispatched: false };
    f.window.emit("blur", { type: "blur" });
    assert.equal(f.held.current, null); assert.deepEqual(calls.at(-1), { revision: rendered, stop: true });
    const blurred = calls.length; f.window.advance(200); assert.equal(calls.length, blurred);

    f.setNewMove(true);
    f.held.current = { ...pointer, button: 2, startedAt: f.window.now, dispatched: false };
    f.window.advance(100);
    assert.deepEqual(f.secondary, [[299, 619]], "the installed interval uses the latest committed movement mode");
    assert.equal(calls.length, blurred, "a deliberate NewMove route uses the click path");
    f.setNewMove(false);
    for (const event of ["pagehide", "visibilitychange", "cleanup"]) {
      f.held.current = { ...pointer, button: 2, startedAt: f.window.now, dispatched: false };
      f.window.advance(100);
      const liveHold = calls.at(-1).args[4];
      assert.equal(liveHold(), true);
      if (event === "visibilitychange") {
        f.document.visibilityState = "hidden";
        f.document.emit(event, { type: event });
        f.document.visibilityState = "visible";
      } else if (event === "cleanup") f.cleanup();
      else f.window.emit(event, { type: event });
      assert.equal(liveHold(), false, event);
    }
  } finally { f.cleanup(); }
  assert.equal(f.installs, 1); assert.equal(f.window.timers, 0); assert.equal(f.window.listeners, 0);
  assert.equal(f.document.listeners, 0);
});
