// Real local gateway/gameplay. The default provider is explicitly a test double;
// --live-openai reads the user's local vault and makes ONE bounded plan request.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { createTrialFixture } from "./companion-trial-fixture.mjs";
import { sha256, gatewayExecutableArgument } from "./local-gateway.mjs";
import { ResidentActor } from "../src/resident-actor.mjs";
import { loadLocalModelSettings } from "../src/local-model-settings.mjs";
import { buildModelContext } from "../src/model/memory-context.mjs";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";

const args = process.argv.slice(2), live = args.includes("--live-openai");
const liveIndex = args.indexOf("--live-openai");
let localSettings;
if (live) {
  if (!args[liveIndex + 1]) throw new Error("Pass the exact local bundle directory after --live-openai.");
  localSettings = await loadLocalModelSettings({ bundleDir: resolve(args[liveIndex + 1]) });
  args.splice(liveIndex, 2);
  if (localSettings.provider !== "openai") throw new Error("Configure the local model before a live model acceptance run.");
}
const gatewayExecutable = gatewayExecutableArgument(args), checks = [], commands = {}, evidence = [];
const priorKey = process.env.OPENAI_API_KEY, priorArchiveKey = process.env.MIR2_COMPANION_ARCHIVE_KEY;
let fixture, actor, owner, phase = "prepare", providerCalls = 0, modelCallProof = null;
const distance = (a, b) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));
const note = value => console.log(JSON.stringify({ type: "modelAcceptancePhase", phase: value }));
const poll = async (read, accept, label, timeout = 25_000) => {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) { const value = await read(); if (accept(value)) return value; await sleep(150); }
  throw new Error(`Acceptance observation unavailable: ${label}`);
};
try {
  fixture = await createTrialFixture({ gatewayExecutable });
  process.env.MIR2_COMPANION_ARCHIVE_KEY = fixture.archiveKey;
  process.env.OPENAI_API_KEY = live ? localSettings.apiKey : "sk-synthetic-network-test-only";
  const profile = structuredClone(fixture.profileA);
  Object.assign(profile.runtimeService, { provider: "openai", model: live ? localSettings.model : "gpt-6-luna", credentialRef: "OPENAI_API_KEY", promptVersion: "alo-objective-v2", maxCallsPerHour: 20 });
  const mockProvider = { async plan({ context }) {
    providerCalls++;
    await sleep(1000);
    return { plan: { reply: "我会在你身边寻找两只普通怪物，先支援，途中不拾取，完成后回来。", objective: { kind: "hunt", monsterName: null, count: 2, maxDistanceFromOwner: 6, loot: "never", returnToOwner: true }, rememberedEpisodeIds: context.memories.filter(m => m.kind === "owner-instruction").slice(0, 1).map(m => m.episodeId) },
      call: { model: "gpt-6-luna", status: "completed", requestId: "mock-network-only", inputTokens: 0, outputTokens: 0 } };
  } };
  actor = new ResidentActor(profile, { ...(live ? {} : { modelClient: mockProvider }) });
  actor.client.on("command", ({ type }) => { commands[type] = (commands[type] ?? 0) + 1; });
  actor.client.on("frame", ({ event }) => {
    if (["ObjectStruck", "ObjectDied"].includes(event.packet)) {
      evidence.push({ at: Date.now(), packet: event.packet, objectId: event.payload.objectId, ...(event.packet === "ObjectStruck" ? { attackerId: event.payload.attackerId } : {}) });
      if (evidence.length > 200) evidence.shift();
    }
  });
  await actor.start({ accountId: fixture.actor.accountId, password: fixture.actor.password });
  owner = await fixture.newOwnerClient();
  const control = (operation, payload = {}) => owner.companionControl(operation, { residentId: profile.companion.residentId, payload });
  const view = async () => { const receipt = await control("view"); assert.equal(receipt.status, "ok"); return receipt.result; };
  await control("teach", { expectedPolicyVersion: 1 });
  await owner.setAllowGroup(true); await owner.inviteMember(fixture.actor.characterName);
  await poll(() => actor.client.snapshot(), s => s.party.pendingInvite?.name === fixture.owner.characterName, "owner invite");
  assert.equal((await actor.companion.tick()).kind, "acceptInvite");
  await poll(() => owner.snapshot(), s => s.party.members.includes(fixture.actor.characterName), "shared roster");
  checks.push("real authenticated owner invitation and shared-zone membership; policy version 2");
  const collision = await loadClientCollision("0");
  async function walkOwner(target, desiredDistance = 0, maxSteps = 65) {
    for (let i = 0; i < maxSteps; i++) {
      const state = owner.snapshot(); if (distance(state.player, target) <= desiredDistance) return;
      const occupied = state.entities.filter(e => e.objectId !== state.playerObjectId && e.dead !== true).map(e => ({ x: e.x, y: e.y }));
      const direction = nextWalkDirection({ player: state.player, target, desiredDistance, collision, occupied });
      if (!direction) throw new Error("No ordinary owner path to observed monster.");
      await owner.walk(direction); await actor.companion.tick(); await sleep(650);
    }
    throw new Error("Bounded owner travel exhausted.");
  }
  phase = "prepare-observed-targets"; note(phase);
  let target = owner.snapshot().entities.filter(e => e.kind === "monster" && e.name === "Scarecrow" && !e.dead).sort((a, b) => distance(owner.snapshot().player, a) - distance(owner.snapshot().player, b))[0];
  if (!target) { await walkOwner({ x: 300, y: 610 }, 3); target = owner.snapshot().entities.find(e => e.kind === "monster" && e.name === "Scarecrow" && !e.dead); }
  assert.ok(target, "Use only an ordinary server-spawned visible target.");
  await walkOwner(target, 4);
  for (let i = 0; i < 15 && distance(actor.client.snapshot().player, owner.snapshot().player) > 2; i++) { await actor.companion.tick(); await sleep(650); }
  const candle = actor.client.snapshot().inventoryItems.find(e => e.name === "Candle");
  if (candle) await actor.client.dropOwnedItem(candle.uniqueId);
  phase = "private-natural-instruction"; note(phase);
  const instruction = "在附近打两只怪，再回来找我，途中别捡东西。", start = Date.now();
  const receipt = await control("instruct", { text: instruction, expectedPolicyVersion: 2 });
  assert.equal(receipt.status, "ok"); assert.ok(["planning", "running"].includes(receipt.result.runtime.modelTask.status));
  const acknowledgementMs = Date.now() - start;
  const planned = await poll(view, v => ["running", "failed"].includes(v.runtime.modelTask.status), "model result", 25_000);
  modelCallProof = { taskStatus: planned.runtime.modelTask.status, call: planned.runtime.modelTask.lastCall ?? null, error: planned.runtime.modelTask.error ?? null };
  assert.equal(planned.runtime.modelTask.status, "running", planned.runtime.modelTask.error);
  assert.equal(planned.runtime.modelTask.requestedKills, 2);
  assert.equal(planned.runtime.modelTask.lastCall.status, "completed");
  checks.push("private natural instruction acknowledged before execution, producing one constrained model goal");
  phase = "server-observed-two-monsters-and-return"; note(phase);
  let lastKills = 0, last = planned, performedModelAttack = false;
  const deadline = Date.now() + 140_000;
  while (Date.now() < deadline) {
    const action = await actor.companion.tick();
    performedModelAttack ||= action.reason === "model-hunt-observed-monster" && action.performed === true;
    assert.notEqual(action.kind, "loot", "The current no-loot task must suppress ordinary collection.");
    last = await view();
    if (["failed", "paused"].includes(last.runtime.modelTask.status)) throw new Error("The live objective stopped before its observed completion.");
    if (last.runtime.modelTask.status === "completed") break;
    if (action.reason === "model-no-allowed-monster-visible" && last.runtime.modelTask.completedKills < 2) {
      const next = owner.snapshot().entities.filter(e => e.kind === "monster" && e.name === "Scarecrow" && !e.dead).sort((a,b) => distance(owner.snapshot().player,a)-distance(owner.snapshot().player,b))[0];
      if (next) await walkOwner(next, 4, 25);
    }
    if (last.runtime.modelTask.completedKills !== lastKills) { lastKills = last.runtime.modelTask.completedKills; note(`confirmed-${lastKills}-of-2`); }
    await sleep(650);
  }
  assert.ok(performedModelAttack); assert.equal(last.runtime.modelTask.completedKills, 2); assert.equal(last.runtime.modelTask.status, "completed");
  assert.ok(distance(actor.client.snapshot().player, owner.snapshot().player) <= 2);
  assert.equal(last.trial.usage.modelCalls, 1); assert.equal(last.trial.costs.modelMicros, null);
  const archive = actor.store.archive(), context = buildModelContext({ state: actor.client.snapshot(), store: actor.store, text: "上次打两只怪回来", leaderName: actor.companion.leaderName });
  assert.ok(context.memories.some(e => e.kind === "objective-progress" && e.outcome === "observed"));
  checks.push("model-selected ordinary attack, own-strike then death progress 2/2, no pickup, real return within two tiles");
  const remembered = actor.store.growth().episodes.filter(e => e.payload?.kind === "objective-progress").map(e => ({ episodeId: e.episodeId, payloadHash: e.payloadHash }));
  await actor.stop(); actor = null;
  checks.push("observed objectives and source hashes committed in the durable character archive before ordinary logout");
  const summary = { schema: 1, passed: true, provider: live ? "openai-real-http" : "explicit-test-double-no-http", liveModelValidated: live,
    model: profile.runtimeService.model, acknowledgementMs, modelCall: modelCallProof, actualModelCalls: live ? 1 : 0, mockProviderCalls: live ? 0 : providerCalls,
    gatewaySha256: await sha256(fixture.gateway.executable), contentProfile: fixture.gateway.contentProfile, checks, commands,
    archiveIdentityHash: createHash("sha256").update(archive.archiveId).digest("hex"), remembered, evidence, humanTrial: false };
  await writeFile(resolve(fixture.output, "model-acceptance-summary.json"), JSON.stringify(summary, null, 2));
  console.log(JSON.stringify({ type: "modelAcceptancePassed", output: fixture.output, provider: summary.provider, checks: checks.length }));
} catch {
  if (fixture) await writeFile(resolve(fixture.output, "model-acceptance-failure.json"), JSON.stringify({ passed: false, phase, liveModelValidated: false, modelCall: modelCallProof, checks, commands, evidence }, null, 2));
  console.error(JSON.stringify({ type: "modelAcceptanceFailed", phase })); process.exitCode = 1;
} finally {
  await actor?.stop().catch(() => {}); await owner?.close().catch(() => {}); await fixture?.close();
  if (priorKey === undefined) delete process.env.OPENAI_API_KEY; else process.env.OPENAI_API_KEY = priorKey;
  if (priorArchiveKey === undefined) delete process.env.MIR2_COMPANION_ARCHIVE_KEY; else process.env.MIR2_COMPANION_ARCHIVE_KEY = priorArchiveKey;
  if (localSettings) localSettings.apiKey = null;
}
