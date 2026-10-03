import fs from 'node:fs';
import path from 'node:path';
import cp from 'node:child_process';
import crypto from 'node:crypto';
import zlib from 'node:zlib';
import assert from 'node:assert/strict';
import {fileURLToPath} from 'node:url';

const directory = path.dirname(fileURLToPath(import.meta.url));
const mode = process.argv[2] || 'working';
assert(['working', 'index', 'HEAD'].includes(mode));
const git = args => cp.execFileSync('git', args, {cwd: directory, maxBuffer: 32 * 1024 * 1024});
const repo = git(['rev-parse', '--show-toplevel']).toString().trim();
const qa = path.relative(repo, directory).split(path.sep).join('/');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const stored = relative => {
  assert(!relative.includes('..') && !path.isAbsolute(relative));
  return mode === 'working'
    ? fs.readFileSync(path.join(repo, qa, relative))
    : git(['show', mode === 'index' ? ':' + qa + '/' + relative : 'HEAD:' + qa + '/' + relative]);
};
const manifest = JSON.parse(stored('manifest.json'));
const originals = new Map(manifest.payloads.map(row => [row.originalPath || row.path, row]));
const read = relative => {
  const row = originals.get(relative);
  if (!row) return stored(relative);
  const bytes = stored(row.path);
  return row.encoding === 'gzip' ? zlib.gunzipSync(bytes) : bytes;
};
const json = name => JSON.parse(read(name).toString());
const raw = name => json('raw/' + name);
const d = json('diagnosis.json'), b = raw('source-commit-binding.json');
assert.equal(d.source, b.source);
assert.equal(manifest.source, d.source);
assert.equal(manifest.payloadCount, 406);
assert.equal(manifest.payloads.length, manifest.payloadCount);
assert.equal(manifest.totalBytes, 36479296);
assert.equal(manifest.totalOriginalBytes, 663495775);
assert.equal(manifest.losslessGzipCount, 32);
assert.equal(manifest.totalBytes, manifest.payloads.reduce((n, row) => n + row.bytes, 0));
assert.equal(manifest.totalOriginalBytes, manifest.payloads.reduce((n, row) => n + row.originalBytes, 0));
for (const row of manifest.payloads) {
  const bytes = stored(row.path);
  assert.equal(bytes.length, row.bytes, row.path);
  assert.equal(sha(bytes), row.sha256, row.path);
  assert(!/\.(apk|aab|so|jks|keystore|pem|p12)$/i.test(row.path));
  const original = row.encoding === 'gzip' ? zlib.gunzipSync(bytes) : bytes;
  assert.equal(original.length, row.originalBytes, row.path + ' original length');
  assert.equal(sha(original), row.originalSha256, row.path + ' original SHA');
}
assert.equal(d.goalStatus, 'Active');
assert.equal(d.fullNI12Status, 'PARTIAL');
assert.equal(d.fullNI13Status, 'PARTIAL');
assert.equal(d.version, 35);
assert.equal(d.versionName, '0.1.32-gameshop-phone');
assert.equal(d.frozenWindows, '3d735745f1117d42a7859e87604a106351dca935');
git(['merge-base', '--is-ancestor', d.frozenWindows, d.source]);
assert.deepEqual(git(['diff-tree', '--no-commit-id', '--name-only', '-r', d.source])
  .toString().trim().split('\n').sort(), [...d.scope].sort());
assert.deepEqual(b.sourceScope, d.scope);
assert.equal(b.parent, d.previousPublishedAndroid);
assert.equal(b.inputs.length, 50);
assert.equal(d.inputCount, 50);
for (const row of b.inputs) {
  const bytes = git(['show', d.source + ':' + row.path]);
  assert.equal(bytes.length, row.bytes);
  assert.equal(sha(bytes), row.sha256, row.path);
}
for (const file of b.protectedPathsUnchanged) {
  assert.equal(sha(git(['show', d.source + ':' + file])),
    sha(git(['show', b.parent + ':' + file])), file);
}
for (const file of [
  'mir2-web3/apps/game-client/client-bevy/src/crystal_ui/game_shop_dialog.rs',
  'mir2-web3/apps/game-client/client-bevy/src/crystal_ui/overlays.rs',
]) {
  const before = git(['show', b.parent + ':' + file]).toString();
  const after = git(['show', d.source + ':' + file]).toString();
  const marker = '\nmod tests {';
  assert(before.includes(marker) && after.includes(marker), file);
  assert.equal(before.split(marker)[1], after.split(marker)[1], 'old tests unchanged: ' + file);
}
const hitTests = 'mir2-web3/apps/game-client/client-bevy/src/crystal_ui/game_shop_hit_tests.rs';
assert.equal(sha(git(['show', b.parent + ':' + hitTests])), sha(git(['show', d.source + ':' + hitTests])));
const red = raw('working-phone-compiled-red-v35-result.json');
const green = raw('working-phone-generated-green-v35-result.json');
const behavior = raw('working-phone-behavior-v35-result.json');
assert.equal(red.code, 101);
assert.equal(red.sourceUnchanged, true);
assert.equal(green.code, 0);
assert.equal(behavior.code, 0);
assert(read('raw/working-phone-compiled-red-v35.log').toString().includes('1 passed; 2 failed;'));
assert(read('raw/working-phone-generated-green-v35.log').toString().includes('3 passed; 0 failed;'));
assert(read('raw/working-phone-behavior-v35.log').toString().includes('14 passed; 0 failed;'));
assert.equal(raw('working-phone-green-preparation-v35-result.json').code, 101);
assert(read('raw/working-phone-green-preparation-v35.log').toString().includes('E0716'));
assert.equal(green.source, b.parent);
assert.equal(green.workingInputs, true);
assert.equal(green.sourceUnchanged, true);
assert.deepEqual(green.sourceBefore, green.sourceAfter);
assert.equal(behavior.source, b.parent);
assert.equal(behavior.workingInputs, true);
assert.equal(behavior.sourceUnchanged, true);
assert.deepEqual(behavior.sourceBefore, behavior.sourceAfter);
for (const row of behavior.sourceBefore) {
  assert.equal(row.sha256, b.inputs.find(input => input.path === row.path).sha256);
}
for (const row of green.sourceBefore.filter(row => !row.path.endsWith('/game_shop_phone.rs'))) {
  assert.equal(row.sha256, b.inputs.find(input => input.path === row.path).sha256);
}
assert.equal(raw('evidence-verifier-working-v35-result.json').exitCode, 1);
assert(read('raw/evidence-verifier-working-v35.log').toString().includes('e99078467c5229e73bc13fa4be13a1aee7f8a598bdd836e9783face93254ef21'));
assert.equal(raw('evidence-verifier-preparation-not-product.json').wholeGoalComplete, false);
assert.equal(d.gates.length, 7);
for (const label of d.gates) {
  const result = raw(label + '-result.json'), log = read('raw/' + label + '.log').toString();
  assert.equal(result.source, d.source);
  assert.equal(result.code, 0);
  assert.equal(result.sourceUnchanged, true);
  assert.deepEqual(result.sourceBefore, b.inputs);
  assert.deepEqual(result.sourceAfter, b.inputs);
  let counts;
  if (label.startsWith('bound-rust-normal-')) counts = [359, 0];
  if (label.startsWith('bound-rust-preview-')) counts = [387, 0];
  if (label.startsWith('bound-shared-native-player-')) counts = [1281, 10];
  if (label.startsWith('bound-runtime-')) counts = [292, 1];
  if (counts) assert(log.includes('test result: ok. ' + counts[0] + ' passed; 0 failed; ' + counts[1] + ' ignored;'));
  if (label.startsWith('bound-api31-')) {
    assert(result.cwd.endsWith('/platform-android'));
    assert.deepEqual(result.args.slice(0, 7), ['+1.95.0', 'ndk', '-t', 'arm64-v8a', '--platform', '31', 'check']);
    assert(log.includes('Finished'));
  }
  if (label.startsWith('bound-java-')) {
    assert(result.args.includes('--no-build-cache') && result.args.includes('--rerun-tasks'));
    assert(log.includes('BUILD SUCCESSFUL'));
    assert.equal(result.xml.length, 12);
  }
}
const java = raw('java-results.json');
assert.equal(java.length, 2);
for (let variant = 0; variant < 2; variant++) {
  const result = java[variant];
  assert.equal(result.tests, 52);
  assert.equal(result.failures + result.errors + result.ignored, 0);
  assert.equal(result.suites.length, 6);
  for (const suite of result.suites) {
    assert.equal(typeof suite.file, 'string');
    const xml = read('raw/bound-java-full-v35-xml-' + variant + '/' + suite.file).toString();
    const head = xml.match(/<testsuite\b[^>]*>/)[0];
    const value = key => Number(head.match(new RegExp(key + '="(\\d+)"'))?.[1] ?? 0);
    assert.equal(value('tests'), suite.tests);
    assert.equal(value('failures') + value('errors') + value('skipped'), 0);
  }
}
const audited = raw('source-and-package-baseline.json');
const packages = raw('package-baseline.json'), installed = raw('install-baseline.json');
for (const result of [audited, packages, installed]) assert.equal(result.source, d.source);
assert.deepEqual(audited.inputs, b.inputs);
assert.equal(audited.selectedResourceCountEach, 6647);
assert.equal(audited.metadataCountEach, 3);
assert.equal(packages.versionCode, 35);
assert.equal(installed.versionCode, 35);
assert.equal(packages.gatewayUrl, '');
assert.equal(packages.artifacts.length, 2);
assert.deepEqual(installed.artifacts, d.artifacts);
for (const artifact of packages.artifacts) {
  const audit = audited.artifacts.find(row => row.variant === artifact.variant);
  const installation = installed.artifacts.find(row => row.variant === artifact.variant);
  assert.equal(audit.sha256, artifact.sha256);
  assert.equal(installation.installedSha256, artifact.sha256);
  assert.equal(audit.nativeElfMatchesThisVariantStrippedOutput, true);
  assert.equal(audit.personalJniDiagnosticCompiled, artifact.variant === 'preview');
  const config = read('raw/' + artifact.variant + '-BuildConfig.java.txt').toString();
  assert(config.includes('VERSION_CODE = 35'));
  assert(config.includes('VERSION_NAME = "0.1.32-gameshop-phone"'));
  assert(config.includes('MIR2_GATEWAY_URL = ""'));
  assert(config.includes('UI_PREVIEW = ' + (artifact.variant === 'preview')));
  assert.match(read('raw/' + artifact.variant + '-badging.txt').toString(), /versionCode='35'/);
  const before = read('raw/' + artifact.variant + '-package-before.txt').toString();
  const after = read('raw/' + artifact.variant + '-package-after.txt').toString();
  assert.match(before, /versionCode=34\b/);
  assert.match(after, /versionCode=35\b/);
  assert.equal(before.match(/firstInstallTime=(.*)/)[1], after.match(/firstInstallTime=(.*)/)[1]);
  assert.equal(installation.upgradePreservedDataFlag, true);
}
assert.deepEqual(raw('original-worktrees-before.json'), raw('original-worktrees-after.json'));
const publication = raw('source-publication-verified.json');
assert.equal(publication.localAndRemoteExact, true);
assert.equal(publication.source, d.source);
assert.equal(publication.pr.headSha, d.source);
assert.equal(publication.pr.draft, true);
assert.equal(publication.windowsRemote, '6ae080711fc7b3aaf06dd9c6bcf63f121682dbe4');
assert.equal(publication.windowsNotPushed, true);
assert.equal(publication.normalNonForcePush, true);
assert.equal(d.screenshots.length, 32);
for (const shot of d.screenshots) {
  assert.equal(shot.inspectedOriginal, true);
  assert.equal(shot.imageEdited, false);
  const png = read(shot.path);
  assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  assert.equal(png.readUInt32BE(16), shot.width);
  assert.equal(png.readUInt32BE(20), shot.height);
  assert(shot.width === 2340 && shot.height === 1080 || shot.width === 1560 && shot.height === 720);
}
const oldImage = 'mir2-web3/docs/generated/player-qa/native-android-gameshop-page-cache-20261003/raw/ui-gameshop-jni/104-tap.png';
assert(read('before/v34-desktop-scaled-gameshop.png').equals(git(['show', b.parent + ':' + oldImage])));
assert(read('raw/device-list.txt').toString().includes('emulator-5554'));
assert.equal(read('raw/device-api.txt').toString().trim(), '31');
assert.equal(read('raw/device-size-before.txt').toString(), read('raw/device-size-after-v35.txt').toString());
assert.equal(read('raw/device-density-before.txt').toString(), read('raw/device-density-after-v35.txt').toString());
assert.equal(read('raw/device-size-after-v35.txt').toString().trim(), 'Physical size: 1080x2340');
assert.equal(read('raw/device-density-after-v35.txt').toString().trim(), 'Physical density: 440');
assert.equal(raw('screen-restore-verified.json').originalSizeRestored, true);

const ui = raw('ui-evidence-audit.json');
assert.equal(ui.source, d.source);
const patterns = {
  gl506: /E emuglGLESv2_enc:.*GL error 0x506/,
  uninitializedColor: /D eglCodecCommon:.*rbo not color renderable\. format: 0x0/,
  fatalOrPanic: /FATAL EXCEPTION|thread '.*' panicked|Fatal signal/,
  javaStart: /I Mir2UiPreview: PERSONAL_JNI_OFFLINE_START/,
  javaSent: /I Mir2UiPreview: PERSONAL_JNI_OFFLINE_SENT/,
  sharedConsumer: /I event .*ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE/,
};
const pidCounts = [];
for (const scene of ui.scenes) {
  const commands = raw('ui-' + scene.scene + '/commands.json');
  assert.equal(commands.source, d.source);
  assert.equal(commands.versionCode, 35);
  assert.equal(commands.pid, scene.pid);
  const union = Object.fromEntries(Object.keys(patterns).map(key => [key, new Map()]));
  for (const snapshot of scene.snapshots) {
    const log = read('raw/ui-' + scene.scene + '/' + snapshot.log).toString();
    const local = Object.fromEntries(Object.keys(patterns).map(key => [key, new Map()]));
    for (const line of log.split('\n')) {
      for (const [key, pattern] of Object.entries(patterns)) {
        if (pattern.test(line)) local[key].set(line, (local[key].get(line) || 0) + 1);
      }
    }
    for (const key of Object.keys(patterns)) {
      assert.equal([...local[key].values()].reduce((n, value) => n + value, 0), snapshot.snapshotCounts[key]);
      for (const [line, count] of local[key]) union[key].set(line, Math.max(count, union[key].get(line) || 0));
    }
    const frame = snapshot.lastCompletePhoneFrame;
    if (frame) {
      const info = log.split('\n').filter(line => line.includes('I event ')).join('\n');
      const actualTexts = new Set([...info.matchAll(/ANDROID_PHONE_GAMESHOP_TEXTtext = ("(?:[^"\\]|\\.)*"); font_dp = ([^;]+); glyphs = (\d+); alpha_masks = (\d+);/g)]
        .map(match => JSON.stringify({text: JSON.parse(match[1]), fontDp: Number(match[2]),
          glyphs: Number(match[3]), alphaMasks: Number(match[4])})));
      const actualControls = new Set([...info.matchAll(/ANDROID_PHONE_GAMESHOP_CONTROLcontrol = (.*); size_dp = Vec2\(([^,]+), ([^)]+)\); center_dp = Vec2\(([^,]+), ([^)]+)\);/g)]
        .map(match => JSON.stringify({control: match[1],
          sizeDp: [Number(match[2]), Number(match[3])],
          centerDp: [Number(match[4]), Number(match[5])]})));
      for (const text of frame.texts) {
        assert(actualTexts.has(JSON.stringify(text)));
        assert(text.fontDp >= 14 - 0.01);
      }
      for (const control of frame.controls) {
        assert(control.sizeDp.every(size => size >= 48 - 0.01));
        assert(actualControls.has(JSON.stringify(control)));
      }
    }
    if (scene.scene === 'normal-intent-isolation') {
      assert(!log.includes('ANDROID_UI_PREVIEW_READY'));
      assert(!log.includes('PERSONAL_JNI_OFFLINE_SENT'));
      assert(!log.includes('ANDROID_PERSONAL_JNI_SHARED_CONSUMER_NOT_LIVE'));
    }
  }
  const counts = Object.fromEntries(Object.entries(union).map(([key, records]) =>
    [key, [...records.values()].reduce((n, count) => n + count, 0)]));
  assert.deepEqual(counts, scene.counts);
  assert.equal(counts.fatalOrPanic, 0);
  assert.equal(counts.javaStart, scene.scene === 'normal-intent-isolation' ? 0 : 1);
  assert.equal(counts.javaSent, counts.javaStart);
  if (scene.scene !== 'normal-intent-isolation') {
    assert(counts.sharedConsumer >= 1);
    assert(commands.commands.some(row => row.args?.includes('KEYCODE_HOME')));
  }
  const expectedSha = packages.artifacts.find(row =>
    row.variant === (scene.scene === 'normal-intent-isolation' ? 'debug' : 'preview')).sha256;
  for (const command of commands.commands) {
    assert.equal(command.exitCode, 0);
    const args = command.args || [];
    assert(!args.includes('wipe-data'));
    assert(!(args.includes('pm') && args.includes('clear')));
    assert(!(args.includes('logcat') && args.includes('-c')));
    if (command.name.endsWith('-installed-sha.txt')) {
      assert.equal(read('raw/ui-' + scene.scene + '/' + command.name).toString().trim().split(/\s+/)[0], expectedSha);
    }
    if (args[0] === 'shell' && args[1] === 'input') assert(['tap', 'swipe', 'keyevent'].includes(args[2]));
    if (args[0] === 'shell' && args[1] === 'wm') {
      assert.equal(args[2], 'size');
      assert(args.length === 3 || ['720x1560', 'reset'].includes(args[3]));
    }
  }
  assert.deepEqual(counts, d.scenes.find(row => row.scene === scene.scene).totals);
  pidCounts.push(scene.pid);
}
assert.equal(new Set(pidCounts).size, 2);
assert.equal(d.totalDistinctPidGl506, 157);
assert.equal(d.totalDistinctPidUninitializedColor, 158);
assert.equal(ui.totalDistinctPidGl506, 157);
assert.equal(ui.observedFatalOrPanic, 0);
const shop = ui.scenes.find(scene => scene.scene === 'gameshop-jni');
const snapshot = name => shop.snapshots.find(row => row.screenshot === name);
const text = name => snapshot(name).lastCompletePhoneFrame.texts.map(row => row.text);
for (let index = 0; index < 13; index++) assert.equal(snapshot((89 + 8 * index) + '-tap.png').page, (index + 2) + ' / 14');
assert.equal(snapshot('193-tap.png').page, '14 / 14');
assert.equal(snapshot('201-tap.png').page, '13 / 14');
assert.equal(snapshot('209-home.png').page, '13 / 14');
assert.equal(snapshot('218-resize.png').page, '13 / 14');
assert.equal(snapshot('228-tap.png').page, '14 / 14');
assert.equal(snapshot('244-resize.png').page, '14 / 14');
assert(text('185-tap.png').some(value => value.includes('STOCK: 3')));
assert(text('8-tap.png').includes('200 Gold / 50 Credits'));
assert(!text('16-swipe.png').includes('Confirm purchase'));
for (const name of ['32-tap.png', '40-tap.png', '48-home.png']) assert(text(name).includes('Confirm purchase'));
assert(!text('57-tap.png').includes('Confirm purchase'));
assert(snapshot('16-swipe.png').lastCompletePhoneFrame.scrollAreas[0].offset[1] > 0);
assert(snapshot('73-swipe.png').lastCompletePhoneFrame.scrollAreas.at(-1).offset[1] > 0);
assert(snapshot('236-tap.png').lastCompletePhoneFrame.scrollAreas[0].offset[1] > 0);
for (const row of shop.snapshots.slice(0, 27)) assert.equal(row.snapshotCounts.gl506, 0);
assert.equal(snapshot('218-resize.png').snapshotCounts.gl506, 64);
assert.equal(snapshot('244-resize.png').snapshotCounts.gl506, 145);
for (const flag of [
  'newNativeApksBuilt', 'actualOfflineJavaJniVerified', 'phoneShop48dpTargetsAccepted',
  'phoneShop14dpTextAccepted', 'actualShopPagingVerified', 'shopFinalPageBoundaryVerified',
  'shopPreviousAndSamePidResumeVerified', 'shopQuantitySharedPromptCancelVerified',
  'swipeDoesNotSubmitPurchaseVerified', 'coveredSharedPromptVerified', 'independentFiltersVerified',
  'normalIntentIsolationVerified', 'compactPagingAndControlsReachableVerified',
  'temporaryScreenOverrideRestored', 'rawLogsLosslesslyCompressed', 'rawLogRingOverwriteHandled',
  'errorsNotSummedAcrossCumulativeSnapshots',
]) assert.equal(d[flag], true, flag);
for (const flag of [
  'wholePhoneUiAccepted', 'wholePhoneShopAccepted', 'allCompactLayoutsAccepted',
  'shopPreviewActualDeviceAccepted', 'shopImeAccepted', 'nineLanguagesAccepted',
  'multiTouchPhysicalAccepted', 'storageInventoryLayoutAccepted',
  'storageThisVersionDeviceRegressionAccepted', 'realAuthenticationAccepted',
  'realNetworkAccepted', 'purchaseAccepted', 'storageTransferAccepted',
  'physicalDeviceAccepted', 'rendererFixAccepted', 'zeroRendererErrorAccepted',
  'fullResourcePackAccepted', 'windowsHostBuildAccepted', 'appDataContentHashed',
  'untrackedRecursivelyHashed', 'wholeGoalComplete',
]) assert.equal(d[flag], false, flag);
console.log(JSON.stringify({mode, source: d.source, payloads: manifest.payloadCount,
  bytes: manifest.totalBytes, losslessGzip: 32, inputs: 50, screenshots: 32,
  gates: 7, distinctPidGl506: 157, goalStatus: 'Active', wholeGoalComplete: false}));
