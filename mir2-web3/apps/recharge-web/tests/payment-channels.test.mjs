import test from "node:test";
import assert from "node:assert/strict";
import {readFile} from "node:fs/promises";
import vm from "node:vm";
import * as core from "../public/core.js";
import * as locales from "../public/locales.js";
import * as activities from "../public/activities.js";

// Exercise the actual page event handlers with a retained node tree. Browser
// layout, live provider checkout and settlement are separate acceptance gates.
class Element {
  constructor(tag) {
    this.tagName=tag;this.children=[];this.attributes={};this.dataset={};this.listeners={};
    this.textContent="";this.className="";this.value="";this.hidden=false;this.disabled=false;
    this.classList={toggle:(name,on)=>{
      const parts=new Set(this.className.split(" ").filter(Boolean));
      on?parts.add(name):parts.delete(name);this.className=[...parts].join(" ");
    }};
  }
  append(...children) {this.children.push(...children);}
  replaceChildren(...children) {this.children=children;}
  setAttribute(key,value) {this.attributes[key]=value;}
  removeAttribute(key) {delete this.attributes[key];}
  addEventListener(name,callback) {this.listeners[name]=callback;}
  dispatchEvent(event) {return this.listeners[event.type]?.(event);}
  querySelectorAll(tag) {return this.children.flatMap(child=>[...(child.tagName===tag?[child]:[]),...child.querySelectorAll(tag)]);}
}
const quote=(id,amountMinor=1000)=>({id,currency:"usd",amountMinor,credits:1000});
const catalogue={enabled:true,checkoutEnabled:true,credit:0,pendingCredit:0,offers:[quote("stripe-usd10")],orders:[],
  monthlyCardCreditPrice:1000,monthlyCard:{active:false,serverNowMs:1800000000000,expiresAtMs:null},
  paymentProviders:[{id:"stripe",label:"Stripe",livemode:true,offers:[quote("stripe-usd10")]},
    {id:"waffo",label:"WAFFO",livemode:false,offers:[quote("waffo-usd10"),quote("waffo-usd20",2000)]}]};

async function page() {
  const html=await readFile(new URL("../public/index.html",import.meta.url),"utf8");
  const source=await readFile(new URL("../public/app.js",import.meta.url),"utf8");
  const elements=new Map([...html.matchAll(/<([a-z-]+)[^>]*\bid="([^"]+)"/g)].map(([,tag,id])=>[id,new Element(tag)]));
  const data=new Map(),storage={getItem:key=>data.get(key)??null,setItem:(key,value)=>data.set(key,value),removeItem:key=>data.delete(key)};
  let allowNewOrder=false,sequence=0;
  class Session {
    constructor(){this.requests=[];this.fail=true;}
    async checkout(character,offer,nonce,provider) {
      this.requests.push({character,offer,nonce,provider});
      if(this.fail) throw new core.PortalError("connectionTimeout");
      return {orderId:"waffo-known-order",provider,checkoutUrl:"https://cashier.waffo.com/known-order"};
    }
  }
  const context={...core,...locales,...activities,PortalSession:Session,
    renderActivities(){},renderCampaignRules(){},renderPlayerCenter(){},
    document:{getElementById:id=>elements.get(id),createElement:tag=>new Element(tag),querySelectorAll:()=>[],
      documentElement:{},addEventListener(){},hidden:false},
    localStorage:storage,sessionStorage:storage,navigator:{language:"en"},location:{hash:"#recharge"},
    window:{addEventListener(){},confirm:()=>allowNewOrder},Intl,Date,
    crypto:{randomUUID:()=>`00000000-0000-0000-0000-${String(++sequence).padStart(12,"0")}`},
    setInterval:()=>1,clearInterval(){},setTimeout,clearTimeout};
  const app=vm.runInNewContext(source.replace(/^import [\s\S]*?;\r?\n/gm,"")+"\n;({state,session,renderStatus,createCheckout})",context);
  Object.assign(app.state,{loggedIn:true,scope:"owned-account-hash",characterIndex:7,status:core.checkedStatus(catalogue)});
  app.renderStatus();
  return {...app,elements,storage,allowNewOrder:value=>{allowNewOrder=value;}};
}

test("the player chooses an advertised channel before seeing that channel's quotes and payment mode", async () => {
  const app=await page(),select=app.elements.get("payment-provider");
  assert.deepEqual(select.children.map(option=>option.value),["","stripe","waffo"]);
  assert.equal(app.elements.get("offers").querySelectorAll("button").length,0);
  assert.equal(app.elements.get("payment-mode").hidden,true);
  select.value="waffo";select.dispatchEvent({type:"change"});
  assert.deepEqual(app.elements.get("offers").querySelectorAll("button").map(button=>button.dataset.offerId),["waffo-usd10","waffo-usd20"]);
  assert.equal(app.elements.get("payment-mode-label").textContent,"WAFFO · Test payments");
  assert.equal(app.elements.get("payment-mode-detail").textContent,"No real money is charged.");
  select.value="stripe";select.dispatchEvent({type:"change"});
  assert.equal(app.elements.get("payment-mode-label").textContent,"Stripe · Live payments");
  assert.equal(app.elements.get("payment-mode-detail").textContent,"Real money is charged.");
});

test("an uncertain result freezes the provider and nonce until an explicit new-order action", async () => {
  const app=await page(),select=app.elements.get("payment-provider");
  select.value="waffo";select.dispatchEvent({type:"change"});
  await app.createCheckout("waffo-usd10");
  assert.equal(select.disabled,true);
  assert.equal(app.elements.get("new-order").hidden,false);
  assert.equal(app.state.intent.provider,"waffo");
  assert.equal(app.elements.get("offers").querySelectorAll("button")[1].disabled,true);
  select.value="stripe";select.dispatchEvent({type:"change"});
  assert.equal(select.value,"waffo");
  await app.createCheckout("stripe-usd10");
  assert.equal(app.session.requests.length,1);
  app.session.fail=false;await app.createCheckout("waffo-usd10");
  assert.equal(app.session.requests.length,2);
  assert.equal(app.session.requests[1].nonce,app.session.requests[0].nonce);
  assert.equal(app.session.requests[1].provider,"waffo");
  assert.equal(app.elements.get("checkout-link").textContent,"Continue to WAFFO");
  app.elements.get("new-order").dispatchEvent({type:"click"});
  assert.equal(app.state.intent.provider,"waffo");
  app.allowNewOrder(true);app.elements.get("new-order").dispatchEvent({type:"click"});
  assert.equal(app.state.intent,null);assert.equal(select.disabled,false);
  select.value="stripe";select.dispatchEvent({type:"change"});
  app.session.fail=true;await app.createCheckout("stripe-usd10");
  assert.equal(app.session.requests[2].provider,"stripe");
  assert.notEqual(app.session.requests[2].nonce,app.session.requests[1].nonce);
});
