import assert from 'node:assert/strict';
import {execFileSync,spawnSync} from 'node:child_process';
import {openSync,closeSync,copyFileSync,mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {resolve,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
const qa=dirname(fileURLToPath(import.meta.url));
const android=resolve('mir2-web3/apps/game-client/platform-android');
const expected=process.argv[2];
assert.match(expected??'',/^[a-f0-9]{40}$/);
const git=args=>execFileSync('git',args,{encoding:'utf8'});
const env={...process.env,CARGO_TARGET_DIR:join(android,'target/shared-sync-build-cache'),
 JAVA_HOME:'/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home',
 ANDROID_SDK_ROOT:'/Users/henryliu/Library/Android/sdk',ANDROID_HOME:'/Users/henryliu/Library/Android/sdk',
 ANDROID_NDK_HOME:'/Users/henryliu/Library/Android/sdk/ndk/26.1.10909125',
 MIR2_ANDROID_UI_ASSET_ROOT:join(android,'target/weight-assets-KtITw2/shared-ui-assets'),
 MIR2_ANDROID_WORLD_ASSET_ROOT:join(android,'target/shared-sync-build-cache/local-world-20260911-objects'),
 MIR2_ANDROID_ENTITY_ASSET_ROOT:join(android,'target/shared-sync-build-cache/entity-proof-assets-with-archer-bow-20260912'),
 MIR2_ANDROID_ENTITY_ASSET_PACK_ID:'android-archer-mount-bow-proof-20260912',
 MIR2_GATEWAY_WS_URL:'',MIR2_ANDROID_MODE:'package',MIR2_ANDROID_RUST_PROFILE:'release'};
mkdirSync(join(qa,'final-apks'),{recursive:true});
for(const [name,variant] of [['debug','debug'],['preview','uiPreview']]){
 const before=git(['rev-parse','HEAD']),statusBefore=git(['status','--porcelain']);
 writeFileSync(join(qa,name+'-source-before.txt'),before);
 writeFileSync(join(qa,name+'-status-before.txt'),statusBefore);
 assert.equal(before.trim(),expected);assert.equal(statusBefore,'');
 writeFileSync(join(qa,name+'-build-start.txt'),new Date().toISOString()+'\n');
 const fd=openSync(join(qa,name+'-build.log'),'wx');
 const result=spawnSync('bash',[join(android,'build-android.sh')],{env:{...env,MIR2_ANDROID_VARIANT:variant},timeout:3600000,stdio:['ignore',fd,fd]});
 closeSync(fd);
 writeFileSync(join(qa,name+'-build-end.txt'),new Date().toISOString()+'\n');
 assert.equal(result.status,0,name+': '+(result.error?.message??'see original log'));
 const after=git(['rev-parse','HEAD']),statusAfter=git(['status','--porcelain']);
 writeFileSync(join(qa,name+'-source-after.txt'),after);writeFileSync(join(qa,name+'-status-after.txt'),statusAfter);
 assert.equal(after.trim(),expected);assert.equal(statusAfter,'');
 const apk=join(qa,'final-apks','mir2-native-quest-workspace-'+name+'-v23.apk');
 copyFileSync(join(android,'android/app/build/outputs/apk',variant,'app-'+variant+'.apk'),apk);
 const config=readFileSync(join(android,'android/app/build/generated/source/buildConfig',variant,'com/mir2/web3/BuildConfig.java'));
 assert.match(config.toString(),/MIR2_GATEWAY_URL = ""/);
 writeFileSync(join(qa,name+'-BuildConfig.java.txt'),config);
 writeFileSync(join(qa,name+'-badging.txt'),execFileSync('/Users/henryliu/Library/Android/sdk/build-tools/34.0.0/aapt',['dump','badging',apk]));
 console.log(JSON.stringify({variant:name,source:expected,apk}));
}
