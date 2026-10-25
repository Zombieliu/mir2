// External-only source and formatting provenance. No tests, Cargo, or services.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';

const root = import.meta.dirname;
const base = path.dirname(root);
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const sha = data => crypto.createHash('sha256').update(data).digest('hex');
const read = async file => { const data = await fs.readFile(file); return { data, pin: { file, bytes: data.length, sha256: sha(data) } }; };
const inputs = [
  ['apps/gateway/src/routing.rs', 'routing.rs.txt', 1389827, '9135f2f3a019b04c1bb6ffe3717f730c65632df4e642702057bfbf7a699564de'],
  ['apps/gateway/src/routing/tests/personal_monster_projection_tests.rs', 'personal_monster_projection_tests.rs.txt', 30917, '7d474f50150108175bd96268c2f22270f916bbfdefc8a47e5f4fd0c249ffc0fc'],
  ['apps/simulation/src/runtime/zone/runtime.rs', 'zone-runtime.rs.txt', 749298, 'd19840e87e6b2e718303d90d02a03e8c1c4941e069f0b168d9d253644950f3b6'],
];
const current = await Promise.all(inputs.map(async ([relative, name, bytes, digest]) => {
  const result = await read(path.join(project, relative));
  assert.equal(result.pin.bytes, bytes, relative);
  assert.equal(result.pin.sha256, digest, relative);
  return { ...result, name };
}));
const oldProof = await read(path.join(root, 'candidate-source-preservation-v1.json'));
const proof = JSON.parse(oldProof.data);
assert.equal(current[0].pin.sha256, proof.sourcePins[0].sha256);
assert.equal(current[2].pin.sha256, proof.sourcePins[2].sha256);
const originalTests = await read(path.join(root, 'preimages/personal_monster_projection_tests.rs.txt'));
assert.equal(originalTests.pin.bytes, 12555);
assert.ok(current[1].data.subarray(0, originalTests.pin.bytes).equals(originalTests.data));
const formatted = await read(path.join(root, 'format-candidate-v1/personal_monster_projection_tests.rs'));
assert.ok(formatted.data.equals(current[1].data));
const newTests = [...current[1].data.subarray(originalTests.pin.bytes).toString('utf8').matchAll(/fn (personal_journey_[a-z0-9_]+)\(/g)].map(match => match[1]);
assert.deepEqual(newTests, proof.newTests);
const preparation = await read(path.join(root, 'format-candidate-v1/preparation-v1.json'));
const application = await read(path.join(root, 'format-application-v1/application-receipt-v1.json'));
const testBeforeFormat = await read(path.join(root, 'format-application-v1/personal_monster_projection_tests-before.rs.txt'));
assert.equal(testBeforeFormat.pin.sha256, proof.sourcePins[1].sha256);
const fmtFailure = await read(path.join(base, 'probe14-personal-projection-rustfmt37-v1.log'));
assert.equal(fmtFailure.pin.bytes, 9871);
assert.equal(fmtFailure.pin.sha256, '8c5b900335f13312c37c622a000752fd4108dccdb0cc2f7af581705d844a44ce');
const behavioralGreen = await read(path.join(base, 'probe14-personal-journey-green-v2.log'));
assert.match(behavioralGreen.data.toString('utf8'), /test result: ok\. 9 passed; 0 failed/);
const output = path.join(root, 'final-format-candidate-v1');
await fs.mkdir(output); // Fail closed if this immutable capture already exists.
const copies = [];
for (const entry of current) {
  const destination = path.join(output, entry.name);
  await fs.writeFile(destination, entry.data, { flag: 'wx' });
  const copy = await read(destination);
  assert.equal(copy.pin.sha256, entry.pin.sha256);
  copies.push({ source: entry.pin, copy: copy.pin });
}
const receipt = {
  schemaVersion: 1,
  capturedAt: new Date().toISOString(),
  state: 'FORMATTED_SOURCE_CAPTURE_PENDING_ROOT_FINAL_VALIDATION',
  baseCodeCommit: 'b84d5d02a8299b6ac36c5e56cb7006f6bc80d3bd',
  sourceCopies: copies,
  originalBehavioralInverseProof: oldProof.pin,
  unchangedProductionPinsReuseOriginalInverseProof: true,
  formatterPreparation: preparation.pin,
  formatterApplication: application.pin,
  preFormatTest: testBeforeFormat.pin,
  appliedTestMatchesActualFormatterCandidateRaw: true,
  originalTestPrefixBytes: originalTests.pin.bytes,
  originalTestPrefixRawExact: true,
  newTestNamesUnchanged: newTests,
  validationHistory: {
    behavioralFocused9: { log: behavioralGreen.pin, actualLogCounts: { passed: 9, failed: 0 }, sourceTestSha256: testBeforeFormat.pin.sha256, preFormat: true },
    firstRustfmt37: { log: fmtFailure.pin, exitCodeReportedByRoot: 1, preservedFailure: true },
    adjacent186: { executionOwnedBy: '/root', rootReported: { passed: 177, failed: 5, ignored: 4 }, claim: 'not passing' },
    baselineFive: { executionOwnedBy: '/root', rootReported: { passed: 0, failed: 5 }, interpretation: 'Same five failures on the original b84d preimages; detailed root receipts remain separate.' },
    finalFormattingAndModuleTests: 'pending root execution after the separately assigned fixture work is frozen',
  },
  limitation: 'Formatting provenance is actual rustfmt output equality, not an asserted whitespace-only character transform; rustfmt can also normalize optional trailing commas. No final focused or adjacent pass is inferred from the pre-format log.',
  testsRunByThisScript: false,
  trackedFilesWrittenByThisScript: false,
  archiveExecuted: false,
  capacityAccepted: false,
  p0Status: 'pending',
  p2Status: 'frozen',
};
const receiptFile = path.join(output, 'source-preservation-format-addendum-v1.json');
await fs.writeFile(receiptFile, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ receipt: (await read(receiptFile)).pin, sourcePins: current.map(item => item.pin), testsRun: false, publicArchiveWritten: false }));
