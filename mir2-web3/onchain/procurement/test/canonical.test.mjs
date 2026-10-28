import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync, mkdtempSync, unlinkSync, rmdirSync, existsSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { intentBytes, proofBytes, hash, hex, validateAttestation, u64, verifyHash } from '../service/canonical.mjs';
import { validateConfig, validateCommands, dispatch, receipt, observeOrder, lookupOrder, reserveGas } from '../service/driver.mjs';

const f=JSON.parse(readFileSync(new URL('../fixtures/intent-v1.json',import.meta.url)));
const h=x=>'0x'+x.repeat(32);
const c={enabled:true,network:'testnet',environmentTag:'isolated-test',fullnodeUrl:'https://fullnode.testnet.sui.io:443',chainIdentifier:'4c78adac',
  networkHash:hex(hash(Buffer.from('4c78adac'))),packageId:h('22'),frameworkPackageId:h('99'),dappStorageId:h('33'),configId:h('aa'),
  oraclePublicKey:f.oracle_public_key,oracleKeyId:'1',gasBudgetMist:'10000000',maxGasBudgetMist:'20000000',dailyGasBudgetMist:'100000000'};

test('frozen BCS and both Ed25519 attestations match the cross-language vector',()=>{
  assert.equal(hex(intentBytes(f.intent)),f.canonical_intent_hex);
  assert.equal(intentBytes(f.intent).length,400);
  assert.equal(hex(hash(intentBytes(f.intent))),f.intent_sha256);
  assert.equal(hex(proofBytes(f.proof)),f.canonical_proof_hex);
  assert.equal(hex(hash(proofBytes(f.proof))),f.proof_sha256);
  assert.equal(verifyHash(intentBytes(f.intent),f.actor_signature,f.intent.actor_public_key),true);
  assert.equal(verifyHash(proofBytes(f.proof),f.oracle_signature,f.oracle_public_key),true);
  assert.doesNotThrow(()=>validateAttestation(f.intent,f.proof,f.actor_signature,f.oracle_signature,{...c,networkHash:f.intent.network_hash},'1999999970000'));
});

test('signed price/identity/order/conditions cannot be substituted',()=>{
  for(const [k,value] of [['price_gold','500001'],['buyer_ref',h('56')],['order_id',h('45')],['conditions_hash',h('67')]]) {
    assert.throws(()=>validateAttestation({...f.intent,[k]:value},f.proof,f.actor_signature,f.oracle_signature,{...c,networkHash:f.intent.network_hash},'1999999970000'));
  }
});
test('cross-network replay, expired proof and forged Oracle are rejected',()=>{
  assert.throws(()=>validateAttestation(f.intent,f.proof,f.actor_signature,f.oracle_signature,c,'1999999970000'));
  assert.throws(()=>validateAttestation(f.intent,f.proof,f.actor_signature,f.oracle_signature,{...c,networkHash:f.intent.network_hash},f.proof.valid_until_ms));
  assert.throws(()=>validateAttestation(f.intent,f.proof,f.actor_signature,`0x${'00'.repeat(64)}`,{...c,networkHash:f.intent.network_hash},'1999999970000'));
});
test('canonical serializer rejects overflow, alternate numbers and unknown signed fields',()=>{
  for(const v of ['01','-1','18446744073709551616','1.0',1])assert.throws(()=>u64(v));
  assert.throws(()=>intentBytes({...f.intent,price_gold:'4294967296'}));
  assert.throws(()=>intentBytes({...f.intent,quantity:'2'}));
  assert.throws(()=>intentBytes({...f.intent,extra:true}));
  assert.throws(()=>intentBytes({...f.intent,domain:'wrong'}));
});
test('disabled/default/mainnet configuration fails before SDK construction or network',async()=>{
  assert.doesNotThrow(()=>validateConfig(c));
  for(const bad of [{}, {...c,enabled:false},{...c,network:'mainnet'},{...c,chainIdentifier:'35834a8a'},
    {...c,fullnodeUrl:'http://example.com'}, {...c,gasBudgetMist:'100000001'}, {...c,networkHash:h('00')}])assert.throws(()=>validateConfig(bad));
  await assert.rejects(dispatch({protocolVersion:1,operation:'build',config:{enabled:false}}));
  await assert.rejects(dispatch({protocolVersion:1,operation:'build',config:c,payload:{action:'expire',orderObjectId:h('bb')}},{}),/Explicit isolated-test signing keys/);
});
test('sponsorship accepts one bounded procurement call and rejects arbitrary transfer/package/budget',()=>{
  const object=objectId=>({Object:{SharedObject:{objectId}}});
  const data={commands:[{$kind:'MoveCall',MoveCall:{package:c.packageId,module:'procurement_system',function:'expire_order',typeArguments:[]}}],
    gasData:{budget:c.gasBudgetMist},inputs:[object(c.dappStorageId),object(c.configId),object(h('bb')),object('0x6')]};
  assert.doesNotThrow(()=>validateCommands(data,c));
  for(const bad of [{...data,gasData:{budget:'999999999'}},{...data,commands:[{$kind:'TransferObjects',TransferObjects:{}}]},
    {...data,commands:[{MoveCall:{...data.commands[0].MoveCall,package:h('ee')}}]},
    {...data,commands:[...data.commands,...data.commands]}, {...data,inputs:[]}])assert.throws(()=>validateCommands(bad,c));
});

test('missing transaction effects are unknown and cannot become a refund decision',()=>{
  assert.throws(()=>receipt({digest:'anything',checkpoint:'1'}),/unknown/);
  assert.equal(receipt({digest:'x',checkpoint:'2',effects:{status:{status:'failure',error:'MoveAbort'}}}).success,false);
  assert.equal(receipt({digest:'x',checkpoint:null,effects:{status:{status:'success'}}}).finalized,false);
});

test('real Dubhe SDK creates and signs the allowed PTB without auto keys or RPC',async()=>{
  const {Dubhe}=await import('@0xobelisk/sui-client');
  const {Transaction}=await import('@mysten/sui-modern/transactions');
  const {verifyTransactionSignature}=await import('@mysten/sui-modern/verify');
  const entry={isEntry:false,visibility:'Public',typeParameters:[],parameters:[],return:[]};
  const metadata={intent:{name:'intent',structs:{},exposedFunctions:{new_intent:{...entry},new_proof:{...entry}}},
    procurement_system:{name:'procurement_system',structs:{},exposedFunctions:{fill_order:{...entry}}}};
  const dubhe=new Dubhe({networkType:'localnet',secretKey:h('03'),packageId:c.packageId,metadata,fullnodeUrls:['http://127.0.0.1:1']});
  const tx=new Transaction();tx.setGasBudget(c.gasBudgetMist);tx.setGasPrice(1);
  tx.setGasPayment([{objectId:h('dd'),version:'1',digest:'11111111111111111111111111111111'}]);
  const shared=(id,mutable)=>tx.sharedObjectRef({objectId:id,initialSharedVersion:'1',mutable});
  const ds=shared(c.dappStorageId,false),cfg=shared(c.configId,false),order=shared(h('bb'),true),clock=shared('0x6',false);
  const i=await dubhe.tx.intent.new_intent({tx,params:[],isRaw:true});
  const p=await dubhe.tx.intent.new_proof({tx,params:[],isRaw:true});
  await dubhe.tx.procurement_system.fill_order({tx,params:[ds,cfg,order,clock,i,p,tx.pure.vector('u8',[]),tx.pure.vector('u8',[])],isRaw:true});
  tx.setSender(dubhe.getAddress());
  const signed=await dubhe.signTxn(await tx.build());
  const raw=Buffer.from(signed.bytes,'base64');
  const decoded=Transaction.from(raw);
  assert.doesNotThrow(()=>validateCommands(decoded.getData(),c));
  const signer=await verifyTransactionSignature(raw,signed.signature);
  assert.equal(signer.toSuiAddress(),dubhe.getAddress());
  assert.equal((await decoded.getDigest()).length>20,true);
});

function observationMock({race=false,unfinalizedClock=false,omitEvent=false}={}){
  const orderId=h('44'),objectId=h('bb'),type=`${c.frameworkPackageId}::dapp_service::ObjectStorage<${c.packageId}::procurement_system::Order>`;
  let reads=0;const trace=[];
  return {trace,
    async getObject({id}){trace.push(id==='0x6'?'clock':'order');
      if(id==='0x6')return {data:{type:'0x2::clock::Clock',version:'42',previousTransaction:'clock-digest',content:{fields:{timestamp_ms:'2000000000001'}}}};
      reads++;return {data:{type,version:race&&reads>1?'3':'2',previousTransaction:race&&reads>1?'activate-digest':'create-digest',owner:{Shared:{}},
        content:{fields:{dapp_key:`${c.packageId}::dapp_key::DappKey`,object_type:[...Buffer.from('order')],entity_id:[...Buffer.from(orderId.slice(2),'hex')],data:{fields:{id:{id:h('cc')}}}}}}};},
    async getDynamicFields(){trace.push('bag');return {data:[],hasNextPage:false};},
    async getTransactionBlock({digest}){trace.push(digest);if(digest==='clock-digest')return {digest,checkpoint:unfinalizedClock?null:'20',effects:{status:{status:'success'}}};
      return {digest,checkpoint:'10',effects:{status:{status:'success'}},objectChanges:[{type:'created',objectId,objectType:type}],events:omitEvent?[]:[{id:{eventSeq:'0'},
        type:`${c.frameworkPackageId}::dubhe_events::Dubhe_Object_Created`,parsedJson:{dapp_key:`${c.packageId}::dapp_key::DappKey`,object_id:objectId,
          entity_id:[...Buffer.from(orderId.slice(2),'hex')],object_type:[...Buffer.from('order')]}}]};},
    async getLatestCheckpointSequenceNumber(){trace.push('latest');return '21';}
  };
}
test('safe created-order observation samples confirmed Clock before a stable complete object',async()=>{
  const client=observationMock();const result=await observeOrder(client,c,h('bb'));
  assert.equal(result.status,0);assert.equal(result.observation.coherent,true);
  assert.equal(result.observation.clock.checkpoint,'20');assert.equal(result.observation.transactionCheckpoint,'10');
  assert.deepEqual(client.trace,['clock','clock-digest','order','bag','order','create-digest','latest']);
});
test('created-to-active race is rejected rather than combining old status with a later Clock',async()=>{
  await assert.rejects(observeOrder(observationMock({race:true}),c,h('bb')),/changed during observation/);
  await assert.rejects(observeOrder(observationMock({unfinalizedClock:true}),c,h('bb')),/Clock is not finalized/);
  await assert.rejects(observeOrder(observationMock({omitEvent:true}),c,h('bb')),/matching framework\/package event/);
});
test('registry absence is Clock-first and rejected if a concurrent create changes the registry',async()=>{
  function registry(race){const base=observationMock();let reads=0;
    const oldGet=base.getObject.bind(base),oldTx=base.getTransactionBlock.bind(base);
    base.getObject=async args=>args.id==='0x6'?oldGet(args):{data:{version:race&&++reads>1?'3':'2',previousTransaction:'registry-digest'}};
    base.getTransactionBlock=async args=>args.digest==='clock-digest'?oldTx(args):{digest:'registry-digest',checkpoint:'11',effects:{status:{status:'success'}},objectChanges:[{type:'mutated',objectId:c.dappStorageId}]};
    return base;
  }
  const result=await lookupOrder(registry(false),c,h('44'));
  assert.equal(result.found,false);assert.equal(result.observation.registryAbsence,true);
  assert.equal(result.observation.clock.checkpoint,'20');
  await assert.rejects(lookupOrder(registry(true),c,h('44')),/Registry changed/);
});
test('durable gas reservations deduplicate exact digests, enforce a daily cap and fail closed on stale locks',()=>{
  const dir=mkdtempSync(join(tmpdir(),'mir2-procurement-gas-')),file=join(dir,'ledger.json'),env={MIR2_PROCUREMENT_GAS_LEDGER:file};
  try {
    const limits={...c,dailyGasBudgetMist:'20000000'};
    reserveGas(limits,'digest-a',env);reserveGas(limits,'digest-a',env);
    assert.equal(Object.keys(JSON.parse(readFileSync(file)).reservations).length,1);
    reserveGas(limits,'digest-b',env);
    assert.throws(()=>reserveGas(limits,'digest-c',env),/Daily gas budget/);
    writeFileSync(file+'.lock','');assert.throws(()=>reserveGas(limits,'digest-a',env),/EEXIST/);
    unlinkSync(file+'.lock');writeFileSync(file,'corrupted');assert.throws(()=>reserveGas(limits,'digest-a',env));
  } finally {
    for(const path of [file,file+'.lock'])if(existsSync(path))unlinkSync(path);
    rmdirSync(dir);
  }
});
test('open-order observation binds recoverable condition bytes to the authorized on-chain hash',async()=>{
  function clientFor(tamper){
    const client=observationMock(),previous=client.getTransactionBlock.bind(client),condition=[0,0,0,0,0];
    const fields={publish_intent_hash:[...Buffer.from(f.intent_sha256.slice(2),'hex')],buyer_ref:[...Buffer.from(f.intent.buyer_ref.slice(2),'hex')],
      buyer_public_key:[...Buffer.from(f.intent.actor_public_key.slice(2),'hex')],template_id:245,conditions_hash:[...hash(Buffer.from(condition))],
      conditions_bcs:tamper?[1,0,0,0,0]:condition,quantity:'1',price_gold:'500000',expires_at_ms:'2000000000000',rule_version:'1',
      escrow_lock_id:[...Buffer.from(f.proof.asset_lock_id.slice(2),'hex')],status:1,fence:'0',fill_id:new Array(32).fill(0),
      seller_ref:new Array(32).fill(0),asset_instance_hash:new Array(32).fill(0)};
    client.getDynamicFields=async()=>({hasNextPage:false,data:Object.keys(fields).map(key=>({name:{type:'vector<u8>',value:[...Buffer.from(key)]}}))});
    client.getDynamicFieldObject=async({name})=>({data:{content:{fields:{value:fields[Buffer.from(name.value).toString()]}}}});
    client.getTransactionBlock=async args=>{
      const tx=await previous(args);if(args.digest==='clock-digest')return tx;
      tx.objectChanges[0].type='mutated';tx.events=[{id:{eventSeq:'15'},type:`${c.packageId}::procurement_system::OrderTransition`,
        parsedJson:{...fields,object_id:h('bb'),order_id:[...Buffer.from('44'.repeat(32),'hex')]}}];return tx;
    };return client;
  }
  const result=await observeOrder(clientFor(false),c,h('bb'));
  assert.equal(result.status,1);assert.equal(result.fields.conditions_bcs,'0x0000000000');
  await assert.rejects(observeOrder(clientFor(true),c,h('bb')),/Condition preimage/);
});
