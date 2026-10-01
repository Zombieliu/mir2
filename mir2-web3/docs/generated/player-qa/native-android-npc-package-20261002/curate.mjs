// Copy only explicit proof files, preserving bytes. Never archive APKs/assets.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {createHash} from 'node:crypto';
const qa = 'mir2-web3/docs/generated/player-qa/native-android-npc-package-20261002';
const raw = 'mir2-web3/apps/game-client/platform-android/target';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const files = [];
for (const version of ['v17', 'v18']) {
  const source = path.join(raw, `npc-preview-${version}-20261002`);
  for (const child of ['', 'device']) {
    const directory = path.join(source, child);
    for (const name of fs.readdirSync(directory)) {
      const src = path.join(directory, name);
      if (!fs.statSync(src).isFile() || !/\.(?:log|json|txt|xml|mjs|png)$/.test(name)) continue;
      assert(!name.endsWith('.apk') && !name.includes('resource-proof'));
      const target = path.join(qa, version, child, name);
      const bytes = fs.readFileSync(src);
      fs.mkdirSync(path.dirname(target), {recursive: true});
      fs.copyFileSync(src, target);
      assert(bytes.equals(fs.readFileSync(target)), target);
      files.push({source: src, target, bytes: bytes.length, sha256: hash(bytes)});
    }
  }
}
fs.writeFileSync(path.join(qa, 'curation-integrity.json'), JSON.stringify({
  versions: ['v17', 'v18'], originalBytesPreserved: true, apkAndAssetDirectoriesExcluded: true, files,
}, null, 2) + '\n');
console.log(JSON.stringify({files: files.length, originalBytesPreserved: true, noApksOrSourceAssets: true}));
