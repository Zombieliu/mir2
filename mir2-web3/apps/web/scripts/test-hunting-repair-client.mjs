import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import test from "node:test";
// An isolated checkout may reuse an explicitly named installed compiler without
// installing dependencies or modifying its node_modules tree.
const ts = createRequire(import.meta.url)(process.env.MIR2_TEST_TYPESCRIPT_PATH ?? "typescript");

const source = new URL("../lib/hunting-repair-client.ts", import.meta.url);
const compilerOptions = { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS,
  moduleResolution: ts.ModuleResolutionKind.Node10, strict: true, skipLibCheck: true, types: [],
  lib: ["lib.es2022.d.ts", "lib.dom.d.ts"] };
const program = ts.createProgram([fileURLToPath(source)], { ...compilerOptions, noEmit: true });
const diagnostics = ts.getPreEmitDiagnostics(program).filter(value => value.category === ts.DiagnosticCategory.Error);
assert.equal(diagnostics.length, 0, diagnostics.map(value => ts.flattenDiagnosticMessageText(value.messageText, "\n")).join("\n"));
const modules = new Map();
function load(name) {
  assert(["hunting-repair-client", "npc-purchase-client"].includes(name), "Unexpected pure module");
  if (modules.has(name)) return modules.get(name);
  const module = { exports: {} };
  modules.set(name, module.exports);
  const compiled = ts.transpileModule(readFileSync(new URL(`../lib/${name}.ts`, import.meta.url), "utf8"),
    { compilerOptions, reportDiagnostics: true });
  assert.equal((compiled.diagnostics ?? []).filter(value => value.category === ts.DiagnosticCategory.Error).length, 0);
  new Function("require", "module", "exports", compiled.outputText)(id => {
    assert.equal(name, "hunting-repair-client"); assert.equal(id, "./npc-purchase-client");
    return load("npc-purchase-client");
  }, module, module.exports);
  return module.exports;
}
const api = load("hunting-repair-client");
const uid = "9007199254740993", quoteId = "a".repeat(64), otherId = "b".repeat(64);
const selection = { uniqueId: uid, special: false };
const quote = (patch = {}) => ({ quoteId, ...selection, costBoundGold: "123", createdAtMs: 1000, expiresAtMs: 31000, ...patch });
const receipt = (patch = {}) => ({ quoteId, costBoundGold: "123", free: false, operationKey: "repair-operation",
  requestDigest: "repair-digest", businessDay: 20000, committedAtMs: 2000, ...patch });
function snapshot() {
  const stage5Systems = { group: {}, guild: {}, social: {},
    relationship: { allowLoverRecall: false, cooldownUntilMs: 0, allowMarriage: false, partnerName: "", marriedDateBinaryDatetime: 0,
      mapName: "", marriedDays: 0, pendingRequestFrom: null, pendingDivorceFrom: null },
    mentor: { isMentor: false, cooldownUntilMs: 0, allowMentor: false, name: "", level: 0, online: false,
      menteeExp: 0, pendingRequestFrom: null, pendingRequestLevel: 0 }, mail: [], gameShopIndividualPurchases: {},
    economyProjectionEventIds: [], trade: null, auction: [], refine: {}, conquest: {}, guildTerritory: {}, hero: null,
    heroLearnedMagics: [], profession: {}, appearance: {}, nameLists: [], intelligentCreatures: [], summonedIntelligentCreatureType: null,
    intelligentCreaturePearls: 0, itemRental: {}, attackMode: 0, petMode: 0, pkDecayElapsedTicks: 0 };
  return { tick: 0, mapTitle: "Bichon", mapFileName: "0", inSafeZone: true, lightSetting: 0, playerObjectId: 7,
    playerHp: 100, playerMaxHp: 100, playerMp: 40, playerMaxMp: 40, playerCrystalStats: [], playerPkPoints: 0,
    playerExperience: 0, playerMaxExperience: 100, gold: 100, credit: 0, cityCurrencies: {}, currentWeight: 0, playerWeights: null,
    maxWeight: 100, freeBagSlots: 40, maxBagSlots: 40, inventoryCapacity: 46, npcGoldTradeCapacity: null, storageSize: 80,
    hasExpandedStorage: false, hasStoragePassword: false, requireStoragePassword: false, storagePasswordLastSetBinaryDatetime: 0,
    expandedStorageExpiryTimeBinaryDatetime: 0,
    entities: [{ objectId: 7, name: "Actor", class: "Warrior", gender: "Male", level: 1, x: 10, y: 10, direction: "Down", dead: false }],
    beltItems: [], inventoryItems: [], storageItems: [], equipmentItems: [], heroInventoryItems: [], heroEquipmentItems: [],
    heroInventoryCapacity: 0, heroStats: [], heroVitals: null, heroWeights: { bag: 0, wear: 0, hand: 0 }, questLog: [],
    knownSkills: [], activeBuffs: [], activeNpcDialog: null, stage5Systems, terrainPatches: [], decorObjects: [], groundDrops: [],
    mapTransfers: [], interactionHints: [], sceneView: null };
}
const frame = (requestId, reply, current = null) => JSON.stringify({ type: "huntingRepairOwner", protocolVersion: 1,
  requestId, reply, snapshot: current });
function fixture(mode = "hunting-v1", sharedOwner) {
  const owner = sharedOwner ?? {}, actorScope = Object.freeze({ account: "actor", character: 0, incarnation: "server-owned" });
  const state = { current: true, mode, selected: { ...selection }, selectionProof: true, now: 1000,
    throwSend: false, apply: true, throwApply: false, onChange: null, duringApply: null, duringCurrent: null };
  const sent = [], applied = [], changes = [];
  const client = new api.HuntingRepairClient({ owner, actorScope, mode, timeoutMs: 100,
    isCurrent: (captured, scope, capturedMode) => {
      state.duringCurrent?.();
      return captured === owner && scope === actorScope && state.current && capturedMode === state.mode;
    },
    selectionMatches: (captured, selected) => captured === owner && state.selectionProof
      && selected.uniqueId === state.selected.uniqueId && selected.special === state.selected.special,
    now: () => state.now,
    send: (captured, raw) => { assert.equal(captured, owner); sent.push({ raw, request: JSON.parse(raw) }); if (state.throwSend) throw Error("send uncertain"); },
    applySnapshot: value => { applied.push(value); state.duringApply?.(); if (state.throwApply) throw Error("projection failed"); return state.apply; },
    onChange: value => { changes.push(value); state.onChange?.(value); } });
  const last = () => sent.at(-1).request;
  const receive = (reply, current = null, id = last().requestId) => client.receive(frame(id, reply, current));
  const quoted = (patch = {}) => { assert(client.quote(state.selected)); assert(receive({ kind: "quoted", quote: quote(patch) })); };
  return { client, owner, actorScope, state, sent, applied, changes, last, receive, quoted };
}

test("strict parser accepts only the exact repair envelope and required nullable snapshot", () => {
  const raw = frame("1", { kind: "quoted", quote: quote() });
  const value = api.parseHuntingRepairFrame(raw);
  assert(Object.isFrozen(value)); assert(Object.isFrozen(value.reply.quote));
  for (const mutate of [v => delete v.snapshot, v => v.extra = true, v => v.protocolVersion = 2,
    v => v.requestId = "0", v => v.requestId = "01", v => v.requestId = 1,
    v => v.reply.extra = true, v => v.reply.quote.uniqueId = 0, v => v.reply.quote.special = "false",
    v => v.reply.quote.costBoundGold = 123, v => v.reply.quote.quoteId = "0".repeat(64),
    v => v.reply.quote.quoteId = "A".repeat(64), v => v.reply.quote.costBoundGold = "18446744073709551616",
    v => v.reply.quote.expiresAtMs++, v => v.reply.quote.createdAtMs = -1,
    v => v.reply.quote.expiresAtMs = Number.MAX_SAFE_INTEGER + 1]) {
    const bad = JSON.parse(raw); mutate(bad); assert.throws(() => api.parseHuntingRepairFrame(JSON.stringify(bad)));
  }
});
test("duplicate and escaped duplicate fields cannot be normalized into a valid frame", () => {
  const raw = frame("1", { kind: "quoted", quote: quote() });
  for (const bad of [raw.replace('"type":', '"type":"elsewhere","type":'),
    raw.replace('"requestId":"1"', '"requestId":"1","request\\u0049d":"2"'),
    raw.replace('"kind":"quoted"', '"kind":"quoted","kind":"quoted"'),
    raw.replace('"snapshot":null', '"snapshot":null,"__proto__":{}')]) {
    assert.throws(() => api.parseHuntingRepairFrame(bad));
  }
});
test("rejection codes are exact strings without array/object coercion", () => {
  for (const code of [["expired"], ["notSubmitted"], { code: "expired" }, null, 0, true])
    assert.throws(() => api.parseHuntingRepairFrame(frame("1", { kind: "rejected", quoteId: null, code })));
  assert(api.parseHuntingRepairFrame(frame("1", { kind: "rejected", quoteId: null, code: "expired" })));
});
test("owner probe recognizes top-level escaped markers and never nested or quoted substrings", () => {
  assert(api.isHuntingRepairOwnerFrame('{"ty\\u0070e":"huntingRepairOwner"}'));
  assert(api.isHuntingRepairOwnerFrame('{"type":"elsewhere","type":"huntingRepairOwner"}'));
  assert(api.isHuntingRepairOwnerFrame('{"deep":' + '['.repeat(70) + '0' + ']'.repeat(70) + ',"type":"huntingRepairOwner"}'));
  for (const raw of ['{"payload":{"type":"huntingRepairOwner"}}', '[{"type":"huntingRepairOwner"}]',
    '{"message":"type: huntingRepairOwner"}', '"huntingRepairOwner"', '{"type":"npcPurchaseOwner"}'])
    assert.equal(api.isHuntingRepairOwnerFrame(raw), false, raw);
});
test("quoted timestamps and immutable receipt money are independently validated", () => {
  for (const patch of [{ free: true }, { costBoundGold: "0" }, { operationKey: "" }, { requestDigest: "bad\u0000value" },
    { operationKey: "汉".repeat(400) }, { requestDigest: "\u0080" }, { committedAtMs: -1 }, { businessDay: 1.1 }])
    assert.throws(() => api.parseHuntingRepairFrame(frame("1", { kind: "committed", receipt: receipt(patch), replayed: true, reloadRequired: false })));
  assert(api.parseHuntingRepairFrame(frame("18446744073709551615", { kind: "committed",
    receipt: receipt({ costBoundGold: "0", free: true }), replayed: false, reloadRequired: false })));
  assert.throws(() => api.parseHuntingRepairFrame(frame("1", { kind: "committed",
    receipt: receipt({ operationKey: "\ud800" }), replayed: false, reloadRequired: false })));
});
test("full snapshot is allowed only on synchronized committed results", () => {
  const committed = { kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false };
  assert(api.parseHuntingRepairFrame(frame("1", committed, snapshot())).snapshot);
  for (const reply of [{ kind: "quoted", quote: quote() }, { kind: "pending", quoteId },
    { ...committed, reloadRequired: true }]) assert.throws(() => api.parseHuntingRepairFrame(frame("1", reply, snapshot())));
  for (const key of ["inventoryItems", "gold", "stage5Systems", "entities"]) {
    const partial = snapshot(); delete partial[key]; assert.throws(() => api.parseHuntingRepairFrame(frame("1", committed, partial)));
  }
});
test("full snapshot u64 remains exact without Number conversion and unknown unsafe leaves reject", () => {
  const current = snapshot(); current.inventoryItems.push({ key: "ore", name: "Ore", description: "Exact", quantity: 1,
    icon: 1, slot: 0, container: "bag1", uniqueId: uid });
  const raw = frame("1", { kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, current)
    .replace('"uniqueId":"' + uid + '"', '"uniqueId":' + uid);
  assert.equal(api.parseHuntingRepairFrame(raw).snapshot.inventoryItems[0].uniqueId, uid);
  assert.throws(() => api.parseHuntingRepairFrame(raw.replace('"gold":100', '"gold":9007199254740993')));
});
test("the frame limit counts UTF-8 bytes and blocks frames above 2 MiB", () => {
  const current = snapshot(); current.sceneView = "汉".repeat(700000);
  const raw = frame("1", { kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, current);
  assert(raw.length < 2 * 1024 * 1024); assert.throws(() => api.parseHuntingRepairFrame(raw));
});
test("legacy or unannounced/current-mode mismatch never sends or falls back", () => {
  const old = fixture("legacy"); assert.equal(old.client.quote(selection), false); assert.equal(old.sent.length, 0);
  const f = fixture(); f.state.mode = "legacy"; assert.equal(f.client.quote(selection), false); assert.equal(f.sent.length, 0);
});
test("captured physical owner and frozen actor scope cannot be rebound through options", () => {
  assert.throws(() => new api.HuntingRepairClient({ owner: {}, actorScope: {}, mode: "hunting-v1" }));
  const f = fixture(); f.quoted(); assert(Object.isFrozen(f.client.view())); assert(Object.isFrozen(f.client.view().quote));
  f.state.current = false; assert.equal(f.client.confirm(), false); assert.equal(f.sent.length, 1);
});
test("quote keeps zero and u64-max ids as canonical strings", () => {
  for (const value of ["0", "18446744073709551615", uid]) {
    const f = fixture(); f.state.selected.uniqueId = value; f.quoted({ uniqueId: value });
    assert.equal(f.last().action.uniqueId, value); assert.equal(typeof f.last().action.uniqueId, "string");
  }
  for (const value of [0, "01", "-1", "18446744073709551616"]) {
    const f = fixture(); assert.equal(f.client.quote({ uniqueId: value, special: false }), false); assert.equal(f.sent.length, 0);
  }
});
test("quote reply must match request id uid and normal/special mode exactly", () => {
  const f = fixture(); assert(f.client.quote(selection));
  assert.equal(f.receive({ kind: "quoted", quote: quote() }, null, "999"), false);
  assert.equal(f.receive({ kind: "quoted", quote: quote({ uniqueId: "1" }) }), false);
  assert.equal(f.receive({ kind: "quoted", quote: quote({ special: true }) }), false);
  assert.equal(f.client.confirm(), false);
  assert(f.receive({ kind: "quoted", quote: quote() })); assert(f.client.confirm());
});
test("fresh Quote and Confirm require the same current physical item selection", () => {
  const f = fixture(); f.state.selectionProof = false; assert.equal(f.client.quote(selection), false);
  f.state.selectionProof = true; f.quoted(); f.state.selected.special = true;
  assert.equal(f.client.confirm(), false); assert.equal(f.sent.length, 1);
});
test("a changed item selection before quote reply cannot authorize a charge", () => {
  const f = fixture(); assert(f.client.quote(selection)); f.state.selected.uniqueId = "2";
  assert.equal(f.receive({ kind: "quoted", quote: quote() }), false); assert.equal(f.client.confirm(), false);
});
test("expired Quote and double-click Confirm do not create another request", () => {
  const expired = fixture(); expired.quoted(); expired.state.now = 31000;
  assert.equal(expired.client.confirm(), false); assert.equal(expired.sent.length, 1);
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  assert.equal(f.client.confirm(), false); assert.equal(f.client.quote(selection), false); assert.equal(f.sent.length, 2);
});
test("send exception after Confirm retains the original id and permits only Query", () => {
  const f = fixture(); f.quoted(); f.state.throwSend = true;
  assert.equal(f.client.confirm(), false); assert.equal(f.client.view().phase, "pending"); assert.equal(f.client.view().quoteId, quoteId);
  assert.equal(f.client.confirm(), false); assert.equal(f.client.quote(selection), false);
  f.state.throwSend = false; assert(f.client.query()); assert.deepEqual(f.last().action, { kind: "query", quoteId });
});
test("Confirm timeout retains original operation and ignores the superseded late response", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm()); const confirmId = f.last().requestId;
  f.state.now += 100; f.client.checkTimeout(); assert.equal(f.client.view().phase, "pending");
  assert.equal(f.client.confirm(), false); assert(f.client.query());
  assert.equal(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot(), confirmId), false);
  assert.equal(f.applied.length, 0);
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false }, snapshot()));
  assert.equal(f.client.view().phase, "committed"); assert.equal(f.applied.length, 1);
});
test("lost unconfirmed Quote permits a new Quote but not an implicit Confirm", () => {
  const f = fixture(); assert(f.client.quote(selection)); const old = f.last().requestId;
  f.state.now += 100; f.client.checkTimeout(); assert(f.client.quote(selection));
  assert.notEqual(f.last().requestId, old); assert.equal(f.client.confirm(), false);
  assert.equal(f.receive({ kind: "quoted", quote: quote() }, null, old), false);
});
test("close/cancel invalidates an unconfirmed Quote and pending Quote response", () => {
  const f = fixture(); assert(f.client.quote(selection)); const old = f.last().requestId; f.client.resetView();
  assert.equal(f.receive({ kind: "quoted", quote: quote() }, null, old), false); assert.equal(f.client.confirm(), false);
  f.quoted(); f.client.resetView(); assert.equal(f.client.confirm(), false);
});
test("closing the UI preserves an entered original and its current response", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm()); f.client.resetView();
  assert.equal(f.client.view().visible, false); assert.equal(f.client.view().phase, "pending");
  assert(f.receive({ kind: "pending", quoteId })); assert(f.client.query());
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false }, snapshot()));
  assert.equal(f.client.view().phase, "committed"); assert.equal(f.client.view().visible, false);
});
test("known receipt without snapshot never releases pending or replays old wallet values", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }));
  assert.equal(f.client.view().phase, "pending"); assert.equal(f.applied.length, 0);
  assert.equal(f.client.quote(selection), false); assert.equal(f.client.confirm(), false); assert(f.client.query());
  const current = snapshot(); current.gold = 99;
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false }, current));
  assert.equal(f.applied[0].gold, 99); assert.equal(f.client.view().phase, "committed");
});
test("failed or throwing snapshot application stays pending until a later successful Query", () => {
  for (const fail of ["apply", "throwApply"]) {
    const f = fixture(); f.quoted(); assert(f.client.confirm()); f.state[fail] = fail === "apply" ? false : true;
    assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot()));
    assert.equal(f.client.view().phase, "pending"); assert.equal(f.client.quote(selection), false);
    f.state.apply = true; f.state.throwApply = false; assert(f.client.query());
    assert(f.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false }, snapshot()));
    assert.equal(f.client.view().phase, "committed");
  }
});
test("snapshot callback that loses physical ownership cannot release the pending source", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm()); f.state.duringApply = () => { f.state.current = false; };
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot()));
  assert.equal(f.client.view().phase, "pending"); assert.equal(f.client.query(), false);
});
test("reloadRequired explicitly retires and preserves the immutable receipt without applying", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: true }));
  assert.equal(f.client.view().phase, "retired"); assert.equal(f.client.view().reloadRequired, true);
  assert.equal(f.client.view().receipt.operationKey, "repair-operation"); assert.equal(f.applied.length, 0);
  assert.equal(f.client.query(), false); assert.equal(f.client.quote(selection), false);
});
test("retired old connection cannot apply a response on a new socket or actor", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm()); const oldId = f.last().requestId; f.client.retireConnection();
  assert.equal(f.client.view().quoteId, quoteId); const next = fixture();
  assert.equal(f.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false }, snapshot(), oldId), false);
  assert.equal(f.applied.length, 0); assert.equal(next.applied.length, 0);
  assert(next.client.query(quoteId)); assert.equal(next.client.confirm(), false);
  assert(next.receive({ kind: "committed", receipt: receipt(), replayed: true, reloadRequired: false }, snapshot()));
  assert.equal(next.client.view().phase, "committed");
});
test("one live adapter per physical owner prevents UI remount from forgetting Pending", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm()); assert.throws(() => fixture("hunting-v1", f.owner));
  const old = f.last().requestId; f.client.retireConnection(); const next = fixture("hunting-v1", f.owner);
  assert(next.client.query(quoteId)); assert.notEqual(next.last().requestId, old);
});
test("correlation rejects another quote result and changed immutable receipt payload", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  assert.equal(f.receive({ kind: "pending", quoteId: otherId }), false);
  assert.equal(f.receive({ kind: "committed", receipt: receipt({ costBoundGold: "124" }), replayed: false, reloadRequired: false }, snapshot()), false);
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false })); assert(f.client.query());
  assert.equal(f.receive({ kind: "committed", receipt: receipt({ operationKey: "changed" }), replayed: true, reloadRequired: false }, snapshot()), false);
  assert.equal(f.applied.length, 0); assert.equal(f.client.view().phase, "pending");
});
test("receipt field order does not change the immutable original identity", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false })); assert(f.client.query());
  const reversed = Object.fromEntries(Object.entries(receipt()).reverse());
  assert(f.receive({ kind: "committed", receipt: reversed, replayed: true, reloadRequired: false }, snapshot()));
});
test("unavailable or unauthenticated rejection cannot clear an uncertain Confirm", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  assert(f.receive({ kind: "rejected", quoteId, code: "unavailable" })); assert.equal(f.client.view().phase, "pending");
  assert(f.client.query()); assert(f.receive({ kind: "rejected", quoteId, code: "unauthenticated" }));
  assert.equal(f.client.quote(selection), false); assert.equal(f.client.confirm(), false);
});
test("only an original Query proving NotSubmitted releases an uncommitted attempt", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm()); assert(f.receive({ kind: "unknown", quoteId }));
  assert(f.client.query()); assert(f.receive({ kind: "rejected", quoteId, code: "notSubmitted" }));
  assert.equal(f.client.view().phase, "rejected"); assert.equal(f.client.confirm(), false); assert(f.client.quote(selection));
  assert.equal(f.receive({ kind: "quoted", quote: quote() }), false, "already-entered quote cannot be Confirmed again");
});
test("NotSubmitted cannot erase an already known committed receipt", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false })); assert(f.client.query());
  assert(f.receive({ kind: "rejected", quoteId, code: "notSubmitted" }));
  assert.equal(f.client.view().phase, "pending"); assert(f.client.view().receipt); assert.equal(f.client.quote(selection), false);
});
test("reentrant UI callbacks cannot submit a Confirm or Query while receiving a Quote or receipt", () => {
  const f = fixture(); f.state.onChange = view => {
    if (view.phase === "quoted") assert.equal(f.client.confirm(), false);
    if (view.receipt) assert.equal(f.client.query(), false);
  };
  f.quoted(); assert.equal(f.sent.length, 1); assert(f.client.confirm());
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot()));
  assert.equal(f.sent.length, 2); assert.equal(f.client.view().phase, "committed");
});
test("UI cancellation during Quote notification aborts sending before any charge", () => {
  const f = fixture(); f.state.onChange = view => { if (view.phase === "quoting") f.client.resetView(); };
  assert.equal(f.client.quote(selection), false); assert.equal(f.sent.length, 0); assert.equal(f.client.confirm(), false);
});
test("mode or actor authority change after Confirm rejects the old snapshot", () => {
  for (const change of [state => state.mode = "legacy", state => state.current = false]) {
    const f = fixture(); f.quoted(); assert(f.client.confirm()); change(f.state);
    assert.equal(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot()), false);
    assert.equal(f.applied.length, 0); assert.equal(f.client.view().phase, "pending");
  }
});
test("selection changes at the final Confirm boundary never send a charge", () => {
  const f = fixture(); f.quoted();
  f.state.onChange = view => { if (view.phase === "pending") f.state.selectionProof = false; };
  assert.equal(f.client.confirm(), false); assert.equal(f.sent.length, 1); assert.equal(f.client.view().phase, "pending");
  f.state.onChange = null; assert(f.client.query()); assert.equal(f.last().action.kind, "query");
});
test("receipt notification that retires the owner cannot apply a snapshot", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  f.state.onChange = view => { if (view.receipt) f.client.retireConnection(); };
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot()));
  assert.equal(f.client.view().phase, "retired"); assert.equal(f.applied.length, 0);
});
test("free repair still requires a complete applied result before allowing another action", () => {
  const f = fixture(); f.quoted({ costBoundGold: "0" }); assert(f.client.confirm());
  const free = receipt({ costBoundGold: "0", free: true });
  assert(f.receive({ kind: "committed", receipt: free, replayed: false, reloadRequired: false }));
  assert.equal(f.client.quote(selection), false); assert(f.client.query());
  assert(f.receive({ kind: "committed", receipt: free, replayed: true, reloadRequired: false }, snapshot()));
  assert.equal(f.client.view().phase, "committed");
});
test("isCurrent callback retiring the adapter cannot grant permission to apply afterward", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  f.state.duringCurrent = () => { if (f.client.view().receipt) f.client.retireConnection(); };
  assert(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot()));
  assert.equal(f.client.view().phase, "retired"); assert.equal(f.applied.length, 0);
});
test("view consumers and observers cannot mutate retained nested quote authority", () => {
  const f = fixture(); assert(f.client.quote(selection));
  const serverQuote = quote(), raw = frame(f.last().requestId, { kind: "quoted", quote: serverQuote });
  serverQuote.quoteId = otherId; serverQuote.costBoundGold = "0"; serverQuote.expiresAtMs = 1;
  f.state.onChange = view => {
    if (view.quote) assert.throws(() => { view.quote.costBoundGold = "0"; }, TypeError);
  };
  assert(f.client.receive(raw));
  const view = f.client.view();
  for (const [key, value] of [["quoteId", otherId], ["expiresAtMs", 0], ["uniqueId", "0"], ["special", true], ["costBoundGold", "0"]])
    assert.throws(() => { view.quote[key] = value; }, TypeError);
  assert.throws(() => { view.quote = quote({ quoteId: otherId }); }, TypeError);
  assert.throws(() => Object.defineProperty(view.quote, "quoteId", { value: otherId }), TypeError);
  assert(f.client.confirm()); assert.deepEqual(f.last().action, { kind: "confirm", quoteId });
  assert.equal(f.client.view().quote.costBoundGold, "123");
});
test("retained receipt and the entire accepted snapshot graph are deeply frozen", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  const reply = { kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false };
  const raw = frame(f.last().requestId, reply, snapshot());
  const parsed = api.parseHuntingRepairFrame(raw);
  const todo = [parsed];
  while (todo.length) {
    const value = todo.pop();
    if (value !== null && typeof value === "object") { assert(Object.isFrozen(value)); todo.push(...Object.values(value)); }
  }
  assert(f.client.receive(raw));
  assert.throws(() => { f.client.view().receipt.operationKey = "changed"; }, TypeError);
  assert.throws(() => { f.client.view().receipt.costBoundGold = "0"; }, TypeError);
  assert.throws(() => { f.applied[0].entities[0].objectId = 8; }, TypeError);
  assert.throws(() => f.applied[0].inventoryItems.push({}), TypeError);
  assert.equal(f.client.view().receipt.operationKey, "repair-operation");
});
test("callbacks timing out the final dispatch control cannot send an unowned Confirm", () => {
  const f = fixture(); f.quoted();
  f.state.duringCurrent = () => {
    if (f.client.view().phase === "pending") { f.state.now += 100; f.client.checkTimeout(); }
  };
  assert.equal(f.client.confirm(), false); assert.equal(f.sent.length, 1); assert.equal(f.client.view().phase, "pending");
  f.state.duringCurrent = null; assert(f.client.query()); assert.equal(f.last().action.kind, "query");
});
test("callbacks timing out a response control cannot apply its superseded snapshot", () => {
  const f = fixture(); f.quoted(); assert(f.client.confirm());
  f.state.duringCurrent = () => { f.state.now += 100; f.client.checkTimeout(); };
  assert.equal(f.receive({ kind: "committed", receipt: receipt(), replayed: false, reloadRequired: false }, snapshot()), false);
  assert.equal(f.applied.length, 0); assert.equal(f.client.view().phase, "pending");
  f.state.duringCurrent = null; assert(f.client.query());
});
