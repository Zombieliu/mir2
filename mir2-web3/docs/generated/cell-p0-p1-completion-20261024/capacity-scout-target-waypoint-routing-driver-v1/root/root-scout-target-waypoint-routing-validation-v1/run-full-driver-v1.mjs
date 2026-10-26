import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
const out = import.meta.dirname;
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const previousFile = 'C:/mir2-build/p01-completion-20261024/root-scout-selection-diagnostics-validation-v1/root-full-driver-receipt-v1.json';
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const pin = async file => { const b = await fs.readFile(file); return { file, bytes: b.length, sha256: sha(b) }; };
const previous = JSON.parse(await fs.readFile(previousFile, 'utf8'));
assert.equal(previous.fullNineInputRegressionPassed, true);
assert.deepEqual(previous.counts, { tests: 232, passed: 232, failed: 0, cancelled: 0, skipped: 0, todo: 0 });
const expected = new Map([
  ['apps/web/scripts/playtest-capacity-soak.mjs', '8acb34b5c1815bab0ab82ec26fff1b770cbbf159779f7c55103dacb51c127d56'],
  ['apps/web/scripts/playtest-capacity-soak.test.mjs', 'efa0779b746d138894f63300c1e484bee72ae405d41d9e3d0271b25ad0069656'],
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
const preservationFile = 'C:/mir2-build/p01-completion-20261024/scout-target-waypoint-routing-v1/source-preservation-and-focused-receipt-v1.json';
const maintenanceFile = 'C:/mir2-build/p01-completion-20261024/scout-target-waypoint-routing-v1/existing-test-search-count-maintenance-v1.json';
assert.equal((await pin(preservationFile)).sha256, '3f073e4cf97dcd1d85ccefe24d37263477e5f63ef341c23273e7d864f0d082cc');
assert.equal((await pin(maintenanceFile)).sha256, '1119032c4e1b5acab94158ba006d6affc5f30cf1aeffe71bf57afab74be6c1df');
const preservation = JSON.parse(await fs.readFile(preservationFile, 'utf8')), maintenance = JSON.parse(await fs.readFile(maintenanceFile, 'utf8'));
assert.deepEqual(preservation.currentSourcePins, sourceBefore); assert.equal(preservation.sourceSha256, sha(JSON.stringify(sourceBefore)));
assert.equal(maintenance.rules.length, 6); assert.deepEqual(maintenance.changedOriginalCases, [3173,3308,3447]);
let restoredTest = test.toString('utf8'); const eol = restoredTest.includes('\r\n') ? '\r\n' : '\n';
for (const rule of [...maintenance.rules].reverse()) { const after = rule.after.replace(/\r?\n/g,eol), before = rule.before.replace(/\r?\n/g,eol); assert.equal(restoredTest.split(after).length-1,1); restoredTest=restoredTest.replace(after,before); }
const restoredBytes=Buffer.from(restoredTest,'utf8'); assert.equal(sha(restoredBytes.subarray(0,278248)), '7cf64c8b28a7b57381dc4beaa5fa01e2d77378fa977e869205b1dc941893b68a');
assert.deepEqual({bytes:restoredBytes.length-278248,sha256:sha(restoredBytes.subarray(278248))},preservation.preservation.newTestsAppendedAfterOriginalPrefixInverse); assert.equal(preservation.preservation.newTestCount,5);
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
const fullNineInputRegressionPassed = terminal.exitCode === 0 && terminal.signal === null && sourcePinsUnchangedAfterRun && JSON.stringify(counts) === JSON.stringify({ tests: 237, passed: 237, failed: 0, cancelled: 0, skipped: 0, todo: 0 });
const receipt = { schemaVersion: 1, startedAt, endedAt, elapsedMs, executable: process.execPath, nodeVersion: process.version, cwd: project, args, ...terminal, sourceBefore, sourceAfter, sourceSha256: sha(JSON.stringify(sourceBefore)), sourcePinsUnchangedAfterRun, fullNineInputRegressionPassed, counts, log, stderr: stderrPin, prior232Receipt: await pin(previousFile), sourcePreservationReceipt: await pin(preservationFile), existingTestMaintenance: await pin(maintenanceFile), capacityAccepted: false, realCapacityRunPerformed: false, p2ImplementationPermitted: false };
await fs.writeFile(path.join(out, 'root-full-driver-receipt-v1.json'), JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ terminal, counts, fullNineInputRegressionPassed, sourcePinsUnchangedAfterRun, receipt: await pin(path.join(out, 'root-full-driver-receipt-v1.json')) }));
assert.equal(fullNineInputRegressionPassed, true);
