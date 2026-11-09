import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import test from "node:test";
import ts from "typescript";

const require = createRequire(import.meta.url);
const root = fileURLToPath(new URL("../", import.meta.url));
const modules = new Map();
function load(path) {
  if (modules.has(path)) return modules.get(path);
  if (extname(path) === ".json") return require(path);
  const module = { exports: {} };
  const code = ts.transpileModule(readFileSync(path, "utf8"), { compilerOptions: {
    module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX, esModuleInterop: true,
  } }).outputText;
  new Function("require", "module", "exports", code)(name => {
    if (name === "./original-client-overlays") {
      return { SpriteButton: ({ label }) => createElement("button", { "aria-label": label }) };
    }
    if (!name.startsWith(".")) return require(name);
    const target = resolve(dirname(path), name);
    return load(extname(target) ? target : existsSync(`${target}.ts`) ? `${target}.ts` : `${target}.tsx`);
  }, module, module.exports);
  modules.set(path, module.exports);
  return module.exports;
}

const { HelpWindow } = load(resolve(root, "app/components/original-client-help-window.tsx"));
const { buildTranslator } = load(resolve(root, "lib/localization.ts"));
const { defaultCrystalKeyBindings } = load(resolve(root, "lib/player-ui-preferences.ts"));
const catalog = require(resolve(root, "lib/generated/r16-native-localization.json"));
const entries = new Map(catalog.entries.map(entry => [entry.key, entry.values]));
const escaped = value => renderToStaticMarkup(createElement("span", null, value)).slice(6, -7);
const render = (language, page, bindings) => renderToStaticMarkup(createElement(HelpWindow, {
  language, page, bindings, t: buildTranslator(language), onClose() {}, scrollOffsets: new Map(),
}));
const numberedKey = (prefix, index) => `${prefix}.${String(index).padStart(2, "0")}`;

test("all three Help shortcut pages use Windows R16 copy and order in every locale", () => {
  const rowStarts = [0, 18, 36, 39];
  for (const [index, { code, dir }] of catalog.languages.entries()) {
    for (let page = 0; page < 3; page++) {
      const html = render(code, page);
      const caption = escaped(entries.get("game.013")[index]);
      assert.ok(html.includes(`role="dialog" aria-label="${caption}"`));
      assert.ok(html.includes(`>${caption}</span>`));
      assert.doesNotMatch(html, /\/original-ui\/Title\/57\.png/);
      for (const key of [page === 2 ? "game.015" : "game.014", "game.016", "game.017"]) {
        assert.ok(html.includes(escaped(entries.get(key)[index])), `${code}, page ${page + 1}, ${key}`);
      }
      let offset = 0;
      for (let row = rowStarts[page]; row < rowStarts[page + 1]; row++) {
        const key = numberedKey("help.shortcut", row);
        const next = html.indexOf(escaped(entries.get(key)[index]), offset);
        assert.ok(next >= offset, `${code}, page ${page + 1}, ${key}`);
        offset = next + 1;
      }
      assert.ok(!html.includes('role="region"') && !html.includes("/original-ui/Help/"));
      if (page === 2) {
        assert.match(html, /dir="ltr"[^>]*>\/\(username\)<\/span>/);
        assert.match(html, /!~\(text\)/);
        assert.ok(html.includes(`<bdi dir="${dir}">`));
      }
    }
  }
});

test("Help shows the player's edited binding next to the translated description", () => {
  const bindings = defaultCrystalKeyBindings();
  Object.assign(bindings.find(binding => binding.function === "Guilds"), { key: "F9", ctrl: 1, alt: 0, shift: 0, tilde: 0 });
  const html = render("zh-CN", 0, bindings);
  assert.match(html, /Ctrl \+ F9<\/span><span[^>]+>行会（打开\/关闭）<\/span>/);
});

test("all translated image pages contain both shared text fields and no original English bitmap", () => {
  for (const [index, { code, dir }] of catalog.languages.entries()) {
    if (code === "en") continue;
    for (let image = 0; image < 42; image++) {
      const html = render(code, image + 3);
      const key = numberedKey("help.page", image);
      const title = entries.get(`${key}.title`)[index];
      assert.ok(html.includes(`role="region" aria-label="${escaped(title)}" dir="${dir}"`), `${code} ${key} title`);
      assert.ok(html.includes(escaped(entries.get(`${key}.body`)[index])), `${code} ${key} body`);
      assert.ok(html.includes(escaped(entries.get("help.scroll_hint")[index])), `${code} ${key} hint`);
      assert.doesNotMatch(html, /\/original-ui\/Help\//);
    }
  }
});

test("English and unsupported legacy locales render the original Help bitmap range", () => {
  for (const code of ["en", "es"]) {
    for (let image = 0; image < 42; image++) {
      const html = render(code, image + 3);
      assert.ok(html.includes(`src="/original-ui/Help/${image}.png"`));
      assert.doesNotMatch(html, /role="region"/);
    }
  }
});
