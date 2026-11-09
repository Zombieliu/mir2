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

test("single-pass placeholders and unknown-key fallbacks preserve opaque player content", () => {
  const translated = localizer.text("zh-CN", "client.AC", ["Deer{1}", "42"]);
  assert(translated.includes("Deer{1}"));
  assert(translated.includes("42"));
  assert.equal(localizer.text("zh-CN", "missing.userData", [], "Deer"), "Deer");
  assert.equal(localizer.text("ru", "missing.userData", [], "阿洛{1}"), "阿洛{1}");
  const t = localizer.buildTranslator("zh-CN");
  assert.notEqual(content.localizeCrystalItemName("Candle", t), "Candle");
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
