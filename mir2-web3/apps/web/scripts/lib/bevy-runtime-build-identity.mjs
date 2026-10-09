import path from "node:path";
import { verifyBevyRuntimeVersion } from "./bevy-runtime-version.mjs";

export function resolveNextBuildDistDirectory(requiredServerFiles, appRoot) {
  if (typeof appRoot !== "string" || !path.isAbsolute(appRoot)) {
    throw new Error("Next standalone app root must be absolute");
  }
  const configuredDistDir = requiredServerFiles?.config?.distDir;
  const distDir = configuredDistDir === undefined ? ".next" : configuredDistDir;
  if (typeof distDir !== "string" || !distDir || distDir.includes("\0") || distDir.includes("\\") ||
      path.posix.isAbsolute(distDir) || path.win32.isAbsolute(distDir) || distDir.includes(":")) {
    throw new Error("Next build distDir must be a relative path inside the standalone app root");
  }
  const segments = distDir.split("/");
  if (segments.some((segment) => !segment || segment === "." || segment === "..")) {
    throw new Error("Next build distDir contains an invalid path segment");
  }
  const resolvedRoot = path.resolve(appRoot);
  const resolvedDistDir = path.resolve(resolvedRoot, distDir);
  const relative = path.relative(resolvedRoot, resolvedDistDir);
  if (!relative || relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error("Next build distDir escapes the standalone app root");
  }
  return resolvedDistDir;
}

function canonical(manifest) {
  const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
  return JSON.stringify({
    schemaVersion: manifest.schemaVersion, version: manifest.version,
    packages: [...manifest.packages].sort((a, b) => compare(a.id, b.id)),
    files: [...manifest.files].sort((a, b) => compare(a.path, b.path)),
  });
}

export function assertCompiledBevyRuntimeManifest(requiredServerFiles, expectedManifest) {
  const serialized = requiredServerFiles?.config?.env?.MIR2_BEVY_RUNTIME_BUILD_MANIFEST;
  if (typeof serialized !== "string" || !serialized) {
    throw new Error("Next build has no compiled runtime manifest identity; rebuild before thin packaging");
  }
  let raw;
  try { raw = JSON.parse(serialized); }
  catch { throw new Error("Next compiled runtime manifest identity is invalid JSON"); }
  const compiled = verifyBevyRuntimeVersion(raw, "Next compiled runtime manifest");
  const expected = verifyBevyRuntimeVersion(expectedManifest, "thin runtime manifest");
  if (canonical(compiled) !== canonical(expected)) {
    throw new Error("Next compiled runtime manifest differs from the pinned thin runtime; rebuild before packaging");
  }
  return compiled;
}
