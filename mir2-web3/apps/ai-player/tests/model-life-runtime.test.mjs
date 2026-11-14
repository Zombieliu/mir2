import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { CompanionModelRuntime } from "../src/model/companion-model-runtime.mjs";
import { OpenAIPlanClient, ModelCallError } from "../src/model/openai-client.mjs";
import { LIFE_PLAN_SCHEMA, validateLifePlan } from "../src/model/life-plan.mjs";
import { projectLifeResources } from "../src/life/resources.mjs";

// Every provider in this file is a test double or a Responses client with a
// synthetic fetch implementation. These tests never call an external model.
const KEY = "sk-synthetic-life-model-test-only";
const PLAN = () => ({ mode: "hunt", waypointIndex: null, reply: "先观察已允许的路线，健康且有药时再巡猎。", rememberedEpisodeIds: [] });
const ROUTE = [{ x: 286, y: 628 }, { x: 294, y: 633 }, { x: 301, y: 626 }];
const settle = () => new Promise(resolve => setImmediate(resolve));
const ownerGoal = () => ({ reply: "在安全范围内跟随。", objective: { kind: "follow", monsterName: null, count: 0, maxDistanceFromOwner: 6, loot: "never", returnToOwner: false }, rememberedEpisodeIds: [] });
const teaching = () => ({ supported: true, reply: "建议组队时先支援。", changes: { assistBeforeLoot: true, lootOnlyAfterCombat: null, askBeforeLeaving: null }, rememberedEpisodeIds: [] });

function fixture(t, { pending = false, provider = "openai", configured = true, maxCallsPerHour = 20, modelClient } = {}) {
  const root = mkdtempSync(join(tmpdir(), "alo-life-model-")), path = join(root, "memory.sqlite"), stores = [], runtimes = [];
  const clock = { now: Date.now() }, binding = { realmId: "life-model-test", residentId: "alo", characterName: "Alo", characterIndex: 2 };
  const identity = { archiveId: "archive:life-model", ownerRef: "owner:life-model", worldEpoch: "world:life-model" };
  let store, runtime, nextPlan = PLAN(); const inputs = [], waiting = [], usage = [];
  const open = () => { store = new CompanionStore({ path, binding, ...identity }); stores.push(store); return store; }; open();
  class Client extends EventEmitter {
    worldEpoch = "world:life-model"; activeCharacterIndex = 2;
    state = { ready: true, connectionEpoch: 1, revision: 1, mapFileName: "0", mapIndex: 0, playerObjectId: 12, selfZoneObjectId: 50001,
      player: { objectId: 12, kind: "selfPlayer", name: "Alo", x: 288, y: 616, dead: false }, playerHp: 18, playerMaxHp: 18, gold: 85,
      inventoryStale: false, inventoryItems: [{ uniqueId: 1n, name: "(HP)DrugSmall", quantity: 2, container: "bag1" }], beltItems: [],
      freeBagSlots: 40, currentWeight: 5, maxWeight: 100, entities: [], groundDrops: [], party: { members: [] },
      accountId: "synthetic-private-account", serverMac: "synthetic-private-admission", admission: "synthetic-private-grant" };
    snapshot() { return structuredClone(this.state); }
  }
  const client = new Client(), lifeView = { status: "running", lifePermitId: "life:phase-test", activationPending: false, modelDecisionsUsed: 1,
    startedAtMs: clock.now - 1000, expiresAtMs: clock.now - 1000 + 1_200_000, durationSeconds: 1200 };
  const controller = { leaderName: "Owner", recallRequested: false, lifeController: { view: () => structuredClone(lifeView) }, resourceView: () => projectLifeResources(client.snapshot(), { observedAtMs: clock.now }) };
  const configuration = { provider, model: provider === "rules" ? "rules" : "gpt-6-luna", credentialRef: provider === "rules" ? null : "OPENAI_API_KEY", maxCallsPerHour };
  const fake = { async plan(input) {
    inputs.push(input);
    const output = { plan: structuredClone(input.schema.properties.mode ? nextPlan : input.schema.properties.supported ? teaching() : ownerGoal()),
      call: { model: "gpt-6-luna", status: "completed", requestId: "req_synthetic_life", inputTokens: 17, outputTokens: 9 } };
    return pending ? await new Promise(resolve => { waiting.push({ resolve: () => resolve(output), input }); }) : output;
  } };
  const create = () => { runtime = new CompanionModelRuntime({ client, store, controller, configuration, environment: configured ? { OPENAI_API_KEY: KEY } : {},
    clock: () => clock.now, modelClient: modelClient ?? fake, onUsage: event => usage.push(event) }); runtimes.push(runtime); return runtime; }; create();
  t.after(async () => { for (const instance of runtimes) await instance.close(); for (const value of stores) value.close(); });
  const input = (changes = {}) => ({ lifePermitId: lifeView.lifePermitId, reason: "authorized", state: client.snapshot(), resources: { ...controller.resourceView(), credential: "synthetic-private-resource" }, routeId: "bichon-south-v1", waypoints: structuredClone(ROUTE), ...changes });
  return { client, controller, lifeView, clock, inputs, waiting, usage, configuration, get store() { return store; }, get runtime() { return runtime; },
    input, plan: changes => runtime.planLife(input(changes)), setPlan(value) { nextPlan = value; }, resolveNext() { waiting.shift().resolve(); },
    async restart() { await runtime.close(); store.close(); open(); return create(); } };
}

test("finite life schema permits only route, hunt and rest, bounded ordinal and grounded citation syntax", () => {
  assert.deepEqual(validateLifePlan(PLAN()), { mode: "hunt", reply: PLAN().reply, rememberedEpisodeIds: [] });
  assert.equal(validateLifePlan({ ...PLAN(), mode: "route", waypointIndex: 2 }).waypointIndex, 2);
  for (const invalid of [{ ...PLAN(), maxGold: 50 }, { ...PLAN(), position: { x: 288, y: 616 } }, { ...PLAN(), mode: "trade" },
    { ...PLAN(), waypointIndex: 3 }, { ...PLAN(), waypointIndex: -1 }, { ...PLAN(), waypointIndex: "0" }, { ...PLAN(), reply: "字".repeat(401) },
    { ...PLAN(), reply: "sk-synthetic-model-secret-forbidden" }, { ...PLAN(), rememberedEpisodeIds: ["same", "same"] }, { ...PLAN(), rememberedEpisodeIds: ["invalid id"] }]) {
    assert.throws(() => validateLifePlan(invalid), { code: "invalid-plan" });
  }
  assert.equal(LIFE_PLAN_SCHEMA.additionalProperties, false); assert.equal(LIFE_PLAN_SCHEMA.properties.reply.maxLength, 400);
});

test("a valid independent phase uses bounded current resources and memories without forwarding identities, grants or caller secrets", async t => {
  const f = fixture(t); f.store.recordEpisode({ episodeId: "observed:previous-home", kind: "life-progress", action: "return-home", outcome: "observed", summary: "上次实际回到比奇安全点旁。" });
  f.setPlan({ ...PLAN(), rememberedEpisodeIds: ["observed:previous-home"] });
  const output = await f.plan({ resources: { gold: 999999, apiKey: "synthetic-private-resource" } });
  assert.equal(output.mode, "hunt"); assert.equal(f.inputs.length, 1); assert.equal(f.usage.length, 1);
  const input = f.inputs[0], serialized = JSON.stringify(input.context);
  assert.equal(input.context.resources.gold, 85); assert.equal(input.context.resources.hpPotionCount, 2);
  assert.deepEqual(input.context.route.waypoints, ROUTE); assert.equal(input.context.world.owner.visible, false);
  assert.equal(input.context.world.party.grouped, false); assert.ok(Buffer.byteLength(serialized) <= 16 * 1024);
  for (const secret of [KEY, "synthetic-private-account", "synthetic-private-admission", "synthetic-private-grant", "synthetic-private-resource", f.lifeView.lifePermitId]) assert.equal(serialized.includes(secret), false);
  const growth = f.store.growth().episodes;
  assert.equal(growth.filter(row => row.payload?.kind === "model-call").length, 1);
  const proposal = growth.find(row => row.payload?.kind === "model-plan"); assert.equal(proposal.payload.outcome, "proposed"); assert.equal(proposal.payload.action, "life-hunt");
  assert.equal(f.runtime.view().lifePlan.references[0].episodeId, "observed:previous-home");
  assert.deepEqual(JSON.parse(growth.find(row => row.payload?.kind === "model-usage").payload.summary), { inputTokens: 17, outputTokens: 9 });
  assert.equal(f.runtime.view().lifePlan.lastCall.inputTokens, 17); assert.equal(f.runtime.view().modelCalls, 1);
});

test("the existing Responses client uses strict life JSON with synthetic fetch and no live API or implicit retries", async t => {
  const requests = [];
  const client = new OpenAIPlanClient({ apiKey: KEY, model: "gpt-6-luna", fetchImpl: async (url, options) => {
    requests.push({ url, body: JSON.parse(options.body) });
    return new Response(JSON.stringify({ status: "completed", model: "gpt-6-luna", output: [{ type: "message", role: "assistant", content: [{ type: "output_text", text: JSON.stringify({ ...PLAN(), mode: "route", waypointIndex: 1 }) }] }], usage: { input_tokens: 31, output_tokens: 12 } }), { headers: { "content-type": "application/json", "x-request-id": "req_synthetic_fetch" } });
  } });
  const f = fixture(t, { modelClient: client }); assert.equal((await f.plan()).waypointIndex, 1);
  assert.equal(requests.length, 1); assert.equal(requests[0].url, "https://api.openai.com/v1/responses"); assert.equal(requests[0].body.store, false);
  assert.equal(requests[0].body.text.format.strict, true); assert.deepEqual(requests[0].body.text.format.schema, LIFE_PLAN_SCHEMA);
  assert.equal(f.runtime.view().lifePlan.lastCall.outputTokens, 12);
});

test("rules or absent local configuration makes no model attempt or cost record", async t => {
  for (const options of [{ provider: "rules" }, { configured: false }]) {
    const f = fixture(t, options); assert.equal(await f.plan(), null); assert.equal(f.inputs.length, 0); assert.equal(f.runtime.view().modelCalls, 0); assert.equal(f.usage.length, 0);
  }
});

test("unactivated, inactive, expired, out-of-world or noncurrent life phases never issue a provider request", async t => {
  for (const change of [f => { f.lifeView.activationPending = true; }, f => { f.lifeView.status = "paused"; }, f => { f.lifeView.status = "returning"; },
    f => { f.lifeView.modelDecisionsUsed = 0; }, f => { f.lifeView.modelDecisionsUsed = 5; }, f => { f.clock.now = f.lifeView.expiresAtMs; },
    f => { f.client.state.mapFileName = "0141"; }, f => { f.client.worldEpoch = "other-world"; }, f => { f.client.activeCharacterIndex = 3; }, f => { f.controller.recallRequested = true; },
    f => { f.client.state.playerHp = 0; }, f => { f.lifeView.startedAtMs = f.clock.now + 1; }]) {
    const f = fixture(t); change(f); assert.equal(await f.plan(), null); assert.equal(f.inputs.length, 0); assert.equal(f.usage.length, 0);
  }
  const f = fixture(t); assert.equal(await f.plan({ lifePermitId: "life:stale" }), null);
  assert.equal(await f.plan({ routeId: "custom-route" }), null); assert.equal(await f.plan({ waypoints: [{ x: 999, y: 999 }] }), null); assert.equal(f.inputs.length, 0);
});

test("each life ordinal has one durable call identity including repeats, tombstones and different runtime instances", async t => {
  const f = fixture(t); await f.plan(); assert.equal(await f.plan({ reason: "resumed" }), null); assert.equal(f.inputs.length, 1);
  const id = `life:${createHash("sha256").update(`${f.lifeView.lifePermitId}:1`).digest("hex")}:call`;
  assert.equal(f.store.growth().episodes.some(row => row.episodeId === id), true);
  f.store.deleteEpisode({ episodeId: id }); await f.restart();
  assert.equal(await f.plan(), null); assert.equal(f.inputs.length, 1); assert.equal(f.runtime.view().modelCalls, 1);
  f.lifeView.modelDecisionsUsed = 2; assert.ok(await f.plan()); assert.equal(f.inputs.length, 2);
});

test("life, goals and teaching share the same durable hourly quota even after deleting call memories and restarting", async t => {
  const f = fixture(t, { maxCallsPerHour: 1 }); await f.plan();
  assert.throws(() => f.runtime.instruct({ requestId: "goal:blocked", text: "跟随主人", expectedPolicyVersion: 1 }), { code: "call-budget" });
  assert.throws(() => f.runtime.proposeTeaching({ requestId: "teaching:blocked", text: "组队先支援", expectedPolicyVersion: 1 }), { code: "call-budget" });
  const call = f.store.growth().episodes.find(row => row.payload?.kind === "model-call"); f.store.deleteEpisode({ episodeId: call.episodeId }); await f.restart();
  f.lifeView.modelDecisionsUsed = 2; assert.equal(await f.plan(), null); assert.equal(f.inputs.length, 1); assert.equal(f.runtime.view().modelCalls, 1);
  const other = fixture(t, { maxCallsPerHour: 1 }); other.runtime.instruct({ requestId: "first-goal", text: "跟随主人", expectedPolicyVersion: 1 }); await settle();
  assert.equal(await other.plan(), null); assert.equal(other.inputs.length, 1);
});

test("the life interface accepts no ordinal beyond four and never bypasses the global twenty-call ceiling", async t => {
  const f = fixture(t);
  for (let ordinal = 1; ordinal <= 4; ordinal++) { f.lifeView.modelDecisionsUsed = ordinal; assert.ok(await f.plan()); }
  f.lifeView.modelDecisionsUsed = 5; assert.equal(await f.plan(), null); assert.equal(f.inputs.length, 4);
  const capped = fixture(t, { maxCallsPerHour: 100 });
  for (let index = 0; index < 20; index++) capped.store.recordEpisode({ episodeId: `goal:${createHash("sha256").update(String(index)).digest("hex")}:call`, kind: "model-call", action: "responses", outcome: "attempted" });
  assert.equal(await capped.plan(), null); assert.equal(capped.inputs.length, 0);
});

test("late responses cannot apply after world, activation, life identity, ordinal, recall, deadline or policy changes", async t => {
  for (const change of [f => { f.client.state.connectionEpoch++; }, f => { f.lifeView.activationPending = true; }, f => { f.lifeView.lifePermitId = "life:new"; },
    f => { f.lifeView.modelDecisionsUsed++; }, f => { f.controller.recallRequested = true; }, f => { f.clock.now = f.lifeView.expiresAtMs; },
    f => { f.store.teachSupportBeforeLoot({ feedbackId: "changed-during-life-plan" }); }]) {
    const f = fixture(t, { pending: true }), task = f.plan(); await settle(); change(f); f.resolveNext();
    assert.equal(await task, null); assert.equal(f.store.growth().episodes.some(row => row.payload?.kind === "model-plan"), false); assert.equal(f.runtime.view().lifePlan.references.length, 0);
  }
});

test("deleted or invented citations never become a model proposal and invalidation aborts without leaking old context", async t => {
  const f = fixture(t, { pending: true }); f.store.recordEpisode({ episodeId: "remembered:actual-home", kind: "life-progress", action: "return-home", outcome: "observed", summary: "实际回到安全点。" });
  f.setPlan({ ...PLAN(), rememberedEpisodeIds: ["remembered:actual-home"] }); const task = f.plan(); await settle();
  f.store.deleteEpisode({ episodeId: "remembered:actual-home" }); f.runtime.invalidateMemory("记忆已删除。", { clear: true });
  assert.equal(f.waiting[0].input.signal.aborted, true); f.resolveNext(); assert.equal(await task, null); assert.equal(f.runtime.view().lifePlan.status, "idle");
  assert.equal(f.store.growth().episodes.some(row => row.payload?.kind === "model-plan"), false);
  const invented = fixture(t); invented.setPlan({ ...PLAN(), rememberedEpisodeIds: ["not:in-context"] }); assert.equal(await invented.plan(), null);
  assert.equal(invented.runtime.view().lifePlan.lastCall.inputTokens, 17);
});

test("deleting a referenced fact without a separate runtime notification still invalidates the asynchronous result", async t => {
  const f = fixture(t, { pending: true }); f.store.recordEpisode({ episodeId: "remembered:deletable-home", kind: "life-progress", action: "return-home", outcome: "observed", summary: "实际到家。" });
  f.setPlan({ ...PLAN(), rememberedEpisodeIds: ["remembered:deletable-home"] }); const task = f.plan(); await settle();
  f.store.deleteEpisode({ episodeId: "remembered:deletable-home" }); f.resolveNext();
  assert.equal(await task, null); assert.equal(f.store.growth().episodes.some(row => row.payload?.kind === "model-plan"), false);
  assert.equal(f.runtime.view().lifePlan.references.length, 0);
});

test("an unpersisted call cannot reach the provider, while a persisted interrupted attempt cannot be charged again", async t => {
  const f = fixture(t), record = f.store.recordEpisode.bind(f.store);
  f.store.recordEpisode = event => { if (event.kind === "model-call") throw new Error("synthetic storage failure"); return record(event); };
  assert.equal(await f.plan(), null); assert.equal(f.inputs.length, 0); assert.equal(f.usage.length, 0); assert.equal(f.runtime.view().modelCalls, 0);
  f.store.recordEpisode = record;
  assert.ok(await f.plan()); assert.equal(f.inputs.length, 1);
  const cancelled = fixture(t); await cancelled.runtime.close(); let calls = 0;
  const runtime = new CompanionModelRuntime({ client: cancelled.client, store: cancelled.store, controller: cancelled.controller, configuration: cancelled.configuration,
    clock: () => cancelled.clock.now, environment: { OPENAI_API_KEY: KEY }, modelClient: { async plan() { calls++; return { plan: PLAN() }; } },
    onUsage: () => { cancelled.lifeView.activationPending = true; } }); t.after(() => runtime.close());
  assert.equal(await runtime.planLife(cancelled.input()), null); assert.equal(calls, 0); assert.equal(runtime.view().modelCalls, 1);
  cancelled.lifeView.activationPending = false;
  assert.equal(await runtime.planLife(cancelled.input()), null); assert.equal(calls, 0);
});

test("strict JSON rejects economic fields and provider failure is safe, counted once and never retried", async t => {
  const f = fixture(t); f.setPlan({ ...PLAN(), budgetGold: 999, sellEquipment: true }); assert.equal(await f.plan(), null);
  assert.equal(f.runtime.view().lifePlan.status, "failed"); assert.equal(f.inputs.length, 1); assert.equal(await f.plan(), null); assert.equal(f.inputs.length, 1);
  let calls = 0;
  const failed = fixture(t, { modelClient: { async plan() { calls++; throw new Error("synthetic-private-provider-error-payload"); } } });
  assert.equal(await failed.plan(), null); assert.equal(await failed.plan(), null); assert.equal(calls, 1);
  assert.equal(JSON.stringify(failed.runtime.view()).includes("synthetic-private-provider-error-payload"), false);
  assert.equal(failed.store.growth().episodes.some(row => row.payload?.kind === "model-plan"), false);
});

test("missing usage remains null rather than an invented zero or monetary estimate", async t => {
  const f = fixture(t, { modelClient: { async plan() { return { plan: PLAN(), call: { status: "completed", model: "gpt-6-luna" } }; } } });
  assert.ok(await f.plan()); assert.equal(f.runtime.view().lifePlan.lastCall.inputTokens, null); assert.equal(f.runtime.view().lifePlan.lastCall.outputTokens, null);
  const usage = f.store.growth().episodes.find(row => row.payload?.kind === "model-usage"); assert.deepEqual(JSON.parse(usage.payload.summary), { inputTokens: null, outputTokens: null });
});

test("closing aborts the phase request and late provider data cannot write to the closed memory store", async t => {
  const f = fixture(t, { pending: true }), task = f.plan(); await settle();
  await f.runtime.close(); assert.equal(f.waiting[0].input.signal.aborted, true); f.store.close(); f.resolveNext(); assert.equal(await task, null);
});
