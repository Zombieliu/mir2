const DEFAULT_ORIGIN_URL =
  "https://mir2-web3-web.vercel.app";
const DEFAULT_GATEWAY_ORIGIN_URL = "https://165.154.65.136.sslip.io";
const DEFAULT_ASSET_CACHE_CONTROL = "public, max-age=31536000, immutable";
const ASSET_NOT_FOUND_CACHE_CONTROL = "public, max-age=60";
const ASSET_WORKER_SCRIPT_PATH = "/mir2-asset-worker.js";
const ASSET_WORKER_CACHE_CONTROL = "public, max-age=0, must-revalidate";

const HOP_BY_HOP_HEADERS = [
  "connection",
  "keep-alive",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
];

export interface Env {
  ASSET_ORIGIN_URL?: string;
  GATEWAY_ORIGIN_URL?: string;
  MIR2_ASSET_OBJECT_PREFIX?: string;
  MIR2_ASSET_VERSION?: string;
  MIR2_ASSETS?: R2Bucket;
  ORIGIN_URL?: string;
  VERCEL_BYPASS_SECRET?: string;
}

const ASSET_CONTENT_TYPES: Record<string, string> = {
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".wav": "audio/wav",
  ".cur": "image/x-icon",
  ".gif": "image/gif",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
};

function canHaveRequestBody(method: string): boolean {
  return method !== "GET" && method !== "HEAD";
}

function isGatewayRequest(url: URL): boolean {
  return url.pathname === "/ws" || url.pathname.startsWith("/ws/");
}

function isStaticAssetRequest(url: URL): boolean {
  return (
    url.pathname.startsWith("/original-ui/") ||
    url.pathname.startsWith("/original-map/") ||
    url.pathname.startsWith("/generated/original-map-blend/")
  );
}

function originalUiMetaLibrary(url: URL): string | null {
  if (url.pathname !== "/api/original-ui-meta") return null;

  const library = url.searchParams.get("library") ?? "";
  const normalized = library
    .replaceAll("\\", "/")
    .split("/")
    .filter(Boolean)
    .join("/");
  if (
    !normalized ||
    normalized.startsWith("/") ||
    normalized.startsWith("Map/") ||
    normalized.split("/").some((segment) => segment === "." || segment === "..")
  ) {
    return null;
  }
  return normalized;
}

function rewriteToAssetOrigin(request: Request, assetOriginUrl: string): Request {
  const origin = new URL(assetOriginUrl);
  const target = new URL(request.url);

  target.protocol = origin.protocol;
  target.hostname = origin.hostname;
  target.port = origin.port;
  target.username = "";
  target.password = "";
  target.pathname = `${origin.pathname.replace(/\/+$/, "")}${target.pathname}`;

  const headers = new Headers(request.headers);
  headers.delete("host");

  return new Request(target, {
    headers,
    method: request.method,
    redirect: "manual",
  });
}

function rewriteToGateway(request: Request, gatewayOriginUrl: string): Request {
  const incomingUrl = new URL(request.url);
  const origin = new URL(gatewayOriginUrl);
  const target = new URL(request.url);

  target.protocol = origin.protocol;
  target.hostname = origin.hostname;
  target.port = origin.port;
  target.username = "";
  target.password = "";

  const headers = new Headers(request.headers);
  headers.delete("host");
  headers.set("x-forwarded-host", incomingUrl.host);
  headers.set("x-forwarded-proto", incomingUrl.protocol.replace(":", ""));

  const clientIp = request.headers.get("cf-connecting-ip");
  if (clientIp) {
    headers.set("x-forwarded-for", clientIp);
  }

  return new Request(target, {
    body: canHaveRequestBody(request.method) ? request.body : undefined,
    cf: request.cf,
    duplex: "half",
    headers,
    method: request.method,
    redirect: "manual",
  } as RequestInit & { duplex: "half" });
}

function rewriteToOrigin(
  request: Request,
  originUrl: string,
  vercelBypassSecret?: string,
): Request {
  const incomingUrl = new URL(request.url);
  const origin = new URL(originUrl);
  const target = new URL(request.url);

  target.protocol = origin.protocol;
  target.hostname = origin.hostname;
  target.port = origin.port;
  target.username = "";
  target.password = "";

  const headers = new Headers(request.headers);
  headers.set("x-forwarded-host", incomingUrl.host);
  headers.set("x-forwarded-proto", incomingUrl.protocol.replace(":", ""));

  const clientIp = request.headers.get("cf-connecting-ip");
  if (clientIp) {
    headers.set("x-forwarded-for", clientIp);
  }

  if (vercelBypassSecret) {
    headers.set("x-vercel-protection-bypass", vercelBypassSecret);
  }

  return new Request(target, {
    body: canHaveRequestBody(request.method) ? request.body : undefined,
    cf: request.cf,
    duplex: "half",
    headers,
    method: request.method,
    redirect: "manual",
  } as RequestInit & { duplex: "half" });
}

function rewriteLocationHeader(
  responseHeaders: Headers,
  publicUrl: URL,
  originUrl: string,
): void {
  const location = responseHeaders.get("location");
  if (!location) {
    return;
  }

  const origin = new URL(originUrl);
  const rewritten = new URL(location, origin);
  if (rewritten.host !== origin.host) {
    return;
  }

  rewritten.protocol = publicUrl.protocol;
  rewritten.host = publicUrl.host;
  responseHeaders.set("location", rewritten.toString());
}

function cleanResponseHeaders(response: Response, publicUrl: URL, originUrl: string): Headers {
  const headers = new Headers(response.headers);

  for (const header of HOP_BY_HOP_HEADERS) {
    headers.delete(header);
  }

  rewriteLocationHeader(headers, publicUrl, originUrl);
  appendNoTransformForHtml(headers);
  applyAssetWorkerHeaders(headers, publicUrl);
  disableHttp3AltSvc(headers);
  return headers;
}

function applyAssetWorkerHeaders(headers: Headers, publicUrl: URL): void {
  if (publicUrl.pathname !== ASSET_WORKER_SCRIPT_PATH) {
    return;
  }

  headers.set("cache-control", ASSET_WORKER_CACHE_CONTROL);
  headers.set("service-worker-allowed", "/");
}

function cleanAssetResponse(response: Response): Response {
  const headers = new Headers(response.headers);
  for (const header of HOP_BY_HOP_HEADERS) {
    headers.delete(header);
  }
  disableHttp3AltSvc(headers);
  headers.set("x-mir2-domain-proxy", "asset");
  return new Response(response.body, {
    headers,
    status: response.status,
    statusText: response.statusText,
  });
}

async function serveStaticAssetFromR2(
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  publicUrl: URL,
): Promise<Response> {
  const releaseConfig = resolveAssetReleaseConfig(env);
  if (releaseConfig.errorResponse) {
    return releaseConfig.errorResponse;
  }

  const { assetVersion, assetObjectPrefix, assetOriginUrl } = releaseConfig;

  if (request.method !== "GET" && request.method !== "HEAD") {
    return assetError("method_not_allowed", 405, "method_not_allowed", publicUrl.pathname, null);
  }

  if (!env.MIR2_ASSETS) {
    const requestAssetOriginUrl = assetOriginUrl.includes("{version}")
      ? assetOriginUrl.replace("{version}", assetVersion)
      : assetOriginUrl;
    const assetResponse = await fetch(rewriteToAssetOrigin(request, requestAssetOriginUrl));
    return cleanAssetResponse(assetResponse);
  }

  const objectKey = assetObjectKeyForPath(publicUrl.pathname, assetObjectPrefix);
  if (!objectKey) {
    return assetError("asset_not_found", 404, "invalid_asset_path", publicUrl.pathname, null);
  }

  const cacheKey = new Request(`https://mir2-r2-cache.local/${objectKey}`, { method: "GET" });
  if (request.method === "GET") {
    const cached = await caches.default.match(cacheKey);
    if (cached) {
      const cachedHeaders = new Headers(cached.headers);
      cachedHeaders.set("x-mir2-domain-proxy", "r2-asset");
      cachedHeaders.set("x-mir2-edge-cache", "HIT");
      return new Response(cached.body, {
        headers: cachedHeaders,
        status: cached.status,
        statusText: cached.statusText,
      });
    }
  }

  const object = await env.MIR2_ASSETS.get(objectKey);
  if (!object) {
    return assetError("asset_not_found", 404, "asset_not_found", publicUrl.pathname, objectKey);
  }

  const headers = assetObjectHeaders(objectKey, object, env, assetVersion, "MISS");
  const response = new Response(request.method === "HEAD" ? null : object.body, {
    headers,
    status: 200,
  });
  if (request.method === "GET") {
    ctx.waitUntil(caches.default.put(cacheKey, response.clone()));
  }
  return response;
}

async function serveOriginalUiMetaFromR2(
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  library: string,
): Promise<Response | null> {
  const publicUrl = new URL(request.url);
  publicUrl.pathname = `/original-ui/${library}/meta.json`;
  publicUrl.search = "";
  const response = await serveStaticAssetFromR2(request, env, ctx, publicUrl);
  return response.ok ? response : null;
}

function assetObjectHeaders(
  objectKey: string,
  object: R2Object,
  env: Env,
  assetVersion: string,
  cacheState: "MISS" | "HIT",
): Headers {
  const headers = new Headers();
  headers.set("cache-control", object.httpMetadata?.cacheControl || DEFAULT_ASSET_CACHE_CONTROL);
  headers.set("content-type", object.httpMetadata?.contentType || contentTypeForObjectKey(objectKey));
  headers.set("etag", object.httpEtag);
  headers.set("x-mir2-asset-key", objectKey);
  headers.set("x-mir2-asset-version", assetVersion);
  headers.set("x-mir2-domain-proxy", "r2-asset");
  headers.set("x-mir2-edge-cache", cacheState);
  disableHttp3AltSvc(headers);
  return headers;
}

function assetError(
  error: string,
  status: number,
  code: string,
  publicPath: string,
  objectKey: string | null,
): Response {
  const headers = new Headers({
    "cache-control": status === 404 ? ASSET_NOT_FOUND_CACHE_CONTROL : "no-store",
    "content-type": "application/json; charset=utf-8",
  });
  if (publicPath) {
    headers.set("x-mir2-public-path", publicPath);
  }
  if (objectKey) {
    headers.set("x-mir2-asset-key", objectKey);
  }
  disableHttp3AltSvc(headers);
  const payload = {
    error,
    code,
    publicPath,
    objectKey,
  };
  return new Response(JSON.stringify(payload), {
    headers,
    status,
  });
}

function assetObjectKeyForPath(pathname: string, prefix: string): string {
  if (!prefix) {
    return "";
  }

  const assetPath = pathname
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
  if (!assetPath || assetPath.includes("..") || assetPath.includes("\\")) {
    return "";
  }
  return `${prefix}/${assetPath}`;
}

function normalizeAssetObjectPrefix(value: string | undefined): string {
  return String(value ?? "").trim().replace(/^\/+|\/+$/g, "");
}

type AssetReleaseConfigResult = {
  assetVersion: string;
  assetObjectPrefix: string;
  assetOriginUrl: string;
  errorResponse: null | Response;
};

function resolveAssetReleaseConfig(env: Env): AssetReleaseConfigResult {
  const assetVersion = normalizeAssetVersion(env.MIR2_ASSET_VERSION || "");
  const objectPrefix = normalizeAssetObjectPrefix(env.MIR2_ASSET_OBJECT_PREFIX);
  const assetOriginUrl = (env.ASSET_ORIGIN_URL || "").trim();

  if (!assetVersion || !objectPrefix || !assetOriginUrl) {
    return {
      assetVersion: "",
      assetObjectPrefix: "",
      assetOriginUrl: "",
      errorResponse: assetError(
        "asset_release_config_missing",
        503,
        "asset_release_config_missing",
        "",
        null,
      ),
    };
  }

  return {
    assetVersion,
    assetObjectPrefix: objectPrefix,
    assetOriginUrl,
    errorResponse: null,
  };
}

function normalizeAssetVersion(value: string): string {
  return value
    .trim()
    .replace(/[^a-zA-Z0-9._-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80);
}

function contentTypeForObjectKey(objectKey: string): string {
  const dotIndex = objectKey.lastIndexOf(".");
  const extension = dotIndex >= 0 ? objectKey.slice(dotIndex).toLowerCase() : "";
  return ASSET_CONTENT_TYPES[extension] || "application/octet-stream";
}

function disableHttp3AltSvc(headers: Headers): void {
  headers.set("alt-svc", "clear");
}

function appendNoTransformForHtml(headers: Headers): void {
  const contentType = headers.get("content-type") ?? "";
  if (!contentType.toLowerCase().includes("text/html")) {
    return;
  }

  const cacheControl = headers.get("cache-control") ?? "";
  if (/\bno-transform\b/i.test(cacheControl)) {
    return;
  }

  headers.set(
    "cache-control",
    cacheControl ? `${cacheControl}, no-transform` : "no-transform",
  );
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const publicUrl = new URL(request.url);
    if (isGatewayRequest(publicUrl)) {
      const gatewayOriginUrl =
        env.GATEWAY_ORIGIN_URL || DEFAULT_GATEWAY_ORIGIN_URL;
      return fetch(rewriteToGateway(request, gatewayOriginUrl));
    }

    if (isStaticAssetRequest(publicUrl)) {
      return serveStaticAssetFromR2(request, env, ctx, publicUrl);
    }

    if (request.method === "GET" || request.method === "HEAD") {
      const library = originalUiMetaLibrary(publicUrl);
      if (library) {
        const assetResponse = await serveOriginalUiMetaFromR2(request, env, ctx, library);
        if (assetResponse) return assetResponse;
      }
    }

    const originUrl = env.ORIGIN_URL || DEFAULT_ORIGIN_URL;
    const proxyRequest = rewriteToOrigin(
      request,
      originUrl,
      env.VERCEL_BYPASS_SECRET,
    );
    const response = await fetch(proxyRequest);

    return new Response(response.body, {
      headers: cleanResponseHeaders(response, publicUrl, originUrl),
      status: response.status,
      statusText: response.statusText,
    });
  },
};
