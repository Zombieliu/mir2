import test from "node:test";
import assert from "node:assert/strict";
import { createHash, timingSafeEqual } from "node:crypto";
import { createImporter } from "../src/importer.mjs";

Object.defineProperty(crypto.subtle, "timingSafeEqual", { configurable: true, value: (left, right) => timingSafeEqual(Buffer.from(left), Buffer.from(right)) });
const TOKEN = "a".repeat(64);
const HOST = "https://mir2-full-installer-import.zombiesliu.workers.dev";
const sha = value => createHash("sha256").update(value).digest("hex");
const checksum = value => Uint8Array.from(Buffer.from(value, "hex")).buffer;
function fixture() {
  const data = new TextEncoder().encode("MZ-exact-verified-installer");
  const identity = sha(data);
  const prefix = "mir2/native/windows-full-installers/" + identity + "/";
  const part = { offset: 0, bytes: data.length, sha256: sha(data), key: prefix + "parts/" + sha(data) + ".bin" };
  const files = ["release.json", "release.p7s", "SHA256SUMS.txt", "README.zh-CN.txt"].map(name => ({ name, bytes: data.length, sha256: sha(data), key: prefix + "files/" + name, contentType: "application/octet-stream" }));
  return { data, release: { schema: 1, sha256: identity, bytes: data.length, parts: [part], files } };
}
function request(path = "/import/0", options = {}) {
  return new Request(HOST + path, { method: "POST", headers: { authorization: "Bearer " + TOKEN, ...options.headers }, ...options });
}
function storage(release) {
  const objects = new Map();
  const calls = [];
  return {
    objects, calls,
    async head(key) { calls.push(["head", key]); return objects.get(key) ?? null; },
    async put(key, body, options) {
      calls.push(["put", key, body, options]);
      assert.equal(options.onlyIf.get("if-none-match"), "*");
      if (objects.has(key)) return null;
      const reader = body.getReader();
      const hasher = createHash("sha256");
      let length = 0;
      try {
        for (;;) { const next = await reader.read(); if (next.done) break; hasher.update(next.value); length += next.value.byteLength; }
      } finally { reader.releaseLock(); }
      const actual = hasher.digest("hex");
      if (actual !== options.sha256) throw new Error("native_checksum_mismatch");
      const object = { size: length, checksums: { sha256: checksum(actual) }, customMetadata: options.customMetadata, httpMetadata: options.httpMetadata };
      objects.set(key, object);
      return object;
    },
  };
}
function source(data, entry, total, overrides = {}) {
  return new Response(data, { status: 206, headers: { "content-length": String(entry.bytes), "content-range": `bytes ${entry.offset}-${entry.offset + entry.bytes - 1}/${total}`, ...overrides } });
}

test("imports only the pinned range, original body and native checksum with create-once", async () => {
  const { data, release } = fixture();
  const bucket = storage(release);
  const origin = source(data, release.parts[0], release.bytes);
  const originalBody = origin.body;
  const handler = createImporter(release, async (url, options) => {
    assert.equal(url, `https://165.154.65.136.sslip.io/client-updates/installers/${release.sha256}/Mir2Setup.exe`);
    assert.equal(options.redirect, "error");
    assert.equal(options.headers.get("range"), `bytes=0-${data.length - 1}`);
    assert.equal(options.headers.get("accept-encoding"), "identity");
    return origin;
  });
  const response = await handler(request(), { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN });
  assert.equal(response.status, 200);
  assert.equal((await response.json()).passed, true);
  const put = bucket.calls.find(call => call[0] === "put");
  assert.equal(put[2], originalBody);
  assert.equal(put[3].sha256, release.parts[0].sha256);
  assert.equal(put[3].customMetadata.sourceInstaller, release.sha256);
});

test("authorization and exact import paths reject before storage/network access", async () => {
  const { release } = fixture();
  const handler = createImporter(release, async () => assert.fail("network must not be reached"));
  const bucket = storage(release);
  const env = { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN };
  for (const value of [request("/import/0", { headers: {} }), request("/import/0", { headers: { authorization: "Bearer " + "b".repeat(64) } }), request("/import/0?url=https://example.com"), request("/import/%30"), request("/import/00"), request("/import/99"), request("/import/0", { method: "GET" }), new Request("https://example.com/import/0", { method: "POST" })]) {
    assert.ok([401, 404].includes((await handler(value, env)).status));
  }
  assert.equal(bucket.calls.length, 0);
});

test("request bodies cannot supply URLs, keys or instructions", async () => {
  const { release } = fixture();
  const handler = createImporter(release, async () => assert.fail("network must not be reached"));
  const bucket = storage(release);
  const response = await handler(request("/import/0", { body: "https://example.com", headers: { authorization: "Bearer " + TOKEN } }), { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN });
  assert.equal(response.status, 400);
  assert.equal(bucket.calls.length, 0);
});

for (const [name, override] of [["wrong length", { "content-length": "1" }], ["wrong range", { "content-range": "bytes 1-2/3" }], ["compression", { "content-encoding": "gzip" }]]) {
  test(`origin ${name} fails before any write and cancels its body`, async () => {
    const { release } = fixture();
    let canceled = false;
    const body = new ReadableStream({ cancel() { canceled = true; } });
    const response = source(body, release.parts[0], release.bytes, override);
    const bucket = storage(release);
    const result = await createImporter(release, async () => response)(request(), { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN });
    assert.equal(result.status, 502);
    assert.equal(bucket.calls.some(call => call[0] === "put"), false);
    assert.equal(canceled, true);
  });
}

test("native hash failure cannot publish a wrong body", async () => {
  const { data, release } = fixture();
  const corrupt = data.slice(); corrupt[2] ^= 1;
  const bucket = storage(release);
  const result = await createImporter(release, async () => source(corrupt, release.parts[0], release.bytes))(request(), { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN });
  assert.equal(result.status, 502);
  assert.equal(bucket.objects.size, 0);
});

test("a verified immutable object is idempotent without another origin fetch", async () => {
  const { data, release } = fixture();
  const bucket = storage(release);
  let fetches = 0;
  const handler = createImporter(release, async () => { fetches++; return source(data, release.parts[0], release.bytes); });
  const env = { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN };
  assert.equal((await handler(request(), env)).status, 200);
  const again = await handler(request(), env);
  assert.equal(again.status, 200);
  assert.equal((await again.json()).alreadyPresent, true);
  assert.equal(fetches, 1);
});

test("forged custom metadata cannot replace a mismatching native checksum", async () => {
  const { release } = fixture();
  const bucket = storage(release);
  const part = release.parts[0];
  bucket.objects.set(part.key, { size: part.bytes, customMetadata: { sha256: part.sha256, sourceInstaller: release.sha256 }, checksums: { sha256: checksum("f".repeat(64)) } });
  const handler = createImporter(release, async () => assert.fail("must not overwrite"));
  assert.equal((await handler(request(), { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN })).status, 409);
  assert.equal(bucket.calls.some(call => call[0] === "put"), false);
});

test("only the four pinned companion files can be imported as whole objects", async () => {
  const { data, release } = fixture();
  const bucket = storage(release);
  const handler = createImporter(release, async (url, options) => {
    assert.equal(url.endsWith("/release.json"), true);
    assert.equal(options.headers.has("range"), false);
    return new Response(data, { headers: { "content-length": String(data.length) } });
  });
  const result = await handler(request("/import/1"), { MIR2_ASSETS: bucket, IMPORT_TOKEN: TOKEN });
  assert.equal(result.status, 200);
});

test("manifest cannot escape its release prefix or admit oversized/gapped parts", () => {
  const { release } = fixture();
  for (const mutate of [r => r.parts[0].key = "foreign/object", r => r.parts[0].offset = 1, r => r.parts[0].bytes = 64 * 1024 * 1024 + 1, r => r.files[0].name = "../private", r => r.files[0].bytes = 32769]) {
    const changed = structuredClone(release); mutate(changed);
    assert.throws(() => createImporter(changed));
  }
});
