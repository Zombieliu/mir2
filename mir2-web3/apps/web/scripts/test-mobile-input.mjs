import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

const inputPath = new URL("../app/components/original-client-mobile-input.ts", import.meta.url);
const compiled = ts.transpileModule(readFileSync(inputPath, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  reportDiagnostics: true,
});
assert.deepEqual((compiled.diagnostics ?? []).filter((item) => item.category === ts.DiagnosticCategory.Error), []);
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);
const { mir2MobileMoveIntentFromNippleData: fromNipple, mir2MobileMoveIntentFromVector: fromScreen } = module.exports;

test("the mobile component calls the tested nipplejs adapter", () => {
  const controls = readFileSync(new URL("../app/components/original-client-mobile-controls.tsx", import.meta.url), "utf8");
  assert.match(controls, /mir2MobileMoveIntentFromNippleData\(\s*data,/);
});

test("nipplejs Y-up vectors map to all eight screen-space directions", () => {
  for (const [x, y, direction] of [
    [0, 0.5, "Up"], [0.5, 0.5, "UpRight"], [0.5, 0, "Right"], [0.5, -0.5, "DownRight"],
    [0, -0.5, "Down"], [-0.5, -0.5, "DownLeft"], [-0.5, 0, "Left"], [-0.5, 0.5, "UpLeft"],
  ]) {
    assert.equal(fromNipple({ vector: { x, y }, force: 0.1 }, false)?.direction, direction);
  }
  assert.equal(fromScreen({ x: 0, y: 0.5 }, false)?.direction, "Down", "the generic screen converter stays Y-down");
});

test("missing or invalid nipple axes fail closed; force is optional", () => {
  for (const data of [null, undefined, {}, { vector: {} }, { vector: { x: 0.5 } },
    { vector: { y: 0.5 } }, { vector: { x: "0.5", y: 0.5 } },
    { vector: { x: 0.5, y: Number.NaN } }, { vector: { x: Infinity, y: 0.5 } }]) {
    assert.equal(fromNipple(data, false), null);
  }
  assert.equal(fromNipple({ vector: { x: 0, y: 0.5 } }, false)?.mode, "walk");
  assert.equal(fromNipple({ vector: { x: 0, y: 0.5 }, force: Infinity }, false)?.mode, "walk");
});

test("the adapter retains dead zone, release hysteresis, run force and direction hysteresis", () => {
  const nipple = (x, y, force = 0) => ({ vector: { x, y }, force });
  assert.equal(fromNipple(nipple(0, 0.2), false), null, "entry dead zone");
  assert.equal(fromNipple(nipple(0, 0.2), false, "Up")?.direction, "Up", "held release threshold");
  assert.equal(fromNipple(nipple(0, 0.17), false, "Up"), null, "exit dead zone");
  assert.equal(fromNipple(nipple(0, 0.5), false)?.mode, "walk");
  assert.equal(fromNipple(nipple(0, 0.5, 0.8), false)?.mode, "run", "force threshold");
  assert.equal(fromNipple(nipple(0, 0.5), true)?.mode, "run", "run lock");
  const nearBoundary = Math.PI / 6;
  const pastBoundary = Math.PI / 3;
  assert.equal(fromNipple(nipple(Math.cos(nearBoundary) * 0.5, Math.sin(nearBoundary) * 0.5), false, "Right")?.direction,
    "Right", "adjacent octant stays held near the boundary");
  assert.equal(fromNipple(nipple(Math.cos(pastBoundary) * 0.5, Math.sin(pastBoundary) * 0.5), false, "Right")?.direction,
    "UpRight", "the direction changes beyond hysteresis");
});

test("the registered joystick move handler stops once on dead-zone, malformed and missing-vector transitions", async () => {
  const source = readFileSync(new URL("../app/components/original-client-mobile-controls.tsx", import.meta.url), "utf8");
  const compiledControls = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX },
    reportDiagnostics: true,
  });
  assert.deepEqual((compiledControls.diagnostics ?? []).filter((item) => item.category === ts.DiagnosticCategory.Error), []);
  const handlers = new Map();
  const manager = {
    on: (event, handler) => handlers.set(event, handler),
    off: () => {}, destroy: () => {}, reposition: () => {},
  };
  let refIndex = 0;
  const react = {
    memo: (component) => component,
    useCallback: (callback) => callback,
    useEffect: (effect) => { effect(); },
    useLayoutEffect: (effect) => { effect(); },
    useMemo: (factory) => factory(),
    useRef: (initial) => ({ current: refIndex++ === 0 ? {} : initial }),
    useState: (initial) => [initial, () => {}],
  };
  const dependencies = {
    react,
    "react/jsx-runtime": { jsx: () => ({}), jsxs: () => ({}) },
    nipplejs: { __esModule: true, default: { setLogLevel: () => {}, create: () => manager } },
    "../../lib/tutorial-steps": { TUTORIAL_CONTROL_EVENT: "tutorial-control", TUTORIAL_STEP_EVENT: "tutorial-step" },
    "./original-client-scene-layout": { CRYSTAL_MOVE_INPUT_INTERVAL_MS: 100 },
    "./original-client-mobile-input": module.exports,
  };
  const controlsModule = { exports: {} };
  new Function("exports", "module", "require", compiledControls.outputText)(
    controlsModule.exports, controlsModule, (id) => {
      if (Object.hasOwn(dependencies, id)) return dependencies[id];
      throw new Error(`Unexpected controls dependency: ${id}`);
    },
  );
  const directions = [];
  let stops = 0;
  const priorWindow = globalThis.window;
  const priorDocument = globalThis.document;
  const priorCustomEvent = globalThis.CustomEvent;
  globalThis.window = {
    addEventListener: () => {}, removeEventListener: () => {}, dispatchEvent: () => {},
    setInterval: () => 1, clearInterval: () => {}, setTimeout: () => 2, clearTimeout: () => {},
  };
  globalThis.document = { hidden: false, addEventListener: () => {}, removeEventListener: () => {} };
  globalThis.CustomEvent = class { constructor(type, options) { this.type = type; this.detail = options?.detail; } };
  try {
    controlsModule.exports.OriginalClientMobileControls({
      enabled: true, forceVisible: true,
      t: (_key, _args, fallback) => fallback,
      world: { groundDrops: [], beltItems: [], knownSkills: [] }, player: null, selectedEntity: null,
      onDirectionIntent: (direction, mode) => directions.push({ direction, mode }),
      onDirectionStop: () => { stops += 1; },
      onPrimaryTargetAction: () => {}, onApproachTarget: () => {}, onPickGroundDrop: () => {},
      onToggleInventory: () => {}, onToggleCharacter: () => {}, onToggleQuestLog: () => {},
      onCastSkill: () => {}, onUseItem: () => {},
    });
    await new Promise(setImmediate);
    const start = handlers.get("start");
    const move = handlers.get("move");
    assert.equal(typeof start, "function", "the real component registers a nipplejs start handler");
    assert.equal(typeof move, "function", "the real component registers a nipplejs move handler");
    start();
    move({}, { vector: { x: 0, y: 0.6 }, force: 0.1 });
    assert.deepEqual(directions.map((entry) => entry.direction), ["Up"]);
    move({}, { vector: { x: 0, y: 0.1 }, force: 0.1 });
    assert.equal(stops, 1, "dead zone clears a held movement immediately");
    move({}, { vector: { x: 0, y: 0.1 } });
    assert.equal(stops, 1, "already-null intent does not stop twice");
    move({}, { vector: { x: 0, y: 0.6 } });
    move({}, { vector: { x: "bad", y: 0.6 } });
    assert.equal(stops, 2, "malformed axis clears the new movement");
    move({}, { vector: { x: 0, y: 0.6 } });
    move({}, { force: 0.8 });
    assert.equal(stops, 3, "missing vector clears the new movement");
    move({}, { force: 0.8 });
    assert.equal(stops, 3, "repeated missing vector remains stopped");
    assert.deepEqual(directions.map((entry) => entry.direction), ["Up"],
      "re-entering the same direction preserves the existing dispatch deduplication");
  } finally {
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
    if (priorDocument === undefined) delete globalThis.document;
    else globalThis.document = priorDocument;
    if (priorCustomEvent === undefined) delete globalThis.CustomEvent;
    else globalThis.CustomEvent = priorCustomEvent;
  }
});

test("real More control reports open, hidden and unmounted state before paint", () => {
  const source = readFileSync(new URL("../app/components/original-client-mobile-controls.tsx", import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX,
  } }).outputText;
  const values = [], announced = [];
  const priorWindow = globalThis.window;
  globalThis.window = { __mir2MobileControls: undefined };
  let cursor = 0, effects = [], cleanups = [];
  const react = {
    memo: (component) => component,
    useCallback: (callback) => callback,
    useEffect: () => {},
    useLayoutEffect: (effect) => effects.push(effect),
    useMemo: (factory) => factory(),
    useRef: (initial) => ({ current: initial }),
    useState(initial) {
      const index = cursor++;
      if (!(index in values)) values[index] = initial;
      return [values[index], (next) => { values[index] = typeof next === "function" ? next(values[index]) : next; }];
    },
  };
  const element = (type, props) => ({ type, props });
  const dependencies = {
    react,
    "react/jsx-runtime": { jsx: element, jsxs: element, Fragment: Symbol("fragment") },
    "../../lib/tutorial-steps": { TUTORIAL_CONTROL_EVENT: "tutorial-control", TUTORIAL_STEP_EVENT: "tutorial-step" },
    "./original-client-scene-layout": { CRYSTAL_MOVE_INPUT_INTERVAL_MS: 100 },
    "./original-client-mobile-input": module.exports,
  };
  const controlsModule = { exports: {} };
  new Function("exports", "module", "require", code)(controlsModule.exports, controlsModule, (id) => {
    if (Object.hasOwn(dependencies, id)) return dependencies[id];
    throw new Error(`Unexpected controls dependency: ${id}`);
  });
  const props = { enabled: true, forceVisible: true,
    t: (_key, _args, fallback) => fallback,
    world: { groundDrops: [], beltItems: [], knownSkills: [] }, player: null, selectedEntity: null,
    onDirectionIntent() {}, onDirectionStop() {}, onPrimaryTargetAction() {}, onApproachTarget() {},
    onPickGroundDrop() {}, onToggleInventory() {}, onToggleCharacter() {}, onToggleQuestLog() {},
    onCastSkill() {}, onUseItem() {}, onSecondaryOpenChange: (open) => announced.push(open) };
  function render(enabled) {
    for (const cleanup of cleanups) cleanup();
    cursor = 0;
    effects = [];
    const tree = controlsModule.exports.OriginalClientMobileControls({ ...props, enabled });
    cleanups = effects.map((effect) => effect()).filter((cleanup) => typeof cleanup === "function");
    return tree;
  }
  function findMore(node) {
    if (!node || typeof node !== "object") return null;
    if (node.props?.className?.includes("mir-mobile-utility-toggle")) return node;
    const children = node.props?.children;
    for (const child of Array.isArray(children) ? children : [children]) {
      const found = findMore(child);
      if (found) return found;
    }
    return null;
  }
  try {
    const closed = render(true);
    assert.equal(announced.at(-1), false);
    const more = findMore(closed);
    assert.ok(more, "the actual rendered control exposes More");
    more.props.onClick();
    render(true);
    assert.equal(announced.at(-1), true, "opening More yields the HP image in the same layout phase");
    render(false);
    assert.equal(announced.at(-1), false, "a hidden control cannot retain More ownership");
    render(true);
    assert.equal(announced.at(-1), true, "the still-open menu is reported when controls become visible again");
    for (const cleanup of cleanups) cleanup();
    assert.equal(announced.at(-1), false, "unmount releases the local menu gate");
  } finally {
    if (priorWindow === undefined) delete globalThis.window;
    else globalThis.window = priorWindow;
  }
});

function mobileLifecycleHarness({ deferNippleImport = false } = {}) {
  const source = readFileSync(new URL("../app/components/original-client-mobile-controls.tsx", import.meta.url), "utf8");
  const code = ts.transpileModule(source, { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX,
  } }).outputText;
  const prior = { window: globalThis.window, document: globalThis.document, CustomEvent: globalThis.CustomEvent, now: Date.now };
  const listeners = (target) => ({
    addEventListener(type, callback) {
      const set = target.get(type) ?? new Set();
      set.add(callback);
      target.set(type, set);
    },
    removeEventListener(type, callback) { target.get(type)?.delete(callback); },
    dispatchEvent(event) { for (const callback of [...(target.get(event.type) ?? [])]) callback(event); },
  });
  const windowListeners = new Map(), documentListeners = new Map();
  const intervals = new Map(), timeouts = new Map();
  let timerId = 0, clock = 1_000;
  globalThis.window = {
    ...listeners(windowListeners),
    setInterval(callback) { const id = ++timerId; intervals.set(id, callback); return id; },
    clearInterval(id) { intervals.delete(id); },
    setTimeout(callback) { const id = ++timerId; timeouts.set(id, callback); return id; },
    clearTimeout(id) { timeouts.delete(id); },
  };
  globalThis.document = { hidden: false, ...listeners(documentListeners) };
  globalThis.CustomEvent = class { constructor(type, options) { this.type = type; this.detail = options?.detail; } };
  Date.now = () => clock;

  const managers = [];
  const nipple = { setLogLevel() {}, create() {
    const handlers = new Map();
    const manager = {
      handlers, destroyed: false, offCount: 0,
      on(type, callback) { handlers.set(type, callback); },
      off(type, callback) { if (handlers.get(type) === callback) handlers.delete(type); this.offCount += 1; },
      destroy() { this.destroyed = true; }, reposition() {},
      emit(type, ...args) { handlers.get(type)?.(...args); },
    };
    managers.push(manager);
    return manager;
  } };
  let resolveImport;
  const nippleDependency = deferNippleImport
    ? new Promise((resolve) => { resolveImport = () => resolve({ default: nipple }); })
    : { default: nipple };

  const slots = [], effects = new Map();
  let hookCursor = 0, pendingEffects = [], dirty = false, mounted = true, tree;
  const sameDeps = (a, b) => a && b && a.length === b.length && a.every((value, index) => Object.is(value, b[index]));
  const react = {
    memo: (component) => component,
    useRef(initial) { const index = hookCursor++; return slots[index] ??= { current: initial }; },
    useState(initial) {
      const index = hookCursor++;
      if (!(index in slots)) slots[index] = initial;
      return [slots[index], (next) => {
        const value = typeof next === "function" ? next(slots[index]) : next;
        if (!Object.is(value, slots[index])) { slots[index] = value; dirty = true; }
      }];
    },
    useMemo(factory, deps) {
      const index = hookCursor++;
      if (!slots[index] || !sameDeps(slots[index].deps, deps)) slots[index] = { deps, value: factory() };
      return slots[index].value;
    },
    useCallback(callback, deps) {
      const index = hookCursor++;
      if (!slots[index] || !sameDeps(slots[index].deps, deps)) slots[index] = { deps, value: callback };
      return slots[index].value;
    },
    useEffect(callback, deps) { pendingEffects.push({ index: hookCursor++, callback, deps, kind: "passive" }); },
    useLayoutEffect(callback, deps) { pendingEffects.push({ index: hookCursor++, callback, deps, kind: "layout" }); },
  };
  const element = (type, props) => ({ type, props });
  const dependencies = {
    react,
    "react/jsx-runtime": { jsx: element, jsxs: element, Fragment: Symbol("fragment") },
    nipplejs: nippleDependency,
    "../../lib/tutorial-steps": { TUTORIAL_CONTROL_EVENT: "tutorial-control", TUTORIAL_STEP_EVENT: "tutorial-step" },
    "./original-client-scene-layout": { CRYSTAL_MOVE_INPUT_INTERVAL_MS: 100 },
    "./original-client-mobile-input": module.exports,
  };
  const controlsModule = { exports: {} };
  new Function("exports", "module", "require", code)(controlsModule.exports, controlsModule, (id) => {
    if (Object.hasOwn(dependencies, id)) return dependencies[id];
    throw new Error(`Unexpected controls dependency: ${id}`);
  });
  const directions = [], stops = [];
  let props = {
    enabled: true, forceVisible: true,
    t: (_key, _args, fallback) => fallback,
    world: { groundDrops: [], beltItems: [], knownSkills: [] }, player: null, selectedEntity: null,
    onDirectionIntent: (direction, mode) => directions.push({ direction, mode }),
    onDirectionStop: () => stops.push(clock),
    onPrimaryTargetAction() {}, onApproachTarget() {}, onPickGroundDrop() {},
    onToggleInventory() {}, onToggleCharacter() {}, onToggleQuestLog() {},
    onCastSkill() {}, onUseItem() {},
  };
  const findNode = (node, predicate) => {
    if (!node || typeof node !== "object") return null;
    if (predicate(node)) return node;
    const children = node.props?.children;
    for (const child of Array.isArray(children) ? children : [children]) {
      const found = findNode(child, predicate);
      if (found) return found;
    }
    return null;
  };
  function render() {
    hookCursor = 0;
    pendingEffects = [];
    tree = controlsModule.exports.OriginalClientMobileControls(props);
    const zone = findNode(tree, (node) => node.props?.["data-mobile-joystick"]);
    if (zone?.props.ref && !zone.props.ref.current) zone.props.ref.current = {};
    const changed = pendingEffects.filter((next) => !sameDeps(effects.get(next.index)?.deps, next.deps));
    for (const next of changed) effects.get(next.index)?.cleanup?.();
    for (const kind of ["layout", "passive"]) {
      for (const next of changed.filter((entry) => entry.kind === kind)) {
        effects.set(next.index, { deps: next.deps, cleanup: next.callback() });
      }
    }
  }
  function flush() {
    for (let i = 0; dirty && i < 10; i += 1) { dirty = false; render(); }
    assert.equal(dirty, false, "React mock settled");
  }
  render(); flush();
  return {
    managers, directions, stops, windowListeners, documentListeners, intervals, timeouts,
    get debug() { return globalThis.window.__mir2MobileControls; },
    get tree() { return tree; },
    get clock() { return clock; },
    advance(ms) { clock += ms; },
    flush,
    async settleImport() { await Promise.resolve(); await Promise.resolve(); flush(); },
    resolveImport() { resolveImport?.(); },
    setEnabled(enabled) { props = { ...props, enabled }; render(); flush(); },
    tick() { for (const callback of [...intervals.values()]) callback(); flush(); },
    runTimeouts() {
      for (const [id, callback] of [...timeouts]) {
        if (timeouts.delete(id)) callback();
      }
      flush();
    },
    event(type) { globalThis.window.dispatchEvent({ type }); flush(); },
    visibility(hidden) { globalThis.document.hidden = hidden; globalThis.document.dispatchEvent({ type: "visibilitychange" }); flush(); },
    findNode: (predicate) => findNode(tree, predicate),
    unmount() {
      if (!mounted) return;
      mounted = false;
      for (const effect of effects.values()) effect.cleanup?.();
      effects.clear();
    },
    restore() {
      if (mounted) this.unmount();
      Date.now = prior.now;
      for (const key of ["window", "document", "CustomEvent"]) {
        if (prior[key] === undefined) delete globalThis[key];
        else globalThis[key] = prior[key];
      }
    },
  };
}

const upMove = { vector: { x: 0, y: 0.6 }, force: 0.1 };

test("blur, hidden and pagehide retire a held real gesture once, and foreground cannot replay it", async () => {
  const h = mobileLifecycleHarness();
  try {
    await h.settleImport();
    const old = h.managers[0];
    const staleStart = old.handlers.get("start"), staleMove = old.handlers.get("move"), staleEnd = old.handlers.get("end");
    assert.equal(typeof staleStart, "function");
    assert.equal(typeof staleMove, "function");
    assert.equal(typeof staleEnd, "function");
    old.emit("start"); old.emit("move", {}, upMove);
    assert.deepEqual(h.directions.map((item) => item.direction), ["Up"]);
    h.event("blur");
    assert.equal(h.stops.length, 1);
    assert.equal(h.debug.active, false);
    assert.equal(h.debug.lastIntent, null);
    assert.equal(h.debug.lastSentAt, null);
    assert.equal(old.destroyed, true);
    h.visibility(true); h.event("pagehide"); h.advance(500); h.tick();
    assert.equal(h.stops.length, 1, "overlapping retirement is idempotent");
    assert.equal(h.directions.length, 1, "hidden timer cannot redispatch");
    h.visibility(false); h.event("pageshow"); h.event("focus");
    await h.settleImport();
    h.advance(500); h.tick();
    assert.equal(h.directions.length, 1, "foreground alone cannot resume held direction");
    staleStart(); staleMove({}, upMove); staleEnd();
    assert.equal(h.directions.length, 1);
    const current = h.managers.at(-1);
    assert.notEqual(current, old);
    current.emit("move", {}, upMove);
    assert.equal(h.directions.length, 1, "move without a real new start is ignored");
    current.emit("start"); current.emit("move", {}, upMove);
    assert.equal(h.directions.length, 2);
    staleMove({}, { vector: { x: 0.6, y: 0 }, force: 0.1 });
    staleEnd();
    assert.equal(h.debug.active, true, "old end cannot cancel the new owner");
    assert.equal(h.debug.lastIntent.direction, "Up", "old move cannot replace the new owner");
  } finally { h.restore(); }
});

test("hidden and pagehide independently retire movement without a touch end", async () => {
  for (const retire of ["hidden", "pagehide"]) {
    const h = mobileLifecycleHarness();
    try {
      await h.settleImport();
      h.managers[0].emit("start"); h.managers[0].emit("move", {}, upMove);
      if (retire === "hidden") h.visibility(true);
      else h.event("pagehide");
      h.advance(500); h.tick();
      assert.equal(h.stops.length, 1, `${retire} stops the existing queue`);
      assert.equal(h.debug.active, false);
      assert.equal(h.directions.length, 1);
    } finally { h.restore(); }
  }
});

test("disabled transition and unmount remove listeners, interval and manager ownership", async () => {
  const h = mobileLifecycleHarness();
  try {
    await h.settleImport();
    const manager = h.managers[0];
    const lateMove = manager.handlers.get("move");
    manager.emit("start"); manager.emit("move", {}, upMove);
    h.setEnabled(false);
    assert.equal(h.stops.length, 1);
    assert.equal(h.debug.active, false);
    assert.equal(h.debug.lastSentAt, null);
    assert.equal(manager.destroyed, true);
    assert.equal(manager.offCount, 3);
    assert.equal(h.intervals.size, 0);
    lateMove({}, upMove);
    assert.equal(h.directions.length, 1);
    h.unmount();
    assert.equal(h.stops.length, 1);
    assert.equal(h.windowListeners.get("blur")?.size, 0);
    assert.equal(h.documentListeners.get("visibilitychange")?.size, 0);
    assert.equal(h.debug, undefined);
  } finally { h.restore(); }
});

test("active unmount retires synchronously and a later enable requires a new manager start", async () => {
  const h = mobileLifecycleHarness();
  try {
    await h.settleImport();
    const old = h.managers[0];
    const oldMove = old.handlers.get("move"), oldEnd = old.handlers.get("end");
    old.emit("start"); old.emit("move", {}, upMove);
    h.setEnabled(false);
    h.setEnabled(true); await h.settleImport();
    assert.equal(h.managers.length, 2);
    const current = h.managers[1];
    current.emit("move", {}, upMove);
    assert.equal(h.directions.length, 1);
    current.emit("start"); current.emit("move", {}, upMove);
    assert.equal(h.directions.length, 2);
    oldMove({}, upMove); oldEnd();
    assert.equal(h.debug.active, true);
    h.unmount();
    assert.equal(h.stops.length, 2, "active unmount sends the existing stop callback");
    assert.equal(current.destroyed, true);
    assert.equal(h.intervals.size, 0);
    assert.equal(h.debug, undefined);
  } finally { h.restore(); }
});

test("a deferred nipple import cannot create a manager after disable or unmount", async () => {
  for (const exit of ["disable", "unmount"]) {
    const h = mobileLifecycleHarness({ deferNippleImport: true });
    try {
      await Promise.resolve();
      if (exit === "disable") h.setEnabled(false);
      else h.unmount();
      h.resolveImport(); await h.settleImport();
      assert.equal(h.managers.length, 0, `${exit} fences late import completion`);
      assert.equal(h.directions.length, 0);
    } finally { h.restore(); }
  }
});

test("existing debug held dispatch retires and cannot replay on hidden tick or foreground", async () => {
  const h = mobileLifecycleHarness();
  try {
    await h.settleImport();
    assert.equal(h.debug.dispatchDirection("Right", "walk"), true);
    assert.equal(h.debug.active, true);
    const staleDispatch = h.debug.dispatchDirection;
    h.visibility(true); h.advance(500); h.tick();
    assert.equal(h.debug.active, false);
    assert.equal(h.directions.length, 1);
    h.visibility(false); await h.settleImport(); h.advance(500); h.tick();
    assert.equal(h.directions.length, 1);
    assert.equal(staleDispatch("Right", "walk"), false);
    assert.equal(h.debug.dispatchDirection("Right", "walk"), false, "fresh real start is required");
    h.managers.at(-1).emit("start");
    assert.equal(h.debug.dispatchDirection("Right", "walk"), true);
  } finally { h.restore(); }
});

test("real registered movement keeps run lock, walk, busy and throttle behavior", async () => {
  const h = mobileLifecycleHarness();
  try {
    await h.settleImport();
    const manager = h.managers[0];
    manager.emit("start"); manager.emit("move", {}, upMove);
    assert.deepEqual(h.directions, [{ direction: "Up", mode: "run" }]);
    h.tick();
    assert.equal(h.directions.length, 1, "same intent is throttled before the interval");
    h.advance(100); h.tick();
    assert.equal(h.directions.length, 2, "held input repeats at the existing interval");
    manager.emit("end");
    const runButton = h.findNode((node) => node.props?.className?.includes("mir-mobile-action wheel run"));
    assert.ok(runButton);
    runButton.props.onClick(); h.flush();
    globalThis.window.__mir2Stage5 = { state: { screen: "game", movementPlan: { active: true } } };
    manager.emit("start"); manager.emit("move", {}, { vector: { x: 0.6, y: 0 }, force: 0.1 });
    assert.equal(h.directions.length, 2, "busy transport blocks the new direction");
    assert.equal(h.debug.movementBusy, true);
    globalThis.window.__mir2Stage5.state.movementPlan = null;
    h.advance(100); h.tick();
    assert.deepEqual(h.directions.at(-1), { direction: "Right", mode: "walk" });
  } finally { h.restore(); }
});

test("retired Run callback cannot replace a real successor component debug owner", async () => {
  const first = mobileLifecycleHarness();
  let successor;
  try {
    await first.settleImport(); first.runTimeouts();
    const run = first.findNode((node) => node.props?.className?.includes("mir-mobile-action wheel run"));
    assert.ok(run);
    run.props.onClick(); first.flush();
    const captured = [...first.timeouts.values()];
    assert.equal(captured.length, 1, "real Run click owns one publication timer");
    first.unmount();
    assert.equal(first.timeouts.size, 0, "unmount cancels the owned timer");
    run.props.onClick();
    assert.equal(first.timeouts.size, 0, "retired Run handler cannot schedule another timer");
    successor = mobileLifecycleHarness();
    await successor.settleImport();
    successor.managers[0].emit("start"); successor.managers[0].emit("move", {}, upMove);
    const owner = successor.debug;
    assert.equal(owner.active, true);
    for (const callback of captured) callback();
    assert.equal(successor.debug, owner, "captured retired callback cannot replace successor view");
    assert.equal(successor.debug.lastIntent.direction, "Up");
    assert.equal(successor.directions.length, 1, "old Run callback cannot dispatch movement");
  } finally { successor?.restore(); first.restore(); }
});

test("captured Run callbacks stay retired across same-instance lifecycle return", async () => {
  for (const boundary of ["blur", "hidden", "pagehide", "disable"]) {
    const h = mobileLifecycleHarness();
    try {
      await h.settleImport(); h.runTimeouts();
      const run = h.findNode((node) => node.props?.className?.includes("mir-mobile-action wheel run"));
      run.props.onClick(); h.flush();
      const captured = [...h.timeouts.values()];
      assert.equal(captured.length, 1);
      if (boundary === "hidden") h.visibility(true);
      else if (boundary === "disable") h.setEnabled(false);
      else h.event(boundary);
      assert.equal(h.timeouts.size, 0, `${boundary} cancels the real Run timer`);
      const retiredView = h.debug;
      assert.equal(retiredView.active, false);
      for (const callback of captured) callback();
      assert.equal(h.debug, retiredView, `${boundary} stale callback cannot change inactive view`);
      if (boundary === "hidden") h.visibility(false);
      else if (boundary === "pagehide") h.event("pageshow");
      else if (boundary === "disable") h.setEnabled(true);
      else h.event("focus");
      await h.settleImport();
      const current = h.managers.at(-1);
      current.emit("start"); current.emit("move", {}, upMove);
      const currentView = h.debug;
      const currentMoves = h.directions.length;
      for (const callback of captured) callback();
      assert.equal(h.debug, currentView, `${boundary} stale callback cannot replace a new same-instance owner`);
      assert.equal(h.directions.length, currentMoves);
    } finally { h.restore(); }
  }
});

test("rapid Run clicks coalesce and the current timer publishes the latest run lock", async () => {
  const h = mobileLifecycleHarness();
  try {
    await h.settleImport(); h.runTimeouts();
    const run = h.findNode((node) => node.props?.className?.includes("mir-mobile-action wheel run"));
    const captured = [];
    for (let i = 0; i < 3; i += 1) {
      run.props.onClick();
      captured.push(...h.timeouts.values());
      assert.equal(h.timeouts.size, 1, "only the latest Run publication remains owned");
    }
    h.flush();
    const priorView = h.debug;
    captured[0](); captured[1]();
    assert.equal(h.debug, priorView, "coalesced callbacks cannot publish");
    h.runTimeouts();
    assert.equal(h.timeouts.size, 0);
    assert.equal(h.debug.runLocked, false, "latest legitimate click is published");
    assert.notEqual(h.debug, priorView);
  } finally { h.restore(); }
});
