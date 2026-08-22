import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";

import {
  buildAgyArgs,
  buildGeminiArgs,
  buildPrompt,
  buildVercelPrompt,
  buildVercelRequestBody,
  discoverVercelModel,
  findStructuredReview,
  geminiPackageRootForWindowsShim,
  isPathInside,
  parseArgs,
  summarizeGatewayUsage,
  validateReview,
} from "./review.mjs";

test("parseArgs accepts split and equals forms", () => {
  assert.deepEqual(parseArgs(["--reference", "a.png", "--candidate=b.png", "--dry-run"]), {
    reference: "a.png",
    candidate: "b.png",
    "dry-run": true,
  });
});

test("parseArgs rejects unknown flags and missing values", () => {
  assert.throws(() => parseArgs(["--reference"]), /requires a value/);
  assert.throws(() => parseArgs(["--not-a-review-option", "value"]), /Unknown argument/);
});

test("buildPrompt labels untrusted evidence and candidate role", () => {
  const prompt = buildPrompt({
    referencePath: path.resolve("reference.png"),
    candidatePath: path.resolve("candidate.png"),
    contextPath: path.resolve("capture.json"),
    candidateLabel: "Windows-native",
  });
  assert.match(prompt, /untrusted visual evidence/);
  assert.match(prompt, /Reference image:/);
  assert.match(prompt, /Windows-native candidate image:/);
  assert.match(prompt, /capture\.json/);
  assert.match(prompt, /do not edit files/);
  assert.match(prompt, /Do not call tools or terminal commands/);
});

test("buildAgyArgs enforces read-only structured print mode", () => {
  const args = buildAgyArgs({
    prompt: "review",
    schemaPath: "schema.json",
    model: "gemini-test",
    effort: "high",
    extraDirs: ["C:\\outside"],
  });
  assert.deepEqual(args.slice(0, 2), ["--print", "review"]);
  assert.ok(args.includes("--output-format"));
  assert.ok(args.includes("--json-schema"));
  assert.ok(args.includes("plan"));
  assert.ok(args.includes("gemini-test"));
  assert.ok(args.includes("C:\\outside"));
  assert.ok(!args.includes("--dangerously-skip-permissions"));
});

test("buildGeminiArgs uses read-only headless mode", () => {
  const args = buildGeminiArgs({
    prompt: "review",
    model: "gemini-test",
    extraDirs: ["C:\\outside"],
  });
  assert.deepEqual(args.slice(0, 2), ["--prompt", "review"]);
  assert.ok(args.includes("--output-format"));
  assert.ok(args.includes("--approval-mode"));
  assert.ok(args.includes("plan"));
  assert.ok(args.includes("--skip-trust"));
  assert.ok(args.includes("--include-directories"));
  assert.ok(!args.includes("--yolo"));
});

test("buildVercelRequestBody sends ordered high-detail images without credentials", () => {
  const prompt = buildVercelPrompt({
    candidateLabel: "Windows-native-login",
    contextText: null,
    schemaText: "{}",
  });
  const body = buildVercelRequestBody({
    model: "google/gemini-3.7-flash",
    prompt,
    referenceDataUrl: "data:image/png;base64,REF",
    candidateDataUrl: "data:image/png;base64,CANDIDATE",
    schema: { type: "object", additionalProperties: false, properties: {}, required: [] },
    effort: "medium",
    serviceTier: "flex",
    candidateLabel: "Windows-native-login",
  });
  assert.equal(body.model, "google/gemini-3.7-flash");
  assert.equal(body.service_tier, "flex");
  assert.equal(body.messages[0].content[2].image_url.url, "data:image/png;base64,REF");
  assert.equal(body.messages[0].content[4].image_url.url, "data:image/png;base64,CANDIDATE");
  assert.equal(body.providerOptions.google.thinkingLevel, "medium");
  assert.equal(body.response_format.type, "json_schema");
  assert.doesNotMatch(JSON.stringify(body), /api.?key|authorization|bearer/i);
});

test("discoverVercelModel validates the live-catalog response shape", async () => {
  const fakeFetch = async () => new Response(JSON.stringify({
    data: [{
      id: "google/gemini-3.7-flash",
      name: "Gemini 3.7 Flash",
      type: "language",
      context_window: 1_000_000,
      max_tokens: 65_536,
      tags: ["vision"],
      modalities: { input: ["text", "image"], output: ["text"] },
      pricing: { input: "0.00000075", output: "0.00000375" },
    }],
  }), { status: 200 });
  const model = await discoverVercelModel("google/gemini-3.7-flash", fakeFetch);
  assert.equal(model.id, "google/gemini-3.7-flash");
  assert.equal(model.contextWindow, 1_000_000);
});

test("findStructuredReview unwraps nested and stringified CLI output", () => {
  const review = {
    verdict: "candidate",
    score: 71,
    scores: { hudAndPanels: 60 },
    issues: [],
  };
  assert.deepEqual(findStructuredReview({ result: JSON.stringify(review) }), review);
  assert.deepEqual(findStructuredReview({ nested: [{ payload: JSON.stringify(review) }] }), review);
});

test("isPathInside rejects siblings", () => {
  const parent = path.resolve("root", "repo");
  assert.equal(isPathInside(parent, path.join(parent, "images", "a.png")), true);
  assert.equal(isPathInside(parent, path.resolve("root", "other", "a.png")), false);
});

test("Windows Gemini npm shim resolves to the installed package root", () => {
  assert.equal(
    geminiPackageRootForWindowsShim("C:\\Users\\tester\\AppData\\Roaming\\npm\\gemini.cmd"),
    path.resolve(
      "C:\\Users\\tester\\AppData\\Roaming\\npm",
      "node_modules",
      "@google",
      "gemini-cli",
    ),
  );
});

test("validateReview accepts a complete schema-shaped review", () => {
  const review = {
    verdict: "candidate",
    score: 71,
    summary: "可继续修正",
    sceneAlignment: { sameScene: true, confidence: 0.9, blockers: [] },
    scores: {
      mapAndObjects: 80,
      entitiesAndAnimation: 70,
      hudAndPanels: 60,
      typography: 65,
      colorAndLighting: 75,
      scaleAndDpi: 70,
    },
    issues: [],
    acceptedDifferences: [],
    nextActions: ["修正登录面板"],
  };
  assert.equal(validateReview(review), review);
});

test("summarizeGatewayUsage applies the selected service-tier rates", () => {
  const summary = summarizeGatewayUsage(
    { usage: { prompt_tokens: 1000, completion_tokens: 200, total_tokens: 1200 }, service_tier: "flex" },
    {
      pricing: {
        input: "0.00000075",
        output: "0.00000375",
        service_tiers: { flex: { input: "0.000000375", output: "0.000001875" } },
      },
    },
    "flex",
  );
  assert.equal(summary.estimatedUpperBoundUsd, 0.00075);
  assert.equal(summary.responseServiceTier, "flex");
});
