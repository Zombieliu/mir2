import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
export const BASE = 'C:/mir2-build/p01-completion-20261024';
export const PROJECT = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
export const DIRECTORY = path.join(BASE, 'native-chase-preference-evidence-v1');
export const TOPIC = path.join(PROJECT, 'docs/generated/cell-p0-p1-completion-20261024');
export const CHILD = 'capacity-native-chase-preference-validation-v1';
export const SOURCE_SHA = 'db4e22c69b4f1365d8135023e158f4de00ff35d58330cf7eca98801780e674e7';
export const OLD_FREEZE = 'capacity-native-witness-approach-driver-frozen-v1.json';
export const BACKEND_COMMIT = '46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b';
export const CODE_BEFORE = '4cab00893b861395b63906b1b8183a021d397cdb';
export const FREEZE_FILE = path.join(BASE, 'capacity-native-chase-preference-driver-frozen-v1.json');
export const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
export function safeRelative(value) {
  assert.equal(typeof value, 'string'); assert.ok(value && !path.isAbsolute(value) && !/^[a-z]:/i.test(value));
  const parts = value.split(/[\\/]/); assert.ok(parts.every(part => part && !['.', '..'].includes(part) && !/^(private|secrets?|stores?|recordings?|bin)$/i.test(part)));
  return parts.join('/');
}
function contained(root, file) { const rel = path.relative(root, file); return rel && !path.isAbsolute(rel) && rel !== '..' && !rel.startsWith('..' + path.sep); }
export async function safeFile(root, relative) {
  const normalized = safeRelative(relative), realRoot = await fs.realpath(root), candidate = path.join(root, normalized);
  const stat = await fs.lstat(candidate); assert.ok(stat.isFile() && !stat.isSymbolicLink(), 'A regular, nonsymlink input is required');
  const real = await fs.realpath(candidate); assert.ok(contained(realRoot, real), 'Input resolves outside its allowed root'); return real;
}
export async function bytesUnder(root, relative) { return fs.readFile(await safeFile(root, relative)); }
export async function pinUnder(root, relative) { const bytes = await bytesUnder(root, relative); return { file: safeRelative(relative), bytes: bytes.length, sha256: sha(bytes) }; }
export async function jsonUnder(root, relative) { return JSON.parse((await bytesUnder(root, relative)).toString('utf8')); }
export function assertPin(actual, expected) { assert.equal(actual.bytes, expected.bytes); assert.equal(actual.sha256, expected.sha256); }
export async function checkExternalPin(pin) {
  const rel = path.relative(BASE, pin.file ?? pin.path); const actual = await pinUnder(BASE, rel); assertPin(actual, pin); return actual;
}
export const SOURCES = Object.freeze([
  'native-chase-preference-main-before-v1.mjs.txt', 'native-chase-preference-main-mutation-v1.json', 'apply-native-chase-preference-v1.mjs', 'verify-native-chase-preference-v1.mjs',
  ...['append-receipt-v1.json', 'append-receipt-v2.json', 'append-tests-v1.mjs', 'appended-cases-v1.mjs.txt', 'before-187.test.mjs.txt',
    'before-focused-revision-v2.test.mjs.txt', 'focused-green-v1.json', 'focused-green-v1.log', 'focused-green-v2.json', 'focused-green-v2.log',
    'patch-tests-v2.mjs', 'run-focused-green-v1.mjs', 'run-focused-green-v2.mjs'].map(name => 'native-chase-preference-tests-v1/' + name),
  ...['source-review-v1.json', 'test-review-v1.json', 'test-review-v2.json', 'final-integration-review-v1.json'].map(name => 'native-chase-preference-independent-review-v1/' + name),
  ...['main-current-before-red-v1.mjs.txt', 'old-red-v1.log', 'current-green-v1.log', 'red-green-receipt-v1.json', 'full-current-v1.log', 'full-current-receipt-v1.json'].map(name => 'native-chase-preference-verification-v1/' + name),
]);
export const OWN_FILES = Object.freeze(['evidence-lib.mjs', 'archive-public.mjs', 'freeze-driver.mjs', 'prepare-evidence.mjs', 'prepare-evidence-v2.mjs', 'README.md']);
function counts(log) { return Object.fromEntries(['tests', 'pass', 'fail', 'cancelled', 'skipped'].map(key => { const found = log.match(new RegExp('^# ' + key + ' (\\d+)$', 'm')); assert.ok(found, 'Missing actual TAP footer ' + key); return [key, Number(found[1])]; })); }
function assertPassing(result, expected) { assert.equal(result.exitCode, 0); assert.equal(result.signal ?? null, null); assert.equal(result.error ?? null, null); assert.deepEqual(result.counts, { tests: expected, pass: expected, fail: 0, cancelled: 0, skipped: 0 }); }
export async function validateEvidence() {
  const old = await jsonUnder(BASE, OLD_FREEZE), redGreen = await jsonUnder(BASE, 'native-chase-preference-verification-v1/red-green-receipt-v1.json');
  const full = await jsonUnder(BASE, 'native-chase-preference-verification-v1/full-current-receipt-v1.json');
  assert.equal(old.files.length, 9); const files = [];
  for (const previous of old.files) { const current = await pinUnder(PROJECT, previous.file); files.push(current); if (!/playtest-capacity-soak(?:\.test)?\.mjs$/.test(previous.file)) assertPin(current, previous); }
  assert.equal(files.length, 9); assert.equal(sha(JSON.stringify(files)), SOURCE_SHA);
  for (const receipt of [redGreen, full]) { assert.deepEqual(receipt.sourceBefore, receipt.sourceAfter); assert.deepEqual(receipt.sourceBefore.files, files); assert.equal(receipt.sourceBefore.sourceSha256, SOURCE_SHA); assert.equal(receipt.backendChanged, false); assert.equal(receipt.capacityAccepted, false); }
  assert.equal(full.sourcePinsUnchangedAfterRun, true); assertPassing(full, 200);
  assert.equal(redGreen.currentSourceRestored, true); assert.equal(redGreen.redExpectedBehavioralFailure, true); assert.equal(redGreen.red.exitCode, 1);
  assert.deepEqual(redGreen.red.counts, { tests: 13, pass: 4, fail: 9, cancelled: 0, skipped: 0 }); assertPassing(redGreen.green, 13);
  for (const result of [full, redGreen.red, redGreen.green]) { await checkExternalPin(result.log); assert.deepEqual(counts((await fs.readFile(result.log.file)).toString('utf8')), result.counts); }
  const initial = await jsonUnder(BASE, 'native-chase-preference-tests-v1/focused-green-v1.json'), focused = await jsonUnder(BASE, 'native-chase-preference-tests-v1/focused-green-v2.json');
  assert.equal(initial.tests, 13); assert.equal(initial.passed, 12); assert.equal(initial.failed, 1); assert.equal(initial.exitCode, 1); await checkExternalPin(initial.log);
  assert.equal(focused.tests, 13); assert.equal(focused.passed, 13); assert.equal(focused.failed, 0); assert.equal(focused.exitCode, 0); assert.equal(focused.old187PrefixByteIdentical, true); await checkExternalPin(focused.log);
  const main = await bytesUnder(PROJECT, 'apps/web/scripts/playtest-capacity-soak.mjs'), test = await bytesUnder(PROJECT, 'apps/web/scripts/playtest-capacity-soak.test.mjs');
  const original = await bytesUnder(BASE, 'native-chase-preference-tests-v1/before-187.test.mjs.txt');
  assert.equal(original.length, 224817); assert.equal(sha(original), 'de493f04c78bb961e2a5cfd230ce9c1e4843ec9057edf34ca055edaecaaf7c59'); assert.ok(test.subarray(0, original.length).equals(original));
  const proof = await jsonUnder(BASE, 'native-chase-preference-main-mutation-v1.json'); assert.equal(proof.ruleCount, 8); let inverse = main.toString('utf8');
  for (const rule of [...proof.rules].reverse()) { assert.equal(inverse.split(rule.after).length, 2); inverse = inverse.replace(rule.after, rule.before); }
  assert.ok(Buffer.from(inverse).equals(await bytesUnder(BASE, 'native-chase-preference-main-before-v1.mjs.txt')));
  const revision = await jsonUnder(BASE, 'native-chase-preference-tests-v1/append-receipt-v2.json'); let testInverse = test.toString('utf8');
  for (const rule of [...revision.mutations].reverse()) { assert.equal(testInverse.split(rule.after).length, 2); testInverse = testInverse.replace(rule.after, rule.before); }
  assert.ok(Buffer.from(testInverse).equals(await bytesUnder(BASE, 'native-chase-preference-tests-v1/before-focused-revision-v2.test.mjs.txt')));
  const sourceReview = await jsonUnder(BASE, 'native-chase-preference-independent-review-v1/source-review-v1.json'), testReview = await jsonUnder(BASE, 'native-chase-preference-independent-review-v1/test-review-v2.json');
  assert.equal(sourceReview.verdict.sourceContractPassed, true); assert.deepEqual(sourceReview.verdict.blockingFindingIds, []);
  assert.equal(testReview.verdict.staticTestQualityReviewed, true); assert.deepEqual(testReview.verdict.blockingFindingIds, []);
  // The final independent review is preserved and byte-pinned; its own field
  // names are not interpreted as a second test runner or capacity decision.
  const integrationReview = await pinUnder(BASE, 'native-chase-preference-independent-review-v1/final-integration-review-v1.json');
  assertPin(integrationReview, { bytes: 6013, sha256: 'c92ea8583a1faea74ebd0b1890d9f878d59341e11eb481dc2829760d5d453555' });
  return { old, files, sourceSha256: SOURCE_SHA, redGreen, full, focused, original187: { bytes: original.length, sha256: sha(original), byteIdentical: true }, integrationReview };
}
