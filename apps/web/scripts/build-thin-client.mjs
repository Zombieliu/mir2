#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultWebRoot = path.resolve(scriptDir, "..");
const args = parseArgs(process.argv.slice(2));
const webRoot = path.resolve(args.webRoot ?? defaultWebRoot);
const nextRoot = path.resolve(webRoot, args.nextDir ?? ".next");
const publicRoot = path.resolve(webRoot, "public");
const outputRoot = path.resolve(webRoot, args.output ?? ".mir2-thin-client");
const reportPath = path.resolve(
  webRoot,
  args.report ?? "../../docs/generated/remote-assets/latest-thin-client-size.json",
);
const skipBuild = booleanArg(args.skipBuild, false);
const reportOnly = booleanArg(args.reportOnly, false);
const budgetBytes = numberArg(args.budgetMb, 360) * 1024 * 1024;
const R2_UI_ROOTS = new Set([
  "AArmour",
  "AHair",
  "ARArmour",
  "ARHair",
  "ARWeapon",
  "AWeapon",
  "CArmour",
  "CHair",
  "ChrSel",
  "CWeapon",
  "Cursors",
  "Items",
  "MMap",
  "MapLinkIcon",
  "Monster",
  "NPC",
  "Prguse",
  "Prguse2",
  "Sound",
  "Title",
]);
const LOCAL_UI_FALLBACKS = new Set([
  "original-ui/Prguse/2092.png",
  "original-ui/Prguse/2094.png",
  "original-ui/Prguse/2095.png",
]);
const LOCAL_DEBUG_RUNTIME_FILES = new Set([
  "debug/map-samples/smtile-72.png",
  "debug/map-samples/smtile-80.png",
]);

assertSafeOutput(webRoot, outputRoot);

if (!skipBuild) {
  run(process.platform === "win32" ? "npm.cmd" : "npm", ["run", "build"], {
    cwd: webRoot,
    env: {
      ...process.env,
      MIR2_NEXT_STANDALONE: "1",
      MIR2_USE_PREBUILT_BEVY_RUNTIME: process.env.MIR2_USE_PREBUILT_BEVY_RUNTIME ?? "1",
      // The deployed R2 release manifest is a delivery index, not the source
      // inventory. Keep the full local manifest authoritative while packaging
      // a thin client so a stale remote release cannot shrink source coverage.
      MIR2_ORIGINAL_ASSET_MANIFEST_MODE:
        process.env.MIR2_ORIGINAL_ASSET_MANIFEST_MODE ?? "filesystem",
    },
  });
}

const sourceStats = {
  public: await collectStats(publicRoot),
  nextTotal: await collectStats(nextRoot),
  nextCache: await collectStats(path.join(nextRoot, "cache")),
  nextDev: await collectStats(path.join(nextRoot, "dev")),
  nextServer: await collectStats(path.join(nextRoot, "server")),
  nextStatic: await collectStats(path.join(nextRoot, "static")),
};

let packageStats = await collectStats(outputRoot);
let serverEntry = null;
let excluded = [];

if (!reportOnly) {
  const standaloneRoot = path.join(nextRoot, "standalone");
  const standaloneStats = await collectStats(standaloneRoot);
  if (!standaloneStats.exists) {
    throw new Error(
      `Missing ${standaloneRoot}. Run npm run build:thin, or rebuild with MIR2_NEXT_STANDALONE=1.`,
    );
  }

  await fs.rm(outputRoot, { recursive: true, force: true });
  await fs.mkdir(outputRoot, { recursive: true });
  await fs.cp(standaloneRoot, outputRoot, { recursive: true, force: true });

  const serverPath = await findServerEntry(outputRoot);
  serverEntry = path.relative(outputRoot, serverPath).split(path.sep).join("/");
  const appRoot = path.dirname(serverPath);
  await copyTree(path.join(nextRoot, "static"), path.join(appRoot, ".next", "static"));
  excluded = await copyThinPublic(publicRoot, path.join(appRoot, "public"));

  await fs.writeFile(
    path.join(outputRoot, "THIN-CLIENT-README.txt"),
    [
      "Mir2 thin client / standalone web runtime",
      "",
      `Start: node ${serverEntry}`,
      "Build-time environment: NEXT_PUBLIC_MIR2_ASSET_BASE_URL and MIR2_ASSET_VERSION",
      "Runtime environment: MIR2_ASSET_BASE_URL and MIR2_R2_PROXY_BASE",
      "Large original UI/entity media remains in the source checkout and is fetched from versioned R2 at runtime.",
      "The packed map atlas remains inside this package; raw map PNGs are used only by the DOM compatibility fallback.",
      "",
    ].join("\n"),
    "utf8",
  );
  packageStats = await collectStats(outputRoot);
}

const runtimeStats = {
  webgpu: await collectStats(path.join(publicRoot, "bevy-runtime", "pkg-webgpu")),
  webgl2: await collectStats(path.join(publicRoot, "bevy-runtime", "pkg-webgl2")),
  removedLegacyMirror: await collectStats(path.join(publicRoot, "bevy-runtime", "pkg")),
};

const report = {
  ok: reportOnly || (packageStats.exists && packageStats.bytes <= budgetBytes),
  generatedAt: new Date().toISOString(),
  mode: reportOnly ? "report-only" : "standalone-thin-client",
  webRoot,
  outputRoot,
  serverEntry,
  budgetBytes,
  sourceStats,
  runtimeStats,
  package: packageStats,
  excluded,
  notes: [
    ".next/cache and .next/dev are compiler caches, not player distribution files.",
    "The source public directory stays complete for deterministic generation and offline development.",
    "A browser downloads only the selected WebGPU or WebGL2 runtime, never both backends.",
    "Original map and allowlisted UI/entity media are fetched from versioned R2 and cached by mir2-asset-worker.js.",
    "Promote this package only after release:doctor and browser smoke pass against the configured immutable R2 prefix.",
  ],
};

await fs.mkdir(path.dirname(reportPath), { recursive: true });
await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
console.log(JSON.stringify(report, null, 2));

if (!report.ok) {
  throw new Error(
    `Thin client is ${formatBytes(packageStats.bytes)}, over the ${formatBytes(budgetBytes)} budget.`,
  );
}

async function copyThinPublic(sourceRoot, destinationRoot) {
  const excludedEntries = [];
  await copyTree(sourceRoot, destinationRoot, (relativePath, entry) => {
    const normalized = relativePath.split(path.sep).join("/");
    const first = normalized.split("/")[0];
    const parts = normalized.split("/");

    if (first === "debug") {
      if (entry.isDirectory()) return true;
      if (LOCAL_DEBUG_RUNTIME_FILES.has(normalized)) return true;
      if (!excludedEntries.some((item) => item.path === "debug")) {
        excludedEntries.push({ path: "debug", reason: "QA-only debug fixtures" });
      }
      return false;
    }

    const remoteUiRoot = first === "original-ui" && R2_UI_ROOTS.has(parts[1]);
    if (remoteUiRoot) {
      if (entry.isDirectory()) return true;
      if (LOCAL_UI_FALLBACKS.has(normalized)) return true;
      const rootPath = `original-ui/${parts[1]}`;
      if (!excludedEntries.some((item) => item.path === rootPath)) {
        excludedEntries.push({ path: rootPath, reason: "versioned R2 UI/media root" });
      }
      return false;
    }

    const exclusion =
      first.startsWith(".")
        ? "temporary or hidden build directory"
        : normalized === "bevy-runtime/pkg" || normalized.startsWith("bevy-runtime/pkg/")
          ? "unused legacy WebGL2 mirror"
          : first === "original-map"
            ? "versioned R2 map media"
            : null;

    if (exclusion && (entry.isDirectory() || entry.isFile())) {
      if (!excludedEntries.some((item) => item.path === normalized)) {
        excludedEntries.push({ path: normalized, reason: exclusion });
      }
      return false;
    }
    return entry.name !== ".DS_Store";
  });
  return excludedEntries;
}

async function copyTree(sourceRoot, destinationRoot, filter = () => true) {
  const sourceStats = await collectStats(sourceRoot);
  if (!sourceStats.exists) return;
  await fs.mkdir(destinationRoot, { recursive: true });
  const entries = await fs.readdir(sourceRoot, { withFileTypes: true });
  for (const entry of entries) {
    const sourcePath = path.join(sourceRoot, entry.name);
    const destinationPath = path.join(destinationRoot, entry.name);
    const relativePath = path.relative(publicRoot, sourcePath);
    if (!filter(relativePath, entry)) continue;
    if (entry.isDirectory()) {
      await copyTree(sourcePath, destinationPath, filter);
    } else if (entry.isFile()) {
      await fs.copyFile(sourcePath, destinationPath);
    }
  }
}

async function findServerEntry(root) {
  const candidates = [];
  async function visit(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      const candidate = path.join(directory, entry.name);
      if (entry.isDirectory()) await visit(candidate);
      else if (entry.isFile() && entry.name === "server.js") candidates.push(candidate);
    }
  }
  await visit(root);
  const preferred =
    candidates.find((candidate) => path.relative(root, candidate) === "server.js") ??
    candidates.find((candidate) =>
      candidate.split(path.sep).slice(-3).join("/").endsWith("apps/web/server.js"),
    );
  const selected = preferred ?? candidates[0];
  if (!selected) throw new Error(`No standalone server.js found under ${root}`);
  return selected;
}

async function collectStats(targetPath) {
  try {
    const stat = await fs.stat(targetPath);
    if (stat.isFile()) return { exists: true, bytes: stat.size, files: 1, directories: 0 };
    if (!stat.isDirectory()) return { exists: true, bytes: 0, files: 0, directories: 0 };
  } catch (error) {
    if (error?.code === "ENOENT") return { exists: false, bytes: 0, files: 0, directories: 0 };
    throw error;
  }

  let bytes = 0;
  let files = 0;
  let directories = 1;
  for (const entry of await fs.readdir(targetPath, { withFileTypes: true })) {
    const child = await collectStats(path.join(targetPath, entry.name));
    bytes += child.bytes;
    files += child.files;
    directories += child.directories;
  }
  return { exists: true, bytes, files, directories };
}

function run(command, commandArgs, options) {
  const result = spawnSync(command, commandArgs, { ...options, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} ${commandArgs.join(" ")} exited ${result.status}`);
}

function assertSafeOutput(root, candidate) {
  const relative = path.relative(root, candidate);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Refusing to replace output outside web root: ${candidate}`);
  }
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const flag = values[index];
    if (!flag.startsWith("--")) throw new Error(`Unknown argument: ${flag}`);
    const key = flag.slice(2);
    const value = values[index + 1];
    if (!value || value.startsWith("--")) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = value;
    index += 1;
  }
  return parsed;
}

function booleanArg(value, fallback) {
  if (value == null) return fallback;
  if (typeof value === "boolean") return value;
  if (["1", "true", "yes", "on"].includes(String(value).toLowerCase())) return true;
  if (["0", "false", "no", "off"].includes(String(value).toLowerCase())) return false;
  throw new Error(`Invalid boolean: ${value}`);
}

function numberArg(value, fallback) {
  const numeric = Number(value);
  return Number.isFinite(numeric) && numeric > 0 ? numeric : fallback;
}

function formatBytes(value) {
  return `${(value / 1024 / 1024).toFixed(1)} MiB`;
}
