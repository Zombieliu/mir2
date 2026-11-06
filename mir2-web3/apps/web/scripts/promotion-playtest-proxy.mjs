#!/usr/bin/env node
// A loopback-only, fixed-origin public boundary for the promotion playtest.
// Never logs request headers, URLs, bodies, cookies or credentials.
import http from "node:http";
import { isIP } from "node:net";
import { pathToFileURL } from "node:url";

export const MAX_BODY_BYTES = 16_384;
const MAX_TARGET_BYTES = 8_192;
const READ = ["GET", "HEAD"];
const API = new Map(Object.entries({
  bootstrap: READ, public: READ, invite: READ, me: READ, members: READ,
  settlements: READ, export: READ, "payout-address": [...READ, "POST"],
  click: ["POST"], apply: ["POST"], candidate: ["POST"], confirm: ["POST"], appeals: ["POST"],
  "auth/challenge": ["POST"], "auth/poll": ["POST"], "auth/logout": ["POST"], "auth/password": ["POST"],
}));
const ASSETS = new Set([
  "/favicon.svg", "/favicon.ico", "/pwa/apple-touch-icon.png",
  "/original-ui/Cursors/Cursor_Default.CUR", "/original-ui/Cursors/Cursor_Normal_Atk.CUR",
  "/original-ui/Cursors/Cursor_Npc.cur", "/original-ui/Cursors/Cursor_TextPrompt.CUR",
]);
const REQUEST_HEADERS = new Set([
  "accept", "accept-encoding", "accept-language", "cache-control", "pragma", "user-agent",
  "cookie", "origin", "referer", "content-type", "content-length", "range", "if-none-match", "if-modified-since",
  "sec-fetch-dest", "sec-fetch-mode", "sec-fetch-site", "sec-fetch-user", "x-promotion-csrf", "idempotency-key",
  "rsc", "next-router-state-tree", "next-router-prefetch", "next-router-segment-prefetch", "next-url",
]);
const HOP_HEADERS = new Set(["connection", "keep-alive", "proxy-authenticate", "proxy-authorization", "te", "trailer", "transfer-encoding", "upgrade"]);
const SECURITY_HEADERS = {
  "cache-control": "no-store", pragma: "no-cache", "x-robots-tag": "noindex, nofollow, noarchive",
  "referrer-policy": "no-referrer", "x-content-type-options": "nosniff", "x-frame-options": "DENY",
  "permissions-policy": "camera=(), microphone=(), geolocation=()",
};

export function validateProxyConfig({ publicOrigin, upstream = "http://127.0.0.1:7359", port = 7374 }) {
  const publicUrl = new URL(publicOrigin);
  if (publicUrl.protocol !== "https:" || publicUrl.username || publicUrl.password || publicUrl.search || publicUrl.hash ||
      publicUrl.pathname !== "/" || publicUrl.port || isIP(publicUrl.hostname) ||
      publicUrl.hostname === "localhost" || publicUrl.hostname.endsWith(".localhost")) throw new Error("A fixed public HTTPS origin is required");
  const target = new URL(upstream);
  if (target.protocol !== "http:" || target.username || target.password || target.search || target.hash || target.pathname !== "/" ||
      !["127.0.0.1", "localhost", "[::1]"].includes(target.hostname)) throw new Error("Upstream must be a loopback HTTP origin");
  if (target.hostname === "localhost") target.hostname = "127.0.0.1";
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("Invalid loopback port");
  return { publicOrigin: publicUrl.origin, publicHost: publicUrl.host, upstream: target.origin, port };
}

export function promotionPathRule(rawTarget) {
  if (typeof rawTarget !== "string" || Buffer.byteLength(rawTarget) > MAX_TARGET_BYTES || !rawTarget.startsWith("/") ||
      rawTarget.startsWith("//") || /[\\\x00-\x20\x7f#]/.test(rawTarget)) return null;
  const [rawPath] = rawTarget.split("?", 1);
  let pathname;
  try { pathname = decodeURIComponent(rawPath); } catch { return null; }
  // No encoded separators, double encoding, dot traversal or hidden files.
  if (/%(?:2f|5c)/i.test(rawPath) || /[%\\\x00-\x20\x7f]/.test(pathname) ||
      pathname.split("/").some(v => v === "." || v === ".." || v.startsWith("."))) return null;
  if (pathname === "/") return { kind: "redirect", methods: READ };
  if (pathname === "/robots.txt") return { kind: "robots", methods: READ };
  if (pathname === "/manifest.webmanifest") return { kind: "manifest", methods: READ };
  if (pathname === "/promotions" || pathname === "/promotions/" ||
      /^\/promotions\/invite\/[A-Za-z0-9_-]{1,128}\/?$/.test(pathname)) return { kind: "page", methods: READ };
  if (pathname.startsWith("/api/promotions/")) {
    const methods = API.get(pathname.slice("/api/promotions/".length));
    return methods ? { kind: "api", methods } : null;
  }
  if (ASSETS.has(pathname)) return { kind: "asset", methods: READ };
  // Next's client JS/CSS/media are required. Raw source, source maps, HMR
  // endpoints, server files and arbitrary game assets are never forwarded.
  if (pathname.startsWith("/_next/static/") && /\.(?:js|css|woff2?|ttf|otf|png|jpe?g|webp|svg|ico)$/i.test(pathname) &&
      !pathname.toLowerCase().endsWith(".map")) return { kind: "asset", methods: READ };
  return null;
}

function answer(res, status, body, extra = {}, head = false) {
  const bytes = Buffer.from(body);
  res.writeHead(status, { ...SECURITY_HEADERS, "content-type": "text/plain; charset=utf-8", "content-length": bytes.length, ...extra });
  res.end(head ? undefined : bytes);
}
function oneHeader(req, name) {
  let count = 0;
  for (let i = 0; i < req.rawHeaders.length; i += 2) if (req.rawHeaders[i].toLowerCase() === name) count++;
  return count <= 1;
}
async function bodyBytes(req) {
  return new Promise((resolve, reject) => {
    const chunks = []; let size = 0;
    const clean = () => { req.off("data", data); req.off("end", end); req.off("error", error); req.off("aborted", error); };
    const error = () => { clean(); reject(new Error("bodyRejected")); };
    const data = chunk => { size += chunk.length; if (size > MAX_BODY_BYTES) { clean(); req.pause(); reject(new Error("bodyTooLarge")); } else chunks.push(chunk); };
    const end = () => { clean(); resolve(Buffer.concat(chunks, size)); };
    req.on("data", data); req.once("end", end); req.once("error", error); req.once("aborted", error);
  });
}

export function createPromotionPlaytestProxy(options) {
  const config = validateProxyConfig(options);
  const server = http.createServer({ maxHeaderSize: 16_384 }, async (req, res) => {
    const rule = promotionPathRule(req.url);
    if (!oneHeader(req, "host") || !oneHeader(req, "origin") ||
        ![config.publicHost, `${config.publicHost}:443`].includes(req.headers.host?.toLowerCase())) return answer(res, 421, "Public host required\n");
    const origin = req.headers.origin;
    if (origin !== undefined && origin !== config.publicOrigin) return answer(res, 403, "Origin rejected\n");
    if (!rule) return answer(res, 404, "Not found\n");
    if (!rule.methods.includes(req.method)) return answer(res, 405, "Method rejected\n", { allow: rule.methods.join(", ") });
    if (req.headers.authorization !== undefined || req.headers["next-action"] !== undefined || req.headers.upgrade ||
        (rule.kind === "api" && req.headers["sec-fetch-site"] === "cross-site") ||
        (req.method === "POST" && origin !== config.publicOrigin)) return answer(res, 403, "Browser request rejected\n");
    const length = req.headers["content-length"];
    if (length !== undefined && (!/^(?:0|[1-9][0-9]*)$/.test(length) || BigInt(length) > BigInt(MAX_BODY_BYTES))) return answer(res, 413, "Body too large\n");
    if ((READ.includes(req.method) && (req.headers["transfer-encoding"] || (length && length !== "0"))) ||
        (req.headers["content-encoding"] && req.headers["content-encoding"] !== "identity")) return answer(res, 400, "Body encoding rejected\n");
    let body;
    try { body = await bodyBytes(req); } catch { return answer(res, 413, "Body too large\n", { connection: "close" }); }
    if (body.length && !req.headers["content-type"]?.toLowerCase().startsWith("application/json")) return answer(res, 415, "JSON required\n");
    if (rule.kind === "redirect") return answer(res, 302, "", { location: "/promotions" });
    if (rule.kind === "robots") return answer(res, 200, "User-agent: *\nDisallow: /\n", {}, req.method === "HEAD");
    if (rule.kind === "manifest") return answer(res, 200, JSON.stringify({ name: "Numeron promotion playtest", short_name: "Numeron", start_url: "/promotions", scope: "/promotions", display: "browser", icons: [{ src: "/favicon.svg", sizes: "any", type: "image/svg+xml" }] }), { "content-type": "application/manifest+json" }, req.method === "HEAD");
    const headers = { host: new URL(config.upstream).host };
    const connectionTokens = new Set((req.headers.connection ?? "").toLowerCase().split(",").map(v => v.trim()));
    for (const [key, value] of Object.entries(req.headers)) if (REQUEST_HEADERS.has(key) && !connectionTokens.has(key) && value !== undefined) headers[key] = value;
    // Preserve the validated external Origin and browser cookies verbatim.
    // No forwarded IP/host, Cloudflare identity or client role headers are trusted.
    if (body.length) headers["content-length"] = String(body.length);
    const pending = http.request(new URL(req.url, config.upstream), { method: req.method, headers, timeout: 25_000 }, upstream => {
      const responseHeaders = {};
      const responseTokens = new Set((upstream.headers.connection ?? "").toLowerCase().split(",").map(v => v.trim()));
      for (const [key, value] of Object.entries(upstream.headers)) if (!HOP_HEADERS.has(key) && !responseTokens.has(key) &&
          !key.startsWith("access-control-") && !["server", "x-powered-by"].includes(key) && value !== undefined) responseHeaders[key] = value;
      if (responseHeaders.location) {
        try {
          const location = new URL(responseHeaders.location, config.upstream);
          if (![config.upstream, config.publicOrigin].includes(location.origin) || !promotionPathRule(location.pathname + location.search)) throw new Error("redirectRejected");
          responseHeaders.location = location.pathname + location.search;
        } catch { upstream.destroy(); return answer(res, 502, "Redirect rejected\n"); }
      }
      res.writeHead(upstream.statusCode ?? 502, { ...responseHeaders, ...SECURITY_HEADERS });
      upstream.on("error", () => res.destroy());
      upstream.pipe(res);
    });
    pending.on("timeout", () => pending.destroy(new Error("upstreamTimeout")));
    pending.on("error", () => { if (!res.headersSent) answer(res, 502, "Upstream unavailable\n"); else res.destroy(); });
    res.on("close", () => { if (!res.writableFinished) pending.destroy(); });
    pending.end(body);
  });
  const refuseSocket = (_req, socket) => socket.end("HTTP/1.1 403 Forbidden\r\nConnection: close\r\nX-Robots-Tag: noindex, nofollow\r\nContent-Length: 0\r\n\r\n");
  server.on("upgrade", refuseSocket);
  server.on("connect", refuseSocket);
  server.headersTimeout = 10_000;
  server.requestTimeout = 30_000;
  server.keepAliveTimeout = 5_000;
  server.maxRequestsPerSocket = 100;
  return server;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const options = {}; const args = process.argv.slice(2);
  if (args.length === 1 && args[0] === "--help") {
    console.log("node promotion-playtest-proxy.mjs --public-origin https://HOST --upstream http://127.0.0.1:7359 --port 7374");
  } else {
    while (args.length) {
      const flag = args.shift(), value = args.shift();
      if (!["--public-origin", "--upstream", "--port"].includes(flag) || !value || Object.hasOwn(options, flag)) throw new Error("Invalid or duplicate argument");
      options[flag] = value;
    }
    const config = validateProxyConfig({ publicOrigin: options["--public-origin"], upstream: options["--upstream"], port: options["--port"] === undefined ? 7374 : Number(options["--port"]) });
    const server = createPromotionPlaytestProxy(config);
    server.listen(config.port, "127.0.0.1", () => console.log(`Promotion-only proxy http://127.0.0.1:${config.port} -> ${config.upstream}; public origin ${config.publicOrigin}`));
    const close = () => server.close(() => process.exit(0));
    process.on("SIGINT", close); process.on("SIGTERM", close);
  }
}
