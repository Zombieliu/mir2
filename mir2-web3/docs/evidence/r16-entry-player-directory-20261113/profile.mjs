import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
export function profileProcesses(profile) {
 const needle=profile.replaceAll('\\','/');assert(!needle.includes("'"));
 const command=`$target='${needle}'; @(Get-CimInstance Win32_Process -Filter "Name='chrome.exe'" | Where-Object { $_.CommandLine -and $_.CommandLine.Replace([char]92,[char]47).Contains($target) } | Select-Object ProcessId,ParentProcessId) | ConvertTo-Json -Compress`;
 const raw=execFileSync('powershell.exe',['-NoProfile','-Command',command],{encoding:'utf8',windowsHide:true}).trim();
 const rows=raw?JSON.parse(raw):[];return Array.isArray(rows)?rows:[rows];
}
export async function launchProfile({chromium,profile,language}) {
 let closed=false;
 const context=await chromium.launchPersistentContext(profile,{headless:true,executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',
  locale:language,viewport:{width:1280,height:960},args:['--disable-background-timer-throttling','--disable-renderer-backgrounding']});
 context.on('close',()=>{closed=true;});
 const page=context.pages()[0]??await context.newPage();await page.goto('about:blank');
 return{page,context,handle:{close:()=>context.close(),isConnected:()=>!closed}};
}
export async function fontCacheInventory(page) {
 await page.waitForFunction(()=>navigator.serviceWorker.controller,null,{timeout:15000});
 let worker;
 for(let i=0;i<100;i++){
  worker=page.context().serviceWorkers().find(w=>w.url().includes('mir2-asset-worker.js'));
  if(worker&&await worker.evaluate(()=>runtimeConfig.assetVersion==='20261108-r16-08ec86d5105c').catch(()=>false))break;
  await new Promise(resolve=>setTimeout(resolve,100));
 }
 assert(worker,'normal configured asset worker must exist');
 const names=['NotoSansCJKsc-Regular.otf','NotoSansTC.ttf','NotoSans-Regular.ttf','NotoSansDevanagari-Regular.ttf','NotoSansThai-Regular.ttf','NotoSansArabic-Regular.ttf'];
 return worker.evaluate(async names=>{
  const cache=await caches.open(cacheName('static-critical')),rows=[];
  for(const name of names){const response=await cache.match(new Request(location.origin+'/original-ui/fonts/'+name));
   rows.push({name,present:Boolean(response),contentLength:response?.headers.get('content-length')??null});void response?.body?.cancel().catch(()=>{});}
  return{assetVersion:runtimeConfig.assetVersion,cacheNamespace:runtimeConfig.version,rows};
 },names);
}
