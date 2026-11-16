import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../app/api/asset-manifest/route.ts", import.meta.url), "utf8");
const tree = ts.createSourceFile("route.ts", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const names = new Set(["createAssetVersion", "createCacheNamespace", "inputIdentitySize",
  "stableJsonDigestInput", "stripVolatileJsonFields", "stableJsonStringify", "normalizeAssetVersion"]);
const functions = tree.statements.filter(node => ts.isFunctionDeclaration(node) && names.has(node.name?.text));
assert.equal(functions.length, names.size);
const compiled = ts.transpileModule(functions.map(node => node.getText(tree)).join("\n"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;
const load = env => new Function("createHash", "process", compiled +
  "\nreturn { createAssetVersion, createCacheNamespace, stableJsonDigestInput };")(createHash, { env });
const remote = { assetBaseUrl: "https://assets.example.test/v/one", browserFallbackBaseUrls: [], objectPrefix: "one" };
const atlas = { kind: "mir2-bevy-entity-atlas-manifest", generatedAt: "today", stats: { durationMs: 20684, pageCount: 1 },
  atlases: [{ sha256: "a".repeat(64), frames: [{ x: 1, width: 28, durationMs: 500 }] }] };
function state(api, value, pretty = false) {
  const raw = JSON.stringify(value, null, pretty ? 2 : undefined);
  return { name: "bevy-entity-atlas-manifest", exists: true, size: Buffer.byteLength(raw),
    contentSha256: createHash("sha256").update(api.stableJsonDigestInput(raw)).digest("hex"), digestSource: "stableJson" };
}
function identity(api, input, config = remote) {
  const version = api.createAssetVersion([input]);
  return { version, namespace: api.createCacheNamespace(version, [input], config) };
}

test("independent atlas exports retain identity despite build timing, timestamps and JSON size", () => {
  const api = load({}), first = state(api, atlas);
  const rebuilt = state(api, { ...atlas, generatedAt: "a different and longer timestamp", stats: { ...atlas.stats, durationMs: 7 } }, true);
  assert.notEqual(first.size, rebuilt.size);
  assert.deepEqual(identity(api, first), identity(api, rebuilt));
});

test("the pinned asset version still has a stable cache namespace across identical exports", () => {
  const api = load({ MIR2_PINNED_ASSET_VERSION: "release-one" });
  const first = state(api, atlas), next = state(api, { ...atlas, stats: { ...atlas.stats, durationMs: 1000000 } }, true);
  assert.deepEqual(identity(api, first), identity(api, next));
});

test("real atlas pixels, geometry and animation timings continue to invalidate both identities", () => {
  const api = load({}), before = identity(api, state(api, atlas));
  for (const change of [v => v.atlases[0].sha256 = "b".repeat(64), v => v.atlases[0].frames[0].x++,
    v => v.atlases[0].frames[0].durationMs++, v => v.stats.pageCount++]) {
    const next = structuredClone(atlas); change(next);
    const after = identity(api, state(api, next));
    assert.notEqual(after.version, before.version);
    assert.notEqual(after.namespace, before.namespace);
  }
});

test("durationMs in unrelated manifests remains content", () => {
  const api = load({});
  const a = { ...atlas, kind: "other-manifest" }, b = structuredClone(a); b.stats.durationMs++;
  assert.notDeepEqual(identity(api, state(api, a)), identity(api, state(api, b)));
});

test("worker bytes, resolver changes and manual cache busting still invalidate caches", () => {
  const api = load({}), input = state(api, atlas), before = identity(api, input);
  assert.notEqual(identity(api, input, { ...remote, assetBaseUrl: "https://other.example.test/v/two" }).namespace, before.namespace);
  assert.notEqual(identity(load({ MIR2_ASSET_CACHE_BUSTER: "force-refresh" }), input).namespace, before.namespace);
  const worker = { name: "asset-worker", exists: true, size: 10, contentSha256: "a".repeat(64), digestSource: "raw" };
  assert.notDeepEqual(identity(api, worker), identity(api, { ...worker, contentSha256: "b".repeat(64) }));
});
