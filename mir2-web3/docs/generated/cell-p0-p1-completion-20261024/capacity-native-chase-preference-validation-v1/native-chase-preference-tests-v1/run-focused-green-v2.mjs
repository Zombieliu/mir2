import fs from 'node:fs/promises'; import path from 'node:path'; import crypto from 'node:crypto'; import assert from 'node:assert/strict'; import { spawnSync } from 'node:child_process';
const directory = 'C:/mir2-build/p01-completion-20261024/native-chase-preference-tests-v1', project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const files = ['apps/web/scripts/playtest-capacity-soak.mjs', 'apps/web/scripts/playtest-capacity-soak.test.mjs', 'apps/web/scripts/playtest-capacity-soak-core.mjs'].map(p => path.join(project, p));
const pin = async file => { const b = await fs.readFile(file); return { file, bytes: b.length, sha256: crypto.createHash('sha256').update(b).digest('hex') }; };
const before = await Promise.all(files.map(pin)), startedAt = new Date().toISOString();
const args = ['--test', '--test-name-pattern=^native chase preference', 'apps/web/scripts/playtest-capacity-soak.test.mjs'];
const run = spawnSync(process.execPath, args, { cwd: project, windowsHide: true, encoding: null, maxBuffer: 4 * 1024 * 1024 });
const log = path.join(directory, 'focused-green-v2.log'); await fs.writeFile(log, Buffer.concat([run.stdout ?? Buffer.alloc(0), run.stderr ?? Buffer.alloc(0)]), { flag: 'wx' });
const after = await Promise.all(files.map(pin)); assert.deepEqual(before, after);
const text = (run.stdout ?? Buffer.alloc(0)).toString('utf8'), number = k => Number(new RegExp('^# ' + k + ' (\\d+)$', 'm').exec(text)?.[1]);
const result = { schemaVersion: 2, startedAt, endedAt: new Date().toISOString(), command: [process.execPath, ...args], exitCode: run.status,
  tests: number('tests'), passed: number('pass'), failed: number('fail'), skipped: number('skipped'), sourceBefore: before, sourceAfter: after, sourceUnchanged: true,
  log: await pin(log), old187PrefixByteIdentical: (await fs.readFile(files[1])).subarray(0, 224817).equals(await fs.readFile(path.join(directory, 'before-187.test.mjs.txt'))),
  priorFailedFocusedReceiptPreserved: await pin(path.join(directory, 'focused-green-v1.json')), mutation: await pin(path.join(directory, 'append-receipt-v2.json')),
  scope: 'Final 13 new focused chase-preference regression only; no game realm, old-main swap or full regression run.' };
const receipt = path.join(directory, 'focused-green-v2.json'); await fs.writeFile(receipt, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ receipt: await pin(receipt), log: result.log, exitCode: result.exitCode, tests: result.tests, passed: result.passed, failed: result.failed, skipped: result.skipped, old187PrefixByteIdentical: result.old187PrefixByteIdentical }));
if (run.status !== 0) process.exitCode = run.status ?? 1;
