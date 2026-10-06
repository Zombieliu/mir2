// Fail-closed comparison between the entity pack lock embedded in the Android APK and
// the exact manifest bytes published for the Web client. This script never
// downloads atlas pages and never approves a release; it only proves that the
// two consumers name the same immutable manifest.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const MAX_JSON_BYTES = 16 * 1024 * 1024;
const FETCH_TIMEOUT_MS = 15_000;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const PACK_ID_PATTERN = /^[A-Za-z0-9._-]{1,96}$/;

function requireValue(value, message) {
  if (!value) throw new Error(message);
}

function sha256(bytes) {
  return crypto.createHash('sha256').update(bytes).digest('hex');
}

function readBoundedFile(fileName) {
  const resolved = fs.realpathSync(fileName);
  const stat = fs.statSync(resolved);
  requireValue(stat.isFile(), 'Alignment input is not a regular file');
  requireValue(stat.size > 0 && stat.size <= MAX_JSON_BYTES, 'Alignment input byte count is out of bounds');
  return fs.readFileSync(resolved);
}

async function readBoundedHttps(source) {
  const url = new URL(source);
  requireValue(url.protocol === 'https:', 'Remote release manifests must use HTTPS');
  requireValue(!url.username && !url.password, 'Release manifest URLs must not contain credentials');
  requireValue(!url.search && !url.hash, 'Release manifest URLs must not contain query data or fragments');
  const response = await fetch(url, {
    redirect: 'error',
    signal: AbortSignal.timeout(FETCH_TIMEOUT_MS),
    headers: { accept: 'application/json' },
  });
  requireValue(response.status === 200, `Release manifest returned HTTP ${response.status}`);
  const declaredLength = response.headers.get('content-length');
  if (declaredLength !== null) {
    const length = Number(declaredLength);
    requireValue(Number.isSafeInteger(length) && length > 0 && length <= MAX_JSON_BYTES,
      'Release manifest Content-Length is out of bounds');
  }
  requireValue(response.body, 'Release manifest response has no body');
  const reader = response.body.getReader();
  const chunks = [];
  let byteCount = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    byteCount += value.byteLength;
    requireValue(byteCount <= MAX_JSON_BYTES, 'Release manifest response exceeds the byte limit');
    chunks.push(value);
  }
  requireValue(byteCount > 0, 'Release manifest response is empty');
  return Buffer.concat(chunks.map(chunk => Buffer.from(chunk)), byteCount);
}

export async function readManifestSource(source) {
  if (source.startsWith('https://')) return readBoundedHttps(source);
  requireValue(!source.includes('://'), 'Release manifest must be a local file or HTTPS URL');
  return readBoundedFile(source);
}

function parseJson(bytes, label) {
  try {
    return JSON.parse(bytes.toString('utf8'));
  } catch (error) {
    throw new Error(`${label} is invalid JSON: ${error.message}`);
  }
}

function requireSafeCount(value, label) {
  requireValue(Number.isSafeInteger(value) && value >= 0, `${label} is invalid`);
  return value;
}

function validateLock(lock) {
  requireValue(lock?.schemaVersion === 1 && lock?.kind === 'mir2-android-entity-pack-lock',
    'Android entity pack lock schema is unsupported');
  requireValue(PACK_ID_PATTERN.test(lock.packId ?? ''), 'Android entity pack ID is invalid');
  requireValue(SHA256_PATTERN.test(lock.manifestSha256 ?? ''),
    'Android entity manifest SHA-256 is invalid');
  for (const key of ['manifestBytes', 'atlasCount', 'pageCount', 'rectCount', 'pageBytes']) {
    requireSafeCount(lock[key], `Android lock ${key}`);
  }
  requireValue(lock.manifestBytes > 0 && lock.manifestBytes <= MAX_JSON_BYTES,
    'Android lock manifestBytes is out of bounds');
  requireValue(lock.atlasCount > 0 && lock.atlasCount <= 64,
    'Android lock atlasCount is out of bounds');
  requireValue(lock.pageCount > 0 && lock.pageCount <= 128,
    'Android lock pageCount is out of bounds');
  requireValue(lock.rectCount > 0 && lock.rectCount <= 100_000,
    'Android lock rectCount is out of bounds');
}

function summarizeManifest(manifest) {
  requireValue(manifest?.schemaVersion === 2
      && manifest?.kind === 'mir2-bevy-entity-atlas-manifest'
      && Array.isArray(manifest.atlases)
      && manifest.atlases.length > 0
      && manifest.atlases.length <= 64,
  'Published entity manifest schema or atlas count is unsupported');
  let pageCount = 0;
  let rectCount = 0;
  let pageBytes = 0;
  for (const atlas of manifest.atlases) {
    requireValue(Array.isArray(atlas?.pages) && atlas.pages.length > 0 && atlas.pages.length <= 32,
      'Published entity atlas page count is unsupported');
    requireValue(Array.isArray(atlas?.rects) && atlas.rects.length > 0,
      'Published entity atlas rectangle list is invalid');
    pageCount += atlas.pages.length;
    rectCount += atlas.rects.length;
    for (const page of atlas.pages) {
      pageBytes += requireSafeCount(page?.imageBytes, 'Published entity page imageBytes');
    }
  }
  requireValue(pageCount <= 128 && rectCount <= 100_000,
    'Published entity manifest exceeds the Android aggregate budget');
  requireValue(Number.isSafeInteger(pageBytes), 'Published entity page byte total is invalid');
  return { atlasCount: manifest.atlases.length, pageCount, rectCount, pageBytes };
}

export function verifyEntityReleaseAlignment(lockBytes, manifestBytes, expectedPackId = '') {
  requireValue(lockBytes.length > 0 && lockBytes.length <= MAX_JSON_BYTES,
    'Android entity pack lock byte count is out of bounds');
  requireValue(manifestBytes.length > 0 && manifestBytes.length <= MAX_JSON_BYTES,
    'Published entity manifest byte count is out of bounds');
  const lock = parseJson(lockBytes, 'Android entity pack lock');
  const manifest = parseJson(manifestBytes, 'Published entity manifest');
  validateLock(lock);
  if (expectedPackId) {
    requireValue(PACK_ID_PATTERN.test(expectedPackId), 'Expected entity pack ID is invalid');
    requireValue(lock.packId === expectedPackId,
      `Android entity pack ID differs: expected ${expectedPackId}, got ${lock.packId}`);
  }
  const manifestHash = sha256(manifestBytes);
  requireValue(lock.manifestSha256 === manifestHash,
    `Entity manifest SHA-256 differs: Android=${lock.manifestSha256} Web=${manifestHash}`);
  requireValue(lock.manifestBytes === manifestBytes.length,
    `Entity manifest byte count differs: Android=${lock.manifestBytes} Web=${manifestBytes.length}`);
  const summary = summarizeManifest(manifest);
  for (const key of ['atlasCount', 'pageCount', 'rectCount', 'pageBytes']) {
    requireValue(lock[key] === summary[key],
      `Entity manifest ${key} differs: Android=${lock[key]} Web=${summary[key]}`);
  }
  return {
    kind: 'mir2-android-web-entity-release-alignment',
    aligned: true,
    packId: lock.packId,
    manifestSha256: manifestHash,
    manifestBytes: manifestBytes.length,
    ...summary,
  };
}

async function main() {
  const [lockFile, manifestSource, expectedPackId = ''] = process.argv.slice(2);
  if (!lockFile || !manifestSource) {
    console.error('Usage: node verify-entity-release-alignment.mjs ANDROID_PACK_LOCK WEB_MANIFEST_FILE_OR_HTTPS_URL [EXPECTED_PACK_ID]');
    process.exitCode = 2;
    return;
  }
  try {
    const lockBytes = readBoundedFile(lockFile);
    const manifestBytes = await readManifestSource(manifestSource);
    const result = verifyEntityReleaseAlignment(lockBytes, manifestBytes, expectedPackId);
    console.log(JSON.stringify({ ...result, manifestSource }, null, 2));
  } catch (error) {
    console.error(`Entity release alignment failed: ${error.message}`);
    process.exitCode = 1;
  }
}

if (process.argv[1]
    && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main();
}
