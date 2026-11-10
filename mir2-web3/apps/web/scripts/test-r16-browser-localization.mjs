import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const sourceUrl = new URL("../lib/r16-browser-localization.ts", import.meta.url);
const source = readFileSync(sourceUrl, "utf8");
const compiled = ts.transpileModule(source, { fileName: sourceUrl.pathname, reportDiagnostics: true,
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, strict: true } });
assert.equal(compiled.diagnostics?.length ?? 0, 0);
const module = { exports: {} };
new Function("require", "module", "exports", compiled.outputText)(specifier => {
  throw new Error(`Unexpected browser dictionary runtime dependency: ${specifier}`);
}, module, module.exports);
const { browserText, R16_BROWSER_DICTIONARIES, R16_BROWSER_LANGUAGES, R16_BROWSER_TEXT_KEYS } = module.exports;

const expectedLanguages = ["zh-CN", "zh-TW", "en", "pt-BR", "ru", "hi", "id", "vi", "th", "ar"];
const expectedKeys = [
  "ui.audio", "ui.masterVolume", "ui.music", "ui.musicVolume", "ui.effects", "ui.effectsVolume",
  "ui.muteAll", "ui.unmuteAll", "ui.on", "ui.off", "ui.passkeyLogin", "ui.walletLogin", "ui.quickEnter", "ui.languageSettings",
  "browser.settings", "browser.settingsDetail", "browser.accountSecurity", "browser.accountInfo", "browser.signIn", "browser.diagnostics",
  "ui.walletQrAlt", "ui.walletQrFailed", "ui.walletQrRetryHint", "ui.walletApproveOnDevice", "ui.walletMissing",
  "ui.questClientLoading", "ui.questClientUnavailable", "ui.questActionPending", "ui.questActionRewardRequired",
  "ui.questActionTalkToNpc", "ui.questActionStateChanged", "ui.retry", "ui.sceneLoadUnavailable", "ui.fishingReel", "ui.characterCreateUnconfirmed",
  "ui.gameTextLoading", "ui.gameTextLoadUnavailable", "ui.confirmReturnToCharacterSelect",
];

test("browser dictionaries use the ten native languages in the native order", () => {
  assert.deepEqual(R16_BROWSER_LANGUAGES, expectedLanguages);
  assert.deepEqual(Object.keys(R16_BROWSER_DICTIONARIES), expectedLanguages);
  assert.equal(new Set(R16_BROWSER_TEXT_KEYS).size, expectedKeys.length);
  assert.deepEqual(R16_BROWSER_TEXT_KEYS, expectedKeys);
});

for (const language of expectedLanguages) test(`${language} supplies every browser control without blank values`, () => {
  const dictionary = R16_BROWSER_DICTIONARIES[language];
  assert.deepEqual(Object.keys(dictionary).sort(), [...expectedKeys].sort());
  for (const key of expectedKeys) {
    const value = browserText(language, key);
    assert.equal(typeof value, "string", `${language}/${key}`);
    assert.equal(value, value.trim(), `${language}/${key} must not have edge whitespace`);
    assert.ok(value.length > 0, `${language}/${key} must not be blank`);
    assert.doesNotMatch(value, /^[\s\u200B-\u200F\uFEFF]*$/u);
    assert.equal(value, dictionary[key]);
  }
  if (language !== "en") assert.notEqual(dictionary["browser.signIn"], R16_BROWSER_DICTIONARIES.en["browser.signIn"]);
});

test("unknown languages and keys preserve the caller's fallback", () => {
  for (const language of ["", "es", "fr", "zh", "__proto__", "constructor", "toString"]) {
    assert.equal(browserText(language, "browser.settings"), undefined);
  }
  for (const language of expectedLanguages) for (const key of ["", "missing", "__proto__", "constructor", "toString"]) {
    assert.equal(browserText(language, key), undefined);
  }
});

test("native wallet captions are not redefined by the browser dictionary", () => {
  for (const language of expectedLanguages) for (const key of [
    "ui.walletTitle", "ui.walletStatus", "ui.walletRefresh", "ui.walletCancel", "ui.walletCountdown",
  ]) assert.equal(browserText(language, key), undefined);
});

test("browser settings explain the extra volume/account/sign-in scope", () => {
  assert.equal(browserText("zh-CN", "browser.settings"), "浏览器附加设置");
  assert.equal(browserText("zh-TW", "browser.settings"), "瀏覽器附加設定");
  assert.equal(browserText("en", "browser.settings"), "Additional browser settings");
  assert.match(browserText("en", "browser.settingsDetail"), /Volume, account and sign-in/u);
  for (const language of expectedLanguages) assert.match(browserText(language, "ui.walletMissing"), /Sui/u);
});

test("type-only native compatibility does not pull a runtime dependency", () => {
  assert.match(source, /import type \{ R16NativeLanguage \} from "\.\/r16-native-localization"/u);
  assert.doesNotMatch(compiled.outputText, /require\(/u);
  assert.equal(browserText("ar", "ui.masterVolume"), "مستوى الصوت الرئيسي");
  assert.equal(browserText("hi", "browser.signIn"), "साइन इन करें");
});

test("quest feedback covers every language without exposing implementation terms", () => {
  const keys = ["ui.questClientLoading", "ui.questClientUnavailable", "ui.questActionPending", "ui.questActionRewardRequired",
    "ui.questActionTalkToNpc", "ui.questActionStateChanged", "ui.retry"];
  for (const language of expectedLanguages) for (const key of keys) {
    const value = browserText(language, key);
    assert.ok(value?.trim(), `${language}/${key}`);
    assert.doesNotMatch(value, /wasm|websocket|\b(?:ABI|ECS|shared|rejection|dispatch|client|modelRevision|ownerRevision)\b/iu);
  }
  assert.equal(browserText("zh-CN", "ui.questActionPending"), "请求处理中，请稍候。");
  assert.equal(browserText("zh-TW", "ui.retry"), "重試");
  assert.equal(browserText("en", "ui.questActionTalkToNpc"), "Talk to the quest giver first.");
});
