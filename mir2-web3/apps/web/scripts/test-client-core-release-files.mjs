import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import path from "node:path";
import os from "node:os";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { readClientCoreRelease, verifyClientCoreManifest, assertCompiledClientCoreManifest,
  CLIENT_CORE_BUILD_MANIFEST_ENV } from "./lib/client-core-release-files.mjs";
import { copyPortableStandalone, copySelectedPublic, selectThinPublicEntry } from "./lib/portable-next-standalone.mjs";

const names = ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"];
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const payloads = [Buffer.from("export const core = 1;\n"), Buffer.from([0, 97, 115, 109, 1, 0, 0, 0])];
const manifestFor = (parts) => ({ abiVersion: 1, version: hash(Buffer.concat(parts)),
  sourceSha256: "1".repeat(64), files: Object.fromEntries(names.map((name, index) =>
    [name, { bytes: parts[index].length, sha256: hash(parts[index]) }])) });
const manifest = manifestFor(payloads);
const compiled = (value) => ({ config: { env: { [CLIENT_CORE_BUILD_MANIFEST_ENV]: JSON.stringify(value) } } });
const clone = (value) => JSON.parse(JSON.stringify(value));
async function write(file, bytes) { await fs.mkdir(path.dirname(file), { recursive: true }); await fs.writeFile(file, bytes); }
async function fixture(t) {
  const parent = process.env.MIR2_PORTABLE_TEST_ROOT ?? os.tmpdir();
  await fs.mkdir(parent, { recursive: true });
  const root = await fs.mkdtemp(path.join(parent, "core-release-fixture-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  return root;
}
async function release(root, value = manifest, parts = payloads, presentationParts = null) {
  await write(path.join(root, "lib", "generated", "client_core_runtime.json"), JSON.stringify(value));
  for (let i = 0; i < names.length; i += 1) await write(path.join(root, "public", "client-core", value.version, names[i]), parts[i]);
  if (value.presentation && presentationParts) {
    for (let i = 0; i < names.length; i += 1) await write(path.join(root, "public", "client-core", value.presentation.version, names[i]), presentationParts[i]);
  }
}

test("Core ABI1 manifest rejects extra filenames, traversal, invalid identities and byte budgets", () => {
  assert.equal(verifyClientCoreManifest(manifest).version, manifest.version);
  for (const alter of [
    (m) => { m.abiVersion = 2; }, (m) => { m.version = "../escape"; },
    (m) => { m.version = "A".repeat(64); }, (m) => { m.sourceSha256 = "bad"; },
    (m) => { m.extra = 1; }, (m) => { m.files["../escape"] = m.files[names[0]]; },
    (m) => { delete m.files[names[1]]; }, (m) => { m.files[names[0]].extra = 1; },
    (m) => { m.files[names[0]].bytes = 0; }, (m) => { m.files[names[0]].bytes = 1.5; },
    (m) => { m.files[names[0]].sha256 = "not-a-hash"; },
    (m) => { m.files[names[0]].bytes = 204801; }, (m) => { m.files[names[1]].bytes = 262144; },
  ]) { const value = clone(manifest); alter(value); assert.throws(() => verifyClientCoreManifest(value)); }
  const boundary = clone(manifest); boundary.files[names[0]].bytes = 204800; boundary.files[names[1]].bytes = 262143;
  assert.doesNotThrow(() => verifyClientCoreManifest(boundary));
});

test("compiled Core identity rejects missing, malformed and stale Next closure independently of Bevy", () => {
  assert.deepEqual(assertCompiledClientCoreManifest(compiled(manifest), manifest), verifyClientCoreManifest(manifest));
  assert.throws(() => assertCompiledClientCoreManifest({}, manifest), /no compiled Core/);
  assert.throws(() => assertCompiledClientCoreManifest({ config: { env: { [CLIENT_CORE_BUILD_MANIFEST_ENV]: "{" } } }, manifest), /invalid JSON/);
  const next = manifestFor([Buffer.from("export const core = 2;\n"), payloads[1]]);
  assert.throws(() => assertCompiledClientCoreManifest(compiled(manifest), next), /differs/);
  const sourceOnly = clone(manifest); sourceOnly.sourceSha256 = "2".repeat(64);
  assert.throws(() => assertCompiledClientCoreManifest(compiled(manifest), sourceOnly), /differs/);
  const reorder = { ...manifest, files: Object.fromEntries(Object.entries(manifest.files).reverse()) };
  assert.doesNotThrow(() => assertCompiledClientCoreManifest(compiled(reorder), manifest));
});

test("declared Core reads verify both leaf bytes and bundle hash without touching historical files", async (t) => {
  const root = await fixture(t); await release(root);
  const old = path.join(root, "public", "client-core", "2".repeat(64), "historical.js");
  const extra = path.join(root, "public", "client-core", manifest.version, "extra.bin");
  await write(old, "keep history"); await write(extra, "keep source extra");
  const before = await fs.readFile(path.join(root, "lib", "generated", "client_core_runtime.json"));
  const current = readClientCoreRelease({ webRoot: root });
  assert.equal(current.files.length, 2);
  for (let i = 0; i < names.length; i += 1) {
    assert.equal(current.files[i].bytes, payloads[i].length); assert.equal(current.files[i].sha256, hash(payloads[i]));
  }
  assert.deepEqual(await fs.readFile(path.join(root, "lib", "generated", "client_core_runtime.json")), before);
  assert.equal(await fs.readFile(old, "utf8"), "keep history"); assert.equal(await fs.readFile(extra, "utf8"), "keep source extra");
  assert.throws(() => readClientCoreRelease({ webRoot: root, manifest, requireExactClosure: true }), /undeclared entries/);
});

test("Core reader rejects byte drift and separately rejects a false concat version", async (t) => {
  const root = await fixture(t); await release(root);
  await fs.writeFile(path.join(root, "public", "client-core", manifest.version, names[0]), Buffer.from("tampered"));
  assert.throws(() => readClientCoreRelease({ webRoot: root }), /size\/hash mismatch/);
  const falseVersion = clone(manifest); falseVersion.version = "3".repeat(64);
  await release(root, falseVersion);
  assert.throws(() => readClientCoreRelease({ webRoot: root }), /bundle hash/);
});

test("standalone and source public copying retain only current two Core leaves with exact post-copy hashes", async (t) => {
  const root = await fixture(t); const source = path.join(root, "source"); await release(source);
  const oldRelative = "client-core/" + "4".repeat(64) + "/" + names[0];
  const extraRelative = "client-core/" + manifest.version + "/extra.bin";
  await write(path.join(source, "public", ...oldRelative.split("/")), "old");
  await write(path.join(source, "public", ...extraRelative.split("/")), "extra");
  const selected = readClientCoreRelease({ webRoot: source });
  assertCompiledClientCoreManifest(compiled(manifest), selected.manifest);
  const coreFiles = new Set(selected.files.map((entry) => entry.relativePath));
  const selection = (relative, directory) => selectThinPublicEntry(relative, directory, {
    runtimeVersion: "bevy-0123456789abcdef", runtimeFiles: new Set(), coreVersion: manifest.version, coreFiles,
  });
  const standalone = path.join(root, "standalone"); const appRelativePath = "mir2-web3/apps/web";
  const tracedApp = path.join(standalone, ...appRelativePath.split("/"));
  await write(path.join(tracedApp, "server.js"), "server");
  for (const file of [...coreFiles, oldRelative, extraRelative]) {
    await write(path.join(tracedApp, "public", ...file.split("/")), await fs.readFile(path.join(source, "public", ...file.split("/"))));
  }
  const destination = path.join(root, "package");
  await copyPortableStandalone({ sourceRoot: standalone, destinationRoot: destination, appRelativePath, publicSelection: selection });
  const packagedApp = path.join(destination, ...appRelativePath.split("/"));
  const merge = await copySelectedPublic({ sourceRoot: path.join(source, "public"), destinationRoot: path.join(packagedApp, "public"), selection });
  assert.equal(merge.files, 2); assert.equal(merge.collisions, 2);
  const packed = readClientCoreRelease({ webRoot: packagedApp, manifest, requireExactClosure: true });
  assert.deepEqual(await fs.readdir(path.join(packagedApp, "public", "client-core")), [manifest.version]);
  assert.deepEqual((await fs.readdir(packed.versionDirectory)).sort(), [...names].sort());
  for (let i = 0; i < names.length; i += 1) assert.deepEqual(await fs.readFile(packed.files[i].localPath), payloads[i]);
  assert.deepEqual(packed.files.map(({ bytes, sha256 }) => ({ bytes, sha256 })), names.map((name) => manifest.files[name]));
  await assert.rejects(fs.lstat(path.join(packagedApp, "public", ...oldRelative.split("/"))), { code: "ENOENT" });
  await assert.rejects(fs.lstat(path.join(packagedApp, "public", ...extraRelative.split("/"))), { code: "ENOENT" });
  assert.equal(await fs.readFile(path.join(source, "public", ...oldRelative.split("/")), "utf8"), "old");
  assert.equal(await fs.readFile(path.join(source, "public", ...extraRelative.split("/")), "utf8"), "extra");
  await write(path.join(packed.versionDirectory, "extra.bin"), "bad package extra");
  assert.throws(() => readClientCoreRelease({ webRoot: packagedApp, manifest, requireExactClosure: true }), /undeclared entries/);
});

test("Core publication after Next rejects stale compiled identity before selecting its new URL", async (t) => {
  const root = await fixture(t); await release(root); const oldRequiredServerFiles = compiled(manifest);
  const newer = manifestFor([Buffer.from("export const core = 2;\n"), payloads[1]]);
  await release(root, newer, [Buffer.from("export const core = 2;\n"), payloads[1]]);
  assert.throws(() => assertCompiledClientCoreManifest(oldRequiredServerFiles, readClientCoreRelease({ webRoot: root }).manifest), /differs/);
  assert.deepEqual(readClientCoreRelease({ webRoot: root }).manifest, verifyClientCoreManifest(newer));
  assert.equal(await fs.readFile(path.join(root, "public", "client-core", manifest.version, names[0]), "utf8"), payloads[0].toString());
});

test("selected Core symlinks are rejected without following external file content", async (t) => {
  const root = await fixture(t); await release(root);
  const leaf = path.join(root, "public", "client-core", manifest.version, names[0]);
  const external = path.join(root, "outside.js"); await write(external, payloads[0]); await fs.unlink(leaf);
  try { await fs.symlink(external, leaf, "file"); }
  catch (error) { if (error?.code === "EPERM" || error?.code === "EACCES") { t.skip("OS denied symlink fixture: " + error.code); return; } throw error; }
  assert.throws(() => readClientCoreRelease({ webRoot: root }), /linked or irregular/);
});

const presentationPayloads = [Buffer.from("export const presentation = 1;\n"), Buffer.from([0, 97, 115, 109, 1, 0, 0, 0, 1])];
const dualManifest = { ...manifest, presentation: manifestFor(presentationPayloads) };

test("dual Core manifest validates Presentation independently and preserves legacy four-key compatibility", () => {
  const current = verifyClientCoreManifest(dualManifest);
  assert.deepEqual(verifyClientCoreManifest(manifest), verifyClientCoreManifest({ ...manifest }));
  assert.equal(Object.hasOwn(verifyClientCoreManifest(manifest), "presentation"), false);
  assert.equal(current.presentation.version, dualManifest.presentation.version);
  assert.equal(Object.isFrozen(current.presentation), true); assert.equal(Object.isFrozen(current.presentation.files), true);
  assert.notEqual(current.version, current.presentation.version);
  for (const mutate of [
    (value) => { value.presentation = null; }, (value) => { value.presentation = []; },
    (value) => { value.presentation.extra = true; }, (value) => { delete value.presentation.sourceSha256; },
    (value) => { value.presentation.abiVersion = 2; }, (value) => { value.presentation.version = value.version; },
    (value) => { value.presentation.version = "../escape"; }, (value) => { value.presentation.sourceSha256 = "bad"; },
    (value) => { value.presentation.sourceSha256 = "2".repeat(64); },
    (value) => { value.presentation.files["../escape"] = value.presentation.files[names[0]]; },
    (value) => { delete value.presentation.files[names[1]]; },
    (value) => { value.presentation.files[names[0]].extra = true; },
    (value) => { value.presentation.files[names[0]].bytes = 0; },
    (value) => { value.presentation.files[names[0]].bytes = 1.5; },
    (value) => { value.presentation.files[names[0]].sha256 = "bad"; },
    (value) => { value.presentation.files[names[0]].bytes = 204801; },
    (value) => { value.presentation.files[names[1]].bytes = 262144; },
  ]) { const value = clone(dualManifest); mutate(value); assert.throws(() => verifyClientCoreManifest(value)); }
  const boundary = clone(dualManifest);
  boundary.presentation.files[names[0]].bytes = 204800; boundary.presentation.files[names[1]].bytes = 262143;
  assert.doesNotThrow(() => verifyClientCoreManifest(boundary));
});

test("compiled closure includes the entire Presentation identity and is independent of key order", () => {
  assert.deepEqual(assertCompiledClientCoreManifest(compiled(dualManifest), dualManifest), verifyClientCoreManifest(dualManifest));
  assert.throws(() => assertCompiledClientCoreManifest(compiled(manifest), dualManifest), /differs/);
  assert.throws(() => assertCompiledClientCoreManifest(compiled(dualManifest), manifest), /differs/);
  for (const mutate of [
    (value) => { value.presentation.sourceSha256 = "2".repeat(64); },
    (value) => { value.presentation.version = "3".repeat(64); },
    (value) => { value.presentation.files[names[0]].sha256 = "4".repeat(64); },
    (value) => { value.presentation.files[names[1]].bytes += 1; },
  ]) { const value = clone(dualManifest); mutate(value); assert.throws(() => assertCompiledClientCoreManifest(compiled(value), dualManifest), /differs/); }
  const reversed = { ...dualManifest, presentation: Object.fromEntries(Object.entries({ ...dualManifest.presentation,
    files: Object.fromEntries(Object.entries(dualManifest.presentation.files).reverse()) }).reverse()) };
  assert.deepEqual(assertCompiledClientCoreManifest(compiled(reversed), dualManifest), verifyClientCoreManifest(dualManifest));
});

test("dual reader returns four exact leaves and rejects missing active bundles or undeclared package content", async (t) => {
  const root = await fixture(t); await release(root, dualManifest, payloads, presentationPayloads);
  const selected = readClientCoreRelease({ webRoot: root, requireExactClosure: true });
  assert.equal(selected.files.length, 4); assert.equal(selected.versionDirectories.length, 2);
  assert.deepEqual(selected.versionDirectories.map((entry) => path.basename(entry)), [manifest.version, dualManifest.presentation.version]);
  for (let i = 0; i < selected.files.length; i += 1) assert.deepEqual(await fs.readFile(selected.files[i].localPath), [...payloads, ...presentationPayloads][i]);
  const historical = path.join(root, "public", "client-core", "5".repeat(64), "keep.js");
  await write(historical, "history");
  assert.equal(readClientCoreRelease({ webRoot: root }).files.length, 4);
  assert.throws(() => readClientCoreRelease({ webRoot: root, requireExactClosure: true }), /undeclared entries/);
  assert.equal(await fs.readFile(historical, "utf8"), "history");
  const extra = path.join(selected.versionDirectories[1], "extra.bin"); await write(extra, "extra");
  assert.equal(readClientCoreRelease({ webRoot: root }).files.length, 4);
  assert.throws(() => readClientCoreRelease({ webRoot: root, requireExactClosure: true }), /undeclared entries/);
  await fs.unlink(selected.files[2].localPath);
  assert.throws(() => readClientCoreRelease({ webRoot: root }), /ENOENT/);
});

test("dual reader rejects Presentation byte drift and a separately falsified concatenated identity", async (t) => {
  const root = await fixture(t); await release(root, dualManifest, payloads, presentationPayloads);
  await fs.appendFile(path.join(root, "public", "client-core", dualManifest.presentation.version, names[1]), "drift");
  assert.throws(() => readClientCoreRelease({ webRoot: root }), /size\/hash mismatch/);
  const falseVersion = clone(dualManifest); falseVersion.presentation.version = "6".repeat(64);
  await release(root, falseVersion, payloads, presentationPayloads);
  assert.throws(() => readClientCoreRelease({ webRoot: root }), /bundle hash/);
});

test("Thin standalone and source merge retain only both active Core packages with four exact leaves", async (t) => {
  const root = await fixture(t); const source = path.join(root, "source"); await release(source, dualManifest, payloads, presentationPayloads);
  const selected = readClientCoreRelease({ webRoot: source }); const coreFiles = new Set(selected.files.map((entry) => entry.relativePath));
  assertCompiledClientCoreManifest(compiled(dualManifest), selected.manifest);
  const obsolete = "client-core/" + "7".repeat(64) + "/" + names[0];
  const extra = "client-core/" + dualManifest.presentation.version + "/extra.bin";
  await write(path.join(source, "public", ...obsolete.split("/")), "old"); await write(path.join(source, "public", ...extra.split("/")), "extra");
  const selection = (relative, directory) => selectThinPublicEntry(relative, directory, {
    runtimeVersion: "bevy-0123456789abcdef", runtimeFiles: new Set(), coreVersion: manifest.version, coreFiles,
  });
  assert.equal(selection("client-core/" + dualManifest.presentation.version, true), true);
  assert.equal(selection(obsolete, false), false); assert.equal(selection(extra, false), false);
  assert.equal(selection("client-core/" + "7".repeat(64), true), false);
  const standalone = path.join(root, "standalone"), appRelativePath = "mir2-web3/apps/web";
  const tracedApp = path.join(standalone, ...appRelativePath.split("/")); await write(path.join(tracedApp, "server.js"), "server");
  for (const relative of [...coreFiles, obsolete, extra]) await write(path.join(tracedApp, "public", ...relative.split("/")), await fs.readFile(path.join(source, "public", ...relative.split("/"))));
  const destination = path.join(root, "package");
  await copyPortableStandalone({ sourceRoot: standalone, destinationRoot: destination, appRelativePath, publicSelection: selection });
  const packagedApp = path.join(destination, ...appRelativePath.split("/"));
  const merge = await copySelectedPublic({ sourceRoot: path.join(source, "public"), destinationRoot: path.join(packagedApp, "public"), selection });
  assert.equal(merge.files, 4); assert.equal(merge.collisions, 4);
  const packed = readClientCoreRelease({ webRoot: packagedApp, manifest: dualManifest, requireExactClosure: true });
  assert.deepEqual((await fs.readdir(path.join(packagedApp, "public", "client-core"))).sort(), [manifest.version, dualManifest.presentation.version].sort());
  assert.equal(packed.files.length, 4);
  for (let i = 0; i < packed.files.length; i += 1) assert.deepEqual(await fs.readFile(packed.files[i].localPath), [...payloads, ...presentationPayloads][i]);
  await assert.rejects(fs.lstat(path.join(packagedApp, "public", ...obsolete.split("/"))), { code: "ENOENT" });
  await assert.rejects(fs.lstat(path.join(packagedApp, "public", ...extra.split("/"))), { code: "ENOENT" });
  assert.equal(await fs.readFile(path.join(source, "public", ...obsolete.split("/")), "utf8"), "old");
  assert.equal(await fs.readFile(path.join(source, "public", ...extra.split("/")), "utf8"), "extra");
  await write(path.join(packed.versionDirectories[1], "extra.bin"), "bad");
  assert.throws(() => readClientCoreRelease({ webRoot: packagedApp, manifest: dualManifest, requireExactClosure: true }), /undeclared entries/);
});

test("selected Presentation symlinks cannot substitute outside bytes", async (t) => {
  const root = await fixture(t); await release(root, dualManifest, payloads, presentationPayloads);
  const leaf = path.join(root, "public", "client-core", dualManifest.presentation.version, names[0]);
  const outside = path.join(root, "outside.js"); await write(outside, presentationPayloads[0]); await fs.unlink(leaf);
  try { await fs.symlink(outside, leaf, "file"); }
  catch (error) { if (error?.code === "EPERM" || error?.code === "EACCES") { t.skip("OS denied symlink fixture: " + error.code); return; } throw error; }
  assert.throws(() => readClientCoreRelease({ webRoot: root }), /linked or irregular/);
});
