// Modern Sui gRPC transport. Dubhe 2.0's JSON-RPC transport is intentionally not used.
import { bytes, hex, u64 } from './canonical.mjs';

const address=s=>`0x${String(s).replace(/^0x/,'').padStart(64,'0')}`;
const type=s=>String(s??'').replace(/0x0{1,63}2::/g,'0x2::');
export class Reader {
  constructor(data){this.data=Buffer.from(data);this.offset=0;}
  take(n){if(!Number.isSafeInteger(n)||n<0||this.offset+n>this.data.length)throw Error('Truncated BCS');const b=this.data.subarray(this.offset,this.offset+n);this.offset+=n;return b;}
  u8(){return this.take(1)[0];}
  u32(){return this.take(4).readUInt32LE();}
  u64(){return this.take(8).readBigUInt64LE().toString();}
  length(){let n=0,shift=0;for(let i=0;i<5;i++){const b=this.u8();n+=(b&127)*2**shift;if(!(b&128)){if(i&&b===0)throw Error('Noncanonical ULEB');if(n>65536)throw Error('BCS vector too large');return n;}shift+=7;}throw Error('ULEB overflow');}
  vector(){return [...this.take(this.length())];}
  string(){return Buffer.from(this.vector()).toString('utf8');}
  address(){return hex(this.take(32));}
  done(){if(this.offset!==this.data.length)throw Error('Trailing BCS bytes');}
}
export function decodeObject(object){
  const t=type(object.type),r=new Reader(object.content??[]);let fields;
  if(t==='0x2::clock::Clock'){fields={id:{id:r.address()},timestamp_ms:r.u64()};r.done();}
  else if(t.endsWith('::procurement_system::Config')){
    fields={id:{id:r.address()},network_hash:r.vector(),dapp_storage_id:r.address(),oracle_public_key:r.vector(),oracle_key_id:r.u64(),publishing_enabled:r.u8()===1};r.done();
  }else if(t.includes('::dapp_service::ObjectStorage<')){
    fields={id:{id:r.address()},dapp_key:r.string(),object_type:r.vector(),entity_id:r.vector(),data:{fields:{id:{id:r.address()},size:r.u64()}}};r.done();
  }else if(t.endsWith('::dapp_service::DappStorage')){
    // Only this verified fixed prefix is needed for domain matching; the rest is framework-owned fee/metadata.
    fields={id:{id:r.address()},dapp_key:r.string()};
  }else if(t==='package')fields=null;
  else fields={};
  const owner=object.owner?.$kind==='Shared'?{Shared:{initial_shared_version:object.owner.Shared.initialSharedVersion}}
    :object.owner?.$kind==='AddressOwner'?{AddressOwner:object.owner.AddressOwner}:object.owner;
  return {data:{objectId:object.objectId,type:t,version:object.version,digest:object.digest,owner,previousTransaction:object.previousTransaction,
    content:t==='package'?{dataType:'package'}:{dataType:'moveObject',fields}}};
}
export function decodeEvent(event,digest,index){
  const t=type(event.eventType),r=new Reader(event.contents?.value??[]);let p=null;
  if(t.endsWith('::procurement_system::OrderTransition')){
    p={order_id:r.vector(),object_id:r.address(),status:r.u8(),fill_id:r.vector(),buyer_ref:r.vector(),seller_ref:r.vector(),
      template_id:r.u32(),conditions_hash:r.vector(),quantity:r.u64(),price_gold:r.u64(),rule_version:r.u64(),asset_instance_hash:r.vector(),fence:r.u64()};r.done();
  }else if(t.endsWith('::dubhe_events::Dubhe_Object_Created')){
    p={dapp_key:r.string(),object_type:r.vector(),object_id:r.address(),entity_id:r.vector()};r.done();
  }else if(t.endsWith('::procurement_system::FenceRevoked')){
    p={order_id:r.vector(),object_id:r.address(),fence:r.u64()};r.done();
  }
  return {type:t,parsedJson:p,id:{txDigest:digest,eventSeq:String(index)},bcs:Buffer.from(event.contents?.value??[]).toString('base64')};
}
export function decodeTransaction(tx){
  if(!tx?.effects?.status||typeof tx.effects.status.success!=='boolean')throw Error('gRPC effects missing');
  const gas=tx.effects.gasUsed;
  return {digest:tx.digest,checkpoint:tx.checkpoint==null?null:String(tx.checkpoint),
    effects:{status:{status:tx.effects.status.success?'success':'failure',error:tx.effects.status.error?.description??null},
      gasUsed:gas?Object.fromEntries(Object.entries(gas).map(([k,v])=>[k,String(v)])):null},
    events:(tx.events?.events??[]).map((e,i)=>decodeEvent(e,tx.digest,i)),
    objectChanges:(tx.effects.changedObjects??[]).filter(o=>[2,3].includes(o.outputState)).map(o=>({
      type:o.outputState===3?'published':o.idOperation===2?'created':'mutated',objectId:address(o.objectId),
      objectType:type(o.objectType),version:String(o.outputVersion),digest:o.outputDigest}))};
}
function fieldValue(t,b){const r=new Reader(b);let v;if(t==='u8')v=r.u8();else if(t==='u32')v=r.u32();else if(t==='u64')v=r.u64();else if(t==='address')v=r.address();else if(t==='vector<u8>')v=r.vector();else throw Error('Unexpected procurement field value type '+t);r.done();return v;}
function fieldName(t,b){const r=new Reader(b);if(t==='vector<u8>'){const v=r.vector();r.done();return v;}
  if(t.endsWith('::dapp_service::ObjectEntityIdKey')){const v={type_tag:r.vector(),entity_id:r.vector()};r.done();return v;}
  return {bcs:hex(b)};
}
export async function queryFilteredEvents(c,{cursor,limit},fetcher=globalThis.fetch) {
  const endpoint=c.eventGraphqlUrl??(c.network==='testnet'?'https://graphql.testnet.sui.io/graphql':null);
  if(!endpoint)throw Error('Explicit localnet eventGraphqlUrl or eventReplayMode=checkpoint required');
  const url=new URL(endpoint);
  if(url.username||url.password||url.protocol!=='https:'&&!(c.network==='localnet'&&url.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(url.hostname))
    ||url.hostname.includes('mainnet'))throw Error('Invalid GraphQL test-network endpoint');
  if(!Number.isInteger(limit)||limit<1||limit>50)throw Error('Invalid GraphQL replay limit');
  let after=null,legacyCursorReset=false;
  if(cursor!=null){
    if(typeof cursor.graphql==='string'&&cursor.graphql.length>0&&cursor.graphql.length<=4096&&Object.keys(cursor).length===1)after=cursor.graphql;
    else if(cursor.checkpoint!=null||cursor.txDigest||cursor.eventSeq){legacyCursorReset=true;}
    else throw Error('Invalid GraphQL opaque cursor');
  }
  u64(c.startCheckpoint);
  const start=BigInt(c.startCheckpoint)>0n?BigInt(c.startCheckpoint)-1n:0n;
  if(start>BigInt(Number.MAX_SAFE_INTEGER))throw Error('GraphQL UInt53 checkpoint overflow');
  const query='query ProcurementEvents($module:String!,$after:String,$first:Int!,$start:UInt53){events(first:$first,after:$after,filter:{module:$module,afterCheckpoint:$start}){pageInfo{hasNextPage endCursor} edges{cursor node{sequenceNumber transaction{digest} contents{type{repr} bcs}}}}}';
  const response=await fetcher(url.href,{method:'POST',headers:{'Content-Type':'application/json'},
    body:JSON.stringify({query,variables:{module:c.packageId+'::procurement_system',after,first:limit,start:Number(start)}}),signal:AbortSignal.timeout(20000)});
  if(!response.ok)throw Error('GraphQL replay HTTP '+response.status);
  const result=await response.json();if(result.errors?.length)throw Error('GraphQL replay unavailable: '+result.errors.map(e=>e.message).join('; '));
  const page=result.data?.events;
  if(!page||!Array.isArray(page.edges)||page.edges.length>limit||typeof page.pageInfo?.hasNextPage!=='boolean')throw Error('Malformed GraphQL event page');
  const data=[];
  for(const edge of page.edges){
    if(typeof edge.cursor!=='string'||!edge.cursor.length||edge.cursor.length>4096)throw Error('Malformed GraphQL edge cursor');
    const node=edge.node,t=node?.contents?.type?.repr;
    // These are only wakeups. Ignore framework field-write noise; fullnode
    // queryOrder/finalized effects remain mandatory for any asset decision.
    if(![c.packageId+'::procurement_system::OrderTransition',c.packageId+'::procurement_system::FenceRevoked',
      c.frameworkPackageId+'::dubhe_events::Dubhe_Object_Created'].includes(t))continue;
    const digest=node.transaction?.digest,bcs=node.contents.bcs,index=node.sequenceNumber;
    if(typeof digest!=='string'||!/^[1-9A-HJ-NP-Za-km-z]{43,44}$/.test(digest)||!Number.isSafeInteger(index)||index<0
      ||typeof bcs!=='string'||bcs.length>90000||Buffer.from(bcs,'base64').toString('base64')!==bcs)throw Error('Malformed indexed event');
    data.push({...decodeEvent({eventType:t,contents:{value:Buffer.from(bcs,'base64')}},digest,index),
      cursor:{graphql:edge.cursor},source:'graphql-indexer-wakeup'});
  }
  const end=page.pageInfo.endCursor;
  if(end!=null&&(typeof end!=='string'||!end.length||end.length>4096))throw Error('Malformed GraphQL page cursor');
  // Sui may advance page endCursor to its scanned watermark beyond the last
  // matching edge. Edge cursors remain the exact per-event commit positions.
  if(page.pageInfo.hasNextPage&&!end)throw Error('GraphQL continuation cursor missing');
  return {data,nextCursor:end?{graphql:end}:after?{graphql:after}:null,hasNextPage:page.pageInfo.hasNextPage,legacyCursorReset};
}
// Checkpoint replay is an ordered, bounded wakeup stream, never an asset receipt.
// Cursor positions point AFTER the emitted event and are safe to persist with it.
export async function replayCheckpointEvents({packageId,startCheckpoint,cursor,limit,current,fetchCheckpoint,checkpointBatchSize=32,concurrency=8}) {
  if(cursor==null&&startCheckpoint==null)throw Error('Deployment startCheckpoint is required for gap-free replay');
  const state=cursor??{checkpoint:startCheckpoint,transactionIndex:0,eventIndex:0};u64(state.checkpoint);
  if(!Number.isInteger(state.transactionIndex)||state.transactionIndex<0||!Number.isInteger(state.eventIndex)||state.eventIndex<0
    ||!Number.isInteger(limit)||limit<1||limit>50||!Number.isInteger(checkpointBatchSize)||checkpointBatchSize<1||checkpointBatchSize>32
    ||!Number.isInteger(concurrency)||concurrency<1||concurrency>8)throw Error('Invalid replay cursor/limit/bounds');
  const first=BigInt(state.checkpoint),head=BigInt(current);
  if(first>head)return {data:[],nextCursor:state,hasNextPage:false};
  const last=first+BigInt(checkpointBatchSize)-1n<head?first+BigInt(checkpointBatchSize)-1n:head;
  const data=[];
  // Each window prefetches independently; processing stays ordered even when
  // later requests finish first. Never advance a cursor beyond unread data.
  for(let window=first;window<=last;window+=BigInt(concurrency)){
    const requests=[];
    for(let cp=window;cp<=last&&cp<window+BigInt(concurrency);cp++)requests.push(fetchCheckpoint(cp));
    const blocks=await Promise.all(requests);
    for(let i=0;i<blocks.length;i++){
      const cp=window+BigInt(i),block=blocks[i];
      if(block?.sequenceNumber!==cp||!Array.isArray(block.transactions))throw Error('Checkpoint replay response mismatch');
      const initialTransaction=cp===first?state.transactionIndex:0;
      for(let ti=initialTransaction;ti<block.transactions.length;ti++){
        const tx=block.transactions[ti],events=tx.events?.events??[];
        const initialEvent=cp===first&&ti===state.transactionIndex?state.eventIndex:0;
        for(let ei=initialEvent;ei<events.length;ei++){
          const event=events[ei];
          if(type(event.eventType).startsWith(packageId+'::procurement_system::')){
            const nextCursor={checkpoint:String(cp),transactionIndex:ti,eventIndex:ei+1};
            data.push({...decodeEvent(event,tx.digest,ei),cursor:nextCursor});
            if(data.length===limit)return {data,nextCursor,hasNextPage:true};
          }
        }
      }
    }
  }
  return {data,nextCursor:{checkpoint:String(last+1n),transactionIndex:0,eventIndex:0},hasNextPage:last<head};
}
export async function createGrpcFacade(c){
  const {SuiGrpcClient}=await import('@mysten/sui-modern/grpc');
  const {fromBase58}=await import('@mysten/sui-modern/utils');
  const grpc=new SuiGrpcClient({network:c.network,baseUrl:c.fullnodeUrl});
  const cache=new Map();
  const api={grpc,
    async getChainIdentifier(){
      const genesis=(await grpc.core.getChainIdentifier()).chainIdentifier;
      if(c.genesisDigest&&genesis!==c.genesisDigest)throw Error('Full genesis digest mismatch');
      return Buffer.from(fromBase58(genesis)).subarray(0,4).toString('hex');
    },
    async getObject({id}){try{return decodeObject((await grpc.core.getObject({objectId:id,include:{content:true,previousTransaction:true}})).object);}catch(e){if(e.code==='NOT_FOUND')return {error:{code:'notExists'}};throw e;}},
    async multiGetObjects({ids}){return Promise.all(ids.map(id=>api.getObject({id})));},
    async getLatestCheckpointSequenceNumber(){const{response}=await grpc.ledgerService.getServiceInfo({});if(response.checkpointHeight==null)throw Error('gRPC checkpoint height unavailable');return String(response.checkpointHeight);},
    async getTransactionBlock({digest}){const{response}=await grpc.ledgerService.getTransaction({digest,readMask:{paths:['digest','checkpoint','effects','events']}});if(!response.transaction)throw Error('Transaction not found');return decodeTransaction(response.transaction);},
    async getDynamicFields({parentId,cursor,limit}){
      const page=await grpc.listDynamicFields({parentId,cursor,limit,include:{value:true}});
      const data=page.dynamicFields.map(f=>{const name={type:type(f.name.type),value:fieldName(type(f.name.type),f.name.bcs)};
        cache.set(parentId+JSON.stringify(name),{name:f.name,value:f.value});return {name,objectId:f.fieldId,objectType:type(f.type)};});
      return {data,nextCursor:page.cursor,hasNextPage:page.hasNextPage};
    },
    async getDynamicFieldObject({parentId,name}){
      const cached=cache.get(parentId+JSON.stringify(name));if(!cached)throw Error('Dynamic field was not read in this observation');
      const value=fieldValue(type(cached.value.type),cached.value.bcs);
      return {data:{content:{fields:{value}}}};
    },
    async getNormalizedMoveModulesByPackage(){
      // Dubhe raw builders only need an exposed-function map; verify each function's real ABI exists via gRPC.
      const modules={},reads=[];for(const [moduleName,names]of Object.entries({intent:['new_intent','new_proof'],procurement_system:['create_order','activate_order','fill_order','cancel_order','expire_order','revoke_fence']})){
        modules[moduleName]={name:moduleName,structs:{},exposedFunctions:{}};
        for(const name of names)reads.push((async()=>{const{function:fn}=await grpc.core.getMoveFunction({packageId:c.packageId,moduleName,name});
          modules[moduleName].exposedFunctions[name]={visibility:fn.visibility,isEntry:fn.isEntry,typeParameters:[],parameters:[],return:[]};})());
      }await Promise.all(reads);return modules;
    },
    async executeExact(raw,signatures){const result=await grpc.core.executeTransaction({transaction:raw,signatures,include:{effects:true,events:true}});const tx=result.Transaction??result.FailedTransaction;return {digest:tx.digest};},
    async queryEvents({cursor,limit}){
      if(c.eventReplayMode!=='checkpoint')return queryFilteredEvents(c,{cursor,limit});
      return replayCheckpointEvents({packageId:c.packageId,startCheckpoint:c.startCheckpoint,cursor,limit,
        current:await api.getLatestCheckpointSequenceNumber(),fetchCheckpoint:async cp=>{
          const{response}=await grpc.ledgerService.getCheckpoint({checkpointId:{oneofKind:'sequenceNumber',sequenceNumber:cp},
            readMask:{paths:['sequence_number','transactions.digest','transactions.events']}});
          return response.checkpoint;
        }});
    }
  };return api;
}
