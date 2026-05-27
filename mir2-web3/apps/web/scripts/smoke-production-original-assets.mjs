const DEFAULT_WEB_BASE_URL = "https://mir2.obelisk.build";
const DEFAULT_ASSET_BASE_TEMPLATE = "https://assets.mir2.obelisk.build/mir2/v/{version}";
const DEFAULT_PATHS = [
  "/original-map/WemadeMir2/Objects23/1426.png",
  "/original-map/WemadeMir2/Objects23/1428.png",
  "/original-ui/NPC/27/0.png",
  "/original-ui/NPC/83/0.png",
  "/original-ui/Monster/139/14.png",
];

const args = parseArgs(process.argv.slice(2));
const webBaseUrl = normalizeBaseUrl(args.webBaseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_WEB_BASE_URL);
const assetVersion = await resolveAssetVersion(webBaseUrl, args.assetVersion ?? process.env.MIR2_ASSET_VERSION);
const assetBaseUrl = normalizeBaseUrl(
  resolveVersionTemplate(
    args.assetBaseUrl ?? process.env.MIR2_ASSET_BASE_URL ?? process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? DEFAULT_ASSET_BASE_TEMPLATE,
    assetVersion,
  ),
);
const paths = parsePaths(args.paths ?? process.env.MIR2_ORIGINAL_ASSET_SMOKE_PATHS ?? DEFAULT_PATHS.join(","));

const results = [];
let ok = true;
for (const baseUrl of [webBaseUrl, assetBaseUrl]) {
  for (const path of paths) {
    const url = `${baseUrl}${path}`;
    const result = await probe(url);
    results.push({
      baseUrl,
      path,
      ...result,
    });
    if (!result.ok) ok = false;
  }
}

console.log(
  JSON.stringify(
    {
      ok,
      assetVersion,
      webBaseUrl,
      assetBaseUrl,
      results,
    },
    null,
    2,
  ),
);

if (!ok) {
  process.exitCode = 1;
}

async function resolveAssetVersion(baseUrl, configuredVersion) {
  const normalized = sanitizeAssetVersion(String(configuredVersion ?? ""));
  if (normalized) return normalized;

  const manifestUrl = `${baseUrl}/api/asset-manifest`;
  const response = await fetch(manifestUrl, { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`Failed to resolve asset version from ${manifestUrl}: HTTP ${response.status}`);
  }
  const manifest = await response.json();
  const manifestVersion = sanitizeAssetVersion(String(manifest.version ?? ""));
  if (!manifestVersion) {
    throw new Error("MIR2_ASSET_VERSION is empty and /api/asset-manifest did not provide a version.");
  }
  return manifestVersion;
}

async function probe(url) {
  const startedAt = Date.now();
  let response;
  try {
    response = await fetch(url, { method: "HEAD", cache: "no-store" });
    if (response.status === 405 || response.status === 501) {
      response = await fetch(url, { method: "GET", cache: "no-store" });
    }
  } catch (error) {
    return {
      ok: false,
      status: null,
      elapsedMs: Date.now() - startedAt,
      error: error instanceof Error ? error.message : String(error),
    };
  }

  return {
    ok: response.ok,
    status: response.status,
    elapsedMs: Date.now() - startedAt,
    contentType: response.headers.get("content-type"),
    cacheControl: response.headers.get("cache-control"),
    assetKey: response.headers.get("x-mir2-asset-key"),
  };
}

function resolveVersionTemplate(value, version) {
  return String(value).replaceAll("{version}", version);
}

function normalizeBaseUrl(value) {
  return String(value || "").trim().replace(/\/+$/, "");
}

function sanitizeAssetVersion(value) {
  return String(value || "")
    .trim()
    .replace(/[^a-zA-Z0-9._-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80);
}

function parsePaths(value) {
  return String(value || "")
    .split(",")
    .map((path) => `/${path.trim().replace(/^\/+/, "")}`)
    .filter((path) => path.length > 1);
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) continue;
    const equals = token.indexOf("=");
    if (equals !== -1) {
      parsed[token.slice(2, equals)] = token.slice(equals + 1);
      continue;
    }
    const key = token.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}
