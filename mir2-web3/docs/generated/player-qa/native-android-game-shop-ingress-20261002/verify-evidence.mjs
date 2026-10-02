// Same raw-byte/Git verification pattern as the preceding v30 evidence leaf.
import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import {dirname,join,relative} from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const qa=dirname(fileURLToPath(import.meta.url)), mode=process.argv[2]??'working';
assert(['working','index','HEAD'].includes(mode));
const git=args=>execFileSync('git',args,{maxBuffer:32*1024*1024});
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const json=path=>JSON.parse(readFileSync(join(qa,path)));
const manifest=json('manifest.json'), diagnosis=json('diagnosis.json');
const pathInRepo=relative(process.cwd(),qa);
function bytes(path) {
 const disk=readFileSync(join(qa,path));
 if(mode==='working')return disk;
 const stored=git(['show',(mode==='index'?':':'HEAD:')+pathInRepo+'/'+path]);
 assert(stored.equals(disk),path+' Git/disk byte mismatch');return stored;
}
assert.equal(manifest.source,'92033b910cc9ddd996c416ec4024af4625e78b31');
assert.equal(diagnosis.source,manifest.source);
assert.equal(new Set(manifest.payloads.map(row=>row.path)).size,manifest.payloads.length);
for(const row of manifest.payloads) {
 assert(!row.path.includes('..'));
 assert(!/\.(apk|aab|jks|keystore|p12|pem)$/i.test(row.path));
 const original=bytes(row.path);
 assert.equal(original.length,row.bytes,row.path);assert.equal(hash(original),row.sha256,row.path);
}
const walk=dir=>readdirSync(dir,{withFileTypes:true}).flatMap(item=>{
 assert(!item.isSymbolicLink());const path=join(dir,item.name);
 return item.isDirectory()?walk(path):[relative(qa,path)];
});
assert.deepEqual([...walk(join(qa,'raw')),'diagnosis.json'].sort(),manifest.payloads.map(row=>row.path).sort());
git(['merge-base','--is-ancestor',diagnosis.frozenWindows,manifest.source]);
const source=json('raw/source-commit-binding.json');
assert.equal(source.sourceCommit,manifest.source);assert(source.clean);
const scope=git(['diff-tree','--no-commit-id','--name-only','-r',manifest.source]).toString().trim().split('\n');
assert.equal(scope.length,6);assert.deepEqual(scope.sort(),source.inputs.map(row=>row.path).sort());
for(const input of source.inputs)assert.equal(hash(git(['show',manifest.source+':'+input.path])),input.sha256);
const binding=json('raw/source-and-projection-bindings.json');
assert(binding.noWindowsChanges);assert.equal(binding.projections.length,6);
for(const row of binding.projections) {
 assert(row.sameTokens);assert.equal(row.normalizedFrozenSha256,row.normalizedSharedSha256);
}
for(const name of diagnosis.boundGates) {
 const gate=json('raw/'+name+'-result.json');
 assert.equal(gate.source,manifest.source);assert.equal(gate.code,0);assert.equal(gate.signal,null);
 assert(gate.sourceUnchanged);assert.deepEqual(gate.sourceBefore,source.inputs);
 assert.deepEqual(gate.sourceAfter,source.inputs);
}
assert.equal(diagnosis.boundGates.length,7);
for(const [name,count,ignored] of [
 ['bound-rust-normal',343,0],['bound-rust-preview',368,0],
 ['bound-shared-native-player',1257,10],['bound-runtime',292,1]]) {
 assert(readFileSync(join(qa,'raw/'+name+'.log'),'utf8').includes(
  'test result: ok. '+count+' passed; 0 failed; '+ignored+' ignored;'));
}
for(const variant of ['bound-java-debug','bound-java-preview']) {
 const files=readdirSync(join(qa,'raw',variant)).filter(name=>/^TEST-.*\.xml$/.test(name));
 assert.equal(files.length,5);let total=0;
 for(const file of files) {
  const start=readFileSync(join(qa,'raw',variant,file),'utf8').match(/<testsuite\b[^>]*>/)[0];
  const value=key=>Number(start.match(new RegExp(key+'="(\\d+)"'))[1]);
  total+=value('tests');assert.equal(value('failures')+value('errors')+value('skipped'),0);
 }
 assert.equal(total,44);
}
const red=readFileSync(join(qa,'raw/java-red-debug.xml'),'utf8');
assert.match(red,/<testsuite[^>]*tests="2"[^>]*failures="2"[^>]*errors="0"/);
assert.equal(hash(bytes('raw/java-red-test-source.java.txt')),json('raw/baseline.json').redTest.sha256);
assert.equal(json('raw/java-red-debug-result.json').code,1);
const prep=json('raw/android-api31-normal-result.json');assert.equal(prep.code,1);
assert(readFileSync(join(qa,'raw/android-api31-normal.log'),'utf8').includes('could not find `Cargo.toml`'));
const boundary=json('raw/final-source-boundary-audit.json');
assert(boundary.clean && boundary.legacyGameShopRulesSameBytes);
assert(boundary.preserved.every(row=>row.sameGitState));
assert(boundary.untouched.every(row=>row.workingSha256===row.parentSha256));
for(const renderer of json('raw/renderer-baseline-unchanged.json').files) {
 assert(renderer.sameBytes);
 assert(git(['show',manifest.source+':'+renderer.path]).equals(
  git(['show','277ab0a6566b5241c418844e5366865547cd03e0:'+renderer.path])));
}
for(const field of ['newApkBuilt','newScreenshots','liveAuthentication','liveJavaJni',
 'realPlayerLoop','physicalDeviceAccepted','zeroErrorRendererAccepted','wholeWindowsParityAccepted'])
 assert.equal(diagnosis[field],false);
assert.equal(diagnosis.goalStatus,'active');
console.log(JSON.stringify({mode,source:manifest.source,payloads:manifest.payloads.length,
 result:'PASS',newApkBuilt:false,wholeGoalComplete:false}));
