import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {pathToFileURL} from 'node:url';
import {setTimeout as sleep} from 'node:timers/promises';
const base=import.meta.dirname,repo='E:/mir2-player-journey/mir2-web3';
const label=process.argv[2]??'initial';assert(/^[a-z0-9-]+$/.test(label));
const config=JSON.parse(await readFile(base+'/private-qa/browser-config.local.json','utf8'));
const identity=JSON.parse(await readFile(base+'/private-qa/identity.local.json','utf8'));
const peers=JSON.parse(await readFile(base+'/private-qa/peers.local.json','utf8'));
const output=base+'/browser-'+label;await mkdir(output);
const {ObservationReducer}=await import(pathToFileURL(repo+'/apps/ai-player/src/observation.mjs'));
const {parseGatewayJson}=await import(pathToFileURL(repo+'/apps/ai-player/src/lossless-json.mjs'));
const require=createRequire(import.meta.url),{chromium}=require('F:/mir2-builds/web-r16-20261104/tools/node_modules/playwright');
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const bindings=JSON.parse(await readFile(repo+'/apps/game-client/client-bevy/src/crystal_ui/keyboard_defaults.json','utf8'));
const chord=fn=>{const b=bindings.find(row=>row.function===fn);assert(b);return [...['Control','Alt','Shift'].filter(key=>b[{Control:'ctrl',Alt:'alt',Shift:'shift'}[key]]===1),/^[A-Z]$/.test(b.key)?'Key'+b.key:b.key].join('+');};
const backend=process.argv.includes('--webgpu')?'webgpu':'webgl2-shared';
const suffix=backend==='webgpu'?'GPU':'GL';
const proof={schema:'mir2.r16.guild-ranks.browser.v1',passed:false,backend,startedAt:new Date().toISOString(),phases:[],commands:[],packets:[],checks:[],screenshots:[],pageErrors:[],consoleErrors:[],consoleWarnings:[],networkErrors:[],connections:[],protocol:[],errors:[],fixtureGuildCreatedBeforeStartup:true,guildCreationViaGameplayClaimed:false};
const json=value=>JSON.stringify(value,(_key,item)=>typeof item==='bigint'?item.toString():item);
let browser,phase='setup',observationFailure;const sessions=[];
const phaseTo=value=>{phase=value;proof.phases.push(value);console.log(JSON.stringify({phase:value}));};
async function poll(read,accept,code,ms=30000){const until=Date.now()+ms;while(Date.now()<until){if(observationFailure)throw observationFailure;const value=await read();if(accept(value))return value;await sleep(70);}throw new Error(code);}
const commands=(session,type)=>proof.commands.filter(row=>row.actor===session.actor&&row.type===type);
const rankPackets=(session,after=0)=>session.guildPackets.slice(after).filter(e=>e.packet==='GuildMemberChange');
const permissionKeys=['CanChangeRank','CanRecruit','CanKick','CanStoreItem','CanRetrieveItem','CanAlterAlliance','CanChangeNotice','CanActivateBuff'];
const guild=session=>session.page.locator('[data-guild-name="RankQA"]');
const rank=(session,index)=>guild(session).locator('[data-rank-index="'+index+'"]');
const rankName=(session,index)=>rank(session,index).locator('input:not([type="checkbox"])');
const checkbox=(session,index,key)=>rank(session,index).locator('[data-guild-rank-permission="'+key+'"]');
const stage=session=>session.page.locator('.client-stage-frame');
const key=async(session,fn)=>{await stage(session).focus();await session.page.keyboard.press(chord(fn),{delay:80});};
async function ui(session){return session.page.evaluate(()=>{const r=window.__mir2BevyRuntime,read=key=>{try{return JSON.parse(r?.[key]?.()??'null');}catch{return null;}};return{hud:read('getMir2HudUiStatus'),quest:read('getMir2QuestUiStatus')};});}
async function shot(session,name){const file=output+'/'+name+'.png';await session.page.screenshot({path:file,mask:[session.page.locator('.login-input:visible,[data-login-auth-field]:visible,.select-identity-id:visible')]});proof.screenshots.push({name,sha256:hash(await readFile(file))});}
async function ready(session){
 await poll(()=>session.page.evaluate(()=>{const frame=document.querySelector('.client-stage-frame');return document.querySelector('main')?.dataset.clientScreen==='game'&&frame&&!frame.classList.contains('scene-assets-pending');}),Boolean,'WORLD_NOT_READY',60000);
 await poll(()=>ui(session),value=>value.hud?.ready&&value.quest?.ready,'SHARED_UI_NOT_READY',60000);
 await poll(()=>session.observation.snapshot().ready,Boolean,'OBSERVATION_NOT_READY');
}
async function enter(session){await session.page.locator('.select-action.start button').click();await ready(session);}
async function login(actor,role){
 const context=await browser.newContext({locale:'en-US',viewport:{width:1280,height:960}}),page=await context.newPage();page.setDefaultTimeout(20000);
 const session={actor,role,context,page,observation:new ObservationReducer(),guildPackets:[],activeSocket:null,console:[],rawEvents:[]};sessions.push(session);
 page.on('console',message=>{if(['error','warning'].includes(message.type())){session.console.push({type:message.type(),phase,text:message.text()});proof[message.type()==='error'?'consoleErrors':'consoleWarnings'].push({actor,phase,messageHash:hash(message.text())});}});
 page.on('crash',()=>proof.pageErrors.push({actor,phase,kind:'crash'}));
 page.on('pageerror',error=>proof.pageErrors.push({actor,messageHash:hash(error.message)}));
 page.on('response',response=>{const url=new URL(response.url());if(response.status()>=400&&url.origin===config.origin)proof.networkErrors.push({actor,status:response.status(),pathHash:hash(url.pathname)});});
 page.on('websocket',socket=>{if(socket.url()!==config.gatewayUrl)return;session.activeSocket=socket;session.observation.resetConnection();
  proof.connections.push({actor,phase,kind:'open',at:new Date().toISOString()});socket.on('close',()=>proof.connections.push({actor,phase,kind:'close',at:new Date().toISOString()}));
  socket.on('framesent',frame=>{if(session.activeSocket!==socket)return;try{
   const raw=parseGatewayJson(String(frame.payload)),row={actor,phase,type:raw.type};
   if(raw.type==='editGuildMember')Object.assign(row,{changeType:raw.changeType,rankIndex:raw.rankIndex,name:raw.name,rankName:raw.rankName});
   if(raw.type==='requestGuildInfo')row.infoType=raw.infoType;
   proof.commands.push({...row,at:new Date().toISOString()});
  }catch{observationFailure=new Error('TX_OBSERVATION_FAILED');}});
  socket.on('framereceived',frame=>{if(session.activeSocket!==socket)return;try{
   const event=parseGatewayJson(String(frame.payload));session.rawEvents.push(event);if(event.type==='companionResult'||event.type==='identitySession')return;
   proof.protocol.push({actor,phase,type:event.type,packet:event.packet,at:new Date().toISOString(),...(event.packet==='StartGame'?{result:event.payload.result}:{})});
   session.observation.apply(event);if(event.type==='realmInfo')assert.equal(event.payload.realmId,config.realmId);
   if(event.type==='packet'&&['GuildStatus','GuildMemberChange'].includes(event.packet)){
    session.guildPackets.push(event);const payload=event.payload;
    proof.packets.push({actor,phase,packet:event.packet,status:payload.status,rankIndex:payload.rankIndex,actorIsSelf:payload.name===role.name,
     ...(event.packet==='GuildStatus'?{myRankId:payload.myRankId,myOptions:payload.myOptions,guildName:payload.guildName}:{}),
     ...(Array.isArray(payload.ranks)?{ranks:payload.ranks.map(r=>({index:r.index,name:r.name,options:r.options,members:Array.isArray(r.members)?r.members.length:null}))}:{})});
   }
  }catch{observationFailure=new Error('RX_OBSERVATION_FAILED');}});
 });
 const url=new URL(config.origin);url.searchParams.set('gatewayWs',config.gatewayUrl);url.searchParams.set('bevyQuestUi','1');url.searchParams.set('bevyBagUi','1');
 if(backend!=='webgpu'){url.searchParams.set('bevyBackend','webgl2');url.searchParams.set('bevySharedCanvas','1');}
 await page.goto(url.href,{waitUntil:'domcontentloaded',timeout:90000});
 await poll(()=>page.locator('.login-input.account').isEnabled(),Boolean,'LOGIN_NOT_READY',60000);
 await page.locator('.login-input.account').fill(role.accountId);await page.locator('.login-input.password').fill(role.password);await page.locator('.login-button.ok button').click();
 await page.locator('.select-overlay').waitFor({state:'visible',timeout:45000});
 const cards=page.locator('.select-character-slot-card:not(.empty)');
 const candidate=cards.filter({has:page.locator('.name').filter({hasText:new RegExp('^'+role.name+'$')})});await candidate.click();
 await enter(session);session.initialPosition={x:session.observation.snapshot().player.x,y:session.observation.snapshot().player.y};
 return session;
}
async function ranks(session){
 if(!await guild(session).isVisible())await key(session,'Guilds');await guild(session).waitFor({state:'visible'});
 await guild(session).locator('[role="tab"][data-guild-tab="ranks"]').click();
 await poll(()=>guild(session).locator('[data-rank-index]').count(),count=>count===3,'RANK_TABLE_NOT_COMPLETE');
 await poll(()=>session.guildPackets.some(e=>e.packet==='GuildMemberChange'&&e.payload.status===255),Boolean,'NO_REAL_FULL_ROSTER');
}
async function tableIntact(session){assert.equal(await guild(session).locator('[data-rank-index]').count(),3);for(let i=0;i<3;i++)assert.equal(await rankName(session,i).count(),1);}
async function renameRank(session,index,name){
 await poll(()=>rankName(session,index).isEnabled(),Boolean,'RANK_NAME_NOT_EDITABLE');
 await rankName(session,index).fill(name);const before=commands(session,'editGuildMember').length,after=session.guildPackets.length;
 await rank(session,index).locator('button').click();
 const event=await poll(()=>rankPackets(session,after).find(e=>e.payload.status===7&&e.payload.ranks?.[0]?.index===index&&e.payload.ranks[0].name===name),Boolean,'REAL_RENAME_ACK_MISSING');
 const sent=commands(session,'editGuildMember').slice(before);assert.deepEqual(sent.map(({type,changeType,rankIndex,name,rankName})=>({type,changeType,rankIndex,name,rankName})),[{type:'editGuildMember',changeType:3,rankIndex:index,name:'',rankName:name}]);
 await tableIntact(session);assert.equal(await rankName(session,index).inputValue(),name);
 proof.checks.push({name:'singleRenameKeepsOtherRanks',actor:session.actor,rankIndex:index,passed:true,topLevelRankIndex:event.payload.rankIndex,nestedRankIndex:event.payload.ranks[0].index});
 return event;
}
async function toggle(session,index,permission,draft){
 const input=checkbox(session,index,permission);await poll(()=>input.isEnabled(),Boolean,'PERMISSION_NOT_EDITABLE');
 const value=!await input.isChecked(),before=commands(session,'editGuildMember').length,after=session.guildPackets.length;
 await input.click();
 const option=permissionKeys.indexOf(permission);assert(option>=0);
 const event=await poll(()=>rankPackets(session,after).find(e=>e.payload.status===7&&e.payload.ranks?.[0]?.index===index&&Boolean(e.payload.ranks[0].options&(1<<option))===value),Boolean,'REAL_OPTION_ACK_MISSING');
 await poll(()=>input.isChecked(),checked=>checked===value,'CHECKBOX_NOT_AUTHORITATIVE');
 const sent=commands(session,'editGuildMember').slice(before);assert.deepEqual(sent.map(({type,changeType,rankIndex,name,rankName})=>({type,changeType,rankIndex,name,rankName})),[{type:'editGuildMember',changeType:5,rankIndex:index,name:String(value),rankName:String(option)}]);
 if(draft!==undefined)assert.equal(await rankName(session,index).inputValue(),draft);
 await tableIntact(session);proof.checks.push({name:'singleAuthoritativePermission'+(draft!==undefined?'PreservesDraft':''),actor:session.actor,permission,enabled:value,passed:true,rankIndex:index});return event;
}
async function logout(session){
 const player=session.observation.snapshot().player;assert(player,'Actual player transform must exist before ordinary logout');
 assert.deepEqual({x:player.x,y:player.y},session.initialPosition);
 await key(session,'Logout');await session.page.locator('.select-overlay').waitFor({state:'visible',timeout:30000});
}
try{
 phaseTo('ordinary-owner-login');browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,args:['--no-sandbox','--disable-dev-shm-usage']});proof.browser=browser.version();
 const owner=await login('owner',identity);await ranks(owner);await shot(owner,'owner-initial-ranks');
 phaseTo('single-rename-and-real-rank-deltas');const memberName='Trail'+suffix;await renameRank(owner,2,memberName);await toggle(owner,2,'CanStoreItem');
 const draft='Unsaved'+suffix;await rankName(owner,2).fill(draft);await toggle(owner,2,'CanRecruit',draft);await shot(owner,'owner-permissions-draft-retained');
 await rankName(owner,2).fill('ab');assert.equal(await rank(owner,2).locator('button').isDisabled(),true);await rankName(owner,2).fill(memberName);
 proof.checks.push({name:'NativeShortNameRejectedByOrdinaryUI',passed:true});
 phaseTo('members-refresh-and-normal-reentry');const readBefore=commands(owner,'requestGuildInfo').length;
 await guild(owner).locator('[role="tab"][data-guild-tab="members"]').click();
 await poll(()=>commands(owner,'requestGuildInfo').length,count=>count===readBefore+1,'MEMBERS_READ_NOT_SENT');
 await shot(owner,'owner-members-after-deltas');await key(owner,'Guilds');await logout(owner);await enter(owner);await ranks(owner);
 assert.equal(await rankName(owner,2).inputValue(),memberName);await tableIntact(owner);proof.checks.push({name:'NormalLogoutReentryKeepsActualRenamedRank',passed:true});
 phaseTo('independent-officer-hierarchy');const p=peers[0],officer=await login('officer',{...p,name:p.characterName});await ranks(officer);
 assert.equal(await rankName(officer,0).isDisabled(),true);assert.equal(await rank(officer,0).locator('button').isDisabled(),true);
 for(const permission of permissionKeys)assert.equal(await checkbox(officer,0,permission).isDisabled(),true);
 assert.equal(await rankName(officer,1).isEnabled(),true);assert.equal(await rankName(officer,2).isEnabled(),true);
 proof.checks.push({name:'OrdinaryOfficerCannotEditHigherRankCanEditOwnAndLower',passed:true});
 const ownerEdits=commands(owner,'editGuildMember').length,ownerPacketStart=owner.guildPackets.length,officerName='Officer'+suffix;
 await renameRank(officer,1,officerName);const lastOption=await toggle(officer,2,'CanRetrieveItem');
 await poll(()=>rankName(owner,1).inputValue(),value=>value===officerName,'OTHER_MEMBER_RENAME_NOT_VISIBLE');
 const expectedRetrieve=Boolean(lastOption.payload.ranks[0].options&16);
 await poll(()=>checkbox(owner,2,'CanRetrieveItem').isChecked(),value=>value===expectedRetrieve,'OTHER_MEMBER_OPTION_NOT_VISIBLE');
 assert.equal(commands(owner,'editGuildMember').length,ownerEdits);await tableIntact(owner);
 assert.ok(rankPackets(owner,ownerPacketStart).some(e=>e.payload.status===7&&e.payload.name===p.characterName&&e.payload.ranks?.[0]?.index===1));
 proof.checks.push({name:'OtherMemberRealDeltasKeepOwnerRosterWithoutCommandReplay',passed:true});
 await shot(officer,'officer-hierarchy-and-authoritative-options');await shot(owner,'owner-sees-officer-updates');
 phaseTo('final-reentry-and-preservation');await key(officer,'Guilds');await logout(officer);await key(owner,'Guilds');await logout(owner);await enter(owner);await ranks(owner);
 assert.equal(await rankName(owner,1).inputValue(),officerName);assert.equal(await rankName(owner,2).inputValue(),memberName);
 assert.equal(await checkbox(owner,2,'CanRetrieveItem').isChecked(),expectedRetrieve);await tableIntact(owner);await shot(owner,'owner-final-persisted-ranks');
 await key(owner,'Guilds');await logout(owner);
 for(const session of sessions){
  const forbidden=proof.commands.filter(row=>row.actor===session.actor&&['walk','run','turn','attack','rangeAttack','magic','useItem','pickUp'].includes(row.type));assert.deepEqual(forbidden,[]);
  assert(commands(session,'requestGuildInfo').every(row=>row.infoType===1));
  assert(commands(session,'requestGuildInfo').length<=8,'No full-roster request loop');
  assert.equal(commands(session,'login').length,1);assert.equal(commands(session,'editGuildMember').filter(row=>row.changeType===3).length,session.actor==='owner'?1:1);
  assert.equal(commands(session,'editGuildMember').filter(row=>row.changeType===5).length,session.actor==='owner'?2:1);
  proof.checks.push({name:'NoWorldMovementCombatItemsOrRosterReadLoop',actor:session.actor,passed:true});
 }
 assert.deepEqual(proof.pageErrors,[]);assert.deepEqual(proof.networkErrors,[]);assert.deepEqual(proof.consoleErrors,[]);
 proof.checks.push({name:'FinalOrdinaryReentryReadsSharedDurableChanges',passed:true});proof.passed=true;phaseTo('complete');
}catch(error){
 await writeFile(output+'/error.private.json',JSON.stringify({message:error.message,stack:error.stack}),{mode:0o600});
 proof.errors.push({phase,code:error.code??null,messageHash:hash(error.message)});console.log(JSON.stringify({failed:true,phase,messageHash:hash(error.message)}));process.exitCode=1;
}finally{
 for(const session of sessions){try{if(!proof.passed)await shot(session,session.actor+'-failure');}catch{}
  try{await writeFile(output+'/'+session.actor+'-diagnostics.private.json',json({ui:await ui(session),state:session.observation.snapshot(),guildPackets:session.guildPackets,events:session.rawEvents,console:session.console,screen:await session.page.locator('main').getAttribute('data-client-screen')}),{mode:0o600});}catch(error){await writeFile(output+'/'+session.actor+'-diagnostic-error.private.txt',error.message,{mode:0o600});}
 }
 await browser?.close().catch(()=>{});proof.completedAt=new Date().toISOString();await writeFile(output+'/proof.json',JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({passed:proof.passed,checks:proof.checks.length,commands:proof.commands.length,errors:proof.errors.length}));
}
