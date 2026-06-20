// qa-items.mjs — AI-driven "real player" QA loop for the full ITEM LIFECYCLE.
//
// The existing qa-playthrough.mjs only OPENS the inventory window. This harness
// exercises an item end-to-end the way a player does and records EVERY broken
// step into a structured report under docs/generated/item-qa/:
//
//   log in -> enter world ->
//     1. OPEN inventory        — click `.hud-button.inventory button`, assert `.inventory-window`
//     2. PICK UP a ground drop — kill a wild monster (Woomyon "1" / Serpent "2"),
//                                 walk onto the drop tile, confirm it lands in the bag
//     3. USE a consumable      — drink a potion, confirm HP/MP (or its stack count) changes
//     4. EQUIP an item         — equip a bag item, confirm it moves to an equipment slot
//     5. NPC BUY / SELL        — open the Bichon ("0") merchant, sell an item then buy one,
//                                 confirm `state.gold` moves the right way each time
//
// Why a real browser (not a protocol bot): the inventory/character/shop windows
// and the ground-drop / NPC-dialog DOM only exist in the real client, and the
// whole point is to verify the cross-layer wiring (page.tsx handler -> gateway
// BrowserCommand -> ClientPacket -> simulation -> ServerPacket -> state). Each
// mutation is driven through the SAME wired command the window handler sends
// (e.g. `useItem`/`equipItem`/`sellItem` in apps/web/app/page.tsx), and verified
// by the authoritative `window.__mir2Stage5.state` plus the server's own WS
// frames (the ground truth), while the windows are verified by asserting their
// DOM rendered.
//
// This file deliberately reuses the patterns proven in qa-playthrough.mjs /
// capture-web-movement-jitter.mjs (CDP client, chrome launch, login flow, tile
// clicking, combat chase+swing) so it stays consistent with the house QA
// harnesses, with zero new dependencies. It is fully self-contained.
//
// Environment: a backend (gateway+simulation) must be reachable — on a localhost
// origin the client uses the LOCAL gateway (ws://127.0.0.1:7110/ws). A freshly
// created Crystal character starts with 0 gold / no items, and GM seeding
// (@MAKE/@GIVEGOLD) only works on a test-server / GM-authed stack, so on a vanilla
// stack the use/equip/sell/buy beats need an item to first reach the bag (a looted
// drop) — when none is available the harness records that as a low/medium issue
// rather than a hard failure. Each beat is best-effort and adaptive: it reads the
// live state and acts on whatever is actually there.
//
// Usage:
//   node ./scripts/qa-items.mjs --headed [--baseUrl http://127.0.0.1:3023]
//        [--account NAME --password PW] [--createAccount true]
//        [--runId my-run] [--gatewayWs ws://127.0.0.1:7110/ws]
//
// Each beat is best-effort: a failing beat is recorded as an issue and the loop
// keeps going so it captures as much of the lifecycle as possible.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3023";
// The web client resolves the gateway WS from the page origin: on localhost it
// uses ws://127.0.0.1:7110/ws, and honours a `?gatewayWs=` override ONLY on
// localhost (apps/web/app/page.tsx:988 resolveGatewayWebSocketUrl). Pass
// --gatewayWs to point at a different local/hosted backend.
const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS ?? null;
// Load with debug diagnostics enabled (?mir2Debug=1) so the scene-motion and
// readiness hooks are live; append the gateway override when provided.
const RUN_URL = buildRunUrl(baseUrl, gatewayWs);
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultAccountName();
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultCharacterName();
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 280));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "item-qa", `items-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const COMBAT_WINDOW_MS = numberArg(args.combatWindowMs, 30_000); // chase+swing to force a drop
const DROP_POLL_MS = numberArg(args.dropPollMs, 9_000); // wait for a kill to drop loot
const PICKUP_TIMEOUT_MS = numberArg(args.pickupTimeoutMs, 9_000); // walk-to + same-cell PickUp
const MUTATION_TIMEOUT_MS = numberArg(args.mutationTimeoutMs, 6_000); // use/equip/sell/buy reflect
const NPC_DIALOG_TIMEOUT_MS = numberArg(args.npcDialogTimeoutMs, 14_000);

// Hunting fields the pickup beat hops to for a killable wild monster (the Bichon
// spawn town has none). The transfer key map token equals mapFileName ("1"/"2");
// see QUICK_TRANSFER_OPTIONS in apps/web/app/page.tsx:972.
const HUNTING_FIELDS = [
  { map: "1", x: 315, y: 82, label: "Woomyon Woods (1)" },
  { map: "2", x: 503, y: 483, label: "Serpent Valley (2)" },
];
// The Bichon spawn town ("0") — has the merchant NPC(s) (e.g. GTMerchant_Jamie)
// the buy/sell beat needs.
const MERCHANT_TOWN = { map: "0", x: 330, y: 270, label: "Bichon Province (0)" };

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from qa-playthrough.mjs) — captures console, network
// failures, and WebSocket frames (the server's authoritative truth, which the
// buy/sell/pickup beats parse for NPCGoods / SellItem / GainedItem / LoseGold).
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
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-2000);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-2000);
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
    beat.state = await readItemState(ctx.client);
  } catch {
    /* state read is best-effort */
  }
  beat.endedAt = Date.now();
  beat.durationMs = beat.endedAt - beat.startedAt;
  ctx.beats.push(beat);
  return beat;
}

// ---------------------------------------------------------------------------
// In-page state reader — compact, item-centric view of the authoritative client
// state (window.__mir2Stage5.state, apps/web/app/page.tsx:4245). Carries gold,
// HP/MP, every item container, ground drops, and the inventory-window DOM.
// ---------------------------------------------------------------------------
async function readItemState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const entities = Array.isArray(state.entities) ? state.entities : [];
      const monsters = entities.filter((e) => e && e.kind === "monster");
      const npcs = entities.filter((e) => e && e.kind === "npc");
      const drops = Array.isArray(state.groundDrops) ? state.groundDrops : [];
      const compactItem = (it) => it ? {
        key: it.key ?? null, name: it.name ?? null, uniqueId: it.uniqueId ?? null,
        slot: it.slot ?? null, container: it.container ?? null, quantity: it.quantity ?? null,
      } : null;
      const compactEquip = (it) => it ? {
        slot: it.slot ?? null, name: it.name ?? null, attack: it.attack ?? null, defence: it.defence ?? null,
      } : null;
      const inv = Array.isArray(state.inventoryItems) ? state.inventoryItems : [];
      const belt = Array.isArray(state.beltItems) ? state.beltItems : [];
      const equip = Array.isArray(state.equipmentItems) ? state.equipmentItems : [];
      return {
        capturedAt: Date.now(),
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        mapFileName: state.mapFileName ?? null,
        sceneInteractionReady: state.sceneInteractionReady ?? null,
        sceneAssetReadiness: state.sceneAssetReadiness ?? null,
        player: state.player ? { x: state.player.x, y: state.player.y } : null,
        playerObjectId: state.playerObjectId ?? null,
        gold: typeof state.gold === "number" ? state.gold : null,
        playerHp: typeof state.playerHp === "number" ? state.playerHp : null,
        playerMaxHp: typeof state.playerMaxHp === "number" ? state.playerMaxHp : null,
        playerMp: typeof state.playerMp === "number" ? state.playerMp : null,
        playerMaxMp: typeof state.playerMaxMp === "number" ? state.playerMaxMp : null,
        inventoryCount: inv.length,
        beltCount: belt.length,
        equipmentCount: equip.length,
        inventoryItems: inv.slice(0, 60).map(compactItem),
        beltItems: belt.slice(0, 20).map(compactItem),
        equipmentItems: equip.slice(0, 20).map(compactEquip),
        groundDropCount: drops.length,
        groundDrops: drops.slice(0, 30).map((d) => ({ objectId: String(d.objectId), name: d.name ?? null, x: d.x, y: d.y, icon: d.icon ?? null, quantity: d.quantity ?? null })),
        monsterCount: monsters.length,
        npcCount: npcs.length,
        activeNpcDialog: state.activeNpcDialog ? {
          npcObjectId: state.activeNpcDialog.npcObjectId ?? null,
          npcName: state.activeNpcDialog.npcName ?? null,
          linkCount: Array.isArray(state.activeNpcDialog.links) ? state.activeNpcDialog.links.length : null,
        } : null,
        inventoryWindowOpen: Boolean(document.querySelector(".inventory-window")),
        npcDialogDom: Boolean(document.querySelector(".npc-dialog-panel")),
        loadingMapVisible: (document.body?.innerText ?? "").includes("Loading map"),
      };
    })()
  `);
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

// ---------------------------------------------------------------------------
// Movement + interaction primitives (adapted from qa-playthrough.mjs).
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

async function transferTo(client, map, x, y, readyTimeoutMs = SCENE_READY_TIMEOUT_MS) {
  await sendCommand(client, { type: "transferMap", key: `crystal:${map}:${x}:${y}` });
  await waitUntilSoft(
    client,
    `(() => { const s = window.__mir2Stage5?.state; return s?.mapFileName === ${JSON.stringify(map)}; })()`,
    20_000,
  );
  await waitUntilSoft(
    client,
    "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
    readyTimeoutMs,
  );
}

async function readPlayerPos(client) {
  return client.evaluate(
    `(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`,
  );
}

// ---------------------------------------------------------------------------
// Combat primitives — the pickup beat kills a wild monster to force a drop.
// Mirrors qa-playthrough.mjs: monsters do NOT auto-approach (only NPCs do), so we
// walk melee-adjacent first, then swing by clicking the monster's tile
// (handleViewportTileAction -> activateEntity -> attackTarget, page.tsx:6016).
// ---------------------------------------------------------------------------
// The killable wild monster a level-1 character should target: the NEAREST live
// monster that is not a stationary guard (ArcherGuards/Guards have ~9999 HP and
// never die to an unarmed char). Nearest = most likely to stay in range before
// the volatile on-demand pool despawns it; a wounded one (low current hp) breaks
// the tie so we finish what a prior swing started. A fresh unarmed warrior still
// deals >=1 DC/swing (combat.rs:426 floors damage at 1).
async function readWeakestMonster(client) {
  return client.evaluate(`
    (() => {
      const GUARD = /guard|archer|soldier|gate|tower|wall|sabuk|keeper|cannon|gemstone|statue|totem/i;
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const p = s.player ?? { x: 0, y: 0 };
      let mons = ents.filter((e) => e && e.kind === "monster" && !e.dead && !GUARD.test(String(e.name ?? "")));
      if (!mons.length) return null; // only guards / nothing wild here
      const curHp = (m) => (typeof m.hp === "number" ? m.hp : 9e9);
      const dist = (m) => Math.max(Math.abs(m.x - p.x), Math.abs(m.y - p.y));
      // Engage the nearest; among the closest, prefer an already-wounded one.
      mons.sort((a, b) => (dist(a) - dist(b)) || (curHp(a) - curHp(b)));
      const m = mons[0];
      return { objectId: String(m.objectId), name: m.name ?? null, x: m.x, y: m.y, hp: m.hp ?? null, maxHp: m.maxHp ?? null, dead: Boolean(m.dead), candidateCount: mons.length };
    })()
  `);
}

// Walk a short box around the current tile to nudge the on-demand monster pool
// (#83) into spawning WILD monsters near the player when only guards / none are
// in view. Stops the moment a non-guard monster appears so we don't wander away
// from a freshly-spawned target (which would let it despawn).
async function nudgeForSpawns(client) {
  const p = await readPlayerPos(client);
  if (!p) return;
  const legs = [
    { x: p.x + 6, y: p.y + 6 },
    { x: p.x - 6, y: p.y + 6 },
    { x: p.x - 6, y: p.y - 6 },
    { x: p.x + 6, y: p.y - 6 },
  ];
  for (const leg of legs) {
    if (await readWeakestMonster(client)) return; // a wild monster appeared — stop
    if (!(await clickTile(client, leg.x, leg.y, "left"))) {
      await sendCommand(client, { type: "moveTo", x: leg.x, y: leg.y, mode: "run" });
    }
    await delay(1400);
  }
}

async function readMonsterById(client, objectId) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const m = ents.find((e) => e && String(e.objectId) === ${JSON.stringify(String(objectId))});
      if (!m) return null;
      return { objectId: String(m.objectId), name: m.name ?? null, x: m.x, y: m.y, hp: m.hp ?? null, maxHp: m.maxHp ?? null, dead: Boolean(m.dead) };
    })()
  `);
}

// The tile one step in front of the monster on the player's side (clicking the
// monster's OWN tile attacks, it does not walk).
function stepTowardTile(player, monster) {
  const dx = Math.sign(monster.x - player.x);
  const dy = Math.sign(monster.y - player.y);
  return { x: monster.x - dx, y: monster.y - dy };
}

// Chase + swing the given monster until it dies or the window expires. Returns
// { killed, engaged, damaged, startHp, endHp, swings }. A monster that vanishes
// before we ever swing at it is a DESPAWN (the volatile on-demand pool), NOT a
// kill — counting that as a kill was hiding "we never actually fought" behind a
// fake success, so killed requires we engaged (swings > 0).
async function killMonster(client, monster, windowMs) {
  let swings = 0;
  let sawAlive = false;
  const startHp = typeof monster.hp === "number" ? monster.hp : (typeof monster.maxHp === "number" ? monster.maxHp : null);
  let lastHp = startHp;
  let minHp = startHp;
  let diedWhileEngaged = false;
  const deadline = Date.now() + windowMs;
  while (Date.now() < deadline) {
    const m = await readMonsterById(client, monster.objectId);
    if (!m) {
      // Gone: a kill only if we had been hitting it; otherwise it despawned.
      diedWhileEngaged = swings > 0;
      break;
    }
    if (m.dead) {
      diedWhileEngaged = swings > 0;
      break;
    }
    sawAlive = true;
    if (typeof m.hp === "number") {
      lastHp = m.hp;
      if (minHp == null || m.hp < minHp) minHp = m.hp;
    }
    const p = await readPlayerPos(client);
    const chebyshev = p ? Math.max(Math.abs(m.x - p.x), Math.abs(m.y - p.y)) : 99;
    if (chebyshev <= 1) {
      if (!(await clickTile(client, m.x, m.y, "left"))) {
        await sendCommand(client, { type: "attack", objectId: Number(monster.objectId) });
      }
      swings += 1;
      await delay(240); // swing as fast as the server's attack cadence allows
      continue;
    } else if (p) {
      const step = stepTowardTile(p, m);
      if (!(await clickTile(client, step.x, step.y, "left"))) {
        await sendCommand(client, { type: "moveTo", x: step.x, y: step.y, mode: "run" });
      }
    }
    await delay(360);
  }
  const damaged = startHp != null && minHp != null && minHp < startHp;
  return { killed: diedWhileEngaged, engaged: swings > 0, sawAlive, damaged, startHp, endHp: diedWhileEngaged ? 0 : lastHp, minHp, swings };
}

// ---------------------------------------------------------------------------
// Item classification + commands — mirror the wired client handlers so each
// mutation drives the exact same BrowserCommand the window's onClick produces.
// ---------------------------------------------------------------------------

// Port of equipmentSlotForItemKey (apps/web/app/components/original-client-inventory-utils.ts:37):
// the inventory window classifies an item as equip-or-use by this name/key regex.
function equipmentSlotForItemKey(key) {
  const k = String(key ?? "");
  if (/Sword|Dagger|Blade|Axe|Staff|Wand|Bow|Crossbow|Mace/i.test(k)) return "weapon";
  if (/Helmet|Helm/i.test(k)) return "helmet";
  if (/Armour|Armor|Robe|Dress/i.test(k)) return "armour";
  if (/Necklace|Chain/i.test(k)) return "necklace";
  if (/Bracelet|Bangle/i.test(k)) return "braceletLeft";
  if (/Ring/i.test(k)) return "ringLeft";
  if (/Boots|Shoes/i.test(k)) return "boots";
  if (/Belt/i.test(k)) return "belt";
  if (/Amulet|Poison|Charm/i.test(k)) return "amulet";
  if (/Torch|Candle/i.test(k)) return "torch";
  if (/Horse|Mount/i.test(k)) return "mount";
  return null;
}

// Port of equipmentSlotIndex (apps/web/app/page.tsx:11750) — the equip target
// slot index the `equipItem` BrowserCommand carries in its `to` field.
function equipmentSlotIndex(slot) {
  return {
    weapon: 0, armour: 1, helmet: 2, torch: 3, necklace: 4,
    braceletLeft: 5, braceletRight: 6, ringLeft: 7, ringRight: 8,
    amulet: 9, belt: 10, boots: 11, stone: 12, mount: 13,
  }[slot] ?? 0;
}

// A consumable a player drinks (potion/food). Mirrors the inventory action-panel
// classifier (apps/web/app/components/original-client-inventory-action-panels.tsx:309).
function isConsumableItem(item) {
  const hay = `${item?.key ?? ""} ${item?.name ?? ""}`;
  if (equipmentSlotForItemKey(hay)) return false;
  return /Potion|Pill|Drug|Meat|Food|Drink|Apple|Wine|Herb|Antidote|Elixir|Cure/i.test(hay);
}

function gridForContainer(container) {
  return container === "belt" ? "belt" : container === "quest" ? "questInventory" : "inventory";
}

// Drive `useItem` (apps/web/app/page.tsx:5244) — the inventory window's onUseItem.
async function commandUseItem(client, item) {
  return sendCommand(client, {
    type: "useItem",
    key: item.key,
    uniqueId: item.uniqueId,
    slot: item.slot,
    grid: gridForContainer(item.container),
  });
}

// Drive `equipItem` (apps/web/app/page.tsx:5271) — the inventory window's onEquipItem.
async function commandEquipItem(client, item, slot) {
  return sendCommand(client, {
    type: "equipItem",
    uniqueId: item.uniqueId,
    grid: gridForContainer(item.container),
    to: equipmentSlotIndex(slot),
  });
}

// Drive `sellItem` (apps/web/app/page.tsx:5406) — the inventory window's onSellItem.
async function commandSellItem(client, uniqueId, count) {
  return sendCommand(client, { type: "sellItem", uniqueId, count });
}

// Drive the NPC-merchant buy. The gateway maps {type:"buyItem", itemIndex, count,
// panelType} -> ClientPacket::BuyItem (apps/gateway/src/web.rs:2843); and Crystal's
// BuyItem.item_index is actually the GOODS ENTRY's uniqueId, not the template
// index (apps/simulation/src/runtime/tests.rs:24447 passes list[0].unique_id).
async function commandBuyItem(client, goodsUniqueId, count, panelType) {
  return sendCommand(client, { type: "buyItem", itemIndex: goodsUniqueId, count, panelType });
}

// Best-effort GM item seeding (@MAKE <name> [count], gm_commands.rs:159) — a no-op
// for a non-GM/non-test account, exactly like @MOB in qa-playthrough.mjs. Used
// only to improve the odds the equip/use beats have a target; never depended on.
async function seedItemBestEffort(client, name, count = 1) {
  await sendCommand(client, { type: "chat", message: `@MAKE ${name} ${count}` }).catch(() => {});
  await delay(700);
}

// ---------------------------------------------------------------------------
// Inventory-window + NPC-dialog DOM helpers.
// ---------------------------------------------------------------------------

// Open the inventory window via the HUD button the way a player does. The HUD
// SpriteButton reacts to a direct programmatic .click() (no onPointerActivate,
// apps/web/app/components/original-client-overlays.tsx:714). The window root is
// `.inventory-window` (original-client-inventory-window.tsx).
async function openInventoryWindow(client) {
  const already = await client.evaluate(`document.querySelector(".inventory-window") != null`);
  if (already) return true;
  const clicked = await client.evaluate(
    `(() => { const b = document.querySelector(".hud-button.inventory button"); if (!b) return false; b.click(); return true; })()`,
  );
  if (!clicked) return false;
  return waitForSelectorSoft(client, ".inventory-window", 5_000);
}

// Whether the inventory window currently renders a card for this item (its
// `.inventory-item-card` aria-label is the item name, original-client-inventory-window.tsx:692).
// Confirms the item is actually presented in the window before we drive its action.
async function inventoryCardVisible(client, name) {
  return client.evaluate(`
    (() => {
      const cards = Array.from(document.querySelectorAll(".inventory-item-card"));
      return cards.some((c) => (c.getAttribute("aria-label") ?? "") === ${JSON.stringify(String(name ?? ""))});
    })()
  `);
}

// Open (and confirm) an NPC's dialog the way a player would — click its tile so
// the client auto-walks then interacts, with a direct-interact fallback (mirrors
// qa-playthrough.mjs openNpcDialog).
async function openNpcDialog(client, npc, timeoutMs = NPC_DIALOG_TIMEOUT_MS) {
  const clicked = await clickTile(client, npc.x, npc.y, "left");
  if (!clicked) await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  const mine = `window.__mir2Stage5?.state?.activeNpcDialog != null && String(window.__mir2Stage5.state.activeNpcDialog.npcObjectId) === ${JSON.stringify(String(npc.objectId))}`;
  if (await waitUntilSoft(client, mine, timeoutMs)) return true;
  await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  return waitUntilSoft(client, mine, Math.min(4_000, timeoutMs));
}

// Walk up to a visible-but-distant NPC, then open its dialog. A far NPC's tile is
// outside the DOM tile overlay, so clickTile fails and the bare `interact` is
// out of range — the merchant only opens once we are adjacent. Re-reads the NPC's
// live position (it's static, but we close the distance in steps).
async function approachAndOpenNpc(client, npc) {
  const deadline = Date.now() + 20_000;
  while (Date.now() < deadline) {
    const p = await readPlayerPos(client);
    if (!p) break;
    if (Math.max(Math.abs(npc.x - p.x), Math.abs(npc.y - p.y)) <= 1) break; // adjacent
    // Walk to the tile just on our side of the NPC (pathfinding covers the rest).
    const step = stepTowardTile(p, npc);
    if (!(await clickTile(client, step.x, step.y, "left"))) {
      await sendCommand(client, { type: "moveTo", x: step.x, y: step.y, mode: "run" });
    }
    await delay(750);
  }
  return openNpcDialog(client, npc);
}

async function clickNpcDialogLink(client, index) {
  return client.evaluate(
    `(() => { const l = document.querySelectorAll(".npc-dialog-links button, .npc-dialog-links a"); if (!l[${index}]) return false; l[${index}].click(); return true; })()`,
  );
}

// Click the dialog link of a given kind. Links carry `data-link-kind`
// (buy/sell/repair/quest/…, original-client-dialogs.tsx:337,344) so we can hit the
// "Buy" / "Sell" option directly instead of guessing by index.
async function clickNpcDialogLinkByKind(client, kinds) {
  const list = Array.isArray(kinds) ? kinds : [kinds];
  return client.evaluate(`
    (() => {
      const want = ${JSON.stringify(list)};
      const links = Array.from(document.querySelectorAll(".npc-dialog-links button, .npc-dialog-links a"));
      for (const k of want) {
        const hit = links.find((l) => (l.getAttribute("data-link-kind") ?? "") === k);
        if (hit) { hit.click(); return k; }
      }
      return null;
    })()
  `);
}

// Click the dialog link most likely to open the merchant's stock. Crystal general
// stores hang the buy/sell panel off an "@Trade" option (GTMerchant_Jamie's only
// store link is "•trade" -> @trade), which npcLinkKind tags "default" — so kind
// alone misses it. Scores links by target/label and clicks the best trade-ish one
// (preferring @trade/@buy/goods over the Sabuk "@agitbuy" tax buy). Returns the
// clicked target, or null when no trade-ish link exists.
async function clickBestTradeLink(client, skipTargets = []) {
  return client.evaluate(`
    (() => {
      const skip = new Set(${JSON.stringify(skipTargets)});
      const links = Array.from(document.querySelectorAll(".npc-dialog-links button, .npc-dialog-links a"));
      const score = (l) => {
        const s = ((l.getAttribute("data-target") || "") + " " + (l.textContent || "")).toLowerCase();
        if (/@trade\\b|\\btrade\\b/.test(s)) return 6;
        if (/\\bgoods\\b|\\bshop\\b|\\bstore\\b|\\bwares\\b|\\bstock\\b/.test(s)) return 5;
        if (/(^|[^a-z])buy([^a-z]|$)/.test(s) && !/agit/.test(s)) return 4;
        if (/@sell\\b|(^|[^a-z])sell([^a-z]|$)/.test(s)) return 3;
        if (/agitbuy|agittrade|agit/.test(s)) return 1; // Sabuk/guild-hall trade — last resort
        return 0;
      };
      let best = null, bestScore = 0;
      for (const l of links) {
        const target = l.getAttribute("data-target") || "";
        if (skip.has(target)) continue; // don't re-walk a page we already opened
        const sc = score(l);
        if (sc > bestScore) { bestScore = sc; best = l; }
      }
      if (best && bestScore > 0) { best.click(); return (best.getAttribute("data-target") || best.textContent || "").trim().slice(0, 30); }
      return null;
    })()
  `);
}

// Snapshot the current dialog's link labels + kinds (for the report).
async function npcDialogLinks(client) {
  return client.evaluate(`
    (() => Array.from(document.querySelectorAll(".npc-dialog-links button, .npc-dialog-links a"))
      .map((l) => ({ label: (l.textContent ?? "").trim().slice(0, 40), kind: l.getAttribute("data-link-kind") ?? null, target: l.getAttribute("data-target") ?? null })))()
  `);
}

async function npcDialogLinkCount(client) {
  return client.evaluate(`document.querySelectorAll(".npc-dialog-links button, .npc-dialog-links a").length`);
}

function isLikelyMerchant(name) {
  return /merchant|trader|grocer|store|supplies|smith|repair|weapon|armou?r|potion|jamie|gt[_ ]?merchant/i.test(
    String(name ?? ""),
  );
}

// ---------------------------------------------------------------------------
// WebSocket-frame helpers — the server's authoritative truth. The buy/sell beat
// reads the merchant's goods list and confirms purchases from these frames (the
// gateway wraps each ServerPacket as {type:"packet", packet, payload}, camelCase,
// apps/gateway/src/web.rs server_packet_to_event).
// ---------------------------------------------------------------------------
// Parse received packet frames newer than a TIMESTAMP cursor (Date.now()). The
// frame buffer is capped and rolls under heavy object-frame traffic, so an
// index-based `.slice(n)` cursor silently misses frames during a long beat;
// `frame.at` (stamped in the same Node clock as Date.now()) is roll-proof.
function parsePacketFrames(client, sinceTime) {
  const out = [];
  for (const f of client.webSocketFramesReceived) {
    if (typeof sinceTime === "number" && f.at < sinceTime) continue;
    try {
      const o = JSON.parse(f.payloadData);
      if (o && o.packet) out.push(o);
    } catch {
      /* non-JSON frame */
    }
  }
  return out;
}

// The most recent NPCGoods list (Vec<UserItem>, page.tsx case "NPCGoods":7846;
// gateway shape web.rs:3978) seen since `sinceTime`. Each entry has uniqueId +
// itemIndex; buy uses the entry's uniqueId (see commandBuyItem).
function latestNpcGoods(client, sinceTime) {
  const packets = parsePacketFrames(client, sinceTime);
  for (let i = packets.length - 1; i >= 0; i -= 1) {
    if (packets[i].packet === "NPCGoods") {
      const list = Array.isArray(packets[i].payload?.list) ? packets[i].payload.list : [];
      return { list, panelType: packets[i].payload?.panelType ?? 0, rate: packets[i].payload?.rate ?? null };
    }
  }
  return null;
}

function sawPacket(client, sinceTime, packetName, predicate) {
  const packets = parsePacketFrames(client, sinceTime);
  return packets.some((p) => p.packet === packetName && (!predicate || predicate(p.payload ?? {})));
}

async function waitForPacket(client, sinceTime, packetName, timeoutMs, predicate) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (sawPacket(client, sinceTime, packetName, predicate)) return true;
    await delay(150);
  }
  return false;
}

// ---------------------------------------------------------------------------
// Login + enter-world (ported from qa-playthrough.mjs beats 1–5). Keeps the
// phantom-character-slot handling (LoginSuccess characters:[] yet a stale slot
// exists) — only OUR named character is trusted as "server-backed".
// ---------------------------------------------------------------------------
async function loginAndEnterWorld(ctx) {
  const client = ctx.client;

  await runBeat(ctx, "open client + login screen", async () => {
    await navigate(client, RUN_URL);
    await waitUntil(
      client,
      "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
      "client stage ready",
      30_000,
    );
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "login") await waitForSelectorSoft(client, ".login-overlay", 8_000);
  });

  await runBeat(ctx, "register + log in to character select", async (beat) => {
    beat.account = account;
    let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "select" || screen === "game") {
      beat.note = `already past login (screen=${screen})`;
      return;
    }
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    if (createAccount) {
      await click(client, ".login-button.account button");
      await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000);
      await delay(1800);
    }
    await fillInput(client, ".login-input.account", account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.ok button");
    const reached = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'select'", 30_000);
    if (!reached) {
      ctx.addIssue({
        category: "flow",
        severity: "high",
        beat: beat.id,
        summary: "login did not reach character select",
        detail: { account, screen: await client.evaluate("window.__mir2Stage5?.state?.screen ?? null") },
      });
    }
  });

  await runBeat(ctx, "create character + enter world", async (beat) => {
    let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    beat.characterName = characterName;
    if (screen !== "game") {
      const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
      if (!(await client.evaluate(haveOurChar))) {
        await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
        const created = await waitUntilSoft(client, haveOurChar, 15_000);
        if (!created) {
          ctx.addIssue({
            category: "flow",
            severity: "high",
            beat: beat.id,
            summary: "character creation not reflected (newCharacter produced no server-backed character)",
            detail: { characterName },
          });
        }
      }
      const startIndex = await client.evaluate(
        `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
      );
      beat.startIndex = startIndex;
      await sendCommand(client, { type: "startGame", characterIndex: startIndex });
      const entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
      if (!entered) {
        const result = startGameResultFrom(client.webSocketFramesReceived.slice(-40));
        ctx.addIssue({
          category: "flow",
          severity: "high",
          beat: beat.id,
          summary:
            result === null
              ? "startGame sent but never entered the game (no StartGame reply)"
              : `startGame rejected by server (StartGame result=${result})`,
          detail: { startIndex, result },
        });
        throw new Error(`did not enter game (StartGame result=${result})`);
      }
    }

    await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 8_000).catch(() => {});
    const ready = await waitUntilSoft(
      client,
      "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
      SCENE_READY_TIMEOUT_MS,
    );
    if (!ready) {
      ctx.addIssue({
        category: "render",
        severity: "high",
        beat: beat.id,
        summary: "scene never became interaction/asset ready after entering world",
        detail: { mapFileName: (await readItemState(client)).mapFileName },
      });
    }
    await delay(1200);
  });
}

// ---------------------------------------------------------------------------
// The item lifecycle.
// ---------------------------------------------------------------------------
async function itemLifecycle(ctx) {
  const client = ctx.client;

  // Beat — OPEN the inventory window (the foundation the existing loop stops at).
  await runBeat(ctx, "open inventory window", async (beat) => {
    const opened = await openInventoryWindow(client);
    const gridNodes = await client.evaluate(
      `document.querySelectorAll(".inventory-grid, .inventory-item-card").length`,
    );
    beat.inventory = { opened, gridNodes };
    if (!opened) {
      ctx.addIssue({
        category: "ui",
        severity: "high",
        beat: beat.id,
        summary: "clicking the inventory HUD button did not render the inventory window (.inventory-window absent)",
        detail: beat.inventory,
      });
    } else if (!gridNodes) {
      ctx.addIssue({
        category: "ui",
        severity: "medium",
        beat: beat.id,
        summary: "inventory window opened but rendered no grid/item nodes",
        detail: beat.inventory,
      });
    }
  });

  // Beat — PICK UP a ground drop. Hop to a hunting field, kill a wild monster to
  // force loot, then walk onto the drop tile (clickTile -> handleViewportTileAction
  // -> pickGroundDrop, page.tsx:6050) so the client auto-fires PickUp on arrival
  // (Crystal same-cell rule). Verify the item lands in the bag.
  await runBeat(ctx, "pick up a ground drop", async (beat) => {
    const visited = [];
    const kills = [];
    let drop = await firstGroundDrop(client);

    // No drop here yet → go hunt one onto the ground. On each field, nudge the
    // on-demand pool into spawning, then kill the WEAKEST non-guard monster (a
    // level-1 unarmed char dies to guards but can chip a low-HP wild monster). A
    // single kill often drops nothing (Crystal drop RNG), so grind several kills
    // until loot appears, under a global time cap so the beat stays bounded.
    const huntDeadline = Date.now() + numberArg(args.huntCapMs, 150_000);
    for (const field of HUNTING_FIELDS) {
      if (drop || Date.now() > huntDeadline) break;
      const here = await client.evaluate(`window.__mir2Stage5?.state?.mapFileName === ${JSON.stringify(field.map)}`);
      if (!here) {
        await transferTo(client, field.map, field.x, field.y).catch((err) =>
          ctx.addIssue({ category: "flow", severity: "low", beat: beat.id, summary: `transfer to ${field.label} failed`, detail: String(err?.message ?? err) }),
        );
      }
      visited.push(field.label);
      // Best-effort GM spawn of a weak native monster; a no-op for a non-GM
      // account (like @MOB in qa-playthrough). Then walk to trigger real spawns
      // if no WILD (non-guard) monster is already in view.
      await sendCommand(client, { type: "chat", message: "@MOB Deer" }).catch(() => {});
      await delay(800);
      if (!(await readWeakestMonster(client))) await nudgeForSpawns(client);

      // Grind weak monsters on this field until one drops loot or time runs out.
      for (let attempt = 0; attempt < 6 && !drop && Date.now() < huntDeadline; attempt += 1) {
        let monster = await readWeakestMonster(client);
        if (!monster) {
          await nudgeForSpawns(client);
          monster = await readWeakestMonster(client);
        }
        if (!monster) break;
        const outcome = await killMonster(client, monster, COMBAT_WINDOW_MS);
        kills.push({ name: monster.name, maxHp: monster.maxHp, ...outcome });
        if (outcome.killed) {
          // Loot lands a beat after death — poll groundDrops (any new drop).
          drop = await pollForGroundDrop(client, DROP_POLL_MS);
        }
      }
    }
    beat.visitedFields = visited;
    beat.kills = kills;
    const killCount = kills.filter((k) => k.killed).length;
    const engagedCount = kills.filter((k) => k.engaged).length;
    beat.killCount = killCount;
    beat.engagedCount = engagedCount;

    if (!drop) {
      // Distinguish the failure modes so a "no drop" isn't misread as "no monsters".
      const summary = killCount
        ? `killed ${killCount} wild monster(s) but none dropped loot (Crystal drop RNG) — nothing to pick up`
        : engagedCount
          ? `engaged ${engagedCount} monster(s) but finished none in ${COMBAT_WINDOW_MS}ms (level-1 unarmed character)`
          : "no wild monster could be engaged on the hunting fields (only guards / volatile spawns)";
      ctx.addIssue({
        category: killCount ? "item" : "combat",
        severity: killCount ? "low" : "medium",
        beat: beat.id,
        summary,
        detail: { visited, killCount, engagedCount, kills },
      });
      return;
    }
    beat.drop = { objectId: drop.objectId, name: drop.name, at: { x: drop.x, y: drop.y } };

    const before = await readItemState(client);
    const invBefore = before.inventoryCount;
    const goldBefore = before.gold;

    // Walk onto the drop's tile; the client auto-picks on arrival.
    if (!(await clickTile(client, drop.x, drop.y, "left"))) {
      await sendCommand(client, { type: "moveTo", x: drop.x, y: drop.y, mode: "run" });
    }
    const dropGone = await waitUntilSoft(
      client,
      `!(window.__mir2Stage5?.state?.groundDrops ?? []).some((d) => String(d.objectId) === ${JSON.stringify(String(drop.objectId))})`,
      PICKUP_TIMEOUT_MS,
    );
    // Belt-and-suspenders: if standing on it but still present, fire explicit pickups.
    if (!dropGone) {
      await sendCommand(client, { type: "pickUpTile" });
      await sendCommand(client, { type: "pickUp", objectId: Number(drop.objectId) });
      await delay(1200);
    }

    const after = await readItemState(client);
    // Picked up = the bag grew, OR gold grew (gold-pile drop), OR the drop is gone
    // and a GainedItem/GainGold frame confirmed it.
    const invGrew = after.inventoryCount > invBefore;
    const goldGrew = typeof after.gold === "number" && typeof goldBefore === "number" && after.gold > goldBefore;
    const stillOnGround = after.groundDrops.some((d) => String(d.objectId) === String(drop.objectId));
    beat.pickup = { invBefore, invAfter: after.inventoryCount, goldBefore, goldAfter: after.gold, stillOnGround };

    if (!invGrew && !goldGrew && stillOnGround) {
      ctx.addIssue({
        category: "item",
        severity: "high",
        beat: beat.id,
        summary: `walked onto drop "${drop.name ?? drop.objectId}" but it never entered the inventory (still on the ground)`,
        detail: beat.pickup,
      });
    } else if (!invGrew && !goldGrew) {
      ctx.addIssue({
        category: "item",
        severity: "medium",
        beat: beat.id,
        summary: `drop "${drop.name ?? drop.objectId}" left the ground but no inventory/gold change was observed`,
        detail: beat.pickup,
      });
    }
  });

  // Beat — USE a consumable (potion). Drives `useItem` (the window's onUseItem).
  // Run after the combat beat so HP is likely below max → the heal is observable.
  // Success = HP up, OR MP up, OR the stack count dropped (the server consumed it).
  await runBeat(ctx, "use a consumable (potion)", async (beat) => {
    let state = await readItemState(client);
    let consumable = pickConsumable(state);
    if (!consumable) {
      await seedItemBestEffort(client, "HealingPotion", 3);
      state = await readItemState(client);
      consumable = pickConsumable(state);
    }
    if (!consumable) {
      ctx.addIssue({
        category: "item",
        severity: "low",
        beat: beat.id,
        summary: "no consumable in bag/belt to use (none in the starter kit, none looted, GM @MAKE unavailable)",
        detail: { inventory: state.inventoryItems, belt: state.beltItems },
      });
      return;
    }
    beat.consumable = consumable;
    await openInventoryWindow(client);
    beat.cardVisible = await inventoryCardVisible(client, consumable.name).catch(() => null);

    const hpBefore = state.playerHp;
    const mpBefore = state.playerMp;
    const countBefore = consumableCount(state, consumable);
    const wsBefore = Date.now();

    const sent = await commandUseItem(client, consumable);
    beat.sent = sent;
    const changed = await waitUntilSoft(
      client,
      `(() => {
        const s = window.__mir2Stage5?.state ?? {};
        const items = [...(s.inventoryItems ?? []), ...(s.beltItems ?? [])];
        const cur = items.find((it) => it && it.uniqueId === ${Number(consumable.uniqueId)});
        const curCount = cur ? (cur.quantity ?? 1) : 0;
        const hpUp = typeof s.playerHp === "number" && ${hpBefore == null ? "false" : `s.playerHp !== ${Number(hpBefore)}`};
        const mpUp = typeof s.playerMp === "number" && ${mpBefore == null ? "false" : `s.playerMp !== ${Number(mpBefore)}`};
        return hpUp || mpUp || curCount < ${Number(countBefore)};
      })()`,
      MUTATION_TIMEOUT_MS,
    );

    const after = await readItemState(client);
    const countAfter = consumableCount(after, consumable);
    const hpChanged = hpBefore != null && after.playerHp != null && after.playerHp !== hpBefore;
    const mpChanged = mpBefore != null && after.playerMp != null && after.playerMp !== mpBefore;
    const consumed = countAfter < countBefore;
    const sawUse = sawPacket(client, wsBefore, "GainedItem") || sawPacket(client, wsBefore, "UseItem") || sawPacket(client, wsBefore, "RefreshItem");
    beat.use = {
      item: consumable.name,
      hpBefore, hpAfter: after.playerHp, mpBefore, mpAfter: after.playerMp,
      countBefore, countAfter, hpChanged, mpChanged, consumed, sawUse, changed,
    };

    if (!hpChanged && !mpChanged && !consumed) {
      const hpFull = hpBefore != null && state.playerMaxHp != null && hpBefore >= state.playerMaxHp;
      const mpFull = mpBefore != null && state.playerMaxMp != null && mpBefore >= state.playerMaxMp;
      ctx.addIssue({
        category: "item",
        // A potion that does nothing at full HP/MP is correct Crystal behaviour →
        // informational. Otherwise the use path is genuinely broken.
        severity: hpFull && mpFull ? "low" : "medium",
        beat: beat.id,
        summary:
          hpFull && mpFull
            ? `used "${consumable.name}" at full HP/MP — no change (expected; potion not consumed when full)`
            : `used "${consumable.name}" but HP, MP, and stack count were all unchanged (use not reflected)`,
        detail: beat.use,
      });
    }
  });

  // Beat — EQUIP an item. Drives `equipItem` (the window's onEquipItem) for a bag
  // item the client classifies as gear, and verifies it moves to an equip slot.
  await runBeat(ctx, "equip a bag item", async (beat) => {
    let state = await readItemState(client);
    let target = pickEquippable(state);
    if (!target) {
      // Best-effort: GM-make a basic warrior weapon to equip.
      await seedItemBestEffort(client, "WoodenSword", 1);
      state = await readItemState(client);
      target = pickEquippable(state);
    }
    if (!target) {
      ctx.addIssue({
        category: "item",
        severity: "low",
        beat: beat.id,
        summary: "no equippable item in the bag (starter gear already equipped, nothing equippable looted, GM @MAKE unavailable)",
        detail: { inventory: state.inventoryItems, equipment: state.equipmentItems },
      });
      return;
    }
    beat.target = { name: target.item.name, slot: target.slot, uniqueId: target.item.uniqueId };
    await openInventoryWindow(client);
    beat.cardVisible = await inventoryCardVisible(client, target.item.name).catch(() => null);

    const equipBefore = JSON.stringify(state.equipmentItems);
    const equipCountBefore = state.equipmentCount;
    const slotFilledBefore = state.equipmentItems.some((e) => e.slot === target.slot);

    const sent = await commandEquipItem(client, target.item, target.slot);
    beat.sent = sent;

    const moved = await waitUntilSoft(
      client,
      `(() => {
        const s = window.__mir2Stage5?.state ?? {};
        const inv = (s.inventoryItems ?? []).some((it) => it && it.uniqueId === ${Number(target.item.uniqueId)});
        const equip = Array.isArray(s.equipmentItems) ? s.equipmentItems : [];
        const slotNow = equip.some((e) => e && e.slot === ${JSON.stringify(target.slot)});
        // Equipped = it left the bag, OR its target equip slot is now filled.
        return !inv || slotNow;
      })()`,
      MUTATION_TIMEOUT_MS,
    );

    const after = await readItemState(client);
    const stillInBag = after.inventoryItems.some((it) => it.uniqueId === target.item.uniqueId);
    const slotFilledAfter = after.equipmentItems.some((e) => e.slot === target.slot);
    const equipChanged = JSON.stringify(after.equipmentItems) !== equipBefore || after.equipmentCount !== equipCountBefore;
    beat.equip = {
      item: target.item.name, slot: target.slot,
      stillInBag, slotFilledBefore, slotFilledAfter, equipChanged, moved,
    };

    if (stillInBag && !slotFilledAfter && !equipChanged) {
      ctx.addIssue({
        category: "item",
        severity: "high",
        beat: beat.id,
        summary: `equip of "${target.item.name}" did not move it to the ${target.slot} slot (still in bag, equipment unchanged)`,
        detail: beat.equip,
      });
    }
  });

  // Beat — NPC BUY / SELL at the Bichon merchant. Open the merchant's Buy/Sell
  // panel (its dialog links trigger NPCGoods/NPCSell, npc.rs:629), then SELL a bag
  // item (gold up) and BUY one back from the goods list (gold down). Sell/Buy both
  // require an in-range open merchant service (sell_item_impl, npc.rs:977).
  await runBeat(ctx, "NPC buy + sell (gold changes)", async (beat) => {
    // Return to the town that has the merchant.
    const here = await client.evaluate(`window.__mir2Stage5?.state?.mapFileName === ${JSON.stringify(MERCHANT_TOWN.map)}`);
    if (!here) {
      await transferTo(client, MERCHANT_TOWN.map, MERCHANT_TOWN.x, MERCHANT_TOWN.y).catch((err) =>
        ctx.addIssue({ category: "flow", severity: "low", beat: beat.id, summary: "transfer to merchant town failed", detail: String(err?.message ?? err) }),
      );
    }
    // Close the inventory window so it doesn't obscure NPC clicks.
    await client.evaluate(`(() => { const w = document.querySelector(".inventory-window"); if (w) { const b = document.querySelector(".hud-button.inventory button"); if (b) b.click(); } })()`).catch(() => {});
    await delay(400);

    const state = await readItemState(client);
    const merchants = await listMerchantCandidates(client);
    beat.merchantCandidates = merchants.map((m) => m.name);
    if (!merchants.length) {
      ctx.addIssue({
        category: "item",
        severity: "medium",
        beat: beat.id,
        summary: "no merchant-like NPC found on the spawn town to exercise buy/sell",
        detail: { mapFileName: state.mapFileName, npcCount: state.npcCount },
      });
      return;
    }

    // Open a merchant and its Buy/Sell service: click the NPC, then click the
    // "Buy" (or any) dialog link until the server sends NPCGoods. Trigger the
    // "Sell" link too so the sell service is active (sell_item_impl, npc.rs:977).
    let opened = null;
    let goods = null;
    let openedSell = false;
    const triedMerchants = [];
    for (const npc of merchants) {
      if (goods) break;
      const live = await reachMerchant(client, npc);
      if (!live) {
        triedMerchants.push({ name: npc.name, opened: false });
        continue;
      }
      opened = live;
      const wsAt = Date.now();
      const liveLinks = await npcDialogLinks(client).catch(() => []);
      beat.dialogLinks = liveLinks;
      // Each pass: prefer a kind-classified buy/sell link, else the best trade-ish
      // link (@trade/goods/buy — the general-store entry kind classification
      // misses), else walk by index. @Trade may open the panel directly or a
      // sub-page whose Buy/Sell links the next pass then clicks.
      const tried = [];
      for (let pass = 0; pass < 8 && !goods; pass += 1) {
        const wsLink = Date.now();
        let what = await clickNpcDialogLinkByKind(client, ["buy", "sell"]);
        if (what === "sell") openedSell = true;
        if (!what) what = await clickBestTradeLink(client, tried); // skip pages already walked
        if (!what) what = (await clickNpcDialogLink(client, pass)) ? `index:${pass}` : null;
        if (!what) break;
        tried.push(what);
        await waitForPacket(client, wsLink, "NPCGoods", 2500).catch(() => false);
        const g = latestNpcGoods(client, wsAt);
        if (g) goods = g;
        if (sawPacket(client, wsAt, "NPCSell")) openedSell = true;
        if (!(await client.evaluate(`window.__mir2Stage5?.state?.activeNpcDialog != null`)) && !goods) {
          await openNpcDialog(client, live, 4_000); // a sub-page closed the dialog → re-open
        }
      }
      beat.triedLinkKinds = tried;
      triedMerchants.push({ name: live.name, opened: true, links: liveLinks, triedLinkKinds: tried, reachedGoods: Boolean(goods), openedSell });
      // If we got a buy panel but no sell service yet, click the Sell link too.
      if (goods && !openedSell && (await client.evaluate(`window.__mir2Stage5?.state?.activeNpcDialog != null`))) {
        const k = await clickNpcDialogLinkByKind(client, ["sell"]);
        if (k) openedSell = true;
      }
    }
    beat.triedMerchants = triedMerchants;
    beat.merchant = opened?.name ?? null;
    beat.goodsCount = goods?.list?.length ?? 0;
    beat.openedSell = openedSell;

    if (!opened) {
      ctx.addIssue({
        category: "item",
        severity: "medium",
        beat: beat.id,
        summary: "could not open any merchant NPC's dialog on the spawn town",
        detail: { merchants: beat.merchantCandidates, tried: triedMerchants },
      });
      return;
    }
    if (!goods) {
      ctx.addIssue({
        category: "item",
        severity: "medium",
        beat: beat.id,
        summary: `merchant "${opened.name}" opened a dialog but no Buy/Sell goods panel (NPCGoods) was reached via its links`,
        detail: { merchant: opened.name, dialogLinks: beat.dialogLinks, triedLinkKinds: beat.triedLinkKinds, tried: triedMerchants },
      });
    }

    // ── SELL ────────────────────────────────────────────────────────────────
    const sellState = await readItemState(client);
    const sellTarget = pickSellable(sellState);
    if (!sellTarget) {
      ctx.addIssue({ category: "item", severity: "low", beat: beat.id, summary: "no sellable bag item to sell", detail: { inventory: sellState.inventoryItems } });
    } else {
      const goldBefore = sellState.gold;
      const wsBefore = Date.now();
      await commandSellItem(client, sellTarget.uniqueId, 1);
      const sold = await waitUntilSoft(
        client,
        `(() => { const g = window.__mir2Stage5?.state?.gold; return typeof g === "number" && ${goldBefore == null ? "false" : `g > ${Number(goldBefore)}`}; })()`,
        MUTATION_TIMEOUT_MS,
      );
      const afterSell = await readItemState(client);
      const sellOk = sawPacket(client, wsBefore, "SellItem", (p) => p.success === true);
      const goldUp = typeof afterSell.gold === "number" && typeof goldBefore === "number" && afterSell.gold > goldBefore;
      beat.sell = { item: sellTarget.name, uniqueId: sellTarget.uniqueId, goldBefore, goldAfter: afterSell.gold, goldUp, sellOk, sold };
      if (!goldUp && !sellOk) {
        ctx.addIssue({
          category: "item",
          severity: "high",
          beat: beat.id,
          summary: `sold "${sellTarget.name}" but gold did not increase and no SellItem(success) frame arrived`,
          detail: beat.sell,
        });
      }
    }

    // ── BUY ─────────────────────────────────────────────────────────────────
    // Refresh the goods list (selling adds a buy-back entry); buy the cheapest /
    // first affordable item, retrying across a few entries since prices aren't in
    // the packet (server refuses an unaffordable buy with no gold change).
    const freshGoods = latestNpcGoods(client, 0) ?? goods;
    if (!freshGoods || !freshGoods.list.length) {
      ctx.addIssue({ category: "item", severity: "medium", beat: beat.id, summary: "no goods list available to buy from", detail: { merchant: opened.name } });
      return;
    }
    const buyState = await readItemState(client);
    const goldBefore = buyState.gold;
    const invBefore = buyState.inventoryCount;
    const wsBefore = Date.now();
    let bought = false;
    const tried = [];
    for (const good of freshGoods.list.slice(0, 6)) {
      const uniqueId = good.uniqueId ?? good.unique_id;
      if (uniqueId == null) continue;
      await commandBuyItem(client, uniqueId, 1, freshGoods.panelType ?? 0);
      const ok = await waitUntilSoft(
        client,
        `(() => {
          const s = window.__mir2Stage5?.state ?? {};
          const g = s.gold;
          const goldDown = typeof g === "number" && ${goldBefore == null ? "false" : `g < ${Number(goldBefore)}`};
          const invUp = (s.inventoryItems ?? []).length > ${Number(invBefore)};
          return goldDown || invUp;
        })()`,
        3500,
      );
      tried.push({ uniqueId, itemIndex: good.itemIndex ?? good.item_index ?? null, ok });
      if (ok) {
        bought = true;
        break;
      }
    }
    const afterBuy = await readItemState(client);
    const goldDown = typeof afterBuy.gold === "number" && typeof goldBefore === "number" && afterBuy.gold < goldBefore;
    const gained = sawPacket(client, wsBefore, "GainedItem") || sawPacket(client, wsBefore, "LoseGold");
    beat.buy = { goldBefore, goldAfter: afterBuy.gold, invBefore, invAfter: afterBuy.inventoryCount, goldDown, gained, bought, tried };
    if (!goldDown && !gained) {
      ctx.addIssue({
        category: "item",
        // If we never reached a goods panel, this is downstream of that; keep it
        // medium (could be pure affordability with 0 gold).
        severity: goods ? "high" : "medium",
        beat: beat.id,
        summary: "bought from the merchant but gold did not decrease and no GainedItem/LoseGold frame arrived",
        detail: beat.buy,
      });
    }
  });

  // Beat — wrap: final snapshot for the report.
  await runBeat(ctx, "wrap + final item snapshot", async (beat) => {
    beat.final = await readItemState(client);
  });
}

// ---------------------------------------------------------------------------
// Item selection helpers (read from the authoritative state).
// ---------------------------------------------------------------------------
async function firstGroundDrop(client) {
  const drops = await client.evaluate(`
    (() => {
      const d = window.__mir2Stage5?.state?.groundDrops ?? [];
      return d.map((x) => ({ objectId: String(x.objectId), name: x.name ?? null, x: x.x, y: x.y, icon: x.icon ?? null }));
    })()
  `);
  return Array.isArray(drops) && drops.length ? drops[0] : null;
}

async function pollForGroundDrop(client, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const drop = await firstGroundDrop(client).catch(() => null);
    if (drop) return drop;
    await delay(500);
  }
  return null;
}

function pickConsumable(state) {
  const pool = [...(state.inventoryItems ?? []), ...(state.beltItems ?? [])];
  return pool.find((it) => it && isConsumableItem(it)) ?? null;
}

function consumableCount(state, consumable) {
  const pool = [...(state.inventoryItems ?? []), ...(state.beltItems ?? [])];
  const cur = pool.find((it) => it && it.uniqueId === consumable.uniqueId);
  return cur ? (cur.quantity ?? 1) : 0;
}

function pickEquippable(state) {
  for (const it of state.inventoryItems ?? []) {
    const slot = equipmentSlotForItemKey(`${it.key ?? ""} ${it.name ?? ""}`);
    if (slot) return { item: it, slot };
  }
  return null;
}

// A bag item safe to sell — not on the quest tab, has a uniqueId. Prefer a
// stackable consumable so we never sell looted gear we still want to equip.
function pickSellable(state) {
  const bag = (state.inventoryItems ?? []).filter((it) => it && it.container !== "quest" && it.uniqueId != null);
  return bag.find((it) => isConsumableItem(it)) ?? bag[0] ?? null;
}

// Re-resolve an NPC by name nearest the player (its objectId/coords refresh after
// a transfer). Returns the live entity or null.
async function resolveNpcByName(client, name) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const p = s.player ?? { x: 0, y: 0 };
      const cand = ents.filter((e) => e && e.kind === "npc" && (e.name ?? "") === ${JSON.stringify(String(name ?? ""))})
        .map((e) => ({ objectId: String(e.objectId), name: e.name ?? null, x: e.x, y: e.y, d: Math.max(Math.abs(e.x - p.x), Math.abs(e.y - p.y)) }))
        .sort((a, b) => a.d - b.d);
      return cand[0] ?? null;
    })()
  `);
}

// Get adjacent to a merchant the reliable way: teleport one tile off it
// (transferMap honours arbitrary in-map coords, as the field hops show), re-resolve
// it by name, then open its dialog with a short timeout. Tries a few adjacent
// offsets (the south tile may be blocked); a single late walk-in is the fallback.
// Returns the live NPC whose dialog opened, or null. Kept fast so the beat doesn't
// spend minutes — same-map teleports settle quickly (8s ready cap).
async function reachMerchant(client, npc) {
  for (const off of [{ dx: 0, dy: 1 }, { dx: 1, dy: 0 }, { dx: -1, dy: 0 }]) {
    await transferTo(client, "0", npc.x + off.dx, npc.y + off.dy, 8_000).catch(() => {});
    await delay(400);
    const fresh = (await resolveNpcByName(client, npc.name)) ?? npc;
    if (await openNpcDialog(client, fresh, 3_500)) return fresh;
  }
  // Last resort: walk in from wherever we landed.
  const fresh = (await resolveNpcByName(client, npc.name)) ?? npc;
  return (await approachAndOpenNpc(client, fresh)) ? fresh : null;
}

async function listMerchantCandidates(client) {
  const npcs = await client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const p = s.player ?? { x: 0, y: 0 };
      return ents.filter((e) => e && e.kind === "npc")
        .map((e) => ({ objectId: String(e.objectId), name: e.name ?? null, x: e.x, y: e.y, d: Math.abs(e.x - p.x) + Math.abs(e.y - p.y) }))
        .sort((a, b) => a.d - b.d);
    })()
  `);
  if (!Array.isArray(npcs)) return [];
  // Merchant-likely first, then any NPC as a fallback (the script may name the
  // merchant differently); cap to a few so the beat stays bounded (each one costs
  // a teleport + dialog probe).
  const ranked = [...npcs].sort((a, b) => (isLikelyMerchant(b.name) ? 1 : 0) - (isLikelyMerchant(a.name) ? 1 : 0));
  return ranked.slice(0, 3);
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
    const severity = kind === "runtime" ? "high" : kind === "ui" || kind === "map" || kind === "entity" ? "medium" : "low";
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
      { sent: ctx.client.webSocketFramesSent.slice(-200), received: ctx.client.webSocketFramesReceived.slice(-200) },
      null,
      2,
    ),
  );

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  const md = [];
  md.push(`# Item-lifecycle QA report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\`  ·  account \`${account}\`  ·  character \`${characterName}\``);
  md.push(`- Beats: ${summary.beatsOk}/${summary.beats} ok  ·  Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push(`- By category: ${Object.entries(byCategory).map(([k, v]) => `${k} ${v}`).join(", ") || "none"}`);
  md.push("");
  md.push("## Issues (by severity)");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues detected — the full item lifecycle worked._");
  } else {
    md.push("| # | Severity | Category | Beat | Summary |");
    md.push("|---|---|---|---|---|");
    for (const i of sortedIssues) {
      md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.beat ?? "—"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    }
  }
  md.push("");
  md.push("## Lifecycle (beats)");
  md.push("");
  for (const b of ctx.beats) {
    md.push(`### Beat ${b.id} — ${b.title} ${b.ok ? "✅" : "❌"}  (${b.durationMs}ms)`);
    if (b.error) md.push(`- error: \`${b.error.slice(0, 300)}\``);
    if (b.note) md.push(`- note: ${b.note}`);
    if (b.state) {
      md.push(
        `- state: screen=\`${b.state.screen}\` map=\`${b.state.mapFileName ?? "?"}\` gold=${b.state.gold ?? "—"} hp=${b.state.playerHp ?? "—"}/${b.state.playerMaxHp ?? "—"} bag=${b.state.inventoryCount} equip=${b.state.equipmentCount} drops=${b.state.groundDropCount}`,
      );
    }
    if (b.pickup) md.push(`- pickup: ${JSON.stringify(b.pickup)}`);
    if (b.use) md.push(`- use: ${JSON.stringify(b.use)}`);
    if (b.equip) md.push(`- equip: ${JSON.stringify(b.equip)}`);
    if (b.sell) md.push(`- sell: ${JSON.stringify(b.sell)}`);
    if (b.buy) md.push(`- buy: ${JSON.stringify(b.buy)}`);
    if (b.inventory) md.push(`- inventory: ${JSON.stringify(b.inventory)}`);
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
  md.push(`cd apps/web && node ./scripts/qa-items.mjs --headed --baseUrl ${baseUrl}${gatewayWs ? ` --gatewayWs ${gatewayWs}` : ""}`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-items-${process.pid}-${Date.now()}`);
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
      `(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, wsState: window.__mir2Stage5?.state?.wsState ?? null, body: document.body?.innerText?.slice(0, 300) ?? "" }))()`,
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
function buildRunUrl(base, gateway) {
  const sep = base.includes("?") ? "&" : "?";
  let url = `${base}${sep}mir2Debug=1`;
  // The client honours ?gatewayWs= only on a localhost origin (page.tsx:995).
  if (gateway) url += `&gatewayWs=${encodeURIComponent(gateway)}`;
  return url;
}

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
  // Resource-load 404s (map decor / sprite-lib meta / sounds not served from local
  // dev or a stale R2 release) are already collected + grouped by the network
  // collector; don't also flood the console list with one entry per missing URL.
  if (/Failed to load resource/i.test(text) && /\b(404|Not Found)\b/i.test(text)) return false;
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
  return `IQ${suffix}`.slice(0, 10).toUpperCase();
}

function defaultAccountName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `IQ${suffix}`.slice(0, 12).toUpperCase();
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
  console.log(`qa-items: target=${baseUrl} account=${account} char=${characterName} headed=${headed}${gatewayWs ? ` gatewayWs=${gatewayWs}` : ""}`);
  console.log(`qa-items: output=${outputDir}`);

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

    await loginAndEnterWorld(ctx);
    await itemLifecycle(ctx);

    collectNetworkIssues(ctx);
    collectConsoleIssues(ctx);
    await writeReport(ctx);

    const s = ctx.issues.reduce((acc, i) => ((acc[i.severity] = (acc[i.severity] ?? 0) + 1), acc), {});
    console.log(`\nqa-items done: ${ctx.beats.filter((b) => b.ok).length}/${ctx.beats.length} beats ok, ${ctx.issues.length} issues (high ${s.high ?? 0}, medium ${s.medium ?? 0}, low ${s.low ?? 0}).`);
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
  } catch (error) {
    console.error("qa-items fatal:", error);
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
