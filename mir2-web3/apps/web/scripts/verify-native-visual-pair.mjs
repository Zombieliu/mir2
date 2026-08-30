#!/usr/bin/env node

import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { TextDecoder } from "node:util";
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
const CAPTURE_ATTESTATION_SCHEMA_VERSION = "mir2-native-capture-attestation-v1";
const PAIR_SCHEMA_VERSION = "mir2-native-visual-pair-v1";
const REQUIRED_WIDTH = 1024;
const REQUIRED_HEIGHT = 768;
const MAX_CAPTURE_DELTA_MS = 5 * 60 * 1000;
const MIN_SCENE_ALIGNMENT_CONFIDENCE = 0.9;
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const SHA1_THUMBPRINT_PATTERN = /^[0-9A-F]{40}$/;
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
    "candidate-attestation",
    "candidate-attestation-signature",
    "candidate-attestation-spki",
    "candidate-package-verification",
    "candidate-release-statement",
    "candidate-release-signature",
    "trusted-capture-spki-sha256",
    "trusted-release-signer-thumbprint",
    "expected-candidate",
    "expected-source-revision",
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
  candidateAttestationPath,
  candidateAttestationSignaturePath,
  candidateAttestationSpkiPath,
  candidatePackageVerificationPath,
  candidateReleaseStatementPath,
  candidateReleaseSignaturePath,
  trustedCaptureSpkiSha256,
  trustedReleaseSignerThumbprint,
  expectedCandidate,
  expectedSourceRevision,
}) {
  const [
    referenceImage,
    candidateImage,
    referenceState,
    candidateState,
    candidateAttestation,
    candidateAttestationSignature,
    candidateAttestationSpki,
    candidatePackageVerification,
    candidateReleaseStatement,
    candidateReleaseSignature,
  ] = await Promise.all([
    describePng(referenceImagePath, "--reference-image"),
    describePng(candidateImagePath, "--candidate-image"),
    readJsonFile(referenceStatePath, "--reference-state"),
    readJsonFile(candidateStatePath, "--candidate-state"),
    readJsonFile(candidateAttestationPath, "--candidate-attestation", { canonical: true }),
    readBinaryFile(candidateAttestationSignaturePath, "--candidate-attestation-signature"),
    readBinaryFile(candidateAttestationSpkiPath, "--candidate-attestation-spki"),
    readJsonFile(candidatePackageVerificationPath, "--candidate-package-verification"),
    readJsonFile(candidateReleaseStatementPath, "--candidate-release-statement", { canonical: true }),
    readBinaryFile(candidateReleaseSignaturePath, "--candidate-release-signature"),
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
  const attestation = validateCandidateAttestation({
    statement: candidateAttestation,
    signature: candidateAttestationSignature,
    spki: candidateAttestationSpki,
    packageVerification: candidatePackageVerification,
    releaseStatement: candidateReleaseStatement,
    releaseSignature: candidateReleaseSignature,
    candidate,
    candidateImage,
    candidateState,
    trustedCaptureSpkiSha256,
    trustedReleaseSignerThumbprint,
    expectedCandidate,
    expectedSourceRevision,
  });
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
    candidate: {
      ...pairCaptureDescriptor(candidate, candidateImage, candidateState.path),
      attestation,
    },
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
  if (report.provider === "vercel" && !/gemini/i.test(report.model)) {
    throw new Error("Vercel visual review must identify a Gemini model.");
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

export function validateCandidateAttestation({
  statement,
  signature,
  spki,
  packageVerification,
  releaseStatement,
  releaseSignature,
  candidate,
  candidateImage,
  candidateState,
  trustedCaptureSpkiSha256,
  trustedReleaseSignerThumbprint,
  expectedCandidate,
  expectedSourceRevision,
}) {
  const trustedSpki = normalizeSha256(
    trustedCaptureSpkiSha256,
    "--trusted-capture-spki-sha256",
  );
  const trustedReleaseSigner = normalizeThumbprint(
    trustedReleaseSignerThumbprint,
    "--trusted-release-signer-thumbprint",
  );
  const expectedCandidateIdentity = normalizeCandidate(
    expectedCandidate,
    "--expected-candidate",
  );
  const expectedRevision = normalizeRevision(
    expectedSourceRevision,
    "--expected-source-revision",
  );
  const value = statement.value;
  const fields = [
    "schemaVersion",
    "attestedAt",
    "runId",
    "scene",
    "capturedAt",
    "stateSha256",
    "imageSha256",
    "processId",
    "processStartUtc",
    "exeSha256",
    "candidate",
    "sourceRevision",
    "packageManifestSha256",
    "releaseStatementSha256",
    "releaseSignatureSha256",
    "packageVerificationSha256",
    "trustedReleaseSignerThumbprint",
    "signatureAlgorithm",
    "evidenceSignerSpkiSha256",
  ];
  assertClosedObject(value, statement.path, fields);
  assertEqual(
    value.schemaVersion,
    CAPTURE_ATTESTATION_SCHEMA_VERSION,
    `${statement.path} schemaVersion`,
  );
  assertEqual(value.signatureAlgorithm, "RSA-PKCS1-SHA256", `${statement.path} signatureAlgorithm`);
  if (!RUN_ID_PATTERN.test(String(value.runId ?? ""))) {
    throw new Error(`${statement.path} has an invalid runId.`);
  }
  if (!SCENES.has(value.scene)) {
    throw new Error(`${statement.path} has an unsupported scene.`);
  }
  if (!Number.isSafeInteger(value.processId) || value.processId <= 0) {
    throw new Error(`${statement.path} processId must be a positive integer.`);
  }
  if (!/^[0-9a-f]{40}$/.test(String(value.sourceRevision ?? ""))) {
    throw new Error(`${statement.path} sourceRevision must be a lowercase Git revision.`);
  }
  if (!/^WN-CANDIDATE-[A-Za-z0-9._-]+$/.test(String(value.candidate ?? ""))) {
    throw new Error(`${statement.path} candidate identity is invalid.`);
  }
  assertEqual(value.candidate, expectedCandidateIdentity, "expected Candidate identity");
  assertEqual(value.sourceRevision, expectedRevision, "expected source revision");
  for (const field of [
    "stateSha256",
    "imageSha256",
    "exeSha256",
    "packageManifestSha256",
    "releaseStatementSha256",
    "releaseSignatureSha256",
    "packageVerificationSha256",
    "evidenceSignerSpkiSha256",
  ]) {
    assertSha256(value[field], `${statement.path} ${field}`);
    if (value[field] !== value[field].toLowerCase()) {
      throw new Error(`${statement.path} ${field} must be lowercase.`);
    }
  }

  const capturedAt = strictTimestamp(value.capturedAt, `${statement.path} capturedAt`);
  const processStart = strictTimestamp(value.processStartUtc, `${statement.path} processStartUtc`);
  const attestedAt = strictTimestamp(value.attestedAt, `${statement.path} attestedAt`);
  if (processStart > capturedAt || capturedAt > attestedAt) {
    throw new Error(`${statement.path} process/capture/attestation timestamps are out of order.`);
  }
  if (attestedAt - capturedAt > 15 * 60 * 1000) {
    throw new Error(`${statement.path} was signed more than 15 minutes after capture.`);
  }

  assertEqual(value.runId, candidate.runId, "candidate attestation runId");
  assertEqual(value.scene, candidate.scene, "candidate attestation scene");
  assertEqual(value.capturedAt, candidate.capturedAt, "candidate attestation capturedAt");
  assertEqual(value.stateSha256, candidateState.sha256, "candidate state SHA-256");
  assertEqual(value.imageSha256, candidateImage.sha256, "candidate image SHA-256");
  assertEqual(
    value.exeSha256,
    candidate.build.executableSha256.toLowerCase(),
    "candidate EXE SHA-256",
  );
  assertEqual(value.sourceRevision, candidate.build.sourceRevision, "candidate source revision");
  assertEqual(
    value.packageManifestSha256,
    candidate.build.assetManifestSha256.toLowerCase(),
    "candidate package-manifest SHA-256",
  );
  assertEqual(
    value.releaseStatementSha256,
    releaseStatement.sha256,
    "release statement SHA-256",
  );
  assertEqual(
    value.releaseSignatureSha256,
    releaseSignature.sha256,
    "release signature SHA-256",
  );
  assertEqual(
    value.packageVerificationSha256,
    packageVerification.sha256,
    "package verification SHA-256",
  );
  assertEqual(
    normalizeThumbprint(value.trustedReleaseSignerThumbprint, "attested release signer"),
    trustedReleaseSigner,
    "trusted release signer",
  );

  validateReleaseStatement(releaseStatement.value, value, releaseStatement.path);
  validatePackageVerification(
    packageVerification.value,
    value,
    releaseStatement.value,
    trustedReleaseSigner,
    packageVerification.path,
  );

  const actualSpkiSha256 = crypto.createHash("sha256").update(spki.bytes).digest("hex");
  assertEqual(actualSpkiSha256, trustedSpki, "trusted capture signer SPKI SHA-256");
  assertEqual(value.evidenceSignerSpkiSha256, trustedSpki, "attested capture signer SPKI SHA-256");
  let publicKey;
  try {
    publicKey = crypto.createPublicKey({ key: spki.bytes, format: "der", type: "spki" });
  } catch (error) {
    throw new Error(`${spki.path} is not a valid DER SPKI public key: ${error.message}`);
  }
  if (publicKey.asymmetricKeyType !== "rsa") {
    throw new Error(`${spki.path} evidence signer must use RSA.`);
  }
  const modulusLength = publicKey.asymmetricKeyDetails?.modulusLength;
  if (!Number.isSafeInteger(modulusLength) || modulusLength < 3072) {
    throw new Error(`${spki.path} evidence signer RSA modulus must be at least 3072 bits.`);
  }
  const signatureValid = crypto.verify(
    "sha256",
    statement.bytes,
    { key: publicKey, padding: crypto.constants.RSA_PKCS1_PADDING },
    signature.bytes,
  );
  if (!signatureValid) throw new Error(`${statement.path} evidence signature is invalid.`);

  return {
    schemaVersion: value.schemaVersion,
    statement: fileDescriptor(statement),
    signature: fileDescriptor(signature),
    signerSpki: fileDescriptor(spki),
    packageVerification: fileDescriptor(packageVerification),
    releaseStatement: fileDescriptor(releaseStatement),
    releaseSignature: fileDescriptor(releaseSignature),
    trustedCaptureSpkiSha256: trustedSpki,
    trustedReleaseSignerThumbprint: trustedReleaseSigner,
  };
}

function validateReleaseStatement(release, attestation, label) {
  assertClosedObject(release, label, [
    "schema",
    "candidate",
    "exeSha256",
    "packageManifestSha256",
    "packageManifestAggregateSha256",
    "versionSha256",
    "buildAttestationSha256",
    "gitRevision",
    "worktreeDirty",
    "worktreeStatusSha256",
  ]);
  assertEqual(release.schema, "mir2.windows.release-statement.v1", `${label} schema`);
  assertEqual(release.candidate, attestation.candidate, `${label} candidate`);
  assertEqual(String(release.exeSha256).toLowerCase(), attestation.exeSha256, `${label} EXE SHA-256`);
  assertEqual(
    String(release.packageManifestSha256).toLowerCase(),
    attestation.packageManifestSha256,
    `${label} package-manifest SHA-256`,
  );
  assertEqual(release.gitRevision, attestation.sourceRevision, `${label} source revision`);
  if (release.worktreeDirty !== false) throw new Error(`${label} must bind a clean worktree.`);
  for (const field of [
    "exeSha256",
    "packageManifestSha256",
    "packageManifestAggregateSha256",
    "versionSha256",
    "buildAttestationSha256",
    "worktreeStatusSha256",
  ]) {
    assertSha256(release[field], `${label} ${field}`);
  }
}

function validatePackageVerification(
  verification,
  attestation,
  releaseStatement,
  trustedReleaseSigner,
  label,
) {
  if (!verification || typeof verification !== "object" || Array.isArray(verification)) {
    throw new Error(`${label} must be an object.`);
  }
  assertEqual(verification.schema, "mir2.windows.package-verification.v4", `${label} schema`);
  if (verification.passed !== true || verification.detachedSignatureValid !== true) {
    throw new Error(`${label} did not pass detached Candidate verification.`);
  }
  if (
    verification.nonvisual !== true
    || verification.launchRequested !== false
    || verification.visualAccepted !== false
  ) {
    throw new Error(`${label} must be a nonvisual package verification result.`);
  }
  if (
    verification.attestationPresent !== true
    || verification.packageManifestPresent !== true
    || verification.peValid !== true
  ) {
    throw new Error(`${label} is missing a validated attestation, manifest, or PE image.`);
  }
  if (!Array.isArray(verification.failures) || verification.failures.length !== 0) {
    throw new Error(`${label} must contain an empty failures array.`);
  }
  assertEqual(
    normalizeThumbprint(verification.trustedSignerThumbprint, `${label} trusted signer`),
    trustedReleaseSigner,
    `${label} trusted release signer`,
  );
  assertEqual(String(verification.exeSha256).toLowerCase(), attestation.exeSha256, `${label} EXE SHA-256`);
  assertEqual(
    String(verification.packageManifestSha256).toLowerCase(),
    attestation.packageManifestSha256,
    `${label} package-manifest SHA-256`,
  );
  assertEqual(
    String(verification.packageManifestAggregateSha256).toLowerCase(),
    String(releaseStatement.packageManifestAggregateSha256).toLowerCase(),
    `${label} package-manifest aggregate SHA-256`,
  );
  assertEqual(
    String(verification.attestationSha256).toLowerCase(),
    String(releaseStatement.buildAttestationSha256).toLowerCase(),
    `${label} build-attestation SHA-256`,
  );
  if (!Number.isSafeInteger(verification.packageFileCount) || verification.packageFileCount <= 0) {
    throw new Error(`${label} packageFileCount must be positive.`);
  }
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

async function readBinaryFile(value, flag, { maxBytes = 1024 * 1024 } = {}) {
  const filePath = await requireFile(value, flag);
  const bytes = await fs.readFile(filePath);
  if (bytes.length === 0) throw new Error(`${flag} must not be empty: ${filePath}`);
  if (bytes.length > maxBytes) {
    throw new Error(`${flag} exceeds the ${maxBytes}-byte limit: ${filePath}`);
  }
  return {
    path: filePath,
    bytes,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
}

async function readJsonFile(value, flag, { canonical = false } = {}) {
  const binary = await readBinaryFile(value, flag);
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(binary.bytes);
  } catch (error) {
    throw new Error(`${flag} is not valid UTF-8: ${binary.path}: ${error.message}`);
  }
  if (text.charCodeAt(0) === 0xfeff) {
    throw new Error(`${flag} must not contain a UTF-8 BOM: ${binary.path}`);
  }
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch (error) {
    throw new Error(`${flag} is not valid JSON: ${binary.path}: ${error.message}`);
  }
  if (canonical && JSON.stringify(parsed) !== text) {
    throw new Error(`${flag} must be compact canonical JSON with no BOM or surrounding whitespace: ${binary.path}`);
  }
  return { ...binary, text, value: parsed };
}

async function requireFile(value, flag) {
  if (!value || value === true) throw new Error(`${flag} is required.`);
  const filePath = path.resolve(String(value));
  const stat = await fs.lstat(filePath).catch(() => null);
  if (!stat?.isFile()) throw new Error(`${flag} file does not exist or is not a regular file: ${filePath}`);
  if (stat.isSymbolicLink()) throw new Error(`${flag} must not be a symbolic link: ${filePath}`);
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

function normalizeSha256(value, label) {
  if (typeof value !== "string") throw new Error(`${label} must be a SHA-256 hex digest.`);
  const normalized = value.trim().toLowerCase();
  if (!SHA256_PATTERN.test(normalized)) throw new Error(`${label} must be a SHA-256 hex digest.`);
  return normalized;
}

function normalizeThumbprint(value, label) {
  if (typeof value !== "string") throw new Error(`${label} must be a SHA-1 certificate thumbprint.`);
  const normalized = value.replace(/[\s:]/g, "").toUpperCase();
  if (!SHA1_THUMBPRINT_PATTERN.test(normalized)) {
    throw new Error(`${label} must be a 40-character SHA-1 certificate thumbprint.`);
  }
  return normalized;
}

function normalizeCandidate(value, label) {
  if (typeof value !== "string" || !/^WN-CANDIDATE-[A-Za-z0-9._-]+$/.test(value)) {
    throw new Error(`${label} must be a formal WN-CANDIDATE identity.`);
  }
  return value;
}

function normalizeRevision(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(`${label} must be a lowercase 40-character Git revision.`);
  }
  return value;
}

function strictTimestamp(value, label) {
  if (typeof value !== "string") throw new Error(`${label} must be an explicit UTC timestamp.`);
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,7}))?(?:Z|\+00:00)$/.exec(value);
  if (!match) throw new Error(`${label} must be an explicit UTC ISO-8601 timestamp.`);
  const [year, month, day, hour, minute, second] = match.slice(1, 7).map(Number);
  const millisecond = Number((match[7] ?? "").padEnd(3, "0").slice(0, 3));
  if (year < 2000) throw new Error(`${label} has an unsupported calendar year.`);
  const timestamp = Date.UTC(year, month - 1, day, hour, minute, second, millisecond);
  const roundTrip = new Date(timestamp);
  if (
    roundTrip.getUTCFullYear() !== year
    || roundTrip.getUTCMonth() !== month - 1
    || roundTrip.getUTCDate() !== day
    || roundTrip.getUTCHours() !== hour
    || roundTrip.getUTCMinutes() !== minute
    || roundTrip.getUTCSeconds() !== second
    || roundTrip.getUTCMilliseconds() !== millisecond
  ) {
    throw new Error(`${label} is not a valid calendar timestamp.`);
  }
  return timestamp;
}

function fileDescriptor(evidence) {
  return {
    path: evidence.path,
    bytes: evidence.bytes.length,
    sha256: evidence.sha256,
  };
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
    candidateAttestationPath: args["candidate-attestation"],
    candidateAttestationSignaturePath: args["candidate-attestation-signature"],
    candidateAttestationSpkiPath: args["candidate-attestation-spki"],
    candidatePackageVerificationPath: args["candidate-package-verification"],
    candidateReleaseStatementPath: args["candidate-release-statement"],
    candidateReleaseSignaturePath: args["candidate-release-signature"],
    trustedCaptureSpkiSha256: args["trusted-capture-spki-sha256"],
    trustedReleaseSignerThumbprint: args["trusted-release-signer-thumbprint"],
    expectedCandidate: args["expected-candidate"],
    expectedSourceRevision: args["expected-source-revision"],
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
    --candidate-image <native.png> --candidate-state <native.json> \\
    --candidate-attestation <capture-attestation.json> \\
    --candidate-attestation-signature <capture-attestation.sig> \\
    --candidate-attestation-spki <capture-attestation.spki.der> \\
    --candidate-package-verification <package-verification.json> \\
    --candidate-release-statement <RELEASE-STATEMENT.json> \\
    --candidate-release-signature <RELEASE-STATEMENT.p7s> \\
    --trusted-capture-spki-sha256 <sha256> \\
    --trusted-release-signer-thumbprint <sha1> \\
    --expected-candidate <WN-CANDIDATE-id> \\
    --expected-source-revision <40-char-git-sha> [options]

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
