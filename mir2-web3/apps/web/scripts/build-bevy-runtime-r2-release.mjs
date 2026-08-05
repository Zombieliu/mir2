#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { constants as zlibConstants, gzipSync } from "node:zlib";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const repoRoot = path.resolve(webRoot, "..", "..");
const args = parseArgs(process.argv.slice(2));
const runtimeManifestPath = path.join(webRoot, "lib", "generated", "bevy_runtime_version.json");
const runtimeManifest = JSON.parse(await fs.readFile(runtimeManifestPath, "utf8"));
const runtimeVersion = String(runtimeManifest.version ?? "").trim();
const assetVersion = normalizeVersion(args.assetVersion ?? process.env.MIR2_ASSET_VERSION ?? "");
const objectPrefix = normalizePrefix(args.objectPrefix ?? process.env.MIR2_ASSET_OBJECT_PREFIX ?? "");
const assetBaseUrl = String(
  args.assetBaseUrl ?? process.env.MIR2_ASSET_BASE_URL ?? process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL ?? "",
).replace(/\/+$/, "");
const outputPath = path.resolve(
  args.output ??
    path.join(repoRoot, "docs", "generated", "remote-assets", `bevy-runtime-${runtimeVersion}.json`),
);

if (!/^bevy-[a-f0-9]{16}$/i.test(runtimeVersion)) {
  throw new Error(`Invalid runtime version in ${runtimeManifestPath}`);
}
if (!assetVersion || !objectPrefix) {
  throw new Error("Set MIR2_ASSET_VERSION and MIR2_ASSET_OBJECT_PREFIX, or pass --assetVersion and --objectPrefix.");
}

const gzipOptions = {
  level: zlibConstants.Z_BEST_COMPRESSION,
  mtime: 0,
};
const files = [];
for (const entry of runtimeManifest.files ?? []) {
  const manifestPath = String(entry.path ?? "");
  const relativePath = manifestPath.replace(/^public\//, "");
  if (!/^bevy-runtime\/pkg-(?:webgpu|webgl2)\/mir2_bevy_runtime(?:_bg\.wasm|\.js)$/.test(relativePath)) {
    throw new Error(`Unexpected runtime path: ${manifestPath}`);
  }
  const stagePath = path.join(webRoot, ...manifestPath.split("/"));
  const bytes = await fs.readFile(stagePath);
  const sha256 = hash(bytes);
  if (sha256 !== entry.sha256) {
    throw new Error(`${manifestPath} differs from the runtime manifest`);
  }
  const wasm = relativePath.endsWith(".wasm");
  const encoded = wasm ? gzipSync(bytes, gzipOptions) : null;
  files.push({
    path: `/${relativePath}`,
    relativePath,
    stagePath,
    objectKey: `${objectPrefix}/${relativePath}`,
    size: bytes.byteLength,
    sha256,
    contentType: wasm ? "application/wasm" : "text/javascript; charset=utf-8",
    cacheControl: "public, max-age=31536000, immutable",
    sources: ["bevy-runtime"],
    ...(encoded
      ? {
          contentEncoding: "gzip",
          encodedSize: encoded.byteLength,
          encodedSha256: hash(encoded),
        }
      : {}),
  });
}

if (files.length !== 4) {
  throw new Error(`Expected four runtime files, found ${files.length}`);
}

const release = {
  schemaVersion: 1,
  kind: "mir2-bevy-runtime-r2-release",
  version: assetVersion,
  generatedAt: new Date().toISOString(),
  assetBaseUrl: assetBaseUrl || null,
  objectPrefix,
  cacheControl: "public, max-age=31536000, immutable",
  bevyRuntime: {
    enabled: true,
    version: runtimeVersion,
    contentEncoding: "gzip",
    logicalBytes: files.reduce((sum, file) => sum + file.size, 0),
    storageBytes: files.reduce((sum, file) => sum + (file.encodedSize ?? file.size), 0),
  },
  files,
};

await fs.mkdir(path.dirname(outputPath), { recursive: true });
await fs.writeFile(outputPath, `${JSON.stringify(release, null, 2)}\n`, "utf8");
console.log(JSON.stringify({
  ok: true,
  outputPath,
  assetVersion,
  objectPrefix,
  runtimeVersion,
  fileCount: files.length,
  logicalBytes: release.bevyRuntime.logicalBytes,
  storageBytes: release.bevyRuntime.storageBytes,
}, null, 2));

function hash(bytes) {
  return crypto.createHash("sha256").update(bytes).digest("hex");
}

function normalizeVersion(value) {
  return String(value).trim().replace(/[^a-zA-Z0-9._-]/g, "-").replace(/-+/g, "-").replace(/^-+|-+$/g, "").slice(0, 80);
}

function normalizePrefix(value) {
  return String(value).trim().replace(/^\/+|\/+$/g, "");
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
