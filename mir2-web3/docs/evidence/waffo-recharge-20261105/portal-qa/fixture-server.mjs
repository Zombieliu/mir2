// Isolated UI-only fixture. It never calls a gateway, merchant API or payment
// provider. Public source files remain unchanged; fixture state is appended to
// the served app.js buffer and every fetch is confined to this loopback server.
import http from "node:http";
import {readFile,appendFile} from "node:fs/promises";
import path from "node:path";
import {fileURLToPath} from "node:url";

const root=fileURLToPath(new URL("../../../../apps/recharge-web/public/",import.meta.url));
const receipt=new URL("./requests.jsonl",import.meta.url);
const quote=(id,amount,credits)=>({id,label:"Local fixture quote",currency:"usd",amountMinor:amount,amountLabel:`USD ${(amount/100).toFixed(2)}`,credits});
const stripe=[quote("usd1",100,100),quote("usd5",500,500),quote("usd10",1000,1000),quote("usd20",2000,2000)];
const waffo=[quote("waffo-usd1",100,100),quote("waffo-usd5",500,500),quote("waffo-usd10",1000,1000),quote("waffo-usd20",2000,2000)];
const status={enabled:true,checkoutEnabled:true,credit:140,pendingCredit:0,offers:stripe,
  paymentProviders:[{id:"stripe",label:"Stripe",livemode:true,offers:stripe},{id:"waffo",label:"WAFFO",livemode:false,offers:waffo}],
  orders:[{id:"rch_"+"1".repeat(64),provider:"stripe",state:"applied"},{id:"rch_"+"2".repeat(64),provider:"waffo",state:"checkoutBound"}],
  monthlyCardCreditPrice:1000,monthlyCard:{active:false,serverNowMs:1800000000000,expiresAtMs:null}};
const character={index:7,name:"Local UI Fixture",level:12,class:"Warrior"};
const fixture=`
const fixtureStatus=${JSON.stringify(status)};
session.token="local-ui-fixture-only";session.accountId="local-ui-fixture";session.characters=[${JSON.stringify(character)}];
session.applyPending=async()=>fixtureStatus;
session.activities=async()=>{throw new PortalError("fixtureActivitiesUnavailable")};
session.close=async()=>{};
Object.assign(state,{loggedIn:true,scope:"loopback-only-ui-fixture",characterIndex:7,status:fixtureStatus});
byId("login-form").hidden=true;byId("signed-in").hidden=false;byId("account-name").textContent="local-ui-fixture";
const fixtureCharacter=node("option",null,"Local UI Fixture · Level 12");fixtureCharacter.value="7";
byId("character").replaceChildren(fixtureCharacter);byId("character").value="7";
const fixtureBanner=node("p","notice","LOCAL UI FIXTURE · simulated quotes and modes · no provider requests or charging");
document.querySelector("main").prepend(fixtureBanner);
renderStatus();
`;
const types={".html":"text/html; charset=utf-8",".css":"text/css; charset=utf-8",".js":"text/javascript; charset=utf-8",".svg":"image/svg+xml"};
const attempts=new Map();
const server=http.createServer(async(req,res)=>{
  const url=new URL(req.url,"http://127.0.0.1");
  const send=(code,body,type="application/json")=>{res.writeHead(code,{"Content-Type":type,"Cache-Control":"no-store","Content-Security-Policy":"default-src 'self'; connect-src 'self'; img-src 'self'; style-src 'self'; script-src 'self'; form-action 'none'"});res.end(body);};
  if(url.pathname==="/playtest/v1/billing/status") return send(200,JSON.stringify(status));
  if(url.pathname==="/playtest/v1/billing/checkout"&&req.method==="POST"){
    let body="";for await(const chunk of req)body+=chunk;
    const order=JSON.parse(body);await appendFile(receipt,JSON.stringify({at:new Date().toISOString(),body:order})+"\n");
    const count=(attempts.get(order.orderRequestId)||0)+1;attempts.set(order.orderRequestId,count);
    if(count===1)return send(503,JSON.stringify({error:"billingResultUnconfirmed"}));
    return send(200,JSON.stringify({provider:order.provider,orderId:"fixture-"+order.provider+"-order",
      checkoutUrl:order.provider==="waffo"?"https://cashier.waffo.com/local-fixture-do-not-pay":"https://checkout.stripe.com/c/pay/local-fixture-do-not-pay"}));
  }
  if(req.method!=="GET")return send(405,"method unavailable");
  try{
    const relative=decodeURIComponent(url.pathname).replace(/^\/+/,"")||"index.html";
    const file=path.resolve(root,relative);if(!file.startsWith(root))throw new Error("outside fixture");
    let body=await readFile(file);if(relative==="app.js")body=Buffer.concat([body,Buffer.from(fixture)]);
    send(200,body,types[path.extname(file)]||"application/octet-stream");
  }catch{return send(404,"not found");}
});
server.listen(17455,"127.0.0.1",()=>process.stdout.write("Local-only fixture: http://127.0.0.1:17455/#recharge\n"));
process.on("SIGINT",()=>server.close());
