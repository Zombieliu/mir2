import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const base = 'C:/mir2-build/p01-completion-20261024';
const repository = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = async name => { const bytes = await fs.readFile(name); return { path: name, bytes: bytes.length, sha256: sha(bytes) }; };
const read = async name => JSON.parse((await fs.readFile(name, 'utf8')).replace(/^\uFEFF/, ''));
const rustName = path.join(base, 'formal11-correlated-slow-stage-validation-v1/focused-execution-v1.json');
const nodeName = path.join(base, 'attack-deadline-poll-fix-v1/frozen-full-driver-receipt-v1.json');
const fmtName = path.join(base, 'attack-deadline-poll-fix-v1/integration-rustfmt-execution-v1.json');
const rust = await read(rustName), node = await read(nodeName), fmt = await read(fmtName);
assert.equal(rust.exit.code, 0); assert.equal(rust.exit.signal, null); assert.equal(rust.sourceUnchanged, true);
assert.equal(rust.tests.passed, 8); assert.equal(rust.tests.failed, 0); assert.equal(rust.tests.ignored, 0);
assert.equal(node.exitCode, 0); assert.equal(node.signal, null); assert.equal(node.sourcePinsUnchangedAfterRun, true);
assert.deepEqual(node.sourceBefore, node.sourceAfter);
assert.deepEqual(node.counts, { tests: 210, pass: 210, fail: 0, cancelled: 0, skipped: 0, todo: 0 });
assert.equal(fmt.exitCode, 0); assert.equal(fmt.error, null); assert.equal(fmt.sourceBeforeAfterIdentical, true);
const webPath = 'mir2-web3/apps/gateway/src/web.rs', web = await pin(path.join(repository, webPath));
const webPin = { path: webPath, bytes: web.bytes, sha256: web.sha256 };
for (const source of [rust.sourceBefore, rust.sourceAfter]) {
  assert.equal(source.file, 'apps/gateway/src/web.rs'); assert.equal(source.bytes, web.bytes); assert.equal(source.sha256, web.sha256);
}
assert.deepEqual(fmt.sourceBefore, webPin); assert.deepEqual(fmt.sourceAfter, webPin);
for (const source of node.sourceBefore.files) {
  const current = await pin(path.join(repository, 'mir2-web3', source.file));
  assert.equal(current.bytes, source.bytes); assert.equal(current.sha256, source.sha256);
}
assert.equal(node.sourceBefore.files.length, 9);
assert.equal(sha(JSON.stringify(node.sourceBefore.files)), node.sourceBefore.sourceSha256);
const previousManifestName = path.join(base, 'backend-source-personal-monster-projection-v1.json');
const previousManifest = await read(previousManifestName), unchanged = [];
assert.equal(previousManifest.files.length, 44);
for (const source of previousManifest.files) {
  const current = await pin(path.join(repository, source.path));
  if (source.path === webPath) { assert.notEqual(current.sha256, source.sha256); continue; }
  assert.equal(current.bytes, source.bytes); assert.equal(current.sha256, source.sha256); unchanged.push(source.path);
}
assert.equal(unchanged.length, 43);
const definitions = [
  ['rustFocused', rust.exit.code, rust.tests.passed, rust.tests.failed, rust.log.file, rustName],
  ['nodeRegression', node.exitCode, node.counts.pass, node.counts.fail, node.log.path, nodeName],
  ['rustfmt', fmt.exitCode, fmt.checkedFiles, 0, fmt.log.path, fmtName],
];
const actualChecks = [];
for (const [kind, exitCode, passed, failed, logName, receiptName] of definitions) {
  const log = await pin(logName), expected = kind === 'rustFocused' ? rust.log : kind === 'nodeRegression' ? node.log : fmt.log;
  assert.equal(log.bytes, expected.bytes); assert.equal(log.sha256, expected.sha256);
  actualChecks.push({ kind, exitCode, passed, failed, log, receipt: await pin(receiptName) });
}
const result = { schemaVersion: 1, verifiedAt: new Date().toISOString(),
  scope: 'V2 final evidence polling and connection-correlated diagnostic timing only; no gameplay or gate changes.',
  sourcePins: [webPin], driverSourcePins: node.sourceBefore.files, driverSourceSha256: node.sourceBefore.sourceSha256,
  checks: { rustFocusedPassed: true, rustfmtPassed: true, nodeRegressionPassed: true }, actualChecks,
  backendComparison: { previousManifest: await pin(previousManifestName), inputCount: 44, unchangedRawInputs: unchanged,
    unchangedRawInputCount: 43, changedBackendPaths: [webPath] },
  rustScope: '8 distinct socket diagnostic tests: 7 unchanged existing plus 1 new; not a full workspace suite.',
  nodeScope: 'One actual four-file 210-test run with exact nine raw source inputs before and after.',
  preliminaryRuns: { main: 185, adjacent: 25, separateExecutions: true, usedAsSingleFullRunEvidence: false },
  releaseBuiltForThisSource: false, fullLifecycleForThisSource: false, capacityAccepted: false, fixedCellStarted: false };
const output = path.join(base, 'root-attack-deadline-functional-validation-v1.json');
await fs.writeFile(output, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
process.stdout.write(JSON.stringify({ receipt: await pin(output), checks: result.checks, driverSourceSha256: result.driverSourceSha256,
  backendInputCount: 44, unchangedBackendInputs: 43, capacityAccepted: false }) + '\n');
