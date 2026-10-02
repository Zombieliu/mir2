import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {readFileSync,writeFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const root=dirname(fileURLToPath(import.meta.url));
const current=JSON.parse(readFileSync(join(root,'package-baseline.json')));
const old=JSON.parse(readFileSync('mir2-web3/apps/game-client/platform-android/target/surface-probe-v25-20261002/package-baseline.json'));
assert.equal(current.versionCode,29);
const adb='/Users/henryliu/Library/Android/sdk/platform-tools/adb';
const records=[];
function run(name,args){
 const started=new Date().toISOString();
 const bytes=execFileSync(adb,['-s','emulator-5554',...args],{timeout:60000,maxBuffer:16*1024*1024});
 writeFileSync(join(root,name),bytes,{flag:'wx'});
 records.push({name,args,started,finished:new Date().toISOString(),exitCode:0});
 writeFileSync(join(root,'installation.json'),JSON.stringify({source:current.source,versionCode:29,oldVersionCode:28,dataPreserving:true,commands:records},null,2)+'\n');
 return bytes.toString();
}
run('install-devices.txt',['devices','-l']);
assert.equal(run('install-api.txt',['shell','getprop','ro.build.version.sdk']).trim(),'31');
for(const a of current.artifacts){
 const app=a.variant==='debug'?'com.mir2.web3':'com.mir2.web3.uipreview',previous=old.artifacts.find(b=>b.variant===a.variant);
 assert.equal(createHash('sha256').update(readFileSync(a.path)).digest('hex'),a.sha256);
 const previousPath=run('install-'+a.variant+'-path-before.txt',['shell','pm','path',app]).trim().replace(/^package:/,'');
 assert.equal(run('install-'+a.variant+'-sha-before.txt',['shell','sha256sum',previousPath]).trim().split(/\s+/)[0],previous.sha256);
 run('install-'+a.variant+'-stop.txt',['shell','am','force-stop',app]);
 assert.match(run('install-'+a.variant+'-result.txt',['install','-r','-t',a.path]),/Success/);
 const installedPath=run('install-'+a.variant+'-path-after.txt',['shell','pm','path',app]).trim().replace(/^package:/,'');
 assert.equal(run('install-'+a.variant+'-sha-after.txt',['shell','sha256sum',installedPath]).trim().split(/\s+/)[0],a.sha256);
 assert.match(run('install-'+a.variant+'-package.txt',['shell','dumpsys','package',app]),/versionCode=29\b/);
}
console.log(JSON.stringify({source:current.source,installedBothVariants:29,dataPreserving:true}));
