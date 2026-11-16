// Audits native UI literals/specs plus the complete configured UI-library ranges.
// Optional --dataDir verifies original pixels and classifies original empty slots.
import { readFile, readdir, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import sharp from 'sharp';
import { parseLibrary, decodeFrameRgba, decodeMaskFrameRgba } from './crystal-library.mjs';

const root = path.resolve(import.meta.dirname, '../../..');
const args = Object.fromEntries(process.argv.slice(2).reduce((a, v, i, all) => v.startsWith('--') ? [...a, [v.slice(2), all[i + 1]]] : a, []));
const assets = path.resolve(args.assets ?? path.join(root, 'apps/web/public/original-ui'));
const manifest = JSON.parse(await readFile(path.join(import.meta.dirname, 'crystal-ui-export-manifest.json')));
const libraries = new Set(['Prguse', 'Prguse2', 'Title', 'ChrSel', 'BuffIcon', 'GuildSkill', 'MagIcon', 'MagIcon2', 'MapLinkIcon']);
const requirements = new Map();
const add = (lib, index, origin) => {
  if (!libraries.has(lib)) return;
  const key = `${lib}/${Number(index)}`;
  if (!requirements.has(key)) requirements.set(key, { lib, index: Number(index), origins: new Set() });
  requirements.get(key).origins.add(origin);
};
const configured = new Set();
for (const lib of libraries) {
  const config = manifest.libraries[lib] ?? {};
  const indexes = [...(config.indices ?? [])];
  for (const [start, end] of config.ranges ?? []) for (let i = start; i <= end; i++) indexes.push(i);
  for (const index of indexes) { configured.add(`${lib}/${index}`); add(lib, index, 'manifest'); }
}
async function walk(dir) {
  const entries = await readdir(dir, { withFileTypes: true });
  return (await Promise.all(entries.filter(e => !['target', 'node_modules', '.git'].includes(e.name)).map(e => e.isDirectory() ? walk(path.join(dir, e.name)) : path.join(dir, e.name)))).flat();
}
let scannedFiles = 0;
for (const file of await walk(path.join(root, 'apps/game-client'))) {
  if (!file.endsWith('.rs') || file.includes(`${path.sep}target${path.sep}`) || /tests?\.rs$/.test(file)) continue;
  const text = (await readFile(file, 'utf8')).split('#[cfg(test)]')[0];
  scannedFiles++;
  const origin = path.relative(root, file).replaceAll('\\', '/');
  for (const m of text.matchAll(/original-ui\/([\w/]+)\/(\d+)\.png/g)) add(m[1], m[2], origin);
  for (const m of text.matchAll(/"(Prguse2?|Title|ChrSel|BuffIcon|GuildSkill|MagIcon2?|MapLinkIcon)"\s*,\s*(\d+)\b/g)) add(m[1], m[2], origin);
  for (const m of text.matchAll(/CrystalButtonSpec::new\(\s*"(\w+)"\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/g)) for (const index of m.slice(2)) add(m[1], index, origin);
  for (const m of text.matchAll(/\(\s*"(\w+)"\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*\d+\.\d+/g)) for (const index of m.slice(2)) add(m[1], index, origin);
  for (const m of text.matchAll(/library:\s*"(\w+)"\s*,\s*normal:\s*(\d+)\s*,\s*hover:\s*(\d+)\s*,\s*pressed:\s*(\d+)/g)) for (const index of m.slice(2)) add(m[1], index, origin);
  for (const m of text.matchAll(/"(\w+)"\s*,\s*vec!\[([\d,\s]+)\]/g)) for (const n of m[2].matchAll(/\d+/g)) add(m[1], n[0], origin);
  // Common required_frames loops with literal arrays and chained inclusive ranges.
  for (const m of text.matchAll(/for\s+(\w+)\s+in\s+([\s\S]{1,350}?)\s*\{\s*frames\.push\(\("(\w+)",\s*\1\)\)/g)) {
    const expression = m[2];
    if (/^\[[\d,\s]+\]$/.test(expression)) for (const n of expression.matchAll(/\d+/g)) add(m[3], n[0], origin);
    else for (const range of expression.matchAll(/(\d+)\.\.=([\d]+)/g)) for (let n = +range[1]; n <= +range[2]; n++) add(m[3], n, origin);
  }
}
const report = { scannedFiles, libraries: [...libraries], requiredFrames: requirements.size, verifiedFrames: 0, sourceHashes: {}, issues: [], sourceEmpty: [], limitations: ['Preview animation base-plus-frame arithmetic, server-selected icons, computed skill indexes and service-mode-selected titles are not inferred unless expressed in a supported literal/spec/array/range form. Configured UI-library ranges are fully checked.', 'This verifies assets, not native interaction or visual parity.'] };
const hash = b => createHash('sha256').update(b).digest('hex');
for (const lib of libraries) {
  let meta;
  try { meta = JSON.parse(await readFile(path.join(assets, lib, 'meta.json'))); } catch { meta = { frames: [] }; }
  const frames = new Map(meta.frames.map(f => [f.index, f]));
  const source = args.dataDir ? parseLibrary(await readFile(path.join(args.dataDir, `${lib}.Lib`))) : null;
  if (source) {
    report.sourceHashes[lib] = hash(source.buffer);
    if (meta.sourceLibrary?.sha256 && meta.sourceLibrary.sha256 !== report.sourceHashes[lib]) report.issues.push({ key: lib, kind: 'source-library-hash' });
  }
  for (const requirement of requirements.values()) {
    if (requirement.lib !== lib) continue;
    const { index } = requirement;
    const key = `${lib}/${index}`;
    const origins = [...requirement.origins];
    const issue = kind => report.issues.push({ key, kind, origins });
    if (!configured.has(key)) issue('missing-export-manifest');
    const frame = source?.frames[index];
    if (source && (!frame || frame.width <= 0 || frame.height <= 0)) {
      if (origins.some(origin => origin !== 'manifest')) issue('source-empty-ui-reference');
      let exportedPng = 'absent';
      try { const bytes = await readFile(path.join(assets, lib, `${index}.png`)); try { await sharp(bytes).raw().toBuffer(); exportedPng = 'decodable'; } catch { exportedPng = 'invalid-legacy-png'; } } catch {}
      report.sourceEmpty.push({ key, origins, kind: frame ? 'zero-dimension-source' : 'absent-source-slot', exportedPng });
      continue;
    }
    let bytes, decoded;
    try { bytes = await readFile(path.join(assets, lib, `${index}.png`)); } catch { issue('missing-png'); continue; }
    try { decoded = await sharp(bytes).ensureAlpha().raw().toBuffer({ resolveWithObject: true }); } catch { issue('invalid-png'); continue; }
    const metadata = frames.get(index);
    if (!metadata) issue('missing-frame-metadata');
    else {
      if (decoded.info.width !== metadata.width || decoded.info.height !== metadata.height) issue('metadata-dimensions');
      if (metadata.pngSha256 && metadata.pngSha256 !== hash(bytes)) issue('metadata-png-hash');
      if (metadata.rgbaSha256 && metadata.rgbaSha256 !== hash(decoded.data)) issue('metadata-rgba-hash');
    }
    if (frame && (!decodeFrameRgba(source, frame).equals(decoded.data) || frame.width !== decoded.info.width || frame.height !== decoded.info.height)) issue('source-pixel-mismatch');
    if (frame?.maskRgba) {
      try {
        const mask = await sharp(await readFile(path.join(assets, lib, `${index}.mask.png`))).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
        if (mask.info.width !== frame.maskWidth || mask.info.height !== frame.maskHeight || !mask.data.equals(decodeMaskFrameRgba(source, frame))) issue('source-mask-mismatch');
      } catch { issue('missing-or-invalid-mask'); }
    }
    report.verifiedFrames++;
  }
}
if (args.report) { await mkdir(path.dirname(path.resolve(args.report)), { recursive: true }); await writeFile(args.report, JSON.stringify(report, null, 2) + '\n'); }
console.log(JSON.stringify({ ...report, sourceEmpty: report.sourceEmpty.length, issues: report.issues }, null, 2));
if (report.issues.length) process.exitCode = 1;
