// Integrity/IME/log audit, not OCR. Manual original-frame observations are explicit.
// Run from repository root. A real error-log failure remains a nonzero result.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const [qa] = process.argv.slice(2);
assert(qa, 'exact QA target required');
const source = '8cd2e7eaeead8c71f74fbbaa9250bb2d875fed04';
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const packages = JSON.parse(read('package-v16.json'));
assert.equal(packages.source, source);
for (const variant of ['debug', 'preview']) {
  assert.match(read(`${variant}-install.log`), /^Success$/m);
  assert.match(read(`${variant}-installed-version.txt`), /versionCode=16/);
  assert.match(read(`${variant}-installed-version.txt`), /versionName=0.1.13-chat-editor/);
}
for (const name of ['before-installed-version.txt', 'before-repeat-version.txt']) assert.match(read(name), /versionCode=15/);
for (const name of ['before-start-chat.log', 'before-start-hud.log', 'before-start-chat-repeat.log', 'debug-start.log', 'preview-start-chat.log']) {
  assert.match(read(name), /^Status: ok$/m, name);
}
const frames = [
  'before-chat', 'before-keyboard-hidden', 'before-keyboard-hidden-settled',
  'before-draft-retap', 'before-hud', 'before-chat-repeat', 'debug-login',
  'preview-chat', 'preview-draft-typed', 'preview-keyboard-hidden',
  'preview-keyboard-reopened', 'preview-second-keyboard-hidden',
  'preview-keyboard-held-reopened', 'preview-draft-edited-after-reopen',
].map(name => {
  const bytes = fs.readFileSync(path.join(qa, `${name}.png`));
  assert.equal(bytes.subarray(1, 4).toString(), 'PNG', name);
  assert.equal(bytes.readUInt32BE(16), 2340, name);
  assert.equal(bytes.readUInt32BE(20), 1080, name);
  return {name: `${name}.png`, width: 2340, height: 1080, sha256: hash(bytes)};
});
const ime = [
  ['before-keyboard-hidden-settled-ime.txt', false], ['before-draft-tap-ime.txt', false],
  ['preview-chat-ime.txt', true], ['preview-typed-ime.txt', true],
  ['preview-keyboard-hidden-ime.txt', false], ['preview-keyboard-reopened-ime.txt', true],
  ['preview-second-keyboard-hidden-ime.txt', false], ['preview-keyboard-held-reopened-ime.txt', true],
  ['preview-edited-after-reopen-ime.txt', true],
].map(([name, shown]) => {
  assert.match(read(name), new RegExp(`\\bmInputShown=${shown}\\b`), name);
  return {name, inputShown: shown};
});
for (const name of ['debug-login-resumed.txt', 'preview-chat-resumed.txt', 'preview-keyboard-reopened-resumed.txt', 'preview-keyboard-held-reopened-resumed.txt', 'preview-draft-edited-after-reopen-resumed.txt']) {
  assert.match(read(name), /com\.mir2\.web3(?:\.uipreview)?\//, name);
}
const logs = [
  'before-chat-pid.log', 'before-retap-pid.log', 'before-hud-pid.log', 'before-chat-repeat-pid.log',
  'debug-login-pid.log', 'preview-chat-pid.log', 'preview-draft-typed-pid.log',
  'preview-keyboard-hidden-pid.log', 'preview-keyboard-reopened-pid.log',
  'preview-keyboard-held-reopened-pid.log', 'preview-draft-edited-after-reopen-pid.log',
].map(name => {
  const bytes = fs.readFileSync(path.join(qa, name)), entries = bytes.toString().split('\n').filter(line => /^\d{2}-\d{2}\s/.test(line));
  const errors = entries.filter(line => /\s[EF]\s/.test(line));
  const glErrors = entries.filter(line => /GL error 0x506/.test(line));
  const fatal = entries.filter(line => /\sF\s|panicked at|FATAL EXCEPTION|Fatal signal/.test(line));
  return {name, lineEntries: entries.length, errorLineEntries: errors.length, gl506LineEntries: glErrors.length,
    uniqueGl506FullLines: new Set(glErrors).size, fatalPanicSignalLineEntries: fatal.length,
    sha256: hash(bytes), errorLines: errors};
});
const showCalls = name => read(name).split('\n').filter(line => /showSoftInput\(\)/.test(line)).length;
assert.equal(showCalls('preview-chat-pid.log'), 1);
assert.equal(showCalls('preview-keyboard-reopened-pid.log'), 2);
assert.equal(showCalls('preview-keyboard-held-reopened-pid.log'), 3);
assert.equal(showCalls('preview-draft-edited-after-reopen-pid.log'), 3);
assert.match(read('device-process-after.txt'), /22274.+Mir2_API_31_ARM64/);
assert.equal(read('device-state-after.txt').trim(), 'device');
const measuredNewLogs = logs.filter(log => log.name.startsWith('preview-') || log.name.startsWith('debug-'));
const errorFreeLogGate = measuredNewLogs.every(log => log.errorLineEntries === 0 && log.fatalPanicSignalLineEntries === 0);
const report = {
  source, frozenWindows: packages.frozenWindows, versionCode: 16,
  nativeDiagnosticApksInstalled: true, gatewayUrl: '', frames, ime, logs,
  actions: [
    {version: 15, action: 'Android Back, then actual draft tap at1000,978', result: 'FAIL: IME remains hidden; old failure retained'},
    {version: 16, action: 'Actual soft keyboard q@240,694 then w@445,694', result: 'PASS: original shared draft visibly qw'},
    {version: 16, action: 'Android Back, then actual draft tap@1000,978', result: 'PASS: IME false->true and qw visibly retained'},
    {version: 16, action: 'Second Back, then input swipe1000,978->1000,978 for1500ms', result: 'PASS: IME false->true, qw retained and exactly one additional showSoftInput call'},
    {version: 16, action: 'Actual soft keyboard e@655,694 after reopening', result: 'PASS: original shared draft visibly qwe, not stale/closed editor'},
  ],
  manualSemantics: 'Primary agent inspected original uncropped SDK frames. The before-keyboard-hidden frame is a retained hide animation, not stable-hidden evidence; the settled frame is separate. This script checks byte integrity/IME/logs, not OCR.',
  cumulativeLogBoundary: 'Preview PID captures contain repeated copies of the same startup GL entries, not new reports on each capture. Counts are raw line entries, with unique full lines separately reported.',
  originalReopenDefectFixedInThisSpecimen: true, draftPreservedAndEditableInThisSpecimen: true,
  noSendKeyPressed: true, noServerEchoClaim: true, heldGestureShowSoftInputDelta: 1,
  errorFreeLogGate, rendererFixAttempted: false, renderStabilityAccepted: false,
  glInvestigation: 'Old v15 initial chat/HUD captures were error-free; the next v15 chat startup reproduced GL506. V16 chat reproduced it too. Timing near initial IME is a lead, not a verified root cause. No runtime/GPU/backend switch was applied.',
  device: {kind: 'API31 arm64 emulator, not physical phone', avd: read('device-avd.txt').trim(), fingerprint: read('device-fingerprint.txt').trim(), density: read('device-density.txt').trim(), samePid22274BeforeAfter: true},
  ownedAppsStoppedAfterCapture: true, emulatorAndAppDataRetained: true,
  fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
};
fs.writeFileSync(path.join(qa, 'runtime-v16.json'), JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report, null, 2));
if (!errorFreeLogGate) process.exitCode = 1;
