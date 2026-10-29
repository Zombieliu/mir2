// Public deterministic vectors only; never use these keys for deployed assets.
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import {mkdtempSync,writeFileSync,readFileSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join,resolve,sep} from 'node:path';
import { Ed25519Keypair } from '@mysten/sui/keypairs/ed25519';
import { Transaction } from '@mysten/sui/transactions';
import { COIN_TYPE,GENESIS_DIGEST,NETWORK_HASH,validateConfig,validateOrder,decodeInvoice,decodeEvent,integer } from '../service/model.mjs';
import { buildPay,observeCreateWithClient,observeInvoiceWithClient,reserveIssuerGas,submitPrepared,validateGasEnvironment,validatePrepared,verifyWalletProof } from '../service/driver.mjs';

const h=n=>'0x'+n.repeat(64),d='11111111111111111111111111111111';
const key=Ed25519Keypair.fromSecretKey(new Uint8Array(32).fill(42));
const c={enabled:true,environmentTag:'isolated-test',network:'testnet',chainIdentifier:'4c78adac',networkHash:NETWORK_HASH,
  fullnodeUrl:'https://fullnode.testnet.sui.io:443',packageId:h('1'),configId:h('2'),adminCapId:h('3'),issuerAddress:key.toSuiAddress(),merchantAddress:h('4'),coinType:COIN_TYPE,gasBudgetMist:'50000000'};
const order={orderRef:h('5'),accountRef:h('6'),snapshotHash:h('7'),payer:h('8'),merchant:h('4'),amountMicros:'1234567',expiresAtMs:'20000'};
const invoiceId=h('9'),digest='HLXrxYBPxWicch7jh4VeQANZMs1v9tGLbh1ib4pqvYyc';
const scalar=n=>{const b=Buffer.alloc(8);b.writeBigUInt64LE(BigInt(n));return b;};
const addr=v=>Buffer.from(v.slice(2),'hex');
const vec=v=>{const b=typeof v==='string'?Buffer.from(v):v;return Buffer.concat([Buffer.from([b.length]),b]);};
const common=(o=order)=>Buffer.concat([addr(c.configId),vec(addr(c.networkHash)),...['orderRef','accountRef','snapshotHash'].map(k=>vec(addr(o[k]))),addr(o.payer),addr(o.merchant),vec(c.coinType.slice(2)),scalar(o.amountMicros),scalar(o.expiresAtMs)]);
const invoiceBytes=(paid=true,o=order)=>Buffer.concat([addr(invoiceId),common(o),Buffer.from([paid?1:0]),scalar(paid?'10000':'0')]);
const eventBytes=(paid=true,o=order)=>Buffer.concat([addr(invoiceId),common(o),...(paid?[scalar('10000')]:[addr(c.issuerAddress),scalar('5000')])]);
function fixture(paid=true){
  const object={objectId:invoiceId,type:c.packageId+'::recharge::Invoice',version:'4',digest:d,previousTransaction:digest,
    owner:{$kind:'Shared',Shared:{initialSharedVersion:'2'}},content:invoiceBytes(paid)};
  const event={eventType:c.packageId+'::recharge::'+(paid?'PaymentReceived':'InvoiceCreated'),packageId:c.packageId,module:'recharge',sender:paid?order.payer:c.issuerAddress,contents:{value:eventBytes(paid)}};
  const tx={digest,checkpoint:12n,transaction:{sender:event.sender},effects:{status:{success:true},changedObjects:[{objectId:invoiceId,objectType:object.type,outputState:2,idOperation:paid?0:2,outputVersion:4n,outputDigest:d}]},events:{events:[event]}};
  const client={core:{getObject:async()=>({object})},ledgerService:{getTransaction:async()=>({response:{transaction:tx}})}};
  return {object,event,tx,client};
}

test('configuration only permits the exact official testnet/native USDC domain',()=>{
  assert.equal(validateConfig(c),c);
  for(const patch of [{network:'mainnet'},{chainIdentifier:'35834a8a'},{coinType:'0x'+ 'a'.repeat(64)+'::usdc::USDC'},
    {fullnodeUrl:'https://fullnode.testnet.sui.io.evil.invalid'},{fullnodeUrl:'https://user@fullnode.testnet.sui.io:443'},
    {fullnodeUrl:'http://127.0.0.1:9000'},{environmentTag:'production'},{gasBudgetMist:'50000001'},{enabled:false}]){
    assert.throws(()=>validateConfig({...c,...patch}));
  }
});
test('monetary values preserve u64 precision and reject floating point/ambiguous decimals',()=>{
  assert.equal(integer('18446744073709551615'),(1n<<64n)-1n);
  for(const v of [1234567,'1.2','01','-1','18446744073709551616'])assert.throws(()=>integer(v));
  assert.throws(()=>validateOrder({...order,amountMicros:'0'},c));
  assert.throws(()=>validateOrder({...order,merchant:h('a')},c));
});
test('BCS invoice and payment event have a frozen field order',()=>{
  const actual=decodeInvoice(invoiceBytes());
  assert.deepEqual(actual,{invoiceId,configId:c.configId,networkHash:NETWORK_HASH,...order,coinType:COIN_TYPE,paid:true,paidAtMs:'10000'});
  assert.equal(decodeEvent(eventBytes(),'paid').paidAtMs,'10000');
  assert.throws(()=>decodeInvoice(Buffer.concat([invoiceBytes(),Buffer.from([0])])));
  const invalid=invoiceBytes();invalid[invalid.length-9]=2;assert.throws(()=>decodeInvoice(invalid));
  assert.throws(()=>decodeInvoice(invoiceBytes().subarray(1)));
});
test('verified paid receipt binds fullnode transaction, sender, BCS event and invoice object',async()=>{
  const f=fixture();const result=await observeInvoiceWithClient(f.client,c,{invoiceId,order});
  assert.equal(result.state,'paid');assert.equal(result.receipt.transactionDigest,digest);
  assert.equal(result.receipt.amountMicros,order.amountMicros);assert.equal(result.receipt.eventSequence,'0');
  assert.equal(result.receipt.invoiceId,invoiceId);assert.equal(result.receipt.checkpoint,'12');
});
test('unpaid invoice verifies creation and never grants credits',async()=>{
  const f=fixture(false),p={invoiceId,order};assert.equal((await observeInvoiceWithClient(f.client,c,p)).state,'unpaid');
  assert.equal((await observeCreateWithClient(f.client,c,{order,prepared:{digest}})).invoiceId,invoiceId);
});
test('not finalized, missing transaction or missing object remains unknown',async()=>{
  for(const change of [f=>{delete f.tx.checkpoint;},f=>{f.client.ledgerService.getTransaction=async()=>({response:{}});},
    f=>{f.client.core.getObject=async()=>{throw {code:'NOT_FOUND'};};}]){
    const f=fixture();change(f);assert.equal((await observeInvoiceWithClient(f.client,c,{invoiceId,order})).state,'unknown');
  }
});
test('failed create is explicit only after a checkpoint, without issuing a new invoice',async()=>{
  const f=fixture(false);f.tx.effects.status.success=false;
  assert.equal((await observeCreateWithClient(f.client,c,{order,prepared:{digest}})).state,'failed');
});
test('payment receipt rejects forged sender, foreign package, wrong event type and duplicate events',async()=>{
  for(const change of [f=>{f.tx.transaction.sender=h('a');},f=>{f.event.sender=h('b');},f=>{f.event.packageId=h('c');},
    f=>{f.event.eventType=h('d')+'::recharge::PaymentReceived';},f=>{f.tx.events.events.push(f.event);}]){
    const f=fixture();change(f);await assert.rejects(()=>observeInvoiceWithClient(f.client,c,{invoiceId,order}));
  }
});
test('payment cannot be replayed for a different order/account/offer/payer/merchant/amount/expiry',async()=>{
  for(const k of Object.keys(order)){
    const f=fixture(),other={...order,[k]:['amountMicros','expiresAtMs'].includes(k)?'99999':h('a')};
    await assert.rejects(()=>observeInvoiceWithClient(f.client,c,{invoiceId,order:other}));
  }
});
test('invoice mutation effects and payment timestamp must match the returned object',async()=>{
  for(const change of [f=>{f.tx.effects.changedObjects[0].outputDigest='wrong';},f=>{f.tx.effects.changedObjects[0].outputVersion=5n;},
    f=>{f.event.contents.value=Buffer.concat([eventBytes().subarray(0,-8),scalar('10001')]);},
    f=>{f.tx.effects.changedObjects=[];},f=>{f.tx.effects.status.success=false;}]){
    const f=fixture();change(f);await assert.rejects(()=>observeInvoiceWithClient(f.client,c,{invoiceId,order}));
  }
});
test('a changing fullnode observation does not invent a settlement receipt',async()=>{
  const f=fixture();let calls=0;f.client.core.getObject=async()=>({object:++calls===1?f.object:{...f.object,version:'5'}});
  assert.deepEqual(await observeInvoiceWithClient(f.client,c,{invoiceId,order}),{state:'unknown'});
});
test('wallet proof verifies Sui personal-message signature for exactly this address/message',async()=>{
  const message='MIR2_SUI_USDC_BIND_V1\norder-and-expiry-bound-by-server';const signature=(await key.signPersonalMessage(Buffer.from(message))).signature;
  assert.equal((await verifyWalletProof({message,signature,address:key.toSuiAddress()})).verified,true);
  await assert.rejects(()=>verifyWalletProof({message,signature,address:h('1')}));
  await assert.rejects(()=>verifyWalletProof({message:message+'changed',signature,address:key.toSuiAddress()}));
});
async function prepared(o=order,mutate=()=>{}){
  const tx=new Transaction();tx.setSender(c.issuerAddress);tx.setGasOwner(c.issuerAddress);tx.setGasBudget(c.gasBudgetMist);tx.setGasPrice(1000);
  tx.setGasPayment([{objectId:h('a'),version:'1',digest:d}]);
  tx.moveCall({target:c.packageId+'::recharge::create_invoice',arguments:[
    tx.objectRef({objectId:c.adminCapId,version:'1',digest:d}),
    tx.sharedObjectRef({objectId:c.configId,initialSharedVersion:'1',mutable:false}),
    tx.sharedObjectRef({objectId:'0x6',initialSharedVersion:'1',mutable:false}),
    ...['orderRef','accountRef','snapshotHash'].map(k=>tx.pure.vector('u8',[...addr(o[k])])),
    tx.pure.address(o.payer),tx.pure.u64(o.amountMicros),tx.pure.u64(o.expiresAtMs)]});
  mutate(tx);const raw=await tx.build();return {bytesB64:Buffer.from(raw).toString('base64'),signature:(await key.signTransaction(raw)).signature,digest:await tx.getDigest()};
}
test('exact issuer create bytes/signature bind every issued invoice term',async()=>{
  const p=await prepared();await validatePrepared(c,p,order);
  for(const k of ['orderRef','accountRef','snapshotHash','payer','amountMicros','expiresAtMs']){
    await assert.rejects(()=>validatePrepared(c,p,{...order,[k]:['amountMicros','expiresAtMs'].includes(k)?'99999':h('a')}));
  }
  await assert.rejects(()=>validatePrepared(c,{...p,digest:'H1Gg1DiHTgMFv8ZBtaPydsUHDPtvwW3n16iZrihdmqQd'},order));
  const foreign=await prepared(order,tx=>tx.transferObjects([tx.gas],tx.pure.address(h('b'))));
  await assert.rejects(()=>validatePrepared(c,foreign,order));
});
function buildFixture(){
  const f=fixture(false),cfg={objectId:c.configId,type:c.packageId+'::recharge::Config',version:'1',digest:d,previousTransaction:digest,
    owner:{$kind:'Shared',Shared:{initialSharedVersion:'1'}},content:Buffer.concat([addr(c.configId),addr(c.adminCapId),addr(c.issuerAddress),vec(addr(c.networkHash)),vec(c.coinType.slice(2)),addr(c.merchantAddress),Buffer.from([1])])};
  const cap={objectId:c.adminCapId,type:c.packageId+'::recharge::AdminCap',owner:{$kind:'AddressOwner',AddressOwner:c.issuerAddress},content:Buffer.concat([addr(c.adminCapId),addr(c.issuerAddress)])};
  f.client.core.getChainIdentifier=async()=>({chainIdentifier:GENESIS_DIGEST});
  f.client.core.getObject=async({objectId})=>({object:objectId===c.packageId?{type:'package'}:objectId===c.configId?cfg:objectId===c.adminCapId?cap:f.object});
  f.client.core.listCoins=async()=>({objects:[{objectId:h('b'),version:'2',digest:d,balance:'2000000',type:'0x2::coin::Coin<'+COIN_TYPE+'>',owner:{$kind:'AddressOwner',AddressOwner:order.payer}}],hasNextPage:false,cursor:null});
  return f;
}
test('wallet pay contains exact native USDC split, invoice call and payer change, without issuer gas',async()=>{
  const f=buildFixture(),result=await buildPay(c,{invoiceId,order},{},f.client),tx=Transaction.from(result.transactionJson).getData();
  assert.equal(result.gasPayer,'wallet');assert.equal(tx.sender,order.payer);assert.equal(tx.gasData.owner,null);
  const call=tx.commands.find(v=>v.MoveCall)?.MoveCall;assert.equal(call.function,'pay');assert.deepEqual(call.typeArguments,[COIN_TYPE]);
  assert.equal(tx.commands.length,3);
});
test('coin selection fails closed for insufficient or foreign owned USDC',async()=>{
  const f=buildFixture();f.client.core.listCoins=async()=>({objects:[],hasNextPage:false,cursor:null});
  await assert.rejects(()=>buildPay(c,{invoiceId,order},{},f.client),/usdcInsufficientBalance/);
  const other=buildFixture();other.client.core.listCoins=async()=>({objects:[{balance:'2000000',type:'fake',owner:{$kind:'AddressOwner',AddressOwner:order.payer}}],hasNextPage:false});
  await assert.rejects(()=>buildPay(c,{invoiceId,order},{},other.client),/usdcCoinOwnershipMismatch/);
});
test('CLI returns bounded JSON only and does not expose arbitrary errors or secrets',()=>{
  const cli=new URL('../service/cli.mjs',import.meta.url);
  for(const input of ['{"operation":"bad","config":{},"params":{}}','x'.repeat(1048577)]){
    const r=spawnSync(process.execPath,[cli.pathname.replace(/^\/([A-Za-z]:)/,'$1')],{input,encoding:'utf8'});
    assert.equal(r.status,1);const reply=JSON.parse(r.stdout);assert.equal(reply.ok,false);assert.match(reply.error,/^usdc/);
    assert.equal(r.stderr,'');assert.ok(r.stdout.length<256);
  }
});

function gasFixture(t,maximum='50000000'){
  const dir=mkdtempSync(join(tmpdir(),'mir2-usdc-gas-'));
  t.after(()=>{const target=resolve(dir),parent=resolve(tmpdir());if(!target.startsWith(parent+sep)||!target.split(sep).at(-1).startsWith('mir2-usdc-gas-'))throw Error('unsafe test cleanup');rmSync(target,{recursive:true,force:true});});
  const path=join(dir,'ledger.json'),env={MIR2_SUI_USDC_GAS_LEDGER_PATH:path,MIR2_SUI_USDC_GAS_MAX_MIST:maximum};
  return {path,dir,env};
}
const reservationFixture={digest,bytesB64:Buffer.from('public-test-transaction').toString('base64'),signature:'public-test-signature'};
const digest2='H1Gg1DiHTgMFv8ZBtaPydsUHDPtvwW3n16iZrihdmqQd';
test('issuer gas guard requires an absolute durable path and positive bounded lifetime limit',()=>{
  for(const env of [{},{MIR2_SUI_USDC_GAS_LEDGER_PATH:'relative.json'},
    {MIR2_SUI_USDC_GAS_LEDGER_PATH:resolve('test.json'),MIR2_SUI_USDC_GAS_MAX_MIST:'0'},
    {MIR2_SUI_USDC_GAS_LEDGER_PATH:resolve('test.json'),MIR2_SUI_USDC_GAS_MAX_MIST:'1000000001'},
    {MIR2_SUI_USDC_GAS_LEDGER_PATH:resolve('test.json'),MIR2_SUI_USDC_GAS_MAX_MIST:'-1'}])assert.throws(()=>validateGasEnvironment(env));
});
test('exact digest retries survive a subprocess restart without spending another gas slot',async t=>{
  const f=gasFixture(t);
  assert.equal(reserveIssuerGas(c,reservationFixture,f.env).duplicate,false);
  const module=new URL('../service/driver.mjs',import.meta.url).href;
  const code='const {reserveIssuerGas}=await import(process.argv[1]);const p=JSON.parse(process.argv[2]);console.log(JSON.stringify(reserveIssuerGas(p.c,p.tx,p.env)));';
  const child=spawnSync(process.execPath,['--input-type=module','-e',code,module,JSON.stringify({c,tx:reservationFixture,env:f.env})],{encoding:'utf8'});
  assert.equal(child.status,0,child.stderr);assert.equal(JSON.parse(child.stdout).duplicate,true);
  assert.equal(Object.keys(JSON.parse(readFileSync(f.path,'utf8')).reservations).length,1);
  assert.throws(()=>reserveIssuerGas(c,{...reservationFixture,digest:digest2},f.env),/usdcGasLifetimeBudgetExceeded/);
});
test('lifetime reservations are cumulative, not a per-day or per-process counter',t=>{
  const f=gasFixture(t,'100000000');reserveIssuerGas(c,reservationFixture,f.env);
  assert.equal(reserveIssuerGas(c,{...reservationFixture,digest:digest2},f.env).reservedMist,'100000000');
  assert.throws(()=>reserveIssuerGas(c,{...reservationFixture,digest:'5Mb7wW8P8mb5mHH5Use1gwvBvUD5r52PXMdXwvXcHGE'},f.env),/usdcGasLifetimeBudgetExceeded/);
});
test('a ledger cannot be reused for another domain, issuer, gas budget or widened lifetime cap',t=>{
  const f=gasFixture(t);reserveIssuerGas(c,reservationFixture,f.env);
  for(const patch of [{configId:h('a')},{packageId:h('a')},{issuerAddress:h('a')},{gasBudgetMist:'40000000'}])
    assert.throws(()=>reserveIssuerGas({...c,...patch},reservationFixture,f.env),/usdcGasLedgerDomainMismatch/);
  assert.throws(()=>reserveIssuerGas(c,reservationFixture,{...f.env,MIR2_SUI_USDC_GAS_MAX_MIST:'100000000'}),/usdcGasLedgerDomainMismatch/);
  assert.throws(()=>reserveIssuerGas(c,{...reservationFixture,signature:'changed'},f.env),/usdcGasReservationMismatch/);
});
test('a locked, truncated, corrupt or over-budget ledger fails closed',t=>{
  const f=gasFixture(t);writeFileSync(f.path+'.lock','simulated interrupted worker');
  assert.throws(()=>reserveIssuerGas(c,reservationFixture,f.env),/usdcGasLedgerLocked/);
  rmSync(f.path+'.lock');writeFileSync(f.path,'{');
  assert.throws(()=>reserveIssuerGas(c,reservationFixture,f.env),/usdcGasLedgerInvalid/);
  rmSync(f.path);reserveIssuerGas(c,reservationFixture,f.env);
  const ledger=JSON.parse(readFileSync(f.path,'utf8'));ledger.reservations[digest2]={...ledger.reservations[digest]};writeFileSync(f.path,JSON.stringify(ledger));
  assert.throws(()=>reserveIssuerGas(c,reservationFixture,f.env),/usdcGasLedgerInvalid/);
});
test('unknown issuer submission retains its full reservation before exact replay',async t=>{
  const f=gasFixture(t),mock=buildFixture(),p=await prepared();let calls=0;
  mock.client.core.executeTransaction=async()=>{calls++;assert.equal(Object.keys(JSON.parse(readFileSync(f.path,'utf8')).reservations).length,1);throw Error('simulated RPC timeout after submission');};
  await assert.rejects(()=>submitPrepared(c,{prepared:p,order},f.env,mock.client));
  assert.equal(calls,1);
  mock.client.core.executeTransaction=async()=>{calls++;return {Transaction:{digest:p.digest}};};
  assert.equal((await submitPrepared(c,{prepared:p,order},f.env,mock.client)).submitted,true);
  assert.equal(calls,2);assert.equal(Object.keys(JSON.parse(readFileSync(f.path,'utf8')).reservations).length,1);
  const other=await prepared(order,tx=>tx.setGasPrice(1001));
  await assert.rejects(()=>submitPrepared(c,{prepared:other,order},f.env,mock.client),/usdcGasLifetimeBudgetExceeded/);
  assert.equal(calls,2);
});
