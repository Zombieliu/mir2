#!/usr/bin/env node

import crypto from "node:crypto";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const WebSocket = require(process.env.MIR2_WS_MODULE ?? "ws");

const gatewayUrl = process.env.MIR2_GATEWAY_URL ?? "https://127.0.0.1:7110";
const wsUrl = process.env.MIR2_GATEWAY_WS_URL ?? gatewayUrl.replace(/^http/, "ws") + "/ws";
const origin = process.env.MIR2_WEB_ORIGIN ?? "https://mir2.obelisk.build";
const timeoutMs = Number(process.env.MIR2_ACCEPTANCE_TIMEOUT_MS ?? 20_000);
const accountId = `ci_${Date.now()}`;
const characterName = `CI${String(Date.now()).slice(-8)}`;
const originalPassword = `Aa1!${crypto.randomBytes(12).toString("hex")}`;
const recoveredPassword = `Bb2!${crypto.randomBytes(12).toString("hex")}`;

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function connect() {
  return new Promise((resolve, reject) => {
    const socket = new WebSocket(wsUrl, { headers: { Origin: origin } });
    const messages = [];
    const waiters = [];
    const timer = setTimeout(() => reject(new Error("WebSocket connection timed out")), timeoutMs);
    socket.on("open", () => {
      clearTimeout(timer);
      resolve({ socket, messages, waiters });
    });
    socket.on("message", (data) => {
      let message;
      try {
        message = JSON.parse(data.toString());
      } catch {
        return;
      }
      messages.push(message);
      for (const waiter of [...waiters]) waiter(message);
    });
    socket.on("error", reject);
  });
}

function send(client, value) {
  client.socket.send(JSON.stringify(value));
}

function waitFor(client, predicate, label) {
  const existing = client.messages.find(predicate);
  if (existing) return Promise.resolve(existing);
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      client.waiters.splice(client.waiters.indexOf(onMessage), 1);
      const gatewayError = [...client.messages].reverse().find((message) => message.type === "error");
      reject(new Error(`${label} timed out${gatewayError ? `: ${gatewayError.message}` : ""}`));
    }, timeoutMs);
    const onMessage = (message) => {
      if (!predicate(message)) return;
      clearTimeout(timer);
      client.waiters.splice(client.waiters.indexOf(onMessage), 1);
      resolve(message);
    };
    client.waiters.push(onMessage);
  });
}

async function request(path, options = {}) {
  const response = await fetch(`${gatewayUrl}${path}`, options);
  const text = await response.text();
  let body = null;
  if (text) {
    try {
      body = JSON.parse(text);
    } catch {
      body = text;
    }
  }
  return { response, body };
}

function bearer(token) {
  return { Authorization: `Bearer ${token}`, "Content-Type": "application/json" };
}

async function login(client, password) {
  send(client, { type: "clientVersion" });
  await waitFor(client, (message) => message.packet === "ClientVersion", "ClientVersion");
  send(client, { type: "login", accountId, password });
  const [login, identity] = await Promise.all([
    waitFor(client, (message) => message.packet === "LoginSuccess", "LoginSuccess"),
    waitFor(client, (message) => message.type === "identitySession", "identitySession"),
  ]);
  return { login, identity };
}

async function startGame(client, characterIndex) {
  send(client, { type: "startGame", characterIndex });
  const start = await waitFor(client, (message) => message.packet === "StartGame", "StartGame");
  if (start.payload?.result !== 4) throw new Error(`StartGame rejected with result ${start.payload?.result}`);
  await waitFor(client, (message) => message.packet === "UserInformation", "UserInformation");
}

async function main() {
  const first = await connect();
  send(first, { type: "clientVersion" });
  await waitFor(first, (message) => message.packet === "ClientVersion", "ClientVersion");
  send(first, {
    type: "newAccount",
    accountId,
    password: originalPassword,
    birthDateBinary: 0,
    userName: accountId,
    secretQuestion: "",
    secretAnswer: "",
    emailAddress: "",
  });
  const created = await waitFor(first, (message) => message.packet === "NewAccount", "NewAccount");
  if (created.payload?.result !== 8) throw new Error(`NewAccount rejected with result ${created.payload?.result}`);

  send(first, { type: "login", accountId, password: originalPassword });
  const [firstLogin, firstIdentity] = await Promise.all([
    waitFor(first, (message) => message.packet === "LoginSuccess", "first LoginSuccess"),
    waitFor(first, (message) => message.type === "identitySession", "first identitySession"),
  ]);
  if (firstLogin.payload?.characters?.length) throw new Error("fresh account unexpectedly has characters");

  send(first, { type: "newCharacter", name: characterName, gender: "Male", class: "Warrior" });
  const character = await waitFor(first, (message) => message.packet === "NewCharacterSuccess", "NewCharacterSuccess");
  const characterIndex = character.payload?.character?.index;
  if (!Number.isInteger(characterIndex)) throw new Error("character index is missing");
  await startGame(first, characterIndex);

  const initialOverview = await request("/v1/identity/me", {
    headers: bearer(firstIdentity.token),
  });
  if (!initialOverview.response.ok || initialOverview.body?.accountId !== accountId) {
    throw new Error(`identity overview failed with HTTP ${initialOverview.response.status}`);
  }

  const rotated = await request("/v1/identity/recovery-codes/rotate", {
    method: "POST",
    headers: bearer(firstIdentity.token),
    body: "{}",
  });
  if (!rotated.response.ok || rotated.body?.recoveryCodes?.length !== 10) {
    throw new Error(`recovery code rotation failed with HTTP ${rotated.response.status}`);
  }

  const recovered = await request("/v1/identity/recover", {
    method: "POST",
    headers: { "Content-Type": "application/json", Origin: origin },
    body: JSON.stringify({
      accountId,
      recoveryCode: rotated.body.recoveryCodes[0],
      newPassword: recoveredPassword,
    }),
  });
  if (!recovered.response.ok || recovered.body?.accepted !== true) {
    throw new Error(`account recovery failed with HTTP ${recovered.response.status}`);
  }
  first.socket.close();

  // Give the previous Gate15 reconnect lease time to leave the active path.
  await delay(Number(process.env.MIR2_RECOVERY_RELOGIN_DELAY_MS ?? 16_000));
  const second = await connect();
  const { login: secondLogin, identity: secondIdentity } = await login(second, recoveredPassword);
  const recoveredCharacter = secondLogin.payload?.characters?.find((entry) => entry.index === characterIndex);
  if (!recoveredCharacter) throw new Error("recovered login did not load the existing character");
  await startGame(second, characterIndex);

  const currentSessionId = secondIdentity.session?.sessionId;
  if (!currentSessionId) throw new Error("current identity session id is missing");
  const revoked = await request("/v1/identity/sessions/revoke", {
    method: "POST",
    headers: bearer(secondIdentity.token),
    body: JSON.stringify({ sessionId: currentSessionId, reason: "production_acceptance" }),
  });
  if (!revoked.response.ok || revoked.body?.accepted !== true) {
    throw new Error(`session revocation failed with HTTP ${revoked.response.status}`);
  }
  const rejected = await request("/v1/identity/me", { headers: bearer(secondIdentity.token) });
  if (rejected.response.status !== 401) {
    throw new Error(`revoked session remained usable with HTTP ${rejected.response.status}`);
  }
  second.socket.close();

  console.log(JSON.stringify({
    ok: true,
    accountId,
    characterName,
    characterIndex,
    assertions: {
      accountCreated: true,
      characterCreated: true,
      firstStartGame: true,
      postgresIdentitySession: true,
      recoveryCodesIssued: 10,
      passwordRecovered: true,
      recoveredPasswordLogin: true,
      existingCharacterReloaded: true,
      secondStartGame: true,
      currentSessionRevoked: true,
      revokedSessionRejected: true,
    },
  }, null, 2));
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
});
