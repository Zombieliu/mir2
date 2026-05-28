const DEFAULT_WEB_BASE_URL = "https://mir2.obelisk.build";
const REQUIRED_ORIGINAL_ASSET_PATHS = [
  "/original-ui/Title/320.png",
  "/original-ui/Prguse/1084.png",
  "/original-ui/Cursors/Cursor_Default.CUR",
  "/original-ui/Title/32.png",
  "/original-ui/Title/30.png",
  "/original-ui/ChrSel/0.png",
  "/original-ui/Cursors/Cursor_TextPrompt.CUR",
];
const BEVY_RUNTIME_PATHS = [
  "/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js",
  "/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js",
];

const args = parseArgs(process.argv.slice(2));
const webBaseUrl = normalizeBaseUrl(args.webBaseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_WEB_BASE_URL);

const results = [];
let ok = true;

for (const path of REQUIRED_ORIGINAL_ASSET_PATHS) {
  const result = await probe(`${webBaseUrl}${path}`);
  results.push({ kind: "original-asset", path, ...result });
  if (!result.ok) ok = false;
}

const runtimeChecks = [];
let runtimeOk = false;
for (const path of BEVY_RUNTIME_PATHS) {
  const result = await probe(`${webBaseUrl}${path}`);
  runtimeChecks.push({ kind: "bevy-runtime", path, ...result });
  if (result.ok) runtimeOk = true;
}
if (!runtimeOk) ok = false;

console.log(
  JSON.stringify(
    {
      ok,
      webBaseUrl,
      results,
      runtimeChecks,
      runtimeOk,
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
      error: error instanceof Error ? error.message : String(error),
    };
  }

  return {
    ok: response.ok,
    status: response.status,
    elapsedMs: Date.now() - startedAt,
    contentType: response.headers.get("content-type"),
    cacheControl: response.headers.get("cache-control"),
  };
}

function normalizeBaseUrl(value) {
  return String(value || "").trim().replace(/\/+$/, "");
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) continue;
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
