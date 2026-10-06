import {execFileSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {readFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const qa = dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(readFileSync(join(qa, 'curation-integrity.json')));
const revision = process.argv[2] ?? 'HEAD';
const mismatches = [];
for (const file of manifest.files) {
  for (const [location, bytes] of [
    ['working-file', readFileSync(file.target)],
    ['git-blob', execFileSync('git', ['show', `${revision}:${file.target}`], {maxBuffer: 8 * 1024 * 1024})],
  ]) {
    const sha256 = createHash('sha256').update(bytes).digest('hex');
    if (bytes.length !== file.bytes || sha256 !== file.sha256) {
      mismatches.push({path: file.target, location, expectedBytes: file.bytes,
        actualBytes: bytes.length, expectedSha256: file.sha256, actualSha256: sha256});
    }
  }
}
console.log(JSON.stringify({revision, rawFiles: manifest.files.length,
  originalBytesPreservedInWorkingFilesAndGit: mismatches.length === 0, mismatches}, null, 2));
if (mismatches.length) process.exitCode = 1;
