import {spawnSync} from 'node:child_process';
import {openSync,closeSync,mkdirSync,writeFileSync,readFileSync,readdirSync,copyFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
const android=resolve('mir2-web3/apps/game-client/platform-android');
const client=resolve('mir2-web3/apps/game-client');
const name=process.argv[2];
const folder=join(android,'target/quest-ingress-20261002',process.argv[3] ?? 'candidate-gates');
mkdirSync(folder,{recursive:true});
const env={...process.env,
 CARGO_TARGET_DIR:join(android,'target/shared-sync-build-cache'),
 JAVA_HOME:'/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home',
 ANDROID_SDK_ROOT:'/Users/henryliu/Library/Android/sdk',
 ANDROID_HOME:'/Users/henryliu/Library/Android/sdk',
 ANDROID_NDK_HOME:'/Users/henryliu/Library/Android/sdk/ndk/26.1.10909125',
 MIR2_ANDROID_UI_ASSET_ROOT:join(android,'target/weight-assets-KtITw2/shared-ui-assets'),
 MIR2_ANDROID_WORLD_ASSET_ROOT:join(android,'target/shared-sync-build-cache/local-world-20260911-objects'),
 MIR2_ANDROID_ENTITY_ASSET_ROOT:join(android,'target/shared-sync-build-cache/entity-proof-assets-with-archer-bow-20260912'),
 MIR2_ANDROID_ENTITY_ASSET_PACK_ID:'android-archer-mount-bow-proof-20260912'};
const serial=['--','--test-threads=1'];
const rust=(project,features=[])=>['+1.95.0','test','--manifest-path',join(client,project,'Cargo.toml'),
 ...(project==='platform-windows'?['--bin','mir2-platform-windows']:['--lib']),'--locked','--offline',...features];
const gates={
 'focused-quest': ['cargo',[...rust('platform-android'),'quest_',...serial]],
 'android-normal': ['cargo',[...rust('platform-android'),...serial]],
 'android-preview': ['cargo',[...rust('platform-android',['--features','ui-preview']),...serial]],
 'shared-bevy': ['cargo',[...rust('client-bevy',['--features','native-player-ui']),...serial]],
 'shared-runtime': ['cargo',[...rust('runtime'),...serial]],
 'windows-quest': ['cargo',[...rust('platform-windows'),'quest',...serial]],
 'windows-tooltip': ['cargo',[...rust('platform-windows'),'tooltip',...serial]],
 'windows-completed': ['cargo',[...rust('platform-windows'),'completed',...serial]],
 'windows-bridge': ['cargo',[...rust('platform-windows'),'gameplay_bridge::tests::',...serial]],
 'windows-projection': ['cargo',[...rust('platform-windows'),'gameplay_bridge::tests::quest_',...serial]],
 'windows-gateway-quest': ['cargo',[...rust('platform-windows'),'gateway::tests::quest_',...serial]],
 'windows-exact-ack': ['cargo',[...rust('platform-windows'),'gameplay_bridge::tests::world_snapshot_decodes_exact_rejected_quest_operation_ack',...serial]],
 'java': ['./gradlew',['--no-daemon','--offline','--rerun-tasks','testDebugUnitTest','testUiPreviewUnitTest'],{cwd:join(android,'android')}],
 'api31-debug':['bash',[join(android,'build-android.sh')],{env:{...env,MIR2_ANDROID_MODE:'check',MIR2_ANDROID_VARIANT:'debug'}}],
 'api31-preview':['bash',[join(android,'build-android.sh')],{env:{...env,MIR2_ANDROID_MODE:'check',MIR2_ANDROID_VARIANT:'uiPreview'}}],
};
if(!gates[name])throw new Error('Unknown scoped gate '+name);
const sourceFiles=['client-bevy/src/lib.rs','client-bevy/src/native_quest_ingress.rs',
 'runtime/src/native_ingest.rs',
 'platform-windows/src/gameplay_bridge.rs','platform-windows/src/gateway.rs',
 'platform-android/src/lib.rs','platform-android/src/shared_shell.rs','platform-android/src/quest_ingress.rs',
 'platform-android/src/quest_host_tests.rs','platform-android/android/app/src/main/java/com/mir2/web3/GatewaySession.java',
 'platform-android/android/app/src/test/java/com/mir2/web3/GatewaySessionTest.java'];
const hashes=()=>Object.fromEntries(sourceFiles.map(path=>[path,createHash('sha256').update(readFileSync(join(client,path))).digest('hex')]));
const before=hashes();
const [command,args,options={}]=gates[name];
const started=new Date().toISOString();
const fd=openSync(join(folder,name+'.log'),'wx');
const result=spawnSync(command,args,{env,stdio:['ignore',fd,fd],timeout:3600000,...options});
closeSync(fd);
if(name==='java'){
 const reports=[];
 for(const variant of ['Debug','UiPreview']){
  const reportRoot=join(android,'android/app/build/test-results/test'+variant+'UnitTest');
  const dest=join(folder,'java-xml',variant);
  mkdirSync(dest,{recursive:true});
  for(const file of readdirSync(reportRoot).filter(file=>file.endsWith('.xml'))){
   const raw=readFileSync(join(reportRoot,file));
   copyFileSync(join(reportRoot,file),join(dest,file));
   const opening=raw.toString('utf8').match(/<testsuite\b[^>]*>/)?.[0]??'';
   const number=key=>Number(opening.match(new RegExp(`\\b${key}="(\\d+)"`))?.[1]);
   reports.push({variant,file,tests:number('tests'),failures:number('failures'),errors:number('errors'),
    skipped:number('skipped'),sha256:createHash('sha256').update(raw).digest('hex')});
  }
 }
 writeFileSync(join(folder,'java-results.json'),JSON.stringify(reports,null,2)+'\n',{flag:'wx'});
}
const after=hashes();
const record={name,command,args,started,finished:new Date().toISOString(),status:result.status,error:result.error?.message,
 sourceHashesBefore:before,sourceHashesAfter:after,sourceUnchanged:JSON.stringify(before)===JSON.stringify(after)};
writeFileSync(join(folder,name+'.json'),JSON.stringify(record,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({gate:name,status:result.status,sourceUnchanged:record.sourceUnchanged}));
process.exit(result.status===0&&record.sourceUnchanged?0:1);
