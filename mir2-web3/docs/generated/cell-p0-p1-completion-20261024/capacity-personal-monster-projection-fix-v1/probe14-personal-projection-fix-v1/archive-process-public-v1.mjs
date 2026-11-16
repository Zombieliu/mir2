// Explicit, reversible public process archive. No source edits or test execution.
import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';

const base = path.dirname(import.meta.dirname);
const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
const topic = path.join(project, 'docs/generated/cell-p0-p1-completion-20261024');
const childName = 'capacity-personal-monster-projection-fix-v1';
const output = path.join(topic, childName);
const digest = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const entries = [];
function add(relative, destination = relative, expected) { entries.push({ source: path.join(base, relative), destination, expected }); }
function group(directory, files) { for (const file of files) add(directory + '/' + file); }

for (const file of [
  'probe14-personal-journey-red-v1.log', 'probe14-personal-journey-red-v2.log',
  'probe14-personal-journey-red-v3.log', 'probe14-personal-journey-generic-red-v1.log',
  'probe14-personal-journey-green-v1.log', 'probe14-personal-journey-green-v2.log',
  'probe14-personal-projection-rustfmt37-v1.log', 'probe14-personal-projection-rustfmt39-v1.log',
  'probe14-personal-projection-rustfmt39-v2.log', 'probe14-personal-projection-adjacent-v1.log',
  'probe14-personal-projection-baseline-five-v1.log', 'probe14-personal-projection-focused-modules-v1.log',
  'probe14-personal-projection-baseline-overflow-v1.log', 'probe14-personal-projection-overflow-green-v1.log',
]) add(file, 'execution/' + file);
for (const file of [
  'run-capacity-cargo.ps1', 'root-personal-projection-adjacent-v1.ps1',
  'root-personal-projection-baseline-five-v1.mjs', 'root-personal-projection-baseline-five-v1.ps1',
  'root-personal-projection-baseline-overflow-v1.mjs', 'root-personal-projection-baseline-overflow-v1.ps1',
  'root-personal-projection-remaining-adjacent-v2.ps1',
]) add(file, 'root-scripts/' + file);
add('probe14-personal-projection-adjacent-execution-v1.json', 'execution/adjacent-execution-v1.json');
add('root-personal-projection-final-source-inputs-v1.json', 'historical-inputs/root-final-source-inputs-v1.json');
group('probe14-personal-projection-fix-v1', [
  'capture-preimages-v1.mjs', 'capture-candidate-proof-v1.mjs', 'capture-final-format-proof-v1.mjs',
  'candidate-source-preservation-v1.json', 'candidate-three-file-diff-v1.patch',
  'public-whitelist-draft-v1.json', 'root-red-execution-receipt-v1.json',
  'personal-journey-regressions-v1.rs.txt', 'personal-monster-tests-red-v1.rs.txt',
  'personal-monster-tests-red-v2.rs.txt', 'personal-monster-tests-red-v3.rs.txt',
  'personal-monster-tests-green-v1.rs.txt', 'personal-monster-tests-generic-red-v1.rs.txt',
  'routing-green-v1.rs.txt', 'archive-process-public-v1.mjs',
]);
group('probe14-personal-projection-fix-v1/preimages', [
  'manifest-v1.json', 'routing.rs.txt', 'personal_monster_projection_tests.rs.txt', 'zone-runtime.rs.txt',
]);
group('probe14-personal-projection-fix-v1/format-candidate-v1', [
  'original-personal-tests.rs.txt', 'preparation-v1.json', 'rustfmt-candidate-check-v1.log',
]);
add('probe14-personal-projection-fix-v1/format-candidate-v1/personal_monster_projection_tests.rs', 'probe14-personal-projection-fix-v1/format-candidate-v1/personal_monster_projection_tests.rs.txt');
add('probe14-personal-projection-fix-v1/format-candidate-v1/reproduced.rs', 'probe14-personal-projection-fix-v1/format-candidate-v1/reproduced.rs.txt');
group('probe14-personal-projection-fix-v1/format-application-v1', ['application-receipt-v1.json', 'personal_monster_projection_tests-before.rs.txt']);
group('probe14-personal-projection-fix-v1/final-format-candidate-v1', [
  'routing.rs.txt', 'personal_monster_projection_tests.rs.txt', 'zone-runtime.rs.txt',
  'source-preservation-format-addendum-v1.json',
]);
group('probe14-personal-projection-baseline-five-v1', [
  'execution-v1.json', 'routing.rs.txt.candidate', 'personal_monster_projection_tests.rs.txt.candidate', 'zone-runtime.rs.txt.candidate',
]);
group('probe14-personal-projection-baseline-overflow-v1', [
  'execution-v1.json', 'routing.rs.candidate', 'personal_monster_projection_tests.rs.candidate',
  'runtime.rs.candidate', 'live_aoi_order_tests.rs.candidate', 'pending_zone_aoi_tests.rs.candidate',
]);
group('probe14-personal-projection-adjacent-v1', ['existing-adjacent-selection-v1.json', 'prepare-existing-selection-v1.mjs', 'README.md']);
const fixturePins = [
  ['live_aoi_order_tests.rs.before.txt', 26272, '95da0e53b5c0ec844c85a383b327e07130a41d2aa09846272720dfe63874f7d3'],
  ['pending_zone_aoi_tests.rs.before.txt', 12970, '83d1bd558fad894ebeb7862de11eaf81b71f442d7bb57b11ebc2acefb3fa1267'],
  ['preimage-manifest-v1.json', 1190, 'ffa9046ce923aaa20ca433497b67197a7706998c4b3e7cc0d10e43b351e7dca4'],
  ['mutation-proof-v1.json', 7494, '6081c6b4cb325d449705990b9888b72564e5c0b9b0a61b46e6f95365099490a5'],
  ['static-verification-v1.json', 1389, '8f63220d39d0493d0b35128b09d1ccc103ff1aa279793ac01854bddb0ddf06bc'],
  ['README.md', 1001, 'b029d312622ffe045cfce30cd73090d460a5f8280703226702d62784cc8e4f83'],
  ['live_aoi_order_tests.rs.before-overflow-v2.txt', 27740, 'addfedf29cd0c36b2865df2e40b07b1c91cd10130a981d9d5f55447ab94c85af'],
  ['overflow-mutation-proof-v2.json', 3682, 'faaef41c2998e5844930d26708b711c2f693b5329d3ae83a1157e8957758e90e'],
  ['overflow-static-verification-v2.json', 1149, '59c0da1939e816cdceb16de450e8b398e60bb2532dba7f0c81e43dd86fb538ef'],
];
for (const [file, bytes, sha256] of fixturePins) add('probe14-adjacent-fixture-repair-v1/' + file, 'probe14-adjacent-fixture-repair-v1/' + file, { bytes, sha256 });
const currentSources = [
  ['apps/gateway/src/routing.rs', 'routing.rs.txt', 1389827, '9135f2f3a019b04c1bb6ffe3717f730c65632df4e642702057bfbf7a699564de'],
  ['apps/gateway/src/routing/tests/personal_monster_projection_tests.rs', 'personal_monster_projection_tests.rs.txt', 30917, '7d474f50150108175bd96268c2f22270f916bbfdefc8a47e5f4fd0c249ffc0fc'],
  ['apps/simulation/src/runtime/zone/runtime.rs', 'zone-runtime.rs.txt', 749298, 'd19840e87e6b2e718303d90d02a03e8c1c4941e069f0b168d9d253644950f3b6'],
  ['apps/gateway/src/routing/tests/live_aoi_order_tests.rs', 'live_aoi_order_tests.rs.txt', 28187, '0f21bfd95760a94f189c6b95567b0e65f1a0018cf88e90b2342ff753010a7279'],
  ['apps/gateway/src/routing/tests/pending_zone_aoi_tests.rs', 'pending_zone_aoi_tests.rs.txt', 13491, '01a11394c304296aefc450eb128a18a358c3e854c25e6627a2b3ff4484d14716'],
];
for (const [relative, destination, bytes, sha256] of currentSources) entries.push({ source: path.join(project, relative), destination: 'candidate-source-after-fixture-v2/' + destination, expected: { bytes, sha256 } });

const sourceBytes = [];
const seen = new Set();
for (const entry of entries) {
  assert.ok(!seen.has(entry.destination), 'Duplicate destination'); seen.add(entry.destination);
  assert.ok(!path.isAbsolute(entry.destination) && !entry.destination.split('/').includes('..') && !/\.rs$/i.test(entry.destination), 'No unsafe or live Rust archive path');
  assert.ok(!entry.source.split(/[\\/]/).some(part => /^(private|store|recordings|bin|secret|secrets)$/i.test(part)), 'Explicit public source only');
  const stat = await fs.lstat(entry.source); assert.ok(stat.isFile() && !stat.isSymbolicLink());
  const bytes = await fs.readFile(entry.source), pin = { path: entry.source, bytes: bytes.length, sha256: digest(bytes) };
  if (entry.expected) assert.deepEqual({ bytes: pin.bytes, sha256: pin.sha256 }, entry.expected, entry.source);
  sourceBytes.push({ ...entry, bytes, pin });
}
function text(relative) { return sourceBytes.find(entry => entry.source === path.join(base, relative)).bytes.toString('utf8'); }
assert.match(text('probe14-personal-journey-red-v3.log'), /test result: FAILED\. 0 passed; 6 failed/);
assert.match(text('probe14-personal-journey-green-v2.log'), /test result: ok\. 9 passed; 0 failed/);
assert.match(text('probe14-personal-projection-adjacent-v1.log'), /test result: FAILED\. 177 passed; 5 failed; 4 ignored/);
assert.match(text('probe14-personal-projection-focused-modules-v1.log'), /test result: FAILED\. 26 passed; 1 failed; 0 ignored/);
assert.match(text('probe14-personal-projection-overflow-green-v1.log'), /test result: ok\. 1 passed; 0 failed/);
assert.equal(JSON.parse(text('probe14-personal-projection-baseline-five-v1/execution-v1.json')).candidateRestoredExactly, true);
assert.equal(JSON.parse(text('probe14-personal-projection-baseline-overflow-v1/execution-v1.json')).candidateRestoredExactly, true);

const pending = ['Remaining adjacent178 actual run and terminal log', 'Two explicitly isolated quest-cadence cases', 'Final aggregate functional validation and committed44-input source certificate', 'Fresh Release/full16 and renewed capacity profile'];
const whitelist = { schemaVersion: 1, createdAt: new Date().toISOString(), scope: 'Already-ended original bugfix and fixture-maintenance process only; remaining validation pending.', files: sourceBytes.map(entry => ({ source: entry.pin, destination: entry.destination })), pending, originalVerdictsPreserved: true, noCapacityAcceptance: true, rustSourcesMappedToTextOrCandidate: true };
const whitelistFile = path.join(import.meta.dirname, 'public-whitelist-process-v1.json');
const whitelistBytes = Buffer.from(JSON.stringify(whitelist, null, 2) + '\n');
await fs.writeFile(whitelistFile, whitelistBytes, { flag: 'wx' });
sourceBytes.push({ source: whitelistFile, destination: 'public-whitelist-process-v1.json', bytes: whitelistBytes, pin: { path: whitelistFile, bytes: whitelistBytes.length, sha256: digest(whitelistBytes) } });

await fs.mkdir(output); // New public child only, exclusive creation.
const copies = [];
for (const entry of sourceBytes) {
  const destination = path.join(output, entry.destination);
  await fs.mkdir(path.dirname(destination), { recursive: true });
  await fs.writeFile(destination, entry.bytes, { flag: 'wx' });
  const copied = await fs.readFile(destination), sourceAfter = await fs.readFile(entry.source);
  assert.ok(copied.equals(entry.bytes) && sourceAfter.equals(entry.bytes), 'Byte copy/source-after equality');
  copies.push({ source: entry.pin, destination: entry.destination, bytes: copied.length, sha256: digest(copied), sourceAfterMatched: true });
}
const readme = `# Personal/shared monster projection fix: preserved process\n\nThis package preserves the original failed attempts, source preimages, actual controls, candidate source and formatting/fixture-maintenance provenance. P0 remains pending and P2 remains frozen. It does not certify a new Release or capacity pass.\n\nThe final production boundary suppresses stale personal monster projection packets only while they are still pure personal journey output. The generic mixed-origin filter retains its original ObjectMonster-only behavior; real Zone AOI removal and actual owned/unknown monster cases are covered separately. See the [source inverse](probe14-personal-projection-fix-v1/candidate-source-preservation-v1.json), [format addendum](probe14-personal-projection-fix-v1/final-format-candidate-v1/source-preservation-format-addendum-v1.json) and [candidate sources](candidate-source-after-fixture-v2/routing.rs.txt). Original source paths and unchanged byte/SHA mappings are in the [explicit whitelist](public-whitelist-process-v1.json). Rust snapshots use .rs.txt or .candidate names so documentation does not expand the backend Rust closure.\n\n- Red v1 compiled but aborted after the first expected assertion held a mutex during unwinding; it is not six target failures. Red v2 had three target Remove failures and three spawn fixture setup failures. [Red v3](execution/probe14-personal-journey-red-v3.log) had all six target assertion failures with the real personal/Zone preconditions.\n- Initial green8 is historical. A subsequent [generic-boundary red](execution/probe14-personal-journey-generic-red-v1.log) exposed overbroad removal filtering, which was narrowed before the [pre-format final9](execution/probe14-personal-journey-green-v2.log) passed. No current final module pass is inferred from that earlier source snapshot.\n- The original rustfmt37 check failed and is preserved. The two later rustfmt39 checks returned zero and have empty original logs. External formatter output, original fe50 preimage and application proof are retained; old12555-byte test prefix remains exact.\n- [Adjacent186](execution/adjacent-execution-v1.json) actually recorded177 passed,5 failed,4 ignored. The [b84d baseline-five control](probe14-personal-projection-baseline-five-v1/execution-v1.json) reproduced the same five failures and restored the candidate exactly in finally. Ignored cases are not counted as passing.\n- The [focused modules](execution/probe14-personal-projection-focused-modules-v1.log) recorded26 passed and1 overflow expectation failure. The [original-code overflow control](probe14-personal-projection-baseline-overflow-v1/execution-v1.json) reproduced that expectation failure and restored the candidate. The explicitly authorized [fixture-v1 inverse](probe14-adjacent-fixture-repair-v1/mutation-proof-v1.json) and [overflow-v2 inverse](probe14-adjacent-fixture-repair-v1/overflow-mutation-proof-v2.json) preserve the actual maintenance changes; the isolated overflow case then passed. Fixture receipts describe their tool checks and do not claim to have executed root's Cargo logs.\n\nRemaining178, isolated quest2 and the final functional summary were not completed when this version was authored and are excluded. They will require a later explicit append with their real verdicts; no pending run is converted to PASS here. No private account/store, binaries, protocol raw samples, monitor histories or recording payloads are copied.\n`;
await fs.writeFile(path.join(output, 'README.md'), readme, { flag: 'wx' });
const readmeBytes = await fs.readFile(path.join(output, 'README.md'));
const receipt = { schemaVersion: 1, copiedAt: new Date().toISOString(), topicChild: childName, files: copies, copies: copies.length, totalSourceBytes: copies.reduce((sum, entry) => sum + entry.bytes, 0), copiedSourceAggregateSha256: digest(JSON.stringify(copies)), authoredReadme: { path: 'README.md', bytes: readmeBytes.length, sha256: digest(readmeBytes) }, allSourceAfterMatched: true, publicRustSourceFilesAdded: 0, excludedPending: pending, originalVerdictsPreserved: true, testsRun: false, sourceFilesWritten: false, capacityAccepted: false, p0Status: 'pending', p2Status: 'frozen' };
const receiptBytes = Buffer.from(JSON.stringify(receipt, null, 2) + '\n');
const receiptFile = path.join(import.meta.dirname, 'public-copy-process-receipt-v1.json');
await fs.writeFile(receiptFile, receiptBytes, { flag: 'wx' });
await fs.writeFile(path.join(output, 'public-copy-provenance-v1.json'), receiptBytes, { flag: 'wx' });
assert.ok((await fs.readFile(path.join(output, 'public-copy-provenance-v1.json'))).equals(receiptBytes));
for (const match of readme.matchAll(/\]\(([^)]+)\)/g)) await fs.access(path.join(output, match[1]));
console.log(JSON.stringify({ publicDirectory: output, copies: copies.length, sourceBytes: receipt.totalSourceBytes, publicFiles: copies.length + 2, receipt: { path: receiptFile, bytes: receiptBytes.length, sha256: digest(receiptBytes) }, noLiveRustSources: true, finalValidationPending: true }));
