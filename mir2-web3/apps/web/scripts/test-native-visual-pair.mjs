import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { deflateSync } from "node:zlib";

import {
  buildAcceptanceGate,
  buildReviewArgs,
  parsePairArgs,
  validateReviewGate,
  verifyVisualPair,
} from "./verify-native-visual-pair.mjs";

function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngChunk(type, data) {
  const typeBytes = Buffer.from(type, "ascii");
  const result = Buffer.alloc(12 + data.length);
  result.writeUInt32BE(data.length, 0);
  typeBytes.copy(result, 4);
  data.copy(result, 8);
  result.writeUInt32BE(crc32(Buffer.concat([typeBytes, data])), 8 + data.length);
  return result;
}

function pngImage(width = 1024, height = 768, marker = 0) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  const rowBytes = width * 4;
  const pixels = Buffer.alloc((rowBytes + 1) * height);
  if (marker) pixels[1] = marker;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", deflateSync(pixels)),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

async function createFixture(directory, overrides = {}) {
  const referenceImage = path.join(directory, "reference.png");
  const candidateImage = path.join(directory, "candidate.png");
  await fs.writeFile(referenceImage, pngImage());
  await fs.writeFile(candidateImage, pngImage(1024, 768, 1));
  const referenceHash = crypto.createHash("sha256").update(await fs.readFile(referenceImage)).digest("hex");
  const candidateHash = crypto.createHash("sha256").update(await fs.readFile(candidateImage)).digest("hex");
  const common = {
    schemaVersion: "mir2-native-visual-capture-v1",
    runId: "vis-pair-test-001",
    scene: "in-game",
    capturedAt: "2026-08-24T00:00:00.000Z",
    logicalSize: { width: 1024, height: 768 },
    dpiScale: 1,
    uiState: "in-game-default-hud",
    world: { map: "0", x: 287, y: 618, light: "day" },
  };
  const referenceState = {
    ...common,
    producer: "crystal-original",
    imagePath: "reference.png",
    imageSha256: referenceHash,
    build: {
      sourceRevision: "crystal-reference",
      executableSha256: "1".repeat(64),
      assetManifestSha256: "2".repeat(64),
    },
    ...overrides.reference,
  };
  const candidateState = {
    ...common,
    producer: "windows-native",
    imagePath: "candidate.png",
    imageSha256: candidateHash,
    build: {
      sourceRevision: "abcdef123456",
      executableSha256: "3".repeat(64),
      assetManifestSha256: "4".repeat(64),
    },
    ...overrides.candidate,
  };
  const referenceStatePath = path.join(directory, "reference.json");
  const candidateStatePath = path.join(directory, "candidate.json");
  await fs.writeFile(referenceStatePath, JSON.stringify(referenceState), "utf8");
  await fs.writeFile(candidateStatePath, JSON.stringify(candidateState), "utf8");
  return {
    referenceImagePath: referenceImage,
    candidateImagePath: candidateImage,
    referenceStatePath,
    candidateStatePath,
  };
}

async function withFixture(context, overrides = {}) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-visual-pair-test-"));
  context.after(() => fs.rm(directory, { recursive: true, force: true }));
  return createFixture(directory, overrides);
}

function reviewContract() {
  return {
    context: {
      path: path.resolve("pair-context.json"),
      bytes: 321,
      sha256: "a".repeat(64),
    },
    schemaSha256: "b".repeat(64),
  };
}

function acceptedReview(pair, contract, overrides = {}) {
  return {
    generatedAt: "2026-08-24T00:01:00.000Z",
    provider: "antigravity",
    model: "account-default",
    schemaSha256: contract.schemaSha256,
    evidence: [pair.reference.image, pair.candidate.image, contract.context],
    review: {
      verdict: "accepted",
      score: 94,
      summary: "同场景通过",
      sceneAlignment: { sameScene: true, confidence: 0.95, blockers: [] },
      scores: {
        mapAndObjects: 94,
        entitiesAndAnimation: 93,
        hudAndPanels: 94,
        typography: 92,
        colorAndLighting: 95,
        scaleAndDpi: 96,
      },
      issues: [],
      acceptedDifferences: [],
      nextActions: ["人工手感签署"],
      ...overrides,
    },
  };
}

test("pair CLI parser accepts review options and rejects unknown flags", () => {
  assert.deepEqual(parsePairArgs(["--reference-image=a.png", "--require-review"]), {
    "reference-image": "a.png",
    "require-review": true,
  });
  assert.throws(() => parsePairArgs(["--unknown", "x"]), /Unknown argument/);
});

test("model review can only hand off to human acceptance", () => {
  assert.deepEqual(buildAcceptanceGate(null, false), {
    pairValid: true,
    reviewRequired: false,
    review: null,
    modelPassed: false,
    humanAcceptanceRequired: true,
    humanAccepted: false,
    passed: false,
    status: "READY_FOR_MODEL_REVIEW",
  });
  const gate = buildAcceptanceGate({ passed: true, score: 94 }, true);
  assert.equal(gate.status, "READY_FOR_HUMAN_ACCEPTANCE");
  assert.equal(gate.modelPassed, true);
  assert.equal(gate.humanAccepted, false);
  assert.equal(gate.passed, false);
});

test("valid same-scene 1024x768 captures produce a bound pair manifest", async (context) => {
  const fixture = await withFixture(context);
  const pair = await verifyVisualPair(fixture);
  assert.equal(pair.schemaVersion, "mir2-native-visual-pair-v1");
  assert.equal(pair.runId, "vis-pair-test-001");
  assert.deepEqual(pair.alignment.world, { map: "0", x: 287, y: 618, light: "day" });
  assert.match(pair.reference.image.sha256, /^[0-9a-f]{64}$/);
  assert.notEqual(pair.reference.image.sha256, pair.candidate.image.sha256);
});

test("pair validation fails closed for mismatched authority and wrong dimensions", async (context) => {
  const mismatch = await withFixture(context, {
    candidate: { world: { map: "0", x: 288, y: 618, light: "day" } },
  });
  await assert.rejects(verifyVisualPair(mismatch), /world\.x/);

  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-visual-pair-size-test-"));
  context.after(() => fs.rm(directory, { recursive: true, force: true }));
  const wrong = await createFixture(directory);
  await fs.writeFile(wrong.candidateImagePath, pngImage(800, 600));
  await assert.rejects(verifyVisualPair(wrong), /must be 1024x768/);

  const truncated = await withFixture(context);
  await fs.writeFile(truncated.candidateImagePath, pngImage().subarray(0, 24));
  await assert.rejects(verifyVisualPair(truncated), /not a complete PNG/);
});

test("review gate binds image hashes and rejects P1, wrong scene, or low score", async (context) => {
  const fixture = await withFixture(context);
  const pair = await verifyVisualPair(fixture);
  const contract = reviewContract();
  assert.equal(validateReviewGate(acceptedReview(pair, contract), pair, 92, contract).passed, true);
  assert.equal(validateReviewGate(acceptedReview(pair, contract, { score: 91 }), pair, 92, contract).passed, false);
  assert.equal(validateReviewGate(acceptedReview(pair, contract, {
    issues: [{
      id: "VIS-001",
      priority: "P1",
      category: "hud",
      title: "HUD mismatch",
      evidence: "wrong frame",
      recommendation: "fix frame",
      confidence: 0.9,
      referenceRegion: "hud",
      candidateRegion: "hud",
    }],
  }), pair, 92, contract).passed, false);
  assert.equal(validateReviewGate(acceptedReview(pair, contract, {
    sceneAlignment: { sameScene: false, confidence: 0.2, blockers: ["coordinate mismatch"] },
  }), pair, 92, contract).passed, false);
  assert.equal(validateReviewGate(acceptedReview(pair, contract, {
    sceneAlignment: { sameScene: true, confidence: 0.2, blockers: [] },
  }), pair, 92, contract).passed, false);
  assert.equal(validateReviewGate(acceptedReview(pair, contract, {
    sceneAlignment: { sameScene: true, confidence: 0.95, blockers: ["actor mismatch"] },
  }), pair, 92, contract).passed, false);
  const wrongEvidence = acceptedReview(pair, contract);
  wrongEvidence.evidence[1] = { ...wrongEvidence.evidence[1], sha256: "f".repeat(64) };
  assert.throws(() => validateReviewGate(wrongEvidence, pair, 92, contract), /review candidate SHA-256/);
  const missingContext = acceptedReview(pair, contract);
  missingContext.evidence.pop();
  assert.throws(() => validateReviewGate(missingContext, pair, 92, contract), /exactly ordered/);
});

test("review command always receives pair context and explicit output", async (context) => {
  const fixture = await withFixture(context);
  const pair = await verifyVisualPair(fixture);
  const args = buildReviewArgs({
    pair,
    contextPath: "pair-context.json",
    outputDirectory: "review-output",
    provider: "antigravity",
    effort: "high",
  });
  assert.ok(args.includes("--context"));
  assert.ok(args.includes("pair-context.json"));
  assert.ok(args.includes("--output"));
  assert.ok(args.includes("review-output"));
  assert.ok(args.includes("antigravity"));
});
