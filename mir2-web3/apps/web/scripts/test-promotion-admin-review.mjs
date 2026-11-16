import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";
import { createRequire } from "node:module";
import ts from "typescript";
const require = createRequire(import.meta.url);

function load(name) {
  const source=readFileSync(new URL(`../app/promotions/${name}.ts`,import.meta.url),"utf8");
  const output=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
  const module={exports:{}};
  vm.runInNewContext(output,{module,exports:module.exports,TextEncoder});
  return module.exports;
}
const input=load("promotion-admin-input");
const base={document:'{"amountMicroUsd":"9007199254740993","coinAmountAtomic":"18446744073709551615"}',hash:"a".repeat(64),resourceId:"payout-fixture",evidenceRef:"independent-review-fixture"};
const plain=value=>JSON.parse(JSON.stringify(value));

test("review documents preserve exact monetary strings and reject lossy numeric input",()=>{
  assert.equal(input.parsePromotionReviewJson(base.document).amountMicroUsd,"9007199254740993");
  assert.equal(input.parsePromotionReviewJson(base.document).coinAmountAtomic,"18446744073709551615");
  for(const value of ['{"amount":9007199254740993}','{"nested":{"amount":1.01}}','[]','null','"document"','{"constructor":{}}','{"__proto__":{}}','{"x":"'+"x".repeat(12000)+'"}']){
    assert.throws(()=>input.parsePromotionReviewJson(value));
  }
});

test("launch and payment instructions remain explicit draft submissions with exact parsed documents",()=>{
  const launch=plain(input.promotionReviewCommand("proposeLaunch",base));
  assert.deepEqual(launch,{action:"proposeLaunch",manifest:JSON.parse(base.document),evidenceRef:base.evidenceRef});
  const freeze=plain(input.promotionReviewCommand("freezePayoutIntent",base));
  assert.deepEqual(freeze,{action:"freezePayoutIntent",intent:JSON.parse(base.document),evidenceRef:base.evidenceRef});
  for(const value of [launch,freeze]){
    assert.equal(value.actor,undefined);
    assert.equal(value.enabled,undefined);
    assert.equal(value.signatures,undefined);
  }
});

test("every approval is bound to a separately entered complete lowercase SHA-256",()=>{
  assert.deepEqual(plain(input.promotionReviewCommand("approveLaunch",base)),{action:"approveLaunch",manifestSha256:base.hash,evidenceRef:base.evidenceRef});
  assert.deepEqual(plain(input.promotionReviewCommand("approvePayout",base)),{action:"approvePayout",payoutId:base.resourceId,intentSha256:base.hash,evidenceRef:base.evidenceRef});
  for(const hash of ["","a".repeat(63),"a".repeat(65),"A".repeat(64),"z".repeat(64)]){
    for(const action of ["approveLaunch","approvePayout"]) assert.throws(()=>input.promotionReviewCommand(action,{...base,hash}));
  }
  assert.throws(()=>input.promotionReviewCommand("approvePayout",{...base,resourceId:""}));
  assert.throws(()=>input.promotionReviewCommand("approvePayout",{...base,resourceId:"../other"}));
});

test("payment enable and disable require an explicit supported action and evidence",()=>{
  assert.deepEqual(plain(input.promotionReviewCommand("payoutOn",base)),{action:"setPayout",enabled:true,evidenceRef:base.evidenceRef});
  assert.deepEqual(plain(input.promotionReviewCommand("payoutOff",base)),{action:"setPayout",enabled:false,evidenceRef:base.evidenceRef});
  assert.throws(()=>input.promotionReviewCommand("submitPayout",base));
  assert.throws(()=>input.promotionReviewCommand("payoutOn",{...base,evidenceRef:""}));
});

test("actual-revenue funding removes budget and member caps without manufacturing a cash pool",()=>{
  const result=plain(input.promotionFundingLimits("revenue_share",{budget:"",memberLimit:"",leaderLimit:"",perLeaderMemberLimit:"",revenueCap:""}));
  assert.deepEqual(result,{fundingMode:"revenue_share",budgetMicroUsd:0,memberLimit:0,leaderLimit:0,perLeaderMemberLimit:0,perMemberRevenueCapMicroUsd:0});
  const capped=plain(input.promotionFundingLimits("budget_capped",{budget:"100.000001",memberLimit:"200",leaderLimit:"5",perLeaderMemberLimit:"50",revenueCap:"30"}));
  assert.equal(capped.budgetMicroUsd,100000001);
  assert.equal(capped.perMemberRevenueCapMicroUsd,30000000);
  assert.equal(capped.memberLimit,200);
  assert.throws(()=>input.promotionFundingLimits("budget_capped",{budget:"",memberLimit:"",leaderLimit:"",perLeaderMemberLimit:"",revenueCap:""}));
  assert.throws(()=>input.promotionFundingLimits("unknown",{}));
});

test("authorized three-month season dates are represented in UTC without local-time drift",()=>{
  const season=input.PROMOTION_DRAFT_SEASON_UTC;
  assert.equal(Date.parse(season.startsAt+"Z"),Date.parse("2026-11-01T00:00:00+08:00"));
  assert.equal(Date.parse(season.endsAt+"Z"),Date.parse("2027-02-01T00:00:00+08:00"));
});

test("all supported administrator locales contain independent launch and payment review copy",()=>{
  const dictionaries=load("promotion-admin-copy").PROMOTION_LAUNCH_COPY;
  assert.equal(Object.keys(dictionaries).length,10);
  const keys=Object.keys(dictionaries.en).sort();
  for(const [locale,copy] of Object.entries(dictionaries)){
    assert.deepEqual(Object.keys(copy).sort(),keys);
    for(const value of Object.values(copy)) assert.ok(value.trim(),locale);
    if(locale!=="en") assert.notEqual(copy.executionPolicy,dictionaries.en.executionPolicy);
  }
  const funding=load("promotion-admin-copy").PROMOTION_FUNDING_COPY;
  assert.equal(Object.keys(funding).length,10);
  for(const [locale,copy] of Object.entries(funding)){
    assert.equal(Object.keys(copy).length,6);
    for(const value of Object.values(copy))assert.ok(value.trim());
    assert.ok(copy.actualRevenue.includes("USD")&&copy.actualRevenue.includes("USDC"),locale);
    assert.ok(copy.manifestRequirements.includes("minimumPayoutMicroUsd")&&copy.manifestRequirements.includes("identityTaxPolicyRef"),locale);
    if(locale!=="en")assert.notEqual(copy.noFixedBudget,funding.en.noFixedBudget);
  }
});

test("administrator review displays server switch state, configuration and exact manifest hashes",()=>{
  const cache=new Map();
  function componentModule(name){
    if(cache.has(name)) return cache.get(name);
    const extension=name==="promotion-admin"?"tsx":"ts";
    const source=readFileSync(new URL(`../app/promotions/${name}.${extension}`,import.meta.url),"utf8");
    const output=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS,jsx:ts.JsxEmit.ReactJSX,esModuleInterop:true}}).outputText;
    const module={exports:{}};
    vm.runInNewContext(output,{module,exports:module.exports,TextEncoder,require:specifier=>{
      if(specifier.endsWith(".module.css")) return {__esModule:true,default:new Proxy({},{get:(_target,key)=>String(key)})};
      if(specifier.startsWith("./")) return componentModule(specifier.slice(2));
      if(specifier==="../../lib/promotion-season-context") return load(specifier);
      return require(specifier);
    }});
    cache.set(name,module.exports);return module.exports;
  }
  const {createElement}=require("react");
  const {renderToStaticMarkup}=require("react-dom/server");
  const Review=componentModule("promotion-admin").PromotionLaunchReview;
  const dictionaries=load("promotion-copy").PROMOTION_COPY;
  const labels=load("promotion-admin-copy").PROMOTION_LAUNCH_COPY;
  const data={payoutEnabled:false,launchConfiguration:{budgetMicroUsd:"9007199254740993",fundingMode:"revenue_share"},launchConfigurationSha256:"a".repeat(64),launchApproval:{manifestSha256:"b".repeat(64),manifest:{configurationSha256:"a".repeat(64),network:"sui:testnet"},proposedBy:"operations-fixture",approvedBy:null}};
  for(const locale of ["en","zh-Hant","ar"]){
    const html=renderToStaticMarkup(createElement(Review,{data,copy:dictionaries[locale],locale}));
    assert.ok(html.includes(dictionaries[locale].closed));
    assert.ok(html.includes(dictionaries[locale].pending));
    assert.ok(html.includes(data.launchConfigurationSha256));
    assert.ok(html.includes(data.launchApproval.manifestSha256));
    assert.ok(html.includes("9007199254740993"));
    assert.ok(!html.includes(`<strong>${labels[locale].payoutState}: ${labels[locale].enabled}</strong>`));
  }
});
