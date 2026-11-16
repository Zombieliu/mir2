import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {copyFile,mkdir,readFile,readdir,lstat,symlink,writeFile,constants} from 'node:fs/promises';
import {join,dirname} from 'node:path';
const run=import.meta.dirname,base='D:/mir2-builds/r16-reconnect-browser-20261108',old='D:/mir2-builds/r16-group-20261107',repo='E:/mir2-player-journey/mir2-web3';
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const candidate=JSON.parse(await readFile(run+'/candidate-build.json','utf8'));assert.equal(candidate.exitCode,0);
assert.equal(hash(await readFile(run+'/mir2-gateway-r16-reconnect.exe')),candidate.exeSha256);
await mkdir(base);
for(const dir of ['apps/web','private-data','private-qa','realm-cwd'])await mkdir(join(base,dir),{recursive:true,mode:0o700});
let count=0;
async function copy(from,to){const stat=await lstat(from);assert(!stat.isSymbolicLink());if(stat.isDirectory()){await mkdir(to,{recursive:true});for(const name of await readdir(from))await copy(join(from,name),join(to,name));}else{assert(stat.isFile());await mkdir(dirname(to),{recursive:true});await copyFile(from,to,constants.COPYFILE_EXCL);count++;}}
for(const dir of ['app','lib'])await copy(old+'/apps/web/'+dir,base+'/apps/web/'+dir);
for(const file of ['package.json','next.config.ts','proxy.ts','next-env.d.ts'])await copy(old+'/apps/web/'+file,base+'/apps/web/'+file);
for(const dir of ['node_modules','public'])await symlink(old+'/apps/web/'+dir,base+'/apps/web/'+dir,'junction');
for(const[from,to]of [[repo+'/packages',base+'/packages'],[repo+'/config',base+'/config'],[repo+'/apps/game-client',base+'/apps/game-client']])await symlink(from,to,'junction');
const config=JSON.parse(await readFile(old+'/apps/web/tsconfig.json','utf8'));config.compilerOptions.incremental=false;config.include=['next-env.d.ts','app/**/*.ts','app/**/*.tsx','lib/**/*.ts','lib/**/*.tsx'];await writeFile(base+'/apps/web/tsconfig.json',JSON.stringify(config,null,2)+'\n',{flag:'wx'});
const snapshot=await readFile(old+'/private-data/accounts.json');await writeFile(base+'/private-data/accounts.json',snapshot,{flag:'wx',mode:0o600});await writeFile(base+'/private-qa/accounts.source-snapshot.json',snapshot,{flag:'wx',mode:0o600});
for(const file of ['identity.local.json','peers.local.json','realm-state.local.json'])await copy(old+'/private-qa/'+file,base+'/private-qa/'+file);
const browserConfig=JSON.parse(await readFile(old+'/private-qa/browser-config.local.json','utf8'));browserConfig.origin='http://127.0.0.1:19352';browserConfig.gatewayUrl='ws://127.0.0.1:19252/ws';await writeFile(base+'/private-qa/browser-config.local.json',JSON.stringify(browserConfig),{flag:'wx',mode:0o600});
const candidatePath=run.replaceAll('\\','/')+'/mir2-gateway-r16-reconnect.exe';
let launchRealm=await readFile(old+'/launch-realm.mjs','utf8');launchRealm=launchRealm.replace("const executable='D:/mir2-builds/r16-character-constraints-20261107/mir2-gateway-r16-character-constraints.exe';","const executable="+JSON.stringify(candidatePath)+";").replace('3dd4f2b5c053f0d35c0ba172130a5731eb8944afd4b78e7605c03a9271fcad3e',candidate.exeSha256).replaceAll('19250','19252').replaceAll('19280','19282').replace('r16-group-proof','r16-reconnect-proof');assert(launchRealm.includes(candidatePath));await writeFile(base+'/launch-realm.mjs',launchRealm,{flag:'wx'});
let launchWeb=await readFile(old+'/launch-web.mjs','utf8');launchWeb=launchWeb.replaceAll('19250','19252').replaceAll('19351','19352').replaceAll('.next-group-proof','.next-reconnect-proof');await writeFile(base+'/launch-web.mjs',launchWeb,{flag:'wx'});
let browser=await readFile(old+'/browser.mjs','utf8');browser=browser.replace('mir2.r16.group.browser.v1','mir2.r16.reconnect.browser.v1');await writeFile(base+'/browser.mjs',browser,{flag:'wx'});
const files=['app/page.tsx','app/original-client-shell.tsx','app/components/original-client-group-window.tsx','lib/group-operations.ts','lib/stage5-window-adapters.ts','lib/use-window-keydown.ts'];const frontendPins={};
for(const file of files){const original=await readFile(repo+'/apps/web/'+file),copied=await readFile(base+'/apps/web/'+file);assert.equal(hash(copied),hash(original));frontendPins[file]=hash(original);}
const manifest=JSON.parse(await readFile(base+'/apps/web/lib/generated/bevy_runtime_version.json','utf8'));assert.equal(manifest.version,'bevy-db5e53a91f4d11de');for(const file of manifest.files)assert.equal(hash(await readFile(base+'/apps/web/'+file.path)),file.sha256);
await writeFile(base+'/source-proof.json',JSON.stringify({schema:'mir2.r16.reconnect.browser-source.v1',createdAt:new Date().toISOString(),gatewaySha256:candidate.exeSha256,frontendPins,renderer:manifest.version,sourceAccountSnapshotSha256:hash(snapshot),copiedFiles:count,privateSnapshotCopiedReadOnly:true,oldServicesAndExePreserved:true},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({prepared:true,copiedFiles:count,frontendMatched:true,gatewaySha256:candidate.exeSha256}));
