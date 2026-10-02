import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, openSync, closeSync} from 'node:fs';
import {execFileSync, spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {resolve} from 'node:path';
const base = 'mir2-web3/apps/game-client/platform-android/';
const file = base + 'android/app/src/main/java/com/mir2/web3/GatewaySession.java';
const actual = readFileSync(file, 'utf8');
const baseline = execFileSync('git', ['show', `2c2e5e214693859d7d56e414fb3ff9233e58409c:${file}`], {encoding:'utf8'});
const patch = (before, after) => '*** Begin Patch\n*** Update File: ' + file + '\n@@\n'
  + before.trimEnd().split('\n').map(l=>'-'+l).join('\n') + '\n'
  + after.trimEnd().split('\n').map(l=>'+'+l).join('\n') + '\n*** End Patch\n';
const hash = value => createHash('sha256').update(value).digest('hex');
// Temporarily remove ONLY this leaf's owned Java change using apply_patch.
// No checkout/reset/stash, no user file or branch switching. Always restore
// the exact saved bytes after the baseline fixture run, including on failure.
let result;
try {
  execFileSync('apply_patch', [], {input:patch(actual, baseline)});
  const fd = openSync(base+'target/quest-ingress-20261002/java-quest-compiled-red.log','wx');
  result = spawnSync('./gradlew', ['--no-daemon','--offline','--rerun-tasks','testDebugUnitTest',
    '--tests','com.mir2.web3.GatewaySessionTest.questMetadataStagesOnlyDuringAuthenticatedSelectedStart',
    '--tests','com.mir2.web3.GatewaySessionTest.oversizedQuestMetadataFailsClosedBeforeOwnerBootstrap'], {
      cwd:base+'android', timeout:180000, stdio:['ignore',fd,fd], env:{...process.env,
        JAVA_HOME:'/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home',
        ANDROID_SDK_ROOT:'/Users/henryliu/Library/Android/sdk',
        ANDROID_HOME:'/Users/henryliu/Library/Android/sdk'}});
  closeSync(fd);
} finally {
  execFileSync('apply_patch', [], {input:patch(readFileSync(file,'utf8'),actual)});
  assert.equal(readFileSync(file,'utf8'), actual);
  writeFileSync(base+'target/quest-ingress-20261002/java-red-source-binding.json',JSON.stringify({
    source:'2c2e5e214693859d7d56e414fb3ff9233e58409c',baselineFile:file,
    baselineSha256:hash(baseline),finalOwnedSourceRestoredSha256:hash(actual),
    testSourceSha256:hash(readFileSync(base+'android/app/src/test/java/com/mir2/web3/GatewaySessionTest.java')),
    exactCurrentSourceRestored:true,status:result?.status,error:result?.error?.message,
    environmentOnlyFirstAttemptRetained:'java-quest-red.log; SDK missing, no product test ran'},null,2)+'\n');
}
console.log(JSON.stringify({gate:'java-quest-compiled-red',status:result.status,sourceRestored:true}));
assert.notEqual(result.status, 0, 'Baseline must fail the added quest tests');
