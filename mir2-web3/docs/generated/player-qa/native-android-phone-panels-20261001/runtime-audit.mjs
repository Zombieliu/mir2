// Offline capture integrity and bounded, manually observed UI results. No ADB mutation.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const [qa] = process.argv.slice(2);
assert(qa, 'provide the exact v11 QA directory');
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const packageProof = JSON.parse(read('package-v11.json'));
assert.equal(packageProof.source, '6fe6a17ac229e9ebf0181a1ce02062ae00db71f4');
assert.equal(read('device-api.txt').trim(), '31');
assert.match(read('device-density.txt'), /Physical density: 440/);
for (const variant of ['debug', 'preview']) {
  assert.match(read(`${variant}-install.log`), /Success/);
  assert.match(read(`${variant}-installed-version.txt`), /versionCode=11/);
  assert.match(read(`${variant}-installed-version.txt`), /versionName=0.1.8-phone-panels/);
}
const names = [
  'debug-login.png', 'preview-inventory.png', 'preview-inventory-bag2.png',
  'preview-inventory-quest.png', 'preview-inventory-bag2-held.png',
  'preview-inventory-restored.png', 'preview-inventory-closed.png',
  'preview-character.png', 'preview-character-stats1.png',
  'preview-character-stats2.png', 'preview-character-spells.png',
  'preview-character-restored.png', 'preview-character-closed.png',
];
const captures = names.map(name => {
  const png = fs.readFileSync(path.join(qa, name));
  assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  assert.equal(png.readUInt32BE(16), 2340);
  assert.equal(png.readUInt32BE(20), 1080);
  return { name, width: 2340, height: 1080, bytes: png.length,
    sha256: crypto.createHash('sha256').update(png).digest('hex') };
});
const logs = ['preview-inventory-final-pid5324.log',
  'preview-character-final-pid5739.log', 'debug-login-pid5871.log'].map(name => {
  const lines = read(name).split('\n');
  return { name, capturedLines: lines.length,
    errorFatalPanicSignalLines: lines.filter(line =>
      /ERROR\b|panicked|FATAL EXCEPTION|Fatal signal|SIGSEGV|SIGABRT/.test(line)).length };
});
const starts = ['start-preview-inventory.log', 'start-preview-character.log',
  'start-debug.log'].map(name => {
  const log = read(name);
  return { name, status: log.match(/Status: (\S+)/)?.[1],
    waitTimeMs: Number(log.match(/WaitTime: (\d+)/)?.[1]) };
});
const result = {
  source: packageProof.source, versionCode: 11, deviceApi: 31,
  fingerprint: read('device-fingerprint.txt').trim(),
  physicalSize: read('device-size.txt').trim(), density: 440,
  captures, logs, starts,
  observationMethod: 'Manual inspection of original ADB captures and actual tap sequence; geometry is not inferred from screenshot hashes.',
  observed: {
    ordinaryBagAndCharacterClearStatusChatAndThumbs: true,
    sixBeltTargetsVisibleInSidebar: true,
    bagQuestTabAndReturnRespond: true,
    bagCloseRestoresNormalHud: true,
    allFourCharacterPagesAndCloseRespond: true,
    disabledSecondBagUnlockAccepted: false,
    characterStatsValuesAccepted: false,
    allMinimum48DpControlsAccepted: false,
    realItemDragUseEquipReceiptsAccepted: false,
    launchPerformanceAccepted: false,
    fullPhoneUiAccepted: false,
    authenticatedGameplayAccepted: false,
    physicalDeviceAccepted: false,
  },
  retainedGaps: [
    'Preview character fixture has no crystal_stats; shared values are intentionally absent until supplied, not fabricated.',
    'Character scene SPELLS has generic fixtures, not the separate typed skills-scene icon evidence.',
    'Quest-tab immediate capture has transient missing tab artwork; caption stability not accepted.',
    'Second bag is disabled because this offline specimen is not expanded; tap and hold are not unlock evidence.',
    'Normal login is small and says Test server not configured; neither credentials nor saves were used.',
    'Inventory and normal launches timed out before later visible frames.',
    'Compact/IME, all dialogs, frame and item drag geometry, live packets and physical device remain open.',
  ],
};
fs.writeFileSync(path.join(qa, 'runtime-v11.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify({ source: result.source, captureCount: captures.length, logs, starts, observed: result.observed }, null, 2));
