import test from "node:test";
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {ACTIVITY_LOCALES, activityText, classText, clearedPortalSession, currentPortalResult,
  portalTab, registeredRewards, rewardView, renderActivities, renderPlayerCenter, sessionEndNotice} from "../public/activities.js";
import {LOCALES} from "../public/locales.js";

const character = {index:7,name:"Owned adventurer",class:"Warrior",gender:"Male",level:50};
const reward = {id:"closed-v1-lv40",phase:"closed",kind:"title",requiredLevel:40,amount:1,
  qualifiedLevel:50,eligible:true,claimed:false,claimedAtMs:null,cosmeticClass:null};
const context = {loggedIn:true,busy:false,characterIndex:7,characters:[character]};
const value = {schemaVersion:9,phase:"closed",phaseStartMs:1791753600000,phaseEndMs:null,
  characters:[character],maxLevel:50,rewards:[reward],
  carryover:{realVerifiedUsdMinor:0,verifiedCredits:0,monthlyDays:0,testOrdersExcluded:3,
    reviewOrders:0,records:[],settlement:"recorded"}};

test("navigation hashes select views only and never treat payment or script input as state", () => {
  assert.equal(portalTab("#player"),"player"); assert.equal(portalTab("#recharge"),"recharge");
  for (const hash of ["", "#paid", "#activities?paid=true", "#<script>", "#PLAYER"])
    assert.equal(portalTab(hash),"activities");
});

test("public activity authentication and ban errors end the portal session with the correct notice", () => {
  for (const code of ["activitiesAuthenticationRequired","billingAuthenticationRequired","sessionExpired"])
    assert.equal(sessionEndNotice(code),"sessionExpired");
  for (const code of ["activitiesAccountBanned","billingAccountBanned","accountBanned"])
    assert.equal(sessionEndNotice(code),"loginFailed");
  for (const code of ["activitiesTooManyRequests","activitiesDisabled","invalidResponse",undefined])
    assert.equal(sessionEndNotice(code),null);
});

test("revocation clears personal snapshots immediately and rejects results from an earlier login", () => {
  const active={epoch:4,loggedIn:true,busy:false,scope:"private-account",characterIndex:7,
    activities:value,status:{credit:500},activitiesFailed:true,offerId:"usd-5",
    checkout:{orderId:"private-order",checkoutUrl:"https://checkout.stripe.com/c/pay/test"},tab:"player"};
  assert.equal(currentPortalResult(active,4),true);
  for (const code of ["activitiesAuthenticationRequired","activitiesAccountBanned"]) {
    assert.ok(sessionEndNotice(code));
    const signedOut=clearedPortalSession(active);
    assert.equal(signedOut.epoch,5);assert.equal(signedOut.loggedIn,false);
    assert.equal(signedOut.activitiesFailed,false);assert.equal(signedOut.tab,"player");
    for (const key of ["scope","characterIndex","activities","status","offerId","checkout"])
      assert.equal(signedOut[key],null,key);
    assert.equal(currentPortalResult(signedOut,4),false);
    assert.equal(currentPortalResult(signedOut,5),false);
    const nextLogin={...signedOut,epoch:6,loggedIn:true};
    assert.equal(currentPortalResult(nextLogin,4),false);
    assert.equal(currentPortalResult(nextLogin,6),true);
  }
  assert.equal(active.loggedIn,true);assert.equal(active.activities,value);
});

test("client level alone cannot enable registration without server eligibility and an owned character", () => {
  assert.equal(rewardView(reward,"closed",context).canRegister,true);
  for (const change of [{loggedIn:false},{busy:true},{characterIndex:8},{characterIndex:null},{characters:[]}])
    assert.equal(rewardView(reward,"closed",{...context,...change}).canRegister,false);
  assert.equal(rewardView({...reward,eligible:false,qualifiedLevel:500},"closed",context).canRegister,false);
  assert.equal(rewardView(null,"closed",context).canRegister,false);
});

test("past, future, public and disabled phases remain locked even if a stale payload says eligible", () => {
  assert.equal(rewardView({...reward,phase:"beta"},"closed",context).key,"phaseLocked");
  assert.equal(rewardView(reward,"beta",context).key,"pastLocked");
  for (const phase of ["beta","public","disabled",undefined])
    assert.equal(rewardView(reward,phase,context).canRegister,false);
});

test("registered entitlement always presents public-launch reservation and cannot be registered again", () => {
  for (const phase of ["closed","beta","public","disabled"])
    assert.deepEqual(rewardView({...reward,claimed:true},phase,context),{key:"registeredReward",canRegister:false,progress:100});
  assert.equal(rewardView({...reward,qualifiedLevel:9},"closed",context).progress,22.5);
});

test("claim history includes only server-confirmed registrations and preserves stable reward order", () => {
  const a={...reward,id:"closed-v1-lv30",claimed:true,claimedAtMs:2};
  const b={...reward,id:"closed-v1-lv40",claimed:true,claimedAtMs:3};
  assert.deepEqual(registeredRewards({...value,rewards:[reward,a,b]}).map(row=>row.id),[b.id,a.id]);
  assert.deepEqual(registeredRewards(null),[]);
});

test("every new HTML label and activity key is translated in all nine existing languages", async () => {
  assert.deepEqual(Object.keys(ACTIVITY_LOCALES),Object.keys(LOCALES));
  const html=await readFile(new URL("../public/index.html",import.meta.url),"utf8");
  for (const [locale,texts] of Object.entries(ACTIVITY_LOCALES)) {
    assert.deepEqual(Object.keys(texts),Object.keys(ACTIVITY_LOCALES.en));
    for (const [,key] of html.matchAll(/data-activity-i18n="([^"]+)"/g)) assert.ok(texts[key],`${locale}:${key}`);
    for (const key of Object.keys(texts)) assert.ok(activityText(locale,key).trim(),`${locale}:${key}`);
    for (const klass of ["Warrior","Wizard","Taoist","Assassin","Archer"]) assert.notEqual(classText(locale,klass),"—");
    assert.ok(texts.testExclusion && texts.publicDelivery && texts.claimUnknown);
  }
});

// A small retained node tree checks actual render policy without pretending to
// validate browser layout or replacing the real visual acceptance gate.
class Node {
  constructor(tag){this.tagName=tag;this.children=[];this.attributes={};this.dataset={};this.textContent="";this.className="";
    this.classList={toggle:(name,on)=>{const parts=new Set(this.className.split(" ").filter(Boolean));on?parts.add(name):parts.delete(name);this.className=[...parts].join(" ");}};}
  append(...children){this.children.push(...children);}
  replaceChildren(...children){this.children=children;}
  setAttribute(key,value){this.attributes[key]=value;}
  addEventListener(name,callback){this[name]=callback;}
}
function flatten(node){return [node,...node.children.flatMap(flatten)];}
function rendered(callback){const previous=globalThis.document;globalThis.document={createElement:tag=>new Node(tag)};
  try {const container=new Node("section");callback(container);return flatten(container);}finally{globalThis.document=previous;}}

test("logged-out campaign rules contain no fabricated level, progress bar, eligibility or claim action", () => {
  const nodes=rendered(container=>renderActivities(container,{locale:"en",loggedIn:false,busy:false,value:null,onClaim(){throw new Error("no claim");}}));
  assert.equal(nodes.filter(node=>node.tagName==="progress").length,0);
  assert.equal(nodes.filter(node=>node.tagName==="button").length,0);
  assert.equal(nodes.filter(node=>node.className.includes("milestone-level")).length,3);
  assert.ok(nodes.some(node=>node.textContent===ACTIVITY_LOCALES.en.loginProgress));
});

test("registered titles and outfits render as reservations rather than immediate delivery", () => {
  const data={...value,rewards:[{...reward,claimed:true,claimedAtMs:1791753601000},
    {...reward,id:"closed-v1-lv50",kind:"cosmetic",requiredLevel:50,claimed:true,claimedAtMs:1791753602000,cosmeticClass:"Warrior"}]};
  const nodes=rendered(container=>renderActivities(container,{locale:"en",loggedIn:true,busy:false,characterIndex:7,value:data,onClaim(){throw new Error("already registered");}}));
  assert.equal(nodes.filter(node=>node.tagName==="button").length,0);
  assert.ok(nodes.some(node=>node.textContent===ACTIVITY_LOCALES.en.publicDelivery));
  assert.ok(nodes.some(node=>node.textContent===ACTIVITY_LOCALES.en.registeredReward));
  assert.ok(nodes.some(node=>node.textContent==="Warrior"));
});

test("personal rendering uses exact server values, excludes test orders and treats remote names as text", () => {
  const remoteName="<img src=x onerror=alert(1)>";
  const data={...value,characters:[{...character,name:remoteName}],carryover:{...value.carryover,testOrdersExcluded:5}};
  const nodes=rendered(container=>renderPlayerCenter(container,{locale:"en",loggedIn:true,busy:false,characterIndex:7,value:data,status:null,onSelect(){}}));
  assert.ok(nodes.some(node=>node.textContent===remoteName));
  assert.equal(nodes.filter(node=>node.tagName==="img").length,0);
  assert.ok(nodes.some(node=>node.textContent===ACTIVITY_LOCALES.en.testExclusion));
  assert.ok(nodes.some(node=>node.tagName==="dd"&&node.textContent==="5"));
  assert.equal(nodes.filter(node=>node.tagName==="strong"&&node.textContent==="—").length,3);
  assert.ok(nodes.some(node=>node.textContent==="$0.00"));
});

test("activity failure never promotes saved registration buttons while history remains visible", () => {
  const nodes=rendered(container=>renderActivities(container,{locale:"en",loggedIn:true,busy:false,failed:true,characterIndex:7,value,onClaim(){}}));
  const buttons=nodes.filter(node=>node.tagName==="button");assert.equal(buttons.length,1);assert.equal(buttons[0].disabled,true);
  assert.ok(nodes.some(node=>node.textContent===ACTIVITY_LOCALES.en.activityUnavailable));
});

test("carryover records do not imply a settled return or replace the actual character wallet", () => {
  const carryover={...value.carryover,realVerifiedUsdMinor:1900,verifiedCredits:1900,
    records:[{orderId:"known-order",amountUsdMinor:1900,credits:1900,recordedAtMs:1791753601000,state:"recorded"}]};
  const status={credit:700,pendingCredit:0,monthlyCard:{active:false,expiresAtMs:null}};
  const nodes=rendered(container=>renderPlayerCenter(container,{locale:"en",loggedIn:true,busy:false,
    characterIndex:7,value:{...value,carryover},status,onSelect(){}}));
  assert.ok(nodes.some(node=>node.tagName==="strong"&&node.textContent==="700"));
  assert.ok(nodes.some(node=>node.tagName==="dd"&&node.textContent==="1,900"));
  assert.ok(nodes.some(node=>node.tagName==="dd"&&node.textContent==="0"));
  assert.ok(nodes.some(node=>node.textContent===ACTIVITY_LOCALES.en.carryIntro));
  assert.match(ACTIVITY_LOCALES.en.carryIntro,/not a settled return or the current wallet balance/);
  assert.match(ACTIVITY_LOCALES.en.carryIntro,/require reconciliation/);
});
