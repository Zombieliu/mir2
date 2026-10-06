import assert from 'node:assert/strict';
import {readFileSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const qa = dirname(fileURLToPath(import.meta.url));
const source = '78e7d2309f6a6b92cea1737c76c5eedfd863e46e';
const artifact = JSON.parse(readFileSync(join(qa, 'v19/package-v19.json')));
assert.equal(artifact.source, source);
const touches = JSON.parse(readFileSync(join(qa, 'v19/touch-device-v3/commands.json')));
assert.equal(touches.source, source);
assert.equal(touches.apkSha256, artifact.artifacts.find(a => a.variant === 'preview').sha256);
const read = relative => readFileSync(join(qa, relative), 'utf8');
const entries = [];
const pids = new Set();
const inspect = (directory, scene, suffix) => {
  const pid = Number(read(`${directory}/${scene}-pid.txt`).trim());
  assert(Number.isInteger(pid) && !pids.has(pid));
  pids.add(pid);
  const file = `${directory}/${scene}${suffix}-pid.log`;
  const lines = read(file).split(/\r?\n/);
  const canonical = lines.filter(line => /\sI event\s/.test(line));
  const matching = pattern => lines.filter(line => pattern.test(line));
  const frameName = suffix ? `${scene}-after.png` : `${scene}.png`;
  const frame = readFileSync(join(qa, directory, frameName));
  assert.equal(frame.readUInt32BE(16), 2340);
  assert.equal(frame.readUInt32BE(20), 1080);
  const result = {directory, scene, pid, log: file,
    gl506: matching(/GL error 0x506/).length,
    missingPaths: matching(/Path not found/).length,
    fatalOrPanic: matching(/FATAL EXCEPTION|panicked at|Fatal signal|thread .*panicked/),
    npcConsumer: canonical.filter(line => line.includes('ANDROID_NPC_PREVIEW_SHARED_CONSUMER_OPEN_NOT_LIVE')),
    localClose: canonical.filter(line => line.includes('ANDROID_NPC_CLOSE_SHARED_LOCAL_UI_EXIT_REQUEST_NOT_LIVE'))};
  entries.push(result);
  return result;
};
for (const scene of ['debug-login', 'npcshop', 'npcshop-sell', 'npcshop-repair', 'npcshop-srepair']) {
  const result = inspect('v19/device', scene, '');
  if (scene !== 'debug-login') assert.equal(result.npcConsumer.length, 1);
  if (scene === 'npcshop') assert.match(result.npcConsumer[0], /icon_width = 36; icon_height = 26/);
  assert.equal(result.localClose.length, 0);
}
for (const stage of touches.stages) {
  const result = inspect('v19/touch-device-v3', stage.name, '-after');
  assert.equal(result.pid, stage.pid);
  assert.equal(result.npcConsumer.length, 1);
  assert.equal(result.localClose.length, 1);
  assert(!read(`v19/touch-device-v3/${stage.name}-before-pid.log`).includes('ANDROID_NPC_CLOSE_SHARED_LOCAL_UI_EXIT_REQUEST_NOT_LIVE'));
}
assert.equal(touches.stages.length, 4);
const gl506 = entries.reduce((count, entry) => count + entry.gl506, 0);
assert.equal(gl506, 135);
assert.equal(entries.reduce((count, entry) => count + entry.missingPaths, 0), 0);
assert.equal(entries.flatMap(entry => entry.fatalOrPanic).length, 0);
const result = {source, versionCode: 19, logCounting: 'one latest cumulative log per distinct PID; no before/after double count; canonical event only for duplicated Rust markers',
  distinctPids: pids.size, entries, gl506, selectedCaptureMissingPaths: 0,
  actualSharedNpcCloseTaps: 4, screenshotsInspectedSeparatelyByPrimary: true,
  zeroErrorRendererGate: 'FAIL', fullPhoneUiAccepted: false, actualJniAuthenticatedNpcAccepted: false,
  onlineTransactionsAccepted: false, physicalDeviceAccepted: false, wholeGoal: 'Active'};
writeFileSync(join(qa, 'runtime-audit.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({distinctPids: pids.size, actualSharedNpcCloseTaps: 4, gl506,
  zeroErrorRendererGate: 'FAIL', wholeGoal: 'Active'}));
