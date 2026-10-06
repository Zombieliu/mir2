// Run from repository root; retain old failures and bind exact v16 inputs/APKs.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const [mode, qa, source, resourceRoot] = process.argv.slice(2);
assert(['source', 'package'].includes(mode) && qa && /^[a-f0-9]{40}$/.test(source), 'source|package, QA target and exact source required');
const frozen = '3f5e61533235921369bc13a7760b4a56b0e467e5';
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const git = (revision, file) => execFileSync('git', ['show', `${revision}:${file}`], {maxBuffer: 4 * 1024 * 1024});
const output = (name, result) => {
  fs.writeFileSync(path.join(qa, name), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
};
if (mode === 'source') {
  const rust = [['editor-after.log', 3], ['normal-final.log', 276], ['preview-final.log', 290]].map(([name, passed]) => {
    const match = read(name).match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/);
    assert(match, name);
    assert.deepEqual(match.slice(1).map(Number), [passed, 0, 0], name);
    return {name, passed, failed: 0, ignored: 0};
  });
  assert.match(read('editor-before.log'), /test result: FAILED\. 0 passed; 3 failed; 0 ignored/);
  for (const test of ['chat_input_press_is_captured_before_shared_children_are_rebuilt', 'phone_shared_chat_input_is_a_host_editor_target_without_changing_text_or_authority', 'same_focused_chat_can_reopen_on_a_new_press_but_held_press_and_received_rows_cannot']) {
    assert.match(read('editor-before.log'), new RegExp(`test shared_shell::chat_editor_tests::${test} \\.\\.\\. FAILED`));
  }
  assert.match(read('editor-test-setup-error.log'), /error\[E04(?:25|33)\]/);
  assert.match(read('java-test-setup-error.log'), /before Gradle executed/);
  assert.match(read('java-final.log'), /BUILD SUCCESSFUL/);
  assert.match(read('java-final.log'), /42 actionable tasks: 42 executed/);
  const java = ['Debug', 'UiPreview'].map(variant => {
    const directory = `mir2-web3/apps/game-client/platform-android/android/app/build/test-results/test${variant}UnitTest`;
    const files = fs.readdirSync(directory).filter(name => name.endsWith('.xml'));
    assert.equal(files.length, 5);
    const totals = {tests: 0, failures: 0, errors: 0, skipped: 0};
    for (const name of files) {
      const bytes = fs.readFileSync(path.join(directory, name));
      const suite = bytes.toString().match(/<testsuite[^>]+>/)?.[0];
      assert(suite, name);
      for (const key of Object.keys(totals)) {
        totals[key] += Number(suite.match(new RegExp(`${key}="(\\d+)"`))[1]);
      }
      fs.writeFileSync(path.join(qa, `${variant}-${name}`), bytes);
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
    'platform-android/src/chat_editor_tests.rs', 'platform-android/src/shared_shell.rs',
    'platform-android/android/app/build.gradle', 'platform-android/build-android.sh',
    'platform-android/src/text_input.rs', 'platform-android/src/ui_preview.rs',
    'platform-android/src/phone_panels.rs',
    'platform-android/android/app/src/main/java/com/mir2/web3/GatewaySession.java',
    'platform-android/android/app/src/main/java/com/mir2/web3/MainActivity.java',
    'client-bevy/src/native_chat_ingress.rs', 'client-bevy/src/crystal_ui/chat.rs',
    'runtime/src/native_ingest.rs',
  ].map(name => {
    const file = `mir2-web3/apps/game-client/${name}`, bytes = fs.readFileSync(file);
    assert(bytes.equals(git(source, file)), file);
    return {path: file, sha256: hash(bytes)};
  });
  output('source-results-v16.json', {
    source, frozenWindows: frozen, rust, java, sourceFiles: files,
    failureFirst: {compiledBefore: true, passed: 0, failed: 3, rustImportAndJavaRedirectSetupErrorsRetainedSeparately: true},
    arm64Api31: {normal: true, actualPreviewFeature: true},
    rootCause: 'Original shared CrystalChatInput lacked Button/NativeTextInputTarget, so a same-focused-field retap after Android Back never entered existing EditorTouch keyboard reactivation.',
    regressionBoundary: 'Injected Interaction / real shared child markers and PreUpdate-before-Update capture; not real picking, Java IME or send/echo acceptance.',
    review: 'Four-file bounded read-only review: no new P0/P1/P2; reviewer did not run tests/build/device.',
    existingExtremeImeControlOverflowOpen: true, previousV15GlErrorsRetained: true,
    fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  });
} else {
  assert(resourceRoot, 'approved resource root required');
  const controls = JSON.parse(read('source-results-v16.json'));
  assert.equal(controls.source, source);
  for (const file of controls.sourceFiles) assert.equal(hash(git(source, file.path)), file.sha256);
  const artifacts = ['debug', 'preview'].map(variant => {
    for (const edge of ['before', 'after']) {
      assert.equal(read(`${variant}-source-${edge}.txt`).trim(), source);
      assert.equal(read(`${variant}-status-${edge}.txt`), '');
    }
    const config = read(`${variant}-BuildConfig.java.txt`), badging = read(`${variant}-badging.txt`);
    assert.match(config, /VERSION_CODE = 16/);
    assert.match(config, /VERSION_NAME = "0.1.13-chat-editor"/);
    assert.match(config, /MIR2_GATEWAY_URL = ""/);
    assert.match(config, new RegExp(`UI_PREVIEW = ${variant === 'preview'}`));
    assert.match(badging, /versionCode='16'/);
    assert.match(badging, /sdkVersion:'31'/);
    assert.match(badging, /targetSdkVersion:'35'/);
    assert.match(badging, /native-code: 'arm64-v8a'/);
    assert(read(`${variant}-build.log`).includes('package gate passed'));
    const apk = path.join(qa, 'final-apks', `mir2-native-chat-editor-${variant}-v16.apk`);
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
        if (library === 'UI_32bit') assert(bytes.equals(git(frozen, `mir2-web3/apps/web/public/${entry.slice('assets/'.length)}`)), entry);
      }
      let metadataSha256 = null;
      if (['Items', 'StateItem', 'UI_32bit'].includes(library)) {
        const relative = `original-ui/${library}/meta.json`, bytes = fs.readFileSync(path.join(unpack, 'assets', relative));
        assert(bytes.equals(fs.readFileSync(path.join(resourceRoot, relative))), relative);
        if (library === 'UI_32bit') assert(bytes.equals(git(frozen, `mir2-web3/apps/web/public/${relative}`)));
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
  output('package-v16.json', {
    source, frozenWindows: frozen, versionCode: 16, versionName: '0.1.13-chat-editor',
    cleanBeforeAfterBothBuilds: true, sourceGateHashesMatchCommittedInputs: true,
    gatewayUrl: '', minimumApi: 31, targetApi: 35, abi: 'arm64-v8a', artifacts,
    each6647PngAndThreeMetadataMatchInputs: true, fourWeightFramesMatchFrozenGitBytes: true,
    packageKind: 'Rust release cdylib inside Gradle Debug diagnostic variants, not store Release',
    previousV15GlErrorsRetained: true, fullResourceReleaseAccepted: false, fullPhoneUiAccepted: false,
    authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  });
}
