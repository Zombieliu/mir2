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
