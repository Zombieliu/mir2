// Actual Chrome + current Page/QR effect bodies + real Worker/original SC font.
// Loopback fixture only; no game account or real wallet approval.
import assert from 'node:assert/strict';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createServer} from 'node:http';
import {createRequire} from 'node:module';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import ts from 'typescript';
import QRCode from 'qrcode';

assert(process.argv.length===3&&process.env.MIR2_CHROME_BIN&&process.env.MIR2_PLAYWRIGHT_PACKAGE);
const output=resolve(process.argv[2]);await mkdir(output,{recursive:false});
const web=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const{chromium}=createRequire(resolve(process.env.MIR2_PLAYWRIGHT_PACKAGE))('playwright');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const compile=source=>ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
const sources={};
for(const[name,path]of[['helper','wallet-qr-font-preparation'],['request','asset-prewarm-request'],['font','bevy-runtime-font-prewarm'],['policy','bevy-runtime-policy']]){
 let source=await readFile(join(web,'lib',path+'.ts'),'utf8');if(name==='font')source=source.replace('"./asset-prewarm-request"','"/request.js"');sources['/'+name+'.js']=compile(source);
}
const pageSource=await readFile(join(web,'app/page.tsx'),'utf8'),qrSource=await readFile(join(web,'app/components/dubhe-wallet-qr-login.tsx'),'utf8');
function effect(source,match){let found;const ast=ts.createSourceFile('component.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
 const visit=node=>{if(ts.isCallExpression(node)&&node.expression.getText(ast)==='useEffect'&&match(node.arguments[0]?.getText(ast)??''))found=node.arguments[0];ts.forEachChild(node,visit);};visit(ast);assert(found);return found.getText(ast);}
const pageEffect=effect(pageSource,text=>text.startsWith('function prepareFontDuringWalletConfirmation'));
const qrEffect=effect(qrSource,text=>text.includes('observeWalletQrImage'));
const worker=await readFile(join(web,'public/mir2-asset-worker.js'));
const logical='/original-ui/fonts/NotoSansCJKsc-Regular.otf',font=await readFile(join(web,'public',logical.slice(1)));
const pin={bytes:16437364,sha256:'2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b'};assert.equal(font.length,pin.bytes);assert.equal(sha(font),pin.sha256);
const dataUrl=await QRCode.toDataURL('https://wallet.dubhe.obelisk.build/connect/approve?session=qr-font-loopback-fixture',{width:240,margin:4,errorCorrectionLevel:'M'});
sources['/fixture.js']=compile(`import{observeWalletQrImage,prepareFontAfterWalletQrImage,WALLET_QR_IMAGE_LIFECYCLE}from'/helper.js';
import{prewarmBevyRuntimePrimaryFont}from'/font.js';import{resolveBevyRuntimeBootDecision}from'/policy.js';
const bevyRuntimeStartup={runtimeAllowed:true},clientProfile={layout:'desktop',input:'keyboardMouse'};
const dubheQrStateRef={current:{status:'pending',expiresAt:Date.now()+60000}},qrCodeRef={current:document.querySelector('#qr')},imageUrl=qrCodeRef.current.src;
window.events=[];const markMir2CacheMilestone=(name,detail)=>window.events.push({name,detail,at:performance.now()});
window.addEventListener(WALLET_QR_IMAGE_LIFECYCLE,event=>window.events.push({name:'image-'+event.detail.phase,id:event.detail.id,at:performance.now(),decoded:qrCodeRef.current.complete&&qrCodeRef.current.naturalWidth===240}));
export const startPage=()=>(${pageEffect})();export const startImage=()=>(${qrEffect})();
export function retireImage(cleanup){cleanup();dubheQrStateRef.current=null;document.querySelector('#qr-panel').hidden=true;}
export const readForGame=()=>prewarmBevyRuntimePrimaryFont();
export async function verifyFont(pin){const response=await caches.match(location.origin+${JSON.stringify(logical)});if(!response)throw Error('complete font cache missing');const bytes=await response.arrayBuffer();const digest=async data=>[...new Uint8Array(await crypto.subtle.digest('SHA-256',data))].map(x=>x.toString(16).padStart(2,'0')).join('');
 const fullSha256=await digest(bytes);const partNames=(await caches.keys()).filter(name=>name.includes('font-parts-'+pin.sha256.slice(0,16)));let checkedParts=0;
 for(const name of partNames){const cache=await caches.open(name);for(const key of await cache.keys()){const response=await cache.match(key);const hash=new URL(key.url).pathname.split('/').at(-1);if(await digest(await response.arrayBuffer())!==hash)throw Error('part SHA mismatch');checkedParts++;}}
 const face=new FontFace('OriginalCJK',bytes);await face.load();document.fonts.add(face);const context=document.querySelector('canvas').getContext('2d');context.font='24px OriginalCJK';context.fillStyle='#eaca78';context.fillText('比奇省 · 冒险日记 · 阿洛',12,48);
 return{bytes:bytes.byteLength,sha256:fullSha256,checkedParts,fontFaceLoaded:face.status==='loaded'};}
`);
const requests=[],held=new Set();let phase='shared-fill',active=0,peak=0,browser,server;
const proof={kind:'actual-qr-effects-and-worker-shared-font-preparation',passed:false,
 scope:'Loopback QR matrix fixture using the real Page and QR observer effects, helper and current Worker; complete original SC body/63 parts. Not a game, real wallet approval, public performance or full six-font qualification.',
 sourceSha256:{page:sha(pageSource),qr:sha(qrSource),helper:sha(await readFile(join(web,'lib/wallet-qr-font-preparation.ts'))),worker:sha(worker)},pin,requests,closed:{}};
try{
 server=createServer((request,response)=>{
  const pathname=new URL(request.url,'http://fixture').pathname;
  response.setHeader('cache-control','no-store');response.setHeader('access-control-allow-origin','*');response.setHeader('access-control-expose-headers','content-range,content-length');
  if(pathname==='/mir2-asset-worker.js'){response.writeHead(200,{'content-type':'application/javascript','service-worker-allowed':'/'});response.end(worker);return;}
  if(sources[pathname]){response.writeHead(200,{'content-type':'application/javascript'});response.end(sources[pathname]);return;}
  if(pathname==='/'){response.setHeader('content-type','text/html');response.end('<!doctype html><html><head><title>QR font preparation fixture</title><style>body{background:#18130d;color:#eaca78;font:18px sans-serif;padding:24px}#qr-panel{width:300px;border:1px solid #ba963e;padding:12px;text-align:center}img{display:block;margin:12px auto}canvas{display:block;margin-top:16px}button{padding:8px 18px}</style></head><body><section id="qr-panel"><b>QR font preparation fixture</b><img id="qr" width="240" height="240" src="'+dataUrl+'"><p>Local matrix · no wallet approval</p></section><canvas width="560" height="80"></canvas></body></html>');return;}
  const path=pathname.replace(/^\/mir2\/v\/current/,'');
  if(path!==logical){response.writeHead(404);response.end();return;}
  const range=/^bytes=(\d+)-(\d+)$/.exec(request.headers.range??''),start=range?Number(range[1]):0,end=range?Number(range[2]):font.length-1;
  const row={phase,path:pathname,range:request.headers.range??null,start,length:end-start+1,at:Date.now()};requests.push(row);active++;peak=Math.max(peak,active);
  response.once('close',()=>{active--;row.closedAt=Date.now();});
  response.writeHead(range?206:200,{'content-type':'font/otf','content-length':end-start+1,...(range?{'content-range':`bytes ${start}-${end}/${font.length}`}:{})});
  if(phase==='shared-fill'&&range&&start===0){response.write(font.subarray(start,start+8192));const finish=()=>{held.delete(finish);if(!response.destroyed)response.end(font.subarray(start+8192,end+1));};held.add(finish);return;}
  response.end(font.subarray(start,end+1));
 });
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const port=server.address().port,origin='http://127.0.0.1:'+port;
 browser=await chromium.launch({executablePath:process.env.MIR2_CHROME_BIN,headless:true});proof.browserVersion=browser.version();
 const configure=page=>page.evaluate(async port=>{
  await navigator.serviceWorker.register('/mir2-asset-worker.js');await navigator.serviceWorker.ready;
  if(!navigator.serviceWorker.controller)await new Promise(resolve=>navigator.serviceWorker.addEventListener('controllerchange',resolve,{once:true}));
  await new Promise((resolve,reject)=>{const timer=setTimeout(()=>reject(Error('Worker configuration timeout')),5000);const listener=event=>{if(event.data?.type!=='MIR2_ASSET_CACHE_CONFIGURED')return;clearTimeout(timer);navigator.serviceWorker.removeEventListener('message',listener);resolve();};navigator.serviceWorker.addEventListener('message',listener);
   navigator.serviceWorker.controller.postMessage({type:'MIR2_ASSET_CACHE_CONFIG',manifestVersion:'current',cacheNamespace:'qr-font-fixture',manifest:{version:'current',remoteAssets:{enabled:true,assetBaseUrl:'http://localhost:'+port+'/mir2/v/current'},resourcePacks:[{cacheTier:'critical',urls:['/original-ui/fonts/NotoSansCJKsc-Regular.otf']}]}});
  });window.__mir2AssetCache={enabled:true,status:'registered'};
 },port);
 const contexts=[];const errors=[],unexpected=[];
 const open=async()=>{const context=await browser.newContext({viewport:{width:780,height:560},serviceWorkers:'allow'});contexts.push(context);context.on('request',request=>{const hostname=new URL(request.url()).hostname;if(!['127.0.0.1','localhost'].includes(hostname))unexpected.push(hostname);});const page=await context.newPage();page.on('pageerror',error=>errors.push(error.message));await page.goto(origin);await configure(page);return{page,context};};
 const first=await open();
 await first.page.evaluate(async()=>{window.fixture=await import('/fixture.js');window.stopPage=fixture.startPage();window.stopImage=fixture.startImage();});
 await first.page.waitForFunction(()=>window.events.some(e=>e.name==='walletQrFontPreparationStart'));
 await first.page.screenshot({path:join(output,'qr-visible-before-font-completes.png')});
 assert(held.size===1);const requestsBeforeJoin=requests.length;
 const started=await first.page.evaluate(()=>{const ready=window.events.find(e=>e.name==='image-ready');fixture.retireImage(window.stopImage);window.gameRead=fixture.readForGame();return{ready,events:window.events,qrHidden:document.querySelector('#qr-panel').hidden};});
 assert(started.ready.decoded&&started.qrHidden);for(const finish of [...held])finish();
 const result=await first.page.evaluate(()=>window.gameRead);assert.equal(result.completed.length,1);assert.equal(result.bytes,pin.bytes);
 const cached=await first.page.evaluate(pin=>window.fixture.verifyFont(pin),pin);
 assert.deepEqual(cached,{bytes:pin.bytes,sha256:pin.sha256,checkedParts:63,fontFaceLoaded:true});
 const firstEvents=await first.page.evaluate(()=>{window.stopPage();return window.events;});
 assert(!firstEvents.some(e=>e.name==='walletQrFontPreparationComplete'),'retired QR reader must not publish a late success');
 const firstRequests=requests.filter(row=>row.phase==='shared-fill');assert.equal(firstRequests.length,16);assert(firstRequests.every(row=>row.range));assert.equal(firstRequests.reduce((sum,row)=>sum+row.length,0),pin.bytes);assert(peak<=2);
 await first.page.screenshot({path:join(output,'shared-font-complete.png')});
 await first.context.close();proof.shared={passed:true,requestsBeforeJoin,totalRangeRequests:firstRequests.length,peak,started,result,cached,events:firstEvents};
 phase='cancel-before-confirmation';const second=await open();
 await second.page.evaluate(async()=>{window.__mir2AssetCache.status='registered-unconfirmed';window.fixture=await import('/fixture.js');window.stopPage=fixture.startPage();window.stopImage=fixture.startImage();});
 await second.page.waitForFunction(()=>window.events.some(e=>e.name==='image-ready'));
 await second.page.evaluate(()=>{fixture.retireImage(window.stopImage);window.__mir2AssetCache.status='registered';});
 await second.page.waitForTimeout(250);const cancelEvents=await second.page.evaluate(()=>{window.stopPage();return window.events;});
 assert(!cancelEvents.some(e=>e.name==='walletQrFontPreparationStart'));assert.equal(requests.filter(row=>row.phase==='cancel-before-confirmation').length,0);
 await second.context.close();proof.cancelBeforeConfirmation={passed:true,events:cancelEvents,upstreamFontRequests:0};
 assert.deepEqual(errors,[]);assert.deepEqual(unexpected,[]);proof.pageErrors=errors;proof.unexpectedNetworkOrigins=unexpected;proof.passed=true;
}catch(error){proof.failure={name:error.name,message:error.message};process.exitCode=1;}
finally{
 for(const finish of [...held])finish();await browser?.close();proof.closed.browser=!browser?.isConnected();
 if(server?.listening){server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}proof.closed.server=!server?.listening;
 await writeFile(join(output,'receipt.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({passed:proof.passed,failure:proof.failure,rangeRequests:proof.shared?.totalRangeRequests,checkedParts:proof.shared?.cached.checkedParts,closed:proof.closed,receipt:join(output,'receipt.json')}));
}
