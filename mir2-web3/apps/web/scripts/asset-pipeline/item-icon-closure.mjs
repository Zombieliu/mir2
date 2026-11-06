import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import sharp from "sharp";

const WEB_ROOT = path.resolve(import.meta.dirname, "../..");
export const DEFAULT_ITEM_MANIFEST_PATH = path.resolve(
  WEB_ROOT, "../../packages/game-data/data/generated/crystal_item_manifest.json",
);
export const DEFAULT_ITEM_ICON_ROOT = path.join(WEB_ROOT, "public/original-ui/Items");

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

// MirItemCell.cs:2511+ draws UserItem.Image, not ItemInfo.Index or a name guess.
// ItemData.cs:641-681 normally returns Info.Image, but stackable Amulet shapes
// 0/1/2 choose these source frames by Count. Keep every threshold image even
// though none of those eleven frames occurs in the current catalogue.Image.
const AMULET_STACK_IMAGES = new Map([
  [0, [3660, 3661, 3662]],
  [1, [3673, 3674, 2960, 3675]],
  [2, [3670, 3671, 2961, 3672]],
]);

// Include every catalogue row, including all class/level variants used by
// equipment and preview surfaces, in addition to the UserItem count rules.
export function collectItemIconRequirements(manifest) {
  if (!Array.isArray(manifest?.items) || manifest.items.length === 0) {
    throw new Error("Item catalogue must contain a non-empty items array");
  }
  const indexes = new Set();
  const requirements = manifest.items.map((item) => {
    const itemIndex = item?.item_index;
    const image = item?.image;
    if (!Number.isSafeInteger(itemIndex) || itemIndex < 0 || indexes.has(itemIndex)) {
      throw new Error(`Invalid or duplicate item catalogue index: ${itemIndex}`);
    }
    if (!Number.isSafeInteger(image) || image < 0 || image > 65535) {
      throw new Error(`Invalid item image for catalogue index ${itemIndex}: ${image}`);
    }
    if (!Number.isSafeInteger(item.item_type) || item.item_type < 0 || item.item_type > 255
      || !Number.isSafeInteger(item.shape) || item.shape < -32768 || item.shape > 32767
      || !Number.isSafeInteger(item.stack_size) || item.stack_size < 0 || item.stack_size > 65535) {
      throw new Error(`Invalid item type/shape/stack size for catalogue index ${itemIndex}`);
    }
    indexes.add(itemIndex);
    const stackImages = item.item_type === 8 && item.stack_size > 0
      ? [...(AMULET_STACK_IMAGES.get(item.shape) ?? [])] : [];
    return { itemIndex, name: String(item.name ?? ""), image, stackImages };
  }).sort((a, b) => a.itemIndex - b.itemIndex);
  const uniqueCatalogueImages = [...new Set(requirements.map((item) => item.image))].sort((a, b) => a - b);
  const uniqueStackImages = [...new Set(requirements.flatMap((item) => item.stackImages))].sort((a, b) => a - b);
  return {
    requirements,
    uniqueCatalogueImages,
    uniqueStackImages,
    uniqueImages: [...new Set([...uniqueCatalogueImages, ...uniqueStackImages])].sort((a, b) => a - b),
  };
}

export async function inspectItemIconClosure({
  itemManifestPath = DEFAULT_ITEM_MANIFEST_PATH,
  itemIconRoot = DEFAULT_ITEM_ICON_ROOT,
} = {}) {
  const catalogueBytes = await readFile(itemManifestPath);
  const { requirements, uniqueCatalogueImages, uniqueStackImages, uniqueImages } =
    collectItemIconRequirements(JSON.parse(catalogueBytes));
  const meta = JSON.parse(await readFile(path.join(itemIconRoot, "meta.json"), "utf8"));
  const invalidMetadata = [];
  const missingMetadata = [];
  const missingFiles = [];
  const invalidImages = [];
  const frames = new Map();

  if (!Array.isArray(meta.frames)) {
    throw new Error("Item icon metadata must contain a frames array");
  }
  if (meta.sourceLibrary?.path !== "Items.Lib"
    || !/^[a-f0-9]{64}$/.test(meta.sourceLibrary?.sha256 ?? "")
    || !Number.isSafeInteger(meta.sourceLibrary?.bytes) || meta.sourceLibrary.bytes <= 0) {
    invalidMetadata.push({ reason: "missing-source-library-identity" });
  }
  if (meta.itemCatalogue?.sha256 !== sha256(catalogueBytes)
    || meta.itemCatalogue?.itemCount !== requirements.length
    || meta.itemCatalogue?.catalogueImageCount !== uniqueCatalogueImages.length
    || meta.itemCatalogue?.stackImageCount !== uniqueStackImages.length
    || meta.itemCatalogue?.uniqueImageCount !== uniqueImages.length) {
    invalidMetadata.push({ reason: "catalogue-identity-or-denominator-mismatch" });
  }
  for (const frame of meta.frames) {
    const index = frame?.index;
    if (!Number.isSafeInteger(index) || index < 0 || frames.has(index)) {
      invalidMetadata.push({ index, reason: "invalid-or-duplicate-frame-index" });
      continue;
    }
    frames.set(index, frame);
    if (frame.path !== `/original-ui/Items/${index}.png`
      || !Number.isSafeInteger(frame.width) || frame.width <= 0
      || !Number.isSafeInteger(frame.height) || frame.height <= 0
      || !Number.isSafeInteger(frame.x) || !Number.isSafeInteger(frame.y)) {
      invalidMetadata.push({ index, reason: "invalid-frame-path-or-geometry" });
    }
  }

  // Bound decoding work rather than creating a worker for every catalogue row.
  for (let start = 0; start < uniqueImages.length; start += 16) {
    await Promise.all(uniqueImages.slice(start, start + 16).map(async (index) => {
      const frame = frames.get(index);
      if (!frame) missingMetadata.push(index);
      let png;
      try {
        png = await readFile(path.join(itemIconRoot, `${index}.png`));
      } catch (error) {
        if (error.code !== "ENOENT") throw error;
        missingFiles.push(index);
        return;
      }
      try {
        const { data, info } = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
        if (!frame || info.channels !== 4 || info.width !== frame.width || info.height !== frame.height) {
          invalidImages.push({ index, reason: "png-metadata-geometry-mismatch" });
        }
        if (!/^[a-f0-9]{64}$/.test(frame?.rgbaSha256 ?? "") || sha256(data) !== frame.rgbaSha256) {
          invalidImages.push({ index, reason: "source-pixel-hash-mismatch" });
        }
        if (!/^[a-f0-9]{64}$/.test(frame?.pngSha256 ?? "") || sha256(png) !== frame.pngSha256) {
          invalidImages.push({ index, reason: "png-file-hash-mismatch" });
        }
      } catch {
        invalidImages.push({ index, reason: "invalid-png" });
      }
    }));
  }
  missingFiles.sort((a, b) => a - b);
  missingMetadata.sort((a, b) => a - b);
  invalidImages.sort((a, b) => a.index - b.index || a.reason.localeCompare(b.reason));
  return {
    itemCount: requirements.length,
    catalogueImageCount: uniqueCatalogueImages.length,
    stackImageCount: uniqueStackImages.length,
    uniqueImageCount: uniqueImages.length,
    requirements,
    uniqueCatalogueImages,
    uniqueStackImages,
    uniqueImages,
    sourceLibrary: meta.sourceLibrary,
    itemCatalogue: meta.itemCatalogue,
    missingFiles,
    missingMetadata,
    invalidMetadata,
    invalidImages,
  };
}

export async function assertItemIconClosure(options) {
  const report = await inspectItemIconClosure(options);
  if (report.missingFiles.length || report.missingMetadata.length
    || report.invalidMetadata.length || report.invalidImages.length) {
    throw new Error(`Catalogue item icon closure is incomplete: ${JSON.stringify({
      missingFiles: { count: report.missingFiles.length, first: report.missingFiles.slice(0, 16) },
      missingMetadata: { count: report.missingMetadata.length, first: report.missingMetadata.slice(0, 16) },
      invalidMetadata: { count: report.invalidMetadata.length, first: report.invalidMetadata.slice(0, 16) },
      invalidImages: { count: report.invalidImages.length, first: report.invalidImages.slice(0, 16) },
    })}`);
  }
  return report;
}
