#!/usr/bin/env node

// Creates an authoritative empty-roster account for same-scene Crystal
// character-select visual captures. The native client then logs in with the
// returned credentials and deliberately omits MIR2_NATIVE_CHARACTER_INDEX.

let cli;
try {
  cli = parseCli(process.argv.slice(2));
} catch (error) {
  console.error(JSON.stringify({ ok: false, status: "BLOCKED", error: String(error?.message ?? error), desktopTouched: false }, null, 2));
  process.exitCode = 1;
  process.exit();
}
if (cli.mode === "help") {
  printHelp();
  process.exit(0);
}
if (cli.mode === "self-test" || cli.mode === "dry-run") {
  console.log(JSON.stringify({
    ok: true,
    status: cli.mode === "self-test" ? "HANDOFF" : "CONFIRM_REQUIRED",
    mode: cli.mode,
    gatewayUrl: cli.gatewayUrl,
    desktopTouched: false,
    accountMutation: "not executed",
  }, null, 2));
  process.exit(0);
}
if (!cli.allowAccountMutation) {
  console.error(JSON.stringify({
    ok: false,
    status: "CONFIRM_REQUIRED",
    error: "This fixture creates an account; pass --allow-account-mutation explicitly.",
    desktopTouched: false,
  }, null, 2));
  process.exitCode = 2;
  process.exit();
}

const gatewayUrl = cli.gatewayUrl;
const timeoutMs = cli.timeoutMs;
const runToken = `${Date.now()}${process.pid}`.replace(/[^0-9]/g, "");
const accountId = (process.env.MIR2_NATIVE_FIXTURE_ACCOUNT ?? `ns${runToken}`).slice(0, 18);
const password = process.env.MIR2_NATIVE_FIXTURE_PASSWORD ?? "native-pass";
const socket = new WebSocket(gatewayUrl);

const timeout = setTimeout(() => {
  socket.close();
  fail(`timed out after ${timeoutMs}ms`);
}, timeoutMs);

try {
  await waitForEvent(socket, "open");
  send({ type: "clientVersion" });
  await waitForPacket("ClientVersion");

  send({
    type: "newAccount",
    accountId,
    password,
    birthDateBinary: 0,
    userName: accountId,
    secretQuestion: "",
    secretAnswer: "",
    emailAddress: "",
  });
  const accountReply = await waitForPacket("NewAccount");
  if (Number(accountReply.payload?.result) !== 8) {
    fail(`NewAccount rejected with result ${accountReply.payload?.result ?? "missing"}`);
  }

  send({ type: "login", accountId, password });
  const loginReply = await waitForPacket("LoginSuccess");
  const characters = loginReply.payload?.characters;
  if (!Array.isArray(characters) || characters.length !== 0) {
    fail(`expected an empty roster, received ${Array.isArray(characters) ? characters.length : "invalid"}`);
  }

  clearTimeout(timeout);
  socket.close();
  process.stdout.write(`${JSON.stringify({ status: "HANDOFF", gatewayUrl, accountId, password, characterCount: 0, desktopTouched: false, accountMutation: true })}\n`);
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}

function send(message) {
  socket.send(JSON.stringify(message));
}

function waitForEvent(target, type) {
  return new Promise((resolve, reject) => {
    const onEvent = (event) => {
      cleanup();
      resolve(event);
    };
    const onError = (event) => {
      cleanup();
      reject(event.error ?? new Error(`WebSocket ${type} failed`));
    };
    const cleanup = () => {
      target.removeEventListener(type, onEvent);
      target.removeEventListener("error", onError);
    };
    target.addEventListener(type, onEvent, { once: true });
    target.addEventListener("error", onError, { once: true });
  });
}

function waitForPacket(packetName) {
  return new Promise((resolve, reject) => {
    const onMessage = (event) => {
      let message;
      try {
        message = JSON.parse(String(event.data));
      } catch {
        return;
      }
      if (message.type !== "packet" || message.packet !== packetName) return;
      cleanup();
      resolve(message);
    };
    const onError = (event) => {
      cleanup();
      reject(event.error ?? new Error(`waiting for ${packetName} failed`));
    };
    const cleanup = () => {
      socket.removeEventListener("message", onMessage);
      socket.removeEventListener("error", onError);
    };
    socket.addEventListener("message", onMessage);
    socket.addEventListener("error", onError, { once: true });
  });
}

function fail(message) {
  clearTimeout(timeout);
  process.stderr.write(`${JSON.stringify({ ok: false, status: "BLOCKED", error: `prepare-native-select-fixture: ${message}`, desktopTouched: false })}\n`);
  process.exitCode = 1;
}

function parseCli(argv) {
  const args = {
    gatewayUrl: process.env.MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:7110/ws",
    timeoutMs: process.env.MIR2_NATIVE_FIXTURE_TIMEOUT_MS ?? 10_000,
    allowAccountMutation: false,
    mode: "run",
  };
  let positionalGateway = null;
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--help" || token === "-h") { args.mode = "help"; continue; }
    if (token === "--self-test") { args.mode = "self-test"; continue; }
    if (token === "--dry-run") { args.mode = "dry-run"; continue; }
    if (token === "--allow-account-mutation") { args.allowAccountMutation = true; continue; }
    if (token === "--gateway-url" || token === "--timeout-ms") {
      const value = argv[++index];
      if (value === undefined || value === "" || value.startsWith("--")) throw new Error(`${token} requires a value`);
      if (token === "--gateway-url") args.gatewayUrl = value;
      else args.timeoutMs = value;
      continue;
    }
    if (token.startsWith("--")) throw new Error(`unknown argument: ${token}`);
    if (positionalGateway !== null) throw new Error(`unexpected positional argument: ${token}`);
    positionalGateway = token;
  }
  if (positionalGateway !== null) args.gatewayUrl = positionalGateway;
  let parsedUrl;
  try { parsedUrl = new URL(String(args.gatewayUrl)); } catch { throw new Error(`gateway URL is invalid: ${args.gatewayUrl}`); }
  if (!["ws:", "wss:"].includes(parsedUrl.protocol)) throw new Error(`gateway URL must use ws or wss: ${args.gatewayUrl}`);
  args.gatewayUrl = parsedUrl.toString();
  args.timeoutMs = Number(args.timeoutMs);
  if (!Number.isSafeInteger(args.timeoutMs) || args.timeoutMs < 1000) throw new Error("--timeout-ms must be an integer >= 1000");
  return args;
}

function printHelp() {
  console.log(`Usage:
  node apps/game-client/platform-windows/scripts/prepare-native-select-fixture.mjs [gateway-url] [options]

Options:
  --gateway-url URL          Gateway ws(s) URL
  --timeout-ms N             Fixture timeout (default: 10000)
  --dry-run                  Validate only; do not open a socket
  --self-test                Validate only; do not open a socket
  --allow-account-mutation   Explicitly permit account creation

Safety: this fixture creates a disposable account but never deletes an account,
changes a password, or controls the desktop. Live mode is CONFIRM_REQUIRED.`);
}
