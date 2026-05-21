import { mkdir, readFile, stat, writeFile } from "node:fs/promises";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const DEFAULT_CLIENT_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const PUBLIC_UI_INDEX_PATH = path.join(WORKSPACE_ROOT, "public", "original-ui", "source-libraries.generated.json");
const FULL_INDEX_PATH = path.join(REPO_ROOT, "docs", "generated", "assets", "full-crystal-client-index.json");
const MIN_LIBRARY_COUNT = Number(process.env.MIR2_MIN_CRYSTAL_LIBRARY_COUNT ?? 1000);
const MIN_LIBRARY_FRAME_COUNT = Number(process.env.MIR2_MIN_CRYSTAL_LIBRARY_FRAME_COUNT ?? 1000000);
const MIN_MAP_COUNT = Number(process.env.MIR2_MIN_CRYSTAL_MAP_COUNT ?? 400);
const MIN_SOUND_COUNT = Number(process.env.MIR2_MIN_CRYSTAL_SOUND_COUNT ?? 1000);

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

async function main() {
  const clientRoot = path.resolve(process.argv[2] ?? process.env.CRYSTAL_CLIENT_ROOT ?? DEFAULT_CLIENT_ROOT);
  const dataDir = path.basename(clientRoot).toLowerCase() === "data" ? clientRoot : path.join(clientRoot, "Data");
  const mapDir = path.basename(clientRoot).toLowerCase() === "data" ? path.join(path.dirname(clientRoot), "Map") : path.join(clientRoot, "Map");
  const soundDir = path.basename(clientRoot).toLowerCase() === "data" ? path.join(path.dirname(clientRoot), "Sound") : path.join(clientRoot, "Sound");

  const libraries = {};
  const categories = {};
  for (const filePath of walkFiles(dataDir, (name) => /\.lib$/i.test(name))) {
    const rel = normalizePath(path.relative(dataDir, filePath)).replace(/\.Lib$/i, "");
    const info = await readLibraryHeader(filePath);
    const category = rel.includes("/") ? rel.split("/")[0] : "(root)";
    libraries[rel] = {
      category,
      version: info.version,
      frames: info.frames,
      sourceBytes: info.sourceBytes,
    };
    const current = categories[category] ?? { libraries: 0, frames: 0, sourceBytes: 0 };
    current.libraries += 1;
    current.frames += info.frames;
    current.sourceBytes += info.sourceBytes;
    categories[category] = current;
  }

  const maps = walkFiles(mapDir, (name) => /\.map$/i.test(name));
  const sounds = walkFiles(soundDir, (name) => /\.wav$/i.test(name));
  const fullIndex = {
    generatedAt: new Date().toISOString(),
    clientRoot,
    dataDir,
    mapDir,
    soundDir,
    libraryCount: Object.keys(libraries).length,
    libraryFrameCount: Object.values(libraries).reduce((sum, entry) => sum + entry.frames, 0),
    categories,
    maps: {
      count: maps.length,
      sourceBytes: await totalBytes(maps),
    },
    sounds: {
      count: sounds.length,
      sourceBytes: await totalBytes(sounds),
    },
    libraries,
  };
  const validationFailures = validateFullClientIndex(fullIndex);
  if (validationFailures.length > 0 && process.env.MIR2_ALLOW_PARTIAL_CRYSTAL_ASSETS !== "1") {
    throw new Error(`Crystal full-client asset index is incomplete:\n- ${validationFailures.join("\n- ")}`);
  }

  await mkdir(path.dirname(PUBLIC_UI_INDEX_PATH), { recursive: true });
  await mkdir(path.dirname(FULL_INDEX_PATH), { recursive: true });
  await writeFile(
    PUBLIC_UI_INDEX_PATH,
    `${JSON.stringify(
      {
        generatedAt: fullIndex.generatedAt,
        libraryCount: fullIndex.libraryCount,
        libraries,
      },
      null,
      2,
    )}\n`,
    "utf8",
  );
  await writeFile(FULL_INDEX_PATH, `${JSON.stringify(fullIndex, null, 2)}\n`, "utf8");

  console.log(
    `Indexed ${fullIndex.libraryCount} libraries, ${fullIndex.maps.count} maps, ${fullIndex.sounds.count} sounds`,
  );
  if (validationFailures.length > 0) {
    console.warn(`Partial Crystal asset index allowed:\n- ${validationFailures.join("\n- ")}`);
  }
}

function walkFiles(root, predicate) {
  if (!existsSync(root)) {
    return [];
  }

  const files = [];
  const stack = [root];
  while (stack.length) {
    const dir = stack.pop();
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const filePath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        stack.push(filePath);
      } else if (predicate(entry.name)) {
        files.push(filePath);
      }
    }
  }
  return files.sort();
}

async function readLibraryHeader(filePath) {
  const buffer = await readFile(filePath);
  let offset = 0;
  const version = buffer.readInt32LE(offset);
  offset += 4;
  const frames = buffer.readInt32LE(offset);
  return {
    version,
    frames,
    sourceBytes: buffer.length,
  };
}

async function totalBytes(files) {
  let total = 0;
  for (const filePath of files) {
    total += (await stat(filePath)).size;
  }
  return total;
}

function normalizePath(value) {
  return value.split(path.sep).join("/");
}

function validateFullClientIndex(index) {
  const failures = [];
  if (index.libraryCount < MIN_LIBRARY_COUNT) {
    failures.push(`library count ${index.libraryCount} < ${MIN_LIBRARY_COUNT}`);
  }
  if (index.libraryFrameCount < MIN_LIBRARY_FRAME_COUNT) {
    failures.push(`library frame count ${index.libraryFrameCount} < ${MIN_LIBRARY_FRAME_COUNT}`);
  }
  if (index.maps.count < MIN_MAP_COUNT) {
    failures.push(`map count ${index.maps.count} < ${MIN_MAP_COUNT}`);
  }
  if (index.sounds.count < MIN_SOUND_COUNT) {
    failures.push(`sound count ${index.sounds.count} < ${MIN_SOUND_COUNT}`);
  }
  return failures;
}
