/** Dedicated native delivery; no credentials and no mutable object cache. */
export const PUBLIC_BASE = "https://assets.mir2.obelisk.build/client-updates/";
export const OBJECT_PREFIX = "mir2/native/windows-invited/";
export const POINTER_KEY = OBJECT_PREFIX + "channels/invited.json";
export const MAX_ARTIFACT = 128 * 1024 * 1024;
export const MAX_METADATA = 32 * 1024 * 1024;
export const MAX_POINTER = 4096;
const IMMUTABLE = "public, max-age=31536000, immutable";
const POINTER_SCHEMA = "mir2.windows.r2-channel.v1";
const POINTER_FIELDS = ["schema", "channel", "platform", "sequence", "sourceRevision", "feedSha256", "signatureSha256"];
const ALIASES = new Set(["latest.json", "latest.p7s"]);

export function validatePointer(value) {
  if (!value || Array.isArray(value) || typeof value !== "object") throw new Error("invalid_pointer");
  if (Object.keys(value).sort().join(",") !== [...POINTER_FIELDS].sort().join(",")) throw new Error("invalid_pointer");
  if (value.schema !== POINTER_SCHEMA || value.channel !== "invited" || value.platform !== "windows-x64" ||
      !Number.isSafeInteger(value.sequence) || value.sequence < 1 ||
      typeof value.sourceRevision !== "string" || typeof value.feedSha256 !== "string" || typeof value.signatureSha256 !== "string" ||
      !/^[0-9a-f]{40}$/.test(value.sourceRevision) ||
      !/^[0-9a-f]{64}$/.test(value.feedSha256) ||
      !/^[0-9a-f]{64}$/.test(value.signatureSha256)) throw new Error("invalid_pointer");
  return value;
}

export function feedDirectory(pointer) {
  return "feeds/s" + pointer.sequence + "-" + pointer.feedSha256 + "/";
}

export function relativePath(url) {
  if (url.protocol !== "https:" || url.host !== "assets.mir2.obelisk.build" ||
      url.href.includes("?") || url.href.includes("#") || !url.pathname.startsWith("/client-updates/")) return null;
  const raw = url.pathname.slice("/client-updates/".length);
  // Separators must be literal. Decode exactly once; any residual '%' is invalid.
  if (/%(?:2f|5c)/i.test(raw)) return null;
  let path;
  try { path = decodeURIComponent(raw); } catch { return null; }
  if (!path || !/^[A-Za-z0-9._/-]+$/.test(path) ||
      path.split("/").some((s) => !s || s === "." || s === ".." || s.startsWith("."))) return null;
  if (ALIASES.has(path)) return path;
  return /^(releases|feeds|installers)\/[^/]/.test(path) ? path : null;
}

function commonHeaders(cacheControl) {
  return new Headers({
    "cache-control": cacheControl,
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "etag, content-length, content-range, accept-ranges, x-mir2-sequence, x-mir2-feed-sha256, x-mir2-feed-path, x-mir2-signature-path",
    "x-content-type-options": "nosniff",
  });
}

function errorResponse(code, status, extra = {}) {
  const headers = commonHeaders("no-store");
  headers.set("content-type", "application/json");
  for (const [key, value] of Object.entries(extra)) headers.set(key, value);
  return new Response(JSON.stringify({ error: code }), { status, headers });
}

function metadataLimit(path) {
  return /\.(json|p7s|sig|txt)$/i.test(path) ? MAX_METADATA : MAX_ARTIFACT;
}

function validateObject(object, path, limit = metadataLimit(path)) {
  if (!Number.isSafeInteger(object.size) || object.size < 0 || object.size > limit) throw new Error("invalid_storage_size");
  const encoding = object.httpMetadata?.contentEncoding;
  if (encoding && encoding !== "identity") throw new Error("invalid_storage_encoding");
  if (typeof object.httpEtag !== "string" || !/^"[^"\r\n]+"$/.test(object.httpEtag)) throw new Error("invalid_storage_etag");
}

async function readBounded(body, maximum, expected) {
  const reader = body.getReader();
  const chunks = [];
  let length = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > maximum) throw new Error("invalid_pointer");
      chunks.push(value);
    }
  } finally {
    await reader.cancel().catch(() => {});
  }
  if (length !== expected) throw new Error("invalid_pointer");
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  return bytes;
}

async function loadPointer(bucket) {
  // One R2 get returns body + metadata from one atomic pointer version.
  const object = await bucket.get(POINTER_KEY);
  if (!object) throw new Error("pointer_unavailable");
  validateObject(object, "channels/invited.json", MAX_POINTER);
  let text, value;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(await readBounded(object.body, MAX_POINTER, object.size));
    value = validatePointer(JSON.parse(text));
  } catch {
    throw new Error("invalid_pointer");
  }
  // Canonical minified JSON also rejects duplicate fields and alternative numbers.
  if (text !== JSON.stringify(value) && text !== JSON.stringify(value) + "\n") throw new Error("invalid_pointer");
  return value;
}

export function parseRange(value, size) {
  const match = /^bytes=(\d*)-(\d*)$/i.exec(value.trim());
  if (!match || (!match[1] && !match[2]) || size === 0) return null;
  const first = match[1] ? Number(match[1]) : null;
  const last = match[2] ? Number(match[2]) : null;
  if ((first !== null && !Number.isSafeInteger(first)) || (last !== null && !Number.isSafeInteger(last))) return null;
  if (first === null) {
    if (last === 0) return null;
    const length = Math.min(last, size);
    return { offset: size - length, length };
  }
  if (first >= size || (last !== null && last < first)) return null;
  return { offset: first, length: Math.min(last === null ? size - 1 : last, size - 1) - first + 1 };
}

function etagMatches(value, etag) {
  return value.split(",").some((tag) => tag.trim() === "*" || tag.trim().replace(/^W\//, "") === etag);
}

function allowRange(ifRange, object) {
  if (!ifRange) return true;
  if (ifRange.startsWith('"') || ifRange.startsWith("W/")) return ifRange === object.httpEtag;
  const date = Date.parse(ifRange);
  const uploaded = object.uploaded ? new Date(object.uploaded).getTime() : NaN;
  // If-Range uses exact date equality, unlike If-Unmodified-Since (RFC 9110).
  return Number.isFinite(date) && Number.isFinite(uploaded) && Math.floor(uploaded / 1000) * 1000 === date;
}

function contentType(path) {
  if (path.endsWith(".json")) return "application/json";
  if (path.endsWith(".p7s")) return "application/pkcs7-signature";
  if (path.endsWith(".gz")) return "application/gzip";
  return "application/octet-stream";
}

function objectHeaders(object, path, alias, pointer, range, cacheState) {
  const headers = commonHeaders(alias ? "no-store" : IMMUTABLE);
  headers.set("content-type", contentType(path));
  headers.set("etag", object.httpEtag);
  headers.set("accept-ranges", "bytes");
  headers.set("content-length", String(range ? range.length : object.size));
  headers.set("x-mir2-native-cache", cacheState);
  if (object.uploaded) headers.set("last-modified", new Date(object.uploaded).toUTCString());
  if (range) headers.set("content-range", "bytes " + range.offset + "-" + (range.offset + range.length - 1) + "/" + object.size);
  if (pointer) {
    headers.set("x-mir2-sequence", String(pointer.sequence));
    headers.set("x-mir2-feed-sha256", pointer.feedSha256);
    headers.set("x-mir2-feed-path", feedDirectory(pointer) + "latest.json");
    headers.set("x-mir2-signature-path", feedDirectory(pointer) + "latest.p7s");
  }
  // .gz files are artifacts, not an HTTP Content-Encoding representation.
  return headers;
}

function notModified(headers) {
  const copy = new Headers(headers);
  copy.delete("content-length");
  copy.delete("content-range");
  return new Response(null, { status: 304, headers: copy });
}

async function serve(request, env, ctx, cache) {
  if (request.method !== "GET" && request.method !== "HEAD") return errorResponse("method_not_allowed", 405, { allow: "GET, HEAD" });
  const url = new URL(request.url);
  const path = relativePath(url);
  if (path === null) return errorResponse("not_found", 404);
  const alias = ALIASES.has(path);
  try {
    const pointer = alias ? await loadPointer(env.MIR2_ASSETS) : null;
    const resolved = alias ? feedDirectory(pointer) + path : path;
    const key = OBJECT_PREFIX + resolved;
    const cacheKey = new Request(PUBLIC_BASE + resolved, { method: "GET" });
    const rangeHeader = request.method === "GET" ? request.headers.get("range") : null;
    if (!alias && !rangeHeader && cache) {
      const cached = await cache.match(cacheKey);
      if (cached) {
        const headers = new Headers(cached.headers);
        headers.set("cache-control", IMMUTABLE);
        headers.set("x-mir2-native-cache", "HIT");
        headers.delete("content-encoding");
        if (request.headers.has("if-none-match") && etagMatches(request.headers.get("if-none-match"), headers.get("etag"))) return notModified(headers);
        return new Response(request.method === "HEAD" ? null : cached.body, { status: 200, headers });
      }
    }
    const object = await env.MIR2_ASSETS.head(key);
    if (!object) return errorResponse("not_found", 404);
    validateObject(object, resolved);
    const expectedHash = pointer && (path === "latest.json" ? pointer.feedSha256 : pointer.signatureSha256);
    if (expectedHash && object.customMetadata?.sha256 && object.customMetadata.sha256 !== expectedHash) throw new Error("invalid_storage_hash");
    let headers = objectHeaders(object, resolved, alias, pointer, null, alias || rangeHeader ? "BYPASS" : "MISS");
    if (request.headers.has("if-none-match") && etagMatches(request.headers.get("if-none-match"), object.httpEtag)) return notModified(headers);
    let range = null;
    if (rangeHeader && allowRange(request.headers.get("if-range"), object)) {
      range = parseRange(rangeHeader, object.size);
      if (!range) return errorResponse("range_not_satisfiable", 416, { "content-range": "bytes */" + object.size, "accept-ranges": "bytes", etag: object.httpEtag });
    }
    headers = objectHeaders(object, resolved, alias, pointer, range, alias || rangeHeader ? "BYPASS" : "MISS");
    if (request.method === "HEAD") return new Response(null, { status: 200, headers });
    const data = await env.MIR2_ASSETS.get(key, range ? { range } : undefined);
    if (!data) return errorResponse("not_found", 404);
    validateObject(data, resolved);
    if (data.size !== object.size || data.httpEtag !== object.httpEtag ||
        (range && (!data.range || data.range.offset !== range.offset || data.range.length !== range.length))) throw new Error("inconsistent_storage_object");
    const response = new Response(data.body, { status: range ? 206 : 200, headers });
    if (!alias && !range && !rangeHeader && cache) {
      // A cache failure cannot change an otherwise successful download.
      ctx.waitUntil(cache.put(cacheKey, response.clone()).catch(() => {}));
    }
    return response;
  } catch (error) {
    const code = error instanceof Error && /^(invalid_|inconsistent_)/.test(error.message) ? "invalid_storage_object" : "delivery_unavailable";
    return errorResponse(code, code === "invalid_storage_object" ? 502 : 503);
  }
}

export async function handle(request, env, ctx, cache = globalThis.caches?.default) {
  const response = await serve(request, env, ctx, cache);
  // HEAD also suppresses bodies on storage/path failures, while retaining headers.
  return request.method === "HEAD" && response.body
    ? new Response(null, { status: response.status, headers: response.headers })
    : response;
}

export default { fetch: handle };
