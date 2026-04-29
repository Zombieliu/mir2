import { execFile } from "node:child_process";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
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
const READY_TIMEOUT_MS = numberFromEnv("MIR2_WS_LOAD_READY_TIMEOUT_MS", 15_000);
const CLOSE_TIMEOUT_MS = numberFromEnv("MIR2_WS_LOAD_CLOSE_TIMEOUT_MS", 5_000);

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
    startedAt: new Date().toISOString(),
    finishedAt: null,
    durationMs: 0,
    opened: 0,
    ready: 0,
    closed: 0,
    errors: 0,
    messages: 0,
    commandsSent: 0,
    keepAliveLatenciesMs: [],
    clientFailures: [],
    rssSamples: [],
    rss: null,
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
          if (!result.ready) metrics.clientFailures.push(result);
        } catch (error) {
          metrics.errors += 1;
          metrics.clientFailures.push({ index, error: String(error?.message ?? error) });
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

  await fs.mkdir(path.dirname(OUTPUT_PATH), { recursive: true });
  await fs.writeFile(OUTPUT_PATH, `${JSON.stringify(metrics, null, 2)}\n`);
  console.log(`WS load completed: ready=${metrics.ready}/${CLIENTS}, errors=${metrics.errors}, messages=${metrics.messages}`);
  console.log(`Wrote ${OUTPUT_PATH}`);

  if (metrics.ready !== CLIENTS || metrics.errors > 0) {
    process.exitCode = 1;
  }
}

async function runClient(index, runId, metrics) {
  const accountId = `load-ws-${runId}-${index}`;
  const characterName = `Load${index}${runId.replace(/[^a-z0-9]/gi, "").slice(-6)}`;
  const password = "load-pass";
  const pendingKeepAlives = new Map();
  let loginSuccess = false;
  let userInformation = false;
  let createdCharacterIndex = null;

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
    if (payload.packet === "LoginSuccess") loginSuccess = true;
    if (payload.packet === "NewCharacterSuccess") {
      createdCharacterIndex = payload.payload?.character?.index ?? null;
    }
    if (payload.packet === "UserInformation") userInformation = true;
    const keepAliveTime = payload.payload?.time;
    if (payload.packet === "KeepAlive" && pendingKeepAlives.has(keepAliveTime)) {
      metrics.keepAliveLatenciesMs.push(Date.now() - pendingKeepAlives.get(keepAliveTime));
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
    send(ws, metrics, { type: "login", accountId, password });
    await waitFor(() => loginSuccess, READY_TIMEOUT_MS, `login ${index}`);
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
    send(ws, metrics, { type: "startGame", characterIndex: createdCharacterIndex });
    await waitFor(() => userInformation, READY_TIMEOUT_MS, `startGame ${index}`);
    metrics.ready += 1;

    const directions = ["Right", "Down", "Left", "Up"];
    for (let action = 0; action < ACTIONS; action += 1) {
      const time = Date.now() * 1000 + index * 100 + action;
      pendingKeepAlives.set(time, Date.now());
      send(ws, metrics, { type: "keepAlive", time });
      send(ws, metrics, { type: action % 3 === 0 ? "run" : "walk", direction: directions[action % directions.length] });
      if (action % 10 === 0) {
        send(ws, metrics, { type: "chat", message: `load ${index}:${action}` });
      }
      if (action % 15 === 0) {
        send(ws, metrics, {
          type: "stage5Command",
          action: "social.friend",
          args: [`load-peer-${action}`],
        });
      }
      await delay(THINK_MS);
    }

    await delay(250);
    return { index, ready: true };
  } finally {
    if (!ws.closed) {
      ws.close();
      await waitFor(() => ws.closed, CLOSE_TIMEOUT_MS, `close ${index}`).catch(() => {});
    }
  }
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
    if (this.url.protocol !== "ws:") {
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
  }

  async connect() {
    const port = Number(this.url.port || 80);
    const host = this.url.hostname;
    const pathName = `${this.url.pathname || "/"}${this.url.search || ""}`;
    const key = crypto.randomBytes(16).toString("base64");

    this.socket = net.createConnection({ host, port });
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
    this.socket.write(encodeClientFrame(0x1, Buffer.from(text, "utf8")));
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
  if (process.platform !== "win32") return null;
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

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
