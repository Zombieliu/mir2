// Exact source/package controls for the phone-only focused chat repair.
// Run from repository root. APKs/unpacked licensed resources stay ignored.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const [mode, qa, resourceRoot] = process.argv.slice(2);
assert(['source', 'package'].includes(mode) && qa, 'source|package and exact QA target required');
const source = 'd009f240247261146c1053b5ec8cdd934724cad8';
const frozen = '3f5e61533235921369bc13a7760b4a56b0e467e5';
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const output = (name, result) => {
  fs.writeFileSync(path.join(qa, name), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
};
if (mode === 'source') {
  const rust = [['chat-after.log', 6], ['normal-final.log', 273], ['preview-final.log', 287]].map(([name, passed]) => {
    const match = read(name).match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/);
    assert(match, name);
    assert.deepEqual(match.slice(1).map(Number), [passed, 0, 0], name);
    return {name, passed, failed: 0, ignored: 0};
  });
  assert.match(read('chat-before.log'), /test result: FAILED\. 2 passed; 4 failed; 0 ignored/);
  assert.match(read('chat-before.log'), /received row 1; expected visible suffix from 0/);
  assert.match(read('chat-before.log'), /received row 7; expected visible suffix from 10/);
  assert.match(read('chat-test-setup-error.log'), /error\[E0451\]/);
  assert.match(read('java-final.log'), /BUILD SUCCESSFUL/);
  assert.match(read('java-final.log'), /42 actionable tasks: 42 executed/);
  const java = ['Debug', 'UiPreview'].map(variant => {
    const directory = `mir2-web3/apps/game-client/platform-android/android/app/build/test-results/test${variant}UnitTest`;
    const files = fs.readdirSync(directory).filter(name => name.endsWith('.xml'));
    assert.equal(files.length, 5);
    const totals = {tests: 0, failures: 0, errors: 0, skipped: 0};
    for (const name of files) {
      const xml = fs.readFileSync(path.join(directory, name), 'utf8');
      const suite = xml.match(/<testsuite[^>]+>/)?.[0];
      assert(suite, name);
      for (const key of Object.keys(totals)) {
        totals[key] += Number(suite.match(new RegExp(`${key}="(\\d+)"`))[1]);
      }
      fs.writeFileSync(path.join(qa, `${variant}-${name}`), xml);
    }
    assert.deepEqual(totals, {tests: 37, failures: 0, errors: 0, skipped: 0});
    return {variant, suites: files.length, ...totals, environment: 'local synthetic TLS/MockWebServer'};
  });
  for (const [name, variant] of [['debug', 'debug'], ['preview', 'uiPreview']]) {
    assert(read(`api31-${name}.log`).includes(`check ${variant} aarch64-linux-android`));
    assert(read(`api31-${name}.log`).includes('target check passed'));
  }
  const files = [
    'platform-android/src/phone_hud.rs', 'platform-android/src/phone_chat_layout_tests.rs',
    'platform-android/android/app/build.gradle', 'platform-android/build-android.sh',
    'platform-android/src/chat_ingress.rs', 'platform-android/src/shared_shell.rs',
    'platform-android/src/ui_preview.rs', 'platform-android/src/phone_panels.rs',
    'platform-android/android/app/src/main/java/com/mir2/web3/GatewaySession.java',
    'client-bevy/src/native_chat_ingress.rs', 'client-bevy/src/crystal_ui/chat.rs',
    'runtime/src/native_ingest.rs',
  ].map(name => {
    const file = `mir2-web3/apps/game-client/${name}`;
    const bytes = fs.readFileSync(file);
    assert(bytes.equals(execFileSync('git', ['show', `${source}:${file}`], {maxBuffer: 4 * 1024 * 1024})), file);
    return {path: file, sha256: hash(bytes)};
  });
  output('source-results-v15.json', {
    source, frozenWindows: frozen, rust, java, sourceFiles: files,
    failureFirst: {compiledBefore: true, passed: 2, failed: 4, fixturePrivateFieldSetupErrorRetainedSeparately: true},
    arm64Api31: {normal: true, actualPreviewFeature: true},
    newTests: {count: 6, densityScaleSourceSizeCombinations: 96, actualSharedChatChildNodes: true},
    review: 'Bounded read-only three-file review found no new P0/P1/P2; did not execute tests/build/device.',
    existingExtremeImeControlOverflowOpen: true, sharedRuntimeChatEvictionUnchangedAndOpen: true,
    fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  });
} else {
  assert(resourceRoot, 'approved resource root required');
  const controls = JSON.parse(read('source-results-v15.json'));
  assert.equal(controls.source, source);
  for (const file of controls.sourceFiles) {
    assert.equal(hash(execFileSync('git', ['show', `${source}:${file.path}`], {maxBuffer: 4 * 1024 * 1024})), file.sha256);
  }
  const artifacts = ['debug', 'preview'].map(variant => {
    for (const edge of ['before', 'after']) {
      assert.equal(read(`${variant}-source-${edge}.txt`).trim(), source);
      assert.equal(read(`${variant}-status-${edge}.txt`), '');
    }
    const config = read(`${variant}-BuildConfig.java.txt`), badging = read(`${variant}-badging.txt`);
    assert.match(config, /VERSION_CODE = 15/);
    assert.match(config, /VERSION_NAME = "0.1.12-chat-layout"/);
    assert.match(config, /MIR2_GATEWAY_URL = ""/);
    assert.match(config, new RegExp(`UI_PREVIEW = ${variant === 'preview'}`));
    assert.match(badging, /versionCode='15'/);
    assert.match(badging, /sdkVersion:'31'/);
    assert.match(badging, /targetSdkVersion:'35'/);
    assert.match(badging, /native-code: 'arm64-v8a'/);
    assert(read(`${variant}-build.log`).includes('package gate passed'));
    const apk = path.join(qa, 'final-apks', `mir2-native-chat-layout-${variant}-v15.apk`);
    const entries = execFileSync('unzip', ['-Z1', apk], {encoding: 'utf8', maxBuffer: 16 * 1024 * 1024}).trim().split('\n');
    const libraries = [['Items', 1003], ['StateItem', 5192], ['MagIcon', 224], ['MagIcon2', 224], ['UI_32bit', 4]];
    const unpack = fs.mkdtempSync(path.join(qa, `${variant}-resource-proof-`));
    execFileSync('unzip', ['-q', apk, ...libraries.map(([name]) => `assets/original-ui/${name}/*`), '-d', unpack]);
    let pngCount = 0;
    const proof = libraries.map(([library, count]) => {
      const pngs = entries.filter(entry => new RegExp(`^assets/original-ui/${library}/[0-9]+\\.png$`).test(entry));
      assert.equal(pngs.length, count);
      assert.equal(new Set(pngs).size, count);
      for (const entry of pngs) {
        const bytes = fs.readFileSync(path.join(unpack, entry));
        assert(bytes.equals(fs.readFileSync(path.join(resourceRoot, entry.slice('assets/'.length)))), entry);
        if (library === 'UI_32bit') {
          assert(bytes.equals(execFileSync('git', ['show', `${frozen}:mir2-web3/apps/web/public/${entry.slice('assets/'.length)}`])), entry);
        }
      }
      let metadataSha256 = null;
      if (['Items', 'StateItem', 'UI_32bit'].includes(library)) {
        const relative = `original-ui/${library}/meta.json`, bytes = fs.readFileSync(path.join(unpack, 'assets', relative));
        assert(bytes.equals(fs.readFileSync(path.join(resourceRoot, relative))));
        if (library === 'UI_32bit') {
          assert(bytes.equals(execFileSync('git', ['show', `${frozen}:mir2-web3/apps/web/public/${relative}`])));
        }
        metadataSha256 = hash(bytes);
      }
      pngCount += count;
      return {library, pngCount: count, allSourceBytesMatch: true, metadataSha256};
    });
    assert.equal(pngCount, 6647);
    const sha256 = hash(fs.readFileSync(apk));
    assert(read(`${variant}-build.log`).includes(sha256));
    return {variant, source, path: apk, bytes: fs.statSync(apk).size, sha256, libraries: proof};
  });
  output('package-v15.json', {
    source, frozenWindows: frozen, versionCode: 15, versionName: '0.1.12-chat-layout',
    cleanBeforeAfterBothBuilds: true, sourceGateHashesMatchCommittedInputs: true,
    gatewayUrl: '', minimumApi: 31, targetApi: 35, abi: 'arm64-v8a', artifacts,
    each6647PngAndThreeMetadataMatchInputs: true, fourWeightFramesMatchFrozenGitBytes: true,
    packageKind: 'Rust release cdylib inside Gradle Debug diagnostic variants, not store Release',
    fullResourceReleaseAccepted: false, fullPhoneUiAccepted: false,
    authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  });
}
