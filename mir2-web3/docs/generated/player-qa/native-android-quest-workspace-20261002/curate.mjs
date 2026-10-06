import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,readdirSync,mkdirSync,copyFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {join} from 'node:path';
const qa='mir2-web3/docs/generated/player-qa/native-android-quest-workspace-20261002';
const input='mir2-web3/apps/game-client/platform-android/target/quest-workspace-v23-20261002';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const walk=(root,path='')=>readdirSync(join(root,path),{withFileTypes:true}).flatMap(entry=>{
 const child=join(path,entry.name);return entry.isDirectory()?walk(root,child):[child];
});
const roots=readdirSync(input,{withFileTypes:true}).filter(entry=>entry.isFile()).map(entry=>entry.name);
const selected=[...roots,...['final-source','final-source-2','device','touch-device','detail-device'].flatMap(path=>walk(input,path))];
assert(selected.every(file=>/\.(mjs|log|json|xml|txt|png)$/.test(file)),'No APK/cache/assets/secrets');
assert.equal(selected.filter(file=>file.endsWith('.png')).length,10);
const rows=selected.sort().map(original=>{
 const file=join('raw','v23',original),destination=join(qa,file);
 mkdirSync(join(destination,'..'),{recursive:true});copyFileSync(join(input,original),destination);
 const bytes=readFileSync(join(input,original));assert(bytes.equals(readFileSync(destination)),file);
 return{file,bytes:bytes.length,sha256:hash(bytes)};
});
writeFileSync(join(qa,'manifest.json'),JSON.stringify({source:'ef5e56d1c8e51abd1fad3ee79be9a4ce1aa604e4',
 rawArtifacts:rows.length,originalUncroppedPng:10,rawBytesUnmodified:true,files:rows},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({rawArtifacts:rows.length,originalPng:10,allOriginalBytesMatch:true}));
