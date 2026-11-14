import { createHash, randomUUID } from "node:crypto";
import { createReadStream, createWriteStream } from "node:fs";
import fs from "node:fs/promises";
import http from "node:http";
import https from "node:https";
import path from "node:path";
import { Transform, Writable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { constants, createGunzip, createGzip } from "node:zlib";

// Bound file operations, not individual requests: each file still verifies the
// existing object, PUTs at most once when missing, then verifies the full body.
export const FIXED_R16_FILE_CONCURRENCY = 32;

// This publisher accepts one reviewed finite payload, not an arbitrary public tree.
// sourceSha in the dispatch identifies the frozen CI implementation; the material
// source below is retained separately because the archive predates that commit.
export const FIXED_R16 = Object.freeze({
  repository: "Zombieliu/mir2",
  materialSourceSha: "91fc453b70f3e28adc0a49ecdd3695963b160a1f",
  version: "20261108-r16-08ec86d5105c",
  prefix: "mir2/v/20261108-r16-08ec86d5105c",
  publicOrigin: "https://assets.mir2.obelisk.build",
  bundleName: "r16-assets-20261108-r16-08ec86d5105c.tar.gz",
  bundleBytes: 318602609,
  bundleSha256: "7328514b323fe73169332f0c6dca35cacc872ff021ddce8212db3ac216bc3dcc",
  payloadSha256: "08ec86d5105c7c00405c7342fff3392ba118216d780e22d50e716c47cccdd453",
  manifestSha256: "32255399aaae2dc9a133a5827252256ece782aaae5d237152ffebc2d8f272155",
  manifestBytes: 15716826,
  fileCount: 42738,
  payloadBytes: 417098594,
  wireBytes: 341600433,
  gzipFiles: 8,
  gzipOs: 10,
  fontFiles: 17,
  finiteFiles: 42724,
  runtimeVersion: "bevy-a46905128c316656",
  fallbackPrefixes: ["mir2/v/20260818-main-97887dcd", "mir2/v/20260730-fullcrystal-f71b89aa-gzip1"],
  maxFileBytes: 64 * 1024 * 1024,
  maxExpandedBytes: 512 * 1024 * 1024,
});

export const METADATA_NAMES = Object.freeze([
  "asset-upload-plan.portable.json", "manifest-upload-plan.portable.json", "payload-pins.json",
  "font-pins.json", "release/remote-asset-release.json", "verification-receipt.json", "verify-upload-bundle.mjs",
]);
const SHA256 = /^[a-f0-9]{64}$/;
const SHA1 = /^[a-f0-9]{40}$/;
const IMMUTABLE = "public, max-age=31536000, immutable";
const MANIFEST_CACHE = "public, max-age=60, stale-while-revalidate=300";

export class FixedAssetError extends Error {
  constructor(code) { super(code); this.name = "FixedAssetError"; this.code = code; }
}
export function requireFixed(condition, code) { if (!condition) throw new FixedAssetError(code); }
export const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");

export function validateDispatch(input, contract = FIXED_R16) {
  requireFixed(input.repository === contract.repository, "REPOSITORY_MISMATCH");
  requireFixed(SHA1.test(input.sourceSha) && input.sourceSha === input.githubSha, "SOURCE_SHA_MISMATCH");
  requireFixed(typeof input.releaseTag === "string" && /^[A-Za-z0-9][A-Za-z0-9._/-]{0,127}$/.test(input.releaseTag)
    && !input.releaseTag.includes("..") && !input.releaseTag.includes("//"), "RELEASE_TAG_INVALID");
  for (const field of ["bundleSha256", "payloadSha256", "manifestSha256"]) {
    requireFixed(SHA256.test(input[field]) && input[field] === contract[field], "FIXED_HASH_MISMATCH");
  }
}

export function safeRelative(value) {
  requireFixed(typeof value === "string" && value.length > 0 && value.length <= 512
    && !/^[\/]|[\\:%?#\u0000-\u001f\u007f]/u.test(value)
    && value.split("/").every(part => part && part !== "." && part !== ".." && !part.startsWith(".")), "UNSAFE_PATH");
  return value;
}

export async function hashFile(filename, expectedBytes) {
  const stat = await fs.lstat(filename);
  requireFixed(stat.isFile() && !stat.isSymbolicLink(), "NONREGULAR_FILE");
  if (expectedBytes !== undefined) requireFixed(stat.size === expectedBytes, "FILE_SIZE_MISMATCH");
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(filename)) hash.update(chunk);
  return { bytes: stat.size, sha256: hash.digest("hex") };
}

class TarReader {
  constructor(stream) { this.iterator = stream[Symbol.asyncIterator](); this.pending = Buffer.alloc(0); }
  async take(size, optional = false) {
    if (size === 0) return Buffer.alloc(0);
    while (this.pending.length < size) {
      const next = await this.iterator.next();
      if (next.done) { requireFixed(optional && this.pending.length === 0, "TAR_TRUNCATED"); return null; }
      this.pending = this.pending.length ? Buffer.concat([this.pending, next.value]) : next.value;
    }
    const bytes = this.pending.subarray(0, size); this.pending = this.pending.subarray(size); return bytes;
  }
}
function tarText(block, offset, size) {
  const field = block.subarray(offset, offset + size), end = field.indexOf(0);
  requireFixed(end < 0 || field.subarray(end).every(value => value === 0), "TAR_FIELD_PADDING");
  try { return new TextDecoder("utf-8", { fatal: true }).decode(end < 0 ? field : field.subarray(0, end)); }
  catch { throw new FixedAssetError("TAR_TEXT_INVALID"); }
}
function tarOctal(block, offset, size) {
  const text = block.subarray(offset, offset + size).toString("ascii").replace(/[\0 ]+$/g, "").trim();
  requireFixed(/^[0-7]+$/.test(text), "TAR_NUMBER_INVALID");
  const value = Number.parseInt(text, 8);
  requireFixed(Number.isSafeInteger(value) && value >= 0, "TAR_NUMBER_INVALID"); return value;
}

// Strict regular-file USTAR only: no system tar, links, PAX, devices or path repair.
// The archive's embedded .mjs is data and is never imported or executed.
export async function extractFixedBundle(bundle, destination, contract = FIXED_R16, onProgress = () => {}) {
  const actual = await hashFile(bundle, contract.bundleBytes);
  requireFixed(actual.sha256 === contract.bundleSha256, "BUNDLE_HASH_MISMATCH");
  await fs.mkdir(destination, { recursive: false, mode: 0o700 });
  const source = createReadStream(bundle), unzip = createGunzip();
  source.on("error", error => unzip.destroy(error)); source.pipe(unzip);
  const reader = new TarReader(unzip), names = [], seen = new Set(); let expandedBytes = 0;
  try {
    while (true) {
      const block = await reader.take(512);
      if (block.every(value => value === 0)) {
        requireFixed((await reader.take(512)).every(value => value === 0), "TAR_END_INVALID");
        requireFixed(await reader.take(1, true) === null, "TAR_TRAILING_DATA"); break;
      }
      const checksum = tarOctal(block, 148, 8), copy = Buffer.from(block); copy.fill(32, 148, 156);
      requireFixed(copy.reduce((sum, value) => sum + value, 0) === checksum, "TAR_CHECKSUM_INVALID");
      requireFixed(block.subarray(257, 263).equals(Buffer.from("ustar\0"))
        && block.subarray(263, 265).toString() === "00", "TAR_FORMAT_INVALID");
      requireFixed(block[156] === 48 && block.subarray(157, 257).every(value => value === 0), "TAR_NONREGULAR_ENTRY");
      requireFixed(tarOctal(block, 100, 8) === 0o644 && tarOctal(block, 108, 8) === 0
        && tarOctal(block, 116, 8) === 0 && tarOctal(block, 136, 12) === 0, "TAR_METADATA_INVALID");
      const prefix = tarText(block, 345, 155), suffix = tarText(block, 0, 100);
      const name = safeRelative(prefix ? `${prefix}/${suffix}` : suffix), key = name.toLowerCase();
      requireFixed(name.startsWith("payload/") || METADATA_NAMES.includes(name), "TAR_UNEXPECTED_ENTRY");
      requireFixed(!seen.has(key), "TAR_DUPLICATE_ENTRY"); seen.add(key);
      requireFixed(!names.length || names.at(-1) < name, "TAR_ORDER_INVALID"); names.push(name);
      requireFixed(names.length <= contract.fileCount + METADATA_NAMES.length, "TAR_FILE_LIMIT");
      const size = tarOctal(block, 124, 12); expandedBytes += size;
      requireFixed(size <= contract.maxFileBytes && expandedBytes <= contract.maxExpandedBytes, "TAR_SIZE_LIMIT");
      const filename = path.join(destination, ...name.split("/"));
      await fs.mkdir(path.dirname(filename), { recursive: true, mode: 0o700 });
      const handle = await fs.open(filename, "wx", 0o600);
      try {
        for (let remaining = size; remaining > 0;) {
          const bytes = await reader.take(Math.min(64 * 1024, remaining));
          let offset = 0;
          while (offset < bytes.length) { const wrote = await handle.write(bytes, offset); offset += wrote.bytesWritten; }
          remaining -= bytes.length;
        }
      } finally { await handle.close(); }
      const padding = (512 - size % 512) % 512;
      requireFixed((await reader.take(padding)).every(value => value === 0), "TAR_PADDING_INVALID");
      if (names.length % 10000 === 0) onProgress({ phase: "extract", files: names.length });
    }
    requireFixed(names.length === contract.fileCount + METADATA_NAMES.length, "TAR_FILE_COUNT_MISMATCH");
    return { names, expandedBytes };
  } finally { source.destroy(); unzip.destroy(); }
}

async function boundedJson(root, name, limit = 32 * 1024 * 1024) {
  const filename = path.join(root, ...safeRelative(name).split("/")), stat = await fs.lstat(filename);
  requireFixed(stat.isFile() && !stat.isSymbolicLink() && stat.size <= limit, "JSON_FILE_INVALID");
  try { return JSON.parse(await fs.readFile(filename, "utf8")); }
  catch { throw new FixedAssetError("JSON_INVALID"); }
}

export async function runBounded(items, concurrency, operation) {
  let next = 0, failure;
  await Promise.all(Array.from({ length: Math.min(concurrency, items.length) }, async () => {
    while (!failure && next < items.length) {
      const item = items[next++];
      try { await operation(item); } catch (error) { failure ??= error; }
    }
  }));
  if (failure) throw failure;
}

// Windows-generated gzip pins include OS=10. Normalize that one header byte on
// Linux too, then enforce the original encoded SHA. Deflate differences fail.
async function encodedCopy(source, destination, contract) {
  await pipeline(createReadStream(source), createGzip({ level: constants.Z_BEST_COMPRESSION, mtime: 0 }),
    normalizeGzipHeader(contract.gzipOs), createWriteStream(destination, { flags: "wx", mode: 0o600 }));
  return hashFile(destination);
}

export function normalizeGzipHeader(os) {
  let head = Buffer.alloc(0), emitted = false;
  return new Transform({
    transform(chunk, _encoding, done) {
      if (emitted) { done(null, chunk); return; }
      head = Buffer.concat([head, chunk]);
      if (head.length < 10) { done(); return; }
      if (!head.subarray(0, 8).equals(Buffer.from([31, 139, 8, 0, 0, 0, 0, 0]))) {
        done(new FixedAssetError("GZIP_HEADER_INVALID")); return;
      }
      head[9] = os; emitted = true; done(null, head);
    },
  });
}

export async function validateExtracted(root, extracted, contract = FIXED_R16, onProgress = () => {}) {
  const plan = await boundedJson(root, "asset-upload-plan.portable.json");
  const manifestPlan = await boundedJson(root, "manifest-upload-plan.portable.json");
  const pins = await boundedJson(root, "payload-pins.json"), fonts = await boundedJson(root, "font-pins.json");
  for (const current of [plan, manifestPlan]) {
    requireFixed(current.schemaVersion === 1 && current.publishReleaseManifest === false
      && current.version === contract.version && current.objectPrefix === contract.prefix
      && current.assetBaseUrl === `${contract.publicOrigin}/${contract.prefix}`
      && current.payloadSha256 === contract.payloadSha256, "PLAN_IDENTITY_MISMATCH");
  }
  requireFixed(plan.kind === "mir2-r16-finite-upload-plan"
    && manifestPlan.kind === "mir2-r16-manifest-last-upload-plan", "PLAN_KIND_INVALID");
  requireFixed(pins.schemaVersion === 1 && pins.payloadSha256 === contract.payloadSha256
    && Array.isArray(pins.files) && pins.files.length === contract.fileCount
    && sha256(JSON.stringify(pins.files)) === contract.payloadSha256, "PAYLOAD_TREE_MISMATCH");
  requireFixed(Array.isArray(plan.files) && plan.files.length === contract.fileCount
    && Array.isArray(manifestPlan.files) && manifestPlan.files.length === 1, "PLAN_FILE_COUNT_MISMATCH");
  const expectedNames = [...METADATA_NAMES, ...pins.files.map(pin => `payload/${safeRelative(pin.p)}`)].sort();
  requireFixed(JSON.stringify(expectedNames) === JSON.stringify(extracted.names), "EXTRACTED_TREE_MISMATCH");
  const pinMap = new Map(), files = []; let totalBytes = 0, wireBytes = 0, gzipFiles = 0;
  for (const [index, pin] of pins.files.entries()) {
    requireFixed(Object.keys(pin).sort().join(",") === "h,p,s" && SHA256.test(pin.h)
      && Number.isSafeInteger(pin.s) && pin.s >= 0 && pin.s <= contract.maxFileBytes, "PIN_INVALID");
    requireFixed(index === 0 || pins.files[index - 1].p < pin.p, "PIN_ORDER_INVALID");
    const file = plan.files[index];
    requireFixed(file.relativePath === pin.p && file.path === `/${pin.p}` && file.stagePath === `payload/${pin.p}`
      && file.objectKey === `${contract.prefix}/${pin.p}` && file.sha256 === pin.h && file.size === pin.s, "PLAN_FILE_MISMATCH");
    validateHeaders(file, IMMUTABLE);
    const localPath = path.join(root, ...file.stagePath.split("/"));
    files.push({ ...file, localPath, wirePath: localPath, wireSize: pin.s, wireSha256: pin.h });
    pinMap.set(pin.p, pin); totalBytes += pin.s;
    if (file.contentEncoding !== undefined) {
      requireFixed(file.contentEncoding === "gzip" && SHA256.test(file.encodedSha256)
        && Number.isSafeInteger(file.encodedSize) && file.encodedSize > 0, "GZIP_METADATA_INVALID");
      gzipFiles++; wireBytes += file.encodedSize;
    } else { requireFixed(file.encodedSize === undefined && file.encodedSha256 === undefined, "ENCODING_METADATA_INVALID"); wireBytes += pin.s; }
  }
  requireFixed(totalBytes === contract.payloadBytes && wireBytes === contract.wireBytes
    && gzipFiles === contract.gzipFiles, "PAYLOAD_TOTAL_MISMATCH");
  requireFixed(fonts.schemaVersion === 1 && Array.isArray(fonts.files) && fonts.files.length === contract.fontFiles
    && new Set(fonts.files.map(file => file.p)).size === contract.fontFiles, "FONT_PINS_INVALID");
  for (const font of fonts.files) requireFixed(font.p.startsWith("original-ui/fonts/")
    && JSON.stringify(font) === JSON.stringify(pinMap.get(font.p)), "FONT_PIN_MISMATCH");
  const releasePath = path.join(root, "release", "remote-asset-release.json");
  const manifestDigest = await hashFile(releasePath, contract.manifestBytes);
  requireFixed(manifestDigest.sha256 === contract.manifestSha256, "MANIFEST_HASH_MISMATCH");
  const manifest = manifestPlan.files[0];
  requireFixed(manifest.path === "/remote-asset-release.json" && manifest.relativePath === "remote-asset-release.json"
    && manifest.stagePath === "release/remote-asset-release.json"
    && manifest.objectKey === `${contract.prefix}/remote-asset-release.json` && manifest.sha256 === contract.manifestSha256
    && manifest.size === contract.manifestBytes && manifest.contentEncoding === undefined, "MANIFEST_PLAN_MISMATCH");
  validateHeaders(manifest, MANIFEST_CACHE);
  const release = await boundedJson(root, "release/remote-asset-release.json");
  requireFixed(release.schemaVersion === 1 && release.kind === "mir2-remote-asset-release"
    && release.version === contract.version && release.objectPrefix === contract.prefix
    && release.assetBaseUrl === plan.assetBaseUrl && release.pathMode === "relative"
    && JSON.stringify(release.fallbackObjectPrefixes) === JSON.stringify(contract.fallbackPrefixes)
    && release.fallbackObjectPrefix === contract.fallbackPrefixes[0]
    && JSON.stringify(release.browserFallbackBaseUrls) === JSON.stringify(contract.fallbackPrefixes.map(p => `${contract.publicOrigin}/${p}`))
    && release.fullCrystalPack?.verified === false && release.candidateCoverage?.allMapsVerified === false
    && release.candidateCoverage?.payloadSha256 === contract.payloadSha256
    && release.candidateCoverage?.localCandidateFilesVerified === contract.fileCount
    && release.candidateCoverage?.finiteFiles === contract.finiteFiles
    && release.bevyRuntime?.version === contract.runtimeVersion, "MANIFEST_IDENTITY_MISMATCH");
  requireFixed(!/"(?:stagePath|localPath|outputDir|password|accessKeyId|secretAccessKey|token)"\s*:/iu.test(JSON.stringify(release)), "MANIFEST_PRIVATE_FIELD");
  requireFixed(Array.isArray(release.files), "MANIFEST_FILES_INVALID");
  const releaseFiles = new Map();
  for (const entry of release.files) {
    safeRelative(entry.p); requireFixed(!releaseFiles.has(entry.p) && SHA256.test(entry.h)
      && Number.isSafeInteger(entry.s) && entry.s >= 0, "MANIFEST_ENTRY_INVALID");
    if (entry.inheritedPrefix !== undefined) requireFixed(contract.fallbackPrefixes.includes(entry.inheritedPrefix), "MANIFEST_INHERITED_PREFIX_INVALID");
    releaseFiles.set(entry.p, entry);
  }
  for (const file of files) {
    const entry = releaseFiles.get(file.relativePath);
    requireFixed(entry && entry.s === file.size && entry.h === file.sha256 && entry.c === file.contentType
      && entry.e === file.contentEncoding && entry.es === file.encodedSize && entry.eh === file.encodedSha256
      && entry.inheritedPrefix === undefined, "MANIFEST_OVERLAY_MISMATCH");
  }
  let completed = 0;
  await runBounded(files, 8, async file => {
    const digest = await hashFile(file.localPath, file.size);
    requireFixed(digest.sha256 === file.sha256, "PAYLOAD_FILE_HASH_MISMATCH");
    if (++completed % 10000 === 0) onProgress({ phase: "raw-verify", files: completed });
  });
  const encodedRoot = path.join(path.dirname(root), "encoded"); await fs.mkdir(encodedRoot, { mode: 0o700 });
  for (const [index, file] of files.entries()) if (file.contentEncoding === "gzip") {
    const wirePath = path.join(encodedRoot, `${index}.gz`), encoded = await encodedCopy(file.localPath, wirePath, contract);
    requireFixed(encoded.bytes === file.encodedSize && encoded.sha256 === file.encodedSha256, "ENCODED_FILE_HASH_MISMATCH");
    Object.assign(file, { wirePath, wireSize: encoded.bytes, wireSha256: encoded.sha256 });
  }
  return { contract, files, manifest: { ...manifest, localPath: releasePath, wirePath: releasePath,
    wireSize: manifest.size, wireSha256: manifest.sha256 }, release, review: {
    prefix: contract.prefix, publicBaseUrl: plan.assetBaseUrl, files: files.length, bytes: totalBytes,
    wireBytes, gzipFiles, gzipOs: contract.gzipOs, fonts: fonts.files.length,
    gzipHashesVerifiedLocally: true,
    payloadSha256: contract.payloadSha256, manifestSha256: contract.manifestSha256,
    allMapsVerified: false, inheritedObjectsReverified: false, manifestLast: true, overwriteExisting: false,
  } };
}

function validateHeaders(file, cache) {
  requireFixed(typeof file.contentType === "string" && file.contentType.length <= 128
    && /^[a-z0-9+./;= -]+$/.test(file.contentType) && file.cacheControl === cache, "PLAN_HEADERS_INVALID");
}

function request(url, { method = "GET", headers = {}, body, timeoutMs = 45000 } = {}) {
  return new Promise((resolve, reject) => {
    const transport = url.protocol === "https:" ? https : http;
    const req = transport.request(url, { method, headers, signal: AbortSignal.timeout(timeoutMs) }, resolve);
    req.setTimeout(timeoutMs, () => req.destroy(new FixedAssetError("HTTP_TIMEOUT")));
    req.on("error", () => reject(new FixedAssetError("HTTP_NETWORK_ERROR")));
    if (body) { body.on("error", () => req.destroy()); body.pipe(req); }
    else req.end();
  });
}

function publicUrl(contract, relativePath, nonce) {
  const url = new URL(`${contract.publicOrigin}/${contract.prefix}/${safeRelative(relativePath).split("/").map(encodeURIComponent).join("/")}`);
  url.searchParams.set("mir2_verify", nonce); return url;
}
const compactCache = value => String(value ?? "").replace(/\s/g, "").toLowerCase();

export async function verifyRemoteFile(file, contract, nonce, allowMissing = false) {
  const response = await request(publicUrl(contract, file.relativePath, nonce), { headers: { "Accept-Encoding": "identity", "Cache-Control": "no-cache" } });
  if (allowMissing && response.statusCode === 404) { response.destroy(); return false; }
  try {
    requireFixed(response.statusCode === 200, "PUBLIC_HTTP_STATUS");
    const actualEncoding = response.headers["content-encoding"] || undefined;
    const storageEncoding = response.headers["x-mir2-storage-content-encoding"] || undefined;
    const decodedGzip = file.contentEncoding === "gzip" && actualEncoding === undefined && storageEncoding === "gzip";
    const expectedWireSize = decodedGzip ? file.size : file.wireSize;
    const expectedWireHash = decodedGzip ? file.sha256 : file.wireSha256;
    const exactCacheControl = compactCache(response.headers["cache-control"]) === compactCache(file.cacheControl);
    // The existing trusted CDN's Cache API HIT omits only `immutable` while
    // retaining the full one-year public lifetime. This is response metadata,
    // not proof that the original stored header was identical. No other cache
    // state, lifetime, visibility or omitted directive is compatible here.
    const hitImmutableOmitted = file.cacheControl === IMMUTABLE
      && response.headers["x-mir2-edge-cache"] === "HIT"
      && compactCache(response.headers["cache-control"]) === "public,max-age=31536000";
    requireFixed(response.headers["content-type"] === file.contentType
      && (exactCacheControl || hitImmutableOmitted)
      && (actualEncoding === file.contentEncoding || decodedGzip)
      && (!storageEncoding || storageEncoding === file.contentEncoding)
      && !response.headers["x-mir2-fallback-object-key"]
      && (!response.headers["x-mir2-asset-key"] || response.headers["x-mir2-asset-key"] === file.objectKey), "PUBLIC_HEADERS_MISMATCH");
    if (response.headers["content-length"] !== undefined) requireFixed(Number(response.headers["content-length"]) === expectedWireSize, "PUBLIC_CONTENT_LENGTH_MISMATCH");
    const wireHash = createHash("sha256"), rawHash = createHash("sha256"); let wireBytes = 0, rawBytes = 0;
    const wire = new Transform({ transform(chunk, _encoding, done) {
      wireBytes += chunk.length;
      if (wireBytes > expectedWireSize) { done(new FixedAssetError("PUBLIC_WIRE_SIZE_MISMATCH")); return; }
      wireHash.update(chunk); done(null, chunk);
    } });
    const sink = new Writable({ write(chunk, _encoding, done) {
      rawBytes += chunk.length;
      if (rawBytes > file.size) { done(new FixedAssetError("PUBLIC_RAW_SIZE_MISMATCH")); return; }
      rawHash.update(chunk); done();
    } });
    await pipeline(response, wire, ...(actualEncoding === "gzip" ? [createGunzip()] : []), sink);
    requireFixed(wireBytes === expectedWireSize && wireHash.digest("hex") === expectedWireHash
      && rawBytes === file.size && rawHash.digest("hex") === file.sha256, "PUBLIC_BYTES_MISMATCH");
    return { representation: file.contentEncoding === "gzip" ? (decodedGzip ? "gzip-decoded" : "gzip-wire") : "identity",
      cacheControlForm: exactCacheControl ? "exact" : "cdn-hit-immutable-omitted" };
  } finally { response.destroy(); }
}

function workerEndpoint(value, allowLoopback) {
  let url;
  try { url = new URL(value); } catch { throw new FixedAssetError("WORKER_URL_INVALID"); }
  requireFixed((url.protocol === "https:" || (allowLoopback && url.protocol === "http:" && url.hostname === "127.0.0.1"))
    && !url.username && !url.password && !url.search && !url.hash
    && ["/", "/upload", ""].includes(url.pathname), "WORKER_URL_INVALID");
  url.pathname = "/upload"; return url;
}

async function workerPut(file, endpoint, secret) {
  const url = new URL(endpoint); url.searchParams.set("key", file.objectKey);
  const headers = { Authorization: `Bearer ${secret}`, "Content-Type": file.contentType,
    "Content-Length": String(file.wireSize), "Cache-Control": file.cacheControl, "X-Mir2-Sha256": file.sha256,
    ...(file.contentEncoding ? { "X-Mir2-Content-Encoding": "gzip", "X-Mir2-Encoded-Sha256": file.wireSha256 } : {}),
  };
  const body = createReadStream(file.wirePath);
  let response;
  try {
    response = await request(url, { method: "PUT", headers, body });
    requireFixed(response.statusCode === 200, "WORKER_HTTP_STATUS");
    const chunks = []; let size = 0;
    for await (const chunk of response) { size += chunk.length; requireFixed(size <= 4096, "WORKER_RESPONSE_LIMIT"); chunks.push(chunk); }
    let data; try { data = JSON.parse(Buffer.concat(chunks).toString()); } catch { throw new FixedAssetError("WORKER_RESPONSE_INVALID"); }
    requireFixed(data.ok === true && data.key === file.objectKey, "WORKER_RESPONSE_INVALID");
  } finally { body.destroy(); response?.destroy(); }
}

export async function publishValidated(validated, options = {}) {
  const { files, manifest, contract } = validated;
  const receipt = options.receipt ?? {};
  Object.assign(receipt, { review: validated.review, uploaded: 0, existingVerified: 0, verified: 0,
    manifestPublished: false, manifestVerified: false, remoteWrites: 0, putAttempts: 0, unknownWriteOutcomes: 0,
    publicGzipWireVerified: 0, publicGzipDecodedVerified: 0,
    publicCacheControlExactVerified: 0, publicCacheControlHitImmutableOmittedVerified: 0, manifestCacheControlForm: null,
    dryRun: options.dryRun === true });
  if (options.dryRun === true) { receipt.passed = true; return receipt; }
  requireFixed(typeof options.secret === "string" && options.secret.length >= 16 && !/[\r\n]/.test(options.secret), "WORKER_SECRET_MISSING");
  const endpoint = workerEndpoint(options.workerUrl, options.allowLoopback === true);
  requireFixed(contract.publicOrigin.startsWith("https://") || (options.allowLoopback === true
    && /^http:\/\/127\.0\.0\.1:\d+$/.test(contract.publicOrigin)), "PUBLIC_ORIGIN_INVALID");
  const nonce = randomUUID();
  const recordPublicCacheControl = proof => {
    if (proof.cacheControlForm === "exact") receipt.publicCacheControlExactVerified++;
    else if (proof.cacheControlForm === "cdn-hit-immutable-omitted") receipt.publicCacheControlHitImmutableOmittedVerified++;
    else throw new FixedAssetError("PUBLIC_CACHE_PROOF_INVALID");
  };
  const putOnce = async file => {
    receipt.putAttempts++;
    try { await workerPut(file, endpoint, options.secret); }
    catch (error) {
      // The existing endpoint cannot prove a failed request never reached R2.
      // Preserve ambiguity; the next dispatch must read back, never blindly PUT.
      receipt.unknownWriteOutcomes++; receipt.failedPublicPath = file.relativePath; throw error;
    }
    receipt.remoteWrites++;
  };
  const alreadyPublished = await verifyRemoteFile(manifest, contract, `manifest-before-${nonce}`, true);
  await runBounded(files, FIXED_R16_FILE_CONCURRENCY, async file => {
    const exists = await verifyRemoteFile(file, contract, `before-${nonce}`, true);
    let representation = exists;
    if (exists) receipt.existingVerified++;
    else {
      requireFixed(!alreadyPublished, "PUBLISHED_RELEASE_INCOMPLETE");
      // No blind retry after an ambiguous PUT. A new reviewed dispatch can read
      // back and retain the exact object; mismatching existing bytes always fail.
      await putOnce(file); receipt.uploaded++;
      representation = await verifyRemoteFile(file, contract, `after-${nonce}`);
    }
    if (representation.representation === "gzip-wire") receipt.publicGzipWireVerified++;
    if (representation.representation === "gzip-decoded") receipt.publicGzipDecodedVerified++;
    recordPublicCacheControl(representation);
    receipt.verified++;
    if (receipt.verified % 500 === 0 || receipt.verified === files.length) options.onProgress?.({ phase: "public-verify", files: receipt.verified, total: files.length });
  });
  requireFixed(receipt.verified === files.length, "ASSET_VERIFICATION_INCOMPLETE");
  if (!alreadyPublished) {
    await putOnce(manifest); receipt.manifestPublished = true;
  }
  const manifestProof = await verifyRemoteFile(manifest, contract, `manifest-after-${nonce}`);
  recordPublicCacheControl(manifestProof); receipt.manifestCacheControlForm = manifestProof.cacheControlForm;
  receipt.manifestVerified = true; receipt.passed = true; return receipt;
}

async function githubJson(url, token) {
  const response = await request(new URL(url), { headers: { Authorization: `Bearer ${token}`,
    Accept: "application/vnd.github+json", "User-Agent": "mir2-fixed-r16-publisher", "X-GitHub-Api-Version": "2022-11-28" } });
  try {
    requireFixed(response.statusCode === 200, "GITHUB_HTTP_STATUS");
    let size = 0; const chunks = [];
    for await (const chunk of response) { size += chunk.length; requireFixed(size <= 1024 * 1024, "GITHUB_RESPONSE_LIMIT"); chunks.push(chunk); }
    try { return JSON.parse(Buffer.concat(chunks).toString()); } catch { throw new FixedAssetError("GITHUB_JSON_INVALID"); }
  } finally { response.destroy(); }
}

export function validateReleaseMetadata(release, input, contract = FIXED_R16) {
  requireFixed(release.draft === true && release.tag_name === input.releaseTag
    && release.target_commitish === input.sourceSha, "DRAFT_SOURCE_MISMATCH");
  const matches = release.assets?.filter(asset => asset.name === contract.bundleName) ?? [];
  requireFixed(matches.length === 1 && Number.isSafeInteger(matches[0].id)
    && matches[0].size === contract.bundleBytes && matches[0].state === "uploaded", "RELEASE_ASSET_MISMATCH");
  return matches[0];
}

export async function resolveDraftRelease(input, token, contract = FIXED_R16, readJson = githubJson) {
  const base = `https://api.github.com/repos/${contract.repository}`;
  const matches = []; let complete = false;
  // GitHub's /releases/tags/:tag endpoint is published-only. Drafts require an
  // authenticated list, with bounded pagination and an exact unique tag match.
  for (let page = 1; page <= 10; page++) {
    const releases = await readJson(`${base}/releases?per_page=50&page=${page}`, token);
    requireFixed(Array.isArray(releases) && releases.length <= 50, "DRAFT_LIST_INVALID");
    for (const release of releases) if (release?.tag_name === input.releaseTag) matches.push(release);
    requireFixed(matches.length <= 1, "DRAFT_TAG_NOT_UNIQUE");
    if (releases.length < 50) { complete = true; break; }
  }
  requireFixed(complete, "DRAFT_LIST_LIMIT");
  requireFixed(matches.length === 1, "DRAFT_NOT_VISIBLE_OR_NOT_FOUND");
  const id = matches[0].id;
  requireFixed(Number.isSafeInteger(id) && id > 0, "DRAFT_ID_INVALID");
  // Re-read by ID: do not trust URLs supplied by list data, or stale draft/source
  // metadata if another release operation changed it during the lookup.
  const release = await readJson(`${base}/releases/${id}`, token);
  requireFixed(release?.id === id, "DRAFT_ID_MISMATCH");
  validateReleaseMetadata(release, input, contract);
  return release;
}

export async function downloadDraftBundle(input, destination, token, contract = FIXED_R16) {
  requireFixed(typeof token === "string" && token.length >= 10, "GITHUB_TOKEN_MISSING");
  const base = `https://api.github.com/repos/${contract.repository}`;
  const release = await resolveDraftRelease(input, token, contract);
  const asset = validateReleaseMetadata(release, input, contract);
  let url = new URL(`${base}/releases/assets/${asset.id}`);
  for (let redirects = 0; redirects <= 3; redirects++) {
    const response = await request(url, { headers: { Accept: "application/octet-stream", "Accept-Encoding": "identity",
      "User-Agent": "mir2-fixed-r16-publisher", ...(url.hostname === "api.github.com" ? { Authorization: `Bearer ${token}` } : {}) } });
    if ([301, 302, 303, 307, 308].includes(response.statusCode)) {
      const location = response.headers.location; response.destroy();
      requireFixed(typeof location === "string" && redirects < 3, "GITHUB_REDIRECT_INVALID");
      url = new URL(location, url);
      requireFixed(url.protocol === "https:" && !url.username && !url.password
        && ["release-assets.githubusercontent.com", "objects.githubusercontent.com"].includes(url.hostname), "GITHUB_REDIRECT_INVALID");
      continue;
    }
    try {
      requireFixed(response.statusCode === 200 && !response.headers["content-encoding"], "GITHUB_ASSET_HTTP_STATUS");
      let size = 0;
      const limit = new Transform({ transform(chunk, _encoding, done) {
        size += chunk.length;
        if (size > contract.bundleBytes) done(new FixedAssetError("BUNDLE_DOWNLOAD_LIMIT")); else done(null, chunk);
      } });
      await pipeline(response, limit, createWriteStream(destination, { flags: "wx", mode: 0o600 }));
      const digest = await hashFile(destination, contract.bundleBytes);
      requireFixed(digest.sha256 === contract.bundleSha256, "BUNDLE_HASH_MISMATCH");
      return { releaseId: release.id, assetId: asset.id, draft: true, sourceSha: input.sourceSha };
    } finally { response.destroy(); }
  }
  throw new FixedAssetError("GITHUB_REDIRECT_INVALID");
}
