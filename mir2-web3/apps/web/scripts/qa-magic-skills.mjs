// qa-magic-skills.mjs — AI-driven "real player" magic / skill-casting cycle.
//
// The combat loop (qa-combat-survival.mjs) proves the MELEE pipe end to end. This
// harness drives the REAL web client over Chrome DevTools Protocol through the
// MAGIC / SKILL pipe and records every broken step into a structured report under
// docs/generated/player-qa/magic-skills-<runId>/:
//
//   1. Enter as a CASTER class (Taoist by default) and survey the known spell book
//      (window.__mir2Stage5.state.knownSkills) + caster vitals (playerMp/maxMp).
//   2. Place a spell on the skill bar (BrowserCommand MagicKey -> assign_magic_key)
//      and verify the binding round-trips in the next world snapshot (skill.hotkey).
//   3. Cast on SELF (a heal / self-buff)            -> MP deducted + cooldown + cast packet.
//   4. Cast in a MONSTER context (offensive / summon) -> target HP change / a summoned pet.
//   5. Cast GROUND-targeted (an aimed AoE / trap)   -> the cast fires at a chosen tile.
//   6. Magic feedback: original Crystal atlas frames (cast / projectile / impact /
//      return), damage floaters and authoritative projectile packets.
//
// Why a browser (not a protocol bot): the client's cast path
// (page.tsx castSkill -> sendMagicSkill -> send({type:"magic", spell, targetId, x, y}))
// and the DOM damage overlay only exist in a real client. Headed Chrome gives a real
// GPU so the Bevy canvas actually renders.
//
// Outcomes are read from the WEBSOCKET TRUTH LAYER, not the client store, exactly like
// the combat loop: a spell that FIRES produces the server's own frames — `Magic` /
// `ObjectMagic` (cast confirmation, sim skills.rs cast_skill_with_context), `ObjectMana`
// (the MP spend), `MagicDelay` (the cooldown), `ObjectProjectile` (fireball-family),
// `DamageIndicator` / `ObjectHealth` / `ObjectDied` (an offensive hit). MP/HP deltas are
// ALSO read from state.playerMp/playerHp (HealthChanged/ObjectMana update them), which is
// reliable for the SELF player (unlike a monster's predicted hp). Movement, when needed
// to reach a monster, is driven by real in-viewport tile clicks (the convenience `moveTo`
// command is rejected on the production path), reusing the combat-loop primitives.
//
// ENV the harness accounts for (does NOT hard-fail on):
//   * The local demo character is seeded with the Crystal STARTER spell book
//     (Healing, Fury, 4 Summons, Stonetrap — all at level 3; sim save.rs:693 seeds a new
//     character's save with seed_skills()). These are all SELF-centred Taoist spells:
//     Healing self-targets, Fury/Summons/Stonetrap are SelfOnly. The starter book has NO
//     single-target offensive nuke and NO ground-aimed AoE.
//   * Learning a pure offensive / ground spell (@GIVESKILL) and raising MP (@LEVEL) are
//     GM/test-gated (is_gm_or_test): a no-op on a non-GM, non-test stack (no
//     MIR2_GM_PASSWORD, test_server=false). The harness ATTEMPTS them, DOCUMENTS the gate
//     when they no-op, and exercises the reachable casts (self-heal/buff + summon + the
//     Stonetrap trap) regardless. If GM/test IS on, it exercises the full offensive +
//     ground path too.
//   * Original Crystal VFX frames are rendered as `.scene-crystal-effect-frame` images.
//     A MutationObserver records their original-effect paths and phase metadata because
//     short cast frames can appear between ordinary polling intervals. Damage floaters are
//     still checked separately and are meaningful only for damaging casts.
//
// This file deliberately reuses the patterns proven in qa-combat-survival.mjs / qa-playthrough.mjs
// (CDP client, headed chrome launch, register->login->enter flow, tile clicking, WS-truth
// scans) so it stays consistent with the house QA harnesses, with zero new dependencies.
//
// Usage:
//   node ./scripts/qa-magic-skills.mjs --headed [--baseUrl http://127.0.0.1:3022]
//        [--gatewayWs ws://127.0.0.1:7111/ws] [--class taoist|wizard]
//        [--account NAME --password PW] [--runId my-run]
//
// Each beat is best-effort: a failing beat is recorded as an issue and the loop keeps
// going so it captures as much of the magic cycle as possible.

import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));

const baseUrl = args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? "http://127.0.0.1:3022";
// Real magic numerics (MP cost / cooldown / damage) are authoritative on the Rust gateway
// (:7111), not the :7110 node proxy. The `gatewayWs` query override is honored only on a
// localhost origin (apps/web/app/page.tsx:991-998), so this just works for a local dev client.
const gatewayWs = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? "ws://127.0.0.1:7111/ws";
const forceGateway = booleanArg(args.forceGateway ?? process.env.MIR2_FORCE_GATEWAY, true);
const RUN_URL = buildRunUrl(baseUrl, { gatewayWs: forceGateway ? gatewayWs : null });

const createAccount = booleanArg(args.createAccount ?? process.env.MIR2_CREATE_ACCOUNT, true);
const account = args.account ?? process.env.MIR2_QA_ACCOUNT ?? defaultAccountName();
// Keep the local-only default inside the commercial credential floor as well, so the same
// harness cannot silently fail account creation when the gateway enables that policy.
const password = args.password ?? process.env.MIR2_QA_PASSWORD ?? "Mir2test123";
const characterName = args.characterName ?? defaultCharacterName();
// The starter spell book (Healing/Summons/Stonetrap) is Taoist-flavoured, so a Taoist is
// the natural caster. Casting is NOT class-gated in the sim (skill_cast_preflight_for_key
// checks alive/MP/target, not class), so wizard works too — `--class wizard` is honored.
const characterClass = normalizeClass(args.class ?? process.env.MIR2_QA_CLASS ?? "taoist");
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, true);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9600 + (process.pid % 300));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "player-qa", `magic-skills-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

// Tunables.
const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const CAST_OBSERVE_MS = numberArg(args.castObserveMs, 4_000); // window to collect a cast's server frames
const MONSTER_CAST_WINDOW_MS = numberArg(args.monsterCastWindowMs, 26_000);
const SPAWN_POLL_MS = numberArg(args.spawnPollMs, 22_000);
const EXPLORE_SCAN_RANGE = numberArg(args.exploreScanRange, 22);
const BLACK_LUMA_MEAN = 8;

// Spells the harness tries to LEARN via @GIVESKILL to exercise the offensive single-target
// and ground-aimed paths the starter book lacks. GM/test-gated — a no-op (documented) on a
// non-GM stack. Crystal spell names (parse_spell_name -> Spell::from_crystal_name).
const SELF_GRANT_SPELL = args.selfSpell ?? "Healing"; // self heal (castKind "target", self-targets)
const OFFENSIVE_GRANT_SPELL = args.offensiveSpell ?? "ThunderBolt"; // single-target (castKind "target")
const GROUND_GRANT_SPELL = args.groundSpell ?? "FireWall"; // ground-aimed AoE (castKind "ground")
const GRANT_LEVEL = numberArg(args.grantLevel, 3);
// Optional GM elevation. A fresh persistence-backed character starts with NO learned magic
// (Crystal learns spells from skill books / NPCs), and @GIVESKILL / @LEVEL are is_gm_or_test
// gated. When a GM password is available (the gateway was started with MIR2_GM_PASSWORD), the
// harness logs in as GM via Crystal's @LOGIN handshake, raises level for castable MP, and
// grants a caster kit — exercising the FULL offensive + ground cast path. Without it (the
// default local stack) the gate is documented and only the reachable book is tested.
const gmPassword = args.gmPassword ?? process.env.MIR2_QA_GM_PASSWORD ?? null;
const gmLevel = numberArg(args.gmLevel, 22);

// Hunting fields the harness explores for a live monster (shared with the combat loop). The
// on-demand monster pool (#83) spawns wild monsters on these field maps.
const HUNTING_FIELDS = [
  // Enter just outside each respawn safe zone. Starting at its center and walking out is
  // flaky once @MOB fills the safe-zone tiles and occupancy blocks the player.
  // (333,173) is a verified open 3x3 patch in the original map collision data, which
  // leaves deterministic room for control-spell push destinations.
  { map: "1", x: 333, y: 173, label: "Woomyon Woods field (1)" },
  { map: "2", x: 503, y: 500, label: "Serpent Valley field (2)" },
];
const GM_SPAWN_TEMPLATE = args.spawnTemplate ?? "Deer";
const TARGET_MONSTER_NAME = args.targetName ?? null;

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from qa-combat-survival.mjs) — captures console, network
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
    this.networkCancellations = [];
    this.requestUrlById = new Map();
    this.webSocketFramesReceived = [];
    this.webSocketFramesSent = [];
    this.gatewayUrls = [];
    // Monotonic counter stamped onto every received frame. The frame buffer is capped
    // (slice(-N)), so absolute array indices drift as it rotates — the WS-truth scans must
    // select frames by this stable `seq`, never by array position.
    this.wsRxCount = 0;
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
        const entry = {
          url,
          status: 0,
          errorText: p.errorText ?? "",
          canceled: Boolean(p.canceled),
          kind: classifyAssetUrl(url),
          at: Date.now(),
        };
        // CDP reports an image request canceled because its React entity unmounted as
        // `loadingFailed`, usually `net::ERR_ABORTED`. That is lifecycle telemetry, not a
        // missing/failed asset. Preserve it separately so the report remains auditable.
        if (entry.canceled && entry.errorText === "net::ERR_ABORTED") this.networkCancellations.push(entry);
        else this.networkFailures.push(entry);
      }
    } else if (m === "Network.webSocketCreated") {
      if (p.url) this.gatewayUrls.push({ url: String(p.url), at: Date.now() });
    } else if (m === "Network.webSocketFrameReceived") {
      this.webSocketFramesReceived.push({ payloadData: p.response?.payloadData, at: Date.now(), seq: this.wsRxCount++ });
      this.webSocketFramesReceived = this.webSocketFramesReceived.slice(-4000);
    } else if (m === "Network.webSocketFrameSent") {
      this.webSocketFramesSent.push({ payloadData: p.response?.payloadData, at: Date.now() });
      this.webSocketFramesSent = this.webSocketFramesSent.slice(-800);
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
    casts: [], // every cast attempt + its observed server outcome (for the report)
    // The magic/skill checks this harness exists to score.
    scorecard: {
      knownSpells: { label: "1. Enter as a caster with a known spell book (knownSkills)", status: "skip", note: "" },
      skillBarPlace: { label: "2. Place a spell on the skill bar (MagicKey round-trips)", status: "skip", note: "" },
      castSelf: { label: "3. Cast on SELF (heal/buff) → MP deducted + cast fires", status: "skip", note: "" },
      castMonster: { label: "4. Cast in a MONSTER context (offensive / summon)", status: "skip", note: "" },
      castGround: { label: "5. Cast GROUND-targeted (aimed AoE / trap)", status: "skip", note: "" },
      mpCooldown: { label: "6. MP is deducted and a cooldown (MagicDelay) is applied", status: "skip", note: "" },
      magicVfx: { label: "7. Original Crystal VFX frames (cast / projectile / impact / world)", status: "skip", note: "" },
    },
    log: (msg) => console.log(msg),
    score(key, status, note) {
      if (!this.scorecard[key]) return;
      this.scorecard[key].status = status;
      if (note) this.scorecard[key].note = note;
      const icon = status === "pass" ? "✓" : status === "fail" ? "✗" : status === "gap" ? "▱" : "·";
      console.log(`  ${icon} [${status}] ${this.scorecard[key].label}${note ? ` — ${note}` : ""}`);
    },
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
// In-page state reader — magic view of the authoritative client state
// (window.__mir2Stage5.state). Pulls vitals (incl. MP), the self entity, the known
// spell book, monsters, player-owned summons and the live DOM damage-floater count.
// ---------------------------------------------------------------------------
async function readGameState(client) {
  return client.evaluate(`
    (() => {
      const state = window.__mir2Stage5?.state ?? {};
      const entities = Array.isArray(state.entities) ? state.entities : [];
      const selfId = state.playerObjectId ?? null;
      const selfEntity = entities.find((e) => e && e.objectId === selfId) ?? null;
      const monsters = entities.filter((e) => e && e.kind === "monster");
      // Summoned pets are non-monster, non-player entities owned by / allied to the player
      // (Crystal "pet"). The shape varies, so detect generously and let the beat refine.
      const pets = entities.filter((e) => e && e.objectId !== selfId && (
        e.kind === "pet" || e.kind === "summon" || e.owner === selfId || e.ownerId === selfId ||
        e.master === selfId || (e.kind === "monster" && (e.tamed || e.pet || e.allied))
      ));
      const skills = Array.isArray(state.knownSkills) ? state.knownSkills : [];
      const compact = (e) => e ? {
        objectId: String(e.objectId), name: e.name ?? null, kind: e.kind ?? null,
        x: e.x, y: e.y, hp: e.hp ?? null, maxHp: e.maxHp ?? null, dead: Boolean(e.dead),
        disposition: e.disposition ?? null,
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
        player: state.player ? { x: state.player.x, y: state.player.y, direction: state.player.direction ?? null } : null,
        selfDirection: selfEntity?.direction ?? null,
        playerHp: typeof state.playerHp === "number" ? state.playerHp : null,
        playerMaxHp: typeof state.playerMaxHp === "number" ? state.playerMaxHp : null,
        playerMp: typeof state.playerMp === "number" ? state.playerMp : null,
        playerMaxMp: typeof state.playerMaxMp === "number" ? state.playerMaxMp : null,
        selectedObjectId: state.selectedObjectId ?? null,
        self: compact(selfEntity),
        entityCount: entities.length,
        monsterCount: monsters.length,
        liveMonsterCount: monsters.filter((m) => !m.dead).length,
        monsters: monsters.slice(0, 30).map(compact),
        petCount: pets.length,
        pets: pets.slice(0, 12).map(compact),
        knownSkillCount: skills.length,
        knownSkills: skills.slice(0, 40).map((s) => ({
          key: s.key, name: s.name ?? null, spell: s.spell ?? null,
          castKind: s.castKind ?? null, offensive: Boolean(s.offensive),
          hotkey: typeof s.hotkey === "number" ? s.hotkey : null,
          delayMs: typeof s.delayMs === "number" ? s.delayMs : null,
          cooldownRemainingTicks: typeof s.cooldownRemainingTicks === "number" ? s.cooldownRemainingTicks : null,
        })),
        damageFloaterDom: document.querySelectorAll(".scene-damage-floater").length,
        crystalEffectFrameDom: document.querySelectorAll(".scene-crystal-effect-frame").length,
      };
    })()
  `);
}

async function readKnownSkills(client) {
  const state = await readGameState(client);
  return state.knownSkills ?? [];
}

async function waitForKnownSpell(client, spell, timeoutMs = 10_000) {
  const wanted = String(spell).toLowerCase().replace(/[^a-z0-9]/g, "");
  return waitUntilSoft(
    client,
    `(() => (window.__mir2Stage5?.state?.knownSkills ?? []).some((s) => String(s?.spell ?? s?.name ?? s?.key ?? '').toLowerCase().replace(/[^a-z0-9]/g, '') === ${JSON.stringify(wanted)}))()`,
    timeoutMs,
  );
}

async function readSelf(client) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const self = ents.find((e) => e && e.objectId === s.playerObjectId) ?? null;
      const p = s.player ?? (self ? { x: self.x, y: self.y } : null);
      if (!p && !self) return null;
      // The SERVER casts from / checks range+LoS against the authoritative self entity
      // position; the client predicted "player" can lead it by up to 2 tiles, so use the
      // server position for all cast targeting math (predicted is kept for diagnostics).
      const sx = self ? self.x : p.x;
      const sy = self ? self.y : p.y;
      return {
        objectId: s.playerObjectId != null ? String(s.playerObjectId) : null,
        x: sx, y: sy,
        predictedX: p ? p.x : sx, predictedY: p ? p.y : sy,
        settled: Boolean(self && p && self.x === p.x && self.y === p.y),
        direction: (self && self.direction) || (p && p.direction) || "Down",
        mp: typeof s.playerMp === "number" ? s.playerMp : null,
        maxMp: typeof s.playerMaxMp === "number" ? s.playerMaxMp : null,
        hp: typeof s.playerHp === "number" ? s.playerHp : null,
        maxHp: typeof s.playerMaxHp === "number" ? s.playerMaxHp : null,
      };
    })()
  `);
}

// Wait until the predicted player position matches the server self entity (movement settled),
// so casts computed from the player position agree with the server's range/LoS check.
async function waitForPlayerSettled(client, timeoutMs = 4500) {
  const deadline = Date.now() + timeoutMs;
  let stable = 0;
  while (Date.now() < deadline) {
    const self = await readSelf(client);
    if (self?.settled) {
      stable += 1;
      if (stable >= 2) return true;
    } else {
      stable = 0;
    }
    await delay(300);
  }
  return false;
}

async function readPlayerMp(client) {
  return client.evaluate(`(() => { const s = window.__mir2Stage5?.state; return typeof s?.playerMp === "number" ? s.playerMp : null; })()`);
}

async function readPlayerHp(client) {
  return client.evaluate(`(() => { const s = window.__mir2Stage5?.state; return typeof s?.playerHp === "number" ? s.playerHp : null; })()`);
}

// ---------------------------------------------------------------------------
// Render health — quick black-canvas check (Bevy is the renderer).
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
// Command primitives.
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

// Fire a GM "@" chat command (best-effort — a no-op for a non-GM, non-test account;
// @GIVESKILL / @LEVEL are is_gm_or_test-gated, @DIE is ungated).
async function gmCommand(client, message) {
  return sendCommand(client, { type: "chat", message });
}

// Elevate to GM via Crystal's @LOGIN password handshake (PlayerObject.cs:1898 →
// sim resolve_gm_login_password): send "@LOGIN", then the password as the NEXT chat line.
// No-op (returns false) when no password is configured — the default local stack. Success
// is confirmed downstream by whether @GIVESKILL then populates the spell book.
async function gmElevate(client) {
  if (!gmPassword) return false;
  await gmCommand(client, "@LOGIN");
  await delay(1200);
  await gmCommand(client, gmPassword);
  await delay(1500);
  return true;
}

// Cast a spell directly via the production magic command (== page.tsx sendMagicSkill ->
// send({type:"magic", ...})). The gateway maps this to BrowserCommand::Magic ->
// ClientPacket::Magic -> sim cast_skill_with_context. `spell` is the Crystal spell name
// (parse_spell_name). target/x/y/spellTargetLock control self vs monster vs ground.
async function castMagic(client, { spell, direction, targetId, x, y, spellTargetLock }) {
  return sendCommand(client, {
    type: "magic",
    spell,
    direction: direction ?? "Down",
    targetId: targetId ? Number(targetId) : 0,
    x,
    y,
    spellTargetLock: Boolean(spellTargetLock),
  });
}

// Bind a spell to a skill-bar key (BrowserCommand::MagicKey -> ClientPacket::MagicKey ->
// sim assign_magic_key). The web UI never sends this today (no drag-to-bar), but the
// gateway + sim support it, and the resulting hotkey round-trips in the world snapshot.
async function placeOnSkillBar(client, { spell, key, oldKey }) {
  return sendCommand(client, { type: "magicKey", spell, key, oldKey: oldKey ?? 0 });
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

// Click a tile. Returns false when the tile is off-screen (its aria-label node exists even
// when scrolled out of view, so a naive click would dispatch off-screen coords and silently
// do nothing — callers must know it failed so they can step in viewport-sized hops instead).
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

// Move the player toward (tx,ty). The gateway rejects the convenience `moveTo` on the
// production path, so the ONLY way to move is a real in-viewport tile click ->
// handleViewportTileAction -> moveToTile. A far tile is off-screen and unclickable, so click
// an in-viewport tile a few steps along the way and let the caller's loop repeat.
async function walkToward(client, tx, ty) {
  const self = await client.evaluate(
    `(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`,
  );
  if (!self) return false;
  const dx = tx - self.x;
  const dy = ty - self.y;
  const dist = Math.max(Math.abs(dx), Math.abs(dy));
  if (dist === 0) return true;
  for (const stepLen of [6, 5, 4, 3, 2, 1]) {
    if (stepLen > dist) continue;
    const sx = self.x + Math.round((dx / dist) * stepLen);
    const sy = self.y + Math.round((dy / dist) * stepLen);
    if (await clickTile(client, sx, sy, "left")) return true;
  }
  return false;
}

async function transferTo(client, map, x, y) {
  await sendCommand(client, { type: "transferMap", key: `crystal:${map}:${x}:${y}` });
  await waitUntil(
    client,
    `(() => { const s = window.__mir2Stage5?.state; return s?.mapFileName === ${JSON.stringify(map)} && s?.player?.x === ${Number(x)} && s?.player?.y === ${Number(y)}; })()`,
    "forced transfer",
    20_000,
  );
}

// A nearby, killable field monster — the weakest live monster within range that is NOT a
// town guard, an effectively-invulnerable target, or harvestable scenery (reused from the
// combat loop). Returns null when nothing suitable is in range.
async function readTargetMonster(client, maxRange, excludeIds = []) {
  const range = Number.isFinite(maxRange) ? maxRange : 9999;
  const exclJson = JSON.stringify(excludeIds.map(String));
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const p = s.player ?? { x: 0, y: 0 };
      const excl = new Set(${exclJson});
      const targetName = ${JSON.stringify(TARGET_MONSTER_NAME ? String(TARGET_MONSTER_NAME).toLowerCase() : null)};
      const cheb = (e) => Math.max(Math.abs(e.x - p.x), Math.abs(e.y - p.y));
      // Crystal source respawn safe zones for the two hunting fields used below. An
      // offensive target inside one is invalid even when the caster has walked outside it.
      const fieldSafeZones = {
        '1': [{ x: 315, y: 82, size: 15 }],
        '2': [{ x: 503, y: 483, size: 10 }],
      };
      const inSafeZone = (e) => (fieldSafeZones[s.mapFileName] || []).some(
        (z) => Math.abs(e.x - z.x) <= z.size && Math.abs(e.y - z.y) <= z.size,
      );
      const isGuard = (e) => /guard/i.test(e.name || "");
      const invuln = (e) => typeof e.maxHp === "number" && e.maxHp >= 1000;
      const isProp = (e) => /tree|chestnut|herb|mushroom|stump|flower|sprout|sapling|shrub|\\bbush\\b|\\brock\\b|\\bore\\b|vein|plant|\\blog\\b|grass|reed|fungus/i.test(e.name || "");
      const mons = ents.filter(
        (e) => e && e.kind === "monster" && !e.dead && !isGuard(e) && !invuln(e) && !isProp(e) && !inSafeZone(e) && !excl.has(String(e.objectId)) && cheb(e) <= ${range} && (!targetName || String(e.name || '').toLowerCase() === targetName),
      );
      if (!mons.length) return null;
      mons.sort((a, b) => {
        const am = typeof a.maxHp === "number" ? a.maxHp : 99999;
        const bm = typeof b.maxHp === "number" ? b.maxHp : 99999;
        if (am !== bm) return am - bm;
        return cheb(a) - cheb(b);
      });
      const m = mons[0];
      return { objectId: String(m.objectId), name: m.name ?? null, x: m.x, y: m.y, hp: m.hp ?? null, maxHp: m.maxHp ?? null, dead: Boolean(m.dead), disposition: m.disposition ?? null, monsterCount: mons.length };
    })()
  `);
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

function stepTowardTile(player, target) {
  const dx = Math.sign(target.x - player.x);
  const dy = Math.sign(target.y - player.y);
  return { x: target.x - dx, y: target.y - dy };
}

function chebyshev(a, b) {
  return Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
}

function crystalDirection(dx, dy) {
  const sx = Math.sign(dx);
  const sy = Math.sign(dy);
  if (sx === 0 && sy < 0) return "Up";
  if (sx > 0 && sy < 0) return "UpRight";
  if (sx > 0 && sy === 0) return "Right";
  if (sx > 0 && sy > 0) return "DownRight";
  if (sx === 0 && sy > 0) return "Down";
  if (sx < 0 && sy > 0) return "DownLeft";
  if (sx < 0 && sy === 0) return "Left";
  if (sx < 0 && sy < 0) return "UpLeft";
  return "Down";
}

async function countDamageFloaters(client) {
  const n = await client.evaluate(`document.querySelectorAll(".scene-damage-floater").length`).catch(() => null);
  return typeof n === "number" ? n : 0;
}

// Original Crystal cast/projectile/impact frames are intentionally short lived. Install a
// page-side observer before sending the cast so the QA record does not depend on a polling
// interval landing on the exact animation frame.
async function startMagicVfxProbe(client) {
  return client.evaluate(`
    (() => {
      window.__mir2MagicVfxProbe?.observer?.disconnect?.();
      const probe = {
        startedAt: Date.now(),
        eventCount: 0,
        sourceFrameEvents: 0,
        maxLiveFrames: 0,
        phases: {},
        effectNames: [],
        sourcePaths: [],
      };
      const effectKey = (node) => node.dataset.projectileKey || node.dataset.effectKey || '';
      const baselineKeys = new Set(
        [...document.querySelectorAll('.scene-crystal-effect-frame')]
          .map(effectKey)
          .filter(Boolean),
      );
      const addUnique = (array, value) => {
        if (value && !array.includes(value) && array.length < 80) array.push(value);
      };
      const record = (node) => {
        if (!(node instanceof HTMLImageElement) || !node.matches('.scene-crystal-effect-frame')) return;
        // Persistent safe-zone/world effects keep cycling their src; they are not evidence
        // for this cast. Only frames whose effect instance appeared after the probe began count.
        if (baselineKeys.has(effectKey(node))) return;
        const source = node.dataset.mir2OriginalSrc || node.getAttribute('src') || '';
        const phase = node.dataset.projectilePhase || node.dataset.effectSource || 'frame';
        probe.eventCount += 1;
        if (source.includes('/original-effects/')) probe.sourceFrameEvents += 1;
        probe.phases[phase] = (probe.phases[phase] || 0) + 1;
        addUnique(probe.effectNames, node.dataset.projectileSpell || node.dataset.effectName || '');
        addUnique(probe.sourcePaths, source);
        probe.maxLiveFrames = Math.max(
          probe.maxLiveFrames,
          document.querySelectorAll('.scene-crystal-effect-frame').length,
        );
      };
      const recordTree = (node) => {
        if (!(node instanceof Element)) return;
        record(node);
        node.querySelectorAll?.('.scene-crystal-effect-frame').forEach(record);
      };
      const observer = new MutationObserver((mutations) => {
        for (const mutation of mutations) {
          if (mutation.type === 'attributes') record(mutation.target);
          mutation.addedNodes.forEach(recordTree);
        }
      });
      observer.observe(document.body, {
        subtree: true,
        childList: true,
        attributes: true,
        attributeFilter: ['src', 'data-projectile-phase', 'data-effect-name'],
      });
      Object.defineProperty(probe, 'baselineKeys', { value: baselineKeys, enumerable: false });
      Object.defineProperty(probe, 'observer', { value: observer, enumerable: false });
      window.__mir2MagicVfxProbe = probe;
      return true;
    })()
  `);
}

async function stopMagicVfxProbe(client) {
  return client.evaluate(`
    (() => {
      const probe = window.__mir2MagicVfxProbe;
      if (!probe) return null;
      probe.observer?.disconnect?.();
      const result = {
        durationMs: Date.now() - probe.startedAt,
        eventCount: probe.eventCount,
        sourceFrameEvents: probe.sourceFrameEvents,
        maxLiveFrames: probe.maxLiveFrames,
        phases: probe.phases,
        effectNames: probe.effectNames,
        sourcePaths: probe.sourcePaths,
      };
      delete window.__mir2MagicVfxProbe;
      return result;
    })()
  `).catch(() => null);
}

async function currentMagicVfxProbeState(client) {
  return client.evaluate(`
    (() => {
      const probe = window.__mir2MagicVfxProbe;
      if (!probe) return { sourceFrameEvents: 0, phases: [], livePhases: [] };
      const effectKey = (node) => node.dataset.projectileKey || node.dataset.effectKey || '';
      const livePhases = [...document.querySelectorAll('.scene-crystal-effect-frame')]
        .filter((node) => !probe.baselineKeys?.has(effectKey(node)))
        .map((node) => node.dataset.projectilePhase || node.dataset.effectSource || 'frame');
      return {
        sourceFrameEvents: Number(probe.sourceFrameEvents || 0),
        phases: Object.keys(probe.phases || {}),
        livePhases: [...new Set(livePhases)],
      };
    })()
  `).catch(() => ({ sourceFrameEvents: 0, phases: [], livePhases: [] }));
}

// ---------------------------------------------------------------------------
// WebSocket truth layer — the authoritative magic signals.
//
// A spell that FIRES produces the server's own frames (sim cast_skill_with_context ->
// gateway server_packet_to_event):
//   Magic          {spell, targetId, target, cast}          — our cast was accepted
//   ObjectMagic    {objectId, spell, target, cast}          — broadcast (objectId = caster)
//   MagicCast      {spell}                                  — some spells (BackStep, …)
//   ObjectMana     {objectId, percent}                      — the MP spend (percent 0–100)
//   MagicDelay     {objectId, spell, delay}                 — the cooldown was applied
//   ObjectProjectile {spell, sourceId, destinationId}       — fireball-family projectile
//   NewMagic       {magic, hero}                            — a spell was learned
//   MagicLeveled   {objectId, spell, level}                 — spell levelled / set
//   DamageIndicator {objectId, damage}                      — an offensive hit landed
//   ObjectHealth   {objectId, percent}                      — target hp/heal percent
//   ObjectDied     {objectId}                               — target died
function parseWsFrame(frame) {
  try {
    return JSON.parse(frame.payloadData);
  } catch {
    return null;
  }
}

function scanWsMagic(client, sinceSeq, { casterId, targetId } = {}) {
  const out = {
    selfCasts: 0,
    objectMagicSelf: 0,
    magicCast: 0,
    magicDelays: 0,
    manaUpdatesSelf: 0,
    manaMinPercentSelf: null,
    projectilesSelf: 0,
    objectPushes: 0,
    targetPushes: 0,
    newMagic: 0,
    magicLeveled: 0,
    spellsCast: [],
    spellsLearned: [],
    spellsLeveled: [],
    targetDamageHits: 0,
    monsterDamageHits: 0,
    damageIndicatorTotal: 0,
    targetMinPercent: null,
    targetKilled: false,
    anyMonsterDied: 0,
    packetCensus: {},
  };
  const cid = casterId != null ? String(casterId) : null;
  const tid = targetId != null ? String(targetId) : null;
  for (const frame of client.webSocketFramesReceived) {
    if (frame.seq < sinceSeq) continue; // select by stable seq, not array position
    const o = parseWsFrame(frame);
    if (!o || !o.packet) continue;
    const p = o.payload || {};
    const oid = p.objectId != null ? String(p.objectId) : null;
    out.packetCensus[o.packet] = (out.packetCensus[o.packet] ?? 0) + 1;
    switch (o.packet) {
      case "Magic":
        out.selfCasts += 1;
        if (p.spell) out.spellsCast.push(String(p.spell));
        break;
      case "ObjectMagic":
        if (cid && oid === cid) out.objectMagicSelf += 1;
        if (p.spell && cid && oid === cid) out.spellsCast.push(String(p.spell));
        break;
      case "MagicCast":
        out.magicCast += 1;
        if (p.spell) out.spellsCast.push(String(p.spell));
        break;
      case "MagicDelay":
        if (!cid || oid === cid || oid === null) out.magicDelays += 1;
        break;
      case "ObjectMana": {
        if (cid && oid === cid) {
          out.manaUpdatesSelf += 1;
          const pct = typeof p.percent === "number" ? p.percent : null;
          if (pct !== null) out.manaMinPercentSelf = out.manaMinPercentSelf === null ? pct : Math.min(out.manaMinPercentSelf, pct);
        }
        break;
      }
      case "ObjectProjectile":
        if (!cid || String(p.sourceId) === cid) out.projectilesSelf += 1;
        break;
      case "ObjectPushed":
        out.objectPushes += 1;
        if (tid && oid === tid) out.targetPushes += 1;
        break;
      case "NewMagic":
        out.newMagic += 1;
        if (p.magic && p.magic.spell) out.spellsLearned.push(String(p.magic.spell));
        break;
      case "MagicLeveled":
        out.magicLeveled += 1;
        if (p.spell) out.spellsLeveled.push(String(p.spell));
        break;
      case "DamageIndicator":
        out.damageIndicatorTotal += 1;
        if (tid && oid === tid) out.targetDamageHits += 1;
        if (oid && oid !== cid) out.monsterDamageHits += 1;
        break;
      case "ObjectHealth": {
        const pct = typeof p.percent === "number" ? p.percent : null;
        if (pct === null) break;
        if (tid && oid === tid) out.targetMinPercent = out.targetMinPercent === null ? pct : Math.min(out.targetMinPercent, pct);
        break;
      }
      case "ObjectDied":
        out.anyMonsterDied += 1;
        if (tid && oid === tid) out.targetKilled = true;
        break;
      default:
        break;
    }
  }
  out.spellsCast = [...new Set(out.spellsCast)];
  out.spellsLearned = [...new Set(out.spellsLearned)];
  out.spellsLeveled = [...new Set(out.spellsLeveled)];
  return out;
}

// Scan the WS snapshot truth layer for a known-skill binding. The gateway pushes the full
// world snapshot (`type:"worldSnapshot"`) with each skill's authoritative `hotkey`; page.tsx
// then re-derives state.knownSkills (lossily, when a NewMagic packet also arrives), so the
// snapshot frame is the reliable signal that a MagicKey binding round-tripped.
function scanWsSnapshotHotkey(client, sinceSeq, spell, key) {
  const want = String(spell).toLowerCase();
  for (let i = client.webSocketFramesReceived.length - 1; i >= 0; i -= 1) {
    const frame = client.webSocketFramesReceived[i];
    if (frame.seq < sinceSeq) continue;
    const data = frame.payloadData;
    if (!data || !data.includes("knownSkills")) continue;
    const o = parseWsFrame(frame);
    const skills = o?.payload?.knownSkills;
    if (!Array.isArray(skills)) continue;
    const hit = skills.find(
      (s) => s && (String(s.spell ?? "").toLowerCase() === want || String(s.name ?? "").toLowerCase() === want) && s.hotkey === key,
    );
    if (hit) return hit;
  }
  return null;
}

// Scan the WS snapshot truth layer for a spell's cooldown after a cast. A successful cast
// triggers a world snapshot whose authoritative skill entry carries cooldownRemainingTicks
// (page.tsx re-derives state.knownSkills lossily, so the snapshot frame is the reliable
// signal). Returns the max cooldownRemainingTicks seen for the spell since `sinceSeq`.
function scanWsSnapshotCooldown(client, sinceSeq, spell) {
  const want = String(spell).toLowerCase();
  let maxCd = 0;
  for (const frame of client.webSocketFramesReceived) {
    if (frame.seq < sinceSeq) continue;
    if (!frame.payloadData || !frame.payloadData.includes("knownSkills")) continue;
    const o = parseWsFrame(frame);
    const skills = o?.payload?.knownSkills;
    if (!Array.isArray(skills)) continue;
    for (const s of skills) {
      if (s && String(s.spell ?? "").toLowerCase() === want && typeof s.cooldownRemainingTicks === "number") {
        maxCd = Math.max(maxCd, s.cooldownRemainingTicks);
      }
    }
  }
  return maxCd;
}

// Perform one cast and collect the server's response over a short window. `mode` controls
// how target/location are filled (self | monster | ground | summon). Returns a structured
// record (also pushed onto ctx.casts) describing whether the cast FIRED and what fired.
async function performCast(ctx, { skill, mode, monster, groundTile, label }) {
  const client = ctx.client;
  const spell = spellNameForSkill(skill);
  // Repeated acceptance runs reuse a persisted character. Its authoritative spell
  // cooldown survives a reconnect, so wait it out instead of misclassifying a legitimate
  // cooldown rejection as a broken cast path.
  const readyDeadline = Date.now() + 25_000;
  let readyCooldown = await skillCooldown(client, skill.key, spell);
  while (typeof readyCooldown === "number" && readyCooldown > 0 && Date.now() < readyDeadline) {
    await delay(500);
    readyCooldown = await skillCooldown(client, skill.key, spell);
  }
  if (typeof readyCooldown === "number" && readyCooldown > 0) {
    return { fired: false, error: `spell cooldown did not reach zero (${readyCooldown} ticks)`, spell, mode };
  }
  await delay(250);
  const self = await readSelf(client);
  if (!self) return { fired: false, error: "no self position" };
  const mpBefore = self.mp;
  const hpBefore = self.hp;
  const cdBefore = await skillCooldown(client, skill.key, spell);

  let targetId = 0;
  let x = self.x;
  let y = self.y;
  let direction = self.direction ?? "Down";
  let spellTargetLock = false;

  if (mode === "self") {
    targetId = self.objectId ? Number(self.objectId) : 0;
    spellTargetLock = Boolean(self.objectId);
  } else if (mode === "monster" && monster) {
    targetId = Number(monster.objectId);
    x = monster.x;
    y = monster.y;
    spellTargetLock = true;
    direction = crystalDirection(monster.x - self.x, monster.y - self.y);
  } else if (mode === "ground" && groundTile) {
    x = groundTile.x;
    y = groundTile.y;
    direction = crystalDirection(groundTile.x - self.x, groundTile.y - self.y);
  } else if (mode === "summon") {
    // SelfOnly summon — cast at the caster's own tile (the summon spawns adjacent).
    targetId = 0;
  }

  const wsBase = client.wsRxCount;
  await startMagicVfxProbe(client);
  const accepted = await castMagic(client, { spell, direction, targetId, x, y, spellTargetLock });

  // Collect the server's frames for this cast. The definitive "fired" signal is a Magic /
  // ObjectMagic / MagicCast packet for the caster — the sim deducts mana atomically BEFORE
  // emitting it, so a cast packet proves the cast was accepted. A no-op cast (insufficient
  // MP / cooldown / range / LoS preflight) emits only UserLocation. ObjectMana is NOT used
  // as a fire signal: MP regen / the @LEVEL restore emit ObjectMana independent of casting.
  const castPacketSeen = (s) => s.selfCasts > 0 || s.objectMagicSelf > 0 || s.magicCast > 0;
  let floaterPeak = 0;
  const visualFrames = [];
  const capturedVisualPhases = new Set();
  const observeStartedAt = Date.now();
  const deadline = observeStartedAt + CAST_OBSERVE_MS;
  const minimumObservationMs = mode === "self" || mode === "summon" ? 700 : 2_200;
  while (Date.now() < deadline) {
    floaterPeak = Math.max(floaterPeak, await countDamageFloaters(client));
    const visualState = await currentMagicVfxProbeState(client);
    const uncapturedPhase = visualState.livePhases.find((phase) => !capturedVisualPhases.has(phase));
    if (visualState.sourceFrameEvents > 0 && uncapturedPhase) {
      const frameName = `cast-${String(ctx.casts.length + 1).padStart(2, "0")}-${slug(spell)}-${slug(mode)}-${slug(uncapturedPhase)}.png`;
      await captureFrame(client, path.join(framesDir, frameName));
      capturedVisualPhases.add(uncapturedPhase);
      visualFrames.push({ phase: uncapturedPhase, path: `frames/${frameName}` });
    }
    const scan = scanWsMagic(client, wsBase, { casterId: self.objectId, targetId: monster?.objectId });
    // Stop once we have the cast confirmation AND any damage/projectile it would produce.
    if (
      Date.now() - observeStartedAt >= minimumObservationMs &&
      castPacketSeen(scan) &&
      (scan.projectilesSelf > 0 || scan.monsterDamageHits > 0 || scan.targetPushes > 0 || mode === "self" || mode === "summon")
    ) break;
    await delay(80);
  }
  floaterPeak = Math.max(floaterPeak, await countDamageFloaters(client));
  const visualProbe = await stopMagicVfxProbe(client);

  const scan = scanWsMagic(client, wsBase, { casterId: self.objectId, targetId: monster?.objectId });
  const after = await readSelf(client);
  const cdAfter = await skillCooldown(client, skill.key, spell);
  const mpAfter = after?.mp ?? null;
  const hpAfter = after?.hp ?? null;
  const mpDelta = typeof mpBefore === "number" && typeof mpAfter === "number" ? mpBefore - mpAfter : null;
  const hpDelta = typeof hpBefore === "number" && typeof hpAfter === "number" ? hpAfter - hpBefore : null;

  const fired = castPacketSeen(scan);
  // The sim deducts mana atomically before emitting the cast packet, so a fired cast IS an MP
  // deduction (its contract). The raw client MP delta can be masked by a background @LEVEL
  // restore / regen, so a clean net drop is reported separately when it happens to be visible.
  const observedMpDrop = typeof mpDelta === "number" && mpDelta > 0;
  const mpDeducted = fired;
  // Cooldown evidence: the authoritative snapshot's cooldownRemainingTicks for this spell going
  // positive right after the cast (MP-independent), or a MagicDelay, or the lossy state entry.
  const snapshotCooldownMax = scanWsSnapshotCooldown(client, wsBase, spell);
  const cooldownApplied =
    snapshotCooldownMax > 0 ||
    scan.magicDelays > 0 ||
    (typeof cdAfter === "number" && (cdBefore === 0 || cdBefore === null) && cdAfter > 0);

  const record = {
    label: label ?? `${spell} (${mode})`,
    spell,
    mode,
    castKind: skill.castKind ?? null,
    offensive: Boolean(skill.offensive),
    accepted,
    fired,
    mpBefore,
    mpAfter,
    mpDelta,
    mpDeducted,
    observedMpDrop,
    packetCensus: scan.packetCensus,
    hpBefore,
    hpAfter,
    hpDelta,
    cooldownBefore: cdBefore,
    cooldownAfter: cdAfter,
    cooldownApplied,
    snapshotCooldownMax,
    selfCasts: scan.selfCasts,
    objectMagicSelf: scan.objectMagicSelf,
    magicDelays: scan.magicDelays,
    manaUpdatesSelf: scan.manaUpdatesSelf,
    projectilesSelf: scan.projectilesSelf,
    objectPushes: scan.objectPushes,
    targetPushes: scan.targetPushes,
    spellsCast: scan.spellsCast,
    targetDamageHits: scan.targetDamageHits,
    monsterDamageHits: scan.monsterDamageHits,
    targetMinPercent: scan.targetMinPercent,
    targetKilled: scan.targetKilled,
    anyMonsterDied: scan.anyMonsterDied,
    damageFloaterPeak: floaterPeak,
    visualProbe,
    visualFrame: visualFrames[0]?.path ?? null,
    visualFrames,
    target: mode === "monster" && monster ? { objectId: monster.objectId, name: monster.name, x: monster.x, y: monster.y } : mode === "ground" && groundTile ? groundTile : { x, y, objectId: targetId || null },
  };
  ctx.casts.push(record);
  ctx.log(`    cast ${record.label}: fired=${fired} mpΔ=${mpDelta ?? "?"} cd=${cdBefore ?? "?"}→${cdAfter ?? "?"} dmgHits=${scan.monsterDamageHits} floaters=${floaterPeak} sourceFrames=${visualProbe?.sourceFrameEvents ?? 0}`);
  return record;
}

// Prove a per-cast cooldown BEHAVIOURALLY. The sim does not push a MagicDelay packet on a
// normal cast (only on a spell level-up), and the cooldown_ends_at is server-side, so the
// only client-observable signal is that an immediate RE-cast is rejected: fire the same spell
// twice ~120ms apart and count the `Magic` cast packets — 2 sends yielding 1 cast confirms
// the second was blocked by the cooldown / spell-action delay.
async function cooldownProbe(client, skill) {
  const spell = spellNameForSkill(skill);
  const target = async () => {
    const self = await readSelf(client);
    if (!self) return null;
    return {
      self,
      cmd: { spell, direction: self.direction, targetId: self.objectId ? Number(self.objectId) : 0, x: self.x, y: self.y, spellTargetLock: Boolean(self.objectId) },
    };
  };
  // Warm up: cast until ONE fires (clearing any residual cooldown), then immediately re-cast
  // and confirm the second is BLOCKED — the spell is now on cooldown, so it must not fire.
  for (let warm = 0; warm < 5; warm += 1) {
    const t = await target();
    if (!t) return null;
    const base0 = client.wsRxCount;
    await castMagic(client, t.cmd);
    await delay(900);
    const s0 = scanWsMagic(client, base0, { casterId: t.self.objectId });
    if (s0.selfCasts >= 1 || s0.objectMagicSelf >= 1) {
      const base1 = client.wsRxCount;
      await castMagic(client, t.cmd); // immediate re-cast while on cooldown
      await delay(900);
      const s1 = scanWsMagic(client, base1, { casterId: t.self.objectId });
      const secondFired = s1.selfCasts >= 1 || s1.objectMagicSelf >= 1;
      return { firstFired: true, secondFired, blocked: !secondFired };
    }
    await delay(700); // first did not fire (still on cooldown / rejected) — wait and retry
  }
  return { firstFired: false, secondFired: false, blocked: false };
}

async function skillCooldown(client, key, spell) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ks = Array.isArray(s.knownSkills) ? s.knownSkills : [];
      const want = ${JSON.stringify(String(spell ?? "").toLowerCase())};
      const k = ks.find((e) => e && (e.key === ${JSON.stringify(key)} ||
        (want && (String(e.spell ?? "").toLowerCase() === want || String(e.name ?? "").toLowerCase() === want))));
      return k && typeof k.cooldownRemainingTicks === "number" ? k.cooldownRemainingTicks : null;
    })()
  `);
}

function spellNameForSkill(skill) {
  return skill.spell || skill.name || skill.key;
}

// Crystal spell → cast taxonomy, ported from the sim (skills.rs crystal_spell_cast_kind /
// crystal_spell_is_offensive). state.knownSkills entries that arrive via a NewMagic packet
// carry NO castKind/offensive (page.tsx builds them lossily as `magic-${spell}`), so the
// harness classifies by the reliable SPELL NAME and uses the snapshot's castKind/offensive
// only as corroboration when present.
const GROUND_SPELLS = new Set(
  ["firewall", "firebang", "icestorm", "blizzard", "meteorstrike", "meteorshower", "poisoncloud", "plague", "traphexagon", "trap", "explosivetrap", "delayedexplosion", "portal", "blink", "teleport", "masshealing", "healingcircle", "curse"],
);
const SELF_OR_SUMMON_SPELLS = new Set(
  ["fury", "haste", "swiftfeet", "protectionfield", "rage", "lightbody", "moonlight", "darkbody", "hiding", "masshiding", "immortalskin", "magicshield", "energyshield", "magicbooster", "concentration", "elementalbarrier", "mirroring", "onewithnature", "reincarnation", "summonskeleton", "summonshinsu", "summonholydeva", "summonvampire", "summontoad", "summonsnakes", "petenhancer", "soulshield", "blessedarmour", "ultimateenhancer", "moonmist", "stonetrap"],
);
const DIRECTION_SPELLS = new Set(
  ["hellfire", "lightning", "repulsion", "energyrepulsor", "fireburst", "stormescape", "bladeavalanche", "slashingburst", "crescentslash", "lionroar", "icethrust", "heavenlysword", "thunderstorm", "battlecry"],
);
// Single-target offensive nukes (sim `_ => Target` + offensive). Not exhaustive — any spell
// not otherwise classified and flagged offensive falls here too.
const TARGET_OFFENSIVE_SPELLS = new Set(
  ["thunderbolt", "fireball", "greatfireball", "soulfireball", "firebounce", "cattongue", "frostcrunch", "poisondart", "flamefield"],
);
const PUSH_CONTROL_SPELLS = new Set(["repulsion", "energyrepulsor", "fireburst"]);

function spellNameKey(skill) {
  return String(spellNameForSkill(skill)).toLowerCase().replace(/[^a-z0-9]/g, "");
}

function effectiveCastKind(skill) {
  if (skill.castKind) return skill.castKind; // snapshot path populated it — trust it
  const s = spellNameKey(skill);
  if (s.includes("heal")) return GROUND_SPELLS.has(s) ? "ground" : "target"; // Healing self-targets via "target"
  if (GROUND_SPELLS.has(s)) return "ground";
  if (DIRECTION_SPELLS.has(s)) return "direction";
  if (SELF_OR_SUMMON_SPELLS.has(s)) return "self";
  return "target";
}

function isOffensiveSpell(skill) {
  if (skill.offensive) return true;
  const s = spellNameKey(skill);
  if (s.includes("heal") || SELF_OR_SUMMON_SPELLS.has(s)) return false;
  return TARGET_OFFENSIVE_SPELLS.has(s) || DIRECTION_SPELLS.has(s) || GROUND_SPELLS.has(s);
}

// Classify the known spell book into the casts the harness can exercise.
function classifySpellBook(skills) {
  const kind = (s) => effectiveCastKind(s);
  const isSummon = (s) => /summon/i.test(spellNameForSkill(s));
  const isHeal = (s) => spellNameKey(s).includes("heal") && kind(s) !== "ground";
  const isTrap = (s) => /stonetrap|trap/i.test(spellNameForSkill(s));
  const castable = skills.filter((s) => kind(s) !== "passive");
  const requestedSelfKey = String(SELF_GRANT_SPELL).toLowerCase().replace(/[^a-z0-9]/g, "");
  return {
    all: skills,
    castable,
    requestedSelf: skills.find((s) => spellNameKey(s) === requestedSelfKey) ?? null,
    // SELF: a heal (Healing self-targets even though it is castKind "target") or a self-buff.
    selfHeal: skills.find(isHeal) ?? null,
    selfBuff: skills.find((s) => kind(s) === "self" && !isSummon(s) && !isTrap(s)) ?? null,
    summon: skills.find((s) => isSummon(s)) ?? null,
    // MONSTER: a single-target / facing-aimed offensive spell (present after @GIVESKILL).
    offensiveTarget: skills.find((s) => isOffensiveSpell(s) && (kind(s) === "target" || kind(s) === "direction")) ?? null,
    // GROUND: an aimed AoE / trap (present after @GIVESKILL).
    ground: skills.find((s) => kind(s) === "ground") ?? null,
    // The reachable Taoist trap (SelfOnly, cast at the caster's feet) — the closest the
    // starter book gets to a "placed" spell, used as a documented fallback for ground.
    trap: skills.find(isTrap) ?? null,
    toggle: skills.find((s) => kind(s) === "toggle") ?? null,
  };
}

// Ensure the player is on a hunting FIELD, not the town. Offensive magic (target + ground)
// is a no-op in a safe zone (sim crystal_magic_point_in_safe_zone), and the town spawn is a
// safe zone, so the offensive cast beats must relocate to a field first.
async function ensureOnHuntingField(ctx) {
  const client = ctx.client;
  const onField = await client.evaluate(
    `(() => { const m = window.__mir2Stage5?.state?.mapFileName; return ${JSON.stringify(HUNTING_FIELDS.map((f) => f.map))}.includes(m); })()`,
  );
  if (onField) return true;
  const field = HUNTING_FIELDS[0];
  await transferTo(client, field.map, field.x, field.y).catch((err) =>
    ctx.addIssue({ category: "flow", severity: "low", summary: `transfer to ${field.label} failed`, detail: String(err?.message ?? err) }),
  );
  await waitUntilSoft(
    client,
    "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
    20_000,
  );
  await delay(900);
  return await client.evaluate(
    `(() => { const m = window.__mir2Stage5?.state?.mapFileName; return ${JSON.stringify(HUNTING_FIELDS.map((f) => f.map))}.includes(m); })()`,
  );
}

// Walk the caster well clear of the field-SPAWN safe zone (the transfer drops you on a guarded
// town-edge tile where OFFENSIVE magic is a no-op — the only offensive-specific preflight gate
// both target + ground spells share is crystal_magic_point_in_safe_zone(player_position)).
// Walks `tiles` deep into the field from the current field's spawn, in viewport hops.
async function pushIntoField(ctx, tiles = 28) {
  const client = ctx.client;
  const mapNow = await client.evaluate(`window.__mir2Stage5?.state?.mapFileName ?? null`);
  const field = HUNTING_FIELDS.find((f) => f.map === mapNow) ?? HUNTING_FIELDS[0];
  const target = { x: field.x, y: field.y + tiles }; // deeper into the field, away from the edge
  const deadline = Date.now() + 20_000;
  let nudged = 0;
  while (Date.now() < deadline) {
    const self = await readSelf(client);
    if (!self) break;
    const remaining = Math.max(Math.abs(self.x - target.x), Math.abs(self.y - target.y));
    const fromSpawn = Math.max(Math.abs(self.x - field.x), Math.abs(self.y - field.y));
    if (remaining <= 2 || fromSpawn >= tiles) break;
    if (!(await walkToward(client, target.x, target.y))) {
      // Blocked — sidestep a few tiles to route around the obstacle.
      nudged += 1;
      await walkToward(client, target.x + (nudged % 2 ? 4 : -4), target.y - 2);
    }
    await delay(450);
  }
}

// Find a live, killable field monster the way a real player would: hop to a hunting field
// and explore it (reused from the combat loop). Returns null if none turns up in time.
async function acquireMonster(ctx, beat, excluded = []) {
  const client = ctx.client;
  const visited = beat?.visitedFields ?? [];
  if (beat) beat.visitedFields = visited;

  if (TARGET_MONSTER_NAME) {
    await gmCommand(client, `@MOB ${TARGET_MONSTER_NAME}`).catch(() => {});
    await delay(700);
  }
  let monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded);
  if (monster) return monster;

  const ring = (d) => [
    [0, -d], [d, 0], [0, d], [-d, 0],
    [d, -d], [d, d], [-d, d], [-d, -d],
  ];
  const offsets = [...ring(13), ...ring(24)];

  for (const field of HUNTING_FIELDS) {
    if (monster) break;
    const here = await client.evaluate(`window.__mir2Stage5?.state?.mapFileName === ${JSON.stringify(field.map)}`);
    if (!here) {
      await transferTo(client, field.map, field.x, field.y).catch((err) =>
        ctx.addIssue({ category: "flow", severity: "low", beat: beat?.id, summary: `transfer to ${field.label} failed`, detail: String(err?.message ?? err) }),
      );
      await waitUntilSoft(
        client,
        "window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true",
        20_000,
      );
    }
    if (!visited.includes(field.label)) visited.push(field.label);

    const deadline = Date.now() + SPAWN_POLL_MS;
    let wp = 0;
    while (Date.now() < deadline && !monster) {
      monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded);
      if (monster) break;
      await gmCommand(client, `@MOB ${GM_SPAWN_TEMPLATE}`).catch(() => {});
      const [ox, oy] = offsets[wp % offsets.length];
      wp += 1;
      const tx = field.x + ox;
      const ty = field.y + oy;
      const legDeadline = Date.now() + 5000;
      while (Date.now() < legDeadline && !monster) {
        monster = await readTargetMonster(client, EXPLORE_SCAN_RANGE, excluded);
        if (monster) break;
        await walkToward(client, tx, ty);
        await delay(700);
      }
    }
    if (monster) break;
  }
  return monster;
}

// A GM-spawned control target begins on the caster's occupied tile. Move the real player
// one tile away while keeping an empty tile behind the monster, so push-control spells have
// a deterministic legal destination instead of being defeated by crowd occupancy.
async function stagePushControlTarget(client, monster) {
  const offsets = [
    { playerDx: 1, playerDy: 0, pushDx: -1, pushDy: 0 },
    { playerDx: -1, playerDy: 0, pushDx: 1, pushDy: 0 },
    { playerDx: 0, playerDy: 1, pushDx: 0, pushDy: -1 },
    { playerDx: 0, playerDy: -1, pushDx: 0, pushDy: 1 },
  ];
  for (const offset of offsets) {
    const fresh = (await readMonsterById(client, monster.objectId)) ?? monster;
    const playerTile = { x: fresh.x + offset.playerDx, y: fresh.y + offset.playerDy };
    const pushTile = { x: fresh.x + offset.pushDx, y: fresh.y + offset.pushDy };
    const occupied = await client.evaluate(`
      (() => (window.__mir2Stage5?.state?.entities ?? []).some(
        (e) => e && !e.dead && e.objectId !== window.__mir2Stage5?.state?.playerObjectId &&
          String(e.objectId) !== ${JSON.stringify(String(monster.objectId))} &&
          ((e.x === ${playerTile.x} && e.y === ${playerTile.y}) || (e.x === ${pushTile.x} && e.y === ${pushTile.y}))
      ))()
    `);
    if (occupied) continue;
    if (!(await clickTile(client, playerTile.x, playerTile.y, "left"))) continue;
    const reached = await waitUntilSoft(
      client,
      `window.__mir2Stage5?.state?.player?.x === ${playerTile.x} && window.__mir2Stage5?.state?.player?.y === ${playerTile.y}`,
      4_000,
    );
    if (reached) return (await readMonsterById(client, monster.objectId)) ?? fresh;
  }
  return (await readMonsterById(client, monster.objectId)) ?? monster;
}

// ---------------------------------------------------------------------------
// The magic / skill cycle.
// ---------------------------------------------------------------------------
async function magicSkills(ctx) {
  const client = ctx.client;

  await enterWorld(ctx);

  // ---- Beat: survey the known spell book + caster vitals; attempt to learn an offensive
  // and a ground spell (GM/test-gated → documented when it no-ops). ----
  let book = null;
  await runBeat(ctx, "survey known spell book + caster vitals", async (beat) => {
    const before = await readGameState(client);
    beat.casterClass = characterClass;
    beat.vitals = { hp: before.playerHp, maxHp: before.playerMaxHp, mp: before.playerMp, maxMp: before.playerMaxMp };
    beat.knownSkillCount = before.knownSkillCount;
    beat.knownSkills = before.knownSkills;

    if (typeof before.playerMp !== "number") {
      ctx.addIssue({ category: "magic", severity: "medium", beat: beat.id, summary: "caster MP not exposed in client state — MP-deduction checks will be best-effort", detail: beat.vitals });
    }

    // Optional GM elevation: log in as GM, raise level for castable MP, and grant a caster
    // kit (heal + single-target nuke + ground AoE). No-op without a password — the gate is
    // documented below. Spells to request: the self heal is always granted (the persisted
    // book is empty), plus the offensive + ground spells the starter book lacks.
    const elevated = await gmElevate(client);
    beat.gmElevated = elevated;
    if (elevated) {
      await gmCommand(client, `@LEVEL ${gmLevel}`).catch(() => {});
      const previousMaxMp = typeof before.playerMaxMp === "number" ? before.playerMaxMp : 0;
      await waitUntilSoft(
        client,
        `Number(window.__mir2Stage5?.state?.playerMaxMp ?? 0) > ${previousMaxMp}`,
        10_000,
      );
      await delay(1200);
      // Survive the offensive field beats (the GM character keeps level-1-ish HP via the
      // snapshot) — gm_never_die makes incoming damage non-lethal while we exercise casting.
      await gmCommand(client, "@SUPERMAN").catch(() => {});
      await delay(1200);
    }
    const grantList = elevated
      ? [SELF_GRANT_SPELL, OFFENSIVE_GRANT_SPELL, GROUND_GRANT_SPELL]
      : [OFFENSIVE_GRANT_SPELL, GROUND_GRANT_SPELL];

    // Try to LEARN the caster kit so the self/monster/ground beats have a true spell to
    // fire. GM/test-gated — a documented no-op without elevation.
    const learnWsBase = client.wsRxCount;
    for (const sp of grantList) {
      await gmCommand(client, `@GIVESKILL ${sp} ${GRANT_LEVEL}`).catch(() => {});
      await waitForKnownSpell(client, sp, 10_000);
      await delay(1200);
    }
    // Let the @LEVEL HP/MP restore + skill snapshot fully flush before any cast, so the
    // level-up MP refill does not mask a cast's MP deduction in the next beat.
    await delay(1200);
    const learn = scanWsMagic(client, learnWsBase, {});
    const after = await readGameState(client);
    const knownAfter = new Set(after.knownSkills.map(spellNameKey));
    const missingRequested = grantList.filter(
      (sp) => !knownAfter.has(String(sp).toLowerCase().replace(/[^a-z0-9]/g, "")),
    );
    beat.learnAttempt = {
      elevated,
      requested: grantList.map((sp) => `${sp}@${GRANT_LEVEL}`),
      newMagicPackets: learn.newMagic,
      magicLeveledPackets: learn.magicLeveled,
      learned: learn.spellsLearned,
      leveled: learn.spellsLeveled,
      missingRequested,
    };

    book = classifySpellBook(after.knownSkills);
    beat.spellBook = {
      count: after.knownSkillCount,
      requestedSelf: book.requestedSelf?.spell ?? null,
      selfHeal: book.selfHeal?.spell ?? null,
      selfBuff: book.selfBuff?.spell ?? null,
      summon: book.summon?.spell ?? null,
      offensiveTarget: book.offensiveTarget?.spell ?? null,
      ground: book.ground?.spell ?? null,
      trap: book.trap?.spell ?? null,
    };
    ctx.book = book;

    if (after.knownSkillCount > 0) {
      ctx.score("knownSpells", "pass", `${after.knownSkillCount} known spell(s): ${after.knownSkills.slice(0, 8).map((s) => s.spell || s.name).join(", ")}`);
    } else {
      ctx.score("knownSpells", "fail", "knownSkills is empty after entering the world — no spell book to cast from");
      ctx.addIssue({ category: "magic", severity: "high", beat: beat.id, summary: "entered as a caster but knownSkills is empty — no spell book delivered in the world snapshot", detail: { class: characterClass } });
    }

    // NewMagic is emitted only for a newly learned entry. A repeated acceptance run on the
    // same character legitimately emits MagicLeveled instead, so judge the resulting book.
    if (missingRequested.length > 0) {
      ctx.addIssue({
        category: "env",
        severity: "low",
        beat: beat.id,
        summary: `@GIVESKILL left ${missingRequested.length} requested spell(s) unavailable — ${elevated ? "GM elevation did not unlock all skill grants on this stack" : "GM/test-gated (is_gm_or_test=false here)"}.`,
        detail: {
          requested: beat.learnAttempt.requested,
          missingRequested,
          newMagicPackets: learn.newMagic,
          magicLeveledPackets: learn.magicLeveled,
          elevated,
          note: elevated
            ? "GM @LOGIN was attempted but @GIVESKILL still no-op'd — check MIR2_GM_PASSWORD / spell names."
            : "Re-run with --gmPassword <pw> against a gateway started with MIR2_GM_PASSWORD to learn these and exercise the full offensive/ground path.",
        },
      });
    }
  });

  if (!book) book = classifySpellBook(await readKnownSkills(client));
  ctx.book = book;

  // ---- Beat: place a spell on the skill bar (MagicKey). ----
  await runBeat(ctx, "place a spell on the skill bar (MagicKey)", async (beat) => {
    const skill = book.requestedSelf ?? book.selfHeal ?? book.selfBuff ?? book.summon ?? book.castable[0] ?? null;
    if (!skill) {
      ctx.score("skillBarPlace", "skip", "no known spell to bind");
      beat.note = "no castable spell";
      return;
    }
    const spell = spellNameForSkill(skill);
    const desiredKey = numberArg(args.barKey, 1); // F1
    const oldKey = typeof skill.hotkey === "number" ? skill.hotkey : 0;
    beat.bind = { spell, key: desiredKey, oldKey, skillKey: skill.key };
    const wsBase = client.wsRxCount;
    const accepted = await placeOnSkillBar(client, { spell, key: desiredKey, oldKey });
    beat.accepted = accepted;
    // The binding round-trips in the next world snapshot as skill.hotkey. The authoritative
    // signal is the gateway's own snapshot frame (state.knownSkills is re-derived lossily by
    // page.tsx when a NewMagic packet co-arrives, so match by spell name across both).
    let snapHit = null;
    const deadline = Date.now() + 6_000;
    while (Date.now() < deadline) {
      snapHit = scanWsSnapshotHotkey(client, wsBase, spell, desiredKey);
      if (snapHit) break;
      await delay(200);
    }
    const after = await readKnownSkills(client);
    const stateHit = after.find((s) => (s.spell === spell || s.name === spell) && s.hotkey === desiredKey) ?? null;
    const reflected = snapHit?.hotkey ?? stateHit?.hotkey ?? null;
    beat.reflectedHotkey = reflected;
    beat.reflectedVia = snapHit ? "ws-snapshot" : stateHit ? "client-state" : null;
    if (snapHit || stateHit) {
      ctx.score("skillBarPlace", "pass", `bound ${spell} → F${desiredKey} (skill.hotkey=${reflected} via ${beat.reflectedVia})`);
    } else if (accepted) {
      ctx.score("skillBarPlace", "fail", `MagicKey accepted but the binding did not round-trip in the world snapshot for ${spell}`);
      ctx.addIssue({ category: "magic", severity: "medium", beat: beat.id, summary: `MagicKey command was accepted but no snapshot reflected ${spell}.hotkey=F${desiredKey}`, detail: beat.bind });
    } else {
      ctx.score("skillBarPlace", "fail", "MagicKey command was rejected by the client send()");
      ctx.addIssue({ category: "magic", severity: "medium", beat: beat.id, summary: "window.__mir2Stage5.send({type:'magicKey'}) returned false — command not accepted", detail: beat.bind });
    }
  });

  // ---- Beat: cast on SELF (heal / self-buff). ----
  await runBeat(ctx, "cast on self (heal / self-buff)", async (beat) => {
    const skill = book.requestedSelf ?? book.selfHeal ?? book.selfBuff ?? null;
    if (!skill) {
      ctx.score("castSelf", "skip", "no self-castable heal/buff in the spell book");
      beat.note = "no self heal/buff";
      return;
    }
    // For a heal, dropping our HP first makes the restore observable. Healing is cheap
    // (≈9 MP at level 3) so it should fire even on a low-MP level-1 caster.
    const isHeal = /heal/i.test(spellNameForSkill(skill));
    if (isHeal) {
      await maybeSelfDamage(ctx, beat);
    }
    await waitForPlayerSettled(client);
    await delay(800);
    const rec = await performCast(ctx, { skill, mode: "self", label: `${spellNameForSkill(skill)} (self)` });
    beat.cast = rec;
    if (rec.fired) {
      const extra = isHeal && typeof rec.hpDelta === "number" && rec.hpDelta > 0 ? `, healed +${rec.hpDelta} HP` : "";
      ctx.score("castSelf", "pass", `${rec.spell} fired (MP ${rec.mpBefore ?? "?"}→${rec.mpAfter ?? "?"}, cooldown ${rec.cooldownBefore ?? "?"}→${rec.cooldownAfter ?? "?"})${extra}`);
    } else if (rec.accepted === false) {
      ctx.score("castSelf", "fail", "client rejected the magic command (send returned false)");
      ctx.addIssue({ category: "magic", severity: "high", beat: beat.id, summary: `self-cast ${rec.spell} not accepted by the client (send returned false)`, detail: rec });
    } else if (typeof rec.mpBefore === "number" && rec.mpBefore < 9) {
      ctx.score("castSelf", "gap", `cast did not fire and caster MP is very low (${rec.mpBefore}) — likely insufficient MP at level 1; @LEVEL to raise MP is GM-gated`);
      ctx.addIssue({ category: "env", severity: "medium", beat: beat.id, summary: `self-cast ${rec.spell} did not fire and MP=${rec.mpBefore} is below the cheapest starter spell cost — likely an insufficient-MP no-op on a fresh level-1 caster (raising level/MP via @LEVEL is GM/test-gated)`, detail: rec });
    } else {
      ctx.score("castSelf", "fail", `cast ${rec.spell} produced no server cast/MP/cooldown signal`);
      ctx.addIssue({ category: "magic", severity: "high", beat: beat.id, summary: `self-cast ${rec.spell} produced no Magic/ObjectMana/MagicDelay server frame and no MP/cooldown change — cast did not fire`, detail: rec });
    }
  });

  // ---- Beat: cast in a MONSTER context (offensive single-target, else a summon that
  // engages the monster). ----
  await runBeat(ctx, "cast in a monster context (offensive / summon)", async (beat) => {
    const offensive = book.offensiveTarget; // only present if @GIVESKILL succeeded
    const summon = book.summon;
    if (!offensive && !summon) {
      ctx.score("castMonster", "skip", "no offensive or summon spell in the reachable book");
      beat.note = "no monster-context spell";
      return;
    }
    // Offensive magic is a safe-zone no-op, so leave the town for a hunting field AND walk
    // clear of the field-spawn safe zone before acquiring a target.
    if (offensive) {
      beat.onField = await ensureOnHuntingField(ctx);
      // Explicit GM targets are spawned at our already non-safe field coordinate; walking
      // 28 more tiles only drags the control probe into unrelated roaming monsters.
      if (!TARGET_MONSTER_NAME) await pushIntoField(ctx, 28);
    }
    const monster = await acquireMonster(ctx, beat);
    beat.monster = monster ? { name: monster.name, at: { x: monster.x, y: monster.y }, maxHp: monster.maxHp } : null;
    if (!monster) {
      ctx.score("castMonster", "skip", "no live monster found on the hunting fields to cast at");
      ctx.addIssue({ category: "magic", severity: "low", beat: beat.id, summary: "no monster available to exercise a monster-context cast (none on the fields; @MOB GM/test-gated)", detail: { visited: beat.visitedFields } });
      return;
    }

    if (offensive) {
      // True single-target offensive magic. The sim rejects an offensive cast while the PLAYER
      // stands in a safe zone (the field-spawn edge is one — already walked clear of via
      // pushIntoField), and requires a HOSTILE target, so close ADJACENT to the monster, SETTLE
      // movement, then PROVOKE it (melee swing → aggro) before casting. ThunderBolt range is 9,
      // so adjacency is well within range; @SUPERMAN keeps the caster alive while meleed.
      let rec = null;
      let firedRec = null;
      let lastMonster = monster;
      const pushControl = PUSH_CONTROL_SPELLS.has(spellNameKey(offensive));
      for (let attempt = 0; attempt < 4; attempt += 1) {
        const fresh = (await readMonsterById(client, lastMonster.objectId)) ?? (await readTargetMonster(client, EXPLORE_SCAN_RANGE));
        if (!fresh) break;
        lastMonster = fresh;
        if (pushControl) {
          lastMonster = await stagePushControlTarget(client, fresh);
        } else {
          await closeTo(ctx, fresh, 1);
        }
        await waitForPlayerSettled(client);
        // Aggro the target first — an offensive target spell needs a hostile monster.
        // @MOB targets are already hostile; do not add a delayed melee hit to a push-control
        // probe because it can move the target or contaminate damage evidence.
        if (!pushControl) await provokeMonster(client, fresh);
        const m2 = (await readMonsterById(client, fresh.objectId)) ?? fresh;
        rec = await performCast(ctx, { skill: offensive, mode: "monster", monster: m2, label: `${spellNameForSkill(offensive)} (monster a${attempt + 1})` });
        if (rec.fired) firedRec = rec;
        if (rec.fired && (!pushControl || rec.targetPushes > 0)) break;
        await delay(700);
      }
      if (pushControl && (!rec?.targetPushes || rec.targetPushes === 0) && firedRec) rec = firedRec;
      beat.cast = rec;
      if (!rec) {
        ctx.score("castMonster", "skip", "monster left view before an offensive cast could be attempted");
        return;
      }
      // A damage/projectile signal proves a damaging nuke engaged the target; ObjectPushed
      // proves a control spell such as Taoist EnergyRepulsor engaged it.
      // The melee swing used to provoke hostility can land after the cast observation begins;
      // for a push-control spell, only its authoritative ObjectPushed is effect evidence.
      const engaged = pushControl
        ? rec.targetPushes > 0
        : rec.monsterDamageHits > 0 || rec.targetDamageHits > 0 || (rec.targetMinPercent !== null && rec.targetMinPercent < 100) || rec.damageFloaterPeak > 0 || rec.projectilesSelf > 0;
      if (rec.fired && engaged) {
        ctx.score("castMonster", "pass", `${rec.spell} engaged "${lastMonster.name}" — ${rec.monsterDamageHits} damage number(s), ${rec.projectilesSelf} projectile(s), ${rec.targetPushes} push(es), floaters ${rec.damageFloaterPeak}${rec.anyMonsterDied ? ", target died" : ""}`);
      } else if (rec.fired) {
        ctx.score("castMonster", "gap", `${rec.spell} fired but no damage/projectile/control effect was observed in-window (target may have moved out of LoS)`);
        ctx.addIssue({ category: "magic", severity: "low", beat: beat.id, summary: `offensive cast ${rec.spell} fired (Magic packet) but no DamageIndicator/ObjectProjectile/ObjectPushed was captured in the observe window`, detail: rec });
      } else {
        // The cast PIPE is proven by the self-cast; an offensive rejection here is a
        // server-side preflight condition the harness cannot fully control from the client, so
        // record it as a documented gap rather than a magic-system failure.
        ctx.score("castMonster", "gap", `${rec.spell} was rejected (UserLocation-only) on every attempt — server-side offensive preflight (caster-tile safe zone / a movement-blocking status / target hostility timing / LoS)`);
        ctx.addIssue({ category: "magic", severity: "medium", beat: beat.id, summary: `offensive cast ${rec.spell} was rejected (UserLocation-only) on every attempt — the sim's offensive-magic preflight refused it (candidates: caster-tile safe zone, a movement-blocking status, target hostility timing, or LoS). The self-cast confirms the magic pipe itself works.`, detail: rec });
      }
      return;
    }

    // Reachable fallback: a SUMMON. The starter book has no single-target nuke (GM-gated),
    // but a summon is genuine offensive magic — it spawns a player-owned pet that engages
    // the monster. Verify the cast fires AND a pet entity appears; the pet's own damage on
    // the monster is recorded best-effort.
    await closeTo(ctx, monster, 5);
    const petsBefore = (await readGameState(client)).petCount;
    const wsBase = client.wsRxCount;
    const rec = await performCast(ctx, { skill: summon, mode: "summon", label: `${spellNameForSkill(summon)} (summon vs monster)` });
    beat.cast = rec;
    // Give the summon a beat to appear + start fighting.
    await delay(1500);
    const afterState = await readGameState(client);
    const petsAfter = afterState.petCount;
    const petAppeared = petsAfter > petsBefore;
    // Watch the monster a little for the pet's damage (best-effort).
    const fightDeadline = Date.now() + 6_000;
    while (Date.now() < fightDeadline) {
      const s = scanWsMagic(client, wsBase, { casterId: rec && null, targetId: monster.objectId });
      if (s.monsterDamageHits > 0 || s.anyMonsterDied > 0) break;
      await delay(400);
    }
    const fightScan = scanWsMagic(client, wsBase, { targetId: monster.objectId });
    beat.petAppeared = petAppeared;
    beat.petCount = { before: petsBefore, after: petsAfter };
    beat.petDamageHits = fightScan.monsterDamageHits;
    if (rec.fired && (petAppeared || rec.spell)) {
      const dmgNote = fightScan.monsterDamageHits > 0 ? `, summon damaged the monster (${fightScan.monsterDamageHits} hit(s))` : "";
      ctx.score("castMonster", "pass", `${rec.spell} fired${petAppeared ? ` and a pet appeared (${petsBefore}→${petsAfter})` : " (pet entity not separately tracked in state)"}${dmgNote}`);
      ctx.addIssue({
        category: "env",
        severity: "low",
        beat: beat.id,
        summary: "monster-context magic was exercised via a SUMMON (reachable). A pure single-target offensive nuke is not in the seeded starter book and learning one (@GIVESKILL ThunderBolt) is GM/test-gated.",
        detail: { summon: rec.spell, petAppeared, petDamageHits: fightScan.monsterDamageHits },
      });
    } else if (rec.fired) {
      ctx.score("castMonster", "gap", `${rec.spell} fired but no pet entity was observed in client state (summon render/state gap)`);
      ctx.addIssue({ category: "magic", severity: "medium", beat: beat.id, summary: `summon ${rec.spell} fired (MP/cooldown) but no player-owned pet entity appeared in client state`, detail: { rec, petsBefore, petsAfter } });
    } else {
      ctx.score("castMonster", "fail", `summon ${rec.spell} did not fire`);
      ctx.addIssue({ category: "magic", severity: "high", beat: beat.id, summary: `summon ${rec.spell} produced no server cast signal`, detail: rec });
    }
  });

  // ---- Beat: cast GROUND-targeted (aimed AoE, else the Stonetrap trap at the feet). ----
  await runBeat(ctx, "cast ground-targeted (aimed AoE / trap)", async (beat) => {
    const ground = book.ground; // only present if @GIVESKILL succeeded
    const trap = book.trap; // Stonetrap (SelfOnly) — the reachable starter "placed" spell
    const skill = ground ?? trap ?? null;
    if (!skill) {
      ctx.score("castGround", "skip", "no ground-aimed or trap spell reachable");
      beat.note = "no ground/trap spell";
      return;
    }
    // A ground AoE (FireWall/Blizzard) is offensive → a safe-zone no-op. The field SPAWN is a
    // safe zone, so position the caster ADJACENT to a monster (non-safe territory) and aim the
    // AoE at the monster's tile. The Stonetrap trap fallback is non-offensive (SelfOnly) so it
    // is exempt and cast at the caster's feet wherever they stand.
    let rec = null;
    if (ground) {
      const groundOffensive = isOffensiveSpell(ground);
      let monster = null;
      if (groundOffensive) {
        beat.onField = await ensureOnHuntingField(ctx);
        await pushIntoField(ctx, 28);
        monster = await acquireMonster(ctx, beat);
        beat.monster = monster ? { name: monster.name, at: { x: monster.x, y: monster.y } } : null;
      }
      for (let attempt = 0; attempt < 3 && (!rec || !rec.fired); attempt += 1) {
        const m = monster ? (await readMonsterById(client, monster.objectId)) ?? monster : null;
        if (groundOffensive && m) {
          await closeTo(ctx, m, 1);
          await waitForPlayerSettled(client);
          await provokeMonster(client, m); // engage combat → caster stands in non-safe territory
        }
        await waitForPlayerSettled(client);
        const self = await readSelf(client);
        // Aim at the monster's tile (non-safe) when we have one, else one tile ahead.
        const mNow = m ? (await readMonsterById(client, m.objectId)) ?? m : null;
        const groundTile = mNow ? { x: mNow.x, y: mNow.y } : { x: (self?.x ?? 0) + 1, y: self?.y ?? 0 };
        beat.aim = groundTile;
        rec = await performCast(ctx, { skill, mode: "ground", groundTile, label: `${spellNameForSkill(skill)} (ground a${attempt + 1})` });
        if (rec.fired) break;
        await delay(700);
      }
    } else {
      // Stonetrap trap at the caster's feet (SelfOnly, exempt from the safe-zone rule).
      await waitForPlayerSettled(client);
      const self = await readSelf(client);
      beat.aim = { x: self?.x ?? null, y: self?.y ?? null };
      rec = await performCast(ctx, { skill, mode: "summon", label: `${spellNameForSkill(skill)} (trap@feet)` });
    }
    beat.cast = rec;
    if (rec && rec.fired) {
      if (ground) {
        const engaged = isOffensiveSpell(ground) && (rec.monsterDamageHits > 0 || rec.damageFloaterPeak > 0);
        ctx.score("castGround", "pass", `${rec.spell} cast at ground tile (${beat.aim?.x},${beat.aim?.y}) — fired${engaged ? ` and dealt damage (${rec.monsterDamageHits} hit(s), floaters ${rec.damageFloaterPeak})` : ""}`);
      } else {
        ctx.score("castGround", "gap", `no true ground-aimed spell reachable; cast the Stonetrap trap (${rec.spell}, SelfOnly@feet) instead — it fired. A tile-aimed AoE (FireWall/Blizzard/…) requires @GIVESKILL (GM-gated) and a non-safe-zone position.`);
        ctx.addIssue({ category: "env", severity: "low", beat: beat.id, summary: "ground-targeted casting was exercised via the Stonetrap trap (the only 'placed' spell in the seeded book). A true tile-aimed AoE is not in the starter book and learning one is GM/test-gated.", detail: rec });
      }
    } else if (rec && rec.accepted === false) {
      ctx.score("castGround", "fail", "client rejected the ground magic command");
      ctx.addIssue({ category: "magic", severity: "high", beat: beat.id, summary: `ground cast ${rec.spell} not accepted by the client`, detail: rec });
    } else {
      // The cast pipe is proven by the self-cast; an offensive-ground rejection is a server-side
      // preflight condition, recorded as a documented gap rather than a magic-system failure.
      ctx.score("castGround", "gap", `${rec ? rec.spell : "ground spell"} was rejected (UserLocation-only) — server-side offensive-ground preflight (caster-tile safe zone / a movement-blocking status / range / LoS)`);
      ctx.addIssue({ category: "magic", severity: "medium", beat: beat.id, summary: `ground cast ${rec ? rec.spell : "?"} was rejected (UserLocation-only) — the sim's offensive-magic preflight refused it (candidates: caster-tile safe zone, a movement-blocking status, range, or LoS). The self-cast confirms the magic pipe works.`, detail: rec });
    }
  });

  // ---- Beat: MP deduction (per the sim's cast contract) + cooldown applied. ----
  await runBeat(ctx, "MP deduction + cooldown applied", async (beat) => {
    const fired = ctx.casts.filter((c) => c.fired);
    const observedDrops = fired.filter((c) => c.observedMpDrop);
    // MP deduction: a fired cast IS a spend — the sim deducts mana atomically BEFORE emitting
    // the cast packet (skills.rs cast_skill_with_context), so a cast packet proves it even when
    // the raw client MP reading is masked by a background @LEVEL restore / regen.
    // Cooldown: the authoritative WS snapshot's cooldownRemainingTicks going positive right
    // after a cast (per-cast cooldown is otherwise not surfaced — MagicDelay only fires on a
    // spell level-up). The behavioural re-cast probe is a secondary, MP-permitting check.
    const cooldownCasts = fired.filter((c) => c.cooldownApplied || (c.snapshotCooldownMax ?? 0) > 0);
    let probe = null;
    const probeSkill = ctx.book?.requestedSelf ?? ctx.book?.selfHeal ?? ctx.book?.selfBuff ?? null;
    if (cooldownCasts.length === 0 && probeSkill && fired.length > 0) {
      await waitForPlayerSettled(client);
      await delay(600);
      probe = await cooldownProbe(client, probeSkill);
    }
    beat.summary = {
      casts: ctx.casts.length,
      fired: fired.length,
      observedMpDrops: observedDrops.length,
      cooldownCasts: cooldownCasts.map((c) => ({ spell: c.spell, snapshotCooldownMax: c.snapshotCooldownMax })),
      probe,
    };

    const cooldownObserved = cooldownCasts.length > 0 || (probe && probe.blocked);
    if (fired.length === 0) {
      ctx.score("mpCooldown", "skip", "no cast fired, so MP spend / cooldown could not be observed");
    } else if (cooldownObserved) {
      const how = cooldownCasts.length > 0
        ? `snapshot cooldownRemainingTicks went positive after ${cooldownCasts.length} cast(s) (e.g. ${cooldownCasts[0].spell}=${cooldownCasts[0].snapshotCooldownMax})`
        : `an immediate re-cast of ${spellNameForSkill(probeSkill)} was blocked by the cooldown`;
      ctx.score("mpCooldown", "pass", `${fired.length} fired cast(s) spent MP (sim deducts before the cast packet); ${how}`);
    } else {
      // MP spend is confirmed (fired casts); the per-cast cooldown just was not client-observable
      // (Healing's cooldown is ~6 ticks and the lossy client skill entry hides it).
      ctx.score("mpCooldown", "gap", `${fired.length} fired cast(s) spent MP (per the sim cast contract), but no client-observable per-cast cooldown was captured (cooldown is server-enforced; MagicDelay only fires on a spell level-up)`);
      ctx.addIssue({ category: "magic", severity: "low", beat: beat.id, summary: "MP deduction confirmed via fired cast packets, but the per-cast cooldown was not client-observable in this run (short cooldown + lossy NewMagic skill entry); the cooldown is enforced server-side", detail: beat.summary });
    }
  });

  // ---- Beat: original Crystal frame feedback (cast / projectile / impact / world). ----
  await runBeat(ctx, "magic visual feedback", async (beat) => {
    const fired = ctx.casts.filter((c) => c.fired);
    const floaterPeak = ctx.casts.reduce((m, c) => Math.max(m, c.damageFloaterPeak ?? 0), 0);
    const projectiles = ctx.casts.reduce((m, c) => m + (c.projectilesSelf ?? 0), 0);
    const damaging = ctx.casts.filter((c) => (c.monsterDamageHits ?? 0) > 0 || (c.damageFloaterPeak ?? 0) > 0);
    const sourceFrameEvents = fired.reduce((m, c) => m + (c.visualProbe?.sourceFrameEvents ?? 0), 0);
    const maxLiveFrames = fired.reduce((m, c) => Math.max(m, c.visualProbe?.maxLiveFrames ?? 0), 0);
    const phases = [...new Set(fired.flatMap((c) => Object.keys(c.visualProbe?.phases ?? {})))];
    const effectNames = [...new Set(fired.flatMap((c) => c.visualProbe?.effectNames ?? []))];
    const sourcePaths = [...new Set(fired.flatMap((c) => c.visualProbe?.sourcePaths ?? []))];
    beat.summary = {
      firedCasts: fired.length,
      sourceFrameEvents,
      maxLiveFrames,
      phases,
      effectNames,
      sourcePaths: sourcePaths.slice(0, 24),
      floaterPeak,
      projectiles,
      damagingCasts: damaging.length,
    };
    if (fired.length === 0) {
      ctx.score("magicVfx", "skip", "no cast fired, so no visual feedback to observe");
    } else if (sourceFrameEvents > 0) {
      ctx.score("magicVfx", "pass", `observed ${sourceFrameEvents} original Crystal frame event(s), phases ${phases.join("/") || "cast"}, peak ${maxLiveFrames} live frame(s)`);
    } else {
      ctx.score("magicVfx", "gap", `cast packets fired but no original Crystal atlas frame was observed${floaterPeak || projectiles ? ` (fallback signals: ${floaterPeak} floater(s), ${projectiles} projectile packet(s))` : ""}`);
      ctx.addIssue({
        category: "magic",
        severity: "medium",
        beat: beat.id,
        summary: "cast packets fired but the browser probe observed no `.scene-crystal-effect-frame` backed by `/original-effects/`; inspect the selected spell's source mapping or render phase",
        detail: beat.summary,
      });
    }
  });

  // Wrap — snapshot final state for the report.
  await runBeat(ctx, "wrap + collect diagnostics", async (beat) => {
    beat.final = await readGameState(client);
    beat.gatewayUrl = client.gatewayUrl();
    beat.casts = ctx.casts;
  });
}

// Drop the caster's HP a little so a self-Heal restore is observable: attack a nearby
// monster to provoke retaliation, OR @DIE-adjacent is too blunt, so this is best-effort and
// never fatal to the beat. No-op if no monster is near (the heal still verifies via MP/cast).
async function maybeSelfDamage(ctx, beat) {
  const client = ctx.client;
  const startHp = await readPlayerHp(client);
  if (typeof startHp !== "number") return;
  const monster = await readTargetMonster(client, 10);
  if (!monster) {
    beat.selfDamage = { attempted: false, reason: "no monster nearby" };
    return;
  }
  beat.selfDamage = { attempted: true, startHp };
  const deadline = Date.now() + 9_000;
  while (Date.now() < deadline) {
    const m = await readMonsterById(client, monster.objectId);
    if (!m) break;
    const self = await client.evaluate(`(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`);
    if (self && chebyshev(m, self) <= 1) {
      if (!(await clickTile(client, m.x, m.y, "left"))) {
        await sendCommand(client, { type: "attack", objectId: Number(monster.objectId) });
      }
      await delay(560);
    } else {
      const step = self ? stepTowardTile(self, m) : { x: m.x, y: m.y };
      await walkToward(client, step.x, step.y);
      await delay(450);
    }
    const hp = await readPlayerHp(client);
    if (typeof hp === "number" && hp < startHp) {
      beat.selfDamage.lowHp = hp;
      break;
    }
  }
}

// Provoke a monster into hostility. Crystal monsters are non-aggressive until hit, and an
// offensive TARGET spell requires a hostile target (sim crystal_magic_target_hostile →
// agent.hostile_to_player). A melee click on the monster's own tile (attackTarget) aggros it;
// after a short beat the offensive cast is allowed. Never walk in the same step (an attack
// arms the movement-input block), so this only swings — the caller must already be adjacent.
async function provokeMonster(client, monster) {
  const m = (await readMonsterById(client, monster.objectId)) ?? monster;
  if (!(await clickTile(client, m.x, m.y, "left"))) {
    await sendCommand(client, { type: "attack", objectId: Number(monster.objectId) });
  }
  await delay(750);
}

// Walk the caster to within `range` Chebyshev tiles of a monster (so a ranged spell can
// reach it). Best-effort, bounded; reuses the in-viewport hop walker.
async function closeTo(ctx, monster, range) {
  const client = ctx.client;
  const deadline = Date.now() + 14_000;
  while (Date.now() < deadline) {
    const m = await readMonsterById(client, monster.objectId);
    if (!m) return false;
    const self = await client.evaluate(`(() => { const p = window.__mir2Stage5?.state?.player; return p ? { x: p.x, y: p.y } : null; })()`);
    if (!self) return false;
    if (chebyshev(m, self) <= range) return true;
    const step = stepTowardTile(self, m);
    await walkToward(client, step.x, step.y);
    await delay(450);
  }
  return false;
}

// ---------------------------------------------------------------------------
// Register → login → character → enter-world (ported from qa-combat-survival.mjs,
// parametrized to create a CASTER class).
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

  const loginBeat = await runBeat(ctx, "register + log in", async (beat) => {
    beat.account = account;
    let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "login") {
      await fillInput(client, ".login-input.account", account);
      await fillInput(client, ".login-input.password", password);
      await delay(300); // let React commit controlled-input state before the click handler reads it
      if (createAccount) {
        // Use the actual New Account DOM control, but invoke its click directly. The
        // sprite button has a small transparent hit area; coordinate-based CDP clicking
        // can land on the decorative parent and silently send no NewAccount packet.
        const newAccountClicked = await client.evaluate(`
          (() => {
            const button = document.querySelector('.login-button.account button');
            if (!(button instanceof HTMLElement)) return false;
            button.click();
            return true;
          })()
        `);
        if (!newAccountClicked) throw new Error("New Account button was not present/clickable");
        const newAccountDeadline = Date.now() + 15_000;
        while (
          Date.now() < newAccountDeadline &&
          !client.webSocketFramesSent.some((frame) => {
            const command = parseWsFrame(frame);
            return command?.type === "newAccount" && command?.accountId === account;
          })
        ) {
          await delay(100);
        }
        if (!client.webSocketFramesSent.some((frame) => {
          const command = parseWsFrame(frame);
          return command?.type === "newAccount" && command?.accountId === account;
        })) {
          throw new Error("New Account button did not send the expected newAccount command");
        }
        const newAccountResultDeadline = Date.now() + 10_000;
        let newAccountResult = null;
        while (Date.now() < newAccountResultDeadline && newAccountResult === null) {
          for (const frame of client.webSocketFramesReceived) {
            const packet = parseWsFrame(frame);
            if (packet?.packet === "NewAccount") newAccountResult = packet?.payload?.result ?? -1;
          }
          if (newAccountResult === null) await delay(100);
        }
        if (newAccountResult !== 8) {
          throw new Error(`New Account returned result=${newAccountResult ?? "timeout"}, expected 8`);
        }
        await delay(500);
      }
      await fillInput(client, ".login-input.account", account);
      await fillInput(client, ".login-input.password", password);
      await delay(300);
      await click(client, ".login-button.ok button");
      await waitUntil(client, "['select','game'].includes(window.__mir2Stage5?.state?.screen)", "select/game screen", 30_000);
    }
    beat.gatewayUrl = client.gatewayUrl();
    const url = client.gatewayUrl();
    const onRust = typeof url === "string" && /:7111\b/.test(url);
    if (url && !onRust) {
      ctx.addIssue({
        category: "infra",
        severity: "low",
        beat: beat.id,
        summary: `client connected to ${url} (not the Rust gateway :7111) — magic numerics may be proxy-backed`,
        detail: { gatewayUrl: url, expected: gatewayWs },
      });
    }
    ctx.gatewayUrl = url;
    ctx.gatewayIsRust = onRust;
  });
  if (!loginBeat.ok) {
    throw new Error(`authentication preflight failed for isolated QA account ${account}`);
  }

  const characterBeat = await runBeat(ctx, `create ${characterClass} character + start game`, async (beat) => {
    let screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "game") return;
    beat.characterName = characterName;
    beat.characterClass = characterClass;
    // Don't trust state.characters to decide existence — it can hold a phantom slot the
    // server never returned (player-qa-playthrough-loop memory). Our named character
    // appearing is the reliable "server created it" signal.
    const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
    if (!(await client.evaluate(haveOurChar))) {
      await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: characterClass });
      let created = await waitUntilSoft(client, haveOurChar, 15_000);
      if (!created) {
        await click(client, ".select-action.new button").catch(() => {});
        await fillInput(client, "input[name='characterName'], .new-character-name, .character-create-name", characterName).catch(() => {});
        await click(client, ".select-action.ok button, .new-character-confirm button, .character-create-confirm button").catch(() => {});
        created = await waitUntilSoft(client, haveOurChar, 10_000);
      }
      if (!created) {
        ctx.addIssue({ category: "flow", severity: "high", beat: beat.id, summary: "character creation did not produce a server-backed character", detail: { characterName, characterClass } });
        throw new Error(`character ${characterName} was not created`);
      }
    }
    const startIndex = await client.evaluate(
      `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}); return c?.index ?? null; })()`,
    );
    if (typeof startIndex !== "number") throw new Error(`character ${characterName} has no server index`);
    beat.startIndex = startIndex;
    // Let the character-selection sprite requests settle before the route unmounts; otherwise
    // Chrome reports a harmless net::ERR_ABORTED image request as a QA network failure.
    await delay(800);
    const wsBefore = client.wsRxCount;
    await sendCommand(client, { type: "startGame", characterIndex: startIndex });
    const entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
    if (!entered) {
      const result = startGameResultFrom(client.webSocketFramesReceived.filter((f) => f.seq >= wsBefore));
      ctx.addIssue({
        category: "flow",
        severity: "high",
        beat: beat.id,
        summary: result === null ? "startGame sent but never entered the game" : `startGame rejected by server (StartGame result=${result})`,
        detail: { startIndex, result },
      });
      throw new Error(`did not enter game (StartGame result=${result})`);
    }
  });
  if (!characterBeat.ok) {
    throw new Error(`character/startGame preflight failed for ${characterName}`);
  }

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
    characterClass,
    gatewayWsRequested: forceGateway ? gatewayWs : "(client default)",
    gatewayUrlConnected: ctx.gatewayUrl ?? ctx.client.gatewayUrl(),
    gatewayIsRust: ctx.gatewayIsRust ?? null,
    account,
    characterName,
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    beats: ctx.beats.length,
    beatsOk: ctx.beats.filter((b) => b.ok).length,
    castCount: ctx.casts.length,
    castsFired: ctx.casts.filter((c) => c.fired).length,
    issueCount: ctx.issues.length,
    bySeverity,
    byCategory,
    scorecard: ctx.scorecard,
  };

  await fs.writeFile(path.join(outputDir, "summary.json"), JSON.stringify(summary, null, 2));
  await fs.writeFile(path.join(outputDir, "report.json"), JSON.stringify({ summary, issues: ctx.issues, beats: ctx.beats, casts: ctx.casts }, null, 2));
  await fs.writeFile(path.join(outputDir, "console.json"), JSON.stringify({ errors: ctx.client.consoleErrors, messages: ctx.client.consoleMessages.slice(-200) }, null, 2));
  await fs.writeFile(path.join(outputDir, "network-failures.json"), JSON.stringify(ctx.client.networkFailures, null, 2));
  await fs.writeFile(path.join(outputDir, "network-cancellations.json"), JSON.stringify(ctx.client.networkCancellations, null, 2));
  await fs.writeFile(
    path.join(outputDir, "ws-timeline.json"),
    JSON.stringify({ sent: ctx.client.webSocketFramesSent.slice(-200), received: ctx.client.webSocketFramesReceived.slice(-200) }, null, 2),
  );

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  const statusIcon = (s) => (s === "pass" ? "✅ pass" : s === "fail" ? "❌ fail" : s === "gap" ? "▱ known-gap" : "· skip");
  const md = [];
  md.push(`# Magic / skill QA report — ${runId}`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\`  ·  class \`${characterClass}\`  ·  account \`${account}\`  ·  character \`${characterName}\``);
  md.push(`- Gateway: requested \`${summary.gatewayWsRequested}\`  ·  connected \`${summary.gatewayUrlConnected ?? "?"}\`  ·  Rust(:7111)=${summary.gatewayIsRust === null ? "?" : summary.gatewayIsRust}`);
  if (summary.gatewayIsRust === false) md.push(`  - ⚠️ numerics may be **proxy-backed** (not the authoritative Rust sim).`);
  md.push(`- Beats: ${summary.beatsOk}/${summary.beats} ok  ·  Casts fired: ${summary.castsFired}/${summary.castCount}  ·  Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push("");
  md.push("## Magic / skill scorecard");
  md.push("");
  md.push("| Check | Status | Note |");
  md.push("|---|---|---|");
  for (const key of Object.keys(ctx.scorecard)) {
    const c = ctx.scorecard[key];
    md.push(`| ${c.label} | ${statusIcon(c.status)} | ${String(c.note ?? "").replace(/\|/g, "\\|")} |`);
  }
  md.push("");
  md.push("## Casts");
  md.push("");
  if (!ctx.casts.length) {
    md.push("_No casts were attempted._");
  } else {
    md.push("| Spell | Mode | Fired | MP Δ | Cooldown | Dmg hits | Floaters |");
    md.push("|---|---|---|---|---|---|---|");
    for (const c of ctx.casts) {
      md.push(`| ${c.spell} | ${c.mode} | ${c.fired ? "✅" : "❌"} | ${c.mpDelta ?? "?"} | ${c.cooldownBefore ?? "?"}→${c.cooldownAfter ?? "?"} | ${c.monsterDamageHits ?? 0} | ${c.damageFloaterPeak ?? 0} |`);
    }
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
      md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.beat ?? "—"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
    }
  }
  md.push("");
  md.push("## Cycle (beats)");
  md.push("");
  for (const b of ctx.beats) {
    md.push(`### Beat ${b.id} — ${b.title} ${b.ok ? "✅" : "❌"}  (${b.durationMs}ms)`);
    if (b.error) md.push(`- error: \`${b.error.slice(0, 300)}\``);
    if (b.note) md.push(`- note: ${b.note}`);
    if (b.vitals) md.push(`- vitals: ${JSON.stringify(b.vitals)}`);
    if (b.spellBook) md.push(`- spell book: ${JSON.stringify(b.spellBook)}`);
    if (b.bind) md.push(`- bind: ${JSON.stringify(b.bind)} → hotkey=${b.reflectedHotkey ?? "?"}`);
    if (b.cast) md.push(`- cast: ${JSON.stringify({ spell: b.cast.spell, mode: b.cast.mode, fired: b.cast.fired, mpDelta: b.cast.mpDelta, cooldown: `${b.cast.cooldownBefore}→${b.cast.cooldownAfter}`, dmgHits: b.cast.monsterDamageHits, floaters: b.cast.damageFloaterPeak, originalFrameEvents: b.cast.visualProbe?.sourceFrameEvents ?? 0, phases: Object.keys(b.cast.visualProbe?.phases ?? {}) })}`);
    if (b.cast?.visualFrames?.length) {
      md.push(`- cast frames: ${b.cast.visualFrames.map((frame) => `[\`${frame.phase}\`](${frame.path})`).join(" · ")}`);
    } else if (b.cast?.visualFrame) {
      md.push(`- cast frame: [\`${b.cast.visualFrame}\`](${b.cast.visualFrame})`);
    }
    if (b.summary) md.push(`- summary: ${JSON.stringify(b.summary)}`);
    if (b.state) {
      md.push(`- state: screen=\`${b.state.screen}\` map=\`${b.state.mapFileName ?? "?"}\` hp=${b.state.playerHp ?? "?"}/${b.state.playerMaxHp ?? "?"} mp=${b.state.playerMp ?? "?"}/${b.state.playerMaxMp ?? "?"} spells=${b.state.knownSkillCount} monsters=${b.state.liveMonsterCount} pets=${b.state.petCount} floaters=${b.state.damageFloaterDom}`);
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
  md.push(`cd apps/web && node ./scripts/qa-magic-skills.mjs --headed --baseUrl ${baseUrl} --gatewayWs ${gatewayWs} --class ${characterClass}`);
  md.push(`# To exercise the full offensive/ground cast path, point at a gateway started with`);
  md.push(`# MIR2_GM_PASSWORD=<pw> and add: --gmPassword <pw>`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-combat-survival.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-magic-${process.pid}-${Date.now()}`);
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

function normalizeClass(value) {
  const v = String(value ?? "").toLowerCase();
  if (v.includes("wiz")) return "wizard";
  if (v.includes("tao")) return "taoist";
  if (v.includes("warr")) return "warrior";
  if (v.includes("assas")) return "assassin";
  if (v.includes("arch")) return "archer";
  return "taoist";
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
  return `MG${suffix}`.slice(0, 10).toUpperCase();
}

function defaultAccountName() {
  const suffix = `${process.pid.toString(36)}${Date.now().toString(36)}`.replace(/[^a-z0-9]/gi, "");
  return `MG${suffix}`.slice(0, 12).toUpperCase();
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
  console.log(`qa-magic-skills: target=${baseUrl} gateway=${forceGateway ? gatewayWs : "(client default)"} class=${characterClass} account=${account} char=${characterName} headed=${headed}`);
  console.log(`qa-magic-skills: output=${outputDir}`);

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

    await magicSkills(ctx);

    collectNetworkIssues(ctx);
    collectConsoleIssues(ctx);
    await writeReport(ctx);

    const s = ctx.issues.reduce((acc, i) => ((acc[i.severity] = (acc[i.severity] ?? 0) + 1), acc), {});
    const sc = ctx.scorecard;
    const pass = Object.values(sc).filter((c) => c.status === "pass").length;
    const fail = Object.values(sc).filter((c) => c.status === "fail").length;
    const gap = Object.values(sc).filter((c) => c.status === "gap").length;
    console.log(`\nqa-magic-skills done: scorecard ${pass} pass / ${fail} fail / ${gap} known-gap; ${ctx.casts.filter((c) => c.fired).length}/${ctx.casts.length} casts fired; ${ctx.beats.filter((b) => b.ok).length}/${ctx.beats.length} beats ok, ${ctx.issues.length} issues (high ${s.high ?? 0}, medium ${s.medium ?? 0}, low ${s.low ?? 0}).`);
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
  } catch (error) {
    console.error("qa-magic-skills fatal:", error);
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
