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
const { getCopy, locales } = await import(await moduleUrl("site-copy"));
const { getProductCopy } = await import(await moduleUrl("product-copy"));
const { getDownloadCopy } = await import(await moduleUrl("download-copy"));
const { currentProduct, releaseFacts } = await import(await moduleUrl("release-facts"));
assert.equal(currentProduct.started, "2026-05");
assert.equal(currentProduct.internalValidationCompletedAt, null);
assert.equal(currentProduct.launched, false);
assert.equal(currentProduct.externalPlaytestStarted, false);
const forbidden=/12K\+|12,684|8,912|testnet\.numeron\.world|\bMir\b|\bBichon\b|\bSabuk\b|比奇|沙巴克|玛法|ミル|미르|비천/iu;
const problems=[];
function inspect(value,path){if(typeof value === "string" && forbidden.test(value))problems.push(`${path}: ${value.slice(0,100)}`);else if(Array.isArray(value))value.forEach((item,index)=>inspect(item,`${path}[${index}]`));else if(value && typeof value === "object")for(const [key,item]of Object.entries(value))inspect(item,`${path}.${key}`);}
for(const locale of locales){
  const site=getCopy(locale), product=getProductCopy(locale), download=getDownloadCopy(locale), facts=releaseFacts(locale);
  inspect({site,product,download},locale);
  assert.ok(facts.developmentStart.includes("2026"));
  assert.ok(!/2026|May|五月|5 月/.test(facts.currentValidation),`${locale}: current validation must not inherit the development start date`);
  assert.equal(site.metrics[0].label,facts.startedLabel);
  assert.equal(product.proof.copy,facts.currentValidation);
  assert.equal(product.proof.verified,facts.temporaryScreenshot);
  assert.ok(site.watch.liveCopy.includes(facts.temporaryScreenshot));
  assert.ok(download.intro.includes(facts.temporaryScreenshot));
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
    assert.ok(!text.includes("120 SEC"),`${path}: unverified AI interval`);
    if(path.endsWith("/press.html"))assert.ok(html.includes("press-asset-note"),`${path}: missing temporary-image note`);
    assert.ok(!html.includes("SIGNED INSTALLER: N/A"));
    assert.ok(html.includes('"creativeWorkStatus":"In development"'));
    assert.ok(html.includes('"name":"Development start"') && html.includes('"name":"Current internal validation"'));
    count++;
  }
  console.log(`Final build checked on${count} localized pages.`);
}
