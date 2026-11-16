import assert from "node:assert/strict";
import fs from "node:fs";
import { createHash } from "node:crypto";
import test from "node:test";
import { buildQuestPresentationText, generateQuestPresentationText, rustString,
  QUEST_PRESENTATION_CODES, QUEST_PRESENTATION_KEYS } from "./build-quest-presentation-text.mjs";

const canonical = JSON.parse(fs.readFileSync(new URL("../../game-data/data/generated/localization_bundle.json", import.meta.url)));
const web = JSON.parse(fs.readFileSync(new URL("../../../apps/web/lib/generated/localization_bundle.json", import.meta.url)));
test("exact approved subset is deterministic and matches both committed mirrors", () => {
  assert.equal(QUEST_PRESENTATION_KEYS.length, 24);
  assert.equal(new Set(QUEST_PRESENTATION_KEYS).size, 24);
  assert.deepEqual(QUEST_PRESENTATION_CODES, ["en", "es", "pt-BR", "zh-CN"]);
  const first = generateQuestPresentationText(canonical, web);
  assert.equal(first, generateQuestPresentationText(canonical, web));
  assert.equal(first, buildQuestPresentationText(true));
  assert.equal(first.split("\n").filter(line => line.startsWith("    (")).length, 24);
  assert.doesNotMatch(first, /generatedAt|E:\\|localization_bundle|include_str!/);
});
test("mirror drift, missing keys and wrong value types fail closed", () => {
  const drift = structuredClone(web); drift.languages.en.texts["ui.quest"] += "!";
  assert.throws(() => generateQuestPresentationText(canonical, drift), /mirrors differ/);
  for (const value of [undefined, 3, null]) {
    const missing = structuredClone(canonical); missing.languages.es.texts["ui.quest"] = value;
    assert.throws(() => generateQuestPresentationText(missing, structuredClone(missing)), /Missing canonical/);
  }
});
test("Rust string escaping retains UTF8 and safely encodes controls", () => {
  assert.equal(rustString('"\\\n\r\t\0\x7f Misión 任务'), '"\\"\\\\\\n\\r\\t\\u{0}\\u{7f} Misión 任务"');
});

test("two semantic headings and 18 reviewed Portuguese corrections preserve the frozen remainder", () => {
  const oldKeys = ["ui.quest", "ui.close", "ui.previous", "ui.next", "ui.questTrack", "ui.questEmpty",
    "ui.questAccept", "ui.questComplete", "ui.questAbandon", "ui.questShare", "ui.questNoReward",
    "ui.questObjective", "ui.questReward", "ui.questRewardExp", "ui.questRewardGold",
    "ui.questRewardCredit", "ui.questRewardSelect", "ui.questTimeLimit", "ui.questStage.available",
    "ui.questStage.inProgress", "ui.questStage.readyToTurnIn", "ui.questStage.completed"];
  const addedKeys = ["ui.questReturnHeading", "ui.questProgressHeading"];
  assert.deepEqual(QUEST_PRESENTATION_KEYS, [...oldKeys, ...addedKeys].sort());
  const goldens = {en: ["Return", "Progress"], "zh-CN": ["交付地点", "任务进度"],
    es: ["Lugar de entrega", "Progreso"], "pt-BR": ["Local de entrega", "Progresso"]};
  for (const [code, [ret, progress]] of Object.entries(goldens)) {
    assert.equal(canonical.languages[code].texts[addedKeys[0]], ret);
    assert.equal(canonical.languages[code].texts[addedKeys[1]], progress);
    assert.notEqual(canonical.languages[code].texts["ui.questReturnTo"], ret);
    assert.notEqual(canonical.languages[code].texts["ui.questObjective"], progress);
  }
  // Independent reviewed copy from 2af40352307184b033516e10d200e860ed3bbf82.
  // Windows retained English fallbacks/three singular labels in these cells;
  // the integrated source preserves the existing Portuguese corrections.
  const portugueseGoldens = [
    ["ui.next", "Next", "Próxima"],
    ["ui.previous", "Previous", "Anterior"],
    ["ui.questAbandon", "Abandon", "Abandonar"],
    ["ui.questAccept", "Accept", "Aceitar"],
    ["ui.questComplete", "Complete", "Concluir"],
    ["ui.questEmpty", "No quests in this category.", "Nenhuma missão nesta categoria."],
    ["ui.questNoReward", "No reward listed.", "Nenhuma recompensa informada."],
    ["ui.questObjective", "Objective", "Objetivo"],
    ["ui.questReward", "Reward", "Recompensa"],
    ["ui.questRewardCredit", "Credit {0}", "Crédito {0}"],
    ["ui.questRewardGold", "Gold {0}", "Ouro {0}"],
    ["ui.questRewardSelect", "Choose one:", "Escolha uma:"],
    ["ui.questShare", "Share", "Compartilhar"],
    ["ui.questStage.completed", "Concluída", "Concluídas"],
    ["ui.questStage.inProgress", "Ativa", "Ativas"],
    ["ui.questStage.readyToTurnIn", "Pronta", "Prontas"],
    ["ui.questTimeLimit", "Time: {0}", "Tempo: {0}"],
    ["ui.questTrack", "Track", "Rastrear"],
  ];
  assert.equal(portugueseGoldens.length, 18);
  assert.equal(new Set(portugueseGoldens.map(([key]) => key)).size, 18);
  const legacyCanonical = structuredClone(canonical), legacyWeb = structuredClone(web);
  for (const [key, before, after] of portugueseGoldens) {
    assert.ok(oldKeys.includes(key), key);
    assert.notEqual(before, after, key);
    assert.equal(canonical.languages["pt-BR"].texts[key], after, key);
    assert.equal(web.languages["pt-BR"].texts[key], after, key);
    legacyCanonical.languages["pt-BR"].texts[key] = before;
    legacyWeb.languages["pt-BR"].texts[key] = before;
  }
  const generated = generateQuestPresentationText(canonical, web);
  const withoutNewRows = text => text.split("\n").filter(line =>
    !addedKeys.some(key => line.startsWith('    ("' + key + '"'))).join("\n");
  assert.equal(createHash("sha256").update(withoutNewRows(generated)).digest("hex"),
    "3bbd6237c550038ec8dbcf37e4f0df06154ee9f0a1f0c17f8e9801ff372044c1");
  // Restore only the 18 reviewed Portuguese cells. The original frozen hash
  // still proves all other 70 cells, code order, escaping and surrounding bytes.
  const legacy = generateQuestPresentationText(legacyCanonical, legacyWeb);
  assert.equal(createHash("sha256").update(withoutNewRows(legacy)).digest("hex"),
    "d0176e710929a9b559260f2f0fac00cfe50ea1f76b0cacde00ba41366c555871");
  assert.ok(fs.readFileSync(new URL("../../game-data/data/generated/localization_bundle.json", import.meta.url))
    .equals(fs.readFileSync(new URL("../../../apps/web/lib/generated/localization_bundle.json", import.meta.url))),
    "both actual committed mirrors have identical whole bytes");
});
test("missing null and wrong-type new heading keys fail for each of the four exact codes", () => {
  for (const code of ["en", "zh-CN", "es", "pt-BR"]) {
    for (const key of ["ui.questReturnHeading", "ui.questProgressHeading"]) {
      for (const value of [undefined, null, 3, true, {}, []]) {
        const malformed = structuredClone(canonical); malformed.languages[code].texts[key] = value;
        assert.throws(() => generateQuestPresentationText(malformed, structuredClone(malformed)),
          /Missing canonical Quest text/, code + ":" + key);
      }
    }
  }
});
