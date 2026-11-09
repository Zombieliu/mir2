import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { setTimeout as delay } from 'node:timers/promises';

const [repo, root, fixture, leaf='browser-r1'] = process.argv.slice(2);
assert(repo && root && fixture && /^browser-r\d+$/.test(leaf));
const read = async path => JSON.parse(await readFile(path, 'utf8'));
const [config, startup, owner, visitors] = await Promise.all([
  read(join(root, 'public-config.json')), read(join(root, 'startup.json')),
  read(join(fixture, 'identity.local.json')), read(join(fixture, 'peers.local.json')),
]);
const guest = visitors[0];
const manifest = await read(join(repo, 'apps/web/lib/generated/bevy_runtime_version.json'));
const keyboard = await read(join(repo, 'apps/game-client/client-bevy/src/crystal_ui/keyboard_defaults.json'));
const load = path => import(pathToFileURL(join(repo, path)));
const { GatewayClient, DIRECTIONS } = await load('apps/ai-player/src/gateway-client.mjs');
const { ObservationReducer } = await load('apps/ai-player/src/observation.mjs');
const { parseGatewayJson } = await load('apps/ai-player/src/lossless-json.mjs');
const { chromium } = createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json')('playwright');
const Ws = createRequire(join(repo, 'apps/web/package.json'))('next/dist/compiled/ws');
const out = join(root, leaf); await mkdir(out);
const hash = data => createHash('sha256').update(data).digest('hex');
const stringify = value => JSON.stringify(value, (_, item) => typeof item === 'bigint' ? item.toString() : item);

const proof = {
  schema: 1, passed: false, reproduced: false, mode: config.mode, phase: 'startup',
  pageSha256: startup.pageSha256, gatewaySha256: startup.gatewaySha256, runtimeVersion: manifest.version,
  scope: { fileClone: true, preparedGuild: true, ordinaryAccounts: 2, privilegedCommands: 0, modelCalls: 0,
    holdOnlyMatchingRealGuildAck: true, injectGameplayActions: false },
  checks: [], trace: [], screenshots: [], modules: [], errors: [], assets: {},
};
const authoritative = new ObservationReducer(), received = new ObservationReducer();
authoritative.resetConnection(); received.resetConnection();
const packets = [], commands = [], moduleReads = [];
let browser, context, page, peer, cdp, clientRoute, hold, rawGuild, fatal, primaryEntry;
let fullSnapshots=0, lastFullSnapshotTick=null;
const primary = () => authoritative.snapshot();
const offset = [[0,-1],[1,-1],[1,0],[1,1],[0,1],[-1,1],[-1,0],[-1,-1]];
const note = (event, details = {}) => proof.trace.push({ order: proof.trace.length + 1, at: Date.now(), event, ...details });
const phase = value => { proof.phase = value; console.log(value); };
const checked = (name, details = {}) => { proof.checks.push({ name, ...details }); console.log(name); };
async function poll(get, accept, code, ms=15000) {
  const deadline = Date.now() + ms;
  do { if (fatal) throw Error(fatal); const result = await get(); if (accept(result)) return result; await delay(75); }
  while (Date.now() < deadline);
  throw Error(code);
}
const after = (actor, packet, cursor) => packets.filter(row => row.actor === actor && row.packet === packet && row.order > cursor);
const sent = (actor, type, cursor=0) => commands.filter(row => row.actor === actor && row.type === type && row.order > cursor);
const position = s => [s.player.x, s.player.y, s.player.direction];
const assets = s => ({
  gold: s.gold,
  inventory: s.inventoryItems.map(i => ({ uidHash: hash(String(i.uniqueId)), slot: i.slot, count: i.quantity, name: i.name })).sort((a,b) => a.slot-b.slot),
  belt: s.beltItems.map(i => ({ uidHash: hash(String(i.uniqueId)), slot: i.slot, count: i.quantity })).sort((a,b) => a.slot-b.slot),
});
const assetsEqual = (a,b) => stringify(a) === stringify(b);
const closed = s => s.trade.active === false && s.trade.uncertain === false;
const guild = () => page.locator('[data-guild-name="RankQA"]');
const picker = () => guild().locator('[aria-live="polite"]').filter({hasText:/Waiting for item update|等待.*物品/});
const refresh = () => guild().getByRole('button', {name:/^(Refresh|刷新)$/});
const trade = () => page.locator('[data-trade-state="open"]');
const inventoryChoice = item => guild().getByRole('button', {name: new RegExp('^'+item.name+' ×'+item.quantity+'$')});
const loggable = new Set(['StartGame','TradeRequest','TradeAccept','TradeItem','TradeCancel','DepositTradeItem','GuildStatus',
  'GuildStorageList','GuildStorageItemChange','LogOutSuccess','GainedItem','GainedGold']);
const allowed = new Set(['version','clientVersion','login','startGame','logOut','logout','turn','walk','run','requestGuildInfo',
  'guildStorageItemChange','tradeRequest','tradeReply','tradeCancel','depositTradeItem','mapInfo','ping','pong','clientReady','requestUserName',
  'requestMapInfo','closeDialog','dayTime','clientVersionV2','getClientInfo']);
function observeCommand(actor, value) {
  const type = value.type;
  if (/^(qa\.|event\.|moveTo|stage5Command|passkeyLogin|newAccount|newCharacter|tradeConfirm|tradeGold|useItem|dropItem)/i.test(type))
    throw Error('UNEXPECTED_ECONOMIC_OR_PRIVILEGED_COMMAND');
  const data = allowed.has(type) ? { type, ...(type === 'guildStorageItemChange' ? { changeType:value.changeType, from:value.from, to:value.to } : {}),
    ...(type === 'depositTradeItem' ? { from:value.from, to:value.to } : {}) } : { type };
  const row = { order: proof.trace.length+1, actor, ...data };
  note('command', { actor, ...data }); commands.push(row);
}
function observePacket(actor, packet, payload, raw) {
  const row = { order: proof.trace.length+1, actor, packet, payload };
  packets.push(row);
  if (!loggable.has(packet)) return;
  const projection = packet === 'GuildStorageList' ? { slots:payload.items?.length } : {
    changeType:payload.changeType, from:payload.from, to:payload.to, user:payload.user,
    unlock:payload.unlock, success:payload.success, result:payload.result,
    itemUidHash:payload.item?.item?.unique_id === undefined ? undefined : hash(String(payload.item.item.unique_id)),
  };
  note('server-packet', { actor, packet, ...projection, ...(raw&&['GuildStorageItemChange','TradeCancel','DepositTradeItem'].includes(packet) ? {wireSha256:hash(raw)} : {}) });
}
async function shot(name) {
  const file = name+'.png';
  await page.screenshot({path:join(out,file), mask:[page.locator('.login-input:visible,[data-login-auth-field]:visible,.select-identity-id:visible')], timeout:10000});
  proof.screenshots.push({ file, sha256:hash(await readFile(join(out,file))) });
}
async function key(name) {
  const binding = keyboard.find(r => r.function === name); assert(binding);
  await page.locator('.client-stage-frame').focus();
  const chord = [binding.ctrl&&'Control',binding.alt&&'Alt',binding.shift&&'Shift',binding.key].filter(Boolean).join('+');
  await page.keyboard.press(chord, { delay:60 });
}
async function enterPrimary() {
  const seq = proof.trace.length;
  await page.locator('.select-character-slot-card').filter({hasText:owner.name}).click();
  await page.locator('.select-action.start button').click();
  const session = await poll(() => after('primary','StartGame',seq), rows => rows.some(row => row.payload.result === 4), 'PRIMARY_NO_STARTGAME', 75000);
  primaryEntry = session.find(row => row.payload.result === 4).order;
  await poll(primary, s => s.ready && s.player?.name === owner.name, 'PRIMARY_NOT_IN_WORLD', 75000);
  await poll(() => page.locator('.client-stage-frame').getAttribute('class'), c => c && !c.includes('scene-assets-pending'), 'SCENE_NOT_READY', 90000);
  await poll(() => rawGuild, g => g?.name === 'RankQA', 'GUILD_MEMBERSHIP_NOT_LOADED');
}
async function startPair() {
  phase('ordinary-account-entry');
  await page.locator('.login-input.account:enabled').waitFor({timeout:90000});
  await page.locator('.login-input.account').fill(owner.accountId); await page.locator('.login-input.password').fill(owner.password);
  await page.locator('.login-button.ok button').click(); await page.locator('.select-overlay').waitFor({timeout:60000});
  await enterPrimary();
  class OriginWs extends Ws { constructor(url) { super(url,{origin:config.origin}); } }
  peer = new GatewayClient({url:config.gatewayUrl,realmId:config.realmId,WebSocketImpl:OriginWs,timeoutMs:20000});
  peer.on('frame', ({event}) => { if (event.type === 'packet') observePacket('peer',event.packet,event.payload); });
  peer.on('command', command => observeCommand('peer',command));
  await peer.connect(); await peer.login(guest.accountId,guest.password); await peer.startGame(guest.characterIndex);
  checked('both-normal-entries-with-original-characters',{primary:position(primary()),peer:position(peer.snapshot()),guild:rawGuild.name});
}
async function face() {
  const { x, y, direction } = primary().player, dir = DIRECTIONS.indexOf(direction);
  assert(dir >= 0);
  const [dx,dy] = offset[dir], target = {x:x+dx,y:y+dy};
  for (let step=0; step<8; step++) {
    const current = peer.snapshot().player;
    if (current.x === target.x && current.y === target.y) break;
    const sx=Math.sign(target.x-current.x), sy=Math.sign(target.y-current.y);
    const choices=[[sx,sy],[0,sy],[sx,0]].filter(([a,b]) => (a||b) && (current.x+a!==x || current.y+b!==y));
    assert(choices.length, 'PEER_ALIGNMENT_NO_LEGAL_CANDIDATE');
    const [a,b]=choices[0], index=offset.findIndex(([vx,vy])=>vx===a&&vy===b);
    await peer.walk(DIRECTIONS[index]); await delay(550);
  }
  const actual=peer.snapshot().player;
  assert(actual.x===target.x&&actual.y===target.y,'PEER_NOT_ON_FACING_TILE');
  const opposite=DIRECTIONS[(dir+4)%8]; await peer.turn(opposite);
  await poll(() => peer.snapshot(),s=>s.player.direction===opposite&&s.entities.some(e=>e.kind==='player'&&e.x===x&&e.y===y&&e.direction===direction),'MUTUAL_FACING_NOT_OBSERVED');
}
async function beginTrade() {
  const seq=proof.trace.length; await peer.requestPlayerTrade();
  await poll(() => after('primary','TradeRequest',seq),rows=>rows.length===1,'TRADE_INVITE_ABSENT');
  await page.locator('[data-r16-trade-accept]').click();
  await poll(primary,s=>s.trade.active&&s.trade.ownOfferKnown&&!s.trade.uncertain,'PRIMARY_OFFER_NOT_READY');
  await poll(() => peer.snapshot(),s=>s.trade.active&&s.trade.ownOfferKnown&&!s.trade.uncertain,'PEER_OFFER_NOT_READY',25000);
  await trade().waitFor();
}
async function putTradeItem(item) {
  const choice=trade().locator('[data-r16-trade-bag-picker] button[title]').filter({hasText:item.name+' ×'+item.quantity});
  const seq=proof.trace.length;
  assert.equal(await choice.count(),1,'TRADE_ITEM_CHOICE_AMBIGUOUS');
  await choice.click(); await trade().locator('[data-r16-trade-side="own"] button[data-trade-slot="0"]').click();
  await poll(() => after('primary','DepositTradeItem',seq),r=>r.some(e=>e.payload.success===true&&e.payload.from===item.slot&&e.payload.to===0),'REAL_TRADE_DEPOSIT_NOT_ACCEPTED');
  await poll(() => trade().locator('[data-r16-trade-confirm]').isEnabled(),Boolean,'TRADE_STILL_WAITING');
}
async function readBank() {
  // Crystal sends its full list once per guild session. Live changes arrive
  // separately. Read an already received list only as the precondition here.
  const cached=after('primary','GuildStorageList',primaryEntry).find(r=>r.payload.items?.length===112);
  if(cached)return cached.payload.items;
  const seq=proof.trace.length; await refresh().click();
  const listed=await poll(() => after('primary','GuildStorageList',seq),rows=>rows.some(r=>r.payload.items?.length===112),'REAL_BANK_LIST_NOT_RECEIVED');
  return listed.find(r=>r.payload.items?.length===112).payload.items;
}
async function openBank() {
  await key('Guilds'); await guild().waitFor();
  // Both panels are draggable by players. Keep the Guild picker clear of the
  // trade offer panels using the real title drag, without forcing any clicks.
  const stage = await page.locator('.client-stage-frame').boundingBox();
  const window = await guild().boundingBox();
  const handle = await guild().locator('[data-guild-drag-handle]').boundingBox();
  const dx = Math.round(stage.x+24-window.x), dy = Math.round(stage.y+12-window.y);
  if (Math.abs(dx)>3 || Math.abs(dy)>3) {
    const x=Math.round(handle.x+handle.width/2), y=Math.round(handle.y+handle.height/2);
    await page.mouse.move(x,y); await page.mouse.down();
    await page.mouse.move(x+dx,y+dy,{steps:12}); await page.mouse.up();
    await poll(()=>guild().boundingBox(),r=>r&&Math.abs(r.x-(stage.x+24))<8&&Math.abs(r.y-(stage.y+12))<8,'REAL_GUILD_TITLE_DRAG_FAILED');
  }
  await guild().locator('button[data-guild-tab="storage"]').click();
  return readBank();
}
async function closeBank() { await guild().getByRole('button',{name:/^(Close|关闭)$/}).click(); await guild().waitFor({state:'hidden'}); }
function transferInBag(who, item, changedSlot) {
  const asset = assets(who), uidHash = hash(String(item.uniqueId));
  const hasItem = asset.inventory.find(row => row.uidHash===uidHash && row.count===item.quantity);
  return changedSlot===null ? !hasItem : hasItem?.slot===changedSlot;
}
function release() {
  assert(hold?.raw && hold.event && clientRoute);
  const h=hold; hold=null;
  received.apply(h.event); clientRoute.send(h.raw);
  note('guild-ack-released-unmodified',{wireSha256:hash(h.raw),heldMs:Date.now()-h.at});
  return h;
}
async function pendingCancel({item,kind,bagTarget,bankSlot}) {
  const changeType=kind==='deposit'?0:1;
  phase('open-trade-and-guild-'+kind);
  await beginTrade();
  const before=assets(primary()), peerBefore=assets(peer.snapshot());
  const offered=primary().inventoryItems.find(i=>String(i.uniqueId)!==String(item.uniqueId));
  assert(offered,'TRADE_ITEM_MISSING');
  await putTradeItem(offered);
  const bank=await openBank();
  if (changeType===0) assert.equal(bank[bankSlot],null,'DEPOSIT_SLOT_NOT_EMPTY');
  else assert.equal(String(bank[bankSlot]?.item?.unique_id),String(item.uniqueId),'STORED_UID_MISSING');
  const command={changeType,from:changeType===0?item.slot:bankSlot,to:changeType===0?bankSlot:bagTarget};
  hold={expected:command,raw:null};
  const beforeMutation=proof.trace.length;
  if (changeType===0) {
    await inventoryChoice(item).click(); await guild().locator('button[data-storage-slot="'+bankSlot+'"]').click();
  } else {
    await guild().locator('button[data-storage-slot="'+bankSlot+'"]').click();
    await guild().getByRole('button',{name:new RegExp('^(Empty bag|空背包|背包空格) ?'+(bagTarget+1)+'$')}).click();
  }
  await poll(()=>hold,h=>h?.raw,'NO_MATCHING_REAL_GUILD_ACK');
  assert.equal(sent('primary','guildStorageItemChange',beforeMutation).filter(r=>r.changeType===changeType).length,1);
  assert.equal(hold.event.payload.changeType,changeType,'REAL_GUILD_MUTATION_REJECTED');
  const uid = changeType===0 ? hold.event.payload.item?.item?.unique_id : item.uniqueId;
  assert.equal(String(uid),String(item.uniqueId),'GUILD_MUTATION_UID_MISMATCH');
  await poll(primary,s=>transferInBag(s,item,bagTarget),'COMMITTED_GUILD_BAG_SNAPSHOT_MISSING');
  const bankPickerShown=await picker().isVisible();
  if(changeType===0)assert(bankPickerShown,'DEPOSIT_PENDING_HINT_NOT_VISIBLE');
  // Retrieve can expose the new bag snapshot before the old bank row is
  // removed. The product hides the picker for that ambiguous UID projection.
  // Refresh stays rendered and tracks the independent item barrier directly.
  assert(await refresh().isDisabled(),'GUILD_REFRESH_NOT_GUARDED');
  assert(await guild().locator('button[data-storage-slot="'+bankSlot+'"]').isDisabled(),'GUILD_STORAGE_CELL_NOT_GUARDED');
  note('guild-item-controls-blocked-with-ack-held',{changeType,bankPickerShown});
  assert(await trade().locator('[data-r16-trade-confirm]').isDisabled(),'TRADE_CONFIRM_NOT_GUARDED');
  const cancel=trade().locator('[data-r16-trade-cancel]'); assert(await cancel.isEnabled(),'CANCEL_CONTROL_DISABLED');
  await shot(kind+'-guild-pending');
  const beforeClick=proof.trace.length;
  phase('click-cancel-before-guild-ack-'+kind);
  await cancel.click();
  if (config.mode==='baseline') {
    try { await poll(()=>sent('primary','tradeCancel',beforeClick),rows=>rows.length>0,'CANCEL_SWALLOWED',2200); }
    catch (error) { if (error.message!=='CANCEL_SWALLOWED') throw error; }
    assert.equal(sent('primary','tradeCancel',beforeClick).length,0,'BASELINE_UNEXPECTEDLY_SENT_CANCEL');
    assert(await trade().isVisible()&&await picker().isVisible(),'BASELINE_DID_NOT_STAY_PENDING');
    assert.equal(after('primary','TradeCancel',beforeClick).length,0);
    proof.reproduced=true;
    checked('baseline-visible-click-does-not-send-while-guild-ack-held',{heldRealGuildChange:changeType,tradeStayedOpen:true,wireCancelCommands:0});
    await shot(kind+'-baseline-click-swallowed');
    release(); await poll(()=>refresh().isEnabled(),Boolean,'BASELINE_GUILD_ACK_DID_NOT_RELEASE');
    await closeBank(); await cancel.click();
    await poll(primary,closed,'BASELINE_CONTROL_CANCEL_DID_NOT_CLOSE');
    await poll(()=>peer.snapshot(),closed,'BASELINE_CONTROL_PEER_DID_NOT_CLOSE');
    return;
  }
  await poll(()=>sent('primary','tradeCancel',beforeClick),rows=>rows.length===1,'TRADE_CANCEL_WIRE_MISSING');
  await poll(primary,closed,'PRIMARY_TRADE_NOT_CLOSED');
  await poll(()=>peer.snapshot(),closed,'PEER_TRADE_NOT_CLOSED');
  await trade().waitFor({state:'hidden'});
  assert.equal(after('primary','TradeCancel',beforeClick).filter(r=>r.payload.unlock===false).length,1);
  assert.equal(after('peer','TradeCancel',beforeClick).filter(r=>r.payload.unlock===false).length,1);
  assert(await refresh().isDisabled(),'TRADE_TERMINAL_UNBLOCKED_GUILD');
  assert(await guild().locator('button[data-storage-slot="'+bankSlot+'"]').isDisabled(),'TRADE_TERMINAL_UNLOCKED_GUILD_SLOT');
  if(changeType===0)assert(await picker().isVisible(),'GUILD_PENDING_HINT_DISAPPEARED');
  await shot(kind+'-trade-closed-guild-still-pending');
  const beforeRelease=fullSnapshots, released=release();
  await delay(350);
  const afterIdle=fullSnapshots, blockedBeforeRead=await refresh().isDisabled();
  if (afterIdle===beforeRelease) assert(blockedBeforeRead,'GUILD_UNBLOCKED_WITHOUT_LATER_FULL_SNAPSHOT');
  // An idle Gateway can send only KeepAlive after this deliberately reordered
  // ACK. Members is a real read that normally publishes a complete snapshot.
  const beforeRead=proof.trace.length;
  await guild().locator('button[data-guild-tab="members"]').click();
  await poll(()=>sent('primary','requestGuildInfo',beforeRead),r=>r.length===1,'ORDINARY_GUILD_ROSTER_READ_MISSING');
  await poll(()=>fullSnapshots,n=>n>afterIdle,'POST_ACK_COMPLETE_SNAPSHOT_MISSING');
  const afterRead=fullSnapshots;
  await guild().locator('button[data-guild-tab="storage"]').click();
  await poll(()=>refresh().isEnabled(),Boolean,'GUILD_STILL_BLOCKED_AFTER_OWN_ACK_AND_SNAPSHOT');
  note('guild-unblocked-by-own-ack-and-later-full-snapshot',{beforeRelease,afterIdle,blockedBeforeRead,afterRead,lastFullSnapshotTick});
  assert(!await trade().isVisible(),'LATE_GUILD_ACK_REOPENED_TRADE');
  assert.equal(sent('primary','tradeCancel',beforeClick).length,1);
  assert.equal(sent('primary','guildStorageItemChange',beforeMutation).filter(r=>r.changeType===changeType).length,1);
  const cell=guild().locator('button[data-storage-slot="'+bankSlot+'"]');
  const label=await poll(()=>cell.getAttribute('aria-label'),
    value=>changeType===0?value===item.name:/^(Empty storage|空仓库|仓库空格)/.test(value??''),
    'LIVE_GUILD_DELTA_NOT_RENDERED');
  const expected=structuredClone(before), uidHash=hash(String(item.uniqueId));
  if (changeType===0) expected.inventory=expected.inventory.filter(i=>i.uidHash!==uidHash);
  else expected.inventory=[...expected.inventory,{uidHash,slot:bagTarget,count:item.quantity,name:item.name}].sort((a,b)=>a.slot-b.slot);
  assert.deepEqual(assets(primary()),expected); assert.deepEqual(assets(received.snapshot()),expected);
  assert.deepEqual(assets(peer.snapshot()),peerBefore);
  checked('one-trade-cancel-and-one-guild-'+kind,{guildCommand:command,oneTerminalPerPlayer:true,
    guildStillPendingAfterTradeTerminal:true,exactAssets:true,liveBankCellLabel:label,ackWireSha256:hash(released.raw),reopenAfterLateAck:false});
  proof.assets[kind]={primary:expected,peer:peerBefore,bankSlot,bankItem:changeType===0?uidHash:null};
  await closeBank();
}
async function logoutBoth() {
  const seq=proof.trace.length; await page.locator('.client-stage-frame').focus(); await page.keyboard.press('Alt+KeyX');
  const yes=page.locator('[role="dialog"]:visible').getByRole('button',{name:/^(Yes|是|确认|确定)$/});
  await delay(180); if (!after('primary','LogOutSuccess',seq).length && await yes.count()===1) await yes.click();
  await poll(()=>after('primary','LogOutSuccess',seq),r=>r.length===1,'PRIMARY_LOGOUT_ACK_MISSING');
  await page.locator('.select-overlay').waitFor();
  assert(await peer.logout(),'PEER_LOGOUT_ACK_MISSING');
}
async function reenterAndRead(kind, item) {
  phase('same-character-save-readback-'+kind);
  await logoutBoth(); await enterPrimary(); await peer.startGame(guest.characterIndex);
  assert.deepEqual(assets(primary()),proof.assets[kind].primary);
  assert.deepEqual(assets(peer.snapshot()),proof.assets[kind].peer);
  const bank=await openBank(), value=bank[proof.assets[kind].bankSlot];
  assert.equal(value?hash(String(value.item.unique_id)):null,proof.assets[kind].bankItem);
  if(value) assert.equal(value.item.count,item.quantity);
  await closeBank();
  checked('both-ordinary-character-readbacks-'+kind,{bankAndBothAssetSetsExact:true,newPrimaryStartOrder:primaryEntry});
}
try {
  browser=await chromium.launch({headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',args:['--enable-unsafe-swiftshader']});
  context=await browser.newContext({locale:'en-US',viewport:{width:1280,height:960}});
  page=await context.newPage(); page.setDefaultTimeout(9000);
  page.on('pageerror',error=>proof.errors.push({type:'page',hash:hash(error.message)}));
  page.on('console',entry=>{if(entry.type()==='error')proof.errors.push({type:'console',hash:hash(entry.text())});});
  page.on('response',response=>{
    const url=new URL(response.url());
    if(response.status()>=400)proof.errors.push({type:'http',path:url.pathname,status:response.status()});
    if(url.pathname.endsWith('/mir2_bevy_runtime.js'))moduleReads.push(response.body().then(data=>{
      const sha256=hash(data); proof.modules.push({path:url.pathname,sha256,pinned:manifest.files.some(f=>f.path.endsWith('.js')&&f.sha256===sha256)});
    }).catch(()=>{}));
  });
  await page.routeWebSocket(config.gatewayUrl, route => {
    clientRoute=route; const real=route.connectToServer();
    route.onMessage(raw=>{
      try { observeCommand('primary',parseGatewayJson(String(raw))); real.send(raw); }
      catch { fatal='CLIENT_WIRE_POLICY_REJECTED'; }
    });
    real.onMessage(raw=>{
      try {
        const event=parseGatewayJson(String(raw)); authoritative.apply(event);
        if(event.type==='error'){note('gateway-error',{code:event.code??event.payload?.code??null});fatal='GATEWAY_ERROR';}
        if(event.type==='realmInfo')assert(event.payload.realmId===config.realmId);
        if(event.type==='worldSnapshot'){rawGuild=event.payload.stage5Systems?.guild??null; fullSnapshots++; lastFullSnapshotTick=event.payload.tick;}
        if(event.type==='packet')observePacket('primary',event.packet,event.payload,raw);
        if(hold&&!hold.raw&&event.type==='packet'&&event.packet==='GuildStorageItemChange'&&event.payload.user===owner.characterIndex
          && event.payload.changeType%3===hold.expected.changeType&&event.payload.from===hold.expected.from&&event.payload.to===hold.expected.to) {
          hold.raw=raw; hold.event=event; hold.at=Date.now();
          note('withheld-matching-real-guild-ack',{wireSha256:hash(raw),...hold.expected,userMatches:true}); return;
        }
        received.apply(event); route.send(raw);
      } catch { fatal='SERVER_WIRE_OBSERVATION_FAILED'; }
    });
  });
  phase('cold-navigation'); await page.goto(config.origin,{waitUntil:'commit',timeout:20000});
  await startPair();
  cdp=await context.newCDPSession(page); await cdp.send('Profiler.enable'); await cdp.send('Profiler.startPreciseCoverage',{callCount:true,detailed:true});
  await face();
  const item=primary().inventoryItems.find(i=>i.name==='Candle'); assert(item,'NO_ORDINARY_CANDLE');
  await pendingCancel({item,kind:'deposit',bagTarget:null,bankSlot:0});
  if (config.mode==='candidate') {
    await reenterAndRead('deposit',item); await face();
    const bagTarget=primary().inventoryItems.every(i=>i.slot!==4) ? 4 : item.slot;
    await pendingCancel({item,kind:'retrieve',bagTarget,bankSlot:0});
    await reenterAndRead('retrieve',item);
  }
  phase('both-final-normal-logouts'); await logoutBoth(); checked('both-final-logout-acks');
  const coverage=await cdp.send('Profiler.takePreciseCoverage');
  proof.nativeBridgeCalls=coverage.result.flatMap(s=>s.functions).filter(f=>f.functionName==='processMir2SocialBudget')
    .reduce((n,f)=>n+Math.max(0,...f.ranges.map(r=>r.count)),0);
  await cdp.send('Profiler.stopPreciseCoverage'); await Promise.all(moduleReads);
  assert(proof.nativeBridgeCalls>0&&proof.modules.some(m=>m.pinned),'PINNED_NATIVE_BRIDGE_NOT_OBSERVED');
  proof.passed=config.mode==='candidate';
} catch(error) {
  const message=error.message.split('\n')[0];
  proof.failure={phase:proof.phase,name:error.name,code:error.code??null,description:/^[A-Z0-9_ ]+$/.test(message)?message:null,messageSha256:hash(error.message)};
  await writeFile(join(root,'private',leaf+'-failure.local.txt'),error.stack??error.message,{flag:'wx'});
  try { await shot('failure'); } catch {}
} finally {
  if(hold?.raw)release();
  await peer?.close().catch(()=>{}); await browser?.close();
  proof.browserClosed=true;
  await writeFile(join(out,'proof.json'),stringify(proof),{flag:'wx'});
  console.log(stringify({mode:proof.mode,passed:proof.passed,reproduced:proof.reproduced,phase:proof.phase,checks:proof.checks.map(x=>x.name),failure:proof.failure??null}));
  process.exitCode=proof.passed || proof.reproduced && !proof.failure ? 0 : 1;
}
