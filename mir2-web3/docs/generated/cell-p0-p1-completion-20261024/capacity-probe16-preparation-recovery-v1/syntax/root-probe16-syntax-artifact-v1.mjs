import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
const base = 'C:/mir2-build/p01-completion-20261024';
const root = path.join(base, 'release-native-10-v2-attack-deadline-feasibility-16');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = async file => { const bytes = await fs.readFile(file); return { file, bytes: bytes.length, sha256: sha(bytes) }; };
const frozenName = path.join(root, 'launcher-source-frozen-v1.json'), frozen = JSON.parse(await fs.readFile(frozenName, 'utf8'));
const before = [];
for (const expected of frozen.files) {
  const actual = await pin(expected.file); assert.deepEqual(actual, expected); before.push(actual);
}
assert.equal(before.length, 99);
const scripts = (await fs.readdir(root)).filter(name => name.endsWith('.mjs')).sort();
assert.equal(scripts.length, 30);
const startedAt = new Date().toISOString(), records = [], logName = path.join(root, 'launcher-syntax-check.log');
const log = await fs.open(logName, 'wx');
try {
  for (const name of scripts) {
    const file = path.join(root, name), source = await pin(file), args = ['--check', file];
    const actual = spawnSync(process.execPath, args, { windowsHide: true, encoding: 'utf8' });
    const record = { file: name, source, executable: process.execPath, args,
      exitCode: actual.status, signal: actual.signal, error: actual.error ? String(actual.error) : null,
      stdout: actual.stdout ?? '', stderr: actual.stderr ?? '' };
    records.push(record); await log.write(JSON.stringify(record) + '\n');
  }
} finally { await log.close(); }
const after = [];
for (const expected of frozen.files) { const actual = await pin(expected.file); assert.deepEqual(actual, expected); after.push(actual); }
const passed = records.filter(record => record.exitCode === 0 && !record.signal && !record.error).length;
const result = { schemaVersion: 1, startedAt, endedAt: new Date().toISOString(), sourceSeal: await pin(frozenName),
  checkedScripts: scripts.length, passed, failed: scripts.length - passed, records,
  immutable99InputsUnchangedBeforeAfter: JSON.stringify(before) === JSON.stringify(after), log: await pin(logName),
  purpose: 'Generate the previously missing actual syntax log after partial account-only preparation; no module execution or account changes.',
  accountsCreatedByThisCheck: 0, realmStarted: false, capacityAccepted: false };
const output = path.join(base, 'probe16-missing-syntax-artifact-actual-v1.json');
await fs.writeFile(output, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
process.stdout.write(JSON.stringify({ receipt: await pin(output), checkedScripts: scripts.length, passed,
  failed: result.failed, source99Unchanged: result.immutable99InputsUnchangedBeforeAfter, log: result.log }) + '\n');
assert.equal(passed, scripts.length); assert.equal(result.immutable99InputsUnchangedBeforeAfter, true);
