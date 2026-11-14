import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import fs from "node:fs/promises";
import { registerHooks } from "node:module";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { FIXED_R16 } from "./lib/fixed-r16-assets.mjs";
import { assertNumeronFiniteProfile, LEGACY_PRODUCTION_ASSET_CONFIG, NUMERON_PRODUCTION_ASSET_CONFIG,
  loadProductionAssetConfig, productionAssetConfigName, resolveProductionAssetConfigPath,
  verifyProductionRemoteManifest } from "./lib/production-asset-config.mjs";
import { readAssetBaseUrl } from "./fetch-prebuilt-bevy-runtime.mjs";
import { readRemoteMapAtlasBuildPlan, readVerifiedRemoteMapAtlasRelease } from "./build-map-atlas-pack.mjs";
import { loadProductionFullPackConfig } from "./build-native-keyed-map-pack.mjs";

const execute = promisify(execFile), scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, ".."), projectRoot = path.resolve(webRoot, "../..");
const finitePath = resolveProductionAssetConfigPath(projectRoot, NUMERON_PRODUCTION_ASSET_CONFIG);
const legacyPath = resolveProductionAssetConfigPath(projectRoot, LEGACY_PRODUCTION_ASSET_CONFIG);
const digest = value => createHash("sha256").update(value).digest("hex");

async function withEnv(values, operation) {
  const previous = Object.fromEntries(Object.keys(values).map(name => [name, process.env[name]]));
  try { for (const [name, value] of Object.entries(values)) { if (value === undefined) delete process.env[name]; else process.env[name] = value; } return await operation(); }
  finally { for (const [name, value] of Object.entries(previous)) { if (value === undefined) delete process.env[name]; else process.env[name] = value; } }
}

test("the default keeps the existing full-pack/map-atlas profile and permits only two exact filenames", async () => {
  await withEnv({ MIR2_PRODUCTION_ASSET_CONFIG: undefined }, async () => {
    assert.equal(productionAssetConfigName(), LEGACY_PRODUCTION_ASSET_CONFIG);
    assert.equal(resolveProductionAssetConfigPath(), legacyPath);
    const old = loadProductionAssetConfig();
    assert.equal(old.version, "20260818-main-97887dcd");
    assert.equal(old.fullCrystalPack.enabled, true); assert.equal(old.fullCrystalPack.verified, true);
    assert.equal(old.mapAtlas.enabled, true); assert.equal(old.mapAtlas.verified, true);
    assert.equal((await readVerifiedRemoteMapAtlasRelease(legacyPath)).contentHash, old.mapAtlas.contentHash);
    assert.equal((await loadProductionFullPackConfig(legacyPath)).contentHash, old.fullCrystalPack.contentHash);
  });
  for (const name of ["../production-web-assets.json", "config/numeron-r16-web-assets.json", "C:/profile.json", "/tmp/profile.json",
    "numeron-r16-web-assets.json?other", "NUMERON-R16-WEB-ASSETS.JSON", "other.json", " production-web-assets.json", null]) {
    assert.throws(() => productionAssetConfigName(name), /filename only/);
  }
});

test("the Numeron profile binds the finite art manifest, six R16 renderer leaves and browser-safe fallbacks", async () => {
  const profile = loadProductionAssetConfig(projectRoot, NUMERON_PRODUCTION_ASSET_CONFIG);
  assertNumeronFiniteProfile(profile);
  assert.equal(profile.schemaVersion, 3); assert.equal(profile.objectPrefix, FIXED_R16.prefix);
  assert.equal(profile.assetBaseUrl, `${FIXED_R16.publicOrigin}/${FIXED_R16.prefix}`);
  assert.equal(profile.remoteReleaseSha256, FIXED_R16.manifestSha256);
  assert.equal(profile.finiteCoverage.payloadSha256, FIXED_R16.payloadSha256);
  assert.equal(profile.finiteCoverage.files, FIXED_R16.finiteFiles);
  assert.equal(profile.finiteCoverage.payloadFiles, FIXED_R16.fileCount);
  assert.equal(profile.browserFallbackBaseUrls[0], `${FIXED_R16.publicOrigin}/hotlink-ok/${FIXED_R16.prefix}`);
  for (const prefix of FIXED_R16.fallbackPrefixes) assert(profile.browserFallbackBaseUrls.some(base => base.endsWith(`/${prefix}`)));
  assert(profile.browserFallbackBaseUrls.every(base => !base.startsWith("https://mir2.obelisk.build/")));
  const runtimeBytes = await fs.readFile(path.join(webRoot, "lib/generated/bevy_runtime_version_numeron_r16.json"));
  assert.equal(digest(runtimeBytes), "36bdaa0116064c9b41cf8f54a547e53319011180e2275e1c5549e2ed7a33041b");
  const runtime = JSON.parse(runtimeBytes);
  assert.equal(runtime.version, profile.bevyRuntime.version);
  assert.equal(FIXED_R16.runtimeVersion, "bevy-a46905128c316656");
  assert.equal(runtime.files.length, 6);
  const runtimeBase = await readAssetBaseUrl(finitePath, runtime.version);
  assert.equal(runtimeBase, profile.assetBaseUrl);
  const urls = runtime.files.map(file => `${runtimeBase}/bevy-runtime/v/${runtime.version}/${file.path.replace(/^public\/bevy-runtime\//, "")}`);
  assert.equal(new Set(urls).size, 6);
  for (const directory of ["pkg-webgpu", "pkg-webgl2", "pkg-webgl2-shared"]) for (const name of ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"])
    assert(urls.includes(`${profile.assetBaseUrl}/bevy-runtime/v/${runtime.version}/${directory}/${name}`));
  await assert.rejects(readAssetBaseUrl(finitePath, "bevy-wrong"), /different immutable Bevy/);
  for (const change of [
    value => { value.assetBaseUrl += "/hotlink-ok"; }, value => { value.profile = "arbitrary-finite"; },
    value => { value.remoteReleaseSha256 = "0".repeat(64); }, value => { value.mapAtlas.verified = true; },
    value => { value.fullCrystalPack.enabled = true; }, value => { value.bevyRuntime.assetBaseUrl = "https://old.example.test"; },
    value => { value.bevyRuntime.version = FIXED_R16.runtimeVersion; },
  ]) { const changed = structuredClone(profile); change(changed); assert.throws(() => assertNumeronFiniteProfile(changed), /fixed release or disabled/); }
});

test("remote manifest bytes and identity are checked before using the finite inventory", async () => {
  const manifest = { version: "fixture", objectPrefix: "mir2/v/fixture", assetBaseUrl: "https://assets.example.test/mir2/v/fixture" };
  const bytes = Buffer.from(JSON.stringify(manifest));
  const config = { ...manifest, remoteReleaseUrl: `${manifest.assetBaseUrl}/remote-asset-release.json`, remoteReleaseSha256: digest(bytes) };
  const result = await verifyProductionRemoteManifest(config, async url => { assert.equal(url, config.remoteReleaseUrl); return new Response(bytes); });
  assert.equal(result.verified, true); assert.equal(result.bytes, bytes.length);
  await assert.rejects(verifyProductionRemoteManifest(config, async () => new Response("changed")), /SHA256 mismatch/);
  await assert.rejects(verifyProductionRemoteManifest({ ...config, version: "wrong" }, async () => new Response(bytes)), /identity mismatch/);
  await assert.rejects(verifyProductionRemoteManifest(config, async () => new Response("missing", { status: 404 })), /unavailable/);
  const old = loadProductionAssetConfig(projectRoot, LEGACY_PRODUCTION_ASSET_CONFIG);
  assert.equal((await verifyProductionRemoteManifest(old, async () => { throw Error("legacy must not add a fetch"); })).skipped, true);
});

test("finite Atlas is explicitly disabled, never passes the full atlas gate and cannot enable full-pack compilation", async () => {
  const finite = await readRemoteMapAtlasBuildPlan(finitePath);
  assert.deepEqual(finite, { enabled: false, verified: false, atlasBudgetVerified: false,
    releaseVersion: FIXED_R16.version, profile: "numeron-r16-finite" });
  await assert.rejects(readVerifiedRemoteMapAtlasRelease(finitePath), /not complete or verified/);
  await assert.rejects(loadProductionFullPackConfig(finitePath), /does not enable verified Full Crystal/);
  const { stdout } = await execute(process.execPath, [path.join(scriptDir, "build-map-atlas-pack.mjs")], {
    cwd: webRoot, env: { ...process.env, MIR2_PRODUCTION_ASSET_CONFIG: NUMERON_PRODUCTION_ASSET_CONFIG,
      MIR2_ORIGINAL_ASSET_MANIFEST_MODE: "remote-release" },
  });
  const result = JSON.parse(stdout); assert.equal(result.skipped, true); assert.equal(result.enabled, false);
  assert.equal(result.verified, false); assert.equal(result.atlasBudgetVerified, false);
  assert.match(result.reason, /no atlas build or budget acceptance/);
  const budget = await execute(process.execPath, ["--test", path.join(scriptDir, "test-map-atlas-budget.mjs")], {
    cwd: webRoot, env: { ...process.env, MIR2_PRODUCTION_ASSET_CONFIG: NUMERON_PRODUCTION_ASSET_CONFIG,
      MIR2_ORIGINAL_ASSET_MANIFEST_MODE: "remote-release", npm_lifecycle_event: "verify:map-atlas-budget", NODE_TEST_CONTEXT: undefined },
  });
  assert.match(budget.stdout, /# SKIP selected finite profile has no enabled atlas; atlas performance budget was not verified/);
  assert.match(budget.stdout, /# skipped 1\b/);
});

test("Next production exposes the selected profile while its unset default remains the original release", async () => {
  const helperUrl = new URL("../lib/standalone-full-pack-tracing.ts", import.meta.url);
  const hooks = registerHooks({ resolve(specifier, context, nextResolve) {
    if (specifier === "./lib/standalone-full-pack-tracing" && context.parentURL?.includes("next.config.ts")) return nextResolve(helperUrl.href, context);
    return nextResolve(specifier, context);
  } });
  try {
    for (const selection of [undefined, NUMERON_PRODUCTION_ASSET_CONFIG]) await withEnv({ VERCEL: "1", VERCEL_ENV: "production",
      MIR2_PRODUCTION_ASSET_CONFIG: selection }, async () => {
      const { default: next } = await import(new URL(`../next.config.ts?profile-test=${selection ?? "legacy"}`, import.meta.url));
      const expected = loadProductionAssetConfig(projectRoot, selection);
      assert.equal(next.env.MIR2_PINNED_ASSET_VERSION, expected.version);
      assert.equal(next.env.MIR2_PINNED_ASSET_BASE_URL, expected.assetBaseUrl);
      assert.deepEqual(JSON.parse(next.env.MIR2_PINNED_ASSET_BROWSER_FALLBACK_BASE_URLS), expected.browserFallbackBaseUrls);
      assert.equal(next.env.MIR2_PINNED_MAP_ATLAS_ENABLED, selection ? "" : "1");
      assert.equal(next.env.MIR2_PINNED_CRYSTAL_FULL_PACK_ENABLED, selection ? "" : "1");
    });
  } finally { hooks.deregister(); }
});

test("all script entry points reject path selection before work and shell uses the same selector", async () => {
  for (const script of ["build-map-atlas-pack.mjs", "build-native-keyed-map-pack.mjs", "fetch-prebuilt-bevy-runtime.mjs", "lib/production-asset-config.mjs"]) {
    await assert.rejects(execute(process.execPath, [path.join(scriptDir, script), ...(script.startsWith("lib/") ? ["--path"] : [])], {
      cwd: webRoot, env: { ...process.env, MIR2_PRODUCTION_ASSET_CONFIG: "../private.json" },
    }), error => { assert.match(error.stderr, /filename only/); return true; });
  }
  const shell = await fs.readFile(path.join(scriptDir, "vercel-build.sh"), "utf8");
  assert.match(shell, /PRODUCTION_ASSET_CONFIG="\$\(node \.\/scripts\/lib\/production-asset-config\.mjs --path\)"/);
  assert.match(shell, /production-asset-config\.mjs --verify-remote/);
  assert.doesNotMatch(shell, /PRODUCTION_ASSET_CONFIG="\$\(pwd\)\/\.\.\/\.\.\/config\/production-web-assets\.json"/);
  const { stdout } = await execute(process.execPath, [path.join(scriptDir, "lib/production-asset-config.mjs"), "--path"], {
    cwd: webRoot, env: { ...process.env, MIR2_PRODUCTION_ASSET_CONFIG: NUMERON_PRODUCTION_ASSET_CONFIG },
  });
  assert.equal(path.resolve(stdout.trim()), finitePath);
});

test("a selected config must be a regular file, never an environment-selected path or link", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-production-profile-"));
  try {
    await fs.mkdir(path.join(root, "config"));
    const target = path.join(root, "real.json"); await fs.copyFile(legacyPath, target);
    await fs.symlink(target, path.join(root, "config", LEGACY_PRODUCTION_ASSET_CONFIG), "file");
    assert.throws(() => resolveProductionAssetConfigPath(root, LEGACY_PRODUCTION_ASSET_CONFIG), /regular file/);
  } finally {
    const resolved = await fs.realpath(root), temp = await fs.realpath(os.tmpdir());
    assert.equal(path.dirname(resolved), temp); assert(path.basename(resolved).startsWith("mir2-production-profile-"));
    await fs.rm(resolved, { recursive: true, force: true });
  }
});
