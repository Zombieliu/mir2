import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';
import { economy, exactSettlement } from './receipt-lib.mjs';

const [repo, root, fixture, leaf='browser-r1'] = process.argv.slice(2);
assert(repo && root && fixture && /^browser-r\d+$/.test(leaf));
const read = async path => JSON.parse(await readFile(path,'utf8'));
const [startup, identities, manifest] = await Promise.all([
  read(join(root,'startup.json')), read(join(fixture,'ui-identities.local.json')),
  read(join(repo,'apps/web/lib/generated/bevy_runtime_version.json')),
]);
const { ObservationReducer } = await import(pathToFileURL(join(repo,'apps/ai-player/src/observation.mjs')));
const { parseGatewayJson } = await import(pathToFileURL(join(repo,'apps/ai-player/src/lossless-json.mjs')));
const { chromium } = createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json')('playwright');
const out=join(root,leaf);
await mkdir(out);

const roles=['primary','peer'];
const amounts={primary:137,peer:59}, names={primary:'Candle',peer:'WoodenSword'};
const opposite=role=>role==='primary'?'peer':'primary';
const trace=[], states={}, moduleReads=[];
const hash = raw => createHash('sha256').update(raw).digest('hex');
const proof={schema:1,scope:'two normal WebGPU UI contexts, isolated File, wire passthrough',
  sourceHead:startup.sourceHead,pageSha256:startup.pageSha256,gatewaySha256:startup.gatewaySha256,
  runtime:manifest.version,passed:false,checks:[],screenshots:[],errors:[],modules:[],trace};
const relevant = new Set(['TradeRequest','TradeAccept','TradeItem','TradeGold','TradeCancel','TradeConfirm','DepositTradeItem',
  'GainedItem','GainedGold','LoseGold','StartGame','LogOutSuccess','UserLocation','ObjectTurn','ObjectWalk']);
const commandTypes = new Set(['tradeRequest','tradeReply','tradeGold','tradeConfirm','depositTradeItem','tradeCancel','logOut','turn','walk','run']);
const privateForbidden = /^(qa\.|event\.|stage5Command|moveTo|passkeyLogin|newAccount|newCharacter|dropItem|dropGold|useItem)/i;
let browser, fatal, phase='startup';
function step(next) { phase=next; console.log(next); }
function add(role, detail) { trace.push({order:trace.length+1,role,phase,...detail}); }
function check(name, details={}) { proof.checks.push({name,...details}); console.log(name); }
async function poll(sample, accept, error, timeout=30000) {
  const end=Date.now()+timeout;
  while(Date.now()<end) {
    if(fatal)throw Error(fatal);
    const current=await sample();
    if(accept(current))return current;
    await delay(70);
  }
  throw Error(error);
}
const packets=(role,name,after=0)=>trace.filter(row=>row.order>after&&row.role===role&&row.packet===name);
const sent=(role,type)=>trace.filter(row=>row.role===role&&row.command?.type===type);
const own=role=>states[role].observation.snapshot();
const readyWorld=role=>own(role).ready&&own(role).player?.name===identities[role].characterName;
const inventory=()=>Object.fromEntries(roles.map(role=>[role,economy(own(role))]));
const windowFor=role=>states[role].page.locator('[data-trade-state="open"]');
const offerSide=(role,side)=>windowFor(role).locator('[data-r16-trade-side="'+side+'"]');
const closed=role=>own(role).trade.active===false&&own(role).trade.uncertain===false&&states[role].rawTrade===null;
async function screen(role,name) {
  const file=name+'.png';
  const page=states[role].page;
  await page.screenshot({path:join(out,file),mask:[page.locator('.login-input:visible,[data-login-auth-field]:visible,.select-identity-id:visible')]});
  proof.screenshots.push({role,file,sha256:hash(await readFile(join(out,file)))});
}
function observe(role,raw) {
  const state=states[role];
  const event=parseGatewayJson(String(raw));
  state.observation.apply(event);
  if(event.type==='realmInfo')assert.equal(event.payload.realmId,startup.realmId);
  if(event.type==='error') {
    fatal='GATEWAY_ERROR';
    add(role,{errorCode:event.code??event.payload?.code??null});
  }
  if(event.type==='worldSnapshot') {
    state.rawTrade=event.payload.stage5Systems?.trade??null;
    if(phase==='settlement') add(role,{snapshot:true,gold:event.payload.gold,tick:event.payload.tick,
      trade:state.rawTrade?{prepared:state.rawTrade.escrowPrepared,locked:state.rawTrade.locked,offeredGold:state.rawTrade.offeredGold}:null,
      economyHash:hash(JSON.stringify(economy(event.payload)))});
    return;
  }
  if(event.type!=='packet'||!relevant.has(event.packet))return;
  const {from,to,success,unlock,amount,gold,result}=event.payload;
  const detail={packet:event.packet,from,to,success,unlock,amount,gold,result,sha256:hash(raw)};
  if(['UserLocation','ObjectTurn','ObjectWalk'].includes(event.packet)){
    const {location,direction}=event.payload;
    detail.position=location; detail.direction=direction;
  }
  if(event.packet==='GainedItem'){
    const item=event.payload.item;
    detail.item={uidHash:hash(String(item.unique_id)),itemIndex:item.item_index,count:item.count};
  }
  if(event.packet==='TradeItem')detail.items=event.payload.tradeItems?.filter(Boolean)
    .map(item=>({itemIndex:item.item_index,uidHash:hash(String(item.unique_id)),count:item.count}));
  if(['TradeGold','TradeConfirm','LoseGold','GainedGold','DepositTradeItem'].includes(event.packet))detail.wire=String(raw);
  add(role,detail);
}
async function openPage(role) {
  const context=await browser.newContext({locale:'en-US',viewport:{width:1280,height:960}});
  const page=await context.newPage(); page.setDefaultTimeout(20000);
  const observation=new ObservationReducer(); observation.resetConnection();
  const cdp=await context.newCDPSession(page), sockets=new Map();
  states[role]={page,context,observation,rawTrade:null,cdp};
  await cdp.send('Network.enable');
  cdp.on('Network.webSocketCreated',value=>{
    if(value.url===startup.gatewayUrl) {sockets.set(value.requestId,true); add(role,{connection:'created'});}
  });
  cdp.on('Network.webSocketClosed',value=>{if(sockets.has(value.requestId))add(role,{connection:'closed'});});
  page.on('framenavigated',frame=>{
    if(frame===page.mainFrame())add(role,{navigation:new URL(frame.url()).pathname});
  });
  page.on('pageerror',error=>proof.errors.push({role,type:'page',messageHash:hash(error.message)}));
  page.on('console',entry=>{
    if(entry.type()==='error')proof.errors.push({role,type:'console',messageHash:hash(entry.text())});
    if(/fast refresh|full reload/i.test(entry.text())) add(role,{devReloadSignal:true,messageHash:hash(entry.text())});
  });
  page.on('response',response=>{
    const url=new URL(response.url());
    if(response.status()>=400)proof.errors.push({role,type:'http',status:response.status(),path:url.pathname});
    if(url.pathname.endsWith('/mir2_bevy_runtime.js'))moduleReads.push(response.body().then(bytes=>{
      const sha256=hash(bytes); proof.modules.push({role,path:url.pathname,sha256,pinned:manifest.files.some(f=>f.path.endsWith('.js')&&f.sha256===sha256)});
    }).catch(()=>{}));
  });
  await page.routeWebSocket(startup.gatewayUrl,route=>{
    observation.resetConnection(); states[role].rawTrade=null;
    const ws=route.connectToServer();
    route.onMessage(raw=>{
      try {
        const v=parseGatewayJson(String(raw));
        assert(!privateForbidden.test(v.type));
        if(commandTypes.has(v.type))add(role,{command:v});
        ws.send(raw);
      } catch(error) { fatal='CLIENT_WIRE_FAILED'; add(role,{failureHash:hash(error.message)}); }
    });
    ws.onMessage(raw=>{
      try { observe(role,raw); route.send(raw); }
      catch(error) { fatal='SERVER_WIRE_FAILED'; add(role,{failureHash:hash(error.message)}); }
    });
  });
  await page.goto(startup.origin,{waitUntil:'commit',timeout:25000});
  const id=identities[role];
  await page.locator('.login-input.account:enabled').waitFor({timeout:90000});
  await page.locator('.login-input.account').fill(id.accountId);
  await page.locator('.login-input.password').fill(id.password);
  await page.locator('.login-button.ok button').click();
  await page.locator('.select-overlay').waitFor({timeout:60000});
  await enter(role);
  await cdp.send('Profiler.enable');
  await cdp.send('Profiler.startPreciseCoverage',{callCount:true,detailed:true});
}
async function enter(role) {
  const {page}=states[role], cursor=trace.length;
  await page.bringToFront();
  await page.locator('.select-character-slot-card').filter({hasText:identities[role].characterName}).click();
  await page.locator('.select-action.start button').click();
  await poll(()=>packets(role,'StartGame',cursor),rows=>rows.some(r=>r.result===4),'START_GAME_ACK_'+role,85000);
  await poll(()=>readyWorld(role),Boolean,'PLAYER_NOT_READY_'+role,85000);
  await poll(()=>page.locator('.client-stage-frame').getAttribute('class'),c=>c&&!c.includes('scene-assets-pending'),'SCENE_NOT_READY_'+role,85000);
}
async function requestAndAccept(tradePhase='settlement') {
  step(tradePhase);
  const cursor=trace.length;
  const inviter=states.peer.page, receiver=states.primary.page;
  for(const role of roles) {
    assert(await states[role].page.locator('main[data-client-screen="game"]').isVisible(),'LIVE_GAME_SCREEN_MISSING_'+role);
    assert(readyWorld(role),'LIVE_SOCKET_OBSERVATION_MISSING_'+role);
  }
  const p=own('primary').player,b=own('peer').player;
  assert.deepEqual([p.x,p.y,p.direction,b.x,b.y,b.direction],[288,616,'Down',288,617,'Up']);
  await poll(()=>own('peer').entities.some(e=>e.kind==='player'&&e.x===p.x&&e.y===p.y&&e.direction===p.direction),Boolean,'PEER_FACING_NOT_VISIBLE');
  await inviter.bringToFront(); await inviter.locator('.client-stage-frame').focus();
  for(const [key,y,direction] of [['ArrowDown',618,'Down'],['ArrowUp',617,'Up']]){
    await inviter.keyboard.press(key);
    await poll(()=>own('peer').player,actor=>actor.x===288&&actor.y===y&&actor.direction===direction,'ORDINARY_FACING_WALK_'+key);
    await delay(480);
  }
  await poll(()=>own('primary').entities.some(e=>e.kind==='player'&&e.x===288&&e.y===617&&e.direction==='Up'),Boolean,'OWNER_DID_NOT_OBSERVE_FACING_WALK');
  await inviter.keyboard.press('KeyT');
  await poll(()=>packets('primary','TradeRequest',cursor),r=>r.length===1,'REAL_TRADE_INVITE_MISSING');
  await receiver.bringToFront();
  await receiver.locator('[data-r16-trade-accept]').click();
  for(const role of roles) {
    await poll(()=>own(role).trade,s=>s.active&&s.ownOfferKnown&&!s.uncertain,'TRADE_OPEN_UNKNOWN_'+role,35000);
    await windowFor(role).waitFor();
  }
  check('normal-keyboard-invitation-and-visible-acceptance',{phase:tradePhase,mutualFacing:true,ordinaryPeerWalks:2});
}
async function offerItem(role,item) {
  const {page}=states[role]; await page.bringToFront();
  const choice=windowFor(role).locator('[data-r16-trade-bag-picker] button[title]').filter({hasText:item.name+' ×'+item.quantity});
  const cursor=trace.length;
  assert.equal(await choice.count(),1);
  await choice.click(); await offerSide(role,'own').locator('[data-trade-slot="0"]').click();
  await poll(()=>packets(role,'DepositTradeItem',cursor),r=>r.some(p=>p.from===item.slot&&p.to===0&&p.success===true),'ITEM_OFFER_ACK_'+role);
  await poll(()=>offerSide(role,'own').locator('[data-trade-slot="0"]').getAttribute('aria-label'),n=>n===item.name,'OWN_OFFER_NOT_VISIBLE_'+role);
  await poll(()=>offerSide(role,'own').locator('[data-r16-trade-gold]').isEnabled(),Boolean,'ITEM_BARRIER_DID_NOT_RELEASE_'+role);
}
async function offerGold(role,amount,before) {
  const {page}=states[role]; await page.bringToFront();
  const gold=offerSide(role,'own').locator('[data-r16-trade-gold]');
  await gold.click();
  await windowFor(role).locator('[data-r16-trade-gold-input]').fill(String(amount));
  await windowFor(role).locator('[data-r16-trade-gold-submit]').click();
  await poll(()=>own(role),s=>s.trade.ownGold===amount&&s.gold===before-amount,'OWN_GOLD_OFFER_NOT_APPLIED_'+role);
  await poll(()=>gold.innerText(),v=>v.trim()===String(amount),'GOLD_OFFER_NOT_VISIBLE_'+role);
}
async function confirm(role) {
  await states[role].page.bringToFront();
  await offerSide(role,'own').locator('[data-r16-trade-confirm]').click();
}
async function logout(role) {
  const {page}=states[role], cursor=trace.length;
  await page.bringToFront(); await page.locator('.client-stage-frame').focus();
  await page.keyboard.press('Alt+KeyX');
  const yes=page.locator('[role="dialog"]:visible').getByRole('button',{name:/^(Yes|是|确认|确定)$/});
  await delay(180);
  if(!packets(role,'LogOutSuccess',cursor).length&&await yes.count()===1)await yes.click();
  await poll(()=>packets(role,'LogOutSuccess',cursor),r=>r.length===1,'LOGOUT_ACK_'+role,40000);
  await page.locator('.select-overlay').waitFor();
}

try {
  browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',
    args:['--enable-unsafe-swiftshader','--disable-background-timer-throttling','--disable-renderer-backgrounding']});
  step('normal-ui-logins');
  for(const role of roles)await openPage(role);
  const before=inventory();
  proof.assetsBefore=before;
  assert.deepEqual({primary:before.primary.gold,peer:before.peer.gold},{primary:1000,peer:700});
  await requestAndAccept();
  const items=Object.fromEntries(roles.map(role=>[role,own(role).inventoryItems.find(item=>item.name===names[role])]));
  assert(roles.every(role=>items[role]?.quantity===1));
  for(const role of roles)await offerItem(role,items[role]);
  for(const role of roles)await offerGold(role,amounts[role],before[role].gold);
  for(const role of roles) {
    const other=opposite(role);
    await poll(()=>offerSide(role,'guest').locator('[data-trade-slot="0"]').getAttribute('title'),label=>label===names[other],'REMOTE_ITEM_DISPLAY_'+role);
    await offerSide(role,'guest').getByText(String(amounts[other]),{exact:true}).waitFor();
    await screen(role,role+'-offers-before-confirm');
  }
  check('both-ui-offers-visible-and-gold-held',{primary:863,peer:641});
  await confirm('primary');
  await poll(()=>states.primary.rawTrade,t=>t?.escrowPrepared===true&&t.locked===true,'FIRST_CONFIRM_NOT_PREPARED');
  await poll(()=>offerSide('primary','own').locator('[data-r16-trade-confirm]').getAttribute('aria-pressed'),v=>v==='true','FIRST_CONFIRM_NOT_VISIBLE');
  assert(await windowFor('peer').isVisible());
  assert(roles.every(role=>['TradeConfirm','GainedGold','GainedItem'].every(packet=>packets(role,packet).length===0)));
  const ownOffer=offerSide('primary','own');
  assert.equal(await ownOffer.locator('[data-trade-slot="0"]').getAttribute('aria-label'),'Candle');
  const heldGold=ownOffer.locator('[data-r16-trade-gold]');
  assert.equal((await heldGold.innerText()).trim(),'137');
  assert(await heldGold.isDisabled());
  await screen('primary','primary-prepared-with-quote');
  check('first-confirm-prepares-and-does-not-deliver',{first:'primary',deliveries:0,visibleOwnQuoteAndLock:true});
  await confirm('peer');
  for(const role of roles) {
    await poll(()=>closed(role),Boolean,'REAL_TRADE_DID_NOT_CLOSE_'+role,40000);
    await windowFor(role).waitFor({state:'hidden'});
  }
  const {expected,received}=exactSettlement(before,items,trace);
  proof.assetsAfter=inventory();
  assert.deepEqual(proof.assetsAfter,expected);
  proof.received=received;
  assert.equal(expected.primary.gold+expected.peer.gold,1700);
  for(const role of roles){
    for(const type of ['depositTradeItem','tradeGold','tradeConfirm'])assert.equal(sent(role,type).length,1);
    assert.equal(sent(role,'tradeCancel').length,0);
  }
  await screen('primary','primary-completed-wallet');
  check('once-only-item-and-gold-delivery-before-terminal',{primary:expected.primary.gold,peer:expected.peer.gold,total:1700});
  step('ordinary-save-readbacks');
  for(const role of roles)await logout(role);
  for(const role of roles) {
    await enter(role);
    assert.deepEqual(economy(own(role)),expected[role]);
    assert(closed(role));
  }
  check('same-role-ui-readback-and-no-open-trade',{bothExact:true});

  await requestAndAccept('prepared-unlock');
  const cursor=trace.length;
  const returnItem=own('primary').inventoryItems.find(item=>item.name==='BaseDress(M)');
  assert(returnItem?.quantity===1);
  await offerItem('primary',returnItem);
  await offerGold('primary',17,expected.primary.gold);
  await confirm('primary');
  await poll(()=>states.primary.rawTrade,t=>t?.escrowPrepared===true&&t.locked===true,'REFUND_CASE_NEVER_PREPARED');
  await poll(()=>offerSide('primary','own').locator('[data-r16-trade-confirm]').getAttribute('aria-pressed'),v=>v==='true','REFUND_UNLOCK_NOT_VISIBLE');
  await confirm('primary');
  for(const role of roles){
    await poll(()=>closed(role),Boolean,'REFUND_TRADE_DID_NOT_CLOSE_'+role,40000);
    await windowFor(role).waitFor({state:'hidden'});
    assert.equal(packets(role,'TradeCancel',cursor).filter(p=>p.unlock===false).length,1);
    assert.equal(packets(role,'TradeConfirm',cursor).length,0);
  }
  assert.equal(sent('primary','tradeConfirm').filter(r=>r.order>cursor&&r.command.locked===false).length,1);
  assert.deepEqual(inventory(),expected);
  check('prepared-unlock-refunds-once-under-current-r16-gateway',{walletRefunded:17,returnedItem:returnItem.name,bothExact:true});
  step('final-ordinary-logouts');
  for(const role of roles)await logout(role);
  proof.finalLogouts=true;
  proof.nativeBridgeCalls={};
  for(const role of roles) {
    const cdp=states[role].cdp;
    const result=await cdp.send('Profiler.takePreciseCoverage');
    proof.nativeBridgeCalls[role]=result.result.flatMap(script=>script.functions).filter(f=>f.functionName==='processMir2SocialBudget')
      .reduce((sum,f)=>sum+Math.max(...f.ranges.map(r=>r.count)),0);
    await cdp.send('Profiler.stopPreciseCoverage');
  }
  await Promise.all(moduleReads);
  assert(roles.every(role=>proof.nativeBridgeCalls[role]>0&&proof.modules.some(m=>m.role===role&&m.pinned&&m.path.includes('pkg-webgpu'))));
  proof.passed=true;
} catch(error) {
  const message=error.message.split('\n')[0];
  proof.failure={phase,name:error.name,code:error.code??null,messageHash:hash(error.message),
    safeCode:/^[A-Z0-9_ ]+$/.test(message)?message:null};
  await writeFile(join(root,'private',leaf+'-failure.local.txt'),error.stack??error.message,{flag:'wx'});
  for(const role of Object.keys(states))try{await screen(role,role+'-failure');}catch{}
} finally {
  await browser?.close();
  proof.browserClosed=true; proof.phase=phase;
  await writeFile(join(out,'proof.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
  console.log(JSON.stringify({passed:proof.passed,phase,checks:proof.checks.map(c=>c.name),failure:proof.failure??null}));
  process.exitCode=proof.passed?0:1;
}
