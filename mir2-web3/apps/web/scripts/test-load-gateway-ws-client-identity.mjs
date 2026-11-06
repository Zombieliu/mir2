import assert from "node:assert/strict";
import test from "node:test";

import {
  assertSafeHandshakeHeaderValue,
  isLoopbackWebSocketUrl,
  loadAccountId,
  loadHarnessClientIdentity,
} from "./load-gateway-ws-client-identity.mjs";

test("loopback URL recognition is fail closed", () => {
  assert.equal(isLoopbackWebSocketUrl("ws://127.0.0.1:7210/ws"), true);
  assert.equal(isLoopbackWebSocketUrl("wss://localhost/ws"), true);
  assert.equal(isLoopbackWebSocketUrl("ws://[::1]:7210/ws"), true);
  assert.equal(isLoopbackWebSocketUrl("wss://gateway.example/ws"), false);
  assert.equal(isLoopbackWebSocketUrl("https://127.0.0.1/ws"), false);
});

test("ordinary load clients share a stable non-secret user agent", () => {
  assert.deepEqual(
    loadHarnessClientIdentity({
      index: 4,
      runId: "run",
      simulateDistinctClients: false,
      wsUrl: "wss://gateway.example/ws",
    }),
    { userAgent: "mir2-ws-load/1", clientIp: null },
  );
});

test("distinct loopback clients get deterministic benchmark addresses and devices", () => {
  assert.deepEqual(
    loadHarnessClientIdentity({
      index: 300,
      runId: "run:unsafe",
      simulateDistinctClients: true,
      wsUrl: "ws://127.0.0.1:7210/ws",
    }),
    {
      userAgent: "mir2-ws-load-distinct/run_unsafe/300",
      clientIp: "198.18.1.44",
    },
  );
});

test("distinct identity simulation refuses remote targets", () => {
  assert.throws(
    () =>
      loadHarnessClientIdentity({
        index: 0,
        runId: "run",
        simulateDistinctClients: true,
        wsUrl: "wss://gateway.example/ws",
      }),
    /restricted to a loopback/,
  );
});

test("handshake values reject header injection", () => {
  assert.equal(assertSafeHandshakeHeaderValue("safe/value", "User-Agent"), "safe/value");
  assert.throws(
    () => assertSafeHandshakeHeaderValue("safe\r\nInjected: yes", "User-Agent"),
    /forbidden ASCII control character/,
  );
  assert.throws(
    () => assertSafeHandshakeHeaderValue("safe\u0000value", "User-Agent"),
    /forbidden ASCII control character/,
  );
  assert.throws(
    () => assertSafeHandshakeHeaderValue("safe\u007fvalue", "User-Agent"),
    /forbidden ASCII control character/,
  );
});

test("account index width is explicit and backward compatible", () => {
  assert.equal(loadAccountId("soak-", 7), "soak-7");
  assert.equal(loadAccountId("soak-", 7, 3), "soak-007");
  assert.throws(() => loadAccountId("soak-", 7, 13), /between 0 and 12/);
});
