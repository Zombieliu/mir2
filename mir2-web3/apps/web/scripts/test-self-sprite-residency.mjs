import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

const modules = new Map();
function load(path) {
  if (modules.has(path)) return modules.get(path).exports;
  const module = { exports: {} }; modules.set(path, module);
  const code = ts.transpileModule(readFileSync(path, "utf8"), {
    fileName: path, compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  }).outputText;
  new Function("require", "module", "exports", code)(
    name => load(resolve(dirname(path), name + ".ts")), module, module.exports);
  return module.exports;
}
const lib = fileURLToPath(new URL("../lib/", import.meta.url));
const { SelfSpriteResidency } = load(resolve(lib, "self-sprite-residency.ts"));
const { loadSelfSpriteImage, SelfSpriteImageError } = load(resolve(lib, "self-sprite-image.ts"));
const settle = async () => { for (let i = 0; i < 4; i++) await new Promise(resolve => setImmediate(resolve)); };
const layer = (source, role = "body", x = 7) => ({ source, role, path: "/" + source + ".png",
  width: 1, height: 1, x, y: -9, shadowX: 0, shadowY: 0, maskPath: null });
const pose = (key, layers = [layer(key)]) => ({ key, layers, nameplateTop: -30 });
function fixture(limits, now = Date.now) {
  const requests = [], released = [];
  const actor = new SelfSpriteResidency((frame, signal) => new Promise((resolve, reject) => {
    const request = { frame, signal, reject, finish() {
      resolve({ url: "blob:" + frame.source, bytes: 10, release: () => released.push(frame.source) });
    } }; requests.push(request);
  }), now, limits);
  actor.start();
  return { actor, requests, released, complete: async source => {
    const request = requests.find(row => row.frame.source === source && !row.done);
    assert(request, source); request.done = true; request.finish(); await settle();
  } };
}

test("changing animation targets retains requests and only publishes a complete current pose", async () => {
  const f = fixture();
  f.actor.setTarget(pose("a", [layer("a"), layer("hair-a", "hair")]));
  f.actor.setTarget(pose("b", [layer("b", "body", 42), layer("hair-b", "hair", 17)]));
  assert.equal(f.requests.length, 4);
  assert(f.requests.every(row => !row.signal.aborted));
  await f.complete("a"); await f.complete("hair-a");
  assert.equal(f.actor.getSnapshot(), null, "late old-pose completion is only warm cache");
  await f.complete("b"); assert.equal(f.actor.getSnapshot(), null, "body must not borrow another pose's hair");
  await f.complete("hair-b");
  const ready = f.actor.getSnapshot();
  assert.equal(ready.key, "b"); assert.equal(ready.layers[0].x, 42); assert.equal(ready.layers[1].x, 17);
  f.actor.presentedPose(ready);
  f.actor.setTarget(pose("walk"));
  assert.equal(f.actor.getSnapshot(), ready, "keep decoded image and its geometry while walking PNG is pending");
  await f.complete("walk"); assert.equal(f.actor.getSnapshot().key, "walk");
  f.actor.stop(); assert(f.requests.every(row => row.signal.aborted || row.done));
});

test("latest demand replaces queued frames without canceling the bounded in-flight reads", async () => {
  const f = fixture({ entries: 64, bytes: 10000, network: 1, local: 1 });
  for (const key of ["old", "queued", "latest"]) f.actor.setTarget(pose(key));
  assert.equal(f.requests.length, 1); assert.equal(f.requests[0].signal.aborted, false);
  await f.complete("old");
  assert.deepEqual(f.requests.map(row => row.frame.source), ["old", "latest"]);
  await f.complete("latest"); assert.equal(f.actor.getSnapshot().key, "latest"); f.actor.stop();
});

test("a verified local pack can take over while four network reads remain pending", async () => {
  const f = fixture(); f.actor.setTarget(pose("net", [layer("n1"), layer("n2", "hair")]), [layer("n3"), layer("n4")]);
  assert.equal(f.actor.stats().network, 4);
  f.actor.setTarget(pose("pack", [layer("data:body"), layer("data:hair", "hair")]));
  assert.equal(f.actor.stats().network, 4); assert.equal(f.actor.stats().local, 2);
  await f.complete("data:body"); await f.complete("data:hair");
  assert.equal(f.actor.getSnapshot().key, "pack");
  f.actor.stop(); assert(f.requests.slice(0, 4).every(row => row.signal.aborted));
});

test("LRU eviction never revokes a URL still presented by React", async () => {
  const f = fixture({ entries: 3, bytes: 30, network: 4, local: 2 });
  f.actor.setTarget(pose("a")); await f.complete("a"); f.actor.presentedPose(f.actor.getSnapshot());
  f.actor.setTarget(pose("b")); await f.complete("b");
  f.actor.setTarget(pose("c")); await f.complete("c");
  assert(!f.released.includes("a"));
  f.actor.presentedPose(f.actor.getSnapshot());
  f.actor.setTarget(pose("d")); await f.complete("d");
  assert(f.released.includes("a")); assert(!f.released.includes("c"));
  assert(f.actor.stats().cached <= 3); assert(f.actor.stats().bytes <= 30); f.actor.stop();
});

test("owner retirement and StrictMode restart reject late completion from the previous generation", async () => {
  const f = fixture(); f.actor.setTarget(pose("same")); const old = f.requests[0];
  f.actor.stop(); assert(old.signal.aborted); f.actor.start(); f.actor.setTarget(pose("same"));
  old.finish(); await settle(); assert.equal(f.actor.getSnapshot(), null); assert.equal(f.actor.stats().active, 1);
  assert.deepEqual(f.released, ["same"]);
  f.requests[1].finish(); await settle(); assert.equal(f.actor.getSnapshot().key, "same"); f.actor.stop();
});

test("transient failures back off and stop after three attempts; failure history is bounded", async () => {
  let now = 0, calls = 0;
  const actor = new SelfSpriteResidency(async () => { calls++; throw Error("offline"); }, () => now);
  actor.start(); actor.setTarget(pose("retry")); await settle();
  actor.setTarget(pose("retry")); await settle(); assert.equal(calls, 1);
  now = 5000; actor.setTarget(pose("retry")); await settle(); assert.equal(calls, 2);
  now = 15000; actor.setTarget(pose("retry")); await settle(); assert.equal(calls, 3);
  now = 100000; actor.setTarget(pose("retry")); await settle(); assert.equal(calls, 3); actor.stop();
  const failed = new SelfSpriteResidency(async () => { throw new SelfSpriteImageError(true); }); failed.start();
  for (let i = 0; i < 280; i++) { failed.setTarget(pose("frame-" + i)); await settle(); }
  assert(failed.stats().failures <= 256); failed.stop();
});

const png = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/l9sAAAAASUVORK5CYII=", "base64");
async function images(run, dimensions = [1, 1]) {
  const oldImage = globalThis.Image, create = URL.createObjectURL, revoke = URL.revokeObjectURL;
  const urls = [], revoked = [], decoded = [];
  class FakeImage {
    complete = false; naturalWidth = dimensions[0]; naturalHeight = dimensions[1];
    set src(value) { urls.push(value); queueMicrotask(() => { this.complete = true; this.onload?.(); }); }
    decode() { decoded.push(true); return Promise.resolve(); }
    removeAttribute() {}
  }
  globalThis.Image = FakeImage;
  URL.createObjectURL = () => "blob:owned-" + urls.length; URL.revokeObjectURL = url => revoked.push(url);
  try { await run({ urls, revoked, decoded }); }
  finally { globalThis.Image = oldImage; URL.createObjectURL = create; URL.revokeObjectURL = revoke; }
}
test("PNG bytes are bounded, verified and decoded once before an owned local URL is published", async () => {
  await images(async f => {
    const requests = [];
    const image = await loadSelfSpriteImage(layer("https://assets.example/body.png"), new AbortController().signal,
      ["https://assets.example/body.png"], async (url, options) => { requests.push({ url, options }); return new Response(png); });
    assert.equal(requests.length, 1); assert.equal(requests[0].options.credentials, "omit");
    assert.equal(requests[0].options.redirect, "error"); assert.equal(f.decoded.length, 1);
    assert.match(image.url, /^blob:/); assert.equal(image.bytes, png.length + 4);
    image.release(); image.release(); assert.deepEqual(f.revoked, [image.url]);
  });
});
test("invalid PNG or metadata dimensions are terminal; failed decode releases the local URL", async () => {
  await images(async f => {
    await assert.rejects(loadSelfSpriteImage(layer("bad"), new AbortController().signal, ["bad"], async () => new Response("not PNG")), e => e.terminal);
    assert.equal(f.urls.length, 0);
    await assert.rejects(loadSelfSpriteImage({ ...layer("bad"), width: 2 }, new AbortController().signal,
      ["bad"], async () => new Response(png)), e => e.terminal);
  });
  await images(async f => {
    await assert.rejects(loadSelfSpriteImage(layer("bad"), new AbortController().signal,
      ["bad"], async () => new Response(png)), e => e.terminal);
    assert.equal(f.revoked.length, 1);
  }, [2, 1]);
});
test("owner cancellation stops network work and cannot publish an image", async () => {
  const owner = new AbortController(); let signal;
  const promise = loadSelfSpriteImage(layer("slow"), owner.signal, ["slow"], async (_url, options) => {
    signal = options.signal;
    return new Promise((resolve, reject) => signal.addEventListener("abort", () => reject(signal.reason), { once: true }));
  });
  owner.abort(); await assert.rejects(promise); assert(signal.aborted);
});
test("full-pack images retain the verified data URI and require no object URL", async () => {
  await images(async f => {
    const source = "data:image/png;base64," + png.toString("base64");
    const image = await loadSelfSpriteImage(layer(source), new AbortController().signal, [source], async () => new Response(png));
    assert.equal(image.url, source); image.release(); assert.deepEqual(f.revoked, []);
  });
});

test("standing warmup uses only this direction's four bodies, and held poses keep layer geometry", () => {
  const file = fileURLToPath(new URL("../app/components/original-client-self-sprite.tsx", import.meta.url));
  const ast = ts.createSourceFile(file, readFileSync(file, "utf8"), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const selected = ast.statements.filter(node => ts.isFunctionDeclaration(node) &&
    ["selfSpritePose", "selfStandingWarmFrames", "displayedSprite"].includes(node.name?.text));
  const code = ts.transpileModule(selected.map(node => node.getText(ast).replace(/^export /, "")).join("\n"), {
    compilerOptions: { target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const functions = new Function("originalAssetPath", code + "\nreturn {selfSpritePose,selfStandingWarmFrames,displayedSprite};")(path => path);
  const body = { ...layer("b"), path: "/original-ui/CArmour/00/17.png" };
  const sprite = { body, hair: null, mount: null, rearWeapons: [], frontWeapons: [], nameplateTop: -40, effect: { id: 9 },
    preloadFrames: Array.from({ length: 900 }, (_, i) => ({ ...body, path: "/original-ui/CArmour/00/" + i + ".png" })) };
  assert.deepEqual(functions.selfStandingWarmFrames(sprite, null).map(f => f.path), [16, 17, 18, 19].map(i => "/original-ui/CArmour/00/" + i + ".png"));
  assert.deepEqual(functions.selfStandingWarmFrames({ ...sprite, body: { ...body, path: "/original-ui/CArmour/00/80.png" } }, null), []);
  const held = functions.selfSpritePose(sprite, null);
  const target = { ...sprite, body: { ...body, x: 500 }, effect: { id: 10 } };
  const displayed = functions.displayedSprite(target, held);
  assert.equal(displayed.body.x, body.x); assert.equal(displayed.effect.id, 10);
  assert.equal(functions.displayedSprite(target, null).body, null);
});
