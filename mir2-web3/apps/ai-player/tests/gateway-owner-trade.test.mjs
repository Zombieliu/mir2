import assert from "node:assert/strict";
import test from "node:test";
import { setImmediate as settle } from "node:timers/promises";
import { GatewayClient } from "../src/gateway-client.mjs";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";
import { world, tradeState } from "./gift-fixture.mjs";

function fixture() {
  let socket;
  const data = world({ gold: 85,
    entities: [{ kind: "selfPlayer", objectId: 1000, name: "DonorTest", x: 288, y: 616, direction: "Right", hp: 18 },
      { kind: "player", objectId: 50002, name: "AloReceiver", x: 289, y: 616, direction: "Left", hp: 18 }] });
  class Socket extends EventTarget {
    readyState = 1; commands = [];
    constructor() {
      super(); socket = this;
      queueMicrotask(() => {
        this.event({ type: "realmInfo", payload: { realmId: "owner-trade-test", worldEpoch: "owner-trade-epoch", features: { sharedParty: true, companionControl: true } } });
        this.packet("Connected", {});
      });
    }
    event(event) { this.dispatchEvent(new MessageEvent("message", { data: stringifyGatewayJson(event) })); }
    packet(packet, payload = {}) { this.event({ type: "packet", packet, payload }); }
    snapshot() { this.event({ type: "worldSnapshot", payload: data }); }
    send(raw) {
      const command = parseGatewayJson(raw); this.commands.push(command);
      queueMicrotask(() => {
        if (command.type === "clientVersion") this.packet("ClientVersion", { result: 1 });
        else if (command.type === "login") this.packet("LoginSuccess", { characters: [{ name: "DonorTest", index: 8 }] });
        else if (command.type === "startGame") { this.packet("StartGame", { result: 4, resolution: 1024 }); this.snapshot(); }
        else this.onCommand?.(command);
      });
    }
    close() { this.readyState = 3; this.dispatchEvent(new Event("close")); }
  }
  const client = new GatewayClient({ url: "ws://127.0.0.1:7010/ws", realmId: "owner-trade-test", timeoutMs: 50, WebSocketImpl: Socket });
  return { client, data, get socket() { return socket; },
    async enter() { await client.connect(); await client.login("synthetic-owner", "test-only"); await client.startGame(8); },
    open() { socket.packet("TradeAccept", { partnerName: "AloReceiver" }); data.stage5Systems.trade = tradeState({ partner: "AloReceiver" }); socket.snapshot(); },
    applyDelta(amount) {
      data.gold -= amount; data.stage5Systems.trade.offeredGold += amount; data.stage5Systems.trade.heldGold += amount;
      socket.packet("LoseGold", { gold: amount }); socket.snapshot();
    },
  };
}

test("player trade request uses the ordinary front-facing packet without a name or success receipt", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  assert.deepEqual(await f.client.requestPlayerTrade(), { sent: true, confirmed: false });
  assert.deepEqual(f.socket.commands.at(-1), { type: "tradeRequest" });
  assert.equal(f.client.snapshot().trade.active, false);
  f.data.entities[1].direction = "Up"; f.socket.snapshot();
  const before = f.socket.commands.length;
  await assert.rejects(f.client.requestPlayerTrade(), /mutually facing/);
  assert.equal(f.socket.commands.length, before);
});

test("a 50 game-gold owner offer is observed as real held custody and wallet debit, not final delivery", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter(); f.open();
  f.socket.onCommand = command => { assert.deepEqual(command, { type: "tradeGold", amount: 50 }); f.applyDelta(command.amount); };
  assert.deepEqual(await f.client.offerTradeGold(50), { sent: true, offerObserved: true, tradeId: "a".repeat(32), offeredGold: 50, walletGold: 35 });
  assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
  const count = f.socket.commands.length;
  await assert.rejects(f.client.offerTradeGold(40), /available wallet/); // Held 50 cannot fund a second delta.
  assert.equal(f.socket.commands.length, count);
});

test("ordinary gold amounts are additive deltas and unchanged snapshots cannot confirm an offer", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter(); f.open();
  f.socket.onCommand = command => {
    assert.equal(command.type, "tradeGold"); f.socket.snapshot();
    setImmediate(() => f.applyDelta(command.amount));
  };
  assert.equal((await f.client.offerTradeGold(7)).offeredGold, 7);
  assert.equal((await f.client.offerTradeGold(11)).offeredGold, 18);
  assert.equal(f.client.snapshot().gold, 67);
  assert.deepEqual(f.socket.commands.filter(command => command.type === "tradeGold").map(command => command.amount), [7, 11]);
});

test("invalid amounts, locked trades, missing snapshots and unknown trade states never send owner gold", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter(); f.open();
  const before = f.socket.commands.length;
  for (const amount of [0, -1, 1.5, "50", 86, 0x1_0000_0000, Number.MAX_SAFE_INTEGER + 1]) await assert.rejects(f.client.offerTradeGold(amount), /positive trade gold delta/);
  for (const patch of [{ locked: true }, { escrowPrepared: true }]) {
    f.data.stage5Systems.trade = tradeState({ partner: "AloReceiver", ...patch }); f.socket.snapshot();
    await assert.rejects(f.client.offerTradeGold(1), /unlocked player trade/);
    await assert.rejects(f.client.confirmPlayerTrade(), /unlocked player trade/);
  }
  f.data.stage5Systems.trade = tradeState({ partner: "AloReceiver" }); f.socket.snapshot();
  f.socket.event({ type: "worldSnapshot", payload: { ...f.data, stage5Systems: {} } });
  await assert.rejects(f.client.offerTradeGold(1), /unlocked player trade/);
  assert.equal(f.socket.commands.length, before);
});

test("owner confirmation reports observed preparation without pretending the gift was delivered", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter(); f.open();
  f.socket.onCommand = command => {
    assert.deepEqual(command, { type: "tradeConfirm", locked: true });
    Object.assign(f.data.stage5Systems.trade, { locked: true, escrowPrepared: true }); f.socket.snapshot();
  };
  assert.deepEqual(await f.client.confirmPlayerTrade(), { sent: true, confirmed: false, prepared: true, tradeId: "a".repeat(32) });
  assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
  await assert.rejects(f.client.confirmPlayerTrade(), /unlocked player trade/);
  assert.equal(f.socket.commands.filter(command => command.type === "tradeConfirm").length, 1);
});

test("even a final protocol packet leaves receipt attribution to the full trade observer", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter(); f.open();
  f.socket.onCommand = command => {
    assert.equal(command.type, "tradeConfirm"); f.socket.packet("TradeConfirm"); f.data.stage5Systems.trade = null; f.socket.snapshot();
  };
  const sent = await f.client.confirmPlayerTrade();
  assert.equal(sent.confirmed, false); assert.equal(sent.prepared, false);
  assert.equal(f.client.snapshot().trade.lastFinalConfirm.tradeId, "a".repeat(32));
});

test("unknown owner gold or confirmation outcomes cannot be automatically resent on the same trade", async t => {
  for (const kind of ["gold", "confirm"]) {
    const f = fixture(); t.after(() => f.client.close()); await f.enter(); f.open();
    await assert.rejects(kind === "gold" ? f.client.offerTradeGold(50) : f.client.confirmPlayerTrade(), error => error.code === "PLAYER_TRADE_UNKNOWN");
    const count = f.socket.commands.length;
    await assert.rejects(f.client.offerTradeGold(50), /known outcome/);
    await assert.rejects(f.client.confirmPlayerTrade(), /known outcome/);
    assert.equal(f.socket.commands.length, count);
  }
});

test("a concurrent ordinary action is rejected before sending or marking an economic outcome unknown", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter(); f.open();
  const walking = f.client.walk("Right");
  await assert.rejects(f.client.offerTradeGold(1), /current player action/);
  f.socket.packet("UserLocation", { x: 288, y: 616, direction: "Right" }); await walking;
  await settle();
  f.socket.onCommand = command => { assert.equal(command.type, "tradeGold"); f.applyDelta(command.amount); };
  assert.equal((await f.client.offerTradeGold(1)).offeredGold, 1);
});
