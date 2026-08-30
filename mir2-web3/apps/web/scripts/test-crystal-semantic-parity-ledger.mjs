#!/usr/bin/env node

import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { gitInfo } from "./verify-crystal-semantic-parity-ledger.mjs";

const SOURCE_VERIFIER = path.join(path.dirname(fileURLToPath(import.meta.url)), "verify-crystal-semantic-parity-ledger.mjs");
const VERIFIER_VERSION = "crystal-semantic-parity-verifier-v3";
const INVENTORY_SCHEMA = "crystal-semantic-source-inventory-v2";
const EVIDENCE_SCHEMA = "crystal-semantic-evidence-v1";
const CREATED_AT = "2020-01-01T00:00:00Z";
const EXPIRES_AT = "2099-01-01T00:00:00Z";
const CHALLENGE = "a1".repeat(32);
const PACKAGE_ID = "windows-candidate-malicious-fixture";
const CONTROLLED_ROOTS = ["Client", "Server", "Shared"];
const { privateKey, publicKey } = crypto.generateKeyPairSync("rsa", { modulusLength: 3072 });
const PUBLIC_PEM = publicKey.export({ type: "spki", format: "pem" });
const SIGNER_PIN = digest(publicKey.export({ type: "spki", format: "der" }));

function digest(value) { return crypto.createHash("sha256").update(value).digest("hex"); }
function canonical(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
}
function signed(value, key = privateKey) {
  return { ...value, signatureBase64: crypto.sign("sha256", Buffer.from(canonical(value)), { key, padding: crypto.constants.RSA_PKCS1_PADDING }).toString("base64") };
}
function write(filePath, value) { fs.mkdirSync(path.dirname(filePath), { recursive: true }); fs.writeFileSync(filePath, value); }
function writeJson(filePath, value) { write(filePath, `${JSON.stringify(value, null, 2)}\n`); }
function git(cwd, args) { return execFileSync("git", args, { cwd, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim(); }
function initRepo(root) { fs.mkdirSync(root, { recursive: true }); git(root, ["init", "--initial-branch", "main"]); git(root, ["config", "user.email", "semantic-test@example.invalid"]); git(root, ["config", "user.name", "Semantic Test"]); }
function commit(root) { git(root, ["add", "."]); git(root, ["commit", "-m", "fixture"]); return git(root, ["rev-parse", "HEAD"]); }

function makeFixture(options = {}) {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-ledger-adversarial-"));
  const implementationRoot = path.join(base, "implementation");
  const crystalRoot = path.join(base, "Crystal");
  const evidenceRoot = path.join(implementationRoot, "docs", "generated", "crystal-semantic-parity");
  const packageRoot = path.join(implementationRoot, "dist", "mir2-windows-candidate");
  const verifier = path.join(implementationRoot, "apps", "web", "scripts", "verify-crystal-semantic-parity-ledger.mjs");
  initRepo(implementationRoot);
  write(verifier, fs.readFileSync(SOURCE_VERIFIER));
  write(path.join(implementationRoot, ".gitignore"), "docs/generated/crystal-semantic-parity/\ndist/\n");
  write(path.join(implementationRoot, "src", "native.rs"), "pub fn run() {\n}\n");
  if (options.withKey !== false) write(path.join(implementationRoot, "docs", "parity", "trusted-crystal-semantic-parity-signer.pem"), PUBLIC_PEM);
  const policy = {
    schemaVersion: "crystal-semantic-parity-policy-v1",
    policyId: "adversarial-fixture-policy",
    verifierVersion: VERIFIER_VERSION,
    inventorySchemaVersion: INVENTORY_SCHEMA,
    evidenceSchemaVersion: EVIDENCE_SCHEMA,
    crystalRootRelative: "../Crystal",
    controlledRoots: CONTROLLED_ROOTS,
    evidenceRootRelative: "docs/generated/crystal-semantic-parity",
    packageManifestRelative: "dist/mir2-windows-candidate/package-manifest.json",
    trustedSignerPublicKeyRelative: "docs/parity/trusted-crystal-semantic-parity-signer.pem",
    trustedSignerSpkiSha256: SIGNER_PIN,
    challengeRelative: "challenge/expected.json",
    challengeAuthority: "external-one-time-required",
  };
  const policyPath = path.join(implementationRoot, "docs", "parity", "crystal-semantic-parity-policy.json");
  if (options.withPolicy !== false) writeJson(policyPath, policy);
  const policySha256 = options.withPolicy === false ? "9".repeat(64) : digest(fs.readFileSync(policyPath));
  const implementationRevision = commit(implementationRoot);

  initRepo(crystalRoot);
  const sources = {
    "Client/Client.cs": "public class ClientRoot { }\n",
    "Server/Behavior.cs": "public class Behavior {\n    public void Run() { }\n}\n",
    "Shared/Shared.cs": "public class SharedRoot { }\n",
  };
  for (const [relative, content] of Object.entries(sources)) write(path.join(crystalRoot, relative), content);
  const crystalRevision = commit(crystalRoot);
  const sourceFiles = Object.entries(sources).map(([relative, content]) => ({
    path: relative,
    sha256: digest(Buffer.from(content)),
    encoding: "utf-8",
    bytes: Buffer.byteLength(content),
    lineCount: content.split(/\r\n|\n|\r/).length,
    controlledRoot: relative.split("/")[0],
  })).sort((a, b) => a.path.localeCompare(b.path));
  const aggregateSha256 = digest(Buffer.from([...CONTROLLED_ROOTS.map((root) => `root\t${root}\n`), ...sourceFiles.map((file) => `file\t${file.path}\t${file.bytes}\t${file.lineCount}\t${file.sha256}\n`)].join("")));
  const inventory = {
    schemaVersion: INVENTORY_SCHEMA,
    generator: "generate-crystal-semantic-source-inventory.mjs",
    referenceRootRelative: "../Crystal",
    controlledRoots: CONTROLLED_ROOTS,
    crystalRevision,
    sourceRootClean: true,
    sourceFileInventoryComplete: process.platform !== "win32",
    semanticLeafInventoryComplete: false,
    inventoryComplete: false,
    aggregateSha256,
    counts: { controlledRoots: 3, sourceFiles: sourceFiles.length },
    sourceFiles,
  };
  const inventoryPath = path.join(evidenceRoot, "inventory", "report.json");
  writeJson(inventoryPath, inventory);

  const packageBytes = Buffer.from("not-a-real-exe-but-hash-bound");
  const packageFile = path.join(packageRoot, "mir2-platform-windows.exe");
  write(packageFile, packageBytes);
  const packageFiles = [{ path: "mir2-platform-windows.exe", sha256: digest(packageBytes), bytes: packageBytes.length }];
  const packageManifest = signed({
    schemaVersion: "mir2-windows-package-manifest-v1",
    packageIdentity: PACKAGE_ID,
    implementationRevision,
    policySha256,
    challenge: CHALLENGE,
    createdAt: CREATED_AT,
    expiresAt: EXPIRES_AT,
    signerSpkiSha256: SIGNER_PIN,
    aggregateSha256: digest(Buffer.from(packageFiles.map((file) => `${file.path}\t${file.bytes}\t${file.sha256}\n`).join(""))),
    files: packageFiles,
  });
  const packageManifestPath = path.join(packageRoot, "package-manifest.json");
  if (options.withPackage !== false) writeJson(packageManifestPath, packageManifest);

  const challenge = signed({
    schemaVersion: "crystal-semantic-parity-challenge-v1",
    challenge: CHALLENGE,
    issuedAt: CREATED_AT,
    expiresAt: EXPIRES_AT,
    implementationRevision,
    packageIdentity: PACKAGE_ID,
    signerSpkiSha256: SIGNER_PIN,
  });
  const challengePath = path.join(evidenceRoot, "challenge", "expected.json");
  writeJson(challengePath, challenge);

  const evidence = ["CRYSTAL_TRACE", "IMPLEMENTATION_TRACE", "SEMANTIC_DIFF", "PERSISTENCE", "NEGATIVE_TEST"].map((kind, index) => {
    const envelope = signed({
      schemaVersion: EVIDENCE_SCHEMA,
      kind,
      createdAt: CREATED_AT,
      expiresAt: EXPIRES_AT,
      crystalRevision,
      implementationRevision,
      verifierVersion: VERIFIER_VERSION,
      policySha256,
      ...(kind === "IMPLEMENTATION_TRACE" ? { packageIdentity: PACKAGE_ID } : {}),
      challenge: CHALLENGE,
      signerSpkiSha256: SIGNER_PIN,
      payload: payloadFor(kind),
    });
    const relative = `capability/${index}-${kind}.json`;
    const absolute = path.join(evidenceRoot, relative);
    writeJson(absolute, envelope);
    return {
      kind,
      path: relative,
      sha256: digest(fs.readFileSync(absolute)),
      schemaVersion: EVIDENCE_SCHEMA,
      createdAt: CREATED_AT,
      crystalRevision,
      implementationRevision,
      verifierVersion: VERIFIER_VERSION,
      policySha256,
      expiresAt: EXPIRES_AT,
      challenge: CHALLENGE,
      signerPinSha256: SIGNER_PIN,
      ...(kind === "IMPLEMENTATION_TRACE" ? { packageIdentity: PACKAGE_ID } : {}),
    };
  });
  const ledger = {
    schemaVersion: "crystal-semantic-parity-ledger-v1",
    crystalRevision,
    implementationRevision,
    inventoryComplete: false,
    inventoryEvidence: { path: "inventory/report.json", sha256: digest(fs.readFileSync(inventoryPath)), schemaVersion: INVENTORY_SCHEMA, createdAt: CREATED_AT },
    policySha256,
    releasePackageIdentity: PACKAGE_ID,
    capabilities: [{
      id: "MOVE.CPACKET.WALK.ACCEPT",
      domain: "MOVE",
      description: "authoritative walk acceptance",
      severity: "P1",
      crystalSources: [{ path: "Server/Behavior.cs", symbol: "Behavior.Run", lineStart: 1, lineEnd: 3 }],
      dataIdentifiers: [],
      contract: { preconditions: ["authenticated"], inputs: ["CPacket.Walk"], clock: "fixed 50ms tick", rng: "none", stateDeltas: ["position changes"], outbound: ["ObjectWalk"], clientConsequences: ["sprite moves"], persistence: ["position reloads"], negativeCases: ["collision rejects"] },
      implementationSources: [{ path: "src/native.rs", symbol: "run", lineStart: 1, lineEnd: 2 }],
      tests: ["walk_accepts"],
      evidence,
      knownDeviations: [],
      status: "VERIFIED",
      verifiedRevision: implementationRevision,
      packageIdentity: PACKAGE_ID,
    }],
  };
  const ledgerPath = path.join(evidenceRoot, "ledger.json");
  writeJson(ledgerPath, ledger);
  return { base, implementationRoot, crystalRoot, evidenceRoot, packageRoot, verifier, ledgerPath, ledger, inventory, inventoryPath, policyPath, challengePath, packageManifestPath };
}

function payloadFor(kind) {
  const h = "c".repeat(64);
  if (kind === "CRYSTAL_TRACE" || kind === "IMPLEMENTATION_TRACE") return { traceSchema: "ordered-trace-v1", eventCount: 1, orderedDigestSha256: h };
  if (kind === "SEMANTIC_DIFF") return { matches: true, differenceCount: 0, diffSha256: h };
  if (kind === "PERSISTENCE") return { beforeSha256: h, afterSha256: h, reloadMatches: true };
  return { testCount: 1, failedCount: 0, reportSha256: h };
}
function inventoryAggregate(sourceFiles) { return digest(Buffer.from([...CONTROLLED_ROOTS.map((root) => `root\t${root}\n`), ...sourceFiles.map((file) => `file\t${file.path}\t${file.bytes}\t${file.lineCount}\t${file.sha256}\n`)].join(""))); }
function saveLedger(f) { writeJson(f.ledgerPath, f.ledger); }
function updateInventory(f) { writeJson(f.inventoryPath, f.inventory); f.ledger.inventoryEvidence.sha256 = digest(fs.readFileSync(f.inventoryPath)); saveLedger(f); }
function updateEvidence(f, index, value) { const reference = f.ledger.capabilities[0].evidence[index]; writeJson(path.join(f.evidenceRoot, reference.path), value); reference.sha256 = digest(fs.readFileSync(path.join(f.evidenceRoot, reference.path))); saveLedger(f); }
function run(f, extra = []) { return spawnSync(process.execPath, [f.verifier, f.ledgerPath, ...extra], { encoding: "utf8" }); }
function completeArgs(f) { return ["--require-complete", "--implementation-root", f.implementationRoot, "--crystal-root", f.crystalRoot, "--evidence-root", f.evidenceRoot]; }
function cleanup(f) { fs.rmSync(f.base, { recursive: true, force: true }); }

test("ordinary mode never counts claimed VERIFIED rows as formal VERIFIED", () => {
  const f = makeFixture();
  try { const result = run(f); assert.equal(result.status, 0, result.stderr); assert.match(result.stdout, process.platform === "win32" ? /SOURCE_FILE_INVENTORY_INCOMPLETE/ : /SEMANTIC_INVENTORY_INCOMPLETE/); assert.match(result.stdout, /sourceRootClean=true/); assert.match(result.stdout, /claimedMarkedVerified=1/); assert.match(result.stdout, /formalVerified=0/); }
  finally { cleanup(f); }
});

test("clean source-file inventory is not semantic inventory completion", () => {
  const f = makeFixture();
  try {
    f.inventory.semanticLeafInventoryComplete = true;
    f.inventory.inventoryComplete = true;
    f.ledger.inventoryComplete = true;
    updateInventory(f);
    const result = run(f);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /unsupported\/trusted semantic leaf inventory.*must remain false/);
  } finally { cleanup(f); }
});

test("scoped Git status ignores a dirty sibling but detects target dirtiness and repository-root dirtiness", () => {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "mir2-ledger-scoped-git-"));
  const crystalRoot = path.join(base, "Crystal");
  const implementationRoot = path.join(base, "mir2-web3");
  try {
    initRepo(base);
    write(path.join(crystalRoot, "Server", "Behavior.cs"), "class Behavior { }\n");
    write(path.join(implementationRoot, "apps", "web", "scripts", "native.mjs"), "export {};\n");
    commit(base);
    assert.equal(gitInfo(crystalRoot, "Crystal root").clean, true);
    write(path.join(implementationRoot, "apps", "web", "scripts", "sibling-only.mjs"), "export {};\n");
    assert.equal(gitInfo(crystalRoot, "Crystal root").clean, true);
    assert.equal(gitInfo(implementationRoot, "implementation root").clean, false);
    assert.equal(gitInfo(base, "repository root").clean, false);
    write(path.join(crystalRoot, "Server", "Behavior.cs"), "class Behavior { changed(); }\n");
    assert.equal(gitInfo(crystalRoot, "Crystal root").clean, false);
  } finally { fs.rmSync(base, { recursive: true, force: true }); }
});

test("ordinary mode verifies a scoped-dirty inventory as incomplete and formal mode rejects it", () => {
  const f = makeFixture();
  try {
    const dirtyPath = path.join(f.crystalRoot, "Server", "Behavior.cs");
    const dirtyContent = `${fs.readFileSync(dirtyPath, "utf8")}// dirty\n`;
    fs.writeFileSync(dirtyPath, dirtyContent);
    const entry = f.inventory.sourceFiles.find((source) => source.path === "Server/Behavior.cs");
    entry.sha256 = digest(Buffer.from(dirtyContent));
    entry.bytes = Buffer.byteLength(dirtyContent);
    entry.lineCount = dirtyContent.split(/\r\n|\n|\r/).length;
    f.inventory.sourceRootClean = false;
    f.inventory.sourceFileInventoryComplete = false;
    f.inventory.aggregateSha256 = inventoryAggregate(f.inventory.sourceFiles);
    f.inventory.inventoryComplete = false;
    f.ledger.inventoryComplete = false;
    updateInventory(f);
    let result = run(f);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /SOURCE_FILE_INVENTORY_INCOMPLETE/);
    assert.match(result.stdout, /sourceRootClean=false/);
    assert.match(result.stdout, /formalVerified=0/);
    result = run(f, completeArgs(f));
    assert.equal(result.status, 1);
    assert.match(result.stderr, /BLOCKED.*scoped worktree is dirty/);
  } finally { cleanup(f); }
});

test("ordinary mode rejects forged sourceRootClean or inventoryComplete claims", () => {
  const f = makeFixture();
  try {
    fs.appendFileSync(path.join(f.crystalRoot, "Server", "Behavior.cs"), "// dirty\n");
    let result = run(f);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /sourceRootClean/);
    f.inventory.sourceRootClean = false;
    updateInventory(f);
    result = run(f);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /sourceFileInventoryComplete|sourceFiles|aggregate/);
  } finally { cleanup(f); }
});

test("formal mode is BLOCKED when semantic leaf inventory is unavailable", () => {
  const f = makeFixture();
  try { const result = run(f, completeArgs(f)); assert.equal(result.status, 1); assert.match(result.stderr, /BLOCKED.*semantic inventory/); assert.doesNotMatch(result.stdout, /COMPLETE|formalVerified=[1-9]/); }
  finally { cleanup(f); }
});

test("inventory cannot self-report completion or forge files/count/aggregate", () => {
  const f = makeFixture();
  try {
    f.inventory.sourceFiles.pop(); f.inventory.counts.sourceFiles -= 1; f.inventory.aggregateSha256 = "0".repeat(64); updateInventory(f);
    const result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /sourceFiles|aggregate/);
    delete f.inventory.sourceFiles; updateInventory(f); const missing = run(f); assert.equal(missing.status, 1); assert.match(missing.stderr, /sourceFiles/);
  } finally { cleanup(f); }
});

test("inventory rejects caller-selected roots and wrong controlled-root schema", () => {
  const f = makeFixture();
  try {
    f.inventory.controlledRoots = ["Server", "Shared", "tools"]; updateInventory(f);
    const wrongInventory = run(f); assert.equal(wrongInventory.status, 1); assert.match(wrongInventory.stderr, /controlledRoots/);
    const other = path.join(f.base, "other"); fs.mkdirSync(other); const wrongRoot = run(f, ["--evidence-root", other]); assert.equal(wrongRoot.status, 1); assert.match(wrongRoot.stderr, /repository-fixed|caller-selected/);
  } finally { cleanup(f); }
});

test("all modes reject missing, hash-mismatched, expired, or string evidence", () => {
  const f = makeFixture();
  try {
    f.ledger.capabilities[0].evidence[0].sha256 = "0".repeat(64); saveLedger(f); let result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /sha256/);
    const ref = f.ledger.capabilities[0].evidence[0]; fs.unlinkSync(path.join(f.evidenceRoot, ref.path)); result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /does not exist/);
    write(path.join(f.evidenceRoot, ref.path), "just a string"); ref.sha256 = digest(Buffer.from("just a string")); saveLedger(f); result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /JSON|object|value at byte/);
  } finally { cleanup(f); }
});

test("type-specific evidence rejects an empty object payload", () => {
  const f = makeFixture();
  try {
    const ref = f.ledger.capabilities[0].evidence[2]; const envelope = JSON.parse(fs.readFileSync(path.join(f.evidenceRoot, ref.path), "utf8")); envelope.payload = {}; delete envelope.signatureBase64; updateEvidence(f, 2, signed(envelope));
    const result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /payload.*missing|unexpected|matches/);
  } finally { cleanup(f); }
});

test("all modes reject expired evidence even without complete flag", () => {
  const f = makeFixture();
  try {
    const ref = f.ledger.capabilities[0].evidence[0]; const envelope = JSON.parse(fs.readFileSync(path.join(f.evidenceRoot, ref.path), "utf8")); ref.expiresAt = "2020-01-01T00:00:00Z"; envelope.expiresAt = ref.expiresAt; delete envelope.signatureBase64; updateEvidence(f, 0, signed(envelope));
    const result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /expired/);
  } finally { cleanup(f); }
});

test("unsafe absolute/traversal paths, bad spans, and mismatched domain are rejected", () => {
  const f = makeFixture();
  try {
    f.ledger.inventoryEvidence.path = "../escape.json"; saveLedger(f); let result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /unsafe|relative/);
    f.ledger.inventoryEvidence.path = "inventory/report.json"; f.ledger.capabilities[0].crystalSources[0].lineStart = 5; f.ledger.capabilities[0].crystalSources[0].lineEnd = 2; saveLedger(f); result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /lineStart/);
    f.ledger.capabilities[0].crystalSources[0].lineStart = 1; f.ledger.capabilities[0].crystalSources[0].lineEnd = 3; f.ledger.capabilities[0].domain = "AUTH"; saveLedger(f); result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /domain/);
  } finally { cleanup(f); }
});

test("safeRelative rejects control characters, trailing dot/space, and Windows DOS device basenames", () => {
  for (const badPath of ["Server/worker. ", "Server/worker.", "Server/CON.txt", "Server/aux.md", "Server/COM1.log", "Server/Lpt9.data", "Server/bad\u0001.cs"]) {
    const f = makeFixture();
    try {
      f.ledger.capabilities[0].crystalSources[0].path = badPath;
      saveLedger(f);
      const result = run(f);
      assert.equal(result.status, 1);
      assert.match(result.stderr, /control character|segment ending in a dot or space|DOS device basename/);
    } finally { cleanup(f); }
  }
});

test("inventory source paths reject Windows case-fold collisions even on case-sensitive filesystems", () => {
  const f = makeFixture();
  try {
    const duplicate = { ...f.inventory.sourceFiles.find((source) => source.path === "Client/Client.cs"), path: "Client/client.cs" };
    f.inventory.sourceFiles.push(duplicate);
    f.inventory.sourceFiles.sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
    f.inventory.counts.sourceFiles += 1;
    updateInventory(f);
    const result = run(f);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /case-folding/);
  } finally { cleanup(f); }
});

test("formal mode does not reach fixed policy/package/key gates while semantic inventory is incomplete", () => {
  for (const option of [{ withPolicy: false }, { withPackage: false }, { withKey: false }]) {
    const f = makeFixture(option);
    try { const result = run(f, completeArgs(f)); assert.equal(result.status, 1); assert.match(result.stderr, /BLOCKED.*semantic inventory/); assert.doesNotMatch(result.stdout, /COMPLETE/); }
    finally { cleanup(f); }
  }
});

test("formal mode rejects wrong signer, challenge, verifier version, and package binding", () => {
  const f = makeFixture();
  try {
    const ref = f.ledger.capabilities[0].evidence[0]; ref.signerPinSha256 = "d".repeat(64); saveLedger(f); let result = run(f, completeArgs(f)); assert.equal(result.status, 1); assert.match(result.stderr, /signer/);
    ref.signerPinSha256 = SIGNER_PIN; ref.challenge = "e".repeat(64); saveLedger(f); result = run(f, completeArgs(f)); assert.equal(result.status, 1); assert.match(result.stderr, /challenge/);
    ref.challenge = CHALLENGE; ref.verifierVersion = "caller-verifier"; saveLedger(f); result = run(f, completeArgs(f)); assert.equal(result.status, 1); assert.match(result.stderr, /verifierVersion/);
    ref.verifierVersion = VERIFIER_VERSION; f.ledger.capabilities[0].packageIdentity = "wrong-package"; saveLedger(f); result = run(f, completeArgs(f)); assert.equal(result.status, 1); assert.match(result.stderr, /semantic inventory/);
  } finally { cleanup(f); }
});

test("forged dirty inventory and symlink/reparse evidence are rejected", (t) => {
  const f = makeFixture();
  try {
    fs.appendFileSync(path.join(f.crystalRoot, "Server", "Behavior.cs"), "// dirty\n"); let result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /sourceRootClean/);
    git(f.crystalRoot, ["checkout", "--", "Server/Behavior.cs"]);
    const target = path.join(f.evidenceRoot, "inventory", "report.json"); const link = path.join(f.evidenceRoot, "inventory", "link.json");
    try { fs.symlinkSync(target, link, "file"); } catch (error) { t.skip(`symlink unavailable: ${error.code ?? error.message}`); return; }
    f.ledger.inventoryEvidence.path = "inventory/link.json"; saveLedger(f); result = run(f); assert.equal(result.status, 1); assert.match(result.stderr, /symlink|reparse/);
  } finally { cleanup(f); }
});

test("formal mode rejects forged evidence and package signatures before trust when semantic inventory is incomplete", () => {
  const f = makeFixture();
  try {
    const ref = f.ledger.capabilities[0].evidence[0];
    const envelopePath = path.join(f.evidenceRoot, ref.path);
    const envelope = JSON.parse(fs.readFileSync(envelopePath, "utf8"));
    envelope.signatureBase64 = Buffer.alloc(384, 7).toString("base64");
    updateEvidence(f, 0, envelope);
    let result = run(f, completeArgs(f));
    assert.equal(result.status, 1);
    assert.match(result.stderr, /semantic inventory/);

    const fresh = makeFixture();
    try {
      const manifest = JSON.parse(fs.readFileSync(fresh.packageManifestPath, "utf8"));
      manifest.signatureBase64 = Buffer.alloc(384, 9).toString("base64");
      writeJson(fresh.packageManifestPath, manifest);
      result = run(fresh, completeArgs(fresh));
      assert.equal(result.status, 1);
      assert.match(result.stderr, /semantic inventory/);
    } finally {
      cleanup(fresh);
    }
  } finally {
    cleanup(f);
  }
});
