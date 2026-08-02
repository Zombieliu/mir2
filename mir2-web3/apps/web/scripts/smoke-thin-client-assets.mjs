#!/usr/bin/env node

import { access } from "node:fs/promises";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));
const webRoot = path.resolve(import.meta.dirname, "..");
const packageRoot = path.resolve(webRoot, args.package ?? ".mir2-thin-client");
const assetBaseUrl = String(
  args.assetBaseUrl ??
    process.env.MIR2_ASSET_BASE_URL ??
    process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ??
    "",
).replace(/\/+$/, "");

if (!assetBaseUrl) {
  throw new Error(
    "Set --assetBaseUrl or MIR2_ASSET_BASE_URL to the immutable R2 release root.",
  );
}

const localRequired = [
  "server.js",
  "public/mir2-asset-worker.js",
  "public/generated/map-atlas/manifest.json",
  "public/bevy-entity-atlases/manifest.json",
  "public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime.js",
  "public/bevy-runtime/pkg-webgpu/mir2_bevy_runtime_bg.wasm",
  "public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js",
  "public/bevy-runtime/pkg-webgl2/mir2_bevy_runtime_bg.wasm",
  "public/original-ui/Prguse/2092.png",
  "public/original-ui/Prguse/2094.png",
  "public/original-ui/Prguse/2095.png",
  "public/debug/map-samples/smtile-72.png",
  "public/debug/map-samples/smtile-80.png",
];

const remoteCritical = [
  "/original-ui/ChrSel/0.png",
  "/original-ui/ChrSel/20.png",
  "/original-ui/Prguse/65.png",
  "/original-ui/Prguse/1084.png",
  "/original-ui/Prguse/2090.png",
  "/original-ui/Prguse/2221.png",
  "/original-ui/Title/30.png",
  "/original-ui/Title/31.png",
  "/original-ui/Title/40.png",
  "/original-ui/Title/320.png",
  "/original-ui/MMap/0.png",
  "/original-ui/Sound/Login2.wav",
];

const local = await Promise.all(
  localRequired.map(async (relativePath) => {
    try {
      await access(path.join(packageRoot, relativePath));
      return { path: relativePath, ok: true };
    } catch {
      return { path: relativePath, ok: false };
    }
  }),
);

const remote = await Promise.all(
  remoteCritical.map(async (assetPath) => {
    const url = `${assetBaseUrl}${assetPath}`;
    try {
      const response = await fetch(url, { method: "HEAD", redirect: "follow" });
      return {
        path: assetPath,
        ok: response.ok,
        status: response.status,
        contentLength: Number(response.headers.get("content-length") ?? 0),
        cacheControl: response.headers.get("cache-control"),
        edgeCache: response.headers.get("x-mir2-edge-cache") ?? response.headers.get("cf-cache-status"),
      };
    } catch (error) {
      return {
        path: assetPath,
        ok: false,
        status: 0,
        error: error instanceof Error ? error.message : String(error),
      };
    }
  }),
);

let releaseManifest = null;
try {
  const response = await fetch(`${assetBaseUrl}/remote-asset-release.json`, { cache: "no-store" });
  if (response.ok) {
    const parsed = JSON.parse(await response.text());
    releaseManifest = {
      ok: true,
      version: parsed.version ?? null,
      generatedAt: parsed.generatedAt ?? null,
      fileCount: Number(parsed.stats?.fileCount ?? parsed.files?.length ?? 0),
      advisory:
        Number(parsed.stats?.fileCount ?? parsed.files?.length ?? 0) < 10_000
          ? "Release manifest is stale or partial; repair it before using it as the authoritative R2 inventory."
          : null,
    };
  } else {
    releaseManifest = { ok: false, status: response.status };
  }
} catch (error) {
  releaseManifest = {
    ok: false,
    error: error instanceof Error ? error.message : String(error),
  };
}

const report = {
  ok: local.every((entry) => entry.ok) && remote.every((entry) => entry.ok),
  generatedAt: new Date().toISOString(),
  packageRoot,
  assetBaseUrl,
  local,
  remote,
  releaseManifest,
};

console.log(JSON.stringify(report, null, 2));
if (!report.ok) process.exitCode = 1;

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const flag = values[index];
    if (!flag.startsWith("--")) throw new Error(`Unknown argument: ${flag}`);
    const value = values[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${flag} requires a value`);
    parsed[flag.slice(2)] = value;
    index += 1;
  }
  return parsed;
}
