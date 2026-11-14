import assert from "node:assert/strict";
import test from "node:test";
import { GatewayClient } from "../src/gateway-client.mjs";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";

const player = { objectId: 12, kind: "selfPlayer", name: "AchuanAI", x: 288, y: 608, direction: "Down", hp: 18, maxHp: 18 };
const world = () => ({ mapFileName: "0", playerObjectId: 12, playerHp: 18, playerMaxHp: 18, gold: 0, entities: [{ ...player }], inventoryItems: [], beltItems: [], equipmentItems: [], questLog: [], groundDrops: [] });

function sockets({ realmId = "bichon-test", rejectLogin = false, holdWalk = false } = {}) {
  const instances = [];
  class Socket extends EventTarget {
    readyState = 0;
    commands = [];
    constructor() {
      super();
      instances.push(this);
      queueMicrotask(() => {
        this.readyState = 1;
        this.dispatchEvent(new Event("open"));
        this.event({ type: "realmInfo", payload: { realmId, identityToken: "must-not-be-exposed" } });
        this.packet("Connected", {});
        this.event({ type: "worldSnapshot", payload: world() });
      });
    }
    event(event) { this.dispatchEvent(new MessageEvent("message", { data: stringifyGatewayJson(event) })); }
    packet(packet, payload) { this.event({ type: "packet", packet, payload }); }
    send(raw) {
      const command = parseGatewayJson(raw);
      this.commands.push(command);
      queueMicrotask(() => {
        if (command.type === "clientVersion") this.packet("ClientVersion", { result: 1 });
        if (command.type === "login") {
          this.event({ type: "identitySession", payload: { token: "private-session-token" } });
          this.packet(rejectLogin ? "Login" : "LoginSuccess", rejectLogin ? { result: 3 } : { characters: [{ index: 9, name: "AchuanAI", class: "Warrior" }] });
        }
        if (command.type === "startGame") {
          this.packet("StartGame", { result: 4, resolution: 1024 });
          this.event({ type: "worldSnapshot", payload: world() });
        }
        if (command.type === "walk" && !holdWalk) this.packet("UserLocation", { x: 288, y: 608, direction: command.direction });
        if (command.type === "logOut") {
          this.packet("LogOutSuccess", { characters: [] });
          this.event({ type: "worldSnapshot", payload: { ...world(), playerObjectId: null, entities: [] } });
        }
      });
    }
    close() { this.readyState = 3; this.dispatchEvent(new Event("close")); }
  }
  return { Socket, instances };
}

function client(Socket) {
  return new GatewayClient({ url: "ws://127.0.0.1:7010/ws", realmId: "bichon-test", WebSocketImpl: Socket, timeoutMs: 300 });
}

async function enter(subject) {
  await subject.connect();
  await subject.login("dedicated-account", "private-password");
  return subject.startGame(9);
}

test("realm mismatch closes before credentials are sent", async () => {
  const { Socket, instances } = sockets({ realmId: "another-world" });
  const subject = client(Socket);
  await assert.rejects(subject.connect(), /realmId/);
  assert.equal(instances[0].commands.length, 0);
  assert.equal(subject.snapshot().ready, false);
});

test("handshake and pre-login snapshot do not establish entry or expose identity tokens", async (t) => {
  const { Socket } = sockets();
  const subject = client(Socket);
  t.after(() => subject.close());
  const seen = [];
  subject.on("frame", ({ event }) => seen.push(event));
  await subject.connect();
  assert.equal(subject.snapshot().ready, false);
  await assert.rejects(subject.walk("Up"), /authenticated/);
  await subject.login("dedicated-account", "private-password");
  assert.equal(subject.snapshot().ready, false);
  assert.equal(JSON.stringify(seen).includes("private-session-token"), false);
  assert.equal(JSON.stringify(seen).includes("must-not-be-exposed"), false);
  const state = await subject.startGame(9);
  assert.equal(state.ready, true);
  assert.equal(state.player.x, 288);
});

test("a failed Login does not create an account or enter the world", async (t) => {
  const { Socket, instances } = sockets({ rejectLogin: true });
  const subject = client(Socket);
  t.after(() => subject.close());
  await subject.connect();
  await assert.rejects(subject.login("dedicated-account", "private-password"), /rejected login/);
  await assert.rejects(subject.startGame(9), /authenticated/);
  assert.deepEqual(instances[0].commands.map((command) => command.type), ["clientVersion", "login"]);
});

test("a movement correction is not success and overlapping actions cannot share an ACK", async (t) => {
  const { Socket, instances } = sockets({ holdWalk: true });
  const subject = client(Socket);
  t.after(() => subject.close());
  await enter(subject);
  const walking = subject.walk("Right");
  await assert.rejects(subject.walk("Left"), /current player action/);
  instances[0].packet("UserLocation", { x: 288, y: 608, direction: "Right" });
  assert.equal((await walking).moved, false);
});

test("same-text chat is not a speaker receipt and control commands never reach the socket", async (t) => {
  const { Socket, instances } = sockets();
  const subject = client(Socket);
  t.after(() => subject.close());
  await enter(subject);
  assert.deepEqual(await subject.sayNearby("Hello Bichon"), { sent: true, confirmed: false });
  const before = instances[0].commands.length;
  await assert.rejects(subject.sayNearby(" @ADDSTORAGE"), /prefix/);
  assert.equal(instances[0].commands.length, before);
});

test("server errors reject immediately without exposing their private message", async (t) => {
  const { Socket, instances } = sockets({ holdWalk: true });
  const subject = client(Socket);
  t.after(() => subject.close());
  await enter(subject);
  const waiting = subject.walk("Up");
  instances[0].event({ type: "error", message: "secret-backend-detail" });
  await assert.rejects(waiting, (error) => /unconfirmed/.test(error.message) && !error.message.includes("secret-backend-detail"));
  assert.equal(subject.snapshot().ready, false);
});

test("malformed frames invalidate ready before the close handshake finishes", async (t) => {
  const { Socket, instances } = sockets();
  const subject = client(Socket);
  t.after(() => subject.close());
  await enter(subject);
  instances[0].close = () => { instances[0].readyState = 2; };
  instances[0].dispatchEvent(new MessageEvent("message", { data: "invalid-json" }));
  assert.equal(subject.snapshot().ready, false);
  instances[0].readyState = 3;
  instances[0].dispatchEvent(new Event("close"));
});

test("late events from an old socket cannot reset a new session", async (t) => {
  const { Socket, instances } = sockets();
  const subject = client(Socket);
  t.after(() => subject.close());
  await enter(subject);
  const old = instances[0];
  await subject.close();
  await enter(subject);
  const epoch = subject.snapshot().connectionEpoch;
  old.event({ type: "worldSnapshot", payload: { ...world(), mapFileName: "unapproved" } });
  old.dispatchEvent(new Event("error"));
  old.dispatchEvent(new Event("close"));
  assert.equal(subject.snapshot().connectionEpoch, epoch);
  assert.equal(subject.snapshot().ready, true);
  assert.equal((await subject.walk("Right")).moved, false);
});

test("connect is rejected while an asynchronous close is pending", async () => {
  const { Socket, instances } = sockets();
  const subject = client(Socket);
  await enter(subject);
  const old = instances[0];
  old.readyState = 2;
  const closing = subject.close();
  await assert.rejects(subject.connect(), /closing/);
  old.dispatchEvent(new Event("close"));
  await closing;
  await enter(subject);
  assert.equal(subject.snapshot().ready, true);
  await subject.close();
});

test("remote cleartext sockets and credential URLs are rejected", () => {
  for (const url of ["ws://example.com/ws", "wss://user:password@example.com/ws", "wss://example.com/ws?token=secret", "ws://example.com/playtest/ws", "wss://user:password@example.com/playtest/ws", "wss://example.com/playtest/ws?token=secret", "wss://example.com/other/ws", "wss://example.com/playtest/ws#fragment"]) {
    assert.throws(() => new GatewayClient({ url, realmId: "bichon-test" }));
  }
});

test("R16 remote route is supported without changing transport or authentication rules", () => {
  const subject = new GatewayClient({ url: "wss://example.com/playtest/ws", realmId: "bichon-test" });
  assert.equal(subject.url, "wss://example.com/playtest/ws");
  assert.equal(subject.snapshot().ready, false);
  assert.equal(subject.features.companionControl, false);
});
