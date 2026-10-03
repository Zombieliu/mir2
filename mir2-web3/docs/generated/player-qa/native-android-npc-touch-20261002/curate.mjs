import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';

// Run from the repository root. Copy only these proof directories, never APKs,
// extracted resources, build caches, or standalone licensed source images.
const qa = 'mir2-web3/docs/generated/player-qa/native-android-npc-touch-20261002';
const raw = 'mir2-web3/apps/game-client/platform-android/target';
const files = [];
for (const [version, sourceName, children] of [
  ['v18-failed', 'npc-touch-v18-20261002', ['']],
  ['v19', 'npc-touch-v19-20261002', ['', 'final-source', 'device',
    'touch-device', 'touch-device-v2', 'touch-device-v3']],
]) {
  for (const child of children) {
    const directory = path.join(raw, sourceName, child);
    for (const name of fs.readdirSync(directory).sort()) {
      const source = path.join(directory, name);
      if (!fs.statSync(source).isFile() || !/\.(log|json|txt|xml|mjs|png)$/.test(name)) continue;
      assert(!source.includes('resource-proof') && !source.includes('final-apks'));
      const target = path.join(qa, version, child, name);
      const bytes = fs.readFileSync(source);
      fs.mkdirSync(path.dirname(target), {recursive: true});
      if (fs.existsSync(target)) assert(bytes.equals(fs.readFileSync(target)), target);
      else fs.writeFileSync(target, bytes, {flag: 'wx'});
      files.push({source, target, bytes: bytes.length,
        sha256: createHash('sha256').update(bytes).digest('hex')});
    }
  }
}
const manifest = {versions: ['v18-failed', 'v19'], originalBytesPreserved: true,
  excluded: ['APK files', 'resource-proof-*', 'build caches', 'licensed source image inputs'], files};
fs.writeFileSync(path.join(qa, 'curation-integrity.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(JSON.stringify({rawFiles: files.length, originalBytesPreserved: true,
  noApksOrLicensedSourceAssets: true}));
