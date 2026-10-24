import { createWriteStream } from 'node:fs';

export const MAX_LINE_BYTES = 8 * 1024;
export const MAX_PUBLIC_BYTES = 32 * 1024 * 1024;
export const MAX_WRITE_BUFFER_BYTES = 1024 * 1024;
export const MAX_FINISH_WAIT_MS = 15000;
export const ALLOWED_STAGES = Object.freeze([
  'serial_gate.zone_outbound_wait', 'socket_input.queue_residence', 'serial_gate.input_wait',
  'session_action.execute', 'session_action.flush', 'runtime_tick.execute',
  'runtime_tick.save_checkpoint', 'runtime_tick.flush', 'flush.responses', 'flush.external_state',
  'flush.save', 'snapshot.build_and_send', 'zone_owner.in_process.execute_request',
  'zone_owner.hosted.runtime_lock_wait', 'zone_owner.hosted.execute_request', 'zone_cadence.total',
  'zone_cadence.gate_wait', 'zone_cadence.manager_tick', 'zone_cadence.dispatch',
  'shared_session.predrain.monster_activation', 'shared_session.predrain.shared_entity_projection',
  'shared_session.predrain.pending_lock_wait', 'shared_session.predrain.journey_reposition',
  'shared_session.predrain', 'shared_session.inner_execute', 'shared_session.reconcile', 'shared_session.tail',
]);
const allowed = new Set(ALLOWED_STAGES);
const pattern = /^\[gateway-slow-stage\] stage=([a-z0-9_.]+) duration_ms=(\d+)$/;
function bounded(value, maximum) {
  if (!Number.isSafeInteger(value) || value < 1 || value > maximum) throw Error('Invalid bounded diagnostic limit');
  return value;
}
function deferred() { let resolve; const promise = new Promise(done => { resolve = done; }); return { promise, resolve }; }

function capture({ input, sink, requestStop, lineLimit, byteLimit, writeLimit, finishWaitMs, now }) {
  const line = Buffer.alloc(lineLimit), reasons = new Set(), inputDone = deferred(), sinkDone = deferred();
  let used = 0, maxBufferedLineBytes = 0, inputEnded = false, writerFinished = false, stopRequested = false;
  let accepting = true, rowsQueued = 0, bytesQueued = 0, maxQueuedWriteBytes = 0, finished;
  const fail = reason => {
    reasons.add(reason); accepting = false; used = 0; line.fill(0); input.off('data', onData);
    if (!stopRequested) {
      stopRequested = true;
      try { requestStop(reason); } catch { reasons.add('request-stop-error'); }
    }
  };
  const emit = length => {
    if (length && line[length - 1] === 13) length--;
    for (let index = 0; index < length; index++) if (line[index] > 127) return;
    const match = pattern.exec(line.toString('ascii', 0, length));
    if (!match || !allowed.has(match[1])) return;
    if (!/^(?:0|[1-9]\d*)$/.test(match[2])) { fail('noncanonical-duration'); return; }
    const durationMs = Number(match[2]), observedAtMs = now();
    if (!Number.isSafeInteger(durationMs) || durationMs < 0) { fail('unsafe-duration'); return; }
    if (!Number.isSafeInteger(observedAtMs) || observedAtMs < 0) { fail('invalid-receive-clock'); return; }
    const row = JSON.stringify({ observedAtMs, stage: match[1], durationMs }) + '\n';
    const bytes = Buffer.byteLength(row);
    if (bytesQueued + bytes > byteLimit) { fail('public-byte-cap'); return; }
    if (sink.writableLength + bytes > writeLimit) { fail('write-buffer-cap'); return; }
    let writable;
    try { writable = sink.write(row); } catch { fail('write-error'); return; }
    rowsQueued++; bytesQueued += bytes;
    maxQueuedWriteBytes = Math.max(maxQueuedWriteBytes, sink.writableLength);
    if (!writable) fail('write-backpressure');
  };
  function onData(chunk) {
    if (!accepting) return;
    if (!Buffer.isBuffer(chunk)) { fail('non-buffer-stderr'); return; }
    for (const byte of chunk) {
      if (!accepting) break;
      if (byte === 10) { const length = used; used = 0; emit(length); line.fill(0); }
      else {
        if (used === lineLimit) { fail('line-byte-cap'); break; }
        line[used++] = byte; maxBufferedLineBytes = Math.max(maxBufferedLineBytes, used);
      }
    }
  }
  const onEnd = () => {
    inputEnded = true;
    if (used) fail('partial-line-at-eof');
    inputDone.resolve(true);
  };
  const onInputError = () => { fail('stderr-read-error'); inputDone.resolve(false); };
  const onInputClose = () => {
    if (!inputEnded) fail('stderr-closed-before-end');
    inputDone.resolve(inputEnded);
  };
  const onWriterError = () => { fail('metadata-write-error'); sinkDone.resolve(false); };
  const onWriterFinish = () => { writerFinished = true; sinkDone.resolve(true); };
  const onWriterClose = () => {
    if (!writerFinished) fail('metadata-closed-before-finish');
    sinkDone.resolve(writerFinished);
  };
  sink.on('error', onWriterError); sink.on('finish', onWriterFinish); sink.on('close', onWriterClose);
  input.on('data', onData); input.on('end', onEnd); input.on('error', onInputError); input.on('close', onInputClose);
  if (input.readableEnded) onEnd();
  async function wait(promise, timeoutReason) {
    let timer;
    try { return await Promise.race([promise, new Promise(resolve => { timer = setTimeout(() => { fail(timeoutReason); resolve(false); }, finishWaitMs); })]); }
    finally { clearTimeout(timer); }
  }
  return Object.freeze({
    finish({ gatewayExited }) {
      if (finished) return finished;
      finished = (async () => {
        if (!gatewayExited) fail('gateway-not-exited');
        else await wait(inputDone.promise, 'stderr-end-timeout');
        input.off('data', onData); input.off('end', onEnd); input.off('close', onInputClose);
        line.fill(0); used = 0;
        if (!sink.destroyed && !sink.writableEnded) { try { sink.end(); } catch { fail('metadata-end-error'); sinkDone.resolve(false); } }
        const sinkFinished = await wait(sinkDone.promise, 'metadata-finish-timeout');
        if (!sinkFinished && !sink.destroyed) sink.destroy();
        return { schemaVersion: 1, complete: reasons.size === 0 && inputEnded && writerFinished,
          reasons: [...reasons], stopRequested, stderrEnded: inputEnded, writerFinished, rowsQueued, bytesQueued, maxBufferedLineBytes,
          lineLimitBytes: lineLimit, totalLimitBytes: byteLimit, writeLimitBytes: writeLimit,
          writeHighWaterMarkBytes: sink.writableHighWaterMark, maxQueuedWriteBytes,
          scope: 'Only complete ASCII exact-format lines from the frozen static GatewaySlowStage label allowlist. observedAtMs is the launcher receive clock, not server span start/end. Queued counts alone do not prove durable output when incomplete. Existing50ms logging is left-censored; missing rows do not prove absence of a stall.' };
      })();
      return finished;
    },
  });
}

export function createSlowStagePublicCapture(options) {
  if (!options || Object.keys(options).some(key => !['input', 'outputFile', 'requestStop'].includes(key)) ||
      typeof options.outputFile !== 'string' || typeof options.requestStop !== 'function') throw Error('Invalid production slow-stage capture options');
  const sink = createWriteStream(options.outputFile, { flags: 'wx', mode: 0o600, highWaterMark: MAX_WRITE_BUFFER_BYTES });
  return capture({ input: options.input, sink, requestStop: options.requestStop, lineLimit: MAX_LINE_BYTES,
    byteLimit: MAX_PUBLIC_BYTES, writeLimit: MAX_WRITE_BUFFER_BYTES, finishWaitMs: MAX_FINISH_WAIT_MS, now: Date.now });
}

// Synthetic sinks can lower limits. They cannot raise or override any production ceiling.
export function createSyntheticSlowStageCapture({ input, sink, requestStop, lineLimit = MAX_LINE_BYTES,
  byteLimit = MAX_PUBLIC_BYTES, writeLimit = MAX_WRITE_BUFFER_BYTES, finishWaitMs = MAX_FINISH_WAIT_MS, now = Date.now }) {
  return capture({ input, sink, requestStop, lineLimit: bounded(lineLimit, MAX_LINE_BYTES),
    byteLimit: bounded(byteLimit, MAX_PUBLIC_BYTES), writeLimit: bounded(writeLimit, MAX_WRITE_BUFFER_BYTES),
    finishWaitMs: bounded(finishWaitMs, MAX_FINISH_WAIT_MS), now });
}
