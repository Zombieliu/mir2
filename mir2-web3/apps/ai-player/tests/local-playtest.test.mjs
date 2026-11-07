import test from "node:test";
import assert from "node:assert/strict";
import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { prepareLocalPlaytest, validateLocalPlaytestState } from "../src/local-playtest.mjs";

const dataRoot = fileURLToPath(new URL("../.data/", import.meta.url));
function state() {
  const owner = { accountId: "ait1234567890", password: "Ab1234567890abcdef7", characterName: "OwnerA", characterIndex: 2 };
  const actor = { accountId: "ait0987654321", password: "Ab1234567890abcdef7", characterName: "AloA", characterIndex: 4 };
  return { schema: "mir2-alo-r16-local-playtest/1", owner, actor, archiveKey: "af".repeat(32), bootstrapOutput: resolve(dataRoot, "bootstrap"),
    companionPolicy: { realmId: "ai-local-test", worldEpoch: "epoch-1", enrollments: [{ residentId: "alo", archiveId: "archive-1", owner: { accountId: owner.accountId, characterIndex: 2 }, actor: { accountId: actor.accountId, characterIndex: 4 } }] },
    profile: { realmId: "ai-local-test", characterName: "AloA", companion: { residentId: "alo", archiveId: "archive-1", worldEpoch: "epoch-1", characterIndex: 4, leaderName: "OwnerA", ownerRef: `owner:${createHash("sha256").update(owner.accountId).digest("hex")}`, memoryPath: resolve(dataRoot, "alo.sqlite") },
      runtimeService: { controlEnabled: true, provider: "rules", model: "rules", credentialRef: null, archiveKeyEnv: "MIR2_COMPANION_ARCHIVE_KEY", experiment: { environment: "test", offer: null, ledgerPath: resolve(dataRoot, "experience.sqlite") } } } };
}

test("local restart preserves a consistent generated identity and exact owner/world/archive binding", () => {
  const original = state(), loaded = JSON.parse(JSON.stringify(original));
  assert.deepEqual(validateLocalPlaytestState(loaded, dataRoot), original);
  for (const mutate of [
    value => { value.profile.companion.archiveId = "other"; },
    value => { value.companionPolicy.worldEpoch = "other"; },
    value => { value.companionPolicy.enrollments[0].owner.characterIndex++; },
    value => { value.profile.companion.ownerRef = "owner:forged"; },
    value => { value.actor.characterIndex++; },
    value => { value.profile.companion.leaderName = "different"; },
  ]) {
    const invalid = structuredClone(original); mutate(invalid);
    assert.throws(() => validateLocalPlaytestState(invalid, dataRoot), /not replaced/);
  }
});

test("local launch rejects paths outside its private data and any model or payment activation", () => {
  for (const mutate of [
    value => { value.bootstrapOutput = resolve(dataRoot, "../foreign"); },
    value => { value.profile.companion.memoryPath = resolve(dataRoot, "../foreign.sqlite"); },
    value => { value.profile.runtimeService.experiment.ledgerPath = resolve(dataRoot, "../ledger.sqlite"); },
    value => { value.profile.runtimeService.model = "external-model"; },
    value => { value.profile.runtimeService.credentialRef = "secret-ref"; },
    value => { value.profile.runtimeService.experiment.environment = "trial"; },
    value => { value.profile.runtimeService.experiment.offer = { price: "1" }; },
  ]) {
    const invalid = state(); mutate(invalid);
    assert.throws(() => validateLocalPlaytestState(invalid, dataRoot));
  }
});

test("corrupt saved local state is retained and fails before starting a gateway or substituting a character", async () => {
  const directory = resolve(dataRoot, `launcher-test-${randomUUID()}`);
  await mkdir(directory, { recursive: true });
  const path = resolve(directory, "state.local.json"), encoded = "{broken-existing-private-state";
  await writeFile(path, encoded);
  try {
    await assert.rejects(prepareLocalPlaytest({ gatewayExecutable: "missing-test-binary", stateFile: path }), /no new character/);
    assert.equal(await readFile(path, "utf8"), encoded);
    assert.deepEqual(await readdir(directory), ["state.local.json"]);
  } finally {
    // Only this test's exact UUID directory, resolved inside our app data.
    assert.ok(directory.startsWith(resolve(dataRoot) + (process.platform === "win32" ? "\\" : "/")));
    await rm(directory, { recursive: true });
  }
});
