import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync,openSync,closeSync,fsyncSync,renameSync,unlinkSync} from 'node:fs';
import {isAbsolute} from 'node:path';
import { address,assertTerms,decodeConfig,decodeInvoice,digest,exactKeys,finalized,GENESIS_DIGEST,hash32,integer,matchingEvent,NETWORK_HASH,objectEffect,Reader,validateConfig,validateOrder } from './model.mjs';

const unknown=()=>({state:'unknown'});
const txMask=['digest','checkpoint','effects','events','transaction','timestamp'];
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
  if(invoice.paid){event=matchingEvent(tx,c,'paid',o,p.invoiceId);if(event.paidAtMs!==invoice.paidAtMs)throw Error('usdcPaidTimeMismatch');}
  else matchingEvent(tx,c,'created',o,p.invoiceId);
  const second=await getObject(client,p.invoiceId);if(!sameObject(first,second))return unknown();
  if(!invoice.paid)return {state:'unpaid',invoiceId:p.invoiceId,expiresAtMs:o.expiresAtMs,checkpoint:String(tx.checkpoint)};
  const {paid,...receipt}=invoice;
  return {state:'paid',receipt:{...receipt,packageId:c.packageId,transactionDigest:tx.digest,eventSequence:event.eventSequence,checkpoint:String(tx.checkpoint)}};
}
export async function observeInvoice(c,p,env=process.env,providedClient){
  exactKeys(p,['invoiceId','order']);const{client}=await load(c,providedClient);return observeInvoiceWithClient(client,c,p);
}
export async function buildPay(c,p,env=process.env,providedClient){
  exactKeys(p,['invoiceId','order']);const o=validateOrder(p.order,c),l=await load(c,providedClient);
  const observed=await observeInvoiceWithClient(l.client,c,p);if(observed.state!=='unpaid')throw Error('usdcInvoiceNotPayable');
  const inv=shared(await getObject(l.client,p.invoiceId),'Invoice',c);
  assertTerms(decodeInvoice(inv.content),o,c);
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
  // No issuer gas owner, no signature and no execution. Wallet displays and
  // authorizes the complete exact USDC transfer, and supplies its own SUI gas.
  return {transactionJson:await tx.toJSON(),invoiceId:p.invoiceId,payer:o.payer,gasPayer:'wallet'};
}
export async function verifyWalletProof(p){
  exactKeys(p,['message','signature','address']);address(p.address);
  if(typeof p.message!=='string'||Buffer.byteLength(p.message)>16384||typeof p.signature!=='string'||p.signature.length>4096)throw Error('usdcWalletProofInvalid');
  const {verifyPersonalMessageSignature}=await import('@mysten/sui/verify');
  const key=await verifyPersonalMessageSignature(Buffer.from(p.message,'utf8'),p.signature,{address:p.address});
  if(key.toSuiAddress()!==p.address)throw Error('usdcWalletAddressMismatch');return {verified:true,address:p.address};
}
export async function dispatch(request,env=process.env){
  exactKeys(request,['operation','config','params']);
  if(request.operation==='verifyWalletProof'){validateConfig(request.config);return verifyWalletProof(request.params);}
  validateConfig(request.config);
  if(request.operation==='configureValidate'){exactKeys(request.params,[]);validateGasEnvironment(env);const l=await load(request.config);return {validated:true,network:'testnet',networkHash:NETWORK_HASH,publishingEnabled:l.config.publishingEnabled};}
  const operations={prepareInvoice,submitPrepared,observeCreate,buildPay,observeInvoice};
  if(!Object.hasOwn(operations,request.operation))throw Error('usdcOperationInvalid');
  return operations[request.operation](request.config,request.params,env);
}
