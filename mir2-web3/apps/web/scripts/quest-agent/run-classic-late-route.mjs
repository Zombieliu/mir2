import fs from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { applyProtocolObservation, hasAuthoritativePlayerDeath } from './protocol-observation.mjs';
import { createNavigator, selfPlayer } from './protocol-play.mjs';
import { createMapTraveler } from './protocol-travel.mjs';
import { loadProtocolCollisionMap } from './protocol-navigation.mjs';
import { clearTravelBlockingMonster } from './protocol-combat.mjs';
import { combatAction, combatApproachRange, useSupplies, useClassRecovery } from './protocol-loadout.mjs';
import { refreshCombatWorldSnapshot } from './protocol-refresh.mjs';
import { sha256, SOURCE_HASHES, ROUTES, PRESERVED_DEFECT, SOURCE_ONLY_ROOM, BOSSES, LATE_BOOKS,
  loadClassicSources, validateScenario, validateEndpoint, validateWebOrigin, assertPair, assertCommand, freshSnapshot,
  exactNpc, npcLink, bagCount, groundItemIdentity, bossCatalog, redactEvidence } from './classic-late-route-policy.mjs';

const sleep = ms => new Promise(resolve=>setTimeout(resolve,ms));
const mapName = client => String(client.snapshot?.mapFileName ?? '');
const receivedAfter = (client, after, predicate) => client.events.find(event=>event.direction==='received' && event.sequence>after && predicate(event));
const mapPacketName = event => event.payload?.info?.fileName ?? event.payload?.fileName;
const skillKnown = (snapshot,name) => (snapshot?.knownSkills??[]).some(skill=>[skill.name,skill.key].some(value=>String(value??'').replaceAll('-','').toLowerCase()===name.replaceAll('-','').toLowerCase()));

/** Separate transport because the existing newcomer ProtocolClient explicitly
 * limits itself to loopback. No fake URL or bypass of that class's guard. */
export class ClassicProtocolClient {
  constructor(url, output, {WebSocketImpl=globalThis.WebSocket, appendFile=fs.appendFile, secrets=[], now=Date.now,
    setTimeoutImpl=setTimeout,clearTimeoutImpl=clearTimeout,webOrigin=new URL(url.replace(/^ws/,'http')).origin}={}) {
    validateEndpoint(url); this.url=url; this.output=output; this.WebSocketImpl=WebSocketImpl;
    this.webOrigin=validateWebOrigin(webOrigin);
    this.appendFile=appendFile; this.secrets=secrets; this.now=now; this.events=[]; this.sequence=0;
    this.snapshot=null; this.writeQueue=Promise.resolve(); this.questDefinitions=new Map();
    this.scheduleTimeout=setTimeoutImpl;this.cancelTimeout=clearTimeoutImpl;this.lastVitalsRefreshAt=null;this.vitalsTimer=null;
  }
  record(direction,value) {
    const event={...redactEvidence(structuredClone(value),this.secrets), movementDirection:value.direction,
      sequence:++this.sequence, at:new Date(this.now()).toISOString(), direction};
    this.events.push(event); if(this.events.length>10000)this.events.splice(0,1000);
    this.writeQueue=this.writeQueue.then(()=>this.appendFile(this.output,JSON.stringify(event)+'\n'));
    this.writeQueue.catch(error=>{this.failure=error;});
    return event;
  }
  observeGatewayMessage(message) {
    this.snapshot=applyProtocolObservation(this.snapshot,message); this.record('received',message);
    if(message.packet==='NewQuestInfo')this.questDefinitions.set(Number(message.payload?.info?.index),message.payload.info);
    if(message.type==='error')this.failure=new Error(`Gateway rejected ordinary command: ${message.message??'unknown'}`);
    if(message.packet==='DamageIndicator'&&Number(message.payload?.objectId)===Number(this.snapshot?.playerObjectId))this.refreshDamageVitals();
  }
  refreshDamageVitals() {
    if(this.ws?.readyState!==1)return;
    const elapsed=this.lastVitalsRefreshAt==null?500:this.now()-this.lastVitalsRefreshAt;
    if(elapsed>=500&&this.vitalsTimer==null){this.lastVitalsRefreshAt=this.now();this.send({type:'clientVersion'});return;}
    if(this.vitalsTimer!=null)return;
    this.vitalsTimer=this.scheduleTimeout(()=>{this.vitalsTimer=null;if(this.ws?.readyState!==1||this.failure)return;
      this.lastVitalsRefreshAt=this.now();this.send({type:'clientVersion'});},Math.max(0,500-elapsed));
  }
  questDefinition(id) {return this.questDefinitions.get(Number(id))??null;}
  async connect() {
    this.record('lifecycle',{type:'connection',url:this.url,webOrigin:this.webOrigin});
    // Node >=22 WebSocketInit supports headers. The ordinary server Origin
    // allowlist remains enforced; its exact permitted origin is root-attested.
    this.ws=new this.WebSocketImpl(this.url,{headers:{Origin:this.webOrigin}});
    this.ws.addEventListener('message',event=>{try{this.observeGatewayMessage(JSON.parse(event.data));}catch(error){this.failure=error;}});
    this.ws.addEventListener('error',()=>{this.failure=new Error('Classic Gateway transport failed');});
    this.ws.addEventListener('close',()=>{this.closed=true;});
    await this.wait(()=>this.ws.readyState===1,'authenticated transport open');
    this.send({type:'clientVersion'});
    this.timer=setInterval(()=>{if(this.ws.readyState===1)this.send({type:'keepAlive',time:this.now()});},2000);
    this.stopHandler=()=>{this.failure=new Error('Classic journey interrupted; normal logout cleanup is required');};
    process.once('SIGINT',this.stopHandler);process.once('SIGTERM',this.stopHandler);
  }
  send(command) {
    assertCommand(command);
    if(this.failure)throw this.failure;
    if(this.ws?.readyState!==1)throw new Error('Classic transport is not open');
    this.record('sent',command); this.ws.send(JSON.stringify(command));
  }
  async wait(predicate,label,timeout=20000) {
    const deadline=this.now()+timeout;
    while(this.now()<deadline) {
      if(this.failure)throw this.failure;
      const result=predicate();if(result)return result;
      if(this.closed)throw new Error(`Connection closed waiting for ${label}`);
      await sleep(40);
    }
    throw new Error(`Timeout waiting for ${label}`);
  }
  async request(command,packet,timeout) {
    const after=this.sequence;this.send(command);
    return this.wait(()=>receivedAfter(this,after,event=>event.packet===packet),packet,timeout);
  }
  async close() {clearInterval(this.timer);if(this.vitalsTimer!=null)this.cancelTimeout(this.vitalsTimer);
    if(this.stopHandler){process.removeListener('SIGINT',this.stopHandler);process.removeListener('SIGTERM',this.stopHandler);}
    if(this.ws?.readyState===1)this.ws.close();await this.writeQueue;}
}

export async function refreshSnapshot(client,map=mapName(client)) {
  const after=client.sequence;client.send({type:'clientVersion'});
  return client.wait(()=>freshSnapshot(client.events,after,map),`fresh source snapshot ${map}`);
}

/** A static portal is only a constraint. The hop requires a fresh live
 * transfer, real movement, a server map packet and a settled destination. */
export async function executeClassicHop(client,travel,edge,routeId,index) {
  if(edge.from===SOURCE_ONLY_ROOM||edge.to===SOURCE_ONLY_ROOM)throw new Error('Resource-only D71653 is not a player route');
  if(mapName(client)!==edge.from||client.snapshot?.mapSnapshotPending)throw new Error('Wrong/stale hop origin');
  const source=await refreshSnapshot(client,edge.from);
  const transfers=(source.payload.mapTransfers??[]).filter(transfer=>transfer.toMapFileName===edge.to &&
    edge.portals.some(portal=>portal.source.x>=transfer.bounds?.minX && portal.source.x<=transfer.bounds?.maxX &&
      portal.source.y>=transfer.bounds?.minY && portal.source.y<=transfer.bounds?.maxY));
  if(!transfers.length)throw new Error(`No live walking transfer ${edge.from}->${edge.to}`);
  const before=client.sequence;
  // Some original manifest destinations are invalid terrain/out of bounds.
  // Prefer a source the live ValidPoint-gated runtime actually publishes;
  // an authored but unusable first record is not a doorway substitute.
  const preferred=edge.portals.find(portal=>transfers.some(transfer=>
    portal.source.x>=transfer.bounds?.minX && portal.source.x<=transfer.bounds?.maxX &&
    portal.source.y>=transfer.bounds?.minY && portal.source.y<=transfer.bounds?.maxY));
  const traversed=await travel(edge.to,{preferredTransferSource:preferred.source,resolveBlockingMonster:false,
    navigationOptions:{detectPositionCycles:true}});
  if(traversed.length!==1||traversed[0].fromMapFileName!==edge.from||traversed[0].toMapFileName!==edge.to)throw new Error('Unexpected intermediate route/landing cannot be credited');
  const transition=receivedAfter(client,before,event=>['MapChanged','MapInformation'].includes(event.packet)&&mapPacketName(event)===edge.to);
  if(!transition)throw new Error('No authoritative fresh map transition packet');
  let settled=freshSnapshot(client.events,transition.sequence,edge.to);
  if(!settled)settled=await refreshSnapshot(client,edge.to);
  const actor=settled.payload.entities?.find(actor=>actor.objectId===settled.payload.playerObjectId);
  if(!actor||!edge.portals.some(portal=>Number(actor.x)===portal.destination.x && Number(actor.y)===portal.destination.y))throw new Error('Wrong destination position after real transition');
  const movements=client.events.filter(event=>event.direction==='sent'&&event.sequence>before&&event.sequence<transition.sequence&&['walk','run'].includes(event.type));
  if(!movements.length)throw new Error('Transition without ordinary walking/running cannot be credited');
  const sourcePosition=receivedAfter(client,before,event=>event.sequence<transition.sequence && event.packet==='UserLocation' &&
    edge.portals.some(portal=>Number(event.payload?.location?.x)===portal.source.x && Number(event.payload?.location?.y)===portal.source.y));
  return {routeId,index,from:edge.from,to:edge.to,status:'observed',beforeSequence:before,sourceSnapshotSequence:source.sequence,
    transitionSequence:transition.sequence,destinationSnapshotSequence:settled.sequence,afterSequence:client.sequence,
    sourcePositionSequence:sourcePosition?.sequence??null,sourcePositionProof:sourcePosition?'direct-UserLocation':'transition-implied-not-direct'};
}

export async function observeClassicNpc(client,navigate,template,target=null) {
  if(mapName(client)!==template.map_file_name)throw new Error('NPC is not in the current authoritative map');
  await navigate(template.location,1);
  const snapshot=await refreshSnapshot(client,template.map_file_name);
  const npc=exactNpc(snapshot.payload,template);
  if(!npc)throw new Error('Missing fresh exact live NPC identity');
  const actor=snapshot.payload.entities?.find(actor=>actor.objectId===snapshot.payload.playerObjectId);
  if(!actor||Math.max(Math.abs(actor.x-npc.x),Math.abs(actor.y-npc.y))>1)throw new Error('NPC needs an ordinary adjacent player');
  const before=client.sequence;client.send({type:'interact',objectId:npc.objectId});
  const dialog=await client.wait(()=>receivedAfter(client,before,event=>event.type==='worldSnapshot'&&event.payload?.mapFileName===template.map_file_name
    &&event.payload?.activeNpcDialog?.npcObjectId===npc.objectId),'fresh actual NPC body/links');
  const link=target?npcLink(dialog.payload,npc.objectId,target):null;
  return {npcObjectId:npc.objectId,scriptKey:template.script_key,map:template.map_file_name,sourceSnapshotSequence:snapshot.sequence,
    dialogSequence:dialog.sequence,body:dialog.payload.activeNpcDialog.body,links:dialog.payload.activeNpcDialog.links,
    ...(link?{target:link.target}:{})};
}

export async function useNaturallyPickedUpBook(client,pickup,className,sources) {
  if(!pickup||pickup.status!=='pickedUp'||pickup.provenance!=='fresh-kill-associated-ground-item')throw new Error('Book requires its own observed ground/pickup receipt');
  const before=await refreshSnapshot(client);
  const item=before.payload.inventoryItems?.find(item=>String(item.uniqueId)===String(pickup.uniqueId));
  const template=sources.items.items.find(template=>template.item_index===Number(item?.tooltipSource?.info?.index??item?.key?.replace('crystal-item-','')));
  const actor=selfPlayer(client);
  if(!template||template.item_type!==20||!LATE_BOOKS[className]?.includes(template.name)||!sources.profile.itemWhitelist.includes(template.name)
    ||!(template.required_class & ({Warrior:1,Wizard:2,Taoist:4}[className]))||template.required_type!==0||actor.level<template.required_amount
    ||skillKnown(before.payload,template.name)||bagCount(before.payload,pickup.uniqueId)<1)throw new Error('Late book class/profile/level/owned/unknown-skill eligibility failed');
  const ack=await client.request({type:'useItem',grid:'inventory',uniqueId:pickup.uniqueId},'UseItem');
  if(ack.payload?.success!==true||String(ack.payload?.uniqueId)!==String(pickup.uniqueId))throw new Error('Owned late book UseItem ACK failed');
  const after=await refreshSnapshot(client);
  const magic=receivedAfter(client,before.sequence,event=>event.packet==='NewMagic'&&event.payload?.magic?.spell===template.name);
  if(bagCount(after.payload,pickup.uniqueId)!==bagCount(before.payload,pickup.uniqueId)-1||!skillKnown(after.payload,template.name)||!magic)throw new Error('Book needs one debit, NewMagic and learned authority');
  return {spell:template.name,itemIndex:template.item_index,uniqueId:pickup.uniqueId,pickupId:pickup.id,
    beforeSnapshotSequence:before.sequence,useAckSequence:ack.sequence,newMagicSequence:magic.sequence,afterSnapshotSequence:after.sequence,
    status:'learned',activationAccepted:false};
}

export async function normalLogoutRelogin(client,scenario) {
  const before=await refreshSnapshot(client);
  const logout=await client.request({type:'logOut'},'LogOutSuccess',20000);
  const start=await client.request({type:'startGame',characterIndex:scenario.characterIndex},'StartGame');
  if(Number(start.payload?.result)!==4)throw new Error('Normal saved character restart rejected');
  const after=await refreshSnapshot(client,before.payload.mapFileName);
  const a=before.payload.entities?.find(entity=>entity.objectId===before.payload.playerObjectId);
  const b=after.payload.entities?.find(entity=>entity.objectId===after.payload.playerObjectId);
  if(a?.name!==scenario.characterName||b?.name!==scenario.characterName||a?.x!==b?.x||a?.y!==b?.y)throw new Error('Normal logout/relogin transform identity differs');
  for(const item of before.payload.inventoryItems??[])if(bagCount(before.payload,item.uniqueId)!==bagCount(after.payload,item.uniqueId))throw new Error('Normal save lost/changed a held unique-ID item');
  for(const skill of before.payload.knownSkills??[])if(!skillKnown(after.payload,skill.name??skill.key))throw new Error('Normal save lost a learned skill');
  const finalLogout=await client.request({type:'logOut'},'LogOutSuccess',20000);
  return {mechanism:'normal-LogOutSuccess-and-public-relogin',beforeSnapshotSequence:before.sequence,
    logoutSequence:logout.sequence,startSequence:start.sequence,reloginSnapshotSequence:after.sequence,finalLogoutSequence:finalLogout.sequence,
    rawDurableStoreBytesVerified:false,status:'observed'};
}

async function attemptNaturalBoss(client,navigate,name,sources,scenario,report,checkDeadline) {
  const catalog=bossCatalog(sources,name);
  for(let ordinal=0;ordinal<scenario.maxKillsPerTarget;ordinal++) {
    checkDeadline();const attempt={id:`${name}-${ordinal+1}`,name,map:catalog.policy.map,ordinal:ordinal+1,
      startedAt:new Date().toISOString(),beforeSequence:client.sequence,status:'running',outcome:'notObservedWithinBudget',
      sourceDropPath:catalog.policy.dropPath,sourceRespawnIndexes:catalog.policy.respawns,dropDenominatorsUnchanged:true,
      naturalSpawnAccepted:false,combatAccepted:false,groundDropSequences:[],pickupIds:[]};
    report.bossAttempts.push(attempt);
    try {
      let target;const waitUntil=Math.min(report.deadlineMs,Date.now()+scenario.spawnWaitMs);
      while(Date.now()<waitUntil&&!target) {
        checkDeadline();await refreshSnapshot(client,catalog.policy.map);
        target=client.snapshot.entities?.find(entity=>entity.name===name&&String(entity.kind).toLowerCase()==='monster'&&!entity.dead&&Number(entity.hp)>0);
        if(!target)await sleep(Math.min(2000,Math.max(1,waitUntil-Date.now())));
      }
      if(!target){attempt.status='incomplete';attempt.outcome='absentWithinBudget';continue;}
      attempt.objectId=target.objectId;attempt.observedTarget=structuredClone(target);
      // Current public snapshots may not expose source respawn identity. That
      // missing field is recorded, never supplied from static nearby spawn data.
      attempt.sourceRespawnIndex=target.sourceRespawnIndex??target.respawnIndex??null;
      attempt.naturalSpawnAccepted=catalog.policy.respawns.includes(Number(attempt.sourceRespawnIndex));
      if(Number(target.hp)!==catalog.monster.hp)throw new Error('Boss was already damaged or lacks exact imported initial HP');
      const dropsBefore=new Set((client.snapshot.groundDrops??[]).map(drop=>String(drop.objectId)));
      const killed=await clearTravelBlockingMonster(client,target,navigate,{action:combatAction,approachRange:combatApproachRange,
        sustain:async owner=>{await useSupplies(owner,{hpThreshold:0.75,mpThreshold:0.4});await useClassRecovery(owner,{hpThreshold:0.7});},
        sleep:async ms=>{checkDeadline();await sleep(Math.min(ms,Math.max(1,report.deadlineMs-Date.now())));},
        maxAttackAttempts:120,attackCadenceMs:650,refreshWhileWaiting:refreshCombatWorldSnapshot});
      const death=receivedAfter(client,attempt.beforeSequence,event=>event.packet==='ObjectDied'&&Number(event.payload?.info?.objectId)===Number(target.objectId));
      if(!killed.cleared||!death)throw new Error('Disappearance/AOI remove cannot substitute for actual ObjectDied');
      attempt.deathSequence=death.sequence;attempt.outcome='observedKill';attempt.status='observed';
      const settled=await refreshSnapshot(client,catalog.policy.map);
      attempt.settlementSnapshotSequence=settled.sequence;
      const drops=(settled.payload.groundDrops??[]).filter(drop=>!dropsBefore.has(String(drop.objectId))&&drop.sourceMonster===name);
      attempt.noDrop=drops.length===0;attempt.groundDropSequences=drops.map(drop=>({objectId:drop.objectId,snapshotSequence:settled.sequence,kind:drop.loot?.kind,name:drop.name}));
      for(const drop of drops.filter(drop=>drop.loot?.kind==='inventoryItem')) {
        if(Number(drop.ownerObjectId)!==Number(client.snapshot.playerObjectId))continue; // outsider/group ownership is a separate lane
        const identity=groundItemIdentity(drop);
        await navigate(drop,0);
        const before=await refreshSnapshot(client,catalog.policy.map);const beforeIds=new Set((before.payload.inventoryItems??[]).map(item=>String(item.uniqueId)));
        const pickAfter=client.sequence;client.send({type:'pickUp',objectId:drop.objectId});
        const after=await refreshSnapshot(client,catalog.policy.map);
        const item=after.payload.inventoryItems?.find(item=>!beforeIds.has(String(item.uniqueId))
          && (identity?.uniqueId == null || identity.uniqueId === String(item.uniqueId))
          && (item.key===drop.loot?.key||item.name===drop.name)&&Number(item.quantity)===Number(drop.quantity));
        const pickup={id:`${attempt.id}-drop-${drop.objectId}`,attemptId:attempt.id,groundObjectId:drop.objectId,groundSnapshotSequence:settled.sequence,
          beforeSnapshotSequence:before.sequence,afterSnapshotSequence:after.sequence,beforeSequence:pickAfter,provenance:'fresh-kill-associated-ground-item',
          // Public name association is useful evidence but not exact source-kill
          // ownership proof. Root's independent server journal gate remains open.
          sourceKillIdentityAccepted:false,sourceItemUidAccepted:identity?.uniqueId != null && identity.uniqueId === String(item?.uniqueId),
          ownershipAccepted:Number(drop.ownerObjectId)===Number(before.payload.playerObjectId),
          status:item?'pickedUp':'failedPickup',uniqueId:item?.uniqueId??null,itemName:drop.name,quantity:drop.quantity};
        report.pickups.push(pickup);attempt.pickupIds.push(pickup.id);
        const itemTemplate=sources.items.items.find(template=>`crystal-item-${template.item_index}`===item?.key);
        if(item&&LATE_BOOKS[scenario.className]?.includes(itemTemplate?.name)) {
          try {report.books.push(await useNaturallyPickedUpBook(client,pickup,scenario.className,sources));}
          catch(error){report.books.push({pickupId:pickup.id,status:'failed',error:error.message});}
        }
      }
    } catch(error){attempt.status='failed';attempt.error=error.message;attempt.outcome=hasAuthoritativePlayerDeath(client.snapshot)?'playerDeath':'failedCombat';}
    finally {attempt.finishedAt=new Date().toISOString();attempt.afterSequence=client.sequence;}
    if(hasAuthoritativePlayerDeath(client.snapshot))throw new Error('Player death retained; no automatic revive/reset/retry-to-pass');
  }
}

export async function runClassicLateRoute({input,output,mapPackRoot,pairReceiptFile,pairReceiptSha256},dependencies={}) {
  const scenario=validateScenario(input);const sources=await loadClassicSources();
  const pairBytes=await fs.readFile(pairReceiptFile);
  if(!/^[a-f0-9]{64}$/.test(pairReceiptSha256??'')||sha256(pairBytes)!==pairReceiptSha256)throw new Error('Root paired receipt hash mismatch');
  const pair=JSON.parse(pairBytes);
  const endpoint=validateEndpoint(scenario.gatewayUrl);endpoint.protocol=endpoint.protocol==='wss:'?'https:':'http:';endpoint.pathname='/health';
  const healthResponse=await (dependencies.fetch??fetch)(endpoint,{signal:AbortSignal.timeout(10000)});
  if(!healthResponse.ok)throw new Error(`Paired Gateway health HTTP${healthResponse.status}`);
  const health=await healthResponse.json();const pairIdentity=assertPair(pair,health,scenario);
  const root=path.resolve(output);await fs.mkdir(root,{recursive:true});
  const reportFile=path.join(root,'report.json');
  try{await fs.access(reportFile);throw new Error('Use a NEW evidence directory; never overwrite an old run');}catch(error){if(error.code!=='ENOENT')throw error;}
  const report={schema:'mir2.classic-late-route-run.v1',className:scenario.className,lane:scenario.lane,startingState:scenario.startingState,
    gatewayUrl:scenario.gatewayUrl,webOrigin:scenario.webOrigin,accountFingerprint:sha256(scenario.accountId),sourceHashes:{...SOURCE_HASHES},pairedReceiptSha256:pairReceiptSha256,pairIdentity,health,
    declaredNaturalKillLimit:scenario.maxKillsPerTarget,spawnWaitMs:scenario.spawnWaitMs,timeoutMs:scenario.timeoutMs,
    startedAt:new Date().toISOString(),deadlineMs:Date.now()+scenario.timeoutMs,status:'running',
    freshLevelJourneyAccepted:false,runtimeGameplayAccepted:false,nativeVisualAccepted:false,
    routes:[],npcActions:[],bossAttempts:[],pickups:[],books:[],failures:[],deaths:[],save:null,preservedDefects:[]};
  const redact=value=>redactEvidence(value,[scenario.password,scenario.accountId]);
  const write=()=>fs.writeFile(reportFile,JSON.stringify(redact(report),null,2));
  const checkDeadline=()=>{if(Date.now()>=report.deadlineMs)throw new Error('Original ordinary route deadline exhausted');};
  const client=dependencies.client??new ClassicProtocolClient(scenario.gatewayUrl,path.join(root,'trace.jsonl'),{secrets:[scenario.password,scenario.accountId],webOrigin:scenario.webOrigin});
  const bounded=new Proxy(client,{get(target,key){
    if(['send','request','wait'].includes(key))return(...args)=>{checkDeadline();if(key!=='wait')assertCommand(args[0]);
      if(key==='wait')return target.wait(args[0],args[1],Math.min(args[2]??20000,Math.max(1,report.deadlineMs-Date.now())));
      return target[key](...args);};
    const value=Reflect.get(target,key);return typeof value==='function'?value.bind(target):value;
  }});
  const loadCollisionMap=id=>loadProtocolCollisionMap(id,mapPackRoot?{packagedMapRoot:mapPackRoot}:{});
  const navigate=dependencies.navigate??createNavigator(bounded,{loadCollisionMap});
  const travel=dependencies.travel??createMapTraveler(bounded,navigate,{loadCollisionMap,maxBlockingMonsterClears:8}); // NO relocation callback
  let started=false;
  try {
    await client.connect();await bounded.request({type:'login',accountId:scenario.accountId,password:scenario.password},'LoginSuccess');
    const start=await bounded.request({type:'startGame',characterIndex:scenario.characterIndex},'StartGame');
    if(Number(start.payload?.result)!==4)throw new Error('Authenticated StartGame did not accept');
    started=true;const initial=await refreshSnapshot(bounded,scenario.lane==='main-round-trips'?'0':scenario.lane==='stone-entry'?'D715':'D10051');
    const actor=selfPlayer(client);
    if(actor?.name!==scenario.characterName||actor?.class!==scenario.className||Number(actor?.level)!==scenario.startingState.level)throw new Error('Prepared/ordinary starting identity differs');
    report.initialSnapshotSequence=initial.sequence;
    if(scenario.lane==='main-round-trips') {
      for(const family of ['stone','zuma','redmoon']) {
        for(let i=0;i<sources.portals[family].length;i++) {checkDeadline();report.routes.push(await executeClassicHop(bounded,travel,sources.portals[family][i],family,i));await write();}
        for(const [name,boss] of Object.entries(BOSSES))if(boss.map===mapName(client))await attemptNaturalBoss(bounded,navigate,name,sources,scenario,report,checkDeadline);
        const returnId=`${family}-return`;
        for(let i=0;i<sources.portals[returnId].length;i++) {checkDeadline();report.routes.push(await executeClassicHop(bounded,travel,sources.portals[returnId][i],returnId,i));await write();}
      }
    } else if(scenario.lane==='stone-entry') {
      const template=sources.npcs.npcs.find(npc=>npc.loaded_object_id===1142);
      const action=await observeClassicNpc(bounded,navigate,template,'@stonetomba');report.npcActions.push(action);
      const before=await refreshSnapshot(bounded,'D715');const count=snapshot=>(snapshot.inventoryItems??[]).filter(item=>item.key==='crystal-item-1080').reduce((n,item)=>n+Number(item.quantity),0);
      if(count(before.payload)<1)throw new Error('No lawful owned canonical StoneHeart');
      const selected=client.sequence;bounded.send({type:'selectNpcDialog',target:action.target});
      const transition=await bounded.wait(()=>receivedAfter(client,selected,event=>['MapChanged','MapInformation'].includes(event.packet)&&mapPacketName(event)==='D710A'),'ordinary Stone entry');
      const after=await refreshSnapshot(bounded,'D710A');const landed=selfPlayer(client);
      if(landed?.x!==29||landed?.y!==17||count(after.payload)!==count(before.payload)-1)throw new Error('Stone entry lacks exact destination/one-item debit');
      Object.assign(action,{selectionAfterSequence:selected,transitionSequence:transition.sequence,beforeItemSnapshotSequence:before.sequence,afterItemSnapshotSequence:after.sequence,status:'entryObserved'});
      report.preservedDefects.push({id:'ANCIENT-OUTER-WALKING-EXIT',status:'sourceHasNoOuterWalkingEdge',walkingExitAccepted:false,
        note:'D710A-D713A have internal bidirectional edges only. Ordinary owned escape item is a separate return mechanism, never a fabricated walking exit.'});
    } else {
      await navigate(PRESERVED_DEFECT.source,0);const source=await refreshSnapshot(bounded,'D10051');
      const landed=selfPlayer(client);
      if(landed?.x!==178||landed?.y!==53)throw new Error('Unbound source coordinate was not actually reached');
      if(source.payload.mapTransfers?.some(transfer=>transfer.toMapFileName==='D10061'))throw new Error('Unexpected repaired D10061 binding changes frozen defect policy');
      report.preservedDefects.push({id:'D10061-UNBOUND-NEEDMOVE',status:'observedNoEntry',sourceSnapshotSequence:source.sequence,entryAccepted:false});
    }
    report.status='observed';
  } catch(error){report.status='incomplete';report.failures.push({at:new Date().toISOString(),sequence:client.sequence,message:error.message});}
  finally {
    if(hasAuthoritativePlayerDeath(client.snapshot))report.deaths.push({sequence:client.sequence,map:mapName(client),at:new Date().toISOString()});
    if(started&&!client.closed) {
      // Cleanup is allowed beyond the diagnostic deadline to save normally;
      // those extra seconds cannot extend route/combat acceptance.
      const priorFailure=client.failure;client.failure=null;
      try {report.save=await normalLogoutRelogin(client,scenario);}catch(error){report.failures.push({phase:'normal-save',message:error.message});}
      finally {client.failure??=priorFailure;}
    }
    try {await client.close();}catch(error){report.failures.push({phase:'trace-close',message:error.message});}
    report.finishedAt=new Date().toISOString();report.elapsedMs=Date.parse(report.finishedAt)-Date.parse(report.startedAt);
    await write();await fs.copyFile(pairReceiptFile,path.join(root,'paired-gateway-receipt.json'));
  }
  return report;
}

if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const {values}=parseArgs({options:{input:{type:'string'},output:{type:'string'},'map-pack-root':{type:'string'},
    'pair-receipt':{type:'string'},'pair-sha256':{type:'string'},plan:{type:'boolean',default:false}}});
  if(values.plan) {
    await loadClassicSources();console.log(JSON.stringify({schema:'mir2.classic-late-route-plan.v1',routes:ROUTES,sourceHashes:SOURCE_HASHES,
      plannedOnly:true,executed:false,runtimeGameplayAccepted:false,preservedDefect:PRESERVED_DEFECT,resourceOnlyExcluded:SOURCE_ONLY_ROOM},null,2));
  } else {
    if(!values.input||!values.output||!values['pair-receipt'])throw new Error('--input, --output, --pair-receipt, --pair-sha256 are required');
    const report=await runClassicLateRoute({input:JSON.parse(await fs.readFile(values.input)),output:values.output,mapPackRoot:values['map-pack-root'],
      pairReceiptFile:values['pair-receipt'],pairReceiptSha256:values['pair-sha256']});
    console.log(JSON.stringify({status:report.status,runtimeGameplayAccepted:false,reportFile:path.resolve(values.output,'report.json')}));
  }
}
