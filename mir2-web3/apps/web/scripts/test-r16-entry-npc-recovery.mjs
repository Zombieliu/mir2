import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const entryFile = new URL("../lib/r16-entry-scene.ts", import.meta.url);
const metadata = JSON.parse(readFileSync(new URL("../lib/generated/r16-entry-visuals.json", import.meta.url)));
const bytes = readFileSync(new URL("../public" + metadata.npcs.path, import.meta.url));
const release = metadata.assetReleaseId;
const settle = async () => { for (let n = 0; n < 8; n++) await new Promise(setImmediate); };
const deferred = () => { let resolve; const promise = new Promise(r => { resolve = r; }); return { promise, resolve }; };

function fixture(fetcher) {
  const modules = new Map();
  function load(file) {
    if (modules.has(file.href)) return modules.get(file.href).exports;
    const module = { exports: {} }; modules.set(file.href, module);
    const code = ts.transpileModule(readFileSync(file, "utf8"), {
      compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    }).outputText;
    const require = name => name.endsWith(".json")
      ? { default: JSON.parse(readFileSync(new URL(name, file))) }
      : load(new URL(name + ".ts", file));
    new Function("module", "exports", "require", "fetch", code)(module, module.exports, require, fetcher);
    return module.exports;
  }
  return load(entryFile);
}

test("NPC pack recovers in the same observer after both 20-second header waits fail", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [], updates = [];
  const entry = fixture(async (url, options) => {
    calls.push({ url, options });
    if (calls.length <= 2) return new Promise(() => {});
    return new Response(bytes);
  });
  const done = entry.loadR16EntryNpcsWithRecovery(release, new AbortController().signal, v => updates.push(v));
  await settle(); t.mock.timers.tick(20_000); await settle();
  t.mock.timers.tick(200); await settle(); t.mock.timers.tick(20_000); await settle();
  assert.equal(calls.length, 2); assert.deepEqual(updates, [null]);
  assert(calls.every(c => c.options.signal.reason.name === "TimeoutError"));
  t.mock.timers.tick(999); await settle(); assert.equal(calls.length, 2);
  t.mock.timers.tick(1); await done;
  assert.equal(calls.length, 3); assert.equal(Object.keys(updates[1].libraries).length, 8);
  assert.deepEqual(updates[1], JSON.parse(bytes));
  assert.deepEqual(calls.map(c => [c.url, c.options.cache, c.options.priority, c.options.credentials]), [
    [metadata.npcs.path, "force-cache", "high", "omit"],
    [metadata.npcs.path, "reload", "high", "omit"],
    [metadata.npcs.path, "reload", "high", "omit"],
  ]);
  assert.equal(await entry.preloadR16EntryNpcs(release), updates[1]);
});

test("two HTTP503 responses preserve a transient outcome and recover verified NPC bytes", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [], updates = [];
  const entry = fixture(async (url, options) => {
    calls.push({ url, options }); return calls.length <= 2 ? new Response(null, { status: 503 }) : new Response(bytes);
  });
  const done = entry.loadR16EntryNpcsWithRecovery(release, new AbortController().signal, v => updates.push(v));
  await settle(); t.mock.timers.tick(200); await settle();
  assert.deepEqual(updates, [null]); t.mock.timers.tick(1_000); await done;
  assert.equal(calls.length, 3); assert.deepEqual(updates[1], JSON.parse(bytes));
});

test("one retired NPC observer does not cancel another observer's shared recovery", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const retired = new AbortController(), calls = [], a = [], b = [];
  const entry = fixture(async (url, options) => {
    calls.push({ url, options }); if (calls.length <= 2) throw new TypeError("temporary connection failure"); return new Response(bytes);
  });
  const first = entry.loadR16EntryNpcsWithRecovery(release, retired.signal, v => a.push(v));
  const second = entry.loadR16EntryNpcsWithRecovery(release, new AbortController().signal, v => b.push(v));
  await settle(); t.mock.timers.tick(200); await settle(); retired.abort(); await first;
  t.mock.timers.tick(1_000); await second;
  assert.deepEqual(a, [null]); assert.equal(calls.length, 3); assert.deepEqual(b[1], JSON.parse(bytes));
  t.mock.timers.tick(600_000); await settle(); assert.equal(calls.length, 3);
});

test("persistent NPC transport failure stops at four requests instead of looping", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [], updates = [];
  const entry = fixture(async url => { calls.push(url); return new Response(null, { status: 503 }); });
  const done = entry.loadR16EntryNpcsWithRecovery(release, new AbortController().signal, v => updates.push(v));
  await settle(); t.mock.timers.tick(200); await settle();
  t.mock.timers.tick(1_000); await settle(); t.mock.timers.tick(200); await done;
  t.mock.timers.tick(600_000); await settle();
  assert.equal(calls.length, 4); assert.deepEqual(updates, [null]);
});

test("definitive responses, Retry-After and corrupt NPC bytes never trigger recovery", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const corrupt = Buffer.from(bytes); corrupt[64] ^= 1;
  const cases = [
    () => new Response(null, { status: 404 }),
    () => new Response(null, { status: 503, headers: { "retry-after": "120" } }),
    () => new Response(null, { status: 429, headers: { "retry-after": "0" } }),
    () => new Response(corrupt), () => new Response(bytes.subarray(0, 4096)),
    () => new Response(Buffer.concat([bytes, Buffer.from("x")])),
  ];
  for (const response of cases) {
    let calls = 0; const updates = [];
    const entry = fixture(async () => { calls++; return response(); });
    await entry.loadR16EntryNpcsWithRecovery(release, new AbortController().signal, v => updates.push(v));
    t.mock.timers.tick(600_000); await settle();
    assert.equal(calls, 1); assert.deepEqual(updates, [null]);
  }
});

test("wrong release, retired owner and late shared completion cannot publish stale NPCs", async () => {
  const body = deferred(), owner = new AbortController(), updates = [], calls = [];
  const entry = fixture(async (url, options) => { calls.push({ url, options }); return body.promise; });
  assert.equal(entry.loadR16EntryNpcsWithRecovery("wrong-release", owner.signal, v => updates.push(v)), null);
  const stopped = new AbortController(); stopped.abort();
  assert.equal(entry.loadR16EntryNpcsWithRecovery(release, stopped.signal, v => updates.push(v)), null);
  assert.equal(calls.length, 0);
  const pending = entry.loadR16EntryNpcsWithRecovery(release, owner.signal, v => updates.push(v));
  owner.abort(); body.resolve(new Response(bytes)); await pending;
  assert.deepEqual(updates, []); assert.equal(calls.length, 1);
  assert.equal(calls[0].options.signal.aborted, false, "shared cache read stays usable");
  assert.deepEqual(await entry.preloadR16EntryNpcs(release), JSON.parse(bytes));
});

test("the real Shell entry-art effect owns NPC recovery and retires it on cleanup", async () => {
  const file = new URL("../app/original-client-shell.tsx", import.meta.url), text = readFileSync(file, "utf8");
  const tree = ts.createSourceFile(file.pathname, text, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let effect;
  const visit = node => {
    if (ts.isCallExpression(node) && node.expression.getText(tree) === "useEffect"
      && node.arguments[1]?.getText(tree) === "[warmEntryArt]") effect = node.arguments[0].getText(tree);
    ts.forEachChild(node, visit);
  }; visit(tree); assert(effect);
  const compiled = ts.transpileModule(`const effect = ${effect}; exports.effect = effect;`, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const response = deferred(), updates = [], entry = fixture(() => response.promise), exports = {};
  let pending, owner;
  new Function("exports", "warmEntryArt", "process", "loadR16EntryNpcsWithRecovery", "preloadR16EntryObjects", "setEntryNpcs", "setEntryObjects", compiled)(
    exports, true, { env: { MIR2_PINNED_ASSET_VERSION: release } },
    (...args) => { owner = args[1]; pending = entry.loadR16EntryNpcsWithRecovery(...args); return pending; },
    () => [], v => updates.push(v), () => assert.fail("no object part was provided"),
  );
  const cleanup = exports.effect(); assert.equal(owner.aborted, false);
  cleanup(); assert.equal(owner.aborted, true); response.resolve(new Response(bytes)); await pending;
  assert.deepEqual(updates, []); assert.deepEqual(await entry.preloadR16EntryNpcs(release), JSON.parse(bytes));
});
