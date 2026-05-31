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

const REQUIRED_ASSETS = [
  ...LOGIN_TITLE_PATHS,
  ...LOGIN_CHRSEL_PATHS,
  "/original-ui/Prguse/1084.png",
  "/original-ui/Cursors/Cursor_Default.CUR",
  "/original-ui/Cursors/Cursor_TextPrompt.CUR",
  ...EXTRA_ORIGINAL_ASSET_PATHS,
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

  let failed = false;
  const report = {
    manifestPath,
    releaseVersion,
    checks: {
      manifest: { ok: true, missing: [], requiredCount: REQUIRED_ASSETS.length },
      r2: { ok: true, results: [] },
      worker: { ok: true, results: [] },
    bevyRuntime: {
      ok: true,
      paths: [],
      message: "",
    },
    },
  };

  if (checkManifest) {
    const missingManifestPaths = REQUIRED_ASSETS.filter((assetPath) => !manifestPathSet.has(assetPath));
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
      for (const required of REQUIRED_ASSETS) {
        const url = `${requiredForR2.assetBaseUrl}/${required.replace(/^\/+/, "")}`;
        const result = await probe(url);
        report.checks.r2.results.push({ path: required, ...result });
        if (!result.ok) {
          report.checks.r2.ok = false;
          failed = true;
          console.error(`[release-doctor] R2 miss: ${url} -> ${result.error ?? `HTTP ${result.status}`}`);
        }
      }
    }
    if (report.checks.r2.ok) {
      console.log("[release-doctor] R2 asset presence check passed.");
    }
  }

  if (checkWorker) {
    for (const required of REQUIRED_ASSETS) {
      const url = `${webBaseUrl}/${required.replace(/^\/+/, "")}`;
      const result = await probe(url);
      report.checks.worker.results.push({ path: required, ...result });
      if (!result.ok) {
        report.checks.worker.ok = false;
        failed = true;
        console.error(`[release-doctor] worker miss: ${url} -> ${result.error ?? `HTTP ${result.status}`}`);
      }
    }
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
