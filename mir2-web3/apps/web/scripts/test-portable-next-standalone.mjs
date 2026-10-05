import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import { test } from "node:test";
import {
  auditPortableTree, classifyPortableStandalonePath, copyPortableStandalone,
  copySelectedPublic, copyVerifiedRequiredServerFiles, materializeStandaloneDependencies,
  selectThinPublicEntry, selectMapAtlasClosure, compactJsonWhitespace, collectSharpNativePackages,
} from "./lib/portable-next-standalone.mjs";

const fixtureParent = process.env.MIR2_PORTABLE_TEST_ROOT ?? os.tmpdir();
await fs.mkdir(fixtureParent, { recursive: true });
const fixtureRoot = await fs.mkdtemp(path.join(fixtureParent, "portable-next-fixture-"));
const runtimeVersion = "bevy-0123456789abcdef";
const runtimeFile = `bevy-runtime-releases/${runtimeVersion}/pkg-webgl2/mir2_bevy_runtime.js`;
const coreVersion = "a".repeat(64);
const coreFiles = new Set(["mir2_platform_web.js", "mir2_platform_web_bg.wasm"].map(
  (name) => `client-core/${coreVersion}/${name}`));
const selectPublic = (relative, directory) => selectThinPublicEntry(relative, directory, {
  runtimeVersion, runtimeFiles: new Set([runtimeFile, `bevy-runtime-releases/${runtimeVersion}/runtime-manifest.json`]),
  coreVersion, coreFiles,
});
const write = async (file, contents = "fixture") => {
  await fs.mkdir(path.dirname(file), { recursive: true });
  await fs.writeFile(file, contents);
};
const exists = async (file) => fs.lstat(file).then(() => true, (error) => {
  if (error?.code === "ENOENT") return false;
  throw error;
});
const nativePlatform = `${process.platform}-${process.arch}`;
async function installSharpFixture(dependency) {
  const binaryName = `@img/sharp-${nativePlatform}`;
  const companionName = `@img/sharp-libvips-${nativePlatform}`;
  await write(path.join(dependency, "sharp", "package.json"), JSON.stringify({ name: "sharp", version: "0.34.5",
    main: "index.js", optionalDependencies: { [binaryName]: "0.34.5", [companionName]: "1.2.4" } }));
  await write(path.join(dependency, "sharp", "index.js"), "module.exports = {};");
  await write(path.join(dependency, "detect-libc", "package.json"), JSON.stringify({ name: "detect-libc", main: "index.js" }));
  await write(path.join(dependency, "detect-libc", "index.js"),
    "module.exports = { isNonGlibcLinuxSync: () => false, familySync: () => '' };");
  const binaryRoot = path.join(dependency, ...binaryName.split("/"));
  await write(path.join(binaryRoot, "package.json"), JSON.stringify({ name: binaryName, version: "0.34.5",
    exports: { "./sharp.node": `./lib/sharp-${nativePlatform}.node`, "./versions": "./versions.json" } }));
  await write(path.join(binaryRoot, "LICENSE"));
  await write(path.join(binaryRoot, "versions.json"), '{"sharp":"0.34.5"}');
  await write(path.join(binaryRoot, "lib", `sharp-${nativePlatform}.node`), Buffer.from([1, 2, 3]));
  if (process.platform === "win32") await write(path.join(binaryRoot, "lib", "libvips.dll"), Buffer.from([4]));
  const companionRoot = path.join(dependency, ...companionName.split("/"));
  await write(path.join(companionRoot, "package.json"), JSON.stringify({ name: companionName, version: "1.2.4" }));
  await write(path.join(companionRoot, "LICENSE"));
  await write(path.join(companionRoot, "lib", "libvips.dat"), Buffer.from([5]));
  return { binaryName, companionName, binaryRoot, companionRoot };
}

test("selection cuts only audited remote media and exact external roots, preserving local metadata", () => {
  const keep = [
    "original-effects/boom/1.png", "original-ui/fonts/NotoSansCJK.otf",
    "original-ui/UI_32bit/472.png", "original-ui/Items/meta.json",
    "original-ui/Prguse/2092.png", "generated/map-atlas/a.png",
    "generated/original-map-blend/a.png", "bevy-entity-atlases/a.png",
    "pwa/icon.png", "bootstrap/scene.json", ...coreFiles, runtimeFile,
  ];
  for (const candidate of keep) assert.equal(selectPublic(candidate, false), true, candidate);
  const skip = [
    "original-ui/Items/2.png", "original-ui/Prguse/2091.png",
    "original-ui/Help/1.png", "original-ui/DNItems/1.png", "original-ui/StateItem/1.png",
    "original-ui/MagIcon/1.png", "original-ui/MagIcon2/1.png", "original-ui/BuffIcon/1.png",
    "original-ui/GuildSkill/1.png", "original-ui/Mount/1.png",
    "original-ui/Pet/1.png", "original-ui/Gate/1.png",
    "generated/native-map-keyed/a.png", "generated/original-map/a.png",
    "original-effects/.env.local", "original-ui/fonts/private.key",
    `bevy-runtime-releases/bevy-fedcba9876543210/pkg-webgl2/mir2_bevy_runtime.js`,
    "bevy-runtime/pkg-webgl2/mir2_bevy_runtime.js",
    "client-core/runtime.js", `client-core/${"b".repeat(64)}/mir2_platform_web.js`,
    `client-core/${coreVersion}/extra.bin`,
  ];
  for (const candidate of skip) assert.equal(selectPublic(candidate, false), false, candidate);
  for (const candidate of ["original-ui/Help/meta.json", "original-ui/BuffIcon/meta.json",
    "original-ui/Mount/meta.json", "original-ui/Pet/meta.json", "original-ui/Gate/meta.json",
    "original-ui/Mount/LICENSE.txt", "original-ui/Pet/text.woff2", "original-ui/gdi-text/glyph.png"]) {
    assert.equal(selectPublic(candidate, false), true, candidate);
  }
  const app = "mir2-web3/apps/web";
  assert.deepEqual(classifyPortableStandalonePath("mir2-web3\\apps\\web\\node_modules", app, true, selectPublic),
    { copy: false, reason: "external dependency junction" });
  assert.equal(classifyPortableStandalonePath("mir2-web3\\apps\\web\\public\\generated\\crystal-packs\\full", app,
    true, selectPublic).copy, false);
  assert.equal(classifyPortableStandalonePath("mir2-web3/apps/web/public/original-effects/a.png", app,
    false, selectPublic).copy, true);
  for (const candidate of ["../escape", "C:\\outside", "/outside", "a//b"]) {
    assert.throws(() => selectPublic(candidate, false), /Unsafe portable path/);
  }
});

test("standalone selection precedes copying linked public and dependency trees", async (t) => {
  const base = path.join(fixtureRoot, "selected-copy");
  const source = path.join(base, "source");
  const destination = path.join(base, "package");
  const app = path.join(source, "mir2-web3", "apps", "web");
  await write(path.join(app, "server.js"), "server");
  await write(path.join(app, ".next-custom", "server", "app", "page.js"), "page");
  await write(path.join(app, "public", "original-effects", "effect.png"));
  await write(path.join(app, "public", "original-ui", "Items", "meta.json"), '{ "frames": [] }\n');
  await write(path.join(app, "public", "original-ui", "Items", "large.png"));
  const external = path.join(base, "external");
  await write(path.join(external, "private.bin"));
  try {
    await fs.symlink(external, path.join(app, "node_modules"), process.platform === "win32" ? "junction" : "dir");
    await fs.mkdir(path.join(app, "public", "generated", "crystal-packs"), { recursive: true });
    await fs.symlink(external, path.join(app, "public", "generated", "crystal-packs", "full"),
      process.platform === "win32" ? "junction" : "dir");
  } catch (error) {
    if (error?.code === "EPERM" || error?.code === "EACCES") {
      t.skip(`OS denied junction/symlink fixture: ${error.code}`);
      return;
    }
    throw error;
  }
  const copied = await copyPortableStandalone({ sourceRoot: source, destinationRoot: destination,
    appRelativePath: "mir2-web3/apps/web", publicSelection: selectPublic });
  const packagedApp = path.join(destination, "mir2-web3", "apps", "web");
  assert.equal(copied.files, 4);
  assert.equal(await exists(path.join(packagedApp, "node_modules")), false);
  assert.equal(await exists(path.join(packagedApp, "public", "generated", "crystal-packs")), false);
  assert.equal(await exists(path.join(packagedApp, "public", "original-ui", "Items", "large.png")), false);
  assert.equal(await exists(path.join(packagedApp, "public", "original-ui", "Items", "meta.json")), true);
  assert.equal((await auditPortableTree(destination)).links, 0);
  assert.equal(await exists(path.join(app, "public", "original-ui", "Items", "large.png")), true);
  await assert.rejects(copyPortableStandalone({ sourceRoot: source, destinationRoot: destination,
    appRelativePath: "mir2-web3/apps/web", publicSelection: selectPublic }), /overwrite/);
  await assert.rejects(copyPortableStandalone({ sourceRoot: source, destinationRoot: path.join(source, "nested"),
    appRelativePath: "mir2-web3/apps/web", publicSelection: selectPublic }), /overlap/);
});

test("source public walk preserves selected metadata, assets, and pinned immutable release", async () => {
  const base = path.join(fixtureRoot, "public-walk");
  const source = path.join(base, "source");
  const destination = path.join(base, "output");
  const retain = ["original-effects/e.png", "original-ui/fonts/f.otf", "original-ui/Items/meta.json",
    "original-ui/Prguse/2092.png", "generated/map-atlas/a.png", runtimeFile,
    `bevy-runtime-releases/${runtimeVersion}/runtime-manifest.json`];
  for (const file of [...retain, "original-ui/Items/2.png", "generated/native-map-keyed/a.png"])
    await write(path.join(source, ...file.split("/")), file.endsWith(".json") ? '{ "path": "a b" }\n' : file);
  assert.equal((await copySelectedPublic({ sourceRoot: source, destinationRoot: destination,
    selection: selectPublic })).files, retain.length);
  for (const file of retain) assert.equal(await exists(path.join(destination, ...file.split("/"))), true, file);
  assert.equal(await exists(path.join(destination, "original-ui", "Items", "2.png")), false);
  assert.equal(await exists(path.join(destination, "generated", "native-map-keyed")), false);
  assert.equal((await auditPortableTree(destination)).links, 0);
});

test("absent standalone required-server-files is copied byte-for-byte and conflicting metadata rejected", async () => {
  const base = path.join(fixtureRoot, "required-server-files");
  const source = path.join(base, "source", "required-server-files.json");
  const destination = path.join(base, "app", ".next-custom", "required-server-files.json");
  const bytes = Buffer.from(JSON.stringify({ config: { distDir: ".next-custom", env: {
    MIR2_BEVY_RUNTIME_BUILD_MANIFEST: "full-original-identity",
  } } }));
  await write(source, bytes);
  assert.equal(await exists(destination), false);
  assert.deepEqual(await copyVerifiedRequiredServerFiles({ sourcePath: source, destinationPath: destination,
    sourceBytes: bytes }), JSON.parse(bytes.toString()));
  assert.deepEqual(await fs.readFile(destination), bytes);
  await copyVerifiedRequiredServerFiles({ sourcePath: source, destinationPath: destination, sourceBytes: bytes });
  await fs.writeFile(destination, "{}");
  await assert.rejects(copyVerifiedRequiredServerFiles({ sourcePath: source, destinationPath: destination,
    sourceBytes: bytes }), /differs/);
});

test("NFT dependency copy materializes only the verified physical closure, including native files", async () => {
  const base = path.join(fixtureRoot, "dependency-copy");
  const standalone = path.join(base, "source", "standalone");
  const sourceApp = path.join(standalone, "mir2-web3", "apps", "web");
  const destinationApp = path.join(base, "package", "mir2-web3", "apps", "web");
  const dependency = path.join(base, "deps");
  await write(path.join(sourceApp, "server.js"), "require('next');");
  await write(path.join(sourceApp, ".next-custom", "server", "app", "api", "route.js"),
    "require('react');");
  await write(path.join(destinationApp, "server.js"), "require('next');");
  for (const pkg of ["next", "react", "react-dom"]) {
    await write(path.join(dependency, pkg, "package.json"), JSON.stringify({ name: pkg, main: "index.js" }));
    await write(path.join(dependency, pkg, "index.js"), "module.exports = {};");
  }
  await write(path.join(dependency, "next", "dist", "server", "lib", "start-server.js"),
    "module.exports = {};");
  await write(path.join(dependency, "next", "native", "binding.node"), Buffer.from([1, 2, 3]));
  const nativeFixture = await installSharpFixture(dependency);
  await fs.symlink(dependency, path.join(sourceApp, "node_modules"), process.platform === "win32" ? "junction" : "dir");
  const nodeFiles = ["next/index.js", "next/dist/server/lib/start-server.js", "next/native/binding.node",
    "react/index.js", "react-dom/index.js"];
  let observedEntries = [];
  const stats = await materializeStandaloneDependencies({
    sourceAppRoot: sourceApp, destinationAppRoot: destinationApp,
    sourceStandaloneRoot: standalone, sourceDependencyRoot: path.join(sourceApp, "node_modules"),
    distDir: ".next-custom",
    trace: async (entries) => {
      observedEntries = entries;
      return { fileList: new Set(nodeFiles.map((file) => path.relative(path.parse(dependency).root,
        path.join(dependency, ...file.split("/"))))), warnings: new Set([{ code: "DYNAMIC_OPTIONAL" }]) };
    },
  });
  assert.equal(observedEntries.some((entry) => entry.endsWith("route.js")), true);
  assert.equal(observedEntries.some((entry) => entry.endsWith("server.js")), true);
  assert.equal(stats.nativeBindings, 2);
  assert.equal(stats.targetPlatform, nativePlatform);
  assert.equal(stats.nativePackages.length, 2);
  assert.equal(stats.nativePackageFiles >= 8, true);
  assert.equal(stats.nativePackageBytes > 0, true);
  assert.equal(stats.nativePackages[0].destination, `node_modules/${nativeFixture.binaryName}`);
  assert.equal(stats.warningCodes.DYNAMIC_OPTIONAL, 1);
  assert.equal(stats.files >= 17, true);
  const copied = path.join(destinationApp, "node_modules");
  assert.equal((await auditPortableTree(copied)).links, 0);
  for (const pkg of ["next", "react", "react-dom"]) {
    const resolved = createRequire(path.join(destinationApp, "server.js")).resolve(pkg);
    assert.equal(resolved.startsWith(copied), true);
  }
  assert.deepEqual(await fs.readFile(path.join(copied, "next", "native", "binding.node")),
    Buffer.from([1, 2, 3]));
  for (const relative of ["package.json", "LICENSE", "versions.json",
    `lib/sharp-${nativePlatform}.node`]) {
    assert.equal(await exists(path.join(copied, ...nativeFixture.binaryName.split("/"), relative)), true);
  }
  assert.equal(await exists(path.join(copied, ...nativeFixture.companionName.split("/"), "LICENSE")), true);
  if (process.platform === "win32") {
    assert.equal(await exists(path.join(copied, ...nativeFixture.binaryName.split("/"), "lib/libvips.dll")), true);
  }
});

test("installed Sharp target package is a full physical native closure", async () => {
  const sourceApp = path.resolve(import.meta.dirname,
    "../.next-web-input-routing/standalone/mir2-web3/apps/web");
  const root = await fs.realpath(path.join(sourceApp, "node_modules"));
  const closure = await collectSharpNativePackages(root);
  assert.equal(closure.targetPlatform, nativePlatform);
  assert.equal(closure.packages.some((pkg) => pkg.name === `@img/sharp-${nativePlatform}`), true);
  assert.equal(closure.packages.reduce((sum, pkg) => sum + pkg.files, 0), closure.files.size);
  assert.equal(closure.packages.reduce((sum, pkg) => sum + pkg.bytes, 0) > 0, true);
  if (process.platform === "win32") {
    assert.equal(closure.files.has(path.join("@img", `sharp-${nativePlatform}`, "lib", "libvips-42.dll")), true);
  }
});

test("Sharp native closure fails missing binary, missing metadata and linked entries", async () => {
  const dependency = path.join(fixtureRoot, "sharp-negative", "deps");
  const fixture = await installSharpFixture(dependency);
  const assertFixtureDirectory = (candidate) => {
    const relative = path.relative(fixtureRoot, path.resolve(candidate));
    assert.equal(relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) &&
      !path.isAbsolute(relative), true, candidate);
  };
  const binary = path.join(fixture.binaryRoot, "lib", `sharp-${nativePlatform}.node`);
  const backup = `${binary}.backup`;
  await fs.rename(binary, backup);
  await assert.rejects(collectSharpNativePackages(dependency), /binary or version metadata missing/);
  await fs.rename(backup, binary);
  const versions = path.join(fixture.binaryRoot, "versions.json");
  await fs.rename(versions, `${versions}.backup`);
  await assert.rejects(collectSharpNativePackages(dependency), /binary or version metadata missing/);
  await fs.rename(`${versions}.backup`, versions);
  assertFixtureDirectory(fixture.companionRoot);
  assertFixtureDirectory(`${fixture.companionRoot}.backup`);
  await fs.rename(fixture.companionRoot, `${fixture.companionRoot}.backup`);
  await assert.rejects(collectSharpNativePackages(dependency), /Missing target Sharp native package/);
  await fs.rename(`${fixture.companionRoot}.backup`, fixture.companionRoot);
  await fs.rename(binary, backup);
  try {
    await fs.symlink(backup, binary, "file");
    await assert.rejects(collectSharpNativePackages(dependency), /Linked Sharp native entry/);
  } catch (error) {
    if (error?.code !== "EPERM" && error?.code !== "EACCES") throw error;
  }
  const escape = path.join(fixtureRoot, "sharp-escape", "deps");
  const escapedFixture = await installSharpFixture(escape);
  const external = path.join(fixtureRoot, "external-native-package");
  assertFixtureDirectory(escapedFixture.binaryRoot);
  assertFixtureDirectory(external);
  await fs.rename(escapedFixture.binaryRoot, external);
  try {
    await fs.symlink(external, escapedFixture.binaryRoot, process.platform === "win32" ? "junction" : "dir");
    await assert.rejects(collectSharpNativePackages(escape), /not a real directory/);
  } catch (error) {
    if (error?.code !== "EPERM" && error?.code !== "EACCES") throw error;
  }
  const missing = path.join(fixtureRoot, "sharp-missing", "deps");
  await write(path.join(missing, "sharp", "package.json"), JSON.stringify({ name: "sharp",
    optionalDependencies: { [fixture.binaryName]: "0.34.5" } }));
  await write(path.join(missing, "@img", "placeholder"));
  await assert.rejects(collectSharpNativePackages(missing), /Missing target Sharp native package/);
});

test("NFT missing listed file is fatal, without quietly treating it as an optional warning", async () => {
  const base = path.join(fixtureRoot, "missing-dependency");
  const standalone = path.join(base, "standalone");
  const sourceApp = path.join(standalone, "apps", "web");
  const dependency = path.join(base, "deps");
  await write(path.join(sourceApp, "server.js"));
  await write(path.join(sourceApp, ".next", "server", "app", "page.js"));
  for (const pkg of ["next", "react", "react-dom"]) {
    await write(path.join(dependency, pkg, "package.json"), JSON.stringify({ name: pkg, main: "index.js" }));
    await write(path.join(dependency, pkg, "index.js"));
  }
  await write(path.join(dependency, "next", "dist", "server", "lib", "start-server.js"));
  await installSharpFixture(dependency);
  await fs.symlink(dependency, path.join(sourceApp, "node_modules"), process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(materializeStandaloneDependencies({ sourceAppRoot: sourceApp,
    destinationAppRoot: path.join(base, "package", "apps", "web"), sourceStandaloneRoot: standalone,
    sourceDependencyRoot: path.join(sourceApp, "node_modules"), distDir: ".next",
    trace: async () => ({ fileList: new Set([path.relative(path.parse(base).root,
      path.join(dependency, "next", "missing.js"))]), warnings: new Set([{ code: "OPTIONAL" }]) }),
  }), /missing dependency/);
});

test("installed NFT walks an isolated standalone fixture through the guarded callbacks", async () => {
  const base = path.join(fixtureRoot, "installed-nft");
  const standalone = path.join(base, "standalone");
  const sourceApp = path.join(standalone, "apps", "web");
  const destinationApp = path.join(base, "package", "apps", "web");
  const dependency = path.join(base, "deps");
  await write(path.join(sourceApp, "server.js"),
    "require('next'); require('next/dist/server/lib/start-server');");
  await write(path.join(sourceApp, ".next", "server", "app", "page.js"),
    "require('react'); require('react-dom');");
  await write(path.join(destinationApp, "server.js"), "require('next');");
  for (const pkg of ["next", "react", "react-dom"]) {
    await write(path.join(dependency, pkg, "package.json"), JSON.stringify({ name: pkg, main: "index.js" }));
    await write(path.join(dependency, pkg, "index.js"), "module.exports = {};");
  }
  await write(path.join(dependency, "next", "dist", "server", "lib", "start-server.js"),
    "module.exports = {};");
  await installSharpFixture(dependency);
  await fs.symlink(dependency, path.join(sourceApp, "node_modules"), process.platform === "win32" ? "junction" : "dir");
  const { nodeFileTrace } = createRequire(import.meta.url)("next/dist/compiled/@vercel/nft");
  const stats = await materializeStandaloneDependencies({ sourceAppRoot: sourceApp,
    destinationAppRoot: destinationApp, sourceStandaloneRoot: standalone,
    sourceDependencyRoot: path.join(sourceApp, "node_modules"), distDir: ".next", trace: nodeFileTrace });
  assert.equal(stats.entries, 7);
  assert.equal(stats.files >= 16, true);
  assert.equal((await auditPortableTree(path.join(destinationApp, "node_modules"))).links, 0);
});

test("actual current atlas closure selects only declared pages and validates their regular paths", async () => {
  const publicRoot = path.resolve(import.meta.dirname, "../public");
  const closure = await selectMapAtlasClosure(publicRoot);
  const manifest = JSON.parse(await fs.readFile(path.join(publicRoot, "generated/map-atlas/manifest.json"), "utf8"));
  assert.equal(closure.pages, manifest.pages.length);
  assert.equal(closure.files.has("generated/map-atlas/manifest.json"), true);
  for (const page of manifest.pages) assert.equal(closure.files.has(page.u.slice(1)), true);
  const historic = (await fs.readdir(path.join(publicRoot, "generated/map-atlas")))
    .find((name) => name.startsWith("manifest.") && name.endsWith(".json"));
  assert.equal(closure.files.has(`generated/map-atlas/${historic}`), false);
  const selected = (relative, isDirectory) => selectThinPublicEntry(relative, isDirectory, {
    runtimeVersion, runtimeFiles: new Set([runtimeFile]), mapAtlasFiles: closure.files,
  });
  assert.equal(selected("generated/map-atlas", true), true);
  assert.equal(selected(`generated/map-atlas/${historic}`, false), false);
});

test("pinned atlas keeps exact manifest/page union; missing, duplicate, escape and changed pins fail", async () => {
  const root = path.join(fixtureRoot, "atlas-pin");
  const pageA = "generated/map-atlas/tiles/p0.abc.png";
  const pageB = "generated/map-atlas/tiles/p1.def.png";
  const manifest = (pages) => JSON.stringify({ schemaVersion: 2, kind: "mir2-map-atlas-manifest", pages });
  const record = (u, p) => ({ l: "tiles", p, u: `/${u}` });
  await write(path.join(root, pageA));
  await write(path.join(root, pageB));
  await write(path.join(root, "generated/map-atlas/manifest.json"), manifest([record(pageA, 0)]));
  const pinBytes = manifest([record(pageB, 1)]);
  const hash = createHash("sha256").update(pinBytes).digest("hex");
  const pin = `generated/map-atlas/manifest.${hash}.json`;
  await write(path.join(root, pin), pinBytes);
  const closure = await selectMapAtlasClosure(root, {
    pinnedEnabled: true, pinnedManifestPath: `/${pin}`, pinnedContentHash: hash,
  });
  assert.deepEqual([...closure.files].sort(), ["generated/map-atlas/manifest.json", pin, pageA, pageB].sort());
  const historic = "generated/map-atlas/tiles/historic.png";
  await write(path.join(root, historic));
  const destination = path.join(fixtureRoot, "atlas-pin-copy");
  const selected = (relative, isDirectory) => selectThinPublicEntry(relative, isDirectory, {
    runtimeVersion, runtimeFiles: new Set([runtimeFile]), mapAtlasFiles: closure.files,
  });
  await copySelectedPublic({ sourceRoot: root, destinationRoot: destination, selection: selected });
  assert.deepEqual(await fs.readFile(path.join(destination, "generated/map-atlas/manifest.json")),
    await fs.readFile(path.join(root, "generated/map-atlas/manifest.json")));
  assert.deepEqual(await fs.readFile(path.join(destination, pin)), Buffer.from(pinBytes));
  assert.equal(await exists(path.join(destination, pageA)), true);
  assert.equal(await exists(path.join(destination, pageB)), true);
  assert.equal(await exists(path.join(destination, historic)), false);
  await assert.rejects(selectMapAtlasClosure(root, { pinnedEnabled: true }), /incomplete/);
  await assert.rejects(selectMapAtlasClosure(root, { pinnedManifestPath: `/${pin}`, pinnedContentHash: "0".repeat(64) }), /inconsistent/);
  await fs.writeFile(path.join(root, pin), "{}");
  await assert.rejects(selectMapAtlasClosure(root, { pinnedManifestPath: `/${pin}`, pinnedContentHash: hash }), /bytes do not match/);
  await fs.writeFile(path.join(root, pin), pinBytes);
  await fs.writeFile(path.join(root, "generated/map-atlas/manifest.json"), manifest([record(pageA, 0), record(pageA, 1)]));
  await assert.rejects(selectMapAtlasClosure(root), /Duplicate/);
  await fs.writeFile(path.join(root, "generated/map-atlas/manifest.json"), manifest([record(pageA, 0), record(pageB, 0)]));
  await assert.rejects(selectMapAtlasClosure(root), /Duplicate/);
  await fs.writeFile(path.join(root, "generated/map-atlas/manifest.json"), manifest([record("generated/map-atlas/../escape.png", 0)]));
  await assert.rejects(selectMapAtlasClosure(root), /Unsafe/);
  await fs.writeFile(path.join(root, "generated/map-atlas/manifest.json"), manifest([record("generated/map-atlas/missing.png", 0)]));
  await assert.rejects(selectMapAtlasClosure(root), /ENOENT/);
  const linked = "generated/map-atlas/linked.png";
  try {
    await fs.symlink(path.join(root, pageA), path.join(root, linked), "file");
    await fs.writeFile(path.join(root, "generated/map-atlas/manifest.json"), manifest([record(linked, 0)]));
    await assert.rejects(selectMapAtlasClosure(root), /not a regular path/);
  } catch (error) {
    if (error?.code !== "EPERM" && error?.code !== "EACCES") throw error;
  }
});

test("JSON whitespace compaction preserves every string, escape, numeric token and key order", async () => {
  const raw = String.raw`{ "z" : 1e+03 , "a" : " space \\n \\u0041 \\\" " , "n" : -0.00 , "nested" : [ true , null ] }`;
  const compact = compactJsonWhitespace(Buffer.from(raw));
  assert.equal(compact.toString(), String.raw`{"z":1e+03,"a":" space \\n \\u0041 \\\" ","n":-0.00,"nested":[true,null]}`);
  assert.deepEqual(JSON.parse(compact.toString()), JSON.parse(raw));
  await assert.rejects(async () => compactJsonWhitespace('{"broken": }'), SyntaxError);
});

test("all actual original-ui JSON can be compacted without changing parsed values", async () => {
  const root = path.resolve(import.meta.dirname, "../public/original-ui");
  let files = 0;
  let sourceBytes = 0;
  let compactBytes = 0;
  async function visit(dir) {
    for (const entry of await fs.readdir(dir, { withFileTypes: true })) {
      const candidate = path.join(dir, entry.name);
      if (entry.isDirectory()) await visit(candidate);
      else if (entry.isFile() && entry.name.endsWith(".json")) {
        const input = await fs.readFile(candidate);
        const output = compactJsonWhitespace(input);
        assert.deepEqual(JSON.parse(output.toString("utf8")), JSON.parse(input.toString("utf8")));
        files += 1;
        sourceBytes += input.length;
        compactBytes += output.length;
      }
    }
  }
  await visit(root);
  assert.equal(files > 200, true);
  assert.equal(compactBytes < sourceBytes, true);
});

test("actual root asset manifest compacts without changing assetHash or source bytes", async () => {
  const sourcePath = path.resolve(import.meta.dirname, "../public/original-asset-manifest.generated.json");
  const source = await fs.readFile(sourcePath);
  const sourceHash = createHash("sha256").update(source).digest("hex");
  const output = compactJsonWhitespace(source);
  const before = JSON.parse(source.toString("utf8"));
  const after = JSON.parse(output.toString("utf8"));
  assert.equal(output.length < source.length, true);
  assert.equal(after.assetHash, before.assetHash);
  assert.deepEqual(after, before);
  assert.equal(createHash("sha256").update(await fs.readFile(sourcePath)).digest("hex"), sourceHash);
});

test("two selected public sources must agree and retain effects, fonts, GDI and three local fallbacks", async () => {
  const base = path.join(fixtureRoot, "two-public-sources");
  const standalone = path.join(base, "standalone");
  const app = path.join(standalone, "apps", "web");
  const source = path.join(base, "source-public");
  const output = path.join(base, "output");
  const rootManifest = "original-asset-manifest.generated.json";
  const manifestSource = String.raw`{ "assetHash" : "a b\\u0041" , "n" : 1e+03 , "ordered" : [ true , null ] }` + "\n";
  const entries = [rootManifest, "original-effects/a.png", "original-ui/fonts/font.otf", "original-ui/fonts/LICENSE.txt",
    "original-ui/gdi-text/glyph.png", "original-ui/Items/meta.json",
    "original-ui/Mount/meta.json", "original-ui/Pet/meta.json", "original-ui/Gate/meta.json",
    "generated/original-map-blend/meta.json",
    "original-ui/Prguse/2092.png", "original-ui/Prguse/2094.png", "original-ui/Prguse/2095.png"];
  for (const relative of entries) {
    const contents = relative === rootManifest ? manifestSource :
      relative.endsWith(".json") ? '{ "n": 1e+03, "s": "a b" }\n' : relative;
    await write(path.join(app, "public", relative), contents);
    await write(path.join(source, relative), contents);
  }
  for (const remote of ["Mount", "Pet", "Gate"]) {
    await write(path.join(app, "public/original-ui", remote, "media.png"), remote);
    await write(path.join(source, "original-ui", remote, "media.png"), remote);
  }
  await write(path.join(app, "server.js"));
  const copied = await copyPortableStandalone({ sourceRoot: standalone, destinationRoot: output,
    appRelativePath: "apps/web", publicSelection: selectPublic });
  const selected = await copySelectedPublic({ sourceRoot: source, destinationRoot: path.join(output, "apps/web/public"),
    selection: selectPublic });
  assert.equal(selected.collisions, entries.length);
  assert.equal(copied.jsonCompactions.length, 5);
  assert.equal(selected.jsonCompactions.length, 5);
  for (const relative of entries) assert.equal(await exists(path.join(output, "apps/web/public", relative)), true);
  for (const remote of ["Mount", "Pet", "Gate"]) {
    assert.equal(await exists(path.join(output, "apps/web/public/original-ui", remote, "media.png")), false);
    assert.equal(await exists(path.join(source, "original-ui", remote, "media.png")), true);
  }
  assert.equal((await auditPortableTree(output)).links, 0);
  assert.equal(copied.bytes, (await auditPortableTree(output)).bytes);
  assert.equal(copied.sourceBytes - copied.bytes,
    copied.jsonCompactions.reduce((sum, row) => sum + row.sourceBytes - row.outputBytes, 0));
  const compactManifest = await fs.readFile(path.join(output, "apps/web/public", rootManifest), "utf8");
  assert.equal(compactManifest, String.raw`{"assetHash":"a b\\u0041","n":1e+03,"ordered":[true,null]}`);
  assert.deepEqual(JSON.parse(compactManifest), JSON.parse(manifestSource));
  assert.equal(await fs.readFile(path.join(source, rootManifest), "utf8"), manifestSource);
  assert.equal(await fs.readFile(path.join(app, "public", rootManifest), "utf8"), manifestSource);
  assert.equal(await fs.readFile(path.join(output, "apps/web/public/generated/original-map-blend/meta.json"), "utf8"),
    '{ "n": 1e+03, "s": "a b" }\n');
  assert.equal((await fs.readFile(path.join(output, "apps/web/public/original-ui/Items/meta.json"), "utf8")),
    '{"n":1e+03,"s":"a b"}');
  await fs.writeFile(path.join(source, rootManifest), manifestSource.replace("1e+03", "1000"));
  await assert.rejects(copySelectedPublic({ sourceRoot: source,
    destinationRoot: path.join(output, "apps/web/public"), selection: selectPublic }), /collision differs/);
  await fs.writeFile(path.join(source, rootManifest), manifestSource);
  await fs.writeFile(path.join(source, "original-ui/Prguse/2092.png"), "different");
  await assert.rejects(copySelectedPublic({ sourceRoot: source,
    destinationRoot: path.join(output, "apps/web/public"), selection: selectPublic }), /collision differs/);
  assert.equal(await exists(path.join(source, "original-ui/Prguse/2092.png")), true);
});

test("exact scene cache and copied NFT metadata are excluded without hiding dist/server/static", () => {
  const app = "apps/web";
  for (const relative of [".next/cache/mir2-scene-blueprints/a.json", ".next-web/server/app/page.js.nft.json"]) {
    assert.equal(classifyPortableStandalonePath(`${app}/${relative}`, app, false, selectPublic).copy, false);
  }
  for (const relative of [".next-web/server/app/page.js", ".next-web/static/chunks/app/page.js",
    "lib/generated/map.json", ".next/cache/other/a.json"]) {
    assert.equal(classifyPortableStandalonePath(`${app}/${relative}`, app, false, selectPublic).copy, true);
  }
});

console.log(`portable fixture retained at ${fixtureRoot}`);
