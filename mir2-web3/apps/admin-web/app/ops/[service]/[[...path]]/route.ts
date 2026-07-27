import { NextRequest, NextResponse } from "next/server";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

type RouteContext = {
  params: Promise<{
    service: string;
    path?: string[];
  }>;
};

const services = {
  grafana: {
    originVariable: "DUBHE_GRAFANA_ORIGIN_URL",
    defaultOrigin: "https://relay-hk.obelisk.build/home/ops/grafana"
  },
  prometheus: {
    originVariable: "DUBHE_PROMETHEUS_ORIGIN_URL",
    defaultOrigin: "https://relay-hk.obelisk.build/home/ops/prometheus"
  }
} as const;

async function proxyObservability(request: NextRequest, context: RouteContext) {
  const { path = [], service } = await context.params;
  if (!(service in services)) {
    return NextResponse.json({ error: "observability service not found" }, { status: 404 });
  }
  const serviceConfig = services[service as keyof typeof services];
  const proxyToken = process.env.DUBHE_OBSERVABILITY_PROXY_TOKEN?.trim();
  if (!proxyToken) {
    return NextResponse.json(
      { error: "observability proxy is not configured" },
      { status: 503 }
    );
  }

  const configuredOrigin =
    process.env[serviceConfig.originVariable]?.trim() ?? serviceConfig.defaultOrigin;
  const origin = new URL(configuredOrigin.replace(/\/+$/, "") + "/");
  origin.pathname = `${origin.pathname}${path.map(encodeURIComponent).join("/")}`;
  origin.search = request.nextUrl.search;

  const headers = new Headers(request.headers);
  for (const name of [
    "authorization",
    "connection",
    "cookie",
    "host",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-proto"
  ]) {
    headers.delete(name);
  }
  headers.set("accept-encoding", "identity");
  headers.set("x-dubhe-observability-token", proxyToken);
  headers.set("x-forwarded-host", request.nextUrl.host);
  headers.set("x-forwarded-proto", "https");

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15_000);
  try {
    const upstream = await fetch(origin, {
      method: request.method,
      headers,
      body:
        request.method === "GET" || request.method === "HEAD"
          ? undefined
          : await request.arrayBuffer(),
      cache: "no-store",
      redirect: "manual",
      signal: controller.signal
    });
    const responseHeaders = new Headers(upstream.headers);
    for (const name of [
      "connection",
      "content-encoding",
      "content-length",
      "set-cookie",
      "transfer-encoding"
    ]) {
      responseHeaders.delete(name);
    }
    responseHeaders.set("cache-control", "private, no-store, max-age=0");
    responseHeaders.set("x-content-type-options", "nosniff");
    return new NextResponse(request.method === "HEAD" ? null : upstream.body, {
      status: upstream.status,
      statusText: upstream.statusText,
      headers: responseHeaders
    });
  } catch (error) {
    const timedOut = error instanceof Error && error.name === "AbortError";
    return NextResponse.json(
      {
        error: timedOut
          ? "observability upstream timed out"
          : "observability upstream unavailable"
      },
      { status: timedOut ? 504 : 502 }
    );
  } finally {
    clearTimeout(timeout);
  }
}

export const GET = proxyObservability;
export const HEAD = proxyObservability;
export const POST = proxyObservability;
