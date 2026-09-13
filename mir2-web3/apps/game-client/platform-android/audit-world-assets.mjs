// Read-only preflight for existing local native-render inputs. No downloads,
// staging, authentication, or claim that these assets have rendered on Android.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { gunzipSync } from 'node:zlib';

const [publicInput, mapInput] = process.argv.slice(2);
if (!publicInput || !mapInput) {
  console.error('Usage: node audit-world-assets.mjs PUBLIC_ROOT MAP_PACK_ROOT');
  process.exit(2);
}
const sha256 = data => crypto.createHash('sha256').update(data).digest('hex');
function requireValue(ok, message) { if (!ok) throw new Error(message); }
function readInside(root, relative, limit) {
  const target = fs.realpathSync(path.join(root, relative));
  requireValue(target.startsWith(root + path.sep), 'Asset escapes selected root');
  const stat = fs.statSync(target);
  requireValue(stat.isFile() && stat.size <= limit, 'Missing or oversized asset');
  return fs.readFileSync(target);
}

try {
  const publicRoot = fs.realpathSync(publicInput);
  const mapRoot = fs.realpathSync(mapInput);
  const manifestBytes = readInside(publicRoot, 'bevy-entity-atlases/manifest.json', 8 * 1024 * 1024);
  const manifest = JSON.parse(manifestBytes);
  requireValue(manifest.schemaVersion === 2 && Array.isArray(manifest.atlases), 'Unsupported entity manifest');
  requireValue(manifest.atlases.length > 0 && manifest.atlases.length <= 32, 'Invalid atlas count');
  const atlases = [];
  for (const atlas of manifest.atlases) {
    requireValue(Array.isArray(atlas.pages) && atlas.pages.length > 0 && atlas.pages.length <= 32, 'Invalid page count');
    requireValue(Array.isArray(atlas.rects) && atlas.rects.length <= 100_000, 'Invalid rectangle count');
    const pages = atlas.pages.map(page => {
      requireValue(typeof page.imageFile === 'string' && /^[a-zA-Z0-9_-][a-zA-Z0-9._-]*\.png$/.test(page.imageFile), 'Unsafe image filename');
      const bytes = readInside(publicRoot, `bevy-entity-atlases/${page.imageFile}`, 32 * 1024 * 1024);
      requireValue(bytes.length === page.imageBytes && sha256(bytes) === page.sha256, 'Atlas page size/hash mismatch');
      requireValue(bytes.length >= 24 && bytes.subarray(0, 8).equals(Buffer.from([137,80,78,71,13,10,26,10])) && bytes.toString('ascii',12,16) === 'IHDR', 'Invalid PNG header');
      requireValue(bytes.readUInt32BE(16) === page.width && bytes.readUInt32BE(20) === page.height, 'PNG dimensions differ from manifest');
      return {file: page.imageFile, bytes: bytes.length, sha256: page.sha256, width: page.width, height: page.height};
    });
    for (const rect of atlas.rects) {
      const index = rect.pageIndex ?? 0;
      requireValue(Number.isInteger(index) && index >= 0 && index < pages.length, 'Invalid rectangle page');
      const page = pages[index];
      requireValue(['x','y','width','height'].every(key => Number.isInteger(rect[key]) && rect[key] >= 0), 'Invalid rectangle geometry');
      requireValue(rect.x + rect.width <= page.width && rect.y + rect.height <= page.height, 'Rectangle exceeds page');
    }
    atlases.push({key: atlas.key, rectangles: atlas.rects.length, pages});
  }
  const packed = readInside(mapRoot, '0.map.gz', 32 * 1024 * 1024);
  const unpacked = gunzipSync(packed, {maxOutputLength: 128 * 1024 * 1024});
  requireValue(unpacked.length > 0, 'Empty Bichon map');
  console.log(JSON.stringify({
    kind: 'android-local-world-asset-preflight', publicRoot, mapRoot,
    manifestSha256: sha256(manifestBytes), generatedAt: manifest.generatedAt,
    atlases, bichon: {file:'0.map.gz', bytes:packed.length, sha256:sha256(packed),
      decodedBytes:unpacked.length, decodedSha256:sha256(unpacked)},
    mapAtlasManifestPresent: fs.existsSync(path.join(publicRoot, 'generated/map-atlas/manifest.json')),
    keyedMapManifestPresent: fs.existsSync(path.join(publicRoot, 'generated/native-map-keyed/manifest.json')),
    limits: 'Local integrity only; map format parsing, full PNG decode, release provenance and Android rendering not verified',
  }, null, 2));
} catch (error) {
  console.error(`World asset preflight failed: ${error.message}`);
  process.exitCode = 1;
}
