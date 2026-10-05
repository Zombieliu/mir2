import test from "node:test";
import assert from "node:assert/strict";
import { handle, OBJECT_PREFIX, POINTER_KEY, PUBLIC_BASE, MAX_ARTIFACT, MAX_METADATA, MAX_POINTER, feedDirectory } from "../src/index.mjs";

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
