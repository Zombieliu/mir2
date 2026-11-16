// Actual SDK BCS and driver functions; RPC and indexer data are local fixtures.
// No signing key, wallet session, environment authority or network is used.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { Transaction,coinWithBalance } from '@mysten/sui/transactions';
import { SuiJsonRpcClient } from '@mysten/sui/jsonRpc';
import { bcs } from '@mysten/sui/bcs';
import { blake2b } from '@noble/hashes/blake2.js';
import { toBase58 } from '@mysten/sui/utils';
import { COIN_TYPE,GENESIS_DIGEST,NETWORK_HASH,validateDirectOrder,protobufTimestampMs } from '../service/model.mjs';
import { DIRECT_DISCOVERY_PAGE_SIZE,DIRECT_GRAPHQL_URL,discoverDirectTransfers,observeDirectTransfer } from '../service/driver.mjs';

const h=n=>'0x'+n.repeat(64),d=toBase58(new Uint8Array(32).fill(1)),effectsDigest=toBase58(new Uint8Array(32).fill(2));
const c={enabled:true,environmentTag:'isolated-test',network:'testnet',chainIdentifier:'4c78adac',networkHash:NETWORK_HASH,
  fullnodeUrl:'https://fullnode.testnet.sui.io:443',packageId:h('1'),configId:h('2'),adminCapId:h('3'),issuerAddress:h('e'),merchantAddress:h('4'),coinType:COIN_TYPE,gasBudgetMist:'50000000'};
const order={orderRef:h('5'),accountRef:h('6'),snapshotHash:h('7'),payer:h('8'),merchant:h('4'),amountMicros:'1234567',expiresAtMs:'20000',createdAtMs:'10000'};
const env=new Proxy({},{get(){throw Error('read-only operation touched an environment secret');}});
const scalar=n=>{const b=Buffer.alloc(8);b.writeBigUInt64LE(BigInt(n));return b;};
const addr=v=>Buffer.from(v.slice(2),'hex');
const vec=v=>{const b=typeof v==='string'?Buffer.from(v):v;return Buffer.concat([Buffer.from([b.length]),b]);};
const owner=address=>({kind:1,address});
const timestamp=ms=>({seconds:BigInt(ms)/1000n,nanos:Number(BigInt(ms)%1000n)*1000000});
const iso=ms=>new Date(Number(ms)).toISOString();
const usdCoin='0x2::coin::Coin<'+COIN_TYPE+'>',suiCoin='0x2::coin::Coin<0x2::sui::SUI>';
const digestAt=i=>toBase58(createHash('sha256').update(String(i)).digest());

function publicClient(){
  const cfg={objectId:c.configId,type:c.packageId+'::recharge::Config',version:'1',digest:d,previousTransaction:d,
    owner:{$kind:'Shared',Shared:{initialSharedVersion:'1'}},content:Buffer.concat([addr(c.configId),addr(c.adminCapId),addr(c.issuerAddress),vec(addr(c.networkHash)),vec(c.coinType.slice(2)),addr(c.merchantAddress),Buffer.from([1])])};
  const cap={objectId:c.adminCapId,type:c.packageId+'::recharge::AdminCap',owner:{$kind:'AddressOwner',AddressOwner:c.issuerAddress},content:Buffer.concat([addr(c.adminCapId),addr(c.issuerAddress)])};
  const calls=[];
  return {calls,core:{getChainIdentifier:async()=>({chainIdentifier:GENESIS_DIGEST}),getObject:async({objectId})=>{
    calls.push(['getObject',objectId]);return {object:objectId===c.packageId?{type:'package'}:objectId===c.configId?cfg:objectId===c.adminCapId?cap:null};
  },executeTransaction:async()=>{throw Error('read-only operation attempted broadcast');}}};
}
async function fixture({payer=order.payer,gasOwner=payer,kind='direct',paidAtMs=12000,wholeCoin=false,merge=false}={}){
  const sdk=new Transaction();sdk.setSender(payer);sdk.setGasOwner(gasOwner);sdk.setGasBudget(c.gasBudgetMist);sdk.setGasPrice(1000);
  sdk.setGasPayment([{objectId:h('a'),version:'1',digest:d}]);
  const coin=sdk.objectRef({objectId:h('b'),version:'1',digest:d});
  if(merge)sdk.mergeCoins(coin,[sdk.objectRef({objectId:h('d'),version:'1',digest:d})]);
  if(kind==='invoice')sdk.moveCall({target:c.packageId+'::recharge::pay',typeArguments:[c.coinType],arguments:[coin]});
  else if(kind==='merge')sdk.mergeCoins(coin,[sdk.objectRef({objectId:h('d'),version:'1',digest:d})]);
  else if(wholeCoin)sdk.transferObjects([coin],sdk.pure.address(order.merchant));
  else {const [payment]=sdk.splitCoins(coin,[sdk.pure.u64(order.amountMicros)]);sdk.transferObjects([payment],sdk.pure.address(order.merchant));}
  if(kind==='unsupported-standard')sdk.moveCall({target:'0x2::coin::into_balance',typeArguments:[COIN_TYPE],arguments:[coin]});
  if(kind==='wrong-framework-type')sdk.moveCall({target:'0x2::coin::destroy_zero',typeArguments:['0x2::sui::SUI'],arguments:[coin]});
  const raw=await sdk.build(),digest=await sdk.getDigest();
  const object=(objectId,version,balance,address,type)=>({objectId,version:BigInt(version),digest:d,owner:owner(address),objectType:type,
    balance:BigInt(balance),contents:{value:Buffer.concat([addr(objectId),scalar(balance)])}});
  const effect=(objectId,type,created=false,address=payer)=>({objectId,objectType:type,inputState:created?1:2,inputVersion:created?undefined:1n,
    inputDigest:created?undefined:d,inputOwner:created?undefined:owner(payer),outputState:2,outputVersion:2n,outputDigest:d,outputOwner:owner(address)});
  const objects=[object(h('a'),1,100000000,payer,suiCoin),object(h('a'),2,99990000,payer,suiCoin),
    object(h('b'),1,wholeCoin?order.amountMicros:BigInt(order.amountMicros)+10n,payer,usdCoin),
    object(h('b'),2,wholeCoin?order.amountMicros:10,wholeCoin?order.merchant:payer,usdCoin)];
  const changedObjects=[effect(h('a'),suiCoin),effect(h('b'),usdCoin,false,wholeCoin?order.merchant:payer)];
  if(!wholeCoin){objects.push(object(h('c'),2,order.amountMicros,order.merchant,usdCoin));changedObjects.push(effect(h('c'),usdCoin,true,order.merchant));}
  if(merge){
    const initial=objects[2];initial.balance=500000n;initial.contents.value=Buffer.concat([addr(h('b')),scalar(initial.balance)]);
    objects.push(object(h('d'),1,BigInt(order.amountMicros)+10n-initial.balance,payer,usdCoin));
    changedObjects.push({objectId:h('d'),objectType:usdCoin,inputState:2,inputVersion:1n,inputDigest:d,inputOwner:owner(payer),outputState:1,idOperation:3});
  }
  const tx={digest,checkpoint:12n,timestamp:timestamp(paidAtMs),transaction:{sender:payer,bcs:{value:raw}},
    effects:{digest:effectsDigest,transactionDigest:digest,status:{success:true},changedObjects},objects:{objects},events:{events:[]},
    balanceChanges:[{address:order.payer,coinType:COIN_TYPE,amount:'-'+order.amountMicros},{address:order.merchant,coinType:COIN_TYPE,amount:order.amountMicros}]};
  const checkpoint={sequenceNumber:12n,digest:d,summary:{sequenceNumber:12n,timestamp:timestamp(paidAtMs)},contents:{transactions:[{transaction:digest,effects:effectsDigest}]}};
  const client=publicClient();client.ledgerService={getTransaction:async request=>{client.calls.push(['transaction',request]);return {response:{transaction:tx}};},
    getCheckpoint:async request=>{client.calls.push(['checkpoint',request]);return {response:{checkpoint}};}};
  return {client,tx,checkpoint,digest,observe:()=>observeDirectTransfer(c,{order,digest},env,client)};
}
async function walletIntent(f,{balances=['500000',(BigInt(order.amountMicros)+10n-500000n).toString()],addressBalance='0',gasAddressBalance=false}={}){
  const tx=new Transaction();tx.setSender(order.payer);tx.setGasOwner(order.payer);tx.setGasBudget(c.gasBudgetMist);tx.setGasPrice(1000);
  tx.setGasPayment(gasAddressBalance?[]:[{objectId:h('a'),version:'1',digest:d}]);
  if(gasAddressBalance)tx.setExpiration({ValidDuring:{minEpoch:'100',maxEpoch:'101',minTimestamp:null,maxTimestamp:null,chain:GENESIS_DIGEST,nonce:7}});
  tx.transferObjects([coinWithBalance({type:COIN_TYPE,balance:BigInt(order.amountMicros)})],tx.pure.address(order.merchant));
  const sdk=new SuiJsonRpcClient({network:'testnet',transport:{request:async()=>{throw Error('unexpected live RPC');}}});
  f.client.core.resolveTransactionPlugin=()=>sdk.core.resolveTransactionPlugin();
  f.client.core.getBalance=async({coinType})=>({balance:coinType?.includes('::sui::SUI')?{balance:'100000000',coinBalance:'0',addressBalance:'100000000'}:
    {balance:(balances.reduce((n,b)=>n+BigInt(b),0n)+BigInt(addressBalance)).toString(),coinBalance:balances.reduce((n,b)=>n+BigInt(b),0n).toString(),addressBalance}});
  f.client.core.getCurrentSystemState=async()=>({systemState:{epoch:'100',referenceGasPrice:'1000'}});
  f.client.core.listCoins=async()=>({objects:balances.map((balance,index)=>({objectId:h(index===0?'b':'d'),version:'1',digest:d,balance})),hasNextPage:false,cursor:null});
  const raw=await tx.build({client:f.client}),digest=await tx.getDigest();
  f.tx.transaction.bcs.value=raw;f.tx.digest=digest;f.tx.effects.transactionDigest=digest;f.checkpoint.contents.transactions[0].transaction=digest;
  return {data:Transaction.from(raw).getData(),observe:()=>observeDirectTransfer(c,{order,digest},env,f.client)};
}
function sealEffects(f){
  f.tx.effects.gasUsed??={computationCost:10000n,storageCost:0n,storageRebate:0n,nonRefundableStorageFee:0n};
  for(const effect of f.tx.effects.changedObjects)effect.idOperation??=effect.inputState===1&&effect.outputState===2?2:1;
  const raw=bcs.TransactionEffects.serialize({V2:{status:{Success:true},executedEpoch:'100',gasUsed:f.tx.effects.gasUsed,
    transactionDigest:f.tx.digest,gasObjectIndex:null,eventsDigest:null,dependencies:[],lamportVersion:'2',unchangedConsensusObjects:[],auxDataDigest:null,
    changedObjects:f.tx.effects.changedObjects.map(effect=>[effect.objectId,{
      inputState:effect.inputState===2?{Exist:[[String(effect.inputVersion),effect.inputDigest],{AddressOwner:effect.inputOwner.address}]}:{NotExist:true},
      outputState:effect.outputState===4?{AccumulatorWriteV1:{address:{address:effect.accumulatorWrite.address,ty:effect.accumulatorWrite.accumulatorType},
        operation:effect.accumulatorWrite.operation===1?{Merge:true}:{Split:true},value:{Integer:String(effect.accumulatorWrite.value)}}}:
        effect.outputState===2?{ObjectWrite:[effect.outputDigest,{AddressOwner:effect.outputOwner.address}]}:{NotExist:true},
      idOperation:effect.idOperation===2?{Created:true}:effect.idOperation===3?{Deleted:true}:{None:true},
    }]),
  }}).toBytes();
  f.tx.effects.bcs={value:raw};f.tx.effects.digest=toBase58(blake2b(Buffer.concat([Buffer.from('TransactionEffects::'),raw]),{dkLen:32}));
  f.checkpoint.contents.transactions[0].effects=f.tx.effects.digest;
}
async function addressBalanceFixture({mixed=false,gasAddressBalance=false}={}){
  const f=await fixture(),coinBalance=mixed?700000n:0n,withdrawal=BigInt(order.amountMicros)-coinBalance;
  const intent=await walletIntent(f,{balances:mixed?[coinBalance.toString()]:[],addressBalance:withdrawal.toString(),gasAddressBalance});
  f.tx.objects.objects=f.tx.objects.objects.filter(object=>object.objectId===h('c')||!gasAddressBalance&&object.objectId===h('a')||mixed&&object.objectId===h('b')&&object.version===1n);
  f.tx.effects.changedObjects=f.tx.effects.changedObjects.filter(effect=>effect.objectId===h('c')||!gasAddressBalance&&effect.objectId===h('a')||mixed&&effect.objectId===h('b'));
  if(mixed){
    const input=f.tx.objects.objects.find(object=>object.objectId===h('b'));
    input.balance=coinBalance;input.contents.value=Buffer.concat([addr(h('b')),scalar(coinBalance)]);
    const effect=f.tx.effects.changedObjects.find(effect=>effect.objectId===h('b'));
    effect.outputState=1;effect.idOperation=3;delete effect.outputVersion;delete effect.outputDigest;delete effect.outputOwner;
  }
  f.tx.effects.changedObjects.push({objectId:h('d'),inputState:1,outputState:4,idOperation:1,
    accumulatorWrite:{address:order.payer,accumulatorType:'0x2::balance::Balance<'+COIN_TYPE+'>',operation:2,value:withdrawal}});
  if(gasAddressBalance)f.tx.effects.changedObjects.push({objectId:h('a'),inputState:1,outputState:4,idOperation:1,
    accumulatorWrite:{address:order.payer,accumulatorType:'0x2::balance::Balance<0x2::sui::SUI>',operation:2,value:10000n}});
  sealEffects(f);return {...f,data:intent.data,observe:intent.observe};
}

test('direct contract keeps invoice fields exact and preserves u64 time precision',()=>{
  assert.equal(validateDirectOrder(order,c),order);
  for(const change of [o=>delete o.createdAtMs,o=>{o.createdAtMs=10000;},o=>{o.createdAtMs='0';},o=>{o.createdAtMs=o.expiresAtMs;},o=>{o.amountMicros='0';},o=>{o.credits=999;}]){
    const other={...order};change(other);assert.throws(()=>validateDirectOrder(other,c));
  }
  assert.equal(protobufTimestampMs(timestamp(12001)),'12001');
  assert.throws(()=>protobufTimestampMs({seconds:12,nanos:0}));
  assert.throws(()=>protobufTimestampMs({seconds:12n,nanos:1}));
});
test('direct receipt requires real SDK BCS, finalized checkpoint inclusion and two independent exact net flows',async()=>{
  for(const wholeCoin of [false,true]){
    const f=await fixture({wholeCoin}),result=await f.observe();
    assert.deepEqual(result,{state:'paid',receipt:{genesisDigest:GENESIS_DIGEST,transactionDigest:f.digest,checkpoint:'12',paidAtMs:'12000',payer:order.payer,merchant:order.merchant,coinType:COIN_TYPE,amountMicros:order.amountMicros}});
    assert.deepEqual(await f.observe(),result,'repeat observation is deterministic; allocation is the PG consumer responsibility');
    assert.ok(f.client.calls.some(v=>v[0]==='transaction'&&v[1].readMask.paths.includes('objects')));
    assert.ok(f.client.calls.some(v=>v[0]==='checkpoint'));
  }
});
test('wrong full genesis and altered public merchant configuration fail before looking at payment',async()=>{
  const f=await fixture();f.client.core.getChainIdentifier=async()=>({chainIdentifier:'wrong'});
  await assert.rejects(()=>f.observe(),/usdcWrongNetwork/);
  await assert.rejects(()=>observeDirectTransfer({...c,coinType:'0x2::sui::SUI'},{order,digest:f.digest},env,f.client));
  await assert.rejects(()=>observeDirectTransfer(c,{order:{...order,merchant:h('f')},digest:f.digest},env,f.client));
  assert.equal(f.client.calls.length,0);
});
test('real SDK multi-Coin MergeCoins payment includes consumed input history and the exact total USDC flow',async()=>{
  const f=await fixture({merge:true});
  assert.deepEqual(Transaction.from(f.tx.transaction.bcs.value).getData().commands.map(c=>c.$kind),['MergeCoins','SplitCoins','TransferObjects']);
  assert.equal((await f.observe()).state,'paid');
  assert.deepEqual(await f.observe(),await f.observe());
});
test('the actual SDK coinWithBalance resolver builds supported multi-Coin and exact-balance destroy_zero payments',async()=>{
  const merged=await fixture({merge:true}),intent=await walletIntent(merged);
  assert.deepEqual(intent.data.commands.map(c=>c.$kind),['MergeCoins','SplitCoins','TransferObjects']);
  assert.equal((await intent.observe()).state,'paid');
  const exact=await fixture({merge:true}),source=exact.tx.objects.objects.find(o=>o.objectId===h('d'));
  source.balance-=10n;source.contents.value=Buffer.concat([addr(h('d')),scalar(source.balance)]);
  exact.tx.objects.objects=exact.tx.objects.objects.filter(o=>!(o.objectId===h('b')&&o.version===2n));
  const effect=exact.tx.effects.changedObjects.find(e=>e.objectId===h('b'));
  effect.outputState=1;effect.idOperation=3;delete effect.outputVersion;delete effect.outputDigest;delete effect.outputOwner;
  const exactIntent=await walletIntent(exact,{balances:['500000',(BigInt(order.amountMicros)-500000n).toString()]});
  const destroy=exactIntent.data.commands.find(c=>c.MoveCall)?.MoveCall;
  assert.equal(destroy.function,'destroy_zero');assert.deepEqual(destroy.typeArguments,[COIN_TYPE]);assert.equal(destroy.arguments.length,1);
  assert.equal((await exactIntent.observe()).state,'paid');
});
test('merged sources require exact native type, payer ownership, historical BCS and a corresponding deletion effect',async()=>{
  for(const change of [f=>{f.tx.objects.objects[5].owner=owner(h('f'));},
    f=>{f.tx.objects.objects[5].objectType=suiCoin;},f=>{f.tx.effects.changedObjects[3].objectType=suiCoin;},
    f=>{f.tx.objects.objects[5].contents.value=Buffer.concat([addr(h('d')),scalar(1)]);delete f.tx.objects.objects[5].balance;},
    f=>{f.tx.effects.changedObjects[3].idOperation=1;},f=>{f.tx.effects.changedObjects[3].inputDigest=effectsDigest;}]){
    const f=await fixture({merge:true});change(f);assert.notEqual((await f.observe()).state,'paid');
  }
  for(const change of [f=>f.tx.objects.objects.pop(),f=>f.tx.effects.changedObjects.pop(),f=>delete f.tx.objects.objects[5].contents]){
    const f=await fixture({merge:true});change(f);assert.deepEqual(await f.observe(),{state:'unknown'},'missing indexed history must be retried, not skipped as unpaid');
  }
});
test('actual SDK pure and mixed address-balance USDC payments require finalized raw-effects accumulator proofs',async()=>{
  for(const mixed of [false,true])for(const gasAddressBalance of [false,true]){
    const f=await addressBalanceFixture({mixed,gasAddressBalance});
    assert.ok(f.data.commands.some(command=>command.MoveCall?.function==='redeem_funds'));
    assert.ok(f.data.commands.some(command=>command.MoveCall?.function==='send_funds'));
    if(mixed)assert.ok(f.data.commands.some(command=>command.$kind==='MergeCoins'));
    assert.equal((await f.observe()).state,'paid');
  }
});
test('Balance redeem plus from_balance is supported only through its exact native-USDC result reference',async()=>{
  for(const wrongType of [false,true]){
    const f=await addressBalanceFixture(),tx=new Transaction();tx.setSender(order.payer);tx.setGasOwner(order.payer);tx.setGasBudget(c.gasBudgetMist);tx.setGasPrice(1000);
    tx.setGasPayment([{objectId:h('a'),version:'1',digest:d}]);
    const [balance]=tx.moveCall({target:'0x2::balance::redeem_funds',typeArguments:[COIN_TYPE],arguments:[tx.withdrawal({amount:order.amountMicros,type:COIN_TYPE})]});
    const [coin]=tx.moveCall({target:'0x2::coin::from_balance',typeArguments:[wrongType?'0x2::sui::SUI':COIN_TYPE],arguments:[balance]});
    const [payment]=tx.splitCoins(coin,[tx.pure.u64(order.amountMicros)]);
    tx.transferObjects([payment],tx.pure.address(order.merchant));
    tx.moveCall({target:'0x2::coin::destroy_zero',typeArguments:[COIN_TYPE],arguments:[coin]});
    const raw=await tx.build(),digest=await tx.getDigest();
    f.tx.digest=digest;f.tx.transaction.bcs.value=raw;f.tx.effects.transactionDigest=digest;f.checkpoint.contents.transactions[0].transaction=digest;sealEffects(f);
    const result=await observeDirectTransfer(c,{order,digest},env,f.client);
    assert.equal(result.state,wrongType?'unpaid':'paid');
  }
});
test('an accumulator JSON balance cannot replace or contradict its checkpoint-bound effects BCS',async()=>{
  for(const change of [f=>delete f.tx.effects.bcs,f=>{f.tx.effects.bcs.value[4]^=1;},
    f=>{f.tx.effects.changedObjects.find(e=>e.accumulatorWrite?.accumulatorType.includes('usdc::USDC')).accumulatorWrite.value+=1n;},
    f=>{f.tx.effects.gasUsed.storageCost+=1n;}]){
    const f=await addressBalanceFixture();change(f);assert.deepEqual(await f.observe(),{state:'unknown'});
  }
});
test('even hash-consistent accumulator effects reject wrong payer, coin, operation, net amount or SUI gas',async()=>{
  for(const change of [f=>{f.tx.effects.changedObjects.find(e=>e.objectId===h('d')).accumulatorWrite.value+=1n;},
    f=>{f.tx.effects.changedObjects.find(e=>e.objectId===h('d')).accumulatorWrite.address=h('f');},
    f=>{f.tx.effects.changedObjects.find(e=>e.objectId===h('d')).accumulatorWrite.accumulatorType='0x2::balance::Balance<0x2::sui::SUI>';},
    f=>{f.tx.effects.changedObjects.find(e=>e.objectId===h('d')).accumulatorWrite.operation=1;},
    f=>{f.tx.effects.changedObjects.find(e=>e.objectId===h('a')).accumulatorWrite.value+=1n;}]){
    const f=await addressBalanceFixture({mixed:true,gasAddressBalance:true});change(f);sealEffects(f);assert.notEqual((await f.observe()).state,'paid');
  }
});
test('missing/nonfinal transaction, checkpoint membership, indexed objects or balances remain unknown',async()=>{
  for(const change of [f=>delete f.tx.checkpoint,f=>{f.tx.digest=d;},f=>{f.checkpoint.sequenceNumber=13n;},
    f=>{f.checkpoint.contents.transactions=[];},f=>{f.checkpoint.contents.transactions[0].effects=d;},
    f=>{f.checkpoint.summary.timestamp=timestamp(12001);},f=>delete f.tx.timestamp,f=>delete f.tx.transaction.bcs,
    f=>delete f.tx.objects,f=>{f.tx.objects.objects.pop();},f=>delete f.tx.balanceChanges,f=>{f.tx.effects.changedObjects[1].objectType=undefined;},
    f=>delete f.tx.objects.objects[2].owner,f=>delete f.tx.effects.changedObjects[2].outputOwner,f=>delete f.tx.objects.objects[4].contents]){
    const f=await fixture();change(f);assert.deepEqual(await f.observe(),{state:'unknown'});
  }
  const absent=await fixture();absent.client.ledgerService.getTransaction=async()=>({response:{}});assert.deepEqual(await absent.observe(),{state:'unknown'});
  const unavailable=await fixture();unavailable.client.ledgerService.getCheckpoint=async()=>{throw Error('RPC unavailable');};assert.deepEqual(await unavailable.observe(),{state:'unknown'});
});
test('failed, historical and exact-deadline transfers are not eligible; later observation of on-time payment is',async()=>{
  for(const paidAtMs of [9999,20000,20001]){const f=await fixture({paidAtMs});assert.deepEqual(await f.observe(),{state:'unpaid'});}
  for(const paidAtMs of [10000,19999]){const f=await fixture({paidAtMs});assert.equal((await f.observe()).state,'paid');}
  const failed=await fixture();failed.tx.effects.status.success=false;assert.deepEqual(await failed.observe(),{state:'unpaid'});
});
test('another sender, sponsored gas and ordinary invoice/Move calls cannot be claimed as direct transfers',async()=>{
  for(const options of [{payer:h('f')},{gasOwner:c.issuerAddress},{kind:'invoice'},{kind:'merge'},{kind:'wrong-framework-type'}]){
    const f=await fixture(options);assert.deepEqual(await f.observe(),{state:'unpaid'});
  }
  for(const change of [f=>{f.tx.events.events.push({eventType:c.packageId+'::recharge::PaymentReceived'});},
    f=>{f.tx.effects.eventsDigest=d;},f=>{f.tx.transaction.bcs.value=Buffer.from([1,2,3]);}]){
    const f=await fixture();change(f);assert.notEqual((await f.observe()).state,'paid');
  }
});
test('recognized but unsupported standard native-USDC framework shapes stay pending instead of being skipped',async()=>{
  const f=await fixture({kind:'unsupported-standard'});assert.deepEqual(await f.observe(),{state:'unknown'});
});
test('USDC amount, asset, BCS ownership and third party deltas cannot be spoofed independently',async()=>{
  for(const change of [f=>{f.tx.balanceChanges[1].amount='1';},f=>{f.tx.balanceChanges[0].amount='-1';},
    f=>{f.tx.balanceChanges[1].coinType='0x2::sui::SUI';},f=>{f.tx.balanceChanges.push({address:h('f'),coinType:COIN_TYPE,amount:'1'});},
    f=>{f.tx.objects.objects[4].contents.value=Buffer.concat([addr(h('c')),scalar(1)]);delete f.tx.objects.objects[4].balance;},
    f=>{f.tx.effects.changedObjects[2].outputOwner=owner(h('f'));},f=>{f.tx.objects.objects[2].owner=owner(h('f'));},
    f=>{f.tx.objects.objects[2].objectType=suiCoin;},f=>{f.tx.effects.changedObjects[1].objectType='0x2::coin::Coin<0x2::sui::SUI>';},
    f=>{f.tx.effects.changedObjects[2].objectType=c.packageId+'::recharge::Invoice';},
    f=>{f.tx.objects.objects[2].digest=effectsDigest;},f=>{f.tx.effects.changedObjects.push({...f.tx.effects.changedObjects[2]});}]){
    const f=await fixture();change(f);assert.notEqual((await f.observe()).state,'paid');
  }
});

function indexFixture({last=100,first=0,pages=[[]]}={}){
  const client=publicClient(),calls=[];let pageIndex=0,metadataLast=last;
  const point=sequence=>({sequenceNumber:sequence,timestamp:iso(sequence*300)});
  const fetcher=async(url,options)=>{
    assert.equal(url,DIRECT_GRAPHQL_URL);assert.equal(options.redirect,'error');assert.equal(options.credentials,'omit');assert.equal(options.method,'POST');
    const request=JSON.parse(options.body);calls.push(request);
    let data;
    if(request.query.includes('query DirectIndex'))data={chainIdentifier:GENESIS_DIGEST,genesis:{digest:GENESIS_DIGEST},serviceConfig:{availableRange:{first:point(first),last:point(metadataLast)}}};
    else if(request.query.includes('query DirectCheckpoint'))data={checkpoint:point(request.variables.sequence)};
    else if(request.query.includes('query DirectTransfers(')){
      assert.equal(request.variables.payer,order.payer);assert.ok(request.query.includes('first:'+DIRECT_DISCOVERY_PAGE_SIZE));
      const nodes=pages[pageIndex++]??[],hasNextPage=pageIndex<pages.length;
      data={transactions:{nodes,pageInfo:{hasNextPage,endCursor:hasNextPage?'page-'+pageIndex:null}}};
    }else throw Error('unexpected query');
    return new Response(JSON.stringify({data}),{status:200});
  };
  return {client,calls,fetcher,setLast:value=>{metadataLast=value;},
    discover:cursor=>discoverDirectTransfers(c,{order,cursor:cursor??null},env,client,fetcher)};
}
const node=(i,checkpoint=40)=>({digest:digestAt(i),sender:{address:order.payer},effects:{timestamp:iso(checkpoint*300),checkpoint:{sequenceNumber:checkpoint}}});

test('discovery scans forward from the creation checkpoint and returns all pages beyond fifty candidates',async()=>{
  const all=Array.from({length:57},(_,i)=>node(i)),pages=[];
  for(let i=0;i<all.length;i+=DIRECT_DISCOVERY_PAGE_SIZE)pages.push(all.slice(i,i+DIRECT_DISCOVERY_PAGE_SIZE));
  const f=indexFixture({pages}),a=await f.discover();
  assert.equal(a.state,'scanned');assert.equal(a.digests.length,DIRECT_DISCOVERY_PAGE_SIZE);assert.equal(a.scanComplete,false);assert.equal(a.windowComplete,false);
  assert.equal(a.indexedThroughCheckpoint,'100');assert.equal(a.indexedThroughMs,'30000');
  const queryA=f.calls.find(q=>q.query.includes('query DirectTransfers('));assert.equal(queryA.variables.from,33);assert.equal(queryA.variables.through,101);assert.equal(queryA.variables.after,null);
  let b=a,collected=[...a.digests];
  for(let i=1;i<pages.length;i++){b=await f.discover(b.cursor);collected.push(...b.digests);}
  assert.equal(b.scanComplete,true);assert.equal(b.windowComplete,true);assert.deepEqual(collected,all.map(n=>n.digest));
  assert.equal(f.calls.filter(q=>q.query.includes('query DirectTransfers('))[1].variables.after,'page-1');
  const end=await f.discover(b.cursor);assert.deepEqual(end.digests,[]);assert.equal(end.windowComplete,true);
});
test('indexer lag is pending and completed cursors later scan only the newly indexed checkpoint interval',async()=>{
  const f=indexFixture({last:50,pages:[[],[node(1,60)]]}),a=await f.discover();
  assert.equal(a.scanComplete,undefined,'empty page with hasNextPage cannot claim completion');assert.equal(a.state,'unknown');
  const lag=indexFixture({last:50}),b=await lag.discover();assert.equal(b.scanComplete,true);assert.equal(b.windowComplete,false);
  lag.setLast(100);const next=await lag.discover(b.cursor);assert.equal(next.windowComplete,true);
  const calls=lag.calls.filter(q=>q.query.includes('query DirectTransfers('));assert.equal(calls[1].variables.from,50);
});
test('discovery never treats out-of-window candidates or index data as payment receipts',async()=>{
  const f=indexFixture({pages:[[node(1,40),node(2,75)]]}),result=await f.discover();
  assert.deepEqual(result.digests,[digestAt(1)]);assert.equal(result.receipt,undefined);
  const future=indexFixture({last:20}),pending=await future.discover();assert.equal(pending.windowComplete,false);assert.deepEqual(pending.digests,[]);
});
test('large history lookup returns bounded resumable search progress instead of restarting after every CLI budget',async()=>{
  const f=indexFixture({last:400000000,pages:[[node(1)]]});let cursor=null,result;
  for(let attempt=0;attempt<5;attempt++){
    const before=f.calls.length;result=await f.discover(cursor);
    assert.equal(result.state,'scanned');assert.ok(f.calls.slice(before).filter(q=>q.query.includes('query DirectCheckpoint')).length<=8);
    if(result.scanComplete)break;
    assert.equal(result.windowComplete,false);assert.deepEqual(result.digests,[]);assert.notEqual(result.cursor,cursor);
    cursor=result.cursor;
  }
  assert.equal(result.windowComplete,true);assert.deepEqual(result.digests,[digestAt(1)]);
  assert.ok(f.calls.filter(q=>q.query.includes('query DirectCheckpoint')).length<40);
});
test('a failed mid-search RPC returns only validated seek progress and the next call resumes it',async()=>{
  const f=indexFixture({last:400000000}),base=f.fetcher;let checkpoints=0;
  const interrupted=async(url,options)=>{
    if(JSON.parse(options.body).query.includes('query DirectCheckpoint')&&++checkpoints===3)throw Error('bounded RPC timeout');
    return base(url,options);
  };
  const partial=await discoverDirectTransfers(c,{order,cursor:null},env,f.client,interrupted);
  assert.equal(partial.state,'scanned');assert.equal(partial.scanComplete,false);assert.deepEqual(partial.digests,[]);
  const saved=JSON.parse(partial.cursor);assert.equal(saved.phase,'seek');assert.ok(BigInt(saved.hi)<400000000n);
  const before=f.calls.length;await f.discover(partial.cursor);
  assert.equal(f.calls.slice(before).find(q=>q.query.includes('query DirectCheckpoint')).variables.sequence,Number((BigInt(saved.lo)+BigInt(saved.hi))/2n));
});
test('a page timeout after completing seek preserves the scan start and retries the same first page without seeking again',async()=>{
  const f=indexFixture({pages:[[node(1)]]}),base=f.fetcher;let failedPage;
  const interrupted=async(url,options)=>{
    const request=JSON.parse(options.body);
    if(request.query.includes('query DirectTransfers(')){failedPage=request;throw Error('bounded page timeout');}
    return base(url,options);
  };
  const partial=await discoverDirectTransfers(c,{order,cursor:null},env,f.client,interrupted);
  assert.equal(partial.state,'scanned');assert.equal(partial.scanComplete,false);assert.equal(partial.windowComplete,false);assert.deepEqual(partial.digests,[]);
  const saved=JSON.parse(partial.cursor);
  assert.equal(saved.phase,'scan');assert.equal(saved.from,'33');assert.equal(saved.after,null);assert.equal(saved.done,false);assert.equal(saved.lo,null);assert.equal(saved.hi,null);
  const before=f.calls.length,resumed=await f.discover(partial.cursor),requests=f.calls.slice(before);
  assert.equal(requests.filter(q=>q.query.includes('query DirectCheckpoint')).length,0);
  assert.deepEqual(requests.find(q=>q.query.includes('query DirectTransfers(')).variables,failedPage.variables);
  assert.deepEqual(resumed.digests,[digestAt(1)]);assert.equal(resumed.scanComplete,true);assert.equal(resumed.windowComplete,true);
});
test('cursor bindings reject a different order, tampering and malformed pagination without advancing',async()=>{
  const f=indexFixture({pages:[[node(1)],[node(2)]]}),first=await f.discover();
  await assert.rejects(()=>discoverDirectTransfers(c,{order:{...order,accountRef:h('a')},cursor:first.cursor},env,f.client,f.fetcher),/usdcDirectCursorInvalid/);
  await assert.rejects(()=>f.discover('not-json'),/usdcDirectCursorInvalid/);
  const changed=JSON.parse(first.cursor);changed.through='01';await assert.rejects(()=>f.discover(JSON.stringify(changed)));
  changed.through=100;await assert.rejects(()=>f.discover(JSON.stringify(changed)));
  for(const pages of [[[node(1),node(1)]],[Array.from({length:51},(_,i)=>node(i))]]){
    const invalid=indexFixture({pages});assert.deepEqual(await invalid.discover(),{state:'unknown'});
  }
});
test('wrong index genesis, pruned history, server errors and missing pages fail closed without an unpaid claim',async()=>{
  const wrong=indexFixture(),base=wrong.fetcher;
  const wrongFetch=async(...args)=>{const response=await base(...args),body=await response.json();if(body.data.chainIdentifier)body.data.chainIdentifier='wrong';return new Response(JSON.stringify(body));};
  await assert.rejects(()=>discoverDirectTransfers(c,{order,cursor:null},env,wrong.client,wrongFetch),/usdcWrongNetwork/);
  assert.deepEqual(await indexFixture({first:40}).discover(),{state:'unknown'});
  const f=indexFixture();
  for(const fetcher of [async()=>{throw Error('timeout');},async()=>new Response(JSON.stringify({errors:[{message:'index not ready'}]})),async()=>new Response('unavailable',{status:503}),async()=>new Response('x'.repeat(1048577))]){
    assert.deepEqual(await discoverDirectTransfers(c,{order,cursor:null},env,f.client,fetcher),{state:'unknown'});
  }
});
