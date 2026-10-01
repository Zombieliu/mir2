// Exercise the actual Gradle sparse-frame gate. Fixtures stay in ignored target.
// Offline Gradle dependency resolution is not a wrapper network-traffic audit.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const platform = path.dirname(fileURLToPath(import.meta.url));
assert(process.argv[2], 'Pass the approved diagnostic UI root with sparse UI_32bit');
const source = fs.realpathSync(process.argv[2]);
const original = path.join(source, 'original-ui');
const manifest = JSON.parse(fs.readFileSync(path.join(original, 'manifest.generated.json')));
const metadata = JSON.parse(fs.readFileSync(path.join(original, 'UI_32bit', 'meta.json')));
const sourceIdentity = () => {
  const digest = createHash('sha256');
  for (const relative of ['manifest.generated.json', 'UI_32bit/meta.json',
    ...[470, 471, 472, 473].map(index => `UI_32bit/${index}.png`)]) {
    digest.update(`${relative}\0`);
    digest.update(fs.readFileSync(path.join(original, relative)));
  }
  return digest.digest('hex');
};
const before = sourceIdentity();
fs.mkdirSync(path.join(platform, 'target'), { recursive: true });
const retained = fs.mkdtempSync(path.join(platform, 'target', 'weight-frame-gate-'));
const cases = [{ name: 'completeSparse', accepted: true }, ...[
  'missingLibrary', 'missingMetadata', 'invalidMetadata', 'duplicateMetadata',
  'missing471', 'wrongMetadataPath', 'fractionalIndex', 'wrongGeometry',
  'negativeOffset', 'missingPng', 'truncatedPng', 'dimensionMismatch',
  'oversizedHeader', 'wrongSignature',
  'missingManifestEntry', 'duplicateManifest', 'wrongManifestPath',
  'manifestMetadataMismatch', 'manifestCountMismatch',
].map(name => ({ name, accepted: false }))];
const selected = process.argv[3] ? cases.filter(test => test.name === process.argv[3]) : cases;
assert(selected.length, 'Unknown weight gate case');
const results = [];
for (const test of selected) {
  let root = source;
  if (!test.accepted) {
    root = path.join(retained, test.name);
    const out = path.join(root, 'original-ui');
    fs.mkdirSync(out, { recursive: true });
    for (const entry of fs.readdirSync(original, { withFileTypes: true })) {
      if (entry.name === 'manifest.generated.json' || entry.name === 'UI_32bit') continue;
      fs.symlinkSync(path.join(original, entry.name), path.join(out, entry.name));
    }
    const changed = structuredClone(manifest);
    const meta = structuredClone(metadata);
    const library = path.join(out, 'UI_32bit');
    if (test.name !== 'missingLibrary') {
      fs.mkdirSync(library);
      for (const index of [470, 471, 472, 473]) {
        if (test.name === 'missingPng' && index === 471) continue;
        const sourceIndex = test.name === 'dimensionMismatch' && index === 471 ? 472 : index;
        const bytes = test.name === 'truncatedPng' && index === 471
          ? Buffer.from('not a PNG') : fs.readFileSync(path.join(original, 'UI_32bit', `${sourceIndex}.png`));
        if (test.name === 'oversizedHeader' && index === 471) bytes.writeUInt32BE(0x7fffffff, 16);
        if (test.name === 'wrongSignature' && index === 471) bytes[0] = 0;
        fs.writeFileSync(path.join(library, `${index}.png`), bytes);
      }
      const frame = meta.frames.find(frame => frame.index === 471);
      if (test.name === 'duplicateMetadata') meta.frames[1] = { ...meta.frames[0] };
      if (test.name === 'missing471') meta.frames = meta.frames.filter(frame => frame.index !== 471);
      if (test.name === 'wrongMetadataPath') frame.path = '/original-ui/UI_32bit/470.png';
      if (test.name === 'fractionalIndex') frame.index = 471.5;
      if (test.name === 'wrongGeometry') frame.width = 85;
      if (test.name === 'negativeOffset') frame.x = -1;
      if (test.name !== 'missingMetadata') fs.writeFileSync(path.join(library, 'meta.json'),
        test.name === 'invalidMetadata' ? '{broken' : JSON.stringify(meta));
    }
    if (['missingLibrary', 'missingManifestEntry'].includes(test.name)) delete changed.libraries.UI_32bit;
    if (test.name === 'duplicateManifest') changed.libraries.UI_32bit.frames[1] = { ...changed.libraries.UI_32bit.frames[0] };
    if (test.name === 'wrongManifestPath') changed.libraries.UI_32bit.frames[1].path = '/original-ui/UI_32bit/470.png';
    if (test.name === 'manifestMetadataMismatch') changed.libraries.UI_32bit.frames[1].width = 83;
    if (test.name === 'manifestCountMismatch') changed.libraries.UI_32bit.count = 475;
    fs.writeFileSync(path.join(out, 'manifest.generated.json'), JSON.stringify(changed));
  }
  const result = spawnSync(path.join(platform, 'android', 'gradlew'),
    ['--no-daemon', '--offline', 'validateSharedUi'], {
      cwd: path.join(platform, 'android'), encoding: 'utf8', timeout: 60000,
      env: { ...process.env, MIR2_ANDROID_UI_ASSET_ROOT: root },
    });
  const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`;
  fs.writeFileSync(path.join(retained, `${test.name}.log`), output);
  assert(!result.error, result.error?.message);
  assert.equal(result.status === 0, test.accepted, `${test.name}: unexpected gate result\n${output}`);
  if (!test.accepted) assert.match(output, /UI_32bit weight-bar/,
    `${test.name}: expected resource guard, not an environment failure\n${output}`);
  results.push({ name: test.name, accepted: test.accepted, status: result.status });
  console.log(`${test.name}: ${test.accepted ? 'accepted' : 'rejected'} as expected`);
}
assert.equal(sourceIdentity(), before, 'Approved weight assets were changed');
const result = { passed: results.length, total: selected.length, results,
  fixtureRoot: retained, sourceUnchanged: true, sourceIdentity: before,
  apkBuilt: false, gradleOffline: true, gameplayNetworkUsed: false,
  networkTrafficObserved: false };
fs.writeFileSync(path.join(retained, 'results.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result));
