import { spawn } from "node:child_process";
import fsSync from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const DEFAULT_BASE_URL = "http://127.0.0.1:13010";
const DEFAULT_GATEWAY_WS_URL = "ws://127.0.0.1:7210/ws";
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "two-client-zone");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const gatewayWsUrl = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? DEFAULT_GATEWAY_WS_URL;
const baseUrl = buildBaseUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL, gatewayWsUrl);
const outputDir = path.resolve(args.output ?? process.env.MIR2_TWO_CLIENT_ZONE_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const runId = args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
const prefix = args.prefix ?? `two-client-zone-${runId}`;
const password = args.password ?? process.env.MIR2_TWO_CLIENT_ZONE_PASSWORD ?? "zone-pass";
const qaControlToken =
  args.qaControlToken ?? process.env.MIR2_QA_CONTROL_TOKEN ?? null;
const map = args.map ?? process.env.MIR2_TWO_CLIENT_ZONE_MAP ?? "0";
const accountA = args.accountA ?? process.env.MIR2_TWO_CLIENT_ZONE_ACCOUNT_A ?? `zonea${runId}`;
const accountB = args.accountB ?? process.env.MIR2_TWO_CLIENT_ZONE_ACCOUNT_B ?? `zoneb${runId}`;
const characterA = (args.characterA ?? process.env.MIR2_TWO_CLIENT_ZONE_CHARACTER_A ?? `ZA${runId.slice(-6)}`).slice(0, 10);
const characterB = (args.characterB ?? process.env.MIR2_TWO_CLIENT_ZONE_CHARACTER_B ?? `ZB${runId.slice(-6)}`).slice(0, 10);
const startAx = numberArg(args.ax ?? process.env.MIR2_TWO_CLIENT_ZONE_AX, 330);
const startAy = numberArg(args.ay ?? process.env.MIR2_TWO_CLIENT_ZONE_AY, 270);
const startBx = numberArg(args.bx ?? process.env.MIR2_TWO_CLIENT_ZONE_BX, 332);
const startBy = numberArg(args.by ?? process.env.MIR2_TWO_CLIENT_ZONE_BY, 270);
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 500));
const cdpCommandTimeoutMs = numberArg(
  args.cdpCommandTimeoutMs ?? process.env.MIR2_CDP_COMMAND_TIMEOUT_MS,
  15_000,
);
const observerPulseAfterMove = booleanArg(
  args.observerPulseAfterMove ?? process.env.MIR2_TWO_CLIENT_ZONE_PULSE_AFTER_MOVE,
  false,
);
const maxObserverMovementLatencyMs = numberArg(
  args.maxObserverMovementLatencyMs ?? process.env.MIR2_TWO_CLIENT_ZONE_MAX_OBSERVER_MOVE_MS,
  250,
);
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const skipTransfers = booleanArg(
  args.skipTransfers ?? process.env.MIR2_TWO_CLIENT_ZONE_SKIP_TRANSFERS,
  false,
);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

class CdpClient {
  constructor(wsUrl, label) {
    this.wsUrl = wsUrl;
    this.label = label;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleErrors = [];
    this.network404s = [];
    this.packetFrames = [];
  }

  async connect() {
    this.ws = new WebSocket(this.wsUrl);
    this.ws.addEventListener("message", (event) => this.handleMessage(event.data));
    await new Promise((resolve, reject) => {
      this.ws.addEventListener("open", resolve, { once: true });
      this.ws.addEventListener("error", reject, { once: true });
    });
  }

  handleMessage(raw) {
    const message = JSON.parse(raw);
    if (message.id && this.pending.has(message.id)) {
      const { resolve, reject, timeout } = this.pending.get(message.id);
      this.pending.delete(message.id);
      clearTimeout(timeout);
      if (message.error) {
        reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      } else {
        resolve(message.result ?? {});
      }
      return;
    }

    if (message.method === "Runtime.consoleAPICalled" && message.params?.type === "error") {
      this.consoleErrors.push({
        source: "console",
        text: (message.params.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
      });
    }

    if (message.method === "Runtime.exceptionThrown") {
      const details = message.params?.exceptionDetails;
      this.consoleErrors.push({
        source: "exception",
        text: details?.exception?.description ?? details?.text ?? "runtime exception",
      });
    }

    if (message.method === "Log.entryAdded") {
      const entry = message.params?.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrors.push({ source: entry.source ?? "log", text: entry.text ?? "" });
      }
    }

    if (message.method === "Network.responseReceived") {
      const response = message.params?.response;
      if (response?.status === 404 && !String(response.url ?? "").includes("favicon")) {
        this.network404s.push(response.url);
      }
    }

    if (
      message.method === "Network.webSocketFrameReceived" ||
      message.method === "Network.webSocketFrameSent"
    ) {
      const payloadData = message.params?.response?.payloadData ?? "";
      if (typeof payloadData === "string" && isZonePacketPayload(payloadData)) {
        this.packetFrames.push({
          direction: message.method.endsWith("Sent") ? "sent" : "received",
          payloadData: payloadData.slice(0, 800),
          at: Date.now(),
        });
        this.packetFrames = this.packetFrames.slice(-60);
      }
    }
  }

  send(method, params = {}, timeoutMs = 15_000) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        if (!this.pending.delete(id)) return;
        reject(new Error(`${this.label} CDP ${method} timed out after ${cdpCommandTimeoutMs}ms`));
      }, cdpCommandTimeoutMs);
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timeout);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timeout);
          reject(error);
        },
      });
      try {
        this.ws.send(JSON.stringify({ id, method, params }));
      } catch (error) {
        const pending = this.pending.get(id);
        this.pending.delete(id);
        pending?.reject(error);
      }
    });
  }

  async evaluate(expression) {
    let lastError;
    for (let attempt = 1; attempt <= 3; attempt += 1) {
      try {
        const result = await this.send("Runtime.evaluate", {
          expression,
          awaitPromise: true,
          returnByValue: true,
          userGesture: true,
        });
        if (result.exceptionDetails) {
          throw new Error(`${this.label} evaluate failed: ${result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails)}`);
        }
        return result.result?.value;
      } catch (error) {
        lastError = error;
        if (attempt < 3) {
          await delay(250 * attempt);
        }
      }
    }
    throw lastError;
  }

  close() {
    for (const { reject, timeout } of this.pending.values()) {
      clearTimeout(timeout);
      reject(new Error(`${this.label} CDP connection closed`));
    }
    this.pending.clear();
    this.ws?.close();
  }
}

async function main() {
  await fs.mkdir(outputDir, { recursive: true });
  const chrome = await launchChrome();
  const clients = [];

  try {
    clients.push(await makeClient("A"), await makeClient("B"));
    const accounts = [
      { account: accountA, character: characterA, x: startAx, y: startAy },
      { account: accountB, character: characterB, x: startBx, y: startBy },
    ];

    // Start sequentially so two fresh characters never contend for the same
    // bootstrap cell. Park A away from B's default spawn until B has joined.
    await loginAndStart(clients[0], accounts[0]);
    if (!skipTransfers) {
      await transferTo(clients[0], Math.max(0, startAx - 6), startAy);
    }
    await pulseBoth([clients[0]], 2);
    await loginAndStart(clients[1], accounts[1]);
    if (!skipTransfers) {
      await transferTo(clients[1], accounts[1].x, accounts[1].y);
      await transferTo(clients[0], accounts[0].x, accounts[0].y);
    }
    await pulseBoth(clients, 4);

    let social = null;
    if (socialAcceptance) {
      social = await runSocialAcceptance(clients);
    } else {
      await waitUntilClient(
        clients[0],
        `(() => (window.__mir2Stage5?.state?.entities ?? []).some((entity) => entity?.name === ${JSON.stringify(characterB)} && entity?.kind === "player"))()`,
        "A sees B",
        15_000,
      );
      await waitUntilClient(
        clients[1],
        `(() => (window.__mir2Stage5?.state?.entities ?? []).some((entity) => entity?.name === ${JSON.stringify(characterA)} && entity?.kind === "player"))()`,
        "B sees A",
        15_000,
      );
    }

    const movementSentAt = Date.now();
    await sendClientCommand(clients[0], { type: "walk", direction: "Right" }, "A walk right");
    if (observerPulseAfterMove) {
      await pulseBoth(clients, 5);
    }
    await waitUntilClient(
      clients[1],
      `(() => (window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === "ObjectWalk" || event?.packet === "ObjectRun"))()`,
      "B receives A movement broadcast",
      15_000,
    );
    const observerMovementFrame = clients[1].packetFrames.find(
      (frame) =>
        frame.direction === "received" &&
        frame.at >= movementSentAt &&
        /ObjectWalk|ObjectRun/.test(frame.payloadData),
    );
    const observerMovementLatencyMs = observerMovementFrame
      ? observerMovementFrame.at - movementSentAt
      : null;

    const chatMessage = `zone smoke ${runId}`;
    await sendClientCommand(clients[1], { type: "chat", message: chatMessage }, "B chat");
    await pulseBoth(clients, 3);
    await waitUntilClient(
      clients[0],
      `(() => (window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === "ObjectChat" && JSON.stringify(event?.payload ?? {}).includes(${JSON.stringify(chatMessage)})))()`,
      "A receives B chat broadcast",
      15_000,
    );

    const summaries = await Promise.all(clients.map((client) => readSummary(client)));
    const screenshots = [];
    for (let index = 0; index < clients.length; index += 1) {
      screenshots.push(await captureScreenshot(clients[index], `${prefix}-${index === 0 ? "a" : "b"}.png`));
    }

    const report = {
      ok: false,
      runId,
      baseUrl,
      gatewayWsUrl,
      map,
      observerPulseAfterMove,
      maxObserverMovementLatencyMs,
      observerMovementLatencyMs,
      accounts,
      summaries,
      screenshots,
      consoleErrors: clients.flatMap((client) =>
        client.consoleErrors.map((entry) => ({ client: client.label, ...entry })),
      ),
      nonFaviconNetwork404s: clients.flatMap((client) =>
        client.network404s.map((url) => ({ client: client.label, url })),
      ),
      packetFrames: clients.map((client) => ({
        client: client.label,
        frames: client.packetFrames.slice(-16),
      })),
      observations: {
        aSeesB: summaries[0].entities.some((entity) => entity.name === characterB && entity.kind === "player"),
        bSeesA: summaries[1].entities.some((entity) => entity.name === characterA && entity.kind === "player"),
      },
    };
    report.assertions = {
      bothGame: summaries.every((summary) => summary.screen === "game"),
      ...(!socialAcceptance
        ? {
            aSeesB: report.observations.aSeesB,
            bSeesA: report.observations.bSeesA,
          }
        : {}),
      bSawMovementBroadcast: clients[1].packetFrames.some((frame) => /ObjectWalk|ObjectRun/.test(frame.payloadData)),
      bMovementWithoutPulseWithinBudget:
        observerPulseAfterMove ||
        (Number.isFinite(observerMovementLatencyMs) &&
          observerMovementLatencyMs <= maxObserverMovementLatencyMs),
      bRemotePresentationEnabled:
        summaries[1].bevyMovementShadow?.presentation?.enabled === true,
      bRemotePresentationObservedPacket:
        Number(summaries[1].bevyMovementShadow?.presentation?.remoteMotionEventCount ?? 0) > 0,
      bRemotePresentationDrovePackedOffset:
        Number(summaries[1].bevyMovementShadow?.presentation?.offsetMatchCount ?? 0) > 0,
      bRemotePresentationNoDecodeOrQueueDrops:
        summaries[1].bevyMovementShadow?.presentation?.decodeErrorCount === 0 &&
        summaries[1].bevyMovementShadow?.presentation?.pendingEventDropCount === 0,
      aSawChatBroadcast: clients[0].packetFrames.some((frame) => frame.payloadData.includes("ObjectChat") && frame.payloadData.includes(chatMessage)),
      socialAcceptancePassed: !socialAcceptance || social?.ok === true,
      noConsoleErrors: report.consoleErrors.length === 0,
      noNonFavicon404s: report.nonFaviconNetwork404s.length === 0,
    };
    report.ok = Object.values(report.assertions).every(Boolean);

    const reportPath = path.join(outputDir, `${prefix}.json`);
    await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(
      JSON.stringify(
        {
          ok: report.ok,
          reportPath,
          assertions: report.assertions,
          screenshots,
        },
        null,
        2,
      ),
    );
    if (!report.ok) {
      process.exitCode = 1;
    }
  } finally {
    for (const client of clients) {
      client.close();
    }
    await stopChrome(chrome);
  }
}

async function makeClient(label) {
  const target = await createPageTarget();
  const client = new CdpClient(target.webSocketDebuggerUrl, label);
  await client.connect();
  await client.send("Page.enable");
  await client.send("Runtime.enable");
  await client.send("Log.enable");
  await client.send("Network.enable");
  await setViewport(client);
  await client.send("Page.navigate", { url: baseUrl });
  await waitUntilClient(
    client,
    `document.readyState === "complete" || document.readyState === "interactive"`,
    `${label} page load`,
    20_000,
  );
  await waitUntilClient(
    client,
    `["login", "select", "game"].includes(window.__mir2Stage5?.state?.screen)`,
    `${label} stage ready`,
    25_000,
  );
  return client;
}

async function loginAndStart(client, accountInfo) {
  const screen = await client.evaluate(`window.__mir2Stage5?.state?.screen ?? null`);
  if (screen === "login") {
    await waitUntilClient(
      client,
      `Boolean(document.querySelector(".login-input.account") && document.querySelector(".login-input.password"))`,
      `${client.label} login form`,
      15_000,
    );
    await fillInput(client, ".login-input.account", accountInfo.account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.account button");
    await waitUntilClient(
      client,
      `window.__mir2Stage5?.state?.wsState === "open"`,
      `${client.label} account socket open`,
      15_000,
    );
    await delay(1_000);
    // Account creation can clear the controlled inputs. Refill before login so
    // the second click never submits an empty password.
    await fillInput(client, ".login-input.account", accountInfo.account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.ok button");
    await waitUntilClient(client, `window.__mir2Stage5?.state?.screen === "select"`, `${client.label} select`, 20_000);
  }

  await sendClientCommand(
    client,
    { type: "newCharacter", name: accountInfo.character, gender: "male", class: "warrior" },
    `${client.label} new character`,
  );
  await waitUntilClient(
    client,
    `(window.__mir2Stage5?.state?.characters ?? []).some((character) => character?.name === ${JSON.stringify(accountInfo.character)})`,
    `${client.label} character created`,
    15_000,
  );
  const startGame = await client.evaluate(`
    (() => {
      const character = (window.__mir2Stage5?.state?.characters ?? []).find((entry) => entry?.name === ${JSON.stringify(accountInfo.character)});
      const previousSnapshotAt = window.__mir2PacketRuntime?.lastSnapshotAt ?? null;
      const sent = window.__mir2Stage5?.send?.({ type: "startGame", characterIndex: character?.index ?? 0 }) === true;
      return { sent, previousSnapshotAt };
    })()
  `);
  if (!startGame?.sent) {
    throw new Error(`${client.label} start game command was not accepted by the Web client.`);
  }
  await waitUntilClient(
    client,
    `(() => {
      const state = window.__mir2Stage5?.state;
      return state?.screen === "game" &&
        (Boolean(state?.player) || (state?.entities ?? []).some((entity) => entity?.kind === "selfPlayer"));
    })()`,
    `${client.label} game`,
    25_000,
  );
  await waitUntilClient(
    client,
    `(() => {
      const snapshotAt = window.__mir2PacketRuntime?.lastSnapshotAt;
      const previousSnapshotAt = ${JSON.stringify(startGame.previousSnapshotAt)};
      return Number.isFinite(snapshotAt) &&
        (previousSnapshotAt == null || snapshotAt > previousSnapshotAt);
    })()`,
    `${client.label} start game world snapshot`,
    20_000,
  );
}

async function transferTo(client, x, y) {
  const action = { type: "transferMap", key: `crystal:${map}:${x}:${y}` };
  const command = qaControlToken ? { type: "qaControl", token: qaControlToken, action } : action;
  const transfer = await client.evaluate(`
    (() => {
      const previousSnapshotAt = window.__mir2PacketRuntime?.lastSnapshotAt ?? null;
      const sent = window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true;
      return { sent, previousSnapshotAt };
    })()
  `);
  if (!transfer?.sent) {
    throw new Error(`${client.label} transfer command was not accepted by the Web client.`);
  }
  await waitUntilClient(
    client,
    `(() => {
      const state = window.__mir2Stage5?.state;
      const packetRuntime = window.__mir2PacketRuntime;
      const snapshotAt = packetRuntime?.lastSnapshotAt;
      const previousSnapshotAt = ${JSON.stringify(transfer.previousSnapshotAt)};
      const activeMap = packetRuntime?.mapFileName ?? state?.mapFileName;
      return String(activeMap) === ${JSON.stringify(map)} &&
        state?.player?.x === ${x} &&
        state?.player?.y === ${y} &&
        Number.isFinite(snapshotAt) &&
        (previousSnapshotAt == null || snapshotAt > previousSnapshotAt);
    })()`,
    `${client.label} transfer ${x},${y} world snapshot`,
    20_000,
  );
}

async function pulseBoth(clients, count) {
  for (let index = 0; index < count; index += 1) {
    const action = { type: "tick" };
    await Promise.all(
      clients.map((client) =>
        sendClientCommand(
          client,
          qaControlToken ? { type: "qaControl", token: qaControlToken, action } : action,
          `${client.label} tick`,
        ),
      ),
    );
    await delay(450);
  }
}

async function runSocialAcceptance(clients) {
  const [leader, recruit] = clients;
  const guildName = `P176${runId.slice(-6)}`.slice(0, 20);
  const rivalGuildName = `R176${runId.slice(-6)}`.slice(0, 20);
  const campaignId = `sabuk-live-${runId}`;
  const guildMessage = `guild acceptance ${runId}`;

  await sendClientCommand(leader, { type: "switchGroup", allowGroup: true }, "leader enables group");
  await sendClientCommand(recruit, { type: "switchGroup", allowGroup: true }, "recruit enables group");
  await sendClientCommand(leader, { type: "addMember", name: characterB }, "leader adds group member");
  await sendClientCommand(recruit, { type: "addMember", name: characterA }, "recruit confirms two-sided group roster");
  await waitForGatewayPacket(leader, "AddMember", "leader group roster packet");
  await waitForGatewayPacket(recruit, "AddMember", "recruit group roster packet");
  await waitUntilClient(
    leader,
    `(window.__mir2Stage5?.state?.stage5Systems?.group?.members ?? []).some((member) => (typeof member === "string" ? member : member?.name) === ${JSON.stringify(characterB)})`,
    "leader sees accepted group member",
    15_000,
  );
  await waitUntilClient(
    recruit,
    `(window.__mir2Stage5?.state?.stage5Systems?.group?.members ?? []).some((member) => (typeof member === "string" ? member : member?.name) === ${JSON.stringify(characterA)})`,
    "recruit sees accepted group leader",
    15_000,
  );
  await pulseBoth(clients, 3);

  await sendClientCommand(
    leader,
    { type: "stage5Command", action: "guild.create", args: [guildName] },
    "leader creates guild",
  );
  await pulseBoth(clients, 2);
  await sendClientCommand(
    leader,
    { type: "editGuildMember", changeType: 0, rankIndex: 0, name: characterB, rankName: "" },
    "leader guild invite",
  );
  await pulseBoth(clients, 2);
  await waitForGatewayPacket(recruit, "GuildInvite", "recruit guild invite");
  await sendClientCommand(recruit, { type: "guildInvite", acceptInvite: true }, "recruit accepts guild");
  await pulseBoth(clients, 3);
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.stage5Systems?.guild?.name === ${JSON.stringify(guildName)}`,
    "recruit guild membership",
    15_000,
  );

  await sendClientCommand(recruit, { type: "chat", message: `!~${guildMessage}` }, "recruit guild chat");
  await pulseBoth(clients, 2);
  await waitUntilClient(
    leader,
    `(window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === "Chat" && JSON.stringify(event?.payload ?? {}).includes(${JSON.stringify(guildMessage)}))`,
    "leader receives guild chat",
    15_000,
  );
  const guildChatDelivered = true;

  // Fresh characters spawn on the same tile. Move the leader one normal step
  // so the Crystal trade request has an adjacent remote partner.
  await sendClientCommand(leader, { type: "walk", direction: "Right" }, "leader adjacent trade step");
  await pulseBoth(clients, 2);
  await sendClientCommand(leader, { type: "tradeRequest" }, "leader trade request");
  await pulseBoth(clients, 2);
  await waitForGatewayPacket(recruit, "TradeRequest", "recruit trade request");
  const tradeRequestDelivered = true;
  await sendClientCommand(recruit, { type: "tradeReply", acceptInvite: true }, "recruit accepts trade");
  await pulseBoth(clients, 2);
  await waitForGatewayPacket(leader, "TradeAccept", "leader trade acceptance");
  const tradeAccepted = true;
  await sendClientCommand(leader, { type: "tradeCancel" }, "leader cancels empty acceptance trade");
  await pulseBoth(clients, 2);
  await waitUntilClient(
    leader,
    `window.__mir2Stage5?.state?.stage5Systems?.trade == null`,
    "leader trade cancel projection",
    15_000,
  );
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.stage5Systems?.trade == null`,
    "recruit trade cancel projection",
    15_000,
  );

  const acceptedGuildSummaries = await Promise.all(clients.map((client) => readSummary(client)));

  // Party members are intentionally protected from ordinary PK. Disband the
  // live group before splitting into rival guilds so this phase exercises
  // hostile player combat rather than repeatedly hitting a friendly target.
  await sendClientCommand(leader, { type: "switchGroup", allowGroup: false }, "leader leaves group before PK");
  await sendClientCommand(recruit, { type: "switchGroup", allowGroup: false }, "recruit leaves group before PK");
  await pulseBoth(clients, 2);
  await waitUntilClient(
    leader,
    `(window.__mir2Stage5?.state?.stage5Systems?.group?.members ?? []).length === 0`,
    "leader group cleared before PK",
    15_000,
  );
  await waitUntilClient(
    recruit,
    `(window.__mir2Stage5?.state?.stage5Systems?.group?.members ?? []).length === 0`,
    "recruit group cleared before PK",
    15_000,
  );

  // Split the accepted member into a second authoritative guild so the same
  // two live browser sessions can exercise both ordinary PK and a two-guild
  // Sabuk campaign instead of merely replaying a single-session fixture.
  await sendClientCommand(
    leader,
    { type: "editGuildMember", changeType: 1, rankIndex: 0, name: characterB, rankName: "" },
    "leader removes recruit for rival guild",
  );
  await pulseBoth(clients, 2);
  await waitUntilClient(
    leader,
    `(window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === "GuildMemberChange" && Number(event?.payload?.status) === 1 && event?.payload?.name === ${JSON.stringify(characterB)})`,
    "leader authoritative guild removal packet",
    15_000,
  );
  await sendClientCommand(recruit, { type: "requestGuildInfo", infoType: 1 }, "recruit refreshes guild after removal");
  await pulseBoth(clients, 2);
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.stage5Systems?.guild?.name === ""`,
    "recruit authoritative guild removal",
    15_000,
  );
  await sendClientCommand(
    recruit,
    { type: "stage5Command", action: "guild.create", args: [rivalGuildName] },
    "recruit creates rival guild",
  );
  await pulseBoth(clients, 2);
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.stage5Systems?.guild?.name === ${JSON.stringify(rivalGuildName)}`,
    "recruit rival guild creation",
    15_000,
  );

  const recruitObjectId = await leader.evaluate(`
    (window.__mir2Stage5?.state?.entities ?? [])
      .find((entity) => entity?.kind === "player" && entity?.name === ${JSON.stringify(characterB)})
      ?.objectId ?? null
  `);
  if (recruitObjectId === null || recruitObjectId === undefined) {
    throw new Error("Leader could not resolve the live recruit object id for PK acceptance.");
  }
  let hasEquippedWeapon = await leader.evaluate(`
    (window.__mir2Stage5?.state?.equipmentItems ?? [])
      .some((item) => item?.slot === "weapon" || item?.equipSlot === "weapon")
  `);
  if (!hasEquippedWeapon) {
    const starterWeapon = await leader.evaluate(`
      (window.__mir2Stage5?.state?.inventoryItems ?? [])
        .find((item) =>
          item?.equipSlot === "weapon" || /wooden.?sword/i.test(String(item?.name ?? item?.key ?? ""))
        ) ?? null
    `);
    if (starterWeapon?.uniqueId === undefined || starterWeapon?.uniqueId === null) {
      throw new Error("Leader has neither an equipped weapon nor a Wooden Sword in inventory for PK acceptance.");
    }
    await sendClientCommand(
      leader,
      { type: "equipItem", uniqueId: Number(starterWeapon.uniqueId), grid: "inventory", to: 0 },
      "leader equips starter weapon for PK",
    );
    await pulseBoth(clients, 2);
    await waitUntilClient(
      leader,
      `(window.__mir2Stage5?.state?.equipmentItems ?? []).some((item) => item?.slot === "weapon" || item?.equipSlot === "weapon")`,
      "leader authoritative starter weapon equip",
      15_000,
    );
    hasEquippedWeapon = true;
  }
  // Crystal mode 5 is the unrestricted all-target PK mode (mode 4 is the
  // red/brown-name filter and correctly refuses a fresh lawful character).
  await sendClientCommand(leader, { type: "changeAMode", mode: 5 }, "leader enables all-target attack mode");
  await pulseBoth(clients, 1);
  await waitUntilClient(
    leader,
    `Number(window.__mir2Stage5?.state?.stage5Systems?.attackMode) === 5`,
    "leader all-target attack mode",
    15_000,
  );
  let recruitDied = false;
  // Level-1 accuracy is intentionally low against another player; allow a
  // deterministic upper bound large enough to finish the real 18 HP target.
  for (let strike = 0; strike < 40 && !recruitDied; strike += 1) {
    await sendClientCommand(
      leader,
      { type: "attack", objectId: Number(recruitObjectId) },
      `leader PK strike ${strike + 1}`,
    );
    await pulseBoth(clients, 1);
    // Vary the sampling phase of the deterministic Crystal accuracy roll;
    // a fixed 900 ms cadence can repeatedly land on the same miss bucket.
    await delay(137 + (strike % 5) * 47);
    recruitDied = await recruit.evaluate(`
      window.__mir2Stage5?.state?.player?.dead === true ||
        Number(window.__mir2Stage5?.state?.playerHp ?? 1) <= 0
    `);
  }
  if (!recruitDied) {
    const combatDebug = await Promise.all(
      clients.map((client) =>
        client.evaluate(`
          (() => {
            const state = window.__mir2Stage5?.state ?? {};
            return {
              label: ${JSON.stringify("browser")},
              player: state.player ?? null,
              inSafeZone: state.inSafeZone ?? null,
              attackMode: state.stage5Systems?.attackMode ?? null,
              group: state.stage5Systems?.group ?? null,
              guild: state.stage5Systems?.guild ?? null,
              trade: state.stage5Systems?.trade ?? null,
              equipmentItems: state.equipmentItems ?? [],
              remotePlayers: (state.entities ?? []).filter((entity) => entity?.kind === "player"),
              combatEvents: (window.__mir2GatewayEventHistory ?? [])
                .filter((event) => ["ObjectAttack", "ObjectStruck", "ObjectHealth", "ObjectDied"].includes(event?.packet))
                .slice(0, 20),
            };
          })()
        `),
      ),
    );
    throw new Error(`PK attacks did not kill the recruit: ${JSON.stringify(combatDebug)}`);
  }
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.player?.dead === true || Number(window.__mir2Stage5?.state?.playerHp ?? 1) <= 0`,
    "recruit PK death",
    15_000,
  );
  await waitUntilClient(
    leader,
    `Number(window.__mir2Stage5?.state?.playerPkPoints ?? 0) > 0`,
    "leader PK point award",
    15_000,
  );
  const pkEvidence = {
    configuredDamageMultiplier,
    attackerPkPoints: await leader.evaluate(`Number(window.__mir2Stage5?.state?.playerPkPoints ?? 0)`),
    victimDead: await recruit.evaluate(`Boolean(window.__mir2Stage5?.state?.player?.dead)`),
    victimSawDeathPacket: await recruit.evaluate(`
      (window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === "ObjectDied")
    `),
  };
  await sendClientCommand(recruit, { type: "townRevive" }, "recruit town revive after PK");
  await pulseBoth(clients, 3);
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.player?.dead === false && Number(window.__mir2Stage5?.state?.playerHp ?? 0) > 0`,
    "recruit revived after PK",
    15_000,
  );

  const conquestBaseTick = await leader.evaluate(
    `Number(window.__mir2Stage5?.state?.stage5Systems?.conquest?.currentTick ?? 0)`,
  );
  const registrationClosesTick = conquestBaseTick + 50;
  const startsAtTick = conquestBaseTick + 100;
  const endsAtTick = conquestBaseTick + 200;
  await sendClientCommand(
    leader,
    {
      type: "stage5Command",
      action: "conquest.schedule",
      args: [
        "Sabuk",
        "5",
        String(registrationClosesTick),
        String(startsAtTick),
        String(endsAtTick),
        "2500",
        "2",
        campaignId,
        "0150",
        map,
      ],
    },
    "leader schedules live Sabuk campaign",
  );
  await pulseBoth(clients, 2);
  await sendClientCommand(
    leader,
    { type: "stage5Command", action: "conquest.register", args: ["Sabuk"] },
    "leader guild registers for Sabuk",
  );
  await sendClientCommand(
    recruit,
    { type: "stage5Command", action: "conquest.register", args: ["Sabuk"] },
    "rival guild registers for Sabuk",
  );
  await pulseBoth(clients, 6);
  const bothGuildsRegisteredExpression = `(() => {
    const guilds = window.__mir2Stage5?.state?.stage5Systems?.conquest?.campaigns?.sabuk?.registeredGuilds ?? [];
    return guilds.includes(${JSON.stringify(guildName)}) && guilds.includes(${JSON.stringify(rivalGuildName)});
  })()`;
  await waitUntilClient(
    leader,
    bothGuildsRegisteredExpression,
    "leader observes both Sabuk registrations",
    30_000,
  );
  await waitUntilClient(
    recruit,
    bothGuildsRegisteredExpression,
    "rival observes both Sabuk registrations",
    30_000,
  );
  await sendClientCommand(
    leader,
    { type: "stage5Command", action: "conquest.advance", args: [String(startsAtTick)] },
    "leader starts live Sabuk campaign",
  );
  await pulseBoth(clients, 6);
  await waitUntilClient(
    recruit,
    `(window.__mir2Stage5?.state?.stage5Systems?.conquest?.activeWars ?? []).includes("Sabuk")`,
    "rival observes active Sabuk war",
    30_000,
  );
  await sendClientCommand(
    recruit,
    { type: "transferMap", key: "crystal:0150:10:13" },
    "rival enters Sabuk palace",
  );
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.mapFileName === "0150" && window.__mir2Stage5?.state?.player?.x === 10 && window.__mir2Stage5?.state?.player?.y === 13`,
    "rival arrives in Sabuk palace",
    30_000,
  );
  await pulseBoth(clients, 6);
  await waitUntilClient(
    recruit,
    `(() => {
      const campaign = window.__mir2Stage5?.state?.stage5Systems?.conquest?.campaigns?.sabuk;
      return campaign?.captureCandidateGuild === ${JSON.stringify(rivalGuildName)} && Number(campaign?.captureProgressTicks ?? 0) >= 2;
    })()`,
    "rival Sabuk capture becomes authoritative",
    30_000,
  );
  await sendClientCommand(
    leader,
    { type: "stage5Command", action: "conquest.advance", args: [String(endsAtTick)] },
    "leader settles live Sabuk campaign",
  );
  await pulseBoth(clients, 6);
  await waitUntilClient(
    leader,
    `window.__mir2Stage5?.state?.stage5Systems?.conquest?.castleOwner === ${JSON.stringify(rivalGuildName)}`,
    "leader observes rival Sabuk ownership",
    30_000,
  );
  await waitUntilClient(
    recruit,
    `window.__mir2Stage5?.state?.stage5Systems?.conquest?.castleOwner === ${JSON.stringify(rivalGuildName)}`,
    "rival observes Sabuk ownership",
    30_000,
  );
  await sendClientCommand(
    recruit,
    { type: "stage5Command", action: "conquest.claimReward", args: ["Sabuk"] },
    "rival claims Sabuk reward",
  );
  await pulseBoth(clients, 2);
  await waitUntilClient(
    recruit,
    `Number(window.__mir2Stage5?.state?.stage5Systems?.guild?.storageGold ?? 0) >= 2500`,
    "rival receives Sabuk reward",
    15_000,
  );
  // The generic two-client movement/chat checks that follow this social loop
  // intentionally exercise same-map visibility. Return the palace occupant to
  // the original test map after the conquest evidence has been committed.
  await transferTo(recruit, startBx, startBy);
  await pulseBoth(clients, 3);

  const summaries = await Promise.all(clients.map((client) => readSummary(client)));
  const assertions = {
    acceptedGroupVisible: acceptedGuildSummaries.every(
      (summary) => summary.groupMembers.includes(characterA) && summary.groupMembers.includes(characterB),
    ),
    acceptedGuildVisible: acceptedGuildSummaries.every((summary) => summary.guildName === guildName),
    guildChatDelivered,
    tradeRequestDelivered,
    tradeAccepted,
    rivalGuildVisible: summaries[0].guildName === guildName && summaries[1].guildName === rivalGuildName,
    livePkDeath: pkEvidence.victimDead && pkEvidence.victimSawDeathPacket,
    livePkPoints: pkEvidence.attackerPkPoints > 0,
    livePkRevive: summaries[1].player?.dead === false && Number(summaries[1].playerHp ?? 0) > 0,
    sabukSharedOwner: summaries.every((summary) => summary.conquest?.castleOwner === rivalGuildName),
    sabukSettled: summaries.every(
      (summary) => summary.conquest?.campaigns?.sabuk?.phase === "settled",
    ),
    sabukRewardClaimed:
      summaries[1].conquest?.campaigns?.sabuk?.rewardClaimed === true &&
      Number(summaries[1].guildStorageGold ?? 0) >= 2500,
  };
  return {
    ok: Object.values(assertions).every(Boolean),
    guildName,
    rivalGuildName,
    guildMessage,
    campaignId,
    pkEvidence,
    acceptedGuildSummaries,
    assertions,
    summaries,
  };
}

async function waitForGatewayPacket(client, packet, label) {
  await waitUntilClient(
    client,
    `(window.__mir2GatewayEventHistory ?? []).some((event) => event?.packet === ${JSON.stringify(packet)})`,
    label,
    15_000,
  );
}

async function sendClientCommand(client, command, label) {
  const ok = await client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
  if (!ok) {
    throw new Error(`${label} command was not accepted by the Web client.`);
  }
}

async function readSummary(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      return {
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        accountId: state.accountId ?? null,
        player: state.player ?? null,
        playerHp: state.playerHp ?? null,
        playerMaxHp: state.playerMaxHp ?? null,
        playerPkPoints: state.playerPkPoints ?? null,
        playerObjectId: state.playerObjectId ?? null,
        mapFileName: state.mapFileName ?? null,
        worldTick: state.worldTick ?? null,
        bevyRuntime: window.__mir2BevyRuntimeDebug ?? null,
        bevyEntityRenderer: window.__mir2BevyEntityRendererDebug ?? null,
        bevyMovementShadow: state.bevyMovementShadow ?? null,
        logsTail: (state.logs ?? []).slice(-8).map((line) => line?.text ?? String(line)),
        groupMembers: (state.stage5Systems?.group?.members ?? []).map((member) =>
          typeof member === "string" ? member : member?.name,
        ),
        guildName: state.stage5Systems?.guild?.name ?? "",
        guildStorageGold: state.stage5Systems?.guild?.storageGold ?? 0,
        conquest: state.stage5Systems?.conquest ?? null,
        tradeState: state.stage5Systems?.trade ?? null,
        entities: (state.entities ?? [])
          .map((entity) => ({
            kind: entity.kind,
            name: entity.name,
            objectId: entity.objectId,
            x: entity.x,
            y: entity.y,
            direction: entity.direction,
            hp: entity.hp,
            maxHp: entity.maxHp,
            dead: entity.dead,
          }))
          .sort((left, right) => String(left.name).localeCompare(String(right.name))),
      };
    })()
  `);
}

async function captureScreenshot(client, fileName) {
  await client.send("Page.bringToFront");
  await waitUntilClient(
    client,
    `window.__mir2Stage5?.state?.sceneInteractionReady === true`,
    `${client.label} scene ready for screenshot`,
    15_000,
  ).catch(() => undefined);
  await delay(250);
  const result = await client.send("Page.captureScreenshot", {
    format: "png",
    captureBeyondViewport: false,
  });
  const screenshotPath = path.join(outputDir, fileName);
  await fs.writeFile(screenshotPath, Buffer.from(result.data, "base64"));
  return screenshotPath;
}

async function fillInput(client, selector, value) {
  const ok = await client.evaluate(`
    (() => {
      const input = document.querySelector(${JSON.stringify(selector)});
      if (!input) return false;
      const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value").set;
      setter.call(input, ${JSON.stringify(value)});
      input.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertText" }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      return true;
    })()
  `);
  if (!ok) {
    throw new Error(`${client.label} could not fill ${selector}`);
  }
}

async function click(client, selector) {
  const ok = await client.evaluate(`
    (() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (!node) return false;
      node.click();
      return true;
    })()
  `);
  if (!ok) {
    throw new Error(`${client.label} could not click ${selector}`);
  }
}

async function waitUntilClient(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastValue = null;
  while (Date.now() < deadline) {
    lastValue = await client.evaluate(`Boolean(${expression})`).catch((error) => ({ error: String(error) }));
    if (lastValue === true) {
      return;
    }
    await delay(100);
  }
  const debug = await client
    .evaluate(`
      (() => ({
        url: location.href,
        readyState: document.readyState,
        bodyText: document.body?.innerText?.slice(0, 500) ?? "",
        screen: window.__mir2Stage5?.state?.screen ?? null,
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        mapFileName: window.__mir2Stage5?.state?.mapFileName ?? null,
        player: window.__mir2Stage5?.state?.player ?? null,
        packetRuntime: window.__mir2PacketRuntime ?? null,
        entities: (window.__mir2Stage5?.state?.entities ?? []).map((entity) => ({
          kind: entity.kind,
          name: entity.name,
          x: entity.x,
          y: entity.y,
        })),
        gatewayEvents: (window.__mir2GatewayEventHistory ?? []).slice(-20).map((event) => ({
          packet: event?.packet ?? null,
          payload: event?.payload ?? null,
        })),
      }))()
    `)
    .catch((error) => ({ debugError: String(error) }));
  throw new Error(`Timed out waiting for ${label}; last=${JSON.stringify(lastValue)}; debug=${JSON.stringify(debug).slice(0, 8_000)}`);
}

async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-two-client-zone-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      "--disable-gpu",
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      "--no-proxy-server",
      "--proxy-bypass-list=*",
      "--no-first-run",
      "--no-default-browser-check",
      `--window-size=${DEFAULT_VIEWPORT.width},${DEFAULT_VIEWPORT.height}`,
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome();
  return chrome;
}

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) {
    return;
  }
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve)).catch(() => undefined);
  if (chrome.userDataDir) {
    await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => undefined);
  }
}

async function createPageTarget() {
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?about:blank`, { method: "PUT" });
  if (!response.ok) {
    throw new Error(`Chrome target creation failed: ${response.status}`);
  }
  return response.json();
}

async function waitForChrome() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
      if (response.ok) {
        return;
      }
    } catch {
      await delay(100);
    }
  }
  throw new Error("Timed out waiting for Chrome debug endpoint.");
}

async function setViewport(client) {
  await client.send("Emulation.setDeviceMetricsOverride", DEFAULT_VIEWPORT);
  await client.send("Emulation.setVisibleSize", {
    width: DEFAULT_VIEWPORT.width,
    height: DEFAULT_VIEWPORT.height,
  });
}

function buildBaseUrl(rawBaseUrl, wsUrl) {
  const url = new URL(rawBaseUrl);
  if (!url.searchParams.has("gatewayWs")) {
    url.searchParams.set("gatewayWs", wsUrl);
  }
  if (!url.searchParams.has("autoTick")) {
    url.searchParams.set("autoTick", "0");
  }
  for (const [key, value] of [
    ["bevyBackend", "webgpu"],
    ["bevyEntities", "1"],
    ["bevyMap", "1"],
    ["bevyEntityInterp", "1"],
    ["bevyRemoteMotion", "1"],
  ]) {
    if (!url.searchParams.has(key)) {
      url.searchParams.set(key, value);
    }
  }
  if (!url.searchParams.has("codexBust")) {
    url.searchParams.set("codexBust", String(Date.now()));
  }
  return url.toString();
}

function isZonePacketPayload(payloadData) {
  return /ObjectPlayer|ObjectWalk|ObjectRun|ObjectTurn|ObjectChat|ObjectRemove|UserLocation|GroupInvite|GuildInvite|TradeRequest|TradeAccept|DeleteGroup|GuildMemberChange/.test(payloadData);
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  ];
  return candidates.find((candidate) => fsSync.existsSync(candidate)) ?? null;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) {
      continue;
    }
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (!next || next.startsWith("--")) {
      parsed[key] = "true";
      continue;
    }
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

function numberArg(value, fallback) {
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") {
    return fallback;
  }
  if (typeof value === "boolean") {
    return value;
  }
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
