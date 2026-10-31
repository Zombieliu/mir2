import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { setImmediate as turn } from "node:timers/promises";
import { createDownloadHandler } from "../src/reader.mjs";

const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const nativeSha = (hex) => Uint8Array.from(Buffer.from(hex, "hex")).buffer;
const bytes = (text) => new TextEncoder().encode(text);

function fixture() {
  const data = [bytes("MZ0123456789ABC"), bytes("defghijklmnopq"), bytes("XYZ")];
  const complete = Buffer.concat(data);
  const digest = sha(complete);
  const prefix = `private/installers/${digest}`;
  const objects = new Map();
  let offset = 0;
  const parts = data.map((data, index) => {
    const part = { offset, bytes: data.length, sha256: sha(data), key: `${prefix}/part-${index}.bin` };
    offset += data.length;
    objects.set(part.key, { data, expected: part });
    return part;
  });
  const files = ["release.json", "release.p7s", "SHA256SUMS.txt", "README.zh-CN.txt"].map((name) => {
    const data = bytes(name === "README.zh-CN.txt" ? "直接下载完整安装包。\n" : `${name}: verified fixture\n`);
    const file = { name, bytes: data.length, sha256: sha(data), key: `${prefix}/${name}`,
      contentType: name.endsWith("json") ? "application/json" : "application/octet-stream" };
    objects.set(file.key, { data, expected: file });
    return file;
  });
  const release = { schema: 1, sha256: digest, sourceInstaller: digest, bytes: complete.length,
    publicBase: `https://assets.mir2.obelisk.build/client-updates/installers/${digest}/`,
    lastModified: "Fri, 30 Oct 2026 08:00:00 GMT",
    allowedHosts: ["assets.mir2.obelisk.build", "mir2-full-installer-downloads.zombiesliu.workers.dev"],
    parts, files };
  return { release, objects, complete };
}

async function collect(body) {
  const reader = body.getReader();
  const chunks = [];
  let length = 0;
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      chunks.push(value);
      length += value.length;
    }
  } finally { reader.releaseLock(); }
  const output = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { output.set(chunk, offset); offset += chunk.length; }
  return output;
}

class MemoryCache {
  entries = new Map();
  matches = [];
  puts = [];
  events = [];
  async match(request) {
    this.matches.push(request);
    if (this.matchError) throw new Error("Cache lookup failed");
    const value = this.entries.get(request.url);
    return value ? new Response(value.data, { status: value.status ?? 200, headers: value.headers }) : undefined;
  }
  async put(request, response) {
    this.puts.push({ request, response });
    assert.equal(request.method, "GET");
    assert.equal(response.status, 200);
    assert.ok(Number(response.headers.get("Content-Length")) <= 64 * 1024 * 1024);
    if (this.putError) throw new Error("Cache write failed");
    if (this.gate) await this.gate;
    const data = await collect(response.body);
    this.entries.set(request.url, { data, headers: new Headers(response.headers) });
    this.events.push(`cache-complete:${request.url}`);
  }
}

function context() {
  return { tasks: [], waitUntil(task) { this.tasks.push(task); }, async settle() { await Promise.all(this.tasks); } };
}

function storage(release, objects, events = []) {
  const state = { gets: [], reads: [], cancels: [], active: 0, maxActive: 0, arrayBufferCalls: 0, overrides: new Map() };
  state.get = async (key) => {
    state.gets.push(key);
    events.push(`get:${key}`);
    if (state.getError) throw new Error("R2 read failed");
    const source = objects.get(key);
    if (!source) return null;
    state.active += 1;
    state.maxActive = Math.max(state.maxActive, state.active);
    let position = 0;
    let finished = false;
    const finish = () => { if (!finished) { finished = true; state.active -= 1; } };
    const data = source.actualData ?? source.data;
    const body = new ReadableStream({
      pull(controller) {
        state.reads.push(key);
        if (position === data.length) {
          finish();
          events.push(`end:${key}`);
          controller.close();
          return;
        }
        const end = Math.min(position + (source.chunkSize ?? 3), data.length);
        controller.enqueue(data.subarray(position, end));
        position = end;
      },
      cancel(reason) { state.cancels.push({ key, reason }); finish(); },
    }, { highWaterMark: 0 });
    body.tee = () => { throw new Error("Installer parts must not use an unbounded tee"); };
    return {
      size: source.expected.bytes,
      customMetadata: { sha256: source.expected.sha256, sourceInstaller: release.sha256 },
      checksums: { sha256: nativeSha(source.expected.sha256) },
      httpMetadata: {}, body,
      arrayBuffer() { state.arrayBufferCalls += 1; throw new Error("Whole-part buffering is forbidden"); },
      ...state.overrides.get(key),
    };
  };
  return state;
}

function setup(input = fixture()) {
  const cache = new MemoryCache();
  const ctx = context();
  const bucket = storage(input.release, input.objects, cache.events);
  const handler = createDownloadHandler(input.release);
  const request = (headers = {}, method = "GET", name = "Mir2Setup.exe", origin) =>
    new Request((origin ? input.release.publicBase.replace("https://assets.mir2.obelisk.build", origin) : input.release.publicBase) + name,
      { method, headers });
  const run = (headers, method, name, origin) => handler(request(headers, method, name, origin), { MIR2_ASSETS: bucket }, ctx, cache);
  return { ...input, cache, ctx, bucket, handler, request, run };
}

test("full installer preserves raw MZ bytes and only caches complete independent parts", async () => {
  const f = setup();
  const response = await f.run();
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("ETag"), `"${f.release.sha256}"`);
  assert.equal(response.headers.get("Content-Length"), String(f.complete.length));
  assert.equal(response.headers.get("Content-Type"), "application/octet-stream");
  assert.equal(response.headers.get("Content-Disposition"), 'attachment; filename="Mir2Setup.exe"');
  assert.equal(response.headers.get("Accept-Ranges"), "bytes");
  assert.match(response.headers.get("Cache-Control"), /immutable, no-transform/);
  assert.equal(response.headers.get("x-mir2-first-chunk-cache"), "MISS");
  assert.deepEqual(Buffer.from(await collect(response.body)), f.complete);
  await f.ctx.settle();
  assert.equal(f.bucket.maxActive, 1);
  assert.equal(f.bucket.arrayBufferCalls, 0);
  assert.equal(f.cache.entries.size, f.release.parts.length);
  for (const { request, response } of f.cache.puts) {
    assert.match(request.url, /\/__chunks\/[0-9a-f]{64}\.bin$/);
    assert.notEqual(Number(response.headers.get("Content-Length")), f.release.bytes);
    assert.equal(request.headers.size ?? [...request.headers].length, 0);
  }
  const reads = f.bucket.gets.length;
  const cached = await f.run();
  assert.equal(cached.headers.get("x-mir2-first-chunk-cache"), "HIT");
  assert.deepEqual(Buffer.from(await collect(cached.body)), f.complete);
  assert.equal(f.bucket.gets.length, reads);
  const firstCache = f.cache.events.findIndex((event) => event.includes("cache-complete:"));
  const secondGet = f.cache.events.indexOf(`get:${f.release.parts[1].key}`);
  assert.ok(firstCache < secondGet, "the next R2 object must wait for the current cache writer");
});

test("a range crossing a part boundary concatenates only the requested original bytes", async () => {
  const f = setup();
  const boundary = f.release.parts[1].offset;
  const response = await f.run({ Range: `bytes=${boundary - 2}-${boundary + 2}` });
  assert.equal(response.status, 206);
  assert.equal(response.headers.get("Content-Range"), `bytes ${boundary - 2}-${boundary + 2}/${f.release.bytes}`);
  assert.equal(response.headers.get("Content-Length"), "5");
  assert.deepEqual(Buffer.from(await collect(response.body)), f.complete.subarray(boundary - 2, boundary + 3));
  await f.ctx.settle();
  assert.deepEqual(f.bucket.gets, f.release.parts.slice(0, 2).map((part) => part.key));
  assert.equal(f.bucket.maxActive, 1);
});

test("suffix, open-ended and oversized end ranges follow single-byte-range semantics", async () => {
  for (const [header, from, to] of [["bytes=-5", 27, 32], ["bytes=15-", 15, 32],
    ["bytes=0-9999999999999999999999999", 0, 32], ["bytes=-9999999999999999999999999", 0, 32],
    ["bytes=0002-0003", 2, 4]]) {
    const f = setup();
    const response = await f.run({ Range: header });
    assert.equal(response.status, 206, header);
    assert.deepEqual(Buffer.from(await collect(response.body)), f.complete.subarray(from, to), header);
    await f.ctx.settle();
  }
});

test("invalid ranges return 416 without touching R2 or cache; HEAD ignores every Range", async () => {
  const f = setup();
  for (const range of ["bytes=32-", "bytes=9-8", "bytes=-0", "bytes=-", "bytes=0-1,3-4", "items=1-2", "bytes=+1-2"]) {
    const response = await f.run({ Range: range });
    assert.equal(response.status, 416, range);
    assert.equal(response.headers.get("Content-Range"), "bytes */32");
    const head = await f.run({ Range: range }, "HEAD");
    assert.equal(head.status, 200);
    assert.equal(head.body, null);
    assert.equal(head.headers.get("Content-Length"), "32");
    assert.equal(head.headers.get("x-mir2-first-chunk-cache"), null);
  }
  assert.equal(f.bucket.gets.length, 0);
  assert.equal(f.cache.matches.length, 0);
});

test("If-None-Match weak tags, lists and star take priority over Range without reading a body", async () => {
  const f = setup();
  for (const tag of ["*", `"${f.release.sha256}"`, `W/"${f.release.sha256}"`, `"other", W/"${f.release.sha256}"`]) {
    for (const method of ["GET", "HEAD"]) {
      const response = await f.run({ "If-None-Match": tag, Range: "bytes=9999-" }, method);
      assert.equal(response.status, 304, tag);
      assert.equal(response.body, null);
      assert.equal(response.headers.get("ETag"), `"${f.release.sha256}"`);
    }
  }
  assert.equal(f.bucket.gets.length, 0);
  assert.equal(f.cache.matches.length, 0);
});

test("If-Range needs the strong ETag or exact Last-Modified; weak and mismatched validators produce 200", async () => {
  for (const validator of ["strong", "date", "weak", "wrong", "old-date"]) {
    const f = setup();
    const value = { strong: `"${f.release.sha256}"`, date: f.release.lastModified,
      weak: `W/"${f.release.sha256}"`, wrong: '"another"', "old-date": "Thu, 29 Oct 2026 08:00:00 GMT" }[validator];
    const matching = validator === "strong" || validator === "date";
    const response = await f.run({ Range: matching ? "bytes=0-1" : "bytes=999999-", "If-Range": value });
    assert.equal(response.status, matching ? 206 : 200, validator);
    assert.deepEqual(Buffer.from(await collect(response.body)), matching ? f.complete.subarray(0, 2) : f.complete);
    await f.ctx.settle();
  }
});

test("only the five exact HTTPS paths and approved hosts are readable; writes and internal cache paths stay closed", async () => {
  const f = setup();
  const badUrls = [
    `${f.release.publicBase}__chunks/${f.release.parts[0].sha256}.bin`, `${f.release.publicBase}.env`,
    `${f.release.publicBase}Mir2Setup.exe/`, `${f.release.publicBase}mir2setup.exe`,
    `${f.release.publicBase}Mir2Setup.exe?`, `${f.release.publicBase}Mir2Setup.exe?v=1`,
    `${f.release.publicBase}Mir2Setup.exe#fragment`, `${f.release.publicBase}%4dir2Setup.exe`,
    `${f.release.publicBase}%2fMir2Setup.exe`, `${f.release.publicBase}%252fMir2Setup.exe`,
    `${f.release.publicBase}../${f.release.sha256}/Mir2Setup.exe`, `${f.release.publicBase}//Mir2Setup.exe`,
    f.release.publicBase, "https://assets.mir2.obelisk.build/client-updates/five-class/latest.json",
    f.release.publicBase.replace("https:", "http:") + "Mir2Setup.exe",
    f.release.publicBase.replace("assets.mir2.obelisk.build", "evil.example") + "Mir2Setup.exe",
    f.release.publicBase.replace("assets.mir2.obelisk.build", "assets.mir2.obelisk.build:444") + "Mir2Setup.exe",
  ];
  // Keep deliberately noncanonical raw paths intact instead of letting Request
  // normalize dot segments before the boundary is exercised.
  for (const url of badUrls) {
    const response = await f.handler({ url, method: "GET", headers: new Headers() }, { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
    assert.equal(response.status, 404, url);
  }
  for (const method of ["PUT", "POST", "DELETE", "PATCH", "OPTIONS"]) {
    const response = await f.run({}, method);
    assert.equal(response.status, 405, method);
    assert.equal(response.headers.get("Allow"), "GET, HEAD");
  }
  const staged = await f.run({}, "HEAD", "Mir2Setup.exe", "https://mir2-full-installer-downloads.zombiesliu.workers.dev");
  assert.equal(staged.status, 200);
  assert.equal(f.bucket.gets.length, 0);
  assert.equal(f.cache.matches.length, 0);
});

test("malformed manifest identities, gaps, overlaps, duplicate companions and oversized parts are rejected", () => {
  const changes = [
    (r) => { r.schema = 2; }, (r) => { r.sha256 = r.sha256.toUpperCase(); },
    (r) => { r.sourceInstaller = "a".repeat(64); }, (r) => { r.parts[1].offset += 1; },
    (r) => { r.parts[1].offset -= 1; }, (r) => { r.parts[0].bytes = 64 * 1024 * 1024 + 1; },
    (r) => { r.files[1] = r.files[0]; }, (r) => { r.files[0].bytes = 32769; },
    (r) => { r.publicBase += "other/"; }, (r) => { r.allowedHosts = ["https://assets.mir2.obelisk.build"]; },
    (r) => { r.parts[0].key = "https://other.example/payload"; },
  ];
  for (const change of changes) {
    const { release } = fixture();
    change(release);
    assert.throws(() => createDownloadHandler(release), TypeError);
  }
});

test("native SHA, custom SHA, source installer, size, encoding and whole-object shape all fail closed", async () => {
  for (const target of ["part", "companion"]) {
    for (const kind of ["size", "missing-native", "string-native", "wrong-native", "short-native", "custom-sha", "source", "gzip", "partial"]) {
      const f = setup();
      const expected = target === "part" ? f.release.parts[0] : f.release.files[0];
      const invalid = {
        size: { size: expected.bytes + 1 }, "missing-native": { checksums: {} },
        "string-native": { checksums: { sha256: expected.sha256 } },
        "wrong-native": { checksums: { sha256: nativeSha("0".repeat(64)) } },
        "short-native": { checksums: { sha256: new Uint8Array(31) } },
        "custom-sha": { customMetadata: { sha256: "0".repeat(64), sourceInstaller: f.release.sha256 } },
        source: { customMetadata: { sha256: expected.sha256, sourceInstaller: "0".repeat(64) } },
        gzip: { httpMetadata: { contentEncoding: "gzip" } }, partial: { range: { offset: 0, length: expected.bytes } },
      }[kind];
      f.bucket.overrides.set(expected.key, invalid);
      const response = await f.run({}, "GET", target === "part" ? "Mir2Setup.exe" : expected.name);
      assert.equal(response.status, 503, `${target}/${kind}`);
      assert.equal(f.bucket.reads.length, 0);
      assert.equal(f.bucket.cancels.length, 1);
      assert.equal(f.cache.puts.length, 0);
    }
  }
});

test("an offset ArrayBufferView native checksum is read without including unrelated backing bytes", async () => {
  const f = setup();
  const expected = f.release.parts[0];
  const backing = new Uint8Array(48);
  backing.set(new Uint8Array(nativeSha(expected.sha256)), 8);
  f.bucket.overrides.set(expected.key, { checksums: { sha256: new DataView(backing.buffer, 8, 32) } });
  const response = await f.run({ Range: "bytes=0-1" });
  assert.equal(response.status, 206);
  assert.equal(Buffer.from(await collect(response.body)).toString(), "MZ");
  await f.ctx.settle();
});

test("companions verify actual bytes before caching, including cached body corruption", async () => {
  const f = setup();
  const file = f.release.files[3];
  const response = await f.run({}, "GET", file.name);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("ETag"), `"${file.sha256}"`);
  assert.deepEqual(await collect(response.body), f.objects.get(file.key).data);
  await f.ctx.settle();
  assert.equal(f.bucket.gets.length, 1);
  const hit = await f.run({ Range: "bytes=0-2" }, "GET", file.name);
  assert.equal(hit.status, 206);
  assert.deepEqual(await collect(hit.body), f.objects.get(file.key).data.subarray(0, 3));
  assert.equal(f.bucket.gets.length, 1);
  const entry = [...f.cache.entries.values()][0];
  entry.data = Uint8Array.from(entry.data);
  entry.data[0] ^= 1;
  assert.equal((await f.run({}, "GET", file.name)).status, 503);
  assert.equal(f.bucket.gets.length, 1, "a corrupt cache entry must not trigger an alternate source");

  const corrupt = setup();
  const corruptFile = corrupt.release.files[0];
  const source = corrupt.objects.get(corruptFile.key);
  source.actualData = Uint8Array.from(source.data);
  source.actualData[0] ^= 1;
  assert.equal((await corrupt.run({}, "GET", corruptFile.name)).status, 503);
  assert.equal(corrupt.cache.puts.length, 0);
});

test("HEAD and conditional companion requests use the manifest without reading or caching objects", async () => {
  const f = setup();
  for (const file of f.release.files) {
    const head = await f.run({ Range: "bytes=99999-" }, "HEAD", file.name);
    assert.equal(head.status, 200);
    assert.equal(head.body, null);
    assert.equal(head.headers.get("Content-Length"), String(file.bytes));
    const unchanged = await f.run({ "If-None-Match": `W/"${file.sha256}"` }, "GET", file.name);
    assert.equal(unchanged.status, 304);
  }
  assert.equal(f.bucket.gets.length, 0);
  assert.equal(f.cache.matches.length, 0);
});

test("missing first part, storage exceptions and cache lookup exceptions return 503 without network fallback", async () => {
  for (const kind of ["missing", "storage", "cache"]) {
    const f = setup();
    if (kind === "missing") f.objects.delete(f.release.parts[0].key);
    if (kind === "storage") f.bucket.getError = true;
    if (kind === "cache") f.cache.matchError = true;
    const response = await f.run();
    assert.equal(response.status, 503, kind);
    assert.equal(response.headers.get("Location"), null);
    assert.equal(f.cache.puts.length, 0);
  }
});

test("cache entries with wrong identity, length, status or encoding are rejected without an R2 read", async () => {
  for (const kind of ["sha", "source", "length", "status", "encoding", "cookie", "marker"]) {
    const f = setup();
    await collect((await f.run()).body);
    await f.ctx.settle();
    const entry = f.cache.entries.get(`${f.release.publicBase}__chunks/${f.release.parts[0].sha256}.bin`);
    if (kind === "sha") entry.headers.set("x-mir2-sha256", "0".repeat(64));
    if (kind === "source") entry.headers.set("x-mir2-source-installer", "0".repeat(64));
    if (kind === "length") entry.headers.set("Content-Length", "999");
    if (kind === "status") entry.status = 206;
    if (kind === "encoding") entry.headers.set("Content-Encoding", "gzip");
    if (kind === "cookie") entry.headers.set("Set-Cookie", "private=value");
    if (kind === "marker") entry.headers.delete("x-mir2-download-cache");
    const count = f.bucket.gets.length;
    assert.equal((await f.run()).status, 503, kind);
    assert.equal(f.bucket.gets.length, count);
  }
});

test("client cancellation reaches the current R2 reader, aborts its cache write and never opens another part", async () => {
  const f = setup();
  const response = await f.run();
  assert.equal(f.bucket.reads.length, 0, "returning headers must not prebuffer a part");
  const reader = response.body.getReader();
  assert.equal((await reader.read()).done, false);
  await reader.cancel("player stopped download");
  await f.ctx.settle();
  assert.equal(f.bucket.gets.length, 1);
  assert.equal(f.bucket.cancels.length, 1);
  assert.equal(f.bucket.active, 0);
  assert.equal(f.cache.entries.size, 0);
});

test("a request abort also stops the current R2 reader and propagates an error to the download", async () => {
  const f = setup();
  const abort = new AbortController();
  const request = new Request(f.release.publicBase + "Mir2Setup.exe", { signal: abort.signal });
  const response = await f.handler(request, { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
  const reader = response.body.getReader();
  await reader.read();
  abort.abort("connection disconnected");
  await assert.rejects(reader.read(), /cancelled/);
  await f.ctx.settle();
  assert.equal(f.bucket.cancels.length, 1);
  assert.equal(f.bucket.gets.length, 1);
  assert.equal(f.bucket.active, 0);
});

test("missing later parts error the response stream instead of returning a successful truncated installer", async () => {
  const f = setup();
  f.objects.delete(f.release.parts[1].key);
  const response = await f.run();
  assert.equal(response.status, 200);
  await assert.rejects(collect(response.body), /Untrusted installer object/);
  await f.ctx.settle();
  assert.equal(f.bucket.gets.length, 2);
});

test("cache failures propagate, cancel the source and do not become unhandled background rejections", async () => {
  const f = setup();
  f.cache.putError = true;
  const response = await f.run();
  if (response.status === 200) await assert.rejects(collect(response.body), /Cache write failed/);
  else assert.equal(response.status, 503);
  await f.ctx.settle();
  await turn();
  assert.equal(f.bucket.gets.length, 1);
  assert.equal(f.bucket.cancels.length, 1);
  assert.equal(f.cache.entries.size, 0);
  const small = setup();
  small.cache.putError = true;
  assert.equal((await small.run({}, "GET", small.release.files[0].name)).status, 503);
  await small.ctx.settle();
});

test("a cache write rejected after consuming data errors the client stream and cancels R2", async () => {
  const f = setup();
  f.cache.put = async (_request, response) => {
    const reader = response.body.getReader();
    await reader.read();
    throw new Error("Cache stopped during ingestion");
  };
  const response = await f.run();
  if (response.status === 200) await assert.rejects(collect(response.body), /Cache stopped during ingestion/);
  else assert.equal(response.status, 503);
  await f.ctx.settle();
  assert.equal(f.bucket.cancels.length, 1);
  assert.equal(f.bucket.gets.length, 1);
});

test("a 1,026,815,762-byte manifest stays lazy, bounded by the current chunk and client/cache backpressure", async () => {
  const f = fixture();
  const limit = 64 * 1024 * 1024;
  f.release.bytes = 1_026_815_762;
  f.release.parts = Array.from({ length: 16 }, (_, index) => ({
    offset: index * limit, bytes: Math.min(limit, f.release.bytes - index * limit),
    sha256: sha(`virtual authenticated R2 part ${index}`), key: `private/virtual-${index}.bin`,
  }));
  let reads = 0;
  let cancelled = 0;
  const gets = [];
  const bucket = { async get(key) {
    gets.push(key);
    const part = f.release.parts.find((part) => part.key === key);
    return { size: part.bytes, checksums: { sha256: nativeSha(part.sha256) },
      customMetadata: { sha256: part.sha256, sourceInstaller: f.release.sha256 }, httpMetadata: {},
      arrayBuffer() { throw new Error("buffering a large R2 object is forbidden"); },
      body: new ReadableStream({
        pull(controller) {
          reads += 1;
          const data = new Uint8Array(64 * 1024);
          if (reads === 1) data.set([0x4d, 0x5a]);
          controller.enqueue(data);
        },
        cancel() { cancelled += 1; },
      }, { highWaterMark: 0 }),
    };
  } };
  const cache = new MemoryCache();
  let releaseCache;
  cache.gate = new Promise((resolve) => { releaseCache = resolve; });
  const ctx = context();
  const handler = createDownloadHandler(f.release);
  const response = await handler(new Request(f.release.publicBase + "Mir2Setup.exe"), { MIR2_ASSETS: bucket }, ctx, cache);
  assert.equal(response.headers.get("Content-Length"), "1026815762");
  assert.equal(gets.length, 1);
  assert.equal(reads, 0);
  const reader = response.body.getReader();
  const first = reader.read();
  await turn();
  assert.equal(reads, 1, "blocked cache consumption must backpressure the R2 reader");
  releaseCache();
  const chunk = await first;
  assert.equal(chunk.value.length, 64 * 1024);
  assert.deepEqual([...chunk.value.subarray(0, 2)], [0x4d, 0x5a]);
  await turn();
  assert.equal(reads, 1, "a stalled client must backpressure the source even when cache is faster");
  await reader.cancel("bounded streaming proved");
  await ctx.settle();
  assert.equal(cancelled, 1);
  assert.equal(gets.length, 1);
  assert.equal(cache.entries.size, 0);
});

test("default cache lookup and CORS remain public read-only without cookie forwarding", async () => {
  const f = setup();
  const previous = globalThis.caches;
  globalThis.caches = { default: f.cache };
  try {
    const request = f.request({ Cookie: "session=private", Authorization: "Bearer irrelevant", Range: "bytes=0-1" });
    const response = await f.handler(request, { MIR2_ASSETS: f.bucket }, f.ctx);
    assert.equal(response.headers.get("Access-Control-Allow-Origin"), "*");
    assert.equal(response.headers.get("Access-Control-Allow-Methods"), "GET, HEAD");
    assert.equal(response.headers.get("Access-Control-Allow-Credentials"), null);
    assert.equal(response.headers.get("Set-Cookie"), null);
    assert.equal(Buffer.from(await collect(response.body)).toString(), "MZ");
    await f.ctx.settle();
    assert.ok(f.cache.matches.every((request) => !request.headers.has("Cookie") && !request.headers.has("Authorization")));
  } finally {
    if (previous === undefined) delete globalThis.caches;
    else globalThis.caches = previous;
  }
});

test("Workers FixedLengthStream is used for both HTTP length and each full-part cache write, with cancellation preserved", async () => {
  const previous = globalThis.FixedLengthStream;
  const constructed = [];
  globalThis.FixedLengthStream = class extends TransformStream {
    constructor(expected) {
      let count = 0;
      super({
        transform(value, controller) {
          count += value.byteLength;
          assert.ok(count <= expected);
          controller.enqueue(value);
        },
        flush() { assert.equal(count, expected); },
      });
      constructed.push(expected);
    }
  };
  try {
    const f = setup();
    const response = await f.run();
    assert.deepEqual(Buffer.from(await collect(response.body)), f.complete);
    await f.ctx.settle();
    assert.ok(constructed.includes(f.release.bytes));
    for (const part of f.release.parts) assert.ok(constructed.includes(part.bytes));

    const short = setup();
    const partial = await short.run({ Range: "bytes=0-1" });
    assert.equal(Buffer.from(await collect(partial.body)).toString(), "MZ");
    await short.ctx.settle();
    assert.ok(constructed.includes(2));
    assert.equal(short.cache.entries.size, 1);

    const cancelled = setup();
    const reader = (await cancelled.run()).body.getReader();
    await reader.read();
    await reader.cancel("stop native HTTP stream");
    await cancelled.ctx.settle();
    assert.equal(cancelled.bucket.cancels.length, 1);
    assert.equal(cancelled.bucket.gets.length, 1);
  } finally {
    if (previous === undefined) delete globalThis.FixedLengthStream;
    else globalThis.FixedLengthStream = previous;
  }
});
