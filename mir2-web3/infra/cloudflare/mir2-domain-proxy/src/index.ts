const DEFAULT_ORIGIN_URL =
  "https://mir2-web3-web.vercel.app";
const DEFAULT_GATEWAY_ORIGIN_URL = "https://165.154.65.136.sslip.io";
const DEFAULT_ASSET_ORIGIN_URL =
  "https://assets.mir2.obelisk.build/mir2/v/37596e16d64fde7c";

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
  ORIGIN_URL?: string;
  VERCEL_BYPASS_SECRET?: string;
}

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

function rewriteOriginalUiMetaToAssetOrigin(
  request: Request,
  assetOriginUrl: string,
  library: string,
): Request {
  const origin = new URL(assetOriginUrl);
  const target = new URL(request.url);

  target.protocol = origin.protocol;
  target.hostname = origin.hostname;
  target.port = origin.port;
  target.username = "";
  target.password = "";
  target.pathname = `${origin.pathname.replace(/\/+$/, "")}/original-ui/${library}/meta.json`;
  target.search = "";

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
  disableHttp3AltSvc(headers);
  return headers;
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
  async fetch(request: Request, env: Env): Promise<Response> {
    const publicUrl = new URL(request.url);
    if (isGatewayRequest(publicUrl)) {
      const gatewayOriginUrl =
        env.GATEWAY_ORIGIN_URL || DEFAULT_GATEWAY_ORIGIN_URL;
      return fetch(rewriteToGateway(request, gatewayOriginUrl));
    }

    if (isStaticAssetRequest(publicUrl)) {
      const assetOriginUrl = env.ASSET_ORIGIN_URL || DEFAULT_ASSET_ORIGIN_URL;
      const assetResponse = await fetch(rewriteToAssetOrigin(request, assetOriginUrl));
      return cleanAssetResponse(assetResponse);
    }

    if (request.method === "GET" || request.method === "HEAD") {
      const library = originalUiMetaLibrary(publicUrl);
      if (library) {
        const assetOriginUrl = env.ASSET_ORIGIN_URL || DEFAULT_ASSET_ORIGIN_URL;
        const assetResponse = await fetch(
          rewriteOriginalUiMetaToAssetOrigin(request, assetOriginUrl, library),
        );
        if (assetResponse.ok) {
          return cleanAssetResponse(assetResponse);
        }
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
