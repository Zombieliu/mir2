import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import ts from "typescript";

import {
  assembleMagicEffectsFromMeta,
  MAP_EFFECTS,
  normalizeAdditiveRgba,
  OBJECT_EFFECTS,
  runCrystalMagicEffectExport,
  SPELL_EFFECT_ENUM,
  SPELL_EFFECTS,
  WORLD_SPELL_EFFECTS,
  validateEffectDefinitions,
} from "./export-crystal-magic-effects.mjs";

const nodeRequire = createRequire(import.meta.url);

function loadTypeScriptModule(url) {
  const source = readFileSync(fileURLToPath(url), "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, esModuleInterop: true },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => (specifier.startsWith("node:") ? nodeRequire(specifier.slice(5)) : nodeRequire(specifier));
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, require);
  return module.exports;
}

const effects = loadTypeScriptModule(new URL("../lib/crystal-magic-effects.ts", import.meta.url));

function effectsFetch(outputDir) {
  return async (url) => {
    const file = path.join(outputDir, String(url).replace(/^\/original-effects\//, ""));
    if (!existsSync(file)) return { ok: false, status: 404, json: async () => ({}) };
    return { ok: true, status: 200, json: async () => JSON.parse(readFileSync(file, "utf8")) };
  };
}

function allRequiredIndices() {
  const byLibrary = new Map();
  for (const spec of [...SPELL_EFFECTS, ...WORLD_SPELL_EFFECTS, ...OBJECT_EFFECTS, ...MAP_EFFECTS]) {
    if (!byLibrary.has(spec.library)) byLibrary.set(spec.library, new Set());
    for (let value = 0; value < (spec.valueCount ?? 1); value += 1) {
      for (let direction = 0; direction < (spec.directionCount ?? 1); direction += 1) {
        const base = spec.base + value * (spec.valueStride ?? 0) + direction * (spec.directionStride ?? 0);
        for (let frame = 0; frame < spec.count; frame += 1) byLibrary.get(spec.library).add(base + frame);
      }
    }
  }
  return byLibrary;
}

function syntheticUiMeta(indices) {
  return {
    count: Math.max(...indices) + 1,
    frames: indices.map((index) => ({ index, width: 4 + (index % 3), height: 5, x: -2, y: -3 })),
  };
}

function buildSyntheticLib(frameIndices) {
  const sorted = [...frameIndices].sort((left, right) => left - right);
  const count = sorted[sorted.length - 1] + 1;
  const headerSize = 8 + count * 4;
  const offsets = new Array(count).fill(0);
  const blocks = [];
  let cursor = headerSize;
  for (const index of sorted) {
    offsets[index] = cursor;
    const bgra = Buffer.alloc(4 * 4 * 4);
    for (let pixel = 0; pixel < 16; pixel += 1) {
      bgra[pixel * 4] = index & 0xff;
      bgra[pixel * 4 + 1] = 0x40;
      bgra[pixel * 4 + 2] = 0x80;
      bgra[pixel * 4 + 3] = 0xff;
    }
    const compressed = gzipSync(bgra);
    const frameHeader = Buffer.alloc(17);
    let offset = 0;
    frameHeader.writeInt16LE(4, offset); offset += 2;
    frameHeader.writeInt16LE(4, offset); offset += 2;
    frameHeader.writeInt16LE(-2, offset); offset += 2;
    frameHeader.writeInt16LE(-3, offset); offset += 2;
    frameHeader.writeInt16LE(0, offset); offset += 2;
    frameHeader.writeInt16LE(0, offset); offset += 2;
    frameHeader.writeUInt8(0, offset); offset += 1;
    frameHeader.writeInt32LE(compressed.length, offset);
    blocks.push(Buffer.concat([frameHeader, compressed]));
    cursor += frameHeader.length + compressed.length;
  }
  const header = Buffer.alloc(headerSize);
  header.writeInt32LE(2, 0);
  header.writeInt32LE(count, 4);
  for (let index = 0; index < count; index += 1) header.writeInt32LE(offsets[index], 8 + index * 4);
  return Buffer.concat([header, ...blocks]);
}

function completeMetaFetch() {
  const metas = Object.fromEntries([...allRequiredIndices()].map(([library, indices]) => [library, syntheticUiMeta([...indices])]));
  const fetchImpl = async (url) => {
    const match = String(url).match(/\/original-ui\/([^/]+)\/meta\.json$/);
    const meta = metas[match?.[1]];
    return { ok: Boolean(meta), status: meta ? 200 : 404, json: async () => meta ?? {} };
  };
  return { metas, fetchImpl };
}

async function testAssembleMode() {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-fx-assemble-"));
  const outputDir = path.join(root, "original-effects");
  const { fetchImpl } = completeMetaFetch();
  const summary = await assembleMagicEffectsFromMeta({ assetBaseUrl: "https://r2.example/v/x", outputDir, fetchImpl });
  assert.deepEqual(summary.available, ["Effect", "Magic", "Magic2", "Magic3"]);

  const manifestPath = path.join(outputDir, "effects.generated.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.equal(manifest.schemaVersion, 2);
  assert.equal(manifest.generatedAt, null, "manifest has no clock-dependent data");
  assert.equal(manifest.spell_effects.length, SPELL_EFFECTS.length);
  const trapWorldSpell = manifest.ground_effects.find(
    (entry) => entry.spell === "TrapHexagon" && entry.provenance.source.includes("SpellObject.cs"),
  );
  assert.deepEqual(
    { base: trapWorldSpell.base, count: trapWorldSpell.count, interval: trapWorldSpell.interval },
    { base: 1390, count: 10, interval: 100 },
  );
  assert.equal(trapWorldSpell.repeat, true);
  assert.deepEqual(manifest.spell_effect_enum, SPELL_EFFECT_ENUM.map((entry) => entry.name), "legacy enum array remains compatible");
  assert.deepEqual(manifest.spell_effect_map, SPELL_EFFECT_ENUM, "explicit numeric map is authoritative");
  assert.equal(manifest.map_effects.find((entry) => entry.effect === "Mine").effectId, 12);
  assert.equal(manifest.map_effects.find((entry) => entry.effect === "Mine").valueCount, 8);
  assert.deepEqual(manifest.map_effects.find((entry) => entry.effect === "Mine").valueRanges[7], { value: 7, base: 56, end: 58 });

  const haste = manifest.spell_effects.find((entry) => entry.spell === "Haste");
  assert.equal(haste.spellId, 93);
  assert.equal(haste.directionCount, 8);
  assert.equal(haste.directionStride, 10);
  assert.deepEqual(haste.directionRanges[7], { direction: 7, base: 2210, end: 2215 });
  assert.equal(haste.light, 6);
  assert.deepEqual(haste.provenance, {
    source: "Crystal/Client/MirObjects/PlayerObject.cs::MirAction.Spell",
    symbol: "Spell.Haste",
  });

  const magicMeta = JSON.parse(readFileSync(path.join(outputDir, "Magic", "meta.json"), "utf8"));
  assert.equal(magicMeta.frames["0"].path, "/original-ui/Magic/0.png");
  assert.equal(magicMeta.frames["0"].width, 4);
  assert.equal(magicMeta.frames["0"].x, -2);

  const assets = await effects.loadEffectAssets(effectsFetch(outputDir));
  assert.equal(effects.resolveSpellEffect(assets, "FireBall").frames.length, 10);
  assert.equal(effects.resolveMapEffect(assets, "TrapHexagon").frames[0].path, "/original-ui/Magic/1390.png");
  assert.equal(effects.resolveMapEffect(assets, "TrapHexagon").repeat, true);
  assert.equal(effects.resolveSpellEffect(assets, "Haste", 7).frames[0].path, "/original-ui/Magic2/2210.png");
  assert.equal(effects.resolveSpellEffect(assets, "Haste", 8), null);
  assert.equal(effects.effectNameForNumber(assets, 12), "Mine");
  const mine = effects.resolveMapEffectByNumber(assets, 12, 7);
  assert.equal(mine.frames[0].path, "/original-ui/Effect/56.png");
  assert.equal(mine.light, 0);
  assert.equal(effects.resolveMapEffectByNumber(assets, 12, 8), null);

  const firstManifest = readFileSync(manifestPath, "utf8");
  await assembleMagicEffectsFromMeta({ assetBaseUrl: "https://r2.example/v/x", outputDir, fetchImpl });
  assert.equal(readFileSync(manifestPath, "utf8"), firstManifest, "repeated export is byte deterministic");
  rmSync(root, { recursive: true, force: true });
}

async function testLibMode() {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-fx-lib-"));
  const dataDir = path.join(root, "Data");
  const outputDir = path.join(root, "original-effects");
  const fs = nodeRequire("node:fs");
  fs.mkdirSync(dataDir, { recursive: true });
  for (const [library, indices] of allRequiredIndices()) fs.writeFileSync(path.join(dataDir, `${library}.Lib`), buildSyntheticLib(indices));

  const summary = await runCrystalMagicEffectExport({ dataDir, outputDir, deflateLevel: 0 });
  assert.deepEqual(summary.available, ["Effect", "Magic", "Magic2", "Magic3"]);
  const png = readFileSync(path.join(outputDir, "Magic", "0.png"));
  assert.ok(png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])));
  const assets = await effects.loadEffectAssets(effectsFetch(outputDir));
  assert.equal(effects.resolveSpellEffect(assets, "FireBall").frames[0].path, "/original-effects/Magic/0.png");
  rmSync(root, { recursive: true, force: true });
}

async function testStrictValidation() {
  validateEffectDefinitions();
  const root = mkdtempSync(path.join(tmpdir(), "mir2-fx-invalid-"));
  const { metas, fetchImpl } = completeMetaFetch();
  metas.Magic.frames = metas.Magic.frames.filter((frame) => frame.index !== 0);
  await assert.rejects(
    assembleMagicEffectsFromMeta({ assetBaseUrl: "https://r2.example/v/x", outputDir: root, fetchImpl }),
    /Magic is missing 1 required frame\(s\): 0/,
  );
  rmSync(root, { recursive: true, force: true });
}

function testAdditiveAlphaNormalization() {
  assert.deepEqual(
    [...normalizeAdditiveRgba(Uint8Array.from([64, 128, 32, 255, 0, 0, 0, 255]))],
    [128, 255, 64, 128, 0, 0, 0, 0],
  );
}

async function main() {
  testAdditiveAlphaNormalization();
  await testAssembleMode();
  await testLibMode();
  await testStrictValidation();
  console.log(`magic effect export e2e tests passed (${SPELL_EFFECTS.length} spells; deterministic metadata + directional/map numeric resolution + strict validation)`);
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
