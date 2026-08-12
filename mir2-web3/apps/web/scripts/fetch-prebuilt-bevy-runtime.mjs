#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(webRoot, "..", "..");
const defaultManifestPath = path.join(webRoot, "lib", "generated", "bevy_runtime_version.json");
const defaultConfigPath = path.join(repoRoot, "config", "production-web-assets.json");
const defaultOutputDir = path.join(webRoot, "public", "bevy-runtime");
const runtimePathPattern = /^public\/bevy-runtime\/pkg-(?:webgpu|webgl2)\/mir2_bevy_runtime(?:_bg\.wasm|\.js)$/;

export async function installPrebuiltRuntime(options = {}) {
  const manifestPath = path.resolve(options.manifestPath ?? defaultManifestPath);
  const configPath = path.resolve(options.configPath ?? defaultConfigPath);
  const outputDir = path.resolve(options.outputDir ?? defaultOutputDir);
  const manifest = await readJson(manifestPath, "runtime version manifest");
  const records = validateRuntimeManifest(manifest, manifestPath);
  const assetBaseUrl = normalizeBaseUrl(
    options.assetBaseUrl ?? process.env.MIR2_BEVY_RUNTIME_BASE_URL ?? (await readAssetBaseUrl(configPath)),
  );

  if (await runtimeMatches(outputDir, records)) {
    return { ok: true, reused: true, version: manifest.version, outputDir, fileCount: records.length };
  }

  await refuseSymlink(outputDir);
  const parentDir = path.dirname(outputDir);
  const nonce = `${process.pid}-${Date.now()}-${crypto.randomBytes(6).toString("hex")}`;
  const stagingDir = path.join(parentDir, `.bevy-runtime-fetch-${nonce}`);
  const backupDir = path.join(parentDir, `.bevy-runtime-backup-${nonce}`);
  await fs.mkdir(stagingDir, { recursive: true });

  let movedExisting = false;
  let published = false;
  try {
    for (const record of records) {
      const packagePath = record.path.replace(/^public\/bevy-runtime\//, "");
      const sourceUrl = `${assetBaseUrl}/bevy-runtime/v/${manifest.version}/${packagePath}`;
      const destination = path.join(stagingDir, ...packagePath.split("/"));
      const bytes = await fetchBytes(sourceUrl, {
        attempts: positiveInteger(options.attempts, 3),
        timeoutMs: positiveInteger(options.timeoutMs, 60_000),
        fetchImpl: options.fetchImpl ?? globalThis.fetch,
      });
      verifyBytes(bytes, record, sourceUrl);
      await fs.mkdir(path.dirname(destination), { recursive: true });
      await fs.writeFile(destination, bytes);
    }

    if (!(await runtimeMatches(stagingDir, records))) {
      throw new Error("Downloaded Bevy runtime failed the final staged verification.");
    }

    try {
      await fs.rename(outputDir, backupDir);
      movedExisting = true;
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
    try {
      await fs.rename(stagingDir, outputDir);
      published = true;
    } catch (error) {
      if (movedExisting) await fs.rename(backupDir, outputDir);
      movedExisting = false;
      throw error;
    }
    if (movedExisting) await fs.rm(backupDir, { recursive: true, force: true });

    return { ok: true, reused: false, version: manifest.version, outputDir, fileCount: records.length };
  } finally {
    if (!published) await fs.rm(stagingDir, { recursive: true, force: true });
  }
}

export function validateRuntimeManifest(manifest, manifestPath = "runtime manifest") {
  const version = String(manifest?.version ?? "").trim();
  if (!/^bevy-[a-f0-9]{16}$/i.test(version)) {
    throw new Error(`Invalid Bevy runtime version in ${manifestPath}.`);
  }
  if (!Array.isArray(manifest?.files) || manifest.files.length !== 4) {
    throw new Error(`Expected exactly four Bevy runtime files in ${manifestPath}.`);
  }

  const records = manifest.files.map((entry) => ({
    path: String(entry?.path ?? ""),
    sha256: String(entry?.sha256 ?? "").toLowerCase(),
  }));
  const uniquePaths = new Set(records.map((record) => record.path));
  if (uniquePaths.size !== 4) throw new Error(`Duplicate Bevy runtime paths in ${manifestPath}.`);
  for (const record of records) {
    if (!runtimePathPattern.test(record.path) || !/^[a-f0-9]{64}$/.test(record.sha256)) {
      throw new Error(`Invalid Bevy runtime file entry in ${manifestPath}: ${JSON.stringify(record)}`);
    }
  }
  for (const backend of ["webgpu", "webgl2"]) {
    for (const suffix of [".js", "_bg.wasm"]) {
      const expected = `public/bevy-runtime/pkg-${backend}/mir2_bevy_runtime${suffix}`;
      if (!uniquePaths.has(expected)) throw new Error(`Missing Bevy runtime file in ${manifestPath}: ${expected}`);
    }
  }
  return records;
}

async function runtimeMatches(outputDir, records) {
  for (const record of records) {
    const packagePath = record.path.replace(/^public\/bevy-runtime\//, "");
    const filePath = path.join(outputDir, ...packagePath.split("/"));
    let bytes;
    try {
      bytes = await fs.readFile(filePath);
    } catch (error) {
      if (error?.code === "ENOENT") return false;
      throw error;
    }
    if (sha256(bytes) !== record.sha256) return false;
  }
  return true;
}

async function fetchBytes(url, { attempts, timeoutMs, fetchImpl }) {
  if (typeof fetchImpl !== "function") throw new Error("This Node.js runtime does not provide fetch().");
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const response = await fetchImpl(url, {
        headers: { accept: "application/wasm, text/javascript, application/octet-stream" },
        redirect: "follow",
        signal: controller.signal,
      });
      if (!response.ok) throw new Error(`HTTP ${response.status} ${response.statusText}`);
      return Buffer.from(await response.arrayBuffer());
    } catch (error) {
      lastError = error;
      if (attempt === attempts) break;
    } finally {
      clearTimeout(timeout);
    }
  }
  throw new Error(`Failed to download ${url} after ${attempts} attempt(s): ${errorMessage(lastError)}`);
}

function verifyBytes(bytes, record, sourceUrl) {
  if (bytes.byteLength === 0) throw new Error(`Downloaded an empty Bevy runtime file: ${sourceUrl}`);
  const actual = sha256(bytes);
  if (actual !== record.sha256) {
    throw new Error(`SHA-256 mismatch for ${sourceUrl}: expected ${record.sha256}, received ${actual}`);
  }
}

async function readAssetBaseUrl(configPath) {
  const config = await readJson(configPath, "production asset config");
  const assetBaseUrl = normalizeBaseUrl(config?.assetBaseUrl);
  const fallbackObjectPrefix = normalizeObjectPrefix(config?.fallbackObjectPrefix);
  if (!fallbackObjectPrefix) return assetBaseUrl;
  if (!fallbackObjectPrefix.startsWith("mir2/v/")) {
    throw new Error(`Invalid Bevy runtime fallback object prefix: ${fallbackObjectPrefix}`);
  }
  const assetOrigin = new URL(assetBaseUrl).origin;
  return `${assetOrigin}/${fallbackObjectPrefix}`;
}

async function readJson(filePath, label) {
  try {
    return JSON.parse(await fs.readFile(filePath, "utf8"));
  } catch (error) {
    throw new Error(`Could not read ${label} ${filePath}: ${errorMessage(error)}`);
  }
}

async function refuseSymlink(filePath) {
  try {
    const stats = await fs.lstat(filePath);
    if (stats.isSymbolicLink()) throw new Error(`Refusing to replace symlinked runtime directory: ${filePath}`);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
}

function normalizeBaseUrl(value) {
  const normalized = String(value ?? "").trim().replace(/\/+$/, "");
  let url;
  try {
    url = new URL(normalized);
  } catch {
    throw new Error(`Invalid Bevy runtime asset base URL: ${normalized || "<empty>"}`);
  }
  if (!/^https?:$/.test(url.protocol)) throw new Error(`Unsupported Bevy runtime URL protocol: ${url.protocol}`);
  return normalized;
}

function normalizeObjectPrefix(value) {
  return String(value ?? "").trim().replace(/^\/+|\/+$/g, "");
}

function positiveInteger(value, fallback) {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback;
}

function sha256(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (!value.startsWith("--")) continue;
    const key = value.slice(2);
    const next = values[index + 1];
    parsed[key] = next && !next.startsWith("--") ? values[++index] : true;
  }
  return parsed;
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const args = parseArgs(process.argv.slice(2));
  try {
    const result = await installPrebuiltRuntime({
      manifestPath: args.manifest,
      configPath: args.config,
      outputDir: args.output,
      assetBaseUrl: args.assetBaseUrl,
      attempts: args.attempts,
      timeoutMs: args.timeoutMs,
    });
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(`[bevy-runtime-fetch] ${errorMessage(error)}`);
    process.exitCode = 1;
  }
}
