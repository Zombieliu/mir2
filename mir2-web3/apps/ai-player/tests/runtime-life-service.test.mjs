import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { setImmediate as settle } from "node:timers/promises";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { CompanionRuntimeService } from "../src/companion-runtime-service.mjs";
import { grant as lifeGrant, childGrant, world } from "./life-fixture.mjs";

const BINDING = { realmId: "life-service-world", residentId: "alo-life-service", characterName: "Alo", characterIndex: 2 };
const IDENTITY = { archiveId: "life-service-archive", ownerRef: "owner:life-service", worldEpoch: "life-service-epoch", displayName: "阿洛" };
const KEY_ENV = "MIR2_LIFE_SERVICE_SYNTHETIC_TEST_KEY";
const RESOURCES = { observedAtMs: 1000, known: true, hp: 18, maxHp: 18, gold: 85, hpPotionCount: 2, usableHpPotionCount: 2,
  freeBagSlots: 40, currentWeight: 5, maxWeight: 100, inventoryCurrent: true, medicineUnresolved: false };
const grant = (changes = {}) => lifeGrant({ worldEpoch: IDENTITY.worldEpoch, startedAtMs: 1000, expiresAtMs: 1_201_000, ...changes });
const authorize = (value = grant()) => ({ expectedPolicyVersion: 1, budgetGold: 50, durationSeconds: value.durationSeconds, grant: value });

function fixture(t, { resourceMethod = true, model = false } = {}) {
  const directory = mkdtempSync(join(tmpdir(), "mir2-runtime-life-service-test-"));
  const store = new CompanionStore({ path: join(directory, "growth.sqlite"), binding: BINDING, ...IDENTITY });
  const archive = store.archive(), replies = [], actions = [], reports = [];
  let now = 1000, next = 0, modelCalls = 0;
  class Client extends EventEmitter {
    worldEpoch = IDENTITY.worldEpoch;
    activeCharacterIndex = 2;
    registerCompanionRuntime() { return Promise.resolve({ status: "ok", result: { ...archive, generation: 7 } }); }
    replyCompanionRuntime(reply) { replies.push(reply); this.emit("reply", reply); }
    snapshot() { return world({ party: { members: ["Alo", "Owner"] } }); }
  }
  const client = new Client(), controller = { recallRequested: false, leaderName: "Owner",
    ...(resourceMethod ? { resourceView: () => structuredClone(RESOURCES) } : {}) };
  const life = {
    current: { status: "idle", phase: "idle", activationPending: false, lifePermitId: null, resources: { ...RESOURCES, gold: 20 } },
    authorize(value, options) {
      actions.push(["authorizeLife", structuredClone(value), structuredClone(options)]);
      this.current = { ...this.current, lifePermitId: value.lifePermitId, status: "running", activationPending: options?.awaitActivation === true,
        phase: options?.awaitActivation === true ? "awaiting-server-activation" : "patrolling", budgetGold: value.budgetGold, chargedGold: 0,
        purchasedQuantity: 0, expiresAtMs: value.expiresAtMs };
      return this.view();
    },
    resume(value, options) {
      actions.push(["resumeLife", structuredClone(value), structuredClone(options)]);
      this.current = { ...this.current, lifePermitId: value.lifePermitId, status: "running", activationPending: options?.awaitActivation === true,
        phase: options?.awaitActivation === true ? "awaiting-server-activation" : "resting", expiresAtMs: value.expiresAtMs };
      return this.view();
    },
    confirmActivation(lifePermitId) {
      if (lifePermitId !== this.current.lifePermitId || this.current.status !== "running") throw new Error("Only the current pending life can activate.");
      if (this.current.activationPending) { actions.push(["confirmActivation", lifePermitId]); this.current.activationPending = false; this.current.phase = "patrolling"; }
      return this.view();
    },
    cancel(reason) { actions.push(["cancelLife", reason]); this.current = { ...this.current, status: "returning", phase: "returning-home", activationPending: false }; return this.view(); },
    view() { return structuredClone(this.current); },
  };
  const supply = { authorize(value) { actions.push(["authorizeSupply", structuredClone(value)]); }, cancel(reason) { actions.push(["cancelSupply", reason]); }, view: () => ({ status: "idle" }) };
  const gifts = { current: { status: "idle", receivedGold: null, receivedItems: [], reason: "暂无交易" }, view() { return structuredClone(this.current); } };
  const configuration = { runtimeId: "life-service-test", provider: model ? "openai" : "rules", model: model ? "gpt-6-luna" : "rules",
    promptVersion: model ? "alo-objective-v1" : "rules-v1", archiveKeyEnv: KEY_ENV, ...(model ? { credentialRef: "OPENAI_API_KEY" } : {}) };
  const service = new CompanionRuntimeService({ client, store, controller, lifeController: life, giftController: gifts, supplyController: supply,
    configuration, realmId: BINDING.realmId, report: value => reports.push(value), clock: () => now,
    environment: { [KEY_ENV]: "4c".repeat(32), ...(model ? { OPENAI_API_KEY: "sk-synthetic-test-only-no-network" } : {}) },
    ...(model ? { modelClient: { async plan() { modelCalls++; return { plan: { reply: "合成测试计划", objective: { kind: "follow", monsterName: null, count: 0, maxDistanceFromOwner: 6, loot: "never", returnToOwner: false }, rememberedEpisodeIds: [] }, call: { model: "gpt-6-luna", status: "completed" } }; } } } : {}) });
  t.after(async () => {
    await service.close(); store.close();
    const absolute = resolve(directory);
    assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("mir2-runtime-life-service-test-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  return { client, store, controller, service, life, supply, gifts, actions, reports, replies,
    get modelCalls() { return modelCalls; }, advance(ms) { now += ms; },
    frame(operation, payload = {}, requestId = `life-service:${++next}`) {
      return { requestId, residentId: archive.residentId, archiveId: archive.archiveId, ownerRef: archive.ownerRef, worldEpoch: archive.worldEpoch,
        generation: 7, operation, payload, deadlineAtMs: now + 1000 };
    },
    async request(operation, payload = {}, requestId) {
      const frame = this.frame(operation, payload, requestId);
      const result = new Promise((resolve, reject) => {
        const timer = setTimeout(() => { client.off("reply", received); reject(new Error("Life service did not return its private operation receipt.")); }, 1000);
        const received = value => { if (value.requestId === frame.requestId) { clearTimeout(timer); client.off("reply", received); resolve(value); } };
        client.on("reply", received);
      });
      client.emit("companionRequest", frame); return await result;
    },
  };
}

test("life authorization requires a current policy and trusted grant and waits for server activation", async t => {
  const f = fixture(t); await f.service.start();
  assert.equal((await f.request("authorizeLife", { expectedPolicyVersion: 1, budgetGold: 50, durationSeconds: 1200 })).status, "error");
  assert.equal((await f.request("authorizeLife", { ...authorize(), expectedPolicyVersion: 2 })).status, "conflict");
  assert.equal((await f.request("authorizeLife", { ...authorize(), teleport: true })).status, "error");
  assert.equal(f.actions.length, 0);
  const accepted = await f.request("authorizeLife", authorize());
  assert.equal(accepted.status, "ok");
  assert.deepEqual(f.actions, [["authorizeLife", grant(), { awaitActivation: true }]]);
  assert.equal(accepted.result.runtime.lifeTask.status, "running");
  assert.equal(accepted.result.runtime.lifeTask.activationPending, true);
  assert.equal(accepted.result.runtime.lifeTask.phase, "awaiting-server-activation");
  assert.equal(f.actions.some(action => action[0] === "confirmActivation"), false);
});

test("resuming passes the unchanged original grant with activation wait and retains prior spending", async t => {
  const f = fixture(t); await f.service.start();
  const original = grant(); f.life.current = { ...f.life.current, status: "paused", lifePermitId: original.lifePermitId, chargedGold: 40, purchasedQuantity: 1, expiresAtMs: original.expiresAtMs };
  assert.equal((await f.request("resumeLife", { expectedPolicyVersion: 1, lifePermitId: original.lifePermitId })).status, "error");
  assert.equal((await f.request("resumeLife", { expectedPolicyVersion: 2, lifePermitId: original.lifePermitId, grant: original })).status, "conflict");
  const resumed = await f.request("resumeLife", { expectedPolicyVersion: 1, lifePermitId: original.lifePermitId, grant: original });
  assert.equal(resumed.status, "ok"); assert.deepEqual(f.actions, [["resumeLife", original, { awaitActivation: true }]]);
  assert.equal(resumed.result.runtime.lifeTask.activationPending, true);
  assert.equal(resumed.result.runtime.lifeTask.expiresAtMs, original.expiresAtMs);
  assert.equal(resumed.result.runtime.lifeTask.chargedGold, 40); assert.equal(resumed.result.runtime.lifeTask.purchasedQuantity, 1);
});

test("only the authenticated matching internal lifeActivated operation unlocks an accepted life", async t => {
  const f = fixture(t); await f.service.start(); await f.request("authorizeLife", authorize());
  for (const changes of [{ ownerRef: "owner:foreign" }, { generation: 6 }, { archiveId: "archive:foreign" }, { worldEpoch: "epoch:foreign" }, { deadlineAtMs: 999 }]) {
    const frame = { ...f.frame("lifeActivated", { lifePermitId: grant().lifePermitId }), ...changes };
    const before = f.replies.length; f.client.emit("companionRequest", frame); await settle();
    assert.equal(f.replies.length, before); assert.equal(f.life.view().activationPending, true);
  }
  assert.equal((await f.request("lifeActivated", { lifePermitId: "life:wrong" })).status, "error");
  assert.equal((await f.request("lifeActivated", { lifePermitId: grant().lifePermitId, budgetGold: 999 })).status, "error");
  assert.equal(f.life.view().activationPending, true);
  const activated = await f.request("lifeActivated", { lifePermitId: grant().lifePermitId });
  assert.equal(activated.status, "ok"); assert.equal(activated.result.runtime.lifeTask.activationPending, false);
  assert.deepEqual(f.actions.filter(action => action[0] === "confirmActivation"), [["confirmActivation", grant().lifePermitId]]);
});

test("late activation for an older life cannot start the current replacement", async t => {
  const f = fixture(t); await f.service.start(); await f.request("authorizeLife", authorize());
  const replacement = grant({ lifePermitId: "life:replacement" }); await f.request("authorizeLife", authorize(replacement));
  assert.equal((await f.request("lifeActivated", { lifePermitId: grant().lifePermitId })).status, "error");
  assert.equal(f.life.view().lifePermitId, replacement.lifePermitId); assert.equal(f.life.view().activationPending, true);
  assert.equal(f.actions.some(action => action[0] === "confirmActivation"), false);
  assert.equal((await f.request("lifeActivated", { lifePermitId: replacement.lifePermitId })).status, "ok");
});

test("cancelLife and recall cancel the independent life without reactivating it via legacy resume", async t => {
  const f = fixture(t); await f.service.start(); await f.request("authorizeLife", authorize());
  assert.equal((await f.request("cancelLife", { spendLimit: 0 })).status, "error");
  assert.equal((await f.request("cancelLife")).status, "ok");
  assert.ok(f.actions.some(action => action[0] === "cancelLife" && action[1] === "owner-revoked"));
  assert.equal((await f.request("lifeActivated", { lifePermitId: grant().lifePermitId })).status, "error");
  await f.request("recall"); assert.equal(f.controller.recallRequested, true);
  assert.ok(f.actions.some(action => action[0] === "cancelLife" && action[1] === "owner-recalled"));
  await f.request("resume"); assert.equal(f.controller.recallRequested, false);
  assert.equal(f.actions.some(action => action[0] === "resumeLife" || action[0] === "confirmActivation"), false);
});

test("owner runtime view separates live resources, life budget and unknown gifts", async t => {
  const f = fixture(t); await f.service.start(); await f.request("authorizeLife", authorize());
  f.gifts.current = { status: "unknown", receivedGold: null, receivedItems: [], reason: "等待完整状态确认" };
  const viewed = await f.request("view");
  assert.equal(viewed.status, "ok"); assert.deepEqual(viewed.result.runtime.resources, RESOURCES);
  assert.equal(viewed.result.runtime.lifeTask.budgetGold, 50); assert.equal(viewed.result.runtime.resources.gold, 85);
  assert.deepEqual(viewed.result.runtime.giftTask, f.gifts.view()); assert.equal(viewed.result.runtime.giftTask.receivedGold, null);
  assert.equal(JSON.stringify(viewed.result).includes(KEY_ENV), false);
});

test("resource view falls back to observed life resources for a controller test double", async t => {
  const f = fixture(t, { resourceMethod: false }); await f.service.start();
  assert.deepEqual((await f.request("view")).result.runtime.resources, f.life.view().resources);
  f.life.current.resources = null;
  assert.equal((await f.request("view")).result.runtime.resources, null);
});

test("awaiting activation rejects new model instructions and standalone supply without spending or calling a model", async t => {
  const f = fixture(t, { model: true }); await f.service.start(); await f.request("authorizeLife", authorize());
  const instructed = await f.request("instruct", { expectedPolicyVersion: 1, text: "现在立刻出发狩猎。" });
  assert.equal(instructed.status, "error"); assert.equal(f.modelCalls, 0);
  const supplied = await f.request("authorizeSupply", { expectedPolicyVersion: 1, budgetGold: 50, quantity: 1, expiresInSeconds: 300,
    grant: childGrant({ worldEpoch: IDENTITY.worldEpoch }) });
  assert.equal(supplied.status, "error"); assert.equal(f.actions.some(action => action[0] === "authorizeSupply"), false);
  assert.equal(f.life.view().activationPending, true); assert.equal(f.modelCalls, 0);
  assert.equal((await f.request("recall")).status, "ok"); assert.equal(f.controller.recallRequested, true);
});

test("unresolved ordinary gifts prevent authorizing or resuming an independent life", async t => {
  const f = fixture(t); await f.service.start();
  f.gifts.current = { status: "unknown", receivedGold: null, receivedItems: [], reason: "原交易待核对" };
  assert.equal((await f.request("authorizeLife", authorize())).status, "error");
  assert.equal((await f.request("resumeLife", { expectedPolicyVersion: 1, lifePermitId: grant().lifePermitId, grant: grant() })).status, "error");
  assert.equal(f.actions.length, 0);
  f.gifts.current.status = "idle";
  const ordinary = f.client.snapshot.bind(f.client);
  f.client.snapshot = () => ({ ...ordinary(), trade: { active: true, uncertain: false } });
  assert.equal((await f.request("authorizeLife", authorize())).status, "error");
  assert.equal(f.actions.length, 0);
});

test("owner-view failure after local authorization stays unknown and never bypasses server activation", async t => {
  const f = fixture(t); await f.service.start();
  const ownerView = f.store.ownerView.bind(f.store);
  f.store.ownerView = () => { throw new Error("synthetic private view unavailable"); };
  const outcome = await f.request("authorizeLife", authorize());
  assert.equal(outcome.status, "unknown"); assert.equal(f.life.view().activationPending, true);
  assert.equal(f.actions.some(action => action[0] === "confirmActivation"), false);
  assert.equal(JSON.stringify(outcome).includes("synthetic private view unavailable"), false);
  f.store.ownerView = ownerView;
  assert.equal((await f.request("view")).result.runtime.lifeTask.activationPending, true);
});
