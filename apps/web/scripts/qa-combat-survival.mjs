// qa-combat-survival.mjs - AI-driven "real player" full combat / survival cycle.
//
// The existing qa-playthrough.mjs only checks the single beat "a monster takes
// damage". This harness drives the REAL web client over Chrome DevTools Protocol
// through the WHOLE fight-and-survive loop and records EVERY broken step into a
// structured report under docs/generated/combat-survival-qa/:
//
//   1. Attack a monster -> its HP drops and it DIES (entity gone / dead flag).
//   2. Damage indicators appear (.scene-damage-floater - the only damage signal
//      visible under the Bevy renderer; combat juice lives in the DOM overlay).
//   3. You TAKE damage -> state.playerHp drops.
//   4. On death -> respawn/revive flow works.
//   5. LOOT after a kill (state.groundDrops -> walk onto the drop -> pickGroundDrop)
//      -> item enters inventory (state.inventoryItems grows).
//   6. XP / level changes (server GainExperience / LevelChanged).
//
// Why a browser (not a protocol bot): the client's attack/loot/death paths
// (page.tsx attackTarget/activateEntity -> send({type:"attack"}); pickGroundDrop ->
// send({type:"pickUp"})) and the DOM damage overlay only exist in a real client.
// Headed Chrome gives a real GPU so the Bevy canvas actually renders.
//
// Outcomes are read from the WEBSOCKET TRUTH LAYER, not the client store: a monster's
// client `entity.hp` is unreliable (ObjectHealth is a percent and the on-demand snapshot
// re-asserts the template max), and render-facing state.player can lead the server.
// So kill/damage/XP/drop/death are judged from the server's own frames
// (DamageIndicator, ObjectHealth, ObjectDied, GainExperience, ObjectItem, GainedItem),
// while movement/action gating uses state.authoritativePlayer when available.
// Movement likewise can only be driven by real in-viewport clicks -> moveToTile (the
// convenience `moveTo` command is rejected on the production player path), and a swing
// must NOT share a step with a walk (an attack arms the client's movement-input block).
//
// Real combat NUMERICS live only on the Rust gateway+sim. By default this harness
// points the client at the Rust gateway ws://127.0.0.1:7111/ws via the localhost-only
// `?gatewayWs=` override (apps/web/app/page.tsx:996). If the client ends up on the
// :7110 node proxy instead, the run is still validated but flagged numerics-proxy-backed.
//
// KNOWN GAP it accounts for (does NOT hard-fail on): player/monster death+attack
// +struck ANIMATIONS may not render yet (action frames are R2-only / meta-shadowed;
// fix in flight on branch claude/fervent-wescoff-af8df9). So the LOGICAL outcome
// (hp drop, dead flag, drop spawned) is asserted SEPARATELY from the animation,
// which is recorded as its own known-gap finding.
//
// This file deliberately reuses the patterns proven in qa-playthrough.mjs (CDP
// client, headed chrome launch, login flow, tile clicking, transferTo) so it stays
// consistent with the house QA harnesses, with zero new dependencies. It is
// self-contained: it drives its own register -> login -> enter-world -> hunt loop.
//
// Usage:
//   node ./scripts/qa-combat-survival.mjs --headed [--baseUrl http://127.0.0.1:3022]
//        [--gatewayWs ws://127.0.0.1:7111/ws] [--account NAME --password PW]
//        [--field 1|2] [--runId my-run]
//
// Each beat is best-effort: a failing beat is recorded as an issue and the loop
// keeps going so it captures as much of the cycle as possible.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3022";
// Real combat numerics are authoritative on the Rust gateway (:7111), not the
// :7110 node proxy. The `gatewayWs` query override is honored only on a localhost
// origin (apps/web/app/page.tsx:991-998), so this just works for a local dev client.
const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:7111/ws";
const forceGateway = booleanArg(args.forceGateway ?? process.env.MIR2_FORCE_GATEWAY, true);
// Load with scene-motion / debug hooks enabled (?mir2Debug=1) for parity with the
// playthrough harness and extra diagnostics; the gateway override targets the Rust sim.
const RUN_URL = buildRunUrl(baseUrl, { gatewayWs: forceGateway ? gatewayWs : null });

const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultAccountName();
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test1";
const characterName = args.characterName ?? defaultCharacterName();
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9300 + (process.pid % 300));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "combat-survival-qa", `run-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };
const MIR_DIRECTIONS = [
  "Up",
  "UpRight",
  "Right",
  "DownRight",
  "Down",
  "DownLeft",
  "Left",
  "UpLeft",
];
const MIR_DIRECTION_DELTAS = {
  Up: { x: 0, y: -1 },
  UpRight: { x: 1, y: -1 },
  Right: { x: 1, y: 0 },
  DownRight: { x: 1, y: 1 },
  Down: { x: 0, y: 1 },
  DownLeft: { x: -1, y: 1 },
  Left: { x: -1, y: 0 },
  UpLeft: { x: -1, y: -1 },
};

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const COMBAT_WINDOW_MS = numberArg(args.combatWindowMs, 40_000); // per-fight budget (breaks early on kill / player death)
const MAX_FIGHTS = numberArg(args.maxFights, 5); // attempts to land one clean kill (skips phantom/unreachable targets between tries)
// Radius (Chebyshev tiles) within which a field monster counts as "found" while
// exploring. Town guards / invulnerable targets are excluded separately
// (readTargetMonster) - that exclusion is what keeps the player out of the town SAFE
// ZONE (where the sim makes combat a no-op - gm/combat parity: "safe-zone players are
// immune to combat damage").
const EXPLORE_SCAN_RANGE = numberArg(args.exploreScanRange, 22);
// How long to explore a field for a live (on-demand-pool) monster before moving on.
// Killable monsters spawn deeper in the field than the town-edge transfer spawn, so the
// player walks waypoints outward; the pool (#83) also populates over time / with movement.
const SPAWN_POLL_MS = numberArg(args.spawnPollMs, 26_000);
const TAKE_DAMAGE_WINDOW_MS = numberArg(args.takeDamageWindowMs, 16_000);
const TAKE_DAMAGE_SETTLE_MS = numberArg(args.takeDamageSettleMs, 20_000);
const LOOT_WINDOW_MS = numberArg(args.lootWindowMs, 14_000);
const DEATH_WINDOW_MS = numberArg(args.deathWindowMs, 9_000);
const REVIVE_WINDOW_MS = numberArg(args.reviveWindowMs, 9_000);
const QA_SEED_WINDOW_MS = numberArg(args.qaSeedWindowMs, 30_000);
const ALLOW_QA_PICKUP_SEED = booleanArg(args.allowQaPickupSeed ?? process.env.MIR2_ALLOW_QA_PICKUP_SEED, false);
const PREFER_QA_PICKUP_SEED = booleanArg(args.preferQaPickupSeed ?? process.env.MIR2_PREFER_QA_PICKUP_SEED, false);
const ALLOW_QA_COMBAT_SEED = booleanArg(args.allowQaCombatSeed ?? process.env.MIR2_ALLOW_QA_COMBAT_SEED, false);
const PREFER_QA_COMBAT_SEED = booleanArg(args.preferQaCombatSeed ?? process.env.MIR2_PREFER_QA_COMBAT_SEED, false);
const QA_CONTROL_TOKEN = args.qaControlToken ?? process.env.MIR2_QA_CONTROL_TOKEN ?? null;
const BLACK_LUMA_MEAN = 8;

// Hunting fields the harness hunts on. mapFileName equals the transfer key map token
// (see QUICK_TRANSFER_OPTIONS in apps/web/app/page.tsx). Prefer the Bichon starter
// outskirts first: Crystal places Hen/Scarecrow/Deer there, which are the right
// kill/loot/XP proof targets for a fresh level-1 character. Woomyon/Serpent remain
// fallback survival/combat fields when the starter outskirts are hunted out.
const HUNTING_FIELDS = [
  { map: "0", x: 288, y: 616, safeSize: 10, label: "Bichon starter outskirts (0)" },
  { map: "1", x: 315, y: 82, safeSize: 15, label: "Woomyon Woods (1)" },
  { map: "2", x: 503, y: 483, safeSize: 10, label: "Serpent Valley (2)" },
];

function fieldForMap(mapFileName) {
  return HUNTING_FIELDS.find((f) => f.map === String(mapFileName));
}

function fieldCombatAnchor(field, tiles = 18) {
  const clearTiles = Math.max(tiles, (field?.safeSize ?? 0) + 3);
  return { x: field.x, y: field.y + clearTiles };
}

function isKnownFieldSafeZoneState(state) {
  const field = fieldForMap(state?.mapFileName);
  const p = state?.authoritativePlayer ?? state?.player;
  if (!field || !p) return Boolean(state?.inSafeZone);
  const knownSafe = Math.max(Math.abs(p.x - field.x), Math.abs(p.y - field.y)) <= (field.safeSize ?? 0);
  return Boolean(state?.inSafeZone) || knownSafe;
}

function carriedItemCount(state) {
  const inventory = Array.isArray(state?.inventoryItems)
    ? state.inventoryItems.length
    : typeof state?.inventoryCount === "number"
      ? state.inventoryCount
      : 0;
  const belt = Array.isArray(state?.beltItems) ? state.beltItems.length : 0;
  return inventory + belt;
}
// Dev/test @MOB seed templates. Deer is intentionally passive in Crystal, so it
// is useful for a low-risk kill/XP proof but invalid as a retaliation target.
const GM_KILL_SPAWN_TEMPLATE = args.killSpawnTemplate ?? args.spawnTemplate ?? "Deer";
const GM_RETALIATION_SPAWN_TEMPLATE = args.retaliationSpawnTemplate ?? args.spawnTemplate ?? "RakingCat0";

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from qa-playthrough.mjs) - captures console, network
// failures, and WebSocket frames (the server's authoritative truth), plus the
// gateway URL the client actually connected to (to flag numerics-proxy-backed).
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
    this.webSocketUrlById = new Map();
    this.webSocketFramesReceived = [];
    this.webSocketFramesSent = [];
    this.webSocketEvents = [];
    this.gatewayUrls = [];
    // Monotonic counter stamped onto every received frame. The frame buffer is capped
    // (slice(-N)), so absolute array indices drift as it rotates - the WS-truth scans
    // must select frames by this stable `seq`, never by array position.
    this.wsRxCount = 0;
    this.wsTxCount = 0;
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
    } else if (m === "Network.webSocketCreated") {
      // The gateway socket the client opened - reveals whether it reached the Rust
      // gateway (:7111, real numerics) or the :7110 node proxy.
      if (p.requestId && p.url) this.webSocketUrlById.set(p.requestId, String(p.url));
      if (p.url) {
        const event = { requestId: p.requestId ?? null, url: String(p.url), at: Date.now() };
        this.gatewayUrls.push(event);
        this.webSocketEvents.push({ type: "created", ...event });
        this.webSocketEvents = this.webSocketEvents.slice(-300);
      }
    } else if (m === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({
        requestId: p.requestId ?? null,
        url: this.webSocketUrlById.get(p.requestId) ?? null,
        payloadData: p.response?.payloadData,
        at: Date.now(),
        seq: this.wsRxCount++,
      });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-4000);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({
        requestId: p.requestId ?? null,
        url: this.webSocketUrlById.get(p.requestId) ?? null,
        payloadData: p.response?.payloadData,
        at: Date.now(),
        seq: this.wsTxCount++,
      });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-800);
    } else if (m === "Network.webSocketClosed") {
      const event = {
        type: "closed",
        requestId: p.requestId ?? null,
        url: this.webSocketUrlById.get(p.requestId) ?? null,
        at: Date.now(),
      };
      this.webSocketEvents.push(event);
      this.webSocketEvents = this.webSocketEvents.slice(-300);
    }
  }

  send(method, params = {}, timeoutMs = 15_000) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`CDP ${method} timed out after ${timeoutMs}ms`));
      }, timeoutMs);
      this.pending.set(id, {
        resolve: (value) => {
          clearTimeout(timer);
          resolve(value);
        },
        reject: (error) => {
          clearTimeout(timer);
          reject(error);
        },
      });
      try {
        this.ws.send(JSON.stringify({ id, method, params }));
      } catch (error) {
        clearTimeout(timer);
        this.pending.delete(id);
        reject(error);
      }
    });
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

  // Best gateway URL guess: the first ws:// frame-bearing socket that isn't the CDP
  // channel itself. We filter to the mir2 gateway path ("/ws").
  gatewayUrl() {
    const hit = this.gatewayUrls.find((g) => /\/ws(\?|$)/.test(g.url));
    return hit?.url ?? this.gatewayUrls[0]?.url ?? null;
  }

  close() {
    this.ws?.close();
  }
}

// ---------------------------------------------------------------------------
// Run context + issue / scorecard collection.
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
    // The six combat/survival checks this harness exists to score. Each is set to
    // "pass" | "fail" | "gap" | "skip" with a note as the run proceeds.
    scorecard: {
      attackKill: { label: "1. Attack a monster -> HP drops and it dies", status: "skip", note: "" },
      damageIndicators: { label: "2. Damage indicators appear (.scene-damage-floater)", status: "skip", note: "" },
      takeDamage: { label: "3. You take damage -> playerHp drops", status: "skip", note: "" },
      deathRevive: { label: "4. On death -> respawn/revive flow works", status: "skip", note: "" },
      loot: { label: "5. Loot a kill -> item enters inventory", status: "skip", note: "" },
      experience: { label: "6. XP / level changes (playerExperience)", status: "skip", note: "" },
      deathAnimation: { label: "(known gap) death/attack/struck animations render", status: "skip", note: "" },
    },
    log: (msg) => console.log(msg),
    score(key, status, note) {
      if (!this.scorecard[key]) return;
      this.scorecard[key].status = status;
      if (note) this.scorecard[key].note = note;
      const icon = status === "pass" ? "PASS" : status === "fail" ? "FAIL" : status === "gap" ? "GAP" : "SKIP";
      console.log(`  ${icon} [${status}] ${this.scorecard[key].label}${note ? ` - ${note}` : ""}`);
    },
    addIssue(issue) {
      const id = `${issue.category}-${String(issues.length + 1).padStart(3, "0")}`;
      const full = { id, severity: "medium", ...issue };
      issues.push(full);
      console.log(`  ISSUE [${full.severity}/${full.category}] ${full.summary}`);
      return full;
    },
  };
}

async function runBeat(ctx, title, fn) {
  ctx.seq += 1;
  const id = ctx.seq;
  const beat = { id, title, startedAt: Date.now(), ok: true };
  ctx.log(`\n> beat ${id}: ${title}`);
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
    ctx.log(`  error: ${beat.error.slice(0, 300)}`);
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
  try {
    await writeReport(ctx);
  } catch (err) {
    ctx.partialReportError = String(err?.message ?? err);
    ctx.log(`  partial report write failed: ${ctx.partialReportError.slice(0, 200)}`);
  }
  return beat;
}

// ---------------------------------------------------------------------------
// In-page state reader - combat/survival view of the authoritative client state
// (window.__mir2Stage5.state). Pulls vitals, the self entity, monsters, ground
// drops, inventory size and the live DOM damage-floater count.
// ---------------------------------------------------------------------------
async function readGameState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const entities = Array.isArray(state.entities) ? state.entities : [];
      const monsters = entities.filter((e) => e && e.kind === "monster");
      const selfId = state.playerObjectId ?? null;
      const selfEntity = entities.find((e) => e && e.objectId === selfId) ?? null;
      const drops = Array.isArray(state.groundDrops) ? state.groundDrops : [];
      const renderPlayer = state.player ?? null;
      const authoritativePlayer = state.authoritativePlayer ?? null;
      const combatSafeZones = [{"map":"0","x":288,"y":616,"size":10},{"map":"1","x":315,"y":82,"size":15},{"map":"2","x":503,"y":483,"size":10}];
      const playerForSafe = authoritativePlayer ?? renderPlayer;
      const knownCombatSafeZone = combatSafeZones.some((z) =>
        String(state.mapFileName ?? "") === z.map &&
        playerForSafe &&
        Math.max(Math.abs(playerForSafe.x - z.x), Math.abs(playerForSafe.y - z.y)) <= z.size
      );
      const compact = (e) => e ? {
        objectId: String(e.objectId), name: e.name ?? null, mapFileName: state.mapFileName ?? null, x: e.x, y: e.y,
        hp: e.hp ?? null, maxHp: e.maxHp ?? null, dead: Boolean(e.dead),
        level: e.level ?? null, disposition: e.disposition ?? null,
        attackAnimation: e.attackAnimation ?? null,
        anim: {
          attack: typeof e.attackStartedAt === "number" ? e.attackStartedAt : null,
          struck: typeof e.struckStartedAt === "number" ? e.struckStartedAt : null,
          die: typeof e.dieStartedAt === "number" ? e.dieStartedAt : null,
          revive: typeof e.reviveStartedAt === "number" ? e.reviveStartedAt : null,
        },
      } : null;
      return {
        capturedAt: Date.now(),
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        mapFileName: state.mapFileName ?? null,
        mapTitle: state.mapTitle ?? null,
        sceneInteractionReady: state.sceneInteractionReady ?? null,
        sceneAssetReadiness: state.sceneAssetReadiness ?? null,
        loadingMapVisible: (document.body?.innerText ?? "").includes("Loading map"),
        playerObjectId: selfId,
        player: renderPlayer ? { x: renderPlayer.x, y: renderPlayer.y, direction: renderPlayer.direction ?? null } : null,
        authoritativePlayer: authoritativePlayer
          ? { x: authoritativePlayer.x, y: authoritativePlayer.y, direction: authoritativePlayer.direction ?? null }
          : null,
        playerHp: typeof state.playerHp === "number" ? state.playerHp : null,
        playerMaxHp: typeof state.playerMaxHp === "number" ? state.playerMaxHp : null,
        playerMp: typeof state.playerMp === "number" ? state.playerMp : null,
        playerExperience: typeof state.playerExperience === "number" ? state.playerExperience : null,
        playerMaxExperience: typeof state.playerMaxExperience === "number" ? state.playerMaxExperience : null,
        playerLevel: selfEntity?.level ?? null,
        rawInSafeZone: Boolean(state.inSafeZone),
        inSafeZone: Boolean(state.inSafeZone) || knownCombatSafeZone,
        gold: typeof state.gold === "number" ? state.gold : null,
        self: compact(selfEntity),
        selfDead: Boolean(selfEntity?.dead),
        entityCount: entities.length,
        monsterCount: monsters.length,
        liveMonsterCount: monsters.filter((m) => !m.dead).length,
        monsters: monsters.slice(0, 30).map(compact),
        groundDropCount: drops.length,
        groundDrops: drops.slice(0, 30).map((d) => ({
          objectId: String(d.objectId), name: d.name ?? null, x: d.x, y: d.y,
          quantity: d.quantity ?? null, icon: d.icon ?? null, sourceMonster: d.sourceMonster ?? null,
        })),
        inventoryCount: Array.isArray(state.inventoryItems) ? state.inventoryItems.length : null,
        inventoryItems: Array.isArray(state.inventoryItems)
          ? state.inventoryItems.slice(0, 60).map((item) => ({
              key: item.key ?? null,
              name: item.name ?? null,
              uniqueId: item.uniqueId ?? null,
              slot: item.slot ?? null,
              container: item.container ?? null,
              quantity: item.quantity ?? null,
            }))
          : [],
        beltItems: Array.isArray(state.beltItems)
          ? state.beltItems.slice(0, 12).map((item) => ({
              key: item.key ?? null,
              name: item.name ?? null,
              uniqueId: item.uniqueId ?? null,
              slot: item.slot ?? null,
              container: item.container ?? null,
              quantity: item.quantity ?? null,
            }))
          : [],
        freeBagSlots: typeof state.freeBagSlots === "number" ? state.freeBagSlots : null,
        damageFloaterDom: document.querySelectorAll(".scene-damage-floater").length,
      };
    })()
  `);
}

// ---------------------------------------------------------------------------
// Render health - quick black-canvas check (Bevy is the renderer).
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

function lumaMean(lumas) {
  if (!Array.isArray(lumas) || !lumas.length) return null;
  return lumas.reduce((a, b) => a + b, 0) / lumas.length;
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

// ---------------------------------------------------------------------------
// Movement + interaction primitives (adapted from qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function sendCommandProbe(client, command) {
  return client.evaluate(`
    (() => {
      const bridge = window.__mir2Stage5;
      const before = window.__mir2LastCommand ?? null;
      const beforeGatewayEvent = window.__mir2LastGatewayEvent ?? null;
      const sent = bridge?.send?.(${JSON.stringify(command)}) === true;
      const state = bridge?.state ?? {};
      return {
        sent,
        screen: state.screen ?? null,
        wsState: state.wsState ?? null,
        before,
        after: window.__mir2LastCommand ?? null,
        beforeGatewayEvent,
        lastGatewayEvent: window.__mir2LastGatewayEvent ?? null,
        lastCommand: state.lastCommand ?? null,
        at: Date.now(),
      };
    })()
  `);
}

async function sendCommand(client, command) {
  const result = await sendCommandProbe(client, command);
  return result?.sent === true;
}

function qaControlCommand(action) {
  if (!QA_CONTROL_TOKEN) return null;
  return { type: "qaControl", token: QA_CONTROL_TOKEN, action };
}

async function sendQaControlProbe(client, action, fallbackCommand = null) {
  const command = qaControlCommand(action) ?? fallbackCommand;
  if (!command) {
    return { sent: false, qaControl: false, error: "QA control token is not configured" };
  }
  const probe = await sendCommandProbe(client, command);
  return { ...probe, qaControl: Boolean(QA_CONTROL_TOKEN), commandType: command.type };
}

async function seedQaGroundDropForPickup(client, beat, itemName = "Blue Potion", itemKey = "blue-potion") {
  const seed = { itemName, itemKey, used: true };
  beat.qaPickupSeed = seed;

  let state = await readGameState(client);
  let item = state.inventoryItems.find((entry) => entry.name === itemName);
  if (!item) {
    seed.giveItem = await sendQaControlProbe(
      client,
      { type: "stage5Command", action: "qa.giveItem", args: [itemKey, "1"] },
      { type: "stage5Command", action: "qa.giveItem", args: [itemKey, "1"] },
    );
    await sendCommand(client, { type: "tick" });
    const inventoryDeadline = Date.now() + QA_SEED_WINDOW_MS;
    while (Date.now() < inventoryDeadline && !item) {
      await delay(150);
      state = await readGameState(client);
      item = state.inventoryItems.find((entry) => entry.name === itemName);
    }
  }

  if (!item || item.uniqueId === null || item.uniqueId === undefined) {
    seed.error = "qa.giveItem did not produce a droppable inventory item";
    seed.state = {
      inventoryItems: state.inventoryItems,
      beltItems: state.beltItems,
    };
    return null;
  }

  const existingDropIds = new Set(state.groundDrops.map((drop) => String(drop.objectId)));
  seed.dropItem = await sendCommandProbe(client, {
    type: "dropItem",
    key: item.key ?? itemKey,
    uniqueId: item.uniqueId,
    count: 1,
    heroInventory: false,
  });
  await sendCommand(client, { type: "tick" });

  const dropDeadline = Date.now() + 10_000;
  let drop = null;
  while (Date.now() < dropDeadline && !drop) {
    await delay(150);
    state = await readGameState(client);
    drop =
      state.groundDrops.find((entry) => entry.name === itemName && !existingDropIds.has(String(entry.objectId))) ??
      state.groundDrops.find((entry) => entry.name === itemName) ??
      null;
  }

  if (!drop) {
    seed.error = "dropItem did not create a visible ground drop";
    seed.state = {
      inventoryItems: state.inventoryItems,
      groundDrops: state.groundDrops,
    };
    return null;
  }

  seed.drop = drop;
  return drop;
}

async function seedQaHostileMonster(client, beat, template = GM_RETALIATION_SPAWN_TEMPLATE, options = {}) {
  const seed = { template, used: true };
  beat.qaCombatSeed = seed;

  const before = await readGameState(client);
  const beforeIds = new Set(before.monsters.map((monster) => String(monster.objectId)));
  seed.before = {
    map: before.mapFileName,
    player: before.authoritativePlayer ?? before.player ?? null,
    liveMonsterCount: before.liveMonsterCount,
  };
  seed.command = await sendQaControlProbe(
    client,
    { type: "stage5Command", action: "event.spawn", args: [template, "1"] },
    { type: "stage5Command", action: "event.spawn", args: [template, "1"] },
  );
  await sendCommand(client, { type: "tick" });

  const deadline = Date.now() + QA_SEED_WINDOW_MS;
  let monster = null;
  const rejectedCandidates = [];
  const wantedName = String(template ?? "").trim().toLowerCase();
  while (Date.now() < deadline && !monster) {
    await delay(180);
    const candidate = await readTargetMonster(client, EXPLORE_SCAN_RANGE, [...beforeIds], {
      requireHostile: true,
      preferHostile: true,
      requireRetaliator: Boolean(options.requireRetaliator),
    });
    if (!candidate) continue;
    const candidateName = String(candidate.name ?? "").trim().toLowerCase();
    if (wantedName && candidateName !== wantedName) {
      if (rejectedCandidates.length < 8) rejectedCandidates.push(candidate);
      continue;
    }
    monster = candidate;
  }

  if (!monster) {
    const after = await readGameState(client).catch(() => null);
    seed.error = `event.spawn did not produce a visible ${template} hostile monster`;
    seed.rejectedCandidates = rejectedCandidates;
    seed.after = after
      ? {
          map: after.mapFileName,
          player: after.authoritativePlayer ?? after.player ?? null,
          liveMonsterCount: after.liveMonsterCount,
          monsters: after.monsters,
        }
      : null;
    return null;
  }

  seed.monster = monster;
  return monster;
}

async function waitForMovementInputReady(client, maxWaitMs = 2_500) {
  const deadline = Date.now() + maxWaitMs;
  while (Date.now() < deadline) {
    const remaining = await client.evaluate(`
      (() => Math.max(0, (window.__mir2Stage5?.movementInputBlockedUntil ?? 0) - Date.now()))()
    `);
    if (!remaining) return true;
    await delay(Math.min(remaining, 200));
  }
  return false;
}

// Fire a GM "@" chat command (best-effort - a no-op for a non-GM, non-test
// account; @MOB/@REVIVE are GM/test-gated, @DIE is ungated).
async function gmCommand(client, message) {
  return sendCommand(client, { type: "chat", message });
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

// Click a tile. Returns false when the tile is off-screen: its aria-label node exists
// in the DOM even when scrolled out of the visible viewport, so a naive click would
// dispatch at off-screen coords and silently do nothing - callers must know it failed
// so they can step in viewport-sized hops instead.
async function clickTile(client, x, y, button = "left") {
  const point = await tilePoint(client, x, y);
  if (!point) return false;
  if (point.x < 2 || point.y < 2 || point.x > VIEWPORT.width - 2 || point.y > VIEWPORT.height - 2) return false;
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

function directionFromPoint(source, target, fallback = "Down") {
  const dx = Math.sign(target.x - source.x);
  const dy = Math.sign(target.y - source.y);
  if (dx === 0 && dy < 0) return "Up";
  if (dx > 0 && dy < 0) return "UpRight";
  if (dx > 0 && dy === 0) return "Right";
  if (dx > 0 && dy > 0) return "DownRight";
  if (dx === 0 && dy > 0) return "Down";
  if (dx < 0 && dy > 0) return "DownLeft";
  if (dx < 0 && dy === 0) return "Left";
  if (dx < 0 && dy < 0) return "UpLeft";
  return fallback;
}

function pointMoveInDirection(source, direction, distance = 1) {
  const delta = MIR_DIRECTION_DELTAS[direction] ?? { x: 0, y: 0 };
  return { x: source.x + delta.x * distance, y: source.y + delta.y * distance };
}

async function waitForPlayerPositionChange(client, from, timeoutMs = 1_400) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
      const next = await client.evaluate(
      `(() => { const s = window.__mir2Stage5?.state ?? {}; const p = s.authoritativePlayer ?? s.player; return p ? { x: p.x, y: p.y } : null; })()`,
    );
    if (next && (next.x !== from.x || next.y !== from.y)) return true;
    await delay(100);
  }
  return false;
}

async function waitForPickupRouteProgress(client, drop, carriedBefore, invBefore, from, timeoutMs = 2_800) {
  const deadline = Date.now() + timeoutMs;
  let moved = false;
  let latest = null;
  while (Date.now() < deadline) {
    latest = await readGameState(client).catch(() => null);
    const self = latest?.authoritativePlayer ?? latest?.self ?? latest?.player ?? null;
    if (self && from && (self.x !== from.x || self.y !== from.y)) {
      moved = true;
    }
    const stillThere = latest?.groundDrops?.some((d) => String(d.objectId) === String(drop.objectId)) ?? true;
    const grew =
      latest &&
      (carriedItemCount(latest) > carriedBefore ||
        (typeof latest.inventoryCount === "number" && latest.inventoryCount > invBefore));
    const onDropTile = self && self.x === drop.x && self.y === drop.y;
    if (!stillThere || grew || onDropTile) {
      return { moved, onDropTile: Boolean(onDropTile), stillThere, grew: Boolean(grew), state: latest };
    }
    await delay(120);
  }
  return { moved, onDropTile: false, stillThere: true, grew: false, state: latest };
}

async function waitForStableGameState(client, settleMs = 1_200, timeoutMs = 10_000) {
  const deadline = Date.now() + timeoutMs;
  let lastKey = null;
  let stableSince = 0;
  let latest = null;
  while (Date.now() < deadline) {
    latest = await readGameState(client).catch(() => null);
    const player = latest?.authoritativePlayer ?? latest?.player ?? null;
    const ready =
      latest?.wsState === "open" &&
      latest?.sceneInteractionReady === true &&
      latest?.loadingMapVisible === false &&
      player;
    const key = ready ? `${latest.mapFileName}:${player.x}:${player.y}:${player.direction ?? ""}` : null;
    if (key && key === lastKey) {
      if (Date.now() - stableSince >= settleMs) return latest;
    } else {
      lastKey = key;
      stableSince = Date.now();
    }
    await delay(180);
  }
  return latest ?? (await readGameState(client));
}

function sortedDirectionsToward(source, target) {
  const direct = directionFromPoint(source, target, source.direction ?? "Down");
  return [...MIR_DIRECTIONS].sort((left, right) => {
    const leftPoint = pointMoveInDirection(source, left);
    const rightPoint = pointMoveInDirection(source, right);
    const leftDistance = chebyshev(leftPoint, target);
    const rightDistance = chebyshev(rightPoint, target);
    if (leftDistance !== rightDistance) return leftDistance - rightDistance;
    if (left === direct) return -1;
    if (right === direct) return 1;
    return MIR_DIRECTIONS.indexOf(left) - MIR_DIRECTIONS.indexOf(right);
  });
}

async function directWalkToward(client, tx, ty, options = {}) {
  const preferRun = options.preferRun === true;
  const runTimeoutMs = Number.isFinite(Number(options.runTimeoutMs)) ? Number(options.runTimeoutMs) : 1_300;
  const walkTimeoutMs = Number.isFinite(Number(options.walkTimeoutMs)) ? Number(options.walkTimeoutMs) : 950;
  const snapshot = await client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const p = s.authoritativePlayer ?? s.player ?? null;
      const entities = Array.isArray(s.entities) ? s.entities : [];
      return {
        player: p ? { x: p.x, y: p.y, direction: p.direction ?? null } : null,
        playerObjectId: s.playerObjectId ?? null,
        occupied: entities
          .filter((e) => e && !e.dead && e.objectId !== s.playerObjectId && typeof e.x === "number" && typeof e.y === "number")
          .map((e) => String(e.x) + "," + String(e.y)),
      };
    })()
  `);
  const self = snapshot?.player;
  if (!self) return false;
  const target = { x: tx, y: ty };
  if (self.x === target.x && self.y === target.y) return true;
  const occupied = new Set(snapshot.occupied ?? []);
  let attempted = false;
  for (const direction of sortedDirectionsToward(self, target)) {
    const actions = [];
    if (preferRun && chebyshev(self, target) >= 2) actions.push({ type: "run", stride: 2, timeoutMs: runTimeoutMs });
    actions.push({ type: "walk", stride: 1, timeoutMs: walkTimeoutMs });
    for (const action of actions) {
      const path = Array.from({ length: action.stride }, (_, index) => pointMoveInDirection(self, direction, index + 1));
      if (path.some((point) => occupied.has(`${point.x},${point.y}`))) continue;
      const sent = await sendCommand(client, { type: action.type, direction });
      if (!sent) continue;
      attempted = true;
      await sendCommand(client, { type: "tick" });
      const moved = await waitForPlayerPositionChange(client, self, action.timeoutMs);
      if (moved) return true;
    }
  }
  return attempted;
}

// Move the player toward (tx,ty). The gateway rejects the convenience `moveTo` command
// on the production path (world_runtime.rs:90 "debug MoveTo is not allowed"), so the
// ONLY way to move is a real viewport click - handleViewportTileAction -> moveToTile
// (Crystal walk/run packets). A far tile is off-screen and unclickable, so we click an
// in-viewport tile a few steps along the way and let the caller's loop repeat. If the
// current renderer has no DOM tile hit layer, fall back to a normal one-step walk packet
// instead of the forbidden debug MoveTo path.
async function walkToward(client, tx, ty) {
  const self = await client.evaluate(
    `(() => { const s = window.__mir2Stage5?.state ?? {}; const p = s.authoritativePlayer ?? s.player; return p ? { x: p.x, y: p.y } : null; })()`,
  );
  if (!self) return false;
  const dx = tx - self.x;
  const dy = ty - self.y;
  const dist = Math.max(Math.abs(dx), Math.abs(dy));
  if (dist === 0) return true;
  // Try progressively shorter hops so we always land on a clickable on-screen tile.
  for (const stepLen of [6, 5, 4, 3, 2, 1]) {
    if (stepLen > dist) continue;
    const sx = self.x + Math.round((dx / dist) * stepLen);
    const sy = self.y + Math.round((dy / dist) * stepLen);
    if (await clickTile(client, sx, sy, "left")) return true;
  }
  return directWalkToward(client, tx, ty);
}

async function transferTo(client, map, x, y) {
  const key = `crystal:${map}:${x}:${y}`;
  const probe = await sendQaControlProbe(
    client,
    { type: "transferMap", key },
    { type: "transferMap", key },
  );
  const deadline = Date.now() + 20_000;
  let lastTickAt = 0;
  while (Date.now() < deadline) {
    const matched = await client.evaluate(
      `(() => { const s = window.__mir2Stage5?.state; const p = s?.authoritativePlayer ?? s?.player; return s?.mapFileName === ${JSON.stringify(map)} && p?.x === ${Number(x)} && p?.y === ${Number(y)}; })()`,
    ).catch(() => false);
    if (matched) return;
    if (Date.now() - lastTickAt > 500) {
      await sendCommand(client, { type: "tick" }).catch(() => {});
      lastTickAt = Date.now();
    }
    await delay(150);
  }
  const debug = await client
    .evaluate(`(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, lastCommand: window.__mir2LastCommand ?? null, stateLastCommand: window.__mir2Stage5?.state?.lastCommand ?? null, lastGatewayEvent: window.__mir2LastGatewayEvent ?? null, gatewayEvents: (window.__mir2GatewayEventHistory ?? []).slice(0, 8), map: window.__mir2Stage5?.state?.mapFileName ?? null, player: window.__mir2Stage5?.state?.authoritativePlayer ?? window.__mir2Stage5?.state?.player ?? null, body: document.body?.innerText?.slice(0, 200) ?? "" }))()`)
    .catch((e) => ({ debugError: String(e) }));
  throw new Error(`Timed out waiting for forced transfer; probe=${JSON.stringify(probe)} debug=${JSON.stringify(debug)}`);
}

async function pushIntoField(ctx, tiles = 18) {
  const client = ctx.client;
  const mapNow = await client.evaluate(`window.__mir2Stage5?.state?.mapFileName ?? null`);
  const field = HUNTING_FIELDS.find((f) => f.map === mapNow) ?? HUNTING_FIELDS[0];
  const target = fieldCombatAnchor(field, tiles);
  const deadline = Date.now() + 20_000;
  let nudged = 0;
  while (Date.now() < deadline) {
    const state = await readGameState(client).catch(() => null);
    const self = state?.authoritativePlayer ?? state?.player;
    if (!self) break;
    const fromSpawn = Math.max(Math.abs(self.x - field.x), Math.abs(self.y - field.y));
    const remaining = Math.max(Math.abs(self.x - target.x), Math.abs(self.y - target.y));
    if (!isKnownFieldSafeZoneState(state) && (remaining <= 2 || fromSpawn >= tiles)) break;
    if (!(await walkToward(client, target.x, target.y))) {
      nudged += 1;
      await walkToward(client, target.x + (nudged % 2 ? 4 : -4), target.y - 2);
    }
    await delay(450);
  }
}

// Re-resolve a specific monster by id (it moves; may die/despawn -> null), including
// its animation-intent flags so we can report whether the client REQUESTED the
// die/struck animation (separate from whether the sprite frames actually render).
async function readMonsterById(client, objectId) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const m = ents.find((e) => e && String(e.objectId) === ${JSON.stringify(String(objectId))});
      if (!m) return null;
      return {
        objectId: String(m.objectId), name: m.name ?? null, mapFileName: s.mapFileName ?? null, x: m.x, y: m.y,
        hp: m.hp ?? null, maxHp: m.maxHp ?? null, dead: Boolean(m.dead),
        struckStartedAt: typeof m.struckStartedAt === "number" ? m.struckStartedAt : null,
        dieStartedAt: typeof m.dieStartedAt === "number" ? m.dieStartedAt : null,
      };
    })()
  `);
}

// Count the live floating combat numbers (Crystal DamageIndicator - the DOM
// ".scene-damage-floater" overlay). Under the Bevy renderer this is the ground-truth
// "damage happened" visual signal.
async function countDamageFloaters(client) {
  const n = await client.evaluate(`document.querySelectorAll(".scene-damage-floater").length`).catch(() => null);
  return typeof n === "number" ? n : 0;
}

// ---------------------------------------------------------------------------
// WebSocket truth layer - the authoritative combat/survival signals.
//
// The client's per-entity `hp` is unreliable for monsters: ObjectHealth carries a
// `percent` and the on-demand snapshot keeps re-asserting the template max, so a
// monster's `entity.hp` can read pinned at maxHp even as the server drives it to 0.
// The server's own packets are the ground truth, so the kill / damage / XP / drop /
// player-death checks read the captured WS frames, not the bridge:
//   DamageIndicator {damage, objectId}   - a damage number applied to objectId
//   ObjectHealth     {objectId, percent}  - objectId's HP percent (0-100)
//   ObjectDied       {objectId, location} - objectId died
//   ObjectRevived    {objectId}           - objectId revived
//   GainExperience   {amount}             - player gained XP
//   LevelChanged     {level, experience}  - player levelled
//   ObjectItem       {objectId, name, location, image} - a ground drop appeared
//   GainedItem       {item}               - an item entered the player's bag
function parseWsFrame(frame) {
  try {
    return JSON.parse(frame.payloadData);
  } catch {
    return null;
  }
}

function sentCommandFramesSince(client, sinceSeq, types = []) {
  const allowed = types.length ? new Set(types) : null;
  const out = [];
  for (const frame of client.webSocketFramesSent) {
    if (typeof frame.seq === "number" && frame.seq < sinceSeq) continue;
    let payload = null;
    try {
      payload = JSON.parse(frame.payloadData);
    } catch {
      continue;
    }
    if (!payload || typeof payload.type !== "string") continue;
    if (allowed && !allowed.has(payload.type)) continue;
    out.push({
      seq: frame.seq,
      requestId: frame.requestId ?? null,
      url: frame.url ?? null,
      at: frame.at,
      payload,
    });
  }
  return out;
}

// Summarize the server's authoritative outcomes across received frames [sinceIdx..].
function scanWsOutcomes(client, sinceIdx, { targetId, playerId } = {}) {
  const out = {
    targetDamageHits: 0,
    targetMinPercent: null,
    targetKilled: false,
    // Crystal melee is directional - a swing hits whatever is in the facing tile, not
    // necessarily the tracked target - and on a single-player field only WE attack, so
    // monster-wide deaths/damage (objectId != player) attribute to us.
    monstersDied: 0,
    monsterDamageHits: 0,
    anyMonsterDied: 0,
    damageIndicatorTotal: 0,
    damageIndicators: [],
    xpGained: 0,
    leveledUp: false,
    levelTo: null,
    drops: [],
    droppedHere: [],
    gainedItems: 0,
    playerDamageHits: 0,
    playerMinPercent: null,
    playerDied: false,
    playerRevived: false,
    objectAttackTotal: 0,
    playerObjectAttack: 0,
    monsterObjectAttack: 0,
    targetObjectAttack: 0,
    objectStruckTotal: 0,
    playerObjectStruck: 0,
    targetObjectStruck: 0,
  };
  const tid = targetId != null ? String(targetId) : null;
  const pid = playerId != null ? String(playerId) : null;
  for (const frame of client.webSocketFramesReceived) {
    if (frame.seq < sinceIdx) continue; // select by stable seq, not array position
    const o = parseWsFrame(frame);
    if (!o || !o.packet) continue;
    const p = o.payload || {};
    const oid = p.objectId != null ? String(p.objectId) : null;
    switch (o.packet) {
      case "ObjectAttack":
        out.objectAttackTotal += 1;
        if (tid && oid === tid) out.targetObjectAttack += 1;
        if (pid && oid === pid) out.playerObjectAttack += 1;
        else if (oid) out.monsterObjectAttack += 1;
        break;
      case "ObjectStruck":
        out.objectStruckTotal += 1;
        if (tid && oid === tid) out.targetObjectStruck += 1;
        if (pid && oid === pid) out.playerObjectStruck += 1;
        break;
      case "DamageIndicator":
        out.damageIndicatorTotal += 1;
        out.damageIndicators.push({ objectId: oid, damage: p.damage ?? null });
        if (tid && oid === tid) out.targetDamageHits += 1;
        if (pid && oid === pid) out.playerDamageHits += 1;
        else if (oid) out.monsterDamageHits += 1;
        break;
      case "ObjectHealth": {
        const pct = typeof p.percent === "number" ? p.percent : null;
        if (pct === null) break;
        if (tid && oid === tid) out.targetMinPercent = out.targetMinPercent === null ? pct : Math.min(out.targetMinPercent, pct);
        if (pid && oid === pid) out.playerMinPercent = out.playerMinPercent === null ? pct : Math.min(out.playerMinPercent, pct);
        break;
      }
      case "ObjectDied":
        out.anyMonsterDied += 1;
        if (tid && oid === tid) out.targetKilled = true;
        if (pid && oid === pid) out.playerDied = true;
        else if (oid) out.monstersDied += 1;
        break;
      case "ObjectRevived":
        if (pid && oid === pid) out.playerRevived = true;
        break;
      case "GainExperience":
        out.xpGained += typeof p.amount === "number" ? p.amount : 0;
        break;
      case "LevelChanged":
        out.leveledUp = true;
        if (typeof p.level === "number") out.levelTo = p.level;
        break;
      case "ObjectItem":
        out.drops.push({ objectId: oid, name: p.name ?? null, x: p.location?.x ?? null, y: p.location?.y ?? null, image: p.image ?? null });
        break;
      case "GainedItem":
      case "GainedQuestItem":
        out.gainedItems += 1;
        break;
      default:
        break;
    }
  }
  return out;
}

function scanHasCombatEvidence(scan) {
  return (
    scan.targetDamageHits > 0 ||
    scan.monsterDamageHits > 0 ||
    scan.targetKilled ||
    scan.monstersDied > 0 ||
    (scan.targetMinPercent !== null && scan.targetMinPercent < 100)
  );
}

async function scanWsOutcomesSettled(client, sinceIdx, options, settleMs = 900) {
  if (settleMs > 0) await delay(settleMs);
  return scanWsOutcomes(client, sinceIdx, options);
}

// A nearby, killable field monster - the weakest (lowest known maxHp, tie-broken by
// distance) live monster within `maxRange` Chebyshev tiles that is NOT a town guard or
// an effectively-invulnerable target (maxHp >= 1000). Guards/bosses are dead-end targets
// for a fresh character and chasing them tends to drag the player into the town safe
// zone where combat is a no-op. Returns null when nothing suitable is in range.
async function readTargetMonster(client, maxRange, excludeIds = [], options = {}) {
  const range = Number.isFinite(maxRange) ? maxRange : 9999;
  const exclJson = JSON.stringify(excludeIds.map(String));
  const requireHostile = Boolean(options.requireHostile);
  const preferHostile = Boolean(options.preferHostile);
  const requireRetaliator = Boolean(options.requireRetaliator);
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const p = s.authoritativePlayer ?? s.player ?? { x: 0, y: 0 };
      const combatSafeZones = [{"map":"0","x":288,"y":616,"size":10},{"map":"1","x":315,"y":82,"size":15},{"map":"2","x":503,"y":483,"size":10}];
      const inKnownCombatSafeZone = combatSafeZones.some((z) =>
        String(s.mapFileName ?? "") === z.map &&
        Math.max(Math.abs(p.x - z.x), Math.abs(p.y - z.y)) <= z.size
      );
      if (s.inSafeZone || inKnownCombatSafeZone) return null;
      const excl = new Set(${exclJson});
      const cheb = (e) => Math.max(Math.abs(e.x - p.x), Math.abs(e.y - p.y));
      const isGuard = (e) => /guard|archer|sentry|royal|solider|soldier|lieutenant/i.test(e.name || "");
      const invuln = (e) => typeof e.maxHp === "number" && e.maxHp >= 1000;
      const requireHostile = ${JSON.stringify(requireHostile)};
      const preferHostile = ${JSON.stringify(preferHostile)};
      const requireRetaliator = ${JSON.stringify(requireRetaliator)};
      // Harvestable scenery (ChestnutTree, herbs, ...) is kind:"monster" but cannot be
      // melee-killed (you Harvest it), so it is a dead-end attack target - exclude it.
      const isProp = (e) => /scarecrow|training|dummy|tree|chestnut|herb|mushroom|stump|flower|sprout|sapling|shrub|\\bbush\\b|\\brock\\b|\\bore\\b|vein|plant|\\blog\\b|grass|reed|fungus/i.test(e.name || "");
      const isPassiveAnimal = (e) => /^(deer|hen|chicken|pig|cow|sheep)$/i.test(String(e.name || "").trim());
      const mons = ents.filter(
        (e) => e && e.kind === "monster" && !e.dead && !isGuard(e) && !invuln(e) && !isProp(e) && !excl.has(String(e.objectId)) && (!requireHostile || e.disposition === "hostile") && (!requireRetaliator || !isPassiveAnimal(e)) && cheb(e) <= ${range},
      );
      if (!mons.length) return null;
      mons.sort((a, b) => {
        if (preferHostile && a.disposition !== b.disposition) {
          if (a.disposition === "hostile") return -1;
          if (b.disposition === "hostile") return 1;
        }
        const am = typeof a.maxHp === "number" ? a.maxHp : 99999;
        const bm = typeof b.maxHp === "number" ? b.maxHp : 99999;
        if (am !== bm) return am - bm;
        return cheb(a) - cheb(b);
      });
      const m = mons[0];
      return { objectId: String(m.objectId), name: m.name ?? null, mapFileName: s.mapFileName ?? null, x: m.x, y: m.y, hp: m.hp ?? null, maxHp: m.maxHp ?? null, dead: Boolean(m.dead), disposition: m.disposition ?? null, distance: cheb(m), monsterCount: mons.length };
    })()
  `);
}

async function readPlayerObjectId(client) {
  return client.evaluate(`window.__mir2Stage5?.state?.playerObjectId ?? null`);
}

async function readPlayerHp(client) {
  return client.evaluate(`(() => { const s = window.__mir2Stage5?.state; return typeof s?.playerHp === "number" ? s.playerHp : null; })()`);
}

// The tile one step in front of the monster on the player's side - walk here to get
// melee-adjacent (clicking the monster's OWN tile attacks, it does not walk; only
// NPCs auto-approach in activateEntity, apps/web/app/page.tsx:5997).
function stepTowardTile(player, monster) {
  const dx = Math.sign(monster.x - player.x);
  const dy = Math.sign(monster.y - player.y);
  return { x: monster.x - dx, y: monster.y - dy };
}

function chebyshev(a, b) {
  return Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
}

function meleeApproachTile(player, monster, iteration = 0) {
  const preferred = stepTowardTile(player, monster);
  const candidates = [
    preferred,
    ...MIR_DIRECTIONS.map((direction) => pointMoveInDirection(monster, direction)),
  ];
  const unique = new Map();
  for (const tile of candidates) {
    if (tile.x === monster.x && tile.y === monster.y) continue;
    unique.set(`${tile.x},${tile.y}`, tile);
  }
  const sorted = [...unique.values()].sort((left, right) => {
    const leftDistance = chebyshev(player, left);
    const rightDistance = chebyshev(player, right);
    if (leftDistance !== rightDistance) return leftDistance - rightDistance;
    const leftLine = Math.abs(left.x - monster.x) + Math.abs(left.y - monster.y);
    const rightLine = Math.abs(right.x - monster.x) + Math.abs(right.y - monster.y);
    return leftLine - rightLine;
  });
  return sorted[Math.floor(iteration / 6) % sorted.length] ?? preferred;
}

async function stepOffOverlappingMonster(client, self, monster) {
  if (!self || !monster || self.x !== monster.x || self.y !== monster.y) return false;
  const directions = ["Left", "Right", "Up", "Down", "UpLeft", "UpRight", "DownLeft", "DownRight"];
  for (const direction of directions) {
    const tile = pointMoveInDirection(monster, direction);
    await walkToward(client, tile.x, tile.y);
    await delay(520);
    const moved = await client.evaluate(
      `(() => { const s = window.__mir2Stage5?.state ?? {}; const p = s.authoritativePlayer ?? s.player; return p ? { x: p.x, y: p.y } : null; })()`,
    );
    if (moved && (moved.x !== monster.x || moved.y !== monster.y)) return true;
  }
  return false;
}

// Chase a monster and swing at it with a direct attack (== attackTarget,
// page.tsx:4945 send({type:"attack",objectId})). Outcomes are read from the WS truth
// layer (the client's monster `hp` is unreliable). Because Crystal melee is directional
// and only WE attack on the field, a kill/damage is detected map-wide (any monster), not
// pinned to the tracked id. Stops on a monster death, the player's own death, when the
// target becomes unreachable (likely fled / behind a safe-zone wall), or on timeout.
async function fightMonster(client, monster, windowMs, playerId) {
  const wsBase = client.wsRxCount;
  const targetId = String(monster.objectId);
  let swings = 0;
  let floaterPeak = 0;
  let dieIntent = false;
  let struckIntent = false;
  let playerDied = false;
  let reachedMelee = false;
  let abandoned = false;
  let unresponsive = false;
  let farIters = 0; // consecutive iterations unable to reach melee - give up (unreachable)
  const deadline = Date.now() + windowMs;
  while (Date.now() < deadline) {
    const scan = scanWsOutcomes(client, wsBase, { targetId, playerId });
    if (scan.monstersDied > 0 || scan.targetKilled) break;
    if (scan.playerDied || (await readPlayerHp(client)) === 0) {
      playerDied = true;
      break;
    }
    // Many swings in melee but the server never struck the target = it isn't a valid
    // live target here: a phantom/corpse the client still shows but the server removed
    // (common on a long-lived, hunted-out field), or it's blocked just out of true range
    // (the predicted self can lead the server by up to 2 tiles). Give up so the caller
    // can exclude this id and try a different monster.
    if (swings >= 16 && !struckIntent && scan.monsterDamageHits === 0 && scan.targetDamageHits === 0) {
      const settled = await scanWsOutcomesSettled(client, wsBase, { targetId, playerId }, 1800);
      const latest = await readMonsterById(client, monster.objectId).catch(() => null);
      const stateShowsDamage =
        latest &&
        typeof latest.hp === "number" &&
        typeof latest.maxHp === "number" &&
        latest.hp < latest.maxHp;
      if (scanHasCombatEvidence(settled) || stateShowsDamage || latest?.struckStartedAt) {
        if (latest?.struckStartedAt) struckIntent = true;
      } else {
        unresponsive = true;
        abandoned = true;
        break;
      }
    }
    const m = await readMonsterById(client, monster.objectId);
    if (m?.struckStartedAt) struckIntent = true;
    if (m?.dieStartedAt) dieIntent = true;
    if (!m) {
      // The tracked monster left the client view. If a monster died this fight it was
      // our kill; otherwise re-acquire a nearby one to keep swinging at the spot.
      if (scanWsOutcomes(client, wsBase, { playerId }).monstersDied > 0) break;
      const next = await readTargetMonster(client, 3);
      if (!next) {
        abandoned = true;
        break;
      }
      monster = next;
    } else {
      // Approach with the same server-ACK position used for action gating. CRUCIAL:
      // never walk and attack in the same step - an attack arms movementInputBlockedUntil,
      // which suppresses the very next moveToTile, so attacking while approaching freezes
      // the player just out of range. So: ONLY walk when far, ONLY swing when adjacent.
      const self = await client.evaluate(
        `(() => { const s = window.__mir2Stage5?.state ?? {}; const p = s.authoritativePlayer ?? s.player; return p ? { x: p.x, y: p.y } : null; })()`,
      );
      if (self && self.x === m.x && self.y === m.y) {
        await stepOffOverlappingMonster(client, self, m);
        continue;
      }
      if (self && chebyshev(m, self) <= 1) {
        reachedMelee = true;
        farIters = 0;
        // Swing via a click on the monster's (in-viewport) tile, == activateEntity ->
        // attackTarget; the server auto-faces and the hit lands once we're truly adjacent.
        if (!(await clickTile(client, m.x, m.y, "left"))) {
          await sendCommand(client, { type: "attack", objectId: Number(monster.objectId) });
        }
        swings += 1;
        await delay(560); // melee attack cadence
      } else {
        farIters += 1;
        if (farIters > 70) {
          abandoned = true; // unable to close - fled / unreachable
          break;
        }
        // Walk toward the tile ADJACENT to the monster, never its own tile: clicking the
        // monster's tile is an attack (handleViewportTileAction), not a walk, which would
        // whiff from range and block the next move. Walk only - no attack here.
        const step = self ? meleeApproachTile(self, m, farIters) : { x: m.x, y: m.y };
        await directWalkToward(client, step.x, step.y, { preferRun: false, walkTimeoutMs: 1_150 });
        await delay(450);
      }
    }
    floaterPeak = Math.max(floaterPeak, await countDamageFloaters(client));
  }
  if (swings > 0 || reachedMelee) {
    await delay(900);
  }
  floaterPeak = Math.max(floaterPeak, await countDamageFloaters(client));
  const finalM = await readMonsterById(client, monster.objectId);
  if (finalM?.dieStartedAt) dieIntent = true;
  if (finalM?.struckStartedAt) struckIntent = true;
  const scan = await scanWsOutcomesSettled(client, wsBase, { targetId, playerId }, 600);
  const killed = scan.targetKilled || scan.monstersDied > 0;
  const stateTargetDamaged =
    finalM &&
    typeof finalM.hp === "number" &&
    typeof finalM.maxHp === "number" &&
    finalM.hp < finalM.maxHp;
  const damaged =
    scan.monsterDamageHits > 0 ||
    scan.targetDamageHits > 0 ||
    (scan.targetMinPercent !== null && scan.targetMinPercent < 100) ||
    stateTargetDamaged;
  if (damaged || killed) {
    unresponsive = false;
  }
  return {
    objectId: targetId,
    name: monster.name,
    maxHp: monster.maxHp ?? null,
    swings,
    killed,
    damaged,
    playerDied,
    reachedMelee,
    abandoned,
    unresponsive,
    monstersDied: scan.monstersDied,
    monsterDamageHits: scan.monsterDamageHits,
    minPercent:
      scan.targetMinPercent ??
      (stateTargetDamaged && typeof finalM.hp === "number" && typeof finalM.maxHp === "number" && finalM.maxHp > 0
        ? Math.max(0, Math.round((finalM.hp / finalM.maxHp) * 100))
        : null),
    damageFloaterPeak: floaterPeak,
    serverDamageNumbers: scan.monsterDamageHits + scan.targetDamageHits,
    xpGainedDuringFight: scan.xpGained,
    dropsDuringFight: scan.drops,
    dieAnimationIntent: dieIntent,
    struckAnimationIntent: struckIntent,
  };
}

// Find a live, killable field monster the way a real player would: hop to a hunting
// field and EXPLORE it. The killable monsters spawn deeper in the field, away from the
// guarded town edge where the transfer drops you, so radiate waypoints out from the
// field spawn and scan for a non-guard / non-invulnerable monster after each move (with
// a best-effort @MOB, a no-op unless GM/test). Returns null if none turns up in time.
async function acquireMonster(ctx, beat, excluded = [], options = {}) {
  const client = ctx.client;
  const visited = beat?.visitedFields ?? [];
  if (beat) beat.visitedFields = visited;
  const spawnTemplate = options.spawnTemplate ?? GM_KILL_SPAWN_TEMPLATE;
  const allowQaSeed = Boolean(options.allowQaSeed) && ALLOW_QA_COMBAT_SEED;
  const preferQaSeed = Boolean(options.preferQaSeed) && PREFER_QA_COMBAT_SEED;

  const start = await readGameState(client).catch(() => null);
  if (HUNTING_FIELDS.some((f) => f.map === start?.mapFileName) && isKnownFieldSafeZoneState(start)) {
    const field = fieldForMap(start?.mapFileName) ?? HUNTING_FIELDS[0];
    const anchor = fieldCombatAnchor(field, 18);
    await transferTo(client, field.map, anchor.x, anchor.y).catch((err) =>
      ctx.addIssue({ category: "flow", severity: "low", beat: beat?.id, summary: `transfer to ${field.label} combat anchor failed`, detail: String(err?.message ?? err) }),
    );
  }

  if (preferQaSeed) {
    ctx.log(`  * using QA-seeded ${spawnTemplate} to verify hostile combat deterministically`);
    const seeded = await seedQaHostileMonster(client, beat, spawnTemplate, options);
    if (seeded) return seeded;
  }

  let monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded, options);
  if (monster) return monster;
  if (preferQaSeed) {
    monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded, options);
    if (monster) return monster;
  }

  // Two rings x 8 compass directions out from the field spawn.
  const ring = (d) => [
    [0, -d], [d, 0], [0, d], [-d, 0],
    [d, -d], [d, d], [-d, d], [-d, -d],
  ];
  const offsets = [...ring(13), ...ring(24)];

  for (const field of HUNTING_FIELDS) {
    if (monster) break;
    const here = await client.evaluate(
      `window.__mir2Stage5?.state?.mapFileName === ${JSON.stringify(field.map)}`,
    );
    if (!here) {
      const anchor = fieldCombatAnchor(field, 18);
      await transferTo(client, field.map, anchor.x, anchor.y).catch((err) =>
        ctx.addIssue({ category: "flow", severity: "low", beat: beat?.id, summary: `transfer to ${field.label} failed`, detail: String(err?.message ?? err) }),
      );
      await waitUntilSoft(
        client,
        "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
        20_000,
      );
    }
    if (!visited.includes(field.label)) visited.push(field.label);
    await pushIntoField(ctx, 18);

    const deadline = Date.now() + SPAWN_POLL_MS;
    let wp = 0;
    while (Date.now() < deadline && !monster) {
      monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded, options);
      if (monster) break;
      if (spawnTemplate) {
        await gmCommand(client, `@MOB ${spawnTemplate}`).catch(() => {});
        await delay(250);
        monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded, options);
        if (monster) break;
      }
      const [ox, oy] = offsets[wp % offsets.length];
      wp += 1;
      const anchor = fieldCombatAnchor(field, 18);
      const tx = anchor.x + ox;
      const ty = anchor.y + oy;
      // Walk toward the waypoint in in-viewport hops (direct moveTo is rejected on the
      // production path), scanning as we go.
      const legDeadline = Date.now() + 5000;
      while (Date.now() < legDeadline && !monster) {
        monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded, options);
        if (monster) break;
        await walkToward(client, tx, ty);
        await delay(700);
      }
    }
    if (monster) break;
  }
  if (!monster && allowQaSeed && !preferQaSeed) {
    ctx.log(`  * no hostile field monster found; using QA-seeded ${spawnTemplate} as fallback`);
    monster = await seedQaHostileMonster(client, beat, spawnTemplate, options);
  }
  return monster;
}

// ---------------------------------------------------------------------------
// The combat / survival cycle.
// ---------------------------------------------------------------------------
async function combatSurvival(ctx) {
  const client = ctx.client;

  await enterWorld(ctx);

  // Hop to a hunting field up front so every later beat operates where monsters spawn.
  await runBeat(ctx, "reach a hunting field", async (beat) => {
    const before = await readGameState(client);
    const preferredField = HUNTING_FIELDS[0];
    const preferredAnchor = fieldCombatAnchor(preferredField, 18);
    const currentField = fieldForMap(before.mapFileName);
    const player = before.authoritativePlayer ?? before.player;
    const farFromPreferredStarter =
      currentField?.map === preferredField.map &&
      player &&
      Math.max(Math.abs(player.x - preferredAnchor.x), Math.abs(player.y - preferredAnchor.y)) > EXPLORE_SCAN_RANGE;
    if (!currentField || farFromPreferredStarter) {
      await transferTo(client, preferredField.map, preferredAnchor.x, preferredAnchor.y).catch((err) =>
        ctx.addIssue({ category: "flow", severity: "high", beat: beat.id, summary: `could not reach hunting field ${preferredField.label}`, detail: String(err?.message ?? err) }),
      );
    } else {
      beat.note = `already on a hunting field (${before.mapFileName})`;
    }
    await waitUntilSoft(
      client,
      "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
      SCENE_READY_TIMEOUT_MS,
    );
    await delay(900);
    const after = await readGameState(client);
    if (isKnownFieldSafeZoneState(after)) {
      await pushIntoField(ctx, 18);
    }
    const finalState = await readGameState(client);
    beat.map = finalState.mapFileName;
    beat.monsterCount = finalState.monsterCount;
    beat.inSafeZone = isKnownFieldSafeZoneState(finalState);
    const grid = await captureLumaGrid(client).catch(() => null);
    const mean = lumaMean(grid);
    if (mean !== null && mean < BLACK_LUMA_MEAN) {
      ctx.addIssue({ category: "render", severity: "high", beat: beat.id, summary: `hunting field canvas is effectively black (mean luma ${mean.toFixed(1)})`, detail: { map: finalState.mapFileName } });
    }
    if (!HUNTING_FIELDS.some((f) => f.map === finalState.mapFileName)) {
      ctx.addIssue({ category: "flow", severity: "high", beat: beat.id, summary: `not on a hunting field (on "${finalState.mapFileName ?? "?"}") - combat beats may find no monsters`, detail: { map: finalState.mapFileName } });
    }
    if (isKnownFieldSafeZoneState(finalState)) {
      ctx.addIssue({ category: "flow", severity: "high", beat: beat.id, summary: "still in a safe zone after field push; combat beats would be no-op", detail: { map: finalState.mapFileName, player: finalState.player } });
    }
  });

  // Baseline vitals/inventory captured before any fighting (for deltas later).
  const baseline = await readGameState(client);
  const playerId = await readPlayerObjectId(client);
  ctx.playerId = playerId;
  ctx.baseline = {
    playerHp: baseline.playerHp,
    playerMaxHp: baseline.playerMaxHp,
    playerLevel: baseline.playerLevel,
    inventoryCount: baseline.inventoryCount,
    freeBagSlots: baseline.freeBagSlots,
    gold: baseline.gold,
  };

  // ---- Beats 1, 2 & 6 share one hunt: fight a monster to the death and observe
  // HP-drop / death (1), damage numbers (2) and XP gain (6) - ALL read from the WS
  // truth layer (the client's monster `hp` is unreliable). ----
  const huntWsBase = client.wsRxCount;
  const fights = [];
  let killRecord = null;
  const excludedMonsters = [];
  await runBeat(ctx, "combat: attack a monster until it dies", async (beat) => {
    let acquiredAny = false;
    let playerDiedInHunt = false;
    let unresponsiveCount = 0;
    for (let attempt = 0; attempt < MAX_FIGHTS && !killRecord; attempt += 1) {
      const monster = await acquireMonster(ctx, beat, excludedMonsters);
      if (!monster) break;
      acquiredAny = true;
      ctx.log(`  * attempt ${attempt + 1}/${MAX_FIGHTS}: ${monster.name ?? monster.objectId} @ (${monster.x},${monster.y}) maxHp=${monster.maxHp ?? "?"}`);
      const rec = await fightMonster(client, monster, COMBAT_WINDOW_MS, playerId);
      fights.push(rec);
      ctx.log(`    -> swings=${rec.swings} killed=${rec.killed} damageNumbers=${rec.serverDamageNumbers} unresponsive=${rec.unresponsive} playerDied=${rec.playerDied}`);
      if (rec.killed) killRecord = rec;
      // A target that never registered a hit is a phantom/unreachable entity - exclude
      // it so the next attempt engages a different monster.
      if (rec.unresponsive && rec.objectId) {
        excludedMonsters.push(rec.objectId);
        unresponsiveCount += 1;
      }
      if (rec.playerDied) {
        playerDiedInHunt = true;
        break;
      }
    }
    beat.fights = fights;
    beat.playerDiedInHunt = playerDiedInHunt;
    beat.unresponsiveCount = unresponsiveCount;

    const anyDamage = fights.some((f) => f.damaged);
    const floaterPeak = fights.reduce((m, f) => Math.max(m, f.damageFloaterPeak), 0);
    const serverDamageNumbers = fights.reduce((m, f) => m + f.serverDamageNumbers, 0);
    const dieIntent = fights.some((f) => f.dieAnimationIntent);
    const struckIntent = fights.some((f) => f.struckAnimationIntent);
    const allUnresponsive = fights.length > 0 && fights.every((f) => f.unresponsive);
    beat.summary = { acquiredAny, killed: Boolean(killRecord), playerDiedInHunt, anyDamage, floaterPeak, serverDamageNumbers };

    // Beat 1 - attack -> HP drops and it dies (server ObjectDied is the truth).
    if (!acquiredAny) {
      ctx.score("attackKill", "skip", "no monster available on the hunting fields");
      ctx.addIssue({ category: "combat", severity: "medium", beat: beat.id, summary: "no monster available to exercise combat (none on the fields; @MOB spawn is GM/test-gated)", detail: { visited: beat.visitedFields } });
    } else if (killRecord) {
      ctx.score("attackKill", "pass", `killed "${killRecord.name}" - server ObjectDied (${killRecord.serverDamageNumbers} damage number(s) over ${killRecord.swings} swings)`);
    } else if (playerDiedInHunt) {
      ctx.score("attackKill", "fail", "the monster killed the player before it died (fresh level-1 character is fragile vs field monsters)");
      ctx.addIssue({ category: "combat", severity: "medium", beat: beat.id, summary: "combat is mutual and lethal but the player died before landing a kill (fresh level-1 character vs field monster - no GM/test powers to set up a winnable fight)", detail: fights });
    } else if (anyDamage) {
      ctx.score("attackKill", "fail", "monster took server-side damage but did not die within the window");
      ctx.addIssue({ category: "combat", severity: "high", beat: beat.id, summary: "attacking drove the monster's server HP down but it did not die within the combat window", detail: fights });
    } else if (allUnresponsive) {
      // Every target accepted swings but the server never struck back - they are
      // phantom/stale entities (a long-lived field hunted out, so the client still shows
      // corpses the server already removed) or are wedged just out of true range by the
      // 2-tile client prediction lead. The combat PIPE is unproven here, not broken.
      ctx.score("attackKill", "gap", `attacked ${fights.length} target(s) but none responded - stale/unreachable field entities (likely a hunted-out field on the shared sim)`);
      ctx.addIssue({
        category: "combat",
        severity: "medium",
        beat: beat.id,
        summary: `swung at ${fights.length} monster(s) in melee but the server emitted no ObjectStruck/DamageIndicator for any - they appear to be phantom/stale entities the server has already removed (field hunted out on the shared persistent sim) or wedged out of range by client prediction`,
        detail: { excluded: excludedMonsters, fights },
      });
    } else {
      const swungInMelee = fights.some((f) => f.reachedMelee && f.swings > 0);
      ctx.score("attackKill", "fail", swungInMelee ? "swung in melee but the server registered no damage" : "could not engage a monster in melee");
      ctx.addIssue({
        category: "combat",
        severity: "high",
        beat: beat.id,
        summary: swungInMelee
          ? "swung at a monster in melee range but the server registered no damage - possible town safe zone (combat no-op) or out-of-range desync"
          : "could not reach a monster to attack (target fled / unreachable)",
        detail: fights,
      });
    }

    // Beat 2 - damage indicators. The server's DamageIndicator is the source signal;
    // .scene-damage-floater is the render. Pass when the numbers are actually flowing.
    if (!acquiredAny) {
      ctx.score("damageIndicators", "skip", "no combat occurred");
    } else if (serverDamageNumbers > 0 && floaterPeak > 0) {
      ctx.score("damageIndicators", "pass", `${serverDamageNumbers} server DamageIndicator(s); DOM .scene-damage-floater peak ${floaterPeak}`);
    } else if (serverDamageNumbers > 0) {
      ctx.score("damageIndicators", "fail", `server sent ${serverDamageNumbers} DamageIndicator(s) but no .scene-damage-floater rendered`);
      ctx.addIssue({ category: "combat", severity: "medium", beat: beat.id, summary: "server sent damage numbers but no floating damage numbers (.scene-damage-floater) rendered in the DOM", detail: { serverDamageNumbers, floaterPeak } });
    } else if (anyDamage) {
      ctx.score("damageIndicators", "fail", "monster HP fell but no damage numbers were emitted");
      ctx.addIssue({ category: "combat", severity: "medium", beat: beat.id, summary: "monster server HP fell but no DamageIndicator / .scene-damage-floater appeared", detail: fights });
    } else {
      ctx.score("damageIndicators", "skip", "no damage landed to indicate");
    }

    // Known gap - death/attack/struck ANIMATION. We observe whether the CLIENT set the
    // animation intent flags; whether the action sprite frames RENDER is the R2-only /
    // meta-shadowing gap (project memory actor-sprite-lib-truncation; fix in flight on
    // branch claude/fervent-wescoff-af8df9).
    if (acquiredAny && killRecord) {
      if (dieIntent || struckIntent) {
        ctx.score("deathAnimation", "gap", `client set animation intent (die=${dieIntent} struck=${struckIntent}); action sprite frames are a known R2-only/meta-shadowing gap`);
        ctx.addIssue({ category: "animation", severity: "low", beat: beat.id, summary: "death/struck animation INTENT is set by the client, but action sprite frames are a known R2-only/meta-shadowing gap (not rendered)", detail: { dieIntent, struckIntent, note: "Logical combat outcome verified via the WS truth layer; tracking branch claude/fervent-wescoff-af8df9." } });
      } else {
        ctx.score("deathAnimation", "fail", "client never set die/struck animation intent on a killed monster");
        ctx.addIssue({ category: "animation", severity: "medium", beat: beat.id, summary: "a monster died (server ObjectDied) but the client never set die/struck animation intent flags", detail: fights });
      }
    } else {
      ctx.score("deathAnimation", "skip", "no kill to observe an animation on");
    }
  });

  // Beat 6 - XP / level changed across the hunt (server GainExperience / LevelChanged).
  // The wire is the pass/fail source; state.playerExperience is captured as supporting
  // telemetry when available.
  await runBeat(ctx, "progression: XP / level changes from kills", async (beat) => {
    const hunt = scanWsOutcomes(client, huntWsBase, { playerId });
    const xpState = await readGameState(client).catch(() => null);
    beat.xp = {
      xpGained: hunt.xpGained,
      leveledUp: hunt.leveledUp,
      levelTo: hunt.levelTo,
      killed: Boolean(killRecord),
      stateExperience: xpState?.playerExperience ?? null,
      stateMaxExperience: xpState?.playerMaxExperience ?? null,
    };
    if (hunt.xpGained > 0 || hunt.leveledUp) {
      ctx.score("experience", "pass", `gained ${hunt.xpGained} XP${hunt.leveledUp ? ` and levelled to ${hunt.levelTo}` : ""} (server GainExperience)`);
    } else if (killRecord) {
      ctx.score("experience", "fail", "killed a monster but no GainExperience packet arrived");
      ctx.addIssue({ category: "progression", severity: "high", beat: beat.id, summary: "killed a monster (server ObjectDied) but no GainExperience was sent", detail: beat.xp });
    } else {
      ctx.score("experience", "skip", "no kill landed, so no XP was expected");
    }
  });

  // Beat 5 - loot a kill: the drop lands on the corpse tile (server ObjectItem). Walk
  // onto it and pick it up; verify the item enters inventory (state.inventoryItems
  // grows, or a GainedItem packet arrives).
  await runBeat(ctx, "loot: pick up a ground drop after a kill", async (beat) => {
    if ((await readPlayerHp(client)) === 0) {
      ctx.score("loot", "skip", "player is dead - cannot loot");
      beat.note = "player dead before loot";
      return;
    }
    const lootWsBase = client.wsRxCount;
    const lootWsTxBase = client.wsTxCount;
    const beforeLootState = await readGameState(client);
    const invBefore = beforeLootState.inventoryCount ?? 0;
    const carriedBefore = carriedItemCount(beforeLootState);
    beat.inventoryBefore = invBefore;
    beat.carriedBefore = carriedBefore;

    // Prefer the drop the server announced from our kill; else any nearby ground drop.
    const fromKill = killRecord?.dropsDuringFight?.[0] ?? null;
    let drop = null;
    let dropSource = fromKill ? "kill" : "nearby";
    let triedQaPickupSeed = false;
    if (!fromKill && ALLOW_QA_PICKUP_SEED && PREFER_QA_PICKUP_SEED) {
      dropSource = "qa-seed";
      triedQaPickupSeed = true;
      ctx.log("  * using QA-seeded Blue Potion to verify pickup chain deterministically");
      drop = await seedQaGroundDropForPickup(client, beat);
    }
    const dropDeadline = Date.now() + 4_000;
    while (Date.now() < dropDeadline && !drop) {
      const s = await readGameState(client);
      if (fromKill) {
        drop =
          s.groundDrops.find((d) => String(d.objectId) === String(fromKill.objectId)) ??
          (typeof fromKill.x === "number" ? { objectId: fromKill.objectId, name: fromKill.name, x: fromKill.x, y: fromKill.y } : null);
      }
      if (!drop && s.groundDrops.length) {
        const p = s.authoritativePlayer ?? s.player ?? { x: 0, y: 0 };
        drop = [...s.groundDrops].sort((a, b) => (Math.abs(a.x - p.x) + Math.abs(a.y - p.y)) - (Math.abs(b.x - p.x) + Math.abs(b.y - p.y)))[0];
      }
      if (!drop) await delay(400);
    }
    if (!drop && ALLOW_QA_PICKUP_SEED && !triedQaPickupSeed) {
      dropSource = "qa-seed";
      ctx.log("  * no kill/nearby drop available; using QA-seeded Blue Potion to verify pickup chain");
      drop = await seedQaGroundDropForPickup(client, beat);
    }
    if (!drop) {
      ctx.score("loot", "skip", "no ground drop available to loot (drop RNG / drops cleared)");
      ctx.addIssue({ category: "loot", severity: "low", beat: beat.id, summary: "no ground drop available after the kill - could not exercise pickup", detail: { inventoryBefore: invBefore, killDrop: fromKill } });
      return;
    }
    beat.drop = drop;
    beat.dropSource = dropSource;
    ctx.log(`  * looting "${drop.name ?? drop.objectId}" @ (${drop.x},${drop.y}) source=${dropSource}`);

    // Approach + pick up. Prefer the real client path: clicking the drop tile sets the
    // pending pickup target, runs the Crystal movement router, then auto-fires PickUp
    // once the server says the player is standing on the item. If the tile is not
    // clickable, fall back to explicit one-step movement packets.
    let reached = false;
    let lastDropClickAt = 0;
    beat.pickupAttempts = [];
    await waitForMovementInputReady(client);
    const lootDeadline = Date.now() + LOOT_WINDOW_MS;
    while (Date.now() < lootDeadline) {
      const s = await readGameState(client);
      const stillThere = s.groundDrops.some((d) => String(d.objectId) === String(drop.objectId));
      const grewNow =
        carriedItemCount(s) > carriedBefore ||
        (typeof s.inventoryCount === "number" && s.inventoryCount > invBefore);
      if (!stillThere || grewNow) break;
      const pickupSelf = s.authoritativePlayer ?? s.self ?? s.player;
      const onDropTile = pickupSelf && pickupSelf.x === drop.x && pickupSelf.y === drop.y;
      if (onDropTile) {
        reached = true;
        const objectPickup = await sendCommandProbe(client, { type: "pickUp", objectId: Number(drop.objectId) });
        const tilePickup = await sendCommandProbe(client, { type: "pickUpTile" });
        beat.pickupAttempts.push({
          at: Date.now(),
          self: pickupSelf ? { x: pickupSelf.x, y: pickupSelf.y } : null,
          renderPlayer: s.player ? { x: s.player.x, y: s.player.y } : null,
          entitySelf: s.self ? { x: s.self.x, y: s.self.y } : null,
          drop: { objectId: drop.objectId, x: drop.x, y: drop.y },
          objectPickup,
          tilePickup,
        });
      } else {
        const now = Date.now();
        const clickedDropTile = now - lastDropClickAt > 1_250
          ? await clickTile(client, drop.x, drop.y, "left")
          : false;
        if (clickedDropTile) {
          lastDropClickAt = now;
          await sendCommand(client, { type: "tick" });
          const progress = await waitForPickupRouteProgress(
            client,
            drop,
            carriedBefore,
            invBefore,
            pickupSelf,
            5_000,
          );
          beat.pickupRouteProgress = [
            ...(beat.pickupRouteProgress ?? []),
            {
              at: Date.now(),
              source: "drop-click",
              moved: progress.moved,
              onDropTile: progress.onDropTile,
              stillThere: progress.stillThere,
              grew: progress.grew,
              self: progress.state?.authoritativePlayer ?? progress.state?.self ?? progress.state?.player ?? null,
            },
          ];
          // A canvas/drop click may take several Crystal ticks to ACK. Do not
          // immediately inject extra walk packets here, or the harness can queue
          // a second step that walks through the drop tile before PickUp commits.
        } else {
          await directWalkToward(client, drop.x, drop.y, { preferRun: false, walkTimeoutMs: 2_500 });
        }
      }
      await delay(500);
    }
    let after = await readGameState(client);
    const lootScan = scanWsOutcomes(client, lootWsBase, { playerId });
    if (lootScan.gainedItems > 0 && carriedItemCount(after) <= carriedBefore) {
      const inventorySyncDeadline = Date.now() + 2_000;
      while (Date.now() < inventorySyncDeadline && carriedItemCount(after) <= carriedBefore) {
        await delay(150);
        after = await readGameState(client);
      }
    }
    beat.inventoryAfter = after.inventoryCount;
    beat.carriedAfter = carriedItemCount(after);
    beat.gainedItemPackets = lootScan.gainedItems;
    beat.reachedTile = reached;
    beat.dropStillOnGround = after.groundDrops.some((d) => String(d.objectId) === String(drop.objectId));
    beat.sentPickupFrames = sentCommandFramesSince(client, lootWsTxBase, ["pickUp", "pickUpTile"]);
    const grew =
      beat.carriedAfter > carriedBefore ||
      (typeof after.inventoryCount === "number" && after.inventoryCount > invBefore);
    if (grew || lootScan.gainedItems > 0) {
      const sourceNote = dropSource === "qa-seed" ? "QA-seeded ground drop" : "ground drop";
      ctx.score("loot", "pass", `carried ${carriedBefore} -> ${beat.carriedAfter} (inventory ${invBefore} -> ${after.inventoryCount})${lootScan.gainedItems ? ` (GainedItem x${lootScan.gainedItems})` : ""} after looting "${drop.name ?? drop.objectId}" (${sourceNote})`);
    } else if (!beat.dropStillOnGround && beat.sentPickupFrames.length === 0) {
      ctx.score("loot", "skip", `drop "${drop.name ?? drop.objectId}" disappeared before a pickup packet was sent`);
      ctx.addIssue({
        category: "loot",
        severity: "low",
        beat: beat.id,
        summary: "ground drop disappeared before the client sent pickUp/pickUpTile - likely stale shared drop or snapshot removal, not a pickup result",
        detail: { drop, reachedTile: reached, sentPickupFrames: beat.sentPickupFrames },
      });
    } else if (!beat.dropStillOnGround) {
      ctx.score("loot", "fail", "drop left the ground but the item did not enter inventory");
      ctx.addIssue({ category: "loot", severity: "high", beat: beat.id, summary: "ground drop was removed but carried items did not grow (pickup not reflected in client state)", detail: { drop, inventoryBefore: invBefore, inventoryAfter: after.inventoryCount, carriedBefore, carriedAfter: beat.carriedAfter, pickupAttempts: beat.pickupAttempts, sentPickupFrames: beat.sentPickupFrames } });
    } else {
      ctx.score("loot", "fail", "could not pick up the ground drop");
      const bridgeRejectedPickup = beat.pickupAttempts.some((attempt) => !attempt.objectPickup?.sent && !attempt.tilePickup?.sent);
      if (bridgeRejectedPickup) {
        ctx.addIssue({ category: "infra", severity: "high", beat: beat.id, summary: "pickup command bridge returned false while the player was adjacent to the drop", detail: { drop, pickupAttempts: beat.pickupAttempts } });
      }
      ctx.addIssue({ category: "loot", severity: "medium", beat: beat.id, summary: "could not pick up the ground drop (never reached the tile, or PickUp not reflected)", detail: { drop, reachedTile: reached, inventoryBefore: invBefore, inventoryAfter: after.inventoryCount, carriedBefore, carriedAfter: beat.carriedAfter, pickupAttempts: beat.pickupAttempts, sentPickupFrames: beat.sentPickupFrames } });
    }
  });

  // Beat 3 - you TAKE damage: field monsters are non-aggressive (they only retaliate
  // when hit), so provoke incoming damage by ATTACKING a monster and watch playerHp
  // fall. If the kill hunt already cost HP (mutual combat), that counts immediately.
  await runBeat(ctx, "survival: take damage from a monster", async (beat) => {
    const start = await readGameState(client);
    const startHp = start.playerHp ?? start.self?.hp ?? null;
    beat.startHp = startHp;
    if (typeof startHp !== "number") {
      ctx.score("takeDamage", "fail", "playerHp not exposed in client state");
      ctx.addIssue({ category: "survival", severity: "medium", beat: beat.id, summary: "playerHp missing from client state - cannot verify taking damage", detail: start.self });
      return;
    }
    if (startHp === 0 || (typeof ctx.baseline.playerHp === "number" && startHp < ctx.baseline.playerHp)) {
      ctx.score("takeDamage", "pass", `playerHp already fell ${ctx.baseline.playerHp ?? "?"} -> ${startHp} during the kill hunt (mutual combat)`);
      beat.viaHunt = true;
      return;
    }
    // Provoke retaliation: attack a monster and watch the player's HP fall.
    const monster = await acquireMonster(ctx, beat, [], {
      requireHostile: true,
      preferHostile: true,
      requireRetaliator: true,
      spawnTemplate: GM_RETALIATION_SPAWN_TEMPLATE,
      allowQaSeed: true,
      preferQaSeed: true,
    });
    beat.target = monster
      ? {
          objectId: monster.objectId,
          name: monster.name,
          mapFileName: monster.mapFileName ?? null,
          at: { x: monster.x, y: monster.y },
          hp: monster.hp ?? null,
          maxHp: monster.maxHp ?? null,
          disposition: monster.disposition,
          distance: monster.distance ?? null,
        }
      : null;
    if (!monster) {
      ctx.score("takeDamage", "skip", "no hostile monster available to provoke damage from");
      ctx.addIssue({ category: "survival", severity: "low", beat: beat.id, summary: "no hostile monster present to provoke incoming damage (neutral/passive animals are excluded; @MOB is GM/test-gated)", detail: { startHp } });
      return;
    }
    const wsBase = client.wsRxCount;
    const wsTxBase = client.wsTxCount;
    let lowHp = startHp;
    let attackAttempts = 0;
    let reachedMelee = false;
    let targetLost = false;
    let mapMismatch = false;
    const approachTrace = [];
    const deadline = Date.now() + TAKE_DAMAGE_WINDOW_MS;
    while (Date.now() < deadline) {
      const m = await readMonsterById(client, monster.objectId);
      if (!m) {
        targetLost = true;
        break;
      }
      // Same rule as the kill loop: only walk when far, only swing when adjacent (an
      // attack blocks the next walk), so we close in then trade blows.
      const current = await readGameState(client).catch(() => null);
      const self = current?.authoritativePlayer ?? current?.player ?? null;
      if (m.mapFileName != null && current?.mapFileName != null && String(m.mapFileName) !== String(current.mapFileName)) {
        mapMismatch = true;
        break;
      }
      if (approachTrace.length < 24) {
        approachTrace.push({
          at: Date.now(),
          map: current?.mapFileName ?? m.mapFileName ?? null,
          self,
          target: { x: m.x, y: m.y, dead: m.dead, hp: m.hp ?? null, maxHp: m.maxHp ?? null },
          distance: self ? chebyshev(m, self) : null,
        });
      }
      if (self && self.x === m.x && self.y === m.y) {
        await stepOffOverlappingMonster(client, self, m);
        continue;
      }
      if (self && chebyshev(m, self) <= 1) {
        reachedMelee = true;
        attackAttempts += 1;
        if (!(await clickTile(client, m.x, m.y, "left"))) {
          await sendCommand(client, { type: "attack", objectId: Number(monster.objectId) });
        }
        await sendCommand(client, { type: "tick" });
        await delay(700);
        await sendCommand(client, { type: "tick" });
      } else {
        const step = self ? meleeApproachTile(self, m) : { x: m.x, y: m.y };
        await directWalkToward(client, step.x, step.y, { preferRun: false, walkTimeoutMs: 1_150 });
        await sendCommand(client, { type: "tick" });
        await delay(450);
      }
      const hp = await readPlayerHp(client);
      if (typeof hp === "number") lowHp = Math.min(lowHp, hp);
      if (typeof hp === "number" && hp < startHp) break;
    }
    const settleStartedAt = Date.now();
    const settledScan = await scanWsOutcomesSettled(
      client,
      wsBase,
      { targetId: monster.objectId, playerId: ctx.playerId },
      TAKE_DAMAGE_SETTLE_MS,
    );
    const settledHp = await readPlayerHp(client).catch(() => null);
    if (typeof settledHp === "number") lowHp = Math.min(lowHp, settledHp);
    beat.lowHp = lowHp;
    const attackFrames = sentCommandFramesSince(client, wsTxBase, ["attack"]);
    const scan = settledScan;
    beat.survival = {
      target: beat.target,
      startHp,
      lowHp,
      settleMs: Date.now() - settleStartedAt,
      reachedMelee,
      attackAttempts,
      sentAttackFrames: attackFrames.map((frame) => ({
        seq: frame.seq,
        at: frame.at,
        payload: frame.payload,
      })),
      targetLost,
      mapMismatch,
      approachTrace,
      scan,
    };
    beat.serverPlayerDamageHits = scan.playerDamageHits;
    if (lowHp < startHp) {
      ctx.score("takeDamage", "pass", `playerHp fell ${startHp} -> ${lowHp} from monster retaliation (mutual combat)`);
    } else if (scan.playerDamageHits > 0 || (scan.playerMinPercent !== null && scan.playerMinPercent < 100)) {
      ctx.score("takeDamage", "pass", `server applied damage to the player (${scan.playerDamageHits} hit(s), min ${scan.playerMinPercent}%)`);
    } else if (scan.monsterDamageHits > 0 && scan.targetDamageHits === 0 && scan.playerDamageHits === 0) {
      ctx.score("takeDamage", "fail", `attack frames damaged another monster, but not the tracked hostile target; playerHp stayed ${startHp}`);
      ctx.addIssue({
        category: "survival",
        severity: "medium",
        beat: beat.id,
        summary: "survival probe produced monster damage on an untracked object before proving retaliation; target selection/pathing is still not deterministic enough",
        detail: beat.survival,
      });
    } else if (!attackFrames.length && !scan.playerObjectAttack && !scan.targetObjectStruck && !scan.targetDamageHits) {
      const why = mapMismatch
        ? "target moved to a different map before an attack was accepted"
        : targetLost
          ? "target disappeared before an attack was accepted"
          : reachedMelee
            ? "reached melee but no client attack frame was captured"
            : "never reached melee before timeout";
      ctx.score("takeDamage", "fail", `could not prove a real attack on "${monster.name ?? "?"}" (${why})`);
      ctx.addIssue({
        category: "survival",
        severity: "medium",
        beat: beat.id,
        summary: `hostile-retaliation probe did not send/accept an attack (${why}) - this is an approach/targeting harness gap, not proof that monsters fail to retaliate`,
        detail: beat.survival,
      });
    } else if (attackFrames.length && !scan.playerObjectAttack && !scan.targetObjectStruck && !scan.targetDamageHits) {
      ctx.score("takeDamage", "fail", `sent ${attackFrames.length} attack frame(s), but the server emitted no player ObjectAttack/ObjectStruck`);
      ctx.addIssue({
        category: "survival",
        severity: "medium",
        beat: beat.id,
        summary: "attack command reached the WebSocket trace but did not produce an accepted server combat packet before timeout",
        detail: beat.survival,
      });
    } else {
      ctx.score("takeDamage", "fail", `attacked "${monster.name ?? "?"}" but playerHp never dropped (stayed ${startHp})`);
      ctx.addIssue({ category: "survival", severity: "medium", beat: beat.id, summary: `attacked a monster but took no return damage (playerHp stayed ${startHp}) - monster may not be retaliating`, detail: beat.survival });
    }
  });

  // Beat 4 - death -> revive flow. @DIE is ungated (gm_commands.rs:1147) so death is
  // deterministic. Revival now has a player-facing path: the `townRevive` command
  // (BrowserCommand -> ClientPacket::TownRevive -> PlayerObject.TownRevive parity)
  // restores HP/MP, respawns at the bind/town point, and replies `Revived`. Assert
  // the LOGICAL death, then drive `townRevive` and confirm the player comes back.
  await runBeat(ctx, "death + revive flow", async (beat) => {
    let before = await waitForStableGameState(client, 1_200, 12_000);
    if (!before.selfDead && before.playerHp !== 0) {
      const fallback = HUNTING_FIELDS[2];
      const anchor = fieldCombatAnchor(fallback, 20);
      beat.deathIsolationTransfer = { map: fallback.map, x: anchor.x, y: anchor.y, from: { map: before.mapFileName, pos: before.authoritativePlayer ?? before.player, playerHp: before.playerHp } };
      await transferTo(client, fallback.map, anchor.x, anchor.y)
        .then(() => {
          beat.deathIsolationTransfer.ok = true;
        })
        .catch((err) => {
          beat.deathIsolationTransfer.ok = false;
          beat.deathIsolationTransfer.error = String(err?.message ?? err);
        });
      before = await waitForStableGameState(client, 1_200, 12_000);
      beat.deathIsolationTransfer.after = {
        map: before.mapFileName,
        pos: before.authoritativePlayer ?? before.player,
        playerHp: before.playerHp,
        selfDead: before.selfDead,
      };
    }
    beat.before = {
      playerHp: before.playerHp,
      selfDead: before.selfDead,
      map: before.mapFileName,
      pos: before.authoritativePlayer ?? before.player,
      renderPos: before.player,
    };
    if (before.selfDead) {
      beat.note = "player already dead before the beat";
    }
    // Deterministically die. @DIE is ungated, but a single send can be missed amid a
    // packet flood, so resend a few times until the dead state reflects.
    const deadExpr =
      "(() => { const s = window.__mir2Stage5?.state; if (!s) return false; const self = (s.entities||[]).find((e)=>e && e.objectId===s.playerObjectId); return s.playerHp === 0 || Boolean(self?.dead); })()";
    const deathWsBase = client.wsRxCount;
    let died = before.selfDead || before.playerHp === 0;
    for (let attempt = 0; attempt < 5 && !died; attempt += 1) {
      if (attempt === 3) {
        beat.midDeathRetryState = await waitForStableGameState(client, 900, 8_000);
      }
      await gmCommand(client, "@DIE").catch(() => {});
      died = await waitUntilSoft(client, deadExpr, DEATH_WINDOW_MS);
    }
    if (!died && before.mapFileName === "0") {
      const fallback = HUNTING_FIELDS[2];
      const anchor = fieldCombatAnchor(fallback, 20);
      beat.deathRetryTransfer = { map: fallback.map, x: anchor.x, y: anchor.y };
      await transferTo(client, fallback.map, anchor.x, anchor.y)
        .then(() => {
          beat.deathRetryTransfer.ok = true;
        })
        .catch((err) => {
          beat.deathRetryTransfer.ok = false;
          beat.deathRetryTransfer.error = String(err?.message ?? err);
        });
      beat.deathRetryTransfer.state = await waitForStableGameState(client, 900, 8_000);
      for (let attempt = 0; attempt < 3 && !died; attempt += 1) {
        await gmCommand(client, "@DIE").catch(() => {});
        died = await waitUntilSoft(client, deadExpr, DEATH_WINDOW_MS);
      }
    }
    // The Death/ObjectDied packets can arrive a frame before the React/Bevy
    // state mirror reflects hp=0. Treat server packet truth as valid, then give
    // the local mirror a short settle window before reading `afterDeath`.
    const deathScan = scanWsOutcomes(client, deathWsBase, { playerId: ctx.playerId });
    if (!died && deathScan.playerDied) {
      died = true;
      await waitUntilSoft(client, deadExpr, 2_500);
    }
    let dead = await readGameState(client);
    if (!died && (dead.playerHp === 0 || dead.selfDead)) {
      died = true;
    }
    if (died && dead.playerHp !== 0 && !dead.selfDead) {
      await delay(800);
      dead = await readGameState(client);
    }
    beat.afterDeath = { playerHp: dead.playerHp, selfDead: dead.selfDead };
    beat.deathPackets = {
      playerDied: deathScan.playerDied,
      playerDamageHits: deathScan.playerDamageHits,
    };
    if (!died) {
      ctx.score("deathRevive", "fail", "@DIE did not put the player into a dead state");
      ctx.addIssue({ category: "survival", severity: "high", beat: beat.id, summary: "@DIE was sent but the player never entered a dead state (playerHp<=0 / self.dead)", detail: beat.afterDeath ?? dead.self });
      return;
    }
    ctx.log(`  * player dead: playerHp=${dead.playerHp} selfDead=${dead.selfDead}`);

    // Drive the player-facing revive: the `townRevive` command is ungated (Crystal's
    // TownRevive only requires the player be dead), so it works for any account. A
    // single send can be missed in a packet flood, so resend until the player is alive.
    const aliveExpr =
      "(() => { const s = window.__mir2Stage5?.state; if (!s) return false; const self = (s.entities||[]).find((e)=>e && e.objectId===s.playerObjectId); return (typeof s.playerHp === 'number' && s.playerHp > 0) && !self?.dead; })()";
    let revived = false;
    for (let attempt = 0; attempt < 3 && !revived; attempt += 1) {
      await sendCommand(client, { type: "townRevive" }).catch(() => {});
      revived = await waitUntilSoft(client, aliveExpr, REVIVE_WINDOW_MS);
    }
    const after = await readGameState(client);
    beat.afterRevive = {
      playerHp: after.playerHp,
      selfDead: after.selfDead,
      map: after.mapFileName,
      pos: after.authoritativePlayer ?? after.player,
      renderPos: after.player,
    };
    // Single-map world: revive respawns at the bind/town spawn within the same map,
    // so record the position change rather than a map change.
    beat.movedToTown = before.mapFileName !== after.mapFileName;
    beat.respawnPos = after.authoritativePlayer ?? after.player;
    if (revived) {
      const respawn = after.authoritativePlayer ?? after.player;
      ctx.score("deathRevive", "pass", `player revived in town via townRevive (playerHp ${dead.playerHp} -> ${after.playerHp}, dead -> alive, respawn @ ${respawn ? `(${respawn.x},${respawn.y})` : "?"})`);
    } else {
      ctx.score("deathRevive", "fail", "townRevive did not restore the player (still dead after the revive window)");
      ctx.addIssue({
        category: "survival",
        severity: "high",
        beat: beat.id,
        summary: "death works but the townRevive command did not bring the player back - the wired revive path failed to restore HP / clear the dead state",
        detail: { afterDeath: beat.afterDeath, afterRevive: beat.afterRevive },
      });
    }
  });

  // Wrap - snapshot final state for the report.
  await runBeat(ctx, "wrap + collect diagnostics", async (beat) => {
    beat.final = await readGameState(client);
    beat.gatewayUrl = client.gatewayUrl();
  });
}

// ---------------------------------------------------------------------------
// Register -> login -> character -> enter-world (beats, ported from qa-playthrough).
// ---------------------------------------------------------------------------
async function enterWorld(ctx) {
  const client = ctx.client;

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

  await runBeat(ctx, "register + log in", async (beat) => {
    beat.account = account;
    let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "login") {
      await fillInput(client, ".login-input.account", account);
      await fillInput(client, ".login-input.password", password);
      if (createAccount) {
        await click(client, ".login-button.account button").catch(() => {});
        await waitUntilSoft(client, "window.__mir2Stage5?.state?.wsState === 'open'", 15_000);
        await delay(1500);
      }
      await fillInput(client, ".login-input.account", account);
      await fillInput(client, ".login-input.password", password);
      await click(client, ".login-button.ok button");
      await waitUntil(client, "['select','game'].includes(window.__mir2Stage5?.state?.screen)", "select/game screen", 30_000);
    }
    beat.gatewayUrl = client.gatewayUrl();
    // Flag whether we actually reached the Rust gateway (real numerics) or a proxy.
    const url = client.gatewayUrl();
    const onRust = typeof url === "string" && /:7111\b/.test(url);
    if (url && !onRust) {
      ctx.addIssue({
        category: "infra",
        severity: "low",
        beat: beat.id,
        summary: `client connected to ${url} (not the Rust gateway :7111) - combat numerics may be proxy-backed`,
        detail: { gatewayUrl: url, expected: gatewayWs },
      });
    }
    ctx.gatewayUrl = url;
    ctx.gatewayIsRust = onRust;
  });

  await runBeat(ctx, "create character + start game", async (beat) => {
    let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "game") return;
    beat.characterName = characterName;
    // Don't trust state.characters to decide existence - it can hold a phantom slot
    // the server never returned (player-qa-playthrough-loop memory). Our named
    // character appearing is the reliable "server created it" signal.
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
        ctx.addIssue({ category: "flow", severity: "high", beat: beat.id, summary: "character creation did not produce a server-backed character", detail: { characterName } });
      }
    }
    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
    );
    beat.startIndex = startIndex;
    const attempts = [];
    let entered = false;
    for (let attempt = 1; attempt <= 3; attempt += 1) {
      screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null").catch(() => null);
      if (screen === "game") {
        entered = true;
        break;
      }
      const wsBefore = client.wsRxCount;
      const probe = await sendCommandProbe(client, { type: "startGame", characterIndex: startIndex });
      const attemptEntered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 15_000);
      const result = startGameResultFrom(client.webSocketFramesReceived.filter((f) => f.seq >= wsBefore));
      const afterScreen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null").catch(() => null);
      attempts.push({ attempt, sent: probe.sent, result, screenBefore: screen, screenAfter: afterScreen });
      if (attemptEntered || afterScreen === "game") {
        entered = true;
        break;
      }
      await delay(700);
    }
    beat.startGameAttempts = attempts;
    if (!entered) {
      entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 10_000);
    }
    if (!entered) {
      const result = attempts.length ? attempts[attempts.length - 1].result : null;
      ctx.addIssue({
        category: "flow",
        severity: "high",
        beat: beat.id,
        summary: result === null ? "startGame sent but never entered the game" : `startGame rejected by server (StartGame result=${result})`,
        detail: { startIndex, result, attempts },
      });
      throw new Error(`did not enter game (StartGame result=${result})`);
    }
  });

  await runBeat(ctx, "enter world + scene ready", async (beat) => {
    await waitUntil(client, "!document.querySelector('.login-transition-overlay')", "login transition cleared", 8_000).catch(() => {});
    const ready = await waitUntilSoft(
      client,
      "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
      SCENE_READY_TIMEOUT_MS,
    );
    if (!ready) {
      ctx.addIssue({ category: "render", severity: "high", beat: beat.id, summary: "scene never became interaction/asset ready after entering world", detail: null });
    }
    await delay(1000);
    const grid = await captureLumaGrid(client).catch(() => null);
    const mean = lumaMean(grid);
    beat.canvasLumaMean = mean === null ? null : Math.round(mean * 10) / 10;
    if (mean !== null && mean < BLACK_LUMA_MEAN) {
      ctx.addIssue({ category: "render", severity: "high", beat: beat.id, summary: `canvas is effectively black after entering world (mean luma ${mean.toFixed(1)})`, detail: null });
    }
  });
}

// ---------------------------------------------------------------------------
// Cross-cutting collectors (run after the cycle).
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
async function writeFileAtomic(filePath, data) {
  const tmp = `${filePath}.tmp-${process.pid}`;
  await fs.writeFile(tmp, data);
  let lastError = null;
  for (let attempt = 0; attempt < 4; attempt += 1) {
    try {
      if (attempt > 0) {
        await delay(25 * attempt);
      }
      await fs.rm(filePath, { force: true });
      await fs.rename(tmp, filePath);
      return;
    } catch (err) {
      lastError = err;
    }
  }
  await fs.rm(tmp, { force: true }).catch(() => {});
  throw lastError;
}

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
    gatewayWsRequested: forceGateway ? gatewayWs : "(client default)",
    gatewayUrlConnected: ctx.gatewayUrl ?? ctx.client.gatewayUrl(),
    gatewayIsRust: ctx.gatewayIsRust ?? null,
    account,
    characterName,
    qaSeeds: {
      windowMs: QA_SEED_WINDOW_MS,
      pickup: {
        allow: ALLOW_QA_PICKUP_SEED,
        prefer: PREFER_QA_PICKUP_SEED,
      },
      combat: {
        allow: ALLOW_QA_COMBAT_SEED,
        prefer: PREFER_QA_COMBAT_SEED,
        retaliationTemplate: GM_RETALIATION_SPAWN_TEMPLATE,
      },
    },
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    beats: ctx.beats.length,
    beatsOk: ctx.beats.filter((b) => b.ok).length,
    issueCount: ctx.issues.length,
    bySeverity,
    byCategory,
    scorecard: ctx.scorecard,
  };

  await writeFileAtomic(path.join(outputDir, "summary.json"), JSON.stringify(summary, null, 2));
  await writeFileAtomic(path.join(outputDir, "report.json"), JSON.stringify({ summary, issues: ctx.issues, beats: ctx.beats }, null, 2));
  await writeFileAtomic(path.join(outputDir, "console.json"), JSON.stringify({ errors: ctx.client.consoleErrors, messages: ctx.client.consoleMessages.slice(-200) }, null, 2));
  await writeFileAtomic(path.join(outputDir, "network-failures.json"), JSON.stringify(ctx.client.networkFailures, null, 2));
  await writeFileAtomic(
    path.join(outputDir, "ws-timeline.json"),
    JSON.stringify({
      events: ctx.client.webSocketEvents.slice(-100),
      sent: ctx.client.webSocketFramesSent.slice(-200),
      received: ctx.client.webSocketFramesReceived.slice(-200),
    }, null, 2),
  );

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  const statusIcon = (s) => (s === "pass" ? "pass" : s === "fail" ? "fail" : s === "gap" ? "known-gap" : "skip");
  const md = [];
  md.push(`# Combat / survival QA report - ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\` - account \`${account}\` - character \`${characterName}\``);
  md.push(`- Gateway: requested \`${summary.gatewayWsRequested}\` - connected \`${summary.gatewayUrlConnected ?? "?"}\` - Rust(:7111)=${summary.gatewayIsRust === null ? "?" : summary.gatewayIsRust}`);
  md.push(`- QA seeds: window=${QA_SEED_WINDOW_MS}ms; pickup allow=${ALLOW_QA_PICKUP_SEED} prefer=${PREFER_QA_PICKUP_SEED}; combat allow=${ALLOW_QA_COMBAT_SEED} prefer=${PREFER_QA_COMBAT_SEED} retaliation=\`${GM_RETALIATION_SPAWN_TEMPLATE}\``);
  if (summary.gatewayIsRust === false) md.push("  - WARNING: numerics may be proxy-backed (not the authoritative Rust sim).");
  md.push(`- Beats: ${summary.beatsOk}/${summary.beats} ok - Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push("");
  md.push("## Combat / survival scorecard");
  md.push("");
  md.push("| Check | Status | Note |");
  md.push("|---|---|---|");
  for (const key of Object.keys(ctx.scorecard)) {
    const c = ctx.scorecard[key];
    md.push(`| ${c.label} | ${statusIcon(c.status)} | ${String(c.note ?? "").replace(/\|/g, "\\|")} |`);
  }
  md.push("");
  md.push("## Issues (by severity)");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues detected._");
  } else {
    md.push("| # | Severity | Category | Beat | Summary |");
    md.push("|---|---|---|---|---|");
    for (const i of sortedIssues) {
      md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.beat ?? "-"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    }
  }
  md.push("");
  md.push("## Cycle (beats)");
  md.push("");
  for (const b of ctx.beats) {
    md.push(`### Beat ${b.id} - ${b.title} ${b.ok ? "OK" : "FAIL"} (${b.durationMs}ms)`);
    if (b.error) md.push(`- error: \`${b.error.slice(0, 300)}\``);
    if (b.note) md.push(`- note: ${b.note}`);
    if (b.startGameAttempts) md.push(`- startGame attempts: ${JSON.stringify(b.startGameAttempts)}`);
    if (b.state) {
      md.push(`- state: screen=\`${b.state.screen}\` map=\`${b.state.mapFileName ?? "?"}\` safe=${b.state.inSafeZone ?? "?"} hp=${b.state.playerHp ?? "?"}/${b.state.playerMaxHp ?? "?"} monsters=${b.state.liveMonsterCount}/${b.state.monsterCount} drops=${b.state.groundDropCount} inv=${b.state.inventoryCount ?? "?"} floaters=${b.state.damageFloaterDom}`);
    }
    if (b.summary) md.push(`- combat: ${JSON.stringify(b.summary)}`);
    if (b.survival) md.push(`- survival: ${JSON.stringify(b.survival)}`);
    if (b.xp) md.push(`- xp: ${JSON.stringify(b.xp)}`);
    if (b.drop) md.push(`- drop: ${JSON.stringify(b.drop)}`);
    if (b.qaPickupSeed) md.push(`- qa pickup seed: ${JSON.stringify(b.qaPickupSeed)}`);
    if (b.qaCombatSeed) md.push(`- qa combat seed: ${JSON.stringify(b.qaCombatSeed)}`);
    if (b.deathIsolationTransfer) md.push(`- death isolation transfer: ${JSON.stringify(b.deathIsolationTransfer)}`);
    if (b.afterRevive) md.push(`- death/revive: ${JSON.stringify({ before: b.before, afterDeath: b.afterDeath, afterRevive: b.afterRevive, movedToTown: b.movedToTown })}`);
    if (b.frame) md.push(`- frame: [\`${b.frame}\`](${b.frame})`);
    md.push("");
  }
  md.push("## Detailed issue evidence");
  md.push("");
  for (const i of sortedIssues) {
    md.push(`### ${i.id} - [${i.severity}/${i.category}] ${i.summary}`);
    if (i.beat) md.push(`- beat: ${i.beat}`);
    if (i.detail !== undefined) md.push("```json\n" + JSON.stringify(i.detail, null, 2) + "\n```");
    md.push("");
  }
  md.push("## Reproduce");
  md.push("");
  md.push("```bash");
  md.push(`cd apps/web && node ./scripts/qa-combat-survival.mjs --headed --baseUrl ${baseUrl} --gatewayWs ${gatewayWs} --qaSeedWindowMs ${QA_SEED_WINDOW_MS} --allowQaPickupSeed ${ALLOW_QA_PICKUP_SEED} --preferQaPickupSeed ${PREFER_QA_PICKUP_SEED} --allowQaCombatSeed ${ALLOW_QA_COMBAT_SEED} --preferQaCombatSeed ${PREFER_QA_COMBAT_SEED} --retaliationSpawnTemplate ${GM_RETALIATION_SPAWN_TEMPLATE}`);
  md.push("```");
  await writeFileAtomic(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-playthrough.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-combat-${process.pid}-${Date.now()}`);
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
  while (Date.now() < deadline) {
    const v = await client.evaluate(`Boolean(${expression})`).catch(() => null);
    if (v) return;
    await delay(120);
  }
  const debug = await client
    .evaluate(`(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, body: document.body?.innerText?.slice(0, 200) ?? "" }))()`)
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
function buildRunUrl(base, { gatewayWs }) {
  const params = [];
  params.push("mir2Debug=1");
  if (gatewayWs) params.push(`gatewayWs=${encodeURIComponent(gatewayWs)}`);
  return `${base}${base.includes("?") ? "&" : "?"}${params.join("&")}`;
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

function defaultCharacterName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `CB${suffix}`.slice(0, 10).toUpperCase();
}

function defaultAccountName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `CB${suffix}`.slice(0, 12).toUpperCase();
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
  console.log(`qa-combat-survival: target=${baseUrl} gateway=${forceGateway ? gatewayWs : "(client default)"} account=${account} char=${characterName} headed=${headed}`);
  console.log(`qa-combat-survival: output=${outputDir}`);

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

    await combatSurvival(ctx);

    collectNetworkIssues(ctx);
    collectConsoleIssues(ctx);
    await writeReport(ctx);

    const s = ctx.issues.reduce((acc, i) => ((acc[i.severity] = (acc[i.severity] ?? 0) + 1), acc), {});
    const sc = ctx.scorecard;
    const pass = Object.values(sc).filter((c) => c.status === "pass").length;
    const fail = Object.values(sc).filter((c) => c.status === "fail").length;
    const gap = Object.values(sc).filter((c) => c.status === "gap").length;
    console.log(`\nqa-combat-survival done: scorecard ${pass} pass / ${fail} fail / ${gap} known-gap; ${ctx.beats.filter((b) => b.ok).length}/${ctx.beats.length} beats ok, ${ctx.issues.length} issues (high ${s.high ?? 0}, medium ${s.medium ?? 0}, low ${s.low ?? 0}).`);
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
  } catch (error) {
    console.error("qa-combat-survival fatal:", error);
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
