/**
 * mir2-probe — Gateway metrics poller (L5).
 *
 * Pulls `GET /metrics` from the running gateway and caches the latest result
 * so probe.snapshot() can surface gateway tick/save timing in the dev overlay
 * without blocking on a network round-trip. The poller is dev-only by
 * convention: it is started when the probe SDK is started (i.e. `?probe=1`),
 * runs at 2 Hz, and tolerates the gateway being unreachable (next poll keeps
 * the last good value with a staleAtMs marker).
 *
 * URL derivation mirrors resolveGatewayWebSocketUrl in page.tsx but emits an
 * http(s) URL instead of ws(s). Localhost `?gatewayWs=ws://host:port/ws`
 * overrides are honored as `http://host:port/metrics` only on localhost for
 * the same security reason as the WS override.
 */

import type { ProbeGatewayLayer } from "./schema";

type GatewayMetricsResponse = {
  startedAtMs: number;
  sampledAtMs: number;
  tick: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
  sharedTick?: {
    count: number;
    pending?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
    expire?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
    command?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
    autoPickup?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
    observer?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
    observerDetail?: {
      npc?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
      apply?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
      shared?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
      drops?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
      quest?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
      zoneObserver?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
    };
    zone?: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
  };
  save: { count: number; msTotal: number; msLast: number; msMax: number; msAvg: number };
  worldSnapshotCount: number;
};

const POLL_INTERVAL_MS = 500;
const FETCH_TIMEOUT_MS = 1500;

let pollerHandle: number | null = null;
let lastMetrics: GatewayMetricsResponse | null = null;
let lastMetricsAtMs: number | null = null;
let lastFetchMs: number | null = null;
let lastError: string | null = null;
let baseUrl: string | null = null;

function isLocalHostname(hostname: string): boolean {
  return (
    hostname === "localhost" ||
    hostname === "127.0.0.1" ||
    hostname === "0.0.0.0" ||
    hostname.endsWith(".localhost") ||
    hostname === "[::1]"
  );
}

function resolveGatewayHttpBaseUrl(): string | null {
  if (typeof window === "undefined") return null;
  const configured = process.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL?.trim();
  const local = "http://127.0.0.1:7110";
  const hosted = "https://165.154.65.136.sslip.io";
  const onLocalHost = isLocalHostname(window.location.hostname);

  // localhost ?gatewayWs override; same security gate as the WS override.
  if (onLocalHost) {
    const q = new URLSearchParams(window.location.search).get("gatewayWs");
    if (q && /^wss?:\/\//.test(q)) {
      return q.replace(/^ws/, "http").replace(/\/ws$/, "");
    }
  }

  if (configured) {
    return configured.replace(/^wss?/, (m) => (m === "wss" ? "https" : "http")).replace(/\/ws$/, "");
  }
  return onLocalHost ? local : hosted;
}

async function fetchMetricsOnce(): Promise<void> {
  const url = baseUrl ? `${baseUrl}/metrics` : null;
  if (!url) return;
  const fetchStart = typeof performance !== "undefined" ? performance.now() : Date.now();
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
    const res = await fetch(url, {
      method: "GET",
      cache: "no-store",
      signal: controller.signal,
      headers: { accept: "application/json" },
    });
    clearTimeout(timeout);
    if (!res.ok) {
      lastError = `http ${res.status}`;
      return;
    }
    const json = (await res.json()) as GatewayMetricsResponse;
    if (!json || typeof json !== "object") {
      lastError = "bad json";
      return;
    }
    lastMetrics = json;
    lastMetricsAtMs = Date.now();
    lastFetchMs = Math.round(((typeof performance !== "undefined" ? performance.now() : Date.now())) - fetchStart);
    lastError = null;
  } catch (e) {
    lastError = e instanceof Error ? e.message : String(e);
  }
}

function tick(): void {
  void fetchMetricsOnce();
}

export function startGatewayProbe(): void {
  if (pollerHandle !== null) return;
  baseUrl = resolveGatewayHttpBaseUrl();
  if (!baseUrl) return;
  // First fetch immediately so a snapshot() taken right after start() can
  // already see gateway data.
  void fetchMetricsOnce();
  if (typeof window !== "undefined" && typeof window.setInterval === "function") {
    pollerHandle = window.setInterval(tick, POLL_INTERVAL_MS);
  }
}

export function stopGatewayProbe(): void {
  if (pollerHandle !== null && typeof window !== "undefined" && typeof window.clearInterval === "function") {
    window.clearInterval(pollerHandle);
  }
  pollerHandle = null;
  baseUrl = null;
  // Do NOT clear lastMetrics — late snapshot() calls may still want the last
  // known gateway state. Caller can stop() and call readGatewayLayer() to see
  // final values; restart re-inits via fetchMetricsOnce().
}

export function isGatewayProbeRunning(): boolean {
  return pollerHandle !== null;
}

export function readGatewayLayer(): ProbeGatewayLayer {
  return {
    health: null,
    metrics: lastMetrics as unknown as Record<string, unknown> | null,
    healthFetchedAtMs: lastMetricsAtMs,
    healthFetchMs: lastFetchMs,
  };
}

export function getGatewayProbeDebug(): {
  baseUrl: string | null;
  polling: boolean;
  lastMetricsAtMs: number | null;
  lastFetchMs: number | null;
  lastError: string | null;
} {
  return {
    baseUrl,
    polling: pollerHandle !== null,
    lastMetricsAtMs,
    lastFetchMs,
    lastError,
  };
}
