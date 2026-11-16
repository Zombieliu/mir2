import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import ts from "typescript";
import { renderToStaticMarkup } from "react-dom/server";

const require = createRequire(import.meta.url);
const compile = (source, jsx = false) => ts.transpileModule(source, { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, jsx: jsx ? ts.JsxEmit.ReactJSX : undefined,
} }).outputText;
const load = path => { const exports = {}; new Function("exports", compile(readFileSync(new URL(path, import.meta.url), "utf8")))(exports); return exports; };
const { assetCacheProgressView } = load("../lib/asset-cache-progress.ts");
const { browserText, R16_BROWSER_LANGUAGES } = load("../lib/r16-browser-localization.ts");
const registrar = readFileSync(new URL("../app/components/asset-cache-registrar.tsx", import.meta.url), "utf8");
const ast = ts.createSourceFile("registrar.tsx", registrar, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const panel = ast.statements.find(n => ts.isFunctionDeclaration(n) && n.name?.text === "CacheProgressPanel");
assert(panel);
const renderPanel = (language, summary, runs) => {
  const exports = {};
  new Function("require", "exports", "assetCacheProgressView", "browserText", "R16_BROWSER_LANGUAGES", "document", "formatBytes",
    compile(panel.getText(ast) + "\nexport { CacheProgressPanel };", true))(
    require, exports, assetCacheProgressView, browserText, R16_BROWSER_LANGUAGES, { documentElement: { lang: language } }, n => `${n} B`);
  return renderToStaticMarkup(exports.CacheProgressPanel({ snapshot: { summary: { cachedLikeCount: 0, cacheStorageEntryCount: 12, transferBytes: 4096, ...summary }, prewarmRuns: runs } }));
};
const summary = (ok, failed, total = 113) => ({ prewarmRequested: total, prewarmOk: ok, prewarmFailed: failed });
const running = [{ status: "running" }], retrying = [{ status: "running", recovery: { status: "running" } }], done = [{ status: "done", recovery: { status: "done" } }];

test("an unfinished pack with failed attempts remains loading and does not mutate diagnostic history", () => {
  const totals = summary(32, 3), before = JSON.stringify(totals);
  const view = assetCacheProgressView(totals, running);
  assert.equal(view.state, "loading"); assert.equal(view.percent, 28); assert.equal(view.ok, 32); assert.equal(view.unresolved, 3);
  assert.equal(JSON.stringify(totals), before);
});
test("retrying three failed full reads is 110/113, not failed or 100 percent", () => {
  const view = assetCacheProgressView(summary(110, 3), retrying);
  assert.equal(view.state, "retrying"); assert.equal(view.percent, 97); assert(view.visible); assert.equal(view.unresolved, 3);
});
test("completed recovery hides the indicator and completed unresolved failures remain visible", () => {
  const recovered = assetCacheProgressView(summary(113, 0), done);
  assert.equal(recovered.state, "complete"); assert.equal(recovered.percent, 100); assert.equal(recovered.visible, false);
  const exhausted = assetCacheProgressView(summary(110, 3), done);
  assert.equal(exhausted.state, "failed"); assert.equal(exhausted.percent, 97); assert(exhausted.visible);
});
test("cancelled obsolete work does not make a new load look retrying, and later work reopens progress", () => {
  const cancelled = [{ status: "cancelled", recovery: { status: "running" } }];
  assert.equal(assetCacheProgressView(summary(0, 0, 0), cancelled).visible, false);
  const next = assetCacheProgressView(summary(113, 0, 120), [...cancelled, ...running]);
  assert.equal(next.state, "loading"); assert(next.visible); assert.equal(next.percent, 94);
});
test("the actual panel consumes the shared presentation and all ten dictionaries own its nine keys", () => {
  assert.match(panel.getText(ast), /assetCacheProgressView\(summary, snapshot.prewarmRuns\)/);
  assert(!panel.getText(ast).includes('prewarmFailed > 0 ? "失败"'));
  const visibility = ast.statements.find(n => ts.isFunctionDeclaration(n) && n.name?.text === "shouldShowCacheProgress");
  assert.match(visibility.getText(ast), /assetCacheProgressView\(snapshot.summary, snapshot.prewarmRuns\)\.visible/);
  for (const language of R16_BROWSER_LANGUAGES) for (const key of ["title", "loading", "retrying", "failed", "complete", "preparing", "local", "remote", "pending"]) {
    const text = browserText(language, "ui.assetCache." + key); assert.equal(typeof text, "string"); assert(text.trim());
  }
});

const fixtures = [];
for (const language of R16_BROWSER_LANGUAGES) {
  for (const [state, totals, runs] of [["loading", summary(32, 3), running], ["retrying", summary(110, 3), retrying], ["failed", summary(110, 3), done], ["complete", summary(113, 0), done]]) {
    test(`actual ${language} panel renders ${state} with matching language, counters and direction`, () => {
      const html = renderPanel(language, totals, runs);
      assert(html.includes(browserText(language, "ui.assetCache." + state))); assert(html.includes(`lang="${language}"`));
      assert(html.includes(`dir="${language === "ar" ? "rtl" : "ltr"}"`)); assert(html.includes(`${totals.prewarmOk}/${totals.prewarmRequested}`));
      if (state === "loading" || state === "retrying") assert(!html.includes(browserText(language, "ui.assetCache.failed")));
      const view = assetCacheProgressView(totals, runs);
      fixtures.push({ language, state, html: view.visible ? html : '<p>Completed; indicator hidden.</p>' });
    });
  }
}
test("regional document locales follow the selected canonical Windows language", () => {
  for (const [locale, language] of [["en-US", "en"], ["zh-Hant-TW", "zh-TW"], ["pt-PT", "pt-BR"], ["ar-SA", "ar"]]) {
    assert(renderPanel(locale, summary(110, 3), retrying).includes(`lang="${language}"`));
  }
});
test.after(() => {
  if (!process.env.MIR2_CACHE_PROGRESS_FIXTURE_OUTPUT) return;
  const cells = fixtures.map(f => `<article data-locale="${f.language}" data-state="${f.state}"><h2>${f.language} — ${f.state}</h2><div class="panel">${f.html}</div></article>`).join("");
  writeFileSync(process.env.MIR2_CACHE_PROGRESS_FIXTURE_OUTPUT, `<!doctype html><meta charset="utf-8"><title>Actual resource progress panels</title><style>body{background:#29251f;color:#eee;font:14px sans-serif}main{display:grid;grid-template-columns:repeat(4,310px);gap:8px}h2{font-size:14px;margin:4px}.panel{position:relative;width:308px;height:160px;border:1px solid #777}*{box-sizing:border-box}</style><main>${cells}</main>`, { flag: "wx" });
});
