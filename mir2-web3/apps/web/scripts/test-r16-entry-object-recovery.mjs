import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import test from "node:test";
import ts from "typescript";

const web = new URL("../", import.meta.url);
const metadata = JSON.parse(readFileSync(new URL("lib/generated/r16-entry-visuals.json", web)));
const parts = metadata.objectParts;
const bodies = new Map(parts.map(part => [part.path, readFileSync(new URL("public" + part.path, web))]));
const expected = Object.assign({}, ...parts.map(part => JSON.parse(bodies.get(part.path))));
function assertImages(actual, wanted) {
  assert.equal(Object.keys(actual).length, Object.keys(wanted).length, "all original object images must be present");
  const digest = value => createHash("sha256").update(value).digest("hex");
  for (const key of Object.keys(wanted)) {
    assert.equal(typeof actual[key], "string", key);
    assert.equal(digest(actual[key]), digest(wanted[key]), key);
  }
}
const settle = async () => { for (let n = 0; n < 8; n++) await new Promise(setImmediate); };
const compile = source => ts.transpileModule(source, { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022,
} }).outputText;

function fixture(fetcher) {
  const modules = new Map();
  const load = file => {
    if (modules.has(file.href)) return modules.get(file.href).exports;
    const module = { exports: {} }; modules.set(file.href, module);
    new Function("module", "exports", "require", "fetch", compile(readFileSync(file, "utf8")))(
      module, module.exports, name => name.endsWith(".json")
        ? { default: JSON.parse(readFileSync(new URL(name, file))) } : load(new URL(name + ".ts", file)), fetcher);
    return module.exports;
  };
  const entry = load(new URL("lib/r16-entry-scene.ts", web));
  const shell = readFileSync(new URL("app/original-client-shell.tsx", web), "utf8");
  const tree = ts.createSourceFile("shell.tsx", shell, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  let effect;
  const visit = node => {
    if (ts.isCallExpression(node) && node.expression.getText(tree) === "useEffect"
      && node.arguments[1]?.getText(tree) === "[warmEntryArt]") effect = node.arguments[0].getText(tree);
    ts.forEachChild(node, visit);
  }; visit(tree); assert(effect);
  const pending = [];
  const start = () => {
    let images = {}, owner;
    const exports = {};
    const scope = { exports, warmEntryArt: true, process: { env: { MIR2_PINNED_ASSET_VERSION: metadata.assetReleaseId } },
      loadR16EntryNpcsWithRecovery(_release, signal) { owner = signal; return null; }, setEntryNpcs() {},
      preloadR16EntryObjects(...args) { const reads = entry.preloadR16EntryObjects(...args); pending.push(...(reads ?? [])); return reads; },
      loadR16EntryObjectsWithRecovery(...args) {
        assert.equal(args[1], owner, "the same entry-art lifecycle owns both resource observers");
        const read = entry.loadR16EntryObjectsWithRecovery(...args); if (read) pending.push(read); return read;
      }, setEntryObjects(update) { images = update(images); } };
    new Function(...Object.keys(scope), compile(`exports.effect = ${effect};`))(...Object.values(scope));
    const cleanup = exports.effect();
    return { cleanup, images: () => images, owner: () => owner };
  };
  return { entry, start, complete: () => Promise.all(pending) };
}

test("actual entry-art effect recovers a complete object part after both original header waits fail", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [], target = parts[0].path;
  const f = fixture(async (url, options) => {
    calls.push({ url, options });
    if (url === target && calls.filter(row => row.url === target).length <= 2) return new Promise(() => {});
    return new Response(bodies.get(url));
  });
  const live = f.start();
  try {
    await settle(); t.mock.timers.tick(20_000); await settle();
    t.mock.timers.tick(200); await settle(); t.mock.timers.tick(20_000); await settle();
    assert.equal(calls.filter(row => row.url === target).length, 2);
    assert(Object.keys(live.images()).length > 0, "other verified parts stay visible during recovery");
    assert(Object.keys(live.images()).length < 90);
    t.mock.timers.tick(999); await settle(); assert.equal(calls.filter(row => row.url === target).length, 2);
    t.mock.timers.tick(1); await f.complete(); await settle();
    assertImages(live.images(), expected);
    assert.deepEqual(calls.filter(row => row.url === target).map(row => row.options.cache), ["force-cache", "reload", "reload"]);
    assert(calls.every(row => row.options.priority === "high" && row.options.credentials === "omit"));
    assert(calls.filter(row => row.url === target).slice(0, 2).every(row => row.options.signal.reason.name === "TimeoutError"));
  } finally { live.cleanup(); }
});

test("one retired entry-art observer cannot start a retry or receive another observer's recovery", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const counts = new Map();
  const f = fixture(async url => {
    const count = (counts.get(url) ?? 0) + 1; counts.set(url, count);
    return count <= 2 ? new Response(null, { status: 503 }) : new Response(bodies.get(url));
  });
  const retired = f.start(), live = f.start();
  await settle(); t.mock.timers.tick(200); await settle(); retired.cleanup();
  assert.equal(retired.owner().aborted, true); assert.equal(live.owner().aborted, false);
  t.mock.timers.tick(1_000); await f.complete(); await settle();
  assert.deepEqual(retired.images(), {}); assertImages(live.images(), expected);
  assert(parts.every(part => counts.get(part.path) === 3), "live owners share the same verified retry");
  live.cleanup();
});

test("retiring the only entry-art observer during backoff prevents additional object requests", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let calls = 0;
  const f = fixture(async () => { calls++; return new Response(null, { status: 503 }); });
  const live = f.start(); await settle(); t.mock.timers.tick(200); await settle(); live.cleanup();
  await f.complete(); t.mock.timers.tick(600_000); await settle();
  assert.equal(calls, parts.length * 2); assert.deepEqual(live.images(), {});
});

test("persistent object failure stops after one bounded recovery round", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const counts = new Map();
  const f = fixture(async url => { counts.set(url, (counts.get(url) ?? 0) + 1); return new Response(null, { status: 503 }); });
  const live = f.start(); await settle(); t.mock.timers.tick(200); await settle();
  t.mock.timers.tick(1_000); await settle(); t.mock.timers.tick(200); await f.complete();
  t.mock.timers.tick(600_000); await settle();
  assert(parts.every(part => counts.get(part.path) === 4)); assert.deepEqual(live.images(), {});
  live.cleanup();
});

test("a completed shared retry stays cached but cannot publish into a retired entry", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const counts = new Map(), delayed = new Map();
  const f = fixture(async url => {
    const count = (counts.get(url) ?? 0) + 1; counts.set(url, count);
    return count <= 2 ? new Response(null, { status: 503 }) : new Promise(resolve => delayed.set(url, resolve));
  });
  const live = f.start(); await settle(); t.mock.timers.tick(200); await settle();
  t.mock.timers.tick(1_000); await settle(); assert.equal(delayed.size, parts.length); live.cleanup();
  for (const part of parts) delayed.get(part.path)(new Response(bodies.get(part.path)));
  await f.complete(); assert.deepEqual(live.images(), {});
  const cached = await Promise.all(f.entry.preloadR16EntryObjects(metadata.assetReleaseId));
  assertImages(Object.assign({}, ...cached), expected);
  assert(parts.every(part => counts.get(part.path) === 3));
});

test("wrong release and already retired object owners do not start network or publish", () => {
  let calls = 0;
  const f = fixture(async () => { calls++; throw new TypeError("must not fetch"); });
  const retired = new AbortController(); retired.abort();
  assert.equal(f.entry.loadR16EntryObjectsWithRecovery("another-release", new AbortController().signal, () => assert.fail()), null);
  assert.equal(f.entry.loadR16EntryObjectsWithRecovery(metadata.assetReleaseId, retired.signal, () => assert.fail()), null);
  assert.equal(calls, 0);
});

test("definitive, Retry-After and rejected full object bytes cannot authorize recovery", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const target = parts[0].path, raw = bodies.get(target), corrupt = Buffer.from(raw); corrupt[64] ^= 1;
  const cases = [() => new Response(null, { status: 404 }),
    () => new Response(null, { status: 503, headers: { "retry-after": "120" } }),
    () => new Response(null, { status: 429, headers: { "retry-after": "0" } }),
    () => new Response(corrupt), () => new Response(raw.subarray(0, 4096)),
    () => new Response(Buffer.concat([raw, Buffer.from("x")]))];
  for (const response of cases) {
    let calls = 0;
    const f = fixture(async url => { if (url === target) { calls++; return response(); } return new Response(bodies.get(url)); });
    const live = f.start(); await f.complete(); await settle(); t.mock.timers.tick(600_000); await settle();
    assert.equal(calls, 1); assertImages(live.images(), Object.assign({}, ...parts.slice(1).map(part => JSON.parse(bodies.get(part.path)))));
    live.cleanup();
  }
});
