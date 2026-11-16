import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function mount() {
  let cursor = 0;
  const slots = [], effects = [], cleanups = [], hosts = [];
  const same = (a, b) => a?.length === b.length && b.every((v, i) => Object.is(v, a[i]));
  const react = {
    useRef(value) { const i = cursor++; return slots[i] ??= { current: value }; },
    useState(value) { const i = cursor++; if (!(i in slots)) slots[i] = value; return [slots[i], next => { slots[i] = typeof next === "function" ? next(slots[i]) : next; }]; },
    useCallback(fn, deps) { const i = cursor++; if (!same(slots[i]?.deps, deps)) slots[i] = { deps, fn }; return slots[i].fn; },
    useEffect(fn, deps) { const i = cursor++; if (!same(slots[i], deps)) { slots[i] = deps; effects.push(() => { cleanups[i]?.(); cleanups[i] = fn(); }); } },
  };
  class BevyBagHost {
    constructor(options) { this.options = options; this.calls = []; this.id = hosts.length + 1; hosts.push(this); }
    tick() {}
    stop() { this.stopped = true; }
    pointerContext() { return { id: this.id, latest: this.options.read() }; }
    pointer(edge) { this.calls.push(edge); return true; }
    allows(token) { return token.id === this.id; }
    yieldToReact() { this.yielded = true; }
  }
  const window = { setInterval: () => 1, clearInterval() {}, addEventListener() {}, removeEventListener() {} };
  const document = { addEventListener() {}, removeEventListener() {} };
  const module = { exports: {} };
  const code = ts.transpileModule(readFileSync(new URL("../lib/use-bevy-bag-ui.ts", import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("require", "exports", "window", "document", code)(name => {
    if (name === "react") return react;
    assert.equal(name, "./bevy-bag-ui"); return { BevyBagHost };
  }, module.exports, window, document);
  return {
    hosts,
    render(options) { cursor = 0; const value = module.exports.useBevyBagUi(options); for (const fn of effects.splice(0)) fn(); return value; },
    unmount() { for (const fn of cleanups) fn?.(); },
  };
}

test("Bag callbacks remain identical through parent renders during a physical gesture", () => {
  const h = mount(), runtimeRef = { current: {} };
  const a = h.render({ requested: true, runtimeGeneration: 1, runtimeRef, read: () => "first" });
  const lease = { pointer: a.pointer, context: a.pointerContext };
  assert(a.pointer({ phase: "down" }));
  const b = h.render({ requested: true, runtimeGeneration: 1, runtimeRef, read: () => "latest" });
  assert.equal(b.pointer, lease.pointer); assert.equal(b.pointerContext, lease.context);
  assert.equal(b.allows, a.allows); assert.equal(b.yieldToReact, a.yieldToReact);
  assert.equal(lease.context().latest, "latest");
  assert(lease.pointer({ phase: "up" }));
  assert.deepEqual(h.hosts[0].calls.map(e => e.phase), ["down", "up"]);
  h.unmount();
});

test("Stable callbacks use the new live host after renderer replacement and stop after unmount", () => {
  const h = mount(), runtimeRef = { current: {} };
  const a = h.render({ requested: true, runtimeGeneration: 1, runtimeRef, read: () => 1 });
  const b = h.render({ requested: true, runtimeGeneration: 2, runtimeRef, read: () => 2 });
  assert(h.hosts[0].stopped); assert.equal(a.pointer, b.pointer);
  assert.equal(a.pointerContext().id, 2); assert(a.allows({ id: 2 })); assert(!a.allows({ id: 1 }));
  a.pointer({ phase: "down" }); assert.equal(h.hosts[0].calls.length, 0); assert.equal(h.hosts[1].calls.length, 1);
  h.unmount(); assert.equal(a.pointerContext(), null); assert.equal(a.pointer({ phase: "up" }), false); assert.equal(a.allows({ id: 2 }), false);
});
