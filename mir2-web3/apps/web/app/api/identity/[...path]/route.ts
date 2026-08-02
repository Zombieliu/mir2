import { NextResponse } from "next/server";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

const ALLOWED_PATHS = new Set([
  "me",
  "sessions/revoke",
  "sessions/revoke-others",
  "recovery-codes/rotate",
  "credentials/revoke",
  "credentials/bind-sui",
  "recover",
]);

type RouteContext = { params: Promise<{ path: string[] }> };

export async function GET(request: Request, context: RouteContext) {
  return proxyIdentityRequest(request, context, "GET");
}

export async function POST(request: Request, context: RouteContext) {
  return proxyIdentityRequest(request, context, "POST");
}

async function proxyIdentityRequest(request: Request, context: RouteContext, method: "GET" | "POST") {
  const origin = request.headers.get("origin");
  if (method === "POST" && !isSameOriginBrowserRequest(request, origin)) {
    return NextResponse.json({ error: "cross-origin identity request rejected" }, { status: 403 });
  }
  const { path } = await context.params;
  const relative = path.join("/");
  if (!ALLOWED_PATHS.has(relative)) {
    return NextResponse.json({ error: "identity endpoint not found" }, { status: 404 });
  }
  const base = process.env.MIR2_GATEWAY_HTTP_URL?.trim().replace(/\/+$/u, "");
  if (!base) {
    return NextResponse.json({ error: "identity service is not configured" }, { status: 503 });
  }
  const authorization = request.headers.get("authorization");
  if (relative !== "recover" && (!authorization || authorization.length > 4096)) {
    return NextResponse.json({ error: "identity session is required" }, { status: 401 });
  }
  const headers = new Headers({ accept: "application/json" });
  if (authorization) headers.set("authorization", authorization);
  let body: string | undefined;
  if (method === "POST") {
    body = await request.text();
    if (body.length > 16_384) {
      return NextResponse.json({ error: "identity request is too large" }, { status: 413 });
    }
    headers.set("content-type", "application/json");
  }
  try {
    const response = await fetch(`${base}/v1/identity/${relative}`, {
      method,
      headers,
      body,
      cache: "no-store",
      signal: AbortSignal.timeout(8_000),
    });
    const payload = await response.text();
    return new NextResponse(payload, {
      status: response.status,
      headers: {
        "content-type": response.headers.get("content-type") ?? "application/json",
        "cache-control": "no-store",
      },
    });
  } catch {
    return NextResponse.json({ error: "identity service is unavailable" }, { status: 503 });
  }
}

function isSameOriginBrowserRequest(request: Request, origin: string | null): boolean {
  if (!origin) {
    return request.headers.get("sec-fetch-site") === "same-origin";
  }
  let originUrl: URL;
  try {
    originUrl = new URL(origin);
  } catch {
    return false;
  }
  const requestUrl = new URL(request.url);
  const forwardedHost = request.headers.get("x-forwarded-host")?.trim();
  const host = request.headers.get("host")?.trim();
  const expectedHost = forwardedHost || host || requestUrl.host;
  if (originUrl.host !== expectedHost) return false;

  const forwardedProtocol = request.headers.get("x-forwarded-proto")?.trim().replace(/:$/u, "");
  const expectedProtocol = forwardedProtocol ? `${forwardedProtocol}:` : requestUrl.protocol;
  return originUrl.protocol === expectedProtocol;
}
