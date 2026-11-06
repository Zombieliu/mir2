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

import { readClientCoreRelease, verifyClientCoreManifest } from "./lib/client-core-release-files.mjs";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const crateRoot = path.resolve(webRoot, "../game-client/platform-web");
const targetRoot = path.resolve(process.env.MIR2_CLIENT_CORE_TARGET_DIR || path.join(crateRoot, "target"));
const publicRoot = path.join(webRoot, "public/client-core");
const manifestPath = path.join(webRoot, "lib/generated/client_core_runtime.json");
const files = ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"];
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");

export function sourceFingerprint() {
  const coreRoot = path.resolve(crateRoot, "../client-core");
  const wireRoot = path.resolve(crateRoot, "../client-wire");
  const projectRoot = path.resolve(webRoot, "../..");
  const sources = [path.join(crateRoot, "Cargo.toml"), path.join(crateRoot, "Cargo.lock"),
    path.join(crateRoot, "rust-toolchain.toml"), path.join(coreRoot, "Cargo.toml"),
    path.join(wireRoot, "Cargo.toml"),
    fileURLToPath(import.meta.url), fileURLToPath(new URL("./lib/renderer-wasm-opt.mjs", import.meta.url)),
    fileURLToPath(new URL("./lib/client-core-release-files.mjs", import.meta.url))];
  function collect(directory) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const filename = path.join(directory, entry.name);
      if (entry.isDirectory()) collect(filename);
      else if (entry.isFile() && entry.name.endsWith(".rs")) sources.push(filename);
    }
  }
  collect(path.join(coreRoot, "src"));
  collect(path.join(wireRoot, "src"));
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
  const manifest = verifyClientCoreManifest(JSON.parse(fs.readFileSync(manifestPath, "utf8")));
  if (!manifest.presentation) throw new Error("Client-core manifest requires its Presentation bundle; run npm run client-core:build");
  if (!manifest.npcPurchase) throw new Error("Client-core manifest requires its NPC Purchase bundle; run npm run client-core:build");
  const sourceSha256 = sourceFingerprint();
  if ([manifest, manifest.presentation, manifest.npcPurchase].some((bundle) => bundle.sourceSha256 !== sourceSha256)) {
    throw new Error("Shared client source changed; rebuild and commit its small WASM packages before publishing");
  }
  const release = readClientCoreRelease({ webRoot, manifest });
  for (let index = 0; index < release.files.length; index += files.length) {
    assertClientCoreReleaseBudget({ wasmBytes: fs.readFileSync(release.files[index + 1].localPath),
      jsBytes: fs.readFileSync(release.files[index].localPath) });
  }
  console.log("[client-core] verified " + manifest.version + "; wasm=" + manifest.files[files[1]].bytes +
    " bytes; presentation=" + manifest.presentation.version + "; wasm=" + manifest.presentation.files[files[1]].bytes +
    " bytes; npcPurchase=" + manifest.npcPurchase.version + "; wasm=" + manifest.npcPurchase.files[files[1]].bytes + " bytes");
}

function buildBundle(label, features, toolConfig, bindgen) {
  run(process.env.CARGO_BIN || "cargo", ["+1.95.0", "build", "--locked", "--release",
    "--target", "wasm32-unknown-unknown", "--target-dir", targetRoot, ...features]);
  const temporaryRoot = path.resolve(os.tmpdir());
  const staging = fs.mkdtempSync(path.join(temporaryRoot, "mir2-client-core-"));
  let failure = null, bytes;
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
    console.log("[client-core:" + label + "] wasm-opt=" + JSON.stringify(optimization));
    bytes = files.map((name) => fs.readFileSync(path.join(staging, name)));
    assertClientCoreReleaseBudget({ wasmBytes: bytes[1], jsBytes: bytes[0] });
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
  return { version: hash(Buffer.concat(bytes)), bytes };
}

function ordinaryDirectory(candidate) {
  const resolved = path.resolve(candidate);
  let cursor = path.parse(resolved).root;
  for (const part of ["", ...path.relative(cursor, resolved).split(path.sep).filter(Boolean)]) {
    cursor = part ? path.join(cursor, part) : cursor;
    const stat = fs.lstatSync(cursor);
    if (stat.isSymbolicLink() || !stat.isDirectory()) throw new Error("Client-core publication directory is linked or irregular");
  }
}
function publishBundle(bundle) {
  fs.mkdirSync(publicRoot, { recursive: true });
  ordinaryDirectory(publicRoot);
  const destination = path.join(publicRoot, bundle.version);
  fs.mkdirSync(destination, { recursive: true });
  ordinaryDirectory(destination);
  files.forEach((name, index) => {
    const output = path.join(destination, name);
    if (fs.existsSync(output)) {
      const stat = fs.lstatSync(output);
      if (stat.isSymbolicLink() || !stat.isFile() || !fs.readFileSync(output).equals(bundle.bytes[index])) {
        throw new Error("Client-core immutable publication collision: " + name);
      }
    } else fs.writeFileSync(output, bundle.bytes[index], { flag: "wx" });
  });
}
function describeBundle(bundle, sourceSha256) {
  return { abiVersion: 1, version: bundle.version, sourceSha256,
    files: Object.fromEntries(files.map((name, index) => [name,
      { bytes: bundle.bytes[index].length, sha256: hash(bundle.bytes[index]) }])) };
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
  const sourceSha256 = sourceFingerprint();
  // All three builds use the same guarded Cargo executable. Capture each bundle
  // before the next feature build replaces the target artifact; nothing is published yet.
  const core = buildBundle("core", [], toolConfig, bindgen);
  const presentation = buildBundle("presentation", ["--features", "presentation-ui"], toolConfig, bindgen);
  const npcPurchase = buildBundle("npc-purchase", ["--features", "npc-purchase-policy"], toolConfig, bindgen);
  if (sourceFingerprint() !== sourceSha256) throw new Error("Shared client source changed during the three builds");
  const manifest = { ...describeBundle(core, sourceSha256),
    presentation: describeBundle(presentation, sourceSha256),
    npcPurchase: describeBundle(npcPurchase, sourceSha256) };
  verifyClientCoreManifest(manifest);
  // All optimization, metadata, budget and cleanup checks have passed for all three.
  // Only then write immutable leaves, and publish their shared manifest last.
  publishBundle(core);
  publishBundle(presentation);
  publishBundle(npcPurchase);
  readClientCoreRelease({ webRoot, manifest });
  const json = JSON.stringify(manifest, null, 2) + "\n";
  fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
  if (!fs.existsSync(manifestPath) || fs.readFileSync(manifestPath, "utf8") !== json) {
    const temporaryManifest = manifestPath + "." + process.pid + ".tmp";
    fs.writeFileSync(temporaryManifest, json);
    fs.renameSync(temporaryManifest, manifestPath);
  }
  verify();
}
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { main(); }
  catch (error) {
    // Do not let default inspection serialize arbitrary child result/env objects.
    console.error(formatRendererOptimizationError(error));
    process.exitCode = 1;
  }
}
