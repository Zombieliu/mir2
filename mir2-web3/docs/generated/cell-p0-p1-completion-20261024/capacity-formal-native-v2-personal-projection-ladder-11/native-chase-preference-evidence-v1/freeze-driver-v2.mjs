import fs from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { BASE, PROJECT, DIRECTORY, TOPIC, CHILD, SOURCE_SHA, BACKEND_COMMIT, CODE_BEFORE, FREEZE_FILE, pinUnder, bytesUnder, jsonUnder, sha, assertPin, validateEvidence } from './evidence-lib.mjs';
const args = process.argv.slice(2); assert.equal(args.length, 1); const match = /^--bind-code-commit=([0-9a-f]{40})$/.exec(args[0] ?? '');
assert.ok(match, 'Only a real post-commit --bind-code-commit=<40hex> can create the first immutable driver freeze'); const commit = match[1];
assert.ok(![CODE_BEFORE, BACKEND_COMMIT, '6d70d02d0bc390156b77a3e48acee714fc87cda3'].includes(commit), 'The actual new code commit cannot be a historical evidence commit');
const repository = path.dirname(PROJECT), projectPrefix = path.basename(PROJECT) + '/';
function git(...argv) { const result = spawnSync('git', argv, { cwd: repository, encoding: null, windowsHide: true, maxBuffer: 8 * 1024 * 1024 }); assert.equal(result.status, 0, (result.stderr ?? Buffer.alloc(0)).toString('utf8')); return result.stdout; }
function filteredObject(input, file) { const result = spawnSync('git', ['hash-object', '--path=' + file, '--stdin'], { cwd: repository, input, encoding: null, windowsHide: true, maxBuffer: 1024 * 1024 }); assert.equal(result.status, 0, (result.stderr ?? Buffer.alloc(0)).toString('utf8')); return result.stdout.toString('utf8').trim(); }
assert.equal(git('rev-parse', 'HEAD').toString('utf8').trim(), commit); assert.equal(git('status', '--porcelain', '--untracked-files=no').toString('utf8').trim(), '', 'Tracked tree must be clean after the code/evidence commit');
git('merge-base', '--is-ancestor', CODE_BEFORE, commit); git('merge-base', '--is-ancestor', BACKEND_COMMIT, commit);
const verified = await validateEvidence();
const gitBindings = [];
for (const file of verified.files) { const gitPath = projectPrefix + file.file, current = await bytesUnder(PROJECT, file.file), filtered = filteredObject(current, gitPath), committed = git('rev-parse', commit + ':' + gitPath).toString('utf8').trim(); assert.equal(filtered, committed, 'Git-attribute-filtered raw source does not map to the committed code object');
  gitBindings.push({ file: file.file, rawBytes: current.length, rawSha256: sha(current), gitAttributeFilteredObject: filtered, committedGitObject: committed, committedBlobBytes: git('show', commit + ':' + gitPath).length, committedBlobSha256: sha(git('show', commit + ':' + gitPath)), rawWorktreeRewritten: false }); }
assertPin(await pinUnder(BASE, 'backend-source-socket-diagnostics-v1.json'), { bytes: 14000, sha256: '4da9d4dac73773bae041c98b641874a9ad4427217c1b36bb453f1a014ad7b058' });
const backend = await jsonUnder(BASE, 'backend-source-socket-diagnostics-v1.json'); assert.equal(backend.files.length, 41);
async function pinManifestBackendSource(file) {
  // This exact reviewed Rust source path is not an executable binary. The
  // generic public-copy guard still rejects every arbitrary bin/private path.
  if (file !== 'mir2-web3/apps/gateway/src/bin/seed_capacity_accounts.rs') return pinUnder(repository, file);
  const candidate = path.join(repository, file), stat = await fs.lstat(candidate), root = await fs.realpath(repository), real = await fs.realpath(candidate), relative = path.relative(root, real);
  assert.ok(stat.isFile() && !stat.isSymbolicLink());
  assert.ok(relative && !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('..' + path.sep), 'Exact Rust source must resolve within the repository');
  const bytes = await fs.readFile(real); return { file, bytes: bytes.length, sha256: sha(bytes) };
}
for (const file of backend.files) { assertPin(await pinManifestBackendSource(file.path), file); assert.ok(git('show', commit + ':' + file.path).equals(git('show', BACKEND_COMMIT + ':' + file.path)), 'Backend changed after its immutable build'); }
const backendDiff = git('diff', '--name-only', BACKEND_COMMIT, commit, '--', ':(glob)**/*.rs', ':(glob)**/Cargo.toml', ':(glob)**/Cargo.lock', '.cargo').toString('utf8').trim(); assert.equal(backendDiff, '', 'No backend source/config diff is allowed');
const archive = await jsonUnder(DIRECTORY, 'archive-execution-v1.json'), whitelist = await jsonUnder(DIRECTORY, 'archive-whitelist-final-v1.json'); assert.equal(archive.sourceSha256, SOURCE_SHA);
for (const entry of whitelist.files) { assertPin(await pinUnder(path.join(TOPIC, CHILD), entry.target), entry); git('cat-file', '-e', commit + ':' + projectPrefix + 'docs/generated/cell-p0-p1-completion-20261024/' + CHILD + '/' + entry.target); }
const declarations = Object.fromEntries(['navigationCoordination', 'nativePatrolPlanning', 'measurementClock', 'nativeHpMaintenance', 'nativeAttackWitness'].map(key => { assert.ok(verified.old[key]); return [key, verified.old[key]]; }));
const nativeChasePreference = { freshReceiptMs: 3000,
  scope: 'Client-side preference within the original first4 candidates, not an exclusive claim or a capacity result.',
  evidenceRule: 'Only fresh matching nonzero chase UserLocation receipts establish monotonic first/last ACK priority; active living same-map fighters and live targets only.',
  selectionRule: 'Keep adjacent witnessed attacks first, then prefer targets without an earlier recent pursuer; retain every original first4 candidate and path fallback.',
  budgetRule: 'Original4-target/8-goal/4000-node searches, cadence, offered/completed accounting and finite chase budgets are unchanged.',
  restoreRule: 'Resume clears receipt preference only; original chase elapsed time, ACK count and visited positions remain.' };
const unchanged = verified.files.filter(file => !/playtest-capacity-soak(?:\.test)?\.mjs$/.test(file.file));
const evidencePin = async relative => { const value = await pinUnder(BASE, relative); return { file: path.join(BASE, relative), bytes: value.bytes, sha256: value.sha256 }; };
const fullReceipt = await evidencePin('native-chase-preference-verification-v1/full-current-receipt-v1.json'), redGreenReceipt = await evidencePin('native-chase-preference-verification-v1/red-green-receipt-v1.json');
const freeze = { schemaVersion: 1, frozenAt: new Date().toISOString(), state: 'final-functional-source-frozen-post-code-commit',
  scope: 'Client-only, nonexclusive native chase preference within the original first4 candidate window. Functional200 tests and original thresholds are preserved; no new real game capacity claim.',
  repositoryBaselineCommit: CODE_BEFORE, actualCodeCommit: commit, driverSourceCommit: commit, backendSourceCommit: BACKEND_COMMIT, rustBackendModified: false,
  freezeBuilder: { revision: 2, script: { ...(await pinUnder(DIRECTORY, 'freeze-driver-v2.mjs')), file: path.join(DIRECTORY, 'freeze-driver-v2.mjs') }, mutationProof: { ...(await pinUnder(DIRECTORY, 'freeze-driver-v2-mutation-proof-v1.json')), file: path.join(DIRECTORY, 'freeze-driver-v2-mutation-proof-v1.json') }, originalBuilderUnchanged: true },
  sourceSha256: SOURCE_SHA, files: verified.files, changedFiles: verified.files.filter(file => !unchanged.includes(file)), otherSevenInputsUnchanged: true, otherSevenInputs: unchanged,
  gitSourceBindings: gitBindings, gitBindingRule: 'git hash-object --path=<repository path> --stdin applies the actual Git attributes to the tested raw bytes. Raw evidence and committed canonical objects are separately recorded; no worktree byte is normalized or rewritten.',
  original187: verified.original187, newFocusedTests: 13, functionalVerificationOnly: true,
  focused: { passed: 13, failed: 0, cancelled: 0, skipped: 0, exactSourceSha256: SOURCE_SHA, log: verified.redGreen.green.log, receipt: redGreenReceipt },
  redGreen: { originalMain: verified.redGreen.temporaryOriginal, originalPassed: 4, originalFailed: 9, currentPassed: 13, finallyRestored: true, receipt: redGreenReceipt },
  fullRegression: { passed: 200, failed: 0, cancelled: 0, skipped: 0, durationMs: verified.full.elapsedMs, exactSourceSha256: SOURCE_SHA,
    exactSourceManifest: fullReceipt, log: verified.full.log, receipt: fullReceipt, finalSourceFullRegressionPerformed: true, sourcePinsUnchangedAfterRun: true },
  ...declarations, nativeChasePreference,
  evidenceArtifacts: await Promise.all(['native-chase-preference-main-mutation-v1.json', 'native-chase-preference-tests-v1/append-receipt-v2.json',
    'native-chase-preference-independent-review-v1/source-review-v1.json', 'native-chase-preference-independent-review-v1/test-review-v1.json',
    'native-chase-preference-independent-review-v1/test-review-v2.json', 'native-chase-preference-independent-review-v1/final-integration-review-v1.json'].map(evidencePin)),
  validationArchive: { topicChild: CHILD, committedIn: commit, whitelist: await evidencePin('native-chase-preference-evidence-v1/archive-whitelist-final-v1.json'), copyReceipt: { ...archive.copyReceipt, file: path.join(TOPIC, CHILD, 'public-copy-receipt-v1.json') } },
  originalPolicyUnchanged: { nativeAttacksPerMinute: 18, nativePositiveHitsPerMinute: 12, minimumRunRatio: 0.5, movementIntervalMs: 750, movementP95Ms: 750, originalSearchBudget: '4 targets ×8 goals ×4000 nodes' },
  capacityAccepted: false, realCapacityRunPerformedByThisFreeze: false, p2ImplementationPermitted: false,
  limitations: ['A planning preference is not an exclusive reservation: two fighters may choose the same target before either receives a qualifying ACK.', 'TTL is optional client planning evidence; original authoritative routing, combat outcomes and all offered/completed accounting remain.', 'The external wire wrapper28 and adapter8 retain separate original evidence; these200 core tests do not claim to cover either.', 'No original failed probe/formal report is changed or promoted.'] };
await fs.writeFile(FREEZE_FILE, JSON.stringify(freeze, null, 2) + '\n', { flag: 'wx' });
const result = { freeze: await pinUnder(BASE, path.basename(FREEZE_FILE)), actualCodeCommit: commit, sourceSha256: SOURCE_SHA, backendInputCount: 41, otherSevenInputsUnchanged: true, old187ByteIdentical: true, fullActual: 200, capacityAccepted: false };
await fs.writeFile(path.join(DIRECTORY, 'driver-freeze-execution-v1.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' }); console.log(JSON.stringify(result));
