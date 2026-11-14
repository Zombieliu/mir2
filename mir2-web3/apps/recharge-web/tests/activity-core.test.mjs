import test from "node:test";
import assert from "node:assert/strict";
import {checkedActivities, PortalSession} from "../public/core.js";

const keys = [["closed",30,"monthlyDays",30],["closed",40,"title",1],["closed",50,"cosmetic",1],["beta",40,"title",1],["beta",50,"cosmetic",1]];
function snapshot() {
  return {schemaVersion:9, phase:"closed", phaseStartMs:1700000000000, phaseEndMs:null,
    characters:[{index:3,name:"Scout",class:"Warrior",gender:"Male",level:50}],maxLevel:50,
    rewards:keys.map(([phase,level,kind,amount])=>({id:`${phase}-v1-lv${level}`,phase,kind,amount,
      requiredLevel:level,qualifiedLevel:phase === "closed" ? 50 : 0,
      eligible:phase === "closed",claimed:false,claimedAtMs:null,cosmeticClass:null})),
    carryover:{realVerifiedUsdMinor:0,verifiedCredits:0,monthlyDays:0,testOrdersExcluded:1,
      reviewOrders:0,records:[],settlement:"recorded"}};
}
test("campaign replies cannot invent reward IDs, amounts, received mail or cash provenance",()=>{
  const good=snapshot(); assert.equal(checkedActivities(good).rewards.length,5);
  for (const patch of [{schemaVersion:8},{maxLevel:-1},{phase:"officialLive"},
    {rewards:[...good.rewards.slice(1),good.rewards[1]]},
    {rewards:good.rewards.map((r,i)=> i===0 ? {...r,amount:300}:r)},
    {rewards:good.rewards.map((r,i)=> i===0 ? {...r,claimed:true}:r)},
    {carryover:{...good.carryover,monthlyDays:30}},
    {carryover:{...good.carryover,verifiedCredits:"1000"}},
    {carryover:{...good.carryover,records:[{orderId:"browser-says-paid",credits:100,amountUsdMinor:100,recordedAtMs:1,state:"recorded"}]}}])
    assert.throws(()=>checkedActivities({...good,...patch}),/invalidResponse/);
});
test("campaign claim binds current memory session and sends no amount, level, account or launch date",async()=>{
  let sent;
  const session=new PortalSession({fetcher:async(url,options)=>{sent={url,options};return {ok:true,text:async()=>JSON.stringify(snapshot())};}});
  session.token="own-bearer"; session.characters=[{index:3}];
  await session.claimActivity(3,"closed-v1-lv30");
  assert.equal(sent.url,"/playtest/v1/activities/claim");
  assert.deepEqual(JSON.parse(sent.options.body),{characterIndex:3,rewardId:"closed-v1-lv30",requestId:1});
  assert.equal(sent.options.headers.Authorization,"Bearer own-bearer");
  assert.equal(sent.options.cache,"no-store"); assert.equal(sent.options.credentials,"omit");
  await assert.rejects(session.claimActivity(4,"closed-v1-lv30"),/sessionExpired/);
  await assert.rejects(session.claimActivity(3,"admin-grant-1000"),/invalidIntent/);
});
test("account status works without a selected character and cannot send a bearer to another origin",async()=>{
  let sent;
  const session=new PortalSession({fetcher:async(url)=>{sent=url;return {ok:true,text:async()=>JSON.stringify(snapshot())};}});
  session.token="own-bearer";
  await session.activities(); assert.match(sent,/^\/playtest\/v1\/activities\/status\?requestId=1$/);
  await assert.rejects(session.api("/status",{},"https://evil.invalid"),/invalidIntent/);
});
test("campaign response after sign-out cannot restore the previous player's reservations",async()=>{
  let finish;
  const session=new PortalSession({fetcher:()=>new Promise(resolve=>{finish=resolve;})});
  session.token="own-bearer";
  const pending=session.activities(); await session.close();
  finish({ok:true,text:async()=>JSON.stringify(snapshot())});
  await assert.rejects(pending,/sessionExpired/);
});
