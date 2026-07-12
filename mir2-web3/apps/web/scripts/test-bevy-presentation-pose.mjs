import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

const sourceUrl = new URL("../app/components/original-client-presentation-pose.ts", import.meta.url);
const shellUrl = new URL("../app/original-client-shell.tsx", import.meta.url);
const sourcePath = fileURLToPath(sourceUrl);
const source = readFileSync(sourceUrl, "utf8");
const compilerOptions = {
  module: ts.ModuleKind.CommonJS,
  target: ts.ScriptTarget.ES2022,
  strict: true,
  skipLibCheck: true,
};
const program = ts.createProgram([sourcePath], { ...compilerOptions, noEmit: true });
const typeErrors = ts
  .getPreEmitDiagnostics(program)
  .filter((diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error);
assert.deepEqual(
  typeErrors,
  [],
  typeErrors.map((diagnostic) => ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n")).join("\n"),
);

const compiled = ts.transpileModule(source, { compilerOptions, fileName: sourcePath });
const module = { exports: {} };
new Function("exports", "module", compiled.outputText)(module.exports, module);
const {
  BEVY_PRESENTATION_POSE_MAX_ENTITIES,
  compareBevyPresentationPoseProvenance,
  parseBevyPresentationPoseFrame,
  readBevyPresentationPoseFrame,
} = module.exports;

let passed = 0;
function test(label, fn) {
  fn();
  passed += 1;
  console.log(`ok ${passed} - ${label}`);
}

function frame(overrides = {}) {
  return {
    ready: true,
    version: 1,
    frameId: 7,
    generatedAtMs: 10_000,
    bridgeEnabled: true,
    rendererEnabled: true,
    camera: { x: 40, y: 16, source: "selfWindow" },
    provenance: {
      appliedMapRevision: 12,
      mapCenter: { x: 332, y: 275 },
      entityCenter: { x: 332, y: 275 },
    },
    entities: [
      { objectId: "remote", x: -40, y: -16, source: "remotePacket" },
      { objectId: "self", x: -40, y: -16, source: "snapshotWindow" },
    ],
    frameOverflowCount: 0,
    totalOverflowCount: 0,
    ...overrides,
  };
}

test("parses a fresh frame into constant-time entity lookup", () => {
  const parsed = parseBevyPresentationPoseFrame(JSON.stringify(frame()), 10_120);
  assert.equal(parsed.frameId, 7);
  assert.equal(parsed.ageMs, 120);
  assert.deepEqual(parsed.camera, { x: 40, y: 16, source: "selfWindow" });
  assert.deepEqual(parsed.entities.get("remote"), {
    x: -40,
    y: -16,
    source: "remotePacket",
    motion: null,
  });
  assert.deepEqual(parsed.provenance, {
    mapRevision: 12,
    mapCenter: { x: 332, y: 275 },
    entityCenter: { x: 332, y: 275 },
  });
});

test("compares applied map and entity provenance before an atomic DOM commit", () => {
  const parsed = parseBevyPresentationPoseFrame(JSON.stringify(frame()), 10_100);
  assert.equal(
    compareBevyPresentationPoseProvenance(parsed, {
      mapRevision: 12,
      mapCenter: { x: 332, y: 275 },
      entityCenter: { x: 332, y: 275 },
    }),
    "match",
  );
  assert.equal(
    compareBevyPresentationPoseProvenance(parsed, {
      mapRevision: 11,
      mapCenter: { x: 332, y: 275 },
      entityCenter: { x: 332, y: 275 },
    }),
    "mapRevisionMismatch",
  );
  assert.equal(
    compareBevyPresentationPoseProvenance(parsed, {
      mapRevision: 12,
      mapCenter: { x: 331, y: 275 },
      entityCenter: { x: 331, y: 275 },
    }),
    "mapCenterMismatch",
  );
  assert.equal(
    compareBevyPresentationPoseProvenance(parsed, {
      mapRevision: 12,
      mapCenter: { x: 332, y: 275 },
      entityCenter: { x: 331, y: 275 },
    }),
    "internalCenterMismatch",
  );
  const internallySplit = parseBevyPresentationPoseFrame(
    JSON.stringify(
      frame({
        provenance: {
          appliedMapRevision: 12,
          mapCenter: { x: 332, y: 275 },
          entityCenter: { x: 331, y: 275 },
        },
      }),
    ),
    10_100,
  );
  assert.equal(
    compareBevyPresentationPoseProvenance(internallySplit, {
      mapRevision: 12,
      mapCenter: { x: 332, y: 275 },
      entityCenter: { x: 332, y: 275 },
    }),
    "internalCenterMismatch",
  );
  assert.equal(
    compareBevyPresentationPoseProvenance(parsed, {
      mapRevision: 12,
      mapCenter: { x: 332, y: 275 },
      entityCenter: { x: 331, y: 275 },
    }),
    "internalCenterMismatch",
  );
});

test("keeps legacy frames readable but marks missing provenance unavailable", () => {
  const parsed = parseBevyPresentationPoseFrame(
    JSON.stringify(
      frame({
        provenance: undefined,
      }),
    ),
    10_100,
  );
  assert.deepEqual(parsed.provenance, { mapRevision: null, mapCenter: null, entityCenter: null });
  assert.equal(
    compareBevyPresentationPoseProvenance(parsed, {
      mapRevision: 12,
      mapCenter: { x: 332, y: 275 },
      entityCenter: { x: 332, y: 275 },
    }),
    "unavailable",
  );
});

test("submits map and entity producers as one complete layout transaction", () => {
  const shellSource = readFileSync(shellUrl, "utf8");
  assert.equal(
    (shellSource.match(/onBevyMapRenderStateChange\(bevyMapRenderState\)/g) ?? []).length,
    1,
  );
  assert.equal(
    (shellSource.match(/onBevyEntityRenderStateChange\(bevyEntityRenderState\)/g) ?? []).length,
    1,
  );
  assert.match(
    shellSource,
    /useLayoutEffect\(\(\) => \{[\s\S]*?onBevyMapRenderStateChange\(bevyMapRenderState\)[\s\S]*?onBevyEntityRenderStateChange\(bevyEntityRenderState\)[\s\S]*?if \(completeScene\) \{[\s\S]*?submittedPresentationContextRef\.current = \{/,
  );
  assert.doesNotMatch(shellSource, /\.\.\.submittedPresentationContextRef\.current/);
});

test("accepts local-command camera and self pose ownership", () => {
  const parsed = parseBevyPresentationPoseFrame(
    JSON.stringify(
      frame({
        camera: { x: 40, y: 0, source: "localCommand" },
        entities: [
          {
            objectId: "self",
            x: -40,
            y: 0,
            source: "localCommand",
            motion: { frameIndex: 0, mode: "walk", direction: "Right" },
          },
        ],
      }),
    ),
    10_120,
  );
  assert.deepEqual(parsed?.camera, { x: 40, y: 0, source: "localCommand" });
  assert.deepEqual(parsed?.entities.get("self"), {
    x: -40,
    y: 0,
    source: "localCommand",
    motion: { frameIndex: 0, phaseCount: 6, mode: "walk", direction: "Right" },
  });
});

test("accepts all eight mounted-walk presentation phases", () => {
  const parsed = parseBevyPresentationPoseFrame(
    JSON.stringify(
      frame({
        camera: { x: 6, y: 0, source: "localCommand" },
        entities: [
          {
            objectId: "self",
            x: -6,
            y: 0,
            source: "localCommand",
            motion: { frameIndex: 7, phaseCount: 8, mode: "walk", direction: "Right" },
          },
        ],
      }),
    ),
    10_120,
  );
  assert.deepEqual(parsed?.entities.get("self")?.motion, {
    frameIndex: 7,
    phaseCount: 8,
    mode: "walk",
    direction: "Right",
  });
});

test("preserves the runtime method receiver", () => {
  const runtime = {
    getMir2PresentationPoses() {
      assert.equal(this, runtime);
      return JSON.stringify(frame());
    },
  };
  assert.equal(readBevyPresentationPoseFrame(runtime, 10_100)?.frameId, 7);
});

test("rejects stale, unrelated future-clock, disabled, and not-ready frames", () => {
  assert.equal(parseBevyPresentationPoseFrame(JSON.stringify(frame()), 10_251), null);
  assert.equal(parseBevyPresentationPoseFrame(JSON.stringify(frame()), 8_999), null);
  assert.equal(parseBevyPresentationPoseFrame(JSON.stringify(frame({ rendererEnabled: false })), 10_100), null);
  assert.equal(parseBevyPresentationPoseFrame(JSON.stringify(frame({ bridgeEnabled: false })), 10_100), null);
  assert.equal(parseBevyPresentationPoseFrame(JSON.stringify(frame({ ready: false })), 10_100), null);
});

test("rejects malformed offsets, sources, duplicates, and oversized frames", () => {
  assert.equal(
    parseBevyPresentationPoseFrame(
      JSON.stringify(frame({ entities: [{ objectId: "bad", x: null, y: 0, source: "static" }] })),
      10_100,
    ),
    null,
  );
  assert.equal(
    parseBevyPresentationPoseFrame(
      JSON.stringify(frame({ entities: [{ objectId: "bad", x: 0, y: 0, source: "authority" }] })),
      10_100,
    ),
    null,
  );
  assert.equal(
    parseBevyPresentationPoseFrame(
      JSON.stringify(
        frame({
          entities: [
            {
              objectId: "bad-motion",
              x: 0,
              y: 0,
              source: "localCommand",
              motion: { frameIndex: 6, mode: "walk", direction: "Right" },
            },
          ],
        }),
      ),
      10_100,
    ),
    null,
  );
  assert.equal(
    parseBevyPresentationPoseFrame(
      JSON.stringify(frame({ entities: [frame().entities[0], frame().entities[0]] })),
      10_100,
    ),
    null,
  );
  assert.equal(
    parseBevyPresentationPoseFrame(
      JSON.stringify(frame({ provenance: { ...frame().provenance, mapCenter: { x: 332 } } })),
      10_100,
    ),
    null,
  );
  const entities = Array.from({ length: BEVY_PRESENTATION_POSE_MAX_ENTITIES + 1 }, (_, index) => ({
    objectId: `p${index}`,
    x: 0,
    y: 0,
    source: "static",
  }));
  assert.equal(parseBevyPresentationPoseFrame(JSON.stringify(frame({ entities })), 10_100), null);
});

test("missing APIs, invalid JSON, and runtime exceptions safely fall back", () => {
  assert.equal(readBevyPresentationPoseFrame(null, 10_100), null);
  assert.equal(readBevyPresentationPoseFrame({}, 10_100), null);
  assert.equal(readBevyPresentationPoseFrame({ getMir2PresentationPoses: () => "{" }, 10_100), null);
  assert.equal(
    readBevyPresentationPoseFrame(
      {
        getMir2PresentationPoses() {
          throw new Error("runtime unavailable");
        },
      },
      10_100,
    ),
    null,
  );
});

console.log(`bevy presentation pose tests passed (${passed})`);
