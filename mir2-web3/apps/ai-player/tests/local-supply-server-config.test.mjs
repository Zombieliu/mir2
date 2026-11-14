import assert from "node:assert/strict";
import test from "node:test";
import { randomUUID } from "node:crypto";
import { access, mkdir, mkdtemp, readFile, readdir, realpath, rm, symlink, writeFile } from "node:fs/promises";
import { basename, dirname, join, parse, resolve } from "node:path";
import { tmpdir } from "node:os";
import { canonicalPrivateDirectory, localSupplyServerEnvironment } from "../src/local-supply-server-config.mjs";
import { appRoot, isolatedEnvironment, prepareLocalGatewayPaths, resolveLocalOutputRoot } from "../scripts/local-gateway.mjs";
import { prepareLocalPlaytest } from "../src/local-playtest.mjs";

async function temporary(t) {
  const root = await mkdtemp(join(tmpdir(), "mir2-local-supply-config-"));
  t.after(async () => {
    assert.equal(dirname(root), resolve(tmpdir())); assert.ok(basename(root).startsWith("mir2-local-supply-config-"));
    await rm(root, { recursive: true, force: true });
  });
  return root;
}
const configArguments = directory => ({ directory, allowedRoot: directory, realmId: "ai-private-test", worldEpoch: "epoch:test" });

test("private supply key is generated independently once and reused for the fixed world", async t => {
  const root = await temporary(t), args = configArguments(root);
  const first = await localSupplyServerEnvironment(args), second = await localSupplyServerEnvironment(args);
  assert.deepEqual(Object.keys(first).sort(), ["MIR2_COMPANION_SUPPLY_JOURNAL_PATH", "MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY"].sort());
  assert.equal(first.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY, second.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY);
  assert.match(first.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY, /^[0-9a-f]{64}$/);
  assert.notEqual(first.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY, "af".repeat(32), "an archive key is not reused as the server MAC");
  assert.equal(first.MIR2_COMPANION_SUPPLY_JOURNAL_PATH, join(await realpath(root), "supply-server.journal.json"));
  assert.deepEqual(await readdir(root), ["supply-server.local.json"]);
  const before = await readFile(join(root, "supply-server.local.json"), "utf8");
  await writeFile(first.MIR2_COMPANION_SUPPLY_JOURNAL_PATH, "retained-synthetic-journal");
  await writeFile(`${first.MIR2_COMPANION_SUPPLY_JOURNAL_PATH}.initialized`, "retained-synthetic-head");
  const restarted = await localSupplyServerEnvironment(args);
  assert.equal(restarted.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY, first.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY);
  assert.equal(await readFile(join(root, "supply-server.local.json"), "utf8"), before);
});

test("a missing key with any surviving journal publication artifact fails without generating a replacement", async t => {
  for (const name of ["supply-server.journal.json", "supply-server.journal.json.initialized", "supply-server.journal.json.lock", "supply-server.journal.json.tmp.partial"]) {
    const root = await temporary(t), path = join(root, name), original = "retained-private-fixture";
    await writeFile(path, original);
    await assert.rejects(localSupplyServerEnvironment(configArguments(root)), /not reset or replaced/);
    assert.equal(await readFile(path, "utf8"), original); assert.deepEqual(await readdir(root), [name]);
  }
});

test("corrupt or mismatched private configuration is retained byte for byte", async t => {
  const fixtures = ["{corrupt", "x".repeat(8193)];
  const template = { schema: "mir2-companion-supply-local/1", realmId: "ai-private-test", worldEpoch: "epoch:test", journalFileName: "supply-server.journal.json", macKeyHex: "ab".repeat(32) };
  for (const update of [{ worldEpoch: "other" }, { realmId: "ai-other" }, { macKeyHex: "0".repeat(64) }, { macKeyHex: "AB".repeat(32) }, { journalFileName: "../other.json" }, { archiveKey: "af".repeat(32) }]) fixtures.push(JSON.stringify({ ...template, ...update }));
  for (const encoded of fixtures) {
    const root = await temporary(t), path = join(root, "supply-server.local.json"); await writeFile(path, encoded);
    await assert.rejects(localSupplyServerEnvironment(configArguments(root)), /not reset or replaced/);
    assert.equal(await readFile(path, "utf8"), encoded);
  }
});

test("gateway-only supply secrets do not enter actor/native inherited environments", () => {
  const inherited = { PATH: "fixture-bin", SystemRoot: "fixture-system", MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY: "test-only-server-mac", MIR2_COMPANION_SUPPLY_JOURNAL_PATH: "test-only-server-journal", OPENAI_API_KEY: "test-only-api", MIR2_COMPANION_ARCHIVE_KEY: "test-only-archive" };
  const actor = isolatedEnvironment({ MIR2_AI_PLAYER_ACCOUNT: "synthetic", MIR2_COMPANION_ARCHIVE_KEY: "explicit-actor-archive" }, inherited);
  const native = isolatedEnvironment({ MIR2_NATIVE_ACCOUNT: "synthetic-owner" }, inherited);
  const gateway = isolatedEnvironment({ MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY: "explicit-server-mac", MIR2_COMPANION_SUPPLY_JOURNAL_PATH: "explicit-server-journal" }, inherited);
  for (const child of [actor, native]) {
    assert.equal(Object.hasOwn(child, "MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY"), false);
    assert.equal(Object.hasOwn(child, "MIR2_COMPANION_SUPPLY_JOURNAL_PATH"), false);
    assert.equal(Object.hasOwn(child, "OPENAI_API_KEY"), false);
  }
  assert.equal(actor.MIR2_COMPANION_ARCHIVE_KEY, "explicit-actor-archive");
  assert.equal(Object.hasOwn(native, "MIR2_COMPANION_ARCHIVE_KEY"), false);
  assert.equal(gateway.MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY, "explicit-server-mac");
});

test("explicit private output roots contain both fresh outputs and reused account stores", async t => {
  const root = await temporary(t), outputRoot = join(root, "private space"), bootstrap = join(outputRoot, "bootstrap");
  await mkdir(bootstrap, { recursive: true }); await writeFile(join(bootstrap, "accounts.json"), "{}");
  const paths = await prepareLocalGatewayPaths({ prefix: "experience", runId: randomUUID(), outputRoot, reuse: { output: bootstrap, realmId: "ai-private-test" } });
  assert.equal(paths.outputRoot, await realpath(outputRoot));
  assert.equal(dirname(paths.output), paths.outputRoot); assert.equal(paths.accountStorePath, await realpath(join(bootstrap, "accounts.json")));
  await assert.rejects(prepareLocalGatewayPaths({ prefix: "experience", runId: randomUUID(), reuse: { output: bootstrap, realmId: "ai-private-test" } }), /private local account store/);
});

test("a different explicit root and malformed reuse cannot broaden the selected boundary", async t => {
  const root = await temporary(t), selected = join(root, "selected"), outside = join(root, "outside");
  await mkdir(selected); await mkdir(outside); await writeFile(join(outside, "accounts.json"), "{}");
  for (const reuse of [{ output: outside, realmId: "ai-private-test" }, { output: selected, realmId: "production" }, { output: outside, realmId: "ai-private-test", accountStorePath: "override" }]) await assert.rejects(prepareLocalGatewayPaths({ prefix: "experience", runId: randomUUID(), outputRoot: selected, reuse }));
  assert.deepEqual(await readdir(selected), []);
  await assert.rejects(prepareLocalGatewayPaths({ prefix: "../escape", runId: randomUUID(), outputRoot: selected }));
  for (const invalidRoot of ["relative-private", parse(resolve(appRoot)).root, resolve(appRoot), resolve(appRoot, "../..")]) await assert.rejects(resolveLocalOutputRoot(invalidRoot));
});

test("junctions cannot redirect reuse, key configuration, or directory creation outside its root", async t => {
  const root = await temporary(t), selected = join(root, "selected"), outside = join(root, "outside");
  await mkdir(selected); await mkdir(outside); await writeFile(join(outside, "accounts.json"), "{}");
  const alias = join(selected, "junction"); await symlink(outside, alias, process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(prepareLocalGatewayPaths({ prefix: "experience", runId: randomUUID(), outputRoot: selected, reuse: { output: alias, realmId: "ai-private-test" } }), /private local account store/);
  await assert.rejects(canonicalPrivateDirectory(join(alias, "must-not-create"), selected));
  await assert.rejects(localSupplyServerEnvironment({ ...configArguments(join(alias, "must-not-create")), allowedRoot: selected }));
  await assert.rejects(access(join(outside, "must-not-create")), { code: "ENOENT" });
  assert.deepEqual(await readdir(outside), ["accounts.json"]);
});

test("an explicit test root rejects an existing foreign local state before trying a gateway", async t => {
  const root = await temporary(t), stateFile = join(root, "local-playtest", "state.local.json");
  await mkdir(dirname(stateFile)); const encoded = "{retained-invalid-synthetic-state"; await writeFile(stateFile, encoded);
  await assert.rejects(prepareLocalPlaytest({ gatewayExecutable: "missing-test-binary", outputRoot: root, stateFile }), /no new character/);
  assert.equal(await readFile(stateFile, "utf8"), encoded);
  assert.deepEqual(await readdir(dirname(stateFile)), ["state.local.json"]);
});
