import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { BRANCH, RELEASE_ID, RELEASE_TAG, validatePins, downloadPinnedChunks, reassembleAndVerify, canonicalGzipMetadata, releaseManifest } from './chunks.mjs';

export async function preparePinnedArtifact(value, { directory, token, fetchImpl, log }) {
  const pin = validatePins(value);
  const files = await downloadPinnedChunks(pin, { directory: path.join(directory, 'parts'), token, fetchImpl, log });
  const rebuilt = await reassembleAndVerify(pin, files, directory);
  const encoded = await canonicalGzipMetadata(rebuilt.stagePath);
  const manifest = releaseManifest(pin, rebuilt.stagePath, encoded);
  const manifestPath = path.join(directory, 'release.json');
  const evidence = {
    verified: true, releaseId: RELEASE_ID, releaseTag: RELEASE_TAG,
    artifact: pin.artifact, transferGzip: pin.transferGzip,
    canonicalUploaderGzip: encoded, objectKey: pin.key, publicUrl: pin.publicUrl,
    cloudflareCredentialsUsedDuringPreparation: false,
  };
  await writeFile(manifestPath, JSON.stringify(manifest, null, 2) + '\n');
  await writeFile(path.join(directory, 'preparation.json'), JSON.stringify(evidence, null, 2) + '\n');
  return { pin, manifest, manifestPath, evidence };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    if (process.env.GITHUB_REF !== `refs/heads/${BRANCH}` || !process.env.RUNNER_TEMP) throw new Error('Unexpected publication branch or runner context');
    const value = JSON.parse(await readFile(new URL('./artifact-pins.json', import.meta.url), 'utf8'));
    const result = await preparePinnedArtifact(value, {
      directory: path.join(process.env.RUNNER_TEMP, 'mir2-apk-v41'),
      token: process.env.GH_TOKEN,
      log: value => console.log(JSON.stringify(value)),
    });
    console.log(JSON.stringify({ prepared: true, filename: result.pin.artifact.filename, logicalBytes: result.pin.artifact.size, logicalSha256: result.pin.artifact.sha256, transferGzip: { size: result.pin.transferGzip.size, sha256: result.pin.transferGzip.sha256 }, canonicalUploaderGzip: result.evidence.canonicalUploaderGzip, objectKey: result.pin.key }));
  } catch (error) { console.error(error.message || 'Pinned APK preparation failed'); process.exitCode = 1; }
}
