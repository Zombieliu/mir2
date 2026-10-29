import test from "node:test";
import assert from "node:assert/strict";
import { handle, OBJECT_PREFIX, POINTER_KEY, PUBLIC_BASE, MAX_ARTIFACT, MAX_METADATA, MAX_FIVE_CLASS_METADATA, FIVE_CLASS_OBJECT_PREFIX, FIVE_CLASS_POINTER_KEY, FIVE_CLASS_PUBLIC_BASE, MAX_POINTER, feedDirectory,
  PACKED_OBJECT_PREFIX, PACKED_POINTER_KEY, PACKED_PUBLIC_BASE, MAX_PACKED_CATALOG, MAX_PACKED_OBJECT, MAX_PACKED_MANIFEST, MAX_PACKED_SMALL } from "../src/index.mjs";

const bytes = (text) => new TextEncoder().encode(text);
const pointer = (sequence = 1) => ({
  schema: "mir2.windows.r2-channel.v1", channel: "invited", platform: "windows-x64",
  sequence, sourceRevision: String(sequence).repeat(40), feedSha256: "a".repeat(64),
  signatureSha256: "b".repeat(64),
});
const stream = (value) => new ReadableStream({ start(controller) { controller.enqueue(value); controller.close(); } });

class Bucket {
  rows = new Map();
  calls = [];
  afterSnapshot = null;
  fail = false;
  put(path, text = "0123456789", extra = {}) {
    this.rows.set(OBJECT_PREFIX + path, {
      bytes: typeof text === "string" ? bytes(text) : text,
      httpEtag: '"etag-' + path + '"', httpMetadata: {},
      uploaded: new Date("2026-10-01T00:00:00Z"), ...extra,
    });
  }
  meta(row) {
    return { size: row.declaredSize ?? row.bytes.length, httpEtag: row.httpEtag,
      httpMetadata: row.httpMetadata, uploaded: row.uploaded, customMetadata: row.customMetadata };
  }
  async head(key) {
    this.calls.push(["head", key]);
    if (this.fail) throw new Error("private backend details");
    const row = this.rows.get(key);
    return row ? this.meta(row) : null;
  }
  async get(key, options) {
    this.calls.push(["get", key, options]);
    if (this.fail) throw new Error("private backend details");
    const row = this.rows.get(key);
    if (!row) return null;
    const snapshot = { ...row, bytes: new Uint8Array(row.bytes) };
    if (this.afterSnapshot) this.afterSnapshot(key);
    const range = options?.range;
    return { ...this.meta(snapshot), body: stream(range ? snapshot.bytes.slice(range.offset, range.offset + range.length) : snapshot.bytes),
      ...(range ? { range: { ...range } } : {}) };
  }
}
class Cache {
  rows = new Map();
  matches = [];
  puts = [];
  async match(request) { this.matches.push(request.url); return this.rows.get(request.url)?.clone(); }
  async put(request, response) {
    this.puts.push(request.url);
    const body = await response.arrayBuffer();
    this.rows.set(request.url, new Response(body, { status: response.status, headers: response.headers }));
  }
}
function setup() {
  const bucket = new Bucket();
  const cache = new Cache();
  const pending = [];
  const ctx = { waitUntil(promise) { pending.push(promise); } };
  const fetch = (path = "releases/r1/a.gz", headers = {}, method = "GET") =>
    handle(new Request(PUBLIC_BASE + path, { method, headers }), { MIR2_ASSETS: bucket }, ctx, cache);
  const done = () => Promise.all(pending);
  return { bucket, cache, ctx, fetch, done };
}

test("immutable literal gzip bytes cache under one canonical URL", async () => {
  const f = setup();
  const gzip = Uint8Array.from([31, 139, 8, 0, 0, 0]);
  f.bucket.put("releases/r1/a.gz", gzip);
  const first = await f.fetch();
  assert.equal(first.status, 200);
  assert.equal(first.headers.get("content-encoding"), null);
  assert.match(first.headers.get("cache-control"), /31536000, immutable/);
  assert.deepEqual(new Uint8Array(await first.arrayBuffer()), gzip);
  await f.done();
  const second = await f.fetch("r%65leases/r1/a.gz");
  assert.equal(second.headers.get("x-mir2-native-cache"), "HIT");
  assert.equal(f.cache.puts.length, 1);
  assert.equal(f.cache.matches[1], PUBLIC_BASE + "releases/r1/a.gz");
  assert.deepEqual(new Uint8Array(await second.arrayBuffer()), gzip);
});

test("bounded closed/open/suffix ranges clamp ends and carry real offsets", async () => {
  for (const [range, result, text] of [
    ["bytes=0-3", "bytes 0-3/10", "0123"],
    ["bytes=4-", "bytes 4-9/10", "456789"],
    ["bytes=-3", "bytes 7-9/10", "789"],
    ["bytes=-100", "bytes 0-9/10", "0123456789"],
    ["bytes=8-99", "bytes 8-9/10", "89"],
    ["Bytes=2-4", "bytes 2-4/10", "234"],
  ]) {
    const f = setup(); f.bucket.put("releases/r1/a.gz");
    const response = await f.fetch(undefined, { Range: range });
    assert.equal(response.status, 206, range);
    assert.equal(response.headers.get("content-range"), result, range);
    assert.equal(response.headers.get("content-length"), String(text.length));
    assert.equal(await response.text(), text);
    assert.equal(f.cache.puts.length, 0);
  }
});

test("invalid, multiple, huge and unsatisfiable ranges return proper 416", async () => {
  for (const range of ["bytes=10-", "bytes=8-2", "bytes=-0", "bytes=0-1,4-5", "bytes=-", "items=1-2", "bytes=999999999999999999999-"]) {
    const f = setup(); f.bucket.put("releases/r1/a.gz");
    const response = await f.fetch(undefined, { Range: range });
    assert.equal(response.status, 416, range);
    assert.equal(response.headers.get("content-range"), "bytes */10");
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.equal(f.bucket.calls.filter((c) => c[0] === "get").length, 0);
  }
  const f = setup(); f.bucket.put("releases/r1/a.gz", "");
  assert.equal((await f.fetch(undefined, { Range: "bytes=0-" })).headers.get("content-range"), "bytes */0");
});

test("If-Range strong tag/date gate and weak/stale gates", async () => {
  for (const [value, status] of [
    ['"etag-releases/r1/a.gz"', 206], ['W/"etag-releases/r1/a.gz"', 200], ['"other"', 200],
    ["Thu, 01 Oct 2026 00:00:00 GMT", 206], ["Wed, 30 Sep 2026 00:00:00 GMT", 200],
    ["Fri, 02 Oct 2026 00:00:00 GMT", 200], ["invalid", 200],
  ]) {
    const f = setup(); f.bucket.put("releases/r1/a.gz");
    const response = await f.fetch(undefined, { Range: "bytes=0-3", "If-Range": value });
    assert.equal(response.status, status, value);
    assert.equal(await response.text(), status === 206 ? "0123" : "0123456789");
  }
});

test("HEAD is bodyless, does not get an artifact, and ignores Range", async () => {
  const f = setup(); f.bucket.put("releases/r1/a.gz");
  const response = await f.fetch(undefined, { Range: "bytes=0-2" }, "HEAD");
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("content-length"), "10");
  assert.equal(response.headers.get("content-range"), null);
  assert.equal(await response.text(), "");
  assert.equal(f.bucket.calls.filter((c) => c[0] === "get").length, 0);
});

test("If-None-Match gives bodyless 304 before ranges, including cache hits", async () => {
  const f = setup(); f.bucket.put("releases/r1/a.gz");
  for (const value of ['"etag-releases/r1/a.gz"', 'W/"etag-releases/r1/a.gz"', '"other", "etag-releases/r1/a.gz"', "*"]) {
    const response = await f.fetch(undefined, { "If-None-Match": value, Range: "bytes=99-" });
    assert.equal(response.status, 304);
    assert.equal(response.headers.get("content-length"), null);
    assert.equal(await response.text(), "");
  }
  const first = await f.fetch(); await first.text(); await f.done();
  const hit = await f.fetch(undefined, { "If-None-Match": "*" }, "HEAD");
  assert.equal(hit.status, 304);
  assert.equal(hit.headers.get("x-mir2-native-cache"), "HIT");
});

test("path traversal, aliases outside allowlist, encoded separators and all queries are refused", async () => {
  const f = setup();
  for (const path of ["channels/invited.json", "other/a", "latest.json/child", "releases//a", "releases/.hidden", "releases/%252e%252e/a", "releases/a%2Fb", "releases/a%5Cb", "releases/%00a", "releases/%ZZ", "releases/a?cache=1", "releases/a?", "releases/a#frag", "../mir2/v/web/a"]) {
    const response = await f.fetch(path);
    assert.equal(response.status, 404, path);
  }
  assert.equal(f.bucket.calls.length, 0);
});

test("only GET and HEAD are accepted", async () => {
  const f = setup();
  for (const method of ["PUT", "POST", "DELETE", "OPTIONS"]) assert.equal((await f.fetch(undefined, {}, method)).status, 405);
  assert.equal(f.bucket.calls.length, 0);
});

test("latest aliases take one atomic pointer snapshot and never touch Cache API", async () => {
  const f = setup();
  const p1 = pointer(1), p2 = pointer(2);
  f.bucket.put("channels/invited.json", JSON.stringify(p1) + "\n");
  for (const p of [p1, p2]) {
    f.bucket.put(feedDirectory(p) + "latest.json", "feed-" + p.sequence, { customMetadata: { sha256: p.feedSha256 } });
    f.bucket.put(feedDirectory(p) + "latest.p7s", "sig-" + p.sequence, { customMetadata: { sha256: p.signatureSha256 } });
  }
  f.bucket.afterSnapshot = (key) => {
    if (key === POINTER_KEY) f.bucket.put("channels/invited.json", JSON.stringify(p2) + "\n");
  };
  const old = await f.fetch("latest.json");
  assert.equal(old.headers.get("x-mir2-sequence"), "1");
  assert.equal(await old.text(), "feed-1");
  assert.equal(old.headers.get("cache-control"), "no-store");
  const current = await f.fetch("latest.p7s");
  assert.equal(current.headers.get("x-mir2-sequence"), "2");
  assert.equal(await current.text(), "sig-2");
  assert.equal(current.headers.get("cache-control"), "no-store");
  assert.equal(f.cache.matches.length, 0);
  assert.equal(f.cache.puts.length, 0);
  assert.equal(f.bucket.calls.filter((c) => c[0] === "get" && c[1] === POINTER_KEY).length, 2);
});

test("latest HEAD/304 keep no-store and generation headers", async () => {
  const f = setup(); const p = pointer();
  f.bucket.put("channels/invited.json", JSON.stringify(p));
  const path = feedDirectory(p) + "latest.json";
  f.bucket.put(path, "feed", { customMetadata: { sha256: p.feedSha256 } });
  const head = await f.fetch("latest.json", {}, "HEAD");
  assert.equal(head.headers.get("cache-control"), "no-store");
  assert.equal(head.headers.get("x-mir2-feed-path"), path);
  const response = await f.fetch("latest.json", { "If-None-Match": '"etag-' + path + '"' });
  assert.equal(response.status, 304);
  assert.equal(response.headers.get("cache-control"), "no-store");
  assert.equal(response.headers.get("x-mir2-signature-path"), feedDirectory(p) + "latest.p7s");
  assert.equal(f.cache.matches.length, 0);
});

test("strict pointer schema rejects unknown fields, wrong fields, duplicate keys and oversized body", async () => {
  for (const value of [
    { ...pointer(), extra: true }, { ...pointer(), sequence: 0 }, { ...pointer(), sequence: 1.5 },
    { ...pointer(), feedSha256: "../unsafe" }, { ...pointer(), platform: "linux" },
    { ...pointer(), sourceRevision: "A".repeat(40) },
    { ...pointer(), sourceRevision: ["a".repeat(40)] }, { ...pointer(), feedSha256: ["a".repeat(64)] },
    { ...pointer(), signatureSha256: ["b".repeat(64)] },
  ]) {
    const f = setup(); f.bucket.put("channels/invited.json", JSON.stringify(value));
    const response = await f.fetch("latest.json");
    assert.equal(response.status, 502);
    assert.equal(response.headers.get("cache-control"), "no-store");
  }
  const f = setup();
  const duplicate = JSON.stringify(pointer()).replace('"sequence":1', '"sequence":1,"sequence":1');
  f.bucket.put("channels/invited.json", duplicate);
  assert.equal((await f.fetch("latest.json")).status, 502);
  f.bucket.put("channels/invited.json", "x".repeat(MAX_POINTER + 1));
  assert.equal((await f.fetch("latest.json")).status, 502);
  f.bucket.put("channels/invited.json", "x".repeat(MAX_POINTER + 1), { declaredSize: 1 });
  assert.equal((await f.fetch("latest.json")).status, 502);
  f.bucket.put("channels/invited.json", Uint8Array.from([0xff]));
  assert.equal((await f.fetch("latest.json")).status, 502);
});

test("oversized objects and HTTP storage encoding are rejected without caching", async () => {
  for (const [path, extra] of [
    ["releases/r1/a.gz", { declaredSize: MAX_ARTIFACT + 1 }],
    ["feeds/s1-a/a.json", { declaredSize: MAX_METADATA + 1 }],
    ["releases/r1/a.gz", { httpMetadata: { contentEncoding: "gzip" } }],
    ["feeds/s1-a/a.json", { httpMetadata: { contentEncoding: "br" } }],
  ]) {
    const f = setup(); f.bucket.put(path, "x", extra);
    const response = await f.fetch(path);
    assert.equal(response.status, 502);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.equal(f.cache.puts.length, 0);
  }
});

test("pointer hash metadata mismatch is fail-closed", async () => {
  const f = setup(); const p = pointer();
  f.bucket.put("channels/invited.json", JSON.stringify(p));
  f.bucket.put(feedDirectory(p) + "latest.json", "feed", { customMetadata: { sha256: "c".repeat(64) } });
  assert.equal((await f.fetch("latest.json")).status, 502);
});

test("storage errors and absent objects do not leak backend errors or populate caches", async () => {
  const f = setup();
  assert.equal((await f.fetch()).status, 404);
  assert.equal((await f.fetch("latest.json")).status, 503);
  f.bucket.fail = true;
  const response = await f.fetch();
  assert.equal(response.status, 503);
  assert.doesNotMatch(await response.text(), /private|backend/);
  assert.equal(f.cache.puts.length, 0);
});

test("HEAD failures are bodyless and never cached", async () => {
  const f = setup();
  for (const path of ["channels/invited.json", "releases/missing.gz", "latest.json"]) {
    const response = await f.fetch(path, {}, "HEAD");
    assert.ok(response.status >= 400);
    assert.equal(await response.text(), "");
    assert.equal(response.headers.get("cache-control"), "no-store");
  }
  f.bucket.put("releases/r1/a.gz", "x", { httpMetadata: { contentEncoding: "gzip" } });
  assert.equal(await (await f.fetch(undefined, {}, "HEAD")).text(), "");
  assert.equal(f.cache.puts.length, 0);
});

test("inconsistent R2 range metadata or changed object cannot be served or cached", async () => {
  for (const change of [
    (data) => { data.range.offset += 1; },
    (data) => { data.range.length += 1; },
    (data) => { delete data.range; },
    (data) => { data.httpEtag = '"changed"'; },
    (data) => { data.size += 1; },
  ]) {
    const f = setup(); f.bucket.put("releases/r1/a.gz");
    const get = f.bucket.get.bind(f.bucket);
    f.bucket.get = async (...args) => { const data = await get(...args); change(data); return data; };
    const response = await f.fetch(undefined, { Range: "bytes=1-3" });
    assert.equal(response.status, 502);
    assert.equal(response.headers.get("cache-control"), "no-store");
    assert.equal(f.cache.puts.length, 0);
  }
});


test("five-class aliases read their own atomic pointer and preserve legacy discovery", async () => {
  const f = setup();
  const legacy = pointer(1);
  const modern = pointer(2);
  f.bucket.put("channels/invited.json", JSON.stringify(legacy));
  f.bucket.put(feedDirectory(legacy) + "latest.json", "legacy-signed-feed");
  f.bucket.put(feedDirectory(legacy) + "latest.p7s", "legacy-signature");
  const putModern = (key, body) => f.bucket.rows.set(FIVE_CLASS_OBJECT_PREFIX + key, {
    bytes: bytes(body), httpEtag: '"modern-' + key + '"', httpMetadata: {},
  });
  putModern("channels/invited.json", JSON.stringify(modern));
  putModern(feedDirectory(modern) + "latest.json", "five-class-signed-feed");
  putModern(feedDirectory(modern) + "latest.p7s", "five-class-signature");
  for (const [base, feed, signature, sequence] of [
    [PUBLIC_BASE, "legacy-signed-feed", "legacy-signature", "1"],
    [FIVE_CLASS_PUBLIC_BASE, "five-class-signed-feed", "five-class-signature", "2"],
  ]) {
    for (const [name, body] of [["latest.json", feed], ["latest.p7s", signature]]) {
      const response = await handle(new Request(base + name), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
      assert.equal(response.status, 200);
      assert.equal(response.headers.get("cache-control"), "no-store");
      assert.equal(response.headers.get("x-mir2-sequence"), sequence);
      assert.equal(await response.text(), body);
    }
  }
  assert.ok(f.bucket.calls.some(([op, key]) => op === "get" && key === POINTER_KEY));
  assert.ok(f.bucket.calls.some(([op, key]) => op === "get" && key === FIVE_CLASS_POINTER_KEY));
  assert.equal(f.cache.puts.length, 0);
});

test("identical immutable object names have separate five-class cache and R2 keys", async () => {
  const f = setup();
  const path = "releases/r1/a.gz";
  f.bucket.put(path, "legacy-bytes");
  f.bucket.rows.set(FIVE_CLASS_OBJECT_PREFIX + path, {
    bytes: bytes("five-class-bytes"), httpEtag: '"five-class-bytes"', httpMetadata: {},
  });
  for (let pass = 0; pass < 2; pass++) {
    for (const [base, body] of [[PUBLIC_BASE, "legacy-bytes"], [FIVE_CLASS_PUBLIC_BASE, "five-class-bytes"]]) {
      const response = await handle(new Request(base + path), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
      assert.equal(response.status, 200);
      assert.equal(response.headers.get("x-mir2-native-cache"), pass === 0 ? "MISS" : "HIT");
      assert.equal(await response.text(), body);
      await f.done();
    }
  }
  assert.deepEqual(f.cache.puts, [PUBLIC_BASE + path, FIVE_CLASS_PUBLIC_BASE + path]);
});

test("five-class missing or invalid pointers never fall back to legacy discovery", async () => {
  const f = setup();
  f.bucket.put("channels/invited.json", JSON.stringify(pointer(1)));
  f.bucket.put(feedDirectory(pointer(1)) + "latest.json", "legacy-signed-feed");
  let response = await handle(new Request(FIVE_CLASS_PUBLIC_BASE + "latest.json"), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
  assert.equal(response.status, 503);
  f.bucket.rows.set(FIVE_CLASS_POINTER_KEY, {
    bytes: bytes("not-json"), httpEtag: '"invalid-five-class"', httpMetadata: {},
  });
  response = await handle(new Request(FIVE_CLASS_PUBLIC_BASE + "latest.json"), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
  assert.equal(response.status, 502);
  assert.ok(!f.bucket.calls.some(([op, key]) => op === "get" && key === POINTER_KEY));
});

test("five-class supports measured metadata while old channel retains its original bound", async () => {
  const f = setup();
  const path = "releases/r1/PACKAGE-MANIFEST.json";
  f.bucket.put(path, "", { declaredSize: MAX_METADATA + 1 });
  f.bucket.rows.set(FIVE_CLASS_OBJECT_PREFIX + path, {
    bytes: bytes(""), declaredSize: MAX_METADATA + 1, httpEtag: '"large-manifest"', httpMetadata: {},
  });
  let response = await f.fetch(path, {}, "HEAD");
  assert.equal(response.status, 502);
  response = await handle(new Request(FIVE_CLASS_PUBLIC_BASE + path, { method: "HEAD" }), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
  assert.equal(response.status, 200);
  assert.equal(response.headers.get("content-length"), String(MAX_METADATA + 1));
  f.bucket.rows.get(FIVE_CLASS_OBJECT_PREFIX + path).declaredSize = MAX_FIVE_CLASS_METADATA + 1;
  response = await handle(new Request(FIVE_CLASS_PUBLIC_BASE + path, { method: "HEAD" }), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
  assert.equal(response.status, 502);
});

test("five-class route rejects encoded scope changes, separators and nested discovery", async () => {
  for (const path of [
    "five%2dclass/latest.json", "five-class%2flatest.json", "five-class/%2flatest.json",
    "five-class/%5clatest.json", "five-class/five-class/latest.json", "other-class/latest.json",
    "five-class/channels/invited.json", "five-class/latest.json?scope=legacy",
  ]) {
    const f = setup();
    const response = await f.fetch(path);
    assert.equal(response.status, 404, path);
    assert.equal(f.bucket.calls.length, 0, path);
  }
});


test("five-class loose repair paths preserve literal Source spaces and one cache key", async () => {
  const f = setup();
  const literal = "releases/r1/mir2-assets/original-ui/AWeapon/00 L/1.png";
  f.bucket.rows.set(FIVE_CLASS_OBJECT_PREFIX + literal, {
    bytes: bytes("source-frame"), httpEtag: '"source-space-frame"', httpMetadata: {},
  });
  for (const path of [literal.replace("00 L", "00%20L"), literal]) {
    const response = await handle(new Request(FIVE_CLASS_PUBLIC_BASE + path), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
    assert.equal(response.status, 200);
    assert.equal(await response.text(), "source-frame");
    await f.done();
  }
  assert.equal(f.cache.puts.length, 1);
  assert.equal(f.cache.puts[0], FIVE_CLASS_PUBLIC_BASE + literal.replace("00 L", "00%20L"));
  const head = await handle(new Request(FIVE_CLASS_PUBLIC_BASE + literal, { method: "HEAD" }), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
  assert.equal(head.status, 200);
  assert.equal(head.headers.get("content-length"), "12");
  assert.equal(await head.text(), "");
  const legacy = await f.fetch(literal);
  assert.equal(legacy.status, 404);
});

test("five-class space allowance still rejects Windows normalization and encoded separators", async () => {
  for (const path of [
    "releases/r1/mir2-assets/AWeapon/00%20/1.png", "releases/r1/mir2-assets/AWeapon/00./1.png",
    "releases/r1/mir2-assets/AWeapon/%20/1.png", "releases/r1/mir2-assets/AWeapon/00%2520L/1.png",
    "releases/r1/mir2-assets/AWeapon/00%2fL/1.png", "releases/r1/mir2-assets/AWeapon/00%5cL/1.png",
    "releases/r1/mir2-assets/AWeapon/00%3aL/1.png",
  ]) {
    const f = setup();
    const response = await handle(new Request(FIVE_CLASS_PUBLIC_BASE + path), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
    assert.equal(response.status, 404, path);
    assert.equal(f.bucket.calls.length, 0, path);
  }
});

const generationDigest = "c".repeat(64);
const packedGeneration = "generations/" + generationDigest + "/";
const packPath = "objects/packs/" + "d".repeat(64) + ".m2p";
function putPacked(f, path, body, extra = {}) { f.bucket.put("packed/" + path, body, extra); }
function fetchPacked(f, path, headers = {}, method = "GET") {
  return handle(new Request(PACKED_PUBLIC_BASE + path, { method, headers }), { MIR2_ASSETS: f.bucket }, f.ctx, f.cache);
}

test("packed aliases use only their own atomic pointer and never cache discovery", async () => {
  const f = setup();
  const p1 = pointer(3), p2 = pointer(4);
  putPacked(f, "channels/invited.json", JSON.stringify(p1));
  f.bucket.put("channels/invited.json", JSON.stringify(pointer(1)));
  for (const p of [p1, p2]) {
    putPacked(f, feedDirectory(p) + "latest.json", "packed-feed-" + p.sequence, { customMetadata: { sha256: p.feedSha256 } });
    putPacked(f, feedDirectory(p) + "latest.p7s", "packed-signature-" + p.sequence, { customMetadata: { sha256: p.signatureSha256 } });
  }
  f.bucket.afterSnapshot = (key) => {
    if (key === PACKED_POINTER_KEY) putPacked(f, "channels/invited.json", JSON.stringify(p2));
  };
  const first = await fetchPacked(f, "latest.json");
  assert.equal(first.status, 200);
  assert.equal(first.headers.get("cache-control"), "no-store");
  assert.equal(first.headers.get("x-mir2-sequence"), "3");
  assert.equal(await first.text(), "packed-feed-3");
  const signature = await fetchPacked(f, "latest.p7s");
  assert.equal(signature.status, 200);
  assert.equal(signature.headers.get("cache-control"), "no-store");
  assert.equal(signature.headers.get("x-mir2-sequence"), "4");
  assert.equal(await signature.text(), "packed-signature-4");
  assert.equal(f.bucket.calls.filter(([op, key]) => op === "get" && key === PACKED_POINTER_KEY).length, 2);
  assert.ok(!f.bucket.calls.some(([, key]) => key === POINTER_KEY || key === FIVE_CLASS_POINTER_KEY));
  assert.equal(f.cache.matches.length, 0);
  assert.equal(f.cache.puts.length, 0);
});

test("packed absent or invalid discovery cannot fall back to another channel", async () => {
  const f = setup();
  f.bucket.put("channels/invited.json", JSON.stringify(pointer(1)));
  f.bucket.put(feedDirectory(pointer(1)) + "latest.json", "legacy-feed");
  assert.equal((await fetchPacked(f, "latest.json")).status, 503);
  putPacked(f, "channels/invited.json", "invalid-pointer");
  assert.equal((await fetchPacked(f, "latest.p7s")).status, 502);
  assert.ok(!f.bucket.calls.some(([, key]) => key === POINTER_KEY));
  putPacked(f, "channels/invited.json", JSON.stringify(pointer(3)));
  putPacked(f, feedDirectory(pointer(3)) + "latest.json", "wrong hash", { customMetadata: { sha256: "f".repeat(64) } });
  assert.equal((await fetchPacked(f, "latest.json")).status, 502);
});

test("packed exact generation controls and content addressed objects are immutable streams", async () => {
  const paths = ["GENERATION.json", "GENERATION.p7s", "catalog.json", "mir2-platform-windows.exe", "mir2-client.toml"].map((name) => packedGeneration + name);
  paths.push(packPath);
  for (const path of paths) {
    const f = setup(); putPacked(f, path, "same-authenticated-bytes");
    const response = await fetchPacked(f, path);
    assert.equal(response.status, 200, path);
    assert.match(response.headers.get("cache-control"), /31536000, immutable/);
    assert.equal(response.headers.get("content-encoding"), null);
    assert.equal(response.headers.get("location"), null);
    assert.equal(await response.text(), "same-authenticated-bytes");
    await f.done();
    const head = await fetchPacked(f, path, {}, "HEAD");
    assert.equal(head.headers.get("x-mir2-native-cache"), "HIT");
    assert.equal(await head.text(), "");
    assert.deepEqual(f.cache.puts, [PACKED_PUBLIC_BASE + path]);
    assert.ok(f.bucket.calls.every(([, key]) => key.startsWith(PACKED_OBJECT_PREFIX)));
  }
});

test("packed HEAD Range ETag and If-Range retain artifact semantics", async () => {
  const f = setup(); putPacked(f, packPath, "0123456789");
  const head = await fetchPacked(f, packPath, { Range: "bytes=2-3" }, "HEAD");
  assert.equal(head.status, 200);
  assert.equal(head.headers.get("content-length"), "10");
  assert.equal(await head.text(), "");
  assert.equal(f.bucket.calls.filter(([op]) => op === "get").length, 0);
  const etag = head.headers.get("etag");
  const partial = await fetchPacked(f, packPath, { Range: "bytes=2-5", "If-Range": etag });
  assert.equal(partial.status, 206);
  assert.equal(partial.headers.get("content-range"), "bytes 2-5/10");
  assert.equal(await partial.text(), "2345");
  const suffix = await fetchPacked(f, packPath, { Range: "bytes=-3" });
  assert.equal(await suffix.text(), "789");
  const unsatisfiable = await fetchPacked(f, packPath, { Range: "bytes=10-" });
  assert.equal(unsatisfiable.status, 416);
  assert.equal(unsatisfiable.headers.get("content-range"), "bytes */10");
  const stale = await fetchPacked(f, packPath, { Range: "bytes=2-5", "If-Range": '"stale"' });
  assert.equal(stale.status, 200);
  assert.equal(await stale.text(), "0123456789");
  const unchanged = await fetchPacked(f, packPath, { "If-None-Match": etag });
  assert.equal(unchanged.status, 304);
  assert.equal(await unchanged.text(), "");
  assert.equal(f.cache.puts.length, 0);
});

test("packed object specific bounds do not expand either legacy metadata limit", async () => {
  for (const [path, maximum] of [
    [packedGeneration + "catalog.json", MAX_PACKED_CATALOG], [packPath, MAX_PACKED_OBJECT],
    [packedGeneration + "GENERATION.json", MAX_PACKED_MANIFEST],
    [packedGeneration + "GENERATION.p7s", MAX_PACKED_SMALL],
    [packedGeneration + "mir2-client.toml", MAX_PACKED_SMALL],
    [packedGeneration + "mir2-platform-windows.exe", MAX_ARTIFACT],
  ]) {
    const f = setup(); putPacked(f, path, "", { declaredSize: maximum });
    const accepted = await fetchPacked(f, path, {}, "HEAD");
    assert.equal(accepted.status, 200, path);
    assert.equal(accepted.headers.get("content-length"), String(maximum));
    f.bucket.rows.get(PACKED_OBJECT_PREFIX + path).declaredSize++;
    assert.equal((await fetchPacked(f, path, {}, "HEAD")).status, 502, path);
    assert.equal(f.bucket.calls.filter(([op]) => op === "get").length, 0);
  }
  const f = setup(), p = pointer(3);
  putPacked(f, "channels/invited.json", JSON.stringify(p));
  putPacked(f, feedDirectory(p) + "latest.json", "", { declaredSize: MAX_PACKED_SMALL + 1 });
  assert.equal((await fetchPacked(f, "latest.json", {}, "HEAD")).status, 502);
  assert.equal(MAX_METADATA, 32 * 1024 * 1024);
  assert.equal(MAX_FIVE_CLASS_METADATA, 64 * 1024 * 1024);
});

test("packed reader refuses extra files private pointers scope escapes and nonlowercase identities before storage", async () => {
  const denied = [
    "packed", "packed/", "p%61cked/latest.json", "packed%2flatest.json", "packed/five-class/latest.json",
    "packed/channels/invited.json", "packed/feeds/s1-a/latest.json", "packed/releases/r1/a.gz", "packed/installers/setup.exe",
    "packed/generations", "packed/generations/" + generationDigest,
    "packed/generations/" + "C".repeat(64) + "/catalog.json", "packed/generations/short/catalog.json",
    ...["PACK-FEED.json", "PACK-FEED.p7s", "BUILD-ATTESTATION.json", "VERSION.json", "PACKAGE-MANIFEST.json", "provenance.json", "catalog.json/child", "catalog.json.", "catalog.json%20", "extra.txt"].map((name) => "packed/" + packedGeneration + name),
    "packed/objects/packs/" + "D".repeat(64) + ".m2p", "packed/objects/packs/" + "d".repeat(63) + ".m2p",
    "packed/" + packPath + "/extra", "packed/objects/packs/" + "d".repeat(64) + ".M2P",
    "packed/objects/packs//" + "d".repeat(64) + ".m2p", "packed/objects/packs/%2e%2e%2fprivate",
    "packed/" + packPath + "?cache=1", "packed/" + packPath + "?", "packed/" + packPath + "#fragment",
  ];
  for (const path of denied) {
    const f = setup();
    assert.equal((await f.fetch(path)).status, 404, path);
    assert.equal(f.bucket.calls.length, 0, path);
  }
  const f = setup();
  for (const method of ["POST", "PUT", "PATCH", "DELETE", "OPTIONS"]) {
    assert.equal((await fetchPacked(f, packPath, {}, method)).status, 405, method);
  }
  assert.equal(f.bucket.calls.length, 0);
});

test("packed large body is returned before its stream finishes without buffering or redirects", async () => {
  const f = setup(); const path = packedGeneration + "catalog.json";
  let controller;
  f.bucket.head = async (key) => {
    assert.equal(key, PACKED_OBJECT_PREFIX + path);
    return { size: 11, httpEtag: '"streaming"', httpMetadata: {} };
  };
  f.bucket.get = async (key) => ({
    ...await f.bucket.head(key),
    body: new ReadableStream({ start(value) { controller = value; controller.enqueue(bytes("first")); } }),
  });
  let timeout;
  try {
    const response = await Promise.race([
      handle(new Request(PACKED_PUBLIC_BASE + path), { MIR2_ASSETS: f.bucket }, f.ctx, null),
      new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error("reader buffered response")), 1000); }),
    ]);
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("location"), null);
    controller.enqueue(bytes("second")); controller.close();
    assert.equal(await response.text(), "firstsecond");
  } finally { clearTimeout(timeout); }
});
