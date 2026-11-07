import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../app/api/asset-manifest/route.ts", import.meta.url), "utf8");
const tree = ts.createSourceFile("route.ts", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TS);
const names = new Set(["createReleaseCapabilities", "normalizeSha256", "normalizeFullPackIndexPath", "normalizeMapAtlasManifestPath", "normalizePositiveInteger"]);
const functions = tree.statements.filter(node => ts.isFunctionDeclaration(node) && names.has(node.name?.text));
assert.equal(functions.length, names.size);
const compiled = ts.transpileModule(functions.map(node => node.getText(tree)).join("\n"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;
const evaluate = env => new Function("process", compiled + "\nreturn createReleaseCapabilities;")({ env });
const hash = "a".repeat(64), pinnedHash = "b".repeat(64);
const index = { name: "crystal-full-pack-index", exists: true, contentSha256: hash, size: 800000 };
const pins = { MIR2_PINNED_CRYSTAL_FULL_PACK_ENABLED: "1", MIR2_PINNED_CRYSTAL_FULL_PACK_VERIFIED: "1",
  MIR2_PINNED_CRYSTAL_FULL_PACK_CONTENT_HASH: pinnedHash,
  MIR2_PINNED_CRYSTAL_FULL_PACK_PATH: "/generated/crystal-packs/full/index.json",
  MIR2_PINNED_CRYSTAL_FULL_PACK_LIBRARY_COUNT: "1440", MIR2_PINNED_CRYSTAL_FULL_PACK_PAGE_COUNT: "4446" };

test("an index retained by a finite package does not advertise absent full-pack shards", () => {
  const result = evaluate({})("finite-r16", [index]).crystalFullPack;
  assert.equal(result.enabled, false);
  assert.equal(result.verified, false);
  assert.equal(result.contentHash, hash, "the retained index still participates in release metadata");
});

test("local index presence cannot override a disabled or unverified release", () => {
  for (const env of [{ ...pins, MIR2_PINNED_CRYSTAL_FULL_PACK_ENABLED: "0" },
    { ...pins, MIR2_PINNED_CRYSTAL_FULL_PACK_VERIFIED: "0" },
    { ...pins, MIR2_PINNED_CRYSTAL_FULL_PACK_VERIFIED: "" }]) {
    const capability = evaluate(env)("disabled", [index]).crystalFullPack;
    assert.equal(capability.enabled, false);
    assert.equal(capability.verified, false);
  }
});

test("a publisher-verified pinned release remains available without local payloads", () => {
  const result = evaluate(pins)("published", []).crystalFullPack;
  assert.deepEqual(result, { enabled: true, verified: true, indexPath: pins.MIR2_PINNED_CRYSTAL_FULL_PACK_PATH,
    contentHash: pinnedHash, libraryCount: 1440, pageCount: 4446 });
});

test("a local index cannot supply missing or malformed publisher verification pins", () => {
  for (const fields of [{ MIR2_PINNED_CRYSTAL_FULL_PACK_CONTENT_HASH: "" },
    { MIR2_PINNED_CRYSTAL_FULL_PACK_CONTENT_HASH: "bad" },
    { MIR2_PINNED_CRYSTAL_FULL_PACK_PATH: "" },
    { MIR2_PINNED_CRYSTAL_FULL_PACK_PATH: "https://unrelated.test/index.json" }]) {
    const result = evaluate({ ...pins, ...fields })("invalid-pins", [index]).crystalFullPack;
    assert.equal(result.enabled, false);
    assert.equal(result.verified, false);
  }
});

test("map-atlas verification remains independent of a retained full-pack index", () => {
  const result = evaluate({ MIR2_PINNED_MAP_ATLAS_ENABLED: "1", MIR2_PINNED_MAP_ATLAS_VERIFIED: "1",
    MIR2_PINNED_MAP_ATLAS_CONTENT_HASH: hash, MIR2_PINNED_MAP_ATLAS_MANIFEST_PATH: `/generated/map-atlas/manifest.${hash}.json`,
    MIR2_PINNED_MAP_ATLAS_PAGE_COUNT: "57", MIR2_PINNED_MAP_ATLAS_MAX_PAGE_BYTES: "468963" })("maps", [index]);
  assert.equal(result.crystalFullPack.enabled, false);
  assert.equal(result.mapAtlas.enabled, true);
  assert.equal(result.mapAtlas.verified, true);
  assert.equal(result.mapAtlas.pageCount, 57);
});
