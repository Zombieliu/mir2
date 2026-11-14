import test from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { access, mkdir, mkdtemp, readFile, rm, symlink, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { closeOwnedGatewayChild, gatewayExecutableArgument, isolatedEnvironment, R16_GAMEPLAY_BASELINE, resolveGatewayExecutable } from "./local-gateway.mjs";
import { assembleNativeFixture, nativeFixtureArguments, R16_RESOURCE_ANCHORS, resourceManifestEntries, verifyResourceEntry } from "./native-resource-input.mjs";
import { fieldApproachStep, homeReturnBudget } from "./field-encounter.mjs";

async function temporary(t) {
  const root = await mkdtemp(join(tmpdir(), "mir2-fixture-test-"));
  t.after(async () => {
    assert.equal(dirname(root), resolve(tmpdir()));
    assert.ok(basename(root).startsWith("mir2-fixture-test-"));
    await rm(root, { recursive: true, force: true });
  });
  return root;
}

test("gateway paths accept exact named/legacy arguments and reject duplicate or malformed options", () => {
  assert.equal(gatewayExecutableArgument([]), undefined);
  assert.equal(gatewayExecutableArgument(["--gateway-exe", "space folder/game.exe"]), resolve("space folder/game.exe"));
  assert.equal(gatewayExecutableArgument(["space folder/game.exe"]), resolve("space folder/game.exe"));
  for (const args of [["--gateway-exe"], ["--gateway-exe", ""], ["--gateway-exe", "--other"], ["--gateway-exe", "a", "--gateway-exe", "b"], ["--unsafe"]]) {
    assert.throws(() => gatewayExecutableArgument(args));
  }
});

test("binary discovery stays in canonical build directories; a missing explicit binary never falls back", async t => {
  const root = await temporary(t), suffix = process.platform === "win32" ? ".exe" : "";
  const legacy = join(root, "target-ai-player/debug", `mir2-gateway${suffix}`);
  const canonical = join(root, "target/debug", `mir2-gateway${suffix}`);
  await mkdir(dirname(legacy), { recursive: true }); await writeFile(legacy, "legacy-input");
  assert.equal(await resolveGatewayExecutable(undefined, { root }), legacy);
  await mkdir(dirname(canonical), { recursive: true }); await writeFile(canonical, "canonical-input");
  assert.equal(await resolveGatewayExecutable(undefined, { root }), canonical);
  assert.equal(await resolveGatewayExecutable(legacy, { root }), legacy);
  await assert.rejects(resolveGatewayExecutable(join(root, "missing.exe"), { root }));
});

test("local child environment omits inherited production/database/admin/model settings", () => {
  const keys = ["MIR2_ACCOUNT_STORE_DATABASE_URL", "DATABASE_URL", "MIR2_COMPANION_POLICY_PATH", "MIR2_SHARED_PARTY_ENABLED", "MIR2_AI_LIVE_ENABLED"];
  const before = keys.map(key => process.env[key]);
  try {
    keys.forEach(key => { process.env[key] = "fixture-only-inherited-value"; });
    const result = isolatedEnvironment({ MIR2_RUNTIME_ENV: "test", MIR2_AI_LIVE_ENABLED: "0" });
    keys.filter(key => key !== "MIR2_AI_LIVE_ENABLED").forEach(key => assert.equal(Object.hasOwn(result, key), false));
    assert.equal(result.MIR2_AI_LIVE_ENABLED, "0"); assert.equal(result.MIR2_RUNTIME_ENV, "test");
  } finally { keys.forEach((key, index) => { if (before[index] === undefined) delete process.env[key]; else process.env[key] = before[index]; }); }
});

test("native fixture exposes explicit binary/resource paths and opt-in ordinary login only", () => {
  const parsed = nativeFixtureArguments(["--native-exe", "fresh/game.exe", "--gateway-exe", "fresh/gateway.exe", "--candidate-root", "signed R16", "--release-control-root", "control", "--native-output-root", "fresh/output", "--launch-native"]);
  assert.equal(parsed.nativeExecutable, resolve("fresh/game.exe"));
  assert.equal(parsed.gatewayExecutable, resolve("fresh/gateway.exe"));
  assert.equal(parsed.candidateRoot, resolve("signed R16")); assert.equal(parsed.launchNative, true);
  assert.equal(parsed.nativeOutputRoot, resolve("fresh/output"));
  assert.equal(nativeFixtureArguments([]).launchNative, false);
  for (const args of [["--native-exe"], ["--native-exe", ""], ["--native-exe", "a", "--native-exe", "b"], ["--launch-native", "--launch-native"], ["--skip-signature"], ["--generation-root", "anything"]]) {
    assert.throws(() => nativeFixtureArguments(args));
  }
});

function resourceManifest() {
  const names = ["crystal-map-pack/0.map.gz", "bevy-entity-atlases/manifest.json", "generated/map-atlas/manifest.json", "generated/native-map-keyed/manifest.json",
    "original-effects/effects.generated.json", "original-ui/Items/meta.json", "original-ui/Items/0.png", "original-ui/Items/3792.png", "original-ui/StateItem/meta.json",
    "original-ui/StateItem/30.png", "original-ui/StateItem/5152.png", "original-ui/Prguse2/meta.json", ...[1202,1203,1204,1205].map(n=>`original-ui/Prguse2/${n}.png`),
    ...["Default","Normal_Atk","Compulsion_Atk","Npc"].map(name=>`original-ui/Cursors/Cursor_${name}.png`)];
  const files = names.map(path => ({ path: `mir2-assets/${path}`, size: 1, sha256: "a".repeat(64) }));
  return { schema: "mir2.windows.package-manifest.v4", fileCount: files.length, files };
}

test("trusted R16 metadata anchors are exact SHA-256 values", () => {
  assert.match(R16_RESOURCE_ANCHORS.receiptSha256, /^[a-f\d]{64}$/);
  assert.match(R16_RESOURCE_ANCHORS.versionSha256, /^[a-f\d]{64}$/);
  assert.equal(R16_RESOURCE_ANCHORS.candidate, "WN-CANDIDATE-20261101-unified-16");
  assert.equal(R16_GAMEPLAY_BASELINE.contentProfile, "platinum_fiveclass");
  assert.equal(R16_GAMEPLAY_BASELINE.profileVersion, 1);
  assert.match(R16_GAMEPLAY_BASELINE.generationManifestSha256, /^[a-f\d]{64}$/);
});

test("resource closure rejects path traversal, drive paths, aliases and missing startup resources", () => {
  assert.equal(resourceManifestEntries(resourceManifest()).length, 20);
  for (const path of ["../outside", "mir2-assets/../outside", "C:/outside", "/outside", "mir2-assets\\outside", "mir2-assets//outside", "mir2-assets/./outside"]) {
    const value = resourceManifest(); value.files[0].path = path; assert.throws(() => resourceManifestEntries(value));
  }
  const duplicate = resourceManifest(); duplicate.files[1].path = duplicate.files[0].path.toUpperCase(); assert.throws(() => resourceManifestEntries(duplicate));
  const missing = resourceManifest(); missing.files.shift(); missing.fileCount -= 1; assert.throws(() => resourceManifestEntries(missing));
  const count = resourceManifest(); count.fileCount += 1; assert.throws(() => resourceManifestEntries(count));
});

test("resource validation detects same-size corruption and rejects out-of-root reads", async t => {
  const root = await temporary(t), path = "asset.bin", original = Buffer.from("original");
  await writeFile(join(root, path), original);
  const entry = { path, size: original.length, sha256: createHash("sha256").update(original).digest("hex") };
  await verifyResourceEntry(root, entry);
  await writeFile(join(root, path), "modified");
  await assert.rejects(verifyResourceEntry(root, entry));
  await assert.rejects(verifyResourceEntry(root, { ...entry, path: "../outside" }));
});

test("assembly refuses unverified resources and refuses overlap with the original R16 package", async t => {
  const root = await temporary(t), exe = join(root, "input.exe"); await writeFile(exe, "fixture-only-no-execution");
  await assert.rejects(assembleNativeFixture({ directory: join(root, "new"), nativeExecutable: exe, resources: {} }));
  const resources = { root, assetRoot: join(root, "mir2-assets"), cmsVerified: true, entries: [{}], verifiedResourceCount: 1 };
  await assert.rejects(assembleNativeFixture({ directory: join(root, "overwrite"), nativeExecutable: exe, resources }));
});

test("a junction cannot turn the new fixture destination into an R16 source write", async t => {
  const root = await temporary(t), source = join(root, "original"), alias = join(root, "alias"), exe = join(root, "input.exe");
  await mkdir(source); await writeFile(exe, "fixture-only-no-execution");
  await symlink(source, alias, process.platform === "win32" ? "junction" : "dir");
  const resources = { root: source, assetRoot: join(source, "mir2-assets"), cmsVerified: true, entries: [{}], verifiedResourceCount: 1 };
  await assert.rejects(assembleNativeFixture({ directory: join(alias, "new"), nativeExecutable: exe, resources }));
  await assert.rejects(access(join(source, "new")), { code: "ENOENT" });
});

test("an existing destination is never merged or overwritten", async t => {
  const root = await temporary(t), source = join(root, "original"), destination = join(root, "existing"), exe = join(root, "input.exe");
  await mkdir(source); await mkdir(destination); await writeFile(exe, "fixture-only-no-execution");
  await writeFile(join(destination, "mir2-platform-windows.exe"), "existing-user-input");
  const resources = { root: source, assetRoot: join(source, "mir2-assets"), cmsVerified: true, entries: [{}], verifiedResourceCount: 1 };
  await assert.rejects(assembleNativeFixture({ directory: destination, nativeExecutable: exe, resources }), { code: "EEXIST" });
  assert.equal(await readFile(join(destination, "mir2-platform-windows.exe"), "utf8"), "existing-user-input");
});

async function ownedProcess(t, script, stdin = "pipe") {
  const child = spawn(process.execPath, ["-e", script], { windowsHide: true, stdio: [stdin, "pipe", "pipe"], env: isolatedEnvironment({}) });
  let output = "";
  child.stdout.on("data", data => { output += data; }); child.stderr.resume();
  t.after(async () => { await closeOwnedGatewayChild(child, { timeoutMs: 100, killWaitMs: 1500 }); });
  await once(child.stdout, "data");
  return { child, output: () => output };
}

test("owned gateway shutdown writes the real stdin line and concurrent close is idempotent", async t => {
  const { child, output } = await ownedProcess(t, "let value='';process.stdin.on('data',b=>{value+=b;if(value.includes('shutdown\\n')){process.stdout.write('settled\\n',()=>process.exit(0))}});process.stdout.write('ready\\n');");
  const first = closeOwnedGatewayChild(child, { timeoutMs: 1500, killWaitMs: 1000 });
  assert.equal(closeOwnedGatewayChild(child), first);
  await first;
  assert.equal(child.exitCode, 0); assert.equal(child.signalCode, null); assert.match(output(), /settled/);
  await closeOwnedGatewayChild(child);
});

test("an owned child with no stdin closes within the fallback bound", async t => {
  const { child } = await ownedProcess(t, "process.stdout.write('ready\\n');setInterval(()=>{},1000)", "ignore");
  const startedAt = Date.now();
  await closeOwnedGatewayChild(child, { timeoutMs: 100, killWaitMs: 1500 });
  assert.ok(Date.now() - startedAt < 2500);
  assert.ok(child.exitCode !== null || child.signalCode !== null);
  await closeOwnedGatewayChild(child);
});

test("an unresponsive owned child gets the graceful window before the bounded fallback", async t => {
  const { child } = await ownedProcess(t, "process.stdin.resume();process.stdout.write('ready\\n');setInterval(()=>{},1000)");
  const startedAt = Date.now();
  await closeOwnedGatewayChild(child, { timeoutMs: 100, killWaitMs: 1500 });
  const elapsed = Date.now() - startedAt;
  assert.ok(elapsed >= 80 && elapsed < 2500, `Bounded fallback elapsed ${elapsed}ms`);
});

test("already exited children and absent handles are safe to close repeatedly", async () => {
  const child = spawn(process.execPath, ["-e", "process.exit(0)"], { windowsHide: true, stdio: "ignore", env: isolatedEnvironment({}) });
  await once(child, "exit");
  await closeOwnedGatewayChild(child); await closeOwnedGatewayChild(child); await closeOwnedGatewayChild(undefined);
  await assert.rejects(closeOwnedGatewayChild(child, { timeoutMs: 60_000 }));
});

test("field approach skips a walkable but isolated nearest destination", () => {
  const collision = { mapFileName: "0", width: 8, height: 8,
    blocked: new Set(["3,1", "4,1", "5,1", "3,2", "5,2", "3,3", "4,3"]) };
  const result = fieldApproachStep({ player: { x: 0, y: 3 }, monster: { x: 5, y: 3 }, home: { x: 0, y: 0 }, safeRadius: 0,
    collision, occupied: [{ x: 5, y: 3 }] });
  assert.ok(result);
  assert.notDeepEqual(result.tile, { x: 4, y: 2 });
  assert.equal(Math.max(Math.abs(result.tile.x-5), Math.abs(result.tile.y-3)), 1);
  assert.ok(!collision.blocked.has(`${result.tile.x},${result.tile.y}`));
});

test("field setup does not turn a safe or occupied destination into a legal path", () => {
  const collision = { mapFileName: "0", width: 8, height: 8, blocked: new Set() };
  assert.equal(fieldApproachStep({ player: { x: 0, y: 3 }, monster: { x: 5, y: 3 }, home: { x: 5, y: 3 }, safeRadius: 1, collision }), null);
  const occupied = [[4,2], [5,2], [6,2], [4,3], [6,3], [4,4], [5,4], [6,4]].map(([x,y])=>({x,y}));
  assert.equal(fieldApproachStep({ player: { x: 0, y: 3 }, monster: { x: 5, y: 3 }, home: { x: 0, y: 0 }, safeRadius: 0, collision, occupied }), null);
});

test("retreat travel budget accounts for a real collision detour and has a fixed occupancy allowance", () => {
  const collision = { mapFileName: "0", width: 8, height: 8,
    blocked: new Set(Array.from({ length: 7 }, (_, x) => `${x},3`)) };
  const budget = homeReturnBudget({ player: { x: 0, y: 2 }, home: { x: 0, y: 4 }, collision });
  assert.deepEqual(budget, { pathSteps: 16, occupancyAllowance: 16, maxTicks: 32 });
});

test("retreat fixture rejects unreachable or overlong paths instead of waiting indefinitely", () => {
  const closed = { mapFileName: "0", width: 8, height: 8,
    blocked: new Set(Array.from({ length: 8 }, (_, x) => `${x},3`)) };
  assert.equal(homeReturnBudget({ player: { x: 0, y: 2 }, home: { x: 0, y: 4 }, collision: closed }), null);
  assert.equal(homeReturnBudget({ player: { x: 0, y: 0 }, home: { x: 99, y: 0 },
    collision: { mapFileName: "0", width: 100, height: 1, blocked: new Set() } }), null);
});
