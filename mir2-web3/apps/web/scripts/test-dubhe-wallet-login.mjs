import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import { test } from "node:test";
import vm from "node:vm";

import ts from "typescript";

const clientSource = readFileSync(new URL("../lib/dubhe-wallet-login.ts", import.meta.url), "utf8");
const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");

function compile(source) {
  return ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
  }).outputText;
}

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

const flush = () => new Promise((resolve) => setImmediate(resolve));

function clientHarness() {
  const state = {
    now: Date.now(), current: true, controller: new AbortController(),
    document: { visibilityState: "visible" }, states: [], requests: [], exchanges: [],
    timers: new Map(), timerId: 0, onState: null,
  };
  state.created = {
    sessionId: "session-123", approvalUrl: "https://wallet.dubhe.obelisk.build/connect/approve?session=session-123",
    expiresAt: state.now + 120_000, intentToken: "private-browser-intent",
  };
  state.proof = {
    accountId: "sui:0x123", provider: "suiWallet", token: "verified-wallet-proof",
    expiresAt: state.created.expiresAt,
  };
  state.login = {
    accountId: "player:canonical-account", playerId: "canonical-player", provider: "suiWallet",
    token: "canonical-channel-session", expiresAt: state.now + 3_600_000,
  };
  state.fetch = async (url) => Response.json(url.endsWith("/status")
    ? { status: "approved", login: state.proof } : state.created);
  state.exchange = async () => state.login;
  const schedule = (callback, ms) => {
    const id = ++state.timerId;
    state.timers.set(id, { at: state.now + Math.max(0, ms), callback });
    state.onTimer?.(ms);
    return id;
  };
  const module = { exports: {} };
  const context = vm.createContext({
    module, exports: module.exports, URL, Response, DOMException, TypeError,
    AbortSignal: {
      any: signals => AbortSignal.any(signals),
      timeout(ms) {
        const controller = new AbortController();
        schedule(() => controller.abort(new DOMException("Test request deadline", "TimeoutError")), ms);
        return controller.signal;
      },
    },
    document: state.document,
    Date: class extends Date { static now() { return state.now; } },
    setTimeout: schedule,
    clearTimeout: (id) => state.timers.delete(id),
    fetch: async (...args) => { state.requests.push(args); return state.fetch(...args); },
    require: (name) => {
      assert.equal(name, "./passkey-auth");
      return { requestChannelSessionToken: async (...args) => {
        state.exchanges.push(args);
        return state.exchange(...args);
      } };
    },
  });
  vm.runInContext(compile(clientSource), context);
  state.run = () => module.exports.requestDubheWalletLoginToken({
    signal: state.controller.signal, isCurrent: () => state.current,
    onState: (value) => { state.states.push(value); state.onState?.(value); },
  });
  state.tick = async (ms) => {
    state.now += ms;
    const due = [...state.timers].filter(([, timer]) => timer.at <= state.now);
    for (const [id, timer] of due) {
      state.timers.delete(id);
      timer.callback();
    }
    await flush();
  };
  return state;
}

test("Dubhe approval exchanges the proof once for the canonical channel account", async () => {
  const state = clientHarness();
  const login = await state.run();
  assert.deepEqual(login, state.login);
  assert.equal(state.exchanges.length, 1);
  assert.equal(state.exchanges[0][0], "suiWallet");
  assert.equal(state.exchanges[0][1], state.proof.token);
  assert.equal(state.exchanges[0][2], state.controller.signal);
  assert.deepEqual(state.states.map((value) => value.status), ["pending", "approved"]);
  assert.equal(state.requests.length, 2);
  assert.equal(state.requests[1][1].headers.authorization, `Bearer ${state.created.intentToken}`);
  assert.equal(state.requests[0][1].credentials, "same-origin");
  assert.equal(state.requests[0][1].cache, "no-store");
  assert.ok(!state.states.some((value) => JSON.stringify(value).includes(state.created.intentToken)));
});

test("wallet rejection and expiry never exchange an identity proof", async () => {
  for (const status of ["rejected", "expired"]) {
    const state = clientHarness();
    state.fetch = async (url) => Response.json(url.endsWith("/status") ? { status } : state.created);
    assert.equal(await state.run(), null);
    assert.equal(state.exchanges.length, 0);
    assert.equal(state.states.at(-1).status, status);
  }
});

test("aborted or superseded create, poll, approved callback, and exchange cannot return a login", async () => {
  for (const stage of ["create", "poll", "approved", "exchange"]) {
    for (const retire of ["abort", "stale"]) {
      const state = clientHarness();
      const pending = deferred();
      const invalidate = () => retire === "abort" ? state.controller.abort() : (state.current = false);
      state.fetch = async (url) => {
        if (stage === "create" && !url.endsWith("/status")) return pending.promise;
        if (stage === "poll" && url.endsWith("/status")) return pending.promise;
        return Response.json(url.endsWith("/status")
          ? { status: "approved", login: state.proof } : state.created);
      };
      if (stage === "approved") state.onState = (value) => { if (value.status === "approved") invalidate(); };
      if (stage === "exchange") state.exchange = () => pending.promise;
      const result = state.run();
      const rejected = assert.rejects(result, (error) => error.name === "AbortError");
      await flush();
      if (stage !== "approved") {
        invalidate();
        pending.resolve(stage === "exchange" ? state.login : Response.json(stage === "create"
          ? state.created : { status: "approved", login: state.proof }));
      }
      await rejected;
      assert.equal(state.exchanges.length, stage === "exchange" ? 1 : 0, `${stage}/${retire}`);
    }
  }
});

test("a slow HTTP 200 create can finish, while a stalled body retries once unless cancelled or retired", { timeout: 10_000 }, async () => {
  for (const mode of ["slow", "timeout", "cancel", "stale"]) {
    const state = clientHarness(), firstHeaders = deferred(), retryDelay = deferred();
    let creates = 0;
    let finishFirstBody;
    const server = createServer((request, response) => {
      if (request.url.endsWith("/status")) {
        response.end(JSON.stringify({ status: "approved", login: state.proof }));
      } else if (++creates === 1) {
        response.writeHead(200, { "content-type": "application/json" });
        response.flushHeaders();
        const json = JSON.stringify(state.created);
        response.write(json.slice(0, 18));
        finishFirstBody = () => response.end(json.slice(18));
      } else {
        response.end(JSON.stringify(state.created));
      }
    });
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(0, "127.0.0.1", resolve);
    });
    try {
      const origin = `http://127.0.0.1:${server.address().port}`;
      state.fetch = async (url, init) => {
        const response = await fetch(origin + url, init);
        if (state.requests.length === 1) firstHeaders.resolve(response.status);
        return response;
      };
      state.onTimer = ms => { if (ms === 250) retryDelay.resolve("retry"); };
      const result = state.run();
      const settled = result.then(() => "settled", () => "settled");
      assert.equal(await Promise.race([firstHeaders.promise, settled]), 200);
      await flush();
      assert.deepEqual(state.states, [], "headers alone cannot publish a code");
      if (mode === "slow") {
        await state.tick(13_309);
        finishFirstBody();
        assert.deepEqual(await result, state.login);
        assert.equal(creates, 1);
        assert.equal(state.exchanges.length, 1);
        continue;
      }
      if (mode === "cancel") state.controller.abort();
      if (mode === "stale") state.current = false;
      if (mode !== "cancel") await state.tick(20_000);
      if (mode === "timeout") {
        assert.equal(await Promise.race([retryDelay.promise, settled]), "retry");
        assert.equal(state.controller.signal.aborted, false);
        assert.equal(creates, 1);
        await state.tick(250);
        assert.deepEqual(await result, state.login);
        assert.equal(creates, 2);
        assert.equal(state.requests.length, 3);
        assert.equal(state.exchanges.length, 1);
      } else {
        await assert.rejects(result, { name: "AbortError" });
        await state.tick(250);
        assert.equal(creates, 1);
        assert.equal(state.requests.length, 1);
        assert.equal(state.exchanges.length, 0);
      }
    } finally {
      server.closeAllConnections();
      await new Promise(resolve => server.close(resolve));
    }
  }
});

test("a transient poll retries the same intent and keeps its code, visibility and expiry", async () => {
  for (const mode of ["visible", "hidden", "expired"]) {
    const state = clientHarness();
    let polls = 0;
    if (mode === "expired") state.created.expiresAt = state.now + 100;
    state.fetch = async url => {
      if (!url.endsWith("/status")) return Response.json(state.created);
      if (++polls === 1) throw new TypeError("Temporary poll transport failure");
      return Response.json({ status: "approved", login: state.proof });
    };
    const result = state.run();
    await flush();
    assert.equal(polls, 1);
    assert.deepEqual(state.states.map(value => value.status), ["pending"]);
    if (mode === "hidden") state.document.visibilityState = "hidden";
    await state.tick(250);
    if (mode === "expired") {
      assert.equal(await result, null);
      assert.equal(state.states.at(-1).status, "expired");
      assert.equal(polls, 1);
      assert.equal(state.exchanges.length, 0);
      continue;
    }
    if (mode === "hidden") {
      assert.equal(polls, 1);
      state.document.visibilityState = "visible";
      await state.tick(250);
    }
    assert.deepEqual(await result, state.login);
    assert.equal(polls, 2);
    assert.deepEqual(state.states.map(value => value.status), ["pending", "approved"]);
    assert.equal(state.requests.length, 3);
    for (const [, request] of state.requests.slice(1)) {
      assert.equal(request.headers.authorization, `Bearer ${state.created.intentToken}`);
      assert.equal(request.body, undefined);
    }
    assert.equal(state.exchanges.length, 1);
  }
});

test("QR retries are bounded, abortable, and never retry auth responses or exchange", async () => {
  for (const mode of ["repeat", "cancel", "stale"]) {
    const state = clientHarness();
    state.fetch = async () => { throw new TypeError("Transport unavailable"); };
    const result = state.run();
    const rejected = assert.rejects(result, { name: mode === "repeat" ? "TypeError" : "AbortError" });
    await flush();
    if (mode === "cancel") state.controller.abort();
    if (mode === "stale") state.current = false;
    await state.tick(250);
    await rejected;
    assert.equal(state.requests.length, mode === "repeat" ? 2 : 1);
  }
  for (const response of [new Response("Unauthorized", { status: 403 }), new Response('{"approvalUrl":')]) {
    const state = clientHarness();
    state.fetch = async () => response;
    await assert.rejects(state.run());
    assert.equal(state.requests.length, 1);
  }
  const state = clientHarness();
  state.exchange = async () => { throw new TypeError("Exchange outcome unknown"); };
  await assert.rejects(state.run(), { name: "TypeError" });
  assert.equal(state.exchanges.length, 1);
});

test("local expiry stops polling without an extra request", async () => {
  const state = clientHarness();
  state.created.expiresAt = state.now + 1_000;
  state.fetch = async (url) => Response.json(url.endsWith("/status") ? { status: "pending" } : state.created);
  const result = state.run();
  await flush();
  assert.equal(state.requests.length, 2);
  await state.tick(1_000);
  assert.equal(await result, null);
  assert.equal(state.requests.length, 2);
  assert.equal(state.exchanges.length, 0);
  assert.equal(state.states.at(-1).status, "expired");
});

test("same-device hidden tabs wait for return before polling or claiming an approved login", async () => {
  const state = clientHarness();
  state.document.visibilityState = "hidden";
  const result = state.run();
  await flush();
  assert.equal(state.requests.length, 1);
  state.document.visibilityState = "visible";
  state.onState = (value) => { if (value.status === "approved") state.document.visibilityState = "hidden"; };
  await state.tick(250);
  assert.equal(state.requests.length, 2);
  assert.equal(state.exchanges.length, 0);
  state.document.visibilityState = "visible";
  await state.tick(250);
  assert.deepEqual(await result, state.login);
  assert.equal(state.exchanges.length, 1);
});

test("hidden tabs expire without polling and cancellation stops a hidden wait", async () => {
  for (const abort of [false, true]) {
    const state = clientHarness();
    state.created.expiresAt = state.now + 500;
    state.document.visibilityState = "hidden";
    const result = state.run();
    const settled = abort ? assert.rejects(result, (error) => error.name === "AbortError") : result;
    await flush();
    if (abort) state.controller.abort();
    else await state.tick(500);
    assert.equal(await settled, abort ? undefined : null);
    assert.equal(state.requests.length, 1);
    assert.equal(state.exchanges.length, 0);
  }
});

test("malformed create, status, and approved proof responses fail without exchange", async () => {
  const mutations = [
    (state) => { state.created.intentToken = ""; },
    (state) => { state.created.expiresAt = state.now; },
    (state) => { state.created.expiresAt = state.now + 300_001; },
    (state) => { state.created.approvalUrl = "http://insecure.example/approve"; },
    (state) => { state.created.approvalUrl = "not-a-url"; },
    (state) => { state.proof.provider = "suiPasskey"; },
    (state) => { state.proof.token = ""; },
    (state) => { state.proof.accountId = "another:account"; },
    (state) => { state.proof.expiresAt = state.now - 1; },
    (state) => { state.proof.expiresAt = state.created.expiresAt + 1; },
    (state) => { state.fetch = async () => Response.json([]); },
    (state) => { state.fetch = async () => new Response("error", { status: 500 }); },
    (state) => { state.fetch = async (url) => Response.json(url.endsWith("/status") ? { status: "unknown" } : state.created); },
  ];
  for (const mutate of mutations) {
    const state = clientHarness();
    mutate(state);
    await assert.rejects(state.run());
    assert.equal(state.exchanges.length, 0);
  }
});

// Exercise the actual page handlers and lease comparisons without importing the
// 20,000-line component or unrelated storage/runtime test loaders.
const pageAst = ts.createSourceFile("page.tsx", pageSource, ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
const pageFunctions = new Map();
let lifecycle;
function visit(node) {
  if (ts.isFunctionDeclaration(node) && node.name) pageFunctions.set(node.name.text, node.getText(pageAst));
  if (ts.isExpressionStatement(node) && ts.isCallExpression(node.expression)
    && node.expression.expression.getText(pageAst) === "useEffect"
    && node.getText(pageAst).includes("dubheQrAbortRef.current?.abort()")) lifecycle = node.getText(pageAst);
  ts.forEachChild(node, visit);
}
visit(pageAst);

function pageHarness() {
  const names = ["setLoginBusy", "authSurfaceCurrent", "captureIdentityIntent", "identityIntentCurrent",
    "cancelDubheQrLogin", "submitDubheQrLogin", "submitIdentitySession"];
  for (const name of names) assert.ok(pageFunctions.has(name), `actual page function ${name}`);
  assert.ok(lifecycle, "actual QR useEffect lifecycle");
  const state = {
    document: { visibilityState: "visible" }, loginBusyRef: { current: false },
    loginAuthRef: { current: { epoch: 1, surface: "login" } }, oauthIntentRef: { current: 0 },
    socketRef: { current: { id: "socket-1" } }, equipmentConnectionGenerationRef: { current: 1 },
    activeReconnectAuthRef: { current: null }, screenRef: { current: "login" },
    dubheQrAbortRef: { current: null }, accountIdRef: { current: "" },
    requests: [], commands: [], busy: [], qr: null, errors: [], cleanup: null,
    screen: "login", loginAuthState: { epoch: 1 },
  };
  const context = vm.createContext({
    ...state, AbortController, Object,
    window: { location: { pathname: "/game" } },
    isPromotionGamePath: () => false,
    requestDubheWalletLoginToken: (options) => {
      const pending = deferred();
      state.requests.push({ ...pending, options });
      return pending.promise;
    },
    setLoginBusyState: (value) => state.busy.push(value),
    setDubheQr: (value) => { state.qr = value; },
    setLoginErrorKey: (value) => state.errors.push(value),
    setWalletPickerOpen: () => {}, resetGatewayReconnectState: () => {},
    setAccountId: () => {}, setIdentityProvider: () => {}, markMir2CacheMilestone: () => {},
    queuePreauthAttempt: (attempt) => { state.commands.push(attempt.command); return true; },
    useEffect: (effect) => { state.cleanup = effect(); },
  });
  vm.runInContext(compile(`${names.map((name) => pageFunctions.get(name)).join("\n")}
    function installLifecycle() { ${lifecycle} }
    globalThis.handlers = { submitDubheQrLogin, cancelDubheQrLogin, installLifecycle };`), context);
  state.handlers = context.handlers;
  state.handlers.installLifecycle();
  return state;
}

const canonicalLogin = {
  accountId: "player:canonical", provider: "suiWallet", token: "channel-session", expiresAt: Date.now() + 3_600_000,
};

test("actual page submits the canonical channel session once", async () => {
  const state = pageHarness();
  const result = state.handlers.submitDubheQrLogin();
  assert.equal(state.loginBusyRef.current, true);
  state.requests[0].resolve(canonicalLogin);
  await result;
  assert.equal(state.commands.length, 1);
  assert.deepEqual(JSON.parse(JSON.stringify(state.commands[0])), {
    type: "passkeyLogin", accountId: canonicalLogin.accountId, token: canonicalLogin.token,
  });
  assert.equal(state.qr, null);
});

test("actual page cancel and retry retire old requests without clearing the newer busy flag", async () => {
  const state = pageHarness();
  const first = state.handlers.submitDubheQrLogin();
  const firstRequest = state.requests[0];
  state.handlers.cancelDubheQrLogin();
  assert.equal(firstRequest.options.signal.aborted, true);
  const second = state.handlers.submitDubheQrLogin();
  const newerOperation = state.dubheQrAbortRef.current;
  firstRequest.options.onState({ status: "approved" });
  firstRequest.resolve(canonicalLogin);
  await first;
  assert.equal(state.commands.length, 0);
  assert.equal(state.loginBusyRef.current, true);
  assert.equal(state.dubheQrAbortRef.current, newerOperation);
  assert.equal(state.qr.status, "creating");
  state.requests[1].resolve(null);
  await second;
  assert.equal(state.loginBusyRef.current, false);
});

test("actual page QR retry also ignores an older rejected promise", async () => {
  const state = pageHarness();
  const first = state.handlers.submitDubheQrLogin();
  const second = state.handlers.submitDubheQrLogin();
  assert.equal(state.requests[0].options.signal.aborted, true);
  state.requests[0].reject(new Error("old network failure"));
  await first;
  assert.equal(state.loginBusyRef.current, true);
  assert.equal(state.qr.status, "creating");
  state.requests[1].resolve(null);
  await second;
  assert.equal(state.commands.length, 0);
});

test("actual page identity leases reject epoch, socket, generation, auth, screen, and intent changes", async () => {
  for (const retire of [
    (state) => { state.loginAuthRef.current.epoch += 1; },
    (state) => { state.socketRef.current = { id: "socket-2" }; },
    (state) => { state.equipmentConnectionGenerationRef.current += 1; },
    (state) => { state.activeReconnectAuthRef.current = { kind: "another-auth" }; },
    (state) => { state.screenRef.current = "game"; },
    (state) => { state.oauthIntentRef.current += 1; },
    (state) => { state.loginAuthRef.current.surface = "registration"; },
  ]) {
    const state = pageHarness();
    const result = state.handlers.submitDubheQrLogin();
    retire(state);
    state.requests[0].resolve(canonicalLogin);
    await result;
    assert.equal(state.commands.length, 0);
  }
});

test("actual page lifecycle aborts retired QR and prevents old completion from clearing newer busy", async () => {
  const state = pageHarness();
  const first = state.handlers.submitDubheQrLogin();
  state.loginAuthRef.current.epoch += 1;
  state.cleanup();
  state.handlers.installLifecycle();
  assert.equal(state.requests[0].options.signal.aborted, true);
  state.loginBusyRef.current = true; // A new authentication operation owns the surface.
  state.requests[0].resolve(canonicalLogin);
  await first;
  assert.equal(state.commands.length, 0);
  assert.equal(state.loginBusyRef.current, true);
  assert.equal(state.qr, null);
});
