#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  assertNoSymlinkAncestors,
  computeRootSha256,
  validateOriginalAssetManifest,
  writeJsonCreateOnly,
} from "./build-crystal-original-asset-manifest.mjs";

export const BUILD_EVIDENCE_SCHEMA = "mir2-native-build-evidence-v1";
export const ASSET_EVIDENCE_SCHEMA = "mir2-native-asset-evidence-v1";
export const BUILD_EVIDENCE_PRODUCER = "crystal-original-build-evidence";
export const ASSET_EVIDENCE_PRODUCER = "crystal-original-asset-evidence";
export const RUN_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]{0,95}$/;

const SHA256_PATTERN = /^[0-9a-f]{64}$/;

export function parseOriginalEvidenceArgs(argv) {
  const result = {};
  const allowed = new Set(["run-id", "executable", "asset-manifest", "output-dir"]);
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "-h" || token === "--help") {
      result.help = true;
      continue;
    }
    if (!token.startsWith("--")) throw new Error(`Unexpected positional argument: ${token}`);
    const equals = token.indexOf("=");
    const key = token.slice(2, equals > 2 ? equals : undefined);
    if (!allowed.has(key)) throw new Error(`Unknown argument: --${key}`);
    const value = equals > 2 ? token.slice(equals + 1) : argv[++index];
    if (!value || value.startsWith("--")) throw new Error(`--${key} requires a value.`);
    result[key] = value;
  }
  if (result.help) return result;
  for (const key of ["run-id", "executable", "asset-manifest", "output-dir"]) {
    if (!result[key]) throw new Error(`--${key} is required.`);
  }
  assertRunId(result["run-id"]);
  return result;
}

export async function prepareOriginalVisualEvidence({ runId, executable, assetManifest, outputDir, observedAt = new Date().toISOString() }) {
  assertRunId(runId);
  assertIsoTimestamp(observedAt, "observedAt");
  if (typeof executable !== "string" || !executable) throw new Error("executable is required");
  if (typeof assetManifest !== "string" || !assetManifest) throw new Error("assetManifest is required");
  if (typeof outputDir !== "string" || !outputDir) throw new Error("outputDir is required");

  const executablePath = path.resolve(executable);
  const assetManifestPath = path.resolve(assetManifest);
  const evidenceDirectory = path.resolve(outputDir);
  const buildEvidencePath = path.join(evidenceDirectory, "original-build-evidence.json");
  const assetEvidencePath = path.join(evidenceDirectory, "original-asset-evidence.json");
  await assertAbsent(buildEvidencePath, "original-build-evidence.json");
  await assertAbsent(assetEvidencePath, "original-asset-evidence.json");

  const executableDescriptor = await describeRegularFile(executablePath, "executable");
  const manifestDescriptor = await describeRegularFile(assetManifestPath, "asset manifest");
  let manifest;
  try {
    manifest = JSON.parse(await fs.readFile(manifestDescriptor.path, "utf8"));
  } catch (error) {
    throw new Error(`asset manifest is not valid JSON: ${error.message}`);
  }
  validateOriginalAssetManifest(manifest);
  // Recompute from the parsed entries as a second explicit check at the evidence boundary.
  if (manifest.rootSha256 !== computeRootSha256(manifest.files)) {
    throw new Error("asset manifest rootSha256 does not match canonical entries");
  }
  await ensureDirectory(evidenceDirectory);

  const buildEvidence = {
    schemaVersion: BUILD_EVIDENCE_SCHEMA,
    producer: BUILD_EVIDENCE_PRODUCER,
    runId,
    observedAt,
    sourceRevision: `crystal-original-artifact-${executableDescriptor.sha256}`,
    executable: {
      path: relativeEvidencePath(evidenceDirectory, executableDescriptor.path, "executable"),
      sha256: executableDescriptor.sha256,
      bytes: executableDescriptor.bytes,
    },
  };
  const assetEvidence = {
    schemaVersion: ASSET_EVIDENCE_SCHEMA,
    producer: ASSET_EVIDENCE_PRODUCER,
    runId,
    observedAt,
    assetManifest: {
      path: relativeEvidencePath(evidenceDirectory, manifestDescriptor.path, "asset manifest"),
      sha256: manifestDescriptor.sha256,
      bytes: manifestDescriptor.bytes,
    },
  };

  // Both targets are preflighted above. Each file is created atomically and
  // strict capture remains fail-closed unless both artifacts exist.
  await writeJsonCreateOnly(buildEvidencePath, buildEvidence);
  try {
    await writeJsonCreateOnly(assetEvidencePath, assetEvidence);
  } catch (error) {
    // Do not delete a path after a competing writer may have raced this process.
    // A partial pair cannot become acceptance evidence because both are required.
    throw error;
  }
  return {
    outputDir: evidenceDirectory,
    buildEvidencePath,
    assetEvidencePath,
    buildEvidence,
    assetEvidence,
  };
}

async function describeRegularFile(filePath, label) {
  await assertNoSymlinkAncestors(filePath, `${label} path`);
  let stats;
  try {
    stats = await fs.lstat(filePath);
  } catch (error) {
    throw new Error(`${label} cannot be read: ${error.message}`);
  }
  if (stats.isSymbolicLink()) throw new Error(`${label} is a symlink/reparse point: ${filePath}`);
  if (!stats.isFile()) throw new Error(`${label} is not an ordinary file: ${filePath}`);
  const bytes = await fs.readFile(filePath);
  const afterRead = await fs.lstat(filePath);
  if (afterRead.isSymbolicLink() || !afterRead.isFile()) throw new Error(`${label} changed while being read: ${filePath}`);
  return {
    path: filePath,
    bytes: bytes.length,
    sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
  };
}

async function ensureDirectory(directoryPath) {
  try {
    await assertNoSymlinkAncestors(directoryPath, "output-dir path");
    await fs.mkdir(directoryPath, { recursive: true });
    await assertNoSymlinkAncestors(directoryPath, "output-dir path");
    const stats = await fs.lstat(directoryPath);
    if (stats.isSymbolicLink() || !stats.isDirectory()) throw new Error("output-dir is not an ordinary directory");
  } catch (error) {
    throw new Error(`output-dir cannot be prepared: ${error.message}`);
  }
}

async function assertAbsent(target, label) {
  try {
    await fs.lstat(target);
  } catch (error) {
    if (error?.code === "ENOENT") return;
    throw new Error(`${label} cannot be checked: ${error.message}`);
  }
  throw new Error(`refusing to overwrite existing ${label}: ${target}`);
}

function relativeEvidencePath(baseDirectory, target, label) {
  const relative = path.relative(baseDirectory, target).split(path.sep).join("/");
  if (!relative || path.isAbsolute(relative) || /^[A-Za-z]:[\\/]/.test(relative) || relative.includes("\\")) {
    throw new Error(`${label} must be on the same volume as output-dir and have a relative evidence path`);
  }
  return relative;
}

function assertRunId(value) {
  if (typeof value !== "string" || !RUN_ID_PATTERN.test(value)) throw new Error("run-id must match [A-Za-z0-9][A-Za-z0-9._-]{0,95}");
}

function assertIsoTimestamp(value, label) {
  if (typeof value !== "string" || !Number.isFinite(Date.parse(value))) throw new Error(`${label} must be an ISO timestamp`);
}

function printHelp() {
  console.log("Usage: node prepare-original-visual-evidence.mjs --run-id <id> --executable <file> --asset-manifest <json> --output-dir <dir>");
}

const currentFile = fileURLToPath(import.meta.url);
if (process.argv[1] && path.resolve(process.argv[1]) === currentFile) {
  try {
    const options = parseOriginalEvidenceArgs(process.argv.slice(2));
    if (options.help) {
      printHelp();
    } else {
      const result = await prepareOriginalVisualEvidence({
        runId: options["run-id"],
        executable: options.executable,
        assetManifest: options["asset-manifest"],
        outputDir: options["output-dir"],
      });
      console.log(JSON.stringify({ ok: true, buildEvidencePath: result.buildEvidencePath, assetEvidencePath: result.assetEvidencePath }));
    }
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
