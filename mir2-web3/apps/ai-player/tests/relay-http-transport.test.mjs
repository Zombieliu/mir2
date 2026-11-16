import test from "node:test";
import assert from "node:assert/strict";
import net from "node:net";
import tls from "node:tls";
import { readFileSync } from "node:fs";
import { Dispatcher, EnvHttpProxyAgent, MockAgent, Request as UndiciRequest, getGlobalDispatcher } from "undici";
import { createRelayHttpTransport } from "../src/relay-http-transport.mjs";

const ORIGIN = "https://dubhe-connect-relay.zombiesliu.workers.dev";
const SESSIONS = `${ORIGIN}/sessions`, POLL = `${SESSIONS}/session_123-abc`;
const MESSAGE = "The relay HTTPS transport is unavailable.";
const SECRET = "synthetic-proxy-password";
const CA = type => type === "default" ? ["bundled-ca", "shared-ca"] : ["system-ca", "shared-ca"];
const POST = { method: "POST", body: "{}" };
function sanitized(error, reason) {
  assert.equal(error.message, MESSAGE); assert.equal(error.code, "RELAY_TRANSPORT_UNAVAILABLE");
  if (reason) assert.equal(error.reason, reason);
  assert.equal(error.cause, undefined);
  assert.deepEqual(Object.keys(error).sort(), ["code", "reason"]);
  assert(!String(error).includes(SECRET)); assert(!error.stack.includes(SECRET)); assert(!JSON.stringify(error).includes(SECRET));
  assert(!JSON.stringify(error).includes("proxy.invalid"));
  return true;
}
function fixture(t, environment = {}, overrides = {}, nativeFetch) {
  const calls = [], nativeCalls = [], agents = [], caCalls = [];
  const nativeResult = Promise.resolve(new Response("native"));
  const transport = createRelayHttpTransport({ environment, nativeFetch: nativeFetch ?? ((...args) => { nativeCalls.push(args); return nativeResult; }) }, {
    getCACertificates(type) { caCalls.push(type); return CA(type); },
    agentFactory(options) { const agent = { options, destroys: 0, async destroy() { this.destroys++; } }; agents.push(agent); return agent; },
    async fetchImpl(input, options) { calls.push({ input, options }); return new Response("{}", { status: 200 }); },
    ...overrides,
  });
  t.after(() => transport.close());
  return { transport, calls, nativeCalls, nativeResult, agents, caCalls };
}
function ownedMockDispatcher(mock) {
  // MockAgent inherits an unimplemented destroy(); its test adapter owns close.
  // Production still destroys the real EnvHttpProxyAgent's sockets promptly.
  let closing;
  const close = () => closing ??= mock.close();
  return new class extends Dispatcher {
    dispatch(options, handler) { return mock.dispatch(options, handler); }
    close() { return close(); }
    destroy() { return close(); }
  }();
}

test("import is inert, preserves global fetch/dispatcher and uses pinned official Undici 7.30.1", async t => {
  const beforeFetch = globalThis.fetch, beforeDispatcher = getGlobalDispatcher();
  let sockets = 0, caReads = 0, nativeCalls = 0;
  t.mock.method(net.Socket.prototype, "connect", () => { sockets++; throw Error("network forbidden"); });
  t.mock.method(tls, "getCACertificates", () => { caReads++; throw Error("CA must stay lazy"); });
  t.mock.method(globalThis, "fetch", () => { nativeCalls++; throw Error("fetch must stay inert"); });
  const mockedFetch = globalThis.fetch;
  await import(`../src/relay-http-transport.mjs?inert-relay-test`);
  assert.equal(globalThis.fetch, mockedFetch); assert.equal(getGlobalDispatcher(), beforeDispatcher);
  assert.equal(sockets, 0); assert.equal(caReads, 0); assert.equal(nativeCalls, 0);
  assert.equal(typeof beforeFetch, "function");
  assert.equal(JSON.parse(readFileSync(new URL("../node_modules/undici/package.json", import.meta.url))).version, "7.30.1");
});

test("factory snapshots explicit proxies but CA/agent creation stays lazy until a valid relay fetch", async t => {
  const environment = { HTTP_PROXY: "http://proxy.invalid:3128", HTTPS_PROXY: "https://secure-proxy.invalid:8443", NO_PROXY: "localhost,.local" };
  const f = fixture(t, environment);
  assert.equal(f.agents.length, 0); assert.deepEqual(f.caCalls, []);
  environment.HTTP_PROXY = "http://changed.invalid:1"; environment.HTTPS_PROXY = "http://changed.invalid:2"; environment.NO_PROXY = "*";
  await f.transport.fetch(POLL);
  assert.equal(f.agents.length, 1); assert.deepEqual(f.caCalls, ["default", "system"]);
  const options = f.agents[0].options;
  assert.equal(options.httpProxy, "http://proxy.invalid:3128/");
  assert.equal(options.httpsProxy, "https://secure-proxy.invalid:8443/"); assert.equal(options.noProxy, "localhost,.local");
  await f.transport.fetch(SESSIONS, POST);
  assert.equal(f.agents.length, 1); assert.deepEqual(f.caCalls, ["default", "system"]);
});

test("all non-relay origins and Request/URL objects pass through natively with exact input/init/promise identity, even after close", async t => {
  const f = fixture(t, { HTTPS_PROXY: `http://user:${SECRET}@proxy.invalid:3128` });
  const inputs = ["http://127.0.0.1:19040/health", "http://localhost:19141/api/auth", "http://[::1]:19141/health",
    new URL("http://127.0.0.1:19040/control"), new Request("http://localhost:19040/account", { ...POST, body: "untouched-native-body" }),
    new UndiciRequest("http://[::1]:19040/account", POST), "https://api.openai.com/v1/responses", "https://wallet.dubhe.obelisk.build/",
    "https://dubhe-connect-relay.zombiesliu.workers.dev.attacker.invalid/sessions", `${ORIGIN}:8443/sessions`,
    "http://dubhe-connect-relay.zombiesliu.workers.dev/sessions", "not a URL"];
  const init = { method: "DELETE", redirect: "follow", dispatcher: { native: true }, headers: { Host: "native-host" } };
  for (const input of inputs) {
    assert.equal(f.transport.fetch(input, init), f.nativeResult);
    assert.equal(f.nativeCalls.at(-1)[0], input); assert.equal(f.nativeCalls.at(-1)[1], init);
    if (input instanceof Request || input instanceof UndiciRequest) assert.equal(input.bodyUsed, false);
  }
  assert.equal(f.calls.length, 0); assert.equal(f.agents.length, 0); assert.deepEqual(f.caCalls, []);
  await f.transport.close();
  assert.equal(f.transport.fetch(inputs[4], init), f.nativeResult); assert.equal(inputs[4].bodyUsed, false);
  await assert.rejects(f.transport.fetch(POLL), error => sanitized(error, "closed"));
});

test("native failures are passed through unchanged instead of being mistaken for relay errors", t => {
  const original = new Error("native-only failure"), promise = Promise.reject(original);
  promise.catch(() => {});
  const f = fixture(t, {}, {}, () => promise);
  assert.equal(f.transport.fetch("http://127.0.0.1/control"), promise);
  const sync = fixture(t, {}, {}, () => { throw original; });
  assert.throws(() => sync.transport.fetch("http://localhost/control"), error => error === original);
});

test("overridden URL/Request accessors cannot retarget a local request into the relay dispatcher", async t => {
  const f = fixture(t);
  for (const RequestClass of [Request, UndiciRequest]) {
    const local = new RequestClass("http://127.0.0.1:19040/control", POST);
    Object.defineProperty(local, "url", { get() { throw Error(SECRET); } });
    assert.equal(f.transport.fetch(local), f.nativeResult);
    assert.equal(f.nativeCalls.at(-1)[0], local); assert.equal(local.bodyUsed, false);
  }
  class RelayURL extends URL { toString() { return "http://localhost:19040/control"; } }
  await f.transport.fetch(new RelayURL(POLL));
  assert.equal(f.calls[0].input, POLL); assert.equal(f.agents.length, 1);
});

test("lowercase precedence, explicit empty values, HTTP fallback and selected ALL_PROXY are snapshotted", async t => {
  const cases = [
    [{}, ["", "", ""]],
    [{ http_proxy: "", HTTP_PROXY: "http://ignored.invalid:1", https_proxy: "http://lower.invalid:2", HTTPS_PROXY: "http://ignored.invalid:3", no_proxy: "", NO_PROXY: "*" }, ["", "http://lower.invalid:2/", ""]],
    [{ HTTP_PROXY: "http://proxy.invalid:4" }, ["http://proxy.invalid:4/", "http://proxy.invalid:4/", ""]],
    [{ ALL_PROXY: "https://all.invalid:5", NO_PROXY: "localhost, .local" }, ["https://all.invalid:5/", "https://all.invalid:5/", "localhost, .local"]],
    [{ HTTP_PROXY: "http://selected.invalid:6", HTTPS_PROXY: "http://selected.invalid:6", ALL_PROXY: "socks5://unused.invalid:7" }, ["http://selected.invalid:6/", "http://selected.invalid:6/", ""]],
  ];
  for (const [environment, expected] of cases) {
    const f = fixture(t, environment); await f.transport.fetch(POLL);
    const { httpProxy, httpsProxy, noProxy } = f.agents[0].options;
    assert.deepEqual([httpProxy, httpsProxy, noProxy], expected);
  }
});

test("direct, target and proxy TLS enforce system/default CA union, hostname verification and TLS1.2 minimum", async t => {
  const f = fixture(t, { HTTPS_PROXY: `https://user:${SECRET}@proxy.invalid:8443` });
  const signal = new AbortController().signal;
  await f.transport.fetch(SESSIONS, { ...POST, signal, rejectUnauthorized: false, minVersion: "TLSv1",
    dispatcher: { unsafe: true }, connect: { rejectUnauthorized: false }, redirect: "follow" });
  const options = f.agents[0].options;
  for (const key of ["connect", "requestTls", "proxyTls"]) {
    assert.deepEqual(options[key].ca, ["bundled-ca", "shared-ca", "system-ca"]);
    assert.equal(options[key].rejectUnauthorized, true); assert.equal(options[key].checkServerIdentity, tls.checkServerIdentity);
    assert.equal(options[key].minVersion, "TLSv1.2"); assert.equal(options[key].timeout, 10_000);
  }
  assert.equal(options.connect.servername, new URL(ORIGIN).hostname); assert.equal(options.requestTls.servername, new URL(ORIGIN).hostname);
  assert.equal(options.proxyTls.servername, undefined); assert.equal(options.connections, 1);
  assert.equal(f.calls[0].options.dispatcher, f.agents[0]); assert.equal(f.calls[0].options.redirect, "error");
  assert.equal(f.calls[0].options.signal, signal);
});

test("the uninjected production agent factory creates a private real EnvHttpProxyAgent without opening sockets", async t => {
  let sockets = 0, dispatcher;
  const before = getGlobalDispatcher();
  t.mock.method(net.Socket.prototype, "connect", () => { sockets++; throw Error("network forbidden"); });
  const transport = createRelayHttpTransport({ environment: {}, nativeFetch() { assert.fail("fixed relay must use its own agent"); } }, {
    getCACertificates: CA,
    async fetchImpl(input, options) { dispatcher = options.dispatcher; assert.equal(input, POLL); return new Response("{}"); },
  });
  t.after(() => transport.close());
  await transport.fetch(POLL); assert(dispatcher instanceof EnvHttpProxyAgent);
  assert.notEqual(dispatcher, before); assert.equal(getGlobalDispatcher(), before); assert.equal(sockets, 0);
});

test("bad proxy settings, environment getters and explicit TLS bypass fail with only safe configuration errors", () => {
  for (const options of [null, 1, [], { get environment() { throw Error(SECRET); } }, { get nativeFetch() { throw Error(SECRET); } }])
    assert.throws(() => createRelayHttpTransport(options), error => sanitized(error, "configuration"));
  for (const environment of [null, 1, { HTTPS_PROXY: "socks5://proxy.invalid:1" }, { HTTPS_PROXY: `http://user:${SECRET}@proxy.invalid/path` },
    { HTTPS_PROXY: `http://proxy.invalid:1?token=${SECRET}` }, { HTTPS_PROXY: `not a URI ${SECRET}` },
    { HTTPS_PROXY: "http://proxy.invalid:1\nother" }, { HTTPS_PROXY: 42 }, { NO_PROXY: "x".repeat(8193) },
    { NO_PROXY: "localhost\u200b" }, { NODE_TLS_REJECT_UNAUTHORIZED: "0" }, { get HTTP_PROXY() { throw Error(SECRET); } }]) {
    assert.throws(() => createRelayHttpTransport({ environment, nativeFetch() {} }), error => sanitized(error, "configuration"));
  }
  assert.throws(() => createRelayHttpTransport({ environment: {}, nativeFetch: null }), error => sanitized(error, "configuration"));
  for (const dependencies of [{ fetchImpl: null }, { agentFactory: null }, { getCACertificates: 0 }])
    assert.throws(() => createRelayHttpTransport({ environment: {}, nativeFetch() {} }, dependencies), error => sanitized(error, "configuration"));
});

test("lazy CA/agent failures are sanitized and sticky, never rebuilding an invalid owned dispatcher", async t => {
  for (const overrides of [{ getCACertificates() { throw Error(SECRET); } }, { getCACertificates: () => [] },
    { getCACertificates: () => ["valid", 0] }, { getCACertificates: () => "invalid-array" }, { agentFactory() { throw Error(SECRET); } },
    { agentFactory: () => ({}) }]) {
    let builds = 0;
    const failing = overrides.agentFactory ?? (() => { builds++; assert.fail("CA must be valid before an agent is built"); });
    const f = fixture(t, {}, { ...overrides, agentFactory(options) { builds++; return failing(options); } });
    await assert.rejects(f.transport.fetch(POLL), error => sanitized(error, "configuration"));
    await assert.rejects(f.transport.fetch(POLL), error => sanitized(error, "configuration"));
    assert(builds <= 1); assert.equal(f.calls.length, 0);
  }
});

test("only safe relay session paths and GET/POST methods are admitted; validated headers cannot change after inspection", async t => {
  const f = fixture(t), headers = { "Content-Type": "application/json", "X-Relay-Test": "test" };
  await f.transport.fetch(new URL(SESSIONS), { ...POST, headers, redirect: "follow", dispatcher: { unsafe: true } });
  await f.transport.fetch(POLL);
  headers.Host = "attacker.invalid";
  assert.equal(f.calls[0].options.headers.has("host"), false); assert.equal(f.calls[0].options.headers.get("x-relay-test"), "test");
  assert.equal(f.calls[0].input, SESSIONS); assert.equal(f.calls[0].options.redirect, "error");
  assert.equal(f.calls[1].options.method, "GET");
  for (const url of [`${ORIGIN}/`, `${ORIGIN}/admin`, `${SESSIONS}/`, `${POLL}/approve`, `${POLL}?token=${SECRET}`, `${POLL}#${SECRET}`,
    `${POLL}?`, `${POLL}#`, `${SESSIONS}/%2Fescape`, `${SESSIONS}/${"a".repeat(257)}`, `https://user:${SECRET}@${new URL(ORIGIN).host}/sessions`]) {
    await assert.rejects(f.transport.fetch(url, POST), error => sanitized(error, "configuration"));
  }
  for (const init of [null, [], { method: "DELETE" }, { method: "HEAD" }, { method: "POST " }, { ...POST, headers: { Host: "bad" } },
    { headers: [["pRoXy-AuThOrIzAtIoN", SECRET]] }, { get headers() { throw Error(SECRET); } }, { headers: { "bad\nname": SECRET } }]) {
    await assert.rejects(f.transport.fetch(POLL, init), error => sanitized(error, "configuration"));
  }
  assert.equal(f.calls.length, 2); assert.equal(f.nativeCalls.length, 0); assert.equal(f.agents.length, 1);
});

test("both native and Undici Request brands use the real Undici fetch without losing their relay POST bodies", async t => {
  let sockets = 0;
  t.mock.method(net.Socket.prototype, "connect", () => { sockets++; throw Error("network forbidden"); });
  const mock = new MockAgent(); mock.disableNetConnect();
  const pool = mock.get(ORIGIN);
  const f = fixture(t, {}, { agentFactory: () => ownedMockDispatcher(mock), fetchImpl: undefined });
  for (const RequestClass of [Request, UndiciRequest]) {
    pool.intercept({ path: "/sessions", method: "POST" }).reply(201, async ({ body }) => {
      const chunks = []; for await (const chunk of body) chunks.push(Buffer.from(chunk));
      return { created: true, body: Buffer.concat(chunks).toString("utf8") };
    });
    const request = new RequestClass(SESSIONS, { method: "POST", body: '{"test":"body"}', headers: { "Content-Type": "application/json" } });
    assert.deepEqual(await (await f.transport.fetch(request)).json(), { created: true, body: '{"test":"body"}' });
    assert.equal(request.bodyUsed, true);
  }
  assert.equal(sockets, 0); assert.equal(f.nativeCalls.length, 0);
});

test("real official proxy routing honors explicit noProxy hosts/wildcards/ports without any socket", async t => {
  let sockets = 0;
  t.mock.method(net.Socket.prototype, "connect", () => { sockets++; throw Error("network forbidden"); });
  const host = new URL(ORIGIN).hostname;
  for (const [environment, expected] of [[{}, "proxy"], [{ NO_PROXY: host }, "direct"], [{ NO_PROXY: ".workers.dev" }, "direct"],
    [{ NO_PROXY: "*.workers.dev" }, "direct"], [{ NO_PROXY: `${host}:443` }, "direct"], [{ NO_PROXY: `${host}:8443` }, "proxy"],
    [{ NO_PROXY: "*" }, "direct"], [{ no_proxy: "", NO_PROXY: "*" }, "proxy"]]) {
    const routes = [];
    const f = fixture(t, { HTTPS_PROXY: "http://proxy.invalid:3128", ...environment }, {
      fetchImpl: undefined,
      agentFactory: options => new EnvHttpProxyAgent({ ...options, factory(origin, connection) {
        routes.push(typeof connection.connect === "function" ? "proxy" : "direct");
        const mock = new MockAgent(); mock.disableNetConnect(); const pool = mock.get(origin);
        pool.intercept({ path: "/sessions/session_123-abc", method: "GET" }).reply(200, { pending: true });
        let closing;
        const close = () => closing ??= mock.close();
        return new class extends Dispatcher {
          dispatch(options, handler) { return pool.dispatch(options, handler); }
          close() { return close(); }
          destroy() { return close(); }
        }();
      } }),
    });
    assert.deepEqual(await (await f.transport.fetch(POLL)).json(), { pending: true }); assert.deepEqual(routes, [expected]);
  }
  assert.equal(sockets, 0);
});

test("real redirect responses fail instead of sending a relay request to localhost or another origin", async t => {
  let sockets = 0;
  t.mock.method(net.Socket.prototype, "connect", () => { sockets++; throw Error("network forbidden"); });
  const mock = new MockAgent(); mock.disableNetConnect();
  mock.get(ORIGIN).intercept({ path: "/sessions/session_123-abc", method: "GET" })
    .reply(302, "", { headers: { location: "http://127.0.0.1:19040/control" } });
  const f = fixture(t, {}, { fetchImpl: undefined, agentFactory: () => ownedMockDispatcher(mock) });
  await assert.rejects(f.transport.fetch(POLL, { redirect: "follow" }), error => sanitized(error, "network"));
  assert.equal(f.nativeCalls.length, 0); assert.equal(sockets, 0);
});

test("relay failures expose only fixed safe categories, including hostile error getters and forged codes", async t => {
  for (const [error, reason] of [
    [Object.assign(Error(SECRET), { cause: { code: "SELF_SIGNED_CERT_IN_CHAIN", message: SECRET } }), "tls-trust"],
    [Object.assign(Error(SECRET), { code: "ERR_TLS_CERT_ALTNAME_INVALID" }), "tls-trust"],
    [Object.assign(Error(SECRET), { cause: { code: "UND_ERR_CONNECT_TIMEOUT" } }), "timeout"],
    [Object.assign(Error(SECRET), { name: "AbortError" }), "aborted"],
    [Object.assign(Error(SECRET), { code: "RELAY_TRANSPORT_UNAVAILABLE", reason: SECRET }), "network"],
    [{ get code() { throw Error(SECRET); } }, "network"],
  ]) {
    const f = fixture(t, {}, { async fetchImpl() { throw error; } });
    await assert.rejects(f.transport.fetch(POLL), value => sanitized(value, reason));
    assert.equal(f.agents.length, 1);
  }
});

test("caller cancellation is propagated once and an already-aborted request cannot create the agent", async t => {
  const f = fixture(t, {}, { fetchImpl(input, options) {
    return new Promise((resolve, reject) => options.signal.addEventListener("abort", () => reject(Object.assign(Error(SECRET), { name: "AbortError" })), { once: true }));
  } });
  const controller = new AbortController(), pending = assert.rejects(f.transport.fetch(POLL, { signal: controller.signal }), error => sanitized(error, "aborted"));
  controller.abort(SECRET); await pending;
  const untouched = fixture(t), aborted = new AbortController(); aborted.abort(SECRET);
  await assert.rejects(untouched.transport.fetch(POLL, { signal: aborted.signal }), error => sanitized(error, "aborted"));
  assert.equal(untouched.agents.length, 0); assert.deepEqual(untouched.caCalls, []);
});

test("idempotent close cancels only owned work; sibling/global/native transports remain usable", async t => {
  const before = getGlobalDispatcher(); let rejectPending, destroys = 0;
  const f = fixture(t, {}, { fetchImpl: () => new Promise((resolve, reject) => { rejectPending = reject; }),
    agentFactory: () => ({ async destroy() { destroys++; rejectPending(Error(SECRET)); } }) });
  const sibling = fixture(t);
  const pending = assert.rejects(f.transport.fetch(POLL), error => sanitized(error, "closed"));
  const closing = f.transport.close(); assert.equal(f.transport.close(), closing);
  await closing; await pending; assert.equal(destroys, 1);
  assert.equal(f.transport.fetch("http://localhost:19040/health"), f.nativeResult);
  await sibling.transport.fetch(POLL); assert.equal(sibling.agents[0].destroys, 0);
  assert.equal(getGlobalDispatcher(), before);
  await assert.rejects(f.transport.fetch(POLL), error => sanitized(error, "closed"));
});

test("unused close never builds an agent, and destroy failures are sanitized without retry", async t => {
  const unused = fixture(t); await unused.transport.close(); assert.equal(unused.agents.length, 0); assert.deepEqual(unused.caCalls, []);
  let destroys = 0;
  const transport = createRelayHttpTransport({ environment: {}, nativeFetch() {} }, { getCACertificates: CA,
    agentFactory: () => ({ destroy() { destroys++; throw Error(SECRET); } }), fetchImpl: async () => new Response("{}") });
  await transport.fetch(POLL);
  const closing = transport.close(); assert.equal(transport.close(), closing);
  await assert.rejects(closing, error => sanitized(error, "close"));
  await assert.rejects(transport.close(), error => sanitized(error, "close")); assert.equal(destroys, 1);
});
