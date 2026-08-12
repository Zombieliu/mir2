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
  "content.quest.emperorsProblem.title": "皇帝的难题",
  "content.quest.emperorsProblem.summary": "比奇省有一件与皇帝有关的事情等待调查。",
  "content.quest.emperorsProblem.stage.available.objective": "与任务相关的 NPC 交谈并接受调查。",
  "content.quest.emperorsProblem.stage.available.progressLabel": "未接受",
  "content.quest.emperorsProblem.stage.available.tracker": "按照任务标记寻找相关 NPC。",
  "content.quest.emperorsProblem.stage.inProgress.objective": "按照原版任务指引，调查皇帝遇到的难题。",
  "content.quest.emperorsProblem.stage.inProgress.progressLabel": "调查中",
  "content.quest.emperorsProblem.stage.inProgress.tracker": "继续按照比奇省内的任务标记推进。",
  "content.quest.emperorsProblem.stage.readyToTurnIn.objective": "返回任务指定的 NPC 处，报告调查结果。",
  "content.quest.emperorsProblem.stage.readyToTurnIn.progressLabel": "可提交",
  "content.quest.emperorsProblem.stage.readyToTurnIn.tracker": "返回任务标记指向的 NPC。",
  "content.quest.emperorsProblem.stage.completed.objective": "皇帝难题的调查已经完成。",
  "content.quest.emperorsProblem.stage.completed.progressLabel": "已完成",
  "content.quest.emperorsProblem.stage.completed.tracker": "调查已经结束。",
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

const emperorsProblem = localizeQuestEntry({
  ...smith,
  questId: 154,
  title: "Emperors Problem",
  summary: "English summary",
  stage: "available",
}, t);
assert.equal(emperorsProblem.title, "皇帝的难题");
assert.equal(emperorsProblem.summary, "比奇省有一件与皇帝有关的事情等待调查。");
assert.equal(emperorsProblem.objective, "与任务相关的 NPC 交谈并接受调查。");
assert.equal(emperorsProblem.progressLabel, "未接受");
assert.equal(emperorsProblem.tracker, "按照任务标记寻找相关 NPC。");

const emperorStagePresentations = [
  {
    stage: "inProgress",
    objective: "按照原版任务指引，调查皇帝遇到的难题。",
    progressLabel: "调查中",
    tracker: "继续按照比奇省内的任务标记推进。",
  },
  {
    stage: "readyToTurnIn",
    objective: "返回任务指定的 NPC 处，报告调查结果。",
    progressLabel: "可提交",
    tracker: "返回任务标记指向的 NPC。",
  },
  {
    stage: "completed",
    objective: "皇帝难题的调查已经完成。",
    progressLabel: "已完成",
    tracker: "调查已经结束。",
  },
];

for (const expected of emperorStagePresentations) {
  const stagePresentation = localizeQuestEntry({
    ...smith,
    questId: 154,
    title: "Emperors Problem",
    summary: "English summary",
    stage: expected.stage,
  }, t);
  assert.equal(stagePresentation.title, "皇帝的难题");
  assert.equal(stagePresentation.objective, expected.objective);
  assert.equal(stagePresentation.progressLabel, expected.progressLabel);
  assert.equal(stagePresentation.tracker, expected.tracker);
}

console.log("quest localization tests passed");
