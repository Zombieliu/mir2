// qa-economy.mjs — AI-driven "real player" QA loop for the ITEM/GOLD ECONOMY:
// STORAGE (bank), MAIL, and MARKET (auction). Sibling to qa-items.mjs (which
// covers the item lifecycle); this harness exercises the three "where do items
// and gold go" subsystems end-to-end the way a player does and records every
// broken step into a structured report under docs/generated/economy-qa/:
//
//   log in -> create chars A + B -> enter world as A ->
//     1. SETUP            — qa.giveItem-seed a few bag items; best-effort earn a
//                            little gold by selling one at a town merchant.
//     2. STORAGE / bank   — open storage (qa.openStorage), SET a password, re-lock
//                            and UNLOCK with it, DEPOSIT a bag item then WITHDRAW
//                            it, asserting the bag/warehouse slot counts move.
//     3. MARKET / auction — CONSIGN (list) a bag item, BROWSE it (refresh+search)
//                            in the Market window, attempt a BUY, then reclaim it.
//     4. MAIL (self)      — send a letter (+gold) to our OWN name (an immediate,
//                            fully-observable round-trip), then read + claim it.
//     5. MAIL (to char B) — send a letter (+gold) to the SECOND character, switch
//                            to B (re-login), and receive + claim it on B.
//
// Why a real browser (not a protocol bot): the storage/mail/market windows, the
// inventory-grid DOM, and the cross-layer wiring (page.tsx handler -> gateway
// BrowserCommand -> ClientPacket -> simulation -> ServerPacket -> world state)
// only exist in the real client. Each mutation is driven through the SAME wired
// command the window's button sends (e.g. `storeItem`/`unlockStorage`/`sendMail`/
// `marketBuy` in apps/web/app/page.tsx), and verified by the authoritative
// `window.__mir2Stage5.state` plus the server's own WS frames (the ground truth),
// while the windows are verified by asserting their DOM rendered.
//
// Environment / gates (documented so a "no-op" beat is read correctly):
//   * A backend (gateway+simulation) must be reachable. On a localhost origin the
//     client uses the LOCAL gateway (ws://127.0.0.1:7110/ws); pass --gatewayWs to
//     point elsewhere (honoured only on a localhost origin, page.tsx:995).
//   * A freshly created Crystal character starts with 0 gold / no items. This
//     harness seeds items through the NON-gated stage5 QA hook `qa.giveItem`
//     (apps/simulation/src/runtime/stage5.rs:272) — unlike GM @MAKE/@GIVEGOLD,
//     it works on a vanilla (non-test-server) stack. Storage is opened the same
//     way via `qa.openStorage`. Gold has no QA faucet, so the gold attachment is
//     best-effort: it is only non-zero if a merchant sale (beat 1) earned some.
//   * Known wiring gaps this harness documents (not harness bugs): mail item
//     attachment is unwired (sendMailMessage hardcodes itemsIdx zeros and the mail
//     window mount passes no attachableItems prop) — mail can attach gold only;
//     the market window's onList/onBid are not mounted, so consign/bid are driven
//     through the underlying BrowserCommand; and a fresh single session can only
//     see its OWN auction listings, so a self-buy is correctly rejected
//     (MarketFail reason 6) rather than completing.
//
// Each beat is best-effort and adaptive: a failing beat is recorded as an issue
// and the loop keeps going so it captures as much of the economy as possible.
//
// Usage:
//   node ./scripts/qa-economy.mjs --headed [--baseUrl http://127.0.0.1:3023]
//        [--account NAME --password PW] [--createAccount true]
//        [--runId my-run] [--gatewayWs ws://127.0.0.1:7110/ws] [--seedGold false]

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3023";
const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS ?? null;
const RUN_URL = buildRunUrl(baseUrl, gatewayWs);
const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultAccountName();
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultCharacterName("A");
const characterNameB = args.characterNameB ?? defaultCharacterName("B");
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const seedGold = booleanArg(args.seedGold ?? process.env.MIR2_QA_SEED_GOLD, true);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 280));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "economy-qa", `economy-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const MUTATION_TIMEOUT_MS = numberArg(args.mutationTimeoutMs, 6_000); // command reflects in state
const WS_WAIT_MS = numberArg(args.wsWaitMs, 4_500); // wait for an authoritative frame
const NPC_DIALOG_TIMEOUT_MS = numberArg(args.npcDialogTimeoutMs, 14_000);
const GOLD_FAUCET_CAP_MS = numberArg(args.goldFaucetCapMs, 60_000);

// Items seeded through qa.giveItem (NOT GM): kebab keys -> title-cased names
// server-side (stage5_item_name). A potion is a clean consumable; iron-sword is
// the same key Crystal's seeded auction uses, so it consigns/sells cleanly.
const SEED_ITEMS = [
  { key: "blue-potion", count: 5 },
  { key: "red-potion", count: 3 },
  { key: "iron-sword", count: 1 },
];

// The Bichon spawn town ("0") has the merchant NPC(s) the gold faucet sells to.
const MERCHANT_TOWN = { map: "0", x: 330, y: 270, label: "Bichon Province (0)" };

const STORAGE_PASSWORD = "vault1234";

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from qa-items.mjs) — captures console, network failures,
// and WebSocket frames (the server's authoritative truth; the storage/mail/market
// beats parse these for StoreItem / StoragePasswordResult / MailSent /
// ParcelCollected / ConsignItem / MarketFail / NPCMarket).
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
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-3000);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-3000);
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
    beat.state = await readEconomyState(ctx.client);
  } catch {
    /* state read is best-effort */
  }
  beat.endedAt = Date.now();
  beat.durationMs = beat.endedAt - beat.startedAt;
  ctx.beats.push(beat);
  return beat;
}

// ---------------------------------------------------------------------------
// In-page state reader — economy-centric view of the authoritative client state
// (window.__mir2Stage5.state, apps/web/app/page.tsx:4245). Carries gold/credit,
// every item container (bag/storage), the storage password flags, and the
// stage5 mail + auction slices, plus the window DOM presence flags.
// ---------------------------------------------------------------------------
async function readEconomyState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const num = (v) => (typeof v === "number" ? v : null);
      const compactItem = (it) => it ? {
        key: it.key ?? null, name: it.name ?? null, uniqueId: it.uniqueId ?? null,
        slot: it.slot ?? null, container: it.container ?? null, quantity: it.quantity ?? null,
      } : null;
      const inv = Array.isArray(state.inventoryItems) ? state.inventoryItems : [];
      const storage = Array.isArray(state.storageItems) ? state.storageItems : [];
      const belt = Array.isArray(state.beltItems) ? state.beltItems : [];
      const equip = Array.isArray(state.equipmentItems) ? state.equipmentItems : [];
      const stage5 = state.stage5Systems ?? {};
      const mailRaw = Array.isArray(stage5.mail) ? stage5.mail : [];
      const auctionRaw = Array.isArray(stage5.auction) ? stage5.auction : [];
      const mail = mailRaw.map((m) => ({
        id: m.mailId ?? m.id ?? m.mail_id ?? null,
        from: m.senderName ?? m.sender_name ?? m.from ?? null,
        to: m.to ?? null,
        gold: num(m.gold) ?? 0,
        itemCount: typeof m.itemCount === "number" ? m.itemCount : (Array.isArray(m.items) ? m.items.length : 0),
        opened: m.opened === true,
        collected: m.collected === true || m.claimed === true,
        locked: m.locked === true,
        deleted: m.deleted === true,
      }));
      const auction = auctionRaw.map((a) => ({
        id: a.id ?? a.auctionId ?? a.listingId ?? a.uniqueId ?? null,
        seller: a.seller ?? null,
        itemName: a.itemName ?? a.name ?? a.item_key ?? a.itemKey ?? null,
        price: num(a.price) ?? null,
        sold: a.sold === true,
        cancelled: a.cancelled === true,
        auction: a.auction === true,
      }));
      const characters = Array.isArray(state.characters)
        ? state.characters.map((c) => ({ index: c.index ?? null, name: c.name ?? null }))
        : [];
      return {
        capturedAt: Date.now(),
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        mapFileName: state.mapFileName ?? null,
        sceneInteractionReady: state.sceneInteractionReady ?? null,
        sceneAssetReadiness: state.sceneAssetReadiness ?? null,
        player: state.player ? { x: state.player.x, y: state.player.y } : null,
        playerObjectId: state.playerObjectId ?? null,
        gold: num(state.gold),
        credit: num(state.credit),
        inventoryCount: inv.length,
        inventoryItems: inv.slice(0, 60).map(compactItem),
        beltCount: belt.length,
        equipmentCount: equip.length,
        storageCount: storage.length,
        storageItems: storage.slice(0, 60).map(compactItem),
        hasStoragePassword: state.hasStoragePassword === true,
        requireStoragePassword: state.requireStoragePassword === true,
        storageSessionUnlocked: state.storageSessionUnlocked === true,
        hasExpandedStorage: state.hasExpandedStorage === true,
        storagePasswordLastSetBinaryDatetime: state.storagePasswordLastSetBinaryDatetime ?? null,
        mailCount: mail.length,
        mail: mail.slice(0, 30),
        auctionCount: auction.length,
        auction: auction.slice(0, 30),
        characters,
        activeInventoryTab: state.activeInventoryTab ?? null,
        npcCount: Array.isArray(state.entities) ? state.entities.filter((e) => e && e.kind === "npc").length : 0,
        inventoryWindowOpen: Boolean(document.querySelector(".inventory-window")),
        storagePanelOpen: Boolean(document.querySelector(".storage-password-panel, .inventory-storage-panel")),
        marketWindowOpen: Boolean(document.querySelector("[data-market-mode]")),
        mailWindowOpen: Boolean(document.querySelector("[data-mail-view]")),
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
// Command drivers — each mirrors the wired client handler so the mutation drives
// the exact same BrowserCommand the window's onClick produces (apps/web/app/page.tsx).
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

// stage5 action channel (page.tsx runStage5Command). The qa.* hooks are NOT
// test-server-gated (stage5.rs dispatch), so seeding/opening works on any stack.
async function runStage5Command(client, action, commandArgs = []) {
  return sendCommand(client, { type: "stage5Command", action, args: commandArgs.map(String) });
}
async function qaGiveItem(client, key, count = 1) {
  return runStage5Command(client, "qa.giveItem", [key, String(count)]);
}
async function qaOpenStorage(client) {
  return runStage5Command(client, "qa.openStorage", []);
}

// Storage (page.tsx storeItem/takeBackItem/unlockStorage/setStoragePassword).
async function commandStoreItem(client, fromSlot, toSlot) {
  return sendCommand(client, { type: "storeItem", from: fromSlot, to: toSlot });
}
async function commandTakeBackItem(client, fromSlot, toSlot) {
  return sendCommand(client, { type: "takeBackItem", from: fromSlot, to: toSlot });
}
async function commandSetStoragePassword(client, currentPassword, newPassword) {
  return sendCommand(client, { type: "setStoragePassword", currentPassword, newPassword });
}
async function commandUnlockStorage(client, storagePassword) {
  return sendCommand(client, { type: "unlockStorage", password: storagePassword });
}

// Market (page.tsx marketBuyListing/marketCancelListing/marketSearch/marketRefresh
// + the unmounted-in-window ConsignItem BrowserCommand, web.rs:894).
async function commandConsignItem(client, uniqueId, price, marketType = 0) {
  return sendCommand(client, { type: "consignItem", uniqueId, price, marketType });
}
async function commandMarketRefresh(client) {
  return sendCommand(client, { type: "marketRefresh" });
}
async function commandMarketSearch(client, matchText) {
  return sendCommand(client, { type: "marketSearch", matchText });
}
async function commandMarketBuy(client, auctionId, bidPrice = 0) {
  return sendCommand(client, { type: "marketBuy", auctionId, bidPrice });
}
async function commandMarketGetBack(client, auctionId, mode = 0) {
  return sendCommand(client, { type: "marketGetBack", mode, auctionId });
}

// Mail (page.tsx sendMailMessage/openMailMessage/claimMailAttachment). itemsIdx is
// always zeros — exactly what the wired sendMailMessage sends (item attach unwired).
async function commandSendMail(client, name, message, gold = 0) {
  return sendCommand(client, {
    type: "sendMail",
    name,
    message,
    gold: Math.max(0, Math.floor(gold)),
    itemsIdx: [0, 0, 0, 0, 0],
    stamped: false,
  });
}
async function commandReadMail(client, mailId) {
  return sendCommand(client, { type: "readMail", mailId });
}
async function commandCollectParcel(client, mailId) {
  return sendCommand(client, { type: "collectParcel", mailId });
}

// Inventory NPC sell (page.tsx sellItem) — the gold faucet uses this.
async function commandSellItem(client, uniqueId, count) {
  return sendCommand(client, { type: "sellItem", uniqueId, count });
}
async function commandBuyItem(client, goodsUniqueId, count, panelType) {
  return sendCommand(client, { type: "buyItem", itemIndex: goodsUniqueId, count, panelType });
}

// ---------------------------------------------------------------------------
// Window openers / DOM helpers.
// ---------------------------------------------------------------------------
// The Market (Alt+M) and Mail (Alt+L) windows toggle from a window-level keydown
// listener that requires `event.altKey` (page.tsx:1589). Dispatch the synthetic
// event only when the window isn't already open (the hotkey is a toggle).
async function dispatchAltHotkey(client, letter) {
  return client.evaluate(`
    (() => {
      window.dispatchEvent(new KeyboardEvent("keydown", {
        key: ${JSON.stringify(letter)},
        code: "Key" + ${JSON.stringify(letter.toUpperCase())},
        altKey: true, bubbles: true,
      }));
      return true;
    })()
  `);
}
async function openMarketWindow(client) {
  if (await client.evaluate(`document.querySelector("[data-market-mode]") != null`)) return true;
  await dispatchAltHotkey(client, "m");
  return waitForSelectorSoft(client, "[data-market-mode]", 5_000);
}
async function openMailWindow(client) {
  if (await client.evaluate(`document.querySelector("[data-mail-view]") != null`)) return true;
  await dispatchAltHotkey(client, "l");
  return waitForSelectorSoft(client, "[data-mail-view]", 5_000);
}
async function closeWindowIfOpen(client, selector, letter) {
  if (await client.evaluate(`document.querySelector(${JSON.stringify(selector)}) != null`)) {
    await dispatchAltHotkey(client, letter);
    await delay(250);
  }
}
async function openInventoryWindow(client) {
  if (await client.evaluate(`document.querySelector(".inventory-window") != null`)) return true;
  const clicked = await client.evaluate(
    `(() => { const b = document.querySelector(".hud-button.inventory button"); if (!b) return false; b.click(); return true; })()`,
  );
  if (!clicked) return false;
  return waitForSelectorSoft(client, ".inventory-window", 5_000);
}
// Whether the Market window currently renders a row for a listing id.
async function marketRowVisible(client, listingId) {
  return client.evaluate(
    `document.querySelector('[data-listing-id="' + ${JSON.stringify(String(listingId))} + '"]') != null`,
  );
}
// Whether the Mail window renders an inbox row whose sender matches `from`.
// Polls briefly — the window opens a render tick before the mail prop flows in.
async function mailRowVisible(client, from, timeoutMs = 2_500) {
  const needle = String(from ?? "").toLowerCase();
  return waitUntilSoft(
    client,
    `(() => {
      const rows = Array.from(document.querySelectorAll('[data-mail-view] [role="button"]'));
      return rows.some((r) => (r.getAttribute("aria-label") ?? "").toLowerCase().includes(${JSON.stringify(needle)}));
    })()`,
    timeoutMs,
  );
}

// ---------------------------------------------------------------------------
// WebSocket-frame helpers — the server's authoritative truth (the gateway wraps
// each ServerPacket as {type:"packet", packet, payload}, camelCase).
// ---------------------------------------------------------------------------
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
function sawPacket(client, sinceTime, packetName, predicate) {
  return parsePacketFrames(client, sinceTime).some(
    (p) => p.packet === packetName && (!predicate || predicate(p.payload ?? {})),
  );
}
async function waitForPacket(client, sinceTime, packetName, timeoutMs, predicate) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (sawPacket(client, sinceTime, packetName, predicate)) return true;
    await delay(150);
  }
  return false;
}
// The most recent payload of a packet type since `sinceTime` (for reading a
// result/reason field — e.g. MarketFail.reason, StorageUnlockResult.result).
function latestPacketPayload(client, sinceTime, packetName) {
  const packets = parsePacketFrames(client, sinceTime);
  for (let i = packets.length - 1; i >= 0; i -= 1) {
    if (packets[i].packet === packetName) return packets[i].payload ?? {};
  }
  return null;
}
function latestNpcGoods(client, sinceTime) {
  const packets = parsePacketFrames(client, sinceTime);
  for (let i = packets.length - 1; i >= 0; i -= 1) {
    if (packets[i].packet === "NPCGoods") {
      const list = Array.isArray(packets[i].payload?.list) ? packets[i].payload.list : [];
      return { list, panelType: packets[i].payload?.panelType ?? 0 };
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Movement / NPC primitives (subset of qa-items.mjs) — the gold faucet needs to
// reach a town merchant; the rest of the economy is NPC-less (qa.openStorage and
// the market commands work in-place).
// ---------------------------------------------------------------------------
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
async function readPlayerPos(client) {
  return client.evaluate(
    `(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`,
  );
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
function stepTowardTile(player, target) {
  const dx = Math.sign(target.x - player.x);
  const dy = Math.sign(target.y - player.y);
  return { x: target.x - dx, y: target.y - dy };
}
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
  const merchanty = /merchant|trader|grocer|store|supplies|smith|repair|weapon|armou?r|potion|jamie|gt[_ ]?merchant/i;
  const ranked = [...npcs].sort((a, b) => (merchanty.test(b.name ?? "") ? 1 : 0) - (merchanty.test(a.name ?? "") ? 1 : 0));
  return ranked.slice(0, 3);
}
async function openNpcDialog(client, npc, timeoutMs = NPC_DIALOG_TIMEOUT_MS) {
  const clicked = await clickTile(client, npc.x, npc.y, "left");
  if (!clicked) await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  const mine = `window.__mir2Stage5?.state?.activeNpcDialog != null && String(window.__mir2Stage5.state.activeNpcDialog.npcObjectId) === ${JSON.stringify(String(npc.objectId))}`;
  if (await waitUntilSoft(client, mine, timeoutMs)) return true;
  await sendCommand(client, { type: "interact", objectId: Number(npc.objectId) });
  return waitUntilSoft(client, mine, Math.min(4_000, timeoutMs));
}
async function approachAndOpenNpc(client, npc) {
  const deadline = Date.now() + 18_000;
  while (Date.now() < deadline) {
    const p = await readPlayerPos(client);
    if (!p) break;
    if (Math.max(Math.abs(npc.x - p.x), Math.abs(npc.y - p.y)) <= 1) break;
    const step = stepTowardTile(p, npc);
    if (!(await clickTile(client, step.x, step.y, "left"))) {
      await sendCommand(client, { type: "moveTo", x: step.x, y: step.y, mode: "run" });
    }
    await delay(750);
  }
  return openNpcDialog(client, npc);
}
async function reachMerchant(client, npc) {
  for (const off of [{ dx: 0, dy: 1 }, { dx: 1, dy: 0 }, { dx: -1, dy: 0 }]) {
    await transferTo(client, "0", npc.x + off.dx, npc.y + off.dy, 8_000).catch(() => {});
    await delay(400);
    const fresh = (await resolveNpcByName(client, npc.name)) ?? npc;
    if (await openNpcDialog(client, fresh, 3_500)) return fresh;
  }
  const fresh = (await resolveNpcByName(client, npc.name)) ?? npc;
  return (await approachAndOpenNpc(client, fresh)) ? fresh : null;
}
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
        if (/agitbuy|agittrade|agit/.test(s)) return 1;
        return 0;
      };
      let best = null, bestScore = 0;
      for (const l of links) {
        const target = l.getAttribute("data-target") || "";
        if (skip.has(target)) continue;
        const sc = score(l);
        if (sc > bestScore) { bestScore = sc; best = l; }
      }
      if (best && bestScore > 0) { best.click(); return (best.getAttribute("data-target") || best.textContent || "").trim().slice(0, 30); }
      return null;
    })()
  `);
}
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

// ---------------------------------------------------------------------------
// Login + character setup. Creates BOTH characters at the select screen (the
// mail-to-second-character beat needs B to already exist so the recipient
// resolves), then enters as A. Keeps qa-playthrough's phantom-character-slot
// handling — only our NAMED characters are trusted as server-backed.
// ---------------------------------------------------------------------------
async function openClientAndLoginScreen(ctx) {
  const client = ctx.client;
  await navigate(client, RUN_URL);
  await waitUntil(
    client,
    "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)",
    "client stage ready",
    30_000,
  );
  const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen === "login") await waitForSelectorSoft(client, ".login-overlay", 8_000);
}

// Log in (optionally registering first) until the character-select screen.
async function loginToSelect(ctx, { register }) {
  const client = ctx.client;
  let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
  if (screen === "select" || screen === "game") return true;
  await fillInput(client, ".login-input.account", account);
  await fillInput(client, ".login-input.password", password);
  if (register) {
    await click(client, ".login-button.account button");
    await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000);
    await delay(1800);
  }
  await fillInput(client, ".login-input.account", account);
  await fillInput(client, ".login-input.password", password);
  await click(client, ".login-button.ok button");
  return waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'select'", 30_000);
}

// Ensure a server-backed character with `name` exists; return its index (or null).
async function ensureCharacter(ctx, name) {
  const client = ctx.client;
  const haveChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(name)}); })()`;
  if (!(await client.evaluate(haveChar))) {
    await sendCommand(client, { type: "newCharacter", name, gender: "male", class: "warrior" });
    const created = await waitUntilSoft(client, haveChar, 15_000);
    if (!created) {
      ctx.addIssue({
        category: "flow",
        severity: "medium",
        beat: ctx.seq,
        summary: `character creation not reflected for "${name}" (newCharacter produced no server-backed character)`,
        detail: { name },
      });
      return null;
    }
  }
  return client.evaluate(
    `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(name)}); return c?.index ?? null; })()`,
  );
}

async function enterWorldAs(ctx, characterIndex, label) {
  const client = ctx.client;
  await sendCommand(client, { type: "startGame", characterIndex });
  const entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
  if (!entered) {
    const result = startGameResultFrom(client.webSocketFramesReceived.slice(-40));
    ctx.addIssue({
      category: "flow",
      severity: "high",
      beat: ctx.seq,
      summary:
        result === null
          ? `startGame (${label}) sent but never entered the game (no StartGame reply)`
          : `startGame (${label}) rejected by server (StartGame result=${result})`,
      detail: { characterIndex, result },
    });
    return false;
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
      severity: "medium",
      beat: ctx.seq,
      summary: `scene never became interaction/asset ready after entering world as ${label}`,
      detail: { mapFileName: (await readEconomyState(client)).mapFileName },
    });
  }
  await delay(1200);
  return true;
}

async function loginAndPrepare(ctx) {
  const client = ctx.client;

  await runBeat(ctx, "open client + login screen", async () => {
    await openClientAndLoginScreen(ctx);
  });

  await runBeat(ctx, "register + log in to character select", async (beat) => {
    beat.account = account;
    const reached = await loginToSelect(ctx, { register: createAccount });
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

  await runBeat(ctx, "create characters A + B, enter world as A", async (beat) => {
    beat.characterName = characterName;
    beat.characterNameB = characterNameB;
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "game") {
      beat.note = "already in game";
      return;
    }
    ctx.charAIndex = await ensureCharacter(ctx, characterName);
    ctx.charBIndex = await ensureCharacter(ctx, characterNameB);
    beat.charAIndex = ctx.charAIndex;
    beat.charBIndex = ctx.charBIndex;
    if (ctx.charBIndex == null) {
      ctx.addIssue({
        category: "flow",
        severity: "low",
        beat: beat.id,
        summary: "second character (B) could not be created — the cross-character mail beat will be skipped/limited",
        detail: { characterNameB },
      });
    }
    const startIndex = ctx.charAIndex ?? 0;
    await enterWorldAs(ctx, startIndex, `A=${characterName}`);
  });
}

// ---------------------------------------------------------------------------
// Beat 1 — SETUP: seed bag items (qa.giveItem) + best-effort earn a little gold.
// ---------------------------------------------------------------------------
async function setupInventory(ctx) {
  const client = ctx.client;
  await runBeat(ctx, "seed items + (best-effort) earn gold", async (beat) => {
    const before = await readEconomyState(client);
    for (const seed of SEED_ITEMS) {
      await qaGiveItem(client, seed.key, seed.count);
      await delay(350);
    }
    // Wait for all distinct seed stacks to land before snapshotting (each seed key
    // is its own stack), so the report and the later beats see the full kit.
    const seedTarget = Number(before.inventoryCount) + SEED_ITEMS.length;
    await waitUntilSoft(
      client,
      `(window.__mir2Stage5?.state?.inventoryItems ?? []).length >= ${seedTarget}`,
      MUTATION_TIMEOUT_MS,
    );
    const afterSeed = await readEconomyState(client);
    beat.seeded = {
      requested: SEED_ITEMS,
      invBefore: before.inventoryCount,
      invAfter: afterSeed.inventoryCount,
      items: afterSeed.inventoryItems.map((it) => it.name),
    };
    if (afterSeed.inventoryCount <= before.inventoryCount) {
      ctx.addIssue({
        category: "economy",
        severity: "high",
        beat: beat.id,
        summary: "qa.giveItem did not add any bag items (the QA seeding hook may be unavailable on this stack)",
        detail: beat.seeded,
      });
    }

    // Best-effort gold faucet: a fresh char has 0 gold and there is no gold QA
    // hook, so sell ONE seeded item at a town merchant (the only reliable, wired
    // gold source). If it fails, the mail beats fall back to gold-free letters.
    beat.goldBefore = afterSeed.gold;
    if (!seedGold) {
      beat.goldFaucet = { skipped: true, reason: "--seedGold false" };
      return;
    }
    if (typeof afterSeed.gold === "number" && afterSeed.gold > 0) {
      beat.goldFaucet = { skipped: true, reason: "already has gold", gold: afterSeed.gold };
      return;
    }
    const earned = await tryEarnGold(ctx, beat);
    const afterGold = await readEconomyState(client);
    beat.goldAfter = afterGold.gold;
    beat.goldFaucet = { ...earned, goldAfter: afterGold.gold };
    if (!earned.sold) {
      ctx.addIssue({
        category: "economy",
        severity: "low",
        beat: beat.id,
        summary: "could not earn gold from a merchant sale — mail gold-attachment will be exercised with 0 gold (env-gated, not a wiring bug)",
        detail: earned,
      });
    }
  });
}

// Reach a Bichon-town merchant, open its Buy/Sell service, and sell one seeded
// item so we have gold for the mail attachment. Bounded + best-effort.
async function tryEarnGold(ctx, beat) {
  const client = ctx.client;
  const deadline = Date.now() + GOLD_FAUCET_CAP_MS;
  const here = await client.evaluate(`window.__mir2Stage5?.state?.mapFileName === ${JSON.stringify(MERCHANT_TOWN.map)}`);
  if (!here) {
    await transferTo(client, MERCHANT_TOWN.map, MERCHANT_TOWN.x, MERCHANT_TOWN.y).catch(() => {});
  }
  const merchants = await listMerchantCandidates(client);
  beat.merchantCandidates = merchants.map((m) => m.name);
  let openedSell = false;
  for (const npc of merchants) {
    if (openedSell || Date.now() > deadline) break;
    const live = await reachMerchant(client, npc);
    if (!live) continue;
    const wsAt = Date.now();
    const tried = [];
    for (let pass = 0; pass < 6 && !openedSell; pass += 1) {
      let what = await clickNpcDialogLinkByKind(client, ["sell", "buy"]);
      if (!what) what = await clickBestTradeLink(client, tried);
      if (!what) break;
      tried.push(what);
      await waitForPacket(client, wsAt, "NPCGoods", 2200).catch(() => false);
      if (sawPacket(client, wsAt, "NPCSell") || sawPacket(client, wsAt, "NPCGoods")) openedSell = true;
      if (!(await client.evaluate(`window.__mir2Stage5?.state?.activeNpcDialog != null`)) && !openedSell) {
        await openNpcDialog(client, live, 4_000);
      }
    }
    if (openedSell) beat.merchant = live.name;
  }
  if (!openedSell) return { sold: false, reason: "no merchant Buy/Sell service reached", merchants: beat.merchantCandidates };

  const state = await readEconomyState(client);
  const sellable = state.inventoryItems.find((it) => it && it.uniqueId != null && it.container !== "quest");
  if (!sellable) return { sold: false, reason: "no sellable item in bag" };
  const goldBefore = state.gold;
  const wsBefore = Date.now();
  await commandSellItem(client, sellable.uniqueId, 1);
  const sold = await waitUntilSoft(
    client,
    `(() => { const g = window.__mir2Stage5?.state?.gold; return typeof g === "number" && ${goldBefore == null ? "g > 0" : `g > ${Number(goldBefore)}`}; })()`,
    MUTATION_TIMEOUT_MS,
  );
  const sellOk = sawPacket(client, wsBefore, "SellItem", (p) => p.success === true);
  const after = await readEconomyState(client);
  return { sold: sold || sellOk, item: sellable.name, goldBefore, goldAfter: after.gold, sellOk };
}

// ---------------------------------------------------------------------------
// Beat 2 — STORAGE / bank.
// ---------------------------------------------------------------------------
async function storageBeat(ctx) {
  const client = ctx.client;
  await runBeat(ctx, "storage: open + password + deposit/withdraw", async (beat) => {
    // Ensure at least one depositable bag item.
    let state = await readEconomyState(client);
    if (!state.inventoryItems.some((it) => it && it.uniqueId != null)) {
      await qaGiveItem(client, "blue-potion", 2);
      await delay(500);
      state = await readEconomyState(client);
    }

    // 1) OPEN storage via the non-gated qa hook -> NPCStorage opens the inventory
    //    window in storage mode (+ UserStorage seeds the warehouse grid).
    const wsOpen = Date.now();
    await qaOpenStorage(client);
    const opened = await waitUntilSoft(
      client,
      "document.querySelector('.inventory-window') != null",
      5_000,
    );
    const sawStorageOpen = await waitForPacket(client, wsOpen, "NPCStorage", WS_WAIT_MS).catch(() => false);
    beat.open = {
      inventoryWindowOpen: opened,
      sawNPCStorage: sawStorageOpen,
      sawUserStorage: sawPacket(client, wsOpen, "UserStorage"),
    };
    if (!opened || !sawStorageOpen) {
      ctx.addIssue({
        category: "storage",
        severity: "high",
        beat: beat.id,
        summary: "qa.openStorage did not open the storage service (no inventory window / NPCStorage frame)",
        detail: beat.open,
      });
    }

    // 2) SET a storage password (set mode sends current="").
    const wsSet = Date.now();
    await commandSetStoragePassword(client, "", STORAGE_PASSWORD);
    const setReflected = await waitUntilSoft(
      client,
      "window.__mir2Stage5?.state?.hasStoragePassword === true",
      MUTATION_TIMEOUT_MS,
    );
    const sawSetFrame = await waitForPacket(client, wsSet, "StoragePasswordResult", WS_WAIT_MS).catch(() => false);
    const setPayload = latestPacketPayload(client, wsSet, "StoragePasswordResult");
    beat.setPassword = { reflected: setReflected, sawFrame: sawSetFrame, result: setPayload?.result ?? null, hasPassword: setPayload?.hasPassword ?? null };
    if (!setReflected && !sawSetFrame) {
      ctx.addIssue({
        category: "storage",
        severity: "high",
        beat: beat.id,
        summary: "setting a storage password was not reflected (no hasStoragePassword + no StoragePasswordResult frame)",
        detail: beat.setPassword,
      });
    }

    // 3) RE-LOCK (re-open re-locks the session server-side) then UNLOCK with the
    //    password. The unlock result is the authoritative signal; the client's
    //    storageSessionUnlocked is secondary (qa.openStorage's re-lock isn't pushed).
    await qaOpenStorage(client);
    await delay(400);
    const wsUnlock = Date.now();
    await commandUnlockStorage(client, STORAGE_PASSWORD);
    const sawUnlockFrame = await waitForPacket(client, wsUnlock, "StorageUnlockResult", WS_WAIT_MS).catch(() => false);
    const unlockPayload = latestPacketPayload(client, wsUnlock, "StorageUnlockResult");
    const unlockedOk = unlockPayload ? unlockPayload.result === 0 || unlockPayload.result === 4 : false;
    await waitUntilSoft(client, "window.__mir2Stage5?.state?.storageSessionUnlocked === true", MUTATION_TIMEOUT_MS);
    const afterUnlock = await readEconomyState(client);
    beat.unlock = {
      sawFrame: sawUnlockFrame,
      result: unlockPayload?.result ?? null,
      unlockedByResult: unlockedOk,
      sessionUnlocked: afterUnlock.storageSessionUnlocked,
    };
    if (!sawUnlockFrame && !afterUnlock.storageSessionUnlocked) {
      ctx.addIssue({
        category: "storage",
        severity: "medium",
        beat: beat.id,
        summary: "unlocking storage with the password produced no StorageUnlockResult and the session stayed locked",
        detail: beat.unlock,
      });
    }

    // 4) DEPOSIT a bag item into a free warehouse slot, asserting slot counts move.
    const depositState = await readEconomyState(client);
    const item = depositState.inventoryItems.find((it) => it && it.uniqueId != null);
    if (!item) {
      ctx.addIssue({ category: "storage", severity: "low", beat: beat.id, summary: "no bag item available to deposit", detail: { inventory: depositState.inventoryItems } });
      return;
    }
    const toStorageSlot = firstFreeSlot(depositState.storageItems.map((s) => s.slot));
    const invBeforeDep = depositState.inventoryCount;
    const storeBeforeDep = depositState.storageCount;
    const wsDep = Date.now();
    await commandStoreItem(client, item.slot, toStorageSlot);
    const deposited = await waitUntilSoft(
      client,
      `(() => {
        const s = window.__mir2Stage5?.state ?? {};
        const inBag = (s.inventoryItems ?? []).some((it) => it && it.uniqueId === ${Number(item.uniqueId)});
        const store = Array.isArray(s.storageItems) ? s.storageItems : [];
        return !inBag || store.length > ${Number(storeBeforeDep)};
      })()`,
      MUTATION_TIMEOUT_MS,
    );
    const sawStoreOk = sawPacket(client, wsDep, "StoreItem", (p) => p.success === true);
    const afterDep = await readEconomyState(client);
    beat.deposit = {
      item: item.name, fromSlot: item.slot, toStorageSlot,
      invBefore: invBeforeDep, invAfter: afterDep.inventoryCount,
      storageBefore: storeBeforeDep, storageAfter: afterDep.storageCount,
      sawStoreSuccess: sawStoreOk, reflected: deposited,
    };
    const depositedOk = afterDep.storageCount > storeBeforeDep && afterDep.inventoryCount < invBeforeDep;
    if (!depositedOk && !sawStoreOk) {
      ctx.addIssue({
        category: "storage",
        severity: "high",
        beat: beat.id,
        summary: `deposit of "${item.name}" did not move it to the warehouse (bag/storage slot counts unchanged, no StoreItem success)`,
        detail: beat.deposit,
      });
      return;
    }

    // 5) WITHDRAW it back into a free bag slot, asserting the counts move back.
    const wState = await readEconomyState(client);
    const stored = wState.storageItems.find((s) => s.slot === toStorageSlot) ?? wState.storageItems[0];
    if (!stored) {
      ctx.addIssue({ category: "storage", severity: "medium", beat: beat.id, summary: "deposited item not found in storage to withdraw", detail: { storage: wState.storageItems } });
      return;
    }
    const toBagSlot = firstFreeSlot(wState.inventoryItems.map((i) => i.slot));
    const invBeforeW = wState.inventoryCount;
    const storeBeforeW = wState.storageCount;
    const wsW = Date.now();
    await commandTakeBackItem(client, stored.slot, toBagSlot);
    const withdrawn = await waitUntilSoft(
      client,
      `(() => {
        const s = window.__mir2Stage5?.state ?? {};
        const inv = Array.isArray(s.inventoryItems) ? s.inventoryItems : [];
        const store = Array.isArray(s.storageItems) ? s.storageItems : [];
        return inv.length > ${Number(invBeforeW)} || store.length < ${Number(storeBeforeW)};
      })()`,
      MUTATION_TIMEOUT_MS,
    );
    const sawTakeOk = sawPacket(client, wsW, "TakeBackItem", (p) => p.success === true);
    const afterW = await readEconomyState(client);
    beat.withdraw = {
      item: stored.name, fromStorageSlot: stored.slot, toBagSlot,
      invBefore: invBeforeW, invAfter: afterW.inventoryCount,
      storageBefore: storeBeforeW, storageAfter: afterW.storageCount,
      sawTakeBackSuccess: sawTakeOk, reflected: withdrawn,
    };
    const withdrawnOk = afterW.inventoryCount > invBeforeW && afterW.storageCount < storeBeforeW;
    if (!withdrawnOk && !sawTakeOk) {
      ctx.addIssue({
        category: "storage",
        severity: "high",
        beat: beat.id,
        summary: `withdraw of "${stored.name}" did not move it back to the bag (slot counts unchanged, no TakeBackItem success)`,
        detail: beat.withdraw,
      });
    }
  });
}

// ---------------------------------------------------------------------------
// Beat 3 — MARKET / auction: consign -> browse -> buy(attempt) -> reclaim.
// ---------------------------------------------------------------------------
async function marketBeat(ctx) {
  const client = ctx.client;
  await runBeat(ctx, "market: consign + browse + buy + reclaim", async (beat) => {
    // Close storage's inventory window so the market window is unobstructed.
    await client.evaluate(`(() => { const w = document.querySelector(".inventory-window"); if (w) { const b = document.querySelector(".hud-button.inventory button"); if (b) b.click(); } })()`).catch(() => {});
    await delay(300);

    let state = await readEconomyState(client);
    let item = state.inventoryItems.find((it) => it && it.uniqueId != null && it.container !== "quest");
    if (!item) {
      await qaGiveItem(client, "iron-sword", 1);
      await delay(500);
      state = await readEconomyState(client);
      item = state.inventoryItems.find((it) => it && it.uniqueId != null && it.container !== "quest");
    }
    if (!item) {
      ctx.addIssue({ category: "market", severity: "low", beat: beat.id, summary: "no bag item to consign on the market", detail: { inventory: state.inventoryItems } });
      return;
    }

    // 1) CONSIGN (list) the item. The window's onList is unmounted, so this drives
    //    the underlying ConsignItem BrowserCommand directly (price > 0 required).
    const listPrice = 100;
    const invBefore = state.inventoryCount;
    const auctionBefore = state.auctionCount;
    const wsList = Date.now();
    await commandConsignItem(client, item.uniqueId, listPrice, 0);
    const consigned = await waitUntilSoft(
      client,
      `!(window.__mir2Stage5?.state?.inventoryItems ?? []).some((it) => it && it.uniqueId === ${Number(item.uniqueId)})`,
      MUTATION_TIMEOUT_MS,
    );
    const sawConsign = sawPacket(client, wsList, "ConsignItem", (p) => p.success === true);
    beat.consign = { item: item.name, uniqueId: item.uniqueId, price: listPrice, invBefore, leftBag: consigned, sawConsignSuccess: sawConsign };
    if (!consigned && !sawConsign) {
      ctx.addIssue({
        category: "market",
        severity: "high",
        beat: beat.id,
        summary: `consigning "${item.name}" did not list it (still in bag, no ConsignItem success)`,
        detail: beat.consign,
      });
    }

    // 2) BROWSE: refresh -> NPCMarket -> world.stage5Systems.auction; open the
    //    Market window and confirm our listing renders + a search filters to it.
    const wsRefresh = Date.now();
    await commandMarketRefresh(client);
    await waitForPacket(client, wsRefresh, "NPCMarket", WS_WAIT_MS).catch(() => false);
    await waitUntilSoft(client, "(window.__mir2Stage5?.state?.stage5Systems?.auction ?? []).length > 0", MUTATION_TIMEOUT_MS);
    const browseState = await readEconomyState(client);
    const ours = findOurListing(browseState.auction, characterName);
    const marketOpened = await openMarketWindow(client);
    const rowVisible = ours ? await marketRowVisible(client, ours.id).catch(() => false) : false;
    beat.browse = {
      auctionCount: browseState.auctionCount,
      auctionBefore,
      ourListing: ours ? { id: ours.id, itemName: ours.itemName, seller: ours.seller, price: ours.price } : null,
      marketWindowOpen: marketOpened,
      rowVisible,
    };
    if (!ours) {
      ctx.addIssue({
        category: "market",
        severity: "high",
        beat: beat.id,
        summary: "after consigning + refreshing, our listing was not present in the market listings (stage5Systems.auction)",
        detail: { auction: browseState.auction },
      });
    } else if (!marketOpened || !rowVisible) {
      ctx.addIssue({
        category: "market",
        severity: "medium",
        beat: beat.id,
        summary: "market listing exists in state but did not render a row in the Market window",
        detail: beat.browse,
      });
    }
    // Search filter. The server matches the query against the listing's item KEY
    // (e.g. "red-potion" -> "redpotion") / its title-cased name, NOT the gateway-
    // resolved Crystal template name the row shows ("(HP)DrugSmall"), so search by
    // a token of the CONSIGNED item's name (e.g. "Potion"/"Sword").
    const searchQuery = String(item.name ?? "").split(/\s+/).filter(Boolean).pop() ?? "";
    if (searchQuery) {
      const wsSearch = Date.now();
      await commandMarketSearch(client, searchQuery);
      await waitForPacket(client, wsSearch, "NPCMarket", WS_WAIT_MS).catch(() => false);
      const afterSearch = await readEconomyState(client);
      const matched = afterSearch.auction.some((a) => a.id === ours.id);
      beat.search = { query: searchQuery, matched };
      if (!matched) {
        ctx.addIssue({
          category: "market",
          severity: "low",
          beat: beat.id,
          summary: `market search for "${searchQuery}" did not return our own listing (search filter may not match the consigned item key)`,
          detail: { query: searchQuery, auction: afterSearch.auction },
        });
      }
    }

    if (!ours) return;

    // 3) BUY (attempt). A fresh single session only sees its OWN listing; the
    //    server correctly REJECTS a self-purchase (MarketFail reason 6). That
    //    rejection is the proof the buy command is wired + processed. A non-self
    //    listing (if one existed) would instead complete (LoseGold + item gained).
    const goldBeforeBuy = browseState.gold;
    const wsBuy = Date.now();
    await commandMarketBuy(client, ours.id, 0);
    const boughtOrRejected = await waitUntilSoft(
      client,
      `(() => {
        const s = window.__mir2Stage5?.state ?? {};
        const g = s.gold;
        return (typeof g === "number" && ${goldBeforeBuy == null ? "false" : `g < ${Number(goldBeforeBuy)}`});
      })()`,
      3000,
    );
    const sawBuyFail = await waitForPacket(client, wsBuy, "MarketFail", WS_WAIT_MS).catch(() => false);
    const failPayload = latestPacketPayload(client, wsBuy, "MarketFail");
    const gained = sawPacket(client, wsBuy, "LoseGold");
    beat.buy = {
      listingId: ours.id,
      goldBefore: goldBeforeBuy,
      goldAfter: (await readEconomyState(client)).gold,
      completed: boughtOrRejected || gained,
      rejected: sawBuyFail,
      failReason: failPayload?.reason ?? null,
      selfSellerExpected: true,
    };
    if (!boughtOrRejected && !sawBuyFail && !gained) {
      ctx.addIssue({
        category: "market",
        severity: "high",
        beat: beat.id,
        summary: "market buy produced neither a purchase (LoseGold) nor a MarketFail — the buy command may not be reaching the server",
        detail: beat.buy,
      });
    } else if (sawBuyFail && failPayload?.reason === 6) {
      beat.buy.note = "self-purchase correctly rejected (reason 6) — buy path is wired; a non-self listing is needed to complete a real purchase in this single-session model";
    }

    // 4) RECLAIM the consignment (marketGetBack) — returns the item to the bag.
    const reclaimState = await readEconomyState(client);
    const invBeforeReclaim = reclaimState.inventoryCount;
    const wsReclaim = Date.now();
    await commandMarketGetBack(client, ours.id, 0);
    const reclaimed = await waitUntilSoft(
      client,
      `(() => {
        const s = window.__mir2Stage5?.state ?? {};
        const inv = (s.inventoryItems ?? []).length;
        const auction = s.stage5Systems?.auction ?? [];
        const stillListed = auction.some((a) => (a.id ?? a.auctionId) === ${JSON.stringify(ours.id)} && a.cancelled !== true);
        return inv > ${Number(invBeforeReclaim)} || !stillListed;
      })()`,
      MUTATION_TIMEOUT_MS,
    );
    const sawReclaim = sawPacket(client, wsReclaim, "MarketSuccess") || sawPacket(client, wsReclaim, "GainedGold");
    const afterReclaim = await readEconomyState(client);
    beat.reclaim = {
      listingId: ours.id,
      invBefore: invBeforeReclaim,
      invAfter: afterReclaim.inventoryCount,
      reflected: reclaimed,
      sawMarketSuccess: sawReclaim,
    };
    if (!reclaimed && !sawReclaim) {
      ctx.addIssue({
        category: "market",
        severity: "medium",
        beat: beat.id,
        summary: "reclaiming the consignment (marketGetBack) returned no item to the bag and no MarketSuccess frame",
        detail: beat.reclaim,
      });
    }
    await closeWindowIfOpen(client, "[data-market-mode]", "m");
  });
}

// ---------------------------------------------------------------------------
// Beat 4 — MAIL (self): an immediate, fully-observable round-trip. Sending to our
// OWN name routes the mail into our live mailbox (MailSent + ReceiveMail), so we
// can verify the whole compose->send->receive->read->claim lifecycle at once.
// ---------------------------------------------------------------------------
async function mailSelfBeat(ctx) {
  const client = ctx.client;
  await runBeat(ctx, "mail (self): send + receive + read + claim", async (beat) => {
    const before = await readEconomyState(client);
    // Attach gold only if we have some (no gold QA faucet); a 0-gold letter still
    // sends (cost is 0). Sending to self returns the gold on claim (net zero).
    const attachGold = typeof before.gold === "number" && before.gold > 0 ? Math.min(before.gold, 50) : 0;
    const mailCountBefore = before.mailCount;
    const wsSend = Date.now();
    await commandSendMail(client, characterName, "QA self-mail round-trip.", attachGold);
    const sentOk = await waitForPacket(client, wsSend, "MailSent", WS_WAIT_MS, (p) => p.result === 0).catch(() => false);
    const received = await waitUntilSoft(
      client,
      `(window.__mir2Stage5?.state?.stage5Systems?.mail ?? []).length > ${Number(mailCountBefore)}`,
      MUTATION_TIMEOUT_MS,
    );
    const afterSend = await readEconomyState(client);
    const mine = afterSend.mail.find((m) => (m.from ?? "").toLowerCase() === characterName.toLowerCase());
    beat.send = {
      to: characterName,
      attachGold,
      sawMailSent0: sentOk,
      mailCountBefore,
      mailCountAfter: afterSend.mailCount,
      received,
      goldBefore: before.gold,
      goldAfterSend: afterSend.gold,
      mail: mine ?? null,
    };
    if (!sentOk && !received) {
      ctx.addIssue({
        category: "mail",
        severity: "high",
        beat: beat.id,
        summary: "self-mail did not round-trip (no MailSent result 0 and the inbox did not grow)",
        detail: beat.send,
      });
      return;
    }

    // Confirm the Mail window renders the new letter.
    const mailWindowOpen = await openMailWindow(client);
    const rowVisible = await mailRowVisible(client, characterName).catch(() => false);
    beat.window = { mailWindowOpen, rowVisible };
    if (mailWindowOpen && !rowVisible) {
      ctx.addIssue({
        category: "mail",
        severity: "low",
        beat: beat.id,
        summary: "mail present in state but no row rendered for it in the Mail window",
        detail: { mail: afterSend.mail },
      });
    }

    if (!mine || mine.id == null) {
      await closeWindowIfOpen(client, "[data-mail-view]", "l");
      return;
    }

    // READ (mark opened) then CLAIM the attachment (collectParcel). For a self-mail
    // with gold, the claim returns the gold (gold goes back up).
    await commandReadMail(client, mine.id);
    await waitUntilSoft(
      client,
      `(window.__mir2Stage5?.state?.stage5Systems?.mail ?? []).some((m) => (m.mailId ?? m.id) === ${JSON.stringify(mine.id)} && m.opened === true)`,
      MUTATION_TIMEOUT_MS,
    );
    const goldBeforeClaim = (await readEconomyState(client)).gold;
    const wsClaim = Date.now();
    await commandCollectParcel(client, mine.id);
    const sawCollected = await waitForPacket(client, wsClaim, "ParcelCollected", WS_WAIT_MS).catch(() => false);
    const claimReflected = await waitUntilSoft(
      client,
      `(window.__mir2Stage5?.state?.stage5Systems?.mail ?? []).some((m) => (m.mailId ?? m.id) === ${JSON.stringify(mine.id)} && (m.collected === true || m.claimed === true))`,
      MUTATION_TIMEOUT_MS,
    );
    const afterClaim = await readEconomyState(client);
    const goldReturned = attachGold > 0 && typeof afterClaim.gold === "number" && typeof goldBeforeClaim === "number" && afterClaim.gold > goldBeforeClaim;
    beat.claim = {
      mailId: mine.id,
      attachGold,
      opened: true,
      sawParcelCollected: sawCollected,
      claimReflected,
      goldBeforeClaim,
      goldAfterClaim: afterClaim.gold,
      goldReturned: attachGold > 0 ? goldReturned : "n/a (letter)",
    };
    if (!sawCollected && !claimReflected) {
      ctx.addIssue({
        category: "mail",
        severity: "medium",
        beat: beat.id,
        summary: "claiming the self-mail attachment produced no ParcelCollected frame and no claimed flag",
        detail: beat.claim,
      });
    } else if (attachGold > 0 && !goldReturned) {
      ctx.addIssue({
        category: "mail",
        severity: "medium",
        beat: beat.id,
        summary: `claimed a ${attachGold}-gold self-mail but the gold did not return to the wallet`,
        detail: beat.claim,
      });
    }
    await closeWindowIfOpen(client, "[data-mail-view]", "l");
  });
}

// ---------------------------------------------------------------------------
// Beat 5 — MAIL (to char B): send from A, switch to B (re-login), receive + claim.
// ---------------------------------------------------------------------------
async function mailCrossBeat(ctx) {
  const client = ctx.client;
  await runBeat(ctx, "mail (cross-char): send to B, switch, claim on B", async (beat) => {
    if (ctx.charBIndex == null) {
      ctx.addIssue({
        category: "mail",
        severity: "low",
        beat: beat.id,
        summary: "second character (B) was not created — skipping the cross-character mail beat",
        detail: { characterNameB },
      });
      return;
    }

    // 1) On A: send a letter (+gold if available) to B. MailSent result 0 proves
    //    the recipient resolved and the mail was written to B's persisted save.
    const before = await readEconomyState(client);
    const attachGold = typeof before.gold === "number" && before.gold > 0 ? Math.min(before.gold, 50) : 0;
    const wsSend = Date.now();
    await commandSendMail(client, characterNameB, "QA cross-character delivery.", attachGold);
    const sentOk = await waitForPacket(client, wsSend, "MailSent", WS_WAIT_MS, (p) => p.result === 0).catch(() => false);
    const sentPayload = latestPacketPayload(client, wsSend, "MailSent");
    beat.send = { from: characterName, to: characterNameB, attachGold, sawMailSent0: sentOk, mailSentResult: sentPayload?.result ?? null };
    if (!sentOk) {
      ctx.addIssue({
        category: "mail",
        severity: "high",
        beat: beat.id,
        summary: `sending mail from A to B (${characterNameB}) was not acknowledged with MailSent result 0 (recipient resolution or gold gate)`,
        detail: beat.send,
      });
      return;
    }

    // 2) Switch to character B: log out -> log back in (same account) -> enter as B.
    await sendCommand(client, { type: "logOut" });
    const atLogin = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'login'", 15_000);
    if (!atLogin) {
      ctx.addIssue({ category: "mail", severity: "medium", beat: beat.id, summary: "logout did not return to the login screen — cannot switch to character B", detail: { screen: (await readEconomyState(client)).screen } });
      return;
    }
    await delay(1500); // let the gateway settle the prior session before re-login
    const reSelect = await loginToSelect(ctx, { register: false });
    if (!reSelect) {
      ctx.addIssue({ category: "mail", severity: "medium", beat: beat.id, summary: "re-login (to switch to B) did not reach character select", detail: { account } });
      return;
    }
    // Re-resolve B's index by name (indices are stable but read fresh to be safe).
    const bIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterNameB)}); return c?.index ?? null; })()`,
    );
    if (bIndex == null) {
      ctx.addIssue({ category: "mail", severity: "medium", beat: beat.id, summary: "character B not present at select after re-login", detail: { characterNameB } });
      return;
    }
    const enteredB = await enterWorldAs(ctx, bIndex, `B=${characterNameB}`);
    beat.switch = { loggedOut: true, reSelect, bIndex, entered: enteredB };
    if (!enteredB) return;

    // 3) On B: the delivered mail is loaded into B's live mailbox from the save, but
    //    no non-empty ReceiveMail is pushed on enter-world; a mail op nudges the
    //    server to re-send the full inbox. Probe a few low ids (B's save started
    //    empty, so the delivered letter is id 1) until the inbox surfaces it.
    let bMail = null;
    for (let id = 1; id <= 4 && !bMail; id += 1) {
      const wsProbe = Date.now();
      await commandReadMail(client, id);
      await waitForPacket(client, wsProbe, "ReceiveMail", 2500).catch(() => false);
      const st = await readEconomyState(client);
      bMail = st.mail.find((m) => (m.from ?? "").toLowerCase() === characterName.toLowerCase());
    }
    const bState = await readEconomyState(client);
    beat.receive = {
      inboxCount: bState.mailCount,
      inbox: bState.mail,
      foundFromA: Boolean(bMail),
      mail: bMail ?? null,
    };
    if (!bMail) {
      ctx.addIssue({
        category: "mail",
        severity: "medium",
        beat: beat.id,
        summary: "cross-character mail was delivered (sender saw MailSent result 0) but did not surface in B's live inbox after probing — recipient-side inbox refresh wiring gap",
        detail: { probedIds: [1, 2, 3, 4], inbox: bState.mail },
      });
      return;
    }

    // 4) Claim it on B (collectParcel) — verify the parcel/gold lands on B.
    const goldBeforeClaim = bState.gold;
    const wsClaim = Date.now();
    await commandCollectParcel(client, bMail.id);
    const sawCollected = await waitForPacket(client, wsClaim, "ParcelCollected", WS_WAIT_MS).catch(() => false);
    const claimReflected = await waitUntilSoft(
      client,
      `(window.__mir2Stage5?.state?.stage5Systems?.mail ?? []).some((m) => (m.mailId ?? m.id) === ${JSON.stringify(bMail.id)} && (m.collected === true || m.claimed === true))`,
      MUTATION_TIMEOUT_MS,
    );
    const afterClaim = await readEconomyState(client);
    const goldGained = attachGold > 0 && typeof afterClaim.gold === "number" && typeof goldBeforeClaim === "number" && afterClaim.gold > goldBeforeClaim;
    beat.claim = {
      mailId: bMail.id,
      attachGold,
      sawParcelCollected: sawCollected,
      claimReflected,
      goldBeforeClaim,
      goldAfterClaim: afterClaim.gold,
      goldGained: attachGold > 0 ? goldGained : "n/a (letter)",
    };
    if (!sawCollected && !claimReflected) {
      ctx.addIssue({
        category: "mail",
        severity: "medium",
        beat: beat.id,
        summary: "claiming the cross-character mail on B produced no ParcelCollected frame and no claimed flag",
        detail: beat.claim,
      });
    } else if (attachGold > 0 && !goldGained) {
      ctx.addIssue({
        category: "mail",
        severity: "low",
        beat: beat.id,
        summary: `claimed a ${attachGold}-gold parcel on B but B's gold did not increase`,
        detail: beat.claim,
      });
    }
  });
}

async function wrapBeat(ctx) {
  await runBeat(ctx, "wrap + final economy snapshot", async (beat) => {
    beat.final = await readEconomyState(ctx.client);
  });
}

// ---------------------------------------------------------------------------
// Selection helpers.
// ---------------------------------------------------------------------------
function firstFreeSlot(usedSlots) {
  const used = new Set((usedSlots ?? []).filter((s) => typeof s === "number"));
  for (let i = 0; i < 80; i += 1) {
    if (!used.has(i)) return i;
  }
  return 0;
}
function findOurListing(auction, sellerName) {
  if (!Array.isArray(auction) || !auction.length) return null;
  const mine = auction.filter((a) => (a.seller ?? "").toLowerCase() === String(sellerName ?? "").toLowerCase() && a.cancelled !== true && a.sold !== true);
  if (mine.length) return mine[mine.length - 1];
  // Fall back to the most-recent (highest id) listing — in a fresh single
  // session the only listing is ours, even if the seller label didn't match.
  return [...auction].filter((a) => a.cancelled !== true).sort((a, b) => Number(a.id) - Number(b.id)).pop() ?? null;
}

// ---------------------------------------------------------------------------
// Cross-cutting collectors.
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
    characterNameB,
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
  md.push(`# Economy QA report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\`  ·  account \`${account}\`  ·  chars \`${characterName}\` / \`${characterNameB}\``);
  md.push(`- Beats: ${summary.beatsOk}/${summary.beats} ok  ·  Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push(`- By category: ${Object.entries(byCategory).map(([k, v]) => `${k} ${v}`).join(", ") || "none"}`);
  md.push("");
  md.push("## Issues (by severity)");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues detected — storage, mail, and market all worked._");
  } else {
    md.push("| # | Severity | Category | Beat | Summary |");
    md.push("|---|---|---|---|---|");
    for (const i of sortedIssues) {
      md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.beat ?? "—"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    }
  }
  md.push("");
  md.push("## Beats");
  md.push("");
  for (const b of ctx.beats) {
    md.push(`### Beat ${b.id} — ${b.title} ${b.ok ? "✅" : "❌"}  (${b.durationMs}ms)`);
    if (b.error) md.push(`- error: \`${b.error.slice(0, 300)}\``);
    if (b.note) md.push(`- note: ${b.note}`);
    if (b.state) {
      md.push(
        `- state: screen=\`${b.state.screen}\` map=\`${b.state.mapFileName ?? "?"}\` gold=${b.state.gold ?? "—"} bag=${b.state.inventoryCount} storage=${b.state.storageCount} mail=${b.state.mailCount} auction=${b.state.auctionCount} pw=${b.state.hasStoragePassword}/${b.state.storageSessionUnlocked}`,
      );
    }
    for (const key of ["seeded", "goldFaucet", "open", "setPassword", "unlock", "deposit", "withdraw", "consign", "browse", "search", "buy", "reclaim", "send", "window", "switch", "receive", "claim"]) {
      if (b[key] !== undefined) md.push(`- ${key}: ${JSON.stringify(b[key])}`);
    }
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
  md.push(`cd apps/web && node ./scripts/qa-economy.mjs --headed --baseUrl ${baseUrl}${gatewayWs ? ` --gatewayWs ${gatewayWs}` : ""}`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-items.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-economy-${process.pid}-${Date.now()}`);
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
  if (/Failed to load resource/i.test(text) && /\b(404|Not Found)\b/i.test(text)) return false;
  return true;
}

function slug(text) {
  return String(text).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40);
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

function defaultCharacterName(suffixLetter) {
  // Keep the distinguishing letter right after the prefix so it survives the
  // 10-char cap — otherwise A and B truncate to the SAME name and the
  // cross-character mail beat degenerates into a second self-send.
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `EQ${suffixLetter}${suffix}`.slice(0, 10).toUpperCase();
}

function defaultAccountName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `EQ${suffix}`.slice(0, 12).toUpperCase();
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
  console.log(`qa-economy: target=${baseUrl} account=${account} chars=${characterName}/${characterNameB} headed=${headed}${gatewayWs ? ` gatewayWs=${gatewayWs}` : ""}`);
  console.log(`qa-economy: output=${outputDir}`);

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

    await loginAndPrepare(ctx);
    await setupInventory(ctx);
    await storageBeat(ctx);
    await marketBeat(ctx);
    await mailSelfBeat(ctx);
    await mailCrossBeat(ctx);
    await wrapBeat(ctx);

    collectNetworkIssues(ctx);
    collectConsoleIssues(ctx);
    await writeReport(ctx);

    const s = ctx.issues.reduce((acc, i) => ((acc[i.severity] = (acc[i.severity] ?? 0) + 1), acc), {});
    console.log(`\nqa-economy done: ${ctx.beats.filter((b) => b.ok).length}/${ctx.beats.length} beats ok, ${ctx.issues.length} issues (high ${s.high ?? 0}, medium ${s.medium ?? 0}, low ${s.low ?? 0}).`);
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
  } catch (error) {
    console.error("qa-economy fatal:", error);
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
