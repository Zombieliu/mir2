import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const out = path.dirname(fileURLToPath(import.meta.url));
const cwd = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const source = 'apps/gateway/src/web.rs';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const beforeBytes = await fs.readFile(path.join(cwd, source));
const args = ['run', '1.95.0', 'rustfmt', '--edition', '2021', '--config', 'skip_children=true', '--check', source];
const startedAt = new Date().toISOString(), begin = performance.now();
const actual = spawnSync('rustup.exe', args, { cwd, windowsHide: true, encoding: null });
const log = Buffer.concat([actual.stdout ?? Buffer.alloc(0), actual.stderr ?? Buffer.alloc(0)]);
const logPath = path.join(out, 'integration-rustfmt-v1.log');
await fs.writeFile(logPath, log, { flag: 'wx' });
const afterBytes = await fs.readFile(path.join(cwd, source));
const result = { schemaVersion: 1, startedAt, endedAt: new Date().toISOString(), elapsedMs: performance.now() - begin,
  executable: 'rustup.exe', args, exitCode: actual.status, signal: actual.signal,
  error: actual.error ? String(actual.error) : null,
  sourceBefore: { path: 'mir2-web3/' + source, bytes: beforeBytes.length, sha256: sha(beforeBytes) },
  sourceAfter: { path: 'mir2-web3/' + source, bytes: afterBytes.length, sha256: sha(afterBytes) },
  sourceBeforeAfterIdentical: beforeBytes.equals(afterBytes), checkedFiles: 1,
  log: { path: logPath, bytes: log.length, sha256: sha(log) }, capacityAccepted: false };
await fs.writeFile(path.join(out, 'integration-rustfmt-execution-v1.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
process.stdout.write(JSON.stringify(result) + '\n');
assert.equal(actual.status, 0); assert.equal(actual.error, undefined); assert.equal(result.sourceBeforeAfterIdentical, true);
