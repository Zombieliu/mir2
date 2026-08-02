import type { NextRequest } from "next/server";

// Same-origin R2 asset proxy for local development and standalone thin clients
// (see next.config.ts `rewrites()`). next.config rewrites missing R2-backed paths
// (/original-map, /original-ui, /generated, /bevy-entity-atlases, /Sound) the
// local checkout doesn't have INTO this handler, which does its own server-side
// fetch to R2. The point of a Route Handler rather than a direct rewrite: a
// direct rewrite forwards the browser's `Referer: http://localhost:...`, and R2's
// hotlink protection 403s any request carrying a non-allowed Referer. A Node
// server-side `fetch` sends NO Referer, so R2 returns 200. The browser is then
// same-origin with every asset → no CORS, no 404/403 retry storm, getImageData
// works. The fallback route is inert unless MIR2_R2_PROXY_BASE is configured.

const PROXY_BASE = process.env.MIR2_R2_PROXY_BASE?.replace(/\/+$/, "");

export async function GET(_request: NextRequest, context: { params: Promise<{ path: string[] }> }) {
  if (!PROXY_BASE) {
    return new Response("r2-proxy disabled (set MIR2_R2_PROXY_BASE)", { status: 404 });
  }
  const { path } = await context.params;
  if (!Array.isArray(path) || path.length === 0) {
    return new Response("bad asset path", { status: 400 });
  }

  const target = `${PROXY_BASE}/${path.map((segment) => encodeURIComponent(segment)).join("/")}`;

  let upstream: Response;
  try {
    // No Referer/Origin is sent by a server-side fetch — that is the whole point.
    upstream = await fetch(target, { headers: { Accept: "*/*" }, cache: "no-store" });
  } catch {
    return new Response(null, { status: 502 });
  }

  if (!upstream.ok || !upstream.body) {
    return new Response(null, { status: upstream.status || 502 });
  }

  const headers = new Headers();
  const contentType = upstream.headers.get("content-type");
  if (contentType) {
    headers.set("content-type", contentType);
  }
  const contentLength = upstream.headers.get("content-length");
  if (contentLength) {
    headers.set("content-length", contentLength);
  }
  // Immutable game media — let the browser/SW cache it so the proxy is hit once
  // per asset, not per render.
  headers.set("cache-control", "public, max-age=31536000, immutable");

  return new Response(upstream.body, { status: 200, headers });
}
