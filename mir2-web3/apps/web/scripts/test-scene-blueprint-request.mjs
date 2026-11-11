import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";

import ts from "typescript";

const sourcePath = fileURLToPath(new URL("../lib/scene-blueprint-request.ts", import.meta.url));
const compiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: sourcePath,
  reportDiagnostics: true,
});
const errors = (compiled.diagnostics ?? []).filter(
  (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
);
assert.deepEqual(errors, []);
const loadedModule = { exports: {} };
new Function("exports", "module", compiled.outputText)(loadedModule.exports, loadedModule);

const {
  SCENE_BLUEPRINT_SCHEMA_VERSION,
  createCrystalSceneBlueprintRequestKey,
  createCrystalSceneBlueprintRequestUrl,
  normalizeCrystalSceneBlueprintRequest,
} = loadedModule.exports;

test("nearby player positions share one canonical scene request", () => {
  const a = { mapFileName: "Map/0.map", centerX: 344, centerY: 74, width: 66, height: 68 };
  const b = { mapFileName: "0", centerX: 351, centerY: 83, width: 67, height: 65 };
  assert.equal(createCrystalSceneBlueprintRequestKey(a), createCrystalSceneBlueprintRequestKey(b));
  assert.deepEqual(normalizeCrystalSceneBlueprintRequest(a), {
    mapFileName: "0",
    centerX: 344,
    centerY: 76,
    width: 72,
    height: 72,
  });
});

test("the public request URL is canonical and schema-versioned", () => {
  const request = { mapFileName: "0.map", centerX: 347, centerY: 77, width: 67, height: 65 };
  const url = new URL(createCrystalSceneBlueprintRequestUrl(request), "https://mir2.example");
  assert.equal(url.pathname, "/api/scene/crystal");
  assert.equal(url.searchParams.get("v"), SCENE_BLUEPRINT_SCHEMA_VERSION);
  assert.equal(url.searchParams.get("map"), "0");
  assert.equal(url.searchParams.get("x"), "344");
  assert.equal(url.searchParams.get("y"), "76");
  assert.equal(url.searchParams.get("width"), "72");
  assert.equal(url.searchParams.get("height"), "72");
});


// Source70 exercises the actual release loader and Page effects with deferred
// responses. No real network, game runtime, renderer or protocol is involved.
const capabilitiesSourcePath = fileURLToPath(new URL("../lib/asset-release-capabilities.ts", import.meta.url));
const capabilitiesCompiled = ts.transpileModule(readFileSync(capabilitiesSourcePath, "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022, strict: true },
  fileName: capabilitiesSourcePath, reportDiagnostics: true,
});
assert.deepEqual((capabilitiesCompiled.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
function capabilitiesFixture() {
  const browser = {}, module = { exports: {} };
  new Function("exports", "module", "window", "fetch", capabilitiesCompiled.outputText)(
    module.exports, module, browser, () => assert.fail("fixture must provide its finite fetch port"));
  return { api: module.exports, browser };
}
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const manifestResponse = (version, capabilities = {}) => ({
  ok: true, json: async () => ({ version, capabilities }),
});
async function settleSceneFixture() {
  await new Promise(resolve => setImmediate(resolve));
}

test("asset release versions isolate canonical scene and starter HTTP keys without changing chunks", () => {
  const a = { mapFileName: "Map/0140.map", centerX: 344, centerY: 74, width: 66, height: 68, assetReleaseId: "release-old" };
  const b = { ...a, mapFileName: "0140", centerX: 351, centerY: 83, width: 67, height: 65 };
  const next = { ...b, assetReleaseId: "897f1c8804b37877" };
  assert.equal(createCrystalSceneBlueprintRequestKey(a), createCrystalSceneBlueprintRequestKey(b));
  assert.equal(createCrystalSceneBlueprintRequestUrl(a), createCrystalSceneBlueprintRequestUrl(b));
  assert.notEqual(createCrystalSceneBlueprintRequestKey(b), createCrystalSceneBlueprintRequestKey(next));
  assert.notEqual(createCrystalSceneBlueprintRequestUrl(b), createCrystalSceneBlueprintRequestUrl(next));
  for (const pathname of ["/api/scene/crystal", "/api/scene/starter"]) {
    const url = new URL(createCrystalSceneBlueprintRequestUrl(next, pathname), "https://mir2.example");
    assert.equal(url.pathname, pathname);
    assert.equal(url.searchParams.get("assetRelease"), "897f1c8804b37877");
    assert.equal(url.searchParams.get("v"), SCENE_BLUEPRINT_SCHEMA_VERSION);
    assert.equal(url.searchParams.get("map"), "0140");
    assert.equal(url.searchParams.get("x"), "344");
    assert.equal(url.searchParams.get("y"), "76");
    assert.equal(url.searchParams.get("width"), "72");
    assert.equal(url.searchParams.get("height"), "72");
  }
  for (const invalid of [null, "", " ", "bad/version", "x".repeat(81), 1]) {
    assert.equal(loadedModule.exports.isCrystalSceneAssetReleaseId(invalid), false);
    assert.throws(() => createCrystalSceneBlueprintRequestUrl({ ...a, assetReleaseId: invalid }), /Scene asset release is unavailable/);
    assert.throws(() => createCrystalSceneBlueprintRequestKey({ ...a, assetReleaseId: invalid }), /Scene asset release is unavailable/);
  }
});

test("default capability loading retains force-cache, pending deduplication and accepted memory", async () => {
  const f = capabilitiesFixture(), reply = deferred(), calls = [];
  const fetcher = (url, options) => { calls.push({ url, options }); return reply.promise; };
  const first = f.api.loadAssetReleaseCapabilities(fetcher), second = f.api.loadAssetReleaseCapabilities(fetcher);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "/api/asset-manifest");
  assert.deepEqual(calls[0].options, { cache: "force-cache" });
  reply.resolve(manifestResponse("default-release"));
  assert.equal((await first).releaseId, "default-release");
  assert.equal((await second).releaseId, "default-release");
  assert.equal((await f.api.loadAssetReleaseCapabilities(() => assert.fail("accepted memory must be reused"))).releaseId, "default-release");
});

test("fresh capability loading bypasses old memory and pending defaults without allowing stale publication", async () => {
  const f = capabilitiesFixture(), old = deferred(), fresh = deferred(), calls = [];
  const defaultPromise = f.api.loadAssetReleaseCapabilities((url, options) => { calls.push({ url, options }); return old.promise; });
  f.api.setAssetReleaseCapabilities({ releaseId: "cached-old" });
  const controller = new AbortController();
  const freshPromise = f.api.loadAssetReleaseCapabilities((url, options) => { calls.push({ url, options }); return fresh.promise; },
    { fresh: true, signal: controller.signal });
  assert.equal(calls.length, 2);
  assert.equal(calls[1].url, "/api/asset-manifest");
  assert.equal(calls[1].options.cache, "no-cache");
  assert.equal(calls[1].options.signal, controller.signal);
  fresh.resolve(manifestResponse("fresh-release", { mapAtlas: { enabled: true, verified: false } }));
  const accepted = await freshPromise;
  assert.equal(accepted.releaseId, "fresh-release");
  assert.equal(accepted.mapAtlas.enabled, false, "fresh reads retain capability validation");
  old.resolve(manifestResponse("old-default"));
  assert.equal((await defaultPromise).releaseId, "old-default", "the old caller keeps its own response");
  assert.equal(f.browser.__mir2AssetReleaseCapabilities.releaseId, "fresh-release");
  assert.equal((await f.api.loadAssetReleaseCapabilities(() => assert.fail("fresh accepted memory must persist"))).releaseId, "fresh-release");
});

test("fresh capability races and cancellation never overwrite a newer accepted release", async () => {
  const f = capabilitiesFixture(), old = deferred(), next = deferred();
  const first = f.api.loadAssetReleaseCapabilities(() => old.promise, { fresh: true });
  const second = f.api.loadAssetReleaseCapabilities(() => next.promise, { fresh: true });
  next.resolve(manifestResponse("latest"));
  assert.equal((await second).releaseId, "latest");
  old.resolve(manifestResponse("late-old"));
  assert.equal((await first).releaseId, "late-old");
  assert.equal(f.browser.__mir2AssetReleaseCapabilities.releaseId, "latest");
  const body = deferred(), enteredBody = deferred(), controller = new AbortController();
  const cancelled = f.api.loadAssetReleaseCapabilities(async () => ({
    ok: true, json: () => { enteredBody.resolve(); return body.promise; },
  }), { fresh: true, signal: controller.signal });
  await enteredBody.promise;
  controller.abort("fixture-cancelled");
  body.resolve({ version: "cancelled" });
  assert.equal((await cancelled).releaseId, null);
  assert.equal(f.browser.__mir2AssetReleaseCapabilities.releaseId, "latest");
  const failed = await f.api.loadAssetReleaseCapabilities(async () => { throw new Error("fixture failure"); }, { fresh: true });
  assert.equal(failed.releaseId, null, "failure does not return accepted old memory as a fresh result");
  assert.equal(f.browser.__mir2AssetReleaseCapabilities.releaseId, "latest");
});

const pageSourcePath = fileURLToPath(new URL("../app/page.tsx", import.meta.url));
const pageText = readFileSync(pageSourcePath, "utf8");
const pageAst = ts.createSourceFile(pageSourcePath, pageText, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const releaseEffects = [], sceneEffects = [], pageRequestDeclarations = new Map(), pageHelpers = new Map();
for (const node of pageAst.statements) {
  if (ts.isFunctionDeclaration(node) && ["normalizeMapFileName", "crystalSceneContainsPlayer", "shouldReloadCrystalScene", "originalMapRegionContainsTile"].includes(node.name?.text)) {
    pageHelpers.set(node.name.text, node.getText(pageAst));
  }
}
(function visit(node) {
  if (ts.isCallExpression(node) && node.expression.getText(pageAst) === "useEffect" && ts.isArrowFunction(node.arguments[0])) {
    const text = node.arguments[0].body.getText(pageAst);
    if (text.includes("async function loadSceneAssetRelease()")) releaseEffects.push(node);
    if (text.includes("async function loadSceneBlueprint()")) sceneEffects.push(node);
  }
  if (ts.isVariableDeclaration(node) && ["sceneBlueprintRequest", "sceneBlueprintRequestKey"].includes(node.name.getText(pageAst))) {
    pageRequestDeclarations.set(node.name.getText(pageAst), node.getText(pageAst));
  }
  ts.forEachChild(node, visit);
})(pageAst);
assert.equal(releaseEffects.length, 1);
assert.equal(sceneEffects.length, 1);
assert.equal(pageRequestDeclarations.size, 2);
assert.equal(pageHelpers.size, 4);
assert.equal(releaseEffects[0].arguments[1].getText(pageAst), "[screen, sceneReleaseRetry]");
assert.equal(sceneEffects[0].arguments[1].getText(pageAst), "[sceneBlueprintRequestKey, sceneBlueprintRetry, screen]");
const actualPageEffectsCompiled = ts.transpileModule([...pageHelpers.values()].join("\n")
  + "\nconst " + [...pageRequestDeclarations.values()].join(";\nconst ") + ";"
  + "\nconst actualReleaseEffect=" + releaseEffects[0].arguments[0].getText(pageAst) + ";"
  + "\nconst actualSceneEffect=" + sceneEffects[0].arguments[0].getText(pageAst) + ";",
  { compilerOptions: { module: ts.ModuleKind.None, target: ts.ScriptTarget.ES2022, strict: true }, reportDiagnostics: true });
assert.deepEqual((actualPageEffectsCompiled.diagnostics ?? []).filter(d => d.category === ts.DiagnosticCategory.Error), []);
const sceneReaderModule = { exports: {} };
const sceneReaderSource = readFileSync(new URL("../lib/first-crystal-scene-request.ts", import.meta.url), "utf8");
const sceneReaderCompiled = ts.transpileModule(sceneReaderSource, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
});
new Function("exports", "module", sceneReaderCompiled.outputText)(sceneReaderModule.exports, sceneReaderModule);
function pageSceneFixture({ screen = "game", map = "0140", region = null, pinnedRelease } = {}) {
  const cap = capabilitiesFixture(), calls = [], logs = [], updates = [], milestones = [];
  const state = { release: null, failure: null };
  const player = { objectId: "1", x: 344, y: 74 };
  const world = { mapFileName: map, originalMapRegion: region, playerObjectId: player.objectId, entities: [player] };
  const fetcher = (url, options) => {
    const reply = deferred();
    calls.push({ url, options, ...reply });
    return reply.promise;
  };
  const scope = {
    screen, self: player, sceneAssetReleaseId: null,
    process: { env: { MIR2_PINNED_ASSET_VERSION: pinnedRelease } },
    world, worldRef: { current: world }, screenRef: { current: screen },
    viewportLayout: { rangeX: 20, rangeY: 15 },
    loadedSceneAssetReleaseRef: { current: null }, loadedSceneKeyRef: { current: null }, loadingSceneKeyRef: { current: null },
    loggedSceneResourceErrorsRef: { current: new Set() },
    normalizeCrystalSceneBlueprintRequest, createCrystalSceneBlueprintRequestKey, createCrystalSceneBlueprintRequestUrl,
    isCrystalSceneAssetReleaseId: loadedModule.exports.isCrystalSceneAssetReleaseId,
    loadAssetReleaseCapabilities: cap.api.loadAssetReleaseCapabilities,
    readR16EntryScene: () => null,
    fetch: fetcher,
    fetchFirstCrystalSceneJson: (url, options, request = fetcher) => sceneReaderModule.exports.fetchFirstCrystalSceneJson(url, options, request),
    setSceneAssetReleaseId: value => { state.release = value; },
    setSceneLoadFailure: value => { state.failure = value; },
    updateWorld: update => { scope.world = update(scope.world); scope.worldRef.current = scope.world; updates.push(scope.world); },
    markMir2CacheMilestone: (name, detail) => milestones.push({ name, detail }),
    appendLog: value => logs.push(value), t: (key, values) => [key, ...(values ?? [])].join(":"),
    isAbortError: error => error?.name === "AbortError",
  };
  function render() {
    const inputs = { ...scope, sceneAssetReleaseId: state.release };
    return new Function(...Object.keys(inputs), actualPageEffectsCompiled.outputText
      + "\nreturn {release:actualReleaseEffect,scene:actualSceneEffect,request:sceneBlueprintRequest,key:sceneBlueprintRequestKey};")(...Object.values(inputs));
  }
  return { scope, state, cap, calls, logs, updates, milestones, render };
}
function sceneBlueprint(map = "0140") {
  return { mapTitle: "Warehouse", sceneView: null, terrainPatches: [], decorObjects: [],
    originalMapRegion: { mapFileName: map + ".map", sprites: { real: {} }, cells: [{ x: 344, y: 74 }],
      playBounds: { minX: 0, minY: 0, maxX: 1000, maxY: 1000 },
      regionBounds: { minX: 0, minY: 0, maxX: 1000, maxY: 1000 } } };
}
function sceneResponse(blueprint) {
  return { ok: true, status: 200, json: async () => blueprint, headers: { get: () => "crystal" } };
}

const entryMetadata = JSON.parse(readFileSync(new URL("../lib/generated/r16-entry-scene.json", import.meta.url), "utf8"));
const entryBytes = readFileSync(new URL(`../public${entryMetadata.path}`, import.meta.url));
const entryRequest = { ...entryMetadata.request, assetReleaseId: entryMetadata.assetReleaseId };
const floorMetadata = JSON.parse(readFileSync(new URL("../lib/generated/r16-entry-floor.json", import.meta.url), "utf8"));
const floorBytes = readFileSync(new URL("../public" + floorMetadata.path, import.meta.url));
const visualsMetadata = JSON.parse(readFileSync(new URL("../lib/generated/r16-entry-visuals.json", import.meta.url), "utf8"));
const entrySource = ts.transpileModule(readFileSync(new URL("../lib/r16-entry-scene.ts", import.meta.url), "utf8"), {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText;

function entryFixture(fetcher) {
  const timeouts = [];
  const signals = {
    any: AbortSignal.any.bind(AbortSignal),
    timeout(ms) {
      const controller = new AbortController();
      timeouts.push({ ms, controller });
      return controller.signal;
    },
  };
  const module = { exports: {} };
  const requireEntry = name => {
    if (name === "./generated/r16-entry-scene.json") return { default: entryMetadata };
    if (name === "./generated/r16-entry-floor.json") return { default: floorMetadata };
    if (name === "./generated/r16-entry-visuals.json") return { default: visualsMetadata };
    if (name === "./first-crystal-scene-request") return sceneReaderModule.exports;
    assert.equal(name, "./scene-blueprint-request");
    return loadedModule.exports;
  };
  new Function("exports", "require", "fetch", "AbortSignal", entrySource)(module.exports, requireEntry, fetcher, signals);
  return { ...module.exports, timeouts };
}

test("a verified hosted preload serves only the exact release, map and region key", async () => {
  const calls = [], body = deferred(), owner = new AbortController();
  const entry = entryFixture((url, options) => { calls.push({ url, options }); return body.promise; });
  assert.equal(entry.preloadR16EntryScene(undefined), null);
  assert.equal(entry.readR16EntryScene(entryRequest, "another-release", owner.signal), null);
  assert.equal(entry.readR16EntryScene({ ...entryRequest, centerX: 312 }, entryMetadata.assetReleaseId, owner.signal), null);
  assert.equal(entry.readR16EntryScene({ ...entryRequest, mapFileName: "0140" }, entryMetadata.assetReleaseId, owner.signal), null);
  assert.equal(calls.length, 0);
  const preloaded = entry.preloadR16EntryScene(entryMetadata.assetReleaseId);
  const first = entry.readR16EntryScene(entryRequest, entryMetadata.assetReleaseId, owner.signal);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, entryMetadata.path);
  assert.equal(calls[0].options.cache, "force-cache");
  assert.equal(calls[0].options.credentials, "omit");
  body.resolve(new Response(entryBytes));
  const blueprint = await first;
  assert.equal(blueprint, await preloaded);
  assert.equal(blueprint.originalMapRegion.cells.length, 6819);
  assert.equal(await entry.readR16EntryScene(entryRequest, entryMetadata.assetReleaseId, owner.signal), blueprint);
  assert.equal(calls.length, 1);
});

test("the entry floor preload is shared only for the pinned release", async () => {
  const calls = [];
  const entry = entryFixture(async (url, options) => {
    calls.push({ url, options });
    return new Response(floorBytes);
  });
  assert.equal(entry.preloadR16EntryFloor("another-release"), null);
  const first = entry.preloadR16EntryFloor(entryMetadata.assetReleaseId);
  assert.equal(entry.preloadR16EntryFloor(entryMetadata.assetReleaseId), first);
  const floor = await first;
  assert.equal(Object.keys(floor).length, floorMetadata.frameCount);
  assert.match(floor["/original-map/WemadeMir2/Tiles/137.png"], /^data:image\/png;base64,/);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, floorMetadata.path);
  assert.equal(calls[0].options.credentials, "omit");
});

test("the floor preload survives more than twelve seconds of body progress", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const requests = [];
  let stream;
  const response = new Response(new ReadableStream({ start(controller) { stream = controller; } }));
  const entry = entryFixture(async (url, options) => {
    requests.push({ url, options });
    return response;
  });
  const loading = entry.preloadR16EntryFloor(entryMetadata.assetReleaseId);
  let finished = false;
  void loading.then(() => { finished = true; });
  const chunkSize = Math.ceil(floorBytes.length / 4);
  for (let index = 0; index < 4; index++) {
    t.mock.timers.tick(3_600);
    stream.enqueue(floorBytes.subarray(index * chunkSize, (index + 1) * chunkSize));
    await settleSceneFixture();
  }
  assert.equal(requests.length, 1);
  assert.equal(requests[0].options.signal.aborted, false);
  assert.equal(finished, false);
  stream.close();
  assert.equal(Object.keys(await loading).length, floorMetadata.frameCount);
  assert.equal(await entry.preloadR16EntryFloor(entryMetadata.assetReleaseId), await loading);
});

test("a stalled floor body restarts once at the same address with a reload", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const requests = [];
  const truncated = new Response(new ReadableStream({
    start(controller) { controller.enqueue(floorBytes.subarray(0, 2048)); },
  }));
  const entry = entryFixture(async (url, options) => {
    requests.push({ url, options });
    return requests.length === 1 ? truncated : new Response(floorBytes);
  });
  const loading = entry.preloadR16EntryFloor(entryMetadata.assetReleaseId);
  await settleSceneFixture();
  t.mock.timers.tick(8_000);
  await settleSceneFixture();
  assert.equal(requests[0].options.signal.reason.name, "TimeoutError");
  t.mock.timers.tick(200);
  await settleSceneFixture();
  assert.equal(Object.keys(await loading).length, floorMetadata.frameCount);
  assert.deepEqual(requests.map(({ url, options }) => [url, options.cache, options.credentials]), [
    [floorMetadata.path, "force-cache", "omit"],
    [floorMetadata.path, "reload", "omit"],
  ]);
});

const readEntryVisual = (asset) => readFileSync(new URL("../public" + asset.path, import.meta.url));

test("player, NPC and building packs load independently and publish only pinned bytes", async () => {
  const { male, female } = visualsMetadata.players;
  const [firstObject, secondObject, thirdObject] = visualsMetadata.objectParts;
  const wrong = readEntryVisual(firstObject);
  wrong[50] ^= 1;
  const calls = [];
  const packs = new Map([male, female, visualsMetadata.npcs, ...visualsMetadata.objectParts]
    .map((asset) => [asset.path, readEntryVisual(asset)]));
  let firstObjectFetches = 0;
  const entry = entryFixture(async (url, options) => {
    calls.push({ url, options });
    if (url === firstObject.path && ++firstObjectFetches === 1) return new Response(wrong);
    assert(packs.has(url), url);
    return new Response(packs.get(url));
  });
  const release = entryMetadata.assetReleaseId;
  assert.equal(entry.preloadR16EntryObjects(undefined), null);
  assert.equal(entry.preloadR16EntryPlayer("other", "female"), null);
  const maleLoad = entry.preloadR16EntryPlayer(release, "male");
  assert.equal(entry.preloadR16EntryPlayer(release, "male"), maleLoad);
  const femaleLoad = entry.preloadR16EntryPlayer(release, "female");
  const npcLoad = entry.preloadR16EntryNpcs(release);
  const objectLoads = entry.preloadR16EntryObjects(release);
  const [malePack, femalePack, npcPack, rejected, second, third] =
    await Promise.all([maleLoad, femaleLoad, npcLoad, ...objectLoads]);
  assert.equal(malePack.libraries["CArmour/00"].frames.length, 80);
  assert.equal(femalePack.libraries["CArmour/00"].frames.length, 80);
  assert.match(malePack.images["/original-ui/CArmour/00/44.png"], /^data:image\/png;base64,/);
  assert.match(femalePack.images["/original-ui/CArmour/00/852.png"], /^data:image\/png;base64,/);
  const originalActors = JSON.parse(readEntryVisual(visualsMetadata.actors));
  assert.deepEqual(
    { ...malePack.images, ...femalePack.images, ...npcPack.images },
    originalActors.images,
  );
  assert.equal(Object.keys(npcPack.libraries).length, 8);
  assert.equal(rejected, null);
  const retried = entry.preloadR16EntryObjects(release);
  assert.equal(retried[1], objectLoads[1]);
  assert.equal(retried[2], objectLoads[2]);
  assert.deepEqual({ ...await retried[0], ...second, ...third }, JSON.parse(readEntryVisual(visualsMetadata.objects)));
  assert.deepEqual(calls.map(({ url, options }) => [url, options.priority]), [
    [male.path, "high"], [female.path, "high"], [visualsMetadata.npcs.path, "low"],
    [firstObject.path, "low"], [secondObject.path, "low"], [thirdObject.path, "low"], [firstObject.path, "low"],
  ]);
});

test("a player pack can finish after thirty seconds of continuous body progress", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const bytes = readEntryVisual(visualsMetadata.players.male);
  const calls = [];
  let body;
  const stream = new ReadableStream({ start(controller) { body = controller; } });
  const entry = entryFixture(async (url, options) => {
    calls.push({ url, options });
    return new Response(stream);
  });
  const pending = entry.preloadR16EntryPlayer(entryMetadata.assetReleaseId, "male");
  await settleSceneFixture();
  const chunkSize = Math.ceil(bytes.length / 5);
  for (let index = 0; index < 5; index++) {
    t.mock.timers.tick(6_000);
    body.enqueue(bytes.subarray(index * chunkSize, (index + 1) * chunkSize));
    await settleSceneFixture();
  }
  assert.equal(calls.length, 1);
  assert.equal(calls[0].options.signal.aborted, false);
  body.close();
  assert.equal(Object.keys((await pending).images).length, 160);
});

test("the staged monster packages retain the exact eight directions and independent fallback", async () => {
  const standingBytes = readFileSync(new URL("../public" + visualsMetadata.monsterStanding.path, import.meta.url));
  const walkingBytes = readFileSync(new URL("../public" + visualsMetadata.monsterWalking.path, import.meta.url));
  const changed = Buffer.from(walkingBytes);
  changed[66] ^= 1;
  let walkingFetches = 0;
  const entry = entryFixture(async (url) => new Response(url === visualsMetadata.monsterStanding.path
    ? standingBytes : ++walkingFetches === 1 ? changed : walkingBytes));
  assert.equal(entry.preloadR16EntryMonsterFrames("other", "standing"), null);
  const first = entry.preloadR16EntryMonsterFrames(entryMetadata.assetReleaseId, "standing");
  assert.equal(entry.preloadR16EntryMonsterFrames(entryMetadata.assetReleaseId, "standing"), first);
  const standing = await first;
  assert.equal(Object.keys(standing.images).length, 160);
  assert.equal(await entry.preloadR16EntryMonsterFrames(entryMetadata.assetReleaseId, "walking"), null);
  assert.equal(await entry.preloadR16EntryMonsterFrames(entryMetadata.assetReleaseId, "standing"), standing);
  const walking = await entry.preloadR16EntryMonsterFrames(entryMetadata.assetReleaseId, "walking");
  assert.equal(Object.keys(walking.images).length, 240);
  const guard = walking.libraries["Monster/139"];
  assert.deepEqual(guard.frames.slice(0, 7).map(frame => frame.index), [32, 33, 34, 35, 36, 37, 41]);
  assert.equal(guard.frames.at(-1).index, 100);
  assert(guard.frameSet.actions.some(action => action.actionName === "Attack1"));
});

test("corrupt floor bytes are never published and the prefetch can retry", async () => {
  const wrong = Buffer.from(floorBytes);
  wrong[50] ^= 1;
  let count = 0;
  const entry = entryFixture(async () => new Response(++count === 1 ? wrong : floorBytes));
  assert.equal(await entry.preloadR16EntryFloor(entryMetadata.assetReleaseId), null);
  assert.equal(Object.keys(await entry.preloadR16EntryFloor(entryMetadata.assetReleaseId)).length, floorMetadata.frameCount);
  assert.equal(count, 2);
});

test("changed static bytes are rejected even with the expected response length", async () => {
  let count = 0;
  const wrong = Buffer.from(entryBytes);
  wrong[15] ^= 1;
  const entry = entryFixture(async () => new Response(++count === 1 ? wrong : entryBytes));
  const owner = new AbortController();
  assert.equal(await entry.readR16EntryScene(entryRequest, entryMetadata.assetReleaseId, owner.signal), null);
  assert.equal((await entry.readR16EntryScene(entryRequest, entryMetadata.assetReleaseId, owner.signal)).originalMapRegion.mapFileName, "0.map");
  assert.equal(count, 2);
});

test("the owner and the short entry grace can leave an independently warming map", async () => {
  const body = deferred(), firstOwner = new AbortController(), secondOwner = new AbortController();
  let requests = 0;
  const entry = entryFixture(() => { requests += 1; return body.promise; });
  const first = entry.readR16EntryScene(entryRequest, entryMetadata.assetReleaseId, firstOwner.signal);
  firstOwner.abort();
  assert.equal(await first, null);
  const second = entry.readR16EntryScene(entryRequest, entryMetadata.assetReleaseId, secondOwner.signal);
  assert.equal(entry.timeouts.at(-1).ms, 2_500);
  entry.timeouts.at(-1).controller.abort();
  assert.equal(await second, null);
  assert.equal(entry.timeouts[0].ms, 12_000);
  assert.equal(entry.timeouts[0].controller.signal.aborted, false);
  body.resolve(new Response(entryBytes));
  assert.equal((await entry.preloadR16EntryScene(entryMetadata.assetReleaseId)).originalMapRegion.mapFileName, "0.map");
  assert.equal(requests, 1);
});

test("actual Page installs the warmed entry only for the active character and falls back if outside its bounds", async () => {
  for (const mode of ["valid", "outside", "disposed"]) {
    const f = pageSceneFixture({ map: "0", pinnedRelease: entryMetadata.assetReleaseId });
    Object.assign(f.scope.self, { x: 288, y: 616 });
    f.scope.viewportLayout = { rangeX: 16, rangeY: 17 };
    f.state.release = entryMetadata.assetReleaseId;
    const deferredEntry = deferred();
    f.scope.readR16EntryScene = (request, pin) => {
      assert.deepEqual(request, entryRequest);
      assert.equal(pin, entryMetadata.assetReleaseId);
      return deferredEntry.promise;
    };
    const cleanup = f.render().scene();
    const blueprint = JSON.parse(entryBytes);
    if (mode === "outside") blueprint.originalMapRegion.playBounds.minX = 290;
    if (mode === "disposed") cleanup();
    deferredEntry.resolve(blueprint);
    await settleSceneFixture();
    if (mode === "valid") {
      assert.equal(f.calls.length, 0);
      assert.equal(f.updates.length, 1);
      assert.equal(f.milestones.at(-1).detail.sceneCache, "r16-entry-static");
    } else if (mode === "outside") {
      assert.equal(f.calls.length, 1);
      assert.match(f.calls[0].url, /^\/api\/scene\/crystal\?/);
      f.calls[0].resolve(sceneResponse(sceneBlueprint("0")));
      await settleSceneFixture();
      assert.equal(f.updates.length, 1);
      assert.equal(f.milestones.at(-1).detail.sceneCache, "crystal");
    } else {
      assert.equal(f.calls.length, 0);
      assert.equal(f.updates.length, 0);
    }
    if (mode !== "disposed") cleanup();
  }
});

test("a pinned hosted build requests the authoritative scene while the manifest is still pending", async () => {
  const releaseId = "20261108-r16-08ec86d5105c";
  const f = pageSceneFixture({ map: "0", pinnedRelease: releaseId });
  f.cap.api.setAssetReleaseCapabilities({ releaseId: "cached-old" });
  const pendingManifest = f.cap.api.loadAssetReleaseCapabilities(f.scope.fetch, { fresh: true });
  assert.equal(f.calls[0].url, "/api/asset-manifest");
  const cleanupRelease = f.render().release();
  await settleSceneFixture();
  assert.equal(f.state.release, releaseId);
  const cleanupScene = f.render().scene();
  assert.equal(f.calls.length, 2, "the page sends a scene request without a second manifest GET");
  const url = new URL(f.calls[1].url, "https://mir2.example");
  assert.equal(url.pathname, "/api/scene/crystal");
  assert.equal(url.searchParams.get("map"), "0");
  assert.equal(url.searchParams.get("assetRelease"), releaseId);
  f.calls[1].resolve(sceneResponse(sceneBlueprint("0")));
  await settleSceneFixture();
  assert.equal(f.updates.length, 1);
  assert.equal(f.scope.loadedSceneAssetReleaseRef.current, releaseId);
  f.calls[0].resolve(manifestResponse("cached-old"));
  await pendingManifest;
  assert.equal(f.state.release, releaseId);
  cleanupScene();
  cleanupRelease();
});

test("an invalid build pin uses the same fresh manifest request as an unpinned build", async () => {
  const f = pageSceneFixture({ pinnedRelease: "invalid/release" });
  const cleanupRelease = f.render().release();
  assert.equal(f.calls.length, 1);
  assert.equal(f.calls[0].url, "/api/asset-manifest");
  assert.equal(f.calls[0].options.cache, "no-cache");
  f.calls[0].resolve(manifestResponse("valid-release"));
  await settleSceneFixture();
  assert.equal(f.state.release, "valid-release");
  cleanupRelease();
});

test("actual Page waits for a fresh server release before issuing a versioned Scene request", async () => {
  const f = pageSceneFixture();
  f.cap.api.setAssetReleaseCapabilities({ releaseId: "cached-old" });
  const before = f.render();
  assert.equal(before.request, null);
  assert.equal(before.scene(), undefined);
  assert.equal(f.calls.length, 0);
  const releaseCleanup = before.release();
  assert.equal(f.calls.length, 1);
  assert.equal(f.calls[0].url, "/api/asset-manifest");
  assert.equal(f.calls[0].options.cache, "no-cache");
  f.calls[0].resolve(manifestResponse("897f1c8804b37877"));
  await settleSceneFixture();
  assert.equal(f.state.release, "897f1c8804b37877");
  const ready = f.render(), sceneCleanup = ready.scene();
  assert.equal(f.calls.length, 2);
  const url = new URL(f.calls[1].url, "https://mir2.example");
  assert.equal(url.pathname, "/api/scene/crystal");
  assert.equal(url.searchParams.get("assetRelease"), "897f1c8804b37877");
  assert.equal(f.calls[1].options.cache, "force-cache", "only the manifest freshness policy changes");
  f.calls[1].resolve(sceneResponse(sceneBlueprint()));
  await settleSceneFixture();
  assert.equal(f.updates.length, 1);
  assert.equal(f.scope.loadedSceneAssetReleaseRef.current, "897f1c8804b37877");
  assert.equal(f.scope.loadedSceneKeyRef.current, ready.key);
  sceneCleanup(); releaseCleanup();
});

test("actual Page unavailable or cancelled release never fetches an old Scene or starter fallback", async () => {
  for (const version of [null, "", "invalid/release"]) {
    const f = pageSceneFixture(); f.cap.api.setAssetReleaseCapabilities({ releaseId: "cached-old" });
    const cleanup = f.render().release();
    f.calls[0].resolve(manifestResponse(version));
    await settleSceneFixture();
    assert.equal(f.state.release, null);
    assert.equal(f.render().scene(), undefined);
    assert.equal(f.calls.length, 1);
    assert.equal(f.updates.length, 0);
    assert.equal(f.logs.length, 1);
    assert.deepEqual(f.state.failure, { phase: "release" });
    cleanup();
  }
  const f = pageSceneFixture(), cleanup = f.render().release();
  cleanup();
  assert.equal(f.calls[0].options.signal.aborted, true);
  f.calls[0].resolve(manifestResponse("late-release"));
  await settleSceneFixture();
  assert.equal(f.state.release, null);
  assert.equal(f.render().scene(), undefined);
  assert.equal(f.calls.length, 1);
  assert.equal(f.cap.browser.__mir2AssetReleaseCapabilities, undefined);
  assert.equal(f.logs.length, 0);
});

test("actual Page release changes reload an interior region while unchanged chunks retain margin stability", async () => {
  const f = pageSceneFixture({ region: sceneBlueprint().originalMapRegion });
  f.state.release = "new-release";
  f.scope.loadedSceneAssetReleaseRef.current = "old-release";
  const first = f.render(), cleanup = first.scene();
  assert.equal(f.calls.length, 1, "an old release cannot authorize an interior-region skip");
  f.calls[0].resolve(sceneResponse(sceneBlueprint()));
  await settleSceneFixture();
  cleanup();
  f.scope.self = { x: 351, y: 83 };
  const near = f.render();
  assert.equal(near.key, first.key);
  assert.equal(near.scene(), undefined);
  f.scope.self = { x: 352, y: 83 };
  const across = f.render();
  assert.notEqual(across.key, first.key);
  assert.equal(across.scene(), undefined);
  assert.equal(f.calls.length, 1, "interior chunk crossings do not refetch the scene or manifest");
  f.state.release = "next-release";
  const next = f.render(), nextCleanup = next.scene();
  assert.notEqual(next.key, across.key);
  assert.equal(f.calls.length, 2);
  assert.equal(new URL(f.calls[1].url, "https://mir2.example").searchParams.get("assetRelease"), "next-release");
  nextCleanup();
  f.calls[1].resolve(sceneResponse(sceneBlueprint()));
  await settleSceneFixture();
  assert.equal(f.updates.length, 1);
});

test("actual Page starter fallback retains the captured release and superseded Scene disposal is exact", async () => {
  const starter = pageSceneFixture({ map: "0" }); starter.state.release = "starter-release";
  const starterCleanup = starter.render().scene();
  starter.calls[0].resolve({ ok: false, status: 424,
    json: async () => ({ error: "resource_missing", publicPath: "/original-map/WemadeMir2/Tiles/1.png", mapFileName: "0" }),
  });
  await settleSceneFixture();
  assert.equal(starter.calls.length, 2);
  const url = new URL(starter.calls[1].url, "https://mir2.example");
  assert.equal(url.pathname, "/api/scene/starter");
  assert.equal(url.searchParams.get("assetRelease"), "starter-release");
  starter.calls[1].resolve(sceneResponse(sceneBlueprint("0")));
  await settleSceneFixture();
  assert.equal(starter.updates.length, 1);
  starterCleanup();

  const f = pageSceneFixture(); f.state.release = "same-release";
  const oldCleanup = f.render().scene(), oldBody = deferred(), enteredBody = deferred();
  f.calls[0].resolve({ ok: true, headers: { get: () => "crystal" }, json: () => { enteredBody.resolve(); return oldBody.promise; } });
  await enteredBody.promise;
  oldCleanup();
  const next = f.render(), nextCleanup = next.scene();
  assert.equal(f.calls.length, 2);
  oldBody.resolve(sceneBlueprint());
  await settleSceneFixture();
  assert.equal(f.updates.length, 0);
  assert.equal(f.scope.loadingSceneKeyRef.current, next.key, "old finally cannot clear the replacement request");
  f.calls[1].resolve(sceneResponse(sceneBlueprint()));
  await settleSceneFixture();
  assert.equal(f.updates.length, 1);
  assert.equal(f.scope.loadingSceneKeyRef.current, null);
  nextCleanup();
});

test("actual Page accepts sparse map tiles inside both authoritative region bounds", async () => {
  const f = pageSceneFixture({ map: "0" });
  Object.assign(f.scope.self, { x: 298, y: 273 });
  f.state.release = "exact-release";
  const scene = f.render(), cleanup = scene.scene();
  const blueprint = sceneBlueprint("0");
  Object.assign(blueprint.originalMapRegion, {
    playBounds: { minX: 272, minY: 262, maxX: 320, maxY: 298 },
    regionBounds: { minX: 244, minY: 240, maxX: 348, maxY: 341 },
    cells: [{ x: 298, y: 272, back: "real" }],
  });
  f.calls[0].resolve(sceneResponse(blueprint));
  await settleSceneFixture();
  assert.equal(f.updates.length, 1);
  assert.strictEqual(f.updates[0].originalMapRegion, blueprint.originalMapRegion);
  assert.equal(f.state.failure, null);
  assert.equal(f.scope.loadedSceneKeyRef.current, scene.key);
  cleanup();
});

test("actual Page refuses wrong, empty and out-of-bounds regions, including the starter fallback", async () => {
  const outsidePlayBounds = sceneBlueprint("0");
  outsidePlayBounds.originalMapRegion.playBounds.minY = 75;
  const outsideRegionBounds = sceneBlueprint("0");
  outsideRegionBounds.originalMapRegion.regionBounds.minY = 75;
  const empty = sceneBlueprint("0");
  empty.originalMapRegion.cells = [];
  for (const [blueprint, isStarter] of [[sceneBlueprint("0140"), false], [outsideRegionBounds, false], [empty, false], [outsidePlayBounds, true]]) {
    const f = pageSceneFixture({ map: "0" });
    f.state.release = "exact-release";
    const render = f.render(), cleanup = render.scene();
    if (isStarter) {
      f.calls[0].resolve({ ok: false, status: 424, json: async () => ({ error: "resource_missing", mapFileName: "0" }) });
      await settleSceneFixture();
      assert.equal(f.calls.length, 2);
    }
    f.calls.at(-1).resolve(sceneResponse(blueprint));
    await settleSceneFixture();
    assert.equal(f.updates.length, 0);
    assert.deepEqual(f.state.failure, { phase: "blueprint", sceneKey: render.key });
    assert.equal(f.scope.loadingSceneKeyRef.current, null);
    cleanup();
  }
});
