import assert from "node:assert/strict";
import test from "node:test";
import { GatewayClient } from "../src/gateway-client.mjs";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";
import { OWNER, UID, tradeState, world } from "./gift-fixture.mjs";

function fixture() {
  let socket;
  const data = world();
  class Socket extends EventTarget {
    readyState = 1; commands = [];
    constructor() {
      super(); socket = this;
      queueMicrotask(() => {
        this.event({ type: "realmInfo", payload: { realmId: "test:gifts", worldEpoch: "epoch:gifts", features: { sharedParty: true, companionControl: true } } });
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
        else if (command.type === "login") this.packet("LoginSuccess", { characters: [{ name: "AloGift", index: 2 }] });
        else if (command.type === "startGame") { this.packet("StartGame", { result: 4, resolution: 1024 }); this.snapshot(); }
        else this.onCommand?.(command);
      });
    }
    close() { this.readyState = 3; this.dispatchEvent(new Event("close")); }
  }
  const client = new GatewayClient({ url: "ws://127.0.0.1:7010/ws", realmId: "test:gifts", timeoutMs: 50, WebSocketImpl: Socket });
  return { client, data, get socket() { return socket; }, async enter() { await client.connect(); await client.login("gift-synthetic", "test-only"); await client.startGame(2); } };
}

test("ordinary gift acceptance binds the current exact invitation and never offers assets", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  await assert.rejects(f.client.acceptOwnerGiftInvite(OWNER, "absent"), /current owner/);
  f.socket.packet("TradeRequest", { partnerName: "Stranger" });
  await assert.rejects(f.client.acceptOwnerGiftInvite(OWNER, f.client.snapshot().tradePendingInvite.id), /current owner/);
  f.socket.packet("TradeRequest", { partnerName: OWNER }); const invite = f.client.snapshot().tradePendingInvite;
  f.socket.onCommand = command => {
    assert.deepEqual(command, { type: "tradeReply", acceptInvite: true });
    f.socket.packet("TradeAccept", { partnerName: OWNER }); f.data.stage5Systems.trade = tradeState(); f.socket.snapshot();
  };
  assert.deepEqual(await f.client.acceptOwnerGiftInvite(OWNER, invite.id), { accepted: true, tradeId: "a".repeat(32) });
  assert.equal(f.socket.commands.some(command => ["tradeGold", "depositTradeItem"].includes(command.type)), false);
});

test("gift confirm refuses all observed outgoing assets and uses only one ordinary lock packet", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  f.data.stage5Systems.trade = tradeState({ offeredUniqueIds: { 0: UID } }); f.socket.snapshot();
  await assert.rejects(f.client.confirmReceiveOnlyGift({ tradeId: "a".repeat(32), ownerName: OWNER }), /zero-outgoing/);
  f.data.stage5Systems.trade = tradeState({ offeredGold: 1 }); f.socket.snapshot();
  await assert.rejects(f.client.confirmReceiveOnlyGift({ tradeId: "a".repeat(32), ownerName: OWNER }), /zero-outgoing/);
  f.data.stage5Systems.trade = tradeState(); f.socket.snapshot();
  f.socket.onCommand = command => {
    assert.deepEqual(command, { type: "tradeConfirm", locked: true });
    Object.assign(f.data.stage5Systems.trade, { locked: true, escrowPrepared: true }); f.socket.snapshot();
  };
  assert.deepEqual(await f.client.confirmReceiveOnlyGift({ tradeId: "a".repeat(32), ownerName: OWNER }), { sent: true, confirmed: false });
  await assert.rejects(f.client.confirmReceiveOnlyGift({ tradeId: "a".repeat(32), ownerName: OWNER }), /zero-outgoing/);
  assert.equal(f.socket.commands.filter(command => command.type === "tradeConfirm").length, 1);
});

test("gift cancellation uses ordinary tradeCancel and cannot report a receipt", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  f.data.stage5Systems.trade = tradeState(); f.socket.snapshot();
  f.socket.onCommand = command => {
    assert.deepEqual(command, { type: "tradeCancel" }); f.socket.packet("TradeCancel", { unlock: false }); f.data.stage5Systems.trade = null; f.socket.snapshot();
  };
  assert.deepEqual(await f.client.cancelGiftTrade(), { sent: true, confirmed: false });
  assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
});

test("missing complete trade source prevents confirmation before any socket write", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  f.data.stage5Systems.trade = tradeState(); f.socket.snapshot();
  f.socket.event({ type: "worldSnapshot", payload: { ...f.data, stage5Systems: {} } });
  const before = f.socket.commands.length;
  await assert.rejects(f.client.confirmReceiveOnlyGift({ tradeId: "a".repeat(32), ownerName: OWNER }), /zero-outgoing/);
  assert.equal(f.socket.commands.length, before);
});

test("declining a pending invite uses ordinary reply without inventing its absent ACK", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  f.socket.packet("TradeRequest", { partnerName: OWNER });
  assert.deepEqual(await f.client.cancelGiftTrade(), { sent: true, confirmed: false });
  assert.deepEqual(f.socket.commands.at(-1), { type: "tradeReply", acceptInvite: false });
  assert.equal(f.client.snapshot().tradePendingInvite, null); assert.equal(f.client.snapshot().trade.lastFinalConfirm, null);
});
