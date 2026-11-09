import {createRequire} from 'node:module';
import {readFile,writeFile} from 'node:fs/promises';
import {setTimeout as delay} from 'node:timers/promises';
const {chromium}=createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json')('playwright');
const identities=JSON.parse(await readFile('D:/mir2-builds/r16-parity-20261109/trade-cancel-ui-r1/private/ui-identities.local.json','utf8'));
const b=await chromium.connectOverCDP('http://127.0.0.1:19484');const page=b.contexts()[0].pages().find(p=>p.url().startsWith('http://127.0.0.1:19482'));
const cdp=await page.context().newCDPSession(page);await cdp.send('Network.enable');const packets=[];
cdp.on('Network.webSocketFrameReceived',e=>{try{const p=JSON.parse(e.response.payloadData);if(['DeleteMember','DeleteGroup','LogOutSuccess'].includes(p.packet))packets.push({packet:p.packet});}catch{}});
const group=page.locator('[data-r16-group-window="true"]');const present=(await group.locator('[data-member-name]').allTextContents()).includes(identities.peer.characterName);
if(present){await group.locator('[data-r16-group-remove]').click();await page.locator('[data-r16-group-name]').fill(identities.peer.characterName);await page.locator('[data-r16-group-name-submit]').click();}
await page.waitForFunction(name=>![...document.querySelectorAll('[data-r16-group-window="true"] [data-member-name]')].some(e=>e.getAttribute('data-member-name')===name),identities.peer.characterName,{timeout:10000});
await group.getByRole('button',{name:/^(关闭|Close)$/}).click();await group.waitFor({state:'hidden'});await page.locator('.client-stage-frame canvas:visible').first().focus();await page.keyboard.press('Alt+KeyX');await page.locator('.select-overlay').waitFor({timeout:15000});await delay(200);
await writeFile('D:/mir2-builds/r16-parity-20261109/social-budget-ui-r1/browser-r1/ordinary-cleanup.json',JSON.stringify({ordinaryDomRemovePeerPreviouslyIssued:!present,ordinaryDomRemovePeerIssuedNow:present,peerAbsentFromUiRoster:true,ordinaryLogoutReturnedToSelection:true,packets,at:new Date().toISOString()}),{flag:'wx'});
console.log(JSON.stringify({ordinaryDomRemovePeer:true,ordinaryLogout:true,packetTypes:packets.map(p=>p.packet)}));process.exit(0);
