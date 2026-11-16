import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';
const directory = path.dirname(fileURLToPath(import.meta.url));
const stem = '1793047839765-82a5e52b';
const reportFile = path.join(directory, 'episode/reports', stem + '.preparation-report.json');
const samplesFile = path.join(directory, 'episode/reports', stem + '.preparation.jsonl');
const recordingFile = path.join(directory, 'episode/recordings/0-498068.jsonl');
const output = path.join(directory, 'public-health-guard-evidence-v1.json');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
async function regular(file) { const s = await fsp.lstat(file); if (!s.isFile() || s.isSymbolicLink()) throw Error('Regular input required'); }
async function bytesPin(file) { await regular(file); const bytes = await fsp.readFile(file); return [{ file, bytes: bytes.length, sha256: sha(bytes) }, bytes]; }
const [reportPin, reportBytes] = await bytesPin(reportFile);
const report = JSON.parse(reportBytes), name = report.failedActor.name;
const [samplesPin] = await bytesPin(samplesFile);
const preparations = [];
for await (const line of readline.createInterface({ input: fs.createReadStream(samplesFile), crlfDelay: Infinity })) {
  const row = JSON.parse(line); if (row.type === 'ordinaryPreparation') preparations.push(row);
}
const failed = preparations.find(row => row.actor === name && row.result.status === 'failed');
if (!failed || report.preparedCount !== 4 || report.error !== 'ordinary supplies: owner/map/health changed') throw Error('Unexpected original failure');
const hpFrames = [], healthEvents = [], identitySet = new Set(), maxHpSet = new Set(), levelSet = new Set();
let previous = null, recordingRows = 0, recordingBytes = 0;
const hash = crypto.createHash('sha256'); await regular(recordingFile);
const stream = fs.createReadStream(recordingFile); stream.on('data', chunk => { hash.update(chunk); recordingBytes += chunk.length; });
for await (const line of readline.createInterface({ input: stream, crlfDelay: Infinity })) {
  const row = JSON.parse(line); recordingRows++; const world = row.world ?? {}, entities = world.entities ?? [];
  const owner = entities.find(entity => entity.name === name); if (!owner) continue;
  const near = entities.filter(entity => entity.kind === 'monster' && entity.dead === false &&
    Math.max(Math.abs(entity.x - owner.x), Math.abs(entity.y - owner.y)) <= 8 &&
    (entity.disposition === 'hostile' || entity.objectId === 206214)).map(entity => ({
      objectId: entity.objectId, name: entity.name, x: entity.x, y: entity.y, hp: entity.hp, dead: entity.dead,
      disposition: entity.disposition, distance: Math.max(Math.abs(entity.x-owner.x), Math.abs(entity.y-owner.y))
    })).sort((a,b) => a.distance-b.distance || a.objectId-b.objectId).slice(0,16);
  const current = { recordSequence: row.sequence, capturedAtMs: row.capturedAtMs,
    ownerId: owner.objectId, ownerName: owner.name, map: String(world.mapFileName), x: owner.x, y: owner.y,
    hp: owner.hp, maxHp: owner.maxHp, level: owner.level, dead: owner.dead, inSafeZone: world.inSafeZone, nearbyMonsters: near };
  identitySet.add([current.ownerId,current.ownerName,current.map].join('/')); maxHpSet.add(current.maxHp); levelSet.add(current.level);
  if (!previous || ['ownerId','ownerName','map','hp','maxHp','level','dead'].some(key => current[key] !== previous[key])) hpFrames.push(current);
  previous = current;
  if (row.capturedAtMs >= failed.at-30_000) for (const event of row.events ?? []) {
    if (event.kind === 'health' && event.objectId === owner.objectId) healthEvents.push({ recordSequence: row.sequence,
      capturedAtMs: row.capturedAtMs, eventAtMs: event.atMs, eventSequence: null, from: event.payload.from, to: event.payload.to });
  }
}
const [scriptPin] = await bytesPin(fileURLToPath(import.meta.url));
const last = hpFrames.at(-1), finishedAtMs = Date.parse(report.finishedAt);
const evidence = { schemaVersion: 1, runId: report.runId, scope: 'Public bounded health-guard evidence; no replay or capacity acceptance',
  sources: { report: reportPin, preparationSamples: samplesPin, sharedRecording: { file: recordingFile, bytes: recordingBytes, sha256: hash.digest('hex'), rows: recordingRows }, extractionScript: scriptPin },
  originalVerdict: { ok: report.ok, preparedCount: report.preparedCount, error: report.error },
  failedActor: { name, characterIndex: report.failedActor.characterIndex, before: report.failedActor.before,
    commands: failed.result.commands, movementCommands: failed.result.movementCommands, commandCounts: failed.result.commandCounts,
    purchased: failed.result.purchased, soldVenison: failed.result.soldVenison, funding: failed.result.funding },
  observedIdentityMapKeys: [...identitySet], observedMaxHpValues: [...maxHpSet], observedLevelValues: [...levelSet], hpFrames, healthEvents,
  timing: { failedPreparationSampleAtMs: failed.at, reportFinishedAtMs: finishedAtMs, reportFinishMinusFailureSampleMs: finishedAtMs-failed.at,
    finalRecordedHealthAtMs: last.capturedAtMs, failureSampleMinusFinalHealthMs: failed.at-last.capturedAtMs,
    reportFailedActorElapsedMs: report.failedActor.elapsedMs, sampleElapsedMs: failed.result.elapsedMs,
    reportMinusSampleElapsedMs: report.failedActor.elapsedMs-failed.result.elapsedMs,
    eventCaptureLagMs: healthEvents.map(event => event.capturedAtMs-event.eventAtMs) },
  healthBranch: { originalMinimumRatio: 0.7, lastRecordedHp: last.hp, maxHp: last.maxHp, ratio: last.hp/last.maxHp,
    guardUsesConservativePercentWhenNewer: true, exactClientGuardValueNotRetained: true },
  targetLossDiagnostics: preparations.map(row => ({ name: row.actor, count: row.result.fundingTargetRejectionCount, truncated: row.result.fundingTargetRejectionsTruncated })),
  limits: [ 'The shared recording proves exact world HP changes, not the exact percentage-derived client guard scalar.',
    'Health events provide no attacker ID. Nearby HookingCat and damage are aligned evidence, not identification of the exact damage source.',
    'Shared spectator inventory and gold are not used to infer held stock or earned funds; funding=null means the funding call did not return.',
    'No owner/name/map/maxHP/level change was observed. This is bounded failure evidence, not death or capacity acceptance.',
    'Recording events have no independent event sequence; recordSequence identifies their actual containing frame. Capture time can lag event time.' ],
  capacityAccepted: false, codeChanged: false };
await fsp.writeFile(output, JSON.stringify(evidence,null,2)+'\n', { flag:'wx' });
console.log(JSON.stringify({ output, bytes:(await fsp.stat(output)).size, sha256:sha(await fsp.readFile(output)) }));
