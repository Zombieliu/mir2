import test from 'node:test';
import assert from 'node:assert/strict';
import { performance } from 'node:perf_hooks';
import { SignedClient,equipmentRoundtrip,equipmentRoundtripPlan,equipmentStateHash,equipmentRegressionActors } from './public_signed_probe.mjs';

function fixture(t,mode='success',ackDelay=10,stateDelay=20){
  const descriptor=Object.getOwnPropertyDescriptor(performance,'now'),clock={now:100000};
  Object.defineProperty(performance,'now',{configurable:true,value:()=>clock.now});
  t.after(()=>descriptor?Object.defineProperty(performance,'now',descriptor):delete performance.now);
  const original={mapFileName:'0',playerObjectId:1000,playerHp:18,gold:7,inventoryCapacity:46,maxBagSlots:40,
    entities:[{objectId:1000,name:'SyntheticEquipment',kind:'selfPlayer',x:290,y:616,dead:false}],
    inventoryItems:[{uniqueId:11,itemIndex:1,slot:0,container:'bag1',name:'TinyHP',count:2}],
    beltItems:[{uniqueId:12,itemIndex:1,slot:0,container:'belt',name:'TinyHP',count:1}],
    equipmentItems:[{uniqueId:21,itemIndex:10,slot:'weapon',equipSlot:'weapon',container:'equipment',name:'WoodenSword',count:1,currentDura:4000}]};
  const server=structuredClone(original),rows=[],commands=[],context={connection:0,secrets:['private-test-token'],assert(){},emit(row){rows.push(structuredClone(row));}};
  const c=new SignedClient('wss://example.invalid/ws',{name:'SyntheticEquipment'},0,context);c.originalOrdinal=75;c.inGame=true;c.snapshot=structuredClone(original);
  c.ws={readyState:1,send(text){
    const command=JSON.parse(text);commands.push(command);assert.ok(['removeItem','equipItem'].includes(command.type));
    if(command.type==='removeItem'){
      const item=server.equipmentItems.pop();server.inventoryItems.push({...item,slot:command.to,container:'bag1'});
    }else{
      server.inventoryItems=server.inventoryItems.filter(i=>i.uniqueId!==command.uniqueId);server.equipmentItems=structuredClone(original.equipmentItems);
    }
    const packet=command.type==='removeItem'?'RemoveItem':'EquipItem',payload={grid:'Inventory',uniqueId:command.uniqueId,to:command.to,success:true};
    if(mode==='preAckSnapshot')c.observe({type:'worldSnapshot',payload:structuredClone(server)});
    clock.now+=ackDelay;c.observe({packet,payload:mode==='wrongAck'?{...payload,uniqueId:999}:payload});
    clock.now+=stateDelay;
    if(mode==='cacheOnly')c.snapshot=structuredClone(server);
    else if(mode!=='preAckSnapshot'){
      const snapshot=mode==='wrongState'?structuredClone(original):structuredClone(server);
      if(mode==='wrongOwner'){snapshot.playerObjectId=1001;snapshot.entities[0].objectId=1001;}
      c.observe({type:'worldSnapshot',payload:snapshot});
    }
    if(['cacheOnly','preAckSnapshot','wrongState','wrongOwner'].includes(mode))clock.now+=15001;
  }};
  return {c,clock,commands,rows,original,server};
}

test('ordinary remove/equip uses a real empty Bag cell, exact fresh ACKs and complete snapshots then restores the full fingerprint',async t=>{
  const {c,commands,rows,original}=fixture(t),result={actorOrdinal:0,round:1,passed:false,steps:[]};
  await equipmentRoundtrip(c,result);
  assert.deepEqual(commands,[{type:'removeItem',grid:'inventory',uniqueId:21,to:1},{type:'equipItem',grid:'inventory',uniqueId:21,to:0}]);
  assert.equal(result.passed,true);assert.equal(result.originalOrdinal,75);assert.equal(result.beforeFingerprintHash,equipmentStateHash(original));
  assert.equal(result.beforeFingerprintHash,result.afterFingerprintHash);assert.equal(result.steps.length,2);
  for(const s of result.steps){assert.ok(s.cursorBefore<s.sentSequence&&s.sentSequence<s.ackSequence&&s.ackSequence<s.stateSequence);
    assert.equal(s.ackBudgetMs,15000);assert.equal(s.stateBudgetMs,15000);assert.equal(s.ackElapsedMs,10);assert.equal(s.stateElapsedMs,20);assert.equal(s.passed,true);}
  assert.equal(rows.filter(r=>r.kind==='equipmentRegressionReceipt').length,6);
  const publicEvidence=JSON.stringify({result,rows});assert.equal(publicEvidence.includes('SyntheticEquipment'),false);assert.equal(publicEvidence.includes('private-test-token'),false);
});

test('equipment ACK and post-ACK state retain independent15s budgets even if polling resumes after the ACK deadline',async t=>{
  const {c}=fixture(t,'success',14900,1000);const result=await equipmentRoundtrip(c);
  assert.equal(result.passed,true);assert.ok(result.steps.every(s=>s.ackElapsedMs===14900&&s.stateElapsedMs===1000));
});

test('a genuinely late item ACK fails without a retry or automatic restoration',async t=>{
  const {c,commands}=fixture(t,'success',15001,20),result={steps:[],passed:false};
  await assert.rejects(equipmentRoundtrip(c,result),/timed out waiting for RemoveItem/);
  assert.equal(result.passed,false);assert.equal(result.steps[0].ackWithinBudget,false);assert.equal(commands.length,1);
  assert.equal(c.snapshot.equipmentItems.length,0);
  c.observe({packet:'RemoveItem',payload:{grid:'Inventory',uniqueId:21,to:1,success:true}});
  assert.equal(result.passed,false);
});

test('a late full state snapshot cannot borrow another15s after delayed ACK polling',async t=>{
  const {c,commands}=fixture(t,'success',10,15001),result={steps:[],passed:false};
  await assert.rejects(equipmentRoundtrip(c,result),/fresh complete equipment regression state/);
  assert.equal(result.steps[0].ackWithinBudget,true);assert.equal(result.steps[0].stateWithinBudget,false);assert.equal(commands.length,1);
});

for(const mode of ['cacheOnly','preAckSnapshot','wrongState','wrongOwner','wrongAck'])test('equipment regression rejects '+mode+' evidence instead of skipping an equipped actor',async t=>{
  const {c,commands}=fixture(t,mode),result={steps:[],passed:false};
  await assert.rejects(equipmentRoundtrip(c,result));assert.equal(result.passed,false);assert.equal(commands.length,1);
});

test('roundtrip planning fails on no equipped item/full Bag and actor selection is explicit, bounded and preserves repeated rounds',()=>{
  assert.deepEqual(equipmentRegressionActors(undefined,100),[]);assert.deepEqual(equipmentRegressionActors('25,26,27',100),[25,26,27]);
  assert.deepEqual(equipmentRegressionActors('27,27',100),[27,27]);
  for(const value of ['-1','100','27,','x'])assert.throws(()=>equipmentRegressionActors(value,100));
  assert.throws(()=>equipmentRoundtripPlan({maxBagSlots:40,inventoryItems:[],equipmentItems:[]}),/already equipped/);
  assert.throws(()=>equipmentRoundtripPlan({maxBagSlots:40,inventoryItems:Array.from({length:40},(_,slot)=>({slot})),
    equipmentItems:[{uniqueId:21,slot:'weapon'}]}),/real empty/);
});
