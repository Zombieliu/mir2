// Exercise real Gradle item-metadata gates; preserve approved source bytes.
// --offline controls Gradle dependencies, not a wrapper network traffic trace.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const platform = path.dirname(fileURLToPath(import.meta.url));
assert(process.argv[2], 'Pass the approved diagnostic shared UI root');
const source = path.resolve(process.argv[2]);
const original = path.join(source, 'original-ui');
const libraries = ['Items', 'StateItem'];
const metadata = Object.fromEntries(libraries.map(library => [library,
  JSON.parse(fs.readFileSync(path.join(original, library, 'meta.json')))]));
const sourceIdentity = () => {
  const hash = createHash('sha256');
  for (const library of libraries) {
    hash.update(`${library}\0`);
    hash.update(fs.readFileSync(path.join(original, library, 'meta.json')));
    for (const frame of metadata[library].frames) {
      hash.update(`${frame.path}\0`);
      hash.update(fs.readFileSync(path.join(source, frame.path.slice(1))));
    }
  }
  return hash.digest('hex');
};
const sourceBefore = sourceIdentity();
fs.mkdirSync(path.join(platform, 'target'), { recursive: true });
const retained = fs.mkdtempSync(path.join(platform, 'target', 'item-geometry-gate-'));
const cases = [{ name: 'complete', accepted: true }];
for (const library of libraries) {
  for (const kind of ['missingMeta', 'duplicate', 'wrongPath', 'invalidSize', 'invalidOffset', 'missingFrame']) {
    cases.push({ name: `${kind}-${library}`, library, kind, accepted: false });
  }
}
const selected = process.argv[3] ? cases.filter(test => test.name === process.argv[3]) : cases;
assert(selected.length, 'Unknown gate case');
let passed = 0;
for (const test of selected) {
  let root = source;
  if (!test.accepted) {
    root = path.join(retained, test.name);
    const out = path.join(root, 'original-ui');
    fs.mkdirSync(out, { recursive: true });
    for (const entry of fs.readdirSync(original, { withFileTypes: true })) {
      if (entry.name === test.library) continue;
      fs.symlinkSync(path.join(original, entry.name), path.join(out, entry.name));
    }
    const changed = structuredClone(metadata[test.library]);
    const changedDir = path.join(out, test.library);
    fs.mkdirSync(changedDir);
    // Missing PNG must fail metadata coverage before the older generic count gate.
    const index = changed.frames.findIndex(frame => frame.width > 0 && frame.height > 0);
    assert(index >= 0, 'Approved source needs a nonempty bitmap');
    for (const entry of fs.readdirSync(path.join(original, test.library))) {
      if (entry === 'meta.json' || (test.kind === 'missingFrame'
          && entry === `${changed.frames[index].index}.png`)) continue;
      fs.symlinkSync(path.join(original, test.library, entry), path.join(changedDir, entry));
    }
    if (test.kind === 'duplicate') changed.frames.push({ ...changed.frames[index] });
    if (test.kind === 'wrongPath') changed.frames[index].path = `/original-ui/${test.library}/wrong.png`;
    if (test.kind === 'invalidSize') changed.frames[index].width = -1;
    if (test.kind === 'invalidOffset') changed.frames[index].x = '75';
    if (test.kind !== 'missingMeta') {
      fs.writeFileSync(path.join(changedDir, 'meta.json'), JSON.stringify(changed));
    }
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
    assert.match(output, /item geometry metadata|item geometry frame/,
      `${test.name}: failure must be item geometry, not an environment error\n${output}`);
  }
  console.log(`${test.name}: ${test.accepted ? 'accepted' : 'rejected'} as expected`);
  passed++;
}
assert.equal(sourceIdentity(), sourceBefore, 'Approved metadata/images were modified');
console.log(JSON.stringify({ passed, total: selected.length, fixtureRoot: retained,
  sourceUnchanged: true, sourceIdentity: sourceBefore, apkBuilt: false,
  gradleOffline: true, gameplayNetworkUsed: false, networkTrafficObserved: false }));
