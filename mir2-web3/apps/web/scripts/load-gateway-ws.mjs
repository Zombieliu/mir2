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
const WS_ORIGIN = process.env.MIR2_GATEWAY_WS_ORIGIN?.trim() || null;
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
const PRE_PLAY_SETTLE_MS = numberFromEnv("MIR2_WS_LOAD_PRE_PLAY_SETTLE_MS", 0);
const CHAT_EVERY = numberFromEnv("MIR2_WS_LOAD_CHAT_EVERY", 10);
const READY_TIMEOUT_MS = numberFromEnv("MIR2_WS_LOAD_READY_TIMEOUT_MS", 15_000);
const CONNECT_STAGGER_MS = numberFromEnv("MIR2_WS_LOAD_CONNECT_STAGGER_MS", 0);
const WAIT_FOR_ALL_READY = booleanFromEnv("MIR2_WS_LOAD_WAIT_FOR_ALL_READY", false);
const ALL_READY_SETTLE_MS = numberFromEnv("MIR2_WS_LOAD_ALL_READY_SETTLE_MS", 0);
const CLOSE_TIMEOUT_MS = numberFromEnv("MIR2_WS_LOAD_CLOSE_TIMEOUT_MS", 5_000);
const EXPECT_READY = optionalNumberFromEnv("MIR2_WS_LOAD_EXPECT_READY");
const EXPECT_REJECTED = optionalNumberFromEnv("MIR2_WS_LOAD_EXPECT_REJECTED");
const EXPECT_KEEPALIVE_ACK_RATIO = optionalNumberFromEnv(
  "MIR2_WS_LOAD_EXPECT_KEEPALIVE_ACK_RATIO",
);
const EXPECT_KEEPALIVE_P95_MAX_MS = optionalNumberFromEnv(
  "MIR2_WS_LOAD_EXPECT_KEEPALIVE_P95_MAX_MS",
);
const EXPECT_STEADY_KEEPALIVE_P95_MAX_MS = optionalNumberFromEnv(
  "MIR2_WS_LOAD_EXPECT_STEADY_KEEPALIVE_P95_MAX_MS",
);
const EXPECT_COMBAT_TARGETED_MIN = optionalNumberFromEnv(
  "MIR2_WS_LOAD_EXPECT_COMBAT_TARGETED_MIN",
);
const EXPECT_INVENTORY_ELIGIBLE_MIN = optionalNumberFromEnv(
  "MIR2_WS_LOAD_EXPECT_INVENTORY_ELIGIBLE_MIN",
);
const SEND_KEEPALIVE = booleanFromEnv("MIR2_WS_LOAD_SEND_KEEPALIVE", true);
const SEND_MOVEMENT = booleanFromEnv("MIR2_WS_LOAD_SEND_MOVEMENT", true);
const SEND_CHAT = booleanFromEnv("MIR2_WS_LOAD_SEND_CHAT", true);
const ENABLE_STAGE5_COMMANDS = booleanFromEnv(
  "MIR2_WS_LOAD_ENABLE_STAGE5_COMMANDS",
  false,
);
const SEND_COMBAT = booleanFromEnv("MIR2_WS_LOAD_SEND_COMBAT", false);
const SEND_INVENTORY = booleanFromEnv("MIR2_WS_LOAD_SEND_INVENTORY", false);
const COMBAT_EVERY = numberFromEnv("MIR2_WS_LOAD_COMBAT_EVERY", 5);
const INVENTORY_EVERY = numberFromEnv("MIR2_WS_LOAD_INVENTORY_EVERY", 30);
const GATEWAY_PID = optionalNumberFromEnv("MIR2_GATEWAY_PID");
const REUSE_EXISTING_ACCOUNTS = booleanFromEnv(
  "MIR2_WS_LOAD_REUSE_EXISTING_ACCOUNTS",
  false,
);
const RECONNECT_ONCE = booleanFromEnv("MIR2_WS_LOAD_RECONNECT_ONCE", false);
const RECONNECT_SETTLE_MS = numberFromEnv("MIR2_WS_LOAD_RECONNECT_SETTLE_MS", 250);
const RECONNECT_HOLD_OPEN_MS = numberFromEnv(
  "MIR2_WS_LOAD_RECONNECT_HOLD_OPEN_MS",
  250,
);
const LOOPBACK_SOURCE_SHARDING = booleanFromEnv(
  "MIR2_WS_LOAD_LOOPBACK_SOURCE_SHARDING",
  false,
);
const ACCOUNT_PREFIX = process.env.MIR2_WS_LOAD_ACCOUNT_PREFIX ?? null;
const ACCOUNT_PREFIXES = (process.env.MIR2_WS_LOAD_ACCOUNT_PREFIXES ?? "")
  .split(",")
  .map((value) => value.trim())
  .filter(Boolean);
const ACCOUNT_PASSWORD = process.env.MIR2_WS_LOAD_PASSWORD ?? "load-pass";
const ACCOUNT_PASSWORDS_RAW = process.env.MIR2_WS_LOAD_PASSWORDS?.trim() ?? "";
const ACCOUNT_PASSWORDS =
  ACCOUNT_PASSWORDS_RAW === ""
    ? []
    : ACCOUNT_PASSWORDS_RAW.split(",").map((value) => value.trim());
const ACCOUNT_IDS = (process.env.MIR2_WS_LOAD_ACCOUNT_IDS ?? "")
  .split(",")
  .map((value) => value.trim())
  .filter(Boolean);
const CHARACTER_INDEX = numberFromEnv("MIR2_WS_LOAD_CHARACTER_INDEX", 0);

async function main() {
  const runId = `${Date.now().toString(36)}-${process.pid}`;
  const metrics = {
    type: "websocket",
    wsUrl: WS_URL,
    wsOrigin: WS_ORIGIN,
    runId,
    host: os.hostname(),
    clients: CLIENTS,
    pool: POOL,
    actionsPerClient: ACTIONS,
    thinkMs: THINK_MS,
    holdOpenMs: HOLD_OPEN_MS,
    prePlaySettleMs: PRE_PLAY_SETTLE_MS,
    connectStaggerMs: CONNECT_STAGGER_MS,
    waitForAllReady: WAIT_FOR_ALL_READY,
    allReadySettleMs: ALL_READY_SETTLE_MS,
    chatEvery: CHAT_EVERY,
    expectedReady: EXPECT_READY ?? CLIENTS,
    expectedCapacityRejected: EXPECT_REJECTED ?? 0,
    expectedKeepAliveAckRatio: EXPECT_KEEPALIVE_ACK_RATIO,
    expectedKeepAliveP95MaxMs: EXPECT_KEEPALIVE_P95_MAX_MS,
    expectedSteadyKeepAliveP95MaxMs: EXPECT_STEADY_KEEPALIVE_P95_MAX_MS,
    expectedCombatTargetedMin: EXPECT_COMBAT_TARGETED_MIN,
    expectedInventoryEligibleMin: EXPECT_INVENTORY_ELIGIBLE_MIN,
    sendKeepAlive: SEND_KEEPALIVE,
    sendMovement: SEND_MOVEMENT,
    sendChat: SEND_CHAT,
    sendCombat: SEND_COMBAT,
    sendInventory: SEND_INVENTORY,
    combatEvery: COMBAT_EVERY,
    inventoryEvery: INVENTORY_EVERY,
    stage5CommandsEnabled: ENABLE_STAGE5_COMMANDS,
    reuseExistingAccounts: REUSE_EXISTING_ACCOUNTS,
    reconnectOnce: RECONNECT_ONCE,
    reconnectSettleMs: RECONNECT_SETTLE_MS,
    reconnectHoldOpenMs: RECONNECT_HOLD_OPEN_MS,
    loopbackSourceSharding: LOOPBACK_SOURCE_SHARDING,
    accountPrefix: ACCOUNT_PREFIX,
    accountPrefixes: ACCOUNT_PREFIXES,
    accountIds: ACCOUNT_IDS,
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
    bootstrapEvents: [],
    commandsSent: 0,
    keepAliveCommandsSent: 0,
    movementCommandsSent: 0,
    chatCommandsSent: 0,
    stage5CommandsSent: 0,
    combatCommandsSent: 0,
    combatTargetedAttacksSent: 0,
    combatChaseCommandsSent: 0,
    inventoryCommandsSent: 0,
    inventoryEligibleClients: 0,
    reconnectAttempts: 0,
    reconnectSucceeded: 0,
    keepAliveLatenciesMs: [],
    steadyKeepAliveLatenciesMs: [],
    phaseLatenciesMs: {
      connect: [],
      login: [],
      newCharacter: [],
      startGame: [],
      reconnectConnect: [],
      reconnectLogin: [],
      reconnectStartGame: [],
    },
    clientFailures: [],
    capacityRejections: [],
    serverErrors: [],
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
  metrics.steadyKeepAlive = summarize(metrics.steadyKeepAliveLatenciesMs);
  metrics.phaseLatencies = Object.fromEntries(
    Object.entries(metrics.phaseLatenciesMs).map(([phase, values]) => [
      phase,
      summarize(values),
    ]),
  );
  metrics.keepAliveAckRatio =
    metrics.keepAliveCommandsSent === 0
      ? 1
      : Math.round(
          (metrics.keepAliveLatenciesMs.length / metrics.keepAliveCommandsSent) * 10000,
        ) / 10000;
  metrics.pendingKeepAliveCount = Math.max(
    0,
    metrics.keepAliveCommandsSent - metrics.keepAliveLatenciesMs.length,
  );
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
      ((!SEND_KEEPALIVE || metrics.keepAliveCommandsSent >= metrics.ready) &&
        (!SEND_MOVEMENT || metrics.movementCommandsSent >= metrics.ready) &&
        (!SEND_CHAT || metrics.chatCommandsSent >= metrics.ready)),
    keepAliveAckRatioMatchesExpectation:
      EXPECT_KEEPALIVE_ACK_RATIO === null ||
      metrics.keepAliveAckRatio >= EXPECT_KEEPALIVE_ACK_RATIO,
    keepAliveP95MatchesExpectation:
      EXPECT_KEEPALIVE_P95_MAX_MS === null ||
      (metrics.keepAlive.p95 !== null && metrics.keepAlive.p95 <= EXPECT_KEEPALIVE_P95_MAX_MS),
    steadyKeepAliveP95MatchesExpectation:
      EXPECT_STEADY_KEEPALIVE_P95_MAX_MS === null ||
      (metrics.steadyKeepAlive.p95 !== null &&
        metrics.steadyKeepAlive.p95 <= EXPECT_STEADY_KEEPALIVE_P95_MAX_MS),
    reconnectsSucceeded:
      !RECONNECT_ONCE ||
      (metrics.reconnectAttempts === metrics.ready &&
        metrics.reconnectSucceeded === metrics.reconnectAttempts),
    combatCommandsIssued: !SEND_COMBAT || metrics.combatCommandsSent > 0,
    combatTargetedMatchesExpectation:
      EXPECT_COMBAT_TARGETED_MIN === null ||
      metrics.combatTargetedAttacksSent >= EXPECT_COMBAT_TARGETED_MIN,
    inventoryCommandsIssued: !SEND_INVENTORY || metrics.inventoryCommandsSent > 0,
    inventoryEligibleMatchesExpectation:
      EXPECT_INVENTORY_ELIGIBLE_MIN === null ||
      metrics.inventoryEligibleClients >= EXPECT_INVENTORY_ELIGIBLE_MIN,
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
  if (CONNECT_STAGGER_MS > 0 && index > 0) {
    await delay(index * CONNECT_STAGGER_MS);
  }
  const accountId = loadAccountId(index, runId);
  const characterName = `Load${index}${runId.replace(/[^a-z0-9]/gi, "").slice(-6)}`.slice(
    0,
    10,
  );
  const password = loadAccountPassword(index);
  const pendingKeepAlives = new Map();
  const initialState = newClientState(index);
  const ws = createLoadSocket(metrics, initialState, pendingKeepAlives, false);
  let reconnectWs = null;

  try {
    const connectStartedAt = Date.now();
    await ws.connect();
    metrics.phaseLatenciesMs.connect.push(Date.now() - connectStartedAt);
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
    const loginStartedAt = Date.now();
    send(ws, metrics, { type: "login", accountId, password });
    await waitFor(
      () => initialState.loginSuccess,
      READY_TIMEOUT_MS,
      `login ${index}`,
      () => initialState.serverError || ws.closed,
    );
    metrics.phaseLatenciesMs.login.push(Date.now() - loginStartedAt);
    if (REUSE_EXISTING_ACCOUNTS) {
      initialState.createdCharacterIndex =
        initialState.loginCharacterIndex ?? CHARACTER_INDEX;
    } else {
      const newCharacterStartedAt = Date.now();
      send(ws, metrics, {
        type: "newCharacter",
        name: characterName,
        gender: index % 2 === 0 ? "Male" : "Female",
        class: ["Warrior", "Wizard", "Taoist"][index % 3],
      });
      await waitFor(
        () => initialState.createdCharacterIndex !== null,
        READY_TIMEOUT_MS,
        `newCharacter ${index}`,
        () => initialState.serverError || ws.closed,
      );
      metrics.phaseLatenciesMs.newCharacter.push(Date.now() - newCharacterStartedAt);
    }
    const startGameStartedAt = Date.now();
    send(ws, metrics, {
      type: "startGame",
      characterIndex: initialState.createdCharacterIndex,
    });
    await waitFor(
      () => initialState.userInformation || initialState.capacityRejected,
      READY_TIMEOUT_MS,
      `startGame ${index}`,
      () => initialState.serverError || ws.closed,
    );
    metrics.phaseLatenciesMs.startGame.push(Date.now() - startGameStartedAt);
    if (initialState.capacityRejected) {
      return {
        index,
        ready: false,
        capacityRejected: true,
        error: initialState.capacityError,
      };
    }
    if (PRE_PLAY_SETTLE_MS > 0) {
      await delay(PRE_PLAY_SETTLE_MS);
    }
    metrics.ready += 1;
    if (WAIT_FOR_ALL_READY) {
      await waitFor(
        () =>
          metrics.ready +
            metrics.clientFailures.length +
            metrics.capacityRejections.length ===
          CLIENTS,
        READY_TIMEOUT_MS,
        `all clients ready ${index}`,
      );
      if (ALL_READY_SETTLE_MS > 0) {
        await delay(ALL_READY_SETTLE_MS);
      }
    }
    const bytesSentAtReady = ws.bytesSent;
    const bytesReceivedAtReady = ws.bytesReceived;
    const playStartedAt = Date.now();

    const directions = ["Right", "Down", "Left", "Up"];
    for (let action = 0; action < ACTIONS; action += 1) {
      const time = Date.now() * 1000 + index * 100 + action;
      const player = initialState.playerLocation;
      const combatTarget =
        SEND_COMBAT && player ? nearestVisibleMonster(initialState, player) : null;
      const combatDistance =
        combatTarget && player
          ? Math.max(
              Math.abs(combatTarget.x - player.x),
              Math.abs(combatTarget.y - player.y),
            )
          : null;
      if (SEND_KEEPALIVE) {
        pendingKeepAlives.set(time, {
          sentAtMs: Date.now(),
          steady: metrics.ready === CLIENTS,
        });
      }
      if (SEND_KEEPALIVE && send(ws, metrics, { type: "keepAlive", time })) {
        metrics.keepAliveCommandsSent += 1;
      }
      if (SEND_MOVEMENT) {
        let movementCommand = {
          type: action % 3 === 0 ? "run" : "walk",
          direction: directions[action % directions.length],
        };
        if (combatTarget && player && combatDistance > 1) {
          movementCommand = {
            type: combatDistance > 3 ? "run" : "walk",
            direction: directionToward(
              combatTarget.x - player.x,
              combatTarget.y - player.y,
            ),
          };
        }
        if (send(ws, metrics, movementCommand)) {
          metrics.movementCommandsSent += 1;
          if (combatTarget && combatDistance > 1) {
            metrics.combatCommandsSent += 1;
            metrics.combatChaseCommandsSent += 1;
          }
        }
      }
      if (SEND_CHAT && isStaggeredCadenceDue(action, index, CHAT_EVERY)) {
        if (send(ws, metrics, { type: "chat", message: `load ${index}:${action}` })) {
          metrics.chatCommandsSent += 1;
        }
      }
      if (SEND_COMBAT && isStaggeredCadenceDue(action, index, COMBAT_EVERY)) {
        let command = null;
        if (combatTarget && combatDistance !== null && combatDistance <= 1) {
          command = { type: "attack", objectId: combatTarget.objectId };
        } else if (!combatTarget) {
          command = {
            type: "attackDirection",
            direction: directions[action % directions.length],
          };
        }
        if (command && send(ws, metrics, command)) {
          metrics.combatCommandsSent += 1;
          if (command.type === "attack") metrics.combatTargetedAttacksSent += 1;
        }
      }
      if (
        SEND_INVENTORY &&
        initialState.inventoryMove !== null &&
        isStaggeredCadenceDue(action, index, INVENTORY_EVERY)
      ) {
        const move = initialState.inventoryMove;
        if (send(ws, metrics, {
          type: "moveItem",
          grid: "inventory",
          from: move.from,
          to: move.to,
        })) {
          metrics.inventoryCommandsSent += 1;
          initialState.inventoryMove = { from: move.to, to: move.from };
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

    if (RECONNECT_ONCE) {
      metrics.reconnectAttempts += 1;
      ws.close();
      await waitFor(() => ws.closed, CLOSE_TIMEOUT_MS, `disconnect ${index}`);
      await delay(RECONNECT_SETTLE_MS);

      const reconnectState = newClientState(index);
      reconnectWs = createLoadSocket(metrics, reconnectState, pendingKeepAlives, true);
      const reconnectConnectStartedAt = Date.now();
      await reconnectWs.connect();
      metrics.phaseLatenciesMs.reconnectConnect.push(
        Date.now() - reconnectConnectStartedAt,
      );
      metrics.opened += 1;

      send(reconnectWs, metrics, { type: "clientVersion" });
      const reconnectLoginStartedAt = Date.now();
      send(reconnectWs, metrics, { type: "login", accountId, password });
      await waitFor(
        () => reconnectState.loginSuccess,
        READY_TIMEOUT_MS,
        `reconnect login ${index}`,
        () => reconnectState.serverError || reconnectWs.closed,
      );
      metrics.phaseLatenciesMs.reconnectLogin.push(
        Date.now() - reconnectLoginStartedAt,
      );

      const reconnectStartGameStartedAt = Date.now();
      send(reconnectWs, metrics, {
        type: "startGame",
        characterIndex: initialState.createdCharacterIndex,
      });
      await waitFor(
        () => reconnectState.userInformation || reconnectState.capacityRejected,
        READY_TIMEOUT_MS,
        `reconnect startGame ${index}`,
        () => reconnectState.serverError || reconnectWs.closed,
      );
      metrics.phaseLatenciesMs.reconnectStartGame.push(
        Date.now() - reconnectStartGameStartedAt,
      );
      if (reconnectState.capacityRejected) {
        return {
          index,
          ready: false,
          capacityRejected: true,
          error: reconnectState.capacityError,
        };
      }
      metrics.reconnectSucceeded += 1;
      await delay(RECONNECT_HOLD_OPEN_MS);
    }

    return { index, ready: true };
  } finally {
    if (reconnectWs && !reconnectWs.closed) {
      reconnectWs.close();
      await waitFor(
        () => reconnectWs.closed,
        CLOSE_TIMEOUT_MS,
        `reconnect close ${index}`,
      ).catch(() => {});
    }
    if (!ws.closed) {
      ws.close();
      await waitFor(() => ws.closed, CLOSE_TIMEOUT_MS, `close ${index}`).catch(() => {});
    }
  }
}

function isStaggeredCadenceDue(action, clientIndex, every) {
  if (!Number.isFinite(every) || every <= 0) return false;
  const phase = (clientIndex * 97) % every;
  return (action + phase) % every === 0;
}

function nearestVisibleMonster(state, player) {
  let nearest = null;
  let nearestDistance = Number.POSITIVE_INFINITY;
  for (const monster of state.visibleMonsters.values()) {
    const distance = Math.max(
      Math.abs(monster.x - player.x),
      Math.abs(monster.y - player.y),
    );
    if (distance < nearestDistance) {
      nearest = monster;
      nearestDistance = distance;
    }
  }
  return nearest;
}

function directionToward(dx, dy) {
  const horizontal = Math.sign(dx);
  const vertical = Math.sign(dy);
  if (horizontal > 0 && vertical > 0) return "DownRight";
  if (horizontal > 0 && vertical < 0) return "UpRight";
  if (horizontal < 0 && vertical > 0) return "DownLeft";
  if (horizontal < 0 && vertical < 0) return "UpLeft";
  if (horizontal > 0) return "Right";
  if (horizontal < 0) return "Left";
  if (vertical > 0) return "Down";
  return "Up";
}

function loadAccountId(index, runId) {
  if (ACCOUNT_IDS.length > 0) {
    if (index >= ACCOUNT_IDS.length) {
      throw new Error(
        `MIR2_WS_LOAD_ACCOUNT_IDS provides ${ACCOUNT_IDS.length} accounts for client ${index}`,
      );
    }
    return ACCOUNT_IDS[index];
  }
  if (ACCOUNT_PREFIXES.length > 0) {
    const prefix = ACCOUNT_PREFIXES[index % ACCOUNT_PREFIXES.length];
    const accountIndex = Math.floor(index / ACCOUNT_PREFIXES.length);
    return `${prefix}${accountIndex}`;
  }
  return ACCOUNT_PREFIX === null
    ? `load-ws-${runId}-${index}`
    : `${ACCOUNT_PREFIX}${index}`;
}

function loadAccountPassword(index) {
  if (ACCOUNT_PASSWORDS.length > 0) {
    const passwordIndex =
      ACCOUNT_PREFIXES.length > 0 ? index % ACCOUNT_PREFIXES.length : index;
    const password = ACCOUNT_PASSWORDS[passwordIndex];
    if (!password) {
      throw new Error(
        `MIR2_WS_LOAD_PASSWORDS does not provide a password for client ${index}`,
      );
    }
    return password;
  }
  return ACCOUNT_PASSWORD;
}

function newClientState(index) {
  return {
    index,
    loginSuccess: false,
    loginCharacterIndex: null,
    userInformation: false,
    createdCharacterIndex: null,
    capacityRejected: false,
    capacityError: null,
    serverError: null,
    inventoryMove: null,
    playerLocation: null,
    visibleMonsters: new Map(),
  };
}

function createLoadSocket(metrics, state, pendingKeepAlives, reconnect) {
  return new RawWebSocketClient(
    WS_URL,
    (text) => {
      metrics.messages += 1;
      let payload;
      try {
        payload = JSON.parse(text);
      } catch {
        return;
      }
      if (metrics.bootstrapEvents.length < CLIENTS * 16) {
        metrics.bootstrapEvents.push({
          index: state.index,
          reconnect,
          type: String(payload.type ?? ""),
          packet: String(payload.packet ?? ""),
        });
      }
      if (payload.type === "error") {
        const message = String(payload.message ?? "");
        state.serverError = message || "gateway returned an unspecified error";
        metrics.serverErrors.push({ index: state.index, reconnect, message: state.serverError });
        if (isCapacityError(message)) {
          state.capacityRejected = true;
          state.capacityError = message;
        }
      }
      if (payload.packet === "LoginSuccess" && !state.loginSuccess) {
        const characters = payload.payload?.characters;
        if (Array.isArray(characters) && characters.length > 0) {
          state.loginCharacterIndex = characters[0]?.index ?? null;
        }
        state.loginSuccess = true;
        if (!reconnect) metrics.loginSuccess += 1;
      }
      if (payload.packet === "NewCharacterSuccess") {
        state.createdCharacterIndex = payload.payload?.character?.index ?? null;
        if (!reconnect) metrics.charactersCreated += 1;
      }
      if (payload.packet === "UserInformation" && !state.userInformation) {
        state.playerLocation = payload.payload?.location ?? null;
        state.userInformation = true;
        if (!reconnect) metrics.startedGames += 1;
      }
      if (
        payload.packet === "UserLocation" &&
        Number.isFinite(payload.payload?.x) &&
        Number.isFinite(payload.payload?.y)
      ) {
        state.playerLocation = { x: payload.payload.x, y: payload.payload.y };
      }
      if (payload.packet === "ObjectMonster") {
        const info = payload.payload;
        if (
          Number.isFinite(info?.objectId) &&
          Number.isFinite(info?.location?.x) &&
          Number.isFinite(info?.location?.y) &&
          !info?.dead &&
          !/guard/i.test(String(info?.name ?? ""))
        ) {
          state.visibleMonsters.set(info.objectId, {
            objectId: info.objectId,
            x: info.location.x,
            y: info.location.y,
          });
        }
      }
      if (payload.type === "worldSnapshot" && payload.payload) {
        const snapshot = payload.payload;
        const self = Array.isArray(snapshot.entities)
          ? snapshot.entities.find(
              (entity) => String(entity?.kind ?? "").toLowerCase() === "selfplayer",
            )
          : null;
        if (Number.isFinite(self?.x) && Number.isFinite(self?.y)) {
          state.playerLocation = { x: self.x, y: self.y };
        }
        if (Array.isArray(snapshot.entities)) {
          for (const entity of snapshot.entities) {
            if (
              String(entity?.kind ?? "").toLowerCase() === "monster" &&
              Number.isFinite(entity?.objectId) &&
              Number.isFinite(entity?.x) &&
              Number.isFinite(entity?.y) &&
              !entity?.dead &&
              !/guard/i.test(String(entity?.name ?? ""))
            ) {
              state.visibleMonsters.set(entity.objectId, {
                objectId: entity.objectId,
                x: entity.x,
                y: entity.y,
              });
            }
          }
        }
        if (state.inventoryMove === null && Array.isArray(snapshot.inventoryItems)) {
          const bagIndexes = snapshot.inventoryItems
            .filter((item) => /bag[12]/i.test(String(item?.container ?? "")))
            .map((item) => Number(item.slot) + (/bag2/i.test(item.container) ? 40 : 0))
            .filter(Number.isFinite);
          const occupied = bagIndexes[0];
          const maxBagSlots = Math.max(1, Number(snapshot.maxBagSlots) || 80);
          const used = new Set(bagIndexes);
          let empty = null;
          for (let slot = 0; slot < maxBagSlots; slot += 1) {
            if (!used.has(slot)) {
              empty = slot;
              break;
            }
          }
          if (Number.isFinite(occupied) && Number.isFinite(empty)) {
            state.inventoryMove = { from: occupied, to: empty };
            if (!reconnect) metrics.inventoryEligibleClients += 1;
          }
        }
      }
      if (payload.packet === "ObjectRemove" && Number.isFinite(payload.payload?.objectId)) {
        state.visibleMonsters.delete(payload.payload.objectId);
      }
      const keepAliveTime = payload.payload?.time;
      if (payload.packet === "KeepAlive" && pendingKeepAlives.has(keepAliveTime)) {
        const pending = pendingKeepAlives.get(keepAliveTime);
        const latencyMs = Date.now() - pending.sentAtMs;
        metrics.keepAliveLatenciesMs.push(latencyMs);
        if (pending.steady) {
          metrics.steadyKeepAliveLatenciesMs.push(latencyMs);
        }
        pendingKeepAlives.delete(keepAliveTime);
      }
    },
    () => {
      metrics.errors += 1;
    },
    () => {
      metrics.closed += 1;
    },
    {
      sourceAddress: loadSourceAddress(state.index),
      userAgent: `mir2-load/${state.index}`,
    },
  );
}

function loadSourceAddress(index) {
  if (!LOOPBACK_SOURCE_SHARDING) return null;
  const ordinal = index + 1;
  const thirdOctet = Math.floor((ordinal - 1) / 254);
  if (thirdOctet > 254) {
    throw new Error("loopback source sharding supports at most 64,770 clients");
  }
  const fourthOctet = ((ordinal - 1) % 254) + 1;
  return `127.64.${thirdOctet}.${fourthOctet}`;
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
  constructor(url, onMessage, onError, onClose, options = {}) {
    this.url = new URL(url);
    if (this.url.protocol !== "ws:" && this.url.protocol !== "wss:") {
      throw new Error(`Unsupported WebSocket protocol for load harness: ${this.url.protocol}`);
    }
    this.onMessage = onMessage;
    this.onError = onError;
    this.onClose = onClose;
    this.sourceAddress = options.sourceAddress ?? null;
    this.userAgent = options.userAgent ?? "mir2-load";
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
          localAddress: this.sourceAddress ?? undefined,
        })
      : net.createConnection({
          host,
          port,
          localAddress: this.sourceAddress ?? undefined,
        });
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
      ...(WS_ORIGIN ? [`Origin: ${WS_ORIGIN}`] : []),
      `User-Agent: ${this.userAgent}`,
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

async function waitFor(predicate, timeoutMs, label, abortPredicate = null) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return;
    const abortReason = abortPredicate?.();
    if (abortReason) {
      throw new Error(
        typeof abortReason === "string"
          ? abortReason
          : `Connection closed while waiting for ${label}`,
      );
    }
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
