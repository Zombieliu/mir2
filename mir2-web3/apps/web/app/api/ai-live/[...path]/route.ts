import type { NextRequest } from "next/server";

export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const ALLOWED_PATHS = new Set(["status", "metrics", "control", "distribution"]);

function gatewayBase() {
  const direct = process.env.MIR2_GATEWAY_HTTP_URL?.trim();
  if (direct) return direct.replace(/\/+$/, "");
  const websocket = process.env.NEXT_PUBLIC_MIR2_GATEWAY_WS_URL?.trim();
  if (websocket) {
    const url = new URL(websocket);
    url.protocol = url.protocol === "wss:" ? "https:" : "http:";
    url.pathname = "";
    url.search = "";
    url.hash = "";
    return url.toString().replace(/\/+$/, "");
  }
  return process.env.NODE_ENV === "development" ? "http://127.0.0.1:7110" : null;
}

async function proxy(request: NextRequest, context: { params: Promise<{ path: string[] }> }) {
  const base = gatewayBase();
  if (!base) {
    return Response.json(
      { error: "AI live Gateway proxy is not configured" },
      { status: 503 },
    );
  }
  const { path } = await context.params;
  if (path.length !== 1 || !ALLOWED_PATHS.has(path[0])) {
    return Response.json({ error: "unsupported AI live route" }, { status: 404 });
  }
  if (
    request.method === "POST"
    && path[0] !== "control"
    && path[0] !== "distribution"
  ) {
    return Response.json({ error: "method not allowed" }, { status: 405 });
  }
  if (request.method === "GET" && path[0] === "control") {
    return Response.json({ error: "method not allowed" }, { status: 405 });
  }

  const headers = new Headers({ accept: "application/json" });
  const authorization = request.headers.get("authorization");
  if (authorization) headers.set("authorization", authorization);
  let body: string | undefined;
  if (request.method === "POST") {
    headers.set("content-type", "application/json");
    body = await request.text();
  }
  try {
    const upstream = await fetch(`${base}/ai-live/${path[0]}`, {
      method: request.method,
      headers,
      body,
      cache: "no-store",
      signal: AbortSignal.timeout(8_000),
    });
    return new Response(upstream.body, {
      status: upstream.status,
      headers: {
        "content-type": upstream.headers.get("content-type") ?? "application/json",
        "cache-control": "no-store",
      },
    });
  } catch {
    return Response.json({ error: "AI live Gateway is unavailable" }, { status: 502 });
  }
}

export async function GET(
  request: NextRequest,
  context: { params: Promise<{ path: string[] }> },
) {
  return proxy(request, context);
}

export async function POST(
  request: NextRequest,
  context: { params: Promise<{ path: string[] }> },
) {
  return proxy(request, context);
}
