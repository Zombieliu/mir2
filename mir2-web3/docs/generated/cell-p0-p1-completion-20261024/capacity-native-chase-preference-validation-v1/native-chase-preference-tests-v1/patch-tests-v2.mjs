import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
const directory = 'C:/mir2-build/p01-completion-20261024/native-chase-preference-tests-v1';
const file = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3/apps/web/scripts/playtest-capacity-soak.test.mjs';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const before = await fs.readFile(file), original = await fs.readFile(path.join(directory, 'before-187.test.mjs.txt'));
assert.equal(before.length, 238437); assert.equal(sha(before), '2c95cccec9d2aadf3b4a59bc5e48894ade2951fc50c8c26a1ddd48b903ad24b5');
assert.ok(before.subarray(0, original.length).equals(original));
await fs.writeFile(path.join(directory, 'before-focused-revision-v2.test.mjs.txt'), before, { flag: 'wx' });
const mutations = [
  { purpose: 'Finite future peer receipt is ineligible at current selection time; same existing invalid-peer test count.',
    before: "    f => { f.second.combatPursuit.name = 'other-monster'; }]) {",
    after: "    f => { f.second.combatPursuit.name = 'other-monster'; },\n    f => { f.second.combatPursuit.coordinationLastReceiptMonoMs = 2000; }]) {" },
  { purpose: 'Native and baseline have distinct original combat thresholds; prove each unchanged instead of equating them.',
    before: `  const original = f.context.policy; f.context.policy = capacityPolicy({ profile: 'probe' });
  assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, undefined);
  assert.equal(selectNativeAction(f.client, f.context, 1500).hunting, 'Deer');
  for (const field of ['minimumCombatAttacksPerMinute', 'minimumCombatHitsPerMinute', 'minimumNativeRunRatio', 'maximumMissedPlanRatio',
    'maximumCorrectionRatio', 'movementP95Ms', 'maximumDriverLagP99Ms']) assert.equal(original[field], f.context.policy[field]);`,
    after: `  const original = f.context.policy, nativeBefore = structuredClone(original);
  f.context.policy = capacityPolicy({ profile: 'probe' }); const baselineBefore = structuredClone(f.context.policy);
  assert.equal(selectNativeAction(f.client, f.context, 1500).chaseTargetId, undefined);
  assert.equal(selectNativeAction(f.client, f.context, 1500).hunting, 'Deer');
  assert.deepEqual(original, nativeBefore); assert.deepEqual(f.context.policy, baselineBefore);
  assert.equal(original.minimumCombatAttacksPerMinute, 18); assert.equal(original.minimumCombatHitsPerMinute, 12);
  assert.equal(f.context.policy.minimumCombatAttacksPerMinute, 12); assert.equal(f.context.policy.minimumCombatHitsPerMinute, 3);
  for (const field of ['minimumNativeRunRatio', 'maximumMissedPlanRatio', 'maximumCorrectionRatio', 'movementP95Ms',
    'maximumDriverLagP99Ms']) assert.equal(original[field], f.context.policy[field]);` }
];
let text = before.toString('utf8');
for (const mutation of mutations) { assert.equal(text.split(mutation.before).length, 2); text = text.replace(mutation.before, mutation.after); }
const after = Buffer.from(text, 'utf8'); assert.ok(after.subarray(0, original.length).equals(original));
let inverse = text; for (const mutation of [...mutations].reverse()) inverse = inverse.replace(mutation.after, mutation.before);
assert.ok(Buffer.from(inverse, 'utf8').equals(before));
await fs.writeFile(file, after);
await fs.writeFile(path.join(directory, 'append-receipt-v2.json'), JSON.stringify({ schemaVersion: 2,
  scope: 'Only the newly appended 13 focused tests changed; original 187 test bytes and all previous logs preserved.',
  before: { file, bytes: before.length, sha256: sha(before) }, after: { file, bytes: after.length, sha256: sha(after) },
  original187: { bytes: original.length, sha256: sha(original), byteIdentical: true }, exactInverseToTestedV1: true,
  focusedTestCount: 13, mutations }, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ bytes: after.length, sha256: sha(after), old187ByteIdentical: true, exactInverseToTestedV1: true }));
