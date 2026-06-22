// qa-equip-appearance.mjs — AI-driven QA loop for the EQUIP→AVATAR-APPEARANCE wiring.
//
// Verifies the fix for "装备带上了外观怎么没渲染出来" (equipped gear did not change the
// in-world avatar). Root cause: when an item was equipped from the bag,
// `equipment_state_from_item_state` (apps/simulation/src/runtime/equipment.rs) only
// assigned a render `shape` for six hard-coded legacy item names; every other item got
// `shape = None`, so the self player's `body_library` stayed `CArmour/00` and no
// `CWeapon` layer was drawn. The fix reads the authoritative Crystal template
// `ItemInfo.Shape` (`crystal_item_template_for_item_key(..).shape`) for any worn item.
//
// The loop drives the REAL client end-to-end (page.tsx handler -> gateway BrowserCommand
// -> ClientPacket -> simulation -> ServerPacket/worldSnapshot -> state), the same way the
// house qa-*.mjs harnesses do, with zero new deps:
//
//   1. login + enter world          — fresh MALE WARRIOR (so the seeded gear's class/gender reqs pass)
//   2. baseline sprite              — record the self entity's bodyLibrary/weaponLibrary
//   3. seed real Crystal gear       — qa.giveItem (NON-gated stage5 hook) crystal-item-317
//                                      (BaseDress(M), ItemInfo.Shape 1 -> CArmour/01) and
//                                      crystal-item-221 (WoodenSword, Shape 0 -> CWeapon/00).
//                                      Their seeded display name is "Crystal Item NNN", which
//                                      is NOT in the legacy six-name table, so this beat FAILS
//                                      on the pre-fix server and only passes with the fix.
//   4. equip the gear               — equipItem BrowserCommand (the inventory window's onEquipItem)
//   5. VERIFY appearance            — assert the self entity sprite is now CArmour/01 + CWeapon/00,
//                                      corroborated by the gateway worldSnapshot WS frame
//
// VERDICT: PASS only when, after equipping, body_library == "CArmour/01" and
// weapon_library == "CWeapon/00". Exit code is non-zero on FAIL so it gates in CI/scripts.
//
// Why a real browser (not a protocol bot): the whole point is the cross-layer wiring that
// turns an equip into a rendered avatar. Chosen shapes 0/1 map to libraries that exist
// locally (CArmour/00,01 · CWeapon/00,01) so the frames also actually paint.
//
// Usage:
//   node ./scripts/qa-equip-appearance.mjs [--headed] [--baseUrl http://127.0.0.1:3023]
//        [--gatewayWs ws://127.0.0.1:7110/ws] [--runId my-run]
//
// Environment: a backend (gateway+simulation) built WITH the fix must be reachable. On a
// localhost origin the client uses ws://127.0.0.1:7110/ws; pass --gatewayWs to override
// (honoured only on a localhost origin, page.tsx resolveGatewayWebSocketUrl).

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
const characterName = args.characterName ?? defaultCharacterName();
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const disableGpu = booleanArg(args.disableGpu ?? process.env.MIR2_CHROME_DISABLE_GPU, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9700 + (process.pid % 280));
const runId = args.runId ?? `${new Date().toISOString().replace(/[:.]/g, "-").replace("Z", "")}-${process.pid}`;
const outputDir = path.resolve(
  args.output ?? path.join(process.cwd(), "docs", "generated", "equip-appearance-qa", `appearance-${runId}`),
);
const framesDir = path.join(outputDir, "frames");

const VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const SCENE_READY_TIMEOUT_MS = numberArg(args.sceneReadyTimeoutMs, 45_000);
const MUTATION_TIMEOUT_MS = numberArg(args.mutationTimeoutMs, 8_000);

// The gear the loop seeds + equips and the exact appearance each must produce. Both items
// are req-level 1 so a fresh character can wear them; both shapes map to libraries that
// exist locally so the avatar actually paints. The display name a qa.giveItem key yields is
// "Crystal Item NNN" (stage5_item_name), which is NOT one of the six legacy hard-coded names
// — so before the fix the body stays CArmour/00 and weaponLibrary is null.
const GEAR = [
  {
    key: "crystal-item-317", // BaseDress(M) — Crystal ItemInfo.Shape 1
    label: "BaseDress(M)",
    slot: "armour",
    slotIndex: 1,
    spriteField: "bodyLibrary",
    expect: "CArmour/01",
    preFix: "CArmour/00", // what the buggy server rendered instead
  },
  {
    key: "crystal-item-221", // WoodenSword — Crystal ItemInfo.Shape 0
    label: "WoodenSword",
    slot: "weapon",
    slotIndex: 0,
    spriteField: "weaponLibrary",
    expect: "CWeapon/00",
    preFix: null, // the buggy server drew no weapon layer at all
  },
];

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

let targetAlreadyNavigated = false;

// ---------------------------------------------------------------------------
// CDP client (adapted from qa-items.mjs) — console, network failures, WS frames.
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
    } else if (m === "Network.responseReceived") {
      const r = p.response;
      const url = String(r?.url ?? "");
      if (r && r.status >= 400 && !url.includes("favicon")) {
        this.networkFailures.push({ url, status: r.status, kind: classifyAssetUrl(url), at: Date.now() });
      }
    } else if (m === "Network.requestWillBeSent") {
      if (p.requestId && p.request?.url) this.requestUrlById.set(p.requestId, p.request.url);
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
// Run context + beats.
// ---------------------------------------------------------------------------
function createContext(client) {
  const issues = [];
  const beats = [];
  return {
    client,
    issues,
    beats,
    seq: 0,
    verdict: null,
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
    ctx.addIssue({ category: "flow", severity: "high", beat: id, summary: `beat blocked: ${title}`, detail: beat.error });
    ctx.log(`  ✖ ${beat.error.slice(0, 300)}`);
  }
  const frameName = `${String(id).padStart(2, "0")}-${slug(title)}.png`;
  try {
    await captureFrame(ctx.client, path.join(framesDir, frameName));
    beat.frame = `frames/${frameName}`;
  } catch {
    /* screenshot is best-effort */
  }
  beat.endedAt = Date.now();
  beat.durationMs = beat.endedAt - beat.startedAt;
  ctx.beats.push(beat);
  return beat;
}

async function captureFrame(client, filePath) {
  const shot = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
  await fs.writeFile(filePath, Buffer.from(shot.data, "base64"));
}

// ---------------------------------------------------------------------------
// Truth readers — the self entity sprite from the in-page state (what the renderer
// uses) and, as corroboration, from the gateway worldSnapshot WS frame.
// ---------------------------------------------------------------------------
async function readSelfSprite(client) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const ents = Array.isArray(s.entities) ? s.entities : [];
      const self = ents.find((e) => e && e.kind === "selfPlayer")
        ?? ents.find((e) => e && String(e.objectId) === String(s.playerObjectId));
      if (!self) return null;
      const sp = self.sprite ?? null;
      return {
        objectId: String(self.objectId ?? ""),
        bodyLibrary: sp?.bodyLibrary ?? null,
        weaponLibrary: sp?.weaponLibrary ?? null,
        hairLibrary: sp?.hairLibrary ?? null,
        frameBaseOffset: sp?.frameBaseOffset ?? null,
      };
    })()
  `);
}

// Most recent self-player sprite seen in a gateway worldSnapshot frame since `sinceTime`.
// Best-effort corroboration: returns null if the snapshot is not pushed as a typed frame.
function selfSpriteFromFrames(client, sinceTime) {
  for (let i = client.webSocketFramesReceived.length - 1; i >= 0; i -= 1) {
    const f = client.webSocketFramesReceived[i];
    if (typeof sinceTime === "number" && f.at < sinceTime) continue;
    let o;
    try {
      o = JSON.parse(f.payloadData);
    } catch {
      continue;
    }
    const entities = o?.payload?.entities;
    if (o?.type === "worldSnapshot" && Array.isArray(entities)) {
      const self = entities.find((e) => e && e.kind === "selfPlayer");
      if (self?.sprite) {
        return { bodyLibrary: self.sprite.bodyLibrary ?? null, weaponLibrary: self.sprite.weaponLibrary ?? null };
      }
    }
  }
  return null;
}

async function readInventory(client) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const inv = Array.isArray(s.inventoryItems) ? s.inventoryItems : [];
      const equip = Array.isArray(s.equipmentItems) ? s.equipmentItems : [];
      return {
        screen: s.screen ?? null,
        playerObjectId: s.playerObjectId ?? null,
        inventory: inv.map((it) => ({ key: it.key ?? null, name: it.name ?? null, uniqueId: it.uniqueId ?? null, slot: it.slot ?? null, container: it.container ?? null })),
        equipment: equip.map((it) => ({ slot: it.slot ?? null, name: it.name ?? null })),
      };
    })()
  `);
}

// ---------------------------------------------------------------------------
// Command primitives.
// ---------------------------------------------------------------------------
async function sendCommand(client, command) {
  return client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
}

async function runStage5Command(client, action, commandArgs = []) {
  return sendCommand(client, { type: "stage5Command", action, args: commandArgs.map(String) });
}

// ---------------------------------------------------------------------------
// Login + enter-world (ported from qa-items.mjs) — creates a MALE WARRIOR so the
// seeded gear's required class/gender pass.
// ---------------------------------------------------------------------------
async function loginAndEnterWorld(ctx) {
  const client = ctx.client;

  await runBeat(ctx, "open client + login screen", async () => {
    await navigate(client, RUN_URL);
    await waitUntil(client, "['login', 'select', 'game'].includes(window.__mir2Stage5?.state?.screen)", "client stage ready", 30_000);
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    if (screen === "login") await waitForSelectorSoft(client, ".login-overlay", 8_000);
  });

  await runBeat(ctx, "register + log in to character select", async (beat) => {
    beat.account = account;
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
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

  await runBeat(ctx, "create male warrior + enter world", async (beat) => {
    const screen = await client.evaluate("window.__mir2Stage5?.state?.screen ?? null");
    beat.characterName = characterName;
    if (screen !== "game") {
      const haveOurChar = `(() => { const cs = window.__mir2Stage5?.state?.characters; return Array.isArray(cs) && cs.some((c) => c?.name === ${JSON.stringify(characterName)}); })()`;
      if (!(await client.evaluate(haveOurChar))) {
        await sendCommand(client, { type: "newCharacter", name: characterName, gender: "male", class: "warrior" });
        const created = await waitUntilSoft(client, haveOurChar, 15_000);
        if (!created) {
          ctx.addIssue({ category: "flow", severity: "high", beat: beat.id, summary: "character creation not reflected", detail: { characterName } });
        }
      }
      const startIndex = await client.evaluate(
        `(() => { const cs = window.__mir2Stage5?.state?.characters ?? []; const c = cs.find((e) => e?.name === ${JSON.stringify(characterName)}) ?? cs[0]; return c?.index ?? 0; })()`,
      );
      beat.startIndex = startIndex;
      await sendCommand(client, { type: "startGame", characterIndex: startIndex });
      // Cold-start (first compile + fresh gateway world load) can exceed a short wait;
      // give it room and retry the StartGame once before declaring failure.
      let entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 40_000);
      if (!entered) {
        await sendCommand(client, { type: "startGame", characterIndex: startIndex });
        entered = await waitUntilSoft(client, "window.__mir2Stage5?.state?.screen === 'game'", 25_000);
      }
      if (!entered) {
        const result = startGameResultFrom(client.webSocketFramesReceived.slice(-40));
        ctx.addIssue({
          category: "flow",
          severity: "high",
          beat: beat.id,
          summary: result === null ? "startGame sent but never entered the game" : `startGame rejected (StartGame result=${result})`,
          detail: { startIndex, result },
        });
        throw new Error(`did not enter game (StartGame result=${result})`);
      }
    }
    const ready = await waitUntilSoft(
      client,
      "window.__mir2Stage5?.state?.screen === 'game' && (window.__mir2Stage5?.state?.sceneInteractionReady === true || window.__mir2Stage5?.state?.sceneAssetReadiness?.ready === true)",
      SCENE_READY_TIMEOUT_MS,
    );
    if (!ready) {
      ctx.addIssue({ category: "render", severity: "medium", beat: beat.id, summary: "scene never became interaction/asset ready", detail: {} });
    }
    await delay(1000);
  });
}

// ---------------------------------------------------------------------------
// The appearance verification journey.
// ---------------------------------------------------------------------------
async function appearanceJourney(ctx) {
  const client = ctx.client;
  const results = [];

  // Beat — baseline: record the avatar sprite BEFORE equipping anything.
  await runBeat(ctx, "baseline self sprite", async (beat) => {
    // The self entity arrives a beat after entering the world (first worldSnapshot);
    // wait for it so a slow cold entry does not read an empty state.
    await waitUntilSoft(
      client,
      "(window.__mir2Stage5?.state?.entities ?? []).some((e) => e && e.kind === 'selfPlayer')",
      20_000,
    );
    const baseline = await readSelfSprite(client);
    beat.baseline = baseline;
    if (!baseline) {
      ctx.addIssue({
        category: "render",
        severity: "high",
        beat: beat.id,
        summary: "self player entity / sprite not present in client state — cannot read avatar appearance",
        detail: await readInventory(client),
      });
    }
  });

  // Beat — seed the real Crystal gear through the non-gated qa.giveItem hook.
  await runBeat(ctx, "seed real Crystal gear (qa.giveItem)", async (beat) => {
    const seeded = [];
    for (const gear of GEAR) {
      const invBefore = (await readInventory(client)).inventory.length;
      await runStage5Command(client, "qa.giveItem", [gear.key, "1"]);
      const arrived = await waitUntilSoft(
        client,
        `(() => { const inv = window.__mir2Stage5?.state?.inventoryItems ?? []; return inv.some((it) => it && it.key === ${JSON.stringify(gear.key)}); })()`,
        MUTATION_TIMEOUT_MS,
      );
      seeded.push({ key: gear.key, label: gear.label, arrived, invBefore });
    }
    beat.seeded = seeded;
    const missing = seeded.filter((s) => !s.arrived);
    if (missing.length) {
      ctx.addIssue({
        category: "flow",
        severity: "high",
        beat: beat.id,
        summary: `qa.giveItem did not seed ${missing.map((m) => m.key).join(", ")} (hook unavailable on this stack?)`,
        detail: { seeded, inventory: (await readInventory(client)).inventory },
      });
    }
  });

  // Beat — equip each piece, then VERIFY the avatar sprite changed to the shape-derived
  // library. This is the acceptance gate.
  await runBeat(ctx, "equip gear + verify avatar appearance", async (beat) => {
    const before = await readSelfSprite(client);
    beat.spriteBefore = before;

    for (const gear of GEAR) {
      const inv = await readInventory(client);
      const item = inv.inventory.find((it) => it.key === gear.key);
      if (!item || item.uniqueId == null) {
        ctx.addIssue({ category: "item", severity: "high", beat: beat.id, summary: `seeded ${gear.label} not in bag to equip`, detail: { gear: gear.key, inventory: inv.inventory } });
        continue;
      }
      await sendCommand(client, { type: "equipItem", uniqueId: item.uniqueId, grid: "inventory", to: gear.slotIndex });
      await waitUntilSoft(
        client,
        `(() => {
          const s = window.__mir2Stage5?.state ?? {};
          const stillBag = (s.inventoryItems ?? []).some((it) => it && it.uniqueId === ${Number(item.uniqueId)});
          const slotFilled = (s.equipmentItems ?? []).some((e) => e && e.slot === ${JSON.stringify(gear.slot)});
          return !stillBag || slotFilled;
        })()`,
        MUTATION_TIMEOUT_MS,
      );
    }

    // Give the snapshot a tick to rebuild the self sprite from the new equipment.
    const wsBefore = Date.now();
    await delay(1200);
    const after = await readSelfSprite(client);
    const fromFrame = selfSpriteFromFrames(client, wsBefore);
    beat.spriteAfter = after;
    beat.spriteFromWorldSnapshotFrame = fromFrame;
    beat.equipment = (await readInventory(client)).equipment;

    for (const gear of GEAR) {
      const got = after?.[gear.spriteField] ?? null;
      const frameGot = fromFrame ? fromFrame[gear.spriteField] ?? null : undefined;
      const pass = got === gear.expect;
      results.push({ ...gear, got, frameGot, pass });
      if (!pass) {
        ctx.addIssue({
          category: "appearance",
          severity: "high",
          beat: beat.id,
          summary: `${gear.slot}: avatar ${gear.spriteField} is ${JSON.stringify(got)}, expected "${gear.expect}" after equipping ${gear.label} (pre-fix server renders ${JSON.stringify(gear.preFix)})`,
          detail: { gear: gear.key, expected: gear.expect, got, fromWorldSnapshotFrame: frameGot, spriteBefore: before, spriteAfter: after },
        });
      } else {
        ctx.log(`  ✓ ${gear.slot}: ${gear.spriteField} = "${got}" (${gear.label})`);
      }
    }
    beat.results = results;
  });

  ctx.verdict = results.length === GEAR.length && results.every((r) => r.pass) ? "PASS" : "FAIL";
}

// ---------------------------------------------------------------------------
// Reporting.
// ---------------------------------------------------------------------------
async function writeReport(ctx) {
  const bySeverity = { high: 0, medium: 0, low: 0 };
  for (const issue of ctx.issues) bySeverity[issue.severity] = (bySeverity[issue.severity] ?? 0) + 1;
  const summary = {
    runId,
    baseUrl,
    gatewayWs,
    account,
    characterName,
    verdict: ctx.verdict ?? "FAIL",
    startedAt: ctx.startedAt,
    finishedAt: Date.now(),
    beats: ctx.beats.length,
    beatsOk: ctx.beats.filter((b) => b.ok).length,
    issueCount: ctx.issues.length,
    bySeverity,
  };
  await fs.writeFile(path.join(outputDir, "summary.json"), JSON.stringify(summary, null, 2));
  await fs.writeFile(path.join(outputDir, "report.json"), JSON.stringify({ summary, issues: ctx.issues, beats: ctx.beats }, null, 2));
  await fs.writeFile(
    path.join(outputDir, "console.json"),
    JSON.stringify({ errors: ctx.client.consoleErrors.slice(-100), messages: ctx.client.consoleMessages.slice(-150) }, null, 2),
  );

  const order = { high: 0, medium: 1, low: 2 };
  const sortedIssues = [...ctx.issues].sort((a, b) => (order[a.severity] ?? 9) - (order[b.severity] ?? 9));
  const md = [];
  md.push(`# Equip→appearance QA report — ${runId}`);
  md.push("");
  md.push(`**VERDICT: ${summary.verdict}**`);
  md.push("");
  md.push(`- Target: \`${baseUrl}\`${gatewayWs ? ` · gateway \`${gatewayWs}\`` : ""} · account \`${account}\` · character \`${characterName}\``);
  md.push(`- Beats: ${summary.beatsOk}/${summary.beats} ok · Issues: ${summary.issueCount} (high ${bySeverity.high}, medium ${bySeverity.medium}, low ${bySeverity.low})`);
  md.push("");
  md.push("## What this verifies");
  md.push("");
  md.push("Equipping a real Crystal item must change the in-world avatar: the self player's");
  md.push("`bodyLibrary`/`weaponLibrary` must be derived from the item's Crystal `ItemInfo.Shape`.");
  md.push("Seeded gear: " + GEAR.map((g) => `\`${g.key}\` (${g.label}) → \`${g.spriteField}\`==\`${g.expect}\``).join("; ") + ".");
  md.push("");
  md.push("## Issues");
  md.push("");
  if (!sortedIssues.length) {
    md.push("_No issues — equipping changed the avatar appearance as expected._");
  } else {
    md.push("| # | Severity | Category | Beat | Summary |");
    md.push("|---|---|---|---|---|");
    for (const i of sortedIssues) md.push(`| ${i.id} | ${i.severity} | ${i.category} | ${i.beat ?? "—"} | ${String(i.summary).replace(/\|/g, "\\|")} |`);
  }
  md.push("");
  md.push("## Beats");
  md.push("");
  for (const b of ctx.beats) {
    md.push(`### Beat ${b.id} — ${b.title} ${b.ok ? "✅" : "❌"} (${b.durationMs}ms)`);
    if (b.error) md.push(`- error: \`${b.error.slice(0, 300)}\``);
    if (b.note) md.push(`- note: ${b.note}`);
    if (b.baseline) md.push(`- baseline sprite: ${JSON.stringify(b.baseline)}`);
    if (b.seeded) md.push(`- seeded: ${JSON.stringify(b.seeded)}`);
    if (b.spriteBefore) md.push(`- sprite before: ${JSON.stringify(b.spriteBefore)}`);
    if (b.spriteAfter) md.push(`- sprite after: ${JSON.stringify(b.spriteAfter)}`);
    if (b.spriteFromWorldSnapshotFrame) md.push(`- sprite (worldSnapshot frame): ${JSON.stringify(b.spriteFromWorldSnapshotFrame)}`);
    if (b.equipment) md.push(`- equipment: ${JSON.stringify(b.equipment)}`);
    if (b.results) md.push(`- results: ${JSON.stringify(b.results)}`);
    if (b.frame) md.push(`- frame: [\`${b.frame}\`](${b.frame})`);
    md.push("");
  }
  md.push("## Reproduce");
  md.push("");
  md.push("```bash");
  md.push(`cd apps/web && node ./scripts/qa-equip-appearance.mjs --baseUrl ${baseUrl}${gatewayWs ? ` --gatewayWs ${gatewayWs}` : ""}`);
  md.push("```");
  await fs.writeFile(path.join(outputDir, "report.md"), md.join("\n"));
}

// ---------------------------------------------------------------------------
// Chrome lifecycle + DOM plumbing (from qa-items.mjs).
// ---------------------------------------------------------------------------
async function launchChrome() {
  const userDataDir = path.join(os.tmpdir(), `mir2-qa-appearance-${process.pid}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${debugPort}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      ...(disableGpu ? ["--disable-gpu"] : ["--ignore-gpu-blocklist", "--enable-webgl"]),
      "--autoplay-policy=no-user-gesture-required",
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
    (() => { const node = document.querySelector(${JSON.stringify(selector)}); if (!node) return false; node.click(); return true; })()
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
    .evaluate(`(() => ({ url: location.href, screen: window.__mir2Stage5?.state?.screen ?? null, wsState: window.__mir2Stage5?.state?.wsState ?? null }))()`)
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
  return `EQ${suffix}`.slice(0, 10).toUpperCase();
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
  console.log(`qa-equip-appearance: target=${baseUrl} account=${account} char=${characterName} headed=${headed}${gatewayWs ? ` gatewayWs=${gatewayWs}` : ""}`);
  console.log(`qa-equip-appearance: output=${outputDir}`);

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
    await appearanceJourney(ctx);
    await writeReport(ctx);

    const verdict = ctx.verdict ?? "FAIL";
    console.log(`\nqa-equip-appearance VERDICT: ${verdict} — ${ctx.beats.filter((b) => b.ok).length}/${ctx.beats.length} beats ok, ${ctx.issues.length} issue(s).`);
    console.log(`Report: ${path.join(outputDir, "report.md")}`);
    if (verdict !== "PASS") process.exitCode = 1;
  } catch (error) {
    console.error("qa-equip-appearance fatal:", error);
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
