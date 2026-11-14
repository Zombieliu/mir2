import test from 'node:test';
import assert from 'node:assert/strict';
import { pvpProbeOptions, pvpRawScalar, PvpProbeClient, exercisePvpPair, verifyPvpRelogin } from './playtest-capacity-pvp-probe.mjs';

const positions=[{x:4,y:4},{x:5,y:4}];
const options=()=>pvpProbeOptions({realm:'isolated',durationSeconds:30,fixture:{mapFileName:'0',positions}});
const drug=(edit={})=>({uniqueId:11,quantity:12,name:'(HP)DrugSmall',container:'belt',slot:0,item_index:null,tooltipSource:{info:{item_index:658},realInfo:null},...edit});
const raw=(i=0)=>({mapFileName:'0',playerObjectId:i+101,playerHp:24,playerMaxHp:24,inSafeZone:false,
  entities:positions.map((p,j)=>({kind:j===i?'selfPlayer':'player',objectId:j+101,name:'actor'+j,dead:false,...p})),inventoryItems:[],beltItems:[drug()]});
function fixture(edit={}) {
  let time=10000;const sent=[],rows=[],clients=[];
  const packet=(c,name,payload)=>{const event={direction:'received',type:'packet',packet:name,payload,sequence:++c.sequence};c.events.push(event);return event;};
  for(let i=0;i<2;i++) {
    const snapshot=raw(i);if(edit.namespaced){snapshot.playerObjectId=1000;snapshot.entities.forEach((e,j)=>e.objectId=j===i?1000:50000+j);}
    const c={label:'actor'+i,snapshot,events:[],sequence:0,closed:false,context:{guard:{assert(){}}},
      async pace(){if(edit.onPace)edit.onPace(c);},
      async refresh(){time+=10;if(edit.refreshError)throw edit.refreshError;if(edit.beforeRaw)edit.beforeRaw(c);c.probeRaw=pvpRawScalar(c.snapshot,c.label,++c.sequence);return c.snapshot;},
      async wait(predicate){const row=predicate();if(!row)throw new Error('fixture waiter absent');return row;},
      send(command){sent.push({actor:i,type:command.type,...command});
        if(command.type==='walk'){
          const delta={Up:[0,-1],UpRight:[1,-1],Right:[1,0],DownRight:[1,1],Down:[0,1],DownLeft:[-1,1],Left:[-1,0],UpLeft:[-1,-1]}[command.direction];
          const p=c.snapshot.entities[i],x=p.x+delta[0],y=p.y+delta[1];for(const other of clients)Object.assign(other.snapshot.entities[i],{x,y});packet(c,'UserLocation',{x,y});
        }
        if(command.type==='attack'&&!edit.noDamage){
          clients[1].snapshot.playerHp=20;
          for(const other of (edit.oneSide?[c]:clients)){
            const victimId=other.snapshot.entities[1].objectId,attackerId=other.snapshot.entities[0].objectId;
            packet(other,'ObjectStruck',{objectId:victimId,attackerId});packet(other,'ObjectHealth',{objectId:victimId,percent:83});packet(other,'DamageIndicator',{objectId:victimId,damage:4});
          }
          if(edit.afterAttack)edit.afterAttack(clients);
        }
      },
      async request(command,name){sent.push({actor:i,...command});
        if(command.type==='changeAMode')return packet(c,name,{mode:5});
        assert.equal(command.type,'useItem');
        if(!edit.noConsumption)c.snapshot.beltItems[0].quantity--;
        if(!edit.noHealing)c.snapshot.playerHp=24;
        return packet(c,name,{uniqueId:11,grid:'Belt',success:!edit.rejectDose});
      },
    };clients.push(c);
  }
  return {clients,sent,rows,services:{now:()=>time,sleep:async ms=>{time+=ms;if(edit.onSleep)edit.onSleep(clients)},loadMap:async()=>({mapFileName:'0',type:100,width:10,height:10,blocked:new Uint8Array(100)}),emit:r=>rows.push(r)}};
}

test('PvP options bound two actors/ordinary window/adjacent fixture',()=>{
  assert.equal(options().durationSeconds,30);assert.deepEqual(options().indices,[0,1]);
  for(const durationSeconds of[0,19,46,Infinity])assert.throws(()=>pvpProbeOptions({...options(),durationSeconds}));
  assert.throws(()=>pvpProbeOptions({...options(),indices:[0,0]}));
  assert.throws(()=>pvpProbeOptions({...options(),realm:'public'}));
  assert.throws(()=>pvpProbeOptions({...options(),fixture:{mapFileName:'0',positions:[positions[0],{x:9,y:9}]}}));
});
test('PvP raw scalar rejects percentage-only HP/death/ambiguous or duplicate inventory',()=>{
  const s=raw();assert.equal(pvpRawScalar(s,'actor0',1).hpStock,12);
  for(const mutate of[s=>delete s.playerHp,s=>s.playerHp=25,s=>s.entities[0].dead='false',s=>s.beltItems.push(drug()),s=>s.beltItems[0].realInfo={item_index:659},s=>s.beltItems[0].quantity=NaN]){
    const r=raw();mutate(r);assert.throws(()=>pvpRawScalar(r,'actor0',1));
  }
});
test('PvP mode extension only sends exact ordinary ChangeAMode5 through shared command budget',()=>{
  const c=Object.create(PvpProbeClient.prototype),sent=[];let reserved=0,guarded=0;
  Object.assign(c,{context:{guard:{assert(){guarded++}}},budget:{reserve(){reserved++}},bytes:{sent:0},ws:{send:x=>sent.push(JSON.parse(x))},record(){}});
  c.send({type:'changeAMode',mode:5});assert.equal(reserved,1);assert.equal(guarded,1);assert.deepEqual(sent,[{type:'changeAMode',mode:5}]);
  assert.throws(()=>c.send({type:'changeAMode',mode:4}));assert.throws(()=>c.send({type:'changeAMode',mode:5,qa:true}));assert.equal(sent.length,1);
  assert.throws(()=>c.send({type:'qa.giveItem'}));assert.equal(sent.length,1);
});
test('PvP actual two-client packet shape proves ordinary movement, hit, held dose and exact healing',async()=>{
  const f=fixture(),result=await exercisePvpPair(f.clients,options(),f.services);
  assert.equal(result.movementSteps,2);assert.equal(result.damage.exactHpLoss,4);assert.equal(result.healing.exactHpIncrease,4);
  assert.equal(result.healing.quantityAfter,11);assert.equal(result.launchedAttackCount,null);assert.equal(result.attackResults,'legacy');
  assert.equal(f.sent.filter(r=>r.type==='attack').length,1);assert.equal(f.sent.filter(r=>r.type==='useItem').length,1);
  assert.equal(f.sent.filter(r=>r.type==='changeAMode').length,2);assert.deepEqual(f.rows.map(r=>r.type),['pvpDamage','pvpHealing']);
  assert(!JSON.stringify(f.rows).includes('actor0'));assert(f.sent.every(c=>['walk','changeAMode','attack','useItem'].includes(c.type)));
});
test('PvP missed attacks bounded at16, no healing without proved hit',async()=>{
  const f=fixture({noDamage:true});await assert.rejects(exercisePvpPair(f.clients,options(),f.services),/No dual-client/);
  assert.equal(f.sent.filter(r=>r.type==='attack').length,16);assert.equal(f.sent.filter(r=>r.type==='useItem').length,0);
});
test('PvP one-sided packets do not establish dual-client hit despite exact HP decrease',async()=>{
  const f=fixture({oneSide:true});await assert.rejects(exercisePvpPair(f.clients,options(),f.services),/No dual-client/);
  assert.equal(f.sent.filter(r=>r.type==='useItem').length,0);
});
for(const [name,edit]of[['ACK rejection',{rejectDose:true}],['no actual consumption',{noConsumption:true}],['no exact healing',{noHealing:true}]])test('PvP '+name+' cannot pass dose proof',async()=>{
  const f=fixture(edit);await assert.rejects(exercisePvpPair(f.clients,options(),f.services));assert.equal(f.sent.filter(r=>r.type==='useItem').length,1);
});
test('PvP death ends probe without another attack or revive',async()=>{
  const f=fixture({afterAttack:cs=>cs[1].snapshot.entities[1].dead=true});await assert.rejects(exercisePvpPair(f.clients,options(),f.services),/owner\/map\/life/);
  assert.equal(f.sent.filter(r=>r.type==='attack').length,1);assert.equal(f.sent.filter(r=>r.type==='useItem').length,0);
});
test('PvP safe-zone and changed owner reject before attacks',async()=>{
  for(const mutate of[c=>c.snapshot.inSafeZone=true,c=>c.snapshot.entities[0].name='changed']){
    const f=fixture();mutate(f.clients[0]);await assert.rejects(exercisePvpPair(f.clients,options(),f.services));assert.equal(f.sent.length,0);
  }
});
test('PvP refresh and client failures keep original error identity and never emit credentials',async()=>{
  const error=new Error('token=secret-private-test'),f=fixture({refreshError:error});
  await assert.rejects(exercisePvpPair(f.clients,options(),f.services),e=>e===error);assert.deepEqual(f.rows,[]);
  const g=fixture();g.clients[0].failure=error;await assert.rejects(exercisePvpPair(g.clients,options(),g.services),e=>e===error);
});
test('PvP latest owner movement or map change while paced blocks send',async()=>{
  const f=fixture({onPace:c=>{c.snapshot.mapFileName='1'}});await assert.rejects(exercisePvpPair(f.clients,options(),f.services),/owner\/map\/life/);
  assert.equal(f.sent.length,0);
});
test('PvP complete raw is copied so later packets do not rewrite saved stock evidence',()=>{
  const s=raw(),proof=pvpRawScalar(s,'actor0',4);s.beltItems[0].quantity=0;s.playerHp=1;
  assert.equal(proof.hp,24);assert.equal(proof.hpItems[0].quantity,12);
});
test('PvP receiver-local self1000 versus foreign50000+ IDs drive actual command and dual evidence',async()=>{
  const f=fixture({namespaced:true}),r=await exercisePvpPair(f.clients,options(),f.services);
  assert.equal(f.sent.find(c=>c.type==='attack').objectId,50001);
  assert.deepEqual(r.damage.dualClientHit.targetIds,[50001,1000]);
  assert.deepEqual(r.damage.dualClientHit.attackerIds,[1000,50000]);
  assert.equal(r.damage.exactHpLoss,4);assert.equal(r.healing.quantityAfter,11);
});
test('PvP save/relogin check requires exact normal drain identity/position/UID stock not just HP',()=>{
  const original=pvpRawScalar(raw(),'actor0',1),current=structuredClone(original),saved={state:{name:'actor0',mapFileName:'0',x:4,y:4}};
  assert.equal(verifyPvpRelogin(saved,current,original,'actor0'),true);
  for(const mutate of[r=>r.x++,r=>r.map='1',r=>r.hpItems[0].quantity++,r=>r.hpItems[0].uniqueId++]){
    const r=structuredClone(current);mutate(r);assert.throws(()=>verifyPvpRelogin(saved,r,original,'actor0'));
  }
  assert.throws(()=>verifyPvpRelogin({state:{...saved.state,name:'another'}},current,original,'actor0'));
});

test('PvP raw owner contract is selfPlayer while peer remains player',()=>{
  const r=raw();assert.equal(r.entities[0].kind,'selfPlayer');assert.equal(r.entities[1].kind,'player');
  assert.equal(pvpRawScalar(r,'actor0',1).ownerId,101);
  r.entities[0].kind='player';assert.throws(()=>pvpRawScalar(r,'actor0',1),/owner/);
});

test('PvP actual WorldItemSnapshot nested tooltip info with null optional indices is accepted',()=>{
  const s=raw(),item=s.beltItems[0];
  assert.equal(item.item_index,null);assert.equal(item.tooltipSource.realInfo,null);assert.equal(item.info,undefined);
  assert.equal(pvpRawScalar(s,'actor0',1).hpStock,12);
  item.tooltipSource.realInfo={item_index:658};assert.equal(pvpRawScalar(s,'actor0',2).hpItems[0].uniqueId,11);
  item.tooltipSource.realInfo={item_index:659};assert.throws(()=>pvpRawScalar(s,'actor0',3),/Ambiguous HP658/);
  item.tooltipSource.realInfo=null;item.item_index=659;assert.throws(()=>pvpRawScalar(s,'actor0',4),/Ambiguous HP658/);
});
test('PvP HP template absent or noninteger is rejected despite exact display name',()=>{
  for(const item of[drug({tooltipSource:null}),drug({tooltipSource:{info:{item_index:'658'},realInfo:null}})]){
    const s=raw();s.beltItems=[item];assert.throws(()=>pvpRawScalar(s,'actor0',1),/Ambiguous HP658/);
  }
});
