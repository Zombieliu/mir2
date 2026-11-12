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
  canRetryBevyRuntimeBeforeBoot,
  resolveBevyRuntimeBootDecision,
  shouldRetryBevyRuntimeWithWebGl2,
} = loadedModule.exports;

const params = (values = {}) => ({
  get: (key) => values[key] ?? null,
});

test("only network failures before the Bevy boot remain retryable", () => {
  for (const error of [new TypeError("Failed to fetch runtime binary (HTTP 503)"),
    new DOMException("The operation was aborted", "AbortError"), new Error("importing a module script failed")]) {
    assert.equal(canRetryBevyRuntimeBeforeBoot(false, error), true);
    assert.equal(canRetryBevyRuntimeBeforeBoot(true, error), false);
  }
  for (const error of [new Error("Runtime compiled backend disagrees with selected package"), new Error("GPU adapter unavailable")]) {
    assert.equal(canRetryBevyRuntimeBeforeBoot(false, error), false);
    assert.equal(canRetryBevyRuntimeBeforeBoot(true, error), false);
  }
});

test("the actual page retry gate preserves the single-boot boundary", () => {
  const page = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
  const start = page.indexOf("if (runtimeWindow.__mir2BevyRuntimeBootAttempted ||");
  const end = page.indexOf("const runtimeSupport =", start);
  assert(start >= 0 && end > start);
  const gate = new Function("runtimeWindow", "markMir2CacheMilestone", page.slice(start, end));
  for (const state of [
    { __mir2BevyRuntimeBootAttempted: true, __mir2BevyRuntimeFailed: true, __mir2BevyRuntimeRetryable: true },
    { __mir2BevyRuntimeBootAttempted: true, __mir2BevyRuntimeFailed: false },
    { __mir2BevyRuntimeFailed: true, __mir2BevyRuntimeRetryable: false },
  ]) assert.throws(() => gate(state, () => {}), /reload to retry/);
  const state = { __mir2BevyRuntimeFailed: true, __mir2BevyRuntimeRetryable: true }, marks = [];
  gate(state, (name, detail) => marks.push({ name, detail }));
  assert.equal(state.__mir2BevyRuntimeFailed, false); assert.equal(state.__mir2BevyRuntimeRetryable, false);
  assert.deepEqual(marks, [{ name: "bevyRuntimeRetry", detail: { reason: "network-before-boot" } }]);
  gate(state, () => assert.fail("a repeated render must not manufacture another retry"));
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

// Run the Page's actual configuration and failure blocks. URL intent and the
// current runtime's availability have independent lifetimes; a same-backend
// retry does not rerun the canvas-selection effect.
const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const pageAst = ts.createSourceFile("page.tsx", pageSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const findPageNodes = predicate => {
  const found = [];
  const visit = node => { if (predicate(node)) found.push(node); ts.forEachChild(node, visit); };
  visit(pageAst); return found;
};
const uiConfiguration = findPageNodes(node => ts.isCallExpression(node) && node.expression.getText(pageAst) === "useEffect"
  && ts.isArrowFunction(node.arguments[0]) && node.arguments[0].body.getText(pageAst).includes("const requests = resolveSharedUiRequests"));
assert.equal(uiConfiguration.length, 1);
const pageBoot = findPageNodes(node => ts.isFunctionDeclaration(node) && node.name?.text === "bootRuntime");
assert.equal(pageBoot.length, 1);
const pageFailure = pageBoot[0].body.statements.find(ts.isTryStatement)?.catchClause?.block;
assert(pageFailure);
const projections = findPageNodes(node => ts.isVariableDeclaration(node)
  && ["bevyQuestUiRequested", "bevyBagUiRequested"].includes(node.name.getText(pageAst)));
assert.equal(projections.length, 2);
function runPageBlock(text, scope, result = "") {
  const javascript = ts.transpileModule(text, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), javascript + result)(...Object.values(scope));
}
const sharedModule = { exports: {} };
new Function("exports", "module", ts.transpileModule(readFileSync(new URL("../lib/bevy-shared-canvas-mode.ts", import.meta.url), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText)(sharedModule.exports, sharedModule);
function pageFailureFixture(search = "", strict = false, bootAttempted = false) {
  const state = { quest: false, bag: false, started: true, generation: 1, phase: "running", withdrawals: 0 };
  const window = { location: { search }, __mir2BevyRuntimeBootAttempted: bootAttempted };
  const runtimeRef = { current: { clearMir2StatusSink() {} } };
  const scope = { disposed: false, fontPrewarmController: new AbortController(),
    error: new DOMException("runtime download interrupted", "AbortError"), window, runtimeRef,
    webGl2SharedCanvasPrototype: strict, resolveSharedUiRequests: sharedModule.exports.resolveSharedUiRequests,
    setBevyQuestUiRequested: value => state.quest = value, setBevyBagUiRequested: value => state.bag = value,
    setBevyRuntimeGeneration: update => state.generation = update(state.generation),
    withdrawHudBarDrawPlanOutputs: () => state.withdrawals++, resetBevyMapImageResidency() {},
    setRuntimePhase: value => state.phase = value, setRuntimeMessage() {},
    setBevyRuntimeStarted: value => state.started = value, setBevyEntityRendererReady() {}, setBevyRuntimeBackend() {},
    canRetryBevyRuntimeBeforeBoot, isBevyRuntimeNetworkFailure, markMir2CacheMilestone() {},
    scheduleBevyRuntimeCacheRecovery: () => false, console: { warn() {}, info() {} } };
  runPageBlock(uiConfiguration[0].arguments[0].body.getText(pageAst), scope);
  return { state, scope, window, runtimeRef,
    fail: () => runPageBlock(pageFailure.getText(pageAst), scope),
    requested: () => runPageBlock(projections.map(node => "const " + node.getText(pageAst) + ";").join("\n"), {
      bevyQuestUiRequestedByUrl: state.quest, bevyBagUiRequestedByUrl: state.bag,
      currentUiRequests: sharedModule.exports.resolveSharedUiRequests(search, strict),
    }, "return { quest: bevyQuestUiRequested, bag: bevyBagUiRequested };") };
}

test("Page keeps default shared UI intent across a pre-boot failure on the same backend", () => {
  const f = pageFailureFixture();
  assert.deepEqual(f.requested(), { quest: true, bag: true });
  f.fail();
  assert.equal(f.window.__mir2BevyRuntimeRetryable, true);
  assert.equal(f.runtimeRef.current, null);
  assert.equal(f.state.started, false);
  assert.equal(f.state.withdrawals, 1);
  assert.deepEqual(f.requested(), { quest: true, bag: true }, "a later successful WebGPU retry must still request its original HUD");
});

test("Page failure preserves explicit disabled and mixed shared UI choices", () => {
  for (const [search, strict, expected] of [
    ["?bevyQuestUi=0&bevyBagUi=0", false, { quest: false, bag: false }],
    ["?bevyQuestUi=1&bevyBagUi=0", false, { quest: true, bag: false }],
    ["?bevyBackend=webgl2&bevySharedCanvas=1&bevyQuestUi=1&bevyBagUi=0", true, { quest: true, bag: false }],
  ]) {
    const f = pageFailureFixture(search, strict);
    assert.deepEqual(f.requested(), expected);
    f.fail();
    assert.deepEqual(f.requested(), expected, search);
  }
});

test("preserving Page UI intent does not turn a failed App boot into an automatic retry", () => {
  const f = pageFailureFixture("", false, true);
  f.fail();
  assert.equal(f.window.__mir2BevyRuntimeRetryable, false);
  assert.equal(f.window.__mir2BevyRuntimeBootAttempted, true);
  assert.equal(f.runtimeRef.current, null);
  assert.equal(f.state.started, false);
  assert.deepEqual(f.requested(), { quest: true, bag: true });
});

test("a disposed Page attempt cannot withdraw the newer runtime or its UI choices", () => {
  const f = pageFailureFixture();
  const runtime = f.runtimeRef.current;
  f.scope.disposed = true;
  f.fail();
  assert.equal(f.runtimeRef.current, runtime);
  assert.equal(f.state.generation, 1);
  assert.equal(f.state.withdrawals, 0);
  assert.deepEqual(f.requested(), { quest: true, bag: true });
});

test('an indeterminate wasm-bindgen initialization requires a fresh document for either backend',()=>{
  const error=Object.assign(new TypeError('Failed to fetch after initializer entry'),{runtimeInitializationUncertain:true});
  assert.equal(canRetryBevyRuntimeBeforeBoot(false,error),false);
  assert.equal(shouldRetryBevyRuntimeWithWebGl2('webgpu',true,error),false);
  const fixture=pageFailureFixture();fixture.scope.error=error;fixture.fail();
  assert.equal(fixture.window.__mir2BevyRuntimeRetryable,false);
});
