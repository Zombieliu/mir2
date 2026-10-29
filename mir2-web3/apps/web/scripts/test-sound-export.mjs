// End-to-end test for the local sound pipeline (the path used when CRYSTAL_CLIENT_ROOT points
// at a real Crystal client, e.g. the Drive "Debug" folder). It proves the full chain works
// without needing the real client or clobbering committed assets:
//   1. runCrystalSoundExport reads SoundList.lst + source wavs and copies/indexes them
//   2. a present-sound manifest is derived from the exported files
//   3. the presence-aware resolver (original-sound-index.ts) lights up exactly the sounds whose
//      bytes are present, and the audio layer plays them / records misses
// Uses a synthetic Crystal Sound dir built from REAL SoundList ids+filenames so the assertions
// mirror production.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdtempSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";

import {
  DIRECT_CRYSTAL_SOUND_ENTRIES,
  mergeSoundEntries,
  parseSoundList,
  runCrystalSoundExport,
} from "./export-crystal-sounds.mjs";
import { FIVE_CLASS_MISSING_WAVS, FIVE_CLASS_SKILL_SOUNDS } from "./asset-pipeline/five-class-skill-source.mjs";

const nodeRequire = createRequire(import.meta.url);

// --- minimal but valid WAV (RIFF/WAVE, 8-bit mono, a few samples) -----------------------------
function makeWav(seed = 0) {
  const samples = 8;
  const dataLen = samples;
  const buf = Buffer.alloc(44 + dataLen);
  buf.write("RIFF", 0, "latin1");
  buf.writeUInt32LE(36 + dataLen, 4);
  buf.write("WAVE", 8, "latin1");
  buf.write("fmt ", 12, "latin1");
  buf.writeUInt32LE(16, 16);
  buf.writeUInt16LE(1, 20); // PCM
  buf.writeUInt16LE(1, 22); // mono
  buf.writeUInt32LE(8000, 24); // sample rate
  buf.writeUInt32LE(8000, 28); // byte rate
  buf.writeUInt16LE(1, 32); // block align
  buf.writeUInt16LE(8, 34); // bits/sample
  buf.write("data", 36, "latin1");
  buf.writeUInt32LE(dataLen, 40);
  for (let i = 0; i < dataLen; i += 1) buf[44 + i] = (seed + i) % 256;
  return buf;
}

{
  const merged = mergeSoundEntries(
    [{ id: 10100, fileName: "100.wav" }, { id: 20081, fileName: "M8-1.wav" }],
    DIRECT_CRYSTAL_SOUND_ENTRIES,
  );
  assert.equal(merged.filter((entry) => entry.id === 20081).length, 1);
  assert.equal(merged.find((entry) => entry.id === 51).fileName, "005-1.wav");
  assert.equal(merged.find((entry) => entry.id === 52).fileName, "005-2.wav");
  assert.equal(merged.find((entry) => entry.id === 53).fileName, "005-3.wav");
  assert.equal(merged.find((entry) => entry.id === 20342).fileName, "M34-2.wav");
  assert.equal(merged.find((entry) => entry.id === 20411).fileName, "M41-1.wav");
  assert.equal(merged.find((entry) => entry.id === 20412).fileName, "M41-2.wav");
  assert.equal(merged.find((entry) => entry.id === 20610).fileName, "M61-0.wav");
  assert.equal(merged.find((entry) => entry.id === 20611).fileName, "M61-1.wav");
  assert.equal(merged.find((entry) => entry.id === 20760).fileName, "M76-0.wav");
  assert.equal(merged.find((entry) => entry.id === 20791).fileName, "M79-1.wav");
  assert.throws(
    () => mergeSoundEntries([{ id: 20791, fileName: "wrong.wav" }], DIRECT_CRYSTAL_SOUND_ENTRIES),
    /maps to both/,
  );
}

// --- 1. SoundList parser handles the real-world quirks ----------------------------------------
{
  const sample = [
    ";Movement",
    "10001:  1.wav",
    "0070:  006-0.wav", // leading-zero id, hyphenated name
    "10184:\tfishing_throw.wav", // tab separator
    ";;2385:  commented-out.wav", // doubly-commented, must be ignored
    "2431:  228-1.wav ", // trailing space
    "",
  ].join("\r\n");
  const entries = parseSoundList(sample);
  assert.deepEqual(
    entries,
    [
      { id: 10001, fileName: "1.wav" },
      { id: 70, fileName: "006-0.wav" },
      { id: 10184, fileName: "fishing_throw.wav" },
      { id: 2431, fileName: "228-1.wav" },
    ],
    "SoundList parser must handle leading zeros, tabs, comments, and trailing space",
  );
}

// --- 2. Export copies + indexes, with fallback and missing handling ---------------------------
const tempRoot = mkdtempSync(path.join(tmpdir(), "mir2-sound-e2e-"));
try {
  const clientRoot = path.join(tempRoot, "Debug");
  const soundSrc = path.join(clientRoot, "Sound");
  mkdirSync(soundSrc, { recursive: true });

  // Real ids/filenames from the Crystal SoundList. 23.wav exercises the 22.wav->23.wav fallback.
  // 9999.wav (referenced) has no source file => recorded as missing.
  const soundList = [
    "10100:  100.wav",
    "10146:  Login2.wav",
    "10176:  ride_walk_l.wav",
    "10190:  wolf_attack1.wav",
    "10022:  22.wav", // no 22.wav present -> fallback to 23.wav
    "10023:  23.wav",
    "19999:  9999.wav", // genuinely missing
  ].join("\r\n");
  writeFileSync(path.join(soundSrc, "SoundList.lst"), `${soundList}\r\n`);
  for (const [name, seed] of [
    ["005-1.wav", 15],
    ["005-2.wav", 16],
    ["005-3.wav", 0],
    ["100.wav", 1],
    ["Login2.wav", 2],
    ["ride_walk_l.wav", 3],
    ["wolf_attack1.wav", 4],
    ["23.wav", 5],
    ["M8-1.wav", 6],
    ["M34-0.wav", 7],
    ["M34-1.wav", 8],
    ["M34-2.wav", 9],
    ["M39-0.wav", 10],
    ["M39-1.wav", 11],
    ["M41-1.wav", 12],
    ["M41-2.wav", 13],
    ["M61-0.wav", 14],
    ["M61-1.wav", 15],
    ["M76-0.wav", 16],
    ["M79-1.wav", 17],
    ["M121-0.wav", 18],
    ["M121-2.wav", 19],
    ["M121-5.wav", 20],
  ]) {
    writeFileSync(path.join(soundSrc, name), makeWav(seed));
  }
  for (const entry of DIRECT_CRYSTAL_SOUND_ENTRIES) {
    if (!FIVE_CLASS_MISSING_WAVS.includes(entry.fileName)) {
      writeFileSync(path.join(soundSrc, entry.fileName), makeWav(entry.id));
    }
  }

  const outputDir = path.join(tempRoot, "out", "Sound");
  const indexPath = path.join(tempRoot, "out", "sound-index.generated.json");
  const docPath = path.join(tempRoot, "qa", "source-sounds.json");
  const { summary, copiedSoundCount } = await runCrystalSoundExport({
    clientRoot,
    outputDir,
    indexPath,
    docPath,
    strict: false,
  });

  assert.equal(summary.clientRoot, "Client");
  assert.equal(summary.soundDir, "Client/Sound");
  assert.equal(summary.soundListPath, "Client/Sound/SoundList.lst");
  const publicIndexText = readFileSync(indexPath, "utf8");
  assert.ok(!publicIndexText.includes(tempRoot), "public index must not leak the machine Source root");
  assert.ok(!publicIndexText.includes(clientRoot), "public index must not leak the client machine path");
  const sourceReport = JSON.parse(readFileSync(docPath, "utf8"));
  assert.equal(sourceReport.clientRoot, clientRoot, "isolated QA report retains the actual Source path");
  assert.equal(sourceReport.soundDir, soundSrc);
  assert.deepEqual(sourceReport.sounds, summary.sounds, "path sanitization must not change WAV availability or identity");
  assert.equal(copiedSoundCount, summary.exportedSoundCount);

  const expectedEntryCount = mergeSoundEntries(parseSoundList(soundList), DIRECT_CRYSTAL_SOUND_ENTRIES).length;
  assert.equal(summary.soundListEntryCount, expectedEntryCount);
  assert.equal(summary.exportedSoundCount, expectedEntryCount - 1 - FIVE_CLASS_MISSING_WAVS.length);
  assert.equal(summary.fallbackSoundCount, 1, "22.wav must fall back to 23.wav");
  assert.equal(summary.missingSoundCount, 1 + FIVE_CLASS_MISSING_WAVS.length, "9999.wav and exact-source gaps must be recorded missing");
  assert.equal(summary.sounds["10100"].path, "/original-ui/Sound/100.wav");
  assert.equal(summary.sounds["10100"].exactSourceExists, true);
  assert.equal(summary.sounds["10022"].fallback, true);
  assert.equal(summary.sounds["10022"].fallbackSourceFileName, "23.wav");
  assert.equal(summary.sounds["19999"].sourceExists, false);
  assert.equal(summary.sounds["19999"].path, null);
  assert.equal(summary.sounds["51"].path, "/original-ui/Sound/005-1.wav");
  assert.equal(summary.sounds["52"].path, "/original-ui/Sound/005-2.wav");
  assert.equal(summary.sounds["53"].path, "/original-ui/Sound/005-3.wav");
  assert.equal(summary.sounds["20081"].path, "/original-ui/Sound/M8-1.wav");
  assert.equal(summary.sounds["20340"].path, "/original-ui/Sound/M34-0.wav");
  assert.equal(summary.sounds["20341"].path, "/original-ui/Sound/M34-1.wav");
  assert.equal(summary.sounds["20342"].path, "/original-ui/Sound/M34-2.wav");
  assert.equal(summary.sounds["20390"].path, "/original-ui/Sound/M39-0.wav");
  assert.equal(summary.sounds["20391"].path, "/original-ui/Sound/M39-1.wav");
  assert.equal(summary.sounds["20411"].path, "/original-ui/Sound/M41-1.wav");
  assert.equal(summary.sounds["20412"].path, "/original-ui/Sound/M41-2.wav");
  assert.equal(summary.sounds["20610"].path, "/original-ui/Sound/M61-0.wav");
  assert.equal(summary.sounds["20611"].path, "/original-ui/Sound/M61-1.wav");
  assert.equal(summary.sounds["20760"].path, "/original-ui/Sound/M76-0.wav");
  assert.equal(summary.sounds["20791"].path, "/original-ui/Sound/M79-1.wav");
  for (const [id, name] of [[21210, "M121-0.wav"], [21212, "M121-2.wav"], [21215, "M121-5.wav"]]) {
    assert.equal(summary.sounds[id].path, `/original-ui/Sound/${name}`);
    assert.equal(summary.sounds[id].exactSourceExists, true);
    assert.deepEqual(readFileSync(path.join(outputDir, name)), readFileSync(path.join(soundSrc, name)));
  }
  for (const entry of FIVE_CLASS_SKILL_SOUNDS) {
    const indexed = summary.sounds[entry.id];
    assert.equal(indexed.fallback, false, `${entry.fileName} must never use a substitute`);
    if (entry.expectedSourceAbsent) {
      assert.equal(indexed.sourceExists, false);
      assert.equal(indexed.exactSourceExists, false);
      assert.equal(indexed.path, null);
    } else {
      assert.equal(indexed.exactSourceExists, true);
      assert.deepEqual(readFileSync(path.join(outputDir, entry.fileName)), readFileSync(path.join(soundSrc, entry.fileName)));
    }
  }

  // Files actually landed on disk.
  const copied = new Set(readdirSync(outputDir));
  for (const f of [
    "005-1.wav",
    "005-2.wav",
    "005-3.wav",
    "100.wav",
    "Login2.wav",
    "ride_walk_l.wav",
    "wolf_attack1.wav",
    "22.wav",
    "M8-1.wav",
    "M34-0.wav",
    "M34-1.wav",
    "M34-2.wav",
    "M39-0.wav",
    "M39-1.wav",
    "M41-1.wav",
    "M41-2.wav",
    "M61-0.wav",
    "M61-1.wav",
    "M76-0.wav",
    "M79-1.wav",
    "M121-0.wav",
    "M121-2.wav",
    "M121-5.wav",
  ]) {
    assert.ok(copied.has(f), `exported Sound dir must contain ${f}`);
  }
  assert.ok(!copied.has("9999.wav"), "missing sound must not be written");

  const boundedOutput = path.join(tempRoot, "bounded", "Sound");
  const boundedIndex = path.join(tempRoot, "bounded", "sound-index.generated.json");
  const bounded = await runCrystalSoundExport({ clientRoot: soundSrc, outputDir: boundedOutput, indexPath: boundedIndex,
    docPath: null, copySoundIds: [21210, 21212, 21215, 20940] });
  assert.equal(bounded.copiedSoundCount, 3);
  assert.deepEqual(readdirSync(boundedOutput).sort(), ["M121-0.wav", "M121-2.wav", "M121-5.wav"]);
  assert.deepEqual(bounded.summary.sounds, summary.sounds, "bounded copying preserves the complete Source index");
  assert.equal(bounded.summary.sounds[20940].path, null, "bounded export cannot create an absent Source WAV");
  assert.equal(bounded.summary.soundDir, "Client/Sound", "Sound-directory input uses the same safe relative provenance");
  await assert.rejects(runCrystalSoundExport({ clientRoot, outputDir: boundedOutput, indexPath: boundedIndex,
    docPath: null, copySoundIds: [987654321] }), /invalid or unindexed Source id/);

  // --- 3. present-sound manifest derived from the exported files -------------------------------
  const presentManifest = {
    schemaVersion: 1,
    kind: "mir2-present-sound-manifest",
    soundDir: "public/original-ui/Sound",
    files: readdirSync(outputDir)
      .filter((n) => n.toLowerCase().endsWith(".wav"))
      .sort(),
  };
  presentManifest.presentSoundCount = presentManifest.files.length;

  // --- 4. presence-aware resolver lights up exactly the present sounds -------------------------
  // Model the production split precisely: the committed sound index is generated against the FULL
  // client, so an id can carry a non-null path whose bytes are NOT in the present manifest. That
  // is the real "missing asset" case (raw-asset-limited). An id the export never resolved has
  // path:null and is a different state (not a missing asset, just unreferenced/absent). We inject
  // one of each on top of the freshly-exported index to assert both semantics.
  const soundIndexJson = JSON.parse(readFileSync(indexPath, "utf8"));
  soundIndexJson.sounds["18001"] = {
    fileName: "deferred-bgm.wav",
    path: "/original-ui/Sound/deferred-bgm.wav", // referenced + has a path, but not present on disk
    sourceExists: true,
    exactSourceExists: true,
    fallback: false,
    sourceBytes: 123,
  };
  const soundIndexExports = loadTs(new URL("../lib/original-sound-index.ts", import.meta.url), {
    "./generated/crystal-present-sounds.generated.json": presentManifest,
    "../public/original-ui/sound-index.generated.json": soundIndexJson,
  });

  assert.equal(soundIndexExports.crystalSoundPath(10100), "/original-ui/Sound/100.wav", "present exact sound resolves");
  assert.equal(soundIndexExports.crystalSoundPath(10022), "/original-ui/Sound/22.wav", "fallback-copied sound resolves");
  // path-but-not-present => resolves to null AND counts as a (raw-asset-limited) missing asset
  assert.equal(soundIndexExports.crystalSoundPath(18001), null, "referenced-but-absent sound resolves to null");
  assert.equal(soundIndexExports.crystalSoundIsMissingAsset(18001), true, "referenced-but-absent is a missing asset");
  // never-exported (path:null) => resolves to null but is NOT a missing asset (different state)
  assert.equal(soundIndexExports.crystalSoundPath(19999), null, "unexported sound resolves to null");
  assert.equal(soundIndexExports.crystalSoundIsMissingAsset(19999), false, "unexported (path:null) is not a missing asset");
  // present sound is neither null nor missing
  assert.equal(soundIndexExports.crystalSoundIsMissingAsset(10100), false, "present sound is not a missing asset");
  assert.equal(soundIndexExports.crystalPresentSoundCount(), presentManifest.files.length);

  // --- 5. audio layer plays present, records misses -------------------------------------------
  installAudioGlobals();
  const audioExports = loadTs(new URL("../lib/original-audio.ts", import.meta.url), {
    "./original-sound-index": soundIndexExports,
    "./original-sound-events": loadTs(new URL("../lib/original-sound-events.ts", import.meta.url)),
  });
  audioExports.resetOriginalAudioDiagnosticsForTests();
  audioExports.setOriginalAudioSettings({ effectsEnabled: true });
  assert.equal(audioExports.playOriginalSoundId(10100), true, "present sound plays");
  assert.equal(globalThis.__mir2PlayedAudios.at(-1)?.src, "/original-ui/Sound/100.wav");
  assert.equal(audioExports.playOriginalSoundId(19999), false, "missing sound is skipped");
  const diag = audioExports.getOriginalAudioDiagnostics();
  assert.ok(diag.missingSoundIds.includes(19999), "missing sound recorded in diagnostics");
} finally {
  rmSync(tempRoot, { recursive: true, force: true });
}

console.log("sound export e2e tests passed");

// --- helpers ----------------------------------------------------------------------------------
function loadTs(url, requireMap = {}) {
  const source = readFileSync(fileURLToPath(url), "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      esModuleInterop: true,
      allowSyntheticDefaultImports: true,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    if (specifier.startsWith("node:")) return nodeRequire(specifier.slice(5));
    return nodeRequire(specifier);
  };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, require);
  return module.exports;
}

function installAudioGlobals() {
  const played = [];
  globalThis.__mir2PlayedAudios = played;
  const store = new Map();
  const localStorage = {
    getItem: (k) => (store.has(k) ? store.get(k) : null),
    setItem: (k, v) => store.set(k, String(v)),
    removeItem: (k) => store.delete(k),
  };
  globalThis.window = { localStorage };
  globalThis.localStorage = localStorage;
  globalThis.Audio = class {
    constructor(src) {
      this.src = src ?? null;
      this.volume = 1;
      played.push(this);
    }
    play() {
      return Promise.resolve();
    }
    pause() {}
    addEventListener() {}
  };
}
