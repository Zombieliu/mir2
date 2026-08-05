import runtimeManifest from "../lib/generated/bevy_runtime_version.json" with { type: "json" };

const DEFAULT_WEB_BASE_URL = "https://mir2.obelisk.build";
const args = parseArgs(process.argv.slice(2));
const requireR2 = booleanArg(args.requireR2 ?? process.env.MIR2_REQUIRE_BEVY_RUNTIME_R2, false);
const runtimeVersion = String(args.runtimeVersion ?? runtimeManifest.version ?? "").trim();
if (!/^bevy-[a-f0-9]{16}$/i.test(runtimeVersion)) {
  throw new Error(`Invalid Bevy runtime version: ${runtimeVersion || "empty"}`);
}
const BEVY_RUNTIME_BACKENDS = [
  {
    label: "webgpu",
    paths: [
      `/bevy-runtime/v/${runtimeVersion}/pkg-webgpu/mir2_bevy_runtime.js`,
      `/bevy-runtime/v/${runtimeVersion}/pkg-webgpu/mir2_bevy_runtime_bg.wasm`,
    ],
  },
  {
    label: "webgl2",
    paths: [
      `/bevy-runtime/v/${runtimeVersion}/pkg-webgl2/mir2_bevy_runtime.js`,
      `/bevy-runtime/v/${runtimeVersion}/pkg-webgl2/mir2_bevy_runtime_bg.wasm`,
    ],
  },
];

const webBaseUrl = normalizeBaseUrl(args.webBaseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_WEB_BASE_URL);

const results = [];
let ok = true;

for (const backend of BEVY_RUNTIME_BACKENDS) {
  let backendOk = true;
  for (const path of backend.paths) {
    const isWasm = path.endsWith(".wasm");
    const result = await probe(`${webBaseUrl}${path}`, isWasm);
    results.push({ kind: "bevy-runtime", backend: backend.label, path, ...result });
    if (
      !result.ok ||
      (requireR2 && result.xMir2DomainProxy !== "r2-asset") ||
      (isWasm &&
        (result.contentEncoding !== "gzip" ||
          result.storageContentEncoding !== "gzip" ||
          result.wasmMagicOk !== true))
    ) {
      backendOk = false;
      logFailure({ baseUrl: webBaseUrl, path, ...result });
    }
  }

  if (!backendOk) ok = false;
}

console.log(
  JSON.stringify(
    {
      ok,
      webBaseUrl,
      requireR2,
      results,
    },
    null,
    2,
  ),
);

if (!ok) {
  process.exitCode = 1;
}

async function probe(url, verifyWasm = false) {
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
      contentEncoding: null,
      storageContentEncoding: null,
      wasmMagicOk: verifyWasm ? false : null,
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
    contentEncoding: response.headers.get("content-encoding"),
    storageContentEncoding: response.headers.get("x-mir2-storage-content-encoding"),
    wasmMagicOk: verifyWasm ? false : null,
    bodyPreview: null,
  };

  if (
    result.ok &&
    verifyWasm &&
    result.contentEncoding === "gzip" &&
    result.storageContentEncoding === "gzip"
  ) {
    result.wasmMagicOk = await probeWasmMagic(url);
  }

  if (!result.ok) {
    result.bodyPreview = await readBodyPreview(url, 500);
  }

  return result;
}

async function probeWasmMagic(url) {
  let response;
  try {
    response = await fetch(url, { method: "GET", cache: "no-store" });
    if (!response.ok || !response.body) return false;
    const reader = response.body.getReader();
    const prefix = [];
    while (prefix.length < 4) {
      const { done, value } = await reader.read();
      if (done) break;
      for (const byte of value) {
        prefix.push(byte);
        if (prefix.length === 4) break;
      }
    }
    await reader.cancel().catch(() => undefined);
    return prefix.length === 4 && prefix[0] === 0 && prefix[1] === 97 && prefix[2] === 115 && prefix[3] === 109;
  } catch {
    return false;
  }
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

function logFailure({
  baseUrl,
  path,
  status,
  contentType,
  contentEncoding,
  storageContentEncoding,
  wasmMagicOk,
  xMir2DomainProxy,
  xMir2AssetKey,
  xMir2AssetVersion,
  error,
  bodyPreview,
}) {
  console.log("");
  const url = `${baseUrl}${path}`;
  console.log(`FAIL: ${url}`);
  console.log(`url: ${url}`);
  console.log(`status: ${String(status)}`);
  console.log(`content-type: ${contentType ?? ""}`);
  console.log(`content-encoding: ${contentEncoding ?? ""}`);
  console.log(`x-mir2-storage-content-encoding: ${storageContentEncoding ?? ""}`);
  if (path.endsWith(".wasm")) {
    console.log(`wasm-magic-ok: ${String(wasmMagicOk)}`);
  }
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

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  return ["1", "true", "yes", "on"].includes(String(value).trim().toLowerCase());
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
