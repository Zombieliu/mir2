import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourcePath = new URL("../lib/quest-localization.ts", import.meta.url);
const source = readFileSync(sourcePath, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sourcePath),
});
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);

const { localizeQuestEntry, localizeQuestLog } = module.exports;
const texts = {
  "content.quest.smithFirstTest.title": "铁匠的第一次考验",
  "content.quest.smithFirstTest.summary": "清理村外道路。",
  "content.quest.smithFirstTest.description": "清理鹿和稻草人。",
  "content.quest.smithFirstTest.rewardPreview": "120 经验",
  "content.quest.smithFirstTest.objective.0": "消灭 10 只鹿。",
  "content.quest.smithFirstTest.objective.1": "消灭 10 个稻草人。",
  "content.quest.smithFirstTest.stage.inProgress.objective": "消灭 10 只鹿和 10 个稻草人。",
  "content.quest.smithFirstTest.stage.inProgress.progressLabel": "已消灭 {0}/{1} 个目标",
  "content.quest.smithFirstTest.stage.inProgress.tracker": "继续清理村外道路。",
};
const t = (key, params = [], fallback = key) =>
  params.reduce(
    (value, entry, index) => value.split(`{${index}}`).join(String(entry)),
    texts[key] ?? fallback,
  );

const smith = {
  questId: 5,
  title: "The Smith's 1st Test",
  summary: "English summary",
  objective: "English objective",
  progressLabel: "English progress",
  tracker: "English tracker",
  stage: "inProgress",
  current: 7,
  required: 20,
  rewardPreview: "English reward",
  descriptionLines: ["English description"],
  objectives: [
    { label: "Deer", current: 4, required: 10 },
    { label: "Scarecrow", current: 3, required: 10 },
  ],
};
const localized = localizeQuestEntry(smith, t);

assert.notEqual(localized, smith);
assert.equal(localized.title, "铁匠的第一次考验");
assert.equal(localized.summary, "清理村外道路。");
assert.equal(localized.objective, "消灭 10 只鹿和 10 个稻草人。");
assert.equal(localized.progressLabel, "已消灭 7/20 个目标");
assert.equal(localized.tracker, "继续清理村外道路。");
assert.equal(localized.rewardPreview, "120 经验");
assert.deepEqual(localized.descriptionLines, ["清理鹿和稻草人。"]);
assert.equal(localized.objectives[0].label, "消灭 10 只鹿。");
assert.equal(localized.objectives[1].label, "消灭 10 个稻草人。");
assert.equal(localized.objectives[0].current, 4);
assert.equal(localized.objectives[1].required, 10);

const unknown = { ...smith, questId: 9999 };
assert.equal(localizeQuestEntry(unknown, t), unknown);
const list = localizeQuestLog([smith, unknown], t);
assert.equal(list.length, 2);
assert.equal(list[0].title, "铁匠的第一次考验");
assert.equal(list[1], unknown);

console.log("quest localization tests passed");
