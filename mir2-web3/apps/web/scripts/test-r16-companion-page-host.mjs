import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import test from "node:test";
import ts from "typescript";

// Execute complete current Page declarations. No React/Page import, browser,
// network, Core loader, model provider or service is initialized by this test.
const read = path => readFileSync(new URL(path, import.meta.url), "utf8");
const source = read("../app/page.tsx"), ast = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const wanted = ["captureCompanionSource", "retirePrivateCompanion", "sendPrivateCompanion", "receivePrivateCompanionFrame",
  "changeCompanionOpen", "inviteCompanionToParty", "sendRaw", "suspendEquipmentConnection", "retireEquipmentSession"];
const declarations = new Map(), messageHandlers = [], realmBranches = [], lifecycleCases = new Map();
function callNamed(node, name) { return ts.isCallExpression(node) && node.expression.getText(ast) === name; }
function containsCall(node, name) { let found = false; function visit(n) { if (callNamed(n, name)) found = true; ts.forEachChild(n, visit); } visit(node); return found; }
function walk(node) {
  if (ts.isFunctionDeclaration(node) && wanted.includes(node.name?.text)) { assert(!declarations.has(node.name.text)); declarations.set(node.name.text, node); }
  if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && node.expression.name.text === "addEventListener"
    && node.arguments[0]?.getText(ast) === '"message"' && node.arguments[1] && containsCall(node.arguments[1], "receivePrivateCompanionFrame")) messageHandlers.push(node.arguments[1]);
  if (ts.isIfStatement(node) && ts.isBinaryExpression(node.expression) && node.expression.left.getText(ast) === "event.type"
    && ts.isStringLiteral(node.expression.right) && node.expression.right.text === "realmInfo") realmBranches.push(node.thenStatement);
  if (ts.isCaseClause(node) && ts.isStringLiteral(node.expression) && ["StartGame", "LogOutSuccess"].includes(node.expression.text)) {
    const name = node.expression.text;
    if (containsCall(node, name === "StartGame" ? "suspendEquipmentConnection" : "retireEquipmentSession")) { assert(!lifecycleCases.has(name)); lifecycleCases.set(name, node); }
  }
  ts.forEachChild(node, walk);
}
walk(ast);
assert.deepEqual([...declarations.keys()].sort(), wanted.slice().sort()); assert.equal(messageHandlers.length, 1); assert.equal(realmBranches.length, 1);
function compile(code, scope) {
  const js = ts.transpileModule(code, { fileName: "extracted-host.tsx", compilerOptions: { target: ts.ScriptTarget.ES2022, jsx: ts.JsxEmit.ReactJSX } }).outputText;
  return new Function(...Object.keys(scope), js)(...Object.values(scope));
}
function loadPure(path) {
  const module = { exports: {} }, js = ts.transpileModule(read(path), { fileName: path,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } }).outputText;
  new Function("exports", "module", js)(module.exports, module); return module.exports;
}
const companion = loadPure("../lib/r16-companion-client.ts"), privateFrames = loadPure("../lib/r16-companion-private-frame.ts");
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
function fixture() {
  const trace = [], bodies = [], received = [], logs = [], history = [], revoked = [], groups = [];
  let spectator = false, enteredThrow = false, groupEffect = null;
  const socket = { readyState: 1, send(body) { trace.push("private-send"); bodies.push(body); if (enteredThrow) throw new Error("SYNTHETIC ENTERED SEND FAILURE"); } };
  const scope = {
    window: {}, WebSocket: { OPEN: 1 },
    screenRef: { current: "game" }, equipmentHostSuspendReasonRef: { current: null },
    worldRef: { current: { connected: true, playerObjectId: "42", entities: [{ kind: "selfPlayer", objectId: "42" }] } },
    socketRef: { current: socket }, equipmentConnectionGenerationRef: { current: 3 }, equipmentSessionGenerationRef: { current: 5 },
    equipmentStartGameRef: { current: { connectionGeneration: 3, sessionGeneration: 5, afterSnapshotVersion: 10 } },
    socialCharacterIndexRef: { current: { current: 7, requested: null } }, companionContextEpochRef: { current: 2 },
    companionRealmRef: { current: { socket, generation: 3, info: { realmId: "trial", worldEpoch: "world-1", features: { companionControl: true } } } },
    companionClientRef: { current: null }, companionOpenRef: { current: true },
    isSpectatorBrowserMode: () => spectator,
    setCompanionOpen(value) { scope.companionOpenRef.current = value; trace.push(`open:${value}`); },
    cancelPlayerUiWorldIntent() { trace.push("cancel-world-intent"); },
    sameR16CompanionSource: companion.sameR16CompanionSource,
    readR16CompanionPrivateFrame: privateFrames.readR16CompanionPrivateFrame,
    send(command, options) { groups.push({ command, options }); groupEffect?.(command); return true; },
    groupInviteMember(name) { return scope.send({ type: "addMember", name }); },
    groupOperationsRef: { current: new (loadPure("../lib/group-operations.ts").GroupOperations)() },
    groupHiddenTargetsRef: { current: { scope: "", ids: new Set() } },
    appendLog(...args) { logs.push(args); }, t: (_key, _args, fallback) => fallback ?? _key,
    setRealmInfo(info) { scope.realmInfo = info; },
    // Lifecycle tests deliberately fail the unrelated next statement: the real
    // complete function must already have erased private state before that work.
    npcPurchaseClientRef: { get current() { throw new Error("STOP AFTER PRIVATE RETIREMENT"); } },
  };
  const code = [...declarations.values()].map(n => n.getText(ast)).join("\n");
  const api = compile(code + `\nreturn {${wanted.join(",")}};`, scope);
  const client = new companion.R16CompanionClient({ captureSource: api.captureCompanionSource, sendPrivate: api.sendPrivateCompanion,
    randomId: () => "page-host-fixture", now: () => 1000, sha256: async bytes => sha(bytes), makeDownload: () => "blob:private-fixture", revokeDownload(url) { revoked.push(url); } });
  scope.companionClientRef.current = client;
  const originalReceive = client.receive.bind(client);
  client.receive = (captured, event) => { received.push({ source: captured, event }); return originalReceive(captured, event); };
  const handler = compile(`return (${messageHandlers[0].getText(ast)});`, { ...scope, receivePrivateCompanionFrame: api.receivePrivateCompanionFrame,
    socket, connectionGeneration: 3, performance: { now() { trace.push("clock"); return 1000; } },
    syncHeroRawIngress() { trace.push("hero-sync"); }, currentHeroRawPhysical() { trace.push("hero-physical"); return {}; },
    heroRawIngressRef: { current: { capture() { trace.push("hero-capture"); return {}; }, reject() { trace.push("hero-reject"); } } },
    receiveNpcPurchaseGatewayFrame(event) { trace.push("generic-decode"); history.push(event.data); },
  });
  const applyRealm = compile(`return function applyRealmInfo(event, source, connectionGeneration) ${realmBranches[0].getText(ast)}`, {
    ...scope, retirePrivateCompanion: api.retirePrivateCompanion,
  });
  const currentRequest = () => JSON.parse(bodies.at(-1));
  function reply(result, status = "ok") {
    const request = currentRequest();
    return api.receivePrivateCompanionFrame(JSON.stringify({ type: "companionResult", payload: { requestId: request.requestId, residentId: request.residentId ?? null, operation: request.operation, status, result } }), socket, 3);
  }
  const view = () => ({ archive: { residentId: "alo-1", archiveId: "archive-1", displayName: "阿洛", characterName: "AloAI", realmId: "trial", worldEpoch: "world-1", baseSetting: "PRIVATE SETTING" },
    profile: { policyVersion: 1, style: "collection", party: { assistBeforeLoot: false, lootOnlyAfterCombat: false, askBeforeLeaving: false } }, growthRevision: 1,
    feedback: [], episodes: [{ episodeId: "episode-1", summary: "PRIVATE MEMORY" }], nextCursor: null, runtime: {} });
  async function prime() {
    assert(client.list(client.snapshot().renderLease)); reply({ companions: [{ residentId: "alo-1", archiveId: "archive-1", displayName: "阿洛", online: true }], worldEpoch: "world-1" }); reply(view());
    assert(client.exportArchive(client.snapshot().renderLease)); const bytes = Buffer.from("PRIVATE ARCHIVE");
    reply({ exportId: "export-1", totalBytes: bytes.length, chunkCount: 1, sha256: sha(bytes) });
    reply({ exportId: "export-1", index: 0, chunkCount: 1, dataBase64: bytes.toString("base64") }); await Promise.resolve(); await Promise.resolve();
    assert.equal(client.snapshot().download?.url, "blob:private-fixture");
  }
  return { api, scope, client, socket, trace, bodies, received, logs, history, revoked, groups, handler, applyRealm, prime,
    spectator(value) { spectator = value; }, throwing(value) { enteredThrow = value; }, onGroupSend(effect) { groupEffect = effect; } };
}
const frame = () => ({ type: "companionControl", requestId: "request-1", residentId: "alo-1", operation: "teach", payload: { expectedPolicyVersion: 1 } });

test("actual capture uses current authenticated StartGame, role, physical socket, realm and context only", () => {
  const h = fixture(), s = h.api.captureCompanionSource();
  assert.deepEqual(s, { socket: h.socket, socketGeneration: 3, sessionId: "session:3:5", roleId: "character:7:42", realmId: "trial", worldEpoch: "world-1", contextEpoch: 2 }); assert(Object.isFrozen(s));
  const patches = [
    h => { h.scope.screenRef.current = "login"; }, h => { h.scope.screenRef.current = "select"; }, h => h.spectator(true),
    h => { h.scope.worldRef.current.connected = false; }, h => { h.scope.equipmentHostSuspendReasonRef.current = "logoutPending"; },
    h => { h.scope.socketRef.current = null; }, h => { h.socket.readyState = 3; }, h => { h.scope.equipmentStartGameRef.current = null; },
    h => { h.scope.equipmentStartGameRef.current.connectionGeneration++; }, h => { h.scope.equipmentStartGameRef.current.sessionGeneration++; },
    h => { h.scope.companionRealmRef.current.socket = {}; }, h => { h.scope.companionRealmRef.current.generation++; },
    h => { h.scope.companionRealmRef.current.info.features.companionControl = false; }, h => { h.scope.companionRealmRef.current.info.worldEpoch = ""; },
    h => { h.scope.companionRealmRef.current.info.realmId = "bad/realm"; }, h => { h.scope.socialCharacterIndexRef.current.current = null; },
    h => { h.scope.socialCharacterIndexRef.current.current = -1; }, h => { h.scope.worldRef.current.playerObjectId = "0"; },
    h => { h.scope.worldRef.current.playerObjectId = "4294967296"; }, h => { h.scope.worldRef.current.entities = [{ kind: "monster", objectId: "42" }]; },
    h => { h.scope.companionContextEpochRef.current = Number.MAX_SAFE_INTEGER; },
  ];
  for (const patch of patches) { const next = fixture(); patch(next); assert.equal(next.api.captureCompanionSource(), null); }
});
test("actual private sender bypasses generic state/logging and distinguishes entered throw from not-sent", () => {
  const h = fixture(), captured = h.api.captureCompanionSource(), request = frame();
  assert.equal(h.api.sendPrivateCompanion(captured, request), "sent"); assert.deepEqual(JSON.parse(h.bodies[0]), request);
  assert.deepEqual(h.groups, []); assert.deepEqual(h.logs, []); assert.deepEqual(h.history, []);
  h.throwing(true); assert.equal(h.api.sendPrivateCompanion(captured, request), "unknown"); assert.equal(h.bodies.length, 2);
  h.scope.socketRef.current = null; assert.equal(h.api.sendPrivateCompanion(captured, request), "not-sent"); assert.equal(h.bodies.length, 2);
});
test("every captured source dimension is rechecked after serialization and never upgraded to a new source", () => {
  for (const mutate of [
    h => { h.scope.socketRef.current = { readyState: 1, send() { assert.fail("replacement send"); } }; },
    h => { h.scope.equipmentConnectionGenerationRef.current++; }, h => { h.scope.equipmentSessionGenerationRef.current++; },
    h => { h.scope.socialCharacterIndexRef.current.current++; }, h => { h.scope.worldRef.current.playerObjectId = "43"; h.scope.worldRef.current.entities.push({ kind: "selfPlayer", objectId: "43" }); },
    h => { h.scope.companionRealmRef.current.info.realmId = "next"; }, h => { h.scope.companionRealmRef.current.info.worldEpoch = "next"; },
    h => { h.scope.companionContextEpochRef.current++; },
  ]) {
    const h = fixture(), captured = h.api.captureCompanionSource();
    const request = { ...frame(), toJSON() { mutate(h); return frame(); } };
    assert.equal(h.api.sendPrivateCompanion(captured, request), "not-sent"); assert.equal(h.bodies.length, 0);
  }
  const h = fixture(), captured = h.api.captureCompanionSource();
  assert.equal(h.api.sendPrivateCompanion(captured, { ...frame(), payload: { text: "x".repeat(16384) } }), "not-sent");
  assert.equal(h.api.sendPrivateCompanion(captured, { toJSON() { throw new Error("SYNTHETIC"); } }), "not-sent"); assert.equal(h.bodies.length, 0);
});
test("actual lifecycle functions erase owner-private memory and Blob before unrelated teardown", async () => {
  for (const [name, argument] of [["retirePrivateCompanion"], ["suspendEquipmentConnection", "logoutPending"], ["suspendEquipmentConnection", "socketClosed"], ["retireEquipmentSession"]]) {
    const h = fixture(); await h.prime(); const captured = h.api.captureCompanionSource();
    if (name === "retirePrivateCompanion") h.api[name](); else assert.throws(() => h.api[name](argument), /STOP AFTER PRIVATE RETIREMENT/);
    assert.equal(h.client.snapshot().view, null); assert.equal(h.client.snapshot().download, null); assert.deepEqual(h.revoked, ["blob:private-fixture"]);
    assert.equal(h.scope.companionOpenRef.current, false); assert(h.scope.companionContextEpochRef.current > captured.contextEpoch);
    assert.equal(h.api.sendPrivateCompanion(captured, frame()), "not-sent");
  }
});
test("actual successful StartGame and logout packet branches invoke retirement before replacing the role", async () => {
  const logout = lifecycleCases.get("LogOutSuccess"), start = lifecycleCases.get("StartGame"); assert(logout && start);
  assert(containsCall(logout.statements[0], "retireEquipmentSession"));
  const resultBranch = start.statements.find(n => ts.isIfStatement(n) && n.elseStatement && containsCall(n.elseStatement, "suspendEquipmentConnection")); assert(resultBranch);
  const suspensionIndex = resultBranch.elseStatement.statements.findIndex(node => containsCall(node, "suspendEquipmentConnection"));
  assert.equal(suspensionIndex, 1);
  assert(containsCall(resultBranch.elseStatement.statements[0], "retireCharacterSelection"));
  for (const code of [`return function(){switch('LogOutSuccess'){${logout.getText(ast)}}}`, `return function()${resultBranch.elseStatement.getText(ast)}`]) {
    const h = fixture(); await h.prime();
    h.scope.equipmentHostSuspendReasonRef.current = "logoutPending";
    const run = compile(code, { ...h.scope, source: h.socket, connectionGeneration: 3, payload: { characters: [] },
      gatewayProtocolReadyRef: { current: true }, charactersRef: { current: [] }, selectedCharacterIndexRef: { current: 0 },
      retireCharacterSelection() {}, retireEquipmentSession: h.api.retireEquipmentSession, suspendEquipmentConnection: h.api.suspendEquipmentConnection });
    assert.throws(run, /STOP AFTER PRIVATE RETIREMENT/);
    assert.equal(h.client.snapshot().view, null); assert.equal(h.client.snapshot().download, null); assert.equal(h.scope.companionOpenRef.current, false);
  }
});
test("actual realm changes and feature revocation retire private data and never revive an old Source", async () => {
  for (const change of [{ worldEpoch: "world-2" }, { realmId: "trial-2" }, { features: { companionControl: false } }]) {
    const h = fixture(); await h.prime(); const old = h.api.captureCompanionSource(), oldInfo = h.scope.companionRealmRef.current.info;
    h.applyRealm({ type: "realmInfo", payload: { ...oldInfo, ...change } }, h.socket, 3);
    assert.equal(h.client.snapshot().view, null); assert.equal(h.client.snapshot().download, null); assert.deepEqual(h.revoked, ["blob:private-fixture"]);
    assert.equal(h.scope.companionOpenRef.current, false); assert(h.scope.companionContextEpochRef.current > old.contextEpoch);
    h.applyRealm({ type: "realmInfo", payload: oldInfo }, h.socket, 3);
    assert.equal(companion.sameR16CompanionSource(old, h.api.captureCompanionSource()), false);
  }
});
test("actual open callback requires authority and cancels existing world intent before exposing the window", () => {
  const h = fixture(); h.api.changeCompanionOpen(true); assert.deepEqual(h.trace, ["cancel-world-intent", "open:true"]);
  h.scope.screenRef.current = "login"; h.trace.length = 0; h.api.changeCompanionOpen(true); assert.deepEqual(h.trace, []);
  h.api.changeCompanionOpen(false); assert.deepEqual(h.trace, ["open:false"]);
});
test("actual message handler consumes private/invalid envelopes before Hero, raw ingress, world/history and logs", () => {
  const statements = messageHandlers[0].body.statements;
  assert(ts.isIfStatement(statements[0])); assert(containsCall(statements[1], "receivePrivateCompanionFrame"));
  assert(!statements.slice(0, 2).some(n => containsCall(n, "syncHeroRawIngress") || containsCall(n, "appendLog")));
  const cases = [
    JSON.stringify({ type: "companionResult", payload: { privateResult: "PRIVATE SENTINEL" } }),
    '{"payload":{"privateResult":"PRIVATE SENTINEL"},"type":"companionResult"}',
    '{"type":"companionResult","payload":',
    '{"type":"companionResult","payload":"' + "x".repeat(128 * 1024) + '"}',
  ];
  for (const data of cases) {
    const h = fixture(); h.handler({ data }); assert.deepEqual(h.trace, []); assert.deepEqual(h.history, []); assert.deepEqual(h.logs, []);
  }
  const h = fixture(); h.handler({ data: '{"type":"worldSnapshot","payload":{}}' });
  assert.deepEqual(h.trace, ["clock", "hero-sync", "hero-physical", "hero-capture", "generic-decode"]); assert.equal(h.history.length, 1);
});
test("escaped JSON type names are still private before the actual generic handler", () => {
  for (const type of ['companion\\u0052esult', '\\u0063ompanionResult']) {
    const h = fixture(); h.handler({ data: `{"type":"${type}","payload":{"privateResult":"PRIVATE SENTINEL"}}` });
    assert.deepEqual(h.trace, []); assert.deepEqual(h.history, []); assert.deepEqual(h.logs, []);
  }
});
test("actual private receive drops foreign sockets/generations and absorbs decoder exceptions", () => {
  const h = fixture(), data = '{"type":"companionResult","payload":{"privateResult":"PRIVATE SENTINEL"}}';
  assert.equal(h.api.receivePrivateCompanionFrame(data, {}, 3), true); assert.equal(h.received.length, 0);
  assert.equal(h.api.receivePrivateCompanionFrame(data, h.socket, 4), true); assert.equal(h.received.length, 0);
  h.client.receive = () => { throw new Error("PRIVATE DECODER SENTINEL"); };
  assert.equal(h.api.receivePrivateCompanionFrame(data, h.socket, 3), true); assert.deepEqual(h.logs, []);
  h.scope.screenRef.current = "login"; assert.equal(h.api.receivePrivateCompanionFrame(data, h.socket, 3), true);
});
test("actual party invitation repeats lease/character/source checks between SwitchGroup and AddMember", async () => {
  for (const mutate of [null, h => { h.scope.companionContextEpochRef.current++; }, h => { h.scope.socialCharacterIndexRef.current.current++; },
    h => { h.scope.socketRef.current = null; }, h => { h.client.retire(); }]) {
    const h = fixture(); await h.prime(); const lease = h.client.snapshot().renderLease;
    if (mutate) h.onGroupSend(command => { if (command.type === "switchGroup") mutate(h); });
    assert.equal(h.api.inviteCompanionToParty(lease, "AloAI"), !mutate);
    assert.deepEqual(h.groups.map(g => g.command.type), mutate ? ["switchGroup"] : ["switchGroup", "addMember"]);
  }
  const h = fixture(); await h.prime(); assert.equal(h.api.inviteCompanionToParty(h.client.snapshot().renderLease, "foreign name"), false); assert.deepEqual(h.groups, []);
});
test("actual sendRaw rejects all three private command kinds before evaluating any generic globals", () => {
  const first = declarations.get("sendRaw").body.statements[0]; assert(ts.isIfStatement(first));
  const raw = compile(declarations.get("sendRaw").getText(ast) + "\nreturn sendRaw;", {});
  for (const type of ["companionControl", "companionRuntimeRegister", "companionRuntimeReply"]) {
    const command = new Proxy({ type }, { get(target, key) { assert.equal(key, "type", "no payload read by the general path"); return target[key]; } });
    assert.equal(raw(command), false, "all undeclared generic globals remain unreachable");
  }
  assert.throws(() => raw({ type: "walk" }), ReferenceError, "ordinary routes retain their real guards rather than being stubbed out");
});
