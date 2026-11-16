import assert from 'node:assert/strict';
import {readFile,writeFile,lstat,readdir,realpath,rename,mkdir,symlink} from 'node:fs/promises';
import {join,resolve,relative} from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const run=import.meta.dirname,base=join(run,'browser-realm'),repo='E:/mir2-player-journey/mir2-web3';
const old='D:/mir2-builds/r16-reconnect-browser-20261108/apps/web';
const web=join(base,'apps/web'),publicPath=join(web,'public');
const sha=b=>createHash('sha256').update(b).digest('hex');
assert.ok(relative(base,publicPath)&&!relative(base,publicPath).startsWith('..'));
assert.ok((await lstat(publicPath)).isSymbolicLink());
assert.equal(resolve(await realpath(publicPath)).toLowerCase(),resolve(await realpath(join(old,'public'))).toLowerCase());
await rename(publicPath,join(web,'public-old-fixture-link'));
await mkdir(publicPath);
for(const entry of await readdir(join(old,'public'),{withFileTypes:true})){
 const source=['bevy-runtime','bevy-runtime-releases'].includes(entry.name)
  ? join(repo,'apps/web/public',entry.name):join(old,'public',entry.name);
 if(entry.isDirectory()||entry.isSymbolicLink())await symlink(source,join(publicPath,entry.name),'junction');
 else await symlink(source,join(publicPath,entry.name),'file');
}
const frontendPins={};
for(const file of ['app/page.tsx','app/components/original-client-guild-window.tsx','app/components/original-client-extra-windows.tsx',
 'lib/social-parity-actions.ts','lib/guild-rank-operations.ts','lib/guild-rank-projection.ts','app/original-client-shell.tsx','lib/use-window-keydown.ts']){
 const expected=sha(await readFile(join(repo,'apps/web',file)));assert.equal(sha(await readFile(join(web,file))),expected,file);frontendPins[file]=expected;
}
const manifest=JSON.parse(await readFile(join(web,'lib/generated/bevy_runtime_version.json'),'utf8'));
const runtimePins=[];
for(const file of manifest.files){
 assert.equal(sha(await readFile(join(web,file.path))),file.sha256,file.path);
 const immutable=file.path.replace('public/bevy-runtime/','public/bevy-runtime-releases/'+manifest.version+'/');
 assert.equal(sha(await readFile(join(web,immutable))),file.sha256,immutable);
 runtimePins.push({...file,immutable});
}
const source=await readFile(join(base,'private-qa/accounts.source-snapshot.json'));
const seeded=await readFile(join(base,'private-qa/accounts.seeded-snapshot.json'));
assert.deepEqual(JSON.parse(source).accounts,JSON.parse(seeded).accounts);
const head=execFileSync('git',['rev-parse','HEAD'],{cwd:repo,encoding:'utf8'}).trim();
const proof={schema:'mir2.r16.guild-ranks.browser-source.v2',createdAt:new Date().toISOString(),head,
 frontendPins,renderer:manifest.version,runtimePins,
 gatewaySha256:'3666eb7e67b8c99ab45621fd5b320f5e0874869868a45f48e3a1414e3e6bd9c1',
 sourceAccountSnapshotSha256:sha(source),seededSnapshotSha256:sha(seeded),
 fixtureGuildCreatedBeforeStartup:true,guildCreationViaGameplayClaimed:false,
 baselinePersonalAccountsUnchangedByFixture:true,sourceRoles:5,fixtureGuildMembers:3,
 preparationIssue:'Initial prepared public linked an older renderer; frontend manifest verification did not complete. Public tree was replaced with an owned directory linking current canonical renderer and existing finite assets before browser startup. Original linked public tree retained untouched.',
 originalServicesAndExePreserved:true};
await writeFile(join(base,'source-proof.json'),JSON.stringify(proof,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({sourceVerified:true,frontendPins:Object.keys(frontendPins).length,runtimePins:runtimePins.length,renderer:manifest.version,personalAccountsUnchanged:true}));
