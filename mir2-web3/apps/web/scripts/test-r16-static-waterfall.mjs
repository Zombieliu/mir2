import assert from 'node:assert/strict';
import { EventEmitter } from 'node:events';
import test from 'node:test';
import { startStaticWaterfall } from './lib/r16-static-waterfall.mjs';

const app = 'https://numeron.obelisk.build', cdn = 'https://assets.mir2.obelisk.build/mir2/v/fixed-r16';
function fixture(extra = {}) {
  const context = new EventEmitter(); let at = 0;
  const recorder = startStaticWaterfall({ context, elapsed: () => at, sourceBases: [app, cdn], ...extra });
  const request = (url = app + '/original-ui/Prguse/44.png', options = {}) => ({
    method: () => options.method ?? 'GET', url: () => url, serviceWorker: () => options.worker ?? null,
    resourceType: () => 'image', redirectedFrom: () => options.from ?? null,
    timing: () => options.timing ?? { requestStart: 1, responseStart: 20, responseEnd: 90 },
    sizes: options.sizes ?? (async () => ({ responseBodySize: 20, responseHeadersSize: 100 })),
    failure: () => ({ errorText: options.error ?? 'net::ERR_ABORTED' }),
  });
  const response = (r, status = 200) => ({ request: () => r, status: () => status, fromServiceWorker: () => true });
  return { context, recorder, request, response, clock: value => { at = value; } };
}

test('records page and Worker assets separately with headers/body timing and encoded size metadata', async () => {
  const f = fixture();
  for (const worker of [null, {}]) {
    const request = f.request(app + '/original-ui/Prguse/44.png', { worker });
    f.context.emit('request', request); f.clock(30); f.context.emit('response', f.response(request, worker ? 503 : 200));
    f.clock(100); f.context.emit('requestfinished', request);
  }
  const proof = await f.recorder.stop();
  assert.deepEqual(proof.rows.map(row => row.source), ['page', 'service-worker-upstream']);
  assert.deepEqual(proof.rows.map(row => row.status), [200, 503]);
  assert.equal(proof.rows[0].requestToHeadersMs, 19); assert.equal(proof.rows[0].headersToEndMs, 70);
  assert.equal(proof.rows[0].browserReportedResponseBodyBytes, 20);
  assert.equal(proof.pendingRequests, 0); assert.equal(proof.pendingSizeReadsAtStop, 0);
});

test('excludes auth, payment, websocket, POST and unrelated origins; strips asset query and hash', async () => {
  const f = fixture();
  for (const path of ['/api/wallet/dubhe/session', '/api/channels/session/exchange', '/api/billing/order', '/playtest/ws'])
    f.context.emit('request', f.request(app + path + '?secret=synthetic'));
  f.context.emit('request', f.request(app + '/original-ui/44.png', { method: 'POST' }));
  f.context.emit('request', f.request('https://untrusted.example/original-ui/44.png'));
  f.context.emit('request', f.request('https://user:synthetic@numeron.obelisk.build/original-ui/44.png'));
  f.context.emit('request', f.request(cdn + '/original-ui/fonts/test.ttf?token=synthetic#private'));
  const proof = await f.recorder.stop();
  assert.equal(proof.rows.length, 1); assert.equal(proof.rows[0].category, 'font');
  assert.equal(proof.rows[0].url, cdn + '/original-ui/fonts/test.ttf');
  assert(!JSON.stringify(proof).includes('synthetic')); assert(!JSON.stringify(proof).includes('private'));
});

test('fixed CDN prefix is a whole segment and cannot cover another release', async () => {
  const f = fixture();
  f.context.emit('request', f.request(cdn + '-other/original-ui/44.png'));
  f.context.emit('request', f.request(cdn + '/original-ui/44.png'));
  assert.equal((await f.recorder.stop()).rows.length, 1);
});

test('never reads request or response headers, cookies or payloads', async () => {
  const f = fixture(), request = f.request(), response = f.response(request);
  for (const target of [request, response]) for (const name of ['headers', 'allHeaders', 'postData', 'body', 'text', 'json'])
    Object.defineProperty(target, name, { get() { throw new Error('forbidden private field read'); } });
  f.context.emit('request', request); f.context.emit('response', response); f.context.emit('requestfinished', request);
  assert.equal((await f.recorder.stop()).rows[0].outcome, 'finished');
});

test('row budget remains bounded, counts omitted requests and retains pending and failure evidence', async () => {
  const f = fixture({ maxRows: 2 }), pending = f.request(), failed = f.request();
  f.context.emit('request', pending); f.context.emit('request', pending);
  f.context.emit('request', failed); f.context.emit('request', f.request());
  f.clock(50); f.context.emit('requestfailed', failed);
  const proof = await f.recorder.stop();
  assert.equal(proof.capturedRows, 2); assert.equal(proof.droppedRows, 1); assert.equal(proof.truncated, true);
  assert.equal(proof.pendingRequests, 1); assert.equal(proof.rows[0].outcome, 'pending');
  assert.equal(proof.rows[1].failure, 'net::ERR_ABORTED');
});

test('redirects keep separate rows and size failures never leak errors', async () => {
  const f = fixture(), a = f.request();
  f.context.emit('request', a); f.context.emit('response', f.response(a, 302)); f.context.emit('requestfinished', a);
  const b = f.request(app + '/original-ui/44.png', { from: a, sizes: async () => { throw Error('synthetic-private'); } });
  f.context.emit('request', b); f.context.emit('response', f.response(b)); f.context.emit('requestfinished', b);
  const proof = await f.recorder.stop();
  assert.equal(proof.rows[1].redirectedFrom, 0); assert.equal(proof.rows[1].sizesAvailable, false);
  assert(!JSON.stringify(proof).includes('synthetic-private'));
});

test('stop detaches listeners, bounds late size reads and freezes exported evidence', async () => {
  const f = fixture(); let resolve;
  const request = f.request(app + '/original-ui/44.png', { sizes: () => new Promise(done => { resolve = done; }) });
  f.context.emit('request', request); f.context.emit('requestfinished', request);
  const proof = await f.recorder.stop({ settleMs: 0 });
  assert.equal(proof.pendingSizeReadsAtStop, 1);
  for (const event of ['request', 'response', 'requestfinished', 'requestfailed']) assert.equal(f.context.listenerCount(event), 0);
  const before = JSON.stringify(proof.rows);
  resolve({ responseBodySize: 1000 }); await new Promise(done => setTimeout(done, 0));
  assert.equal(JSON.stringify(f.recorder.snapshot().rows), before);
  proof.rows[0].url = 'modified'; assert.notEqual(f.recorder.snapshot().rows[0].url, 'modified');
});

test('rejects broad, credential-bearing or malformed source and row budgets', () => {
  for (const sourceBases of [['http://remote.example'], [app + '?token=synthetic'], [app + '/%2e%2e'], ['https://user:pass@example.com']])
    assert.throws(() => fixture({ sourceBases }), /invalid static source/);
  for (const maxRows of [0, 8193, NaN, 1.2]) assert.throws(() => fixture({ maxRows }), /invalid waterfall arguments/);
});
