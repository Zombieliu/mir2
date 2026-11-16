import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, readdir, realpath, rm, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve, sep } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import { newPlayableState, parsePlayableArguments, playableActorProfile, playableActorWorkerEnvironment,
  playableBaseEnvironment, playableControlAddress, playableGatewayEnvironment, playableNextEnvironment, playablePaths,
  playablePolicy, playableWalletAuthEnvironment, SERVICE_PORTS, validatePlayableState } from "./r16-playable-service-config.mjs";
import { preparePlayableState } from "./r16-playable-service.mjs";
import { R16_GAMEPLAY_BASELINE } from "../../ai-player/src/generated-gameplay-baseline.mjs";

async function temporary(t) {
  const root = await mkdtemp(join(tmpdir(), "r16-playable-config-"));
  t.after(async () => {
    const canonical = await realpath(root), parent = await realpath(tmpdir());
    assert.ok(canonical.startsWith(parent + sep) && dirname(canonical) === parent && basename(canonical).startsWith("r16-playable-config-"));
    await rm(canonical, { recursive: true });
  });
  return root;
}
function readyState() {
  const state = newPlayableState(); state.phase = "ready"; state.owner.characterIndex = 1; state.actor.characterIndex = 2;
  return validatePlayableState(state);
}
async function readyFiles(t, { initialized = false } = {}) {
  const root = await temporary(t), paths = playablePaths(root), state = readyState(); state.actorStoreInitialized = initialized;
  for (const path of [paths.supply, paths.recovery, paths.memory, paths.logs]) await mkdir(path);
  await writeFile(paths.state, JSON.stringify(state)); await writeFile(paths.accounts, "synthetic existing account bytes");
  await writeFile(paths.policy, JSON.stringify(playablePolicy(state)));
  await writeFile(join(paths.supply, "supply-server.local.json"), JSON.stringify({ schema: "mir2-companion-supply-local/1",
    realmId: state.world.realmId, worldEpoch: state.world.worldEpoch, journalFileName: "supply-server.journal.json", macKeyHex: "af".repeat(32) }));
  if (initialized) for (const path of [paths.actorMemory, paths.actorMemory + ".supply.sqlite", paths.actorMemory + ".life.sqlite", paths.actorMemory + ".gifts.sqlite", paths.experience]) await writeFile(path, "synthetic SQLite preservation bytes");
  return { root, paths, state };
}

test("run arguments require explicit existing-location choices; controls cannot activate or retarget services", () => {
  const root = resolve(tmpdir(), "explicit-r16"), paths = ["--gateway-exe", join(root, "gateway.exe"), "--server-entry", join(root, "web/server.js"),
    "--data-dir", join(root, "private"), "--web-root", join(root, "web"), "--node-exe", join(root, "node.exe")];
  assert.equal(parsePlayableArguments([...paths, "--with-alo"]).withAlo, true);
  assert.deepEqual(SERVICE_PORTS, { gateway: 19040, web: 19041, actor: 19042 });
  assert.equal(parsePlayableArguments(["--data-dir", root, "--command", "stop"]).command, "stop");
  for (const args of [[], paths.slice(0, -2), [...paths, "--data-dir", root], [...paths, "--unknown", root],
    ["--data-dir", "relative"], ["--data-dir", root, "--command", "stop", "--with-alo"]]) assert.throws(() => parsePlayableArguments(args));
});

test("generated credentials fit Crystal's 15-character alphanumeric boundary and each secret has independent entropy", () => {
  for (let i = 0; i < 40; i++) {
    const state = validatePlayableState(newPlayableState(), { allowInitializing: true });
    for (const role of [state.owner, state.actor]) for (const key of ["accountId", "password", "characterName"]) assert.match(role[key], /^[A-Za-z0-9]{8,15}$/);
    assert.equal(new Set(Object.values(state.keys)).size, 5);
    assert.throws(() => validatePlayableState(state), "unfinished bootstrap is not a restartable ready world");
  }
});

test("world, archive and recovery secrets stay unchanged across valid JSON reloads", () => {
  const state = readyState(), loaded = JSON.parse(JSON.stringify(state));
  assert.deepEqual(validatePlayableState(loaded), state);
  const profile = playableActorProfile(loaded, resolve(tmpdir(), "r16-private"));
  assert.equal(profile.companion.archiveId, state.world.archiveId);
  assert.equal(profile.companion.worldEpoch, state.world.worldEpoch);
  assert.equal(profile.runtimeService.provider, "rules");
  const publicConfiguration = JSON.stringify([profile, playablePolicy(loaded)]);
  for (const value of [...Object.values(state.keys), state.owner.password, state.actor.password]) assert.ok(!publicConfiguration.includes(value));
  for (const key of Object.keys(state.keys)) { const broken = structuredClone(state); delete broken.keys[key]; assert.throws(() => validatePlayableState(broken)); }
});

test("Gateway alone receives File recovery/supply keys; only the dedicated wallet signer is shared with Next", () => {
  const state = readyState(), root = resolve(tmpdir(), "r16-private"), paths = playablePaths(root);
  const ambient = { PATH: "system-path", SystemRoot: "system-root", OPENAI_API_KEY: "private-api-key", CODEX_API_KEY: "codex-key",
    DATABASE_URL: "production-database", MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS: "1", HTTPS_PROXY: "http://127.0.0.1:1234",
    MIR2_CONTENT_PROFILE: "crystal_full", MIR2_QUEST_CADENCE: "newcomer-v1", NEXT_PUBLIC_MIR2_QUEST_GUIDANCE: "crystal" };
  const supply = { MIR2_COMPANION_SUPPLY_JOURNAL_PATH: join(paths.supply, "supply-server.journal.json"), MIR2_COMPANION_SUPPLY_JOURNAL_MAC_KEY: "ac".repeat(32) };
  const gateway = playableGatewayEnvironment(state, root, { enrolled: true, supply, environment: ambient }), next = playableNextEnvironment(state, ambient);
  assert.equal(gateway.MIR2_SAVE_RECOVERY_MAC_KEY, state.keys.saveRecoveryMacKey);
  assert.equal(gateway.MIR2_IDENTITY_SESSION_SECRET, state.keys.identitySessionSecret);
  assert.equal(gateway.MIR2_GATEWAY_ALLOW_UNSAFE_LOCAL_PLAYER_COMMANDS, "0");
  assert.equal(gateway.MIR2_ACCOUNT_STORE_BACKEND, "file"); assert.equal(gateway.MIR2_ZONE_ROUTING_MODE, "single");
  assert.equal(gateway.MIR2_CHANNEL_IDENTITY_STORE_PATH, join(root, "channel-identities.json"));
  assert.notEqual(gateway.MIR2_CHANNEL_IDENTITY_STORE_PATH, gateway.MIR2_ACCOUNT_STORE_PATH);
  assert.equal(next.MIR2_CHANNEL_IDENTITY_STORE_PATH, undefined);
  assert.equal(gateway.MIR2_CONTENT_PROFILE, "platinum_fiveclass");
  assert.equal(gateway.MIR2_CONTENT_PROFILE, R16_GAMEPLAY_BASELINE.contentProfile);
  assert.equal(gateway.MIR2_QUEST_CADENCE, R16_GAMEPLAY_BASELINE.questCadence);
  assert.equal(next.NEXT_PUBLIC_MIR2_QUEST_GUIDANCE, R16_GAMEPLAY_BASELINE.questGuidance);
  assert.equal(next.MIR2_QUEST_CADENCE, undefined);
  for (const environment of [gateway, next, playableBaseEnvironment(ambient), playableActorWorkerEnvironment(ambient)]) {
    assert.equal(environment.OPENAI_API_KEY, undefined); assert.equal(environment.CODEX_API_KEY, undefined); assert.equal(environment.DATABASE_URL, undefined);
  }
  assert.equal(next.MIR2_PASSKEY_AUTH_SECRET, gateway.MIR2_PASSKEY_AUTH_SECRET);
  assert.equal(next.MIR2_PASSKEY_ALLOWED_ORIGINS, "http://127.0.0.1:19041");
  for (const key of Object.keys(gateway).filter(key => /MAC_KEY|SECRET|PEPPER|POLICY/.test(key) && key !== "MIR2_PASSKEY_AUTH_SECRET")) assert.equal(next[key], undefined);
  assert.equal(next.HTTPS_PROXY, ambient.HTTPS_PROXY); assert.equal(gateway.HTTPS_PROXY, undefined);
  assert.equal(playableActorWorkerEnvironment(ambient).HTTPS_PROXY, ambient.HTTPS_PROXY);
  assert.equal(next.HOSTNAME, "127.0.0.1"); assert.equal(next.PORT, "19041");
  assert.throws(() => playableGatewayEnvironment(state, root, { enrolled: true, supply: { ...supply, OPENAI_API_KEY: "forged" } }));
});

test("both R16 service environments consume one public gameplay baseline without migrating persistent state", () => {
  const state = readyState(), before = JSON.stringify(state), root = resolve(tmpdir(), "r16-baseline-config-only");
  const gateway = playableGatewayEnvironment(state, root, { environment: {} }), next = playableNextEnvironment(state, {});
  assert.equal(gateway.MIR2_CONTENT_PROFILE, "platinum_fiveclass"); assert.equal(gateway.MIR2_QUEST_CADENCE, "newcomer-v2");
  assert.equal(next.NEXT_PUBLIC_MIR2_QUEST_GUIDANCE, "newcomer-v2"); assert.equal(R16_GAMEPLAY_BASELINE.onboarding, "native");
  assert.equal(JSON.stringify(state), before);
  assert.equal(gateway.MIR2_ACCOUNT_STORE_PATH, playablePaths(root).accounts);
  assert.equal(gateway.MIR2_SAVE_RECOVERY_DIR, playablePaths(root).recovery);
});

test("wallet signing is persistent, world/domain separated and cannot be replaced by ambient public or dev settings", () => {
  const state = readyState(), before = JSON.stringify(state), first = playableWalletAuthEnvironment(state);
  assert.match(first.MIR2_PASSKEY_AUTH_SECRET, /^[a-f0-9]{64}$/);
  assert.ok(!Object.values(state.keys).includes(first.MIR2_PASSKEY_AUTH_SECRET));
  assert.deepEqual(playableWalletAuthEnvironment(JSON.parse(before)), first);
  assert.equal(JSON.stringify(state), before, "configuration never migrates or mutates the existing state");
  for (const field of ["realmId", "worldEpoch"]) {
    const another = structuredClone(state); another.world[field] += "-other";
    assert.notEqual(playableWalletAuthEnvironment(another).MIR2_PASSKEY_AUTH_SECRET, first.MIR2_PASSKEY_AUTH_SECRET);
  }
  const changedRoot = structuredClone(state); changedRoot.keys.identitySessionSecret = "ef".repeat(32);
  assert.notEqual(playableWalletAuthEnvironment(changedRoot).MIR2_PASSKEY_AUTH_SECRET, first.MIR2_PASSKEY_AUTH_SECRET);
  const next = playableNextEnvironment(state, { MIR2_PASSKEY_AUTH_SECRET: "public-inherited-value", MIR2_ALLOW_DEV_PASSKEY_SECRET: "1",
    MIR2_PASSKEY_ALLOWED_ORIGINS: "https://unrelated.example", NEXT_PUBLIC_MIR2_PASSKEY_AUTH_SECRET: "must-not-reach-browser" });
  assert.equal(next.MIR2_PASSKEY_AUTH_SECRET, first.MIR2_PASSKEY_AUTH_SECRET);
  assert.equal(next.MIR2_PASSKEY_ALLOWED_ORIGINS, "http://127.0.0.1:19041");
  assert.equal(next.MIR2_ALLOW_DEV_PASSKEY_SECRET, undefined);
  assert.equal(next.NEXT_PUBLIC_MIR2_PASSKEY_AUTH_SECRET, undefined);
  const broken = structuredClone(state); delete broken.keys.identitySessionSecret;
  assert.throws(() => playableWalletAuthEnvironment(broken));
  assert.throws(() => playableNextEnvironment(broken));
  const bootstrap = newPlayableState();
  assert.doesNotThrow(() => playableGatewayEnvironment(bootstrap, resolve(tmpdir(), "bootstrap-wallet")));
  assert.throws(() => playableNextEnvironment(bootstrap), "Web cannot start before bootstrap is complete");
});

test("first preparation publishes one initializing identity before any account/bootstrap work", async t => {
  const root = await temporary(t), first = await preparePlayableState(root), original = await readFile(playablePaths(root).state, "utf8");
  assert.equal(first.firstRun, true); assert.equal(first.state.phase, "initializing");
  assert.deepEqual(await readdir(root), ["state.local.json"]);
  await assert.rejects(preparePlayableState(root)); assert.equal(await readFile(playablePaths(root).state, "utf8"), original);
});

test("nonempty or corrupt state cannot produce replacement accounts, archives or keys", async t => {
  const root = await temporary(t); await writeFile(join(root, "accounts.json"), "old saved account bytes");
  await assert.rejects(preparePlayableState(root)); assert.deepEqual(await readdir(root), ["accounts.json"]);
  await writeFile(join(root, "state.local.json"), "{partial");
  await assert.rejects(preparePlayableState(root)); assert.equal(await readFile(join(root, "accounts.json"), "utf8"), "old saved account bytes");
  assert.equal(await readFile(join(root, "state.local.json"), "utf8"), "{partial");
});

test("complete ready state reloads without writing a byte or generating a new identity", async t => {
  const f = await readyFiles(t, { initialized: true });
  const paths = [f.paths.state, f.paths.accounts, f.paths.actorMemory, join(f.paths.supply, "supply-server.local.json")];
  const before = await Promise.all(paths.map(path => readFile(path)));
  const loaded = await preparePlayableState(f.root); assert.equal(loaded.firstRun, false); assert.deepEqual(loaded.state, f.state);
  assert.deepEqual(await Promise.all(paths.map(path => readFile(path))), before);
});

test("losing a supply key cannot recreate a spending ledger or reset its budget", async t => {
  const f = await readyFiles(t); await rm(join(f.paths.supply, "supply-server.local.json"));
  await writeFile(join(f.paths.supply, "supply-server.journal.json"), "existing signed spending history");
  await assert.rejects(preparePlayableState(f.root));
  assert.deepEqual(await readdir(f.paths.supply), ["supply-server.journal.json"]);
  assert.equal(await readFile(join(f.paths.supply, "supply-server.journal.json"), "utf8"), "existing signed spending history");
});

test("partially missing SQLite stores and state-publication remnants fail closed", async t => {
  const f = await readyFiles(t, { initialized: true }); await rm(f.paths.actorMemory + ".life.sqlite");
  await assert.rejects(preparePlayableState(f.root)); assert.equal(await readFile(f.paths.actorMemory, "utf8"), "synthetic SQLite preservation bytes");
  const second = await readyFiles(t); await writeFile(second.paths.state + ".pending-incomplete", "partial publication");
  await assert.rejects(preparePlayableState(second.root)); assert.equal(await readFile(second.paths.accounts, "utf8"), "synthetic existing account bytes");
});

test("private local control addresses are deterministic and scoped to the selected data directory", () => {
  const first = resolve(tmpdir(), "r16-one"), second = resolve(tmpdir(), "r16-two");
  assert.ok(playableControlAddress(first, "win32").startsWith("\\\\.\\pipe\\mir2-r16-web-"));
  assert.equal(playableControlAddress(first, "win32"), playableControlAddress(first, "win32"));
  assert.notEqual(playableControlAddress(first, "win32"), playableControlAddress(second, "win32"));
});

test("launcher source keeps EOF detached from shutdown and model loading confined to its Actor worker", async () => {
  const source = await readFile(new URL("./r16-playable-service.mjs", import.meta.url), "utf8");
  assert.match(source, /lines\.on\("close", \(\) => \{\}\)/);
  assert.doesNotMatch(source, /createTrialFixture|startLocalGateway|process\.kill\(/);
  const worker = source.slice(source.indexOf("async function runActorWorker"), source.indexOf("export async function runPlayableService"));
  const service = source.slice(source.indexOf("export async function runPlayableService"));
  assert.match(worker, /loadLocalModelSettings/); assert.doesNotMatch(service, /loadLocalModelSettings|OPENAI_API_KEY/);
  const stop = service.slice(service.indexOf("const stop ="), service.indexOf("const enqueue ="));
  assert.ok(stop.indexOf("await stopActor()") < stop.indexOf("await stopOwnedChild(next)"));
  assert.ok(stop.indexOf("await stopOwnedChild(next)") < stop.indexOf("await stopOwnedChild(gateway, true)"));
});
