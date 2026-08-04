import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const productionConfig = JSON.parse(
  await fs.readFile(path.join(REPO_ROOT, "config", "production-web-assets.json"), "utf8"),
);
const args = parseArgs(process.argv.slice(2));
const releasePath = path.resolve(args.release ?? "/tmp/mir2-map-atlas-v2-release.json");
const release = JSON.parse(await fs.readFile(releasePath, "utf8"));
const baseUrls = parseBaseUrls(
  args.baseUrls ?? productionConfig.browserFallbackBaseUrls ?? [productionConfig.assetBaseUrl],
);
const fullBaseCount = integerArg(args.fullBaseCount, 1);
const concurrency = integerArg(args.concurrency, 8);
const attempts = integerArg(args.attempts, 4);
const referer = String(args.referer ?? "https://mir2.obelisk.build/");

await main();

async function main() {
  if (!Array.isArray(release.files) || release.files.length === 0) {
    throw new Error(`Release ${releasePath} has no files.`);
  }
  if (baseUrls.length === 0) throw new Error("At least one map-atlas base URL is required.");

  const results = [];
  for (let index = 0; index < baseUrls.length; index += 1) {
    const baseUrl = baseUrls[index];
    const full = index < fullBaseCount;
    let completed = 0;
    let downloadedBytes = 0;
    await runPool(release.files, concurrency, async (file) => {
      const relativePath = String(file.relativePath ?? file.path ?? "").replace(/^\/+/, "");
      const url = `${baseUrl}/${relativePath}`;
      const shouldDownload = full || relativePath.endsWith(`manifest.${release.mapAtlas.contentHash}.json`);
      const response = await fetchWithRetry(url, {
        method: shouldDownload ? "GET" : "HEAD",
        headers: { Origin: new URL(referer).origin, Referer: referer },
      });
      if (!response.ok) throw new Error(`${response.status} ${response.statusText}: ${url}`);
      const cors = response.headers.get("access-control-allow-origin");
      if (cors !== "*" && cors !== new URL(referer).origin) {
        throw new Error(`Missing browser CORS header for ${url}: ${cors ?? "none"}`);
      }
      const cacheControl = response.headers.get("cache-control") ?? "";
      if (!hasOneYearImmutablePolicy(cacheControl)) {
        throw new Error(`One-year immutable cache policy missing for ${url}: ${cacheControl || "none"}`);
      }
      const contentLength = Number(response.headers.get("content-length") ?? 0);
      if (contentLength > 0 && contentLength !== Number(file.size)) {
        throw new Error(`Content length mismatch for ${url}: expected ${file.size}, found ${contentLength}`);
      }
      if (shouldDownload) {
        const bytes = Buffer.from(await response.arrayBuffer());
        if (bytes.length !== Number(file.size)) {
          throw new Error(`Body size mismatch for ${url}: expected ${file.size}, found ${bytes.length}`);
        }
        const hash = createHash("sha256").update(bytes).digest("hex");
        if (hash !== file.sha256) {
          throw new Error(`SHA-256 mismatch for ${url}: expected ${file.sha256}, found ${hash}`);
        }
        downloadedBytes += bytes.length;
      }
      completed += 1;
      if (completed % 20 === 0 || completed === release.files.length) {
        console.log(`[map-atlas-verify] ${index + 1}/${baseUrls.length} ${completed}/${release.files.length}`);
      }
    });
    results.push({ baseUrl, mode: full ? "sha256" : "head-plus-manifest-sha256", downloadedBytes });
  }

  console.log(
    JSON.stringify(
      {
        ok: true,
        releasePath,
        contentHash: release.mapAtlas.contentHash,
        fileCount: release.files.length,
        totalBytes: release.mapAtlas.totalBytes,
        results,
      },
      null,
      2,
    ),
  );
}

async function runPool(items, limit, worker) {
  let cursor = 0;
  async function next() {
    while (cursor < items.length) {
      const item = items[cursor];
      cursor += 1;
      await worker(item);
    }
  }
  await Promise.all(Array.from({ length: Math.min(limit, items.length) }, next));
}

async function fetchWithRetry(url, options) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(url, options);
      if (response.ok || response.status < 500 || attempt === attempts) return response;
      await response.body?.cancel();
      lastError = new Error(`${response.status} ${response.statusText}: ${url}`);
    } catch (error) {
      lastError = error;
    }
    console.warn(`[map-atlas-verify] retry ${attempt + 1}/${attempts} ${url}`);
    await new Promise((resolve) => setTimeout(resolve, 500 * attempt));
  }
  throw lastError;
}

function parseBaseUrls(value) {
  const values = Array.isArray(value) ? value : String(value ?? "").split(",");
  return [...new Set(values.map((entry) => String(entry).trim().replace(/\/+$/, "")).filter(Boolean))];
}

function integerArg(value, fallback) {
  const number = Number(value);
  return Number.isSafeInteger(number) && number >= 0 ? number : fallback;
}

function hasOneYearImmutablePolicy(value) {
  if (/\bimmutable\b/i.test(value)) return true;
  const maxAge = /\bmax-age=(\d+)\b/i.exec(value)?.[1];
  return Number(maxAge ?? 0) >= 31_536_000;
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (!value.startsWith("--")) continue;
    const [rawKey, inlineValue] = value.slice(2).split("=", 2);
    parsed[rawKey] = inlineValue ?? values[index + 1];
    if (inlineValue == null) index += 1;
  }
  return parsed;
}
