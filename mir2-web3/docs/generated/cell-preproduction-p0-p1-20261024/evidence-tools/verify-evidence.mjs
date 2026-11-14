import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
const root=path.resolve(process.argv[2]);
const read=async file=>JSON.parse(await fs.readFile(path.join(root,file),'utf8'));
let hashChecks=0;
const folders=['protocol-full-lifecycle','protocol-native-resume','protocol-death-revive-focused'];
for(const folder of folders){
  const manifest=await read(`${folder}/redaction-verification.json`);
  assert.equal(manifest.knownPrivateValueMatches,0,`${folder}: private value scan`);
  assert.equal(manifest.unredactedPrivateFieldMatches,0,`${folder}: sensitive field scan`);
  for(const pin of manifest.files){
    const file=path.resolve(root,folder,pin.file);
    assert.equal(path.dirname(file),path.resolve(root,folder),'manifest file escaped topic folder');
    const bytes=await fs.readFile(file);
    assert.equal(crypto.createHash('sha256').update(bytes).digest('hex'),pin.sha256.toLowerCase(),`${folder}/${pin.file}`);
    assert.equal(bytes.length,pin.bytes);
    hashChecks++;
  }
}
const status=entry=>typeof entry==='string'?entry:entry?.status;
const counts=report=>Object.values(report.checks).reduce((out,entry)=>{const key=status(entry);out[key]=(out[key]??0)+1;return out;},{});
const full=await read('protocol-full-lifecycle/1792844742353-ccf6fb6e.report.json');
assert.equal(full.ok,false);assert.equal(full.fullLifecyclePassed,false);
assert.deepEqual(counts(full),{passed:14,not_run:2});
const native=await read('protocol-native-resume/1792845120462-edfccab1.report.json');
assert.equal(native.ok,true);assert.deepEqual(counts(native),{passed:9});
const focused=await read('protocol-death-revive-focused/1792845858629-b969ae9c.report.json');
assert.equal(focused.ok,true);assert.deepEqual(counts(focused),{passed:7});
const currentSources=await read('review-source-pins.json');
const repositoryRoot=path.resolve(root,'../../../..');
for(const pin of currentSources.files){
  const bytes=await fs.readFile(path.join(repositoryRoot,pin.file));
  const rawSha=crypto.createHash('sha256').update(bytes).digest('hex');
  if(rawSha!==pin.sha256){
    const normalized=crypto.createHash('sha256').update(bytes.toString('utf8').replace(/\r\n/g,'\n')).digest('hex');
    assert.equal(normalized,pin.lfNormalizedSha256,pin.file);
  }
  hashChecks++;
}
let committedEvidenceFiles=0;
if(process.argv.includes('--git-blobs')){
  const relative=path.relative(repositoryRoot,root).replaceAll('\\','/');
  const listed=spawnSync('git',['-c','gc.auto=0','-c','maintenance.auto=false','ls-files','--',relative],{cwd:repositoryRoot,encoding:'utf8'});
  assert.equal(listed.status,0);
  for(const file of listed.stdout.trim().split('\n').filter(Boolean)){
    const blob=spawnSync('git',['-c','gc.auto=0','-c','maintenance.auto=false','cat-file','blob',`HEAD:${file}`],{cwd:repositoryRoot,maxBuffer:64*1024*1024});
    assert.equal(blob.status,0,file);
    assert.deepEqual(blob.stdout,await fs.readFile(path.join(repositoryRoot,file)),file);
    committedEvidenceFiles++;
  }
}
const result={verifiedAtUtc:new Date().toISOString(),hashChecks,committedEvidenceFiles,reportCounts:{originalFull:counts(full),native:counts(native),separateFocusedDeathRevive:counts(focused)},originalFullUnchangedAndIncomplete:true,redactionManifestsVerified:3,scope:'Independent hash/report-status verification. Source-only CRLF/LF variants use explicit normalized pins; recorded evidence bytes never normalize. Known-secret scans themselves were performed by the evidence worker; no private credentials are loaded by this verifier.'};
if(!process.argv.includes('--read-only')) await fs.writeFile(path.join(root,'evidence-verification.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result));
