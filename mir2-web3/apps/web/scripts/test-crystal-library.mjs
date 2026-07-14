import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { gzipSync } from "node:zlib";

import {
  decodeFrame,
  decodeFrameRgba,
  parseLibrary,
} from "./crystal-library.mjs";
import {
  buildCrystalFrameSetCatalog,
  buildCrystalSourceSnapshot,
  validateCrystalSourceSnapshot,
  writeCrystalSourceSnapshot,
} from "./asset-pipeline/source-snapshot.mjs";

const ACTIONS = [
  {
    actionId: 0,
    start: 0,
    count: 4,
    skip: -4,
    interval: 450,
    effectStart: 0,
    effectCount: 0,
    effectSkip: 0,
    effectInterval: 0,
    reverse: false,
    blend: false,
  },
  {
    actionId: 9,
    start: 80,
    count: 6,
    skip: 2,
    interval: 100,
    effectStart: 200,
    effectCount: 3,
    effectSkip: 5,
    effectInterval: 75,
    reverse: true,
    blend: true,
  },
];

function buildSyntheticLibrary({ version, count = 3, frameIndices = [1], actions = [] }) {
  const fixedHeaderBytes = version >= 3 ? 12 : 8;
  const headerBytes = fixedHeaderBytes + count * 4;
  const offsets = new Array(count).fill(0);
  const imageBlocks = [];
  let cursor = headerBytes;

  for (const index of [...frameIndices].sort((left, right) => left - right)) {
    const rgba = Buffer.from([
      0x01, 0x02, 0x03, 0xff,
      0x11, 0x12, 0x13, 0xff,
      0x21, 0x22, 0x23, 0xff,
      0x31, 0x32, 0x33, 0xff,
    ]);
    const bgra = Buffer.alloc(rgba.length);
    for (let pixel = 0; pixel < rgba.length; pixel += 4) {
      bgra[pixel] = rgba[pixel + 2];
      bgra[pixel + 1] = rgba[pixel + 1];
      bgra[pixel + 2] = rgba[pixel];
      bgra[pixel + 3] = rgba[pixel + 3];
    }
    const compressed = gzipSync(bgra);
    const imageHeader = Buffer.alloc(17);
    let offset = 0;
    imageHeader.writeInt16LE(2, offset); offset += 2;
    imageHeader.writeInt16LE(2, offset); offset += 2;
    imageHeader.writeInt16LE(-2, offset); offset += 2;
    imageHeader.writeInt16LE(-3, offset); offset += 2;
    imageHeader.writeInt16LE(4, offset); offset += 2;
    imageHeader.writeInt16LE(5, offset); offset += 2;
    imageHeader.writeUInt8(0, offset); offset += 1;
    imageHeader.writeInt32LE(compressed.length, offset);

    const block = Buffer.concat([imageHeader, compressed]);
    offsets[index] = cursor;
    cursor += block.length;
    imageBlocks.push(block);
  }

  const frameSeek = version >= 3 ? cursor : null;
  const frameSet = version >= 3 ? buildFrameSet(actions) : Buffer.alloc(0);
  const header = Buffer.alloc(headerBytes);
  header.writeInt32LE(version, 0);
  header.writeInt32LE(count, 4);
  if (version >= 3) header.writeInt32LE(frameSeek, 8);
  for (let index = 0; index < offsets.length; index += 1) {
    header.writeInt32LE(offsets[index], fixedHeaderBytes + index * 4);
  }

  return Buffer.concat([header, ...imageBlocks, frameSet]);
}

function buildFrameSet(actions) {
  const output = Buffer.alloc(4 + actions.length * 35);
  output.writeInt32LE(actions.length, 0);
  let offset = 4;
  for (const action of actions) {
    output.writeUInt8(action.actionId, offset); offset += 1;
    output.writeInt32LE(action.start, offset); offset += 4;
    output.writeInt32LE(action.count, offset); offset += 4;
    output.writeInt32LE(action.skip, offset); offset += 4;
    output.writeInt32LE(action.interval, offset); offset += 4;
    output.writeInt32LE(action.effectStart, offset); offset += 4;
    output.writeInt32LE(action.effectCount, offset); offset += 4;
    output.writeInt32LE(action.effectSkip, offset); offset += 4;
    output.writeInt32LE(action.effectInterval, offset); offset += 4;
    output.writeUInt8(action.reverse ? 1 : 0, offset); offset += 1;
    output.writeUInt8(action.blend ? 1 : 0, offset); offset += 1;
  }
  return output;
}

function testVersion2Compatibility() {
  const library = parseLibrary(buildSyntheticLibrary({ version: 2 }));
  assert.equal(library.version, 2);
  assert.equal(library.count, 3);
  assert.equal(library.frameSeek, null);
  assert.deepEqual(library.frameSet, { seek: null, count: 0, actions: [] });
  assert.equal(library.frames[0], null);
  assert.equal(library.frames[1].x, -2);
  assert.equal(library.frames[2], null);
  assert.deepEqual([...decodeFrameRgba(library, library.frames[1]).subarray(0, 4)], [1, 2, 3, 255]);
}

function testVersion3FrameSet() {
  const library = parseLibrary(buildSyntheticLibrary({ version: 3, actions: ACTIONS }));
  assert.equal(library.frameSet.count, 2);
  assert.equal(library.frameSet.seek, library.frameSeek);
  assert.deepEqual(library.frameSet.actions[0], {
    actionId: 0,
    actionName: "Standing",
    start: 0,
    count: 4,
    skip: -4,
    interval: 450,
    effectStart: 0,
    effectCount: 0,
    effectSkip: 0,
    effectInterval: 0,
    reverse: false,
    blend: false,
  });
  assert.equal(library.frameSet.actions[1].actionName, "Attack1");
  assert.equal(library.frameSet.actions[1].effectSkip, 5);
  assert.equal(library.frameSet.actions[1].reverse, true);
  assert.equal(library.frameSet.actions[1].blend, true);
}

function testDirect3dRowPitchDecode() {
  const bgra = Buffer.alloc(128, 0x7f);
  Buffer.from([
    3, 2, 1, 255,
    13, 12, 11, 255,
  ]).copy(bgra, 0);
  Buffer.from([
    23, 22, 21, 255,
    33, 32, 31, 255,
  ]).copy(bgra, 64);

  assert.deepEqual([...decodeFrame(2, 2, gzipSync(bgra))], [
    1, 2, 3, 255,
    11, 12, 13, 255,
    21, 22, 23, 255,
    31, 32, 33, 255,
  ]);
  assert.throws(
    () => decodeFrame(2, 2, gzipSync(Buffer.alloc(15))),
    /decoded layout mismatch/,
  );
}

function testCorruptInputs() {
  const complete = buildSyntheticLibrary({ version: 3, actions: ACTIONS });
  assert.throws(
    () => parseLibrary(complete.subarray(0, complete.length - 1)),
    /Truncated FrameSet action records/,
  );

  const invalidOffset = Buffer.from(buildSyntheticLibrary({ version: 2 }));
  invalidOffset.writeInt32LE(4, 8 + 4);
  assert.throws(() => parseLibrary(invalidOffset), /points inside the library header/);

  const negativeCount = Buffer.alloc(8);
  negativeCount.writeInt32LE(2, 0);
  negativeCount.writeInt32LE(-1, 4);
  assert.throws(() => parseLibrary(negativeCount), /negative|Invalid Crystal library frame count/);

  const invalidFrameSetSeek = Buffer.from(buildSyntheticLibrary({ version: 3, actions: ACTIONS }));
  invalidFrameSetSeek.writeInt32LE(4, 8);
  assert.throws(() => parseLibrary(invalidFrameSetSeek), /FrameSet points inside the library header/);
}

async function testDeterministicSourceSnapshot() {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-crystal-source-"));
  const dataDir = path.join(root, "Data");
  mkdirSync(path.join(dataDir, "NPC"), { recursive: true });
  writeFileSync(path.join(dataDir, "Root.Lib"), buildSyntheticLibrary({ version: 2 }));
  writeFileSync(path.join(dataDir, "NPC", "001.Lib"), buildSyntheticLibrary({ version: 3, actions: ACTIONS }));

  try {
    const first = await buildCrystalSourceSnapshot({ dataDir });
    const second = await buildCrystalSourceSnapshot({ dataDir });
    assert.deepEqual(first, second, "same source produces byte-stable snapshot data");
    assert.equal(first.summary.libraryCount, 2);
    assert.equal(first.summary.parsedLibraryCount, 2);
    assert.equal(first.summary.frameSetLibraryCount, 1);
    assert.equal(first.summary.actionCount, 2);
    assert.equal(first.summary.issueCount, 0);
    assert.deepEqual(first.libraries.map((library) => library.path), ["NPC/001.Lib", "Root.Lib"]);
    assert.deepEqual(validateCrystalSourceSnapshot(first, { minimumLibraryCount: 2 }), []);

    const frameSetCatalog = buildCrystalFrameSetCatalog(first);
    assert.equal(frameSetCatalog.libraryCount, 1);
    assert.equal(frameSetCatalog.actionCount, 2);
    assert.equal(frameSetCatalog.libraries["NPC/001"].actions[1].actionName, "Attack1");
    assert.equal(frameSetCatalog.libraries["NPC/001"].sourceSha256, first.libraries[0].sha256);
    assert.deepEqual(frameSetCatalog, buildCrystalFrameSetCatalog(second));

    const outputPath = path.join(root, "snapshot.json");
    await writeCrystalSourceSnapshot(outputPath, first);
    assert.deepEqual(JSON.parse(readFileSync(outputPath, "utf8")), first);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function testUiExporterCarriesFrameSetAndMergesSelections() {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-crystal-ui-export-"));
  const dataDir = path.join(root, "Data");
  const outputDir = path.join(root, "output");
  mkdirSync(path.join(dataDir, "NPC"), { recursive: true });
  writeFileSync(
    path.join(dataDir, "NPC", "00.Lib"),
    buildSyntheticLibrary({ version: 3, frameIndices: [0, 1, 2], actions: ACTIONS }),
  );
  writeFileSync(
    path.join(dataDir, "NPC", "01.Lib"),
    buildSyntheticLibrary({ version: 3, frameIndices: [0, 1, 2], actions: ACTIONS.slice(0, 1) }),
  );

  try {
    runUiExport(dataDir, outputDir, "NPC/00");
    runUiExport(dataDir, outputDir, "NPC/01");

    const npcMeta = JSON.parse(readFileSync(path.join(outputDir, "NPC", "00", "meta.json"), "utf8"));
    assert.equal(npcMeta.frameSet.count, 2);
    assert.equal(npcMeta.frameSet.actions[1].actionName, "Attack1");
    assert.equal(npcMeta.frameSet.actions[1].blend, true);

    const manifest = JSON.parse(readFileSync(path.join(outputDir, "manifest.generated.json"), "utf8"));
    assert.deepEqual(
      Object.keys(manifest.libraries).sort(),
      ["NPC/00", "NPC/01"],
      "selective export merges rather than erasing existing library metadata",
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

function runUiExport(dataDir, outputDir, libraryName) {
  const exporterPath = path.join(import.meta.dirname, "export-crystal-ui.mjs");
  const result = spawnSync(
    process.execPath,
    [exporterPath, "--dataDir", dataDir, "--outputDir", outputDir, "--libraries", libraryName],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, `UI export failed:\n${result.stdout}\n${result.stderr}`);
}

async function main() {
  testVersion2Compatibility();
  testVersion3FrameSet();
  testDirect3dRowPitchDecode();
  testCorruptInputs();
  await testDeterministicSourceSnapshot();
  testUiExporterCarriesFrameSetAndMergesSelections();
  console.log("Crystal library FrameSet and source snapshot tests passed");
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
