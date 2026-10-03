// Local source controls. Do not interpret these as APK or online acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const [qa] = process.argv.slice(2);
assert(qa, 'Provide the exact evidence directory');
const read = name => fs.readFileSync(path.join(qa,name),'utf8');
const rust = [['normal-final.log',267,0],['preview-final.log',281,0],['shared-final.log',1205,8],
  ['windows-chat-mac.log',2,0]].map(([name,passed,ignored]) => {
  const result = read(name).match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/);
  assert(result,name);
  assert.deepEqual(result.slice(1).map(Number),[passed,0,ignored],name);
  return {name,passed,failed:0,ignored};
});
for (const [file,variant] of [['api31-normal.log','debug'],['api31-preview.log','uiPreview']]) {
  assert(read(file).includes(`check ${variant} aarch64-linux-android`));
  assert(read(file).includes('target check passed'));
}
assert(read('java-final.log').includes('BUILD SUCCESSFUL'));
assert(read('java-final.log').includes('42 actionable tasks: 42 executed'));
assert(read('java-before-corrected.log').includes('1 test completed, 1 failed'));
assert.match(read('java-before-corrected.xml'), /failures="1"/);
const java = ['Debug','UiPreview'].map(variant => {
  const directory = `mir2-web3/apps/game-client/platform-android/android/app/build/test-results/test${variant}UnitTest`;
  const files = fs.readdirSync(directory).filter(file => file.endsWith('.xml'));
  const totals = {tests:0,failures:0,errors:0,skipped:0};
  for (const file of files) {
    const xml = fs.readFileSync(path.join(directory,file),'utf8');
    const suite = xml.match(/<testsuite[^>]+>/)?.[0]; assert(suite,file);
    for (const field of Object.keys(totals)) totals[field] += Number(suite.match(new RegExp(`${field}="(\\d+)"`))[1]);
    fs.writeFileSync(path.join(qa,`${variant}-${file}`),xml);
  }
  assert.deepEqual(totals,{tests:37,failures:0,errors:0,skipped:0});
  return {variant,suiteCount:files.length,...totals,environment:'local synthetic TLS/MockWebServer'};
});
const files = [
  'client-bevy/src/native_chat_ingress.rs','client-bevy/src/lib.rs','platform-windows/src/gateway.rs',
  'platform-android/src/chat_ingress.rs','platform-android/src/chat_ingress_tests.rs',
  'platform-android/src/lib.rs','platform-android/src/player_ingress.rs','platform-android/src/shared_shell.rs',
  'platform-android/src/ui_preview.rs','platform-android/android/app/build.gradle',
  'platform-android/android/app/src/main/java/com/mir2/web3/GatewaySession.java',
  'platform-android/android/app/src/test/java/com/mir2/web3/GatewaySessionTest.java',
].map(file => ({path:`mir2-web3/apps/game-client/${file}`,sha256:createHash('sha256')
  .update(fs.readFileSync(`mir2-web3/apps/game-client/${file}`)).digest('hex')}));
const result = {frozenWindows:'3f5e61533235921369bc13a7760b4a56b0e467e5',rust,java,sourceFiles:files,
  extraction:JSON.parse(read('source-equivalence.json')),arm64Api31:{normal:true,actualPreviewFeature:true},
  hostRetryFifoOnly:true,sharedRuntimeChatEvictionUnchangedAndOpen:true,
  apkAccepted:false,fullPhoneUiAccepted:false,authenticatedGameplayAccepted:false,physicalDeviceAccepted:false};
fs.writeFileSync(path.join(qa,'source-results.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
