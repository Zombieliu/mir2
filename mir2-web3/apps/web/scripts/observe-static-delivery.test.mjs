import assert from "node:assert/strict";
import {EventEmitter} from "node:events";
import test from "node:test";
import {observeStaticDelivery, staticDeliveryUrl} from "./lib/observe-static-delivery.mjs";

const request = (overrides = {}) => ({method: () => "GET", url: () => "https://numeron.obelisk.build/original-ui/Prguse/1.png",
  serviceWorker: () => null, resourceType: () => "image", redirectedFrom: () => null,
  timing: () => ({startTime: 1000, requestStart: 10, responseStart: 20, responseEnd: 50, secret: "PRIVATE_SENTINEL"}),
  sizes: async () => ({responseBodySize: 123, responseHeadersSize: 45, secret: "PRIVATE_SENTINEL"}), ...overrides});
const fixture = options => {
  const context = new EventEmitter(); let clock = 0;
  const observer = observeStaticDelivery({context, elapsed: () => clock++, ...options});
  return {context, observer};
};

test("static URL allowlist strips queries and fragments while excluding auth and unknown origins", () => {
  for (const origin of ["https://numeron.obelisk.build", "https://assets.mir2.obelisk.build",
    "https://pub-72ec6e670a8346d1a6b2177df2643326.r2.dev", "http://127.0.0.1:1234"]) {
    assert.equal(staticDeliveryUrl(origin + "/bevy-runtime/x/runtime.wasm?token=PRIVATE_SENTINEL#private"),
      origin + "/bevy-runtime/x/runtime.wasm");
  }
  assert.equal(staticDeliveryUrl("https://assets.mir2.obelisk.build/mir2/v/20261108-r16-08ec86d5105c/bevy-runtime/v/bevy-0c553a754308e5b8/pkg-webgpu/mir2_bevy_runtime_bg.wasm?rev=123"),
    "https://assets.mir2.obelisk.build/mir2/v/20261108-r16-08ec86d5105c/bevy-runtime/v/bevy-0c553a754308e5b8/pkg-webgpu/mir2_bevy_runtime_bg.wasm");
  assert.ok(staticDeliveryUrl("https://numeron.obelisk.build/client-core/pinned/mir2_platform_web_bg.wasm"));
  assert.ok(staticDeliveryUrl("https://numeron.obelisk.build/generated/crystal-packs/full/monsters.pack"));
  for (const url of ["https://numeron.obelisk.build/api/wallet-login/intent?private=1",
    "https://wallet.dubhe.obelisk.build/connect/approve", "https://other.test/original-ui/1.png",
    "https://user:pass@numeron.obelisk.build/game", "https://numeron.obelisk.build/api/characters/me", "nonsense"]) {
    assert.equal(staticDeliveryUrl(url), null);
  }
});

test("page and Worker requests remain separate even when they fetch the same static URL", async () => {
  const {context, observer} = fixture();
  const page = request(), worker = request({serviceWorker: () => ({})});
  context.emit("request", page); context.emit("request", worker);
  context.emit("response", {request: () => page, status: () => 200, fromServiceWorker: () => true});
  context.emit("requestfinished", page); context.emit("requestfinished", worker);
  const result = await observer.finish();
  assert.equal(result.rows.length, 2);
  assert.deepEqual(result.rows.map(row => row.source), ["page", "service-worker-upstream"]);
  assert.equal(result.rows[0].fromServiceWorker, true);
  assert.equal(result.rows[1].sizes.responseBodySize, 123);
  assert.doesNotMatch(JSON.stringify(result), /PRIVATE_SENTINEL/);
});

test("redirect source, failures and unfinished requests preserve different terminal states", async () => {
  const {context, observer} = fixture();
  const first = request(), redirected = request({redirectedFrom: () => first, failure: () => ({errorText: "net::ERR_ABORTED"})});
  context.emit("request", first); context.emit("requestfinished", first);
  context.emit("request", redirected); context.emit("requestfailed", redirected);
  context.emit("request", request());
  const result = await observer.finish();
  assert.equal(result.rows[1].redirectedFrom, 1);
  assert.equal(result.rows[1].failure, "net::ERR_ABORTED");
  assert.equal(result.rows[1].finishedAtMs, undefined);
  assert.equal(result.rows[2].incomplete, true);
  assert.equal(result.rows[0].incomplete, undefined);
});

test("request cap and excluded POSTs are visible without exporting private failure text", async () => {
  const {context, observer} = fixture({maxRows: 1});
  context.emit("request", request({method: () => "POST"}));
  const value = request({failure: () => ({errorText: "PRIVATE_SENTINEL"})});
  context.emit("request", value); context.emit("requestfailed", value);
  context.emit("request", request());
  const result = await observer.finish();
  assert.equal(result.rows.length, 1);
  assert.equal(result.droppedRequests, 1);
  assert.equal(result.rows[0].failure, "OTHER_NETWORK_ERROR");
  assert.doesNotMatch(JSON.stringify(result), /PRIVATE_SENTINEL/);
});

test("stalled size reads are bounded and cannot mutate the result after finish", async () => {
  const {context, observer} = fixture({finishMs: 10});
  let release;
  const value = request({sizes: () => new Promise(resolve => {release = resolve;})});
  context.emit("request", value); context.emit("requestfinished", value);
  const result = await observer.finish();
  assert.equal(result.sizeReadsDrained, false);
  assert.equal(result.pendingSizesAtFinish, 1);
  const before = JSON.stringify(result);
  release({responseBodySize: 999}); await new Promise(resolve => setImmediate(resolve));
  assert.equal(JSON.stringify(result), before);
  assert.deepEqual(context.eventNames(), []);
  assert.equal(await observer.finish(), result);
});

test("missing request sizes remains unknown and does not become a zero-byte completed download", async () => {
  const {context, observer} = fixture();
  const value = request({sizes: async () => {throw Error("private");}});
  context.emit("request", value); context.emit("requestfinished", value);
  const result = await observer.finish();
  assert.equal(result.rows[0].sizes, undefined);
  assert.equal(result.rows[0].sizesUnavailable, true);
  assert.equal(result.observationErrors, 1);
});
