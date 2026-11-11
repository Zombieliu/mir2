import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

const registrarPath = fileURLToPath(new URL("../app/components/asset-cache-registrar.tsx", import.meta.url));
const requestPath = fileURLToPath(new URL("../lib/asset-prewarm-request.ts", import.meta.url));
function loadCode(source, dependencies = {}) {
  const code = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText;
  const module = { exports: {} };
  new Function("module", "exports", "require", ...Object.keys(dependencies), code)(module, module.exports,
    () => { throw Error("Unexpected dependency"); }, ...Object.values(dependencies));
  return module.exports;
}
function loadPrewarmUrls(fetchImpl) {
  const source = readFileSync(registrarPath, "utf8");
  const ast = ts.createSourceFile(registrarPath, source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const functions = ["prewarmUrls", "fetchWithRetry", "createAbortError", "sleep"].map(name =>
    ast.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === name)?.getText(ast) ?? "");
  const request = existsSync(requestPath) ? loadCode(readFileSync(requestPath, "utf8")) : {};
  return loadCode(functions.join("\n") + "\nexports.prewarmUrls = prewarmUrls;", {
    fetch: fetchImpl, publishPrewarmProgress() {},
    isAbortError: error => error?.name === "AbortError",
    fetchPrewarmAsset: (url, options) => request.fetchPrewarmAsset(url, { ...options, fetchImpl }),
  }).prewarmUrls;
}
const turn = () => new Promise(resolve => setImmediate(resolve));
const request = existsSync(requestPath) ? loadCode(readFileSync(requestPath, "utf8")) : {};
const bytes = values => new Response(new Uint8Array(values));
const brokenBody = () => new Response(new ReadableStream({ start(stream) {
  stream.enqueue(new Uint8Array([9]));
  queueMicrotask(() => stream.error(new TypeError("connection terminated")));
} }));

test("public prewarm progress waits for the complete response body", async () => {
  let stream;
  const response = new Response(new ReadableStream({ start(controller) { stream = controller; } }));
  const metric = { requested: 1, ok: 0, failed: 0, failedUrls: [] };
  const run = loadPrewarmUrls(async () => response)(["/original-ui/Prguse/986.png"], metric, {}, 1);
  await turn();
  assert.equal(metric.ok, 0, "200 headers alone must not count as a warmed image");
  stream.enqueue(new Uint8Array([1, 2]));
  await turn();
  assert.equal(metric.ok, 0, "a body prefix must not count as a warmed image");
  stream.enqueue(new Uint8Array([3]));
  stream.close();
  await run;
  assert.equal(metric.ok, 1);
  assert.equal(metric.failed, 0);
});

test("a disconnected body is retried and only the final complete bytes are returned", async () => {
  let calls = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { fetchImpl: async () =>
    ++calls === 1 ? brokenBody() : bytes([1, 2, 3]) });
  assert.equal(calls, 2);
  assert.deepEqual([...result.body], [1, 2, 3]);
});

test("three broken bodies produce one failed prewarm metric and its URL", async () => {
  let calls = 0;
  const metric = { ok: 0, failed: 0, failedUrls: [] };
  await loadPrewarmUrls(async () => { calls++; return brokenBody(); })(["/asset.png"], metric, {}, 1);
  assert.equal(calls, 3);
  assert.deepEqual(metric, { ok: 0, failed: 1, failedUrls: ["/asset.png"] });
});

test("body inactivity cancels the incomplete stream before retrying", async () => {
  let calls = 0, cancelled = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { bodyIdleTimeoutMs: 30, fetchImpl: async () => {
    if (++calls > 1) return bytes([7, 8]);
    return new Response(new ReadableStream({ start(stream) { stream.enqueue(new Uint8Array([1])); },
      cancel() { cancelled++; } }));
  } });
  assert.equal(calls, 2);
  assert.equal(cancelled, 1);
  assert.deepEqual([...result.body], [7, 8]);
});

test("continuing chunks renew the idle deadline instead of imposing a total body deadline", async () => {
  let timer, n = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { bodyIdleTimeoutMs: 200, fetchImpl: async () =>
    new Response(new ReadableStream({ start(stream) {
      timer = setInterval(() => {
        stream.enqueue(new Uint8Array([++n]));
        if (n === 6) { clearInterval(timer); stream.close(); }
      }, 50);
    }, cancel() { clearInterval(timer); } })) });
  assert.deepEqual([...result.body], [1, 2, 3, 4, 5, 6]);
});

test("an owner cancellation cannot turn a cancelled body prefix into success", async () => {
  const owner = new AbortController();
  let cancelled = 0, calls = 0;
  const result = request.fetchPrewarmAsset("/asset.png", { signal: owner.signal, fetchImpl: async () => {
    calls++;
    return new Response(new ReadableStream({ start(stream) {
      stream.enqueue(new Uint8Array([1]));
      setImmediate(() => owner.abort("screen-left"));
    }, cancel() { cancelled++; } }));
  } });
  await assert.rejects(result, { name: "AbortError" });
  assert.equal(cancelled, 1);
  assert.equal(calls, 1);
});

test("an owner cancellation during backoff prevents another request", async () => {
  const owner = new AbortController();
  let calls = 0;
  const result = request.fetchPrewarmAsset("/asset.png", { signal: owner.signal, fetchImpl: async () => {
    calls++;
    setTimeout(() => owner.abort(), 20);
    throw new TypeError("network failed");
  } });
  await assert.rejects(result, { name: "AbortError" });
  assert.equal(calls, 1);
});

test("a header timeout retires and retries the request", async () => {
  let calls = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { headerTimeoutMs: 20,
    fetchImpl: async (_url, init) => {
      if (++calls > 1) return bytes([8]);
      return new Promise((_resolve, reject) => init.signal.addEventListener("abort", () => reject(init.signal.reason), { once: true }));
    } });
  assert.equal(calls, 2);
  assert.deepEqual([...result.body], [8]);
});

test("non-success responses are cancelled, bounded and never counted as warmed", async () => {
  let calls = 0, cancelled = 0;
  const result = await request.fetchPrewarmAsset("/asset.png", { fetchImpl: async () => {
    calls++;
    return new Response(new ReadableStream({ cancel() { cancelled++; } }), { status: 503 });
  } });
  assert.equal(result.response.status, 503);
  assert.equal(result.body, null);
  assert.equal(calls, 3);
  assert.equal(cancelled, 3);
});

test("oversized bodies are cancelled once without retaining or retrying their prefix", async () => {
  let calls = 0, cancelled = 0;
  await assert.rejects(request.fetchPrewarmAsset("/asset.png", { maxBodyBytes: 2, fetchImpl: async () => {
    calls++;
    return new Response(new ReadableStream({ start(stream) { stream.enqueue(new Uint8Array([1, 2, 3])); },
      cancel() { cancelled++; } }));
  } }), { name: "RangeError" });
  assert.equal(calls, 1);
  assert.equal(cancelled, 1);
});
