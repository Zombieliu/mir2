import { execFile } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import tls from "node:tls";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);

const WS_URL = process.env.MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:7110/ws";
const OUTPUT_PATH = path.resolve(
  process.cwd(),
  "..",
  "..",
  process.env.MIR2_WS_LOAD_OUT ?? "docs/generated/load/latest-ws.json",
);
const CLIENTS = numberFromEnv("MIR2_WS_LOAD_CLIENTS", 64);
const POOL = numberFromEnv("MIR2_WS_LOAD_POOL", Math.min(32, CLIENTS));
const ACTIONS = numberFromEnv("MIR2_WS_LOAD_ACTIONS", 20);
const THINK_MS = numberFromEnv("MIR2_WS_LOAD_THINK_MS", 20);
const HOLD_OPEN_MS = numberFromEnv("MIR2_WS_LOAD_HOLD_OPEN_MS", 250);
const READY_TIMEOUT_MS = numberFromEnv("MIR2_WS_LOAD_READY_TIMEOUT_MS", 15_000);
const CLOSE_TIMEOUT_MS = numberFromEnv("MIR2_WS_LOAD_CLOSE_TIMEOUT_MS", 5_000);
const EXPECT_READY = optionalNumberFromEnv("MIR2_WS_LOAD_EXPECT_READY");
const EXPECT_REJECTED = optionalNumberFromEnv("MIR2_WS_LOAD_EXPECT_REJECTED");
const ENABLE_STAGE5_COMMANDS = booleanFromEnv(
  "MIR2_WS_LOAD_ENABLE_STAGE5_COMMANDS",
  false,
);
const GATEWAY_PID = optionalNumberFromEnv("MIR2_GATEWAY_PID");
const REUSE_EXISTING_ACCOUNTS = booleanFromEnv(
  "MIR2_WS_LOAD_REUSE_EXISTING_ACCOUNTS",
  false,
);
const ACCOUNT_PREFIX = process.env.MIR2_WS_LOAD_ACCOUNT_PREFIX ?? null;
const CHARACTER_INDEX = numberFromEnv("MIR2_WS_LOAD_CHARACTER_INDEX", 0);

async function main() {
  const runId = `${Date.now().toString(36)}-${process.pid}`;
  const metrics = {
    type: "websocket",
    wsUrl: WS_URL,
    runId,
    host: os.hostname(),
    clients: CLIENTS,
    pool: POOL,
    actionsPerClient: ACTIONS,
    thinkMs: THINK_MS,
    holdOpenMs: HOLD_OPEN_MS,
    expectedReady: EXPECT_READY ?? CLIENTS,
    expectedCapacityRejected: EXPECT_REJECTED ?? 0,
    stage5CommandsEnabled: ENABLE_STAGE5_COMMANDS,
    reuseExistingAccounts: REUSE_EXISTING_ACCOUNTS,
    accountPrefix: ACCOUNT_PREFIX,
    characterIndex: CHARACTER_INDEX,
    startedAt: new Date().toISOString(),
    finishedAt: null,
    durationMs: 0,
    opened: 0,
    ready: 0,
    loginSuccess: 0,
    charactersCreated: 0,
    startedGames: 0,
    closed: 0,
    errors: 0,
    capacityRejected: 0,
    messages: 0,
    commandsSent: 0,
    keepAliveCommandsSent: 0,
    movementCommandsSent: 0,
    chatCommandsSent: 0,
    stage5CommandsSent: 0,
    keepAliveLatenciesMs: [],
    clientFailures: [],
    capacityRejections: [],
    network: {
      clientBytesSent: 0,
      clientBytesReceived: 0,
      bootstrapClientBytesSent: 0,
      bootstrapClientBytesReceived: 0,
      playClientBytesSent: 0,
      playClientBytesReceived: 0,
      playDurationMs: 0,
      avgPlayBytesPerSecondPerReadyClient: 0,
      avgPlayBitsPerSecondPerReadyClient: 0,
    },
    rssSamples: [],
    rss: null,
    cpuPercent: null,
    assertions: null,
    ok: false,
  };

  let sampling = true;
  const sampler = sampleGatewayRss(metrics, () => sampling);
  const started = Date.now();
  try {
    await runPool(
      Array.from({ length: CLIENTS }, (_, index) => index),
      POOL,
      async (index) => {
        try {
          const result = await runClient(index, runId, metrics);
          if (result.capacityRejected) {
            metrics.capacityRejected += 1;
            metrics.capacityRejections.push(result);
          } else if (!result.ready) {
            metrics.clientFailures.push(result);
          }
        } catch (error) {
          const failure = clientFailureFromError(index, error);
          if (failure.capacityRejected) {
            metrics.capacityRejected += 1;
            metrics.capacityRejections.push(failure);
          } else {
            metrics.errors += 1;
            metrics.clientFailures.push(failure);
          }
        }
      },
    );
  } finally {
    sampling = false;
    await sampler;
  }

  metrics.finishedAt = new Date().toISOString();
  metrics.durationMs = Date.now() - started;
  metrics.keepAlive = summarize(metrics.keepAliveLatenciesMs);
  metrics.rss = summarize(metrics.rssSamples.map((sample) => sample.workingSetBytes));
  metrics.cpuPercent = summarize(metrics.rssSamples.map((sample) => sample.cpuPercent));
  if (metrics.ready > 0 && metrics.network.playDurationMs > 0) {
    metrics.network.avgPlayBytesPerSecondPerReadyClient =
      Math.round(
        (metrics.network.playClientBytesReceived /
          (metrics.network.playDurationMs / 1000) /
          metrics.ready) *
          100,
      ) / 100;
    metrics.network.avgPlayBitsPerSecondPerReadyClient =
      Math.round(metrics.network.avgPlayBytesPerSecondPerReadyClient * 8 * 100) / 100;
  }
  metrics.assertions = {
    readyMatchesExpectation: metrics.ready === metrics.expectedReady,
    capacityRejectedMatchesExpectation:
      metrics.capacityRejected === metrics.expectedCapacityRejected,
    noUnexpectedErrors: metrics.errors === 0,
    noUnexpectedClientFailures: metrics.clientFailures.length === 0,
    gameplayCommandsSentWhenReady:
      metrics.ready === 0 ||
      (metrics.keepAliveCommandsSent >= metrics.ready &&
        metrics.movementCommandsSent >= metrics.ready &&
        metrics.chatCommandsSent >= metrics.ready),
  };
  metrics.ok = Object.values(metrics.assertions).every(Boolean);

  await fs.mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
  await fs.writeFile(OUTPUT_PATH, `${JSON.stringify(metrics, null, 2)}\n`);
  console.log(
    `WS load completed: ready=${metrics.ready}/${CLIENTS}, capacityRejected=${metrics.capacityRejected}, errors=${metrics.errors}, messages=${metrics.messages}, ok=${metrics.ok}`,
  );
  console.log(`Wrote ${OUTPUT_PATH}`);

  if (!metrics.ok) {
    process.exitCode = 1;
  }
}

async function runClient(index, runId, metrics) {
  const accountId =
    ACCOUNT_PREFIX === null
      ? `load-ws-${runId}-${index}`
      : `${ACCOUNT_PREFIX}${index}`;
  const characterName = `Load${index}${runId.replace(/[^a-z0-9]/gi, "").slice(-6)}`.slice(
    0,
    10,
  );
  const password = "load-pass";
  const pendingKeepAlives = new Map();
  let loginSuccess = false;
  let userInformation = false;
  let createdCharacterIndex = null;
  let capacityRejected = false;
  let capacityError = null;

  const ws = new RawWebSocketClient(
    WS_URL,
    (text) => {
      metrics.messages += 1;
      let payload;
      try {
        payload = JSON.parse(text);
      } catch {
        return;
      }
      if (payload.type === "error") {
        const message = String(payload.message ?? "");
        if (isCapacityError(message)) {
          capacityRejected = true;
          capacityError = message;
        }
      }
      if (payload.packet === "LoginSuccess" && !loginSuccess) {
        loginSuccess = true;
        metrics.loginSuccess += 1;
      }
      if (payload.packet === "NewCharacterSuccess") {
        createdCharacterIndex = payload.payload?.character?.index ?? null;
        metrics.charactersCreated += 1;
      }
      if (payload.packet === "UserInformation" && !userInformation) {
        userInformation = true;
        metrics.startedGames += 1;
      }
      const keepAliveTime = payload.payload?.time;
      if (payload.packet === "KeepAlive" && pendingKeepAlives.has(keepAliveTime)) {
        metrics.keepAliveLatenciesMs.push(
          Date.now() - pendingKeepAlives.get(keepAliveTime),
        );
        pendingKeepAlives.delete(keepAliveTime);
      }
    },
    () => {
      metrics.errors += 1;
    },
    () => {
      metrics.closed += 1;
    },
  );

  try {
    await ws.connect();
    metrics.opened += 1;

    send(ws, metrics, { type: "clientVersion" });
    if (!REUSE_EXISTING_ACCOUNTS) {
      send(ws, metrics, {
        type: "newAccount",
        accountId,
        password,
        birthDateBinary: 0,
        userName: accountId,
        secretQuestion: "",
        secretAnswer: "",
        emailAddress: "",
      });
    }
    send(ws, metrics, { type: "login", accountId, password });
    await waitFor(() => loginSuccess, READY_TIMEOUT_MS, `login ${index}`);
    if (REUSE_EXISTING_ACCOUNTS) {
      createdCharacterIndex = CHARACTER_INDEX;
    } else {
      send(ws, metrics, {
        type: "newCharacter",
        name: characterName,
        gender: index % 2 === 0 ? "Male" : "Female",
        class: ["Warrior", "Wizard", "Taoist"][index % 3],
      });
      await waitFor(
        () => createdCharacterIndex !== null,
        READY_TIMEOUT_MS,
        `newCharacter ${index}`,
      );
    }
    send(ws, metrics, { type: "startGame", characterIndex: createdCharacterIndex });
    await waitFor(
      () => userInformation || capacityRejected,
      READY_TIMEOUT_MS,
      `startGame ${index}`,
    );
    if (capacityRejected) {
      return {
        index,
        ready: false,
        capacityRejected: true,
        error: capacityError,
      };
    }
    const bytesSentAtReady = ws.bytesSent;
    const bytesReceivedAtReady = ws.bytesReceived;
    const playStartedAt = Date.now();
    metrics.ready += 1;

    const directions = ["Right", "Down", "Left", "Up"];
    for (let action = 0; action < ACTIONS; action += 1) {
      const time = Date.now() * 1000 + index * 100 + action;
      pendingKeepAlives.set(time, Date.now());
      if (send(ws, metrics, { type: "keepAlive", time })) {
        metrics.keepAliveCommandsSent += 1;
      }
      if (
        send(ws, metrics, {
          type: action % 3 === 0 ? "run" : "walk",
          direction: directions[action % directions.length],
        })
      ) {
        metrics.movementCommandsSent += 1;
      }
      if (action % 10 === 0) {
        if (send(ws, metrics, { type: "chat", message: `load ${index}:${action}` })) {
          metrics.chatCommandsSent += 1;
        }
      }
      if (ENABLE_STAGE5_COMMANDS && action % 15 === 0) {
        if (send(ws, metrics, {
          type: "stage5Command",
          action: "social.friend",
          args: [`load-peer-${action}`],
        })) {
          metrics.stage5CommandsSent += 1;
        }
      }
      await delay(THINK_MS);
    }

    await delay(HOLD_OPEN_MS);
    recordClientNetworkMetrics(metrics, ws, {
      bytesSentAtReady,
      bytesReceivedAtReady,
      playStartedAt,
      playEndedAt: Date.now(),
    });
    return { index, ready: true };
  } finally {
    if (!ws.closed) {
      ws.close();
      await waitFor(() => ws.closed, CLOSE_TIMEOUT_MS, `close ${index}`).catch(() => {});
    }
  }
}

function recordClientNetworkMetrics(
  metrics,
  ws,
  { bytesSentAtReady, bytesReceivedAtReady, playStartedAt, playEndedAt },
) {
  metrics.network.clientBytesSent += ws.bytesSent;
  metrics.network.clientBytesReceived += ws.bytesReceived;
  metrics.network.bootstrapClientBytesSent += bytesSentAtReady;
  metrics.network.bootstrapClientBytesReceived += bytesReceivedAtReady;
  metrics.network.playClientBytesSent += Math.max(0, ws.bytesSent - bytesSentAtReady);
  metrics.network.playClientBytesReceived += Math.max(
    0,
    ws.bytesReceived - bytesReceivedAtReady,
  );
  metrics.network.playDurationMs += Math.max(0, playEndedAt - playStartedAt);
}

function clientFailureFromError(index, error) {
  const message = String(error?.message ?? error);
  return {
    index,
    error: message,
    capacityRejected: isCapacityError(message),
  };
}

function isCapacityError(message) {
  return /\bcapacity\b|503|service unavailable/i.test(message);
}

function send(ws, metrics, command) {
  if (!ws.open) return false;
  ws.sendJson(command);
  metrics.commandsSent += 1;
  return true;
}

class RawWebSocketClient {
  constructor(url, onMessage, onError, onClose) {
    this.url = new URL(url);
    if (this.url.protocol !== "ws:" && this.url.protocol !== "wss:") {
      throw new Error(`Unsupported WebSocket protocol for load harness: ${this.url.protocol}`);
    }
    this.onMessage = onMessage;
    this.onError = onError;
    this.onClose = onClose;
    this.socket = null;
    this.buffer = Buffer.alloc(0);
    this.open = false;
    this.closed = false;
    this.errorReported = false;
    this.bytesSent = 0;
    this.bytesReceived = 0;
  }

  async connect() {
    const secure = this.url.protocol === "wss:";
    const port = Number(this.url.port || (secure ? 443 : 80));
    const host = this.url.hostname;
    const pathName = `${this.url.pathname || "/"}${this.url.search || ""}`;
    const key = crypto.randomBytes(16).toString("base64");

    this.socket = secure
      ? tls.connect({
          host,
          port,
          servername: host,
        })
      : net.createConnection({ host, port });
    this.socket.setNoDelay(true);
    this.socket.on("error", () => this.reportError());
    this.socket.on("close", () => {
      this.open = false;
      this.closed = true;
      this.onClose();
    });

    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`Timed out waiting for tcp open ${host}:${port}`)), READY_TIMEOUT_MS);
      this.socket.once("connect", () => {
        clearTimeout(timer);
        resolve();
      });
      this.socket.once("error", reject);
    });

    const request = [
      `GET ${pathName} HTTP/1.1`,
      `Host: ${this.url.host}`,
      "Upgrade: websocket",
      "Connection: Upgrade",
      `Sec-WebSocket-Key: ${key}`,
      "Sec-WebSocket-Version: 13",
      "\r\n",
    ].join("\r\n");
    this.socket.write(request);

    const leftover = await this.readHandshake();
    this.open = true;
    this.socket.on("data", (chunk) => this.consumeFrames(chunk));
    if (leftover.length > 0) {
      this.consumeFrames(leftover);
    }
  }

  readHandshake() {
    return new Promise((resolve, reject) => {
      let data = Buffer.alloc(0);
      const timer = setTimeout(() => {
        cleanup();
        reject(new Error("Timed out waiting for WebSocket handshake"));
      }, READY_TIMEOUT_MS);
      const onData = (chunk) => {
        data = Buffer.concat([data, chunk]);
        const headerEnd = data.indexOf("\r\n\r\n");
        if (headerEnd === -1) return;
        const header = data.slice(0, headerEnd).toString("utf8");
        cleanup();
        if (!header.startsWith("HTTP/1.1 101") && !header.startsWith("HTTP/1.0 101")) {
          reject(new Error(`WebSocket handshake failed: ${header.split("\r\n")[0]}`));
          return;
        }
        resolve(data.slice(headerEnd + 4));
      };
      const onError = (error) => {
        cleanup();
        reject(error);
      };
      const cleanup = () => {
        clearTimeout(timer);
        this.socket.off("data", onData);
        this.socket.off("error", onError);
      };
      this.socket.on("data", onData);
      this.socket.on("error", onError);
    });
  }

  sendJson(command) {
    this.sendText(JSON.stringify(command));
  }

  sendText(text) {
    if (!this.open || !this.socket) return;
    const frame = encodeClientFrame(0x1, Buffer.from(text, "utf8"));
    this.bytesSent += frame.length;
    this.socket.write(frame);
  }

  close() {
    if (!this.socket || this.closed) return;
    try {
      if (this.open) {
        this.socket.write(encodeClientFrame(0x8, Buffer.alloc(0)));
      }
      this.socket.end();
      this.socket.destroy();
    } catch {
      this.socket.destroy();
    }
  }

  consumeFrames(chunk) {
    this.bytesReceived += chunk.length;
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 2) {
      const first = this.buffer[0];
      const second = this.buffer[1];
      const opcode = first & 0x0f;
      const masked = (second & 0x80) !== 0;
      let length = second & 0x7f;
      let offset = 2;
      if (length === 126) {
        if (this.buffer.length < offset + 2) return;
        length = this.buffer.readUInt16BE(offset);
        offset += 2;
      } else if (length === 127) {
        if (this.buffer.length < offset + 8) return;
        const high = this.buffer.readUInt32BE(offset);
        const low = this.buffer.readUInt32BE(offset + 4);
        if (high !== 0) {
          this.reportError();
          this.close();
          return;
        }
        length = low;
        offset += 8;
      }

      let mask;
      if (masked) {
        if (this.buffer.length < offset + 4) return;
        mask = this.buffer.slice(offset, offset + 4);
        offset += 4;
      }
      if (this.buffer.length < offset + length) return;

      let payload = this.buffer.slice(offset, offset + length);
      this.buffer = this.buffer.slice(offset + length);
      if (masked && mask) {
        payload = Buffer.from(payload.map((byte, index) => byte ^ mask[index % 4]));
      }

      if (opcode === 0x1) {
        this.onMessage(payload.toString("utf8"));
      } else if (opcode === 0x8) {
        this.close();
        return;
      } else if (opcode === 0x9) {
        this.socket.write(encodeClientFrame(0xa, payload));
      }
    }
  }

  reportError() {
    if (this.errorReported) return;
    this.errorReported = true;
    this.onError();
  }
}

function encodeClientFrame(opcode, payload) {
  const length = payload.length;
  const headerLength = length < 126 ? 2 : length <= 0xffff ? 4 : 10;
  const mask = crypto.randomBytes(4);
  const frame = Buffer.alloc(headerLength + 4 + length);
  frame[0] = 0x80 | opcode;
  if (length < 126) {
    frame[1] = 0x80 | length;
  } else if (length <= 0xffff) {
    frame[1] = 0x80 | 126;
    frame.writeUInt16BE(length, 2);
  } else {
    frame[1] = 0x80 | 127;
    frame.writeUInt32BE(0, 2);
    frame.writeUInt32BE(length, 6);
  }
  const maskOffset = headerLength;
  mask.copy(frame, maskOffset);
  for (let index = 0; index < length; index += 1) {
    frame[maskOffset + 4 + index] = payload[index] ^ mask[index % 4];
  }
  return frame;
}

async function runPool(items, poolSize, worker) {
  let cursor = 0;
  const workers = Array.from({ length: Math.max(1, poolSize) }, async () => {
    while (cursor < items.length) {
      const next = cursor;
      cursor += 1;
      await worker(items[next]);
    }
  });
  await Promise.all(workers);
}

async function sampleGatewayRss(metrics, keepGoing) {
  while (keepGoing()) {
    const sample = await gatewayRssSample();
    if (sample) metrics.rssSamples.push(sample);
    await delay(500);
  }
  const sample = await gatewayRssSample();
  if (sample) metrics.rssSamples.push(sample);
}

async function gatewayRssSample() {
  if (process.platform !== "win32") return gatewayProcessSample();
  const command = [
    "$p = Get-Process -Name mir2-gateway -ErrorAction SilentlyContinue | Sort-Object StartTime -Descending | Select-Object -First 1;",
    "if ($p) {",
    "  [Console]::WriteLine(($p.Id.ToString() + ',' + $p.WorkingSet64.ToString() + ',' + $p.HandleCount.ToString()))",
    "}",
  ].join(" ");
  try {
    const { stdout } = await execFileAsync("powershell", ["-NoProfile", "-Command", command], { windowsHide: true });
    const [pid, workingSetBytes, handleCount] = stdout.trim().split(",").map((value) => Number(value));
    if (!pid || !workingSetBytes) return null;
    return { atUnixMs: Date.now(), pid, workingSetBytes, handleCount };
  } catch {
    return null;
  }
}

async function gatewayProcessSample() {
  const pid = GATEWAY_PID ?? (await newestProcessPid("mir2-gateway"));
  if (!pid) return null;
  try {
    const { stdout } = await execFileAsync("ps", [
      "-o",
      "pid=",
      "-o",
      "rss=",
      "-o",
      "%cpu=",
      "-p",
      String(pid),
    ]);
    const [samplePid, rssKb, cpuPercent] = stdout.trim().split(/\s+/).map(Number);
    if (!samplePid || !rssKb) return null;
    return {
      atUnixMs: Date.now(),
      pid: samplePid,
      workingSetBytes: rssKb * 1024,
      cpuPercent: Number.isFinite(cpuPercent) ? cpuPercent : null,
    };
  } catch {
    return null;
  }
}

async function newestProcessPid(name) {
  try {
    const { stdout } = await execFileAsync("pgrep", ["-n", name]);
    const pid = Number(stdout.trim());
    return Number.isFinite(pid) && pid > 0 ? pid : null;
  } catch {
    return null;
  }
}

async function waitFor(predicate, timeoutMs, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await delay(25);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

function summarize(values) {
  const clean = values.filter((value) => Number.isFinite(value)).sort((a, b) => a - b);
  if (clean.length === 0) return { count: 0, min: null, max: null, avg: null, p95: null };
  const sum = clean.reduce((total, value) => total + value, 0);
  return {
    count: clean.length,
    min: clean[0],
    max: clean[clean.length - 1],
    avg: Math.round((sum / clean.length) * 100) / 100,
    p95: clean[Math.min(clean.length - 1, Math.floor(clean.length * 0.95))],
  };
}

function numberFromEnv(name, fallback) {
  const value = Number(process.env[name]);
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function optionalNumberFromEnv(name) {
  const raw = process.env[name];
  if (raw === undefined || raw === "") return null;
  const value = Number(raw);
  return Number.isFinite(value) && value >= 0 ? value : null;
}

function booleanFromEnv(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw === "") return fallback;
  return /^(1|true|yes|on)$/i.test(raw);
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
