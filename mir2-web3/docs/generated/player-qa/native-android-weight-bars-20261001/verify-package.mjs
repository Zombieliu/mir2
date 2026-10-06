// Actual APK byte audit. Unpacked licensed inputs stay in ignored QA target.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const [qa, sourceRoot, source] = process.argv.slice(2);
assert(qa && sourceRoot && /^[0-9a-f]{40}$/.test(source ?? ''), 'Provide QA, approved resource root and exact source');
assert.equal(source, 'cfac6ecd4bb66ac7c4b0a2625ff178f1703335a0');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const repo = execFileSync('git', ['rev-parse', '--show-toplevel'], { encoding: 'utf8' }).trim();
const checkpoint = JSON.parse(fs.readFileSync(path.join(qa, 'source-results.json')));
for (const file of checkpoint.sourceFiles) {
  const bytes = execFileSync('git', ['show', `${source}:mir2-web3/apps/game-client/platform-android/${file.name}`],
    { cwd: repo, maxBuffer: 2 * 1024 * 1024 });
  assert.equal(hash(bytes), file.sha256, `Source gate drift: ${file.name}`);
}
const frozen = '3f5e61533235921369bc13a7760b4a56b0e467e5';
const original = name => execFileSync('git', ['show', `${frozen}:mir2-web3/apps/web/public/original-ui/UI_32bit/${name}`],
  { cwd: repo, maxBuffer: 2 * 1024 * 1024 });
const artifacts = [];
for (const variant of ['debug', 'preview']) {
  for (const edge of ['before', 'after']) {
    assert.equal(fs.readFileSync(path.join(qa, `${variant}-source-${edge}.txt`), 'utf8').trim(), source);
    assert.equal(fs.readFileSync(path.join(qa, `${variant}-status-${edge}.txt`), 'utf8'), '');
  }
  const apk = path.join(qa, 'final-apks', `mir2-native-weight-bars-${variant}-v13.apk`);
  const config = fs.readFileSync(path.join(qa, `${variant}-BuildConfig.java.txt`), 'utf8');
  assert.match(config, /VERSION_CODE = 13/);
  assert.match(config, /VERSION_NAME = "0.1.10-weight-bars"/);
  assert.match(config, /MIR2_GATEWAY_URL = ""/);
  assert.match(config, new RegExp(`UI_PREVIEW = ${variant === 'preview'}`));
  const badging = fs.readFileSync(path.join(qa, `${variant}-badging.txt`), 'utf8');
  assert.match(badging, /versionCode='13'/);
  assert.match(badging, /sdkVersion:'31'/);
  assert.match(badging, /targetSdkVersion:'35'/);
  assert.match(badging, /native-code: 'arm64-v8a'/);
  const entries = execFileSync('unzip', ['-Z1', apk], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
    .trim().split('\n');
  const unpacked = fs.mkdtempSync(path.join(qa, `${variant}-resource-proof-`));
  const selected = ['Items', 'StateItem', 'MagIcon', 'MagIcon2', 'UI_32bit'];
  execFileSync('unzip', ['-q', apk, ...selected.map(library => `assets/original-ui/${library}/*`), '-d', unpacked]);
  const libraries = [];
  for (const [library, count] of [['Items', 1003], ['StateItem', 5192], ['MagIcon', 224], ['MagIcon2', 224], ['UI_32bit', 4]]) {
    const pngs = entries.filter(entry => new RegExp(`^assets/original-ui/${library}/[0-9]+\\.png$`).test(entry));
    assert.equal(pngs.length, count);
    assert.equal(new Set(pngs).size, count);
    for (const entry of pngs) {
      const bytes = fs.readFileSync(path.join(unpacked, entry));
      assert(bytes.equals(fs.readFileSync(path.join(sourceRoot, entry.slice('assets/'.length)))), entry);
      if (library === 'UI_32bit') assert(bytes.equals(original(path.basename(entry))), `Frozen bytes: ${entry}`);
    }
    if (library === 'UI_32bit') assert.deepEqual(pngs.map(entry => Number(path.basename(entry, '.png')))
      .sort((a, b) => a - b), [470, 471, 472, 473]);
    let metadataSha256 = null;
    if (['Items', 'StateItem', 'UI_32bit'].includes(library)) {
      const relative = `original-ui/${library}/meta.json`;
      const actual = fs.readFileSync(path.join(unpacked, 'assets', relative));
      assert(actual.equals(fs.readFileSync(path.join(sourceRoot, relative))));
      if (library === 'UI_32bit') assert(actual.equals(original('meta.json')));
      metadataSha256 = hash(actual);
    }
    libraries.push({ library, pngCount: count, allBytesMatch: true, metadataSha256 });
  }
  artifacts.push({ variant, source, path: apk, bytes: fs.statSync(apk).size,
    sha256: hash(fs.readFileSync(apk)), pngCountCompared: 6647, libraries });
}
const result = { source, frozenWindows: frozen, sourceGateHashesMatchCommittedBuildInputs: true,
  cleanBeforeAfterBothBuilds: true, versionCode: 13, versionName: '0.1.10-weight-bars', gatewayUrl: '',
  minimumAndroidApi: 31, targetAndroidApi: 35, abi: 'arm64-v8a', artifacts,
  fourWeightPngsAndMetadataMatchFrozenGitBytesEachApk: true,
  fullResourceReleaseAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false };
fs.writeFileSync(path.join(qa, 'package-v13.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result, null, 2));
