// Bounded offline extraction only. Never copies or prints the original stderr.
import fs from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';

const base = 'C:/mir2-build/p01-completion-20261024';
const task = path.join(base, 'full-lifecycle-socket-diagnostics-v2');
const out = path.dirname(import.meta.filename);
const stamp = '1792884512369-f0f261da';
const helper = path.join(base, 'slow-stage-socket-context-v1/slow-stage-public.mjs');
const source = path.join(task, 'private', stamp + '.gateway.stderr.log');
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const read = async file => JSON.parse(await fs.readFile(file));
const pin = async file => { const bytes = await fs.readFile(file); return { bytes: bytes.length, sha256: sha(bytes) }; };
const timeSemantics = 'observedAtMs is the time this stopped-file offline extractor read a whitelisted line. It is NOT the original launcher/server receive time, span start/end, or an event-correlation timestamp. Row order is original stderr order only. No attribution to the earlier07 failure follows.';
const helperPin = await pin(helper);
assert.deepEqual(helperPin, { bytes: 10186, sha256: 'c2a137084f2166cbae603f721c6dde7322790c8dc2e05c2b8d59b79d7a58619f' });
const bindingFile = path.join(task, 'launcher-final-binding-v1.json');
const bindingPin = await pin(bindingFile);
assert.deepEqual(bindingPin, { bytes: 1360, sha256: '8590785f6d419e754dbf8fefda5abcccdbd80268166c00eed9cea2248c98a2da' });
const binding = await read(bindingFile);
assert.equal(binding.expectedSourceCommit, '46880bd1ef415131c4cf85aeb4dd2fa4f6fd4f9b');
assert.equal(binding.backendInputCount, 41);
const realmFile = path.join(task, 'public', stamp + '.realm-evidence.json');
const realm = await read(realmFile);
assert.equal(realm.stamp, stamp); assert.equal(realm.pid, 67840);
assert.equal(realm.ok, true); assert.equal(realm.shutdown.code, 0);
assert.deepEqual(realm.remainingOwnedPortListeners, []);
assert.equal(realm.release.sha256, binding.binary.sha256);
assert.equal(realm.release.sourceCommit, binding.expectedSourceCommit);
assert.equal(realm.configuration.slowStageThresholdMs, 50);
for (const field of ['currentWsConnections', 'currentActiveSessions', 'currentReconnectLeases', 'currentLoginInFlight', 'currentNewCharacterInFlight', 'currentStartGameInFlight']) assert.equal(realm.afterHealth.capacity[field], 0);

const { createSlowStagePublicCapture, ALLOWED_STAGES, ALLOWED_SCOPED_STAGES, ALLOWED_QUEUED_PHASES, ALLOWED_INPUT_CLASSES } = await import(pathToFileURL(helper));
const before = await fs.stat(source);
assert.ok(before.isFile()); assert.ok(before.size <= 32 * 1024 * 1024, 'Offline input bounded to32MiB');
const sourceHash = crypto.createHash('sha256'); let sourceBytes = 0;
const input = createReadStream(source, { highWaterMark: 64 * 1024 });
input.on('data', chunk => { sourceBytes += chunk.length; sourceHash.update(chunk); });
const stopReasons = [];
const rowsFile = path.join(out, 'offline-read-whitelisted-slow-stage.jsonl');
const capture = createSlowStagePublicCapture({ input, outputFile: rowsFile, requestStop: reason => { stopReasons.push(reason); input.resume(); } });
const startedAtMs = Date.now();
const parserStatus = await capture.finish({ gatewayExited: true });
const finishedAtMs = Date.now(), after = await fs.stat(source);
assert.equal(sourceBytes, before.size); assert.equal(after.size, before.size); assert.equal(after.mtimeMs, before.mtimeMs);
const rowsBytes = await fs.readFile(rowsFile);
assert.ok(rowsBytes.length <= 32 * 1024 * 1024);
const rows = rowsBytes.toString('utf8').split('\n').filter(Boolean).map(line => JSON.parse(line));
assert.equal(rows.length, parserStatus.rowsQueued);
const byStage = new Map(), queuePhases = new Map(), inputClasses = new Map(), connections = new Set();
const largeRows = [], nonBootstrapStatic = stage => stage.startsWith('runtime_tick.') || ['socket_turn.chat', 'socket_turn.injection', 'socket_turn.spectator', 'socket_turn.ai_live', 'socket_turn.input_owner_heartbeat'].includes(stage);
for (const [index, row] of rows.entries()) {
  const expected = row.queuedPhase !== undefined ? ['observedAtMs','stage','durationMs','connection','queuedPhase','phaseElapsedMs','inputClass'] : row.connection !== undefined ? ['observedAtMs','stage','durationMs','connection'] : ['observedAtMs','stage','durationMs'];
  assert.deepEqual(Object.keys(row), expected); assert.ok(ALLOWED_STAGES.includes(row.stage));
  assert.ok(Number.isSafeInteger(row.durationMs) && row.durationMs >= 0);
  assert.ok(row.observedAtMs >= startedAtMs && row.observedAtMs <= finishedAtMs);
  if (row.connection !== undefined) { assert.ok(Number.isSafeInteger(row.connection) && row.connection > 0); connections.add(row.connection); assert.ok(row.queuedPhase !== undefined || ALLOWED_SCOPED_STAGES.includes(row.stage)); }
  const item = byStage.get(row.stage) ?? { count: 0, maxMs: 0, scopedCount: 0, queueContextCount: 0 };
  item.count++; item.maxMs = Math.max(item.maxMs, row.durationMs); item.scopedCount += row.connection !== undefined ? 1 : 0; item.queueContextCount += row.queuedPhase !== undefined ? 1 : 0; byStage.set(row.stage, item);
  if (row.queuedPhase !== undefined) { assert.ok(ALLOWED_QUEUED_PHASES.includes(row.queuedPhase)); assert.ok(ALLOWED_INPUT_CLASSES.includes(row.inputClass)); queuePhases.set(row.queuedPhase, (queuePhases.get(row.queuedPhase) ?? 0) + 1); inputClasses.set(row.inputClass, (inputClasses.get(row.inputClass) ?? 0) + 1); }
  if (row.durationMs > 500) largeRows.push({ rowNumber: index + 1, ...row, classification: nonBootstrapStatic(row.stage) ? 'non-bootstrap-static-branch; does not identify underlying cause' : 'bootstrap-or-other-action-unknown; no original receive timestamp or request correlation' });
}
const status = { schemaVersion: 1, mode: 'offline-stopped-private-stderr-strict-whitelist', observedAtMsSemantics: timeSemantics, parserStatusScopeIsGenericOverriddenByOfflineTimeSemantics: true, parserStatus, stopReasons, source: { basename: path.basename(source), bytes: sourceBytes, sha256: sourceHash.digest('hex'), rawCopied: false, rawPrinted: false }, helper: helperPin, finalBinding: bindingPin, realmPublicEvidence: await pin(realmFile), extractionStartedAtMs: startedAtMs, extractionFinishedAtMs: finishedAtMs, gatewayAlreadyExited: true, noServiceOrGameplayExecuted: true };
const aggregate = { schemaVersion: 1, mode: status.mode, observedAtMsSemantics: timeSemantics, complete: parserStatus.complete, rowCount: rows.length, scopedRows: rows.filter(row => row.connection !== undefined).length, queueContextRows: rows.filter(row => row.queuedPhase !== undefined).length, distinctOpaqueConnections: connections.size, byStage: Object.fromEntries([...byStage].sort()), queuedPhases: Object.fromEntries([...queuePhases].sort()), inputClasses: Object.fromEntries([...inputClasses].sort()), greaterThan500Ms: largeRows, nonBootstrapStaticBranchGreaterThan500Ms: largeRows.filter(row => nonBootstrapStatic(row.stage)), scope: 'Valid Some/scoped queue-context producer paths are observable if counts are nonzero. Static branch duration is not a root cause.50ms threshold left-censors output; absence of rows is not proof of no stall. No07 causal identification or new capacity acceptance.', source: status.source, helper: helperPin, finalBinding: bindingPin, rows: { file: path.basename(rowsFile), ...await pin(rowsFile) } };
await fs.writeFile(path.join(out, 'offline-extraction-status.json'), JSON.stringify(status, null, 2) + '\n', { flag: 'wx' });
await fs.writeFile(path.join(out, 'offline-aggregate.json'), JSON.stringify(aggregate, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ complete: parserStatus.complete, reasons: parserStatus.reasons, rows: rows.length, scopedRows: aggregate.scopedRows, queueContextRows: aggregate.queueContextRows, opaqueConnections: connections.size, byStage: aggregate.byStage, greaterThan500Ms: largeRows, output: out, offlineTimeOnly: true }));
if (!parserStatus.complete) process.exitCode = 1;
