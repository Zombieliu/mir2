import assert from "node:assert/strict";
import test from "node:test";
import { gzipSync } from "node:zlib";
import worker from "../src/index.ts";

const prefix = "mir2/v/range-test";
const key = `${prefix}/original-ui/fonts/font.otf`;
const url = `https://assets.mir2.obelisk.build/${key}`;
const bytes = (value) => new TextEncoder().encode(value);
const stream = (value) => new ReadableStream({ start(c) { c.enqueue(value); c.close(); } });

class Bucket {
  rows = new Map();
  calls = [];
  alter = null;

  put(path = key, value = bytes("0123456789"), httpMetadata = {}) {
    this.rows.set(path, { value, httpMetadata });
  }

  async get(path, options) {
    this.calls.push({ path, options });
    const row = this.rows.get(path);
    if (!row) return null;
    const size = row.value.length;
    const input = options?.range;
    const offset = input?.offset ?? 0;
    const length = Math.min(input?.length ?? size - offset, size - offset);
    const object = {
      size, httpEtag: '"font-v1"', httpMetadata: row.httpMetadata,
      body: stream(input ? row.value.slice(offset, offset + length) : row.value),
      // R2 returns offset + length, never an `end` property.
      ...(input ? { range: { offset, length } } : {}),
    };
    return this.alter ? this.alter(object) : object;
  }
}

function setup() {
  const bucket = new Bucket();
  const cache = new Map();
  const reads = [], writes = [], pending = [];
  const previous = globalThis.caches;
  globalThis.caches = { default: {
    async match(request) { reads.push(request.url); return cache.get(request.url)?.clone(); },
    async put(request, response) {
      writes.push(request.url);
      const body = await response.arrayBuffer();
      cache.set(request.url, new Response(body, response));
    },
  } };
  const env = {
    MIR2_ASSETS: bucket, MIR2_ALLOWED_PREFIX: "mir2/v/",
    MIR2_OVERLAY_OBJECT_PREFIX: prefix, MIR2_FALLBACK_OBJECT_PREFIX: "mir2/v/base",
  };
  const fetch = (headers = {}, method = "GET", path = url) => worker.fetch(
    new Request(path, { method, headers }), env,
    { waitUntil(promise) { pending.push(promise); } },
  );
  return { bucket, reads, writes, fetch, flush: () => Promise.all(pending),
    async close() { try { await Promise.all(pending); } finally { globalThis.caches = previous; } },
  };
}

for (const [range, contentRange, body] of [
  ["bytes=0-3", "bytes 0-3/10", "0123"],
  ["bytes=4-", "bytes 4-9/10", "456789"],
  ["bytes=8-99", "bytes 8-9/10", "89"],
  ["Bytes=2-4", "bytes 2-4/10", "234"],
  ["bytes=9-9", "bytes 9-9/10", "9"],
]) {
  test(`actual R2 offset/length yields exact ${range}`, async () => {
    const f = setup();
    try {
      f.bucket.put();
      const response = await f.fetch({ Range: range });
      assert.equal(response.status, 206);
      assert.equal(response.headers.get("content-range"), contentRange);
      assert.equal(response.headers.get("content-length"), String(body.length));
      assert.equal(response.headers.get("x-mir2-edge-cache"), "BYPASS");
      assert.equal(response.headers.get("access-control-allow-origin"), "*");
      assert.match(response.headers.get("access-control-expose-headers"), /content-range/);
      assert.equal(await response.text(), body);
      assert.equal(f.reads.length, 0);
      assert.equal(f.writes.length, 0);
    } finally { await f.close(); }
  });
}

test("hotlink alias preserves the same R2 range and overlay fallback", async () => {
  const f = setup();
  try {
    const base = "mir2/v/base/original-ui/fonts/font.otf";
    f.bucket.put(base);
    const response = await f.fetch({ Range: "bytes=2-5" }, "GET",
      url.replace(".build/", ".build/hotlink-ok/"));
    assert.equal(response.status, 206);
    assert.equal(response.headers.get("content-range"), "bytes 2-5/10");
    assert.equal(response.headers.get("x-mir2-fallback-object-key"), base);
    assert.equal(await response.text(), "2345");
    assert.deepEqual(f.bucket.calls.map((call) => call.path), [key, base]);
    assert.deepEqual(f.bucket.calls.map((call) => call.options.range), [
      { offset: 2, length: 4 }, { offset: 2, length: 4 },
    ]);
  } finally { await f.close(); }
});

test("HEAD ignores Range and exposes full size without a partial status", async () => {
  const f = setup();
  try {
    f.bucket.put();
    const response = await f.fetch({ Range: "bytes=0-3" }, "HEAD");
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("content-range"), null);
    assert.equal(response.headers.get("content-length"), "10");
    assert.equal(await response.text(), "");
    assert.equal(f.bucket.calls[0].options, undefined);
    assert.equal(f.writes.length, 0);
  } finally { await f.close(); }
});

for (const range of ["items=0-3", "bytes=3-1", "bytes=0-1,4-5", "bytes=999999999999999999999-"]) {
  test(`unsupported Range stays a complete 200: ${range}`, async () => {
    const f = setup();
    try {
      f.bucket.put();
      const response = await f.fetch({ Range: range });
      assert.equal(response.status, 200);
      assert.equal(response.headers.get("content-range"), null);
      assert.equal(response.headers.get("content-length"), "10");
      assert.equal(await response.text(), "0123456789");
      assert.equal(f.writes.length, 0);
    } finally { await f.close(); }
  });
}

for (const range of [undefined, { offset: 0, length: 4 }, { offset: 2, length: 5 },
  { offset: 2, length: 0 }, { offset: 2, length: NaN }, { offset: 2, length: 99 },
  { offset: 2.5, length: 4 }, { offset: 2, end: 6 }]) {
  test(`invalid returned range cannot publish 206: ${JSON.stringify(range)}`, async () => {
    const f = setup();
    let cancelled = 0;
    try {
      f.bucket.put();
      f.bucket.alter = (object) => ({ ...object, range,
        body: new ReadableStream({ cancel() { cancelled += 1; } }),
      });
      const response = await f.fetch({ Range: "bytes=2-5" });
      assert.equal(response.status, 502);
      assert.equal(response.headers.get("cache-control"), "no-store");
      assert.equal(response.headers.get("content-range"), null);
      assert.equal(await response.text(), "invalid_range_response");
      assert.equal(cancelled, 1);
      assert.equal(f.writes.length, 0);
    } finally { await f.close(); }
  });
}

test("stored gzip range degrades to a complete decoded body without encoded length", async () => {
  const f = setup();
  try {
    const value = bytes('{"ok":"complete original body"}');
    f.bucket.put(key, gzipSync(value), { contentEncoding: "gzip", contentType: "application/json" });
    const response = await f.fetch({ Range: "bytes=0-3" });
    assert.equal(response.status, 200);
    assert.equal(response.headers.get("content-range"), null);
    assert.equal(response.headers.get("content-length"), null);
    assert.equal(response.headers.get("content-encoding"), null);
    assert.equal(response.headers.get("x-mir2-storage-content-encoding"), "gzip");
    assert.deepEqual(new Uint8Array(await response.arrayBuffer()), value);
    assert.equal(f.bucket.calls.length, 2);
    assert.equal(f.bucket.calls[1].options, undefined);
    assert.equal(f.writes.length, 0);
  } finally { await f.close(); }
});

test("whole immutable object keeps canonical/hotlink cache", async () => {
  const f = setup();
  try {
    f.bucket.put();
    const miss = await f.fetch();
    assert.equal(miss.status, 200);
    assert.equal(miss.headers.get("content-length"), "10");
    assert.equal(await miss.text(), "0123456789");
    await f.flush();
    const hit = await f.fetch({}, "GET", url.replace(".build/", ".build/hotlink-ok/"));
    assert.equal(hit.headers.get("x-mir2-edge-cache"), "HIT");
    assert.equal(await hit.text(), "0123456789");
    assert.equal(f.writes.length, 1);
    assert.equal(f.writes[0], url);
  } finally { await f.close(); }
});

test("uncached conditional HEAD remains bodyless 304", async () => {
  const f = setup();
  try {
    f.bucket.put();
    const conditional = await f.fetch({ "If-None-Match": '"font-v1"' }, "HEAD");
    assert.equal(conditional.status, 304);
    assert.equal(conditional.headers.get("content-length"), null);
    assert.equal(await conditional.text(), "");
  } finally { await f.close(); }
});
