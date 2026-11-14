import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { verifyBevyRuntimeVersion } from "./bevy-runtime-version.mjs";

const FLAT_PREFIX = "public/bevy-runtime/";
const ARTIFACTS = ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"];

function absoluteDirectory(value, label) {
  if (typeof value !== "string" || !path.isAbsolute(value)) {
    throw new Error(`${label} must be an absolute directory`);
  }
  const resolved = path.resolve(value);
  const root = path.parse(resolved).root;
  const rootStat = fs.lstatSync(root);
  if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) throw new Error(`${label} has a symlink root`);
  let cursor = root;
  for (const part of path.relative(root, resolved).split(path.sep).filter(Boolean)) {
    cursor = path.join(cursor, part);
    const stat = fs.lstatSync(cursor);
    if (stat.isSymbolicLink() || !stat.isDirectory()) {
      throw new Error(`${label} has a symlink or non-directory component`);
    }
  }
  return resolved;
}

function inside(parent, target) {
  const relative = path.relative(parent, target);
  return relative && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

function childPath(parent, ...parts) {
  const target = path.resolve(parent, ...parts);
  if (!inside(parent, target)) throw new Error("runtime release path escapes its parent");
  return target;
}

function regularFile(file, label) {
  const stat = fs.lstatSync(file);
  if (stat.isSymbolicLink() || !stat.isFile()) throw new Error(`${label} is not a regular file`);
}

function readJsonFile(file, label) {
  absoluteDirectory(path.dirname(file), `${label} parent`);
  regularFile(file, label);
  try { return JSON.parse(fs.readFileSync(file, "utf8")); }
  catch { throw new Error(`${label} is invalid JSON`); }
}

function expectedEntries(directory, names, label) {
  const actual = fs.readdirSync(directory).sort();
  const expected = [...names].sort();
  if (actual.length !== expected.length || actual.some((name, index) => name !== expected[index])) {
    throw new Error(`${label} has missing or undeclared entries`);
  }
}

function canonical(normalized) {
  const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
  return JSON.stringify({
    schemaVersion: normalized.schemaVersion,
    version: normalized.version,
    packages: [...normalized.packages].sort((a, b) => compare(a.id, b.id)),
    files: [...normalized.files].sort((a, b) => compare(a.path, b.path)),
  });
}

/** Read one verified immutable release without touching the flat current tree. */
export function readImmutableBevyRuntimeRelease({ webRoot, manifestPath } = {}) {
  const root = absoluteDirectory(webRoot, "webRoot");
  const selectedManifestPath = manifestPath === undefined
    ? childPath(root, "lib", "generated", "bevy_runtime_version.json")
    : manifestPath;
  if (typeof selectedManifestPath !== "string" || !path.isAbsolute(selectedManifestPath)
      || !inside(root, path.resolve(selectedManifestPath))) {
    throw new Error("manifestPath must be an absolute path inside webRoot");
  }
  const manifest = readJsonFile(path.resolve(selectedManifestPath), "runtime manifest");
  const normalized = verifyBevyRuntimeVersion(manifest, "runtime manifest");

  const releaseParent = childPath(root, "public", "bevy-runtime-releases");
  absoluteDirectory(releaseParent, "runtime releases parent");
  const versionDirectory = childPath(releaseParent, normalized.version);
  absoluteDirectory(versionDirectory, "runtime version directory");
  expectedEntries(versionDirectory,
    ["runtime-manifest.json", ...normalized.packages.map((item) => item.packageDir)],
    "runtime version directory");
  const saved = readJsonFile(childPath(versionDirectory, "runtime-manifest.json"), "saved runtime manifest");
  const savedNormalized = verifyBevyRuntimeVersion(saved, "saved runtime manifest");
  if (canonical(savedNormalized) !== canonical(normalized)) {
    throw new Error("saved runtime manifest differs from requested version or capabilities");
  }

  for (const { packageDir } of normalized.packages) {
    const packagePath = childPath(versionDirectory, packageDir);
    absoluteDirectory(packagePath, "runtime package directory");
    expectedEntries(packagePath, ARTIFACTS, "runtime package directory");
  }

  const files = normalized.files.map(({ path: flatPath, sha256 }) => {
    if (!flatPath.startsWith(FLAT_PREFIX)) throw new Error("unexpected runtime artifact path");
    const suffix = flatPath.slice(FLAT_PREFIX.length);
    const localPath = childPath(versionDirectory, ...suffix.split("/"));
    absoluteDirectory(path.dirname(localPath), "runtime artifact parent");
    regularFile(localPath, "runtime artifact");
    const actualHash = crypto.createHash("sha256").update(fs.readFileSync(localPath)).digest("hex");
    if (actualHash !== sha256) throw new Error(`runtime artifact hash mismatch: ${flatPath}`);
    const relativePath = `bevy-runtime/v/${normalized.version}/${suffix}`;
    return Object.freeze({ path: `/${relativePath}`, relativePath, localPath, sha256 });
  });
  return Object.freeze({ manifest, normalized, versionDirectory, files: Object.freeze(files) });
}
