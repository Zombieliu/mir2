// qa-persistence.mjs — persistence + reconnect-resume QA loop.
//
// Verifies the one journey the existing qa-playthrough.mjs never touches:
// does a player's world state SURVIVE a logout/login and a WebSocket blip?
// Three steps, each broken sub-step recorded as a structured issue:
//
//   1. Create a fresh account + character, enter the world, walk to a known
//      tile, and snapshot the authoritative state (map, position, inventory,
//      equipment, quests, gold, level).
//   2. LOG OUT (ClientPacket::LogOut → persist_active_character_save, the real
//      Crystal save path) then LOG IN again from a fresh page load with the same
//      account, re-enter the same character, and assert every snapshot field
//      came back (character present in the list, same map/position/inventory/
//      quests/level/gold).
//   3. RECONNECT: simulate a network blip with CDP Network.emulateNetworkConditions
//      (offline → wait → online; force-closing the socket as a fallback) and
//      assert the client RESUMES the same session — recovers to screen "game"
//      with reconnectStatus "idle", never bounces to login/select, keeps the same
//      playerObjectId, and does not duplicate the self entity.
//
// Why a real browser (CDP) and not a protocol bot: only the live client exercises
// the gateway reconnect-resume sequence (clientVersion + login + startGame on
// socket re-open, apps/web/app/page.tsx:4431) and the screen/state machine a
// player actually sees.
//
// IMPORTANT — persistence is REAL only against the Rust gateway + its store.
// The :7110 node ws-proxy is in-memory and will not reflect a true logout/login
// round-trip, so this harness points the client at the Rust gateway on :7111 via
// the `?gatewayWs=` localhost query override (resolveGatewayWebSocketUrl,
// apps/web/app/page.tsx:988). Override with --gatewayWs if yours differs.
//
// Self-contained: reuses the proven CdpClient / launchChrome / login patterns
// from qa-playthrough.mjs + smoke-reconnect-resume.mjs, zero new dependencies.
//
// Usage:
//   node ./scripts/qa-persistence.mjs --headed --baseUrl http://127.0.0.1:3024
//        [--gatewayWs ws://127.0.0.1:7111/ws]
//        [--account NAME --password PW --characterName NAME]
//        [--offlineMs 4000] [--runId my-run]
//
// Exit code is non-zero if any high-severity persistence/reconnect issue is
// recorded, so it doubles as a pass/fail gate. A full report (json + md) lands
// under docs/generated/player-qa/persistence-<runId>/.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrlRaw = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3024";
// Point the client at the Rust gateway (:7111) so the test reflects TRUE
// persistence. The `gatewayWs` query override is honored only on localhost.
const gatewayWsUrl = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:7111/ws";
const RUN_URL = buildRunUrl(baseUrlRaw, gatewayWsUrl);

const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultName("ACC");
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultName("PER");
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 250));
const offlineMs = numberArg(args.offlineMs ?? process.env.MIR2_QA_OFFLINE_MS, 4000);
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "player-qa", `persistence-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const STAGE_READY_TIMEOUT_MS = numberArg(args.stageReadyTimeoutMs, 25_000);
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const ENTER_GAME_TIMEOUT_MS = numberArg(args.enterGameTimeoutMs, 30_000);
const RECONNECT_RECOVER_TIMEOUT_MS = numberArg(args.reconnectRecoverTimeoutMs, 50_000);
const MOVE_SETTLE_MS = 5_000;

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client — captures console, network failures, and WebSocket frames so the
// report can show the gateway's authoritative truth around each transition.
// (Same shape as qa-playthrough.mjs's client.)
// ---------------------------------------------------------------------------
class CdpClient {
  constructor(wsUrl) {
    this.wsUrl = wsUrl;
    this.nextId = 1;
    this.pending = new Map();
    this.consoleMessages = [];
    this.consoleErrors = [];
    this.networkFailures = [];
    this.requestUrlById = new Map();
    this.webSocketFramesReceived = [];
    this.webSocketFramesSent = [];
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
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);
      if (message.error) reject(new Error(`${message.error.message}: ${message.error.data ?? ""}`));
      else resolve(message.result ?? {});
      return;
    }

    const m = message.method;
    const p = message.params ?? {};

    if (m === "Runtime.consoleAPICalled") {
      const entry = {
        source: "console",
        type: p.type ?? "log",
        text: (p.args ?? []).map((arg) => arg.value ?? arg.description ?? "").join(" "),
        at: Date.now(),
      };
      this.consoleMessages.push(entry);
      this.consoleMessages = this.consoleMessages.slice(-400);
      if (p.type === "error" || p.type === "warning") this.consoleErrors.push(entry);
    } else if (m === "Runtime.exceptionThrown") {
      const d = p.exceptionDetails;
      this.consoleErrors.push({
        source: "exception",
        type: "error",
        text: d?.exception?.description ?? d?.text ?? "runtime exception",
        at: Date.now(),
      });
    } else if (m === "Log.entryAdded") {
      const entry = p.entry;
      if (entry?.level === "error" && !String(entry.url ?? "").includes("favicon")) {
        this.consoleErrors.push({
          source: entry.source ?? "log",
          type: "error",
          text: `${entry.text ?? ""}${entry.url ? ` (${entry.url})` : ""}`,
          at: Date.now(),
        });
      }
    } else if (m === "Network.requestWillBeSent") {
      if (p.requestId && p.request?.url) this.requestUrlById.set(p.requestId, p.request.url);
    } else if (m === "Network.responseReceived") {
      const r = p.response;
      const url = String(r?.url ?? "");
      if (r && r.status >= 400 && !url.includes("favicon")) {
        this.networkFailures.push({ url, status: r.status, at: Date.now() });
      }
    } else if (m === "Network.loadingFailed") {
      const url = this.requestUrlById.get(p.requestId) ?? "(unknown)";
      if (!String(url).includes("favicon")) {
        this.networkFailures.push({ url, status: 0, errorText: p.errorText ?? "", at: Date.now() });
      }
    } else if (m === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-800);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-800);
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
      userGesture: true,
    });
    if (result.exceptionDetails) {
      throw new Error(`Evaluation failed: ${result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails)}`);
    }
    return result.result?.value;
  }

  close() {
    this.ws?.close();
  }
}

// ---------------------------------------------------------------------------
// Run context + issue collection.
// ---------------------------------------------------------------------------
function createContext(client) {
  const issues = [];
  const steps = [];
  return {
    client,
    issues,
    steps,
    seq: 0,
    log: (msg) => console.log(msg),
    addIssue(issue) {
      const id = `${issue.category}-${String(issues.length + 1).padStart(3, "0")}`;
      const full = { id, severity: "medium", ...issue };
      issues.push(full);
      console.log(`  ⚠ [${full.severity}/${full.category}] ${full.summary}`);
      return full;
    },
  };
}

async function runStep(ctx, title, fn) {
  ctx.seq += 1;
  const id = ctx.seq;
  const step = { id, title, startedAt: Date.now(), ok: true };
  ctx.log(`\n▶ step ${id}: ${title}`);
  try {
    await fn(step);
  } catch (err) {
    step.ok = false;
    step.error = String(err?.message ?? err);
    ctx.addIssue({
      category: "flow",
      severity: "high",
      step: id,
      summary: `step blocked: ${title}`,
      detail: step.error,
    });
    ctx.log(`  ✖ ${step.error.slice(0, 300)}`);
  }
  const frameName = `${String(id).padStart(2, "0")}-${slug(title)}.png`;
  try {
    await captureFrame(ctx.client, path.join(framesDir, frameName));
    step.frame = `frames/${frameName}`;
  } catch {
    /* screenshot is best-effort */
  }
  step.endedAt = Date.now();
  step.durationMs = step.endedAt - step.startedAt;
  ctx.steps.push(step);
  return step;
}

// ---------------------------------------------------------------------------
// Authoritative persistence-state reader — the compact snapshot we diff across
// logout/login and reconnect. Reads the same `window.__mir2Stage5.state` the UI
// renders from (apps/web/app/page.tsx:4234), so it is exactly what the player has.
// ---------------------------------------------------------------------------
async function readPersistenceState(client) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const selfId = s.playerObjectId ?? null;
      const self = ents.find((e) => e && String(e.objectId) === String(selfId)) ?? null;
      const inv = Array.isArray(s.inventoryItems) ? s.inventoryItems : [];
      const equip = Array.isArray(s.equipmentItems) ? s.equipmentItems : [];
      const quests = Array.isArray(s.questLog) ? s.questLog : [];
      // Duplicate detection: any objectId that appears more than once in the
      // entity list is a render/state duplication bug after a resume.
      const counts = {};
      for (const e of ents) { const k = String(e?.objectId); counts[k] = (counts[k] || 0) + 1; }
      const duplicateObjectIds = Object.entries(counts)
        .filter(([, n]) => n > 1)
        .map(([objectId, n]) => ({ objectId, count: n }));
      const itemSig = (it) => ({
        slot: it?.slot ?? it?.index ?? null,
        name: it?.name ?? null,
        itemIndex: it?.itemIndex ?? it?.itemId ?? null,
        count: it?.count ?? it?.quantity ?? null,
      });
      return {
        capturedAt: Date.now(),
        screen: s.screen ?? null,
        wsState: s.wsState ?? null,
        reconnectMode: s.reconnectStatus?.mode ?? null,
        accountId: s.accountId ?? null,
        selectedCharacterIndex: s.selectedCharacterIndex ?? null,
        mapFileName: s.mapFileName ?? null,
        mapTitle: s.mapTitle ?? null,
        sceneInteractionReady: s.sceneInteractionReady ?? null,
        playerObjectId: selfId,
        player: s.player ? { x: s.player.x, y: s.player.y } : null,
        level: (self && typeof self.level === "number") ? self.level : null,
        hp: s.playerHp ?? null,
        maxHp: s.playerMaxHp ?? null,
        mp: s.playerMp ?? null,
        maxMp: s.playerMaxMp ?? null,
        gold: typeof s.gold === "number" ? s.gold : null,
        inventoryCount: inv.length,
        inventory: inv.map(itemSig),
        equipmentCount: equip.length,
        equipment: equip.map(itemSig),
        questCount: quests.length,
        quests: quests.map((q) => ({
          id: q?.questId ?? q?.index ?? null,
          name: q?.name ?? q?.title ?? null,
          state: q?.state ?? q?.status ?? null,
        })),
        entityCount: ents.length,
        selfEntityCount: ents.filter((e) => e && String(e.objectId) === String(selfId)).length,
        duplicateObjectIds,
        characters: Array.isArray(s.characters)
          ? s.characters.map((c) => ({ name: c?.name ?? null, index: c?.index ?? null, level: c?.level ?? null }))
          : [],
      };
    })()
  `);
}

// Compare a baseline snapshot against an "after" snapshot and record one issue
// per dropped field. `phase` labels the transition (logout/login or reconnect).
function assertStatePersisted(ctx, stepId, phase, before, after) {
  const checks = [];
  const record = (key, ok, severity, summary, detail) => {
    checks.push({ key, ok });
    if (!ok) ctx.addIssue({ category: "persistence", severity, step: stepId, summary, detail });
  };

  const haveOurChar = after.characters.some((c) => c.name === characterName);
  record(
    "characterPresent",
    haveOurChar,
    "high",
    `character "${characterName}" lost after ${phase} (not in the character list / world)`,
    { characters: after.characters },
  );

  record(
    "playerPresent",
    Boolean(after.player) && Boolean(after.playerObjectId),
    "high",
    `player entity missing after ${phase}`,
    { playerObjectId: after.playerObjectId, player: after.player },
  );

  record(
    "map",
    before.mapFileName != null && before.mapFileName === after.mapFileName,
    "high",
    `map changed across ${phase}: "${before.mapFileName}" → "${after.mapFileName}"`,
    { before: before.mapFileName, after: after.mapFileName },
  );

  const posSame =
    before.player && after.player && before.player.x === after.player.x && before.player.y === after.player.y;
  record(
    "position",
    Boolean(posSame),
    "high",
    `position reset across ${phase}: ${posStr(before.player)} → ${posStr(after.player)}`,
    { before: before.player, after: after.player },
  );

  record(
    "inventory",
    sameItems(before.inventory, after.inventory),
    "high",
    `inventory dropped/changed across ${phase} (${before.inventoryCount} → ${after.inventoryCount} item(s))`,
    { beforeCount: before.inventoryCount, afterCount: after.inventoryCount, before: before.inventory, after: after.inventory },
  );

  record(
    "equipment",
    sameItems(before.equipment, after.equipment),
    "medium",
    `equipment dropped/changed across ${phase} (${before.equipmentCount} → ${after.equipmentCount} item(s))`,
    { beforeCount: before.equipmentCount, afterCount: after.equipmentCount, before: before.equipment, after: after.equipment },
  );

  record(
    "quests",
    sameQuests(before.quests, after.quests),
    "high",
    `quests dropped/changed across ${phase} (${before.questCount} → ${after.questCount} quest(s))`,
    { before: before.quests, after: after.quests },
  );

  record(
    "level",
    before.level === after.level,
    "medium",
    `level changed across ${phase}: ${before.level} → ${after.level}`,
    { before: before.level, after: after.level },
  );

  // Gold is only asserted when the server actually reported a value both times
  // (a null gold means "not pushed", not "lost").
  if (before.gold != null && after.gold != null) {
    record(
      "gold",
      before.gold === after.gold,
      "medium",
      `gold changed across ${phase}: ${before.gold} → ${after.gold}`,
      { before: before.gold, after: after.gold },
    );
  }

  return checks;
}

// ---------------------------------------------------------------------------
// Commands + small DOM helpers (mirrors qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

async function readPlayerPos(client) {
  return client.evaluate(
    `(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`,
  );
}

async function tilePoint(client, x, y) {
  const selector = `[aria-label="tile ${x}, ${y}"]`;
  return client.evaluate(`
    (() => {
      const tile = document.querySelector(${JSON.stringify(selector)});
      if (!tile) return null;
      const box = tile.getBoundingClientRect();
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    })()
  `);
}

async function clickTile(client, x, y, button = "left") {
  const point = await tilePoint(client, x, y);
  if (!point) return false;
  await client.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y, button: "none" });
  await client.send("Input.dispatchMouseEvent", {
    type: "mousePressed",
    x: point.x,
    y: point.y,
    button,
    buttons: button === "right" ? 2 : 1,
    clickCount: 1,
  });
  await client.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button, buttons: 0, clickCount: 1 });
  return true;
}

// Walk the player a few tiles from spawn so the persisted position is distinct
// from the default spawn (the strongest, most controllable persistence signal).
// Tries several directions; returns the achieved position (or the original).
async function moveToKnownTile(client) {
  const origin = await readPlayerPos(client);
  if (!origin) return { origin: null, moved: null, changed: false };
  const candidates = [
    { dx: 3, dy: 0 },
    { dx: 0, dy: 3 },
    { dx: -3, dy: 0 },
    { dx: 0, dy: -3 },
    { dx: 2, dy: 2 },
  ];
  for (const c of candidates) {
    const tx = origin.x + c.dx;
    const ty = origin.y + c.dy;
    if (!(await clickTile(client, tx, ty, "left"))) {
      await sendCommand(client, { type: "moveTo", x: tx, y: ty, mode: "run" });
    }
    const changed = await waitUntilSoft(
      client,
      `(() => { const p = window.__mir2Stage5?.state?.player; return p && (p.x !== ${origin.x} || p.y !== ${origin.y}); })()`,
      MOVE_SETTLE_MS,
    );
    if (changed) {
      await delay(600); // let the move finish settling on a final tile
      const moved = await readPlayerPos(client);
      return { origin, moved, changed: true, direction: c };
    }
  }
  return { origin, moved: origin, changed: false };
}

// ---------------------------------------------------------------------------
// Login / world-entry primitives.
// ---------------------------------------------------------------------------
async function waitForStageReady(client, timeoutMs = STAGE_READY_TIMEOUT_MS) {
  await waitUntil(
    client,
    "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
    "client stage ready",
    timeoutMs,
  );
}

// Register the account (when `createIfMissing`) and log in to the character-select
// screen against the REAL Rust gateway (:7111), which — unlike the permissive
// :7110 in-memory proxy — requires the account to actually exist.
//
// We drive the flow through the login-screen BUTTONS (not a raw send), because
// the client's reconnect machinery later reads the credentials it actually logged
// in with: submitLogin() sets activeReconnectAuthRef (apps/web/app/page.tsx:4629)
// and the account/character refs (1659/1661) feed captureGatewayReconnectSnapshot.
// A raw send() login authenticates fine but leaves those refs at their "demo"
// defaults, so the later resume would log in as the wrong account. Hence we fill
// the inputs FOR REAL (verified via DOM value + state.accountId) and click.
//
// Success = the client reaches "select"/"game" (LoginSuccess). A received `Login`
// packet means login FAILED (apps/web/app/page.tsx:6455); we surface its + the
// NewAccount result codes for a diagnosable failure.
async function loginToSelect(ctx, createIfMissing) {
  const client = ctx.client;
  await waitForStageReady(client);
  const screen0 = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen0 === "select" || screen0 === "game") return screen0;

  if (!(await waitForSelectorSoft(client, ".login-input.account", 8_000))) {
    throw new Error("login overlay never rendered (.login-input.account missing)");
  }
  // Fill the credentials FOR REAL and confirm they took — both the DOM value and
  // the client's accountId state (which the refs the reconnect snapshot reads sync
  // from). A swallowed fill here is exactly what made an earlier resume log in as
  // the default "demo" account.
  await fillInput(client, ".login-input.account", account);
  await fillInput(client, ".login-input.password", password);
  const filled = await waitUntilSoft(
    client,
    `(() => {
       const a = document.querySelector(".login-input.account");
       const p = document.querySelector(".login-input.password");
       return Boolean(a && p) && a.value === ${JSON.stringify(account)} && p.value === ${JSON.stringify(password)} &&
         window.__mir2Stage5?.state?.accountId === ${JSON.stringify(account)};
     })()`,
    6_000,
  );
  if (!filled) {
    throw new Error("login inputs did not accept the credentials (state.accountId never matched)");
  }
  await delay(350); // let accountIdRef/passwordRef sync from state (page.tsx:1659)

  const rxBeforeAuth = Date.now();
  if (createIfMissing) {
    // New Account button → createAccount() → connectGateway + newAccount(refs).
    await click(client, ".login-button.account button");
    if (!(await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000))) {
      throw new Error("gateway socket never opened from the login screen (is the gateway up on :7111?)");
    }
    // Wait for the created ack (NewAccount result 8); a non-8 (already exists,
    // rare with fresh names) is logged and accepted — login is the real gate.
    const created = await waitForReceivedPacket(
      client,
      ["NewAccount"],
      rxBeforeAuth,
      10_000,
      (o) => Number(o.result ?? o.payload?.result) === 8,
    );
    if (!created) {
      const anyAck = await waitForReceivedPacket(client, ["NewAccount"], rxBeforeAuth, 500);
      ctx.log(
        `  note: no NewAccount(result=8) ack${anyAck ? ` (result=${anyAck.result ?? anyAck.payload?.result})` : ""}; continuing to login`,
      );
    }
  }

  // OK button → submitLogin() → sets activeReconnectAuthRef + sends clientVersion
  // + login (opening the socket first if this is the re-login path).
  const rxBeforeLogin = Date.now();
  await click(client, ".login-button.ok button");
  const ok = await waitUntilSoft(
    client,
    "['select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
    ENTER_GAME_TIMEOUT_MS,
  );
  if (!ok) {
    const loginFail = loginResultFrom(framesSince(client, rxBeforeLogin));
    throw new Error(
      `login did not reach character-select (Login result=${loginFail ?? "none"}; screen=${await client.evaluate("window.__mir2Stage5?.state?.screen ?? null")})`,
    );
  }
  return client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
}

// From the character-select screen, ensure OUR named character exists, then
// StartGame into it. We never trust state.characters to decide existence (it can
// carry a phantom slot the server never returned — see qa-playthrough.mjs); the
// reliable signal is our named character appearing.
async function startGameAsOurCharacter(ctx, step, allowCreate) {
  const client = ctx.client;
  const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen === "game") return true;

  const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;

  if (allowCreate && !(await client.evaluate(haveOurChar))) {
    await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
    const created = await waitUntilSoft(client, haveOurChar, 15_000);
    if (!created) {
      ctx.addIssue({
        category: "flow",
        severity: "high",
        step: step.id,
        summary: "character creation did not produce a server-backed character",
        detail: { characterName, characters: (await readPersistenceState(client)).characters },
      });
      return false;
    }
  }

  const startIndex = await client.evaluate(
    `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
  );
  step.startIndex = startIndex;

  const wsBefore = Date.now();
  await sendCommand(client, { type: "startGame", characterIndex: startIndex });
  const entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", ENTER_GAME_TIMEOUT_MS);
  if (!entered) {
    const result = startGameResultFrom(framesSince(client, wsBefore));
    ctx.addIssue({
      category: "flow",
      severity: "high",
      step: step.id,
      summary:
        result === null
          ? "startGame sent but never entered the game (no StartGame reply)"
          : `startGame rejected by server (StartGame result=${result})`,
      detail: { startIndex, result },
    });
    return false;
  }
  return true;
}

// Wait for the scene to actually become interaction/asset ready after entering
// the world (so the snapshot reflects a fully-loaded world, not a loading frame).
async function waitForSceneReady(ctx, step, phase) {
  const ready = await waitUntilSoft(
    ctx.client,
    "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
    SCENE_READY_TIMEOUT_MS,
  );
  if (!ready) {
    ctx.addIssue({
      category: "flow",
      severity: "medium",
      step: step.id,
      summary: `scene never became interaction/asset ready (${phase})`,
      detail: null,
    });
  }
  await delay(800); // settle a couple frames
  return ready;
}

// ---------------------------------------------------------------------------
// The three QA steps.
// ---------------------------------------------------------------------------
async function runPersistenceQa(ctx) {
  const client = ctx.client;
  let baseline = null;

  // STEP 1 — fresh account + character, enter world, walk to a known tile, snapshot.
  await runStep(ctx, "create character + enter world + snapshot baseline", async (step) => {
    await navigate(client, RUN_URL);
    await loginToSelect(ctx, true);
    if (!(await startGameAsOurCharacter(ctx, step, true))) {
      throw new Error("could not enter the world to establish a baseline");
    }
    await waitForSceneReady(ctx, step, "enter world");

    const move = await moveToKnownTile(client);
    step.move = move;
    if (!move.changed) {
      ctx.addIssue({
        category: "flow",
        severity: "low",
        step: step.id,
        summary: "could not move off the spawn tile; baseline position is the spawn point",
        detail: move,
      });
    }

    baseline = await readPersistenceState(client);
    step.baseline = baseline;
    ctx.baseline = baseline;
    ctx.log(
      `  baseline: map=${baseline.mapFileName} pos=${posStr(baseline.player)} inv=${baseline.inventoryCount} equip=${baseline.equipmentCount} quests=${baseline.questCount} level=${baseline.level} gold=${baseline.gold} selfId=${baseline.playerObjectId}`,
    );
    if (!baseline.player || !baseline.mapFileName) {
      ctx.addIssue({
        category: "flow",
        severity: "high",
        step: step.id,
        summary: "baseline snapshot has no map/position; later persistence checks will be unreliable",
        detail: baseline,
      });
    }
  });

  if (!baseline) {
    ctx.log("\nAborting: no baseline established (cannot test persistence).");
    return;
  }

  // STEP 2 — LOG OUT, then LOG IN again (fresh page load, same account), re-enter
  // the same character, and assert the snapshot persisted.
  await runStep(ctx, "logout → login (same account) → assert state persisted", async (step) => {
    // (a) Log out the Crystal way — ClientPacket::LogOut runs
    // persist_active_character_save (apps/simulation/src/runtime/packets.rs:7304)
    // then returns the account's character list (LogOutSuccess).
    // This client de-authenticates fully on logout: the LogOutSuccess handler
    // sets screen → "login" (NOT "select") and repopulates the character list from
    // the freshly-persisted account store (apps/web/app/page.tsx:7113). So we wait
    // for "login" and confirm our character is in that post-logout list (= it was
    // persisted at logout, the persist_active_character_save round-trip).
    const wsBefore = Date.now();
    const sent = await sendCommand(client, { type: "logOut" });
    step.logOutSent = sent;
    const reachedLogin = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'login'", ENTER_GAME_TIMEOUT_MS);
    step.reachedLogin = reachedLogin;
    if (!reachedLogin) {
      ctx.addIssue({
        category: "persistence",
        severity: "high",
        step: step.id,
        summary: "logOut did not de-authenticate to the login screen (LogOutSuccess not applied)",
        detail: { screen: await client.evaluate("window.__mir2Stage5?.state?.screen ?? null"), logOutAck: logOutAckFrom(framesSince(client, wsBefore)) },
      });
    } else {
      const atLogout = await readPersistenceState(client);
      step.charactersAtLogout = atLogout.characters;
      const persistedInList = atLogout.characters.some((c) => c.name === characterName);
      if (!persistedInList) {
        ctx.addIssue({
          category: "persistence",
          severity: "high",
          step: step.id,
          summary: `character "${characterName}" missing from the post-logout character list (LogOutSuccess)`,
          detail: { characters: atLogout.characters },
        });
      }
    }

    // (b) Log in again with the same account. We re-login on the SAME socket (no
    // page reload / socket close) deliberately: logOut already cleared the client's
    // in-memory world to DEFAULT_WORLD_STATE (apps/web/app/page.tsx:7113), so the
    // re-login + StartGame below MUST reload the character from the persistent
    // store — a genuine persistence round-trip. (Closing the socket here would
    // instead trip the gateway's reconnect-grace session retention, which is what
    // step 3 exercises on purpose; keeping the socket open isolates persistence
    // from reconnect mechanics.)
    await loginToSelect(ctx, false); // account already exists; re-auth on the open socket
    const afterLoginSelect = await readPersistenceState(client);
    step.charactersAfterRelogin = afterLoginSelect.characters;
    if (!afterLoginSelect.characters.some((c) => c.name === characterName)) {
      ctx.addIssue({
        category: "persistence",
        severity: "high",
        step: step.id,
        summary: `character "${characterName}" did not survive re-login (absent from the reloaded character list)`,
        detail: { characters: afterLoginSelect.characters },
      });
    }

    // (c) Re-enter the SAME character and snapshot the in-world state.
    if (!(await startGameAsOurCharacter(ctx, step, false))) {
      throw new Error("could not re-enter the world after re-login");
    }
    await waitForSceneReady(ctx, step, "after re-login");
    await delay(600);

    const after = await readPersistenceState(client);
    step.after = after;
    step.checks = assertStatePersisted(ctx, step.id, "logout/login", baseline, after);
    ctx.afterLogin = after;
    ctx.log(
      `  after login: map=${after.mapFileName} pos=${posStr(after.player)} inv=${after.inventoryCount} equip=${after.equipmentCount} quests=${after.questCount} level=${after.level} gold=${after.gold} selfId=${after.playerObjectId}`,
    );
  });

  // STEP 3 — RECONNECT: a network blip, then assert the client resumes the SAME
  // session without bouncing to login or losing / duplicating state.
  await runStep(ctx, "reconnect blip → assert session resumes (no bounce, no dup)", async (step) => {
    const inGame = await client.evaluate("window.__mir2Stage5?.state?.screen === 'game'");
    if (!inGame) {
      ctx.addIssue({
        category: "reconnect",
        severity: "high",
        step: step.id,
        summary: "not in-game before the reconnect test (prior step failed to enter the world)",
        detail: null,
      });
      return;
    }
    const before = await readPersistenceState(client);
    step.before = before;

    // (a) Blip the network offline. emulateNetworkConditions tears down the
    // page's sockets; the client should detect the drop and schedule a resume.
    await client.send("Network.emulateNetworkConditions", {
      offline: true,
      latency: 0,
      downloadThroughput: 0,
      uploadThroughput: 0,
    });
    const offlineAt = Date.now();

    // Confirm the client noticed (reconnectStatus leaves "idle"). If Chrome held
    // the socket open under offline emulation, force the close as a fallback so
    // the reconnect path is reliably exercised.
    let leftIdle = await waitUntilSoft(
      client,
      `["scheduled", "connecting", "resuming"].includes(window.__mir2Stage5?.state?.reconnectStatus?.mode)`,
      3_000,
    );
    if (!leftIdle) {
      const forced = await client.evaluate("window.__mir2Stage5?.closeGatewayForReconnectSmoke?.() === true");
      step.forcedClose = forced;
      leftIdle = await waitUntilSoft(
        client,
        `["scheduled", "connecting", "resuming"].includes(window.__mir2Stage5?.state?.reconnectStatus?.mode)`,
        5_000,
      );
    }
    step.sawReconnectStatus = leftIdle;
    step.duringReconnectMode = await client.evaluate("window.__mir2Stage5?.state?.reconnectStatus?.mode ?? null");
    if (!leftIdle) {
      ctx.addIssue({
        category: "reconnect",
        severity: "high",
        step: step.id,
        summary: "network blip did not trigger the reconnect path (reconnectStatus stayed idle)",
        detail: { duringReconnectMode: step.duringReconnectMode },
      });
    }

    // (b) Hold the blip, watching that we DON'T bounce to login/select while
    // offline (a real resume keeps screen === "game" throughout).
    let bouncedScreen = null;
    while (Date.now() - offlineAt < offlineMs) {
      const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
      if (screen === "login" || screen === "select") {
        bouncedScreen = screen;
        break;
      }
      await delay(250);
    }
    step.bouncedScreenWhileOffline = bouncedScreen;

    // (c) Restore connectivity and wait for the session to recover.
    await client.send("Network.emulateNetworkConditions", {
      offline: false,
      latency: 0,
      downloadThroughput: 0,
      uploadThroughput: 0,
    });
    step.restoredAt = Date.now();

    // While recovering, keep sampling the screen so a bounce to login/select at
    // ANY point during the window is caught (the resume must stay in "game").
    const recovered = await waitUntilWatching(
      client,
      `(() => { const s = window.__mir2Stage5?.state; return s?.screen === "game" && s?.wsState === "open" && s?.reconnectStatus?.mode === "idle" && Boolean(s?.player); })()`,
      `(() => window.__mir2Stage5?.state?.screen ?? null)()`,
      (screen) => {
        if ((screen === "login" || screen === "select") && !bouncedScreen) bouncedScreen = screen;
      },
      RECONNECT_RECOVER_TIMEOUT_MS,
    );
    step.recovered = recovered;
    step.bouncedScreen = bouncedScreen;

    if (bouncedScreen) {
      ctx.addIssue({
        category: "reconnect",
        severity: "high",
        step: step.id,
        summary: `reconnect bounced the client to the "${bouncedScreen}" screen instead of resuming in-game`,
        detail: { bouncedScreen },
      });
    }
    if (!recovered) {
      ctx.addIssue({
        category: "reconnect",
        severity: "high",
        step: step.id,
        summary: "client did not resume the game after the network blip (never returned to screen=game / ws=open / reconnect idle)",
        detail: {
          screen: await client.evaluate("window.__mir2Stage5?.state?.screen ?? null"),
          wsState: await client.evaluate("window.__mir2Stage5?.state?.wsState ?? null"),
          reconnectMode: await client.evaluate("window.__mir2Stage5?.state?.reconnectStatus?.mode ?? null"),
        },
      });
      return;
    }

    await delay(800); // let the resumed world snapshot settle
    const after = await readPersistenceState(client);
    step.after = after;
    ctx.afterReconnect = after;
    ctx.log(
      `  after reconnect: screen=${after.screen} ws=${after.wsState} mode=${after.reconnectMode} pos=${posStr(after.player)} selfId=${after.playerObjectId} entities=${after.entityCount}`,
    );

    // (d) Same session: same playerObjectId, no duplicate self entity, state kept.
    if (before.playerObjectId && after.playerObjectId && before.playerObjectId !== after.playerObjectId) {
      ctx.addIssue({
        category: "reconnect",
        severity: "high",
        step: step.id,
        summary: `playerObjectId changed across the reconnect (${before.playerObjectId} → ${after.playerObjectId}) — a NEW session, not a resume`,
        detail: { before: before.playerObjectId, after: after.playerObjectId },
      });
    }
    if (after.selfEntityCount > 1 || after.duplicateObjectIds.length > 0) {
      ctx.addIssue({
        category: "reconnect",
        severity: "high",
        step: step.id,
        summary: `duplicate entities after resume (self×${after.selfEntityCount}; ${after.duplicateObjectIds.length} duplicated objectId(s))`,
        detail: { selfEntityCount: after.selfEntityCount, duplicateObjectIds: after.duplicateObjectIds },
      });
    }
    // The resumed world must still hold the same place/state.
    step.checks = assertStatePersisted(ctx, step.id, "reconnect", before, after);
  });
}

// ---------------------------------------------------------------------------
// Cross-cutting collectors (network / console), mirroring qa-playthrough.mjs.
// ---------------------------------------------------------------------------
function collectConsoleIssues(ctx) {
  const errors = ctx.client.consoleErrors.filter(isCriticalConsoleError);
  if (!errors.length) return;
  const seen = new Map();
  for (const e of errors) {
    const key = (e.text ?? "").slice(0, 160);
    if (!seen.has(key)) seen.set(key, { ...e, count: 0 });
    seen.get(key).count += 1;
  }
  for (const e of seen.values()) {
    ctx.addIssue({
      category: "console",
      severity: "low",
      step: null,
      summary: `console ${e.type}: ${(e.text ?? "").slice(0, 140)}`,
      detail: { count: e.count, source: e.source },
    });
  }
}

// ---------------------------------------------------------------------------
// Reporting.
// ---------------------------------------------------------------------------
async function writeReport(ctx) {
  const bySeverity = { high: 0, medium: 0, low: 0 };
  const byCategory = {};
  for (const issue of ctx.issues) {
    bySeverity[issue.severity] = (bySeverity[issue.severity] ?? 0) + 1;
    byCategory[issue.category] = (byCategory[issue.category] ?? 0) + 1;
  }
  const summary = {
    runId,
    baseUrl: baseUrlRaw,
    gatewayWsUrl,
    account,
    characterName,
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    steps: ctx.steps.length,
    stepsOk: ctx.steps.filter((s) => s.ok).length,
    issueCount: ctx.issues.length,
    bySeverity,
    byCategory,
    passed: bySeverity.high === 0,
  };

  const report = {
    summary,
    issues: ctx.issues,
    baseline: ctx.baseline ?? null,
    afterLogin: ctx.afterLogin ?? null,
    afterReconnect: ctx.afterReconnect ?? null,
    steps: ctx.steps,
  };
  await fs.writeFile(path.join(outputDir, "summary.json"), `${JSON.stringify(summary, null, 2)}\n`);
  await fs.writeFile(path.join(outputDir, "report.json"), `${JSON.stringify(report, null, 2)}\n`);
  await fs.writeFile(path.join(outputDir, "latest-persistence.json"), `${JSON.stringify(report, null, 2)}\n`);
  await fs.writeFile(
    path.join(outputDir, "console.json"),
    `${JSON.stringify({ errors: ctx.client.consoleErrors, network: ctx.client.networkFailures }, null, 2)}\n`,
  );
  await fs.writeFile(
    path.join(outputDir, "ws-timeline.json"),
    `${JSON.stringify({ sent: ctx.client.webSocketFramesSent.slice(-200), received: ctx.client.webSocketFramesReceived.slice(-200) }, null, 2)}\n`,
  );

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  const md = [];
  md.push(`# Persistence + reconnect QA report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrlRaw}\` → gateway \`${gatewayWsUrl}\``);
  md.push(`- Account \`${account}\`  ·  character \`${characterName}\``);
  md.push(`- Result: **${summary.passed ? "PASS" : "FAIL"}** — steps ${summary.stepsOk}/${summary.steps} ok  ·  issues ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push("");
  md.push("## State across transitions");
  md.push("");
  md.push("| Field | Baseline (step 1) | After logout/login (step 2) | After reconnect (step 3) |");
  md.push("|---|---|---|---|");
  const b = ctx.baseline ?? {};
  const l = ctx.afterLogin ?? {};
  const r = ctx.afterReconnect ?? {};
  const rowFor = (label, fn) => md.push(`| ${label} | ${fn(b)} | ${fn(l)} | ${fn(r)} |`);
  rowFor("map", (x) => x.mapFileName ?? "—");
  rowFor("position", (x) => posStr(x.player));
  rowFor("playerObjectId", (x) => x.playerObjectId ?? "—");
  rowFor("level", (x) => fmt(x.level));
  rowFor("inventory", (x) => fmt(x.inventoryCount));
  rowFor("equipment", (x) => fmt(x.equipmentCount));
  rowFor("quests", (x) => fmt(x.questCount));
  rowFor("gold", (x) => fmt(x.gold));
  rowFor("self entities", (x) => fmt(x.selfEntityCount));
  md.push("");
  md.push("## Issues (by severity)");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues detected — state survived logout/login and the reconnect blip._");
  } else {
    md.push("| # | Severity | Category | Step | Summary |");
    md.push("|---|---|---|---|---|");
    for (const i of sortedIssues) {
      md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.step ?? "—"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    }
  }
  md.push("");
  md.push("## Steps");
  md.push("");
  for (const s of ctx.steps) {
    md.push(`### Step ${s.id} — ${s.title} ${s.ok ? "✅" : "❌"}  (${s.durationMs}ms)`);
    if (s.error) md.push(`- error: \`${s.error.slice(0, 300)}\``);
    if (s.move) md.push(`- move: ${JSON.stringify(s.move)}`);
    if (s.checks) md.push(`- checks: ${s.checks.map((c) => `${c.key}${c.ok ? "✓" : "✗"}`).join(" ")}`);
    if (s.sawReconnectStatus !== undefined) md.push(`- reconnect: sawStatus=${s.sawReconnectStatus} mode=${s.duringReconnectMode} forcedClose=${s.forcedClose ?? false} recovered=${s.recovered} bounced=${s.bouncedScreen ?? "no"}`);
    if (s.frame) md.push(`- frame: [\`${s.frame}\`](${s.frame})`);
    md.push("");
  }
  md.push("## Detailed issue evidence");
  md.push("");
  for (const i of sortedIssues) {
    md.push(`### ${i.id} — [${i.severity}/${i.category}] ${i.summary}`);
    if (i.step) md.push(`- step: ${i.step}`);
    if (i.detail !== undefined) md.push("```json\n" + JSON.stringify(i.detail, null, 2) + "\n```");
    md.push("");
  }
  md.push("## Reproduce");
  md.push("");
  md.push("```bash");
  md.push(`cd apps/web && node ./scripts/qa-persistence.mjs --headed --baseUrl ${baseUrlRaw} --gatewayWs ${gatewayWsUrl}`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-persistence-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      ...(disableGpu ? ["--disable-gpu"] : ["--ignore-gpu-blocklist", "--enable-webgl"]),
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      "--no-proxy-server",
      "--proxy-bypass-list=*",
      "--no-first-run",
      "--no-default-browser-check",
      `--window-size=${VIEWPORT.width},${VIEWPORT.height}`,
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome();
  return chrome;
}

async function createPageTarget() {
  const response = await fetch(`http://127.0.0.1:${debugPort}/json/new?${encodeURIComponent(RUN_URL)}`, { method: "PUT" });
  if (!response.ok) throw new Error(`Chrome target creation failed: ${response.status}`);
  const target = await response.json();
  targetAlreadyNavigated = true;
  await delay(2500);
  return target.webSocketDebuggerUrl;
}

async function waitForChrome() {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${debugPort}/json/version`);
      if (response.ok) return;
    } catch {
      await delay(100);
    }
  }
  throw new Error("Timed out waiting for Chrome debug endpoint.");
}

async function setViewport(client, viewport) {
  await client.send("Emulation.setDeviceMetricsOverride", viewport);
  await client.send("Emulation.setVisibleSize", { width: viewport.width, height: viewport.height });
}

async function navigate(client, url) {
  let lastError;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await client.send("Page.navigate", { url });
      await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 20_000);
      const currentUrl = await client.evaluate("window.location.href");
      if (typeof currentUrl !== "string" || !currentUrl.startsWith("chrome-error://")) {
        targetAlreadyNavigated = true;
        return;
      }
      lastError = new Error(`Chrome landed on ${currentUrl}`);
    } catch (error) {
      lastError = error;
    }
    await delay(500);
  }
  throw lastError ?? new Error("Page navigation failed.");
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
  if (!ok) throw new Error(`Could not fill ${selector}`);
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
  if (!ok) throw new Error(`Could not click ${selector}`);
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

async function waitUntil(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return;
    await delay(120);
  }
  const debug = await client
    .evaluate(
      `(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, wsState: window.__mir2Stage5?.state?.wsState ?? null, reconnectMode: window.__mir2Stage5?.state?.reconnectStatus?.mode ?? null, body: document.body?.innerText?.slice(0, 300) ?? "" }))()`,
    )
    .catch((e) => ({ debugError: String(e) }));
  throw new Error(`Timed out waiting for ${label}; debug=${JSON.stringify(debug)}`);
}

async function waitUntilSoft(client, expression, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return true;
    await delay(120);
  }
  return false;
}

async function waitForSelectorSoft(client, selector, timeoutMs) {
  return waitUntilSoft(client, `document.querySelector(${JSON.stringify(selector)}) != null`, timeoutMs);
}

// Like waitUntilSoft but also samples a `watchExpr` each poll and feeds its value
// to `onSample` — used to catch a transient screen bounce during reconnect.
async function waitUntilWatching(client, doneExpr, watchExpr, onSample, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const sample = await client.evaluate(watchExpr).catch(() => null);
    if (sample !== null && sample !== undefined) onSample(sample);
    const done = await client.evaluate(`Boolean(${doneExpr})`).catch(() => null);
    if (done) return true;
    await delay(200);
  }
  return false;
}

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve)).catch(() => undefined);
  if (chrome.userDataDir) {
    await fs.rm(chrome.userDataDir, { recursive: true, force: true }).catch(() => undefined);
  }
}

// ---------------------------------------------------------------------------
// Small utilities.
// ---------------------------------------------------------------------------
function buildRunUrl(rawBaseUrl, wsUrl) {
  const url = new URL(rawBaseUrl);
  if (!url.searchParams.has("gatewayWs")) url.searchParams.set("gatewayWs", wsUrl);
  return url.toString();
}

function posStr(p) {
  return p && typeof p.x === "number" ? `(${p.x},${p.y})` : "—";
}

// Item equality by a stable per-slot signature (order-insensitive).
function sameItems(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b)) return false;
  if (a.length !== b.length) return false;
  const key = (it) => JSON.stringify([it.slot, it.name, it.itemIndex, it.count]);
  const sa = a.map(key).sort();
  const sb = b.map(key).sort();
  return sa.every((v, i) => v === sb[i]);
}

function sameQuests(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b)) return false;
  if (a.length !== b.length) return false;
  const key = (q) => JSON.stringify([q.id, q.name]);
  const sa = a.map(key).sort();
  const sb = b.map(key).sort();
  return sa.every((v, i) => v === sb[i]);
}

// Received frames captured at/after `sinceTime`. Timestamp-keyed (not index-keyed)
// because the frame buffer is capped — indices shift once it fills (mostly with
// Turbopack HMR frames), but timestamps stay valid.
function framesSince(client, sinceTime) {
  return client.webSocketFramesReceived.filter((f) => (f.at ?? 0) >= sinceTime);
}

// Poll the received frames for a gateway packet with one of `names` (optionally
// also satisfying `predicate`) captured at/after `sinceTime`. Gateway frames are
// JSON with a `packet` field; Turbopack HMR frames have none, so it is unambiguous.
async function waitForReceivedPacket(client, names, sinceTime, timeoutMs, predicate = null) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    for (const f of framesSince(client, sinceTime)) {
      try {
        const o = JSON.parse(f.payloadData);
        if (o && names.includes(o.packet) && (!predicate || predicate(o))) return o;
      } catch {
        /* non-JSON (HMR) frame */
      }
    }
    await delay(150);
  }
  return null;
}

// Most recent `Login` (failure) packet's result code in a frame slice, or null.
function loginResultFrom(frames) {
  for (let i = frames.length - 1; i >= 0; i -= 1) {
    try {
      const o = JSON.parse(frames[i].payloadData);
      if (o && o.packet === "Login") return o.result ?? o.payload?.result ?? "unknown";
    } catch {
      /* non-JSON frame */
    }
  }
  return null;
}

// Scan WS frames for the StartGame reply result code (null if none seen).
function startGameResultFrom(frames) {
  for (let i = frames.length - 1; i >= 0; i -= 1) {
    try {
      const o = JSON.parse(frames[i].payloadData);
      if (o && o.packet === "StartGame") return o.payload?.result ?? null;
    } catch {
      /* non-JSON frame */
    }
  }
  return null;
}

function logOutAckFrom(frames) {
  for (let i = frames.length - 1; i >= 0; i -= 1) {
    try {
      const o = JSON.parse(frames[i].payloadData);
      if (o && (o.packet === "LogOutSuccess" || o.packet === "LogOutFailed")) return o.packet;
    } catch {
      /* non-JSON frame */
    }
  }
  return null;
}

function isCriticalConsoleError(error) {
  const text = String(error?.text ?? "");
  if (error?.source === "network" && text.includes("net::ERR_FAILED")) return false;
  if (text.includes("favicon")) return false;
  // Dropping the gateway socket on purpose (the reconnect blip) produces benign
  // socket/network errors — don't surface them as defects.
  if (/ERR_INTERNET_DISCONNECTED|ERR_NETWORK_CHANGED|WebSocket.*(clos|fail)|WebSocket connection to|Failed to fetch|NetworkError/i.test(text)) {
    return false;
  }
  return true;
}

function slug(text) {
  return String(text).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40);
}

function fmt(n) {
  return n === null || n === undefined ? "—" : n;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) continue;
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
  if (value === undefined || value === null || value === "") return fallback;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function booleanArg(value, fallback) {
  if (value === undefined || value === null || value === "") return fallback;
  if (typeof value === "boolean") return value;
  return ["1", "true", "yes", "on"].includes(String(value).toLowerCase());
}

function defaultName(prefix) {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `${prefix}${suffix}`.slice(0, 12).toUpperCase();
}

function findChromePath() {
  const candidates = [
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  ];
  return candidates.find((candidate) => existsSync(candidate)) ?? null;
}

function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
async function main() {
  await fs.mkdir(framesDir, { recursive: true });
  console.log(`qa-persistence: target=${baseUrlRaw} gateway=${gatewayWsUrl} account=${account} char=${characterName} headed=${headed}`);
  console.log(`qa-persistence: output=${outputDir}`);

  const chrome = await launchChrome();
  let client;
  let ctx;
  try {
    const wsUrl = await createPageTarget();
    client = new CdpClient(wsUrl);
    await client.connect();
    await client.send("Runtime.enable");
    await client.send("Log.enable");
    await client.send("Network.enable");
    await client.send("Page.enable");
    await client.send("Page.bringToFront");
    await setViewport(client, VIEWPORT);

    ctx = createContext(client);
    ctx.startedAt = Date.now();

    await runPersistenceQa(ctx);

    collectConsoleIssues(ctx);
    await writeReport(ctx);

    const s = ctx.issues.reduce((acc, i) => ((acc[i.severity] = (acc[i.severity] ?? 0) + 1), acc), {});
    const passed = (s.high ?? 0) === 0;
    console.log(
      `\nqa-persistence ${passed ? "PASS" : "FAIL"}: ${ctx.steps.filter((x) => x.ok).length}/${ctx.steps.length} steps ok, ${ctx.issues.length} issues (high ${s.high ?? 0}, medium ${s.medium ?? 0}, low ${s.low ?? 0}).`,
    );
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
    if (!passed) process.exitCode = 1;
  } catch (error) {
    console.error("qa-persistence fatal:", error);
    if (ctx) {
      try {
        ctx.addIssue({ category: "flow", severity: "high", step: null, summary: "fatal harness error", detail: String(error?.message ?? error) });
        await writeReport(ctx);
      } catch {
        /* ignore */
      }
    }
    process.exitCode = 1;
  } finally {
    client?.close();
    await stopChrome(chrome).catch(() => {});
  }
}

main();
