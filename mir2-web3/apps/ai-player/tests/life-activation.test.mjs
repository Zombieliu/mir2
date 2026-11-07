import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { setImmediate as settle } from "node:timers/promises";
import { LifeController } from "../src/life/life-controller.mjs";
import { fixture, grant, potion } from "./life-fixture.mjs";

test("pending activation cannot attribute earlier survival kills or cross a new connection", async t => {
  const f = fixture(t, { requestPlan: async () => { throw new Error("No pre-activation model call"); } });
  f.controller.authorize(grant(), { awaitActivation: true });
  f.client.state.entities.push({ objectId: 81, kind: "monster", name: "Scarecrow", x: 288, y: 618, hp: 10, dead: false });
  f.controller.observe({ packet: "ObjectStruck", payload: { attackerId: f.client.state.selfZoneObjectId, objectId: 81 } });
  f.controller.observe({ packet: "ObjectDied", payload: { objectId: 81 } });
  assert.equal(f.controller.view().killsObserved, 0); assert.equal(f.facts.length, 0);
  f.client.state.connectionEpoch++;
  f.controller.confirmActivation(grant().lifePermitId);
  assert.equal(f.controller.view().activationPending, true); assert.equal(f.controller.view().modelDecisionsUsed, 0);
});

test("a model rest is finite and a route choice ends at its chosen waypoint", async t => {
  for (const mode of ["rest", "route"]) {
    const f = fixture(t, { requestPlan: async () => ({ mode, waypointIndex: 0, reply: "按有限阶段行动。" }) });
    f.controller.authorize(grant());
    await settle();
    if (mode === "rest") { assert.equal(f.decide().reason, "life-model-requested-rest"); f.client.time = 30_001; }
    else { Object.assign(f.client.state.player, grant().waypoints[0]); }
    assert.equal(f.decide().reason, "life-route-waypoint");
    f.client.state.entities.push({ objectId: 81, kind: "monster", name: "Scarecrow", x: f.client.state.player.x, y: f.client.state.player.y + 2, hp: 10, dead: false });
    assert.equal(f.decide().reason, "life-hunt-observed-monster");
  }
});

function activationFixture(t) {
  const directory = mkdtempSync(join(tmpdir(), "mir2-life-activation-test-")), closers = [], plans = [];
  const requestPlan = async request => { plans.push(structuredClone(request)); return { mode: "route", waypointIndex: 1, reply: "沿固定路线观察。" }; };
  const f = fixture({ after: callback => closers.push(callback) }, { path: join(directory, "life.sqlite"), requestPlan });
  let current = f.controller;
  t.after(async () => {
    for (const close of closers) await close();
    const absolute = resolve(directory);
    assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("mir2-life-activation-test-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  return { ...f, plans, get controller() { return current; },
    decide() { return current.decide({ state: f.client.snapshot(), policy: f.store.profile(), baseAction: { kind: "wait", reason: "no-party-order" }, nowMs: f.client.time }); },
    async restart() {
      await current.close();
      current = new LifeController({ client: f.client, dbPath: f.path, supplyController: f.supply, store: f.store,
        clock: () => f.client.time, requestPlan, onObserved: event => f.facts.push(event) });
      const reopened = current; closers.push(() => reopened.close());
    },
  };
}

test("real life authorization awaiting server activation performs no actions, claims or model attempts", async t => {
  const f = activationFixture(t); f.client.state.inventoryItems = [potion(1)];
  const accepted = f.controller.authorize(grant(), { awaitActivation: true });
  assert.equal(accepted.status, "running"); assert.equal(accepted.phase, "awaiting-server-activation");
  for (let i = 0; i < 6; i++) {
    assert.equal(f.decide().kind, "wait"); assert.equal(f.decide().reason, "life-awaiting-server-activation");
    assert.equal((await f.controller.tick()).performed, false);
  }
  await settle();
  assert.equal(f.client.requests.length, 0); assert.equal(f.supply.authorized.length, 0);
  assert.equal(f.plans.length, 0); assert.equal(f.controller.view().modelDecisionsUsed, 0);
  assert.equal(f.controller.view().childSupplyClaimed, false);
});

test("a wrong activation ID keeps the actual life pending without consuming its model quota", async t => {
  const f = activationFixture(t); f.controller.authorize(grant(), { awaitActivation: true });
  for (const id of ["life:foreign", "", undefined]) f.controller.confirmActivation(id);
  await settle();
  assert.equal(f.controller.view().activationPending, true); assert.equal(f.decide().kind, "wait");
  assert.equal(f.controller.view().modelDecisionsUsed, 0); assert.equal(f.plans.length, 0); assert.equal(f.client.requests.length, 0);
});

test("one matching server confirmation schedules exactly one real model-planning attempt", async t => {
  const f = activationFixture(t); f.controller.authorize(grant(), { awaitActivation: true });
  f.controller.confirmActivation(grant().lifePermitId);
  assert.equal(f.controller.view().activationPending, false); assert.equal(f.controller.view().modelDecisionsUsed, 1);
  for (let i = 0; i < 10; i++) f.controller.confirmActivation(grant().lifePermitId);
  await settle();
  assert.deepEqual(f.plans.map(request => request.reason), ["authorized"]);
  assert.equal(f.controller.view().decisionMode, "model");
  assert.equal(f.decide().reason, "life-route-waypoint");
  await f.controller.tick();
  assert.equal(f.plans.length, 1); assert.equal(f.controller.view().modelDecisionsUsed, 1); assert.equal(f.client.requests.length, 0);
});

test("cancelling a pending life fences every later activation and model attempt", async t => {
  const f = activationFixture(t); f.controller.authorize(grant(), { awaitActivation: true });
  f.controller.cancel("owner-revoked");
  f.controller.confirmActivation(grant().lifePermitId); await settle();
  assert.equal(f.controller.view().status, "returning"); assert.equal(f.plans.length, 0);
  assert.equal(f.decide().reason, "life-session-finished");
  assert.equal(f.controller.view().status, "cancelled");
  f.controller.confirmActivation(grant().lifePermitId); await f.controller.tick();
  assert.equal(f.plans.length, 0); assert.equal(f.controller.view().modelDecisionsUsed, 0); assert.equal(f.client.requests.length, 0);
});

test("restart pauses a pending real life and an old activation cannot revive it", async t => {
  const f = activationFixture(t); f.controller.authorize(grant(), { awaitActivation: true }); await settle();
  await f.restart(); assert.equal(f.controller.view().status, "paused");
  f.controller.confirmActivation(grant().lifePermitId); await settle();
  assert.equal(f.controller.view().status, "paused"); assert.equal(f.controller.view().modelDecisionsUsed, 0);
  assert.equal(f.decide().kind, "wait"); await f.controller.tick();
  assert.equal(f.plans.length, 0); assert.equal(f.client.requests.length, 0);
});

test("resuming the same grant waits for fresh confirmation and preserves committed spending and its one child slot", async t => {
  const f = activationFixture(t); f.controller.authorize(grant(), { awaitActivation: true });
  f.controller.confirmActivation(grant().lifePermitId); await settle();
  f.client.state.inventoryItems = [potion(1)];
  assert.equal(f.decide().reason, "life-claim-one-supply"); await f.controller.tick();
  assert.equal(f.client.requests.length, 1); assert.equal(f.supply.authorized.length, 1);
  f.supply.current = { ...f.supply.current, status: "completed", receiptKnown: true, acquiredQuantity: 1, chargedGold: 40, returnedToAnchor: true };
  f.client.state.inventoryItems = [potion(2)]; f.client.state.gold = 45; f.decide(); await settle();
  assert.equal(f.controller.view().chargedGold, 40); assert.equal(f.controller.view().purchasedQuantity, 1);
  const usedBefore = f.controller.view().modelDecisionsUsed, plansBefore = f.plans.length;
  await f.restart(); assert.equal(f.controller.view().status, "paused");
  const resumed = f.controller.resume(grant(), { awaitActivation: true });
  assert.equal(resumed.activationPending, true); assert.equal(resumed.expiresAtMs, grant().expiresAtMs);
  assert.equal(resumed.chargedGold, 40); assert.equal(resumed.remainingBudgetGold, 10);
  assert.equal(resumed.purchasedQuantity, 1); assert.equal(resumed.childSupplyClaimed, true); assert.equal(resumed.childSupplyPermitId, "supply:life");
  assert.equal(f.decide().reason, "life-awaiting-server-activation"); await f.controller.tick(); await settle();
  assert.equal(f.plans.length, plansBefore); assert.equal(f.controller.view().modelDecisionsUsed, usedBefore); assert.equal(f.client.requests.length, 1);
  f.controller.confirmActivation(grant().lifePermitId); await settle();
  assert.equal(f.controller.view().activationPending, false); assert.equal(f.controller.view().chargedGold, 40);
  assert.equal(f.controller.view().remainingBudgetGold, 10); assert.equal(f.controller.view().childSupplyClaimed, true);
  assert.equal(f.controller.view().expiresAtMs, grant().expiresAtMs); assert.equal(f.client.requests.length, 1);
  f.controller.confirmActivation(grant().lifePermitId); await settle();
  assert.equal(f.plans.filter(request => request.reason === "resumed").length, 1);
});

test("expired or policy-changed activation cannot spend a planning attempt", async t => {
  for (const change of [f => { f.client.time = grant().expiresAtMs; }, f => { f.policy.policyVersion = 2; }]) {
    const f = activationFixture(t); f.controller.authorize(grant(), { awaitActivation: true }); change(f);
    f.controller.confirmActivation(grant().lifePermitId); await settle();
    assert.equal(f.controller.view().activationPending, true); assert.equal(f.plans.length, 0);
    assert.equal(f.controller.view().modelDecisionsUsed, 0); assert.equal(f.client.requests.length, 0);
  }
});
