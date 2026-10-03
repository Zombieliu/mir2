// Local source gates only; this does not certify Windows or Android gameplay.
import fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
const [qa] = process.argv.slice(2);
assert(qa, 'provide the exact scoped evidence directory');
const read = name => fs.readFileSync(path.join(qa,name),'utf8');
const rust = [
  ['normal-final.log',257,0],['preview-final.log',270,0],['shared-final.log',1203,8],
  ['windows-cursor-mac.log',2,0],['windows-weight-mac.log',1,0],
  ['windows-wallet-mac.log',2,0],['windows-appearance-mac.log',1,0],
].map(([file,passed,ignored]) => {
  const match = read(file).match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/);
  assert(match,file); assert.deepEqual(match.slice(1).map(Number),[passed,0,ignored],file);
  return {file,passed,failed:0,ignored};
});
for(const [file,variant] of [['api31-normal.log','debug'],['api31-preview.log','uiPreview']]) {
  assert(read(file).includes(`check ${variant} aarch64-linux-android`),file);
  assert(read(file).includes('target check passed'),file);
}
assert(read('java-final.log').includes('BUILD SUCCESSFUL'));
const java = ['Debug','UiPreview'].map(variant => {
  const directory = `mir2-web3/apps/game-client/platform-android/android/app/build/test-results/test${variant}UnitTest`;
  const files = fs.readdirSync(directory).filter(file => file.endsWith('.xml'));
  const totals = {tests:0,failures:0,errors:0,skipped:0};
  for(const file of files) {
    const xml = fs.readFileSync(path.join(directory,file),'utf8');
    const suite = xml.match(/<testsuite[^>]+>/)?.[0]; assert(suite,file);
    for(const field of Object.keys(totals)) totals[field] += Number(suite.match(new RegExp(`${field}="(\\d+)"`))[1]);
    fs.writeFileSync(path.join(qa,`${variant}-${file}`),xml);
  }
  assert.deepEqual(totals,{tests:36,failures:0,errors:0,skipped:0});
  return {variant,suiteCount:files.length,...totals,environment:'local synthetic TLS/MockWebServer'};
});
const result = {frozenWindows:'3f5e61533235921369bc13a7760b4a56b0e467e5',rust,java,
  arm64Api31:{normal:true,actualPreviewFeature:true},
  extraction:JSON.parse(read('source-equivalence.json')),
  windowsExecutionHost:'macOS source-filter tests, not Windows device acceptance',
  apkAccepted:false,authenticatedGameplayAccepted:false,physicalDeviceAccepted:false};
fs.writeFileSync(path.join(qa,'source-results.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify(result,null,2));
