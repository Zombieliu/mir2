export interface Env {
  MIR2_ALLOWED_PREFIX?: string;
  MIR2_ASSETS: R2Bucket;
  MIR2_CACHE_CONTROL?: string;
}

const DEFAULT_ALLOWED_PREFIX = "mir2/";
const DEFAULT_CACHE_CONTROL = "public, max-age=31536000, immutable";
const RELEASE_MANIFEST_CACHE_CONTROL = "public, max-age=60, stale-while-revalidate=300";
const CORS_EXPOSE_HEADERS = "accept-ranges, content-length, content-range, etag, x-mir2-edge-cache";

const CONTENT_TYPES: Record<string, string> = {
  ".bmp": "image/bmp",
  ".json": "application/json; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".png": "image/png",
  ".wasm": "application/wasm",
  ".wav": "audio/wav",
};

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    if (request.method === "OPTIONS") return options();
    if (request.method !== "GET" && request.method !== "HEAD") {
      return text("method_not_allowed", 405, "no-store");
    }

    const url = new URL(request.url);
    const key = decodeObjectKey(url.pathname);
    if (!isAllowedKey(key, env.MIR2_ALLOWED_PREFIX || DEFAULT_ALLOWED_PREFIX)) {
      return text("not_found", 404, "no-store");
    }

    const rangeHeader = request.headers.get("range");
    if (rangeHeader) {
      return serveFromR2(request, env, key, "BYPASS", parseRange(rangeHeader));
    }

    const cache = caches.default;
    const cacheKey = new Request(cacheUrl(url), { method: "GET" });
    const cached = await cache.match(cacheKey);
    if (cached) return withCacheHeader(cached, request.method, "HIT");

    const response = await serveFromR2(request, env, key, "MISS");
    if (request.method === "GET" && response.ok) {
      ctx.waitUntil(cache.put(cacheKey, response.clone()));
    }
    return response;
  },
};

async function serveFromR2(
  request: Request,
  env: Env,
  key: string,
  cacheState: "MISS" | "BYPASS",
  range?: R2Range,
): Promise<Response> {
  const object = await env.MIR2_ASSETS.get(key, range ? { range } : undefined);
  if (!object) return text("not_found", 404, "no-store", cacheState);

  const etag = object.httpEtag;
  if (!range && etag && request.headers.get("if-none-match") === etag) {
    return new Response(null, {
      status: 304,
      headers: responseHeaders(key, object, env, cacheState),
    });
  }

  const headers = responseHeaders(key, object, env, cacheState);
  const status = range ? 206 : 200;
  if (range && object.range) {
    headers.set("content-range", `bytes ${object.range.offset}-${object.range.end - 1}/${object.size}`);
  }

  if (request.method === "HEAD") {
    return new Response(null, { status, headers });
  }

  return new Response(object.body, { status, headers });
}

function responseHeaders(
  key: string,
  object: R2Object,
  env: Env,
  cacheState: "HIT" | "MISS" | "BYPASS",
): Headers {
  const headers = new Headers();
  const objectCacheControl = object.httpMetadata?.cacheControl;
  headers.set("accept-ranges", "bytes");
  applyCorsHeaders(headers);
  headers.set("cache-control", cacheControlForKey(key, objectCacheControl, env));
  headers.set("content-type", object.httpMetadata?.contentType || inferContentType(key));
  headers.set("etag", object.httpEtag);
  headers.set("x-mir2-edge-cache", cacheState);
  return headers;
}

function cacheControlForKey(key: string, objectValue: string | undefined, env: Env): string {
  if (key.endsWith("/remote-asset-release.json")) return RELEASE_MANIFEST_CACHE_CONTROL;
  return objectValue || env.MIR2_CACHE_CONTROL || DEFAULT_CACHE_CONTROL;
}

function inferContentType(key: string): string {
  const dotIndex = key.lastIndexOf(".");
  const extension = dotIndex >= 0 ? key.slice(dotIndex).toLowerCase() : "";
  return CONTENT_TYPES[extension] || "application/octet-stream";
}

function cacheUrl(url: URL): string {
  const normalized = new URL(url.href);
  normalized.search = "";
  normalized.hash = "";
  return normalized.href;
}

function decodeObjectKey(pathname: string): string {
  const key = pathname
    .split("/")
    .filter(Boolean)
    .map((segment) => {
      try {
        return decodeURIComponent(segment);
      } catch {
        return "";
      }
    })
    .join("/");
  if (!key || key.includes("..") || key.includes("\\")) return "";
  return key;
}

function isAllowedKey(key: string, allowedPrefix: string): boolean {
  const normalizedPrefix = allowedPrefix.trim().replace(/^\/+/, "");
  return Boolean(key && normalizedPrefix && key.startsWith(normalizedPrefix));
}

function parseRange(value: string): R2Range | undefined {
  const match = /^bytes=(\d+)-(\d*)$/i.exec(value.trim());
  if (!match) return undefined;
  const offset = Number(match[1]);
  const end = match[2] ? Number(match[2]) + 1 : undefined;
  if (!Number.isSafeInteger(offset) || offset < 0) return undefined;
  if (end !== undefined && (!Number.isSafeInteger(end) || end <= offset)) return undefined;
  return end === undefined ? { offset } : { offset, length: end - offset };
}

function withCacheHeader(response: Response, method: string, cacheState: "HIT"): Response {
  const headers = new Headers(response.headers);
  applyCorsHeaders(headers);
  headers.set("x-mir2-edge-cache", cacheState);
  return new Response(method === "HEAD" ? null : response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

function options(): Response {
  return new Response(null, {
    status: 204,
    headers: {
      "access-control-allow-origin": "*",
      "access-control-allow-methods": "GET, HEAD, OPTIONS",
      "access-control-allow-headers": "range, if-none-match",
      "access-control-expose-headers": CORS_EXPOSE_HEADERS,
      "access-control-max-age": "86400",
      "cache-control": "public, max-age=86400",
      "alt-svc": "clear",
    },
  });
}

function text(
  body: string,
  status: number,
  cacheControl: string,
  cacheState?: "MISS" | "BYPASS",
): Response {
  const headers = new Headers({
    "cache-control": cacheControl,
    "content-type": "text/plain; charset=utf-8",
  });
  applyCorsHeaders(headers);
  if (cacheState) headers.set("x-mir2-edge-cache", cacheState);
  return new Response(body, { status, headers });
}

function applyCorsHeaders(headers: Headers): void {
  headers.set("access-control-allow-origin", "*");
  headers.set("access-control-allow-methods", "GET, HEAD, OPTIONS");
  headers.set("access-control-allow-headers", "range, if-none-match");
  headers.set("access-control-expose-headers", CORS_EXPOSE_HEADERS);
  headers.set("alt-svc", "clear");
}
