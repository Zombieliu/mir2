// Integrity plus explicitly manual observations of SDK-native diagnostic pixels.
// These assertions do not perform OCR or prove online/player/device acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const [qa] = process.argv.slice(2);
assert(qa, 'exact v15 QA directory required');
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const packageProof = JSON.parse(read('package-v15.json'));
assert.equal(packageProof.source, 'd009f240247261146c1053b5ec8cdd934724cad8');
assert.equal(read('device-api.txt').trim(), '31');
assert.match(read('device-density.txt'), /Physical density: 440/);
assert.match(read('device-avd.txt'), /Mir2_API_31_ARM64/);
for (const variant of ['debug', 'preview']) {
  assert.match(read(`${variant}-install.log`), /Success/);
  assert.match(read(`${variant}-installed-version.txt`), /versionCode=15/);
  assert.match(read(`${variant}-installed-version.txt`), /versionName=0.1.12-chat-layout/);
}
const names = ['debug-login', 'preview-chat', 'preview-chat-keyboard-hidden',
  'preview-chat-reopen-attempt', 'preview-chat-settings-tap',
  'preview-chat-settings-tab', 'preview-chat-settings-cancel'];
const captures = names.map(name => {
  const bytes = fs.readFileSync(path.join(qa, `${name}.png`));
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  assert.deepEqual([bytes.readUInt32BE(16), bytes.readUInt32BE(20)], [2340, 1080]);
  assert.match(read(`${name}-resumed.txt`), new RegExp(`com.mir2.web3${name.startsWith('preview') ? '\\.uipreview' : ''}/`));
  return {name: `${name}.png`, width: 2340, height: 1080, bytes: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex')};
});
const logs = names.map(name => {
  const files = fs.readdirSync(qa).filter(file => new RegExp(`^${name}-pid[0-9]+\\.log$`).test(file));
  assert.equal(files.length, 1, name);
  const text = read(files[0]), lines = text.split('\n');
  if (name.startsWith('preview')) {
    assert(text.includes('ANDROID_OFFLINE_RECEIVED_CHAT_QUEUED_NOT_LIVE'));
    assert.match(text, /queued = 2/);
  }
  const errors = lines.filter(line => /\sE\s|ERROR\b|panicked|FATAL EXCEPTION|Fatal signal|SIGSEGV|SIGABRT/.test(line));
  const fatal = lines.filter(line => /panicked|FATAL EXCEPTION|Fatal signal|SIGSEGV|SIGABRT/.test(line));
  return {name: files[0], capturedLineEntries: lines.length,
    errorFatalPanicSignalEntries: errors.length, fatalPanicSignalEntries: fatal.length,
    errorFreeGatePass: errors.length === 0, errorLines: errors, warningsNotErased: true};
});
assert.match(read(logs.find(log => log.name.startsWith('preview-chat-keyboard-hidden-pid')).name), /EDITOR_BACK preIme action=1 active=true/);
const starts = ['start-debug.log', 'start-preview-chat.log'].map(name => {
  const text = read(name);
  assert.match(text, /Status: ok/);
  return {name, status: 'ok', waitTimeMs: Number(text.match(/WaitTime: (\d+)/)[1])};
});
assert.equal(read('final-app-pids.txt').trim(), '');
assert.match(read('final-device.txt'), /emulator-5554\s+device/);
const oldCapture = fs.readFileSync('mir2-web3/docs/generated/player-qa/native-android-received-chat-20261001/preview-chat.png');
const result = {
  source: packageProof.source, versionCode: 15, deviceApi: 31, density: 440,
  fingerprint: read('device-fingerprint.txt').trim(), captures, logs, starts,
  before: {source: '253b6682f73715b896745ca85a6923be98d0b892', versionCode: 14,
    existingPath: 'mir2-web3/docs/generated/player-qa/native-android-received-chat-20261001/preview-chat.png',
    sha256: createHash('sha256').update(oldCapture).digest('hex'), newestPeerRowVisible: false,
    oldFailureNotEditedOrRecaptured: true},
  observations: {
    method: 'Manually viewed all seven original SDK captures; screenshot integrity script is not semantic OCR.',
    ordinaryEmptyLoginFailsClosedWithoutGateway: true,
    offlineSystemAndLatestPeerRowsBothVisibleAboveKeyboard: true,
    actualAndroidBackDismissesKeyboardAndRetainsBothRows: true,
    focusedDraftTapReopensKeyboard: false,
    actualSetTapOpensOriginalFilterSettings: true,
    actualChatBoxTabTapSelectsTransparencyOptions: true,
    actualCancelTapClosesModalAndRestoresBothRows: true,
    actions: [
      {action: 'Android BACK keyevent4', result: 'keyboard disappears, both received rows remain'},
      {action: 'tap draft at original screenshot physical1000,978', result: 'FAIL: keyboard remains hidden'},
      {action: 'tap Set physical1955,970', result: 'original FILTER modal opens'},
      {action: 'tap CHAT BOX physical1180,229', result: 'transparency page appears'},
      {action: 'tap Cancel physical1105,874', result: 'modal closes, both rows remain'},
    ],
    noTypingSendEchoApplyTogglesTradeScrollSoakOrRealNetworkClaim: true,
  },
  ownedAppsStoppedDataRetainedSameAvdRunning: true,
  existingExtremeImeControlOverflowOpen: true, sharedRuntimeChatEvictionUnchangedAndOpen: true,
  priorV14ClippedRowAndV13HostCrashTimeoutFailuresNotErased: true,
  // Preserve the real error-free gate FAILURE; writing an integrity report
  // must not silently reinterpret emulator renderer errors as a zero-error pass.
  errorFreeLogGatePass: logs.every(log => log.errorFreeGatePass),
  uniqueCapturedErrorLines: [...new Set(logs.flatMap(log => log.errorLines))],
  rendererErrors: 'Six emuglGLESv2_enc GL0x506 framebuffer-completeness error line entries in the first preview PID capture; three unique timestamp/message strings. Cumulative later captures repeat those six entries, not36 new reported entries. Root cause unverified.',
  firstZeroErrorAuditFailureRetained: true,
  fullPhoneUiAccepted: false, fullChatInteractionAccepted: false,
  startupStabilityAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
};
fs.writeFileSync(path.join(qa, 'runtime-v15.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result, null, 2));
if (!result.errorFreeLogGatePass) process.exitCode = 1;
