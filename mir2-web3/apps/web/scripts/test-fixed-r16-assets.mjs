import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { Readable, Writable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { gzipSync, gunzipSync } from "node:zlib";
import { FIXED_R16, FIXED_R16_FILE_CONCURRENCY, METADATA_NAMES, extractFixedBundle, normalizeGzipHeader, publishValidated,
  resolveDraftRelease, runBounded, safeRelative, sha256, validateDispatch, validateExtracted, validateReleaseMetadata } from "./lib/fixed-r16-assets.mjs";

let passed = 0;
const roots = [];
async function test(name, operation) { await operation(); console.log(`ok ${++passed} - ${name}`); }
const json = value => Buffer.from(JSON.stringify(value) + "\n");
const sourceSha = "a".repeat(40);
const execute = promisify(execFile);
const dispatch = () => ({ repository: FIXED_R16.repository, sourceSha, githubSha: sourceSha,
  releaseTag: "r16-fixed-assets-fixture", bundleSha256: FIXED_R16.bundleSha256,
  payloadSha256: FIXED_R16.payloadSha256, manifestSha256: FIXED_R16.manifestSha256 });
const code = expected => error => error.code === expected;

function header(name, size, options = {}) {
  const block = Buffer.alloc(512), octal = (value, length) => value.toString(8).padStart(length - 1, "0") + "\0";
  let suffix = name, prefix = "";
  if (Buffer.byteLength(name) > 100) { const split = name.lastIndexOf("/"); prefix = name.slice(0, split); suffix = name.slice(split + 1); }
  block.write(suffix, 0, 100); block.write(octal(0o644, 8), 100); block.write(octal(0, 8), 108);
  block.write(octal(0, 8), 116); block.write(octal(size, 12), 124); block.write(octal(0, 12), 136);
  block.fill(32, 148, 156); block.write(options.type ?? "0", 156);
  block.write("ustar\0", 257); block.write("00", 263); block.write(prefix, 345, 155);
  if (options.link) block.write(options.link, 157, 100);
  block.write(block.reduce((sum, byte) => sum + byte, 0).toString(8).padStart(6, "0") + "\0 ", 148);
  if (options.badChecksum) block[0] ^= 1;
  return block;
}

async function fixture(origin = "https://assets.example.test", extraImages = 0) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-fixed-test-")); roots.push(directory);
  const bodies = new Map([
    ["bevy-runtime/v/bevy-fixture/pkg-webgpu/mir2_bevy_runtime_bg.wasm", Buffer.from("real gzip fixture ".repeat(300))],
    ["original-ui/Libraries/Prguse/0000.png", Buffer.from("fixture-image")],
    ["original-ui/fonts/NotoSans-Regular.ttf", Buffer.from("fixture-font")],
  ]);
  for (let index = 0; index < extraImages; index++) bodies.set(`original-ui/Libraries/Prguse/extra-${String(index).padStart(4, "0")}.png`, Buffer.from(`fixture-image-${index}`));
  const files = [...bodies].sort(([a], [b]) => a < b ? -1 : 1).map(([p, body]) => ({ p, s: body.length, h: sha256(body) }));
  const payloadSha256 = sha256(JSON.stringify(files));
  const contract = { ...FIXED_R16, version: "fixture-r16", prefix: "mir2/v/fixture-r16", publicOrigin: origin,
    fileCount: files.length, payloadSha256, fontFiles: 1, finiteFiles: 2 + extraImages, runtimeVersion: "bevy-fixture",
    maxExpandedBytes: 1024 * 1024, maxFileBytes: 128 * 1024 };
  const assetBaseUrl = `${origin}/${contract.prefix}`;
  const planFiles = files.map(pin => ({ path: `/${pin.p}`, relativePath: pin.p, stagePath: `payload/${pin.p}`,
    objectKey: `${contract.prefix}/${pin.p}`, size: pin.s, sha256: pin.h,
    contentType: pin.p.endsWith("wasm") ? "application/wasm" : pin.p.endsWith("png") ? "image/png" : "font/ttf",
    cacheControl: "public, max-age=31536000, immutable", sources: ["fixture"] }));
  const wire = new Map(bodies);
  for (const file of planFiles) if (file.relativePath.endsWith("wasm")) {
    const bytes = gzipSync(bodies.get(file.relativePath), { level: 9, mtime: 0 }); bytes[9] = 10;
    wire.set(file.relativePath, bytes);
    Object.assign(file, { contentEncoding: "gzip", encodedSize: bytes.length, encodedSha256: sha256(bytes) });
  }
  Object.assign(contract, { payloadBytes: files.reduce((sum, file) => sum + file.s, 0),
    wireBytes: [...wire.values()].reduce((sum, body) => sum + body.length, 0), gzipFiles: 1 });
  const base = { schemaVersion: 1, publishReleaseManifest: false, version: contract.version,
    objectPrefix: contract.prefix, assetBaseUrl, payloadSha256 };
  const plan = { ...base, kind: "mir2-r16-finite-upload-plan", files: planFiles };
  const release = { schemaVersion: 1, kind: "mir2-remote-asset-release", version: contract.version,
    objectPrefix: contract.prefix, assetBaseUrl, pathMode: "relative", fallbackObjectPrefix: contract.fallbackPrefixes[0],
    fallbackObjectPrefixes: contract.fallbackPrefixes, browserFallbackBaseUrls: contract.fallbackPrefixes.map(prefix => `${origin}/${prefix}`),
    fullCrystalPack: { verified: false }, candidateCoverage: { allMapsVerified: false, payloadSha256,
      localCandidateFilesVerified: files.length, finiteFiles: contract.finiteFiles },
    bevyRuntime: { version: contract.runtimeVersion }, files: planFiles.map(file => ({ p: file.relativePath,
      s: file.size, h: file.sha256, c: file.contentType,
      ...(file.contentEncoding ? { e: "gzip", es: file.encodedSize, eh: file.encodedSha256 } : {}) })) };
  const releaseBytes = json(release); contract.manifestBytes = releaseBytes.length; contract.manifestSha256 = sha256(releaseBytes);
  const manifest = { ...base, kind: "mir2-r16-manifest-last-upload-plan", files: [{ path: "/remote-asset-release.json",
    relativePath: "remote-asset-release.json", stagePath: "release/remote-asset-release.json",
    objectKey: `${contract.prefix}/remote-asset-release.json`, size: releaseBytes.length, sha256: sha256(releaseBytes),
    contentType: "application/json; charset=utf-8", cacheControl: "public, max-age=60, stale-while-revalidate=300" }] };
  const entries = new Map([
    ["asset-upload-plan.portable.json", json(plan)], ["manifest-upload-plan.portable.json", json(manifest)],
    ["payload-pins.json", json({ schemaVersion: 1, payloadSha256, files })],
    ["font-pins.json", json({ schemaVersion: 1, files: files.filter(file => file.p.includes("/fonts/")) })],
    ["release/remote-asset-release.json", releaseBytes], ["verification-receipt.json", json({ passed: true })],
    // A downloaded verifier must remain inert data, even for a valid bundle.
    ["verify-upload-bundle.mjs", Buffer.from("throw new Error('must never execute');")],
    ...[...bodies].map(([p, bytes]) => [`payload/${p}`, bytes]),
  ]);
  const result = { directory, contract, entries, plan, manifest, bodies, wire, release, next: 0 };
  result.archive = async (entryList = [...entries].sort(([a], [b]) => a < b ? -1 : 1), mutateHeader) => {
    const chunks = [];
    for (const [name, bytes] of entryList) chunks.push(mutateHeader?.(name, bytes) ?? header(name, bytes.length), bytes,
      Buffer.alloc((512 - bytes.length % 512) % 512));
    chunks.push(Buffer.alloc(1024));
    const bytes = gzipSync(Buffer.concat(chunks), { level: 9, mtime: 0 });
    contract.bundleBytes = bytes.length; contract.bundleSha256 = sha256(bytes);
    const bundle = path.join(directory, `bundle-${result.next++}.tar.gz`); await fs.writeFile(bundle, bytes);
    return bundle;
  };
  result.unpack = async (bundle = null) => {
    bundle ??= await result.archive();
    const root = path.join(directory, `extracted-${result.next++}`);
    return { root, extracted: await extractFixedBundle(bundle, root, contract) };
  };
  return result;
}

async function localWorker() {
  const storage = new Map(), events = [];
  const state = { rejectKey: undefined, storage, events, decodedGzip: false, assetReplyHeaders: undefined };
  const server = http.createServer(async (request, response) => {
    try {
      const url = new URL(request.url, "http://127.0.0.1");
      if (request.method === "PUT" && url.pathname === "/upload") {
        assert.equal(request.headers.authorization, "Bearer fixture-upload-secret");
        assert.equal(request.headers["content-encoding"], undefined, "opaque gzip transport must not be decoded upstream");
        const key = url.searchParams.get("key"); events.push({ type: "put", key });
        if (key === state.rejectKey) { response.writeHead(503); response.end(); return; }
        assert(!storage.has(key), "existing objects must never be overwritten");
        const chunks = []; for await (const chunk of request) chunks.push(chunk);
        storage.set(key, { bytes: Buffer.concat(chunks), headers: { "content-type": request.headers["content-type"],
          "cache-control": request.headers["cache-control"],
          ...(request.headers["x-mir2-content-encoding"] ? { "content-encoding": request.headers["x-mir2-content-encoding"] } : {}) } });
        response.writeHead(200, { "content-type": "application/json" }); response.end(JSON.stringify({ ok: true, key })); return;
      }
      const key = decodeURIComponent(url.pathname.slice(1)); events.push({ type: "get", key });
      await state.beforeGet?.(key);
      const item = storage.get(key);
      if (!item) { response.writeHead(404); response.end(); return; }
      let bytes = item.bytes; const headers = { ...item.headers };
      if (state.decodedGzip && headers["content-encoding"] === "gzip") {
        bytes = gunzipSync(bytes); delete headers["content-encoding"]; headers["x-mir2-storage-content-encoding"] = "gzip";
      }
      if (!key.endsWith("/remote-asset-release.json") && state.assetReplyHeaders) Object.assign(headers, state.assetReplyHeaders);
      response.writeHead(200, { ...headers, "content-length": bytes.length }); response.end(bytes);
    } catch { response.writeHead(500); response.end(); }
  });
  await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
  return { ...state, state, origin: `http://127.0.0.1:${server.address().port}`, close: () => new Promise(resolve => server.close(resolve)) };
}

try {
  await test("exact hashes, same-repository draft and frozen dispatch source are mandatory", async () => {
    validateDispatch(dispatch());
    for (const field of ["bundleSha256", "payloadSha256", "manifestSha256"]) assert.throws(() => validateDispatch({ ...dispatch(), [field]: "0".repeat(64) }), code("FIXED_HASH_MISMATCH"));
    assert.throws(() => validateDispatch({ ...dispatch(), sourceSha: "b".repeat(40) }), code("SOURCE_SHA_MISMATCH"));
    assert.throws(() => validateDispatch({ ...dispatch(), repository: "other/repo" }), code("REPOSITORY_MISMATCH"));
    for (const releaseTag of ["", "../tag", "a//tag", "tag?secret=bad"]) assert.throws(() => validateDispatch({ ...dispatch(), releaseTag }), code("RELEASE_TAG_INVALID"));
    const release = { draft: true, tag_name: dispatch().releaseTag, target_commitish: sourceSha,
      assets: [{ id: 42, state: "uploaded", name: FIXED_R16.bundleName, size: FIXED_R16.bundleBytes }] };
    assert.equal(validateReleaseMetadata(release, dispatch()).id, 42);
    for (const changed of [{ draft: false }, { target_commitish: "main" }, { tag_name: "other" }])
      assert.throws(() => validateReleaseMetadata({ ...release, ...changed }, dispatch()), code("DRAFT_SOURCE_MISMATCH"));
    assert.throws(() => validateReleaseMetadata({ ...release, assets: [...release.assets, ...release.assets] }, dispatch()), code("RELEASE_ASSET_MISMATCH"));
  });

  await test("draft lookup uses a bounded same-repository list and exact ID readback instead of published tag API", async () => {
    const input = dispatch(), base = `https://api.github.com/repos/${FIXED_R16.repository}`;
    const release = { id: 42, draft: true, tag_name: input.releaseTag, target_commitish: input.sourceSha,
      assets: [{ id: 9, name: FIXED_R16.bundleName, size: FIXED_R16.bundleBytes, state: "uploaded" }] };
    const requests = [];
    const found = await resolveDraftRelease(input, "fixture-token", FIXED_R16, async url => {
      requests.push(url);
      if (url === `${base}/releases?per_page=50&page=1`) return [release];
      assert.equal(url, `${base}/releases/42`); return release;
    });
    assert.equal(found.id, 42); assert.equal(requests.length, 2);
    assert(!requests.some(url => url.includes("/releases/tags/")));
    for (const [detail, expected] of [
      [{ ...release, draft: false }, "DRAFT_SOURCE_MISMATCH"],
      [{ ...release, tag_name: "wrong" }, "DRAFT_SOURCE_MISMATCH"],
      [{ ...release, target_commitish: "main" }, "DRAFT_SOURCE_MISMATCH"],
      [{ ...release, id: 43 }, "DRAFT_ID_MISMATCH"],
      [{ ...release, assets: [] }, "RELEASE_ASSET_MISMATCH"],
    ]) await assert.rejects(resolveDraftRelease(input, "fixture-token", FIXED_R16,
      async url => url.includes("?") ? [release] : detail), code(expected));
    await assert.rejects(resolveDraftRelease(input, "fixture-token", FIXED_R16, async () => []), code("DRAFT_NOT_VISIBLE_OR_NOT_FOUND"));
    const fullPage = [release, ...Array.from({ length: 49 }, (_, index) => ({ id: 100 + index, tag_name: `other-${index}` }))];
    await assert.rejects(resolveDraftRelease(input, "fixture-token", FIXED_R16,
      async url => url.endsWith("page=1") ? fullPage : [release]), code("DRAFT_TAG_NOT_UNIQUE"));
    let pages = 0;
    await assert.rejects(resolveDraftRelease(input, "fixture-token", FIXED_R16,
      async () => { pages++; return Array.from({ length: 50 }, (_, index) => ({ id: index, tag_name: "other" })); }), code("DRAFT_LIST_LIMIT"));
    assert.equal(pages, 10);
  });

  await test("safe extraction rejects traversal, links, PAX, duplicates, corrupt headers and expansion", async () => {
    for (const name of ["../escape", "/absolute", "C:/data", "payload/a/../../escape", "payload\\escape", "payload/a%2fb", "payload/.env"])
      assert.throws(() => safeRelative(name), code("UNSAFE_PATH"));
    for (const [malicious, expected, modify] of [
      [["../escape", Buffer.from("x")], "UNSAFE_PATH"],
      [["payload/link", Buffer.from("x")], "TAR_NONREGULAR_ENTRY", { type: "2", link: "../../secret" }],
      [["payload/pax", Buffer.from("x")], "TAR_NONREGULAR_ENTRY", { type: "x" }],
      [["payload/corrupt", Buffer.from("x")], "TAR_CHECKSUM_INVALID", { badChecksum: true }],
    ]) {
      const f = await fixture(), bundle = await f.archive([malicious], () => header(malicious[0], malicious[1].length, modify));
      await assert.rejects(f.unpack(bundle), code(expected));
    }
    const f = await fixture(), first = ["payload/repeated", Buffer.from("same")];
    await assert.rejects(f.unpack(await f.archive([first, first])), code("TAR_DUPLICATE_ENTRY"));
    const overflow = await fixture(); overflow.contract.maxExpandedBytes = 1;
    await assert.rejects(overflow.unpack(), code("TAR_SIZE_LIMIT"));
    const truncated = await fixture(), bundle = await truncated.archive();
    const bytes = await fs.readFile(bundle); await fs.writeFile(bundle, bytes.subarray(0, -8));
    truncated.contract.bundleBytes = bytes.length - 8; truncated.contract.bundleSha256 = sha256(bytes.subarray(0, -8));
    await assert.rejects(truncated.unpack(bundle));
  });

  await test("archive hash is checked before creating the extraction tree", async () => {
    const f = await fixture(), bundle = await f.archive(); f.contract.bundleSha256 = "0".repeat(64);
    const destination = path.join(f.directory, "absent");
    await assert.rejects(extractFixedBundle(bundle, destination, f.contract), code("BUNDLE_HASH_MISMATCH"));
    await assert.rejects(fs.stat(destination), { code: "ENOENT" });
  });

  await test("the actual CLI accepts SHA256 flags and reports local byte rejection without network", async () => {
    const f = await fixture(), bundle = await f.archive();
    const script = path.join(path.dirname(fileURLToPath(import.meta.url)), "publish-fixed-r16-assets.mjs");
    const repo = path.resolve(path.dirname(script), "../../../..");
    const head = (await execute("git", ["rev-parse", "HEAD"], { cwd: repo })).stdout.trim();
    const output = path.join(f.directory, "cli-output");
    await assert.rejects(execute(process.execPath, [script, "--dry-run", "--bundle", bundle, "--output-dir", output,
      "--repository", FIXED_R16.repository, "--source-sha", head, "--github-sha", head,
      "--release-tag", "fixture-review", "--bundle-sha256", FIXED_R16.bundleSha256,
      "--payload-sha256", FIXED_R16.payloadSha256, "--manifest-sha256", FIXED_R16.manifestSha256]), { code: 1 });
    const receipt = JSON.parse(await fs.readFile(path.join(output, "receipt.json")));
    assert.equal(receipt.errorCode, "FILE_SIZE_MISMATCH"); assert.equal(receipt.sourceSha, head);
    assert.equal(receipt.remoteWrites, 0); assert.equal(receipt.phase, "extract");
  });

  await test("production validation accepts the exact tree and never executes the embedded verifier", async () => {
    const f = await fixture(), { root, extracted } = await f.unpack();
    const validated = await validateExtracted(root, extracted, f.contract);
    assert.equal(validated.files.length, 3); assert.equal(validated.review.gzipFiles, 1);
    assert.equal(validated.review.manifestLast, true); assert.equal(validated.review.allMapsVerified, false);
    assert.equal((await fs.readFile(validated.files[0].wirePath))[9], 10);
  });

  await test("gzip header normalization preserves the reviewed Windows bytes on Unix", async () => {
    const raw = Buffer.from("portable representation"), unix = gzipSync(raw, { level: 9 }); unix[9] = 3;
    const chunks = [];
    await pipeline(Readable.from([unix.subarray(0, 4), unix.subarray(4)]), normalizeGzipHeader(10),
      new Writable({ write(chunk, _encoding, done) { chunks.push(chunk); done(); } }));
    const actual = Buffer.concat(chunks), expected = Buffer.from(unix); expected[9] = 10;
    assert.deepEqual(actual, expected); assert.deepEqual(gunzipSync(actual), raw);
  });

  await test("raw tampering, missing files, unsafe stage paths and encoded hash mismatches fail", async () => {
    for (const variant of ["tamper", "missing", "escape", "encoded"]) {
      const f = await fixture(), { root, extracted } = await f.unpack();
      let expected;
      if (variant === "tamper") {
        const file = path.join(root, "payload", ...f.plan.files[1].relativePath.split("/"));
        const bytes = await fs.readFile(file); bytes[0] ^= 1; await fs.writeFile(file, bytes); expected = "PAYLOAD_FILE_HASH_MISMATCH";
      } else if (variant === "missing") {
        await fs.unlink(path.join(root, "payload", ...f.plan.files[1].relativePath.split("/"))); expected = "ENOENT";
      } else {
        if (variant === "escape") { f.plan.files[0].stagePath = "../private.local.json"; expected = "PLAN_FILE_MISMATCH"; }
        else {
          f.plan.files[0].encodedSha256 = "0".repeat(64);
          // The independently pinned manifest catches a plan-only encoding lie.
          expected = "MANIFEST_OVERLAY_MISMATCH";
        }
        await fs.writeFile(path.join(root, "asset-upload-plan.portable.json"), json(f.plan));
      }
      await assert.rejects(validateExtracted(root, extracted, f.contract), error => error.code === expected);
    }
  });

  await test("matching plan and manifest lies still fail actual deterministic gzip verification", async () => {
    const f = await fixture();
    f.plan.files[0].encodedSha256 = "0".repeat(64); f.release.files[0].eh = "0".repeat(64);
    const releaseBytes = json(f.release); f.contract.manifestBytes = releaseBytes.length;
    f.contract.manifestSha256 = sha256(releaseBytes);
    Object.assign(f.manifest.files[0], { size: releaseBytes.length, sha256: sha256(releaseBytes) });
    f.entries.set("asset-upload-plan.portable.json", json(f.plan));
    f.entries.set("release/remote-asset-release.json", releaseBytes);
    f.entries.set("manifest-upload-plan.portable.json", json(f.manifest));
    const { root, extracted } = await f.unpack();
    await assert.rejects(validateExtracted(root, extracted, f.contract), code("ENCODED_FILE_HASH_MISMATCH"));
  });

  await test("dry-run performs zero Worker or public requests and needs no upload credential", async () => {
    const worker = await localWorker();
    try {
      const f = await fixture(worker.origin), { root, extracted } = await f.unpack();
      const validated = await validateExtracted(root, extracted, f.contract);
      const receipt = await publishValidated(validated, { dryRun: true });
      assert.equal(receipt.remoteWrites, 0); assert.equal(worker.events.length, 0);
    } finally { await worker.close(); }
  });

  await test("the real HTTP transport verifies raw and gzip objects before the final manifest PUT", async () => {
    const worker = await localWorker();
    try {
      const f = await fixture(worker.origin), { root, extracted } = await f.unpack();
      const validated = await validateExtracted(root, extracted, f.contract);
      const options = { workerUrl: worker.origin, secret: "fixture-upload-secret", allowLoopback: true };
      const receipt = await publishValidated(validated, options);
      const puts = worker.events.filter(event => event.type === "put"), manifestKey = validated.manifest.objectKey;
      assert.equal(receipt.remoteWrites, 4); assert.equal(receipt.verified, 3); assert.equal(receipt.manifestVerified, true);
      assert.equal(receipt.putAttempts, 4); assert.equal(receipt.unknownWriteOutcomes, 0);
      assert.equal(receipt.publicGzipWireVerified, 1); assert.equal(receipt.publicGzipDecodedVerified, 0);
      assert.equal(receipt.publicCacheControlExactVerified, 4);
      assert.equal(receipt.publicCacheControlHitImmutableOmittedVerified, 0); assert.equal(receipt.manifestCacheControlForm, "exact");
      assert.equal(puts.at(-1).key, manifestKey);
      const manifestIndex = worker.events.findIndex(event => event.type === "put" && event.key === manifestKey);
      for (const file of validated.files) {
        const putIndex = worker.events.findIndex(event => event.type === "put" && event.key === file.objectKey);
        assert(worker.events.slice(putIndex + 1, manifestIndex).some(event => event.type === "get" && event.key === file.objectKey));
      }
      const second = await publishValidated(validated, options);
      assert.equal(second.remoteWrites, 0); assert.equal(second.existingVerified, 3);
      assert.equal(worker.events.filter(event => event.type === "put").length, 4, "a completed rerun only reads existing bytes");
    } finally { await worker.close(); }
  });

  await test("the real publisher starts exactly32 file operations and retains full verification before manifest publication", async () => {
    assert.equal(FIXED_R16_FILE_CONCURRENCY, 32);
    const worker = await localWorker();
    let release;
    const gate = new Promise(resolve => { release = resolve; });
    try {
      const f = await fixture(worker.origin, 62), { root, extracted } = await f.unpack();
      const validated = await validateExtracted(root, extracted, f.contract);
      assert.equal(validated.files.length, 65);
      let waiting = 0, notify;
      const allStarted = new Promise(resolve => { notify = resolve; });
      worker.state.beforeGet = async key => {
        if (key === validated.manifest.objectKey || worker.storage.has(key)) return;
        waiting++;
        if (waiting === 32) notify();
        await gate;
      };
      const pending = publishValidated(validated, { workerUrl: worker.origin, secret: "fixture-upload-secret", allowLoopback: true });
      let timer;
      try {
        await Promise.race([allStarted, new Promise((_, reject) => { timer = setTimeout(() => reject(Error("32 file operations did not reach the server")), 5000); })]);
        await new Promise(resolve => setImmediate(resolve));
        assert.equal(waiting, 32, "the fixed bound is not exceeded while all file operations are blocked");
        assert.equal(worker.events.filter(event => event.type === "get" && event.key !== validated.manifest.objectKey).length, 32);
        assert.equal(worker.events.filter(event => event.type === "put").length, 0);
      } finally { clearTimeout(timer); release(); }
      const receipt = await pending;
      assert.equal(receipt.verified, 65); assert.equal(receipt.unknownWriteOutcomes, 0);
      assert.equal(receipt.remoteWrites, 66); assert.equal(receipt.putAttempts, 66);
      const puts = worker.events.filter(event => event.type === "put");
      assert.equal(puts.at(-1).key, validated.manifest.objectKey);
      const manifestIndex = worker.events.findIndex(event => event.type === "put" && event.key === validated.manifest.objectKey);
      for (const file of validated.files) {
        const putIndex = worker.events.findIndex(event => event.type === "put" && event.key === file.objectKey);
        assert.equal(puts.filter(event => event.key === file.objectKey).length, 1);
        assert(worker.events.slice(0, putIndex).some(event => event.type === "get" && event.key === file.objectKey));
        assert(worker.events.slice(putIndex + 1, manifestIndex).some(event => event.type === "get" && event.key === file.objectKey));
      }
    } finally { release?.(); await worker.close(); }
  });

  await test("a32-file batch stops new dispatch after the first failure and waits for every in-flight operation", async () => {
    let release, started = 0, completed = 0, settled = false;
    const gate = new Promise(resolve => { release = resolve; });
    const failure = Error("fixture-first-failure");
    const pending = runBounded(Array.from({ length: 65 }, (_, index) => index), FIXED_R16_FILE_CONCURRENCY, async index => {
      started++;
      if (index === 0) throw failure;
      await gate; completed++;
    }).finally(() => { settled = true; });
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(started, 32); assert.equal(completed, 0); assert.equal(settled, false);
    release();
    await assert.rejects(pending, error => error === failure);
    assert.equal(started, 32, "none of the remaining33 files were scheduled");
    assert.equal(completed, 31, "failure is returned only after the other31 operations settle");
  });

  await test("the existing public Worker's decoded gzip semantics are explicit in the receipt", async () => {
    const worker = await localWorker(); worker.state.decodedGzip = true;
    try {
      const f = await fixture(worker.origin), { root, extracted } = await f.unpack();
      const validated = await validateExtracted(root, extracted, f.contract);
      const receipt = await publishValidated(validated, { workerUrl: worker.origin, secret: "fixture-upload-secret", allowLoopback: true });
      assert.equal(receipt.verified, 3); assert.equal(receipt.manifestVerified, true);
      assert.equal(receipt.publicGzipDecodedVerified, 1); assert.equal(receipt.publicGzipWireVerified, 0);
      assert.equal(validated.review.gzipHashesVerifiedLocally, true);
    } finally { await worker.close(); }
  });

  await test("only the proven CDN HIT immutable omission is accepted and counted separately", async () => {
    const worker = await localWorker();
    worker.state.assetReplyHeaders = { "x-mir2-edge-cache": "HIT", "cache-control": "public, max-age=31536000" };
    try {
      const f = await fixture(worker.origin), { root, extracted } = await f.unpack();
      const validated = await validateExtracted(root, extracted, f.contract);
      const options = { workerUrl: worker.origin, secret: "fixture-upload-secret", allowLoopback: true };
      for (const run of [0, 1]) {
        const receipt = await publishValidated(validated, options);
        assert.equal(receipt.manifestVerified, true); assert.equal(receipt.manifestCacheControlForm, "exact");
        assert.equal(receipt.publicCacheControlExactVerified, 1);
        assert.equal(receipt.publicCacheControlHitImmutableOmittedVerified, 3);
        assert.equal(receipt.publicGzipWireVerified, 1);
        assert.equal(receipt.remoteWrites, run === 0 ? 4 : 0);
      }
    } finally { await worker.close(); }
  });

  await test("MISS, unknown HIT, changed cache policy and damaged HIT content still fail without PUT", async () => {
    const policies = [
      { "x-mir2-edge-cache": "MISS", "cache-control": "public, max-age=31536000" },
      { "x-mir2-edge-cache": "BYPASS", "cache-control": "public, max-age=31536000" },
      { "cache-control": "public, max-age=31536000" },
      { "x-mir2-edge-cache": "hit", "cache-control": "public, max-age=31536000" },
      { "x-mir2-edge-cache": "HIT, MISS", "cache-control": "public, max-age=31536000" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "public, max-age=31535999" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "max-age=31536000" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "private, max-age=31536000" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "public, private, max-age=31536000" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "public, no-store, max-age=31536000" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "public, max-age=31536000", "x-mir2-fallback-object-key": "mir2/v/old/same.png" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "public, max-age=31536000", "content-type": "text/plain" },
      { "x-mir2-edge-cache": "HIT", "cache-control": "public, max-age=31536000", "content-encoding": "br" },
    ];
    const worker = await localWorker();
    try {
      const f = await fixture(worker.origin), { root, extracted } = await f.unpack();
      const validated = await validateExtracted(root, extracted, f.contract);
      for (const file of validated.files) worker.storage.set(file.objectKey, { bytes: await fs.readFile(file.wirePath),
        headers: { "content-type": file.contentType, "cache-control": file.cacheControl,
          ...(file.contentEncoding ? { "content-encoding": file.contentEncoding } : {}) } });
      const options = { workerUrl: worker.origin, secret: "fixture-upload-secret", allowLoopback: true };
      for (const headers of policies) {
        worker.state.assetReplyHeaders = headers;
        await assert.rejects(publishValidated(validated, options), code("PUBLIC_HEADERS_MISMATCH"));
      }
      worker.state.assetReplyHeaders = { "x-mir2-edge-cache": "HIT", "cache-control": "public, max-age=31536000" };
      const item = worker.storage.get(validated.files[1].objectKey); item.bytes[0] ^= 1;
      await assert.rejects(publishValidated(validated, options), code("PUBLIC_BYTES_MISMATCH"));
      assert(!worker.events.some(event => event.type === "put"));
    } finally { await worker.close(); }
  });

  await test("conflicting existing bytes and upload failures never publish a manifest or overwrite", async () => {
    for (const variant of ["conflict", "gzip-representation", "failure", "published-incomplete"]) {
      const worker = await localWorker();
      try {
        const f = await fixture(worker.origin), { root, extracted } = await f.unpack();
        const validated = await validateExtracted(root, extracted, f.contract), file = validated.files[1];
        let expected;
        if (variant === "conflict") {
          worker.storage.set(file.objectKey, { bytes: Buffer.alloc(file.size, 1), headers: {
            "content-type": file.contentType, "cache-control": file.cacheControl } }); expected = "PUBLIC_BYTES_MISMATCH";
        } else if (variant === "gzip-representation") {
          const compressed = validated.files[0], bytes = await fs.readFile(compressed.wirePath); bytes[9] = 3;
          worker.storage.set(compressed.objectKey, { bytes, headers: { "content-type": compressed.contentType,
            "cache-control": compressed.cacheControl, "content-encoding": "gzip" } }); expected = "PUBLIC_BYTES_MISMATCH";
        } else if (variant === "failure") { worker.state.rejectKey = file.objectKey; expected = "WORKER_HTTP_STATUS"; }
        else {
          worker.storage.set(validated.manifest.objectKey, { bytes: await fs.readFile(validated.manifest.localPath), headers: {
            "content-type": validated.manifest.contentType, "cache-control": validated.manifest.cacheControl } }); expected = "PUBLISHED_RELEASE_INCOMPLETE";
        }
        const receipt = {};
        await assert.rejects(publishValidated(validated, { workerUrl: worker.origin, secret: "fixture-upload-secret",
          allowLoopback: true, receipt }), code(expected));
        assert(!worker.events.some(event => event.type === "put" && event.key === validated.manifest.objectKey));
        if (variant === "conflict") assert(!worker.events.some(event => event.type === "put" && event.key === file.objectKey));
        if (variant === "published-incomplete") assert(!worker.events.some(event => event.type === "put"));
        if (variant === "failure") { assert.equal(receipt.unknownWriteOutcomes, 1); assert(receipt.putAttempts >= 1); }
      } finally { await worker.close(); }
    }
  });

  await test("fixed workflow excludes every legacy operation even when just one fixed input is supplied", async () => {
    const filename = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../../../.github/workflows/web-assets-r2-release.yml");
    const workflow = (await fs.readFile(filename, "utf8")).replaceAll("\r\n", "\n");
    const job = name => workflow.split(`\n  ${name}:\n`)[1]?.split(/\n  [a-z][a-z0-9-]*:\n/)[0];
    const condition = name => {
      const expression = job(name)?.match(/^    if: \$\{\{ (.+) \}\}$/m)?.[1]; assert(expression);
      return new Function("github", "inputs", `return (${expression});`);
    };
    const defaults = { native_delivery_operation: "off", fixed_release_tag: "", fixed_bundle_sha256: "",
      fixed_payload_sha256: "", fixed_manifest_sha256: "", fixed_source_sha: "" };
    const github = { event_name: "workflow_dispatch" };
    assert.equal(condition("build-web-assets")(github, defaults), true);
    for (const fixed of Object.keys(defaults).filter(key => key.startsWith("fixed_"))) {
      const inputs = { ...defaults, [fixed]: "supplied" };
      assert.equal(condition("publish-fixed-r16-assets")(github, inputs), true);
      for (const [name, operation] of [["build-web-assets", "off"], ["native-download-authority-probe", "probe"],
        ["native-download-tooling-check", "verify"], ["native-download-stage", "stage"], ["native-download-promote", "promote"]])
        assert.equal(condition(name)(github, { ...inputs, native_delivery_operation: operation }), false);
    }
    const fixed = job("publish-fixed-r16-assets");
    assert.match(workflow.split("\njobs:\n")[0], /\npermissions:\n  contents: read\n/);
    assert.match(fixed, /\n    permissions:\n      contents: write\n/);
    for (const name of [...workflow.matchAll(/^  ([a-z][a-z0-9-]+):$/gm)].map(match => match[1])) {
      if (name !== "publish-fixed-r16-assets") assert.doesNotMatch(job(name) ?? "", /contents: write/,
        "draft visibility permission belongs only to the fixed job");
    }
    assert.match(fixed, /ref: \$\{\{ github\.sha \}\}/);
    assert.match(fixed, /FIXED_SOURCE_SHA.*==.*GITHUB_SHA/);
    assert.match(fixed, /persist-credentials: false/); assert.match(fixed, /node-version: 22\.18\.0/);
    assert.doesNotMatch(fixed, /(?:^|\n)\s+(?:run:\s*)?(?:npm (?:install|ci|run build)|cargo |(?:npx )?(?:wrangler|vercel) )/);
    assert.match(fixed, /cancel-in-progress: false/);
  });
} finally {
  // Only exact directories created by this test process are removed.
  const tempRoot = await fs.realpath(os.tmpdir());
  for (const directory of roots) {
    const resolved = await fs.realpath(directory);
    assert.equal(path.dirname(resolved), tempRoot); assert(path.basename(resolved).startsWith("mir2-fixed-test-"));
    await fs.rm(resolved, { recursive: true, force: true });
  }
}
console.log(JSON.stringify({ passed: true, tests: passed, productionWrites: 0 }));
