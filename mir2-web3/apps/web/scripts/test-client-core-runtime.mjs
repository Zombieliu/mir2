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
