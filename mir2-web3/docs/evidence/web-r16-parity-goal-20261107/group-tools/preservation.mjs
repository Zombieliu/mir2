import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile,writeFile} from 'node:fs/promises';
import {isDeepStrictEqual as same} from 'node:util';
const base=import.meta.dirname,repo='E:/mir2-player-journey/mir2-web3';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const baselineBytes=await readFile(base+'/private-qa/accounts.source-snapshot.json'),baseline=JSON.parse(baselineBytes),current=JSON.parse(await readFile(base+'/private-data/accounts.json','utf8'));
const sourceProof=JSON.parse(await readFile(base+'/source-proof.json','utf8'));assert.equal(hash(baselineBytes),sourceProof.sourceAccountSha256);
const identity=JSON.parse(await readFile(base+'/private-qa/identity.local.json','utf8'));
const prior=JSON.parse(await readFile(repo+'/docs/evidence/web-r16-parity-goal-20261107/shortcut-verification.json','utf8'));
const protectedKeys=Object.keys(prior.preservation.protectedFields);assert.equal(protectedKeys.length,26);
const roles=[],accountFields=[];
for(const [accountId,account]of Object.entries(baseline.accounts)){
  const after=current.accounts[accountId];assert(after);
  const differences=Object.keys(account).filter(key=>!['characters','saves','character_last_access_binary_datetimes'].includes(key)&&!same(account[key],after[key]));
  accountFields.push({isTestOwner:accountId===identity.accountId,metadataEqual:differences.length===0,differingFieldNames:differences});
  for(const role of account.characters){
    const next=after.characters.find(row=>row.index===role.index);assert(next&&same(next,role),'Original character metadata changed');
    const before=account.saves[String(role.index)],save=after.saves[String(role.index)];assert(save);
    const protectedFields=Object.fromEntries(protectedKeys.map(key=>[key,same(before[key],save[key])]));
    assert(Object.values(protectedFields).every(Boolean),'Protected personal field changed');
    const differingFields=[...new Set([...Object.keys(before),...Object.keys(save)])].filter(key=>!same(before[key],save[key]));
    const stageBefore=JSON.parse(before.stage5_systems_json??'{}'),stageAfter=JSON.parse(save.stage5_systems_json??'{}');
    roles.push({role:accountId===identity.accountId?(role.index===identity.characterIndex?'ordinary-level1-test-role':'original-level30-role'):'unrelated-baseline-role',level:role.level,metadataEqual:true,protectedFields,all26ProtectedEqual:true,transformEqual:same(before.position,save.position)&&before.direction===save.direction,baselineTransform:{position:before.position,direction:before.direction},currentTransform:{position:save.position,direction:save.direction},saveFieldCount:Object.keys(save).length,differingFields,stage5DifferingKeys:[...new Set([...Object.keys(stageBefore),...Object.keys(stageAfter)])].filter(key=>!same(stageBefore[key],stageAfter[key]))});
  }
}
assert(accountFields.every(row=>row.metadataEqual));
const result={schema:'mir2.r16.group.preservation.v1',createdAt:new Date().toISOString(),immutableBaseline:true,sourceSnapshotSha256:hash(baselineBytes),allBaselineAccountsRetained:true,baselineAccountCount:Object.keys(baseline.accounts).length,afterAccountCount:Object.keys(current.accounts).length,accountsAddedByOrdinaryRegistration:Object.keys(current.accounts).length-Object.keys(baseline.accounts).length,accountFields,roles,originalLiveFileBytesClaimedUnchanged:false,originalThreeGatewayFilesUsedReadOnly:true};
await writeFile(base+'/preservation.json',JSON.stringify(result,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({protectedRoleCount:roles.length,all26ProtectedEqual:true,accountMetadataEqual:true,differences:roles.map(({role,differingFields,stage5DifferingKeys,transformEqual})=>({role,differingFields,stage5DifferingKeys,transformEqual}))}));
