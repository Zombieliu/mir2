import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
const directory = path.dirname(fileURLToPath(import.meta.url)), base = path.dirname(directory);
const [repository, expectedHead] = process.argv.slice(2);
assert.ok(repository && /^[a-f0-9]{40}$/.test(expectedHead));
assert.ok(!['5c2e713863d3e9556a92f856b11df2396c73b137', 'b84d5d02a8299b6ac36c5e56cb7006f6bc80d3bd'].includes(expectedHead));
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = async name => { const bytes = await fs.readFile(name); return { path: name, bytes: bytes.length, sha256: sha(bytes) }; };
const git = (args, input) => execFileSync('git.exe', ['-c', 'gc.auto=0', '-c', 'maintenance.auto=false', ...args],
  { cwd: repository, input, windowsHide: true, env: { ...process.env, GIT_NO_LAZY_FETCH: '1' }, maxBuffer: 16 * 1024 * 1024 });
assert.equal(git(['rev-parse', 'HEAD']).toString('utf8').trim(), expectedHead);
assert.equal(git(['status', '--porcelain', '--untracked-files=no']).length, 0);
const functionalName = path.join(base, 'root-attack-deadline-functional-validation-v1.json');
const functional = JSON.parse(await fs.readFile(functionalName, 'utf8'));
const fullName = path.join(directory, 'frozen-full-driver-receipt-v1.json');
const full = JSON.parse(await fs.readFile(fullName, 'utf8'));
assert.deepEqual(functional.checks, { rustFocusedPassed: true, rustfmtPassed: true, nodeRegressionPassed: true });
assert.deepEqual(full.sourceBefore, full.sourceAfter); assert.equal(full.exitCode, 0); assert.equal(full.signal, null);
assert.deepEqual(full.counts, { tests: 210, pass: 210, fail: 0, cancelled: 0, skipped: 0, todo: 0 });
const files = full.sourceBefore.files;
assert.equal(files.length, 9); assert.deepEqual(files, functional.driverSourcePins);
assert.equal(sha(JSON.stringify(files)), functional.driverSourceSha256);
const oldName = path.join(base, 'capacity-native-chase-preference-driver-frozen-v1.json');
const old = JSON.parse(await fs.readFile(oldName, 'utf8'));
assert.deepEqual(files.map(p => p.file), old.files.map(p => p.file));
const gitSourceBindings = [];
for (const item of files) {
  const relative = 'mir2-web3/' + item.file, bytes = await fs.readFile(path.join(repository, relative));
  assert.equal(bytes.length, item.bytes); assert.equal(sha(bytes), item.sha256);
  const filtered = git(['hash-object', '--path=' + relative, '--stdin'], bytes).toString('utf8').trim();
  const committed = git(['rev-parse', expectedHead + ':' + relative]).toString('utf8').trim();
  assert.equal(filtered, committed);
  if (!/playtest-capacity-soak(?:\.test)?\.mjs$/.test(item.file)) assert.deepEqual(item, old.files.find(p => p.file === item.file));
  gitSourceBindings.push({ file: item.file, gitAttributeFilteredObject: filtered, committedGitObject: committed });
}
const fullLog = await pin(full.log.path); assert.equal(fullLog.bytes, full.log.bytes); assert.equal(fullLog.sha256, full.log.sha256);
const testBefore = await fs.readFile(path.join(directory, 'playtest-capacity-soak.test.mjs.before.txt'));
const testNow = await fs.readFile(path.join(repository, 'mir2-web3/apps/web/scripts/playtest-capacity-soak.test.mjs'));
assert.ok(testNow.subarray(0, testBefore.length).equals(testBefore));
const sourceSha256 = sha(JSON.stringify(files));
const result = { schemaVersion: 1, frozenAt: new Date().toISOString(), state: 'FINAL_COMMITTED_DRIVER_SOURCE',
  actualCodeCommit: expectedHead, driverSourceCommit: expectedHead, backendSourceCommit: expectedHead,
  rustBackendModified: true, rustBackendChangeScope: 'Five existing diagnostic timings gain connection context plus one format test; no gameplay changes.',
  sourceSha256, files, gitSourceBindings, otherSevenInputsUnchanged: true,
  priorFreeze: await pin(oldName), priorMain175RawPrefix: { bytes: testBefore.length, sha256: sha(testBefore), retainedByteIdentically: true },
  fullRegression: { passed: 210, failed: 0, cancelled: 0, skipped: 0, durationMs: full.elapsedMs,
    exactSourceSha256: sourceSha256, finalSourceFullRegressionPerformed: true, sourcePinsUnchangedAfterRun: true,
    receipt: await pin(fullName), log: fullLog },
  rootFunctionalValidation: await pin(functionalName), freezeBuilder: await pin(fileURLToPath(import.meta.url)),
  deadlineEvidence: { onlyV2FinalSynchronousRead: true, noAdditionalWaitOrSend: true, commonReceiptUpperBoundMs: 900,
    exactRequestRequired: true, bothDamageReceiversRequired: true, lateTypedOrEitherDamageExcluded: true,
    ordinaryWaitUnchanged: true, guardFailureClosedPrecedeFinalRead: true },
  focusedTests: { added: 10, oldImplementationPassed: 3, oldImplementationFailed: 7, fixedPassed: 10 },
  originalPolicyUnchanged: true, functionalVerificationOnly: true,
  capacityAccepted: false, realCapacityRunPerformedByThisFreeze: false, p2ImplementationPermitted: false };
const output = path.join(base, 'capacity-attack-deadline-driver-frozen-v1.json');
await fs.writeFile(output, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
process.stdout.write(JSON.stringify({ freeze: await pin(output), sourceSha256, actualCodeCommit: expectedHead, capacityAccepted: false }) + '\n');
