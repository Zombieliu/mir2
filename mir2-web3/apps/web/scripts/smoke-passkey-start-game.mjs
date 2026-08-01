import { createHmac } from "node:crypto";

const gatewayWsUrl = process.env.MIR2_PASSKEY_SMOKE_WS_URL ?? "ws://127.0.0.1:7110/ws";
const timeoutMs = Number(process.env.MIR2_PASSKEY_SMOKE_TIMEOUT_MS ?? 30_000);
const secret = resolvePasskeySecret();
const suffix = `${Date.now().toString(36)}${process.pid.toString(36)}`;
const accountId = `sui:0xsmoke${suffix}`;
const characterName = `Pk${suffix}`.slice(0, 10);
const token = issueGatewayToken(accountId, secret);
const frames = [];

const ws = new WebSocket(gatewayWsUrl);
try {
  await waitForOpen(ws, timeoutMs);
  ws.addEventListener("message", (event) => {
    try {
      frames.push(JSON.parse(String(event.data)));
    } catch {
      frames.push({ type: "invalidJson", payload: String(event.data) });
    }
  });

  send({ type: "clientVersion" });
  send({ type: "passkeyLogin", accountId, token });
  const login = await waitForFrame(
    (frame) => frame.packet === "LoginSuccess",
    "Passkey LoginSuccess",
  );
  if ((login.payload?.characters ?? []).length !== 0) {
    throw new Error("fresh Passkey smoke account unexpectedly contains characters");
  }

  send({
    type: "newCharacter",
    name: characterName,
    gender: "Male",
    class: "Warrior",
  });
  const created = await waitForFrame(
    (frame) => frame.packet === "NewCharacterSuccess",
    "NewCharacterSuccess",
  );
  const characterIndex = created.payload?.character?.index;
  if (!Number.isInteger(characterIndex)) {
    throw new Error(`NewCharacterSuccess omitted character index: ${JSON.stringify(created)}`);
  }

  const startFrameOffset = frames.length;
  send({ type: "startGame", characterIndex });
  const started = await waitForFrame(
    (frame, index) =>
      index >= startFrameOffset && frame.packet === "StartGame" && frame.payload?.result === 4,
    "successful StartGame",
  );
  const userInformation = await waitForFrame(
    (frame, index) => index >= startFrameOffset && frame.packet === "UserInformation",
    "UserInformation",
  );
  const snapshot = await waitForFrame(
    (frame, index) =>
      index >= startFrameOffset &&
      frame.type === "worldSnapshot" &&
      Boolean(frame.payload?.mapFileName) &&
      Number.isInteger(frame.payload?.playerObjectId),
    "authenticated worldSnapshot",
  );

  console.log(
    JSON.stringify(
      {
        ok: true,
        gatewayWsUrl,
        accountId,
        characterName,
        characterIndex,
        startResult: started.payload.result,
        playerObjectId:
          userInformation.payload?.objectId ?? snapshot.payload.playerObjectId,
        mapFileName: snapshot.payload.mapFileName,
      },
      null,
      2,
    ),
  );
} finally {
  ws.close();
}

function send(command) {
  ws.send(JSON.stringify(command));
}

function waitForOpen(socket, waitMs) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error(`Gateway WebSocket did not open within ${waitMs} ms`)),
      waitMs,
    );
    socket.addEventListener(
      "open",
      () => {
        clearTimeout(timeout);
        resolve();
      },
      { once: true },
    );
    socket.addEventListener(
      "error",
      () => {
        clearTimeout(timeout);
        reject(new Error(`Gateway WebSocket failed to open: ${gatewayWsUrl}`));
      },
      { once: true },
    );
  });
}

async function waitForFrame(predicate, label) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const match = frames.find((frame, index) => predicate(frame, index));
    if (match) return match;
    const gatewayError = frames.find((frame) => frame.type === "error");
    if (gatewayError) {
      throw new Error(`Gateway rejected ${label}: ${gatewayError.message ?? JSON.stringify(gatewayError)}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  throw new Error(
    `Timed out waiting for ${label}; recent frames=${JSON.stringify(frames.slice(-8))}`,
  );
}

function issueGatewayToken(targetAccountId, signingSecret) {
  const payload = Buffer.from(
    JSON.stringify({
      auth: "sui-passkey-v1",
      accountId: targetAccountId,
      expMs: Date.now() + 60_000,
    }),
  ).toString("base64url");
  const signature = createHmac("sha256", signingSecret).update(payload).digest("base64url");
  return `${payload}.${signature}`;
}

function resolvePasskeySecret() {
  if (process.env.MIR2_PASSKEY_AUTH_SECRET) return process.env.MIR2_PASSKEY_AUTH_SECRET;
  if (/^(1|true|yes)$/i.test(process.env.MIR2_ALLOW_DEV_PASSKEY_SECRET ?? "")) {
    return "mir2-web3-local-passkey-auth-secret";
  }
  throw new Error(
    "Set MIR2_PASSKEY_AUTH_SECRET, or explicitly set MIR2_ALLOW_DEV_PASSKEY_SECRET=1 for a local Gateway smoke test.",
  );
}
