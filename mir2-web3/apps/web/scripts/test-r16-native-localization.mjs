import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import ts from "typescript";
import { buildR16NativeLocalization, encodeR16NativeLocalization, partitionR16GameplayLocalization, partitionR16NativeLocalization, R16_NATIVE_SLOT_EXCEPTIONS } from "./generate-r16-native-localization.mjs";

const ROOT = resolve(import.meta.dirname, "../../.."), DATA = "packages/game-data/data/native-i18n";
const read = relative => readFileSync(resolve(ROOT, relative), "utf8");
const generatedBytes = read("apps/web/lib/generated/r16-native-localization.json"), generated = JSON.parse(generatedBytes);
const startupPath = "./generated/r16-native-localization-startup.json";
const gameplayPath = "./generated/r16-native-localization-gameplay.json";
const npcPath = "./generated/r16-native-localization-gameplay-npc.json";
const { startup, gameplay } = partitionR16NativeLocalization(generated);
const { core, npc } = partitionR16GameplayLocalization(gameplay);
const order = ["zh-CN", "zh-TW", "en", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"];
function helper(data = generated, readGameplay = () => core, readNpc = () => npc) {
  const exports = {}, source = read("apps/web/lib/r16-native-localization.ts");
  const code = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, esModuleInterop: true } }).outputText;
  new Function("exports", "require", code)(exports, name => {
    if (name === startupPath) return data;
    if (name === gameplayPath) return readGameplay();
    assert.equal(name, npcPath);
    return readNpc();
  });
  return exports;
}
function changed(relative, mutate) {
  return { readSource(file) {
    const text = read(file); if (file !== relative) return text;
    const value = JSON.parse(text); mutate(value); return JSON.stringify(value);
  } };
}
function entry(key, en, zh, aliases = [], domain = "system", extra = {}) {
  return { key, values: order.map(code => code === "zh-CN" ? zh : en), aliases, domain, ...extra };
}
function synthetic(entries) { return helper({ languages: generated.languages, entries }); }
async function syntheticWithNpc(entries) {
  const packs = partitionR16GameplayLocalization({ entries });
  const api = helper({ languages: generated.languages, entries: [] }, () => packs.core, () => packs.npc);
  await api.loadR16NpcLocalization();
  return api;
}

test("the startup, core and NPC catalogs preserve every Native row and original matcher order", async () => {
  for (const [file, pack] of [[startupPath, startup], [gameplayPath, core], [npcPath, npc]]) {
    assert.equal(read("apps/web/lib/" + file.slice(2)), encodeR16NativeLocalization(pack));
  }
  assert(Buffer.byteLength(encodeR16NativeLocalization(startup)) < Buffer.byteLength(generatedBytes) * 0.3);
  assert(Buffer.byteLength(encodeR16NativeLocalization(core)) < Buffer.byteLength(encodeR16NativeLocalization(gameplay)) * 0.75);
  const restored = [...core.entries];
  let npcAt = 0;
  for (const { start, count } of npc.spans) {
    restored.splice(start, 0, ...npc.entries.slice(npcAt, npcAt + count));
    npcAt += count;
  }
  assert.equal(encodeR16NativeLocalization({ entries: restored }), encodeR16NativeLocalization(gameplay));
  assert.equal(npcAt, npc.entries.length);
  const joined = new Map([...startup.entries, ...restored].map(row => [row.key, row]));
  assert.equal(joined.size, generated.entries.length);
  assert(npc.entries.every(row => row.domain === "npc"));
  assert(core.entries.some(row => row.key === "npc.prose.e2eed1b4d37624b7"));
  assert(core.entries.some(row => row.key === "npc.shop.synchronizing"));

  let coreReads = 0, npcReads = 0;
  const native = helper(startup, () => {
    coreReads++;
    if (coreReads === 1) throw new Error("core chunk unavailable");
    return core;
  }, () => {
    npcReads++;
    if (npcReads === 1) throw new Error("NPC chunk unavailable");
    return npc;
  });
  for (const row of startup.entries.filter(row => /^(?:shell|wallet)\./.test(row.key))) {
    for (const [index, language] of order.entries()) assert.equal(native.nativeText(language, row.key), row.values[index]);
  }
  assert.equal(coreReads, 0);
  assert.equal(npcReads, 0);
  assert.equal(native.hasR16GameplayLocalization(), false);
  assert.equal(native.hasR16NpcLocalization(), false);
  assert.equal(native.nativeText("zh-CN", core.entries[0].key), core.entries[0].key);
  const failedCore = native.loadR16GameplayLocalization();
  assert.equal(native.loadR16GameplayLocalization(), failedCore);
  await assert.rejects(failedCore, /core chunk unavailable/);
  assert.equal(native.hasR16GameplayLocalization(), false);
  await native.loadR16GameplayLocalization();
  assert.equal(native.hasR16GameplayLocalization(), true);
  await native.loadR16GameplayLocalization();
  assert.equal(coreReads, 2);
  assert.equal(npcReads, 0);
  assert.equal(native.hasR16NpcLocalization(), false);
  assert.equal(native.nativeText("zh-CN", "npc.prose.e2eed1b4d37624b7"), joined.get("npc.prose.e2eed1b4d37624b7").values[0]);
  assert.equal(native.nativeText("zh-CN", npc.entries[0].key), npc.entries[0].key);
  assert.equal(native.nativeNpcDisplayText("zh-CN", "View{/CORAL}"), "View{/CORAL}");
  assert.equal(native.nativeSystemText("zh-CN", "View", "npc"), "View");

  const failedNpc = native.loadR16NpcLocalization();
  assert.equal(native.loadR16NpcLocalization(), failedNpc);
  await assert.rejects(failedNpc, /NPC chunk unavailable/);
  assert.equal(native.hasR16GameplayLocalization(), true);
  assert.equal(native.hasR16NpcLocalization(), false);
  await native.loadR16NpcLocalization();
  await native.loadR16NpcLocalization();
  assert.equal(native.hasR16NpcLocalization(), true);
  assert.equal(coreReads, 2);
  assert.equal(npcReads, 2);
  for (const row of generated.entries) {
    assert.deepEqual(joined.get(row.key), row);
    for (const [index, language] of order.entries()) assert.equal(native.nativeText(language, row.key), row.values[index]);
  }
});

test("the NPC loader waits for core and rejects a mismatched or malformed pack without installing rows", async () => {
  const attempts = [
    { ...npc, gameplaySha256: "0".repeat(64) },
    { ...npc, spans: [{ start: 0, count: npc.entries.length + 1 }] },
    npc,
  ];
  let coreReads = 0, npcReads = 0;
  const native = helper(startup, () => { coreReads++; return core; }, () => attempts[npcReads++]);
  const optional = native.loadR16NpcLocalization();
  assert.equal(native.loadR16NpcLocalization(), optional);
  await native.loadR16GameplayLocalization();
  assert.equal(coreReads, 1);
  await assert.rejects(optional, /does not match/);
  await assert.rejects(native.loadR16NpcLocalization(), /invalid row positions/);
  assert.equal(native.hasR16GameplayLocalization(), true);
  assert.equal(native.hasR16NpcLocalization(), false);
  assert.equal(native.nativeText("zh-CN", npc.entries[0].key), npc.entries[0].key);
  await native.loadR16NpcLocalization();
  assert.equal(npcReads, 3);
  assert.equal(native.nativeText("zh-CN", npc.entries[0].key), npc.entries[0].values[0]);
});

test("installing NPC rows preserves equal-template precedence and the system matcher", async () => {
  const rows = [
    entry("npc.first", " Greeting {0} ", " 问候{0} ", [" Welcome {0} "], "npc"),
    entry("periodic.second", " Greeting {0} ", " 问候{0} ", ["Welcome {0}"], "both"),
  ];
  const packs = partitionR16GameplayLocalization({ entries: rows });
  const native = helper({ languages: generated.languages, entries: [] }, () => packs.core, () => packs.npc);
  await native.loadR16GameplayLocalization();
  assert.equal(native.nativeSystemText("zh-CN", "Welcome 阿洛"), " 问候阿洛 ");
  assert.equal(native.nativeNpcDisplayText("zh-CN", "Welcome 阿洛"), "Welcome 阿洛");
  await native.loadR16NpcLocalization();
  assert.equal(native.nativeNpcDisplayText("zh-CN", "Welcome 阿洛"), "问候阿洛");
  assert.equal(native.nativeSystemText("zh-CN", "Welcome 阿洛"), " 问候阿洛 ");
});

test("installing the gameplay catalog discards the partial system alias index", async () => {
  const alias = "An authored phrase";
  const native = helper({ languages: generated.languages, entries: [entry("client.example", "Shell copy", "外壳", [alias])] },
    () => partitionR16GameplayLocalization({ entries: [entry("content.example", "Game copy", "游戏", [alias])] }).core);
  assert.equal(native.nativeSystemText("zh-CN", alias), "外壳");
  await native.loadR16GameplayLocalization();
  assert.equal(native.nativeSystemText("zh-CN", alias), alias);
});

test("the generator deterministically projects every Native key and all ten locales, with current source pins", () => {
  const actual = buildR16NativeLocalization();
  assert.equal(encodeR16NativeLocalization(actual), generatedBytes);
  assert.deepEqual(actual.languages.map(language => language.code), order);
  assert.equal(actual.entries.length, 9907); assert.equal(new Set(actual.entries.map(entry => entry.key)).size, 9907);
  assert.equal(Object.keys(actual.sourcePins).length, 20);
  assert.equal(Object.entries(actual.counts).filter(([file]) => !["periodic.json", "conquest.json", "wallet.json", "procurement.json", "native_i18n.rs::BILLING_TRANSLATIONS"].includes(file))
    .reduce((sum, [, count]) => sum + count, 0), 9426);
  assert.equal(actual.counts["native_i18n.rs::BILLING_TRANSLATIONS"], 89);
  for (const [file, sha] of Object.entries(actual.sourcePins))
    assert.equal(sha, createHash("sha256").update(read(file).replaceAll("\r\n", "\n")).digest("hex"), file);
  for (const row of actual.entries) {
    assert.equal(row.values.length, 10, row.key); assert(row.values.every(value => typeof value === "string" && value.trim()), row.key);
  }
  assert.equal(encodeR16NativeLocalization(buildR16NativeLocalization({ readSource: file => read(file).replaceAll("\r\n", "\n").replaceAll("\n", "\r\n") })), generatedBytes);
});

test("guild rank controls share five keyed system phrases in all ten locales without rewriting English or player drafts", () => {
  const api = helper();
  for (const [key, english] of [
    ["game.guild.editRank", "Edit rank"],
    ["game.guild.selectRank", "Select rank"],
    ["game.guild.newRank", "New Rank"],
    ["game.guild.createRankConfirm", "Create new rank?"],
    ["game.guild.stats", "Stats"],
  ]) {
    const row = generated.entries.find(entry => entry.key === key);
    assert(row, key); assert.equal(row.domain, "system");
    assert.equal(api.nativeText("en", key), english, "the singular and original capitalization stay exact");
    for (const code of order) {
      const translated = row.values[order.indexOf(code)];
      assert.equal(api.nativeText(code, key), translated, `${key}/${code}`);
      assert.equal(api.nativeSystemText(code, english), translated, `system source ${key}/${code}`);
      if (code !== "en") assert.notEqual(translated, english, `no English fallback ${key}/${code}`);
      const opaqueDraft = `Officer's draft ${english} 21`;
      assert.equal(api.nativeSystemText(code, opaqueDraft), opaqueDraft, `opaque rank draft ${code}`);
    }
  }
});

test("missing, empty, duplicate, foreign overlay and placeholder/command changes fail instead of filling from English", () => {
  for (const options of [
    changed(`${DATA}/extra/ru.json`, data => { delete data["client.Password"]; }),
    changed(`${DATA}/extra/hi.json`, data => { data["shell.login"] = ""; }),
    changed(`${DATA}/extra/zh-CN.json`, data => { delete data["wallet.login.title"]; }),
    changed(`${DATA}/extra/ar.json`, data => { data["unreviewed.key"] = "unexpected"; }),
    changed(`${DATA}/wallet.json`, data => { delete data.entries[0].ru; }),
    changed(`${DATA}/common.json`, data => { data.entries.push({ ...data.entries[0] }); }),
    changed(`${DATA}/extra/zh-CN.json`, data => { data["client.Password"] += " {new_arg}"; }),
    changed(`${DATA}/extra/vi.json`, data => { data["client.Password"] += " @unreviewedCommand"; }),
  ]) assert.throws(() => buildR16NativeLocalization(options), /missing\/empty|exactly cover|duplicate|placeholder mismatch|displayed command changed/);
});

test("only the eight exact existing Native Simplified slot exceptions are preserved and excluded from system matching", () => {
  const expected = [
    ["client.Exit", "Exit", "退出 ({0})"], ["client.Fishing", "Fishing", "钓鱼 ({0})"],
    ["client.Groups", "Groups", "队伍 ({0})"], ["client.Guild", "Guild", "行会（{0}）"],
    ["client.LogOut", "Log out", "退出登录 ({0})"], ["client.Quests", "Quests", "任务 ({0})"],
    ["client.Skills", "Skills", "技能 ({0})"], ["client.Trade", "Trade", "交易 ({0})"],
  ].map(([key, source, target]) => ({ key, language: "zh-CN", source, target, sourceSlots: [], targetSlots: ["0"] }));
  assert.deepEqual(R16_NATIVE_SLOT_EXCEPTIONS, expected); assert.deepEqual(generated.slotExceptions, expected);
  assert.deepEqual(generated.entries.filter(entry => entry.systemMatch === false).map(entry => entry.key).sort(), expected.map(entry => entry.key).sort());
  for (const exception of expected) {
    const api = synthetic([entry(exception.key, exception.source, exception.target, [], "system", { systemMatch: false })]);
    assert.equal(api.nativeText("zh-CN", exception.key), exception.target, "keyed lookup keeps Native's exact literal");
    assert.equal(api.nativeSystemText("zh-CN", exception.source), exception.source, "the cross-slot row cannot translate a flattened system string");
    assert.equal(api.nativeSystemText("en", exception.target.replace("{0}", "opaque-player-value")), exception.target.replace("{0}", "opaque-player-value"));
    assert.throws(() => buildR16NativeLocalization(changed(`${DATA}/extra/zh-CN.json`, data => { data[exception.key] = exception.target.replace("{0}", "{1}"); })), /placeholder mismatch/);
  }
  assert.throws(() => buildR16NativeLocalization(changed(`${DATA}/extra/zh-CN.json`, data => { data["client.Exit"] = "退出"; })), /obsolete native slot exception/);
});

test("metadata and keyed display match the generated Native source, and unknown languages/keys have explicit fallbacks", () => {
  const api = helper(); assert.deepEqual(api.R16_NATIVE_LANGUAGES, order);
  for (const language of generated.languages) {
    assert.deepEqual(api.nativeLanguageInfo(language.code), language);
    assert.equal(Object.isFrozen(api.nativeLanguageInfo(language.code)), true);
    for (const key of ["shell.login", "client.Password", "content.item.1.name", "wallet.login.title", "billing.monthlyCardItemName"])
      assert.equal(api.nativeText(language.code, key), generated.entries.find(entry => entry.key === key).values[order.indexOf(language.code)]);
  }
  assert.equal(api.nativeLanguageInfo("ar").dir, "rtl"); assert.equal(api.nativeLanguageInfo("zh-TW").nativeName, "繁體中文");
  assert.equal(api.nativeLanguageInfo("zh-CN").fontFamily, "Noto Sans CJK SC"); assert.equal(api.nativeLanguageInfo("hi").fontFamily, "Noto Sans Devanagari");
  assert.equal(api.nativeLanguageInfo("es"), null); assert.equal(api.nativeLanguageInfo("zh-Hant"), null, "host owns normalization aliases");
  assert.equal(api.nativeText("es", "shell.login", "explicit-legacy-spanish"), "explicit-legacy-spanish");
  assert.equal(api.nativeText("ru", "missing.key", "opaque fallback {value}"), "opaque fallback {value}");
  assert.equal(api.nativeText("ru", "missing.key"), "missing.key");
});

test("system aliases match complete owned strings only, preserve unknown text, and keep NPC/context ambiguity separate", async () => {
  const api = await syntheticWithNpc([
    entry("ui.gold", "Gold", "金币"), entry("ui.return", "Return", "交还"), entry("npc.return", "Return", "返回", [], "npc"),
    entry("ui.shared1", "Shared", "甲"), entry("ui.shared2", "Shared", "乙"),
  ]);
  assert.equal(api.nativeSystemText("zh-CN", "Gold"), "金币");
  assert.equal(api.nativeSystemText("zh-CN", "Return"), "交还"); assert.equal(api.nativeSystemText("zh-CN", "Return", "npc"), "返回");
  assert.equal(api.nativeSystemText("zh-CN", "Gold", "npc"), "金币", "NPC domain can fall back to ordinary system vocabulary");
  assert.equal(api.nativeSystemText("zh-CN", "Shared"), "Shared"); assert.equal(api.nativeText("zh-CN", "ui.shared1"), "甲");
  for (const text of ["hello Gold", "account-龍城{0}", "user chat: Gold", "item-not-in-catalog", "{owner} {pet}"])
    assert.equal(api.nativeSystemText("zh-CN", text), text);
  assert.equal(api.nativeSystemText("es", "Gold"), "Gold");
  assert.equal(api.nativeSystemText("zh-CN", " \tGold\n"), " \t金币\n");
  assert.equal(api.nativeSystemText("zh-CN", "Gold\nunknown chat text\nReturn"), "金币\nunknown chat text\n交还");
  assert.equal(api.nativeSystemText("zh-CN", "Gold".repeat(5000)), "Gold".repeat(5000));
});

test("bounded whole-template matching inserts opaque names once, retains color markup and numeric format captures", () => {
  const api = synthetic([
    entry("ui.hello", "Hello, {name}!", "你好，{name}！"),
    entry("ui.pair", "Pair {left} / {right}", "{left} 与 {right}"),
    entry("ui.repeat", "{name} sees {name}", "{name} 看见 {name}"),
    entry("ui.markup", "{Gold/Gold}: {npc_arg0}", "{Gold/Gold}：{npc_arg0}"),
    entry("ui.numeric", "Reward {0:#,##0}", "奖励 {0:0}"),
    entry("ui.adjacent", "{left}{right} earned", "{left}{right} 获得"),
    entry("ui.bare", "{owner} {pet}", "{owner} {pet}"),
  ]);
  assert.equal(api.nativeSystemText("zh-CN", "Hello, 龍城{Gold}!"), "你好，龍城{Gold}！");
  assert.equal(api.nativeSystemText("zh-CN", "Hello, Gold!"), "你好，Gold！");
  assert.equal(api.nativeSystemText("zh-CN", "Pair {right} / 7"), "{right} 与 7", "the inserted first argument is never reparsed as the second placeholder");
  assert.equal(api.nativeSystemText("zh-CN", "Deer sees Deer"), "Deer 看见 Deer");
  assert.equal(api.nativeSystemText("zh-CN", "Deer sees Gold"), "Deer sees Gold", "repeated placeholders must carry identical opaque values");
  assert.equal(api.nativeSystemText("zh-CN", "{Gold/Gold}: 龍城{0}"), "{Gold/Gold}：龍城{0}");
  assert.equal(api.nativeSystemText("zh-CN", "Reward 12,345"), "奖励 12,345");
  assert.equal(api.nativeSystemText("zh-CN", "leftright earned"), "leftright earned", "adjacent arguments are not guessed");
  assert.equal(api.nativeSystemText("zh-CN", "Deer Gold"), "Deer Gold", "a bare capture template cannot establish system vocabulary");
});

test("real authored NPC prose, fixed canonical aliases and embedded monthly-card names use the same generated source", async () => {
  const api = helper(startup), source = generated.entries.find(entry => entry.key === "npc.prose.00d86dfbcdfdbb82");
  await api.loadR16NpcLocalization();
  assert.equal(api.nativeSystemText("zh-CN", source.values[order.indexOf("en")], "npc"), source.values[0]);
  assert.equal(api.nativeSystemText("zh-CN", "MonthlyCard30Days"), api.nativeText("zh-CN", "billing.monthlyCardItemName"));
  const canonical = generated.entries.find(entry => entry.aliases.includes("RakingCat") && entry.key.startsWith("content.monster."));
  assert(canonical); assert.equal(api.nativeText("zh-CN", canonical.key), canonical.values[0]);
});

test("NPC display matches static colour-decorated script copy and keeps captured names opaque", async () => {
  const api = await syntheticWithNpc([
    entry("npc.view", "View", "查看", [], "npc"),
    entry("npc.hello", "Hello {npc_arg0}, welcome!", "你好{npc_arg0}，欢迎！", [], "npc"),
  ]);
  assert.equal(api.nativeNpcDisplayText("zh-CN", "View{/CORAL}"), "查看");
  assert.equal(api.nativeNpcDisplayText("zh-CN", "{View/CORAL}"), "查看");
  assert.equal(api.nativeNpcDisplayText("zh-CN", "View{/UNKNOWN}"), "View{/UNKNOWN}");
  assert.equal(api.nativeNpcDisplayText("zh-CN", "{/CORAL}"), "");
  assert.equal(api.nativeNpcDisplayText("zh-CN", "{/UNKNOWN}"), "{/UNKNOWN}");
  assert.equal(api.nativeNpcDisplayText("zh-CN", "Hello 龍城{View/CORAL}, welcome!"), "你好龍城{View/CORAL}，欢迎！");
  assert.equal(api.nativeNpcDisplayText("zh-CN", "Hello 龍城{/CORAL}, welcome!"), "你好龍城{/CORAL}，欢迎！");
  assert.equal(api.nativeNpcDisplayText("zh-CN", "Unknown dialogue {x/CORAL}"), "Unknown dialogue {x/CORAL}");
});
