import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,readdirSync,mkdirSync,copyFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {join} from 'node:path';
const input='mir2-web3/apps/game-client/platform-android/target/quest-ingress-20261002';
const output='mir2-web3/docs/generated/player-qa/native-android-quest-ingress-20261002';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const walk=path=>readdirSync(join(input,path),{withFileTypes:true}).flatMap(entry=>{
 const child=join(path,entry.name);
 return entry.isDirectory()?walk(child):[child];
});
const files=walk('').sort();
assert(files.every(file=>/\.(log|json|xml|mjs)$/.test(file)),'Unexpected artifact type: do not curate caches, APKs or resources');
const rows=files.map(file=>{
 const original=join(input,file),copy=join(output,'raw',file);
 mkdirSync(join(copy,'..'),{recursive:true});
 copyFileSync(original,copy);
 const bytes=readFileSync(original), copied=readFileSync(copy);
 assert.equal(sha(copied),sha(bytes),file);
 return {file:'raw/'+file,bytes:bytes.length,sha256:sha(bytes)};
});
writeFileSync(join(output,'manifest.json'),JSON.stringify({sourceCommit:'ccdd51a90542d866a3b08096469c2d2819656ce4',
 rawBytesUnmodified:true,rawArtifacts:rows.length,files:rows},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({rawArtifacts:rows.length,allByteIdentical:true}));
