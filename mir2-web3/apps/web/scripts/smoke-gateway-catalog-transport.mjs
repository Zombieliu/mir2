// Pure loopback transport QA: actual Chrome, actual shared WASM, no game account.
import assert from 'node:assert/strict';
import {readFile,mkdir,writeFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {createRequire} from 'node:module';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {gzipSync} from 'node:zlib';
import ts from 'typescript';
const require=createRequire(import.meta.url),{WebSocketServer}=require('next/dist/compiled/ws');
const output=resolve(process.argv[2]??'');assert(process.argv[2]&&process.argv.length===3,'pass a fresh evidence directory');
const {chromium}=createRequire(resolve(process.env.MIR2_PLAYWRIGHT_PACKAGE??'package.json'))('playwright');
assert(process.env.MIR2_CHROME_BIN,'set MIR2_CHROME_BIN to the actual Chrome executable');
await mkdir(output,{recursive:false});
const web=new URL('../',import.meta.url),manifest=JSON.parse(await readFile(new URL('lib/generated/client_core_runtime.json',web)));
assert(manifest.catalogTransport);const leaf=new URL(`public/client-core/${manifest.catalogTransport.version}/`,web);
const source=await readFile(new URL('lib/gateway-catalog-transport.ts',web),'utf8');
const js=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.ESNext,target:ts.ScriptTarget.ES2022}}).outputText;
const [glue,wasm]=await Promise.all(['mir2_platform_web.js','mir2_platform_web_bg.wasm'].map(p=>readFile(new URL(p,leaf))));
const hash=b=>createHash('sha256').update(b).digest('hex');
assert.equal(hash(wasm),manifest.catalogTransport.files['mir2_platform_web_bg.wasm'].sha256);
assert.equal(hash(glue),manifest.catalogTransport.files['mir2_platform_web.js'].sha256);
const info={index:7,name:'药品 Potion',item_type:0,grade:0,required_type:0,required_class:0,required_gender:0,item_set:0,shape:0,weight:1,light:0,required_amount:0,image:1,durability:0,stack_size:99,price:1,start_item:false,effect:0,need_identify:false,show_group_pickup:false,class_based:false,level_based:false,can_mine:false,global_drop_notify:false,bind:0,unique:0,random_stats_id:0,can_fast_run:false,can_awakening:false,slots:0,stats:[],tooltip:null};
const item=JSON.stringify({type:'packet',packet:'NewItemInfo',payload:{info}});
const exact=' \n'+item.replace('"index":7','"future_i64":9223372036854775807,"index":7')+'\t ';
function frame(texts){const body=Buffer.concat(texts.map(t=>{const b=Buffer.from(t),n=Buffer.alloc(4);n.writeUInt32LE(b.length);return Buffer.concat([n,b]);}));
 const h=Buffer.alloc(16);h.write('M2CATGZ1');h.writeUInt32LE(body.length,8);h.writeUInt32LE(texts.length,12);return Buffer.concat([h,gzipSync(body)]);}
const valid=frame([exact,item]),invalid=frame([item,'{"type":"identitySession","token":"private-must-not-leak"}']);
const files=new Map([['/transport.js',['application/javascript',Buffer.from(js)]],['/mir2_platform_web.js',['application/javascript',glue]],['/mir2_platform_web_bg.wasm',['application/wasm',wasm]]]);
const server=createServer((req,res)=>{const file=files.get(req.url);res.writeHead(200,{'Content-Type':file?.[0]??'text/html','Cache-Control':'no-store'});res.end(file?.[1]??'<!doctype html><title>Catalog transport fixture</title>');});
const sockets=new WebSocketServer({server});let browser,context;const requests=[],errors=[];
const proof={scope:'loopback actual shared WASM/Chrome transport; no actual game, account or production latency claim',passed:false,
 policy:manifest.catalogTransport,sourceSha256:hash(source),closed:{browser:false,server:false,sockets:false}};
try{
 sockets.on('connection',(socket,request)=>socket.on('message',()=>{
  const bad=request.url==='/invalid';socket.send('before');socket.send(bad?invalid:valid);socket.send('after');
 }));
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const origin='http://127.0.0.1:'+server.address().port;
 browser=await chromium.launch({executablePath:process.env.MIR2_CHROME_BIN,headless:true});proof.browserVersion=browser.version();
 context=await browser.newContext();context.on('request',r=>requests.push(new URL(r.url()).origin));
 const page=await context.newPage();page.on('pageerror',e=>errors.push(e.message));await page.goto(origin);
 proof.result=await page.evaluate(async({origin,exact,item,validBytes,invalidBytes})=>{
  const policy=await import(origin+'/transport.js'),module=await import(origin+'/mir2_platform_web.js');
  await module.default({module_or_path:new URL(origin+'/mir2_platform_web_bg.wasm')});
  const decoder=policy.createGatewayCatalogDecoder(module),asBuffer=a=>new Uint8Array(a).buffer;
  const direct=decoder.decode(new Uint8Array(validBytes));
  if(JSON.stringify(direct)!==JSON.stringify([exact,item]))throw Error('original text changed');
  const bads=[invalidBytes,validBytes.slice(0,-1),[...validBytes,0]];
  const badCrc=[...validBytes];badCrc[badCrc.length-8]^=1;bads.push(badCrc);
  let rejected=0;for(const bytes of bads){try{decoder.decode(new Uint8Array(bytes));throw Error('accepted invalid batch');}catch(e){if(e.message!=='Invalid bounded gateway catalog batch')throw e;rejected++;}}
  const socketCase=(path,declare,retire)=>new Promise((resolve,reject)=>{
   const socket=new WebSocket(origin.replace('http','ws')+path);socket.binaryType='arraybuffer';let current=true,failed=0;const seen=[];
   const ingress=policy.createGatewayCatalogIngress({current:()=>current,deliver:text=>{seen.push(text);if(retire&&text===exact)current=false;},reject:()=>{failed++;socket.close(4002,'Invalid catalog frame');}});
   const done=()=>{socket.close();resolve({seen,failed});};const timer=setTimeout(()=>{socket.close();reject(Error('transport fixture timed out'));},5000);
   socket.onopen=()=>{if(declare)ingress.declare(decoder);socket.send('begin');};
   socket.onmessage=e=>{ingress.receive(e.data);if(e.data==='after'&&!failed){clearTimeout(timer);done();}};
   socket.onerror=()=>{clearTimeout(timer);reject(Error('loopback socket failed'));};
   socket.onclose=e=>{if(failed){clearTimeout(timer);resolve({seen,failed,closeCode:e.code});}};
  });
  const ordered=await socketCase('/valid',true,false),bad=await socketCase('/invalid',true,false),unnegotiated=await socketCase('/valid',false,false),retired=await socketCase('/valid',true,true);
  if(JSON.stringify(ordered.seen)!==JSON.stringify(['before',exact,item,'after'])||ordered.failed)throw Error('mixed order changed');
  for(const row of [bad,unnegotiated])if(JSON.stringify(row.seen)!=='["before"]'||row.failed!==1||row.closeCode!==4002)throw Error('invalid batch was partially applied or not closed');
  if(JSON.stringify(retired.seen)!==JSON.stringify(['before',exact])||retired.failed)throw Error('retired callback leaked');
  let copied=0;const bound=policy.createGatewayCatalogDecoder({gateway_catalog_abi_version:()=>1,decode_gateway_catalog:()=>{copied++;return '[]';}});
  try{bound.decode(new Uint8Array(policy.MAX_CATALOG_WIRE_BYTES+1));}catch{}if(copied)throw Error('oversize copied to WASM');
  return{passed:true,directExactText:true,malformedBatchesRejected:rejected,mixedOrder:true,noPartialApply:true,unnegotiatedRejected:true,retiredGenerationFenced:true,oversizeBeforeCopy:true,
   cases:[ordered,bad,unnegotiated,retired].map(x=>({delivered:x.seen.length,rejected:x.failed}))};
 },{origin,exact,item,validBytes:[...valid],invalidBytes:[...invalid]});
 assert(requests.every(url=>url===origin));assert.deepEqual(errors,[]);proof.loopbackRequests=requests.length;proof.browserErrors=errors;proof.passed=true;
}catch(error){proof.failure=error.message;throw error;}
finally{
 if(browser){await browser.close();proof.closed.browser=true;}
 for(const socket of sockets.clients)socket.terminate();await new Promise(resolve=>sockets.close(resolve));proof.closed.sockets=true;
 await new Promise(resolve=>server.close(resolve));proof.closed.server=true;
 await writeFile(join(output,'receipt.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
 console.log(JSON.stringify({passed:proof.passed,closed:proof.closed,receipt:join(output,'receipt.json')}));
}
