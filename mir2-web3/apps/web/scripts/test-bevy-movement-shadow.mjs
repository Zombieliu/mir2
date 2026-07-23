import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourceUrl = new URL("../lib/bevy-movement-shadow.ts", import.meta.url);
const sourcePath = fileURLToPath(sourceUrl);
const source = readFileSync(sourceUrl, "utf8");
const compilerOptions = {
  module: ts.ModuleKind.CommonJS,
  target: ts.ScriptTarget.ES2022,
  strict: true,
  skipLibCheck: true,
};

const program = ts.createProgram([sourcePath], {
  ...compilerOptions,
  noEmit: true,
});
const typeErrors = ts
  .getPreEmitDiagnostics(program)
  .filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error);
assert.deepEqual(
  typeErrors,
  [],
  typeErrors.map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n")).join("\n"),
);

const compiled = ts.transpileModule(source, {
  compilerOptions,
  fileName: sourcePath,
  reportDiagnostics: true,
});
const transpileErrors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(transpileErrors, []);

const module = { exports: {} };
const load = new Function("exports", "module", compiled.outputText);
load(module.exports, module);

const {
  BEVY_MOVEMENT_SHADOW_COUNTER_MAX,
  createBevyMovementShadowBridge,
  serializeBevyMovementShadowEvent,
} = module.exports;

let passed = 0;
function test(label, fn) {
  fn();
  passed += 1;
  console.log(`ok ${passed} - ${label}`);
}

const events = [
  {
    type: "reset",
    atMs: 1_000,
    objectId: "41",
    x: 288,
    y: 634,
    direction: "DownLeft",
  },
  {
    type: "intent",
    atMs: 1_010,
    direction: "Right",
    mode: "walk",
    fromX: 288,
    fromY: 634,
    toX: 289,
    toY: 634,
  },
  {
    type: "commandSent",
    atMs: 1_020,
    direction: "DownRight",
    mode: "run",
    fromX: 289,
    fromY: 634,
    toX: 291,
    toY: 636,
    phaseCount: 6,
  },
  {
    type: "authoritative",
    atMs: 1_100,
    packet: "UserLocation",
    objectId: "41",
    isSelf: true,
    x: 291,
    y: 636,
    direction: "DownRight",
    tsPredictedX: 291,
    tsPredictedY: 636,
    tsDisposition: "confirmed",
  },
  {
    type: "remoteMotion",
    atMs: 1_120,
    packet: "ObjectWalk",
    objectId: "92",
    fromX: 300,
    fromY: 640,
    toX: 299,
    toY: 639,
    direction: "UpLeft",
    mode: "walk",
    phaseCount: 8,
  },
  {
    type: "clear",
    atMs: 1_125,
  },
  {
    type: "remoteRemove",
    atMs: 1_130,
    objectId: "92",
  },
];

test("serializes every event schema with a type tag and camelCase fields", () => {
  for (const event of events) {
    const parsed = JSON.parse(serializeBevyMovementShadowEvent(event));
    assert.deepEqual(parsed, event);
    assert.equal(parsed.type, event.type);
    assert.equal("tag" in parsed, false);
  }
});

test("omits absent authoritative optionals and strips accidental fields", () => {
  const parsed = JSON.parse(
    serializeBevyMovementShadowEvent({
      type: "authoritative",
      atMs: 2_000,
      packet: "ObjectTurn",
      objectId: "92",
      isSelf: false,
      x: 299,
      y: 639,
      direction: "Left",
      accidental: "must-not-cross-the-bridge",
    }),
  );
  assert.deepEqual(parsed, {
    type: "authoritative",
    atMs: 2_000,
    packet: "ObjectTurn",
    objectId: "92",
    isSelf: false,
    x: 299,
    y: 639,
    direction: "Left",
  });
});

test("submits exact JSON and preserves runtime method receivers", () => {
  const received = [];
  const runtimeDiagnostics = { fixedStepMs: 100, compared: 5 };
  const presentationDiagnostics = { enabled: true, activeSegmentCount: 2 };
  const localPresentationDiagnostics = { comparisonSampleCount: 7, comparisonMismatchCount: 0 };
  const runtime = {
    pushMir2MovementShadowEvent(json) {
      assert.equal(this, runtime);
      received.push(JSON.parse(json));
    },
    getMir2MovementShadowDiagnostics() {
      assert.equal(this, runtime);
      return runtimeDiagnostics;
    },
    getMir2RemoteMotionPresentationDiagnostics() {
      assert.equal(this, runtime);
      return JSON.stringify(presentationDiagnostics);
    },
    getMir2LocalMotionDiagnostics() {
      assert.equal(this, runtime);
      return JSON.stringify(localPresentationDiagnostics);
    },
  };
  const bridge = createBevyMovementShadowBridge(runtime);

  for (const event of events) bridge.push(event);

  assert.deepEqual(received, events);
  assert.deepEqual(bridge.getDiagnostics(), {
    submitted: events.length,
    dropped: 0,
    errors: 0,
    lastEventType: "remoteRemove",
  });
  assert.equal(bridge.getRuntimeDiagnostics(), runtimeDiagnostics);
  assert.deepEqual(bridge.getPresentationDiagnostics(), presentationDiagnostics);
  assert.deepEqual(bridge.getLocalPresentationDiagnostics(), localPresentationDiagnostics);
});

test("parses JSON diagnostics returned by the wasm runtime", () => {
  const bridge = createBevyMovementShadowBridge({
    getMir2MovementShadowDiagnostics() {
      return JSON.stringify({ fixedIntervalMs: 100, fixedTickCount: 4 });
    },
  });
  assert.deepEqual(bridge.getRuntimeDiagnostics(), {
    fixedIntervalMs: 100,
    fixedTickCount: 4,
  });
});

test("missing runtime and missing optional methods are safe no-ops", () => {
  const absent = createBevyMovementShadowBridge();
  const methodMissing = createBevyMovementShadowBridge({});

  assert.doesNotThrow(() => absent.push(events[0]));
  assert.doesNotThrow(() => methodMissing.push(events[1]));
  assert.equal(absent.getRuntimeDiagnostics(), null);
  assert.equal(methodMissing.getRuntimeDiagnostics(), null);
  assert.equal(absent.getPresentationDiagnostics(), null);
  assert.equal(methodMissing.getPresentationDiagnostics(), null);
  assert.equal(absent.getLocalPresentationDiagnostics(), null);
  assert.equal(methodMissing.getLocalPresentationDiagnostics(), null);
  assert.deepEqual(absent.getDiagnostics(), {
    submitted: 0,
    dropped: 1,
    errors: 0,
    lastEventType: "reset",
  });
  assert.deepEqual(methodMissing.getDiagnostics(), {
    submitted: 0,
    dropped: 1,
    errors: 0,
    lastEventType: "intent",
  });
});

test("runtime and serialization exceptions cannot escape into movement", () => {
  const throwingRuntime = {
    pushMir2MovementShadowEvent() {
      throw new Error("push failed");
    },
    getMir2MovementShadowDiagnostics() {
      throw new Error("diagnostics failed");
    },
  };
  const bridge = createBevyMovementShadowBridge(throwingRuntime);

  assert.doesNotThrow(() => bridge.push(events[2]));
  assert.equal(bridge.getRuntimeDiagnostics(), null);
  assert.deepEqual(bridge.getDiagnostics(), {
    submitted: 0,
    dropped: 1,
    errors: 2,
    lastEventType: "commandSent",
  });

  let runtimeCalls = 0;
  const serializationBridge = createBevyMovementShadowBridge({
    pushMir2MovementShadowEvent() {
      runtimeCalls += 1;
    },
  });
  const malformed = {
    type: "reset",
    get atMs() {
      throw new Error("bad event getter");
    },
    objectId: "1",
    x: 1,
    y: 1,
    direction: "Up",
  };
  assert.doesNotThrow(() => serializationBridge.push(malformed));
  assert.equal(runtimeCalls, 0);
  assert.deepEqual(serializationBridge.getDiagnostics(), {
    submitted: 0,
    dropped: 1,
    errors: 1,
    lastEventType: "reset",
  });
});

test("dynamic runtime resolution is isolated and diagnostics stay bounded", () => {
  let runtime = null;
  const bridge = createBevyMovementShadowBridge(() => runtime, { maxCounterValue: 2 });

  for (let index = 0; index < 4; index += 1) bridge.push(events[0]);
  runtime = { pushMir2MovementShadowEvent() {} };
  for (let index = 0; index < 4; index += 1) bridge.push(events[1]);
  runtime = {
    pushMir2MovementShadowEvent() {
      throw new Error("bounded failure");
    },
  };
  for (let index = 0; index < 4; index += 1) bridge.push(events[4]);

  const diagnostics = bridge.getDiagnostics();
  assert.deepEqual(diagnostics, {
    submitted: 2,
    dropped: 2,
    errors: 2,
    lastEventType: "remoteMotion",
  });
  assert.deepEqual(Object.keys(diagnostics).sort(), ["dropped", "errors", "lastEventType", "submitted"]);
  assert.equal(Object.isFrozen(diagnostics), true);
});

test("resolver failures are swallowed and bounded like runtime failures", () => {
  const bridge = createBevyMovementShadowBridge(
    () => {
      throw new Error("runtime unavailable");
    },
    { maxCounterValue: 1 },
  );

  assert.doesNotThrow(() => bridge.push(events[0]));
  assert.doesNotThrow(() => bridge.push(events[1]));
  assert.equal(bridge.getRuntimeDiagnostics(), null);
  assert.deepEqual(bridge.getDiagnostics(), {
    submitted: 0,
    dropped: 1,
    errors: 1,
    lastEventType: "intent",
  });
});

test("default counter maximum is a finite safe integer", () => {
  assert.equal(BEVY_MOVEMENT_SHADOW_COUNTER_MAX, Number.MAX_SAFE_INTEGER);
  assert.equal(Number.isSafeInteger(BEVY_MOVEMENT_SHADOW_COUNTER_MAX), true);
});

console.log(`bevy movement shadow tests passed (${passed})`);
