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

const journey = loadTypeScriptModule(
  new URL("../lib/newcomer-v2-journey.ts", import.meta.url),
);
const contentLocalization = loadTypeScriptModule(
  new URL("../lib/crystal-content-localization.ts", import.meta.url),
);
const { localizeQuestEntry } = loadTypeScriptModule(
  new URL("../lib/quest-localization.ts", import.meta.url),
  {
    "./crystal-content-localization": contentLocalization,
    "./newcomer-v2-journey": journey,
  },
);

const uiContract = JSON.parse(readFileSync(
  new URL("../../../config/quest-guidance/newcomer-journey-v2-ui.json", import.meta.url),
  "utf8",
));
const journeyConfig = JSON.parse(readFileSync(
  new URL("../../../config/quest-guidance/newcomer-journey-v2.json", import.meta.url),
  "utf8",
));
const graduationConfig = JSON.parse(readFileSync(
  new URL("../../../config/quest-guidance/newcomer-v2-graduation.json", import.meta.url),
  "utf8",
));

assert.deepEqual(
  journey.NEWCOMER_V2_CHAPTERS.map(({ id, questIds }) => ({ id, questIds: [...questIds] })),
  uiContract.chapters.map(({ id, questIds }) => ({ id, questIds })),
  "web chapter recognition must match the actual newcomer-v2 UI contract",
);

const configuredQuestIds = [
  ...journeyConfig.quests.map(({ id }) => id),
  ...journeyConfig.growthRewards.map(({ id }) => id),
].sort((left, right) => left - right);
const projectedQuestIds = journey.NEWCOMER_V2_CHAPTERS
  .flatMap(({ questIds }) => questIds)
  .sort((left, right) => left - right);
assert.deepEqual(projectedQuestIds, configuredQuestIds, "all configured V2 lines must belong to one chapter");
assert.equal(journey.newcomerV2ChapterForQuest(999999), null);

assert.deepEqual(
  [...journey.NEWCOMER_V2_GRADUATION_REQUIRED_COMPLETION_IDS].sort((left, right) => left - right),
  [...graduationConfig.requiredCompletionIds].sort((left, right) => left - right),
  "graduation must require every Native handoff completion exactly once",
);
const completedGraduationRoute = graduationConfig.requiredCompletionIds.map((questId) => ({
  questId,
  stage: "completed",
}));
assert.equal(
  journey.newcomerV2GraduationFor(completedGraduationRoute.slice(1), 30, "warrior"),
  null,
  "a missing server completion row cannot graduate the player",
);
assert.equal(
  journey.newcomerV2GraduationFor(completedGraduationRoute, 29, "warrior"),
  null,
  "level 29 cannot graduate even with all completion rows",
);
assert.equal(
  journey.newcomerV2GraduationFor(completedGraduationRoute, 30, "archer"),
  null,
  "an unknown player class cannot receive a class handoff",
);
assert.equal(
  journey.newcomerV2GraduationFor([], 30, "taoist"),
  null,
  "default or absent quest rows cannot infer a completed route",
);
for (const [classKey, configured] of Object.entries(graduationConfig.classes)) {
  const graduation = journey.newcomerV2GraduationFor(
    completedGraduationRoute,
    30,
    classKey.toLowerCase(),
  );
  assert.equal(graduation.title, graduationConfig.title);
  assert.deepEqual(
    graduation.goals.map(({ key, title, requirements, normalAcquisition }) => ({
      key,
      title,
      requirements,
      normalAcquisition,
    })),
    [
      { key: "equipment", ...configured.equipment },
      { key: "skill", ...configured.skill },
      {
        key: "challenge",
        title: graduationConfig.challenge.title,
        requirements: `Medium challenge: ${graduationConfig.challenge.targets.map((target) => target.name).join(" and ")}.`,
        normalAcquisition: graduationConfig.challenge.normalTravel,
      },
    ].map(({ key, title, requirements, normalAcquisition }) => ({ key, title, requirements, normalAcquisition })),
    `${classKey} graduation goals must mirror the Native handoff`,
  );
}
const graduationWindowSource = readFileSync(
  new URL("../app/components/original-client-quest-log-window.tsx", import.meta.url),
  "utf8",
);
const graduationPanelSource = graduationWindowSource.slice(
  graduationWindowSource.indexOf('data-testid="newcomer-graduation"'),
  graduationWindowSource.indexOf('<div style={style.actions}>'),
);
assert.match(
  graduationPanelSource,
  /onClick=\{\(\) => setSelectedGraduationGoal\(goal\.key\)\}/,
  "graduation choices must be local selection controls",
);
assert.doesNotMatch(
  graduationPanelSource,
  /on(?:AcceptQuest|FinishQuest|TrackQuest|ShareQuest|AbandonQuest)/,
  "graduation choices must not invoke quest callbacks",
);

const liveJourney = journey.newcomerV2JourneyProgress([
  { questId: 2110001, stage: "completed" },
  { questId: 2110005, stage: "inProgress" },
  { questId: 2110006, stage: "available" },
]);
assert.equal(liveJourney.currentChapter.id, "forest");
assert.equal(liveJourney.chapters[0].recordedQuestCount, 1);
assert.equal(liveJourney.chapters[0].completedRecordedQuestCount, 1);
assert.equal(liveJourney.chapters[1].recordedQuestCount, 2);
assert.equal(liveJourney.chapters[1].completedRecordedQuestCount, 0);

const completionOnlyHistory = journey.newcomerV2JourneyProgress([
  { questId: 2110022, stage: "completed" },
]);
assert.equal(completionOnlyHistory.currentChapter, null, "historical completion alone cannot claim a current chapter");
assert.equal(completionOnlyHistory.chapters[5].recordedQuestCount, 1);
assert.equal(completionOnlyHistory.chapters[5].completedRecordedQuestCount, 1);
assert.equal(completionOnlyHistory.recordedQuestCount, 1, "missing V2 rows must remain unknown, not completed");

const englishT = (_key, _params = [], fallback = "") => fallback;
const chineseT = (key, _params = [], fallback = "") => key === "ui.questTrack" ? "追踪" : fallback;
for (const configuredQuest of journeyConfig.quests) {
  const localizedTitle = localizeQuestEntry({
    questId: configuredQuest.id,
    title: "server title",
    summary: "",
    objective: "",
    progressLabel: "",
    stage: "inProgress",
    current: 0,
    required: 0,
    rewardPreview: "",
  }, englishT).title;
  assert.equal(localizedTitle, configuredQuest.title, `V2 title ${configuredQuest.id} must match the runtime quest config`);
}

function objectiveRowsFromRuntimeConfig(quest) {
  return [
    ...quest.kills.map((kill, index) => ({
      label: kill.message,
      current: index,
      required: kill.count,
      done: false,
    })),
    ...quest.flags.map((flag, index) => ({
      label: flag.message,
      current: index % 2,
      required: 1,
      done: index % 2 === 1,
    })),
  ];
}

for (const configuredQuest of journeyConfig.quests) {
  const runtimeRows = objectiveRowsFromRuntimeConfig(configuredQuest);
  for (const rows of [runtimeRows, [...runtimeRows].reverse()]) {
    const localized = localizeQuestEntry({
      questId: configuredQuest.id,
      title: configuredQuest.title,
      summary: "",
      objective: "server objective",
      progressLabel: "",
      stage: "inProgress",
      current: 0,
      required: 0,
      rewardPreview: "",
      objectives: [...rows, { label: "Unknown external objective", current: 4, required: 9, done: false }],
    }, chineseT);
    assert.deepEqual(
      localized.objectives.map(({ current, required, done }) => ({ current, required, done })),
      [
        ...rows.map(({ current, required, done }) => ({ current, required, done })),
        { current: 4, required: 9, done: false },
      ],
      `quest ${configuredQuest.id} must retain every runtime objective's numeric progress`,
    );
    assert.ok(
      localized.objectives.slice(0, rows.length).every((objective, index) => objective.label !== rows[index].label),
      `quest ${configuredQuest.id} must localize every configured kill/flag message regardless of order`,
    );
    assert.deepEqual(
      localized.objectives.at(-1),
      { label: "Unknown external objective", current: 4, required: 9, done: false },
      `quest ${configuredQuest.id} must preserve unknown objective rows`,
    );
  }
}

const node21Rows = objectiveRowsFromRuntimeConfig(
  journeyConfig.quests.find((quest) => quest.id === 2110021),
);
for (const rows of [node21Rows, [...node21Rows].reverse()]) {
  const localized = localizeQuestEntry({
    questId: 2110021,
    title: "Prove your class tactics",
    summary: "",
    objective: "",
    progressLabel: "",
    stage: "inProgress",
    current: 0,
    required: 0,
    rewardPreview: "",
    objectives: rows,
  }, chineseT);
  assert.deepEqual(
    localized.objectives.map(({ label, current, required }) => ({ label, current, required })),
    rows.map((row) => row.label.startsWith("Defeat")
      ? { label: "消灭 3 名 WoomaFighter", current: row.current, required: row.required }
      : { label: "完成职业练习", current: row.current, required: row.required }),
    "node 21 kill and class-practice labels must stay attached to their own counters",
  );
}

const dynamicKillRow = localizeQuestEntry({
  questId: 2110021,
  title: "Prove your class tactics",
  summary: "",
  objective: "",
  progressLabel: "",
  stage: "inProgress",
  current: 0,
  required: 3,
  rewardPreview: "",
  objectives: [{ label: "Kill WoomaFighter 0/3", current: 0, required: 3, done: false }],
}, chineseT);
assert.equal(dynamicKillRow.objectives[0].label, "消灭 3 名 WoomaFighter");
const baseQuest = {
  questId: 2110003,
  title: "Protect the village",
  summary: "server summary",
  objective: "Defeat targets",
  progressLabel: "0/4",
  tracker: "server tracker",
  stage: "inProgress",
  current: 0,
  required: 4,
  rewardPreview: "server reward",
  objectives: [
    { label: "Defeat 2 Scarecrow.", current: 0, required: 2 },
    { label: "Defeat 2 RakingCat.", current: 0, required: 2 },
  ],
};
const localizedChinese = localizeQuestEntry(baseQuest, chineseT);
assert.equal(localizedChinese.title, "保卫村庄");
assert.equal(localizedChinese.objective, "消灭 2 个稻草人和 2 只钉耙猫");
assert.deepEqual(localizedChinese.objectives.map(({ label }) => label), ["消灭 2 个稻草人", "消灭 2 只钉耙猫"]);
assert.equal(localizedChinese.objectives[0].required, 2, "localization preserves server progress");
assert.equal(localizedChinese.stage, "inProgress", "localization cannot complete a quest");
const availableChinese = localizeQuestEntry({ ...baseQuest, stage: "available" }, chineseT);
assert.equal(availableChinese.objective, "Defeat targets", "an unaccepted line keeps its server objective state");

const localizedEnglish = localizeQuestEntry(baseQuest, englishT);
assert.equal(localizedEnglish.title, "Protect the village");
assert.equal(localizedEnglish.objective, "Defeat 2 Scarecrows and 2 Raking Cats");
assert.equal(localizeQuestEntry({ ...baseQuest, questId: 999999 }, chineseT).title, "Protect the village");

console.log("newcomer v2 journey tests passed");
