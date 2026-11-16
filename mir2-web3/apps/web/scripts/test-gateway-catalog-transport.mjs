import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {gzipSync} from 'node:zlib';
import test from 'node:test';
import ts from 'typescript';

const root=new URL('../',import.meta.url);
function loadTs(file,imports={}){
 const source=readFileSync(new URL(file,root),'utf8');
 const js=ts.transpileModule(source,{compilerOptions:{module:ts.ModuleKind.CommonJS,target:ts.ScriptTarget.ES2022}}).outputText;
 const module={exports:{}};new Function('exports','require',js)(module.exports,name=>{assert(name in imports,name);return imports[name];});return module.exports;
}
const transport=loadTs('lib/gateway-catalog-transport.ts');
const {createGatewayCatalogDecoder,createGatewayCatalogIngress,GATEWAY_CATALOG_CAPABILITY,MAX_CATALOG_WIRE_BYTES}=transport;
const {gatewayCatalogDescriptor}=loadTs('lib/gateway-catalog-runtime.ts',{'./gateway-catalog-transport':transport});
const manifest=JSON.parse(readFileSync(new URL('lib/generated/client_core_runtime.json',root)));
assert(manifest.catalogTransport,'build the shared catalog policy before this real-WASM suite');
const leaf=new URL(`public/client-core/${manifest.catalogTransport.version}/`,root);
const glue=readFileSync(new URL('mir2_platform_web.js',leaf));
const wasmBytes=readFileSync(new URL('mir2_platform_web_bg.wasm',leaf));
const wasm=await import('data:text/javascript;base64,'+glue.toString('base64'));
await wasm.default({module_or_path:wasmBytes});
const decoder=createGatewayCatalogDecoder(wasm);
const snapshot=text=>{const bytes=batch([text]);bytes.write('M2WSPGZ1');return bytes;};
const info={index:7,name:'药品 Potion',item_type:0,grade:0,required_type:0,required_class:0,required_gender:0,item_set:0,shape:0,weight:1,light:0,required_amount:0,image:1,durability:0,stack_size:99,price:1,start_item:false,effect:0,need_identify:false,show_group_pickup:false,class_based:false,level_based:false,can_mine:false,global_drop_notify:false,bind:0,unique:0,random_stats_id:0,can_fast_run:false,can_awakening:false,slots:0,stats:[],tooltip:null};
const item=JSON.stringify({type:'packet',packet:'NewItemInfo',payload:{info}});
const exact=' \n'+item.replace('"index":7','"future_i64":9223372036854775807,"index":7')+'\t ';
function rawBatch(body,count=1,length=body.length){const h=Buffer.alloc(16);h.write('M2CATGZ1');h.writeUInt32LE(length,8);h.writeUInt32LE(count,12);return Buffer.concat([h,gzipSync(body)]);}
function batch(texts){return rawBatch(Buffer.concat(texts.map(text=>{const b=Buffer.from(text),n=Buffer.alloc(4);n.writeUInt32LE(b.length);return Buffer.concat([n,b]);})),texts.length);}
const arrayBuffer=b=>b.buffer.slice(b.byteOffset,b.byteOffset+b.byteLength);

test('actual isolated Rust WASM preserves original catalog text under unchanged package budgets',()=>{
 assert.equal(wasm.gateway_catalog_abi_version(),1);
 assert(wasmBytes.length<262144);assert.equal(wasmBytes.length,manifest.catalogTransport.files['mir2_platform_web_bg.wasm'].bytes);
 assert.deepEqual(decoder.decode(batch([exact,item])),[exact,item]);
 const names=WebAssembly.Module.exports(new WebAssembly.Module(wasmBytes)).map(x=>x.name);
 assert(!names.some(n=>/^(?:client_core_abi_version|client_presentation_abi_version|npc_purchase_receipt_abi_version|npcpurchasereceiptbridge_)/.test(n)));
 for(const sibling of [manifest,manifest.presentation,manifest.npcPurchase]){
  const bytes=readFileSync(new URL(`public/client-core/${sibling.version}/mir2_platform_web_bg.wasm`,root));
  assert(!WebAssembly.Module.exports(new WebAssembly.Module(bytes)).some(x=>x.name==='decode_gateway_catalog'));
 }
});

test('actual Rust codec rejects malformed gzip and a noncatalog second envelope before returning any text',()=>{
 const good=batch([item]),crc=Buffer.from(good),size=Buffer.from(good);crc[crc.length-8]^=1;size[size.length-4]^=1;
 const bombs=rawBatch(Buffer.alloc(1024*1024,65),1,128*1024);
 for(const bytes of [crc,size,good.subarray(0,-1),Buffer.concat([good,Buffer.from([0])]),Buffer.concat([good,gzipSync('')]),bombs,
  batch([item,'{"type":"identitySession","token":"private-must-not-leak"}']),batch([item,'{"type":"packet","packet":"NewItemInfo","payload":{"info":{}}}'])]){
  assert.throws(()=>decoder.decode(bytes),e=>e.message==='Invalid bounded gateway catalog batch');
 }
});

test('wire limit is checked before the wasm-bindgen input copy',()=>{
 let called=0;const bound=createGatewayCatalogDecoder({gateway_catalog_abi_version:()=>1,decode_gateway_catalog:()=>{called++;return '[]';}});
 assert.throws(()=>bound.decode(new Uint8Array(MAX_CATALOG_WIRE_BYTES+1)));assert.equal(called,0);
 assert.throws(()=>createGatewayCatalogDecoder({gateway_catalog_abi_version:()=>0,decode_gateway_catalog:()=>''}),/ABI/);
});

test('real shared WASM snapshot capability preserves all original UTF-8 text without widening catalog validation',()=>{
 assert.equal(wasm.gateway_world_snapshot_abi_version(),1);
 assert.deepEqual(decoder.capabilities,[GATEWAY_CATALOG_CAPABILITY,transport.GATEWAY_WORLD_SNAPSHOT_CAPABILITY]);
 const text=' \n{"type":"worldSnapshot","payload":{"hp":18,"name":"阿洛","future":18446744073709551615,"entities":[{"id":7}]}}\t';
 assert.deepEqual(decoder.decode(snapshot(text)),[text]);
 assert.throws(()=>wasm.decode_gateway_catalog(snapshot(text)));
 assert.throws(()=>wasm.decode_gateway_world_snapshot(batch([item])));
});

test('snapshot domain rejects identities, receipts, duplicate tags, nonobjects and malformed frames before delivery',()=>{
 for(const text of ['{"type":"identitySession","token":"private"}','{"type":"npcPurchaseOwner","payload":{}}',
  '{"type":"worldSnapshot","payload":[]}','{"type":"worldSnapshot","type":"worldSnapshot","payload":{}}',
  '{"type":"worldSnapshot","payload":{},"secret":"private"}']){
  assert.throws(()=>decoder.decode(snapshot(text)),e=>e.message==='Invalid bounded gateway catalog batch');
 }
 const good=snapshot('{"type":"worldSnapshot","payload":{}}'),badCount=Buffer.from(good),badCrc=Buffer.from(good);
 badCount.writeUInt32LE(2,12);badCrc[badCrc.length-8]^=1;
 for(const bytes of [badCount,badCrc,good.subarray(0,-1),Buffer.concat([good,Buffer.from([0])])])assert.throws(()=>decoder.decode(bytes));
});

test('legacy immutable catalog modules remain catalog-only and partial snapshot ABIs are rejected',()=>{
 const legacy=createGatewayCatalogDecoder({gateway_catalog_abi_version:wasm.gateway_catalog_abi_version,decode_gateway_catalog:wasm.decode_gateway_catalog});
 assert.deepEqual(legacy.capabilities,[GATEWAY_CATALOG_CAPABILITY]);
 assert.deepEqual(legacy.decode(batch([item])),[item]);
 assert.throws(()=>legacy.decode(snapshot('{"type":"worldSnapshot","payload":{}}')));
 for(const extra of [{gateway_world_snapshot_abi_version:()=>1},{decode_gateway_world_snapshot:()=>''},
  {gateway_world_snapshot_abi_version:()=>2,decode_gateway_world_snapshot:()=>''}]){
  assert.throws(()=>createGatewayCatalogDecoder({gateway_catalog_abi_version:()=>1,decode_gateway_catalog:()=>'',...extra}),/snapshot ABI/);
 }
});

test('physical connection retains its declared decoder and orders Text, snapshot and catalog synchronously',()=>{
 const text='{"type":"worldSnapshot","payload":{"hp":18}}',seen=[];
 const ingress=createGatewayCatalogIngress({current:()=>true,deliver:x=>seen.push(x),reject:()=>assert.fail('valid mixed frames')});
 assert.deepEqual(ingress.capabilitiesFor(null),[]);
 assert.deepEqual(ingress.capabilitiesFor(decoder),decoder.capabilities);
 ingress.declare(decoder);ingress.receive('before');ingress.receive(arrayBuffer(snapshot(text)));ingress.receive(arrayBuffer(batch([item])));ingress.receive('after');
 assert.deepEqual(seen,['before',text,item,'after']);
 const legacy=createGatewayCatalogDecoder({gateway_catalog_abi_version:wasm.gateway_catalog_abi_version,decode_gateway_catalog:wasm.decode_gateway_catalog});
 const pinned=createGatewayCatalogIngress({current:()=>true,deliver:()=>{},reject:()=>{}});
 pinned.declare(legacy);pinned.declare(decoder);
 assert.deepEqual(pinned.capabilitiesFor(decoder),[GATEWAY_CATALOG_CAPABILITY]);
});

test('mixed Text and real binary catalogs remain synchronous and in physical-frame order',()=>{
 const seen=[],ingress=createGatewayCatalogIngress({current:()=>true,deliver:x=>seen.push(x),reject:()=>assert.fail('valid binary')});
 ingress.declare(decoder);ingress.receive('before');ingress.receive(arrayBuffer(batch([exact,item])));ingress.receive('after');
 assert.deepEqual(seen,['before',exact,item,'after']);assert.equal(ingress.declared(),true);
});

test('invalid or unnegotiated binary poisons only that physical connection and never partially applies a batch',()=>{
 for(const [negotiate,bytes] of [[false,batch([item])],[true,batch([item,'{"type":"packet","packet":"UserLocation","payload":{}}'])],[true,Buffer.alloc(MAX_CATALOG_WIRE_BYTES+1)]]){
  const seen=[];let rejected=0;const ingress=createGatewayCatalogIngress({current:()=>true,deliver:x=>seen.push(x),reject:()=>rejected++});
  if(negotiate)ingress.declare(decoder);ingress.receive(arrayBuffer(bytes));ingress.receive('later');
  assert.deepEqual(seen,[]);assert.equal(rejected,1);
 }
});

test('retired connection and mid-batch replacement cannot deliver stale catalog entries',()=>{
 let active=true;const seen=[];const ingress=createGatewayCatalogIngress({current:()=>active,deliver:x=>{seen.push(x);active=false;},reject:()=>assert.fail('retired')});
 ingress.declare(decoder);ingress.receive(arrayBuffer(batch([exact,item])));ingress.receive('later');assert.deepEqual(seen,[exact]);
 let decoded=0;const retired=createGatewayCatalogIngress({current:()=>false,deliver:()=>assert.fail('retired'),reject:()=>assert.fail('retired')});
 retired.declare({decode(){decoded++;return[item];}});retired.receive(arrayBuffer(batch([item])));assert.equal(decoded,0);
});

const pageSource=readFileSync(new URL('app/page.tsx',root),'utf8'),ast=ts.createSourceFile('page.tsx',pageSource,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
function pageFunction(name,env){let node;const visit=n=>{if(ts.isFunctionDeclaration(n)&&n.name?.text===name)node=n;ts.forEachChild(n,visit);};visit(ast);assert(node,name);
 const js=ts.transpileModule(node.getText(ast),{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
 return new Function(...Object.keys(env),js+'\nreturn '+name+';')(...Object.values(env));
}
function pageHarness(){
 const seen=[],closed=[],messages=[],hero=[],channels={current:new WeakMap()},connection={current:1},socket={readyState:1,send:x=>messages.push(JSON.parse(x)),close:(code,reason)=>{closed.push({code,reason});socket.readyState=2;}};
 const env={gatewayCatalogChannelsRef:channels,equipmentConnectionGenerationRef:connection,socketRef:{current:socket},WebSocket:{OPEN:1},createGatewayCatalogIngress,
  receivePrivateCompanionFrame:()=>false,performance:{now:()=>1},syncHeroRawIngress:()=>{},currentHeroRawPhysical:()=>({socket}),
  heroRawIngressRef:{current:{capture:(data)=>{hero.push(data);return null;},reject:()=>{}}},receiveNpcPurchaseGatewayFrame:event=>seen.push(event.data),
  gatewayProtocolReadyRef:{current:true},appendLog:()=>{},t:x=>x,
  npcPurchaseOptInSocketsRef:{current:new WeakSet()},cashReceiptSocketRef:{current:null},questCoreRuntimeRef:{current:null},
  GATEWAY_CATALOG_CAPABILITY,NATIVE_ATTACK_MOVEMENT_CAPABILITY:'nativeAttackOutcomeV2',nativeAttackMovementChannel:()=>({declare:()=>{}})};
 env.gatewayCatalogChannel=pageFunction('gatewayCatalogChannel',env);
 const declare=pageFunction('sendClientCapabilities',env);
 return{env,declare,socket,seen,closed,messages,hero};
}

test('actual Page capability replacement retains catalog with receipts and attack declarations',()=>{
 const h=pageHarness();h.declare(h.socket,'nativeGameShopReceiptV1');assert(!h.messages.at(-1).capabilities.includes(GATEWAY_CATALOG_CAPABILITY));
 h.env.questCoreRuntimeRef.current={gatewayCatalog:decoder};h.declare(h.socket,'nativeGameShopReceiptV1');
 assert(h.messages.at(-1).capabilities.includes(GATEWAY_CATALOG_CAPABILITY));
 h.env.questCoreRuntimeRef.current=null;h.env.cashReceiptSocketRef.current=h.socket;h.env.npcPurchaseOptInSocketsRef.current.add(h.socket);
 h.declare(h.socket,'durableNpcPurchaseOwnerV1');
 assert.deepEqual(new Set(h.messages.at(-1).capabilities),new Set(['durableNpcPurchaseOwnerV1','nativeGameShopReceiptV1','nativeAttackOutcomeV2',GATEWAY_CATALOG_CAPABILITY,transport.GATEWAY_WORLD_SNAPSHOT_CAPABILITY]));
 h.env.gatewayCatalogChannel(h.socket).receive(arrayBuffer(batch([exact,item])));assert.deepEqual(h.seen,[exact,item]);assert.deepEqual(h.hero,[exact,item]);
});

test('actual Page does not negotiate after failed send or carry a decoder onto a replacement socket',()=>{
 const h=pageHarness();h.env.questCoreRuntimeRef.current={gatewayCatalog:decoder};const original=h.socket.send;
 h.socket.send=()=>{throw Error('closed');};assert.throws(()=>h.declare(h.socket,'nativeGameShopReceiptV1'));
 assert.equal(h.env.gatewayCatalogChannel(h.socket).declared(),false);
 h.socket.send=original;h.declare(h.socket,'nativeGameShopReceiptV1');
 const next={readyState:1,send:x=>h.messages.push(JSON.parse(x)),close:()=>{}};h.env.socketRef.current=next;h.env.equipmentConnectionGenerationRef.current++;
 h.env.questCoreRuntimeRef.current=null;h.declare(next,'nativeGameShopReceiptV1');assert(!h.messages.at(-1).capabilities.includes(GATEWAY_CATALOG_CAPABILITY));
 h.env.gatewayCatalogChannel(h.socket).receive(arrayBuffer(batch([item])));assert.deepEqual(h.seen,[]);
});

test('actual Page closes invalid binary, fences later Text and preserves the ordinary hero/raw handler',()=>{
 const h=pageHarness();h.env.questCoreRuntimeRef.current={gatewayCatalog:decoder};h.declare(h.socket,'nativeGameShopReceiptV1');
 const channel=h.env.gatewayCatalogChannel(h.socket);channel.receive('plain');channel.receive(arrayBuffer(batch([item,'bad-private'])));channel.receive('later');
 assert.deepEqual(h.seen,['plain']);assert.equal(h.closed[0].code,4002);assert.equal(h.env.gatewayProtocolReadyRef.current,false);
 const connect=pageFunction('connectGateway',{resolveGatewayWebSocketUrl:()=>''});assert.equal(typeof connect,'function');
 const connectNode=pageSource.slice(pageSource.indexOf('  function connectGateway('),pageSource.indexOf('  function connectGateway(')+5000);
 assert(connectNode.indexOf('socket.binaryType = "arraybuffer"')<connectNode.indexOf('socket.addEventListener("open"'));
});

test('catalog descriptor shares source identity and original independent policy budget',()=>{
 const siblings=[manifest.version,manifest.presentation.version,manifest.npcPurchase.version];
 assert.deepEqual(gatewayCatalogDescriptor(manifest.catalogTransport,manifest.sourceSha256,siblings),manifest.catalogTransport);
 assert.equal(gatewayCatalogDescriptor(undefined,manifest.sourceSha256,siblings),null);
 for(const change of [x=>{x.extra=1;},x=>{x.abiVersion=0;},x=>{x.version=manifest.version;},x=>{x.sourceSha256='0'.repeat(64);},x=>{x.files['mir2_platform_web_bg.wasm'].bytes=262144;}]){
  const value=structuredClone(manifest.catalogTransport);change(value);assert.throws(()=>gatewayCatalogDescriptor(value,manifest.sourceSha256,siblings));
 }
});
