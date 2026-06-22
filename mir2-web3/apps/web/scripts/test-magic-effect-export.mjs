// End-to-end test for the magic/spell effect-atlas pipeline. Proves BOTH producer modes feed the
// REAL client resolver (lib/crystal-magic-effects.ts loadEffectAssets / resolveSpellEffect) without a
// real Crystal client and without clobbering committed assets:
//   • PRIMARY  — assembleMagicEffectsFromMeta: reads the already-extracted /original-ui/<lib>/meta.json
//     (the full-Crystal R2 release) and emits /original-effects manifest+meta pointing at those frames.
//   • SECONDARY — runCrystalMagicEffectExport: extracts frames straight from a synthetic Magic*.Lib.
// Both use REAL frame ranges from SPELL_EFFECTS. Mirrors test-sound-export.mjs / test-audio-system.mjs.

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
  runCrystalMagicEffectExport,
  SPELL_EFFECTS,
} from "./export-crystal-magic-effects.mjs";

const nodeRequire = createRequire(import.meta.url);

// --- in-memory TS loader (same approach as test-audio-system.mjs) -----------------------------
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

// fetch stub that serves the assembled /original-effects/* output from a temp dir.
function effectsFetch(outputDir) {
  return async (url) => {
    const file = path.join(outputDir, String(url).replace(/^\/original-effects\//, ""));
    if (!existsSync(file)) return { ok: false, status: 404, json: async () => ({}) };
    return { ok: true, status: 200, json: async () => JSON.parse(readFileSync(file, "utf8")) };
  };
}

function specFor(spell) {
  return SPELL_EFFECTS.find((s) => s.spell === spell);
}
function indicesFor(spell) {
  const s = specFor(spell);
  return Array.from({ length: s.count }, (_, i) => s.base + i);
}

// --- synthetic /original-ui/<lib>/meta.json (the R2 array format: frames:[{index,...}]) -------
function syntheticUiMeta(indices) {
  return {
    count: Math.max(...indices) + 1,
    frames: indices.map((i) => ({ index: i, width: 4 + (i % 3), height: 5, x: -2, y: -3, path: `/original-ui/lib/${i}.png` })),
  };
}

// --- synthetic .Lib writer (inverse of crystal-library.mjs parseLibrary, version 2) -----------
function buildSyntheticLib(frameIndices) {
  const sorted = [...frameIndices].sort((a, b) => a - b);
  const count = sorted[sorted.length - 1] + 1;
  const headerSize = 8 + count * 4;
  const offsets = new Array(count).fill(0);
  const blocks = [];
  let cursor = headerSize;
  const W = 4;
  const H = 4;
  for (const index of sorted) {
    offsets[index] = cursor;
    const bgra = Buffer.alloc(W * H * 4);
    for (let p = 0; p < W * H; p += 1) {
      bgra[p * 4] = index & 0xff;
      bgra[p * 4 + 1] = 0x40;
      bgra[p * 4 + 2] = 0x80;
      bgra[p * 4 + 3] = 0xff;
    }
    const gz = gzipSync(bgra);
    const head = Buffer.alloc(17); // 6×i16 (12) + u8 shadow (1) + i32 length (4)
    let o = 0;
    head.writeInt16LE(W, o); o += 2;
    head.writeInt16LE(H, o); o += 2;
    head.writeInt16LE(-2, o); o += 2;
    head.writeInt16LE(-3, o); o += 2;
    head.writeInt16LE(0, o); o += 2;
    head.writeInt16LE(0, o); o += 2;
    head.writeUInt8(0, o); o += 1;
    head.writeInt32LE(gz.length, o); o += 4;
    blocks.push(Buffer.concat([head, gz]));
    cursor += head.length + gz.length;
  }
  const header = Buffer.alloc(headerSize);
  header.writeInt32LE(2, 0);
  header.writeInt32LE(count, 4);
  for (let i = 0; i < count; i += 1) header.writeInt32LE(offsets[i], 8 + i * 4);
  return Buffer.concat([header, ...blocks]);
}

// =============================================================================================
async function testAssembleMode() {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-fx-assemble-"));
  const outputDir = path.join(root, "original-effects");

  // The R2 source meta for the three effect libs (real frame ranges from SPELL_EFFECTS).
  const uiMetas = {
    Magic: syntheticUiMeta(indicesFor("FireBall")), // 0..9
    Magic2: syntheticUiMeta(indicesFor("ThunderBolt")), // 20..22
    Magic3: syntheticUiMeta(indicesFor("MagicBooster")), // 80..88
  };
  const fetchImpl = async (url) => {
    const m = String(url).match(/\/original-ui\/(Magic\d?)\/meta\.json$/);
    return { ok: true, status: 200, json: async () => uiMetas[m?.[1]] ?? { frames: [] } };
  };

  const summary = await assembleMagicEffectsFromMeta({ assetBaseUrl: "https://r2.example/v/x", outputDir, fetchImpl });
  assert.deepEqual(summary.available, ["Magic", "Magic2", "Magic3"], "assembled all three libs");

  const manifest = JSON.parse(readFileSync(path.join(outputDir, "effects.generated.json"), "utf8"));
  assert.equal(manifest.mode, "assemble-from-r2-meta");
  assert.equal(manifest.spell_effects.length, SPELL_EFFECTS.length, "all spells in manifest");

  const magicMeta = JSON.parse(readFileSync(path.join(outputDir, "Magic", "meta.json"), "utf8"));
  assert.equal(magicMeta.frames["0"].path, "/original-ui/Magic/0.png", "frame path points at the R2-backfilled UI frame");
  assert.equal(magicMeta.frames["0"].width, 4, "geometry carried from source meta");
  assert.equal(magicMeta.frames["0"].x, -2, "offset carried from source meta");

  const assets = await effects.loadEffectAssets(effectsFetch(outputDir));
  const fb = effects.resolveSpellEffect(assets, "FireBall");
  assert.ok(fb && fb.frames.length === 10, "FireBall resolves to 10 real frames");
  assert.equal(fb.frames[0].path, "/original-ui/Magic/0.png", "resolved frame reuses the R2 UI path");
  assert.ok(effects.resolveSpellEffect(assets, "ThunderBolt").frames.length === 3, "ThunderBolt (Magic2) resolves");
  assert.ok(effects.resolveSpellEffect(assets, "MagicBooster").frames.length === 9, "MagicBooster (Magic3) resolves");
  // a spell whose frames weren't in the (partial) synthetic source meta → null → procedural fallback.
  assert.equal(effects.resolveSpellEffect(assets, "IceStorm"), null, "un-sourced spell falls back to null");

  rmSync(root, { recursive: true, force: true });
}

async function testLibMode() {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-fx-lib-"));
  const dataDir = path.join(root, "Data");
  const outputDir = path.join(root, "original-effects");
  const fs = nodeRequire("node:fs");
  fs.mkdirSync(dataDir, { recursive: true });
  fs.writeFileSync(path.join(dataDir, "Magic.Lib"), buildSyntheticLib(indicesFor("FireBall")));
  fs.writeFileSync(path.join(dataDir, "Magic2.Lib"), buildSyntheticLib(indicesFor("ThunderBolt")));
  fs.writeFileSync(path.join(dataDir, "Magic3.Lib"), buildSyntheticLib(indicesFor("MagicBooster")));

  const summary = await runCrystalMagicEffectExport({ dataDir, outputDir, deflateLevel: 0 });
  assert.deepEqual(summary.available, ["Magic", "Magic2", "Magic3"]);
  assert.ok(existsSync(path.join(outputDir, "Magic", "0.png")), "frame PNG written");
  const png = readFileSync(path.join(outputDir, "Magic", "0.png"));
  assert.ok(png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])), "valid PNG");

  const assets = await effects.loadEffectAssets(effectsFetch(outputDir));
  const fb = effects.resolveSpellEffect(assets, "FireBall");
  assert.ok(fb && fb.frames.length === 10, "FireBall resolves from extracted .Lib frames");
  assert.equal(fb.frames[0].path, "/original-effects/Magic/0.png", "extracted-mode frame path is self-hosted");

  rmSync(root, { recursive: true, force: true });
}

async function main() {
  await testAssembleMode();
  await testLibMode();
  console.log(`magic effect export e2e tests passed (${SPELL_EFFECTS.length} spells; assemble-from-R2-meta + extract-from-.Lib both feed resolveSpellEffect)`);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
