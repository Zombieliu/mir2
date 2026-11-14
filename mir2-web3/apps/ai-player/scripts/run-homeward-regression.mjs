// Review before execution. `node --check` only parses this file.
// Ordinary manual outward staging -> existing Controller's cancelled return.
import { randomUUID, createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { appendFileSync, writeFileSync } from "node:fs";
import { mkdir, realpath, stat, writeFile } from "node:fs/promises";
import { dirname, isAbsolute, parse, relative, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { DatabaseSync } from "node:sqlite";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "./companion-trial-fixture.mjs";
import { repoRoot, resolveGatewayExecutable, sha256 } from "./local-gateway.mjs";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";

const HOME = Object.freeze({ x: 288, y: 616 });
const BOUNDS = Object.freeze({ minX: 270, minY: 595, maxX: 310, maxY: 645 });
const TARGETS = Object.freeze([{ x: 288, y: 637 }, { x: 290, y: 631 }, { x: 289, y: 626 }]);
const OFFSETS = { Up: [0, -1], UpRight: [1, -1], Right: [1, 0], DownRight: [1, 1], Down: [0, 1], DownLeft: [-1, 1], Left: [-1, 0], UpLeft: [-1, -1] };
const HELP = "node scripts/run-homeward-regression.mjs --gateway-exe <absolute-new-gateway.exe> --out-root F:/hwr";
const beganWall = Date.now(), beganMono = performance.now();
const WORK_MS = 9 * 60_000, HARD_MS = 10 * 60_000, CADENCE_MS = 650;
const point = value => Number.isSafeInteger(value?.x) && Number.isSafeInteger(value?.y) ? { x: value.x, y: value.y } : null;
const number = value => Number.isSafeInteger(value) && value >= 0 ? value : null;
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const within = (root, path) => { const part = relative(root, path); return part === "" || part !== ".." && !part.startsWith("..\\") && !part.startsWith("../") && !isAbsolute(part); };
const inside = p => point(p) && p.x >= BOUNDS.minX && p.x <= BOUNDS.maxX && p.y >= BOUNDS.minY && p.y <= BOUNDS.maxY;
const hash = value => createHash("sha256").update(String(value)).digest("hex");
const stateWord = value => typeof value === "string" && /^[a-zA-Z][a-zA-Z0-9:_-]{0,80}$/.test(value) ? value : null;

class RegressionFailure extends Error {
  constructor(code) { super(code); this.code = code; }
}
function requireThat(condition, code) { if (!condition) throw new RegressionFailure(code); }
function argumentsFor(args) {
  if (args.length === 1 && args[0] === "--help") return { help: true };
  const value = {}, flags = new Map([["--gateway-exe", "gatewayExecutable"], ["--out-root", "outputRoot"]]);
  for (let index = 0; index < args.length; index++) {
    const key = flags.get(args[index]), next = args[++index];
    requireThat(key && !value[key] && next && isAbsolute(next), "INVALID_ARGUMENTS"); value[key] = resolve(next);
  }
  requireThat(value.gatewayExecutable && value.outputRoot && process.platform === "win32" && process.versions.node === "22.18.0", "WINDOWS_NODE_AND_PATHS_REQUIRED");
  return value;
}
async function privateRoot(path) {
  requireThat(parse(path).root.toLowerCase() === "f:\\" && path !== parse(path).root && !within(repoRoot, path) && !within(path, repoRoot), "DEDICATED_F_ROOT_REQUIRED");
  let ancestor = path;
  for (;;) {
    try { await stat(ancestor); break; } catch (error) { if (error.code !== "ENOENT") throw error; const parent = dirname(ancestor); requireThat(parent !== ancestor, "INVALID_OUTPUT_ANCESTOR"); ancestor = parent; }
  }
  requireThat(parse(await realpath(ancestor)).root.toLowerCase() === "f:\\", "OUTPUT_ANCESTOR_NOT_F");
  await mkdir(path, { recursive: true }); const root = await realpath(path);
  requireThat(parse(root).root.toLowerCase() === "f:\\", "OUTPUT_ROOT_NOT_F"); return root;
}

let fixture, actor, owner, profile, publicRoot, summaryPath, eventPath, phase = "arguments", currentCase = null;
let heartbeat, watchdog, stopRequested = false, fatal = null, latest = null, failed = null, passed = false, cleanupPromise;
let fixtureClosed = false, ownerClosed = false, actorExited = false, eventCount = 0, evidenceFailed = false;
const cases = [], sourceScripts = [], actorChildren = [], commands = {}, proofs = {};
let gatewaySha256 = null, actualModelCalls = null, actualModelEnabled = null;
function guard() {
  requireThat(!stopRequested && !fatal && !evidenceFailed && performance.now() - beganMono < WORK_MS, fatal ?? "WORK_BUDGET_OR_STOP");
  requireThat(Math.abs((Date.now() - beganWall) - (performance.now() - beganMono)) <= 5000, "WALL_CLOCK_CHANGED");
}
function emit(event, stdout = false) {
  const value = { atMs: Date.now(), ...event };
  if (eventPath && eventCount++ < 8192) {
    try { appendFileSync(eventPath, JSON.stringify(value) + "\n", "utf8"); } catch { evidenceFailed = true; }
  }
  if (stdout) process.stdout.write(JSON.stringify(value) + "\n");
}
function stage(next) { phase = next; emit({ type: "homewardPhase", phase, caseIndex: currentCase?.index ?? null }, true); }
function publicSnapshot(state) {
  return { ready: state?.ready === true, mapFileName: state?.mapFileName === "0" ? "0" : null, position: point(state?.player),
    hp: number(state?.playerHp), maxHp: number(state?.playerMaxHp), dead: state?.player?.dead === true || state?.playerHp === 0,
    gold: number(state?.gold), inSafeZone: state?.inSafeZone === true, inventoryCurrent: state?.inventoryStale === false };
}
function publicLife(task) {
  if (!task || typeof task !== "object") return null;
  return { permitHash: typeof task.lifePermitId === "string" ? hash(task.lifePermitId) : null, status: stateWord(task.status), phase: stateWord(task.phase),
    activationPending: task.activationPending === true, startedAtMs: number(task.startedAtMs), expiresAtMs: number(task.expiresAtMs),
    budgetGold: number(task.budgetGold), maxPurchasedQuantity: number(task.maxPurchasedQuantity), chargedGold: number(task.chargedGold),
    purchasedQuantity: number(task.purchasedQuantity), modelDecisionsUsed: number(task.modelDecisionsUsed) };
}
function diagnostic(state) {
  return { position: point(state?.player), entities: (Array.isArray(state?.entities) ? state.entities : []).filter(value =>
    ["selfPlayer", "player", "monster", "npc"].includes(value.kind) && inside(value)).slice(0, 128).map(value => ({
      ...point(value), kind: value.kind, hp: number(value.hp), dead: typeof value.dead === "boolean" ? value.dead : null,
    })) };
}
function onEvent(message) {
  if (message?.type === "command" && /^[A-Za-z][A-Za-z0-9]{0,48}$/.test(message.command ?? "")) {
    const type = message.command; commands[type] = (commands[type] ?? 0) + 1;
    if (/qa|stage5|spawn|teleport|giveItem|passkey|tradeGold|depositTradeItem|npcPurchaseOwner|buyItem|dropGold/i.test(type)) fatal = "UNEXPECTED_PRIVILEGED_OR_ECONOMIC_COMMAND";
  } else if (message?.type === "proof") {
    const packet = message.event?.packet, payload = message.event?.payload;
    if (["UserLocation", "HealthChanged", "UseItem", "ObjectAttack", "ObjectStruck", "ObjectDied"].includes(packet)) proofs[packet] = (proofs[packet] ?? 0) + 1;
    if (packet === "HealthChanged" && payload?.hp === 0 || packet === "ObjectDied" && actor && [actor.state?.playerObjectId, actor.state?.selfZoneObjectId].includes(payload?.objectId)) fatal = "ACTOR_DIED";
  } else if (message?.type === "fixtureStage") emit({ type: "fixtureStage", stage: stateWord(message.stage), ordinal: number(message.ordinal) });
}
async function controls(operation, payload = {}) {
  guard();
  const reply = await owner.companionControl(operation, { residentId: profile.companion.residentId, requestId: `homeward:${randomUUID()}`, payload });
  requireThat(reply.status === "ok", "OWNER_CONTROL_NOT_CONFIRMED"); return reply.result;
}
function ensureAlive(state) {
  requireThat(state?.ready === true && state.mapFileName === "0" && inside(state.player), "ACTOR_OR_SCOPE_UNAVAILABLE");
  requireThat(number(state.playerHp) !== null && number(state.playerMaxHp) > 0, "HEALTH_UNKNOWN");
  requireThat(state.playerHp > 0 && state.player?.dead !== true, "ACTOR_DIED");
}
async function observe(label) {
  guard();
  const [state, view, model] = await Promise.all([actor.request("snapshot"), controls("view"), actor.request("modelStatus")]);
  const task = view?.runtime?.lifeTask;
  actualModelCalls = number(model?.modelCalls); actualModelEnabled = typeof model?.modelEnabled === "boolean" ? model.modelEnabled : null;
  latest = { label, phase, snapshot: publicSnapshot(state), life: publicLife(task), diagnostic: diagnostic(state), modelCalls: number(model?.modelCalls) };
  ensureAlive(state);
  requireThat(model?.provider === "rules" && model?.modelEnabled !== true && number(model?.modelCalls) === 0, "MODEL_MUST_REMAIN_DISABLED");
  if (task?.lifePermitId) {
    requireThat(task.budgetGold === 50 && task.maxPurchasedQuantity === 1 && task.chargedGold === 0 && task.purchasedQuantity === 0 && task.modelDecisionsUsed === 0, "UNEXPECTED_SPENDING_PURCHASE_OR_MODEL_ATTEMPT");
    if (currentCase?.permitId) requireThat(task.lifePermitId === currentCase.permitId && task.startedAtMs === currentCase.startedAtMs && task.expiresAtMs === currentCase.expiresAtMs, "ORIGINAL_CASE_AUTHORITY_CHANGED");
  }
  return { state, view, task };
}
async function cadence() { guard(); await sleep(CADENCE_MS); guard(); }

async function healthBeforeStaging(state) {
  ensureAlive(state);
  if (state.playerHp / state.playerMaxHp >= 0.65) return state;
  // One genuine Controller action can consume its own potion or withdraw.
  // Do not keep staging outward while timed healing is still unobserved.
  const action = await actor.request("tick");
  currentCase.safetyTicks++;
  emit({ type: "stagingSafetyTick", caseIndex: currentCase.index, kind: stateWord(action?.kind), reason: stateWord(action?.reason) });
  for (let attempt = 0; attempt < 6; attempt++) {
    await cadence(); const actual = await observe("staging-safety-observation"); ensureAlive(actual.state);
    requireThat(actual.task?.status !== "paused", "LIFE_PAUSED_DURING_SAFETY");
    if (actual.state.playerHp / actual.state.playerMaxHp >= 0.65) return actual.state;
  }
  throw new RegressionFailure("SAFETY_INTERRUPTED_STAGING");
}
async function stageActor(collision, target) {
  const until = performance.now() + 100_000, rejected = new Map(); let noPath = 0;
  while (performance.now() < until) {
    const observed = await observe("manual-ordinary-staging");
    requireThat(observed.task?.status === "running" && observed.task?.activationPending === false, "LIFE_NOT_ACTIVE_DURING_STAGING");
    const state = await healthBeforeStaging(observed.state);
    if (distance(state.player, target) === 0) { currentCase.staged = publicSnapshot(state); currentCase.stagingDiagnostic = diagnostic(state); return; }
    const now = performance.now(); for (const [key, expiry] of rejected) if (expiry <= now) rejected.delete(key);
    const occupied = state.entities.filter(value => value.objectId !== state.playerObjectId && value.objectId !== state.selfZoneObjectId &&
      ["player", "monster", "npc"].includes(value.kind) && (value.kind === "npc" || value.dead !== true && value.hp !== 0)).map(point).filter(Boolean);
    for (const key of rejected.keys()) { const [x, y] = key.split(",").map(Number); occupied.push({ x, y }); }
    const direction = nextWalkDirection({ player: state.player, target, desiredDistance: 0, collision, occupied, movementBounds: BOUNDS });
    if (!direction) { requireThat(++noPath <= 12, "STAGING_NO_OBSERVED_PATH"); await cadence(); continue; }
    noPath = 0;
    const from = point(state.player), [dx, dy] = OFFSETS[direction];
    const result = await actor.request("walk", { direction });
    currentCase.manualWalks++;
    const actual = await actor.request("snapshot"); ensureAlive(actual);
    const moved = point(actual.player).x !== from.x || point(actual.player).y !== from.y;
    emit({ type: "manualStagingWalk", caseIndex: currentCase.index, direction, from, actual: point(actual.player), moved, acknowledgedMoved: result?.moved === true });
    if (!moved) {
      const key = `${from.x + dx},${from.y + dy}`; rejected.delete(key); rejected.set(key, performance.now() + 3000);
      if (rejected.size > 64) rejected.delete(rejected.keys().next().value);
    }
    await cadence();
  }
  throw new RegressionFailure("STAGING_TIMEOUT");
}
async function controllerReturn() {
  const until = performance.now() + 90_000;
  while (performance.now() < until) {
    // After cancellation the driver never calls walk: only the existing
    // Controller chooses all movement, medicine and safety actions.
    const action = await actor.request("tick"); currentCase.returnTicks++;
    const actual = await observe("controller-cancelled-homeward");
    emit({ type: "controllerReturnTick", caseIndex: currentCase.index, kind: stateWord(action?.kind), reason: stateWord(action?.reason), snapshot: publicSnapshot(actual.state), life: publicLife(actual.task) });
    requireThat(actual.task?.status !== "paused", "RETURN_PAUSED");
    if (actual.task?.status === "cancelled" && distance(actual.state.player, HOME) <= 1 && actual.state.inSafeZone === true) {
      currentCase.final = publicSnapshot(actual.state); currentCase.finalLife = publicLife(actual.task); currentCase.arrivalDistance = distance(actual.state.player, HOME); return;
    }
    await cadence();
  }
  throw new RegressionFailure("CONTROLLER_RETURN_TIMEOUT");
}
function durableCases() {
  const path = `${profile.companion.memoryPath}.life.sqlite`;
  requireThat(within(fixture.output, path), "EXECUTION_JOURNAL_OUTSIDE_FIXTURE");
  const db = new DatabaseSync(path, { readOnly: true });
  try {
    db.exec("PRAGMA busy_timeout=3000");
    const read = db.prepare("SELECT grant_json,task_json FROM life_tasks WHERE permit_id=?");
    return cases.map(value => {
      const row = read.get(value.permitId); requireThat(Boolean(row), "OLD_CANCELLED_EXECUTION_MISSING");
      const grant = JSON.parse(row.grant_json), task = JSON.parse(row.task_json);
      requireThat(grant.expiresAtMs === value.expiresAtMs && grant.budgetGold === 50 && task.status === "cancelled" && task.spentGold === 0 && task.purchasedQuantity === 0 && task.modelDecisionsUsed === 0, "CANCELLED_EXECUTION_CHANGED");
      return { caseIndex: value.index, permitHash: hash(value.permitId), expiresAtMs: grant.expiresAtMs, status: "cancelled", chargedGold: 0, purchasedQuantity: 0, modelDecisionsUsed: 0 };
    });
  } finally { db.close(); }
}
let durableProof = [];
async function main(options) {
  const root = await privateRoot(options.outputRoot), runRoot = resolve(root, `hw-${randomUUID().replace(/-/g, "").slice(0, 10)}`);
  const privateOutput = resolve(runRoot, "private"); publicRoot = resolve(runRoot, "public");
  requireThat(resolve(privateOutput, "trial-bootstrap-00000000-0000-0000-0000-000000000000", `supply-server.journal.json.initialized.tmp.${"0".repeat(64)}`).length <= 240, "USE_SHORTER_F_OUTPUT");
  await mkdir(publicRoot, { recursive: true }); await mkdir(privateOutput);
  summaryPath = resolve(publicRoot, "homeward-regression-summary.json"); eventPath = resolve(publicRoot, "homeward-events.jsonl");
  await writeFile(eventPath, "", { flag: "wx", mode: 0o600 });
  heartbeat = setInterval(() => emit({ type: "homewardProgress", phase, elapsedSeconds: Math.floor((performance.now() - beganMono) / 1000), caseIndex: currentCase?.index ?? null,
    latest: latest && { snapshot: latest.snapshot, life: latest.life } }, true), 30_000);
  // The deadline owns only this script's child tree. Normal shutdown starts
  // one minute earlier; this final fallback prevents orphaned bootstrap/GW
  // children even if a helper is still awaiting admission when time expires.
  watchdog = setTimeout(() => {
    fatal = "TEN_MINUTE_HARD_DEADLINE"; stopRequested = true;
    try { writeFileSync(summaryPath, JSON.stringify(summary(false, { forcedOwnedTreeShutdown: true }), null, 2) + "\n", { mode: 0o600 }); } catch { /* no success claim */ }
    process.stdout.write(JSON.stringify({ type: "homewardHardDeadline", passed: false, summary: summaryPath }) + "\n");
    spawnSync("taskkill.exe", ["/PID", String(process.pid), "/T", "/F"], { windowsHide: true, stdio: "ignore", timeout: 2000 });
    process.exit(1);
  }, Math.max(1, HARD_MS - (performance.now() - beganMono)));
  const gatewayExecutable = await resolveGatewayExecutable(options.gatewayExecutable); gatewaySha256 = await sha256(gatewayExecutable);
  for (const file of ["scripts/run-homeward-regression.mjs", "scripts/companion-trial-fixture.mjs", "scripts/trial-actor-fixture.mjs", "scripts/local-gateway.mjs", "src/skills/navigation.mjs", "src/companion-controller.mjs", "src/life/life-controller.mjs"]) {
    sourceScripts.push({ path: `apps/ai-player/${file}`, sha256: await sha256(resolve(repoRoot, "apps/ai-player", file)) });
  }
  stage("ordinary-fresh-bootstrap"); fixture = await createTrialFixture({ gatewayExecutable, outputRoot: privateOutput, onEvent }); guard();
  requireThat(within(privateOutput, fixture.output) && within(privateOutput, fixture.bootstrapOutput), "FIXTURE_OUTPUT_ESCAPED");
  profile = fixture.profileA;
  requireThat(profile.runtimeService.provider === "rules" && profile.runtimeService.model === "rules" && profile.runtimeService.credentialRef === null, "RULES_PROFILE_REQUIRED");
  actor = await fixture.startActor(profile, { run: false }); actorChildren.push(actor.child); guard();
  owner = await fixture.newOwnerClient();
  const initial = await observe("initial-zero-wallet"); requireThat(initial.state.gold === 0, "INITIAL_WALLET_MUST_BE_ZERO");
  const collision = await loadClientCollision("0"), permitIds = new Set();
  for (const [index, target] of TARGETS.entries()) {
    currentCase = { index: index + 1, target: { ...target }, passed: false, manualWalks: 0, safetyTicks: 0, returnTicks: 0 }; cases.push(currentCase);
    stage("new-owner-authorized-life");
    const before = await observe("before-new-authorization");
    if (index > 0) requireThat(before.task?.status === "cancelled", "PREVIOUS_LIFE_NOT_CANCELLED");
    requireThat(Number.isSafeInteger(before.view?.profile?.policyVersion), "CURRENT_POLICY_UNAVAILABLE");
    const authorized = await controls("authorizeLife", { expectedPolicyVersion: before.view.profile.policyVersion, budgetGold: 50, durationSeconds: 1200 });
    const task = authorized?.runtime?.lifeTask;
    requireThat(typeof task?.lifePermitId === "string" && !permitIds.has(task.lifePermitId) && task.durationSeconds === 1200 && task.expiresAtMs - task.startedAtMs === 1_200_000, "NEW_FIXED_LIFE_AUTHORITY_REQUIRED");
    permitIds.add(task.lifePermitId); Object.assign(currentCase, { permitId: task.lifePermitId, startedAtMs: task.startedAtMs, expiresAtMs: task.expiresAtMs });
    const activationUntil = performance.now() + 25_000;
    for (;;) {
      const active = await observe("awaiting-server-activation");
      if (active.task?.status === "running" && active.task.activationPending === false) break;
      requireThat(performance.now() < activationUntil && active.task?.status !== "paused", "SERVER_ACTIVATION_NOT_OBSERVED"); await cadence();
    }
    stage("manual-ordinary-outward-staging"); await stageActor(collision, target);
    requireThat(distance(currentCase.staged.position, target) === 0, "STAGING_POINT_NOT_REACHED");
    stage("owner-cancel-life"); await controls("cancelLife");
    stage("controller-only-homeward"); await controllerReturn();
    currentCase.passed = true; emit({ type: "homewardCasePassed", caseIndex: currentCase.index, target, final: currentCase.final, life: currentCase.finalLife }, true);
  }
  durableProof = durableCases(); passed = true;
}
async function cleanup() {
  if (cleanupPromise) return cleanupPromise;
  cleanupPromise = (async () => {
    let okay = true; phase = "cleanup";
    try { await fixture?.stopActor(); } catch { okay = false; }
    actorExited = actorChildren.every(child => child.exitCode !== null || child.signalCode !== null);
    try { if (owner) { if (owner.snapshot().ready) await owner.logout(); await owner.close(); ownerClosed = true; } } catch { okay = false; try { await owner?.close(); } catch {} }
    try { if (fixture) { await fixture.close(); fixtureClosed = true; } } catch { okay = false; }
    return okay && actorExited;
  })();
  return cleanupPromise;
}
function summary(cleanupOkay, extra = {}) {
  return { schema: "mir2-homeward-regression/1", passed: passed && cleanupOkay && !fatal && !failed && !evidenceFailed, verificationMode: "ordinary-manual-staging-controller-cancel-return",
    realTime: true, fakeClock: false, syntheticPrivateWorld: true, humanTrial: false, automaticOutwardAdventureClaimed: false, fullTwentyMinuteExpiryClaimed: false,
    rulesOnlyRequired: true, modelConfigured: actualModelEnabled, modelCalls: actualModelCalls, fundingInjected: false, gatewaySha256, elapsedWallMs: Date.now() - beganWall, maximumMinutes: 10,
    failure: failed ?? (fatal ? { phase, code: fatal } : null), cases: cases.map(({ permitId, ...value }) => ({ ...value, permitHash: permitId ? hash(permitId) : null })),
    durableCancelledExecutions: durableProof, lastObserved: latest, commands, proofCounts: proofs, sourceScripts,
    scope: "Ordinary Actor walk staging inside Bichon bounds, then only existing Controller ticks after owner cancellation. No funding, GM, teleport, fabricated clock, model call, autonomous outward-trip, expiry, restart or full-world claim.",
    cleanup: { returned: cleanupOkay, fixtureClosed, ownerClosed, actorExited, ownedActorProcesses: actorChildren.map(child => ({ pid: child.pid, exited: child.exitCode !== null || child.signalCode !== null })), originalUserStateTouched: false }, ...extra };
}

let options;
try {
  options = argumentsFor(process.argv.slice(2));
  if (options.help) process.stdout.write(HELP + "\nReview first; real execution is bounded to ten minutes. --check starts nothing.\n");
  else {
    const interrupted = () => { stopRequested = true; fatal ??= "OPERATOR_STOPPED"; };
    process.once("SIGINT", interrupted); process.once("SIGTERM", interrupted);
    await main(options);
  }
} catch (error) {
  passed = false; failed = { phase, caseIndex: currentCase?.index ?? null, code: error instanceof RegressionFailure ? error.code : "ORDINARY_OPERATION_UNAVAILABLE" };
  if (currentCase) { currentCase.passed = false; currentCase.failure = { ...failed, lastObserved: latest }; }
} finally {
  if (!options?.help) {
    clearInterval(heartbeat);
    const cleaned = await cleanup(), outcome = summary(cleaned);
    try {
      if (summaryPath) await writeFile(summaryPath, JSON.stringify(outcome, null, 2) + "\n", { flag: "wx", mode: 0o600 });
      else outcome.passed = false;
    } catch { outcome.passed = false; }
    clearTimeout(watchdog);
    process.stdout.write(JSON.stringify({ type: outcome.passed ? "homewardRegressionPassed" : "homewardRegressionIncomplete", passed: outcome.passed,
      summary: summaryPath ?? null, caseCount: cases.length, failure: failed ?? fatal, cleanupReturned: cleaned, fullTwentyMinuteExpiryClaimed: false }) + "\n");
    if (!outcome.passed) process.exitCode = 1;
  }
}
