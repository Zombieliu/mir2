import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
const [mode, source] = process.argv.slice(2);
assert(['source', 'package'].includes(mode));
assert.match(source ?? '', /^[a-f0-9]{40}$/);
const base = 'mir2-web3/apps/game-client/platform-android/target/npc-touch-v19-20261002';
const failed = 'mir2-web3/apps/game-client/platform-android/target/npc-touch-v18-20261002';
const frozen = '3d735745f1117d42a7859e87604a106351dca935';
const uiRoot = 'mir2-web3/apps/game-client/platform-android/target/weight-assets-KtITw2/shared-ui-assets';
const read = name => fs.readFileSync(path.join(base, name), 'utf8');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const git = (revision, file) => execFileSync('git', ['show', `${revision}:${file}`], {maxBuffer: 8 * 1024 * 1024});
const output = (name, result) => {
  fs.writeFileSync(path.join(base, name), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
};
execFileSync('git', ['merge-base', '--is-ancestor', frozen, source]);
if (mode === 'source') {
  for (const name of ['missing-close-red.log', 'missing-item-frame-exact-red.log', 'hidden-target-red.log']) {
    assert.match(fs.readFileSync(path.join(failed, name), 'utf8'), /test result: FAILED\. 0 passed; 1 failed; 0 ignored/);
  }
  assert.match(fs.readFileSync(path.join(failed, 'npc-focused-green.log'), 'utf8'), /error\[E0369\]/);
  const rust = [['android-normal-final.log', 296], ['android-preview-final.log', 315]].map(([name, count]) => {
    const matches = read(`final-source/${name}`).match(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored/);
    assert(matches, name);
    assert.deepEqual(matches.slice(1).map(Number), [count, 0, 0]);
    return {name: `final-source/${name}`, passed: count, failed: 0, ignored: 0};
  });
  assert.match(read('final-source/java-fresh.log'), /42 actionable tasks: 42 executed/);
  const java = ['Debug', 'UiPreview'].map(variant => {
    const dir = `mir2-web3/apps/game-client/platform-android/android/app/build/test-results/test${variant}UnitTest`;
    const files = fs.readdirSync(dir).filter(name => name.endsWith('.xml'));
    assert.equal(files.length, 5);
    const totals = {tests: 0, failures: 0, errors: 0, skipped: 0};
    const suites = files.map(name => {
      const bytes = fs.readFileSync(path.join(dir, name));
      const tag = bytes.toString().match(/<testsuite[^>]+>/)?.[0];
      assert(tag);
      for (const key of Object.keys(totals)) totals[key] += Number(tag.match(new RegExp(`${key}="(\\d+)"`))[1]);
      fs.writeFileSync(path.join(base, 'final-source', `${variant}-${name}`), bytes);
      return {name, sha256: hash(bytes)};
    });
    assert.deepEqual(totals, {tests: 39, failures: 0, errors: 0, skipped: 0});
    return {variant, ...totals, suites, liveNetworkAccepted: false};
  });
  for (const variant of ['debug', 'uiPreview']) assert.match(read(`final-source/api31-${variant}.log`), /target check passed/);
  const previous = JSON.parse(fs.readFileSync('mir2-web3/docs/generated/player-qa/native-android-npc-package-20261002/v18/source-v18.json'));
  const files = [...previous.sourceFiles.map(f => f.path), ...[
    'platform-android/src/mobile_ui.rs', 'client-bevy/src/crystal_ui/widget.rs',
    'client-bevy/src/crystal_ui/npc_item_service.rs', 'client-bevy/src/crystal_ui/panel_layouts.rs',
  ].map(p => `mir2-web3/apps/game-client/${p}`)];
  assert.equal(new Set(files).size, 28);
  const sourceFiles = files.map(file => {
    const bytes = fs.readFileSync(file);
    assert(bytes.equals(git(source, file)), file);
    return {path: file, sha256: hash(bytes)};
  });
  output('source-v19.json', {source, frozenWindows: frozen, rust, java, sourceFiles,
    compiledRedRegressions: 3, setupCompileErrorRetainedSeparately: true,
    intermediate296Predecessor: '295 normal/314 preview before visibility-parent guard; retained, not rebound',
    api31BothVariants: true, sharedAndWindowsSourceChangedThisLeaf: false,
    readOnlyReview: 'Reported P2 reproduced and fixed; no remaining P0/P1/P2 in the three-file scope; reviewer ran no tests/build/ADB',
    fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false});
} else {
  const controls = JSON.parse(read('source-v19.json'));
  assert.equal(controls.source, source);
  for (const file of controls.sourceFiles) assert.equal(hash(git(source, file.path)), file.sha256);
  const artifacts = ['debug', 'preview'].map(variant => {
    for (const edge of ['before', 'after']) {
      assert.equal(read(`${variant}-source-${edge}.txt`).trim(), source);
      assert.equal(read(`${variant}-status-${edge}.txt`), '');
    }
    const config = read(`${variant}-BuildConfig.java.txt`), badging = read(`${variant}-badging.txt`);
    assert.match(config, /VERSION_CODE = 19/);
    assert.match(config, /VERSION_NAME = "0.1.16-npc-touch"/);
    assert.match(config, /MIR2_GATEWAY_URL = ""/);
    assert.match(config, new RegExp(`UI_PREVIEW = ${variant === 'preview'}`));
    assert.match(badging, /versionCode='19'/);
    assert.match(badging, /sdkVersion:'31'/);
    assert.match(badging, /targetSdkVersion:'35'/);
    assert.match(badging, /native-code: 'arm64-v8a'/);
    assert.match(read(`${variant}-build.log`), /package gate passed/);
    assert.match(read(`${variant}-build.log`), /Finished `release` profile/);
    const apk = path.join(base, 'final-apks', `mir2-native-npc-touch-${variant}-v19.apk`);
    const entries = execFileSync('unzip', ['-Z1', apk], {encoding: 'utf8', maxBuffer: 16 * 1024 * 1024}).trim().split('\n');
    assert(entries.includes('lib/arm64-v8a/libmir2_platform_android.so'));
    const libs = [['Items', 1003], ['StateItem', 5192], ['MagIcon', 224], ['MagIcon2', 224], ['UI_32bit', 4]];
    const unpack = fs.mkdtempSync(path.join(base, `${variant}-resource-proof-`));
    execFileSync('unzip', ['-q', apk, ...libs.map(([name]) => `assets/original-ui/${name}/*`), '-d', unpack]);
    const libraries = libs.map(([library, count]) => {
      const images = entries.filter(e => new RegExp(`^assets/original-ui/${library}/[0-9]+\\.png$`).test(e));
      assert.equal(images.length, count);
      assert.equal(new Set(images).size, count);
      for (const entry of images) {
        const bytes = fs.readFileSync(path.join(unpack, entry));
        assert(bytes.equals(fs.readFileSync(path.join(uiRoot, entry.slice('assets/'.length)))), entry);
        if (library === 'UI_32bit') assert(bytes.equals(git(frozen, `mir2-web3/apps/web/public/${entry.slice('assets/'.length)}`)), entry);
      }
      let metadataSha256 = null;
      if (['Items', 'StateItem', 'UI_32bit'].includes(library)) {
        const p = `original-ui/${library}/meta.json`, bytes = fs.readFileSync(path.join(unpack, 'assets', p));
        assert(bytes.equals(fs.readFileSync(path.join(uiRoot, p))), p);
        if (library === 'UI_32bit') assert(bytes.equals(git(frozen, `mir2-web3/apps/web/public/${p}`)));
        metadataSha256 = hash(bytes);
      }
      return {library, pngCount: count, allSourceBytesMatch: true, metadataSha256};
    });
    assert.equal(libraries.reduce((n, l) => n + l.pngCount, 0), 6647);
    const meta = JSON.parse(fs.readFileSync(path.join(unpack, 'assets/original-ui/Items/meta.json')));
    const frame = meta.frames.find(f => f.index === 7);
    assert(frame && frame.width === 36 && frame.height === 26);
    const item = fs.readFileSync(path.join(unpack, 'assets/original-ui/Items/7.png'));
    assert.equal(hash(item), '60f5e8a61589c2a4b5c3cdcf64e93a1101110435333cf2e5484da056f0a7ae6a');
    const sha256 = hash(fs.readFileSync(apk));
    assert(read(`${variant}-build.log`).includes(sha256));
    return {variant, source, path: path.resolve(apk), bytes: fs.statSync(apk).size, sha256,
      libraries, offlineItem7: {width: frame.width, height: frame.height, pngSha256: hash(item)}};
  });
  output('package-v19.json', {source, frozenWindows: frozen, versionCode: 19, versionName: '0.1.16-npc-touch',
    cleanBeforeAfterBothBuilds: true, sourceGateHashesMatchCommittedInputs: true,
    gatewayUrl: '', minimumApi: 31, targetApi: 35, abi: 'arm64-v8a', artifacts,
    each6647PngAndThreeMetadataMatchInputs: true, rendererSourceChangedThisLeaf: false,
    kind: 'Rust release cdylib inside Gradle Debug diagnostic variants; not store Release',
    previousFailedFramesAndLogsRetained: true, fullResourceReleaseAccepted: false,
    fullPhoneUiAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false});
}
