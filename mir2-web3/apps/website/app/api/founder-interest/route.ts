const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const ALLOWED_PLANS = new Set(["free", "token", "architect"]);

export async function POST(request: Request) {
  const contentLength = Number(request.headers.get("content-length") ?? "0");
  if (contentLength > 32_000) return Response.json({ error: "payload_too_large" }, { status: 413 });

  const body = await request.json().catch(() => null) as null | Record<string, unknown>;
  if (!body) return Response.json({ error: "invalid_json" }, { status: 400 });
  if (typeof body.company === "string" && body.company.trim()) return Response.json({ ok: true, mode: "preview" }, { status: 202 });

  const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
  const plan = typeof body.plan === "string" ? body.plan : "";
  if (!EMAIL_PATTERN.test(email) || email.length > 254 || !ALLOWED_PLANS.has(plan) || body.consent !== true) {
    return Response.json({ error: "invalid_request" }, { status: 400 });
  }

  const lead = {
    email,
    plan,
    region: typeof body.region === "string" ? body.region.slice(0, 32) : "unknown",
    locale: typeof body.locale === "string" ? body.locale.slice(0, 16) : "unknown",
    attribution: body.attribution ?? null,
    createdAt: new Date().toISOString(),
  };
  const webhook = process.env.FOUNDER_LEAD_WEBHOOK_URL;
  if (!webhook) return Response.json({ ok: true, mode: "preview" }, { status: 202 });

  const response = await fetch(webhook, {
    method: "POST",
    headers: { "content-type": "application/json", ...(process.env.FOUNDER_LEAD_WEBHOOK_TOKEN ? { authorization: `Bearer ${process.env.FOUNDER_LEAD_WEBHOOK_TOKEN}` } : {}) },
    body: JSON.stringify(lead),
    signal: AbortSignal.timeout(5000),
  }).catch(() => null);
  if (!response?.ok) return Response.json({ error: "lead_delivery_failed" }, { status: 502 });
  return Response.json({ ok: true, mode: "stored" }, { status: 201 });
}
