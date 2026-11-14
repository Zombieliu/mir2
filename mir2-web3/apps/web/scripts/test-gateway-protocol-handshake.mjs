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

const connectGatewaySource = pageSource.split("function connectGateway(")[1].split("\n  useEffect(")[0];
for (const event of ["open", "close", "error", "message"]) {
  const listener = connectGatewaySource.split(`socket.addEventListener("${event}",`)[1];
  assert.match(listener?.split("\n")[1] ?? "", /if \(socketRef\.current !== socket(?:\s*\|\|[^)]*)?\) return;/,
    `the ${event} listener must check the current socket before taking action`);
}
assert.match(pageSource, /case "Connected":\s*setLoginErrorKey\(null\);\s*gatewayProtocolReadyRef\.current = true;\s*flushGatewayProtocolQueue\(\);/);
assert.match(captureSource, /waitForGatewayProtocolReady/);
assert.match(captureSource, /gatewayProtocolReady === true/);

console.log("gateway protocol handshake contract tests passed");
