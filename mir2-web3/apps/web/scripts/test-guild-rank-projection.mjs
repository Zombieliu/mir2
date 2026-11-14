import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

const sourceUrl = new URL("../lib/guild-rank-projection.ts", import.meta.url);
const sourcePath = fileURLToPath(sourceUrl);
function evaluate(url, dependencies = {}) {
  const result = ts.transpileModule(readFileSync(url, "utf8"), {
    fileName: fileURLToPath(url), reportDiagnostics: true,
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, strict: true },
  });
  assert.deepEqual((result.diagnostics ?? []).filter(row => row.category === ts.DiagnosticCategory.Error), []);
  const module = { exports: {} };
  new Function("require", "module", "exports", result.outputText)(id => {
    assert.ok(Object.hasOwn(dependencies, id), "Unexpected runtime dependency: " + id);
    return dependencies[id];
  }, module, module.exports);
  return module.exports;
}
const social = evaluate(new URL("../lib/social-parity-actions.ts", import.meta.url));
const { applyGuildMemberChange } = evaluate(sourceUrl, { "./social-parity-actions": social });
const viewer = { name: "Alice", index: 102 };
const member = (name, id, online = true) => ({ name, id, online,
  lastLoginBinaryDatetime: 123, hasVoted: false });
const rank = (index, name, options, members = []) => ({ index, name, options, members });
const full = () => [rank(0, "Leader", 255, [member("Leader", 100)]),
  rank(1, "Officer", 65, [member("Officer", 101)]),
  rank(2, "Scout", 1, [member("Alice", 102), member("Bob", 103, false)])];
// Real GuildMemberChange fields. There is deliberately no fabricated changeType.
const packet = (status, name = "Leader", ranks = [], rankIndex = 0) => ({ name, rankIndex, status, ranks });
const projection = () => social.projectGuildRanks(full());
const apply = (previous, payload, self = viewer) => {
  assert.equal(Object.hasOwn(payload, "changeType"), false);
  const result = applyGuildMemberChange(previous, payload, self);
  assert.equal(result.kind, "applied");
  return result;
};
const rejected = (previous, payload, self = viewer) =>
  assert.deepEqual(applyGuildMemberChange(previous, payload, self), { kind: "invalid" });
const own = (result, changes = {}) => assert.deepEqual(result.evidence.ownRank, {
  kind: "confirmed", index: 2, name: "Scout", permissions: ["CanChangeRank"], changed: false, ...changes,
});
function freeze(value) {
  if (value && typeof value === "object") { Object.freeze(value); Object.values(value).forEach(freeze); }
  return value;
}

test("actual adapter type checks with its real projection dependency", () => {
  const program = ts.createProgram([sourcePath], { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler, strict: true, noEmit: true, skipLibCheck: true, types: [] });
  assert.deepEqual(ts.getPreEmitDiagnostics(program).map(row => ts.flattenDiagnosticMessageText(row.messageText, "\n")), []);
});

test("status255 alone replaces the complete roster and confirms exact viewer identity", () => {
  const result = apply(null, packet(255, "", full()));
  assert.deepEqual(result.projection, projection());
  assert.equal(result.evidence.fullRoster, true);
  assert.equal(result.evidence.needsFullRoster, false);
  assert.equal(result.evidence.guildLeft, false);
  own(result, { changed: true });
  const mismatch = apply(null, packet(255, "", full()), { ...viewer, index: 999 });
  assert.deepEqual(mismatch.evidence.ownRank, { kind: "unknown" });
  const empty = apply(projection(), packet(255, "", []));
  assert.deepEqual(empty.projection, { ranks: [], members: [] });
  assert.equal(empty.evidence.guildLeft, false, "empty roster alone is not a GuildStatus leave receipt");
  assert.deepEqual(empty.evidence.ownRank, { kind: "unknown" });
});

test("real status7 single-rank packet replaces its index, never top-level zero or the full table", () => {
  const before = projection(), updated = full()[2];
  updated.name = "Rangers";
  updated.options = 2;
  const result = apply(before, packet(7, "Leader", [updated], 0));
  assert.equal(result.projection.ranks.length, 3);
  assert.deepEqual(result.projection.ranks.slice(0, 2), before.ranks.slice(0, 2));
  assert.deepEqual(result.projection.members.filter(row => row.rankIndex < 2), before.members.filter(row => row.rankIndex < 2));
  assert.equal(result.projection.members.find(row => row.id === 102).rank, "Rangers");
  assert.equal(result.evidence.fullRoster, false);
  assert.equal(result.evidence.needsFullRoster, false);
  own(result, { name: "Rangers", permissions: ["CanRecruit"], changed: true });
  assert.deepEqual(before, projection(), "input was not edited");
});

test("unrelated status7 retains own authority; missing member removes own proof without claiming guild leave", () => {
  const unrelated = apply(projection(), packet(7, "Leader", [rank(1, "Officers", 2, [member("Officer", 101)])]));
  assert.deepEqual(unrelated.evidence.ownRank, { kind: "unchanged" });
  const missing = apply(projection(), packet(7, "Leader", [rank(2, "Scout", 0, [])]));
  assert.deepEqual(missing.projection.members.map(row => row.name), ["Leader", "Officer"]);
  assert.deepEqual(missing.evidence.ownRank, { kind: "unknown" });
  assert.equal(missing.evidence.guildLeft, false);
});

test("status7 requires an existing unique target rank and never upgrades a partial or unknown roster", () => {
  const payload = packet(7, "Leader", [rank(7, "Unknown", 255, [member("Alice", 102)])]);
  const before = projection(), result = apply(before, payload);
  assert.deepEqual(result.projection, before);
  assert.equal(result.evidence.needsFullRoster, true);
  assert.equal(result.evidence.fullRoster, false);
  assert.deepEqual(result.evidence.ownRank, { kind: "unknown" });
  const unknown = apply(null, payload);
  assert.equal(unknown.projection, null);
  assert.deepEqual(unknown.evidence.ownRank, { kind: "unknown" });
  const partial = social.projectGuildRanks([full()[2]]);
  const patched = apply(partial, packet(7, "Leader", [rank(2, "Scout", 0, full()[2].members)]));
  assert.equal(patched.projection.ranks.length, 1);
  assert.equal(patched.evidence.fullRoster, false, "a usable delta does not establish completeness");
});

test("status0/1 only change the named member online field, including exact case and unknown names", () => {
  const before = projection();
  for (const [status, online] of [[0, false], [1, true]]) {
    const result = apply(before, packet(status, "Bob"));
    const bob = result.projection.members.find(row => row.name === "Bob");
    assert.equal(bob.online, online);
    assert.deepEqual(result.projection.ranks, before.ranks);
    assert.deepEqual(result.evidence.ownRank, { kind: "unchanged" });
    assert.equal(result.evidence.needsFullRoster, false);
  }
  for (const subject of ["bob", "Unknown"]) {
    const result = apply(before, packet(1, subject));
    assert.deepEqual(result.projection, before);
    assert.equal(result.evidence.needsFullRoster, true);
  }
});

test("status2 has only a name and requests a roster instead of inventing id or rank", () => {
  const before = projection(), result = apply(before, packet(2, "NewMember"));
  assert.deepEqual(result.projection, before);
  assert.equal(result.evidence.needsFullRoster, true);
  assert.equal(result.evidence.fullRoster, false);
  assert.equal(apply(null, packet(2, "NewMember")).projection, null);
});

test("status3/4 remove only the named member, with explicit viewer leave evidence for self", () => {
  for (const status of [3, 4]) {
    const before = projection(), result = apply(before, packet(status, "Bob"));
    assert.deepEqual(result.projection.ranks, before.ranks);
    assert.deepEqual(result.projection.members.map(row => row.name), ["Leader", "Officer", "Alice"]);
    assert.equal(result.evidence.guildLeft, false);
    assert.deepEqual(result.evidence.ownRank, { kind: "unchanged" });
    const self = apply(before, packet(status, "Alice"));
    assert.deepEqual(self.projection, { ranks: [], members: [] });
    assert.equal(self.evidence.guildLeft, true);
    assert.deepEqual(self.evidence.ownRank, { kind: "left" });
    assert.equal(self.evidence.needsFullRoster, false);
    const unknownViewer = apply(before, packet(status, "Alice"), null);
    assert.equal(unknownViewer.evidence.guildLeft, false);
    assert.equal(unknownViewer.projection.ranks.length, 3);
  }
});

test("status5 uses target name and actual rankIndex while preserving every rank and other member", () => {
  const before = projection(), result = apply(before, packet(5, "Bob", [], 1));
  const bob = result.projection.members.find(row => row.name === "Bob");
  assert.deepEqual(bob, { name: "Bob", id: 103, rank: "Officer", rankIndex: 1, online: false });
  assert.deepEqual(result.projection.ranks, before.ranks);
  assert.deepEqual(result.projection.members.filter(row => row.name !== "Bob"), before.members.filter(row => row.name !== "Bob"));
  for (const payload of [packet(5, "Unknown", [], 1), packet(5, "Bob", [], 9)]) {
    const missing = apply(before, payload);
    assert.deepEqual(missing.projection, before);
    assert.equal(missing.evidence.needsFullRoster, true);
  }
  assert.deepEqual(apply(before, packet(5, "Alice", [], 0)).evidence.ownRank, { kind: "unknown" },
    "peer change packet cannot grant own leader rights; current Gateway sends status8 to self");
});

test("real status6 inserts before the lowest rank and shifts its members, not their permissions", () => {
  const before = projection(), result = apply(before, packet(6, "Leader", [rank(2, "Rank 2", 0)]));
  assert.deepEqual(result.projection.ranks.map(row => [row.index, row.name]),
    [[0, "Leader"], [1, "Officer"], [2, "Rank 2"], [3, "Scout"]]);
  assert.deepEqual(result.projection.members.filter(row => row.rankIndex === 3).map(row => row.name), ["Alice", "Bob"]);
  own(result, { index: 3, changed: true });
  assert.equal(result.evidence.fullRoster, false);
  assert.deepEqual(before, projection());
  const leaderOnly = social.projectGuildRanks([full()[0]]);
  const first = apply(leaderOnly, packet(6, "Leader", [rank(1, "Rank 1", 0)]), { name: "Leader", index: 100 });
  assert.deepEqual(first.projection.ranks.map(row => row.index), [0, 1]);
});

test("status6 cannot guess insertion into unknown/sparse state or introduce members/privileges", () => {
  const payload = packet(6, "Leader", [rank(2, "Rank 2", 0)]);
  assert.equal(apply(null, payload).projection, null);
  const sparse = social.projectGuildRanks([full()[0], full()[2]]);
  const result = apply(sparse, payload);
  assert.deepEqual(result.projection, sparse);
  assert.equal(result.evidence.needsFullRoster, true);
  assert.deepEqual(result.evidence.ownRank, { kind: "unknown" });
  rejected(projection(), packet(6, "Leader", [rank(1, "Wrong insertion", 0)]));
  rejected(projection(), packet(6, "Leader", [rank(2, "Rank 2", 255)]));
  rejected(projection(), packet(6, "Leader", [rank(2, "Rank 2", 0, [member("New", 900)])]));
});

test("status8 uses viewer id+name in its rank snapshot, not the actor or top-level index", () => {
  const moved = rank(1, "Officer", 65, [member("Officer", 101), member("Alice", 102)]);
  const before = projection(), result = apply(before, packet(8, "Leader", [moved], 0));
  assert.equal(result.projection.ranks.length, 3);
  assert.equal(result.projection.members.filter(row => row.id === 102).length, 1);
  assert.equal(result.projection.members.find(row => row.id === 102).rankIndex, 1);
  assert.equal(result.projection.members.find(row => row.id === 103).rankIndex, 2);
  own(result, { index: 1, name: "Officer", permissions: ["CanChangeRank", "CanChangeNotice"], changed: true });
  assert.deepEqual(before, projection());
  rejected(before, packet(8, "Alice", [rank(1, "Officer", 255, [member("Alice", 999)])]));
  rejected(before, packet(8, "Leader", [rank(1, "Officer", 255, [member("alice", 102)])]));
});

test("status8 can retain an explicitly partial rank with own proof; missing viewer yields no own authority", () => {
  const payload = packet(8, "Leader", [rank(5, "New target", 2, [member("Alice", 102)])]);
  const result = apply(null, payload);
  assert.equal(result.projection.ranks.length, 1);
  assert.equal(result.evidence.fullRoster, false);
  assert.equal(result.evidence.needsFullRoster, true);
  own(result, { index: 5, name: "New target", permissions: ["CanRecruit"], changed: true });
  assert.deepEqual(apply(null, payload, null).evidence.ownRank, { kind: "unknown" });
});

test("rank snapshot repairs an exact moved identity but rejects id/name conflicts", () => {
  const result = apply(projection(), packet(7, "Leader", [rank(1, "Officer", 65,
    [member("Officer", 101), member("Alice", 102)])]));
  assert.equal(result.projection.members.filter(row => row.name === "Alice").length, 1);
  assert.equal(result.projection.members.find(row => row.id === 102).rankIndex, 1);
  rejected(projection(), packet(7, "Leader", [rank(1, "Officer", 65, [member("Alice", 999)])]));
  rejected(projection(), packet(7, "Leader", [rank(1, "Officer", 65, [member("Imposter", 102)])]));
});

test("unknown statuses and malformed real-wire fields are invalid without changing previous state", () => {
  const before = projection();
  for (const status of [-1, 9, 254, 256, 7.5, "7", null]) rejected(before, packet(status));
  for (const payload of [null, [], {}, { status: 7, changeType: 3, rankIndex: 0, name: "Leader" },
    packet(7, "Leader", []), packet(7, "Leader", full()), packet(0, "Bob", [full()[0]]),
    packet(255, "", [rank(0, "Bad", 256)]), packet(255, "", [rank(256, "Bad", 0)]),
    packet(255, "", [rank(0, "Bad", 0, [member("New", -1)])]), packet(255, "", [rank(0, "\0", 0)]),
    { ...packet(1, "Bob"), rankIndex: 256 }, { ...packet(1, "Bob"), rankIndex: undefined }]) rejected(before, payload);
  for (const self of [{ name: "", index: 102 }, { name: "Alice", index: -1 }, { name: "Alice", index: 1.5 }])
    rejected(before, packet(1, "Bob"), self);
  assert.deepEqual(before, projection());
});

test("duplicate rank/member identities, over-limit arrays and inconsistent previous state fail closed", () => {
  const duplicates = [
    [full()[0], { ...full()[1], index: 0 }],
    [rank(0, "A", 0, [member("A", 1), member("B", 1)])],
    [rank(0, "A", 0, [member("Alice", 1), member("alice", 2)])],
    Array.from({ length: 257 }, (_, index) => rank(index, "R" + index, 0)),
    [rank(0, "A", 0, Array.from({ length: 8193 }, (_, id) => member("M" + id, id)))],
  ];
  for (const ranks of duplicates) rejected(projection(), packet(255, "", ranks));
  const previous = projection(); previous.members[0].rank = "Imposter";
  rejected(previous, packet(1, "Bob"));
  assert.equal(apply(previous, packet(255, "", full())).evidence.fullRoster, true,
    "a valid authoritative full refresh can repair a malformed old projection");
});

test("accessors, exotic records and throwing proxies cannot supply authority", () => {
  let reads = 0;
  const payload = packet(255, "", full());
  Object.defineProperty(payload, "ranks", { get() { reads++; return full(); } });
  rejected(projection(), payload);
  assert.equal(reads, 0);
  const ranks = full();
  Object.defineProperty(ranks[0].members[0], "id", { get() { reads++; return 100; } });
  rejected(projection(), packet(255, "", ranks));
  assert.equal(reads, 0);
  rejected(projection(), Object.create({ ...packet(255, "", full()) }));
  rejected(projection(), new Proxy({}, { getPrototypeOf() { throw new Error("sensitive data"); } }));
  const sparse = new Array(1);
  rejected(projection(), packet(255, "", sparse));
});

test("viewer identity is captured once even when a caller supplies mutable property descriptors", () => {
  const reads = { name: 0, index: 0 };
  const self = new Proxy({ ...viewer }, { getOwnPropertyDescriptor(target, key) {
    if (Object.hasOwn(reads, key)) {
      reads[key]++;
      if (reads[key] > 1) return { ...Object.getOwnPropertyDescriptor(target, key), value: key === "name" ? "Leader" : 100 };
    }
    return Object.getOwnPropertyDescriptor(target, key);
  } });
  const result = apply(null, packet(255, "", full()), self);
  own(result, { changed: true });
  assert.deepEqual(reads, { name: 1, index: 1 });
});

test("opaque names and input objects are preserved exactly and outputs do not alias inputs", () => {
  const name = "龍城{View/CORAL}", raw = [rank(0, "首領{/CORAL}", 65, [member(name, 99)])];
  const payload = freeze(packet(255, "", raw));
  const result = apply(null, payload, { name, index: 99 });
  assert.equal(result.projection.members[0].name, name);
  assert.equal(result.evidence.ownRank.name, "首領{/CORAL}");
  result.projection.members[0].name = "changed";
  result.projection.ranks[0].permissions.length = 0;
  assert.equal(payload.ranks[0].members[0].name, name);
  assert.equal(payload.ranks[0].options, 65);
  assert.deepEqual(result.evidence.ownRank.permissions, ["CanChangeRank", "CanChangeNotice"]);
  const before = freeze(projection()), delta = freeze(packet(7, "Leader", [full()[2]]));
  const unchanged = apply(before, delta);
  own(unchanged);
  assert.notEqual(unchanged.projection, before);
});
