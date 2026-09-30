import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { decodeQuestText, translateName, root } from "./generate-native-i18n-content.mjs";
import { displayTemplate, splitVisibleNpcLine, isStaticallyUnsupportedNpcToken } from "./extract-native-i18n-npc.mjs";
import { validateCatalogues, placeholders, sameValueReason, classifyAliasConflict } from "./verify-native-i18n.mjs";
import * as vocabulary from "./native-i18n-translations.mjs";
import { menuRows } from "./native-i18n-menu-translations.mjs";
import { questRows } from "./native-i18n-quest-translations.mjs";
import { assertReviewedSources } from "./native-i18n-source-guard.mjs";
import { validateValue, verifyExpansionFiles } from "./native-i18n-expansion.mjs";

const read = (file) => JSON.parse(readFileSync(resolve(root, file), "utf8"));
const valid = (key = "sample") => ({ key, en: "Give {amount} to {player}", "zh-TW": "將 {amount} 交給 {player}", "pt-BR": "Entregue {amount} a {player}", aliases: ["Give {amount} to {player}"] });

test("six additional locales preserve gameplay numbers, opaque arguments and displayed commands", () => {
  const entry = { key: "numeric", en: "Pay 50 gold for {0:N2} charges; use @Buy" };
  assert.deepEqual(validateValue(entry, "ar", "ادفع ٥٠ ذهب مقابل {0:N2} شحنة؛ استخدم @Buy"), []);
  assert.ok(validateValue({ key: "pet", en: "{0}_{1} Pet" }, "ar", "{0} {1}").includes("authored words disappeared around parameters"));
  for (const text of ["ادفع 60 ذهب مقابل {0}؛ @Buy", "ادفع 50 ذهب؛ @Buy", "ادفع 50 ذهب مقابل {0} {0}؛ @Buy", "ادفع 50 ذهب مقابل {0}؛ @Sell", "ادفع 50 ذهب مقابل [X1]؛ @Buy"]) {
    assert.ok(validateValue(entry, "ar", text).length, text);
  }
});

test("every additional native locale has the exact source key closure and safe substitutions", () => {
  const report = verifyExpansionFiles();
  assert.deepEqual(report.locales, ["ru", "hi", "id", "vi", "th", "ar"]);
  assert.equal(report.entriesPerLocale, 9385);
  assert.deepEqual(report.errors, []);
});

test("three-language validation preserves named/numbered parameters and rejects dropped/repeated parameters", () => {
  assert.deepEqual(placeholders("{player} {0:N2} {player} {Red/Color}"), ["0", "player", "player"]);
  assert.deepEqual(placeholders("{L:  R: /CORAL} {{npc_arg0}/KHAKI} {2:#,##0} {0:P0}"), ["0", "2", "npc_arg0"]);
  assert.deepEqual(placeholders("{0_value} {4:C2} {5:0.0%;-0.0%} {6:invalid} {Gold/Gold}"), ["0_value", "4", "5"]);
  assert.deepEqual(validateCatalogues({ sample: { entries: [valid()] } }).errors, []);
  for (const incorrect of ["Entregue {amount}", "Entregue {amount} a {player} {player}", "Entregue {value} a {player}"]) {
    const entry = { ...valid(), "pt-BR": incorrect };
    assert.ok(validateCatalogues({ sample: { entries: [entry] } }).errors.some((error) => error.reason === "placeholder mismatch"));
  }
});

test("empty translations and duplicate stable keys fail; equivalent aliases across domains do not", () => {
  const identical = validateCatalogues({ one: { entries: [valid("a")] }, two: { entries: [valid("b")] } });
  assert.equal(identical.errors.length, 0); assert.equal(identical.aliasConflicts.length, 0); assert.equal(identical.equivalentAliasCount, 3);
  const bad = validateCatalogues({ one: { entries: [valid(), { ...valid(), "zh-TW": "" }] } });
  assert.ok(bad.errors.some((error) => error.reason === "duplicate key"));
  assert.ok(bad.errors.some((error) => error.reason === "missing or empty translation"));
});

test("alias ambiguity is reported with both keys and context rather than silently accepted", () => {
  const a = { key: "common.return", en: "Return", "zh-TW": "交付", "pt-BR": "Entrega", aliases: [] };
  const b = { key: "npc.return", en: "Return", "zh-TW": "返回", "pt-BR": "Voltar", aliases: [] };
  const report = validateCatalogues({ "common.json": { entries: [a] }, "npc-menus.json": { entries: [b] } });
  assert.deepEqual(report.aliasConflicts[0].owners.map(({ key }) => key), ["common.return", "npc.return"]);
  assert.match(report.aliasConflicts[0].classification, /context-bound/);
  assert.match(classifyAliasConflict("BichonProvince", [{ file: "common.json", key: "content.map.bichon.name" }, { file: "content.json", key: "content.map.1.name" }]), /canonical content priority/);
});

test("displayed command tokens remain byte-identical; email examples are not commands", () => {
  const command = { key: "command", en: "Use @GuardsHelp", "zh-TW": "使用 @GuardsHelp", "pt-BR": "Use @GuardsHelp", aliases: [] };
  assert.equal(validateCatalogues({ sample: { entries: [command] } }).errors.length, 0);
  assert.ok(validateCatalogues({ sample: { entries: [{ ...command, "pt-BR": "Use @Ajuda" }] } }).errors.some((error) => error.reason === "displayed command token changed"));
  const email = { key: "email", en: "Name@Example.com", "zh-TW": "Name@Example.com", "pt-BR": "Nome@Exemplo.com", aliases: [] };
  assert.equal(validateCatalogues({ sample: { entries: [email] } }).errors.length, 0);
});

test("NPC extraction matches runtime link removal and preserves executable targets outside text", () => {
  assert.deepEqual(splitVisibleNpcLine("I will use <Service/@tele(0,3)> now."), { body: "I will use  now.", rawBody: "I will use  now.", menus: [{ text: "Service", rawText: "Service", target: "@tele(0,3)" }], parameters: [] });
  assert.deepEqual(splitVisibleNpcLine("<<Buy/@buy/Red>>"), { body: "", rawBody: "", menus: [{ text: "Buy", rawText: "Buy", target: "@buy" }], parameters: [] });
  assert.deepEqual(splitVisibleNpcLine("<<BichonWall/@BichonWall>>"), { body: "", rawBody: "", menus: [{ text: "BichonWall", rawText: "BichonWall", target: "@BichonWall" }], parameters: [] });
});

test("NPC runtime values become named opaque captures before colour/link stripping", () => {
  const source = "Hello {<$USERNAME>/KHAKI}, pay %P0 gold to <Enter/@Enter(%ARG(0))>.";
  const result = splitVisibleNpcLine(source);
  assert.equal(result.body, "Hello {npc_arg0}, pay {npc_arg1} gold to .");
  assert.equal(result.rawBody, "Hello {{npc_arg0}/KHAKI}, pay {npc_arg1} gold to .");
  assert.equal(result.menus[0].text, "Enter");
  assert.equal(result.menus[0].target, "@Enter({npc_arg2})");
  assert.deepEqual(result.parameters.map(({ source }) => source), ["<$USERNAME>", "%P0", "%ARG(0)"]);
  assert.equal(displayTemplate("<$OUTPUT(A1)> %INPUTSTR").text, "{npc_arg0} {npc_arg1}");
});

test("actual raw colour aliases coexist with plain labels without erasing player captures", () => {
  const result = splitVisibleNpcLine("Hello {<$USERNAME>/KHAKI}, my name is {<$NPCNAME>/KHAKI}.", { retainUnsupportedTokens: true });
  assert.equal(result.body, "Hello {npc_arg0}, my name is .");
  assert.equal(result.rawBody, "Hello {{npc_arg0}/KHAKI}, my name is {/KHAKI}.");
  assert.deepEqual(placeholders(result.rawBody), ["npc_arg0"]);
  const weapon = splitVisibleNpcLine("I see you're wearing: {<$WEAPON>/CORAL}", { retainUnsupportedTokens: true });
  assert.equal(weapon.rawBody, "I see you're wearing: {/CORAL}");
  assert.equal(weapon.body, "I see you're wearing:");
  for (const token of ["<$USERNAME>", "<$DATE>", "<$CONQUESTOWNER(1)>", "<$OUTPUT(P0)>", "%ARG(0)"]) assert.equal(isStaticallyUnsupportedNpcToken(token), false, token);
  for (const token of ["<$NPCNAME>", "<$WEAPON>", "<$HP>"]) assert.equal(isStaticallyUnsupportedNpcToken(token), true, token);
});

test("original quest text is bounds checked and retains real newlines after colour removal", () => {
  for (const bad of ["", "01000000", "0100000002000000ffffffffffffffffffff"]) assert.throws(() => decodeQuestText(bad));
  const source = read("packages/game-data/data/generated/crystal_quest_packet_manifest.json").quests;
  assert.equal(source.length, 154);
  assert.equal(questRows.length, 154);
  assert.equal(new Set(questRows.map(({ id }) => id)).size, 154);
  for (const quest of source) {
    const text = decodeQuestText(quest.payload_hex);
    assert.equal(text.index, quest.index); assert.equal(text.name, quest.name);
    assert.ok(questRows.some(({ id }) => id === quest.index));
  }
  const entry = read("packages/game-data/data/native-i18n/content.json").entries.find(({ key }) => key === "content.quest.1.description");
  assert.ok(entry.en.includes("\n")); assert.equal(entry.en.includes("\\n"), false);
});

test("consumable medium is not male; armour/costume gender and quantity are preserved", () => {
  for (const name of ["SunPotion(M)", "HealthStone(M)", "StaminaAid(M)", "HardTorch(M)", "WonderBox(M)", "Knapsack(M)"]) {
    const translation = translateName(name, "item");
    assert.ok(translation, name); assert.match(translation["zh-TW"], /（中）$/); assert.match(translation["pt-BR"], /\(médio\)$/);
  }
  for (const name of ["MirArmour(M)", "Ninja(M)", "Formal2(M)"]) assert.match(translateName(name, "item")["pt-BR"], /\(masculino\)$/);
  assert.match(translateName("MirArmour(F)", "item")["pt-BR"], /\(feminino\)$/);
  assert.match(translateName("SunPotion(M)", "item").en, /\(M\)$/);
});

test("translation vocabulary has no duplicate source term within each curated dictionary", () => {
  for (const [name, entries] of [...Object.entries(vocabulary), ["menuRows", menuRows]]) {
    if (!Array.isArray(entries) || !entries.every(Array.isArray)) continue;
    assert.equal(new Set(entries.map(([source]) => source)).size, entries.length, name);
  }
});

test("authored proper names and legitimate same-form Portuguese are distinct from missing prose", () => {
  assert.match(sameValueReason({ key: "content.magic.Portal.name", en: "Portal", "pt-BR": "Portal" }, "pt-BR"), /Portuguese/);
  assert.match(sameValueReason({ key: "content.monster.48.name", en: "Oma", "pt-BR": "Oma" }, "pt-BR"), /creature/);
  assert.equal(sameValueReason({ key: "bad", en: "This untranslated English sentence.", "pt-BR": "This untranslated English sentence." }, "pt-BR"), null);
});

test("released canonical name and all nonempty original quest field IDs have translations", () => {
  assertReviewedSources();
  const entries = read("packages/game-data/data/native-i18n/content.json").entries;
  const byKey = new Map(entries.map((entry) => [entry.key, entry]));
  for (const [family, file, field, id, title, count] of [["item", "crystal_item_manifest.json", "items", "item_index", "name", 1628], ["monster", "crystal_monster_manifest.json", "monsters", "monster_index", "name", 555], ["npc", "crystal_npc_info_manifest.json", "npcs", "npc_index", "name", 375], ["magic", "crystal_magic_manifest.json", "magics", "spell", "name", 109], ["map", "crystal_respawn_manifest.json", "maps", "map_index", "map_title", 464]]) {
    const records = read(`packages/game-data/data/generated/${file}`)[field]; assert.equal(records.length, count, family);
    for (const row of records) if (row[title]?.trim()) {
      const key = `content.${family}.${row[id]}.name`; assert.ok(byKey.has(key), key);
      assert.ok(byKey.get(key).aliases.includes(row[title]), `canonical alias: ${key}`);
    }
  }
  for (const row of read("packages/game-data/data/generated/crystal_quest_packet_manifest.json").quests) {
    const parsed = decodeQuestText(row.payload_hex);
    for (const field of ["description", "task", "return", "completion"]) if (parsed[field].join("\n").trim()) {
      const key = `content.quest.${row.index}.${field}`;
      assert.ok(byKey.has(key));
      assert.ok(byKey.get(key).aliases.includes(parsed[field].join("\n")), `original exact prose alias: ${key}`);
    }
  }
});
