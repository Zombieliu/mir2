import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const sourceUrl = new URL("../lib/use-window-keydown.ts", import.meta.url);
const compiled = ts.transpileModule(readFileSync(sourceUrl, "utf8"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, strict: true },
  fileName: sourceUrl.pathname,
  reportDiagnostics: true,
});
assert.deepEqual((compiled.diagnostics ?? []).filter(row => row.category === ts.DiagnosticCategory.Error), []);

function actualHook(react, window) {
  const module = { exports: {} };
  new Function("exports", "module", "require", "window", compiled.outputText)(
    module.exports, module, name => {
      assert.equal(name, "react");
      return react;
    }, window,
  );
  return module.exports.useWindowKeydown;
}

// One ancestor's capture phase, following DOM's cloned listener list and removed
// flag. Node's EventTarget is unsuitable: newly added listeners may run there.
// Explicit callback checkpoints model a browser-fired event, without claiming
// these CPU fixtures are trusted input or a real browser smoke.
function captureWindow() {
  const listeners = [], microtasks = [], additions = [], removals = [];
  return {
    additions, removals,
    addEventListener(type, listener, capture) {
      additions.push({ type, listener, capture });
      if (!listeners.some(row => !row.removed && row.type === type && row.listener === listener && row.capture === capture))
        listeners.push({ type, listener, capture, removed: false });
    },
    removeEventListener(type, listener, capture) {
      removals.push({ type, listener, capture });
      const row = listeners.find(row => !row.removed && row.type === type && row.listener === listener && row.capture === capture);
      if (row) row.removed = true;
    },
    queueMicrotask(callback) { microtasks.push(callback); },
    dispatch(event) {
      for (const row of [...listeners]) {
        if (row.removed || row.type !== event.type || row.capture !== true) continue;
        row.listener.call(this, event);
        while (microtasks.length) microtasks.shift()();
      }
    },
    get listenerCount() { return listeners.filter(row => !row.removed).length; },
  };
}

// The actual transpiled hook runs against persistent ref slots and separate
// render/commit phases. Cleanup precedes setup for changed layout effects;
// an abandoned render never runs its layout effects.
function fixture(window = captureWindow(), loadHook = actualHook) {
  const slots = [];
  let cursor = 0, work = null, unmounted = false;
  const react = {
    useRef(initial) {
      const index = cursor++;
      slots[index] ??= { kind: "ref", value: { current: initial } };
      assert.equal(slots[index].kind, "ref");
      return slots[index].value;
    },
    useLayoutEffect(setup, deps) {
      const index = cursor++;
      assert.ok(work);
      work.push({ index, setup, deps: deps && [...deps] });
    },
  };
  const hook = loadHook(react, window);
  return {
    window,
    render(handler) {
      assert.equal(unmounted, false);
      cursor = 0; work = [];
      hook(handler);
    },
    commit() {
      assert.equal(unmounted, false);
      assert.ok(work, "commit requires a pending render");
      const changed = work.filter(effect => {
        const old = slots[effect.index];
        return !old || !effect.deps || !old.deps || old.deps.length !== effect.deps.length
          || !effect.deps.every((value, index) => Object.is(value, old.deps[index]));
      });
      work = null;
      for (const effect of changed) slots[effect.index]?.cleanup?.();
      for (const effect of changed) slots[effect.index] = {
        kind: "effect", deps: effect.deps, cleanup: effect.setup(),
      };
    },
    discard() { work = null; },
    unmount() {
      assert.equal(unmounted, false);
      work = null; unmounted = true;
      for (const slot of slots) if (slot?.kind === "effect") slot.cleanup?.();
    },
  };
}

const key = () => ({ type: "keydown", key: "x", code: "KeyX", altKey: true, repeat: false });

test("the actual hook exposes only committed handlers across pending and abandoned renders", () => {
  const x = fixture(), calls = [], event = key();
  const first = value => calls.push(["first", value]), second = value => calls.push(["second", value]);
  x.render(first);
  assert.equal(x.window.listenerCount, 0, "render cannot subscribe or expose its handler");
  x.commit(); x.window.dispatch(event);
  x.render(second); x.window.dispatch(event);
  x.discard(); x.window.dispatch(event);
  assert.deepEqual(calls, [["first", event], ["first", event], ["first", event]]);
  x.render(second); x.commit(); x.window.dispatch(event);
  assert.deepEqual(calls.at(-1), ["second", event]);
  for (let index = 0; index < 30; index++) {
    x.render(value => calls.push([index, value])); x.commit();
  }
  x.window.dispatch(event);
  assert.deepEqual(calls.at(-1), [29, event]);
  assert.equal(x.window.additions.length, 1);
  assert.equal(x.window.removals.length, 0);
  x.unmount();
});

test("initial null, entry and leaving keep one capture subscription and null dispatches nothing", () => {
  const x = fixture(), events = [], handler = event => events.push(event), event = key();
  x.render(null); x.commit(); x.window.dispatch(event);
  assert.deepEqual(events, []);
  assert.equal(x.window.listenerCount, 1);
  x.render(handler); x.window.dispatch(event);
  assert.deepEqual(events, [], "an uncommitted entry cannot activate the handler");
  x.commit(); x.window.dispatch(event);
  assert.deepEqual(events, [event]);
  x.render(null); x.commit(); x.window.dispatch(event);
  assert.deepEqual(events, [event], "committed screen null cannot call an older game handler");
  assert.equal(x.window.additions.length, 1);
  assert.equal(x.window.additions[0].type, "keydown");
  assert.equal(x.window.additions[0].capture, true);
  assert.equal(x.window.removals.length, 0);
  x.unmount();
});

test("unmount removes the exact capture listener and clears an abandoned next handler", () => {
  const x = fixture(), calls = [], event = key();
  x.render(() => calls.push("committed")); x.commit();
  const installed = x.window.additions[0];
  x.render(() => calls.push("uncommitted"));
  x.unmount(); x.window.dispatch(event);
  installed.listener(event);
  assert.deepEqual(calls, [], "even a retained callback cannot invoke a retired committed handler");
  assert.equal(x.window.listenerCount, 0);
  assert.deepEqual(x.window.removals, [installed]);
});

function runCommitDuringDispatch(loadHook) {
  const window = captureWindow(), calls = [], event = key();
  let next = false, x;
  const latest = value => calls.push(["new", value]);
  window.addEventListener("keydown", () => {
    if (next) return;
    next = true;
    window.queueMicrotask(() => { x.render(latest); x.commit(); });
  }, true);
  x = fixture(window, loadHook);
  x.render(value => calls.push(["old", value])); x.commit();
  window.dispatch(event);
  return { x, window, calls, event };
}

test("negative control: rebinding an effect during an earlier listener loses that same dispatch", () => {
  // The former subscription pattern, rather than a replacement implementation
  // of the new hook. This proves the event fixture can reproduce the lost edge.
  const x = runCommitDuringDispatch((react, window) => handler => {
    react.useLayoutEffect(() => {
      if (!handler) return;
      window.addEventListener("keydown", handler, true);
      return () => window.removeEventListener("keydown", handler, true);
    }, [handler]);
  });
  assert.deepEqual(x.calls, [], "removed old callback and newly added callback both miss this capture snapshot");
  x.window.dispatch(x.event);
  assert.deepEqual(x.calls, [["new", x.event]], "the next key proves the replacement really was installed");
  x.x.unmount();
});

test("the actual stable listener forwards once after a preceding same-target listener commits a new handler", () => {
  const x = runCommitDuringDispatch(actualHook);
  assert.deepEqual(x.calls, [["new", x.event]]);
  assert.equal(x.window.additions.length, 2, "the predecessor plus one stable subscription");
  assert.equal(x.window.removals.length, 0);
  x.window.dispatch(x.event);
  assert.deepEqual(x.calls, [["new", x.event], ["new", x.event]]);
  x.x.unmount();
});

test("the actual hook forwards the original key and modifiers without consuming or filtering it", () => {
  const x = fixture(), calls = [];
  let allowed = false;
  const event = { ...key(), repeat: true, defaultPrevented: false,
    preventDefault() { this.defaultPrevented = true; }, stopImmediatePropagation() { assert.fail("unexpected interception"); } };
  x.render(value => {
    assert.equal(value, event);
    if (allowed) { calls.push(value); value.preventDefault(); }
  }); x.commit(); x.window.dispatch(event);
  assert.deepEqual(calls, []); assert.equal(event.defaultPrevented, false);
  allowed = true; x.window.dispatch(event);
  assert.deepEqual(calls, [event]); assert.equal(event.defaultPrevented, true);
  assert.equal(x.window.additions[0].capture, true);
  x.unmount();
});
