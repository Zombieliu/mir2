import { writeFile } from 'node:fs/promises';

const GAME = 'https://numeron.obelisk.build';
const RELAY = 'https://relay.dubhe.obelisk.build';
const MARKS = new Set(['approval-tap-armed', 'approval-round-ended', 'desktop-in-memory-qr-ready']);

function kindOf(raw) {
  let url;
  try { url = new URL(raw); } catch { return null; }
  if (url.username || url.password) return null;
  if (url.origin === GAME) return ({
    '/api/wallet/dubhe/session': 'firstparty-create',
    '/api/wallet/dubhe/session/status': 'firstparty-status',
    '/api/channels/session/exchange': 'firstparty-exchange',
  })[url.pathname] ?? null;
  if (url.origin !== RELAY) return null;
  if (url.pathname === '/sessions') return 'relay-create';
  if (/^\/sessions\/[A-Za-z0-9_-]+\/approve$/.test(url.pathname)) return 'relay-approve';
  if (/^\/sessions\/[A-Za-z0-9_-]+$/.test(url.pathname)) return 'relay-session';
  return null;
}

function safeHeaders(headers) {
  const values = new Map(Object.entries(headers ?? {}).map(([k, v]) => [k.toLowerCase(), String(v)]));
  const result = {};
  const cache = values.get('cache-control');
  if (cache) result.cacheControl = cache.length <= 220 && /^(?:(?:public|private|no-store|no-cache|must-revalidate|proxy-revalidate|immutable|(?:s-maxage|max-age|stale-while-revalidate|stale-if-error)=\d+)\s*,?\s*)+$/i.test(cache)
    ? cache.toLowerCase() : 'other';
  for (const [header, key, pattern] of [
    ['age', 'ageSeconds', /^\d{1,10}$/],
    ['x-vercel-cache', 'vercelCache', /^(?:HIT|MISS|STALE|BYPASS|PRERENDER|REVALIDATED)$/i],
    ['cf-cache-status', 'cfCache', /^(?:HIT|MISS|STALE|BYPASS|DYNAMIC|REVALIDATED|UPDATING|EXPIRED)$/i],
  ]) {
    if (values.has(header)) result[key] = pattern.test(values.get(header)) ? values.get(header) : 'other';
  }
  const date = Date.parse(values.get('date'));
  if (Number.isFinite(date)) result.serverDateEpochMs = date;
  const type = values.get('content-type');
  if (type) result.jsonContentType = /^application\/json(?:\s*;\s*charset=utf-8)?$/i.test(type);
  const pop = values.get('cf-ray')?.split('-').at(-1);
  if (/^[A-Z]{3,4}$/.test(pop ?? '')) result.cfPop = pop;
  const region = values.get('x-vercel-id')?.split('::')[0];
  if (/^[a-z]{3}\d$/.test(region ?? '')) result.vercelRegion = region;
  return result;
}

/** Observe enabled CDP Network events; never send a protocol command or retain capabilities. */
export function attachAuthWireObserver({ cdp, side, output, now = Date.now, maxRows = 512 }) {
  if (!['desktop', 'mobile'].includes(side) || !Number.isInteger(maxRows) || maxRows < 1 || maxRows > 512) throw Error('Invalid observer scope');
  const startedAtEpochMs = now(), rows = [], requests = new Map(), marks = [];
  let stopped = false, dropped = 0, resultPromise;
  const time = (item, timestamp) => Number.isFinite(timestamp) && item.offset !== null ? Math.round(timestamp * 1000 + item.offset) : now();
  const request = event => {
    if (stopped) return;
    const previous = requests.get(event.requestId);
    if (previous && event.redirectResponse) {
      previous.row.redirectAtEpochMs = time(previous, event.timestamp);
      previous.row.outcome = 'redirected';
      requests.delete(event.requestId);
    }
    const kind = kindOf(event.request?.url);
    if (!kind) return;
    if (rows.length >= maxRows) { dropped++; return; }
    const at = Number.isFinite(event.wallTime) ? Math.round(event.wallTime * 1000) : now();
    const row = { ordinal: rows.length + 1, kind, method: ['GET', 'POST', 'OPTIONS'].includes(event.request.method) ? event.request.method : 'other',
      requestAtEpochMs: at, dataEvents: 0, decodedBytesObserved: 0, encodedBytesObserved: 0 };
    rows.push(row);
    requests.set(event.requestId, { row, offset: Number.isFinite(event.timestamp) ? at - event.timestamp * 1000 : null });
  };
  const response = event => {
    const item = requests.get(event.requestId); if (!item || stopped) return;
    const r = event.response ?? {};
    Object.assign(item.row, { headersAtEpochMs: time(item, event.timestamp), status: Number.isInteger(r.status) && r.status >= 100 && r.status <= 599 ? r.status : null,
      protocol: ['h2', 'h3', 'http/1.1', 'http/1.0'].includes(r.protocol) ? r.protocol : 'other',
      fromDiskCache: r.fromDiskCache === true, fromServiceWorker: r.fromServiceWorker === true, cache: safeHeaders(r.headers) });
  };
  const data = event => {
    const item = requests.get(event.requestId); if (!item || stopped) return;
    const row = item.row, at = time(item, event.timestamp);
    row.dataEvents++; row.firstDataAtEpochMs ??= at; row.lastDataAtEpochMs = at;
    for (const [from, to] of [['dataLength', 'decodedBytesObserved'], ['encodedDataLength', 'encodedBytesObserved']]) {
      if (Number.isFinite(event[from]) && event[from] >= 0) row[to] += event[from];
    }
  };
  const finished = event => {
    const item = requests.get(event.requestId); if (!item || stopped) return;
    item.row.finishedAtEpochMs = time(item, event.timestamp);
    if (Number.isFinite(event.encodedDataLength) && event.encodedDataLength >= 0) item.row.totalEncodedBytes = event.encodedDataLength;
    item.row.outcome = item.row.headersAtEpochMs === undefined ? 'finished-without-observed-headers' : 'body-finished';
  };
  const failed = event => {
    const item = requests.get(event.requestId); if (!item || stopped) return;
    const row = item.row;
    row.failedAtEpochMs = time(item, event.timestamp);
    row.failure = /^net::ERR_[A-Z0-9_]+$/.test(event.errorText ?? '') ? event.errorText : 'transport-failure';
    row.cancelled = event.canceled === true;
    row.outcome = row.headersAtEpochMs === undefined ? 'failed-before-headers' : row.dataEvents > 0 ? 'failed-during-body' : 'failed-after-headers';
  };
  const listeners = { 'Network.requestWillBeSent': request, 'Network.responseReceived': response, 'Network.dataReceived': data,
    'Network.loadingFinished': finished, 'Network.loadingFailed': failed };
  for (const [name, listener] of Object.entries(listeners)) cdp.on(name, listener);
  return {
    mark(name) { if (!stopped && MARKS.has(name) && marks.length < 32) marks.push({ name, atEpochMs: now() }); },
    finish() {
      if (resultPromise) return resultPromise;
      stopped = true;
      for (const [name, listener] of Object.entries(listeners)) cdp.off(name, listener);
      const report = { kind: 'passive-auth-wire-v1', side, startedAtEpochMs, endedAtEpochMs: now(), dropped, marks,
        scope: 'Existing CDP Network events only. No fetch, interception, protocol command, focus change, URL, identifier, request headers, payload, credentials or signature retained.',
        rows: rows.map(row => ({ ...row, outcome: row.outcome ?? (row.headersAtEpochMs === undefined ? 'awaiting-headers-at-observer-end' : 'unfinished-body-at-observer-end') })) };
      resultPromise = (async () => { if (output) await writeFile(output, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' }); return report; })();
      return resultPromise;
    },
  };
}
