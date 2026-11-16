import assert from 'node:assert/strict';
import {readFile,writeFile,copyFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const base=import.meta.dirname+'/browser-realm',repo='E:/mir2-player-journey/mir2-web3';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const previous=JSON.parse(await readFile(base+'/source-proof.json','utf8'));
const changed='app/components/original-client-guild-window.tsx';
assert.ok([previous.frontendPins[changed],sha(await readFile(repo+'/apps/web/'+changed))].includes(sha(await readFile(base+'/apps/web/'+changed))));
await copyFile(repo+'/apps/web/'+changed,base+'/apps/web/'+changed);
const frontendPins={};
for(const file of Object.keys(previous.frontendPins)){
 const expected=sha(await readFile(repo+'/apps/web/'+file));assert.equal(sha(await readFile(base+'/apps/web/'+file)),expected,file);frontendPins[file]=expected;
}
for(const file of previous.runtimePins){assert.equal(sha(await readFile(base+'/apps/web/'+file.path)),file.sha256);assert.equal(sha(await readFile(base+'/apps/web/'+file.immutable)),file.sha256);}
await writeFile(base+'/source-proof-final.json',JSON.stringify({...previous,createdAt:new Date().toISOString(),frontendPins,
 changesAfterCheckpoint:[{file:changed,reason:'Use complete one-pixel border values for default and selected states; avoid React shorthand/longhand removal conflict.',gitBlob:execFileSync('git',['hash-object','--','apps/web/'+changed],{cwd:repo,encoding:'utf8'}).trim()}],
 testFixture:'qualified-snapshot.json; see fixture-qualification.json'},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({sourceVerified:true,frontendPins:Object.keys(frontendPins).length,runtimePins:previous.runtimePins.length,renderer:previous.renderer}));
