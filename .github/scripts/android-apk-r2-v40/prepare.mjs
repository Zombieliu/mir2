import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream } from 'node:fs';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { Readable, Transform, Writable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { constants, createGzip } from 'node:zlib';
import { filename, size, sha256, prefix, key, publicUrl } from './artifact.mjs';

try {
  const headers = { authorization: `Bearer ${process.env.GH_TOKEN}`, accept: 'application/vnd.github+json', 'x-github-api-version': '2022-11-28' };
  const releaseResponse = await fetch('https://api.github.com/repos/Zombieliu/mir2/releases/438412927', { headers, redirect: 'error' });
  if (!releaseResponse.ok) throw new Error(`Staging metadata HTTP ${releaseResponse.status}`);
  const release = await releaseResponse.json();
  if (!release.draft || release.tag_name !== 'android-playtest-v40-20261107-cbee2d6a316e-staging') throw new Error('Staging identity mismatch');
  const asset = release.assets.find((item) => item.id === 724714305 && item.name === filename);
  if (asset?.size !== size || asset?.digest !== `sha256:${sha256}` || asset?.state !== 'uploaded') throw new Error('Staging APK digest/size mismatch');
  const redirect = await fetch('https://api.github.com/repos/Zombieliu/mir2/releases/assets/724714305', { headers: { ...headers, accept: 'application/octet-stream' }, redirect: 'manual' });
  if (redirect.status !== 302) throw new Error(`Staging download HTTP ${redirect.status}`);
  const signedUrl = new URL(redirect.headers.get('location'));
  if (signedUrl.protocol !== 'https:' || signedUrl.hostname !== 'release-assets.githubusercontent.com' || signedUrl.username || signedUrl.password || signedUrl.port) throw new Error('Staging download origin mismatch');
  const download = await fetch(signedUrl, { redirect: 'error' });
  if (!download.ok) throw new Error(`Staging bytes HTTP ${download.status}`);
  const stagePath = path.join(process.env.RUNNER_TEMP, filename);
  const hash = createHash('sha256'); let bytes = 0;
  await pipeline(Readable.fromWeb(download.body), new Transform({ transform(chunk, _enc, done) { bytes += chunk.length; hash.update(chunk); done(bytes > size ? new Error('Staging APK exceeded expected size') : null, chunk); } }), createWriteStream(stagePath));
  if (bytes !== size || hash.digest('hex') !== sha256) throw new Error('Downloaded staging APK checksum mismatch');
  console.log('Exact signed APK downloaded and SHA256 verified');
  // The existing uploader and asset Worker support this storage encoding.
  // Downloaded APK bytes and the Android signature remain unchanged.
  const encodedHash = createHash('sha256'); let encodedSize = 0;
  await pipeline(createReadStream(stagePath), createGzip({ level: constants.Z_BEST_COMPRESSION, mtime: 0 }), new Writable({ write(chunk, _enc, done) { encodedSize += chunk.length; encodedHash.update(chunk); done(); } }));
  if (encodedSize > 300 * 1024 * 1024) throw new Error('Storage representation exceeds the existing upload limit');
  const encodedSha256 = encodedHash.digest('hex');
  const manifest = { version: 'android-playtest-v40-20261107', objectPrefix: prefix, assetBaseUrl: publicUrl.slice(0, -(filename.length + 1)), publishReleaseManifest: false, files: [{ relativePath: filename, objectKey: key, stagePath, size, sha256, contentType: 'application/vnd.android.package-archive', cacheControl: 'public, max-age=31536000, immutable', contentEncoding: 'gzip', encodedSize, encodedSha256 }] };
  await writeFile(path.join(process.env.RUNNER_TEMP, 'mir2-apk-r2-release.json'), JSON.stringify(manifest, null, 2));
  console.log(JSON.stringify({ originalBytes: size, originalSha256: sha256, encodedSize, encodedSha256, objectKey: key }));
} catch (error) { console.error(error.message || 'APK staging failed'); process.exitCode = 1; }
