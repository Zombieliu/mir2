#!/usr/bin/env node
// This bundle is intentionally independent of the renderer and its assets.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { optimizeClientCoreReleaseWasm, resolveClientCoreWasmOptConfig, assertClientCoreReleaseBudget,
  assertClientCoreStagingRoot, formatRendererOptimizationError } from "./lib/renderer-wasm-opt.mjs";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const crateRoot = path.resolve(webRoot, "../game-client/platform-web");
const targetRoot = path.resolve(process.env.MIR2_CLIENT_CORE_TARGET_DIR || path.join(crateRoot, "target"));
const publicRoot = path.join(webRoot, "public/client-core");
const manifestPath = path.join(webRoot, "lib/generated/client_core_runtime.json");
const files = ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"];
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");

function sourceFingerprint() {
  const coreRoot = path.resolve(crateRoot, "../client-core");
  const projectRoot = path.resolve(webRoot, "../..");
  const sources = [path.join(crateRoot, "Cargo.toml"), path.join(crateRoot, "Cargo.lock"),
    path.join(crateRoot, "rust-toolchain.toml"), path.join(coreRoot, "Cargo.toml"),
    fileURLToPath(import.meta.url), fileURLToPath(new URL("./lib/renderer-wasm-opt.mjs", import.meta.url))];
  function collect(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const filename = path.join(directory, entry.name);
      if (entry.isDirectory()) collect(filename);
      else if (entry.isFile() && entry.name.endsWith(".rs")) sources.push(filename);
    }
  }
  collect(path.join(coreRoot, "src"));
  collect(path.join(crateRoot, "src"));
  const entries = sources.map((filename) => ({
    name: path.relative(projectRoot, filename).split(path.sep).join("/"),
    text: fs.readFileSync(filename, "utf8").replace(/\r\n/g, "\n"),
  })).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  return hash(JSON.stringify(entries));
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: crateRoot, stdio: "inherit", shell: false });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited ${result.status}`);
}

function verify() {
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (manifest.abiVersion !== 1 || !/^[a-f0-9]{64}$/.test(manifest.version)) {
    throw new Error("Invalid client-core manifest; run npm run client-core:build");
  }
  if (manifest.sourceSha256 !== sourceFingerprint()) {
    throw new Error("Shared client source changed; rebuild and commit its small WASM package before publishing");
  }
  const artifacts = [];
  for (const name of files) {
    const bytes = fs.readFileSync(path.join(publicRoot, manifest.version, name));
    if (hash(bytes) !== manifest.files[name].sha256 || bytes.length !== manifest.files[name].bytes) {
      throw new Error(`client-core artifact mismatch: ${name}`);
    }
    artifacts.push(bytes);
  }
  if (hash(Buffer.concat(artifacts)) !== manifest.version) {
    throw new Error("client-core content does not match its immutable version URL");
  }
  assertClientCoreReleaseBudget({ wasmBytes: artifacts[1], jsBytes: artifacts[0] });
  console.log(`[client-core] verified ${manifest.version}; wasm=${manifest.files[files[1]].bytes} bytes`);
}

function main() {
const prebuilt = (process.env.MIR2_USE_PREBUILT_CLIENT_CORE ?? process.env.MIR2_USE_PREBUILT_BEVY_RUNTIME) === "1";
if (process.argv[2] === "--verify" || (process.argv.length === 2 && prebuilt)) {
  verify();
} else if (process.argv.length > 2) {
  throw new Error("Usage: node scripts/build-client-core.mjs [--verify]");
} else {
  const toolConfig = resolveClientCoreWasmOptConfig(process.env);
  const bindgen = process.env.WASM_BINDGEN_BIN || "wasm-bindgen";
  const bindgenVersion = spawnSync(bindgen, ["--version"], { encoding: "utf8", shell: false });
  if (bindgenVersion.status !== 0 || bindgenVersion.stdout.trim() !== "wasm-bindgen 0.2.118") {
    throw new Error("platform-web requires wasm-bindgen 0.2.118, matching its Cargo.lock");
  }
  run(process.env.CARGO_BIN || "cargo", ["+1.95.0", "build", "--locked", "--release",
    "--target", "wasm32-unknown-unknown", "--target-dir", targetRoot]);
  const temporaryRoot = path.resolve(os.tmpdir());
  const staging = fs.mkdtempSync(path.join(temporaryRoot, "mir2-client-core-"));
  let failure = null;
  try {
    assertClientCoreStagingRoot(staging, temporaryRoot);
    run(bindgen, [path.join(targetRoot, "wasm32-unknown-unknown/release/mir2_platform_web.wasm"),
      "--target", "web", "--out-dir", staging, "--out-name", "mir2_platform_web",
      // Release downloads do not need the debugging function-name section.
      "--remove-name-section"]);
    const optimization = optimizeClientCoreReleaseWasm({
      wasmPath: path.join(staging, files[1]), jsPath: path.join(staging, files[0]),
      stagingRoot: staging, toolConfig,
    });
    console.log(`[client-core] wasm-opt=${JSON.stringify(optimization)}`);
    const bytes = files.map((name) => fs.readFileSync(path.join(staging, name)));
    assertClientCoreReleaseBudget({ wasmBytes: bytes[1], jsBytes: bytes[0] });
    const version = hash(Buffer.concat(bytes));
    const destination = path.join(publicRoot, version);
    fs.mkdirSync(destination, { recursive: true });
    // Versioned URLs never point at a half-published module: publish the small
    // build-time manifest only after both content-addressed files are ready.
    files.forEach((name, index) => fs.writeFileSync(path.join(destination, name), bytes[index]));
    const manifest = { abiVersion: 1, version, sourceSha256: sourceFingerprint(),
      files: Object.fromEntries(files.map((name, index) => [name,
        { bytes: bytes[index].length, sha256: hash(bytes[index]) }])) };
    const json = `${JSON.stringify(manifest, null, 2)}\n`;
    fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
    if (!fs.existsSync(manifestPath) || fs.readFileSync(manifestPath, "utf8") !== json) {
      const temporaryManifest = `${manifestPath}.${process.pid}.tmp`;
      fs.writeFileSync(temporaryManifest, json);
      fs.renameSync(temporaryManifest, manifestPath);
    }
    verify();
  } catch (error) { failure = error; }
  finally {
    try {
      // Recheck the direct mkdtemp name and every ordinary ancestor after tools.
      const owned = assertClientCoreStagingRoot(staging, temporaryRoot);
      fs.rmSync(owned, { recursive: true, force: false });
    } catch (error) {
      const cleanupError = new Error("Client-core staging cleanup refused or failed; residual may remain: " + staging, { cause: error });
      failure = failure ? new AggregateError([failure, cleanupError], "Client-core build and staging cleanup failed") : cleanupError;
    }
  }
  if (failure) throw failure;
}
}
try { main(); }
catch (error) {
  // Do not let default inspection serialize arbitrary child result/env objects.
  console.error(formatRendererOptimizationError(error));
  process.exitCode = 1;
}
