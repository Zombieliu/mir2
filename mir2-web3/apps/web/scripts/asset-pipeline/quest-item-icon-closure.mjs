import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const WEB_ROOT = path.resolve(import.meta.dirname, "../..");
const MIR2_ROOT = path.resolve(WEB_ROOT, "../..");

export const DEFAULT_QUEST_MANIFEST_PATH = path.join(
  MIR2_ROOT,
  "packages/game-data/data/generated/crystal_quest_packet_manifest.json",
);
export const DEFAULT_ITEM_MANIFEST_PATH = path.join(
  MIR2_ROOT,
  "packages/game-data/data/generated/crystal_item_manifest.json",
);
export const DEFAULT_ITEM_ICON_ROOT = path.join(WEB_ROOT, "public/original-ui/Items");

export function inspectQuestItemIconClosure({
  questManifestPath = DEFAULT_QUEST_MANIFEST_PATH,
  itemManifestPath = DEFAULT_ITEM_MANIFEST_PATH,
  itemIconRoot = DEFAULT_ITEM_ICON_ROOT,
} = {}) {
  const questManifest = readJson(questManifestPath);
  const itemManifest = readJson(itemManifestPath);
  const itemTemplates = new Map(
    (itemManifest.items ?? []).map((item) => [String(item.name), item]),
  );
  const questItemNames = [...new Set(
    (questManifest.quests ?? []).flatMap((quest) => [
      ...(quest.carry_items ?? []),
      ...(quest.item_tasks ?? []),
    ]).map((item) => String(item.item_name)),
  )].sort((left, right) => left.localeCompare(right));
  const metadataPath = path.join(itemIconRoot, "meta.json");
  const metadata = readJson(metadataPath);
  const metadataFrames = new Set(
    (metadata.frames ?? []).map((frame) => Number(frame.index)),
  );
  const missingTemplates = [];
  const requirements = [];

  for (const name of questItemNames) {
    const template = itemTemplates.get(name);
    const image = Number(template?.image);
    if (!template || !Number.isInteger(image) || image < 0) {
      missingTemplates.push(name);
      continue;
    }
    requirements.push({ name, itemIndex: Number(template.item_index), image });
  }

  const uniqueImages = [...new Set(requirements.map((item) => item.image))]
    .sort((left, right) => left - right);
  const missingFiles = uniqueImages.filter(
    (image) => !existsSync(path.join(itemIconRoot, `${image}.png`)),
  );
  const missingMetadata = uniqueImages.filter((image) => !metadataFrames.has(image));

  return {
    questItemCount: questItemNames.length,
    uniqueImageCount: uniqueImages.length,
    requirements,
    uniqueImages,
    requiredPaths: uniqueImages.map((image) => `/original-ui/Items/${image}.png`),
    missingTemplates,
    missingFiles,
    missingMetadata,
  };
}

export function assertQuestItemIconClosure(options) {
  const report = inspectQuestItemIconClosure(options);
  if (
    report.missingTemplates.length > 0 ||
    report.missingFiles.length > 0 ||
    report.missingMetadata.length > 0
  ) {
    throw new Error(
      `Quest item icon closure is incomplete: ${JSON.stringify({
        missingTemplates: report.missingTemplates,
        missingFiles: report.missingFiles,
        missingMetadata: report.missingMetadata,
      })}`,
    );
  }
  return report;
}

function readJson(filePath) {
  return JSON.parse(readFileSync(filePath, "utf8"));
}
