#!/usr/bin/env node

import { spawn } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REQUIRED_WIDTH = 1024;
const REQUIRED_HEIGHT = 768;
const MAX_EVIDENCE_DELTA_MS = 5 * 60 * 1000;
const MAX_RELAY_CAPTURE_DELTA_MS = 10 * 1000;
const RUN_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,95}$/;
const SAFE_LABEL_PATTERN = /^[a-z0-9][a-z0-9._-]{0,95}$/;
const SAFE_PROCESS_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,95}$/;
const SAFE_MAP_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._/\\-]{0,159}$/;
const LIGHT_STATE_PATTERN = /^setting=[1-4];mapDarkLight=[0-4]$/;
const SHELL_SLUG = "(?:connecting|login|authenticating|character-select|character-create|starting-game|in-game|connection-lost|change-password|safe-key|delete-confirm)";
const UI_ENUM = "[A-Z][A-Za-z0-9]{0,31}";
const UI_BOOLEAN = "(?:true|false)";
const UI_STATE_PATTERN = new RegExp(
  `^shell=${SHELL_SLUG}(?:;screen=${UI_ENUM};panel=${UI_ENUM};minimap=${UI_BOOLEAN};chatFocused=${UI_BOOLEAN};security=${UI_ENUM};inspect=${UI_BOOLEAN};inventoryOperation=${UI_BOOLEAN};dropConfirm=${UI_BOOLEAN})?$`,
);
const SHA256_PATTERN = /^[0-9a-f]{64}$/;
const SCENES = new Set(["login", "character-select", "in-game", "quest-accepted", "combat", "quest-complete"]);
const WORLD_SCENES = new Set(["in-game", "quest-accepted", "combat", "quest-complete"]);

export function parseOriginalCaptureArgs(argv) {
  const result = { assetEvidence: [] };
  const values = new Set([
    "output", "sidecar", "window-title-pattern", "run-id", "scene", "ui-state",
    "state-evidence", "build-evidence", "asset-evidence", "powershell", "process-id",
  ]);
  const booleans = new Set(["strict-v1", "help", "h"]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "-h") { result.help = true; continue; }
    if (!token.startsWith("--")) throw new Error(`Unexpected positional argument: ${token}`);
    const equals = token.indexOf("=");
    const key = token.slice(2, equals > 2 ? equals : undefined);
    if (!values.has(key) && !booleans.has(key)) throw new Error(`Unknown argument: --${key}`);
    if (booleans.has(key)) {
      if (equals > 2) throw new Error(`--${key} does not take a value.`);
      result[key] = true;
      continue;
    }
    const value = equals > 2 ? token.slice(equals + 1) : argv[++index];
    if (!value || value.startsWith("--")) throw new Error(`--${key} requires a value.`);
    if (key === "asset-evidence") result.assetEvidence.push(value);
    else result[key] = value;
  }
  if (result.help || result.h) return result;
  for (const key of ["output", "run-id", "scene", "ui-state"]) {
    if (!result[key]) throw new Error(`--${key} is required.`);
  }
  assertRunId(result["run-id"]);
  assertScene(result.scene);
  assertUiState(result["ui-state"], "--ui-state");
  if (result["process-id"] !== undefined) result["process-id"] = positiveInteger(result["process-id"], "--process-id");
  if (result["strict-v1"] && result["process-id"] === undefined) {
    throw new Error("--strict-v1 requires --process-id so the original and native windows cannot be confused.");
  }
  return result;
}

export async function captureOriginalVisualPair(options, { runCapture = runPowerShellCapture } = {}) {
  const outputPath = path.resolve(options.output);
  const sidecarPath = path.resolve(options.sidecar ?? `${outputPath}.json`);
  if (path.extname(outputPath).toLowerCase() !== ".png") throw new Error("--output must end in .png.");
  if (await isFile(sidecarPath)) throw new Error(`Refusing to overwrite existing sidecar: ${sidecarPath}`);
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.mkdir(path.dirname(sidecarPath), { recursive: true });

  let stateBeforeCapture = null;
  if (options["strict-v1"] && options["state-evidence"]) {
    try {
      stateBeforeCapture = await readJsonArtifact(options["state-evidence"], "pre-capture state evidence");
    } catch (error) {
      stateBeforeCapture = { error };
    }
  }

  const observed = await runCapture({
    outputPath,
    windowTitlePattern: options["window-title-pattern"] ?? "*Legend of Mir 2*",
    powershell: options.powershell,
    processId: options["process-id"],
  });
  const image = await describePng(outputPath, "capture output");
  const observedFacts = validateObservedMetadata(observed, image, outputPath);
  const suppliedEvidence = await collectSuppliedEvidence({ ...options, __executablePath: observedFacts.executablePath });
  const draft = buildDraftSidecar(options, outputPath, image, observedFacts, suppliedEvidence);
  if (stateBeforeCapture?.error) {
    draft.acceptance.blockers.push(`pre_capture_state_evidence_invalid:${safeErrorCode(stateBeforeCapture.error)}`);
  }
  let sidecar = draft;
  if (options["strict-v1"]) {
    try {
      sidecar = promoteStrictV1(
        options,
        outputPath,
        image,
        observedFacts,
        suppliedEvidence,
        stateBeforeCapture,
      );
    } catch (error) {
      draft.acceptance.blockers.push(`strict_v1_validation_failed:${safeErrorCode(error)}`);
    }
  }
  await writeJsonAtomically(sidecarPath, sidecar);
  return { sidecarPath, sidecar, image, observedFacts };
}

function buildDraftSidecar(options, outputPath, image, observed, evidence) {
  const blockers = ["strict_v1_not_promoted", ...evidence.blockers];
  if (!options["strict-v1"]) blockers.push("strict_v1_not_requested");
  return {
    schemaVersion: "mir2-native-visual-capture-draft-v1",
    producer: "crystal-original",
    observedFacts: {
      capturedAt: observed.capturedAt,
      process: observed.process,
      window: observed.window,
      dpi: observed.dpi,
      image: imageDescriptor(image, outputPath),
      executable: evidence.executable,
    },
    evidenceBoundClaims: {
      suppliedArtifacts: evidence.artifacts,
      build: null,
      world: null,
    },
    operatorAttestedClaims: {
      runId: options["run-id"],
      scene: options.scene,
      uiState: options["ui-state"],
    },
    acceptance: { eligible: false, blockers },
  };
}

function promoteStrictV1(options, outputPath, image, observed, evidence, stateBeforeCapture) {
  const state = evidence.parsed.state;
  const build = evidence.parsed.build;
  const asset = evidence.parsed.asset;
  if (!build || !asset) {
    throw new Error("missing required strict evidence artifacts");
  }
  if (!Number.isSafeInteger(options["process-id"]) || options["process-id"] !== observed.process.pid) {
    throw new Error("strict capture processId does not match the observed original process");
  }
  let world = null;
  if (WORLD_SCENES.has(options.scene)) {
    if (!stateBeforeCapture || stateBeforeCapture.error || !state) {
      throw new Error("world capture requires pre/post relay state evidence");
    }
    const before = validateCrystalOriginalStateRelay(
      stateBeforeCapture.value,
      stateBeforeCapture.descriptor,
      options,
      observed.capturedAt,
      "pre-capture state evidence",
    );
    const after = validateCrystalOriginalStateRelay(
      state.value,
      state.descriptor,
      options,
      observed.capturedAt,
      "post-capture state evidence",
    );
    assertStableRelayState(before, after);
    world = after.world;
  }
  validateBuildEvidence(build.value, options, observed.capturedAt);
  validateAssetEvidence(asset.value, options, observed.capturedAt);
  assertEvidenceTimestamp(build.descriptor, build.value.observedAt, observed.capturedAt, "build evidence");
  assertEvidenceTimestamp(asset.descriptor, asset.value.observedAt, observed.capturedAt, "asset evidence");
  const executable = build.declaredExecutable;
  assertSamePath(executable.path, evidence.executable.path, "build executable path");
  assertEqual(executable.sha256, evidence.executable.sha256, "build executable hash");
  assertEqual(
    build.value.sourceRevision,
    `crystal-original-artifact-${executable.sha256}`,
    "content-addressed Crystal sourceRevision",
  );
  const manifest = asset.declaredAssetManifest;
  if (typeof build.value.sourceRevision !== "string" || !SAFE_LABEL_PATTERN.test(build.value.sourceRevision)) {
    throw new Error("build evidence sourceRevision must be a safe identifier");
  }
  return {
    schemaVersion: "mir2-native-visual-capture-v1",
    producer: "crystal-original",
    runId: options["run-id"],
    scene: options.scene,
    capturedAt: observed.capturedAt,
    imagePath: relativePath(path.dirname(options.sidecar ?? `${outputPath}.json`), outputPath),
    imageSha256: image.sha256,
    logicalSize: { width: REQUIRED_WIDTH, height: REQUIRED_HEIGHT },
    dpiScale: observed.dpi.scale,
    uiState: options["ui-state"],
    world,
    build: {
      sourceRevision: build.value.sourceRevision,
      executableSha256: executable.sha256,
      assetManifestSha256: manifest.sha256,
    },
  };
}

async function collectSuppliedEvidence(options) {
  const entries = [
    ["state", options["state-evidence"]],
    ["build", options["build-evidence"]],
    ["asset", options.assetEvidence?.length === 1 ? options.assetEvidence[0] : null],
  ];
  const result = { artifacts: {}, parsed: {}, blockers: [] };
  if ((options.assetEvidence?.length ?? 0) > 1) result.blockers.push("asset_evidence_must_be_exactly_one_file");
  for (const [kind, value] of entries) {
    if (!value) {
      result.blockers.push(`${kind}_evidence_missing`);
      continue;
    }
    try {
      const parsedEntry = await readJsonArtifact(value, `${kind} evidence`);
      const { descriptor } = parsedEntry;
      result.artifacts[kind] = descriptor;
      const parsed = parsedEntry.value;
      if (kind === "build" && parsed?.executable) {
        parsedEntry.declaredExecutable = await describeDeclaredFile(parsed.executable, descriptor.path, "build executable");
      }
      if (kind === "asset" && parsed?.assetManifest) {
        parsedEntry.declaredAssetManifest = await describeDeclaredFile(parsed.assetManifest, descriptor.path, "asset manifest");
      }
      result.parsed[kind] = parsedEntry;
    } catch (error) {
      result.blockers.push(`${kind}_evidence_invalid:${safeErrorCode(error)}`);
    }
  }
  const executablePath = options.__executablePath;
  result.executable = await describeFile(executablePath, "observed executable");
  return result;
}

async function runPowerShellCapture({ outputPath, windowTitlePattern, powershell, processId }) {
  const scriptPath = path.join(SCRIPT_DIR, "capture-original-visual-pair.ps1");
  const executable = powershell ?? "powershell.exe";
  const args = [
    "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", scriptPath,
    "-OutputPath", outputPath,
    "-WindowTitlePattern", windowTitlePattern,
  ];
  if (processId !== undefined) args.push("-ProcessId", String(processId));
  const result = await runProcess(executable, args);
  if (result.exitCode !== 0) throw new Error(`PowerShell capture failed: ${result.stderr.trim() || result.stdout.trim()}`);
  try { return JSON.parse(result.stdout); }
  catch (error) { throw new Error(`PowerShell capture did not return JSON metadata: ${error.message}`); }
}

function validateObservedMetadata(value, image, outputPath) {
  if (!value || value.ok !== true) throw new Error("PowerShell capture did not report success");
  assertIsoTimestamp(value.capturedAt, "capture timestamp");
  if (!Number.isSafeInteger(value?.process?.pid) || value.process.pid <= 0 || !SAFE_PROCESS_PATTERN.test(value.process.name ?? "")) {
    throw new Error("capture process metadata is invalid");
  }
  if (!Number.isSafeInteger(value?.window?.handle) || value.window.handle === 0) throw new Error("capture window handle is invalid");
  assertEqual(value?.window?.clientArea?.width, REQUIRED_WIDTH, "capture client width");
  assertEqual(value?.window?.clientArea?.height, REQUIRED_HEIGHT, "capture client height");
  if (typeof value?.executable?.path !== "string" || !value.executable.path) throw new Error("capture executable path is missing");
  assertSamePath(value.image?.path, outputPath, "capture image path");
  assertEqual(value.image?.bytes, image.bytes, "capture image byte length");
  assertEqual(value.image?.width, REQUIRED_WIDTH, "capture image width");
  assertEqual(value.image?.height, REQUIRED_HEIGHT, "capture image height");
  if (!Number.isFinite(value?.dpi?.scale) || value.dpi.scale < 0.5 || value.dpi.scale > 4) throw new Error("capture DPI scale is invalid");
  return {
    capturedAt: value.capturedAt,
    process: { pid: value.process.pid, name: value.process.name },
    window: { handle: value.window.handle, clientArea: { width: REQUIRED_WIDTH, height: REQUIRED_HEIGHT } },
    dpi: { value: value.dpi.value, scale: value.dpi.scale },
    executablePath: path.resolve(value.executable.path),
  };
}

export async function createSidecarFromObserved(options, observed) {
  const normalized = { ...options, __executablePath: observed.executablePath };
  return captureOriginalVisualPair(normalized, { runCapture: async () => observed });
}

async function describePng(value, label) {
  const descriptor = await describeFile(value, label);
  const bytes = await fs.readFile(descriptor.path);
  const dimensions = validatePngBytes(bytes, label);
  if (dimensions.width !== REQUIRED_WIDTH || dimensions.height !== REQUIRED_HEIGHT) {
    throw new Error(`${label} must be ${REQUIRED_WIDTH}x${REQUIRED_HEIGHT}`);
  }
  return { ...descriptor, ...dimensions };
}

async function describeFile(value, label) {
  if (!value) throw new Error(`${label} path is missing`);
  const filePath = path.resolve(value);
  const stat = await fs.stat(filePath);
  if (!stat.isFile()) throw new Error(`${label} is not a file`);
  const bytes = await fs.readFile(filePath);
  return { path: filePath, bytes: bytes.length, sha256: crypto.createHash("sha256").update(bytes).digest("hex"), modifiedAt: stat.mtime.toISOString() };
}

async function readJsonArtifact(value, label) {
  const descriptor = await describeFile(value, label);
  try {
    return { descriptor, value: JSON.parse(await fs.readFile(descriptor.path, "utf8")) };
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error.message}`);
  }
}

function imageDescriptor(image, outputPath) {
  return { path: outputPath, bytes: image.bytes, sha256: image.sha256, width: image.width, height: image.height, modifiedAt: image.modifiedAt };
}

async function describeDeclaredFile(value, artifactPath, label) {
  assertClosedObject(value, label, ["path", "sha256", "bytes"]);
  if (typeof value.path !== "string" || !value.path) throw new Error(`${label} path is missing`);
  if (typeof value.sha256 !== "string" || !SHA256_PATTERN.test(value.sha256.toLowerCase())) throw new Error(`${label} SHA-256 is invalid`);
  if (!Number.isSafeInteger(value.bytes) || value.bytes <= 0) throw new Error(`${label} byte length is invalid`);
  const resolved = path.resolve(path.dirname(artifactPath), value.path);
  const descriptor = await describeFile(resolved, label);
  if (descriptor.sha256 !== value.sha256.toLowerCase()) throw new Error(`${label} SHA-256 does not match its file`);
  if (descriptor.bytes !== value.bytes) throw new Error(`${label} byte length does not match its file`);
  return descriptor;
}

function validateBuildEvidence(value, options, captureAt) {
  const label = "build evidence";
  assertClosedObject(value, label, ["schemaVersion", "producer", "runId", "observedAt", "sourceRevision", "executable"]);
  assertEqual(value.schemaVersion, "mir2-native-build-evidence-v1", `${label} schemaVersion`);
  assertEqual(value.producer, "crystal-original-build-evidence", `${label} producer`);
  assertEqual(value.runId, options["run-id"], `${label} runId`);
  assertIsoTimestamp(value.observedAt, `${label} observedAt`);
  if (Math.abs(Date.parse(value.observedAt) - Date.parse(captureAt)) > MAX_EVIDENCE_DELTA_MS) throw new Error(`${label} timestamp is outside the same-run window`);
  if (typeof value.sourceRevision !== "string" || !SAFE_LABEL_PATTERN.test(value.sourceRevision)) {
    throw new Error("build evidence sourceRevision must be a safe identifier");
  }
  assertClosedObject(value.executable, `${label}.executable`, ["path", "sha256", "bytes"]);
}

function validateAssetEvidence(value, options, captureAt) {
  const label = "asset evidence";
  assertClosedObject(value, label, ["schemaVersion", "producer", "runId", "observedAt", "assetManifest"]);
  assertEqual(value.schemaVersion, "mir2-native-asset-evidence-v1", `${label} schemaVersion`);
  assertEqual(value.producer, "crystal-original-asset-evidence", `${label} producer`);
  assertEqual(value.runId, options["run-id"], `${label} runId`);
  assertIsoTimestamp(value.observedAt, `${label} observedAt`);
  if (Math.abs(Date.parse(value.observedAt) - Date.parse(captureAt)) > MAX_EVIDENCE_DELTA_MS) throw new Error(`${label} timestamp is outside the same-run window`);
  assertClosedObject(value.assetManifest, `${label}.assetManifest`, ["path", "sha256", "bytes"]);
}

function assertEvidenceTimestamp(descriptor, observedAt, captureAt, label) {
  if (Math.abs(Date.parse(descriptor.modifiedAt) - Date.parse(observedAt)) > MAX_EVIDENCE_DELTA_MS) throw new Error(`${label} file timestamp does not match declared timestamp`);
  if (Math.abs(Date.parse(descriptor.modifiedAt) - Date.parse(captureAt)) > MAX_EVIDENCE_DELTA_MS) throw new Error(`${label} file is outside the capture run`);
}

function validateCrystalOriginalStateRelay(value, descriptor, options, captureAt, label) {
  assertClosedObject(value, label, [
    "schemaVersion", "producer", "runId", "generatedAtUnixMs", "relay", "world", "packets", "acceptance",
  ]);
  assertEqual(value.schemaVersion, "mir2-crystal-original-state-evidence-v1", `${label} schemaVersion`);
  assertEqual(value.producer, "crystal-original-state-relay", `${label} producer`);
  assertEqual(value.runId, options["run-id"], `${label} runId`);
  assertUnixMsNearCapture(value.generatedAtUnixMs, descriptor, captureAt, label);

  assertClosedObject(value.relay, `${label}.relay`, [
    "bind", "upstream", "connectionId", "connectionActive", "connectedAtUnixMs",
    "lastServerSequence", "decodeErrorCount", "relayExecutableSha256",
  ]);
  if (!isLoopbackEndpoint(value.relay.bind) || !isLoopbackEndpoint(value.relay.upstream)) {
    throw new Error(`${label} relay endpoints must both be loopback`);
  }
  if (!Number.isSafeInteger(value.relay.connectionId) || value.relay.connectionId <= 0) {
    throw new Error(`${label} relay.connectionId is invalid`);
  }
  if (value.relay.connectionActive !== true) throw new Error(`${label} relay connection is not active`);
  if (!Number.isSafeInteger(value.relay.connectedAtUnixMs) || value.relay.connectedAtUnixMs <= 0 || value.relay.connectedAtUnixMs > value.generatedAtUnixMs) {
    throw new Error(`${label} relay.connectedAtUnixMs is invalid`);
  }
  if (!Number.isSafeInteger(value.relay.lastServerSequence) || value.relay.lastServerSequence <= 0) {
    throw new Error(`${label} relay.lastServerSequence is invalid`);
  }
  if (!Number.isSafeInteger(value.relay.decodeErrorCount) || value.relay.decodeErrorCount < 0) {
    throw new Error(`${label} relay.decodeErrorCount is invalid`);
  }
  assertSha256(value.relay.relayExecutableSha256, `${label} relayExecutableSha256`);

  assertClosedObject(value.acceptance, `${label}.acceptance`, ["eligible", "blockers"]);
  if (value.acceptance.eligible !== true || !Array.isArray(value.acceptance.blockers) || value.acceptance.blockers.length !== 0) {
    throw new Error(`${label} relay evidence is not acceptance eligible`);
  }

  const world = validateEvidenceWorld(value.world, "in-game", label);
  assertClosedObject(value.packets, `${label}.packets`, ["startGame", "map", "position", "light"]);
  const packetSpecifications = {
    startGame: new Map([["StartGame", 14]]),
    map: new Map([["MapInformation", 17], ["MapChanged", 98]]),
    position: new Map([["UserInformation", 21], ["UserLocation", 23], ["MapChanged", 98]]),
    light: new Map([["TimeOfDay", 61], ["MapInformation", 17], ["MapChanged", 98]]),
  };
  const packets = {};
  for (const [role, allowedPackets] of Object.entries(packetSpecifications)) {
    packets[role] = validateRelayPacketObservation(
      value.packets[role],
      role,
      allowedPackets,
      value.relay,
      value.generatedAtUnixMs,
      label,
    );
  }
  return {
    world: { map: world.map, x: world.x, y: world.y, light: world.light },
    connectionId: value.relay.connectionId,
    packetFingerprint: Object.fromEntries(
      Object.entries(packets).map(([role, packet]) => [role, `${packet.sequence}:${packet.frameSha256}`]),
    ),
  };
}

function validateRelayPacketObservation(value, role, allowedPackets, relay, generatedAtUnixMs, label) {
  const packetLabel = `${label}.packets.${role}`;
  assertClosedObject(value, packetLabel, [
    "connectionId", "sequence", "observedAtUnixMs", "packet", "packetId", "frameSha256",
  ]);
  assertEqual(value.connectionId, relay.connectionId, `${packetLabel}.connectionId`);
  if (!Number.isSafeInteger(value.sequence) || value.sequence <= 0 || value.sequence > relay.lastServerSequence) {
    throw new Error(`${packetLabel}.sequence is invalid`);
  }
  if (!Number.isSafeInteger(value.observedAtUnixMs)
      || value.observedAtUnixMs < relay.connectedAtUnixMs
      || value.observedAtUnixMs > generatedAtUnixMs) {
    throw new Error(`${packetLabel}.observedAtUnixMs is invalid`);
  }
  const expectedPacketId = allowedPackets.get(value.packet);
  if (expectedPacketId === undefined || value.packetId !== expectedPacketId) {
    throw new Error(`${packetLabel} packet name/id is not authoritative for ${role}`);
  }
  assertSha256(value.frameSha256, `${packetLabel}.frameSha256`);
  return value;
}

function assertStableRelayState(before, after) {
  assertEqual(before.connectionId, after.connectionId, "relay connection across capture");
  assertEqual(JSON.stringify(before.world), JSON.stringify(after.world), "relay world across capture");
  assertEqual(
    JSON.stringify(before.packetFingerprint),
    JSON.stringify(after.packetFingerprint),
    "relay authoritative packet evidence across capture",
  );
}

function validateEvidenceWorld(world, scene, label = "state evidence") {
  if (!WORLD_SCENES.has(scene)) {
    if (world !== null) throw new Error("state evidence world must be null outside a world scene");
    return null;
  }
  assertClosedObject(world, `${label}.world`, ["map", "mapIndex", "x", "y", "direction", "light"]);
  if (!SAFE_MAP_PATTERN.test(world.map ?? "")
      || !Number.isSafeInteger(world.mapIndex)
      || !Number.isSafeInteger(world.x)
      || !Number.isSafeInteger(world.y)
      || !Number.isSafeInteger(world.direction)
      || world.direction < 0
      || world.direction > 7
      || !LIGHT_STATE_PATTERN.test(world.light ?? "")) {
    throw new Error(`${label} world fields are invalid`);
  }
  return { map: world.map, x: world.x, y: world.y, light: world.light };
}

function validatePngBytes(bytes, label) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (bytes.length < 45 || !bytes.subarray(0, 8).equals(signature)) throw new Error(`${label} is not a PNG`);
  let offset = 8; let width; let height; let colorType; let sawIhdr = false; let sawIend = false; const idat = [];
  while (offset < bytes.length) {
    if (offset + 12 > bytes.length) throw new Error(`${label} has a truncated PNG chunk`);
    const length = bytes.readUInt32BE(offset); const typeStart = offset + 4; const dataStart = offset + 8; const dataEnd = dataStart + length; const crcEnd = dataEnd + 4;
    if (crcEnd > bytes.length) throw new Error(`${label} has a truncated PNG payload`);
    const type = bytes.toString("ascii", typeStart, dataStart);
    if (crc32(bytes.subarray(typeStart, dataEnd)) !== bytes.readUInt32BE(dataEnd)) throw new Error(`${label} has an invalid ${type} CRC`);
    if (!sawIhdr && type !== "IHDR") throw new Error(`${label} must begin with IHDR`);
    if (type === "IHDR") {
      if (sawIhdr || length !== 13) throw new Error(`${label} has an invalid IHDR`);
      sawIhdr = true; width = bytes.readUInt32BE(dataStart); height = bytes.readUInt32BE(dataStart + 4);
      const bitDepth = bytes[dataStart + 8]; colorType = bytes[dataStart + 9];
      if (bitDepth !== 8 || ![2, 6].includes(colorType) || bytes[dataStart + 10] !== 0 || bytes[dataStart + 11] !== 0 || bytes[dataStart + 12] !== 0) throw new Error(`${label} is not a canonical RGB/RGBA PNG`);
    } else if (type === "IDAT") { if (sawIend) throw new Error(`${label} has IDAT after IEND`); idat.push(bytes.subarray(dataStart, dataEnd)); }
    else if (type === "IEND") { if (length !== 0 || sawIend || crcEnd !== bytes.length) throw new Error(`${label} has an invalid IEND`); sawIend = true; }
    offset = crcEnd;
  }
  if (!sawIhdr || !sawIend || idat.length === 0) throw new Error(`${label} is incomplete`);
  const decoded = inflateSync(Buffer.concat(idat)); const channels = colorType === 6 ? 4 : 3; const rowBytes = width * channels;
  if (decoded.length !== (rowBytes + 1) * height) throw new Error(`${label} has invalid decoded length`);
  for (let row = 0; row < height; row += 1) if (decoded[row * (rowBytes + 1)] > 4) throw new Error(`${label} has an invalid row filter`);
  return { width, height };
}

function crc32(bytes) { let crc = 0xffffffff; for (const byte of bytes) { crc ^= byte; for (let bit = 0; bit < 8; bit += 1) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0); } return (crc ^ 0xffffffff) >>> 0; }
async function writeJsonAtomically(target, value) { const temporary = `${target}.${process.pid}.${crypto.randomUUID()}.tmp`; await fs.writeFile(temporary, `${JSON.stringify(value, null, 2)}\n`, "utf8"); try { await fs.rename(temporary, target); } catch (error) { await fs.rm(temporary, { force: true }); throw error; } }
function relativePath(from, to) { const relative = path.relative(from, to); return relative || path.basename(to); }
function assertRunId(value) { if (!RUN_ID_PATTERN.test(value)) throw new Error("--run-id must be a safe identifier"); }
function assertScene(value) { if (!SCENES.has(value)) throw new Error(`Unsupported scene: ${value}`); }
function assertSafeLabel(value, label) { if (typeof value !== "string" || !SAFE_LABEL_PATTERN.test(value)) throw new Error(`${label} must be a safe non-private identifier`); }
function assertUiState(value, label) { if (typeof value !== "string" || !UI_STATE_PATTERN.test(value)) throw new Error(`${label} must match the native visibility-only UI state contract`); }
function assertIsoTimestamp(value, label) { if (!Number.isFinite(Date.parse(value))) throw new Error(`${label} must be an ISO timestamp`); }
function assertUnixMsNearCapture(value, descriptor, captureAt, label) {
  if (!Number.isSafeInteger(value) || value <= 0) throw new Error(`${label} generatedAtUnixMs is invalid`);
  const captureMs = Date.parse(captureAt);
  if (Math.abs(value - captureMs) > MAX_RELAY_CAPTURE_DELTA_MS) throw new Error(`${label} heartbeat is outside the capture window`);
  if (Math.abs(Date.parse(descriptor.modifiedAt) - value) > MAX_RELAY_CAPTURE_DELTA_MS) throw new Error(`${label} file timestamp does not match its heartbeat`);
}
function isLoopbackEndpoint(value) {
  if (typeof value !== "string") return false;
  const match = /^(?:127\.0\.0\.1|\[::1\]):([0-9]{1,5})$/.exec(value);
  if (!match) return false;
  const port = Number(match[1]);
  return Number.isSafeInteger(port) && port > 0 && port <= 65535;
}
function assertClosedObject(value, label, fields) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} must be an object`);
  const allowed = new Set(fields);
  const unknown = Object.keys(value).filter((field) => !allowed.has(field));
  if (unknown.length > 0) throw new Error(`${label} contains unknown field(s): ${unknown.join(", ")}`);
  const missing = fields.filter((field) => !Object.hasOwn(value, field));
  if (missing.length > 0) throw new Error(`${label} is missing field(s): ${missing.join(", ")}`);
}
function assertSha256(value, label) {
  if (typeof value !== "string" || !SHA256_PATTERN.test(value.toLowerCase())) throw new Error(`${label} is invalid`);
}
function positiveInteger(value, label) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) throw new Error(`${label} must be a positive integer`);
  return parsed;
}
function assertEqual(left, right, label) { if (left !== right) throw new Error(`${label} does not match`); }
function assertSamePath(left, right, label) { const normalize = (value) => process.platform === "win32" ? path.resolve(value).toLowerCase() : path.resolve(value); if (normalize(left) !== normalize(right)) throw new Error(`${label} does not match`); }
function safeErrorCode(error) { return String(error?.message ?? "invalid").toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "").slice(0, 120) || "invalid"; }
async function isFile(value) { return (await fs.stat(value).catch(() => null))?.isFile() ?? false; }
function runProcess(executable, args) { return new Promise((resolve, reject) => { const child = spawn(executable, args, { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] }); let stdout = ""; let stderr = ""; child.stdout.on("data", (chunk) => { stdout += String(chunk); }); child.stderr.on("data", (chunk) => { stderr += String(chunk); }); child.on("error", reject); child.on("close", (exitCode) => resolve({ exitCode: exitCode ?? 1, stdout, stderr })); }); }

function printHelp() { console.log("Usage: node capture-original-visual-pair.mjs --output <png> --run-id <id> --scene <scene> --ui-state <id> [--process-id <pid> --strict-v1 --state-evidence <json> --build-evidence <json> --asset-evidence <json>]"); }
if (import.meta.url === new URL(process.argv[1], "file:").href) {
  try { const options = parseOriginalCaptureArgs(process.argv.slice(2)); if (options.help || options.h) printHelp(); else { const result = await captureOriginalVisualPair(options); console.log(JSON.stringify({ ok: true, sidecarPath: result.sidecarPath, schemaVersion: result.sidecar.schemaVersion, eligible: result.sidecar.acceptance?.eligible ?? true })); } }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
