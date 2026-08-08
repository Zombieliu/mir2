export const dynamic = "force-dynamic";
export const runtime = "nodejs";

const ALLOWED_ROOTS = new Set([
  "original-ui",
  "original-map",
  "generated",
  "bevy-entity-atlases",
  "bevy-runtime",
]);

type RouteContext = {
  params: Promise<{ path: string[] }>;
};

export async function GET(request: Request, context: RouteContext) {
  return proxyRemoteAsset(request, context);
}

export async function HEAD(request: Request, context: RouteContext) {
  return proxyRemoteAsset(request, context, true);
}

async function proxyRemoteAsset(
  request: Request,
  context: RouteContext,
  headOnly = false,
) {
  const { path } = await context.params;
  if (
    !Array.isArray(path) ||
    path.length < 2 ||
    !ALLOWED_ROOTS.has(path[0]) ||
    path.some((segment) => !segment || segment === "." || segment === "..")
  ) {
    return new Response("invalid_asset_path", { status: 400 });
  }

  const assetBaseUrl = (
    process.env.MIR2_ASSET_BASE_URL ??
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ??
    ""
  ).trim().replace(/\/+$/, "");
  if (!assetBaseUrl) {
    return new Response("remote_asset_base_unconfigured", { status: 503 });
  }

  const encodedPath = path.map((segment) => encodeURIComponent(segment)).join("/");
  const upstreamUrl = `${assetBaseUrl}/${encodedPath}`;
  const range = request.headers.get("range");
  const upstream = await fetch(upstreamUrl, {
    method: headOnly ? "HEAD" : "GET",
    cache: "force-cache",
    headers: range ? { range } : undefined,
  });

  const headers = new Headers();
  for (const name of [
    "content-type",
    "content-length",
    "content-range",
    "accept-ranges",
    "etag",
    "last-modified",
  ]) {
    const value = upstream.headers.get(name);
    if (value) headers.set(name, value);
  }
  headers.set("cache-control", "public, max-age=31536000, immutable");
  headers.set("access-control-allow-origin", "*");

  return new Response(headOnly ? null : upstream.body, {
    status: upstream.status,
    headers,
  });
}
