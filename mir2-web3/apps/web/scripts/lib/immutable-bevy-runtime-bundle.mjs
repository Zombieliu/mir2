import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { verifyBevyRuntimeVersion } from "./bevy-runtime-version.mjs";

const FILE_PREFIX = "public/bevy-runtime/";
const ARTIFACTS = ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"];
const COPY_FALLBACK_CODES = new Set(["EXDEV", "EPERM", "ENOTSUP"]);
const NONCE = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/;

function absoluteDirectory(value, label) {
  if (typeof value !== "string" || !path.isAbsolute(value)) {
    throw new Error(`${label} must be an absolute directory`);
  }
  const resolved = path.resolve(value);
  const root = path.parse(resolved).root;
  let cursor = root;
  const segments = path.relative(root, resolved).split(path.sep).filter(Boolean);
  for (const segment of segments) {
    cursor = path.join(cursor, segment);
    const stat = fs.lstatSync(cursor);
    if (stat.isSymbolicLink() || !stat.isDirectory()) {
      throw new Error(`${label} has a symlink or non-directory component`);
    }
  }
  const rootStat = fs.lstatSync(root);
  if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
    throw new Error(`${label} has a symlink root`);
  }
  return resolved;
}

function childPath(parent, ...parts) {
  const target = path.resolve(parent, ...parts);
  const relative = path.relative(parent, target);
  if (!relative || relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error("runtime bundle path escapes its parent");
  }
  return target;
}

function regularFile(file, label) {
  const stat = fs.lstatSync(file);
  if (stat.isSymbolicLink() || !stat.isFile()) throw new Error(`${label} is not a regular file`);
}

function pathExists(file) {
  try { fs.lstatSync(file); return true; }
  catch (error) { if (error?.code === "ENOENT") return false; throw error; }
}

function hashFile(file) {
  return crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");
}

function artifactRelativePath(file) {
  if (!file.path.startsWith(FILE_PREFIX)) throw new Error("unexpected runtime artifact path");
  return file.path.slice(FILE_PREFIX.length).split("/");
}

function canonicalManifest(manifest) {
  const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
  return JSON.stringify({
    schemaVersion: manifest.schemaVersion,
    version: manifest.version,
    packages: [...manifest.packages].sort((a, b) => compare(a.id, b.id)),
    files: [...manifest.files].sort((a, b) => compare(a.path, b.path)),
  });
}

function exactEntries(directory, expected, label) {
  const actual = fs.readdirSync(directory).sort();
  if (actual.length !== expected.length || actual.some((entry, index) => entry !== [...expected].sort()[index])) {
    throw new Error(`${label} has missing or undeclared entries`);
  }
}

function verifyBundleDirectory(directory, expected) {
  absoluteDirectory(directory, "runtime version directory");
  exactEntries(directory, ["runtime-manifest.json", ...expected.packages.map((item) => item.packageDir)], "runtime version directory");
  const manifestFile = childPath(directory, "runtime-manifest.json");
  regularFile(manifestFile, "saved runtime manifest");
  let saved;
  try { saved = JSON.parse(fs.readFileSync(manifestFile, "utf8")); }
  catch { throw new Error("saved runtime manifest is unreadable"); }
  const savedNormalized = verifyBevyRuntimeVersion(saved, "saved runtime manifest");
  if (canonicalManifest(savedNormalized) !== canonicalManifest(expected)) {
    throw new Error("saved runtime manifest differs from requested capabilities or files");
  }
  for (const { packageDir } of expected.packages) {
    const packagePath = childPath(directory, packageDir);
    absoluteDirectory(packagePath, "runtime package directory");
    exactEntries(packagePath, ARTIFACTS, "runtime package directory");
  }
  for (const file of expected.files) {
    const target = childPath(directory, ...artifactRelativePath(file));
    regularFile(target, "runtime artifact");
    if (hashFile(target) !== file.sha256) throw new Error(`runtime artifact hash mismatch: ${file.path}`);
  }
}

function safeCleanupStaging(stage, parent, expectedName) {
  if (path.dirname(stage) !== parent || path.basename(stage) !== expectedName) {
    throw new Error("refusing to clean staging outside releases parent");
  }
  absoluteDirectory(parent, "releases parent");
  if (!pathExists(stage)) return;
  absoluteDirectory(stage, "runtime staging directory");
  fs.rmSync(stage, { recursive: true, force: false });
}

/**
 * Prepare one immutable version before the caller swaps its flat current tree.
 * The caller holds the publication lock and must replace/remove flat source
 * files by rename: in-place writes would also alter hardlinked immutable bytes.
 */
export function prepareImmutableBevyRuntimeBundle({ sourcePkgParentDir, manifest, releasesParentDir, nonce }) {
  const normalized = verifyBevyRuntimeVersion(manifest, "runtime manifest");
  if (typeof nonce !== "string" || !NONCE.test(nonce)) throw new Error("invalid runtime bundle nonce");
  const source = absoluteDirectory(sourcePkgParentDir, "source package parent");
  const releases = absoluteDirectory(releasesParentDir, "releases parent");
  const sourceToReleases = path.relative(source, releases);
  const releasesToSource = path.relative(releases, source);
  const nested = (relative) => relative && relative !== ".."
    && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
  if (!sourceToReleases || !releasesToSource
      || nested(sourceToReleases) || nested(releasesToSource)) {
    throw new Error("source and releases roots must be disjoint");
  }
  const versionDirectory = childPath(releases, normalized.version);
  // Verify every supplied source byte even when the immutable version exists.
  for (const { packageDir } of normalized.packages) {
    const packagePath = childPath(source, packageDir);
    absoluteDirectory(packagePath, "source package directory");
  }
  for (const file of normalized.files) {
    const sourceFile = childPath(source, ...artifactRelativePath(file));
    regularFile(sourceFile, "source runtime artifact");
    if (hashFile(sourceFile) !== file.sha256) throw new Error(`source runtime hash mismatch: ${file.path}`);
  }
  if (pathExists(versionDirectory)) {
    verifyBundleDirectory(versionDirectory, normalized);
    return versionDirectory;
  }

  const stageName = `.${normalized.version}.${nonce}.staging`;
  const stage = childPath(releases, stageName);
  if (pathExists(stage)) throw new Error("runtime staging directory already exists");
  fs.mkdirSync(stage);
  let staged = true;
  try {
    for (const { packageDir } of normalized.packages) fs.mkdirSync(childPath(stage, packageDir));
    for (const file of normalized.files) {
      const from = childPath(source, ...artifactRelativePath(file));
      const to = childPath(stage, ...artifactRelativePath(file));
      try { fs.linkSync(from, to); }
      catch (error) {
        if (!COPY_FALLBACK_CODES.has(error?.code)) throw error;
        fs.copyFileSync(from, to, fs.constants.COPYFILE_EXCL);
      }
    }
    fs.writeFileSync(childPath(stage, "runtime-manifest.json"), `${JSON.stringify(manifest, null, 2)}\n`, { flag: "wx" });
    verifyBundleDirectory(stage, normalized);
    if (pathExists(versionDirectory)) {
      verifyBundleDirectory(versionDirectory, normalized);
      return versionDirectory;
    }
    fs.renameSync(stage, versionDirectory);
    staged = false;
    return versionDirectory;
  } finally {
    if (staged) safeCleanupStaging(stage, releases, stageName);
  }
}
