import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
const base = 'C:/mir2-build/p01-completion-20261024';
const pilot = path.join(base, 'ordinary-capacity-bank-prepare-v3');
const project = path.resolve(process.argv[2] ?? process.cwd());
if (path.basename(project) !== 'mir2-web3') throw new Error('Pass the actual mir2-web3 project directory');
const output = path.join(project, 'docs/generated/cell-p0-p1-completion-20261024/capacity-ordinary-economic-v1/bank-prepare-v3');
const names = ['source-lib-v1.mjs', 'copy-source-bank-v1.mjs', 'freeze-prepare-v1.mjs', 'run-owned-prepare-v1.mjs', 'owned-child-v1.mjs', 'create-package-v1.mjs', 'prepare-config-pending-v1.json', 'source-bank-plan-pending-v1.json', 'authoring-syntax-v1.json', 'authoring-syntax-v2.json', 'frozen-source-v1.json', 'root-independent-cleanup-v1.json', 'episode/source-bank-copy-receipt-v1.json', 'episode/preparation-episode-receipt-v1.json', 'episode/prepared-index-v1.json', 'episode/cleanup-v1.json', 'episode/owned-process-v1.json', 'episode/health-before-v1.json', 'episode/health-after-v1.json', 'episode/run-intent-v1.json', 'episode/reports/1793050659530-92aeff5e.preparation-report.json', 'episode/reports/1793050659530-92aeff5e.preparation.jsonl'];
const pairs = names.map(name => ({ source: path.join(pilot, name), name: path.basename(name) }));
const qualificationDirectory = path.join(base, 'root-capacity-preparation-validation-v3');
for (const name of ['root-focused-receipt-v1.json', 'complete-capacity-driver.stdout.log', 'complete-capacity-driver.stderr.log', 'ordinary-economy-and-navigation-dependencies.stdout.log', 'ordinary-economy-and-navigation-dependencies.stderr.log']) pairs.push({ source: path.join(qualificationDirectory, name), name });
pairs.push({ source: path.join(base, 'root-capacity-preparation-validation-runner-v4.mjs'), name: 'root-validation-runner.mjs' });
pairs.push({ source: fileURLToPath(import.meta.url), name: 'archive-evidence-v3.mjs' });
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const read = async file => { const stat = await fs.lstat(file); if (!stat.isFile() || stat.isSymbolicLink()) throw new Error('Expected regular public evidence'); return fs.readFile(file); };
const report = JSON.parse(await read(path.join(pilot, 'episode/reports/1793050659530-92aeff5e.preparation-report.json')));
const receipt = JSON.parse(await read(path.join(pilot, 'episode/preparation-episode-receipt-v1.json')));
const qualification = JSON.parse(await read(path.join(qualificationDirectory, 'root-focused-receipt-v1.json')));
const independent = JSON.parse(await read(path.join(pilot, 'root-independent-cleanup-v1.json')));
const recovery = report.failedActor?.preparationRecovery;
if (report.ok !== false || report.preparedCount !== 4 || !report.error?.endsWith('timed out waiting for preparation HP acknowledgement and exact health cue') ||
    recovery?.status !== 'failed' || recovery.before?.hp !== 12 || recovery.before.maximumHp !== 18 || recovery.before.candidate?.uniqueId !== 2 || recovery.before.candidate.quantity !== 1 || recovery.after?.hp !== 12 || recovery.after.selectedQuantity !== 0 || !recovery.acknowledgement ||
    receipt.cleanupComplete !== true || receipt.sourceFirst4InventoryReuse !== true || receipt.measurementPerformed !== false || receipt.capacityAccepted !== false || receipt.p0Accepted !== false ||
    qualification.expectedHead !== '518ef24b65f5322aef1c20683af7f084012050c7' || qualification.allSelectedTestsPassed !== true || qualification.sourceUnchanged !== true || qualification.sourceBefore?.length !== 31 ||
    independent.ownedPid !== 71364 || independent.ownedProcessAbsent !== true || independent.portsFree !== true || independent.remainingListeners?.length !== 0) throw new Error('Unexpected original verdict, actual recovery evidence, cleanup or old518 source qualification');
const expectedCounts = new Map([['complete-capacity-driver', 519], ['ordinary-economy-and-navigation-dependencies', 301]]);
if (qualification.checks?.length !== 2) throw new Error('Expected exactly two actual Root check groups');
for (const check of qualification.checks) {
  const count = expectedCounts.get(check.name), summary = check.summary;
  if (!count || check.exitCode !== 0 || check.signal != null || check.error != null || summary?.tests !== count || summary.pass !== count || ['fail', 'cancelled', 'skipped', 'todo'].some(key => summary[key] !== 0)) throw new Error('Actual Root820 check group differs');
  for (const label of ['stdout', 'stderr']) { const pin = check[label], bytes = await read(pin.file); if (bytes.length !== pin.bytes || sha(bytes) !== pin.sha256) throw new Error('Root original raw log pin differs'); }
}
const inputs = [];
for (const pair of pairs) { const bytes = await read(pair.source); inputs.push({ ...pair, bytes, sha256: sha(bytes) }); }
if (new Set(inputs.map(input => input.name)).size !== inputs.length) throw new Error('Archive filenames collide');
await fs.mkdir(output);
const copies = [];
for (const input of inputs) {
  const target = path.join(output, input.name); await fs.writeFile(target, input.bytes, { flag: 'wx' });
  const copied = await read(target), after = await read(input.source);
  if (copied.length !== input.bytes.length || sha(copied) !== input.sha256 || after.length !== input.bytes.length || sha(after) !== input.sha256) throw new Error('Archive raw byte verification failed');
  copies.push({ source: input.source, file: input.name, bytes: input.bytes.length, sha256: input.sha256 });
}
const manifest = { schemaVersion: 1,
  scope: 'Actual old518 V3 preparation failure and its actual Root820 qualification only. First4 stock reused via ordinary login, no newly completed fifth actor and zero measured CCU. Original report and failure rows remain unmodified. Candidate JS ObjectHealth cue and backend UseItem tick-wake repairs are not qualified by these historical logs.',
  driverSourceCommit: qualification.expectedHead, backendSourceCommit: receipt.backendSourceCommit,
  freshCapacityToolTestsPassed: 519, freshOrdinaryDependencyTestsPassed: 301, actualRootTestsPassed: 820,
  sourceFirst4InventoryReuse: true, seederInvoked: false, loginNotBeforeOverridden: false,
  preparedCount: 4, targetPrepared: 100, originalRunPassed: false, originalError: report.error,
  preparationComplete: false, normalCleanupComplete: true, coldBankEligible: false, formalOrdinaryRelogin100Verified: false,
  measurementPerformed: false, measuredCcuAcceptancePerformed: false, sustainableTwoHourSupplyProven: false, capacityAccepted: false, p0Accepted: false,
  privacy: 'Explicit public allowlist only; no private Store/pool/ledger/secret contents, recovery journals, raw Gateway logs or complete spectator recordings copied.',
  rawByteVerification: { destinationExact: true, sourcesUnchangedDuringCopy: true, actualRootRawLogPinsMatched: true }, copies };
const manifestBytes = Buffer.from(JSON.stringify(manifest, null, 2) + '\n');
await fs.writeFile(path.join(output, 'copied-evidence-manifest.json'), manifestBytes, { flag: 'wx' });
console.log(JSON.stringify({ files: copies.length, copiedBytes: copies.reduce((n, p) => n + p.bytes, 0), output, manifest: { bytes: manifestBytes.length, sha256: sha(manifestBytes) } }));
