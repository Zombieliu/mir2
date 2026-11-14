import test from "node:test";
import assert from "node:assert/strict";
import http from "node:http";
import net from "node:net";
import { once } from "node:events";
import { createPromotionPlaytestProxy, validateProxyConfig, promotionPathRule, MAX_BODY_BYTES } from "./promotion-playtest-proxy.mjs";

const ORIGIN = "https://promotion-playtest.example";
async function listen(server) { server.listen(0, "127.0.0.1"); await once(server, "listening"); return server.address().port; }
async function fixture(t, respond) {
  const received = [];
  const upstream = http.createServer(async (req, res) => {
    const chunks = []; for await (const c of req) chunks.push(c);
    const record = { method: req.method, url: req.url, headers: req.headers, body: Buffer.concat(chunks).toString() };
    received.push(record);
    if (respond) respond(req, res); else { res.writeHead(200, { "content-type": "application/json" }); res.end(JSON.stringify(record)); }
  });
  const upstreamPort = await listen(upstream);
  const proxy = createPromotionPlaytestProxy({ publicOrigin: ORIGIN, upstream: `http://127.0.0.1:${upstreamPort}` });
  const port = await listen(proxy);
  t.after(async () => { proxy.closeAllConnections(); upstream.closeAllConnections(); await Promise.all([new Promise(r => proxy.close(r)), new Promise(r => upstream.close(r))]); });
  function request(path, { method = "GET", headers = {}, body, chunks } = {}) {
    return new Promise((resolve, reject) => {
      const req = http.request({ hostname: "127.0.0.1", port, path, method, headers: { host: "promotion-playtest.example", ...headers }, agent: false }, res => {
        const bytes = []; res.on("data", c => bytes.push(c)); res.on("end", () => resolve({ status: res.statusCode, headers: res.headers, body: Buffer.concat(bytes).toString() }));
      });
      req.on("error", reject);
      if (chunks) for (const c of chunks) req.write(c);
      req.end(body);
    });
  }
  return { request, received, port };
}

test("configuration pins one public HTTPS origin and a loopback HTTP upstream", () => {
  assert.equal(validateProxyConfig({ publicOrigin: ORIGIN, upstream: "http://localhost:7359" }).upstream, "http://127.0.0.1:7359");
  for (const publicOrigin of ["http://example.com", "https://user:secret@example.com", "https://example.com/path", "https://example.com?x=1", "https://localhost", "https://127.0.0.1", "https://example.com:8443"]) assert.throws(() => validateProxyConfig({ publicOrigin }));
  for (const upstream of ["http://example.com", "https://127.0.0.1", "http://127.0.0.1/admin", "http://127.0.0.1?url=bad", "http://user:secret@127.0.0.1"]) assert.throws(() => validateProxyConfig({ publicOrigin: ORIGIN, upstream }));
  for (const port of [0, -1, 65536, 1.5]) assert.throws(() => validateProxyConfig({ publicOrigin: ORIGIN, port }));
});

test("only promotion pages, required client assets and explicit BFF endpoints are allowed", () => {
  for (const path of ["/promotions", "/promotions/", "/promotions/invite/NUMERON_123-a", "/api/promotions/bootstrap", "/api/promotions/auth/challenge", "/api/promotions/payout-address", "/_next/static/chunks/%5Bturbopack%5D_a.js", "/_next/static/css/a.css", "/_next/static/media/font.woff2", "/favicon.svg", "/pwa/apple-touch-icon.png", "/original-ui/Cursors/Cursor_Default.CUR"]) assert.ok(promotionPathRule(path), path);
  for (const path of ["/play", "/api/game", "/api/identity/me", "/api/promotions/admin", "/api/promotions/admin/", "/promotions/admin", "/api/promotions/source-events", "/api/promotions/payout-worker", "/api/promotions/auth/new", "/.env", "/src/main.ts", "/_next/static/a.js.map", "/_next/webpack-hmr", "/__nextjs_original-stack-frame", "/original-map/secret.bin", "/asset-cache-sw.js"]) assert.equal(promotionPathRule(path), null, path);
});

test("encoded separators, double encoding, hidden files, traversal and absolute targets fail closed", () => {
  for (const path of ["https://promotion-playtest.example/promotions", "//evil.example/promotions", "/promotions\\admin", "/promotions/../admin", "/promotions/%2e%2e/admin", "/api/promotions/%61dmin", "/api/promotions%2fpublic", "/api/promotions/%252e%252e/public", "/_next/static/.env.js", "/_next/static/%00a.js", "/promotions#x", "/promotions%zz", "/" + "a".repeat(8192)]) assert.equal(promotionPathRule(path), null, path);
});

test("root redirects locally and robots/manifest contain only the promotion scope", async t => {
  const f = await fixture(t);
  const root = await f.request("/"); assert.equal(root.status, 302); assert.equal(root.headers.location, "/promotions");
  assert.match((await f.request("/robots.txt")).body, /Disallow: \//);
  const manifest = JSON.parse((await f.request("/manifest.webmanifest")).body); assert.equal(manifest.start_url, "/promotions"); assert.equal(manifest.scope, "/promotions");
  assert.equal(f.received.length, 0);
});

test("Host is fixed and spoofed forwarding headers cannot change the boundary", async t => {
  const f = await fixture(t);
  assert.equal((await f.request("/promotions", { headers: { host: "evil.example", "x-forwarded-host": "promotion-playtest.example" } })).status, 421);
  assert.equal((await f.request("/promotions", { headers: { host: "127.0.0.1:7374" } })).status, 421);
  assert.equal((await f.request("/promotions", { headers: { origin: "https://evil.example", "x-forwarded-proto": "https" } })).status, 403);
  assert.equal(f.received.length, 0);
  assert.equal((await f.request("/promotions", { headers: { host: "promotion-playtest.example:443" } })).status, 200);
});

test("external Origin, cookie, CSRF and idempotency headers are forwarded verbatim without identity forgery", async t => {
  const f = await fixture(t);
  const r = await f.request("/api/promotions/apply", { method: "POST", headers: { origin: ORIGIN, cookie: "promotion_session=opaque; click=opaque", "content-type": "application/json", "x-promotion-csrf": "csrf", "idempotency-key": "operation-1", "x-forwarded-for": "1.2.3.4", "x-forwarded-host": "evil.example", forwarded: "host=evil", "cf-connecting-ip": "1.2.3.4", "x-account-id": "victim", "x-role": "finance" }, body: '{"termsVersion":"test"}' });
  assert.equal(r.status, 200); const seen = f.received[0];
  assert.equal(seen.headers.origin, ORIGIN); assert.equal(seen.headers.cookie, "promotion_session=opaque; click=opaque"); assert.equal(seen.headers["x-promotion-csrf"], "csrf"); assert.equal(seen.headers["idempotency-key"], "operation-1"); assert.equal(seen.body, '{"termsVersion":"test"}');
  for (const key of ["x-forwarded-for", "x-forwarded-host", "forwarded", "cf-connecting-ip", "x-account-id", "x-role"]) assert.equal(seen.headers[key], undefined);
  assert.match(seen.headers.host, /^127\.0\.0\.1:/);
});

test("writes require the real configured Origin and reject browser bearer or server-action headers", async t => {
  const f = await fixture(t);
  for (const headers of [{}, { origin: "null" }, { origin: "https://evil.example" }, { origin: ORIGIN, authorization: "Bearer forbidden" }, { origin: ORIGIN, "next-action": "arbitrary-action" }, { origin: ORIGIN, "sec-fetch-site": "cross-site" }]) assert.equal((await f.request("/api/promotions/auth/challenge", { method: "POST", headers })).status, 403);
  assert.equal(f.received.length, 0);
});

test("GET/HEAD pages and static assets cannot invoke writes; admin remains unreachable", async t => {
  const f = await fixture(t);
  for (const [path, method] of [["/promotions", "POST"], ["/_next/static/a.js", "POST"], ["/api/promotions/auth/challenge", "GET"], ["/api/promotions/public", "PUT"], ["/api/promotions/export", "POST"], ["/api/promotions/me", "OPTIONS"]]) assert.equal((await f.request(path, { method, headers: { origin: ORIGIN } })).status, 405);
  for (const path of ["/api/promotions/admin", "/api/promotions/%61dmin", "/promotions/admin", "/api/game", "/.env"]) assert.equal((await f.request(path)).status, 404);
  assert.equal(f.received.length, 0);
});

test("declared and chunked oversized bodies never reach the upstream", async t => {
  const f = await fixture(t);
  assert.equal((await f.request("/api/promotions/apply", { method: "POST", headers: { origin: ORIGIN, "content-type": "application/json", "content-length": MAX_BODY_BYTES + 1 }, body: "x".repeat(MAX_BODY_BYTES + 1) })).status, 413);
  assert.equal((await f.request("/api/promotions/apply", { method: "POST", headers: { origin: ORIGIN, "content-type": "application/json" }, chunks: ["x".repeat(8192), "x".repeat(8193)] })).status, 413);
  assert.equal(f.received.length, 0);
});

test("request body type, compression and read-body smuggling are rejected", async t => {
  const f = await fixture(t);
  assert.equal((await f.request("/api/promotions/apply", { method: "POST", headers: { origin: ORIGIN, "content-type": "text/plain" }, body: "x" })).status, 415);
  assert.equal((await f.request("/api/promotions/apply", { method: "POST", headers: { origin: ORIGIN, "content-encoding": "gzip" }, body: "x" })).status, 400);
  assert.equal((await f.request("/api/promotions/me", { headers: { "content-length": "1" }, body: "x" })).status, 400);
  assert.equal(f.received.length, 0);
});

test("Set-Cookie stays intact and all responses disable indexing and caching", async t => {
  const cookies = ["promotion_session=opaque; Path=/; HttpOnly; Secure; SameSite=Lax", "promotion_click=opaque; Path=/; HttpOnly; Secure; SameSite=Lax"];
  const f = await fixture(t, (_req, res) => { res.writeHead(200, { "content-type": "text/plain", "set-cookie": cookies, "cache-control": "public,max-age=86400", "access-control-allow-origin": "*", "x-powered-by": "framework" }); res.end("ok"); });
  const r = await f.request("/api/promotions/bootstrap"); assert.deepEqual(r.headers["set-cookie"], cookies); assert.equal(r.headers["cache-control"], "no-store"); assert.match(r.headers["x-robots-tag"], /noindex/); assert.equal(r.headers["access-control-allow-origin"], undefined); assert.equal(r.headers["x-powered-by"], undefined);
  assert.equal((await f.request("/api/game")).headers["cache-control"], "no-store");
});

test("redirects remain inside the promotion allowlist", async t => {
  const f = await fixture(t, (req, res) => { const location = req.url.includes("bad") ? "https://evil.example/game" : `http://${req.headers.host}/promotions/invite/TEST`; res.writeHead(307, { location }); res.end(); });
  const good = await f.request("/promotions"); assert.equal(good.status, 307); assert.equal(good.headers.location, "/promotions/invite/TEST");
  assert.equal((await f.request("/promotions?bad=1")).status, 502);
});

test("WebSocket upgrade and HMR are denied without reaching the upstream", async t => {
  const f = await fixture(t);
  const reply = await new Promise((resolve, reject) => { const s = net.connect(f.port, "127.0.0.1"); let text = ""; s.on("connect", () => s.write("GET /_next/webpack-hmr HTTP/1.1\r\nHost: promotion-playtest.example\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n")); s.on("data", b => text += b); s.on("end", () => resolve(text)); s.on("error", reject); });
  assert.match(reply, /^HTTP\/1\.1 403/); assert.equal(f.received.length, 0);
});

test("required CSS, JS and fonts retain their original paths and query strings", async t => {
  const f = await fixture(t);
  for (const path of ["/_next/static/chunks/%5Bturbopack%5D_a.js?v=1", "/_next/static/css/promotion.css", "/_next/static/media/font.woff2", "/original-ui/Cursors/Cursor_Default.CUR"]) { assert.equal((await f.request(path)).status, 200); assert.equal(f.received.at(-1).url, path); }
});

test("encoded admin/source/game path mutations cannot reach the upstream", async t => {
  const f = await fixture(t);
  const forbidden = ["admin", "source-events", "payout-worker", "auth/../admin", ".env", "../../api/game"];
  const paths = new Set();
  for (const value of forbidden) {
    const encodings = [value, encodeURIComponent(value), [...value].map(c => `%${c.charCodeAt(0).toString(16)}`).join("")];
    for (const encoded of encodings) for (const suffix of ["", "/", "?campaignId=public", "/../public", ";public"]) paths.add(`/api/promotions/${encoded}${suffix}`);
  }
  for (const path of paths) assert.equal((await f.request(path)).status, 404, path);
  assert.equal(f.received.length, 0);
});

test("duplicate Host/Origin and ambiguous HTTP framing cannot bypass the request boundary", async t => {
  const f = await fixture(t);
  const requests = [
    "GET /promotions HTTP/1.1\r\nHost: promotion-playtest.example\r\nHost: evil.example\r\nConnection: close\r\n\r\n",
    `POST /api/promotions/auth/challenge HTTP/1.1\r\nHost: promotion-playtest.example\r\nOrigin: ${ORIGIN}\r\nOrigin: https://evil.example\r\nContent-Length: 0\r\nConnection: close\r\n\r\n`,
    `POST /api/promotions/apply HTTP/1.1\r\nHost: promotion-playtest.example\r\nOrigin: ${ORIGIN}\r\nContent-Length: 0\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n0\r\n\r\n`,
    "GET http://evil.example/api/game HTTP/1.1\r\nHost: promotion-playtest.example\r\nConnection: close\r\n\r\n",
  ];
  for (const request of requests) {
    const reply = await new Promise((resolve, reject) => { const socket = net.connect(f.port, "127.0.0.1"); let text = ""; socket.on("connect", () => socket.write(request)); socket.on("data", b => text += b); socket.on("end", () => resolve(text)); socket.on("error", reject); });
    assert.match(reply, /^HTTP\/1\.1 4[0-9]{2}/);
  }
  assert.equal(f.received.length, 0);
});
