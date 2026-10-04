// Synthetic receipt/controller fixtures only. No Gateway connection, player
// route, natural drop, source state mutation or human acceptance is performed.
import test from 'node:test';
import assert from 'node:assert/strict';
import { SOURCE_HASHES, ROUTES, EXPECTED_DIRECTED_HOPS, LEGACY_POLICY, sha256, loadClassicSources,
  assertLegacySource, validateScenario, assertPair, assertCommand, validateEndpoint, bossCatalog, redactEvidence, exactNpc, npcLink } from './classic-late-route-policy.mjs';
import { ClassicProtocolClient, executeClassicHop, observeClassicNpc, useNaturallyPickedUpBook } from './run-classic-late-route.mjs';
import { verifyClassicEvidence, verifyClassicCohort } from './verify-classic-late-route-evidence.mjs';

const sources=await loadClassicSources();
const gatewayUrl='wss://classic-fixture.invalid/ws';
const pair={schema:'mir2.classic-paired-gateway-attestation.v1',gatewayUrl,sourceRevision:'a'.repeat(40),gatewayExecutableSha256:'b'.repeat(64),
  profileId:'platinum_176',profileVersion:27,sourceHashes:{...SOURCE_HASHES},qaNaturalKillDropMultiplier:1,profileDropMultiplier:1,
  debugWorldMutationEnabled:false,namedLegacyPolicy:'source-bound-stone-big-taoist-v1'};
const health={ok:true,ws:'ready',revision:pair.sourceRevision};
const scenario={gatewayUrl,className:'Warrior',accountId:'fixture-account',password:'fixture-secret',characterName:'ReceiptFixture',characterIndex:0,
  lane:'main-round-trips',startingState:{preparedLevel:true,level:50}};
const actor=(x,y)=>({objectId:101,kind:'selfPlayer',name:scenario.characterName,class:'Warrior',level:50,x,y,hp:400,maxHp:400});
const world=(map,x=1,y=1)=>({mapFileName:map,mapSnapshotPending:false,playerObjectId:101,playerHp:400,
  entities:[actor(x,y)],mapTransfers:[],inventoryItems:[{uniqueId:701,key:'crystal-item-719',name:'TownTeleport',quantity:1}],knownSkills:[],groundDrops:[]});

function fixture({sourceAck=true,boss=false,book=false}={}) {
  const events=[];const at=Date.parse('2026-10-04T00:00:00Z');
  const add=(direction,value)=>{const event={...structuredClone(value),direction,sequence:events.length+1,at:new Date(at+(events.length+1)*100).toISOString()};events.push(event);return event;};
  add('sent',{type:'login',accountId:'[redacted]',password:'[redacted]'});add('received',{packet:'LoginSuccess'});
  add('sent',{type:'startGame',characterIndex:0});add('received',{packet:'StartGame',payload:{result:4}});
  const initial=add('received',{type:'worldSnapshot',payload:world('0')});
  const report={schema:'mir2.classic-late-route-run.v1',className:'Warrior',lane:'main-round-trips',startingState:{preparedLevel:true,level:50},
    gatewayUrl,health:structuredClone(health),sourceHashes:{...SOURCE_HASHES},pairedReceiptSha256:sha256(JSON.stringify(pair)),accountFingerprint:sha256('fixture-one'),
    initialSnapshotSequence:initial.sequence,timeoutMs:120*60_000,spawnWaitMs:60*60_000,declaredNaturalKillLimit:boss?1:0,
    startedAt:new Date(at).toISOString(),status:'observed',freshLevelJourneyAccepted:false,runtimeGameplayAccepted:false,nativeVisualAccepted:false,
    routes:[],npcActions:[],bossAttempts:[],pickups:[],books:[],deaths:[],failures:[],preservedDefects:[]};
  let latest=initial.payload;
  let carried=structuredClone(latest.inventoryItems),skills=[];
  const currentWorld=(map,x,y)=>({...world(map,x,y),inventoryItems:structuredClone(carried),knownSkills:structuredClone(skills)});
  for(const [routeId,edges] of Object.entries(sources.portals)) {
    for(const [index,edge]of edges.entries()) {
      const portal=edge.portals[0],before=currentWorld(edge.from,portal.source.x-1,portal.source.y);
      before.mapTransfers=[{key:`${edge.from}-${edge.to}`,mapFileName:edge.from,toMapFileName:edge.to,
        bounds:{minX:portal.source.x,maxX:portal.source.x,minY:portal.source.y,maxY:portal.source.y}}];
      const source=add('received',{type:'worldSnapshot',payload:before});
      add('sent',{type:'walk',movementDirection:'Right'});
      const sourcePosition=sourceAck?add('received',{packet:'UserLocation',payload:{location:portal.source}}):null;
      const transition=add('received',{packet:'MapChanged',payload:{fileName:edge.to,location:portal.destination}});
      const destination=add('received',{type:'worldSnapshot',payload:currentWorld(edge.to,portal.destination.x,portal.destination.y)});latest=destination.payload;
      report.routes.push({routeId,index,from:edge.from,to:edge.to,status:'observed',beforeSequence:source.sequence,sourceSnapshotSequence:source.sequence,
        sourcePositionSequence:sourcePosition?.sequence??null,transitionSequence:transition.sequence,destinationSnapshotSequence:destination.sequence,afterSequence:destination.sequence});
    }
    if(boss&&routeId==='stone') {
      const beforeSequence=events.at(-1).sequence;
      const observation=structuredClone(latest);observation.entities.push({objectId:880,name:'WhiteBoar',kind:'monster',hp:bossCatalog(sources,'WhiteBoar').monster.hp,x:100,y:100});
      add('received',{type:'worldSnapshot',payload:observation});add('sent',{type:'attack',objectId:880});
      add('received',{packet:'DamageIndicator',payload:{objectId:880,amount:20}});
      const died=add('received',{packet:'ObjectDied',payload:{info:{objectId:880}}});
      const bookInfo=sources.items.items.find(item=>item.name==='CounterAttack'&&item.item_type===20);
      if(book)latest.groundDrops=[{objectId:990,name:'CounterAttack',quantity:1,sourceMonster:'WhiteBoar',ownerObjectId:101,
        loot:{kind:'item',key:`crystal-item-${bookInfo.item_index}`}}];
      const settlement=add('received',{type:'worldSnapshot',payload:latest});
      report.bossAttempts.push({id:'WhiteBoar-1',name:'WhiteBoar',map:'D717',objectId:880,ordinal:1,beforeSequence,afterSequence:settlement.sequence,
        startedAt:events[beforeSequence-1].at,finishedAt:settlement.at,status:'observed',outcome:'observedKill',naturalSpawnAccepted:false,combatAccepted:false,
        deathSequence:died.sequence,settlementSnapshotSequence:settlement.sequence,noDrop:!book,
        groundDropSequences:book?[{objectId:990,snapshotSequence:settlement.sequence,kind:'item',name:'CounterAttack'}]:[],pickupIds:book?['pickup-book']:[]});
      if(book) {
        const beforePickup=add('received',{type:'worldSnapshot',payload:latest});add('sent',{type:'pickUp',objectId:990});
        carried.push({uniqueId:17001,key:`crystal-item-${bookInfo.item_index}`,name:'CounterAttack',quantity:1});
        latest={...structuredClone(latest),groundDrops:[],inventoryItems:structuredClone(carried)};
        const afterPickup=add('received',{type:'worldSnapshot',payload:latest});
        report.pickups.push({id:'pickup-book',attemptId:'WhiteBoar-1',groundObjectId:990,groundSnapshotSequence:settlement.sequence,
          beforeSnapshotSequence:beforePickup.sequence,afterSnapshotSequence:afterPickup.sequence,provenance:'fresh-kill-associated-ground-item',
          sourceKillIdentityAccepted:false,ownershipAccepted:true,status:'pickedUp',uniqueId:17001,itemName:'CounterAttack',quantity:1});
        const beforeUse=add('received',{type:'worldSnapshot',payload:latest});add('sent',{type:'useItem',grid:'inventory',uniqueId:17001});
        const ack=add('received',{packet:'UseItem',payload:{success:true,uniqueId:17001}});
        const magic=add('received',{packet:'NewMagic',payload:{magic:{spell:'CounterAttack'}}});
        carried=carried.filter(item=>item.uniqueId!==17001);skills=[{name:'CounterAttack',key:'CounterAttack'}];
        latest={...structuredClone(latest),inventoryItems:structuredClone(carried),knownSkills:structuredClone(skills)};
        const afterUse=add('received',{type:'worldSnapshot',payload:latest});
        report.books.push({spell:'CounterAttack',itemIndex:bookInfo.item_index,uniqueId:17001,pickupId:'pickup-book',
          beforeSnapshotSequence:beforeUse.sequence,useAckSequence:ack.sequence,newMagicSequence:magic.sequence,afterSnapshotSequence:afterUse.sequence,
          status:'learned',activationAccepted:false});
      }
    }
  }
  const before=add('received',{type:'worldSnapshot',payload:latest});const logout=add('received',{packet:'LogOutSuccess'});
  const start=add('received',{packet:'StartGame',payload:{result:4}});const relog=add('received',{type:'worldSnapshot',payload:latest});
  const finalLogout=add('received',{packet:'LogOutSuccess'});
  report.save={status:'observed',mechanism:'normal-LogOutSuccess-and-public-relogin',beforeSnapshotSequence:before.sequence,
    logoutSequence:logout.sequence,startSequence:start.sequence,reloginSnapshotSequence:relog.sequence,finalLogoutSequence:finalLogout.sequence};
  return {report,events,pair:structuredClone(pair),sources,pairReceiptSha256:report.pairedReceiptSha256};
}
const verdict=value=>verifyClassicEvidence(value);

test('actual v27 data binds208 runtime maps, all76 main directed hops and correct nested Boss DropPaths',()=>{
  assert.equal(sources.profile.mapWhitelist.length,208);assert.ok(!sources.profile.mapWhitelist.includes('D71653'));
  assert.equal(EXPECTED_DIRECTED_HOPS,76);assert.equal(Object.values(ROUTES).reduce((n,chain)=>n+chain.length-1,0),76);
  for(const [name,count]of [['WhiteBoar',122],['EvilSnake',140],['ZumaTaurus',179],['RedMoonEvil',155]])assert.equal(bossCatalog(sources,name).table.total_entries,count);
  assert.throws(()=>bossCatalog(sources,'WhiteBoar0'),/Unknown/);
});
for(const change of ['raw','parsed','path','key','condition'])test(`legacy compatibility fails closed on changed${change} source proof`,()=>{
  const script=structuredClone(sources.npcScripts.scripts.find(script=>script.script_key===LEGACY_POLICY[0].key));
  if(change==='raw')script.raw_text+='\n';if(change==='parsed')script.sections[0].lines.push('SET [999] 1');
  if(change==='path')script.relative_path='Other/Stone.txt';if(change==='key')script.script_key='Other/Stone';
  if(change==='condition')script.raw_text=script.raw_text.replace('CHECKQUEST 135 0','CHECKQUEST 135 1');
  assert.throws(()=>assertLegacySource(script),/Unknown\/changed/);
});
test('only declared normal endpoints, finite deadlines and natural kill budgets are accepted',()=>{
  assert.equal(validateScenario(scenario).maxKillsPerTarget,0);assert.equal(validateEndpoint('ws://127.0.0.1:3001/ws').hostname,'127.0.0.1');
  for(const url of ['ws://remote.invalid/ws','wss://name:password@remote.invalid/ws','wss://remote.invalid/ws?token=secret','wss://remote.invalid/ws#token'])assert.throws(()=>validateEndpoint(url));
  for(const changed of [{timeoutMs:121*60_000},{maxKillsPerTarget:4},{spawnWaitMs:151*60_000},{qa:true},{relocation:true},{dropMultiplier:100},{className:'Assassin'}])assert.throws(()=>validateScenario({...scenario,...changed}));
});
test('paired revision, exact actual data hashes and unchanged QA configuration are mandatory',()=>{
  assert.equal(assertPair(pair,health,scenario).executableIdentitySource,'root-deployment-attestation');
  for(const changed of [{sourceRevision:'c'.repeat(40)},{profileVersion:26},{qaNaturalKillDropMultiplier:10},{debugWorldMutationEnabled:true},
    {sourceHashes:{...SOURCE_HASHES,maps:'0'.repeat(64)}},{namedLegacyPolicy:'old'}])assert.throws(()=>assertPair({...pair,...changed},health,scenario));
});
test('ordinary command allowlist excludes admin/public raw warp paths',()=>{
  assertCommand({type:'walk',direction:'Right'});
  for(const command of [{type:'MoveTo',x:1,y:1},{type:'Stage5Command'},{type:'event.spawn'},{type:'qa.giveItem'},
    {type:'interact',target:'crystal:D71653:1:1'}])assert.throws(()=>assertCommand(command),/Forbidden/);
});
test('transport redacts nested credentials without changing actual sent login payload',async()=>{
  const lines=[],sent=[];const client=new ClassicProtocolClient(gatewayUrl,'unused',{appendFile:async(_file,line)=>lines.push(line),secrets:['fixture-secret','fixture-account']});
  client.ws={readyState:1,send:value=>sent.push(value)};
  client.send({type:'login',accountId:'fixture-account',password:'fixture-secret'});
  client.record('diagnostic',{nested:{password:'fixture-secret',token:'token-value'},message:'fixture-account fixture-secret'});
  await client.writeQueue;
  assert.match(sent[0],/fixture-secret/);assert.ok(!lines.join('').includes('fixture-secret'));assert.ok(!lines.join('').includes('fixture-account'));assert.ok(!lines.join('').includes('token-value'));
  assert.throws(()=>client.send({type:'MoveTo'}),/Forbidden/);
  assert.deepEqual(redactEvidence({nested:{secretAnswer:'x'}}),{nested:{secretAnswer:'[redacted]'}});
});
test('WSS controller refreshes own damage vitals with a bounded500ms cadence, not every hit or observer hit',async()=>{
  let now=1000,scheduled=null;const sent=[];
  const client=new ClassicProtocolClient(gatewayUrl,'unused',{appendFile:async()=>{},now:()=>now,
    setTimeoutImpl:(callback,ms)=>{scheduled={callback,ms};return 1;},clearTimeoutImpl:()=>{scheduled=null;}});
  client.ws={readyState:1,send:value=>sent.push(JSON.parse(value)),close:()=>{}};client.snapshot=world('D717');
  client.observeGatewayMessage({packet:'DamageIndicator',payload:{objectId:101,amount:5}});
  assert.deepEqual(sent.map(command=>command.type),['clientVersion']);
  now=1100;client.observeGatewayMessage({packet:'DamageIndicator',payload:{objectId:101,amount:5}});
  assert.equal(sent.length,1);assert.equal(scheduled.ms,400);
  client.observeGatewayMessage({packet:'DamageIndicator',payload:{objectId:880,amount:5}});assert.equal(sent.length,1);
  now=1500;scheduled.callback();assert.equal(sent.length,2);await client.close();
});

function hopClient(mode='good') {
  const edge=sources.portals.stone[0],portal=edge.portals[0];const data=world(edge.from,portal.source.x-1,portal.source.y);
  data.mapTransfers=[{key:'live',toMapFileName:edge.to,bounds:{minX:portal.source.x,maxX:portal.source.x,minY:portal.source.y,maxY:portal.source.y}}];
  const client={snapshot:data,events:[],sequence:0};
  const add=(direction,value)=>{const event={...structuredClone(value),direction,sequence:++client.sequence,at:new Date().toISOString()};client.events.push(event);return event;};
  client.send=command=>{assertCommand(command);add('sent',command);if(command.type==='clientVersion')add('received',{type:'worldSnapshot',payload:client.snapshot});};
  client.wait=async(predicate,label)=>{const value=predicate();if(!value)throw new Error(`Fixture timeout ${label}`);return value;};
  const travel=async()=>{
    if(mode!=='static'){client.send({type:'walk',direction:'Right'});add('received',{packet:'UserLocation',payload:{location:portal.source}});
      add('received',{packet:'MapChanged',payload:{fileName:mode==='wrong-map'?'3':edge.to}});
      client.snapshot=world(edge.to,mode==='wrong-position'?0:portal.destination.x,portal.destination.y);add('received',{type:'worldSnapshot',payload:client.snapshot});}
    return [{fromMapFileName:edge.from,toMapFileName:edge.to}];
  };
  return {client,travel,edge};
}
test('prepared hop controller requires fresh live transfer, actual map packet and exact landing',async()=>{
  const {client,travel,edge}=hopClient();const observed=await executeClassicHop(client,travel,edge,'stone',0);
  assert.equal(observed.sourcePositionProof,'direct-UserLocation');
  for(const mode of ['static','wrong-map','wrong-position']){const value=hopClient(mode);await assert.rejects(()=>executeClassicHop(value.client,value.travel,value.edge,'stone',0));}
  const missing=hopClient();missing.client.snapshot.mapTransfers=[];await assert.rejects(()=>executeClassicHop(missing.client,missing.travel,missing.edge,'stone',0),/No live/);
  const orphan=hopClient();await assert.rejects(()=>executeClassicHop(orphan.client,orphan.travel,{...orphan.edge,to:'D71653'},'stone',0),/Resource-only/);
});
test('NPC matching rejects template-only, stale, wrong-kind/id/position and unexposed link',()=>{
  const template=sources.npcs.npcs.find(npc=>npc.loaded_object_id===1142),data=world('D715',271,85);
  const npc={kind:'npc',objectId:1142,name:template.name,x:272,y:85};data.entities.push(npc);
  assert.equal(exactNpc(data,template)?.objectId,1142);
  for(const changed of [{kind:'monster'},{objectId:926},{x:273}])assert.equal(exactNpc({...data,entities:[{...npc,...changed}]},template),null);
  assert.equal(exactNpc({...data,mapSnapshotPending:true},template),null);
  data.activeNpcDialog={npcObjectId:1142,body:['actual page'],links:[{target:'@stonetomba',text:'enter'}]};
  assert.equal(npcLink(data,1142,'@stonetomba').target,'@stonetomba');
  assert.throws(()=>npcLink(data,926,'@stonetomba'),/wrong/);assert.throws(()=>npcLink(data,1142,'@Quest8'),/expose/);
});
test('unpicked or prepared/vendor book cannot be credited as a naturally acquired use',async()=>{
  await assert.rejects(()=>useNaturallyPickedUpBook({},null,'Warrior',sources),/own observed/);
  await assert.rejects(()=>useNaturallyPickedUpBook({},{status:'pickedUp',provenance:'vendor'},'Warrior',sources),/own observed/);
});
test('complete synthetic receipt structure is accepted only in its bounded lane, never fresh levels/whole world/native',()=>{
  const checked=verdict(fixture());assert.equal(checked.routeTransitionsAccepted,true);assert.equal(checked.sourcePositionsAccepted,true);
  assert.equal(checked.normalRouteAccepted,true);assert.equal(checked.directedHopsObserved,76);assert.equal(checked.preparedLevel,true);
  assert.equal(checked.completeClassicWorld,false);assert.equal(checked.freshLevelJourneyAccepted,false);assert.equal(checked.runtimeGameplayAccepted,false);assert.equal(checked.nativeVisualAccepted,false);
});
test('transition-implied source position remains pending instead of claiming direct source authority',()=>{
  const checked=verdict(fixture({sourceAck:false}));assert.equal(checked.routeTransitionsAccepted,true);assert.equal(checked.sourcePositionsAccepted,false);assert.equal(checked.normalRouteAccepted,false);
});
for(const failure of ['source-hash','static-hop','stale-hop','duplicate-hop','wrong-position','no-intent','missing-save','wrong-save-count','fresh-level','visual','whole-world','deadline','d71653','teleport','other-player'])test(`receipt verifier rejects${failure} false acceptance`,()=>{
  const value=fixture(),hop=value.report.routes[0];
  if(failure==='source-hash')value.report.sourceHashes.maps='0'.repeat(64);
  if(failure==='static-hop')value.events.find(e=>e.sequence===hop.transitionSequence).packet='StaticGraph';
  if(failure==='stale-hop')hop.sourceSnapshotSequence=hop.destinationSnapshotSequence;
  if(failure==='duplicate-hop')value.report.routes.push({...hop});
  if(failure==='wrong-position')value.events.find(e=>e.sequence===hop.destinationSnapshotSequence).payload.entities[0].x=0;
  if(failure==='no-intent')value.events.find(e=>e.sequence===hop.beforeSequence+1).type='keepAlive';
  if(failure==='missing-save')value.report.save=null;
  if(failure==='wrong-save-count')value.events.find(e=>e.sequence===value.report.save.reloginSnapshotSequence).payload.inventoryItems=[];
  if(failure==='fresh-level')value.report.freshLevelJourneyAccepted=true;
  if(failure==='visual')value.report.nativeVisualAccepted=true;
  if(failure==='whole-world')value.report.runtimeGameplayAccepted=true;
  if(failure==='deadline')value.events.find(e=>e.sequence===hop.transitionSequence).at='2026-10-05T00:00:00Z';
  if(failure==='d71653')hop.to='D71653';
  if(failure==='teleport')value.events.find(e=>e.sequence===hop.beforeSequence+1).type='useItem';
  if(failure==='other-player')value.events.find(e=>e.sequence===hop.sourceSnapshotSequence).payload.entities[0].name='Other';
  assert.throws(()=>verdict(value));
});
test('source spawn identity and no-drop outcomes stay separate; no target item is a retained result',()=>{
  const value=fixture({boss:true}),checked=verdict(value);
  assert.equal(checked.bossAttempts[0].combatObserved,true);assert.equal(checked.bossAttempts[0].naturalSpawnAccepted,false);
  assert.equal(checked.bossAttempts[0].combatAccepted,false);assert.equal(checked.bossAttempts[0].noDrop,true);
  value.report.bossAttempts[0].naturalSpawnAccepted=true;assert.throws(()=>verdict(value),/not actually exposed/);
});
test('omitting no-drop, missing ObjectDied, pretending absence is killed, death removal and bigger kill budget are rejected',()=>{
  for(const failure of ['no-drop','died','absent','death','budget']){
    const value=fixture({boss:true}),attempt=value.report.bossAttempts[0];
    if(failure==='no-drop')delete attempt.noDrop;
    if(failure==='died')value.events.find(e=>e.sequence===attempt.deathSequence).packet='ObjectRemove';
    if(failure==='absent'){attempt.outcome='absentWithinBudget';attempt.combatAccepted=true;}
    if(failure==='death')value.events.find(e=>e.sequence===attempt.settlementSnapshotSequence).payload.playerHp=0;
    if(failure==='budget')value.report.declaredNaturalKillLimit=4;
    assert.throws(()=>verdict(value),failure);
  }
});
test('owned pickup/book learning and saved skill are observable, while missing exact source-kill link stays unaccepted',()=>{
  const checked=verdict(fixture({boss:true,book:true}));
  assert.equal(checked.pickups[0].pickupObserved,true);assert.equal(checked.pickups[0].ownershipAccepted,true);
  assert.equal(checked.pickups[0].naturalAcquisitionAccepted,false);assert.equal(checked.books[0].learnAccepted,true);
  assert.equal(checked.books[0].activationAccepted,false);assert.equal(checked.books[0].naturalAcquisitionAccepted,false);
});
for(const failure of ['wrong-owner','preexisting-uid','no-newmagic','failed-ack','wrong-class-book','book-not-debited','book-skill-unsaved','forged-kill-link','learn-is-activation'])test(`item/book receipt rejects${failure}`,()=>{
  const value=fixture({boss:true,book:true}),pickup=value.report.pickups[0],book=value.report.books[0];
  const eventAt=sequence=>value.events.find(event=>event.sequence===sequence);
  if(failure==='wrong-owner')eventAt(pickup.groundSnapshotSequence).payload.groundDrops[0].ownerObjectId=999;
  if(failure==='preexisting-uid')eventAt(pickup.beforeSnapshotSequence).payload.inventoryItems.push({uniqueId:17001,quantity:1});
  if(failure==='no-newmagic')eventAt(book.newMagicSequence).packet='ObjectMagic';
  if(failure==='failed-ack')eventAt(book.useAckSequence).payload.success=false;
  if(failure==='wrong-class-book'){const info=sources.items.items.find(item=>item.name==='Blink'&&item.item_type===20);book.itemIndex=info.item_index;}
  if(failure==='book-not-debited')eventAt(book.afterSnapshotSequence).payload.inventoryItems.push({uniqueId:17001,quantity:1});
  if(failure==='book-skill-unsaved')eventAt(value.report.save.reloginSnapshotSequence).payload.knownSkills=[];
  if(failure==='forged-kill-link')pickup.sourceKillIdentityAccepted=true;
  if(failure==='learn-is-activation')book.activationAccepted=true;
  assert.throws(()=>verdict(value));
});
test('three-class aggregation needs independent accounts and one exact pair and cannot imply complete classic world',()=>{
  const checked=verdict(fixture()),reports=[{accountFingerprint:'one',pairedReceiptSha256:'same'},{accountFingerprint:'two',pairedReceiptSha256:'same'},{accountFingerprint:'three',pairedReceiptSha256:'same'}];
  const verdicts=['Warrior','Wizard','Taoist'].map(className=>({...checked,className}));
  assert.equal(verifyClassicCohort(verdicts,reports).plannedDirectedHops,228);
  assert.equal(verifyClassicCohort(verdicts,reports).runtimeGameplayAccepted,false);
  assert.throws(()=>verifyClassicCohort(verdicts,[reports[0],reports[0],reports[2]]),/distinct/);
  assert.throws(()=>verifyClassicCohort(verdicts,[reports[0],reports[1],{...reports[2],pairedReceiptSha256:'other'}]),/different/);
});
