import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { computeBevyRuntimeVersion } from "./lib/bevy-runtime-version.mjs";
import { prepareImmutableBevyRuntimeBundle } from "./lib/immutable-bevy-runtime-bundle.mjs";
import { readImmutableBevyRuntimeRelease } from "./lib/bevy-runtime-release-files.mjs";

const hash = (bytes) => crypto.createHash("sha256").update(bytes).digest("hex");
const names = ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"];

function fixture(mode = "ui1") {
  const webRoot = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-release-files-test-"));
  const publicRoot = path.join(webRoot, "public");
  const source = path.join(publicRoot, "bevy-runtime");
  const releases = path.join(publicRoot, "bevy-runtime-releases");
  const generated = path.join(webRoot, "lib", "generated");
  fs.mkdirSync(source, { recursive: true });
  fs.mkdirSync(releases);
  fs.mkdirSync(generated, { recursive: true });
  const ids = mode === "ui1" ? ["webgpu", "webgl2", "webgl2-shared"] : ["webgpu", "webgl2"];
  const files = [];
  for (const id of ids) {
    const packageDir = path.join(source, `pkg-${id}`);
    fs.mkdirSync(packageDir);
    for (const name of names) {
      const bytes = Buffer.from(`${mode}:${id}:${name}`);
      fs.writeFileSync(path.join(packageDir, name), bytes);
      files.push({ path: `public/bevy-runtime/pkg-${id}/${name}`, sha256: hash(bytes) });
    }
  }
  const manifest = mode === "legacy" ? { version: "", files } : {
    schemaVersion: 2, version: "", packages: ids.map((id) => ({
      id, backend: id === "webgpu" ? "webgpu" : "webgl2", packageDir: `pkg-${id}`,
      questUiAbiVersion: mode === "ui1" ? 1 : 0,
      bagUiAbiVersion: mode === "ui1" ? 1 : 0,
      primarySharedUiCompiled: id === "webgl2-shared",
    })), files,
  };
  manifest.version = computeBevyRuntimeVersion(manifest);
  const manifestPath = path.join(generated, "bevy_runtime_version.json");
  fs.writeFileSync(manifestPath, JSON.stringify(manifest));
  const versionDirectory = prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir: source,
    manifest, releasesParentDir: releases, nonce: "fixture" });
  return { webRoot, source, releases, manifestPath, versionDirectory, manifest, cleanup() {
    if (path.dirname(webRoot) !== os.tmpdir() || !path.basename(webRoot).startsWith("mir2-release-files-test-")
        || fs.lstatSync(webRoot).isSymbolicLink()) throw new Error("refusing unexpected fixture cleanup");
    fs.rmSync(webRoot, { recursive: true, force: false });
  } };
}

function read(env, manifestPath) {
  return readImmutableBevyRuntimeRelease({ webRoot: env.webRoot, manifestPath });
}

test("legacy four-file, UI0 four-file and UI1 six-file releases resolve immutable URLs", () => {
  for (const mode of ["legacy", "ui0", "ui1"]) {
    const env = fixture(mode);
    try {
      const result = read(env);
      assert.deepEqual(result.manifest, env.manifest);
      assert.equal(result.versionDirectory, env.versionDirectory);
      assert.equal(result.normalized.schemaVersion, mode === "legacy" ? 1 : 2);
      assert.equal(result.files.length, mode === "ui1" ? 6 : 4);
      for (const file of result.files) {
        assert.equal(file.path, `/${file.relativePath}`);
        assert.match(file.relativePath, new RegExp(`^bevy-runtime/v/${env.manifest.version}/pkg-`));
        assert.ok(file.localPath.startsWith(`${env.versionDirectory}${path.sep}`));
        assert.equal(hash(fs.readFileSync(file.localPath)), file.sha256);
      }
    } finally { env.cleanup(); }
  }
});

test("flat source replacement after a read never changes the old immutable selection", () => {
  const env = fixture("ui1");
  try {
    const first = read(env);
    const original = first.files.map((file) => fs.readFileSync(file.localPath));
    fs.renameSync(env.source, path.join(env.webRoot, "retired-flat"));
    fs.mkdirSync(env.source);
    for (const file of env.manifest.files) {
      const suffix = file.path.slice("public/bevy-runtime/".length).split("/");
      fs.mkdirSync(path.join(env.source, suffix[0]), { recursive: true });
      fs.writeFileSync(path.join(env.source, ...suffix), "replacement bytes");
    }
    const second = read(env);
    assert.equal(second.versionDirectory, first.versionDirectory);
    for (let at = 0; at < second.files.length; at++) {
      assert.deepEqual(fs.readFileSync(second.files[at].localPath), original[at]);
    }
  } finally { env.cleanup(); }
});

test("wrong manifest version and corrupted immutable bytes fail closed", () => {
  const env = fixture("ui0");
  try {
    fs.writeFileSync(env.manifestPath, JSON.stringify({ ...env.manifest, version: "bevy-0000000000000000" }));
    assert.throws(() => read(env), /version does not match/);
    fs.writeFileSync(env.manifestPath, JSON.stringify(env.manifest));
    const artifact = path.join(env.versionDirectory, "pkg-webgl2", names[0]);
    fs.renameSync(artifact, `${artifact}.original`);
    fs.writeFileSync(artifact, "changed");
    assert.throws(() => read(env), /undeclared entries|hash mismatch/);
    fs.rmSync(`${artifact}.original`);
    assert.throws(() => read(env), /hash mismatch/);
  } finally { env.cleanup(); }
});

test("saved manifest capability mismatch, missing package pair, and extra files are rejected", () => {
  const env = fixture("ui1");
  try {
    const savedPath = path.join(env.versionDirectory, "runtime-manifest.json");
    const savedBytes = fs.readFileSync(savedPath);
    fs.writeFileSync(savedPath, JSON.stringify({ ...env.manifest,
      packages: env.manifest.packages.map((item) => item.id === "webgl2-shared"
        ? { ...item, primarySharedUiCompiled: false } : item) }));
    assert.throws(() => read(env));
    fs.writeFileSync(savedPath, savedBytes);
    const missing = path.join(env.versionDirectory, "pkg-webgl2-shared", names[1]);
    fs.renameSync(missing, path.join(env.webRoot, "held-wasm"));
    assert.throws(() => read(env), /missing or undeclared/);
    fs.renameSync(path.join(env.webRoot, "held-wasm"), missing);
    fs.writeFileSync(path.join(env.versionDirectory, "extra.js"), "unexpected");
    assert.throws(() => read(env), /missing or undeclared/);
  } finally { env.cleanup(); }
});

test("manifest override must stay absolute and inside webRoot", () => {
  const env = fixture("legacy");
  try {
    assert.equal(read(env, env.manifestPath).versionDirectory, env.versionDirectory);
    assert.throws(() => read(env, "lib/generated/bevy_runtime_version.json"), /absolute/);
    assert.throws(() => read(env, path.join(os.tmpdir(), "outside-manifest.json")), /inside webRoot/);
  } finally { env.cleanup(); }
});

test("immutable file and directory symlinks are rejected where host permits creation", (t) => {
  const env = fixture("ui1");
  try {
    const artifact = path.join(env.versionDirectory, "pkg-webgpu", names[0]);
    const real = path.join(env.webRoot, "held-artifact");
    fs.renameSync(artifact, real);
    try { fs.symlinkSync(real, artifact, "file"); }
    catch (error) {
      if (["EPERM", "EACCES", "ENOTSUP"].includes(error?.code)) { t.skip("symlink creation unavailable on this host"); return; }
      throw error;
    }
    assert.throws(() => read(env), /regular file/);
    fs.rmSync(artifact);
    fs.renameSync(real, artifact);
    const packageDirectory = path.join(env.versionDirectory, "pkg-webgpu");
    const realPackage = path.join(env.webRoot, "held-package");
    fs.renameSync(packageDirectory, realPackage);
    fs.symlinkSync(realPackage, packageDirectory, "dir");
    assert.throws(() => read(env), /symlink/);
    fs.rmSync(packageDirectory);
    fs.renameSync(realPackage, packageDirectory);
    const savedPath = path.join(env.versionDirectory, "runtime-manifest.json");
    const savedReal = path.join(env.webRoot, "held-manifest");
    fs.renameSync(savedPath, savedReal);
    fs.symlinkSync(savedReal, savedPath, "file");
    assert.throws(() => read(env), /regular file/);
    fs.rmSync(savedPath);
    fs.renameSync(savedReal, savedPath);
    const retiredVersion = path.join(env.webRoot, "held-version");
    fs.renameSync(env.versionDirectory, retiredVersion);
    fs.symlinkSync(retiredVersion, env.versionDirectory, "dir");
    assert.throws(() => read(env), /symlink/);
    fs.rmSync(env.versionDirectory);
    fs.renameSync(retiredVersion, env.versionDirectory);
    const link = path.join(env.webRoot, "public", "bevy-runtime-releases-link");
    fs.symlinkSync(env.releases, link, "dir");
    // Move the real releases dir away, then make its expected path a symlink.
    fs.rmSync(link);
    const retired = path.join(env.webRoot, "retired-releases");
    fs.renameSync(env.releases, retired);
    fs.symlinkSync(retired, env.releases, "dir");
    assert.throws(() => read(env), /symlink/);
  } finally { env.cleanup(); }
});
