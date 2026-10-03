// Exact installed package/capture integrity plus bounded manual observations.
// No pixel hashes are treated as gameplay, touch or physical-device acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
const [qa] = process.argv.slice(2);
assert(qa, 'Provide the exact v13 QA target');
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const proof = JSON.parse(read('package-v13.json'));
assert.equal(proof.source, 'cfac6ecd4bb66ac7c4b0a2625ff178f1703335a0');
assert.equal(proof.fourWeightPngsAndMetadataMatchFrozenGitBytesEachApk, true);
assert.equal(read('device-api.txt').trim(), '31');
assert.equal(read('recovered-boot.txt').trim(), '1');
assert.match(read('recovered-device-density.txt'), /Physical density: 440/);
assert.equal(read('device-fingerprint.txt').trim(), read('recovered-device-fingerprint.txt').trim());
for (const variant of ['debug', 'preview']) {
  assert.match(read(`${variant}-install.log`), /Success/);
  assert.match(read(`${variant}-installed-version.txt`), /versionCode=13/);
  assert.match(read(`${variant}-installed-version.txt`), /versionName=0.1.10-weight-bars/);
}
assert.match(read('debug-recovered-installed-version.txt'), /versionCode=13/);
assert.equal(read('final-app-pids.txt'), '');
assert.match(read('final-device.txt'), /emulator-5554\s+device/);
const captures = ['debug-login-recovered.png', 'preview-inventory.png', 'preview-character.png'].map(name => {
  const bytes = fs.readFileSync(path.join(qa, name));
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  const width = bytes.readUInt32BE(16), height = bytes.readUInt32BE(20);
  assert.deepEqual([width, height], [2340, 1080]);
  return { name, width, height, bytes: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex') };
});
const logs = fs.readdirSync(qa).filter(name =>
  /^(debug-login-recovered|preview-inventory|preview-character)-pid\d+\.log$/.test(name)).map(name => {
  const text = read(name), lines = text.split('\n');
  if (name.startsWith('preview-')) assert(text.includes('ANDROID_PLAYER_INGRESS_OFFLINE queued owner42 hp80 mp20 gold12352 credit503 weights66/25/9 xp12.5%; NOT LIVE GAMEPLAY'));
  const result = { name, capturedLines: lines.length,
    errorFatalPanicSignalLines: lines.filter(line => /ERROR\b|panicked|FATAL EXCEPTION|Fatal signal|SIGSEGV|SIGABRT/.test(line)).length,
    missingWeightBarLines: lines.filter(line => /Path not found: original-ui\/UI_32bit\/47[0-3]\.png/.test(line)).length };
  assert.equal(result.errorFatalPanicSignalLines, 0);
  assert.equal(result.missingWeightBarLines, 0);
  return result;
});
assert.equal(logs.length, 3);
const starts = ['start-debug.log', 'start-debug-recovered.log', 'start-preview-inventory.log', 'start-preview-character.log']
  .map(name => ({ name, status: read(name).match(/Status: (\S+)/)?.[1],
    waitTimeMs: Number(read(name).match(/WaitTime: (\d+)/)?.[1]) }));
assert.equal(starts[0].status, 'timeout');
assert(starts.slice(1).every(start => start.status === 'ok'));
const exit = JSON.parse(read('emulator-exit-summary.json'));
assert.equal(exit.app, 'qemu-system-aarch64');
assert.equal(exit.exception.signal, 'SIGABRT');
assert(exit.frames.some(frame => frame.symbol?.includes('CallbackWithSuccessTag')));
const remoteRef = JSON.parse(read('remote-observed-ref.json'));
const pr = JSON.parse(read('remote-observed-pr.json'));
assert.equal(remoteRef.object.sha, 'f04a7439938579480b592b5d4e7381014a612dff');
assert.equal(pr.head, remoteRef.object.sha);
assert.equal(pr.state, 'open');
assert.equal(pr.draft, true);
assert.equal(pr.base, 'codex/playtest-registration');
const result = { source: proof.source, versionCode: 13, deviceApi: 31,
  recoveredFingerprint: read('recovered-device-fingerprint.txt').trim(), density: 440,
  recoveredBootId: read('recovered-boot-id.txt').trim(), captures, logs, starts,
  sourcePublicationConfirmed: false, observedRemoteHead: pr.head,
  observationMethod: 'Manually inspected original ADB capture bytes of SDK diagnostic Intent scenes; no new touch/drag/IME interaction claim.',
  observed: { offlineBagOriginalWeightBarVisible: true, offlineBagAvailableWeight: 34,
    offlineBagAndPhoneHudGold: 12352, offlineCharacterOriginalBodyAndWeaponVisible: true,
    weightAssetErrorsAbsentInCapturedPids: true, allFourWeightAssetBytesAccepted: true,
    phoneHudWeightVisualAccepted: false, alternateWeightRatiosAccepted: false,
    newControlsInteractionAccepted: false, fullPhoneUiAccepted: false,
    fullResourceReleaseAccepted: false, startupOrEmulatorStabilityAccepted: false,
    authenticatedGameplayAccepted: false, physicalDeviceAccepted: false },
  retainedFailures: ['First normal startup timed out13463ms; the emulator host later aborted in a gRPC callback. That is a host crash record, not proof of an Android game crash.',
    'Background recovery did not remain running; the retained-session recovery waited at SDK crash-upload consent. CUA could not expose the standalone SDK application.',
    'The same AVD was recovered with per-launch -crash-report-mode never -no-metrics and no snapshot load/save, no wipe/GPU/auth/global-setting change. The original macOS crash report and app/userdata remain. SDK no-upload handling removed its queued temporary minidump; no recoverable copy of that SDK dump is verified.',
    'Later observed starts638/546/433ms do not erase earlier timeout/crash or accept full startup/stability. Phone HUD has no visually accepted weight row/bar; only BAG471 is rendered here.',
    'Ordinary pushes failed in transport; actual GitHub ref and Draft PR still show f04a74399, not this source or local v12 evidence.'] };
fs.writeFileSync(path.join(qa, 'runtime-v13.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ source: result.source, captures: captures.length, logs, starts,
  observed: result.observed, observedRemoteHead: pr.head }, null, 2));
