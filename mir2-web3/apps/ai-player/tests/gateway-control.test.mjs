import assert from "node:assert/strict";
import test from "node:test";
import { setImmediate as settle } from "node:timers/promises";
import { GatewayClient } from "../src/gateway-client.mjs";
import { CompanionRuntimeService } from "../src/companion-runtime-service.mjs";
import { parseGatewayJson, stringifyGatewayJson } from "../src/lossless-json.mjs";

const ARCHIVE = { archiveId: "archive:test", residentId: "alo-test", ownerRef: "owner:test", worldEpoch: "epoch:test" };
const world = () => ({ mapFileName: "0", playerObjectId: 12, selfZoneObjectId: 50001, playerHp: 18, playerMaxHp: 18,
  entities: [{ objectId: 12, kind: "selfPlayer", name: "Alo", x: 288, y: 616, hp: 18, maxHp: 18 }],
  inventoryItems: [], beltItems: [], equipmentItems: [], groundDrops: [], questLog: [] });

function fixture({ enabled = true, epoch = "epoch:test" } = {}) {
  const sockets = [];
  class Socket extends EventTarget {
    readyState = 1;
    commands = [];
    constructor() {
      super(); sockets.push(this);
      queueMicrotask(() => {
        this.event({ type: "realmInfo", payload: { realmId: "world:test", ...(epoch === null ? {} : { worldEpoch: epoch }), features: { companionControl: enabled, sharedParty: true } } });
        this.packet("Connected", {});
      });
    }
    event(value) { this.dispatchEvent(new MessageEvent("message", { data: stringifyGatewayJson(value) })); }
    packet(packet, payload) { this.event({ type: "packet", packet, payload }); }
    result(requestId, operation, result = {}, status = "ok") { this.event({ type: "companionResult", payload: { requestId, operation, status, residentId: ARCHIVE.residentId, result } }); }
    send(raw) {
      const command = parseGatewayJson(raw); this.commands.push(command);
      queueMicrotask(() => {
        if (command.type === "clientVersion") this.packet("ClientVersion", { result: 1 });
        else if (command.type === "login") this.packet("LoginSuccess", { characters: [{ name: "Alo", index: 2, class: "Warrior" }] });
        else if (command.type === "startGame") { this.packet("StartGame", { result: 4, resolution: 1024 }); this.event({ type: "worldSnapshot", payload: world() }); }
        else this.onCommand?.(command);
      });
    }
    close() { this.readyState = 3; this.dispatchEvent(new Event("close")); }
  }
  const client = new GatewayClient({ url: "ws://127.0.0.1:7010/ws", realmId: "world:test", timeoutMs: 150, WebSocketImpl: Socket });
  return { client, sockets, get socket() { return sockets.at(-1); }, async enter() { await client.connect(); await client.login("test", "test-only"); await client.startGame(2); } };
}

test("private control/register/result frames stay outside ordinary observations, frame observers and replay history", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  const seen = []; f.client.on("frame", frame => seen.push(frame));
  const before = f.client.snapshot(), sequence = f.client.sequence;
  const registration = f.client.registerCompanionRuntime({ ...ARCHIVE, requestId: "register-private" });
  f.socket.result("register-private", "register", { ...ARCHIVE, generation: 3, privateNote: "private-growth-marker" });
  assert.equal((await registration).result.generation, 3);
  const view = f.client.companionControl("view", { residentId: ARCHIVE.residentId, requestId: "view-private" });
  f.socket.result("view-private", "view", { archive: ARCHIVE, summary: "private-growth-marker" });
  assert.equal((await view).result.summary, "private-growth-marker");
  assert.deepEqual(f.client.snapshot(), before); assert.equal(f.client.sequence, sequence);
  assert.deepEqual(seen, []);
  await assert.rejects(f.client.waitFor(event => ["companionResult", "companionRequest"].includes(event.type), { after: 0, timeoutMs: 15 }), error => error.code === "GATEWAY_RESULT_TIMEOUT");
  assert.equal(JSON.stringify(f.client.snapshot()).includes("private-growth-marker"), false);
});

test("outgoing control requires authentication, active game, enabled feature and trusted world epoch", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.client.connect();
  await assert.rejects(f.client.companionControl("list"), /authenticated/);
  await f.client.login("test", "test-only");
  await assert.rejects(f.client.registerCompanionRuntime(ARCHIVE), /entered the world/);
  await f.client.startGame(2);
  for (const options of [{ enabled: false }, { epoch: null }]) {
    const restricted = fixture(options); t.after(() => restricted.client.close()); await restricted.enter();
    const count = restricted.socket.commands.length;
    await assert.rejects(restricted.client.companionControl("list"), /trusted companion controls/);
    await assert.rejects(restricted.client.registerCompanionRuntime(ARCHIVE), /trusted companion controls/);
    assert.equal(restricted.socket.commands.length, count);
  }
});

test("incoming private requests require the same real auth, feature, world and in-game boundaries", async t => {
  for (const options of [{ enabled: true, epoch: "epoch:test" }, { enabled: false }, { epoch: null }]) {
    const f = fixture(options); t.after(() => f.client.close());
    const requests = [], frames = []; f.client.on("companionRequest", request => requests.push(request)); f.client.on("frame", frame => frames.push(frame));
    await f.client.connect();
    const request = { requestId: "runtime-request", ...ARCHIVE, generation: 3, operation: "view", payload: {}, deadlineAtMs: Date.now() + 1000 };
    f.socket.event({ type: "companionRequest", payload: request }); assert.equal(requests.length, 0);
    await f.client.login("test", "test-only"); f.socket.event({ type: "companionRequest", payload: request }); assert.equal(requests.length, 0);
    await f.client.startGame(2); const before = f.client.sequence;
    f.socket.event({ type: "companionRequest", payload: request });
    assert.equal(requests.length, options.enabled === false || options.epoch === null ? 0 : 1);
    assert.equal(f.client.sequence, before);
    assert.equal(frames.some(frame => frame.event.type === "companionRequest"), false);
  }
});

test("mismatched private request IDs and operations cannot resolve another owner's pending operation", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  let settled = false;
  const request = f.client.companionControl("teach", { residentId: ARCHIVE.residentId, requestId: "teach-one", payload: { expectedPolicyVersion: 1 } });
  request.then(() => { settled = true; }, () => { settled = true; });
  f.socket.result("other-id", "teach", { policyVersion: 999 });
  f.socket.result("teach-one", "deleteEpisode", { policyVersion: 999 });
  await settle(); assert.equal(settled, false);
  f.socket.result("teach-one", "teach", { policyVersion: 2 });
  assert.equal((await request).result.policyVersion, 2);
});

test("old socket generations cannot complete a reused request ID after reconnect", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  const oldSocket = f.socket;
  const old = f.client.companionControl("view", { residentId: ARCHIVE.residentId, requestId: "reused" });
  const rejected = assert.rejects(old, /closed/); await f.client.close(); await rejected;
  await f.enter(); let settled = false;
  const current = f.client.companionControl("view", { residentId: ARCHIVE.residentId, requestId: "reused" });
  current.then(() => { settled = true; }, () => { settled = true; });
  oldSocket.result("reused", "view", { summary: "old-private-state" }); await settle();
  assert.equal(settled, false);
  f.socket.result("reused", "view", { summary: "current-private-state" });
  assert.equal((await current).result.summary, "current-private-state");
});

test("shutdown rejects all pending control and registration requests without replaying or publishing them", async () => {
  const f = fixture(); await f.enter();
  const first = f.client.companionControl("view", { requestId: "pending-view", residentId: ARCHIVE.residentId });
  const second = f.client.registerCompanionRuntime({ ...ARCHIVE, requestId: "pending-register" });
  const failures = [assert.rejects(first, /closed/), assert.rejects(second, /closed/)];
  await f.client.close(); await Promise.all(failures);
  assert.equal(f.socket.commands.filter(command => command.requestId === "pending-view").length, 1);
  assert.equal(f.socket.commands.filter(command => command.requestId === "pending-register").length, 1);
});

test("control requests have bounded unique string IDs, payload size and concurrent capacity", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  const count = f.socket.commands.length;
  await assert.rejects(f.client.companionControl("not-an-operation", { requestId: "bad" }), /Unsupported/);
  await assert.rejects(f.client.companionControl("view", { residentId: "alo-01", requestId: 123 }), /Invalid/);
  await assert.rejects(f.client.companionControl("view", { residentId: "alo-01", requestId: "bad/id" }), /Invalid/);
  await assert.rejects(f.client.companionControl("view", { residentId: "alo-01", requestId: "huge", payload: { data: "x".repeat(61 * 1024) } }), /too large/);
  assert.equal(f.socket.commands.length, count);
  const pending = [];
  for (let index = 0; index < 8; index += 1) pending.push(f.client.companionControl("view", { residentId: "alo-01", requestId: "request-" + index }));
  await assert.rejects(f.client.companionControl("view", { residentId: "alo-01", requestId: "request-0" }), /busy/);
  await assert.rejects(f.client.companionControl("view", { residentId: "alo-01", requestId: "ninth" }), /busy/);
  for (let index = 0; index < 8; index += 1) f.socket.result("request-" + index, "view", {});
  assert.equal((await Promise.all(pending)).length, 8);
});

test("oversized private input closes without leaking its payload through ordinary observers", async t => {
  const f = fixture(); t.after(() => f.client.close()); await f.enter();
  const frames = []; f.client.on("frame", frame => frames.push(frame));
  f.socket.event({ type: "companionRequest", payload: { requestId: "too-large", data: "private-marker".repeat(6000) } });
  assert.equal(f.client.snapshot().ready, false);
  assert.equal(JSON.stringify(frames).includes("private-marker"), false);
});

test("runtime admission generation and owner/world fences reject old private requests before mutation", async t => {
  const f = fixture(); await f.enter();
  const mutations = [], replies = [];
  f.socket.onCommand = command => {
    if (command.type === "companionRuntimeRegister") f.socket.result(command.requestId, "register", { ...ARCHIVE, generation: 3 });
    if (command.type === "companionRuntimeReply") replies.push(command);
  };
  const controller = { recallRequested: false, leaderName: "Owner" };
  const store = { archive: () => ({ ...ARCHIVE }), ownerView: () => ({ archive: { ...ARCHIVE }, profile: { policyVersion: 1 }, episodes: [] }),
    teachSupportBeforeLoot: input => mutations.push(input) };
  const service = new CompanionRuntimeService({ client: f.client, store, controller, realmId: "world:test", clock: () => 1000,
    configuration: { runtimeId: "rules-test", promptVersion: "rules-v1", provider: "rules", model: "rules" }, environment: { MIR2_COMPANION_ARCHIVE_KEY: "07".repeat(32) } });
  t.after(async () => { await service.close(); await f.client.close(); }); await service.start();
  const base = { requestId: "current-request", ...ARCHIVE, generation: 3, operation: "teach", payload: { expectedPolicyVersion: 1 }, deadlineAtMs: 2000 };
  for (const changed of [{ generation: 2 }, { generation: 4 }, { ownerRef: "owner:foreign" }, { worldEpoch: "other-world" }, { archiveId: "other-archive" }, { deadlineAtMs: 999 }]) f.socket.event({ type: "companionRequest", payload: { ...base, ...changed } });
  await settle(); assert.equal(mutations.length, 0); assert.equal(replies.length, 0);
  f.socket.event({ type: "companionRequest", payload: base }); await settle();
  assert.equal(mutations.length, 1); assert.equal(mutations[0].source, "gateway-owner");
  assert.equal(mutations[0].operationId, base.requestId); assert.equal(replies[0].generation, 3); assert.equal(replies[0].status, "ok");
});
