import assert from "node:assert/strict";
import test from "node:test";
import { createServer } from "node:net";
import { EventEmitter } from "node:events";
import { randomUUID } from "node:crypto";
import { mkdtempSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { setImmediate as settle } from "node:timers/promises";
import { acquireResidentGuard, ResidentActor, recordSupplyObservation } from "../src/resident-actor.mjs";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { SupplyController } from "../src/supply/supply-controller.mjs";

async function port() {
  const server = createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const result = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return result;
}

function fakeClient({ characterClass = "Warrior", hp = 18 } = {}) {
  const calls = [];
  const state = {
    ready: true, mapFileName: "0", playerHp: hp, playerMaxHp: 18,
    player: { x: 288, y: 608, direction: "Down", dead: false },
    entities: [], inventoryItems: [], questLog: [], gold: 0,
  };
  return {
    calls, state,
    connect: async () => { calls.push("connect"); },
    login: async () => { calls.push("login"); return [{ name: "AchuanAI", index: 1, class: characterClass }]; },
    createAccount: async () => { calls.push("createAccount"); },
    createCharacter: async () => { calls.push("createCharacter"); },
    startGame: async () => { calls.push("startGame"); return structuredClone(state); },
    snapshot: () => structuredClone(state),
    walk: async () => { calls.push("walk"); return { moved: false, after: state.player }; },
    logout: async () => { calls.push("logout"); return true; },
    close: async () => { calls.push("close"); },
  };
}

async function companionFixture(t, { supplyFactory, cleanup = () => {} } = {}) {
  const root = mkdtempSync(join(tmpdir(), "alo-resident-supply-")), archiveId = `actor-${randomUUID()}`, ownerRef = "owner:test", worldEpoch = "actor-supply-epoch", memoryPath = join(root, "memory.sqlite");
  const client = Object.assign(new EventEmitter(), fakeClient());
  client.features = { sharedParty: true, companionControl: true }; client.worldEpoch = worldEpoch; client.activeCharacterIndex = 1;
  Object.assign(client.state, { connectionEpoch: 1, revision: 1, mapIndex: 0, playerObjectId: 1, selfZoneObjectId: 5000, inventoryStale: false,
    beltItems: [], groundDrops: [], freeBagSlots: 20, currentWeight: 0, maxWeight: 100, party: { members: [] } });
  client.state.player.name = "AchuanAI";
  client.setPeaceAttackMode = async () => { client.calls.push("peace"); }; client.setAllowGroup = async () => { client.calls.push("allowGroup"); };
  client.registerCompanionRuntime = async () => ({ status: "ok", result: { archiveId, ownerRef, worldEpoch, generation: 1 } });
  client.replyCompanionRuntime = reply => { client.emit("reply", reply); };
  const profile = { realmId: "resident-supply-test", characterName: "AchuanAI", guardPort: await port(),
    companion: { residentId: `resident-${randomUUID()}`, archiveId, ownerRef, worldEpoch, characterIndex: 1, leaderName: "Owner", memoryPath },
    runtimeService: { controlEnabled: true, runtimeId: "actor-test-runtime", provider: "rules", model: "rules", promptVersion: "rules-test", credentialRef: null, archiveKeyEnv: "ACTOR_SYNTHETIC_ARCHIVE_KEY" } };
  const actor = new ResidentActor(profile, { client, environment: { ACTOR_SYNTHETIC_ARCHIVE_KEY: "5a".repeat(32) }, ...(supplyFactory ? { supplyFactory } : {}) });
  t.after(async () => { cleanup(); await actor.stop({ save: false }); const absolute = resolve(root); assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("alo-resident-supply-")); rmSync(absolute, { recursive: true, force: true }); });
  await actor.start({ accountId: "synthetic-actor", password: "synthetic-password" });
  return { actor, client, root, memoryPath, profile, async view() {
    const reply = new Promise((resolveReply, reject) => {
      const timer = setTimeout(() => reject(new Error("Synthetic private view did not return")), 1000);
      client.once("reply", result => { clearTimeout(timer); resolveReply(result); });
    });
    client.emit("companionRequest", { requestId: `view-${randomUUID()}`, residentId: profile.companion.residentId, archiveId, ownerRef, worldEpoch, generation: 1, operation: "view", payload: {}, deadlineAtMs: Date.now() + 1000 });
    return (await reply).result;
  } };
}

test("resident injects the separate supply journal and service view without exporting executable grants", async t => {
  let options;
  const f = await companionFixture(t, { supplyFactory: input => { options = input; return new SupplyController(input); } });
  assert.equal(options.dbPath, `${f.memoryPath}.supply.sqlite`); assert.equal(existsSync(options.dbPath), true);
  assert.equal(f.actor.companion.supplyController, f.actor.supplyController);
  assert.equal((await f.view()).runtime.supplyTask.status, "idle");
  const grant = { permitId: "private-supply-permit", worldEpoch: f.client.worldEpoch, actorCharacterIndex: 1, mapFileName: "0", npcIndex: 9, npcName: "Merchant_Ruben", npcPosition: { x: 288, y: 608 }, itemIndex: 658, itemName: "(HP)DrugSmall", quantity: 1, maxGold: 50, expiresAtMs: Date.now() + 60000, returnAnchor: { x: 288, y: 616 }, bounds: { minX: 270, minY: 595, maxX: 310, maxY: 635 } };
  f.actor.supplyController.authorize(grant);
  assert.equal((await f.view()).runtime.supplyTask.status, "authorized");
  const archive = f.actor.store.exportArchive({ key: Buffer.alloc(32, 7) });
  assert.equal(archive.includes(grant.permitId), false); assert.equal(archive.includes("maxGold"), false);
  assert.equal(f.actor.store.profile().permissions.spendLimit, 0);
  f.client.state.player.x = 288; f.client.state.player.y = 616;
  f.actor.companion.mapLoader = async () => ({ width: 400, height: 700, blocked: new Set() });
  const beforeWalks = f.client.calls.filter(call => call === "walk").length;
  assert.equal((await f.actor.companion.tick()).kind, "supply");
  assert.equal(f.client.calls.filter(call => call === "walk").length, beforeWalks + 1, "the real supply controller moves through the shared companion action lane without a visible owner or party");
  const supply = f.actor.supplyController; await f.actor.stop({ save: false });
  assert.throws(() => supply.authorize(grant), /closed/);
  const restored = new SupplyController({ client: f.client, dbPath: options.dbPath });
  try { const count = f.client.calls.length; assert.equal(restored.view().status, "paused"); await restored.tick(); assert.equal(f.client.calls.length, count); }
  finally { await restored.close(); }
});

test("only actual supply acquisition and bounded arrival create idempotent portable observed memories", t => {
  const root = mkdtempSync(join(tmpdir(), "alo-observed-supply-")), store = new CompanionStore({ path: join(root, "growth.sqlite"), binding: { realmId: "observed-test", residentId: "alo", characterName: "Alo", characterIndex: 1 } });
  t.after(() => { store.close(); const absolute = resolve(root); assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("alo-observed-supply-")); rmSync(absolute, { recursive: true, force: true }); });
  const acquired = { kind: "supply-acquired", permitId: "confirmed-permit", chargedGold: 40, acquiredQuantity: 1, receiptRevision: "8", incomingUniqueId: "18446744073709551615" };
  assert.throws(() => recordSupplyObservation(store, { ...acquired, kind: "supply-plan" }));
  assert.throws(() => recordSupplyObservation(store, { ...acquired, acquiredQuantity: 0 }));
  assert.equal(store.growth().episodes.length, 0);
  recordSupplyObservation(store, acquired); const revision = store.growth().growthRevision; recordSupplyObservation(store, acquired); assert.equal(store.growth().growthRevision, revision);
  const returned = { ...acquired, kind: "supply-returned", returnAnchor: { x: 288, y: 616 }, actualPosition: { x: 289, y: 616 }, arrivalDistance: 1,
    scope: { mapFileName: "0", maximumDistanceFromAnchor: 1, bounds: { minX: 270, minY: 595, maxX: 310, maxY: 635 } } };
  assert.throws(() => recordSupplyObservation(store, { ...returned, actualPosition: { x: 292, y: 616 }, arrivalDistance: 4 }));
  recordSupplyObservation(store, returned); recordSupplyObservation(store, { ...returned, actualPosition: { x: 287, y: 616 } });
  assert.equal(store.growth().episodes.length, 2); assert.ok(store.growth().episodes.every(row => row.payload.outcome === "observed"));
  assert.throws(() => recordSupplyObservation(store, { ...acquired, chargedGold: 41 }), /conflicts/);
  assert.equal(store.profile().permissions.spendLimit, 0);
});

test("resident stop waits for a running supply tick then its journal close before client and memory", async t => {
  let finishTick, finishClose, cleanupRequested = false; const order = [];
  const f = await companionFixture(t, { cleanup: () => { cleanupRequested = true; finishTick?.({ performed: false }); finishClose?.(); }, supplyFactory: () => ({
    view: () => ({ status: "running", permitId: "stop-permit", expiresAtMs: Date.now() + 60000, acquiredQuantity: 0 }),
    tick: () => { order.push("tick-start"); return new Promise(resolveTick => { finishTick = result => { order.push("tick-end"); finishTick = null; resolveTick(result); }; }); },
    cancel() {}, close: () => { order.push("supply-close-start"); if (cleanupRequested) return Promise.resolve(); return new Promise(resolveClose => { finishClose = () => { order.push("supply-close-end"); finishClose = null; resolveClose(); }; }); },
  }) });
  const originalClose = f.client.close; f.client.close = async () => { order.push("client-close"); await originalClose(); };
  const store = f.actor.store, closeMemory = store.close.bind(store); store.close = () => { order.push("memory-close"); closeMemory(); };
  const running = f.actor.run({ durationMs: 100000 });
  assert.deepEqual(order, ["tick-start"]);
  const stopping = f.actor.stop({ save: false }), again = f.actor.stop({ save: false }); await settle();
  assert.deepEqual(order, ["tick-start"]); assert.equal(store.profile().policyVersion, 1);
  assert.equal((await f.actor.companion.tick()).reason, "controller-closed");
  finishTick({ performed: false }); await settle();
  assert.deepEqual(order, ["tick-start", "tick-end", "supply-close-start"]);
  assert.equal(store.profile().policyVersion, 1); finishClose();
  await stopping; await again; await running;
  assert.deepEqual(order, ["tick-start", "tick-end", "supply-close-start", "supply-close-end", "client-close", "memory-close"]);
  assert.equal(await f.actor.stop(), false);
});

test("the fixed resident guard rejects a second instance and can be released", async () => {
  const guardPort = await port();
  const release = await acquireResidentGuard(guardPort);
  try { await assert.rejects(acquireResidentGuard(guardPort), { code: "EADDRINUSE" }); }
  finally { await release(); }
  await (await acquireResidentGuard(guardPort))();
});

test("existing character binding does not silently create another account or character", async () => {
  const client = fakeClient();
  const actor = new ResidentActor({ characterName: "AchuanAI", guardPort: await port() }, { client });
  await actor.start({ accountId: "test-account", password: "private-password" });
  await actor.stop();
  assert.deepEqual(client.calls, ["connect", "login", "startGame", "logout", "close"]);
});

test("a mismatched class closes and releases the guard without starting the wrong character", async () => {
  const client = fakeClient({ characterClass: "Wizard" });
  const guardPort = await port();
  const actor = new ResidentActor({ characterName: "AchuanAI", guardPort }, { client });
  await assert.rejects(actor.start({ accountId: "test-account", password: "private-password" }), /Warrior/);
  assert.equal(client.calls.includes("startGame"), false);
  assert.equal(client.calls.at(-1), "close");
  await (await acquireResidentGuard(guardPort))();
});

test("diagnostic exploration never moves with unknown or depleted health", async () => {
  for (const hp of [null, 0, 4]) {
    const client = fakeClient({ hp });
    const abort = new AbortController();
    const actor = new ResidentActor({}, {
      client,
      report: (event) => { if (event.type === "residentStatus") abort.abort(); },
    });
    await actor.run({ durationMs: 1_000, explore: true, signal: abort.signal });
    assert.equal(client.calls.includes("walk"), false);
  }
});

test("saved status is emitted only after the actual logout operation resolves", async () => {
  const client = fakeClient();
  const events = [];
  let confirm;
  client.logout = () => new Promise((resolve) => { confirm = resolve; });
  const actor = new ResidentActor({ characterName: "AchuanAI", guardPort: await port() }, { client, report: (event) => events.push(event) });
  await actor.start({ accountId: "test-account", password: "private-password" });
  const stopping = actor.stop();
  assert.equal(events.some((event) => event.saved === true), false);
  confirm(true);
  assert.equal(await stopping, true);
  assert.equal(events.at(-1).saved, true);
  assert.equal(await actor.stop(), false);
});
