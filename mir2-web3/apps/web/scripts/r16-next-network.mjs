/** Loaded only by the local Next child through Node's explicit --import option.
 * Wallet relay HTTPS uses the machine proxy and trusted CAs. Loopback and all
 * unrelated fetches retain native transport; no model settings are read here.
 */
import { createRelayHttpTransport } from "../../ai-player/src/relay-http-transport.mjs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

// Merely importing the runtime closure (including packaging preflight) is inert.
// The service supplies this exact absolute path as an explicit Node preload.
const ownPath = fileURLToPath(import.meta.url);
const isOwnPreload = value => {
  if (typeof value !== "string") return false;
  try { return (value.startsWith("file:") ? fileURLToPath(value) : resolve(value)) === ownPath; }
  catch { return false; }
};
if (process.execArgv.some((arg, index) => arg === "--import" && typeof process.execArgv[index + 1] === "string"
  && isOwnPreload(process.execArgv[index + 1]))) {
  const transport = createRelayHttpTransport();
  globalThis.fetch = transport.fetch;
  process.once("beforeExit", () => { void transport.close().catch(() => {}); });
}
