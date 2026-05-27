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

const REQUIRED_ASSETS = [
  "/original-ui/Title/32.png",
  "/original-ui/Title/30.png",
  "/original-ui/ChrSel/0.png",
  "/original-ui/Cursors/Cursor_Default.CUR",
  "/original-ui/Cursors/Cursor_TextPrompt.CUR",
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
      .map((file) => normalizePath(file?.path))
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
      bevyRuntime: { status: null, message: "" },
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

  if (checkBevyRuntime && (checkR2 || checkWorker)) {
    const bevyUrl = `${webBaseUrl}/mir2_bevy_runtime.js`;
    const bevyResult = await probe(bevyUrl);
    report.checks.bevyRuntime = {
      status: bevyResult.status,
      ok: bevyResult.ok,
      error: bevyResult.error ?? null,
      elapsedMs: bevyResult.elapsedMs,
    };
    if (!bevyResult.ok && bevyResult.status === 404) {
      console.warn(`[release-doctor] separate: mir2_bevy_runtime.js 404 (tracked independently from asset checks).`);
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
