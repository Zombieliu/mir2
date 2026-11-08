import assert from "node:assert/strict";
import { test } from "node:test";
import worker, { KEY, SHA256, SIZE } from "./worker.mjs";

const sourceUrl = "https://release-assets.githubusercontent.com/github-production-release-asset/approved";
const valid = () => ({ size: SIZE, checksums: { sha256: Uint8Array.from(Buffer.from(SHA256, "hex")).buffer } });
const req = (source = sourceUrl, auth = "Bearer test-secret") => new Request("https://upload.example/publish", { method: "POST", headers: { authorization: auth }, body: JSON.stringify({ sourceUrl: source }) });
const env = (APK) => ({ APK, UPLOAD_SECRET: "test-secret", EXPIRES_AT: String(Date.now() + 100000) });

test("rejects unauthenticated and expired calls before bucket access", async () => {
  assert.equal((await worker.fetch(req(sourceUrl, "bad"), env(null))).status, 401);
  assert.equal((await worker.fetch(req(), { ...env(null), EXPIRES_AT: "1" })).status, 403);
});
test("rejects foreign, deceptive, credential-bearing and insecure source URLs", async () => {
  for (const url of ["https://example.com/file", "https://release-assets.githubusercontent.com.evil.test/file", "http://release-assets.githubusercontent.com/file", "https://user:password@release-assets.githubusercontent.com/file"]) {
    assert.equal((await worker.fetch(req(url), env(null))).status, 400);
  }
});
test("existing object must have an independently stored SHA256", async () => {
  const bad = { size: SIZE, customMetadata: { sha256: SHA256 } };
  assert.equal((await worker.fetch(req(), env({ head: async () => bad }))).status, 409);
  const response = await worker.fetch(req(), env({ head: async () => valid() }));
  assert.equal(response.status, 200);
  assert.equal((await response.json()).existing, true);
});
test("streams only pinned bytes with server checksum and create-only condition", async () => {
  const previous = globalThis.fetch;
  let putCalled = false;
  globalThis.fetch = async (_url, options) => {
    assert.equal(options.redirect, "error");
    return new Response("fixture", { headers: { "content-length": String(SIZE) } });
  };
  try {
    const response = await worker.fetch(req(), env({
      head: async () => null,
      put: async (key, body, options) => {
        putCalled = true;
        assert.equal(key, KEY);
        assert.ok(body instanceof ReadableStream);
        assert.equal(options.sha256, SHA256);
        assert.equal(options.onlyIf.get("if-none-match"), "*");
        assert.equal(options.httpMetadata.contentType, "application/vnd.android.package-archive");
        return valid();
      },
    }));
    assert.equal(response.status, 200);
    assert.equal(putCalled, true);
  } finally { globalThis.fetch = previous; }
});
test("rejects wrong upstream size before any R2 write", async () => {
  const previous = globalThis.fetch;
  globalThis.fetch = async () => new Response("bad", { headers: { "content-length": "3" } });
  try {
    const response = await worker.fetch(req(), env({ head: async () => null, put: async () => assert.fail("unexpected write") }));
    assert.equal(response.status, 502);
  } finally { globalThis.fetch = previous; }
});
test("refuses overwrite if another writer wins the create-only condition", async () => {
  const previous = globalThis.fetch;
  globalThis.fetch = async () => new Response("fixture", { headers: { "content-length": String(SIZE) } });
  try { assert.equal((await worker.fetch(req(), env({ head: async () => null, put: async () => null }))).status, 409); }
  finally { globalThis.fetch = previous; }
});
