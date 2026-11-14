const STATIC_PATH = /^(?:\/_next\/static\/|\/bootstrap\/r16-|\/original-ui\/|\/original-map\/|\/bevy-runtime\/|\/bevy-entity-atlases\/|\/api\/r2-proxy\/|\/api\/original-ui-meta(?:\/|$)|\/api\/original-map(?:\/|$)|\/mir2-asset-worker\.js$|\/remote-asset-release\.json$|\/asset-manifest\.json$)/;
const TIMINGS = ['domainLookupStart', 'domainLookupEnd', 'connectStart', 'secureConnectionStart',
  'connectEnd', 'requestStart', 'responseStart', 'responseEnd'];
const TYPES = new Set(['document', 'stylesheet', 'image', 'media', 'font', 'script', 'texttrack',
  'xhr', 'fetch', 'eventsource', 'websocket', 'manifest', 'other']);
const integer = value => Number.isSafeInteger(value) && value >= 0;

function sourcePrefix(value) {
  if (typeof value !== 'string' || value.length > 2048) throw new Error('invalid static source');
  const url = new URL(value);
  const loopback = url.protocol === 'http:' && ['127.0.0.1', 'localhost', '[::1]'].includes(url.hostname);
  if ((url.protocol !== 'https:' && !loopback) || url.username || url.password || url.search || url.hash
    || /%|\/\//.test(url.pathname)
    || (value !== url.origin + url.pathname && !(url.pathname === '/' && value === url.origin)))
    throw new Error('invalid static source');
  return { origin: url.origin, prefix: url.pathname === '/' ? '' : url.pathname.replace(/\/$/, '') };
}

export function normalizeStaticResourceSources(sourceBases) {
  if (!Array.isArray(sourceBases) || sourceBases.length < 1 || sourceBases.length > 8) throw new Error('invalid static sources');
  return sourceBases.map(sourcePrefix).sort((left, right) => right.prefix.length - left.prefix.length);
}

export function staticResourceDescriptor(value, sources) {
  let url;
  try { url = new URL(value); } catch { return null; }
  if (url.username || url.password || !['http:', 'https:'].includes(url.protocol)) return null;
  const source = sources.find(item => item.origin === url.origin
    && (item.prefix === '' || url.pathname.startsWith(item.prefix + '/')));
  if (!source) return null;
  const assetPath = url.pathname.slice(source.prefix.length);
  if (!STATIC_PATH.test(assetPath)) return null;
  let category = 'metadata';
  if (assetPath.startsWith('/_next/static/')) category = 'app-static';
  else if (assetPath.startsWith('/bootstrap/')) category = 'bootstrap';
  else if (assetPath.includes('/fonts/')) category = 'font';
  else if (assetPath.startsWith('/bevy-runtime/')) category = 'runtime';
  else if (assetPath.startsWith('/original-map/')) category = 'map';
  else if (assetPath.startsWith('/original-ui/')) category = /\.(?:wav|mp3|ogg)$/i.test(assetPath) ? 'audio' : 'ui';
  else if (assetPath.startsWith('/bevy-entity-atlases/')) category = 'atlas';
  else if (assetPath.startsWith('/api/r2-proxy/')) category = 'proxy';
  else if (assetPath === '/mir2-asset-worker.js') category = 'worker';
  return { url: url.origin + url.pathname, category };
}

/** Passive, context-owned Playwright events. No routing, bodies, headers or cookies. */
export function startStaticWaterfall({ context, elapsed, sourceBases, maxRows = 4096 }) {
  if (!context?.on || !context?.off || typeof elapsed !== 'function'
    || !Array.isArray(sourceBases) || sourceBases.length < 1 || sourceBases.length > 8
    || !Number.isSafeInteger(maxRows) || maxRows < 1 || maxRows > 8192) throw new Error('invalid waterfall arguments');
  const sources = normalizeStaticResourceSources(sourceBases);
  const rows = [], active = new Map(), byRequest = new WeakMap(), pending = new Set();
  let stopped = false, dropped = 0, pendingAtStop = 0;
  const now = () => {
    const result = elapsed();
    if (!Number.isFinite(result) || result < 0) throw new Error('invalid waterfall clock');
    return Math.round(result);
  };
  const startedAtMs = now();
  function timing(request, row) {
    try {
      const values = request.timing();
      row.timingMs = Object.fromEntries(TIMINGS.filter(name => Number.isFinite(values[name])
        && values[name] >= -1).map(name => [name, values[name]]));
      if (values.responseStart >= 0 && values.requestStart >= 0)
        row.requestToHeadersMs = Math.max(0, values.responseStart - values.requestStart);
      if (values.responseEnd >= 0 && values.responseStart >= 0)
        row.headersToEndMs = Math.max(0, values.responseEnd - values.responseStart);
    } catch { row.timingAvailable = false; }
  }
  function onRequest(request) {
    if (stopped || byRequest.has(request) || request.method() !== 'GET') return;
    const descriptor = staticResourceDescriptor(request.url(), sources);
    if (!descriptor) return;
    if (rows.length >= maxRows) { dropped++; return; }
    const type = request.resourceType();
    const row = { ordinal: rows.length, ...descriptor,
      source: request.serviceWorker() ? 'service-worker-upstream' : 'page',
      resourceType: TYPES.has(type) ? type : 'other', requestAtMs: now(), outcome: 'pending' };
    const predecessor = request.redirectedFrom();
    if (predecessor && byRequest.has(predecessor)) row.redirectedFrom = byRequest.get(predecessor).ordinal;
    rows.push(row); active.set(request, row); byRequest.set(request, row);
  }
  function onResponse(response) {
    if (stopped) return;
    const row = active.get(response.request());
    if (!row) return;
    const status = response.status();
    if (integer(status) && status <= 599) row.status = status;
    row.headersAtMs = now();
    row.fromServiceWorker = response.fromServiceWorker() === true;
  }
  function finish(request, failed) {
    if (stopped) return;
    const row = active.get(request);
    if (!row) return;
    active.delete(request);
    row.finishedAtMs = now();
    row.observedDurationMs = Math.max(0, row.finishedAtMs - row.requestAtMs);
    row.outcome = failed ? 'failed' : 'finished';
    timing(request, row);
    if (failed) {
      const failure = request.failure()?.errorText;
      row.failure = typeof failure === 'string' && /^net::ERR_[A-Z0-9_]{1,64}$/.test(failure) ? failure : 'other';
      return;
    }
    const work = Promise.resolve().then(() => request.sizes()).then(sizes => {
      if (stopped) return;
      row.browserReportedResponseBodyBytes = integer(sizes.responseBodySize) ? sizes.responseBodySize : null;
      row.browserReportedResponseHeadersBytes = integer(sizes.responseHeadersSize) ? sizes.responseHeadersSize : null;
    }).catch(() => { if (!stopped) row.sizesAvailable = false; }).finally(() => pending.delete(work));
    pending.add(work);
  }
  const onFinished = request => finish(request, false);
  const onFailed = request => finish(request, true);
  context.on('request', onRequest); context.on('response', onResponse);
  context.on('requestfinished', onFinished); context.on('requestfailed', onFailed);
  const snapshot = () => ({ schemaVersion: 1, kind: 'r16-passive-static-waterfall', startedAtMs,
    capturedRows: rows.length, maxRows, droppedRows: dropped, truncated: dropped > 0,
    pendingRequests: active.size, pendingSizeReads: pending.size, pendingSizeReadsAtStop: pendingAtStop,
    stopped, workerNetworkEventsRequested: process.env.PW_EXPERIMENTAL_SERVICE_WORKER_NETWORK_EVENTS === '1',
    workerRowsObserved: rows.filter(row => row.source === 'service-worker-upstream').length,
    scope: 'Allowlisted GET assets in one browser context. Worker upstream events require Playwright network observation; page and service-worker rows may represent the same fetch. Byte counts are browser-reported, not a deduplicated network total.',
    rows: structuredClone(rows) });
  return {
    snapshot,
    async stop({ settleMs = 1000 } = {}) {
      if (!Number.isSafeInteger(settleMs) || settleMs < 0 || settleMs > 3000) throw new Error('invalid waterfall settle budget');
      if (!stopped) {
        context.off('request', onRequest); context.off('response', onResponse);
        context.off('requestfinished', onFinished); context.off('requestfailed', onFailed);
        let timer;
        try { await Promise.race([Promise.allSettled([...pending]), new Promise(resolve => { timer = setTimeout(resolve, settleMs); })]); }
        finally { clearTimeout(timer); }
        pendingAtStop = pending.size;
        stopped = true;
      }
      return snapshot();
    },
  };
}
