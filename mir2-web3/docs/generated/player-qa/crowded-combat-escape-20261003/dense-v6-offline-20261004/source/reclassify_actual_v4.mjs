// Offline reclassification only: reads already downloaded, redacted public
// v4 receipts. No network, Gateway, private store, keys or runtime mutation.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { ACCEPTANCE_FALSE, deadActionProof } from './dense_observation.mjs';

const root = dirname(fileURLToPath(import.meta.url));
const execution = resolve(root, '../dense-network-qa-v4-execution-01');
const output = join(root, 'ACTUAL-V4-RECLASSIFIED-02.json');
const sha = value => createHash('sha256').update(value).digest('hex');
const json = path => JSON.parse(readFileSync(path, 'utf8').replace(/^\uFEFF/, ''));
const index = json(join(execution, 'PUBLIC-EXPORT-01.json'));
const downloaded = json(join(execution, 'DOWNLOADED-RECEIPTS-01.json'));
assert.equal(downloaded.binarySha256, '1ed52738885292b734c4055a9fd7d585b79d519779084bca2a617d0a9f5eb9ab');
assert.equal(downloaded.frozenInputsSha256, '054689ac86e4b19f98bc836b1d783a5f6a60d08e0459183a233bb72891809192');
assert.equal(index.privateStoresKeysOrRawGatewayLogsIncluded, false);
assert.equal(downloaded.privateStoresKeysOrRawGatewayLogsIncluded, false);
const verifiedInputs = [];
function verified(relativePath) {
  const entry = index.files.find(e => e.relativePath === relativePath);
  assert.ok(entry, 'Public export lacks ' + relativePath);
  const bytes = readFileSync(join(execution, 'downloaded-public', relativePath));
  assert.equal(bytes.length, entry.bytes); assert.equal(sha(bytes), entry.sha256);
  verifiedInputs.push(entry);
  return bytes.toString('utf8').replace(/^\uFEFF/, '');
}
const original = JSON.parse(verified('public-receipts/death-receipts/report.json'));
assert.equal(original.binarySha256, downloaded.binarySha256);
assert.equal(original.sourceRevision, '6032ef8b3e27dd97bad0b20c8676ef9f185db64b');
const classes = [];
for (const c of original.classes) {
  const previous = c.death?.deadActions;
  if (!previous) {
    classes.push({ className: c.className, originalReportedStatus: c.status, status: 'inconclusive', reason: c.death?.reason ?? null, actualDeathQualified: false });
    continue;
  }
  const events = verified(`public-receipts/death-receipts/${c.className}.jsonl`).trim().split(/\r?\n/).map(line => JSON.parse(line));
  const fingerprint = c.death.deadFingerprint;
  const args = { deathSequence: c.death.death.deathSequence, commandsEndSequence: Math.max(...previous.attemptedCommands.map(e => e.sequence)), ...previous.window,
    spell: previous.castBarrier.spell, before: { map: fingerprint.map, x: fingerprint.x, y: fingerprint.y, direction: fingerprint.direction } };
  const proof = deadActionProof(events, args);
  assert.equal(proof.status, 'failed', c.className + ' actual R17 dead input effects must remain failed');
  assert.equal(proof.castBarrier.status, 'failed'); assert.equal(proof.castBarrierVerified, false);
  assert.ok(proof.absoluteMpSpendReceipts.length > 0);
  const mpRows = proof.absoluteMpSpendReceipts.map(e => ({ sequence: e.sequence, tsMs: e.tsMs, mp: e.state.mp, ownerId: e.state.ownerId, hp: e.state.hp, dead: e.state.dead, provenance: e.state.mpProvenance }));
  classes.push({ className: c.className, actualDeathQualified: c.death.death.status === 'qualified', status: 'failed',
    originalReported: { classStatus: c.status, deadActionsStatus: previous.status, castBarrierStatus: previous.castBarrier.status, castBarrierVerified: previous.castBarrierVerified },
    window: proof.window, inputSequence: proof.castBarrier.actualMagicInputSequence, baselineSequence: proof.castBarrier.baselineSnapshotSequence,
    matchingCastAckCount: proof.matchingCastReceipts.length, absoluteMpSpend: mpRows,
    unattributedTargetDamage: proof.unattributedTargetDamageReceipts.map(e => ({ sequence: e.sequence, tsMs: e.tsMs, ...e.payload })),
    targetDamageSourceAttributed: false,
    ownerLifeMutations: proof.ownerLifeMutationReceipts.map(e => ({ sequence: e.sequence, tsMs: e.tsMs, packet: e.packet ?? e.type, map: e.state.map, x: e.state.x, y: e.state.y, direction: e.state.direction, hp: e.state.hp, dead: e.state.dead })),
    materialConsumptionCount: proof.materialConsumptionReceipts.length,
    unresolvedOwnerHpIncreaseCount: proof.ownerHpIncreaseReceipts.length,
    sourceLevelChangedCount: proof.sourceLevelUpContext.levelChangedReceipts.length,
    separatelyObservedLifecycle: { deadSaveReloginMatched: c.deadPersistenceMatched === true, explicitTownReviveAlive: c.revival?.actualAlive === true,
      revivedSaveReloginMatched: c.revivedPersistenceMatched === true, quietStatus: c.revival?.stop?.status ?? null },
    proof });
}
const receipt = { schema: 'mir2.actual-r17-v4-offline-reclassification.v6', createdUtc: new Date().toISOString(),
  sourceRevision: original.sourceRevision, binarySha256: original.binarySha256,
  v4FrozenManifestSha256: downloaded.frozenInputsSha256, classifierSha256: sha(readFileSync(join(root, 'dense_observation.mjs'))),
  originalPublicExportSha256: sha(readFileSync(join(execution, 'PUBLIC-EXPORT-01.json'))), verifiedPublicInputs: verifiedInputs,
  rawV4FilesAndReportedPassesPreserved: true, noGatewayExecuted: true, noNetworkAccess: true, privateStoresOrKeysRead: false,
  realFreshGameplayRetest: false, sevenMonsterMechanicsAccepted: false, mechanicsAllCasesPassed: false,
  interpretation: 'Both ordinary learned caster dead Magic inputs independently spent absolute MP without animation ACK. Victim-only target damage stays unattributed, and HP increase with Dead=true/unknown would be inconclusive source-refill context. Neither ambiguity appears as an alternative to the actual mana failure. Separate actual death/save/revival/quiet evidence does not erase either dead-cast failure. This only reclassifies the immutable original public trace.',
  classes, ...ACCEPTANCE_FALSE };
writeFileSync(output, JSON.stringify(receipt, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ output, sha256: sha(readFileSync(output)), classes: classes.map(c => ({ className: c.className, status: c.status, originalReported: c.originalReported,
  mp: c.absoluteMpSpend?.map(e => [e.sequence, e.mp]), unattributedTargetDamageSequences: c.unattributedTargetDamage?.map(e => e.sequence), lifeMutationSequences: c.ownerLifeMutations?.map(e => e.sequence), lifecycle: c.separatelyObservedLifecycle })), noGatewayExecuted: true }));
