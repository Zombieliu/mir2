import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import vm from "node:vm";
import ts from "typescript";

const require = createRequire(import.meta.url);
const ORIGIN = "https://auth.example.test";
const RELAY = "https://relay.example.test";
const WALLET = "https://wallet.example.test";

function fixture() {
  const calls = [], sessions = new Map(), modules = new Map();
  const env = {
    MIR2_RUNTIME_ENV: "production",
    MIR2_PASSKEY_AUTH_SECRET: "synthetic-normal-wallet-signing-secret-aaaaaaaaaaaa",
    MIR2_PASSKEY_ALLOWED_ORIGINS: ORIGIN,
    MIR2_DUBHE_CONNECT_RELAY_URL: RELAY,
    MIR2_DUBHE_WALLET_ORIGIN: WALLET,
    MIR2_DUBHE_CONNECT_NETWORK: "testnet",
  };
  const fetch = async (target, init) => {
    calls.push({ target, init });
    if (target === RELAY + "/sessions") {
      const body = JSON.parse(init.body), now = Date.now();
      const id = "synthetic-session";
      const session = {
        id, relayOrigin: RELAY, status: "pending",
        approvalUrl: WALLET + "/connect/approve?session=" + id + "&relay=" + encodeURIComponent(RELAY),
        pollUrl: RELAY + "/sessions/" + id,
        request: {
          appName: body.appName, origin: body.origin, network: body.network,
          walletOrigin: body.walletOrigin, statement: body.statement,
          nonce: "synthetic-server-nonce", requestedAt: new Date(now).toISOString(),
          expiresAt: new Date(now + 120_000).toISOString(),
        },
      };
      sessions.set(session.pollUrl, session);
      return Response.json(session);
    }
    assert.ok(sessions.has(target), "Only the configured synthetic relay may be read");
    return Response.json(sessions.get(target));
  };
  function load(file) {
    const url = new URL(file, import.meta.url);
    if (modules.has(url.href)) return modules.get(url.href);
    const source = readFileSync(url, "utf8");
    const compiled = ts.transpileModule(source, {
      fileName: url.pathname,
      compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
    }).outputText;
    const module = { exports: {} };
    modules.set(url.href, module.exports);
    vm.runInNewContext(compiled, {
      module, exports: module.exports, Buffer, TextEncoder, Request, Response, Headers, URL, AbortSignal,
      process: { env }, fetch,
      require: specifier => {
        if (specifier === "server-only") return {};
        if (specifier.startsWith(".")) return load(new URL(specifier + ".ts", url));
        return require(specifier);
      },
    });
    return module.exports;
  }
  return {
    calls,
    create: load("../app/api/wallet/dubhe/session/route.ts").POST,
    poll: load("../app/api/wallet/dubhe/session/status/route.ts").POST,
  };
}
function request(token, origin = ORIGIN) {
  return new Request(ORIGIN + "/api/wallet/dubhe/session", {
    method: "POST", headers: { origin, ...(token ? { authorization: "Bearer " + token } : {}) },
  });
}
function frameworkContext() {
  return { params: Promise.resolve({}) };
}
function foreignAuthContext() {
  return {
    params: Promise.resolve({}), secret: "synthetic-untrusted-context-secret-bbbbbbbbbbbbb",
    allowedOrigins: ["https://foreign.example.test"], network: "mainnet",
    relayOrigin: "https://foreign-relay.example.test", walletOrigin: "https://foreign-wallet.example.test",
  };
}

test("actual standalone route accepts the framework context without treating params as AuthContext", async () => {
  const f = fixture();
  const created = await f.create(request(), frameworkContext());
  assert.equal(created.status, 200);
  const challenge = await created.json();
  assert.equal(f.calls.length, 1);
  assert.equal(f.calls[0].target, RELAY + "/sessions");
  const relayRequest = JSON.parse(f.calls[0].init.body);
  assert.equal(relayRequest.origin, ORIGIN);
  assert.equal(relayRequest.network, "testnet");
  const pending = await f.poll(request(challenge.intentToken), frameworkContext());
  assert.equal(pending.status, 200);
  assert.deepEqual(await pending.json(), { status: "pending" });
  assert.equal(f.calls[1].target, RELAY + "/sessions/synthetic-session");
});

test("neither standalone route can select signing, relay, wallet or network through its second argument", async () => {
  const f = fixture();
  const created = await f.create(request(), foreignAuthContext());
  assert.equal(created.status, 200);
  const challenge = await created.json();
  const relayRequest = JSON.parse(f.calls[0].init.body);
  assert.equal(relayRequest.network, "testnet");
  assert.equal(relayRequest.walletOrigin, WALLET);
  const pending = await f.poll(request(challenge.intentToken), foreignAuthContext());
  assert.equal(pending.status, 200);
  assert.deepEqual(await pending.json(), { status: "pending" });
  assert.ok(f.calls.every(call => call.target.startsWith(RELAY + "/sessions")));
});

test("a foreign Origin stays denied even if the route context claims to allow it", async () => {
  const f = fixture();
  const denied = await f.create(request(null, "https://foreign.example.test"), foreignAuthContext());
  assert.equal(denied.status, 403);
  assert.equal(f.calls.length, 0);
});
