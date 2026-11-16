import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { DIRECTIONS, GatewayClient } from "../src/gateway-client.mjs";
import { stringifyGatewayJson } from "../src/lossless-json.mjs";
import { gatewayExecutableArgument, resolveGatewayExecutable, sha256, startLocalGateway } from "./local-gateway.mjs";

const executable = await resolveGatewayExecutable(gatewayExecutableArgument(process.argv.slice(2)));

async function waitState(client, predicate) {
  if (predicate(client.snapshot())) return;
  await client.waitFor(() => predicate(client.snapshot()));
}

async function acceptance() {
  const gateway = await startLocalGateway({ executable, prefix: "acceptance" });
  const { realmId, output, runId } = gateway;
  const clients = [];
  const commands = {};
  const checks = [];
  let phase = "gateway startup";
  const startedAt = new Date().toISOString();
  const connectClient = async () => {
    const client = new GatewayClient({ url: gateway.url, realmId, timeoutMs: 25_000 });
    client.on("command", ({ type }) => { commands[type] = (commands[type] ?? 0) + 1; });
    clients.push(client);
    const realm = await client.connect();
    gateway.observedContentProfile = { profileId: realm.profileId, profileVersion: realm.profileVersion };
    assert.equal(realm.profileId, gateway.contentProfile);
    return client;
  };
  const identities = [0, 1].map((index) => ({
    account: `aib${randomBytes(5).toString("hex")}`,
    password: `Ab${randomBytes(8).toString("hex")}7`,
    name: `AI${index}${randomBytes(3).toString("hex")}`,
  }));
  let report;
  try {
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
      contentProfile: gateway.contentProfile, observedContentProfile: gateway.observedContentProfile, gameplayBaseline: gateway.gameplayBaseline,
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
    const failure = { ok: false, stage: "B1-body", startedAt, phase, checks, commands, error: error.message,
      contentProfile: gateway.contentProfile, observedContentProfile: gateway.observedContentProfile ?? null };
    await writeFile(resolve(output, "summary.json"), JSON.stringify(failure, null, 2) + "\n");
    // Diagnostics remain in this private, ignored directory and contain no client outgoing frames.
    await gateway.saveDiagnostics();
    console.error(JSON.stringify({ ok: false, phase, error: error.message, report: resolve(output, "summary.json") }));
    process.exitCode = 1;
  } finally {
    await Promise.allSettled(clients.map((client) => client.close()));
    await gateway.close();
  }
}

acceptance().catch(() => {
  console.error("Local acceptance could not start. Build the gateway first or pass --gateway-exe.");
  process.exitCode = 1;
});
