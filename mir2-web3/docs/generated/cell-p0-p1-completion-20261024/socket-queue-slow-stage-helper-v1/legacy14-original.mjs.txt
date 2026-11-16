import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import { PassThrough, Writable } from 'node:stream';
import { createSyntheticSlowStageCapture, createSlowStagePublicCapture, ALLOWED_STAGES,
  MAX_LINE_BYTES, MAX_PUBLIC_BYTES, MAX_WRITE_BUFFER_BYTES } from './slow-stage-public.mjs';

const clock = 1792873000000, stage = 'session_action.execute';
const wire = (duration = '50', label = stage) => `[gateway-slow-stage] stage=${label} duration_ms=${duration}`;
function fixture({ lineLimit, byteLimit, writeLimit, highWaterMark = 1024 * 1024, final, delayWrite = 0, finishWaitMs = 100 } = {}) {
  const input = new PassThrough(), chunks = [], stops = [];
  // The real launcher independently pipes all original stderr to its private log.
  input.on('data', () => {});
  const sink = new Writable({ highWaterMark,
    write(chunk, encoding, done) { chunks.push(Buffer.from(chunk)); if (delayWrite) setTimeout(done, delayWrite); else done(); },
    final: final ?? (done => done()) });
  const capture = createSyntheticSlowStageCapture({ input, sink, requestStop: reason => stops.push(reason),
    ...(lineLimit ? { lineLimit } : {}), ...(byteLimit ? { byteLimit } : {}), ...(writeLimit ? { writeLimit } : {}), finishWaitMs, now: () => clock });
  return { input, sink, stops, capture, text: () => Buffer.concat(chunks).toString('utf8'),
    rows: () => Buffer.concat(chunks).toString('utf8').trim().split('\n').filter(Boolean).map(JSON.parse) };
}

test('frozen public labels equal every current static Rust GatewaySlowStage label', async () => {
  const project = 'C:/Users/Administrator/.codex/worktrees/cell-preproduction-p0-p1/mir2-player-journey/mir2-web3';
  const labels = new Set();
  for (const file of ['apps/gateway/src/web.rs', 'apps/gateway/src/routing.rs']) {
    const source = await fs.readFile(project + '/' + file, 'utf8');
    for (const match of source.matchAll(/GatewaySlowStage::(?:start|since)\(\s*"([^"]+)"/g)) labels.add(match[1]);
  }
  assert.equal(ALLOWED_STAGES.length, 27);
  assert.deepEqual([...ALLOWED_STAGES].sort(), [...labels].sort());
});

test('split chunks and CRLF emit only the three declared metadata fields at receive time', async () => {
  const f = fixture(), line = wire();
  f.input.write(Buffer.from(line.slice(0, 19))); assert.equal(f.text(), '');
  f.input.write(Buffer.from(line.slice(19) + '\r')); assert.equal(f.text(), '');
  f.input.end(Buffer.from('\n' + wire('0') + '\n'));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, true); assert.equal(result.writerFinished, true); assert.equal(f.stops.length, 0);
  assert.deepEqual(f.rows(), [{ observedAtMs: clock, stage, durationMs: 50 }, { observedAtMs: clock, stage, durationMs: 0 }]);
  assert.equal(result.bytesQueued, Buffer.byteLength(f.text()));
});

test('private account/token lines, non-ASCII, prefix/suffix injections and unknown labels never reach public output', async () => {
  const f = fixture();
  f.input.end(Buffer.from(['account=synthetic-account token=synthetic-token', wire('50', 'synthetic_account'),
    wire() + ' token=synthetic-token', 'prefix ' + wire(), wire('50', stage + 'é'), wire() + '\u0000', wire()].join('\n') + '\n'));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, true); assert.equal(f.rows().length, 1);
  assert.ok(!/synthetic|account|token|é|\\u0000/.test(f.text()));
  assert.deepEqual(Object.keys(f.rows()[0]), ['observedAtMs', 'stage', 'durationMs']);
});

test('an unterminated matching EOF is not published and is explicitly incomplete', async () => {
  const f = fixture(); f.input.end(Buffer.from(wire()));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(f.text(), ''); assert.equal(result.complete, false);
  assert.ok(result.reasons.includes('partial-line-at-eof')); assert.equal(f.stops.length, 1);
});

test('canonical integer validation rejects leading zero and unsafe digits without publishing them', async () => {
  for (const duration of ['050', '9007199254740992']) {
    const f = fixture(); f.input.end(Buffer.from(wire(duration) + '\n'));
    const result = await f.capture.finish({ gatewayExited: true });
    assert.equal(f.text(), ''); assert.equal(result.complete, false); assert.equal(f.stops.length, 1);
  }
  const f = fixture(); f.input.end(Buffer.from(['+1', '-1', '1.5', '1e3'].map(value => wire(value)).join('\n') + '\n'));
  assert.equal((await f.capture.finish({ gatewayExited: true })).complete, true); assert.equal(f.text(), '');
});

test('overlong input is bounded at8KiB and asks for safe stop exactly once', async () => {
  const f = fixture(); f.input.end(Buffer.alloc(MAX_LINE_BYTES + 100, 97));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, false); assert.ok(result.reasons.includes('line-byte-cap'));
  assert.equal(result.maxBufferedLineBytes, MAX_LINE_BYTES); assert.equal(f.text(), ''); assert.equal(f.stops.length, 1);
});

test('the32MiB ceiling is fixed and lower synthetic byte limits stop without buffering a large string', async () => {
  assert.equal(MAX_LINE_BYTES, 8192); assert.equal(MAX_PUBLIC_BYTES, 33554432); assert.equal(MAX_WRITE_BUFFER_BYTES, 1048576);
  const one = Buffer.byteLength(JSON.stringify({ observedAtMs: clock, stage, durationMs: 50 }) + '\n');
  const f = fixture({ byteLimit: one * 2 - 1 }); f.input.end(Buffer.from((wire() + '\n').repeat(3)));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, false); assert.ok(result.reasons.includes('public-byte-cap'));
  assert.equal(f.rows().length, 1); assert.ok(result.bytesQueued <= result.totalLimitBytes); assert.equal(f.stops.length, 1);
  assert.throws(() => createSyntheticSlowStageCapture({ input: new PassThrough(), sink: new PassThrough(), requestStop() {}, byteLimit: MAX_PUBLIC_BYTES + 1 }));
  assert.throws(() => createSyntheticSlowStageCapture({ input: new PassThrough(), sink: new PassThrough(), requestStop() {}, lineLimit: MAX_LINE_BYTES + 1 }));
  assert.throws(() => createSyntheticSlowStageCapture({ input: new PassThrough(), sink: new PassThrough(), requestStop() {}, writeLimit: MAX_WRITE_BUFFER_BYTES + 1 }));
  assert.throws(() => createSlowStagePublicCapture({ input: new PassThrough(), outputFile: 'never-created.jsonl', requestStop() {}, lineLimit: 1 }));
});

test('the independent1MiB write-buffer ceiling stops before queue overflow with a lower synthetic limit', async () => {
  const one = Buffer.byteLength(JSON.stringify({ observedAtMs: clock, stage, durationMs: 50 }) + '\n');
  const f = fixture({ writeLimit: one + 1, delayWrite: 5 }); f.input.end(Buffer.from((wire() + '\n').repeat(3)));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, false); assert.ok(result.reasons.includes('write-buffer-cap'));
  assert.equal(f.rows().length, 1); assert.ok(result.maxQueuedWriteBytes <= result.writeLimitBytes); assert.equal(f.stops.length, 1);
});

test('writer backpressure marks incomplete and flushes its one bounded queued record before finish returns', async () => {
  const f = fixture({ highWaterMark: 1, delayWrite: 5 }); f.input.end(Buffer.from((wire() + '\n').repeat(3)));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, false); assert.ok(result.reasons.includes('write-backpressure'));
  assert.equal(result.rowsQueued, 1); assert.equal(f.rows().length, 1); assert.equal(f.sink.writableFinished, true); assert.equal(f.stops.length, 1);
});

test('finish waits for the actual metadata writer final callback and is idempotent', async () => {
  let finishWriter; const f = fixture({ final: done => { finishWriter = done; } });
  f.input.end(Buffer.from(wire() + '\n'));
  const first = f.capture.finish({ gatewayExited: true }), second = f.capture.finish({ gatewayExited: true });
  assert.equal(first, second); let returned = false; first.then(() => { returned = true; });
  await new Promise(resolve => setImmediate(resolve)); assert.equal(returned, false); assert.equal(typeof finishWriter, 'function');
  finishWriter(); assert.equal((await first).complete, true);
});

test('writer final error stays incomplete and cannot disclose its private error text', async () => {
  const f = fixture({ final: done => done(new Error('token=synthetic-final-secret')) });
  f.input.end(Buffer.from(wire() + '\n'));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, false); assert.equal(result.writerFinished, false);
  assert.ok(result.reasons.includes('metadata-write-error')); assert.equal(f.stops.length, 1);
  assert.ok(!JSON.stringify(result).includes('synthetic-final-secret'));
});

test('a stalled writer finish has a bounded deadline and never claims a complete stream', async () => {
  const f = fixture({ final() {}, finishWaitMs: 10 }); f.input.end(Buffer.from(wire() + '\n'));
  const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, false); assert.ok(result.reasons.includes('metadata-finish-timeout'));
  assert.equal(f.stops.length, 1); assert.equal(f.sink.destroyed, true);
});

test('empty complete stderr is a valid empty projection and says nothing about uncaptured server stages', async () => {
  const f = fixture(); f.input.end(); const result = await f.capture.finish({ gatewayExited: true });
  assert.equal(result.complete, true); assert.equal(result.rowsQueued, 0); assert.equal(f.text(), '');
  assert.ok(result.scope.includes('missing rows do not prove absence of a stall'));
});

test('closed stderr or an unexited Gateway cannot be called complete', async () => {
  const closed = fixture(); closed.input.destroy(new Error('account=synthetic-input-secret'));
  const first = await closed.capture.finish({ gatewayExited: true });
  assert.equal(first.complete, false); assert.equal(closed.stops.length, 1); assert.ok(!JSON.stringify(first).includes('synthetic-input-secret'));
  const alive = fixture(); const second = await alive.capture.finish({ gatewayExited: false });
  assert.equal(second.complete, false); assert.ok(second.reasons.includes('gateway-not-exited')); assert.equal(alive.stops.length, 1);
});
