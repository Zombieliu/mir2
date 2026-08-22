import assert from "node:assert/strict";
import test from "node:test";

import {
  parseListenBindings,
  parseWindowsGatewayIdentity,
  parseWindowsGatewayProcessSample,
} from "./load-gateway-ws-process.mjs";

test("Windows process parser preserves exact PID and extended resource samples", () => {
  assert.deepEqual(
    parseWindowsGatewayProcessSample("75380,1048576,7340032,128,24,9123.5"),
    {
      atUnixMs: null,
      pid: 75380,
      workingSetBytes: 1048576,
      privateBytes: 7340032,
      handleCount: 128,
      threadCount: 24,
      cpuTimeMs: 9123.5,
      cpuPercent: null,
    },
  );
});

test("invalid or missing process samples fail closed", () => {
  assert.equal(parseWindowsGatewayProcessSample("11856,100,200"), null);
  assert.equal(parseWindowsGatewayProcessSample("0,100,200,1,1,1"), null);
});

test("listen binding parser preserves address and ignores malformed rows", () => {
  assert.deepEqual(
    parseListenBindings("127.0.0.1|75380\r\nnot-a-binding\n::1|11856\n"),
    [
      { localAddress: "127.0.0.1", pid: 75380 },
      { localAddress: "::1", pid: 11856 },
    ],
  );
});

test("Gateway executable identity parser requires a complete SHA-256 record", () => {
  const sha256 = "a".repeat(64);
  assert.deepEqual(
    parseWindowsGatewayIdentity(JSON.stringify({
      pid: 75380,
      path: "E:\\candidate\\mir2-gateway.exe",
      bytes: 12345,
      sha256,
    })),
    {
      pid: 75380,
      path: "E:\\candidate\\mir2-gateway.exe",
      bytes: 12345,
      sha256: sha256.toUpperCase(),
    },
  );
  assert.equal(parseWindowsGatewayIdentity("{}"), null);
});
