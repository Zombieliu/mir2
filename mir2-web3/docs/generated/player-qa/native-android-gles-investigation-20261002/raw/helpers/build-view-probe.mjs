import assert from 'node:assert/strict';
import {execFileSync,spawnSync} from 'node:child_process';
import {openSync,closeSync,copyFileSync,mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {createHash} from 'node:crypto';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const qa=dirname(fileURLToPath(import.meta.url));
const android=resolve('mir2-web3/apps/game-client/platform-android');
const mode=process.argv[2],attempt=process.argv[3];
assert(['check','package'].includes(mode));assert.match(attempt??'',/^[a-z0-9-]+$/);
const expected=process.argv[4];
if(mode==='package')assert.match(expected??'',/^[a-f0-9]{40}$/);
const git=args=>execFileSync('git',args,{encoding:'utf8'});
const old=JSON.parse(readFileSync(join(android,'target/quest-reflow-v24-20261002/source-v24.json')));
const files=[...new Set([...old.sourceFiles.map(f=>f.path),'mir2-web3/third_party/bevy_render/src/view/window/mod.rs','mir2-web3/third_party/bevy_render/src/renderer/mod.rs'])];
const hashes=()=>Object.fromEntries(files.map(file=>[file,createHash('sha256').update(readFileSync(file)).digest('hex')]));
const env={...process.env,CARGO_TARGET_DIR:join(android,'target/shared-sync-build-cache'),
 JAVA_HOME:'/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home',
 ANDROID_SDK_ROOT:'/Users/henryliu/Library/Android/sdk',ANDROID_HOME:'/Users/henryliu/Library/Android/sdk',
 ANDROID_NDK_HOME:'/Users/henryliu/Library/Android/sdk/ndk/26.1.10909125',
 MIR2_ANDROID_UI_ASSET_ROOT:join(android,'target/weight-assets-KtITw2/shared-ui-assets'),
 MIR2_ANDROID_WORLD_ASSET_ROOT:join(android,'target/shared-sync-build-cache/local-world-20260911-objects'),
 MIR2_ANDROID_ENTITY_ASSET_ROOT:join(android,'target/shared-sync-build-cache/entity-proof-assets-with-archer-bow-20260912'),
 MIR2_ANDROID_ENTITY_ASSET_PACK_ID:'android-archer-mount-bow-proof-20260912',
 MIR2_GATEWAY_WS_URL:'',MIR2_ANDROID_MODE:mode,MIR2_ANDROID_RUST_PROFILE:'release',MIR2_ANDROID_VARIANT:'uiPreview'};
const head=git(['rev-parse','HEAD']).trim(),status=git(['status','--porcelain']),before=hashes();
if(mode==='package'){assert.equal(head,expected);assert.equal(status,'');}
const record={mode,attempt,source:head,statusBefore:status,inputHashesBefore:before,started:new Date().toISOString()};
const fd=openSync(join(qa,attempt+'.log'),'wx');
const result=spawnSync('bash',[join(android,'build-android.sh')],{env,timeout:3600000,stdio:['ignore',fd,fd]});closeSync(fd);
record.finished=new Date().toISOString();record.exitCode=result.status;record.error=result.error?.message;
record.inputHashesAfter=hashes();record.headAfter=git(['rev-parse','HEAD']).trim();record.statusAfter=git(['status','--porcelain']);
record.sourceUnchanged=JSON.stringify(before)===JSON.stringify(record.inputHashesAfter);
writeFileSync(join(qa,attempt+'.json'),JSON.stringify(record,null,2)+'\n');
assert.equal(result.status,0,'retain original '+attempt+' failure');assert.equal(record.sourceUnchanged,true);
if(mode==='package'){
 assert.equal(record.headAfter,expected);assert.equal(record.statusAfter,'');
 const out=join(qa,'apks');mkdirSync(out,{recursive:true});
 const apk=join(out,'mir2-native-view-probe-preview-v26.apk');
 copyFileSync(join(android,'android/app/build/outputs/apk/uiPreview/app-uiPreview.apk'),apk);
 const bytes=readFileSync(apk),sha256=createHash('sha256').update(bytes).digest('hex');
 const config=readFileSync(join(android,'android/app/build/generated/source/buildConfig/uiPreview/com/mir2/web3/BuildConfig.java'));
 assert.match(config.toString(),/MIR2_GATEWAY_URL = ""/);
 writeFileSync(join(qa,'view-probe-BuildConfig.java.txt'),config);
 writeFileSync(join(qa,'view-probe-badging.txt'),execFileSync('/Users/henryliu/Library/Android/sdk/build-tools/34.0.0/aapt',['dump','badging',apk]));
 const proof={source:expected,versionCode:26,versionName:'0.1.23-view-probe',gatewayUrl:'',
  diagnosticOnly:true,rendererFixAccepted:false,sourceFiles:files.map(path=>({path,sha256:before[path]})),
  artifacts:[{variant:'preview',source:expected,path:apk,bytes:bytes.length,sha256}]};
 writeFileSync(join(qa,'package-view-probe.json'),JSON.stringify(proof,null,2)+'\n');
 console.log(JSON.stringify({source:expected,apk,sha256,diagnosticOnly:true}));
}else console.log(JSON.stringify({mode,attempt,sourceUnchanged:true,inputs:files.length}));

