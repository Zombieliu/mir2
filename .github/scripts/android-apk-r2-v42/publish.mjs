import { execFileSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { Readable } from 'node:stream';
import { canonicalGzipMetadata, downloadPinnedChunks, verifyStreamToFile } from '../android-apk-r2-v41/chunks.mjs';

const branch = 'refs/heads/codex/android-map-entry-v42-20261109';
const origin = 'https://assets.mir2.obelisk.build';
const base = {
  url: origin + '/mir2/v/android-playtest-v41-20261109-69d9ebd4f842/Mir2-Android-Playtest-v41-20261109-arm64.apk',
  size: 527239715, sha256: '69d9ebd4f842624646b4b077a19bc39214dec4241b6428efad37527c388e658e',
};
const version = 'android-playtest-v42-20261109';
const filename = 'Mir2-Android-Playtest-v42-20261109-arm64.apk';
const mime = 'application/vnd.android.package-archive';
const sha = s => typeof s === 'string' && /^[a-f0-9]{64}$/.test(s);
const integer = (n, max) => Number.isSafeInteger(n) && n > 0 && n <= max;

function validate({ apk, delta }) {
  if (!integer(apk?.size, 600 * 1024 * 1024) || !sha(apk?.sha256)) throw Error('Invalid v42 APK pin');
  if (delta?.name !== filename + '.zst' || !integer(delta.assetId, Number.MAX_SAFE_INTEGER)
      || !integer(delta.size, 64 * 1024 * 1024) || !sha(delta.sha256)) throw Error('Invalid v42 transfer pin');
  const prefix = `mir2/v/${version}-${apk.sha256.slice(0, 12)}`;
  const objectKey = `${prefix}/${filename}`;
  return { apk, delta: { ...delta, index: 0 }, prefix, objectKey, url: origin + '/' + objectKey };
}

async function download(url, to, expected, label) {
  const response = await fetch(url, { redirect: 'error' });
  if (response.status !== 200 || !response.body) throw Error(`${label} HTTP ${response.status}`);
  return verifyStreamToFile(Readable.fromWeb(response.body), to, expected, label);
}

async function prepare(directory, { apk, delta, prefix, objectKey }) {
  const [deltaPath] = await downloadPinnedChunks({ transferGzip: { parts: [delta] } }, {
    directory: path.join(directory, 'draft'), token: process.env.GH_TOKEN,
  });
  const basePath = path.join(directory, 'base-v41.apk');
  await download(base.url, basePath, base, 'Published v41 base');
  const stagePath = path.join(directory, filename);
  const rebuiltPath = stagePath + '.rebuilt';
  execFileSync('zstd', ['-d', '-q', '-M1024', '--patch-from=' + basePath, deltaPath, '-o', rebuiltPath], { timeout: 120000 });
  const { createReadStream } = await import('node:fs');
  await verifyStreamToFile(createReadStream(rebuiltPath), stagePath, apk, 'Signed v42 APK');
  const encoded = await canonicalGzipMetadata(stagePath);
  const manifest = { version, objectPrefix: prefix, assetBaseUrl: origin + '/' + prefix, publishReleaseManifest: false,
    files: [{ relativePath: filename, objectKey, stagePath, size: apk.size, sha256: apk.sha256,
      contentType: mime, cacheControl: 'public, max-age=31536000, immutable', contentEncoding: 'gzip',
      encodedSize: encoded.size, encodedSha256: encoded.sha256 }] };
  await writeFile(path.join(directory, 'release.json'), JSON.stringify(manifest, null, 2) + '\n');
  console.log(JSON.stringify({ prepared: true, apk, delta, canonicalGzip: encoded, objectKey }));
}

async function verify(directory, { apk, objectKey, url }) {
  const head = await fetch(url, { method: 'HEAD', redirect: 'error' });
  if (head.status !== 200 || head.headers.get('content-type')?.split(';')[0].trim().toLowerCase() !== mime) throw Error('Public APK metadata mismatch');
  const logical = await download(url, path.join(directory, 'public-v42.apk'), apk, 'Complete public APK');
  const range = await fetch(url, { headers: { range: 'bytes=0-1023' }, redirect: 'error' });
  await range.body?.cancel();
  if (range.status !== 200) throw Error('Unexpected encoded-object Range response');
  console.log(JSON.stringify({ ok: true, url, objectKey, filename, ...logical, versionCode: 42,
    minimumAndroid: 12, abi: 'arm64-v8a', rangeSupported: false, rangeStatus: range.status }));
}

try {
  if (process.env.GITHUB_REF !== branch || !process.env.RUNNER_TEMP) throw Error('Unexpected publication context');
  const pin = validate(JSON.parse(await readFile(new URL('./pins.json', import.meta.url), 'utf8')));
  const directory = path.join(process.env.RUNNER_TEMP, 'mir2-apk-v42');
  await mkdir(directory, { recursive: true });
  if (process.argv[2] === 'prepare') await prepare(directory, pin);
  else if (process.argv[2] === 'verify') await verify(directory, pin);
  else throw Error('Unknown publication step');
} catch (error) {
  console.error(error.message || 'Pinned v42 publication failed');
  process.exitCode = 1;
}
