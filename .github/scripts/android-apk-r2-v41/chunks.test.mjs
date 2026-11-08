import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { Readable } from 'node:stream';
import { gzipSync } from 'node:zlib';
import { RELEASE_ID, RELEASE_TAG, MAX_STORAGE_BYTES, validatePins, verifyReleaseMetadata, checkedDownloadUrl, verifyStreamToFile, downloadPinnedChunks, reassembleAndVerify, canonicalGzipMetadata, releaseManifest } from './chunks.mjs';
import { preparePinnedArtifact } from './prepare.mjs';

const sha = b => createHash('sha256').update(b).digest('hex');
function fixture(encodedOverride) {
  const payload = Buffer.from('PK\x03\x04fixture APK opaque signed bytes\n'.repeat(60));
  const encoded = encodedOverride ?? gzipSync(payload, { level: 1, mtime: 0 });
  const bytes = Array.from({ length: 8 }, (_, i) => encoded.subarray(Math.floor(encoded.length * i / 8), Math.floor(encoded.length * (i + 1) / 8)));
  const value = { schemaVersion: 1, pinned: true, releaseId: RELEASE_ID, releaseTag: RELEASE_TAG, artifact: { filename: 'Fixture-v41-arm64.apk', versionCode: 41, size: payload.length, sha256: sha(payload) }, transferGzip: { size: encoded.length, sha256: sha(encoded), parts: bytes.map((b, index) => ({ index, assetId: 1001 + index, name: `fixture.apk.gz.part${String(index + 1).padStart(2, '0')}`, size: b.length, sha256: sha(b) })) } };
  return { payload, encoded, bytes, value, pin: validatePins(value) };
}
const clone = value => JSON.parse(JSON.stringify(value));
function metadata(f) { return { id: RELEASE_ID, draft: true, tag_name: RELEASE_TAG, assets: f.pin.transferGzip.parts.map(p => ({ id: p.assetId, name: p.name, state: 'uploaded', size: p.size, digest: `sha256:${p.sha256}`, url: 'https://untrusted-advertised-url.invalid/ignored' })) }; }
async function temporary(run) { const directory = await mkdtemp(path.join(os.tmpdir(), 'mir2-v41-chunk-test-')); try { return await run(directory); } finally { await rm(directory, { recursive: true, force: true }); } }
async function localParts(f, directory) { return Promise.all(f.bytes.map(async (b, i) => { const file = path.join(directory, `part-${i}`); await writeFile(file, b); return file; })); }
function fakeNetwork(f, { corrupt = false, location, release = metadata(f) } = {}) {
  const calls = [];
  const fetchImpl = async (input, options) => {
    const url = new URL(input); calls.push({ url, options });
    if (url.hostname === 'api.github.com' && url.pathname.endsWith(`/releases/${RELEASE_ID}`)) return Response.json(release);
    if (url.hostname === 'api.github.com' && /\/releases\/assets\/\d+$/.test(url.pathname)) return new Response(null, { status: 302, headers: { location: location ?? `https://release-assets.githubusercontent.com/fixture/${url.pathname.split('/').at(-1)}?temporary=fixture` } });
    if (url.hostname === 'release-assets.githubusercontent.com') {
      const index = Number(url.pathname.split('/').at(-1)) - 1001;
      const bytes = Buffer.from(f.bytes[index]); if (corrupt && index === 0) bytes[0] ^= 1;
      return new Response(bytes, { headers: { 'content-length': String(bytes.length) } });
    }
    assert.fail('Unexpected request outside fixed GitHub/CDN origins');
  };
  return { fetchImpl, calls };
}

test('valid fixed release pins derive exactly one version/hash-qualified APK key', () => {
  const f = fixture();
  assert.equal(f.pin.key, `mir2/v/android-playtest-v41-20261109-${sha(f.payload).slice(0, 12)}/Fixture-v41-arm64.apk`);
  assert.equal(f.pin.publicUrl, `https://assets.mir2.obelisk.build/${f.pin.key}`);
});
test('pending pins, foreign release, unsafe names and oversized encoding fail closed', () => {
  for (const mutate of [v => v.pinned = false, v => v.releaseId++, v => v.artifact.filename = '../outside.apk', v => v.transferGzip.size = MAX_STORAGE_BYTES + 1, v => v.transferGzip.parts[0].name = '../../part']) {
    const v = clone(fixture().value); mutate(v); assert.throws(() => validatePins(v));
  }
});
test('missing/reordered chunks, duplicate IDs/names and size sum mismatch are rejected', () => {
  for (const mutate of [v => v.transferGzip.parts.pop(), v => v.transferGzip.parts[0].index = 1, v => v.transferGzip.parts[1].assetId = v.transferGzip.parts[0].assetId, v => v.transferGzip.parts[1].name = v.transferGzip.parts[0].name, v => v.transferGzip.parts[0].size++]) {
    const v = clone(fixture().value); mutate(v); assert.throws(() => validatePins(v));
  }
});
test('GitHub stored digests, uploaded state and exact names/sizes are mandatory', () => {
  const f = fixture();
  for (const mutate of [m => m.assets[0].digest = null, m => m.assets[0].digest = 'sha256:' + '0'.repeat(64), m => m.assets[0].state = 'uploading', m => m.assets[0].name = 'other.part', m => m.assets[0].size--]) {
    const m = metadata(f); mutate(m); assert.throws(() => verifyReleaseMetadata(m, f.pin), /metadata\/digest mismatch/);
  }
});
test('streamed checksum mismatch and truncated/oversized chunks fail verification', async () => temporary(async directory => {
  const bytes = Buffer.from('chunk fixture');
  await assert.rejects(verifyStreamToFile(Readable.from([bytes]), path.join(directory, 'hash'), { size: bytes.length, sha256: '0'.repeat(64) }, 'Chunk'), /SHA256 mismatch/);
  await assert.rejects(verifyStreamToFile(Readable.from([bytes]), path.join(directory, 'short'), { size: bytes.length + 1, sha256: sha(bytes) }, 'Chunk'), /size mismatch/);
  await assert.rejects(verifyStreamToFile(Readable.from([bytes]), path.join(directory, 'long'), { size: bytes.length - 1, sha256: sha(bytes) }, 'Chunk'), /exceeded pinned size/);
}));
test('valid chunks concatenate and gunzip to the exact original opaque APK bytes', async () => temporary(async directory => {
  const f = fixture(), files = await localParts(f, directory);
  const out = await reassembleAndVerify(f.pin, files, directory);
  assert.deepEqual(await readFile(out.stagePath), f.payload);
  assert.deepEqual(await readFile(out.gzipPath), f.encoded);
}));
test('wrong chunk order is detected by the complete gzip checksum', async () => temporary(async directory => {
  const f = fixture(), files = await localParts(f, directory); [files[0], files[1]] = [files[1], files[0]];
  await assert.rejects(reassembleAndVerify(f.pin, files, directory), /Concatenated gzip SHA256 mismatch/);
}));
test('invalid and truncated gzip fail even when all transport hashes are valid', async () => {
  const good = fixture().encoded;
  for (const encoded of [Buffer.from('not gzip, but a correctly hashed eight-part transfer'), good.subarray(0, good.length - 5)]) {
    await temporary(async directory => { const f = fixture(encoded); await assert.rejects(reassembleAndVerify(f.pin, await localParts(f, directory), directory), /gzip decoding failed/); });
  }
});
test('decoded APK hash, exact size and decompression size cap are enforced', async () => {
  for (const mutate of [v => v.artifact.sha256 = '0'.repeat(64), v => v.artifact.size++, v => v.artifact.size--]) {
    await temporary(async directory => { const f = fixture(), v = clone(f.value); mutate(v); await assert.rejects(reassembleAndVerify(validatePins(v), await localParts(f, directory), directory), /APK (?:SHA256 mismatch|size mismatch|exceeded pinned size)/); });
  }
});
test('temporary download URL must stay on the exact HTTPS GitHub release CDN', () => {
  for (const location of ['https://evil.invalid/file', 'https://release-assets.githubusercontent.com.evil.invalid/file', 'http://release-assets.githubusercontent.com/file', 'https://user:password@release-assets.githubusercontent.com/file']) assert.throws(() => checkedDownloadUrl(location), /Unexpected GitHub download origin/);
});
test('repository token never reaches the CDN and advertised asset URLs are ignored', async () => temporary(async directory => {
  const f = fixture(), network = fakeNetwork(f);
  await downloadPinnedChunks(f.pin, { directory, token: 'fixture-token-not-a-secret', fetchImpl: network.fetchImpl });
  for (const call of network.calls) {
    if (call.url.hostname === 'api.github.com') assert.equal(call.options.headers.authorization, 'Bearer fixture-token-not-a-secret');
    else { assert.equal(call.options.headers.authorization, undefined); assert.equal(call.options.redirect, 'error'); assert.equal(call.url.hostname, 'release-assets.githubusercontent.com'); }
  }
}));
test('foreign CDN redirects fail before any non-GitHub request is made', async () => temporary(async directory => {
  const f = fixture(), network = fakeNetwork(f, { location: 'https://evil.invalid/download' });
  await assert.rejects(downloadPinnedChunks(f.pin, { directory, token: 'fixture-token-not-a-secret', fetchImpl: network.fetchImpl }), /Unexpected GitHub download origin/);
  assert.ok(network.calls.every(c => c.url.hostname === 'api.github.com'));
}));
test('actual streamed chunk corruption fails despite correct stored GitHub metadata', async () => temporary(async directory => {
  const f = fixture(), network = fakeNetwork(f, { corrupt: true });
  await assert.rejects(downloadPinnedChunks(f.pin, { directory, token: 'fixture-token-not-a-secret', fetchImpl: network.fetchImpl }), /Chunk 0 SHA256 mismatch/);
}));
test('complete offline preparation emits one canonical uploader manifest without Cloudflare credentials', async () => temporary(async directory => {
  const f = fixture(), network = fakeNetwork(f);
  const result = await preparePinnedArtifact(f.value, { directory, token: 'fixture-token-not-a-secret', fetchImpl: network.fetchImpl });
  const expected = gzipSync(f.payload, { level: 9, mtime: 0 });
  assert.equal(result.manifest.publishReleaseManifest, false);
  assert.equal(result.manifest.files.length, 1);
  assert.equal(result.manifest.files[0].sha256, sha(f.payload));
  assert.equal(result.manifest.files[0].encodedSha256, sha(expected));
  assert.equal(result.manifest.files[0].encodedSize, expected.length);
  assert.notEqual(sha(expected), sha(f.encoded), 'transfer encoding may differ from canonical uploader bytes');
  assert.equal(result.evidence.cloudflareCredentialsUsedDuringPreparation, false);
  assert.equal(JSON.stringify(result).includes('fixture-token'), false);
  assert.deepEqual(JSON.parse(await readFile(result.manifestPath)), result.manifest);
}));
test('canonical gzip metadata and release manifest enforce the same 300MiB limit', async () => temporary(async directory => {
  const f = fixture(), file = path.join(directory, 'logical.apk'); await writeFile(file, f.payload);
  const encoded = await canonicalGzipMetadata(file);
  assert.equal(encoded.sha256, sha(gzipSync(f.payload, { level: 9, mtime: 0 })));
  assert.throws(() => releaseManifest(f.pin, file, { size: MAX_STORAGE_BYTES + 1, sha256: encoded.sha256 }), /Invalid uploader gzip metadata/);
}));
