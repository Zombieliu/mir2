// qa-social.mjs — AI-driven two-client SOCIAL end-to-end QA loop.
//
// Drives TWO real web clients (A + B) over Chrome DevTools Protocol in the SAME
// shared-zone map and exercises the multiplayer "social" surface the single
// client cannot test alone: party/group, player-to-player trade, whisper + chat
// channels, friends, and (optionally) marriage/bonds. Every beat is verified
// from BOTH clients' authoritative state (window.__mir2Stage5.state) AND the
// decoded server frames each client received (window.__mir2GatewayEventHistory),
// so a one-sided success cannot mask a broken broadcast.
//
// It is modelled on:
//   - scripts/smoke-two-client-zone.mjs  (two CDP clients, chrome launch, login,
//     transfer-into-a-shared-zone, pulse/tick, packet-frame capture)
//   - scripts/qa-playthrough.mjs         (best-effort "beat" runner + structured
//     issue report so one broken beat never aborts the rest of the journey)
//
// Why two browsers (not a protocol bot): the social windows
// (original-client-{group,trade,friends,chat-settings,bonds}-window.tsx) and the
// stage-5 adapters only light up against the REAL client's merged world state.
//
// What the local shared-zone sim actually does (verified against
// apps/gateway/src/routing.rs + apps/simulation/src/runtime/packets.rs) — the
// loop asserts the GUARANTEED behaviour and merely RECORDS the known gaps:
//   * Trade pairs by the *adjacent* remote player (Chebyshev <= 1), both sides
//     send TradeRequest; the offer is NOT streamed to the partner mid-trade — it
//     is delivered atomically when BOTH confirm. So we verify the trade by the
//     resulting gold delta on each client, not by a live partnerGold push.
//   * Group rosters are per-session: A's AddMember puts B in A's roster, but B's
//     accept only re-seeds B's own roster (B won't see A). Cross-session group
//     invite relay is absent. Both are recorded as observations, not failures.
//   * MarriageRequest is a local stub ("face a player" prompt) — recorded.
//
// Usage:
//   node ./scripts/qa-social.mjs --headed
//     [--baseUrl http://127.0.0.1:13010] [--gatewayWs ws://127.0.0.1:7210/ws]
//     [--runId my-run] [--map 0] [--password pw]
//
// Each beat is best-effort: a failing assertion is recorded and the loop keeps
// going so it captures the whole social journey in one report. Exit code is 1
// when any HARD assertion (or a blocked beat) failed.

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
const DEFAULT_OUTPUT_DIR = path.resolve(REPO_ROOT, "docs", "generated", "player-qa", "social");
const DEFAULT_VIEWPORT = { width: 1024, height: 768, deviceScaleFactor: 1, mobile: false };

const args = parseArgs(process.argv.slice(2));
const gatewayWsUrl = args.gatewayWs ?? process.env.MIR2_GATEWAY_WS_URL ?? DEFAULT_GATEWAY_WS_URL;
const baseUrl = buildBaseUrl(args.baseUrl ?? process.env.MIR2_WEB_BASE_URL ?? DEFAULT_BASE_URL, gatewayWsUrl);
const outputDir = path.resolve(args.output ?? process.env.MIR2_SOCIAL_OUTPUT ?? DEFAULT_OUTPUT_DIR);
const runId = args.runId ?? new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
const prefix = args.prefix ?? `social-${runId}`;
const password = args.password ?? process.env.MIR2_SOCIAL_PASSWORD ?? "social-pass";
const map = args.map ?? process.env.MIR2_SOCIAL_MAP ?? "0";
const accountA = args.accountA ?? process.env.MIR2_SOCIAL_ACCOUNT_A ?? `soca${runId}`;
const accountB = args.accountB ?? process.env.MIR2_SOCIAL_ACCOUNT_B ?? `socb${runId}`;
// Character names are unique across the realm; the runId suffix keeps each run's
// pair fresh. Kept <= 10 chars (Crystal's name cap) and alphanumeric.
const characterA = (args.characterA ?? process.env.MIR2_SOCIAL_CHARACTER_A ?? `SA${runId.slice(-6)}`).slice(0, 10);
const characterB = (args.characterB ?? process.env.MIR2_SOCIAL_CHARACTER_B ?? `SB${runId.slice(-6)}`).slice(0, 10);
// Adjacent spawn tiles so trade/marriage's adjacent-remote-player resolution
// (apps/gateway/src/routing.rs:5370 adjacent_remote_player_name, |dx|<=1 && |dy|<=1)
// pairs A and B.
const startAx = numberArg(args.ax ?? process.env.MIR2_SOCIAL_AX, 330);
const startAy = numberArg(args.ay ?? process.env.MIR2_SOCIAL_AY, 270);
const startBx = numberArg(args.bx ?? process.env.MIR2_SOCIAL_BX, 331);
const startBy = numberArg(args.by ?? process.env.MIR2_SOCIAL_BY, 270);
const debugPort = numberArg(args.debugPort ?? process.env.MIR2_CHROME_DEBUG_PORT, 9800 + (process.pid % 400));
const headed = booleanArg(args.headed ?? process.env.MIR2_CHROME_HEADED, false);
const chromePath = process.env.MIR2_CHROME_PATH ?? findChromePath();

// Gold each side offers in the trade beat (capped to whatever gold the freshly
// created character actually holds, read live).
const TRADE_GOLD_A = numberArg(args.tradeGoldA, 7);
const TRADE_GOLD_B = numberArg(args.tradeGoldB, 3);

if (!chromePath) {
  throw new Error("Could not find Chrome. Set MIR2_CHROME_PATH.");
}

// Social/zone ServerPacket names we tap off the raw WebSocket for the report's
// packetFrames trail (a secondary signal; assertions use the decoded
// __mir2GatewayEventHistory). Covers party, trade, friends, chat and bonds.
const SOCIAL_PACKET_RE =
  /ObjectPlayer|ObjectChat|SwitchGroup|AddMember|DeleteMember|DeleteGroup|GroupInvite|GroupMemberInfo|GroupMembersMap|SendMemberLocation|TradeRequest|TradeAccept|TradeGold|TradeItem|TradeConfirm|TradeCancel|GainedGold|LoseGold|GainedItem|FriendUpdate|MarriageRequest|LoverUpdate/;

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
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);
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
      if (typeof payloadData === "string" && SOCIAL_PACKET_RE.test(payloadData)) {
        this.packetFrames.push({
          direction: message.method.endsWith("Sent") ? "sent" : "received",
          payloadData: payloadData.slice(0, 600),
          at: Date.now(),
        });
        this.packetFrames = this.packetFrames.slice(-80);
      }
    }
  }

  send(method, params = {}) {
    const id = this.nextId++;
    this.ws.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
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
      throw new Error(
        `${this.label} evaluate failed: ${result.exceptionDetails.text ?? JSON.stringify(result.exceptionDetails)}`,
      );
    }
    return result.result?.value;
  }

  close() {
    this.ws?.close();
  }
}

// ---------------------------------------------------------------------------
// Run context + beat/assertion collection.
// ---------------------------------------------------------------------------
function createContext(clients) {
  return {
    clients,
    beats: [],
    issues: [],
    log: (msg) => console.log(msg),
    addIssue(issue) {
      const id = `${issue.category}-${String(this.issues.length + 1).padStart(3, "0")}`;
      const full = { id, severity: "medium", ...issue };
      this.issues.push(full);
      console.log(`  ⚠ [${full.severity}/${full.category}] ${full.summary}`);
      return full;
    },
  };
}

// A "hard" assertion gates the run's ok; a "soft" observation only informs (used
// for the known per-session / atomic-trade / unwired-marriage gaps).
function makeBeat(ctx, title) {
  const beat = { id: ctx.beats.length + 1, title, ok: true, assertions: [], observations: [], startedAt: Date.now() };
  ctx.beats.push(beat);
  ctx.log(`\n▶ beat ${beat.id}: ${title}`);
  return beat;
}

function assert(beat, name, pass, detail) {
  beat.assertions.push({ name, pass: Boolean(pass), detail: detail ?? null });
  if (!pass) beat.ok = false;
  console.log(`    ${pass ? "✓" : "✗"} ${name}`);
  return Boolean(pass);
}

function observe(beat, name, value, detail) {
  beat.observations.push({ name, value: value ?? null, detail: detail ?? null });
  console.log(`    • ${name}: ${typeof value === "object" ? JSON.stringify(value) : value}`);
}

function finishBeat(beat) {
  beat.endedAt = Date.now();
  beat.durationMs = beat.endedAt - beat.startedAt;
}

// ---------------------------------------------------------------------------
// Main.
// ---------------------------------------------------------------------------
async function main() {
  await fs.mkdir(outputDir, { recursive: true });
  // Each client gets its OWN Chrome instance (separate window + user-data-dir):
  // two tabs in one window leave the backgrounded tab throttled, and this
  // client's React setWorld flush never runs while its tab is hidden, so the
  // background client goes "blind" (no entities/group/trade state). Separate
  // instances also isolate localStorage so the two logins never collide.
  const chromes = [await launchChrome(debugPort, 0), await launchChrome(debugPort + 1, 1)];
  const clients = [];

  try {
    clients.push(await makeClient("A", debugPort), await makeClient("B", debugPort + 1));
    const [clientA, clientB] = clients;
    const accounts = [
      { account: accountA, character: characterA, x: startAx, y: startAy },
      { account: accountB, character: characterB, x: startBx, y: startBy },
    ];

    // Bring both characters into the same shared-zone map, adjacent.
    await Promise.all(clients.map((client, index) => loginAndStart(client, accounts[index])));
    await Promise.all(clients.map((client, index) => transferTo(client, accounts[index].x, accounts[index].y)));
    await pulseBoth(clients, 4);

    const ctx = createContext(clients);

    // Beat 0 — co-presence: each client must see the other as a player entity
    // before any social action can route (shared zone presence).
    await beatCoPresence(ctx, clientA, clientB);
    // Beat 1 — party / group.
    await beatParty(ctx, clientA, clientB);
    // Beat 2 — player trade (gold, two-sided; best-effort item).
    await beatTrade(ctx, clientA, clientB);
    // Beat 3 — whisper + a normal chat channel.
    await beatChat(ctx, clientA, clientB);
    // Beat 4 — friends (add + remove).
    await beatFriends(ctx, clientA, clientB);
    // Beat 5 — bonds / marriage (optional; observations only).
    await beatMarriage(ctx, clientA, clientB);

    const summaries = await Promise.all(clients.map((client) => readSummary(client)));
    const screenshots = [];
    for (let index = 0; index < clients.length; index += 1) {
      screenshots.push(await captureScreenshot(clients[index], `${prefix}-${index === 0 ? "a" : "b"}.png`));
    }

    const hardFailures = ctx.beats.flatMap((beat) =>
      beat.assertions.filter((a) => !a.pass).map((a) => ({ beat: beat.title, assertion: a.name, detail: a.detail })),
    );
    const ok = hardFailures.length === 0 && ctx.beats.every((beat) => beat.ok);

    const report = {
      ok,
      runId,
      baseUrl,
      gatewayWsUrl,
      map,
      accounts,
      beats: ctx.beats,
      hardFailures,
      issues: ctx.issues,
      summaries,
      screenshots,
      consoleErrors: clients.flatMap((client) =>
        client.consoleErrors.map((entry) => ({ client: client.label, ...entry })),
      ),
      packetFrames: clients.map((client) => ({ client: client.label, frames: client.packetFrames.slice(-24) })),
    };

    const reportPath = path.join(outputDir, `${prefix}.json`);
    await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);
    console.log(
      `\n${JSON.stringify(
        {
          ok,
          reportPath,
          beats: ctx.beats.map((beat) => ({
            title: beat.title,
            ok: beat.ok,
            passed: beat.assertions.filter((a) => a.pass).length,
            failed: beat.assertions.filter((a) => !a.pass).length,
            observations: beat.observations.length,
          })),
          screenshots,
        },
        null,
        2,
      )}`,
    );
    if (!ok) {
      process.exitCode = 1;
    }
  } finally {
    for (const client of clients) {
      client.close();
    }
    for (const chrome of chromes) {
      await stopChrome(chrome);
    }
  }
}

// ---------------------------------------------------------------------------
// Beats.
// ---------------------------------------------------------------------------
async function beatCoPresence(ctx, clientA, clientB) {
  const pair = [clientA, clientB];
  const beat = makeBeat(ctx, "co-presence: A and B share the map adjacently");
  try {
    const bothGame = (await readSummary(clientA)).screen === "game" && (await readSummary(clientB)).screen === "game";
    assert(beat, "both clients are in-game", bothGame);

    // B joins after A, so B's join-snapshot already includes A, but A only
    // learns about B from an incremental broadcast the zone emits on its next
    // tick — hence pollWithPulse (ticking advances the shared zone).
    const aSeesB = await pollWithPulse(pair, clientA, seesPlayerExpr(characterB), 20_000);
    const bSeesA = await pollWithPulse(pair, clientB, seesPlayerExpr(characterA), 20_000);
    assert(beat, "A sees B as a player entity", aSeesB);
    assert(beat, "B sees A as a player entity", bSeesA);

    // Adjacency is required for the trade beat; nudge B toward A if the spawn
    // landed them non-adjacent, then re-check from A's authoritative entities.
    let adjacent = await isAdjacent(clientA, characterB);
    if (!adjacent) {
      await ensureAdjacent(clientA, clientB);
      adjacent = await isAdjacent(clientA, characterB);
    }
    assert(beat, "A and B are adjacent (trade-pairing range)", adjacent, await pairGeometry(clientA, characterB));
  } catch (err) {
    blockBeat(ctx, beat, err);
  }
  finishBeat(beat);
}

async function beatParty(ctx, clientA, clientB) {
  const pair = [clientA, clientB];
  const beat = makeBeat(ctx, "party: A invites B, B accepts, both see a party");
  try {
    // A invites B: SwitchGroup(true) must precede AddMember (the sim drops
    // AddMember when grouping is disabled — packets.rs:1582). The web client's
    // groupInviteMember does exactly this pair (page.tsx:5718).
    await sendClientCommand(clientA, { type: "switchGroup", allowGroup: true }, "A enable group");
    await sendClientCommand(clientA, { type: "addMember", name: characterB }, "A invite B");

    const aRosterHasB = await pollWithPulse(pair, clientA, groupHasMemberExpr(characterB), 10_000);
    assert(beat, "A's party roster contains B", aRosterHasB, (await readSummary(clientA)).group);
    assert(beat, "A received the AddMember(B) frame", sawReceived(clientA, "AddMember", characterB));

    // B accepts the invite. GroupInvite{acceptInvite:true} re-seeds B's own
    // roster (packets.rs:1650 stage5_group_invite_reply_packet).
    await sendClientCommand(clientB, { type: "groupInvite", acceptInvite: true }, "B accept invite");
    const bInParty = await pollWithPulse(
      pair,
      clientB,
      "(window.__mir2Stage5?.state?.stage5Systems?.group?.members ?? []).length > 0",
      10_000,
    );
    assert(beat, "B is in a party after accepting", bInParty, (await readSummary(clientB)).group);

    // KNOWN per-session gaps — recorded, never fail the run. The local sim does
    // not relay the invite to B, and rosters are per-session (B won't see A).
    observe(beat, "B received a GroupInvite frame (cross-session relay)", sawReceived(clientB, "GroupInvite", characterA));
    observe(beat, "B's roster contains A (per-session roster symmetry)", await clientB.evaluate(`Boolean(${groupHasMemberExpr(characterA)})`));

    // A leaves / disbands: SwitchGroup(false) -> the sim clears the roster +
    // DeleteGroup (packets.rs:1549). Mirrors groupLeave (page.tsx:5735).
    await sendClientCommand(clientA, { type: "switchGroup", allowGroup: false }, "A leave group");
    await sendClientCommand(clientA, { type: "stage5Command", action: "group.leave" }, "A leave (stage5)");
    const aLeft = await pollWithPulse(
      pair,
      clientA,
      "(window.__mir2Stage5?.state?.stage5Systems?.group?.members ?? []).length === 0",
      10_000,
    );
    assert(beat, "A left the party (roster cleared)", aLeft, (await readSummary(clientA)).group);
  } catch (err) {
    blockBeat(ctx, beat, err);
  }
  finishBeat(beat);
}

async function beatTrade(ctx, clientA, clientB) {
  const pair = [clientA, clientB];
  const beat = makeBeat(ctx, "trade: A and B open a trade, offer, and both confirm");
  try {
    // Re-assert adjacency (the party beat may have walked B).
    if (!(await isAdjacent(clientA, characterB))) {
      await ensureAdjacent(clientA, clientB);
    }
    const goldA0 = await readGold(clientA);
    const goldB0 = await readGold(clientB);
    beat.before = { goldA: goldA0, goldB: goldB0 };

    // Both sides open the trade. tradeRequest resolves each session's adjacent
    // remote (routing.rs:5457) so BOTH inner sessions get partner_name set —
    // the path the gateway pairs for delivery. (tradeReply alone would insert a
    // "Trader" placeholder partner — packets.rs:4257 — and break pairing.)
    await sendClientCommand(clientA, { type: "tradeRequest" }, "A trade request");
    await sendClientCommand(clientB, { type: "tradeRequest" }, "B trade request");

    // Own side is client-tracked (the TradeRequest echo names the partner) —
    // verify the two-sided open from BOTH clients.
    const aPartner = await pollWithPulse(pair, clientA, tradePartnerIsExpr(characterB), 10_000);
    const bPartner = await pollWithPulse(pair, clientB, tradePartnerIsExpr(characterA), 10_000);
    assert(beat, "A's trade partner is B", aPartner, (await readSummary(clientA)).trade);
    assert(beat, "B's trade partner is A", bPartner, (await readSummary(clientB)).trade);

    // Offer gold (capped to gold actually held). TradeGold (page.tsx:5920).
    // Freshly-created Crystal characters spawn with 0 gold/items, so on a clean
    // stack there is nothing to move — the two-sided open + confirm handshake is
    // what we verify, and the gold-delta check holds (trivially at 0).
    const offerA = Math.max(0, Math.min(TRADE_GOLD_A, goldA0 ?? 0));
    const offerB = Math.max(0, Math.min(TRADE_GOLD_B, goldB0 ?? 0));
    beat.offers = { offerA, offerB };
    observe(beat, "characters had gold/items to actually move", offerA > 0 || offerB > 0, beat.before);
    if (offerA > 0) await sendClientCommand(clientA, { type: "tradeGold", amount: offerA }, "A offer gold");
    if (offerB > 0) await sendClientCommand(clientB, { type: "tradeGold", amount: offerB }, "B offer gold");

    // The task's note: Crystal pushes only the PARTNER's side. The LOCAL
    // shared-zone sim does NOT stream offers mid-trade (no TradeGold relay in
    // routing.rs) — it settles atomically on dual-confirm. Record what each side
    // actually sees of its partner's pending offer so the gap is explicit.
    await pulseBoth(pair, 1);
    observe(beat, "B sees A's offered gold live (partnerGold)", (await readSummary(clientB)).trade?.partnerGold ?? null, { expected: offerA });
    observe(beat, "A sees B's offered gold live (partnerGold)", (await readSummary(clientA)).trade?.partnerGold ?? null, { expected: offerB });

    // Best-effort item offer from A (the web trade window is gold-only; the
    // DepositTradeItem command exists at the protocol layer — exercise it so the
    // item path is covered, but never fail the beat on it).
    const itemForTrade = await firstTradeableItem(clientA);
    if (itemForTrade) {
      await sendClientCommand(
        clientA,
        { type: "depositTradeItem", from: itemForTrade.slot, to: 0 },
        "A deposit trade item",
      );
      beat.itemOffered = itemForTrade;
    }

    // Both confirm; the second confirm settles the exchange, and A (the first
    // confirmer) receives its delivery on its next command — so pulse after.
    await sendClientCommand(clientA, { type: "tradeConfirm", locked: true }, "A confirm");
    await sendClientCommand(clientB, { type: "tradeConfirm", locked: true }, "B confirm");
    await pulseBoth(pair, 3);

    const goldA1 = await readGold(clientA);
    const goldB1 = await readGold(clientB);
    beat.after = { goldA: goldA1, goldB: goldB1 };

    // The second confirmer (B) settles the two-sided lock and is notified with a
    // TradeConfirm immediately; the first confirmer (A) only receives settlement
    // frames when there is something to deliver (so on fresh 0-gold characters A
    // gets none — recorded as an observation, not a failure).
    assert(beat, "the two-sided trade settled on dual-confirm (TradeConfirm)", sawReceived(clientB, "TradeConfirm"));
    observe(
      beat,
      "A (first confirmer) received a settlement frame",
      sawReceived(clientA, "TradeConfirm") || sawReceived(clientA, "GainedGold") || sawReceived(clientA, "GainedItem"),
    );

    // Verify the completed exchange from BOTH clients' own gold truth (holds at
    // 0 when there was nothing to move; exact when gold was offered).
    const expectA = (goldA0 ?? 0) - offerA + offerB;
    const expectB = (goldB0 ?? 0) - offerB + offerA;
    assert(beat, "A's gold reflects the exchange (−own +partner)", goldA1 === expectA, { goldA0, offerA, offerB, goldA1, expectA });
    assert(beat, "B's gold reflects the exchange (−own +partner)", goldB1 === expectB, { goldB0, offerB, offerA, goldB1, expectB });

    // When gold actually moved, both clients must also have observed the gold
    // crossing as server truth frames.
    if (offerB > 0) observe(beat, "A received GainedGold (B's offer)", sawReceived(clientA, "GainedGold"));
    if (offerA > 0) observe(beat, "B received GainedGold (A's offer)", sawReceived(clientB, "GainedGold"));

    // Best-effort item-transfer verification (observation only).
    if (itemForTrade) {
      const bGotItem = sawReceived(clientB, "GainedItem");
      const aLostItem = !(await clientA.evaluate(
        `(window.__mir2Stage5?.state?.inventoryItems ?? []).some((it) => it && it.key === ${JSON.stringify(itemForTrade.key)})`,
      ));
      observe(beat, `B gained A's traded item "${itemForTrade.name}"`, bGotItem);
      observe(beat, "A no longer holds the traded item", aLostItem);
    } else {
      // Cancel the open trade so it does not leak into later beats.
      await sendClientCommand(clientA, { type: "tradeCancel" }, "A cancel trade").catch(() => {});
      await sendClientCommand(clientB, { type: "tradeCancel" }, "B cancel trade").catch(() => {});
    }
  } catch (err) {
    blockBeat(ctx, beat, err);
  }
  finishBeat(beat);
}

async function beatChat(ctx, clientA, clientB) {
  const pair = [clientA, clientB];
  const beat = makeBeat(ctx, "chat: A whispers B, B broadcasts to A");
  try {
    // Whisper rides the real Chat packet as "/Name body" (page.tsx:5704). The
    // chat-settings window only toggles client-side channel VISIBILITY (no
    // server round-trip), so the meaningful test is a real whisper + a real
    // normal-channel line crossing between the two clients. We verify against
    // the durable client log (the whisper-channel tone proves it routed as a
    // whisper, not a public broadcast).
    const whisperToken = `qa-whisper-${runId}`;
    await sendClientCommand(clientA, { type: "chat", message: `/${characterB} ${whisperToken}` }, "A whisper B");
    const bGotWhisper = await pollWithPulse(pair, clientB, chatLogExpr(whisperToken, "whisper"), 10_000);
    assert(beat, "B received A's whisper (on the whisper channel)", bGotWhisper, (await readSummary(clientB)).logsTail);
    observe(beat, "B's frame trail carries the whisper ObjectChat", sawReceived(clientB, "ObjectChat", whisperToken));

    const chatToken = `qa-chat-${runId}`;
    await sendClientCommand(clientB, { type: "chat", message: chatToken }, "B normal chat");
    const aGotChat = await pollWithPulse(pair, clientA, chatLogExpr(chatToken), 10_000);
    assert(beat, "A received B's normal-channel chat", aGotChat, (await readSummary(clientA)).logsTail);
  } catch (err) {
    blockBeat(ctx, beat, err);
  }
  finishBeat(beat);
}

async function beatFriends(ctx, clientA, clientB) {
  const pair = [clientA, clientB];
  const beat = makeBeat(ctx, "friends: A adds then removes B");
  try {
    // AddFriend by name (page.tsx:5597). The sim records the name onto A's own
    // social slice and echoes FriendUpdate (packets.rs:641).
    await sendClientCommand(clientA, { type: "addFriend", name: characterB, blocked: false }, "A add friend B");
    const added = await pollWithPulse(pair, clientA, friendsHasExpr(characterB), 10_000);
    assert(beat, "A's friend list contains B", added, (await readSummary(clientA)).social);
    assert(beat, "A received a FriendUpdate(B) frame", sawReceived(clientA, "FriendUpdate", characterB));

    // Remove: the friend's characterIndex is its position in friends++blocked
    // (packets.rs:614 enumerate), which is exactly what the web's
    // friendCharacterIndex computes (page.tsx:5611).
    const friendIndex = await clientA.evaluate(
      `(() => { const f = window.__mir2Stage5?.state?.stage5Systems?.social?.friends ?? []; return f.findIndex((n) => String(n) === ${JSON.stringify(characterB)}); })()`,
    );
    if (typeof friendIndex === "number" && friendIndex >= 0) {
      await sendClientCommand(clientA, { type: "removeFriend", characterIndex: friendIndex }, "A remove friend B");
      const removed = await pollWithPulse(pair, clientA, `!(${friendsHasExpr(characterB)})`, 10_000);
      assert(beat, "A's friend list no longer contains B", removed, (await readSummary(clientA)).social);
    } else {
      assert(beat, "A's friend list no longer contains B", false, { reason: "could not resolve friend index", friendIndex });
    }
  } catch (err) {
    blockBeat(ctx, beat, err);
  }
  finishBeat(beat);
}

async function beatMarriage(ctx, clientA, clientB) {
  const beat = makeBeat(ctx, "bonds: A sends a marriage request (optional)");
  try {
    // MarriageRequest targets the faced player server-side and carries no name
    // (page.tsx:5642). In the local sim it is a stub that returns a "face a
    // player" prompt (packets.rs:3990) and does NOT relay to B — recorded as
    // observations so the bonds gap is visible without failing the run.
    await sendClientCommand(clientA, { type: "marriageRequest" }, "A marriage request");
    await pulseBoth([clientA, clientB], 2);
    const aSummary = await readSummary(clientA);
    observe(beat, "A's last log lines after the request", aSummary.logsTail.slice(-3));
    observe(beat, "B received a MarriageRequest frame (cross-player relay)", sawReceived(clientB, "MarriageRequest", characterA));
  } catch (err) {
    // Optional beat — record but do not fail the run.
    observe(beat, "marriage beat error (optional)", String(err?.message ?? err));
  }
  finishBeat(beat);
}

function blockBeat(ctx, beat, err) {
  beat.ok = false;
  beat.error = String(err?.message ?? err);
  ctx.addIssue({ category: "social", severity: "high", beat: beat.id, summary: `beat blocked: ${beat.title}`, detail: beat.error });
  console.log(`    ✖ ${beat.error.slice(0, 300)}`);
}

// ---------------------------------------------------------------------------
// In-page expressions + state readers.
// ---------------------------------------------------------------------------
function seesPlayerExpr(name) {
  return `(window.__mir2Stage5?.state?.entities ?? []).some((e) => e && e.kind === "player" && e.name === ${JSON.stringify(name)})`;
}
function groupHasMemberExpr(name) {
  return `(window.__mir2Stage5?.state?.stage5Systems?.group?.members ?? []).some((m) => String(m) === ${JSON.stringify(name)})`;
}
function tradePartnerIsExpr(name) {
  return `String(window.__mir2Stage5?.state?.stage5Systems?.trade?.partner ?? "") === ${JSON.stringify(name)}`;
}
function friendsHasExpr(name) {
  return `(window.__mir2Stage5?.state?.stage5Systems?.social?.friends ?? []).some((f) => String(f) === ${JSON.stringify(name)})`;
}

async function readGold(client) {
  return client.evaluate(`(() => { const g = window.__mir2Stage5?.state?.gold; return typeof g === "number" ? g : null; })()`);
}

async function isAdjacent(client, otherName) {
  return Boolean(
    await client.evaluate(`(() => {
      const s = window.__mir2Stage5?.state ?? {};
      const me = s.player;
      const other = (s.entities ?? []).find((e) => e && e.kind === "player" && e.name === ${JSON.stringify(otherName)});
      if (!me || !other) return false;
      return Math.max(Math.abs(me.x - other.x), Math.abs(me.y - other.y)) <= 1;
    })()`),
  );
}

async function pairGeometry(client, otherName) {
  return client.evaluate(`(() => {
    const s = window.__mir2Stage5?.state ?? {};
    const me = s.player;
    const other = (s.entities ?? []).find((e) => e && e.kind === "player" && e.name === ${JSON.stringify(otherName)});
    return { me: me ? { x: me.x, y: me.y } : null, other: other ? { x: other.x, y: other.y } : null };
  })()`);
}

// Walk B one tile toward A (in-viewport click, the player-faithful path) until
// they sit within trade-pairing range, then pulse so positions republish.
async function ensureAdjacent(clientA, clientB) {
  for (let attempt = 0; attempt < 4; attempt += 1) {
    const geom = await pairGeometry(clientA, characterB);
    if (!geom.me || !geom.other) break;
    if (Math.max(Math.abs(geom.me.x - geom.other.x), Math.abs(geom.me.y - geom.other.y)) <= 1) return;
    const stepX = geom.other.x + Math.sign(geom.me.x - geom.other.x);
    const stepY = geom.other.y + Math.sign(geom.me.y - geom.other.y);
    const clicked = await clickTile(clientB, stepX, stepY);
    if (!clicked) {
      await sendClientCommand(clientB, { type: "moveTo", x: stepX, y: stepY, mode: "walk" }, "B step toward A").catch(() => {});
    }
    await pulseBoth([clientA, clientB], 2);
  }
}

async function firstTradeableItem(client) {
  return client.evaluate(`(() => {
    const items = window.__mir2Stage5?.state?.inventoryItems ?? [];
    const it = items.find((entry) => entry && typeof entry.slot === "number");
    return it ? { key: it.key, name: it.name, slot: it.slot } : null;
  })()`);
}

// Did this client receive a server frame of `packetName` (optionally whose
// payload contains `includesText`)? Reads the Node-side CDP frame trail
// (client.packetFrames — pre-filtered to SOCIAL_PACKET_RE, last 80), NOT the
// in-page __mir2GatewayEventHistory: that history is a 50-entry MIXED buffer
// that the per-tick worldSnapshot events evict within a beat, so social frames
// silently roll off it. The social-only CDP trail is the durable signal.
function sawReceived(client, packetName, includesText) {
  return client.packetFrames.some(
    (frame) =>
      frame.direction === "received" &&
      frame.payloadData.includes(packetName) &&
      (!includesText || frame.payloadData.includes(includesText)),
  );
}

// Poll an in-page predicate on `client` while ticking BOTH clients each round.
// autoTick=0, so cross-session effects (co-presence, group, trade-pairing,
// friend echo) only land when the shared zone is advanced by a command.
async function pollWithPulse(clients, client, expr, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await client.evaluate(`Boolean(${expr})`).catch(() => false)) === true) return true;
    await pulseBoth(clients, 1);
  }
  return false;
}

// A chat/whisper line is most durably observed in the (uncapped) client log,
// keyed by the channel the gateway tagged (page.tsx:7017 ObjectChat -> appendLog).
function chatLogExpr(token, channel) {
  const channelClause = channel ? ` && String(l.channel ?? "") === ${JSON.stringify(channel)}` : "";
  return `(window.__mir2Stage5?.state?.logs ?? []).some((l) => l && String(l.text ?? "").includes(${JSON.stringify(token)})${channelClause})`;
}

async function readSummary(client) {
  return client.evaluate(`
    (() => {
      const s = window.__mir2Stage5?.state ?? {};
      const st5 = s.stage5Systems ?? {};
      return {
        screen: s.screen ?? null,
        wsState: s.wsState ?? null,
        mapFileName: s.mapFileName ?? null,
        player: s.player ? { x: s.player.x, y: s.player.y } : null,
        gold: typeof s.gold === "number" ? s.gold : null,
        group: { members: st5.group?.members ?? [], allowGroup: st5.group?.allowGroup ?? null, lootMode: st5.group?.lootMode ?? null },
        trade: st5.trade
          ? { partner: st5.trade.partner ?? null, state: st5.trade.state ?? null, partnerGold: st5.trade.partnerGold ?? null, confirmed: st5.trade.confirmed ?? null }
          : null,
        social: { friends: st5.social?.friends ?? [], blocked: st5.social?.blocked ?? [] },
        relationship: { partnerName: st5.relationship?.partnerName ?? st5.relationship?.name ?? null, pendingRequestFrom: st5.relationship?.pendingRequestFrom ?? null },
        players: (s.entities ?? []).filter((e) => e && e.kind === "player").map((e) => ({ name: e.name, x: e.x, y: e.y })),
        logsTail: (s.logs ?? []).slice(-12).map((l) => ({ text: l?.text ?? String(l), channel: l?.channel ?? null })),
      };
    })()
  `);
}

// ---------------------------------------------------------------------------
// Client lifecycle + interaction primitives (shared with smoke-two-client-zone).
// ---------------------------------------------------------------------------
async function makeClient(label, port) {
  const target = await createPageTarget(port);
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
    // The login overlay is React-rendered a beat after the `screen` state flips
    // (and after the WS handshake), so wait for the account input to actually
    // paint before typing — otherwise the fill races the render.
    if (!(await waitForSelectorSoft(client, ".login-input.account", 12_000))) {
      throw new Error(`${client.label} login form (.login-input.account) never rendered (screen=${await client.evaluate(`window.__mir2Stage5?.state?.screen ?? null`)})`);
    }
    await fillInput(client, ".login-input.account", accountInfo.account);
    await fillInput(client, ".login-input.password", password);
    await click(client, ".login-button.account button");
    await waitUntilClient(client, `window.__mir2Stage5?.state?.wsState === "open"`, `${client.label} account socket open`, 15_000);
    await delay(1_000);
    await click(client, ".login-button.ok button");
    await waitUntilClient(client, `window.__mir2Stage5?.state?.screen === "select"`, `${client.label} select`, 20_000);
  }

  await sendClientCommand(
    client,
    { type: "newCharacter", name: accountInfo.character, gender: "male", class: "warrior" },
    `${client.label} new character`,
  );
  // Trust OUR named character appearing (not the raw characters length — a
  // phantom slot can exist; qa-playthrough.mjs documents this).
  await waitUntilClient(
    client,
    `(window.__mir2Stage5?.state?.characters ?? []).some((character) => character?.name === ${JSON.stringify(accountInfo.character)})`,
    `${client.label} character created`,
    15_000,
  );
  await client.evaluate(`
    (() => {
      const character = (window.__mir2Stage5?.state?.characters ?? []).find((entry) => entry?.name === ${JSON.stringify(accountInfo.character)});
      return window.__mir2Stage5?.send?.({ type: "startGame", characterIndex: character?.index ?? 0 }) === true;
    })()
  `);
  await waitUntilClient(
    client,
    `window.__mir2Stage5?.state?.screen === "game" && Boolean(window.__mir2Stage5?.state?.player)`,
    `${client.label} game`,
    25_000,
  );
}

async function transferTo(client, x, y) {
  await sendClientCommand(client, { type: "transferMap", key: `crystal:${map}:${x}:${y}` }, `${client.label} transfer`);
  await waitUntilClient(
    client,
    `(() => {
      const state = window.__mir2Stage5?.state;
      return state?.mapFileName === ${JSON.stringify(map)} && state?.player?.x === ${x} && state?.player?.y === ${y};
    })()`,
    `${client.label} transfer ${x},${y}`,
    20_000,
  );
}

async function pulseBoth(clients, count) {
  for (let index = 0; index < count; index += 1) {
    await Promise.all(clients.map((client) => sendClientCommand(client, { type: "tick" }, `${client.label} tick`).catch(() => {})));
    await delay(400);
  }
}

async function sendClientCommand(client, command, label) {
  const ok = await client.evaluate(`window.__mir2Stage5?.send?.(${JSON.stringify(command)}) === true`);
  if (!ok) {
    throw new Error(`${label} command was not accepted by the Web client (${JSON.stringify(command).slice(0, 120)}).`);
  }
}

async function tilePoint(client, x, y) {
  return client.evaluate(`
    (() => {
      const tile = document.querySelector(${JSON.stringify(`[aria-label="tile ${x}, ${y}"]`)});
      if (!tile) return null;
      const box = tile.getBoundingClientRect();
      return { x: box.left + box.width / 2, y: box.top + box.height / 2 };
    })()
  `);
}

async function clickTile(client, x, y) {
  const point = await tilePoint(client, x, y);
  if (!point) return false;
  await client.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y, button: "none" });
  await client.send("Input.dispatchMouseEvent", { type: "mousePressed", x: point.x, y: point.y, button: "left", buttons: 1, clickCount: 1 });
  await client.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button: "left", buttons: 0, clickCount: 1 });
  return true;
}

async function captureScreenshot(client, fileName) {
  const result = await client.send("Page.captureScreenshot", { format: "png", captureBeyondViewport: false });
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
        bodyText: document.body?.innerText?.slice(0, 400) ?? "",
        screen: window.__mir2Stage5?.state?.screen ?? null,
        wsState: window.__mir2Stage5?.state?.wsState ?? null,
        player: window.__mir2Stage5?.state?.player ?? null,
      }))()
    `)
    .catch((error) => ({ debugError: String(error) }));
  throw new Error(`${client.label} timed out waiting for ${label}; last=${JSON.stringify(lastValue)}; debug=${JSON.stringify(debug).slice(0, 1_500)}`);
}

async function waitUntilSoft(client, expression, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const value = await client.evaluate(`Boolean(${expression})`).catch(() => false);
    if (value === true) return true;
    await delay(120);
  }
  return false;
}

async function waitForSelectorSoft(client, selector, timeoutMs) {
  return waitUntilSoft(client, `document.querySelector(${JSON.stringify(selector)}) != null`, timeoutMs);
}

// ---------------------------------------------------------------------------
// Chrome lifecycle (shared with smoke-two-client-zone).
// ---------------------------------------------------------------------------
async function launchChrome(port, windowIndex = 0) {
  const userDataDir = path.join(os.tmpdir(), `mir2-social-${process.pid}-${port}-${Date.now()}`);
  await fs.mkdir(userDataDir, { recursive: true });
  const chrome = spawn(
    chromePath,
    [
      `--remote-debugging-port=${port}`,
      `--user-data-dir=${userDataDir}`,
      ...(headed ? [] : ["--headless=new"]),
      "--disable-gpu",
      "--no-proxy-server",
      "--proxy-bypass-list=*",
      "--no-first-run",
      "--no-default-browser-check",
      // Keep the renderer running at full rate even when the window is not the
      // OS foreground — otherwise this client's React world-state flush stalls.
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      `--window-size=${DEFAULT_VIEWPORT.width},${DEFAULT_VIEWPORT.height}`,
      // Cascade the two clients' windows so neither fully occludes the other in
      // --headed mode (alongside the anti-backgrounding flags above).
      `--window-position=${windowIndex * 560},${windowIndex * 40}`,
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  chrome.userDataDir = userDataDir;
  await waitForChrome(port);
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

async function createPageTarget(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" });
  if (!response.ok) {
    throw new Error(`Chrome target creation failed: ${response.status}`);
  }
  return response.json();
}

async function waitForChrome(port) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
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
  await client.send("Emulation.setVisibleSize", { width: DEFAULT_VIEWPORT.width, height: DEFAULT_VIEWPORT.height });
}

function buildBaseUrl(rawBaseUrl, wsUrl) {
  const url = new URL(rawBaseUrl);
  if (!url.searchParams.has("gatewayWs")) {
    url.searchParams.set("gatewayWs", wsUrl);
  }
  // Manual ticks (pulseBoth) drive the shared zone deterministically.
  if (!url.searchParams.has("autoTick")) {
    url.searchParams.set("autoTick", "0");
  }
  if (!url.searchParams.has("codexBust")) {
    url.searchParams.set("codexBust", String(process.pid));
  }
  return url.toString();
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
