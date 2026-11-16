const SETUP_MS = 5_000;
const COMMAND_MS = 1_500;
const FINISH_MS = 3_000;
const MAX_ROWS = 512;
const FONT_PATH = /\/original-ui\/fonts\/[^/]+\.(?:ttf|otf|woff2)$/i;

const fault = (code, method) => Object.assign(new Error(code), { code, ...(method ? { method } : {}) });
const bounded = (promise, milliseconds, code, method) => {
  let timer;
  return Promise.race([
    promise,
    new Promise((_, reject) => { timer = setTimeout(() => reject(fault(code, method)), milliseconds); }),
  ]).finally(() => clearTimeout(timer));
};
const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));

/**
 * Passive, browser-context-scoped observation of the asset Worker's font fetches.
 * Call with an existing Page in context, before the font requests of interest.
 * No headers, payloads, query strings, cookies or client/target IDs are returned.
 */
export async function startWorkerFontNetwork({ context, elapsed, workerUrl }) {
  if (!context || typeof elapsed !== 'function') throw fault('WORKER_FONT_ARGUMENT_INVALID');
  let expected;
  try { expected = new URL(workerUrl); } catch { throw fault('WORKER_FONT_URL_INVALID'); }
  const production = expected.origin === 'https://numeron.obelisk.build';
  const loopback = expected.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(expected.hostname);
  if ((!production && !loopback) || expected.pathname !== '/mir2-asset-worker.js' ||
      expected.search || expected.hash || expected.username || expected.password) throw fault('WORKER_FONT_URL_NOT_ALLOWED');
  const page = context.pages().find(candidate => !candidate.isClosed());
  if (!page) throw fault('WORKER_FONT_EXISTING_PAGE_REQUIRED');
  const browser = context.browser();
  if (!browser || typeof browser.newBrowserCDPSession !== 'function') throw fault('WORKER_FONT_BROWSER_CDP_UNAVAILABLE');

  const startedAt = Date.now(), rows = [], errors = [], targets = new Map(), sessions = new Map(), rpc = new Map(), active = new Map();
  let root, seed, contextId, closed = false, serial = 0, generation = 0, workersAttached = 0;
  let ignoredOtherContexts = 0, dropped = 0, finishPromise, setupMs = 0;
  let clockOrigin = Date.now();
  try { const value = Number(elapsed()); if (Number.isFinite(value)) clockOrigin -= value; } catch {}
  const relativeNow = () => {
    try { const value = Number(elapsed()); if (Number.isFinite(value)) return Math.round(value); } catch {}
    return Math.round(Date.now() - clockOrigin);
  };
  const recordError = (code, method, protocolCode) => {
    if (errors.length < 16) errors.push({ code, ...(method ? { method } : {}), ...(Number.isFinite(protocolCode) ? { protocolCode } : {}) });
  };
  const eventAt = (entry, timestamp) => Number.isFinite(timestamp)
    ? Math.round(entry.row.requestAtMs + (timestamp - entry.timestamp) * 1_000) : relativeNow();
  const protocol = value => typeof value === 'string' && /^[A-Za-z0-9/.+_-]{1,32}$/.test(value) ? value : 'unknown';
  const finishEntry = (entry, timestamp) => {
    entry.row.finishedAtMs = eventAt(entry, timestamp);
    entry.row.durationMs = Math.max(0, entry.row.finishedAtMs - entry.row.requestAtMs);
  };

  function network(owner, method, data) {
    if (closed) return;
    const key = owner.sessionId + ':' + data.requestId;
    if (method === 'Network.requestWillBeSent') {
      const previous = active.get(key);
      if (previous) {
        if (data.redirectResponse) {
          previous.row.status = data.redirectResponse.status;
          previous.row.protocol = protocol(data.redirectResponse.protocol);
          previous.row.redirected = true;
          if (Number.isFinite(data.redirectResponse.encodedDataLength)) previous.row.encodedBytes = data.redirectResponse.encodedDataLength;
          finishEntry(previous, data.timestamp);
        } else previous.row.incompleteReason = 'request-id-reused';
        active.delete(key);
      }
      if (data.request?.method !== 'GET') return;
      let url;
      try { url = new URL(data.request.url); } catch { return; }
      if (!['http:', 'https:'].includes(url.protocol) || !FONT_PATH.test(url.pathname)) return;
      if (rows.length >= MAX_ROWS) { dropped++; return; }
      const requestAtMs = Number.isFinite(data.wallTime) ? Math.round(data.wallTime * 1_000 - clockOrigin) : relativeNow();
      const row = { source: 'service-worker-upstream', workerGeneration: owner.generation, url: url.origin + url.pathname,
        requestAtMs, observedAtMs: relativeNow(), encodedBodyBytes: 0 };
      rows.push(row); active.set(key, { row, timestamp: data.timestamp });
      return;
    }
    const entry = active.get(key);
    if (!entry) return;
    if (method === 'Network.responseReceived') {
      entry.row.status = data.response?.status;
      entry.row.protocol = protocol(data.response?.protocol);
      entry.row.headersAtMs = eventAt(entry, data.timestamp);
    } else if (method === 'Network.dataReceived') {
      if (Number.isFinite(data.encodedDataLength)) entry.row.encodedBodyBytes += data.encodedDataLength;
    } else if (method === 'Network.loadingFinished') {
      entry.row.encodedBytes = data.encodedDataLength;
      entry.row.encodedBytesComplete = true;
      finishEntry(entry, data.timestamp); active.delete(key);
    } else if (method === 'Network.loadingFailed') {
      entry.row.netError = /^net::[A-Z0-9_]+$/.test(data.errorText ?? '') ? data.errorText : 'OTHER_NETWORK_ERROR';
      const cors = data.corsErrorStatus?.corsError;
      if (typeof cors === 'string') entry.row.corsError = /^[A-Za-z][A-Za-z0-9_]{0,80}$/.test(cors) ? cors : 'OTHER_CORS_ERROR';
      entry.row.encodedBytes = entry.row.encodedBodyBytes;
      entry.row.encodedBytesComplete = false;
      finishEntry(entry, data.timestamp); active.delete(key);
    }
  }

  function onMessage(data) {
    let message;
    try { message = JSON.parse(data.message); } catch { recordError('WORKER_FONT_CDP_MESSAGE_INVALID'); return; }
    if (message.id) {
      const key = data.sessionId + ':' + message.id, pending = rpc.get(key);
      if (!pending) return;
      rpc.delete(key); clearTimeout(pending.timer);
      if (message.error) pending.reject(fault('WORKER_FONT_CDP_COMMAND_REJECTED', pending.method));
      else pending.resolve(message.result);
      return;
    }
    const owner = sessions.get(data.sessionId);
    if (owner && ['Network.requestWillBeSent', 'Network.responseReceived', 'Network.dataReceived',
      'Network.loadingFinished', 'Network.loadingFailed'].includes(message.method)) {
      try { network(owner, message.method, message.params); } catch { recordError('WORKER_FONT_EVENT_OBSERVATION_FAILED', message.method); }
    }
  }

  async function workerCommand(sessionId, method) {
    const id = ++serial, key = sessionId + ':' + id;
    const reply = new Promise((resolve, reject) => {
      const timer = setTimeout(() => { rpc.delete(key); reject(fault('WORKER_FONT_CDP_COMMAND_TIMEOUT', method)); }, COMMAND_MS);
      rpc.set(key, { resolve, reject, timer, method });
    });
    // A slow outer transport must not produce an unhandled nested timeout.
    void reply.catch(() => {});
    try {
      await bounded(root.send('Target.sendMessageToTarget', {
        sessionId, message: JSON.stringify({ id, method, params: {} }),
      }), COMMAND_MS, 'WORKER_FONT_CDP_TRANSPORT_TIMEOUT', method);
      return await reply;
    } catch (error) {
      const pending = rpc.get(key);
      if (pending) { rpc.delete(key); clearTimeout(pending.timer); pending.reject(error); }
      throw error;
    }
  }

  const matches = info => info.type === 'service_worker' && info.url === expected.href;
  async function attach(info) {
    if (closed || !matches(info)) return;
    if ((info.browserContextId ?? null) !== contextId) { ignoredOtherContexts++; return; }
    if (targets.has(info.targetId)) return targets.get(info.targetId).promise;
    const state = { generation: ++generation, sessionId: null, promise: null };
    targets.set(info.targetId, state);
    state.promise = (async () => {
      const connection = root;
      if (!connection || closed) return;
      const result = await bounded(connection.send('Target.attachToTarget', { targetId: info.targetId, flatten: false }),
        COMMAND_MS, 'WORKER_FONT_CDP_ATTACH_TIMEOUT', 'Target.attachToTarget');
      state.sessionId = result.sessionId;
      if (closed) {
        await bounded(connection.send('Target.detachFromTarget', { sessionId: result.sessionId }), 500, 'WORKER_FONT_LATE_DETACH_TIMEOUT').catch(() => {});
        return;
      }
      sessions.set(result.sessionId, state);
      await workerCommand(result.sessionId, 'Network.enable');
      if (!closed) workersAttached++;
    })().catch(error => { if (!closed) recordError(error.code ?? 'WORKER_FONT_CDP_ATTACH_FAILED', error.method); });
    await state.promise;
  }
  const onCreated = data => { void attach(data.targetInfo); };
  const onDestroyed = data => { targets.delete(data.targetId); };
  const onDetached = data => {
    sessions.delete(data.sessionId);
    for (const [key, entry] of active) if (key.startsWith(data.sessionId + ':')) {
      entry.row.incompleteReason = 'worker-target-detached'; active.delete(key);
    }
  };

  async function cleanup() {
    closed = true;
    if (root) {
      root.off('Target.targetCreated', onCreated);
      root.off('Target.targetDestroyed', onDestroyed);
      root.off('Target.receivedMessageFromTarget', onMessage);
      root.off('Target.detachedFromTarget', onDetached);
    }
    for (const pending of rpc.values()) { clearTimeout(pending.timer); pending.reject(fault('WORKER_FONT_OBSERVER_FINISHED', pending.method)); }
    rpc.clear();
    const detach = [...sessions.keys()].map(sessionId => bounded(root.send('Target.detachFromTarget', { sessionId }), 500,
      'WORKER_FONT_DETACH_TIMEOUT').catch(() => {}));
    await Promise.allSettled(detach);
    sessions.clear();
    if (seed) { const current = seed; seed = null; await bounded(current.detach(), 500, 'WORKER_FONT_PAGE_DETACH_TIMEOUT').catch(() => {}); }
    if (root) { const current = root; root = null; await bounded(current.detach(), 500, 'WORKER_FONT_ROOT_DETACH_TIMEOUT').catch(() => {}); }
  }

  async function setup() {
    const acquiredSeed = await context.newCDPSession(page);
    if (closed) { await bounded(acquiredSeed.detach(), 500, 'WORKER_FONT_LATE_PAGE_DETACH_TIMEOUT').catch(() => {}); return; }
    seed = acquiredSeed;
    const { targetInfo } = await acquiredSeed.send('Target.getTargetInfo');
    if (closed) return;
    contextId = targetInfo.browserContextId ?? null;
    await acquiredSeed.detach();
    if (seed === acquiredSeed) seed = null;
    if (closed) return;
    const acquiredRoot = await browser.newBrowserCDPSession();
    if (closed) { await bounded(acquiredRoot.detach(), 500, 'WORKER_FONT_LATE_ROOT_DETACH_TIMEOUT').catch(() => {}); return; }
    root = acquiredRoot;
    root.on('Target.receivedMessageFromTarget', onMessage);
    root.on('Target.targetCreated', onCreated);
    root.on('Target.targetDestroyed', onDestroyed);
    root.on('Target.detachedFromTarget', onDetached);
    const filter = [{ type: 'service_worker', exclude: false }, { exclude: true }];
    await root.send('Target.setDiscoverTargets', { discover: true, filter });
    if (closed) return;
    const { targetInfos } = await root.send('Target.getTargets', { filter });
    await Promise.allSettled(targetInfos.filter(matches).map(attach));
    if (closed) return;
    setupMs = Date.now() - startedAt;
  }
  try { await bounded(setup(), SETUP_MS, 'WORKER_FONT_SETUP_TIMEOUT'); }
  catch (error) {
    await bounded(cleanup(), FINISH_MS, 'WORKER_FONT_SETUP_CLEANUP_TIMEOUT').catch(() => {});
    throw fault(error.code ?? 'WORKER_FONT_SETUP_FAILED', error.method);
  }

  return {
    finish() {
      return finishPromise ??= (async () => {
        const finishAt = Date.now();
        // Flush already-queued protocol messages; never wait on application I/O.
        await pause(25);
        let cleanupComplete = true;
        try { await bounded(cleanup(), FINISH_MS - 100, 'WORKER_FONT_FINISH_TIMEOUT'); }
        catch { cleanupComplete = false; recordError('WORKER_FONT_FINISH_TIMEOUT'); }
        for (const entry of active.values()) entry.row.incompleteReason ??= 'observation-finished';
        active.clear();
        return { kind: 'r16-passive-worker-font-network-v1', workerUrl: expected.origin + expected.pathname,
          contextScoped: true, source: 'service-worker-upstream-cdp', setupMs, finishMs: Date.now() - finishAt,
          workersAttached, otherContextWorkersIgnored: ignoredOtherContexts, droppedRequests: dropped,
          cleanupComplete, observerErrors: errors, rows,
          limits: ['future-worker requests made before Network.enable may be unobserved; no target is paused',
            'encodedBytes from loadingFinished includes protocol overhead; failed rows only expose observed encoded body counts',
            'only exact Worker URL in the supplied browser context is observed; no headers or payloads are accessed'] };
      })();
    },
  };
}
