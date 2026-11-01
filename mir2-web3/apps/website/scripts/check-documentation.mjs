import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const root = new URL("../", import.meta.url);
async function contentModule(name) {
  const source = await readFile(new URL(`lib/${name}.ts`, root), "utf8");
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`);
}
const { guideArticles } = await contentModule("guide-content");
const { developerArticles } = await contentModule("developer-content");
const collections = { guide: guideArticles, developers: developerArticles };
const routes = Object.entries(collections).flatMap(([area, articles]) => [`/zh-CN/${area}`, ...articles.map(({ slug }) => `/zh-CN/${area}/${slug}`)]);
const knownLinks = new Set(["download", "play", "support", "status", "updates"]);
const publicText = JSON.stringify(collections);
const privateSignals = /[A-Z]:\\|C:\/Users\/|BEGIN (?:RSA |EC )?PRIVATE KEY|sk-proj-[A-Za-z0-9]|NUMERON_INTERNAL_DOCS_EVIDENCE_20261102|GAME-DESIGN-CODEBASE/;
assert.ok(!privateSignals.test(publicText), "Internal paths, economic proposals or private source material entered public content");

let links = 0;
let sections = 0;
const externalLinks = new Set();
for (const [area, articles] of Object.entries(collections)) {
  assert.equal(new Set(articles.map(({ slug }) => slug)).size, articles.length, `Duplicate slugs: ${area}`);
  for (const article of articles) {
    assert.ok(["live", "testing", "planned"].includes(article.status));
    assert.ok(article.environment && article.description);
    assert.equal(new Set(article.sections.map(({ id }) => id)).size, article.sections.length);
    for (const section of article.sections) {
      sections++;
      assert.match(section.id, /^[a-z][a-z0-9-]*$/);
      assert.ok(section.paragraphs?.length || section.items?.length || section.table || section.code, `Empty section: ${article.slug}/${section.id}`);
      if (section.table) for (const row of section.table.rows) assert.equal(row.length, section.table.headers.length);
      for (const { href } of section.links ?? []) {
        links++;
        if (href.startsWith("http")) { externalLinks.add(href); continue; }
        if (href === "@promotions" || knownLinks.has(href)) continue;
        if (href.startsWith("/zh-CN/docs-examples/")) { await readFile(new URL(`public${href}`, root)); continue; }
        assert.ok(routes.includes(`/zh-CN/${href}`), `Broken documentation link: ${href}`);
      }
    }
  }
}

let checkedRoutes = 0;
let checkedAnchors = 0;
const originIndex = process.argv.indexOf("--origin");
if (originIndex >= 0) {
  const origin = new URL(process.argv[originIndex + 1]);
  const get = async (path) => {
    const response = await fetch(new URL(path, origin), { signal: AbortSignal.timeout(15000) });
    assert.equal(response.status, 200, `${path}: HTTP ${response.status}`);
    return response.text();
  };
  for (const path of routes) {
    const html = await get(path);
    checkedRoutes++;
    assert.ok(html.includes('id="docs-content"'));
    assert.ok(html.includes(`href="/zh-CN${path.slice(6)}"`));
    assert.ok(!privateSignals.test(html), `Private material in rendered route: ${path}`);
    const ids = new Set([...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]));
    for (const match of html.matchAll(/href="#([^"]+)"/g)) {
      assert.ok(ids.has(match[1]), `Missing anchor ${match[1]} on ${path}`);
      checkedAnchors++;
    }
  }
  for (const locale of ["en", "ja", "ar"]) {
    const html = await get(`/${locale}/developers/quickstart`);
    assert.ok(html.includes("Translations are pending review."));
    assert.ok(html.includes('content="noindex, follow"'));
    assert.ok(html.includes('lang="zh-CN" dir="ltr"'));
    checkedRoutes++;
  }
  for (const path of ["/zh-CN/guide/not-a-real-page", "/invalid-locale/developers", "/docs/GAME-DESIGN-CODEBASE-DRAFT.zh-CN.md", "/docs/NUMERON-WEBSITE-DOCS-20261102.zh-CN.md", "/.env.production"]) {
    const response = await fetch(new URL(path, origin));
    assert.equal(response.status, 404, `Unknown route must 404: ${path}`);
  }
  for (const path of ["/zh-CN", "/zh-CN/download", "/zh-CN/support"]) {
    const html = await get(path);
    for (const area of ["guide", "developers"]) assert.ok((html.match(new RegExp(`href="/zh-CN/${area}"`, "g")) ?? []).length >= 2, `Header/footer docs missing: ${path}`);
    checkedRoutes++;
  }
  const example = await get("/zh-CN/docs-examples/read-status.mjs");
  assert.equal(example, await readFile(new URL("public/zh-CN/docs-examples/read-status.mjs", root), "utf8"));
  const sitemap = await get("/sitemap.xml");
  for (const path of routes) assert.ok(sitemap.includes(path));
  assert.ok(!sitemap.includes("/en/guide"), "Unreviewed translations must not be advertised in sitemap");
}

let publicBuildFiles = 0;
async function inspectBuild(path) {
  let entries;
  try { entries = await readdir(path, { withFileTypes: true }); } catch (error) { if (error.code === "ENOENT") return; throw error; }
  for (const entry of entries) {
    const target = new URL(`${entry.name}${entry.isDirectory() ? "/" : ""}`, path);
    if (entry.isDirectory()) { await inspectBuild(target); continue; }
    if (!/\.(?:html|rsc|txt|js|css)$/.test(entry.name)) continue;
    const text = await readFile(target, "utf8");
    assert.ok(!privateSignals.test(text), `Internal evidence or private proposal leaked: ${fileURLToPath(target)}`);
    publicBuildFiles++;
  }
}
await inspectBuild(new URL(".next/static/", root));
for (const area of ["guide", "developers"]) await inspectBuild(new URL(`.next/server/app/zh-CN/${area}/`, root));
console.log(JSON.stringify({ passed: true, articles: guideArticles.length + developerArticles.length, sections, contentLinks: links, checkedRoutes, checkedAnchors, publicBuildFiles, externalLinks: [...externalLinks] }, null, 2));
