import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
const out = import.meta.dirname;
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const previousFile = 'C:/mir2-build/p01-completion-20261024/scout-anti-convergence-v1/root-full-driver-receipt-v1.json';
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const pin = async file => { const b = await fs.readFile(file); return { file, bytes: b.length, sha256: sha(b) }; };
const previous = JSON.parse(await fs.readFile(previousFile, 'utf8'));
assert.equal(previous.fullNineInputRegressionPassed, true);
assert.deepEqual(previous.counts, { tests: 226, passed: 226, failed: 0, cancelled: 0, skipped: 0, todo: 0 });
const expected = new Map([
  ['apps/web/scripts/playtest-capacity-soak.mjs', '158aaaba83967153fc8f764895c9bce823993732618be934636654cc4401f0d2'],
  ['apps/web/scripts/playtest-capacity-soak.test.mjs', '7cf64c8b28a7b57381dc4beaa5fa01e2d77378fa977e869205b1dc941893b68a'],
]);
const snapshot = async () => {
  const rows = [];
  for (const prior of previous.sourceBefore) {
    const p = await pin(path.join(project, prior.file));
    const row = { file: prior.file, bytes: p.bytes, sha256: p.sha256 };
    if (expected.has(prior.file)) assert.equal(row.sha256, expected.get(prior.file));
    else assert.deepEqual(row, prior);
    rows.push(row);
  }
  assert.equal(rows.length, 9);
  return rows;
};
const args = [...previous.args], sourceBefore = await snapshot();
assert.deepEqual(args, ['--test', 'apps/web/scripts/playtest-capacity-soak.test.mjs', 'apps/web/scripts/playtest-capacity-observability.test.mjs', 'apps/web/scripts/playtest-capacity-monitor.test.mjs', 'apps/web/scripts/playtest-capacity-telemetry.test.mjs']);
const test = await fs.readFile(path.join(project, 'apps/web/scripts/playtest-capacity-soak.test.mjs'));
assert.equal(sha(test.subarray(0, 270311)), 'f1ba119770bde34135e734780b04539206e859bd7b896977d4e6595555c032ef');
const startedAt = new Date().toISOString(), started = performance.now();
await fs.writeFile(path.join(out, 'root-full-driver-before-v1.json'), JSON.stringify({ startedAt, executable: process.execPath, args, sourceBefore }, null, 2) + '\n', { flag: 'wx' });
const stdout = await fs.open(path.join(out, 'root-full-driver-v1.stdout.log'), 'wx');
const stderr = await fs.open(path.join(out, 'root-full-driver-v1.stderr.log'), 'wx');
let terminal;
try {
  const child = spawn(process.execPath, args, { cwd: project, windowsHide: true, stdio: ['ignore', stdout.fd, stderr.fd] });
  terminal = await new Promise((resolve, reject) => {
    child.once('error', reject);
    child.once('exit', (exitCode, signal) => resolve({ exitCode, signal }));
  });
} finally { await stdout.close(); await stderr.close(); }
const elapsedMs = performance.now() - started, endedAt = new Date().toISOString(), sourceAfter = await snapshot();
const log = await pin(path.join(out, 'root-full-driver-v1.stdout.log')), stderrPin = await pin(path.join(out, 'root-full-driver-v1.stderr.log'));
const tap = await fs.readFile(log.file, 'utf8');
const counts = Object.fromEntries([['tests', 'tests'], ['passed', 'pass'], ['failed', 'fail'], ['cancelled', 'cancelled'], ['skipped', 'skipped'], ['todo', 'todo']].map(([name, key]) => [name, Number(tap.match(new RegExp('^# ' + key + ' (\\d+)$', 'm'))?.[1] ?? NaN)]));
const sourcePinsUnchangedAfterRun = JSON.stringify(sourceBefore) === JSON.stringify(sourceAfter);
const fullNineInputRegressionPassed = terminal.exitCode === 0 && terminal.signal === null && sourcePinsUnchangedAfterRun && JSON.stringify(counts) === JSON.stringify({ tests: 232, passed: 232, failed: 0, cancelled: 0, skipped: 0, todo: 0 });
const receipt = { schemaVersion: 1, startedAt, endedAt, elapsedMs, executable: process.execPath, nodeVersion: process.version, cwd: project, args, ...terminal, sourceBefore, sourceAfter, sourceSha256: sha(JSON.stringify(sourceBefore)), sourcePinsUnchangedAfterRun, fullNineInputRegressionPassed, counts, log, stderr: stderrPin, prior226Receipt: await pin(previousFile), capacityAccepted: false, realCapacityRunPerformed: false, p2ImplementationPermitted: false };
await fs.writeFile(path.join(out, 'root-full-driver-receipt-v1.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ terminal, counts, fullNineInputRegressionPassed, sourcePinsUnchangedAfterRun, receipt: await pin(path.join(out, 'root-full-driver-receipt-v1.json')) }));
assert.equal(fullNineInputRegressionPassed, true);
