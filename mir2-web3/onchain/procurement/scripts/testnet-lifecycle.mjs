// Explicit isolated testnet runner. Never uses ~/.sui or production credentials.
// Contract lifecycle/Gas evidence only; game DB delivery is separately verified by Rust/gateway tests.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { isAbsolute, join, resolve } from 'node:path';
import { randomBytes } from 'node:crypto';
import { Ed25519Keypair } from '@mysten/sui-modern/keypairs/ed25519';
import { Transaction } from '@mysten/sui-modern/transactions';
import { requestIsolatedTestnetGas } from './faucet-v3.mjs';
import { createGrpcFacade } from '../service/grpc.mjs';
import { dispatch } from '../service/driver.mjs';
import { bytes, hash, hex, signHash, publicFromSeed, intentBytes, INTENT_DOMAIN, ORACLE_DOMAIN, ZERO } from '../service/canonical.mjs';

const root=process.env.MIR2_PROCUREMENT_TEST_ROOT;
if(process.argv[2]!=='--run-isolated-testnet'||!root||!isAbsolute(root)||resolve(root).toLowerCase().startsWith(resolve(process.cwd()).toLowerCase()))throw Error('Explicit external isolated-testnet directory and flag required');
const bytecodeFile=process.env.MIR2_PROCUREMENT_TEST_BYTECODE;
if(!bytecodeFile||!isAbsolute(bytecodeFile))throw Error('Explicit locally verified bytecode file required');
const bytecode=JSON.parse(readFileSync(bytecodeFile,'utf8'));
const privateDir=join(root,'private'),evidence=join(root,'evidence');mkdirSync(privateDir,{recursive:true});mkdirSync(evidence,{recursive:true});
const json=(file,value)=>writeFileSync(file,JSON.stringify(value,(_,v)=>typeof v==='bigint'?String(v):v,2)+'\n');
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const keyfile=join(privateDir,'isolated-test-keys.json');
if(!existsSync(keyfile)){
  if(process.env.MIR2_PROCUREMENT_ALLOW_ISOLATED_TEST_KEYS!=='1')throw Error('Isolated test key generation is not enabled');
  json(keyfile,{notice:'ISOLATED TESTNET ONLY. No real funds. Never deploy these keys to production.',sponsor:hex(randomBytes(32)),oracle:hex(randomBytes(32)),buyer:hex(randomBytes(32)),seller:hex(randomBytes(32))});
}
const keys=JSON.parse(readFileSync(keyfile)),sponsor=Ed25519Keypair.fromSecretKey(bytes(keys.sponsor));
const sponsorAddress=sponsor.getPublicKey().toSuiAddress();
const base={enabled:true,network:'testnet',environmentTag:'isolated-test',fullnodeUrl:'https://fullnode.testnet.sui.io:443',chainIdentifier:'4c78adac',
  genesisDigest:'69WiPg3DAQiwdxfncX6wYQ2siKwAe6L9BZthQea3JNMD',networkHash:hex(hash(Buffer.from('4c78adac'))),
  frameworkPackageId:'0xae33be6675639d6f1a5c468ae6bbc457ae4e22e57cd7526741d6143ce219d995',oraclePublicKey:publicFromSeed(keys.oracle),oracleKeyId:'1',
  // Setup maximum is 1.02 SUI; this shared runtime cap keeps the total <=2 test SUI.
  gasBudgetMist:'50000000',maxGasBudgetMist:'50000000',dailyGasBudgetMist:'980000000'};
const hub='0x80391929a8772aba091156fd1f099bf79240d2cf4471a14026b4aa68e3a240cc';
const rpc=await createGrpcFacade(base);if(await rpc.getChainIdentifier()!=='4c78adac')throw Error('Testnet chain mismatch');
const balance=await rpc.grpc.core.getBalance({owner:sponsorAddress});
if(BigInt(balance.balance.balance)<100000000n){await requestIsolatedTestnetGas(sponsorAddress,base.genesisDigest,evidence);await wait(1500);}
json(join(evidence,'environment.json'),{...base,sponsorAddress,hub,custodyFixtureOnly:true,realDatabaseDelivery:false,maxTotalBudgetMist:'2000000000'});

async function finalized(digest){for(let i=0;i<120;i++){try{const r=await rpc.getTransactionBlock({digest});if(r.checkpoint!=null){if(r.effects.status.status!=='success')throw Error('Finalized Move failure: '+r.effects.status.error);return r;}}catch(e){if(e.code!=='NOT_FOUND'&&!/not found|NOT_FOUND|not yet/i.test(e.message))throw e;}await wait(1000);}throw Error('Finality unknown; keep exact bytes and do not refund');}
const setupBudget={publish:'500000000',genesis:'500000000',enable:'20000000'};
async function setup(name,construct){
  const saved=join(evidence,'setup-'+name+'.signed.json');let signed;
  if(existsSync(saved))signed=JSON.parse(readFileSync(saved));else{
    const tx=new Transaction();tx.setSender(sponsorAddress);tx.setGasBudget(setupBudget[name]);construct(tx);
    const raw=await tx.build({client:rpc.grpc});
    const simulation=await rpc.grpc.core.simulateTransaction({transaction:raw,include:{effects:true,events:true}});
    json(join(evidence,'setup-'+name+'.simulation.json'),simulation);
    if(simulation.$kind!=='Transaction'||!simulation.Transaction.status.success)throw Error('Setup simulation failed');
    const sig=await sponsor.signTransaction(raw);
    signed={transactionBytes:Buffer.from(raw).toString('base64'),signatures:[sig.signature],digest:await Transaction.from(raw).getDigest(),gasBudgetMist:setupBudget[name]};
    json(saved,signed);
  }
  const receiptFile=join(evidence,'setup-'+name+'.receipt.json');if(existsSync(receiptFile))return JSON.parse(readFileSync(receiptFile));
  await rpc.executeExact(Buffer.from(signed.transactionBytes,'base64'),signed.signatures);
  const result=await finalized(signed.digest);json(receiptFile,result);return result;
}
const published=await setup('publish',tx=>{const[cap]=tx.publish({modules:bytecode.modules,dependencies:bytecode.dependencies});tx.transferObjects([cap],sponsorAddress);});
const packageId=published.objectChanges.find(o=>o.type==='published')?.objectId;
const adminCap=published.objectChanges.find(o=>o.type==='created'&&o.objectType===packageId+'::procurement_system::AdminCap')?.objectId;
if(!packageId||!adminCap)throw Error('Published package/AdminCap unavailable');
const genesis=await setup('genesis',tx=>tx.moveCall({target:packageId+'::procurement_system::genesis',arguments:[tx.object(adminCap),tx.object(hub),tx.object('0x6'),tx.pure.vector('u8',bytes(base.networkHash)),tx.pure.vector('u8',bytes(base.oraclePublicKey)),tx.pure.u64(1)]}));
const configId=genesis.objectChanges.find(o=>o.type==='created'&&o.objectType===packageId+'::procurement_system::Config')?.objectId;
const dappStorageId=genesis.objectChanges.find(o=>o.type==='created'&&o.objectType===base.frameworkPackageId+'::dapp_service::DappStorage')?.objectId;
if(!configId||!dappStorageId)throw Error('Config/DappStorage unavailable');
await setup('enable',tx=>tx.moveCall({target:packageId+'::procurement_system::set_publishing_enabled',arguments:[tx.object(adminCap),tx.object(configId),tx.pure.bool(true)]}));
const config={...base,packageId,configId,dappStorageId,startCheckpoint:String(published.checkpoint)};
json(join(evidence,'driver-config.json'),config);
const env={...process.env,MIR2_PROCUREMENT_SUI_SECRET_KEY:sponsor.getSecretKey(),MIR2_PROCUREMENT_ORACLE_SECRET_HEX:keys.oracle,MIR2_PROCUREMENT_GAS_LEDGER:join(privateDir,'gas-ledger.json')};
const call=(operation,payload)=>dispatch({protocolVersion:1,operation,config,payload},env);
const conditionsBcs='0x0000000000',conditionsHash=hex(hash(Buffer.from(conditionsBcs.slice(2),'hex')));
const buyerRef=hex(hash(Buffer.from('procurement-v1-isolated-buyer'))),sellerRef=hex(hash(Buffer.from('procurement-v1-isolated-seller')));
async function runOrderAction(label,payload){
  const signedFile=join(evidence,label+'.signed.json');let signed;
  if(existsSync(signedFile))signed=JSON.parse(readFileSync(signedFile));else{signed=await call('build',payload);json(signedFile,signed);}
  const receiptFile=join(evidence,label+'.receipt.json');if(existsSync(receiptFile))return JSON.parse(readFileSync(receiptFile));
  await call('submitExact',signed);const result=await finalized(signed.digest);json(receiptFile,result);return result;
}
const scenarios=[];
for(const ending of ['fill','cancel','expire']){
  const file=join(evidence,ending+'-fixture.json');let fixture;
  if(existsSync(file))fixture=JSON.parse(readFileSync(file));else{
    const clock=await call('readClock',{}),expiry=BigInt(clock.timestampMs)+(ending==='expire'?90000n:600000n);
    const intent={domain:INTENT_DOMAIN,network_hash:config.networkHash,package_id:packageId,dapp_storage_id:dappStorageId,operation:1,
      order_id:hex(randomBytes(32)),fill_id:ZERO,buyer_ref:buyerRef,seller_ref:ZERO,template_id:245,conditions_hash:conditionsHash,quantity:'1',price_gold:'500000',
      expires_at_ms:String(expiry),rule_version:'1',nonce:hex(randomBytes(32)),valid_until_ms:String(expiry),actor_public_key:publicFromSeed(keys.buyer)};
    const proof={domain:ORACLE_DOMAIN,intent_hash:hex(hash(intentBytes(intent))),asset_lock_id:hex(randomBytes(32)),asset_instance_hash:ZERO,oracle_key_id:'1',valid_until_ms:intent.valid_until_ms,fence:'0'};
    fixture={intent,proof,actorSignature:signHash(intentBytes(intent),keys.buyer),custodyFixtureOnly:true};json(file,fixture);
  }
  const created=await runOrderAction(ending+'-create',{action:'create',...fixture});
  const orderObjectId=created.objectChanges.find(o=>o.type==='created'&&o.objectType===base.frameworkPackageId+'::dapp_service::ObjectStorage<'+packageId+'::procurement_system::Order>')?.objectId;
  if(!orderObjectId)throw Error('Order object not in creation effects');
  const activated=await runOrderAction(ending+'-activate',{action:'activate',orderObjectId,conditionsBcs,...fixture});
  let result;
  if(ending==='expire'){
    for(;;){const clock=await call('readClock',{});if(BigInt(clock.timestampMs)>=BigInt(fixture.intent.expires_at_ms))break;await wait(Math.min(10000,Number(BigInt(fixture.intent.expires_at_ms)-BigInt(clock.timestampMs))));}
    result=await runOrderAction(ending+'-terminal',{action:'expire',orderObjectId});
  }else{
    const operation=ending==='fill'?2:3,actorSeed=ending==='fill'?keys.seller:keys.buyer;
    const intent={...fixture.intent,operation,nonce:hex(randomBytes(32)),actor_public_key:publicFromSeed(actorSeed),
      fill_id:operation===2?hex(randomBytes(32)):ZERO,seller_ref:operation===2?sellerRef:ZERO};
    const proof={...fixture.proof,intent_hash:hex(hash(intentBytes(intent))),
      asset_lock_id:operation===2?hex(randomBytes(32)):fixture.proof.asset_lock_id,
      asset_instance_hash:operation===2?hex(hash(Buffer.from('full-original-instance-with-durability-and-added-stats'))):ZERO};
    result=await runOrderAction(ending+'-terminal',{action:ending,orderObjectId,intent,proof,actorSignature:signHash(intentBytes(intent),actorSeed)});
  }
  // A previously verified terminal observation cannot revert to OPEN. Resume
  // subsequent scenarios without re-reading or spending on completed ones.
  const observationFile=join(evidence,ending+'-observation.json');
  const observation=existsSync(observationFile)?JSON.parse(readFileSync(observationFile)):await call('queryOrder',{orderObjectId});
  if(!existsSync(observationFile))json(observationFile,observation);
  const expected={fill:2,cancel:3,expire:4}[ending];if(observation.status!==expected)throw Error('Wrong terminal state');
  scenarios.push({ending,orderId:fixture.intent.order_id,orderObjectId,transactions:[created.digest,activated.digest,result.digest],transactionCount:3,
    gasUsed:[created.effects.gasUsed,activated.effects.gasUsed,result.effects.gasUsed],status:observation.status});
}
const replay=await call('replayEvents',{limit:50});json(join(evidence,'replay-first-page.json'),replay);
json(join(evidence,'result.json'),{passed:true,network:'testnet',custodyFixtureOnly:true,realDatabaseDelivery:false,scenarios,
  publicationTransactionCount:2,initializationCostsSeparate:true,serviceFeeUsdProfitabilityVerified:false,config});
console.log(JSON.stringify({passed:true,network:'testnet',scenarioCount:scenarios.length,packageId,evidenceDirectory:evidence}));
