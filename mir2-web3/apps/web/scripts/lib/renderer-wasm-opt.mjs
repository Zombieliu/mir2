import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { spawnSync } from "node:child_process";
import { gzipSync } from "node:zlib";

export const RENDERER_RELEASE_LIMITS = Object.freeze({
  wasmBytes: 32505856,
  gzipBytes: 7340032,
  jsBytes: 204800,
});

export const CLIENT_CORE_RELEASE_LIMITS = Object.freeze({ wasmBytesExclusive: 262144, jsBytes: 204800 });
export function resolveClientCoreWasmOptConfig(env = process.env) {
  const bin = env.MIR2_CLIENT_CORE_WASM_OPT_BIN;
  const sha256 = env.MIR2_CLIENT_CORE_WASM_OPT_SHA256;
  if (typeof bin !== "string" || !bin || !path.isAbsolute(bin)) {
    throw new Error("Set MIR2_CLIENT_CORE_WASM_OPT_BIN to an absolute Binaryen131 wasm-opt executable");
  }
  if (typeof sha256 !== "string" || !/^[a-f0-9]{64}$/.test(sha256)) {
    throw new Error("Set MIR2_CLIENT_CORE_WASM_OPT_SHA256 to the executable's exact lowercase SHA256");
  }
  return Object.freeze({ bin: path.resolve(bin), sha256 });
}
export function assertClientCoreReleaseSizes(sizes) {
  const { wasmBytes, jsBytes } = sizes;
  if (!Number.isSafeInteger(wasmBytes) || wasmBytes <= 0 || wasmBytes >= CLIENT_CORE_RELEASE_LIMITS.wasmBytesExclusive) {
    throw new Error("client-core wasmBytes exceeds release byte budget or is invalid: " + wasmBytes + " (strictly below " + CLIENT_CORE_RELEASE_LIMITS.wasmBytesExclusive + ")");
  }
  if (!Number.isSafeInteger(jsBytes) || jsBytes <= 0 || jsBytes > CLIENT_CORE_RELEASE_LIMITS.jsBytes) {
    throw new Error("client-core jsBytes exceeds release byte budget or is invalid: " + jsBytes + " (max " + CLIENT_CORE_RELEASE_LIMITS.jsBytes + ")");
  }
  return Object.freeze({ wasmBytes, jsBytes });
}
export function assertClientCoreReleaseBudget({ wasmBytes, jsBytes }) {
  return assertClientCoreReleaseSizes({ wasmBytes: wasmBytes.length, jsBytes: jsBytes.length });
}
export function validateClientCoreReleasePackage({ wasmPath, jsPath }) {
  const wasmBytes = regularBytes(wasmPath), jsBytes = regularBytes(jsPath);
  moduleMetadata(wasmBytes, "client-core");
  return assertClientCoreReleaseBudget({ wasmBytes, jsBytes });
}
/** Only a direct mkdtemp child, with every ancestor still ordinary, may be removed. */
export function assertClientCoreStagingRoot(stagingRoot, temporaryRoot) {
  if (typeof stagingRoot !== "string" || !path.isAbsolute(stagingRoot)
    || typeof temporaryRoot !== "string" || !path.isAbsolute(temporaryRoot)) {
    throw new Error("Client-core temporary roots must be absolute");
  }
  const root = path.resolve(temporaryRoot), staging = path.resolve(stagingRoot);
  if (path.dirname(staging) !== root || !/^mir2-client-core-[A-Za-z0-9]{6}$/.test(path.basename(staging))) {
    throw new Error("Refusing cleanup outside the generated client-core staging directory");
  }
  ordinaryDirectory(root); ordinaryDirectory(staging);
  return staging;
}
// These profiles are private and fixed. A caller cannot select different
// basenames, limits, arguments or a cleanup prefix through its options.
const RENDERER_PROFILE = Object.freeze({ label: "renderer", subject: "Renderer",
  optimizationFlag: "-O1",
  wasmName: "mir2_bevy_runtime_bg.wasm", jsName: "mir2_bevy_runtime.js",
  temporaryPrefix: ".mir2-bevy-opt-", temporaryPattern: /^\.mir2-bevy-opt-\d+-[a-f0-9]{12}\.wasm$/,
  resolveConfig: tool => resolveRendererWasmOptConfig({ MIR2_BEVY_WASM_OPT_BIN: tool?.bin, MIR2_BEVY_WASM_OPT_SHA256: tool?.sha256 }),
  assertBudget: assertRendererReleaseBudget });
const CLIENT_CORE_PROFILE = Object.freeze({ label: "client-core", subject: "Client-core",
  optimizationFlag: "-Oz",
  wasmName: "mir2_platform_web_bg.wasm", jsName: "mir2_platform_web.js",
  temporaryPrefix: ".mir2-client-core-opt-", temporaryPattern: /^\.mir2-client-core-opt-\d+-[a-f0-9]{12}\.wasm$/,
  resolveConfig: tool => resolveClientCoreWasmOptConfig({ MIR2_CLIENT_CORE_WASM_OPT_BIN: tool?.bin, MIR2_CLIENT_CORE_WASM_OPT_SHA256: tool?.sha256 }),
  assertBudget: assertClientCoreReleaseBudget });

const SYSTEM_ENV_NAMES = ["systemroot", "windir", "systemdrive", "comspec", "pathext",
  "temp", "tmp", "userprofile", "homedrive", "homepath", "appdata", "localappdata",
  "os", "processor_architecture", "number_of_processors"];
const hash = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

export function resolveRendererWasmOptConfig(env = process.env) {
  const bin = env.MIR2_BEVY_WASM_OPT_BIN;
  const sha256 = env.MIR2_BEVY_WASM_OPT_SHA256;
  if (typeof bin !== "string" || !bin || !path.isAbsolute(bin)) {
    throw new Error("Set MIR2_BEVY_WASM_OPT_BIN to an absolute Binaryen131 wasm-opt executable");
  }
  if (typeof sha256 !== "string" || !/^[a-f0-9]{64}$/.test(sha256)) {
    throw new Error("Set MIR2_BEVY_WASM_OPT_SHA256 to the executable's exact lowercase SHA256");
  }
  return Object.freeze({ bin: path.resolve(bin), sha256 });
}

function ordinaryDirectory(directory) {
  if (typeof directory !== "string" || !path.isAbsolute(directory)) throw new Error("Renderer directory must be absolute");
  const resolved = path.resolve(directory);
  for (let cursor = resolved;;) {
    const stat = fs.lstatSync(cursor);
    if (!stat.isDirectory() || stat.isSymbolicLink()) throw new Error("Renderer directory is linked or irregular: " + cursor);
    const parent = path.dirname(cursor);
    if (parent === cursor) break;
    cursor = parent;
  }
  return resolved;
}

function regularBytes(file) {
  ordinaryDirectory(path.dirname(file));
  const stat = fs.lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error("Renderer file is linked or irregular: " + file);
  return fs.readFileSync(file);
}
function filePin(file) {
  const bytes = regularBytes(file);
  return { path: file, bytes: bytes.length, sha256: hash(bytes) };
}
function assertSamePin(before, after) {
  if (before.bytes !== after.bytes || before.sha256 !== after.sha256) {
    throw new Error("Renderer optimization input/tool changed: " + before.path);
  }
}
export function assertRendererOutputAbsent(output) {
  ordinaryDirectory(path.dirname(output));
  try { fs.lstatSync(output); }
  catch (error) { if (error.code === "ENOENT") return; throw error; }
  throw new Error("Renderer optimization output already exists: " + output);
}

export function assertRendererReleaseSizes(sizes, label = "renderer") {
  for (const [name, limit] of Object.entries(RENDERER_RELEASE_LIMITS)) {
    const bytes = sizes[name];
    if (!Number.isSafeInteger(bytes) || bytes <= 0 || bytes > limit) {
      throw new Error(label + " " + name + " exceeds release byte budget or is invalid: " + bytes + " (max " + limit + ")");
    }
  }
  return Object.freeze({ wasmBytes: sizes.wasmBytes, gzipBytes: sizes.gzipBytes, jsBytes: sizes.jsBytes });
}
export function assertRendererReleaseBudget({ wasmBytes, jsBytes, label = "renderer" }) {
  // Whole generated WASM, not an estimated or stripped projection.
  return assertRendererReleaseSizes({ wasmBytes: wasmBytes.length,
    gzipBytes: gzipSync(wasmBytes).length, jsBytes: jsBytes.length }, label);
}
export function validateRendererReleasePackage({ wasmPath, jsPath, label }) {
  return assertRendererReleaseBudget({ wasmBytes: regularBytes(wasmPath), jsBytes: regularBytes(jsPath), label });
}

function moduleMetadata(bytes, label) {
  const module = new WebAssembly.Module(bytes);
  if (WebAssembly.Module.customSections(module, "name").length !== 0) {
    throw new Error(label + " release WASM contains a name section");
  }
  const normalize = (rows) => rows.map((row) => JSON.stringify(row)).sort();
  return { imports: normalize(WebAssembly.Module.imports(module)), exports: normalize(WebAssembly.Module.exports(module)) };
}
function checkedResult(result, label) {
  if (!result || result.error || result.status !== 0 || result.signal != null) {
    const reason = result?.error?.message ?? ("status=" + result?.status + " signal=" + result?.signal);
    const error = new Error("wasm-opt " + label + " failed: " + reason + "\n" + String(result?.stderr ?? "").trim());
    error.toolResult = result;
    throw error;
  }
  return { pid: result.pid ?? null, status: result.status, signal: result.signal ?? null,
    stdout: String(result.stdout ?? ""), stderr: String(result.stderr ?? "") };
}

function assertOwnedTemporaryOutput(root, input, output, profile) {
  if (!path.isAbsolute(root) || !path.isAbsolute(input) || !path.isAbsolute(output)) {
    throw new Error("Renderer cleanup paths must be absolute");
  }
  const relative = path.relative(root, output);
  if (!relative || relative === ".." || relative.startsWith(".." + path.sep) || path.isAbsolute(relative) ||
      path.dirname(output) !== path.dirname(input) || !profile.temporaryPattern.test(path.basename(output))) {
    throw new Error("Renderer cleanup output is outside its owned staged package");
  }
  ordinaryDirectory(root);
  ordinaryDirectory(path.dirname(output));
}

// Select only diagnostic fields. Never serialize env, options or arbitrary result objects.
export function formatRendererOptimizationError(error, seen = new Set()) {
  if (error == null || typeof error !== "object") return String(error);
  if (seen.has(error)) return "[repeated error]";
  seen.add(error);
  const lines = [typeof error.message === "string" ? error.message : String(error)];
  if (typeof error.code === "string") lines.push("code=" + error.code);
  const result = error.toolResult;
  if (result && typeof result === "object") {
    lines.push("tool status=" + String(result.status) + " signal=" + String(result.signal));
    if (result.error) lines.push("tool error: " + formatRendererOptimizationError(result.error, seen));
    if (typeof result.stderr === "string" && result.stderr.trim()) lines.push("tool stderr: " + result.stderr.trim());
  }
  if (error instanceof AggregateError) {
    for (const [index, nested] of Array.from(error.errors).entries()) {
      lines.push("cause[" + index + "]: " + formatRendererOptimizationError(nested, seen));
    }
  }
  if (error.cause) lines.push("cause: " + formatRendererOptimizationError(error.cause, seen));
  return lines.join("\n");
}

/** Only the caller's exclusively owned build staging tree may be supplied here. */
function optimizeReleaseWasm(profile, { wasmPath, jsPath, stagingRoot, toolConfig,
  run = spawnSync, sourceEnv = process.env, label = profile.label }) {
  const root = ordinaryDirectory(stagingRoot);
  if (!path.isAbsolute(wasmPath) || !path.isAbsolute(jsPath)) throw new Error("Renderer staged inputs must be absolute");
  const input = path.resolve(wasmPath), js = path.resolve(jsPath);
  const relative = path.relative(root, input);
  if (!relative || relative === ".." || relative.startsWith(".." + path.sep) || path.isAbsolute(relative) ||
      path.dirname(js) !== path.dirname(input) || path.basename(input) !== profile.wasmName ||
      path.basename(js) !== profile.jsName) throw new Error(profile.subject + " inputs must be the staged package pair");
  const config = profile.resolveConfig(toolConfig);
  const toolBefore = filePin(config.bin), inputBefore = filePin(input), jsBefore = filePin(js);
  if (toolBefore.sha256 !== config.sha256) throw new Error("wasm-opt executable SHA256 mismatch");
  const originalMetadata = moduleMetadata(regularBytes(input), "Original " + label);
  const output = path.join(path.dirname(input), profile.temporaryPrefix + process.pid + "-" + crypto.randomBytes(6).toString("hex") + ".wasm");
  assertRendererOutputAbsent(output);
  const env = {};
  for (const key of Object.keys(sourceEnv)) if (SYSTEM_ENV_NAMES.includes(key.toLowerCase())) env[key] = sourceEnv[key];
  env.BINARYEN_CORES = "1";
  const options = { cwd: root, env, shell: false, windowsHide: true, encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"], timeout: 600000, killSignal: "SIGTERM", maxBuffer: 1024 * 1024 };
  let failure = null, record = null;
  try {
    function runPinned(args, label) {
      let commandError = null;
      try {
        return checkedResult(run(config.bin, args, options), label);
      } catch (error) {
        commandError = error;
        throw error;
      } finally {
        // Every individual command attempts all three pins before a next command.
        const pinErrors = [];
        for (const before of [toolBefore, inputBefore, jsBefore]) {
          try { assertSamePin(before, filePin(before.path)); }
          catch (error) { pinErrors.push(error); }
        }
        if (pinErrors.length) throw new AggregateError(commandError ? [commandError, ...pinErrors] : pinErrors,
          "Renderer optimization post-pins failed after " + label);
      }
    }
    const version = runPinned(["--version"], "version");
    if (!/^wasm-opt version 131(?:[ \t].*)?$/.test(version.stdout.trim())) throw new Error("Expected Binaryen wasm-opt version131");
    assertRendererOutputAbsent(output);
    const optimization = runPinned([input, profile.optimizationFlag, "--strip-debug", "-o", output], "optimization");
    const optimizedBytes = regularBytes(output);
    const optimizedMetadata = moduleMetadata(optimizedBytes, "Optimized " + label);
    if (JSON.stringify(optimizedMetadata) !== JSON.stringify(originalMetadata)) {
      throw new Error("wasm-opt changed import/export metadata");
    }
    const budget = profile.assertBudget({ wasmBytes: optimizedBytes, jsBytes: regularBytes(js), label });
    const outputPin = filePin(output);
    // Recheck immediately before the only mutation of the staged input.
    assertSamePin(toolBefore, filePin(config.bin));
    assertSamePin(inputBefore, filePin(input));
    assertSamePin(jsBefore, filePin(js));
    ordinaryDirectory(root);
    ordinaryDirectory(path.dirname(input));
    assertSamePin(outputPin, filePin(output));
    fs.renameSync(output, input);
    record = { tool: toolBefore, input: inputBefore, output: { ...outputPin, path: input },
      budget, version, optimization, metadataMatched: true, rendererInstantiated: false };
  } catch (error) { failure = error; }
  finally {
    try {
      // Check ancestry again after the child; a replaced ancestor must never be followed.
      assertOwnedTemporaryOutput(root, input, output, profile);
      try { fs.unlinkSync(output); }
      catch (error) { if (error.code !== "ENOENT") throw error; }
    } catch (error) {
      const cleanupError = new Error("Renderer temporary cleanup refused or failed; residual may remain: " + output, { cause: error });
      failure = failure ? new AggregateError([failure, cleanupError], "Renderer optimization and temporary cleanup failed") : cleanupError;
    }
  }
  if (failure) throw failure;
  return record;
}

/** Retain the renderer's existing API, limits, diagnostics and exact behavior. */
export function optimizeRendererReleaseWasm(options) {
  return optimizeReleaseWasm(RENDERER_PROFILE, options);
}
export function optimizeClientCoreReleaseWasm(options) {
  return optimizeReleaseWasm(CLIENT_CORE_PROFILE, options);
}
