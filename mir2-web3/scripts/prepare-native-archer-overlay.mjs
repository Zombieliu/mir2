#!/usr/bin/env node
// Local development asset overlay. The installed base is never modified.
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const args = process.argv.slice(2);
assert(args.length === 4, 'Usage: node prepare-native-archer-overlay.mjs --base-root BASE --output NEW_DIRECTORY');
const options = new Map();
for (let i = 0; i < args.length; i += 2) {
  assert(['--base-root', '--output'].includes(args[i]) && !options.has(args[i]), 'Unknown or duplicate option');
  options.set(args[i], args[i + 1]);
}
assert(options.size === 2 && options.get('--base-root') && options.get('--output'), 'Both paths are required');
const project = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const sourceRoot = fs.realpathSync(path.join(project, 'apps/web/public'));
const baseRoot = fs.realpathSync(options.get('--base-root'));
const requestedOutput = path.resolve(options.get('--output'));
// Resolve the existing parent before creating anything, including aliases.
const output = path.join(fs.realpathSync(path.dirname(requestedOutput)), path.basename(requestedOutput));
const within = (parent, child) => {
  const relative = path.relative(parent, child);
  return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative));
};
assert(!fs.existsSync(output), 'Output already exists; preserve previous results');
assert(!within(baseRoot, output) && !within(sourceRoot, output), 'Output must be separate from base/source assets');
assert(!fs.existsSync(path.join(baseRoot, 'archer-overlay-receipt.json')), 'Use the original installed base, not another overlay');
const readJson = p => JSON.parse(fs.readFileSync(p, 'utf8').replace(/^\uFEFF/, ''));
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const shaFile = p => sha(fs.readFileSync(p));
const jsonSha = value => sha(JSON.stringify(value));
const magicRelative = 'original-effects/Magic3';
const manifestRelative = 'original-effects/effects.generated.json';
const metaRelative = `${magicRelative}/meta.json`;
const baseMeta = readJson(path.join(baseRoot, metaRelative));
const baseManifest = readJson(path.join(baseRoot, manifestRelative));
const sourceMeta = readJson(path.join(sourceRoot, metaRelative));
const sourceManifest = readJson(path.join(sourceRoot, manifestRelative));
const names = ['ArcherFocusProc', 'ArcherNormalArrow'];
const indices = [
  ...Array.from({ length: 16 }, (_, direction) => Array.from({ length: 5 }, (_, offset) => 1030 + direction * 10 + offset)).flat(),
  ...Array.from({ length: 10 }, (_, offset) => 2730 + offset),
];
assert(baseManifest.schemaVersion === sourceManifest.schemaVersion, 'Effect schema differs');
assert(baseManifest.available.filter(x => x === 'Magic3').length === 1, 'Base must contain the Magic3 library');
assert(Number.isInteger(baseMeta.count) && Math.max(...indices) < baseMeta.count, 'Frame indices exceed the base library');
assert(baseMeta.frames && !Array.isArray(baseMeta.frames), 'Expected keyed frame metadata');
assert(Array.isArray(baseManifest.client_effects), 'Expected client effects array');

const newMeta = structuredClone(baseMeta);
const newManifest = structuredClone(baseManifest);
const frameRows = indices.map(index => {
  const relative = `${magicRelative}/${index}.png`;
  const meta = sourceMeta.frames[String(index)];
  assert(meta && meta.path === `/${relative}` && meta.index === index && meta.maskPath == null && !meta.hasMask, `Unexpected frame ${index}`);
  assert(/^[a-f0-9]{64}$/.test(meta.pngSha256), `Missing frame hash ${index}`);
  const bytes = fs.readFileSync(path.join(sourceRoot, relative));
  assert(sha(bytes) === meta.pngSha256, `Source PNG hash differs: ${index}`);
  assert(bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10])) && bytes.toString('ascii', 12, 16) === 'IHDR', `Invalid PNG ${index}`);
  assert(bytes.readUInt32BE(16) === meta.width && bytes.readUInt32BE(20) === meta.height, `PNG dimensions differ: ${index}`);
  const existed = Object.hasOwn(baseMeta.frames, String(index));
  if (existed) assert.deepEqual(baseMeta.frames[String(index)], meta, `Conflicting base frame metadata: ${index}`);
  const installedPng = path.join(baseRoot, relative);
  if (fs.existsSync(installedPng)) assert(shaFile(installedPng) === meta.pngSha256, `Conflicting base PNG: ${index}`);
  newMeta.frames[String(index)] = meta;
  return { path: relative, length: bytes.length, sha256: meta.pngSha256, rgbaSha256: meta.rgbaSha256, width: meta.width, height: meta.height, addedMetadata: !existed };
});
const effectRows = names.map(name => {
  const found = sourceManifest.client_effects.filter(entry => entry.effect === name);
  assert(found.length === 1, `Expected exactly one source effect: ${name}`);
  const entry = found[0];
  assert(entry.library === 'Magic3' && entry.blend === true && entry.repeat === false && entry.light === 6, `Invalid effect contract: ${name}`);
  if (name === 'ArcherNormalArrow') {
    assert(entry.kind === 'projectile' && entry.base === 1030 && entry.count === 5 && entry.interval === 30 && entry.directionCount === 16 && entry.directionStride === 10, 'Arrow contract differs');
    assert.deepEqual(entry.directionRanges, Array.from({ length: 16 }, (_, direction) => ({ direction, base: 1030 + direction * 10, end: 1034 + direction * 10 })));
  } else assert(entry.kind === 'attackOverlay' && entry.base === 2730 && entry.count === 10 && entry.interval === 80, 'Focus contract differs');
  const existing = baseManifest.client_effects.filter(value => value.effect === name);
  assert(existing.length <= 1, `Duplicate base effect: ${name}`);
  for (const section of ['object_effects', 'map_effects']) assert(!baseManifest[section].some(value => value.effect === name), `Conflicting later effect: ${name}`);
  if (existing.length) assert.deepEqual(existing[0], entry, `Conflicting base effect: ${name}`);
  else newManifest.client_effects.push(structuredClone(entry));
  return { effect: name, canonicalSha256: jsonSha(entry), added: !existing.length };
});
const soundRows = ['M121-0.wav', 'M121-2.wav', 'M121-5.wav'].map(name => {
  const relative = `original-ui/Sound/${name}`;
  const bytes = fs.readFileSync(path.join(sourceRoot, relative));
  assert(bytes.length >= 44 && bytes.toString('ascii', 0, 4) === 'RIFF' && bytes.toString('ascii', 8, 12) === 'WAVE', `Invalid WAV: ${name}`);
  assert(bytes.readUInt32LE(4) + 8 === bytes.length, `Incomplete WAV: ${name}`);
  const hash = sha(bytes);
  const installedSound = path.join(baseRoot, relative);
  if (fs.existsSync(installedSound)) assert(shaFile(installedSound) === hash, `Conflicting base sound: ${name}`);
  return { path: relative, length: bytes.length, sha256: hash, added: !fs.existsSync(installedSound) };
});
// All pre-existing records and all other manifest sections remain identical.
const oldFrames = structuredClone(newMeta);
for (const row of frameRows) if (row.addedMetadata) delete oldFrames.frames[String(Number(path.basename(row.path, '.png')))];
assert.deepEqual(oldFrames, baseMeta);
const oldManifest = structuredClone(newManifest);
oldManifest.client_effects = oldManifest.client_effects.filter(entry => !effectRows.some(row => row.added && row.effect === entry.effect));
assert.deepEqual(oldManifest, baseManifest);

const inputFiles = [
  ...[baseRoot, sourceRoot].flatMap(root => [metaRelative, manifestRelative].map(relative => ({ path: path.join(root, relative), sha256: shaFile(path.join(root, relative)) }))),
];
const links = [];
const copiedBaseFiles = [];
const normalizeRelative = relative => process.platform === 'win32' ? relative.replaceAll('\\', '/').toLowerCase() : relative;
const physicalDirectories = new Set(['', 'original-effects', magicRelative, 'original-ui', 'original-ui/Sound'].map(normalizeRelative));
function assertOwnedParent(filename) {
  const relative = path.relative(output, path.dirname(filename));
  assert(within(output, path.dirname(filename)), `Destination leaves overlay: ${filename}`);
  let directory = output;
  for (const part of ['', ...relative.split(path.sep).filter(Boolean)]) {
    directory = path.join(directory, part);
    const stat = fs.lstatSync(directory);
    assert(stat.isDirectory() && !stat.isSymbolicLink() && within(output, fs.realpathSync(directory)), `Writable parent is not owned: ${directory}`);
  }
  if (fs.existsSync(filename)) assert(fs.lstatSync(filename).isFile() && !fs.lstatSync(filename).isSymbolicLink(), `Unexpected destination: ${filename}`);
}
function linkDirectory(source, destination) {
  assertOwnedParent(destination);
  const target = fs.realpathSync(source);
  fs.symlinkSync(target, destination, process.platform === 'win32' ? 'junction' : 'dir');
  assert(fs.realpathSync(destination) === target, 'Directory alias differs');
  links.push({ path: path.relative(output, destination), target });
}
function copyLevel(source, destination) {
  if (destination !== output) assertOwnedParent(destination);
  fs.mkdirSync(destination);
  for (const entry of fs.readdirSync(source, { withFileTypes: true })) {
    const from = path.join(source, entry.name), to = path.join(destination, entry.name);
    assert(!entry.isSymbolicLink(), `Base contains an unexpected alias: ${from}`);
    if (entry.isDirectory()) {
      if (physicalDirectories.has(normalizeRelative(path.relative(output, to)))) copyLevel(from, to);
      else linkDirectory(from, to);
    } else {
      assert(entry.isFile(), `Unsupported base entry: ${from}`);
      assertOwnedParent(to);
      fs.copyFileSync(from, to, fs.constants.COPYFILE_EXCL);
      const hash = shaFile(from);
      assert(shaFile(to) === hash, `Copied base file differs: ${from}`);
      copiedBaseFiles.push({ path: path.relative(baseRoot, from), sha256: hash });
    }
  }
}
// All writable branches are physical copies. Other directories only alias the
// installed base, and every write rejects aliases in its destination parents.
copyLevel(baseRoot, output);
for (const relative of [metaRelative, manifestRelative, ...soundRows.map(row => row.path)]) assertOwnedParent(path.join(output, relative));
for (const row of [...frameRows, ...soundRows]) {
  const to = path.join(output, row.path);
  assertOwnedParent(to);
  if (!fs.existsSync(to)) fs.copyFileSync(path.join(sourceRoot, row.path), to, fs.constants.COPYFILE_EXCL);
  assert(shaFile(to) === row.sha256, `Output resource differs: ${row.path}`);
}
assertOwnedParent(path.join(output, metaRelative));
fs.writeFileSync(path.join(output, metaRelative), `${JSON.stringify(newMeta, null, 2)}\n`);
assertOwnedParent(path.join(output, manifestRelative));
fs.writeFileSync(path.join(output, manifestRelative), `${JSON.stringify(newManifest, null, 2)}\n`);
assert.deepEqual(readJson(path.join(output, metaRelative)), newMeta);
assert.deepEqual(readJson(path.join(output, manifestRelative)), newManifest);
for (const file of copiedBaseFiles) {
  assert(shaFile(path.join(baseRoot, file.path)) === file.sha256, `Base changed: ${file.path}`);
  if (![metaRelative, manifestRelative].includes(file.path.replaceAll('\\', '/'))) assert(shaFile(path.join(output, file.path)) === file.sha256, `Unrelated copied resource changed: ${file.path}`);
}
for (const file of inputFiles) assert(shaFile(file.path) === file.sha256, `Input changed: ${file.path}`);
for (const row of [...frameRows, ...soundRows]) assert(shaFile(path.join(sourceRoot, row.path)) === row.sha256, `Source resource changed: ${row.path}`);
const receipt = {
  schema: 'mir2.native-archer-overlay.v1', createdUtc: new Date().toISOString(),
  scope: 'Local development/QA asset root with aliases; not a signed release package',
  sourceRoot, baseRoot, output, sourceInputs: inputFiles, links, copiedBaseFiles,
  frameRows, effectRows, soundRows, requiredPngs: frameRows.length, requiredSounds: soundRows.length,
  oldFrameRecords: Object.keys(baseMeta.frames).length, newFrameRecords: Object.keys(newMeta.frames).length,
  oldClientEffects: baseManifest.client_effects.length, newClientEffects: newManifest.client_effects.length,
  outputMetaSha256: shaFile(path.join(output, metaRelative)), outputManifestSha256: shaFile(path.join(output, manifestRelative)),
  patchSha256: jsonSha({ frameRows, effectRows, soundRows }), unchangedBaseVerified: true, unrelatedRecordsPreserved: true,
};
assertOwnedParent(path.join(output, 'archer-overlay-receipt.json'));
fs.writeFileSync(path.join(output, 'archer-overlay-receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`, { flag: 'wx' });
console.log(JSON.stringify({ output, requiredPngs: receipt.requiredPngs, requiredSounds: receipt.requiredSounds, oldFrameRecords: receipt.oldFrameRecords, newFrameRecords: receipt.newFrameRecords, oldClientEffects: receipt.oldClientEffects, newClientEffects: receipt.newClientEffects, patchSha256: receipt.patchSha256, baseUnchanged: true }));
