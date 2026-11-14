import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import ts from "typescript";

const modules = new Map();
function moduleUrl(name) {
  if (!modules.has(name)) modules.set(name, (async () => {
    const source = await readFile(new URL(`../lib/${name}.ts`, import.meta.url), "utf8");
    let { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } });
    for (const match of [...outputText.matchAll(/from ["']@\/lib\/([^"']+)["']/g)]) outputText = outputText.replace(match[0], `from ${JSON.stringify(await moduleUrl(match[1]))}`);
    return `data:text/javascript;base64,${Buffer.from(outputText).toString("base64")}`;
  })());
  return modules.get(name);
}
// A historical deployment value must never replace the confirmed public origin.
process.env.NEXT_PUBLIC_SITE_URL="https://numeron-legend-of-rebirth.vercel.app";
process.env.VERCEL_URL="numeron-legend-of-rebirth.vercel.app";
const { getCopy, locales, getLocalizedAlternates, publicSiteUrl } = await import(await moduleUrl("site-copy"));
assert.equal(publicSiteUrl,"https://numeron.obelisk.build");
const { getProductCopy } = await import(await moduleUrl("product-copy"));
const { getDownloadCopy } = await import(await moduleUrl("download-copy"));
const { currentProduct, releaseFacts } = await import(await moduleUrl("release-facts"));
assert.equal(currentProduct.started, "2026-05");
assert.equal(currentProduct.internalValidationCompletedAt, null);
assert.equal(currentProduct.launched, false);
assert.equal(currentProduct.externalPlaytestStarted, true);
assert.equal(currentProduct.externalPlaytestStartedAt, null);
const retiredTestStates = ["قيد التطوير · الاختبارات الخارجية قيد التحضير", "Dalam pengembangan · Uji eksternal sedang disiapkan", "Đang phát triển · Đang chuẩn bị thử nghiệm bên ngoài", "กำลังพัฒนา · กำลังเตรียมทดสอบภายนอก", "Geliştirme aşamasında · Harici testler hazırlanıyor", "W rozwoju · Przygotowania do testów zewnętrznych", "В разработке · Подготовка внешних тестов", "En développement · Tests externes en préparation", "In Entwicklung · Externe Spieltests in Vorbereitung", "Em desenvolvimento · Testes externos em preparação", "En desarrollo · Pruebas externas próximamente", "開發中 · 外部測試準備中", "개발 중 · 외부 테스트 준비 중", "開発中 · 外部テスト準備中", "In development · Playtesting coming soon", "开发中 · 外部测试准备中"];
const forbidden=/12K\+|12,684|8,912|testnet\.numeron\.world|\bMir\b|\bBichon\b|\bSabuk\b|比奇|沙巴克|玛法|ミル|미르|비천/iu;
const problems=[];
function inspect(value,path){if(typeof value === "string" && forbidden.test(value))problems.push(`${path}: ${value.slice(0,100)}`);else if(Array.isArray(value))value.forEach((item,index)=>inspect(item,`${path}[${index}]`));else if(value && typeof value === "object")for(const [key,item]of Object.entries(value))inspect(item,`${path}.${key}`);}
for(const locale of locales){
  const site=getCopy(locale), product=getProductCopy(locale), download=getDownloadCopy(locale), facts=releaseFacts(locale);
  for(const path of ["","/watch","/download"]){
    const alternates=getLocalizedAlternates(locale,path);
    assert.equal(alternates.canonical,`${publicSiteUrl}/${locale}${path}`);
    for(const url of Object.values(alternates.languages))assert.equal(new URL(url).origin,publicSiteUrl);
  }
  inspect({site,product,download},locale);
  assert.ok(!retiredTestStates.includes(facts.state), `${locale}: retired external-testing state`);
  for (const retired of retiredTestStates) assert.ok(!JSON.stringify({site,product,download}).includes(retired), `${locale}: retired testing status remains`);
  assert.ok(facts.developmentStart.includes("2026"));
  assert.ok(!/2026|May|五月|5 月/.test(facts.currentValidation),`${locale}: current validation must not inherit the development start date`);
  assert.equal(site.metrics[0].label,facts.startedLabel);
  assert.equal(product.proof.copy,facts.currentValidation);
  assert.equal(product.proof.verified,facts.temporaryScreenshot);
  assert.ok(site.watch.liveCopy.includes(facts.temporaryScreenshot));
  assert.ok(download.intro.includes(facts.temporaryScreenshot));
  assert.ok(product.play.copy.includes(facts.temporaryScreenshot));
  assert.ok(site.membership.plans.every(plan=>plan.monthlyPrice === "—" && plan.annualPrice === "—"));
  assert.equal(site.atlas.metrics.length,0);
}
assert.deepEqual(problems,[],"Unsupported published copy remains");
const css=await readFile(new URL("../app/globals.css",import.meta.url),"utf8");
assert.match(css,/\.channel-rail span::after\s*\{\s*content:\s*"PLANNED"/);
assert.match(css,/\.watch-hero::after\s*\{\s*content:\s*"PLANNED \/ ARCHIVE/);
assert.match(css,/\.hero-art-label-top\s*\{\s*left:\s*5%;\s*right:\s*auto;\s*max-width:\s*55%/);
console.log(`Content verified across${locales.length} locales; development-start field separated from current undated internal validation.`);

if(process.argv.includes("--build")){
  let count=0;
  const buildRoot=new URL("../.next/server/app/",import.meta.url);
  async function htmlFiles(directory){
    const files=[];
    for(const entry of await readdir(new URL(directory,buildRoot),{withFileTypes:true})){
      const path=`${directory}${entry.name}`;
      if(entry.isDirectory())files.push(...await htmlFiles(`${path}/`));
      else if(entry.name.endsWith(".html"))files.push(path);
    }
    return files;
  }
  for(const path of await htmlFiles("")){
    if(!locales.some(locale=>path===`${locale}.html`||path.startsWith(`${locale}/`)))continue;
    const html=await readFile(new URL(path,buildRoot),"utf8");
    const text=html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi,"").replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi,"").replace(/<[^>]*>/g," ");
    assert.ok(!forbidden.test(text),`${path}: retired lore or fake number`);
    for (const retired of retiredTestStates) assert.ok(!text.includes(retired), `${path}: retired external-testing state`);
    assert.ok(!text.includes("120 SEC"),`${path}: unverified AI interval`);
    if(path.endsWith("/press.html"))assert.ok(html.includes("press-asset-note"),`${path}: missing temporary-image note`);
    if(path.endsWith("/play.html")){
      const start=html.indexOf('class="launch-copy"');
      const intro=html.slice(start,html.indexOf("</section>",start));
      assert.ok(start>=0 && intro.includes(releaseFacts(path.split("/")[0]).temporaryScreenshot),`${path}: missing visible temporary-image note beside launch image`);
    }
    assert.ok(!html.includes("SIGNED INSTALLER: N/A"));
    assert.ok(html.includes('"creativeWorkStatus":"In development"'));
    assert.ok(html.includes('"name":"Development start"') && html.includes('"name":"Current internal validation"'));
    const canonical=html.match(/<link\b[^>]*rel="canonical"[^>]*href="([^"]+)"/);
    assert.ok(canonical,`${path}: missing canonical`);
    assert.equal(new URL(canonical[1]).origin,publicSiteUrl,`${path}: wrong canonical origin`);
    for(const link of html.matchAll(/<link\b[^>]*rel="alternate"[^>]*href="([^"]+)"/g))assert.equal(new URL(link[1]).origin,publicSiteUrl,`${path}: wrong hreflang origin`);
    assert.ok(html.includes(`property="og:image" content="${publicSiteUrl}/og.png"`),`${path}: wrong OG image origin`);
    assert.ok(html.includes(`name="twitter:image" content="${publicSiteUrl}/og.png"`),`${path}: wrong Twitter image origin`);
    count++;
  }
  const sitemap=await readFile(new URL("sitemap.xml.body",buildRoot),"utf8");
  for(const location of sitemap.matchAll(/<loc>([^<]+)<\/loc>/g))assert.equal(new URL(location[1]).origin,publicSiteUrl);
  const robots=await readFile(new URL("robots.txt.body",buildRoot),"utf8");
  assert.ok(robots.includes(`Sitemap: ${publicSiteUrl}/sitemap.xml`));
  console.log(`Final build checked on${count} localized pages.`);
}
