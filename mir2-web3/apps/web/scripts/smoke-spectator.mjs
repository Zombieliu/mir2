import fs from "node:fs/promises";
import path from "node:path";

const gatewayWs = process.env.MIR2_SPECTATOR_SMOKE_WS ?? "ws://127.0.0.1:7110";
const gatewayHttp = process.env.MIR2_SPECTATOR_SMOKE_HTTP ?? "http://127.0.0.1:7110";
const token = process.env.MIR2_SPECTATOR_SMOKE_TOKEN ?? "local-spectator-director";
const output = process.env.MIR2_SPECTATOR_SMOKE_OUTPUT
  ?? path.resolve("artifacts/spectator/spectator-smoke.json");

function waitFor(ws, predicate, timeoutMs = 15_000) {
  return new Promise((resolve, reject) => {
    const startedAt = Date.now();
    const poll = setInterval(() => {
      const index = ws.__smokeMessages.findIndex(predicate);
      if (index >= 0) {
        const [message] = ws.__smokeMessages.splice(index, 1);
        clearInterval(poll);
        resolve(message);
      } else if (Date.now() - startedAt >= timeoutMs) {
        clearInterval(poll);
        const summary = ws.__smokeMessages.slice(-12).map((message) => ({
          type: message.type,
          packet: message.packet ?? null,
          map: message.payload?.mapFileName ?? null,
        }));
        reject(new Error(`timed out after ${timeoutMs}ms; buffered=${JSON.stringify(summary)}`));
      }
    }, 20);
  });
}

function open(url) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    ws.__smokeMessages = [];
    ws.addEventListener("message", (event) => {
      ws.__smokeMessages.push(JSON.parse(String(event.data)));
      if (ws.__smokeMessages.length > 200) ws.__smokeMessages.shift();
    });
    const timeout = setTimeout(() => reject(new Error(`open timed out: ${url}`)), 10_000);
    ws.addEventListener("open", () => {
      clearTimeout(timeout);
      resolve(ws);
    }, { once: true });
    ws.addEventListener("error", () => {
      clearTimeout(timeout);
      reject(new Error(`WebSocket error: ${url}`));
    }, { once: true });
  });
}

function send(ws, command) {
  ws.send(JSON.stringify(command));
}

async function main() {
  const player = await open(`${gatewayWs}/ws`);
  await waitFor(player, (message) => message.type === "packet" && message.packet === "Connected");
  send(player, { type: "clientVersion" });
  await waitFor(player, (message) => message.type === "packet" && message.packet === "ClientVersion");
  send(player, { type: "login", accountId: "demo", password: "demo" });
  await waitFor(player, (message) => message.type === "packet" && message.packet === "LoginSuccess");
  const playerWorldPromise = waitFor(
    player,
    (message) => message.type === "worldSnapshot" && message.payload?.mapFileName,
    30_000,
  );
  send(player, { type: "startGame", characterIndex: 0 });
  const playerWorld = await playerWorldPromise;

  const spectatorUrl = new URL(`${gatewayWs}/spectator/ws`);
  spectatorUrl.searchParams.set("map", String(playerWorld.payload.mapFileName));
  spectatorUrl.searchParams.set("delayMs", "0");
  spectatorUrl.searchParams.set("mode", "director");
  spectatorUrl.searchParams.set("token", token);
  const spectator = await open(spectatorUrl);
  const status = await waitFor(
    spectator,
    (message) => message.type === "spectatorStatus" && (message.payload?.events?.length ?? 0) > 0,
    20_000,
  );
  const world = await waitFor(spectator, (message) => message.type === "worldSnapshot", 20_000);

  if (!status.payload?.readOnly || !status.payload?.directorAuthorized || status.payload?.delayMs !== 0) {
    throw new Error(`unexpected spectator authorization: ${JSON.stringify(status.payload)}`);
  }
  if (!Array.isArray(status.payload?.events) || status.payload.events.length < 1) {
    throw new Error("spectator event timeline is empty");
  }
  for (const privateField of ["inventoryItems", "storageItems", "questLog", "knownSkills"]) {
    if (!Array.isArray(world.payload?.[privateField]) || world.payload[privateField].length !== 0) {
      throw new Error(`private field leaked: ${privateField}`);
    }
  }

  const invalidPromise = waitFor(
    spectator,
    (message) => message.type === "error" && String(message.message).includes("invalid spectator control"),
  );
  send(spectator, { type: "walk", x: 1, y: 1 });
  await invalidPromise;

  const followTarget = status.payload.targets?.[0]?.name ?? null;
  const followPromise = waitFor(
    spectator,
    (message) => message.type === "spectatorStatus" && message.payload?.target === followTarget,
  );
  send(spectator, { type: "follow", target: followTarget });
  await followPromise;

  const metricsResponse = await fetch(`${gatewayHttp}/spectator/metrics?token=${encodeURIComponent(token)}`);
  if (!metricsResponse.ok) throw new Error(`metrics failed: ${metricsResponse.status}`);
  const metrics = await metricsResponse.json();
  if (metrics.activeViewers < 1 || metrics.persistedFramesTotal < 1) {
    throw new Error(`unexpected metrics: ${JSON.stringify(metrics)}`);
  }

  const recordingsResponse = await fetch(
    `${gatewayHttp}/spectator/recordings?token=${encodeURIComponent(token)}`,
  );
  if (!recordingsResponse.ok) throw new Error(`recordings failed: ${recordingsResponse.status}`);
  const recordings = await recordingsResponse.json();
  const recordingId = recordings.recordings?.[0]?.recordingId;
  if (!recordingId) throw new Error("no persisted recording found");

  const replayResponse = await fetch(
    `${gatewayHttp}/spectator/replay?token=${encodeURIComponent(token)}&replayId=${encodeURIComponent(recordingId)}`,
  );
  if (!replayResponse.ok) throw new Error(`replay failed: ${replayResponse.status}`);
  const replay = await replayResponse.json();
  if (!Array.isArray(replay.frames) || replay.frames.length < 1) {
    throw new Error("replay contains no frames");
  }

  const report = {
    schema: "obelisk.mir2.spectator-smoke.v1",
    generatedAt: new Date().toISOString(),
    map: playerWorld.payload.mapFileName,
    readOnly: true,
    directorAuthorized: true,
    delayMs: status.payload.delayMs,
    invalidGameplayCommandRejected: true,
    privateFieldsRedacted: true,
    timelineEvents: status.payload.events.length,
    activeViewers: metrics.activeViewers,
    persistedFrames: metrics.persistedFramesTotal,
    recordingId,
    replayFrames: replay.frames.length,
  };
  await fs.mkdir(path.dirname(output), { recursive: true });
  await fs.writeFile(output, `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report, null, 2));
  spectator.close();
  player.close();
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
