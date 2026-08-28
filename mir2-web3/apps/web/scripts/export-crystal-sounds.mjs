import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const LOCAL_CRYSTAL_BUILD_ROOT = path.join(MIR2_ROOT, "Crystal", "Build", "Client", "Debug");
const DOWNLOADED_CRYSTAL_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const DEFAULT_CLIENT_ROOT = existsSync(LOCAL_CRYSTAL_BUILD_ROOT)
  ? LOCAL_CRYSTAL_BUILD_ROOT
  : DOWNLOADED_CRYSTAL_ROOT;
const PUBLIC_SOUND_DIR = path.join(WORKSPACE_ROOT, "public", "original-ui", "Sound");
const PUBLIC_SOUND_INDEX_PATH = path.join(WORKSPACE_ROOT, "public", "original-ui", "sound-index.generated.json");
const DOC_SOUND_INDEX_PATH = path.join(REPO_ROOT, "docs", "generated", "assets", "latest-sound-assets.json");

const FALLBACK_SOUND_FILES = new Map([
  [
    "22.wav",
    {
      sourceFileName: "23.wav",
      reason: "SoundList movement clip 22.wav is absent from the current Crystal source; adjacent movement clip 23.wav keeps id 10022 playable.",
    },
  ],
  [
    "109.wav",
    {
      sourceFileName: "110.wav",
      reason: "SoundList struck clip 109.wav is absent from the current Crystal source; adjacent struck clip 110.wav keeps id 10109 playable.",
    },
  ],
  [
    "ZombieRevive.wav",
    {
      sourceFileName: "64.wav",
      reason: "ClZombie revive clip ZombieRevive.wav is absent from the current Crystal source; nearby undead BoneFamiliar clip 64.wav keeps id 705 playable until the exact source appears.",
    },
  ],
]);

// Crystal derives some sound ids arithmetically (magic uses
// 20000 + Spell*10 + variant; monsters use BaseImage*10 + action variant), so
// these clips are not necessarily listed in SoundList.lst even though the
// client invokes the numeric id directly. Keep the index and exported bytes
// closed over every locally implemented effect/audio checkpoint.
export const DIRECT_CRYSTAL_SOUND_ENTRIES = [
  { id: 51, fileName: "005-1.wav" },
  { id: 52, fileName: "005-2.wav" },
  { id: 53, fileName: "005-3.wav" },
  { id: 20081, fileName: "M8-1.wav" },
  { id: 20340, fileName: "M34-0.wav" },
  { id: 20341, fileName: "M34-1.wav" },
  { id: 20342, fileName: "M34-2.wav" },
  { id: 20390, fileName: "M39-0.wav" },
  { id: 20391, fileName: "M39-1.wav" },
  { id: 20610, fileName: "M61-0.wav" },
  { id: 20611, fileName: "M61-1.wav" },
  { id: 20791, fileName: "M79-1.wav" },
];

// Only auto-run when invoked directly (node scripts/export-crystal-sounds.mjs ...). When the
// module is imported (e.g. by the e2e test) main() must not fire so tests can call the core
// function with isolated paths.
const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

async function main() {
  const clientRoot = path.resolve(process.argv[2] ?? process.env.CRYSTAL_CLIENT_ROOT ?? DEFAULT_CLIENT_ROOT);
  const result = await runCrystalSoundExport({
    clientRoot,
    // Output overrides exist mainly for tests; production defaults to the committed asset paths.
    outputDir: process.env.MIR2_SOUND_OUTPUT_DIR ? path.resolve(process.env.MIR2_SOUND_OUTPUT_DIR) : PUBLIC_SOUND_DIR,
    indexPath: process.env.MIR2_SOUND_INDEX_PATH ? path.resolve(process.env.MIR2_SOUND_INDEX_PATH) : PUBLIC_SOUND_INDEX_PATH,
    docPath: process.env.MIR2_SOUND_DOC_INDEX_PATH ? path.resolve(process.env.MIR2_SOUND_DOC_INDEX_PATH) : DOC_SOUND_INDEX_PATH,
    strict: process.env.MIR2_CRYSTAL_SOUND_STRICT === "1",
  });

  console.log(
    `Exported ${result.summary.exportedSoundCount}/${result.summary.soundListEntryCount} SoundList entries from ${result.summary.sourceWavCount} source wavs`,
  );
  if (result.summary.missingSoundCount) {
    console.warn(
      `Missing ${result.summary.missingSoundCount} SoundList wavs: ${result.summary.missingSounds.map((entry) => entry.fileName).join(", ")}`,
    );
  }
  if (result.summary.fallbackSoundCount) {
    console.warn(
      `Fallback ${result.summary.fallbackSoundCount} SoundList wavs: ${result.summary.fallbackSounds
        .map((entry) => `${entry.fileName}->${entry.fallbackSourceFileName}`)
        .join(", ")}`,
    );
  }
  if (result.strict && result.summary.missingSoundCount) {
    process.exitCode = 1;
  }
}

// Core export: read SoundList.lst, index the source wavs, copy each referenced clip into
// outputDir, and write the sound index json (to both indexPath and docPath). Returns the
// summary so callers (CLI + tests) can react. Pure wrt globals except the filesystem writes it
// is given, which makes it safe to exercise against temp dirs.
export async function runCrystalSoundExport({ clientRoot, outputDir, indexPath, docPath, strict = false }) {
  const resolvedClientRoot = path.resolve(clientRoot);
  const soundDir =
    path.basename(resolvedClientRoot).toLowerCase() === "sound"
      ? resolvedClientRoot
      : path.join(resolvedClientRoot, "Sound");
  const soundListPath = path.join(soundDir, "SoundList.lst");

  if (!existsSync(soundDir)) {
    throw new Error(`Crystal sound directory is missing: ${soundDir}`);
  }
  if (!existsSync(soundListPath)) {
    throw new Error(`Crystal SoundList.lst is missing: ${soundListPath}`);
  }

  const sourceFiles = indexSourceWavs(soundDir);
  const entries = mergeSoundEntries(
    parseSoundList(await readFile(soundListPath, "utf8")),
    DIRECT_CRYSTAL_SOUND_ENTRIES,
  );
  const sounds = {};
  const missing = [];
  const fallbackSounds = [];
  let copiedCount = 0;
  let fallbackCount = 0;

  await mkdir(outputDir, { recursive: true });

  for (const entry of entries) {
    let sourcePath = sourceFiles.get(entry.fileName.toLowerCase());
    const fallback = sourcePath ? null : FALLBACK_SOUND_FILES.get(entry.fileName);
    if (fallback) {
      sourcePath = sourceFiles.get(fallback.sourceFileName.toLowerCase());
    }

    if (!sourcePath) {
      missing.push(entry);
      sounds[String(entry.id)] = {
        fileName: entry.fileName,
        path: null,
        sourceExists: false,
        exactSourceExists: false,
        fallback: false,
        sourceBytes: 0,
      };
      continue;
    }

    const outputPath = path.join(outputDir, entry.fileName);
    await mkdir(path.dirname(outputPath), { recursive: true });
    await copyFile(sourcePath, outputPath);
    copiedCount += 1;
    sounds[String(entry.id)] = {
      fileName: entry.fileName,
      path: `/original-ui/Sound/${entry.fileName}`,
      sourceExists: true,
      exactSourceExists: !fallback,
      fallback: Boolean(fallback),
      ...(fallback
        ? {
            fallbackSourceFileName: fallback.sourceFileName,
            fallbackReason: fallback.reason,
          }
        : {}),
      sourceBytes: (await stat(sourcePath)).size,
    };
    if (fallback) {
      fallbackCount += 1;
      fallbackSounds.push({
        id: entry.id,
        fileName: entry.fileName,
        fallbackSourceFileName: fallback.sourceFileName,
        fallbackReason: fallback.reason,
      });
    }
  }

  const summary = {
    generatedAt: new Date().toISOString(),
    clientRoot: resolvedClientRoot,
    soundDir,
    soundListPath,
    soundListEntryCount: entries.length,
    exportedSoundCount: copiedCount,
    fallbackSoundCount: fallbackCount,
    sourceWavCount: sourceFiles.size,
    missingSoundCount: missing.length,
    missingSounds: missing,
    fallbackSounds,
    sounds,
  };

  await mkdir(path.dirname(indexPath), { recursive: true });
  await writeFile(indexPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  if (docPath) {
    await mkdir(path.dirname(docPath), { recursive: true });
    await writeFile(docPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  }

  return { summary, strict };
}

export function indexSourceWavs(soundDir) {
  const files = new Map();
  for (const entry of readdirSync(soundDir, { withFileTypes: true })) {
    if (!entry.isFile() || !/\.wav$/i.test(entry.name)) {
      continue;
    }
    files.set(entry.name.toLowerCase(), path.join(soundDir, entry.name));
  }
  return files;
}

export function mergeSoundEntries(soundListEntries, directEntries) {
  const merged = new Map();
  for (const entry of [...soundListEntries, ...directEntries]) {
    const existing = merged.get(entry.id);
    if (existing && existing.fileName.toLowerCase() !== entry.fileName.toLowerCase()) {
      throw new Error(`Crystal sound id ${entry.id} maps to both ${existing.fileName} and ${entry.fileName}`);
    }
    merged.set(entry.id, entry);
  }
  return [...merged.values()].sort((left, right) => left.id - right.id);
}

export function parseSoundList(text) {
  const entries = [];
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith(";")) {
      continue;
    }
    const match = /^(\d+)\s*:\s*(\S.+?)\s*$/.exec(line);
    if (!match) {
      continue;
    }
    entries.push({
      id: Number(match[1]),
      fileName: match[2].replaceAll("\\", "/").split("/").filter(Boolean).join("/"),
    });
  }
  return entries;
}
