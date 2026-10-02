import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,readdirSync,mkdirSync,copyFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {join} from 'node:path';
const qa='mir2-web3/docs/generated/player-qa/native-android-npc-exit-20261002';
const input='mir2-web3/apps/game-client/platform-android/target/quest-exit-v21-20261002';
const source='545a34abb34415e61a6dd2f274f48914f0d0b698';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const walk=(root,path='')=>readdirSync(join(root,path),{withFileTypes:true}).flatMap(entry=>{
 const child=join(path,entry.name);
 return entry.isDirectory()?walk(root,child):[child];
});
const roots=readdirSync(input,{withFileTypes:true}).filter(entry=>entry.isFile()).map(entry=>entry.name);
const selected=[...roots,...['final-source','device','touch-device','menu-device'].flatMap(p=>walk(input,p))].sort();
assert(selected.every(file=>/\.(mjs|log|json|xml|txt|png)$/.test(file)),'No APK/assets/cache curation');
assert.equal(selected.filter(file=>file.endsWith('.png')).length,7);
const rows=selected.map(file=>{
 const original=join(input,file),destination=join(qa,'raw','v21',file);
 mkdirSync(join(destination,'..'),{recursive:true});
 copyFileSync(original,destination);
 const bytes=readFileSync(original);
 assert(bytes.equals(readFileSync(destination)),file);
 return {file:join('raw','v21',file),bytes:bytes.length,sha256:hash(bytes)};
});
writeFileSync(join(qa,'manifest.json'),JSON.stringify({source,rawArtifacts:rows.length,
 originalUncroppedPng:7,rawBytesUnmodified:true,files:rows},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({rawArtifacts:rows.length,originalPng:7,allOriginalBytesMatch:true}));
