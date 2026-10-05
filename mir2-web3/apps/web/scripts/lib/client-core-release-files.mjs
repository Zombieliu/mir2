import fs from "node:fs";
import path from "node:path";
import { createHash } from "node:crypto";

export const CLIENT_CORE_BUILD_MANIFEST_ENV = "MIR2_CLIENT_CORE_BUILD_MANIFEST";
const NAMES = ["mir2_platform_web.js", "mir2_platform_web_bg.wasm"];
const HEX = /^[a-f0-9]{64}$/;
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
function exactKeys(value, keys, label) {
  if (!value || typeof value !== "object" || Array.isArray(value) ||
      Object.keys(value).sort().join("|") !== [...keys].sort().join("|")) {
    throw new Error(label + " has missing or undeclared fields");
  }
}
export function verifyClientCoreManifest(manifest) {
  exactKeys(manifest, ["abiVersion", "version", "sourceSha256", "files"], "Core manifest");
  if (manifest.abiVersion !== 1 || typeof manifest.version !== "string" || !HEX.test(manifest.version) ||
      typeof manifest.sourceSha256 !== "string" || !HEX.test(manifest.sourceSha256)) {
    throw new Error("Invalid Core ABI/version/source identity");
  }
  exactKeys(manifest.files, NAMES, "Core files");
  const files = {};
  for (const name of NAMES) {
    const entry = manifest.files[name];
    exactKeys(entry, ["bytes", "sha256"], "Core file " + name);
    if (!Number.isSafeInteger(entry.bytes) || entry.bytes <= 0 ||
        typeof entry.sha256 !== "string" || !HEX.test(entry.sha256)) {
      throw new Error("Invalid Core file size/hash: " + name);
    }
    if (name.endsWith(".wasm") ? entry.bytes >= 262144 : entry.bytes > 204800) {
      throw new Error("Core file exceeds its byte budget: " + name);
    }
    files[name] = Object.freeze({ bytes: entry.bytes, sha256: entry.sha256 });
  }
  return Object.freeze({ abiVersion: 1, version: manifest.version,
    sourceSha256: manifest.sourceSha256, files: Object.freeze(files) });
}
export function assertCompiledClientCoreManifest(requiredServerFiles, expectedManifest) {
  const serialized = requiredServerFiles?.config?.env?.[CLIENT_CORE_BUILD_MANIFEST_ENV];
  if (typeof serialized !== "string" || !serialized) {
    throw new Error("Next build has no compiled Core manifest identity; rebuild before packaging");
  }
  let parsed;
  try { parsed = JSON.parse(serialized); }
  catch { throw new Error("Next compiled Core manifest identity is invalid JSON"); }
  const compiled = verifyClientCoreManifest(parsed);
  const expected = verifyClientCoreManifest(expectedManifest);
  if (JSON.stringify(compiled) !== JSON.stringify(expected)) {
    throw new Error("Next compiled Core manifest differs from current Core; rebuild before packaging");
  }
  return compiled;
}
function directory(candidate) {
  if (typeof candidate !== "string" || !path.isAbsolute(candidate)) throw new Error("Core root must be absolute");
  const resolved = path.resolve(candidate);
  let cursor = path.parse(resolved).root;
  for (const part of ["", ...path.relative(cursor, resolved).split(path.sep).filter(Boolean)]) {
    cursor = part ? path.join(cursor, part) : cursor;
    const stat = fs.lstatSync(cursor);
    if (stat.isSymbolicLink() || !stat.isDirectory()) throw new Error("Core directory is linked or irregular: " + cursor);
  }
  return resolved;
}
function regularBytes(candidate) {
  directory(path.dirname(candidate));
  const stat = fs.lstatSync(candidate);
  if (stat.isSymbolicLink() || !stat.isFile()) throw new Error("Core file is linked or irregular: " + candidate);
  return fs.readFileSync(candidate);
}
function entries(candidate, expected) {
  const actual = fs.readdirSync(candidate).sort();
  if (JSON.stringify(actual) !== JSON.stringify([...expected].sort())) {
    throw new Error("Packaged Core has missing or undeclared entries: " + candidate);
  }
}
/** Reads only declared leaves; historical/source extra files remain untouched. */
export function readClientCoreRelease({ webRoot, manifest, requireExactClosure = false } = {}) {
  const root = directory(webRoot);
  const normalized = verifyClientCoreManifest(manifest ?? JSON.parse(regularBytes(
    path.join(root, "lib", "generated", "client_core_runtime.json")).toString("utf8")));
  const parent = directory(path.join(root, "public", "client-core"));
  const versionDirectory = directory(path.join(parent, normalized.version));
  if (requireExactClosure) { entries(parent, [normalized.version]); entries(versionDirectory, NAMES); }
  const artifacts = [];
  const files = NAMES.map((name) => {
    const localPath = path.join(versionDirectory, name);
    const bytes = regularBytes(localPath);
    const expected = normalized.files[name];
    if (bytes.length !== expected.bytes || hash(bytes) !== expected.sha256) {
      throw new Error("Core artifact size/hash mismatch: " + name);
    }
    artifacts.push(bytes);
    return Object.freeze({ name, relativePath: "client-core/" + normalized.version + "/" + name,
      localPath, bytes: bytes.length, sha256: expected.sha256 });
  });
  if (hash(Buffer.concat(artifacts)) !== normalized.version) throw new Error("Core bundle hash differs from immutable version");
  return Object.freeze({ manifest: normalized, versionDirectory, files: Object.freeze(files) });
}
