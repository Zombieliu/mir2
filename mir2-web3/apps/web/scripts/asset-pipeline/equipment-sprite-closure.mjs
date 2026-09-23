import { readFile } from "node:fs/promises";
import path from "node:path";
import sharp from "sharp";
import { allPresentFrameIndices, decodeFrameRgba, parseLibrary } from "../crystal-library.mjs";
import { sha256 } from "./item-icon-closure.mjs";

const WEB_ROOT = path.resolve(import.meta.dirname, "../..");
export const DEFAULT_EQUIPMENT_REQUIREMENTS = path.join(import.meta.dirname, "natural-equipment-sprites.json");
export const DEFAULT_EQUIPMENT_ITEM_CATALOGUE = path.resolve(WEB_ROOT, "../../packages/game-data/data/generated/crystal_item_manifest.json");
export const DEFAULT_EQUIPMENT_SOURCE_CATALOGUE = path.resolve(WEB_ROOT, "../../docs/generated/assets/crystal-source-snapshot.generated.json");

// Crystal's ordinary CArmour/CHair body uses the 808 female offset; CWeapon
// uses 416. Check the complete ordinary (unmounted) action block, including
// movement, combat, death and revive, rather than accepting a hair-only actor.
export function collectEquippedSpriteRequirements(profiles, itemCatalogue) {
  if (!Array.isArray(profiles) || profiles.length === 0 || !Array.isArray(itemCatalogue?.items)) {
    throw new Error("Nonempty equipment profiles and item catalogue are required");
  }
  const items = new Map(itemCatalogue.items.map((item) => [`crystal-item-${item.item_index}`, item]));
  const result = new Map();
  for (const profile of profiles) {
    if (!["Warrior", "Wizard", "Taoist"].includes(profile.class)
        || !["Male", "Female"].includes(profile.gender)
        || !Number.isSafeInteger(profile.hair) || profile.hair < 0 || profile.hair > 14
        || !Array.isArray(profile.equipment)) {
      throw new Error("Unsupported ordinary C sprite profile");
    }
    const selected = new Map();
    for (const equipped of profile.equipment) {
      if (!["armour", "weapon"].includes(equipped.slot)) continue;
      const item = items.get(equipped.key);
      if (selected.has(equipped.slot) || !item
          || item.item_type !== (equipped.slot === "weapon" ? 1 : 2)
          || !Number.isSafeInteger(item.shape) || item.shape < 0 || item.shape >= 100) {
        throw new Error(`Invalid equipped ${equipped.slot}: ${equipped.key}`);
      }
      selected.set(equipped.slot, item.shape);
    }
    const shape = (value) => String(value).padStart(2, "0");
    const female = profile.gender === "Female";
    for (const [library, offset] of [
      [`CArmour/${shape(selected.get("armour") ?? 0)}`, female ? 808 : 0],
      [`CHair/${shape(profile.hair)}`, female ? 808 : 0],
      ...(selected.has("weapon") ? [[`CWeapon/${shape(selected.get("weapon"))}`, female ? 416 : 0]] : []),
    ]) {
      if (!result.has(library)) result.set(library, new Set());
      for (let frame = offset; frame < offset + 416; frame++) result.get(library).add(frame);
    }
  }
  return [...result].map(([library, frames]) => ({ library, frames: [...frames].sort((a, b) => a - b) }))
    .sort((a, b) => a.library.localeCompare(b.library));
}

export async function inspectEquippedSpriteClosure({
  assetRoot = path.join(WEB_ROOT, "public"),
  requirementsPath = DEFAULT_EQUIPMENT_REQUIREMENTS,
  itemCataloguePath = DEFAULT_EQUIPMENT_ITEM_CATALOGUE,
  sourceCataloguePath = DEFAULT_EQUIPMENT_SOURCE_CATALOGUE,
  sourceDataDir,
} = {}) {
  const profileBytes = await readFile(requirementsPath);
  const profileManifest = JSON.parse(profileBytes);
  if (profileManifest.schemaVersion !== 1 || profileManifest.kind !== "mir2-natural-equipment-sprite-requirements") {
    throw new Error("Equipment requirement schema/kind mismatch");
  }
  const requirements = collectEquippedSpriteRequirements(profileManifest.profiles, JSON.parse(await readFile(itemCataloguePath)));
  const catalogue = JSON.parse(await readFile(sourceCataloguePath));
  const origins = new Map(catalogue.libraries.map((library) => [library.path, library]));
  let atlas = { atlases: [] };
  try {
    atlas = JSON.parse(await readFile(path.join(assetRoot, "bevy-entity-atlases/manifest.json")));
  } catch (error) {
    // A first atlas build can use a complete, verified standalone export.
    // An existing malformed manifest must still fail rather than be ignored.
    if (error.code !== "ENOENT") throw error;
  }
  const atlasFrames = new Map();
  for (const family of atlas.atlases ?? []) {
    const pages = family.pages ?? [family];
    for (const rect of family.rects ?? []) {
      const framePath = String(rect.key ?? "").split("|", 1)[0];
      if (!requirements.some(({ library }) => framePath.startsWith(`/original-ui/${library}/`))) continue;
      atlasFrames.set(framePath, { rect, page: pages[rect.pageIndex ?? 0] });
    }
  }
  const errors = [];
  const checkedPages = new Map();
  const libraries = [];
  let atlasFrameCount = 0;
  let standaloneFrameCount = 0;
  let sourceFrameCount = 0;
  for (const required of requirements) {
    const errorsBeforeLibrary = errors.length;
    const fallback = required.frames.filter((frame) => !atlasFrames.has(`/original-ui/${required.library}/${frame}.png`));
    let meta;
    let original;
    const frames = new Map();
    if (fallback.length) {
      try {
        meta = JSON.parse(await readFile(path.join(assetRoot, "original-ui", required.library, "meta.json")));
        const origin = origins.get(`${required.library}.Lib`);
        if (!origin || meta.sourceLibrary?.path !== origin.path || meta.sourceLibrary.sha256 !== origin.sha256
            || meta.sourceLibrary.bytes !== origin.byteLength || !Array.isArray(meta.frames)) {
          throw new Error("standalone source identity/metadata mismatch");
        }
        for (const frame of meta.frames) {
          if (frames.has(frame.index)) throw new Error("duplicate standalone frame metadata");
          frames.set(frame.index, frame);
        }
        if (sourceDataDir) {
          const bytes = await readFile(path.join(sourceDataDir, `${required.library}.Lib`));
          if (sha256(bytes) !== origin.sha256 || bytes.length !== origin.byteLength) throw new Error("original library hash mismatch");
          original = parseLibrary(bytes);
        }
      } catch (error) {
        errors.push({ library: required.library, reason: error.message });
      }
    }
    const checkedFrames = original ? allPresentFrameIndices(original) : required.frames;
    let libraryStandalone = 0;
    for (const frameIndex of checkedFrames) {
      const framePath = `/original-ui/${required.library}/${frameIndex}.png`;
      const binding = atlasFrames.get(framePath);
      try {
        if (binding) {
          const { rect, page } = binding;
          if (!page || !/^\/bevy-entity-atlases\/[A-Za-z0-9._-]+\.png$/.test(page.imageUrl ?? "")
              || !/^[a-f0-9]{64}$/.test(page.sha256 ?? "")) throw new Error("atlas page is not integrity-bound");
          if (!checkedPages.has(page.imageUrl)) {
            const bytes = await readFile(path.join(assetRoot, page.imageUrl.slice(1)));
            const decoded = await sharp(bytes).metadata();
            if (sha256(bytes) !== page.sha256 || bytes.length !== page.imageBytes
                || decoded.width !== page.width || decoded.height !== page.height) throw new Error("atlas page hash/dimensions mismatch");
            checkedPages.set(page.imageUrl, page.sha256);
          }
          if (checkedPages.get(page.imageUrl) !== page.sha256) throw new Error("conflicting atlas page identity");
          if (![rect.x, rect.y, rect.width, rect.height, rect.offsetX, rect.offsetY].every(Number.isSafeInteger)
              || rect.x < 0 || rect.y < 0 || rect.width <= 0 || rect.height <= 0
              || rect.x + rect.width > page.width || rect.y + rect.height > page.height) throw new Error("invalid atlas frame geometry");
          if (original) {
            const source = original.frames[frameIndex];
            if (source.width !== rect.width || source.height !== rect.height || source.x !== rect.offsetX || source.y !== rect.offsetY) {
              throw new Error("atlas frame differs from original geometry/offset");
            }
          }
          atlasFrameCount++;
          // With originals supplied, also prove every standalone exported
          // frame. An atlas binding must not hide a corrupt local fallback.
          if (!original) continue;
        }
        const frame = frames.get(frameIndex);
        if (!frame || frame.path !== framePath || ![frame.width, frame.height, frame.x, frame.y].every(Number.isSafeInteger)
            || frame.width <= 0 || frame.height <= 0 || !/^[a-f0-9]{64}$/.test(frame.pngSha256 ?? "")
            || !/^[a-f0-9]{64}$/.test(frame.rgbaSha256 ?? "")) throw new Error("missing/invalid standalone frame metadata");
        const png = await readFile(path.join(assetRoot, framePath.slice(1)));
        const { data, info } = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
        if (sha256(png) !== frame.pngSha256 || sha256(data) !== frame.rgbaSha256
            || info.width !== frame.width || info.height !== frame.height || info.channels !== 4) throw new Error("standalone PNG/RGBA hash or dimensions mismatch");
        if (original) {
          const source = original.frames[frameIndex];
          if (source.width !== frame.width || source.height !== frame.height || source.x !== frame.x || source.y !== frame.y
              || sha256(decodeFrameRgba(original, source)) !== frame.rgbaSha256) throw new Error("standalone frame differs from original art/offset");
          sourceFrameCount++;
        }
        standaloneFrameCount++;
        libraryStandalone++;
      } catch (error) {
        errors.push({ library: required.library, frame: frameIndex, reason: error.message });
      }
    }
    libraries.push({ library: required.library, requiredFrames: required.frames.length,
      checkedStandaloneFrames: libraryStandalone, errorCount: errors.length - errorsBeforeLibrary });
  }
  const firstErrorByLibrary = new Map();
  for (const error of errors) {
    const first = firstErrorByLibrary.get(error.library);
    if (!first || (first.frame !== undefined && error.frame === undefined)) firstErrorByLibrary.set(error.library, error);
  }
  const sampled = [...firstErrorByLibrary.values()];
  return { ok: errors.length === 0, requirementsSha256: sha256(profileBytes), libraries,
    atlasFrameCount, standaloneFrameCount, sourceFrameCount, checkedAtlasPages: checkedPages.size,
    errorCount: errors.length,
    errors: [...sampled, ...errors.filter((entry) => !sampled.includes(entry))].slice(0, 12) };
}

export function assertEquippedSpriteClosure(report) {
  if (!report.ok) throw new Error(`Equipped sprite closure failed: ${JSON.stringify(report)}`);
  return report;
}
