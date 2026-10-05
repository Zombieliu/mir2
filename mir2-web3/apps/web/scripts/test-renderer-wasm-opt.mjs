import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import crypto from "node:crypto";
import { gzipSync } from "node:zlib";
import test from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { RENDERER_RELEASE_LIMITS, resolveRendererWasmOptConfig, assertRendererOutputAbsent,
  assertRendererReleaseSizes, assertRendererReleaseBudget, validateRendererReleasePackage,
  optimizeRendererReleaseWasm, formatRendererOptimizationError, CLIENT_CORE_RELEASE_LIMITS,
  resolveClientCoreWasmOptConfig, assertClientCoreReleaseSizes, assertClientCoreReleaseBudget,
  validateClientCoreReleasePackage, assertClientCoreStagingRoot, optimizeClientCoreReleaseWasm } from "./lib/renderer-wasm-opt.mjs";

const EMPTY = Buffer.from([0, 97, 115, 109, 1, 0, 0, 0]);
const NOTE = Buffer.concat([EMPTY, Buffer.from([0, 6, 4, 110, 111, 116, 101, 1])]);
const NAMED = Buffer.concat([EMPTY, Buffer.from([0, 5, 4, 110, 97, 109, 101])]);
const EXPORTED = Buffer.concat([EMPTY, Buffer.from([
  1, 4, 1, 96, 0, 0, 3, 2, 1, 0,
  7, 5, 1, 1, 102, 0, 0, 10, 4, 1, 2, 0, 11,
])]);
const IMPORTED = Buffer.concat([EMPTY, Buffer.from([
  1, 4, 1, 96, 0, 0, 2, 7, 1, 1, 97, 1, 120, 0, 0,
])]);
const sha = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const ok = (stdout = "") => ({ pid: 12345, status: 0, signal: null, error: null, stdout, stderr: "" });
function fixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-renderer-opt-test-"));
  const stagingRoot = path.join(root, "owned-staging"), packageDir = path.join(stagingRoot, "pkg-webgl2-shared");
  fs.mkdirSync(packageDir, { recursive: true });
  const wasmPath = path.join(packageDir, "mir2_bevy_runtime_bg.wasm"), jsPath = path.join(packageDir, "mir2_bevy_runtime.js");
  const bin = path.join(root, "wasm-opt-fixture.exe"), canonical = path.join(root, "published-before.wasm");
  fs.writeFileSync(wasmPath, NOTE); fs.writeFileSync(jsPath, "export const fixture = true;\n");
  fs.writeFileSync(bin, "fake tool bytes; only injected runner is called");
  fs.writeFileSync(canonical, "published pointer remains independent");
  t.after(() => {
    const absolute = path.resolve(root), relative = path.relative(os.tmpdir(), absolute);
    assert(relative && relative !== ".." && !relative.startsWith(".." + path.sep) && !path.isAbsolute(relative));
    fs.rmSync(absolute, { recursive: true, force: true });
  });
  return { root, stagingRoot, packageDir, wasmPath, jsPath, bin, canonical, sourceEnv: {},
    toolConfig: { bin, sha256: sha(fs.readFileSync(bin)) } };
}
function runner(bytes = EMPTY, onOptimize = null) {
  return (bin, args, options) => {
    if (args.length === 1 && args[0] === "--version") return ok("wasm-opt version 131 (version_131)\n");
    assert.equal(args[1], "-O1"); assert.equal(args[2], "--strip-debug"); assert.equal(args[3], "-o");
    fs.writeFileSync(args[4], bytes, { flag: "wx" });
    return onOptimize ? onOptimize(bin, args, options) : ok();
  };
}
function unchanged(f) {
  assert.deepEqual(fs.readFileSync(f.wasmPath), NOTE);
  assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
  assert.deepEqual(fs.readdirSync(f.packageDir).sort(), ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"]);
}
function linkOrSkip(t, target, destination, type = "file") {
  try { fs.symlinkSync(target, destination, type); return true; }
  catch (error) {
    if (process.platform === "win32" && ["EPERM", "EACCES"].includes(error.code)) {
      t.skip("Windows symlink creation not permitted: " + error.code); return false;
    }
    throw error;
  }
}

test("renderer optimizer requires both explicit absolute executable and exact SHA256", () => {
  assert.throws(() => resolveRendererWasmOptConfig({}), /MIR2_BEVY_WASM_OPT_BIN/);
  assert.throws(() => resolveRendererWasmOptConfig({ MIR2_BEVY_WASM_OPT_BIN: "wasm-opt", MIR2_BEVY_WASM_OPT_SHA256: "a".repeat(64) }), /absolute/);
  for (const bad of [undefined, "", "a".repeat(63), "A".repeat(64), "x".repeat(64)]) {
    assert.throws(() => resolveRendererWasmOptConfig({ MIR2_BEVY_WASM_OPT_BIN: path.resolve("tool"), MIR2_BEVY_WASM_OPT_SHA256: bad }), /SHA256/);
  }
  assert.deepEqual(resolveRendererWasmOptConfig({ MIR2_BEVY_WASM_OPT_BIN: path.resolve("tool"), MIR2_BEVY_WASM_OPT_SHA256: "a".repeat(64) }),
    { bin: path.resolve("tool"), sha256: "a".repeat(64) });
});

test("renderer tool hash refusal occurs before any injected command", (t) => {
  const f = fixture(t); let calls = 0;
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, toolConfig: { bin: f.bin, sha256: "0".repeat(64) }, run: () => { calls++; throw Error("must not run"); } }), /SHA256 mismatch/);
  assert.equal(calls, 0); unchanged(f);
});

test("renderer optimizer accepts only actual Binaryen131 version output", (t) => {
  for (const stdout of ["wasm-opt version 130\n", "wasm-opt version 1310\n", "131", "", "warning\nwasm-opt version 131\n"]) {
    const f = fixture(t); let calls = 0;
    assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: () => { calls++; return ok(stdout); } }), /version131/);
    assert.equal(calls, 1); unchanged(f);
  }
});

test("renderer optimizer refuses version command status signal and error failures", (t) => {
  for (const outcome of [{ ...ok(), status: 1 }, { ...ok(), signal: "SIGTERM" }, { ...ok(), status: null, error: Object.assign(Error("timeout"), { code: "ETIMEDOUT" }) }]) {
    const f = fixture(t);
    assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: () => outcome }), /version failed/);
    unchanged(f);
  }
});

test("renderer optimization uses only fixed conservative arguments and bounded hidden child options", (t) => {
  const f = fixture(t), calls = [], injected = runner();
  const result = optimizeRendererReleaseWasm({ ...f, sourceEnv: { SystemRoot: "fixture-system", NODE_OPTIONS: "forbidden", PATH: "untrusted", BINARYEN_CORES: "9" },
    run: (bin, args, options) => { calls.push({ bin, args, options }); return injected(bin, args, options); } });
  assert.equal(calls.length, 2);
  assert.equal(calls[0].bin, f.bin); assert.deepEqual(calls[0].args, ["--version"]);
  assert.equal(calls[1].bin, f.bin); assert.equal(calls[1].args[0], f.wasmPath);
  assert.deepEqual(calls[1].args.slice(1, 4), ["-O1", "--strip-debug", "-o"]);
  assert.equal(path.dirname(calls[1].args[4]), f.packageDir);
  for (const call of calls) {
    assert.equal(call.options.cwd, f.stagingRoot); assert.equal(call.options.shell, false);
    assert.equal(call.options.windowsHide, true); assert.equal(call.options.timeout, 600000);
    assert.equal(call.options.killSignal, "SIGTERM"); assert.equal(call.options.maxBuffer, 1048576);
    assert.deepEqual(call.options.env, { SystemRoot: "fixture-system", BINARYEN_CORES: "1" });
  }
  assert.deepEqual(fs.readFileSync(f.wasmPath), EMPTY);
  assert.equal(result.input.sha256, sha(NOTE)); assert.equal(result.output.sha256, sha(EMPTY));
  assert.equal(result.tool.sha256, f.toolConfig.sha256); assert.equal(result.metadataMatched, true);
  assert.equal(result.version.status, 0); assert.equal(result.optimization.status, 0);
  assert.equal(result.rendererInstantiated, false); assert.equal(result.budget.wasmBytes, EMPTY.length);
  assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
  assert.deepEqual(fs.readdirSync(f.packageDir).sort(), ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"]);
});

test("renderer optimization lifecycle failure never renames staged input or published pointer", (t) => {
  for (const outcome of [{ ...ok(), status: 2 }, { ...ok(), signal: "SIGTERM" }, { ...ok(), status: null, error: Object.assign(Error("ETIMEDOUT"), { code: "ETIMEDOUT" }) }]) {
    const f = fixture(t);
    assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: runner(EMPTY, () => outcome) }), /optimization failed/);
    unchanged(f);
  }
});

test("renderer synchronous runner IO throws are propagated with staged input intact", (t) => {
  const f = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: () => { throw Error("injected spawn IO failure"); } }), /spawn IO failure/);
  unchanged(f);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: (bin, args) => {
    if (args[0] === "--version") return ok("wasm-opt version 131\n");
    throw Error("injected optimization IO failure");
  } }), /optimization IO failure/);
  unchanged(f);
});

test("renderer missing or invalid optimized WASM cannot replace staged bytes", (t) => {
  const missing = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...missing, run: () => ok("wasm-opt version 131\n") }), /ENOENT/);
  unchanged(missing);
  const invalid = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...invalid, run: runner(Buffer.from("not wasm")) }));
  unchanged(invalid);
});

test("renderer optimized name section is refused before staged rename", (t) => {
  const f = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: runner(NAMED) }), /name section/);
  unchanged(f);
});

test("renderer import and export metadata changes are independently refused", (t) => {
  for (const output of [IMPORTED, EXPORTED]) {
    const f = fixture(t);
    assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: runner(output) }), /import\/export metadata/);
    unchanged(f);
  }
});

test("renderer release numeric byte caps retain inclusive boundaries for all three limits", () => {
  const atCap = { ...RENDERER_RELEASE_LIMITS };
  assert.deepEqual(assertRendererReleaseSizes(atCap), atCap);
  for (const name of Object.keys(atCap)) {
    assert.throws(() => assertRendererReleaseSizes({ ...atCap, [name]: atCap[name] + 1 }), /byte budget/);
    for (const bad of [0, -1, NaN, Infinity, 1.5]) assert.throws(() => assertRendererReleaseSizes({ ...atCap, [name]: bad }), /invalid/);
  }
});

test("renderer budget measures full actual WASM gzip and JavaScript bytes", (t) => {
  const f = fixture(t);
  const wasm = fs.readFileSync(f.wasmPath), js = fs.readFileSync(f.jsPath);
  assert.deepEqual(assertRendererReleaseBudget({ wasmBytes: wasm, jsBytes: js }),
    { wasmBytes: wasm.length, gzipBytes: gzipSync(wasm).length, jsBytes: js.length });
  assert.deepEqual(validateRendererReleasePackage({ wasmPath: f.wasmPath, jsPath: f.jsPath, label: "fixture" }),
    assertRendererReleaseBudget({ wasmBytes: wasm, jsBytes: js }));
  assert.throws(() => assertRendererReleaseBudget({ wasmBytes: Buffer.alloc(RENDERER_RELEASE_LIMITS.wasmBytes + 1), jsBytes: js }), /wasmBytes/);
  assert.throws(() => assertRendererReleaseBudget({ wasmBytes: wasm, jsBytes: Buffer.alloc(RENDERER_RELEASE_LIMITS.jsBytes + 1) }), /jsBytes/);
});

test("renderer optimized output over JavaScript cap does not rename the original WASM", (t) => {
  const f = fixture(t); fs.writeFileSync(f.jsPath, Buffer.alloc(RENDERER_RELEASE_LIMITS.jsBytes + 1));
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: runner() }), /jsBytes/);
  unchanged(f);
});

test("renderer post-pins reject changed tool and prevent staged replacement", (t) => {
  const f = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: runner(EMPTY, () => { fs.writeFileSync(f.bin, "changed-tool"); return ok(); }) }), /post-pins failed/);
  unchanged(f);
});

test("renderer post-pins detect input drift and never publish optimized output", (t) => {
  const f = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: runner(EMPTY, () => { fs.writeFileSync(f.wasmPath, NAMED); return ok(); }) }), /post-pins failed/);
  assert.deepEqual(fs.readFileSync(f.wasmPath), NAMED);
  assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
  assert.deepEqual(fs.readdirSync(f.packageDir).sort(), ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"]);
});

test("renderer output absence rejects existing ordinary and dangling linked leaves", (t) => {
  const f = fixture(t), output = path.join(f.packageDir, "owned-output.wasm");
  fs.writeFileSync(output, "already exists");
  assert.throws(() => assertRendererOutputAbsent(output), /already exists/);
  fs.unlinkSync(output);
  if (!linkOrSkip(t, path.join(f.root, "missing-target.wasm"), output)) return;
  assert.throws(() => assertRendererOutputAbsent(output), /already exists/);
  assert.equal(fs.lstatSync(output).isSymbolicLink(), true);
});

test("renderer temp output symlink is refused without following its target", (t) => {
  const f = fixture(t), target = path.join(f.root, "missing-external-target.wasm");
  if (process.platform === "win32") {
    const probe = path.join(f.root, "link-permission-probe");
    if (!linkOrSkip(t, target, probe)) return;
    fs.unlinkSync(probe);
  }
  const run = (bin, args) => {
    if (args[0] === "--version") return ok("wasm-opt version 131\n");
    fs.symlinkSync(target, args[4], "file");
    return ok();
  };
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run }), /linked or irregular/);
  assert.equal(fs.existsSync(target), false); unchanged(f);
});

test("renderer linked tool or package directory cannot be followed", (t) => {
  const f = fixture(t), linked = path.join(f.root, "linked-tool.exe");
  if (!linkOrSkip(t, f.bin, linked)) return;
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, toolConfig: { bin: linked, sha256: f.toolConfig.sha256 }, run: runner() }), /linked or irregular/);
  unchanged(f);
  const linkedRoot = path.join(f.root, "linked-staging");
  if (!linkOrSkip(t, f.stagingRoot, linkedRoot, process.platform === "win32" ? "junction" : "dir")) return;
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, stagingRoot: linkedRoot,
    wasmPath: path.join(linkedRoot, "pkg-webgl2-shared", "mir2_bevy_runtime_bg.wasm"),
    jsPath: path.join(linkedRoot, "pkg-webgl2-shared", "mir2_bevy_runtime.js"), run: runner() }), /linked or irregular/);
  unchanged(f);
});

test("renderer optimizer refuses inputs outside its explicitly owned staging root", (t) => {
  const f = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, stagingRoot: f.packageDir, wasmPath: path.join(f.root, "mir2_bevy_runtime_bg.wasm"), run: runner() }), /staged package pair/);
  unchanged(f);
});


test("renderer post-pins detect JavaScript drift before replacing WASM", (t) => {
  const f = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: runner(EMPTY, () => { fs.writeFileSync(f.jsPath, "changed-js"); return ok(); }) }), /post-pins failed/);
  unchanged(f);
  assert.equal(fs.readFileSync(f.jsPath, "utf8"), "changed-js");
});

test("renderer simultaneous runner and post-pin failures preserve both error causes", (t) => {
  const f = fixture(t);
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: () => {
    fs.writeFileSync(f.bin, "changed-tool");
    throw Error("primary IO error");
  } }), (error) => {
    assert.equal(error instanceof AggregateError, true);
    assert.equal(error.errors.length, 2);
    assert.match(error.errors[0].message, /primary IO error/);
    assert.match(error.errors[1].message, /input\/tool changed/);
    return true;
  });
  unchanged(f);
});


test("renderer version post-pins reject each drift before any optimization command", (t) => {
  for (const key of ["bin", "wasmPath", "jsPath"]) {
    const f = fixture(t); let calls = 0;
    const drift = key === "wasmPath" ? NAMED : Buffer.from("version-stage-drift");
    assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: (bin, args) => {
      calls++;
      assert.deepEqual(args, ["--version"]);
      fs.writeFileSync(f[key], drift);
      return ok("wasm-opt version 131\n");
    } }), /post-pins failed after version/);
    assert.equal(calls, 1);
    assert.deepEqual(fs.readFileSync(f.wasmPath), key === "wasmPath" ? drift : NOTE);
    assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
    assert.deepEqual(fs.readdirSync(f.packageDir).sort(), ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"]);
  }
});

test("renderer version throw still attempts all three post-pins and blocks optimization", (t) => {
  const f = fixture(t); let calls = 0;
  assert.throws(() => optimizeRendererReleaseWasm({ ...f, run: () => {
    calls++;
    fs.writeFileSync(f.bin, "drift-tool"); fs.writeFileSync(f.wasmPath, NAMED); fs.writeFileSync(f.jsPath, "drift-js");
    throw Object.assign(Error("injected version IO"), { code: "EIO" });
  } }), (error) => {
    assert.equal(error instanceof AggregateError, true);
    assert.equal(error.errors.length, 4);
    const diagnostic = formatRendererOptimizationError(error);
    assert.match(diagnostic, /injected version IO/); assert.match(diagnostic, /code=EIO/);
    for (const file of [f.bin, f.wasmPath, f.jsPath]) assert(diagnostic.includes(file));
    return true;
  });
  assert.equal(calls, 1);
  assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
});

test("renderer cleanup refuses retargeted package ancestry and preserves outside same-name leaf", (t) => {
  const f = fixture(t), saved = path.join(f.stagingRoot, "original-package-saved"), outside = path.join(f.root, "outside-staging");
  fs.mkdirSync(outside);
  const probe = path.join(f.root, "directory-link-permission-probe"), linkType = process.platform === "win32" ? "junction" : "dir";
  if (!linkOrSkip(t, outside, probe, linkType)) return;
  fs.unlinkSync(probe);
  let generated, outsideLeaf, failure;
  try {
    optimizeRendererReleaseWasm({ ...f, run: (bin, args) => {
      if (args[0] === "--version") return ok("wasm-opt version 131\n");
      generated = args[4];
      fs.writeFileSync(generated, EMPTY, { flag: "wx" });
      fs.renameSync(f.packageDir, saved);
      outsideLeaf = path.join(outside, path.basename(generated));
      fs.writeFileSync(outsideLeaf, "outside-owned-staging-sentinel", { flag: "wx" });
      fs.symlinkSync(outside, f.packageDir, linkType);
      return { ...ok(), status: null, signal: "SIGTERM",
        error: Object.assign(Error("tool timeout"), { code: "ETIMEDOUT" }), stderr: "actual fixture stderr" };
    } });
    assert.fail("retargeted ancestor must refuse output and cleanup");
  } catch (error) { failure = error; }
  try {
    assert.equal(failure instanceof AggregateError, true);
    assert.equal(fs.readFileSync(outsideLeaf, "utf8"), "outside-owned-staging-sentinel");
    assert.deepEqual(fs.readFileSync(path.join(saved, path.basename(generated))), EMPTY);
    assert.deepEqual(fs.readFileSync(path.join(saved, "mir2_bevy_runtime_bg.wasm")), NOTE);
    const diagnostic = formatRendererOptimizationError(failure);
    for (const reason of ["ETIMEDOUT", "status=null", "signal=SIGTERM", "actual fixture stderr",
      "post-pins failed after optimization", "linked or irregular", "cleanup refused or failed", "residual may remain"]) {
      assert(diagnostic.includes(reason), reason + " must survive builder-facing diagnostics");
    }
    assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
  } finally {
    if (fs.lstatSync(f.packageDir).isSymbolicLink()) fs.unlinkSync(f.packageDir);
    fs.renameSync(saved, f.packageDir);
  }
  assert.deepEqual(fs.readFileSync(generated), EMPTY);
  fs.unlinkSync(generated);
  unchanged(f);
});

test("renderer diagnostic formatter preserves nested tool failures without serializing environment values", () => {
  const childError = Object.assign(Error("child timeout"), { code: "ETIMEDOUT" });
  const toolFailure = Object.assign(Error("command failure"), { toolResult: {
    status: null, signal: "SIGTERM", error: childError, stderr: "fixture stderr", env: { SECRET: "never-print-env-value" },
    options: { env: { SECRET: "never-print-option-value" } }, stdout: "never-print-arbitrary-stdout",
  } });
  const cleanupFailure = new Error("cleanup refused; residual retained", { cause: Error("linked ancestor") });
  const formatted = formatRendererOptimizationError(new AggregateError([toolFailure, Error("post-pin drift"), cleanupFailure], "outer aggregate"));
  for (const reason of ["outer aggregate", "command failure", "status=null", "signal=SIGTERM", "ETIMEDOUT", "fixture stderr",
    "post-pin drift", "cleanup refused; residual retained", "linked ancestor"]) assert(formatted.includes(reason));
  for (const excluded of ["never-print-env-value", "never-print-option-value", "never-print-arbitrary-stdout"]) assert.equal(formatted.includes(excluded), false);
});


// Core uses the same private lifecycle machinery with a fixed pair/profile;
// these runners write only owned test fixtures and never execute a real tool.
function coreFixture(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-core-opt-test-"));
  const stagingRoot = fs.mkdtempSync(path.join(root, "mir2-client-core-")), packageDir = stagingRoot;
  const wasmPath = path.join(packageDir, "mir2_platform_web_bg.wasm"), jsPath = path.join(packageDir, "mir2_platform_web.js");
  const bin = path.join(root, "wasm-opt-fixture.exe"), canonical = path.join(root, "published-before.wasm");
  fs.writeFileSync(wasmPath, NOTE); fs.writeFileSync(jsPath, "export const fixture = true;\n");
  fs.writeFileSync(bin, "fake tool bytes; only injected runner is called"); fs.writeFileSync(canonical, "published pointer remains independent");
  t.after(() => {
    const absolute = path.resolve(root), relative = path.relative(os.tmpdir(), absolute);
    assert(relative && relative !== ".." && !relative.startsWith(".." + path.sep) && !path.isAbsolute(relative));
    fs.rmSync(absolute, { recursive: true, force: true });
  });
  return { root, stagingRoot, packageDir, wasmPath, jsPath, bin, canonical, sourceEnv: {},
    toolConfig: { bin, sha256: sha(fs.readFileSync(bin)) } };
}
function coreUnchanged(f) {
  assert.deepEqual(fs.readFileSync(f.wasmPath), NOTE);
  assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
  assert.deepEqual(fs.readdirSync(f.packageDir).sort(), ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"]);
}

test("Core optimizer requires its own absolute tool and SHA config without renderer or PATH fallback", () => {
  for (const env of [{}, { MIR2_BEVY_WASM_OPT_BIN: path.resolve("tool"), MIR2_BEVY_WASM_OPT_SHA256: "a".repeat(64) },
    { PATH: "untrusted", MIR2_CLIENT_CORE_WASM_OPT_BIN: "wasm-opt", MIR2_CLIENT_CORE_WASM_OPT_SHA256: "a".repeat(64) }]) {
    assert.throws(() => resolveClientCoreWasmOptConfig(env), /MIR2_CLIENT_CORE_WASM_OPT_BIN/);
  }
  for (const bad of [undefined, "", "A".repeat(64), "a".repeat(63), "x".repeat(64)]) {
    assert.throws(() => resolveClientCoreWasmOptConfig({ MIR2_CLIENT_CORE_WASM_OPT_BIN: path.resolve("tool"), MIR2_CLIENT_CORE_WASM_OPT_SHA256: bad }), /SHA256/);
  }
  const config = resolveClientCoreWasmOptConfig({ MIR2_CLIENT_CORE_WASM_OPT_BIN: path.resolve("tool"), MIR2_CLIENT_CORE_WASM_OPT_SHA256: "a".repeat(64) });
  assert(Object.isFrozen(config)); assert.deepEqual(config, { bin: path.resolve("tool"), sha256: "a".repeat(64) });
});

test("Core release size gate is strictly below256KiB and retains inclusive200KiB JavaScript", () => {
  assert.deepEqual(CLIENT_CORE_RELEASE_LIMITS, { wasmBytesExclusive: 262144, jsBytes: 204800 });
  assert.deepEqual(assertClientCoreReleaseSizes({ wasmBytes: 262143, jsBytes: 204800 }), { wasmBytes: 262143, jsBytes: 204800 });
  assert.throws(() => assertClientCoreReleaseSizes({ wasmBytes: 262144, jsBytes: 1 }), /wasmBytes/);
  assert.throws(() => assertClientCoreReleaseSizes({ wasmBytes: 1, jsBytes: 204801 }), /jsBytes/);
  for (const name of ["wasmBytes", "jsBytes"]) for (const bad of [0, -1, 1.5, NaN, Infinity]) {
    assert.throws(() => assertClientCoreReleaseSizes({ wasmBytes: 1, jsBytes: 1, [name]: bad }), /invalid/);
  }
});

test("Core budget measures whole actual pair and never applies the larger renderer budget", (t) => {
  const f = coreFixture(t), wasm = fs.readFileSync(f.wasmPath), js = fs.readFileSync(f.jsPath);
  assert.deepEqual(assertClientCoreReleaseBudget({ wasmBytes: wasm, jsBytes: js }), { wasmBytes: wasm.length, jsBytes: js.length });
  assert.deepEqual(validateClientCoreReleasePackage(f), { wasmBytes: wasm.length, jsBytes: js.length });
  assert.throws(() => assertClientCoreReleaseBudget({ wasmBytes: Buffer.alloc(262144), jsBytes: js }), /wasmBytes/);
  assert.throws(() => assertClientCoreReleaseBudget({ wasmBytes: wasm, jsBytes: Buffer.alloc(204801) }), /jsBytes/);
  coreUnchanged(f);
});

test("Core optimizer uses fixed131 O1 strip-debug hidden single-core commands and pins final actual bytes", (t) => {
  const f = coreFixture(t), calls = [], injected = runner();
  const result = optimizeClientCoreReleaseWasm({ ...f, sourceEnv: { SystemRoot: "fixture-system", NODE_OPTIONS: "not-forwarded", PATH: "untrusted", BINARYEN_CORES: "9" },
    run: (bin, args, options) => { calls.push({ bin, args, options }); return injected(bin, args, options); } });
  assert.equal(calls.length, 2); assert.deepEqual(calls[0].args, ["--version"]);
  assert.deepEqual(calls[1].args.slice(0, 4), [f.wasmPath, "-O1", "--strip-debug", "-o"]);
  assert.match(path.basename(calls[1].args[4]), /^\.mir2-client-core-opt-\d+-[a-f0-9]{12}\.wasm$/);
  assert.equal(path.dirname(calls[1].args[4]), f.stagingRoot);
  for (const call of calls) {
    assert.equal(call.bin, f.bin); assert.equal(call.options.cwd, f.stagingRoot);
    assert.equal(call.options.shell, false); assert.equal(call.options.windowsHide, true);
    assert.equal(call.options.timeout, 600000); assert.equal(call.options.killSignal, "SIGTERM");
    assert.equal(call.options.maxBuffer, 1048576); assert.deepEqual(call.options.env, { SystemRoot: "fixture-system", BINARYEN_CORES: "1" });
  }
  assert.deepEqual(fs.readFileSync(f.wasmPath), EMPTY);
  assert.equal(result.tool.sha256, f.toolConfig.sha256); assert.equal(result.input.sha256, sha(NOTE)); assert.equal(result.output.sha256, sha(EMPTY));
  assert.equal(result.version.status, 0); assert.equal(result.optimization.status, 0); assert.equal(result.metadataMatched, true);
  assert.equal(result.rendererInstantiated, false); assert.deepEqual(result.budget, { wasmBytes: EMPTY.length, jsBytes: fs.readFileSync(f.jsPath).length });
  assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
  assert.deepEqual(fs.readdirSync(f.packageDir).sort(), ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"]);
});

test("Core refuses wrong tool hash and non131 version before any optimization", (t) => {
  const hashMismatch = coreFixture(t); let calls = 0;
  assert.throws(() => optimizeClientCoreReleaseWasm({ ...hashMismatch, toolConfig: { bin: hashMismatch.bin, sha256: "0".repeat(64) }, run: () => { calls++; assert.fail("must not run"); } }), /SHA256 mismatch/);
  assert.equal(calls, 0); coreUnchanged(hashMismatch);
  for (const stdout of ["wasm-opt version 130\n", "wasm-opt version 1310\n", "warning\nwasm-opt version 131\n"]) {
    const f = coreFixture(t); calls = 0;
    assert.throws(() => optimizeClientCoreReleaseWasm({ ...f, run: () => { calls++; return ok(stdout); } }), /version131/);
    assert.equal(calls, 1); coreUnchanged(f);
  }
});

test("Core version and optimization failures never replace staged input or a published pointer", (t) => {
  for (const stage of ["version", "optimization"]) for (const failure of [
    { ...ok(), status: 1 }, { ...ok(), signal: "SIGTERM" },
    { ...ok(), status: null, error: Object.assign(Error("controlled timeout"), { code: "ETIMEDOUT" }), stderr: "controlled stderr" },
  ]) {
    const f = coreFixture(t), optimize = runner(EMPTY, () => failure);
    assert.throws(() => optimizeClientCoreReleaseWasm({ ...f, run: (bin, args, options) => {
      if (stage === "version") return failure;
      return optimize(bin, args, options);
    } }), /failed/);
    coreUnchanged(f);
  }
});

test("Core command postpins detect version drift before optimization and optimization drift before rename", (t) => {
  for (const stage of ["version", "optimization"]) for (const key of ["bin", "wasmPath", "jsPath"]) {
    const f = coreFixture(t), calls = [];
    const injected = runner(EMPTY, () => { fs.writeFileSync(f[key], key === "wasmPath" ? NAMED : "drift"); return ok(); });
    assert.throws(() => optimizeClientCoreReleaseWasm({ ...f, run: (bin, args, options) => {
      calls.push(args);
      if (stage === "version") { fs.writeFileSync(f[key], key === "wasmPath" ? NAMED : "drift"); return ok("wasm-opt version 131\n"); }
      return injected(bin, args, options);
    } }), /post-pins failed/);
    assert.equal(calls.length, stage === "version" ? 1 : 2);
    assert.deepEqual(fs.readFileSync(f.wasmPath), key === "wasmPath" ? NAMED : NOTE);
    assert.equal(fs.readFileSync(f.canonical, "utf8"), "published pointer remains independent");
    assert.deepEqual(fs.readdirSync(f.packageDir).sort(), ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"]);
  }
});

test("Core refuses metadata drift debugging names invalid output and fixed-cap overflow before rename", (t) => {
  // A padding-only custom section: no renderer functions or WASM Instance.
  const atWasmCap = Buffer.concat([EMPTY, Buffer.from([0, 0xf4, 0xff, 0x0f, 0]), Buffer.alloc(262131)]);
  assert.equal(atWasmCap.length, 262144);
  for (const output of [IMPORTED, EXPORTED, NAMED, Buffer.from("not wasm"), atWasmCap]) {
    const f = coreFixture(t);
    if (output === atWasmCap) assert.throws(() => optimizeClientCoreReleaseWasm({ ...f, run: runner(output) }), /wasmBytes/);
    else assert.throws(() => optimizeClientCoreReleaseWasm({ ...f, run: runner(output) }));
    coreUnchanged(f);
  }
  const overJs = coreFixture(t); fs.writeFileSync(overJs.jsPath, Buffer.alloc(204801));
  assert.throws(() => optimizeClientCoreReleaseWasm({ ...overJs, run: runner() }), /jsBytes/); coreUnchanged(overJs);
});

test("Core and renderer wrappers cannot be broadened by caller profile or basename options", (t) => {
  const core = coreFixture(t), renderer = fixture(t); let calls = 0;
  const run = () => { calls++; assert.fail("must not run"); };
  assert.throws(() => optimizeClientCoreReleaseWasm({ ...renderer, run }), /staged package pair/);
  assert.throws(() => optimizeRendererReleaseWasm({ ...core, run }), /staged package pair/);
  assert.equal(calls, 0); coreUnchanged(core); unchanged(renderer);
  fs.writeFileSync(core.jsPath, Buffer.alloc(204801));
  assert.throws(() => optimizeClientCoreReleaseWasm({ ...core, profile: "renderer", limits: RENDERER_RELEASE_LIMITS, run: runner() }), /jsBytes/);
  coreUnchanged(core);
});

test("Core output symlink is refused without following its target", (t) => {
  const f = coreFixture(t), target = path.join(f.root, "outside-output.wasm"); fs.writeFileSync(target, "external sentinel");
  const probe = path.join(f.root, "permission-probe"); if (!linkOrSkip(t, target, probe)) return; fs.unlinkSync(probe);
  assert.throws(() => optimizeClientCoreReleaseWasm({ ...f, run: (bin, args) => {
    if (args[0] === "--version") return ok("wasm-opt version 131\n");
    fs.symlinkSync(target, args[4], "file"); return ok();
  } }), /linked or irregular/);
  assert.equal(fs.readFileSync(target, "utf8"), "external sentinel"); coreUnchanged(f);
});

test("Core recursive cleanup validates direct generated name and every ordinary ancestor", (t) => {
  const f = coreFixture(t); assert.equal(assertClientCoreStagingRoot(f.stagingRoot, f.root), f.stagingRoot);
  for (const invalid of [f.root, path.join(f.root, "mir2-client-core-invalid"), path.join(f.stagingRoot, "mir2-client-core-ABC123")]) {
    assert.throws(() => assertClientCoreStagingRoot(invalid, f.root), /Refusing cleanup/);
  }
  const linked = path.join(f.root, "mir2-client-core-ABC123");
  if (!linkOrSkip(t, f.stagingRoot, linked, process.platform === "win32" ? "junction" : "dir")) return;
  assert.throws(() => assertClientCoreStagingRoot(linked, f.root), /linked or irregular/);
  coreUnchanged(f);
});

test("Core cleanup rejects a retargeted staging ancestor and retains tool postpin and cleanup causes", (t) => {
  const f = coreFixture(t), saved = path.join(f.root, "saved-staging"), outside = path.join(f.root, "outside"); fs.mkdirSync(outside);
  const probe = path.join(f.root, "directory-permission-probe"), type = process.platform === "win32" ? "junction" : "dir";
  if (!linkOrSkip(t, outside, probe, type)) return; fs.unlinkSync(probe);
  let generated, outsideLeaf, failure;
  try {
    optimizeClientCoreReleaseWasm({ ...f, run: (bin, args) => {
      if (args[0] === "--version") return ok("wasm-opt version 131\n");
      generated = args[4]; fs.writeFileSync(generated, EMPTY, { flag: "wx" }); fs.renameSync(f.stagingRoot, saved);
      outsideLeaf = path.join(outside, path.basename(generated)); fs.writeFileSync(outsideLeaf, "outside sentinel", { flag: "wx" });
      fs.symlinkSync(outside, f.stagingRoot, type);
      return { ...ok(), status: null, signal: "SIGTERM", error: Object.assign(Error("tool timeout"), { code: "ETIMEDOUT" }), stderr: "controlled stderr" };
    } });
    assert.fail("retargeted ancestry must refuse");
  } catch (error) { failure = error; }
  try {
    assert(failure instanceof AggregateError);
    assert.equal(fs.readFileSync(outsideLeaf, "utf8"), "outside sentinel");
    assert.deepEqual(fs.readFileSync(path.join(saved, path.basename(generated))), EMPTY);
    assert.deepEqual(fs.readFileSync(path.join(saved, "mir2_platform_web_bg.wasm")), NOTE);
    const message = formatRendererOptimizationError(failure);
    for (const expected of ["ETIMEDOUT", "SIGTERM", "controlled stderr", "post-pins failed", "cleanup refused or failed", "residual may remain"]) assert(message.includes(expected));
    assert.throws(() => assertClientCoreStagingRoot(f.stagingRoot, f.root), /linked or irregular/);
  } finally {
    if (fs.lstatSync(f.stagingRoot).isSymbolicLink()) fs.unlinkSync(f.stagingRoot);
    fs.renameSync(saved, f.stagingRoot);
  }
  fs.unlinkSync(generated); coreUnchanged(f);
});

test("actual Core sourceFingerprint includes optimizer bytes and retains LF normalization sorted old recipe", () => {
  const source = fs.readFileSync(new URL("./build-client-core.mjs", import.meta.url), "utf8");
  const start = source.indexOf("function sourceFingerprint() {"), end = source.indexOf("\nfunction run(", start);
  assert(start >= 0 && end > start); const declaration = source.slice(start, end);
  assert.equal((declaration.match(/import\.meta\.url/g) ?? []).length, 2);
  // Only this complete function runs, against an exact virtual source tree.
  const executable = declaration.replaceAll("import.meta.url", "builderUrl");
  const projectRoot = path.resolve("virtual-core-fingerprint"), webRoot = path.join(projectRoot, "apps", "web"), crateRoot = path.join(projectRoot, "apps", "game-client", "platform-web"), coreRoot = path.join(projectRoot, "apps", "game-client", "client-core");
  const builderFile = path.join(webRoot, "scripts", "build-client-core.mjs"), optimizerFile = path.join(webRoot, "scripts", "lib", "renderer-wasm-opt.mjs");
  const data = new Map([
    [path.join(crateRoot, "Cargo.toml"), "platform crate\n"], [path.join(crateRoot, "Cargo.lock"), "locked\n"],
    [path.join(crateRoot, "rust-toolchain.toml"), "pinned toolchain\n"], [path.join(coreRoot, "Cargo.toml"), "core crate\n"],
    [builderFile, "builder bytes\n"], [optimizerFile, "optimizer bytes\n"],
    [path.join(coreRoot, "src", "z.rs"), "z\n"], [path.join(coreRoot, "src", "a.rs"), "a\n"],
    [path.join(coreRoot, "src", "nested", "b.rs"), "b\n"],
    [path.join(crateRoot, "src", "z.rs"), "web z\n"], [path.join(crateRoot, "src", "a.rs"), "web a\n"],
  ]);
  let reverse = false, hashInput;
  const dirent = (name, directory = false) => ({ name, isDirectory: () => directory, isFile: () => !directory });
  const directoryRows = new Map([[path.join(coreRoot, "src"), [dirent("z.rs"), dirent("nested", true), dirent("a.rs")]],
    [path.join(coreRoot, "src", "nested"), [dirent("b.rs")]], [path.join(crateRoot, "src"), [dirent("z.rs"), dirent("a.rs")]]]);
  const fakeFs = { readdirSync(directory, options) { assert.deepEqual(options, { withFileTypes: true }); assert(directoryRows.has(directory)); const rows = directoryRows.get(directory); return reverse ? [...rows].reverse() : [...rows]; },
    readFileSync(filename, encoding) { assert.equal(encoding, "utf8"); assert(data.has(filename), "unexpected source read " + filename); return data.get(filename); } };
  const run = new Function("fs", "path", "crateRoot", "webRoot", "fileURLToPath", "hash", "builderUrl", executable + "\nreturn sourceFingerprint();");
  const invoke = () => run(fakeFs, path, crateRoot, webRoot, fileURLToPath, json => { hashInput = json; return sha(Buffer.from(json)); }, pathToFileURL(builderFile).href);
  const baseline = invoke(), expected = [...data].map(([filename, text]) => ({ name: path.relative(projectRoot, filename).split(path.sep).join("/"), text })).sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  assert.equal(hashInput, JSON.stringify(expected)); assert.equal(baseline, sha(Buffer.from(JSON.stringify(expected))));
  reverse = true; assert.equal(invoke(), baseline);
  for (const [filename, text] of data) data.set(filename, text.replace(/\n/g, "\r\n"));
  assert.equal(invoke(), baseline); // Existing LF-normalized byte recipe.
  data.set(optimizerFile, "optimizer changed\r\n"); assert.notEqual(invoke(), baseline);
  assert.equal(JSON.parse(hashInput).filter(row => row.name === "apps/web/scripts/lib/renderer-wasm-opt.mjs").length, 1);
});
