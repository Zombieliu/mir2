import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";
import { createSocialBudgetTestRuntime } from "./lib/social-budget-test-runtime.mjs";

// Semantic oracle: the published Rust/Windows implementation in the real
// renderer WASM. The TypeScript under test owns transport custody only.
const manifest = JSON.parse(readFileSync(new URL("../lib/generated/bevy_runtime_version.json", import.meta.url), "utf8"));
const backend = "webgl2-shared";
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
// The shared helper verifies the immutable glue and raw WASM against this
// manifest and compiles once. It never launches the renderer or supplies a
// JavaScript replacement for Native admission/completion rules.
const nativeA = createSocialBudgetTestRuntime();
const nativeB = createSocialBudgetTestRuntime();
const directNative = createSocialBudgetTestRuntime();
const fixtures = JSON.parse(readFileSync(new URL("./fixtures/social-budget-command-mapping.json", import.meta.url), "utf8"));

function loadLeaf(name) {
  const url = new URL(`../lib/${name}.ts`, import.meta.url);
  const source = readFileSync(url, "utf8");
  const result = ts.transpileModule(source, { fileName: fileURLToPath(url), reportDiagnostics: true,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, strict: true } });
  assert.deepEqual((result.diagnostics ?? []).filter(item => item.category === ts.DiagnosticCategory.Error), []);
  const module = { exports: {} };
  new Function("module", "exports", "require", result.outputText)(module, module.exports,
    () => { throw new Error("transport leaf unexpectedly acquired a runtime dependency"); });
  return { ...module.exports, sourceSha256: sha256(Buffer.from(source)) };
}
const { SocialOperationBudgetHost, sourceSha256 } = loadLeaf("social-operation-budget-host");
const { SocialIncomingReplies, socialReplyCommand } = loadLeaf("social-incoming-replies");
console.log(JSON.stringify({ oracle: "real-renderer-wasm", backend, version: manifest.version,
  wasmSha256: manifest.files.find(file => file.path === "public/bevy-runtime/pkg-webgl2-shared/mir2_bevy_runtime_bg.wasm").sha256,
  hostSourceSha256: sourceSha256,
  nodeWithoutDOM: typeof window === "undefined", servicesStarted: 0, networkConnections: 0 }));

const owner = (fields = {}) => ({ socket: {}, connectionGeneration: 1, sessionGeneration: 2,
  playerObjectId: 1000, sceneRevision: 3, mapFileName: "0", ...fields });
const invite = name => ({ type: "addMember", name });
const remove = name => ({ type: "delMember", name });
function runtime(module = createSocialBudgetTestRuntime()) {
  const calls = [];
  return { calls, processMir2SocialBudget(input) {
    const request = JSON.parse(input);
    const response = JSON.parse(module.processMir2SocialBudget(input));
    calls.push({ request, response });
    return JSON.stringify(response);
  } };
}
function setup(source = owner()) {
  const transport = runtime();
  const host = new SocialOperationBudgetHost(() => transport);
  return { source, transport, host };
}
function seed(host, source, name = "guild") {
  for (const step of fixtures.seedSequences[name]) {
    host.receive(source, step.operation === "setViewer" ? "UserInformation" : step.packet, step.payload);
  }
}
function pending(host, source) {
  const result = host.inspect(source);
  assert.equal(result?.ok, true);
  return result;
}
function reserve(host, source, command, now = 1000) {
  const result = host.reserve(command, source, now);
  assert.equal(result.ok, true);
  assert.ok(result.ticket);
  return result.ticket;
}
function entered(host, source, command, now = 1000) {
  const ticket = reserve(host, source, command, now);
  assert.equal(host.validate(ticket, source, command), true);
  assert.equal(host.enter(ticket, source, command), true);
  return ticket;
}
const permission = { type: "editGuildMember", changeType: 5, rankIndex: 2, name: "true", rankName: "3" };
const rankReceipt = (actor = "Leader", options = 10) => ({ status: 7, name: actor, rankIndex: 0,
  ranks: [{ index: 2, name: "Members", options, members: [{ id: 8, name: "Peer", online: true }] }] });

test("the real Native WASM bridge initializes in Node without a DOM or renderer run", () => {
  const input = { version: 1, operation: "reset", owner: { ...fixtures.owner, generation: 1 } };
  assert.equal(JSON.parse(directNative.processMir2SocialBudget(JSON.stringify(input))).ok, true);
  assert.equal(typeof window, "undefined");
});

test("late runtime replays authenticated receives in order; it never buffers a covered outgoing command", () => {
  let current = null;
  const transport = runtime(), source = owner();
  const host = new SocialOperationBudgetHost(() => current);
  seed(host, source);
  assert.deepEqual(host.reserve(invite("BeforeRuntime"), source, 1000), { ok: false });
  assert.equal(transport.calls.length, 0);
  current = transport;
  assert.equal(pending(host, source).pendingCount, 0);
  assert.deepEqual(transport.calls.slice(0, 4).map(call => call.request.operation === "packet"
    ? call.request.packet : call.request.operation), ["reset", "UserInformation", "GuildStatus", "GuildMemberChange"]);
  assert.equal(transport.calls.some(call => call.request.command?.name === "BeforeRuntime"), false);
  const ticket = entered(host, source, permission);
  assert.equal(ticket.kind, "guildRankPermission");
  host.receive(source, "GuildMemberChange", rankReceipt());
  assert.equal(pending(host, source).pendingCount, 0, "real typed Native ACK proves queued identity and roster reached Rust");
});

test("late receive count and byte limits fail closed instead of dropping an ACK, and a new physical connection can start", () => {
  for (const overflow of ["count", "bytes"]) {
    let current = null;
    const transport = runtime(), source = owner();
    const host = new SocialOperationBudgetHost(() => current);
    if (overflow === "count") {
      for (let index = 0; index < 128; index++) host.receive(source, "SwitchGroup", { allowGroup: index % 2 === 0 });
    } else host.receive(source, "GuildStatus", { guildName: "x".repeat(8 * 1024 * 1024) });
    current = transport;
    assert.equal(host.inspect(source), null);
    assert.deepEqual(host.reserve(remove("Blocked"), source, 1000), { ok: false });
    assert.equal(transport.calls.length, 0);
    const replacement = owner({ connectionGeneration: 2 });
    host.receive(replacement, "UserInformation", { name: "Leader", realId: 7, objectId: 1000 });
    assert.equal(pending(host, replacement).pendingCount, 0);
    assert.equal(transport.calls.some(call => call.request.payload?.guildName?.length > 1000), false);
  }
});

test("retiring before runtime arrival discards only the retired buffer and forbids that same physical owner", () => {
  let current = null;
  const transport = runtime(), source = owner();
  const host = new SocialOperationBudgetHost(() => current);
  host.receive(source, "UserInformation", { name: "Retired", realId: 7, objectId: 1000 });
  host.retire(); current = transport;
  assert.deepEqual(host.reserve(remove("Old"), source, 1000), { ok: false });
  const next = owner({ connectionGeneration: 2 });
  host.receive(next, "UserInformation", { name: "Current", realId: 8, objectId: 1000 });
  assert.equal(pending(host, next).pendingCount, 0);
  assert.equal(transport.calls.some(call => call.request.payload?.name === "Retired"), false);
});

test("scene changes retain entered Native work, reject stale scene tickets, and allow exact unsent capacity return", () => {
  const { host, source } = setup();
  entered(host, source, remove("Waiting"));
  const unsent = reserve(host, source, remove("NotSent"));
  const nextScene = { ...source, sceneRevision: 4, mapFileName: "D001" };
  assert.equal(pending(host, nextScene).pendingCount, 2);
  assert.equal(host.validate(unsent, nextScene, remove("NotSent")), false);
  assert.equal(host.enter(unsent, nextScene, remove("NotSent")), false);
  host.cancelUnsent(unsent);
  assert.equal(pending(host, nextScene).pendingCount, 1);
  host.receive(source, "DeleteMember", { name: "Waiting" });
  assert.equal(pending(host, nextScene).pendingCount, 1, "a stale source does not deliver the otherwise matching ACK");
  assert.equal(host.inspect({ ...nextScene, mapFileName: "WrongMapAtSameRevision" }), null);
  host.receive(nextScene, "DeleteMember", { name: "Waiting" });
  assert.equal(pending(host, nextScene).pendingCount, 0);
});

test("A to B to C to old A cannot launder a retired socket into a fresh Native generation", () => {
  const { host, source, transport } = setup();
  const oldTicket = entered(host, source, remove("OldA"));
  const b = owner({ connectionGeneration: 2, sessionGeneration: 1, playerObjectId: 2000 });
  assert.equal(pending(host, b).pendingCount, 0);
  const c = owner({ connectionGeneration: 3, sessionGeneration: 4, playerObjectId: 3000 });
  entered(host, c, remove("CurrentC"));
  const before = transport.calls.length;
  for (const stale of [source, b, { ...c, socket: {} }, { ...c, sessionGeneration: 3 }, { ...c, playerObjectId: 999 }]) {
    host.receive(stale, "DeleteMember", { name: "CurrentC" });
    assert.equal(host.inspect(stale), null);
    assert.deepEqual(host.reserve(remove("Stale"), stale, 1000), { ok: false });
    assert.equal(host.enter(oldTicket, stale, remove("OldA")), false);
  }
  assert.equal(transport.calls.length, before);
  assert.equal(pending(host, c).pendingCount, 1);
  const nextRole = { ...c, sessionGeneration: 5, playerObjectId: 3001 };
  assert.equal(pending(host, nextRole).pendingCount, 0);
  assert.equal(host.inspect(c), null);
  host.retire();
  assert.equal(host.inspect(nextRole), null);
  assert.deepEqual(host.reserve(remove("Retired"), nextRole, 1000), { ok: false });
});

test("invalid physical owners never establish a Native binding or spend a slot", () => {
  const { host, source, transport } = setup();
  for (const bad of [null, { ...source, socket: null }, { ...source, connectionGeneration: 0 },
    { ...source, sessionGeneration: 0 }, { ...source, playerObjectId: 0 },
    { ...source, playerObjectId: 0x1_0000_0000 }, { ...source, sceneRevision: -1 },
    { ...source, mapFileName: "" }, { ...source, mapFileName: "bad\0map" }]) {
    assert.deepEqual(host.reserve(remove("Bad"), bad, 1000), { ok: false });
  }
  assert.equal(transport.calls.length, 0);
  assert.equal(pending(host, source).pendingCount, 0);
});

test("a renderer replacement cannot replace the module holding a physical session's Native ACK history", () => {
  const first = runtime(nativeA), replacement = runtime(nativeB), source = owner();
  let current = first;
  const host = new SocialOperationBudgetHost(() => current);
  entered(host, source, remove("HeldByFirst"));
  current = replacement;
  assert.equal(pending(host, source).pendingCount, 1);
  host.receive(source, "DeleteMember", { name: "HeldByFirst" });
  assert.equal(pending(host, source).pendingCount, 0);
  assert.equal(replacement.calls.length, 0);
  current = null;
  entered(host, source, remove("StillFirst"));
  assert.equal(pending(host, source).pendingCount, 1);
  current = replacement;
  const next = owner({ connectionGeneration: 2 });
  assert.equal(pending(host, next).pendingCount, 0);
  assert.equal(first.calls.at(-1).request.operation, "retire");
  assert.ok(replacement.calls.some(call => call.request.operation === "reset"));
});

test("exact unsent and forged-ticket fences return capacity, while entered socket-unknown work remains pending", () => {
  const { host, source } = setup();
  const command = remove("Unsent"), ticket = reserve(host, source, command);
  assert.equal(host.enter({ ...ticket }, source, command), false, "numeric token knowledge is not host custody");
  assert.equal(host.validate(ticket, source, remove("DifferentDTO")), false);
  assert.equal(host.enter(ticket, source, remove("DifferentDTO")), false);
  host.cancelUnsent(ticket);
  assert.equal(pending(host, source).pendingCount, 0);
  assert.equal(host.enter(ticket, source, command), false);
  const unknown = entered(host, source, remove("Unknown"));
  assert.throws(() => { throw new Error("simulated socket.send entered and threw"); }, /entered/);
  host.cancelUnsent(unknown);
  assert.equal(pending(host, source).pendingCount, 1);
  assert.equal(host.enter(unknown, source, remove("Unknown")), false);
  assert.deepEqual(host.reserve(remove("Unknown"), source, 1000), { ok: false });
  host.receive(source, "Chat", { message: "Unknown could not be found" });
  host.receive(source, "SwitchGroup", { allowGroup: false });
  assert.equal(pending(host, source).pendingCount, 1);
  host.receive(source, "DeleteMember", { name: "Other" });
  assert.equal(pending(host, source).pendingCount, 1);
  host.receive(source, "DeleteMember", { name: "Unknown" });
  assert.equal(pending(host, source).pendingCount, 0);
});

test("an ordinary AddMember spends one reservation and permits a new explicit invite without any receipt", () => {
  const { host, source, transport } = setup();
  const retained = entered(host, source, remove("Blade"));
  const command = invite("bLaDe"), first = reserve(host, source, command);
  assert.equal(first.kind, "groupAdd");
  assert.equal(pending(host, source).pendingCount, 2);
  assert.equal(host.enter(first, source, command), true);
  assert.deepEqual(transport.calls.at(-1).response.released, [first.token]);
  assert.deepEqual(pending(host, source).pending.map(row => row.token), [retained.token]);
  assert.equal(host.enter(first, source, command), false);
  host.receive(source, "Chat", { chatType: "System", message: "Blade has declined your group invite." });
  assert.deepEqual(pending(host, source).pending.map(row => row.token), [retained.token]);
  const second = entered(host, source, command);
  assert.ok(second.token > first.token);
  host.receive(source, "AddMember", { name: "Blade" });
  assert.deepEqual(pending(host, source).pending.map(row => row.token), [retained.token]);
});

test("a reply before entry is not completion and invalidates that reservation instead of granting a late send", () => {
  const { host, source } = setup();
  const command = remove("Early"), ticket = reserve(host, source, command);
  host.receive(source, "DeleteMember", { name: "Early" });
  assert.equal(pending(host, source).pendingCount, 1);
  assert.equal(host.validate(ticket, source, command), false);
  assert.equal(host.enter(ticket, source, command), false);
  host.cancelUnsent(ticket);
  assert.equal(pending(host, source).pendingCount, 0);
});

test("real Native Group, Guild and Trade share eight slots; exact cross-domain ACKs free only matching work", () => {
  const { host, source } = setup(); seed(host, source); seed(host, source, "tradeOpen");
  for (const name of ["Wait0", "Wait1", "Wait2"]) entered(host, source, remove(name));
  entered(host, source, { type: "switchGroup", allowGroup: true });
  entered(host, source, { type: "editGuildNotice", notice: ["Bounded notice"] });
  entered(host, source, { type: "guildStorageGoldChange", changeType: 0, amount: 25 });
  entered(host, source, { type: "tradeGold", amount: 25 });
  entered(host, source, { type: "depositTradeItem", from: 42, to: 1 });
  assert.equal(pending(host, source).pendingCount, 8);
  assert.deepEqual(host.reserve(invite("Ninth"), source, 1000), { ok: false });
  host.receive(source, "GuildStorageGoldChange", { changeType: 3, amount: 24 });
  assert.equal(pending(host, source).pendingCount, 8);
  host.receive(source, "GuildStorageGoldChange", { changeType: 3, amount: 25 });
  assert.equal(pending(host, source).pendingCount, 7);
  const ninth = reserve(host, source, invite("Ninth"));
  assert.equal(pending(host, source).pendingCount, 8); host.cancelUnsent(ninth);
  const offer = fixtures.snapshotFixtures[0];
  host.snapshot(source, offer.snapshot, offer.inventoryJson);
  assert.equal(pending(host, source).pendingCount, 6, "the real own-offer projection acknowledges gold, not an item tuple");
  host.snapshot(source, offer.snapshot, offer.inventoryJson);
  assert.equal(pending(host, source).pendingCount, 6, "identical own-offer projection is not another ACK");
  host.receive(source, "DepositTradeItem", { from: 41, to: 1, success: false });
  assert.equal(pending(host, source).pendingCount, 6);
  host.receive(source, "DepositTradeItem", { from: 42, to: 1, success: false });
  assert.equal(pending(host, source).pendingCount, 5);
  host.receive(source, "DeleteMember", { name: "Wait0" });
  assert.equal(pending(host, source).pendingCount, 4);
  host.receive(source, "GuildNoticeChange", { notice: [], update: -1 });
  assert.equal(pending(host, source).pendingCount, 3);
  host.receive(source, "SwitchGroup", { allowGroup: true });
  assert.equal(pending(host, source).pendingCount, 2);
});

test("real GuildNoticeChange and GroupMembersMap arrivals reach Native; map metadata is not a GroupRemove ACK", () => {
  const { host, source, transport } = setup(); seed(host, source);
  entered(host, source, { type: "editGuildNotice", notice: ["New notice"] });
  host.receive(source, "GuildNoticeChange", { update: -1, notice: [] });
  assert.equal(transport.calls.at(-1).request.packet, "GuildNoticeChange");
  assert.equal(transport.calls.at(-1).response.handled, true);
  assert.equal(pending(host, source).pendingCount, 0);
  entered(host, source, remove("Peer"));
  host.receive(source, "GroupMembersMap", { playerName: "Peer", playerMap: "D001" });
  assert.equal(transport.calls.at(-1).request.packet, "GroupMembersMap");
  assert.equal(transport.calls.at(-1).response.handled, true);
  assert.equal(pending(host, source).pendingCount, 1);
});

test("typed permission completion comes from the real Native actor, nested target and expected bit cursor", () => {
  const { host, source } = setup(); seed(host, source);
  entered(host, source, permission);
  host.receive(source, "GuildMemberChange", { changeType: 5, name: "Leader", rankIndex: 2, ranks: [] });
  assert.equal(pending(host, source).pendingCount, 1);
  host.receive(source, "GuildMemberChange", rankReceipt("Officer"));
  assert.equal(pending(host, source).pendingCount, 1);
  host.receive(source, "GuildMemberChange", fixtures.seedSequences.guild[2].payload);
  assert.equal(pending(host, source).pendingCount, 1, "a full roster cannot replace the typed terminal packet");
  host.receive(source, "GuildMemberChange", rankReceipt());
  assert.equal(pending(host, source).pendingCount, 0);
});

test("real incoming UI epochs remain opaque and never become Native request data or wire fields", () => {
  const { host, source, transport } = setup();
  const replies = new SocialIncomingReplies();
  const before = globalThis.__mir2SocialReplyEpoch;
  globalThis.__mir2SocialReplyEpoch = Math.max(Number(before ?? 0), 50000);
  try {
    for (const [kind, packet, name] of [["group", "GroupInvite", "Inviter"], ["guild", "GuildInvite", "GuildInviter"]]) {
      for (let index = 0; index < 2; index++) {
        host.receive(source, packet, { name });
        assert.equal(replies.receive(kind, name, null, source), true);
      }
      const ui = replies.list(source).find(request => request.kind === kind);
      assert.ok(ui.epoch > 50000);
      const proof = replies.reserve(kind, ui.epoch, true, source);
      assert.ok(proof);
      const command = socialReplyCommand(proof);
      const ticket = reserve(host, source, command);
      assert.equal(replies.claim(proof, source, command), true);
      assert.equal(host.enter(ticket, source, command), true);
      const request = transport.calls.findLast(call => call.request.operation === "reserve").request;
      assert.equal(Object.hasOwn(request, "invite"), false);
      assert.equal(JSON.stringify(request).includes(String(ui.epoch)), false);
      assert.deepEqual(Object.keys(request.command).sort(), ["acceptInvite", "type"]);
    }
    assert.equal(pending(host, source).pendingCount, 2);
  } finally {
    // Keep the real monotonic allocator intact; restoring an old epoch would
    // create the very replay opportunity this test is meant to avoid.
    assert.ok(globalThis.__mir2SocialReplyEpoch >= (before ?? 0));
  }
});

test("actual transient UI commands, declines and independent domains never consume the long-lived eight slots", () => {
  const { host, source, transport } = setup();
  for (let index = 0; index < 8; index++) entered(host, source, remove("Wait" + index));
  const outside = [{ type: "tradeRequest" }, { type: "tradeReply", acceptInvite: true },
    { type: "tradeReply", acceptInvite: false }, { type: "tradeConfirm", locked: true },
    { type: "tradeConfirm", locked: false }, { type: "groupInvite", acceptInvite: false },
    { type: "guildInvite", acceptInvite: false }, { type: "guildStorageItemChange", changeType: 3, from: 0, to: 0 },
    { type: "refreshFriends" }, { type: "mentorReply", acceptInvite: true }, { type: "marriageReply", acceptInvite: true }];
  const before = transport.calls.length;
  for (const command of outside) {
    assert.equal(SocialOperationBudgetHost.covers(command), false);
    assert.deepEqual(host.reserve(command, source, 1000), { ok: true, ticket: null });
  }
  assert.equal(transport.calls.length, before, "outside-budget is not a Native admission or authority decision");
  assert.equal(pending(host, source).pendingCount, 8);
  assert.deepEqual(host.reserve(invite("StillNinth"), source, 1000), { ok: false });
});

test("only Native's explicit rename expiry returns that slot; elapsed time does not clear group removal", () => {
  const { host, source } = setup(); seed(host, source);
  entered(host, source, { type: "editGuildMember", changeType: 3, rankIndex: 2, name: "", rankName: "Scouts" });
  entered(host, source, remove("Unknown"));
  const atDeadline = reserve(host, source, remove("AtDeadline"), 6000);
  assert.equal(pending(host, source).pendingCount, 3); host.cancelUnsent(atDeadline);
  const later = reserve(host, source, remove("AfterDeadline"), 6001);
  assert.deepEqual(pending(host, source).pending.map(row => row.kind), ["groupRemove", "groupRemove"]);
  host.cancelUnsent(later);
  assert.deepEqual(host.reserve(remove("ClockRegression"), source, 6000), { ok: false });
  assert.equal(pending(host, source).pendingCount, 1);
});

test("bridge failure after real Native entry freezes the host without replaying or fabricating an unsent cancellation", () => {
  const source = owner(), trace = [];
  let corruptEnter = true;
  const transport = { processMir2SocialBudget(input) {
    const request = JSON.parse(input), output = nativeA.processMir2SocialBudget(input);
    trace.push(request);
    if (request.operation === "enter" && corruptEnter) { corruptEnter = false; return "not-json"; }
    return output;
  } };
  const host = new SocialOperationBudgetHost(() => transport);
  const command = remove("NativeEntered"), ticket = reserve(host, source, command);
  assert.equal(host.enter(ticket, source, command), false);
  const lastOwner = trace.at(-1).owner;
  const actual = JSON.parse(nativeA.processMir2SocialBudget(JSON.stringify({ version: 1, operation: "inspect", owner: lastOwner })));
  assert.equal(actual.pendingCount, 1); assert.equal(actual.pending[0].entered, true);
  const before = trace.length;
  host.cancelUnsent(ticket);
  assert.deepEqual(host.reserve(remove("Retry"), source, 1000), { ok: false });
  assert.equal(host.inspect(source), null);
  assert.equal(trace.length, before, "the failed binding cannot issue any automatic recovery action");
});
