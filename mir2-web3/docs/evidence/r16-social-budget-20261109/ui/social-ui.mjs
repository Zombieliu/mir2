import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {dirname,join} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import readline from 'node:readline';
const root=dirname(fileURLToPath(import.meta.url)),repo='E:/mir2-player-journey/mir2-web3';
const req=createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json');
const {chromium}=req('playwright');
const webRequire=createRequire(join(repo,'apps/web/package.json'));
const Ws=webRequire('next/dist/compiled/ws');
const {GatewayClient}=await import(pathToFileURL(join(repo,'apps/ai-player/src/gateway-client.mjs')));
const {ObservationReducer}=await import(pathToFileURL(join(repo,'apps/ai-player/src/observation.mjs')));
const {parseGatewayJson}=await import(pathToFileURL(join(repo,'apps/ai-player/src/lossless-json.mjs')));
const config=JSON.parse(await readFile(join(root,'public-config.json'),'utf8'));
const identityRoot='D:/mir2-builds/r16-parity-20261109/trade-cancel-ui-r1/private';
const state=JSON.parse(await readFile(join(identityRoot,'state.local.json'),'utf8'));
const identities=JSON.parse(await readFile(join(identityRoot,'ui-identities.local.json'),'utf8'));state.owner=identities.primary;state.actor=identities.peer;
const reuse=true;
const manifest=JSON.parse(await readFile(join(repo,'apps/web/lib/generated/bevy_runtime_version.json'),'utf8'));assert.equal(manifest.version,'bevy-0cd68510886134e0');
const leaf=process.env.R16_QA_BROWSER_RUN??'browser-r1';assert(/^browser-r[1-9][0-9]*$/.test(leaf));
const run=join(root,leaf);await mkdir(run);
const hash=v=>createHash('sha256').update(typeof v==='string'||Buffer.isBuffer(v)?v:JSON.stringify(v)).digest('hex');
const proof={schema:1,passed:false,phase:'startup',runtimeVersion:manifest.version,scope:{isolatedLoopback:true,existingOrdinaryAccounts:2,newRegistrations:0,reusedExistingIsolatedAccounts:reuse,primaryPhysicalUi:true,transparentRealServerWebSocket:true,onlyMatchingItemAckDelayed:true,passiveCdpCoverage:true,noBridgeOrGameActionInvokedByDriver:true,guildMembershipNotFabricated:true,modelCalls:0,privilegedCommands:0},checks:[],events:[],commands:{primary:{},peer:{}},bridgeCoverage:[],moduleResponses:[],screenshots:[],errors:[],httpErrors:[],pageErrors:[],consoleErrors:[]};
const note=(type,fields={})=>proof.events.push({at:Date.now(),type,...fields});
const phase=name=>{proof.phase=name;console.log(JSON.stringify({phase:name}));};
const poll=async(read,predicate,code,ms=15000)=>{const end=Date.now()+ms;while(Date.now()<end){if(fatal)throw Error(fatal);const v=await read();if(await predicate(v))return v;await delay(75);}throw Error(code);};
const check=(name,fields={})=>{proof.checks.push({name,passed:true,...fields});console.log(JSON.stringify({check:name,passed:true}));};
const command=(who,v)=>{const type=v.type??'invalid';proof.commands[who][type]=(proof.commands[who][type]??0)+1;note('command',{who,command:type,...(['depositTradeItem','retrieveTradeItem'].includes(type)?{from:v.from,to:v.to}:{})});assert(!/^(qa\.|event\.|moveTo|stage5Command|passkeyLogin|tradeConfirm|newAccount|newCharacter)/i.test(type),'FORBIDDEN_COMMAND');};
const serverObservation=new ObservationReducer(),deliveredObservation=new ObservationReducer();
serverObservation.resetConnection();deliveredObservation.resetConnection();
let browser,context,page,peer,hold=null,fatal=null,sequence=0,clientRoute,rawTrade=null,lastItemCommand=null,rawGuild=null,cdp=null,coverageStarted=false;
const moduleReads=[];
const packets=[];
const observe=(e,delivered)=>{
 if(e.type==='realmInfo')assert(e.payload.realmId===config.realmId&&e.payload.profileId==='platinum_fiveclass','REALM_MISMATCH');
 if(e.type==='error'){fatal='REAL_GATEWAY_ERROR';note('gateway-error',{code:e.payload?.code??e.code??null});}
 (delivered?deliveredObservation:serverObservation).apply(e);
 if(!delivered&&e.type==='worldSnapshot'){rawTrade=e.payload.stage5Systems?.trade??null;rawGuild=e.payload.stage5Systems?.guild??null;}
 if(!delivered&&e.type==='packet'){
  const item={at:Date.now(),seq:++sequence,packet:e.packet,payload:e.payload};packets.push(item);
  if(['StartGame','LogOutSuccess','SwitchGroup','AddMember','DeleteMember','DeleteGroup','GroupInvite','GroupMemberInfo','GroupMembersMap','TradeRequest','TradeAccept','DepositTradeItem','RetrieveTradeItem','TradeCancel'].includes(e.packet))note('server-packet',{packet:e.packet,result:e.payload.result,success:e.payload.success,from:e.payload.from,to:e.payload.to,unlock:e.payload.unlock});
 }
};
const current=()=>serverObservation.snapshot();
const econ=s=>({gold:s.gold,inventory:s.inventoryItems.map(i=>({uidHash:hash(String(i.uniqueId)),slot:i.slot,count:i.quantity,name:i.name})).sort((a,b)=>a.slot-b.slot),belt:s.beltItems.map(i=>({uidHash:hash(String(i.uniqueId)),slot:i.slot,count:i.quantity})).sort((a,b)=>a.slot-b.slot)});
const snapshot=()=>page.evaluate(()=>{const main=document.querySelector('main');return{screen:main?.dataset.clientScreen,tradeVisible:!!document.querySelector('[data-trade-state="open"]'),bagInput:main?.dataset.bagInputStatus??null,movement:main?.dataset.movementInputStatus??null,focus:document.activeElement?.tagName,frameClass:document.querySelector('.client-stage-frame')?.className};});
const shot=async name=>{const file=name+'.png';await page.screenshot({path:join(run,file),mask:[page.locator('.login-input.account,.login-input.password,[data-login-auth-field]')],maskColor:'#090909',timeout:15000});proof.screenshots.push(file);};
const focus=async()=>{const focused=await page.evaluate(()=>document.hasFocus()&&!/^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement?.tagName??''));if(!focused){const canvas=page.locator('.client-stage-frame canvas:visible').first();await canvas.focus();}};
const packetsAfter=(seq,name)=>packets.filter(p=>p.seq>seq&&p.packet===name);
async function registration(){
 phase('ordinary-primary-ui-registration');
 await page.locator('.login-input.account').waitFor({state:'visible',timeout:90000});
 await poll(()=>page.locator('.login-input.account').isEnabled(),Boolean,'LOGIN_NOT_READY',90000);
 await page.locator('.login-button.account button').click();
 const form=page.locator('[data-login-auth-surface="registration"]');await form.waitFor();
 const fields={accountId:state.owner.accountId,password:state.owner.password,confirmPassword:state.owner.password,userName:'Isolated QA',birthDate:'2000-01-01',secretQuestion:'Private local test',secretAnswer:hash(state.owner.password).slice(0,20),emailAddress:state.owner.accountId+'@example.invalid'};
 for(const [field,value]of Object.entries(fields))await form.locator(`[data-login-auth-field="${field}"]`).fill(value);
 const seq=sequence;await form.locator('[data-login-auth-submit="registration"]').click();
 await poll(()=>packetsAfter(seq,'NewAccount'),p=>p.some(v=>v.payload.result===8),'PRIMARY_REGISTER_NO_SUCCESS');
 if(await form.isVisible())await form.locator('[data-login-auth-close]').first().click();
 check('primary-ordinary-ui-registration',{result:8});
}
async function loginAndCreate(){
 phase('primary-ui-existing-character-login-enter');
 await page.locator('.login-input.account').fill(state.owner.accountId);await page.locator('.login-input.password').fill(state.owner.password);
 await page.locator('.login-button.ok button').click();await page.locator('.select-overlay').waitFor({timeout:60000});
 if(!reuse){
 await page.locator('.select-action.new button').click();await page.locator('.select-create-name-field input').fill(state.owner.characterName);
 const seq=sequence;await page.locator('.select-create-actions button').first().click();
 const created=await poll(()=>packetsAfter(seq,'NewCharacterSuccess'),p=>p.length,'PRIMARY_CREATE_NO_SUCCESS');state.owner.characterIndex=created.at(-1).payload.character.index;
 }
 const slot=page.locator('.select-character-slot-card').filter({hasText:state.owner.characterName});await slot.click();
 await page.locator('.select-action.start button').click();
 await poll(current,s=>s.ready&&s.player?.name===state.owner.characterName,'PRIMARY_WORLD_NOT_READY',90000);
 await poll(()=>page.locator('.client-stage-frame').getAttribute('class'),s=>s&&!s.includes('scene-assets-pending'),'SCENE_NOT_READY',90000);
 await shot('01-primary-in-town');check('primary-ui-existing-character-in-real-world',{position:{x:current().player.x,y:current().player.y},map:current().mapFileName});
}
async function peerStart(){
 phase('ordinary-existing-peer-login-enter');
 class OriginWs extends Ws{constructor(url){super(url,{origin:config.origin});}}
 peer=new GatewayClient({url:config.gatewayUrl,realmId:config.realmId,WebSocketImpl:OriginWs,timeoutMs:20000});
 peer.on('command',v=>command('peer',v));await peer.connect();if(!reuse)await peer.createAccount(state.actor.accountId,state.actor.password);await peer.login(state.actor.accountId,state.actor.password);
 if(!reuse){const character=await peer.createCharacter(state.actor.characterName);state.actor.characterIndex=character.index;}
 await peer.startGame(state.actor.characterIndex);
 if(!reuse)await writeFile(join(root,'private/ui-identities.local.json'),JSON.stringify({primary:state.owner,peer:state.actor}),{flag:'wx',mode:0o600});
 check('second-ordinary-player-same-realm',{adjacentInitially:Math.max(Math.abs(current().player.x-peer.snapshot().player.x),Math.abs(current().player.y-peer.snapshot().player.y))<=1});
}
const dirs=['Up','UpRight','Right','DownRight','Down','DownLeft','Left','UpLeft'];
const delta=[[0,-1],[1,-1],[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1]];
async function align(){
 phase('ordinary-adjacent-facing');
 let a=current().player,b=peer.snapshot().player;
 // New characters share the ordinary newcomer location. Move only the peer
 // one legal tile if it overlaps; the owner never receives hidden actions.
 const facing=dirs.indexOf(a.direction);assert(facing>=0,'PRIMARY_DIRECTION_NOT_KNOWN');
 const wanted={x:a.x+delta[facing][0],y:a.y+delta[facing][1]};
 for(let n=0;n<8&&(b.x!==wanted.x||b.y!==wanted.y);n++){
  const dx=Math.sign(wanted.x-b.x),dy=Math.sign(wanted.y-b.y),index=delta.findIndex(([x,y])=>x===dx&&y===dy);
  assert(index>=0,'PEER_ALIGNMENT_INVALID_DIRECTION');await peer.walk(dirs[index]);await delay(650);b=peer.snapshot().player;
 }
 assert(b.x===wanted.x&&b.y===wanted.y,'PEER_NOT_AT_PRIMARY_FACING_TILE');
 const index=(facing+4)%8;await peer.turn(dirs[index]);
 await poll(()=>peer.snapshot(),s=>s.entities.some(e=>e.kind==='player'&&e.x===a.x&&e.y===a.y&&e.direction===a.direction),'MUTUAL_FACING_NOT_OBSERVED');
 // The ordinary peer requests the trade; incoming UI acceptance needs no
 // owner-facing mutation and is exactly the production invitation path.
 check('ordinary-peer-adjacent-facing',{primaryPosition:{x:a.x,y:a.y},peerPosition:{x:b.x,y:b.y},peerDirection:dirs[index]});
}
async function beginTrade(){
 await focus();const seq=sequence;
 await peer.requestPlayerTrade();
 await poll(()=>packetsAfter(seq,'TradeRequest'),p=>p.length,'PRIMARY_INVITE_NOT_RECEIVED');
 const accept=page.locator('[data-r16-trade-accept]');await accept.waitFor({timeout:10000});await accept.click();
 await poll(current,s=>s.trade.active&&s.trade.ownOfferKnown&&!s.trade.uncertain,'REAL_TRADE_NOT_ACTIVE');
 await page.locator('[data-trade-state="open"]').waitFor();
 await poll(()=>page.locator('[data-r16-trade-bag-picker] button[title]').first().isEnabled(),Boolean,'TRADE_ITEM_UI_NOT_READY');
}
function arm(packet){assert(!hold,'ACK_HOLD_ALREADY_ACTIVE');hold={packet,raw:null,seq:null,receivedAt:null,releasedAt:null};}
function release(){assert(hold?.raw,'NO_REAL_HELD_ACK');const h=hold;observe(h.event,true);clientRoute.send(h.raw);h.releasedAt=Date.now();note('held-ack-released',{packet:h.packet,rawSha256:hash(h.raw),heldForMs:h.releasedAt-h.receivedAt});hold=null;return h;}
async function deposit(item){
 const button=page.locator('[data-r16-trade-bag-picker] button[title]').filter({hasText:item.name+' ×'+item.quantity});
 assert.equal(await button.count(),1,'OWNED_ITEM_SELECTOR_NOT_UNIQUE');await button.click();await page.locator('[data-r16-trade-side="own"] button[data-trade-slot="0"]').click();
}
async function cancelHeld(kind,baseline){
 await poll(()=>hold,h=>h?.raw,'MATCHING_REAL_ACK_NOT_RECEIVED');
 const held=hold;assert.equal(held.event.payload.success,true,'REAL_ITEM_OPERATION_REJECTED');
 assert.equal(held.event.payload.from,held.expected.from);assert.equal(held.event.payload.to,held.expected.to);
 let expected=structuredClone(baseline);
 if(kind==='retrieve'){
  const item=expected.inventory.find(i=>i.uidHash===proof.starter.uidHash);assert(item,'RETRIEVED_ITEM_NOT_OWNED');
  item.slot=held.expected.to;expected.inventory.sort((a,b)=>a.slot-b.slot);
 }
 const picker=page.locator('[data-r16-trade-bag-picker] button[title]').first();
 assert(await picker.isDisabled(),'ITEM_PENDING_NOT_VISIBLE');
 const cancel=page.locator('[data-r16-trade-cancel]');assert(await cancel.isEnabled(),'CANCEL_DISABLED_DURING_ITEM_PENDING');
 note('pending-ui-before-cancel',{operation:kind,ackWithheld:true,cancelEnabled:true,allItemControlsDisabled:true});await shot(kind+'-pending-cancel-enabled');
 const seq=sequence;await cancel.click();
 await poll(()=>packetsAfter(seq,'TradeCancel'),p=>p.some(v=>v.payload.unlock===false),'REAL_TERMINAL_TRADE_CANCEL_MISSING');
 await poll(current,s=>!s.trade.active&&!s.trade.uncertain&&JSON.stringify(econ(s))===JSON.stringify(expected),'CANCEL_ASSETS_NOT_EXACTLY_RESTORED');
 await page.locator('[data-trade-state="open"]').waitFor({state:'hidden'});
 await poll(()=>peer.snapshot(),s=>!s.trade.active&&!s.trade.uncertain,'PEER_TRADE_NOT_CLOSED');
 const cancels=proof.commands.primary.tradeCancel,operations=proof.commands.primary[kind==='deposit'?'depositTradeItem':'retrieveTradeItem'];
 const old=release();await delay(1300);
 assert.equal(await page.locator('[data-trade-state="open"]').count(),0,'LATE_ACK_REOPENED_TRADE');
 assert.deepEqual(econ(current()),expected);assert.equal(proof.commands.primary.tradeCancel,cancels);assert.equal(proof.commands.primary[kind==='deposit'?'depositTradeItem':'retrieveTradeItem'],operations);
 check(kind+'-pending-cancel-real-server-restores-assets',{heldPacket:old.packet,from:old.expected.from,to:old.expected.to,heldMs:old.releasedAt-old.receivedAt,realSuccess:true,terminalUnlock:false,exactUidSlotQuantityAndGoldRestored:true,retrievalSlotMoveExpected:kind==='retrieve',lateAckReleasedUnmodified:true,noResurrection:true,noRepeatedEconomicCommands:true});await shot(kind+'-cancelled-after-old-ack');
}
async function scenarios(){
 const baseline=econ(current()),peerBaseline=econ(peer.snapshot());
 proof.assetsBefore={primary:baseline,peer:peerBaseline};
 const item=current().inventoryItems.find(i=>i.name==='Candle');assert(item&&item.quantity>0,'NO_ORDINARY_STARTER_CANDLE');
 proof.starter={name:item.name,quantity:item.quantity,uidHash:hash(String(item.uniqueId)),slot:item.slot};
 if(process.env.R16_QA_RETRIEVE_ONLY!=='1'){
  phase('deposit-ack-delay-ui-cancel');await beginTrade();arm('DepositTradeItem');await deposit(item);await cancelHeld('deposit',baseline);
  assert.deepEqual(econ(peer.snapshot()),peerBaseline);
 }
 phase('retrieve-ack-delay-ui-cancel');await beginTrade();
 const seq=sequence;await deposit(item);await poll(()=>packetsAfter(seq,'DepositTradeItem'),p=>p.some(v=>v.payload.success),'SECOND_DEPOSIT_NOT_SUCCESSFUL');
 await poll(()=>rawTrade,t=>t&&String(t.offeredUniqueIds?.['0'])===String(item.uniqueId)&&t.offeredSlots?.['0']===item.slot,'SERVER_OFFER_CUSTODY_NOT_SEEN');
 const offer=page.locator('[data-r16-trade-side="own"] button[data-trade-slot="0"]');await poll(()=>offer.isEnabled(),Boolean,'RETRIEVE_UI_STILL_PENDING');
 await offer.click();const empty=page.locator('[data-r16-trade-bag-picker] button').filter({hasText:/^(Empty bag|空背包|背包空格)/});
 // Empty-bag labels are localization output. Select an enabled physical empty
 // cell by actual DOM state after the visible offered item selection.
 const empties=page.locator('[data-r16-trade-bag-picker] button:not([title]):enabled');assert(await empties.count()>0,'NO_VISIBLE_EMPTY_BAG_DESTINATION');
 arm('RetrieveTradeItem');await empties.first().click();await cancelHeld('retrieve',baseline);assert.deepEqual(econ(peer.snapshot()),peerBaseline);
 proof.assetsAfter={primary:econ(current()),peer:econ(peer.snapshot()),expectedCandleSlotMove:{from:item.slot,to:lastItemCommand.to}};
}
const keyboard=JSON.parse(await readFile(join(repo,'apps/game-client/client-bevy/src/crystal_ui/keyboard_defaults.json'),'utf8'));
async function key(name){const row=keyboard.find(r=>r.function===name);assert(row,'NATIVE_KEY_BINDING_MISSING');const chord=[row.ctrl===1?'Control':null,row.alt===1?'Alt':null,row.shift===1?'Shift':null,row.key].filter(Boolean).join('+');await focus();await page.keyboard.press(chord);note('native-key',{function:name,chord});}
async function coverage(stage){
 const result=await cdp.send('Profiler.takePreciseCoverage');
 const rows=[];const host=[];
 for(const script of result.result){
  for(const f of script.functions){
   if(f.functionName==='processMir2SocialBudget')rows.push({path:script.url.split('?')[0],calls:Math.max(0,...f.ranges.map(r=>r.count))});
   if(script.url.includes('social-operation-budget-host')&&['reserve','validate','enter','receive','snapshot','retire'].includes(f.functionName))host.push({method:f.functionName,calls:Math.max(0,...f.ranges.map(r=>r.count))});
  }
 }
 const value={stage,bridge:rows,bridgeCalls:rows.reduce((n,r)=>n+r.calls,0),host};proof.bridgeCoverage.push(value);return value;
}
async function groupAndGuild(){
 if(process.env.R16_QA_SKIP_VALIDATED_PARTY!=='1'){
 phase('ordinary-dom-shared-party');const a=econ(current()),b=econ(peer.snapshot());
 await peer.setAllowGroup(true);await key('Group');
 const group=page.locator('[data-r16-group-window="true"]');await group.waitFor();
 await group.locator('[data-r16-group-add]').click();await page.locator('[data-r16-group-name]').fill(state.actor.characterName);await page.locator('[data-r16-group-name-submit]').click();
 await poll(()=>peer.snapshot(),s=>s.party.pendingInvite?.name===state.owner.characterName,'REAL_GROUP_INVITE_NOT_RECEIVED');assert.equal(peer.snapshot().party.members.length,0,'INVITE_OPTIMISTICALLY_JOINED_PEER');
 await peer.acceptGroupInvite();
 await poll(current,s=>s.party.members.length===2&&s.party.members.includes(state.actor.characterName),'PRIMARY_SHARED_GROUP_NOT_AUTHORITATIVE');
 await poll(()=>peer.snapshot(),s=>s.party.members.length===2&&s.party.members.includes(state.owner.characterName),'PEER_SHARED_GROUP_NOT_AUTHORITATIVE');
 await poll(()=>group.locator('[data-member-name]').count(),n=>n===2,'GROUP_UI_ROSTER_NOT_TWO');assert.deepEqual(econ(current()),a);assert.deepEqual(econ(peer.snapshot()),b);
 const cov=await coverage('ordinary-dom-shared-party');assert(cov.bridgeCalls>0,'PAGE_NATIVE_BRIDGE_NOT_CALLED_DURING_GROUP');
 check('ordinary-dom-two-player-shared-party',{authoritativeMembers:2,bothClientsAgree:true,assetsUnchanged:true,actualNativeBridgeCalls:cov.bridgeCalls});await shot('02-ordinary-shared-party');
 await group.getByRole('button',{name:/^(关闭|Close)$/}).click();await group.waitFor({state:'hidden'});
 }else proof.groupProofReference='browser-r2/proof.json';
 phase('ordinary-guild-window-no-membership');proof.guildObservation={snapshotHasObject:rawGuild!==null,hasNonemptyGuildName:typeof rawGuild?.name==='string'&&rawGuild.name.trim().length>0};assert(!proof.guildObservation.hasNonemptyGuildName,'UNEXPECTED_REAL_GUILD_MEMBERSHIP');
 const before={...proof.commands.primary};await key('Guilds');
 const guild=page.getByRole('region',{name:/行会|公会|Guild/});await guild.waitFor();
 assert.equal(await guild.locator('input,textarea,select').count(),0,'NO_GUILD_WINDOW_HAS_MUTATION_EDITOR');
 assert.equal(await guild.locator('button').count(),1,'NO_GUILD_WINDOW_HAS_UNEXPECTED_ACTION');
 const forbiddenTypes=['editGuildMember','editGuildNotice','guildStorageGoldChange','guildStorageItemChange','guildInvite'];
 assert(forbiddenTypes.every(t=>(proof.commands.primary[t]??0)===(before[t]??0)),'UNAUTHORIZED_GUILD_MUTATION_SENT');
 check('ordinary-guild-window-membership-gate',{realMembership:false,mutationEditors:0,onlyCloseButton:true,noGuildMutationCommands:true,guildRulesOnlyControllerVerified:true});await shot('03-guild-no-membership');
 await guild.getByRole('button',{name:/^(关闭|Close)$/}).click();await guild.waitFor({state:'hidden'});await coverage('guild-window-readonly');
}
async function logout(){
 phase('normal-logouts');await focus();const seq=sequence;await page.keyboard.press('Alt+KeyX');
 const logoutDialog=page.locator('[role="dialog"]:visible');
 await delay(250);
 // R16 shortcut normally sends directly; if the real confirmation is shown,
 // click its ordinary visible positive button, never a runtime action.
 if(!(packetsAfter(seq,'LogOutSuccess').length)&&await logoutDialog.count()===1){const yes=logoutDialog.getByRole('button',{name:/^(是|Yes|确认|确定)$/});if(await yes.count()===1)await yes.click();}
 await poll(()=>packetsAfter(seq,'LogOutSuccess'),p=>p.length,'PRIMARY_NORMAL_LOGOUT_NOT_ACKNOWLEDGED');
 assert(await peer.logout(),'PEER_NORMAL_LOGOUT_NOT_ACKNOWLEDGED');check('both-normal-logouts-real-ack');
}
try{
 browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',args:['--remote-debugging-port=19484','--remote-debugging-address=127.0.0.1','--enable-unsafe-swiftshader']});
 context=await browser.newContext({locale:'zh-CN',viewport:{width:1280,height:960}});page=await context.newPage();page.setDefaultTimeout(5000);
 page.on('pageerror',e=>proof.pageErrors.push({name:e.name,messageHash:hash(e.message)}));page.on('console',m=>{if(m.type()==='error')proof.consoleErrors.push({messageHash:hash(m.text())});});
 page.on('response',r=>{const u=new URL(r.url());if(r.status()>=400)proof.httpErrors.push({path:u.pathname,status:r.status()});if(u.pathname.endsWith('/mir2_bevy_runtime.js'))moduleReads.push((async()=>{const bytes=await r.body(),digest=hash(bytes);proof.moduleResponses.push({path:u.pathname,status:r.status(),bytes:bytes.length,sha256:digest,matchesFrozenRuntime:manifest.files.some(f=>f.path.endsWith('.js')&&f.sha256===digest)});})().catch(()=>{}));});
 await page.routeWebSocket(config.gatewayUrl,ws=>{
  clientRoute=ws;const real=ws.connectToServer();
  ws.onMessage(raw=>{try{const v=parseGatewayJson(typeof raw==='string'?raw:raw.toString());command('primary',v);
   if(['depositTradeItem','retrieveTradeItem'].includes(v.type)){lastItemCommand={type:v.type,from:v.from,to:v.to};if(hold&&hold.packet===(v.type==='depositTradeItem'?'DepositTradeItem':'RetrieveTradeItem'))hold.expected={from:v.from,to:v.to};}
   real.send(raw);}catch(e){fatal='CLIENT_WIRE_OR_POLICY_FAILED';}});
  real.onMessage(raw=>{try{
   const event=parseGatewayJson(typeof raw==='string'?raw:raw.toString());observe(event,false);
   if(hold&&hold.expected&&event.type==='packet'&&event.packet===hold.packet&&event.payload.from===hold.expected.from&&event.payload.to===hold.expected.to&&!hold.raw){hold.raw=raw;hold.event=event;hold.seq=sequence;hold.receivedAt=Date.now();note('real-ack-withheld',{packet:event.packet,from:event.payload.from,to:event.payload.to,success:event.payload.success,rawSha256:hash(raw)});return;}
   observe(event,true);ws.send(raw);
  }catch(e){fatal='SERVER_WIRE_OBSERVATION_FAILED';}});
 });
 phase('local-cold-navigation');await page.goto(config.origin,{waitUntil:'commit',timeout:15000});
 if(!reuse)await registration();else{await page.locator('.login-input.account').waitFor({state:'visible',timeout:90000});await poll(()=>page.locator('.login-input.account').isEnabled(),Boolean,'LOGIN_NOT_READY',90000);}
 await loginAndCreate();cdp=await context.newCDPSession(page);await cdp.send('Profiler.enable');await cdp.send('Profiler.startPreciseCoverage',{callCount:true,detailed:true});coverageStarted=true;
 await peerStart();await coverage('bootstrap');await groupAndGuild();await align();await scenarios();
 const tradeCov=await coverage('ordinary-trade-pending-cancels');assert(tradeCov.bridgeCalls>0,'PAGE_NATIVE_BRIDGE_NOT_CALLED_DURING_TRADE');
 check('page-native-bridge-real-trade-coverage',{actualCalls:tradeCov.bridgeCalls,gameActionsInjected:false,bridgeWrapperInstalled:false});
 await logout();await coverage('normal-logout');await cdp.send('Profiler.stopPreciseCoverage');coverageStarted=false;
 await Promise.all(moduleReads);assert(proof.moduleResponses.some(r=>r.matchesFrozenRuntime),'ACTUAL_LOADED_JS_NOT_FROZEN_RUNTIME');
 proof.passed=true;
}catch(e){await writeFile(join(root,'private',leaf+'-failure.local.txt'),String(e.stack??e.message),{flag:'wx',mode:0o600});const firstLine=e.message.split(/\r?\n/)[0];proof.errors.push({phase:proof.phase,name:e.name,code:e.code??null,messageHash:hash(e.message),safeCode:/^[A-Z0-9_ ]+$/.test(firstLine)?firstLine:null});try{proof.failureUi=await snapshot();await shot('failure');}catch{}console.log(JSON.stringify({passed:false,phase:proof.phase,name:e.name,messageHash:hash(e.message)}));}
finally{
 if(hold?.raw){release();}
 if(coverageStarted){try{await coverage('failure');await cdp.send('Profiler.stopPreciseCoverage');}catch{}}
 proof.finalPrimary=current().ready?{economy:econ(current()),tradeActive:current().trade.active,position:{x:current().player.x,y:current().player.y}}:null;
 await writeFile(join(run,'proof.json'),JSON.stringify(proof,null,2));
 console.log(JSON.stringify({proof:join(run,'proof.json'),passed:proof.passed,checks:proof.checks.length,phase:proof.phase,headlessStillOpen:!proof.passed}));
 if(!proof.passed){console.log('Owned headless instance retained; send close to release.');const input=readline.createInterface({input:process.stdin,terminal:false});await new Promise(resolve=>{input.on('line',v=>{if(v.trim()==='close'){input.close();resolve();}});input.on('close',resolve);});}
 await peer?.close().catch(()=>{});await browser?.close();
 await writeFile(join(run,'browser-closed.json'),JSON.stringify({closed:true,at:new Date().toISOString(),normalLogouts:proof.checks.some(c=>c.name==='both-normal-logouts-real-ack')}));
 process.exitCode=proof.passed?0:1;
}
