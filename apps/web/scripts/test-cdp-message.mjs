import assert from "node:assert/strict";

import { decodeCdpMessage, isCriticalConsoleError } from "./cdp-message.mjs";

const payload = JSON.stringify({ id: 7, result: { ok: true } });
const bytes = new TextEncoder().encode(payload);

assert.deepEqual(await decodeCdpMessage(payload), { id: 7, result: { ok: true } });
assert.deepEqual(await decodeCdpMessage(Buffer.from(payload)), { id: 7, result: { ok: true } });
assert.deepEqual(await decodeCdpMessage(bytes), { id: 7, result: { ok: true } });
assert.deepEqual(await decodeCdpMessage(bytes.buffer.slice(0)), { id: 7, result: { ok: true } });
assert.deepEqual(await decodeCdpMessage({ text: async () => payload }), {
  id: 7,
  result: { ok: true },
});
await assert.rejects(() => decodeCdpMessage({}), /Unsupported CDP message payload/);

assert.equal(
  isCriticalConsoleError({
    source: "other",
    text: "Unchecked runtime.lastError: The message port closed before a response was received.",
  }),
  false,
);
assert.equal(isCriticalConsoleError({ source: "network", text: "net::ERR_FAILED" }), false);
assert.equal(isCriticalConsoleError({ source: "network", text: "GET /favicon.ico 404" }), false);
assert.equal(isCriticalConsoleError({ source: "javascript", text: "TypeError: real app failure" }), true);

console.log("cdp message decoding tests passed");
