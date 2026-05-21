#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const profile = process.argv[2] ?? "release";
const profiles = {
  dev: { cargoProfileDir: "debug", cargoFlags: [] },
  release: { cargoProfileDir: "release", cargoFlags: ["--release"] },
};

if (!profiles[profile]) {
  fail(`Unsupported profile: ${profile}. Use "dev" or "release".`);
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const runtimeDir = path.resolve(webRoot, "..", "game-client", "runtime");
const pkgDir = path.resolve(webRoot, "public", "bevy-runtime", "pkg");
const pkgParentDir = path.dirname(pkgDir);
const tmpPkgDir = path.join(pkgParentDir, `.pkg-${process.pid}-${Date.now()}`);
const runtimeVersionPath = path.resolve(webRoot, "lib", "generated", "bevy_runtime_version.json");
const prebuiltRuntimeFiles = [
  path.join(pkgDir, "mir2_bevy_runtime.js"),
  path.join(pkgDir, "mir2_bevy_runtime_bg.wasm"),
];
const wasmFile = path.resolve(
  runtimeDir,
  "target",
  "wasm32-unknown-unknown",
  profiles[profile].cargoProfileDir,
  "mir2_bevy_runtime.wasm",
);

const cargoBin = process.env.CARGO_BIN || "cargo";
const rustupBin = process.env.RUSTUP_BIN || "rustup";
const wasmBindgenBin = process.env.WASM_BINDGEN_BIN || "wasm-bindgen";

if (process.env.MIR2_USE_PREBUILT_BEVY_RUNTIME === "1") {
  ensurePrebuiltRuntime();
  writeRuntimeVersionManifest();
  console.log(`[bevy-runtime] using prebuilt package at ${pkgDir}`);
  process.exit(0);
}

const expectedWasmBindgenVersion = readExpectedWasmBindgenVersion(runtimeDir);

try {
  console.log(`[bevy-runtime] profile=${profile}`);
  console.log(`[bevy-runtime] runtime=${runtimeDir}`);
  console.log(`[bevy-runtime] output=${pkgDir}`);

  ensureCommand(cargoBin, ["--version"], "cargo", [
    "Install Rust from https://rustup.rs/ or set CARGO_BIN to a cargo executable.",
  ]);

  ensureCommand(wasmBindgenBin, ["--version"], "wasm-bindgen", [
    installWasmBindgenHint(expectedWasmBindgenVersion),
    "Or set WASM_BINDGEN_BIN to a wasm-bindgen executable.",
  ]);

  ensureWasmBindgenVersion(wasmBindgenBin, expectedWasmBindgenVersion);
  ensureWasmTarget();

  runChecked(
    cargoBin,
    ["build", "--target", "wasm32-unknown-unknown", ...profiles[profile].cargoFlags],
    {
      cwd: runtimeDir,
      label: "cargo build",
    },
  );

  if (!fs.existsSync(wasmFile)) {
    fail(`Cargo build finished but the expected WASM file was not found: ${wasmFile}`);
  }

  fs.mkdirSync(pkgParentDir, { recursive: true });
  fs.rmSync(tmpPkgDir, { recursive: true, force: true });
  fs.mkdirSync(tmpPkgDir, { recursive: true });

  runChecked(
    wasmBindgenBin,
    [
      "--target",
      "web",
      "--out-dir",
      tmpPkgDir,
      "--out-name",
      "mir2_bevy_runtime",
      wasmFile,
    ],
    {
      cwd: runtimeDir,
      label: "wasm-bindgen",
    },
  );

  fs.rmSync(pkgDir, { recursive: true, force: true });
  fs.renameSync(tmpPkgDir, pkgDir);
  writeRuntimeVersionManifest();
  console.log(`[bevy-runtime] wrote ${pkgDir}`);
} catch (error) {
  fs.rmSync(tmpPkgDir, { recursive: true, force: true });
  if (error instanceof Error) {
    fail(error.message);
  }
  fail(String(error));
}

function ensureCommand(command, args, label, hints) {
  const result = capture(command, args);
  if (result.error?.code === "ENOENT") {
    fail([`Missing ${label} command: ${command}`, ...hints].join("\n"));
  }
  if (result.status !== 0) {
    fail([`Unable to run ${label}: ${command} ${args.join(" ")}`, outputOf(result), ...hints].join("\n"));
  }
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
    for (const block of lockText.split(/\n(?=\[\[package\]\]\n)/)) {
      if (!block.includes('name = "wasm-bindgen"')) {
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

function ensurePrebuiltRuntime() {
  const missing = prebuiltRuntimeFiles.filter((filePath) => !isNonEmptyFile(filePath));
  if (missing.length > 0) {
    fail(`MIR2_USE_PREBUILT_BEVY_RUNTIME=1 but required files are missing: ${missing.join(", ")}`);
  }
}

function writeRuntimeVersionManifest() {
  const files = prebuiltRuntimeFiles.map((filePath) => {
    const bytes = fs.readFileSync(filePath);
    return {
      path: path.relative(webRoot, filePath).split(path.sep).join("/"),
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

  const manifest = {
    version: `bevy-${combined.digest("hex").slice(0, 16)}`,
    files,
  };

  fs.mkdirSync(path.dirname(runtimeVersionPath), { recursive: true });
  fs.writeFileSync(runtimeVersionPath, `${JSON.stringify(manifest, null, 2)}\n`);
  console.log(`[bevy-runtime] version=${manifest.version}`);
}

function isNonEmptyFile(filePath) {
  try {
    return fs.statSync(filePath).isFile() && fs.statSync(filePath).size > 0;
  } catch {
    return false;
  }
}

function installWasmBindgenHint(version) {
  const versionArg = version ? ` --version ${version}` : "";
  return `Install matching wasm-bindgen CLI: cargo install wasm-bindgen-cli${versionArg} --locked`;
}

function runChecked(command, args, options = {}) {
  console.log(`[bevy-runtime] ${options.label ?? command}`);
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    env: process.env,
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

function capture(command, args, options = {}) {
  return spawnSync(command, args, {
    cwd: options.cwd,
    env: process.env,
    encoding: "utf8",
    shell: false,
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function outputOf(result) {
  return [result.stdout, result.stderr].filter(Boolean).join("\n").trim();
}

function fail(message) {
  console.error(`[bevy-runtime] ${message}`);
  process.exit(1);
}
