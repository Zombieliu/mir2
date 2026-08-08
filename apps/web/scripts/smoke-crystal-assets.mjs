import { access, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const PUBLIC_ORIGINAL_UI_DIR = path.join(WORKSPACE_ROOT, "public", "original-ui");
const SOURCE_INDEX_PATH = path.join(PUBLIC_ORIGINAL_UI_DIR, "source-libraries.generated.json");
const SOUND_INDEX_PATH = path.join(PUBLIC_ORIGINAL_UI_DIR, "sound-index.generated.json");
const FULL_INDEX_PATH = path.join(REPO_ROOT, "docs", "generated", "assets", "full-crystal-client-index.json");
const OUTPUT_PATH = path.join(REPO_ROOT, "docs", "generated", "assets", "latest-crystal-asset-pipeline-smoke.json");

const REQUIRED_SOURCE_LIBRARIES = ["AArmour/02", "NPC/223", "Monster/010", "Magic", "Prguse"];
const REQUIRED_SOUND_IDS = [10100, 10146, 10147, 10184, 30001];
const failures = [];

const sourceIndex = await readJson(SOURCE_INDEX_PATH, "source library index");
const soundIndex = await readJson(SOUND_INDEX_PATH, "sound index");
const fullIndex = await readJson(FULL_INDEX_PATH, "full client index");

if (Number(fullIndex.libraryCount) < 1000) {
  failures.push(`expected at least 1000 indexed libraries, got ${fullIndex.libraryCount}`);
}
if (Number(fullIndex.libraryFrameCount) < 1000000) {
  failures.push(`expected at least 1000000 indexed frames, got ${fullIndex.libraryFrameCount}`);
}
if (Number(fullIndex.maps?.count) < 400) {
  failures.push(`expected at least 400 maps, got ${fullIndex.maps?.count}`);
}
if (Number(fullIndex.sounds?.count) < 1000) {
  failures.push(`expected at least 1000 source sounds, got ${fullIndex.sounds?.count}`);
}

for (const libraryKey of REQUIRED_SOURCE_LIBRARIES) {
  if (!sourceIndex.libraries?.[libraryKey]) {
    failures.push(`source library ${libraryKey} is missing from source-libraries.generated.json`);
  }
}

for (const soundId of REQUIRED_SOUND_IDS) {
  const sound = soundIndex.sounds?.[String(soundId)];
  if (!sound?.path || sound.sourceExists !== true) {
    failures.push(`sound ${soundId} is missing from sound-index.generated.json`);
    continue;
  }
  await assertFile(path.join(WORKSPACE_ROOT, "public", sound.path), `sound ${soundId} file`);
}

const summary = {
  generatedAt: new Date().toISOString(),
  libraryCount: fullIndex.libraryCount,
  libraryFrameCount: fullIndex.libraryFrameCount,
  mapCount: fullIndex.maps?.count ?? 0,
  sourceSoundCount: fullIndex.sounds?.count ?? 0,
  soundListEntryCount: soundIndex.soundListEntryCount ?? 0,
  exportedSoundCount: soundIndex.exportedSoundCount ?? 0,
  missingSoundCount: soundIndex.missingSoundCount ?? 0,
  requiredSourceLibraries: REQUIRED_SOURCE_LIBRARIES,
  requiredSoundIds: REQUIRED_SOUND_IDS,
  failures,
};

console.log(JSON.stringify(summary, null, 2));
await mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
await writeFile(OUTPUT_PATH, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
console.log(`Wrote ${OUTPUT_PATH}`);

if (failures.length > 0) {
  process.exitCode = 1;
}

async function readJson(filePath, label) {
  try {
    return JSON.parse(await readFile(filePath, "utf8"));
  } catch (error) {
    failures.push(`${label} is not readable: ${error instanceof Error ? error.message : String(error)}`);
    return {};
  }
}

async function assertFile(filePath, label) {
  try {
    await access(filePath);
  } catch {
    failures.push(`${label} is missing: ${filePath}`);
  }
}
