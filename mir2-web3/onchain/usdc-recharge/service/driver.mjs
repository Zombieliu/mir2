import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync,openSync,closeSync,fsyncSync,renameSync,unlinkSync} from 'node:fs';
import {isAbsolute} from 'node:path';
import { address,assertDirectTransfer,assertPaymentBalanceChanges,assertTerms,decodeConfig,decodeInvoice,digest,DIRECT_ORDER_KEYS,exactKeys,finalized,GENESIS_DIGEST,hash32,integer,matchingEvent,NETWORK_HASH,objectEffect,protobufTimestampMs,Reader,validateConfig,validateDirectOrder,validateOrder } from './model.mjs';

const unknown=()=>({state:'unknown'});
const txMask=['digest','checkpoint','effects','events','transaction','timestamp','balance_changes'];
const id=v=>`0x${String(v).replace(/^0x/,'').padStart(64,'0').toLowerCase()}`;
function isMissing(e){return ['NOT_FOUND','UNAVAILABLE','DEADLINE_EXCEEDED'].includes(e?.code);}
async function getTx(client,d){digest(d);try{const{response}=await client.ledgerService.getTransaction({digest:d,readMask:{paths:txMask}});return response.transaction??null;}catch(e){if(isMissing(e))return null;throw e;}}
async function getObject(client,objectId){try{return(await client.core.getObject({objectId,include:{content:true,previousTransaction:true}})).object;}catch(e){if(isMissing(e))return null;throw e;}}
function shared(o,type,c){if(!o||o.type!==c.packageId+'::recharge::'+type||o.owner?.$kind!=='Shared'||!o.content||!o.previousTransaction)throw Error('usdcChainObjectInvalid');return o;}
function sameObject(a,b){return a?.objectId===b?.objectId&&a?.version===b?.version&&a?.digest===b?.digest&&a?.previousTransaction===b?.previousTransaction;}
export async function load(c,providedClient){
  validateConfig(c);
  const {SuiGrpcClient}=await import('@mysten/sui/grpc');
  const {fromBase58}=await import('@mysten/sui/utils');
  const client=providedClient??new SuiGrpcClient({network:'testnet',baseUrl:c.fullnodeUrl});
  const genesis=(await client.core.getChainIdentifier()).chainIdentifier;
  if(genesis!==GENESIS_DIGEST||Buffer.from(fromBase58(genesis)).subarray(0,4).toString('hex')!==c.chainIdentifier)throw Error('usdcWrongNetwork');
  const [packageObject,cfg,cap]=await Promise.all([getObject(client,c.packageId),getObject(client,c.configId),getObject(client,c.adminCapId)]);
  if(packageObject?.type!=='package')throw Error('usdcPackageInvalid');
  shared(cfg,'Config',c);const decoded=decodeConfig(cfg.content);
  for(const k of ['configId','adminCapId','issuerAddress','networkHash','coinType','merchantAddress'])if(decoded[k]!==c[k])throw Error('usdcConfigBindingMismatch');
  if(cap?.type!==c.packageId+'::recharge::AdminCap'||cap?.owner?.$kind!=='AddressOwner'||cap.owner.AddressOwner!==c.issuerAddress)throw Error('usdcIssuerCapabilityInvalid');
  const r=new Reader(cap.content??[]);if(r.address()!==c.adminCapId||r.address()!==c.issuerAddress)throw Error('usdcIssuerCapabilityInvalid');r.done();
  return {client,configObject:cfg,config:decoded,adminCap:cap};
}
function signer(c,env){
  const secret=env.MIR2_SUI_USDC_ISSUER_SECRET_KEY;
  if(typeof secret!=='string'||!secret.startsWith('suiprivkey'))throw Error('usdcIssuerKeyMissing');return secret;
}
async function issuer(c,env){
  const {decodeSuiPrivateKey}=await import('@mysten/sui/cryptography');
  const {Ed25519Keypair}=await import('@mysten/sui/keypairs/ed25519');
  const decoded=decodeSuiPrivateKey(signer(c,env));if(decoded.scheme!=='ED25519')throw Error('usdcIssuerKeyInvalid');
  const key=Ed25519Keypair.fromSecretKey(decoded.secretKey);if(key.toSuiAddress()!==c.issuerAddress)throw Error('usdcIssuerKeyMismatch');return key;
}
function vectorArg(v){const b=Buffer.from(hash32(v).slice(2),'hex');return Buffer.concat([Buffer.from([32]),b]).toString('base64');}
function u64Arg(v){const b=Buffer.alloc(8);b.writeBigUInt64LE(integer(v));return b.toString('base64');}
function inputOf(data,arg){if(arg?.$kind!=='Input'||!Number.isInteger(arg.Input)||!data.inputs[arg.Input])throw Error('usdcPreparedArgumentsInvalid');return data.inputs[arg.Input];}
function objectId(input){return id(input.Object?.ImmOrOwnedObject?.objectId??input.Object?.SharedObject?.objectId??'');}
export async function validatePrepared(c,p,o){
  validateConfig(c);validateOrder(o,c);exactKeys(p,['bytesB64','signature','digest']);digest(p.digest);
  if(typeof p.bytesB64!=='string'||typeof p.signature!=='string'||p.signature.length>4096)throw Error('usdcPreparedInvalid');
  const raw=Buffer.from(p.bytesB64,'base64');if(raw.length>65536||raw.toString('base64')!==p.bytesB64)throw Error('usdcPreparedInvalid');
  const {Transaction}=await import('@mysten/sui/transactions');
  const tx=Transaction.from(raw),d=tx.getData();
  if(await tx.getDigest()!==p.digest||d.sender!==c.issuerAddress||id(d.gasData?.owner??d.sender)!==c.issuerAddress
    ||d.gasData?.budget!==c.gasBudgetMist||d.commands?.length!==1)throw Error('usdcPreparedDomainMismatch');
  const call=d.commands[0]?.MoveCall;
  if(!call||id(call.package)!==c.packageId||call.module!=='recharge'||call.function!=='create_invoice'||call.typeArguments?.length||call.arguments?.length!==9)throw Error('usdcPreparedCommandInvalid');
  const inputs=call.arguments.map(a=>inputOf(d,a));
  const expectedObjects=[c.adminCapId,c.configId,id('0x6')];
  for(let i=0;i<3;i++)if(objectId(inputs[i])!==expectedObjects[i])throw Error('usdcPreparedObjectMismatch');
  const expectedPure=[vectorArg(o.orderRef),vectorArg(o.accountRef),vectorArg(o.snapshotHash),Buffer.from(o.payer.slice(2),'hex').toString('base64'),u64Arg(o.amountMicros),u64Arg(o.expiresAtMs)];
  for(let i=0;i<6;i++)if(inputs[i+3]?.Pure?.bytes!==expectedPure[i])throw Error('usdcPreparedOrderMismatch');
  const {verifyTransactionSignature}=await import('@mysten/sui/verify');
  if((await verifyTransactionSignature(raw,p.signature)).toSuiAddress()!==c.issuerAddress)throw Error('usdcPreparedSignatureInvalid');
  return {raw,tx};
}
export async function prepareInvoice(c,p,env=process.env,providedClient){
  exactKeys(p,['order']);const o=validateOrder(p.order,c);const l=await load(c,providedClient);
  if(!l.config.publishingEnabled)throw Error('usdcNewInvoicesDisabled');
  const key=await issuer(c,env),{Transaction}=await import('@mysten/sui/transactions');
  const tx=new Transaction();tx.setSender(c.issuerAddress);tx.setGasBudget(c.gasBudgetMist);
  tx.moveCall({target:c.packageId+'::recharge::create_invoice',arguments:[tx.object(c.adminCapId),tx.object(c.configId),tx.object('0x6'),
    ...['orderRef','accountRef','snapshotHash'].map(k=>tx.pure.vector('u8',[...Buffer.from(o[k].slice(2),'hex')])),
    tx.pure.address(o.payer),tx.pure.u64(o.amountMicros),tx.pure.u64(o.expiresAtMs)]});
  const raw=await tx.build({client:l.client});const signature=(await key.signTransaction(raw)).signature;
  const result={bytesB64:Buffer.from(raw).toString('base64'),signature,digest:await Transaction.from(raw).getDigest()};
  await validatePrepared(c,result,o);return result;
}
export function validateGasEnvironment(env){
  const path=env.MIR2_SUI_USDC_GAS_LEDGER_PATH;
  if(typeof path!=='string'||!isAbsolute(path)||path.length>4096)throw Error('usdcGasLedgerPathRequired');
  const maximum=env.MIR2_SUI_USDC_GAS_MAX_MIST??'50000000';
  if(integer(maximum)===0n||integer(maximum)>1000000000n)throw Error('usdcGasLifetimeBudgetInvalid');
  return {path,maximum};
}
// Reserve the full maximum BEFORE broadcast. Reservations never reset by day,
// process restart, success, failure or unknown result. A duplicate exact digest
// consumes no new budget. A crashed exclusive lock fails closed for operator
// reconciliation, rather than silently issuing an additional transaction.
export function reserveIssuerGas(c,p,env=process.env){
  validateConfig(c);digest(p.digest);const {path,maximum}=validateGasEnvironment(env);
  const domain={version:1,network:'testnet',chainIdentifier:c.chainIdentifier,genesisDigest:GENESIS_DIGEST,
    networkHash:c.networkHash,packageId:c.packageId,configId:c.configId,issuerAddress:c.issuerAddress,
    gasBudgetMist:c.gasBudgetMist,maxLifetimeBudgetMist:maximum};
  const sha=v=>createHash('sha256').update(v).digest('hex');
  const reservation={maximumMist:c.gasBudgetMist,transactionSha256:sha(Buffer.from(p.bytesB64,'base64')),signatureSha256:sha(p.signature)};
  const lock=path+'.lock';let fd;
  try{fd=openSync(lock,'wx');}catch{throw Error('usdcGasLedgerLocked');}
  try{
    let ledger;
    try{const raw=readFileSync(path);if(raw.length>1048576)throw Error('usdcGasLedgerInvalid');ledger=JSON.parse(raw.toString('utf8'));}
    catch(e){if(e.code!=='ENOENT')throw Error('usdcGasLedgerInvalid');ledger={...domain,reservations:{}};}
    for(const [k,v]of Object.entries(domain))if(ledger[k]!==v)throw Error('usdcGasLedgerDomainMismatch');
    if(!ledger.reservations||typeof ledger.reservations!=='object'||Array.isArray(ledger.reservations))throw Error('usdcGasLedgerInvalid');
    let reserved=0n;
    for(const [d,r]of Object.entries(ledger.reservations)){
      digest(d);exactKeys(r,['maximumMist','transactionSha256','signatureSha256']);
      if(r.maximumMist!==c.gasBudgetMist||!/^([a-f0-9]{64})$/.test(r.transactionSha256)||!/^([a-f0-9]{64})$/.test(r.signatureSha256))throw Error('usdcGasLedgerInvalid');
      reserved+=integer(r.maximumMist);
    }
    if(reserved>integer(maximum))throw Error('usdcGasLedgerInvalid');
    if(Object.hasOwn(ledger.reservations,p.digest)){
      const previous=ledger.reservations[p.digest];
      if(Object.keys(reservation).some(k=>previous[k]!==reservation[k]))throw Error('usdcGasReservationMismatch');
      return {reservedMist:reserved.toString(),maximumMist:maximum,duplicate:true};
    }
    const total=reserved+integer(c.gasBudgetMist);
    if(total>integer(maximum))throw Error('usdcGasLifetimeBudgetExceeded');
    ledger.reservations[p.digest]=reservation;
    const output=Buffer.from(JSON.stringify(ledger));if(output.length>1048576)throw Error('usdcGasLedgerFull');
    const temp=path+'.'+process.pid+'.tmp';const out=openSync(temp,'wx');
    try{writeFileSync(out,output);fsyncSync(out);}finally{closeSync(out);}
    renameSync(temp,path);
    return {reservedMist:total.toString(),maximumMist:maximum,duplicate:false};
  }finally{closeSync(fd);unlinkSync(lock);}
}
export async function submitPrepared(c,p,env=process.env,providedClient){
  exactKeys(p,['prepared','order']);const {raw}=await validatePrepared(c,p.prepared,p.order);const{client}=await load(c,providedClient);
  // The only broadcast operation, and only the durably saved exact issuer create.
  // This operation never accepts a client-provided general transaction to sponsor.
  reserveIssuerGas(c,p.prepared,env);
  const result=await client.core.executeTransaction({transaction:raw,signatures:[p.prepared.signature],include:{effects:true,events:true}});
  const tx=result.Transaction??result.FailedTransaction;
  if(tx?.digest!==p.prepared.digest)throw Error('usdcSubmissionDigestMismatch');return {submitted:true,digest:tx.digest};
}
export async function observeCreateWithClient(client,c,p){
  const o=validateOrder(p.order,c);const tx=await getTx(client,p.prepared.digest);
  if(!finalized(tx,p.prepared.digest))return unknown();
  if(!tx.effects.status.success)return {state:'failed',digest:tx.digest,checkpoint:String(tx.checkpoint)};
  const event=matchingEvent(tx,c,'created',o);objectEffect(tx,c,event.invoiceId,true);
  const first=shared(await getObject(client,event.invoiceId),'Invoice',c);const invoice=assertTerms(decodeInvoice(first.content),o,c);
  if(invoice.invoiceId!==event.invoiceId)throw Error('usdcInvoiceObjectIdMismatch');
  const second=await getObject(client,event.invoiceId);if(!sameObject(first,second))return unknown();
  return {state:'created',invoiceId:event.invoiceId,digest:tx.digest,checkpoint:String(tx.checkpoint)};
}
export async function observeCreate(c,p,env=process.env,providedClient){
  exactKeys(p,['prepared','order']);await validatePrepared(c,p.prepared,p.order);const{client}=await load(c,providedClient);
  return observeCreateWithClient(client,c,p);
}
export async function observeInvoiceWithClient(client,c,p){
  address(p.invoiceId);const o=validateOrder(p.order,c),first=await getObject(client,p.invoiceId);if(!first)return unknown();
  shared(first,'Invoice',c);const invoice=assertTerms(decodeInvoice(first.content),o,c);
  if(invoice.invoiceId!==p.invoiceId)throw Error('usdcInvoiceObjectIdMismatch');
  const tx=await getTx(client,first.previousTransaction);
  if(!finalized(tx,first.previousTransaction))return unknown();
  if(!tx.effects.status.success)throw Error('usdcInvoiceFailedMutation');
  const effect=objectEffect(tx,c,p.invoiceId,!invoice.paid);
  if(String(effect.outputVersion)!==first.version||effect.outputDigest!==first.digest)throw Error('usdcInvoiceEffectsMismatch');
  let event;
  if(invoice.paid){event=matchingEvent(tx,c,'paid',o,p.invoiceId);if(event.paidAtMs!==invoice.paidAtMs)throw Error('usdcPaidTimeMismatch');assertPaymentBalanceChanges(tx,o,c);}
  else matchingEvent(tx,c,'created',o,p.invoiceId);
  const second=await getObject(client,p.invoiceId);if(!sameObject(first,second))return unknown();
  if(!invoice.paid)return {state:'unpaid',invoiceId:p.invoiceId,expiresAtMs:o.expiresAtMs,checkpoint:String(tx.checkpoint)};
  const {paid,...receipt}=invoice;
  return {state:'paid',receipt:{...receipt,packageId:c.packageId,transactionDigest:tx.digest,eventSequence:event.eventSequence,checkpoint:String(tx.checkpoint)}};
}
export async function observeInvoice(c,p,env=process.env,providedClient){
  exactKeys(p,['invoiceId','order']);const{client}=await load(c,providedClient);return observeInvoiceWithClient(client,c,p);
}
async function payTransaction(c,p,providedClient){
  exactKeys(p,['invoiceId','order']);const o=validateOrder(p.order,c),l=await load(c,providedClient);
  const observed=await observeInvoiceWithClient(l.client,c,p);if(observed.state!=='unpaid')throw Error('usdcInvoiceNotPayable');
  const inv=shared(await getObject(l.client,p.invoiceId),'Invoice',c);
  const invoice=assertTerms(decodeInvoice(inv.content),o,c);
  let coins=[],cursor=null,total=0n;
  for(let page=0;page<8&&total<integer(o.amountMicros);page++){
    const response=await l.client.core.listCoins({owner:o.payer,coinType:c.coinType,limit:50,cursor});
    for(const coin of response.objects){
      const type=coin.type.replace(/0x0{1,63}2::/g,'0x2::');
      if(type!=='0x2::coin::Coin<'+c.coinType+'>'||coin.owner?.$kind!=='AddressOwner'||coin.owner.AddressOwner!==o.payer)throw Error('usdcCoinOwnershipMismatch');
      integer(coin.balance);if(BigInt(coin.balance)>0n){coins.push(coin);total+=BigInt(coin.balance);}
      if(total>=integer(o.amountMicros))break;
      if(coins.length>=32)throw Error('usdcTooManyCoinObjects');
    }
    if(!response.hasNextPage)break;
    if(!response.cursor||response.cursor===cursor)throw Error('usdcCoinCursorInvalid');cursor=response.cursor;
  }
  if(total<integer(o.amountMicros))throw Error('usdcInsufficientBalance');
  const {Transaction}=await import('@mysten/sui/transactions');const tx=new Transaction();tx.setSender(o.payer);tx.setGasBudget(c.gasBudgetMist);
  const refs=coins.map(v=>tx.objectRef({objectId:v.objectId,version:v.version,digest:v.digest}));
  if(refs.length>1)tx.mergeCoins(refs[0],refs.slice(1));
  const [payment]=tx.splitCoins(refs[0],[tx.pure.u64(o.amountMicros)]);
  tx.moveCall({target:c.packageId+'::recharge::pay',typeArguments:[c.coinType],arguments:[
    tx.sharedObjectRef({objectId:c.configId,initialSharedVersion:l.configObject.owner.Shared.initialSharedVersion,mutable:false}),
    tx.sharedObjectRef({objectId:p.invoiceId,initialSharedVersion:inv.owner.Shared.initialSharedVersion,mutable:true}),payment,tx.object('0x6')]});
  tx.transferObjects([refs[0]],tx.pure.address(o.payer));
  return {tx,order:o,loaded:l,invoice};
}
export async function buildPay(c,p,env=process.env,providedClient){
  const {tx,order:o}=await payTransaction(c,p,providedClient);
  // No issuer gas owner, no signature and no execution. Wallet displays and
  // authorizes the complete exact USDC transfer, and supplies its own SUI gas.
  return {transactionJson:await tx.toJSON(),invoiceId:p.invoiceId,payer:o.payer,gasPayer:'wallet'};
}
async function validatePayGas(client,data,o,c){
  if(data.sender!==o.payer||data.gasData?.owner!==o.payer||data.gasData.budget!==c.gasBudgetMist
    ||integer(data.gasData.price)===0n||!Array.isArray(data.gasData.payment))throw Error('usdcPayBytesDomainMismatch');
  const budget=integer(c.gasBudgetMist),payments=data.gasData.payment;
  if(payments.length===0){
    const result=await client.core.getBalance({owner:o.payer,coinType:'0x2::sui::SUI'});
    if(integer(result.balance.addressBalance)<budget)throw Error('usdcInsufficientGasBalance');
    return;
  }
  let balance=0n;const seen=new Set();
  for(const ref of payments){
    address(ref.objectId);if(seen.has(ref.objectId)||integer(ref.version)===0n)throw Error('usdcPayGasObjectInvalid');seen.add(ref.objectId);
    const coin=await getObject(client,ref.objectId);
    if(!coin||coin.objectId!==ref.objectId||coin.version!==ref.version||coin.digest!==ref.digest
      ||coin.owner?.$kind!=='AddressOwner'||coin.owner.AddressOwner!==o.payer
      ||coin.type?.replace(/0x0{1,63}2::/g,'0x2::')!=='0x2::coin::Coin<0x2::sui::SUI>')throw Error('usdcPayGasOwnershipMismatch');
    const reader=new Reader(coin.content??[]);if(reader.address()!==ref.objectId)throw Error('usdcPayGasObjectInvalid');
    balance+=integer(reader.u64());reader.done();
  }
  if(balance<budget)throw Error('usdcInsufficientGasBalance');
}
export async function buildPayBytes(c,p,env=process.env,providedClient){
  const {tx,order:o,loaded:l,invoice}=await payTransaction(c,p,providedClient);
  if(invoice.paid||BigInt(Date.now())>=integer(o.expiresAtMs))throw Error('usdcInvoiceNotPayable');
  // Only the already-bound payer supplies gas. This operation never loads an
  // issuer key, signs, sponsors or submits a transaction.
  tx.setGasOwner(o.payer);
  const raw=Buffer.from(await tx.build({client:l.client}));
  if(raw.length===0||raw.length>65536)throw Error('usdcPayBytesTooLarge');
  const {Transaction}=await import('@mysten/sui/transactions'),parsed=Transaction.from(raw);
  if(!raw.equals(Buffer.from(await parsed.build())))throw Error('usdcPayBytesNonCanonical');
  await validatePayGas(l.client,parsed.getData(),o,c);
  const observed=await observeInvoiceWithClient(l.client,c,p);
  if(observed.state!=='unpaid'||BigInt(Date.now())>=integer(o.expiresAtMs))throw Error('usdcInvoiceNotPayable');
  const transactionBytesB64=raw.toString('base64'),transactionDigest=await parsed.getDigest();digest(transactionDigest);
  return {transactionBytesB64,transactionDigest,invoiceId:p.invoiceId,payer:o.payer,gasPayer:'wallet'};
}
export async function verifyWalletProof(p){
  exactKeys(p,['message','signature','address']);address(p.address);
  if(typeof p.message!=='string'||Buffer.byteLength(p.message)>16384||typeof p.signature!=='string'||p.signature.length>4096)throw Error('usdcWalletProofInvalid');
  const {verifyPersonalMessageSignature}=await import('@mysten/sui/verify');
  const key=await verifyPersonalMessageSignature(Buffer.from(p.message,'utf8'),p.signature,{address:p.address});
  if(key.toSuiAddress()!==p.address)throw Error('usdcWalletAddressMismatch');return {verified:true,address:p.address};
}

export async function observeDirectTransfer(c,p,env=process.env,providedClient){
  exactKeys(p,['order','digest']);validateConfig(c);const o=validateDirectOrder(p.order,c);digest(p.digest);
  // load validates the full public merchant/package configuration and genesis,
  // but never reads issuer authority or performs a signing/broadcast operation.
  const {client}=await load(c,providedClient);
  let tx,checkpoint;
  try{
    const result=await client.ledgerService.getTransaction({digest:p.digest,readMask:{paths:[...txMask,'objects']}});
    tx=result.response.transaction;
    if(!finalized(tx,p.digest)||integer(String(tx.checkpoint))===0n)return unknown();
    const resultCheckpoint=await client.ledgerService.getCheckpoint({checkpointId:{oneofKind:'sequenceNumber',sequenceNumber:tx.checkpoint},readMask:{paths:['sequence_number','digest','summary.sequence_number','summary.timestamp','contents.transactions.transaction','contents.transactions.effects']}});
    checkpoint=resultCheckpoint.response.checkpoint;
  }catch{return unknown();}
  const inclusion=checkpoint?.contents?.transactions?.filter(info=>info.transaction===p.digest);
  if(!checkpoint||String(checkpoint.sequenceNumber)!==String(tx.checkpoint)||String(checkpoint.summary?.sequenceNumber)!==String(tx.checkpoint)||
    inclusion?.length!==1||!tx.effects.digest||inclusion[0].effects!==tx.effects.digest||tx.effects.transactionDigest!==p.digest)return unknown();
  let paidAtMs;
  try{
    paidAtMs=protobufTimestampMs(checkpoint.summary?.timestamp);
    if(protobufTimestampMs(tx.timestamp)!==paidAtMs)return unknown();
  }catch{return unknown();}
  if(!tx.effects.status.success)return {state:'unpaid'};
  if(integer(paidAtMs)<integer(o.createdAtMs)||integer(paidAtMs)>=integer(o.expiresAtMs))return {state:'unpaid'};
  try{
    const raw=tx.transaction?.bcs?.value;
    if(!(raw instanceof Uint8Array)||!raw.length||raw.length>65536)return unknown();
    const {Transaction}=await import('@mysten/sui/transactions');
    const parsed=Transaction.from(raw);
    if(await parsed.getDigest()!==p.digest)return unknown();
    if(tx.transaction.sender!==parsed.getData().sender)return unknown();
    if(tx.effects.changedObjects?.some(effect=>effect.outputState===4))await verifyDirectAccumulatorEffects(tx);
    assertDirectTransfer(tx,parsed.getData(),o,c);
  }catch(error){
    if(['usdcDirectObjectsUnavailable','usdcDirectUnsupported','usdcBalanceChangesUnavailable','usdcCoinTypeInvalid'].includes(error?.message))return unknown();
    return {state:'unpaid'};
  }
  return {state:'paid',receipt:{genesisDigest:GENESIS_DIGEST,transactionDigest:p.digest,checkpoint:String(tx.checkpoint),paidAtMs,
    payer:o.payer,merchant:o.merchant,coinType:c.coinType,amountMicros:o.amountMicros}};
}

async function verifyDirectAccumulatorEffects(tx){
  const raw=tx.effects.bcs?.value;
  if(!(raw instanceof Uint8Array)||!raw.length||raw.length>262144)throw Error('usdcDirectObjectsUnavailable');
  const [{bcs},{blake2b},{toBase58}]=await Promise.all([import('@mysten/sui/bcs'),import('@noble/hashes/blake2.js'),import('@mysten/sui/utils')]);
  if(toBase58(blake2b(Buffer.concat([Buffer.from('TransactionEffects::'),raw]),{dkLen:32}))!==tx.effects.digest)throw Error('usdcDirectObjectsUnavailable');
  let decoded;
  try{decoded=bcs.TransactionEffects.parse(raw);}catch{throw Error('usdcDirectObjectsUnavailable');}
  const effects=decoded.V2;
  if(!effects||effects.status.$kind!=='Success'||effects.transactionDigest!==tx.digest||effects.eventsDigest!==null||
    !Buffer.from(bcs.TransactionEffects.serialize(decoded).toBytes()).equals(Buffer.from(raw)))throw Error('usdcDirectObjectsUnavailable');
  for(const key of ['computationCost','storageCost','storageRebate','nonRefundableStorageFee']){
    if(String(tx.effects.gasUsed?.[key])!==effects.gasUsed[key])throw Error('usdcDirectObjectsUnavailable');
  }
  const accumulators=effects.changedObjects.filter(([,change])=>change.outputState.$kind==='AccumulatorWriteV1');
  const indexed=tx.effects.changedObjects.filter(change=>change.outputState===4);
  if(accumulators.length!==indexed.length)throw Error('usdcDirectObjectsUnavailable');
  const ownerMatches=(raw,indexed)=>{
    if(raw?.$kind==='AddressOwner')return indexed?.kind===1&&indexed.address===raw.AddressOwner;
    if(raw?.$kind==='ObjectOwner')return indexed?.kind===2&&indexed.address===raw.ObjectOwner;
    if(raw?.$kind==='Shared')return indexed?.kind===3&&String(indexed.version)===raw.Shared.initialSharedVersion;
    if(raw?.$kind==='Immutable')return indexed?.kind===4;
    if(raw?.$kind==='ConsensusAddressOwner')return indexed?.kind===5&&indexed.address===raw.ConsensusAddressOwner.owner&&String(indexed.version)===raw.ConsensusAddressOwner.startVersion;
    return false;
  };
  if(effects.changedObjects.length!==tx.effects.changedObjects.length)throw Error('usdcDirectObjectsUnavailable');
  for(const [objectId,change]of effects.changedObjects){
    const matches=tx.effects.changedObjects.filter(effect=>effect.objectId===objectId);
    if(matches.length!==1)throw Error('usdcDirectObjectsUnavailable');
    const effect=matches[0],input=change.inputState,output=change.outputState;
    if(effect.idOperation!==({None:1,Created:2,Deleted:3})[change.idOperation.$kind])throw Error('usdcDirectObjectsUnavailable');
    if(input.$kind==='NotExist'){
      if(effect.inputState!==1)throw Error('usdcDirectObjectsUnavailable');
    }else if(input.$kind!=='Exist'||effect.inputState!==2||String(effect.inputVersion)!==input.Exist[0][0]||effect.inputDigest!==input.Exist[0][1]||!ownerMatches(input.Exist[1],effect.inputOwner))throw Error('usdcDirectObjectsUnavailable');
    if(output.$kind==='NotExist'){
      if(effect.outputState!==1)throw Error('usdcDirectObjectsUnavailable');
    }else if(output.$kind==='ObjectWrite'){
      if(effect.outputState!==2||String(effect.outputVersion)!==effects.lamportVersion||effect.outputDigest!==output.ObjectWrite[0]||!ownerMatches(output.ObjectWrite[1],effect.outputOwner))throw Error('usdcDirectObjectsUnavailable');
    }else if(output.$kind==='AccumulatorWriteV1'){
      if(effect.outputState!==4)throw Error('usdcDirectObjectsUnavailable');
    }else throw Error('usdcDirectObjectsUnavailable');
  }
  const type=value=>typeof value==='string'?value.replace(/0x0{1,63}2::/g,'0x2::'):null;
  for(const [objectId,change]of accumulators){
    const rawWrite=change.outputState.AccumulatorWriteV1,matches=indexed.filter(effect=>effect.objectId===objectId);
    if(matches.length!==1||rawWrite.value.$kind!=='Integer')throw Error('usdcDirectObjectsUnavailable');
    const write=matches[0].accumulatorWrite;
    if(write?.address!==rawWrite.address.address||type(write.accumulatorType)!==type(rawWrite.address.ty)||
      write.operation!==(rawWrite.operation.$kind==='Merge'?1:2)||String(write.value)!==rawWrite.value.Integer)throw Error('usdcDirectObjectsUnavailable');
  }
}

export const DIRECT_GRAPHQL_URL='https://graphql.testnet.sui.io/graphql';
// Four 19s observation subprocesses plus discovery fit the 120s job lease even
// at their worst-case bound. Pagination continues until the entire window ends.
export const DIRECT_DISCOVERY_PAGE_SIZE=4;
const INDEX_QUERY=`query DirectIndex {
  chainIdentifier genesis:checkpoint(sequenceNumber:0){digest}
  serviceConfig{availableRange(type:"Query",field:"transactions",filters:["sentAddress"]){first{sequenceNumber timestamp}last{sequenceNumber timestamp}}}
}`;
const PAGE_QUERY=`query DirectTransfers($payer:SuiAddress!,$from:UInt53!,$through:UInt53!,$after:String){
  transactions(first:${DIRECT_DISCOVERY_PAGE_SIZE},after:$after,filter:{sentAddress:$payer,afterCheckpoint:$from,beforeCheckpoint:$through}){
    pageInfo{hasNextPage endCursor}nodes{digest sender{address}effects{timestamp checkpoint{sequenceNumber}}}
  }
}`;
const CHECKPOINT_QUERY='query DirectCheckpoint($sequence:UInt53!){checkpoint(sequenceNumber:$sequence){sequenceNumber timestamp}}';
function indexNumber(value){
  const n=typeof value==='number'&&Number.isSafeInteger(value)?BigInt(value):typeof value==='string'?integer(value):null;
  if(n==null||n<0n||n>=BigInt(Number.MAX_SAFE_INTEGER))throw Error('usdcDirectIndexInvalid');return n;
}
function indexTime(value){
  if(typeof value!=='string'||!/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,3})?Z$/.test(value))throw Error('usdcDirectIndexInvalid');
  const ms=Date.parse(value);if(!Number.isSafeInteger(ms)||ms<0)throw Error('usdcDirectIndexInvalid');return BigInt(ms);
}
function indexPoint(value){
  return {sequence:indexNumber(value?.sequenceNumber),time:indexTime(value?.timestamp)};
}
async function directGraphql(query,variables,fetcher,deadline){
  const remaining=deadline-Date.now();if(remaining<=0)throw Error('usdcDirectIndexUnavailable');
  const response=await fetcher(DIRECT_GRAPHQL_URL,{method:'POST',redirect:'error',credentials:'omit',headers:{'content-type':'application/json'},
    body:JSON.stringify({query,variables}),signal:AbortSignal.timeout(Math.min(4000,remaining))});
  if(!response.ok)throw Error('usdcDirectIndexUnavailable');
  // Stream with a hard bound; a large public response must not exhaust a CLI.
  let size=0;const chunks=[];
  for await(const chunk of response.body){size+=chunk.byteLength;if(size>1048576)throw Error('usdcDirectIndexInvalid');chunks.push(Buffer.from(chunk));}
  const result=JSON.parse(Buffer.concat(chunks).toString('utf8'));
  if(result.errors?.length||!result.data||typeof result.data!=='object')throw Error('usdcDirectIndexUnavailable');return result.data;
}
function directBinding(o){return createHash('sha256').update(JSON.stringify(DIRECT_ORDER_KEYS.map(key=>o[key]))).digest('hex');}
function parseDirectCursor(value,binding){
  if(value===null)return null;
  if(typeof value!=='string'||value.length>4096)throw Error('usdcDirectCursorInvalid');
  let cursor;try{cursor=JSON.parse(value);}catch{throw Error('usdcDirectCursorInvalid');}
  exactKeys(cursor,['v','binding','from','through','throughMs','after','done','phase','lo','hi']);
  integer(cursor.from);integer(cursor.through);
  if(cursor.phase==='seek'){integer(cursor.lo);integer(cursor.hi);}
  if(cursor.v!==1||cursor.binding!==binding||typeof cursor.done!=='boolean'||
    !(cursor.after===null||typeof cursor.after==='string'&&cursor.after.length>0&&cursor.after.length<=2048&&!/[\x00-\x1f]/.test(cursor.after))||
    indexNumber(cursor.from)>indexNumber(cursor.through)||integer(cursor.throughMs)>BigInt(Number.MAX_SAFE_INTEGER)||cursor.done&&cursor.after!==null||
    !['seek','scan'].includes(cursor.phase)||cursor.phase==='scan'&&(cursor.lo!==null||cursor.hi!==null)||
    cursor.phase==='seek'&&(cursor.done||cursor.after!==null||indexNumber(cursor.lo)>indexNumber(cursor.hi)||indexNumber(cursor.hi)>indexNumber(cursor.through)))throw Error('usdcDirectCursorInvalid');
  return cursor;
}
function discoveryResult(cursor,digests,o){
  return {state:'scanned',digests,cursor:JSON.stringify(cursor),indexedThroughCheckpoint:cursor.through,indexedThroughMs:cursor.throughMs,
    scanComplete:cursor.done,windowComplete:cursor.done&&integer(cursor.throughMs)>=integer(o.expiresAtMs)};
}
export async function discoverDirectTransfers(c,p,env=process.env,providedClient,providedFetch=globalThis.fetch){
  exactKeys(p,['order','cursor']);validateConfig(c);const o=validateDirectOrder(p.order,c),binding=directBinding(o);
  let cursor=parseDirectCursor(p.cursor,binding);
  await load(c,providedClient);
  const deadline=Date.now()+10000;
  let seekProgress=false,pageConsumed=false;
  try{
    const query=(q,variables={})=>directGraphql(q,variables,providedFetch,deadline);
    const metadata=await query(INDEX_QUERY);
    if(metadata.chainIdentifier!==GENESIS_DIGEST||metadata.genesis?.digest!==GENESIS_DIGEST)throw Error('usdcWrongNetwork');
    const first=indexPoint(metadata.serviceConfig?.availableRange?.first),last=indexPoint(metadata.serviceConfig?.availableRange?.last);
    if(first.sequence>last.sequence||first.time>last.time||first.time>integer(o.createdAtMs))return unknown();
    if(cursor){
      if(indexNumber(cursor.from)+1n<first.sequence||indexNumber(cursor.through)>last.sequence||integer(cursor.throughMs)>last.time)return unknown();
      if(cursor.done){
        if(integer(cursor.throughMs)>=integer(o.expiresAtMs)||indexNumber(cursor.through)===last.sequence)return discoveryResult(cursor,[],o);
        cursor={v:1,binding,from:cursor.through,through:last.sequence.toString(),throughMs:last.time.toString(),after:null,done:false,phase:'scan',lo:null,hi:null};
      }
    }else{
      // Find the checkpoint immediately before the creation time. Starting from
      // "last 50" would permanently miss older pages for a busy payer.
      cursor={v:1,binding,from:(first.sequence===0n?0n:first.sequence-1n).toString(),through:last.sequence.toString(),throughMs:last.time.toString(),after:null,done:false,
        phase:'seek',lo:first.sequence.toString(),hi:last.sequence.toString()};
    }
    if(cursor.phase==='seek'){
      let lo=indexNumber(cursor.lo),hi=indexNumber(cursor.hi),from=indexNumber(cursor.from);
      // Persist the binary-search bounds instead of repeating an initial
      // ~30-RPC search whenever a bounded subprocess or network times out.
      for(let step=0;step<8&&lo<=hi;step++){
        if(deadline-Date.now()<1000)break;
        const mid=(lo+hi)/2n,point=indexPoint((await query(CHECKPOINT_QUERY,{sequence:Number(mid)})).checkpoint);
        if(point.sequence!==mid||point.time<first.time||point.time>last.time)return unknown();
        if(point.time<integer(o.createdAtMs)){from=mid;lo=mid+1n;}else hi=mid-1n;
        seekProgress=true;
        cursor={...cursor,from:from.toString(),lo:lo.toString(),hi:hi.toString()};
        if(lo>hi)cursor={...cursor,phase:'scan',lo:null,hi:null};
      }
      if(cursor.phase==='seek')return discoveryResult(cursor,[],o);
    }
    if(cursor.from===cursor.through){cursor.done=true;return discoveryResult(cursor,[],o);}
    const data=await query(PAGE_QUERY,{payer:o.payer,from:Number(indexNumber(cursor.from)),through:Number(indexNumber(cursor.through)+1n),after:cursor.after});
    const page=data.transactions;
    if(!Array.isArray(page?.nodes)||page.nodes.length>DIRECT_DISCOVERY_PAGE_SIZE||typeof page.pageInfo?.hasNextPage!=='boolean')return unknown();
    const {hasNextPage,endCursor}=page.pageInfo;
    if(hasNextPage&&(!page.nodes.length||typeof endCursor!=='string'||!endCursor.length||endCursor.length>2048||endCursor===cursor.after||/[\x00-\x1f]/.test(endCursor)))return unknown();
    const seen=new Set(),digests=[];
    for(const node of page.nodes){
      digest(node.digest);if(seen.has(node.digest)||node.sender?.address!==o.payer)return unknown();seen.add(node.digest);
      const point=indexPoint({...node.effects?.checkpoint,timestamp:node.effects?.timestamp});
      if(point.sequence<=indexNumber(cursor.from)||point.sequence>indexNumber(cursor.through)||point.time>integer(cursor.throughMs))return unknown();
      if(point.time>=integer(o.createdAtMs)&&point.time<integer(o.expiresAtMs))digests.push(node.digest);
    }
    pageConsumed=true;
    cursor={...cursor,after:hasNextPage?endCursor:null,done:!hasNextPage};
    return discoveryResult(cursor,digests,o);
  }catch(error){
    if(error?.message==='usdcWrongNetwork')throw error;
    // The final seek step may already have entered scan before its first page
    // times out. Preserve that verified start without consuming any page.
    if(seekProgress&&!pageConsumed&&cursor?.done===false)return discoveryResult(cursor,[],o);
    // Unknown means retry the SAME persisted cursor. It never means unpaid,
    // and an unavailable/pruned page must not advance a scan watermark.
    return unknown();
  }
}
export async function dispatch(request,env=process.env){
  exactKeys(request,['operation','config','params']);
  if(request.operation==='verifyWalletProof'){validateConfig(request.config);return verifyWalletProof(request.params);}
  validateConfig(request.config);
  if(request.operation==='configureValidate'){exactKeys(request.params,[]);validateGasEnvironment(env);const l=await load(request.config);return {validated:true,network:'testnet',networkHash:NETWORK_HASH,publishingEnabled:l.config.publishingEnabled};}
  const operations={prepareInvoice,submitPrepared,observeCreate,buildPay,buildPayBytes,observeInvoice,observeDirectTransfer,discoverDirectTransfers};
  if(!Object.hasOwn(operations,request.operation))throw Error('usdcOperationInvalid');
  return operations[request.operation](request.config,request.params,env);
}
