import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import ts from "typescript";

const source = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const tree = ts.createSourceFile("page.tsx", source, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const functions = new Map();
let logoutCase;
function visit(node) {
  if (ts.isFunctionDeclaration(node)) functions.set(node.name?.text, node);
  if (ts.isCaseClause(node) && node.expression.getText(tree) === '"LogOutSuccess"') logoutCase = node;
  ts.forEachChild(node, visit);
}
visit(tree);
assert(logoutCase && functions.has("handleGatewayEvent"));
const sourceGuard = functions.get("handleGatewayEvent").body.statements[0].getText(tree);
assert.match(sourceGuard, /connectionGeneration.*socketRef/);
function compile(code, scope) {
  const output = ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText;
  return new Function(...Object.keys(scope), output)(...Object.values(scope));
}
const character = index => ({ index, name: "Role" + index, class: "Warrior", level: 30 });

function fixture() {
  const calls = [];
  const socket = { readyState: 1, close() { calls.push("socket-close"); this.readyState = 3; } };
  const auth = Object.freeze({ kind: "sui", accountId: "fixture", token: "fixture-session" });
  const scope = {
    WebSocket: { OPEN: 1 }, socketRef: { current: socket }, gatewayProtocolReadyRef: { current: true },
    equipmentConnectionGenerationRef: { current: 3 }, equipmentSessionGenerationRef: { current: 8 },
    equipmentStartGameRef: { current: { connectionGeneration: 3, sessionGeneration: 8 } },
    equipmentHostSuspendReasonRef: { current: "logoutPending" }, screenRef: { current: "game" },
    characterSelectionRef: { current: null }, characterCreateRef: { current: null },
    charactersRef: { current: [character(4), character(7)] }, selectedCharacterIndexRef: { current: 1 },
    socialCharacterIndexRef: { current: { requested: null, current: 7 } }, accountIdRef: { current: "fixture" },
    activeReconnectAuthRef: { current: auth }, hudProjectionRef: { current: false },
    questDefinitionByIdRef: { current: new Map([[1, {}]]) }, questAuthoritativeStageByIdRef: { current: new Map([[1, {}]]) },
    DEFAULT_WORLD_STATE: { connected: false, playerObjectId: null, entities: [], inventoryItems: [] },
    world: { connected: true, playerObjectId: 42, entities: [{ objectId: 42 }], inventoryItems: [{ uniqueId: 99 }], mapFileName: "0" },
    language: "zh-CN", token: "fixture-session", identityOpen: true, characterTab: "spells", inventoryTab: "bag4",
    retireCharacterSelection() { calls.push("selection-retired"); scope.characterSelectionRef.current = null; },
    retireEquipmentSession() {
      calls.push("role-retired");
      scope.equipmentStartGameRef.current = null;
      scope.socialCharacterIndexRef.current = { requested: null, current: null };
      scope.equipmentHostSuspendReasonRef.current = "connectionUnavailable";
    },
    clearPendingQuestActions() { calls.push("quests-retired"); },
    cancelPendingEntitySounds() { calls.push("sounds-retired"); },
    resetGatewayReconnectState() { calls.push("reconnect-reset"); },
    closeTouchSecondaryWindows(except, force) {
      assert.equal(except, undefined); assert.equal(force, true);
      assert.equal(scope.hudProjectionRef.current, true, "authoritative close applies directly to React state");
      calls.push("windows-closed");
    },
    setIdentitySessionToken(value) { scope.token = value; },
    setIdentitySecurityOpen(value) { scope.identityOpen = value; },
    setActiveInventoryTab(value) { scope.inventoryTab = value; },
    setActiveCharacterTab(value) { scope.characterTab = value; },
    setLoginBusy(value) { assert.equal(value, false); },
    setLoginErrorKey(value) { scope.notice = value; },
    parseCharacters(payload) {
      calls.push("roster-parsed");
      return payload.characters.map(row => ({ ...row, index: row.index ?? row.Index }));
    },
    setCharacters(value) { scope.renderedCharacters = value; },
    setSelectedCharacterIndex(value) { scope.renderedIndex = value; },
    setScreen(value) { scope.renderedScreen = value; },
    updateWorld(update) { scope.world = update(scope.world); },
    markMir2CacheMilestone() {}, send(command) { calls.push(command); return true; },
  };
  const handle = compile(`return function(event, connectionGeneration, source) {
    ${sourceGuard}
    const payload = event.payload ?? {};
    switch(event.packet) { ${logoutCase.getText(tree)} }
  };`, scope);
  const selection = compile(functions.get("currentCharacterSelection").getText(tree) + "\nreturn currentCharacterSelection;", scope);
  const start = () => compile(functions.get("startSelectedCharacter").getText(tree) + "\nreturn startSelectedCharacter;", {
    ...scope, characters: scope.renderedCharacters ?? scope.charactersRef.current,
    selectedCharacterIndex: scope.renderedIndex ?? scope.selectedCharacterIndexRef.current,
    currentCharacterSelection: selection,
  })();
  const apply = (roster = [character(4), character(7)], from = socket, generation = 3) =>
    handle({ packet: "LogOutSuccess", payload: { characters: roster } }, generation, from);
  return { scope, socket, auth, calls, apply, handle, selection, start };
}

test("current logout returns to authenticated selection and reuses the active role ID", () => {
  const f = fixture();
  f.apply([character(4), character(12), character(7)]);
  assert.equal(f.scope.renderedScreen, "select");
  assert.equal(f.scope.selectedCharacterIndexRef.current, 2);
  assert.equal(f.scope.renderedIndex, 2);
  assert.equal(f.scope.charactersRef.current, f.scope.renderedCharacters);
  assert.equal(f.selection().socket, f.socket);
  assert.equal(f.scope.activeReconnectAuthRef.current, f.auth);
  assert.equal(f.scope.token, "fixture-session");
  assert.equal(f.scope.identityOpen, false);
  assert.equal(f.scope.characterTab, "char");
  assert.equal(f.scope.inventoryTab, "bag1");
  assert.equal(f.scope.equipmentStartGameRef.current, null);
  assert.equal(f.scope.world.playerObjectId, null);
  assert.deepEqual(f.scope.world.inventoryItems, []);
  assert.equal(f.scope.questDefinitionByIdRef.current.size, 0);
  assert.equal(f.scope.hudProjectionRef.current, false);
  assert(f.calls.indexOf("role-retired") < f.calls.indexOf("roster-parsed"));
  f.start();
  assert.deepEqual(f.calls.at(-1), { type: "startGame", characterIndex: 7 });
});

test("logout selects the first remaining role when the active ID disappeared, and preserves an empty roster", () => {
  const removed = fixture(); removed.apply([character(12), character(4)]); removed.start();
  assert.equal(removed.scope.selectedCharacterIndexRef.current, 0);
  assert.deepEqual(removed.calls.at(-1), { type: "startGame", characterIndex: 12 });
  const empty = fixture(); empty.apply([]); const before = empty.calls.length; empty.start();
  assert.deepEqual(empty.scope.charactersRef.current, []);
  assert.equal(empty.calls.length, before);
  assert.equal(empty.selection().socket, empty.socket);
});

test("foreign, stale, unrequested and malformed logout packets never grant a selection owner", () => {
  const mutations = [
    f => { f.scope.equipmentConnectionGenerationRef.current++; },
    f => { f.scope.equipmentSessionGenerationRef.current++; },
    f => { f.scope.equipmentStartGameRef.current = null; },
    f => { f.scope.equipmentHostSuspendReasonRef.current = null; },
    f => { f.scope.gatewayProtocolReadyRef.current = false; },
    f => { f.socket.readyState = 3; },
    f => { f.scope.screenRef.current = "login"; },
  ];
  for (const mutate of mutations) {
    const f = fixture(); mutate(f); f.apply(); assert.equal(f.scope.characterSelectionRef.current, null); assert.deepEqual(f.calls, []);
  }
  const foreign = fixture(); foreign.apply(undefined, { readyState: 1 }); assert.deepEqual(foreign.calls, []);
  for (const roster of [null, {}, [null], [{}], [character(-1)], [character(1.5)], [character(7), character(7)], [{ index: 7 }]]) {
    const f = fixture(); f.apply(roster); assert.deepEqual(f.calls, []); assert.equal(f.selection(), null);
  }
});

test("a duplicate logout cannot replace the returned selection or retire a newer game session", () => {
  const f = fixture(); f.apply(); const owner = f.selection(), count = f.calls.length;
  f.apply([]); assert.equal(f.selection(), owner); assert.equal(f.calls.length, count);
  f.scope.screenRef.current = "game";
  f.scope.equipmentSessionGenerationRef.current++;
  f.scope.equipmentStartGameRef.current = { connectionGeneration: 3, sessionGeneration: 9 };
  f.scope.equipmentHostSuspendReasonRef.current = null;
  f.apply([]); assert.equal(f.calls.length, count); assert.equal(f.scope.equipmentStartGameRef.current.sessionGeneration, 9);
});

test("selection authority expires with its socket or connection generation", () => {
  for (const mutate of [f => { f.socket.readyState = 3; }, f => { f.scope.equipmentConnectionGenerationRef.current++; },
    f => { f.scope.socketRef.current = { readyState: 1 }; }]) {
    const f = fixture(); f.apply(); mutate(f); const before = f.calls.length; f.start();
    assert.equal(f.selection(), null); assert.equal(f.calls.length, before);
  }
});
