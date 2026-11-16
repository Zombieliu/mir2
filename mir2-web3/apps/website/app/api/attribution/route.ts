export async function POST(request: Request) {
  const contentLength = Number(request.headers.get("content-length") ?? "0");
  if (contentLength > 24_000) return new Response(null, { status: 413 });
  const webhook = process.env.ATTRIBUTION_WEBHOOK_URL;
  if (!webhook) return new Response(null, { status: 204 });
  const payload = await request.text();
  const response = await fetch(webhook, {
    method: "POST",
    headers: { "content-type": "application/json", ...(process.env.ATTRIBUTION_WEBHOOK_TOKEN ? { authorization: `Bearer ${process.env.ATTRIBUTION_WEBHOOK_TOKEN}` } : {}) },
    body: payload,
    signal: AbortSignal.timeout(3000),
  }).catch(() => null);
  return new Response(null, { status: response?.ok ? 204 : 502 });
}
