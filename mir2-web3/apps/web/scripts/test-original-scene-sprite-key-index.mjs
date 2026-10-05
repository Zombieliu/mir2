import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import {
  createOriginalSceneSpriteKeyIndex, serializeOriginalSceneSpriteKeyIndex,
  synchronizeOriginalSceneSpriteKeyIndex,
} from "./lib/original-scene-sprite-key-index.mjs";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = path.join(webRoot, "public", "original-ui", "manifest.generated.json");
const sourcePath = path.join(webRoot, "public", "original-ui", "source-libraries.generated.json");
const indexPath = path.join(webRoot, "lib", "generated", "original_scene_sprite_library_keys.json");

test("committed compact index exactly preserves every exported key from the full source manifest", async () => {
  const manifestBytes = await fs.readFile(manifestPath);
  const source = JSON.parse(await fs.readFile(sourcePath, "utf8"));
  const expected = createOriginalSceneSpriteKeyIndex(manifestBytes);
  const actual = JSON.parse(await fs.readFile(indexPath, "utf8"));
  assert.equal(Object.keys(source.libraries).length, 1440);
  assert.equal(expected.keys.length, 198);
  assert.deepEqual(actual, expected);
  assert.equal(actual.sourceSha256, createHash("sha256").update(manifestBytes).digest("hex"));
  assert.deepEqual(actual.keys, Object.keys(JSON.parse(manifestBytes).libraries)
    .map((key) => key.replaceAll("\\", "/")).sort());
  assert.equal((await synchronizeOriginalSceneSpriteKeyIndex({ manifestPath, outputPath: indexPath,
    check: true })).changed, false);
  const exported = new Set(actual.keys);
  const sourceOnly = new Set(Object.keys(source.libraries)
    .map((key) => key.replaceAll("\\", "/")).filter((key) => !key.startsWith("Map/")));
  assert.equal([...exported].some((key) => !sourceOnly.has(key)), true);
  assert.equal([...sourceOnly].some((key) => !exported.has(key)), true);
});

test("normalization, map keys and invalid source shapes keep the old availability semantics", () => {
  const fixture = Buffer.from(JSON.stringify({ libraries: {
    "Title\\000": {}, "Map/exported": {}, "Monster/001": {},
  } }));
  const result = createOriginalSceneSpriteKeyIndex(fixture);
  assert.deepEqual(result.keys, ["Map/exported", "Monster/001", "Title/000"]);
  assert.throws(() => createOriginalSceneSpriteKeyIndex(Buffer.from("{")), /invalid JSON/);
  assert.throws(() => createOriginalSceneSpriteKeyIndex(Buffer.from("{}")), /no libraries/);
  assert.throws(() => createOriginalSceneSpriteKeyIndex(Buffer.from('{"libraries":[]}')), /no libraries/);
  assert.throws(() => createOriginalSceneSpriteKeyIndex(Buffer.from('{"libraries":{}}')), /valid library keys/);
  assert.throws(() => serializeOriginalSceneSpriteKeyIndex({ ...result, keys: ["b", "a"] }), /Malformed/);
  assert.throws(() => serializeOriginalSceneSpriteKeyIndex({ ...result, keys: ["a", "a"] }), /Malformed/);
});

test("check mode rejects missing, stale and malformed derived files while write mode repairs them", async () => {
  const parent = process.env.MIR2_INDEX_TEST_ROOT ?? os.tmpdir();
  await fs.mkdir(parent, { recursive: true });
  const directory = await fs.mkdtemp(path.join(parent, "sprite-key-index-"));
  const manifest = path.join(directory, "manifest.json");
  const output = path.join(directory, "keys.json");
  await fs.writeFile(manifest, '{"libraries":{"A/0":{},"B/1":{}}}\n');
  await assert.rejects(synchronizeOriginalSceneSpriteKeyIndex({ manifestPath: manifest,
    outputPath: output, check: true }), /missing, malformed or stale/);
  assert.equal((await synchronizeOriginalSceneSpriteKeyIndex({ manifestPath: manifest,
    outputPath: output })).changed, true);
  assert.equal((await synchronizeOriginalSceneSpriteKeyIndex({ manifestPath: manifest,
    outputPath: output, check: true })).changed, false);
  await fs.writeFile(output, "{\n");
  await assert.rejects(synchronizeOriginalSceneSpriteKeyIndex({ manifestPath: manifest,
    outputPath: output, check: true }), /missing, malformed or stale/);
  await synchronizeOriginalSceneSpriteKeyIndex({ manifestPath: manifest, outputPath: output });
  await fs.writeFile(manifest, '{"libraries":{"A/0":{},"B/1":{},"C/2":{}}}\n');
  await assert.rejects(synchronizeOriginalSceneSpriteKeyIndex({ manifestPath: manifest,
    outputPath: output, check: true }), /missing, malformed or stale/);
  await synchronizeOriginalSceneSpriteKeyIndex({ manifestPath: manifest, outputPath: output });
  assert.deepEqual(JSON.parse(await fs.readFile(output, "utf8")).keys, ["A/0", "B/1", "C/2"]);
});
