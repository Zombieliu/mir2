/**
 * mir2-probe — passive frame-timing observer (Layer 6).
 *
 * This is the only layer that requires a genuinely new observer in P1: every
 * other layer reads an existing window global. Browser-side FPS + longtasks:
 *
 *  - longtask: PerformanceObserver entryType `longtask` pushes tasks > 50ms
 *    that actually blocked the main thread. Cross-checked against any
 *    `sceneAssetReadiness.ready` style work that flares during sustained run.
 *  - rAF deltas: a single rAF loop records frame timestamps into a 30-deep
 *    ring for FPS estimation. We deliberately do NOT chain rAF with
 *    `requestAnimationFrame` of the existing 30Hz motion clock to avoid
 *    coupling probe timing with gameplay timing — a probe stall should not
 *    feedback into the scene clock.
 *
 * No `window.__mir2*` global depends on this module — it is read by the probe
 * SDK at sample time, and exposes no observable state otherwise. Output is
 * stored in a module-local ring buffer; nothing is broadcast to other layers.
 */

type LongTaskAttribution = {
  name: string | null;
  entryType: string | null;
  containerType: string | null;
  containerName: string | null;
  containerSrc: string | null;
  containerId: string | null;
  scriptUrl: string | null;
};

type LongTaskBucket = {
  atMs: number;
  duration: number;
  name: string | null;
  attribution: LongTaskAttribution[];
};

type ResourceBucket = {
  atMs: number;
  startTime: number;
  responseEnd: number;
  duration: number;
  path: string;
  initiatorType: string | null;
  transferSize: number | null;
  encodedBodySize: number | null;
  decodedBodySize: number | null;
  deliveryType: string | null;
};

const RING_MAX = 30;
const LONGTASK_WINDOW_MS = 1000;
const LONGTASK_RING_MAX = 256;
const LONGTASK_DETAIL_MAX = 8;
const RESOURCE_RING_MAX = 256;
const RECENT_RESOURCE_WINDOW_MS = 5000;
const RECENT_RESOURCE_DETAIL_MAX = 16;
const REPORT_WINDOW_FRAMES = 30;
const REPORT_WINDOW_MS = 1000;
const TIMER_INTERVAL_MS = 100;
const RAF_SPIKE_THRESHOLD_MS = 120;
const TIMER_DRIFT_SPIKE_THRESHOLD_MS = 120;

let started = false;
const rafDeltas: number[] = [];
let lastRafTs: number | null = null;
let rafHandle: number | null = null;
const timerDrifts: number[] = [];
let lastTimerAt: number | null = null;
let timerHandle: number | null = null;
let rafDeltaSpikeCountTotal = 0;
let timerDriftSpikeCountTotal = 0;
let longtaskCountTotal = 0;
let lastRafDeltaSpikeAtMs: number | null = null;
let lastTimerDriftSpikeAtMs: number | null = null;
let lastLongtaskAtMs: number | null = null;

const longTasks: LongTaskBucket[] = [];
let longtaskObserver: PerformanceObserver | null = null;
const resources: ResourceBucket[] = [];
const resourceKeys = new Set<string>();
let resourceObserver: PerformanceObserver | null = null;

function trimRing<T>(arr: T[], max: number): void {
  if (arr.length > max) arr.splice(0, arr.length - max);
}

function flushLongTasks(now: number): { within: LongTaskBucket[]; oldestKeptAtMs: number | null } {
  const cutoff = now - LONGTASK_WINDOW_MS;
  let oldestKeptAtMs: number | null = null;
  const within: LongTaskBucket[] = [];
  for (let i = 0; i < longTasks.length; i += 1) {
    const t = longTasks[i];
    if (t.atMs >= cutoff) {
      within.push(t);
      if (oldestKeptAtMs === null || t.atMs < oldestKeptAtMs) oldestKeptAtMs = t.atMs;
    }
  }
  return { within, oldestKeptAtMs };
}

function bucketize(within: LongTaskBucket[]): {
  lt50: number;
  b50to100: number;
  b100to200: number;
  ge200: number;
} {
  // entries are by spec duration >= 50ms; the <50 bucket counts the cheapest
  // bucket only for forward-compatibility with smaller entries injected by
  // synthesised longtask observers (e.g. some browsers fire sub-50 task entries
  // when the entry type is `task`); current Chromium only yields >=50.
  const out = { lt50: 0, b50to100: 0, b100to200: 0, ge200: 0 };
  for (const t of within) {
    if (t.duration < 50) out.lt50 += 1;
    else if (t.duration < 100) out.b50to100 += 1;
    else if (t.duration < 200) out.b100to200 += 1;
    else out.ge200 += 1;
  }
  return out;
}

function shapeLongTaskAttribution(entry: PerformanceEntry): LongTaskAttribution[] {
  const attributed = entry as PerformanceEntry & {
    attribution?: Array<{
      name?: string;
      entryType?: string;
      containerType?: string;
      containerName?: string;
      containerSrc?: string;
      containerId?: string;
      scriptUrl?: string;
    }>;
  };
  const attribution = Array.isArray(attributed.attribution) ? attributed.attribution : [];
  return attribution.slice(0, 6).map((item) => ({
    name: typeof item.name === "string" ? item.name : null,
    entryType: typeof item.entryType === "string" ? item.entryType : null,
    containerType: typeof item.containerType === "string" ? item.containerType : null,
    containerName: typeof item.containerName === "string" ? item.containerName : null,
    containerSrc: typeof item.containerSrc === "string" ? item.containerSrc : null,
    containerId: typeof item.containerId === "string" ? item.containerId : null,
    scriptUrl: typeof item.scriptUrl === "string" ? item.scriptUrl : null,
  }));
}

function recordResourceEntry(entry: PerformanceResourceTiming): void {
  const responseEnd = Number.isFinite(entry.responseEnd) && entry.responseEnd > 0
    ? entry.responseEnd
    : entry.startTime + entry.duration;
  const key = `${entry.name}:${entry.startTime}:${entry.duration}:${entry.transferSize}`;
  if (resourceKeys.has(key)) return;
  resourceKeys.add(key);
  resources.push({
    atMs: responseEnd,
    startTime: entry.startTime,
    responseEnd,
    duration: entry.duration,
    path: resourcePath(entry.name),
    initiatorType: entry.initiatorType || null,
    transferSize: typeof entry.transferSize === "number" ? entry.transferSize : null,
    encodedBodySize: typeof entry.encodedBodySize === "number" ? entry.encodedBodySize : null,
    decodedBodySize: typeof entry.decodedBodySize === "number" ? entry.decodedBodySize : null,
    deliveryType: typeof (entry as PerformanceResourceTiming & { deliveryType?: string }).deliveryType === "string"
      ? (entry as PerformanceResourceTiming & { deliveryType?: string }).deliveryType!
      : null,
  });
  trimRing(resources, RESOURCE_RING_MAX);
  if (resourceKeys.size > RESOURCE_RING_MAX * 2) {
    resourceKeys.clear();
    for (const resource of resources) {
      resourceKeys.add(`${resource.path}:${resource.startTime}:${resource.duration}:${resource.transferSize ?? ""}`);
    }
  }
}

function resourcePath(name: string): string {
  try {
    const url = new URL(name, window.location.href);
    return `${url.pathname}${url.search}`;
  } catch {
    return name;
  }
}

function computeFps(now: number): number | null {
  if (rafDeltas.length === 0) return null;
  let sum = 0;
  let count = 0;
  let windowStart = now - REPORT_WINDOW_MS;
  for (let i = rafDeltas.length - 1; i >= 0; i -= 1) {
    const delta = rafDeltas[i];
    if (delta <= 0 || delta > 1000) continue;
    sum += delta;
    count += 1;
    if (count >= REPORT_WINDOW_FRAMES) break;
    if (sum > REPORT_WINDOW_MS && count >= 5) break;
    void windowStart;
  }
  if (count === 0) return null;
  const avg = sum / count;
  if (avg <= 0 || avg > 1000) return null;
  return Math.min(240, Math.round(1000 / avg));
}

function computeRafDeltaAvg(): number | null {
  if (rafDeltas.length === 0) return null;
  let sum = 0;
  let count = 0;
  for (let i = rafDeltas.length - 1; i >= 0 && count < REPORT_WINDOW_FRAMES; i -= 1) {
    const delta = rafDeltas[i];
    if (delta > 0 && delta < 1000) {
      sum += delta;
      count += 1;
    }
  }
  if (count === 0) return null;
  return Math.round(sum / count);
}

function computeRafDeltaMax(): number | null {
  if (rafDeltas.length === 0) return null;
  let max: number | null = null;
  let count = 0;
  for (let i = rafDeltas.length - 1; i >= 0 && count < REPORT_WINDOW_FRAMES; i -= 1) {
    const delta = rafDeltas[i];
    if (delta > 0 && delta < 5000) {
      max = max === null ? delta : Math.max(max, delta);
      count += 1;
    }
  }
  return max === null ? null : Math.round(max);
}

function computeTimerDriftAvg(): number | null {
  if (timerDrifts.length === 0) return null;
  let sum = 0;
  let count = 0;
  for (let i = timerDrifts.length - 1; i >= 0 && count < REPORT_WINDOW_FRAMES; i -= 1) {
    const drift = timerDrifts[i];
    if (drift >= 0 && drift < 5000) {
      sum += drift;
      count += 1;
    }
  }
  return count === 0 ? null : Math.round(sum / count);
}

function computeTimerDriftMax(): number | null {
  if (timerDrifts.length === 0) return null;
  let max: number | null = null;
  let count = 0;
  for (let i = timerDrifts.length - 1; i >= 0 && count < REPORT_WINDOW_FRAMES; i -= 1) {
    const drift = timerDrifts[i];
    if (drift >= 0 && drift < 5000) {
      max = max === null ? drift : Math.max(max, drift);
      count += 1;
    }
  }
  return max === null ? null : Math.round(max);
}

function rafLoop(ts: number): void {
  if (lastRafTs !== null) {
    const delta = ts - lastRafTs;
    if (delta >= 0 && delta < 5000) {
      rafDeltas.push(delta);
      if (delta >= RAF_SPIKE_THRESHOLD_MS) {
        rafDeltaSpikeCountTotal += 1;
        lastRafDeltaSpikeAtMs = performance.now();
      }
      trimRing(rafDeltas, RING_MAX);
    }
  }
  lastRafTs = ts;
  rafHandle = requestAnimationFrame(rafLoop);
}

function timerLoop(): void {
  const now = performance.now();
  if (lastTimerAt !== null) {
    const drift = Math.max(0, now - lastTimerAt - TIMER_INTERVAL_MS);
    timerDrifts.push(drift);
    if (drift >= TIMER_DRIFT_SPIKE_THRESHOLD_MS) {
      timerDriftSpikeCountTotal += 1;
      lastTimerDriftSpikeAtMs = now;
    }
    trimRing(timerDrifts, RING_MAX);
  }
  lastTimerAt = now;
}

export function startFrameProbe(): void {
  if (started) return;
  started = true;
  lastRafTs = null;
  rafDeltas.length = 0;
  timerDrifts.length = 0;
  lastTimerAt = null;
  longTasks.length = 0;
  resources.length = 0;
  resourceKeys.clear();
  rafDeltaSpikeCountTotal = 0;
  timerDriftSpikeCountTotal = 0;
  longtaskCountTotal = 0;
  lastRafDeltaSpikeAtMs = null;
  lastTimerDriftSpikeAtMs = null;
  lastLongtaskAtMs = null;
  if (typeof PerformanceObserver !== "undefined") {
    try {
      const obs = new PerformanceObserver((list) => {
        const entries = list.getEntries();
        for (let i = 0; i < entries.length; i += 1) {
          const e = entries[i];
          if (e.entryType !== "longtask") continue;
          longTasks.push({
            atMs: e.startTime ?? performance.now(),
            duration: e.duration,
            name: e.name || null,
            attribution: shapeLongTaskAttribution(e),
          });
          longtaskCountTotal += 1;
          lastLongtaskAtMs = e.startTime ?? performance.now();
          trimRing(longTasks, LONGTASK_RING_MAX);
        }
      });
      try {
        obs.observe({ type: "longtask", buffered: true } as PerformanceObserverInit);
      } catch {
        obs.observe({ entryTypes: ["longtask"] });
      }
      longtaskObserver = obs;
    } catch {
      // longtask not supported on FF/Safari — leave longTasks empty. Caller
      // still gets rAF/timer drift timing which is the bulk of the value.
      longtaskObserver = null;
    }
    try {
      const obs = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          if (entry.entryType === "resource") {
            recordResourceEntry(entry as PerformanceResourceTiming);
          }
        }
      });
      obs.observe({ type: "resource", buffered: true } as PerformanceObserverInit);
      resourceObserver = obs;
    } catch {
      resourceObserver = null;
    }
  }
  if (typeof requestAnimationFrame === "function") {
    rafHandle = requestAnimationFrame(rafLoop);
  }
  if (typeof window !== "undefined" && typeof window.setInterval === "function") {
    lastTimerAt = performance.now();
    timerHandle = window.setInterval(timerLoop, TIMER_INTERVAL_MS);
  }
}

export function stopFrameProbe(): void {
  if (!started) return;
  started = false;
  if (rafHandle !== null && typeof cancelAnimationFrame === "function") {
    cancelAnimationFrame(rafHandle);
    rafHandle = null;
  }
  if (longtaskObserver) {
    try {
      longtaskObserver.disconnect();
    } catch {
      /* ignore */
    }
    longtaskObserver = null;
  }
  if (resourceObserver) {
    try {
      resourceObserver.disconnect();
    } catch {
      /* ignore */
    }
    resourceObserver = null;
  }
  if (timerHandle !== null && typeof window !== "undefined") {
    window.clearInterval(timerHandle);
    timerHandle = null;
  }
  rafDeltas.length = 0;
  timerDrifts.length = 0;
  longTasks.length = 0;
  resources.length = 0;
  resourceKeys.clear();
  lastRafTs = null;
  lastTimerAt = null;
  rafDeltaSpikeCountTotal = 0;
  timerDriftSpikeCountTotal = 0;
  longtaskCountTotal = 0;
  lastRafDeltaSpikeAtMs = null;
  lastTimerDriftSpikeAtMs = null;
  lastLongtaskAtMs = null;
}

export function isFrameProbeRunning(): boolean {
  return started;
}

export function readFrameProbe(): {
  rafDeltaAvg: number | null;
  rafDeltaMax: number | null;
  fpsInLastSecond: number | null;
  timerDriftAvg: number | null;
  timerDriftMax: number | null;
  longtasksInLastSecond: number | null;
  longestLongtaskMs: number | null;
  longtaskCountTotal: number;
  rafDeltaSpikeCountTotal: number;
  timerDriftSpikeCountTotal: number;
  lastLongtaskAtMs: number | null;
  lastRafDeltaSpikeAtMs: number | null;
  lastTimerDriftSpikeAtMs: number | null;
  recentLongtasks: LongTaskBucket[];
  recentResources: ResourceBucket[];
  longtaskBuckets: {
    lt50: number;
    b50to100: number;
    b100to200: number;
    ge200: number;
  } | null;
} {
  const now = performance.now();
  const { within } = flushLongTasks(now);
  const buckets = bucketize(within);
  const longest = within.length > 0
    ? within.reduce((m, t) => (t.duration > m ? t.duration : m), 0)
    : 0;
  const recentResources = resources
    .filter((resource) => resource.atMs >= now - RECENT_RESOURCE_WINDOW_MS)
    .slice(-RECENT_RESOURCE_DETAIL_MAX);
  return {
    rafDeltaAvg: computeRafDeltaAvg(),
    rafDeltaMax: computeRafDeltaMax(),
    fpsInLastSecond: computeFps(now),
    timerDriftAvg: computeTimerDriftAvg(),
    timerDriftMax: computeTimerDriftMax(),
    longtasksInLastSecond: within.length,
    longestLongtaskMs: longest > 0 ? Math.round(longest) : null,
    longtaskCountTotal,
    rafDeltaSpikeCountTotal,
    timerDriftSpikeCountTotal,
    lastLongtaskAtMs,
    lastRafDeltaSpikeAtMs,
    lastTimerDriftSpikeAtMs,
    recentLongtasks: longTasks.slice(-LONGTASK_DETAIL_MAX),
    recentResources,
    longtaskBuckets: buckets,
  };
}
