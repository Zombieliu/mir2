// Integrity and actual process/state audit. Manual frame observations are not OCR.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const [qa] = process.argv.slice(2);
assert(qa, 'exact curated QA directory required');
const root = path.join(qa, 'v18', 'device');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const source = 'aa1f2c4ada37ed880b71da5168d0b080583524a3';
const pkg = JSON.parse(fs.readFileSync(path.join(qa, 'v18', 'package-v18.json')));
assert.equal(pkg.source, source);
assert.equal(pkg.frozenWindows, '3d735745f1117d42a7859e87604a106351dca935');
for (const variant of ['debug', 'preview']) {
  assert.match(read(`${variant}-install.log`), /^Success$/m);
  assert.match(read(`${variant}-installed-package.txt`), /versionCode=18/);
  assert.match(read(`${variant}-installed-package.txt`), /versionName=0.1.15-npc-consumer/);
}
const frames = [
  'debug-login', 'preview-npcshop', 'preview-npcshop-sell', 'preview-npcshop-repair',
  'preview-npcshop-srepair', 'preview-srepair-after-close', 'preview-world-render',
  'preview-chat', 'preview-chat-keyboard-hidden', 'preview-chat-keyboard-reopened',
].map(name => {
  const bytes = fs.readFileSync(path.join(root, `${name}.png`));
  assert.equal(bytes.subarray(1, 4).toString(), 'PNG', name);
  assert.equal(bytes.readUInt32BE(16), 2340, name);
  assert.equal(bytes.readUInt32BE(20), 1080, name);
  return {name: `${name}.png`, width: 2340, height: 1080, sha256: hash(bytes)};
});
const sceneRuns = ['npcshop', 'npcshop-sell', 'npcshop-repair', 'npcshop-srepair', 'world-render', 'chat'];
assert.match(read('debug-start.log'), /^Status: ok$/m);
for (const scene of sceneRuns) assert.match(read(`preview-${scene}-start.log`), /^Status: ok$/m);
const npc = [
  ['npcshop', 'Buy', 1], ['npcshop-sell', 'Sell', 0],
  ['npcshop-repair', 'Repair', 0], ['npcshop-srepair', 'SpecialRepair', 0],
].map(([scene, mode, count]) => {
  const log = read(`preview-${scene}-pid.log`);
  assert.match(log, new RegExp(`I event .+ANDROID_NPC_INGRESS_OFFLINE_NOT_LIVEscene = "${scene}"; catalogues = ${count}; services = 1; rejected = 0;`));
  const observer = log.split('\n').filter(line => /I event .+ANDROID_NPC_PREVIEW_SHARED_CONSUMER_OPEN_NOT_LIVE/.test(line));
  assert.equal(observer.length, 1, scene);
  assert.match(observer[0], new RegExp(`scene = "${scene}"; mode = ${mode}; goods = ${count};`));
  if (scene === 'npcshop') assert.match(observer[0], /first_id = 9007199254740993;/);
  assert.match(read(`preview-${scene}-ime.txt`), /\bmInputShown=false\b/);
  return {scene, mode, catalogueCount: count, serviceCount: 1, originalFrameWindowObserved: true,
    offlineOnly: true, uiStateReceiptIsNotGpuOrServerReceipt: true};
});
assert.match(read('preview-world-render-pid.log'), /ANDROID_WORLD_RENDER_READY.*map_tiles = 849; entities = 8; entity_layers = 13;/);
assert.match(read('preview-chat-ime.txt'), /\bmInputShown=true\b/);
assert.match(read('preview-chat-keyboard-hidden-ime.txt'), /\bmInputShown=false\b/);
assert.match(read('preview-chat-keyboard-reopened-ime.txt'), /\bmInputShown=true\b/);
const logs = [
  'debug-login', ...sceneRuns.map(scene => `preview-${scene}`),
  'preview-srepair-after-close', 'preview-chat-keyboard-reopened',
].map(name => {
  const bytes = fs.readFileSync(path.join(root, `${name}-pid.log`));
  const entries = bytes.toString().split('\n').filter(line => /^\d{2}-\d{2}\s/.test(line));
  const errors = entries.filter(line => /\s[EF]\s/.test(line));
  const gl = entries.filter(line => /GL error 0x506/.test(line));
  const fatal = entries.filter(line => /\sF\s|panicked at|FATAL EXCEPTION|Fatal signal/.test(line));
  return {name: `${name}-pid.log`, lineEntries: entries.length, errorLineEntries: errors.length,
    gl506LineEntries: gl.length, uniqueGl506FullLines: new Set(gl).size,
    fatalPanicSignalLineEntries: fatal.length, sha256: hash(bytes), errorLines: errors};
});
const primary = logs.filter(log => !/after-close|reopened/.test(log.name));
assert.deepEqual(primary.map(log => log.gl506LineEntries), [11, 6, 10, 0, 18, 0, 0]);
assert(logs.every(log => log.fatalPanicSignalLineEntries === 0));
assert.match(read('preview-npcshop-pid.log'), /Path not found: original-ui\/Items\/658.png/);
const closed = logs.find(log => log.name === 'preview-srepair-after-close-pid.log');
const original = logs.find(log => log.name === 'preview-npcshop-srepair-pid.log');
assert.deepEqual(closed.errorLines, original.errorLines, 'post-close capture repeats the same startup errors');
assert.equal(read('task-app-pids-after.txt'), '');
assert.equal(read('device-state-after.txt').trim(), 'device');
for (const name of ['device-process-before.txt', 'device-process-after.txt']) assert.match(read(name), /22274.+Mir2_API_31_ARM64/);
const beforeRoot = path.join(qa, 'v17', 'device');
assert.match(fs.readFileSync(path.join(beforeRoot, 'preview-installed-package.txt'), 'utf8'), /versionCode=17/);
assert.match(fs.readFileSync(path.join(beforeRoot, 'preview-npcshop-pid.log'), 'utf8'), /catalogues = 1; services = 1; rejected = 0;/);
const integrity = JSON.parse(fs.readFileSync(path.join(qa, 'curation-integrity.json')));
for (const file of integrity.files) assert.equal(hash(fs.readFileSync(file.target)), file.sha256, file.target);
const errorFreeLogGate = logs.every(log => log.errorLineEntries === 0);
const result = {
  source, frozenWindows: pkg.frozenWindows, versionCode: 18, gatewayUrl: '',
  actualInstallAndSourcePackageGate: true, frames, npc, logs,
  priorV17MissingWindow: {source: '9458ae4a1e11fbfe409f08c1af014fd0703ebff4', accepted: false, originalFramePreserved: true},
  offlineNpcWindowOpeningObserved: true, nativeSceneRenderSpecimen: {mapTiles: 849, entities: 8, layers: 13, offlineOnly: true},
  chatSmoke: {twoReceivedSyntheticLinesVisible: true, actualBackAndRetapIme: [true, false, true],
    textEntryOrDraftRetentionRevalidated: false, noSendKeyPressed: true, noServerEchoClaim: true},
  failures: {
    itemImage: 'Synthetic Buy fixture names icon658, absent from the sparse approved Items pack; label/price visible but image missing. Not a live server-packet/resource-completeness pass.',
    closeTap: 'Actual1278,158 tap on SpecialRepair red X did not close the visible window. Picking/calibration/root cause still unverified; original after frame retained.',
    renderer: 'GL0x506 in normal and multiple NPC startups without any showSoftInput call. Earlier initial-IME timing is not an established cause. No GPU/runtime/backend change in this leaf.',
  },
  manualSemantics: 'Primary agent inspected all original uncropped SDK frames; this script validates byte/state/log integrity, not OCR or touch-target calibration.',
  logBoundary: '45 raw GL entries in seven distinct startup PIDs; after-close repeats the same18 entries and is not18 additional errors. Unique full lines recorded separately.',
  errorFreeLogGate, renderStabilityAccepted: false, completeNpcFlowAccepted: false,
  fullResourceReleaseAccepted: false, fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  device: {kind: 'API31 arm64 emulator, not phone', avd: read('device-avd.txt').trim(),
    fingerprint: read('device-fingerprint.txt').trim(), density: read('device-density.txt').trim(),
    sameDedicatedEmulatorPid22274: true, taskAppsStoppedAfterCapture: true, emulatorAndAppDataRetained: true},
};
fs.writeFileSync(path.join(qa, 'runtime-v18.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({source, offlineNpcWindows: 4, primaryGlEntries: 45, errorFreeLogGate,
  fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false}));
if (!errorFreeLogGate) process.exitCode = 1;
