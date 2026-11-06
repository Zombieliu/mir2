// Offline only. Read immutable, already downloaded public receipts and frozen pure helpers.
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { deadActionProof } from '../dense-network-r18-inputs-01/dense_observation.mjs';
import { sourceWorldObservation, normalLogoutProof, sameOnlineCorpseProof, freshLoadProof, sourceLoadQuietProof } from '../dense-network-r18-inputs-01/source_load_observation.mjs';

const root = dirname(fileURLToPath(import.meta.url));
const inputs = resolve(root, '../dense-network-r18-inputs-01');
const publicRoot = join(root, 'downloaded-public');
const sha = b => createHash('sha256').update(b).digest('hex');
const json = p => JSON.parse(readFileSync(p, 'utf8'));
const manifest = json(join(root, 'PUBLIC-EXPORT-01.json'));
const verifiedInputs = [];
function verified(relativePath) {
  const entry = manifest.files.find(e => e.relativePath === relativePath);
  assert.ok(entry);
  const b = readFileSync(join(publicRoot, relativePath));
  assert.equal(b.length, entry.bytes); assert.equal(sha(b), entry.sha256);
  verifiedInputs.push(entry);
  return b.toString('utf8');
}
const trace = name => verified(`public-receipts/death-receipts/${name}.jsonl`).trim().split(/\r?\n/).map(JSON.parse);
const report = JSON.parse(verified('public-receipts/death-receipts/report.json'));
assert.equal(report.sourceRevision, '321316b791120a6fe549e4006623e63333a5543f');
assert.equal(report.binarySha256, '4064f6bce2337bbd1d251b37e7219163fd524828ddb83d90031232a0b1e25ca8');
assert.equal(sha(readFileSync(join(inputs, 'dense_observation.mjs'))), 'f78103fb5ddb1516caac0f0b088bacd01421c5d9ede603a5c75a3c2b2c95dc0d');
assert.equal(sha(readFileSync(join(inputs, 'FROZEN-INPUTS.json'))), 'b7ac4b4f33cc746248a6e6c0f2d9d97a5822fc4be5e953e5f1e08aad34dfc0c0');
const classes = [];
for (const c of report.classes) {
  if (!c.death.deadActions) {
    assert.equal(c.className, 'Warrior'); assert.equal(c.death.status, 'inconclusive');
    classes.push({ className: c.className, actualDeath: c.death.death, status: 'inconclusive', noFreshLoadExecuted: !c.freshLoad.executed });
    continue;
  }
  const events = trace(c.className), freshEvents = trace(`${c.className}-fresh-load`);
  const old = c.death.deadActions, fp = c.death.deadFingerprint;
  const conn = c.death.terminalOnlineObservation.connectionId;
  assert.ok(events.every(e => e.connectionId === conn));
  const proof = deadActionProof(events, { deathSequence: c.death.death.deathSequence,
    commandsEndSequence: c.death.commandsEndSequence, ...old.window,
    before: { map: fp.map, x: fp.x, y: fp.y, direction: fp.direction }, spell: old.castBarrier.spell });
  assert.deepEqual(proof, old, `${c.className}: immutable dead proof must be exactly reproducible`);
  const terminal = sourceWorldObservation(events.find(e => e.sequence === c.death.terminalOnlineObservation.receiptSequence), c.death.terminalOnlineObservation.window);
  assert.deepEqual(terminal, c.death.terminalOnlineObservation);
  const boundary = normalLogoutProof(events, c.normalLogout.window);
  assert.deepEqual(boundary, c.normalLogout);
  assert.deepEqual(sameOnlineCorpseProof(terminal), c.sameOnlineCorpse);
  const fresh = sourceWorldObservation(freshEvents.find(e => e.sequence === c.freshLogin.sourceBootstrap.receiptSequence), c.freshLogin.sourceBootstrap.window);
  assert.deepEqual(fresh, c.freshLogin.sourceBootstrap);
  assert.ok(freshEvents.every(e => e.connectionId === fresh.connectionId));
  const bind = c.freshLoad.expectedBindPoint;
  const load = freshLoadProof({ terminal, boundary, fresh,
    expectedBindPoint: { map_file_name: bind.map, position: { x: bind.x, y: bind.y } }, terminalLocationValid: null });
  assert.equal(load.status, c.freshLoad.status);
  assert.deepEqual(load.fresh, c.freshLoad.fresh);
  assert.equal(load.sourceFreshLoadContext, true);
  const scope = c.freshQuiet.window;
  const anchorEvent = freshEvents.findLast(e => e.direction === 'received' && e.type === 'worldSnapshot' && e.sequence <= scope.afterSequence);
  const anchor = sourceWorldObservation(anchorEvent, { connectionId: fresh.connectionId, ownerId: fresh.ownerId,
    afterSequence: c.freshLogin.startAckSequence, fromMs: c.freshLogin.startAckMs, endMs: scope.fromMs });
  const quiet = sourceLoadQuietProof(freshEvents, { anchor, scope, drift: c.freshQuiet.drift });
  assert.deepEqual(quiet, c.freshQuiet);
  assert.ok(proof.window.endMs < terminal.receiptMs && terminal.receiptMs <= boundary.ackMs && boundary.closedMs < fresh.receiptMs);
  assert.equal(proof.status, 'passed'); assert.equal(proof.castBarrierVerified, true);
  assert.equal(c.retainedResume.executed, false); assert.equal(c.onlineTownRevive.executed, false);
  classes.push({ className: c.className, status: proof.status,
    originalDeadWindowReproducedExactly: true, terminalWorldReproducedExactly: true, normalLogoutReproducedExactly: true,
    freshWorldReproducedExactly: true, freshQuietReproducedExactly: true,
    deadWindow: proof.window, actualMagicInputSequence: proof.castBarrier.actualMagicInputSequence,
    baselineSequence: proof.castBarrier.baselineSnapshotSequence,
    absoluteMpObservationSequences: proof.castBarrier.absoluteMpObservationSequences,
    absoluteMpSpendCount: proof.absoluteMpSpendReceipts.length, materialConsumptionCount: proof.materialConsumptionReceipts.length,
    matchingCastCount: proof.matchingCastReceipts.length, unattributedTargetDamageCount: proof.unattributedTargetDamageReceipts.length,
    ownerHpAmbiguityCount: proof.ownerHpIncreaseReceipts.length, ownerLifeMutationCount: proof.ownerLifeMutationReceipts.length,
    sameOnlineCorpse: { sequence: terminal.receiptSequence, hp: terminal.hp, explicitDead: terminal.explicitDead },
    normalLogout: boundary, freshLoad: { status: load.status, connectionId: fresh.connectionId, sequence: fresh.receiptSequence,
      hp: fresh.hp, mp: fresh.mp, maxHp: fresh.maxHp, maxMp: fresh.maxMp, explicitDead: fresh.explicitDead, location: fresh.location },
    quiet: { status: quiet.status, window: quiet.window, durationMs: quiet.drift.durationMs, authorityCount: quiet.drift.authoritySampleCount },
    freshLifeReceiptsDoNotChangeClosedDeadVerdict: true, directPrivateStoreInspection: false, directDurableHp0Verified: false });
}
const output = join(root, 'OFFLINE-CLASSIFIER-REVIEW-01.json');
const result = { schema: 'mir2.r18-offline-closed-window-recheck.v1', createdUtc: new Date().toISOString(),
  sourceRevision: report.sourceRevision, binarySha256: report.binarySha256,
  frozenInputsSha256: sha(readFileSync(join(inputs, 'FROZEN-INPUTS.json'))), classifierSha256: sha(readFileSync(join(inputs, 'dense_observation.mjs'))),
  noGatewayExecuted: true, noNetworkAccess: true, immutableOriginalFilesPreserved: true, privateStoresKeysOrRawLogsRead: false,
  verificationComplete: true, verifiedInputs, classes, sevenMonsterMechanicsAccepted: false,
  fullP1Accepted: false, nativeInputAccepted: false, nativeVisualAccepted: false, humanAccepted: false,
  ordinaryProgressionAccepted: false, loadAccepted: false, capacityAccepted: false };
writeFileSync(output, JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ output, sha256: sha(readFileSync(output)), classes: classes.map(c => ({ className: c.className, status: c.status, durationMs: c.quiet?.durationMs })) }));
