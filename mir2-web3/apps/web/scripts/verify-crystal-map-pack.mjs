#!/usr/bin/env node
// Guard against the "empty map pack" regression.
//
// The runtime serves real per-cell map layouts from the committed
// lib/generated/crystal-map-pack/*.map.gz. If that directory is empty — which is
// exactly what happens when the generator's *library* step is committed but its
// *map* step is not — every map falls back to a synthetic placeholder (the
// dirt/grass fill seen off-spawn on Bichon). The sibling crystal-map-library-meta
// being present is NOT enough on its own.
//
// This verifies the pack is populated and that sample maps decode to a real map
// (sane dimensions; for Crystal type-100, a non-zero non-empty cell count). It
// needs no running server, so it is cheap to run in CI / pre-commit. The live
// "originalMapRegion.cells > 0 && sprites > 0" assertion is covered by
// smoke-crystal-map-api.mjs against a deployed scene API.
//
// Usage:
//   node scripts/verify-crystal-map-pack.mjs                 # default checks
//   node scripts/verify-crystal-map-pack.mjs --maps 0,0100,GA9 --strict
import fs from "node:fs";
import path from "node:path";
import { gunzipSync } from "node:zlib";

const PACK_DIR = path.resolve(import.meta.dirname, "..", "lib", "generated", "crystal-map-pack");
const args = parseArgs(process.argv.slice(2));
const minCount = Number(args.require ?? 1);
const strict = args.strict === true;
const sampleMaps = (args.maps ?? "0,0100,GA9").split(",").map((s) => s.trim()).filter(Boolean);

let failures = 0;
let warnings = 0;

// 1) The pack must be non-empty.
const packFiles = fs.existsSync(PACK_DIR)
  ? fs.readdirSync(PACK_DIR).filter((name) => name.toLowerCase().endsWith(".map.gz"))
  : [];
if (packFiles.length < minCount) {
  fail(
    `crystal-map-pack has ${packFiles.length} *.map.gz (need >= ${minCount}).\n` +
      `  -> generate it (generate-crystal-map-runtime-data.mjs) and commit ` +
      `lib/generated/crystal-map-pack/. Library-meta alone does not render maps.`,
  );
} else {
  ok(`crystal-map-pack: ${packFiles.length} *.map.gz present`);
}

// 2) Sample maps must decode to a real, non-empty map.
for (const stem of sampleMaps) {
  const fileName = `${encodeURIComponent(stem.toLowerCase())}.map.gz`;
  const filePath = path.join(PACK_DIR, fileName);
  if (!fs.existsSync(filePath)) {
    (strict ? fail : warn)(`map "${stem}": ${fileName} not in pack`);
    continue;
  }
  let bytes;
  try {
    bytes = gunzipSync(fs.readFileSync(filePath));
  } catch (error) {
    fail(`map "${stem}": gunzip failed: ${error.message}`);
    continue;
  }
  const type = detectMapType(bytes);
  const width = detectMapWidth(bytes, type);
  const height = detectMapHeight(bytes, type);
  if (!(width > 0) || !(height > 0)) {
    fail(`map "${stem}": bad dimensions ${width}x${height} (type ${type})`);
    continue;
  }
  if (type === 100) {
    const nonEmpty = countType100NonEmptyCells(bytes, width, height);
    if (nonEmpty > 0) {
      ok(`map "${stem}": type 100, ${width}x${height}, ${nonEmpty} non-empty cells`);
    } else {
      fail(`map "${stem}": type 100, ${width}x${height}, 0 non-empty cells (would render empty)`);
    }
  } else {
    ok(`map "${stem}": type ${type}, ${width}x${height}, ${bytes.length} bytes`);
  }
}

if (warnings > 0) console.error(`\n${warnings} warning(s).`);
if (failures > 0) {
  console.error(`\nverify-crystal-map-pack: FAILED (${failures} error(s)).`);
  process.exit(1);
}
console.log("\nverify-crystal-map-pack: OK");

// --- helpers: type detection ported 1:1 from crystal-map-loader.ts ---
function detectMapType(b) {
  if (b[2] === 0x43 && b[3] === 0x23) return 100;
  if (b[0] === 0) return 5;
  if (b[0] === 0x0f && b[5] === 0x53 && b[14] === 0x33) return 6;
  if (b[0] === 0x15 && b[4] === 0x32 && b[6] === 0x41 && b[19] === 0x31) return 4;
  if (b[0] === 0x10 && b[2] === 0x61 && b[7] === 0x31 && b[14] === 0x31) return 1;
  if (b[4] === 0x0f || (b[4] === 0x03 && b[18] === 0x0d && b[19] === 0x0a)) {
    const width = b[0] + (b[1] << 8);
    const height = b[2] + (b[3] << 8);
    return b.length > 52 + width * height * 14 ? 3 : 2;
  }
  if (b[0] === 0x0d && b[1] === 0x4c && b[7] === 0x20 && b[11] === 0x6d) return 7;
  return 0;
}

function detectMapWidth(b, type) {
  if (type === 1) return b.readInt16LE(21) ^ b.readInt16LE(23);
  if (type === 4) return b.readInt16LE(31) ^ b.readInt16LE(33);
  if (type === 5) return b.readInt16LE(22);
  if (type === 6) return b.readInt16LE(16);
  if (type === 7) return b.readInt16LE(21);
  if (type === 100) return b.readInt16LE(4);
  return b.readInt16LE(0);
}

function detectMapHeight(b, type) {
  if (type === 1) return b.readInt16LE(25) ^ b.readInt16LE(23);
  if (type === 4) return b.readInt16LE(35) ^ b.readInt16LE(33);
  if (type === 5) return b.readInt16LE(24);
  if (type === 6) return b.readInt16LE(18);
  if (type === 7) return b.readInt16LE(25);
  if (type === 100) return b.readInt16LE(6);
  return b.readInt16LE(2);
}

function countType100NonEmptyCells(b, width, height) {
  let offset = 8;
  let count = 0;
  for (let i = 0; i < width * height; i += 1) {
    if (offset + 26 > b.length) break;
    const backImage = b.readInt32LE(offset + 2) & 0x1fffffff;
    const middleImage = b.readInt16LE(offset + 8);
    const frontImage = b.readInt16LE(offset + 12);
    if (backImage !== 0 || middleImage !== 0 || frontImage !== 0) count += 1;
    offset += 26;
  }
  return count;
}

function ok(message) {
  console.log(`  ok   ${message}`);
}
function warn(message) {
  warnings += 1;
  console.error(`  warn ${message}`);
}
function fail(message) {
  failures += 1;
  console.error(`  FAIL ${message}`);
}

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--maps") out.maps = argv[(i += 1)];
    else if (a === "--require") out.require = argv[(i += 1)];
    else if (a === "--strict") out.strict = true;
    else throw new Error(`Unknown argument: ${a}`);
  }
  return out;
}
