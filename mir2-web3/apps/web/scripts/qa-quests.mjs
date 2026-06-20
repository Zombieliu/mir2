// qa-quests.mjs — AI-driven "real player" walkthrough of the FULL quest arc.
//
// Drives the REAL web client over Chrome DevTools Protocol through the whole
// quest lifecycle that nothing else exercises end-to-end today:
//
//   register -> login -> create character -> enter world -> reach the quest hub
//   -> ACCEPT a quest (NPC dialog link + the wired AcceptQuest command)
//   -> confirm it enters the quest log (+ the standalone quest-log window renders)
//   -> SATISFY / short-circuit the objective (the canonical starter quest is a
//      no-kill "talk/delivery" quest that goes straight to readyToTurnIn)
//   -> SHARE it (ShareQuest) and ABANDON a quest (AbandonQuest)
//   -> FINISH it (FinishQuest) at the turn-in NPC
//   -> verify the REWARD lands (gold / item, cross-checked against the WS truth
//      layer + the post-finish world snapshot) and the quest LEAVES the active log
//
// and records EVERY problem it sees into a structured report under
// docs/generated/player-qa/.
//
// Why a browser (not a protocol bot): the quest arc threads five layers
// (ClientPacket -> simulation -> gateway server_packet_to_event -> page.tsx case
// handlers -> stage5 window adapters -> the quest-log window). Only a real client
// exercises the page.tsx ChangeQuest/CompleteQuest/NewQuestInfo handlers, the
// questLog snapshot merge, and the QuestLogWindow render — a bare WS bot would
// not.
//
// HOW THE ARC IS DRIVEN (cited to the source):
//   * Accept  — the real-player path is the NPC dialog link `@quest:accept:<id>`
//     (apps/simulation/src/runtime/quests.rs:1232) selected via
//     send({type:"selectNpcDialog", target}) (apps/web/app/page.tsx:11219). The
//     deterministic spine is the wired command send({type:"acceptQuest",
//     npcIndex, questIndex}) -> BrowserCommand::AcceptQuest -> ClientPacket::
//     AcceptQuest (apps/gateway/src/web.rs:1077, :3175) ->
//     stage5_accept_quest_packet (apps/simulation/src/runtime/packets.rs:2900).
//   * Finish  — NPC link `@quest:finish:<id>` (quests.rs:1252) and/or the wired
//     command send({type:"finishQuest", questIndex}) -> ClientPacket::FinishQuest
//     -> stage5_finish_quest_packet (packets.rs:2937), which grants gold/credit/
//     exp/items (complete_crystal_quest, quests.rs:746) and emits CompleteQuest.
//   * Abandon — send({type:"abandonQuest", questIndex}) (page.tsx:5764).
//   * Share   — send({type:"shareQuest", questIndex}) (page.tsx:5760).
//   Quest commands are NOT in should_send_world_snapshot_for_action's exclusion
//   list (apps/gateway/src/web.rs), so the gateway pushes a fresh world snapshot
//   (with the updated gold + inventory) right after accept/finish — that is how
//   the reward becomes visible client-side in state.gold / state.inventoryItems /
//   state.beltItems. NOTE: Crystal auto-routes Potion/Scroll/Amulet rewards into
//   the belt region of the inventory (AddItem, HumanObject.cs:1596); the web port
//   surfaces that as the separate state.beltItems, so reward verification here
//   counts the player's whole carried possession (bag + belt), not just the bag.
//
// ENV — needs a COMPLETABLE quest reachable on the local stack:
//   * Crystal quest #1 (the spawn-town "assistant" -> "craft lady" delivery
//     quest) is a no-kill quest proven completable by the simulation test
//     `original_crystal_loaded_npc_links_accept_and_finish_quest`
//     (apps/simulation/src/runtime/tests.rs:27629): accept at the NPC near
//     crystal:0:283:606, turn in at the NPC near (293,619). Accepting it sends
//     the quest straight to readyToTurnIn (begin_quest, quests.rs:646), so the
//     objective is satisfied without killing anything — the harness short-circuits
//     "talk/collect/kill" via this talk quest, no GM needed.
//   * GM-gated alternative (documented, NOT relied on): @SETQUEST <id> 1 marks a
//     quest complete (gm_set_quest, apps/simulation/src/runtime/gm_commands.rs:1407)
//     but is is_gm_or_test-gated and a no-op on the local stack (no
//     MIR2_GM_PASSWORD / test_server=false), so kill/collect objectives on quests
//     OTHER than the no-kill starter cannot be short-circuited here.
//   * Abandon is demonstrated on the guide quest (id 1001,
//     crystal_compat.rs:1) which accepts to inProgress from anywhere
//     (npcIndex:0) and is then abandoned, leaving quest #1's finish arc untouched.
//   * Share without a party returns a "quest not shared" system message
//     (stage5_share_quest_packet, packets.rs:2992) — the expected env-limited
//     outcome (no second client to invite); recorded, not failed.
//
// This file deliberately reuses the patterns proven in qa-playthrough.mjs /
// capture-web-movement-jitter.mjs (CDP client, chrome launch, login flow, tile
// clicking) so it stays consistent with the house QA harnesses, zero new deps.
//
// Usage:
//   node ./scripts/qa-quests.mjs --headed [--baseUrl http://127.0.0.1:3001]
//        [--account NAME --password PW] [--createAccount true]
//        [--questId 1] [--abandonQuestId 1001] [--questHub crystal:0:283:606]
//        [--runId my-run]
//
// Each beat is best-effort: a failing beat is recorded as an issue and the loop
// keeps going so it captures as much of the arc as possible.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3001";
// Load with debug hooks enabled (harmless; keeps parity with qa-playthrough).
const RUN_URL = `${baseUrl}${baseUrl.includes("?") ? "&" : "?"}mir2Debug=1`;
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultAccountName();
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultCharacterName();
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9600 + (process.pid % 300));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "player-qa", `quests-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

// The canonical completable quest + where to find its NPCs (see ENV above).
const QUEST_ID = numberArg(args.questId, 1); // Crystal quest #1 — no-kill delivery.
const ABANDON_QUEST_ID = numberArg(args.abandonQuestId, 1001); // guide quest.
const QUEST_HUB = args.questHub ?? "crystal:0:283:606"; // assistant NPC area, map 0.
const FINISH_HUB = args.finishHub ?? "crystal:0:293:619"; // craft-lady NPC area.

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const NPC_DIALOG_TIMEOUT_MS = numberArg(args.npcDialogTimeoutMs, 14_000);
const QUEST_TIMEOUT_MS = numberArg(args.questTimeoutMs, 10_000);
const SNAPSHOT_SETTLE_MS = numberArg(args.snapshotSettleMs, 1500); // let post-action snapshot land
const BLACK_LUMA_MEAN = 8;
const FLAT_LUMA_VARIANCE = 6;

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from qa-playthrough.mjs) — captures console, network
// failures, and WebSocket frames (the server's authoritative truth layer).
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
        this.networkFailures.push({ url, status: r.status, kind: classifyAssetUrl(url), at: Date.now() });
      }
    } else if (m === "Network.loadingFailed") {
      const url = this.requestUrlById.get(p.requestId) ?? "(unknown)";
      if (!String(url).includes("favicon")) {
        this.networkFailures.push({ url, status: 0, errorText: p.errorText ?? "", kind: classifyAssetUrl(url), at: Date.now() });
      }
    } else if (m === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({ payloadData: p.response?.payloadData, at: Date.now() });
      // Keep a generous rolling window: the quest arc scans frames by TIMESTAMP
      // (see wsPacketsSince), so the buffer only needs to outlive one beat's
      // worth of frames — but a large cap also keeps the saved ws-timeline useful.
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-4000);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-4000);
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
  const beats = [];
  return {
    client,
    framesDir,
    issues,
    beats,
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

async function runBeat(ctx, title, fn) {
  ctx.seq += 1;
  const id = ctx.seq;
  const beat = { id, title, startedAt: Date.now(), ok: true };
  ctx.log(`\n▶ beat ${id}: ${title}`);
  try {
    await fn(beat);
  } catch (err) {
    beat.ok = false;
    beat.error = String(err?.message ?? err);
    ctx.addIssue({
      category: "flow",
      severity: "high",
      beat: id,
      summary: `beat blocked: ${title}`,
      detail: beat.error,
    });
    ctx.log(`  ✖ ${beat.error.slice(0, 300)}`);
  }
  const frameName = `${String(id).padStart(2, "0")}-${slug(title)}.png`;
  try {
    await captureFrame(ctx.client, path.join(framesDir, frameName));
    beat.frame = `frames/${frameName}`;
  } catch {
    /* screenshot is best-effort */
  }
  try {
    beat.state = await readGameState(ctx.client);
  } catch {
    /* state read is best-effort */
  }
  beat.endedAt = Date.now();
  beat.durationMs = beat.endedAt - beat.startedAt;
  ctx.beats.push(beat);
  return beat;
}

// ---------------------------------------------------------------------------
// In-page state reader — compact, quest-focused view of the client state.
// (state.questLog / state.gold / state.inventoryItems / state.activeNpcDialog
// are all exposed via window.__mir2Stage5.state, apps/web/app/page.tsx:4347+.)
// ---------------------------------------------------------------------------
async function readGameState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const entities = Array.isArray(state.entities) ? state.entities : [];
      const npcs = entities.filter((e) => e && e.kind === "npc");
      const questLog = Array.isArray(state.questLog) ? state.questLog : [];
      const inv = Array.isArray(state.inventoryItems) ? state.inventoryItems : [];
      const belt = Array.isArray(state.beltItems) ? state.beltItems : [];
      const dialog = state.activeNpcDialog ?? null;
      const bodyText = document.body?.innerText ?? "";
      let invQty = 0;
      for (const it of inv) invQty += Number(it?.quantity ?? it?.count ?? 1) || 0;
      let beltQty = 0;
      for (const it of belt) beltQty += Number(it?.quantity ?? it?.count ?? 1) || 0;
      const questWindow = document.querySelector("[data-quest-stage-filter]");
      return {
        capturedAt: Date.now(),
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        sceneInteractionReady: state.sceneInteractionReady ?? null,
        sceneAssetReadiness: state.sceneAssetReadiness ?? null,
        player: state.player ? { x: state.player.x, y: state.player.y, direction: state.player.direction ?? null } : null,
        playerObjectId: state.playerObjectId ?? null,
        gold: typeof state.gold === "number" ? state.gold : null,
        credit: typeof state.credit === "number" ? state.credit : null,
        entityCount: entities.length,
        npcCount: npcs.length,
        npcs: npcs.slice(0, 40).map((e) => ({ objectId: e.objectId, name: e.name ?? null, x: e.x, y: e.y })),
        inventoryCount: inv.length,
        inventoryQty: invQty,
        beltCount: belt.length,
        beltQty: beltQty,
        questLogCount: questLog.length,
        questLog: questLog.slice(0, 40).map((q) => ({
          questId: q?.questId ?? null,
          title: q?.title ?? null,
          stage: q?.stage ?? null,
          current: q?.current ?? null,
          required: q?.required ?? null,
          rewardPreview: q?.rewardPreview ?? null,
        })),
        activeNpcDialog: dialog
          ? {
              npcObjectId: dialog.npcObjectId ?? null,
              npcName: dialog.npcName ?? null,
              title: dialog.title ?? null,
              links: Array.isArray(dialog.links)
                ? dialog.links.map((l) => ({ text: l?.text ?? null, target: l?.target ?? null }))
                : [],
            }
          : null,
        npcDialogDom: Boolean(document.querySelector(".npc-dialog-panel")),
        questWindowOpen: Boolean(questWindow),
        questWindowRows: document.querySelectorAll("[data-quest-id]").length,
        loadingMapVisible: bodyText.includes("Loading map"),
      };
    })()
  `);
}

// ---------------------------------------------------------------------------
// Render health — light canvas-black check (Bevy is the renderer). The quest
// arc is mostly DOM/state, but a black canvas at the hub is still worth flagging.
// ---------------------------------------------------------------------------
async function canvasRect(client) {
  return client.evaluate(`
    (() => {
      const c = document.querySelector("#mir2-web3-canvas") || document.querySelector("canvas");
      if (!c) return null;
      const b = c.getBoundingClientRect();
      return { x: Math.max(0, b.left), y: Math.max(0, b.top), width: b.width, height: b.height };
    })()
  `);
}

async function captureLumaGrid(client, grid = 8) {
  const rect = await canvasRect(client);
  const clip = rect && rect.width > 4 && rect.height > 4 ? { ...rect, scale: 1 } : undefined;
  const shot = await client.send("Page.captureScreenshot", {
    format: "jpeg",
    quality: 45,
    captureBeyondViewport: false,
    ...(clip ? { clip } : {}),
  });
  return client.evaluate(`
    (async () => {
      const img = new Image();
      img.src = "data:image/jpeg;base64,${shot.data}";
      try { await img.decode(); } catch { return null; }
      const G = ${grid};
      const cv = document.createElement("canvas");
      cv.width = G; cv.height = G;
      const ctx = cv.getContext("2d");
      ctx.drawImage(img, 0, 0, G, G);
      const d = ctx.getImageData(0, 0, G, G).data;
      const lumas = [];
      for (let i = 0; i < d.length; i += 4) lumas.push(0.299 * d[i] + 0.587 * d[i + 1] + 0.114 * d[i + 2]);
      return lumas;
    })()
  `);
}

function lumaStats(lumas) {
  if (!Array.isArray(lumas) || !lumas.length) return { mean: null, variance: null };
  const mean = lumas.reduce((a, b) => a + b, 0) / lumas.length;
  const variance = lumas.reduce((a, b) => a + (b - mean) * (b - mean), 0) / lumas.length;
  return { mean, variance };
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

async function assessRenderHealth(ctx, beatId, phase) {
  const lumas = await captureLumaGrid(ctx.client).catch(() => null);
  const { mean, variance } = lumaStats(lumas);
  const state = await readGameState(ctx.client).catch(() => ({}));
  if (mean !== null && mean < BLACK_LUMA_MEAN) {
    ctx.addIssue({
      category: "render",
      severity: "high",
      beat: beatId,
      summary: `canvas is effectively black during ${phase} (mean luma ${mean.toFixed(1)})`,
      detail: { mean, variance, mapFileName: state.mapFileName, sceneAssetReadiness: state.sceneAssetReadiness },
    });
  } else if (variance !== null && variance < FLAT_LUMA_VARIANCE) {
    ctx.addIssue({
      category: "render",
      severity: "low",
      beat: beatId,
      summary: `canvas is near-uniform / blank during ${phase} (variance ${variance.toFixed(1)})`,
      detail: { mean, variance, mapFileName: state.mapFileName },
    });
  }
  if (state.loadingMapVisible) {
    ctx.addIssue({
      category: "render",
      severity: "medium",
      beat: beatId,
      summary: `"Loading map…" overlay still visible during ${phase}`,
      detail: { mapFileName: state.mapFileName },
    });
  }
  return { mean, variance };
}

// ---------------------------------------------------------------------------
// Command / DOM primitives (adapted from qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
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

// Forced map transfer to exact coords; soft (no exact-position requirement) so a
// hub that drops us at the map's default entry point still proceeds.
async function transferTo(client, key) {
  const [, map] = key.split(":");
  await sendCommand(client, { type: "transferMap", key });
  return waitUntilSoft(
    client,
    `(() => { const s = window.__mir2Stage5?.state; return s?.mapFileName === ${JSON.stringify(map)} && (s?.sceneInteractionReady === true || s?.sceneAssetReadiness?.ready === true); })()`,
    SCENE_READY_TIMEOUT_MS,
  );
}

// Open (and confirm) an NPC's dialog the way a player would — click its tile so
// the client auto-walks then interacts, with a direct-interact fallback.
async function openNpcDialog(client, npc) {
  const clicked = await clickTile(client, npc.x, npc.y, "left");
  if (!clicked) await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  const mine = `window.__mir2Stage5?.state?.activeNpcDialog != null && String(window.__mir2Stage5.state.activeNpcDialog.npcObjectId) === ${JSON.stringify(String(npc.objectId))}`;
  if (await waitUntilSoft(client, mine, NPC_DIALOG_TIMEOUT_MS)) return true;
  await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  return waitUntilSoft(client, mine, 6_000);
}

// Dispatch a real key event (used to toggle the standalone quest-log window via
// the "q" hotkey, apps/web/app/page.tsx:1593). Blur focus first so the global
// keydown handler fires rather than typing into an input.
async function pressKey(client, key, code, vk) {
  await client
    .evaluate(`(() => { const a = document.activeElement; if (a && typeof a.blur === "function") a.blur(); })()`)
    .catch(() => {});
  await client.send("Input.dispatchKeyEvent", { type: "keyDown", key, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk });
  await client.send("Input.dispatchKeyEvent", { type: "keyUp", key, code, windowsVirtualKeyCode: vk, nativeVirtualKeyCode: vk });
}

// ---------------------------------------------------------------------------
// Quest helpers.
// ---------------------------------------------------------------------------
// QuestStage serializes camelCase (apps/simulation/src/config.rs:3317):
// available | inProgress | readyToTurnIn | completed.
function classifyStage(stage) {
  const s = String(stage ?? "").toLowerCase();
  return {
    raw: stage ?? null,
    available: s === "available",
    inProgress: s === "inprogress",
    readyToTurnIn: s === "readytoturnin",
    completed: s === "completed",
    active: s === "inprogress" || s === "readytoturnin",
  };
}

async function readQuestLog(client) {
  const ql = await client.evaluate(`
    (() => {
      const ql = window.__mir2Stage5?.state?.questLog;
      if (!Array.isArray(ql)) return [];
      return ql.map((q) => ({
        questId: q?.questId ?? null,
        title: q?.title ?? null,
        stage: q?.stage ?? null,
        current: q?.current ?? null,
        required: q?.required ?? null,
        rewardPreview: q?.rewardPreview ?? null,
        objective: q?.objective ?? null,
      }));
    })()
  `);
  return Array.isArray(ql) ? ql : [];
}

async function questById(client, id) {
  const ql = await readQuestLog(client);
  return ql.find((q) => Number(q.questId) === Number(id)) ?? null;
}

// Scan the active NPC dialog for a `@quest:<action>:<id>` link (the real-player
// accept/finish path, apps/simulation/src/runtime/quests.rs:1216).
async function findDialogQuestLink(client, action) {
  return client.evaluate(`
    (() => {
      const d = window.__mir2Stage5?.state?.activeNpcDialog;
      const links = d && Array.isArray(d.links) ? d.links : [];
      const re = /^@quest:${action}:(\\d+)/i;
      for (const l of links) {
        const m = re.exec(String(l?.target ?? ""));
        if (m) return { target: l.target, text: l.text ?? null, questId: Number(m[1]) };
      }
      return null;
    })()
  `);
}

// Parse the received WS frames (the {type:"packet", packet, payload} envelope,
// apps/gateway/src/web.rs) that arrived at/after `sinceTs` (ms epoch) for a given
// packet name. Scanning by TIMESTAMP, not array index, keeps this correct against
// the rolling buffer (a length-based "since index" silently returns nothing once
// the buffer caps out and indices shift — a real false-negative we hit).
function wsPacketsSince(client, sinceTs, packetName) {
  const out = [];
  for (const f of client.webSocketFramesReceived) {
    if (typeof f.at === "number" && f.at < sinceTs) continue;
    try {
      const o = JSON.parse(f.payloadData);
      if (o && o.packet === packetName) out.push(o.payload ?? o);
    } catch {
      /* non-JSON frame */
    }
  }
  return out;
}

// True iff a CompleteQuest frame carrying `questId` arrived at/after `sinceTs`.
function sawCompleteQuest(client, sinceTs, questId) {
  return wsPacketsSince(client, sinceTs, "CompleteQuest").some((p) => {
    const ids = Array.isArray(p?.completedQuests) ? p.completedQuests : [];
    return ids.map(Number).includes(Number(questId));
  });
}

// The payload of the latest worldSnapshot frame at/after `sinceTs` — the server's
// authoritative post-action state (gold + inventoryItems + questLog), used to
// corroborate the reward landing independently of the React client state.
function latestWorldSnapshotSince(client, sinceTs) {
  let latest = null;
  for (const f of client.webSocketFramesReceived) {
    if (typeof f.at === "number" && f.at < sinceTs) continue;
    try {
      const o = JSON.parse(f.payloadData);
      if (o && (o.packet === "worldSnapshot" || o.type === "worldSnapshot")) latest = o.payload ?? o;
    } catch {
      /* non-JSON frame */
    }
  }
  return latest;
}

// Crystal stores the belt as the top BeltSize slots of the SINGLE inventory array
// (Info.Inventory), and GainItem -> AddItem auto-routes Potions/Scrolls/Amulets into
// that belt region (Crystal/Server/MirObjects/HumanObject.cs:1596). The web port
// splits that one array into separate `inventoryItems` (bag) + `beltItems` (belt)
// fields, both rendered for the player (the belt bar is the belt-dialog,
// original-client-panels.tsx). A reward potion like quest #1's (HP)DrugSmall is
// ItemType.Potion, so the server auto-routes it into the belt and the snapshot
// carries it under `beltItems`, NOT `inventoryItems`. "Did the reward land" must
// therefore look at the player's WHOLE carried possession (bag + belt), or a
// belt-routed reward false-negatives as "not delivered".
async function inventorySignature(client) {
  const sig = await client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const bag = Array.isArray(state.inventoryItems) ? state.inventoryItems : [];
      const belt = Array.isArray(state.beltItems) ? state.beltItems : [];
      const items = [...bag, ...belt];
      const byKey = {};
      let totalQty = 0;
      for (const it of items) {
        const key = String(it?.key ?? it?.name ?? it?.uniqueId ?? "?");
        const qty = Number(it?.quantity ?? it?.count ?? 1) || 0;
        byKey[key] = (byKey[key] ?? 0) + qty;
        totalQty += qty;
      }
      return { count: items.length, bagCount: bag.length, beltCount: belt.length, totalQty, byKey };
    })()
  `);
  return sig ?? { count: 0, bagCount: 0, beltCount: 0, totalQty: 0, byKey: {} };
}

// Diff two inventory signatures into a list of newly-gained {key, delta}.
function inventoryGains(before, after) {
  const gains = [];
  const a = before?.byKey ?? {};
  const b = after?.byKey ?? {};
  for (const key of Object.keys(b)) {
    const delta = (b[key] ?? 0) - (a[key] ?? 0);
    if (delta > 0) gains.push({ key, delta });
  }
  return gains;
}

function parseRewardExpectation(rewardPreview) {
  const t = String(rewardPreview ?? "");
  // crystal_quest_reward_preview (apps/simulation/src/runtime/quests.rs:203)
  // renders fixed rewards as "{name} x{count}" and select rewards as
  // "Choose: a / b / c". Pull the item NAMES so we can corroborate the promised
  // item against the authoritative post-finish snapshot by name.
  const itemNames = [];
  for (const m of t.matchAll(/([^,]+?)\s+x\d+/gi)) itemNames.push(m[1].trim());
  const choose = /choose:\s*(.+)$/i.exec(t);
  if (choose) for (const name of choose[1].split("/")) itemNames.push(name.trim());
  return {
    text: t,
    gold: /gold/i.test(t),
    exp: /exp/i.test(t),
    credit: /credit/i.test(t),
    item: /\bx\d+\b|choose:/i.test(t),
    itemNames: itemNames.filter(Boolean),
  };
}

// Does the authoritative post-finish worldSnapshot (server truth) carry a promised
// reward item anywhere in the player's carried possession (bag + belt)? Crystal
// auto-routes Potion/Scroll/Amulet rewards into the belt, so both arrays matter.
function snapshotContainsRewardItem(snap, reward) {
  if (!snap || !Array.isArray(reward?.itemNames) || reward.itemNames.length === 0) return false;
  const items = [
    ...(Array.isArray(snap.inventoryItems) ? snap.inventoryItems : []),
    ...(Array.isArray(snap.beltItems) ? snap.beltItems : []),
  ];
  return reward.itemNames.some((name) =>
    items.some((it) => String(it?.name ?? "").toLowerCase() === name.toLowerCase()),
  );
}

// ---------------------------------------------------------------------------
// The quest journey.
// ---------------------------------------------------------------------------
async function questJourney(ctx) {
  const client = ctx.client;
  // Shared arc state threaded across beats.
  const arc = {
    questId: QUEST_ID,
    acceptedViaNpc: null, // {questId, npcName, target}
    ready: false, // arc quest reached readyToTurnIn (objective satisfied)
    rewardPreview: null,
  };

  // --- Bootstrap: register -> login -> character -> enter world. ------------
  await runBeat(ctx, "open client + login screen", async () => {
    await navigate(client, RUN_URL);
    await waitUntil(
      client,
      "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
      "client stage ready",
      25_000,
    );
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "login") await waitForSelectorSoft(client, ".login-overlay", 8_000);
  });

  await runBeat(ctx, "register account", async (beat) => {
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    beat.account = account;
    if (screen !== "login") {
      beat.note = `already past login (screen=${screen}); skipping registration`;
      return;
    }
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    if (createAccount) {
      await click(client, ".login-button.account button");
      await waitUntil(client, "window.__mir2Stage5?.state?.wsState === 'open'", "account-creation socket open", 15_000);
      await delay(2000);
    }
  });

  await runBeat(ctx, "login to character select", async () => {
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "select" || screen === "game") return;
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.ok button");
    await waitUntil(client, "window.__mir2Stage5?.state?.screen === 'select'", "select screen", 30_000);
  });

  await runBeat(ctx, "create character + start game", async (beat) => {
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "game") return;
    beat.characterName = characterName;

    // Create OUR OWN server-backed character; do NOT trust state.characters to
    // decide existence (it can carry a phantom slot the server never returned).
    const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
    if (!(await client.evaluate(haveOurChar))) {
      await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
      let created = await waitUntilSoft(client, haveOurChar, 15_000);
      if (!created) {
        await click(client, ".select-action.new button").catch(() => {});
        await fillInput(client, "input[name='characterName'], .new-character-name, .character-create-name", characterName).catch(() => {});
        await click(client, ".select-action.ok button, .new-character-confirm button, .character-create-confirm button").catch(() => {});
        created = await waitUntilSoft(client, haveOurChar, 10_000);
      }
      if (!created) {
        ctx.addIssue({
          category: "flow",
          severity: "high",
          beat: beat.id,
          summary: "character creation did not produce a server-backed character",
          detail: { characterName },
        });
      }
    }

    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
    );
    beat.startIndex = startIndex;
    const wsBefore = client.webSocketFramesReceived.length;
    await sendCommand(client, { type: "startGame", characterIndex: startIndex });
    const entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
    if (!entered) {
      const result = startGameResultFrom(client.webSocketFramesReceived.slice(wsBefore));
      beat.startGameResult = result;
      ctx.addIssue({
        category: "flow",
        severity: "high",
        beat: beat.id,
        summary:
          result === null
            ? "startGame sent but never entered the game (no StartGame reply seen)"
            : `startGame rejected by server (StartGame result=${result})`,
        detail: { startIndex, result },
      });
      throw new Error(`did not enter game (StartGame result=${result})`);
    }
  });

  await runBeat(ctx, "enter world + reach quest hub", async (beat) => {
    await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 8_000).catch(() => {});
    await waitUntilSoft(
      client,
      "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
      SCENE_READY_TIMEOUT_MS,
    );
    // Hop to the spawn-town quest hub (the assistant NPC area). Best-effort: the
    // arc still completes via the wired commands if the hub transfer is rejected.
    beat.questHub = QUEST_HUB;
    const arrived = await transferTo(client, QUEST_HUB).catch(() => false);
    beat.arrivedAtHub = arrived;
    await delay(1000);
    const st = await readGameState(client);
    beat.mapFileName = st.mapFileName;
    beat.player = st.player;
    beat.npcCount = st.npcCount;
    beat.render = await assessRenderHealth(ctx, beat.id, "quest hub");
  });

  // --- Accept: NPC-dialog real-player path + the wired command spine. -------
  await runBeat(ctx, "accept quest (NPC dialog + AcceptQuest command)", async (beat) => {
    // (a) Opportunistic real-player accept: look at the nearest loaded NPCs for a
    // `@quest:accept:<id>` dialog link and select it like a player would.
    const state = await readGameState(client);
    const p = state.player ?? { x: 0, y: 0 };
    const ranked = [...state.npcs].sort(
      (a, b) => Math.abs(a.x - p.x) + Math.abs(a.y - p.y) - (Math.abs(b.x - p.x) + Math.abs(b.y - p.y)),
    );
    beat.npcsConsidered = ranked.slice(0, 6).map((n) => ({ name: n.name, x: n.x, y: n.y }));
    for (const npc of ranked.slice(0, 6)) {
      if (arc.acceptedViaNpc) break;
      if (!(await openNpcDialog(client, npc))) continue;
      const link = await findDialogQuestLink(client, "accept");
      if (!link) continue;
      const before = Date.now();
      await sendCommand(client, { type: "selectNpcDialog", target: link.target });
      const taken = await waitUntilSoft(
        client,
        `(() => { const q = (window.__mir2Stage5?.state?.questLog ?? []).find((e) => Number(e?.questId) === ${Number(link.questId)}); return q && /inprogress|readytoturnin/i.test(String(q.stage)); })()`,
        QUEST_TIMEOUT_MS,
      );
      const changeFrames = wsPacketsSince(client, before, "ChangeQuest").filter((f) => Number(f?.questId) === Number(link.questId));
      if (taken || changeFrames.some((f) => f?.taken === true)) {
        arc.acceptedViaNpc = { questId: link.questId, npcName: npc.name ?? String(npc.objectId), target: link.target };
        ctx.log(`  ✓ accepted quest ${link.questId} via NPC "${npc.name ?? npc.objectId}" dialog link`);
      }
    }
    if (!arc.acceptedViaNpc) {
      ctx.addIssue({
        category: "quest",
        severity: "low",
        beat: beat.id,
        summary:
          "no quest-giver NPC offered an accept link from the hub (on-demand NPC not loaded here / object-id mismatch); falling back to the wired AcceptQuest command",
        detail: { hub: QUEST_HUB, npcsConsidered: beat.npcsConsidered },
      });
    }

    // (b) Deterministic spine: ensure the canonical completable quest is accepted
    // via the wired command (this IS the real ClientPacket::AcceptQuest path).
    let canonical = await questById(client, QUEST_ID);
    const canonicalActiveAlready = classifyStage(canonical?.stage).active;
    if (!canonicalActiveAlready) {
      const wsBefore = Date.now();
      const sent = await sendCommand(client, { type: "acceptQuest", npcIndex: 0, questIndex: QUEST_ID });
      beat.acceptCommandSent = sent;
      await waitUntilSoft(
        client,
        `(() => { const q = (window.__mir2Stage5?.state?.questLog ?? []).find((e) => Number(e?.questId) === ${Number(QUEST_ID)}); return q && /inprogress|readytoturnin/i.test(String(q.stage)); })()`,
        QUEST_TIMEOUT_MS,
      );
      await delay(SNAPSHOT_SETTLE_MS); // let the post-accept world snapshot land
      beat.acceptChangeFrames = wsPacketsSince(client, wsBefore, "ChangeQuest").filter((f) => Number(f?.questId) === Number(QUEST_ID));
      canonical = await questById(client, QUEST_ID);
    } else {
      beat.note = `canonical quest ${QUEST_ID} already active (stage ${canonical?.stage})`;
      canonical = await questById(client, QUEST_ID);
    }

    // Decide which quest carries the arc: prefer a readyToTurnIn quest (finishable
    // without a kill). The canonical #1 reaches readyToTurnIn on accept.
    const canonicalCls = classifyStage(canonical?.stage);
    arc.questId = QUEST_ID;
    arc.ready = canonicalCls.readyToTurnIn;
    arc.rewardPreview = canonical?.rewardPreview ?? null;
    beat.arcQuestId = arc.questId;
    beat.arcStage = canonical?.stage ?? null;
    beat.arcReady = arc.ready;
    beat.rewardPreview = arc.rewardPreview;
    beat.acceptedViaNpc = arc.acceptedViaNpc;

    if (!canonicalCls.active) {
      ctx.addIssue({
        category: "quest",
        severity: "high",
        beat: beat.id,
        summary: `canonical quest ${QUEST_ID} did not enter the active quest log after AcceptQuest (stage=${canonical?.stage ?? "absent"})`,
        detail: { canonical, acceptedViaNpc: arc.acceptedViaNpc },
      });
    } else {
      ctx.log(`  ✓ quest ${arc.questId} accepted (stage ${canonical?.stage}); reward preview: ${arc.rewardPreview ?? "—"}`);
    }
  });

  // --- Confirm it entered the quest log (+ window renders). -----------------
  await runBeat(ctx, "confirm quest in log + quest-log window renders", async (beat) => {
    const q = await questById(client, arc.questId);
    beat.questInLog = Boolean(q);
    beat.questStage = q?.stage ?? null;
    if (!q || !classifyStage(q.stage).active) {
      ctx.addIssue({
        category: "quest",
        severity: "high",
        beat: beat.id,
        summary: `accepted quest ${arc.questId} is not present as an active entry in state.questLog`,
        detail: { quest: q },
      });
    }
    // Open the standalone quest-log window (the "q" hotkey -> showQuestLog ->
    // QuestLogWindow, apps/web/app/page.tsx:1593,11228) and confirm it rendered.
    let windowOpen = await client.evaluate(`document.querySelector("[data-quest-stage-filter]") != null`);
    if (!windowOpen) {
      await pressKey(client, "q", "KeyQ", 81);
      windowOpen = await waitForSelectorSoft(client, "[data-quest-stage-filter]", 5_000);
    }
    // Fallback: the HUD quest button opens the inventory's quest tab.
    if (!windowOpen) {
      await client
        .evaluate(`(() => { const b = document.querySelector(".hud-button.quest button"); if (b) b.click(); })()`)
        .catch(() => {});
      windowOpen = await waitForSelectorSoft(client, "[data-quest-id], .inventory-window", 5_000);
    }
    const rows = await client.evaluate(`document.querySelectorAll("[data-quest-id]").length`);
    const rowForArc = await client.evaluate(
      `document.querySelector('[data-quest-id="${Number(arc.questId)}"]') != null`,
    );
    beat.questWindowOpen = Boolean(windowOpen);
    beat.questWindowRows = rows;
    beat.questRowForArc = Boolean(rowForArc);
    if (!windowOpen) {
      ctx.addIssue({
        category: "ui",
        severity: "medium",
        beat: beat.id,
        summary: "could not open a quest-log view (neither the 'q' window nor the HUD quest tab rendered)",
        detail: { rows },
      });
    }
  });

  // --- Satisfy / short-circuit the objective. -------------------------------
  await runBeat(ctx, "satisfy / short-circuit the objective", async (beat) => {
    const q = await questById(client, arc.questId);
    const cls = classifyStage(q?.stage);
    beat.stage = q?.stage ?? null;
    beat.current = q?.current ?? null;
    beat.required = q?.required ?? null;
    if (cls.readyToTurnIn) {
      arc.ready = true;
      beat.method = "talk/delivery — no kill required";
      beat.note =
        `quest ${arc.questId} reached readyToTurnIn on accept (begin_quest, quests.rs:646): a no-kill delivery quest, objective auto-satisfied — the reachable short-circuit on the local stack`;
      ctx.log(`  ✓ objective short-circuited: quest ${arc.questId} is ready to turn in`);
      return;
    }
    if (cls.inProgress) {
      // The quest carries a kill/collect/flag objective. Those are advanced by
      // gameplay (advance_crystal_quest_kill, quests.rs:979) and cannot be
      // short-circuited without GM (@SETQUEST is is_gm_or_test-gated and a no-op
      // on the local stack — no MIR2_GM_PASSWORD). Documented, not failed.
      arc.ready = false;
      beat.method = "kill/collect objective — not short-circuitable without GM";
      ctx.addIssue({
        category: "quest",
        severity: "low",
        beat: beat.id,
        summary: `quest ${arc.questId} is inProgress with a kill/collect objective (${q?.current ?? 0}/${q?.required ?? "?"}); not short-circuitable on the local stack (GM @SETQUEST is gated/no-op) — finish arc will be best-effort`,
        detail: { quest: q },
      });
      return;
    }
    beat.note = `quest ${arc.questId} stage=${q?.stage ?? "absent"}; objective state unknown`;
  });

  // --- Share (ShareQuest) — env-limited without a party. ---------------------
  await runBeat(ctx, "share quest (ShareQuest)", async (beat) => {
    const wsBefore = Date.now();
    const sent = await sendCommand(client, { type: "shareQuest", questIndex: arc.questId });
    beat.shareCommandSent = sent;
    await delay(800);
    const shareFrames = wsPacketsSince(client, wsBefore, "ShareQuest");
    // No party member -> stage5_share_quest_packet returns a "quest not shared"
    // system message (packets.rs:2992); a successful share would push a ShareQuest
    // packet instead. Either way the command path is exercised.
    beat.shareFrames = shareFrames.length;
    beat.outcome = shareFrames.length > 0 ? "shared-to-party" : "no-party (quest-not-shared system message)";
    if (!sent) {
      ctx.addIssue({
        category: "quest",
        severity: "medium",
        beat: beat.id,
        summary: "ShareQuest command was not accepted by the client send bridge",
        detail: { questId: arc.questId },
      });
    } else if (shareFrames.length === 0) {
      ctx.addIssue({
        category: "quest",
        severity: "low",
        beat: beat.id,
        summary:
          "ShareQuest produced no broadcast — expected on the local stack (no second client to party with; share requires a group member)",
        detail: { questId: arc.questId },
      });
    }
  });

  // --- Abandon (AbandonQuest) — demonstrated on the guide quest. -------------
  await runBeat(ctx, "abandon quest (AbandonQuest)", async (beat) => {
    const abandonId = ABANDON_QUEST_ID;
    beat.abandonQuestId = abandonId;
    // Ensure the guide quest is active (accept it if needed; it goes inProgress
    // from anywhere with npcIndex:0 — begin_quest template path, quests.rs:633).
    let q = await questById(client, abandonId);
    if (!classifyStage(q?.stage).active) {
      await sendCommand(client, { type: "acceptQuest", npcIndex: 0, questIndex: abandonId });
      await waitUntilSoft(
        client,
        `(() => { const e = (window.__mir2Stage5?.state?.questLog ?? []).find((x) => Number(x?.questId) === ${Number(abandonId)}); return e && /inprogress|readytoturnin/i.test(String(e.stage)); })()`,
        QUEST_TIMEOUT_MS,
      );
      await delay(SNAPSHOT_SETTLE_MS);
      q = await questById(client, abandonId);
    }
    beat.stageBeforeAbandon = q?.stage ?? null;
    const wasActive = classifyStage(q?.stage).active;
    if (!wasActive) {
      ctx.addIssue({
        category: "quest",
        severity: "medium",
        beat: beat.id,
        summary: `could not bring guide quest ${abandonId} to an active stage to demonstrate abandon (stage=${q?.stage ?? "absent"})`,
        detail: { quest: q },
      });
      return;
    }
    // Abandon it.
    const wsBefore = Date.now();
    const sent = await sendCommand(client, { type: "abandonQuest", questIndex: abandonId });
    beat.abandonCommandSent = sent;
    const leftActive = await waitUntilSoft(
      client,
      `(() => { const e = (window.__mir2Stage5?.state?.questLog ?? []).find((x) => Number(x?.questId) === ${Number(abandonId)}); return !e || !/inprogress|readytoturnin/i.test(String(e.stage)); })()`,
      QUEST_TIMEOUT_MS,
    );
    await delay(SNAPSHOT_SETTLE_MS);
    const after = await questById(client, abandonId);
    beat.stageAfterAbandon = after?.stage ?? "(removed)";
    beat.abandonChangeFrames = wsPacketsSince(client, wsBefore, "ChangeQuest").filter((f) => Number(f?.questId) === Number(abandonId)).length;
    if (!leftActive || classifyStage(after?.stage).active) {
      ctx.addIssue({
        category: "quest",
        severity: "high",
        beat: beat.id,
        summary: `AbandonQuest did not drop guide quest ${abandonId} from the active log (stage still ${after?.stage})`,
        detail: { before: beat.stageBeforeAbandon, after: beat.stageAfterAbandon },
      });
    } else {
      ctx.log(`  ✓ abandoned guide quest ${abandonId} (${beat.stageBeforeAbandon} -> ${beat.stageAfterAbandon})`);
    }
  });

  // --- Finish (FinishQuest) + reward verification. --------------------------
  await runBeat(ctx, "finish quest (FinishQuest) + reward lands", async (beat) => {
    const q = await questById(client, arc.questId);
    const cls = classifyStage(q?.stage);
    beat.stageBeforeFinish = q?.stage ?? null;
    beat.arcReady = arc.ready;
    const reward = parseRewardExpectation(q?.rewardPreview ?? arc.rewardPreview);
    beat.rewardExpectation = reward;

    if (!cls.readyToTurnIn) {
      // The arc quest is not ready (kill/collect objective unmet). Still call
      // FinishQuest to confirm the server correctly refuses an unmet turn-in
      // (stage5_finish_quest_packet returns early unless readyToTurnIn,
      // packets.rs:2945), then record the env limitation.
      const wsBefore = Date.now();
      await sendCommand(client, { type: "finishQuest", questIndex: arc.questId, selectedItemIndex: -1 });
      await delay(SNAPSHOT_SETTLE_MS);
      const completed = sawCompleteQuest(client, wsBefore, arc.questId);
      beat.finishRejectedWhileNotReady = !completed;
      ctx.addIssue({
        category: "quest",
        severity: completed ? "high" : "low",
        beat: beat.id,
        summary: completed
          ? `FinishQuest completed quest ${arc.questId} while NOT readyToTurnIn (objective bypass bug?)`
          : `quest ${arc.questId} was not readyToTurnIn (objective unmet, GM-gated short-circuit unavailable); FinishQuest correctly produced no completion — reward arc skipped on the local stack`,
        detail: { quest: q },
      });
      return;
    }

    // Capture reward baselines.
    const goldBefore = (await readGameState(client)).gold;
    const creditBefore = (await readGameState(client)).credit;
    const invBefore = await inventorySignature(client);
    const wsBefore = Date.now();

    // (a) Real-player turn-in: hop to the finish NPC area and select its
    // `@quest:finish:<id>` link if one is loaded.
    let finishedViaNpc = false;
    await transferTo(client, FINISH_HUB).catch(() => {});
    await delay(800);
    {
      const st = await readGameState(client);
      const p = st.player ?? { x: 0, y: 0 };
      const ranked = [...st.npcs].sort(
        (a, b) => Math.abs(a.x - p.x) + Math.abs(a.y - p.y) - (Math.abs(b.x - p.x) + Math.abs(b.y - p.y)),
      );
      for (const npc of ranked.slice(0, 6)) {
        if (finishedViaNpc) break;
        if (!(await openNpcDialog(client, npc))) continue;
        const link = await findDialogQuestLink(client, "finish");
        if (!link || Number(link.questId) !== Number(arc.questId)) continue;
        await sendCommand(client, { type: "selectNpcDialog", target: link.target });
        finishedViaNpc = await waitUntilSoft(
          client,
          `(() => { return (window.__mir2Stage5?.state?.questLog ?? []).some((e) => Number(e?.questId) === ${Number(arc.questId)} && /completed/i.test(String(e.stage))); })()`,
          QUEST_TIMEOUT_MS,
        );
        if (finishedViaNpc) beat.finishNpc = npc.name ?? String(npc.objectId);
      }
    }
    beat.finishedViaNpc = finishedViaNpc;

    // (b) Deterministic spine: the wired FinishQuest command (real ClientPacket::
    // FinishQuest). selectedItemIndex -1 = "no reward choice" (web.rs:1184).
    if (!finishedViaNpc) {
      await sendCommand(client, { type: "finishQuest", questIndex: arc.questId, selectedItemIndex: -1 });
    }
    // FinishQuest is a snapshot-triggering action, so a fresh world snapshot
    // (with updated gold + inventory) follows — let it land before re-reading.
    await waitUntilSoft(
      client,
      `(() => { return (window.__mir2Stage5?.state?.questLog ?? []).some((e) => Number(e?.questId) === ${Number(arc.questId)} && /completed/i.test(String(e.stage))); })()`,
      QUEST_TIMEOUT_MS,
    );
    await delay(SNAPSHOT_SETTLE_MS);

    // Verify: (1) authoritative WS CompleteQuest truth, (2) quest left the active
    // log, (3) reward delta client-side.
    const completeFrame = sawCompleteQuest(client, wsBefore, arc.questId);
    const after = await questById(client, arc.questId);
    const afterCls = classifyStage(after?.stage);
    const goldAfter = (await readGameState(client)).gold;
    const creditAfter = (await readGameState(client)).credit;
    const invAfter = await inventorySignature(client);
    const gains = inventoryGains(invBefore, invAfter);
    const goldDelta = typeof goldAfter === "number" && typeof goldBefore === "number" ? goldAfter - goldBefore : null;
    const creditDelta =
      typeof creditAfter === "number" && typeof creditBefore === "number" ? creditAfter - creditBefore : null;
    // Authoritative server truth for the reward: the post-finish worldSnapshot's
    // own inventory/gold (independent of the React client state). Count bag + belt:
    // a Potion/Scroll reward auto-routes into the belt (see inventorySignature).
    const snapAfter = latestWorldSnapshotSince(client, wsBefore);
    const snapBagCount =
      snapAfter && Array.isArray(snapAfter.inventoryItems) ? snapAfter.inventoryItems.length : null;
    const snapBeltCount =
      snapAfter && Array.isArray(snapAfter.beltItems) ? snapAfter.beltItems.length : null;
    const snapInventoryCount =
      snapBagCount === null && snapBeltCount === null ? null : (snapBagCount ?? 0) + (snapBeltCount ?? 0);
    const snapGold = snapAfter && typeof snapAfter.gold === "number" ? snapAfter.gold : null;
    // Authoritative completion + reward truth from the WS layer (CompleteQuest frame
    // + the post-finish snapshot's own questLog/possession). The React client state
    // (`after` / `gains`) can lag the snapshot merge — especially right after the
    // turn-in NPC's map transfer churns snapshots — so a verdict built only on it
    // false-negatives a reward that the server actually granted. Prefer WS truth and
    // fall back to client state.
    const snapStage =
      snapAfter && Array.isArray(snapAfter.questLog)
        ? snapAfter.questLog.find((q) => Number(q?.questId) === Number(arc.questId))?.stage ?? null
        : null;
    const completedAuthoritative = completeFrame || /completed/i.test(String(snapStage ?? ""));
    const stillActive = afterCls.active && !completedAuthoritative;
    const snapHasPromisedItem = snapshotContainsRewardItem(snapAfter, reward);
    // The reward landed if the client gained it OR the authoritative snapshot shows
    // it by name, OR the snapshot's total carried possession (bag + belt) grew.
    const itemLandedAuthoritative =
      snapHasPromisedItem ||
      (snapInventoryCount != null && snapInventoryCount > (invBefore?.count ?? 0));

    beat.finish = {
      completeQuestFrameSeen: completeFrame,
      stageAfter: after?.stage ?? "(removed)",
      snapshotStageAfter: snapStage,
      completedAuthoritative,
      leftActiveLog: !stillActive,
      goldBefore,
      goldAfter,
      goldDelta,
      creditBefore,
      creditAfter,
      creditDelta,
      inventoryGains: gains,
      snapshotHasPromisedItem: snapHasPromisedItem,
      snapshotInventoryCount: snapInventoryCount,
      snapshotBagCount: snapBagCount,
      snapshotBeltCount: snapBeltCount,
      snapshotGold: snapGold,
      rewardExpectation: reward,
    };
    ctx.log(
      `  finish: completeFrame=${completeFrame} stage=${after?.stage}/snap=${snapStage ?? "—"} goldΔ=${goldDelta ?? "—"} creditΔ=${creditDelta ?? "—"} items+=${gains.length} snapItem=${snapHasPromisedItem}`,
    );

    // (1) Authoritative completion.
    if (!completeFrame) {
      ctx.addIssue({
        category: "quest",
        severity: "high",
        beat: beat.id,
        summary: `FinishQuest produced no CompleteQuest frame for quest ${arc.questId} (server did not complete it)`,
        detail: beat.finish,
      });
    }
    // (2) Left the active log — trust the authoritative completion signal (the
    // CompleteQuest frame / snapshot stage), not the possibly-lagging React state.
    if (stillActive) {
      ctx.addIssue({
        category: "quest",
        severity: "high",
        beat: beat.id,
        summary: `quest ${arc.questId} is still active (client=${after?.stage}, snapshot=${snapStage ?? "—"}) after FinishQuest — did not leave the active log`,
        detail: beat.finish,
      });
    }
    // (3) Reward landed. Use the quest's own reward preview to set expectations:
    // a Gold/item reward must reach the player (the post-finish snapshot carries it,
    // in the bag OR the belt); an EXP-only reward is not client-readable (state omits
    // playerExperience) so the CompleteQuest frame is the authority there. Each
    // signal also accepts the authoritative WS snapshot to survive client-merge lag.
    const goldLanded = (goldDelta ?? 0) > 0 || (snapGold != null && snapGold > (goldBefore ?? 0));
    const creditLanded = (creditDelta ?? 0) > 0;
    const itemLanded = gains.length > 0 || itemLandedAuthoritative;
    const anyClientReward = goldLanded || creditLanded || itemLanded;
    beat.finish.rewardLandedClientSide = anyClientReward;
    if (reward.gold && !goldLanded) {
      ctx.addIssue({
        category: "quest",
        severity: "medium",
        beat: beat.id,
        summary: `quest ${arc.questId} reward preview promised gold ("${reward.text}") but client gold did not increase after finish`,
        detail: beat.finish,
      });
    } else if (reward.item && !itemLanded) {
      ctx.addIssue({
        category: "quest",
        severity: "medium",
        beat: beat.id,
        summary: `quest ${arc.questId} reward preview promised an item ("${reward.text}") but no bag/belt item appeared after finish`,
        detail: beat.finish,
      });
    } else if (!anyClientReward && completeFrame && !reward.gold && !reward.item && reward.exp) {
      // EXP-only reward: applied server-side (CompleteQuest); not surfaced client-side.
      ctx.addIssue({
        category: "quest",
        severity: "low",
        beat: beat.id,
        summary: `quest ${arc.questId} reward is EXP-only ("${reward.text}"); applied server-side (CompleteQuest seen) but not visible client-side (state omits playerExperience)`,
        detail: beat.finish,
      });
    } else if (!anyClientReward && completeFrame) {
      ctx.addIssue({
        category: "quest",
        severity: "low",
        beat: beat.id,
        summary: `quest ${arc.questId} completed (CompleteQuest seen) but no gold/credit/item delta observed client-side; reward preview was "${reward.text || "—"}"`,
        detail: beat.finish,
      });
    } else if (anyClientReward) {
      const itemNote = itemLanded
        ? gains.length > 0
          ? `${gains.length} item(s)`
          : "item present in post-finish snapshot (belt/bag)"
        : null;
      ctx.log(
        `  ✓ reward landed: ${[goldLanded ? `+${goldDelta ?? snapGold} gold` : null, creditLanded ? `+${creditDelta} credit` : null, itemNote].filter(Boolean).join(", ")}`,
      );
    }
  });

  // --- Wrap. ----------------------------------------------------------------
  await runBeat(ctx, "wrap + collect diagnostics", async (beat) => {
    beat.final = await readGameState(client);
    beat.arc = arc;
  });
}

// ---------------------------------------------------------------------------
// Cross-cutting collectors (run after the journey).
// ---------------------------------------------------------------------------
function collectNetworkIssues(ctx) {
  const failures = ctx.client.networkFailures;
  if (!failures.length) return;
  const byKind = new Map();
  for (const f of failures) {
    const key = f.kind ?? "other";
    if (!byKind.has(key)) byKind.set(key, []);
    byKind.get(key).push(f);
  }
  for (const [kind, list] of byKind) {
    const severity = kind === "runtime" ? "high" : kind === "map" || kind === "ui" || kind === "entity" ? "medium" : "low";
    const sample = list.slice(0, 6).map((f) => `${f.status || f.errorText || "ERR"} ${f.url}`);
    ctx.addIssue({
      category: "network",
      severity,
      beat: null,
      summary: `${list.length} ${kind} asset request(s) failed`,
      detail: { kind, count: list.length, sample, note: "If files exist in git, the R2 release may be stale (see ASSET-RELEASE-RUNBOOK)." },
    });
  }
}

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
      severity: e.type === "error" ? "medium" : "low",
      beat: null,
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
    baseUrl,
    account,
    characterName,
    questId: QUEST_ID,
    abandonQuestId: ABANDON_QUEST_ID,
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    beats: ctx.beats.length,
    beatsOk: ctx.beats.filter((b) => b.ok).length,
    issueCount: ctx.issues.length,
    bySeverity,
    byCategory,
  };

  await fs.writeFile(path.join(outputDir, "summary.json"), JSON.stringify(summary, null, 2));
  await fs.writeFile(
    path.join(outputDir, "report.json"),
    JSON.stringify({ summary, issues: ctx.issues, beats: ctx.beats }, null, 2),
  );
  await fs.writeFile(
    path.join(outputDir, "console.json"),
    JSON.stringify({ errors: ctx.client.consoleErrors, messages: ctx.client.consoleMessages.slice(-200) }, null, 2),
  );
  await fs.writeFile(path.join(outputDir, "network-failures.json"), JSON.stringify(ctx.client.networkFailures, null, 2));
  await fs.writeFile(
    path.join(outputDir, "ws-timeline.json"),
    JSON.stringify(
      {
        sent: ctx.client.webSocketFramesSent.slice(-200),
        received: ctx.client.webSocketFramesReceived.slice(-300),
      },
      null,
      2,
    ),
  );

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  const md = [];
  md.push(`# Player-QA quest-arc report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\`  ·  account \`${account}\`  ·  character \`${characterName}\``);
  md.push(`- Quest arc: #${QUEST_ID} (finish) · #${ABANDON_QUEST_ID} (abandon demo)`);
  md.push(`- Beats: ${summary.beatsOk}/${summary.beats} ok  ·  Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push(`- By category: ${Object.entries(byCategory).map(([k, v]) => `${k} ${v}`).join(", ") || "none"}`);
  md.push("");
  md.push("## Issues (by severity)");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues detected._");
  } else {
    md.push("| # | Severity | Category | Beat | Summary |");
    md.push("|---|---|---|---|---|");
    for (const i of sortedIssues) {
      md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.beat ?? "—"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    }
  }
  md.push("");
  md.push("## Quest arc (beats)");
  md.push("");
  for (const b of ctx.beats) {
    md.push(`### Beat ${b.id} — ${b.title} ${b.ok ? "✅" : "❌"}  (${b.durationMs}ms)`);
    if (b.error) md.push(`- error: \`${b.error.slice(0, 300)}\``);
    if (b.note) md.push(`- note: ${b.note}`);
    if (b.state) {
      md.push(
        `- state: screen=\`${b.state.screen}\` map=\`${b.state.mapFileName ?? "?"}\` pos=${b.state.player ? `(${b.state.player.x},${b.state.player.y})` : "—"} npcs=${b.state.npcCount} quests=${b.state.questLogCount} gold=${b.state.gold ?? "—"}`,
      );
    }
    if (b.acceptedViaNpc) md.push(`- accepted via NPC: quest ${b.acceptedViaNpc.questId} @ "${b.acceptedViaNpc.npcName}" (\`${b.acceptedViaNpc.target}\`)`);
    if (b.arcQuestId !== undefined) md.push(`- arc quest: #${b.arcQuestId} stage=${b.arcStage} ready=${b.arcReady} reward="${b.rewardPreview ?? "—"}"`);
    if (b.questWindowOpen !== undefined) md.push(`- quest-log window: open=${b.questWindowOpen} rows=${b.questWindowRows} rowForArc=${b.questRowForArc}`);
    if (b.method) md.push(`- objective: ${b.method}`);
    if (b.outcome) md.push(`- share: ${b.outcome}`);
    if (b.stageBeforeAbandon !== undefined) md.push(`- abandon: ${b.stageBeforeAbandon} -> ${b.stageAfterAbandon}`);
    if (b.finish) md.push(`- finish: ${JSON.stringify(b.finish)}`);
    if (b.render) md.push(`- render: mean=${fmt(b.render.mean)} variance=${fmt(b.render.variance)}`);
    if (b.frame) md.push(`- frame: [\`${b.frame}\`](${b.frame})`);
    md.push("");
  }
  md.push("## Detailed issue evidence");
  md.push("");
  for (const i of sortedIssues) {
    md.push(`### ${i.id} — [${i.severity}/${i.category}] ${i.summary}`);
    if (i.beat) md.push(`- beat: ${i.beat}`);
    if (i.detail !== undefined) md.push("```json\n" + JSON.stringify(i.detail, null, 2) + "\n```");
    md.push("");
  }
  md.push("## Reproduce");
  md.push("");
  md.push("```bash");
  md.push(`cd apps/web && node ./scripts/qa-quests.mjs --headed --baseUrl ${baseUrl} --account ${account}`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-quests-${process.pid}-${Date.now()}`);
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
  await delay(3000);
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
  if (targetAlreadyNavigated) {
    try {
      await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 15_000);
      const currentUrl = await client.evaluate("window.location.href");
      if (typeof currentUrl !== "string" || !currentUrl.startsWith("chrome-error://")) return;
    } catch {
      targetAlreadyNavigated = false;
    }
  }
  let lastError;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      await client.send("Page.navigate", { url });
      await waitUntil(client, "document.readyState === 'complete' || document.readyState === 'interactive'", "page load", 15_000);
      const currentUrl = await client.evaluate("window.location.href");
      if (typeof currentUrl !== "string" || !currentUrl.startsWith("chrome-error://")) return;
      lastError = new Error(`Chrome landed on ${currentUrl}`);
    } catch (error) {
      lastError = error;
    }
    await delay(400);
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

async function waitUntil(client, expression, label, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastValue = null;
  while (Date.now() < deadline) {
    lastValue = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (lastValue) return;
    await delay(120);
  }
  const debug = await client
    .evaluate(
      `(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, sceneReady: window.__mir2Stage5?.state?.sceneInteractionReady ?? null, body: document.body?.innerText?.slice(0, 300) ?? "" }))()`,
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

async function stopChrome(chrome) {
  if (!chrome || chrome.killed) return;
  chrome.kill();
  await new Promise((resolve) => chrome.once("exit", resolve));
}

// ---------------------------------------------------------------------------
// Small utilities.
// ---------------------------------------------------------------------------
function classifyAssetUrl(url) {
  const t = String(url ?? "");
  if (t.includes("/bevy-runtime/")) return "runtime";
  if (t.includes("/original-ui/")) return "ui";
  if (t.includes("/original-map/")) return "map";
  if (t.includes("entity") || t.includes("atlas")) return "entity";
  if (/\.(png|webp|ktx2|basis|wav|mp3|ogg)(\?|$)/i.test(t)) return "asset";
  return "other";
}

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

function isCriticalConsoleError(error) {
  const text = String(error?.text ?? "");
  if (error?.source === "network" && text.includes("net::ERR_FAILED")) return false;
  if (text.includes("favicon")) return false;
  return true;
}

function slug(text) {
  return String(text).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40);
}

function round2(n) {
  return Math.round(n * 100) / 100;
}

function fmt(n) {
  return n === null || n === undefined ? "—" : round2(n);
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

function defaultCharacterName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `QQ${suffix}`.slice(0, 10).toUpperCase();
}

function defaultAccountName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `QQ${suffix}`.slice(0, 12).toUpperCase();
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
  console.log(`qa-quests: target=${baseUrl} account=${account} char=${characterName} questId=${QUEST_ID} headed=${headed}`);
  console.log(`qa-quests: output=${outputDir}`);

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

    await questJourney(ctx);

    collectNetworkIssues(ctx);
    collectConsoleIssues(ctx);
    await writeReport(ctx);

    const s = ctx.issues.reduce((acc, i) => ((acc[i.severity] = (acc[i.severity] ?? 0) + 1), acc), {});
    console.log(
      `\nqa-quests done: ${ctx.beats.filter((b) => b.ok).length}/${ctx.beats.length} beats ok, ${ctx.issues.length} issues (high ${s.high ?? 0}, medium ${s.medium ?? 0}, low ${s.low ?? 0}).`,
    );
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
  } catch (error) {
    console.error("qa-quests fatal:", error);
    if (ctx) {
      try {
        ctx.addIssue({ category: "flow", severity: "high", beat: null, summary: "fatal harness error", detail: String(error?.message ?? error) });
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
