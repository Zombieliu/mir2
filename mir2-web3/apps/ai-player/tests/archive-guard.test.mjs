import assert from "node:assert/strict";
import test from "node:test";
import { createHash, randomUUID } from "node:crypto";
import { createServer } from "node:net";
import { spawn } from "node:child_process";
import { acquireArchiveGuard } from "../src/archive-guard.mjs";
import { ResidentActor } from "../src/resident-actor.mjs";

async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => { server.once("error", reject); server.listen(0, "127.0.0.1", resolve); });
  const port = server.address().port;
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  return port;
}

test("archive lock uses the stable hash and a released lock can be acquired again", async t => {
  const id = "guard-" + randomUUID();
  let release = await acquireArchiveGuard(id);
  t.after(async () => { if (release) await release(); });
  const digest = createHash("sha256").update("numeron-companion-archive-lock/1:" + id).digest();
  const expectedPort = 20_000 + digest.readUInt32BE(0) % 20_000;
  const probe = createServer();
  await assert.rejects(new Promise((resolve, reject) => { probe.once("error", reject); probe.listen({ host: "127.0.0.1", port: expectedPort }, resolve); }), { code: "EADDRINUSE" });
  if (probe.listening) await new Promise(resolve => probe.close(resolve));
  await assert.rejects(acquireArchiveGuard(id), /already running|unavailable/);
  await release(); release = null;
  release = await acquireArchiveGuard(id);
});

test("different path, runtime ID and guardPort cannot bypass an active archive before login", async t => {
  const archiveId = "guard-prelogin-" + randomUUID();
  let release = await acquireArchiveGuard(archiveId);
  t.after(async () => { if (release) await release(); });
  const calls = [];
  const archiveKeyEnv = "MIR2_TEST_ARCHIVE_GUARD_KEY_" + randomUUID().replaceAll("-", "").toUpperCase();
  process.env[archiveKeyEnv] = "07".repeat(32);
  t.after(() => { delete process.env[archiveKeyEnv]; });
  const client = { snapshot: () => ({ ready: false }), connect: async () => { calls.push("connect"); throw new Error("Reached network after lock release"); }, close: async () => { calls.push("close"); } };
  const profile = { realmId: "trial", characterName: "Alo", guardPort: await freePort(),
    companion: { archiveId, residentId: "alo", ownerRef: "owner:test", worldEpoch: "epoch", characterIndex: 2, leaderName: "Owner", memoryPath: "unused-runner-a.sqlite" },
    runtimeService: { controlEnabled: true, runtimeId: "runtime-a", provider: "rules", model: "rules", promptVersion: "rules-v1", credentialRef: null, archiveKeyEnv } };
  for (const [index, guardPort] of [[1, await freePort()], [2, await freePort()]]) {
    const changed = { ...profile, guardPort, companion: { ...profile.companion, memoryPath: "another-directory/runner-" + index + ".sqlite" }, runtimeService: { ...profile.runtimeService, runtimeId: "runtime-" + index } };
    await assert.rejects(new ResidentActor(changed, { client }).start({ accountId: "test", password: "test-only" }), /already running|unavailable/);
  }
  assert.deepEqual(calls, [], "archive rejection must precede connect, login and database creation");
  await release(); release = null;
  await assert.rejects(new ResidentActor(profile, { client }).start({ accountId: "test", password: "test-only" }), /Reached network/);
  assert.deepEqual(calls, ["connect", "close"]);
  const again = await acquireArchiveGuard(archiveId); await again();
});

test("the same archive in a distinct Node process is rejected until that process stops", async t => {
  const id = "guard-process-" + randomUUID(), moduleUrl = new URL("../src/archive-guard.mjs", import.meta.url).href;
  const child = spawn(process.execPath, ["--input-type=module", "-e", "import{acquireArchiveGuard}from'" + moduleUrl + "';const release=await acquireArchiveGuard(" + JSON.stringify(id) + ");process.send('held');process.on('message',async value=>{if(value==='release'){await release();process.disconnect();}});"], { windowsHide: true, stdio: ["ignore", "ignore", "pipe", "ipc"] });
  child.stderr.on("data", () => {});
  t.after(() => { if (child.exitCode === null) child.kill(); });
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Child archive guard did not start")), 5000);
    child.once("error", error => { clearTimeout(timeout); reject(error); });
    child.once("message", message => { clearTimeout(timeout); message === "held" ? resolve() : reject(new Error("Unexpected child guard response")); });
  });
  await assert.rejects(acquireArchiveGuard(id), /already running|unavailable/);
  const exited = new Promise((resolve, reject) => { child.once("error", reject); child.once("exit", code => code === 0 ? resolve() : reject(new Error("Child guard failed"))); });
  child.send("release"); await exited;
  const release = await acquireArchiveGuard(id); await release();
});

test("missing, non-reference and control-character archive identities cannot acquire locks", async () => {
  for (const value of [null, "", "archive with spaces", "archive\nother", "阿洛"]) await assert.rejects(acquireArchiveGuard(value), /stable archive identity/);
});
