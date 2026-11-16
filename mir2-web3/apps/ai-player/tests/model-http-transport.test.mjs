import test from "node:test";
import assert from "node:assert/strict";
import net from "node:net";
import tls from "node:tls";
import { Dispatcher, EnvHttpProxyAgent, MockAgent } from "undici";
import { createModelFetch } from "../src/model/http-transport.mjs";

const ENDPOINT = "https://api.openai.com/v1/responses";
const MESSAGE = "The model HTTPS transport is unavailable.";
const CA = type => type === "default" ? ["bundled-ca", "shared-ca"] : ["system-ca", "shared-ca"];
const POST = { method: "POST", body: "{}" };
function sanitized(error) {
  assert.equal(error.message, MESSAGE);
  assert.equal(error.code, "MODEL_TRANSPORT_UNAVAILABLE");
  assert.equal(error.cause, undefined);
  assert.ok(!String(error).includes("synthetic-password"));
  return true;
}
function fixture(t, environment = {}, overrides = {}) {
  const calls = [], agents = [];
  const transport = createModelFetch({ environment }, {
    getCACertificates: CA,
    agentFactory(options) { const agent = { options, destroys: 0, async destroy() { this.destroys++; } }; agents.push(agent); return agent; },
    async fetchImpl(url, options) { calls.push({ url, options }); return new Response("{}", { status: 200 }); },
    ...overrides,
  });
  t.after(() => transport.close());
  return { transport, calls, agents };
}

test("proxy settings are a private explicit snapshot, including absent variables", async t => {
  const environment = { HTTP_PROXY: "http://proxy.invalid:3128", HTTPS_PROXY: "https://secure-proxy.invalid:8443", NO_PROXY: "localhost,.local" };
  const f = fixture(t, environment);
  assert.equal(f.agents[0].options.httpProxy, "http://proxy.invalid:3128/");
  assert.equal(f.agents[0].options.httpsProxy, "https://secure-proxy.invalid:8443/");
  assert.equal(f.agents[0].options.noProxy, "localhost,.local");
  environment.HTTPS_PROXY = "http://changed.invalid:9999";
  assert.equal(f.agents[0].options.httpsProxy, "https://secure-proxy.invalid:8443/");
  const direct = fixture(t);
  assert.equal(direct.agents[0].options.httpProxy, "");
  assert.equal(direct.agents[0].options.httpsProxy, "");
  assert.equal(direct.agents[0].options.noProxy, "");
});

test("official lowercase precedence, HTTP fallback and HTTP(S) ALL_PROXY fallback are preserved", t => {
  const lower = fixture(t, { http_proxy: "", HTTP_PROXY: "http://ignored.invalid:1", https_proxy: "http://lower.invalid:2", HTTPS_PROXY: "http://ignored.invalid:3", no_proxy: "", NO_PROXY: "*" });
  assert.equal(lower.agents[0].options.httpProxy, "");
  assert.equal(lower.agents[0].options.httpsProxy, "http://lower.invalid:2/");
  assert.equal(lower.agents[0].options.noProxy, "");
  const httpOnly = fixture(t, { HTTP_PROXY: "http://proxy.invalid:4" });
  assert.equal(httpOnly.agents[0].options.httpsProxy, "http://proxy.invalid:4/");
  const all = fixture(t, { ALL_PROXY: "https://all.invalid:5" });
  assert.equal(all.agents[0].options.httpProxy, "https://all.invalid:5/");
  assert.equal(all.agents[0].options.httpsProxy, "https://all.invalid:5/");
  const unusedAll = fixture(t, { HTTP_PROXY: "http://selected.invalid:6", HTTPS_PROXY: "http://selected.invalid:6", ALL_PROXY: "socks5://unused.invalid:7" });
  assert.equal(unusedAll.agents[0].options.httpsProxy, "http://selected.invalid:6/");
});

test("direct, target and proxy TLS all retain verification and union default/system trust", t => {
  const f = fixture(t, { HTTPS_PROXY: "https://proxy.invalid:8443" });
  const options = f.agents[0].options;
  for (const name of ["connect", "requestTls", "proxyTls"]) {
    assert.deepEqual(options[name].ca, ["bundled-ca", "shared-ca", "system-ca"]);
    assert.equal(options[name].rejectUnauthorized, true);
    assert.equal(options[name].checkServerIdentity, tls.checkServerIdentity);
    assert.equal(options[name].minVersion, "TLSv1.2");
  }
  assert.equal(options.connect.servername, "api.openai.com");
  assert.equal(options.requestTls.servername, "api.openai.com");
  assert.equal(options.proxyTls.servername, undefined);
  assert.equal(options.connections, 1);
});

test("unsupported proxies, malformed configuration and TLS bypass fail without sensitive errors", () => {
  const construct = environment => createModelFetch({ environment }, { getCACertificates: CA, agentFactory() { throw Error("must not create an agent"); } });
  for (const environment of [
    { HTTPS_PROXY: "socks5://proxy.invalid:1" }, { HTTPS_PROXY: "http://user:synthetic-password@proxy.invalid/path" },
    { HTTPS_PROXY: "http://proxy.invalid:1?token=synthetic-password" }, { HTTPS_PROXY: "not a URI synthetic-password" },
    { HTTPS_PROXY: "http://proxy.invalid:1\nother" }, { HTTPS_PROXY: 42 }, { NO_PROXY: "x".repeat(8193) },
    { NODE_TLS_REJECT_UNAUTHORIZED: "0" },
  ]) assert.throws(() => construct(environment), sanitized);
  assert.throws(() => createModelFetch({}, { getCACertificates() { throw new Error("synthetic-password"); } }), sanitized);
});

test("the sole POST endpoint is fixed and caller options cannot replace its dispatcher or follow redirects", async t => {
  const f = fixture(t);
  const signal = new AbortController().signal;
  const response = await f.transport.fetch(new URL(ENDPOINT), { ...POST, signal, redirect: "follow", dispatcher: { unsafe: true }, headers: { Authorization: "Bearer synthetic-value" } });
  assert.equal(response.status, 200);
  assert.equal(f.calls[0].url, ENDPOINT);
  assert.equal(f.calls[0].options.dispatcher, f.agents[0]);
  assert.equal(f.calls[0].options.redirect, "error");
  assert.equal(f.calls[0].options.signal, signal);
  assert.equal(f.calls[0].options.headers.Authorization, "Bearer synthetic-value");
  for (const url of ["http://api.openai.com/v1/responses", "https://api.openai.com.attacker.invalid/v1/responses", "https://api.openai.com/v1/models", `${ENDPOINT}?query=1`, `${ENDPOINT}#fragment`, "https://user:synthetic-password@api.openai.com/v1/responses"]) {
    await assert.rejects(f.transport.fetch(url, POST), sanitized);
  }
  for (const init of [{ method: "GET" }, { ...POST, headers: { Host: "attacker.invalid" } }, { ...POST, headers: { "Proxy-Authorization": "synthetic-password" } }]) {
    await assert.rejects(f.transport.fetch(ENDPOINT, init), sanitized);
  }
  assert.equal(f.calls.length, 1);
});

test("network and TLS failures retain only fixed safe categories, not causes or proxy passwords", async t => {
  for (const [code, expected] of [["SELF_SIGNED_CERT_IN_CHAIN", "tls-trust"], ["ERR_TLS_CERT_ALTNAME_INVALID", "tls-trust"], ["UND_ERR_CONNECT_TIMEOUT", "timeout"], ["PRIVATE_CODE", "network"]]) {
    const f = fixture(t, {}, { async fetchImpl() { throw Object.assign(new Error("http://user:synthetic-password@proxy.invalid"), { cause: { code } }); } });
    await assert.rejects(f.transport.fetch(ENDPOINT, POST), error => { sanitized(error); assert.equal(error.reason, expected); return true; });
  }
  const forged = fixture(t, {}, { async fetchImpl() { throw Object.assign(new Error("synthetic-password"), { code: "MODEL_TRANSPORT_UNAVAILABLE", reason: "configuration" }); } });
  await assert.rejects(forged.transport.fetch(ENDPOINT, POST), sanitized);
});

test("close is idempotent and cancels only the owned dispatcher without waiting for a stalled response", async t => {
  let rejectPending, destroyed = 0;
  const f = fixture(t, {}, {
    fetchImpl() { return new Promise((resolve, reject) => { rejectPending = reject; }); },
    agentFactory() { return { async destroy() { destroyed++; rejectPending(new Error("synthetic-password")); } }; },
  });
  const pending = assert.rejects(f.transport.fetch(ENDPOINT, POST), sanitized);
  const closing = f.transport.close();
  assert.equal(f.transport.close(), closing);
  await closing; await pending;
  assert.equal(destroyed, 1);
  await assert.rejects(f.transport.fetch(ENDPOINT, POST), error => { sanitized(error); assert.equal(error.reason, "closed"); return true; });
});

test("dispatcher cleanup failures are sanitized and never retried", async () => {
  let destroyed = 0;
  const transport = createModelFetch({ environment: {} }, { getCACertificates: CA, agentFactory() { return { destroy() { destroyed++; throw new Error("synthetic-password"); } }; } });
  await assert.rejects(transport.close(), sanitized);
  await assert.rejects(transport.close(), sanitized);
  assert.equal(destroyed, 1);
});

test("real official EnvHttpProxyAgent honors NO_PROXY hosts, wildcards, ports and lowercase precedence without any socket", async t => {
  let socketAttempts = 0;
  t.mock.method(net.Socket.prototype, "connect", () => { socketAttempts++; throw new Error("Network forbidden in this test"); });
  const cases = [
    [{}, "proxy"], [{ NO_PROXY: "api.openai.com" }, "direct"], [{ NO_PROXY: ".openai.com" }, "direct"],
    [{ NO_PROXY: "*.openai.com" }, "direct"], [{ NO_PROXY: "api.openai.com:443" }, "direct"],
    [{ NO_PROXY: "api.openai.com:8443" }, "proxy"], [{ NO_PROXY: "localhost, api.openai.com" }, "direct"],
    [{ NO_PROXY: "*" }, "direct"], [{ no_proxy: "", NO_PROXY: "*" }, "proxy"],
  ];
  for (const [environment, expected] of cases) {
    const routes = [];
    const transport = createModelFetch({ environment: { HTTPS_PROXY: "http://proxy.invalid:3128", ...environment } }, {
      getCACertificates: CA,
      agentFactory: options => new EnvHttpProxyAgent({ ...options, factory(origin, connection) {
        routes.push(typeof connection.connect === "function" ? "proxy" : "direct");
        const mock = new MockAgent(); mock.disableNetConnect();
        const pool = mock.get(origin); pool.intercept({ path: "/v1/responses", method: "POST" }).reply(200, { mock: true });
        // Transfer cleanup ownership to this adapter. Destroying MockPool and
        // later closing its parent would otherwise double-close the same pool.
        let stopping;
        const stop = () => stopping ??= mock.close();
        return new class extends Dispatcher {
          dispatch(options, handler) { return pool.dispatch(options, handler); }
          close() { return stop(); }
          destroy() { return stop(); }
        }();
      } }),
    });
    try {
      const result = await transport.fetch(ENDPOINT, POST);
      assert.deepEqual(await result.json(), { mock: true });
      assert.deepEqual(routes, [expected]);
    } finally { await transport.close(); }
  }
  assert.equal(socketAttempts, 0);
});
