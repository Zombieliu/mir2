import assert from "node:assert/strict";
import net from "node:net";

import { selectChromeDebugPort } from "./cdp-debug-port.mjs";

const dynamicPort = await selectChromeDebugPort();
assert.equal(dynamicPort, 0);

const occupied = net.createServer();
await new Promise((resolve, reject) => {
  occupied.once("error", reject);
  occupied.listen({ host: "127.0.0.1", port: 0, exclusive: true }, resolve);
});
const occupiedAddress = occupied.address();
assert.equal(typeof occupiedAddress, "object");
const occupiedPort = occupiedAddress.port;

await assert.rejects(
  selectChromeDebugPort(occupiedPort),
  new RegExp(`Chrome debug port ${occupiedPort} is already in use`),
);
await new Promise((resolve, reject) => occupied.close((error) => (error ? reject(error) : resolve())));
assert.equal(await selectChromeDebugPort(occupiedPort), occupiedPort);

await assert.rejects(selectChromeDebugPort(0), /Invalid Chrome debug port/);
await assert.rejects(selectChromeDebugPort(65_536), /Invalid Chrome debug port/);
await assert.rejects(selectChromeDebugPort("not-a-port"), /Invalid Chrome debug port/);

console.log("CDP debug port tests passed");
