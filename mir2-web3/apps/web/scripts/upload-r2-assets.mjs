import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { createReadStream } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_MANIFEST = path.resolve(
  REPO_ROOT,
  "docs",
  "generated",
  "remote-assets",
  "latest-remote-asset-release.json",
);

const args = parseArgs(process.argv.slice(2));
const manifestPath = path.resolve(args.manifest ?? process.env.MIR2_REMOTE_ASSET_RELEASE_MANIFEST ?? DEFAULT_MANIFEST);
const dryRun = booleanArg(args.dryRun ?? process.env.MIR2_R2_DRY_RUN, false);
const bucket = args.bucket ?? process.env.MIR2_R2_BUCKET ?? "";
const ensureBucket = booleanArg(args.ensureBucket ?? process.env.MIR2_R2_ENSURE_BUCKET, false);
const concurrency = numberArg(args.concurrency ?? process.env.MIR2_R2_UPLOAD_CONCURRENCY, 4);
const includeReleaseManifest = booleanArg(args.includeReleaseManifest ?? process.env.MIR2_R2_UPLOAD_RELEASE_MANIFEST, true);
const remote = booleanArg(args.remote ?? process.env.MIR2_R2_REMOTE, true);
const maxAttempts = numberArg(args.maxAttempts ?? process.env.MIR2_R2_UPLOAD_ATTEMPTS, 3);
const verifyOriginalAssets = booleanArg(
  args.verifyOriginalAssets ?? process.env.MIR2_R2_VERIFY_ORIGINAL_ASSETS,
  false,
);
const verifyOriginalAssetConcurrency = numberArg(
  args.verifyOriginalAssetConcurrency ?? process.env.MIR2_R2_VERIFY_ORIGINAL_ASSET_CONCURRENCY,
  16,
);
const uploadDriverInput = String(args.driver ?? process.env.MIR2_R2_UPLOAD_DRIVER ?? "r2-s3").toLowerCase();
const uploadDriver = normalizeUploadDriver(uploadDriverInput);
const cloudflareAccountId = args.accountId ?? process.env.CLOUDFLARE_ACCOUNT_ID ?? "";
const cloudflareApiToken = process.env.CLOUDFLARE_API_TOKEN ?? "";
const workerUploadUrl = normalizeOptionalUrl(args.workerUrl ?? process.env.MIR2_R2_UPLOAD_WORKER_URL ?? "");
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
  const uploads = await buildUploadList(release);

  if (includeReleaseManifest) {
    uploads.push({
      path: "/remote-asset-release.json",
      relativePath: "remote-asset-release.json",
      stagePath: manifestPath,
      objectKey: joinObjectKey(release.objectPrefix, "remote-asset-release.json"),
      size: (await fs.stat(manifestPath)).size,
      contentType: "application/json; charset=utf-8",
      cacheControl: "public, max-age=60, stale-while-revalidate=300",
      sources: ["release-manifest"],
    });
  }

  const totalBytes = uploads.reduce((sum, upload) => sum + upload.size, 0);
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
          verifyOriginalAssets,
          sample: uploads.slice(0, 8).map((upload) => ({
            objectKey: upload.objectKey,
            size: upload.size,
            contentType: upload.contentType,
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
  await runPool(uploads, concurrency, async (upload) => {
    await uploadWithRetry(upload);
    completed += 1;
    if (completed % 25 === 0 || completed === uploads.length) {
      console.log(`[mir2-r2] uploaded ${completed}/${uploads.length}`);
    }
  });

  if (verifyOriginalAssets) {
    await verifyUploadedOriginalAssets(release, uploads);
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
        verifiedOriginalAssetCount: verifyOriginalAssets
          ? uploads.filter((upload) => upload.sources?.includes("original-asset-manifest")).length
          : 0,
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
    const response = await fetch(url, { method: "HEAD" });
    if (!response.ok) {
      throw new Error(
        `Original asset missing after R2 upload: HTTP ${response.status} ${url} objectKey=${upload.objectKey}`,
      );
    }
    completed += 1;
    if (completed % 500 === 0 || completed === originalAssetUploads.length) {
      console.log(`[mir2-r2] verified original assets ${completed}/${originalAssetUploads.length}`);
    }
  });
}

async function buildUploadList(release) {
  if (!Array.isArray(release.files)) {
    throw new Error(`Release manifest ${manifestPath} has no files array.`);
  }

  const uploads = [];
  for (const file of release.files) {
    if (!file.stagePath || !file.objectKey) {
      throw new Error(`Invalid release file entry: ${JSON.stringify(file)}`);
    }
    const stats = await fs.stat(file.stagePath);
    if (!stats.isFile()) throw new Error(`Not a file: ${file.stagePath}`);
    uploads.push({
      path: file.path,
      relativePath: file.relativePath,
      stagePath: file.stagePath,
      objectKey: file.objectKey,
      size: stats.size,
      contentType: file.contentType || "application/octet-stream",
      cacheControl: file.cacheControl || "public, max-age=31536000, immutable",
      sources: file.sources ?? [],
    });
  }
  return uploads;
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
      if (attempt >= maxAttempts) break;
      const delayMs = 750 * attempt;
      console.warn(`[mir2-r2] retry ${attempt + 1}/${maxAttempts} ${upload.objectKey}`);
      await sleep(delayMs);
    }
  }
  throw lastError;
}

async function uploadViaWorker(upload) {
  const body = await fs.readFile(upload.stagePath);
  const endpoint = new URL("/upload", workerUploadUrl);
  endpoint.searchParams.set("key", upload.objectKey);
  const response = await fetch(endpoint, {
    method: "PUT",
    headers: {
      Authorization: `Bearer ${workerUploadSecret}`,
      "Content-Type": upload.contentType,
      "Cache-Control": upload.cacheControl,
    },
    body,
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(
      `R2 upload Worker failed for ${upload.objectKey}: HTTP ${response.status} ${text || response.statusText}`,
    );
  }
}

async function uploadViaS3(upload) {
  const { PutObjectCommand } = await import("@aws-sdk/client-s3");
  const body = createReadStream(upload.stagePath);
  const client = await createS3Client();
  await client.send(
    new PutObjectCommand({
      Bucket: bucket,
      Key: upload.objectKey,
      Body: body,
      ContentType: upload.contentType,
      CacheControl: upload.cacheControl,
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
    credentials: {
      accessKeyId: s3AccessKeyId,
      secretAccessKey: s3SecretAccessKey,
      ...(s3SessionToken ? { sessionToken: s3SessionToken } : {}),
    },
  });

  return s3Client;
}

async function uploadViaCloudflareApi(upload) {
  const body = await fs.readFile(upload.stagePath);
  const endpoint = new URL(
    `https://api.cloudflare.com/client/v4/accounts/${encodeURIComponent(
      cloudflareAccountId,
    )}/r2/buckets/${encodeURIComponent(bucket)}/objects/${encodeObjectKey(upload.objectKey)}`,
  );
  const response = await fetch(endpoint, {
    method: "PUT",
    headers: {
      Authorization: `Bearer ${cloudflareApiToken}`,
      "Content-Type": upload.contentType,
      "Cache-Control": upload.cacheControl,
    },
    body,
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
    throw new Error(`Cloudflare R2 API upload failed for ${upload.objectKey}: HTTP ${response.status} ${message}`);
  }
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
