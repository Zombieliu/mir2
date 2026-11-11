import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const modules = new Map();
function load(relative) {
  const filename = path.resolve(root, relative);
  if (modules.has(filename)) return modules.get(filename);
  if (filename.endsWith(".json")) return JSON.parse(readFileSync(filename, "utf8"));
  const module = { exports: {} }; modules.set(filename, module.exports);
  const compiled = ts.transpileModule(readFileSync(filename, "utf8"), { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, esModuleInterop: true,
  } });
  new Function("require", "module", "exports", compiled.outputText)(specifier => {
    assert(specifier.startsWith("./"), "localization has no external runtime dependency");
    return load(path.relative(root, path.resolve(path.dirname(filename), specifier.endsWith(".json") ? specifier : `${specifier}.ts`)));
  }, module, module.exports);
  modules.set(filename, module.exports); return module.exports;
}
const localizer = load("lib/localization.ts"), native = load("lib/r16-native-localization.ts");
const auth = load("lib/r16-auth-layout.ts"), runtime = load("lib/r16-runtime-language.ts");
const content = load("lib/crystal-content-localization.ts");

test("real combined translator exposes exactly Windows choices and native shell/wallet copy", () => {
  assert.deepEqual(localizer.SUPPORTED_LANGUAGES, ["zh-CN", "zh-TW", "en", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"]);
  for (const language of localizer.SUPPORTED_LANGUAGES) {
    assert.equal(localizer.languageNativeName(language), native.nativeLanguageInfo(language).nativeName);
    assert.equal(localizer.languageLocale(language), native.nativeLanguageInfo(language).locale);
    for (const key of Object.keys(auth.R16_AUTH_CAPTIONS)) assert.equal(auth.r16AuthCaption(language, key), native.nativeText(language, `shell.${key}`));
    assert.equal(localizer.text(language, "wallet.login.title"), native.nativeText(language, "wallet.login.title"));
    assert.equal(localizer.text(language, "shell.warrior"), native.nativeText(language, "shell.warrior"));
    assert(localizer.text(language, "ui.walletLogin") !== "Wallet");
  }
});

test("saved choices win; initial choice follows supported browser locale, with canonical aliases", () => {
  for (const [input, expected] of [["ZH_tw", "zh-TW"], ["zh-Hant-HK", "zh-TW"], ["zh-HK", "zh-TW"],
    ["zh-Hans", "zh-CN"], ["zh-SG", "zh-CN"], ["pt-BR", "pt-BR"], ["ru-RU", "ru"], ["hi-IN", "hi"],
    ["id-ID", "id"], ["vi-VN", "vi"], ["th-TH", "th"], ["ar-SA", "ar"], ["en-GB", "en"], ["es", "en"]]) {
    assert.equal(localizer.normalizeLanguage(input), expected);
  }
  assert.equal(localizer.preferredLanguage("zh-TW", ["ru-RU"]), "zh-TW");
  assert.equal(localizer.preferredLanguage(null, ["fr-FR", "hi-IN", "en-US"]), "hi");
  assert.equal(localizer.preferredLanguage("es", ["zh-CN"]), "zh-CN");
  assert.equal(localizer.preferredLanguage(null, ["fr-FR"]), "en");
});

test("new presentation locales cannot leak unsupported codes into strict wire/WASM hosts", () => {
  for (const language of localizer.SUPPORTED_LANGUAGES) {
    assert(["en", "zh-CN", "es", "pt-BR"].includes(runtime.questRuntimeLanguage(language)));
    assert(["en", "zh-CN"].includes(runtime.commerceRuntimeLanguage(language)));
  }
  assert.equal(runtime.questRuntimeLanguage("pt-BR"), "pt-BR");
  assert.equal(runtime.questRuntimeLanguage("zh-TW"), "en");
  assert.equal(runtime.commerceRuntimeLanguage("pt-BR"), "en");
  const page = readFileSync(path.join(root, "app/page.tsx"), "utf8");
  assert.equal((page.match(/type: "setLanguage", language: questRuntimeLanguage\(language\)/g) ?? []).length, 2);
  assert.equal((page.match(/language: commerceRuntimeLanguage\(language\)/g) ?? []).length, 2);
  assert.match(page, /language: questRuntimeLanguage\(language\),\s*displayLanguage: questDisplayLanguage\(language\),\s*hudBarPlayerObjectId/);
  assert.match(page, /if \(typeof window === "undefined" \|\| !isClientReady\) return;/);
});

test("single-pass placeholders and unknown-key fallbacks preserve opaque player content", async () => {
  const translated = localizer.text("zh-CN", "client.AC", ["Deer{1}", "42"]);
  assert(translated.includes("Deer{1}"));
  assert(translated.includes("42"));
  assert.equal(localizer.text("zh-CN", "missing.userData", [], "Deer"), "Deer");
  assert.equal(localizer.text("ru", "missing.userData", [], "阿洛{1}"), "阿洛{1}");
  await native.loadR16GameplayLocalization();
  const t = localizer.buildTranslator("zh-CN");
  assert.notEqual(content.localizeCrystalItemName("Candle", t), "Candle");
});

test("the Web newcomer journal uses the Windows title and instruction in every display language", async () => {
  await native.loadR16GameplayLocalization();
  const { localizeQuestEntry } = load("lib/quest-localization.ts");
  const catalog = load("lib/generated/r16-native-localization.json");
  const rows = new Map(catalog.entries.map(row => [row.key, row.values]));
  const ids = catalog.entries.filter(row => /^quest\.(?:21100\d{2}|21200\d{2})\.title$/.test(row.key))
    .map(row => Number(row.key.split(".")[1]));
  assert.equal(ids.length, 26);
  for (const questId of ids) {
    const quest = { questId, title: "Wire title", summary: "Wire summary", objective: "Wire objective",
      progressLabel: "1/2", stage: "available", current: 1, required: 2, rewardPreview: "50 gold",
      descriptionLines: ["Wire line one", "Wire line two"] };
    for (const [index, language] of localizer.SUPPORTED_LANGUAGES.entries()) {
      const localized = localizeQuestEntry(quest, localizer.buildTranslator(language));
      assert.equal(localized.title, rows.get("quest." + questId + ".title")[index]);
      assert.deepEqual(localized.descriptionLines, [rows.get("quest." + questId + ".description")[index]]);
      assert.equal(localized.objective, quest.objective);
      assert.equal(localized.current, quest.current);
      assert.equal(localized.required, quest.required);
      assert.equal(localized.rewardPreview, quest.rewardPreview);
    }
    assert.deepEqual(quest.descriptionLines, ["Wire line one", "Wire line two"]);
  }
});

test("the R16 newcomer journal renders the canonical objectives with authoritative counters in all ten languages", async () => {
  await native.loadR16GameplayLocalization();
  const { localizeQuestEntry, formatQuestText } = load("lib/quest-localization.ts");
  const catalog = load("lib/generated/r16-native-localization.json");
  const journey = JSON.parse(readFileSync(path.resolve(root, "../../config/quest-guidance/newcomer-journey-v2.json"), "utf8"));
  const rows = new Map(catalog.entries.map(row => [row.key, row.values]));
  const monsters = {
    Scarecrow: 39, RakingCat: 44, Oma: 48, ForestYeti: 55, Skeleton: 85,
    Zombie2: 110, Zombie3: 112, Dung: 242, WoomaSoldier: 244, WoomaFighter: 246,
  };
  const substitute = (template, args) => template.replace(/\{([a-z]+)\}/g, (_, key) => String(args[key]));
  assert.equal(journey.quests.length, 22);
  for (const [index, language] of localizer.SUPPORTED_LANGUAGES.entries()) {
    const t = localizer.buildTranslator(language);
    const at = key => rows.get(key)[index];
    for (const quest of journey.quests) {
      const killRows = (quest.kills ?? []).flatMap(kill => {
        const [, name] = /^Defeat \d+ (.+)\.$/.exec(kill.message);
        const displayName = at("content.monster." + monsters[name] + ".name");
        return [
          [{ label: kill.message, current: 0, required: kill.count, done: false },
            substitute(at("quest.objective.defeat"), { count: kill.count, name: displayName })],
          [{ label: "Kill " + name + " (1 / " + kill.count + ")", current: 1, required: kill.count, done: kill.count === 1 },
            substitute(at("quest.objective.kill"), { name: displayName })],
        ];
      });
      const flagRows = (quest.flags ?? []).map(flag => {
        const catalogRow = catalog.entries.find(row => row.key.startsWith("quest.journey.copy.") && row.values[2] === flag.message);
        assert(catalogRow, flag.message);
        return [{ label: flag.message, current: 0, required: 1, done: false }, catalogRow.values[index]];
      });
      const checks = [...killRows, ...flagRows].reverse();
      const objectives = checks.map(([objective]) => objective);
      const source = {
        questId: quest.id, title: quest.title, summary: "", objective: "", progressLabel: "Progress 1/2",
        tracker: "Newcomer V2 main node 1.", stage: "inProgress", current: 1, required: 2, rewardPreview: "", objectives,
      };
      const projected = localizeQuestEntry(source, t);
      assert.deepEqual(projected.objectives, checks.map(([objective, label]) => ({ ...objective, label })), language + " " + quest.id);
      assert.equal(projected.progressLabel, at("ui.questProgressHeading") + " 1/2");
      assert.equal(projected.tracker, substitute(at("quest.objective.node"), { node: 1 }));
      assert.deepEqual([projected.stage, projected.current, projected.required], ["inProgress", 1, 2]);
      assert.deepEqual(source.objectives, objectives);
      for (const [row, expected] of flagRows) {
        const withSuffix = localizeQuestEntry({ ...source, objectives: [{ ...row, label: row.label + " (0 / 1)" }] }, t);
        assert.equal(withSuffix.objectives[0].label, expected);
      }
    }
    assert.equal(formatQuestText(t, "quest.chapter.progress", "Chapter {progress}", { progress: "1/6" }),
      substitute(at("quest.chapter.progress"), { progress: "1/6" }));
    assert.equal(formatQuestText(t, "quest.objective.kill", "Kill {name}", { name: "Opaque {name}" }),
      substitute(at("quest.objective.kill"), { name: "Opaque {name}" }));
  }
  const source = {
    questId: 2110002, title: "Equip your weapon", summary: "", objective: "Return to the Quest Diary",
    progressLabel: "Ready to turn in.", tracker: "Return to the Quest Diary", stage: "readyToTurnIn",
    current: 1, required: 1, rewardPreview: "",
    objectives: [{ label: "Kill Unknown {name}", current: 0, required: 3 }],
    rewards: { selectItems: [{ name: "(HP)DrugSmall", selectionIndex: 3, itemIndex: 55, count: 10 }] },
  };
  const t = localizer.buildTranslator("zh-TW");
  const ready = localizeQuestEntry(source, t);
  assert.equal(ready.tracker, native.nativeText("zh-TW", "quest.journey.copy.18"));
  assert.equal(ready.objective, ready.tracker);
  assert.equal(ready.progressLabel, "可交付");
  assert.deepEqual(ready.objectives, source.objectives);
  assert.deepEqual(ready.rewards.selectItems, [{ ...source.rewards.selectItems[0],
    name: content.localizeCrystalItemName("(HP)DrugSmall", t) }]);
  const available = localizeQuestEntry({ ...source, stage: "available", tracker: source.title + " is available." }, t);
  assert.equal(available.tracker, "");
  const jane = localizeQuestEntry({
    ...source, questId: 2110001, objective: "Return to Assistant Jane.", tracker: "Return to Assistant Jane.",
  }, t);
  assert.equal(jane.tracker, "返回助理 Jane");
  assert.equal(jane.objective, jane.tracker);
  const diary = localizeQuestEntry({ ...source, tracker: "Return to the Quest Diary." }, t);
  assert.equal(diary.tracker, native.nativeText("zh-TW", "quest.journey.copy.18"));
  for (const level of [15, 20, 25, 30]) {
    const questId = 2120000 + level;
    const reward = { ...source, questId, stage: "inProgress", objective: "No automatic completion condition.",
      tracker: "No automatic completion condition.", progressLabel: "Level " + level + "+, up to 30", objectives: undefined };
    for (const language of localizer.SUPPORTED_LANGUAGES) {
      const translated = localizer.buildTranslator(language);
      const projected = localizeQuestEntry(reward, translated);
      assert.equal(projected.objective, native.nativeText(language, "quest." + questId + ".description"));
      assert.equal(projected.tracker, projected.objective);
      assert.equal(projected.progressLabel, formatQuestText(translated, "quest.ui.levelTitle", "Lv {level} {title}", {
        level: level + "–30", title: "",
      }).trim());
      const claimed = localizeQuestEntry({ ...reward, stage: "completed", objective: "Growth reward claimed." }, translated);
      assert.equal(claimed.objective, native.nativeText(language, "content.quest.generic.stage.completed.objective"));
    }
  }
});

test("the fallback chapter and progress labels use the Windows keys in every display language", async () => {
  await native.loadR16GameplayLocalization();
  const { formatQuestText } = load("lib/quest-localization.ts");
  const journey = load("lib/newcomer-v2-journey.ts");
  const filename = path.join(root, "app/components/original-client-quest-log-window.tsx");
  const source = ts.createSourceFile(filename, readFileSync(filename, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const wanted = ["newcomerV2ChapterLabel", "newcomerV2JourneyLabel", "newcomerV2JourneyTooltip"];
  const functions = source.statements.filter(node => ts.isFunctionDeclaration(node) && wanted.includes(node.name?.text));
  assert.equal(functions.length, 3);
  const code = ts.transpileModule(functions.map(node => node.getText(source)).join("\n"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const labels = new Function("formatQuestText", "NEWCOMER_V2_CHAPTERS", "localizeNewcomerV2Text",
    code + ";return { newcomerV2ChapterLabel, newcomerV2JourneyLabel, newcomerV2JourneyTooltip };")(
    formatQuestText, journey.NEWCOMER_V2_CHAPTERS, journey.localizeNewcomerV2Text);
  for (const language of localizer.SUPPORTED_LANGUAGES) {
    const t = localizer.buildTranslator(language);
    for (const [index, chapter] of journey.NEWCOMER_V2_CHAPTERS.entries()) {
      const state = { ...chapter, number: index + 1, completedRecordedQuestCount: 1 };
      const label = labels.newcomerV2ChapterLabel(state, t);
      assert.equal(label, [
        formatQuestText(t, "quest.chapter.progress", "Chapter {progress}", { progress: (index + 1) + "/6" }),
        native.nativeText(language, "quest.journey.copy." + (index + 1)),
        formatQuestText(t, "quest.ui.chapterProgress", "This chapter {done}/{total}", { done: 1, total: chapter.questIds.length }),
      ].join(" · "));
    }
  }
});

test("graduation choices match Native class targets and instructions in every display language", async () => {
  await native.loadR16GameplayLocalization();
  const { localizeNewcomerV2Graduation } = load("lib/quest-localization.ts");
  const journey = load("lib/newcomer-v2-journey.ts");
  const config = JSON.parse(readFileSync(path.resolve(root, "../../config/quest-guidance/newcomer-v2-graduation.json"), "utf8"));
  const maps = JSON.parse(readFileSync(path.resolve(root, "../../packages/game-data/data/generated/crystal_respawn_manifest.json"), "utf8"));
  const map = maps.maps.find(row => row.map_file_name === config.challenge.mapFileName);
  assert(map, "the challenge uses a real Crystal map");
  const completed = config.requiredCompletionIds.map(questId => ({ questId, stage: "completed" }));
  for (const [className, configured] of Object.entries(config.classes)) {
    const source = journey.newcomerV2GraduationFor(completed, config.minimumLevel, className);
    const fingerprint = JSON.stringify(source);
    for (const language of localizer.SUPPORTED_LANGUAGES) {
      const t = localizer.buildTranslator(language);
      const view = localizeNewcomerV2Graduation(source, t);
      assert.equal(view.title, native.nativeText(language, "quest.journey.copy.6"));
      assert.deepEqual(view.goals.map(row => row.key), ["equipment", "skill", "challenge"]);
      const [equipment, skill, challenge] = view.goals;
      assert.equal(equipment.title, native.nativeText(language, "content.item." + configured.equipment.itemIndex + ".name"));
      assert.equal(equipment.label, native.nativeText(language, "game.006"));
      assert.equal(equipment.requirements, t.systemText(configured.equipment.requirements) + " " + native.nativeText(language, "quest.feedback.81"));
      assert.equal(equipment.normalAcquisition, t.systemText(configured.equipment.normalAcquisition));
      assert.equal(skill.label, native.nativeText(language, "common.6df1bb18a59a"));
      assert.equal(skill.title, t.systemText(configured.skill.title));
      assert.equal(skill.requirements, t.systemText(configured.skill.requirements) + " " + native.nativeText(language, "quest.feedback.82"));
      assert.equal(skill.normalAcquisition, t.systemText(configured.skill.normalAcquisition));
      assert.equal(challenge.label, native.nativeText(language, "common.27cf1792f7bb"));
      assert.equal(challenge.title, t.systemText(config.challenge.title));
      assert.equal(challenge.requirements, native.nativeText(language, "quest.feedback.83")
        .replace("{difficulty}", t.systemText(config.challenge.difficulty))
        .replace("{map}", native.nativeText(language, "content.map." + map.map_index + ".name")));
      assert.equal(challenge.normalAcquisition, t.systemText(config.challenge.normalTravel) + " " + t.systemText(config.challenge.detail));
      assert.equal(JSON.stringify(source), fingerprint, "display changes must not mutate the handoff source");
    }
    assert.equal(localizeNewcomerV2Graduation(journey.newcomerV2GraduationFor(completed, 29, className), localizer.buildTranslator("zh-TW")), null);
    assert.equal(localizeNewcomerV2Graduation(journey.newcomerV2GraduationFor(completed.slice(1), 30, className), localizer.buildTranslator("zh-TW")), null);
  }
});

test("real nameplate display boundary localizes only NPCs and monsters", () => {
  const filename = path.join(root, "app/components/original-client-scene-visual-layers.tsx");
  const source = ts.createSourceFile(filename, readFileSync(filename, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const declaration = source.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === "entityDisplayName");
  assert(declaration);
  const compiled = ts.transpileModule(declaration.getText(source), { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  const display = new Function("localizeCrystalEntityName", compiled + ";return entityDisplayName;")(() => "translated");
  for (const kind of ["player", "hero", "self"]) assert.equal(display({ kind, name: "Deer" }, null), "Deer");
  for (const kind of ["npc", "monster"]) assert.equal(display({ kind, name: "Deer" }, null), "translated");
});
