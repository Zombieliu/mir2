// Real-time, ordinary local Windows life acceptance. Run only after reviewing
// the new Gateway and this script. --check does not start services or read keys.
import assert from "node:assert/strict";
import { randomUUID, createHash } from "node:crypto";
import { appendFileSync } from "node:fs";
import { mkdir, realpath, stat, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, parse, relative, resolve } from "node:path";
import { devNull } from "node:os";
import { performance } from "node:perf_hooks";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "./companion-trial-fixture.mjs";
import { isolatedEnvironment, resolveGatewayExecutable, repoRoot, sha256 } from "./local-gateway.mjs";
import { fundSyntheticSupplyActor } from "./supply-acceptance-funding.mjs";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";
import { facingDirection } from "../src/companion-policy.mjs";
import { loadLocalModelSettings } from "../src/local-model-settings.mjs";
import { loadLocalModelNetwork, modelProcessEnvironment } from "../src/model-runtime-environment.mjs";
import { purchaseReceipt } from "../src/supply/purchase-wire.mjs";

const HELP = "node scripts/run-life-acceptance.mjs --gateway-exe <absolute-new-gateway.exe> --out-root F:/life-test [--settings-bundle <existing-local-settings-directory>]";
const HOME = { x: 288, y: 616 }, LIFE_MS = 1_200_000, RESTART_AFTER_MS = 480_000;
const PREPARATION_BOUNDS = { minX: 270, minY: 595, maxX: 310, maxY: 645 };
const OFFSETS = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] };
const POTIONS = new Set(["(HP)DrugSmall", "(HP)DrugMedium", "(HP)DrugLarge", "(HP)DrugXL"]);
const n = value => Number.isSafeInteger(value) ? value : null;
const point = value => value && n(value.x) !== null && n(value.y) !== null ? { x: value.x, y: value.y } : null;
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const hash = value => createHash("sha256").update(typeof value === "string" ? value : JSON.stringify(value)).digest("hex");
const within = (root, path) => { const part = relative(root, path); return part === "" || part !== ".." && !part.startsWith("..\\") && !part.startsWith("../") && !isAbsolute(part); };
const text = (value, maximum = 160) => {
  if (typeof value !== "string" || /\b(?:sk|ghp|github_pat)[-_][a-z0-9_-]{8,}|\bbearer\s+\S+|(?:password|api[ _-]?key|secret)\s*[:=]\s*\S+/iu.test(value)) return null;
  return [...value.replace(/[\p{Cc}\p{Cf}\p{Cs}]/gu, " ")].slice(0, maximum).join("");
};
const identifierHash = value => typeof value === "string" && value ? hash(value) : null;
const countPotions = state => state.inventoryStale !== false ? null : [...state.inventoryItems ?? [], ...state.beltItems ?? []]
  .filter(item => POTIONS.has(item.name)).reduce((sum, item) => n(item.quantity) !== null && item.quantity >= 0 && sum !== null ? sum + item.quantity : null, 0);

function argumentsFor(args) {
  if (args.length === 1 && args[0] === "--help") return { help: true };
  const result = {}, flags = new Map([["--gateway-exe", "gatewayExecutable"], ["--out-root", "outputRoot"], ["--settings-bundle", "settingsBundle"]]);
  for (let index = 0; index < args.length; index++) {
    if (args[index] === "--return-check-seconds") {
      const value = args[++index];
      if (result.returnCheckSeconds !== undefined || !/^(?:[6-9][0-9]|1[0-7][0-9]|180)$/.test(value ?? "")) throw new Error("Use 60–180 real seconds for the separate cancellation return check.");
      result.returnCheckSeconds = Number(value); continue;
    }
    const key = flags.get(args[index]), value = args[++index];
    if (!key || result[key] || !value || !isAbsolute(value)) throw new Error("Use explicit absolute local acceptance paths.");
    result[key] = resolve(value);
  }
  if (!result.gatewayExecutable || !result.outputRoot || process.platform !== "win32" || process.versions.node !== "22.18.0") throw new Error("Windows, Node.js 22.18.0 and explicit new Gateway/output paths are required.");
  return result;
}

async function privateRoot(path) {
  if (parse(path).root.toLowerCase() !== "f:\\" || path === parse(path).root || within(repoRoot, path) || within(path, repoRoot)) throw new Error("Use a dedicated short F output directory outside the source tree.");
  let ancestor = path;
  for (;;) {
    try { await stat(ancestor); break; } catch (error) { if (error.code !== "ENOENT") throw error; const parent = dirname(ancestor); if (parent === ancestor) throw error; ancestor = parent; }
  }
  if (parse(await realpath(ancestor)).root.toLowerCase() !== "f:\\") throw new Error("The output ancestor must resolve to F.");
  await mkdir(path, { recursive: true }); const root = await realpath(path);
  if (parse(root).root.toLowerCase() !== "f:\\") throw new Error("The private output must resolve to F.");
  return root;
}

let fixture, actor, owner, profile, sessionRoot, files, phase = "arguments", fatal = null, modelEnvironment = {}, latest = null;
let lifePermitId, lifeStartedAt, lifeExpiresAt, modelEnabled = false, gatewayHash, funding, giftProof, activationProof, restartProof, finalProof;
let heartbeat, hardTimer, stopRequested = false, cleanupPromise, mainPassed = false, fixtureClosed = false, ownerClosed = false, evidenceFailure = false;
let failurePhase = null, failureCode = null, failedAssertion = null, failureHint = null;
const beganWall = Date.now(), beganMonotonic = performance.now(), actorPids = [], actorChildren = [], checkpoints = [], limits = [];
const commands = {}, proofCounts = {}, modelCalls = new Map(), ownIds = new Set(), hits = new Map(), samples = [];
let proofLines = 0, proofDropped = 0, participationDeaths = 0, observedHits = 0, lastSampleWrittenAt = 0, maximumHomeDistance = 0, visitedHuntArea = false;
function emit(channel, value, stdout = false) {
  const event = { atMs: Date.now(), ...value };
  if (files) { try { appendFileSync(files[channel], JSON.stringify(event) + "\n", "utf8"); } catch { evidenceFailure = true; } }
  if (stdout) process.stdout.write(JSON.stringify(event) + "\n");
}
function stage(value) { phase = value; emit("report", { type: "lifeAcceptancePhase", phase }, true); }
function guard() {
  if (stopRequested || fatal || evidenceFailure || performance.now() - beganMonotonic > 28 * 60_000) throw new Error("The bounded real-time acceptance must stop.");
  if (Math.abs((Date.now() - beganWall) - (performance.now() - beganMonotonic)) > 5000) { fatal = "wall-clock-changed"; throw new Error("Real elapsed time could not be confirmed."); }
}
function publicSnapshot(state) {
  const ownerVisible = state.entities?.some(entity => entity.kind === "player" && entity.name === fixture?.owner.characterName) === true;
  return { ready: state.ready === true, mapFileName: text(state.mapFileName, 32), position: point(state.player), hp: n(state.playerHp), maxHp: n(state.playerMaxHp),
    dead: state.player?.dead === true || state.playerHp === 0, gold: n(state.gold), hpPotions: countPotions(state), freeBagSlots: n(state.freeBagSlots),
    currentWeight: n(state.currentWeight), maxWeight: n(state.maxWeight), inventoryCurrent: state.inventoryStale === false, inSafeZone: state.inSafeZone === true,
    ownerVisible, tradeActive: state.trade?.active === true, tradeUncertain: state.trade?.uncertain === true };
}
function onEvent(message) {
  if (message?.type === "command" && typeof message.command === "string") {
    if (/qa|stage5|spawn|teleport|giveItem|passkey/i.test(message.command)) fatal = "unexpected-privileged-command";
    if (/^[A-Za-z][A-Za-z0-9]{0,48}$/.test(message.command)) { commands[message.command] = (commands[message.command] ?? 0) + 1; emit("proof", { type: "ordinaryActorCommand", command: message.command }); }
    return;
  }
  if (message?.type === "proof") {
    const event = message.event ?? {}, payload = event.payload ?? {}, packet = event.packet;
    if (event.type === "snapshotPosition") {
      if (Number.isInteger(payload.selfZoneObjectId)) ownIds.add(payload.selfZoneObjectId);
      if (proofLines++ < 50_000) emit("proof", { type: "position", position: point(payload.player), tick: typeof payload.tick === "bigint" ? payload.tick.toString() : n(payload.tick) }); else proofDropped++;
      return;
    }
    if (!["ObjectStruck", "ObjectAttack", "ObjectDied", "ObjectWalk", "UserLocation", "UseItem", "DeleteItem", "GainedItem", "ObjectItem", "HealthChanged"].includes(packet)) return;
    proofCounts[packet] = (proofCounts[packet] ?? 0) + 1;
    if (packet === "ObjectStruck" && ownIds.has(payload.attackerId) && n(payload.objectId) !== null && Date.now() >= (lifeStartedAt ?? Infinity) && Date.now() < (lifeExpiresAt ?? 0)) { observedHits++; hits.set(payload.objectId, Date.now()); }
    if (packet === "ObjectDied" && hits.has(payload.objectId)) { if (Date.now() - hits.get(payload.objectId) <= 30_000) participationDeaths++; hits.delete(payload.objectId); }
    const item = payload.item;
    if (proofLines++ < 50_000) emit("proof", { type: "ordinaryGameEvent", packet, objectId: n(payload.objectId), attackerId: n(payload.attackerId), position: point(payload.location ?? payload),
      hp: n(payload.hp), success: typeof payload.success === "boolean" ? payload.success : null,
      ...(item ? { item: { itemIndex: n(item.itemIndex ?? item.item_index), quantity: n(item.count ?? item.quantity), uniqueIdHash: hash(String(item.uniqueId ?? item.unique_id)) } } : {}) }); else proofDropped++;
    return;
  }
  if (message?.type === "report" && ["fixtureRunStopped", "residentStopped", "companionDecision", "companionStep", "companionNavigationBlocked", "companionMovementRejected"].includes(message.event?.type)) {
    const value = message.event;
    if (value.type === "fixtureRunStopped" && !["restart-stop", "cleanup"].includes(phase)) fatal = "automatic-actor-stopped";
    emit("report", { type: value.type, kind: text(value.kind, 24), reason: text(value.reason), direction: Object.hasOwn(OFFSETS, value.direction ?? "") ? value.direction : null,
      from: point(value.from), target: point(value.target), ...(value.type === "companionMovementRejected" ? { destination: point(value.destination) } : {}), saved: value.saved === true,
      ...(value.type === "companionNavigationBlocked" ? { occupied: (value.occupied ?? []).slice(0, 128).map(point).filter(Boolean), occupiedTruncated: value.occupiedTruncated === true } : {}) });
  }
}

function readExecution(path, sql) {
  const db = new DatabaseSync(path, { readOnly: true });
  try { db.exec("PRAGMA busy_timeout=3000"); return db.prepare(sql).all(); } finally { db.close(); }
}
function lifeState() {
  const path = `${profile.companion.memoryPath}.life.sqlite`;
  assert.ok(within(fixture.output, path));
  const row = readExecution(path, "SELECT grant_json,task_json FROM life_tasks ORDER BY sequence DESC LIMIT 1")[0];
  if (!row) return null;
  const grant = JSON.parse(row.grant_json), task = JSON.parse(row.task_json);
  return { id: grant.lifePermitId, startedAtMs: n(grant.startedAtMs), expiresAtMs: n(grant.expiresAtMs), durationSeconds: n(grant.durationSeconds),
    status: text(task.status, 32), phase: text(task.step, 48), activationPending: task.activationPending === true,
    budgetGold: n(grant.budgetGold), maxPurchasedQuantity: n(grant.maxPurchasedQuantity), spentGold: n(task.spentGold), purchasedQuantity: n(task.purchasedQuantity), childSupplyClaimed: task.childSupplyClaimed === true,
    childSupplyPermitId: task.childSupplyPermitId, killsObserved: n(task.killsObserved), modelDecisionsUsed: n(task.modelDecisionsUsed), expired: task.expired === true,
    navigation: task.navigation ?? null, reason: text(task.reason) };
}
function publicLife(value) { if (!value) return null; const { id, childSupplyPermitId, ...safe } = value; return { ...safe, lifeIdHash: identifierHash(id), childSupplyIdHash: identifierHash(childSupplyPermitId) }; }
function receipts() {
  const path = `${profile.companion.memoryPath}.supply.sqlite`;
  assert.ok(within(fixture.output, path));
  return readExecution(path, "SELECT permit_id,task_json FROM supply_tasks ORDER BY sequence DESC LIMIT 4").map(row => {
    const task = JSON.parse(row.task_json); if (!task.operation || !task.receipt) return null;
    const receipt = purchaseReceipt(task.receipt, task.operation), committed = receipt.entry.outcome.committed;
    return { permitId: row.permit_id, operationHash: hash(task.operation), attempted: task.attempted === true, receiptRevision: receipt.entry.serverRevision,
      committed: Boolean(committed), chargedGold: committed?.charged ?? 0, quantity: committed?.admittedCount ?? 0,
      currency: text(committed?.currency, 16), source: text(committed?.source, 16), incomingUniqueIdHash: committed ? hash(committed.incomingUniqueId) : null };
  }).filter(Boolean);
}
function publicReceipts(value) { return value.map(({ permitId, ...receipt }) => ({ ...receipt, permitIdHash: identifierHash(permitId) })); }
function publicModel(value) {
  const life = value.lifePlan ?? {}, call = life.lastCall ?? {};
  const known = { model: text(call.model ?? value.model, 100), status: text(call.status, 40), requestId: text(call.requestId, 160), responseId: text(call.responseId, 160), inputTokens: n(call.inputTokens), outputTokens: n(call.outputTokens) };
  if (known.status === "completed" && (known.requestId || known.responseId)) modelCalls.set(`${known.requestId}:${known.responseId}`, known);
  return { configured: value.modelEnabled === true, provider: text(value.provider, 24), model: text(value.model, 100), modelCalls: n(value.modelCalls),
    lifePlanStatus: text(life.status, 32), reply: text(life.reply, 400), ordinal: n(life.ordinal), lastCall: known };
}
async function collect(label, force = false) {
  guard();
  const [snapshot, model] = await Promise.all([actor.request("snapshot"), actor.request("modelStatus")]);
  ownIds.add(snapshot.playerObjectId); ownIds.add(snapshot.selfZoneObjectId);
  const current = publicSnapshot(snapshot), life = lifeState(), economic = receipts();
  if (!current.ready || !current.position || current.dead) { fatal = current.dead ? "actor-died" : "actor-world-unavailable"; throw new Error("The real actor is not safely observable."); }
  if (life?.status === "paused" && ["independent-life-with-owner-logged-out", "resumed-independent-life-until-real-expiry"].includes(phase)) { fatal = "life-paused-before-completion"; throw new Error("The real life stopped and needs attention; do not wait out a non-running loop."); }
  maximumHomeDistance = Math.max(maximumHomeDistance, distance(current.position, HOME)); if (current.position.y >= 625) visitedHuntArea = true;
  latest = { label, phase, snapshot: current, life: publicLife(life), receipts: publicReceipts(economic), model: publicModel(model) };
  if (force || Date.now() - lastSampleWrittenAt >= 5000) { emit("samples", { type: "lifeSample", ...latest }); lastSampleWrittenAt = Date.now(); if (samples.length < 12 || force) samples.push(latest); }
  if (modelEnabled) assert.ok(model.modelCalls <= 4, "The real life must not exceed four provider attempts.");
  assert.ok(economic.filter(value => value.committed).length <= 1); assert.ok(economic.reduce((sum, value) => sum + value.chargedGold, 0) <= 50);
  return { snapshot, model, life, economic, public: latest };
}
async function poll(read, accept, timeoutMs = 40_000) {
  const until = performance.now() + timeoutMs;
  while (performance.now() < until) { guard(); const value = await read(); if (accept(value)) return value; await sleep(500); }
  throw new Error("A bounded ordinary life observation did not arrive.");
}
async function realWait(ms) { const until = performance.now() + ms; while (performance.now() < until) { guard(); await sleep(Math.min(1000, until - performance.now())); } }
async function controls(operation, payload = {}) {
  guard();
  const reply = await owner.companionControl(operation, { residentId: profile.companion.residentId, requestId: `life-acceptance:${randomUUID()}`, payload });
  emit("controls", { type: "ownerControl", operation, status: text(reply.status, 24) }); assert.equal(reply.status, "ok"); return reply.result;
}
async function ownerWalk(collision, target, desiredDistance = 0) {
  for (let step = 0; step < 120; step++) {
    guard(); const state = owner.snapshot();
    assert.ok(state.ready && state.mapFileName === "0" && state.playerHp === state.playerMaxHp && state.playerHp > 0, "The synthetic owner must remain healthy during preparation.");
    if (distance(state.player, target) <= desiredDistance) return;
    const occupied = state.entities.filter(value => value.objectId !== state.playerObjectId && value.objectId !== state.selfZoneObjectId && value.dead !== true).map(point).filter(Boolean);
    const direction = nextWalkDirection({ player: state.player, target, desiredDistance, collision, occupied, movementBounds: PREPARATION_BOUNDS });
    assert.ok(direction, "Only an ordinary observed collision-respecting owner path is allowed."); await owner.walk(direction); await realWait(650);
  }
  throw new Error("The ordinary owner walk exhausted its allowance.");
}
async function openOwner() { owner = await fixture.newOwnerClient(); ownerClosed = false; return owner; }
async function closeOwner() { if (!owner) return; try { if (owner.snapshot().ready) assert.equal(await owner.logout(), true); } finally { await owner.close(); owner = null; ownerClosed = true; } }
async function cleanup() {
  if (cleanupPromise) return cleanupPromise;
  cleanupPromise = (async () => { phase = "cleanup"; let okay = true;
    const escalation = setTimeout(() => { for (const child of actorChildren) if (child.exitCode === null && child.signalCode === null) child.kill(); fixture?.gateway.close().catch(() => {}); }, 45_000);
    try {
      try { await fixture?.stopActor(); } catch { okay = false; }
      actor = null;
      try { await closeOwner(); } catch { okay = false; }
      try { await fixture?.close(); fixtureClosed = Boolean(fixture); } catch { okay = false; }
    } finally { clearTimeout(escalation); }
    return okay;
  })();
  return cleanupPromise;
}

async function main(options) {
  const root = await privateRoot(options.outputRoot); sessionRoot = resolve(root, `lf-${randomUUID().replace(/-/g, "").slice(0, 10)}`);
  const publication = resolve(sessionRoot, "trial-bootstrap-00000000-0000-0000-0000-000000000000", `supply-server.journal.json.initialized.tmp.${"0".repeat(64)}`);
  assert.ok(publication.length <= 240, "Use a shorter F output path for real Win32 journal publication.");
  await mkdir(sessionRoot); files = Object.fromEntries(["report", "proof", "controls", "samples"].map(channel => [channel, resolve(sessionRoot, `life-${channel}.jsonl`)]));
  for (const file of Object.values(files)) await writeFile(file, "", { flag: "wx", mode: 0o600 });
  emit("report", { type: "lifeAcceptanceStarted", driverPid: process.pid, parentPid: process.ppid, sessionRoot, maximumMinutes: 30, realLifeSeconds: 1200, fakeClock: false }, true);
  heartbeat = setInterval(() => emit("report", { type: "lifeAcceptanceProgress", phase, elapsedSeconds: Math.floor((performance.now() - beganMonotonic) / 1000), latest }, true), 30_000);
  const gatewayExecutable = await resolveGatewayExecutable(options.gatewayExecutable); gatewayHash = await sha256(gatewayExecutable);
  stage("fresh-ordinary-private-world"); fixture = await createTrialFixture({ gatewayExecutable, outputRoot: sessionRoot, onEvent });
  assert.ok(within(sessionRoot, fixture.output) && within(sessionRoot, fixture.bootstrapOutput));
  profile = fixture.profileA; assert.ok(within(fixture.output, profile.companion.memoryPath));
  if (options.settingsBundle) {
    const settingsBundle = await realpath(options.settingsBundle);
    assert.ok(!within(settingsBundle, sessionRoot) && !within(sessionRoot, settingsBundle));
    const local = await loadLocalModelSettings({ bundleDir: settingsBundle, env: isolatedEnvironment({}) });
    assert.equal(local.provider, "openai");
    try { modelEnvironment = modelProcessEnvironment(local.apiKey, await loadLocalModelNetwork({ bundleDir: settingsBundle })); } finally { delete local.apiKey; }
    Object.assign(profile.runtimeService, { provider: "openai", model: "gpt-6-luna", credentialRef: "OPENAI_API_KEY", promptVersion: "alo-life-v1", maxCallsPerHour: 20 }); modelEnabled = true;
    await writeFile(fixture.profileAPath, JSON.stringify(profile, null, 2) + "\n", { mode: 0o600 });
  }
  const collision = await loadClientCollision("0");
  stage("owner-ordinary-starter-resale");
  // The inherited quest-client trace is deliberately sent to the OS null
  // device; this script never persists its private login/session/world frames.
  funding = await fundSyntheticSupplyActor({ url: fixture.gateway.url, identity: fixture.owner, tracePath: devNull, collision, checkpoint: stage });
  assert.equal(funding.finalGold, 85); assert.equal(funding.syntheticOnly, true);
  stage("owner-save-lease-settle"); await realWait(31_000);
  actor = await fixture.startActor(profile, { run: true, modelEnvironment }); actorPids.push(actor.pid); actorChildren.push(actor.child);
  ownIds.add(actor.state.playerObjectId); ownIds.add(actor.state.selfZoneObjectId); await openOwner();
  stage("ordinary-owner-gift-facing");
  let start = await collect("actor-before-owner-gift", true); assert.equal(start.model.modelCalls, 0); const actorBeforeGold = start.snapshot.gold;
  const offset = OFFSETS[start.snapshot.player.direction]; assert.ok(offset);
  await ownerWalk(collision, { x: start.snapshot.player.x + offset[0], y: start.snapshot.player.y + offset[1] });
  await owner.turn(facingDirection(owner.snapshot().player, start.snapshot.player)); await realWait(650);
  start = await collect("ready-for-mutual-facing-trade", true);
  const actualOffset = OFFSETS[start.snapshot.player.direction]; assert.ok(actualOffset);
  assert.deepEqual(point(owner.snapshot().player), { x: start.snapshot.player.x + actualOffset[0], y: start.snapshot.player.y + actualOffset[1] });
  const ownerBeforeGold = owner.snapshot().gold; assert.equal(ownerBeforeGold, 85);
  stage("ordinary-owner-gift-invite");
  await owner.requestPlayerTrade(); await poll(() => collect("owner-trade-accepted"), value => value.snapshot.trade.active && value.snapshot.trade.partnerName === fixture.owner.characterName);
  await poll(async () => owner.snapshot(), value => value.trade?.active === true && value.trade?.ownOfferKnown === true && value.fullSnapshot?.tradeKnown === true && value.inventoryStale === false);
  stage("ordinary-owner-gift-offer");
  const offered = await owner.offerTradeGold(50); assert.equal(offered.offerObserved, true);
  await poll(() => controls("view"), value => value.runtime.giftTask?.confirmAttempted === true);
  stage("ordinary-owner-gift-confirm"); await owner.confirmPlayerTrade();
  const gift = await poll(() => controls("view"), value => value.runtime.giftTask?.status === "completed");
  const afterGift = await collect("owner-gift-completed", true), ownerAfter = owner.snapshot();
  assert.equal(ownerAfter.gold, ownerBeforeGold - 50); assert.equal(afterGift.snapshot.gold, actorBeforeGold + 50); assert.equal(gift.runtime.giftTask.receivedGold, 50);
  assert.equal(afterGift.model.modelCalls, 0); assert.equal(ownerAfter.trade.active, false); assert.equal(ownerAfter.trade.uncertain, false);
  giftProof = { ownerBeforeGold, ownerAfterGold: ownerAfter.gold, actorBeforeGold, actorAfterGold: afterGift.snapshot.gold, receivedGold: 50,
    giftTaskCompleted: true, ownerFinalTradeObserved: Boolean(ownerAfter.trade.lastFinalConfirm), actorPaidAssets: false, modelCallsBeforeLife: 0 };
  checkpoints.push({ phase: "gift-confirmed", ...giftProof });
  stage("owner-ordinary-walk-outside-actor-AOI"); await ownerWalk(collision, { x: 309, y: 634 });
  const separated = await poll(() => collect("owner-outside-actual-actor-view"), value => value.public.snapshot.ownerVisible === false);
  stage("authorize-real-twenty-minute-life"); const view = await controls("view");
  const authorized = await controls("authorizeLife", { expectedPolicyVersion: view.profile.policyVersion, budgetGold: 50, durationSeconds: 1200 });
  if (authorized.runtime.lifeTask?.activationPending === true) assert.equal(authorized.runtime.modelCalls, 0);
  const active = await poll(() => collect("life-server-activated", true), value => value.life?.status === "running" && value.life.activationPending === false);
  lifePermitId = active.life.id; lifeStartedAt = active.life.startedAtMs; lifeExpiresAt = active.life.expiresAtMs;
  assert.equal(lifeExpiresAt - lifeStartedAt, LIFE_MS); assert.equal(active.life.budgetGold, 50); assert.equal(active.life.maxPurchasedQuantity, 1); assert.equal(active.life.durationSeconds, 1200);
  activationProof = { lifeIdHash: hash(lifePermitId), startedAtMs: lifeStartedAt, expiresAtMs: lifeExpiresAt, durationSeconds: 1200, budgetGold: 50,
    ownerWasOutsideActorView: separated.public.snapshot.ownerVisible === false, actorPositionAtActivation: active.public.snapshot.position, modelEnabled };
  await closeOwner(); stage("independent-life-with-owner-logged-out");
  await poll(() => collect("owner-logged-out"), value => value.public.snapshot.ownerVisible === false);
  if (options.returnCheckSeconds !== undefined) {
    await poll(() => collect("short-observe-real-away-route"), value => value.snapshot.player.y >= 628 && distance(value.snapshot.player, HOME) >= 7, options.returnCheckSeconds * 1000);
    if (Date.now() < lifeStartedAt + options.returnCheckSeconds * 1000) await realWait(lifeStartedAt + options.returnCheckSeconds * 1000 - Date.now());
    const before = await collect("short-before-owner-cancel", true);
    stage("short-owner-cancel-and-return"); await openOwner(); await controls("cancelLife");
    const returned = await poll(() => collect("short-cancel-return", true), value => ["cancelled", "paused"].includes(value.life?.status), 90_000);
    assert.equal(returned.life.status, "cancelled");
    assert.ok(distance(returned.snapshot.player, HOME) <= 1 && returned.snapshot.inSafeZone === true);
    assert.equal(returned.life.spentGold, 40); assert.equal(returned.life.purchasedQuantity, 1);
    assert.equal(returned.economic.filter(value => value.committed).length, 1);
    finalProof = { kind: "short-ordinary-cancel-return", actualElapsedSinceGrantMs: Date.now() - lifeStartedAt,
      before: before.public, final: returned.public, actualArrivalDistance: distance(returned.snapshot.player, HOME), expired: false,
      originalDeadlinePreserved: returned.life.expiresAtMs === lifeExpiresAt, fullTwentyMinuteExpiryClaimed: false };
    mainPassed = true; return;
  }
  while (Date.now() < lifeStartedAt + RESTART_AFTER_MS) { await collect("first-life-process"); await realWait(2000); }
  const beforeRestart = await collect("before-real-process-restart", true), firstPid = actor.pid;
  stage("restart-stop"); await fixture.stopActor(); actor = null; hits.clear(); stage("real-thirty-one-second-actor-lease-wait"); await realWait(31_000);
  actor = await fixture.startActor(profile, { run: true, modelEnvironment }); actorPids.push(actor.pid); actorChildren.push(actor.child); assert.notEqual(actor.pid, firstPid);
  const reboot = await collect("different-process-before-owner-resume", true);
  assert.equal(reboot.life.id, lifePermitId); assert.equal(reboot.life.status, "paused"); assert.equal(reboot.life.expiresAtMs, lifeExpiresAt);
  assert.equal(reboot.life.childSupplyClaimed, beforeRestart.life.childSupplyClaimed); assert.equal(reboot.life.spentGold, beforeRestart.life.spentGold);
  assert.equal(reboot.model.modelCalls, beforeRestart.model.modelCalls);
  await realWait(5000); const stillPaused = await collect("paused-does-not-renew-money-or-model", true);
  assert.equal(stillPaused.life.status, "paused"); assert.equal(stillPaused.life.spentGold, reboot.life.spentGold); assert.equal(stillPaused.model.modelCalls, reboot.model.modelCalls);
  assert.deepEqual(stillPaused.economic.map(value => value.operationHash), reboot.economic.map(value => value.operationHash));
  await openOwner(); const resumeView = await controls("view"); await controls("resumeLife", { expectedPolicyVersion: resumeView.profile.policyVersion, lifePermitId });
  const resumed = await poll(() => collect("same-life-resumed", true), value => value.life?.id === lifePermitId && value.life.status === "running" && value.life.activationPending === false);
  assert.equal(resumed.life.expiresAtMs, lifeExpiresAt); assert.equal(resumed.life.childSupplyClaimed, reboot.life.childSupplyClaimed); assert.equal(resumed.life.spentGold, reboot.life.spentGold);
  restartProof = { firstPid, secondPid: actor.pid, differentProcess: true, actualLeaseWaitMs: 31_000, sameMemoryPath: true, sameLifeIdHash: hash(lifePermitId),
    sameOriginalDeadline: true, childSupplyClaimed: reboot.life.childSupplyClaimed, spentGoldBeforeResume: reboot.life.spentGold, modelCallsBeforeResume: reboot.model.modelCalls,
    pausedObservationMs: 5000, noAutomaticNewSpending: true, noAutomaticModelCall: true, ownerExplicitlyResumed: true };
  checkpoints.push({ phase: "restart-resume", ...restartProof }); await closeOwner(); stage("resumed-independent-life-until-real-expiry");
  while (Date.now() < lifeExpiresAt) { await collect("resumed-life-process"); await realWait(2000); }
  stage("observe-real-expiry-and-safe-return");
  const completed = await poll(() => collect("expiry-return", true), value => value.life?.status === "completed" || value.life?.status === "paused", 180_000);
  if (completed.life.status !== "completed") limits.push("The real life paused before safe completion; no full loop success is claimed.");
  assert.equal(completed.life.status, "completed"); assert.equal(completed.life.expired, true); assert.equal(completed.life.expiresAtMs, lifeExpiresAt);
  assert.ok(Date.now() >= lifeExpiresAt); assert.ok(distance(completed.snapshot.player, HOME) <= 1 && completed.snapshot.inSafeZone === true);
  const paid = completed.economic.filter(value => value.committed); assert.equal(paid.length, 1); assert.equal(paid[0].chargedGold, 40); assert.equal(paid[0].quantity, 1);
  assert.equal(paid[0].currency, "gold"); assert.equal(paid[0].source, "trade"); assert.equal(completed.life.spentGold, 40); assert.equal(completed.life.purchasedQuantity, 1);
  assert.ok(completed.life.killsObserved > 0 && participationDeaths > 0, "A real observed own hit followed by death is required; an empty or idle world is not a hunt success.");
  assert.ok(visitedHuntArea && maximumHomeDistance > 1);
  if (modelEnabled) assert.ok([...modelCalls.values()].some(value => value.inputTokens !== null && value.outputTokens !== null), "Configured-model acceptance requires an actual completed provider response with known usage.");
  finalProof = { lifeIdHash: hash(lifePermitId), actualCompletedAtMs: Date.now(), actualElapsedSinceGrantMs: Date.now() - lifeStartedAt, expired: true,
    final: completed.public, actualArrivalDistance: distance(completed.snapshot.player, HOME), observedHits, ownHitThenDeathEvidence: participationDeaths,
    visitedHuntArea, maximumHomeDistance, receiptCount: paid.length, originalDeadlinePreserved: true };
  mainPassed = true;
}

let options;
try {
  options = argumentsFor(process.argv.slice(2));
  if (options.help) process.stdout.write(HELP + "\nReal execution takes about 24–28 minutes; --check performs syntax checking only.\n");
  else {
    const interrupted = () => { stopRequested = true; fatal ??= "operator-stopped"; }; process.once("SIGINT", interrupted); process.once("SIGTERM", interrupted);
    // Reserve a minute for owned-child shutdown. No fixture is allowed to run
    // on indefinitely if an ordinary observation or network helper stalls.
    const deadline = new Promise((_, reject) => { hardTimer = setTimeout(() => { stopRequested = true; fatal = "twenty-nine-minute-watchdog"; reject(new Error("Acceptance time budget reached.")); }, 29 * 60_000); });
    await Promise.race([main(options), deadline]);
  }
} catch (error) {
  const knownErrors = ["A complete idle player trade observation is required.", "Observe one adjacent mutually facing player before requesting a trade.", "A complete current unlocked player trade with a known outcome is required.", "Player trade gold result is unknown; do not replay the additive offer.", "Player trade confirmation is unknown; wait for its final receipt without replaying confirmation.", "A bounded ordinary life observation did not arrive.", "The isolated actor operation failed."];
  failureHint = knownErrors.includes(error?.message) ? error.message : null;
  mainPassed = false;
  failurePhase = phase;
  failureCode = ["ERR_ASSERTION", "COMPANION_RESULT_UNKNOWN", "GATEWAY_RESULT_TIMEOUT", "LIFE_SUPPLY_NOT_CONFIRMED", "LOCAL_MODEL_SETTINGS_UNAVAILABLE", "LOCAL_MODEL_NETWORK_UNAVAILABLE"].includes(error?.code) ? error.code : "UNAVAILABLE";
  if (error?.code === "ERR_ASSERTION") {
    const primitive = value => typeof value === "boolean" || Number.isFinite(value) || value === null ? value : typeof value === "string" && ["ok", "running", "paused", "completed", "cancelled", "gold", "trade", "openai"].includes(value) ? value : null;
    failedAssertion = { operator: text(error.operator, 32), actual: primitive(error.actual), expected: primitive(error.expected) };
  }
  if (error?.code === "ERR_ASSERTION") limits.push("A concrete ordinary gameplay or persistence assertion failed; inspect the bounded checkpoints.");
  else limits.push("The ordinary real-time flow could not complete; no current-phase success is claimed.");
} finally {
  clearTimeout(hardTimer); clearInterval(heartbeat);
  if (!options?.help) {
    const cleanupOkay = await cleanup();
    const closedActors = actorChildren.map(child => ({ pid: child.pid, exited: child.exitCode !== null || child.signalCode !== null }));
    const summary = { schema: "mir2-life-acceptance/1", passed: mainPassed && cleanupOkay && closedActors.every(value => value.exited) && !fatal && !evidenceFailure,
      verificationMode: options?.returnCheckSeconds !== undefined ? "short-ordinary-cancel-return" : "full-real-twenty-minute-expiry",
      realTime: true, fakeClock: false, syntheticPrivateWorld: true, humanTrial: false, durationSeconds: 1200, fatal: text(fatal, 64), failurePhase, failureCode, failureHint, failedAssertion,
      driverPid: process.pid, parentPid: process.ppid, gatewaySha256: gatewayHash ?? null, elapsedWallMs: Date.now() - beganWall,
      modelConfigured: modelEnabled, configuredModel: modelEnabled ? "gpt-6-luna" : "rules", completedProviderCalls: [...modelCalls.values()],
      funding, giftProof, activationProof, restartProof, finalProof, checkpoints, sampledCheckpoints: samples,
      commands, proofCounts, proofDropped, limits,
      scope: options?.returnCheckSeconds !== undefined ? "Ordinary local /ws economy, real away route and explicit cancellation return; this short slice does not claim a twenty-minute expiry or restart." : "Ordinary local /ws economy and real twenty-minute life with one real process restart; no QA/admin grants, artificial combat, clock substitution, Gateway restart or real network-failure claim.",
      cleanup: { attempted: true, returned: cleanupOkay, fixtureClosed, ownerClosed, actors: closedActors, actorPids, originalUserStateTouched: false },
      sourceScripts: [] };
    if (sessionRoot) {
      for (const path of [resolve(repoRoot, "apps/ai-player/scripts/run-life-acceptance.mjs"), resolve(repoRoot, "apps/ai-player/scripts/trial-actor-fixture.mjs"), resolve(repoRoot, "apps/ai-player/scripts/supply-acceptance-funding.mjs")]) {
        try { summary.sourceScripts.push({ path, sha256: await sha256(path) }); } catch { summary.passed = false; }
      }
      const summaryPath = resolve(sessionRoot, "life-acceptance-summary.json");
      await writeFile(summaryPath, JSON.stringify(summary, null, 2) + "\n", { flag: "wx", mode: 0o600 }).catch(() => { summary.passed = false; });
      process.stdout.write(JSON.stringify({ type: summary.passed ? "lifeAcceptancePassed" : "lifeAcceptanceIncomplete", output: sessionRoot, summary: summaryPath, phase: failurePhase ?? phase,
        realTime: true, modelConfigured: modelEnabled, durationSeconds: 1200, actorProcessesExited: closedActors.every(value => value.exited), cleanupReturned: cleanupOkay }) + "\n");
      if (!summary.passed) process.exitCode = 1;
    } else if (!options?.help) { process.stderr.write("The real-time life fixture could not be prepared; no service success is claimed.\n"); process.exitCode = 1; }
    modelEnvironment = {};
  }
}
