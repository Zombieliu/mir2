import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { computeBevyRuntimeVersion } from "./lib/bevy-runtime-version.mjs";
import { prepareImmutableBevyRuntimeBundle } from "./lib/immutable-bevy-runtime-bundle.mjs";

const ids = ["webgpu", "webgl2", "webgl2-shared"];
const artifacts = ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"];
const sha = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");

function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-immutable-bundle-test-"));
  const source = path.join(root, "flat");
  const releases = path.join(root, "releases");
  fs.mkdirSync(source);
  fs.mkdirSync(releases);
  return { root, source, releases, cleanup() {
    if (path.dirname(root) !== os.tmpdir() || !path.basename(root).startsWith("mir2-immutable-bundle-test-")) {
      throw new Error("refusing to remove an unexpected test directory");
    }
    if (fs.lstatSync(root).isSymbolicLink()) throw new Error("refusing to remove symlink test root");
    fs.rmSync(root, { recursive: true, force: false });
  } };
}

function writeBundle(source, label, withShared = true, abi = 1) {
  const selected = withShared ? ids : ids.slice(0, 2);
  const packages = selected.map((id) => ({ id, backend: id === "webgpu" ? "webgpu" : "webgl2",
    packageDir: `pkg-${id}`, questUiAbiVersion: abi, bagUiAbiVersion: abi,
    primarySharedUiCompiled: id === "webgl2-shared" }));
  const files = [];
  for (const id of selected) {
    const packageDir = path.join(source, `pkg-${id}`);
    fs.mkdirSync(packageDir);
    for (const name of artifacts) {
      const bytes = Buffer.from(`${label}:${id}:${name}`);
      fs.writeFileSync(path.join(packageDir, name), bytes);
      files.push({ path: `public/bevy-runtime/pkg-${id}/${name}`, sha256: sha(bytes) });
    }
  }
  const manifest = { schemaVersion: 2, version: "", packages, files };
  manifest.version = computeBevyRuntimeVersion(manifest);
  return manifest;
}

function fileIn(directory, file) {
  return path.join(directory, ...file.path.slice("public/bevy-runtime/".length).split("/"));
}

test("published version is complete and older bytes survive a flat-current directory swap", () => {
  const env = fixture();
  try {
    const oldManifest = writeBundle(env.source, "old");
    const oldDir = prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: env.source,
      manifest: oldManifest, releasesParentDir: env.releases, nonce: "old1" });
    const original = new Map(oldManifest.files.map((file) => [file.path, fs.readFileSync(fileIn(oldDir, file))]));
    assert.deepEqual(fs.readdirSync(env.releases), [oldManifest.version]);
    assert.deepEqual(JSON.parse(fs.readFileSync(path.join(oldDir, "runtime-manifest.json"), "utf8")), oldManifest);
    fs.renameSync(env.source, path.join(env.root, "retired-flat"));
    fs.mkdirSync(env.source);
    const nextManifest = writeBundle(env.source, "next");
    const nextDir = prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: env.source,
      manifest: nextManifest, releasesParentDir: env.releases, nonce: "next1" });
    assert.notEqual(oldDir, nextDir);
    for (const file of oldManifest.files) assert.deepEqual(fs.readFileSync(fileIn(oldDir, file)), original.get(file.path));
    for (const file of nextManifest.files) assert.equal(sha(fs.readFileSync(fileIn(nextDir, file))), file.sha256);
    assert.deepEqual(fs.readdirSync(env.releases).sort(), [oldManifest.version, nextManifest.version].sort());
  } finally { env.cleanup(); }
});

test("valid version reuse checks bytes and semantic manifest even if metadata ordering differs", () => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "reuse");
    const directory = prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: env.source,
      manifest, releasesParentDir: env.releases, nonce: "first" });
    const saved = fs.readFileSync(path.join(directory, "runtime-manifest.json"), "utf8");
    const reordered = { ...manifest, packages: [...manifest.packages].reverse(), files: [...manifest.files].reverse() };
    assert.equal(prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: env.source,
      manifest: reordered, releasesParentDir: env.releases, nonce: "again" }), directory);
    assert.equal(fs.readFileSync(path.join(directory, "runtime-manifest.json"), "utf8"), saved);
    assert.deepEqual(fs.readdirSync(env.releases), [manifest.version]);
  } finally { env.cleanup(); }
});

test("bad version, incomplete pair, and source hash mismatch publish nothing", () => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "bad");
    const args = (changed, nonce) => ({ sourcePkgParentDir: env.source, manifest: changed,
      releasesParentDir: env.releases, nonce });
    assert.throws(() => prepareImmutableBevyRuntimeBundle(args({ ...manifest, version: "bevy-0000000000000000" }, "version")));
    assert.throws(() => prepareImmutableBevyRuntimeBundle(args({ ...manifest, files: manifest.files.slice(0, -1) }, "missing")));
    const tampered = { ...manifest, files: manifest.files.map((file, index) => index === 0
      ? { ...file, sha256: "a".repeat(64) } : file) };
    tampered.version = computeBevyRuntimeVersion(tampered);
    assert.throws(() => prepareImmutableBevyRuntimeBundle(args(tampered, "hash")), /source runtime hash mismatch/);
    assert.deepEqual(fs.readdirSync(env.releases), []);
  } finally { env.cleanup(); }
});

test("existing version corruption cannot be overwritten or repaired silently", () => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "corrupt");
    const options = { sourcePkgParentDir: env.source, manifest, releasesParentDir: env.releases, nonce: "first" };
    const directory = prepareImmutableBevyRuntimeBundle(options);
    const artifact = fileIn(directory, manifest.files[0]);
    fs.writeFileSync(artifact, "corrupted");
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, nonce: "second" }), /hash mismatch/);
    assert.equal(fs.readFileSync(artifact, "utf8"), "corrupted");
    assert.deepEqual(fs.readdirSync(env.releases), [manifest.version]);
  } finally { env.cleanup(); }
});

test("reuse still rejects a changed flat source while preserving the good version", () => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "stable");
    const options = { sourcePkgParentDir: env.source, manifest, releasesParentDir: env.releases, nonce: "first" };
    const directory = prepareImmutableBevyRuntimeBundle(options);
    const published = fs.readFileSync(fileIn(directory, manifest.files[0]));
    // Replace the flat directory by rename, then put different bytes at the
    // same declared path; never mutate the hardlinked inode in place.
    fs.renameSync(env.source, path.join(env.root, "retired-flat"));
    fs.mkdirSync(env.source);
    const changed = writeBundle(env.source, "different");
    assert.notEqual(changed.version, manifest.version);
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, nonce: "reuse" }), /source runtime hash mismatch/);
    assert.deepEqual(fs.readFileSync(fileIn(directory, manifest.files[0])), published);
    assert.deepEqual(fs.readdirSync(env.releases), [manifest.version]);
  } finally { env.cleanup(); }
});

test("saved capability/version corruption and undeclared destination entries reject reuse", () => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "metadata");
    const options = { sourcePkgParentDir: env.source, manifest, releasesParentDir: env.releases, nonce: "first" };
    const directory = prepareImmutableBevyRuntimeBundle(options);
    const manifestPath = path.join(directory, "runtime-manifest.json");
    fs.writeFileSync(manifestPath, JSON.stringify({ ...manifest,
      packages: manifest.packages.map((item) => item.id === "webgl2-shared"
        ? { ...item, primarySharedUiCompiled: false } : item) }));
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, nonce: "again" }));
    fs.writeFileSync(manifestPath, JSON.stringify(manifest));
    fs.writeFileSync(path.join(directory, "unexpected.bin"), "foreign");
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, nonce: "third" }), /undeclared entries/);
  } finally { env.cleanup(); }
});

test("existing nonce staging is never deleted or overwritten", () => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "stage");
    const stage = path.join(env.releases, `.${manifest.version}.same.staging`);
    fs.mkdirSync(stage);
    fs.writeFileSync(path.join(stage, "sentinel"), "owner");
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: env.source,
      manifest, releasesParentDir: env.releases, nonce: "same" }), /already exists/);
    assert.equal(fs.readFileSync(path.join(stage, "sentinel"), "utf8"), "owner");
  } finally { env.cleanup(); }
});

test("cross-device hardlink failure uses exclusive copy, other link errors do not", () => {
  const env = fixture();
  const original = fs.linkSync;
  try {
    const manifest = writeBundle(env.source, "copy");
    fs.linkSync = () => { const error = new Error("different volume"); error.code = "EXDEV"; throw error; };
    const directory = prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: env.source,
      manifest, releasesParentDir: env.releases, nonce: "copy" });
    for (const file of manifest.files) assert.equal(sha(fs.readFileSync(fileIn(directory, file))), file.sha256);
    fs.linkSync = () => { const error = new Error("I/O fault"); error.code = "EIO"; throw error; };
    fs.renameSync(env.source, path.join(env.root, "retired"));
    fs.mkdirSync(env.source);
    const next = writeBundle(env.source, "io");
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: env.source,
      manifest: next, releasesParentDir: env.releases, nonce: "io" }), /I\/O fault/);
    assert.deepEqual(fs.readdirSync(env.releases), [manifest.version]);
  } finally { fs.linkSync = original; env.cleanup(); }
});

test("rejects traversal metadata and relative roots", () => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "paths");
    const options = { sourcePkgParentDir: env.source, manifest, releasesParentDir: env.releases, nonce: "path" };
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, sourcePkgParentDir: "flat" }));
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, releasesParentDir: "releases" }));
    const bad = { ...manifest, packages: manifest.packages.map((item) => item.id === "webgpu"
      ? { ...item, packageDir: "../outside" } : item) };
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, manifest: bad }));
    assert.deepEqual(fs.readdirSync(env.releases), []);
  } finally { env.cleanup(); }
});

test("rejects source file, destination version, and root symlinks when host permits them", (t) => {
  const env = fixture();
  try {
    const manifest = writeBundle(env.source, "links");
    const sourceFile = fileIn(env.source, manifest.files[0]);
    const real = `${sourceFile}.real`;
    fs.renameSync(sourceFile, real);
    try { fs.symlinkSync(real, sourceFile, "file"); }
    catch (error) {
      if (["EPERM", "EACCES", "ENOTSUP"].includes(error?.code)) { t.skip("symlink creation unavailable on this host"); return; }
      throw error;
    }
    const options = { sourcePkgParentDir: env.source, manifest, releasesParentDir: env.releases, nonce: "link" };
    assert.throws(() => prepareImmutableBevyRuntimeBundle(options), /regular file/);
    fs.rmSync(sourceFile);
    fs.renameSync(real, sourceFile);
    const sourcePackage = path.join(env.source, "pkg-webgpu");
    const realPackage = path.join(env.source, "pkg-webgpu-real");
    fs.renameSync(sourcePackage, realPackage);
    fs.symlinkSync(realPackage, sourcePackage, "dir");
    assert.throws(() => prepareImmutableBevyRuntimeBundle(options), /symlink/);
    fs.rmSync(sourcePackage);
    fs.renameSync(realPackage, sourcePackage);
    const outside = path.join(env.root, "outside");
    fs.mkdirSync(outside);
    const versionLink = path.join(env.releases, manifest.version);
    fs.symlinkSync(outside, versionLink, "dir");
    assert.throws(() => prepareImmutableBevyRuntimeBundle(options), /symlink/);
    fs.rmSync(versionLink);
    fs.symlinkSync(path.join(env.root, "missing-version"), versionLink, "dir");
    assert.throws(() => prepareImmutableBevyRuntimeBundle(options), /symlink/,
      "a dangling destination link is still occupied");
    fs.rmSync(versionLink);
    const releasesLink = path.join(env.root, "releases-link");
    fs.symlinkSync(env.releases, releasesLink, "dir");
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, releasesParentDir: releasesLink }), /symlink/);
    fs.rmSync(releasesLink);
    const directory = prepareImmutableBevyRuntimeBundle(options);
    const packageDirectory = path.join(directory, "pkg-webgl2");
    const realDestinationPackage = path.join(directory, "pkg-webgl2-real");
    fs.renameSync(packageDirectory, realDestinationPackage);
    fs.symlinkSync(realDestinationPackage, packageDirectory, "dir");
    assert.throws(() => prepareImmutableBevyRuntimeBundle({ ...options, nonce: "reuse" }), /undeclared entries|symlink/);
  } finally { env.cleanup(); }
});
