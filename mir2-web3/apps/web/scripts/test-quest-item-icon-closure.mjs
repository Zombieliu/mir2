import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import {
  assertQuestItemIconClosure,
  inspectQuestItemIconClosure,
} from "./asset-pipeline/quest-item-icon-closure.mjs";

test("quest item icon closure detects and closes missing PNG and metadata frames", () => {
  const root = mkdtempSync(path.join(tmpdir(), "mir2-quest-item-icons-"));
  const questManifestPath = path.join(root, "quests.json");
  const itemManifestPath = path.join(root, "items.json");
  const itemIconRoot = path.join(root, "Items");
  mkdirSync(itemIconRoot, { recursive: true });
  writeJson(questManifestPath, {
    quests: [{ carry_items: [{ item_name: "QuestLeaf" }], item_tasks: [] }],
  });
  writeJson(itemManifestPath, {
    items: [{ name: "QuestLeaf", item_index: 1001, image: 412 }],
  });
  writeJson(path.join(itemIconRoot, "meta.json"), { frames: [] });

  try {
    const incomplete = inspectQuestItemIconClosure({
      questManifestPath,
      itemManifestPath,
      itemIconRoot,
    });
    assert.deepEqual(incomplete.missingFiles, [412]);
    assert.deepEqual(incomplete.missingMetadata, [412]);
    assert.throws(
      () => assertQuestItemIconClosure({ questManifestPath, itemManifestPath, itemIconRoot }),
      /Quest item icon closure is incomplete/,
    );

    writeFileSync(path.join(itemIconRoot, "412.png"), "fixture");
    writeJson(path.join(itemIconRoot, "meta.json"), { frames: [{ index: 412 }] });
    const complete = assertQuestItemIconClosure({
      questManifestPath,
      itemManifestPath,
      itemIconRoot,
    });
    assert.equal(complete.questItemCount, 1);
    assert.deepEqual(complete.requiredPaths, ["/original-ui/Items/412.png"]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

function writeJson(filePath, value) {
  writeFileSync(filePath, `${JSON.stringify(value)}\n`, "utf8");
}
