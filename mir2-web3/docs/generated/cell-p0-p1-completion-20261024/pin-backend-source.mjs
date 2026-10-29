import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';

const root = process.cwd();
const git = args => execFileSync('git.exe', args, { cwd: root, windowsHide: true });
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const sourcePath = file => /^mir2-web3\/(?:apps\/(?:gateway|simulation)\/.*\.rs|(?:.*\/)?Cargo\.(?:toml|lock)|\.cargo\/.*)$/.test(file);
const changed = git(['diff', '--name-only', '-z', 'HEAD']).toString('utf8').split('\0').filter(Boolean);
const added = git(['ls-files', '--others', '--exclude-standard', '-z', '--', 'mir2-web3/apps/gateway/src', 'mir2-web3/apps/simulation/src', 'mir2-web3/apps/simulation/tests']).toString('utf8').split('\0').filter(Boolean);
const inputs = [...new Set([...changed, ...added, 'mir2-web3/Cargo.toml', 'mir2-web3/Cargo.lock',
  'mir2-web3/apps/gateway/Cargo.toml', 'mir2-web3/apps/simulation/Cargo.toml', 'mir2-web3/.cargo/config.toml'])]
  .filter(sourcePath).sort();
const files = [];
for (const relative of inputs) {
  try {
    const bytes = await fs.readFile(path.join(root, relative));
    files.push({ path: relative, bytes: bytes.length, sha256: sha(bytes) });
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    files.push({ path: relative, missing: true });
  }
}
const canonical = JSON.stringify(files);
const pin = { schemaVersion: 1, capturedAt: new Date().toISOString(),
  gitBase: git(['rev-parse', 'HEAD']).toString('utf8').trim(),
  scope: 'Exact affected Rust source and Cargo inputs over the Git base; runtime content and configuration are pinned separately.',
  sourceManifestSha256: sha(Buffer.from(canonical)), files,
  buildArguments: ['+1.95.0', 'build', '--release', '--locked', '--offline', '-p', 'mir2-gateway', '--bin', 'mir2-gateway'],
};
const output = process.argv[2];
if (!output) throw new Error('output path required');
await fs.writeFile(output, JSON.stringify(pin, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ output, sourceManifestSha256: pin.sourceManifestSha256, fileCount: files.length, gitBase: pin.gitBase }));
