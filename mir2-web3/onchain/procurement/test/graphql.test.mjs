import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {queryFilteredEvents} from '../service/grpc.mjs';
const dir=new URL('../validation/20261028/',import.meta.url);
const config=JSON.parse(readFileSync(new URL('driver-config.json',dir)));
const event=JSON.parse(readFileSync(new URL('fill-terminal.receipt.json',dir))).events.find(e=>e.type.endsWith('::OrderTransition'));
const edge=cursor=>({cursor,node:{sequenceNumber:Number(event.id.eventSeq),transaction:{digest:event.id.txDigest},contents:{type:{repr:event.type},bcs:event.bcs}}});
const page=edges=>({ok:true,json:async()=>({data:{events:{edges,pageInfo:{hasNextPage:false,endCursor:edges.at(-1)?.cursor??null}}}})});

test('filtered GraphQL binds the module/start and persists opaque edge cursors across mid-page restart',async()=>{
  const first=await queryFilteredEvents(config,{limit:50},async(url,init)=>{
    assert.equal(url,'https://graphql.testnet.sui.io/graphql');
    const body=JSON.parse(init.body);
    assert.equal(body.variables.module,config.packageId+'::procurement_system');
    assert.equal(body.variables.start,Number(config.startCheckpoint)-1);
    assert.equal(body.variables.after,null);return page([edge('opaque-A'),edge('opaque-B')]);
  });
  assert.equal(first.data.length,2);
  assert.deepEqual(first.data[0].cursor,{graphql:'opaque-A'});
  const resumed=await queryFilteredEvents(config,{cursor:first.data[0].cursor,limit:50},async(_url,init)=>{
    assert.equal(JSON.parse(init.body).variables.after,'opaque-A');return page([edge('opaque-B')]);
  });
  assert.deepEqual(resumed.data.map(e=>e.cursor),[{graphql:'opaque-B'}]);
  assert.deepEqual(resumed.nextCursor,{graphql:'opaque-B'});
  assert.equal(resumed.data[0].source,'graphql-indexer-wakeup');
});

test('empty GraphQL polls retain the last cursor and legacy checkpoint cursors replay from deployment',async()=>{
  const empty=await queryFilteredEvents(config,{cursor:{graphql:'opaque-last'},limit:50},async()=>page([]));
  assert.deepEqual(empty.nextCursor,{graphql:'opaque-last'});assert.equal(empty.hasNextPage,false);
  const legacy=await queryFilteredEvents(config,{cursor:{checkpoint:'400174657',transactionIndex:5,eventIndex:5},limit:50},async(_url,init)=>{
    assert.equal(JSON.parse(init.body).variables.after,null);return page([]);
  });
  assert.equal(legacy.legacyCursorReset,true);
});

test('indexer errors, malformed cursors and wrong network endpoints fail without a receipt',async()=>{
  await assert.rejects(()=>queryFilteredEvents(config,{limit:50},async()=>({ok:true,json:async()=>({errors:[{message:'indexer unavailable'}]})})),/indexer unavailable/);
  await assert.rejects(()=>queryFilteredEvents(config,{limit:50},async()=>({ok:true,json:async()=>({data:{events:{edges:[edge('A')],pageInfo:{hasNextPage:true,endCursor:12}}}})})),/Malformed GraphQL page cursor/);
  await assert.rejects(()=>queryFilteredEvents({...config,eventGraphqlUrl:'https://graphql.mainnet.sui.io/graphql'},{limit:50},async()=>{throw Error('must not fetch');}),/Invalid GraphQL/);
  await assert.rejects(()=>queryFilteredEvents(config,{cursor:{graphql:'',extra:true},limit:50},async()=>{throw Error('must not fetch');}),/Invalid GraphQL opaque cursor/);
});

test('GraphQL page watermark can advance beyond the final matching edge',async()=>{
  const result=await queryFilteredEvents(config,{limit:50},async()=>({ok:true,json:async()=>({data:{events:{edges:[edge('edge-A')],pageInfo:{hasNextPage:false,endCursor:'scan-watermark'}}}})}));
  assert.deepEqual(result.data[0].cursor,{graphql:'edge-A'});
  assert.deepEqual(result.nextCursor,{graphql:'scan-watermark'});
});
