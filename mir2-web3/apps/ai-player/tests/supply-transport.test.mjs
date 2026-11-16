import assert from "node:assert/strict";
import test from "node:test";
import { GatewayClient } from "../src/gateway-client.mjs";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";
import { intent, operation, producer, receipt, ownerFrame, world, shop, goods } from "./supply-fixture.mjs";

function fixture({ handle, timeoutMs = 150, beginProducer = producer() } = {}) {
  let socket;
  class Socket extends EventTarget {
    readyState = 1; commands = [];
    constructor() { super(); socket = this; queueMicrotask(() => { this.event({ type: "realmInfo", payload: { realmId: "world:test", worldEpoch: "epoch:test", features: { companionControl: true, sharedParty: true } } }); this.packet("Connected", {}); }); }
    event(value) { this.raw(stringifyGatewayJson(value)); }
    raw(raw) { this.dispatchEvent(new MessageEvent("message", { data: raw })); }
    packet(packet, payload) { this.event({ type: "packet", packet, payload }); }
    send(raw) {
      const command = parseGatewayJson(raw); this.commands.push(command);
      queueMicrotask(() => {
        if (command.type === "clientVersion") this.packet("ClientVersion", { result: 1 });
        else if (command.type === "login") this.packet("LoginSuccess", { characters: [{ name: "Alo", index: 2, class: "Warrior" }] });
        else if (command.type === "startGame") { this.packet("StartGame", { result: 4, resolution: 1024 }); this.event({ type: "worldSnapshot", payload: world() }); }
        else if (command.type === "clientCapabilities") {}
        else if (command.type === "npcPurchaseOwner" && command.action.kind === "begin") this.event(ownerFrame(command, { kind: "producer", producer: beginProducer }, world(), beginProducer));
        else handle?.(this, command);
      });
    }
    close() { this.readyState = 3; this.dispatchEvent(new Event("close")); }
  }
  const client = new GatewayClient({ url: "ws://127.0.0.1:7010/ws", realmId: "world:test", timeoutMs, WebSocketImpl: Socket });
  return { client, get socket() { return socket; }, async enter() { await client.connect(); await client.login("test", "test-only"); await client.startGame(2); } };
}

test("ordinary observed NPC dialogue and typed owner quote retain the exact goods selector", async t => {
  const f = fixture({ handle(socket, command) {
    if (command.type === "interact") socket.event({ type: "worldSnapshot", payload: world() });
    if (command.type === "selectNpcDialog") socket.packet("NPCGoods", { panelType: 0, rate: 1, list: [goods({ uniqueId: 18446744073709551615n, purchaseItemIndex: "18446744073709551615" })] });
    if (command.action?.kind === "quote") socket.event(ownerFrame(command, { kind: "quote", intent: intent({ request: command.action.request }) }));
  } });
  t.after(() => f.client.close()); await f.enter();
  assert.equal(f.client.activeCharacterIndex, 2);
  await f.client.interactNpc(8); const observed = await f.client.selectNpcBuyLink();
  assert.equal(observed.list[0].purchaseItemIndex, "18446744073709551615");
  await f.client.beginNpcPurchase();
  const result = await f.client.quoteNpcPurchase({ itemIndex: "18446744073709551615", count: 1, panelType: 0 });
  assert.equal(result.reply.intent.request.itemIndex, "18446744073709551615");
  assert.equal(f.socket.commands.some(c => c.type === "buyItem"), false);
  assert.deepEqual(f.socket.commands.find(c => c.type === "clientCapabilities").capabilities, ["durableNpcPurchaseOwnerV1"]);
  await assert.rejects(f.client.interactNpc(1234), /observed|Observe/);
});

test("ordinary deltas precede the correlated full owner snapshot without double charging or losing the Zone alias", async t => {
  const f = fixture({ handle(socket, command) {
    if (command.action?.kind !== "purchase") return;
    const op = command.action.operation;
    socket.packet("LoseGold", { gold: 40 });
    socket.packet("GainedItem", { item: { uniqueId: 9007199254740993n, count: 1 } });
    const owned = world({ gold: 160, inventoryItems: [{ key: "hp", uniqueId: 9007199254740993n, name: "(HP)DrugSmall", quantity: 1, slot: 0, container: "bag1" }] });
    delete owned.selfZoneObjectId;
    socket.event(ownerFrame(command, { kind: "purchase", receipt: receipt(op), replayed: false }, owned, producer({ serverRevision: "2" })));
  } });
  t.after(() => f.client.close()); await f.enter(); await f.client.beginNpcPurchase();
  const result = await f.client.purchaseNpcItem(operation());
  assert.equal(result.reply.receipt.entry.outcome.committed.charged, 40);
  assert.equal(f.client.snapshot().gold, 160);
  assert.equal(f.client.snapshot().selfZoneObjectId, 50001);
  assert.equal(f.client.snapshot().inventoryStale, false);
  assert.equal(f.client.snapshot().inventoryItems[0].uniqueId, 9007199254740993n);
  assert.equal(f.client.snapshot().nativeNpcShop.scriptKey, shop().scriptKey);
  const before = f.client.snapshot();
  f.socket.event(ownerFrame({ requestId: "999" }, { kind: "producer", producer: producer() }, world({ gold: 999 }), producer()));
  assert.deepEqual(f.client.snapshot(), before, "unsolicited and late snapshots have no applied authority");
});

test("known receipt without the complete pair does not refresh economic observation", async t => {
  const f = fixture({ handle(socket, command) {
    if (command.action?.kind === "purchase") {
      socket.packet("LoseGold", { gold: 40 });
      socket.packet("GainedItem", {});
      socket.event(ownerFrame(command, { kind: "purchase", receipt: receipt(command.action.operation), replayed: false }, null, null));
    }
  } });
  t.after(() => f.client.close()); await f.enter(); await f.client.beginNpcPurchase();
  const result = await f.client.purchaseNpcItem(operation());
  assert.equal(result.snapshot, null);
  assert.equal(f.client.snapshot().inventoryStale, true);
  assert.equal(f.client.snapshot().npcPurchaseAuthority?.serverRevision, "1", "receipt did not invent a newer complete revision");
});

test("an invalid correlated receipt closes safely and cannot be applied", async t => {
  const f = fixture({ handle(socket, command) {
    if (command.action?.kind === "purchase") socket.event(ownerFrame(command, { kind: "purchase", receipt: receipt(operation({ sequence: "2" })), replayed: false }, world({ gold: 1 }), producer({ serverRevision: "2" })));
  } });
  t.after(() => f.client.close()); await f.enter(); await f.client.beginNpcPurchase();
  await assert.rejects(f.client.purchaseNpcItem(operation()), /validation|closed/);
  assert.equal(f.client.snapshot().ready, false);
  assert.equal(f.socket.commands.filter(c => c.action?.kind === "purchase").length, 1);
});

test("purchase timeout and overlapping ordinary action never create another purchase", async t => {
  const f = fixture({ timeoutMs: 35 }); t.after(() => f.client.close()); await f.enter(); await f.client.beginNpcPurchase();
  const pending = f.client.purchaseNpcItem(operation());
  await assert.rejects(f.client.walk("Up"), /current player action/);
  await assert.rejects(pending, e => e.code === "NPC_PURCHASE_UNKNOWN");
  assert.equal(f.socket.commands.filter(c => c.action?.kind === "purchase").length, 1);
  assert.equal(f.socket.commands.some(c => c.type === "buyItem"), false);
});

test("full owner snapshot for a different actor or map fails closed", async t => {
  for (const change of [s => s.playerObjectId = 13, s => s.mapFileName = "0141", s => s.entities[0].name = "Other"]) {
    const f = fixture({ handle(socket, command) { if (command.action?.kind === "quote") { const state = world(); change(state); socket.event(ownerFrame(command, { kind: "quote", intent: intent({ request: command.action.request }) }, state)); } } });
    await f.enter(); await f.client.beginNpcPurchase();
    await assert.rejects(f.client.quoteNpcPurchase(operation().intent.request), /validation|closed/);
    assert.equal(f.client.snapshot().ready, false); await f.client.close();
  }
});

test("new connection producer may query the original retired scope, while a regressed authority cannot replace state", async t => {
  const current = producer({ producerScope: "44".repeat(32), serverRevision: "3" });
  const f = fixture({ beginProducer: current, handle(socket, command) {
    if (command.action?.kind === "query") socket.event(ownerFrame(command, { kind: "recovery", receipt: { ...receipt(command.action.operation), producerScope: current.producerScope } }, world({ gold: 160 }), current));
    if (command.action?.kind === "quote") socket.event(ownerFrame(command, { kind: "quote", intent: intent({ request: command.action.request }) }, world(), { ...current, serverRevision: "2" }));
  } });
  t.after(() => f.client.close()); await f.enter(); await f.client.beginNpcPurchase();
  const recovered = await f.client.queryNpcPurchase(operation());
  assert.equal(recovered.reply.receipt.entry.operation.requestScope, operation().requestScope);
  assert.equal(recovered.authority.producerScope, current.producerScope);
  await assert.rejects(f.client.purchaseNpcItem(operation()), /retired producer/);
  await assert.rejects(f.client.quoteNpcPurchase(operation().intent.request), /validation|closed/);
  assert.equal(f.client.snapshot().ready, false);
});
