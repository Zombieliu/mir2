import assert from "node:assert/strict";
import test from "node:test";
import { EventEmitter } from "node:events";
import { createHash } from "node:crypto";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { performance } from "node:perf_hooks";
import { setImmediate as settle } from "node:timers/promises";
import { DatabaseSync } from "node:sqlite";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { CompanionRuntimeService } from "../src/companion-runtime-service.mjs";

const BINDING = { realmId: "service-world", residentId: "alo-service", characterName: "Alo", characterIndex: 2 };
const IDENTITY = { archiveId: "service-archive", ownerRef: "owner:service", worldEpoch: "service-epoch", displayName: "阿洛" };
const KEY_HEX = "5a".repeat(32), KEY_ENV = "MIR2_RUNTIME_SERVICE_TEST_KEY";

function fixture(t, { pendingRegistration = false, experiment = false } = {}) {
  const root = mkdtempSync(join(tmpdir(), "mir2-runtime-service-test-")), path = join(root, "growth.sqlite");
  const store = new CompanionStore({ path, binding: BINDING, ...IDENTITY });
  const archive = store.archive(), reports = [], replies = [];
  let now = 1000, next = 0, resolveRegistration;
  class Client extends EventEmitter {
    worldEpoch = IDENTITY.worldEpoch;
    registerCompanionRuntime() { return pendingRegistration ? new Promise(resolve => { resolveRegistration = resolve; }) : Promise.resolve({ status: "ok", result: { ...archive, generation: 7 } }); }
    replyCompanionRuntime(reply) { replies.push(reply); this.emit("reply", reply); }
    snapshot() { return { ready: true, playerObjectId: 1, selfZoneObjectId: 50001, player: { name: "Alo" }, party: { members: ["Alo", "Owner"] }, entities: [], connectionEpoch: 1, revision: 1 }; }
  }
  const client = new Client(), configuration = { runtimeId: "runner-test", provider: "rules", model: "rules", promptVersion: "rules-v1", archiveKeyEnv: KEY_ENV,
    ...(experiment ? { experiment: { experimentId: "test-experiment", ledgerPath: join(root, "experience.sqlite"), currency: "CNY", environment: "test" } } : {}) };
  const controller = { recallRequested: false, leaderName: "Owner" };
  const service = new CompanionRuntimeService({ client, store, controller, configuration, realmId: BINDING.realmId,
    clock: () => now, report: value => reports.push(value), environment: { [KEY_ENV]: KEY_HEX } });
  t.after(async () => {
    await service.close(); store.close();
    const absolute = resolve(root);
    assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("mir2-runtime-service-test-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  return { root, path, store, client, service, replies, reports, controller,
    advance(ms) { now += ms; }, lateRegistration() { resolveRegistration({ status: "ok", result: { ...archive, generation: 7 } }); },
    frame(operation, payload = {}, requestId = "request-" + ++next) { return { requestId, residentId: archive.residentId, archiveId: archive.archiveId, ownerRef: archive.ownerRef, worldEpoch: archive.worldEpoch, generation: 7, operation, payload, deadlineAtMs: now + 1000 }; },
    async request(operation, payload = {}, requestId) {
      const frame = this.frame(operation, payload, requestId);
      const result = new Promise((resolve, reject) => {
        const timer = setTimeout(() => { client.off("reply", received); reject(new Error("Runtime request did not produce its private receipt")); }, 1000);
        const received = value => { if (value.requestId === frame.requestId) { clearTimeout(timer); client.off("reply", received); resolve(value); } };
        client.on("reply", received);
      });
      client.emit("companionRequest", frame);
      return { frame, reply: await result };
    },
  };
}

for (const operation of ["deleteFeedback", "revokeFeedback", "deleteEpisode", "clearGrowth"]) {
  test(operation + " committed before owner-view failure returns unknown and invalidates every old export", async t => {
    const f = fixture(t); await f.service.start();
    const taught = await f.request("teach", { expectedPolicyVersion: 1 }, "teaching-one");
    assert.equal(taught.reply.status, "ok");
    f.store.recordEpisode({ episodeId: "private-event", kind: "decision", summary: "private text that clear must remove" });
    const exports = [];
    for (let index = 0; index < 2; index += 1) exports.push((await f.request("exportBegin")).reply.result);
    const sourceEpisode = f.store.growth().feedback.find(row => row.feedbackId === "teaching-one").episodeId;
    const originalView = f.store.ownerView.bind(f.store);
    f.store.ownerView = () => { throw new Error("private post-commit owner-view failure"); };
    const payload = { expectedPolicyVersion: 2, ...(operation === "clearGrowth" ? { confirmArchiveId: IDENTITY.archiveId } : operation === "deleteEpisode" ? { episodeId: sourceEpisode } : { feedbackId: "teaching-one" }) };
    const outcome = await f.request(operation, payload);
    assert.equal(outcome.reply.status, "unknown");
    assert.equal(JSON.stringify(outcome.reply).includes("private post-commit"), false);
    const db = new DatabaseSync(f.path, { readOnly: true });
    const feedback = db.prepare("SELECT status,payload_json FROM feedback WHERE feedback_id='teaching-one'").get();
    assert.equal(feedback.status, operation === "revokeFeedback" ? "revoked" : "deleted");
    if (operation !== "revokeFeedback") assert.equal(feedback.payload_json, "{}");
    db.close();
    assert.equal(f.store.profile().party.assistBeforeLoot, false);
    f.store.ownerView = originalView;
    for (const exported of exports) {
      const chunk = await f.request("exportChunk", { exportId: exported.exportId, index: 0 });
      assert.equal(chunk.reply.status, "error"); assert.equal(Object.hasOwn(chunk.reply.result, "dataBase64"), false);
    }
    const refreshed = await f.request("view"); assert.equal(refreshed.reply.result.profile.policyVersion, 3);
  });
}

test("teaching whose profile receipt fails after SQL commit is unknown and revokes its previous export", async t => {
  const f = fixture(t); await f.service.start();
  const previous = (await f.request("exportBegin")).reply.result;
  const originalProfile = f.store.profile.bind(f.store);
  f.store.profile = () => { throw new Error("post-commit profile failure"); };
  assert.equal((await f.request("teach", { expectedPolicyVersion: 1 }, "committed-teaching")).reply.status, "unknown");
  f.store.profile = originalProfile;
  assert.equal(f.store.profile().policyVersion, 2);
  assert.equal(f.store.growth().feedback[0].feedbackId, "committed-teaching");
  assert.equal((await f.request("exportChunk", { exportId: previous.exportId, index: 0 })).reply.status, "error");
});

test("chunked exports reconstruct the entire HMAC snapshot and expose neither key nor runtime configuration", async t => {
  const f = fixture(t); await f.service.start();
  for (let index = 0; index < 145; index += 1) f.store.recordEpisode({ episodeId: "history-" + index, kind: "decision", summary: "真实经历".repeat(50) });
  await f.request("teach", { expectedPolicyVersion: 1 });
  const view = (await f.request("view")).reply.result;
  assert.equal(JSON.stringify(view).includes(KEY_HEX), false); assert.equal(JSON.stringify(view).includes(KEY_ENV), false);
  const start = (await f.request("exportBegin")).reply;
  assert.equal(start.status, "ok"); assert.ok(start.result.chunkCount > 1);
  const chunks = [];
  for (let index = 0; index < start.result.chunkCount; index += 1) {
    const chunk = (await f.request("exportChunk", { exportId: start.result.exportId, index })).reply;
    assert.equal(chunk.status, "ok"); assert.equal(chunk.result.index, index); assert.equal(chunk.result.chunkCount, start.result.chunkCount);
    const data = Buffer.from(chunk.result.dataBase64, "base64"); assert.ok(data.length <= 16384); chunks.push(data);
  }
  const complete = Buffer.concat(chunks);
  assert.equal(complete.length, start.result.totalBytes);
  assert.equal(createHash("sha256").update(complete).digest("hex"), start.result.sha256);
  assert.equal(complete.toString("utf8").includes(KEY_HEX), false); assert.equal(complete.toString("utf8").includes(KEY_ENV), false);
  const envelope = JSON.parse(complete.toString("utf8"));
  assert.equal(envelope.playerGrowth.episodes.length, 146);
  assert.equal(Object.hasOwn(envelope, "runtimeService"), false);
  assert.equal(JSON.stringify(envelope).includes("promptVersion"), false);
  const restored = CompanionStore.importArchive({ path: join(f.root, "runner-b.sqlite"), binding: BINDING, ...IDENTITY, key: Buffer.from(KEY_HEX, "hex"), envelope: complete.toString("utf8") });
  try { assert.deepEqual(restored.archive(), f.store.archive()); assert.deepEqual(restored.growth(), f.store.growth()); }
  finally { restored.close(); }
});

test("expired exports and invalid chunk indices are unavailable without truncating or reusing snapshots", async t => {
  const f = fixture(t); await f.service.start();
  const exported = (await f.request("exportBegin")).reply.result;
  for (const index of [-1, exported.chunkCount, 1.5]) assert.equal((await f.request("exportChunk", { exportId: exported.exportId, index })).reply.status, "error");
  f.advance(120_001);
  assert.equal((await f.request("exportChunk", { exportId: exported.exportId, index: 0 })).reply.status, "error");
  assert.equal((await f.request("exportBegin")).reply.status, "ok");
});

test("closing pending registration releases queued requests promptly and a late ACK never starts metering", async t => {
  const f = fixture(t, { pendingRegistration: true, experiment: true });
  const starting = f.service.start();
  const startRejected = assert.rejects(starting, /closed during registration/);
  f.client.emit("companionRequest", f.frame("teach", { expectedPolicyVersion: 1 }, "queued-before-admission"));
  let timeout; const began = performance.now();
  try { await Promise.race([f.service.close(), new Promise((resolve, reject) => { timeout = setTimeout(() => reject(new Error("Pending registration close exceeded 500ms")), 500); })]); }
  finally { clearTimeout(timeout); }
  assert.ok(performance.now() - began < 500);
  assert.equal(f.client.listenerCount("companionRequest"), 0); assert.equal(f.client.listenerCount("frame"), 0);
  f.lateRegistration(); await startRejected; await settle();
  assert.equal(f.store.profile().policyVersion, 1); assert.equal(f.replies.length, 0);
  assert.equal(f.client.listenerCount("frame"), 0);
  assert.equal(f.reports.some(row => row.type === "companionControlReady"), false);
  const db = new DatabaseSync(join(f.root, "experience.sqlite"), { readOnly: true });
  assert.equal(db.prepare("SELECT COUNT(*) AS n FROM runtime_sessions").get().n, 0); db.close();
});
