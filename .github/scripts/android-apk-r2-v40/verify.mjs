import { createHash } from 'node:crypto';
import { appendFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { filename, size, sha256, key, publicUrl } from './artifact.mjs';

try {
  const head = await fetch(publicUrl, { method: 'HEAD', redirect: 'error' });
  if (head.status !== 200 || head.headers.get('content-type')?.split(';')[0] !== 'application/vnd.android.package-archive') throw new Error(`Public APK metadata mismatch: HTTP ${head.status}`);
  const response = await fetch(publicUrl, { redirect: 'error' });
  if (response.status !== 200) throw new Error(`Public download HTTP ${response.status}`);
  const hash = createHash('sha256'); let bytes = 0;
  for await (const chunk of response.body) { bytes += chunk.length; hash.update(chunk); if (bytes > size) throw new Error('Public download exceeded expected size'); }
  if (bytes !== size || hash.digest('hex') !== sha256) throw new Error('Public APK full-download checksum mismatch');
  const range = await fetch(publicUrl, { headers: { range: 'bytes=0-1023' }, redirect: 'error' });
  const rangeStatus = range.status;
  await range.body?.cancel();
  // Existing encoded-object delivery falls back to the complete decoded file.
  // Record that policy instead of claiming support for byte-range resume.
  if (rangeStatus !== 200) throw new Error(`Unexpected encoded-object Range policy: HTTP ${rangeStatus}`);
  const result = { ok: true, url: publicUrl, objectKey: key, bytes, sha256, versionCode: 40, minimumAndroid: 12, abi: 'arm64-v8a', storageEncoding: 'gzip', rangeSupported: false, rangePolicy: 'full decoded response; restart a failed download', stage: 'basic playtest; three-class level30 not accepted' };
  await writeFile(path.join(process.env.RUNNER_TEMP, 'mir2-apk-r2-result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
  await appendFile(process.env.GITHUB_STEP_SUMMARY, `Verified signed Android v40 basic playtest: [${filename}](${publicUrl})\n\n${size} bytes; SHA256 \`${sha256}\`. Complete public download matches the signed APK. Android 12+, ARM64.\n\nThe existing gzip storage route serves the original APK bytes. It restarts interrupted downloads rather than supporting byte-range resume. Three-class level30 remains unaccepted.\n`);
} catch (error) { console.error(error.message || 'Public APK verification failed'); process.exitCode = 1; }
