import fs from 'node:fs/promises'; import { createReadStream } from 'node:fs'; import { createInterface } from 'node:readline'; import path from 'node:path'; import crypto from 'node:crypto'; import assert from 'node:assert/strict';
const base = 'C:/mir2-build/p01-completion-20261024', root = path.join(base, 'release-native-10-v2-chase-preference-feasibility-14'), out = path.join(base, 'probe14-movement-correction-analysis-v1');
const runId = '1792923531277-bc2bda2e', actor = 'L8baceb00', raw = path.join(root, 'reports', runId + '.samples.jsonl'), reportFile = path.join(root, 'reports', runId + '.report.json');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const reportBytes = await fs.readFile(reportFile), report = JSON.parse(reportBytes); assert.equal(report.runId, runId); assert.equal(report.ok, false);
const point = value => value && Number.isSafeInteger(value.x) && Number.isSafeInteger(value.y) ? { x: value.x, y: value.y } : null;
const choose = (value, keys) => Object.fromEntries(keys.filter(key => value?.[key] !== undefined).map(key => [key, value[key]]));
const entity = value => ({ ...choose(value, ['objectId', 'kind', 'dead', 'hidden']), ...point(value) });
const view = value => value ? { ...choose(value, ['map', 'totalCount', 'projectionLimit', 'truncated']), owner: value.owner ? entity(value.owner) : null, entities: (value.entities ?? []).slice(0, 16).map(entity) } : null;
const hint = value => value ? { ...choose(value, ['kind', 'sendCursor', 'sentAtMs']), origin: point(value.origin), first: point(value.first), target: point(value.target), cells: (value.cells ?? []).slice(0, 7).map(point) } : null;
function wireProjection(row, line) { return { line, ...choose(row, ['type', 'schemaVersion', 'connection', 'diagnosticSequence', 'observedAtMs', 'monotonicMs', 'phase', 'event', 'source', 'sequenceBefore', 'sequenceAfter', 'receipt', 'originalOutcome', 'beforePresent', 'afterPresent', 'wireClassification', 'unknownMotion', 'hintCleared', 'pending', 'rememberedIds', 'rememberedEvictions', 'rememberedTruncated', 'priorPendingSuperseded', 'truncated', 'reason']),
  ...(row.wire ? { wire: choose(row.wire, ['objectId', 'x', 'y', 'direction']) } : {}), ...(row.command ? { command: choose(row.command, ['kind', 'direction']) } : {}),
  ...(row.before ? { before: view(row.before) } : {}), ...(row.after ? { after: view(row.after) } : {}),
  ...(row.hint ? { hint: hint(row.hint) } : {}), ...(row.hintBefore ? { hintBefore: hint(row.hintBefore) } : {}), ...(row.hintAfter ? { hintAfter: hint(row.hintAfter) } : {}) }; }
function correctionProjection(row, line) { return { line, ...choose(row, ['type', 'actor', 'connection', 'at', 'monotonicMs', 'sequence', 'phase', 'kind', 'mapFileName', 'ownHp', 'ownPoison', 'playerPoison', 'nearbyOccupantCount', 'occupantsTruncated']), origin: point(row.origin), first: point(row.first), target: point(row.target), actual: point(row.actual), temporaryAvoidance: row.temporaryAvoidance, nearbyOccupants: (row.nearbyOccupants ?? []).slice(0, 16).map(entity) }; }
function actionProjection(row, line) { return { line, ...choose(row, ['type', 'actor', 'phase', 'at', 'kind', 'status', 'sentAtMs', 'scheduledAtMs', 'nativeOpportunity', 'nativeCompleted', 'latencyMs', 'plannedLatencyMs', 'plannedIntervalMs']), plannedTarget: point(row.plannedTarget), actual: point(row.actual), temporaryAvoidance: row.temporaryAvoidance }; }
const digest = crypto.createHash('sha256'), stream = createReadStream(raw), rings = new Map(), cases = [], corrections = [], transforms = [], actionCounts = {}, typeCounts = {}, priorActions = [], lastSnapshots = new Map();
let bytes = 0, rows = 0; stream.on('data', chunk => { bytes += chunk.length; digest.update(chunk); });
for await (const text of createInterface({ input: stream, crlfDelay: Infinity })) {
  rows++; assert.ok(Buffer.byteLength(text) <= 16 * 1024 * 1024); if (!text) continue; const row = JSON.parse(text); typeCounts[row.type] = (typeCounts[row.type] ?? 0) + 1;
  if (row.type === 'compactNavigationWire' && [1, 2].includes(row.connection)) {
    const projected = wireProjection(row, rows); if(projected.source==='worldSnapshot')lastSnapshots.set(row.connection,projected); const ring = rings.get(row.connection) ?? []; ring.push(projected); if (ring.length > 96) ring.shift(); rings.set(row.connection, ring);
    for (const item of cases) if (projected.observedAtMs >= item.action.at && projected.observedAtMs <= item.action.at + 1500 && item.following.length < 48) item.following.push(projected);
  }
  if (row.type === 'movementCorrection' && row.actor === actor) { corrections.push(correctionProjection(row, rows)); assert.ok(corrections.length <= 32); }
  if (row.type === 'selfTransform' && row.actor === actor) {
    const item = { line: rows, ...choose(row, ['type', 'actor', 'connection', 'at', 'sequence', 'monotonicMs', 'phase', 'source', 'mapFileName']), wire: choose(row.wire, ['objectId', 'x', 'y', 'direction']), before: choose(row.before, ['playerObjectId', 'position', 'direction']), after: choose(row.after, ['playerObjectId', 'position', 'direction']) };
    transforms.push(item); if (transforms.length > 12) transforms.shift();
  }
  if (row.type === 'action' && row.actor === actor && row.phase === 'stage-2' && ['walk', 'run'].includes(row.kind)) {
    const key = row.kind + '/' + row.status; actionCounts[key] = (actionCounts[key] ?? 0) + 1;
    const recentActions=[...priorActions];priorActions.push(actionProjection(row,rows));if(priorActions.length>12)priorActions.shift();
    if (row.status !== 'corrected') continue;
    assert.ok(cases.length < 16); const preceding = (rings.get(1) ?? []).filter(item => item.observedAtMs <= row.at).slice(-64), send = [...preceding].reverse().find(item => item.event === 'send' && item.hint?.sentAtMs === row.sentAtMs);
    const matched = [...corrections].reverse().find(item => item.at <= row.at && row.at - item.at < 100 && item.kind === row.kind && JSON.stringify(item.target) === JSON.stringify(point(row.plannedTarget)));
    const receipt = send ? [...preceding].reverse().find(item => item.event === 'observe' && item.source === 'UserLocation' && item.receipt?.monotonicMs >= row.sentAtMs && item.sequenceAfter > send.sequenceBefore) : null;
    cases.push({ number: cases.length + 1, action: actionProjection(row, rows), movementCorrection: matched ?? null, matchedSend: send ?? null, matchedOwnerReceipt: receipt ?? null, preceding, following: [], recentActualSelfTransforms: [...transforms], previousPlannedMovements:recentActions, lastFullSnapshot:lastSnapshots.get(1)??null });
  }
}
const rawPin = { file: raw, bytes, rows, sha256: digest.digest('hex') }; assert.equal(bytes, 5219958); assert.equal(rows, 4314); assert.equal(rawPin.sha256, '0ebbc11aab62b1db86436117268037002f9687031e32d3e26527e6011feca1c1');
assert.equal(cases.length,5); assert.deepEqual(cases.map(item=>item.action.line),[1642,3473,3482,3887,3897]);
const summaryCases = cases.map(item => {
  const h = item.matchedSend?.hint ?? item.movementCorrection, occupied = p => (item.matchedSend?.before?.entities ?? []).filter(e => e.x === p?.x && e.y === p?.y), owner = item.matchedSend?.before?.owner;
  return { number: item.number, insideFinalFailingWindow:item.number>1, actionLine: item.action.line, at: item.action.at, kind: item.action.kind, sentAtMs: item.action.sentAtMs,
    origin: point(h?.origin ?? owner), first: point(h?.first), target: point(h?.target), actual: point(item.action.actual),
    sourceCorrectionLine: item.movementCorrection?.line ?? null, sendLine: item.matchedSend?.line ?? null, receiptLine: item.matchedOwnerReceipt?.line ?? null,
    packetSequence: item.matchedOwnerReceipt?.receipt?.sequence ?? item.movementCorrection?.sequence ?? null,
    actualOwnerAfterPacket: item.matchedOwnerReceipt?.after?.owner ?? null,
    atSendLocalFirstOccupants: occupied(h?.first), atSendLocalTargetOccupants: occupied(h?.target),
    nearbyAfterReceipt: item.movementCorrection?.nearbyOccupants ?? [],
    localWireViewTruncated: item.matchedSend?.before?.truncated ?? null, diagnosticWindow: item.matchedSend?.pending?.diagnosticWindow ?? null,
    evidenceScope: 'The exact client send/observe folds and public correction projection; not a simultaneous authoritative occupancy census.' };
});
const selected = { schemaVersion: 1, runId, actor, raw: rawPin, report: { file: reportFile, bytes: reportBytes.length, sha256: hash(reportBytes) }, caseCount: cases.length,
  caps: { precedingWireRowsPerCase: 64, followingRowsPerCase: 48, followingElapsedMs: 1500, recentChangedOwnerTransforms: 12 }, cases };
const summary = { schemaVersion: 1, runId, actor, raw: rawPin, report: selected.report, originalReportOk: false, originalCapacityAccepted: false,
  firstFailure: report.continuousAcceptance.firstFailure.evidence, stage2MovementCounts: actionCounts, cases: summaryCases,
  limitations: ['No server-authoritative simultaneous occupancy or rejection reason is present in these ordinary wire traces.', 'No spectator recording or private file was read.', 'Only the fixed241..244×594..599 diagnostic region and previously remembered non-player IDs are covered.', 'A same-position UserLocation can be observed by the wrapper even though the core selfTransform stream intentionally omits unchanged transforms.', 'Client snapshot membership is neither a global live-monster census nor proof of authority ordering.'],
  rawSingleStreamingPass: false, analysisRawFullReads:2, firstReadStoppedAtWrongFourTotalCasesAssertion:true, perExecutionRawPasses:1, typeCounts, sourceChanged: false, thresholdsChanged: false, capacityAccepted: false };
await fs.writeFile(path.join(out, 'selected-corrections-v2.json'), JSON.stringify(selected, null, 2) + '\n', { flag: 'wx' });
await fs.writeFile(path.join(out, 'summary-v2.json'), JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ raw: rawPin, actionCounts, cases: summaryCases }));
