import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { createReadStream } from "node:fs";
import path from "node:path";
import { Writable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { constants as zlibConstants, createGzip } from "node:zlib";

import { loadCasUploadPlan } from "./asset-pipeline/cas-release.mjs";
import { normalizeWorkerUploadPath } from "./asset-pipeline/upload-safety.mjs";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_MANIFEST = path.resolve(
  REPO_ROOT,
  "docs",
  "generated",
  "remote-assets",
  "latest-remote-asset-release.json",
);
const FULL_PACK_GZIP_OPTIONS = Object.freeze({
  level: zlibConstants.Z_BEST_COMPRESSION,
  mtime: 0,
});

const args = parseArgs(process.argv.slice(2));
const manifestPath = path.resolve(args.manifest ?? process.env.MIR2_REMOTE_ASSET_RELEASE_MANIFEST ?? DEFAULT_MANIFEST);
const dryRun = booleanArg(args.dryRun ?? process.env.MIR2_R2_DRY_RUN, false);
const resumeExistingAssets = booleanArg(
  args.resumeExistingAssets ?? process.env.MIR2_R2_RESUME_EXISTING_ASSETS,
  false,
);
const bucket = args.bucket ?? process.env.MIR2_R2_BUCKET ?? "";
const ensureBucket = booleanArg(args.ensureBucket ?? process.env.MIR2_R2_ENSURE_BUCKET, false);
const concurrency = numberArg(args.concurrency ?? process.env.MIR2_R2_UPLOAD_CONCURRENCY, 4);
const includeReleaseManifestOverride =
  args.includeReleaseManifest ?? process.env.MIR2_R2_UPLOAD_RELEASE_MANIFEST;
const remote = booleanArg(args.remote ?? process.env.MIR2_R2_REMOTE, true);
const maxAttempts = numberArg(args.maxAttempts ?? process.env.MIR2_R2_UPLOAD_ATTEMPTS, 3);
const progressEvery = numberArg(args.progressEvery ?? process.env.MIR2_R2_UPLOAD_PROGRESS_EVERY, 25);
const verifyOriginalAssets = booleanArg(
  args.verifyOriginalAssets ?? process.env.MIR2_R2_VERIFY_ORIGINAL_ASSETS,
  false,
);
const verifyOriginalAssetConcurrency = numberArg(
  args.verifyOriginalAssetConcurrency ?? process.env.MIR2_R2_VERIFY_ORIGINAL_ASSET_CONCURRENCY,
  16,
);
const verifyOriginalAssetTimeoutMs = numberArg(
  args.verifyOriginalAssetTimeoutMs ?? process.env.MIR2_R2_VERIFY_ORIGINAL_ASSET_TIMEOUT_MS,
  15_000,
);
const verifyOriginalAssetAttempts = numberArg(
  args.verifyOriginalAssetAttempts ?? process.env.MIR2_R2_VERIFY_ORIGINAL_ASSET_ATTEMPTS,
  Math.max(maxAttempts, 8),
);
const uploadDriverInput = String(args.driver ?? process.env.MIR2_R2_UPLOAD_DRIVER ?? "r2-s3").toLowerCase();
const uploadDriver = normalizeUploadDriver(uploadDriverInput);
const cloudflareAccountId = args.accountId ?? process.env.CLOUDFLARE_ACCOUNT_ID ?? "";
const cloudflareApiToken = process.env.CLOUDFLARE_API_TOKEN ?? "";
const cloudflareApiBaseUrl = normalizeOptionalUrl(
  args.apiBaseUrl ??
    process.env.MIR2_CLOUDFLARE_API_BASE_URL ??
    "https://api.cloudflare.com/client/v4",
);
const workerUploadUrl = normalizeOptionalUrl(args.workerUrl ?? process.env.MIR2_R2_UPLOAD_WORKER_URL ?? "");
const workerUploadPath = normalizeWorkerUploadPath(
  args.workerPath ?? process.env.MIR2_R2_UPLOAD_WORKER_PATH ?? "/upload",
);
const workerUploadSecret = process.env.MIR2_R2_UPLOAD_SECRET ?? "";
const s3Endpoint = normalizeOptionalUrl(
  args.s3Endpoint ??
    process.env.MIR2_R2_S3_ENDPOINT ??
    "",
);
const s3AccessKeyId = String(
  args.s3AccessKeyId ?? process.env.MIR2_R2_ACCESS_KEY_ID ?? "",
).trim();
const s3SecretAccessKey = String(
  args.s3SecretAccessKey ??
    process.env.MIR2_R2_SECRET_ACCESS_KEY ??
    "",
).trim();
const s3SessionToken = String(
  args.s3SessionToken ??
    process.env.MIR2_R2_SESSION_TOKEN ??
    "",
).trim();
const wranglerBin = process.platform === "win32" ? "npx.cmd" : "npx";
let s3Client;

async function main() {
  const release = JSON.parse(await fs.readFile(manifestPath, "utf8"));
  const includeReleaseManifest = booleanArg(
    includeReleaseManifestOverride,
    release.publishReleaseManifest !== false,
  );
  if (release.fullCrystalPack?.enabled === true && !["api", "s3", "worker"].includes(uploadDriver)) {
    throw new Error(
      "Verified full Crystal packs must use MIR2_R2_UPLOAD_DRIVER=api, worker, or r2-s3 " +
        "so large shards are streamed directly to R2.",
    );
  }
  if (resumeExistingAssets && !verifyOriginalAssets) {
    throw new Error("Resuming an existing R2 asset prefix requires MIR2_R2_VERIFY_ORIGINAL_ASSETS=1.");
  }
  const legacyAssetUploads = await buildUploadList(release);
  await verifyEncodedUploads(legacyAssetUploads);
  const encodedUploads = legacyAssetUploads.filter((upload) => upload.contentEncoding);
  if (encodedUploads.length > 0 && !["api", "s3", "worker"].includes(uploadDriver)) {
    throw new Error("Content-encoded release assets require MIR2_R2_UPLOAD_DRIVER=api, worker, or r2-s3.");
  }
  if (encodedUploads.length > 0 && release.cas) {
    throw new Error("Content-encoded releases cannot also publish raw CAS assets.");
  }
  const legacyUploadByPath = new Map(legacyAssetUploads.map((upload) => [upload.relativePath, upload]));
  const casUploads = release.cas
    ? await loadCasUploadPlan(release, {
        resolveStagePath: (relativePath) => legacyUploadByPath.get(relativePath)?.stagePath,
      })
    : null;
  const assetUploads = casUploads ? [...legacyAssetUploads, ...casUploads.assets] : legacyAssetUploads;
  const immutableManifestUploads = casUploads ? [casUploads.manifest] : [];
  let releaseManifestUpload = null;

  if (includeReleaseManifest) {
    releaseManifestUpload = {
      path: "/remote-asset-release.json",
      relativePath: "remote-asset-release.json",
      stagePath: manifestPath,
      objectKey: joinObjectKey(release.objectPrefix, "remote-asset-release.json"),
      size: (await fs.stat(manifestPath)).size,
      contentType: "application/json; charset=utf-8",
      cacheControl: "public, max-age=60, stale-while-revalidate=300",
      sources: ["release-manifest"],
    };
  }
  const uploads = [
    ...(resumeExistingAssets ? [] : assetUploads),
    ...immutableManifestUploads,
    ...(releaseManifestUpload ? [releaseManifestUpload] : []),
    ...(casUploads ? [casUploads.channel] : []),
  ];

  const totalBytes = uploads.reduce((sum, upload) => sum + upload.size, 0);
  const logicalTotalBytes = uploads.reduce((sum, upload) => sum + (upload.logicalSize ?? upload.size), 0);
  if (dryRun) {
    console.log(
      JSON.stringify(
        {
          ok: true,
          dryRun: true,
          manifestPath,
          bucket: bucket || null,
          driver: uploadDriverInput,
          objectPrefix: release.objectPrefix,
          assetBaseUrl: release.assetBaseUrl ?? null,
          uploadCount: uploads.length,
          totalBytes,
          logicalTotalBytes,
          encodedUploadCount: encodedUploads.length,
          storageSavingsBytes: logicalTotalBytes - totalBytes,
          verifyOriginalAssets,
          resumeExistingAssets,
          publishOrder: {
            assets: assetUploads.length,
            immutableManifests: immutableManifestUploads.length,
            legacyReleaseManifest: releaseManifestUpload ? 1 : 0,
            mutableChannelLast: casUploads?.channel.objectKey ?? null,
          },
          sample: uploads.slice(0, 8).map((upload) => ({
            objectKey: upload.objectKey,
            size: upload.size,
            logicalSize: upload.logicalSize ?? upload.size,
            contentType: upload.contentType,
            contentEncoding: upload.contentEncoding ?? null,
          })),
        },
        null,
        2,
      ),
    );
    return;
  }

  if (!bucket) {
    throw new Error("Set MIR2_R2_BUCKET or pass --bucket before uploading to R2.");
  }

  if (!["wrangler", "api", "worker", "s3", "r2-s3"].includes(uploadDriverInput)) {
    throw new Error(
      `Unsupported MIR2_R2_UPLOAD_DRIVER: ${uploadDriverInput}. Expected "wrangler", "api", "worker", or "r2-s3".`,
    );
  }

  if (uploadDriver === "wrangler") {
    await runWrangler(["wrangler", "whoami"], { label: "wrangler whoami" });
    if (ensureBucket) {
      await runWrangler(["wrangler", "r2", "bucket", "create", bucket], {
        label: `ensure bucket ${bucket}`,
        allowFailure: true,
      });
    }
  }

  if (uploadDriver === "api" && (!cloudflareAccountId || !cloudflareApiToken)) {
    throw new Error(
      "Set CLOUDFLARE_ACCOUNT_ID and CLOUDFLARE_API_TOKEN before using MIR2_R2_UPLOAD_DRIVER=api.",
    );
  }

  if (uploadDriver === "worker" && (!workerUploadUrl || !workerUploadSecret)) {
    throw new Error("Set MIR2_R2_UPLOAD_WORKER_URL and MIR2_R2_UPLOAD_SECRET before using MIR2_R2_UPLOAD_DRIVER=worker.");
  }

  if (uploadDriver === "s3" && !s3AccessKeyId) {
    throw new Error(
      "Set MIR2_R2_ACCESS_KEY_ID and MIR2_R2_SECRET_ACCESS_KEY before using MIR2_R2_UPLOAD_DRIVER=r2-s3. " +
        "These are Cloudflare R2 S3 API credentials; CLOUDFLARE_API_TOKEN is only used for Cloudflare worker/R2 control-path operations and is not used for r2-s3 uploads.",
    );
  }

  if (uploadDriver === "s3" && !s3SecretAccessKey) {
    throw new Error(
      "Set MIR2_R2_ACCESS_KEY_ID and MIR2_R2_SECRET_ACCESS_KEY before using MIR2_R2_UPLOAD_DRIVER=r2-s3. " +
        "These are Cloudflare R2 S3 API credentials; CLOUDFLARE_API_TOKEN is only used for Cloudflare worker/R2 control-path operations and is not used for r2-s3 uploads.",
    );
  }

  if (uploadDriver === "s3") {
    resolveS3Endpoint();
  }

  let completed = 0;
  const uploadOne = async (upload) => {
    await uploadWithRetry(upload);
    completed += 1;
    if (completed % progressEvery === 0 || completed === uploads.length) {
      console.log(`[mir2-r2] uploaded ${completed}/${uploads.length}`);
    }
  };
  if (resumeExistingAssets) {
    console.log(`[mir2-r2] resuming existing asset prefix; verifying ${assetUploads.length} assets before publishing manifests`);
  } else {
    await runPool(assetUploads, concurrency, uploadOne);
  }

  if (verifyOriginalAssets) {
    await verifyUploadedOriginalAssets(release, legacyAssetUploads);
  }

  for (const immutableManifestUpload of immutableManifestUploads) {
    await uploadOne(immutableManifestUpload);
  }

  if (releaseManifestUpload) {
    await uploadOne(releaseManifestUpload);
  }

  // The channel is the only mutable CAS object and must become visible last.
  if (casUploads) {
    await uploadOne(casUploads.channel);
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        dryRun: false,
        manifestPath,
        bucket,
        driver: uploadDriverInput,
        objectPrefix: release.objectPrefix,
        assetBaseUrl: release.assetBaseUrl ?? null,
        uploadCount: uploads.length,
        totalBytes,
        logicalTotalBytes,
        encodedUploadCount: encodedUploads.length,
        storageSavingsBytes: logicalTotalBytes - totalBytes,
        verifiedOriginalAssetCount: verifyOriginalAssets
          ? legacyAssetUploads.filter((upload) => upload.sources?.includes("original-asset-manifest")).length
          : 0,
        casManifestObjectKey: casUploads?.manifest.objectKey ?? null,
        channelObjectKey: casUploads?.channel.objectKey ?? null,
      },
      null,
      2,
    ),
  );
}

async function verifyUploadedOriginalAssets(release, uploads) {
  const assetBaseUrl = normalizeOptionalUrl(release.assetBaseUrl ?? "");
  if (!assetBaseUrl) {
    throw new Error("MIR2_R2_VERIFY_ORIGINAL_ASSETS=1 requires release.assetBaseUrl.");
  }

  const originalAssetUploads = uploads.filter((upload) =>
    Array.isArray(upload.sources) && upload.sources.includes("original-asset-manifest"),
  );
  if (!originalAssetUploads.length) {
    throw new Error("No release files are tagged with original-asset-manifest; cannot verify published originals.");
  }

  let completed = 0;
  await runPool(originalAssetUploads, verifyOriginalAssetConcurrency, async (upload) => {
    const url = `${assetBaseUrl}/${String(upload.relativePath || upload.path || "").replace(/^\/+/, "")}`;
    await verifyUploadedOriginalAsset(upload, url);
    completed += 1;
    if (completed % 500 === 0 || completed === originalAssetUploads.length) {
      console.log(`[mir2-r2] verified original assets ${completed}/${originalAssetUploads.length}`);
    }
  });
}

async function verifyUploadedOriginalAsset(upload, url) {
  let lastError;
  for (let attempt = 1; attempt <= verifyOriginalAssetAttempts; attempt += 1) {
    try {
      const response = await fetch(url, {
        method: "HEAD",
        signal: AbortSignal.timeout(verifyOriginalAssetTimeoutMs),
      });
      if (!response.ok) {
        throw createUploadHttpError(
          `Original asset missing after R2 upload: HTTP ${response.status} ${url} objectKey=${upload.objectKey}`,
          response,
        );
      }
      return;
    } catch (error) {
      lastError = error;
      if (attempt >= verifyOriginalAssetAttempts || !isRetryableUploadError(error)) break;
      const delayMs = uploadRetryDelayMs(error, attempt);
      console.warn(
        `[mir2-r2] retry original asset verification ${attempt + 1}/${verifyOriginalAssetAttempts} in ${delayMs}ms ${url}`,
      );
      await sleep(delayMs);
    }
  }
  throw lastError;
}

async function buildUploadList(release) {
  if (!Array.isArray(release.files)) {
    throw new Error(`Release manifest ${manifestPath} has no files array.`);
  }

  const uploads = [];
  for (const file of release.files) {
    const relativePath = normalizeReleaseFileRelativePath(file);
    const stagePath = file.stagePath ?? path.join(WEB_ROOT, "public", relativePath);
    const objectKey = file.objectKey ?? joinObjectKey(release.objectPrefix, relativePath);
    if (!stagePath || !objectKey) {
      throw new Error(`Invalid release file entry: ${JSON.stringify(file)}`);
    }
    const stats = await fs.stat(stagePath);
    if (!stats.isFile()) throw new Error(`Not a file: ${stagePath}`);
    const expectedSize = numberOrNull(file.size ?? file.s);
    if (expectedSize !== null && expectedSize !== stats.size) {
      throw new Error(`Release source size mismatch for ${relativePath}: expected ${expectedSize}, found ${stats.size}`);
    }
    const contentEncoding = normalizeContentEncoding(file.contentEncoding ?? file.e);
    const encodedSize = numberOrNull(file.encodedSize ?? file.es);
    const encodedSha256 = normalizeSha256(file.encodedSha256 ?? file.eh);
    if (contentEncoding && (!encodedSize || !encodedSha256)) {
      throw new Error(`Encoded release file is missing encodedSize or encodedSha256: ${relativePath}`);
    }
    uploads.push({
      path: file.path ?? `/${relativePath}`,
      relativePath,
      stagePath,
      objectKey,
      size: contentEncoding ? encodedSize : stats.size,
      logicalSize: stats.size,
      contentType: file.contentType ?? file.c ?? "application/octet-stream",
      contentEncoding,
      cacheControl: file.cacheControl || "public, max-age=31536000, immutable",
      sources: file.sources ?? file.src ?? [],
      sha256: file.sha256 ?? file.h ?? null,
      encodedSha256,
    });
  }
  return uploads;
}

async function verifyEncodedUploads(uploads) {
  const encodedUploads = uploads.filter((upload) => upload.contentEncoding);
  if (!encodedUploads.length) return;
  let completed = 0;
  await runPool(encodedUploads, Math.min(concurrency, 4), async (upload) => {
    const actual = await gzipFileMetadata(upload.stagePath);
    if (actual.size !== upload.size || actual.sha256 !== upload.encodedSha256) {
      throw new Error(
        `Encoded release metadata mismatch for ${upload.relativePath}: ` +
          `expected ${upload.size}/${upload.encodedSha256}, found ${actual.size}/${actual.sha256}`,
      );
    }
    completed += 1;
    if (completed % 250 === 0 || completed === encodedUploads.length) {
      console.log(`[mir2-r2] verified encoded assets ${completed}/${encodedUploads.length}`);
    }
  });
}

async function gzipFileMetadata(filePath) {
  const hash = createHash("sha256");
  let size = 0;
  const sink = new Writable({
    write(chunk, _encoding, callback) {
      size += chunk.length;
      hash.update(chunk);
      callback();
    },
  });
  await pipeline(createReadStream(filePath), createGzip(FULL_PACK_GZIP_OPTIONS), sink);
  return { size, sha256: hash.digest("hex") };
}

function normalizeReleaseFileRelativePath(file) {
  const value = file.relativePath ?? file.p ?? file.path ?? "";
  return String(value).replace(/^\/+/, "");
}

async function uploadWithRetry(upload) {
  let lastError;
  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    try {
      if (uploadDriver === "api") await uploadViaCloudflareApi(upload);
      else if (uploadDriver === "worker") await uploadViaWorker(upload);
      else if (uploadDriver === "s3") await uploadViaS3(upload);
      else {
        await runWrangler(
          [
            "wrangler",
            "r2",
            "object",
            "put",
            `${bucket}/${upload.objectKey}`,
            remote ? "--remote" : "--local",
            "--file",
            upload.stagePath,
            "--content-type",
            upload.contentType,
            "--cache-control",
            upload.cacheControl,
          ],
          { label: upload.objectKey },
        );
      }
      return;
    } catch (error) {
      lastError = error;
      if (attempt >= maxAttempts || !isRetryableUploadError(error)) break;
      const delayMs = uploadRetryDelayMs(error, attempt);
      console.warn(
        `[mir2-r2] retry ${attempt + 1}/${maxAttempts} in ${delayMs}ms ${upload.objectKey}`,
      );
      await sleep(delayMs);
    }
  }
  throw lastError;
}

function isRetryableUploadError(error) {
  const status = Number(error?.status ?? error?.$metadata?.httpStatusCode);
  if (!Number.isFinite(status) || status <= 0) return true;
  return status === 408 || status === 425 || status === 429 || status >= 500;
}

function uploadRetryDelayMs(error, attempt) {
  const retryAfterMs = Number(error?.retryAfterMs);
  if (Number.isFinite(retryAfterMs) && retryAfterMs >= 0) {
    return Math.min(Math.max(Math.ceil(retryAfterMs), 1_000), 60_000);
  }
  const baseDelayMs = Number(error?.status) === 429 ? 5_000 : 750;
  return Math.min(baseDelayMs * (2 ** Math.max(attempt - 1, 0)), 30_000);
}

async function uploadViaWorker(upload) {
  const source = createReadStream(upload.stagePath);
  const body = upload.contentEncoding === "gzip"
    ? source.pipe(createGzip(FULL_PACK_GZIP_OPTIONS))
    : source;
  const endpoint = new URL(workerUploadPath, workerUploadUrl);
  endpoint.searchParams.set("key", upload.objectKey);
  const headers = new Headers({
    Authorization: `Bearer ${workerUploadSecret}`,
    "Content-Type": upload.contentType,
    "Content-Length": String(upload.size),
    "Cache-Control": upload.cacheControl,
  });
  // Headers stringifies `undefined` instead of omitting it. Only attach
  // optional representation metadata when a real value exists, otherwise an
  // identity asset becomes the literal unsupported encoding "undefined".
  if (upload.contentEncoding) {
    // This request body is an opaque storage representation, not an HTTP
    // representation for Cloudflare to decode. A private header prevents edge
    // request normalization from stripping Content-Encoding or transparently
    // decoding the bytes before the upload Worker stores them.
    headers.set("X-Mir2-Content-Encoding", upload.contentEncoding);
  }
  if (upload.sha256) headers.set("X-Mir2-Sha256", upload.sha256);
  if (upload.encodedSha256) headers.set("X-Mir2-Encoded-Sha256", upload.encodedSha256);
  const response = await fetch(endpoint, {
    method: "PUT",
    headers,
    body,
    duplex: "half",
  });
  const text = await response.text();
  if (!response.ok) {
    throw createUploadHttpError(
      `R2 upload Worker failed for ${upload.objectKey}: HTTP ${response.status} ${text || response.statusText}`,
      response,
    );
  }
}

async function uploadViaS3(upload) {
  const { PutObjectCommand } = await import("@aws-sdk/client-s3");
  const source = createReadStream(upload.stagePath);
  const body = upload.contentEncoding === "gzip"
    ? source.pipe(createGzip(FULL_PACK_GZIP_OPTIONS))
    : source;
  const client = await createS3Client();
  const metadata = {};
  if (upload.sha256) metadata.sha256 = upload.sha256;
  if (upload.encodedSha256) metadata.encodedsha256 = upload.encodedSha256;
  await client.send(
    new PutObjectCommand({
      Bucket: bucket,
      Key: upload.objectKey,
      Body: body,
      ContentLength: upload.size,
      ContentType: upload.contentType,
      ContentEncoding: upload.contentEncoding || undefined,
      CacheControl: upload.cacheControl,
      Metadata: Object.keys(metadata).length > 0 ? metadata : undefined,
    }),
  );
}

function resolveS3Endpoint() {
  if (s3Endpoint) {
    return s3Endpoint;
  }

  if (!cloudflareAccountId) {
    throw new Error(
      "Set MIR2_R2_S3_ENDPOINT or CLOUDFLARE_ACCOUNT_ID before using MIR2_R2_UPLOAD_DRIVER=r2-s3.",
    );
  }

  return `https://${cloudflareAccountId}.r2.cloudflarestorage.com`;
}

function normalizeUploadDriver(value) {
  if (value === "r2-s3") return "s3";
  return value;
}

async function createS3Client() {
  if (s3Client) return s3Client;

  const { S3Client } = await import("@aws-sdk/client-s3");
  s3Client = new S3Client({
    region: "auto",
    endpoint: resolveS3Endpoint(),
    forcePathStyle: true,
    requestChecksumCalculation: "WHEN_REQUIRED",
    responseChecksumValidation: "WHEN_REQUIRED",
    credentials: {
      accessKeyId: s3AccessKeyId,
      secretAccessKey: s3SecretAccessKey,
      ...(s3SessionToken ? { sessionToken: s3SessionToken } : {}),
    },
  });

  return s3Client;
}

async function uploadViaCloudflareApi(upload) {
  const source = createReadStream(upload.stagePath);
  const body = upload.contentEncoding === "gzip"
    ? source.pipe(createGzip(FULL_PACK_GZIP_OPTIONS))
    : source;
  const endpoint = new URL(
    `${cloudflareApiBaseUrl.replace(/\/+$/, "")}/accounts/${encodeURIComponent(
      cloudflareAccountId,
    )}/r2/buckets/${encodeURIComponent(bucket)}/objects/${encodeObjectKey(upload.objectKey)}`,
  );
  const response = await fetch(endpoint, {
    method: "PUT",
    headers: {
      Authorization: `Bearer ${cloudflareApiToken}`,
      "Content-Type": upload.contentType,
      "Content-Encoding": upload.contentEncoding || undefined,
      "Content-Length": String(upload.size),
      "Cache-Control": upload.cacheControl,
    },
    body,
    duplex: "half",
  });
  const text = await response.text();
  let payload = null;
  if (text) {
    try {
      payload = JSON.parse(text);
    } catch {
      payload = null;
    }
  }
  if (!response.ok || payload?.success === false) {
    const message = payload?.errors?.map((error) => error.message).join("; ") || text || response.statusText;
    throw createUploadHttpError(
      `Cloudflare R2 API upload failed for ${upload.objectKey}: HTTP ${response.status} ${message}`,
      response,
    );
  }
}

function createUploadHttpError(message, response) {
  const error = new Error(message);
  error.status = response.status;
  error.retryAfterMs = parseRetryAfterMs(response.headers.get("retry-after"));
  return error;
}

function parseRetryAfterMs(value) {
  const normalized = String(value ?? "").trim();
  if (!normalized) return null;
  const seconds = Number(normalized);
  if (Number.isFinite(seconds) && seconds >= 0) {
    return Math.ceil(seconds * 1_000);
  }
  const timestamp = Date.parse(normalized);
  if (!Number.isFinite(timestamp)) return null;
  return Math.max(0, timestamp - Date.now());
}

function encodeObjectKey(objectKey) {
  return String(objectKey || "")
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/");
}

function runWrangler(args, options = {}) {
  return runCommand(wranglerBin, args, options);
}

function runCommand(command, commandArgs, { label, allowFailure = false } = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, commandArgs, {
      cwd: REPO_ROOT,
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => {
      const text = chunk.toString();
      stdout += text;
      process.stdout.write(text);
    });
    child.stderr.on("data", (chunk) => {
      const text = chunk.toString();
      stderr += text;
      process.stderr.write(text);
    });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0 || allowFailure) {
        resolve({ code, stdout, stderr });
        return;
      }
      reject(new Error(`${label ?? command} failed with exit ${code}`));
    });
  });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function runPool(items, limit, worker) {
  let index = 0;
  async function next() {
    while (index < items.length) {
      const item = items[index];
      index += 1;
      await worker(item);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, next));
}

function joinObjectKey(prefix, relativePath) {
  const cleanPrefix = String(prefix || "").trim().replace(/^\/+|\/+$/g, "");
  const cleanPath = String(relativePath || "").replace(/^\/+/, "");
  return cleanPrefix ? `${cleanPrefix}/${cleanPath}` : cleanPath;
}

function numberArg(value, fallback) {
  const numeric = Number(value);
  return Number.isFinite(numeric) && numeric > 0 ? Math.trunc(numeric) : fallback;
}

function numberOrNull(value) {
  if (value == null || value === "") return null;
  const numeric = Number(value);
  return Number.isSafeInteger(numeric) && numeric >= 0 ? numeric : null;
}

function normalizeContentEncoding(value) {
  const encoding = String(value || "").trim().toLowerCase();
  if (!encoding) return null;
  if (encoding !== "gzip") {
    throw new Error(`Unsupported release content encoding: ${encoding}`);
  }
  return encoding;
}

function normalizeSha256(value) {
  const digest = String(value || "").trim().toLowerCase();
  return /^[a-f0-9]{64}$/.test(digest) ? digest : null;
}

function booleanArg(value, fallback) {
  if (value == null) return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function normalizeOptionalUrl(value) {
  const text = String(value || "").trim();
  if (!text) return "";
  return new URL(text).href.replace(/\/+$/, "");
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (!token.startsWith("--")) continue;
    const key = token.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = true;
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
