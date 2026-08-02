#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const profiles = {
  dev: { cargoProfileDir: "debug", cargoFlags: [] },
  release: { cargoProfileDir: "wasm-release", cargoFlags: ["--profile", "wasm-release"] },
};
const requestedArgs = process.argv.slice(2);
const selfCheckMode = requestedArgs.some((arg) => arg === "--self-check" || arg === "self-check");
const profileArgs = requestedArgs.filter((arg) => Object.hasOwn(profiles, arg));
const unsupportedArgs = requestedArgs.filter(
  (arg) => !Object.hasOwn(profiles, arg) && arg !== "--self-check" && arg !== "self-check",
);
const profile = profileArgs[0] ?? "release";
const cliError = unsupportedArgs.length > 0
  ? `Unsupported arguments: ${unsupportedArgs.join(" ")}`
  : profileArgs.length > 1
    ? `Specify only one profile: ${profileArgs.join(" ")}`
    : null;

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const runtimeDir = path.resolve(webRoot, "..", "game-client", "runtime");
const pkgParentDir = path.resolve(webRoot, "public", "bevy-runtime");
const runtimePackages = [
  {
    backend: "webgpu",
    packageDirName: "pkg-webgpu",
    cargoFeatureFlags: ["--no-default-features", "--features", "webgpu"],
  },
  {
    backend: "webgl2",
    packageDirName: "pkg-webgl2",
    cargoFeatureFlags: ["--no-default-features", "--features", "webgl2"],
  },
];
const runtimeVersionPath = path.resolve(webRoot, "lib", "generated", "bevy_runtime_version.json");
const buildNonce = `${process.pid}-${Date.now()}-${crypto.randomBytes(6).toString("hex")}`;
const buildLockDir = path.join(path.dirname(pkgParentDir), ".bevy-runtime-build.lock");
const cargoTargetRootDir = resolveCargoTargetRoot(runtimeDir, process.env);
const publishLayout = createPublishLayout(pkgParentDir, runtimeVersionPath, buildNonce);

const cargoBin = process.env.CARGO_BIN || "cargo";
const rustupBin = process.env.RUSTUP_BIN || "rustup";
const wasmBindgenBin = process.env.WASM_BINDGEN_BIN || "wasm-bindgen";
const wasmBindgenRustMinStack = resolveWasmBindgenRustMinStack(process.env);

try {
  if (cliError) {
    fail(cliError);
  } else if (selfCheckMode) {
    runSelfCheck();
  } else {
    runBuild();
  }
} catch (error) {
  console.error(`[bevy-runtime] ${errorMessage(error)}`);
  process.exitCode = 1;
}

function runBuild() {
  if (!profiles[profile]) {
    fail(`Unsupported profile: ${profile}. Use "dev" or "release".`);
  }

  let buildLock = null;
  try {
    buildLock = acquireBuildLock(buildLockDir, {
      owner: createLockOwner(buildNonce),
      onStaleLock: recoverStaleBuild,
    });
    console.log(`[bevy-runtime] lock=${buildLockDir} pid=${process.pid}`);

    if (process.env.MIR2_USE_PREBUILT_BEVY_RUNTIME === "1") {
      usePrebuiltRuntime();
      return;
    }

    buildRuntimeFromSource();
  } finally {
    try {
      cleanupBuildStaging(publishLayout);
    } finally {
      releaseBuildLock(buildLock);
    }
  }
}

function buildRuntimeFromSource() {
  const expectedWasmBindgenVersion = readExpectedWasmBindgenVersion(runtimeDir);
  const pinnedToolchain = readPinnedRustToolchain(runtimeDir);

  console.log(`[bevy-runtime] profile=${profile}`);
  console.log(`[bevy-runtime] runtime=${runtimeDir}`);
  console.log(`[bevy-runtime] output=${pkgParentDir}`);
  console.log(`[bevy-runtime] staging=${publishLayout.stagingParentDir}`);
  console.log(`[bevy-runtime] cargo-target-root=${cargoTargetRootDir}`);
  console.log(`[bevy-runtime] wasm-bindgen-rust-min-stack=${wasmBindgenRustMinStack}`);

  ensureCargoVersion(cargoBin, pinnedToolchain);
  ensureCommand(wasmBindgenBin, ["--version"], "wasm-bindgen", [
    installWasmBindgenHint(expectedWasmBindgenVersion),
    "Or set WASM_BINDGEN_BIN to a wasm-bindgen executable.",
  ]);
  ensureWasmBindgenVersion(wasmBindgenBin, expectedWasmBindgenVersion);
  ensureWasmTarget();

  fs.mkdirSync(publishLayout.stagedPkgParentDir, { recursive: true });
  for (const runtimePackage of runtimePackages) {
    buildRuntimePackage(runtimePackage, publishLayout.stagedPkgParentDir);
  }
  writeRuntimeVersionManifest({
    sourcePkgParentDir: publishLayout.stagedPkgParentDir,
    outputPath: publishLayout.stagedManifestPath,
    publishedPkgParentDir: pkgParentDir,
    manifestRootDir: webRoot,
  });
  validateRuntimeBundle({
    sourcePkgParentDir: publishLayout.stagedPkgParentDir,
    manifestPath: publishLayout.stagedManifestPath,
    publishedPkgParentDir: pkgParentDir,
    manifestRootDir: webRoot,
  });

  publishStagedArtifacts(publishLayout);
  console.log(`[bevy-runtime] wrote ${pkgParentDir}`);
}

function usePrebuiltRuntime() {
  ensurePrebuiltRuntime();
  fs.mkdirSync(publishLayout.stagingParentDir, { recursive: true });
  const manifest = writeRuntimeVersionManifest({
    sourcePkgParentDir: pkgParentDir,
    outputPath: publishLayout.stagedManifestPath,
    publishedPkgParentDir: pkgParentDir,
    manifestRootDir: webRoot,
  });
  validateRuntimeBundle({
    sourcePkgParentDir: pkgParentDir,
    manifestPath: publishLayout.stagedManifestPath,
    publishedPkgParentDir: pkgParentDir,
    manifestRootDir: webRoot,
  });
  publishManifestAtomically(
    publishLayout.stagedManifestPath,
    runtimeVersionPath,
    publishLayout.manifestTempPath,
  );
  console.log(`[bevy-runtime] version=${manifest.version}`);
  console.log(`[bevy-runtime] using prebuilt packages at ${pkgParentDir}`);
}

function buildRuntimePackage(runtimePackage, stagedPkgParentDir) {
  const packageDir = path.join(stagedPkgParentDir, runtimePackage.packageDirName);
  const targetDir = cargoTargetDirForBackend(cargoTargetRootDir, runtimePackage.backend);
  const wasmFile = cargoWasmArtifactPath(targetDir, profile);
  const cargoBuildEnv = buildCargoEnv(profile, targetDir);

  console.log(`[bevy-runtime] backend=${runtimePackage.backend}`);
  console.log(`[bevy-runtime] backend-target=${targetDir}`);
  console.log(`[bevy-runtime] backend-wasm=${wasmFile}`);
  if (process.platform === "win32") {
    console.log(
      `[bevy-runtime] cargo-defaults jobs=${cargoBuildEnv.CARGO_BUILD_JOBS} incremental=${cargoBuildEnv.CARGO_INCREMENTAL}`,
    );
  }

  runCargoBuildWithRetries(
    cargoBin,
    [
      "build",
      "--target",
      "wasm32-unknown-unknown",
      ...profiles[profile].cargoFlags,
      ...runtimePackage.cargoFeatureFlags,
    ],
    {
      cwd: runtimeDir,
      label: `cargo build (${runtimePackage.backend})`,
      env: cargoBuildEnv,
    },
  );

  if (!isNonEmptyFile(wasmFile)) {
    fail(`Cargo build finished but the expected ${runtimePackage.backend} WASM file was not found: ${wasmFile}`);
  }

  fs.mkdirSync(packageDir, { recursive: true });
  runWasmBindgenWithRetries(
    wasmBindgenBin,
    [
      "--target",
      "web",
      "--out-dir",
      packageDir,
      "--out-name",
      "mir2_bevy_runtime",
      wasmFile,
    ],
    {
      cwd: runtimeDir,
      env: { ...process.env, RUST_MIN_STACK: wasmBindgenRustMinStack },
      label: `wasm-bindgen (${runtimePackage.backend})`,
      resetOutputDir: packageDir,
      validateWasmPath: path.join(packageDir, "mir2_bevy_runtime_bg.wasm"),
    },
  );
}

function runtimeOutputFiles(packageDir) {
  return [
    path.join(packageDir, "mir2_bevy_runtime.js"),
    path.join(packageDir, "mir2_bevy_runtime_bg.wasm"),
  ];
}

function runtimeArtifactRecords(sourcePkgParentDir, publishedPkgParentDir, manifestRootDir) {
  return runtimePackages.flatMap(({ backend, packageDirName }) => {
    const sourceFiles = runtimeOutputFiles(path.join(sourcePkgParentDir, packageDirName));
    const publishedFiles = runtimeOutputFiles(path.join(publishedPkgParentDir, packageDirName));
    return sourceFiles.map((sourcePath, index) => ({
      backend,
      sourcePath,
      manifestPath: path.relative(manifestRootDir, publishedFiles[index]).split(path.sep).join("/"),
    }));
  });
}

function createRuntimeVersionManifest(records) {
  const files = records.map(({ sourcePath, manifestPath }) => {
    const bytes = fs.readFileSync(sourcePath);
    return {
      path: manifestPath,
      sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    };
  });

  const combined = crypto.createHash("sha256");
  for (const file of files) {
    combined.update(file.path);
    combined.update("\0");
    combined.update(file.sha256);
    combined.update("\0");
  }

  return {
    version: `bevy-${combined.digest("hex").slice(0, 16)}`,
    files,
  };
}

function writeRuntimeVersionManifest({
  sourcePkgParentDir,
  outputPath,
  publishedPkgParentDir,
  manifestRootDir,
}) {
  const records = runtimeArtifactRecords(sourcePkgParentDir, publishedPkgParentDir, manifestRootDir);
  const manifest = createRuntimeVersionManifest(records);
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, `${JSON.stringify(manifest, null, 2)}\n`);
  return manifest;
}

function validateRuntimeBundle({
  sourcePkgParentDir,
  manifestPath,
  publishedPkgParentDir,
  manifestRootDir,
}) {
  const records = runtimeArtifactRecords(sourcePkgParentDir, publishedPkgParentDir, manifestRootDir);
  for (const record of records) {
    if (!isNonEmptyFile(record.sourcePath)) {
      fail(`Generated ${record.backend} runtime file is missing or empty: ${record.sourcePath}`);
    }
    if (record.sourcePath.endsWith(".wasm")) {
      validateGeneratedWasm(record.sourcePath, record.backend);
    }
  }

  if (!isNonEmptyFile(manifestPath)) {
    fail(`Generated runtime version manifest is missing or empty: ${manifestPath}`);
  }

  let actualManifest;
  try {
    actualManifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  } catch (error) {
    fail(`Generated runtime version manifest is invalid JSON: ${manifestPath}\n${errorMessage(error)}`);
  }

  const expectedManifest = createRuntimeVersionManifest(records);
  if (JSON.stringify(actualManifest) !== JSON.stringify(expectedManifest)) {
    fail(`Generated runtime version manifest does not match the staged runtime files: ${manifestPath}`);
  }

  console.log(`[bevy-runtime] validated version=${actualManifest.version}`);
  return actualManifest;
}

function validateGeneratedWasm(wasmPath, backend) {
  const validate = globalThis.WebAssembly?.validate;
  if (typeof validate !== "function") {
    fail("This Node.js runtime does not provide WebAssembly.validate().");
  }

  const bytes = fs.readFileSync(wasmPath);
  if (!validate(bytes)) {
    fail(`Generated ${backend} WASM failed WebAssembly.validate(): ${wasmPath}`);
  }
}

function ensurePrebuiltRuntime() {
  const records = runtimeArtifactRecords(pkgParentDir, pkgParentDir, webRoot);
  const missing = records.map(({ sourcePath }) => sourcePath).filter((filePath) => !isNonEmptyFile(filePath));
  if (missing.length > 0) {
    fail(`MIR2_USE_PREBUILT_BEVY_RUNTIME=1 but required files are missing: ${missing.join(", ")}`);
  }
}

function resolveCargoTargetRoot(root, env) {
  const configuredRoot = env.MIR2_BEVY_CARGO_TARGET_ROOT || env.CARGO_TARGET_DIR;
  const targetBase = configuredRoot ? path.resolve(root, configuredRoot) : path.join(root, "target");
  return path.join(targetBase, "mir2-bevy-runtime");
}

function cargoTargetDirForBackend(targetRoot, backend) {
  if (!runtimePackages.some((runtimePackage) => runtimePackage.backend === backend)) {
    fail(`Unsupported Bevy runtime backend target: ${backend}`);
  }
  return path.join(targetRoot, backend);
}

function cargoWasmArtifactPath(targetDir, selectedProfile) {
  return path.join(
    targetDir,
    "wasm32-unknown-unknown",
    profiles[selectedProfile].cargoProfileDir,
    "mir2_bevy_runtime.wasm",
  );
}

function buildCargoEnv(selectedProfile, targetDir, platform = process.platform, sourceEnv = process.env) {
  const env = { ...sourceEnv, CARGO_TARGET_DIR: targetDir };
  if (platform !== "win32") {
    return env;
  }

  // Bevy's wasm dependency graph is most reliable on Windows with
  // one non-incremental Cargo job. Dev also strips debug data from dependencies.
  env.CARGO_BUILD_JOBS ??= "1";
  env.CARGO_INCREMENTAL ??= "0";
  if (selectedProfile === "dev") {
    env.CARGO_PROFILE_DEV_DEBUG ??= "0";
    env.CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG ??= "0";
  }
  return env;
}

function readPinnedRustToolchain(root) {
  const toolchainPath = path.join(root, "rust-toolchain.toml");
  if (!fs.existsSync(toolchainPath)) {
    fail(`Missing pinned Rust toolchain file: ${toolchainPath}`);
  }

  const contents = fs.readFileSync(toolchainPath, "utf8");
  const channel = contents.match(/^\s*channel\s*=\s*["']([^"']+)["']/m)?.[1];
  const version = channel?.match(/^(\d+\.\d+\.\d+)$/)?.[1];
  if (!version) {
    fail(`Expected an exact Rust version in ${toolchainPath}, found: ${channel ?? "<missing channel>"}`);
  }

  return { path: toolchainPath, channel, version };
}

function ensureCargoVersion(command, pinnedToolchain) {
  const result = capture(command, ["--version"], { cwd: runtimeDir });
  if (result.error?.code === "ENOENT") {
    fail(
      [
        `Missing cargo command: ${command}`,
        "Install Rust from https://rustup.rs/ or set CARGO_BIN to a cargo executable.",
      ].join("\n"),
    );
  }
  if (result.status !== 0) {
    fail(
      [
        `Unable to run cargo: ${command} --version`,
        outputOf(result),
        "Install Rust from https://rustup.rs/ or set CARGO_BIN to a cargo executable.",
      ].join("\n"),
    );
  }

  const cargoOutput = outputOf(result);
  const actualVersion = cargoOutput.match(/\bcargo\s+(\d+\.\d+\.\d+)\b/)?.[1];
  if (!actualVersion) {
    fail(`Unable to parse Cargo version from: ${cargoOutput}`);
  }
  if (actualVersion !== pinnedToolchain.version) {
    fail(
      `Cargo version ${actualVersion} does not match ${pinnedToolchain.channel} pinned by ${pinnedToolchain.path}.`,
    );
  }

  console.log(
    `[bevy-runtime] cargo=${actualVersion} pinned=${pinnedToolchain.channel} toolchain=${pinnedToolchain.path}`,
  );
}

function ensureCommand(command, args, label, hints, options = {}) {
  const result = capture(command, args, options);
  if (result.error?.code === "ENOENT") {
    fail([`Missing ${label} command: ${command}`, ...hints].join("\n"));
  }
  if (result.status !== 0) {
    fail([`Unable to run ${label}: ${command} ${args.join(" ")}`, outputOf(result), ...hints].join("\n"));
  }
  return result;
}

function ensureWasmTarget() {
  if (process.env.MIR2_SKIP_RUSTUP_TARGET_CHECK === "1") {
    return;
  }

  const installed = capture(rustupBin, ["target", "list", "--installed"], { cwd: runtimeDir });
  if (installed.error?.code === "ENOENT" || installed.status !== 0) {
    console.warn(
      "[bevy-runtime] rustup target check skipped; rustup is unavailable. Cargo will report the target error if wasm32-unknown-unknown is missing.",
    );
    return;
  }

  if (installed.stdout.split(/\r?\n/).includes("wasm32-unknown-unknown")) {
    return;
  }

  runChecked(rustupBin, ["target", "add", "wasm32-unknown-unknown"], {
    cwd: runtimeDir,
    label: "rustup target add wasm32-unknown-unknown",
  });
}

function ensureWasmBindgenVersion(command, expectedVersion) {
  if (!expectedVersion) {
    return;
  }

  const result = capture(command, ["--version"]);
  const actualVersion = result.stdout.match(/\d+\.\d+\.\d+/)?.[0];
  if (!actualVersion) {
    console.warn("[bevy-runtime] could not read wasm-bindgen version; continuing.");
    return;
  }

  if (actualVersion !== expectedVersion) {
    fail(
      [
        `wasm-bindgen CLI version ${actualVersion} does not match the Rust dependency version ${expectedVersion}.`,
        "The CLI and crate versions must match for generated bindings.",
        installWasmBindgenHint(expectedVersion),
      ].join("\n"),
    );
  }
}

function readExpectedWasmBindgenVersion(root) {
  const lockPath = path.join(root, "Cargo.lock");
  if (fs.existsSync(lockPath)) {
    const lockText = fs.readFileSync(lockPath, "utf8");
    for (const block of lockText.split(/\r?\n(?=\[\[package\]\]\r?\n)/)) {
      if (!/^name = "wasm-bindgen"$/m.test(block)) {
        continue;
      }
      const version = block.match(/version = "([^"]+)"/)?.[1];
      if (version) {
        return version;
      }
    }
  }

  const manifestPath = path.join(root, "Cargo.toml");
  if (!fs.existsSync(manifestPath)) {
    return null;
  }

  const manifest = fs.readFileSync(manifestPath, "utf8");
  return manifest.match(/^\s*wasm-bindgen\s*=\s*"([^"]+)"/m)?.[1] ?? null;
}

function createPublishLayout(finalPkgParentDir, finalManifestPath, nonce) {
  const transactionRoot = path.dirname(finalPkgParentDir);
  const stagingParentDir = path.join(transactionRoot, `.bevy-runtime-staging-${nonce}`);
  const backupParentDir = path.join(transactionRoot, `.bevy-runtime-backup-${nonce}`);
  return {
    nonce,
    finalPkgParentDir,
    finalManifestPath,
    stagingParentDir,
    stagedPkgParentDir: path.join(stagingParentDir, "bevy-runtime"),
    stagedManifestPath: path.join(stagingParentDir, "bevy_runtime_version.json"),
    backupParentDir,
    backupPkgParentDir: path.join(backupParentDir, "bevy-runtime"),
    journalPath: path.join(backupParentDir, "publish-state.json"),
    manifestTempPath: path.join(
      path.dirname(finalManifestPath),
      `.${path.basename(finalManifestPath)}.${nonce}.tmp`,
    ),
  };
}

function publishStagedArtifacts(layout, hooks = {}) {
  if (!fs.statSync(layout.stagedPkgParentDir).isDirectory()) {
    fail(`Staged runtime directory is missing: ${layout.stagedPkgParentDir}`);
  }
  if (!isNonEmptyFile(layout.stagedManifestPath)) {
    fail(`Staged runtime manifest is missing: ${layout.stagedManifestPath}`);
  }

  fs.mkdirSync(layout.backupParentDir, { recursive: true });
  const state = {
    version: 1,
    nonce: layout.nonce,
    originalPackageExisted: fs.existsSync(layout.finalPkgParentDir),
    packageBackedUp: false,
    packagePublished: false,
    committed: false,
  };
  writePublishJournal(layout, state);

  try {
    if (state.originalPackageExisted) {
      renameWithRetry(layout.finalPkgParentDir, layout.backupPkgParentDir, "back up current runtime package");
      state.packageBackedUp = true;
      writePublishJournal(layout, state);
    }

    renameWithRetry(layout.stagedPkgParentDir, layout.finalPkgParentDir, "publish staged runtime package");
    state.packagePublished = true;
    writePublishJournal(layout, state);

    hooks.beforeManifestPublish?.();
    publishManifestAtomically(layout.stagedManifestPath, layout.finalManifestPath, layout.manifestTempPath);
  } catch (publishError) {
    const rollbackErrors = rollbackPackagePublication(layout, state);
    if (rollbackErrors.length === 0) {
      removePathBestEffort(layout.backupParentDir);
      fail(`Runtime publish failed; previous package was restored.\n${errorMessage(publishError)}`);
    }
    fail(
      [
        `Runtime publish failed and rollback was incomplete. Backup retained at ${layout.backupParentDir}.`,
        errorMessage(publishError),
        ...rollbackErrors,
      ].join("\n"),
    );
  }

  // The manifest replacement is atomic. Once it succeeds, the staged package
  // and manifest are a complete validated publication and must not be rolled back.
  state.committed = true;
  try {
    writePublishJournal(layout, state);
  } catch (error) {
    console.warn(`[bevy-runtime] published successfully but could not finalize transaction journal: ${errorMessage(error)}`);
  }
  removePathBestEffort(layout.backupParentDir, "published runtime backup");
  console.log(`[bevy-runtime] published transaction=${layout.nonce}`);
}

function publishManifestAtomically(stagedManifestPath, finalManifestPath, tempManifestPath) {
  fs.mkdirSync(path.dirname(finalManifestPath), { recursive: true });
  fs.rmSync(tempManifestPath, { force: true });
  try {
    fs.copyFileSync(stagedManifestPath, tempManifestPath, fs.constants.COPYFILE_EXCL);
    renameWithRetry(tempManifestPath, finalManifestPath, "publish runtime version manifest");
  } finally {
    fs.rmSync(tempManifestPath, { force: true });
  }
}

function rollbackPackagePublication(layout, state) {
  const errors = [];
  const backupExists = fs.existsSync(layout.backupPkgParentDir);
  const publishedPackageExists = fs.existsSync(layout.finalPkgParentDir);
  const stagedPackageExists = fs.existsSync(layout.stagedPkgParentDir);
  const packageWasPublished =
    state.packagePublished || (!stagedPackageExists && publishedPackageExists && (backupExists || !state.originalPackageExisted));

  if (packageWasPublished && publishedPackageExists) {
    try {
      fs.mkdirSync(layout.stagingParentDir, { recursive: true });
      const rollbackDestination = stagedPackageExists
        ? path.join(layout.stagingParentDir, `failed-published-bevy-runtime-${Date.now()}`)
        : layout.stagedPkgParentDir;
      renameWithRetry(layout.finalPkgParentDir, rollbackDestination, "move failed runtime publication aside");
    } catch (error) {
      errors.push(`Unable to move the newly published package out of the formal path: ${errorMessage(error)}`);
    }
  }

  if (state.originalPackageExisted) {
    if (fs.existsSync(layout.backupPkgParentDir) && !fs.existsSync(layout.finalPkgParentDir)) {
      try {
        renameWithRetry(layout.backupPkgParentDir, layout.finalPkgParentDir, "restore previous runtime package");
      } catch (error) {
        errors.push(`Unable to restore the previous package: ${errorMessage(error)}`);
      }
    } else if (!fs.existsSync(layout.finalPkgParentDir)) {
      errors.push(`Previous package backup is missing: ${layout.backupPkgParentDir}`);
    }
  }

  return errors;
}

function writePublishJournal(layout, state) {
  fs.writeFileSync(layout.journalPath, `${JSON.stringify(state, null, 2)}\n`);
}

function recoverStaleBuild(owner) {
  const nonce = owner?.transactionNonce;
  if (!isSafeBuildNonce(nonce)) {
    return;
  }

  const staleLayout = createPublishLayout(pkgParentDir, runtimeVersionPath, nonce);
  if (!fs.existsSync(staleLayout.backupParentDir)) {
    cleanupBuildStaging(staleLayout);
    return;
  }

  let publishedBundleIsValid = false;
  try {
    validateRuntimeBundle({
      sourcePkgParentDir: pkgParentDir,
      manifestPath: runtimeVersionPath,
      publishedPkgParentDir: pkgParentDir,
      manifestRootDir: webRoot,
    });
    publishedBundleIsValid = true;
  } catch {
    publishedBundleIsValid = false;
  }

  if (publishedBundleIsValid) {
    console.warn(`[bevy-runtime] recovered completed transaction from stale lock: ${nonce}`);
    removePathBestEffort(staleLayout.backupParentDir);
    cleanupBuildStaging(staleLayout);
    return;
  }

  const state = readPublishJournal(staleLayout) ?? {
    originalPackageExisted: fs.existsSync(staleLayout.backupPkgParentDir),
    packagePublished: !fs.existsSync(staleLayout.stagedPkgParentDir),
  };
  const rollbackErrors = rollbackPackagePublication(staleLayout, state);
  if (rollbackErrors.length > 0) {
    fail(
      [
        `Could not recover interrupted runtime transaction ${nonce}.`,
        ...rollbackErrors,
        `Backup retained at ${staleLayout.backupParentDir}.`,
      ].join("\n"),
    );
  }

  console.warn(`[bevy-runtime] restored previous package from interrupted transaction: ${nonce}`);
  removePathBestEffort(staleLayout.backupParentDir);
  cleanupBuildStaging(staleLayout);
}

function readPublishJournal(layout) {
  try {
    return JSON.parse(fs.readFileSync(layout.journalPath, "utf8"));
  } catch {
    return null;
  }
}

function cleanupBuildStaging(layout) {
  fs.rmSync(layout.stagingParentDir, { recursive: true, force: true });
  fs.rmSync(layout.manifestTempPath, { force: true });
}

function createLockOwner(transactionNonce) {
  return {
    version: 1,
    pid: process.pid,
    token: crypto.randomUUID(),
    createdAt: new Date().toISOString(),
    cwd: process.cwd(),
    command: [process.execPath, ...process.argv].join(" "),
    transactionNonce,
  };
}

function acquireBuildLock(lockPath, options = {}) {
  const owner = options.owner ?? createLockOwner(null);
  const checkProcessAlive = options.isProcessAlive ?? isProcessAlive;
  const warn = options.warn ?? ((message) => console.warn(message));
  const ownerPath = path.join(lockPath, "owner.json");
  fs.mkdirSync(path.dirname(lockPath), { recursive: true });

  for (let attempt = 0; attempt < 8; attempt += 1) {
    try {
      fs.mkdirSync(lockPath);
      try {
        fs.writeFileSync(ownerPath, `${JSON.stringify(owner, null, 2)}\n`, { flag: "wx" });
      } catch (error) {
        fs.rmSync(lockPath, { recursive: true, force: true });
        throw error;
      }
      return { path: lockPath, token: owner.token };
    } catch (error) {
      if (error?.code !== "EEXIST") {
        throw error;
      }
    }

    const lockState = inspectBuildLock(lockPath, checkProcessAlive);
    if (lockState.kind === "live") {
      fail(
        [
          `Another Bevy runtime build is already running (pid=${lockState.owner.pid}, started=${lockState.owner.createdAt ?? "unknown"}).`,
          `Lock: ${lockPath}`,
          `Command: ${lockState.owner.command ?? "unknown"}`,
        ].join("\n"),
      );
    }
    if (lockState.kind === "initializing") {
      if (attempt < 3) {
        sleepSync(25);
        continue;
      }
      fail(`Another Bevy runtime build is acquiring the lock: ${lockPath}`);
    }

    const takeoverPath = claimStaleLockTakeover(lockPath, owner, checkProcessAlive);
    if (!takeoverPath) {
      sleepSync(25);
      continue;
    }

    // The owner can change between the stale snapshot above and our exclusive
    // takeover claim. Never recover or overwrite a process that became live in
    // that window; release our claim and restart the acquisition loop instead.
    const currentOwner = readLockOwner(lockPath);
    const expectedOwnerToken = lockState.owner?.token ?? null;
    const currentOwnerToken = currentOwner?.token ?? null;
    const takeoverOwner = readLockMetadata(takeoverPath);
    if (currentOwnerToken !== expectedOwnerToken || takeoverOwner?.token !== owner.token) {
      releaseStaleLockTakeover(takeoverPath, owner.token);
      sleepSync(25);
      continue;
    }

    try {
      options.onStaleLock?.(lockState.owner);
      replaceLockOwner(ownerPath, owner);
    } catch (error) {
      releaseStaleLockTakeover(takeoverPath, owner.token);
      throw error;
    }

    try {
      fs.rmSync(takeoverPath, { force: true });
    } catch (error) {
      warn(`[bevy-runtime] stale lock was claimed but takeover metadata could not be removed: ${errorMessage(error)}`);
    }
    warn(`[bevy-runtime] replaced stale build lock (${lockState.reason}): ${lockPath}`);
    return { path: lockPath, token: owner.token };
  }

  fail(`Unable to acquire Bevy runtime build lock after concurrent retries: ${lockPath}`);
}

function claimStaleLockTakeover(lockPath, owner, checkProcessAlive) {
  const takeoverPath = path.join(lockPath, "takeover.json");
  try {
    fs.writeFileSync(takeoverPath, `${JSON.stringify(owner, null, 2)}\n`, { flag: "wx" });
    return takeoverPath;
  } catch (error) {
    if (error?.code !== "EEXIST") {
      if (error?.code === "ENOENT") {
        return null;
      }
      throw error;
    }
  }

  const claimant = readLockMetadata(takeoverPath);
  if (claimant) {
    let alive = true;
    try {
      alive = checkProcessAlive(claimant.pid);
    } catch {
      alive = true;
    }
    if (alive) {
      fail(
        [
          `Another process is recovering a stale Bevy runtime build lock (pid=${claimant.pid}, started=${claimant.createdAt ?? "unknown"}).`,
          `Lock: ${lockPath}`,
        ].join("\n"),
      );
    }
  } else {
    try {
      if (Date.now() - fs.statSync(takeoverPath).mtimeMs < 5_000) {
        return null;
      }
    } catch (error) {
      if (error?.code === "ENOENT") {
        return null;
      }
      throw error;
    }
  }

  const staleTakeoverPath = `${takeoverPath}.stale-${process.pid}-${Date.now()}`;
  try {
    fs.renameSync(takeoverPath, staleTakeoverPath);
    fs.rmSync(staleTakeoverPath, { force: true });
  } catch (error) {
    if (error?.code !== "ENOENT") {
      throw error;
    }
  }
  return null;
}

function replaceLockOwner(ownerPath, owner) {
  const nextOwnerPath = `${ownerPath}.next-${owner.token}`;
  try {
    fs.writeFileSync(nextOwnerPath, `${JSON.stringify(owner, null, 2)}\n`, { flag: "wx" });
    fs.renameSync(nextOwnerPath, ownerPath);
  } finally {
    fs.rmSync(nextOwnerPath, { force: true });
  }
}

function releaseStaleLockTakeover(takeoverPath, token) {
  const claimant = readLockMetadata(takeoverPath);
  if (claimant?.token === token) {
    fs.rmSync(takeoverPath, { force: true });
  }
}

function inspectBuildLock(lockPath, checkProcessAlive) {
  const owner = readLockOwner(lockPath);
  if (owner) {
    let alive = true;
    try {
      alive = checkProcessAlive(owner.pid);
    } catch {
      alive = true;
    }
    return alive
      ? { kind: "live", owner }
      : { kind: "stale", owner, reason: `owner pid ${owner.pid} is not running` };
  }

  let ageMs = 0;
  try {
    ageMs = Date.now() - fs.statSync(lockPath).mtimeMs;
  } catch (error) {
    if (error?.code === "ENOENT") {
      return { kind: "stale", owner: null, reason: "lock disappeared during inspection" };
    }
    throw error;
  }

  if (ageMs < 5_000) {
    return { kind: "initializing", owner: null };
  }
  return { kind: "stale", owner: null, reason: "lock metadata is missing or invalid" };
}

function readLockOwner(lockPath) {
  return readLockMetadata(path.join(lockPath, "owner.json"));
}

function readLockMetadata(metadataPath) {
  try {
    const owner = JSON.parse(fs.readFileSync(metadataPath, "utf8"));
    if (!Number.isInteger(owner.pid) || owner.pid <= 0 || typeof owner.token !== "string") {
      return null;
    }
    return owner;
  } catch {
    return null;
  }
}

function releaseBuildLock(lock) {
  if (!lock || !fs.existsSync(lock.path)) {
    return;
  }

  const owner = readLockOwner(lock.path);
  if (!owner || owner.token !== lock.token) {
    console.warn(`[bevy-runtime] build lock ownership changed; refusing to remove ${lock.path}`);
    return;
  }
  fs.rmSync(lock.path, { recursive: true, force: true });
}

function isProcessAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error?.code === "ESRCH") {
      return false;
    }
    return true;
  }
}

function isSafeBuildNonce(nonce) {
  return typeof nonce === "string" && /^\d+-\d+-[a-f0-9]{12}$/.test(nonce);
}

function sleepSync(milliseconds) {
  const signal = new Int32Array(new SharedArrayBuffer(4));
  Atomics.wait(signal, 0, 0, milliseconds);
}

function runSelfCheck() {
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-bevy-runtime-self-check-"));
  try {
    selfCheckCargoPaths(tempRoot);
    selfCheckBuildLock(tempRoot);
    selfCheckPublication(tempRoot);
    const pinnedToolchain = readPinnedRustToolchain(runtimeDir);
    assertSelfCheck(pinnedToolchain.version === "1.95.0", "rust-toolchain.toml pin was not read as 1.95.0");
    console.log("[bevy-runtime] self-check passed (no Cargo build was run)");
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function selfCheckCargoPaths(tempRoot) {
  const fakeRuntimeRoot = path.join(tempRoot, "runtime");
  const targetRoot = resolveCargoTargetRoot(fakeRuntimeRoot, {
    CARGO_TARGET_DIR: path.join(tempRoot, "persistent-cargo-cache"),
  });
  const webgpuTarget = cargoTargetDirForBackend(targetRoot, "webgpu");
  const webgl2Target = cargoTargetDirForBackend(targetRoot, "webgl2");
  assertSelfCheck(webgpuTarget !== webgl2Target, "backend Cargo target directories overlap");
  assertSelfCheck(
    cargoWasmArtifactPath(webgpuTarget, "release") !== cargoWasmArtifactPath(webgl2Target, "release"),
    "backend WASM artifact paths overlap",
  );
  assertSelfCheck(
    webgpuTarget === cargoTargetDirForBackend(targetRoot, "webgpu"),
    "backend Cargo target directory is not deterministic",
  );

  const releaseEnv = buildCargoEnv("release", webgpuTarget, "win32", {});
  const devEnv = buildCargoEnv("dev", webgl2Target, "win32", {});
  assertSelfCheck(releaseEnv.CARGO_BUILD_JOBS === "1", "Windows release jobs default is not 1");
  assertSelfCheck(releaseEnv.CARGO_INCREMENTAL === "0", "Windows release incremental default is not 0");
  assertSelfCheck(devEnv.CARGO_BUILD_JOBS === "1", "Windows dev jobs default is not 1");
  assertSelfCheck(devEnv.CARGO_INCREMENTAL === "0", "Windows dev incremental default is not 0");
  assertSelfCheck(devEnv.CARGO_PROFILE_DEV_DEBUG === "0", "Windows dev debug data is not disabled");
  assertSelfCheck(
    devEnv.CARGO_PROFILE_DEV_BUILD_OVERRIDE_DEBUG === "0",
    "Windows dev build override debug data is not disabled",
  );
}

function selfCheckBuildLock(tempRoot) {
  const lockPath = path.join(tempRoot, "lock-check", ".bevy-runtime-build.lock");
  fs.mkdirSync(lockPath, { recursive: true });
  fs.writeFileSync(
    path.join(lockPath, "owner.json"),
    `${JSON.stringify({ pid: 999_999_999, token: "dead", createdAt: new Date().toISOString() })}\n`,
  );
  const acquired = acquireBuildLock(lockPath, {
    owner: { pid: process.pid, token: "self-check-stale-replacement", createdAt: new Date().toISOString() },
    isProcessAlive: () => false,
    warn: () => {},
  });
  releaseBuildLock(acquired);
  assertSelfCheck(!fs.existsSync(lockPath), "stale lock was not replaced and released");

  fs.mkdirSync(lockPath, { recursive: true });
  fs.writeFileSync(
    path.join(lockPath, "owner.json"),
    `${JSON.stringify({ pid: process.pid, token: "live", createdAt: new Date().toISOString() })}\n`,
  );
  expectSelfCheckFailure(
    () => acquireBuildLock(lockPath, { isProcessAlive: () => true, warn: () => {} }),
    "already running",
  );
  assertSelfCheck(fs.existsSync(lockPath), "live lock was incorrectly removed");
  fs.rmSync(lockPath, { recursive: true, force: true });

  fs.mkdirSync(lockPath, { recursive: true });
  fs.writeFileSync(
    path.join(lockPath, "owner.json"),
    `${JSON.stringify({ pid: 999_999_999, token: "dead", createdAt: new Date().toISOString() })}\n`,
  );
  fs.writeFileSync(
    path.join(lockPath, "takeover.json"),
    `${JSON.stringify({ pid: process.pid, token: "live-takeover", createdAt: new Date().toISOString() })}\n`,
  );
  expectSelfCheckFailure(
    () => acquireBuildLock(lockPath, { isProcessAlive: (pid) => pid === process.pid, warn: () => {} }),
    "recovering a stale",
  );
  assertSelfCheck(fs.existsSync(lockPath), "active stale-lock takeover was incorrectly removed");
  fs.rmSync(lockPath, { recursive: true, force: true });
}

function selfCheckPublication(tempRoot) {
  const fakeWebRoot = path.join(tempRoot, "web");
  const finalPackageDir = path.join(fakeWebRoot, "public", "bevy-runtime");
  const finalManifestPath = path.join(fakeWebRoot, "lib", "generated", "bevy_runtime_version.json");

  const invalidPackageDir = path.join(fakeWebRoot, "invalid-staging", "bevy-runtime");
  const invalidManifestPath = path.join(fakeWebRoot, "invalid-staging", "bevy_runtime_version.json");
  writeFakeRuntimeBundle(invalidPackageDir, "invalid");
  fs.writeFileSync(path.join(invalidPackageDir, "pkg-webgpu", "mir2_bevy_runtime_bg.wasm"), "not wasm");
  writeRuntimeVersionManifest({
    sourcePkgParentDir: invalidPackageDir,
    outputPath: invalidManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });
  expectSelfCheckFailure(
    () =>
      validateRuntimeBundle({
        sourcePkgParentDir: invalidPackageDir,
        manifestPath: invalidManifestPath,
        publishedPkgParentDir: finalPackageDir,
        manifestRootDir: fakeWebRoot,
      }),
    "failed WebAssembly.validate",
  );

  writeFakeRuntimeBundle(finalPackageDir, "old");
  writeRuntimeVersionManifest({
    sourcePkgParentDir: finalPackageDir,
    outputPath: finalManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });

  const rollbackLayout = createPublishLayout(finalPackageDir, finalManifestPath, "self-check-rollback");
  writeFakeRuntimeBundle(rollbackLayout.stagedPkgParentDir, "new-rollback");
  writeRuntimeVersionManifest({
    sourcePkgParentDir: rollbackLayout.stagedPkgParentDir,
    outputPath: rollbackLayout.stagedManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });
  validateRuntimeBundle({
    sourcePkgParentDir: rollbackLayout.stagedPkgParentDir,
    manifestPath: rollbackLayout.stagedManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });
  expectSelfCheckFailure(
    () =>
      publishStagedArtifacts(rollbackLayout, {
        beforeManifestPublish: () => {
          throw new Error("intentional self-check publish failure");
        },
      }),
    "previous package was restored",
  );
  assertSelfCheck(readFakeRuntimeMarker(finalPackageDir) === "old", "publish rollback did not restore old package");
  validateRuntimeBundle({
    sourcePkgParentDir: finalPackageDir,
    manifestPath: finalManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });

  const successLayout = createPublishLayout(finalPackageDir, finalManifestPath, "self-check-success");
  writeFakeRuntimeBundle(successLayout.stagedPkgParentDir, "new-success");
  writeRuntimeVersionManifest({
    sourcePkgParentDir: successLayout.stagedPkgParentDir,
    outputPath: successLayout.stagedManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });
  validateRuntimeBundle({
    sourcePkgParentDir: successLayout.stagedPkgParentDir,
    manifestPath: successLayout.stagedManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });
  publishStagedArtifacts(successLayout);
  assertSelfCheck(readFakeRuntimeMarker(finalPackageDir) === "new-success", "validated package was not published");
  validateRuntimeBundle({
    sourcePkgParentDir: finalPackageDir,
    manifestPath: finalManifestPath,
    publishedPkgParentDir: finalPackageDir,
    manifestRootDir: fakeWebRoot,
  });
}

function writeFakeRuntimeBundle(root, marker) {
  const packageNames = runtimePackages.map(({ packageDirName }) => packageDirName);
  const minimalWasm = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
  for (const packageDirName of packageNames) {
    const packageDir = path.join(root, packageDirName);
    fs.mkdirSync(packageDir, { recursive: true });
    fs.writeFileSync(path.join(packageDir, "mir2_bevy_runtime.js"), `export const marker = ${JSON.stringify(marker)};\n`);
    fs.writeFileSync(path.join(packageDir, "mir2_bevy_runtime_bg.wasm"), minimalWasm);
  }
}

function readFakeRuntimeMarker(root) {
  const contents = fs.readFileSync(path.join(root, "pkg-webgpu", "mir2_bevy_runtime.js"), "utf8");
  return contents.match(/marker = "([^"]+)"/)?.[1] ?? null;
}

function assertSelfCheck(condition, message) {
  if (!condition) {
    fail(`Self-check failed: ${message}`);
  }
}

function expectSelfCheckFailure(callback, expectedMessage) {
  try {
    callback();
  } catch (error) {
    assertSelfCheck(
      errorMessage(error).includes(expectedMessage),
      `expected failure containing "${expectedMessage}", received "${errorMessage(error)}"`,
    );
    return;
  }
  fail(`Self-check failed: expected failure containing "${expectedMessage}"`);
}

function runChecked(command, args, options = {}) {
  console.log(`[bevy-runtime] ${options.label ?? command}`);
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env ?? process.env,
    shell: false,
    stdio: "inherit",
  });

  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${options.label ?? command} failed with exit code ${result.status}`);
  }
}

function resolveWasmBindgenRustMinStack(env) {
  const configured = Number.parseInt(
    env.MIR2_WASM_BINDGEN_RUST_MIN_STACK ?? env.RUST_MIN_STACK ?? "",
    10,
  );
  // Large Bevy WASM modules can overflow wasm-bindgen's default Windows stack.
  return String(Number.isFinite(configured) && configured > 0 ? configured : 64 * 1024 * 1024);
}

function runWasmBindgenWithRetries(command, args, options = {}) {
  const configuredAttempts = Number.parseInt(process.env.MIR2_WASM_BINDGEN_ATTEMPTS ?? "", 10);
  const maxAttempts = Number.isFinite(configuredAttempts) && configuredAttempts > 0
    ? Math.min(configuredAttempts, 20)
    : process.platform === "win32"
      ? 8
      : 1;
  let lastError = null;

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    if (attempt > 1 && options.resetOutputDir) {
      fs.rmSync(options.resetOutputDir, { recursive: true, force: true });
      fs.mkdirSync(options.resetOutputDir, { recursive: true });
    }
    console.log(`[bevy-runtime] ${options.label ?? command}`);
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: options.env ?? process.env,
      shell: false,
      stdio: "inherit",
    });
    if (result.error) {
      lastError = result.error;
    } else if (result.status === 0) {
      if (!options.validateWasmPath || isValidWasmFile(options.validateWasmPath)) {
        return;
      }
      lastError = new Error(
        `${options.label ?? command} returned success but generated invalid WASM: ${options.validateWasmPath}`,
      );
    } else {
      lastError = new Error(`${options.label ?? command} failed with exit code ${result.status}`);
    }
    if (attempt < maxAttempts) {
      console.warn(
        `[bevy-runtime] ${options.label ?? command} failed on attempt ${attempt}/${maxAttempts}; retrying with a clean staged output directory.`,
      );
    }
  }

  throw lastError ?? new Error(`${options.label ?? command} failed`);
}

function isValidWasmFile(filePath) {
  try {
    return isNonEmptyFile(filePath) && WebAssembly.validate(fs.readFileSync(filePath));
  } catch {
    return false;
  }
}

function runCargoBuildWithRetries(command, args, options = {}) {
  const configuredAttempts = Number.parseInt(process.env.MIR2_BEVY_CARGO_BUILD_ATTEMPTS ?? "", 10);
  const maxAttempts = Number.isFinite(configuredAttempts) && configuredAttempts > 0
    ? Math.min(configuredAttempts, 50)
    : process.platform === "win32"
      ? 20
      : 1;
  let lastError = null;

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    console.log(`[bevy-runtime] ${options.label ?? command}`);
    const result = spawnSync(command, args, {
      cwd: options.cwd,
      env: options.env ?? process.env,
      encoding: "utf8",
      shell: false,
      stdio: ["ignore", "pipe", "pipe"],
    });
    if (result.stdout) {
      process.stdout.write(result.stdout);
    }
    if (result.stderr) {
      process.stderr.write(result.stderr);
    }
    if (!result.error && result.status === 0) {
      return;
    }

    lastError = result.error ?? new Error(`${options.label ?? command} failed with exit code ${result.status}`);
    const diagnostics = [result.error?.message, result.stdout, result.stderr].filter(Boolean).join("\n");
    if (!isTransientRustCompilerFailure(diagnostics) || attempt >= maxAttempts) {
      break;
    }
    console.warn(
      `[bevy-runtime] ${options.label ?? command} hit a transient rustc failure on attempt ${attempt}/${maxAttempts}; retrying with the persistent target cache.`,
    );
  }

  throw lastError ?? new Error(`${options.label ?? command} failed`);
}

function isTransientRustCompilerFailure(diagnostics) {
  return /(?:internal compiler error|primitive read not possible|scalar size mismatch|rustc-LLVM ERROR|STATUS_ACCESS_VIOLATION|STATUS_STACK_BUFFER_OVERRUN|STATUS_ILLEGAL_INSTRUCTION|0xc0000005|0xc0000409|0xc000001d)/i.test(
    diagnostics,
  );
}

function capture(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd,
    env: options.env ?? process.env,
    encoding: "utf8",
    shell: false,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function isNonEmptyFile(filePath) {
  try {
    const stat = fs.statSync(filePath);
    return stat.isFile() && stat.size > 0;
  } catch {
    return false;
  }
}

function renameWithRetry(sourcePath, destinationPath, label) {
  const maxAttempts = process.platform === "win32" ? 100 : 1;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      fs.renameSync(sourcePath, destinationPath);
      return;
    } catch (error) {
      const transient = ["EACCES", "EBUSY", "EPERM"].includes(error?.code);
      if (!transient || attempt >= maxAttempts) {
        throw error;
      }
      if (attempt === 1) {
        console.warn(`[bevy-runtime] Windows file lock while trying to ${label}; retrying.`);
      }
      sleepSync(50);
    }
  }
}

function removePathBestEffort(filePath, label = "transaction path") {
  try {
    fs.rmSync(filePath, { recursive: true, force: true });
  } catch (error) {
    console.warn(`[bevy-runtime] could not remove ${label} ${filePath}: ${errorMessage(error)}`);
  }
}

function installWasmBindgenHint(version) {
  const versionArg = version ? ` --version ${version}` : "";
  return `Install matching wasm-bindgen CLI: cargo install wasm-bindgen-cli${versionArg} --locked`;
}

function outputOf(result) {
  return [result.stdout, result.stderr].filter(Boolean).join("\n").trim();
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function fail(message) {
  throw new Error(message);
}
