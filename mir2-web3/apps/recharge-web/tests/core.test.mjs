import test from "node:test";
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import {IntentJournal, PortalSession, accountScope, checkedCheckoutUrl, checkedCharacters, checkedStatus, checkoutBody} from "../public/core.js";
import {LOCALES, normalizeLocale} from "../public/locales.js";

const status = {enabled:true, checkoutEnabled:true, credit:0, pendingCredit:0,
  offers:[{id:"usd10",currency:"usd",amountMinor:1000,credits:1000,amountLabel:"$10.00"}],
  orders:[], monthlyCardCreditPrice:1000,
  monthlyCard:{active:false,serverNowMs:1800000000000,expiresAtMs:null}};
test("payment links cannot change provider, protocol, port or authority", () => {
  assert.equal(checkedCheckoutUrl("https://checkout.stripe.com/c/pay/cs_test_sample#fragment"), "https://checkout.stripe.com/c/pay/cs_test_sample#fragment");
  for (const url of ["javascript:alert(1)","https://checkout.stripe.com.evil.example/c/pay/x",
    "https://user@checkout.stripe.com/c/pay/x","http://checkout.stripe.com/c/pay/x",
    "https://checkout.stripe.com:444/c/pay/x","https://checkout.stripe.com/other",
    "https://checkout.stripe.com\\@evil.example/c/pay/x"," https://checkout.stripe.com/c/pay/x"])
    assert.throws(() => checkedCheckoutUrl(url), /invalidCheckoutUrl/);
});

test("WAFFO checkout links are confined to the official merchant cashier and selected channel", () => {
  const url = "https://cashier.waffo.com/merchant-order-key?token=opaque";
  assert.equal(checkedCheckoutUrl(url, "waffo"), url);
  assert.throws(() => checkedCheckoutUrl(url), /invalidCheckoutUrl/);
  for (const value of ["http://cashier.waffo.com/order", "https://cashier.waffo.com.evil.example/order",
    "https://user@cashier.waffo.com/order", "https://cashier.waffo.com:444/order", "https://cashier.waffo.com/",
    "https://checkout.waffo.com/order", "https://pay.waffo.ai/order", "https://checkout.stripe.com/c/pay/other",
    "https://cashier.waffo.com/order\n", "https://cashier.waffo.com\\@evil.example/order"])
    assert.throws(() => checkedCheckoutUrl(value, "waffo"), /invalidCheckoutUrl/);
  assert.throws(() => checkedCheckoutUrl(url, "unconfigured"), /invalidCheckoutUrl/);
});

test("only advertised providers are offered and their quotes and modes stay independent", () => {
  const providers = [{id:"stripe",label:"Stripe",livemode:true,offers:status.offers},
    {id:"waffo",label:"WAFFO",livemode:false,offers:[{...status.offers[0],amountMinor:900}]}];
  const checked = checkedStatus({...status,paymentProviders:providers,orders:[{id:"waffo-order",provider:"waffo",state:"paidPending"}]});
  assert.deepEqual(checked.paymentProviders.map(provider => [provider.id,provider.livemode,provider.offers[0].amountMinor]),
    [["stripe",true,1000],["waffo",false,900]]);
  assert.equal(checked.orders[0].provider,"waffo");
  assert.equal(checked.advertisedProviders,true);
  assert.deepEqual(checkedStatus({...status,paymentProviders:[]}).paymentProviders,[]);
  assert.equal(checkedStatus(status).paymentProviders[0].id,"stripe");
  assert.equal(checkedStatus(status).advertisedProviders,false);
  assert.deepEqual(checkedStatus({...status,checkoutEnabled:false}).paymentProviders,[]);
  for (const paymentProviders of [[...providers,providers[0]], [{...providers[1],livemode:"false"}],
    [{...providers[1],id:"unknown"}], [{...providers[1],offers:[...status.offers,...status.offers]}]])
    assert.throws(() => checkedStatus({...status,paymentProviders}), /invalidResponse/);
});
test("catalogue rejects mixed order states, duplicate offers and invalid balances", () => {
  assert.equal(checkedStatus(status).credit,0);
  for (const patch of [{credit:-1},{credit:2**32},{credit:"1000"},{pendingCredit:0.5},
    {offers:[...status.offers,...status.offers]},{orders:[{id:"a",state:"paidByBrowser"}]},
    {offers:[{...status.offers[0],id:"../checkout"}]},{checkoutEnabled:"yes"}])
    assert.throws(() => checkedStatus({...status,...patch}), /invalidResponse/);
});
test("only the current authenticated character can become an order recipient", async () => {
  const session = new PortalSession({fetcher: () => { throw new Error("must never send"); }});
  session.token="owned-session";session.characters=[{index:26}];
  await assert.rejects(session.checkout(27,"usd10","web-00000000-0000-0000-0000-000000000001"), /sessionExpired/);
  assert.throws(() => checkedCharacters([{index:1,name:"a"},{index:1,name:"b"}]),/invalidResponse/);
});
test("checkout sends identifiers only, never client price, identity or paid claims", () => {
  const body = checkoutBody(26,"usd10","web-00000000-0000-0000-0000-000000000001",1);
  assert.deepEqual(Object.keys(body).sort(),["characterIndex","includePaymentProviders","offerId","orderRequestId","provider","requestId"]);
  assert.equal(body.includePaymentProviders,true);assert.equal(body.provider,"stripe");
  assert.throws(() => checkoutBody(26,"usd10","foreign nonce",1), /invalidIntent/);
  assert.deepEqual(checkoutBody(26,"usd10","web-00000000-0000-0000-0000-000000000001",1,"waffo"),
    {...body,provider:"waffo"});
  assert.throws(() => checkoutBody(26,"usd10","web-00000000-0000-0000-0000-000000000001",1,"unknown"), /invalidIntent/);
});

test("checkout verifies the selected provider before returning a payment link", async () => {
  let response = {orderId:"waffo-order",provider:"waffo",checkoutUrl:"https://cashier.waffo.com/order-key"}, request;
  const session = new PortalSession({fetcher:async (url,options) => {
    request={url,...options}; return {ok:true,text:async()=>JSON.stringify(response)};
  }});
  session.token="owned-session";session.characters=[{index:26}];
  const nonce="web-00000000-0000-0000-0000-000000000001";
  assert.equal((await session.checkout(26,"usd10",nonce,"waffo")).provider,"waffo");
  assert.equal(JSON.parse(request.body).provider,"waffo");
  response={...response,provider:"stripe"};
  await assert.rejects(session.checkout(26,"usd10",nonce,"waffo"), /invalidResponse/);
  delete response.provider;
  await assert.rejects(session.checkout(26,"usd10",nonce,"waffo"), /invalidResponse/);
  response={orderId:"legacy-stripe",checkoutUrl:"https://checkout.stripe.com/c/pay/legacy"};
  assert.equal((await session.checkout(26,"usd10",nonce)).provider,"stripe");
  assert.equal(JSON.parse(request.body).provider,"stripe");
  assert.equal(JSON.parse(request.body).includePaymentProviders,true);
});
test("lost reply and reload keep the same nonce and isolate accounts, characters and offers", async () => {
  const data=new Map(),storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value),removeItem:key=>data.delete(key)};
  let sequence=0;
  const uuid=()=>`00000000-0000-0000-0000-${String(++sequence).padStart(12,"0")}`;
  const alice=await accountScope("alice"),bob=await accountScope("bob");
  const journal=new IntentJournal(storage,uuid),first=journal.getOrCreate(alice,26,"usd10");
  assert.equal(new IntentJournal(storage,uuid).getOrCreate(alice,26,"usd10"), first);
  assert.notEqual(journal.getOrCreate(bob,26,"usd10"), first);
  assert.notEqual(journal.getOrCreate(alice,27,"usd10"), first);
  assert.notEqual(journal.getOrCreate(alice,26,"usd5"), first);
  assert.ok([...data.keys()].every(key=>!key.includes("alice")&&!key.includes("bob")));
  assert.ok([...data.values()].every(value=>/^web-/.test(value)));
  journal.clear(alice,26,"usd10"); assert.notEqual(journal.getOrCreate(alice,26,"usd10"), first);
});
test("unavailable or corrupt storage stops financial requests", () => {
  const journal=new IntentJournal({getItem(){throw new Error("blocked");}});
  assert.throws(()=>journal.getOrCreate("scope",26,"usd10"), /storageUnavailable/);
  assert.throws(()=>new IntentJournal({getItem:()=>"invalid"}).getOrCreate("scope",26,"usd10"), /invalidIntent/);
});

test("channel-specific retry intents preserve legacy Stripe orders and never reuse WAFFO nonces", () => {
  const data=new Map(), storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value),removeItem:key=>data.delete(key)};
  const old="web-00000000-0000-0000-0000-000000000001", fresh="00000000-0000-0000-0000-000000000002";
  data.set("numeron-checkout-v1:scope:26:usd10",old);
  const journal=new IntentJournal(storage,()=>fresh);
  assert.equal(journal.getOrCreate("scope",26,"usd10","stripe"),old);
  const waffo=journal.getOrCreate("scope",26,"usd10","waffo");
  assert.notEqual(waffo,old);
  assert.equal(new IntentJournal(storage).getOrCreate("scope",26,"usd10","waffo"),waffo);
  journal.clear("scope",26,"usd10","stripe");
  assert.equal(data.has("numeron-checkout-v1:scope:26:usd10"),false);
  assert.equal(journal.getOrCreate("scope",26,"usd10","waffo"),waffo);
  assert.ok([...data.keys()].some(key=>key.includes(":waffo:")));
});
test("HTTP billing uses a memory bearer, no cookies, redirects or cached personal data", async () => {
  let request;
  const session=new PortalSession({fetcher:async (url,options)=>{
    request={url,options};return{ok:true,text:async()=>JSON.stringify(status)};
  }});
  session.token="owned-bearer";session.characters=[{index:26}];
  assert.equal((await session.status(26)).credit,0);
  assert.equal(request.options.headers.Authorization,"Bearer owned-bearer");
  assert.equal(request.options.credentials,"omit"); assert.equal(request.options.cache,"no-store");
  assert.equal(request.options.redirect,"error"); assert.match(request.url,/characterIndex=26/);
  assert.match(request.url,/includePaymentProviders=true/);
  assert.ok(!request.url.includes("owned-bearer"));
});
test("a reply received after sign-out cannot revive the old account", async () => {
  let release;
  const session=new PortalSession({fetcher:()=>new Promise(resolve=>{release=resolve;})});
  session.token="old-bearer";session.characters=[{index:26}];
  const pending=session.status(26); await session.close();
  release({ok:true,text:async()=>JSON.stringify(status)});
  await assert.rejects(pending,/sessionExpired/); assert.equal(session.token,null);
});
test("native browser fetch keeps its Window receiver when called through the session", async () => {
  const original=globalThis.fetch;
  globalThis.fetch=function(){
    assert.equal(this,globalThis,"browser fetch rejects another receiver");
    return Promise.resolve({ok:true,text:async()=>JSON.stringify(status)});
  };
  try {
    const session=new PortalSession();session.token="owned-bearer";session.characters=[{index:26}];
    assert.equal((await session.status(26)).credit,0);
  } finally { globalThis.fetch=original; }
});
test("website sign-out clears memory immediately and revokes only its own grant before closing", async () => {
  const sent=[],closed=[];let request,release;
  const session=new PortalSession({fetcher:(url,options)=>{
    request={url,options};return new Promise(resolve=>{release=resolve;});
  }});
  session.token="own-bearer";session.sessionId="own-website-session";
  session.accountId="alice";session.characters=[{index:26}];
  session.socket={readyState:1,send:value=>sent.push(JSON.parse(value)),close:()=>closed.push(true)};
  const closing=session.close();
  assert.equal(session.token,null);assert.equal(session.sessionId,null);
  assert.equal(session.accountId,null);assert.deepEqual(session.characters,[]);
  assert.equal(closed.length,0);
  assert.equal(request.url,"/playtest/v1/identity/sessions/revoke");
  assert.deepEqual(JSON.parse(request.options.body),{sessionId:"own-website-session",reason:"recharge_website_sign_out"});
  assert.equal(request.options.keepalive,true);assert.equal(request.options.credentials,"omit");
  assert.equal(request.options.headers.Authorization,"Bearer own-bearer");
  release({ok:true});await closing;
  assert.deepEqual(sent,[{type:"disconnect"}]);assert.equal(closed.length,1);
});
test("nine languages cover every visible text and include RTL and locale normalization", async () => {
  const html=await readFile(new URL("../public/index.html",import.meta.url),"utf8");
  for(const [locale,texts] of Object.entries(LOCALES)) {
    for(const [,key] of html.matchAll(/data-i18n="([^"]+)"/g)) assert.ok(texts[key],`${locale}:${key}`);
    assert.deepEqual(Object.keys(texts),Object.keys(LOCALES.en));
  }
  assert.equal(Object.keys(LOCALES).length,9);
  assert.equal(normalizeLocale("zh-CN"),"zh-Hant");assert.equal(normalizeLocale("pt-PT"),"pt-BR");
  assert.equal(normalizeLocale("ru-RU"),"ru");assert.equal(normalizeLocale("<script>"),"en");
});
test("the page never treats a return query as payment authority or renders remote HTML", async () => {
  const app=await readFile(new URL("../public/app.js",import.meta.url),"utf8");
  assert.ok(!app.includes("innerHTML"));assert.ok(!app.includes("location.search"));
  assert.ok(!app.includes("localStorage.setItem(\"password"));
});
