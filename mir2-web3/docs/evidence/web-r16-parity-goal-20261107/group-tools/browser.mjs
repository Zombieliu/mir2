import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import {setTimeout as sleep} from 'node:timers/promises';
const base=import.meta.dirname,repo='E:/mir2-player-journey/mir2-web3',label=process.argv[2]??'initial';assert(/^[a-z0-9-]+$/.test(label));
const config=JSON.parse(await readFile(base+'/private-qa/browser-config.local.json','utf8'));
const identity=JSON.parse(await readFile(base+'/private-qa/identity.local.json','utf8'));
const peerFile=base+'/private-qa/peers.local.json',peerIdentities=JSON.parse(await readFile(peerFile,'utf8'));
const output=base+'/browser-'+label;await mkdir(output);
const {GatewayClient}=await import(pathToFileURL(repo+'/apps/ai-player/src/gateway-client.mjs'));
const {ObservationReducer}=await import(pathToFileURL(repo+'/apps/ai-player/src/observation.mjs'));
const {parseGatewayJson}=await import(pathToFileURL(repo+'/apps/ai-player/src/lossless-json.mjs'));
const require=createRequire(import.meta.url),{chromium}=require('F:/mir2-builds/web-r16-20261104/tools/node_modules/playwright');
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const bindings=JSON.parse(await readFile(repo+'/apps/game-client/client-bevy/src/crystal_ui/keyboard_defaults.json','utf8'));
const chord=fn=>{const b=bindings.find(row=>row.function===fn);assert(b);return [...['Control','Alt','Shift'].filter(key=>b[{Control:'ctrl',Alt:'alt',Shift:'shift'}[key]]===1),/^[A-Z]$/.test(b.key)?'Key'+b.key:b.key].join('+');};
const safeName=name=>name===identity.name?'owner':name===peerIdentities[0].characterName?'peerA':name===peerIdentities[1].characterName?'peerB':'other';
const proof={schema:'mir2.r16.group.browser.v1',passed:false,requestedBackend:process.argv.includes('--webgpu')?'webgpu':'webgl2-shared',startedAt:new Date().toISOString(),phases:[],commands:[],packets:[],checks:[],screenshots:[],pageErrors:[],networkErrors:[],errors:[]};
const observation=new ObservationReducer(),peers=[],relays=[];let browser,page,phase='setup',activeSocket,observationFailure;
const phaseTo=value=>{phase=value;proof.phases.push(value);console.log(JSON.stringify({phase:value}));};
async function poll(read,accept,code,ms=30000){const until=Date.now()+ms;while(Date.now()<until){if(observationFailure)throw observationFailure;const value=await read();if(accept(value))return value;await sleep(70);}throw new Error(code);}
const commands=type=>proof.commands.filter(row=>row.type===type),state=()=>observation.snapshot();
const ui=()=>page.evaluate(()=>{const r=window.__mir2BevyRuntime,read=key=>{try{return JSON.parse(r?.[key]?.()??'null');}catch{return null;}};return {bag:read('getMir2BagUiStatus'),hud:read('getMir2HudUiStatus'),quest:read('getMir2QuestUiStatus')};});
const stage=()=>page.locator('.client-stage-frame');
const key=async fn=>{await stage().focus();await page.keyboard.press(chord(fn),{delay:80});};
const selected=()=>page.evaluate(()=>window.__mir2Stage5?.state?.selectedObjectId??null);
const allowButton=()=>page.locator('[data-r16-group-window] button[aria-label][aria-pressed]');
async function pointFor(id){return poll(()=>page.evaluate(objectId=>{
  const nodes=[...document.querySelectorAll('[data-object-id]')].filter(node=>node.getAttribute('data-object-id')===objectId&&node.matches('.entity-nameplate,.entity-sprite-hit'));
  for(const node of nodes){const r=node.getBoundingClientRect();if(r.width<1||r.height<1||getComputedStyle(node).visibility!=='visible')continue;
    for(const dy of [0.5,0.25,0.75])for(const dx of [0.5,0.25,0.75]){const x=r.left+r.width*dx,y=r.top+r.height*dy;
      const layers=document.elementsFromPoint(x,y),top=layers[0];
      if(top?.id?.includes('canvas')&&layers.some(layer=>layer.closest('[data-object-id]')?.getAttribute('data-object-id')===objectId))return{x,y,topSharedCanvas:true};
    }
  }return null;
},String(id)),Boolean,'NO_PHYSICAL_WORLD_MARKER',15000);}
async function shot(name){const file=output+'/'+name+'.png';await page.screenshot({path:file,mask:[page.locator('.login-input:visible,[data-login-auth-field]:visible,.select-identity-id:visible')]});proof.screenshots.push({name,sha256:hash(await readFile(file))});}
async function setOwnerAllow(allow){const before=commands('switchGroup').length;const current=await allowButton().getAttribute('aria-pressed');if((current==='true')!==allow)await allowButton().click();await poll(()=>allowButton().getAttribute('aria-pressed'),value=>value===String(allow),'ALLOW_UI_NOT_AUTHORITATIVE');return commands('switchGroup').length-before;}
async function nameOperation(remove,name){await page.locator(remove?'[data-r16-group-remove]':'[data-r16-group-add]').click();await page.locator('[data-r16-group-name]').fill(name);await page.locator('[data-r16-group-name-submit]').click();await page.locator('[data-r16-group-name]').waitFor({state:'hidden'});}
try{
  phaseTo('ordinary-peer-registration');
  for(let index=0;index<peerIdentities.length;index++){
    const identity=peerIdentities[index],client=new GatewayClient({url:config.gatewayUrl,realmId:config.realmId,timeoutMs:20000});peers.push(client);await client.connect();
    client.on('frame',({event})=>{if(['GroupInvite','GroupMemberInfo','DeleteGroup','SwitchGroup'].includes(event.packet))proof.packets.push({source:'peer'+index,phase,packet:event.packet,...(typeof event.payload?.allowGroup==='boolean'?{allowGroup:event.payload.allowGroup}:{})});});
    if(identity.characterIndex===null){const store=JSON.parse(await readFile(base+'/private-data/accounts.json','utf8'));if(!Object.hasOwn(store.accounts,identity.accountId))await client.createAccount(identity.accountId,identity.password);await client.login(identity.accountId,identity.password);const role=await client.createCharacter(identity.characterName);identity.characterIndex=role.index;await writeFile(peerFile,JSON.stringify(peerIdentities),{mode:0o600});}
    else await client.login(identity.accountId,identity.password);
    await client.startGame(identity.characterIndex);
    identity.testTargetX??=client.snapshot().player.x+(index===0?-3:3);await writeFile(peerFile,JSON.stringify(peerIdentities),{mode:0o600});
    for(let step=0;step<6&&client.snapshot().player.x!==identity.testTargetX;step++){const result=await client.walk(client.snapshot().player.x<identity.testTargetX?'Right':'Left');await sleep(300);if(!result.moved)break;}
    await client.setAllowGroup(index===1);
  }
  phaseTo('ordinary-browser-login');
  browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,args:['--no-sandbox','--disable-dev-shm-usage']});proof.browser=browser.version();
  const context=await browser.newContext({locale:'zh-CN',viewport:{width:1280,height:960}});page=await context.newPage();page.setDefaultTimeout(20000);
  if(process.argv.includes('--reconnect'))await page.routeWebSocket(config.gatewayUrl,ws=>{relays.push({client:ws,server:ws.connectToServer()});});
  page.on('pageerror',error=>proof.pageErrors.push(hash(error.message)));
  page.on('response',response=>{const url=new URL(response.url());if(response.status()>=400&&url.origin===config.origin)proof.networkErrors.push({status:response.status(),pathHash:hash(url.pathname)});});
  page.on('websocket',socket=>{if(socket.url()!==config.gatewayUrl)return;activeSocket=socket;observation.resetConnection();
    socket.on('framesent',frame=>{if(activeSocket!==socket)return;try{const raw=parseGatewayJson(String(frame.payload));proof.commands.push({phase,type:raw.type,...(['addMember','delMember'].includes(raw.type)?{subject:safeName(raw.name)}:{}),...(raw.type==='switchGroup'?{allowGroup:raw.allowGroup}:{})});}catch{observationFailure=new Error('TX_OBSERVATION_FAILED');}});
    socket.on('framereceived',frame=>{if(activeSocket!==socket)return;try{const event=parseGatewayJson(String(frame.payload));if(event.type==='companionResult'||event.type==='identitySession')return;observation.apply(event);
      if(event.type==='realmInfo')assert.equal(event.payload.realmId,config.realmId);
      if(event.type==='packet')proof.packets.push({source:'browser',phase,packet:event.packet,...(typeof event.payload?.allowGroup==='boolean'?{allowGroup:event.payload.allowGroup}:{}),...(['AddMember','DeleteMember','GroupInvite'].includes(event.packet)?{subject:safeName(event.payload.name)}:{})});
    }catch{observationFailure=new Error('RX_OBSERVATION_FAILED');}});
  });
  const url=new URL(config.origin);url.searchParams.set('gatewayWs',config.gatewayUrl);if(!process.argv.includes('--webgpu')){url.searchParams.set('bevyBackend','webgl2');url.searchParams.set('bevySharedCanvas','1');}url.searchParams.set('bevyQuestUi','1');url.searchParams.set('bevyBagUi','1');
  await page.goto(url.href,{waitUntil:'domcontentloaded',timeout:90000});
  await poll(()=>page.locator('.login-input.account').isEnabled(),Boolean,'LOGIN_NOT_READY',60000);
  await page.locator('.login-input.account').fill(identity.accountId);await page.locator('.login-input.password').fill(identity.password);await page.locator('.login-button.ok button').click();
  await page.locator('.select-overlay').waitFor({state:'visible',timeout:45000});
  await page.locator('.select-character-slot-card:not(.empty)').filter({has:page.locator('.name').filter({hasText:new RegExp('^'+identity.name+'$')})}).click();
  await page.locator('.select-action.start button').click();
  await poll(()=>page.evaluate(()=>{const frame=document.querySelector('.client-stage-frame');return document.querySelector('main')?.dataset.clientScreen==='game'&&frame&&!frame.classList.contains('scene-assets-pending');}),Boolean,'WORLD_NOT_READY',60000);
  await poll(ui,value=>value.hud?.ready&&value.quest?.ready,'SHARED_UI_NOT_READY',60000);
  const initialPosition={x:state().player.x,y:state().player.y};proof.initialPosition=initialPosition;
  await poll(()=>state().entities,value=>peerIdentities.every(identity=>value.some(row=>row.name===identity.characterName)),'PEERS_NOT_IN_REAL_AOI');
  phaseTo('target-refuses-sender-remains-off');
  await key('Group');await page.locator('[data-r16-group-window]').waitFor({state:'visible'});await setOwnerAllow(false);
  const switchBefore=commands('switchGroup').length,addBefore=commands('addMember').length;
  await nameOperation(false,peerIdentities[0].characterName);
  await poll(()=>commands('addMember').length,count=>count===addBefore+1,'REFUSED_INVITE_NOT_SENT');await sleep(1000);
  assert.equal(commands('switchGroup').length,switchBefore);assert.equal(await allowButton().getAttribute('aria-pressed'),'false');assert.equal(peers[0].snapshot().party.pendingInvite,null);
  proof.checks.push({name:'refusedTargetDoesNotEnableSender',passed:true});await shot('refused-invite');
  if(process.argv.includes('--reconnect')){
    phaseTo('physical-connection-close-and-confirmed-session-reset');const starts=commands('startGame').length;assert.equal(relays.length,1);
    await relays[0].server.close({code:3001,reason:'bounded test connection close'});
    await relays[0].client.close({code:3001,reason:'bounded test connection close'});
    await poll(()=>commands('startGame').length,count=>count===starts+1,'AUTOMATIC_SESSION_REENTRY_NOT_SENT',60000);
    await poll(()=>state().ready,Boolean,'AUTOMATIC_SESSION_REENTRY_NOT_READY',60000);await poll(ui,value=>value.hud?.ready,'RECONNECTED_HUD_NOT_READY',60000);
    if(!await page.locator('[data-r16-group-window]').isVisible())await key('Group');await page.locator('[data-r16-group-window]').waitFor({state:'visible'});
    const repeated=commands('addMember').length;await nameOperation(false,peerIdentities[0].characterName);
    await poll(()=>commands('addMember').length,count=>count===repeated+1,'OLD_PENDING_BLOCKED_NEW_CONFIRMED_SESSION');
    await sleep(300);assert.equal(commands('switchGroup').length,switchBefore);assert.equal(await allowButton().getAttribute('aria-pressed'),'false');
    proof.checks.push({name:'realConnectionCloseNewStartGameAllowsExplicitSameNameWithoutReplay',passed:true,connections:relays.length,injectedCloseCode:3001,realServerConnected:true,noSyntheticGamePackets:true});
  }
  await key('Group');await page.locator('[data-r16-group-window]').waitFor({state:'hidden'});
  phaseTo('fresh-hover-and-blocked-overlays');
  const a=state().entities.find(row=>row.name===peerIdentities[0].characterName),b=state().entities.find(row=>row.name===peerIdentities[1].characterName);assert(a&&b);
  const aPoint=await pointFor(a.objectId);await page.mouse.click(aPoint.x,aPoint.y,{delay:80});await poll(selected,value=>value===String(a.objectId),'SELECTION_A_NOT_ESTABLISHED');
  const bPoint=await pointFor(b.objectId);await page.mouse.move(bPoint.x,bPoint.y);
  await key('Inventory');const bag=await poll(ui,value=>value.bag?.ready&&value.bag.inputEnabled&&value.bag.inputRegions.length>0,'BAG_NOT_OPEN');
  const rect=bag.bag.inputRegions[0],geom=await stage().evaluate(node=>{const r=node.getBoundingClientRect();return{x:r.x,y:r.y,width:r.width,height:r.height,logicalWidth:Number(node.dataset.viewportSceneWidth),logicalHeight:Number(node.dataset.viewportSceneHeight)};});
  await page.mouse.move(geom.x+(rect.left+rect.width/2)*geom.width/geom.logicalWidth,geom.y+(rect.top+Math.min(20,rect.height/2))*geom.height/geom.logicalHeight);
  const blockedBefore=commands('addMember').length;await key('AddGroupMember');await sleep(350);assert.equal(commands('addMember').length,blockedBefore);proof.checks.push({name:'sharedBagBlocksHoveredWorld',passed:true});await shot('bag-blocked');
  await key('Inventory');await poll(ui,value=>value.bag?.inputRegions.length===0||!value.bag?.inputEnabled,'BAG_NOT_CLOSED');
  await key('Group');await page.locator('[data-r16-group-add]').click();await page.locator('[data-r16-group-name]').fill(peerIdentities[1].characterName);await page.keyboard.press(chord('AddGroupMember'),{delay:80});await sleep(250);assert.equal(commands('addMember').length,blockedBefore);await page.keyboard.press('Escape');await key('Group');
  await sleep(2200);await page.mouse.move(bPoint.x,bPoint.y);await stage().focus();const inviteAfter=peers[1].sequence;
  await page.keyboard.press(chord('AddGroupMember'),{delay:80});
  await poll(()=>commands('addMember').length,count=>count===blockedBefore+1,'FRESH_HOVER_INVITE_NOT_SENT');assert.equal(commands('addMember').at(-1).subject,'peerB');assert.equal(await selected(),String(a.objectId));assert.equal(commands('switchGroup').length,switchBefore);
  await peers[1].waitFor(event=>event.packet==='GroupInvite',{after:inviteAfter});
  await page.keyboard.down('Control');await page.keyboard.down('KeyG');await page.keyboard.down('KeyG');await page.keyboard.up('KeyG');await page.keyboard.up('Control');await sleep(250);assert.equal(commands('addMember').length,blockedBefore+1);
  proof.checks.push({name:'hoverBOverridesSelectionAWithoutSwitchOrDuplicate',passed:true,topSharedCanvas:bPoint.topSharedCanvas});
  phaseTo('authoritative-join-and-removal');await peers[1].acceptGroupInvite();
  await poll(()=>state().party.members,value=>value.includes(identity.name)&&value.includes(peerIdentities[1].characterName),'PARTY_NOT_AUTHORITATIVE');
  await key('Group');await poll(()=>page.locator('[data-r16-group-window] [data-member-name]').count(),count=>count===2,'PARTY_UI_NOT_UPDATED');assert.equal(await allowButton().getAttribute('aria-pressed'),'true');await shot('party-established');
  await sleep(2200);await nameOperation(true,peerIdentities[1].characterName);
  await poll(()=>state().party.members,value=>value.length===0,'REMOVAL_NOT_AUTHORITATIVE');assert.equal(commands('delMember').length,1);assert.equal(commands('delMember')[0].subject,'peerB');proof.checks.push({name:'realGroupRosterAndRemoval',passed:true});
  await key('Group');phaseTo('normal-logout-and-reentry');await key('Logout');await page.locator('.select-overlay').waitFor({state:'visible',timeout:30000});
  await page.locator('.select-action.start button').click();await poll(()=>state().ready,Boolean,'REENTRY_NOT_READY');await poll(ui,value=>value.hud?.ready,'REENTRY_HUD_NOT_READY');
  assert.deepEqual({x:state().player.x,y:state().player.y},initialPosition);await key('Logout');await page.locator('.select-overlay').waitFor({state:'visible'});
  assert.equal(commands('logOut').length,2);assert.equal(proof.commands.filter(row=>['walk','run','turn','attack','rangeAttack','magic','useItem','pickUp'].includes(row.type)).length,0);
  assert.deepEqual(proof.pageErrors,[]);assert.deepEqual(proof.networkErrors,[]);proof.checks.push({name:'twoNormalLogoutsPreserveTransformAndAssetsNoGameplayMutation',passed:true});proof.passed=true;phaseTo('complete');
}catch(error){await writeFile(output+'/error.private.json',JSON.stringify({message:error.message,stack:error.stack}),{mode:0o600});proof.errors.push({phase,code:error.code??null,messageHash:hash(error.message)});console.log(JSON.stringify({failed:true,phase,messageHash:hash(error.message)}));process.exitCode=1;}
finally{
  if(page&&!page.isClosed()){try{if(!proof.passed)await shot('failure');}catch{}await writeFile(output+'/diagnostics.private.json',JSON.stringify({selected:await selected().catch(()=>null),ui:await ui().catch(()=>null),state:state()}),{mode:0o600});}
  for(const client of peers){try{if(client.snapshot().ready)await client.logout();}catch{}await client.close().catch(()=>{});}
  await browser?.close().catch(()=>{});proof.completedAt=new Date().toISOString();await writeFile(output+'/proof.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({passed:proof.passed,checks:proof.checks.length,commands:proof.commands.length,errors:proof.errors.length}));
}
