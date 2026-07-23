import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const captureSource = readFileSync(
  new URL("./capture-crystal-parity.mjs", import.meta.url),
  "utf8",
);

assert.match(pageSource, /pendingGatewayProtocolActionRef/);
assert.doesNotMatch(
  pageSource,
  /pending(?:Login|NewAccount|SuiLogin|Bootstrap)Ref/,
  "protocol readiness must keep one latest pending action instead of independent flags",
);
assert.match(
  pageSource,
  /const pendingAction = pendingGatewayProtocolActionRef\.current;\s*pendingGatewayProtocolActionRef\.current = null;/,
  "flushing must consume the pending action exactly once",
);

const staleSocketGuardCount = (
  pageSource.match(/if \(socketRef\.current !== socket\) return;/g) ?? []
).length;
assert.ok(
  staleSocketGuardCount >= 4,
  `open, error, message, and close paths need stale-socket guards; found ${staleSocketGuardCount}`,
);
assert.match(pageSource, /case "Connected":\s*setLoginErrorKey\(null\);\s*gatewayProtocolReadyRef\.current = true;\s*flushGatewayProtocolQueue\(\);/);
assert.match(captureSource, /waitForGatewayProtocolReady/);
assert.match(captureSource, /gatewayProtocolReady === true/);

console.log("gateway protocol handshake contract tests passed");
