import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/bevy-runtime-policy.ts", import.meta.url));
const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: sourcePath,
  reportDiagnostics: true,
});
const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, []);
const loadedModule = { exports: {} };
new Function("exports", "module", "require", compiled.outputText)(
  loadedModule.exports,
  loadedModule,
  () => ({}),
);

const {
  isBevyRuntimeNetworkFailure,
  resolveBevyRuntimeBootDecision,
  shouldRetryBevyRuntimeWithWebGl2,
} = loadedModule.exports;

const params = (values = {}) => ({
  get: (key) => values[key] ?? null,
});

const desktop = {
  layout: "desktop",
  input: "keyboardMouse",
  coarsePointer: false,
  maxTouchPoints: 0,
  userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
  params: params(),
};

test("desktop keeps the Bevy runtime enhancement enabled", () => {
  assert.deepEqual(resolveBevyRuntimeBootDecision(desktop), {
    mode: "eager",
    reason: "desktop-default",
  });
});

test("touch-first phones and tablets use the immediately playable compatibility renderer", () => {
  assert.equal(
    resolveBevyRuntimeBootDecision({
      ...desktop,
      layout: "touch",
      input: "touch",
      coarsePointer: true,
      maxTouchPoints: 5,
      userAgent: "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) Mobile",
    }).mode,
    "compatibility",
  );
  assert.equal(
    resolveBevyRuntimeBootDecision({
      ...desktop,
      maxTouchPoints: 5,
      userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15) Version/18 Safari",
    }).mode,
    "compatibility",
    "iPadOS desktop user agents must still avoid the startup WASM download",
  );
});

test("a fine-pointer desktop with a touch screen is not misclassified as mobile", () => {
  assert.equal(
    resolveBevyRuntimeBootDecision({
      ...desktop,
      maxTouchPoints: 10,
      userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    }).mode,
    "eager",
  );
});

test("QA query controls override automatic device policy", () => {
  assert.equal(
    resolveBevyRuntimeBootDecision({
      ...desktop,
      layout: "touch",
      input: "touch",
      coarsePointer: true,
      params: params({ bevyRuntime: "1" }),
    }).mode,
    "eager",
  );
  assert.equal(
    resolveBevyRuntimeBootDecision({
      ...desktop,
      params: params({ bevyRuntime: "0" }),
    }).mode,
    "disabled",
  );
  assert.equal(
    resolveBevyRuntimeBootDecision({
      ...desktop,
      params: params({ skipRuntime: "1", bevyRuntime: "1" }),
    }).mode,
    "disabled",
    "the emergency skip switch has highest priority",
  );
});

test("network failures never trigger a second full runtime download", () => {
  for (const error of [
    new TypeError("Load failed"),
    new TypeError("Failed to fetch dynamically imported module"),
    new DOMException("The operation was aborted", "AbortError"),
  ]) {
    assert.equal(isBevyRuntimeNetworkFailure(error), true);
    assert.equal(shouldRetryBevyRuntimeWithWebGl2("webgpu", true, error), false);
  }

  const gpuError = new Error("Unable to find a GPU");
  assert.equal(isBevyRuntimeNetworkFailure(gpuError), false);
  assert.equal(shouldRetryBevyRuntimeWithWebGl2("webgpu", true, gpuError), true);
  assert.equal(shouldRetryBevyRuntimeWithWebGl2("webgl2", true, gpuError), false);
});
