import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import test from 'node:test';
import {
  readManifestSource,
  verifyEntityReleaseAlignment,
} from './verify-entity-release-alignment.mjs';

function fixture() {
  const manifestBytes = Buffer.from(`${JSON.stringify({
    schemaVersion: 2,
    kind: 'mir2-bevy-entity-atlas-manifest',
    atlases: [{
      key: 'player',
      pages: [{ imageFile: 'player.png', imageBytes: 123, width: 1, height: 1, sha256: 'a'.repeat(64) }],
      rects: [{ key: 'body', pageIndex: 0, x: 0, y: 0, width: 1, height: 1 }],
    }],
  })}\n`);
  const manifestSha256 = crypto.createHash('sha256').update(manifestBytes).digest('hex');
  const lock = {
    schemaVersion: 1,
    kind: 'mir2-android-entity-pack-lock',
    packId: 'release-20260912',
    sourceMode: 'override',
    manifestSha256,
    manifestBytes: manifestBytes.length,
    atlasCount: 1,
    pageCount: 1,
    rectCount: 1,
    pageBytes: 123,
  };
  return { lock, manifestBytes };
}

test('accepts the exact published manifest named by the Android lock', () => {
  const { lock, manifestBytes } = fixture();
  const result = verifyEntityReleaseAlignment(
    Buffer.from(JSON.stringify(lock)),
    manifestBytes,
    'release-20260912',
  );
  assert.deepEqual(result, {
    kind: 'mir2-android-web-entity-release-alignment',
    aligned: true,
    packId: 'release-20260912',
    manifestSha256: lock.manifestSha256,
    manifestBytes: manifestBytes.length,
    atlasCount: 1,
    pageCount: 1,
    rectCount: 1,
    pageBytes: 123,
  });
});

test('rejects a different manifest even when its aggregate counts match', () => {
  const { lock, manifestBytes } = fixture();
  const changed = Buffer.from(manifestBytes.toString('utf8').replace('player.png', 'other_.png'));
  assert.throws(
    () => verifyEntityReleaseAlignment(Buffer.from(JSON.stringify(lock)), changed),
    /SHA-256 differs/,
  );
});

test('rejects lock aggregate drift after the manifest hash matches', () => {
  const { lock, manifestBytes } = fixture();
  lock.pageBytes += 1;
  assert.throws(
    () => verifyEntityReleaseAlignment(Buffer.from(JSON.stringify(lock)), manifestBytes),
    /pageBytes differs/,
  );
});

test('rejects a release label that does not match the APK lock', () => {
  const { lock, manifestBytes } = fixture();
  assert.throws(
    () => verifyEntityReleaseAlignment(
      Buffer.from(JSON.stringify(lock)),
      manifestBytes,
      'release-20260913',
    ),
    /pack ID differs/,
  );
});

test('rejects non-HTTPS remote sources before any request', async () => {
  await assert.rejects(
    readManifestSource('http://example.invalid/manifest.json'),
    /local file or HTTPS URL/,
  );
});

test('rejects credential-like query data before any request', async () => {
  await assert.rejects(
    readManifestSource('https://example.invalid/manifest.json?token=secret'),
    /query data or fragments/,
  );
});
