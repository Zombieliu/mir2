const DEFAULT_WEB_BASE_URL = "https://mir2.obelisk.build";
const BEVY_RUNTIME_BACKENDS = [
  {
    label: "webgpu",
    paths: [
      "/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js",
      "/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm",
    ],
  },
  {
    label: "webgl2",
    paths: [
      "/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js",
      "/bevy-runtime/pkg-webgl2/mir2_bevy_runtime_bg.wasm",
    ],
  },
];

const args = parseArgs(process.argv.slice(2));
const webBaseUrl = normalizeBaseUrl(args.webBaseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_WEB_BASE_URL);

const results = [];
let ok = false;

for (const backend of BEVY_RUNTIME_BACKENDS) {
  let backendOk = true;
  for (const path of backend.paths) {
    const result = await probe(`${webBaseUrl}${path}`);
    results.push({ kind: "bevy-runtime", backend: backend.label, path, ...result });
    if (!result.ok) {
      backendOk = false;
      logFailure({ baseUrl: webBaseUrl, path, ...result });
    }
  }

  if (backendOk) {
    ok = true;
  }
}

console.log(
  JSON.stringify(
    {
      ok,
      webBaseUrl,
      results,
    },
    null,
    2,
  ),
);

if (!ok) {
  process.exitCode = 1;
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

function logFailure({ baseUrl, path, status, contentType, xMir2DomainProxy, xMir2AssetKey, xMir2AssetVersion, error, bodyPreview }) {
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

function normalizeBaseUrl(value) {
  return String(value || "").trim().replace(/\/+$/, "");
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
