import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import test from "node:test";
import ts from "typescript";

function load(name) {
  const exports = {};
  const source = readFileSync(new URL("../lib/" + name + ".ts", import.meta.url), "utf8");
  new Function("exports", ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText)(exports);
  return exports;
}
const { joinPinnedEntryActorParts } = load("pinned-entry-actor-parts");
const { createPinnedBootstrapChunk } = load("pinned-bootstrap-chunk");
const manifest = JSON.parse(readFileSync(new URL("../lib/generated/r16-entry-walking-parts.json", import.meta.url)));
const original = readFileSync(new URL("../public/bootstrap/r16-visuals/" + manifest.sourceSha256 + ".json", import.meta.url));
const values = manifest.parts.map(part => JSON.parse(readFileSync(new URL("../public" + part.path, import.meta.url))));
const hash = body => createHash("sha256").update(body).digest("hex");
const turn = async () => { for (let i = 0; i < 12; i++) await new Promise(setImmediate); };
const deferred = () => { let resolve;const promise = new Promise(yes => resolve = yes);return { promise, resolve }; };
const parent = (parts) => createPinnedBootstrapChunk({ bytes: manifest.sourceBytes, sha256: manifest.sourceSha256 },
  signal => joinPinnedEntryActorParts(parts, signal), "json-line");

test("five bounded parts reconstruct every original byte, PNG and library in manifest order", async () => {
  const gates = manifest.parts.map(deferred), calls = [];let active = 0, maximum = 0;
  const parts = manifest.parts.map((part, index) => ({ ...part, load: async () => {
    calls.push(index);active++;maximum = Math.max(maximum, active);
    try { await gates[index].promise;return { default: values[index] }; } finally { active--; }
  } }));
  const get = parent(parts), pending = get();assert.equal(get(), pending);await turn();
  assert.deepEqual(calls, [0, 1]);
  for (const index of [1, 2, 0, 3, 4]) { gates[index].resolve();await turn(); }
  const value = await pending, bytes = Buffer.from(JSON.stringify(value) + "\n");
  assert.deepEqual(bytes, original);assert.equal(hash(bytes), manifest.sourceSha256);assert.equal(bytes.length, 764724);
  assert.equal(maximum, 2);assert.equal(active, 0);assert.deepEqual(calls, [0, 1, 2, 3, 4]);
  assert.equal(Object.keys(value.images).length, 240);assert.equal(Object.keys(value.libraries).length, 5);
  assert(manifest.parts.every(part => part.bytes <= 256 * 1024));assert.equal(get(), pending);
});

test("all five reads share the original twenty-second deadline and cannot launch queued parts after it", async t => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const gates = manifest.parts.map(deferred), calls = [];
  const parts = manifest.parts.map((part, index) => ({ ...part, load: async () => {
    calls.push(index);await gates[index].promise;return { default: values[index] };
  } }));
  const get = parent(parts), pending = get();let published = false;
  pending.then(() => { published = true; }, () => {});
  const rejected = assert.rejects(pending, error => error.name === "TimeoutError");
  await turn();t.mock.timers.tick(15000);gates[0].resolve();await turn();
  t.mock.timers.tick(4000);gates[1].resolve();await turn();assert.deepEqual(calls, [0, 1, 2, 3]);
  t.mock.timers.tick(999);await turn();assert.equal(published, false);
  t.mock.timers.tick(1);await rejected;
  for (const gate of gates) gate.resolve();await turn();
  assert.equal(published, false);assert.equal(get(), pending);assert.deepEqual(calls, [0, 1, 2, 3]);
});

test("a corrupt first part prevents publication and drains no further imports", async () => {
  const gate = deferred(), calls = [], broken = structuredClone(values[0]);
  const path = Object.keys(broken.images)[0];broken.images[path] = broken.images[path].slice(0, -1) + "A";
  const parts = manifest.parts.map((part, index) => ({ ...part, load: async () => {
    calls.push(index);if (index === 1) await gate.promise;
    return { default: index === 0 ? broken : values[index] };
  } }));
  await assert.rejects(parent(parts)(), /digest mismatch/);gate.resolve();await turn();assert.deepEqual(calls, [0, 1]);
});

test("valid individual pieces still require the original whole-pack digest", async () => {
  const parts = manifest.parts.map((part, index) => ({ ...part, load: async () => ({ default: values[index] }) })).reverse();
  await assert.rejects(parent(parts)(), /chunk digest mismatch/);
});

test("invalid part counts, duplicate libraries and oversized pins start no imports", async () => {
  let calls = 0;
  const parts = manifest.parts.map(part => ({ ...part, load: async () => { calls++;return { default: {} }; } }));
  for (const input of [[], Array.from({ length: 9 }, () => parts[0]), [parts[0], parts[0]],
    [{ ...parts[0], bytes: 256 * 1024 + 1 }], [{ ...parts[0], sha256: "not-a-digest" }]])
    await assert.rejects(joinPinnedEntryActorParts(input, new AbortController().signal), /Invalid pinned actor parts/);
  assert.equal(calls, 0);
});

test("a correctly pinned payload with missing frame PNGs is not an actor part", async () => {
  const value = structuredClone(values[0]);delete value.images[Object.keys(value.images)[0]];
  const bytes = Buffer.from(JSON.stringify(value) + "\n"), controller = new AbortController();
  const part = { ...manifest.parts[0], bytes: bytes.length, sha256: hash(bytes), load: async () => ({ default: value }) };
  await assert.rejects(joinPinnedEntryActorParts([part], controller.signal), /Invalid pinned actor frames/);
});

test("an already retired observation cannot start the compiled reads", async () => {
  const controller = new AbortController();controller.abort();let calls = 0;
  const parts = manifest.parts.map(part => ({ ...part, load: async () => { calls++;return { default: {} }; } }));
  await assert.rejects(joinPinnedEntryActorParts(parts, controller.signal), error => error.name === "AbortError");assert.equal(calls, 0);
});
