import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) while loading ${url}`);
  };
  new Function("exports", "module", "require", compiled.outputText)(module.exports, module, require);
  return module.exports;
}

const contentLocalization = loadTypeScriptModule(
  new URL("../lib/crystal-content-localization.ts", import.meta.url),
);
const { localizeQuestEntry, localizeQuestLog } = loadTypeScriptModule(
  new URL("../lib/quest-localization.ts", import.meta.url),
  { "./crystal-content-localization": contentLocalization },
);
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

const webBundleSource = readFileSync(
  new URL("../lib/generated/localization_bundle.json", import.meta.url),
  "utf8",
);
const gameDataBundleSource = readFileSync(
  new URL("../../../packages/game-data/data/generated/localization_bundle.json", import.meta.url),
  "utf8",
);
assert.equal(webBundleSource, gameDataBundleSource, "web and game-data localization bundles must stay in sync");
const localizationBundle = JSON.parse(webBundleSource);
const zhTexts = localizationBundle.languages["zh-CN"].texts;
const enTexts = localizationBundle.languages.en.texts;
const zhT = (key, params = [], fallback = key) =>
  params.reduce(
    (value, entry, index) => value.split(`{${index}}`).join(String(entry)),
    zhTexts[key] ?? enTexts[key] ?? fallback,
  );

const routeTitles = new Map([
  [1, "简助理的请求"],
  [2, "朱迪女工匠的请求"],
  [3, "与屠夫交谈"],
  [4, "屠夫的狩猎委托"],
  [5, "铁匠的第一次考验"],
  [6, "铁匠的第二次考验"],
  [7, "拜访战士导师"],
  [8, "基本剑术考验"],
  [9, "前往比奇城"],
  [22, "森林雪人的威胁"],
]);

for (const [questId, expectedTitle] of routeTitles) {
  for (const stage of ["available", "inProgress", "readyToTurnIn", "completed"]) {
    const routeQuest = localizeQuestEntry({
      ...smith,
      questId,
      title: "English title",
      summary: "English summary",
      objective: "English objective",
      progressLabel: "English progress",
      tracker: "English tracker",
      rewardPreview: "English reward",
      descriptionLines: ["English description"],
      stage,
    }, zhT);
    assert.equal(routeQuest.title, expectedTitle, `quest ${questId} ${stage} title`);
    for (const value of [
      routeQuest.summary,
      routeQuest.objective,
      routeQuest.progressLabel,
      routeQuest.tracker,
      routeQuest.rewardPreview,
      routeQuest.descriptionLines?.join("\n"),
    ]) {
      assert.doesNotMatch(
        value ?? "",
        /English/,
        `quest ${questId} ${stage} must not fall back to English`,
      );
    }
  }
}

const butcherHunt = localizeQuestEntry({
  ...smith,
  questId: 4,
  title: "Hunt for the Butcher",
  stage: "available",
  descriptionLines: ["English description"],
  objectives: [{ label: "Collect Deer Meat", current: 0, required: 5 }],
  npc: "Merchant_John",
  rewards: {
    items: [{ name: "OldCopperRing", itemIndex: 1175, count: 1 }],
  },
}, zhT);
assert.equal(butcherHunt.descriptionLines[0], "替屠夫约翰猎鹿并带回五块鹿肉。需要查看原版屠宰说明时可按 H 键。");
assert.equal(butcherHunt.objectives[0].label, "收集 5 块鹿肉。");
assert.equal(butcherHunt.npc, "屠夫_约翰");
assert.equal(butcherHunt.rewards.items[0].name, "旧铜戒指");

assert.equal(zhT("ui.questAccept"), "接受");
assert.equal(zhT("ui.questComplete"), "完成");
assert.equal(zhT("log.realmInfo", ["platinum_176", "platinum_176", 25]), "服务器 platinum_176 · 配置 platinum_176 v25");
assert.equal(contentLocalization.localizeCrystalMapTitle("BichonProvince", zhT), "比奇省");
assert.equal(contentLocalization.localizeCrystalEntityName("Deer", zhT), "鹿");
assert.equal(contentLocalization.localizeCrystalEntityName("Royal_Guard", zhT), "皇家_卫兵");
assert.equal(contentLocalization.localizeCrystalEntityName("ForestYeti", zhT), "森林雪人");
assert.equal(contentLocalization.localizeCrystalEntityName("ForestYeti0", zhT), "森林雪人");
assert.equal(contentLocalization.localizeCrystalEntityName("MIRDM", zhT), "MIRDM");
assert.equal(contentLocalization.localizeCrystalItemName("PrecisionPendant", zhT), "精准吊坠");

const minePanelSource = readFileSync(
  new URL("../app/components/onchain-mine-panel.tsx", import.meta.url),
  "utf8",
);
assert.match(minePanelSource, /t: OnchainMineTranslateFn/);
assert.match(minePanelSource, /t\("ui\.onchainMine\.title"/);
assert.doesNotMatch(minePanelSource, /<strong>On-chain Mine \(testnet\)<\/strong>/);
assert.equal(zhT("ui.onchainMine.title"), "链上矿脉（测试网）");
assert.equal(zhT("ui.onchainMine.redeem"), "兑换金币");

const sceneLayerSource = readFileSync(
  new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url),
  "utf8",
);
assert.match(sceneLayerSource, /t\("client\.OwnerHero"/);
assert.doesNotMatch(sceneLayerSource, /text: `\$\{entity\.ownerName\}'s Hero`/);

console.log("quest localization tests passed");
