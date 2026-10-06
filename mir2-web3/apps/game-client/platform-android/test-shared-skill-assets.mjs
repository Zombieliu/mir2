// Exercise the real Gradle build gate only. No APK/gameplay/asset publication.
// Gradle dependencies must be cached; --offline is not a wrapper traffic trace.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const platform = path.dirname(fileURLToPath(import.meta.url));
const source = path.resolve(process.argv[2] ?? '');
assert(process.argv[2], 'Pass the approved diagnostic shared UI root');
const original = path.join(source, 'original-ui');
const manifest = JSON.parse(fs.readFileSync(path.join(original, 'manifest.generated.json')));
const sourceIdentity = () => {
  const digest = createHash('sha256');
  digest.update(fs.readFileSync(path.join(original, 'manifest.generated.json')));
  for (const library of ['MagIcon', 'MagIcon2']) {
    for (const frame of manifest.libraries[library].frames) {
      digest.update(`${frame.path}\0`);
      digest.update(fs.readFileSync(path.join(source, frame.path.slice(1))));
    }
  }
  return digest.digest('hex');
};
const sourceBefore = sourceIdentity();
fs.mkdirSync(path.join(platform, 'target'), { recursive: true });
const retained = fs.mkdtempSync(path.join(platform, 'target', 'skill-frame-gate-'));
const cases = [{ name: 'complete', accepted: true }];
for (const library of ['MagIcon', 'MagIcon2']) {
  cases.push(...['duplicate', 'missing54', 'wrongPath54'].map(kind => ({
    name: `${kind}-${library}`, accepted: false, library, kind,
  })));
}
const selected = process.argv[3] ? cases.filter(test => test.name === process.argv[3]) : cases;
assert(selected.length > 0, 'Unknown gate case');
let passed = 0;
for (const test of selected) {
  let root = source;
  if (!test.accepted) {
    root = path.join(retained, test.name);
    const out = path.join(root, 'original-ui');
    fs.mkdirSync(out, { recursive: true });
    for (const entry of fs.readdirSync(original, { withFileTypes: true })) {
      if (entry.name === 'manifest.generated.json' || entry.name === test.library) continue;
      fs.symlinkSync(path.join(original, entry.name), path.join(out, entry.name));
    }
    const changed = structuredClone(manifest);
    const frames = changed.libraries[test.library].frames;
    const icon0 = frames.find(frame => frame.index === 0);
    assert(icon0, 'Source must include frame0');
    fs.mkdirSync(path.join(out, test.library));
    for (const frame of frames) {
      fs.symlinkSync(path.join(source, frame.path.slice(1)), path.join(root, frame.path.slice(1)));
    }
    if (test.kind === 'duplicate') {
      changed.libraries[test.library].frames = Array.from({ length: 224 }, () => ({ ...icon0 }));
    } else {
      const index = frames.findIndex(frame => frame.index === 54);
      assert(index >= 0, 'Source must include frame54');
      if (test.kind === 'missing54') {
        frames[index] = { ...icon0, index: 224, path: `/original-ui/${test.library}/224.png` };
        fs.symlinkSync(path.join(source, icon0.path.slice(1)), path.join(out, test.library, '224.png'));
      } else {
        frames[index] = { ...frames[index], path: icon0.path };
      }
    }
    fs.writeFileSync(path.join(out, 'manifest.generated.json'), JSON.stringify(changed));
  }
  const result = spawnSync(path.join(platform, 'android', 'gradlew'),
    ['--no-daemon', '--offline', 'validateSharedUi'], {
      cwd: path.join(platform, 'android'), encoding: 'utf8',
      env: { ...process.env, MIR2_ANDROID_UI_ASSET_ROOT: root },
    });
  const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
  assert(!result.error, result.error?.message);
  assert.equal(result.status === 0, test.accepted, `${test.name}: unexpected gate result\n${output}`);
  if (!test.accepted) {
    assert.match(output, /magic-icon frame coverage|magic-icon frame path/,
      `${test.name}: failure must be the coverage guard, not an environment error\n${output}`);
  }
  console.log(`${test.name}: ${test.accepted ? 'accepted' : 'rejected'} as expected`);
  passed++;
}
assert.equal(sourceIdentity(), sourceBefore, 'Approved manifest/icons were modified');
console.log(JSON.stringify({ passed, total: selected.length, fixtureRoot: retained,
  sourceUnchanged: true, sourceIdentity: sourceBefore, apkBuilt: false,
  gradleOffline: true, gameplayNetworkUsed: false, networkTrafficObserved: false }));
