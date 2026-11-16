import test from 'node:test';
import assert from 'node:assert/strict';
import { replayCheckpointEvents } from '../service/grpc.mjs';
const packageId='0x'+'22'.repeat(32);
const event=(ours=true)=>({eventType:(ours?packageId:'0x2')+'::procurement_system::FutureEvent',contents:{value:[]}});

test('each event carries its exact next checkpoint cursor for restart within a page',async()=>{
  const blocks=new Map([
    [10n,{sequenceNumber:10n,transactions:[]}],
    [11n,{sequenceNumber:11n,transactions:[{digest:'tx11',events:{events:[event(false),event(),event()]}}]}],
    [12n,{sequenceNumber:12n,transactions:[{digest:'tx12',events:{events:[event()]}}]}],
    [13n,{sequenceNumber:13n,transactions:[]}],
  ]);
  const base={packageId,startCheckpoint:'10',limit:50,current:'13',fetchCheckpoint:async cp=>blocks.get(cp)};
  const first=await replayCheckpointEvents(base);
  assert.deepEqual(first.data.map(e=>e.cursor),[
    {checkpoint:'11',transactionIndex:0,eventIndex:2},{checkpoint:'11',transactionIndex:0,eventIndex:3},
    {checkpoint:'12',transactionIndex:0,eventIndex:1}]);
  // Simulate a crash after committing only the first event and its cursor.
  const resumed=await replayCheckpointEvents({...base,cursor:first.data[0].cursor});
  assert.deepEqual(resumed.data.map(e=>e.id),first.data.slice(1).map(e=>e.id));
  assert.deepEqual(resumed.nextCursor,{checkpoint:'14',transactionIndex:0,eventIndex:0});
  assert.equal(resumed.hasNextPage,false);
  const limited=await replayCheckpointEvents({...base,limit:1});
  assert.deepEqual(limited.nextCursor,first.data[0].cursor);
  assert.deepEqual(limited.data[0].id,{txDigest:'tx11',eventSeq:'1'});
});

test('explicit checkpoint fallback advances 32 empty checkpoints using at most eight concurrent reads',async()=>{
  let active=0,maximum=0;const reads=[];
  const result=await replayCheckpointEvents({packageId,startCheckpoint:'10',limit:50,current:'300',fetchCheckpoint:async cp=>{
    reads.push(cp);maximum=Math.max(maximum,++active);
    await new Promise(r=>setTimeout(r,Number(8n-cp%8n)));active--;
    return {sequenceNumber:cp,transactions:[]};
  }});
  assert.equal(reads.length,32);assert.equal(maximum,8);
  assert.deepEqual(result.nextCursor,{checkpoint:'42',transactionIndex:0,eventIndex:0});
  assert.equal(result.hasNextPage,true);assert.deepEqual(result.data,[]);
});

test('concurrent prefetch is emitted in checkpoint order and errors never advance persisted state',async()=>{
  const result=await replayCheckpointEvents({packageId,startCheckpoint:'1',limit:50,current:'8',fetchCheckpoint:async cp=>{
    await new Promise(r=>setTimeout(r,Number(9n-cp)));
    return {sequenceNumber:cp,transactions:[{digest:String(cp),events:{events:[event()]}}]};
  }});
  assert.deepEqual(result.data.map(e=>e.id.txDigest),['1','2','3','4','5','6','7','8']);
  await assert.rejects(()=>replayCheckpointEvents({packageId,startCheckpoint:'1',limit:50,current:'8',fetchCheckpoint:async cp=>{
    if(cp===5n)throw Error('temporary RPC failure');return{sequenceNumber:cp,transactions:[]};
  }}),/temporary RPC failure/);
});
