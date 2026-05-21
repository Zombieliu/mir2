import { copyFile, mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const DEFAULT_CLIENT_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
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

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  const clientRoot = path.resolve(process.argv[2] ?? process.env.CRYSTAL_CLIENT_ROOT ?? DEFAULT_CLIENT_ROOT);
  const soundDir = path.basename(clientRoot).toLowerCase() === "sound" ? clientRoot : path.join(clientRoot, "Sound");
  const soundListPath = path.join(soundDir, "SoundList.lst");
  const strict = process.env.MIR2_CRYSTAL_SOUND_STRICT === "1";

  if (!existsSync(soundDir)) {
    throw new Error(`Crystal sound directory is missing: ${soundDir}`);
  }
  if (!existsSync(soundListPath)) {
    throw new Error(`Crystal SoundList.lst is missing: ${soundListPath}`);
  }

  const sourceFiles = indexSourceWavs(soundDir);
  const entries = parseSoundList(await readFile(soundListPath, "utf8"));
  const sounds = {};
  const missing = [];
  const fallbackSounds = [];
  let copiedCount = 0;
  let fallbackCount = 0;

  await mkdir(PUBLIC_SOUND_DIR, { recursive: true });

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

    const outputPath = path.join(PUBLIC_SOUND_DIR, entry.fileName);
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
    clientRoot,
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

  await mkdir(path.dirname(PUBLIC_SOUND_INDEX_PATH), { recursive: true });
  await mkdir(path.dirname(DOC_SOUND_INDEX_PATH), { recursive: true });
  await writeFile(PUBLIC_SOUND_INDEX_PATH, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  await writeFile(DOC_SOUND_INDEX_PATH, `${JSON.stringify(summary, null, 2)}\n`, "utf8");

  console.log(
    `Exported ${summary.exportedSoundCount}/${summary.soundListEntryCount} SoundList entries from ${summary.sourceWavCount} source wavs`,
  );
  if (missing.length) {
    console.warn(`Missing ${missing.length} SoundList wavs: ${missing.map((entry) => entry.fileName).join(", ")}`);
  }
  if (fallbackCount) {
    console.warn(
      `Fallback ${fallbackCount} SoundList wavs: ${fallbackSounds
        .map((entry) => `${entry.fileName}->${entry.fallbackSourceFileName}`)
        .join(", ")}`,
    );
  }
  if (strict && missing.length) {
    process.exitCode = 1;
  }
}

function indexSourceWavs(soundDir) {
  const files = new Map();
  for (const entry of readdirSync(soundDir, { withFileTypes: true })) {
    if (!entry.isFile() || !/\.wav$/i.test(entry.name)) {
      continue;
    }
    files.set(entry.name.toLowerCase(), path.join(soundDir, entry.name));
  }
  return files;
}

function parseSoundList(text) {
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
