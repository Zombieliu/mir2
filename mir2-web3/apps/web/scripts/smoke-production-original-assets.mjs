const DEFAULT_WEB_BASE_URL = "https://mir2.obelisk.build";
const DEFAULT_ASSET_BASE_TEMPLATE = "https://assets.mir2.obelisk.build/mir2/v/{version}";

const LOGIN_TITLE_PATHS = [
  ...makeRange(30, 32),
  ...makeRange(320, 334),
].map((value) => `/original-ui/Title/${value}.png`);

const LOGIN_CHRSEL_PATHS = Array.from({ length: 19 }, (_, index) => `/original-ui/ChrSel/${index}.png`);

const EXTRA_ORIGINAL_ASSET_PATHS = [
  "/original-ui/Sound/Login2.wav",
  "/original-ui/Sound/100.wav",
  "/original-ui/Prguse/44.png",
  "/original-ui/Prguse/65.png",
  "/original-ui/Prguse/940.png",
  "/original-ui/Title/40.png",
  ...makeRange(340, 362).map((value) => `/original-ui/Title/${value}.png`),
];

const REQUIRED_PATHS = [
  ...LOGIN_TITLE_PATHS,
  ...LOGIN_CHRSEL_PATHS,
  "/original-ui/Prguse/1084.png",
  "/original-ui/Cursors/Cursor_Default.CUR",
  "/original-ui/Cursors/Cursor_TextPrompt.CUR",
  ...EXTRA_ORIGINAL_ASSET_PATHS,
];

const DEFAULT_REQUIRED_PATHS = REQUIRED_PATHS;

const args = parseArgs(process.argv.slice(2));
const webBaseUrl = normalizeBaseUrl(args.webBaseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_WEB_BASE_URL);
const configuredAssetVersion = sanitizeAssetVersion(args.assetVersion ?? process.env.MIR2_ASSET_VERSION ?? "");
const assetVersion = configuredAssetVersion
  ? configuredAssetVersion
  : await resolveAssetVersion(webBaseUrl);
const assetBaseUrl = normalizeBaseUrl(
  resolveVersionTemplate(
    args.assetBaseUrl ?? process.env.MIR2_ASSET_BASE_URL ?? process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? DEFAULT_ASSET_BASE_TEMPLATE,
    assetVersion,
  ),
);
const targets = parseTargets(args.targets ?? process.env.MIR2_ORIGINAL_ASSET_SMOKE_TARGETS ?? "web,cdn");
const pathMode = (args.pathMode ?? process.env.MIR2_ORIGINAL_ASSET_SMOKE_PATH_MODE ?? "extended").toLowerCase();
const basePathList =
  pathMode === "required" ? REQUIRED_PATHS : DEFAULT_REQUIRED_PATHS;
const paths = parsePaths(args.paths ?? process.env.MIR2_ORIGINAL_ASSET_SMOKE_PATHS ?? basePathList.join(","));

const results = [];
let ok = true;
for (const baseUrl of targets) {
  for (const path of paths) {
    const url = `${baseUrl}${path}`;
    const result = await probe(url);
    results.push({
      baseUrl,
      path,
      ...result,
    });
    if (!result.ok) {
      logFailure({ baseUrl, path, ...result });
      ok = false;
    }
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
      contentType: null,
      cacheControl: null,
      xMir2DomainProxy: null,
      xMir2AssetKey: null,
      xMir2AssetVersion: null,
      bodyPreview: null,
      error: error instanceof Error ? error.message : String(error),
    };
  }

  const result = {
    ok: response.ok,
    status: response.status,
    elapsedMs: Date.now() - startedAt,
    contentType: response.headers.get("content-type"),
    cacheControl: response.headers.get("cache-control"),
    xMir2DomainProxy: response.headers.get("x-mir2-domain-proxy"),
    xMir2AssetKey: response.headers.get("x-mir2-asset-key"),
    xMir2AssetVersion: response.headers.get("x-mir2-asset-version"),
    bodyPreview: null,
  };

  if (!result.ok) {
    result.bodyPreview = await readBodyPreview(url, 500);
  }

  return result;
}

async function readBodyPreview(url, maxChars) {
  try {
    const response = await fetch(url, { method: "GET", cache: "no-store" });
    const text = await response.text();
    return text.slice(0, maxChars);
  } catch {
    return null;
  }
}

function logFailure({ baseUrl, path, status, contentType, xMir2DomainProxy, xMir2AssetKey, xMir2AssetVersion, bodyPreview, error }) {
  console.log("");
  const url = `${baseUrl}${path}`;
  console.log(`FAIL: ${url}`);
  console.log(`url: ${url}`);
  console.log(`status: ${String(status)}`);
  console.log(`content-type: ${contentType ?? ""}`);
  console.log(`x-mir2-domain-proxy: ${xMir2DomainProxy ?? ""}`);
  console.log(`x-mir2-asset-key: ${xMir2AssetKey ?? ""}`);
  console.log(`x-mir2-asset-version: ${xMir2AssetVersion ?? ""}`);
  if (error) {
    console.log(`error: ${error}`);
  }
  console.log(`body[0..500]: ${bodyPreview ?? ""}`);
}

function makeRange(start, end) {
  const values = [];
  for (let value = start; value <= end; value += 1) {
    values.push(value);
  }
  return values;
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

function parseTargets(value) {
  const defaultTargets = ["web", "cdn"];
  const requested = String(value || "")
    .split(",")
    .map((target) => target.trim().toLowerCase())
    .filter(Boolean);

  const normalized = requested.length ? requested : defaultTargets;
  const deduped = [];
  for (const target of normalized) {
    if (target !== "web" && target !== "cdn") {
      throw new Error(`Invalid smoke target '${target}'. Expected one of web,cdn`);
    }
    if (!deduped.includes(target)) {
      deduped.push(target);
    }
  }

  return deduped.flatMap((target) => (target === "web" ? [webBaseUrl] : [assetBaseUrl]));
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
