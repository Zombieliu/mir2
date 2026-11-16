import test from 'node:test';
import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import { attachAuthWireObserver } from './lib/auth-wire-observer.mjs';

function setup(options = {}) {
  const cdp = new EventEmitter();
  cdp.send = () => { throw Error('Observer must not issue protocol commands'); };
  const observer = attachAuthWireObserver({ cdp, side: 'desktop', now: () => 1_000_000, ...options });
  return { cdp, observer, emit: (name, value) => cdp.emit('Network.' + name, value) };
}
const request = (requestId, url = 'https://numeron.obelisk.build/api/wallet/dubhe/session/status') => ({ requestId, timestamp: 10, wallTime: 1000, request: { method: 'POST', url } });

test('interleaved poll and approval use their own wire clock and expose no capabilities', async () => {
  const { emit, observer } = setup();
  emit('requestWillBeSent', { ...request('poll'), request: { ...request('poll').request, postData: 'private-proof', headers: { Authorization: 'Bearer private-proof' } } });
  emit('requestWillBeSent', { ...request('approve', 'https://relay.dubhe.obelisk.build/sessions/private-session/approve?secret=private-proof'), timestamp: 11, wallTime: 1001 });
  emit('responseReceived', { requestId: 'approve', timestamp: 11.2, response: { status: 200, protocol: 'h2', headers: { 'set-cookie': 'private-proof', 'cache-control': 'no-store', 'x-vercel-cache': 'MISS' } } });
  emit('dataReceived', { requestId: 'approve', timestamp: 11.3, dataLength: 60, encodedDataLength: 42 });
  emit('loadingFinished', { requestId: 'approve', timestamp: 11.4, encodedDataLength: 150 });
  emit('loadingFailed', { requestId: 'poll', timestamp: 22, errorText: 'net::ERR_ABORTED', canceled: true });
  const report = await observer.finish();
  assert.equal(report.rows[0].requestAtEpochMs, 1_000_000);
  assert.equal(report.rows[0].failedAtEpochMs, 1_012_000);
  assert.equal(report.rows[0].outcome, 'failed-before-headers');
  assert.equal(report.rows[1].headersAtEpochMs, 1_001_200);
  assert.equal(report.rows[1].finishedAtEpochMs, 1_001_400);
  assert.equal(report.rows[1].outcome, 'body-finished');
  assert.equal(report.rows[1].decodedBytesObserved, 60);
  assert(!JSON.stringify(report).includes('private-proof'));
  assert(!JSON.stringify(report).includes('private-session'));
  assert(!JSON.stringify(report).includes('set-cookie'));
});

test('headers without body and a partially read body are distinguished at cancellation', async () => {
  const { emit, observer } = setup();
  for (const id of ['headers', 'partial']) {
    emit('requestWillBeSent', request(id));
    emit('responseReceived', { requestId: id, timestamp: 11, response: { status: 200, protocol: 'h2' } });
  }
  emit('dataReceived', { requestId: 'partial', timestamp: 12, dataLength: 4, encodedDataLength: 4 });
  for (const id of ['headers', 'partial']) emit('loadingFailed', { requestId: id, timestamp: 22, errorText: 'net::ERR_ABORTED' });
  const report = await observer.finish();
  assert.deepEqual(report.rows.map(row => row.outcome), ['failed-after-headers', 'failed-during-body']);
  assert.equal(report.rows[1].firstDataAtEpochMs, 1_002_000);
});

test('an unresolved request is retained without inventing a failure or successful body', async () => {
  const { emit, observer } = setup();
  emit('requestWillBeSent', request('noheaders'));
  emit('requestWillBeSent', request('headers'));
  emit('responseReceived', { requestId: 'headers', timestamp: 11, response: { status: 200, fromServiceWorker: true } });
  const report = await observer.finish();
  assert.deepEqual(report.rows.map(row => row.outcome), ['awaiting-headers-at-observer-end', 'unfinished-body-at-observer-end']);
  assert.equal(report.rows[0].failure, undefined);
  assert(report.rows[1].fromServiceWorker);
});

test('cache metadata rejects arbitrary secret-shaped values and unknown endpoints', async () => {
  const { emit, observer } = setup();
  for (const url of ['https://unrelated.example/sessions/private-session', 'https://user:private-proof@relay.dubhe.obelisk.build/sessions/private-session', 'https://numeron.obelisk.build/game?secret=private-proof']) emit('requestWillBeSent', request(url, url));
  emit('requestWillBeSent', request('allowed'));
  emit('responseReceived', { requestId: 'allowed', timestamp: 11, response: { status: 200, headers: {
    'cache-control': 'Bearer private-proof', 'age': 'private-proof', 'x-vercel-cache': 'private-proof',
    'cf-cache-status': 'private-proof', 'cf-ray': 'private-proof-SEA', 'x-vercel-id': 'iad1::private-proof',
    'date': 'Mon, 16 Nov 2026 10:00:00 GMT', 'content-type': 'application/json',
  } } });
  const report = await observer.finish();
  assert.equal(report.rows.length, 1);
  assert(!JSON.stringify(report).includes('private-proof'));
  assert.equal(report.rows[0].cache.cfPop, 'SEA');
  assert.equal(report.rows[0].cache.vercelRegion, 'iad1');
  assert.equal(report.rows[0].cache.cacheControl, 'other');
  assert(Number.isFinite(report.rows[0].cache.serverDateEpochMs));
});

test('bounded capture and marker allowlists do not let unrelated values persist', async () => {
  const { emit, observer } = setup({ maxRows: 1 });
  emit('requestWillBeSent', request('one'));
  emit('requestWillBeSent', request('two'));
  observer.mark('approval-tap-armed'); observer.mark('private-proof');
  const report = await observer.finish();
  assert.equal(report.rows.length, 1); assert.equal(report.dropped, 1);
  assert.deepEqual(report.marks, [{ name: 'approval-tap-armed', atEpochMs: 1_000_000 }]);
  assert(!JSON.stringify(report).includes('private-proof'));
});

test('finish detaches all listeners and is idempotent', async () => {
  const { emit, cdp, observer } = setup();
  emit('requestWillBeSent', request('one'));
  const result = await observer.finish();
  emit('requestWillBeSent', request('two')); observer.mark('approval-round-ended');
  assert.equal(await observer.finish(), result);
  assert.equal(cdp.eventNames().length, 0);
  assert.equal(result.rows.length, 1); assert.equal(result.marks.length, 0);
});

test('redirecting outside auth closes the old hop and cannot attribute the destination body to it', async () => {
  const { emit, observer } = setup();
  emit('requestWillBeSent', request('redirect'));
  emit('requestWillBeSent', { ...request('redirect', 'https://unrelated.example/'), timestamp: 11, redirectResponse: { status: 302 } });
  emit('responseReceived', { requestId: 'redirect', timestamp: 12, response: { status: 200 } });
  emit('loadingFinished', { requestId: 'redirect', timestamp: 13, encodedDataLength: 100 });
  const report = await observer.finish();
  assert.equal(report.rows.length, 1);
  assert.equal(report.rows[0].outcome, 'redirected');
  assert.equal(report.rows[0].redirectAtEpochMs, 1_001_000);
  assert.equal(report.rows[0].status, undefined);
});

test('missing monotonic request time uses the observer clock and invalid status values are discarded', async () => {
  const { emit, observer } = setup();
  const item = request('missing'); delete item.timestamp;
  emit('requestWillBeSent', item);
  emit('responseReceived', { requestId: 'missing', timestamp: 12, response: { status: 'private-proof' } });
  const report = await observer.finish();
  assert.equal(report.rows[0].headersAtEpochMs, 1_000_000);
  assert.equal(report.rows[0].status, null);
  assert(!JSON.stringify(report).includes('private-proof'));
});
