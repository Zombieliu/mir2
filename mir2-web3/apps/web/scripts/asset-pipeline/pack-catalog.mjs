import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const PACK_SCHEMA_VERSION = 1;
export const PACK_CATALOG_SCHEMA_VERSION = 1;
export const PACK_RELEASE_SCHEMA_VERSION = 1;
export const PACK_CATALOG_BUNDLE_SCHEMA_VERSION = 1;

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const REPO_ROOT = path.resolve(import.meta.dirname, "..", "..", "..", "..");
const DEFAULT_INPUT_PATH = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "assets",
  "crystal-source-snapshot.generated.json",
);
const DEFAULT_OUTPUT_PATH = path.join(
  REPO_ROOT,
  "docs",
  "generated",
  "assets",
  "crystal-pack-catalog.generated.json",
);

const CATEGORY_ROOTS = new Map(
  Object.entries({
    characters: [
      "AArmour", "AHair", "AHumEffect", "ARArmour", "ARHair", "ARHumEffect",
      "ARWeapon", "AWeapon", "CArmour", "CHair", "CHelmet", "CHumEffect",
      "CWeapon", "CWeaponEffect",
    ],
    effects: ["Effect.Lib", "Effect2.Lib", "Magic.Lib", "Magic2.Lib", "Magic3.Lib", "MagicC.Lib"],
    entities: [
      "Dragon.Lib", "Fishing", "Monster", "Mount", "NPC", "Pet", "Transform",
      "TransformEffect", "TransformRide2", "TransformRide2Effect", "TransformWeaponEffect",
    ],
    items: ["dnitems.Lib", "Items.Lib", "Stateitem.Lib"],
    maps: ["Background.Lib", "Deco.Lib", "Extra", "Flag", "Gate", "Map", "mmap.Lib", "Siege", "Weather.Lib"],
  }).flatMap(([category, roots]) => roots.map((root) => [root.toLowerCase(), category])),
);

if (process.argv[1] && path.resolve(process.argv[1]) === SCRIPT_PATH) {
  runPackCatalogCli().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

export async function runPackCatalogCli(argv = process.argv.slice(2)) {
  const args = parseArgs(argv);
  const inputPath = path.resolve(args.input ?? args._[0] ?? DEFAULT_INPUT_PATH);
  const outputPath = path.resolve(args.output ?? args._[1] ?? DEFAULT_OUTPUT_PATH);
  const snapshot = JSON.parse(await readFile(inputPath, "utf8"));
  const bundle = compilePackCatalog(snapshot);
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${canonicalJson(bundle)}\n`, "utf8");
  console.log(JSON.stringify({ outputPath, releaseHash: bundle.release.contentHash, ...bundle.catalog.summary }, null, 2));
  return bundle;
}

export function compilePackCatalog(snapshot) {
  validateSourceSnapshot(snapshot);

  const groups = new Map();
  for (const library of [...snapshot.libraries].sort(compareLibraryPaths)) {
    const category = categoryForLibraryPath(library.path);
    if (!groups.has(category)) groups.set(category, []);
    groups.get(category).push(deepCanonicalCopy(library));
  }

  const packs = [...groups]
    .sort(([left], [right]) => compareCodePoints(left, right))
    .map(([category, libraries]) => withContentHash({
      schemaVersion: PACK_SCHEMA_VERSION,
      id: `crystal-${category}`,
      category,
      dependencies: [],
      summary: summarizePack(libraries),
      libraries,
    }));

  const catalog = withContentHash({
    schemaVersion: PACK_CATALOG_SCHEMA_VERSION,
    id: "crystal-client-data",
    source: {
      kind: snapshot.sourceKind,
      layout: snapshot.sourceLayout,
      snapshotSchemaVersion: snapshot.schemaVersion,
      contentHash: semanticSnapshotHash(snapshot),
    },
    dependencies: packs.map((pack) => ({
      kind: "pack",
      id: pack.id,
      schemaVersion: pack.schemaVersion,
      contentHash: pack.contentHash,
    })),
    summary: summarizeCatalog(packs),
    packs: packs.map((pack) => ({
      id: pack.id,
      category: pack.category,
      schemaVersion: pack.schemaVersion,
      contentHash: pack.contentHash,
      dependencies: pack.dependencies,
      summary: pack.summary,
    })),
  });

  const release = withContentHash({
    schemaVersion: PACK_RELEASE_SCHEMA_VERSION,
    id: "crystal-client-data",
    dependencies: [{
      kind: "catalog",
      id: catalog.id,
      schemaVersion: catalog.schemaVersion,
      contentHash: catalog.contentHash,
    }],
    catalog: {
      id: catalog.id,
      schemaVersion: catalog.schemaVersion,
      contentHash: catalog.contentHash,
    },
  });

  const bundle = {
    schemaVersion: PACK_CATALOG_BUNDLE_SCHEMA_VERSION,
    release,
    catalog,
    packs,
  };
  validatePackCatalogBundle(bundle);
  return bundle;
}

export function validateSourceSnapshot(snapshot) {
  assertRecord(snapshot, "snapshot");
  assertEqual(snapshot.schemaVersion, 1, "snapshot.schemaVersion");
  assertNonEmptyString(snapshot.sourceKind, "snapshot.sourceKind");
  assertNonEmptyString(snapshot.sourceLayout, "snapshot.sourceLayout");
  assertEqual(snapshot.hashAlgorithm, "sha256", "snapshot.hashAlgorithm");
  assertHash(snapshot.contentHash, "snapshot.contentHash");
  assertRecord(snapshot.summary, "snapshot.summary");
  if (!Array.isArray(snapshot.libraries) || snapshot.libraries.length === 0) {
    throw new Error("snapshot.libraries must be a non-empty array");
  }

  const paths = new Set();
  for (const [index, library] of snapshot.libraries.entries()) {
    validateLibrary(library, `snapshot.libraries[${index}]`);
    if (paths.has(library.path)) throw new Error(`Duplicate library path: ${library.path}`);
    paths.add(library.path);
  }
  validateSnapshotSummary(snapshot.summary, snapshot.libraries);

  const body = { ...snapshot };
  delete body.contentHash;
  const expectedHash = sha256(JSON.stringify(body, null, 2));
  if (snapshot.contentHash !== expectedHash) {
    throw new Error(`snapshot.contentHash mismatch: expected ${expectedHash}, received ${snapshot.contentHash}`);
  }
  return true;
}

export function validatePackCatalogBundle(bundle) {
  assertRecord(bundle, "bundle");
  assertEqual(bundle.schemaVersion, PACK_CATALOG_BUNDLE_SCHEMA_VERSION, "bundle.schemaVersion");
  if (!Array.isArray(bundle.packs) || bundle.packs.length === 0) throw new Error("bundle.packs must be non-empty");
  assertRecord(bundle.catalog, "bundle.catalog");
  assertRecord(bundle.release, "bundle.release");

  const packById = new Map();
  for (const pack of bundle.packs) {
    assertEqual(pack.schemaVersion, PACK_SCHEMA_VERSION, `${pack.id}.schemaVersion`);
    validateHashedObject(pack, `pack ${pack.id}`);
    if (packById.has(pack.id)) throw new Error(`Duplicate pack id: ${pack.id}`);
    packById.set(pack.id, pack);
  }
  validateHashedObject(bundle.catalog, "catalog");
  validateHashedObject(bundle.release, "release");

  assertEqual(bundle.catalog.schemaVersion, PACK_CATALOG_SCHEMA_VERSION, "catalog.schemaVersion");
  if (!Array.isArray(bundle.catalog.dependencies)) throw new Error("catalog.dependencies must be an array");
  for (const dependency of bundle.catalog.dependencies) {
    const pack = packById.get(dependency.id);
    if (!pack || dependency.kind !== "pack" || dependency.contentHash !== pack.contentHash) {
      throw new Error(`Invalid catalog pack dependency: ${dependency.id}`);
    }
  }
  if (bundle.catalog.dependencies.length !== packById.size) {
    throw new Error("catalog.dependencies must reference every pack exactly once");
  }

  assertEqual(bundle.release.schemaVersion, PACK_RELEASE_SCHEMA_VERSION, "release.schemaVersion");
  const catalogDependency = bundle.release.dependencies?.[0];
  if (
    bundle.release.dependencies?.length !== 1 ||
    catalogDependency.kind !== "catalog" ||
    catalogDependency.id !== bundle.catalog.id ||
    catalogDependency.contentHash !== bundle.catalog.contentHash ||
    bundle.release.catalog?.contentHash !== bundle.catalog.contentHash
  ) {
    throw new Error("release catalog dependency does not match catalog content hash");
  }
  return true;
}

export function computeSourceSnapshotContentHash(snapshotBody) {
  const body = { ...snapshotBody };
  delete body.contentHash;
  return sha256(JSON.stringify(body, null, 2));
}

export function semanticSnapshotHash(snapshot) {
  const body = { ...snapshot, libraries: [...snapshot.libraries].sort(compareLibraryPaths) };
  delete body.contentHash;
  return sha256(canonicalJson(body));
}

export function canonicalJson(value) {
  return JSON.stringify(canonicalize(value), null, 2);
}

export function semanticContentHash(value) {
  const body = { ...value };
  delete body.contentHash;
  return sha256(canonicalJson(body));
}

export function categoryForLibraryPath(libraryPath) {
  const root = libraryPath.split("/")[0].toLowerCase();
  return CATEGORY_ROOTS.get(root) ?? "ui";
}

function validateLibrary(library, label) {
  assertRecord(library, label);
  if (library.status !== "ok") throw new Error(`${label}.status must be \"ok\"`);
  assertRelativeLibraryPath(library.path, `${label}.path`);
  assertInteger(library.byteLength, `${label}.byteLength`, 0);
  assertHash(library.sha256, `${label}.sha256`);
  assertInteger(library.version, `${label}.version`, 2);
  for (const field of ["frameSlotCount", "presentFrameCount", "emptyFrameCount", "invalidFrameOffsetCount"]) {
    assertInteger(library[field], `${label}.${field}`, 0);
  }
  if (library.presentFrameCount + library.emptyFrameCount + library.invalidFrameOffsetCount !== library.frameSlotCount) {
    throw new Error(`${label} frame counts do not add up to frameSlotCount`);
  }
  assertRecord(library.frameSet, `${label}.frameSet`);
  assertInteger(library.frameSet.count, `${label}.frameSet.count`, 0);
  if (!Array.isArray(library.frameSet.actions) || library.frameSet.actions.length !== library.frameSet.count) {
    throw new Error(`${label}.frameSet.actions length must equal frameSet.count`);
  }
  for (const [index, action] of library.frameSet.actions.entries()) {
    validateAction(action, `${label}.frameSet.actions[${index}]`);
  }
  if (!Array.isArray(library.issues)) throw new Error(`${label}.issues must be an array`);
}

function validateAction(action, label) {
  assertRecord(action, label);
  for (const field of [
    "actionId", "start", "count", "skip", "interval", "effectStart", "effectCount", "effectSkip", "effectInterval",
  ]) assertInteger(action[field], `${label}.${field}`);
  if (action.actionName !== null) assertNonEmptyString(action.actionName, `${label}.actionName`);
  if (typeof action.reverse !== "boolean" || typeof action.blend !== "boolean") {
    throw new Error(`${label}.reverse and .blend must be booleans`);
  }
}

function validateSnapshotSummary(summary, libraries) {
  const expected = {
    libraryCount: libraries.length,
    parsedLibraryCount: libraries.length,
    failedLibraryCount: 0,
    sourceBytes: sum(libraries, "byteLength"),
    frameSlotCount: sum(libraries, "frameSlotCount"),
    presentFrameCount: sum(libraries, "presentFrameCount"),
    emptyFrameCount: sum(libraries, "emptyFrameCount"),
    invalidFrameOffsetCount: sum(libraries, "invalidFrameOffsetCount"),
    frameSetLibraryCount: libraries.filter((library) => library.frameSet.count > 0).length,
    actionCount: libraries.reduce((total, library) => total + library.frameSet.count, 0),
  };
  for (const [field, value] of Object.entries(expected)) assertEqual(summary[field], value, `snapshot.summary.${field}`);
}

function summarizePack(libraries) {
  return {
    libraryCount: libraries.length,
    sourceBytes: sum(libraries, "byteLength"),
    frameSlotCount: sum(libraries, "frameSlotCount"),
    presentFrameCount: sum(libraries, "presentFrameCount"),
    emptyFrameCount: sum(libraries, "emptyFrameCount"),
    actionCount: libraries.reduce((total, library) => total + library.frameSet.count, 0),
  };
}

function summarizeCatalog(packs) {
  return {
    packCount: packs.length,
    libraryCount: packs.reduce((total, pack) => total + pack.summary.libraryCount, 0),
    sourceBytes: packs.reduce((total, pack) => total + pack.summary.sourceBytes, 0),
    frameSlotCount: packs.reduce((total, pack) => total + pack.summary.frameSlotCount, 0),
    presentFrameCount: packs.reduce((total, pack) => total + pack.summary.presentFrameCount, 0),
    emptyFrameCount: packs.reduce((total, pack) => total + pack.summary.emptyFrameCount, 0),
    actionCount: packs.reduce((total, pack) => total + pack.summary.actionCount, 0),
  };
}

function withContentHash(value) {
  return { ...value, contentHash: semanticContentHash(value) };
}

function validateHashedObject(value, label) {
  assertHash(value.contentHash, `${label}.contentHash`);
  const expected = semanticContentHash(value);
  if (value.contentHash !== expected) {
    throw new Error(`${label}.contentHash mismatch: expected ${expected}, received ${value.contentHash}`);
  }
}

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value).sort(compareCodePoints).map((key) => [key, canonicalize(value[key])]),
    );
  }
  return value;
}

function deepCanonicalCopy(value) {
  return canonicalize(value);
}

function assertRelativeLibraryPath(value, label) {
  assertNonEmptyString(value, label);
  if (value.includes("\\") || value.startsWith("/") || /^[A-Za-z]:/.test(value) || value.split("/").includes("..")) {
    throw new Error(`${label} must be a normalized relative path`);
  }
}

function assertRecord(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(`${label} must be an object`);
}

function assertNonEmptyString(value, label) {
  if (typeof value !== "string" || value.length === 0) throw new Error(`${label} must be a non-empty string`);
}

function assertInteger(value, label, minimum = Number.MIN_SAFE_INTEGER) {
  if (!Number.isSafeInteger(value) || value < minimum) throw new Error(`${label} must be an integer >= ${minimum}`);
}

function assertHash(value, label) {
  if (typeof value !== "string" || !/^[a-f0-9]{64}$/.test(value)) throw new Error(`${label} must be a lowercase SHA-256 hash`);
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) throw new Error(`${label} must be ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
}

function sum(values, field) {
  return values.reduce((total, value) => total + value[field], 0);
}

function compareLibraryPaths(left, right) {
  return compareCodePoints(left.path, right.path);
}

function compareCodePoints(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function sha256(value) {
  return createHash("sha256").update(value, "utf8").digest("hex");
}

function parseArgs(argv) {
  const parsed = { _: [] };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (!argument.startsWith("--")) {
      parsed._.push(argument);
      continue;
    }
    const equals = argument.indexOf("=");
    if (equals >= 0) parsed[argument.slice(2, equals)] = argument.slice(equals + 1);
    else if (argv[index + 1] && !argv[index + 1].startsWith("--")) parsed[argument.slice(2)] = argv[++index];
    else throw new Error(`Missing value for ${argument}`);
  }
  return parsed;
}
