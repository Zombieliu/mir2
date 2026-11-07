import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import test from "node:test";
import ts from "typescript";
const read = p => readFileSync(new URL(p, import.meta.url), "utf8");
const module = { exports: {} };
new Function("exports", "module", ts.transpileModule(read("../lib/r16-companion-client.ts"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText)(module.exports, module);
const { R16CompanionClient, R16_COMPANION_OPERATIONS, r16CompanionSupplyComplete, r16CompanionTeachingId } = module.exports;
const digest = bytes => createHash("sha256").update(bytes).digest("hex");
const source = () => ({ socket: {}, socketGeneration: 1, sessionId: "session-1", roleId: "canonical-account:role-2", realmId: "trial", worldEpoch: "world-1", contextEpoch: 1 });
const view = (version = 1, extra = {}) => ({ archive: { archiveId: "archive-1", residentId: "alo-1", displayName: "阿洛", characterName: "AloAI", realmId: "trial", worldEpoch: "world-1", baseSetting: "私人设定" },
  profile: { policyVersion: version, style: "collection", party: { assistBeforeLoot: version > 1, lootOnlyAfterCombat: version > 1, askBeforeLeaving: version > 1 } }, growthRevision: version,
  feedback: version > 1 ? [{ feedbackId: "feedback-1", status: "active", summary: "PRIVATE TEACHING", recordedAt: 100 }] : [],
  episodes: [{ episodeId: "episode-1", kind: "decision", summary: "PRIVATE MEMORY", action: "follow", outcome: "selected", recordedAt: 100 }], nextCursor: null,
  runtime: { provider: "rules", model: "rules", configured: false, modelEnabled: false, resources: { medicineUnresolved: false }, ...extra } });
function harness(extra = {}) {
  let current = source(), now = 1000, outcome = "sent";
  const sent = [], downloads = [], revoked = [];
  const host = { captureSource: () => current, now: () => now, randomId: () => "test-nonce", sendPrivate(s, frame) { assert.equal(s.socket, current.socket); sent.push(frame); return outcome; },
    sha256: async bytes => digest(bytes), makeDownload(bytes) { downloads.push(new Uint8Array(bytes)); return `blob:private-${downloads.length}`; }, revokeDownload(url) { revoked.push(url); }, ...extra };
  const client = new R16CompanionClient(host);
  function reply(result, { status = "ok", request = sent.at(-1), from = current, changes = {} } = {}) {
    return client.receive(from, { type: "companionResult", payload: { requestId: request.requestId, ...(request.residentId ? { residentId: request.residentId } : { residentId: null }), operation: request.operation, status, result, ...changes } });
  }
  function ready(value = view()) {
    assert(client.list(client.snapshot().renderLease));
    reply({ companions: [{ residentId: "alo-1", archiveId: "archive-1", displayName: "阿洛", online: true }], worldEpoch: current.worldEpoch });
    assert.equal(sent.at(-1).operation, "view"); reply(value); assert(client.snapshot().view);
  }
  return { client, host, sent, downloads, revoked, reply, ready, lease: () => client.snapshot().renderLease,
    get current() { return current; }, set current(value) { current = value; }, advance(ms) { now += ms; }, outcome(value) { outcome = value; } };
}
test("operation names match the actual Gateway enum and no browser model key enters the projected view", () => {
  const server = read("../../gateway/src/companion_relay.rs").split("pub enum Operation {")[1].split("}", 1)[0];
  const names = [...server.matchAll(/^\s+(\w+),/gm)].map(([, n]) => n[0].toLowerCase() + n.slice(1));
  assert.deepEqual(R16_COMPANION_OPERATIONS, names);
  const h = harness(); h.ready(view(1, { apiKey: "KEY SENTINEL", endpoint: "https://private.invalid", teachingProposal: { status: "idle", secret: "KEY SENTINEL" } }));
  assert(!JSON.stringify(h.client.snapshot()).includes("KEY SENTINEL"));
  assert(!JSON.stringify(h.client.snapshot()).includes("private.invalid"));
  assert(Object.isFrozen(h.client.snapshot().view.profile.party));
});
test("fixed teaching → read memory → authorize one 50-gold supply → revoke uses exact existing wire", () => {
  const h = harness(); h.ready();
  assert(h.client.act(h.lease(), { type: "teach" })); assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 1 });
  h.reply(view(2)); assert.equal(h.client.snapshot().view.profile.policyVersion, 2);
  assert(h.client.refresh(h.lease())); h.reply(view(2));
  assert.equal(h.client.snapshot().view.feedback[0].summary, "PRIVATE TEACHING");
  assert.equal(h.client.snapshot().view.episodes[0].summary, "PRIVATE MEMORY");
  assert(h.client.act(h.lease(), { type: "authorizeSupply" }));
  assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 2, budgetGold: 50, quantity: 1, expiresInSeconds: 300 });
  h.reply(view(2, { supplyTask: { status: "authorized", permitId: "permit-1", budgetGold: 50, chargedGold: null, acquiredQuantity: 0, receiptKnown: false, returnedToAnchor: false } }));
  assert.equal(r16CompanionSupplyComplete(h.client.snapshot().view.runtime.supplyTask), false);
  assert.equal(h.client.act(h.lease(), { type: "authorizeSupply" }), false);
  assert(h.client.act(h.lease(), { type: "cancelSupply" })); assert.deepEqual(h.sent.at(-1).payload, {});
  h.reply(view(2, { supplyTask: { status: "cancelled", permitId: "permit-1", budgetGold: 50, chargedGold: null, receiptKnown: false } }));
  assert.equal(h.client.snapshot().view.runtime.supplyTask.status, "cancelled");
  assert.equal(h.client.snapshot().view.runtime.supplyTask.chargedGold, null);
  assert(h.sent.every(f => f.type === "companionControl" && !Object.hasOwn(f, "password")));
});
test("every source dimension retires private state and rejects old render leases before any new send", () => {
  for (const key of ["socket", "socketGeneration", "sessionId", "roleId", "realmId", "worldEpoch", "contextEpoch"]) {
    const h = harness(); h.ready(); const old = h.lease(), from = h.current, count = h.sent.length;
    h.current = { ...from, [key]: key === "socket" ? {} : typeof from[key] === "number" ? from[key] + 1 : from[key] + "-new" };
    assert.equal(h.client.act(old, { type: "teach" }), false, key); assert.equal(h.sent.length, count);
    assert.equal(h.client.snapshot().view, null); assert.equal(h.client.invitation(old), null); assert.equal(h.client.acceptsSource(from), false);
    assert.equal(h.client.receive(from, { type: "companionResult", payload: { secret: "PRIVATE OLD" } }), true);
    assert(!JSON.stringify(h.client.snapshot()).includes("PRIVATE"));
  }
});
test("source/requestId/operation/resident must all match; malformed private frames are consumed", () => {
  const h = harness(); h.ready(); h.client.act(h.lease(), { type: "teach" }); const request = h.sent.at(-1);
  for (const changes of [{ requestId: "foreign" }, { operation: "recall" }, { residentId: "alo-foreign" }]) {
    assert(h.reply(view(2), { request, changes })); assert.equal(h.client.snapshot().pendingOperation, "teach");
  }
  h.reply(view(2)); assert.equal(h.client.snapshot().view.profile.policyVersion, 2);
  assert.equal(h.client.receive(h.current, { type: "companionResult", payload: "PRIVATE malformed" }), true);
  assert.equal(h.client.receive(h.current, { type: "worldSnapshot", payload: {} }), false);
  assert.equal(h.client.receive(h.current, { type: "companionResult", payload: { secret: "PRIVATE malformed" } }), true);
  assert.equal(h.client.snapshot().view.profile.policyVersion, 2);
});
test("18s timeout freezes mutation, retains the original unknown intent, and never automatically resends", () => {
  const h = harness(); h.ready(); const before = h.sent.length;
  h.client.act(h.lease(), { type: "authorizeSupply" }); const original = h.sent.at(-1);
  h.advance(17_999); h.client.tick(); assert.equal(h.client.snapshot().busy, true);
  h.advance(1); h.client.tick(); assert.equal(h.client.snapshot().busy, false); assert.equal(h.client.snapshot().uncertain, true);
  assert.equal(h.client.snapshot().unknownIntent.requestId, original.requestId);
  for (let i = 0; i < 5; i++) { h.advance(10000); h.client.tick(); assert.equal(h.client.act(h.lease(), { type: "authorizeSupply" }), false); }
  assert.equal(h.sent.length, before + 1);
  assert(h.client.refresh(h.lease())); assert.equal(h.sent.at(-1).operation, "view");
  h.reply(view(1, { supplyTask: { status: "unknown", permitId: "permit-1" } }));
  assert.equal(h.client.snapshot().uncertain, false); assert.equal(h.client.snapshot().unknownIntent.requestId, original.requestId);
  assert.equal(h.client.act(h.lease(), { type: "authorizeSupply" }), false);
});
test("unknown send exceptions freeze; definite not-sent does not invent a committed result", () => {
  for (const outcome of ["unknown", "not-sent", "throws"]) {
    const h = harness(); h.ready(); h.outcome(outcome);
    if (outcome === "throws") h.host.sendPrivate = () => { throw new Error("PRIVATE ERROR"); };
    assert.equal(h.client.act(h.lease(), { type: "teach" }), false);
    assert.equal(h.client.snapshot().uncertain, outcome !== "not-sent"); assert(!h.client.snapshot().notice.includes("PRIVATE"));
  }
});
test("copied/retired leases, policy rollback, foreign archives and stale row IDs cannot mutate", () => {
  const h = harness(); h.ready(view(2)); const lease = h.lease();
  assert.equal(h.client.act({ ...lease }, { type: "teach" }), false);
  assert.equal(h.client.act(lease, { type: "deleteFeedback", feedbackId: "foreign" }), false);
  h.client.refresh(lease); assert.equal(h.client.act(lease, { type: "teach" }), false);
  h.reply(view(1)); assert.equal(h.client.snapshot().view, null);
  h.client.refresh(h.lease()); const wrong = view(3); wrong.archive.archiveId = "foreign"; h.reply(wrong);
  assert.equal(h.client.snapshot().view, null);
  h.client.refresh(h.lease()); h.reply(view(3)); assert.equal(h.client.snapshot().view.profile.policyVersion, 3);
});
test("memory deletion drops private rendered text before sending, with server version and row identity", () => {
  const h = harness(); h.ready(view(2)); const stale = h.lease();
  assert(h.client.act(stale, { type: "deleteEpisode", episodeId: "episode-1" }));
  assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 2, episodeId: "episode-1" });
  assert.equal(h.client.snapshot().view, null); assert(!JSON.stringify(h.client.snapshot()).includes("PRIVATE MEMORY"));
  assert.equal(h.client.act(stale, { type: "teach" }), false);
});
test("model proposal accepts explicit false only under the current policy; life keeps fixed limits", () => {
  const h = harness(); h.ready(view(2, { provider: "openai", model: "configured-node-model", configured: true, modelEnabled: true,
    teachingProposal: { proposalId: "proposal-1", status: "proposed", sourceVersion: 2, changes: { askBeforeLeaving: false } } }));
  assert.equal(r16CompanionTeachingId(h.client.snapshot().view), "proposal-1");
  h.client.act(h.lease(), { type: "confirmTeaching", proposalId: "proposal-1" });
  assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 2, proposalId: "proposal-1" });
  h.reply(view(3)); assert(h.client.act(h.lease(), { type: "authorizeLife", durationSeconds: 1200 }));
  assert.deepEqual(h.sent.at(-1).payload, { expectedPolicyVersion: 3, budgetGold: 50, durationSeconds: 1200 });
  h.reply(view(3)); assert.equal(h.client.act(h.lease(), { type: "authorizeLife", durationSeconds: 9999 }), false);
  h.client.refresh(h.lease()); h.reply(view(3, { teachingProposal: { proposalId: "proposal-2", status: "proposed", sourceVersion: 3, changes: { askBeforeLeaving: false, assistBeforeLoot: "false" } } }));
  assert.equal(r16CompanionTeachingId(h.client.snapshot().view), null);
});
test("a synchronous subscriber changing the character prevents private transmission", () => {
  const h = harness(); h.ready(); const count = h.sent.length;
  const unsub = h.client.subscribe(() => { h.current = null; h.client.syncSource(); });
  assert.equal(h.client.act(h.lease(), { type: "teach" }), false); unsub();
  assert.equal(h.sent.length, count); assert.equal(h.client.snapshot().view, null);
});
async function exportBytes(h, bytes, overrides = {}) {
  h.client.exportArchive(h.lease());
  h.reply({ exportId: "export-1", totalBytes: bytes.length, chunkCount: Math.ceil(bytes.length / 16384), sha256: digest(bytes), ...overrides });
  const count = Math.ceil(bytes.length / 16384);
  for (let index = 0; index < count && h.client.snapshot().busy; index++) {
    assert.equal(h.sent.at(-1).operation, "exportChunk"); assert.deepEqual(h.sent.at(-1).payload, { exportId: "export-1", index });
    h.reply({ exportId: "export-1", index, chunkCount: count, dataBase64: Buffer.from(bytes.subarray(index * 16384, (index + 1) * 16384)).toString("base64") });
  }
  await Promise.resolve(); await Promise.resolve();
}
test("export assembles exact sequential 16KiB chunks, verifies bytes/SHA, and revokes Blob on retirement", async () => {
  const h = harness(); h.ready(); const bytes = new Uint8Array(16384 + 17).map((_, i) => i % 251);
  await exportBytes(h, bytes); assert.equal(h.downloads.length, 1); assert.deepEqual(h.downloads[0], bytes);
  assert.equal(h.client.snapshot().download.fileName, "companion-export-1.json");
  h.client.retire(); assert.deepEqual(h.revoked, ["blob:private-1"]); assert.equal(h.client.snapshot().download, null);
  assert.equal(h.client.snapshot().view, null);
});
test("export rejects excessive manifests, hash mismatch, wrong order and noncanonical base64", async () => {
  for (const kind of ["large", "hash", "order", "base64"]) {
    const h = harness(); h.ready(); const bytes = new Uint8Array([1, 2, 3]);
    if (kind === "large") { h.client.exportArchive(h.lease()); h.reply({ exportId: "export-1", totalBytes: 2097153, chunkCount: 129, sha256: digest(bytes) }); }
    if (kind === "hash") await exportBytes(h, bytes, { sha256: "0".repeat(64) });
    if (["order", "base64"].includes(kind)) {
      h.client.exportArchive(h.lease()); h.reply({ exportId: "export-1", totalBytes: 3, chunkCount: 1, sha256: digest(bytes) });
      h.reply({ exportId: "export-1", index: kind === "order" ? 1 : 0, chunkCount: 1, dataBase64: kind === "base64" ? "A=BC" : "AQID" });
    }
    assert.equal(h.downloads.length, 0, kind); assert.equal(h.client.snapshot().download, null); assert.equal(h.client.snapshot().busy, false);
  }
});
test("export expiration and a delayed hash cannot publish private bytes into a new session", async () => {
  let complete; const h = harness({ sha256: () => new Promise(resolve => { complete = resolve; }) }); h.ready();
  const bytes = new Uint8Array([1, 2, 3]); await exportBytes(h, bytes); assert(complete);
  h.current = { ...h.current, contextEpoch: 2 }; h.client.syncSource(); complete(digest(bytes)); await Promise.resolve(); await Promise.resolve();
  assert.equal(h.downloads.length, 0); assert.equal(h.client.snapshot().download, null);
  const live = harness(); live.ready(); await exportBytes(live, bytes); live.advance(120000); live.client.tick();
  assert.equal(live.client.snapshot().download, null); assert.deepEqual(live.revoked, ["blob:private-1"]);
});
test("completion requires actual charge, quantity, receipt and return evidence", () => {
  const done = { status: "completed", budgetGold: 50, chargedGold: 40, acquiredQuantity: 1, receiptKnown: true, returnedToAnchor: true };
  assert(r16CompanionSupplyComplete(done));
  for (const [key, value] of [["chargedGold", null], ["chargedGold", 51], ["acquiredQuantity", 0], ["receiptKnown", false], ["returnedToAnchor", false], ["budgetGold", null]]) assert.equal(r16CompanionSupplyComplete({ ...done, [key]: value }), false);
});
test("offline/invalid/error receipts remove actionable old views and Node cursors stay strictly numeric", () => {
  for (const status of ["offline", "invalid", "error"]) {
    const h = harness(); h.ready(); h.client.act(h.lease(), { type: "teach" }); h.reply({}, { status });
    assert.equal(h.client.snapshot().view, null); assert.equal(h.client.act(h.lease(), { type: "teach" }), false);
  }
  const h = harness(); h.ready();
  for (const cursor of ["uuid-cursor", "0", "-1", "9007199254740999"]) assert.equal(h.client.refresh(h.lease(), cursor), false);
  assert(h.client.refresh(h.lease(), "12")); assert.deepEqual(h.sent.at(-1).payload, { limit: 20, cursor: "12" });
});
test("export lifetime starts with its request, so network delay cannot extend the 120-second bound", async () => {
  const h = harness(); h.ready(); const bytes = new Uint8Array([1, 2, 3]);
  h.client.exportArchive(h.lease()); h.advance(15000);
  h.reply({ exportId: "export-1", totalBytes: 3, chunkCount: 1, sha256: digest(bytes) });
  h.reply({ exportId: "export-1", index: 0, chunkCount: 1, dataBase64: "AQID" }); await Promise.resolve(); await Promise.resolve();
  assert.equal(h.downloads.length, 1); h.advance(105000); h.client.tick();
  assert.equal(h.client.snapshot().download, null); assert.deepEqual(h.revoked, ["blob:private-1"]);
});
test("offline enrollment and offline receipts recover on a successful current Actor view", () => {
  const h = harness(); h.client.list(h.lease());
  h.reply({ companions: [{ residentId: "alo-1", archiveId: "archive-1", displayName: "阿洛", online: false }], worldEpoch: "world-1" });
  h.reply({}, { status: "offline" });
  assert.equal(h.client.snapshot().enrollments[0].online, false); assert.equal(h.client.invitation(h.lease()), null);
  assert(h.client.refresh(h.lease())); h.reply(view());
  assert.equal(h.client.snapshot().enrollments[0].online, true); assert.equal(h.client.invitation(h.lease()).characterName, "AloAI");
  assert(h.client.act(h.lease(), { type: "teach" })); h.reply({}, { status: "offline" });
  assert.equal(h.client.snapshot().enrollments[0].online, false);
  h.client.refresh(h.lease()); h.reply(view()); assert(h.client.exportArchive(h.lease()));
});
test("same-content read polls keep privacy epoch and server cursor while still retiring each render lease", () => {
  const h = harness(); h.ready(); const epoch = h.client.snapshot().contentEpoch, stale = h.lease();
  h.client.poll(stale); h.reply(view(1, { teachingProposal: { status: "proposed", proposalId: "proposal-1", sourceVersion: 1, changes: { askBeforeLeaving: false } } }));
  assert.equal(h.client.snapshot().contentEpoch, epoch); assert.notEqual(h.lease(), stale); assert.equal(h.client.act(stale, { type: "teach" }), false);
  h.client.refresh(h.lease(), "12"); h.reply(view()); const pageEpoch = h.client.snapshot().contentEpoch;
  h.client.poll(h.lease()); assert.deepEqual(h.sent.at(-1).payload, { limit: 20, cursor: "12" }); h.reply(view());
  assert.equal(h.client.snapshot().contentEpoch, pageEpoch);
  h.client.poll(h.lease()); h.reply(view(2)); assert(h.client.snapshot().contentEpoch > pageEpoch);
});
test("instruction matcher agrees with the actual Node service on invisible formats and NFKC credential checks", () => {
  const plan = read("../../ai-player/src/model/plan.mjs"), ast = ts.createSourceFile("plan.mjs", plan, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const fn = ast.statements.find(n => ts.isFunctionDeclaration(n) && n.name?.text === "instructionText"); assert(fn);
  const service = new Function("Buffer", `return (${fn.getText(ast).replace(/^export\s+/, "")});`)(Buffer);
  const { r16CompanionInstruction } = module.exports;
  for (const character of ["\u0001", "\u007F", "\u0085", "\u200B", "\u200C", "\u200D", "\u2060", "\uFEFF", "\uD800"]) {
    const value = `跟我${character}回城`; assert.throws(() => service(value)); assert.equal(r16CompanionInstruction(value), null);
  }
  for (const value of ["api key=TEST_ONLY_PLACEHOLDER", "密钥是测试占位", "https://user:placeholder@example.invalid", "ａｐｉ＿ｋｅｙ＝ＴＥＳＴ"]) {
    assert.throws(() => service(value)); assert.equal(r16CompanionInstruction(value), null);
  }
  for (const value of ["跟我回城", "ＡＢＣ跟我", "😀".repeat(1000)]) assert.equal(r16CompanionInstruction(value), service(value));
  const h = harness(); h.ready(view(1, { provider: "openai", configured: true, modelEnabled: true })); const count = h.sent.length;
  assert.equal(h.client.act(h.lease(), { type: "instruct", text: "跟我\u200B回城" }), false); assert.equal(h.sent.length, count);
});
test("unchanged read polling preserves an already verified private download until its real expiry", async () => {
  const h = harness(); h.ready(); await exportBytes(h, new Uint8Array([1, 2, 3]));
  const download = h.client.snapshot().download; h.client.poll(h.lease()); h.reply(view());
  assert.equal(h.client.snapshot().download, download); assert.deepEqual(h.revoked, []);
  h.client.poll(h.lease()); h.reply(view(2)); assert.equal(h.client.snapshot().download, null); assert.deepEqual(h.revoked, [download.url]);
});
