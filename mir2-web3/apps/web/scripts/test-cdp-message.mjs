import assert from "node:assert/strict";

import { decodeCdpMessage } from "./cdp-message.mjs";

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

console.log("cdp message decoding tests passed");
