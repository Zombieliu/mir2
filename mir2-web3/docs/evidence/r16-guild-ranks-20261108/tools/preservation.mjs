import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {isDeepStrictEqual as same} from 'node:util';
const base=import.meta.dirname+'/browser-realm',repo='E:/mir2-player-journey/mir2-web3';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const bytes=await readFile(base+'/private-qa/accounts.qualified-snapshot.json');
const before=JSON.parse(bytes),after=JSON.parse(await readFile(base+'/private-data/accounts.json','utf8'));
const fixture=JSON.parse(await readFile(base+'/fixture-qualification.json','utf8'));assert.equal(sha(bytes),fixture.qualifiedSnapshotSha256);
const prior=JSON.parse(await readFile(repo+'/docs/evidence/web-r16-parity-goal-20261107/shortcut-verification.json','utf8'));
const fields=Object.keys(prior.preservation.protectedFields);assert.equal(fields.length,26);
const identity=JSON.parse(await readFile(base+'/private-qa/identity.local.json','utf8'));
const peers=JSON.parse(await readFile(base+'/private-qa/peers.local.json','utf8'));
const roleLabel=(accountId,index)=>accountId===identity.accountId?(index===identity.characterIndex?'ordinary-owner':'original-level30'):accountId===peers[0].accountId?'ordinary-officer':accountId===peers[1].accountId?'uninvolved-member':'unrelated';
assert.deepEqual(Object.keys(before.accounts).sort(),Object.keys(after.accounts).sort());
const roles=[],accountMetadata=[];let exactProtectedComparisons=0,emptyPurchaseJournalsInitialized=0;
for(const [accountId,account]of Object.entries(before.accounts)){
 const next=after.accounts[accountId];assert(next);
 const changed=Object.keys(account).filter(key=>!['characters','saves','character_last_access_binary_datetimes'].includes(key)&&!same(account[key],next[key]));assert.deepEqual(changed,[]);
 accountMetadata.push({retained:true,metadataEqual:true});
 assert.deepEqual(account.characters,next.characters);
 for(const role of account.characters){
  const old=account.saves[String(role.index)],current=next.saves[String(role.index)];assert(old&&current);
  const protectedFields=Object.fromEntries(fields.map(key=>[key,same(old[key],current[key])]));
  const different=fields.filter(key=>!protectedFields[key]);exactProtectedComparisons+=fields.length-different.length;
  const initialized=different.length===1&&different[0]==='npc_purchase_journal'&&old.npc_purchase_journal===undefined;
  if(initialized){
   const journal=current.npc_purchase_journal;assert(journal&&typeof journal==='object');
   assert.deepEqual(Object.keys(journal).sort(),['protocolVersion','accountId','characterIndex','characterName','actor','entries'].sort());
   assert.equal(journal.protocolVersion,1);assert.equal(journal.accountId,accountId);assert.equal(journal.characterIndex,role.index);assert.equal(journal.characterName,role.name);
   assert.deepEqual(journal.entries,[]);assert.equal(journal.actor.length,32);assert.ok(journal.actor.every(b=>Number.isInteger(b)&&b>=0&&b<=255)&&journal.actor.some(b=>b!==0));
   emptyPurchaseJournalsInitialized++;
  }else assert.deepEqual(different,[],'Any changed existing purchase receipt or growth/asset field must fail');
  assert.deepEqual(old.position,current.position);assert.equal(old.direction,current.direction);
  const stageOld=JSON.parse(old.stage5_systems_json??'{}'),stageNew=JSON.parse(current.stage5_systems_json??'{}');
  roles.push({role:roleLabel(accountId,role.index),level:role.level,metadataEqual:true,protectedFields,all26ProtectedExactlyEqual:different.length===0,emptyPurchaseJournalInitialized:initialized,allGrowthAndAssetFieldsEqual:fields.filter(key=>key!=='npc_purchase_journal').every(key=>protectedFields[key]),transformEqual:true,
   saveFieldCount:Object.keys(current).length,differingFields:[...new Set([...Object.keys(old),...Object.keys(current)])].filter(k=>!same(old[k],current[k])),
   stage5DifferingKeys:[...new Set([...Object.keys(stageOld),...Object.keys(stageNew)])].filter(k=>!same(stageOld[k],stageNew[k]))});
 }
}
assert.equal(roles.length,5);
const guildId=Object.keys(before.sharedGuilds)[0];assert.deepEqual(Object.keys(after.sharedGuilds),[guildId]);
const oldGuild=before.sharedGuilds[guildId],newGuild=after.sharedGuilds[guildId];
assert.deepEqual(oldGuild.members,newGuild.members);assert.deepEqual(oldGuild.storage,newGuild.storage);assert.equal(newGuild.gold,0);assert.equal(newGuild.experience,0);assert.equal(newGuild.level,1);
assert.equal(newGuild.ranks.length,3);assert.equal(newGuild.ranks[0].name,'Leader');assert.equal(newGuild.ranks[0].options,255);assert.equal(newGuild.ranks[1].name,'OfficerGPU');assert.equal(newGuild.ranks[1].options,1);assert.equal(newGuild.ranks[2].name,'TrailGPU');
const source=JSON.parse(await readFile(base+'/source-proof-final.json','utf8'));
for(const [file,pin]of Object.entries(source.frontendPins)) {assert.equal(sha(await readFile(repo+'/apps/web/'+file)),pin);assert.equal(sha(await readFile(base+'/apps/web/'+file)),pin);}
for(const file of source.runtimePins){assert.equal(sha(await readFile(base+'/apps/web/'+file.path)),file.sha256);assert.equal(sha(await readFile(base+'/apps/web/'+file.immutable)),file.sha256);}
const result={schema:'mir2.r16.guild-rank-preservation.v1',recordedAt:new Date().toISOString(),immutableBaseline:true,qualifiedSnapshotSha256:sha(bytes),
 baselineAccounts:Object.keys(before.accounts).length,afterAccounts:Object.keys(after.accounts).length,accountsAdded:0,accountMetadata,roles,
 protectedComparisons:roles.length*fields.length,exactProtectedComparisons,emptyPurchaseJournalsInitialized,
 allGrowthAndAssetFieldsEqual:true,existingPurchaseJournalsPreserved:true,newPurchaseReceipts:0,allTransformsEqual:true,
 guild:{membersUnchanged:true,storageUnchanged:true,goldUnchanged:true,experienceUnchanged:true,finalRanks:newGuild.ranks,
  differingFields:[...new Set([...Object.keys(oldGuild),...Object.keys(newGuild)])].filter(k=>!same(oldGuild[k],newGuild[k]))},
 frontendSourcePinsStable:true,runtimePinsStable:true,originalLiveFileBytesClaimedUnchanged:false};
await writeFile(base+'/preservation.json',JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({roles:roles.length,protectedComparisons:result.protectedComparisons,exactProtectedComparisons,emptyPurchaseJournalsInitialized,allGrowthAndAssetFieldsEqual:true,allTransformsEqual:true,membershipStorageAndGoldUnchanged:true}));
