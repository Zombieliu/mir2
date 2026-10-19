import { NextResponse } from "next/server";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";
const MAX_RESPONSE_BYTES = 16_384;
const responseHeaders = { "content-type": "application/json; charset=utf-8", "cache-control": "no-store, private",
  "x-content-type-options": "nosniff" };
function unavailable(status = 503): NextResponse {
  return NextResponse.json({ error: "Wallet service is unavailable." }, { status, headers: responseHeaders });
}
async function boundedText(response: Response): Promise<string> {
  if (!response.body) throw Error("Missing wallet response");
  const reader = response.body.getReader(), decoder = new TextDecoder("utf-8", { fatal: true });
  let bytes = 0, text = "";
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) break;
      bytes += chunk.value.byteLength;
      if (bytes > MAX_RESPONSE_BYTES) { await reader.cancel(); throw Error("Wallet response exceeded limit"); }
      text += decoder.decode(chunk.value, { stream: true });
    }
    return text + decoder.decode();
  } finally { reader.releaseLock(); }
}

/** Fixed read-only endpoint; no caller-selected host, path or asset mutation. */
export async function GET(request: Request): Promise<NextResponse> {
  let query: URLSearchParams;
  try { query = new URL(request.url).searchParams; }
  catch { return NextResponse.json({ error: "Invalid wallet request." }, { status: 400, headers: responseHeaders }); }
  const index = query.get("characterIndex");
  if ([...query.keys()].join() !== "characterIndex" || index === null || !/^(?:0|[1-9][0-9]*)$/.test(index)
    || index.length > 10 || BigInt(index) > 2147483647n)
    return NextResponse.json({ error: "Invalid character index." }, { status: 400, headers: responseHeaders });
  const authorization = request.headers.get("authorization");
  if (!authorization || authorization.length > 4096 || !/^Bearer [A-Za-z0-9._~-]+$/.test(authorization))
    return NextResponse.json({ error: "An identity session is required." }, { status: 401, headers: responseHeaders });
  const configured = process.env.MIR2_GATEWAY_HTTP_URL?.trim().replace(/\/+$/u, "");
  if (!configured) return unavailable();
  let base: URL;
  try {
    base = new URL(configured);
    if (!["http:", "https:"].includes(base.protocol) || base.username || base.password || base.search || base.hash)
      return unavailable();
  } catch { return unavailable(); }
  try {
    const upstream = await fetch(`${base.href.replace(/\/+$/u, "")}/v1/economy/hunting-wallet?characterIndex=${index}`, {
      method: "GET", headers: { accept: "application/json", authorization }, cache: "no-store", redirect: "error",
      signal: AbortSignal.timeout(8_000),
    });
    if (upstream.status !== 200 && (upstream.status < 400 || upstream.status > 599)) return unavailable(502);
    const payload = await boundedText(upstream);
    return new NextResponse(payload, { status: upstream.status, headers: responseHeaders });
  } catch { return unavailable(); }
}
