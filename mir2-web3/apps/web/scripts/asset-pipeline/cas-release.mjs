import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";

export const CAS_RELEASE_SCHEMA_VERSION = 1;
export const CAS_CHANNEL_SCHEMA_VERSION = 1;
export const CAS_DESCRIPTOR_SCHEMA_VERSION = 1;
export const IMMUTABLE_CACHE_CONTROL = "public, max-age=31536000, immutable";
export const CHANNEL_CACHE_CONTROL = "no-cache, max-age=0, must-revalidate";

export function createCasRelease(files, options = {}) {
  const prefix = normalizePrefix(options.prefix ?? "mir2/cas");
  const channelName = normalizeChannelName(options.channel ?? "production");
  const normalizedFiles = files.map(normalizeInputFile).sort((left, right) =>
    compareCodePoints(left.path, right.path),
  );
  assertUnique(normalizedFiles.map((file) => file.path), "release file path");

  const manifestBody = {
    schemaVersion: CAS_RELEASE_SCHEMA_VERSION,
    kind: "mir2-cas-asset-release",
    hashAlgorithm: "sha256",
    files: normalizedFiles.map((file) => ({
      path: file.path,
      sha256: file.sha256,
      size: file.size,
      contentType: file.contentType,
      objectKey: blobObjectKey(prefix, file.sha256),
    })),
  };
  const manifest = manifestBody;
  const manifestJson = canonicalJson(manifest);
  const manifestHash = sha256Text(manifestJson);
  const manifestObjectKey = `${prefix}/releases/sha256/${manifestHash}.json`;
  const channel = {
    schemaVersion: CAS_CHANNEL_SCHEMA_VERSION,
    kind: "mir2-cas-release-channel",
    channel: channelName,
    release: {
      sha256: manifestHash,
      objectKey: manifestObjectKey,
    },
  };

  validateCasManifest(manifest, { prefix, expectedHash: manifestHash });
  validateCasChannel(channel, { prefix, manifestHash });
  return {
    manifest,
    channel,
    manifestJson,
    channelJson: `${canonicalJson(channel)}\n`,
    descriptor: {
      schemaVersion: CAS_DESCRIPTOR_SCHEMA_VERSION,
      prefix,
      hashAlgorithm: "sha256",
      manifest: {
        sha256: manifestHash,
        objectKey: manifestObjectKey,
      },
      channel: {
        name: channelName,
        objectKey: `${prefix}/channels/${channelName}.json`,
      },
    },
  };
}

export async function writeCasReleaseArtifacts(plan, outputDir) {
  const artifactDir = path.join(outputDir, "cas");
  const manifestPath = path.join(artifactDir, `release-${plan.descriptor.manifest.sha256}.json`);
  const channelPath = path.join(artifactDir, `channel-${plan.channel.channel}.json`);
  await fs.mkdir(artifactDir, { recursive: true });
  await fs.writeFile(manifestPath, plan.manifestJson, "utf8");
  await fs.writeFile(channelPath, plan.channelJson, "utf8");
  return {
    ...plan.descriptor,
    manifest: {
      ...plan.descriptor.manifest,
      stagePath: manifestPath,
      size: Buffer.byteLength(plan.manifestJson),
    },
    channel: {
      ...plan.descriptor.channel,
      stagePath: channelPath,
      size: Buffer.byteLength(plan.channelJson),
    },
  };
}

export async function loadCasUploadPlan(release, options = {}) {
  const descriptor = validateDescriptor(release?.cas);
  if (!Array.isArray(release?.files)) throw new Error("release.files must be an array");

  const [manifestText, channelText] = await Promise.all([
    fs.readFile(descriptor.manifest.stagePath, "utf8"),
    fs.readFile(descriptor.channel.stagePath, "utf8"),
  ]);
  const manifest = JSON.parse(manifestText);
  const channel = JSON.parse(channelText);
  if (Buffer.byteLength(manifestText) !== descriptor.manifest.size) throw new Error("CAS manifest artifact size mismatch");
  if (Buffer.byteLength(channelText) !== descriptor.channel.size) throw new Error("CAS channel artifact size mismatch");
  if (manifestText !== canonicalJson(manifest)) throw new Error("CAS manifest artifact is not canonical JSON");
  const manifestHash = validateCasManifest(manifest, {
    prefix: descriptor.prefix,
    expectedHash: descriptor.manifest.sha256,
  });
  validateCasChannel(channel, { prefix: descriptor.prefix, manifestHash });

  if (manifestHash !== descriptor.manifest.sha256) {
    throw new Error("CAS descriptor manifest hash does not match the immutable manifest");
  }
  if (descriptor.manifest.objectKey !== `${descriptor.prefix}/releases/sha256/${manifestHash}.json`) {
    throw new Error("CAS descriptor manifest object key is not content-addressed");
  }
  if (channel.channel !== descriptor.channel.name || channel.release.objectKey !== descriptor.manifest.objectKey) {
    throw new Error("CAS channel does not point at the descriptor manifest");
  }
  if (descriptor.channel.objectKey !== `${descriptor.prefix}/channels/${descriptor.channel.name}.json`) {
    throw new Error("CAS descriptor channel object key does not match its channel name");
  }

  const sourceByPath = new Map();
  for (const file of release.files) {
    const relativePath = normalizeRelativePath(file.relativePath ?? file.p ?? file.path);
    if (sourceByPath.has(relativePath)) throw new Error(`Duplicate release file path: ${relativePath}`);
    sourceByPath.set(relativePath, file);
  }

  const assetsByObjectKey = new Map();
  for (const entry of manifest.files) {
    const source = sourceByPath.get(entry.path);
    if (!source) throw new Error(`CAS manifest file is absent from release.files: ${entry.path}`);
    const sourceHash = source.sha256 ?? source.h;
    const sourceSize = source.size ?? source.s;
    if (sourceHash !== entry.sha256 || sourceSize !== entry.size) {
      throw new Error(`CAS manifest metadata mismatch for ${entry.path}`);
    }
    const upload = {
      path: `/${entry.path}`,
      relativePath: entry.path,
      stagePath: source.stagePath ?? source.localPath ?? options.resolveStagePath?.(entry.path),
      objectKey: entry.objectKey,
      size: entry.size,
      contentType: entry.contentType,
      cacheControl: IMMUTABLE_CACHE_CONTROL,
      sources: ["cas-asset"],
    };
    if (!upload.stagePath) throw new Error(`No staged file is available for CAS asset: ${entry.path}`);
    const existing = assetsByObjectKey.get(entry.objectKey);
    if (existing && (existing.size !== upload.size || existing.contentType !== upload.contentType)) {
      throw new Error(`Conflicting metadata for CAS blob: ${entry.objectKey}`);
    }
    if (!existing) assetsByObjectKey.set(entry.objectKey, upload);
  }
  const assets = [...assetsByObjectKey.values()];
  await mapWithConcurrency(assets, options.verifyConcurrency ?? 8, verifyStagedAsset);

  return {
    assets,
    manifest: {
      path: "/cas-release-manifest.json",
      relativePath: path.basename(descriptor.manifest.stagePath),
      stagePath: descriptor.manifest.stagePath,
      objectKey: descriptor.manifest.objectKey,
      size: descriptor.manifest.size,
      contentType: "application/json; charset=utf-8",
      cacheControl: IMMUTABLE_CACHE_CONTROL,
      sources: ["cas-release-manifest"],
    },
    channel: {
      path: "/cas-release-channel.json",
      relativePath: path.basename(descriptor.channel.stagePath),
      stagePath: descriptor.channel.stagePath,
      objectKey: descriptor.channel.objectKey,
      size: descriptor.channel.size,
      contentType: "application/json; charset=utf-8",
      cacheControl: CHANNEL_CACHE_CONTROL,
      sources: ["cas-release-channel"],
    },
  };
}

export function validateCasManifest(manifest, options = {}) {
  assertRecord(manifest, "CAS manifest");
  if (manifest.schemaVersion !== CAS_RELEASE_SCHEMA_VERSION) throw new Error("Unsupported CAS manifest schemaVersion");
  if (manifest.kind !== "mir2-cas-asset-release" || manifest.hashAlgorithm !== "sha256") {
    throw new Error("Invalid CAS manifest identity");
  }
  if (!Array.isArray(manifest.files)) throw new Error("CAS manifest files must be an array");
  const prefix = normalizePrefix(options.prefix ?? prefixFromObjectKey(manifest.files[0]?.objectKey));
  let previous = null;
  for (const file of manifest.files) {
    const normalized = normalizeInputFile(file);
    if (previous !== null && compareCodePoints(previous, normalized.path) >= 0) {
      throw new Error("CAS manifest files must be unique and sorted by path");
    }
    if (file.objectKey !== blobObjectKey(prefix, normalized.sha256)) {
      throw new Error(`CAS blob object key does not match content hash: ${normalized.path}`);
    }
    previous = normalized.path;
  }
  const actualHash = hashCanonical(manifest);
  if (options.expectedHash && actualHash !== options.expectedHash) {
    throw new Error(`CAS manifest contentHash mismatch: expected ${options.expectedHash}, received ${actualHash}`);
  }
  return actualHash;
}

export function validateCasChannel(channel, { prefix, manifestHash } = {}) {
  assertRecord(channel, "CAS channel");
  if (channel.schemaVersion !== CAS_CHANNEL_SCHEMA_VERSION || channel.kind !== "mir2-cas-release-channel") {
    throw new Error("Invalid CAS channel identity");
  }
  normalizeChannelName(channel.channel);
  assertRecord(channel.release, "CAS channel release");
  assertHash(channel.release.sha256, "CAS channel release sha256");
  const cleanPrefix = normalizePrefix(prefix ?? prefixFromObjectKey(channel.release.objectKey));
  const expectedKey = `${cleanPrefix}/releases/sha256/${channel.release.sha256}.json`;
  if (channel.release.objectKey !== expectedKey) throw new Error("CAS channel release object key is not content-addressed");
  if (manifestHash && channel.release.sha256 !== manifestHash) {
    throw new Error("CAS channel release hash does not match manifest");
  }
  return true;
}

export function canonicalJson(value) {
  return JSON.stringify(sortJson(value));
}

function validateDescriptor(descriptor) {
  assertRecord(descriptor, "release.cas");
  if (descriptor.schemaVersion !== CAS_DESCRIPTOR_SCHEMA_VERSION || descriptor.hashAlgorithm !== "sha256") {
    throw new Error("Invalid CAS release descriptor");
  }
  descriptor.prefix = normalizePrefix(descriptor.prefix);
  assertRecord(descriptor.manifest, "release.cas.manifest");
  assertRecord(descriptor.channel, "release.cas.channel");
  assertHash(descriptor.manifest.sha256, "release.cas.manifest.sha256");
  normalizeChannelName(descriptor.channel.name);
  for (const [label, artifact] of [["manifest", descriptor.manifest], ["channel", descriptor.channel]]) {
    if (!artifact.stagePath || !Number.isSafeInteger(artifact.size) || artifact.size < 1) {
      throw new Error(`Invalid CAS ${label} artifact metadata`);
    }
  }
  return descriptor;
}

function normalizeInputFile(file) {
  assertRecord(file, "CAS release file");
  const filePath = normalizeRelativePath(file.relativePath ?? file.p ?? file.path);
  const sha256 = file.sha256 ?? file.h;
  const size = file.size ?? file.s;
  const contentType = file.contentType ?? file.c ?? "application/octet-stream";
  assertHash(sha256, `${filePath}.sha256`);
  if (!Number.isSafeInteger(size) || size < 0) throw new Error(`${filePath}.size must be a non-negative integer`);
  if (typeof contentType !== "string" || !contentType.trim()) throw new Error(`${filePath}.contentType must be non-empty`);
  return { path: filePath, sha256, size, contentType };
}

function normalizeRelativePath(value) {
  const normalized = String(value ?? "").replace(/^\/+/, "");
  if (!normalized || normalized.includes("\\") || normalized.split("/").some((part) => !part || part === "." || part === "..")) {
    throw new Error(`Invalid release-relative path: ${value}`);
  }
  return normalized;
}

function normalizePrefix(value) {
  const prefix = String(value ?? "").trim().replace(/^\/+|\/+$/g, "");
  if (!prefix || prefix.includes("\\") || prefix.split("/").some((part) => !part || part === "." || part === "..")) {
    throw new Error(`Invalid CAS prefix: ${value}`);
  }
  return prefix;
}

function normalizeChannelName(value) {
  const channel = String(value ?? "").trim().toLowerCase();
  if (!/^[a-z0-9][a-z0-9._-]{0,63}$/.test(channel)) throw new Error(`Invalid CAS channel name: ${value}`);
  return channel;
}

function blobObjectKey(prefix, hash) {
  return `${prefix}/blobs/sha256/${hash.slice(0, 2)}/${hash}`;
}

function prefixFromObjectKey(objectKey) {
  const value = String(objectKey ?? "");
  for (const marker of ["/releases/sha256/", "/blobs/sha256/"]) {
    if (value.includes(marker)) return value.split(marker)[0];
  }
  return "";
}

function hashCanonical(value) {
  return sha256Text(canonicalJson(value));
}

function sha256Text(value) {
  return createHash("sha256").update(value).digest("hex");
}

async function verifyStagedAsset(asset) {
  const stats = await fs.stat(asset.stagePath);
  if (!stats.isFile() || stats.size !== asset.size) throw new Error(`CAS staged asset size mismatch: ${asset.relativePath}`);
  const actualHash = await sha256File(asset.stagePath);
  if (!asset.objectKey.endsWith(`/${actualHash}`)) {
    throw new Error(`CAS staged asset hash mismatch: ${asset.relativePath}`);
  }
}

function sha256File(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(filePath);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
  });
}

async function mapWithConcurrency(items, limit, worker) {
  let index = 0;
  async function next() {
    while (index < items.length) {
      const item = items[index];
      index += 1;
      await worker(item);
    }
  }
  const count = Math.min(Math.max(1, Number(limit) || 1), items.length);
  await Promise.all(Array.from({ length: count }, next));
}

function sortJson(value) {
  if (Array.isArray(value)) return value.map(sortJson);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.keys(value).sort(compareCodePoints).map((key) => [key, sortJson(value[key])]));
  }
  return value;
}

function compareCodePoints(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function assertUnique(values, label) {
  if (new Set(values).size !== values.length) throw new Error(`Duplicate ${label}`);
}

function assertHash(value, label) {
  if (typeof value !== "string" || !/^[a-f0-9]{64}$/.test(value)) throw new Error(`${label} must be a lowercase sha256 hash`);
}

function assertRecord(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} must be an object`);
}
