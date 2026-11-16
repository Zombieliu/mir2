import { createHash } from 'node:crypto';
import { lstat, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(root, '../../..');
const manifestPath = path.join(root, 'manifest.json');
const verify = process.argv.includes('--verify');
const verifySources = process.argv.includes('--verify-sources');
const sourcePaths = [
  'apps/simulation/src/procurement.rs',
  'apps/simulation/src/procurement/tests.rs',
  'apps/simulation/src/procurement/README.md',
  'apps/simulation/src/runtime/procurement.rs',
  'apps/simulation/src/config.rs',
  'apps/simulation/src/billing.rs',
  'apps/simulation/src/db_projection.rs',
  'infra/postgres/migrations/0018_procurement_authority_guard.sql',
  'apps/gateway/src/procurement_worker.rs',
  'apps/gateway/src/procurement_worker_tests.rs',
  'apps/gateway/src/web_procurement.rs',
  'apps/gateway/src/routing/procurement_owner.rs',
  'packages/protocol/src/procurement.rs',
  'apps/game-client/client-bevy/src/native_procurement.rs',
  'apps/game-client/client-bevy/src/quest_ui.rs',
  'apps/game-client/platform-windows/src/native_protocol.rs',
  'apps/game-client/platform-windows/src/gameplay_bridge.rs',
  'onchain/procurement/service/driver.mjs',
  'onchain/procurement/service/grpc.mjs',
  'onchain/procurement/service/canonical.mjs',
  'onchain/procurement/package-lock.json',
  'docs/PERSONAL-PROCUREMENT-V1.zh-CN.md',
  'docs/AGENT-CURRENT.md',
  'docs/NATIVE-WINDOWS-PLAYER-QA.md',
  'docs/FRONTEND-1TO1-GAPS.md',
];
const forbiddenSegment = /^(?:private|keys?|credentials?|node_modules|\.git)$/i;
const forbiddenFile = /^(?:\.env(?:\..*)?|.*\.(?:pem|key|p12|pfx|exe|dll|zip|tgz|tar|gz))$/i;
const allowedExtension = /(?:\.(?:md|json|log|png|jpg|jpeg|mjs|sql)|\.gitattributes|\.gitignore)$/i;
const privateMaterial = /-----BEGIN (?:[A-Z ]*PRIVATE KEY)-----|suiprivkey1[0-9a-z]{20,}/;
const normalize = (value) => value.split(path.sep).join('/');
const sha = (bytes) => createHash('sha256').update(bytes).digest('hex');

function assertSafeRelative(relative) {
  if (path.isAbsolute(relative) || relative.split('/').includes('..')) throw new Error(`Non-relative artifact: ${relative}`);
  const parts = relative.split('/');
  if (parts.some((part) => forbiddenSegment.test(part)) || forbiddenFile.test(parts.at(-1))) throw new Error(`Forbidden artifact name: ${relative}`);
  if (!allowedExtension.test(relative)) throw new Error(`Unexpected artifact type: ${relative}`);
}

async function enumerate(directory = root, prefix = '') {
  const found = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const relative = `${prefix}${entry.name}`;
    const absolute = path.join(directory, entry.name);
    if ((await lstat(absolute)).isSymbolicLink()) throw new Error(`Symlink refused: ${relative}`);
    if (entry.isDirectory()) {
      if (forbiddenSegment.test(entry.name)) throw new Error(`Forbidden directory: ${relative}`);
      found.push(...await enumerate(absolute, `${relative}/`));
    } else {
      if (relative === 'manifest.json') continue;
      assertSafeRelative(relative);
      found.push(relative);
    }
  }
  return found.sort();
}

async function artifact(relative) {
  const bytes = await readFile(path.join(root, relative));
  if (!/\.(?:png|jpg|jpeg)$/i.test(relative) && privateMaterial.test(bytes.toString('utf8'))) throw new Error(`Private material marker refused in ${relative}`);
  return { path: relative, bytes: bytes.length, sha256: sha(bytes), group: relative.includes('/') ? relative.split('/')[0] : 'bundle' };
}

async function sourcePin(relative) {
  // Repository source files are text and Git enforces LF. Normalize only CRLF;
  // evidence artifacts above always retain and hash their original raw bytes.
  const raw = await readFile(path.join(repo, relative));
  const bytes = Buffer.from(raw.toString('utf8').replace(/\r\n/g, '\n'), 'utf8');
  return { path: relative, bytes: bytes.length, sha256: sha(bytes) };
}

const files = await Promise.all((await enumerate()).map(artifact));
if (verify) {
  const existing = JSON.parse(await readFile(manifestPath, 'utf8'));
  if (JSON.stringify(files) !== JSON.stringify(existing.files)) throw new Error('Artifact set/bytes/SHA differ from manifest; inspect changes then regenerate');
  if (verifySources) {
    const pins = await Promise.all(existing.sourceSnapshot.files.map(({ path: relative }) => sourcePin(relative)));
    if (JSON.stringify(pins) !== JSON.stringify(existing.sourceSnapshot.files)) throw new Error('Current checkout differs from the recorded source snapshot');
  }
  console.log(JSON.stringify({ verified: true, files: files.length, rawBytes: files.reduce((sum, file) => sum + file.bytes, 0), sourcePinsVerified: verifySources }));
} else {
  const pins = await Promise.all(sourcePaths.map(sourcePin));
  const manifest = {
    formatVersion: 1,
    generatedAtUtc: new Date().toISOString(),
    scope: 'Curated public procurement V1 evidence; fixtures, embedded SQL, standalone testnet and Windows live remain separate scopes',
    pathConvention: 'files relative to this manifest; sourceSnapshot files relative to repository; copied raw evidence may retain historical local execution paths',
    hashAlgorithm: 'SHA256; files use raw artifact bytes preserved by .gitattributes; sourceSnapshot uses its explicitly declared text normalization',
    excluded: ['manifest.json self-reference', 'private stores and signing keys', 'credentials and environment files', 'node_modules and package archives', 'executable binaries'],
    sourceSnapshot: {
      kind: 'Evidence assembly checkout snapshot; does not retroactively pin historical test executions',
      normalization: 'UTF-8 text with CRLF replaced by LF before byte count and SHA256; no other text or BOM rewriting',
      files: pins,
    },
    files,
  };
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(JSON.stringify({ written: 'manifest.json', files: files.length, rawBytes: files.reduce((sum, file) => sum + file.bytes, 0), sourcePins: pins.length }));
}
