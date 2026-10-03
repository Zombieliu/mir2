import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdtempSync} from 'node:fs';
import {join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const qa=dirname(fileURLToPath(import.meta.url)),repo=process.cwd();
const binding=JSON.parse(readFileSync(join(qa,'source-commit-binding.json')));
const proof=JSON.parse(readFileSync(join(qa,'package-baseline.json')));
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const git=(rev,path)=>execFileSync('git',['show',rev+':'+path],{maxBuffer:8*1024*1024});
assert.equal(proof.source,binding.source);
assert.equal(execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),binding.source);
assert.equal(execFileSync('git',['status','--porcelain'],{encoding:'utf8'}),'');
execFileSync('git',['merge-base','--is-ancestor',binding.frozenWindows,binding.source]);
for(const f of binding.inputs){assert.equal(sha(git(binding.source,f.path)),f.sha256);assert.equal(sha(readFileSync(f.path)),f.sha256);}
for(const p of binding.protectedPathsUnchanged)assert(readFileSync(p).equals(git(binding.parent,p)));
const gates=["bound-rust-normal-v32","bound-rust-preview-v32","bound-shared-native-player-v32","bound-runtime-v32","bound-api31-normal-v32-corrected","bound-api31-preview-v32-corrected","bound-java-full-v32"];
for(const label of gates){const g=JSON.parse(readFileSync(join(qa,label+'-result.json')));assert.equal(g.source,binding.source);assert.equal(g.code,0,label);assert.equal(g.sourceUnchanged,true);assert.deepEqual(g.sourceBefore,binding.inputs);assert.deepEqual(g.sourceAfter,binding.inputs);}
const java=JSON.parse(readFileSync(join(qa,'java-results.json')));
assert.equal(java.length,2);
for(const v of java){assert.equal(v.tests,51);assert.equal(v.failures+v.errors+v.ignored,0);assert.equal(v.suites.length,6);}
const spec=[['Items',1003],['StateItem',5192],['MagIcon',224],['MagIcon2',224],['UI_32bit',4]];
const uiRoot='mir2-web3/apps/game-client/platform-android/target/weight-assets-KtITw2/shared-ui-assets';
assert.equal(proof.artifacts.length,2);
const artifacts=[];
for(const a of proof.artifacts){
 assert.equal(sha(readFileSync(a.path)),a.sha256);
 const variant=a.variant==='debug'?'debug':'uiPreview',capital=a.variant==='debug'?'Debug':'UiPreview';
 const config=readFileSync(join(qa,a.variant+'-BuildConfig.java.txt'),'utf8');
 assert.match(config,/VERSION_CODE = 32/);assert.match(config,/VERSION_NAME = "0.1.29-personal-jni-observer"/);
 assert.match(config,/MIR2_GATEWAY_URL = ""/);
 assert.match(config,new RegExp('UI_PREVIEW = '+(a.variant==='preview')));
 const badging=readFileSync(join(qa,a.variant+'-badging.txt'),'utf8');
 for(const p of [/versionCode='32'/,/sdkVersion:'31'/,/targetSdkVersion:'35'/,/native-code: 'arm64-v8a'/])assert.match(badging,p);
 const entries=execFileSync('unzip',['-Z1',a.path],{encoding:'utf8',maxBuffer:16*1024*1024}).trim().split('\n');
 const native=execFileSync('unzip',['-p',a.path,'lib/arm64-v8a/libmir2_platform_android.so'],{maxBuffer:256*1024*1024});
 assert.equal(native.subarray(0,4).toString('hex'),'7f454c46');
 assert.equal(native.readUInt16LE(18),183); // AArch64 ELF, not a web application.
 assert.equal(native.includes(Buffer.from('ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE')),a.variant==='preview');
 const expected='mir2-web3/apps/game-client/platform-android/android/app/build/intermediates/stripped_native_libs/'+variant+'/strip'+capital+'DebugSymbols/out/lib/arm64-v8a/libmir2_platform_android.so';
 assert(native.equals(readFileSync(expected)));
 const unpack=mkdtempSync(join(qa,a.variant+'-resource-audit-'));
 execFileSync('unzip',['-q',a.path,...spec.map(([name])=>'assets/original-ui/'+name+'/*'),'-d',unpack]);
 const libraries=spec.map(([name,count])=>{
  const images=entries.filter(e=>new RegExp('^assets/original-ui/'+name+'/[0-9]+\\.png$').test(e));assert.equal(images.length,count);
  for(const path of images){const bytes=readFileSync(join(unpack,path));assert(bytes.equals(readFileSync(join(uiRoot,path.slice('assets/'.length)))));
   if(name==='UI_32bit')assert(bytes.equals(git(binding.frozenWindows,'mir2-web3/apps/web/public/'+path.slice('assets/'.length))));}
  let metadataSha256=null;
  if(['Items','StateItem','UI_32bit'].includes(name)){const path='original-ui/'+name+'/meta.json',bytes=readFileSync(join(unpack,'assets',path));assert(bytes.equals(readFileSync(join(uiRoot,path))));metadataSha256=sha(bytes);}
  return{name,pngCount:images.length,allSelectedBytesMatch:true,metadataSha256};
 });
 artifacts.push({...a,nativeElfBytes:native.length,nativeElfSha256:sha(native),nativeElfMatchesThisVariantStrippedOutput:true,personalJniDiagnosticCompiled:a.variant==='preview',libraries});
}
const result={source:binding.source,frozenWindows:binding.frozenWindows,inputs:binding.inputs,
 gates,java,artifacts,selectedResourceCountEach:6647,metadataCountEach:3,
 gatewayUrl:'',uiPreviewOnlySyntheticInputs:true,fullResourcePackAccepted:false,
 rendererFixAccepted:false,wholePhoneUiAccepted:false,realAuthenticationAccepted:false,
 realNetworkAccepted:false,physicalDeviceAccepted:false};
writeFileSync(join(qa,'source-and-package-baseline.json'),JSON.stringify(result,null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({source:binding.source,inputs:binding.inputs.length,gates:gates.length,variants:artifacts.length,selectedPngEach:6647,actualJniAccepted:false}));
