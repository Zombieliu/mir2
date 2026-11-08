import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { createReadStream } from "node:fs";
import { access, mkdir, writeFile } from "node:fs/promises";
import { createServer } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { DIRECTIONS, GatewayClient } from "../src/gateway-client.mjs";
import { stringifyGatewayJson } from "../src/lossless-json.mjs";

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(appRoot, "../..");
const suffix = process.platform === "win32" ? ".exe" : "";
const args = process.argv.slice(2);
let executable = resolve(repoRoot, `target-ai-player/debug/mir2-gateway${suffix}`);
for (let index = 0; index < args.length; index += 1) {
  if (args[index] !== "--gateway-exe" || !args[index + 1]) throw new Error("Use --gateway-exe <path>.");
  executable = resolve(args[++index]);
}

async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

function isolatedEnvironment(overrides) {
  const keys = new Set(["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "TEMP", "TMP", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "COMSPEC", "LANG", "LC_ALL", "TZ"]);
  return {
    ...Object.fromEntries(Object.entries(process.env).filter(([key]) => keys.has(key.toUpperCase()))),
    ...overrides,
  };
}

async function waitHealthy(url, child) {
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error("Isolated gateway exited before becoming ready.");
    try {
      const result = await fetch(url, { signal: AbortSignal.timeout(1_000) });
      if (result.ok) return;
    } catch { /* wait for this child only */ }
    await sleep(200);
  }
  throw new Error("Isolated gateway health check timed out.");
}

async function waitState(client, predicate) {
  if (predicate(client.snapshot())) return;
  await client.waitFor(() => predicate(client.snapshot()));
}

async function sha256(path) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

async function acceptance() {
  await access(executable);
  const runId = randomUUID();
  const realmId = `ai-player-b1-${runId}`;
  const output = resolve(appRoot, ".data", `acceptance-${runId}`);
  await mkdir(output, { recursive: true });
  const webPort = await freePort();
  const env = isolatedEnvironment({
    MIR2_GATEWAY_TCP_ADDR: "127.0.0.1:0",
    MIR2_GATEWAY_WEB_ADDR: `127.0.0.1:${webPort}`,
    MIR2_ACCOUNT_STORE_BACKEND: "file",
    MIR2_ACCOUNT_STORE_PATH: resolve(output, "accounts.json"),
    MIR2_SAVE_RECOVERY_DIR: resolve(output, "save-recovery"),
    MIR2_SAVE_RECOVERY_MAC_KEY: randomBytes(32).toString("hex"),
    MIR2_IDENTITY_SESSION_SECRET: randomBytes(32).toString("hex"),
    MIR2_IDENTITY_RECOVERY_PEPPER: randomBytes(32).toString("hex"),
    MIR2_REALM_ID: realmId,
    MIR2_CONTENT_PROFILE: "crystal_full",
    MIR2_RUNTIME_ENV: "test",
    MIR2_ZONE_ROUTING_MODE: "single",
    MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS: "0",
    MIR2_AI_LIVE_ENABLED: "0",
    MIR2_GATEWAY_TOKIO_WORKER_THREADS: "2",
  });
  const child = spawn(executable, [], { cwd: repoRoot, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  let gatewayDiagnostics = "";
  const capture = (chunk) => { gatewayDiagnostics = (gatewayDiagnostics + chunk.toString()).slice(-32_000); };
  child.stdout.on("data", capture);
  child.stderr.on("data", capture);
  let spawnFailure = false;
  child.on("error", () => { spawnFailure = true; });
  const clients = [];
  const commands = {};
  const checks = [];
  let phase = "gateway startup";
  const startedAt = new Date().toISOString();
  const connectClient = async () => {
    const client = new GatewayClient({ url: `ws://127.0.0.1:${webPort}/ws`, realmId, timeoutMs: 25_000 });
    client.on("command", ({ type }) => { commands[type] = (commands[type] ?? 0) + 1; });
    clients.push(client);
    await client.connect();
    return client;
  };
  const identities = [0, 1].map((index) => ({
    account: `aib${randomBytes(5).toString("hex")}`,
    password: `Ab${randomBytes(8).toString("hex")}7`,
    name: `AI${index}${randomBytes(3).toString("hex")}`,
  }));
  let report;
  try {
    if (spawnFailure) throw new Error("Unable to start the isolated gateway executable.");
    await waitHealthy(`http://127.0.0.1:${webPort}/health`, child);
    phase = "ordinary account entry";
    const players = [];
    for (const identity of identities) {
      const client = await connectClient();
      await client.createAccount(identity.account, identity.password);
      await client.login(identity.account, identity.password);
      const character = await client.createCharacter(identity.name);
      const state = await client.startGame(character.index);
      assert.equal(state.mapFileName, "0");
      assert.equal(state.ready, true);
      players.push({ client, character });
    }
    checks.push("two ordinary accounts and Warriors entered Bichon");
    console.log(JSON.stringify({ phase: "two-players-ready" }));

    for (let index = 0; index < players.length; index += 1) {
      const owner = players[index].client;
      const observer = players[1 - index].client;
      const name = identities[index].name;
      phase = `observer visibility ${index}`;
      await waitState(observer, (state) => state.entities.some((entity) => entity.kind === "player" && entity.name === name));
      const remote = observer.snapshot().entities.find((entity) => entity.kind === "player" && entity.name === name);
      let moved = false;
      phase = `authoritative walk ${index}`;
      for (const direction of DIRECTIONS) {
        const afterSequence = observer.sequence;
        const movement = await owner.walk(direction);
        if (movement.moved) {
          await observer.waitFor((event) => event.packet === "ObjectWalk" && event.payload?.objectId === remote.objectId &&
            event.payload.x === movement.after.x && event.payload.y === movement.after.y, { after: afterSequence });
          moved = true;
          break;
        }
        await sleep(500);
      }
      assert.ok(moved, "Owner must really move; a correction is not a successful walk.");
      const text = `Bichon AI body test ${index} ${runId.slice(0, 8)}`;
      const expectedText = `${name}: ${text}`;
      phase = `nearby chat ${index}`;
      const afterSequence = observer.sequence;
      await owner.sayNearby(text);
      await observer.waitFor((event) => event.packet === "ObjectChat" && event.payload?.objectId === remote.objectId &&
        event.payload.text === expectedText && event.payload.chatType === "Normal", { after: afterSequence });
    }
    checks.push("both observers received the other player's authoritative walk and nearby chat");
    console.log(JSON.stringify({ phase: "shared-movement-and-chat-passed" }));

    const chatCount = commands.chat;
    await assert.rejects(players[0].client.sayNearby("  @ADDSTORAGE"));
    assert.equal(commands.chat, chatCount);
    checks.push("a chat control command was rejected before network transmission");

    const first = players[0];
    phase = "logout, AOI removal and fresh-session reload";
    const observer = players[1].client;
    const oldRemote = observer.snapshot().entities.find((entity) => entity.name === identities[0].name && entity.kind === "player");
    const saved = first.client.snapshot();
    const afterSequence = observer.sequence;
    await first.client.logout();
    await observer.waitFor((event) => event.packet === "ObjectRemove" && event.payload?.objectId === oldRemote.objectId, { after: afterSequence });
    await first.client.close();
    // Current Gateway route leases can outlive explicit logout (default TTL 30s).
    // Preserve that real timing; this is not evidence of immediate reconnect.
    const reloginSettleMs = 31_000;
    console.log(JSON.stringify({ phase: "waiting-for-existing-route-lease", reloginSettleMs }));
    await sleep(reloginSettleMs);
    const reloaded = await connectClient();
    const roster = await reloaded.login(identities[0].account, identities[0].password);
    assert.ok(roster.some((entry) => entry.index === first.character.index));
    const restored = await reloaded.startGame(first.character.index);
    assert.deepEqual({ map: restored.mapFileName, x: restored.player.x, y: restored.player.y },
      { map: saved.mapFileName, x: saved.player.x, y: saved.player.y });
    checks.push("logout removed AOI presence and a fresh network session reloaded the saved transform");
    await reloaded.logout();
    await observer.logout();
    report = {
      schema: "mir2-ai-player-b1-acceptance/1",
      ok: true,
      stage: "B1-body",
      startedAt,
      finishedAt: new Date().toISOString(),
      realmId,
      gatewayBinarySha256: await sha256(executable),
      normalClientOnly: true,
      modelEnabled: false,
      longTermMemoryEnabled: false,
      reloginSettleMs,
      checks,
      commands,
    };
    await writeFile(resolve(output, "summary.json"), stringifyGatewayJson(report) + "\n");
    console.log(JSON.stringify({ ok: true, checks: checks.length, report: resolve(output, "summary.json") }));
  } catch (error) {
    const failure = { ok: false, stage: "B1-body", startedAt, phase, checks, commands, error: error.message };
    await writeFile(resolve(output, "summary.json"), JSON.stringify(failure, null, 2) + "\n");
    // Diagnostics remain in this private, ignored directory and contain no client outgoing frames.
    await writeFile(resolve(output, "gateway.log"), gatewayDiagnostics);
    console.error(JSON.stringify({ ok: false, phase, error: error.message, report: resolve(output, "summary.json") }));
    process.exitCode = 1;
  } finally {
    await Promise.allSettled(clients.map((client) => client.close()));
    if (child.exitCode === null) {
      const exited = new Promise((resolve) => child.once("exit", resolve));
      child.kill();
      await Promise.race([exited, sleep(5_000)]);
    }
  }
}

acceptance().catch(() => {
  console.error("Local acceptance could not start. Build the gateway first or pass --gateway-exe.");
  process.exitCode = 1;
});
