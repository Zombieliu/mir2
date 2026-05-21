import { spawn } from "node:child_process";
import fs from "node:fs/promises";
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
const uploadDriver = String(args.driver ?? process.env.MIR2_R2_UPLOAD_DRIVER ?? "wrangler").toLowerCase();
const cloudflareAccountId = args.accountId ?? process.env.CLOUDFLARE_ACCOUNT_ID ?? process.env.CF_ACCOUNT_ID ?? "";
const cloudflareApiToken = process.env.CLOUDFLARE_API_TOKEN ?? process.env.CF_API_TOKEN ?? "";
const workerUploadUrl = normalizeOptionalUrl(args.workerUrl ?? process.env.MIR2_R2_UPLOAD_WORKER_URL ?? "");
const workerUploadSecret = process.env.MIR2_R2_UPLOAD_SECRET ?? "";
const wranglerBin = process.platform === "win32" ? "npx.cmd" : "npx";

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
          driver: uploadDriver,
          objectPrefix: release.objectPrefix,
          assetBaseUrl: release.assetBaseUrl ?? null,
          uploadCount: uploads.length,
          totalBytes,
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

  if (!["wrangler", "api", "worker"].includes(uploadDriver)) {
    throw new Error(`Unsupported MIR2_R2_UPLOAD_DRIVER: ${uploadDriver}. Expected "wrangler", "api", or "worker".`);
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

  let completed = 0;
  await runPool(uploads, concurrency, async (upload) => {
    await uploadWithRetry(upload);
    completed += 1;
    if (completed % 25 === 0 || completed === uploads.length) {
      console.log(`[mir2-r2] uploaded ${completed}/${uploads.length}`);
    }
  });

  console.log(
    JSON.stringify(
      {
        ok: true,
        dryRun: false,
        manifestPath,
        bucket,
        driver: uploadDriver,
        objectPrefix: release.objectPrefix,
        assetBaseUrl: release.assetBaseUrl ?? null,
        uploadCount: uploads.length,
        totalBytes,
      },
      null,
      2,
    ),
  );
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
