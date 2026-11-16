import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";
import ts from "typescript";

const sourceUrl = new URL("../lib/group-operations.ts", import.meta.url);
const sourcePath = fileURLToPath(sourceUrl);
const compiled = ts.transpileModule(readFileSync(sourceUrl, "utf8"), {
  fileName: sourcePath,
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS, strict: true },
  reportDiagnostics: true,
});
assert.deepEqual((compiled.diagnostics ?? []).filter(row => row.category === ts.DiagnosticCategory.Error), []);
const module = { exports: {} };
new Function("module", "exports", "require", compiled.outputText)(module, module.exports, id => {
  throw new Error("Unexpected runtime dependency: " + id);
});
const { GroupOperations, groupOperationCommand } = module.exports;

const owner = (changes = {}) => ({ socket: {}, connectionGeneration: 1, sessionGeneration: 2,
  sceneRevision: 3, playerObjectId: 4, mapFileName: "0", ...changes });
const source = (changes = {}) => ({ owner: owner(), playerName: "Leader", members: [], ...changes });
function entered(operations, op, name, current) {
  const proof = operations.reserve(op, name, current);
  assert.ok(proof);
  assert.equal(operations.claim(proof, current, groupOperationCommand(proof)), true);
  return proof;
}
const roster = (...names) => ({ members: names.map(name => ({ name, level: 30, class: 0, hp: 80, maxHp: 90, online: true })),
  leaderName: names[0] ?? "" });

test("actual adapter type checks without requiring the application bundle", () => {
  const program = ts.createProgram([sourcePath], { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler, strict: true, noEmit: true, skipLibCheck: true, types: [] });
  assert.deepEqual(ts.getPreEmitDiagnostics(program).map(row => ts.flattenDiagnosticMessageText(row.messageText, "\n")), []);
});

test("commands contain only the requested operation/name and reserve does not send or mutate authority", () => {
  const operations = new GroupOperations(), current = source(), before = structuredClone({ ...current, owner: null });
  const proof = operations.reserve("addMember", "  Jane  ", current);
  assert.ok(proof);
  assert.deepEqual(groupOperationCommand(proof), { type: "addMember", name: "Jane" });
  assert.deepEqual({ ...current, owner: null }, before);
  assert.deepEqual(operations.pending, [{ proof, entered: false }]);
  assert.equal(Object.isFrozen(proof), true);
  assert.equal(Object.isFrozen(proof.owner), true);
  assert.equal(Object.isFrozen(operations.pending), true);
  assert.equal(Object.isFrozen(operations.pending[0]), true);
  assert.equal(operations.allows(proof, current, groupOperationCommand(proof)), true);
  assert.equal(operations.pending[0].entered, false);
});

test("valid names retain case/Unicode and use only the existing trim, length and NUL boundary", () => {
  const operations = new GroupOperations(), current = source();
  for (const name of [null, 17, {}, "", " \t\n ", "bad\0name", "x".repeat(257)])
    assert.equal(operations.reserve("addMember", name, current), null);
  assert.equal(operations.reserve("switchGroup", "Jane", current), null);
  assert.ok(operations.reserve("addMember", "龍城", current));
  assert.ok(operations.reserve("addMember", "Jane", current));
  assert.ok(operations.reserve("addMember", "jane", current), "no invented case folding");
  assert.ok(operations.reserve("addMember", "x".repeat(256), current));
});

test("exact op/name duplicates are blocked but other names and add versus delete can coexist up to eight", () => {
  const operations = new GroupOperations(), current = source();
  const proof = operations.reserve("addMember", "Jane", current);
  assert.equal(operations.reserve("addMember", " Jane ", current), null);
  entered(operations, "delMember", "Jane", current);
  for (let index = 0; index < 6; index++) assert.ok(operations.reserve("addMember", "Other" + index, current));
  assert.equal(operations.pending.length, 8);
  assert.equal(operations.reserve("delMember", "Other", current), null);
  assert.equal(operations.cancelDefinitelyUnsent(proof), true);
  assert.ok(operations.reserve("delMember", "Other", current));
});

test("invite capacity and Native first-member leadership are checked at reservation and again at claim", () => {
  const operations = new GroupOperations(), current = source({ members: ["Leader", "Old"] });
  const proof = operations.reserve("addMember", "Jane", current);
  assert.ok(proof);
  const full = { ...current, members: ["Leader", ...Array.from({ length: 14 }, (_, i) => "Member" + i)] };
  const follower = { ...current, members: ["OtherLeader", "Leader"] };
  assert.equal(operations.reserve("addMember", "Someone", full), null);
  assert.equal(operations.reserve("addMember", "Someone", follower), null);
  assert.equal(operations.reserve("delMember", "Someone", follower), null);
  assert.equal(operations.claim(proof, full, groupOperationCommand(proof)), false);
  assert.equal(operations.claim(proof, follower, groupOperationCommand(proof)), false);
  assert.equal(operations.claim(proof, { ...current, members: ["Leader", "Changed"] }, groupOperationCommand(proof)), true);
  assert.ok(operations.reserve("delMember", "NotLocallyListed", full), "server decides whether a removal target exists");
});

test("invalid or missing source facts cannot authorize either operation", () => {
  const operations = new GroupOperations(), current = source();
  for (const bad of [null, {}, { ...current, owner: null }, { ...current, playerName: null },
    { ...current, playerName: "" }, { ...current, members: null }, { ...current, members: ["Leader", null] },
    { ...current, members: [, "Leader"] }, ...["connectionGeneration", "sessionGeneration", "sceneRevision", "playerObjectId"]
      .flatMap(key => [0, -1, NaN, Infinity, 1.5, Number.MAX_SAFE_INTEGER + 1].map(value => ({ ...current, owner: { ...current.owner, [key]: value } }))),
    { ...current, owner: { ...current.owner, socket: null } }, { ...current, owner: { ...current.owner, mapFileName: "" } }]) {
    assert.equal(operations.reserve("addMember", "Jane", bad), null);
    assert.equal(operations.reserve("delMember", "Jane", bad), null);
  }
  assert.equal(operations.pending.length, 0);
});

test("claim requires the precise socket, connection, session, object, scene, map and player identity", () => {
  const operations = new GroupOperations(), current = source(), proof = operations.reserve("addMember", "Jane", current);
  for (const changes of [{ socket: {} }, { connectionGeneration: 2 }, { sessionGeneration: 3 },
    { playerObjectId: 5 }, { sceneRevision: 4 }, { mapFileName: "D001" }]) {
    assert.equal(operations.claim(proof, { ...current, owner: { ...current.owner, ...changes } }, groupOperationCommand(proof)), false);
  }
  assert.equal(operations.claim(proof, { ...current, playerName: "Different" }, groupOperationCommand(proof)), false);
  assert.equal(operations.claim(proof, { ...current, owner: { ...current.owner } }, groupOperationCommand(proof)), true);
  assert.equal(operations.claim(proof, current, groupOperationCommand(proof)), false);
});

test("captured owner is immutable and copied rather than borrowing later mutations", () => {
  const operations = new GroupOperations(), current = source(), captured = { ...current.owner };
  const proof = operations.reserve("addMember", "Jane", current);
  current.owner.sessionGeneration++;
  assert.equal(proof.owner.sessionGeneration, captured.sessionGeneration);
  assert.equal(operations.claim(proof, current, groupOperationCommand(proof)), false);
  assert.equal(operations.claim(proof, { ...current, owner: captured }, groupOperationCommand(proof)), true);
});

test("fabricated or foreign-instance proofs do not grant authority even when every visible field matches", () => {
  const operations = new GroupOperations(), other = new GroupOperations(), current = source();
  const proof = operations.reserve("delMember", "Jane", current);
  for (const fake of [{ ...proof }, other.reserve("delMember", "Jane", current), null, undefined]) {
    assert.equal(operations.claim(fake, current, { type: "delMember", name: "Jane" }), false);
    assert.equal(operations.cancelDefinitelyUnsent(fake), false);
  }
  assert.equal(operations.claim(proof, current, groupOperationCommand(proof)), true);
});

test("only the exact two-field data DTO can be claimed; accessors, hidden extras and serialization substitutions are rejected", () => {
  const operations = new GroupOperations(), current = source(), proof = operations.reserve("addMember", "Jane", current);
  let getterCalls = 0;
  const hiddenExtra = { type: "addMember", name: "Jane" };
  Object.defineProperty(hiddenExtra, "allowGroup", { value: true });
  const symbolExtra = { type: "addMember", name: "Jane", [Symbol("extra")]: true };
  const accessor = { type: "addMember", get name() { getterCalls++; return "Jane"; } };
  const inherited = Object.create({ type: "addMember" }); inherited.name = "Jane";
  for (const wire of [null, [], {}, { type: "delMember", name: "Jane" }, { type: "addMember", name: "jane" },
    { type: "addMember", name: " Jane " }, { type: "addMember", name: "Jane", allowGroup: true }, hiddenExtra, symbolExtra,
    accessor, inherited, { type: "addMember", name: "Jane", toJSON() { return { type: "logOut" }; } },
    new Proxy({}, { ownKeys() { throw new Error("untrusted DTO"); } })])
    assert.equal(operations.claim(proof, current, wire), false);
  assert.equal(getterCalls, 0);
  const exact = Object.assign(Object.create(null), { type: "addMember", name: "Jane" });
  assert.equal(operations.claim(proof, current, exact), true);
});

test("synchronous reentry during DTO inspection permits at most one claim", () => {
  const operations = new GroupOperations(), current = source(), proof = operations.reserve("addMember", "Jane", current);
  const command = groupOperationCommand(proof), verdicts = [];
  const malicious = new Proxy(command, { ownKeys(target) {
    verdicts.push(operations.claim(proof, current, command));
    return Reflect.ownKeys(target);
  } });
  verdicts.push(operations.claim(proof, current, malicious));
  assert.deepEqual(verdicts, [true, false]);
  assert.equal(operations.pending[0].entered, true);
  assert.equal(operations.claim(proof, current, command), false);
});

test("retirement during final DTO inspection cannot revive a canceled reservation", () => {
  const operations = new GroupOperations(), current = source(), proof = operations.reserve("addMember", "Jane", current);
  const malicious = new Proxy(groupOperationCommand(proof), { ownKeys(target) {
    operations.retire(); return Reflect.ownKeys(target);
  } });
  assert.equal(operations.claim(proof, current, malicious), false);
  assert.equal(operations.pending.length, 0);
});

test("source/leadership changes during DTO inspection are checked before the one send is allowed", () => {
  for (const change of [current => { current.owner = { ...current.owner, sessionGeneration: 3 }; },
    current => { current.members = ["DifferentLeader", "Leader"]; },
    current => { current.members = ["Leader", ...Array.from({ length: 14 }, (_, i) => "Member" + i)]; }]) {
    const operations = new GroupOperations(), current = source(), proof = operations.reserve("addMember", "Jane", current);
    const wire = new Proxy(groupOperationCommand(proof), { ownKeys(target) { change(current); return Reflect.ownKeys(target); } });
    assert.equal(operations.claim(proof, current, wire), false);
    assert.equal(operations.pending[0].entered, false);
  }
});

test("definitely-unsent cancellation releases only reservations; a throw after entered preserves the operation", () => {
  const operations = new GroupOperations(), current = source();
  const unsent = operations.reserve("addMember", "Jane", current);
  assert.equal(operations.outcomeUnknown(unsent), false);
  assert.equal(operations.cancelDefinitelyUnsent(unsent), true);
  assert.equal(operations.claim(unsent, current, groupOperationCommand(unsent)), false);
  const proof = operations.reserve("addMember", "Jane", current);
  assert.throws(() => {
    assert.equal(operations.claim(proof, current, groupOperationCommand(proof)), true);
    throw new Error("socket.send entered, outcome unknown");
  }, /outcome unknown/);
  assert.equal(operations.cancelDefinitelyUnsent(proof), false);
  assert.equal(operations.outcomeUnknown(proof), true);
  assert.equal(operations.reserve("addMember", "Jane", current), null);
  assert.equal(operations.receipt("AddMember", { name: "Jane" }, current.owner), true);
  assert.equal(operations.outcomeUnknown(proof), false);
});

test("AddMember/DeleteMember match only the entered operation and target, not another pending operation", () => {
  const operations = new GroupOperations(), current = source();
  const add = entered(operations, "addMember", "Jane", current);
  const remove = entered(operations, "delMember", "Jane", current);
  const other = entered(operations, "addMember", "Alice", current);
  const reserved = operations.reserve("addMember", "Reserved", current);
  assert.equal(operations.receipt("AddMember", { name: "Janel" }, current.owner), false);
  assert.equal(operations.receipt("AddMember", { name: "Reserved" }, current.owner), false);
  assert.equal(operations.receipt("AddMember", { name: "Jane" }, current.owner), true);
  assert.deepEqual(operations.pending.map(row => row.proof), [remove, other, reserved]);
  assert.equal(operations.receipt("AddMember", { name: "Jane" }, current.owner), false);
  assert.equal(operations.receipt("DeleteMember", { name: "jane" }, current.owner), false);
  assert.equal(operations.receipt("DeleteMember", { name: "Jane" }, current.owner), true);
  assert.equal(operations.pending.some(row => row.proof === add), false);
  assert.ok(operations.reserve("addMember", "Jane", current), "matching ACK permits a fresh ordinary operation");
});

test("a completed invite send permits a new gesture without releasing unrelated custody", () => {
  const operations = new GroupOperations(), current = source();
  const invite = entered(operations, "addMember", "bLaDe", current);
  const remove = entered(operations, "delMember", "Blade", current);
  const unentered = operations.reserve("addMember", "Other", current);
  assert.equal(operations.reserve("addMember", "bLaDe", current), null);
  operations.finishInviteAttempt(remove);
  operations.finishInviteAttempt(unentered);
  operations.finishInviteAttempt(invite);
  assert.deepEqual(operations.pending.map(row => row.proof), [remove, unentered]);
  assert.equal(operations.claim(invite, current, groupOperationCommand(invite)), false);
  const fresh = entered(operations, "addMember", "bLaDe", current);
  assert.notEqual(fresh, invite);
  operations.finishInviteAttempt(invite);
  assert.equal(operations.outcomeUnknown(fresh), true);
});

test("a receipt cannot acknowledge operations that enter during receipt decoding", () => {
  const operations = new GroupOperations(), current = source();
  let fresh;
  const payload = new Proxy({ name: "Jane" }, { getOwnPropertyDescriptor(target, key) {
    if (key === "name") fresh = entered(operations, "addMember", "Jane", current);
    return Reflect.getOwnPropertyDescriptor(target, key);
  } });
  assert.equal(operations.receipt("AddMember", payload, current.owner), false);
  assert.deepEqual(operations.pending, [{ proof: fresh, entered: true }]);
  assert.equal(operations.receipt("AddMember", { name: "Jane" }, current.owner), true);
});

test("full roster membership releases add/removal independently, while unchanged authority has no effect", () => {
  const operations = new GroupOperations(), current = source({ members: ["Leader", "Jane"] });
  const add = entered(operations, "addMember", "Alice", current);
  const remove = entered(operations, "delMember", "Jane", current);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader", "Jane"), current.owner), false);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader", "Jane", "Alice"), current.owner), true);
  assert.deepEqual(operations.pending.map(row => row.proof), [remove]);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader", "Alice"), current.owner), true);
  assert.equal(operations.pending.length, 0);
  assert.equal(operations.outcomeUnknown(add), false);
  entered(operations, "delMember", "Absent", current);
  assert.equal(operations.receipt("GroupMemberInfo", roster(), current.owner), true);
});

test("group additions reconcile canonical ASCII case only on genuine same-owner success", () => {
  const operations = new GroupOperations(), current = source();
  const add = entered(operations, "addMember", "bLaDe", current);
  const unicode = entered(operations, "addMember", "Älo", current);
  assert.deepEqual(groupOperationCommand(add), { type: "addMember", name: "bLaDe" });
  const foreignOwner = { ...current.owner, sessionGeneration: 3 };
  assert.equal(operations.receipt("AddMember", { name: "Blade" }, foreignOwner), false);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader", "Blade"), foreignOwner), false);
  assert.equal(operations.receipt("AddMember", { name: "Blades" }, current.owner), false);
  assert.equal(operations.receipt("DeleteMember", { name: "Blade" }, current.owner), false);
  assert.equal(operations.receipt("GroupInviteResult", { name: "Blade", success: false }, current.owner), false);
  assert.equal(operations.receipt("AddMember", { name: "älo" }, current.owner), false);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader", "älo"), current.owner), false);
  assert.deepEqual(operations.pending.map(row => row.proof), [add, unicode]);
  assert.equal(operations.receipt("AddMember", { name: "Blade" }, current.owner), true);
  assert.deepEqual(operations.pending.map(row => row.proof), [unicode]);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader", "ÄLO"), current.owner), true);
  assert.equal(operations.pending.length, 0);
});

test("malformed full rosters never masquerade as an empty roster and clear a pending removal", () => {
  const operations = new GroupOperations(), current = source();
  entered(operations, "delMember", "Jane", current);
  let getters = 0;
  const getterPayload = { get members() { getters++; return []; } };
  for (const payload of [null, {}, { members: null }, { members: {} }, { members: ["Leader"] },
    { members: [{ name: "Leader" }, {}] }, { members: [null] }, { members: [{ name: "" }] },
    { members: [{ name: "bad\0name" }] }, { members: [, { name: "Leader" }] }, getterPayload,
    { members: new Proxy([], { getOwnPropertyDescriptor() { throw new Error("untrusted roster"); } }) }])
    assert.equal(operations.receipt("GroupMemberInfo", payload, current.owner), false);
  assert.equal(getters, 0);
  assert.equal(operations.pending.length, 1);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader"), current.owner), true);
});

test("only same physical-owner receipts can release; a map/scene change keeps pending but prevents old reservations sending", () => {
  const operations = new GroupOperations(), current = source();
  const proof = entered(operations, "addMember", "Jane", current);
  const reserved = operations.reserve("addMember", "Alice", current);
  for (const changes of [{ socket: {} }, { connectionGeneration: 2 }, { sessionGeneration: 3 }, { playerObjectId: 5 }]) {
    const foreign = { ...current.owner, ...changes };
    assert.equal(operations.receipt("AddMember", { name: "Jane" }, foreign), false);
    assert.equal(operations.receipt("GroupMemberInfo", roster("Jane"), foreign), false);
  }
  const nextScene = { ...current, owner: { ...current.owner, mapFileName: "D001", sceneRevision: 4 } };
  assert.equal(operations.reserve("addMember", "Jane", nextScene), null);
  assert.equal(operations.claim(reserved, nextScene, groupOperationCommand(reserved)), false);
  assert.equal(operations.pending.length, 2);
  assert.equal(operations.receipt("AddMember", { name: "Jane" }, nextScene.owner), true);
  assert.equal(operations.outcomeUnknown(proof), false);
  assert.equal(operations.cancelDefinitelyUnsent(reserved), true);
  assert.ok(operations.reserve("addMember", "Alice", nextScene));
});

test("SwitchGroup, decline/error chat, DeleteGroup and unrelated snapshots never unlock add pending or schedule retries", () => {
  const operations = new GroupOperations(), current = source();
  const proof = entered(operations, "addMember", "Jane", current);
  for (const [packet, payload] of [["SwitchGroup", { allowGroup: true }], ["GroupInviteResult", { name: "Jane", success: false }],
    ["Chat", { message: "Jane declined your invitation." }], ["DeleteGroup", {}], ["snapshot", roster("Jane")],
    ["AddMember", {}], ["AddMember", { name: " Jane " }]])
    assert.equal(operations.receipt(packet, payload, current.owner), false);
  assert.equal(operations.receipt("GroupMemberInfo", roster("Leader"), current.owner), false);
  const originalNow = Date.now;
  try { Date.now = () => Number.MAX_SAFE_INTEGER; assert.equal(operations.outcomeUnknown(proof), true); }
  finally { Date.now = originalNow; }
  assert.equal(operations.reserve("addMember", "Jane", current), null);
  assert.deepEqual(operations.pending, [{ proof, entered: true }]);
});

test("session retirement clears all work and old proofs cannot send or release a new-session request", () => {
  const operations = new GroupOperations(), current = source();
  const old = entered(operations, "addMember", "Jane", current);
  const unsent = operations.reserve("delMember", "Alice", current);
  operations.retire();
  assert.equal(operations.pending.length, 0);
  assert.equal(operations.claim(old, current, groupOperationCommand(old)), false);
  assert.equal(operations.claim(unsent, current, groupOperationCommand(unsent)), false);
  const next = { ...current, owner: { ...current.owner, sessionGeneration: 3 } };
  const fresh = entered(operations, "addMember", "Jane", next);
  assert.equal(operations.receipt("AddMember", { name: "Jane" }, current.owner), false);
  assert.equal(operations.outcomeUnknown(fresh), true);
  assert.equal(operations.receipt("AddMember", { name: "Jane" }, next.owner), true);
});
