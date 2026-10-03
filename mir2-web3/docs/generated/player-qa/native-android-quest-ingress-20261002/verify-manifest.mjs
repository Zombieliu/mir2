import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const root='mir2-web3/docs/generated/player-qa/native-android-quest-ingress-20261002/';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const manifest=JSON.parse(readFileSync(root+'manifest.json'));
const cached=process.argv.includes('--cached');
for(const entry of manifest.files){
 const bytes=readFileSync(root+entry.file);
 assert.equal(bytes.length,entry.bytes,entry.file);
 assert.equal(hash(bytes),entry.sha256,entry.file);
 if(cached){
  const blob=execFileSync('git',['show',':'+root+entry.file],{maxBuffer:8*1024*1024});
  assert.equal(hash(blob),entry.sha256,'index '+entry.file);
 }
}
const binding=JSON.parse(readFileSync(root+'raw/source-binding.json'));
for(const [file,sha] of Object.entries(binding.ownedSourceFiles)){
 const blob=execFileSync('git',['show',binding.sourceCommit+':mir2-web3/apps/game-client/'+file],{maxBuffer:4*1024*1024});
 assert.equal(hash(blob),sha,file);
}
console.log(JSON.stringify({rawArtifacts:manifest.files.length,workingBytesMatch:true,indexBytesMatch:cached,
 sourceCommit:binding.sourceCommit,ownedGitSourceFiles:Object.keys(binding.ownedSourceFiles).length,
 globalParity:false}));
