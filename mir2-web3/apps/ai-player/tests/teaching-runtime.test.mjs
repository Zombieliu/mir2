import test from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve, basename, sep } from "node:path";
import { setImmediate as settle } from "node:timers/promises";
import { DatabaseSync } from "node:sqlite";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { signArchive } from "../src/memory/archive-codec.mjs";
import { CompanionModelRuntime } from "../src/model/companion-model-runtime.mjs";
import { validateTeachingPlan } from "../src/model/teaching-plan.mjs";

const binding = { realmId: "teaching-test", residentId: "alo", characterName: "Alo", characterIndex: 1 };
const identity = { archiveId: "teaching-archive", ownerRef: "owner:test", worldEpoch: "test-epoch" };
const key = Buffer.alloc(32, 7);
const teaching = (changes = { assistBeforeLoot: true, lootOnlyAfterCombat: true }) => ({ supported: true, reply: "建议先支援，并在战斗后拾取。", changes: { assistBeforeLoot: null, lootOnlyAfterCombat: null, askBeforeLeaving: null, ...changes }, rememberedEpisodeIds: [] });
const objective = () => ({ reply: "跟随主人。", objective: { kind: "follow", monsterName: null, count: 0, maxDistanceFromOwner: 6, loot: "never", returnToOwner: false }, rememberedEpisodeIds: [] });

function fixture(t, { pending = false, maxCallsPerHour = 20 } = {}) {
  const root = mkdtempSync(join(tmpdir(), "alo-teaching-")), path = join(root, "growth.sqlite"), stores = [], runtimes = [];
  let store, calls = 0, nextPlan = teaching(); const waiting = [];
  const open = () => { store = new CompanionStore({ path, binding, ...identity }); stores.push(store); return store; }; open();
  class Client extends EventEmitter {
    state = { ready: true, connectionEpoch: 1, mapFileName: "0", mapIndex: 0, playerObjectId: 1, selfZoneObjectId: 5000, player: { name: "Alo", x: 10, y: 10 }, playerHp: 100, playerMaxHp: 100,
      party: { members: ["Alo", "Owner"] }, entities: [{ objectId: 2, kind: "player", name: "Owner", x: 11, y: 10 }], groundDrops: [] };
    snapshot() { return structuredClone(this.state); }
  }
  const client = new Client(), controller = { leaderName: "Owner" };
  const create = () => {
    const runtime = new CompanionModelRuntime({ client, store, controller, configuration: { provider: "openai", model: "gpt-6-luna", credentialRef: "OPENAI_API_KEY", maxCallsPerHour }, environment: { OPENAI_API_KEY: "sk-synthetic-teaching-test-only" },
      modelClient: { async plan(input) { calls++; const output = { plan: structuredClone(input.schema.properties.supported ? nextPlan : objective()), call: { model: "gpt-6-luna", status: "completed", requestId: "req_synthetic_teaching" } }; return pending ? new Promise(resolve => waiting.push(() => resolve(output))) : output; } } });
    runtimes.push(runtime); return runtime;
  };
  let runtime = create();
  t.after(async () => { for (const r of runtimes) await r.close(); for (const s of stores) s.close(); const p = resolve(root); assert.ok(p.startsWith(resolve(tmpdir()) + sep) && basename(p).startsWith("alo-teaching-")); rmSync(p, { recursive: true, force: true }); });
  return { path, root, client, controller, get store() { return store; }, get runtime() { return runtime; }, calls: () => calls,
    setPlan(plan) { nextPlan = plan; }, resolveNext() { waiting.shift()(); },
    propose(id = "lesson-one", text = "以后组队时先支援，打完再捡东西。", expectedPolicyVersion = store.profile().policyVersion) { runtime.proposeTeaching({ requestId: id, text, expectedPolicyVersion }); },
    confirm(id = "confirm-one", proposalId = runtime.view().teachingProposal.proposalId, expectedPolicyVersion = runtime.view().teachingProposal.sourceVersion) { runtime.confirmTeaching({ requestId: id, proposalId, expectedPolicyVersion }); },
    async restart() { await runtime.close(); store.close(); open(); runtime = create(); },
    import(envelope) { const s = CompanionStore.importArchive({ path: join(root, "imported.sqlite"), binding, ...identity, key, envelope }); stores.push(s); return s; } };
}

test("teaching schema permits only partial soft booleans and rejects authority or fabricated citations", () => {
  assert.deepEqual(validateTeachingPlan(teaching({ assistBeforeLoot: false, lootOnlyAfterCombat: null })).changes, { assistBeforeLoot: false });
  for (const malformed of [
    { ...teaching(), spendLimit: 50 }, { ...teaching(), changes: { ...teaching().changes, spendLimit: 50 } },
    { ...teaching(), changes: { assistBeforeLoot: "yes", lootOnlyAfterCombat: null, askBeforeLeaving: null } },
    { ...teaching(), changes: { assistBeforeLoot: null, lootOnlyAfterCombat: null, askBeforeLeaving: null } },
    { ...teaching(), rememberedEpisodeIds: ["same", "same"] }, { ...teaching(), rememberedEpisodeIds: ["bad id"] },
    { ...teaching(), reply: "sk-synthetic-secret-not-allowed" }, { ...teaching(), supported: false },
  ]) assert.throws(() => validateTeachingPlan(malformed), { code: "invalid-plan" });
  assert.equal(validateTeachingPlan({ ...teaching(), supported: false, changes: { assistBeforeLoot: null, lootOnlyAfterCombat: null, askBeforeLeaving: null } }).supported, false);
});

test("preview does not change habits; one explicit confirmation persists a partial patch without another call", async t => {
  const f = fixture(t); f.propose(); await settle();
  const preview = f.runtime.view().teachingProposal;
  assert.equal(preview.status, "proposed"); assert.equal(preview.sourceVersion, 1);
  assert.deepEqual(f.store.profile().party, { assistBeforeLoot: false, lootOnlyAfterCombat: false, askBeforeLeaving: false });
  assert.throws(() => f.confirm("forged", "teaching:unknown"), { code: "stale-plan" });
  f.confirm(); assert.equal(f.calls(), 1); assert.equal(f.runtime.view().teachingProposal.status, "confirmed");
  assert.deepEqual(f.store.profile().party, { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: false });
  const revision = f.store.growth().growthRevision;
  f.runtime.confirmTeaching({ requestId: "confirm-one", proposalId: preview.proposalId, expectedPolicyVersion: 1 });
  assert.equal(f.store.growth().growthRevision, revision); assert.equal(f.calls(), 1);
  assert.throws(() => f.confirm("second-confirm", preview.proposalId, 1), { code: "stale-plan" });
  await f.restart(); assert.equal(f.calls(), 1); assert.equal(f.runtime.view().teachingProposal.status, "idle");
  assert.equal(f.store.profile().party.lootOnlyAfterCombat, true);
});

test("per-field false overlays in original sequence; revocation and deletion restore prior teaching through archive", async t => {
  const f = fixture(t); f.store.teachSupportBeforeLoot({ feedbackId: "preset" });
  f.setPlan(teaching({ assistBeforeLoot: false, lootOnlyAfterCombat: null })); f.propose("less-assist"); await settle(); f.confirm("confirm-assist");
  f.setPlan(teaching({ assistBeforeLoot: null, lootOnlyAfterCombat: false })); f.propose("less-loot"); await settle(); f.confirm("confirm-loot");
  assert.deepEqual(f.store.profile().party, { assistBeforeLoot: false, lootOnlyAfterCombat: false, askBeforeLeaving: true });
  f.store.revokeFeedback({ feedbackId: "confirm-loot", operationId: "revoke-loot", expectedPolicyVersion: 4 });
  assert.deepEqual(f.store.profile().party, { assistBeforeLoot: false, lootOnlyAfterCombat: true, askBeforeLeaving: true });
  f.store.deleteFeedback({ feedbackId: "confirm-assist", operationId: "delete-assist", expectedPolicyVersion: 5 });
  assert.deepEqual(f.store.profile().party, { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: true });
  const envelope = f.store.exportArchive({ key }); assert.equal(JSON.parse(envelope).version, 2);
  assert.deepEqual(f.import(envelope).growth(), f.store.growth());
});

test("confirmation rolls back every new episode, feedback and policy update on a real SQLite write failure", async t => {
  const f = fixture(t); f.propose(); await settle(); const before = f.store.growth();
  const db = new DatabaseSync(f.path); db.exec("CREATE TRIGGER fail_teaching BEFORE INSERT ON feedback BEGIN SELECT RAISE(ABORT,'synthetic confirmation failure'); END;");
  assert.throws(() => f.confirm(), /synthetic confirmation failure/); assert.deepEqual(f.store.growth(), before);
  db.exec("DROP TRIGGER fail_teaching"); db.close(); f.confirm(); assert.equal(f.store.profile().policyVersion, 2); assert.equal(f.calls(), 1);
});

test("deleting one long teaching erases its full source text while preserving an independently cited teaching", async t => {
  const f = fixture(t); f.propose("old-lesson", "以前的独立教学：组队先支援。"); await settle(); f.confirm("old-confirm");
  const oldInstruction = f.store.growth().episodes.find(row => row.payload?.kind === "owner-teaching-instruction");
  f.setPlan({ ...teaching({ assistBeforeLoot: null, lootOnlyAfterCombat: true }), rememberedEpisodeIds: [oldInstruction.episodeId] });
  const privateText = "独有教学正文".repeat(100) + "尾段隐私应当完全删除";
  f.propose("long-lesson", privateText); await settle(); f.confirm("long-confirm");
  f.store.deleteFeedback({ operationId: "erase-long", feedbackId: "long-confirm", expectedPolicyVersion: f.store.profile().policyVersion });
  assert.equal(f.store.growth().episodes.find(row => row.episodeId === oldInstruction.episodeId).status, "active");
  const exported = f.store.exportArchive({ key }); assert.equal(exported.includes("尾段隐私应当完全删除"), false); assert.equal(exported.includes("独有教学正文"), false);
  assert.equal(f.store.growth().feedback.find(row => row.feedbackId === "old-confirm").status, "active");
});

test("source hash tampering cannot be confirmed even if the stored hash column is left unchanged", async t => {
  const f = fixture(t); f.propose(); await settle();
  const instruction = f.store.growth().episodes.find(row => row.payload?.kind === "owner-teaching-instruction");
  const db = new DatabaseSync(f.path), payload = { ...instruction.payload, summary: "伪造的教学" };
  db.prepare("UPDATE episodes SET payload_json=? WHERE episode_id=?").run(JSON.stringify(payload), instruction.episodeId); db.close();
  assert.throws(() => f.confirm(), { code: "stale-plan" }); assert.equal(f.store.profile().policyVersion, 1);
});

test("deleting a proposal or its source prevents confirmation; deleting accepted source revokes the derived habit", async t => {
  const f = fixture(t); f.propose(); await settle();
  const instruction = f.store.growth().episodes.find(row => row.payload?.kind === "owner-teaching-instruction");
  f.store.deleteEpisode({ episodeId: instruction.episodeId });
  assert.throws(() => f.confirm(), { code: "stale-plan" }); assert.equal(f.store.profile().policyVersion, 1);
  f.propose("fresh"); await settle(); f.confirm();
  const accepted = f.store.growth().feedback.find(row => row.feedbackId === "confirm-one");
  f.store.deleteEpisode({ episodeId: accepted.payload.proposalId });
  assert.equal(f.store.growth().feedback.find(row => row.feedbackId === "confirm-one").status, "deleted");
  assert.equal(f.store.profile().party.assistBeforeLoot, false);
  f.store.exportArchive({ key });
});

test("cancel fences a late provider answer and retry; restart cannot auto-confirm an old proposal", async t => {
  const f = fixture(t, { pending: true }); f.propose();
  const proposalId = f.runtime.view().teachingProposal.proposalId;
  f.runtime.cancelTeaching({ proposalId }); f.resolveNext(); await settle();
  assert.equal(f.runtime.view().teachingProposal.status, "cancelled"); f.propose(); assert.equal(f.calls(), 1);
  f.propose("second"); f.resolveNext(); await settle(); const second = f.runtime.view().teachingProposal.proposalId;
  await f.restart(); assert.throws(() => f.confirm("late-confirm", second, 1), { code: "stale-plan" });
  f.propose("second"); assert.equal(f.calls(), 2); assert.equal(f.store.profile().policyVersion, 1);
});

test("objectives and teaching share a durable hourly quota including deleted call tombstones", async t => {
  const f = fixture(t, { maxCallsPerHour: 1 }); f.propose(); await settle();
  const call = f.store.growth().episodes.find(row => row.payload?.kind === "model-call"); f.store.deleteEpisode({ episodeId: call.episodeId });
  assert.throws(() => f.runtime.instruct({ requestId: "new-goal", text: "跟着我", expectedPolicyVersion: 1 }), { code: "call-budget" });
  await f.restart(); assert.equal(f.runtime.view().modelCalls, 1); assert.throws(() => f.propose("another"), { code: "call-budget" });
  assert.equal(f.calls(), 1);
});

test("existing objective requests consume teaching quota and unsupported teaching makes no preference", async t => {
  const f = fixture(t, { maxCallsPerHour: 1 }); f.runtime.instruct({ requestId: "first-goal", text: "跟着我", expectedPolicyVersion: 1 }); await settle();
  assert.throws(() => f.propose(), { code: "call-budget" });
  const g = fixture(t); g.setPlan({ supported: false, reply: "预算属于独立授权，不能通过教学修改。", changes: { assistBeforeLoot: null, lootOnlyAfterCombat: null, askBeforeLeaving: null }, rememberedEpisodeIds: [] });
  g.propose("budget-request", "以后可以花五十金币。"); await settle(); assert.equal(g.runtime.view().teachingProposal.status, "unsupported");
  assert.equal(g.store.profile().permissions.spendLimit, 0); assert.equal(g.store.profile().policyVersion, 1);
});

test("deleted citation and changed policy fence both late proposals and confirmation", async t => {
  const f = fixture(t, { pending: true }); f.propose(); f.store.teachSupportBeforeLoot({ feedbackId: "other" }); f.resolveNext(); await settle();
  assert.equal(f.runtime.view().teachingProposal.status, "failed");
  const g = fixture(t); g.store.recordEpisode({ episodeId: "real-strike", kind: "objective-progress", action: "hunt", outcome: "observed", summary: "服务器观察：上次实际命中了怪物。" });
  g.setPlan({ ...teaching(), rememberedEpisodeIds: ["real-strike"] }); g.propose(); await settle();
  assert.equal(g.runtime.view().teachingProposal.references[0].episodeId, "real-strike");
  g.store.deleteEpisode({ episodeId: "real-strike" }); assert.throws(() => g.confirm(), { code: "stale-plan" });
  const h = fixture(t); h.setPlan({ ...teaching(), rememberedEpisodeIds: ["invented"] }); h.propose(); await settle();
  assert.equal(h.runtime.view().teachingProposal.status, "failed"); assert.equal(h.store.profile().policyVersion, 1);
});

test("schema 2 migrates without rewriting old event bytes or policy and rolls back failed migration", async t => {
  const f = fixture(t); f.store.teachSupportBeforeLoot({ feedbackId: "old-preset" }); const before = f.store.growth(); await f.runtime.close(); f.store.close();
  const db = new DatabaseSync(f.path); const row = db.prepare("SELECT profile_json FROM companion_state").get(), old = JSON.parse(row.profile_json); old.schemaVersion = 2;
  db.prepare("UPDATE companion_state SET schema_version=2,profile_json=?").run(JSON.stringify(old)); db.exec("PRAGMA user_version=2; CREATE TRIGGER fail_v3 BEFORE UPDATE ON companion_state BEGIN SELECT RAISE(ABORT,'synthetic migration failure'); END;");
  assert.throws(() => new CompanionStore({ path: f.path, binding, ...identity }), /synthetic migration failure/); assert.equal(db.prepare("PRAGMA user_version").get().user_version, 2);
  db.exec("DROP TRIGGER fail_v3"); const bytes = db.prepare("SELECT payload_json,payload_hash FROM episodes ORDER BY sequence").all(); db.close();
  await f.restart(); assert.equal(f.store.profile().schemaVersion, 3); assert.deepEqual(f.store.growth(), before);
  const check = new DatabaseSync(f.path, { readOnly: true }); assert.deepEqual(check.prepare("SELECT payload_json,payload_hash FROM episodes ORDER BY sequence").all(), bytes); check.close();
});

test("legacy archive v1 imports presets but cannot smuggle a new preference teaching", async t => {
  const f = fixture(t); f.store.teachSupportBeforeLoot({ feedbackId: "legacy-preset" });
  const old = signArchive({ characterArchive: f.store.archive(), playerGrowth: f.store.growth(), key, version: 1 });
  assert.deepEqual(f.import(old).growth(), f.store.growth());
  f.propose(); await settle(); f.confirm();
  const disguised = signArchive({ characterArchive: f.store.archive(), playerGrowth: f.store.growth(), key, version: 1 });
  assert.throws(() => CompanionStore.importArchive({ path: join(f.root, "bad-v1.sqlite"), binding, ...identity, key, envelope: disguised }), /archive version 2/);
});
