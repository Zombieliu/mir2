import {createRequire} from 'node:module';
import {writeFile} from 'node:fs/promises';
const {chromium}=createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json')('playwright');
const b=await chromium.connectOverCDP('http://127.0.0.1:19484');const page=b.contexts()[0].pages().find(p=>p.url().startsWith('http://127.0.0.1:19482'));
const cdp=await page.context().newCDPSession(page);await cdp.send('Network.enable');let acknowledged=false;cdp.on('Network.webSocketFrameReceived',e=>{try{acknowledged ||= JSON.parse(e.response.payloadData).packet==='LogOutSuccess';}catch{}});
await page.locator('.client-stage-frame canvas:visible').first().focus();await page.keyboard.press('Alt+KeyX');await page.locator('.select-overlay').waitFor({timeout:15000});
await writeFile('D:/mir2-builds/r16-parity-20261109/social-budget-ui-r1/browser-r2/ordinary-cleanup.json',JSON.stringify({ordinaryPrimaryLogoutAck:acknowledged,returnedToSelection:true,at:new Date().toISOString()}),{flag:'wx'});console.log(JSON.stringify({ordinaryPrimaryLogoutAck:acknowledged}));process.exit(0);
