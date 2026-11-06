#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const LEDGER_SCHEMA_VERSION = "crystal-semantic-parity-ledger-v1";
export const INVENTORY_SCHEMA_VERSION = "crystal-semantic-source-inventory-v2";
export const EVIDENCE_SCHEMA_VERSION = "crystal-semantic-evidence-v1";
export const VERIFIER_VERSION = "crystal-semantic-parity-verifier-v3";
export const TRUSTED_CONTROLLED_ROOTS = Object.freeze(["Client", "Server", "Shared"]);

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const SCRIPT_DIR = path.dirname(SCRIPT_PATH);
const IMPLEMENTATION_ROOT = path.resolve(SCRIPT_DIR, "..", "..", "..");
const CRYSTAL_ROOT = path.resolve(IMPLEMENTATION_ROOT, "..", "Crystal");
const EVIDENCE_ROOT_RELATIVE = "docs/generated/crystal-semantic-parity";
const EVIDENCE_ROOT = path.resolve(IMPLEMENTATION_ROOT, ...EVIDENCE_ROOT_RELATIVE.split("/"));
const POLICY_RELATIVE = "docs/parity/crystal-semantic-parity-policy.json";
const PACKAGE_MANIFEST_RELATIVE = "dist/mir2-windows-candidate/package-manifest.json";
const PUBLIC_KEY_RELATIVE = "docs/parity/trusted-crystal-semantic-parity-signer.pem";
const CHALLENGE_RELATIVE = "challenge/expected.json";
const EXPECTED_CRYSTAL_RELATIVE = normalizeRelative(path.relative(IMPLEMENTATION_ROOT, CRYSTAL_ROOT));

const SHA256_RE = /^[0-9a-f]{64}$/;
const REVISION_RE = /^[0-9a-f]{40}$/;
const ID_RE = /^[A-Z0-9_]+(?:\.[A-Z0-9_]+){3,}$/;
const HEX_CHALLENGE_RE = /^[0-9a-f]{32,}$/;
const CONTROL_CHARACTER_RE = /[\u0000-\u001F\u007F-\u009F]/;
const WINDOWS_DOS_DEVICE_BASENAME_RE = /^(?:CON|PRN|AUX|NUL|CLOCK\$|COM[1-9]|LPT[1-9])(?:\..*)?$/i;
const STATUSES = new Set(["UNMAPPED", "MAPPED", "CONTRACT_READY", "IMPLEMENTED_UNVERIFIED", "TRACE_MISMATCH", "VERIFIED", "BLOCKED_EXTERNAL"]);
const EVIDENCE_KINDS = new Set(["CRYSTAL_TRACE", "IMPLEMENTATION_TRACE", "SEMANTIC_DIFF", "PERSISTENCE", "NEGATIVE_TEST", "VISUAL_ORIGINAL", "VISUAL_NATIVE", "VISUAL_REVIEW", "WEB_REGRESSION", "PACKAGE", "TEST_REPORT"]);
const VISUAL_KINDS = new Set(["VISUAL_ORIGINAL", "VISUAL_NATIVE", "VISUAL_REVIEW"]);
const PACKAGE_BOUND_KINDS = new Set(["IMPLEMENTATION_TRACE", "VISUAL_ORIGINAL", "VISUAL_NATIVE", "VISUAL_REVIEW", "PACKAGE"]);
const TOP_KEYS = ["schemaVersion", "crystalRevision", "implementationRevision", "inventoryComplete", "inventoryEvidence", "policySha256", "releasePackageIdentity", "capabilities"];
const INVENTORY_REF_KEYS = ["path", "sha256", "schemaVersion", "createdAt"];
const EVIDENCE_REF_KEYS = ["kind", "path", "sha256", "schemaVersion", "createdAt", "crystalRevision", "implementationRevision", "verifierVersion", "policySha256", "expiresAt", "challenge", "signerPinSha256", "packageIdentity"];
const SOURCE_KEYS = ["path", "symbol", "lineStart", "lineEnd"];
const CONTRACT_KEYS = ["preconditions", "inputs", "clock", "rng", "stateDeltas", "outbound", "clientConsequences", "persistence", "negativeCases"];
const CAPABILITY_KEYS = ["id", "domain", "description", "severity", "crystalSources", "dataIdentifiers", "contract", "implementationSources", "tests", "evidence", "knownDeviations", "status", "verifiedRevision", "packageIdentity"];
const ENVELOPE_KEYS = ["schemaVersion", "kind", "createdAt", "expiresAt", "crystalRevision", "implementationRevision", "verifierVersion", "policySha256", "packageIdentity", "challenge", "signerSpkiSha256", "payload", "signatureBase64"];

export class LedgerValidationError extends Error {
  constructor(message) { super(message); this.name = "LedgerValidationError"; }
}
export class LedgerBlockedError extends LedgerValidationError {
  constructor(message) { super(`BLOCKED: ${message}`); this.name = "LedgerBlockedError"; }
}
function fail(message) { throw new LedgerValidationError(message); }
function blocked(message) { throw new LedgerBlockedError(message); }
function assert(value, message) { if (!value) fail(message); }
function object(value) { return value !== null && typeof value === "object" && !Array.isArray(value); }
function nonEmpty(value, label) { assert(typeof value === "string" && value.length > 0, `${label} must be a non-empty string`); }
function sha(value, label) { assert(typeof value === "string" && SHA256_RE.test(value), `${label} must be lowercase SHA-256`); }
function revision(value, label) { assert(typeof value === "string" && REVISION_RE.test(value), `${label} must be a full lowercase Git revision`); }
function dateTime(value, label) {
  assert(typeof value === "string" && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(value), `${label} must be RFC3339`);
  assert(Number.isFinite(Date.parse(value)), `${label} must be a valid date-time`);
}
function exact(value, allowed, required, label) {
  assert(object(value), `${label} must be an object`);
  const allowedSet = new Set(allowed);
  for (const key of Object.keys(value)) assert(allowedSet.has(key), `${label} has unexpected field ${JSON.stringify(key)}`);
  for (const key of required) assert(Object.hasOwn(value, key), `${label} is missing ${key}`);
}
function strings(value, label, minimum = 0) {
  assert(Array.isArray(value) && value.length >= minimum, `${label} must contain at least ${minimum} item(s)`);
  value.forEach((entry, index) => nonEmpty(entry, `${label}[${index}]`));
}

function safeRelative(value, label) {
  nonEmpty(value, label);
  assert(!CONTROL_CHARACTER_RE.test(value), `${label} contains a control character`);
  assert(!value.includes("\\"), `${label} contains unsafe path separators`);
  assert(!path.posix.isAbsolute(value) && !path.win32.isAbsolute(value), `${label} must be relative`);
  assert(!/^[A-Za-z]:/.test(value), `${label} must not contain a drive prefix`);
  const pieces = value.split("/");
  assert(pieces.every((piece) => piece && piece !== "." && piece !== ".."), `${label} contains unsafe path segments`);
  assert(pieces.every((piece) => !/[. ]$/.test(piece)), `${label} contains a segment ending in a dot or space`);
  assert(pieces.every((piece) => !WINDOWS_DOS_DEVICE_BASENAME_RE.test(piece)), `${label} contains a Windows DOS device basename`);
  assert(pieces.every((piece) => !/[<>:"|?*]/.test(piece)), `${label} contains unsafe Windows path characters`);
}

function inventoryRefShape(value, label) {
  exact(value, INVENTORY_REF_KEYS, INVENTORY_REF_KEYS, label);
  safeRelative(value.path, `${label}.path`); sha(value.sha256, `${label}.sha256`);
  nonEmpty(value.schemaVersion, `${label}.schemaVersion`); dateTime(value.createdAt, `${label}.createdAt`);
}
function evidenceRefShape(value, label) {
  exact(value, EVIDENCE_REF_KEYS, ["kind", "path", "sha256", "schemaVersion", "createdAt", "crystalRevision", "implementationRevision", "verifierVersion", "policySha256"], label);
  assert(EVIDENCE_KINDS.has(value.kind), `${label}.kind is invalid`); safeRelative(value.path, `${label}.path`);
  sha(value.sha256, `${label}.sha256`); nonEmpty(value.schemaVersion, `${label}.schemaVersion`); dateTime(value.createdAt, `${label}.createdAt`);
  revision(value.crystalRevision, `${label}.crystalRevision`); revision(value.implementationRevision, `${label}.implementationRevision`);
  nonEmpty(value.verifierVersion, `${label}.verifierVersion`); sha(value.policySha256, `${label}.policySha256`);
  if (value.expiresAt !== undefined) dateTime(value.expiresAt, `${label}.expiresAt`);
  if (value.challenge !== undefined) nonEmpty(value.challenge, `${label}.challenge`);
  if (value.signerPinSha256 !== undefined) sha(value.signerPinSha256, `${label}.signerPinSha256`);
  if (value.packageIdentity !== undefined) nonEmpty(value.packageIdentity, `${label}.packageIdentity`);
}
function sourceShape(value, label) {
  exact(value, SOURCE_KEYS, SOURCE_KEYS, label); safeRelative(value.path, `${label}.path`); nonEmpty(value.symbol, `${label}.symbol`);
  assert(Number.isInteger(value.lineStart) && value.lineStart >= 1, `${label}.lineStart must be positive`);
  assert(Number.isInteger(value.lineEnd) && value.lineEnd >= 1, `${label}.lineEnd must be positive`);
  assert(value.lineStart <= value.lineEnd, `${label}.lineStart must not exceed lineEnd`);
}
function contractShape(value, label) {
  exact(value, CONTRACT_KEYS, CONTRACT_KEYS, label);
  strings(value.preconditions, `${label}.preconditions`); strings(value.inputs, `${label}.inputs`, 1);
  nonEmpty(value.clock, `${label}.clock`); nonEmpty(value.rng, `${label}.rng`);
  strings(value.stateDeltas, `${label}.stateDeltas`); strings(value.outbound, `${label}.outbound`);
  strings(value.clientConsequences, `${label}.clientConsequences`); strings(value.persistence, `${label}.persistence`);
  strings(value.negativeCases, `${label}.negativeCases`, 1);
}
function capabilityShape(value, index) {
  const label = `capabilities[${index}]`;
  exact(value, CAPABILITY_KEYS, ["id", "domain", "description", "severity", "crystalSources", "dataIdentifiers", "contract", "implementationSources", "tests", "evidence", "knownDeviations", "status"], label);
  assert(typeof value.id === "string" && ID_RE.test(value.id), `${label}.id is invalid`);
  assert(typeof value.domain === "string" && /^[A-Z0-9_]+$/.test(value.domain), `${label}.domain is invalid`);
  assert(value.id.split(".")[0] === value.domain, `${label}.domain must equal the ID prefix`);
  nonEmpty(value.description, `${label}.description`); assert(["P0", "P1", "P2"].includes(value.severity), `${label}.severity is invalid`);
  assert(Array.isArray(value.crystalSources) && value.crystalSources.length > 0, `${label}.crystalSources must be non-empty`);
  value.crystalSources.forEach((entry, sourceIndex) => sourceShape(entry, `${label}.crystalSources[${sourceIndex}]`));
  strings(value.dataIdentifiers, `${label}.dataIdentifiers`); contractShape(value.contract, `${label}.contract`);
  assert(Array.isArray(value.implementationSources), `${label}.implementationSources must be an array`);
  value.implementationSources.forEach((entry, sourceIndex) => sourceShape(entry, `${label}.implementationSources[${sourceIndex}]`));
  strings(value.tests, `${label}.tests`); assert(Array.isArray(value.evidence), `${label}.evidence must be an array`);
  value.evidence.forEach((entry, evidenceIndex) => evidenceRefShape(entry, `${label}.evidence[${evidenceIndex}]`));
  strings(value.knownDeviations, `${label}.knownDeviations`); assert(STATUSES.has(value.status), `${label}.status is invalid`);
  if (value.status === "VERIFIED") {
    revision(value.verifiedRevision, `${label}.verifiedRevision`); nonEmpty(value.packageIdentity, `${label}.packageIdentity`);
  } else {
    assert(value.verifiedRevision === undefined, `${label} non-VERIFIED status may not claim verifiedRevision`);
    assert(value.packageIdentity === undefined, `${label} non-VERIFIED status may not claim packageIdentity`);
  }
}
function ledgerShape(value) {
  exact(value, TOP_KEYS, TOP_KEYS, "ledger"); assert(value.schemaVersion === LEDGER_SCHEMA_VERSION, "ledger schemaVersion is unsupported");
  revision(value.crystalRevision, "ledger.crystalRevision"); revision(value.implementationRevision, "ledger.implementationRevision");
  assert(typeof value.inventoryComplete === "boolean", "ledger.inventoryComplete must be boolean"); inventoryRefShape(value.inventoryEvidence, "ledger.inventoryEvidence");
  sha(value.policySha256, "ledger.policySha256"); assert(value.releasePackageIdentity === null || (typeof value.releasePackageIdentity === "string" && value.releasePackageIdentity.length > 0), "releasePackageIdentity must be null or non-empty");
  assert(Array.isArray(value.capabilities), "ledger.capabilities must be an array"); value.capabilities.forEach(capabilityShape);
  const ids = value.capabilities.map((entry) => entry.id); const sorted = [...ids].sort(compareText);
  assert(new Set(ids).size === ids.length, "capability IDs must be unique"); assert(ids.every((id, index) => id === sorted[index]), "capability IDs must be sorted");
  return value;
}

function strictJson(bytes, label) {
  let text;
  try { text = new TextDecoder("utf-8", { fatal: true }).decode(bytes); } catch (error) { fail(`${label} is not UTF-8: ${error.message}`); }
  assert(!text.startsWith("\uFEFF"), `${label} contains a BOM`); scanDuplicateKeys(text, label);
  try { return JSON.parse(text); } catch (error) { fail(`${label} is not strict JSON: ${error.message}`); }
}
function scanDuplicateKeys(text, label) {
  let offset = 0; const bad = (message) => fail(`${label}: ${message} at byte ${offset}`); const skip = () => { while (/\s/.test(text[offset] ?? "")) offset += 1; };
  const string = () => { if (text[offset] !== '"') bad("expected string"); offset += 1; while (offset < text.length) { const code = text.charCodeAt(offset); if (code === 34) { offset += 1; return; } if (code < 32) bad("control character"); if (code === 92) { offset += 1; if (text[offset] === "u") { if (!/^[0-9a-fA-F]{4}$/.test(text.slice(offset + 1, offset + 5))) bad("unicode escape"); offset += 5; } else if ("\\/\"bfnrt".includes(text[offset] ?? "")) offset += 1; else bad("escape"); } else offset += 1; } bad("unterminated string"); };
  const value = () => { skip(); if (text[offset] === '"') return string(); if (text[offset] === "{") return objectValue(); if (text[offset] === "[") return arrayValue(); for (const literal of ["true", "false", "null"]) if (text.startsWith(literal, offset)) { offset += literal.length; return; } const number = text.slice(offset).match(/^-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/); if (number) { offset += number[0].length; return; } bad("value"); };
  const arrayValue = () => { offset += 1; skip(); if (text[offset] === "]") { offset += 1; return; } while (true) { value(); skip(); if (text[offset] === ",") { offset += 1; continue; } if (text[offset] === "]") { offset += 1; return; } bad("array separator"); } };
  const objectValue = () => { offset += 1; skip(); const keys = new Set(); if (text[offset] === "}") { offset += 1; return; } while (true) { skip(); const start = offset; string(); const key = JSON.parse(text.slice(start, offset)); if (keys.has(key)) bad(`duplicate key ${key}`); keys.add(key); skip(); if (text[offset] !== ":") bad("colon"); offset += 1; value(); skip(); if (text[offset] === ",") { offset += 1; continue; } if (text[offset] === "}") { offset += 1; return; } bad("object separator"); } };
  value(); skip(); assert(offset === text.length, `${label} contains trailing data`);
}

function comparable(value) { let result = path.normalize(value); if (process.platform === "win32" && result.startsWith("\\\\?\\")) result = result.slice(4); return process.platform === "win32" ? result.toLowerCase() : result; }
function contained(root, candidate) { const relative = path.relative(path.resolve(root), path.resolve(candidate)); return relative === "" || (relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative)); }
function noReparse(absolutePath, label) {
  const absolute = path.resolve(absolutePath); const parsed = path.parse(absolute); let current = parsed.root; const relative = path.relative(parsed.root, absolute);
  for (const segment of relative ? relative.split(path.sep) : []) { current = path.join(current, segment); let info; try { info = fs.lstatSync(current); } catch (error) { fail(`${label} does not exist: ${error.message}`); } assert(!info.isSymbolicLink(), `${label} contains a symlink/reparse component`); let real; try { real = fs.realpathSync.native(current); } catch (error) { fail(`${label} realpath failed: ${error.message}`); } assert(comparable(real) === comparable(current), `${label} contains a junction/reparse component`); }
  return absolute;
}
function safeRoot(root, label) { const absolute = noReparse(root, label); const info = fs.lstatSync(absolute); assert(info.isDirectory(), `${label} must be a directory`); return absolute; }
function resolveSafe(root, relative, label) { safeRelative(relative, label); const candidate = path.resolve(root, ...relative.split("/")); assert(contained(root, candidate), `${label} escapes its root`); noReparse(candidate, label); const real = fs.realpathSync.native(candidate); assert(comparable(real) === comparable(candidate), `${label} resolves through reparse`); return candidate; }

function stableStat(left, right) { return left.dev === right.dev && left.ino === right.ino && left.size === right.size && left.mtimeNs === right.mtimeNs && left.ctimeNs === right.ctimeNs; }
function readBound(root, relative, label) {
  const filePath = resolveSafe(root, relative, label); const before = fs.lstatSync(filePath, { bigint: true }); assert(before.isFile() && !before.isSymbolicLink(), `${label} must be a regular file`);
  const noFollow = fs.constants.O_NOFOLLOW ?? 0; let fd;
  try { fd = fs.openSync(filePath, fs.constants.O_RDONLY | noFollow); } catch (error) { fail(`${label} open failed: ${error.message}`); }
  try { const opened = fs.fstatSync(fd, { bigint: true }); assert(opened.isFile() && stableStat(before, opened), `${label} changed before open`); const bytes = fs.readFileSync(fd); const after = fs.fstatSync(fd, { bigint: true }); assert(stableStat(opened, after) && BigInt(bytes.length) === after.size, `${label} changed while reading`); return { bytes, filePath, strongNoFollow: process.platform !== "win32" && noFollow !== 0 }; } finally { fs.closeSync(fd); }
}
function readJsonBound(root, relative, label) { const result = readBound(root, relative, label); return { ...result, value: strictJson(result.bytes, label), sha256: hash(result.bytes) }; }
function hash(bytes) { return crypto.createHash("sha256").update(bytes).digest("hex"); }

export function gitInfo(root, label, options = {}) {
  try {
    const absoluteRoot = path.resolve(root);
    const repositoryRoot = path.resolve(execFileSync("git", ["rev-parse", "--show-toplevel"], { cwd: absoluteRoot, encoding: "utf8" }).trim());
    const head = execFileSync("git", ["rev-parse", "--verify", "HEAD"], { cwd: absoluteRoot, encoding: "utf8" }).trim();
    revision(head, `${label} HEAD`);
    const repositoryComparable = comparable(repositoryRoot);
    const rootComparable = comparable(absoluteRoot);
    const repositoryPrefix = repositoryComparable.endsWith(path.sep) ? repositoryComparable : `${repositoryComparable}${path.sep}`;
    assert(rootComparable === repositoryComparable || rootComparable.startsWith(repositoryPrefix), `${label} is outside repository`);
    const relativeRoot = rootComparable === repositoryComparable ? "" : path.relative(repositoryRoot, absoluteRoot);
    const pathspec = relativeRoot === "" ? "." : normalizeRelative(relativeRoot);
    assert(pathspec === "." || (pathspec.length > 0 && !pathspec.startsWith("../") && !path.isAbsolute(pathspec)), `${label} pathspec escapes repository`);
    const status = execFileSync("git", ["status", "--porcelain=v1", "--untracked-files=all", "--", pathspec], { cwd: repositoryRoot, encoding: "utf8" });
    const clean = status.length === 0;
    if (options.requireClean === true && !clean) blocked(`${label} scoped worktree is dirty`);
    return { repositoryRoot, head, pathspec, clean };
  } catch (error) {
    if (error instanceof LedgerValidationError) throw error;
    fail(`${label} Git inspection failed: ${error.message}`);
  }
}
function requireTracked(root, relative, label) { try { execFileSync("git", ["ls-files", "--error-unmatch", "--", relative], { cwd: root, stdio: "ignore" }); } catch { blocked(`${label} is not tracked by the implementation repository`); } }

function enumerateCrystal(crystalRoot) {
  const files = []; let strong = true;
  for (const controlledRoot of TRUSTED_CONTROLLED_ROOTS) {
    const rootPath = resolveSafe(crystalRoot, controlledRoot, `controlled root ${controlledRoot}`); assert(fs.lstatSync(rootPath).isDirectory(), `controlled root ${controlledRoot} is not a directory`); const pending = [controlledRoot];
    while (pending.length) { const relativeDirectory = pending.pop(); const absoluteDirectory = resolveSafe(crystalRoot, relativeDirectory, relativeDirectory); const entries = fs.readdirSync(absoluteDirectory, { withFileTypes: true }).sort((a, b) => compareText(a.name, b.name)); for (const entry of entries) { const relative = `${relativeDirectory}/${entry.name}`; const absolute = path.resolve(crystalRoot, ...relative.split("/")); const info = fs.lstatSync(absolute); assert(!info.isSymbolicLink(), `inventory source contains symlink/reparse: ${relative}`); noReparse(absolute, relative); if (info.isDirectory()) pending.push(relative); else if (info.isFile() && entry.name.toLowerCase().endsWith(".cs")) { const read = readBound(crystalRoot, relative, relative); strong &&= read.strongNoFollow; let text; try { text = new TextDecoder("utf-8", { fatal: true }).decode(read.bytes); } catch (error) { fail(`inventory source is not UTF-8: ${relative}: ${error.message}`); } files.push({ path: relative, sha256: hash(read.bytes), encoding: "utf-8", bytes: read.bytes.length, lineCount: text.length === 0 ? 0 : text.split(/\r\n|\n|\r/).length, controlledRoot }); } } }
  }
  files.sort((a, b) => compareText(a.path, b.path)); return { files, strong };
}
function inventoryAggregate(files) { return hash(Buffer.from([...TRUSTED_CONTROLLED_ROOTS.map((root) => `root\t${root}\n`), ...files.map((file) => `file\t${file.path}\t${file.bytes}\t${file.lineCount}\t${file.sha256}\n`)].join(""), "utf8")); }
function verifyInventory(report, ledger, roots, crystalGit) {
  const keys = ["schemaVersion", "generator", "referenceRootRelative", "controlledRoots", "crystalRevision", "sourceRootClean", "sourceFileInventoryComplete", "semanticLeafInventoryComplete", "inventoryComplete", "aggregateSha256", "counts", "sourceFiles"];
  exact(report, keys, keys, "inventory report"); assert(report.schemaVersion === INVENTORY_SCHEMA_VERSION, "inventory schemaVersion is unsupported"); assert(report.generator === "generate-crystal-semantic-source-inventory.mjs", "inventory generator is unsupported");
  assert(report.referenceRootRelative === EXPECTED_CRYSTAL_RELATIVE, "inventory referenceRootRelative is not trusted"); assert(JSON.stringify(report.controlledRoots) === JSON.stringify(TRUSTED_CONTROLLED_ROOTS), "inventory controlledRoots must be exactly Client/Server/Shared"); revision(report.crystalRevision, "inventory.crystalRevision"); assert(report.crystalRevision === ledger.crystalRevision && report.crystalRevision === crystalGit.head, "inventory crystalRevision mismatch"); assert(typeof report.sourceRootClean === "boolean" && report.sourceRootClean === crystalGit.clean, "inventory sourceRootClean does not match the actual Crystal scoped worktree"); assert(typeof report.sourceFileInventoryComplete === "boolean", "inventory sourceFileInventoryComplete must be boolean"); assert(typeof report.semanticLeafInventoryComplete === "boolean", "inventory semanticLeafInventoryComplete must be boolean"); assert(report.semanticLeafInventoryComplete === false, "unsupported/trusted semantic leaf inventory is missing; semanticLeafInventoryComplete must remain false"); assert(typeof report.inventoryComplete === "boolean" && report.inventoryComplete === ledger.inventoryComplete, "inventoryComplete mismatch"); sha(report.aggregateSha256, "inventory.aggregateSha256");
  exact(report.counts, ["controlledRoots", "sourceFiles"], ["controlledRoots", "sourceFiles"], "inventory.counts"); assert(report.counts.controlledRoots === 3, "inventory controlledRoots count must be 3"); assert(Number.isInteger(report.counts.sourceFiles) && report.counts.sourceFiles >= 0, "inventory sourceFiles count is invalid"); assert(Array.isArray(report.sourceFiles), "inventory.sourceFiles must be an array");
  const declared = report.sourceFiles; const declaredPaths = declared.map((entry) => entry.path); assert(new Set(declaredPaths).size === declaredPaths.length, "inventory source paths must be unique"); const windowsFoldedPaths = declaredPaths.map((entry) => entry.toLowerCase()); assert(new Set(windowsFoldedPaths).size === windowsFoldedPaths.length, "inventory source paths must be unique under Windows case-folding"); assert(declaredPaths.every((entry, index) => entry === [...declaredPaths].sort(compareText)[index]), "inventory sourceFiles must be sorted");
  for (const [index, entry] of declared.entries()) { const label = `inventory.sourceFiles[${index}]`; exact(entry, ["path", "sha256", "encoding", "bytes", "lineCount", "controlledRoot"], ["path", "sha256", "encoding", "bytes", "lineCount", "controlledRoot"], label); safeRelative(entry.path, `${label}.path`); sha(entry.sha256, `${label}.sha256`); assert(entry.encoding === "utf-8", `${label}.encoding must be utf-8`); assert(Number.isInteger(entry.bytes) && entry.bytes >= 0, `${label}.bytes is invalid`); assert(Number.isInteger(entry.lineCount) && entry.lineCount >= 0, `${label}.lineCount is invalid`); assert(TRUSTED_CONTROLLED_ROOTS.includes(entry.controlledRoot) && (entry.path === entry.controlledRoot || entry.path.startsWith(`${entry.controlledRoot}/`)), `${label}.controlledRoot mismatch`); }
  const actual = enumerateCrystal(roots.crystalRoot); assert(report.counts.sourceFiles === actual.files.length && declared.length === actual.files.length, "inventory sourceFiles count does not match Crystal tree"); assert(JSON.stringify(declared) === JSON.stringify(actual.files), "inventory sourceFiles do not match recomputed Crystal tree"); assert(report.aggregateSha256 === inventoryAggregate(actual.files), "inventory aggregateSha256 mismatch"); const sourceFileInventoryComplete = crystalGit.clean && actual.strong; assert(report.sourceFileInventoryComplete === sourceFileInventoryComplete, "inventory sourceFileInventoryComplete does not match the clean, strong, recomputed Crystal file inventory"); const expectedInventoryComplete = sourceFileInventoryComplete && report.semanticLeafInventoryComplete; assert(report.inventoryComplete === expectedInventoryComplete && ledger.inventoryComplete === expectedInventoryComplete, "inventoryComplete must equal sourceFileInventoryComplete && semanticLeafInventoryComplete"); return { strong: actual.strong, sourceFileInventoryComplete, semanticLeafInventoryComplete: report.semanticLeafInventoryComplete, inventoryComplete: expectedInventoryComplete };
}

function verifySourceReferences(capabilities, roots) {
  const groups = [["crystalSources", roots.crystalRoot], ["implementationSources", roots.implementationRoot]];
  for (const [field, root] of groups) for (const [capIndex, capability] of capabilities.entries()) for (const [sourceIndex, source] of capability[field].entries()) { const label = `capabilities[${capIndex}].${field}[${sourceIndex}]`; const read = readBound(root, source.path, `${label}.path`); let text; try { text = new TextDecoder("utf-8", { fatal: true }).decode(read.bytes); } catch (error) { fail(`${label} is not UTF-8: ${error.message}`); } const lines = text.split(/\r\n|\n|\r/); assert(source.lineEnd <= lines.length, `${label}.lineEnd exceeds source lines`); const token = source.symbol.split(/[.:]/).filter(Boolean).at(-1); const span = lines.slice(source.lineStart - 1, source.lineEnd).join("\n"); assert(new RegExp(`(?:^|[^A-Za-z0-9_])${escapeRegex(token)}(?:$|[^A-Za-z0-9_])`).test(span), `${label}.symbol is absent from declared span`); }
}
function escapeRegex(value) { return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"); }

function payloadShape(kind, payload, label, formal) {
  const integer = (value, field, min = 0) => assert(Number.isInteger(value) && value >= min, `${label}.${field} is invalid`);
  const hashField = (value, field) => sha(value, `${label}.${field}`);
  if (kind === "CRYSTAL_TRACE" || kind === "IMPLEMENTATION_TRACE") { exact(payload, ["traceSchema", "eventCount", "orderedDigestSha256"], ["traceSchema", "eventCount", "orderedDigestSha256"], label); nonEmpty(payload.traceSchema, `${label}.traceSchema`); integer(payload.eventCount, "eventCount"); hashField(payload.orderedDigestSha256, "orderedDigestSha256"); }
  else if (kind === "SEMANTIC_DIFF") { exact(payload, ["matches", "differenceCount", "diffSha256"], ["matches", "differenceCount", "diffSha256"], label); assert(typeof payload.matches === "boolean", `${label}.matches must be boolean`); integer(payload.differenceCount, "differenceCount"); hashField(payload.diffSha256, "diffSha256"); if (formal) assert(payload.matches && payload.differenceCount === 0, `${label} reports semantic differences`); }
  else if (kind === "PERSISTENCE") { exact(payload, ["beforeSha256", "afterSha256", "reloadMatches"], ["beforeSha256", "afterSha256", "reloadMatches"], label); hashField(payload.beforeSha256, "beforeSha256"); hashField(payload.afterSha256, "afterSha256"); assert(typeof payload.reloadMatches === "boolean", `${label}.reloadMatches must be boolean`); if (formal) assert(payload.reloadMatches, `${label} persistence reload mismatches`); }
  else if (kind === "NEGATIVE_TEST" || kind === "WEB_REGRESSION" || kind === "TEST_REPORT") { exact(payload, ["testCount", "failedCount", "reportSha256"], ["testCount", "failedCount", "reportSha256"], label); integer(payload.testCount, "testCount", 1); integer(payload.failedCount, "failedCount"); hashField(payload.reportSha256, "reportSha256"); if (formal) assert(payload.failedCount === 0, `${label} reports failed tests`); }
  else if (kind === "VISUAL_ORIGINAL" || kind === "VISUAL_NATIVE") { exact(payload, ["scene", "imageSha256", "width", "height"], ["scene", "imageSha256", "width", "height"], label); nonEmpty(payload.scene, `${label}.scene`); hashField(payload.imageSha256, "imageSha256"); integer(payload.width, "width", 1); integer(payload.height, "height", 1); }
  else if (kind === "VISUAL_REVIEW") { exact(payload, ["scene", "score", "threshold", "passed", "reviewSha256"], ["scene", "score", "threshold", "passed", "reviewSha256"], label); nonEmpty(payload.scene, `${label}.scene`); assert(Number.isFinite(payload.score) && Number.isFinite(payload.threshold), `${label} score/threshold invalid`); assert(typeof payload.passed === "boolean", `${label}.passed must be boolean`); hashField(payload.reviewSha256, "reviewSha256"); if (formal) assert(payload.passed && payload.score >= payload.threshold, `${label} visual review failed`); }
  else if (kind === "PACKAGE") { exact(payload, ["manifestSha256", "passed"], ["manifestSha256", "passed"], label); hashField(payload.manifestSha256, "manifestSha256"); assert(typeof payload.passed === "boolean", `${label}.passed must be boolean`); if (formal) assert(payload.passed, `${label} package report failed`); }
}
function verifyEvidenceEnvelope(reference, read, ledger, label, formal, trust) {
  assert(read.sha256 === reference.sha256, `${label}.sha256 mismatch`); const value = read.value;
  exact(value, ENVELOPE_KEYS, ["schemaVersion", "kind", "createdAt", "crystalRevision", "implementationRevision", "verifierVersion", "policySha256", "payload"], `${label} file`);
  assert(reference.schemaVersion === EVIDENCE_SCHEMA_VERSION && value.schemaVersion === EVIDENCE_SCHEMA_VERSION, `${label} evidence schema is unsupported`); assert(value.kind === reference.kind, `${label} kind mismatch`); assert(value.createdAt === reference.createdAt, `${label} createdAt mismatch`); dateTime(value.createdAt, `${label}.file.createdAt`);
  for (const field of ["crystalRevision", "implementationRevision", "policySha256"]) assert(value[field] === reference[field] && value[field] === ledger[field], `${label}.${field} is not bound to ledger`);
  assert(value.verifierVersion === reference.verifierVersion, `${label}.verifierVersion metadata mismatch`);
  if (reference.expiresAt !== undefined) { assert(value.expiresAt === reference.expiresAt, `${label} expiresAt mismatch`); dateTime(value.expiresAt, `${label}.expiresAt`); assert(Date.parse(value.expiresAt) >= Date.now(), `${label} is expired`); } else assert(value.expiresAt === undefined, `${label} file has unbound expiresAt`);
  for (const field of ["packageIdentity", "challenge"]) assert(value[field] === reference[field], `${label} ${field} mismatch`);
  if (reference.signerPinSha256 !== undefined) assert(value.signerSpkiSha256 === reference.signerPinSha256, `${label} signer pin mismatch`);
  assert(object(value.payload), `${label}.payload must be an object`); payloadShape(reference.kind, value.payload, `${label}.payload`, formal);
  if (formal) { nonEmpty(reference.expiresAt, `${label}.expiresAt`); assert(reference.verifierVersion === VERIFIER_VERSION, `${label}.verifierVersion is not the trusted verifier version`); assert(reference.challenge === trust.challenge, `${label}.challenge mismatch`); assert(reference.signerPinSha256 === trust.signerPin, `${label}.signerPinSha256 mismatch`); assert(value.signatureBase64 !== undefined, `${label} file lacks signature`); verifySignature(value, trust.publicKey, `${label} file`); if (PACKAGE_BOUND_KINDS.has(reference.kind)) assert(reference.packageIdentity === ledger.releasePackageIdentity, `${label}.packageIdentity mismatch`); }
  return value;
}

function canonical(value) { if (value === null || typeof value !== "object") return JSON.stringify(value); if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`; return `{${Object.keys(value).sort(compareText).map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`; }
function unsigned(value) { const clone = { ...value }; delete clone.signatureBase64; return clone; }
function verifySignature(value, publicKey, label) { nonEmpty(value.signatureBase64, `${label}.signatureBase64`); let signature; try { signature = Buffer.from(value.signatureBase64, "base64"); } catch { fail(`${label} signature is invalid base64`); } assert(signature.length > 0 && signature.toString("base64").replace(/=+$/, "") === value.signatureBase64.replace(/=+$/, ""), `${label} signature is invalid base64`); assert(crypto.verify("sha256", Buffer.from(canonical(unsigned(value))), { key: publicKey, padding: crypto.constants.RSA_PKCS1_PADDING }, signature), `${label} signature is invalid`); }

function loadPolicy(ledger, roots, implementationGit, requireComplete) {
  const policyPath = path.resolve(roots.implementationRoot, ...POLICY_RELATIVE.split("/")); if (!fs.existsSync(policyPath)) { if (requireComplete) blocked(`fixed policy file is missing: ${POLICY_RELATIVE}`); return null; }
  requireTracked(roots.implementationRoot, POLICY_RELATIVE, "fixed policy"); const read = readJsonBound(roots.implementationRoot, POLICY_RELATIVE, "fixed policy"); assert(read.sha256 === ledger.policySha256, "ledger.policySha256 does not match fixed policy");
  const keys = ["schemaVersion", "policyId", "verifierVersion", "inventorySchemaVersion", "evidenceSchemaVersion", "crystalRootRelative", "controlledRoots", "evidenceRootRelative", "packageManifestRelative", "trustedSignerPublicKeyRelative", "trustedSignerSpkiSha256", "challengeRelative", "challengeAuthority"];
  exact(read.value, keys, keys, "fixed policy"); const policy = read.value; assert(policy.schemaVersion === "crystal-semantic-parity-policy-v1", "policy schema is unsupported"); nonEmpty(policy.policyId, "policy.policyId"); assert(policy.verifierVersion === VERIFIER_VERSION, "policy verifierVersion mismatch"); assert(policy.inventorySchemaVersion === INVENTORY_SCHEMA_VERSION && policy.evidenceSchemaVersion === EVIDENCE_SCHEMA_VERSION, "policy evidence/inventory schema mismatch"); assert(policy.crystalRootRelative === EXPECTED_CRYSTAL_RELATIVE, "policy crystal root is not fixed"); assert(JSON.stringify(policy.controlledRoots) === JSON.stringify(TRUSTED_CONTROLLED_ROOTS), "policy controlled roots are not fixed"); assert(policy.evidenceRootRelative === EVIDENCE_ROOT_RELATIVE && policy.packageManifestRelative === PACKAGE_MANIFEST_RELATIVE && policy.trustedSignerPublicKeyRelative === PUBLIC_KEY_RELATIVE && policy.challengeRelative === CHALLENGE_RELATIVE, "policy trust paths are not repository-fixed"); assert(policy.challengeAuthority === "external-one-time-required", "policy must require an external one-time challenge authority"); sha(policy.trustedSignerSpkiSha256, "policy.trustedSignerSpkiSha256"); assert(implementationGit.head === ledger.implementationRevision, "policy repository revision mismatch"); return policy;
}
function loadPublicKey(policy, roots) { requireTracked(roots.implementationRoot, PUBLIC_KEY_RELATIVE, "trusted signer public key"); const read = readBound(roots.implementationRoot, PUBLIC_KEY_RELATIVE, "trusted signer public key"); let publicKey; try { publicKey = crypto.createPublicKey(read.bytes); } catch (error) { blocked(`trusted signer public key is invalid: ${error.message}`); } assert(publicKey.asymmetricKeyType === "rsa" && (publicKey.asymmetricKeyDetails?.modulusLength ?? 0) >= 3072, "trusted signer must be RSA-3072 or stronger"); const pin = hash(publicKey.export({ type: "spki", format: "der" })); assert(pin === policy.trustedSignerSpkiSha256, "trusted signer SPKI pin mismatch"); return { publicKey, pin, strong: read.strongNoFollow };
}
function loadChallenge(policy, ledger, roots, signer) { const read = readJsonBound(roots.evidenceRoot, policy.challengeRelative, "fixed expected challenge"); const keys = ["schemaVersion", "challenge", "issuedAt", "expiresAt", "implementationRevision", "packageIdentity", "signerSpkiSha256", "signatureBase64"]; exact(read.value, keys, keys, "expected challenge"); const value = read.value; assert(value.schemaVersion === "crystal-semantic-parity-challenge-v1", "challenge schema unsupported"); assert(HEX_CHALLENGE_RE.test(value.challenge), "challenge must be at least 128-bit lowercase hex"); dateTime(value.issuedAt, "challenge.issuedAt"); dateTime(value.expiresAt, "challenge.expiresAt"); assert(Date.parse(value.issuedAt) <= Date.now() && Date.parse(value.expiresAt) >= Date.now(), "challenge is outside validity window"); assert(value.implementationRevision === ledger.implementationRevision && value.packageIdentity === ledger.releasePackageIdentity && value.signerSpkiSha256 === signer.pin, "challenge binding mismatch"); verifySignature(value, signer.publicKey, "expected challenge"); return { value, strong: read.strongNoFollow };
}
function loadPackage(policy, ledger, roots, signer, challenge) { const read = readJsonBound(roots.implementationRoot, policy.packageManifestRelative, "fixed package manifest"); const keys = ["schemaVersion", "packageIdentity", "implementationRevision", "policySha256", "challenge", "createdAt", "expiresAt", "signerSpkiSha256", "aggregateSha256", "files", "signatureBase64"]; exact(read.value, keys, keys, "package manifest"); const value = read.value; assert(value.schemaVersion === "mir2-windows-package-manifest-v1", "package manifest schema unsupported"); assert(value.packageIdentity === ledger.releasePackageIdentity && value.implementationRevision === ledger.implementationRevision && value.policySha256 === ledger.policySha256 && value.challenge === challenge.value.challenge && value.signerSpkiSha256 === signer.pin, "package manifest binding mismatch"); dateTime(value.createdAt, "package.createdAt"); dateTime(value.expiresAt, "package.expiresAt"); assert(Date.parse(value.expiresAt) >= Date.now(), "package manifest is expired"); assert(Array.isArray(value.files) && value.files.length > 0, "package manifest files must be non-empty"); const packageRoot = path.dirname(read.filePath); const paths = []; let strong = read.strongNoFollow; for (const [index, file] of value.files.entries()) { const label = `package.files[${index}]`; exact(file, ["path", "sha256", "bytes"], ["path", "sha256", "bytes"], label); safeRelative(file.path, `${label}.path`); sha(file.sha256, `${label}.sha256`); assert(Number.isInteger(file.bytes) && file.bytes >= 0, `${label}.bytes invalid`); paths.push(file.path); const content = readBound(packageRoot, file.path, label); strong &&= content.strongNoFollow; assert(content.bytes.length === file.bytes && hash(content.bytes) === file.sha256, `${label} content mismatch`); } assert(new Set(paths).size === paths.length && paths.every((entry, index) => entry === [...paths].sort(compareText)[index]), "package files must be unique and sorted"); const aggregate = hash(Buffer.from(value.files.map((file) => `${file.path}\t${file.bytes}\t${file.sha256}\n`).join(""))); assert(aggregate === value.aggregateSha256, "package aggregateSha256 mismatch"); verifySignature(value, signer.publicKey, "package manifest"); return { value, sha256: read.sha256, strong };
}

function formalCapability(capability, index, ledger, evidenceRecords, trust, packageManifestSha) { const label = `capabilities[${index}]`; assert(capability.status === "VERIFIED", `${label} is not VERIFIED`); assert(capability.verifiedRevision === ledger.implementationRevision, `${label}.verifiedRevision mismatch`); assert(capability.packageIdentity === ledger.releasePackageIdentity, `${label}.packageIdentity mismatch`); assert(capability.implementationSources.length > 0 && capability.tests.length > 0 && capability.evidence.length > 0 && capability.knownDeviations.length === 0, `${label} lacks formal implementation/test/evidence closure`); const kinds = new Set(); for (const record of evidenceRecords) { kinds.add(record.reference.kind); const envelope = verifyEvidenceEnvelope(record.reference, record.read, ledger, record.label, true, trust); if (record.reference.kind === "PACKAGE") assert(envelope.payload.manifestSha256 === packageManifestSha, `${record.label} package manifest hash mismatch`); } for (const required of ["CRYSTAL_TRACE", "IMPLEMENTATION_TRACE", "SEMANTIC_DIFF", "NEGATIVE_TEST"]) assert(kinds.has(required), `${label} lacks ${required}`); if (capability.contract.persistence.length) assert(kinds.has("PERSISTENCE"), `${label} lacks PERSISTENCE`); if (["UI", "HUD", "TEXT", "ANIMATION", "EFFECT", "AUDIO", "ASSET"].includes(capability.domain)) for (const kind of VISUAL_KINDS) assert(kinds.has(kind), `${label} lacks ${kind}`); }

function fixedRoots(options, complete) { if (complete) for (const key of ["implementationRoot", "crystalRoot", "evidenceRoot"]) assert(typeof options[key] === "string" && options[key].length > 0, `--require-complete requires --${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)}`); const expected = { implementationRoot: IMPLEMENTATION_ROOT, crystalRoot: CRYSTAL_ROOT, evidenceRoot: EVIDENCE_ROOT }; for (const [key, value] of Object.entries(expected)) if (options[key] !== undefined) assert(comparable(path.resolve(options[key])) === comparable(value), `${key} is repository-fixed and cannot be caller-selected`); if (options.root !== undefined) assert(!complete && comparable(path.resolve(options.root)) === comparable(EVIDENCE_ROOT), "--root cannot select or satisfy a formal trust root"); return { implementationRoot: safeRoot(IMPLEMENTATION_ROOT, "implementation root"), crystalRoot: safeRoot(CRYSTAL_ROOT, "Crystal root"), evidenceRoot: safeRoot(EVIDENCE_ROOT, "evidence root") }; }

export function verifyLedgerFile(ledgerPath, options = {}) {
  const complete = options.requireComplete === true; const roots = fixedRoots(options, complete); const ledgerRead = readBound(path.dirname(path.resolve(ledgerPath)), path.basename(path.resolve(ledgerPath)), "ledger"); const ledger = ledgerShape(strictJson(ledgerRead.bytes, "ledger"));
  const implementationGit = gitInfo(roots.implementationRoot, "implementation root", { requireClean: complete }); const crystalGit = gitInfo(roots.crystalRoot, "Crystal root", { requireClean: complete }); assert(implementationGit.head === ledger.implementationRevision, "implementation HEAD mismatch"); assert(crystalGit.head === ledger.crystalRevision, "Crystal HEAD mismatch");
  const inventoryRead = readJsonBound(roots.evidenceRoot, ledger.inventoryEvidence.path, "inventory evidence"); assert(inventoryRead.sha256 === ledger.inventoryEvidence.sha256, "inventory evidence hash mismatch"); assert(ledger.inventoryEvidence.schemaVersion === INVENTORY_SCHEMA_VERSION, "inventory evidence schema reference mismatch"); const inventoryStrong = verifyInventory(inventoryRead.value, ledger, roots, crystalGit); verifySourceReferences(ledger.capabilities, roots);
  const records = ledger.capabilities.map((capability, capIndex) => capability.evidence.map((reference, evidenceIndex) => { const label = `capabilities[${capIndex}].evidence[${evidenceIndex}]`; assert(reference.crystalRevision === ledger.crystalRevision && reference.implementationRevision === ledger.implementationRevision && reference.policySha256 === ledger.policySha256, `${label} ledger binding mismatch`); if (reference.expiresAt !== undefined) assert(Date.parse(reference.expiresAt) >= Date.now(), `${label} is expired`); const read = readJsonBound(roots.evidenceRoot, reference.path, label); verifyEvidenceEnvelope(reference, read, ledger, label, false, null); return { reference, read, label }; }));
  const claimedVerifiedCount = ledger.capabilities.filter((capability) => capability.status === "VERIFIED").length;
  if (!complete) { const classification = !inventoryStrong.sourceFileInventoryComplete ? "SOURCE_FILE_INVENTORY_INCOMPLETE" : !inventoryStrong.semanticLeafInventoryComplete ? "SEMANTIC_INVENTORY_INCOMPLETE" : "SEMANTIC_INVENTORIED"; return { ledger, classification, sourceRootClean: crystalGit.clean, sourceFileInventoryComplete: inventoryStrong.sourceFileInventoryComplete, semanticLeafInventoryComplete: inventoryStrong.semanticLeafInventoryComplete, claimedVerifiedCount, formalVerifiedCount: 0 }; }
  if (ledger.inventoryComplete !== true || inventoryStrong.semanticLeafInventoryComplete !== true || ledger.capabilities.length === 0) blocked("formal completion requires complete trusted semantic inventory and non-empty capabilities"); assert(typeof ledger.releasePackageIdentity === "string" && ledger.releasePackageIdentity.length > 0, "formal completion requires releasePackageIdentity"); const policy = loadPolicy(ledger, roots, implementationGit, true); const signer = loadPublicKey(policy, roots); const challenge = loadChallenge(policy, ledger, roots, signer); const packageManifest = loadPackage(policy, ledger, roots, signer, challenge); const trust = { publicKey: signer.publicKey, signerPin: signer.pin, challenge: challenge.value.challenge }; ledger.capabilities.forEach((capability, index) => formalCapability(capability, index, ledger, records[index], trust, packageManifest.sha256));
  const strong = ledgerRead.strongNoFollow && inventoryRead.strongNoFollow && inventoryStrong.strong && signer.strong && challenge.strong && packageManifest.strong && records.flat().every((record) => record.read.strongNoFollow);
  if (!strong) blocked("the local Node/Windows file APIs cannot prove no-follow, TOCTOU-safe verification for every bound file");
  blocked("a trusted external one-time challenge consumption authority/receipt is not configured; local self-consumption is replayable");
}

export function parseArgs(argv) { const result = { requireComplete: false }; const positional = []; const named = new Map([["--implementation-root", "implementationRoot"], ["--crystal-root", "crystalRoot"], ["--evidence-root", "evidenceRoot"], ["--root", "root"]]); for (let index = 0; index < argv.length; index += 1) { const arg = argv[index]; if (arg === "--require-complete") result.requireComplete = true; else if (arg === "--help" || arg === "-h") result.help = true; else if (named.has(arg)) { const value = argv[++index]; if (!value || value.startsWith("-")) fail(`${arg} requires a value`); result[named.get(arg)] = value; } else if (arg.startsWith("-")) fail(`unknown option ${arg}`); else positional.push(arg); } if (!result.help && positional.length !== 1) fail("exactly one ledger path is required"); result.ledgerPath = positional[0]; return result; }
export function usage() { return ["Usage: node verify-crystal-semantic-parity-ledger.mjs <ledger.json> [--require-complete --implementation-root PATH --crystal-root PATH --evidence-root PATH]", "Trust roots are fixed relative to this verifier; supplied paths must exactly match them.", `Verifier version: ${VERIFIER_VERSION}`].join("\n"); }
function compareText(left, right) { return left < right ? -1 : left > right ? 1 : 0; }
function normalizeRelative(value) { return value.split(path.sep).join("/"); }

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(SCRIPT_PATH)) {
  try { const options = parseArgs(process.argv.slice(2)); if (options.help) console.log(usage()); else { const result = verifyLedgerFile(options.ledgerPath, options); console.log(`${result.classification}: ledger/evidence/inventory checked; sourceRootClean=${result.sourceRootClean ?? "not-reported"}; sourceFileInventoryComplete=${result.sourceFileInventoryComplete ?? "not-reported"}; semanticLeafInventoryComplete=${result.semanticLeafInventoryComplete ?? "not-reported"}; claimedMarkedVerified=${result.claimedVerifiedCount}; formalVerified=0`); } }
  catch (error) { console.error(error.message); process.exitCode = 1; }
}
