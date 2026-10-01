// Copy the four frozen Windows originals into a new ignored diagnostic pack.
// No artwork editing, source-pack mutation, publication or APK construction.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const platform = path.dirname(fileURLToPath(import.meta.url));
const [approvedRoot, frozen] = process.argv.slice(2);
assert(approvedRoot && /^[0-9a-f]{40}$/.test(frozen ?? ''),
  'Pass the approved UI root and full frozen Windows commit');
const source = fs.realpathSync(approvedRoot);
const original = path.join(source, 'original-ui');
const manifestPath = path.join(original, 'manifest.generated.json');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const files = root => fs.readdirSync(root, { withFileTypes: true }).flatMap(entry => {
  const name = path.join(root, entry.name);
  assert(!entry.isSymbolicLink(), 'Approved resource pack must have ordinary files, not symlinks');
  return entry.isDirectory() ? files(name) : [name];
});
const prior = files(source).map(name => ({
  relative: path.relative(source, name), sha256: hash(fs.readFileSync(name)),
}));
const manifest = JSON.parse(fs.readFileSync(manifestPath));
assert(!manifest.libraries.UI_32bit && !fs.existsSync(path.join(original, 'UI_32bit')),
  'This supplement requires the unchanged prior pack without UI_32bit');
const gitRoot = execFileSync('git', ['rev-parse', '--show-toplevel'],
  { cwd: platform, encoding: 'utf8' }).trim();
const gitPath = 'mir2-web3/apps/web/public/original-ui/UI_32bit';
const blob = name => execFileSync('git', ['show', `${frozen}:${gitPath}/${name}`],
  { cwd: gitRoot, maxBuffer: 2 * 1024 * 1024 });
const metadataBytes = blob('meta.json');
const metadata = JSON.parse(metadataBytes);
assert.equal(metadata.version, 3);
assert.equal(metadata.count, 474);
assert.deepEqual(metadata.frames.map(frame => frame.index).sort((a, b) => a - b), [470, 471, 472, 473]);
const target = path.join(platform, 'target');
fs.mkdirSync(target, { recursive: true });
const retained = fs.mkdtempSync(path.join(target, 'weight-assets-'));
const output = path.join(retained, 'shared-ui-assets');
fs.cpSync(source, output, { recursive: true, errorOnExist: true, force: false });
const libraryRoot = path.join(output, 'original-ui', 'UI_32bit');
fs.mkdirSync(libraryRoot);
const frames = metadata.frames.map(frame => {
  assert.equal(frame.path, `/original-ui/UI_32bit/${frame.index}.png`);
  const bytes = blob(`${frame.index}.png`);
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
  assert.deepEqual([bytes.readUInt32BE(16), bytes.readUInt32BE(20)], [frame.width, frame.height]);
  fs.writeFileSync(path.join(libraryRoot, `${frame.index}.png`), bytes);
  return { index: frame.index, bytes: bytes.length, sha256: hash(bytes),
    width: frame.width, height: frame.height, x: frame.x, y: frame.y };
});
fs.writeFileSync(path.join(libraryRoot, 'meta.json'), metadataBytes);
manifest.libraries.UI_32bit = metadata;
fs.writeFileSync(path.join(output, 'original-ui', 'manifest.generated.json'),
  JSON.stringify(manifest, null, 2) + '\n');
for (const file of prior) {
  assert.equal(hash(fs.readFileSync(path.join(source, file.relative))), file.sha256,
    `Approved input changed: ${file.relative}`);
  if (file.relative !== path.join('original-ui', 'manifest.generated.json')) {
    assert.equal(hash(fs.readFileSync(path.join(output, file.relative))), file.sha256,
      `Prior copied input changed: ${file.relative}`);
  }
}
const result = { frozenWindows: frozen, approvedRoot: source, output,
  sourceUnchanged: true, priorFiles: prior.length, priorCopiedFilesByteIdentical: prior.length - 1,
  originalManifestSha256: hash(fs.readFileSync(manifestPath)),
  supplementedManifestSha256: hash(fs.readFileSync(path.join(output, 'original-ui', 'manifest.generated.json'))),
  metadataSha256: hash(metadataBytes), libraryExtent: 474, exportedFrames: 4, frames,
  method: 'Original Git blob bytes, not a fresh raw-library export or edited image',
  fullResourceReleaseAccepted: false, authenticatedGameplayAccepted: false, physicalDeviceAccepted: false };
fs.writeFileSync(path.join(retained, 'asset-identity.json'), JSON.stringify(result, null, 2) + '\n');
console.log(JSON.stringify(result, null, 2));
