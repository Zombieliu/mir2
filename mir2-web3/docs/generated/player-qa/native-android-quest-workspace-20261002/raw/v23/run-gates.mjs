import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {openSync,closeSync,writeFileSync,readFileSync,mkdirSync,readdirSync,copyFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const attempt=process.argv[2]??'final-source';
assert(['final-source','final-source-2'].includes(attempt));
const qa=join(dirname(fileURLToPath(import.meta.url)),attempt);
mkdirSync(qa,{recursive:true});
const client=resolve('mir2-web3/apps/game-client');
const android=join(client,'platform-android');
const previous=JSON.parse(readFileSync(join(android,'target/quest-focus-v22-20261002/source-v22.json')));
const files=[...previous.sourceFiles.map(file=>file.path), 'mir2-web3/apps/game-client/platform-android/src/world_input.rs'];
assert.equal(new Set(files).size,38);
const hashes=()=>Object.fromEntries(files.map(file=>[file,createHash('sha256').update(readFileSync(file)).digest('hex')]));
const env={...process.env,CARGO_TARGET_DIR:join(android,'target/shared-sync-build-cache'),
 JAVA_HOME:'/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home',
 ANDROID_SDK_ROOT:'/Users/henryliu/Library/Android/sdk',ANDROID_HOME:'/Users/henryliu/Library/Android/sdk',
 ANDROID_NDK_HOME:'/Users/henryliu/Library/Android/sdk/ndk/26.1.10909125',
 MIR2_ANDROID_UI_ASSET_ROOT:join(android,'target/weight-assets-KtITw2/shared-ui-assets'),
 MIR2_ANDROID_WORLD_ASSET_ROOT:join(android,'target/shared-sync-build-cache/local-world-20260911-objects'),
 MIR2_ANDROID_ENTITY_ASSET_ROOT:join(android,'target/shared-sync-build-cache/entity-proof-assets-with-archer-bow-20260912'),
 MIR2_ANDROID_ENTITY_ASSET_PACK_ID:'android-archer-mount-bow-proof-20260912',MIR2_GATEWAY_WS_URL:''};
const records=[];
function gate(name,command,args,options={}){
 const before=hashes(), started=new Date().toISOString();
 const fd=openSync(join(qa,name+'.log'),'wx');
 const run=spawnSync(command,args,{env,stdio:['ignore',fd,fd],timeout:3600000,...options});
 closeSync(fd);
 const after=hashes();
 const record={name,command,args,started,finished:new Date().toISOString(),exitCode:run.status,
  error:run.error?.message,sourceHashesBefore:before,sourceHashesAfter:after,sourceUnchanged:JSON.stringify(before)===JSON.stringify(after)};
 records.push(record);
 writeFileSync(join(qa,'gate-commands.json'),JSON.stringify(records,null,2)+'\n');
 console.log(JSON.stringify({name,exitCode:run.status,sourceUnchanged:record.sourceUnchanged}));
 assert.equal(run.status,0,name+' failed; retain original evidence');
 assert.equal(record.sourceUnchanged,true,name+' source changed');
 if(name==='java-fresh'){
  const reports=[];
  for(const variant of ['Debug','UiPreview']){
   const dir=join(android,'android/app/build/test-results','test'+variant+'UnitTest');
   const dst=join(qa,'java-xml',variant);
   mkdirSync(dst,{recursive:true});
   for(const file of readdirSync(dir).filter(file=>file.endsWith('.xml'))){
    const raw=readFileSync(join(dir,file)); copyFileSync(join(dir,file),join(dst,file));
    const tag=raw.toString().match(/<testsuite\b[^>]*>/)?.[0]??'';
    const number=key=>Number(tag.match(new RegExp(`\\b${key}="(\\d+)"`))?.[1]);
    reports.push({variant,file,tests:number('tests'),failures:number('failures'),errors:number('errors'),
      skipped:number('skipped'),sha256:createHash('sha256').update(raw).digest('hex')});
   }
  }
  writeFileSync(join(qa,'java-results.json'),JSON.stringify(reports,null,2)+'\n',{flag:'wx'});
  for(const variant of ['Debug','UiPreview']){
   const rows=reports.filter(row=>row.variant===variant); assert.equal(rows.length,5);
   assert.equal(rows.reduce((n,row)=>n+row.tests,0),41);
   for(const row of rows)assert.equal(row.failures+row.errors+row.skipped,0);
  }
 }
}
const rust=['+1.95.0','test','--manifest-path',join(android,'Cargo.toml'),'--lib','--locked','--offline'];
gate('android-normal-final','cargo',[...rust,'--','--test-threads=1']);
gate('android-preview-final','cargo',[...rust,'--features','ui-preview','--','--test-threads=1']);
gate('shared-bevy-final','cargo',['+1.95.0','test','--manifest-path',join(client,'client-bevy/Cargo.toml'),'--lib','--features','native-player-ui','--locked','--offline','--','--test-threads=1']);
gate('java-fresh','./gradlew',['--no-daemon','--offline','--rerun-tasks','testDebugUnitTest','testUiPreviewUnitTest'],{cwd:join(android,'android')});
for(const variant of ['debug','uiPreview'])gate('api31-'+variant,'bash',[join(android,'build-android.sh')],
 {env:{...env,MIR2_ANDROID_MODE:'check',MIR2_ANDROID_VARIANT:variant}});
