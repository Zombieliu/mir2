import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

function compile(source) {
  return ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  } }).outputText;
}
function load(relative, globals = {}) {
  const file = new URL(relative, import.meta.url);
  if (file.pathname.endsWith(".json")) return { default: JSON.parse(readFileSync(file, "utf8")) };
  const module = { exports: {} };
  const require = name => {
    assert(name.startsWith("."), `unexpected runtime dependency ${name}`);
    return load(new URL(name + (name.endsWith(".json") ? "" : ".ts"), file), globals);
  };
  new Function("require", "exports", "module", ...Object.keys(globals), compile(readFileSync(file, "utf8")))(
    require, module.exports, module, ...Object.values(globals));
  return module.exports;
}
const { nextQuestRouteStep } = load("../lib/quest-route-execution.ts");
const { sameQuestWorldLifetime } = load("../lib/quest-world-lifecycle.ts");
const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const names = ["cancelQuestRoute", "finishQuestRoute", "questRouteMovement", "beginQuestRoute"];
const declarations = [];
function visit(node) {
  if (ts.isFunctionDeclaration(node) && names.includes(node.name?.text)) declarations.push(node.getText(ast));
  ts.forEachChild(node, visit);
}
visit(ast);
assert.equal(declarations.length, names.length, "exercise actual Page route callbacks");
const code = compile(declarations.join("\n"));
const flush = async () => { for (let i = 0; i < 8; i++) await Promise.resolve(); };
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
function clock() {
  let now = 1000, sequence = 0;
  const timers = new Map();
  return { now: () => now, timers,
    setTimeout(fn, ms) { const id = ++sequence; timers.set(id, { fn, due: now + ms }); return id; },
    clearTimeout(id) { timers.delete(id); },
    elapse(ms, fire = true) {
      now += ms;
      if (fire) for (const [id, row] of [...timers]) if (row.due <= now) { timers.delete(id); row.fn(); }
    },
  };
}
const identity = { connectionGeneration: 1, sessionGeneration: 1, sceneRevision: 1,
  playerObjectId: 1000, mapFileName: "0" };
function harness(target = { type: "huntRegion", monsterIndex: 44, radius: 50 }) {
  const ref = current => ({ current }), timer = clock(), loading = [], logs = [], plans = [], queries = [], sent = [];
  const lifetime = { identity, generation: 1, runtime: {}, runtimeGeneration: 1, socket: {}, questSource: "scarecrow2/2-cat0/2" };
  const state = { live: lifetime, self: { x: 302, y: 563 }, occupied: [], blocked: false };
  const geometry = { mapFileName: "0", width: 768, height: 768, fingerprint: "a".repeat(64), blockedBits: new Uint8Array(768 * 768 / 8) };
  const routeRef = ref(null), queuedRef = ref(null), planRef = ref(null);
  const deps = {
    window: timer, Date: { now: timer.now }, questRouteRunRef: routeRef, queuedMoveIntentRef: queuedRef, movementPlanRef: planRef,
    runtimeRef: ref({ getMir2QuestRoutePlan() {} }), questMapAuthorityRef: ref({ mapIndex: 0 }),
    worldRef: ref({}),
    currentQuestMapIndex: authority => authority.mapIndex, currentQuestWorldLifetime: () => state.live, sameQuestWorldLifetime,
    document: { hasFocus: () => true }, questWorldBlockedRef: { get current() { return state.blocked; } },
    currentAuthoritativeSelf: () => state.self, questRouteOccupancy: () => state.occupied, questRouteRejectedEdges: () => [],
    questCollisionCacheRef: ref({ load() { const task = deferred(); loading.push(task); return task.promise; } }),
    planBevyQuestRoute: (...args) => { queries.push(args); return plans.shift() ?? null; },
    nextQuestRouteStep, crystalSelfMovementProfile: () => ({ distance: 2 }), movementBlockedStepsRef: ref([]),
    recentMovementBlockedSteps: () => [], movementStepBlockedByRecentCorrection: () => false,
    navigateSharedHud: () => true, setShowQuestLog() {}, cancelLockedMonsterAttack() {}, stopOnchainMining() {},
    crystalRunPrimedUntilRef: ref(0), CRYSTAL_RUN_PRIME_MS: 1000,
    queueCrystalMoveIntent: intent => { queuedRef.current = intent; sent.push(intent); },
    t: (key, args = []) => ({ key, args }), appendLog: (message, tone) => logs.push({ ...message, tone }),
    retireQuestWorldActions: () => api.cancelQuestRoute(),
  };
  const api = new Function(...Object.keys(deps), `${code}; return {${names.join(",")}};`)(...Object.values(deps));
  const action = { ...identity, generation: 1, revision: 1, type: "navigateQuestRoute", questIndex: 2110003,
    resetEpoch: 1, mapIndex: 0, x: 340, y: 550, routeTarget: target };
  const begin = () => { assert.equal(api.beginQuestRoute(action, lifetime), true); return routeRef.current; };
  return { ...api, action, lifetime, state, geometry, timer, loading, logs, plans, queries, sent, routeRef, queuedRef, planRef, begin };
}
const terminal = h => h.logs.filter(row => row.key !== "ui.questRoutePlanning");
const arrivedPlan = h => ({ origin: { ...h.state.self }, destination: { ...h.state.self }, steps: [], radius: h.action.routeTarget.radius ?? 2 });
const straightPlan = h => ({ origin: { ...h.state.self }, destination: { x: h.state.self.x + 2, y: h.state.self.y }, radius: 50,
  steps: [{ x: h.state.self.x + 1, y: h.state.self.y }, { x: h.state.self.x + 2, y: h.state.self.y }] });

for (const distance of [1, 2, 3]) test(`route lookahead: ${distance}-tile movement replans before any of the next three edges becomes blocked`, () => {
  const plan = { origin: { x: 10, y: 10 }, destination: { x: 14, y: 11 }, radius: 0,
    steps: [{ x: 11, y: 10 }, { x: 12, y: 10 }, { x: 13, y: 11 }, { x: 14, y: 11 }] };
  for (const blockedIndex of [0, 1, 2]) {
    const target = plan.steps[blockedIndex];
    assert.deepEqual(nextQuestRouteStep(plan, plan.origin, distance,
      (_from, to) => to.x === target.x && to.y === target.y), { type: "replan" },
    `edge ${blockedIndex + 1} must be checked before walking or running toward the obstruction`);
  }
  const outside = plan.steps[3];
  assert.deepEqual(nextQuestRouteStep(plan, plan.origin, distance,
    (_from, to) => to.x === outside.x && to.y === outside.y), {
    type: "move", point: plan.steps[distance === 1 || distance === 3 ? 0 : 1], direction: "Right",
    mode: distance === 2 ? "run" : "walk",
  }, "the fourth edge is checked only as it enters the three-edge window; bends still degrade to a walk");
});

for (const index of [1, 2]) test(`actual Page: blocker at upcoming edge ${index + 1} reroutes while retaining the same quest destination`, async () => {
  const h = harness(), origin = { ...h.state.self }, destination = { x: origin.x + 4, y: origin.y };
  const plan = { origin, destination, radius: 50,
    steps: [1, 2, 3, 4].map(dx => ({ x: origin.x + dx, y: origin.y })) };
  h.plans.push(plan); const route = h.begin(); h.loading[0].resolve(h.geometry); await flush();
  h.state.occupied = [plan.steps[index]];
  const detour = { ...plan, steps: [
    { x: origin.x + 1, y: origin.y + 1 }, { x: origin.x + 2, y: origin.y + 1 },
    { x: origin.x + 3, y: origin.y + 1 }, destination,
  ] };
  h.plans.push(detour);
  assert.deepEqual(h.questRouteMovement(route, h.state.self, "run", h.timer.now()), {
    point: detour.steps[0], direction: "DownRight", mode: "walk",
  });
  assert.equal(h.queries.length, 2); assert.equal(h.queries[1][1], h.action);
  assert.equal(h.routeRef.current, route); assert.deepEqual(route.plan.destination, destination);
  assert.equal(h.queuedRef.current.targetX, destination.x); assert.equal(h.queuedRef.current.targetY, destination.y);
  assert.deepEqual(terminal(h), [], "replanning cannot report arrival or clear the current goal");
});

for (const [target, key] of [
  [{ type: "huntRegion", monsterIndex: 44, radius: 50 }, "ui.questRouteHuntArrived"],
  [{ type: "taskNpc", objectId: 4 }, "ui.questRouteNpcArrived"],
  [{ type: "mapNpc", objectId: 4 }, "ui.questRouteNpcArrived"],
  [{ type: "supply", vendor: "potions" }, "ui.questRouteNpcArrived"],
  [{ type: "entrance" }, "ui.questRouteArrived"],
]) test(`actual Page: zero-step ${target.type} arrival explains what to do without sending movement`, async () => {
  const h = harness(target); h.plans.push(arrivedPlan(h)); h.begin();
  assert.deepEqual(h.logs[0], { key: "ui.questRoutePlanning", args: [340, 550], tone: "system" });
  h.loading[0].resolve(h.geometry); await flush();
  assert.deepEqual(terminal(h), [{ key, args: [340, 550], tone: "system" }]);
  assert.equal(h.routeRef.current, null); assert.equal(h.queuedRef.current, null);
  assert.equal(h.timer.timers.size, 0); assert.deepEqual(h.sent, []);
});

test("actual Page: a complete route keeps moving until authoritative endpoint arrival, then reports once", async () => {
  const h = harness(), plan = straightPlan(h); h.plans.push(plan); const route = h.begin();
  h.loading[0].resolve(h.geometry); await flush();
  assert.equal(h.timer.timers.size, 0); assert.equal(h.sent.length, 1);
  assert.deepEqual(h.questRouteMovement(route, h.state.self, "run", h.timer.now()), { point: plan.destination, direction: "Right", mode: "run" });
  assert.deepEqual(terminal(h), [], "a visual prediction is not arrival");
  h.state.self = { ...plan.destination };
  assert.equal(h.questRouteMovement(route, h.state.self, "run", h.timer.now()), null);
  assert.equal(h.questRouteMovement(route, h.state.self, "run", h.timer.now()), null);
  assert.equal(terminal(h).length, 1); assert.equal(terminal(h)[0].key, "ui.questRouteHuntArrived");
  assert.equal(h.queuedRef.current, null);
});

for (const outcome of ["unavailable", "arrived", "detour"]) test(`actual Page: occupied next step replans original hunt area and ${outcome}`, async () => {
  const h = harness(), plan = straightPlan(h); h.plans.push(plan); const route = h.begin();
  h.loading[0].resolve(h.geometry); await flush(); h.state.occupied = [plan.steps[0]];
  const detour = { ...plan, steps: [{ x: 303, y: 564 }, { x: 304, y: 563 }] };
  h.plans.push(outcome === "unavailable" ? null : outcome === "arrived" ? arrivedPlan(h) : detour);
  const step = h.questRouteMovement(route, h.state.self, "run", h.timer.now());
  assert.equal(h.queries.length, 2); assert.equal(h.queries[1][1], h.action);
  assert.equal(h.queries[1][1].routeTarget.radius, 50); assert.deepEqual(h.queries[1][4], h.state.occupied);
  if (outcome === "detour") {
    assert.deepEqual(step, { point: detour.steps[0], direction: "DownRight", mode: "walk" });
    assert.equal(h.routeRef.current, route); assert.deepEqual(terminal(h), []);
  } else {
    assert.equal(step, null); assert.equal(h.routeRef.current, null);
    assert.equal(terminal(h)[0].key, outcome === "arrived" ? "ui.questRouteHuntArrived" : "ui.questRouteUnavailable");
  }
});

test("actual Page: loading deadline is active while I/O hangs and late geometry cannot resume the action", async () => {
  const h = harness(); h.begin(); h.timer.elapse(30_000);
  assert.equal(h.routeRef.current, null); assert.equal(terminal(h)[0].key, "ui.questRouteLoadingTimeout");
  h.loading[0].resolve(h.geometry); await flush();
  assert.equal(terminal(h).length, 1); assert.deepEqual(h.sent, []); assert.deepEqual(h.queries, []);
});

test("actual Page: delayed timer delivery still enforces the original collision deadline", async () => {
  const h = harness(); h.begin(); h.timer.elapse(30_000, false);
  h.loading[0].resolve(h.geometry); await flush();
  assert.equal(terminal(h)[0].key, "ui.questRouteLoadingTimeout"); assert.equal(h.timer.timers.size, 0);
  assert.deepEqual(h.sent, []);
});

for (const settled of ["resolve", "reject"]) test(`actual Page: replaced ${settled} callback cannot clear another route or report a stale error`, async () => {
  const h = harness(), old = h.begin(), replacement = h.begin();
  assert.notEqual(old, replacement); assert.equal(h.timer.timers.size, 1);
  const foreignIntent = { kind: "direction", direction: "up" }; h.queuedRef.current = foreignIntent;
  h.loading[0][settled](settled === "resolve" ? h.geometry : Error("old read failed")); await flush();
  assert.equal(h.routeRef.current, replacement); assert.equal(h.queuedRef.current, foreignIntent);
  assert.equal(h.questRouteMovement(old, h.state.self, "run", h.timer.now()), null);
  assert.equal(h.routeRef.current, replacement); assert.deepEqual(terminal(h), []);
  h.cancelQuestRoute(); assert.equal(h.queuedRef.current, foreignIntent); assert.equal(h.timer.timers.size, 0);
});

for (const change of ["quest", "socket", "scene", "blocked"]) test(`actual Page: ${change} change during map loading retires the old objective`, async () => {
  const h = harness(); h.begin();
  if (change === "quest") h.state.live = { ...h.lifetime, questSource: "scarecrow2/2-cat2/2" };
  if (change === "socket") h.state.live = { ...h.lifetime, socket: {} };
  if (change === "scene") h.state.live = { ...h.lifetime, identity: { ...identity, sceneRevision: 2 } };
  if (change === "blocked") h.state.blocked = true;
  h.loading[0].resolve(h.geometry); await flush();
  assert.equal(h.routeRef.current, null); assert.deepEqual(h.sent, []); assert.deepEqual(terminal(h), []);
  assert.equal(h.timer.timers.size, 0);
});

test("actual Page: initial route failure and current loading error both explain unavailability", async () => {
  for (const failedLoad of [false, true]) {
    const h = harness(); h.begin();
    if (failedLoad) h.loading[0].reject(Error("unavailable")); else h.loading[0].resolve(h.geometry);
    await flush(); assert.equal(terminal(h)[0].key, "ui.questRouteUnavailable");
    assert.equal(terminal(h).length, 1); assert.equal(h.timer.timers.size, 0); assert.deepEqual(h.sent, []);
  }
});

function chunk(url, overrides = {}) {
  const q = new URL(url, "https://fixture.invalid").searchParams;
  const minX = Number(q.get("minX")), minY = Number(q.get("minY"));
  return { schemaVersion: 1, source: "crystalMap", mapFileName: q.get("map"), mapWidth: 768, mapHeight: 768,
    geometryFingerprint: "a".repeat(64), bounds: { minX, minY, maxX: Math.min(Number(q.get("maxX")), 767), maxY: Math.min(Number(q.get("maxY")), 767) },
    blockedCells: [{ x: minX, y: minY }], ...overrides };
}

test("collision geometry: at most four concurrent validated readers publish only all nine complete regions", async () => {
  const { QuestCollisionCache } = load("../lib/quest-collision-cache.ts");
  const pending = [], requested = []; let active = 0, peak = 0, settled = false;
  const cache = new QuestCollisionCache(async url => {
    requested.push(url); active++; peak = Math.max(peak, active);
    const row = deferred(); pending.push({ row, url });
    await row.promise; active--; return { ok: true, json: async () => chunk(url) };
  });
  const result = cache.load("0").then(value => { settled = true; return value; });
  assert.equal(requested.length, 1); pending.shift().row.resolve(); await flush();
  assert.equal(requested.length, 5); assert.equal(active, 4); assert.equal(settled, false);
  while (pending.length) { pending.shift().row.resolve(); await flush(); }
  const geometry = await result;
  assert.equal(peak, 4); assert.equal(requested.length, 9); assert.equal(new Set(requested).size, 9);
  for (const x of [0, 256, 512]) for (const y of [0, 256, 512]) {
    const bit = x * 768 + y; assert.notEqual(geometry.blockedBits[bit >>> 3] & 1 << (bit & 7), 0);
  }
  assert.equal(await cache.load("0"), geometry); assert.equal(requested.length, 9);
});

for (const phase of ["headers", "body"]) test(`collision geometry: hung ${phase} aborts at15s, evicts pending and permits a fresh retry`, async () => {
  const timer = clock(), { QuestCollisionCache } = load("../lib/quest-collision-cache.ts", {
    setTimeout: timer.setTimeout, clearTimeout: timer.clearTimeout,
  });
  const late = deferred(); let stalled = true, signal, requested = 0;
  const cache = new QuestCollisionCache(async (url, init) => {
    requested++; signal = init.signal;
    if (stalled && phase === "headers") return late.promise;
    return { ok: true, json: () => stalled ? late.promise : Promise.resolve(chunk(url)) };
  });
  const failed = assert.rejects(cache.load("0"), /timed out/);
  await flush(); timer.elapse(14_999); assert.equal(signal.aborted, false);
  timer.elapse(1); await failed; assert.equal(signal.aborted, true); assert.equal(timer.timers.size, 0);
  stalled = false; const geometry = await cache.load("0"); assert.equal(requested, 10);
  late.resolve(phase === "headers" ? { ok: true, json: async () => chunk("/api/scene/collision?map=0&minX=0&minY=0&maxX=255&maxY=255") } : chunk("/api/scene/collision?map=0&minX=0&minY=0&maxX=255&maxY=255"));
  await flush(); assert.equal(await cache.load("0"), geometry); assert.equal(timer.timers.size, 0);
});

test("collision geometry: a bad parallel region leaves no partial cache and retry validates the entire map", async () => {
  const { QuestCollisionCache } = load("../lib/quest-collision-cache.ts");
  let corrupt = true, requested = 0, active = 0;
  const cache = new QuestCollisionCache(async url => {
    requested++; active++;
    await Promise.resolve(); const value = chunk(url);
    if (corrupt && value.bounds.minX === 256 && value.bounds.minY === 256) value.geometryFingerprint = "b".repeat(64);
    active--; return { ok: true, json: async () => value };
  });
  await assert.rejects(cache.load("0"), /changed/); assert.equal(active, 0);
  const before = requested; corrupt = false; const result = await cache.load("0");
  assert.equal(requested - before, 9); assert.equal(result.fingerprint, "a".repeat(64));
});
