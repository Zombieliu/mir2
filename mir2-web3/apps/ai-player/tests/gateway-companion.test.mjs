import assert from "node:assert/strict";
import test from "node:test";
import { GatewayClient } from "../src/gateway-client.mjs";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";

const uid = 9007199254740999n;
function fixture({ sharedParty = true } = {}) {
  let socket;
  const state = { mapFileName: "0", playerObjectId: 12, selfZoneObjectId: 50001, playerHp: 9, playerMaxHp: 18,
    entities: [{ objectId: 12, kind: "selfPlayer", name: "AloAI", x: 10, y: 10, hp: 9, maxHp: 18 }],
    inventoryItems: [{ key: "hp", name: "(HP)DrugSmall", uniqueId: uid, quantity: 2, container: "bag1", slot: 0 }], beltItems: [],
    stage5Systems: { group: { allowGroup: true, members: [] } } };
  class Socket extends EventTarget {
    readyState = 1;
    commands = [];
    constructor() {
      super(); socket = this;
      queueMicrotask(() => {
        this.event({ type: "realmInfo", payload: { realmId: "test", features: { sharedParty } } });
        this.packet("Connected", {});
      });
    }
    event(event) { this.dispatchEvent(new MessageEvent("message", { data: stringifyGatewayJson(event) })); }
    packet(packet, payload) { this.event({ type: "packet", packet, payload }); }
    snapshot() { this.event({ type: "worldSnapshot", payload: state }); }
    send(raw) {
      const command = parseGatewayJson(raw); this.commands.push(command);
      queueMicrotask(() => {
        if (command.type === "clientVersion") this.packet("ClientVersion", { result: 1 });
        else if (command.type === "login") this.packet("LoginSuccess", { characters: [{ index: 9, name: "AloAI" }] });
        else if (command.type === "startGame") { this.packet("StartGame", { result: 4, resolution: 1024 }); this.snapshot(); }
        else this.onCommand?.(command);
      });
    }
    close() { this.readyState = 3; this.dispatchEvent(new Event("close")); }
  }
  const client = new GatewayClient({ url: "ws://127.0.0.1:7010/ws", realmId: "test", timeoutMs: 100, WebSocketImpl: Socket });
  return { client, state, get socket() { return socket; }, async enter() { await client.connect(); await client.login("dedicated", "secret"); await client.startGame(9); } };
}

test("companion group controls refuse a gateway with only personal group lists", async t => {
  const f = fixture({ sharedParty: false }); t.after(() => f.client.close()); await f.enter();
  const count = f.socket.commands.length;
  await assert.rejects(f.client.inviteMember("Leader"), /authoritative shared/);
  await assert.rejects(f.client.acceptGroupInvite(), /authoritative shared/);
  assert.equal(f.socket.commands.length, count);
});

test("inviting is unconfirmed; accepting requires a real invitation and both roster names", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  await assert.rejects(f.client.acceptGroupInvite(), /No current/);
  assert.deepEqual(await f.client.inviteMember("Leader"), { sent: true, confirmed: false });
  assert.deepEqual(f.client.snapshot().party.members, []);
  f.socket.packet("GroupInvite", { name: "Leader" });
  f.socket.onCommand = command => {
    assert.equal(command.type, "groupInvite"); assert.equal(command.acceptInvite, true);
    f.socket.packet("GroupMemberInfo", { members: [{ name: "Leader" }, { name: "AloAI" }], leaderName: "Leader" });
  };
  const party = await f.client.acceptGroupInvite();
  assert.deepEqual(party.members, ["Leader", "AloAI"]);
});

test("potion use binds exact lossless UID and grid and waits for actual consumption", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  f.socket.onCommand = command => {
    assert.equal(command.uniqueId, uid); assert.equal(command.grid, "inventory");
    f.socket.packet("UseItem", { uniqueId: uid, success: true, grid: "Belt" });
    f.socket.snapshot(); // Pre-action projection has not consumed anything.
    f.socket.packet("UseItem", { uniqueId: uid + 1n, success: false, grid: "Inventory" });
    f.socket.packet("UseItem", { uniqueId: uid, success: true, grid: "Inventory" });
    f.state.inventoryItems[0].quantity = 1; f.socket.snapshot();
  };
  assert.deepEqual(await f.client.useOwnedItem(uid), { success: true, uniqueId: uid });
  assert.equal(f.client.snapshot().inventoryItems[0].quantity, 1);
  await assert.rejects(f.client.useOwnedItem(uid + 1n), /exact owned/);
});

test("R16 nonleader leave uses ordinary group switching and restores invitations after a real empty roster", async t => {
  const f = fixture(); t.after(() => f.client.close());
  f.state.stage5Systems.group.members = ["Leader", "AloAI"];
  await f.enter();
  let restored = false;
  f.socket.onCommand = command => {
    assert.equal(command.type, "switchGroup", "A nonleader must never send the leader-only DelMember command.");
    f.socket.packet("SwitchGroup", { allowGroup: command.allowGroup });
    f.state.stage5Systems.group.allowGroup = command.allowGroup;
    if (!command.allowGroup) {
      f.socket.snapshot(); // The preference echo alone is not departure proof.
      setTimeout(() => {
        f.socket.packet("DeleteGroup", {});
        f.state.stage5Systems.group.members = [];
        f.socket.snapshot();
      }, 5);
    } else {
      assert.deepEqual(f.state.stage5Systems.group.members, []);
      restored = true; f.socket.snapshot();
    }
  };
  const party = await f.client.leaveGroup();
  assert.equal(restored, true);
  assert.equal(party.allowGroup, true);
  assert.deepEqual(party.members, []);
});

test("rejected R16 group switching cannot claim departure or restore a preference speculatively", async t => {
  const f = fixture(); t.after(() => f.client.close());
  f.state.stage5Systems.group.members = ["Leader", "AloAI"];
  await f.enter();
  const commands = [];
  f.socket.onCommand = command => {
    commands.push(command);
    f.socket.packet("SwitchGroup", { allowGroup: true });
  };
  await assert.rejects(f.client.leaveGroup(), /rejected switchGroup/);
  assert.deepEqual(commands, [{ type: "switchGroup", allowGroup: false }]);
  assert.deepEqual(f.client.snapshot().party.members, ["Leader", "AloAI"]);
});

test("pickup ignores unchanged party snapshots and reports only observed inventory change", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  f.socket.onCommand = () => {
    f.socket.snapshot();
    setTimeout(() => { f.state.inventoryItems.push({ key: "candle", name: "Candle", uniqueId: 6, quantity: 1, container: "bag1" }); f.socket.snapshot(); }, 5);
  };
  assert.deepEqual(await f.client.pickUpTile(), { changed: true });
  f.socket.onCommand = () => f.socket.snapshot();
  assert.deepEqual(await f.client.pickUpTile(), { changed: false, confirmed: false });
});

test("an attack send is not hit evidence and Peace mode requires its server echo", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  assert.deepEqual(await f.client.attackDirection("Right"), { sent: true, confirmed: false });
  await Promise.resolve();
  f.socket.onCommand = command => { assert.deepEqual(command, { type: "changeAMode", mode: 0 }); f.socket.packet("ChangeAMode", { mode: 0 }); };
  assert.equal(await f.client.setPeaceAttackMode(), true);
});

test("normal equipment removal requires a visible worn slot and waits for ownership in the bag", async t => {
  const f = fixture(); t.after(() => f.client.close());
  f.state.equipmentItems = [{ slot: "armour", key: "dress", name: "BaseDress(M)", quantity: 1 }];
  await f.enter();
  f.socket.onCommand = command => {
    assert.deepEqual(command, { type: "removeItem", uniqueId: 1, grid: "inventory", to: 1 });
    f.socket.packet("RemoveItem", { uniqueId: 1, grid: "Inventory", to: 1, success: true });
    f.state.equipmentItems = [];
    f.state.inventoryItems.push({ key: "dress", name: "BaseDress(M)", uniqueId: 5, slot: 1, container: "bag1", quantity: 1 });
    f.socket.snapshot();
  };
  await f.client.removeOwnedEquipment("armour");
  assert.ok(f.client.snapshot().inventoryItems.some(item => item.name === "BaseDress(M)"));
  await assert.rejects(f.client.removeOwnedEquipment("armour"), /Owned equipment/);
});
