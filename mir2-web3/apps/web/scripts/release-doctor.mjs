import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_MANIFEST = path.resolve(
  REPO_ROOT,
  "docs",
  "generated",
  "remote-assets",
  "latest-remote-asset-release.json",
);
const DEFAULT_WEB_BASE_URL = "https://mir2.obelisk.build";

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
  ...makeRange(340, 354).map((value) => `/original-ui/Title/${value}.png`),
  ...makeRange(360, 362).map((value) => `/original-ui/Title/${value}.png`),
];
const BEVY_ENTITY_ATLAS_PATHS = [
  "/bevy-entity-atlases/manifest.json",
  "/bevy-entity-atlases/starter-bichon-base.png",
];

const REQUIRED_ASSETS = [
  ...LOGIN_TITLE_PATHS,
  ...LOGIN_CHRSEL_PATHS,
  "/original-ui/Prguse/1084.png",
  "/original-ui/Cursors/Cursor_Default.CUR",
  "/original-ui/Cursors/Cursor_TextPrompt.CUR",
  ...EXTRA_ORIGINAL_ASSET_PATHS,
  ...BEVY_ENTITY_ATLAS_PATHS,
];
const BEVY_RUNTIME_PATHS = [
  "/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js",
  "/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js",
];

const args = parseArgs(process.argv.slice(2));
const manifestPath = path.resolve(args.manifest ?? process.env.MIR2_REMOTE_ASSET_RELEASE_MANIFEST ?? DEFAULT_MANIFEST);
const checkManifest = booleanArg(args.checkManifest ?? process.env.RELEASE_DOCTOR_CHECK_MANIFEST, true);
const checkR2 = booleanArg(args.checkR2 ?? process.env.RELEASE_DOCTOR_CHECK_R2, true);
const checkWorker = booleanArg(args.checkWorker ?? process.env.RELEASE_DOCTOR_CHECK_WORKER, false);
const checkBevyRuntime = booleanArg(args.checkBevyRuntime ?? process.env.RELEASE_DOCTOR_CHECK_BEVY_RUNTIME, true);
const requireFullCrystalPack = booleanArg(
  args.requireFullCrystalPack ?? process.env.RELEASE_DOCTOR_REQUIRE_FULL_CRYSTAL_PACK,
  false,
);
const probeConcurrency = positiveIntegerArg(
  args.probeConcurrency ?? process.env.RELEASE_DOCTOR_PROBE_CONCURRENCY,
  32,
);
const webBaseUrl = normalizeBaseUrl(args.webBaseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_WEB_BASE_URL);
const assetBaseUrlInput = normalizeOptionalUrl(
  args.assetBaseUrl ??
    process.env.ASSET_ORIGIN_URL ??
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ??
    process.env.MIR2_ASSET_BASE_URL ??
    process.env.RELEASE_DOCTOR_ASSET_BASE_URL ??
    "",
);

async function main() {
  const release = JSON.parse(await fs.readFile(manifestPath, "utf8"));
  const releaseVersion = normalizeAssetVersion(release.version ?? "");
  const manifestPathSet = new Set(
    (Array.isArray(release.files) ? release.files : [])
      .map((file) => normalizePath(file?.path ?? file?.p ?? file?.relativePath))
      .filter(Boolean),
  );
  const fullPackPaths = [...manifestPathSet]
    .filter((assetPath) => assetPath.startsWith("/generated/crystal-packs/full/"))
    .sort();
  const fullPackSamples = [
    "/generated/crystal-packs/full/index.json",
    fullPackPaths.find((assetPath) => assetPath.includes("/libraries/") && assetPath.endsWith(".json")),
    fullPackPaths.find((assetPath) => assetPath.includes("/pages/") && assetPath.endsWith(".png")),
  ].filter(Boolean);
  const requiredAssets = requireFullCrystalPack
    ? [...new Set([...REQUIRED_ASSETS, ...fullPackPaths])]
    : REQUIRED_ASSETS;

  let failed = false;
  const report = {
    manifestPath,
    releaseVersion,
    checks: {
      manifest: { ok: true, missing: [], requiredCount: requiredAssets.length },
      r2: { ok: true, results: [] },
      worker: { ok: true, results: [] },
      fullCrystalPack: {
        required: requireFullCrystalPack,
        ok: true,
        fileCount: fullPackPaths.length,
        expectedFileCount: Number(release.fullCrystalPack?.fileCount ?? 0),
        samples: fullPackSamples,
      },
      bevyRuntime: {
        ok: true,
        paths: [],
        message: "",
      },
    },
  };

  if (checkManifest) {
    const missingManifestPaths = requiredAssets.filter((assetPath) => !manifestPathSet.has(assetPath));
    report.checks.manifest.missing = missingManifestPaths;
    if (missingManifestPaths.length > 0) {
      report.checks.manifest.ok = false;
      failed = true;
      console.error(`[release-doctor] manifest missing ${missingManifestPaths.length} required assets`);
      for (const missing of missingManifestPaths) {
        console.error(`- ${missing}`);
      }
    } else {
      console.log("[release-doctor] manifest required assets present.");
    }
  }

  if (requireFullCrystalPack) {
    const fullPack = release.fullCrystalPack ?? {};
    const expectedFileCount = Number(fullPack.fileCount ?? 0);
    const hasAllSampleKinds = fullPackSamples.length === 3;
    if (
      fullPack.enabled !== true ||
      fullPack.verified !== true ||
      !expectedFileCount ||
      fullPackPaths.length !== expectedFileCount ||
      !hasAllSampleKinds
    ) {
      report.checks.fullCrystalPack.ok = false;
      failed = true;
      console.error(
        `[release-doctor] full Crystal pack invalid: enabled=${fullPack.enabled} verified=${fullPack.verified} expectedFiles=${expectedFileCount} manifestFiles=${fullPackPaths.length} sampleKinds=${fullPackSamples.length}/3`,
      );
    } else {
      console.log(
        `[release-doctor] full Crystal pack manifest passed (${fullPack.libraryCount} libraries, ${fullPack.pageCount} pages).`,
      );
    }
  }

  const requiredForR2 = getRequiredAssetUrls({
    assetBaseUrlInput,
    release,
    releaseVersion,
  });

  if (checkR2) {
    if (!requiredForR2.assetBaseUrl) {
      failed = true;
      report.checks.r2.ok = false;
      console.error("[release-doctor] assetBaseUrl is missing. Set ASSET_ORIGIN_URL, NEXT_PUBLIC_MIR2_ASSET_BASE_URL, or release.assetBaseUrl.");
    } else {
      const outcome = await probeAssets(requiredAssets, requiredForR2.assetBaseUrl, "R2");
      report.checks.r2.results = outcome.results;
      report.checks.r2.ok = outcome.ok;
      if (!outcome.ok) failed = true;
    }
    if (report.checks.r2.ok) {
      console.log("[release-doctor] R2 asset presence check passed.");
    }
  }

  if (checkWorker) {
    const outcome = await probeAssets(requiredAssets, webBaseUrl, "worker");
    report.checks.worker.results = outcome.results;
    report.checks.worker.ok = outcome.ok;
    if (!outcome.ok) failed = true;
    if (report.checks.worker.ok) {
      console.log("[release-doctor] worker same-origin smoke passed.");
    }
  }

  if (checkBevyRuntime) {
    let bevyOk = false;
    const checks = [];

    for (const bevyPath of BEVY_RUNTIME_PATHS) {
      const bevyUrl = `${webBaseUrl}${bevyPath}`;
      const bevyResult = await probe(bevyUrl);
      checks.push({ path: bevyPath, ...bevyResult });
      if (bevyResult.ok) {
        bevyOk = true;
      }
    }

    report.checks.bevyRuntime = {
      ok: bevyOk,
      paths: checks,
      status: checks.map((item) => item.status),
      message: bevyOk
        ? "bevy runtime package reachable"
        : "both bevy runtime package paths returned non-2xx",
    };

    if (!bevyOk) {
      failed = true;
      report.checks.bevyRuntime = { ...report.checks.bevyRuntime, ok: false };
      console.error("[release-doctor] separate: bevy runtime package check failed.");
      for (const check of checks) {
        console.error(`- ${check.path}: ${check.error ?? `HTTP ${check.status}`}`);
      }
    } else {
      console.log("[release-doctor] bevy runtime package check passed.");
    }
  }

  console.log(JSON.stringify(report, null, 2));

  if (failed) {
    process.exitCode = 1;
  }
}

async function probeAssets(assetPaths, baseUrl, label) {
  const results = new Array(assetPaths.length);
  let ok = true;
  await runPool(assetPaths, probeConcurrency, async (assetPath, index) => {
    const url = `${baseUrl}/${assetPath.replace(/^\/+/, "")}`;
    const result = await probe(url);
    results[index] = { path: assetPath, ...result };
    if (!result.ok) {
      ok = false;
      console.error(`[release-doctor] ${label} miss: ${url} -> ${result.error ?? `HTTP ${result.status}`}`);
    }
  });
  return { ok, results };
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
  };
}

function getRequiredAssetUrls({ assetBaseUrlInput, release, releaseVersion }) {
  let assetBaseUrl = normalizeOptionalUrl(assetBaseUrlInput || release.assetBaseUrl || "");
  if (assetBaseUrl && assetBaseUrl.includes("{version}")) {
    if (!releaseVersion) {
      return { assetBaseUrl: "", assetObjectPrefix: "" };
    }
    assetBaseUrl = assetBaseUrl.replaceAll("{version}", releaseVersion);
  }
  return { assetBaseUrl, assetObjectPrefix: normalizeObjectPrefix(release.objectPrefix || "") };
}

function normalizeObjectPrefix(value) {
  return String(value || "")
    .trim()
    .replace(/^\/+|\/+$/g, "");
}

async function runPool(items, concurrency, worker) {
  let nextIndex = 0;
  async function next() {
    while (nextIndex < items.length) {
      const index = nextIndex;
      nextIndex += 1;
      await worker(items[index], index);
    }
  }
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length || 1) }, next));
}

function positiveIntegerArg(value, fallback) {
  const number = Number(value ?? fallback);
  if (!Number.isSafeInteger(number) || number <= 0) {
    throw new Error(`Expected a positive integer, received ${value}`);
  }
  return number;
}

function normalizePath(value) {
  const valueAsPath = String(value || "").trim();
  if (!valueAsPath) return "";
  return valueAsPath.startsWith("/") ? valueAsPath : `/${valueAsPath}`;
}

function normalizeBaseUrl(value) {
  return String(value || "")
    .trim()
    .replace(/\/+$/, "");
}

function normalizeOptionalUrl(value) {
  return String(value || "").trim();
}

function normalizeAssetVersion(value) {
  return String(value || "")
    .trim()
    .replace(/[^a-zA-Z0-9._-]/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80);
}

function booleanArg(value, fallback) {
  if (value == null) return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function makeRange(start, end) {
  const values = [];
  for (let value = start; value <= end; value += 1) {
    values.push(value);
  }
  return values;
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

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
