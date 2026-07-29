export const dynamic = "force-dynamic";
export const runtime = "nodejs";

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

export async function POST(request: Request) {
  const base = gatewayBase();
  if (!base) {
    return Response.json(
      { error: "channel session Gateway proxy is not configured" },
      { status: 503 },
    );
  }

  const body = await request.text();
  try {
    const upstream = await fetch(`${base}/v1/channels/session/exchange`, {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/json",
      },
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
    return Response.json({ error: "channel session Gateway is unavailable" }, { status: 502 });
  }
}
