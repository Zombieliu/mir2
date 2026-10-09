import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import ts from "typescript";

function load(path, requireLocal) {
  const module = { exports: {} };
  new Function("exports", "module", "require", ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText)(module.exports, module, requireLocal);
  return module.exports;
}
const identity = load("../lib/world-model/item-identity.ts");
const bag = load("../lib/bevy-bag-ui.ts", () => identity);

test("actual hook StrictMode cleanup/restart retires sink, monotonic owner, frame and pointer subscriptions", () => {
  const previousWindow = globalThis.window, previousDocument = globalThis.document;
  const listeners = new Map(), timers = new Map(); let timer = 0;
  globalThis.window = {
    setInterval(fn) { timers.set(++timer, fn); return timer; }, clearInterval(id) { timers.delete(id); },
    addEventListener(name, fn) { listeners.set(name, fn); }, removeEventListener(name, fn) { if (listeners.get(name) === fn) listeners.delete(name); },
  };
  globalThis.document = {
    addEventListener(name, fn) { listeners.set(name, fn); }, removeEventListener(name, fn) { if (listeners.get(name) === fn) listeners.delete(name); },
  };
  try {
    const refs = []; let refIndex = 0, effect, state;
    const react = { useRef(value) { return refs[refIndex++] ??= { current: value }; },
      useEffect(fn) { effect = fn; }, useState(value) { state ??= value; return [state, (value) => { state = typeof value === "function" ? value(state) : value; }]; } };
    const { useBevyBagUi } = load("../lib/use-bevy-bag-ui.ts", (id) => id === "react" ? react : bag);
    const input = { connectionGeneration: 1, sessionGeneration: 2, ownerRevision: 0, eligible: true,
      bagOpen: true, page: "bag1", presentation: { logicalWidth: 1024, logicalHeight: 768, stageCssScale: 1, touch: false },
      model: { capacity: 46, gold: 200, items: [] }, player: {}, blockedUniqueIds: [] };
    let snapshot, status, sink;
    const owners = [];
    const runtime = {
      getMir2BagUiAbiVersion: () => 1,
      setMir2BagUiSnapshot(json) { snapshot = JSON.parse(json); return true; },
      getMir2BagUiStatus: () => JSON.stringify(status ?? null), setMir2BagUiIntentSink(fn) { sink = fn; },
      clearMir2BagUiIntentSink() { sink = null; }, setMir2BagUiPointerEdge: () => true,
    };
    function echo() {
      status = { ...snapshot, frame: (status?.frame ?? 0) + 1, ready: true,
        appliedRevision: snapshot.revision, appliedModelRevision: snapshot.modelRevision,
        appliedPresentationRevision: snapshot.presentationRevision, inputRegions: [{ left: 400, top: 50, width: 316, height: 236 }], error: null };
    }
    useBevyBagUi({ requested: true, runtimeGeneration: 1, runtimeRef: { current: runtime }, read: () => input,
      onOwner(owner, run, revision) { owners.push({ owner, run, revision }); input.ownerRevision = revision; },
      onIntent: () => ({ accepted: true }),
    });
    let cleanup = effect();
    const tick = () => [...timers.values()][0]();
    echo(); tick(); echo(); tick(); assert.equal(state.active, true);
    const oldSink = sink;
    const old = { ...snapshot, intentSequence: 1, type: "close" };
    const revision = input.ownerRevision;
    cleanup();
    assert.equal(state.active, false); assert.equal(timers.size, 0); assert.equal(listeners.size, 0);
    assert.equal(JSON.parse(oldSink(JSON.stringify(old))).accepted, false);
    assert.ok(input.ownerRevision > revision);
    cleanup = effect();
    assert.equal(timers.size, 1); assert.equal(listeners.size, 2);
    assert.notEqual(snapshot.runGeneration, old.runGeneration);
    echo(); tick(); echo(); tick(); assert.equal(state.active, true);
    assert.equal(JSON.parse(sink(JSON.stringify(old))).accepted, false);
    input.eligible = false; listeners.get("blur")();
    tick(); echo(); tick(); assert.equal(state.active, false, "blurred host cannot reacquire merely because frames advance");
    input.eligible = true; tick(); echo(); tick(); echo(); tick(); assert.equal(state.active, true);
    cleanup();
  } finally {
    if (previousWindow === undefined) delete globalThis.window; else globalThis.window = previousWindow;
    if (previousDocument === undefined) delete globalThis.document; else globalThis.document = previousDocument;
  }
});
