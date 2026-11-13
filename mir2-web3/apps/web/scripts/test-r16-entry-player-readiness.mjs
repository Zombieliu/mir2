import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

// Execute the real body-load effect. Metadata-only startup must not inherit the
// complete, verified pack's decode safety timer and reveal an invisible player.
const file = new URL("../app/original-client-shell.tsx", import.meta.url);
const ast = ts.createSourceFile(file.pathname, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const effects = [];
function visit(node) {
  if (ts.isCallExpression(node) && node.expression.getText(ast) === "useEffect" &&
      node.arguments[0]?.getText(ast).includes("CSS.escape(playerObjectId)")) effects.push(node.arguments[0]);
  ts.forEachChild(node, visit);
}
visit(ast);
assert.equal(effects.length, 1);
const compiled = ts.transpileModule("const effect = " + effects[0].getText(ast) + ";", {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;

function fixture(images = {}) {
  const frames = new Map(), timers = new Map(), listeners = new Map(), ready = [];
  let id = 0, currentBody = null;
  class FakeImage {
    complete = true; naturalWidth = 48; style = { visibility: "visible" };
    matches() { return this === currentBody; }
  }
  const scope = {
    entryPlayerKey: "male:0:288:616", waitingForEntryPlayer: true, playerObjectId: "player-7",
    entryPlayer: { libraries: {}, images }, SCENE_READINESS_SAFETY_MS: 3000,
    setEntryPlayerReadyKey: key => ready.push(key),
    CSS: { escape: value => value }, HTMLImageElement: FakeImage,
    stageFrameRef: { current: {
      querySelector: () => currentBody,
      addEventListener: (name, listener) => listeners.set(name, listener),
      removeEventListener: (name, listener) => { if (listeners.get(name) === listener) listeners.delete(name); },
    } },
    window: {
      requestAnimationFrame: callback => { frames.set(++id, callback); return id; },
      cancelAnimationFrame: handle => frames.delete(handle),
      setTimeout: (callback, ms) => { timers.set(++id, { callback, ms }); return id; },
      clearTimeout: handle => timers.delete(handle),
    },
  };
  const cleanup = new Function(...Object.keys(scope), compiled + "\nreturn effect();")(...Object.values(scope));
  return {
    ready, timers, cleanup, listeners, frames,
    body(overrides = {}) { currentBody = Object.assign(new FakeImage(), overrides); return currentBody; },
    frame() { for (const [key, callback] of frames) { frames.delete(key); callback(); } },
    load(image = currentBody) { listeners.get("load")?.({ target: image }); },
  };
}

test("metadata alone never starts the image-pack timer or releases an absent/undecoded body", () => {
  const f = fixture();
  assert.equal(f.timers.size, 0);
  f.frame(); assert.deepEqual(f.ready, []);
  f.body({ complete: false }); f.load(); assert.deepEqual(f.ready, []);
  f.body({ naturalWidth: 0 }); f.load(); assert.deepEqual(f.ready, []);
  f.body({ style: { visibility: "hidden" } }); f.load(); assert.deepEqual(f.ready, []);
  f.load({ complete: true, naturalWidth: 48 }); assert.deepEqual(f.ready, []);
  f.body(); f.load(); assert.deepEqual(f.ready, ["male:0:288:616"]);
  f.cleanup(); assert.equal(f.listeners.size, 0);
});

test("an already decoded body is recognized on the first animation frame", () => {
  const f = fixture(); f.body(); f.frame();
  assert.deepEqual(f.ready, ["male:0:288:616"]);
  f.cleanup();
});

test("only a verified nonempty image pack retains the existing decode safety timer", () => {
  const f = fixture({ "/original-ui/CArmour/00/0.png": "data:image/png;base64,fixture" });
  assert.deepEqual([...f.timers.values()].map(timer => timer.ms), [3000]);
  f.frame(); assert.deepEqual(f.ready, []);
  [...f.timers.values()][0].callback();
  assert.deepEqual(f.ready, ["male:0:288:616"]);
  f.cleanup(); assert.equal(f.timers.size, 0); assert.equal(f.frames.size, 0);
});

test("retiring a metadata observer removes load and RAF publication", () => {
  const f = fixture(); f.cleanup(); f.body(); f.load(); f.frame();
  assert.deepEqual(f.ready, []); assert.equal(f.listeners.size, 0);
});
