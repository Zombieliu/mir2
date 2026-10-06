import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,readdirSync,mkdirSync,copyFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {join} from 'node:path';
const qa='mir2-web3/docs/generated/player-qa/native-android-quest-focus-20261002';
const input='mir2-web3/apps/game-client/platform-android/target/quest-focus-v22-20261002';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const walk=(root,path='')=>readdirSync(join(root,path),{withFileTypes:true}).flatMap(entry=>{
 const child=join(path,entry.name);
 return entry.isDirectory()?walk(root,child):[child];
});
const roots=readdirSync(input,{withFileTypes:true}).filter(entry=>entry.isFile()).map(entry=>entry.name);
const selected=[...roots,...['final-source','device','touch-device'].flatMap(path=>walk(input,path))];
const files=selected.map(file=>[join(input,file),join('raw','v22',file)]).sort((a,b)=>a[1].localeCompare(b[1]));
assert(selected.every(file=>/\.(mjs|log|json|xml|txt|png)$/.test(file)),'Do not curate APKs/assets/caches');
assert.equal(selected.filter(file=>file.endsWith('.png')).length,7);
const rows=files.map(([original,file])=>{
 const destination=join(qa,file);
 mkdirSync(join(destination,'..'),{recursive:true});
 copyFileSync(original,destination);
 const source=readFileSync(original),copy=readFileSync(destination);
 assert(source.equals(copy),file);
 return {file,bytes:source.length,sha256:hash(source)};
});
writeFileSync(join(qa,'manifest.json'),JSON.stringify({source:'afea4b358680bb1d1a2ec8362ca6ffb538c6d648',
 rawArtifacts:rows.length,originalUncroppedPng:7,rawBytesUnmodified:true,files:rows},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({rawArtifacts:rows.length,originalPng:7,allOriginalBytesMatch:true}));
