import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const nodeRequire = createRequire(import.meta.url);

function loadTypeScriptModule(url, requireMap = {}) {
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
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

function readJson(url) {
  return JSON.parse(readFileSync(fileURLToPath(url), "utf8"));
}

// --- DOM/window mocks so the client audio module runs under Node -------------------------------
const playedAudios = [];
const storageBacking = new Map();

function installWindow() {
  const localStorage = {
    getItem: (key) => (storageBacking.has(key) ? storageBacking.get(key) : null),
    setItem: (key, value) => storageBacking.set(key, String(value)),
    removeItem: (key) => storageBacking.delete(key),
    clear: () => storageBacking.clear(),
  };
  globalThis.window = { localStorage };
  globalThis.localStorage = localStorage;
  globalThis.Audio = class {
    constructor(src) {
      this.src = src ?? null;
      this.volume = 1;
      this.loop = false;
      this.currentTime = 0;
      playedAudios.push(this);
    }
    play() {
      this.played = true;
      return Promise.resolve();
    }
    pause() {}
    addEventListener() {}
  };
}

installWindow();

const presentSoundManifest = readJson(new URL("../lib/generated/crystal-present-sounds.generated.json", import.meta.url));
const soundIndex = readJson(new URL("../public/original-ui/sound-index.generated.json", import.meta.url));

const soundIndexExports = loadTypeScriptModule(new URL("../lib/original-sound-index.ts", import.meta.url), {
  "./generated/crystal-present-sounds.generated.json": presentSoundManifest,
  "../public/original-ui/sound-index.generated.json": soundIndex,
});
const soundEventsExports = loadTypeScriptModule(new URL("../lib/original-sound-events.ts", import.meta.url));
const missingIndexedSoundId =
  Object.keys(soundIndex.sounds ?? {})
    .map(Number)
    .find((soundId) => soundIndexExports.crystalSoundIsMissingAsset(soundId)) ?? null;

function loadAudioModule() {
  return loadTypeScriptModule(new URL("../lib/original-audio.ts", import.meta.url), {
    "./original-sound-index": soundIndexExports,
    "./original-sound-events": soundEventsExports,
  });
}

// --- 1. Presence-aware sound index resolution --------------------------------------------------
{
  assert.equal(soundIndexExports.crystalSoundPath(10100), "/original-ui/Sound/100.wav", "present click sound resolves");
  assert.equal(soundIndexExports.crystalSoundPath(10168), "/original-ui/Sound/NewChar.wav", "present NewChar resolves");
  const previousAssetBaseUrl = process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
  try {
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = "https://assets.example.test/mir2/v/demo/";
    assert.equal(
      soundIndexExports.crystalSoundPath(10103),
      "https://assets.example.test/mir2/v/demo/original-ui/Sound/103.wav",
      "remote-backed sound resolves to the configured release URL",
    );
  } finally {
    if (previousAssetBaseUrl === undefined) {
      delete process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
    } else {
      process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL = previousAssetBaseUrl;
    }
  }
  if (missingIndexedSoundId !== null) {
    assert.equal(soundIndexExports.crystalSoundPath(missingIndexedSoundId), null, "indexed but absent sound bytes resolve to null");
  }
  assert.equal(soundIndexExports.crystalSoundPath(999999), null, "unknown sound id resolves to null");
  assert.equal(soundIndexExports.crystalSoundExists(10146), true);
  if (missingIndexedSoundId !== null) {
    assert.equal(soundIndexExports.crystalSoundExists(missingIndexedSoundId), false);
    assert.equal(
      soundIndexExports.crystalSoundIsMissingAsset(missingIndexedSoundId),
      true,
      "indexed-but-absent is a known missing asset",
    );
  }
  assert.equal(soundIndexExports.crystalSoundIsMissingAsset(10100), false, "present sound is not missing");
  assert.equal(soundIndexExports.crystalSoundIsMissingAsset(999999), false, "unknown id is not a 'missing asset'");
  assert.equal(soundIndexExports.crystalPresentSoundCount(), presentSoundManifest.files.length);
  assert.ok(soundIndexExports.crystalIndexedSoundCount() >= soundIndexExports.crystalPresentSoundCount());
}

// --- 2. Settings migration + volume clamping ---------------------------------------------------
{
  storageBacking.clear();
  // Old persisted settings shape (no volume fields).
  storageBacking.set("mir2.originalAudioSettings", JSON.stringify({ musicEnabled: false, effectsEnabled: true }));
  const audio = loadAudioModule();
  const migrated = audio.getOriginalAudioSettings();
  assert.equal(migrated.musicEnabled, false, "preserved persisted musicEnabled");
  assert.equal(migrated.effectsEnabled, true, "preserved persisted effectsEnabled");
  assert.equal(migrated.musicVolume, 0.72, "missing musicVolume migrates to default");
  assert.equal(migrated.effectsVolume, 0.86, "missing effectsVolume migrates to default");

  const clampedHigh = audio.setOriginalAudioSettings({ musicVolume: 2 });
  assert.equal(clampedHigh.musicVolume, 1, "music volume clamps to 1");
  const clampedLow = audio.setOriginalAudioSettings({ effectsVolume: -3 });
  assert.equal(clampedLow.effectsVolume, 0, "effects volume clamps to 0");
  const invalid = audio.setOriginalAudioSettings({ musicVolume: Number.NaN });
  assert.equal(invalid.musicVolume, 1, "NaN volume keeps the previous value");
}

// --- 3. Playback fallback chain + missing-sound telemetry --------------------------------------
{
  storageBacking.clear();
  playedAudios.length = 0;
  const audio = loadAudioModule();
  audio.resetOriginalAudioDiagnosticsForTests();
  audio.setOriginalAudioSettings({ effectsEnabled: true });

  // Fallback: first id is an absent asset, second is present -> the present one plays.
  const missingProbeSoundId = missingIndexedSoundId ?? 999999;
  const playedFallback = audio.playOriginalSoundIdWithFallback([missingProbeSoundId, 10100]);
  assert.equal(playedFallback, true, "fallback chain plays the first present sound");
  assert.equal(playedAudios.at(-1)?.src, "/original-ui/Sound/100.wav");

  // Semantic event resolves through its chain (characterCreated -> NewChar.wav present).
  const playedEvent = audio.playOriginalSoundEvent("characterCreated");
  assert.equal(playedEvent, true, "characterCreated event plays NewChar.wav");
  assert.equal(playedAudios.at(-1)?.src, "/original-ui/Sound/NewChar.wav");

  // Missing sound is skipped gracefully and recorded for diagnostics.
  const playedMissing = audio.playOriginalSoundId(missingProbeSoundId);
  assert.equal(playedMissing, false, "missing sound returns false");
  const diagnostics = audio.getOriginalAudioDiagnostics();
  assert.ok(diagnostics.missingSoundTotal >= 1, "missing sound recorded");
  assert.ok(diagnostics.missingSoundIds.includes(missingProbeSoundId), "missing sound id tracked");

  // Effects volume setting drives the default playback gain.
  audio.setOriginalAudioSettings({ effectsVolume: 0.5 });
  playedAudios.length = 0;
  audio.playOriginalSoundId(10100);
  assert.equal(playedAudios.at(-1)?.volume, 0.5, "effects volume setting applies to playback");

  // Disabling effects suppresses playback.
  audio.setOriginalAudioSettings({ effectsEnabled: false });
  playedAudios.length = 0;
  assert.equal(audio.playOriginalSoundId(10100), false, "disabled effects suppress playback");
  assert.equal(playedAudios.length, 0);
}

console.log("audio system tests passed");
