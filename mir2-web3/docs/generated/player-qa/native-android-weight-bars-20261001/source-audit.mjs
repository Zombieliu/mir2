// Actual staged bytes and retained source gates, not APK or gameplay acceptance.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
const [qa, platform, identityPath] = process.argv.slice(2);
assert(qa && platform && identityPath, 'Provide QA, Android platform and supplement identity paths');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const identityBytes = fs.readFileSync(identityPath);
const identity = JSON.parse(identityBytes);
assert.equal(identity.frozenWindows, '3f5e61533235921369bc13a7760b4a56b0e467e5');
assert.equal(identity.sourceUnchanged, true);
assert.equal(identity.originalManifestSha256, 'e14940711b944e54baa41254fca3f4b8e0ca0ad3e5d92e76e12a366823ca7735');
assert.equal(identity.exportedFrames, 4);
fs.writeFileSync(path.join(qa, 'asset-identity.json'), identityBytes);
const read = name => fs.readFileSync(path.join(qa, name), 'utf8');
const controls = ['guard-final.log', 'item-adjacent.log', 'skill-adjacent.log'].map((name, index) => {
  const text = read(name);
  const value = JSON.parse(text.trim().split('\n').at(-1));
  assert.equal(value.passed, [20, 13, 7][index]);
  assert.equal(value.total, value.passed);
  assert.equal(value.sourceUnchanged, true);
  assert.equal(value.gradleOffline, true);
  assert.equal(value.networkTrafficObserved, false);
  assert.equal(value.apkBuilt, false);
  return { name, ...value };
});
assert.match(read('guard-before.log'), /missingLibrary: unexpected gate result/);
assert.match(read('guard-before.log'), /BUILD SUCCESSFUL/);
assert.match(read('java-and-stage.log'), /BUILD SUCCESSFUL/);
assert.match(read('java-and-stage.log'), /44 actionable tasks: 44 executed/);
const java = ['Debug', 'UiPreview'].map(variant => {
  const input = path.join(platform, 'android/app/build/test-results', `test${variant}UnitTest`);
  const names = fs.readdirSync(input).filter(name => name.endsWith('.xml'));
  assert.equal(names.length, 5);
  const totals = { tests: 0, failures: 0, errors: 0, skipped: 0 };
  for (const name of names) {
    const bytes = fs.readFileSync(path.join(input, name));
    const header = bytes.toString().match(/<testsuite\b[^>]*>/)?.[0];
    assert(header);
    for (const key of Object.keys(totals)) {
      const value = header.match(new RegExp(`\\b${key}="(\\d+)"`));
      assert(value, key);
      totals[key] += Number(value[1]);
    }
    fs.writeFileSync(path.join(qa, `${variant}-${name}`), bytes);
  }
  assert.deepEqual(totals, { tests: 36, failures: 0, errors: 0, skipped: 0 });
  return { variant, suites: names.length, ...totals };
});
const output = path.join(platform, 'android/app/build/generated/shared-ui-assets');
const staged = [];
for (const [library, count] of [['Items', 1003], ['StateItem', 5192], ['MagIcon', 224], ['MagIcon2', 224], ['UI_32bit', 4]]) {
  const directory = path.join(output, 'original-ui', library);
  const names = fs.readdirSync(directory).filter(name => /^\d+\.png$/.test(name));
  assert.equal(names.length, count);
  for (const name of names) assert(fs.readFileSync(path.join(directory, name)).equals(
    fs.readFileSync(path.join(identity.output, 'original-ui', library, name))), `${library}/${name}`);
  const hasMetadata = ['Items', 'StateItem', 'UI_32bit'].includes(library);
  if (hasMetadata) assert(fs.readFileSync(path.join(directory, 'meta.json')).equals(
    fs.readFileSync(path.join(identity.output, 'original-ui', library, 'meta.json'))), `${library}/meta.json`);
  staged.push({ library, pngCount: count, allBytesMatch: true,
    metadataSha256: hasMetadata ? hash(fs.readFileSync(path.join(directory, 'meta.json'))) : null });
}
const sourceFiles = ['android/app/build.gradle', 'stage-shared-weight-assets.mjs', 'test-shared-weight-assets.mjs']
  .map(name => ({ name, sha256: hash(fs.readFileSync(path.join(platform, name))) }));
const result = { parentSource: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: platform, encoding: 'utf8' }).trim(),
  sourceFiles, controls, java, staged, stagedPngCount: 6647,
  frozenWindows: identity.frozenWindows, priorPackUnchanged: identity.sourceUnchanged,
  sourceOnlyCheckpoint: true, apkBuilt: false, emulatorImageAccepted: false,
  fullResourceReleaseAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false };
fs.writeFileSync(path.join(qa, 'source-results.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result, null, 2));
