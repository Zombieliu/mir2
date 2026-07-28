#!/usr/bin/env node

import crypto from "node:crypto";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import net from "node:net";
import path from "node:path";

const urls = (
  process.env.MIR2_GATE15_PLAYER_WS_URLS ??
  "ws://127.0.0.1:19710/ws,ws://127.0.0.1:19711/ws"
)
  .split(",")
  .map((value) => value.trim())
  .filter(Boolean);
if (urls.length !== 2) {
  throw new Error("MIR2_GATE15_PLAYER_WS_URLS must contain exactly two URLs");
}

const readyPath = path.resolve(
  process.env.MIR2_GATE15_PLAYERS_READY ?? "docs/generated/gate15/players-ready.json",
);
const markerPath = path.resolve(
  process.env.MIR2_GATE15_FAILOVER_MARKER ?? "docs/generated/gate15/failover.marker",
);
const outputPath = path.resolve(
  process.env.MIR2_GATE15_PLAYERS_OUT ?? "docs/generated/gate15/gate15-players.json",
);
const durationMs = numberEnv("MIR2_GATE15_PLAYER_DURATION_MS", 35_000);
const readyTimeoutMs = numberEnv("MIR2_GATE15_PLAYER_READY_TIMEOUT_MS", 20_000);

class RawWebSocket {
  constructor(url, onMessage, onClose) {
    this.url = new URL(url);
    this.onMessage = onMessage;
    this.onClose = onClose;
    this.buffer = Buffer.alloc(0);
    this.open = false;
    this.closed = false;
    this.bytesSent = 0;
    this.bytesReceived = 0;
  }

  async connect() {
    if (this.url.protocol !== "ws:") {
      throw new Error(`Gate 15 harness supports ws:// URLs, got ${this.url}`);
    }
    const port = Number(this.url.port || 80);
    this.socket = net.createConnection({ host: this.url.hostname, port });
    this.socket.setNoDelay(true);
    await new Promise((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`timeout connecting ${this.url}`)),
        readyTimeoutMs,
      );
      this.socket.once("connect", () => {
        clearTimeout(timer);
        resolve();
      });
      this.socket.once("error", reject);
    });
    const key = crypto.randomBytes(16).toString("base64");
    const request = [
      `GET ${this.url.pathname || "/"}${this.url.search || ""} HTTP/1.1`,
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
    this.socket.on("data", (chunk) => this.consume(chunk));
    this.socket.on("close", () => {
      this.open = false;
      this.closed = true;
      this.onClose();
    });
    if (leftover.length > 0) this.consume(leftover);
  }

  readHandshake() {
    return new Promise((resolve, reject) => {
      let data = Buffer.alloc(0);
      const timer = setTimeout(
        () => reject(new Error(`timeout handshaking ${this.url}`)),
        readyTimeoutMs,
      );
      const onData = (chunk) => {
        data = Buffer.concat([data, chunk]);
        const end = data.indexOf("\r\n\r\n");
        if (end < 0) return;
        cleanup();
        const header = data.subarray(0, end).toString("utf8");
        if (!/^HTTP\/1\.[01] 101 /.test(header)) {
          reject(new Error(`WebSocket handshake failed: ${header.split("\r\n")[0]}`));
          return;
        }
        resolve(data.subarray(end + 4));
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

  sendJson(value) {
    if (!this.open) throw new Error(`socket ${this.url} is closed`);
    const frame = encodeClientFrame(0x1, Buffer.from(JSON.stringify(value)));
    this.bytesSent += frame.length;
    this.socket.write(frame);
  }

  consume(chunk) {
    this.bytesReceived += chunk.length;
    this.buffer = Buffer.concat([this.buffer, chunk]);
    while (this.buffer.length >= 2) {
      const opcode = this.buffer[0] & 0x0f;
      const masked = (this.buffer[1] & 0x80) !== 0;
      let length = this.buffer[1] & 0x7f;
      let offset = 2;
      if (length === 126) {
        if (this.buffer.length < 4) return;
        length = this.buffer.readUInt16BE(2);
        offset = 4;
      } else if (length === 127) {
        if (this.buffer.length < 10) return;
        if (this.buffer.readUInt32BE(2) !== 0) {
          throw new Error("oversized WebSocket frame");
        }
        length = this.buffer.readUInt32BE(6);
        offset = 10;
      }
      let mask = null;
      if (masked) {
        if (this.buffer.length < offset + 4) return;
        mask = this.buffer.subarray(offset, offset + 4);
        offset += 4;
      }
      if (this.buffer.length < offset + length) return;
      let payload = this.buffer.subarray(offset, offset + length);
      this.buffer = this.buffer.subarray(offset + length);
      if (mask) {
        payload = Buffer.from(payload.map((byte, index) => byte ^ mask[index % 4]));
      }
      if (opcode === 0x1) this.onMessage(payload.toString("utf8"));
      if (opcode === 0x8) {
        this.close();
        return;
      }
      if (opcode === 0x9) {
        this.socket.write(encodeClientFrame(0x0a, payload));
      }
    }
  }

  close() {
    if (!this.socket || this.closed) return;
    if (this.open) this.socket.write(encodeClientFrame(0x08, Buffer.alloc(0)));
    this.socket.end();
  }
}

class Player {
  constructor(index, url) {
    this.index = index;
    this.url = url;
    this.accountId = `gate15-player-${index}`;
    this.characterName = `Gate15P${index}`;
    this.password = "gate15-pass";
    this.loginSuccess = false;
    this.characterIndex = null;
    this.inGame = false;
    this.closedUnexpectedly = false;
    this.messages = 0;
    this.errors = [];
    this.zoneResponses = 0;
    this.zoneResponsesAfterFailover = 0;
    this.markerSeen = false;
    this.socket = new RawWebSocket(
      url,
      (text) => this.onMessage(text),
      () => {
        if (!this.finished) this.closedUnexpectedly = true;
      },
    );
  }

  onMessage(text) {
    this.messages += 1;
    if (process.env.MIR2_GATE15_PLAYER_DEBUG === "1") {
      try {
        const debugMessage = JSON.parse(text);
        console.log(
          `[player-${this.index}] ${debugMessage.packet ?? debugMessage.type ?? "message"} ${
            debugMessage.message ?? ""
          }`,
        );
      } catch {
        console.log(`[player-${this.index}] non-json`);
      }
    }
    let message;
    try {
      message = JSON.parse(text);
    } catch {
      return;
    }
    if (message.packet === "LoginSuccess") {
      this.loginSuccess = true;
      const existing = message.payload?.characters?.[0]?.index;
      if (Number.isInteger(existing)) this.characterIndex = existing;
    }
    if (message.packet === "NewCharacterSuccess") {
      this.characterIndex = message.payload?.character?.index ?? 0;
    }
    if (message.packet === "UserInformation") this.inGame = true;
    if (message.packet === "UserLocation") {
      this.zoneResponses += 1;
      if (this.markerSeen) this.zoneResponsesAfterFailover += 1;
    }
    if (message.type === "error") {
      this.errors.push(String(message.message ?? "unknown Gateway error"));
    }
  }

  async start() {
    await this.socket.connect();
    this.socket.sendJson({ type: "clientVersion" });
    this.socket.sendJson({
      type: "newAccount",
      accountId: this.accountId,
      password: this.password,
      birthDateBinary: 0,
      userName: this.accountId,
      secretQuestion: "",
      secretAnswer: "",
      emailAddress: "",
    });
    this.socket.sendJson({
      type: "login",
      accountId: this.accountId,
      password: this.password,
    });
    await waitFor(() => this.loginSuccess, `login ${this.accountId}`);
    if (this.characterIndex === null) {
      this.socket.sendJson({
        type: "newCharacter",
        name: this.characterName,
        gender: this.index === 0 ? "Male" : "Female",
        class: this.index === 0 ? "Warrior" : "Wizard",
      });
      await waitFor(() => this.characterIndex !== null, `character ${this.accountId}`);
    }
    this.socket.sendJson({
      type: "startGame",
      characterIndex: this.characterIndex,
    });
    await waitFor(() => this.inGame, `StartGame ${this.accountId}`);
  }

  pulse(sequence) {
    if (!this.socket.open) return;
    this.markerSeen ||= fsSync.existsSync(markerPath);
    if (sequence % 2 === 0) {
      this.socket.sendJson({
        type: "turn",
        direction: ["Right", "Down", "Left", "Up"][sequence % 4],
      });
      return;
    }
    this.socket.sendJson({
      type: sequence % 3 === 0 ? "run" : "walk",
      direction: ["Right", "Down", "Left", "Up"][sequence % 4],
    });
  }

  summary() {
    return {
      index: this.index,
      url: this.url,
      accountId: this.accountId,
      characterIndex: this.characterIndex,
      inGame: this.inGame,
      markerSeen: this.markerSeen,
      closedUnexpectedly: this.closedUnexpectedly,
      messages: this.messages,
      zoneResponses: this.zoneResponses,
      zoneResponsesAfterFailover: this.zoneResponsesAfterFailover,
      errors: this.errors,
      bytesSent: this.socket.bytesSent,
      bytesReceived: this.socket.bytesReceived,
    };
  }

  finish() {
    this.finished = true;
    this.socket.close();
  }
}

async function main() {
  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await Promise.all(
    [readyPath, markerPath, outputPath].map((file) => fs.rm(file, { force: true })),
  );
  const players = [new Player(0, urls[0]), new Player(1, urls[1])];
  const startedAt = Date.now();
  try {
    // Sequential bootstrap keeps JSON account-store creation deterministic;
    // both players remain connected for the fault phase.
    await players[0].start();
    await players[1].start();
    await fs.writeFile(
      readyPath,
      `${JSON.stringify(
        {
          ready: true,
          pid: process.pid,
          players: players.map((player) => player.summary()),
        },
        null,
        2,
      )}\n`,
    );
    let sequence = 0;
    while (Date.now() - startedAt < durationMs) {
      for (const player of players) player.pulse(sequence++);
      await delay(200);
    }
  } finally {
    for (const player of players) player.finish();
  }
  const summaries = players.map((player) => player.summary());
  const assertions = {
    bothReachedGame: summaries.every((player) => player.inGame),
    bothObservedFailover: summaries.every((player) => player.markerSeen),
    bothStayedConnected: summaries.every((player) => !player.closedUnexpectedly),
    bothExecutedZoneCommandsAfterFailover: summaries.every(
      (player) => player.zoneResponsesAfterFailover > 0,
    ),
  };
  const report = {
    ok: Object.values(assertions).every(Boolean),
    startedAt: new Date(startedAt).toISOString(),
    finishedAt: new Date().toISOString(),
    durationMs: Date.now() - startedAt,
    assertions,
    players: summaries,
  };
  await fs.writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
  console.log(
    `Gate 15 players completed: postFailover=${summaries
      .map((player) => player.zoneResponsesAfterFailover)
      .join("/")} ok=${report.ok}`,
  );
  if (!report.ok) process.exitCode = 1;
}

function encodeClientFrame(opcode, payload) {
  const headerLength = payload.length < 126 ? 2 : payload.length <= 0xffff ? 4 : 10;
  const mask = crypto.randomBytes(4);
  const frame = Buffer.alloc(headerLength + 4 + payload.length);
  frame[0] = 0x80 | opcode;
  if (payload.length < 126) {
    frame[1] = 0x80 | payload.length;
  } else if (payload.length <= 0xffff) {
    frame[1] = 0x80 | 126;
    frame.writeUInt16BE(payload.length, 2);
  } else {
    frame[1] = 0x80 | 127;
    frame.writeUInt32BE(0, 2);
    frame.writeUInt32BE(payload.length, 6);
  }
  const maskOffset = headerLength;
  mask.copy(frame, maskOffset);
  for (let index = 0; index < payload.length; index += 1) {
    frame[maskOffset + 4 + index] = payload[index] ^ mask[index % 4];
  }
  return frame;
}

async function waitFor(predicate, label) {
  const deadline = Date.now() + readyTimeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await delay(25);
  }
  throw new Error(`Timed out waiting for ${label}`);
}

function numberEnv(name, fallback) {
  const value = Number(process.env[name] ?? "");
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
