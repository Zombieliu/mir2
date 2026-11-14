import assert from 'node:assert/strict';
import {readFile,writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const repo='E:/mir2-player-journey/mir2-web3',run=import.meta.dirname,label=process.argv[2];assert(['before','after'].includes(label));
const git=(...args)=>execFileSync('git',args,{cwd:repo,encoding:'utf8'}).trim();
assert.equal(git('branch','--show-current'),'codex/windows-player-journey');
const files=[...new Set([...git('diff','--name-only','--relative','--','apps/game-client').split('\n'),...git('ls-files','--others','--exclude-standard','--','apps/game-client').split('\n')].filter(file=>file.endsWith('.rs')))];assert.equal(files.length,13);
const sha=b=>createHash('sha256').update(b).digest('hex'),pins=[],locks=[];
for(const path of files.sort()){const bytes=await readFile(repo+'/'+path);pins.push({path,bytes:bytes.length,sha256WindowsBytes:sha(bytes),gitBlob:git('hash-object','--',path)});}
for(const crate of ['client-bevy','platform-windows','runtime'])for(const name of ['Cargo.toml','Cargo.lock']){
 const path='apps/game-client/'+crate+'/'+name;assert.equal(git('hash-object','--',path),git('rev-parse','HEAD:mir2-web3/'+path));locks.push({path,sha256:sha(await readFile(repo+'/'+path))});
}
const referencePath='Crystal/Client/MirScenes/GameScene.cs';
const reference=await readFile('E:/mir2-player-journey/'+referencePath);
assert.equal(sha(reference),sha(await readFile('E:/mir2/'+referencePath)));
const result={schema:'mir2.r16.native-guild.source.v2',recordedAt:new Date().toISOString(),head:git('rev-parse','HEAD'),branch:git('branch','--show-current'),pins,locks,reference:{path:referencePath,sha256:sha(reference),bothWorktreesIdentical:true},directSourcePinsOnly:true};
if(label==='after'){const before=JSON.parse(await readFile(run+'/source-final-before.json','utf8'));assert.deepEqual(pins,before.pins);assert.deepEqual(locks,before.locks);assert.deepEqual(result.reference,before.reference);}
await writeFile(run+'/source-final-'+label+'.json',JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({sourcePins:pins.length,locksAndManifests:locks.length,canonicalUnchanged:true,label}));
