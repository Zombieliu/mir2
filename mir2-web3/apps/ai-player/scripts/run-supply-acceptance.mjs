// Actual local /ws economy and separate-process recovery. No model calls.
import assert from "node:assert/strict";
import { fork } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, writeFile } from "node:fs/promises";
import { isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "./companion-trial-fixture.mjs";
import { isolatedEnvironment, resolveGatewayExecutable, repoRoot, sha256 } from "./local-gateway.mjs";
import { fundSyntheticSupplyActor } from "./supply-acceptance-funding.mjs";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";

const safe = value => JSON.parse(JSON.stringify(value, (_key, item) => typeof item === "bigint" ? item.toString() : item));
const hash = value => createHash("sha256").update(JSON.stringify(safe(value))).digest("hex");
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const inside = (root, path) => { const p = relative(root, path); return p !== ".." && !p.startsWith(`..\\`) && !p.startsWith("../") && !isAbsolute(p); };
const diagnosticPrimitive = value => value === null || typeof value === "boolean" || typeof value === "number" && Number.isFinite(value)
  || typeof value === "string" && ["ok", "failed", "paused", "completed", "authorized", "running", "returning", "cancelled", "unknown", "idle", "gold", "trade",
    "travel", "open-shop", "select-shop", "begin", "quote", "ready-to-purchase", "purchase-pending", "reconcile", "return", "awaiting-new-authorization", "rejected"].includes(value) ? value : undefined;
const assertionDiagnostic = error => error?.code === "ERR_ASSERTION" ? {
  operator: ["strictEqual", "notStrictEqual", "deepStrictEqual", "==", "!=", "notDeepStrictEqual"].includes(error.operator) ? error.operator : "assert",
  actual: diagnosticPrimitive(error.actual), expected: diagnosticPrimitive(error.expected),
} : undefined;
const options = {};
for (let i = 2; i < process.argv.length; i += 2) {
  const field = { "--gateway-exe": "gatewayExecutable", "--out-root": "outputRoot" }[process.argv[i]];
  if (!field || !process.argv[i + 1] || options[field]) throw new Error("Use --gateway-exe <new local executable> --out-root <private output directory>.");
  options[field] = resolve(process.argv[i + 1]);
}
if (!options.gatewayExecutable || !options.outputRoot || process.versions.node !== "22.18.0") {
  throw new Error("Supply acceptance requires explicit local paths and Node.js 22.18.0.");
}
if (inside(repoRoot, options.outputRoot) || inside(options.outputRoot, repoRoot)) throw new Error("Keep private supply acceptance outputs outside the source tree.");

const checks = [], events = [], purchases = [], commands = {};
let fixture, actor, owner, phase = "prepare", reportPath, funding, budgetRefusal, positive, positiveObservation, cancellation, recovery;
const checkpoint = name => { phase = name; console.log(JSON.stringify({ type: "supplyAcceptancePhase", phase: name })); };
const poll = async (read, accept, label, timeoutMs = 40_000) => {
  const until = Date.now() + timeoutMs;
  while (Date.now() < until) {
    if (events.some(e => e.type === "supplyActorSafetyStopped")) throw new Error("The acceptance stopped on a real character injury.");
    const value = await read(); if (accept(value)) return value; await sleep(200);
  }
  throw new Error(`Bounded supply acceptance observation unavailable: ${label}`);
};

function launchActor(profile) {
  const child = fork(fileURLToPath(new URL("./supply-acceptance-actor.mjs", import.meta.url)), [], {
    cwd: repoRoot, windowsHide: true, execArgv: [], serialization: "advanced", stdio: ["ignore", "ignore", "ignore", "ipc"],
    env: isolatedEnvironment({ MIR2_AI_PLAYER_ACCOUNT: fixture.actor.accountId, MIR2_AI_PLAYER_PASSWORD: fixture.actor.password,
      MIR2_COMPANION_ARCHIVE_KEY: fixture.archiveKey }),
  });
  const pending = new Map(); let sequence = 0, stopped = false;
  const exited = new Promise(done => {
    const finish = code => {
      if (stopped) return;
      stopped = true;
      for (const waiter of pending.values()) { clearTimeout(waiter.timer); waiter.reject(new Error("Private supply actor exited or could not start.")); }
      pending.clear(); done(code);
    };
    child.once("exit", finish);
    child.once("error", () => finish(-1));
  });
  child.on("message", message => {
    if (message.type === "result") {
      const waiter = pending.get(message.id); if (!waiter) return;
      clearTimeout(waiter.timer); pending.delete(message.id);
      message.error ? waiter.reject(new Error("Private supply actor operation failed.")) : waiter.resolve(message.result); return;
    }
    if (message.type === "supplyActorCommand") { commands[message.command] = (commands[message.command] ?? 0) + 1; return; }
    if (message.type === "supplyPurchaseBoundary") purchases.push(safe(message.proof));
    if (events.length < 100) events.push(safe(message));
  });
  const request = (command, fields = {}) => new Promise((done, reject) => {
    if (stopped || !child.connected || pending.size >= 8) return reject(new Error("Private supply actor is unavailable."));
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error("Private supply actor request timed out.")); }, 35_000);
    pending.set(id, { resolve: done, reject, timer });
    child.send({ id, command, ...fields }, error => {
      if (!error || !pending.has(id)) return; clearTimeout(timer); pending.delete(id); reject(new Error("Private supply actor IPC failed."));
    });
  });
  return { pid: child.pid, request, start: () => request("start", { profile }), async close() {
    if (stopped) return;
    try { await request("stop"); } catch { /* terminate only this owned helper if graceful stop fails */ }
    await Promise.race([exited, sleep(8000)]);
    if (!stopped) { child.kill(); await exited; }
  } };
}

async function walkOwnerOutOfActorView(collision) {
  for (const target of [{ x: 309, y: 634 }, { x: 271, y: 634 }, { x: 309, y: 596 }, { x: 271, y: 596 }]) {
    if (collision.blocked.has(`${target.x},${target.y}`)) continue;
    for (let step = 0; step < 140; step++) {
      const state = owner.snapshot();
      if (state.ready !== true || state.mapFileName !== "0" || !state.player || state.player.dead === true
          || !Number.isSafeInteger(state.playerHp) || !Number.isSafeInteger(state.playerMaxHp)
          || state.playerMaxHp <= 0 || state.playerHp !== state.playerMaxHp) {
        throw new Error("The ordinary owner lacks a current healthy Bichon observation.");
      }
      if (distance(state.player, target) === 0) break;
      const occupied = state.entities.filter(e => e.objectId !== state.playerObjectId && e.dead !== true).map(e => ({ x: e.x, y: e.y }));
      const direction = nextWalkDirection({ player: state.player, target, desiredDistance: 0, collision, occupied,
        movementBounds: { minX: 270, minY: 595, maxX: 310, maxY: 635 } });
      if (!direction) break;
      await owner.walk(direction); await sleep(650);
    }
    await sleep(500);
    const observed = await actor.request("snapshot");
    if (!observed.entities.some(e => e.kind === "player" && e.name === fixture.owner.characterName)) {
      return { ownerPosition: { x: owner.snapshot().player.x, y: owner.snapshot().player.y }, actorPosition: observed.player, actorCouldSeeOwner: false };
    }
  }
  throw new Error("No ordinary safe observation position placed the owner outside the actor's actual view.");
}

try {
  await mkdir(options.outputRoot, { recursive: true });
  const gatewayExecutable = await resolveGatewayExecutable(options.gatewayExecutable);
  checkpoint("create-ordinary-private-world");
  fixture = await createTrialFixture({ gatewayExecutable, outputRoot: options.outputRoot });
  assert.ok(inside(options.outputRoot, fixture.output) && relative(options.outputRoot, fixture.output) !== "",
    "The new fixture must honor its external output root and use a distinct run directory.");
  reportPath = resolve(fixture.output, "supply-acceptance-summary.json");
  const collision = await loadClientCollision("0");
  checkpoint("ordinary-owned-starter-resale");
  funding = await fundSyntheticSupplyActor({ url: fixture.gateway.url, identity: fixture.actor,
    tracePath: resolve(fixture.output, "starter-resale-private.jsonl"), collision, checkpoint });
  checks.push("synthetic actor ordinarily sold its own starter sword/dress; exact owned removal and real gold credit; HP potion and candle preserved");
  checkpoint("ordinary-actor-logout-lease-settle"); await sleep(31_000);
  actor = launchActor(fixture.profileA); await actor.start();
  owner = await fixture.newOwnerClient();
  const control = (operation, payload = {}, requestId) => owner.companionControl(operation,
    { residentId: fixture.profileA.companion.residentId, payload, ...(requestId ? { requestId } : {}) });
  const view = async () => { const result = await control("view"); assert.equal(result.status, "ok"); return result.result; };
  checkpoint("owner-ordinary-walk-outside-actual-actor-view");
  const visibility = await walkOwnerOutOfActorView(collision);
  const anchor = { ...visibility.ownerPosition };
  checks.push("owner walked outside the actor's actual AOI before authorization and remains on the authorized return anchor");

  checkpoint("reject-client-budget-override");
  let current = await view();
  const rejected = await control("authorizeSupply", { expectedPolicyVersion: current.profile.policyVersion, budgetGold: 51, quantity: 1, expiresInSeconds: 300 });
  assert.notEqual(rejected.status, "ok");
  assert.equal((await actor.request("audit")).calls.purchase, 0);
  checks.push("a 51-gold authorization above the server cap was rejected without a purchase");

  checkpoint("reject-observed-price-above-39-gold-budget");
  current = await view(); const lowBudgetBefore = await actor.request("snapshot"), lowBudgetAuditBefore = await actor.request("audit");
  const lowBudget = await control("authorizeSupply", { expectedPolicyVersion: current.profile.policyVersion, budgetGold: 39, quantity: 1, expiresInSeconds: 300 });
  assert.equal(lowBudget.status, "ok", "A lower server budget is valid; the observed 40-gold good must still be refused.");
  const lowBudgetResult = await poll(view, value => ["failed", "paused", "completed"].includes(value.runtime.supplyTask?.status),
    "current Ruben price above the smaller grant", 180_000);
  assert.equal(lowBudgetResult.runtime.supplyTask.status, "failed");
  assert.equal(lowBudgetResult.runtime.supplyTask.budgetGold, 39);
  const lowBudgetAfter = await actor.request("snapshot"), lowBudgetAuditAfter = await actor.request("audit");
  assert.equal(lowBudgetAfter.observedPotionGood?.price, 40, "The refusal must follow an actual observed merchant price.");
  assert.equal(lowBudgetAfter.gold, lowBudgetBefore.gold);
  assert.equal(lowBudgetAfter.hpPotions, lowBudgetBefore.hpPotions);
  assert.equal(lowBudgetAuditAfter.calls.purchase, lowBudgetAuditBefore.calls.purchase);
  assert.equal(lowBudgetResult.runtime.supplyTask.receiptKnown, false);
  assert.equal(lowBudgetResult.runtime.supplyTask.acquiredQuantity, 0);
  budgetRefusal = { budgetGold: 39, observedPriceGold: lowBudgetAfter.observedPotionGood.price,
    goldUnchanged: true, ownedPotionQuantityUnchanged: true, noPurchaseAttempted: true,
    actualStatus: lowBudgetResult.runtime.supplyTask.status,
    scope: "observed 40-gold price exceeded the grant and the runner stopped without purchase; the unique refusal cause and server commit-budget enforcement are not established by this observation" };
  checks.push("valid 39-gold grant with observed 40-gold Ruben potion stopped before purchase, with unchanged gold and owned potion quantity; unique refusal cause not inferred");
  assert.equal((await control("cancelSupply", {})).status, "ok");

  checkpoint("authorize-independent-one-potion");
  current = await view(); const before = await actor.request("snapshot");
  const authorized = await control("authorizeSupply", { expectedPolicyVersion: current.profile.policyVersion, budgetGold: 50, quantity: 1, expiresInSeconds: 300 });
  assert.equal(authorized.status, "ok");
  assert.ok(authorized.result.runtime.supplyTask?.permitId, "Only a real server grant starts this task.");
  const completed = await poll(view, value => ["completed", "failed", "paused"].includes(value.runtime.supplyTask?.status), "actual purchase and anchor return", 220_000);
  positiveObservation = Object.fromEntries(["status", "step", "budgetGold", "chargedGold", "acquiredQuantity", "receiptKnown", "returnedToAnchor"]
    .map(field => [field, diagnosticPrimitive(completed.runtime.supplyTask[field])]));
  assert.equal(completed.runtime.supplyTask.status, "completed");
  const final = await actor.request("snapshot"), audit = await actor.request("audit");
  assert.equal(audit.calls.purchase, 1); assert.equal(audit.unsafeObserved, false);
  const first = audit.purchases[0], receipt = first.receipt?.entry?.outcome?.committed;
  assert.ok(receipt && first.hasCompleteSnapshot);
  assert.equal(receipt.currency, "gold"); assert.equal(receipt.source, "trade");
  assert.equal(receipt.charged, 40); assert.equal(receipt.admittedCount, 1);
  assert.equal(first.before.gold - first.after.gold, 40); assert.equal(first.after.hpPotions - first.before.hpPotions, 1);
  assert.equal(before.gold - final.gold, 40);
  assert.equal(completed.runtime.supplyTask.chargedGold, 40);
  assert.equal(completed.runtime.supplyTask.acquiredQuantity, 1);
  assert.equal(completed.runtime.supplyTask.receiptKnown, true);
  assert.equal(completed.runtime.supplyTask.returnedToAnchor, true);
  assert.equal(distance(final.player, anchor), 1, "The owner occupies the return anchor; the actor must arrive adjacent, never stack onto it.");
  positive = { visibility, anchor, actualPosition: final.player, arrivalDistance: distance(final.player, anchor),
    beforeGold: before.gold, afterGold: final.gold, chargedGold: 40, acquiredQuantity: 1,
    purchaseReceipt: first.receipt, authority: first.authority, operationHash: hash(first.operation),
    completePairedSnapshot: first.hasCompleteSnapshot, independentActualMovement: (commands.walk ?? 0) > 0 };
  checks.push("actual independent Ruben interaction, one exact 40-gold/one-potion committed receipt with same-source full snapshot and real arrival beside occupied owner anchor");

  checkpoint("revoke-before-new-purchase");
  current = await view(); const beforeCancel = await actor.request("snapshot"), beforeCancelAudit = await actor.request("audit");
  const next = await control("authorizeSupply", { expectedPolicyVersion: current.profile.policyVersion, budgetGold: 50, quantity: 1, expiresInSeconds: 300 });
  assert.equal(next.status, "ok");
  const revoked = await control("cancelSupply", {}); assert.equal(revoked.status, "ok");
  await sleep(1800);
  const afterCancel = await actor.request("snapshot"), afterCancelAudit = await actor.request("audit");
  assert.equal(afterCancel.gold, beforeCancel.gold); assert.equal(afterCancelAudit.calls.purchase, beforeCancelAudit.calls.purchase);
  cancellation = { revoked: true, goldUnchanged: true, noAdditionalPurchase: true, actualStatus: (await view()).runtime.supplyTask.status };
  checks.push("explicit server cancellation prevented a new economic operation and any further gold debit");

  checkpoint("real-second-purchase-with-explicit-result-loss-injection");
  assert.equal((await actor.request("armDropCommittedResult")).armed, true);
  current = await view(); const recoveryStartGold = (await actor.request("snapshot")).gold;
  const faultTask = await control("authorizeSupply", { expectedPolicyVersion: current.profile.policyVersion, budgetGold: 50, quantity: 1, expiresInSeconds: 300 });
  assert.equal(faultTask.status, "ok");
  await poll(() => actor.request("audit"), value => value.dropInjected, "actual committed method-result loss", 180_000);
  const lostAudit = await actor.request("audit"), originalOperation = lostAudit.operations.at(-1);
  assert.equal(lostAudit.calls.purchase, 2); assert.equal(lostAudit.calls.query, beforeCancelAudit.calls.query);
  const lostPurchase = lostAudit.purchases.at(-1);
  assert.equal(lostPurchase.receipt.entry.outcome.committed.charged, 40);
  assert.equal(lostPurchase.before.gold - lostPurchase.after.gold, 40);
  const unknownView = await view(); assert.equal(unknownView.runtime.supplyTask.status, "unknown");
  const firstPid = actor.pid; await actor.close(); actor = null;
  checkpoint("different-node-process-recovery-lease-settle"); await sleep(31_000);
  actor = launchActor(fixture.profileA); await actor.start(); assert.notEqual(actor.pid, firstPid);
  const recovered = await poll(view, value => value.runtime.supplyTask?.receiptKnown === true && value.runtime.supplyTask?.acquiredQuantity === 1,
    "original durable receipt after process restart", 45_000);
  const recoveredAudit = await actor.request("audit"), recoveredSnapshot = await actor.request("snapshot");
  assert.equal(recoveredAudit.calls.purchase, 0); assert.ok(recoveredAudit.calls.query >= 1);
  for (const query of recoveredAudit.queries) assert.deepEqual(query, originalOperation);
  assert.equal(recoveredSnapshot.gold, recoveryStartGold - 40);
  assert.equal(recovered.runtime.supplyTask.status, "paused");
  recovery = { differentProcess: true, firstProcessId: firstPid, restartedProcessId: actor.pid,
    injection: "real server commit; returned method result deliberately dropped before runner consumed it", originalOperationHash: hash(originalOperation),
    actualReceipt: lostPurchase.receipt, recoveredReceiptRevision: recovered.runtime.supplyTask.receiptRevision,
    queryCalls: recoveredAudit.calls.query, newPurchaseCallsAfterRestart: 0, chargedOnlyOnce: true,
    stateAfterRecovery: recovered.runtime.supplyTask.status, returnClaimedAfterRecovery: false };
  checks.push("different Node process recovered the exact original durable order by Query only; no new purchase, no second debit, paused rather than fabricated return completion");

  const summary = { schema: "mir2-supply-acceptance/1", passed: true, modelCalls: 0, humanTrial: false,
    gatewaySha256: await sha256(fixture.gateway.executable), contentProfile: fixture.gateway.contentProfile,
    funding, budgetRefusal, positive, cancellation, recovery, checks, commands, economicBoundaryProofs: purchases, unknownFaultIsExplicitInjection: true };
  await writeFile(reportPath, JSON.stringify(safe(summary), null, 2) + "\n", { flag: "wx" });
  console.log(JSON.stringify({ type: "supplyAcceptancePassed", output: fixture.output, summary: reportPath, checks: checks.length, modelCalls: 0 }));
} catch (error) {
  const failure = { schema: "mir2-supply-acceptance/1", passed: false, phase, checks, modelCalls: 0, humanTrial: false,
    funding, budgetRefusal, positive, positiveObservation, cancellation, recovery, commands, economicBoundaryProofs: purchases,
    assertion: assertionDiagnostic(error),
    failure: "A bounded acceptance invariant failed; no success is claimed for the current phase." };
  if (fixture) await writeFile(resolve(fixture.output, `supply-acceptance-failure-${randomUUID()}.json`), JSON.stringify(safe(failure), null, 2) + "\n").catch(() => {});
  console.error(JSON.stringify({ type: "supplyAcceptanceFailed", phase, output: fixture?.output ?? null })); process.exitCode = 1;
} finally {
  await actor?.close().catch(() => {});
  await owner?.close().catch(() => {});
  await fixture?.close().catch(() => {});
}
