import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

// Execute bounded expressions from the real Page and language adapters. This
// controlled hook/document/storage seam is not a browser or persistence test.
const web = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const require = createRequire(import.meta.url);
const tsPath = require.resolve("typescript");
const ts = require(tsPath);
const beforePath = process.argv[2];
assert.equal(process.argv.length, 3, "provide exactly one frozen-before Page path");
assert.ok(path.isAbsolute(beforePath), "negative control must be an absolute path");
const inputs = [];
function read(filename) {
  const bytes = readFileSync(filename);
  inputs.push({ path: filename, bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") });
  return bytes.toString("utf8");
}
function parse(filename) {
  return ts.createSourceFile(filename, read(filename), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
}
function nodes(root, predicate) {
  const found = [];
  function visit(node) {
    if (predicate(node)) found.push(node);
    ts.forEachChild(node, visit);
  }
  visit(root);
  return found;
}
function one(root, predicate, label) {
  const found = nodes(root, predicate);
  assert.equal(found.length, 1, `${label}: expected one real AST node`);
  return found[0];
}
function id(node, name) { return !!node && ts.isIdentifier(node) && node.text === name; }
function call(node, name) { return ts.isCallExpression(node) && id(node.expression, name); }
function declaration(root, name) {
  return one(root, node => ts.isVariableDeclaration(node) && id(node.name, name), name);
}
function component(root, name) {
  return one(root, node => ts.isFunctionDeclaration(node) && id(node.name, name), name);
}
function defaultComponent(root) {
  return one(root, node => ts.isFunctionDeclaration(node) && node.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.DefaultKeyword), "default Page component");
}
function evaluate(node, scope = {}) {
  const source = `return (${node.getText()});`;
  const output = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None } });
  return new Function(...Object.keys(scope), output.outputText)(...Object.values(scope));
}
function jsx(root, tag) {
  return one(root, node => (ts.isJsxSelfClosingElement(node) || ts.isJsxOpeningElement(node)) && id(node.tagName, tag), tag);
}
function prop(element, name) {
  const attribute = one(element.attributes, node => ts.isJsxAttribute(node) && id(node.name, name), `${element.tagName.getText()}.${name}`);
  assert.ok(attribute.initializer && ts.isJsxExpression(attribute.initializer) && attribute.initializer.expression);
  return attribute.initializer.expression;
}
function languageProps(element, scope) {
  return { language: evaluate(prop(element, "language"), scope), onLanguageChange: evaluate(prop(element, "onLanguageChange"), scope) };
}

const pageFile = parse(path.join(web, "app/page.tsx"));
const page = defaultComponent(pageFile);
const beforeFile = parse(beforePath);
const beforePage = defaultComponent(beforeFile);
const shell = component(parse(path.join(web, "app/original-client-shell.tsx")), "OriginalClientShell");
const overlays = parse(path.join(web, "app/components/original-client-overlays.tsx"));
const layout = parse(path.join(web, "app/layout.tsx"));
const bundlePath = path.join(web, "lib/generated/localization_bundle.json");
const bundle = JSON.parse(read(bundlePath));
const localizerPath = path.join(web, "lib/localization.ts");
const localizerSource = read(localizerPath);
const compiledLocalizer = ts.transpileModule(localizerSource, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, esModuleInterop: true } });
const localizerModule = { exports: {} };
new Function("exports", "module", "require", compiledLocalizer.outputText)(localizerModule.exports, localizerModule, specifier => {
  assert.equal(specifier, "./generated/localization_bundle.json", "bounded real localization dependency");
  return bundle;
});
const localizer = localizerModule.exports;
read(tsPath);
read(path.join(web, "node_modules/typescript/package.json"));

const effect = one(page, node => call(node, "useEffect") && ts.isFunctionExpression(node.arguments[0]) && id(node.arguments[0].name, "syncPageDocumentLanguage"), "named Page effect");
assert.ok(ts.isArrayLiteralExpression(effect.arguments[1]));
assert.equal(effect.arguments[1].elements.length, 1);
assert.ok(id(effect.arguments[1].elements[0], "locale"), "use existing locale dependency");
const localeExpression = declaration(page, "locale").initializer;
assert.ok(localeExpression);
const state = one(page, node => ts.isVariableDeclaration(node) && ts.isArrayBindingPattern(node.name) && id(node.name.elements[0]?.name, "language") && id(node.name.elements[1]?.name, "setLanguage"), "Page language state");
assert.ok(call(state.initializer, "useState"));
const storedRead = one(page, node => call(node, "setLanguage") && call(node.arguments[0], "normalizeLanguage"), "Page saved-language read");
const storedWrite = one(page, node => ts.isCallExpression(node) && node.expression.getText() === "window.localStorage.setItem" && ts.isStringLiteral(node.arguments[0]) && node.arguments[0].text === "mir2-language", "Page saved-language write");
const pageShell = jsx(page, "OriginalClientShell");
const selector = component(overlays, "LanguageSelector");
const map = one(selector, node => ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && id(node.expression.expression, "SUPPORTED_LANGUAGES") && node.expression.name.text === "map", "real supported-language options");
assert.ok(ts.isArrowFunction(map.arguments[0]) && id(map.arguments[0].parameters[0].name, "option"));
const click = prop(jsx(map.arguments[0], "button"), "onClick");
const html = jsx(layout, "html");
const htmlLang = one(html.attributes, node => ts.isJsxAttribute(node) && id(node.name, "lang"), "SSR HTML declaration");
assert.ok(ts.isStringLiteral(htmlLang.initializer));
const ssrLanguage = htmlLang.initializer.text;
assert.equal(ssrLanguage, "en");
assert.deepEqual(localizer.SUPPORTED_LANGUAGES, ["en", "zh-CN", "es", "pt-BR"]);
const expectedLocale = language => bundle.languages[language].locale;

function fixture(prior = ssrLanguage) {
  let language;
  let current = prior;
  const writes = [];
  const document = { documentElement: { get lang() { return current; }, set lang(value) { current = value; writes.push(value); } } };
  const storageReads = [];
  const storageWrites = [];
  let saved = null;
  const window = { localStorage: {
    getItem(key) { storageReads.push(key); return saved; },
    setItem(key, value) { storageWrites.push([key, value]); saved = value; },
  } };
  const setLanguage = value => { language = value; };
  [language] = evaluate(state.initializer, { useState: initial => [initial, setLanguage] });
  let dependencies;
  let cleanup;
  let setup;
  let setups = 0;
  const useEffect = (callback, next) => {
    if (dependencies && dependencies.length === next.length && next.every((value, index) => Object.is(value, dependencies[index]))) return;
    cleanup?.();
    dependencies = next;
    setup = callback;
    cleanup = callback();
    setups++;
  };
  function render() {
    const locale = evaluate(localeExpression, { language, languageLocale: localizer.languageLocale });
    evaluate(effect, { document, locale, useEffect });
    evaluate(storedWrite, { window, language });
    return locale;
  }
  return {
    document, writes, storageReads, storageWrites, render,
    get language() { return language; }, get setups() { return setups; },
    get cleanup() { return cleanup; },
    restore(value) { saved = value; evaluate(storedRead, { window, setLanguage, normalizeLanguage: localizer.normalizeLanguage }); return render(); },
    choose(screen, option) {
      const shellProps = languageProps(pageShell, { language, setLanguage });
      const overlayName = screen === "login" ? "LoginOverlay" : "SelectOverlay";
      const overlayProps = languageProps(jsx(shell, overlayName), shellProps);
      const selectorProps = languageProps(jsx(component(overlays, overlayName), "LanguageSelector"), overlayProps);
      const handler = evaluate(click, { ...selectorProps, option });
      handler();
      return render();
    },
    strictReplay() { cleanup(); cleanup = setup(); setups++; },
    unmount() { cleanup?.(); dependencies = undefined; },
  };
}

const cases = [];
function check(name, action) { action(); cases.push(name); }
for (const screen of ["login", "select"]) {
  for (const language of localizer.SUPPORTED_LANGUAGES) {
    check(`${screen} selector → Page state → ${language} locale`, () => {
      const f = fixture(); f.render();
      assert.equal(f.choose(screen, language), expectedLocale(language));
      assert.equal(f.language, language);
      assert.equal(f.document.documentElement.lang, expectedLocale(language));
      assert.deepEqual(f.storageWrites.at(-1), ["mir2-language", language]);
      f.unmount(); assert.equal(f.document.documentElement.lang, ssrLanguage);
    });
  }
}
for (const [saved, language] of [["ZH-HANS", "zh-CN"], ["es-ES", "es"], ["pt-PT", "pt-BR"], ["en", "en"], ["unknown", "en"], [null, "en"]]) {
  check(`real stored-language read ${JSON.stringify(saved)} → ${language}`, () => {
    const f = fixture(); f.render();
    assert.equal(f.restore(saved), expectedLocale(language));
    assert.deepEqual(f.storageReads, ["mir2-language"]);
    assert.equal(f.language, language);
    assert.equal(f.document.documentElement.lang, expectedLocale(language));
    assert.deepEqual(f.storageWrites.at(-1), ["mir2-language", language]);
    f.unmount(); assert.equal(f.document.documentElement.lang, ssrLanguage);
  });
}
check("locale update cleans up before applying and restores original on exit", () => {
  const f = fixture("de"); f.render();
  f.choose("login", "zh-CN"); f.choose("select", "pt-BR");
  assert.deepEqual(f.writes, [expectedLocale("en"), "de", expectedLocale("zh-CN"), "de", expectedLocale("pt-BR")]);
  f.unmount(); assert.equal(f.document.documentElement.lang, "de");
});
check("unchanged canonical locale skips setup and retains cleanup", () => {
  const f = fixture(); f.render(); f.restore("zh");
  const cleanup = f.cleanup; const writes = f.writes.length; const setups = f.setups;
  f.restore("zh-Hans"); f.choose("select", "zh-CN");
  assert.equal(f.setups, setups); assert.equal(f.writes.length, writes); assert.equal(f.cleanup, cleanup);
  f.unmount(); assert.equal(f.document.documentElement.lang, ssrLanguage);
});
check("StrictMode-style setup / cleanup / setup retains and releases declaration", () => {
  const f = fixture(); f.restore("pt"); f.strictReplay();
  assert.equal(f.document.documentElement.lang, expectedLocale("pt-BR"));
  assert.equal(f.setups, 2); f.unmount(); assert.equal(f.document.documentElement.lang, ssrLanguage);
});
check("old one-shot cleanup cannot roll back a later mount", () => {
  const f = fixture(); f.restore("zh"); const oldCleanup = f.cleanup;
  f.unmount(); f.document.documentElement.lang = "fr"; f.render();
  oldCleanup(); assert.equal(f.document.documentElement.lang, expectedLocale("zh-CN"));
  f.unmount(); assert.equal(f.document.documentElement.lang, "fr");
});
check("intervening different declaration survives unmount and repeated cleanup", () => {
  const f = fixture(); f.restore("es"); const cleanup = f.cleanup;
  f.document.documentElement.lang = "fr"; f.unmount(); cleanup();
  assert.equal(f.document.documentElement.lang, "fr");
});
check("update preserves intervening different declaration as next prior value", () => {
  const f = fixture(); f.restore("zh"); f.document.documentElement.lang = "fr";
  f.choose("select", "es"); assert.equal(f.document.documentElement.lang, expectedLocale("es"));
  f.unmount(); assert.equal(f.document.documentElement.lang, "fr");
});

const oldEffects = nodes(beforePage, node => call(node, "useEffect") && ts.isFunctionExpression(node.arguments[0]) && id(node.arguments[0].name, "syncPageDocumentLanguage"));
assert.equal(oldEffects.length, 0, "negative control: effect absent, not executed");
const beforeLocale = declaration(beforePage, "locale").initializer;
const negativeControl = localizer.SUPPORTED_LANGUAGES.filter(language => language !== "en").map(language => {
  const locale = evaluate(beforeLocale, { language, languageLocale: localizer.languageLocale });
  const document = { documentElement: { lang: ssrLanguage } };
  // There is no old effect to invoke. Only the real old locale expression runs.
  assert.notEqual(locale, document.documentElement.lang);
  return { language, locale, htmlLanguage: document.documentElement.lang, effectPresent: false, effectExecuted: false };
});
const line = pageFile.getLineAndCharacterOfPosition(effect.getStart()).line + 1;
console.log(JSON.stringify({ scope: "controlled real-source AST/effect/storage fixture", uniqueCases: cases.length, cases, namedEffectLine: line, negativeControl, sameValueExternalWriter: "not distinguishable; outside sole-Page ownership contract", browserDOM: "UNRUN", actualPersistence: "UNRUN", inputs }, null, 2));
