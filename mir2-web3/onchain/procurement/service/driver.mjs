import { readFileSync, writeFileSync, renameSync, openSync, fsyncSync, closeSync, unlinkSync } from 'node:fs';
import { resolve, isAbsolute } from 'node:path';
import { pathToFileURL } from 'node:url';
import { bytes, hex, hash, u64, intentBytes, proofBytes, signHash, publicFromSeed, validateAttestation, revokeBytes } from './canonical.mjs';
import { createGrpcFacade } from './grpc.mjs';

const actions = new Set(['create','activate','fill','cancel','expire','revoke']);
const functions = new Set(['create_order','activate_order','fill_order','cancel_order','expire_order','revoke_fence']);
export function validateConfig(c) {
  if (!c?.enabled || !['testnet','localnet'].includes(c.network) || c.environmentTag !== 'isolated-test') throw Error('Procurement chain driver disabled outside isolated test environments');
  const url = new URL(c.fullnodeUrl);
  if (url.protocol !== 'https:' && !(c.network === 'localnet' && ['127.0.0.1','localhost','[::1]'].includes(url.hostname) && url.protocol === 'http:')) throw Error('RPC requires HTTPS or loopback localnet');
  if (url.username || url.password || typeof c.chainIdentifier !== 'string' || !/^[0-9a-f]{8}$/.test(c.chainIdentifier)) throw Error('Invalid RPC/chain identifier');
  if (c.network === 'testnet' && c.chainIdentifier !== '4c78adac' || c.chainIdentifier === '35834a8a') throw Error('Network identifier mismatch/mainnet forbidden');
  for (const k of ['networkHash','packageId','frameworkPackageId','dappStorageId','configId','oraclePublicKey']) bytes(c[k]);
  if (c.networkHash !== hex(hash(Buffer.from(c.chainIdentifier)))) throw Error('networkHash must be SHA256(UTF8 chainIdentifier)');
  if(c.eventReplayMode!=null&&!['graphql','checkpoint'].includes(c.eventReplayMode))throw Error('Invalid event replay mode');
  for (const k of ['oracleKeyId','gasBudgetMist','maxGasBudgetMist','dailyGasBudgetMist']) u64(c[k]);
  if (BigInt(c.oracleKeyId) === 0n || BigInt(c.gasBudgetMist) === 0n || BigInt(c.gasBudgetMist) > BigInt(c.maxGasBudgetMist)
    || BigInt(c.maxGasBudgetMist) > BigInt(c.dailyGasBudgetMist)) throw Error('Invalid gas limits');
  return c;
}
function canonicalAddress(v) { return `0x${String(v).replace(/^0x/,'').padStart(64,'0').toLowerCase()}`; }
function matchesDappKey(value,c) {
  // Move type_name strings omit "0x"; legacy JSON displays sometimes added it.
  return value===`${c.packageId.slice(2)}::dapp_key::DappKey` || value===`${c.packageId}::dapp_key::DappKey`;
}
function parsedHex(v) {
  if(Array.isArray(v)){if(v.some(n=>!Number.isInteger(n)||n<0||n>255))throw Error('Invalid chain byte vector');return hex(v);}
  return hex(bytes(v));
}
function decodedPure(input) {
  if (input?.Pure?.bytes) return Buffer.from(input.Pure.bytes,'base64');
  throw Error('Expected pure transaction input');
}
export function validateCommands(data, c) {
  const commands = data.commands;
  if (!Array.isArray(commands) || commands.length < 1 || commands.length > 3) throw Error('Invalid command count');
  let systemCalls = 0;
  for (const cmd of commands) {
    if (cmd.$kind !== 'MoveCall' && !cmd.MoveCall) throw Error('Only procurement MoveCall commands may be sponsored');
    const m = cmd.MoveCall;
    if (canonicalAddress(m.package) !== c.packageId || m.typeArguments?.length) throw Error('Foreign package/type argument');
    if (m.module === 'intent' && ['new_intent','new_proof'].includes(m.function)) continue;
    if (m.module !== 'procurement_system' || !functions.has(m.function)) throw Error('Unapproved system call');
    systemCalls++;
  }
  if (systemCalls !== 1 || commands.at(-1)?.MoveCall?.module !== 'procurement_system') throw Error('Exactly one final procurement system call required');
  if (BigInt(data.gasData?.budget ?? '0') !== BigInt(c.gasBudgetMist)) throw Error('Unexpected gas budget');
  const sharedIds = data.inputs.filter(i => i.Object?.SharedObject).map(i => canonicalAddress(i.Object.SharedObject.objectId));
  if (!sharedIds.includes(c.dappStorageId) || !sharedIds.includes(c.configId) || !sharedIds.includes(canonicalAddress('0x6'))) throw Error('Missing configured shared objects');
}
function keyConfig(c, env) {
  if (!env.MIR2_PROCUREMENT_SUI_SECRET_KEY || !env.MIR2_PROCUREMENT_ORACLE_SECRET_HEX) throw Error('Explicit isolated-test signing keys required; no automatic key generation');
  if (publicFromSeed(env.MIR2_PROCUREMENT_ORACLE_SECRET_HEX) !== c.oraclePublicKey) throw Error('Oracle key does not match configured verifier');
}
async function sdk(c, env, needsSigner) {
  validateConfig(c);
  const client = await createGrpcFacade(c);
  if (await client.getChainIdentifier() !== c.chainIdentifier) throw Error('RPC returned the wrong chain');
  // Validate the real package/config boundary on every worker process, never trust an indexer.
  const objects = await client.multiGetObjects({ids:[c.packageId,c.frameworkPackageId,c.configId,c.dappStorageId],options:{showType:true,showContent:true,showOwner:true}});
  if (objects.some(o => o.error || !o.data)) throw Error('Configured chain objects unavailable');
  if (objects[0].data.content?.dataType !== 'package' || objects[1].data.content?.dataType !== 'package') throw Error('Configured package IDs are not packages');
  const cfg = objects[2].data;
  const ds = objects[3].data;
  if (cfg.type !== `${c.packageId}::procurement_system::Config` || !cfg.owner?.Shared
    || ds.type !== `${c.frameworkPackageId}::dapp_service::DappStorage` || !ds.owner?.Shared) throw Error('Wrong config/storage type or ownership');
  const fields = cfg.content?.fields;
  if (!fields || parsedHex(fields.network_hash) !== c.networkHash || canonicalAddress(fields.dapp_storage_id) !== c.dappStorageId
    || parsedHex(fields.oracle_public_key) !== c.oraclePublicKey || String(fields.oracle_key_id) !== c.oracleKeyId) throw Error('On-chain verifier configuration mismatch');
  const key = ds.content?.fields?.dapp_key;
  if (!matchesDappKey(key,c)) throw Error('Storage belongs to another DApp');
  let dubhe;
  if (needsSigner) {
    keyConfig(c,env);
    const { Dubhe } = await import('@0xobelisk/sui-client');
    const metadata = await client.getNormalizedMoveModulesByPackage({package:c.packageId});
    dubhe = new Dubhe({networkType:c.network,secretKey:env.MIR2_PROCUREMENT_SUI_SECRET_KEY,
      packageId:c.packageId,frameworkPackageId:c.frameworkPackageId,dappStorageId:c.dappStorageId,
      fullnodeUrls:[c.fullnodeUrl],metadata});
  }
  return {client,dubhe};
}
export async function chainClock(client) {
  const clock=await client.getObject({id:'0x6',options:{showContent:true,showType:true,showPreviousTransaction:true}});
  if(clock.data?.type!=='0x2::clock::Clock' || !clock.data.previousTransaction)throw Error('Clock object/transaction unavailable');
  const timestampMs=String(clock.data.content?.fields?.timestamp_ms??'');u64(timestampMs);
  const tx=await client.getTransactionBlock({digest:clock.data.previousTransaction,options:{showEffects:true}});
  const r=receipt(tx);
  if(!r.finalized||!r.success||r.digest!==clock.data.previousTransaction)throw Error('Clock is not finalized');
  return {timestampMs,checkpoint:r.checkpoint,version:String(clock.data.version),digest:r.digest,finalized:true};
}
function intentArgs(tx,i) {
  return [tx.pure.vector('u8',bytes(i.network_hash)),tx.pure.address(i.package_id),tx.pure.address(i.dapp_storage_id),tx.pure.u8(i.operation),
    ...['order_id','fill_id','buyer_ref','seller_ref'].map(k=>tx.pure.vector('u8',bytes(i[k]))),tx.pure.u32(i.template_id),tx.pure.vector('u8',bytes(i.conditions_hash)),
    ...['quantity','price_gold','expires_at_ms','rule_version'].map(k=>tx.pure.u64(i[k])),tx.pure.vector('u8',bytes(i.nonce)),tx.pure.u64(i.valid_until_ms),tx.pure.vector('u8',bytes(i.actor_public_key))];
}
function proofArgs(tx,p) {
  return [...['intent_hash','asset_lock_id','asset_instance_hash'].map(k=>tx.pure.vector('u8',bytes(p[k]))), ...['oracle_key_id','valid_until_ms','fence'].map(k=>tx.pure.u64(p[k]))];
}
export async function build(c,p,env=process.env) {
  validateConfig(c); keyConfig(c,env);
  if (!actions.has(p.action)) throw Error('Unsupported action');
  const {client,dubhe} = await sdk(c,env,true);
  const {Transaction} = await import('@mysten/sui-modern/transactions');
  const tx = new Transaction(); tx.setGasBudget(c.gasBudgetMist);
  const params=[tx.object(c.dappStorageId),tx.object(c.configId)];
  if (p.action !== 'create') { bytes(p.orderObjectId); params.push(tx.object(p.orderObjectId)); }
  params.push(tx.object('0x6'));
  let oracleSignature;
  if (['create','activate','fill','cancel'].includes(p.action)) {
    intentBytes(p.intent); proofBytes(p.proof);
    oracleSignature = signHash(proofBytes(p.proof),env.MIR2_PROCUREMENT_ORACLE_SECRET_HEX);
    validateAttestation(p.intent,p.proof,p.actorSignature,oracleSignature,c,(await chainClock(client)).timestampMs);
    const expectedOp = {create:1,activate:1,fill:2,cancel:3}[p.action];
    if (p.intent.operation !== expectedOp) throw Error('Action/intent mismatch');
    const i=await dubhe.tx.intent.new_intent({tx,params:intentArgs(tx,p.intent),isRaw:true});
    const proof=await dubhe.tx.intent.new_proof({tx,params:proofArgs(tx,p.proof),isRaw:true});
    params.push(i,proof,tx.pure.vector('u8',bytes(p.actorSignature,64)),tx.pure.vector('u8',bytes(oracleSignature,64)));
    if(p.action==='activate') {
      if(typeof p.conditionsBcs!=='string'||!/^0x(?:[0-9a-f]{2}){5,512}$/.test(p.conditionsBcs))throw Error('Canonical condition BCS preimage required');
      const conditionBytes=Buffer.from(p.conditionsBcs.slice(2),'hex');
      if(hex(hash(conditionBytes))!==p.intent.conditions_hash)throw Error('Condition preimage does not match authorized hash');
      params.push(tx.pure.vector('u8',conditionBytes));
    }
  } else if (p.action === 'revoke') {
    const body=revokeBytes(c,p.orderId,p.nextFence,p.validUntilMs,p.nonce);
    oracleSignature=signHash(body,env.MIR2_PROCUREMENT_ORACLE_SECRET_HEX);
    params.push(tx.pure.u64(p.nextFence),tx.pure.u64(p.validUntilMs),tx.pure.vector('u8',bytes(p.nonce)),tx.pure.vector('u8',bytes(oracleSignature,64)));
  }
  const fn={create:'create_order',activate:'activate_order',fill:'fill_order',cancel:'cancel_order',expire:'expire_order',revoke:'revoke_fence'}[p.action];
  await dubhe.tx.procurement_system[fn]({tx,params,isRaw:true});
  // signTxn only builds/signs; NOTHING is broadcast before the Rust worker commits this exact result.
  tx.setSender(dubhe.getAddress());
  const built=await tx.build({client:client.grpc});
  const simulated=await client.grpc.core.simulateTransaction({transaction:built,include:{effects:true,events:true}});
  if(simulated.$kind!=='Transaction'||!simulated.Transaction.status.success)throw Error('Procurement transaction simulation failed');
  // Only the already-built bytes enter Dubhe's signer; its obsolete JSON-RPC builder is never invoked.
  const signed=await dubhe.signTxn(built);
  const raw=typeof signed.bytes === 'string' ? Buffer.from(signed.bytes,'base64') : Buffer.from(signed.bytes);
  const recovered=Transaction.from(raw); validateCommands(recovered.getData(),c);
  const digest=await recovered.getDigest();
  return {transactionBytes:raw.toString('base64'),signatures:[signed.signature],digest,action:p.action,
    orderObjectId:p.orderObjectId ?? null,oracleProof:p.proof ?? null,oracleSignature:oracleSignature ?? null,gasBudgetMist:c.gasBudgetMist,
    simulationGasUsed:simulated.Transaction.effects.gasUsed};
}

/// Reserve the *maximum* gas amount before network submission; crash/retry of one digest is free.
/// An interrupted lock fails closed. Operators reconcile/delete a stale lock only after stopping this worker.
export function reserveGas(c,digest,env) {
  const filename=env.MIR2_PROCUREMENT_GAS_LEDGER;
  if (!filename || !isAbsolute(filename)) throw Error('Absolute durable gas-ledger path required');
  const lock=`${filename}.lock`; const fd=openSync(lock,'wx');
  try {
    let ledger; try { ledger=JSON.parse(readFileSync(filename,'utf8')); } catch(e) { if(e.code!=='ENOENT')throw e; ledger={version:1,chain:c.chainIdentifier,reservations:{}}; }
    if(ledger.version!==1 || ledger.chain!==c.chainIdentifier)throw Error('Invalid gas ledger domain');
    if(ledger.reservations[digest])return;
    const day=new Date().toISOString().slice(0,10);
    const total=Object.values(ledger.reservations).filter(r=>r.day===day).reduce((a,r)=>a+BigInt(r.maximumMist),0n);
    if(total+BigInt(c.gasBudgetMist)>BigInt(c.dailyGasBudgetMist))throw Error('Daily gas budget exhausted');
    ledger.reservations[digest]={day,maximumMist:c.gasBudgetMist};
    const temp=`${filename}.${process.pid}.tmp`; const out=openSync(temp,'wx');
    try {writeFileSync(out,JSON.stringify(ledger));fsyncSync(out);}finally{closeSync(out);}
    renameSync(temp,filename);
  } finally {closeSync(fd);unlinkSync(lock);}
}
async function submitExact(c,p,env) {
  const {client,dubhe}=await sdk(c,env,true);
  const {Transaction}=await import('@mysten/sui-modern/transactions');
  if(typeof p.transactionBytes!=='string' || !Array.isArray(p.signatures) || p.signatures.length!==1)throw Error('Exact signed transaction required');
  const raw=Buffer.from(p.transactionBytes,'base64'); if(raw.length>65536 || raw.toString('base64')!==p.transactionBytes)throw Error('Invalid transaction encoding');
  const tx=Transaction.from(raw); validateCommands(tx.getData(),c);
  if(await tx.getDigest()!==p.digest)throw Error('Persisted digest/bytes mismatch');
  const data=tx.getData(); const signer=dubhe.getAddress();
  if(canonicalAddress(data.sender)!==canonicalAddress(signer) || canonicalAddress(data.gasData.owner ?? data.sender)!==canonicalAddress(signer))throw Error('Foreign sponsor/sender');
  const {verifyTransactionSignature}=await import('@mysten/sui-modern/verify');
  const verified=await verifyTransactionSignature(raw,p.signatures[0]);
  if(canonicalAddress(verified.toSuiAddress())!==canonicalAddress(signer))throw Error('Foreign transaction signature');
  reserveGas(c,p.digest,env);
  // Transport errors remain unknown; callers must query the saved digest, never refund on this error.
  const result=await client.executeExact(raw,p.signatures);
  if(result.digest!==p.digest)throw Error('RPC transaction digest mismatch');
  return {submitted:true,digest:result.digest};
}
export function receipt(result) {
  if (!['success','failure'].includes(result.effects?.status?.status)) throw Error('RPC effects missing: final result is unknown');
  if (result.checkpoint != null) u64(String(result.checkpoint));
  return {known:true,finalized:result.checkpoint!=null,success:result.effects?.status?.status==='success',
    checkpoint:result.checkpoint==null?null:String(result.checkpoint),digest:result.digest,
    events:result.events ?? [],objectChanges:result.objectChanges ?? [],gasUsed:result.effects?.gasUsed ?? null,error:result.effects?.status?.error ?? null};
}
const vectorFields=['publish_intent_hash','buyer_ref','buyer_public_key','conditions_hash','escrow_lock_id','fill_id','seller_ref','asset_instance_hash'];
const integerFields=['quantity','price_gold','expires_at_ms','rule_version','fence'];
function normalizeOrderFields(raw) {
  if(raw.status==null){if(Object.keys(raw).length)throw Error('Partially initialized order');return {};}
  const out={};for(const k of vectorFields){out[k]=parsedHex(raw[k]);bytes(out[k]);}
  out.conditions_bcs=Array.isArray(raw.conditions_bcs)?hex(raw.conditions_bcs):raw.conditions_bcs;
  if(typeof out.conditions_bcs!=='string'||!/^0x(?:[0-9a-f]{2}){5,512}$/.test(out.conditions_bcs)
    ||hex(hash(Buffer.from(out.conditions_bcs.slice(2),'hex')))!==out.conditions_hash)throw Error('Condition preimage missing or mismatched');
  for(const k of integerFields){out[k]=String(raw[k]);u64(out[k]);}
  out.status=Number(raw.status);out.template_id=Number(raw.template_id);
  if(![1,2,3,4].includes(out.status)||out.template_id!==245||out.quantity!=='1')throw Error('Unsupported chain order state');
  return out;
}
function boundOrderEvent(c,events,id,orderId,status,fields) {
  for(const e of events){const p=e.parsedJson??{};
    if(status===0 && e.type===`${c.frameworkPackageId}::dubhe_events::Dubhe_Object_Created`
      && matchesDappKey(p.dapp_key,c) && canonicalAddress(p.object_id)===id
      && parsedHex(p.entity_id)===orderId && parsedHex(p.object_type)===hex(Buffer.from('order')))return String(e.id?.eventSeq??'0');
    if(e.type===`${c.packageId}::procurement_system::OrderTransition` && canonicalAddress(p.object_id)===id
      && parsedHex(p.order_id)===orderId && Number(p.status)===status
      && parsedHex(p.fill_id)===fields.fill_id && parsedHex(p.buyer_ref)===fields.buyer_ref
      && parsedHex(p.seller_ref)===fields.seller_ref && String(p.price_gold)===fields.price_gold
      && String(p.rule_version)===fields.rule_version && parsedHex(p.asset_instance_hash)===fields.asset_instance_hash
      && parsedHex(p.conditions_hash)===fields.conditions_hash && Number(p.template_id)===fields.template_id
      && String(p.quantity)===fields.quantity && String(p.fence)===fields.fence)return String(e.id?.eventSeq??'0');
    if(status===1 && e.type===`${c.packageId}::procurement_system::FenceRevoked`
      && canonicalAddress(p.object_id)===id && parsedHex(p.order_id)===orderId && String(p.fence)===fields.fence)return String(e.id?.eventSeq??'0');
  }
  throw Error('Order result lacks a matching framework/package event');
}
/// A compensation decision must use this Clock-first observation, never a late clock attached to an old status.
export async function observeOrder(client,c,id) {
  id=canonicalAddress(id);bytes(id);
  const clock=await chainClock(client);
  const opts={showContent:true,showOwner:true,showType:true,showPreviousTransaction:true};
  const first=await client.getObject({id,options:opts});
  if(first.error)throw Error('Order unavailable; this does not establish safe absence');
  const o=first.data;
  if(o?.type!==`${c.frameworkPackageId}::dapp_service::ObjectStorage<${c.packageId}::procurement_system::Order>` || !o.owner?.Shared)throw Error('Unexpected order object type/owner');
  const f=o.content?.fields;
  if(!matchesDappKey(f?.dapp_key,c)||parsedHex(f.object_type)!==hex(Buffer.from('order')))throw Error('Order belongs to another DApp/entity type');
  const orderId=parsedHex(f.entity_id);bytes(orderId);
  const bag=f.data.fields.id.id,raw={};let cursor;
  do {
    const page=await client.getDynamicFields({parentId:bag,cursor,limit:50});
    for(const field of page.data){
      if(field.name.type!=='vector<u8>')throw Error('Unexpected order field key type');
      const value=await client.getDynamicFieldObject({parentId:bag,name:field.name});
      if(value.error||!value.data?.content)throw Error('Incomplete order field read');
      raw[Buffer.from(field.name.value).toString('utf8')]=value.data.content.fields.value;
    }
    cursor=page.hasNextPage?page.nextCursor:null;
  }while(cursor);
  const second=await client.getObject({id,options:opts});
  if(!second.data||second.data.version!==o.version||second.data.previousTransaction!==o.previousTransaction)throw Error('Order changed during observation; retry without compensation');
  const fields=normalizeOrderFields(raw),status=fields.status??0;
  if(!o.previousTransaction)throw Error('Order previous transaction unavailable');
  const tx=await client.getTransactionBlock({digest:o.previousTransaction,options:{showEffects:true,showEvents:true,showObjectChanges:true}});
  const r=receipt(tx);
  if(!r.finalized||!r.success||r.digest!==o.previousTransaction)throw Error('Order transaction is not finalized');
  if(!r.objectChanges.some(x=>['created','mutated'].includes(x.type)&&canonicalAddress(x.objectId)===id
    && x.objectType===o.type))throw Error('Order not bound to transaction object effects');
  const eventSequence=boundOrderEvent(c,r.events,id,orderId,status,fields);
  const checkpoint=String(await client.getLatestCheckpointSequenceNumber());u64(checkpoint);
  if(BigInt(checkpoint)<BigInt(clock.checkpoint)||BigInt(checkpoint)<BigInt(r.checkpoint))throw Error('RPC checkpoint moved backwards');
  return {found:true,orderObjectId:id,orderId,version:String(o.version),previousTransaction:o.previousTransaction,status,fields,checkpoint,
    observation:{coherent:true,clock,objectVersion:String(o.version),objectPreviousTransaction:o.previousTransaction,
      transactionCheckpoint:r.checkpoint,transactionDigest:r.digest,eventSequence},receipt:r};
}
export async function lookupOrder(client,c,orderId) {
  bytes(orderId);
  const clock=await chainClock(client);
  const options={showContent:true,showType:true,showPreviousTransaction:true};
  const before=await client.getObject({id:c.dappStorageId,options});
  if(!before.data?.previousTransaction)throw Error('Registry unavailable');
  let cursor;
  do {
    const page=await client.getDynamicFields({parentId:c.dappStorageId,cursor,limit:50});
    const hit=page.data.find(x=>x.name.type===`${c.frameworkPackageId}::dapp_service::ObjectEntityIdKey`
      && parsedHex(x.name.value?.entity_id)===orderId && parsedHex(x.name.value?.type_tag)===hex(Buffer.from('order')));
    if(hit){
      const record=await client.getDynamicFieldObject({parentId:c.dappStorageId,name:hit.name});
      const id=record.data?.content?.fields?.value;
      if(!id)throw Error('Incomplete registry record');
      return observeOrder(client,c,id);
    }
    cursor=page.hasNextPage?page.nextCursor:null;
  }while(cursor);
  const after=await client.getObject({id:c.dappStorageId,options});
  if(!after.data||after.data.version!==before.data.version||after.data.previousTransaction!==before.data.previousTransaction)throw Error('Registry changed during absence observation');
  const tx=receipt(await client.getTransactionBlock({digest:before.data.previousTransaction,options:{showEffects:true,showObjectChanges:true}}));
  if(!tx.finalized||!tx.success||tx.digest!==before.data.previousTransaction
    || !tx.objectChanges.some(o=>['created','mutated'].includes(o.type)&&canonicalAddress(o.objectId)===c.dappStorageId))throw Error('Registry effects unavailable');
  const checkpoint=String(await client.getLatestCheckpointSequenceNumber());u64(checkpoint);
  if(BigInt(checkpoint)<BigInt(clock.checkpoint)||BigInt(checkpoint)<BigInt(tx.checkpoint))throw Error('RPC checkpoint moved backwards');
  return {found:false,orderId,checkpoint,observation:{coherent:true,registryAbsence:true,clock,
    registryVersion:String(before.data.version),registryPreviousTransaction:before.data.previousTransaction,
    transactionCheckpoint:tx.checkpoint,transactionDigest:tx.digest},receipt:tx};
}
export async function dispatch(request,env=process.env) {
  if(request.protocolVersion!==1)throw Error('Unsupported driver protocol');
  const c=validateConfig(request.config),p=request.payload??{};
  if(request.operation==='build')return build(c,p,env);
  if(request.operation==='submitExact')return submitExact(c,p,env);
  const {client}=await sdk(c,env,false);
  if(request.operation==='readClock')return chainClock(client);
  if(request.operation==='queryFinality') {
    try { return receipt(await client.getTransactionBlock({digest:p.digest,options:{showEffects:true,showEvents:true,showObjectChanges:true,showInput:true}})); }
    catch(e) { if(e.code==='NOT_FOUND'||/not found|could not find|not yet/i.test(String(e.message)))return {known:false,digest:p.digest}; throw e; }
  }
  if(request.operation==='replayEvents') {
    const limit=p.limit??50;if(!Number.isInteger(limit)||limit<1||limit>50)throw Error('Invalid event page size');
    const result=await client.queryEvents({query:{MoveModule:{package:c.packageId,module:'procurement_system'}},cursor:p.cursor??undefined,limit,order:'ascending'});
    return {events:result.data,nextCursor:result.nextCursor,hasNextPage:result.hasNextPage};
  }
  if(request.operation==='queryOrder') {
    let id=p.orderObjectId;
    if(!id) {
      return lookupOrder(client,c,p.orderId);
    }
    return observeOrder(client,c,id);
  }
  throw Error('Unsupported driver operation');
}
if (process.argv[1] && import.meta.url===pathToFileURL(resolve(process.argv[1])).href) {
  let text='';for await(const chunk of process.stdin){text+=chunk;if(text.length>262144)throw Error('Driver request too large');}
  try {process.stdout.write(`${JSON.stringify({ok:true,result:await dispatch(JSON.parse(text))})}\n`);}
  catch(e){process.stdout.write(`${JSON.stringify({ok:false,error:String(e.message),unknown:true})}\n`);process.exitCode=1;}
}
