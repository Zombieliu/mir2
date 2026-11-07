import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { execFileSync } from "node:child_process";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { payloadHash } from "../src/memory/archive-codec.mjs";
import { buildModelContext, MAX_MODEL_CONTEXT_BYTES, validateMemoryReferences } from "../src/model/memory-context.mjs";

const binding = { realmId: "model-context-world", residentId: "alo-memory", characterName: "Alo", characterIndex: 3, operatorRef: "owner:private-model-test" };
const trusted = { archiveId: "archive:private-model-test", ownerRef: binding.operatorRef, worldEpoch: "epoch:model-test" };

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "mir2-model-context-")), stores = [];
  t.after(() => {
    for (const store of stores) store.close();
    const path = resolve(root);
    assert.ok(path.startsWith(resolve(tmpdir()) + sep) && basename(path).startsWith("mir2-model-context-"));
    rmSync(path, { recursive: true, force: true });
  });
  return { path: join(root, "memory.sqlite"), open() {
    const store = new CompanionStore({ path: this.path, binding, ...trusted }); stores.push(store); return store;
  } };
}

function state() {
  return { mapFileName: "0", playerHp: 11, playerMaxHp: 18,
    player: { objectId: 1000, name: "Alo", x: 288, y: 616 },
    entities: [
      { objectId: 2000, kind: "player", name: "Owner", x: 289, y: 616 },
      { objectId: 3000, kind: "monster", name: "Scarecrow", x: 290, y: 617, hp: 10 },
      { objectId: 3001, kind: "monster", name: "Dead Scarecrow", x: 289, y: 617, dead: true },
      { objectId: 3002, kind: "npc", name: "Merchant", x: 288, y: 617 },
    ],
    groundDrops: [{ objectId: 4000, name: "Candle", x: 287, y: 616, uniqueId: "private-item-uid", exactItem: { account: "private-item-account" } }],
    party: { allowGroup: true, members: ["Owner", "Alo"], leaderName: "Owner", pendingInvite: null } };
}

const contextFor = (store, options = {}) => buildModelContext({ state: state(), store, leaderName: "Owner", ...options });
const observed = (store, episodeId, summary) => store.recordEpisode({ episodeId, kind: "objective-progress", action: "support", outcome: "observed", summary });

test("world projects only current observations and exact real membership, with unknown values kept unknown", t => {
  const store = fixture(t).open(), input = state();
  const context = contextFor(store, { state: input });
  assert.deepEqual(context.world, { mapFileName: "0", position: { x: 288, y: 616 }, health: { hp: 11, maxHp: 18 },
    owner: { visible: true, position: { x: 289, y: 616 } }, party: { grouped: true },
    monsters: [{ objectId: 3000, name: "Scarecrow", position: { x: 290, y: 617 } }],
    nearbyDrops: [{ objectId: 4000, name: "Candle", position: { x: 287, y: 616 } }] });
  assert.deepEqual(context.habits, { assistBeforeLoot: false, lootOnlyAfterCombat: false, askBeforeLeaving: false });
  context.world.position.x = 10;
  assert.equal(input.player.x, 288, "the projection cannot mutate current observations");
  const unknown = contextFor(store, { state: { ...input, playerHp: undefined, playerMaxHp: null,
    entities: [], party: { members: [], pendingInvite: { name: "Owner" } } } });
  assert.deepEqual(unknown.world.health, { hp: null, maxHp: null });
  assert.deepEqual(unknown.world.owner, { visible: false, position: null });
  assert.equal(unknown.world.party.grouped, false, "an invitation is not authoritative membership");
  const wrongCase = contextFor(store, { leaderName: "owner" });
  assert.equal(wrongCase.world.owner.visible, false);
  assert.equal(wrongCase.world.party.grouped, false);
});

test("Chinese bigrams and English words retrieve older relevant SQLite history beyond the recent-100 page", t => {
  const store = fixture(t).open();
  observed(store, "old-chinese", "药水不足时先撤退回城，再为队伍补给。");
  observed(store, "old-english", "A failed escort taught retreat and potion supplies.");
  for (let index = 0; index < 120; index += 1) store.recordEpisode({ episodeId: `recent-${index}`, kind: "decision", action: "wait", summary: "在城内休息。" });
  assert.equal(contextFor(store, { text: "药水不足，需要撤退回城", limit: 1 }).memories[0].episodeId, "old-chinese");
  assert.equal(contextFor(store, { text: "escort retreat potion", limit: 1 }).memories[0].episodeId, "old-english");
  assert.deepEqual(contextFor(store, { text: "", limit: 3 }).memories.map(row => row.episodeId), ["recent-119", "recent-118", "recent-117"]);
});

test("real SQLite reopen preserves selected memories and citation hashes; altered or duplicate citations fail", t => {
  const f = fixture(t); let store = f.open();
  observed(store, "persisted-observation", "真实返回主人附近。");
  const first = contextFor(store, { text: "返回主人" });
  assert.equal(validateMemoryReferences(["persisted-observation"], first, store), true);
  store.close();
  const probe = execFileSync(process.execPath, ["--input-type=module", "-e", `
    import { CompanionStore } from ${JSON.stringify(new URL("../src/memory/companion-store.mjs", import.meta.url).href)};
    import { buildModelContext, validateMemoryReferences } from ${JSON.stringify(new URL("../src/model/memory-context.mjs", import.meta.url).href)};
    const store = new CompanionStore({path:process.argv[1],binding:${JSON.stringify(binding)},...${JSON.stringify(trusted)}});
    try {
      const context=buildModelContext({state:${JSON.stringify(state())},store,text:"返回主人",leaderName:"Owner"});
      process.stdout.write(JSON.stringify({context,valid:validateMemoryReferences(["persisted-observation"],context,store)}));
    } finally { store.close(); }
  `, f.path], { encoding: "utf8", windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
  const reopenedInDifferentProcess = JSON.parse(probe);
  assert.deepEqual(reopenedInDifferentProcess.context, first);
  assert.equal(reopenedInDifferentProcess.valid, true);
  store = f.open();
  assert.deepEqual(contextFor(store, { text: "返回主人" }), first);
  assert.equal(validateMemoryReferences(["persisted-observation"], first, store), true);
  const altered = structuredClone(first); altered.memories[0].summary = "A fabricated different event.";
  assert.equal(validateMemoryReferences(["persisted-observation"], altered, store), false);
  const alteredStamp = structuredClone(first); alteredStamp.memoryStamp[0].payloadHash = "0".repeat(64);
  assert.equal(validateMemoryReferences(["persisted-observation"], alteredStamp, store), false);
  assert.equal(validateMemoryReferences(["not-in-context"], first, store), false);
  assert.equal(validateMemoryReferences(["persisted-observation", "persisted-observation"], first, store), false);
  assert.equal(validateMemoryReferences([], first, store), true);
  assert.equal(validateMemoryReferences("persisted-observation", first, store), false);
  store.close();
  assert.equal(validateMemoryReferences(["persisted-observation"], first, store), false);
});

test("teaching is separate habits; revocation, deletion and clearing cannot be resurrected through old contexts", t => {
  const f = fixture(t); let store = f.open();
  store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "lesson-one", feedbackText: "组队先支援，战斗结束后拾取。" });
  observed(store, "event-one", "唯一旧经历文本：击退稻草人。");
  observed(store, "event-two", "另一次实际返回城内。");
  const context = contextFor(store), teaching = store.growth().feedback[0];
  assert.deepEqual(context.habits, { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: true });
  assert.equal(context.memories.some(row => row.kind === "feedback"), false);
  assert.equal(validateMemoryReferences([teaching.episodeId], context, store), false, "teaching text is not cited as an observed event");
  store.revokeFeedback({ operationId: "revoke-one", feedbackId: "lesson-one", expectedPolicyVersion: 2 });
  assert.equal(store.growth().episodes.find(row => row.episodeId === teaching.episodeId).status, "revoked");
  const forged = structuredClone(context);
  const source = store.growth().episodes.find(row => row.episodeId === teaching.episodeId);
  forged.memories.push({ episodeId: source.episodeId, kind: "feedback", action: "teach-support-before-loot", outcome: "accepted",
    summary: "组队先支援", recordedAt: source.recordedAt, policyVersion: source.policyVersion, payloadHash: source.payloadHash });
  forged.memoryStamp.push({ episodeId: source.episodeId, payloadHash: source.payloadHash });
  assert.equal(validateMemoryReferences([teaching.episodeId], forged, store), false);
  assert.equal(contextFor(store).habits.assistBeforeLoot, false);
  assert.equal(validateMemoryReferences(["event-one", "event-two"], context, store), true);
  store.deleteEpisode({ operationId: "delete-one", episodeId: "event-one", expectedPolicyVersion: 3 });
  assert.equal(validateMemoryReferences(["event-one"], context, store), false);
  assert.equal(validateMemoryReferences(["event-two"], context, store), true);
  assert.equal(JSON.stringify(contextFor(store)).includes("唯一旧经历文本"), false);
  store.clearGrowth({ operationId: "clear-all", expectedPolicyVersion: 3, confirmArchiveId: store.archive().archiveId });
  assert.deepEqual(contextFor(store).memories, []);
  assert.equal(validateMemoryReferences(["event-two"], context, store), false);
  store.close(); store = f.open();
  assert.deepEqual(contextFor(store).memoryStamp, []);
  assert.equal(validateMemoryReferences(["event-one", "event-two"], context, store), false);
});

test("same-ID content changes and corrupted payload hashes invalidate citations against the live SQLite rows", t => {
  const f = fixture(t), store = f.open();
  observed(store, "hash-check", "A real earlier observation.");
  const context = contextFor(store), row = store.growth().episodes.find(value => value.episodeId === "hash-check");
  const db = new DatabaseSync(f.path);
  try {
    const changed = { ...row.payload, summary: "Different replacement observation." };
    db.prepare("UPDATE episodes SET payload_json=?,payload_hash=? WHERE episode_id=?").run(JSON.stringify(changed), payloadHash(changed), row.episodeId);
    assert.equal(validateMemoryReferences([row.episodeId], context, store), false);
    const replacement = contextFor(store);
    assert.equal(validateMemoryReferences([row.episodeId], replacement, store), true);
    db.prepare("UPDATE episodes SET payload_json=? WHERE episode_id=?").run(JSON.stringify({ ...changed, summary: "Corrupted text without updating its hash." }), row.episodeId);
    assert.equal(validateMemoryReferences([row.episodeId], replacement, store), false);
    assert.deepEqual(contextFor(store).memories, []);
  } finally { db.close(); }
});

test("owner instructions and model plans retain their source kinds; model calls and fabricated model facts are excluded", t => {
  const store = fixture(t).open();
  for (const [episodeId, kind, outcome, summary] of [
    ["owner-words", "owner-instruction", "received", "主人委托：支援队友。"],
    ["owner-more-words", "owner-instruction-continuation", "received", "主人委托续文：危险时返回。"],
    ["proposed-plan", "model-plan", "proposed", "A plan predicts an unseen dragon at another position."],
    ["call-attempt", "model-call", "attempted", "A metered model call attempt."],
    ["actual-progress", "objective-progress", "observed", "普通 ObjectStruck 和 ObjectDied 回执确认击杀。"],
    ["unobserved-progress", "objective-progress", "proposed", "An unobserved proposed result."],
    ["fake-model-fact", "model-observation", "observed", "A model claimed a new world fact."],
    ["fake-plan-outcome", "model-plan", "observed", "A proposal incorrectly claims observed execution."],
  ]) store.recordEpisode({ episodeId, kind, outcome, summary, context: { sourceEventRef: "goal:one" } });
  const context = contextFor(store);
  assert.deepEqual(new Set(context.memories.map(row => row.episodeId)), new Set(["owner-words", "owner-more-words", "proposed-plan", "actual-progress"]));
  assert.equal(context.memories.find(row => row.episodeId === "proposed-plan").kind, "model-plan");
  assert.equal(context.memories.find(row => row.episodeId === "owner-more-words").kind, "owner-instruction-continuation");
  assert.deepEqual(context.world.monsters.map(row => row.name), ["Scarecrow"], "a remembered proposal cannot add entities to the observed world");
  assert.equal(context.world.position.x, 288);
  assert.equal(store.growth().episodes.length, 8, "building context never rewrites or records model output as fact");
});

test("credential-looking text, private identity/runtime trees and world chat never enter the context", t => {
  const store = fixture(t).open();
  observed(store, "safe-memory", "A useful ordinary observation.");
  store.recordEpisode({ episodeId: "private-refs", kind: "decision", summary: "A public decision.", actorRef: "account:private-actor-ref", context: { leaderRef: "owner:private-model-test" } });
  for (const [episodeId, summary] of [["owner-ref-text", "ownerRef=owner:private-model-test"], ["account-text", "accountId=private-login"], ["archive-key-text", "archiveKey=private-signing-material"]]) {
    store.recordEpisode({ episodeId, kind: "decision", summary });
  }
  store.recordEpisode({ episodeId: "sk-proj-private-episode-key", kind: "decision", summary: "Safe text with a credential-looking identifier." });
  assert.throws(() => store.recordEpisode({ episodeId: "private-action", kind: "decision", action: "sk-proj-private-action-key", summary: "Safe text with a credential-looking action." }), /credentials/);
  const input = state();
  Object.assign(input, { account: "private-account", password: "private-password", ownerRef: binding.operatorRef, archiveKey: "private-archive-key",
    lastChat: { text: "private-world-chat" }, runtimeService: { provider: "private-provider", credentialRef: "private-credential-ref", promptVersion: "private-prompt-version" } });
  input.entities.push(
    { objectId: 3100, kind: "monster", name: "Bearer private-world-token", x: 290, y: 616 },
    { objectId: 3101, kind: "monster", name: "sk-proj-private-model-key-123456", x: 290, y: 616 },
    { objectId: 3102, kind: "monster", name: "Skirmisher", x: 290, y: 616 });
  input.groundDrops.push({ objectId: 4100, name: "password=private-drop-password", x: 287, y: 617 });
  const context = contextFor(store, { state: input, text: "password=private-query-password" });
  const serialized = JSON.stringify(context);
  for (const privateText of ["private-account", "private-password", "owner:private-model-test", "archive:private-model-test", "private-archive-key",
    "private-world-chat", "private-provider", "private-credential-ref", "private-prompt-version", "private-actor-ref", "private-login",
    "private-signing-material", "private-world-token", "private-model-key", "private-drop-password", "private-query-password", "private-item-uid", "private-item-account",
    "private-episode-key", "private-action-key"]) {
    assert.equal(serialized.includes(privateText), false, `Private input leaked: ${privateText}`);
  }
  for (const key of ["account", "password", "ownerRef", "archiveId", "archiveKey", "runtimeService", "provider", "credentialRef", "promptVersion", "lastChat", "actorRef", "context"]) {
    assert.equal(serialized.includes(`"${key}":`), false, `Private field leaked: ${key}`);
  }
  assert.equal(context.world.monsters.find(row => row.objectId === 3100).name, null);
  assert.equal(context.world.monsters.find(row => row.objectId === 3102).name, "Skirmisher");
  assert.deepEqual(new Set(context.memories.map(row => row.episodeId)), new Set(["safe-memory", "private-refs"]));
});

test("UTF-8 and JSON escaping respect the aggregate 16 KiB limit while retaining critical world fields", t => {
  const store = fixture(t).open();
  for (let index = 0; index < 12; index += 1) store.recordEpisode({ episodeId: `e${index}${"x".repeat(95)}`,
    kind: "d".repeat(64), action: "a".repeat(64), outcome: "o".repeat(64), summary: '"\\'.repeat(240) });
  const input = state();
  input.entities = [...input.entities.filter(row => row.kind === "player"), ...Array.from({ length: 100 }, (_, index) => ({
    objectId: 5000 + index, kind: "monster", hp: 1, name: '"\\'.repeat(64), x: 289 + index, y: 616 }))];
  input.groundDrops = Array.from({ length: 100 }, (_, index) => ({ objectId: 6000 + index, name: '"\\'.repeat(64), x: 287 + index, y: 616 }));
  const context = contextFor(store, { state: input, text: "委托".repeat(500_000), limit: 999 });
  assert.ok(Buffer.byteLength(JSON.stringify(context), "utf8") <= MAX_MODEL_CONTEXT_BYTES);
  assert.equal(context.world.monsters.length, 12); assert.equal(context.world.nearbyDrops.length, 12);
  assert.ok(context.memories.length > 0 && context.memories.length < 8, "optional memories yield to the actual byte budget");
  assert.equal(context.memoryStamp.length, context.memories.length);
  assert.deepEqual(context.world.position, { x: 288, y: 616 });
  assert.deepEqual(context.world.health, { hp: 11, maxHp: 18 });
  assert.equal(context.world.owner.visible, true); assert.equal(context.world.party.grouped, true);
  assert.equal(validateMemoryReferences(context.memories.map(row => row.episodeId), context, store), true);
  assert.deepEqual(contextFor(store, { limit: 0 }).memories, []);
  assert.equal(contextFor(store, { limit: 1 }).memories.length, 1);
  assert.throws(() => contextFor(store, { limit: -1 }), /limit/);
  assert.throws(() => contextFor(store, { limit: NaN }), /limit/);
});

test("oversized or control-bearing world values are discarded without changing stored growth", t => {
  const store = fixture(t).open(), input = state();
  input.mapFileName = "m".repeat(1_000_000);
  input.entities[1].name = "n".repeat(1_000_000);
  input.groundDrops[0].name = "Candle\u202Eprivate";
  const before = store.growth();
  const context = contextFor(store, { state: input });
  assert.equal(context.world.mapFileName, null);
  assert.equal(context.world.monsters[0].name, null);
  assert.equal(context.world.nearbyDrops[0].name, null);
  assert.deepEqual(store.growth(), before);
  assert.ok(Buffer.byteLength(JSON.stringify(context)) < MAX_MODEL_CONTEXT_BYTES);
});
