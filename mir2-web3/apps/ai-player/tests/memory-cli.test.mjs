import assert from "node:assert/strict";
import test from "node:test";
import { randomUUID, createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { CompanionStore } from "../src/memory/companion-store.mjs";
import { runMemoryCommand } from "../src/memory-cli.mjs";
import { acquireArchiveGuard } from "../src/archive-guard.mjs";

const KEY_ENV = "MIR2_MEMORY_CLI_TEST_KEY", KEY_HEX = "6c".repeat(32);
const BINDING = { realmId: "cli-world", residentId: "alo-cli", characterName: "Alo", characterIndex: 2 };
const CLI_PATH = fileURLToPath(new URL("../src/memory-cli.mjs", import.meta.url));
const fileHash = path => createHash("sha256").update(readFileSync(path)).digest("hex");

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), "mir2-memory-cli-test-"));
  const identity = { archiveId: "cli-" + randomUUID(), ownerRef: "owner:cli", worldEpoch: "cli-epoch", displayName: "阿洛", baseSetting: "比奇的合成人冒险者" };
  const source = join(root, "runtime-a.sqlite"), destination = join(root, "runtime-b", "memory.sqlite"), output = join(root, "portable.json");
  const store = new CompanionStore({ path: source, binding: BINDING, ...identity });
  store.teachSupportBeforeLoot({ source: "local-owner", feedbackId: "teaching-stays", feedbackText: "组队时先支援" });
  store.recordEpisode({ episodeId: "episode-stays", kind: "decision", action: "attack", outcome: "selected", summary: "真实的共同冒险" });
  const archive = store.archive(), growth = store.growth(); store.close();
  const profile = (path, runtimeId = "runner-a") => ({ url: "ws://127.0.0.1:7010/ws", realmId: BINDING.realmId, characterName: BINDING.characterName, guardPort: 17001,
    companion: { residentId: BINDING.residentId, ...identity, characterIndex: BINDING.characterIndex, memoryPath: path, leaderName: "Owner" },
    runtimeService: { controlEnabled: true, runtimeId, provider: "rules", model: "rules", promptVersion: runtimeId === "runner-a" ? "rules-v1" : "rules-v2", credentialRef: null, archiveKeyEnv: KEY_ENV } });
  const profileA = join(root, "runner-a.json"), profileB = join(root, "runner-b.json");
  writeFileSync(profileA, JSON.stringify(profile(source))); writeFileSync(profileB, JSON.stringify(profile(destination, "runner-b")));
  t.after(() => {
    const absolute = resolve(root);
    assert.ok(absolute.startsWith(resolve(tmpdir()) + sep) && basename(absolute).startsWith("mir2-memory-cli-test-"));
    rmSync(absolute, { recursive: true, force: true });
  });
  return { root, identity, source, destination, output, profileA, profileB, archive, growth, profile,
    environment: { [KEY_ENV]: KEY_HEX },
    async export() { return runMemoryCommand(["export", "--profile", profileA, "--output", output], { environment: this.environment }); },
    async import(options = {}) { return runMemoryCommand(["import", "--profile", options.profile ?? profileB, "--input", options.input ?? output], { environment: options.environment ?? this.environment }); },
    writeProfile(name, value) { const path = join(root, name + ".json"); writeFileSync(path, JSON.stringify(value)); return path; },
  };
}

test("CLI portability requires a trusted explicit profile and externally supplied signing key", async t => {
  const f = fixture(t);
  await assert.rejects(runMemoryCommand(["export", "--profile", f.profileA, "--output", f.output], { environment: {} }), /trusted owner archive key/);
  assert.equal(existsSync(f.output), false);
  const disabled = f.profile(f.source); disabled.runtimeService.controlEnabled = false;
  const disabledPath = f.writeProfile("disabled", disabled);
  await assert.rejects(runMemoryCommand(["export", "--profile", disabledPath, "--output", f.output], { environment: f.environment }), /trusted P0/);
  await assert.rejects(runMemoryCommand(["export", "--profile", f.profileA], { environment: f.environment }), /required/);
  await assert.rejects(runMemoryCommand(["export", "--profile", f.profileA, "--profile", f.profileB, "--output", f.output], { environment: f.environment }), /arguments/);
  assert.equal(existsSync(f.output), false);
  const inspection = await runMemoryCommand(["inspect", "--profile", f.profileA], { environment: {} });
  assert.equal(inspection.archive.archiveId, f.identity.archiveId); assert.equal(inspection.schemaVersion, 2);
  assert.equal(JSON.stringify(inspection).includes(KEY_HEX), false);
});

test("export creates a new file exclusively and does not overwrite an existing owner backup", async t => {
  const f = fixture(t);
  const exported = await f.export(); assert.equal(exported.type, "companionArchiveExported"); assert.equal(exported.policyVersion, 2);
  const original = readFileSync(f.output, "utf8"), envelope = JSON.parse(original);
  assert.equal(envelope.playerGrowth.episodes.length, 2);
  for (const field of ["runtimeService", "provider", "model", "promptVersion", "credentialRef", "memoryPath", "url", "guardPort"]) {
    assert.equal(Object.hasOwn(envelope, field) || Object.hasOwn(envelope.characterArchive, field) || Object.hasOwn(envelope.playerGrowth, field), false, field);
  }
  assert.equal(original.includes(KEY_HEX), false); assert.equal(original.includes(KEY_ENV), false);
  await assert.rejects(f.export(), error => error.code === "EEXIST");
  assert.equal(readFileSync(f.output, "utf8"), original);
});

test("fresh different-path import keeps stable identity, teaching and episodes despite another runtime/prompt config", async t => {
  const f = fixture(t); await f.export();
  const imported = await f.import(); assert.equal(imported.type, "companionArchiveImported"); assert.equal(imported.archiveId, f.identity.archiveId); assert.equal(imported.policyVersion, 2);
  const b = new CompanionStore({ path: f.destination, binding: BINDING, ...f.identity });
  try { assert.deepEqual(b.archive(), f.archive); assert.deepEqual(b.growth(), f.growth); assert.equal(b.profile().party.assistBeforeLoot, true); }
  finally { b.close(); }
  const retried = await f.import(); assert.deepEqual(retried, imported, "exact retry does not add or reset growth");
});

test("wrong key, foreign binding and over-limit files cannot touch an existing foreign destination", async t => {
  const f = fixture(t); await f.export();
  const target = join(f.root, "foreign.sqlite"), foreignBinding = { ...BINDING, realmId: "foreign-world" };
  const foreignIdentity = { ...f.identity, archiveId: "foreign-archive", ownerRef: "owner:foreign", worldEpoch: "foreign-epoch" };
  const foreign = new CompanionStore({ path: target, binding: foreignBinding, ...foreignIdentity });
  foreign.recordEpisode({ episodeId: "foreign-memory", kind: "decision", summary: "must stay intact" }); foreign.close();
  const digest = fileHash(target), targetProfile = f.profile(target, "runner-b"), targetPath = f.writeProfile("existing-target", targetProfile);
  await assert.rejects(f.import({ profile: targetPath, environment: { [KEY_ENV]: "7d".repeat(32) } }), /signature/);
  assert.equal(fileHash(target), digest);
  for (const changes of [{ ownerRef: "owner:other" }, { archiveId: "other-archive" }, { worldEpoch: "other-epoch" }, { characterIndex: 99 }]) {
    const profile = f.profile(target, "runner-b"); Object.assign(profile.companion, changes);
    await assert.rejects(f.import({ profile: f.writeProfile("wrong-" + Object.keys(changes)[0], profile) }), /binding/);
    assert.equal(fileHash(target), digest);
  }
  await assert.rejects(f.import({ profile: targetPath }), /fresh destination/);
  assert.equal(fileHash(target), digest);
  const huge = join(f.root, "over-limit.json"); writeFileSync(huge, "x".repeat(2 * 1024 * 1024 + 1));
  await assert.rejects(f.import({ profile: targetPath, input: huge }), /size limit/);
  assert.equal(fileHash(target), digest);
  const checked = new CompanionStore({ path: target, binding: foreignBinding, ...foreignIdentity });
  try { assert.equal(checked.recentEpisodes()[0].summary, "must stay intact"); }
  finally { checked.close(); }
});

test("memory CLI cannot import while the same stable archive guard is held by its runner", async t => {
  const f = fixture(t); await f.export();
  const release = await acquireArchiveGuard(f.identity.archiveId);
  try { await assert.rejects(f.import(), /already running|unavailable/); assert.equal(existsSync(f.destination), false); }
  finally { await release(); }
  assert.equal((await f.import()).archiveId, f.identity.archiveId);
});

test("actual CLI processes export A then import B without putting signing material in arguments or output", t => {
  const f = fixture(t);
  const environment = Object.fromEntries(Object.entries(process.env).filter(([name]) => ["PATH", "SYSTEMROOT", "WINDIR", "TEMP", "TMP"].includes(name.toUpperCase())));
  environment[KEY_ENV] = KEY_HEX;
  const run = args => JSON.parse(execFileSync(process.execPath, [CLI_PATH, ...args], { windowsHide: true, encoding: "utf8", env: environment, stdio: ["ignore", "pipe", "pipe"] }).trim());
  const exported = run(["export", "--profile", f.profileA, "--output", f.output]);
  const imported = run(["import", "--profile", f.profileB, "--input", f.output]);
  assert.equal(exported.archiveId, imported.archiveId); assert.equal(imported.policyVersion, 2); assert.equal(imported.growthRevision, exported.growthRevision);
  assert.equal(JSON.stringify([exported, imported]).includes(KEY_HEX), false);
  const opened = new CompanionStore({ path: f.destination, binding: BINDING, ...f.identity });
  try { assert.deepEqual(opened.archive(), f.archive); assert.deepEqual(opened.growth(), f.growth); }
  finally { opened.close(); }
});
