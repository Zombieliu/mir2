// Offline artifact audit. Writes only generated output under the supplied QA directory.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import assert from 'node:assert/strict';
const [qa, sourceRoot] = process.argv.slice(2);
assert(qa && sourceRoot, 'provide exact QA and approved source directories');
const source = '6fe6a17ac229e9ebf0181a1ce02062ae00db71f4';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const artifacts = [];
for (const variant of ['debug', 'preview']) {
  for (const edge of ['before', 'after']) {
    assert.equal(fs.readFileSync(path.join(qa, `${variant}-source-${edge}.txt`), 'utf8').trim(), source);
    assert.equal(fs.readFileSync(path.join(qa, `${variant}-status-${edge}.txt`), 'utf8'), '');
  }
  const apk = path.join(qa, 'final-apks', `mir2-native-phone-panels-${variant}-v11.apk`);
  const config = fs.readFileSync(path.join(qa, `${variant}-BuildConfig.java.txt`), 'utf8');
  assert.match(config, /VERSION_CODE = 11/);
  assert.match(config, /VERSION_NAME = "0.1.8-phone-panels"/);
  assert.match(config, /MIR2_GATEWAY_URL = ""/);
  assert.match(config, new RegExp(`UI_PREVIEW = ${variant === 'preview'}`));
  const badging = fs.readFileSync(path.join(qa, `${variant}-badging.txt`), 'utf8');
  assert.match(badging, /versionCode='11'/);
  assert.match(badging, /sdkVersion:'31'/);
  assert.match(badging, /targetSdkVersion:'35'/);
  assert.match(badging, /native-code: 'arm64-v8a'/);
  const entries = execFileSync('unzip', ['-Z1', apk], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }).trim().split('\n');
  const unpacked = fs.mkdtempSync(path.join(qa, `${variant}-item-icons-`));
  execFileSync('unzip', ['-q', apk, 'assets/original-ui/Items/*', 'assets/original-ui/StateItem/*', 'assets/original-ui/MagIcon/*', 'assets/original-ui/MagIcon2/*', '-d', unpacked]);
  const libraries = [];
  for (const [library, count] of [['Items', 1003], ['StateItem', 5192], ['MagIcon', 224], ['MagIcon2', 224]]) {
    const pngs = entries.filter(entry => new RegExp(`^assets/original-ui/${library}/[0-9]+\\.png$`).test(entry));
    assert.equal(pngs.length, count);
    assert.equal(new Set(pngs).size, count);
    for (const entry of pngs) {
      assert(fs.readFileSync(path.join(unpacked, entry)).equals(fs.readFileSync(path.join(sourceRoot, entry.slice('assets/'.length)))), entry);
    }
    let metadataSha256 = null;
    if (library === 'Items' || library === 'StateItem') {
      const relative = `original-ui/${library}/meta.json`;
      const actual = fs.readFileSync(path.join(unpacked, 'assets', relative));
      assert(actual.equals(fs.readFileSync(path.join(sourceRoot, relative))));
      metadataSha256 = hash(actual);
    }
    libraries.push({ library, pngCount: count, allBytesMatch: true, metadataSha256 });
  }
  artifacts.push({ variant, source, path: apk, bytes: fs.statSync(apk).size, sha256: hash(fs.readFileSync(apk)), libraries });
}
const result = { source, cleanBeforeAfterBothBuilds: true, versionCode: 11, versionName: '0.1.8-phone-panels', gatewayUrl: '', minimumAndroidApi: 31, targetAndroidApi: 35, abi: 'arm64-v8a', artifacts, fullResourceReleaseAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false };
fs.writeFileSync(path.join(qa, 'package-v11.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result, null, 2));
