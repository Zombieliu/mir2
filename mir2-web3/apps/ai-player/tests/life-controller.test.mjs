import assert from "node:assert/strict";
import test from "node:test";
import { LifeController, validateLifeGrant } from "../src/life/life-controller.mjs";
import { fixture, grant, potion, Client, childGrant } from "./life-fixture.mjs";

test("only the exact bounded current Bichon life grant is executable", t => {
  const f = fixture(t);
  for (const changes of [{ durationSeconds: 600 }, { expiresAtMs: 1_800_000 }, { budgetGold: 51 }, { maxPurchasedQuantity: 2 }, { mapFileName: "0141" }, { worldEpoch: "other" }, { actorCharacterIndex: 3 }, { home: { x: 289, y: 616 } }, { bounds: { ...grant().bounds, maxY: 646 } }, { waypoints: [{ x: 271, y: 596 }] }]) assert.throws(() => f.controller.authorize(grant(changes)));
  assert.throws(() => validateLifeGrant({ ...grant(), sellEquipment: true }));
  assert.equal(f.controller.authorize(grant()).status, "running"); assert.equal(f.client.requests.length, 0);
});

test("a healthy independent resident patrols and hunts only observed allowed monsters within its fixed bounds", t => {
  const f = fixture(t); f.controller.authorize(grant());
  assert.equal(f.decide().reason, "life-route-waypoint");
  const monster = { objectId: 81, kind: "monster", name: "Scarecrow", x: 288, y: 618, hp: 10, dead: false, disposition: "hostile" };
  f.client.state.entities.push(monster);
  assert.equal(f.decide().reason, "life-hunt-observed-monster"); assert.equal(f.decide().targetId, 81);
  for (const change of [() => { monster.name = "Boss"; }, () => { monster.name = "Scarecrow"; monster.x = 311; }, () => { monster.x = 288; monster.kind = "player"; }]) {
    change(); assert.equal(f.decide().kind, "follow");
  }
  assert.equal(f.client.snapshot().party.members.length, 0); assert.equal(f.client.requests.length, 0);
});

test("no medicine, low health and full capacity return home before any purchase; unavailable resources are not zeros", t => {
  for (const update of [{ inventoryItems: [potion(1)] }, { playerHp: 12 }, { freeBagSlots: 0 }]) {
    const f = fixture(t); f.controller.authorize(grant()); Object.assign(f.client.state, update); f.client.state.player.y = 628; f.client.state.inSafeZone = false;
    assert.equal(f.decide().reason, "life-return-home"); assert.deepEqual(f.decide().target, grant().home);
    assert.equal(f.client.requests.length, 0);
  }
  const f = fixture(t); f.controller.authorize(grant()); f.client.state.inventoryStale = true;
  assert.equal(f.decide().reason, "life-resource-or-economic-result-unknown"); assert.equal(f.controller.view().resources.hpPotionCount, null);
});

test("at home a healthy life claims one stable child request then resumes only from actual supply completion", async t => {
  const f = fixture(t); f.client.state.inventoryItems = [potion(1)]; f.controller.authorize(grant());
  assert.equal(f.decide().kind, "life"); await f.controller.tick();
  assert.equal(f.client.requests.length, 1); assert.equal(f.supply.authorized.length, 1); assert.equal(f.controller.view().childSupplyClaimed, true);
  await f.controller.tick(); assert.equal(f.client.requests.length, 1);
  assert.equal(f.decide().reason, "life-child-supply-active");
  Object.assign(f.supply.current, { status: "completed", receiptKnown: true, acquiredQuantity: 1, chargedGold: 40 });
  f.client.state.inventoryItems[0].quantity = 2;
  assert.equal(f.decide().reason, "life-route-waypoint");
  assert.equal(f.controller.view().chargedGold, 40); assert.equal(f.controller.view().remainingBudgetGold, 10);
  f.client.state.inventoryItems[0].quantity = 1;
  assert.equal(f.decide().reason, "life-await-owner-supply"); await f.controller.tick(); assert.equal(f.client.requests.length, 1);
});

test("insufficient gold, a smaller budget, full bag and unsafe low health wait at home without spending", async t => {
  for (const [changes, budgetGold] of [[{ gold: 39 }, 50], [{}, 39], [{ freeBagSlots: 0 }, 50], [{ playerHp: 12 }, 50]]) {
    const f = fixture(t); Object.assign(f.client.state, changes, { inventoryItems: [potion(1)] }); f.controller.authorize(grant({ budgetGold }));
    assert.equal(f.decide().kind, "wait"); await f.controller.tick(); assert.equal(f.client.requests.length, 0);
  }
});

test("an unknown child request stays claimed across restart and resume without refreshing its deadline or quota", async t => {
  const f = fixture(t); f.client.state.inventoryItems = [potion(1)];
  f.client.requestLifeSupply = async request => { f.client.requests.push(request); throw new Error("synthetic lost reply"); };
  f.controller.authorize(grant()); f.decide(); await f.controller.tick(); assert.equal(f.client.requests.length, 1);
  await f.controller.close();
  const next = new LifeController({ client: f.client, dbPath: f.path, supplyController: f.supply, store: f.store, clock: () => f.client.time }); t.after(() => next.close());
  assert.equal(next.view().status, "paused"); assert.equal(next.view().childSupplyClaimed, true);
  next.authorize(grant()); assert.equal(next.view().status, "paused");
  assert.throws(() => next.resume(grant({ lifePermitId: "life:other" })));
  next.resume(grant()); next.decide({ state: f.client.snapshot(), policy: f.store.profile(), baseAction: { kind: "wait" }, nowMs: 1000 }); await next.tick();
  assert.equal(f.client.requests.length, 1); assert.equal(next.view().expiresAtMs, 1_200_000); assert.equal(next.view().step, "awaiting-owner");
});

test("an attempted unknown purchase conservatively reserves the full budget instead of claiming it unspent", async t => {
  const f = fixture(t); f.client.state.inventoryItems = [potion(1)]; f.controller.authorize(grant()); f.decide(); await f.controller.tick();
  f.supply.current.status = "unknown";
  assert.equal(f.controller.view().reservedGold, 50); assert.equal(f.controller.view().remainingBudgetGold, 0);
  assert.equal(f.decide().reason, "life-child-supply-active"); assert.equal(f.client.requests.length, 1);
});

test("expiry interrupts hunting and spending, records completion only at the observed safe home, then waits", t => {
  const f = fixture(t); f.controller.authorize(grant()); f.client.state.player.y = 628; f.client.state.inSafeZone = false;
  f.client.time = 1_200_000;
  assert.equal(f.decide().reason, "life-return-home"); assert.equal(f.controller.view().status, "returning"); assert.equal(f.facts.some(e => e.kind === "life-completed"), false);
  f.client.state.player.y = 616; f.client.state.inSafeZone = true;
  assert.equal(f.decide().reason, "life-session-finished"); assert.equal(f.controller.view().status, "completed");
  assert.equal(f.facts.filter(e => e.kind === "life-returned").length, 1); assert.equal(f.facts.filter(e => e.kind === "life-completed").length, 1);
  f.decide(); assert.equal(f.facts.filter(e => e.kind === "life-completed").length, 1); assert.equal(f.client.requests.length, 0);
});

test("cancellation during the awaited child claim cannot start a purchase or revive its single slot", async t => {
  const f = fixture(t); f.client.state.inventoryItems = [potion(1)]; let release;
  f.client.requestLifeSupply = async request => { f.client.requests.push(request); return await new Promise(resolve => { release = resolve; }); };
  f.controller.authorize(grant()); f.decide(); const pending = f.controller.tick(); f.controller.cancel(); release({ grant: childGrant() }); await pending;
  assert.equal(f.supply.authorized.length, 0); assert.equal(f.controller.view().childSupplyClaimed, true); assert.equal(f.client.requests.length, 1);
  f.decide(); await f.controller.tick(); assert.equal(f.client.requests.length, 1);
});

test("state, policy, expiry and medicine changes during an awaited grant cannot authorize economic execution", async t => {
  for (const change of [f => f.client.state.connectionEpoch++, f => f.policy.policyVersion++, f => f.client.time = 1_200_000, f => { f.client.state.inventoryStale = true; }, f => { f.client.state.trade = { active: true, uncertain: true }; }]) {
    const f = fixture(t); f.client.state.inventoryItems = [potion(1)]; let release;
    f.client.requestLifeSupply = () => new Promise(resolve => { release = resolve; });
    f.controller.authorize(grant()); f.decide(); const pending = f.controller.tick(); change(f); release({ grant: childGrant() }); await pending;
    assert.equal(f.supply.authorized.length, 0); assert.equal(f.controller.view().childSupplyClaimed, true);
  }
});

test("a child grant cannot change its parent actor, budget, return point, quantity or lifetime", async t => {
  for (const change of [{ worldEpoch: "another-world" }, { actorCharacterIndex: 3 }, { maxGold: 51 }, { quantity: 2 }, { itemIndex: 659 },
    { returnAnchor: { x: 289, y: 616 } }, { expiresAtMs: 1_200_001 }]) {
    const f = fixture(t); f.client.state.inventoryItems = [potion(1)];
    f.client.requestLifeSupply = async request => { f.client.requests.push(request); return { grant: childGrant(change) }; };
    f.controller.authorize(grant()); f.decide(); await f.controller.tick();
    assert.equal(f.controller.view().childSupplyClaimed, true); assert.equal(f.supply.authorized.length, 0); assert.equal(f.client.requests.length, 1);
    await f.controller.tick(); assert.equal(f.client.requests.length, 1);
  }
});

test("only a real own hit followed by a fresh death event becomes a durable kill fact", async t => {
  const f = fixture(t); f.controller.authorize(grant()); f.client.state.entities.push({ objectId: 80, kind: "monster", name: "Deer", x: 288, y: 617, hp: 10 });
  for (const event of [{ packet: "ObjectRemove", payload: { objectId: 80 } }, { packet: "ObjectDied", payload: { objectId: 80 } }, { packet: "ObjectStruck", payload: { objectId: 80 } }, { packet: "ObjectDied", payload: { objectId: 80 } }]) f.controller.observe(event);
  assert.equal(f.controller.view().killsObserved, 0);
  f.controller.observe({ packet: "ObjectStruck", payload: { attackerId: 50001, objectId: 80 } });
  f.controller.observe({ packet: "ObjectDied", payload: { objectId: 80 } }); f.controller.observe({ packet: "ObjectDied", payload: { objectId: 80 } });
  assert.equal(f.controller.view().killsObserved, 1); assert.equal(f.facts.filter(e => e.kind === "life-kill").length, 1);
  assert.equal(f.facts[0].sourceEventRef, "observation:1:1");
  await f.controller.close(); const next = new LifeController({ client: f.client, dbPath: f.path, supplyController: f.supply, store: f.store, clock: () => f.client.time }); t.after(() => next.close());
  assert.equal(next.view().killsObserved, 1); assert.equal(next.view().status, "paused");
});

test("a current group can retain assistance and loot only after combat while its owner is outside view", t => {
  const f = fixture(t); f.controller.authorize(grant()); f.client.state.party.members = ["Owner", "Alo"];
  f.client.state.entities.push({ objectId: 80, kind: "monster", name: "Scarecrow", x: 288, y: 617, hp: 10 });
  f.client.state.engagements = [{ attackerId: 50001, targetId: 80, observedAt: 0 }];
  f.client.state.groundDrops = [{ objectId: 90, x: 288, y: 616, ownerObjectId: 50001, ownershipRemainingTicks: 10 }];
  const assist = { kind: "attack", reason: "assist-confirmed-party-engagement", policyVersion: 1, targetId: 80, target: { x: 288, y: 617 } };
  assert.equal(f.decide(assist).reason, "assist-confirmed-party-engagement"); assert.notEqual(f.decide().kind, "loot");
  f.client.state.engagements = []; f.client.state.entities = f.client.state.entities.filter(e => e.objectId !== 80);
  assert.equal(f.decide().reason, "life-after-combat-loot");
  f.client.state.groundDrops[0].ownerObjectId = 999; delete f.client.state.groundDrops[0].ownershipRemainingTicks;
  assert.notEqual(f.decide().kind, "loot");
});

test("an already selected live hunting target prevents loot between attack intent and the first hit event", t => {
  const f = fixture(t); f.controller.authorize(grant());
  const target = { objectId: 80, kind: "monster", name: "Deer", x: 288, y: 617, hp: 10 }; f.client.state.entities.push(target);
  assert.equal(f.decide().reason, "life-hunt-observed-monster");
  f.client.state.groundDrops = [{ objectId: 90, x: 288, y: 616, ownerObjectId: 50001 }];
  assert.equal(f.decide().kind, "attack");
  target.dead = true; assert.equal(f.decide().reason, "life-after-combat-loot");
});

test("models select only the finite route vocabulary once per phase and remain fenced by world and four-call quota", async t => {
  const plans = [], f = fixture(t, { requestPlan: async value => { plans.push(value.reason); return { mode: "route", waypointIndex: 2, reply: "沿第三个路点观察。", rememberedEpisodeIds: [] }; } });
  f.controller.authorize(grant()); await new Promise(resolve => setImmediate(resolve));
  assert.equal(f.controller.view().decisionMode, "model"); assert.equal(f.controller.view().waypointIndex, 2);
  for (let n = 0; n < 100; n++) f.decide();
  assert.deepEqual(plans, ["authorized"]); assert.equal(f.controller.view().modelDecisionsUsed, 1);
  f.controller.resume(grant()); await new Promise(resolve => setImmediate(resolve));
  f.controller.resume(grant()); await new Promise(resolve => setImmediate(resolve));
  assert.equal(plans.filter(v => v === "resumed").length, 1);
  const bad = fixture(t, { requestPlan: async () => ({ mode: "trade", maxGold: 999 }) }); bad.controller.authorize(grant()); await new Promise(resolve => setImmediate(resolve));
  assert.equal(bad.controller.view().decisionMode, "rules"); assert.equal(bad.client.requests.length, 0);
});

test("the four phase-planning attempts survive restart and cannot become a per-tick model loop", async t => {
  const plans = [], f = fixture(t, { requestPlan: async request => { plans.push(request.reason); return { mode: "hunt" }; } });
  const settle = () => new Promise(resolve => setImmediate(resolve));
  f.controller.authorize(grant()); await settle();
  f.controller.resume(grant()); await settle(); f.decide(); await settle();
  for (let cycle = 0; cycle < 6; cycle++) {
    f.client.state.player.y = 628; f.client.state.inSafeZone = false; f.client.state.playerHp = 14; f.decide();
    f.client.state.player.y = 616; f.client.state.inSafeZone = true; f.decide();
    f.client.state.playerHp = 18; f.decide(); await settle();
  }
  assert.equal(plans.length, 4); assert.equal(f.controller.view().modelDecisionsUsed, 4);
  for (let n = 0; n < 100; n++) f.decide(); await settle(); assert.equal(plans.length, 4);
  await f.controller.close();
  const next = new LifeController({ client: f.client, dbPath: f.path, supplyController: f.supply, store: f.store, clock: () => f.client.time,
    requestPlan: async () => { plans.push("unexpected-after-restart"); return { mode: "hunt" }; } }); t.after(() => next.close());
  next.resume(grant()); await settle(); assert.equal(plans.length, 4); assert.equal(next.view().modelDecisionsUsed, 4);
});

test("cancel before the scheduled planning microtask or change world during planning cannot issue or apply a late model decision", async t => {
  let calls = 0;
  const cancelled = fixture(t, { requestPlan: async () => { calls++; return { mode: "rest" }; } });
  cancelled.controller.authorize(grant()); cancelled.controller.cancel(); await new Promise(resolve => setImmediate(resolve)); assert.equal(calls, 0);
  let release;
  const moved = fixture(t, { requestPlan: () => new Promise(resolve => { release = resolve; }) }); moved.controller.authorize(grant());
  await new Promise(resolve => setImmediate(resolve)); moved.client.state.connectionEpoch++; release({ mode: "route", waypointIndex: 2 });
  await new Promise(resolve => setImmediate(resolve)); assert.notEqual(moved.controller.view().decisionMode, "model"); assert.equal(moved.controller.view().waypointIndex, 0);
});

test("observed memory delivery retries the same durable fact and blocks new life activity until it can be recorded", t => {
  let unavailable = true; const attemptedIds = [], delivered = [];
  const f = fixture(t, { onObserved: event => { attemptedIds.push(event.eventId); if (unavailable) throw new Error("synthetic memory failure"); delivered.push(event); } });
  f.controller.authorize(grant()); f.client.state.entities.push({ objectId: 80, kind: "monster", name: "Deer", x: 288, y: 617, hp: 10 });
  f.controller.observe({ packet: "ObjectStruck", payload: { attackerId: 12, objectId: 80 } }); f.controller.observe({ packet: "ObjectDied", payload: { objectId: 80 } });
  assert.equal(f.decide().reason, "life-memory-write-unavailable"); assert.equal(f.client.requests.length, 0);
  unavailable = false; f.decide(); assert.equal(delivered.length, 1); assert.equal(new Set(attemptedIds).size, 1);
  f.decide(); assert.equal(delivered.length, 1); assert.equal(f.controller.view().killsObserved, 1);
});

test("a 30-minute permit never extends its deadline when resources fluctuate or authorization is repeated", t => {
  const f = fixture(t), permission = grant({ durationSeconds: 1800, expiresAtMs: 1_800_000 }); f.controller.authorize(permission);
  f.client.time = 1_799_999; assert.equal(f.decide().reason, "life-route-waypoint"); f.controller.authorize(permission);
  assert.equal(f.controller.view().expiresAtMs, 1_800_000); f.client.time = 1_800_000;
  assert.equal(f.decide().reason, "life-session-finished"); assert.equal(f.controller.view().status, "completed");
  assert.equal(f.facts.some(event => event.kind === "life-returned"), false, "starting and ending at home does not invent an excursion or return trip");
});

test("close waits for an in-flight child request and never starts its grant after shutdown", async t => {
  const f = fixture(t); f.client.state.inventoryItems = [potion(1)]; let release;
  f.client.requestLifeSupply = () => new Promise(resolve => { release = resolve; });
  f.controller.authorize(grant()); f.decide(); const pending = f.controller.tick(); let done = false; const closing = f.controller.close().then(() => { done = true; });
  await Promise.resolve(); assert.equal(done, false); release({ grant: childGrant() }); await pending; await closing;
  assert.equal(f.supply.authorized.length, 0); assert.equal(done, true);
});
