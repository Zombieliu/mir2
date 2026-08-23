#!/usr/bin/env node

import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { inflateSync } from "node:zlib";

import {
  loadReviewSchema,
  validateReview,
} from "../../../tools/antigravity-visual-review/review.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const REVIEW_SCRIPT = path.join(REPO_ROOT, "tools", "antigravity-visual-review", "review.mjs");
const DEFAULT_OUTPUT_ROOT = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "player-qa",
  "native-visual-pairs",
);
const CAPTURE_SCHEMA_VERSION = "mir2-native-visual-capture-v1";
const PAIR_SCHEMA_VERSION = "mir2-native-visual-pair-v1";
const REQUIRED_WIDTH = 1024;
const REQUIRED_HEIGHT = 768;
const MAX_CAPTURE_DELTA_MS = 5 * 60 * 1000;
const MIN_SCENE_ALIGNMENT_CONFIDENCE = 0.9;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const RUN_ID_PATTERN = /^[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$/;
const SCENES = new Set([
  "login",
  "character-select",
  "in-game",
  "quest-accepted",
  "combat",
  "quest-complete",
]);
const WORLD_SCENES = new Set(["in-game", "quest-accepted", "combat", "quest-complete"]);

export function parsePairArgs(argv) {
  const result = {};
  const valueFlags = new Set([
    "reference-image",
    "candidate-image",
    "reference-state",
    "candidate-state",
    "review-report",
    "provider",
    "model",
    "effort",
    "service-tier",
    "timeout-ms",
    "retries",
    "minimum-score",
    "output",
  ]);
  const booleanFlags = new Set(["help", "h", "require-review"]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "-h") {
      result.h = true;
      continue;
    }
    if (!token.startsWith("--")) throw new Error(`Unexpected positional argument: ${token}`);
    const equalsIndex = token.indexOf("=");
    const key = token.slice(2, equalsIndex > 2 ? equalsIndex : undefined);
    if (!valueFlags.has(key) && !booleanFlags.has(key)) throw new Error(`Unknown argument: --${key}`);
    if (equalsIndex > 2) {
      const value = token.slice(equalsIndex + 1);
      if (!value) throw new Error(`--${key} requires a value.`);
      result[key] = value;
      continue;
    }
    if (booleanFlags.has(key)) {
      result[key] = true;
      continue;
    }
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`--${key} requires a value.`);
    result[key] = value;
    index += 1;
  }
  return result;
}

export async function verifyVisualPair({
  referenceImagePath,
  candidateImagePath,
  referenceStatePath,
  candidateStatePath,
}) {
  const [referenceImage, candidateImage, referenceState, candidateState] = await Promise.all([
    describePng(referenceImagePath, "--reference-image"),
    describePng(candidateImagePath, "--candidate-image"),
    readJsonFile(referenceStatePath, "--reference-state"),
    readJsonFile(candidateStatePath, "--candidate-state"),
  ]);

  const reference = validateCaptureState(
    referenceState.value,
    referenceState.path,
    referenceImage,
    "crystal-original",
  );
  const candidate = validateCaptureState(
    candidateState.value,
    candidateState.path,
    candidateImage,
    "windows-native",
  );
  assertEqual(reference.runId, candidate.runId, "runId");
  assertEqual(reference.scene, candidate.scene, "scene");
  assertEqual(reference.uiState, candidate.uiState, "uiState");
  assertEqual(reference.logicalSize.width, candidate.logicalSize.width, "logicalSize.width");
  assertEqual(reference.logicalSize.height, candidate.logicalSize.height, "logicalSize.height");
  if (Math.abs(reference.dpiScale - candidate.dpiScale) > 0.001) {
    throw new Error(`Capture mismatch for dpiScale: ${reference.dpiScale} != ${candidate.dpiScale}`);
  }
  const captureDeltaMs = Math.abs(Date.parse(reference.capturedAt) - Date.parse(candidate.capturedAt));
  if (captureDeltaMs > MAX_CAPTURE_DELTA_MS) {
    throw new Error(`Capture timestamps differ by ${captureDeltaMs} ms; maximum is ${MAX_CAPTURE_DELTA_MS} ms.`);
  }
  if (WORLD_SCENES.has(reference.scene)) {
    for (const field of ["map", "x", "y", "light"]) {
      assertEqual(reference.world[field], candidate.world[field], `world.${field}`);
    }
  }

  return {
    schemaVersion: PAIR_SCHEMA_VERSION,
    generatedAt: new Date().toISOString(),
    runId: reference.runId,
    scene: reference.scene,
    alignment: {
      logicalWidth: reference.logicalSize.width,
      logicalHeight: reference.logicalSize.height,
      dpiScale: reference.dpiScale,
      uiState: reference.uiState,
      world: reference.world,
      captureDeltaMs,
    },
    reference: pairCaptureDescriptor(reference, referenceImage, referenceState.path),
    candidate: pairCaptureDescriptor(candidate, candidateImage, candidateState.path),
  };
}

export function validateReviewGate(report, pair, minimumScore, contract) {
  if (!report || typeof report !== "object" || Array.isArray(report)) {
    throw new Error("Visual review report must be an object.");
  }
  if (!contract || typeof contract !== "object") {
    throw new Error("Visual review contract binding is required.");
  }
  if (!Number.isFinite(Date.parse(report.generatedAt))) {
    throw new Error("Visual review report has an invalid generatedAt timestamp.");
  }
  if (!["vercel", "gemini", "antigravity"].includes(report.provider)) {
    throw new Error(`Visual review report has unsupported provider: ${report.provider}`);
  }
  if (typeof report.model !== "string" || report.model.trim().length === 0) {
    throw new Error("Visual review report must identify the model.");
  }
  if ((report.provider === "vercel" || report.provider === "gemini") && !/gemini/i.test(report.model)) {
    throw new Error(`Visual review provider ${report.provider} must identify a Gemini model.`);
  }
  assertEqual(
    String(report.schemaSha256 ?? "").toLowerCase(),
    contract.schemaSha256,
    "review schema SHA-256",
  );
  const review = validateReview(report.review ?? report);
  const evidence = Array.isArray(report.evidence) ? report.evidence : [];
  if (evidence.length !== 3) {
    throw new Error("Visual review report must include exactly ordered reference, candidate, and pair-context evidence.");
  }
  assertEvidenceDescriptor(evidence[0], pair.reference.image, "review reference");
  assertEvidenceDescriptor(evidence[1], pair.candidate.image, "review candidate");
  assertEvidenceDescriptor(evidence[2], contract.context, "review pair context");
  const blockingIssues = review.issues.filter((issue) => issue.priority === "P0" || issue.priority === "P1");
  const failures = [];
  if (review.verdict !== "accepted") failures.push(`verdict=${review.verdict}`);
  if (!review.sceneAlignment.sameScene) failures.push("sameScene=false");
  if (review.sceneAlignment.confidence < MIN_SCENE_ALIGNMENT_CONFIDENCE) {
    failures.push(`sceneAlignmentConfidence=${review.sceneAlignment.confidence}<${MIN_SCENE_ALIGNMENT_CONFIDENCE}`);
  }
  if (review.sceneAlignment.blockers.length > 0) {
    failures.push(`sceneAlignmentBlockers=${review.sceneAlignment.blockers.length}`);
  }
  if (blockingIssues.length > 0) failures.push(`blockingIssues=${blockingIssues.length}`);
  if (review.score < minimumScore) failures.push(`score=${review.score}<${minimumScore}`);
  return {
    passed: failures.length === 0,
    minimumScore,
    verdict: review.verdict,
    score: review.score,
    sameScene: review.sceneAlignment.sameScene,
    sceneAlignmentConfidence: review.sceneAlignment.confidence,
    sceneAlignmentBlockers: review.sceneAlignment.blockers,
    blockingIssueIds: blockingIssues.map((issue) => issue.id),
    provider: report.provider,
    model: report.model,
    failures,
  };
}

function assertEvidenceDescriptor(actual, expected, label) {
  if (!actual || typeof actual !== "object" || Array.isArray(actual)) {
    throw new Error(`${label} evidence descriptor is missing.`);
  }
  if (!samePath(String(actual.path ?? ""), expected.path)) {
    throw new Error(`${label} evidence path does not match the verified pair.`);
  }
  assertEqual(String(actual.sha256 ?? "").toLowerCase(), expected.sha256, `${label} SHA-256`);
  assertEqual(actual.bytes, expected.bytes, `${label} byte length`);
}

export function buildReviewArgs({
  pair,
  contextPath,
  outputDirectory,
  provider,
  model,
  effort,
  serviceTier,
  timeoutMs,
  retries,
}) {
  const args = [
    REVIEW_SCRIPT,
    "--reference",
    pair.reference.image.path,
    "--candidate",
    pair.candidate.image.path,
    "--context",
    contextPath,
    "--label",
    `Windows-native-${pair.scene}`,
    "--provider",
    provider,
    "--output",
    outputDirectory,
  ];
  if (model) args.push("--model", model);
  if (effort) args.push("--effort", effort);
  if (serviceTier) args.push("--service-tier", serviceTier);
  if (timeoutMs) args.push("--timeout-ms", String(timeoutMs));
  if (retries !== undefined) args.push("--retries", String(retries));
  return args;
}

export function buildAcceptanceGate(reviewGate, requireReview) {
  const modelPassed = Boolean(reviewGate?.passed);
  return {
    pairValid: true,
    reviewRequired: requireReview,
    review: reviewGate,
    modelPassed,
    humanAcceptanceRequired: true,
    humanAccepted: false,
    passed: false,
    status: requireReview
      ? modelPassed ? "READY_FOR_HUMAN_ACCEPTANCE" : "MODEL_BLOCKED"
      : "READY_FOR_MODEL_REVIEW",
  };
}

function validateCaptureState(value, statePath, image, expectedProducer) {
  assertClosedObject(value, statePath, [
    "schemaVersion",
    "producer",
    "runId",
    "scene",
    "capturedAt",
    "imagePath",
    "imageSha256",
    "logicalSize",
    "dpiScale",
    "uiState",
    "world",
    "build",
  ]);
  assertEqual(value.schemaVersion, CAPTURE_SCHEMA_VERSION, `${statePath} schemaVersion`);
  assertEqual(value.producer, expectedProducer, `${statePath} producer`);
  if (typeof value.runId !== "string" || !RUN_ID_PATTERN.test(value.runId)) {
    throw new Error(`${statePath} has invalid runId.`);
  }
  if (!SCENES.has(value.scene)) throw new Error(`${statePath} has unsupported scene: ${value.scene}`);
  if (!Number.isFinite(Date.parse(value.capturedAt))) throw new Error(`${statePath} has invalid capturedAt.`);
  if (typeof value.imagePath !== "string" || value.imagePath.length === 0) {
    throw new Error(`${statePath} imagePath must be a non-empty string.`);
  }
  const declaredImagePath = path.resolve(path.dirname(statePath), value.imagePath);
  if (!samePath(declaredImagePath, image.path)) {
    throw new Error(`${statePath} imagePath does not identify ${image.path}.`);
  }
  assertSha256(value.imageSha256, `${statePath} imageSha256`);
  assertEqual(value.imageSha256.toLowerCase(), image.sha256, `${statePath} imageSha256`);
  assertClosedObject(value.logicalSize, `${statePath} logicalSize`, ["width", "height"]);
  assertEqual(value.logicalSize.width, REQUIRED_WIDTH, `${statePath} logicalSize.width`);
  assertEqual(value.logicalSize.height, REQUIRED_HEIGHT, `${statePath} logicalSize.height`);
  if (typeof value.dpiScale !== "number" || !Number.isFinite(value.dpiScale) || value.dpiScale < 0.5 || value.dpiScale > 4) {
    throw new Error(`${statePath} dpiScale must be between 0.5 and 4.`);
  }
  if (typeof value.uiState !== "string" || value.uiState.length === 0) {
    throw new Error(`${statePath} uiState must be a non-empty string.`);
  }
  const world = validateWorld(value.world, value.scene, statePath);
  assertClosedObject(value.build, `${statePath} build`, [
    "sourceRevision",
    "executableSha256",
    "assetManifestSha256",
  ]);
  if (typeof value.build.sourceRevision !== "string" || value.build.sourceRevision.length === 0 || value.build.sourceRevision.length > 160) {
    throw new Error(`${statePath} build.sourceRevision must be 1..160 characters.`);
  }
  assertSha256(value.build.executableSha256, `${statePath} build.executableSha256`);
  assertSha256(value.build.assetManifestSha256, `${statePath} build.assetManifestSha256`);
  return { ...value, world };
}

function validateWorld(world, scene, statePath) {
  if (!WORLD_SCENES.has(scene)) {
    if (world !== null) throw new Error(`${statePath} world must be null for scene ${scene}.`);
    return null;
  }
  assertClosedObject(world, `${statePath} world`, ["map", "x", "y", "light"]);
  if (typeof world.map !== "string" || world.map.length === 0) throw new Error(`${statePath} world.map is required.`);
  if (!Number.isSafeInteger(world.x) || !Number.isSafeInteger(world.y)) throw new Error(`${statePath} world coordinates must be integers.`);
  if (typeof world.light !== "string" || world.light.length === 0) throw new Error(`${statePath} world.light is required.`);
  return world;
}

function pairCaptureDescriptor(state, image, statePath) {
  return {
    producer: state.producer,
    statePath,
    capturedAt: state.capturedAt,
    image,
    build: state.build,
  };
}

async function describePng(value, flag) {
  const imagePath = await requireFile(value, flag);
  if (path.extname(imagePath).toLowerCase() !== ".png") throw new Error(`${flag} must be a PNG file.`);
  const bytes = await fs.readFile(imagePath);
  const { width, height } = validatePngBytes(bytes, flag, imagePath);
  if (width !== REQUIRED_WIDTH || height !== REQUIRED_HEIGHT) {
    throw new Error(`${flag} must be ${REQUIRED_WIDTH}x${REQUIRED_HEIGHT}; received ${width}x${height}.`);
  }
  return {
    path: imagePath,
    bytes: bytes.length,
    width,
    height,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
}

function validatePngBytes(bytes, flag, imagePath) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (bytes.length < 45 || !bytes.subarray(0, 8).equals(signature)) {
    throw new Error(`${flag} is not a complete PNG file: ${imagePath}`);
  }
  let offset = 8;
  let width = null;
  let height = null;
  let colorType = null;
  let sawIhdr = false;
  let sawIend = false;
  const idat = [];
  while (offset < bytes.length) {
    if (offset + 12 > bytes.length) throw new Error(`${flag} has a truncated PNG chunk: ${imagePath}`);
    const length = bytes.readUInt32BE(offset);
    const typeStart = offset + 4;
    const dataStart = typeStart + 4;
    const dataEnd = dataStart + length;
    const crcEnd = dataEnd + 4;
    if (crcEnd > bytes.length) throw new Error(`${flag} has a truncated PNG chunk payload: ${imagePath}`);
    const type = bytes.toString("ascii", typeStart, dataStart);
    const expectedCrc = bytes.readUInt32BE(dataEnd);
    const actualCrc = crc32(bytes.subarray(typeStart, dataEnd));
    if (actualCrc !== expectedCrc) throw new Error(`${flag} has an invalid ${type} CRC: ${imagePath}`);
    if (!sawIhdr && type !== "IHDR") throw new Error(`${flag} PNG must begin with IHDR: ${imagePath}`);
    if (type === "IHDR") {
      if (sawIhdr || length !== 13) throw new Error(`${flag} has an invalid IHDR chunk: ${imagePath}`);
      sawIhdr = true;
      width = bytes.readUInt32BE(dataStart);
      height = bytes.readUInt32BE(dataStart + 4);
      const bitDepth = bytes[dataStart + 8];
      colorType = bytes[dataStart + 9];
      const compression = bytes[dataStart + 10];
      const filter = bytes[dataStart + 11];
      const interlace = bytes[dataStart + 12];
      if (bitDepth !== 8 || ![2, 6].includes(colorType) || compression !== 0 || filter !== 0 || interlace !== 0) {
        throw new Error(`${flag} must be a non-interlaced 8-bit RGB/RGBA PNG: ${imagePath}`);
      }
    } else if (type === "IDAT") {
      if (!sawIhdr || sawIend) throw new Error(`${flag} has IDAT outside the PNG image stream: ${imagePath}`);
      idat.push(bytes.subarray(dataStart, dataEnd));
    } else if (type === "IEND") {
      if (length !== 0 || sawIend) throw new Error(`${flag} has an invalid IEND chunk: ${imagePath}`);
      sawIend = true;
      if (crcEnd !== bytes.length) throw new Error(`${flag} contains trailing bytes after IEND: ${imagePath}`);
    }
    offset = crcEnd;
  }
  if (!sawIhdr || !sawIend || idat.length === 0) {
    throw new Error(`${flag} is missing required PNG chunks: ${imagePath}`);
  }
  let decoded;
  try {
    decoded = inflateSync(Buffer.concat(idat));
  } catch (error) {
    throw new Error(`${flag} has an invalid PNG image stream: ${imagePath}: ${error.message}`);
  }
  const channels = colorType === 6 ? 4 : 3;
  const rowBytes = width * channels;
  const expectedDecodedBytes = (rowBytes + 1) * height;
  if (decoded.length !== expectedDecodedBytes) {
    throw new Error(`${flag} PNG decoded byte length is invalid: ${decoded.length} != ${expectedDecodedBytes}.`);
  }
  for (let row = 0; row < height; row += 1) {
    if (decoded[row * (rowBytes + 1)] > 4) {
      throw new Error(`${flag} PNG has an invalid row filter at row ${row}: ${imagePath}`);
    }
  }
  return { width, height };
}

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

async function readJsonFile(value, flag) {
  const filePath = await requireFile(value, flag);
  const text = await fs.readFile(filePath, "utf8");
  try {
    return { path: filePath, value: JSON.parse(text) };
  } catch (error) {
    throw new Error(`${flag} is not valid JSON: ${filePath}: ${error.message}`);
  }
}

async function requireFile(value, flag) {
  if (!value || value === true) throw new Error(`${flag} is required.`);
  const filePath = path.resolve(String(value));
  const stat = await fs.stat(filePath).catch(() => null);
  if (!stat?.isFile()) throw new Error(`${flag} file does not exist: ${filePath}`);
  return filePath;
}

function assertClosedObject(value, label, fields) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} must be an object.`);
  const allowed = new Set(fields);
  const unknown = Object.keys(value).filter((field) => !allowed.has(field));
  if (unknown.length > 0) throw new Error(`${label} contains unknown field(s): ${unknown.join(", ")}`);
  const missing = fields.filter((field) => !Object.hasOwn(value, field));
  if (missing.length > 0) throw new Error(`${label} is missing field(s): ${missing.join(", ")}`);
}

function assertSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value.toLowerCase())) {
    throw new Error(`${label} must be a lowercase or uppercase SHA-256 hex digest.`);
  }
}

function assertEqual(left, right, label) {
  if (left !== right) throw new Error(`Capture mismatch for ${label}: ${JSON.stringify(left)} != ${JSON.stringify(right)}`);
}

function samePath(left, right) {
  const normalize = (value) => process.platform === "win32" ? path.resolve(value).toLowerCase() : path.resolve(value);
  return normalize(left) === normalize(right);
}

function defaultMinimumScore(scene) {
  return scene === "login" || scene === "character-select" ? 90 : 92;
}

function positiveInteger(value, flag) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${flag} must be a positive integer.`);
  return parsed;
}

function nonNegativeInteger(value, flag) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < 0) throw new Error(`${flag} must be a non-negative integer.`);
  return parsed;
}

async function runProcess(executable, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(executable, args, { cwd, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => { stdout += chunk.toString(); });
    child.stderr.on("data", (chunk) => { stderr += chunk.toString(); });
    child.on("error", reject);
    child.on("close", (exitCode) => resolve({ exitCode: exitCode ?? 1, stdout, stderr }));
  });
}

async function describeFile(filePath) {
  const resolved = await requireFile(filePath, "evidence");
  const bytes = await fs.readFile(resolved);
  return {
    path: resolved,
    bytes: bytes.length,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
}

async function main() {
  const args = parsePairArgs(process.argv.slice(2));
  if (args.help || args.h) {
    printHelp();
    return;
  }
  if (args.provider && args["review-report"]) throw new Error("Use either --provider or --review-report, not both.");
  const pair = await verifyVisualPair({
    referenceImagePath: args["reference-image"],
    candidateImagePath: args["candidate-image"],
    referenceStatePath: args["reference-state"],
    candidateStatePath: args["candidate-state"],
  });
  const outputDirectory = path.resolve(args.output ?? path.join(DEFAULT_OUTPUT_ROOT, pair.runId));
  await fs.mkdir(outputDirectory, { recursive: true });
  const contextPath = path.join(outputDirectory, "pair-context.json");
  const manifestPath = path.join(outputDirectory, "pair-manifest.json");
  await fs.writeFile(contextPath, `${JSON.stringify(pair, null, 2)}\n`, "utf8");
  const [contextEvidence, reviewSchema] = await Promise.all([
    describeFile(contextPath),
    loadReviewSchema(),
  ]);

  let reviewReport = null;
  let reviewPath = null;
  if (args.provider) {
    const reviewDirectory = path.join(outputDirectory, "review");
    const reviewArgs = buildReviewArgs({
      pair,
      contextPath,
      outputDirectory: reviewDirectory,
      provider: args.provider,
      model: args.model,
      effort: args.effort,
      serviceTier: args["service-tier"],
      timeoutMs: args["timeout-ms"] ? positiveInteger(args["timeout-ms"], "--timeout-ms") : undefined,
      retries: args.retries !== undefined ? nonNegativeInteger(args.retries, "--retries") : undefined,
    });
    const result = await runProcess(process.execPath, reviewArgs, REPO_ROOT);
    await fs.writeFile(path.join(outputDirectory, "review-run.stdout.txt"), result.stdout, "utf8");
    await fs.writeFile(path.join(outputDirectory, "review-run.stderr.txt"), result.stderr, "utf8");
    if (result.exitCode !== 0) throw new Error(`Visual review provider failed with exit code ${result.exitCode}.`);
    reviewPath = path.join(reviewDirectory, "review.json");
    reviewReport = (await readJsonFile(reviewPath, "generated review report")).value;
  } else if (args["review-report"]) {
    const loaded = await readJsonFile(args["review-report"], "--review-report");
    reviewPath = loaded.path;
    reviewReport = loaded.value;
  }

  const minimumScore = args["minimum-score"]
    ? positiveInteger(args["minimum-score"], "--minimum-score")
    : defaultMinimumScore(pair.scene);
  if (minimumScore > 100) throw new Error("--minimum-score must be at most 100.");
  const reviewGate = reviewReport
    ? validateReviewGate(reviewReport, pair, minimumScore, {
        context: contextEvidence,
        schemaSha256: reviewSchema.sha256,
      })
    : null;
  const requireReview = Boolean(args["require-review"] || args.provider || args["review-report"]);
  const manifest = {
    ...pair,
    gate: buildAcceptanceGate(reviewGate, requireReview),
    ...(reviewPath ? {
      reviewReport: {
        path: reviewPath,
        sha256: crypto.createHash("sha256").update(await fs.readFile(reviewPath)).digest("hex"),
      },
    } : {}),
  };
  await fs.writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, "utf8");
  if (requireReview && !reviewGate?.passed) {
    throw new Error(`Visual review gate failed: ${reviewGate?.failures.join("; ") || "review missing"}. Manifest: ${manifestPath}`);
  }
  console.log(JSON.stringify({
    ok: true,
    status: manifest.gate.status,
    manifestPath,
    contextPath,
    runId: pair.runId,
    scene: pair.scene,
    desktopTouched: false,
  }, null, 2));
}

function printHelp() {
  console.log(`Usage:
  node apps/web/scripts/verify-native-visual-pair.mjs \\
    --reference-image <original.png> --reference-state <original.json> \\
    --candidate-image <native.png> --candidate-state <native.json> [options]

Options:
  --output <directory>       Pair context, manifest, and optional review output.
  --review-report <path>     Validate an existing structured visual-review report.
  --provider <name>          Run review.mjs with vercel, gemini, or antigravity.
  --model <id>               Optional provider model override.
  --effort <level>           Optional low, medium, or high review effort.
  --service-tier <tier>      Optional Vercel service tier.
  --timeout-ms <number>      Optional review timeout.
  --retries <number>         Optional Vercel retry count.
  --minimum-score <0..100>   Default: 90 login/select, 92 in-game.
  --require-review           Fail unless a passing review is supplied or generated.
`);
}

const isDirectRun = process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url;
if (isDirectRun) {
  main().catch((error) => {
    console.error(JSON.stringify({ ok: false, status: "BLOCKED", error: String(error?.message ?? error), desktopTouched: false }, null, 2));
    process.exitCode = 1;
  });
}
