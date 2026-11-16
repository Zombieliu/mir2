import test from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { setImmediate as settle } from "node:timers/promises";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { CompanionController } from "../src/companion-controller.mjs";
import { CompanionModelRuntime } from "../src/model/companion-model-runtime.mjs";

const KEY = "sk-synthetic-model-fixture-only";
const plan = () => ({ reply: "先支援，打两只后回来，途中不拾取。", objective: { kind: "hunt", monsterName: null, count: 2, maxDistanceFromOwner: 6, loot: "never", returnToOwner: true }, rememberedEpisodeIds: [] });
const scene = () => ({ ready: true, connectionEpoch: 1, revision: 1, mapFileName: "0", mapIndex: 0, playerObjectId: 1, selfZoneObjectId: 50_000,
  player: { objectId: 1, kind: "selfPlayer", name: "Alo", x: 10, y: 10, dead: false }, playerHp: 100, playerMaxHp: 100, inventoryItems: [], beltItems: [],
  inventoryStale: false, freeBagSlots: 10, currentWeight: 0, maxWeight: 100, inSafeZone: false,
  party: { allowGroup: true, members: ["Alo", "Owner"], memberInfo: [{ name: "Owner", hp: 100, maxHp: 100, online: true }] }, engagements: [],
  entities: [{ objectId: 2, kind: "player", name: "Owner", x: 13, y: 10, hp: 100, maxHp: 100 },
    { objectId: 10, kind: "monster", name: "Deer", x: 11, y: 10, hp: 20, disposition: "neutral" }],
  groundDrops: [{ objectId: 50, name: "Candle", x: 10, y: 10, ownerObjectId: 50_000 }] });

function fixture(t, { pending = false, maxCallsPerHour = 20 } = {}) {
  const root = mkdtempSync(join(tmpdir(), "alo-model-runtime-")), path = join(root, "alo.sqlite");
  const binding = { realmId: "model-test", residentId: "alo", characterName: "Alo", characterIndex: 1 };
  const opened = [], runtimes = [];
  const open = () => { const s = new CompanionStore({ path, binding }); opened.push(s); return s; };
  let store = open(), resolvePlan, calls = 0, nextPlan = plan();
  class Client extends EventEmitter {
    state = scene(); sent = [];
    snapshot() { return structuredClone(this.state); }
    async attackDirection(direction) { this.sent.push({ kind: "attack", direction }); }
    async walk(direction) { this.sent.push({ kind: "walk", direction }); return { moved: false }; }
    async pickUpTile() { this.sent.push({ kind: "loot" }); return { changed: false }; }
    async useOwnedItem(uniqueId) { this.sent.push({ kind: "potion" }); return { success: true, uniqueId }; }
    frame(packet, payload) {
      if (packet === "ObjectDied") { const e = this.state.entities.find(e => e.objectId === payload.objectId); if (e) e.dead = true; }
      this.emit("frame", { event: { type: "packet", packet, payload } });
    }
  }
  const client = new Client(), controller = new CompanionController({ client, store, leaderName: "Owner", homePosition: { x: 9, y: 10, mapFileName: "0" }, mapLoader: async () => ({ width: 30, height: 30, blocked: new Set() }) });
  const configuration = { provider: "openai", model: "gpt-6-luna", credentialRef: "OPENAI_API_KEY", maxCallsPerHour };
  const create = () => {
    const runtime = new CompanionModelRuntime({ client, store, controller, configuration, environment: { OPENAI_API_KEY: KEY },
      modelClient: { async plan() { calls++; const output = { plan: structuredClone(nextPlan), call: { model: "gpt-6-luna", status: "completed", requestId: "req_mock", inputTokens: 100, outputTokens: 90 } }; return pending ? new Promise(resolve => { resolvePlan = () => resolve(output); }) : output; } } });
    runtimes.push(runtime); return runtime;
  };
  const runtime = create();
  t.after(async () => {
    for (const r of runtimes) await r.close(); for (const s of opened) s.close();
    const absolute = resolve(root); assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("alo-model-runtime-")); rmSync(absolute, { recursive: true, force: true });
  });
  return { client, controller, store, runtime, calls: () => calls, setPlan(p) { nextPlan = p; }, resolvePlan: () => resolvePlan(),
    instruct(id = "one", text = "在附近打两只怪，再回来找我，途中别捡东西。") { runtime.instruct({ requestId: id, text, expectedPolicyVersion: store.profile().policyVersion }); },
    async restart() { await runtime.close(); store.close(); store = open(); controller.store = store; return create(); } };
}

test("model hunt uses ordinary attacks, suppresses loot, and advances only after own hit followed by actual death", async t => {
  const f = fixture(t); f.instruct(); await settle();
  assert.equal(f.runtime.view().modelTask.status, "running"); assert.equal(f.calls(), 1);
  assert.equal((await f.controller.tick()).reason, "model-hunt-observed-monster"); assert.equal(f.client.sent.at(-1).kind, "attack");
  f.client.frame("ObjectRemove", { objectId: 10 }); assert.equal(f.runtime.view().modelTask.completedKills, 0);
  f.client.frame("ObjectStruck", { objectId: 10, attackerId: 999 }); f.client.frame("ObjectDied", { objectId: 10 });
  assert.equal(f.runtime.view().modelTask.completedKills, 0);
  f.client.state.entities.find(e => e.objectId === 10).dead = false;
  f.client.frame("ObjectStruck", { objectId: 10, attackerId: 50_000 }); f.client.frame("ObjectDied", { objectId: 10 });
  f.client.frame("ObjectDied", { objectId: 10 }); assert.equal(f.runtime.view().modelTask.completedKills, 1);
  f.client.state.entities.push({ objectId: 11, kind: "monster", name: "Hen", x: 10, y: 11, hp: 10, disposition: "neutral" });
  assert.equal((await f.controller.tick()).targetId, 11);
  f.client.frame("ObjectStruck", { objectId: 11, attackerId: 1 }); f.client.frame("ObjectDied", { objectId: 11 });
  assert.equal(f.runtime.view().modelTask.completedKills, 2);
  assert.equal((await f.controller.tick()).reason, "model-return-to-owner");
  f.client.state.player.x = 11;
  assert.equal((await f.controller.tick()).reason, "model-objective-completed");
  assert.equal(f.runtime.view().modelTask.status, "completed"); assert.equal(f.client.sent.some(e => e.kind === "loot"), false);
  const observed = f.store.growth().episodes.filter(e => e.payload?.kind === "objective-progress");
  assert.equal(observed.length, 3); assert.ok(observed.every(e => e.payload.outcome === "observed"));
});

test("real safety and learned support retain priority over an apparently valid model hunt", async t => {
  const f = fixture(t); f.store.teachSupportBeforeLoot({ feedbackId: "support" }); f.instruct(); await settle();
  f.client.state.engagements = [{ attackerId: 2, targetId: 10, observedAt: Date.now() }];
  assert.equal((await f.controller.tick()).reason, "assist-confirmed-party-engagement");
  f.client.state.playerHp = 20;
  assert.equal((await f.controller.tick()).kind, "retreat");
  assert.equal(f.runtime.view().modelTask.completedKills, 0);
});

test("an explicit wait suppresses party assistance but retains genuine grouped self-defence", async t => {
  const f = fixture(t);
  f.setPlan({ ...plan(), objective: { kind: "wait", monsterName: null, count: 0, maxDistanceFromOwner: 6, loot: "never", returnToOwner: false } });
  f.store.teachSupportBeforeLoot({ feedbackId: "support" }); f.instruct(); await settle();
  f.client.state.engagements = [{ attackerId: 2, targetId: 10, observedAt: Date.now() }];
  assert.equal((await f.controller.tick()).reason, "model-owner-requested-wait"); assert.equal(f.client.sent.some(e => e.kind === "attack"), false);
  f.client.state.engagements = [{ attackerId: 10, targetId: 50_000, observedAt: Date.now() }];
  assert.equal((await f.controller.tick()).reason, "defend-confirmed-engagement"); assert.equal(f.client.sent.at(-1).kind, "attack");
  f.client.state.entities = f.client.state.entities.filter(entity => entity.name !== "Owner");
  assert.equal((await f.controller.tick()).reason, "defend-confirmed-engagement", "a still-grouped owner leaving AOI must not suppress real self-defence");
});

test("unknown attacker aliases never count as own strikes; death interrupts a task through revival", async t => {
  const f = fixture(t); f.client.state.selfZoneObjectId = null; f.instruct(); await settle();
  await f.controller.tick();
  f.client.frame("ObjectStruck", { objectId: 10, attackerId: null }); f.client.frame("ObjectDied", { objectId: 10 });
  assert.equal(f.runtime.view().modelTask.completedKills, 0);
  f.client.state.entities.find(e => e.objectId === 10).dead = false;
  f.client.state.playerHp = 0; f.client.state.player.dead = true;
  assert.equal((await f.controller.tick()).reason, "dead"); assert.equal(f.runtime.view().modelTask.status, "paused");
  f.client.state.playerHp = 100; f.client.state.player.dead = false;
  assert.notEqual((await f.controller.tick()).reason, "model-hunt-observed-monster");
  assert.equal(f.runtime.view().modelTask.status, "paused");
});

test("deleting a continuation invalidates every cached private instruction and citation", async t => {
  const f = fixture(t), instruction = "跟着我".repeat(170) + "尾段私有经历";
  f.instruct("two-parts", instruction); await settle();
  const goalId = f.runtime.view().modelTask.goalId;
  f.store.deleteEpisode({ episodeId: `${goalId}:part2`, expectedPolicyVersion: 1 });
  f.runtime.invalidateMemory("已删除记忆");
  assert.equal(f.runtime.view().modelTask.instruction, ""); assert.deepEqual(f.runtime.view().modelTask.references, []);
  assert.equal(JSON.stringify(f.runtime.view()).includes("尾段私有经历"), false);
});

test("a one-tile model leash returns inside the budget and refuses a path that would detour outside it", async t => {
  const f = fixture(t), p = plan(); p.objective.maxDistanceFromOwner = 1; f.setPlan(p);
  f.client.state.entities.find(e => e.name === "Owner").x = 12;
  f.instruct(); await settle();
  const returnStep = await f.controller.tick(); assert.equal(returnStep.reason, "model-stay-inside-owner-leash"); assert.equal(returnStep.desiredDistance, 1);
  assert.equal(returnStep.performed, true); assert.equal(f.client.sent.at(-1).kind, "walk");
  const g = fixture(t); g.setPlan(p); g.controller.mapLoader = async () => ({ width: 30, height: 30, blocked: new Set(["10,9", "10,11"]) });
  g.client.state.player.x = 9; g.client.state.entities.find(e => e.name === "Owner").x = 10;
  g.instruct(); await settle(); const bounded = await g.controller.tick();
  assert.equal(bounded.reason, "model-hunt-observed-monster"); assert.equal(bounded.performed, false); assert.equal(bounded.blocked, true);
  assert.equal(g.client.sent.length, 0);
});

test("planning never suppresses a genuine ordinary self-defence engagement when the owner leaves", async t => {
  const f = fixture(t, { pending: true }); f.instruct();
  f.client.state.party.members = []; f.client.state.engagements = [{ attackerId: 10, targetId: 50_000, observedAt: Date.now() }];
  const action = await f.controller.tick(); assert.equal(action.reason, "defend-confirmed-engagement"); assert.equal(action.performed, true);
  assert.equal(f.client.sent.at(-1).kind, "attack"); f.runtime.pause(); f.resolvePlan(); await settle();
});

test("new policy, world boundary, memory deletion and forged memory references invalidate model results", async t => {
  for (const mutate of [
    f => f.store.teachSupportBeforeLoot({ feedbackId: "new-policy" }),
    f => { f.client.state.connectionEpoch++; },
    f => { f.client.state.mapFileName = "0141"; },
  ]) {
    const f = fixture(t, { pending: true }); f.instruct(); mutate(f); f.resolvePlan(); await settle();
    assert.equal(f.runtime.view().modelTask.status, "failed"); assert.equal(f.client.sent.length, 0);
  }
  const f = fixture(t), forged = plan(); forged.rememberedEpisodeIds = ["imaginary-rescue"];
  f.setPlan(forged); f.instruct(); await settle(); assert.equal(f.runtime.view().modelTask.status, "failed");
  assert.equal(f.store.growth().episodes.some(e => e.payload?.kind === "model-plan"), false);
});

test("recall cancels planning immediately and late model output never changes the old goal", async t => {
  const f = fixture(t, { pending: true }); f.instruct();
  f.controller.recallRequested = true; f.runtime.pause("主人召回");
  assert.equal((await f.controller.tick()).reason, "owner-recall");
  f.resolvePlan(); await settle(); assert.equal(f.runtime.view().modelTask.status, "paused");
  assert.equal(f.store.growth().episodes.some(e => e.payload?.kind === "model-plan"), false);
});

test("instruction retries and restart preserve durable history without another model request", async t => {
  const f = fixture(t); f.instruct("retry-id"); await settle(); f.instruct("retry-id");
  assert.equal(f.calls(), 1);
  const restarted = await f.restart();
  assert.equal(restarted.view().modelTask.status, "paused"); assert.match(restarted.view().modelTask.instruction, /打两只/);
  restarted.instruct({ requestId: "retry-id", text: "在附近打两只怪，再回来找我，途中别捡东西。", expectedPolicyVersion: 1 });
  assert.equal(f.calls(), 1); assert.equal(restarted.view().modelCalls, 1);
});

test("restart recovers the requested quantity and server-observed progress without executing the old goal", async t => {
  const f = fixture(t); f.instruct(); await settle(); await f.controller.tick();
  f.client.frame("ObjectStruck", { objectId: 10, attackerId: 50_000 }); f.client.frame("ObjectDied", { objectId: 10 });
  const reopened = await f.restart(), task = reopened.view().modelTask;
  assert.equal(task.status, "paused"); assert.equal(task.requestedKills, 2); assert.equal(task.completedKills, 1);
  assert.equal(reopened.view().modelCalls, 1); assert.equal(f.calls(), 1);
  assert.equal(f.controller.modelDriver, reopened);
});

test("deleted model call memories cannot reset hourly spending cap, and unconfigured runtime cannot call", async t => {
  const f = fixture(t, { maxCallsPerHour: 1 }); f.instruct(); await settle();
  const call = f.store.growth().episodes.find(e => e.payload?.kind === "model-call");
  f.store.deleteEpisode({ episodeId: call.episodeId, expectedPolicyVersion: 1 });
  assert.throws(() => f.instruct("next"), { code: "call-budget" }); assert.equal(f.calls(), 1);
  const runtime = new CompanionModelRuntime({ client: f.client, store: f.store, controller: f.controller, configuration: { provider: "rules", model: "rules" }, environment: {} });
  try { assert.equal(runtime.view().configured, false); assert.throws(() => runtime.instruct({ requestId: "new", text: "跟着我", expectedPolicyVersion: f.store.profile().policyVersion }), { code: "configuration" }); }
  finally { await runtime.close(); }
});
