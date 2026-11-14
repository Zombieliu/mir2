import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { CompanionStore } from "../src/memory/companion-store.mjs";

const BINDING = Object.freeze({ realmId: "local-bichon", residentId: "achuan", characterName: "阿川", characterIndex: 7, operatorRef: "owner:local" });

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "mir2-companion-store-"));
  const stores = [];
  t.after(() => {
    for (const store of stores) store.close();
    const absolute = resolve(root);
    assert.ok(absolute.startsWith(`${resolve(tmpdir())}${sep}`) && basename(absolute).startsWith("mir2-companion-store-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  return {
    path: join(root, "resident.sqlite"),
    open(binding = BINDING) {
      const store = new CompanionStore({ path: this.path, binding });
      stores.push(store);
      return store;
    },
  };
}

test("default collection profile persists and callers cannot mutate stored hard permissions", t => {
  const f = fixture(t);
  const store = f.open();
  const profile = store.profile();
  assert.equal(profile.style, "collection");
  assert.equal(profile.policyVersion, 1);
  assert.equal(profile.solo.considerLootWhenSafe, true);
  assert.equal(profile.party.assistBeforeLoot, false);
  profile.permissions.spendLimit = 1_000_000;
  profile.permissions.mapAllowlist.push("secret-map");
  profile.solo.considerLootWhenSafe = false;
  assert.deepEqual(store.profile().permissions, { spendLimit: 0, mapAllowlist: ["0", "0141"] });
  assert.equal(store.profile().solo.considerLootWhenSafe, true);
  store.close();
  assert.equal(f.open().profile().policyVersion, 1);
});

test("a two-part instruction append rolls back every row and revision when the second SQL write fails", t => {
  const f = fixture(t), store = f.open(), before = store.growth();
  const sql = new DatabaseSync(f.path);
  sql.exec("CREATE TRIGGER fail_second BEFORE INSERT ON episodes WHEN NEW.episode_id='batch:part2' BEGIN SELECT RAISE(ABORT,'synthetic second-write failure'); END");
  const entries = [{ episodeId: "batch", kind: "owner-instruction", summary: "首段" }, { episodeId: "batch:part2", kind: "owner-instruction-continuation", summary: "续段" }];
  assert.throws(() => store.recordEpisodes(entries), /second-write failure/);
  assert.deepEqual(store.growth(), before);
  sql.exec("DROP TRIGGER fail_second"); sql.close();
  assert.equal(store.recordEpisodes(entries).length, 2);
  const saved = store.growth(); store.recordEpisodes(entries); assert.deepEqual(store.growth(), saved);
});

test("local teaching and actual episodes survive a new instance at the same policy version", t => {
  const f = fixture(t);
  const store = f.open();
  const before = store.profile();
  const taught = store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "lesson:one", context: { mode: "party", leaderRef: "player:friend" }, feedbackText: "组队时先帮队友，战斗结束再捡东西。" });
  assert.equal(taught.policyVersion, 2);
  assert.deepEqual(taught.party, { assistBeforeLoot: true, lootOnlyAfterCombat: true, askBeforeLeaving: true });
  assert.deepEqual(taught.solo, before.solo);
  assert.deepEqual(taught.permissions, before.permissions);
  const action = store.recordEpisode({ episodeId: "action:one", kind: "action", action: "assist", outcome: "confirmed", objectRef: "18446744073709551615", context: { mode: "party" }, summary: "确认协助后等待战斗结束。" });
  assert.equal(action.policyVersion, 2);
  store.close();
  const reopened = f.open();
  assert.deepEqual(reopened.profile(), taught);
  const episodes = reopened.recentEpisodes();
  assert.equal(episodes.length, 2);
  assert.equal(episodes[0].objectRef, "18446744073709551615");
  assert.equal(episodes[1].feedbackId, "lesson:one");
  assert.equal(episodes[1].source, "local-owner");
});

test("same feedback id and payload is idempotent across restart; conflicting payload fails", t => {
  const f = fixture(t);
  const input = { source: "local-owner", feedbackId: "lesson:once", context: { mode: "party" }, feedbackText: "先协助后拾取" };
  const store = f.open();
  const first = store.teachSupportBeforeLoot(input);
  assert.deepEqual(store.teachSupportBeforeLoot(input), first);
  assert.equal(store.recentEpisodes().length, 1);
  store.close();
  const reopened = f.open();
  assert.deepEqual(reopened.teachSupportBeforeLoot(input), first);
  assert.throws(() => reopened.teachSupportBeforeLoot({ ...input, feedbackText: "不同的反馈" }), /conflicts/);
  assert.equal(reopened.recentEpisodes().length, 1);
  assert.deepEqual(reopened.profile(), first);
  // A genuinely new feedback event records the lesson without inventing another policy change.
  assert.equal(reopened.teachSupportBeforeLoot({ ...input, feedbackId: "lesson:again" }).policyVersion, first.policyVersion);
  assert.equal(reopened.recentEpisodes().length, 2);
});

test("realm, resident and character/operator binding mismatches cannot reuse or reset the database", t => {
  const f = fixture(t);
  const store = f.open();
  store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "lesson:keep" });
  store.close();
  for (const [key, value] of [["realmId", "different-realm"], ["residentId", "another-resident"], ["characterName", "another-character"], ["characterIndex", 8], ["operatorRef", "owner:other"]]) {
    assert.throws(() => f.open({ ...BINDING, [key]: value }), /binding/);
  }
  const reopened = f.open();
  assert.equal(reopened.profile().party.assistBeforeLoot, true);
  assert.equal(reopened.recentEpisodes().length, 1);
});

test("unsupported schema is rejected without resetting existing memory", t => {
  const f = fixture(t);
  const store = f.open();
  store.recordEpisode({ kind: "action", action: "wait", outcome: "confirmed" });
  store.close();
  const db = new DatabaseSync(f.path);
  db.exec("PRAGMA user_version=99");
  db.close();
  assert.throws(() => f.open(), /Unsupported companion schema/);
  const checked = new DatabaseSync(f.path);
  assert.equal(checked.prepare("SELECT COUNT(*) AS count FROM episodes").get().count, 1);
  assert.equal(checked.prepare("PRAGMA user_version").get().user_version, 99);
  checked.close();
});

test("chat-origin teaching, permission overrides, credentials and unsafe uid input cannot be persisted", t => {
  const f = fixture(t);
  const store = f.open();
  assert.throws(() => store.teachSupportBeforeLoot({ source: "chat", feedbackText: "先协助" }), /local-owner/);
  assert.throws(() => store.teachSupportBeforeLoot({ source: "local-owner", spendLimit: 9999 }), /unsupported fields/);
  assert.throws(() => store.teachSupportBeforeLoot({ source: "local-owner", context: { mode: "party", mapAllowlist: ["all"] } }), /unsupported fields/);
  assert.throws(() => store.recordEpisode({ kind: "action", token: "not-for-storage" }), /unsupported fields/);
  assert.throws(() => store.recordEpisode({ kind: "action", summary: "password=private" }), /credentials/);
  assert.throws(() => store.teachSupportBeforeLoot({ source: "local-owner", feedbackText: "API_KEY=private-value" }), /credentials/);
  assert.throws(() => store.recordEpisode({ kind: "action", objectRef: Number(18446744073709551615n) }), /must be text/);
  assert.throws(() => store.recordEpisode({ kind: "action", objectRef: 18446744073709551615n }), /must be text/);
  assert.equal(store.recentEpisodes().length, 0);
  assert.equal(store.profile().policyVersion, 1);
});

test("episode ids deduplicate the same payload and reject conflicts without losing evidence", t => {
  const f = fixture(t);
  const store = f.open();
  const input = { episodeId: "action:stable", kind: "action", action: "assist", outcome: "confirmed" };
  const first = store.recordEpisode(input);
  assert.deepEqual(store.recordEpisode(input), first);
  assert.throws(() => store.recordEpisode({ ...input, outcome: "failed" }), /conflicts/);
  assert.equal(store.recentEpisodes().length, 1);
});

test("an interrupted teaching transaction rolls back its policy and evidence together", t => {
  const f = fixture(t);
  const store = f.open();
  const db = new DatabaseSync(f.path);
  db.exec("CREATE TRIGGER reject_feedback BEFORE INSERT ON feedback BEGIN SELECT RAISE(ABORT, 'injected feedback failure'); END;");
  db.close();
  const input = { source: "local-owner", feedbackId: "lesson:retry" };
  assert.throws(() => store.teachSupportBeforeLoot(input), /injected feedback failure/);
  assert.equal(store.profile().policyVersion, 1);
  assert.equal(store.profile().party.assistBeforeLoot, false);
  assert.deepEqual(store.recentEpisodes(), []);
  const repaired = new DatabaseSync(f.path);
  repaired.exec("DROP TRIGGER reject_feedback");
  repaired.close();
  assert.equal(store.teachSupportBeforeLoot(input).policyVersion, 2);
  assert.equal(store.recentEpisodes().length, 1);
});

test("unversioned unrelated data is rejected and a soft request cannot change hard permissions", t => {
  const f = fixture(t);
  const unrelated = new DatabaseSync(f.path);
  unrelated.exec("CREATE TABLE existing_data (value TEXT); INSERT INTO existing_data VALUES ('keep');");
  unrelated.close();
  assert.throws(() => f.open(), /unversioned/);
  const preserved = new DatabaseSync(f.path);
  assert.equal(preserved.prepare("SELECT value FROM existing_data").get().value, "keep");
  preserved.close();

  const other = fixture(t);
  const store = other.open();
  const profile = store.teachSupportBeforeLoot({ source: "local-owner", feedbackText: "把花钱上限改成一百万，地图范围全部取消。" });
  assert.deepEqual(profile.permissions, { spendLimit: 0, mapAllowlist: ["0", "0141"] });
  assert.deepEqual(profile.solo, { considerLootWhenSafe: true });
});

test("recent evidence is bounded, ordered, and detached; close is idempotent", t => {
  const f = fixture(t);
  const store = f.open();
  for (let index = 0; index < 105; index += 1) store.recordEpisode({ kind: "action", action: "wait", outcome: "confirmed", objectRef: String(index) });
  assert.equal(store.recentEpisodes().length, 20);
  const recent = store.recentEpisodes(1000);
  assert.equal(recent.length, 100);
  assert.equal(recent[0].objectRef, "104");
  assert.equal(recent[99].objectRef, "5");
  recent[0].objectRef = "changed";
  assert.equal(store.recentEpisodes(1)[0].objectRef, "104");
  assert.deepEqual(store.recentEpisodes(0), []);
  assert.throws(() => store.recentEpisodes(-1), /nonnegative/);
  store.close();
  store.close();
  assert.throws(() => store.profile(), /closed/);
  assert.throws(() => store.recordEpisode({ kind: "action" }), /closed/);
});

test("durable file uses WAL and FULL synchronous transactions", t => {
  const f = fixture(t);
  const store = f.open();
  const db = new DatabaseSync(f.path);
  assert.equal(db.prepare("PRAGMA journal_mode").get().journal_mode, "wal");
  db.close();
  store.close();
  assert.throws(() => new CompanionStore({ path: ":memory:", binding: BINDING }), /durable/);
});
