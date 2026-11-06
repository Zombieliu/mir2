import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import readline from 'node:readline';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';
import { sha256, SOURCE_HASHES, ROUTES, EXPECTED_DIRECTED_HOPS, SOURCE_ONLY_ROOM, PRESERVED_DEFECT,
  BOSSES, CLASSES, LATE_BOOKS, assertCommand, assertPair, loadClassicSources, exactNpc, npcLink, bagCount, groundItemIdentity, bossCatalog } from './classic-late-route-policy.mjs';

const requireValue=(condition,message)=>{if(!condition)throw new Error(message);};
const transitionName=event=>event?.payload?.info?.fileName??event?.payload?.fileName;
const own=snapshot=>snapshot?.entities?.find(entity=>Number(entity.objectId)===Number(snapshot.playerObjectId));
const known=(snapshot,name)=>(snapshot?.knownSkills??[]).some(skill=>[skill.key,skill.name].some(value=>String(value??'').replaceAll('-','').toLowerCase()===String(name).replaceAll('-','').toLowerCase()));
const itemIndex=item=>Number(item?.tooltipSource?.info?.index??String(item?.key??'').replace('crystal-item-',''));

function indexTrace(events) {
  let previous=0;const indexed=new Map();
  for(const event of events) {
    requireValue(Number.isSafeInteger(event.sequence)&&event.sequence>previous,'Trace sequence missing/reordered/duplicated');
    requireValue(Number.isFinite(Date.parse(event.at)),'Trace event timestamp missing');previous=event.sequence;
    if(event.direction==='sent')assertCommand(event);
    requireValue(!['MoveTo','Stage5Command','qa.giveItem','event.spawn','disconnectedRegionRelocation'].includes(event.type),'Private relocation/QA event in counted ledger');
    indexed.set(event.sequence,event);
  }
  return indexed;
}
function packet(indexed,sequence,name) {
  const event=indexed.get(sequence);requireValue(event?.direction==='received'&&event.packet===name,`Missing real ${name} packet at${sequence}`);return event;
}
function snapshot(indexed,sequence,map) {
  const event=indexed.get(sequence);requireValue(event?.direction==='received'&&event.type==='worldSnapshot'
    &&!event.payload?.mapSnapshotPending&&(!map||event.payload.mapFileName===map),`Missing fresh settled snapshot at${sequence}`);return event.payload;
}
function verifyHop(hop,indexed,sources,events) {
  requireValue(hop.status==='observed'&&hop.from!==SOURCE_ONLY_ROOM&&hop.to!==SOURCE_ONLY_ROOM,'Static/resource-only hop is not runtime evidence');
  const edge=sources.portals[hop.routeId]?.[hop.index];
  requireValue(edge&&edge.from===hop.from&&edge.to===hop.to,'Hop does not match exact bound ordinary route');
  const before=snapshot(indexed,hop.sourceSnapshotSequence,edge.from);
  requireValue(hop.sourceSnapshotSequence<=hop.beforeSequence&&hop.beforeSequence<hop.transitionSequence
    &&hop.transitionSequence<hop.destinationSnapshotSequence&&hop.destinationSnapshotSequence<=hop.afterSequence,'Stale or unordered hop receipt');
  requireValue((before.mapTransfers??[]).some(transfer=>transfer.toMapFileName===edge.to&&edge.portals.some(portal=>
    portal.source.x>=transfer.bounds?.minX&&portal.source.x<=transfer.bounds?.maxX&&portal.source.y>=transfer.bounds?.minY&&portal.source.y<=transfer.bounds?.maxY)),'No actual source live transfer');
  const transition=indexed.get(hop.transitionSequence);
  requireValue(transition?.direction==='received'&&['MapChanged','MapInformation'].includes(transition.packet)&&transitionName(transition)===edge.to,'No actual matching map transition');
  const after=snapshot(indexed,hop.destinationSnapshotSequence,edge.to), actor=own(after);
  requireValue(actor&&edge.portals.some(portal=>actor.x===portal.destination.x&&actor.y===portal.destination.y),'Destination differs from source geometry');
  const interval=events.filter(event=>event.sequence>hop.beforeSequence&&event.sequence<hop.transitionSequence);
  requireValue(interval.some(event=>event.direction==='sent'&&['walk','run'].includes(event.type)),'Map change lacks an ordinary movement intent');
  requireValue(!interval.some(event=>event.direction==='sent'&&!['walk','run','turn','keepAlive','clientVersion'].includes(event.type)),'Teleport/NPC/combat command cannot substitute for this walking hop');
  requireValue(!interval.some(event=>event.direction==='received'&&['MapChanged','MapInformation'].includes(event.packet)&&transitionName(event)!==edge.to),'Unexpected intermediate map erased from hop');
  let sourcePositionAccepted=false;
  if(hop.sourcePositionSequence!=null) {
    const location=packet(indexed,hop.sourcePositionSequence,'UserLocation');
    requireValue(location.sequence>hop.beforeSequence&&location.sequence<hop.transitionSequence
      &&edge.portals.some(portal=>location.payload?.location?.x===portal.source.x&&location.payload?.location?.y===portal.source.y),'Forged committed source coordinate');
    sourcePositionAccepted=true;
  }
  return {routeId:hop.routeId,index:hop.index,transitionAccepted:true,sourcePositionAccepted};
}
function verifyNpc(action,indexed,sources) {
  const template=sources.npcs.npcs.find(npc=>Number(npc.loaded_object_id)===Number(action.npcObjectId));
  requireValue(template&&template.script_key===action.scriptKey&&template.map_file_name===action.map,'Wrong imported NPC proof');
  const before=snapshot(indexed,action.sourceSnapshotSequence,action.map);
  requireValue(exactNpc(before,template),'Source NPC template was not observed as an exact live object');
  const actor=own(before);
  requireValue(actor&&Math.max(Math.abs(actor.x-template.location.x),Math.abs(actor.y-template.location.y))<=1,'NPC player was not adjacent');
  const page=snapshot(indexed,action.dialogSequence,action.map);
  requireValue(action.sourceSnapshotSequence<action.dialogSequence&&page.activeNpcDialog?.npcObjectId===action.npcObjectId,'NPC dialog is stale/wrong object');
  requireValue(JSON.stringify(page.activeNpcDialog.body)===JSON.stringify(action.body)
    &&JSON.stringify(page.activeNpcDialog.links)===JSON.stringify(action.links),'Reported NPC body/links differ from actual received page');
  if(action.target)npcLink(page,action.npcObjectId,action.target);
  if(action.status==='entryObserved') {
    requireValue(action.scriptKey==='MongchonProvince/StoneTemple/Stone'&&String(action.target).toLowerCase()==='@stonetomba','Only reviewed Stone entry belongs to this lane');
    const prior=snapshot(indexed,action.beforeItemSnapshotSequence,'D715');
    const after=snapshot(indexed,action.afterItemSnapshotSequence,'D710A');
    const count=s=>(s.inventoryItems??[]).filter(item=>itemIndex(item)===1080).reduce((n,item)=>n+Number(item.quantity),0);
    const quest=prior.questLog?.find(quest=>Number(quest.questId)===153);
    requireValue(quest?.stage==='completed'&&own(prior)?.level>=22&&own(prior)?.level<=42,'Stone real quest/level gate was not observed');
    requireValue(count(prior)>=1&&count(after)===count(prior)-1&&own(after)?.x===29&&own(after)?.y===17,'Stone canonical one-item debit/landing differs');
    const transition=indexed.get(action.transitionSequence);
    requireValue(transition?.direction==='received'&&['MapChanged','MapInformation'].includes(transition.packet)&&transitionName(transition)==='D710A'
      &&action.selectionAfterSequence<transition.sequence&&transition.sequence<action.afterItemSnapshotSequence,'Missing fresh Stone map receipt');
  }
  return {npcObjectId:action.npcObjectId,liveDialogAccepted:true,entryAccepted:action.status==='entryObserved'};
}
function verifySave(save,indexed) {
  if(!save)return false;
  requireValue(save.status==='observed'&&save.mechanism==='normal-LogOutSuccess-and-public-relogin','Normal save receipt required');
  const before=snapshot(indexed,save.beforeSnapshotSequence);
  packet(indexed,save.logoutSequence,'LogOutSuccess');const start=packet(indexed,save.startSequence,'StartGame');
  requireValue(start.payload?.result===4,'Saved character StartGame was not accepted');
  const after=snapshot(indexed,save.reloginSnapshotSequence,before.mapFileName);packet(indexed,save.finalLogoutSequence,'LogOutSuccess');
  requireValue(save.beforeSnapshotSequence<save.logoutSequence&&save.logoutSequence<save.startSequence
    &&save.startSequence<save.reloginSnapshotSequence&&save.reloginSnapshotSequence<save.finalLogoutSequence,'Save receipt order invalid');
  requireValue(own(before)?.name===own(after)?.name&&own(before)?.x===own(after)?.x&&own(before)?.y===own(after)?.y,'Normal saved transform differs');
  for(const item of before.inventoryItems??[])requireValue(bagCount(before,item.uniqueId)===bagCount(after,item.uniqueId),'Saved unique-ID count differs');
  for(const skill of before.knownSkills??[])requireValue(known(after,skill.key??skill.name),'Learned skill missing after normal relogin');
  return true;
}
function verifyAttempts(report,indexed,events,sources) {
  const result=[];const ids=new Set();const counts={};
  for(const attempt of report.bossAttempts??[]) {
    requireValue(!ids.has(attempt.id)&&BOSSES[attempt.name]&&attempt.map===BOSSES[attempt.name].map,'Duplicate/unknown Boss attempt');ids.add(attempt.id);
    counts[attempt.name]=(counts[attempt.name]??0)+1;requireValue(counts[attempt.name]<=report.declaredNaturalKillLimit&&counts[attempt.name]<=3,'Natural kill budget widened');
    requireValue(Number.isFinite(Date.parse(attempt.startedAt))&&Number.isFinite(Date.parse(attempt.finishedAt))&&attempt.beforeSequence<=attempt.afterSequence,'Failed/absent attempt was not retained');
    requireValue(['observedKill','absentWithinBudget','failedCombat','playerDeath'].includes(attempt.outcome),'Drop/engagement outcome omitted');
    if(attempt.outcome!=='observedKill'){requireValue(attempt.combatAccepted!==true,'Failed/absent combat claimed accepted');result.push({id:attempt.id,combatAccepted:false,outcome:attempt.outcome});continue;}
    const died=packet(indexed,attempt.deathSequence,'ObjectDied');
    requireValue(Number(died.payload?.info?.objectId)===Number(attempt.objectId),'Wrong Boss ObjectDied');
    requireValue(events.some(event=>event.direction==='sent'&&event.sequence>attempt.beforeSequence&&event.sequence<attempt.deathSequence
      &&['attack','attackDirection','magic'].includes(event.type)),'Boss lacks an ordinary combat action');
    const catalog=bossCatalog(sources,attempt.name);
    requireValue(events.some(event=>event.direction==='received'&&event.type==='worldSnapshot'&&event.sequence>attempt.beforeSequence&&event.sequence<attempt.deathSequence
      &&event.payload?.mapFileName===attempt.map&&event.payload?.entities?.some(actor=>Number(actor.objectId)===Number(attempt.objectId)
        &&actor.name===attempt.name&&Number(actor.hp)===catalog.monster.hp)),'Boss lacks an observed original full-HP identity');
    requireValue(events.some(event=>event.direction==='received'&&event.sequence>attempt.beforeSequence&&event.sequence<attempt.deathSequence
      &&((event.packet==='DamageIndicator'&&Number(event.payload?.objectId)===Number(attempt.objectId)&&Number(event.payload?.amount)>0)
        ||(event.packet==='ObjectHealth'&&Number(event.payload?.objectId)===Number(attempt.objectId)))),'Boss lacks received target vitals/damage');
    requireValue(typeof attempt.noDrop==='boolean'&&Array.isArray(attempt.groundDropSequences),'No-drop result omitted; probability is not a guarantee');
    const settlement=snapshot(indexed,attempt.settlementSnapshotSequence,attempt.map);
    for(const drop of attempt.groundDropSequences)requireValue(drop.snapshotSequence===attempt.settlementSnapshotSequence
      &&settlement.groundDrops?.some(actual=>actual.objectId===drop.objectId&&actual.sourceMonster===attempt.name),'Ground-drop receipt invented');
    requireValue(attempt.noDrop===(attempt.groundDropSequences.length===0),'No-drop ledger contradicts actual count');
    // A static spawn index copied into a report is not a live spawn receipt.
    const observation=events.find(event=>event.direction==='received'&&event.type==='worldSnapshot'&&event.sequence>attempt.beforeSequence&&event.sequence<attempt.deathSequence
      &&event.payload?.mapFileName===attempt.map&&event.payload?.entities?.some(actor=>Number(actor.objectId)===Number(attempt.objectId)
        &&actor.name===attempt.name&&BOSSES[attempt.name].respawns.includes(Number(actor.sourceRespawnIndex??actor.respawnIndex))));
    const naturalSpawnAccepted=Boolean(observation);
    requireValue(!attempt.naturalSpawnAccepted||naturalSpawnAccepted,'Source spawn index was not actually exposed live');
    result.push({id:attempt.id,combatObserved:true,naturalSpawnAccepted,combatAccepted:naturalSpawnAccepted,outcome:attempt.outcome,noDrop:attempt.noDrop});
  }
  requireValue(result.length<=12,'Total natural sample budget widened');return result;
}
function verifyAcquisition(report,indexed,events,sources) {
  const pickups=[];
  for(const pickup of report.pickups??[]) {
    const attempt=report.bossAttempts.find(attempt=>attempt.id===pickup.attemptId);
    requireValue(attempt?.outcome==='observedKill'&&pickup.provenance==='fresh-kill-associated-ground-item','Pickup has no real kill/drop provenance ledger');
    const ground=snapshot(indexed,pickup.groundSnapshotSequence,attempt.map).groundDrops?.find(drop=>drop.objectId===pickup.groundObjectId);
    requireValue(ground?.loot?.kind==='inventoryItem'&&ground.sourceMonster===attempt.name,'Gold/preexisting/vendor item cannot count as a natural gear/book drop');
    const identity=groundItemIdentity(ground);
    const before=snapshot(indexed,pickup.beforeSnapshotSequence,attempt.map),after=snapshot(indexed,pickup.afterSnapshotSequence,attempt.map);
    requireValue(events.some(event=>event.direction==='sent'&&event.type==='pickUp'&&Number(event.objectId)===Number(ground.objectId)
      &&event.sequence>pickup.beforeSnapshotSequence&&event.sequence<pickup.afterSnapshotSequence),'Missing ordinary actual ground pickup intent');
    const owner=Number(ground.ownerObjectId)===Number(before.playerObjectId);
    requireValue(!pickup.ownershipAccepted||owner,'Wrong live drop ownership');
    if(pickup.status==='pickedUp') {
      requireValue(bagCount(before,pickup.uniqueId)===0&&bagCount(after,pickup.uniqueId)===Number(pickup.quantity),'Natural new unique-ID/count delta absent');
      const acquired=after.inventoryItems?.find(item=>String(item.uniqueId)===String(pickup.uniqueId));
      requireValue(acquired?.key===ground.loot.key,'Acquired item does not match the authoritative ground template');
      requireValue(identity?.uniqueId == null || identity.uniqueId===String(pickup.uniqueId),
        'Assigned ground UID does not match the actual picked-up UID');
      const learned=(report.books??[]).some(book=>book.pickupId===pickup.id&&book.status==='learned');
      if(report.save&&!learned)requireValue(bagCount(snapshot(indexed,report.save.reloginSnapshotSequence),pickup.uniqueId)===Number(pickup.quantity),'Acquired gear/book was not retained through normal save');
    }
    // Exact source-kill linkage is not exposed by sourceMonster name alone.
    // Require a real linked ID in the authoritative ground snapshot; do not
    // grant an inferred nearby monster or a report-only server flag.
    const exactKillLink=Number(ground.sourceMonsterObjectId)===Number(attempt.objectId);
    const exactItemUid=identity?.uniqueId != null && identity.uniqueId===String(pickup.uniqueId);
    requireValue(!pickup.sourceItemUidAccepted||exactItemUid,'Ground UID allocation was not actually exposed');
    requireValue(!pickup.sourceKillIdentityAccepted||exactKillLink,'Exact kill link is absent from public authoritative ground evidence');
    pickups.push({id:pickup.id,pickupObserved:pickup.status==='pickedUp',ownershipAccepted:owner,
      sourceItemUidAccepted:exactItemUid,naturalAcquisitionAccepted:owner&&exactKillLink&&pickup.status==='pickedUp'});
  }
  const books=[];
  for(const book of report.books??[]) {
    if(book.status==='failed'){books.push({pickupId:book.pickupId,learnAccepted:false});continue;}
    const pickup=report.pickups.find(pickup=>pickup.id===book.pickupId);
    requireValue(pickup?.status==='pickedUp'&&String(pickup.uniqueId)===String(book.uniqueId),'Book unique-ID does not link to its pickup');
    const before=snapshot(indexed,book.beforeSnapshotSequence),after=snapshot(indexed,book.afterSnapshotSequence);
    const info=sources.items.items.find(item=>item.item_index===book.itemIndex);
    requireValue(info?.item_type===20&&LATE_BOOKS[report.className]?.includes(info.name)&&sources.profile.itemWhitelist.includes(info.name)
      &&(info.required_class&({Warrior:1,Wizard:2,Taoist:4}[report.className]))&&own(before)?.level>=info.required_amount,'Book eligibility was not observed');
    requireValue(!known(before,book.spell)&&known(after,book.spell)&&bagCount(after,book.uniqueId)===bagCount(before,book.uniqueId)-1,'Book skill/debit authority inconsistent');
    const ack=packet(indexed,book.useAckSequence,'UseItem'),magic=packet(indexed,book.newMagicSequence,'NewMagic');
    requireValue(ack.payload?.success===true&&String(ack.payload.uniqueId)===String(book.uniqueId)&&magic.payload?.magic?.spell===book.spell,'Book needs success ACK+NewMagic');
    requireValue(book.beforeSnapshotSequence<book.useAckSequence&&book.useAckSequence<book.afterSnapshotSequence&&book.newMagicSequence>book.beforeSnapshotSequence
      &&book.newMagicSequence<book.afterSnapshotSequence,'Stale book ACK/skill receipt');
    const acquisition=pickups.find(entry=>entry.id===book.pickupId);
    requireValue(report.save&&known(snapshot(indexed,report.save.reloginSnapshotSequence),book.spell),'Newly learned book skill was not retained through normal save');
    books.push({pickupId:book.pickupId,spell:book.spell,learnAccepted:true,naturalAcquisitionAccepted:acquisition?.naturalAcquisitionAccepted===true,activationAccepted:false});
    requireValue(book.activationAccepted!==true,'Learning alone cannot prove ordinary skill activation/damage');
  }
  return {pickups,books};
}

export function verifyClassicEvidence({report,events,pair,sources,pairReceiptSha256}) {
  requireValue(report?.schema==='mir2.classic-late-route-run.v1'&&CLASSES.includes(report.className),'Unknown classic evidence schema/class');
  for(const [key,hash]of Object.entries(SOURCE_HASHES))requireValue(report.sourceHashes?.[key]===hash&&sources.hashes?.[key]===hash,`Actual source hash mismatch:${key}`);
  requireValue(report.pairedReceiptSha256===pairReceiptSha256,'Changed paired deployment receipt');
  assertPair(pair,report.health,report);
  const connection=events.find(event=>event.direction==='lifecycle'&&event.type==='connection');
  if(connection)requireValue(connection.url===report.gatewayUrl&&connection.webOrigin===report.webOrigin,
    'Recorded transport Origin differs from its paired allowed origin');
  requireValue(report.freshLevelJourneyAccepted===false&&report.runtimeGameplayAccepted===false&&report.nativeVisualAccepted===false,'Bounded/prepared/static evidence cannot claim whole gameplay, fresh levels or visuals');
  requireValue(report.startingState?.preparedLevel===true||report.startingState?.preparedLevel===false,'Starting-state provenance absent');
  requireValue(report.timeoutMs<=120*60_000&&report.spawnWaitMs<=150*60_000&&report.declaredNaturalKillLimit<=3,'Frozen route/sample deadline widened');
  const indexed=indexTrace(events);
  const initial=snapshot(indexed,report.initialSnapshotSequence);
  requireValue(own(initial)?.class===report.className&&own(initial)?.level===report.startingState.level,'Starting class/level was not actually observed');
  requireValue(events.some(event=>event.direction==='received'&&event.packet==='LoginSuccess'&&event.sequence<report.initialSnapshotSequence)
    &&events.some(event=>event.direction==='received'&&event.packet==='StartGame'&&event.payload?.result===4&&event.sequence<report.initialSnapshotSequence), 'No authenticated LoginSuccess/accepted StartGame bootstrap');
  const hops=(report.routes??[]).map(hop=>verifyHop(hop,indexed,sources,events));
  let previousMap=initial.mapFileName, previousSequence=report.initialSnapshotSequence;
  for(const hop of report.routes??[]) {
    requireValue(hop.from===previousMap&&hop.sourceSnapshotSequence>previousSequence,'Disjoint/reordered hops cannot be joined into one player journey');
    const actor=own(snapshot(indexed,hop.sourceSnapshotSequence,hop.from));
    requireValue(actor?.name===own(initial)?.name&&actor?.class===report.className,'Hop belongs to another character');
    previousMap=hop.to;previousSequence=hop.destinationSnapshotSequence;
  }
  if(report.lane==='main-round-trips') {
    const allowedTransition=event=>(report.routes??[]).some(hop=>event.sequence>hop.beforeSequence&&event.sequence<=hop.destinationSnapshotSequence&&transitionName(event)===hop.to)
      ||(report.save&&event.sequence>report.save.startSequence&&event.sequence<=report.save.reloginSnapshotSequence&&transitionName(event)===previousMap);
    requireValue(!events.some(event=>event.direction==='received'&&event.sequence>report.initialSnapshotSequence
      &&['MapChanged','MapInformation'].includes(event.packet)&&!allowedTransition(event)), 'Uncredited/intermittent teleport erased between walking hops');
  }
  const identities=new Set(hops.map(hop=>`${hop.routeId}:${hop.index}`));requireValue(identities.size===hops.length,'Duplicate walking hop credited');
  const routeTransitionsAccepted=Object.entries(ROUTES).every(([id,chain])=>chain.slice(0,-1).every((_,index)=>identities.has(`${id}:${index}`)))
    &&hops.length===EXPECTED_DIRECTED_HOPS;
  const sourcePositionsAccepted=routeTransitionsAccepted&&hops.every(hop=>hop.sourcePositionAccepted);
  const npcs=(report.npcActions??[]).map(action=>verifyNpc(action,indexed,sources));
  const saveAccepted=verifySave(report.save,indexed);
  if(report.status==='observed')requireValue(saveAccepted,'Observed run omitted normal logout/relogin/save receipt');
  const attempts=verifyAttempts(report,indexed,events,sources);const acquisition=verifyAcquisition(report,indexed,events,sources);
  const originalDeadline=Date.parse(report.startedAt)+report.timeoutMs;
  const counted=[...(report.routes??[]).map(hop=>hop.transitionSequence),...(report.bossAttempts??[]).map(attempt=>attempt.deathSequence).filter(Boolean)];
  requireValue(counted.every(sequence=>Date.parse(indexed.get(sequence)?.at)<=originalDeadline),'Counted route/kill happened after original deadline');
  const observedDeaths=events.filter(event=>event.direction==='received'&&event.type==='worldSnapshot'&&(own(event.payload)?.dead===true||event.payload?.playerHp===0));
  requireValue(!observedDeaths.length||(report.deaths??[]).length>0,'Authoritative death erased from retry ledger');
  for(const defect of report.preservedDefects??[]) {
    requireValue(defect.entryAccepted!==true&&defect.walkingExitAccepted!==true,'Preserved source defect claimed as ordinary reachability');
    if(defect.id==='D10061-UNBOUND-NEEDMOVE'&&defect.status==='observedNoEntry') {
      const source=snapshot(indexed,defect.sourceSnapshotSequence,PRESERVED_DEFECT.from);
      requireValue(own(source)?.x===178&&own(source)?.y===53&&!source.mapTransfers?.some(transfer=>transfer.toMapFileName===PRESERVED_DEFECT.to),'Unbound entry defect was not actually reached/observed');
    }
  }
  return {schema:'mir2.classic-late-route-verdict.v1',evidenceValidationPassed:true,className:report.className,
    preparedLevel:report.startingState.preparedLevel,freshLevelJourneyAccepted:false,routeTransitionsAccepted,sourcePositionsAccepted,
    normalRouteAccepted:routeTransitionsAccepted&&sourcePositionsAccepted&&saveAccepted&&report.status==='observed'&&!observedDeaths.length,
    directedHopsObserved:hops.length,plannedDirectedHops:EXPECTED_DIRECTED_HOPS,saveAccepted,rawDurableStoreBytesVerified:false,
    liveNpcs:npcs,bossAttempts:attempts,...acquisition,legacyQuestJourneyAccepted:false,ordinarySkillActivationAccepted:false,
    completeClassicWorld:false,runtimeGameplayAccepted:false,nativeVisualAccepted:false,
    remaining:['Full authenticated route cohort not implied by fixtures','D10061 supplied-source NeedMove entry remains unbound',
      'D710A-D713A have no authored outer walking exit','Direct committed source coordinate missing if only transition-implied',
      'Exact natural spawn/source-kill linkage missing when omitted by public snapshots','Boss non-drops and failed attempts remain outcomes',
      'Raw durable save bytes, ordinary acquired-book activation and native visuals are separate gates']};
}
export function verifyClassicCohort(verdicts,reports) {
  requireValue(verdicts.length===3&&CLASSES.every(className=>verdicts.some(verdict=>verdict.className===className)),'Three independent classes required');
  requireValue(new Set(reports.map(report=>report.accountFingerprint)).size===3,'Three distinct authenticated accounts required');
  requireValue(reports.every(report=>report.pairedReceiptSha256===reports[0].pairedReceiptSha256),'Cohort uses different Gateway pairs');
  return {classes:CLASSES,normalRoutesAccepted:verdicts.every(verdict=>verdict.normalRouteAccepted),
    directedHopsObserved:verdicts.reduce((count,verdict)=>count+verdict.directedHopsObserved,0),plannedDirectedHops:228,
    completeClassicWorld:false,runtimeGameplayAccepted:false,nativeVisualAccepted:false};
}
export async function verifyClassicEvidenceDirectory(directory) {
  const root=path.resolve(directory),reportBytes=await fs.readFile(path.join(root,'report.json')),
    pairBytes=await fs.readFile(path.join(root,'paired-gateway-receipt.json'));
  const events=[];const lines=readline.createInterface({input:createReadStream(path.join(root,'trace.jsonl')),crlfDelay:Infinity});
  for await(const line of lines){if(line){requireValue(line.length<=16*1024*1024,'Overlarge protocol trace line');events.push(JSON.parse(line));requireValue(events.length<=1000000,'Protocol trace event budget exceeded');}}
  const sources=await loadClassicSources(),pairReceiptSha256=sha256(pairBytes);
  return {...verifyClassicEvidence({report:JSON.parse(reportBytes),events,pair:JSON.parse(pairBytes),sources,pairReceiptSha256}),
    reportSha256:sha256(reportBytes),pairedReceiptSha256:pairReceiptSha256,actualSourceHashes:{...sources.hashes},traceEvents:events.length};
}

if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)) {
  const {values}=parseArgs({options:{input:{type:'string'},output:{type:'string'}}});
  if(!values.input)throw new Error('--input evidence directory required');
  const verdict=await verifyClassicEvidenceDirectory(values.input);
  if(values.output)await fs.writeFile(values.output,JSON.stringify(verdict,null,2),{flag:'wx'});
  console.log(JSON.stringify(verdict,null,2));
}
