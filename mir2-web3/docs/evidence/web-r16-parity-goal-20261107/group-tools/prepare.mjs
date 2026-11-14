import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {copyFile, lstat, mkdir, readFile, readdir, symlink, writeFile, constants} from 'node:fs/promises';
import {dirname, join, relative} from 'node:path';
import {pathToFileURL} from 'node:url';
const base=import.meta.dirname,repo='E:/mir2-player-journey/mir2-web3',previous='D:/mir2-builds/r16-shortcut-20261107';
const web=join(base,'apps/web'),source=join(repo,'apps/web');
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const sha=async path=>hash(await readFile(path));
let count=0;
async function copy(from,to,skip=()=>false){
  if(skip(from))return;
  const st=await lstat(from);assert(!st.isSymbolicLink(),from);
  if(st.isDirectory()){await mkdir(to,{recursive:true});for(const name of await readdir(from))await copy(join(from,name),join(to,name),skip);}
  else{assert(st.isFile());await mkdir(dirname(to),{recursive:true});await copyFile(from,to,constants.COPYFILE_EXCL);count++;}
}
for(const dir of ['apps/web','private-data','private-qa','realm-cwd'])await mkdir(join(base,dir),{recursive:true,mode:0o700});
for(const name of ['app','lib'])await copy(join(source,name),join(web,name),p=>relative(source,p).replaceAll('\\','/')==='lib/generated/bevy_runtime_version.json');
for(const name of ['package.json','next.config.ts','proxy.ts','next-env.d.ts'])await copy(join(source,name),join(web,name));
await copy(join(previous,'apps/web/lib/generated/bevy_runtime_version.json'),join(web,'lib/generated/bevy_runtime_version.json'));
for(const name of ['node_modules','public'])await symlink(join(previous,'apps/web',name),join(web,name),'junction');
for(const [from,to] of [[join(repo,'packages'),join(base,'packages')],[join(repo,'config'),join(base,'config')],[join(repo,'apps/game-client'),join(base,'apps/game-client')]])await symlink(from,to,'junction');
const tsconfig=JSON.parse(await readFile(join(source,'tsconfig.json'),'utf8'));
tsconfig.compilerOptions.incremental=false;tsconfig.include=['next-env.d.ts','app/**/*.ts','app/**/*.tsx','lib/**/*.ts','lib/**/*.tsx'];tsconfig.exclude=['node_modules'];
await writeFile(join(web,'tsconfig.json'),JSON.stringify(tsconfig,null,2)+'\n',{flag:'wx'});
const snapshot=await readFile(join(previous,'private-data/accounts.json'));
await writeFile(join(base,'private-qa/accounts.source-snapshot.json'),snapshot,{flag:'wx',mode:0o600});
await writeFile(join(base,'private-data/accounts.json'),snapshot,{flag:'wx',mode:0o600});
const store=JSON.parse(snapshot),oldIdentity=JSON.parse(await readFile(join(previous,'private-qa/identity.local.json'),'utf8'));
const role=store.accounts[oldIdentity.accountId].characters.find(row=>row.index!==oldIdentity.characterIndex&&row.name.startsWith('Rs'));
assert(role&&role.level===1);
await writeFile(join(base,'private-qa/identity.local.json'),JSON.stringify({...oldIdentity,name:role.name,characterIndex:role.index}),{flag:'wx',mode:0o600});
const {newPlayableState}=await import(pathToFileURL(join(repo,'apps/web/scripts/r16-playable-service-config.mjs')));
const state=newPlayableState();await writeFile(join(base,'private-qa/realm-state.local.json'),JSON.stringify(state),{flag:'wx',mode:0o600});
const peers=Array.from({length:2},()=>newPlayableState().owner);
await writeFile(join(base,'private-qa/peers.local.json'),JSON.stringify(peers),{flag:'wx',mode:0o600});
await writeFile(join(base,'private-qa/browser-config.local.json'),JSON.stringify({origin:'http://127.0.0.1:19351',gatewayUrl:'ws://127.0.0.1:19250/ws',realmId:state.world.realmId}),{flag:'wx',mode:0o600});
const renderer=JSON.parse(await readFile(join(web,'lib/generated/bevy_runtime_version.json'),'utf8'));
assert.equal(renderer.version,'bevy-db5e53a91f4d11de');for(const file of renderer.files)assert.equal(await sha(join(web,file.path)),file.sha256);
const names=['app/page.tsx','app/original-client-shell.tsx','app/components/original-client-group-window.tsx','lib/group-operations.ts','lib/use-window-keydown.ts'];
const pins=Object.fromEntries(await Promise.all(names.map(async file=>{const sourceSha=await sha(join(source,file));assert.equal(await sha(join(web,file)),sourceSha);return[file,sourceSha];})));
await writeFile(join(base,'source-proof.json'),JSON.stringify({schema:'mir2.r16.group.source.v1',pins,renderer:renderer.version,rendererManifestSha256:await sha(join(web,'lib/generated/bevy_runtime_version.json')),sourceAccountSha256:hash(snapshot),copiedFiles:count,usesPreservedOrdinaryLevel1Role:true,createdAt:new Date().toISOString()},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({prepared:true,copiedFiles:count,renderer:renderer.version,usesPreservedOrdinaryLevel1Role:true}));
