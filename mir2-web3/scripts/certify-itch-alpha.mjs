#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourcePath = resolve(repoRoot, "distribution/itch-html5/index.html");
const buildScript = resolve(repoRoot, "scripts/build-itch-html5.sh");
const archivePath = resolve(
  repoRoot,
  "dist/itch/mir2-platinum-176-web-alpha-html5.zip",
);
const reportPath = resolve(
  repoRoot,
  "docs/generated/player-qa/itch-alpha/latest-launcher.json",
);

const assertions = [];

function assert(name, passed, detail) {
  assertions.push({ name, passed: Boolean(passed), detail });
}

function frameAncestorsAllowsItch(csp) {
  if (!csp) return true;
  const directive = csp
    .split(";")
    .map((part) => part.trim())
    .find((part) => part.toLowerCase().startsWith("frame-ancestors"));
  if (!directive) return true;

  const sources = directive.split(/\s+/).slice(1);
  if (sources.includes("'none'")) return false;
  if (sources.includes("*")) return true;
  return sources.some((source) => {
    const normalized = source.toLowerCase();
    return (
      normalized.includes("itch.io") ||
      normalized.includes("itch.zone") ||
      normalized.includes("html-classic.itch.zone")
    );
  });
}

async function fetchWithTimeout(url, options = {}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 15_000);
  try {
    return await fetch(url, {
      ...options,
      cache: "no-store",
      redirect: "follow",
      signal: controller.signal,
    });
  } finally {
    clearTimeout(timeout);
  }
}

let fatalError = null;

try {
  const html = readFileSync(sourcePath, "utf8");
  const gameUrlMatch = html.match(
    /<meta\s+name="mir2-game-url"\s+content="([^"]+)"/,
  );
  const gameUrl = gameUrlMatch?.[1] ?? "";

  assert("source_index_exists", html.length > 0, `${html.length} bytes`);
  assert(
    "production_url_is_https",
    gameUrl.startsWith("https://"),
    gameUrl || "missing mir2-game-url meta",
  );
  assert(
    "launcher_has_primary_action",
    html.includes('id="launch-game"'),
    "launch-game button",
  );
  assert(
    "launcher_has_new_window_fallback",
    html.includes('id="open-external"'),
    "open-external link",
  );
  assert(
    "launcher_has_fullscreen_control",
    html.includes('id="fullscreen-game"') && html.includes("requestFullscreen"),
    "fullscreen button and request",
  );
  assert(
    "launcher_has_qa_hooks",
    html.includes("window.render_game_to_text") &&
      html.includes("window.advanceTime"),
    "render_game_to_text and advanceTime",
  );

  execFileSync("bash", [buildScript], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe",
  });

  const archiveEntries = execFileSync("unzip", ["-Z1", archivePath], {
    cwd: repoRoot,
    encoding: "utf8",
  })
    .trim()
    .split("\n")
    .filter(Boolean);
  execFileSync("unzip", ["-tq", archivePath], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: "pipe",
  });

  assert(
    "archive_has_root_index_only",
    archiveEntries.length === 1 && archiveEntries[0] === "index.html",
    archiveEntries,
  );
  assert(
    "archive_is_small",
    statSync(archivePath).size < 500 * 1024,
    `${statSync(archivePath).size} bytes`,
  );

  if (gameUrl.startsWith("https://")) {
    const homeResponse = await fetchWithTimeout(gameUrl);
    const xFrameOptions = homeResponse.headers.get("x-frame-options");
    const contentSecurityPolicy = homeResponse.headers.get(
      "content-security-policy",
    );

    assert(
      "production_home_reachable",
      homeResponse.ok,
      `HTTP ${homeResponse.status}`,
    );
    assert(
      "production_allows_cross_origin_frame",
      !xFrameOptions &&
        frameAncestorsAllowsItch(contentSecurityPolicy),
      {
        xFrameOptions,
        contentSecurityPolicy,
      },
    );

    const healthUrl = new URL("/health", gameUrl);
    const healthResponse = await fetchWithTimeout(healthUrl);
    let healthPayload = null;
    try {
      healthPayload = await healthResponse.json();
    } catch {
      healthPayload = null;
    }
    assert(
      "production_health_ok",
      healthResponse.ok && healthPayload?.ok === true,
      {
        status: healthResponse.status,
        payload: healthPayload,
      },
    );
  }
} catch (error) {
  fatalError = error instanceof Error ? error.message : String(error);
}

const failedAssertions = assertions.filter((entry) => !entry.passed);
const report = {
  schema: "mir2-itch-alpha-launcher/1",
  generatedAt: new Date().toISOString(),
  passed: fatalError === null && failedAssertions.length === 0,
  fatalError,
  sourcePath: "distribution/itch-html5/index.html",
  archivePath: "dist/itch/mir2-platinum-176-web-alpha-html5.zip",
  assertions,
  failedAssertions: failedAssertions.map((entry) => entry.name),
  manualGates: [
    "itch sandbox embed and fullscreen",
    "fresh-account world entry",
    "two-client shared-Zone presence",
    "logout and reconnect persistence",
    "platinum_176 v6 Realm handshake",
    "Platinum UI contains no Mail, Game Shop, or On-chain Mine",
  ],
};

mkdirSync(dirname(reportPath), { recursive: true });
writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);

console.log(`itch Alpha launcher certificate: ${report.passed ? "PASS" : "FAIL"}`);
console.log(`report: ${reportPath}`);
for (const entry of assertions) {
  console.log(`${entry.passed ? "PASS" : "FAIL"} ${entry.name}`);
}
if (fatalError) console.error(`fatal: ${fatalError}`);

if (!report.passed) process.exitCode = 1;
