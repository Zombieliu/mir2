#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readImmutableBevyRuntimeRelease } from "./lib/bevy-runtime-release-files.mjs";
import { readClientCoreRelease, assertCompiledClientCoreManifest } from "./lib/client-core-release-files.mjs";
import { assertCompiledBevyRuntimeManifest, resolveNextBuildDistDirectory } from "./lib/bevy-runtime-build-identity.mjs";
import {
  auditPortableTree, copyPortableStandalone, copySelectedPublic, copyVerifiedRequiredServerFiles,
  materializeStandaloneDependencies,
  selectThinPublicEntry, selectMapAtlasClosure,
} from "./lib/portable-next-standalone.mjs";

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

const pinnedRuntime = readImmutableBevyRuntimeRelease({ webRoot });
const runtimeManifestRaw = pinnedRuntime.manifest;
const runtimeManifest = pinnedRuntime.normalized;
const sourceRequiredServerFilesPath = path.join(nextRoot, "required-server-files.json");
const sourceRequiredServerFilesBytes = await fs.readFile(sourceRequiredServerFilesPath);
const sourceRequiredServerFiles = JSON.parse(sourceRequiredServerFilesBytes.toString("utf8"));
const compiledRuntimeManifest = assertCompiledBevyRuntimeManifest(sourceRequiredServerFiles, runtimeManifestRaw);
const pinnedCore = readClientCoreRelease({ webRoot });
const compiledCoreManifest = assertCompiledClientCoreManifest(sourceRequiredServerFiles, pinnedCore.manifest);
const coreFilePaths = new Set(pinnedCore.files.map((entry) => entry.relativePath));
const compiledEnv = sourceRequiredServerFiles.config?.env ?? {};
const mapAtlasClosure = await selectMapAtlasClosure(publicRoot, {
  pinnedManifestPath: compiledEnv.MIR2_PINNED_MAP_ATLAS_MANIFEST_PATH ?? "",
  pinnedContentHash: compiledEnv.MIR2_PINNED_MAP_ATLAS_CONTENT_HASH ?? "",
  pinnedEnabled: compiledEnv.MIR2_PINNED_MAP_ATLAS_ENABLED === "1",
});
const runtimeReleasePrefix = `bevy-runtime-releases/${runtimeManifest.version}`;
const runtimeFilePaths = new Set(runtimeManifest.files.map((entry) =>
  entry.path.replace(/^public\/bevy-runtime/, runtimeReleasePrefix)));
runtimeFilePaths.add(`${runtimeReleasePrefix}/runtime-manifest.json`);
const publicSelection = (relative, isDirectory) => selectThinPublicEntry(relative, isDirectory, {
  runtimeVersion: runtimeManifest.version, runtimeFiles: runtimeFilePaths,
  mapAtlasFiles: mapAtlasClosure.files,
  coreVersion: pinnedCore.manifest.version, coreFiles: coreFilePaths,
});

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
let copyStats = null;
let dependencyStats = null;
let publicCopyStats = null;
let jsonCompaction = null;
let packagedCoreClosure = null;

if (!reportOnly) {
  const standaloneRoot = path.join(nextRoot, "standalone");
  const standaloneStats = await collectStats(standaloneRoot);
  if (!standaloneStats.exists) {
    throw new Error(
      `Missing ${standaloneRoot}. Run npm run build:thin, or rebuild with MIR2_NEXT_STANDALONE=1.`,
    );
  }

  const sourceServerPath = await findServerEntry(standaloneRoot);
  const sourceAppRoot = path.dirname(sourceServerPath);
  serverEntry = path.relative(standaloneRoot, sourceServerPath).split(path.sep).join("/");
  const appRelativePath = path.relative(standaloneRoot, sourceAppRoot).split(path.sep).join("/");
  copyStats = await copyPortableStandalone({
    sourceRoot: standaloneRoot, destinationRoot: outputRoot, appRelativePath,
    publicSelection,
  });
  const appRoot = path.join(outputRoot, appRelativePath);
  const copiedNextRoot = resolveNextBuildDistDirectory(sourceRequiredServerFiles, appRoot);
  const copiedRequiredServerFilesPath = path.join(copiedNextRoot, "required-server-files.json");
  const copiedRequiredServerFiles = await copyVerifiedRequiredServerFiles({
    sourcePath: sourceRequiredServerFilesPath, destinationPath: copiedRequiredServerFilesPath,
    sourceBytes: sourceRequiredServerFilesBytes,
  });
  if (resolveNextBuildDistDirectory(copiedRequiredServerFiles, appRoot) !== copiedNextRoot) {
    throw new Error("Copied Next distDir differs from the source build");
  }
  assertCompiledBevyRuntimeManifest(
    copiedRequiredServerFiles, runtimeManifestRaw,
  );
  assertCompiledClientCoreManifest(copiedRequiredServerFiles, pinnedCore.manifest);
  const sourceNextDirectory = resolveNextBuildDistDirectory(sourceRequiredServerFiles, sourceAppRoot);
  dependencyStats = await materializeStandaloneDependencies({
    sourceAppRoot, destinationAppRoot: appRoot, sourceStandaloneRoot: standaloneRoot,
    sourceDependencyRoot: path.join(sourceAppRoot, "node_modules"),
    distDir: path.relative(sourceAppRoot, sourceNextDirectory),
  });
  await copyTree(path.join(nextRoot, "static"), path.join(copiedNextRoot, "static"));
  publicCopyStats = await copySelectedPublic({
    sourceRoot: publicRoot, destinationRoot: path.join(appRoot, "public"), selection: publicSelection,
  });
  const packagedCore = readClientCoreRelease({ webRoot: appRoot,
    manifest: compiledCoreManifest, requireExactClosure: true });
  // Reject a Core publication that raced the copy; the compiled closure must stay exact.
  assertCompiledClientCoreManifest(sourceRequiredServerFiles, readClientCoreRelease({ webRoot }).manifest);
  packagedCoreClosure = { version: packagedCore.manifest.version, abiVersion: packagedCore.manifest.abiVersion,
    activeVersions: packagedCore.versionDirectories.map((directory) => path.basename(directory)),
    files: packagedCore.files.map(({ relativePath, bytes, sha256 }) => ({ relativePath, bytes, sha256 })),
    exactActiveBundleClosure: true, leavesPerBundle: 2 };
  const compactionDetails = new Map();
  for (const detail of [...copyStats.jsonCompactions, ...publicCopyStats.jsonCompactions]) {
    compactionDetails.set(detail.path, detail);
  }
  const details = [...compactionDetails.values()].sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  const detailsPath = path.join(path.dirname(reportPath),
    `${path.basename(reportPath, path.extname(reportPath))}.json-compaction-details.json`);
  await fs.mkdir(path.dirname(detailsPath), { recursive: true });
  await fs.writeFile(detailsPath, `${JSON.stringify(details, null, 2)}\n`, "utf8");
  const sourceBytes = details.reduce((sum, entry) => sum + entry.sourceBytes, 0);
  const outputBytes = details.reduce((sum, entry) => sum + entry.outputBytes, 0);
  jsonCompaction = { files: details.length, sourceBytes, outputBytes,
    savedBytes: sourceBytes - outputBytes, detailsPath };
  delete copyStats.jsonCompactions;
  delete publicCopyStats.jsonCompactions;
  excluded = Object.entries(copyStats.skipped).map(([reason, entries]) => ({ reason, entries }));
  await fs.mkdir(path.join(appRoot, "public", "bevy-runtime"), { recursive: true });
  await fs.writeFile(path.join(appRoot, "public", "bevy-runtime", "runtime-manifest.json"), JSON.stringify(runtimeManifestRaw, null, 2) + "\n");
  const packagedRuntime = readImmutableBevyRuntimeRelease({
    webRoot: appRoot, manifestPath: path.join(appRoot, "public", "bevy-runtime", "runtime-manifest.json"),
  });
  if (packagedRuntime.normalized.version !== runtimeManifest.version) throw new Error("Packaged runtime version changed during copy");

  await fs.writeFile(
    path.join(outputRoot, "THIN-CLIENT-README.txt"),
    [
      "Mir2 thin client / standalone web runtime",
      "",
      `Start: node ${serverEntry}`,
      "Build-time environment: NEXT_PUBLIC_MIR2_ASSET_BASE_URL and MIR2_ASSET_VERSION",
      "Runtime environment: MIR2_R2_PROXY_BASE serves same-origin original-ui misses; MIR2_ASSET_BASE_URL separately describes asset-manifest/browser delivery.",
      "Original-ui Mount, Pet and Gate media is omitted; configure MIR2_R2_PROXY_BASE to serve every omitted path from an immutable asset origin.",
      "The audited source original-asset-manifest has Mount entries but no Pet or Gate entries; that manifest alone is not a complete remote inventory.",
      "This package does not prove riding, pet or door gameplay, public release coverage or device acceptance.",
      "The packed map atlas remains inside this package; raw map PNGs are used only by the DOM compatibility fallback.",
      "",
    ].join("\n"),
    "utf8",
  );
  packageStats = await auditPortableTree(outputRoot);
}

const runtimeStats = {
  ...Object.fromEntries(await Promise.all(runtimeManifest.packages.map(async (entry) => [
    entry.id, await collectStats(path.join(pinnedRuntime.versionDirectory, entry.packageDir)),
  ]))),
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
  copyStats,
  dependencyStats,
  publicCopyStats,
  mapAtlasSelection: { manifests: mapAtlasClosure.manifests, pages: mapAtlasClosure.pages,
    selectedPaths: mapAtlasClosure.files.size },
  jsonCompaction,
  coreSelection: { compiledManifest: compiledCoreManifest, selectedPaths: [...coreFilePaths],
    packagedClosure: packagedCoreClosure },
  remoteOriginalUiMediaRootsAdded: ["Mount", "Pet", "Gate"],
  remoteAssetOriginRequired: "Configure MIR2_R2_PROXY_BASE to serve omitted original-ui media paths through the same-origin miss proxy from an immutable asset origin.",
  notes: [
    ".next/cache and .next/dev are compiler caches, not player distribution files.",
    "The source public directory stays complete for deterministic generation and offline development.",
    "A browser downloads only the selected WebGPU or WebGL2 runtime, never both backends.",
    "Original map and allowlisted UI/entity media, including Mount, Pet and Gate non-JSON files, require MIR2_R2_PROXY_BASE for same-origin misses and are cached by mir2-asset-worker.js.",
    "The audited source original-asset-manifest has Mount entries but no Pet or Gate entries; it is not a complete remote inventory and public release coverage is unverified.",
    "Selected original-ui JSON and exact root original-asset-manifest.generated.json retain parsed data and token spelling after compaction; output physical sizes change the deterministic asset-version namespace.",
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

async function copyTree(sourceRoot, destinationRoot, filter = () => true) {
  try {
    if ((await fs.lstat(sourceRoot)).isSymbolicLink()) throw new Error(`Refusing linked static tree: ${sourceRoot}`);
  } catch (error) {
    if (error?.code === "ENOENT") return;
    throw error;
  }
  const sourceStats = await collectStats(sourceRoot);
  if (!sourceStats.exists) return;
  await fs.mkdir(destinationRoot, { recursive: true });
  const entries = await fs.readdir(sourceRoot, { withFileTypes: true });
  for (const entry of entries) {
    const sourcePath = path.join(sourceRoot, entry.name);
    const destinationPath = path.join(destinationRoot, entry.name);
    const relativePath = path.relative(publicRoot, sourcePath);
    if (!filter(relativePath, entry)) continue;
    if (entry.isSymbolicLink()) throw new Error(`Refusing to copy a linked static entry: ${sourcePath}`);
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
      if (entry.isSymbolicLink() || entry.name === "public" || entry.name === "node_modules") continue;
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
    const stat = await fs.lstat(targetPath);
    if (stat.isSymbolicLink()) return { exists: true, bytes: 0, files: 0, directories: 0, links: 1 };
    if (stat.isFile()) return { exists: true, bytes: stat.size, files: 1, directories: 0, links: 0 };
    if (!stat.isDirectory()) return { exists: true, bytes: 0, files: 0, directories: 0, links: 0 };
  } catch (error) {
    if (error?.code === "ENOENT") return { exists: false, bytes: 0, files: 0, directories: 0, links: 0 };
    throw error;
  }

  let bytes = 0;
  let files = 0;
  let directories = 1;
  let links = 0;
  for (const entry of await fs.readdir(targetPath, { withFileTypes: true })) {
    const child = await collectStats(path.join(targetPath, entry.name));
    bytes += child.bytes;
    files += child.files;
    directories += child.directories;
    links += child.links;
  }
  return { exists: true, bytes, files, directories, links };
}

function run(command, commandArgs, options) {
  const result = spawnSync(command, commandArgs, { ...options, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} ${commandArgs.join(" ")} exited ${result.status}`);
}

function assertSafeOutput(root, candidate) {
  const relative = path.relative(root, candidate);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative) || path.dirname(candidate) !== root) {
    throw new Error(`Portable output must be a new direct child of web root: ${candidate}`);
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
