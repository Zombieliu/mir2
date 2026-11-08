import { createHash } from 'node:crypto';
import { appendFile, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { validatePins } from './chunks.mjs';

try {
  const pin = validatePins(JSON.parse(await readFile(new URL('./artifact-pins.json', import.meta.url), 'utf8')));
  if (!process.env.RUNNER_TEMP || !process.env.GITHUB_STEP_SUMMARY) throw new Error('Missing runner verification context');
  const head = await fetch(pin.publicUrl, { method: 'HEAD', redirect: 'error' });
  if (head.status !== 200 || head.headers.get('content-type')?.split(';')[0].trim().toLowerCase() !== 'application/vnd.android.package-archive') throw new Error(`Public APK metadata mismatch: HTTP ${head.status}`);
  const response = await fetch(pin.publicUrl, { redirect: 'error' });
  if (response.status !== 200 || !response.body) throw new Error(`Public download HTTP ${response.status}`);
  const hash = createHash('sha256'); let size = 0;
  for await (const chunk of response.body) { size += chunk.length; if (size > pin.artifact.size) throw new Error('Public APK exceeded pinned logical size'); hash.update(chunk); }
  const sha256 = hash.digest('hex');
  if (size !== pin.artifact.size || sha256 !== pin.artifact.sha256) throw new Error('Public APK full-download size/SHA256 mismatch');
  const range = await fetch(pin.publicUrl, { headers: { range: 'bytes=0-1023' }, redirect: 'error' });
  const rangeStatus = range.status;
  await range.body?.cancel();
  if (rangeStatus !== 200) throw new Error(`Unexpected existing encoded-object Range policy: HTTP ${rangeStatus}`);
  const result = {
    ok: true, url: pin.publicUrl, objectKey: pin.key, filename: pin.artifact.filename,
    size, sha256, versionCode: 41, minimumAndroid: 12, abi: 'arm64-v8a',
    storageEncoding: 'gzip', rangeSupported: false, rangeStatus,
    rangePolicy: 'complete decoded response; interrupted downloads restart',
    scope: 'six-map entry hotfix; three-class level30 journey remains unaccepted',
  };
  await writeFile(path.join(process.env.RUNNER_TEMP, 'mir2-apk-v41/public-verification.json'), JSON.stringify(result, null, 2) + '\n');
  await appendFile(process.env.GITHUB_STEP_SUMMARY, `Verified signed Android v41 map-entry hotfix: [${pin.artifact.filename}](${pin.publicUrl})\n\n${size} bytes; SHA256 \`${sha256}\`. Public complete download matches the pinned signed APK. Android12+, ARM64.\n\nThe existing encoded-object route returns HTTP200 for Range; interrupted downloads restart. Three-class level30 acceptance remains open.\n`);
  console.log(JSON.stringify(result));
} catch (error) { console.error(error.message || 'Public APK verification failed'); process.exitCode = 1; }
