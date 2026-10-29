import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const out = path.dirname(fileURLToPath(import.meta.url));
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const old = JSON.parse(await fs.readFile(path.join(out, '../capacity-native-chase-preference-driver-frozen-v1.json'), 'utf8'));
const snapshot = async () => {
  const files = [];
  for (const previous of old.files) {
    const bytes = await fs.readFile(path.join(project, previous.file));
    const pin = { file: previous.file, bytes: bytes.length, sha256: sha(bytes) };
    if (!/playtest-capacity-soak(?:\.test)?\.mjs$/.test(previous.file)) assert.deepEqual(pin, previous);
    files.push(pin);
  }
  assert.equal(files.length, 9);
  return { files, sourceSha256: sha(JSON.stringify(files)) };
};
const args = ['--test', 'apps/web/scripts/playtest-capacity-soak.test.mjs',
  'apps/web/scripts/playtest-capacity-observability.test.mjs',
  'apps/web/scripts/playtest-capacity-monitor.test.mjs',
  'apps/web/scripts/playtest-capacity-telemetry.test.mjs'];
const before = await snapshot(), startedAt = new Date().toISOString(), started = performance.now();
await fs.writeFile(path.join(out, 'frozen-full-driver-before-v1.json'), JSON.stringify({ startedAt, executable: process.execPath, args, sourceBefore: before }, null, 2) + '\n', { flag: 'wx' });
const handle = await fs.open(path.join(out, 'frozen-full-driver-v1.log'), 'wx');
let exit;
try {
  const child = spawn(process.execPath, args, { cwd: project, windowsHide: true, stdio: ['ignore', handle.fd, handle.fd] });
  exit = await new Promise((resolve, reject) => { child.once('error', reject); child.once('exit', (exitCode, signal) => resolve({ exitCode, signal })); });
} finally { await handle.close(); }
const elapsedMs = performance.now() - started, endedAt = new Date().toISOString(), after = await snapshot();
const bytes = await fs.readFile(path.join(out, 'frozen-full-driver-v1.log')), log = bytes.toString('utf8');
const counts = Object.fromEntries(['tests', 'pass', 'fail', 'cancelled', 'skipped', 'todo'].map(key => {
  const match = log.match(new RegExp('^# ' + key + ' (\\d+)$', 'm'));
  return [key, match ? Number(match[1]) : null];
}));
const result = { schemaVersion: 1, startedAt, endedAt, elapsedMs, executable: process.execPath, args, ...exit,
  sourceBefore: before, sourceAfter: after, sourcePinsUnchangedAfterRun: JSON.stringify(before) === JSON.stringify(after),
  counts, log: { path: path.join(out, 'frozen-full-driver-v1.log'), bytes: bytes.length, sha256: sha(bytes) },
  precedingMain185AndAdjacent25AreSeparateActualRuns: true, capacityAccepted: false };
await fs.writeFile(path.join(out, 'frozen-full-driver-receipt-v1.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
process.stdout.write(JSON.stringify(result) + '\n');
assert.equal(exit.exitCode, 0); assert.equal(exit.signal, null); assert.equal(result.sourcePinsUnchangedAfterRun, true);
assert.deepEqual(counts, { tests: 210, pass: 210, fail: 0, cancelled: 0, skipped: 0, todo: 0 });
