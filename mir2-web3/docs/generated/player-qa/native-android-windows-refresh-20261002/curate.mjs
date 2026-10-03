import { copyFileSync, readFileSync, writeFileSync, lstatSync, readlinkSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const source = path.resolve('mir2-web3/apps/game-client/platform-android/target/windows-upstream-refresh-20261002');
const output = path.dirname(fileURLToPath(import.meta.url));
const currentHead = spawnSync('git', ['rev-parse', 'HEAD']);
const mergeHead = spawnSync('git', ['rev-parse', '-q', '--verify', 'MERGE_HEAD']);
if (currentHead.status !== 0 || mergeHead.status !== 0
  || currentHead.stdout.toString().trim() !== '9c2a5428d77c6fd2a146b920474eba4b839fdab1'
  || mergeHead.stdout.toString().trim() !== '3d735745f1117d42a7859e87604a106351dca935') {
  throw new Error('Curation is only permitted on the exact resolved pending merge; do not rebind later source');
}
const hash = value => createHash('sha256').update(value).digest('hex');
const proofNames = [
  'source-before.txt', 'status-before.txt', 'upstream-files.txt', 'merge-start.log',
  'merge-source-audit.json', 'conflict-format-before.log', 'conflict-format-after.log',
  'android-tests-first.log', 'android-preview-tests.log', 'shared-ui-tests.log',
  'runtime-tests.log', 'shared-zone-tests.log', 'server-mana-metadata-tests.log',
  'server-transfer-mana-tests.log', 'server-transfer-mana-tests-final.log',
  'gateway-private-monster-tests.log', 'gateway-map-transfer-tests.log',
  'gateway-cadence_map_transfer_tests.log', 'gateway-personal_monster_projection_tests.log',
  'quest-agent-tests.log', 'quest-agent-tests-after-maps.log', 'quest-agent-tests-final.log',
  'api31-debug-check.log', 'api31-preview-check.log',
  'sparse-before.txt', 'sparse-after.txt', 'sparse-after-third-map.txt',
  'sparse-add-maps.log', 'sparse-add-third-map.log',
  'map-input-blobs.txt', 'map-upstream-blobs.txt',
];
const copies = proofNames.map(name => {
  const original = readFileSync(path.join(source, name));
  copyFileSync(path.join(source, name), path.join(output, name));
  const copied = readFileSync(path.join(output, name));
  if (!original.equals(copied)) throw new Error(`Proof byte mismatch: ${name}`);
  return { name, bytes: original.length, sha256: hash(original) };
});
const scopes = [
  'mir2-web3/apps/game-client/platform-android',
  'mir2-web3/apps/game-client/client-bevy', 'mir2-web3/apps/game-client/client-core',
  'mir2-web3/apps/game-client/runtime', 'mir2-web3/apps/game-client/ui-core',
  'mir2-web3/apps/game-client/platform-windows', 'mir2-web3/apps/gateway',
  'mir2-web3/apps/simulation', 'mir2-web3/packages/protocol', 'mir2-web3/packages/game-data',
  'mir2-web3/third_party/bevy_render', 'mir2-web3/apps/web/scripts/quest-agent',
  'mir2-web3/Cargo.toml', 'mir2-web3/Cargo.lock', 'mir2-web3/rust-toolchain.toml',
];
const tracked = spawnSync('git', ['ls-files', '-z', '--', ...scopes], { maxBuffer: 16 * 1024 * 1024 });
if (tracked.error || tracked.status !== 0) throw new Error('Cannot enumerate tracked source scopes');
const sourceNames = tracked.stdout.toString().split('\0').filter(file =>
  /\.(rs|toml|lock|java|gradle|kts|xml|properties|wgsl|sh|mjs|json)$/.test(file));
const sourceFiles = sourceNames.map(file => {
  const stat = lstatSync(file);
  const bytes = stat.isSymbolicLink() ? Buffer.from(readlinkSync(file)) : readFileSync(file);
  return { file, bytes: bytes.length, sha256: hash(bytes) };
});
const sourceReport = { scope: 'Tracked source-scope identity, not coverage or all compiler dependencies',
  sourceFiles };
writeFileSync(path.join(output, 'source-scope-hashes.json'), `${JSON.stringify(sourceReport, null, 2)}\n`);
writeFileSync(path.join(output, 'curation-integrity.json'), `${JSON.stringify({
  scope: 'Exact original proof copies; README/scripts/this manifest are not self-hashed', copies,
}, null, 2)}\n`);
process.stdout.write(`${JSON.stringify({ copied: copies.length, trackedSourceFiles: sourceFiles.length,
  originalBytesEqual: true })}\n`);
