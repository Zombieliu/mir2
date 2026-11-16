import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import sharp from "sharp";
import { assertEquippedSpriteClosure, inspectEquippedSpriteClosure } from "./asset-pipeline/equipment-sprite-closure.mjs";
import { sha256 } from "./asset-pipeline/item-icon-closure.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const PUBLIC_ROOT = path.join(WEB_ROOT, "public");
const DEFAULT_ROOTS = [
  "CArmour/00",
  "CHair/00",
  "CWeapon/00",
  // Crystal harvest always draws CWeapon/01 even when the equipped weapon is
  // the starter CWeapon/00. Keep that authoritative VIS-01 layer local.
  "CWeapon/01",
  "AArmour/00",
  "AHair/00",
  "AWeapon/00 L",
  "AWeapon/00 R",
  "NPC",
  "Monster/000",
  "Monster/003",
  "Monster/004",
  "Monster/005",
  "Monster/006",
  "Monster/007",
  "Monster/010",
  "Monster/012",
  "Monster/139",
];
const ATLAS_PADDING = 1;
const INITIAL_WIDTH = 512;
const MAX_SIZE = 4096;

const args = parseArgs(process.argv.slice(2));
const assetRoot = path.resolve(args.assetRoot ?? PUBLIC_ROOT);
const originalUiRoot = path.join(assetRoot, "original-ui");
const outDir = path.resolve(args.outDir ?? path.join(assetRoot, "bevy-entity-atlases"));
const atlasKey = String(args.atlasKey ?? "starter-bichon-base");
const roots = parseListArg(args.roots, DEFAULT_ROOTS);

async function main() {
  // Keep the existing texture budget: later equipment can use verified local
  // standalone frames. A base package still must close the equipped action
  // blocks, even when those libraries are not packed into the starter atlas.
  if (!args.roots || args.verifyEquipmentClosure === "true" || args.equipmentRequirements) {
    const closure = assertEquippedSpriteClosure(await inspectEquippedSpriteClosure({
      assetRoot,
      requirementsPath: args.equipmentRequirements,
      itemCataloguePath: args.equipmentItemCatalogue,
      sourceCataloguePath: args.equipmentSourceCatalogue,
      sourceDataDir: args.sourceDataDir,
    }));
    console.log(JSON.stringify({ stage: "equipped-sprite-closure", ...closure }));
    if (args.verifyEquipmentClosure === "true") return;
  }
  const sources = await collectSources(roots);
  if (!sources.length) {
    throw new Error("No PNG sources found for Bevy entity atlas pack");
  }

  const packed = packSources(sources);
  const imageFileName = `${atlasKey}.png`;
  const imagePath = path.join(outDir, imageFileName);
  const manifestPath = path.join(outDir, "manifest.json");
  // A bounded library repair must retain existing multi-page starter atlases.
  // Read before writing pixels so a missing/malformed base fails without changes.
  const existingManifest = args.append === "true"
    ? JSON.parse(await fs.readFile(manifestPath, "utf8"))
    : null;
  if (existingManifest && (existingManifest.schemaVersion !== 2 || !Array.isArray(existingManifest.atlases))) {
    throw new Error("Append requires a schema-v2 entity atlas manifest");
  }

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

  const imageBytes = await fs.readFile(imagePath);
  const imageByteLength = imageBytes.length;
  const manifest = {
    ...existingManifest,
    schemaVersion: 2,
    kind: "mir2-bevy-entity-atlas-manifest",
    generatedAt: new Date().toISOString(),
    atlases: [
      {
        key: atlasKey,
        label: `Entity atlas: ${atlasKey}`,
        width: packed.width,
        height: packed.height,
        sourceCount: sources.length,
        imageBytes: imageByteLength,
        rgbaBytes: packed.width * packed.height * 4,
        roots,
        imageUrl: `/bevy-entity-atlases/${imageFileName}`,
        sha256: sha256(imageBytes),
        pages: [{
          imageUrl: `/bevy-entity-atlases/${imageFileName}`,
          width: packed.width,
          height: packed.height,
          imageBytes: imageByteLength,
          sha256: sha256(imageBytes),
        }],
        rects: packed.sources.map((source) => ({
          key: source.key,
          x: source.x,
          y: source.y,
          width: source.width,
          height: source.height,
          pageIndex: 0,
          offsetX: source.offsetX,
          offsetY: source.offsetY,
        })),
      },
    ],
    stats: {
      sourceCount: sources.length,
      roots,
      imageBytes: imageByteLength,
      rgbaBytes: packed.width * packed.height * 4,
    },
  };

  if (existingManifest) {
    manifest.atlases = [
      ...existingManifest.atlases.filter((atlas) => atlas.key !== atlasKey),
      ...manifest.atlases,
    ];
    manifest.stats = {
      ...existingManifest.stats,
      sourceCount: manifest.atlases.reduce((sum, atlas) => sum + atlas.sourceCount, 0),
      roots: [...new Set(manifest.atlases.flatMap((atlas) => atlas.roots))],
      imageBytes: manifest.atlases.reduce((sum, atlas) => sum + atlas.pages.reduce((bytes, page) => bytes + page.imageBytes, 0), 0),
      rgbaBytes: manifest.atlases.reduce((sum, atlas) => sum + atlas.pages.reduce((bytes, page) => bytes + page.width * page.height * 4, 0), 0),
      pageCount: manifest.atlases.reduce((sum, atlas) => sum + atlas.pages.length, 0),
    };
  }
  await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  console.log(
    JSON.stringify(
      {
        ok: true,
        atlasKey,
        sourceCount: sources.length,
        width: packed.width,
        height: packed.height,
        imageBytes: imageByteLength,
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
    const rootDir = path.join(originalUiRoot, root);
    if (!(await exists(rootDir))) {
      continue;
    }
    const files = await walkPngFiles(rootDir);
    let frameMetadata = new Map();
    try {
      const meta = JSON.parse(await fs.readFile(path.join(rootDir, "meta.json")));
      frameMetadata = new Map((meta.frames ?? []).map((frame) => [frame.path, frame]));
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    for (const filePath of files) {
      const metadata = await sharp(filePath).metadata();
      const width = positiveInteger(metadata.width);
      const height = positiveInteger(metadata.height);
      if (!width || !height) {
        continue;
      }
      const urlPath = publicUrlForFile(filePath);
      const original = frameMetadata.get(urlPath);
      if (original && (!Number.isSafeInteger(original.x) || !Number.isSafeInteger(original.y)
          || original.width !== width || original.height !== height)) {
        throw new Error(`Original frame geometry mismatch: ${urlPath}`);
      }
      sources.push({
        filePath,
        path: urlPath,
        key: `${urlPath}|${width}x${height}`,
        width,
        height,
        offsetX: original?.x ?? 0,
        offsetY: original?.y ?? 0,
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
  const relative = path.relative(assetRoot, filePath).split(path.sep);
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
