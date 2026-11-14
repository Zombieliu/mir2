const RESOURCE_PATH = /^\/(?:_next\/static\/|original-ui\/|original-map\/|bevy-runtime\/|bootstrap\/|client-core\/|bevy-entity-atlases\/|bevy-catalog\/|generated\/(?:crystal-packs\/full\/|map-atlas\/|original-map-blend\/)|api\/(?:r2-proxy\/|remote-asset\/|original-ui-meta$|asset-manifest$|scene\/(?:crystal|starter|collision)$))/;
const ENTRY_PATH = /^\/(?:game|mir2-asset-worker\.js)$/;
const ORIGINS = new Set(["https://numeron.obelisk.build", "https://assets.mir2.obelisk.build",
  "https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev"]);
const number = value => typeof value === "number" && Number.isFinite(value);
const nonnegative = value => number(value) && value >= 0;
const TIMING_FIELDS = ["startTime", "domainLookupStart", "domainLookupEnd", "connectStart", "secureConnectionStart",
  "connectEnd", "requestStart", "responseStart", "responseEnd"];

/** Public static paths only; no auth APIs, query strings, headers or bodies. */
export function staticDeliveryUrl(value) {
  try {
    const url = new URL(value);
    const local = url.protocol === "http:" && ["localhost", "127.0.0.1", "[::1]"].includes(url.hostname);
    const staticPath = url.pathname.replace(/^\/mir2\/v\/[A-Za-z0-9_-]+\//, "/");
    if ((!ORIGINS.has(url.origin) && !local) || url.username || url.password
      || (!RESOURCE_PATH.test(staticPath) && !(ENTRY_PATH.test(url.pathname)
        && (url.origin === "https://numeron.obelisk.build" || local)))) return null;
    return url.origin + url.pathname;
  } catch { return null; }
}

/** Passive context events; older Playwright needs a Worker CDP companion for upstream coverage. */
export function observeStaticDelivery({context, elapsed, maxRows = 10_000, finishMs = 2000}) {
  if (!context || typeof context.on !== "function" || typeof elapsed !== "function"
    || !Number.isSafeInteger(maxRows) || maxRows < 1 || maxRows > 20_000
    || !Number.isSafeInteger(finishMs) || finishMs < 1 || finishMs > 5000) throw Error("invalid static observer options");
  const rows = [], requests = new Map(), pending = new Set(), handlers = [];
  let closed = false, dropped = 0, errors = 0, sequence = 0, resultPromise;
  const now = () => {const at = elapsed(); if (!nonnegative(at)) throw Error("invalid observation clock"); return at;};
  const time = (request, row) => {
    try {
      const timing = request.timing();
      row.timing = Object.fromEntries(TIMING_FIELDS.filter(key => number(timing[key]) && timing[key] >= -1)
        .map(key => [key, timing[key]]));
    } catch {errors++;}
  };
  const add = (event, handler) => {context.on(event, handler); handlers.push([event, handler]);};
  add("request", request => {
    if (closed) return;
    try {
      if (request.method() !== "GET") return;
      const url = staticDeliveryUrl(request.url());
      if (!url) return;
      if (rows.length >= maxRows) {dropped++; return;}
      const previous = requests.get(request.redirectedFrom?.());
      const row = {id: ++sequence, source: request.serviceWorker?.() ? "service-worker-upstream" : "page",
        url, requestAtMs: now(), resourceType: request.resourceType(),
        ...(previous ? {redirectedFrom: previous.id} : {})};
      time(request, row); rows.push(row); requests.set(request, row);
    } catch {errors++;}
  });
  add("response", response => {
    if (closed) return;
    try {
      const request = response.request(), row = requests.get(request);
      if (!row) return;
      row.headersAtMs = now(); row.status = response.status(); row.fromServiceWorker = response.fromServiceWorker();
      time(request, row);
    } catch {errors++;}
  });
  add("requestfailed", request => {
    if (closed) return;
    try {
      const row = requests.get(request); if (!row) return;
      row.failedAtMs = now();
      const code = request.failure()?.errorText;
      row.failure = /^net::[A-Z0-9_]+$/.test(code ?? "") ? code : "OTHER_NETWORK_ERROR";
      time(request, row);
    } catch {errors++;}
  });
  add("requestfinished", request => {
    if (closed) return;
    try {
      const row = requests.get(request); if (!row) return;
      row.finishedAtMs = now(); time(request, row);
      const task = Promise.resolve().then(() => request.sizes()).then(sizes => {
        if (closed) return;
        row.sizes = Object.fromEntries(["requestBodySize", "requestHeadersSize", "responseBodySize", "responseHeadersSize"]
          .filter(key => nonnegative(sizes[key])).map(key => [key, sizes[key]]));
      }).catch(() => {if (!closed) {errors++; row.sizesUnavailable = true;}}).finally(() => pending.delete(task));
      pending.add(task);
    } catch {errors++;}
  });
  return {finish() {
    return resultPromise ??= (async () => {
      for (const [event, handler] of handlers) context.off(event, handler);
      const pendingCount = pending.size;
      let timer, drained = true;
      await Promise.race([Promise.allSettled([...pending]), new Promise(resolve => {
        timer = setTimeout(() => {drained = false; resolve();}, finishMs);
      })]).finally(() => clearTimeout(timer));
      closed = true;
      for (const row of rows) if (row.finishedAtMs === undefined && row.failedAtMs === undefined) row.incomplete = true;
      requests.clear();
      return {schema: "mir2.passive-static-delivery.v1", observedUntilMs: now(), rows,
        droppedRequests: dropped, observationErrors: errors, pendingSizesAtFinish: pendingCount,
        sizeReadsDrained: drained, eventListenersRemoved: true,
        observedWorkerRows: rows.filter(row => row.source === "service-worker-upstream").length,
        workerEventCoverageInferred: false,
        coverage: "BrowserContext GET events for allowlisted public static paths; older Playwright requires a separate Worker CDP companion. In-flight requests stay incomplete.",
        payloadsRead: false, headersRead: false, requestsIntercepted: false, networkSettingsChanged: false,
        totalConnectionOrActorIdentityBound: false};
    })();
  }};
}
