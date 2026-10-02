import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdtempSync} from 'node:fs';
import {join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
const qa=dirname(fileURLToPath(import.meta.url));
const proof=JSON.parse(readFileSync(join(qa,'package-baseline.json')));
const source=proof.source,frozen='3d735745f1117d42a7859e87604a106351dca935';
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const git=(rev,path)=>execFileSync('git',['show',rev+':'+path],{maxBuffer:8*1024*1024});
assert.equal(execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8'}).trim(),source);
assert.equal(execFileSync('git',['status','--porcelain'],{encoding:'utf8'}),'');
execFileSync('git',['merge-base','--is-ancestor',frozen,source]);
for(const f of proof.sourceFiles){assert.equal(sha(git(source,f.path)),f.sha256);assert.equal(sha(readFileSync(f.path)),f.sha256);}
for(const file of ['mir2-web3/third_party/bevy_render/src/view/window/mod.rs','mir2-web3/third_party/bevy_render/src/renderer/mod.rs'])assert(readFileSync(file).equals(git('277ab0a6566b5241c418844e5366865547cd03e0',file)));
const gates=JSON.parse(readFileSync(join(qa,'baseline-source-3/gate-commands.json')));
assert.equal(gates.length,6);
for(const g of gates){assert.equal(g.exitCode,0);assert.equal(g.sourceUnchanged,true);for(const f of proof.sourceFiles){assert.equal(g.sourceHashesBefore[f.path],f.sha256);assert.equal(g.sourceHashesAfter[f.path],f.sha256);}}
const spec=[['Items',1003],['StateItem',5192],['MagIcon',224],['MagIcon2',224],['UI_32bit',4]];
const uiRoot='mir2-web3/apps/game-client/platform-android/target/weight-assets-KtITw2/shared-ui-assets';
const artifacts=[];
for(const a of proof.artifacts){
 assert.equal(sha(readFileSync(a.path)),a.sha256);
 const variant=a.variant==='debug'?'debug':'uiPreview';
 const config=readFileSync(join(qa,a.variant+'-BuildConfig.java.txt'),'utf8');
 assert.match(config,/VERSION_CODE = 29/);assert.match(config,/VERSION_NAME = "0.1.26-phone-quest-modal"/);
 assert.match(config,/MIR2_GATEWAY_URL = ""/);assert.match(config,new RegExp('UI_PREVIEW = '+(a.variant==='preview')));
 const badging=readFileSync(join(qa,a.variant+'-badging.txt'),'utf8');
 for(const pattern of [/versionCode='29'/,/sdkVersion:'31'/,/targetSdkVersion:'35'/,/native-code: 'arm64-v8a'/])assert.match(badging,pattern);
 const entries=execFileSync('unzip',['-Z1',a.path],{encoding:'utf8',maxBuffer:16*1024*1024}).trim().split('\n');
 assert(entries.includes('lib/arm64-v8a/libmir2_platform_android.so'));
 const unpack=mkdtempSync(join(qa,a.variant+'-resource-audit-'));
 execFileSync('unzip',['-q',a.path,...spec.map(([name])=>'assets/original-ui/'+name+'/*'),'-d',unpack]);
 const libraries=spec.map(([name,count])=>{
  const images=entries.filter(e=>new RegExp('^assets/original-ui/'+name+'/[0-9]+\\.png$').test(e));assert.equal(images.length,count);
  for(const path of images){const bytes=readFileSync(join(unpack,path));assert(bytes.equals(readFileSync(join(uiRoot,path.slice('assets/'.length)))));if(name==='UI_32bit')assert(bytes.equals(git(frozen,'mir2-web3/apps/web/public/'+path.slice('assets/'.length))));}
  let metadataSha256=null;
  if(['Items','StateItem','UI_32bit'].includes(name)){const path='original-ui/'+name+'/meta.json',bytes=readFileSync(join(unpack,'assets',path));assert(bytes.equals(readFileSync(join(uiRoot,path))));metadataSha256=sha(bytes);}
  return{name,pngCount:images.length,allSelectedBytesMatch:true,metadataSha256};
 });
 assert.equal(libraries.reduce((n,l)=>n+l.pngCount,0),6647);artifacts.push({...a,libraries});
}
writeFileSync(join(qa,'source-and-package-baseline.json'),JSON.stringify({source,frozenWindows:frozen,sourceFiles:proof.sourceFiles,
 sixSourceGatesMatchCommitted41Inputs:true,temporaryRendererProbesRemoved:true,rendererFilesPublishedBaselineIdentical:true,
 artifacts,selectedResourceCountEach:6647,metadataCountEach:3,fullResourcePackAccepted:false,
 rendererFixAccepted:false,wholePhoneUiAccepted:false,realNetworkAccepted:false,physicalDeviceAccepted:false},null,2)+'\n',{flag:'wx'});
console.log(JSON.stringify({source,sourceInputs:proof.sourceFiles.length,variants:artifacts.length,selectedPngEach:6647,rendererFixed:false}));
