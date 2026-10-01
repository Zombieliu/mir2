// Exact APK inputs/contents. Unpacked licensed resources stay in ignored target.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const [qa, resourceRoot] = process.argv.slice(2);
assert(qa && resourceRoot, 'Provide exact QA target and approved resource root');
const source = '253b6682f73715b896745ca85a6923be98d0b892';
const frozen = '3f5e61533235921369bc13a7760b4a56b0e467e5';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const read = name => fs.readFileSync(path.join(qa,name),'utf8');
for (const file of JSON.parse(read('source-results.json')).sourceFiles) {
  assert.equal(hash(execFileSync('git',['show',`${source}:${file.path}`],{maxBuffer:4*1024*1024})),file.sha256,file.path);
}
const artifacts = ['debug','preview'].map(variant => {
  for (const edge of ['before','after']) {
    assert.equal(read(`${variant}-source-${edge}.txt`).trim(),source);
    assert.equal(read(`${variant}-status-${edge}.txt`),'');
  }
  const config = read(`${variant}-BuildConfig.java.txt`), badging = read(`${variant}-badging.txt`);
  assert.match(config,/VERSION_CODE = 14/);
  assert.match(config,/VERSION_NAME = "0.1.11-received-chat"/);
  assert.match(config,/MIR2_GATEWAY_URL = ""/);
  assert.match(config,new RegExp(`UI_PREVIEW = ${variant === 'preview'}`));
  assert.match(badging,/versionCode='14'/);
  assert.match(badging,/sdkVersion:'31'/);
  assert.match(badging,/targetSdkVersion:'35'/);
  assert.match(badging,/native-code: 'arm64-v8a'/);
  assert(read(`${variant}-build.log`).includes('package gate passed'));
  const apk = path.join(qa,'final-apks',`mir2-native-received-chat-${variant}-v14.apk`);
  const entries = execFileSync('unzip',['-Z1',apk],{encoding:'utf8',maxBuffer:16*1024*1024}).trim().split('\n');
  const libraries = [['Items',1003],['StateItem',5192],['MagIcon',224],['MagIcon2',224],['UI_32bit',4]];
  const unpack = fs.mkdtempSync(path.join(qa,`${variant}-resource-proof-`));
  execFileSync('unzip',['-q',apk,...libraries.map(([name])=>`assets/original-ui/${name}/*`),'-d',unpack]);
  let pngCount=0;
  const proof = libraries.map(([library,count]) => {
    const pngs = entries.filter(entry => new RegExp(`^assets/original-ui/${library}/[0-9]+\\.png$`).test(entry));
    assert.equal(pngs.length,count);
    assert.equal(new Set(pngs).size,count);
    for (const entry of pngs) {
      const bytes=fs.readFileSync(path.join(unpack,entry));
      assert(bytes.equals(fs.readFileSync(path.join(resourceRoot,entry.slice('assets/'.length)))),entry);
      if(library==='UI_32bit') assert(bytes.equals(execFileSync('git',['show',`${frozen}:mir2-web3/apps/web/public/${entry.slice('assets/'.length)}`])),entry);
    }
    let metadataSha256=null;
    if(['Items','StateItem','UI_32bit'].includes(library)) {
      const relative=`original-ui/${library}/meta.json`,bytes=fs.readFileSync(path.join(unpack,'assets',relative));
      assert(bytes.equals(fs.readFileSync(path.join(resourceRoot,relative))));
      if(library==='UI_32bit') assert(bytes.equals(execFileSync('git',['show',`${frozen}:mir2-web3/apps/web/public/${relative}`])));
      metadataSha256=hash(bytes);
    }
    pngCount+=count;
    return {library,pngCount:count,allSourceBytesMatch:true,metadataSha256};
  });
  assert.equal(pngCount,6647);
  const sha256=hash(fs.readFileSync(apk));
  assert(read(`${variant}-build.log`).includes(sha256));
  return {variant,source,path:apk,bytes:fs.statSync(apk).size,sha256,libraries:proof};
});
const result={source,frozenWindows:frozen,versionCode:14,versionName:'0.1.11-received-chat',
  cleanBeforeAfterBothBuilds:true,sourceGateHashesMatchCommittedInputs:true,gatewayUrl:'',minimumApi:31,targetApi:35,abi:'arm64-v8a',artifacts,
  each6647PngAndThreeMetadataMatchInputs:true,fourWeightFramesMatchFrozenGitBytes:true,
  packageKind:'Rust release cdylib inside Gradle Debug diagnostic variants, not store Release',
  fullResourceReleaseAccepted:false,fullPhoneUiAccepted:false,authenticatedGameplayAccepted:false,physicalDeviceAccepted:false};
fs.writeFileSync(path.join(qa,'package-v14.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
