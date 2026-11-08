import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream } from 'node:fs';
import { mkdir, rename } from 'node:fs/promises';
import path from 'node:path';
import { Readable, Transform, Writable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { constants, createGunzip, createGzip } from 'node:zlib';

export const REPOSITORY = 'Zombieliu/mir2';
export const RELEASE_ID = 438562899;
export const RELEASE_TAG = 'android-playtest-v41-20261109-cc6ef80a';
export const BRANCH = 'codex/android-map-entry-v41-20261109';
export const MAX_STORAGE_BYTES = 300 * 1024 * 1024;
export const PUBLIC_ORIGIN = 'https://assets.mir2.obelisk.build';
const API_ROOT = `https://api.github.com/repos/${REPOSITORY}`;
const isPositiveSize = n => Number.isSafeInteger(n) && n > 0;
const isSha256 = s => typeof s === 'string' && /^[0-9a-f]{64}$/.test(s);
const safeName = s => typeof s === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]{0,159}$/.test(s) && !s.includes('..');

export function validatePins(value) {
  if (value?.schemaVersion !== 1 || value?.pinned !== true) throw new Error('Final signed artifact pins are not ready');
  if (value.releaseId !== RELEASE_ID || value.releaseTag !== RELEASE_TAG) throw new Error('Unexpected staging release identity');
  const a = value.artifact, g = value.transferGzip;
  if (!safeName(a?.filename) || !a.filename.endsWith('.apk') || a.versionCode !== 41 || !isPositiveSize(a.size) || a.size > 1024 * 1024 * 1024 || !isSha256(a.sha256)) throw new Error('Invalid signed APK pin');
  if (!isPositiveSize(g?.size) || g.size > MAX_STORAGE_BYTES || !isSha256(g.sha256) || !Array.isArray(g.parts) || g.parts.length !== 8) throw new Error('Invalid or oversized gzip transfer pin');
  const ids = new Set(), names = new Set();
  let total = 0;
  const parts = g.parts.map((part, index) => {
    if (part?.index !== index || !isPositiveSize(part.assetId) || ids.has(part.assetId) || !safeName(part.name) || names.has(part.name) || !isPositiveSize(part.size) || part.size > 64 * 1024 * 1024 || !isSha256(part.sha256)) throw new Error('Invalid chunk identity, order or size');
    ids.add(part.assetId); names.add(part.name); total += part.size;
    return Object.freeze({ index, assetId: part.assetId, name: part.name, size: part.size, sha256: part.sha256 });
  });
  if (ids.size !== parts.length || total !== g.size) throw new Error('Duplicate chunk IDs or gzip size sum mismatch');
  const prefix = `mir2/v/android-playtest-v41-20261109-${a.sha256.slice(0, 12)}`;
  const key = `${prefix}/${a.filename}`;
  return Object.freeze({ artifact: Object.freeze({ filename: a.filename, versionCode: 41, size: a.size, sha256: a.sha256 }), transferGzip: Object.freeze({ size: g.size, sha256: g.sha256, parts: Object.freeze(parts) }), prefix, key, publicUrl: `${PUBLIC_ORIGIN}/${key}` });
}

export function verifyReleaseMetadata(release, pin) {
  if (release?.id !== RELEASE_ID || release.draft !== true || release.tag_name !== RELEASE_TAG || !Array.isArray(release.assets)) throw new Error('Staging draft identity mismatch');
  for (const part of pin.transferGzip.parts) {
    const matches = release.assets.filter(a => a.id === part.assetId);
    if (matches.length !== 1 || matches[0].name !== part.name || matches[0].state !== 'uploaded' || matches[0].size !== part.size || matches[0].digest !== `sha256:${part.sha256}`) throw new Error(`Stored GitHub chunk metadata/digest mismatch: part ${part.index}`);
  }
}

export function checkedDownloadUrl(location) {
  let url;
  try { url = new URL(location); } catch { throw new Error('Unexpected GitHub download origin'); }
  if (url.protocol !== 'https:' || url.hostname !== 'release-assets.githubusercontent.com' || url.username || url.password || url.port || url.hash) throw new Error('Unexpected GitHub download origin');
  return url;
}

function verifier(expected, label) {
  const hash = createHash('sha256'); let size = 0;
  return {
    stream: new Transform({ transform(chunk, _encoding, done) { size += chunk.length; if (size > expected.size) return done(new Error(`${label} exceeded pinned size`)); hash.update(chunk); done(null, chunk); } }),
    finish() { if (size !== expected.size) throw new Error(`${label} size mismatch`); const sha256 = hash.digest('hex'); if (sha256 !== expected.sha256) throw new Error(`${label} SHA256 mismatch`); return { size, sha256 }; },
  };
}

export async function verifyStreamToFile(source, destination, expected, label) {
  const check = verifier(expected, label);
  await pipeline(source, check.stream, createWriteStream(destination, { flags: 'wx' }));
  return check.finish();
}

async function safeFetch(fetchImpl, url, options, label) {
  try { return await fetchImpl(url, options); } catch { throw new Error(`${label} request failed`); }
}

export async function downloadPinnedChunks(pin, { directory, token, fetchImpl = fetch, log = () => {} }) {
  if (!token) throw new Error('Missing repository-scoped Actions token');
  await mkdir(directory, { recursive: true });
  const headers = { authorization: `Bearer ${token}`, accept: 'application/vnd.github+json', 'x-github-api-version': '2022-11-28' };
  const response = await safeFetch(fetchImpl, `${API_ROOT}/releases/${RELEASE_ID}`, { headers, redirect: 'error' }, 'Staging metadata');
  if (response.status !== 200) throw new Error(`Staging metadata HTTP ${response.status}`);
  let release;
  try { release = await response.json(); } catch { throw new Error('Invalid staging metadata JSON'); }
  verifyReleaseMetadata(release, pin);
  const files = [];
  for (const part of pin.transferGzip.parts) {
    const redirect = await safeFetch(fetchImpl, `${API_ROOT}/releases/assets/${part.assetId}`, { headers: { ...headers, accept: 'application/octet-stream' }, redirect: 'manual' }, `Chunk ${part.index} redirect`);
    if (redirect.status !== 302) throw new Error(`Chunk ${part.index} download redirect HTTP ${redirect.status}`);
    const location = redirect.headers.get('location');
    await redirect.body?.cancel();
    const url = checkedDownloadUrl(location);
    // The repository token is sent only to api.github.com. CDN requests have
    // no Authorization header and cannot forward through further redirects.
    const body = await safeFetch(fetchImpl, url, { redirect: 'error', headers: { 'accept-encoding': 'identity' } }, `Chunk ${part.index} bytes`);
    if (body.status !== 200 || !body.body) throw new Error(`Chunk ${part.index} bytes HTTP ${body.status}`);
    const encoding = body.headers.get('content-encoding');
    const length = body.headers.get('content-length');
    if ((encoding && encoding.toLowerCase() !== 'identity') || (length && Number(length) !== part.size)) { await body.body.cancel(); throw new Error(`Chunk ${part.index} HTTP representation mismatch`); }
    const file = path.join(directory, `${String(part.index).padStart(2, '0')}-${part.name}`);
    await verifyStreamToFile(Readable.fromWeb(body.body), file, part, `Chunk ${part.index}`);
    files.push(file);
    log({ verifiedChunk: part.index, assetId: part.assetId, name: part.name, size: part.size, sha256: part.sha256 });
  }
  return files;
}

export async function reassembleAndVerify(pin, files, directory) {
  if (files.length !== pin.transferGzip.parts.length) throw new Error('Chunk count mismatch');
  await mkdir(directory, { recursive: true });
  const gzipPath = path.join(directory, 'transfer.apk.gz');
  async function* joined() { for (const file of files) for await (const bytes of createReadStream(file)) yield bytes; }
  await verifyStreamToFile(Readable.from(joined()), gzipPath, pin.transferGzip, 'Concatenated gzip');
  const stagePath = path.join(directory, pin.artifact.filename), temporary = stagePath + '.partial';
  const check = verifier(pin.artifact, 'APK');
  try { await pipeline(createReadStream(gzipPath), createGunzip(), check.stream, createWriteStream(temporary, { flags: 'wx' })); }
  catch (error) { if (error?.message?.startsWith('APK ')) throw error; throw new Error('Pinned gzip decoding failed'); }
  const logical = check.finish();
  await rename(temporary, stagePath);
  return { stagePath, gzipPath, ...logical };
}

export async function canonicalGzipMetadata(stagePath) {
  let size = 0; const hash = createHash('sha256');
  // upload-r2-assets.mjs recompresses the logical file with these same options.
  // Verify transfer gzip separately; do not assume OS/zlib encoders match.
  await pipeline(createReadStream(stagePath), createGzip({ level: constants.Z_BEST_COMPRESSION, mtime: 0 }), new Writable({ write(chunk, _encoding, done) { size += chunk.length; if (size > MAX_STORAGE_BYTES) return done(new Error('Uploader gzip exceeds 300MiB limit')); hash.update(chunk); done(); } }));
  return { size, sha256: hash.digest('hex') };
}

export function releaseManifest(pin, stagePath, encoded) {
  if (!isPositiveSize(encoded.size) || encoded.size > MAX_STORAGE_BYTES || !isSha256(encoded.sha256)) throw new Error('Invalid uploader gzip metadata');
  return { version: 'android-playtest-v41-20261109', objectPrefix: pin.prefix, assetBaseUrl: `${PUBLIC_ORIGIN}/${pin.prefix}`, publishReleaseManifest: false, files: [{ relativePath: pin.artifact.filename, objectKey: pin.key, stagePath, size: pin.artifact.size, sha256: pin.artifact.sha256, contentType: 'application/vnd.android.package-archive', cacheControl: 'public, max-age=31536000, immutable', contentEncoding: 'gzip', encodedSize: encoded.size, encodedSha256: encoded.sha256 }] };
}
