import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import ts from "typescript";

function loadTypeScript(file, scope = {}) {
  const compiled = ts.transpileModule(readFileSync(new URL(`../lib/${file}`, import.meta.url), "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, strict: true },
    fileName: file,
    reportDiagnostics: true,
  });
  assert.deepEqual((compiled.diagnostics ?? []).filter((item) => item.category === ts.DiagnosticCategory.Error), []);
  const exports = {};
  new Function("exports", ...Object.keys(scope), compiled.outputText)(exports, ...Object.values(scope));
  return exports;
}

const { fetchFirstCrystalSceneJson } = loadTypeScript("first-crystal-scene-request.ts");
const crystalUrl = "/api/scene/crystal?v=test&assetRelease=release-a&map=0&x=296&y=620&width=56&height=72";
const options = { cache: "force-cache" };
const unavailable = () => new Response(null, { status: 503 });
const ready = () => Response.json({ map: "0", ready: true }, { headers: { "x-mir2-source": "crystal" } });

function deferred() {
  let resolve;
  const promise = new Promise((done) => { resolve = done; });
  return { promise, resolve };
}

function streamedScene() {
  let body;
  const cancellations = [];
  const stream = new ReadableStream({
    start(controller) { body = controller; },
    cancel(reason) { cancellations.push(reason); },
  });
  return {
    response: new Response(stream, { headers: { "x-mir2-source": "crystal" } }),
    write(text) { body.enqueue(new TextEncoder().encode(text)); },
    close() { body.close(); },
    cancellations,
  };
}

async function tick(t, ms) {
  t.mock.timers.tick(ms);
  for (let turns = 0; turns < 20; turns += 1) await Promise.resolve();
}

for (const stallAt of ["headers", "json body"]) {
  test(`first-scene deadline covers stalled ${stallAt} and retries the exact URL once`, async (t) => {
    t.mock.timers.enable({ apis: ["setTimeout"] });
    const hanging = deferred();
    const calls = [];
    const owner = new AbortController();
    const result = fetchFirstCrystalSceneJson(crystalUrl, { ...options, signal: owner.signal }, (url, init) => {
      calls.push({ url, ...init });
      if (calls.length > 1) return Promise.resolve(ready());
      return stallAt === "headers" ? hanging.promise : Promise.resolve({ ok: true, json: () => hanging.promise });
    });

    await tick(t, 0);
    await tick(t, 7_999);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].signal.aborted, false);
    await tick(t, 1);
    assert.equal(calls[0].signal.reason.name, "TimeoutError");
    assert.equal(owner.signal.aborted, false);
    await tick(t, 199);
    assert.equal(calls.length, 1);
    await tick(t, 1);
    assert.equal(calls.length, 2);
    assert.deepEqual(calls.map(({ url, cache }) => ({ url, cache })), [
      { url: crystalUrl, ...options }, { url: crystalUrl, cache: "reload" },
    ]);
    assert.notEqual(calls[0].signal, calls[1].signal);
    const accepted = await result;
    assert.deepEqual(accepted.body, { map: "0", ready: true });
    assert.equal(accepted.response.headers.get("x-mir2-source"), "crystal");
    hanging.resolve(stallAt === "headers" ? ready() : { map: "stale" });
    await tick(t, 60_000);
    assert.equal(calls.length, 2);
  });
}

test("slow headers leave a separate deadline for valid scene JSON", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const headers = deferred();
  const body = deferred();
  const requests = [];
  const result = fetchFirstCrystalSceneJson(crystalUrl, options, (url, init) => {
    requests.push({ url, ...init });
    return headers.promise;
  });

  await tick(t, 5_600);
  headers.resolve({ ok: true, json: () => body.promise });
  await tick(t, 0);
  await tick(t, 4_500);
  assert.equal(requests.length, 1);
  assert.equal(requests[0].signal.aborted, false);
  body.resolve({ map: "0", ready: true });
  assert.deepEqual((await result).body, { map: "0", ready: true });
  await tick(t, 60_000);
  assert.equal(requests.length, 1);
});

test("a progressing scene body can take more than eight seconds after slow headers", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const headers = deferred();
  const scene = streamedScene();
  const calls = [];
  const result = fetchFirstCrystalSceneJson(crystalUrl, options, (_, init) => {
    calls.push(init);
    return headers.promise;
  });
  await tick(t, 4_500);
  headers.resolve(scene.response);
  await tick(t, 0);
  for (const part of ['{"map":', '"0",', '"name":"比奇",', '"ready":true}']) {
    await tick(t, 3_000);
    scene.write(part);
    await tick(t, 0);
    assert.equal(calls[0].signal.aborted, false);
  }
  await tick(t, 1_000);
  scene.close();
  const { response, body } = await result;
  assert.equal(response, scene.response);
  assert.deepEqual(body, { map: "0", name: "比奇", ready: true });
  await tick(t, 60_000);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].signal.aborted, false);
});

test("a stalled body retries eight seconds after its last bytes and ignores empty chunks", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const scene = streamedScene();
  const calls = [];
  const result = fetchFirstCrystalSceneJson(crystalUrl, options, async (url, init) => {
    calls.push({ url, ...init });
    return calls.length === 1 ? scene.response : ready();
  });
  await tick(t, 4_000);
  scene.write('{"map":');
  await tick(t, 0);
  await tick(t, 7_999);
  scene.write("");
  await tick(t, 0);
  assert.equal(calls[0].signal.aborted, false);
  await tick(t, 1);
  assert.equal(calls[0].signal.reason.name, "TimeoutError");
  await tick(t, 200);
  assert.equal(calls.length, 2);
  assert.equal(calls[1].url, crystalUrl);
  assert.equal(calls[1].cache, "reload");
  assert.deepEqual((await result).body, { map: "0", ready: true });
  assert.deepEqual(scene.cancellations, [calls[0].signal.reason]);
});

test("continuous body progress cannot exceed the absolute scene body limit", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const scene = streamedScene();
  const calls = [];
  const result = fetchFirstCrystalSceneJson(crystalUrl, options, async (_, init) => {
    calls.push(init);
    return calls.length === 1 ? scene.response : ready();
  });
  await tick(t, 0);
  for (const part of ['{"map":', '"0",', '"ready":']) {
    await tick(t, 7_500);
    scene.write(part);
    await tick(t, 0);
  }
  await tick(t, 1_499);
  assert.equal(calls[0].signal.aborted, false);
  await tick(t, 1);
  assert.equal(calls[0].signal.reason.name, "TimeoutError");
  await tick(t, 200);
  assert.deepEqual((await result).body, { map: "0", ready: true });
  assert.equal(calls.length, 2);
  assert.deepEqual(scene.cancellations, [calls[0].signal.reason]);
});

test("a body that stalls after slow headers still retries the same URL", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const headers = deferred();
  const requests = [];
  const result = fetchFirstCrystalSceneJson(crystalUrl, options, (url, init) => {
    requests.push({ url, ...init });
    return requests.length === 1 ? headers.promise : Promise.resolve(ready());
  });

  await tick(t, 5_600);
  headers.resolve({ ok: true, json: () => new Promise(() => {}) });
  await tick(t, 0);
  await tick(t, 7_999);
  assert.equal(requests[0].signal.aborted, false);
  await tick(t, 1);
  assert.equal(requests[0].signal.reason.name, "TimeoutError");
  await tick(t, 200);
  assert.equal(requests.length, 2);
  assert.equal(requests[1].url, crystalUrl);
  assert.equal(requests[1].cache, "reload");
  assert.deepEqual((await result).body, { map: "0", ready: true });
});

test("first-scene transport and transient HTTP failures get only one retry", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let calls = 0;
  const result = fetchFirstCrystalSceneJson(crystalUrl, options, async () => {
    calls += 1;
    if (calls === 1) throw new TypeError("body stream was interrupted");
    return unavailable();
  });
  await tick(t, 0);
  await tick(t, 200);
  assert.equal((await result).response.status, 503);
  await tick(t, 60_000);
  assert.equal(calls, 2);
});

test("both stalled first-scene attempts have finite deadlines", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const signals = [];
  const pending = fetchFirstCrystalSceneJson(crystalUrl, options, (_, init) => {
    signals.push(init.signal);
    return new Promise(() => {});
  });
  const rejected = assert.rejects(pending, { name: "TimeoutError" });
  await tick(t, 8_000);
  await tick(t, 200);
  await tick(t, 8_000);
  await rejected;
  await tick(t, 60_000);
  assert.deepEqual(signals.map((signal) => signal.reason.name), ["TimeoutError", "TimeoutError"]);
});

test("definitive HTTP errors, Retry-After and complete malformed JSON do not retry", async () => {
  const calls = [];
  for (const status of [401, 403, 404, 429, 503]) {
    const response = await fetchFirstCrystalSceneJson(crystalUrl, options, async () => {
      calls.push(status);
      return new Response("not parsed", { status, headers: status >= 429 ? { "Retry-After": "15" } : {} });
    });
    assert.equal(response.response.status, status);
    assert.equal(response.body, null);
  }
  await assert.rejects(fetchFirstCrystalSceneJson(crystalUrl, options, async () => {
    calls.push(200);
    return new Response("{malformed");
  }), SyntaxError);
  assert.deepEqual(calls, [401, 403, 404, 429, 503, 200]);
});

test("a typed Crystal dependency failure returns its original 424 response and JSON", async () => {
  const resourceMissing = { error: "resource_missing", mapFileName: "0", publicPath: "/original-map/Tiles/1.png" };
  const result = await fetchFirstCrystalSceneJson(crystalUrl, options,
    async () => Response.json(resourceMissing, { status: 424 }));
  assert.equal(result.response.status, 424);
  assert.deepEqual(result.body, resourceMissing);
});

for (const cancelAt of ["body", "backoff"]) {
  test(`the scene owner cancels during ${cancelAt} without launching a replacement`, async (t) => {
    t.mock.timers.enable({ apis: ["setTimeout"] });
    const owner = new AbortController();
    const body = deferred();
    const calls = [];
    const pending = fetchFirstCrystalSceneJson(crystalUrl, { ...options, signal: owner.signal }, async (_, init) => {
      calls.push(init.signal);
      return cancelAt === "body" ? { ok: true, json: () => body.promise } : unavailable();
    });
    await tick(t, 0);
    const abort = new DOMException("The character changed maps", "AbortError");
    const rejected = assert.rejects(pending, (error) => error === abort);
    owner.abort(abort);
    await rejected;
    body.resolve({ map: "old" });
    await tick(t, 60_000);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].reason, abort);
  });
}

test("the injected manifest reader keeps a cancelled or older release out of accepted memory", async () => {
  const browser = {};
  const { loadAssetReleaseCapabilities } = loadTypeScript("asset-release-capabilities.ts", { window: browser });
  const first = deferred();
  const old = loadAssetReleaseCapabilities(() => first.promise, { fresh: true, readJson: fetchFirstCrystalSceneJson });
  const latest = await loadAssetReleaseCapabilities(async (url, init) => {
    assert.equal(url, "/api/asset-manifest");
    assert.equal(init.cache, "no-cache");
    assert(init.signal instanceof AbortSignal);
    return Response.json({ version: "latest" });
  }, { fresh: true, readJson: fetchFirstCrystalSceneJson });
  first.resolve(Response.json({ version: "late" }));
  assert.equal(latest.releaseId, "latest");
  assert.equal((await old).releaseId, "late");
  assert.equal(browser.__mir2AssetReleaseCapabilities.releaseId, "latest");

  const owner = new AbortController();
  const result = deferred();
  const cancelled = loadAssetReleaseCapabilities(() => result.promise, {
    fresh: true, signal: owner.signal, readJson: fetchFirstCrystalSceneJson,
  });
  owner.abort();
  result.resolve(Response.json({ version: "cancelled" }));
  assert.equal((await cancelled).releaseId, null);
  assert.equal(browser.__mir2AssetReleaseCapabilities.releaseId, "latest");
});
