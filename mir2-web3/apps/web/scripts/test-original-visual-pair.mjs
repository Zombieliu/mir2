import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { deflateSync } from "node:zlib";

import {
  captureOriginalVisualPair,
  createSidecarFromObserved,
  parseOriginalCaptureArgs,
} from "./capture-original-visual-pair.mjs";
import { buildOriginalAssetManifest } from "./build-crystal-original-asset-manifest.mjs";
import { prepareOriginalVisualEvidence } from "./prepare-original-visual-evidence.mjs";
import { verifyVisualPair } from "./verify-native-visual-pair.mjs";

const RELEASE_SIGNER_THUMBPRINT = "C3".repeat(20);
const { privateKey: capturePrivateKey, publicKey: capturePublicKey } = crypto.generateKeyPairSync(
  "rsa",
  { modulusLength: 3072 },
);
const captureSpki = capturePublicKey.export({ format: "der", type: "spki" });
const captureSpkiSha256 = crypto.createHash("sha256").update(captureSpki).digest("hex");

function crc32(bytes) { let crc = 0xffffffff; for (const byte of bytes) { crc ^= byte; for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0); } return (crc ^ 0xffffffff) >>> 0; }
function chunk(type, data) { const body = Buffer.from(data); const header = Buffer.alloc(8); header.writeUInt32BE(body.length); header.write(type, 4, 4, "ascii"); const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(Buffer.concat([Buffer.from(type), body]))); return Buffer.concat([header, body, crc]); }
function validPng() { const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(1024); ihdr.writeUInt32BE(768, 4); ihdr[8] = 8; ihdr[9] = 2; const raw = Buffer.alloc((1024 * 3 + 1) * 768); return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]); }
async function fixture() { const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-original-pair-")); const output = path.join(root, "original.png"); const executable = path.join(root, "Client.exe"); const manifest = path.join(root, "PACKAGE-MANIFEST.json"); await fs.writeFile(output, validPng()); await fs.writeFile(executable, "fixture executable"); await fs.writeFile(manifest, "fixture assets"); const now = new Date().toISOString(); return { root, output, executable, manifest, now, observed: { ok: true, capturedAt: now, process: { pid: 42, name: "Client" }, window: { handle: 7, clientArea: { width: 1024, height: 768 } }, executable: { path: executable }, dpi: { value: 96, scale: 1 }, image: { path: output, bytes: (await fs.stat(output)).size, width: 1024, height: 768 } } }; }
const hash = async (file) => crypto.createHash("sha256").update(await fs.readFile(file)).digest("hex");
const NATIVE_HUD_UI_STATE = "shell=in-game;screen=InGame;panel=None;minimap=true;chatFocused=false;security=None;inspect=false;inventoryOperation=false;dropConfirm=false";
const optionsFor = (f) => ({
  output: f.output,
  "run-id": "vis-pair-001",
  scene: "in-game",
  "ui-state": NATIVE_HUD_UI_STATE,
  "process-id": 42,
  __executablePath: f.executable,
});

async function writeCandidateAttestation(root, statePath, imagePath, candidateState) {
  const candidateName = "WN-CANDIDATE-ORIGINAL-PAIR-TEST";
  const release = {
    schema: "mir2.windows.release-statement.v1",
    candidate: candidateName,
    exeSha256: candidateState.build.executableSha256,
    packageManifestSha256: candidateState.build.assetManifestSha256,
    packageManifestAggregateSha256: "9".repeat(64),
    versionSha256: "a".repeat(64),
    buildAttestationSha256: "b".repeat(64),
    gitRevision: candidateState.build.sourceRevision,
    worktreeDirty: false,
    worktreeStatusSha256: "c".repeat(64),
  };
  const releaseBytes = Buffer.from(JSON.stringify(release), "utf8");
  const releasePath = path.join(root, "RELEASE-STATEMENT.json");
  const releaseSignaturePath = path.join(root, "RELEASE-STATEMENT.p7s");
  const releaseSignature = Buffer.from("original-pair-detached-cms-fixture", "utf8");
  await fs.writeFile(releasePath, releaseBytes);
  await fs.writeFile(releaseSignaturePath, releaseSignature);

  const verification = {
    schema: "mir2.windows.package-verification.v4",
    packageRoot: root,
    nonvisual: true,
    launchRequested: false,
    sourceRepoCheck: "unavailable",
    currentSourceRevision: "",
    attestationPresent: true,
    attestationSha256: release.buildAttestationSha256.toUpperCase(),
    packageManifestPresent: true,
    packageManifestSha256: candidateState.build.assetManifestSha256.toUpperCase(),
    packageManifestAggregateSha256: release.packageManifestAggregateSha256.toUpperCase(),
    trustedSignerThumbprint: RELEASE_SIGNER_THUMBPRINT,
    detachedSignatureValid: true,
    exeSha256: candidateState.build.executableSha256.toUpperCase(),
    peValid: true,
    peImports: [],
    peDelayImports: [],
    packageFileCount: 12,
    failures: [],
    passed: true,
    visualAccepted: false,
  };
  const verificationBytes = Buffer.from(JSON.stringify(verification), "utf8");
  const verificationPath = path.join(root, "candidate-verification.json");
  await fs.writeFile(verificationPath, verificationBytes);

  const capturedAtMs = Date.parse(candidateState.capturedAt);
  const attestation = {
    schemaVersion: "mir2-native-capture-attestation-v1",
    attestedAt: new Date(capturedAtMs + 2_000).toISOString(),
    runId: candidateState.runId,
    scene: candidateState.scene,
    capturedAt: candidateState.capturedAt,
    stateSha256: await hash(statePath),
    imageSha256: await hash(imagePath),
    processId: 42,
    processStartUtc: new Date(capturedAtMs - 60_000).toISOString(),
    exeSha256: candidateState.build.executableSha256,
    candidate: candidateName,
    sourceRevision: candidateState.build.sourceRevision,
    packageManifestSha256: candidateState.build.assetManifestSha256,
    releaseStatementSha256: crypto.createHash("sha256").update(releaseBytes).digest("hex"),
    releaseSignatureSha256: crypto.createHash("sha256").update(releaseSignature).digest("hex"),
    packageVerificationSha256: crypto.createHash("sha256").update(verificationBytes).digest("hex"),
    trustedReleaseSignerThumbprint: RELEASE_SIGNER_THUMBPRINT,
    signatureAlgorithm: "RSA-PKCS1-SHA256",
    evidenceSignerSpkiSha256: captureSpkiSha256,
  };
  const attestationBytes = Buffer.from(JSON.stringify(attestation), "utf8");
  const attestationPath = path.join(root, "capture-attestation.json");
  const attestationSignaturePath = path.join(root, "capture-attestation.sig");
  const attestationSpkiPath = path.join(root, "capture-attestation.spki.der");
  await fs.writeFile(attestationPath, attestationBytes);
  await fs.writeFile(
    attestationSignaturePath,
    crypto.sign(
      "sha256",
      attestationBytes,
      { key: capturePrivateKey, padding: crypto.constants.RSA_PKCS1_PADDING },
    ),
  );
  await fs.writeFile(attestationSpkiPath, captureSpki);
  return {
    candidateAttestationPath: attestationPath,
    candidateAttestationSignaturePath: attestationSignaturePath,
    candidateAttestationSpkiPath: attestationSpkiPath,
    candidatePackageVerificationPath: verificationPath,
    candidateReleaseStatementPath: releasePath,
    candidateReleaseSignaturePath: releaseSignaturePath,
    trustedCaptureSpkiSha256: captureSpkiSha256,
    trustedReleaseSignerThumbprint: RELEASE_SIGNER_THUMBPRINT,
    expectedCandidate: candidateName,
    expectedSourceRevision: candidateState.build.sourceRevision,
  };
}

function relayState(f, overrides = {}) {
  const generatedAtUnixMs = Date.parse(f.now);
  const observation = (sequence, packet, packetId, marker) => ({
    connectionId: 7,
    sequence,
    observedAtUnixMs: generatedAtUnixMs - 100,
    packet,
    packetId,
    frameSha256: marker.repeat(64),
  });
  return {
    schemaVersion: "mir2-crystal-original-state-evidence-v1",
    producer: "crystal-original-state-relay",
    runId: "vis-pair-001",
    generatedAtUnixMs,
    relay: {
      bind: "127.0.0.1:7010",
      upstream: "127.0.0.1:7000",
      connectionId: 7,
      connectionActive: true,
      connectedAtUnixMs: generatedAtUnixMs - 2_000,
      lastServerSequence: 7,
      decodeErrorCount: 0,
      relayExecutableSha256: "d".repeat(64),
    },
    world: {
      map: "0",
      mapIndex: 0,
      x: 287,
      y: 618,
      direction: 3,
      light: "setting=3;mapDarkLight=0",
    },
    packets: {
      startGame: observation(1, "StartGame", 14, "a"),
      map: observation(2, "MapInformation", 17, "b"),
      position: observation(3, "UserLocation", 23, "c"),
      light: observation(4, "TimeOfDay", 61, "e"),
    },
    acceptance: { eligible: true, blockers: [] },
    ...overrides,
  };
}

test("argument parser rejects unsafe attested text and ambiguous strict window selection", () => {
  assert.throws(() => parseOriginalCaptureArgs(["--output", "a.png", "--run-id", "x", "--scene", "login", "--ui-state", "password=secret"]));
  assert.throws(
    () => parseOriginalCaptureArgs(["--output", "a.png", "--run-id", "x", "--scene", "in-game", "--ui-state", NATIVE_HUD_UI_STATE, "--strict-v1"]),
    /requires --process-id/,
  );
});

test("draft capture records no observed world fields and is ineligible", async (t) => {
  const f = await fixture(); t.after(() => fs.rm(f.root, { recursive: true, force: true }));
  const result = await createSidecarFromObserved(optionsFor(f), f.observed);
  assert.equal(result.sidecar.schemaVersion, "mir2-native-visual-capture-draft-v1");
  assert.equal(result.sidecar.acceptance.eligible, false);
  assert.equal(result.sidecar.evidenceBoundClaims.world, null);
  assert.equal(Object.hasOwn(result.sidecar.observedFacts, "world"), false);
});

test("strict v1 promotes only artifact-bound same-run state, build, and asset evidence", async (t) => {
  const f = await fixture(); t.after(() => fs.rm(f.root, { recursive: true, force: true }));
  const state = path.join(f.root, "state.json"); const build = path.join(f.root, "build.json"); const asset = path.join(f.root, "asset.json");
  await fs.writeFile(state, JSON.stringify(relayState(f)));
  await fs.writeFile(build, JSON.stringify({
    schemaVersion: "mir2-native-build-evidence-v1",
    producer: "crystal-original-build-evidence",
    runId: "vis-pair-001",
    observedAt: f.now,
    sourceRevision: `crystal-original-artifact-${await hash(f.executable)}`,
    executable: { path: f.executable, sha256: await hash(f.executable), bytes: (await fs.stat(f.executable)).size },
  }));
  await fs.writeFile(asset, JSON.stringify({
    schemaVersion: "mir2-native-asset-evidence-v1",
    producer: "crystal-original-asset-evidence",
    runId: "vis-pair-001",
    observedAt: f.now,
    assetManifest: { path: f.manifest, sha256: await hash(f.manifest), bytes: (await fs.stat(f.manifest)).size },
  }));
  const result = await createSidecarFromObserved({ ...optionsFor(f), "strict-v1": true, "state-evidence": state, "build-evidence": build, assetEvidence: [asset] }, f.observed);
  assert.equal(result.sidecar.schemaVersion, "mir2-native-visual-capture-v1");
  assert.deepEqual(result.sidecar.world, { map: "0", x: 287, y: 618, light: "setting=3;mapDarkLight=0" });
  assert.equal(result.sidecar.uiState, NATIVE_HUD_UI_STATE);
  assert.equal(result.sidecar.build.executableSha256, await hash(f.executable));

  const candidateImage = path.join(f.root, "native.png");
  const candidateState = path.join(f.root, "native.json");
  await fs.copyFile(f.output, candidateImage);
  const candidateSidecar = {
    ...result.sidecar,
    producer: "windows-native",
    imagePath: "native.png",
    imageSha256: await hash(candidateImage),
    build: {
      sourceRevision: "d".repeat(40),
      executableSha256: "7".repeat(64),
      assetManifestSha256: "8".repeat(64),
    },
  };
  await fs.writeFile(candidateState, JSON.stringify(candidateSidecar));
  const pair = await verifyVisualPair({
    referenceImagePath: f.output,
    candidateImagePath: candidateImage,
    referenceStatePath: result.sidecarPath,
    candidateStatePath: candidateState,
    ...await writeCandidateAttestation(f.root, candidateState, candidateImage, candidateSidecar),
  });
  assert.equal(pair.alignment.uiState, NATIVE_HUD_UI_STATE);
  assert.deepEqual(pair.alignment.world, result.sidecar.world);
});

test("strict login capture uses operator-attested native UI state and requires no world relay", async (t) => {
  const f = await fixture(); t.after(() => fs.rm(f.root, { recursive: true, force: true }));
  const build = path.join(f.root, "build.json");
  const asset = path.join(f.root, "asset.json");
  await fs.writeFile(build, JSON.stringify({
    schemaVersion: "mir2-native-build-evidence-v1",
    producer: "crystal-original-build-evidence",
    runId: "vis-pair-001",
    observedAt: f.now,
    sourceRevision: `crystal-original-artifact-${await hash(f.executable)}`,
    executable: { path: f.executable, sha256: await hash(f.executable), bytes: (await fs.stat(f.executable)).size },
  }));
  await fs.writeFile(asset, JSON.stringify({
    schemaVersion: "mir2-native-asset-evidence-v1",
    producer: "crystal-original-asset-evidence",
    runId: "vis-pair-001",
    observedAt: f.now,
    assetManifest: { path: f.manifest, sha256: await hash(f.manifest), bytes: (await fs.stat(f.manifest)).size },
  }));
  const result = await createSidecarFromObserved({
    ...optionsFor(f),
    scene: "login",
    "ui-state": "shell=login",
    "strict-v1": true,
    "build-evidence": build,
    assetEvidence: [asset],
  }, f.observed);
  assert.equal(result.sidecar.schemaVersion, "mir2-native-visual-capture-v1");
  assert.equal(result.sidecar.uiState, "shell=login");
  assert.equal(result.sidecar.world, null);
});

test("generated Crystal provenance artifacts promote through the strict capture contract", async (t) => {
  const f = await fixture(); t.after(() => fs.rm(f.root, { recursive: true, force: true }));
  const assetRoot = path.join(f.root, "CrystalClient");
  const manifest = path.join(f.root, "crystal-original-assets.json");
  const evidenceDir = path.join(f.root, "evidence");
  await fs.mkdir(path.join(assetRoot, "Data"), { recursive: true });
  await fs.writeFile(path.join(assetRoot, "Data", "items.dat"), "items");
  await buildOriginalAssetManifest({ assetRoot, output: manifest, includes: ["Data"], generatedAt: f.now });
  const evidence = await prepareOriginalVisualEvidence({
    runId: "vis-pair-001",
    executable: f.executable,
    assetManifest: manifest,
    outputDir: evidenceDir,
    observedAt: f.now,
  });

  const result = await createSidecarFromObserved({
    ...optionsFor(f),
    scene: "login",
    "ui-state": "shell=login",
    "strict-v1": true,
    "build-evidence": evidence.buildEvidencePath,
    assetEvidence: [evidence.assetEvidencePath],
  }, f.observed);
  assert.equal(result.sidecar.schemaVersion, "mir2-native-visual-capture-v1");
  assert.equal(result.sidecar.build.sourceRevision, `crystal-original-artifact-${await hash(f.executable)}`);
  assert.equal(result.sidecar.build.assetManifestSha256, await hash(manifest));
});

test("strict capture fails closed when authoritative state changes during the screenshot", async (t) => {
  const f = await fixture(); t.after(() => fs.rm(f.root, { recursive: true, force: true }));
  const state = path.join(f.root, "state.json");
  const build = path.join(f.root, "build.json");
  const asset = path.join(f.root, "asset.json");
  await fs.writeFile(state, JSON.stringify(relayState(f)));
  await fs.writeFile(build, JSON.stringify({
    schemaVersion: "mir2-native-build-evidence-v1", producer: "crystal-original-build-evidence",
    runId: "vis-pair-001", observedAt: f.now,
    sourceRevision: `crystal-original-artifact-${await hash(f.executable)}`,
    executable: { path: f.executable, sha256: await hash(f.executable), bytes: (await fs.stat(f.executable)).size },
  }));
  await fs.writeFile(asset, JSON.stringify({
    schemaVersion: "mir2-native-asset-evidence-v1", producer: "crystal-original-asset-evidence",
    runId: "vis-pair-001", observedAt: f.now,
    assetManifest: { path: f.manifest, sha256: await hash(f.manifest), bytes: (await fs.stat(f.manifest)).size },
  }));
  const changed = relayState(f);
  changed.world.x = 288;
  changed.packets.position = {
    ...changed.packets.position,
    sequence: 6,
    frameSha256: "f".repeat(64),
  };
  const result = await captureOriginalVisualPair(
    { ...optionsFor(f), "strict-v1": true, "state-evidence": state, "build-evidence": build, assetEvidence: [asset] },
    { runCapture: async () => { await fs.writeFile(state, JSON.stringify(changed)); return f.observed; } },
  );
  assert.equal(result.sidecar.schemaVersion, "mir2-native-visual-capture-draft-v1");
  assert.ok(result.sidecar.acceptance.blockers.some((value) => value.includes("relay_world_across_capture")));
});

test("bad strict evidence produces a draft sidecar with an explicit blocker", async (t) => {
  const f = await fixture(); t.after(() => fs.rm(f.root, { recursive: true, force: true }));
  const state = path.join(f.root, "state.json"); await fs.writeFile(state, "{}");
  const result = await createSidecarFromObserved({ ...optionsFor(f), "strict-v1": true, "state-evidence": state }, f.observed);
  assert.equal(result.sidecar.schemaVersion, "mir2-native-visual-capture-draft-v1");
  assert.ok(result.sidecar.acceptance.blockers.some((value) => value.startsWith("strict_v1_validation_failed")));
});
