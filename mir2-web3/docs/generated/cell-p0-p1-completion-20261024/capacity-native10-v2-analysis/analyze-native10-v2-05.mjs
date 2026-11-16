// Read-only posthoc projection of the already-stopped05 run. Never imports game
// modules, opens sockets, changes verdicts, or reads private credentials/logs.
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { createInterface } from 'node:readline';

const base = 'C:/mir2-build/p01-completion-20261024', output = path.join(base, '05-analysis');
const root = path.join(base, 'release-native-10-v2-feasibility-05'), runId = '1792867932194-8f125fe4';
const reportFile = path.join(root, 'reports', runId + '.report.json'), rawFile = reportFile.replace('.report.json', '.samples.jsonl');
const recordingFile = path.join(root, 'recordings/0-498018.jsonl'), fighter = 'Ldd155c05', observer = 'Ldd155c04';
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const pin = async file => { const bytes = await fsp.readFile(file); return { file, bytes: bytes.length, sha256: sha(bytes) }; };
const read = async file => JSON.parse((await fsp.readFile(file, 'utf8')).replace(/^\uFEFF/, ''));
const writeNew = (name, value) => fsp.writeFile(path.join(output, name), JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });
const check = (condition, message) => { if (!condition) throw new Error(message); };
const cheb = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const report = await read(reportFile), sourcePins = await read(path.join(root, 'source-pins-admitted-v2.json'));
const neededSource = ['apps/web/scripts/playtest-capacity-soak.mjs', 'apps/web/scripts/quest-agent/protocol-navigation.mjs', 'apps/web/scripts/quest-agent/protocol-observation.mjs'];
const verifiedCurrentSource = [];
for (const relative of neededSource) {
  const file = path.join('C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3', relative);
  const before = sourcePins.files.find(row => path.resolve(row.file).toLowerCase() === path.resolve(file).toLowerCase()), current = await pin(file);
  check(before && current.bytes === before.bytes && current.sha256 === before.sha256, 'Current reviewed source differs from the real05 preimage');
  verifiedCurrentSource.push(current);
}
const actions = [], transforms = [], corrections = [], windows = [], pendingAttacks = [], phaseEvents = [], nearbySnapshotEvents = [];
const counts = {}, rawDigest = crypto.createHash('sha256'), beforeRaw = await fsp.stat(rawFile); let rawBytes = 0, rawRows = 0, failure;
const rawStream = fs.createReadStream(rawFile); rawStream.on('data', chunk => { rawBytes += chunk.length; rawDigest.update(chunk); });
for await (const line of createInterface({ input: rawStream, crlfDelay: Infinity })) {
  check(line.length < 1024 * 1024, 'Unexpected raw line growth'); const row = JSON.parse(line); rawRows++;
  counts[row.type] = (counts[row.type] ?? 0) + 1; const item = { rawLine: rawRows, ...row };
  if (row.type === 'action' && [fighter, observer].includes(row.actor)) actions.push(item);
  if (row.type === 'selfTransform' && [fighter, observer].includes(row.actor)) transforms.push(item);
  if (row.type === 'movementCorrection') corrections.push(item);
  if (row.type === 'continuousWindow' && row.actor === fighter) windows.push(item);
  if (row.type === 'continuousFailure' && row.evidence?.actor === fighter) failure = item;
  if (row.type === 'pendingAttackEvidence' && row.actor === fighter) pendingAttacks.push(item);
  if (['phaseStarted', 'phaseEnded', 'stageComplete', 'finished', 'safetyStop'].includes(row.type)) phaseEvents.push(item);
  if (row.actor === fighter && ((row.type === 'action' && row.kind === 'refresh') || row.type === 'playerLifecycle')) nearbySnapshotEvents.push(item);
  check(actions.length < 2000 && transforms.length < 2000 && pendingAttacks.length < 1000 && nearbySnapshotEvents.length < 1000, 'Bounded projection exceeded its fixed record limit');
}
const afterRaw = await fsp.stat(rawFile);
check(rawBytes === beforeRaw.size && rawBytes === afterRaw.size && beforeRaw.mtimeMs === afterRaw.mtimeMs, 'Raw file changed during posthoc read');
const rawPin = { file: rawFile, bytes: rawBytes, sha256: rawDigest.digest('hex') };
check(failure && failure.evidence.correctionRatio === 2 / 19, 'Expected preserved failure missing');
const failingWindow = windows.find(row => row.at === failure.at && !row.passed && !row.final);
const previousWindow = windows.filter(row => row.rawLine < failingWindow.rawLine && !row.final).at(-1);
check(previousWindow, 'Missing previous real continuous-window boundary');
const windowActions = actions.filter(row => row.actor === fighter && row.rawLine > previousWindow.rawLine && row.rawLine < failingWindow.rawLine);
const moves = windowActions.filter(row => ['walk', 'run'].includes(row.kind)), attacks = windowActions.filter(row => row.kind === 'attack');
check(moves.length === 19 && moves.filter(row => row.status === 'corrected').length === 2 && attacks.length === 30 &&
  attacks.every(row => row.attackRequestSent === true && row.attackResult === 'launched' && row.status === 'success' && row.damage > 0 && row.sharedHealthVerified === true), 'Raw window counters differ from the preserved failure');
const cases = corrections.filter(row => row.actor === fighter && row.rawLine > previousWindow.rawLine && row.rawLine < failingWindow.rawLine);
check(cases.length === 2, 'Expected exactly two fighter correction projections');
const low = cases[0].at - 5000, high = cases[1].at + 5000;
const selectedEvents = [...actions, ...transforms, ...corrections].filter(row => [fighter, observer].includes(row.actor) && row.at >= low && row.at <= high).sort((a, b) => a.rawLine - b.rawLine);
check(selectedEvents.length < 200, 'Selected event timeline unexpectedly large');
const recentAttack = pendingAttacks.filter(row => row.endedAtWallMs < cases[0].at).at(-1), nextAttack = pendingAttacks.find(row => row.sentAtWallMs > cases[1].at);
const snapshotOverlap = nearbySnapshotEvents.filter(row => row.at >= low && row.at <= high);

// Only parse recording frames inside a fixed11.5s neighborhood. Hash all raw
// bytes, preserve no inventory/chat/events/full world or entity payload.
const beforeRecording = await fsp.stat(recordingFile), recordingDigest = crypto.createHash('sha256');
const frames = []; let recordingBytes = 0, recordingRows = 0;
const recordingStream = fs.createReadStream(recordingFile); recordingStream.on('data', chunk => { recordingBytes += chunk.length; recordingDigest.update(chunk); });
for await (const line of createInterface({ input: recordingStream, crlfDelay: Infinity })) {
  recordingRows++; check(line.length < 16 * 1024 * 1024, 'Recording line exceeded fixed cap');
  const at = Number(line.match(/"capturedAtMs":(\d+)/)?.[1]);
  if (at < low || at > high) continue;
  const frame = JSON.parse(line), entities = frame.world?.entities ?? [];
  const selected = entities.filter(entity => [50004, 50005, 206602].includes(entity.objectId) || (!entity.dead && cheb(entity, { x: 248, y: 646 }) <= 3));
  frames.push({ recordingLine: recordingRows, sequence: frame.sequence, capturedAtMs: frame.capturedAtMs, mapFileName: frame.mapFileName,
    entities: selected.map(({ objectId, name, kind, x, y, direction, hp, maxHp, dead, level, disposition }) => ({ objectId, name, kind, x, y, direction, hp, maxHp, dead, level, disposition })) });
  check(frames.length <= 80, 'Recording projection exceeds fixed frame cap');
}
const afterRecording = await fsp.stat(recordingFile);
check(recordingBytes === beforeRecording.size && recordingBytes === afterRecording.size && beforeRecording.mtimeMs === afterRecording.mtimeMs, 'Recording changed during read');
const recordingPin = { file: recordingFile, bytes: recordingBytes, sha256: recordingDigest.digest('hex') };
const findings = cases.map(correction => {
  const action = actions.find(row => row.actor === fighter && row.at === correction.at && row.kind === correction.kind && row.status === 'corrected');
  const rival = actions.filter(row => row.actor === observer && ['walk', 'run'].includes(row.kind) && row.status === 'success' && row.plannedTarget?.x === correction.target.x && row.plannedTarget?.y === correction.target.y && row.sentAtMs <= action.sentAtMs).at(-1);
  const rivalReceipt = transforms.find(row => row.actor === observer && row.after.position.x === correction.target.x && row.after.position.y === correction.target.y && Math.abs(row.monotonicMs - rival.sentAtMs) < 20);
  const rivalBefore = transforms.filter(row => row.actor === observer && row.monotonicMs < rival.sentAtMs).at(-1);
  const relevantFrames = frames.filter(frame => frame.capturedAtMs >= correction.at - 600 && frame.capturedAtMs <= correction.at + 600);
  return { classificationRecordedAtMs: correction.at, classificationRecordedAtUtc: new Date(correction.at).toISOString(), rawActionLine: action.rawLine, rawCorrectionLine: correction.rawLine,
    kind: correction.kind, actor: fighter, chaseTargetId: action.chaseTargetId, chaseTargetName: action.hunting,
    origin: correction.origin, first: correction.first, plannedDestination: correction.target, receivedActual: correction.actual,
    intendedDirectionDerivedFromPlan: correction.kind === 'run' ? 'Down' : 'DownLeft', receivedDirection: action.direction,
    sentMonotonicMs: action.sentAtMs, receiptMonotonicMs: correction.monotonicMs, receiptSequence: correction.sequence, latencyMs: action.latencyMs,
    classification: 'Real fixed-rule movement correction: received UserLocation endpoint equals origin, differs from expected endpoint and (for Run) permitted first step.',
    classificationSource: 'Frozen soak.mjs action1110-1124; movementCorrection receipt projection is retained, not a complete wire movement trace.',
    ownHpObservedAtCorrection: correction.ownHp, postReceiptNearbyOccupants: correction.nearbyOccupants,
    sameDestinationObserver: { actor: observer, objectId: 50004, rawActionLine: rival.rawLine, sentMonotonicMs: rival.sentAtMs,
      lastPriorOwnReceipt: rivalBefore, sameDestinationOwnReceipt: rivalReceipt, sendPrecedesFighterByMs: action.sentAtMs - rival.sentAtMs,
      ownReceiptAfterFighterSendMs: rivalReceipt.monotonicMs - action.sentAtMs, ownReceiptRelativeToFighterReceiptMs: rivalReceipt.monotonicMs - correction.monotonicMs },
    spectatorFrameReferences: relevantFrames.map(frame => ({ recordingLine: frame.recordingLine, sequence: frame.sequence, capturedAtMs: frame.capturedAtMs })),
    causeAssessment: 'Strong evidence consistent with concurrent ordinary clients competing for the same destination: observer4 sends first, both are sent before its own acknowledgement, and post-receipt occupancy records show it in the rejected destination.',
    causeNotProved: 'No retained full fighter remote-ObjectRun/send-time snapshot or server movement-rejection reason. Cross-WebSocket receipt order and250ms spectator samples do not establish exact authoritative commit/fanout order.' };
});
await writeNew('selected-events-v1.json', { schemaVersion: 1, runId, raw: rawPin, scope: 'Bounded original public action/own-UserLocation/correction rows for actors4/5 around the two corrections, preserving raw line positions.', selectedAtRangeMs: [low, high], selectedEvents,
  precedingAttack: recentAttack ? { rawLine: recentAttack.rawLine, requestId: recentAttack.requestId, endedAtWallMs: recentAttack.endedAtWallMs, target: recentAttack.target,
    ownerUserLocations: recentAttack.packets.filter(packet => packet.receiver === 'owner' && packet.packet === 'UserLocation') } : null,
  nextAttack: nextAttack ? { rawLine: nextAttack.rawLine, requestId: nextAttack.requestId, sentAtWallMs: nextAttack.sentAtWallMs, target: nextAttack.target, outcome: nextAttack.outcome } : null,
  fighterRefreshOrLifecycleOverlap: snapshotOverlap });
await writeNew('spectator-window-v1.json', { schemaVersion: 1, runId, raw: recordingPin, recordingFramesTotal: recordingRows,
  scope: 'Selected server spectator samples only. No chat, inventory, recording events or full entity/world dumps. Frame capture is sampled and not a per-request execution trace.', selectedAtRangeMs: [low, high], frames });
const metadataFiles = await Promise.all([reportFile, path.join(root, 'source-pins-admitted-v2.json'), path.join(root, 'gateway-build-pin.json'), path.join(root, 'realm-process.json'), path.join(root, 'realm-cleanup.json'), path.join(root, 'scenario.json')].map(pin));
const diagnostics = { schemaVersion: 1, createdAt: new Date().toISOString(), runId, scope: 'Read-only posthoc diagnosis of preserved failed05. No replay, test, service, private log/account read, source edit or verdict reclassification.',
  originalVerdict: { ok: report.ok, playableCapacityAccepted: report.playableCapacityAccepted, error: report.error, safetyStopped: report.safetyStopped, stagePassed: report.stages[0].passed },
  raw: rawPin, rawRows, rawTypeCounts: counts, recording: recordingPin, recordingFrames: recordingRows, sources: metadataFiles, verifiedCurrentSource,
  failedWindow: { previousWindowRawLine: previousWindow.rawLine, previousWindowObservedAtMs: previousWindow.at, failedWindowRawLine: failingWindow.rawLine, failedWindowObservedAtMs: failingWindow.at,
    seconds: failingWindow.seconds, boundaryRule: 'Exact raw line order between successive same-actor continuousWindow markers; wall markers are emission times, not invented exact wire-send times.',
    movementRequests: moves.length, correctedMovement: 2, movementCorrectionRatio: 2 / 19, unchangedMaximumCorrectionRatio: report.policy.maximumCorrectionRatio,
    attackV2Requests: attacks.length, launched: attacks.filter(row => row.attackResult === 'launched').length, positiveDualEndHits: attacks.filter(row => row.status === 'success' && row.damage > 0 && row.sharedHealthVerified).length,
    notLaunched: attacks.filter(row => row.attackResult === 'notLaunched').length, timeouts: windowActions.filter(row => row.status === 'timeout').length,
    nativeActivityPassed: failingWindow.nativeActivity.passed, activityPassed: failingWindow.activityPassed, latencyPassed: failingWindow.latencyPassed, fanoutPassed: failingWindow.fanoutPassed,
    rawFailure: failure },
  findings, otherRecordedCorrections: corrections.filter(row => row.actor !== fighter),
  summary: ['两次fighter corrected都是真实收回原地坐标，按原目标/Run首步比较规则正确计入纠正，2/19=10.526%超过原5%。',
    '两次均与observer4几乎同时抢同一格(248,646)相符；observer4先发送1.3645/1.5524ms，收到其自身确认前fighter已发送。后置occupancy两次均明确玩家50004在该格。',
    '现有记录支持并发目的地竞争的强相容解释，缺fighter逐包remote movement/送前完整快照及server拒绝原因，不能宣称已证明精确服务器执行顺序。',
    '失败窗口30个V2请求均launched且原双端正伤害命中；无NotLaunched或timeout。该失败不能当攻击协议失败或物理机容量上限。'],
  unchanged: { originalFailureRetained: true, gameplayThresholdsChanged: false, sourceChanged: false, newRealmStarted: false, capacityAccepted: false },
  projections: await Promise.all(['selected-events-v1.json', 'spectator-window-v1.json'].map(name => pin(path.join(output, name)))) };
await writeNew('diagnostic-v1.json', diagnostics);
console.log(JSON.stringify({ diagnostic: await pin(path.join(output, 'diagnostic-v1.json')), selectedEvents: selectedEvents.length, selectedFrames: frames.length,
  corrections: findings.map(value => ({ at: value.classificationRecordedAtMs, kind: value.kind, observerSendLeadMs: value.sameDestinationObserver.sendPrecedesFighterByMs })), raw: rawPin, recording: recordingPin, verdictPreserved: true }));
