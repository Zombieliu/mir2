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
  selectThinPublicEntry, selectMapAtlasClosure, selectLocalMapClosure, compactJsonWhitespace, collectSharpNativePackages,
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
        path.join(dependency, ...file.split("/"))))), warnings: new Set([
          { code: "DYNAMIC_OPTIONAL", message: "Synthetic optional branch" },
          new Error(`Failed to resolve dependency "missing-package" from ${sourceApp}`),
          Object.assign(new Error("x".repeat(8193)), { code: "__proto__" }),
        ]) };
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
  assert.equal(stats.warningCount, 3);
  assert.equal(stats.warningCodes.Error, 1);
  assert.equal(Object.hasOwn(stats.warningCodes, "__proto__"), true);
  assert.equal(stats.warningCodes.__proto__, 1);
  assert.deepEqual(stats.warningDetails, [
    { code: "DYNAMIC_OPTIONAL", message: "Synthetic optional branch", messageTruncated: false },
    { code: "Error", message: `Failed to resolve dependency "missing-package" from ${sourceApp}`, messageTruncated: false },
    { code: "__proto__", message: "x".repeat(8192), messageTruncated: true },
  ]);
  assert.deepEqual(JSON.parse(JSON.stringify(stats)).warningDetails, stats.warningDetails);
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
  const distDirectory = process.env.MIR2_NEXT_DIST_DIR ?? ".next-web-input-routing";
  assert.match(distDirectory, /^[A-Za-z0-9._-]+$/, "configured Next dist is a safe single directory");
  assert.notEqual(distDirectory, ".");
  assert.notEqual(distDirectory, "..");
  const sourceApp = path.resolve(import.meta.dirname, "..", distDirectory, "standalone/mir2-web3/apps/web");
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
    .find((name) => name !== "manifest.json" && name.startsWith("manifest.") && name.endsWith(".json"));
  assert.ok(historic, "an actual historical manifest exists");
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
  const atlasManifest = "bevy-entity-atlases/manifest.json";
  const atlasSource = String.raw`{
    "version" : 1, "assetHash" : "atlas hash \u0041",
    "u64" : 18446744073709551615, "i64" : -9223372036854775808,
    "scale" : 1e+03, "signed" : -0.00,
    "pages" : [ { "path" : "pages/a b.png", "sha256" : "0123456789abcdef" } ],
    "escaped" : " quote \" slash \\ newline \n "
  }` + "\n";
  const compactAtlas = String.raw`{"version":1,"assetHash":"atlas hash \u0041","u64":18446744073709551615,"i64":-9223372036854775808,"scale":1e+03,"signed":-0.00,"pages":[{"path":"pages/a b.png","sha256":"0123456789abcdef"}],"escaped":" quote \" slash \\ newline \n "}`;
  const atlasImage = Buffer.from([0, 255, 13, 10, 32, 34, 92, 128]);
  const unchangedAtlasJson = '{ "n": 18446744073709551615, "s": "a b" }\r\n';
  const entries = [rootManifest, "original-effects/a.png", "original-ui/fonts/font.otf", "original-ui/fonts/LICENSE.txt",
    "original-ui/gdi-text/glyph.png", "original-ui/Items/meta.json",
    "original-ui/Mount/meta.json", "original-ui/Pet/meta.json", "original-ui/Gate/meta.json",
    "generated/original-map-blend/meta.json", atlasManifest, "bevy-entity-atlases/pages/a b.png",
    "bevy-entity-atlases/metadata.json",
    "original-ui/Prguse/2092.png", "original-ui/Prguse/2094.png", "original-ui/Prguse/2095.png"];
  for (const relative of entries) {
    const contents = relative === rootManifest ? manifestSource : relative === atlasManifest ? atlasSource :
      relative === "bevy-entity-atlases/pages/a b.png" ? atlasImage :
      relative === "bevy-entity-atlases/metadata.json" ? unchangedAtlasJson :
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
  assert.equal(copied.jsonCompactions.length, 6);
  assert.equal(selected.jsonCompactions.length, 6);
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
  assert.equal(await fs.readFile(path.join(output, "apps/web/public", atlasManifest), "utf8"), compactAtlas);
  assert.deepEqual(JSON.parse(compactAtlas), JSON.parse(atlasSource));
  for (const originalRoot of [source, path.join(app, "public")]) {
    assert.equal(await fs.readFile(path.join(originalRoot, atlasManifest), "utf8"), atlasSource);
    assert.deepEqual(await fs.readFile(path.join(originalRoot, "bevy-entity-atlases/pages/a b.png")), atlasImage);
    assert.equal(await fs.readFile(path.join(originalRoot, "bevy-entity-atlases/metadata.json"), "utf8"), unchangedAtlasJson);
  }
  assert.deepEqual(await fs.readFile(path.join(output, "apps/web/public/bevy-entity-atlases/pages/a b.png")), atlasImage);
  assert.equal(await fs.readFile(path.join(output, "apps/web/public/bevy-entity-atlases/metadata.json"), "utf8"), unchangedAtlasJson);
  const sha256 = (value) => createHash("sha256").update(value).digest("hex");
  for (const result of [copied, selected]) {
    assert.deepEqual(result.jsonCompactions.find((row) => row.path === atlasManifest), {
      path: atlasManifest,
      sourceBytes: Buffer.byteLength(atlasSource), outputBytes: Buffer.byteLength(compactAtlas),
      sourceSha256: sha256(atlasSource), outputSha256: sha256(compactAtlas),
    });
    assert.equal(result.jsonCompactions.some((row) => row.path === "bevy-entity-atlases/metadata.json"), false);
  }
  assert.equal(await fs.readFile(path.join(output, "apps/web/public/generated/original-map-blend/meta.json"), "utf8"),
    '{ "n": 1e+03, "s": "a b" }\n');
  assert.equal((await fs.readFile(path.join(output, "apps/web/public/original-ui/Items/meta.json"), "utf8")),
    '{"n":1e+03,"s":"a b"}');
  await fs.writeFile(path.join(source, rootManifest), manifestSource.replace("1e+03", "1000"));
  await assert.rejects(copySelectedPublic({ sourceRoot: source,
    destinationRoot: path.join(output, "apps/web/public"), selection: selectPublic }), /collision differs/);
  await fs.writeFile(path.join(source, rootManifest), manifestSource);
  // Whitespace-only source changes collide safely; equivalent numeric values with different tokens do not.
  await fs.writeFile(path.join(source, atlasManifest), "\t" + atlasSource + "\r\n");
  const whitespaceCollision = await copySelectedPublic({ sourceRoot: source,
    destinationRoot: path.join(output, "apps/web/public"), selection: selectPublic });
  assert.equal(whitespaceCollision.collisions, entries.length);
  assert.equal(whitespaceCollision.jsonCompactions.length, 6);
  assert.equal(await fs.readFile(path.join(output, "apps/web/public", atlasManifest), "utf8"), compactAtlas);
  await fs.writeFile(path.join(source, atlasManifest), atlasSource.replace("1e+03", "1000"));
  await assert.rejects(copySelectedPublic({ sourceRoot: source,
    destinationRoot: path.join(output, "apps/web/public"), selection: selectPublic }), /collision differs/);
  assert.equal(await fs.readFile(path.join(output, "apps/web/public", atlasManifest), "utf8"), compactAtlas);
  await fs.writeFile(path.join(source, atlasManifest), atlasSource);
  assert.equal(await fs.readFile(path.join(app, "public", atlasManifest), "utf8"), atlasSource);
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


// Local map delivery is opt-in and bounded by the pinned finite request. These
// fixtures exercise actual custody/selection/copy helpers, not a renderer.
const localMapFrames69 = {
  "WemadeMir2/Objects2": [2704,2705,2706,2707,2708,2709,2710,2711,2712,2713,2716,2717,2718,2719,2720,2721,2722,2723,2724,2725,2728,2729,2730,2731,2732,2733,2734,2735,2736,2737,2740,2741,2742,2743,2744,2745,2746,2747,2748,2749,2752,2753,2754,2755,2757,2758,2759,2760,2761,2764,2765,2766,2767,2768,2769,2770,2771,2772,2773,2776,2777,2778,2779,2780,2781,2782,2783,2785,2788,2789,2790,2791,2792,2793,2794,2795,2796,2797,2800,2801,2802,2803,2804,2805,2806,2807,2808,2809,4301,4302,4303,4304,4305,4306,4313,4314,4315,4316,4382,4383,4384,4385,4386,4387,4388],
  "WemadeMir2/Objects3": [3981,3982,3983,3984,3985,3986,3987,3988,3989,8005,8006],
};
async function localMapFixture69(name, wholeWarehouse = false) {
  const base = path.join(fixtureRoot, name), root = path.join(base, "public"), inputPath = path.join(base, "finite.json");
  const missingAssets = Object.entries(localMapFrames69).flatMap(([libraryKey, frames]) => frames.map(frameIndex => ({
    libraryKey, frameIndex, publicPath: `/original-map/${libraryKey}/${frameIndex}.png`,
  })));
  const rows = wholeWarehouse ? missingAssets : [missingAssets[0], missingAssets.at(-1)];
  const assets = {}, bodies = new Map();
  for (const row of rows) {
    const body = Buffer.from("finite map bytes:" + row.publicPath);
    await write(path.join(root, row.publicPath.slice(1)), body);
    bodies.set(row.publicPath, body);
    assets[row.publicPath] = { source: "original-map", size: body.length,
      sha256: createHash("sha256").update(body).digest("hex") };
  }
  const oldPath = "/original-map/WemadeMir2/Objects2/2703.png", oldBody = Buffer.from("old remote map");
  assets[oldPath] = { source: "original-map", size: oldBody.length,
    sha256: createHash("sha256").update(oldBody).digest("hex") };
  assets["/original-ui/Items/1.png"] = { source: "original-ui", size: 8, sha256: "a".repeat(64) };
  await write(path.join(root, oldPath.slice(1)), oldBody);
  const manifest = { schemaVersion: 1, kind: "mir2-original-asset-manifest", assetHash: "b".repeat(64), assets };
  const manifestPath = path.join(root, "original-asset-manifest.generated.json");
  const request = { mapFileName: "0140.map", missingAssets: rows };
  async function pin(value = request) {
    const bytes = JSON.stringify(value);
    await write(inputPath, bytes);
    return { sourcePath: inputPath, sourceSha256: createHash("sha256").update(bytes).digest("hex") };
  }
  const options = await pin();
  const manifestBytes = JSON.stringify(manifest, null, 2) + "\n";
  await write(manifestPath, manifestBytes);
  return { base, root, inputPath, request, manifest, manifestPath, manifestBytes, bodies, oldPath, options, pin };
}

test("local map closure leaves every old default selection unchanged and admits only exact ancestors", async () => {
  const absent = await selectLocalMapClosure(path.join(fixtureRoot, "no-map-request"));
  assert.equal(absent.files.size, 0); assert.deepEqual(absent.frames, []); assert.equal(absent.bytes, 0);
  const f = await localMapFixture69("local-map-selection");
  const closure = await selectLocalMapClosure(f.root, f.options);
  const selected = (relative, directory) => selectThinPublicEntry(relative, directory, {
    runtimeVersion, runtimeFiles: new Set([runtimeFile]), coreVersion, coreFiles, localMapFiles: closure.files,
  });
  for (const row of f.request.missingAssets) {
    assert.equal(selectPublic(row.publicPath.slice(1), false), false);
    assert.equal(selected(row.publicPath.slice(1), false), true);
    const parts = row.publicPath.slice(1).split("/");
    for (let depth = 1; depth < parts.length; depth++) assert.equal(selected(parts.slice(0, depth).join("/"), true), true);
  }
  for (const relative of [f.oldPath.slice(1), "original-map/WemadeMir2/Objects2/meta.json",
    "original-map/WemadeMir2/Objects3/8005.png", "original-map/WemadeMir2/Objects27/1.png"]) {
    assert.equal(selected(relative, false), false);
  }
  for (const relative of ["original-map/ShandaMir2", "original-map/WemadeMir2/Objects27"]) {
    assert.equal(selected(relative, true), false);
  }
  for (const [relative, directory] of [["original-map", true], ["original-map/WemadeMir2", true],
    ["original-map/WemadeMir2/Objects2", true], [f.oldPath.slice(1), false]]) assert.equal(selectPublic(relative, directory), false);
  for (const [relative, directory] of [
    [runtimeFile,false],[[...coreFiles][0],false],["original-ui/Items/1.png",false],
    ["original-ui/Items/meta.json",false],["generated/map-atlas/old.png",false],["pwa",true]]) {
    assert.equal(selected(relative, directory), selectPublic(relative, directory), relative);
  }
});

test("finite Warehouse request copies precisely 116 verified PNG leaves and retains the full manifest", async () => {
  const f = await localMapFixture69("local-map-warehouse-copy", true);
  const closure = await selectLocalMapClosure(f.root, f.options);
  assert.equal(closure.files.size, 116);
  assert.deepEqual([...closure.files].sort(), f.request.missingAssets.map(row => row.publicPath.slice(1)).sort());
  assert.equal(closure.sourceSha256, f.options.sourceSha256);
  const selected = (relative, directory) => selectThinPublicEntry(relative, directory, {
    runtimeVersion, runtimeFiles: new Set([runtimeFile]), localMapFiles: closure.files,
  });
  const destination = path.join(f.base, "package");
  const result = await copySelectedPublic({ sourceRoot: f.root, destinationRoot: destination, selection: selected });
  assert.equal(result.files, 117); // 116 exact leaves and the complete root inventory.
  assert.equal(await exists(path.join(destination, f.oldPath.slice(1))), false);
  const copied = await selectLocalMapClosure(destination, f.options);
  assert.deepEqual(copied.frames, closure.frames);
  assert.equal(copied.bytes, [...f.bodies.values()].reduce((sum, body) => sum + body.length, 0));
  const copiedManifest = JSON.parse(await fs.readFile(path.join(destination, "original-asset-manifest.generated.json"), "utf8"));
  assert.deepEqual(copiedManifest, f.manifest);
  assert.equal(Object.hasOwn(copiedManifest.assets, f.oldPath), true);
  assert.equal(Object.hasOwn(copiedManifest.assets, "/original-ui/Items/1.png"), true);
  assert.equal((await auditPortableTree(destination)).links, 0);
  for (const [publicPath, body] of f.bodies) {
    assert.deepEqual(await fs.readFile(path.join(destination, publicPath.slice(1))), body);
    assert.deepEqual(await fs.readFile(path.join(f.root, publicPath.slice(1))), body);
  }
  assert.equal(await fs.readFile(f.manifestPath, "utf8"), f.manifestBytes);
  await fs.writeFile(path.join(destination, f.request.missingAssets[0].publicPath.slice(1)), "copy changed");
  await assert.rejects(selectLocalMapClosure(destination, f.options), /size\/SHA256/);
});

test("local map custody rejects missing bytes, changed bytes and invalid full-manifest size/hash records", async () => {
  const f = await localMapFixture69("local-map-records"), row = f.request.missingAssets[0];
  const file = path.join(f.root, row.publicPath.slice(1)), body = f.bodies.get(row.publicPath);
  await fs.rename(file, file + ".retained");
  await assert.rejects(selectLocalMapClosure(f.root, f.options), /ENOENT/);
  await fs.rename(file + ".retained", file);
  const changed = Buffer.from(body); changed[0] ^= 1;
  await fs.writeFile(file, changed);
  await assert.rejects(selectLocalMapClosure(f.root, f.options), /size\/SHA256/);
  await fs.writeFile(file, Buffer.concat([body, Buffer.from([0])]));
  await assert.rejects(selectLocalMapClosure(f.root, f.options), /size\/SHA256/);
  await fs.writeFile(file, body);
  for (const change of [
    entry => { entry.size += 1; }, entry => { entry.sha256 = "0".repeat(64); },
    entry => { entry.size = 0; }, entry => { entry.size = 0.5; },
    entry => { entry.sha256 = "invalid"; }, entry => { delete entry.sha256; },
    entry => { entry.source = "original-ui"; },
  ]) {
    const manifest = structuredClone(f.manifest); change(manifest.assets[row.publicPath]);
    await fs.writeFile(f.manifestPath, JSON.stringify(manifest));
    await assert.rejects(selectLocalMapClosure(f.root, f.options), /manifest/);
  }
  const missing = structuredClone(f.manifest); delete missing.assets[row.publicPath];
  await fs.writeFile(f.manifestPath, JSON.stringify(missing));
  await assert.rejects(selectLocalMapClosure(f.root, f.options), /manifest/);
  await fs.writeFile(f.manifestPath, '{"schemaVersion":1,"kind":"wrong","assets":{}}');
  await assert.rejects(selectLocalMapClosure(f.root, f.options), /manifest/);
  await fs.writeFile(f.manifestPath, f.manifestBytes);
  assert.equal((await selectLocalMapClosure(f.root, f.options)).files.size, 2);
});

test("local map request pin rejects incomplete input, duplicate/escaped paths and inconsistent frame identity", async () => {
  const f = await localMapFixture69("local-map-request"), row = f.request.missingAssets[0];
  await assert.rejects(selectLocalMapClosure(f.root, { sourcePath: f.inputPath }), /requires/);
  await assert.rejects(selectLocalMapClosure(f.root, { sourceSha256: f.options.sourceSha256 }), /requires/);
  await assert.rejects(selectLocalMapClosure(f.root, { ...f.options, sourcePath: "finite.json" }), /requires/);
  await assert.rejects(selectLocalMapClosure(f.root, { ...f.options, sourceSha256: "0".repeat(64) }), /pin/);
  await fs.writeFile(f.inputPath, '{}');
  await assert.rejects(selectLocalMapClosure(f.root, f.options), /pin/);
  const requests = [
    { missingAssets: [] }, { missingAssets: new Array(117).fill(row) },
    { missingAssets: [row, row] },
    { missingAssets: [{ ...row, publicPath: "/original-map/../outside.png" }] },
    { missingAssets: [{ ...row, libraryKey: "../outside", publicPath: "/original-map/../outside/2704.png" }] },
    { missingAssets: [{ ...row, publicPath: "/original-ui/Items/2704.png" }] },
    { missingAssets: [{ ...row, publicPath: row.publicPath.replace("2704", "2705") }] },
    { missingAssets: [{ ...row, frameIndex: -1 }] },
    { missingAssets: [{ ...row, frameIndex: 0.5 }] },
  ];
  for (const request of requests) await assert.rejects(selectLocalMapClosure(f.root, await f.pin(request)),
    /explicit frames|Duplicate|Unsafe/);
  const malformed = "{";
  await fs.writeFile(f.inputPath, malformed);
  await assert.rejects(selectLocalMapClosure(f.root, { sourcePath: f.inputPath,
    sourceSha256: createHash("sha256").update(malformed).digest("hex") }), SyntaxError);
  await assert.rejects(selectLocalMapClosure("relative-public", await f.pin()), /absolute real directory/);
});

test("local map custody rejects file, ancestor, manifest, input and public-root symlinks", async () => {
  const f = await localMapFixture69("local-map-links"), row = f.request.missingAssets[0];
  const file = path.join(f.root, row.publicPath.slice(1)), outside = path.join(f.base, "outside.png");
  await write(outside, f.bodies.get(row.publicPath));
  await fs.rename(file, file + ".retained");
  await fs.symlink(outside, file, "file");
  await assert.rejects(selectLocalMapClosure(f.root, f.options), /not a regular path/);
  const manifestFixture = await localMapFixture69("local-map-manifest-link");
  const retainedManifest = manifestFixture.manifestPath + ".retained";
  await fs.rename(manifestFixture.manifestPath, retainedManifest);
  await fs.symlink(retainedManifest, manifestFixture.manifestPath, "file");
  await assert.rejects(selectLocalMapClosure(manifestFixture.root, manifestFixture.options), /not a regular path/);
  const inputFixture = await localMapFixture69("local-map-input-link");
  await fs.rename(inputFixture.inputPath, inputFixture.inputPath + ".retained");
  await fs.symlink(inputFixture.inputPath + ".retained", inputFixture.inputPath, "file");
  await assert.rejects(selectLocalMapClosure(inputFixture.root, inputFixture.options), /not a regular path/);
  const ancestorFixture = await localMapFixture69("local-map-ancestor-link");
  const directory = path.join(ancestorFixture.root, "original-map/WemadeMir2/Objects2");
  await fs.rename(directory, directory + "-retained");
  await fs.symlink(directory + "-retained", directory, process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(selectLocalMapClosure(ancestorFixture.root, ancestorFixture.options), /not a regular path/);
  const rootFixture = await localMapFixture69("local-map-root-link"), rootLink = path.join(rootFixture.base, "linked-public");
  await fs.symlink(rootFixture.root, rootLink, process.platform === "win32" ? "junction" : "dir");
  await assert.rejects(selectLocalMapClosure(rootLink, rootFixture.options), /absolute real directory/);
});

console.log(`portable fixture retained at ${fixtureRoot}`);

async function warehouseSkinReference73() {
  const sourcePath = path.resolve(import.meta.dirname,
    "../../game-client/client-bevy/src/crystal_ui/storage_paint.rs");
  const stat = await fs.lstat(sourcePath);
  assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "canonical warehouse painter is ordinary");
  const source = await fs.readFile(sourcePath, "utf8");
  const declaration = source.match(/pub const STORAGE_REQUIRED_SKINS:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/);
  assert.ok(declaration, "canonical warehouse static skin declaration exists");
  const skins = [...declaration[1].matchAll(/"(original-ui\/[^"\r\n]+\.png)"/g)].map(match => match[1]);
  assert.equal(skins.length, 16, "bounded canonical warehouse static art closure");
  assert.equal(new Set(skins).size, skins.length, "canonical skin paths are unique");
  return skins;
}

test("canonical warehouse static skins are local while adjacent media and old remote roots stay remote", async () => {
  const skins = await warehouseSkinReference73();
  for (const skin of skins) assert.equal(selectPublic(skin, false), true, skin);
  for (const skin of ["original-ui/Prguse/2092.png", "original-ui/Prguse/2094.png",
    "original-ui/Prguse/2095.png"]) assert.equal(selectPublic(skin, false), true, skin);
  for (const remote of ["original-ui/Title/742.png", "original-ui/Title/747.png",
    "original-ui/Title/113.mask.png", "original-ui/Prguse/585.png", "original-ui/Prguse2/359.png",
    "original-ui/Items/2.png", "original-ui/ChrSel/0.png", "original-ui/Sound/1.wav"])
    assert.equal(selectPublic(remote, false), false, remote);
});

test("actual public copy retains exact warehouse bytes, legacy art and metadata without whole-library selection", async () => {
  const skins = await warehouseSkinReference73();
  const base = path.join(fixtureRoot, "warehouse-skins-copy");
  const source = path.join(base, "source"), destination = path.join(base, "package");
  const legacy = ["original-ui/Prguse/2092.png", "original-ui/Prguse/2094.png", "original-ui/Prguse/2095.png"];
  const bodies = new Map([...skins, ...legacy].map(skin => [skin, Buffer.from("exact source bytes: " + skin)]));
  for (const [skin, bytes] of bodies) await write(path.join(source, skin), bytes);
  const meta = { frames: skins.map(skin => ({ path: "/" + skin })) };
  await write(path.join(source, "original-ui/Title/meta.json"), JSON.stringify(meta, null, 2));
  await write(path.join(source, "original-ui/Title/742.png"), "remote neighbor");
  const copy = await copySelectedPublic({ sourceRoot: source, destinationRoot: destination, selection: selectPublic });
  assert.equal(copy.files, bodies.size + 1);
  for (const [skin, bytes] of bodies) {
    assert.deepEqual(await fs.readFile(path.join(destination, skin)), bytes, skin);
    assert.deepEqual(await fs.readFile(path.join(source, skin)), bytes, "source retained: " + skin);
  }
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(destination, "original-ui/Title/meta.json"), "utf8")), meta);
  assert.equal(await exists(path.join(destination, "original-ui/Title/742.png")), false);
  assert.equal((await auditPortableTree(destination)).links, 0);
});

test("absent warehouse skin stays absent after actual copy and is never replaced by a neighboring frame", async () => {
  const skins = await warehouseSkinReference73(), missing = "original-ui/Title/743.png";
  assert.ok(skins.includes(missing));
  const base = path.join(fixtureRoot, "warehouse-skin-missing");
  const source = path.join(base, "source"), destination = path.join(base, "package");
  for (const skin of skins.filter(skin => skin !== missing)) await write(path.join(source, skin), skin);
  await write(path.join(source, "original-ui/Title/742.png"), "different adjacent frame");
  const copy = await copySelectedPublic({ sourceRoot: source, destinationRoot: destination, selection: selectPublic });
  assert.equal(copy.files, skins.length - 1);
  assert.equal(await exists(path.join(source, missing)), false);
  assert.equal(await exists(path.join(destination, missing)), false);
  await assert.rejects(fs.readFile(path.join(destination, missing)), error => error.code === "ENOENT");
  assert.equal(await exists(path.join(destination, "original-ui/Title/742.png")), false);
  for (const skin of skins.filter(skin => skin !== missing))
    assert.equal(await fs.readFile(path.join(destination, skin), "utf8"), skin, skin);
});

test("warehouse local skin copy retains fatal differing-source collision checks", async () => {
  const skins = await warehouseSkinReference73(), skin = "original-ui/Title/113.png";
  assert.ok(skins.includes(skin));
  const base = path.join(fixtureRoot, "warehouse-skin-collision");
  const source = path.join(base, "source"), destination = path.join(base, "package");
  await write(path.join(source, skin), "exact source warehouse password art");
  await write(path.join(destination, skin), "different previously copied art");
  await assert.rejects(copySelectedPublic({ sourceRoot: source, destinationRoot: destination,
    selection: selectPublic }), /collision differs/);
  assert.equal(await fs.readFile(path.join(source, skin), "utf8"), "exact source warehouse password art");
  assert.equal(await fs.readFile(path.join(destination, skin), "utf8"), "different previously copied art");
});

async function bagStorageSkinReference74() {
  const sourcePath = path.resolve(import.meta.dirname,
    "../../game-client/client-bevy/src/crystal_ui/bag_paint.rs");
  const stat = await fs.lstat(sourcePath);
  assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "canonical Bag painter is ordinary");
  const source = await fs.readFile(sourcePath, "utf8");
  const declaration = source.match(/pub const PORTABLE_BAG_REQUIRED_SKINS:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/);
  assert.ok(declaration, "canonical Bag static skin declaration exists");
  const bag = [...declaration[1].matchAll(/"(original-ui\/[^"\r\n]+\.png)"/g)].map(match => match[1]);
  assert.equal(bag.length, 17, "bounded canonical Bag static art closure");
  assert.equal(new Set(bag).size, bag.length, "canonical Bag skin paths are unique");
  const storage = await warehouseSkinReference73();
  const skins = [...new Set([...bag, ...storage])];
  assert.equal(skins.length, 30, "canonical Bag and Storage static art union");
  return skins;
}

test("canonical Bag and Storage static skin union is local without expanding adjacent remote media", async () => {
  const skins = await bagStorageSkinReference74();
  for (const skin of skins) assert.equal(selectPublic(skin, false), true, skin);
  for (const remote of ["original-ui/Title/195.png", "original-ui/Title/199.png",
    "original-ui/Title/736.png", "original-ui/Title/740.png", "original-ui/Prguse/23.png",
    "original-ui/Prguse2/365.png", "original-ui/Prguse2/369.png", "original-ui/Items/2.png"])
    assert.equal(selectPublic(remote, false), false, remote);
  // UI_32bit was already local; its existing neighboring-frame behavior must remain intact.
  assert.equal(selectPublic("original-ui/UI_32bit/472.png", false), true);
});

test("actual public copy retains all thirty genuine Bag and Storage PNG bytes", async () => {
  const skins = await bagStorageSkinReference74();
  const publicRoot = path.resolve(import.meta.dirname, "../public");
  const base = path.join(fixtureRoot, "bag-storage-genuine-skins-copy");
  const source = path.join(base, "source"), destination = path.join(base, "package");
  const bodies = new Map();
  for (const skin of skins) {
    const original = path.join(publicRoot, ...skin.split("/"));
    const stat = await fs.lstat(original);
    assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "genuine source skin is ordinary: " + skin);
    const bytes = await fs.readFile(original);
    assert.deepEqual(bytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
      "genuine source PNG signature: " + skin);
    bodies.set(skin, bytes);
    await write(path.join(source, skin), bytes);
  }
  const remote = "original-ui/Title/195.png";
  await write(path.join(source, remote), "unselected adjacent frame");
  const copy = await copySelectedPublic({ sourceRoot: source, destinationRoot: destination, selection: selectPublic });
  assert.equal(copy.files, 30);
  for (const [skin, bytes] of bodies) {
    assert.deepEqual(await fs.readFile(path.join(destination, skin)), bytes, "packaged original bytes: " + skin);
    assert.deepEqual(await fs.readFile(path.join(source, skin)), bytes, "fixture source retained: " + skin);
    assert.deepEqual(await fs.readFile(path.join(publicRoot, ...skin.split("/"))), bytes,
      "genuine source retained: " + skin);
  }
  assert.equal(await exists(path.join(destination, remote)), false);
  assert.equal((await auditPortableTree(destination)).links, 0);
});

async function shopSkinReference76() {
  const sourcePath = path.resolve(import.meta.dirname,
    "../../game-client/client-bevy/src/crystal_ui/shop_paint.rs");
  const stat = await fs.lstat(sourcePath);
  assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "canonical Shop painter is ordinary");
  const source = await fs.readFile(sourcePath, "utf8");
  const declaration = source.match(/pub const SHOP_REQUIRED_SKINS:\s*&\[&str\]\s*=\s*&\[([\s\S]*?)\];/);
  assert.ok(declaration, "canonical Shop static skin declaration exists");
  const skins = [...declaration[1].matchAll(/"(original-ui\/[^"\r\n]+\.png)"/g)].map(match => match[1]);
  assert.equal(skins.length, 15, "bounded canonical Shop static art closure");
  assert.equal(new Set(skins).size, skins.length, "canonical Shop skin paths are unique");
  return skins;
}

test("canonical Shop static skins join Bag and Storage without selecting neighboring or dynamic media", async () => {
  const shop = await shopSkinReference76(), bagStorage = await bagStorageSkinReference74();
  assert.equal(shop.filter(skin => !bagStorage.includes(skin)).length, 12, "exact missing Shop skin addition");
  const skins = [...new Set([...bagStorage, ...shop])];
  assert.equal(skins.length, 42, "canonical Bag Storage and Shop static union");
  for (const skin of skins) assert.equal(selectPublic(skin, false), true, skin);
  for (const remote of ["original-ui/Prguse/999.png", "original-ui/Prguse/1001.png",
    "original-ui/Prguse/549.png", "original-ui/Prguse/551.png",
    "original-ui/Prguse2/196.png", "original-ui/Prguse2/200.png",
    "original-ui/Prguse2/204.png", "original-ui/Prguse2/206.png", "original-ui/Prguse2/210.png",
    "original-ui/Title/311.png", "original-ui/Title/315.png", "original-ui/Title/312.mask.png",
    "original-ui/Items/2.png", "original-ui/StateItem/2.png", "original-ui/ChrSel/0.png", "original-ui/Sound/1.wav"])
    assert.equal(selectPublic(remote, false), false, remote);
});

test("actual public copy retains all forty-two genuine Bag Storage and Shop PNGs with missing and collision safeguards", async () => {
  const shop = await shopSkinReference76(), bagStorage = await bagStorageSkinReference74();
  const skins = [...new Set([...bagStorage, ...shop])];
  const publicRoot = path.resolve(import.meta.dirname, "../public");
  const base = path.join(fixtureRoot, "bag-storage-shop-genuine-skins-copy");
  const source = path.join(base, "source"), destination = path.join(base, "package");
  const bodies = new Map();
  for (const skin of skins) {
    const original = path.join(publicRoot, ...skin.split("/"));
    const stat = await fs.lstat(original);
    assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "genuine source skin is ordinary: " + skin);
    const bytes = await fs.readFile(original);
    assert.deepEqual(bytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
      "genuine source PNG signature: " + skin);
    bodies.set(skin, bytes);
    await write(path.join(source, skin), bytes);
  }
  assert.equal(shop.filter(skin => !bagStorage.includes(skin)).reduce((bytes, skin) => bytes + bodies.get(skin).length, 0),
    31495, "exact twelve genuine newly selected Shop bytes");
  const copy = await copySelectedPublic({ sourceRoot: source, destinationRoot: destination, selection: selectPublic });
  assert.equal(copy.files, 42);
  for (const [skin, bytes] of bodies) {
    assert.deepEqual(await fs.readFile(path.join(destination, skin)), bytes, "packaged original bytes: " + skin);
    assert.deepEqual(await fs.readFile(path.join(source, skin)), bytes, "fixture source retained: " + skin);
    assert.deepEqual(await fs.readFile(path.join(publicRoot, ...skin.split("/"))), bytes, "genuine source retained: " + skin);
  }
  assert.equal((await auditPortableTree(destination)).links, 0);
  const missing = "original-ui/Prguse/1000.png";
  assert.ok(shop.includes(missing));
  const missingSource = path.join(base, "missing-source"), missingDestination = path.join(base, "missing-package");
  for (const [skin, bytes] of bodies) if (skin !== missing) await write(path.join(missingSource, skin), bytes);
  const incomplete = await copySelectedPublic({ sourceRoot: missingSource, destinationRoot: missingDestination, selection: selectPublic });
  assert.equal(incomplete.files, 41);
  assert.equal(await exists(path.join(missingSource, missing)), false);
  assert.equal(await exists(path.join(missingDestination, missing)), false);
  await assert.rejects(fs.readFile(path.join(missingDestination, missing)), error => error.code === "ENOENT");
  const collisionSource = path.join(base, "collision-source"), collisionDestination = path.join(base, "collision-package");
  const different = bodies.get("original-ui/Prguse/550.png");
  assert.notDeepEqual(different, bodies.get(missing), "two genuine source skins are not substitutes");
  await write(path.join(collisionSource, missing), bodies.get(missing));
  await write(path.join(collisionDestination, missing), different);
  await assert.rejects(copySelectedPublic({ sourceRoot: collisionSource, destinationRoot: collisionDestination,
    selection: selectPublic }), /collision differs/);
  assert.deepEqual(await fs.readFile(path.join(collisionSource, missing)), bodies.get(missing));
  assert.deepEqual(await fs.readFile(path.join(collisionDestination, missing)), different);
});

// These finite alternatives are derived from actual Rust contracts, never the selector's own list.
async function canonicalRust76(relative) {
  const file = path.resolve(import.meta.dirname, "../../game-client", relative);
  const stat = await fs.lstat(file);
  assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "canonical Rust source is ordinary: " + relative);
  return fs.readFile(file, "utf8");
}
function rustFunction76(source, name) {
  const signature = new RegExp("\\bfn\\s+" + name + "\\s*\\(").exec(source);
  assert.ok(signature, "canonical function exists: " + name);
  const start = source.indexOf("{", signature.index);
  let depth = 1, end = start + 1;
  for (; end < source.length && depth; end++) {
    if (source[end] === "{") depth++;
    else if (source[end] === "}") depth--;
  }
  assert.equal(depth, 0, "canonical function has complete body: " + name);
  return source.slice(start + 1, end - 1);
}
const literalAssets76 = text => [...text.matchAll(/"(original-ui\/[^"{}\r\n]+\.png)"/g)].map(match => match[1]);
const integerRange76 = (first, last) => Array.from({ length: last - first + 1 }, (_, i) => first + i);
async function characterSpellsSkinReference76() {
  const character = rustFunction76(await canonicalRust76("client-bevy/src/portable_character_ui.rs"), "required_assets");
  const stats = rustFunction76(await canonicalRust76("client-bevy/src/crystal_ui/character_stats_portable.rs"), "required_assets");
  const spells = rustFunction76(await canonicalRust76("runtime/src/spells_ui_host.rs"), "required_assets");
  const appearance = await canonicalRust76("client-bevy/src/crystal_ui/character_page.rs");
  const classes = rustFunction76(await canonicalRust76("client-bevy/src/crystal_ui/character_stats.rs"), "class_image_index");
  const materials = await canonicalRust76("client-bevy/src/crystal_ui/character_materials.rs");
  assert.match(character, /crystal_character_page_index/); assert.match(character, /class_image_index/);
  assert.match(character, /crystal_character_paper_doll_layers/); assert.match(character, /user_item_image_index/);
  const classNames = /\[([^\]]+)\]\s*\.iter\(\)/.exec(classes);
  assert.ok(classNames);
  const classBase = /\.map\(\|index\|\s*(\d+)\s*\+\s*index as u16\)/.exec(classes);
  assert.ok(classBase);
  const classFrames = [...classNames[1].matchAll(/"([^"]+)"/g)].map((_, i) => Number(classBase[1]) + i);
  const statsRange = /\.chain\(\((\d+)\.\.=(\d+)\)\.map\(\|i\|\s*format!\("original-ui\/Prguse\/\{i\}\.png"\)\)\)/.exec(stats);
  assert.ok(statsRange);
  assert.deepEqual(integerRange76(Number(statsRange[1]), Number(statsRange[2])), classFrames);
  const paths = [...literalAssets76(character), ...literalAssets76(stats),
    ...classFrames.map(index => "original-ui/Prguse/" + index + ".png")];
  assert.match(spells, /character_stats_portable::required_assets\(\)/);
  const staticLoops = [...spells.matchAll(/for i in \[([\d,\s]+)\]\s*\{\s*p\.push\(format!\("original-ui\/([^/"]+)\/\{i\}\.png"\)\);\s*\}/g)];
  assert.equal(staticLoops.length, 2, "only literal static Title and Prguse loops");
  for (const [, values, library] of staticLoops)
    paths.push(...values.split(",").map(v => v.trim()).filter(Boolean).map(index => "original-ui/" + library + "/" + Number(index) + ".png"));
  const spellsRange = /for i in (\d+)\.\.=(\d+)\s*\{\s*p\.push\(format!\("original-ui\/([^/"]+)\/\{i\}\.png"\)\);\s*\}/.exec(spells);
  assert.ok(spellsRange);
  paths.push(...integerRange76(Number(spellsRange[1]), Number(spellsRange[2])).map(index => "original-ui/" + spellsRange[3] + "/" + index + ".png"));
  const genders = [...rustFunction76(appearance, "crystal_character_gender_offset").matchAll(/Some\((\d+)\)/g)].map(m => Number(m[1]));
  assert.deepEqual(genders, [0, 1]);
  const pageBase = /(\d+)\s*\+\s*crystal_character_gender_offset/.exec(rustFunction76(appearance, "crystal_character_page_index"));
  assert.ok(pageBase);
  paths.push(...genders.map(offset => "original-ui/Prguse/" + (Number(pageBase[1]) + offset) + ".png"));
  const core = [...new Set(paths)];
  assert.equal(core.length, 69, "canonical general Character Stats and Spells static alternatives");
  const hair = rustFunction76(appearance, "crystal_character_hair_frame");
  assert.match(hair, /hair >= CRYSTAL_MALE_HAIR_RECTS\.len\(\)/);
  assert.match(hair, /CrystalFrameSpec::new\("Prguse", base \+ hair as u16, rect\)/);
  const hairBranches = [...hair.matchAll(/\((\d+),\s*(true|false)\)\s*=>\s*\((\d+),\s*([A-Z_]+)\[hair\]\)/g)];
  assert.equal(hairBranches.length, 4, "all authored gender/Assassin alternatives");
  const hairPaths = [];
  for (const [, , , base, rects] of hairBranches) {
    const declaration = new RegExp("const " + rects + ":\\s*\\[CrystalRect;\\s*(\\d+)\\]\\s*=\\s*\\[([\\s\\S]*?)\\];").exec(appearance);
    assert.ok(declaration);
    const count = Number(declaration[1]);
    assert.equal([...declaration[2].matchAll(/CrystalRect::new\(/g)].length, count, "actual authored hair rect count");
    assert.equal(count, 9);
    hairPaths.push(...integerRange76(Number(base), Number(base) + count - 1).map(index => "original-ui/Prguse/" + index + ".png"));
  }
  assert.equal(new Set(hairPaths).size, 36);
  const wings = rustFunction76(appearance, "crystal_character_wing_frame");
  const wingBase = /let index = (\d+) \+ wing_offset \+ gender_offset;/.exec(wings);
  assert.ok(wingBase);
  const wingOffsets = [...wings.matchAll(/^\s*\d+\s*=>\s*(\d+),/gm)].map(m => Number(m[1]));
  assert.deepEqual(wingOffsets, [2, 4]);
  const wingPaths = wingOffsets.flatMap(offset => genders.map(gender => "original-ui/Prguse2/" + (Number(wingBase[1]) + offset + gender) + ".png"));
  const materialBase = /format!\("original-ui\/Prguse2\/\{\}\.png",\s*(\d+)\s*\+\s*offset\)/.exec(materials);
  const materialCount = /CrystalCharacterWingMaterials\(pub\(crate\) \[Handle<CrystalAdditiveUiMaterial>;\s*(\d+)\]\)/.exec(materials);
  assert.ok(materialBase); assert.ok(materialCount);
  assert.deepEqual(wingPaths, integerRange76(Number(materialBase[1]), Number(materialBase[1]) + Number(materialCount[1]) - 1).map(index => "original-ui/Prguse2/" + index + ".png"));
  assert.match(spells, /for b in &m\.bindings/); assert.match(spells, /original-ui\/MagIcon2\/\{index\}\.png/);
  const skins = [...new Set([...core, ...hairPaths, ...wingPaths])];
  assert.equal(skins.length, 109);
  assert.equal(skins.some(skin => /\/(?:Items|StateItem|MagIcon2)\//.test(skin)), false, "account-derived frames are not static alternatives");
  return skins;
}

test("canonical Character Stats Spells hair and wing alternatives form the exact bounded static selection", async () => {
  const character = await characterSpellsSkinReference76();
  const inherited = [...new Set([...await bagStorageSkinReference74(), ...await shopSkinReference76()])];
  assert.equal(character.filter(skin => !inherited.includes(skin)).length, 106);
  const skins = [...new Set([...inherited, ...character])];
  assert.equal(skins.length, 148, "Bag30 plus Shop12 plus Character106");
  for (const skin of skins) assert.equal(selectPublic(skin, false), true, skin);
  for (const remote of ["original-ui/Prguse/99.png", "original-ui/Prguse/105.png", "original-ui/Prguse/339.png",
    "original-ui/Prguse/342.png", "original-ui/Prguse/440.png", "original-ui/Prguse/450.png",
    "original-ui/Prguse/460.png", "original-ui/Prguse/470.png", "original-ui/Prguse/480.png",
    "original-ui/Prguse/490.png", "original-ui/Prguse/510.png", "original-ui/Prguse2/1201.png",
    "original-ui/Prguse2/1206.png", "original-ui/Prguse2/1289.png", "original-ui/Prguse2/1325.png",
    "original-ui/Title/505.png", "original-ui/Title/509.png", "original-ui/Title/518.png",
    "original-ui/Items/2.png", "original-ui/StateItem/0.png", "original-ui/MagIcon2/0.png"])
    assert.equal(selectPublic(remote, false), false, remote);
});

test("actual complete static union copies genuine PNG and metadata hashes while absent character art stays absent", async () => {
  const character = await characterSpellsSkinReference76(), bag = await bagStorageSkinReference74(), shop = await shopSkinReference76();
  const inherited = new Set([...bag, ...shop]), skins = [...new Set([...inherited, ...character])];
  const publicRoot = path.resolve(import.meta.dirname, "../public");
  const base = path.join(fixtureRoot, "character-spells-genuine-static-union");
  const source = path.join(base, "source"), destination = path.join(base, "package");
  const bodies = new Map(), metadata = new Map();
  for (const skin of skins) {
    const original = path.join(publicRoot, ...skin.split("/")), stat = await fs.lstat(original);
    assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "genuine static source is ordinary: " + skin);
    const bytes = await fs.readFile(original);
    assert.deepEqual(bytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), skin);
    if (character.includes(skin)) {
      const library = skin.split("/")[1];
      if (!metadata.has(library)) {
        const file = path.join(publicRoot, "original-ui", library, "meta.json"), raw = await fs.readFile(file);
        metadata.set(library, { file, raw, parsed: JSON.parse(raw.toString("utf8")) });
      }
      const frames = metadata.get(library).parsed.frames.filter(frame => frame.path === "/" + skin);
      assert.equal(frames.length, 1, "single canonical exported metadata frame: " + skin);
      assert.equal(createHash("sha256").update(bytes).digest("hex"), frames[0].pngSha256, "genuine PNG equals export hash: " + skin);
    }
    bodies.set(skin, bytes); await write(path.join(source, skin), bytes);
  }
  const byteSum = paths => paths.reduce((sum, skin) => sum + bodies.get(skin).length, 0);
  assert.equal(byteSum(character.filter(skin => !inherited.has(skin))), 503189);
  assert.equal(byteSum(skins.filter(skin => !bag.includes(skin))), 534684, "exact total118 additions");
  const copy = await copySelectedPublic({ sourceRoot: source, destinationRoot: destination, selection: selectPublic });
  assert.equal(copy.files, 148);
  for (const [skin, bytes] of bodies) {
    assert.deepEqual(await fs.readFile(path.join(destination, skin)), bytes, "packaged original bytes: " + skin);
    assert.deepEqual(await fs.readFile(path.join(source, skin)), bytes, "fixture source retained: " + skin);
    assert.deepEqual(await fs.readFile(path.join(publicRoot, ...skin.split("/"))), bytes, "genuine original retained: " + skin);
  }
  for (const { file, raw } of metadata.values()) assert.deepEqual(await fs.readFile(file), raw, "canonical metadata retained byte-for-byte");
  assert.equal((await auditPortableTree(destination)).links, 0);
  const missing = "original-ui/Title/504.png";
  assert.ok(character.includes(missing));
  const missingSource = path.join(base, "missing-source"), missingDestination = path.join(base, "missing-package");
  for (const [skin, bytes] of bodies) if (skin !== missing) await write(path.join(missingSource, skin), bytes);
  assert.equal((await copySelectedPublic({ sourceRoot: missingSource, destinationRoot: missingDestination, selection: selectPublic })).files, 147);
  assert.equal(await exists(path.join(missingSource, missing)), false);
  assert.equal(await exists(path.join(missingDestination, missing)), false);
  await assert.rejects(fs.readFile(path.join(missingDestination, missing)), error => error.code === "ENOENT");
});
// Read the public declarations and actual shared image/seed rules, not the approved selector list.
async function canonicalInput77(relative) {
  const file = path.resolve(import.meta.dirname, "../../..", relative);
  const stat = await fs.lstat(file);
  assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "canonical ordinary input: " + relative);
  return fs.readFile(file, "utf8");
}
function concreteImageRule77(source) {
  const body = rustFunction76(source, "crystal_user_item_image");
  assert.match(body, /if item_type != 8 \|\| stack_size == 0\s*\{\s*return catalogue_image;/);
  assert.match(body, /match \(shape, count\)/);
  assert.match(body, /_\s*=>\s*catalogue_image/);
  const arms = [...body.matchAll(/\((\d+),\s*(_|\d+\.\.(?:=\d+)?)\)\s*=>\s*(\d+),/g)].map(([, shape, range, image]) => {
    const bounds = /^(\d+)\.\.(?:=(\d+))?$/.exec(range);
    return { shape: Number(shape), first: bounds ? Number(bounds[1]) : 0,
      last: bounds?.[2] ? Number(bounds[2]) : Infinity, image: Number(image), fallback: range === "_" };
  });
  assert.equal(arms.length, 11, "all current concrete Amulet/poison match arms");
  const image = (item, count) => {
    if (item.item_type !== 8 || item.stack_size === 0) return item.image;
    return arms.find(arm => arm.shape === item.shape && count >= arm.first && count <= arm.last)?.image ?? item.image;
  };
  const counts = item => [...new Set([1, ...arms.filter(arm => arm.shape === item.shape).flatMap(arm =>
    [arm.first - 1, arm.first, arm.last, arm.last + 1].filter(count => Number.isFinite(count) && count > 0 && count <= 0xffffffff))])];
  return { image, counts };
}
function tradeGood77(line, items) {
  const text = line.trim();
  if (!text || text.startsWith("#") || text.startsWith("[")) return null;
  const split = text.lastIndexOf(" "), suffix = text.slice(split + 1).trim();
  const parsed = split >= 0 && /^\+?\d+$/.test(suffix) ? Number(suffix) : NaN;
  const validCount = Number.isInteger(parsed) && parsed >= 0 && parsed <= 0xffff;
  const name = validCount ? text.slice(0, split).trim() : text;
  const item = items.find(item => item.name.toLowerCase() === name.toLowerCase());
  return item ? { item, count: validCount ? Math.max(1, parsed) : 1 } : null;
}
let finiteCatalogPromise77;
function finiteCatalog77() {
  return finiteCatalogPromise77 ??= (async () => {
    const [npcRaw, scriptRaw, itemRaw, imageSource, dataSource, npcSource, labelsSource,
      inventorySource, shopSource, shopHost, equipmentSource, typesSource, appearance] = await Promise.all([
      canonicalInput77("packages/game-data/data/generated/crystal_npc_info_manifest.json"),
      canonicalInput77("packages/game-data/data/generated/crystal_npc_manifest.json"),
      canonicalInput77("packages/game-data/data/generated/crystal_item_manifest.json"),
      canonicalInput77("packages/protocol/src/item_image.rs"),
      canonicalInput77("packages/game-data/src/lib.rs"),
      canonicalInput77("apps/simulation/src/runtime/npc.rs"),
      canonicalInput77("apps/simulation/src/runtime/npc_script.rs"),
      canonicalInput77("apps/game-client/client-bevy/src/inventory.rs"),
      canonicalInput77("apps/game-client/client-bevy/src/shop.rs"),
      canonicalInput77("apps/game-client/runtime/src/npc_shop_ui_host.rs"),
      canonicalInput77("apps/simulation/src/runtime/equipment.rs"),
      canonicalInput77("packages/protocol/src/types.rs"),
      canonicalInput77("apps/game-client/client-bevy/src/crystal_ui/character_page.rs"),
    ]);
    assert.match(rustFunction76(dataSource, "crystal_item_by_name"), /\.find\(\|item\| item\.name\.eq_ignore_ascii_case\(name\)\)/);
    assert.match(rustFunction76(dataSource, "crystal_npc_script_by_key"), /\.find\(\|script\| script\.script_key == script_key\)/);
    assert.match(rustFunction76(npcSource, "crystal_npc_trade_goods_for_script"), /crystal_npc_goods_for_script_section\(script, "Trade"\)/);
    assert.match(rustFunction76(npcSource, "crystal_npc_goods_for_script_section"), /crystal_npc_section\(script, label\)/);
    assert.match(labelsSource, /fn crystal_npc_section<'a>[\s\S]*?\.find\(\|section\| crystal_npc_labels_match\(&section\.label, label\)\)/);
    assert.match(rustFunction76(labelsSource, "crystal_npc_labels_match"), /section_label\.eq_ignore_ascii_case\(requested_label\)/);
    assert.match(rustFunction76(labelsSource, "crystal_npc_label_base"), /\.trim\(\)\s*\.split_once\('\('\)/);
    const lineRule = rustFunction76(npcSource, "crystal_npc_trade_good_from_line");
    assert.match(lineRule, /trimmed\.rsplit_once\(' '\)/);
    assert.match(lineRule, /count\.trim\(\)\.parse::<u16>\(\)/);
    assert.match(lineRule, /Ok\(parsed\) => \(name\.trim\(\), parsed\.max\(1\)\)/);
    assert.match(lineRule, /Err\(_\) => \(trimmed, 1\)/);
    assert.match(lineRule, /trimmed\.starts_with\('#'\) \|\| trimmed\.starts_with\('\['\)/);
    assert.match(lineRule, /crystal_item_by_name\(item_name\)\?/);
    assert.match(rustFunction76(inventorySource, "user_item_image"), /mir2_protocol::crystal_user_item_image\(\s*self\.info\.item_type,\s*self\.info\.shape,\s*self\.info\.stack_size,\s*self\.info\.image,\s*count,/);
    assert.match(rustFunction76(inventorySource, "concrete_item_image_index"), /source\.user_item_image\(count\)/);
    assert.match(rustFunction76(shopSource, "user_item_image_index"), /concrete_item_image_index\(\s*self\.icon,\s*u32::from\(self\.count\),\s*self\.tooltip_source\.as_ref\(\)/);
    const hostPaths = rustFunction76(shopHost, "catalog_icon_paths");
    assert.match(hostPaths, /good\.user_item_image_index\(\)/);
    assert.match(hostPaths, /format!\("original-ui\/Items\/\{index\}\.png"\)/);
    const { npcs } = JSON.parse(npcRaw), { scripts } = JSON.parse(scriptRaw), { items } = JSON.parse(itemRaw);
    const rule = concreteImageRule77(imageSource);
    const scriptKeys = [...new Set(npcs.map(npc => npc.script_key))];
    const resolved = [], usableTradeScripts = [], unusableTradeSections = [], goods = [];
    let omitted = 0;
    for (const key of scriptKeys) {
      // Full directory key is authoritative; similarly named scripts elsewhere are not substitutes.
      const script = scripts.find(script => script.script_key === key);
      const trade = script?.sections.find(section => section.label.trim().split("(")[0].trim().toLowerCase() === "trade");
      if (!trade) continue;
      resolved.push(script);
      const goodsBefore = goods.length, omittedBefore = omitted;
      for (const line of trade.lines) {
        const good = tradeGood77(line, items);
        if (good) goods.push(good);
        else if (line.trim() && !line.trim().startsWith("#") && !line.trim().startsWith("[")) omitted++;
      }
      if (goods.length > goodsBefore) usableTradeScripts.push(script);
      else unusableTradeSections.push({ scriptKey: key, lineNumber: trade.line_number,
        lineCount: trade.lines.length, unresolved: omitted - omittedBefore });
    }
    const declared = [...new Set(goods.map(({ item, count }) => "original-ui/Items/" + rule.image(item, count) + ".png"))];
    const threshold = goods.filter(({ item }) => item.item_type === 8 && item.stack_size !== 0 && [0, 1, 2].includes(item.shape));
    const itemPaths = [...new Set([...declared, ...threshold.flatMap(({ item }) =>
      rule.counts(item).map(count => "original-ui/Items/" + rule.image(item, count) + ".png"))])];
    const enumEntries = name => {
      const body = new RegExp("pub enum " + name + "\\s*\\{([^}]+)\\}").exec(typesSource);
      assert.ok(body, name + " canonical enum");
      return [...body[1].matchAll(/(\w+)\s*=\s*(\d+)/g)].map(([, name, value]) => ({ name, value: Number(value) }));
    };
    const classes = enumEntries("MirClass"), genders = enumEntries("MirGender");
    const seed = rustFunction76(equipmentSource, "seed_equipment_items_for_character");
    const indices = (field, type, name) => {
      const body = new RegExp("let " + field + " = match \\w+ \\{([\\s\\S]*?)\\};").exec(seed);
      assert.ok(body, "canonical seed match: " + field);
      const arm = new RegExp(type + "::" + name + "\\s*=>\\s*(\\d+)").exec(body[1]) ?? /_\s*=>\s*(\d+)/.exec(body[1]);
      assert.ok(arm, "canonical seed alternative: " + type + "::" + name);
      return Number(arm[1]);
    };
    assert.match(seed, /crystal_start_equipment\(weapon_index, EquipmentSlot::Weapon\)/);
    assert.match(seed, /crystal_start_equipment\(armour_index, EquipmentSlot::Armour\)/);
    assert.match(equipmentSource, /state_image:\s*crystal_item_template_for_item_key\(&self\.key\)\s*\.map\(\|template\| template\.image\)/);
    assert.match(rustFunction76(appearance, "crystal_character_state_item_frame"), /CrystalFrameSpec::new\(\s*"StateItem",\s*item\.state_image,/);
    const combinations = [], starter = new Map();
    for (const classEntry of classes) for (const genderEntry of genders) {
      const worn = [indices("weapon_index", "MirClass", classEntry.name), indices("armour_index", "MirGender", genderEntry.name)]
        .map(index => items.find(item => item.item_index === index));
      for (const item of worn) {
        assert.ok(item, "canonical seed catalogue item");
        assert.notEqual(item.required_class & (1 << classEntry.value), 0, item.name + " class allowed");
        assert.notEqual(item.required_gender & (1 << genderEntry.value), 0, item.name + " gender allowed");
        starter.set(item.item_index, item);
      }
      combinations.push({ class: classEntry.name, gender: genderEntry.name, worn });
    }
    const statePaths = [...new Set([...starter.values()].map(item => "original-ui/StateItem/" + item.image + ".png"))];
    return { npcs, scripts, items, scriptKeys, resolved, usableTradeScripts, unusableTradeSections, goods, omitted, rule, declared, threshold,
      itemPaths, statePaths, starter, classes, genders, combinations, paths: [...itemPaths, ...statePaths] };
  })();
}

test("public directory-specific first Trade goods and shared concrete-stack rules select only the finite Source77 closure", async () => {
  const catalog = await finiteCatalog77();
  assert.equal(catalog.npcs.length, 375); assert.equal(catalog.scriptKeys.length, 364);
  assert.equal(catalog.resolved.length, 131, "all referenced first Trade sections, including unusable sections");
  assert.equal(catalog.usableTradeScripts.length, 128, "referenced Trade sections with at least one resolved catalogue good");
  assert.equal(catalog.goods.length, 2239);
  assert.equal(catalog.omitted, 421, "unresolved nonempty noncomment Trade lines retain production omission semantics");
  assert.deepEqual(catalog.unusableTradeSections.sort((a, b) => a.scriptKey.localeCompare(b.scriptKey)), [
    { scriptKey: "BichonProvince/BichonWall/Lottery", lineNumber: 170, lineCount: 0, unresolved: 0 },
    { scriptKey: "GM/GM-Script", lineNumber: 29, lineCount: 29, unresolved: 29 },
    { scriptKey: "WoomyonWoods/Castle-GI/Stones", lineNumber: 29, lineCount: 0, unresolved: 0 },
  ], "two empty Trade sections and one wholly unresolved section add no goods or fallback icons");
  assert.equal(catalog.declared.length, 696); assert.equal(catalog.threshold.length, 57);
  assert.equal(catalog.itemPaths.length, 704); assert.equal(catalog.itemPaths.filter(asset => !catalog.declared.includes(asset)).length, 8);
  const blacksmiths = catalog.scripts.filter(script => script.script_key.endsWith("/Blacksmith"));
  assert.ok(blacksmiths.length > 1, "same basename exists in different canonical directories");
  for (const script of blacksmiths)
    assert.equal(catalog.scripts.find(candidate => candidate.script_key === script.script_key), script);
  assert.equal(catalog.scripts.find(script => script.script_key === "Blacksmith"), undefined, "basename cannot admit a different directory");
  const duplicate = catalog.items.find((item, i) => catalog.items.findIndex(first => first.name.toLowerCase() === item.name.toLowerCase()) < i);
  assert.ok(duplicate, "canonical catalogue has same-name variants");
  assert.equal(tradeGood77(duplicate.name.toUpperCase(), catalog.items).item,
    catalog.items.find(item => item.name.toLowerCase() === duplicate.name.toLowerCase()), "first case-insensitive name match, not a later variant");
  const first = catalog.items.find(item => !item.name.includes(" "));
  assert.equal(tradeGood77(first.name + " 0", catalog.items).count, 1);
  assert.equal(tradeGood77(first.name + " 65535", catalog.items).count, 65535);
  assert.equal(tradeGood77(first.name + " 65536", catalog.items), null, "out-of-u16 suffix is part of the unresolved name");
  assert.equal(tradeGood77("# " + first.name, catalog.items), null);
  const rule = catalog.rule.image, amulet = { item_type: 8, shape: 0, stack_size: 500, image: 999 };
  assert.deepEqual([1, 199, 200, 299, 300].map(count => rule(amulet, count)), [3660, 3660, 3661, 3661, 3662]);
  for (const [shape, expected] of [[1, [3673, 3674, 2960, 3675]], [2, [3670, 3671, 2961, 3672]]])
    assert.deepEqual([49, 50, 100, 150].map(count => rule({ ...amulet, shape }, count)), expected);
  assert.equal(rule({ ...amulet, item_type: 1 }, 300), 999);
  assert.equal(rule({ ...amulet, stack_size: 0 }, 300), 999);
  assert.equal(rule({ ...amulet, shape: 3 }, 300), 999);
  const inherited = [...new Set([...await bagStorageSkinReference74(), ...await shopSkinReference76(), ...await characterSpellsSkinReference76(),
    "original-ui/Prguse/2092.png", "original-ui/Prguse/2094.png", "original-ui/Prguse/2095.png"])];
  assert.equal(inherited.length, 151);
  const expected = [...new Set([...inherited, ...catalog.paths])].sort();
  assert.equal(expected.length, 860, "151 retained paths plus exact709 public/starter additions");
  const selector = await fs.readFile(path.join(import.meta.dirname, "lib/portable-next-standalone.mjs"), "utf8");
  const declaration = /const LOCAL_UI_FALLBACKS = new Set\(\[([\s\S]*?)\]\);/.exec(selector);
  assert.ok(declaration);
  assert.deepEqual(literalAssets76(declaration[1]).sort(), expected, "exact finite union, no unrelated dynamic expansion");
  for (const asset of expected) assert.equal(selectPublic(asset, false), true, asset);
  const unreferenced = catalog.items.find(item => !catalog.goods.some(good => good.item.item_index === item.item_index) &&
    !catalog.itemPaths.includes("original-ui/Items/" + item.image + ".png"));
  assert.ok(unreferenced, "actual unreferenced catalogue art remains outside the closure");
  for (const remote of ["original-ui/Items/" + unreferenced.image + ".png", "original-ui/Items/999999.png",
    "original-ui/Items/2.png", "original-ui/StateItem/0.png", "original-ui/StateItem/999999.png", "original-ui/MagIcon2/0.png"])
    assert.equal(selectPublic(remote, false), false, remote);
});

test("canonical initial equipment covers all five classes and both genders with only five StateItem frames", async () => {
  const catalog = await finiteCatalog77();
  assert.deepEqual(catalog.classes.map(entry => entry.name), ["Warrior", "Wizard", "Taoist", "Assassin", "Archer"]);
  assert.deepEqual(catalog.genders.map(entry => entry.name), ["Male", "Female"]);
  assert.equal(catalog.combinations.length, 10);
  assert.deepEqual([...catalog.starter.keys()].sort((a, b) => a - b), [221, 281, 298, 317, 318]);
  assert.deepEqual(catalog.statePaths.sort(), [30, 60, 80, 2259, 3390].map(index => "original-ui/StateItem/" + index + ".png").sort());
  for (const { class: className, gender, worn } of catalog.combinations) {
    assert.equal(worn.length, 2, className + "/" + gender);
    for (const item of worn) {
      assert.equal(selectPublic("original-ui/StateItem/" + item.image + ".png", false), true);
      assert.ok(catalog.itemPaths.includes("original-ui/Items/" + catalog.rule.image(item, 1) + ".png"), "starter inventory icon already comes from public Trade closure");
    }
  }
  for (const index of [0, 29, 31, 59, 61, 79, 81, 2258, 2260, 3389, 3391])
    assert.equal(selectPublic("original-ui/StateItem/" + index + ".png", false), false, "neighbor frame remains remote: " + index);
});

test("actual finite public/starter copy preserves genuine PNG hashes and absent or unreferenced art stays absent", async () => {
  const { paths } = await finiteCatalog77();
  assert.equal(paths.length, 709);
  const publicRoot = path.resolve(import.meta.dirname, "../public"), base = path.join(fixtureRoot, "public-starter-genuine77");
  const source = path.join(base, "source"), destination = path.join(base, "package");
  const metadata = new Map(), bodies = new Map();
  let totalBytes = 0;
  for (const asset of paths) {
    const library = asset.split("/")[1];
    if (!metadata.has(library)) {
      const file = path.join(publicRoot, "original-ui", library, "meta.json"), raw = await fs.readFile(file);
      metadata.set(library, { file, raw, frames: JSON.parse(raw.toString("utf8")).frames });
    }
    const file = path.join(publicRoot, ...asset.split("/")), stat = await fs.lstat(file);
    assert.equal(stat.isFile() && !stat.isSymbolicLink(), true, "genuine finite source: " + asset);
    const bytes = await fs.readFile(file), frames = metadata.get(library).frames.filter(frame => frame.path === "/" + asset);
    assert.deepEqual(bytes.subarray(0, 8), Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
    assert.equal(frames.length, 1, asset);
    assert.equal(createHash("sha256").update(bytes).digest("hex"), frames[0].pngSha256, "canonical export bytes: " + asset);
    bodies.set(asset, bytes); totalBytes += bytes.length; await write(path.join(source, asset), bytes);
  }
  assert.equal(totalBytes, 881070);
  const remote = "original-ui/Items/2.png", remoteBytes = await fs.readFile(path.join(publicRoot, remote));
  await write(path.join(source, remote), remoteBytes);
  assert.equal((await copySelectedPublic({ sourceRoot: source, destinationRoot: destination, selection: selectPublic })).files, 709);
  assert.equal(await exists(path.join(destination, remote)), false);
  assert.deepEqual(await fs.readFile(path.join(source, remote)), remoteBytes);
  for (const [asset, bytes] of bodies) {
    assert.deepEqual(await fs.readFile(path.join(destination, asset)), bytes, "packaged bytes: " + asset);
    assert.deepEqual(await fs.readFile(path.join(source, asset)), bytes, "fixture bytes retained: " + asset);
    assert.deepEqual(await fs.readFile(path.join(publicRoot, ...asset.split("/"))), bytes, "genuine source retained: " + asset);
  }
  for (const { file, raw } of metadata.values()) assert.deepEqual(await fs.readFile(file), raw);
  assert.equal((await auditPortableTree(destination)).links, 0);
  const missing = "original-ui/StateItem/30.png", present = "original-ui/Items/0.png";
  assert.ok(bodies.has(missing) && bodies.has(present));
  const missingSource = path.join(base, "missing-source"), missingDestination = path.join(base, "missing-package");
  await write(path.join(missingSource, present), bodies.get(present));
  await write(path.join(missingSource, remote), remoteBytes);
  assert.equal((await copySelectedPublic({ sourceRoot: missingSource, destinationRoot: missingDestination, selection: selectPublic })).files, 1);
  assert.equal(await exists(path.join(missingSource, missing)), false);
  assert.equal(await exists(path.join(missingDestination, missing)), false);
  await assert.rejects(fs.readFile(path.join(missingDestination, missing)), error => error.code === "ENOENT");
});
