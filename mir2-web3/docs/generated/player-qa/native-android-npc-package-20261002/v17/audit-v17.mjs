// Exact source/package proof only. Offline pictures cannot accept live gameplay.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const [mode, qa, source, resourceRoot] = process.argv.slice(2);
assert(['source', 'package'].includes(mode) && qa && /^[a-f0-9]{40}$/.test(source));
const frozen = '3d735745f1117d42a7859e87604a106351dca935';
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const git = (revision, file) => execFileSync('git', ['show', `${revision}:${file}`], {maxBuffer: 8 * 1024 * 1024});
const output = (name, result) => {
  fs.writeFileSync(path.join(qa, name), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
};
if (mode === 'source') {
  assert.match(read('red-old-manual-shop.log'), /test result: FAILED\. 0 passed; 1 failed; 0 ignored/);
  assert.match(read('red-old-manual-shop.log'), /npcshop must wait for received catalogue/);
  assert.match(read('preview-fixtures.log'), /error\[E0609\]/);
  assert.match(read('preview-fixtures.log'), /error\[E0616\]/);
  const rust = [['android-normal-full.log', 290], ['android-preview-full.log', 307]].map(([name, count]) => {
    const result = read(name).match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/);
    assert(result, name);
    assert.deepEqual(result.slice(1).map(Number), [count, 0, 0]);
    return {name, passed: count, failed: 0, ignored: 0};
  });
  assert.match(read('java-fresh.log'), /BUILD SUCCESSFUL/);
  assert.match(read('java-fresh.log'), /42 actionable tasks: 42 executed/);
  const java = ['Debug', 'UiPreview'].map(variant => {
    const directory = `mir2-web3/apps/game-client/platform-android/android/app/build/test-results/test${variant}UnitTest`;
    const files = fs.readdirSync(directory).filter(name => name.endsWith('.xml'));
    assert.equal(files.length, 5);
    const totals = {tests: 0, failures: 0, errors: 0, skipped: 0};
    const suites = files.map(name => {
      const bytes = fs.readFileSync(path.join(directory, name));
      const tag = bytes.toString().match(/<testsuite[^>]+>/)?.[0];
      assert(tag, name);
      for (const key of Object.keys(totals)) totals[key] += Number(tag.match(new RegExp(`${key}="(\\d+)"`))[1]);
      fs.writeFileSync(path.join(qa, `${variant}-${name}`), bytes);
      return {name, sha256: hash(bytes)};
    });
    assert.deepEqual(totals, {tests: 39, failures: 0, errors: 0, skipped: 0});
    return {variant, suites, ...totals, environment: 'local synthetic TLS/MockWebServer, not live account'};
  });
  for (const variant of ['debug', 'uiPreview']) {
    assert(read(`api31-${variant}.log`).includes(`check ${variant} aarch64-linux-android`));
    assert(read(`api31-${variant}.log`).includes('target check passed'));
  }
  const files = [
    'platform-android/src/ui_preview.rs', 'platform-android/src/npc_ingress.rs',
    'platform-android/src/npc_host_tests.rs', 'platform-android/src/player_ingress.rs',
    'platform-android/src/shared_shell.rs', 'platform-android/src/lib.rs',
    'platform-android/src/item_geometry.rs', 'platform-android/src/phone_hud.rs',
    'platform-android/src/phone_panels.rs', 'platform-android/src/text_input.rs',
    'platform-android/android/app/build.gradle', 'platform-android/build-android.sh',
    'platform-android/android/app/src/main/java/com/mir2/web3/GatewaySession.java',
    'platform-android/android/app/src/main/java/com/mir2/web3/MainActivity.java',
    'platform-android/android/app/src/test/java/com/mir2/web3/GatewaySessionTest.java',
    'client-bevy/src/native_npc_ingress.rs', 'client-bevy/src/native_inventory_ingress.rs',
    'client-bevy/src/crystal_ui/overlays.rs', 'client-bevy/src/shop.rs',
    'client-bevy/src/read_model.rs', 'client-bevy/src/big_map.rs',
    'runtime/src/native_ingest.rs', 'runtime/src/lib.rs', 'platform-android/Cargo.lock',
  ].map(name => {
    const file = `mir2-web3/apps/game-client/${name}`, bytes = fs.readFileSync(file);
    assert(bytes.equals(git(source, file)), file);
    return {path: file, sha256: hash(bytes)};
  });
  output('source-v17.json', {
    source, frozenWindows: frozen, rust, java, sourceFiles: files,
    arm64Api31: {normal: true, actualPreviewFeature: true},
    failureFirst: {compiledOldFixture: true, passed: 0, failed: 1, unrelatedTestFieldSetupCompileErrorPreserved: true},
    nativeNpcImplementation: '4e35d3a34041f47370ba7f1338d4b5e0f8d7c0f8',
    regressionBoundary: 'Production Android NPC projector and exact synthetic FIFO; public shared reducer assertions, not actual JNI, window rendering, network or transactions.',
    sharedAndWindowsRegression: 'Prior 4e35 exact code gates remain separate; wider Windows NPC filter retained 19 pass / 6 fail.',
    fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  });
} else {
  assert(resourceRoot, 'approved resource root required');
  const controls = JSON.parse(read('source-v17.json'));
  assert.equal(controls.source, source);
  for (const file of controls.sourceFiles) assert.equal(hash(git(source, file.path)), file.sha256);
  const artifacts = ['debug', 'preview'].map(variant => {
    for (const edge of ['before', 'after']) {
      assert.equal(read(`${variant}-source-${edge}.txt`).trim(), source);
      assert.equal(read(`${variant}-status-${edge}.txt`), '');
    }
    const config = read(`${variant}-BuildConfig.java.txt`), badging = read(`${variant}-badging.txt`);
    assert.match(config, /VERSION_CODE = 17/);
    assert.match(config, /VERSION_NAME = "0.1.14-npc-ingress"/);
    assert.match(config, /MIR2_GATEWAY_URL = ""/);
    assert.match(config, new RegExp(`UI_PREVIEW = ${variant === 'preview'}`));
    assert.match(badging, /versionCode='17'/);
    assert.match(badging, /sdkVersion:'31'/);
    assert.match(badging, /targetSdkVersion:'35'/);
    assert.match(badging, /native-code: 'arm64-v8a'/);
    assert(read(`${variant}-build.log`).includes('package gate passed'));
    assert.match(read(`${variant}-build.log`), /Finished `release` profile/);
    const apk = path.join(qa, 'final-apks', `mir2-native-npc-ingress-${variant}-v17.apk`);
    const entries = execFileSync('unzip', ['-Z1', apk], {encoding: 'utf8', maxBuffer: 16 * 1024 * 1024}).trim().split('\n');
    assert(entries.includes('lib/arm64-v8a/libmir2_platform_android.so'));
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
    return {variant, source, path: path.resolve(apk), bytes: fs.statSync(apk).size, sha256, libraries: proof};
  });
  output('package-v17.json', {
    source, frozenWindows: frozen, versionCode: 17, versionName: '0.1.14-npc-ingress',
    cleanBeforeAfterBothBuilds: true, sourceGateHashesMatchCommittedInputs: true,
    gatewayUrl: '', minimumApi: 31, targetApi: 35, abi: 'arm64-v8a', artifacts,
    each6647PngAndThreeMetadataMatchInputs: true, fourWeightFramesMatchFrozenGitBytes: true,
    packageKind: 'Rust release cdylib inside Gradle Debug diagnostic variants, not store Release',
    previousV16GlErrorsRetained: true, rendererSourceChangedThisLeaf: false,
    fullResourceReleaseAccepted: false, fullPhoneUiAccepted: false,
    authenticatedGameplayAccepted: false, physicalDeviceAccepted: false,
  });
}
