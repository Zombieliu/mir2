const IDS = ["webgpu", "webgl2", "webgl2-shared"];
const ARTIFACTS = ["mir2_bevy_runtime.js", "mir2_bevy_runtime_bg.wasm"];
const VERSION = /^bevy-[0-9a-f]{16}$/;
const HASH = /^[0-9a-f]{64}$/;

function record(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function keysAre(value, expected) {
  const actual = Object.keys(value);
  return actual.length === expected.length && actual.every((key) => expected.includes(key));
}

function invalid(label, detail) {
  throw new TypeError(`${label}: ${detail}`);
}

function packageFor(id, questUiAbiVersion, bagUiAbiVersion, primarySharedUiCompiled) {
  return Object.freeze({
    id,
    backend: id === "webgpu" ? "webgpu" : "webgl2",
    packageDir: `pkg-${id}`,
    questUiAbiVersion,
    bagUiAbiVersion,
    primarySharedUiCompiled,
  });
}

/** Validate untrusted release metadata without importing runtime bytes. */
export function validateBevyRuntimeManifest(manifest, label = "Bevy runtime manifest") {
  if (!record(manifest)) invalid(label, "expected an object");
  const legacy = !Object.hasOwn(manifest, "schemaVersion") && !Object.hasOwn(manifest, "packages");
  if (!keysAre(manifest, legacy ? ["version", "files"] : ["schemaVersion", "version", "packages", "files"])) {
    invalid(label, "unexpected or missing manifest fields");
  }
  if (!legacy && manifest.schemaVersion !== 2) invalid(label, "unsupported schemaVersion");
  if (typeof manifest.version !== "string" || !VERSION.test(manifest.version)) invalid(label, "invalid version");

  let packages;
  if (legacy) {
    packages = [packageFor("webgpu", null, null, null), packageFor("webgl2", null, null, null)];
  } else {
    if (!Array.isArray(manifest.packages) || manifest.packages.length < 2 || manifest.packages.length > 3) {
      invalid(label, "expected two or three packages");
    }
    const found = new Map();
    for (const candidate of manifest.packages) {
      if (!record(candidate) || !keysAre(candidate, ["id", "backend", "packageDir", "questUiAbiVersion", "bagUiAbiVersion", "primarySharedUiCompiled"])) {
        invalid(label, "invalid package record");
      }
      const id = candidate.id;
      if (!IDS.includes(id) || found.has(id)) invalid(label, "unknown or duplicate package id");
      const expected = packageFor(id, candidate.questUiAbiVersion, candidate.bagUiAbiVersion, candidate.primarySharedUiCompiled);
      if (candidate.backend !== expected.backend || candidate.packageDir !== expected.packageDir) {
        invalid(label, "package backend or directory mismatch");
      }
      if (![0, 1].includes(candidate.questUiAbiVersion) || ![0, 1].includes(candidate.bagUiAbiVersion)
          || candidate.questUiAbiVersion !== candidate.bagUiAbiVersion
          || typeof candidate.primarySharedUiCompiled !== "boolean") {
        invalid(label, "invalid package UI capabilities");
      }
      if (candidate.primarySharedUiCompiled !== (id === "webgl2-shared")) {
        invalid(label, "invalid compiled primary capability");
      }
      if (id === "webgl2-shared" && candidate.questUiAbiVersion !== 1) {
        invalid(label, "shared GL2 requires ABI1");
      }
      found.set(id, expected);
    }
    if (!found.has("webgpu") || !found.has("webgl2")) invalid(label, "missing required package");
    const gpuAbi = found.get("webgpu").questUiAbiVersion;
    const leanAbi = found.get("webgl2").questUiAbiVersion;
    if (gpuAbi !== leanAbi && !(found.has("webgl2-shared") && gpuAbi === 1 && leanAbi === 0)) {
      invalid(label, "unsupported GPU and lean GL2 UI ABI combination");
    }
    if (found.has("webgl2-shared") && gpuAbi !== 1) {
      invalid(label, "shared GL2 requires GPU ABI1");
    }
    packages = IDS.filter((id) => found.has(id)).map((id) => found.get(id));
  }

  if (!Array.isArray(manifest.files) || manifest.files.length !== packages.length * ARTIFACTS.length) {
    invalid(label, "incomplete package files");
  }
  const expectedPaths = new Set(packages.flatMap(({ packageDir }) => ARTIFACTS.map(
    (artifact) => `public/bevy-runtime/${packageDir}/${artifact}`,
  )));
  const seenPaths = new Set();
  const files = manifest.files.map((file) => {
    if (!record(file) || !keysAre(file, ["path", "sha256"])
        || typeof file.path !== "string" || !expectedPaths.has(file.path)
        || seenPaths.has(file.path) || typeof file.sha256 !== "string" || !HASH.test(file.sha256)) {
      invalid(label, "unknown, duplicate, or malformed package file");
    }
    seenPaths.add(file.path);
    return Object.freeze({ path: file.path, sha256: file.sha256 });
  });
  if (seenPaths.size !== expectedPaths.size) invalid(label, "missing package file");
  return Object.freeze({
    schemaVersion: legacy ? 1 : 2,
    version: manifest.version,
    packages: Object.freeze(packages),
    files: Object.freeze(files),
  });
}
