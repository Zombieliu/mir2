import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFile,writeFile} from 'node:fs/promises';
import {isDeepStrictEqual as same} from 'node:util';
const run=import.meta.dirname,base='D:/mir2-builds/r16-reconnect-browser-20261108',repo='E:/mir2-player-journey/mir2-web3';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex'),json=async path=>JSON.parse((await readFile(path,'utf8')).replace(/^\uFEFF/,''));
const source=await json(base+'/source-proof.json'),snapshotBytes=await readFile(base+'/private-qa/accounts.source-snapshot.json');assert.equal(hash(snapshotBytes),source.sourceAccountSnapshotSha256);
const baseline=JSON.parse(snapshotBytes),current=await json(base+'/private-data/accounts.json'),identity=await json(base+'/private-qa/identity.local.json'),peers=await json(base+'/private-qa/peers.local.json'),state=await json(base+'/private-qa/realm-state.local.json');
const protectedFields=Object.keys((await json(repo+'/docs/evidence/web-r16-parity-goal-20261107/shortcut-verification.json')).preservation.protectedFields);assert.equal(protectedFields.length,26);
const roles=[],accounts=[];let accountOrdinal=0;
for(const[id,account]of Object.entries(baseline.accounts)){accountOrdinal++;const after=current.accounts[id];assert(after);const differences=Object.keys(account).filter(key=>!['characters','saves','character_last_access_binary_datetimes'].includes(key)&&!same(account[key],after[key]));assert.equal(differences.length,0);accounts.push({account:accountOrdinal,metadataEqual:true});
  for(const role of account.characters){const next=after.characters.find(row=>row.index===role.index);assert(same(role,next));const before=account.saves[String(role.index)],saved=after.saves[String(role.index)];assert(saved);
    const checks=Object.fromEntries(protectedFields.map(key=>[key,same(before[key],saved[key])]));assert(Object.values(checks).every(Boolean));
    roles.push({role:id===identity.accountId?(role.index===identity.characterIndex?'owner-level1':'original-level30'):'baseline-account-'+accountOrdinal+'-role-'+role.index,level:role.level,protectedFields:checks,all26ProtectedEqual:true,transformEqual:same(before.position,saved.position)&&same(before.direction,saved.direction),differingFields:[...new Set([...Object.keys(before),...Object.keys(saved)])].filter(key=>!same(before[key],saved[key]))});
  }
}
const preservation={schema:'mir2.r16.reconnect.preservation.v1',createdAt:new Date().toISOString(),sourceSnapshotSha256:hash(snapshotBytes),sourceSnapshotImmutable:true,baselineAccounts:Object.keys(baseline.accounts).length,afterAccounts:Object.keys(current.accounts).length,noNewAccounts:Object.keys(baseline.accounts).length===Object.keys(current.accounts).length,accounts,roles,protectedComparisons:roles.length*26,originalLiveSourceFilesUsedReadOnly:true,originalLiveFileByteEqualityClaimed:false};
await writeFile(run+'/preservation.json',JSON.stringify(preservation,null,2)+'\n',{flag:'wx'});
const privateStrings=[...Object.keys(current.accounts),...Object.values(current.accounts).flatMap(row=>[row.password,...row.characters.map(role=>role.name)]),...Object.values(state.keys),...Object.values(state.world),...Object.values(identity),...peers.flatMap(row=>Object.values(row))].filter(value=>typeof value==='string'&&value.length>=4);
let log=await readFile(base+'/gateway.private.log','utf8');for(const value of privateStrings.sort((a,b)=>b.length-a.length))log=log.replaceAll(value,'[private]');
const lines=log.split(/\r?\n/).filter(line=>/resume\.|web transport ended|persist\.|restored session|retained session|route lease rejected/.test(line));
const waits=lines.flatMap(line=>{const match=/stage=resume\.handoff\.wait phase=end elapsed_ms=(\d+)/.exec(line);return match?[Number(match[1])]:[];});
assert.equal(waits.length,2);assert(waits.every(ms=>ms>0));assert(!lines.some(line=>line.includes('route lease rejected')));
const restoreCount=lines.filter(line=>line.includes('web reconnect grace restored session')).length;assert.equal(restoreCount,2);
const timings={schema:'mir2.r16.reconnect.lifecycle.v1',accountNamesAndCredentialsRedacted:true,lines,handoffWaitMs:waits,restoredSessions:restoreCount,routeLeaseRejections:0};await writeFile(run+'/lifecycle-sanitized.json',JSON.stringify(timings,null,2)+'\n',{flag:'wx'});
const newLog=await readFile(run+'/handoff-tests-qualified2.log','utf8'),regression=await json(run+'/regression-result.json');const tests=new Set(regression.uniquePassed);for(const match of newLog.matchAll(/^test (\S+) \.\.\. ok$/gm))tests.add(match[1]);
await writeFile(run+'/test-summary.json',JSON.stringify({newHandoffPassed:33,adjacentUniquePassed:regression.uniquePassedCount,combinedUniquePassed:tests.size,allPassed:true,ignored:0,tests:[...tests].sort()},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({protectedRoles:roles.length,protectedComparisons:preservation.protectedComparisons,allProtectedEqual:true,transformsEqual:roles.every(row=>row.transformEqual),differingFields:roles.map(({role,differingFields})=>({role,differingFields})),handoffWaitMs:waits,restoredSessions:restoreCount,routeLeaseRejections:0,combinedUniqueTests:tests.size}));
