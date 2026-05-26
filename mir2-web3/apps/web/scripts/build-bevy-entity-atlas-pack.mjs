import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const ORIGINAL_UI_ROOT = path.join(PUBLIC_ROOT, "original-ui");
const DEFAULT_OUT_DIR = path.join(PUBLIC_ROOT, "bevy-entity-atlases");
const DEFAULT_ROOTS = [
  "CArmour/00",
  "CHair/00",
  "CWeapon/00",
  "AArmour/00",
  "AHair/00",
  "AWeapon/00 L",
  "AWeapon/00 R",
  "NPC",
  "Monster/000",
  "Monster/010",
  "Monster/012",
  "Monster/139",
];
const ATLAS_PADDING = 1;
const INITIAL_WIDTH = 512;
const MAX_SIZE = 4096;

const args = parseArgs(process.argv.slice(2));
const outDir = path.resolve(args.outDir ?? DEFAULT_OUT_DIR);
const atlasKey = String(args.atlasKey ?? "starter-bichon-base");
const roots = parseListArg(args.roots, DEFAULT_ROOTS);

async function main() {
  const sources = await collectSources(roots);
  if (!sources.length) {
    throw new Error("No PNG sources found for Bevy entity atlas pack");
  }

  const packed = packSources(sources);
  const imageFileName = `${atlasKey}.png`;
  const imagePath = path.join(outDir, imageFileName);
  const manifestPath = path.join(outDir, "manifest.json");

  await fs.mkdir(outDir, { recursive: true });
  await sharp({
    create: {
      width: packed.width,
      height: packed.height,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite(
      packed.sources.map((source) => ({
        input: source.filePath,
        left: source.x,
        top: source.y,
      })),
    )
    .png({ compressionLevel: 9, adaptiveFiltering: true })
    .toFile(imagePath);

  const imageStat = await fs.stat(imagePath);
  const manifest = {
    schemaVersion: 1,
    kind: "mir2-bevy-entity-atlas-manifest",
    generatedAt: new Date().toISOString(),
    atlases: [
      {
        key: atlasKey,
        label: "Starter Bichon base player/NPC entity atlas",
        width: packed.width,
        height: packed.height,
        sourceCount: sources.length,
        imageBytes: imageStat.size,
        rgbaBytes: packed.width * packed.height * 4,
        roots,
        imageUrl: `/bevy-entity-atlases/${imageFileName}`,
        rects: packed.sources.map((source) => ({
          key: source.key,
          x: source.x,
          y: source.y,
          width: source.width,
          height: source.height,
        })),
      },
    ],
    stats: {
      sourceCount: sources.length,
      roots,
      imageBytes: imageStat.size,
      rgbaBytes: packed.width * packed.height * 4,
    },
  };

  await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  console.log(
    JSON.stringify(
      {
        ok: true,
        atlasKey,
        sourceCount: sources.length,
        width: packed.width,
        height: packed.height,
        imageBytes: imageStat.size,
        rgbaBytes: packed.width * packed.height * 4,
        manifestPath,
        imagePath,
      },
      null,
      2,
    ),
  );
}

async function collectSources(roots) {
  const sources = [];
  for (const root of roots) {
    const rootDir = path.join(ORIGINAL_UI_ROOT, root);
    if (!(await exists(rootDir))) {
      continue;
    }
    const files = await walkPngFiles(rootDir);
    for (const filePath of files) {
      const metadata = await sharp(filePath).metadata();
      const width = positiveInteger(metadata.width);
      const height = positiveInteger(metadata.height);
      if (!width || !height) {
        continue;
      }
      const urlPath = publicUrlForFile(filePath);
      sources.push({
        filePath,
        path: urlPath,
        key: `${urlPath}|${width}x${height}`,
        width,
        height,
      });
    }
  }
  return sources.sort((a, b) => a.key.localeCompare(b.key));
}

async function walkPngFiles(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const entryPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await walkPngFiles(entryPath)));
    } else if (entry.isFile() && entry.name.toLowerCase().endsWith(".png")) {
      files.push(entryPath);
    }
  }
  return files;
}

function packSources(sources) {
  const sortedSources = [...sources].sort(
    (a, b) => b.height - a.height || b.width - a.width || a.key.localeCompare(b.key),
  );
  const widest = sortedSources.reduce((max, source) => Math.max(max, source.width + ATLAS_PADDING * 2), 1);
  let width = Math.max(INITIAL_WIDTH, nextPowerOfTwo(widest));

  while (width <= MAX_SIZE) {
    let cursorX = ATLAS_PADDING;
    let cursorY = ATLAS_PADDING;
    let rowHeight = 0;
    const packed = [];

    for (const source of sortedSources) {
      if (cursorX + source.width + ATLAS_PADDING > width) {
        cursorX = ATLAS_PADDING;
        cursorY += rowHeight + ATLAS_PADDING;
        rowHeight = 0;
      }

      packed.push({
        ...source,
        x: cursorX,
        y: cursorY,
      });
      cursorX += source.width + ATLAS_PADDING;
      rowHeight = Math.max(rowHeight, source.height);
    }

    const height = nextPowerOfTwo(cursorY + rowHeight + ATLAS_PADDING);
    if (height <= MAX_SIZE) {
      return {
        width,
        height,
        sources: packed.sort((a, b) => a.key.localeCompare(b.key)),
      };
    }
    width *= 2;
  }

  throw new Error(`Atlas sources exceed ${MAX_SIZE}px texture budget`);
}

function publicUrlForFile(filePath) {
  const relative = path.relative(PUBLIC_ROOT, filePath).split(path.sep);
  return `/${relative.map((part) => encodeURIComponent(part)).join("/")}`;
}

function nextPowerOfTwo(value) {
  return 2 ** Math.ceil(Math.log2(Math.max(1, value)));
}

function positiveInteger(value) {
  return Number.isFinite(value) && value > 0 ? Math.trunc(value) : 0;
}

async function exists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index];
    if (!value.startsWith("--")) {
      continue;
    }
    const [rawKey, inlineValue] = value.slice(2).split("=", 2);
    const key = rawKey.trim();
    if (!key) {
      continue;
    }
    if (inlineValue !== undefined) {
      parsed[key] = inlineValue;
    } else {
      parsed[key] = argv[index + 1] && !argv[index + 1].startsWith("--") ? argv[++index] : "true";
    }
  }
  return parsed;
}

function parseListArg(value, fallback) {
  if (!value) {
    return fallback;
  }
  return String(value)
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
