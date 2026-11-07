import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { execFileSync } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { MAX_ARCHIVE_BYTES, signArchive } from "../src/memory/archive-codec.mjs";

const binding = { realmId: "test-world", residentId: "alo-one", characterName: "Alo", characterIndex: 12, operatorRef: "owner:test" };
const trusted = { archiveId: "archive-stable-one", ownerRef: "owner:test", worldEpoch: "epoch-one", displayName: "阿洛", baseSetting: "比奇冒险者" };
const key = Buffer.alloc(32, 7);
const gateway = (operationId, expectedPolicyVersion, extra = {}) => ({ source: "gateway-owner", operationId, expectedPolicyVersion,
  archiveId: trusted.archiveId, ownerRef: trusted.ownerRef, worldEpoch: trusted.worldEpoch, ...extra });

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "mir2-character-archive-")), stores = [];
  t.after(() => {
    for (const store of stores) store.close();
    const absolute = resolve(root);
    assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("mir2-character-archive-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  return { root, path: join(root, "runner-a.sqlite"), stores,
    open(options = {}) { const store = new CompanionStore({ path: this.path, binding, ...trusted, ...options }); stores.push(store); return store; },
    import(envelope, options = {}) { const store = CompanionStore.importArchive({ path: join(root, "runner-b.sqlite"), binding, ...trusted, key, envelope, ...options }); stores.push(store); return store; },
  };
}

function legacy(path, { failUpdate = false, operator = binding.operatorRef } = {}) {
  const oldBinding = { ...binding }; if (operator === undefined) delete oldBinding.operatorRef; else oldBinding.operatorRef = operator;
  const profile = { schemaVersion: 1, policyVersion: 2, style: "collection", binding: oldBinding, solo: { considerLootWhenSafe: true },
    party: { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: true }, permissions: { spendLimit: 0, mapAllowlist: ["0", "0141"] } };
  const payload = { source: "local-owner", context: { mode: "party" }, feedbackText: "原先真实教学" };
  const episode = { kind: "feedback", action: "teach-support-before-loot", outcome: "accepted", feedbackId: "legacy-lesson", ...payload };
  const db = new DatabaseSync(path);
  db.exec("CREATE TABLE companion_state(singleton INTEGER PRIMARY KEY,schema_version INTEGER NOT NULL,binding_json TEXT NOT NULL,profile_json TEXT NOT NULL);" +
    "CREATE TABLE episodes(sequence INTEGER PRIMARY KEY AUTOINCREMENT,episode_id TEXT NOT NULL UNIQUE,recorded_at INTEGER NOT NULL,policy_version INTEGER NOT NULL,payload_json TEXT NOT NULL);" +
    "CREATE TABLE feedback(feedback_id TEXT PRIMARY KEY,payload_json TEXT NOT NULL,result_profile_json TEXT NOT NULL,episode_id TEXT NOT NULL REFERENCES episodes(episode_id)); PRAGMA user_version=1;");
  db.prepare("INSERT INTO companion_state VALUES(1,1,?,?)").run(JSON.stringify(oldBinding), JSON.stringify(profile));
  db.prepare("INSERT INTO episodes VALUES(?,?,?,?,?)").run(3, "old-event", 100, 1, JSON.stringify({ kind: "decision", action: "wait", summary: "旧经历" }));
  db.prepare("INSERT INTO episodes VALUES(?,?,?,?,?)").run(7, "old-teaching-event", 200, 2, JSON.stringify(episode));
  db.prepare("INSERT INTO feedback VALUES(?,?,?,?)").run("legacy-lesson", JSON.stringify(payload), JSON.stringify(profile), "old-teaching-event");
  if (failUpdate) db.exec("CREATE TRIGGER fail_migration BEFORE UPDATE ON companion_state BEGIN SELECT RAISE(ABORT,'migration interrupted'); END;");
  db.close(); return oldBinding;
}

test("schema 1 migrates once, preserves IDs/order/teaching/version, and requires explicit owner/epoch adoption", t => {
  const f = fixture(t); legacy(f.path);
  assert.throws(() => f.open(), /legacy adoption/);
  const store = f.open({ adoptLegacy: true });
  assert.equal(store.profile().schemaVersion, 3); assert.equal(store.profile().policyVersion, 2);
  assert.deepEqual(store.growth().episodes.map(row => [row.episodeId, row.sequence, row.recordedAt]), [["old-event", 3, 100], ["old-teaching-event", 7, 200]]);
  assert.equal(store.growth().feedback[0].feedbackId, "legacy-lesson");
  assert.equal(store.recentEpisodes()[0].feedbackText, "原先真实教学");
  const identity = store.archive(); store.close();
  assert.deepEqual(f.open().archive(), identity);
  assert.equal(f.open().profile().policyVersion, 2);
});

test("legacy migration and trusted adoption rollback atomically without changing an existing owner", t => {
  const f = fixture(t); legacy(f.path, { failUpdate: true });
  assert.throws(() => f.open({ adoptLegacy: true }), /migration interrupted/);
  const db = new DatabaseSync(f.path);
  assert.equal(db.prepare("PRAGMA user_version").get().user_version, 1);
  assert.equal(db.prepare("SELECT COUNT(*) AS n FROM episodes").get().n, 2);
  assert.equal(db.prepare("SELECT COUNT(*) AS n FROM pragma_table_info('episodes') WHERE name='status'").get().n, 0);
  db.exec("DROP TRIGGER fail_migration"); db.close();
  assert.throws(() => f.open({ adoptLegacy: true, ownerRef: "owner:foreign" }), /operator/);
  assert.equal(f.open({ adoptLegacy: true }).profile().policyVersion, 2);
});

test("legacy B2 constructor still opens and creates one persisted identity without claiming an epoch", t => {
  const f = fixture(t); legacy(f.path);
  let store = new CompanionStore({ path: f.path, binding }); f.stores.push(store);
  const id = store.archive().archiveId; assert.equal(store.archive().worldEpoch, null);
  assert.throws(() => store.exportArchive({ key }), /Trusted owner/);
  store.close(); store = new CompanionStore({ path: f.path, binding }); f.stores.push(store);
  assert.equal(store.archive().archiveId, id);
  assert.throws(() => new CompanionStore({ path: f.path, binding, archiveId: id, ownerRef: trusted.ownerRef, worldEpoch: trusted.worldEpoch }), /binding/);
  const adopted = new CompanionStore({ path: f.path, binding, archiveId: id, ownerRef: trusted.ownerRef, worldEpoch: trusted.worldEpoch, adoptLegacy: true }); f.stores.push(adopted);
  assert.equal(adopted.archive().worldEpoch, trusted.worldEpoch);
});

test("trusted owner mutations require matching archive, owner, epoch, operation ID and policy version", t => {
  const store = fixture(t).open();
  for (const changes of [{ ownerRef: "owner:other" }, { archiveId: "archive-other" }, { worldEpoch: "epoch-other" }, { operationId: undefined }, { expectedPolicyVersion: undefined }]) {
    assert.throws(() => store.teachSupportBeforeLoot(gateway("first", 1, changes)));
  }
  assert.equal(store.teachSupportBeforeLoot(gateway("first", 1)).policyVersion, 2);
  assert.equal(store.teachSupportBeforeLoot(gateway("first", 1)).policyVersion, 2, "retry precedes version conflict detection");
  assert.throws(() => store.revokeFeedback(gateway("stale-revoke", 1, { feedbackId: "first" })), error => error.code === "conflict" && error.currentView.profile.policyVersion === 2);
  assert.equal(store.growth().feedback.length, 1);
});

test("revocation recomputes all active lessons, stays idempotent, and replayed IDs cannot reactivate", t => {
  const store = fixture(t).open();
  const lesson = { source: "local-owner", operationId: "op-one", feedbackId: "one", feedbackText: "同一条教学", expectedPolicyVersion: 1 };
  store.teachSupportBeforeLoot(lesson);
  store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "two" });
  store.revokeFeedback({ operationId: "revoke-one", feedbackId: "one", expectedPolicyVersion: 2 });
  assert.equal(store.profile().party.assistBeforeLoot, true);
  const revoke = { operationId: "revoke-two", feedbackId: "two", expectedPolicyVersion: 2 };
  store.revokeFeedback(revoke); const revision = store.growth().growthRevision;
  assert.equal(store.profile().party.assistBeforeLoot, false); assert.equal(store.profile().policyVersion, 3);
  store.revokeFeedback(revoke); assert.equal(store.growth().growthRevision, revision);
  assert.equal(store.teachSupportBeforeLoot(lesson).party.assistBeforeLoot, false);
  assert.equal(store.growth().feedback.length, 2);
  assert.throws(() => store.deleteFeedback({ ...revoke, feedbackId: "one" }), /operationId conflicts/);
});

test("deleting feedback erases every private text copy but keeps non-resurrectable tombstones", t => {
  const f = fixture(t), store = f.open(), marker = "这段错误教学必须删除";
  const lesson = { source: "local-owner", operationId: "teach-private", feedbackId: "private", feedbackText: marker };
  store.teachSupportBeforeLoot(lesson);
  store.deleteFeedback({ operationId: "delete-private", feedbackId: "private", expectedPolicyVersion: 2 });
  assert.equal(store.profile().policyVersion, 3);
  assert.equal(store.growth().feedback[0].payload, null);
  assert.equal(store.growth().episodes[0].payload, null);
  assert.equal(JSON.stringify(store.ownerView()).includes(marker), false);
  assert.equal(store.exportArchive({ key }).includes(marker), false);
  const db = new DatabaseSync(f.path);
  for (const { name } of db.prepare("SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'").all()) {
    assert.equal(JSON.stringify(db.prepare('SELECT * FROM "' + name + '"').all()).includes(marker), false, name);
  }
  db.close();
  store.teachSupportBeforeLoot(lesson);
  assert.equal(store.profile().party.assistBeforeLoot, false);
  assert.equal(store.growth().feedback.length, 1);
});

test("source episode deletion revokes teaching; ordinary episode deletion preserves other habits", t => {
  const store = fixture(t).open();
  store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "lesson" });
  store.recordEpisode({ episodeId: "ordinary", kind: "decision", summary: "错误的经历" });
  store.deleteEpisode({ operationId: "delete-event", episodeId: "ordinary", expectedPolicyVersion: 2 });
  assert.equal(store.profile().party.assistBeforeLoot, true);
  const source = store.growth().feedback[0].episodeId;
  store.deleteEpisode({ operationId: "delete-source", episodeId: source, expectedPolicyVersion: 2 });
  assert.equal(store.profile().party.assistBeforeLoot, false);
  assert.equal(store.growth().feedback[0].status, "deleted");
  assert.equal(store.recordEpisode({ episodeId: "ordinary", kind: "decision", summary: "错误的经历" }).status, "deleted");
});

test("full export/pagination exceed 100 rows and portable import preserves exact committed growth", t => {
  const f = fixture(t), a = f.open();
  for (let index = 0; index < 126; index += 1) a.recordEpisode({ episodeId: "event-" + index, kind: "decision", action: "wait", summary: "真实经历 " + index });
  a.teachSupportBeforeLoot(gateway("teaching", 1));
  assert.equal(a.recentEpisodes(1000).length, 100);
  const ids = []; let cursor = null;
  do { const view = a.ownerView({ cursor, limit: 50 }); ids.push(...view.episodes.map(row => row.episodeId)); cursor = view.nextCursor; } while (cursor);
  assert.equal(new Set(ids).size, 127);
  const archive = a.archive(), growth = a.growth(), envelope = a.exportArchive({ key }); a.close();
  const b = f.import(envelope);
  assert.deepEqual(b.archive(), archive); assert.deepEqual(b.growth(), growth);
  assert.deepEqual(b.growth().relations, []);
  assert.equal(b.profile().permissions.spendLimit, 0);
  assert.deepEqual(b.profile().permissions.mapAllowlist, ["0", "0141"]);
  const encoded = JSON.parse(envelope);
  assert.equal(encoded.playerGrowth.episodes.length, 127);
  for (const name of ["runtimeService", "runtimeId", "provider", "model", "promptVersion", "credentialRef", "memoryPath", "guardPort", "password", "inventoryItems", "gold", "playerHp"]) assert.equal(Object.hasOwn(encoded, name) || Object.hasOwn(encoded.characterArchive, name) || Object.hasOwn(encoded.playerGrowth, name), false, name);
});

test("imports verify external HMAC and every expected binding before touching a fresh target", t => {
  const f = fixture(t), store = f.open(); store.teachSupportBeforeLoot(gateway("one", 1));
  const envelope = store.exportArchive({ key });
  for (const changes of [{ key: Buffer.alloc(32, 8) }, { ownerRef: "owner:other" }, { archiveId: "foreign" }, { worldEpoch: "old-world" },
    { binding: { ...binding, realmId: "another" } }, { binding: { ...binding, characterIndex: 13 } }, { binding: { ...binding, characterName: "Other" } }]) assert.throws(() => f.import(envelope, changes));
  const tampered = JSON.parse(envelope); tampered.playerGrowth.habits.party.assistBeforeLoot = false;
  assert.throws(() => f.import(tampered), /signature/);
  const foreign = JSON.parse(envelope); foreign.runtimeService = { credentialRef: "not-portable" };
  assert.throws(() => f.import(foreign), /unsupported fields/);
  assert.throws(() => f.import("x".repeat(MAX_ARCHIVE_BYTES + 1)), /limit/);
  assert.throws(() => store.exportArchive({ key: "short" }), /32/);
});

test("signed malformed references, extra powers and duplicate IDs are rejected without partial import", t => {
  const f = fixture(t), store = f.open(); store.teachSupportBeforeLoot(gateway("one", 1));
  const source = JSON.parse(store.exportArchive({ key }));
  for (const alter of [
    growth => growth.episodes.push(structuredClone(growth.episodes[0])),
    growth => { growth.feedback[0].episodeId = "absent"; },
    growth => { growth.habits.permissions = { spendLimit: 1000 }; },
    growth => { growth.habits.party.assistBeforeLoot = false; },
    growth => { growth.relations = [{ relationId: "fake", subjectRef: "player:someone", kind: "configured-owner", sourceEpisodeIds: ["absent"] }]; },
  ]) {
    const growth = structuredClone(source.playerGrowth); alter(growth);
    const signed = signArchive({ characterArchive: source.characterArchive, playerGrowth: growth, key });
    assert.throws(() => f.import(signed));
  }
  const valid = f.import(store.exportArchive({ key })); assert.equal(valid.profile().policyVersion, 2);
});

test("exact import retry is idempotent but cannot merge, overwrite, or undo clearGrowth", t => {
  const f = fixture(t), store = f.open(); store.teachSupportBeforeLoot(gateway("lesson", 1));
  const envelope = store.exportArchive({ key }); store.close();
  const b = f.import(envelope); const before = b.growth(); b.close();
  const retry = f.import(envelope); assert.deepEqual(retry.growth(), before);
  assert.throws(() => retry.clearGrowth({ operationId: "clear", confirmArchiveId: "wrong", expectedPolicyVersion: 2 }), /confirmation/);
  const identity = retry.archive();
  retry.clearGrowth({ operationId: "clear", confirmArchiveId: identity.archiveId, expectedPolicyVersion: 2 });
  assert.deepEqual(retry.archive(), identity); assert.equal(retry.profile().party.assistBeforeLoot, false);
  assert.equal(retry.growth().episodes.every(row => row.status === "deleted"), true);
  retry.close(); assert.throws(() => f.import(envelope), /roll back/);
  assert.throws(() => f.import(envelope, { path: f.path }), /fresh destination/);
});

test("owner view stays inside a WebSocket frame and reveals no raw payload or runtime data", t => {
  const store = fixture(t).open();
  for (let index = 0; index < 50; index += 1) store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "feedback-" + index, feedbackText: "教".repeat(500), context: { leaderRef: "玩家".repeat(70), mode: "party" } });
  const view = store.ownerView({ limit: 50 });
  assert.ok(Buffer.byteLength(JSON.stringify(view)) < 60 * 1024);
  assert.equal(view.feedback.length, 50);
  assert.equal(Object.hasOwn(view.episodes[0], "feedbackText"), false);
  assert.throws(() => store.ownerView({ limit: 51 }), /pagination/);
  assert.throws(() => store.ownerView({ cursor: "not-a-cursor" }), /pagination/);
});

test("different actual Node processes export and import the same identity and learned habits", t => {
  const f = fixture(t), aPath = f.path, bPath = join(f.root, "different-runtime", "memory.sqlite"), backup = join(f.root, "archive.json");
  const moduleUrl = new URL("../src/memory/companion-store.mjs", import.meta.url).href;
  const args = JSON.stringify({ binding, ...trusted });
  execFileSync(process.execPath, ["--input-type=module", "-e", "import{writeFileSync}from'node:fs';import{CompanionStore}from'" + moduleUrl + "';const s=new CompanionStore({path:" + JSON.stringify(aPath) + ",..." + args + "});s.teachSupportBeforeLoot({source:'local-owner',feedbackId:'persisted'});writeFileSync(" + JSON.stringify(backup) + ",s.exportArchive({key:Buffer.alloc(32,7)}));s.close();"], { windowsHide: true, stdio: "pipe" });
  const output = execFileSync(process.execPath, ["--input-type=module", "-e", "import{readFileSync}from'node:fs';import{CompanionStore}from'" + moduleUrl + "';const s=CompanionStore.importArchive({path:" + JSON.stringify(bPath) + ",..." + args + ",key:Buffer.alloc(32,7),envelope:readFileSync(" + JSON.stringify(backup) + ",'utf8')});console.log(JSON.stringify({id:s.archive().archiveId,policy:s.profile().policyVersion,assist:s.profile().party.assistBeforeLoot,count:s.growth().episodes.length}));s.close();"], { windowsHide: true, encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] });
  assert.deepEqual(JSON.parse(output.trim()), { id: trusted.archiveId, policy: 2, assist: true, count: 1 });
  assert.equal(JSON.parse(readFileSync(backup, "utf8")).characterArchive.archiveId, trusted.archiveId);
});

test("trusted archive owner/world/identity remain immutable even under an adoption request", t => {
  const f = fixture(t), first = f.open(), before = first.archive(); first.close();
  for (const change of [{ ownerRef: "owner:foreign" }, { archiveId: "different-identity" }, { worldEpoch: "different-world" }]) {
    assert.throws(() => f.open({ ...change, adoptLegacy: true }), /binding|operator/);
  }
  assert.deepEqual(f.open().archive(), before);
  assert.throws(() => f.open({ runtimeService: { model: "not-growth" } }), /unsupported/);
});

test("all feedback beyond recent history and all deletion tombstones survive export/import", t => {
  const f = fixture(t), store = f.open();
  for (let index = 0; index < 115; index += 1) store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "teaching-" + index });
  store.revokeFeedback({ operationId: "revoke-one", feedbackId: "teaching-0" });
  store.deleteFeedback({ operationId: "erase-two", feedbackId: "teaching-1" });
  const snapshot = store.exportArchive({ key }), imported = f.import(snapshot);
  assert.equal(imported.growth().feedback.length, 115);
  assert.equal(imported.growth().feedback[0].status, "revoked");
  assert.equal(imported.growth().feedback[1].status, "deleted");
  assert.equal(imported.growth().feedback[1].payload, null);
  imported.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "teaching-1" });
  assert.equal(imported.growth().feedback[1].status, "deleted");
});

test("an interrupted private deletion rolls back evidence, policy and operation receipt together", t => {
  const f = fixture(t), store = f.open();
  store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "rollback", feedbackText: "保留到事务成功" });
  const before = store.growth(), db = new DatabaseSync(f.path);
  db.exec("CREATE TRIGGER reject_deletion BEFORE UPDATE ON episodes BEGIN SELECT RAISE(ABORT,'deletion interrupted'); END;");
  assert.throws(() => store.deleteFeedback({ operationId: "delete-rollback", feedbackId: "rollback", expectedPolicyVersion: 2 }), /deletion interrupted/);
  assert.deepEqual(store.growth(), before);
  db.exec("DROP TRIGGER reject_deletion"); db.close();
  store.deleteFeedback({ operationId: "delete-rollback", feedbackId: "rollback", expectedPolicyVersion: 2 });
  assert.equal(store.growth().feedback[0].status, "deleted");
});

test("export rejects the full over-limit history instead of silently truncating or modifying it", t => {
  const store = fixture(t).open();
  for (let index = 0; index < 1400; index += 1) store.recordEpisode({ episodeId: "large-" + index, kind: "decision", summary: "经".repeat(500) });
  const revision = store.growth().growthRevision;
  assert.throws(() => store.exportArchive({ key }), /2 MiB.*not truncated/);
  assert.equal(store.growth().episodes.length, 1400);
  assert.equal(store.growth().growthRevision, revision);
});
