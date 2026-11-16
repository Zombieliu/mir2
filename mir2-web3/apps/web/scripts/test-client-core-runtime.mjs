import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const manifest = JSON.parse(readFileSync(new URL("../lib/generated/client_core_runtime.json", import.meta.url), "utf8"));
const packageRoot = new URL(`../public/client-core/${manifest.version}/`, import.meta.url);
const glue = readFileSync(new URL("mir2_platform_web.js", packageRoot), "utf8");
const binary = readFileSync(new URL("mir2_platform_web_bg.wasm", packageRoot));
const runtime = await import(`data:text/javascript;base64,${Buffer.from(glue).toString("base64")}`);
await runtime.default({ module_or_path: binary });

const input = (overrides = {}) => ({
  action: "accept", profile: "newcomer-v2", status: "notStarted",
  acceptNpcIndex: 0, finishNpcIndex: 0, selectableRewardIndices: [],
  dialogActionOffered: false, pending: false, ...overrides,
});
const resolve = (request) => JSON.parse(runtime.resolve_quest_action(JSON.stringify(request)));

test("actual WASM ABI and small renderer-independent payload", () => {
  assert.equal(runtime.client_core_abi_version(), 1);
  assert.equal(binary.length, manifest.files["mir2_platform_web_bg.wasm"].bytes);
  assert.ok(binary.length < 256 * 1024, `quest bridge is ${binary.length} bytes; review mobile startup budget`);
});

test("actual Ranking Inspect ABI is provided by Core with the same guarded admission", () => {
  assert.equal(runtime.ranking_inspect_abi_version(), 1);
  const admit = (json) => JSON.parse(runtime.ranking_inspect_admission(json));
  assert.deepEqual(admit("[1,true,true,false,0,1001,1000]"),
    { version: 1, ok: true, objectId: 0, nextReadyMs: 1501 });
  for (const json of ["[1,true,true,false,0,1000,1000]", "[1,true,true,false,-0,1001,1000]",
    '{"version":1,"vers\\u0069on":1,"opened":true,"rankingsReady":true,"pending":false,"playerId":0,"nowMs":1001,"nextReadyMs":1000}']) {
    assert.deepEqual(admit(json), { version: 1, ok: false });
  }
  const presentation = readFileSync(new URL(
    `../public/client-core/${manifest.presentation.version}/mir2_platform_web_bg.wasm`, import.meta.url));
  assert.equal(WebAssembly.Module.exports(new WebAssembly.Module(presentation))
    .some(({ name }) => name.startsWith("ranking_inspect_")), false);
});

test("packaged Presentation plays an ordered reel and recast from one packet projection", async () => {
  const root = new URL(`../public/client-core/${manifest.presentation.version}/`, import.meta.url);
  const glue = readFileSync(new URL("mir2_platform_web.js", root));
  const binary = readFileSync(new URL("mir2_platform_web_bg.wasm", root));
  const presentation = await import(`data:text/javascript;base64,${glue.toString("base64")}`);
  await presentation.default({ module_or_path: binary });
  const bridge = new presentation.EntityAnimationBridge();
  try {
    const pose = (nowMs, fishing, extra) => JSON.parse(bridge.resolveMir2EntityAnimationPoses(JSON.stringify({
      worldKey: "fishing-test", worldSeed: 7, nowMs,
      entities: [{ objectId: "7", kind: "selfPlayer", action: "standing", direction: "Right", fishing, ...extra }],
    }))).poses[0];
    assert.equal(pose(1000, false).action, "standing");
    assert.equal(pose(1050, true).action, "fishingCast");
    assert.equal(pose(2000, true, { fishingEdges: 1 }).action, "fishingWait");
    assert.equal(pose(2570, true).action, "fishingReel");
    assert.equal(pose(3370, true).action, "fishingCast");
    assert.equal(pose(4170, true).action, "fishingWait");
  } finally { bridge.free(); }
});

test("actual V2 N2 endpoints allow diary accept and authoritative ready finish", () => {
  const content = JSON.parse(readFileSync(new URL("../../../config/quest-guidance/newcomer-journey-v2.json", import.meta.url), "utf8"));
  const quest = content.quests.find((entry) => entry.id === 2110002);
  const request = input({ acceptNpcIndex: quest.startNpcId, finishNpcIndex: quest.finishNpcId });
  assert.deepEqual(resolve(request), { eligible: true, source: "diary", npcIndex: 0 });
  assert.equal(resolve({ ...request, action: "finish", status: "inProgress" }).rejection, "wrongStatus");
  assert.equal(resolve({ ...request, action: "finish", status: "readyToTurnIn" }).source, "diary");
});

test("missing endpoints, wrong profile and malformed metadata never invent diary actions", () => {
  for (const acceptNpcIndex of [undefined, null, "0", -1, 0.5, 2 ** 32]) {
    assert.equal(resolve(input({ acceptNpcIndex })).eligible, false);
  }
  assert.equal(resolve(input({ profile: "crystal" })).eligible, false);
  assert.equal(resolve(input({ profile: "platinum_176" })).eligible, false);
  assert.equal(resolve(input({ action: "finish", status: "readyToTurnIn", finishNpcIndex: undefined })).eligible, false);
});

test("ordinary NPC uses current object id and offer, independently of template index", () => {
  const request = input({ profile: "crystal", acceptNpcIndex: 24, dialogNpcIndex: 10024, dialogActionOffered: true });
  assert.deepEqual(resolve(request), { eligible: true, source: "npcDialog", npcIndex: 10024 });
  assert.equal(resolve({ ...request, dialogActionOffered: false }).eligible, false);
  assert.equal(resolve({ ...request, dialogNpcIndex: 0 }).eligible, false);
  assert.equal(resolve({ ...request, status: null }).eligible, true);
  assert.equal(resolve({ ...request, status: "other" }).eligible, false);
});

test("pending and sparse reward selection guards remain active with an open NPC", () => {
  const request = input({ action: "finish", status: "readyToTurnIn", selectableRewardIndices: [2, 4],
    dialogNpcIndex: 10024, dialogActionOffered: true });
  assert.equal(resolve(request).rejection, "rewardSelectionRequired");
  assert.equal(resolve({ ...request, selectedRewardIndex: 0 }).rejection, "rewardSelectionRequired");
  assert.equal(resolve({ ...request, selectedRewardIndex: 4 }).source, "diary");
  assert.equal(resolve({ ...request, selectedRewardIndex: 4, pending: true }).rejection, "pending");
});

test("actual read-only Cash Preview layer and turn exports move together to Core", () => {
  assert.equal(runtime.cash_preview_abi_version(), 1);
  assert.equal(runtime.cash_preview_turn(8, true), 1);
  assert.equal(runtime.cash_preview_turn(1, false), 8);
  assert.equal(runtime.cash_preview_turn(0, true), 0);
  assert.deepEqual(JSON.parse(runtime.cash_preview_layers(1, 9, 1, 0, false, 2, 150n)), {
    version: 1, known: true, direction: 2,
    layers: [{ library: "CArmour/00", frame: 39 }, { library: "CWeapon/09", frame: 39 }],
  });
  assert.deepEqual(JSON.parse(runtime.cash_preview_layers(1, -1, 1, 0, false, 6, 0n)),
    { version: 1, known: false, direction: 6, layers: [] });
  for (const bundle of [manifest.presentation, manifest.npcPurchase]) {
    const bytes = readFileSync(new URL(
      `../public/client-core/${bundle.version}/mir2_platform_web_bg.wasm`, import.meta.url));
    assert.ok(bytes.length < 256 * 1024);
    assert.equal(WebAssembly.Module.exports(new WebAssembly.Module(bytes))
      .some(({ name }) => name.startsWith("cash_preview_")), false);
  }
});

test("actual Presentation v2 observes queued body blockers without exposing them to the v1 reader", async () => {
  const root = new URL(`../public/client-core/${manifest.presentation.version}/`, import.meta.url);
  const glue = readFileSync(new URL("mir2_platform_web.js", root));
  const binary = readFileSync(new URL("mir2_platform_web_bg.wasm", root));
  const presentation = await import(`data:text/javascript;base64,${glue.toString("base64")}`);
  await presentation.default({ module_or_path: binary });
  const worldKey = "movement-test", worldSeed = 22, objectId = "7", actionSourceKey = "owner:7";
  const identity = { worldKey, worldSeed, objectId, actionSourceKey };

  function withBridge(run) {
    const bridge = new presentation.EntityAnimationBridge();
    try {
      const read = (version, query = {}) => JSON.parse(bridge.getMir2EntityActionPose(JSON.stringify({
        version, ...identity, ...query,
      })));
      const step = (nowMs, action, actionSourceRevision, actionToken, extra) => {
        const result = JSON.parse(bridge.resolveMir2EntityAnimationPoses(JSON.stringify({
          worldKey, worldSeed, nowMs,
          entities: [{ objectId, kind: "selfPlayer", action, direction: "Right", actionSourceKey,
            actionSourceRevision, actionKnown: true, ...(actionToken ? { actionToken } : {}), ...extra }],
        })));
        assert.equal(result.errors, undefined);
        return result.poses[0];
      };
      step(1000, "standing", 1, null, { bootstrapKnown: true });
      return run(step, read);
    } finally { bridge.free(); }
  }
  function feed(pending) {
    return withBridge((step, read) => {
      step(1010, "walking", 2, "owner:walk", { movementActive: true });
      step(1100, "struck", 3, "owner:struck1");
      step(1200, pending, 4, "owner:queued");
      const pose = step(1810, "struck", 5, "owner:struck2");
      assert.equal(pose.queueDepth, 2);
      const v1 = read(1), v2 = read(2);
      assert.deepEqual(read(2, { actionSourceKey: "foreign" }), { version: 1, known: false });
      assert.deepEqual(read(3), { version: 1, known: false });
      const { action, version, ...proof } = v2;
      const { action: v1Action, version: v1Version, ...oldProof } = v1;
      assert.equal(v1Version, 1);
      assert.equal(v1Action, "struck");
      assert.equal(version, 2);
      assert.deepEqual(proof, oldProof);
      return { pose, v1, action, next: step(1910, "struck", 5, "owner:struck2").action };
    });
  }
  const spell = feed("spell"), idle = feed("standing");
  assert.deepEqual(spell.pose, idle.pose);
  assert.deepEqual(spell.v1, idle.v1);
  assert.deepEqual([spell.action, spell.next], ["blocked", "spell"]);
  assert.deepEqual([idle.action, idle.next], ["ready", "standing"]);

  withBridge((step, read) => {
    step(1100, "attack1", 2, "owner:attack");
    step(1699, "attack1", 2, "owner:attack");
    assert.equal(read(2).action, "blocked");
    step(1700, "attack1", 2, "owner:attack");
    assert.equal(read(1).action, "standing");
    assert.equal(read(2).action, "ready");
  });
});
