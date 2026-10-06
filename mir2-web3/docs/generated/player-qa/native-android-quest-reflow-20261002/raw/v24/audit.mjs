import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
const [mode,source]=process.argv.slice(2);
assert(['source','package'].includes(mode));assert.match(source??'',/^[a-f0-9]{40}$/);
const base=path.dirname(fileURLToPath(import.meta.url));
const frozen='3d735745f1117d42a7859e87604a106351dca935',parent='e1928b3239627685167a891e5588c26dbbdcf18e';
const uiRoot='mir2-web3/apps/game-client/platform-android/target/weight-assets-KtITw2/shared-ui-assets';
const read=name=>fs.readFileSync(path.join(base,name),'utf8');
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const git=(revision,file)=>execFileSync('git',['show',`${revision}:${file}`],{maxBuffer:8*1024*1024});
const output=(name,value)=>{fs.writeFileSync(path.join(base,name),JSON.stringify(value,null,2)+'\n',{flag:'wx'});console.log(JSON.stringify({mode,source,result:name}));};
execFileSync('git',['merge-base','--is-ancestor',frozen,source]);
if(mode==='source'){
 assert.match(read('quest-reflow-red.log'),/test result: FAILED\. 0 passed; 1 failed; 0 ignored/);
 assert.match(read('quest-reflow-final.log'),/test result: FAILED\. 5 passed; 2 failed; 0 ignored/);
 assert.match(read('quest-reflow-final-2.log'),/test result: ok\. 8 passed; 0 failed; 0 ignored/);
 const rust=[['android-normal-final.log',327],['android-preview-final.log',350]].map(([name,passed])=>{
  const match=read('final-source-2/'+name).match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/);
  assert(match,name);assert.deepEqual(match.slice(1).map(Number),[passed,0,0]);return{name,passed,failed:0,ignored:0};
 });
 assert.match(read('final-source-2/shared-bevy-final.log'),/test result: ok\. 1238 passed; 0 failed; 10 ignored/);
 assert.match(read('final-source-2/java-fresh.log'),/42 actionable tasks: 42 executed/);
 const java=JSON.parse(read('final-source-2/java-results.json'));
 for(const variant of ['Debug','UiPreview']){
  const rows=java.filter(row=>row.variant===variant);assert.equal(rows.length,5);
  assert.equal(rows.reduce((n,row)=>n+row.tests,0),41);
  for(const row of rows){assert.equal(row.failures+row.errors+row.skipped,0);assert.equal(hash(fs.readFileSync(path.join(base,'final-source-2/java-xml',variant,row.file))),row.sha256);}
 }
 for(const variant of ['debug','uiPreview'])assert.match(read('final-source-2/api31-'+variant+'.log'),/target check passed/);
 const commands=JSON.parse(read('final-source-2/gate-commands.json'));assert.equal(commands.length,6);
 const files=Object.keys(commands[0].sourceHashesAfter);assert.equal(files.length,39);
 const sourceFiles=files.map(file=>{const bytes=fs.readFileSync(file);assert(bytes.equals(git(source,file)),file);return{path:file,sha256:hash(bytes)};});
 for(const command of commands){
  assert.equal(command.exitCode,0);assert.equal(command.sourceUnchanged,true);
  for(const file of sourceFiles){assert.equal(command.sourceHashesBefore[file.path],file.sha256);assert.equal(command.sourceHashesAfter[file.path],file.sha256);}
 }
 const owned=['client-bevy/src/quest_ui.rs','client-bevy/src/quest_phone.rs','platform-android/android/app/build.gradle','platform-android/src/phone_quests.rs'].map(p=>'mir2-web3/apps/game-client/'+p).sort();
 const changed=execFileSync('git',['diff','--name-only',parent,source],{encoding:'utf8'}).trim().split('\n').sort();assert.deepEqual(changed,owned);
 output('source-v24.json',{source,parent,frozenWindows:frozen,rust,java,sourceFiles,owned,
  compiledReflowRegression:{red:1,finalFocused:8},
  retainedSetupErrors:['quest-reflow-green/green-2.log contain compiler errors, not product reds','quest-reflow-final.log 5 pass/2 fail exposed real per-frame child teardown breaking release and scroll; all original logs retained','final-source/android-preview-final.log: E0425 reporter missing explicit TextLayoutInfo import; original retained, only fresh final-source-2 six-gate results count'],
  sharedBevy:{passed:1238,failed:0,ignored:10,ignoredExisting:true},api31BothVariants:true,
  reviewScope:'Optional shared phone presentation plus Android workspace binding; default Windows geometry/controller retained; existing detail action eligibility extracted once for both layouts; primary review, no delegation',
  fullPhoneUiAccepted:false,realNetworkAccepted:false,physicalDeviceAccepted:false});
}else{
 const controls=JSON.parse(read('source-v24.json'));assert.equal(controls.source,source);
 for(const file of controls.sourceFiles)assert.equal(hash(git(source,file.path)),file.sha256);
 const artifacts=['debug','preview'].map(variant=>{
  for(const edge of ['before','after']){assert.equal(read(variant+'-source-'+edge+'.txt').trim(),source);assert.equal(read(variant+'-status-'+edge+'.txt'),'');}
  const config=read(variant+'-BuildConfig.java.txt'),badging=read(variant+'-badging.txt');
  assert.match(config,/VERSION_CODE = 24/);assert.match(config,/VERSION_NAME = "0.1.21-quest-reflow"/);
  assert.match(config,/MIR2_GATEWAY_URL = ""/);assert.match(config,new RegExp('UI_PREVIEW = '+(variant==='preview')));
  for(const pattern of [/versionCode='24'/,/sdkVersion:'31'/,/targetSdkVersion:'35'/,/native-code: 'arm64-v8a'/])assert.match(badging,pattern);
  assert.match(read(variant+'-build.log'),/package gate passed/);assert.match(read(variant+'-build.log'),/Finished `release` profile/);
  const apk=path.join(base,'final-apks','mir2-native-quest-reflow-'+variant+'-v24.apk');
  const entries=execFileSync('unzip',['-Z1',apk],{encoding:'utf8',maxBuffer:16*1024*1024}).trim().split('\n');
  assert(entries.includes('lib/arm64-v8a/libmir2_platform_android.so'));
  const librariesSpec=[['Items',1003],['StateItem',5192],['MagIcon',224],['MagIcon2',224],['UI_32bit',4]];
  const unpack=fs.mkdtempSync(path.join(base,variant+'-resource-proof-'));
  execFileSync('unzip',['-q',apk,...librariesSpec.map(([name])=>'assets/original-ui/'+name+'/*'),'-d',unpack]);
  const libraries=librariesSpec.map(([library,count])=>{
   const images=entries.filter(e=>new RegExp('^assets/original-ui/'+library+'/[0-9]+\\.png$').test(e));
   assert.equal(images.length,count);assert.equal(new Set(images).size,count);
   for(const entry of images){const bytes=fs.readFileSync(path.join(unpack,entry));assert(bytes.equals(fs.readFileSync(path.join(uiRoot,entry.slice('assets/'.length)))),entry);if(library==='UI_32bit')assert(bytes.equals(git(frozen,'mir2-web3/apps/web/public/'+entry.slice('assets/'.length))),entry);}
   let metadataSha256=null;
   if(['Items','StateItem','UI_32bit'].includes(library)){const p='original-ui/'+library+'/meta.json',bytes=fs.readFileSync(path.join(unpack,'assets',p));assert(bytes.equals(fs.readFileSync(path.join(uiRoot,p))),p);if(library==='UI_32bit')assert(bytes.equals(git(frozen,'mir2-web3/apps/web/public/'+p)));metadataSha256=hash(bytes);}
   return{library,pngCount:count,allSourceBytesMatch:true,metadataSha256};
  });
  assert.equal(libraries.reduce((n,l)=>n+l.pngCount,0),6647);
  const sha256=hash(fs.readFileSync(apk));assert(read(variant+'-build.log').includes(sha256));
  return{variant,source,path:path.resolve(apk),bytes:fs.statSync(apk).size,sha256,libraries};
 });
 output('package-v24.json',{source,frozenWindows:frozen,versionCode:24,versionName:'0.1.21-quest-reflow',
  cleanBeforeAfterBothBuilds:true,sourceGateHashesMatchCommittedInputs:true,gatewayUrl:'',minimumApi:31,targetApi:35,abi:'arm64-v8a',artifacts,
  each6647PngAndThreeMetadataMatchInputs:true,kind:'Native Bevy GameActivity; Rust release cdylib inside Gradle Debug diagnostic variants, not store Release',
  gpuRendererSourceChangedThisLeaf:false,olderFailuresRetained:true,fullResourceReleaseAccepted:false,fullPhoneUiAccepted:false,realNetworkAccepted:false,physicalDeviceAccepted:false});
}
