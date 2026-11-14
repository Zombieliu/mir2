import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createRequire} from 'node:module';
const require=createRequire('F:/mir2-builds/web-r16-20261104/tools/package.json'),{chromium}=require('playwright');
const origin='https://numeron.obelisk.build', out='C:/r16-network-isolation-20261114/candidate-install';
await mkdir(out,{recursive:false});
const source=await readFile('E:/mir2-player-journey/mir2-web3/apps/web/public/mir2-asset-worker.js','utf8');
const sha=createHash('sha256').update(source).digest('hex');
const candidate=source+'\nself.__R16_DIAGNOSTIC_SOURCE_SHA='+JSON.stringify(sha)+';\n';
const receipt={passed:false,gameLogin:false,workerScriptSubstitutionOnly:true,publicDeployment:false,sourceSha256:sha,intercepted:0,browserClosed:false};
let browser;
try{
 browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true});
 const context=await browser.newContext(),page=await context.newPage();
 await page.goto(origin+'/version',{waitUntil:'domcontentloaded',timeout:30000});
 await context.route(origin+'/mir2-asset-worker.js',async route=>{receipt.intercepted++;await route.fulfill({status:200,contentType:'application/javascript',headers:{'Cache-Control':'no-store','Service-Worker-Allowed':'/'},body:candidate});});
 await page.evaluate(async()=>{await navigator.serviceWorker.register('/mir2-asset-worker.js');await navigator.serviceWorker.ready;});
 const workers=context.serviceWorkers();assert.equal(workers.length,1);
 receipt.worker=await workers[0].evaluate(()=>({pin:self.__R16_DIAGNOSTIC_SOURCE_SHA??null,concurrency:typeof FONT_DOWNLOAD_CONCURRENCY==='number'?FONT_DOWNLOAD_CONCURRENCY:null}));
 assert.equal(receipt.worker.pin,sha);assert.equal(receipt.worker.concurrency,2);assert.equal(receipt.intercepted,1);receipt.passed=true;
}catch(error){receipt.failure={name:error.name,message:error.message};}
finally{await browser?.close();receipt.browserClosed=true;receipt.producerSha256=createHash('sha256').update(await readFile(new URL(import.meta.url))).digest('hex');await writeFile(out+'/receipt.json',JSON.stringify(receipt,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify(receipt));}
if(!receipt.passed)process.exitCode=1;
