import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,openSync,closeSync} from 'node:fs';
import {execFileSync,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
const out='mir2-web3/apps/game-client/platform-android/target/quest-ingress-20261002/';
const root='mir2-web3/apps/game-client/';
const base='2c2e5e214693859d7d56e414fb3ff9233e58409c';
const files=['platform-windows/src/gameplay_bridge.rs','platform-windows/src/gateway.rs'].map(file=>root+file);
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const owned=new Map(files.map(file=>[file,readFileSync(file,'utf8')]));
const baseline=new Map(files.map(file=>[file,execFileSync('git',['show',`${base}:${file}`],{encoding:'utf8',maxBuffer:8*1024*1024})]));
const patch=(file,before,after)=>`*** Update File: ${file}\n@@\n`+
 before.trimEnd().split('\n').map(line=>'-'+line).join('\n')+'\n'+
 after.trimEnd().split('\n').map(line=>'+'+line).join('\n')+'\n';
const apply=entries=>execFileSync('apply_patch',[],{input:'*** Begin Patch\n'+
 entries.map(([file,before,after])=>patch(file,before,after)).join('')+'*** End Patch\n',maxBuffer:1024*1024});
let result;
try{
 apply(files.map(file=>[file,owned.get(file),baseline.get(file)]));
 const fd=openSync(out+'windows-quest-published-baseline.log','wx');
 result=spawnSync('cargo',['+1.95.0','test','--manifest-path',root+'platform-windows/Cargo.toml',
  '--bin','mir2-platform-windows','--locked','--offline','quest','--','--test-threads=1'],{
  env:{...process.env,CARGO_TARGET_DIR:resolve(root+'platform-android/target/shared-sync-build-cache')},
  stdio:['ignore',fd,fd],timeout:900000});
 closeSync(fd);
}finally{
 apply(files.map(file=>[file,readFileSync(file,'utf8'),owned.get(file)]));
 for(const file of files)assert.equal(readFileSync(file,'utf8'),owned.get(file));
}
const results=text=>{
 const match=text.match(/test result: FAILED\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/);
 return {counts:match?.slice(1).map(Number),failures:[...text.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(match=>match[1]).sort()};
};
const candidate=results(readFileSync(out+'candidate-gates-v2/windows-quest.log','utf8'));
const original=results(readFileSync(out+'windows-quest-published-baseline.log','utf8'));
const same=JSON.stringify(candidate)===JSON.stringify(original);
writeFileSync(out+'windows-baseline-binding.json',JSON.stringify({publishedBaseline:base,status:result.status,
 currentOwnedFilesRestored:true,scope:'Comparison of same Mac test filter, not a full Windows platform gate',
 files:files.map(file=>({file,baselineSha256:hash(baseline.get(file)),restoredOwnedSha256:hash(owned.get(file))})),
 candidate,publishedBaselineResults:original,identicalResultSet:same},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({baselineStatus:result.status,identicalResultSet:same,sourceRestored:true}));
assert.equal(same,true,'Baseline and candidate differences require investigation');
assert(original.counts,'Need actual compiled tests, not a build/setup failure');
