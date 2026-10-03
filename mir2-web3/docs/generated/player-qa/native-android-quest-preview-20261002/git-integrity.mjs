import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const root='mir2-web3/docs/generated/player-qa/native-android-quest-preview-20261002/';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const manifest=JSON.parse(readFileSync(root+'manifest.json'));
const revision=process.argv[2];
assert(revision==='--cached'||revision==='HEAD'||/^[a-f0-9]{40}$/.test(revision??''));
for(const entry of manifest.files){
 const file=root+entry.file,working=readFileSync(file);
 assert.equal(working.length,entry.bytes,file);assert.equal(hash(working),entry.sha256,file);
 const object=revision==='--cached'?':'+file:revision+':'+file;
 const blob=execFileSync('git',['show',object],{maxBuffer:12*1024*1024});
 assert.equal(hash(blob),entry.sha256,'git '+file);
}
const source=JSON.parse(readFileSync(root+'raw/v20/source-v20.json'));
assert.equal(source.source,manifest.source);
for(const entry of source.sourceFiles){
 const blob=execFileSync('git',['show',source.source+':'+entry.path],{maxBuffer:8*1024*1024});
 assert.equal(hash(blob),entry.sha256,entry.path);
}
console.log(JSON.stringify({rawArtifacts:manifest.files.length,workingAndGitBytesIdentical:true,
 committedInputs:source.sourceFiles.length,source:source.source,goal:'Active'}));
