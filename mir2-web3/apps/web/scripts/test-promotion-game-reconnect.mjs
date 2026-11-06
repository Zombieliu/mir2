import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";
import ts from "typescript";

// Execute the real page handlers without importing the game's rendering tree.
// The broker and preauth transport are controlled fixtures; no network is used.
function functionsFrom(file, kind) {
  const source = readFileSync(new URL(file, import.meta.url), "utf8");
  const ast = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, kind);
  const functions = new Map();
  function visit(node) {
    if (ts.isFunctionDeclaration(node) && node.name) functions.set(node.name.text, node.getText(ast));
    ts.forEachChild(node, visit);
  }
  visit(ast);
  return functions;
}
const pageFunctions = functionsFrom("../app/page.tsx", ts.ScriptKind.TSX);
const realmFunctions = functionsFrom("../lib/promotion-game-realm.ts", ts.ScriptKind.TS);
function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
const drain = () => new Promise(resolve => setImmediate(resolve));
const freshLogin = () => ({
  accountId: "original:player", provider: "suiWallet",
  token: "fresh-synthetic-gameplay-grant", expiresAt: Date.now() + 60_000,
});

function harness({ pathname = "/promotions/play", screen = "game" } = {}) {
  const auth = { kind: "sui", accountId: "original:player", token: "expired-synthetic-grant", expiresAt: 1 };
  const snapshot = { auth, characterIndex: 2, characterName: "Original player" };
  const state = {
    window: { location: { pathname } }, document: { visibilityState: "visible" },
    loginAuthRef: { current: { epoch: 7, surface: "login" } }, oauthIntentRef: { current: 3 },
    socketRef: { current: { readyState: 1 } }, equipmentConnectionGenerationRef: { current: 9 },
    activeReconnectAuthRef: { current: auth }, screenRef: { current: screen },
    reconnectSnapshotRef: { current: snapshot }, gatewayProtocolReadyRef: { current: true },
    loginBusyRef: { current: false }, accountIdRef: { current: "" },
    snapshot, requests: [], attempts: [], commands: [], busy: [], errors: [],
    failures: 0, resets: 0, gateAllows: true,
  };
  const names = ["setLoginBusy", "authSurfaceCurrent", "captureIdentityIntent", "identityIntentCurrent",
    "sendGatewayReconnectSequence", "submitSuiLogin", "submitIdentitySession", "submitDubheQrLogin"];
  for (const name of names) assert.ok(pageFunctions.has(name), "actual page function " + name);
  const request = kind => {
    const pending = deferred();
    state.requests.push({ kind, ...pending });
    return pending.promise;
  };
  const context = vm.createContext({
    ...state, Object, exports: {}, WebSocket: { OPEN: 1 },
    requestPromotionGameLogin: () => request("promotion"),
    requestSuiLoginToken: () => request("ordinary"),
    requestDubheWalletLoginToken: () => { throw Error("Promotion entry reached the ordinary QR broker"); },
    setLoginBusyState: value => state.busy.push(value),
    setLoginErrorKey: value => state.errors.push(value),
    setWalletPickerOpen: () => {},
    resetGatewayReconnectState: () => { state.resets++; state.reconnectSnapshotRef.current = null; },
    setAccountId: () => {}, setIdentityProvider: () => {}, markMir2CacheMilestone: () => {},
    failGatewayReconnect: () => { state.failures++; },
    sendPreauthAttempt: attempt => { state.attempts.push(attempt); return state.gateAllows; },
    queuePreauthAttempt: attempt => { state.attempts.push(attempt); return state.gateAllows; },
    send: command => { state.commands.push(command); return true; },
  });
  const program = ts.transpileModule(
    realmFunctions.get("isPromotionGamePath") + "\n" + names.map(name => pageFunctions.get(name)).join("\n")
      + "\nglobalThis.handlers = { sendGatewayReconnectSequence, submitSuiLogin, submitDubheQrLogin };",
    { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS } },
  );
  vm.runInContext(program.outputText.replace(/exports\.isPromotionGamePath = isPromotionGamePath;?/, ""), context);
  state.handlers = context.handlers;
  return state;
}

test("promotion reconnect refreshes its grant and starts only after the existing preauth gate accepts", async () => {
  const f = harness();
  f.handlers.sendGatewayReconnectSequence(f.snapshot);
  assert.deepEqual(f.requests.map(row => row.kind), ["promotion"]);
  const login = freshLogin();
  f.requests[0].resolve(login);
  await drain();
  assert.equal(f.attempts.length, 1);
  assert.equal(f.attempts[0].epoch, 7);
  assert.equal(f.attempts[0].reconnect, true);
  assert.deepEqual({ ...f.attempts[0].command }, { type: "passkeyLogin", accountId: login.accountId, token: login.token });
  assert.deepEqual(f.commands.map(row => ({ ...row })), [{ type: "startGame", characterIndex: 2 }]);
  assert.equal(f.activeReconnectAuthRef.current.token, login.token);
  assert.equal(f.failures, 0);
});

const retirements = [
  f => { f.loginAuthRef.current.epoch++; },
  f => { f.oauthIntentRef.current++; },
  f => { f.socketRef.current = { readyState: 1 }; },
  f => { f.equipmentConnectionGenerationRef.current++; },
  f => { f.activeReconnectAuthRef.current = { kind: "different-identity" }; },
  f => { f.screenRef.current = "login"; },
  f => { f.reconnectSnapshotRef.current = { ...f.snapshot }; },
  f => { f.socketRef.current.readyState = 3; },
  f => { f.gatewayProtocolReadyRef.current = false; },
];
test("a retired promotion reconnect cannot send a grant or start a character", async () => {
  for (const retire of retirements) {
    const f = harness();
    f.handlers.sendGatewayReconnectSequence(f.snapshot);
    retire(f);
    f.requests[0].resolve(freshLogin());
    await drain();
    assert.equal(f.attempts.length, 0);
    assert.equal(f.commands.length, 0);
  }
});

test("late broker failure cannot fail a replacement identity or reconnect snapshot", async () => {
  for (const retire of retirements.slice(0, 7)) {
    const f = harness();
    f.handlers.sendGatewayReconnectSequence(f.snapshot);
    retire(f);
    f.requests[0].reject(new Error("retired broker failure"));
    await drain();
    assert.equal(f.failures, 0);
  }
});

test("a foreign account or rejected preauth gate never starts the requested character", async () => {
  const foreign = harness();
  foreign.handlers.sendGatewayReconnectSequence(foreign.snapshot);
  foreign.requests[0].resolve({ ...freshLogin(), accountId: "another:player" });
  await drain();
  assert.equal(foreign.failures, 1);
  assert.equal(foreign.attempts.length, 0);
  assert.equal(foreign.commands.length, 0);
  const blocked = harness();
  blocked.gateAllows = false;
  blocked.handlers.sendGatewayReconnectSequence(blocked.snapshot);
  blocked.requests[0].resolve(freshLogin());
  await drain();
  assert.equal(blocked.attempts.length, 1);
  assert.equal(blocked.commands.length, 0);
});

test("concurrent refresh responses cannot replay the reconnect authentication", async () => {
  const f = harness();
  f.handlers.sendGatewayReconnectSequence(f.snapshot);
  f.handlers.sendGatewayReconnectSequence(f.snapshot);
  f.requests[0].resolve(freshLogin());
  await drain();
  f.requests[1].resolve(freshLogin());
  await drain();
  assert.equal(f.attempts.length, 1);
  assert.equal(f.commands.length, 1);
});

test("ordinary game reconnect keeps its existing token and never calls the promotion broker", () => {
  const f = harness({ pathname: "/game" });
  f.handlers.sendGatewayReconnectSequence(f.snapshot);
  assert.equal(f.requests.length, 0);
  assert.equal(f.attempts.length, 1);
  assert.equal(f.attempts[0].command.token, "expired-synthetic-grant");
  assert.equal(f.commands.length, 1);
});

test("promotion login and the new QR entry both use the server gameplay broker", async () => {
  for (const entry of ["submitSuiLogin", "submitDubheQrLogin"]) {
    const f = harness({ screen: "login" });
    const pending = f.handlers[entry]("wallet", "client-selected-wallet");
    assert.deepEqual(f.requests.map(row => row.kind), ["promotion"]);
    f.requests[0].resolve(freshLogin());
    await pending;
    assert.equal(f.attempts.length, 1);
    assert.equal(f.attempts[0].epoch, 7);
    assert.equal(f.attempts[0].command.accountId, "original:player");
    assert.equal(f.attempts[0].command.token, "fresh-synthetic-gameplay-grant");
  }
});

test("retired promotion login cannot clear a newer busy state or submit identity", async () => {
  for (const retire of retirements.slice(0, 6)) {
    const f = harness({ screen: "login" });
    const pending = f.handlers.submitSuiLogin("wallet");
    // A changed auth surface owns any subsequent busy state.
    retire(f);
    f.screenRef.current = "registration";
    f.requests[0].resolve(freshLogin());
    await pending;
    assert.equal(f.attempts.length, 0);
    assert.equal(f.loginBusyRef.current, true);
  }
});
