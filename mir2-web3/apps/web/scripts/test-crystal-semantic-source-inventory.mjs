import assert from "node:assert/strict";
import { execFile as execFileCallback } from "node:child_process";
import { createHash } from "node:crypto";
import { constants as fsConstants } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

import {
  buildCrystalSemanticSourceInventory,
  computeInventoryAggregate,
  DEFAULT_CONTROLLED_ROOTS,
  deriveInventoryCompletion,
  INVENTORY_SCHEMA_VERSION,
  REFERENCE_ROOT_RELATIVE,
} from "./generate-crystal-semantic-source-inventory.mjs";

const execFile = promisify(execFileCallback);
const GENERATOR_PATH = path.resolve(import.meta.dirname, "generate-crystal-semantic-source-inventory.mjs");
const VERIFIER_PATH = path.resolve(import.meta.dirname, "verify-crystal-semantic-parity-ledger.mjs");
const TOP_LEVEL_KEYS = [
  "schemaVersion",
  "generator",
  "referenceRootRelative",
  "controlledRoots",
  "crystalRevision",
  "sourceRootClean",
  "sourceFileInventoryComplete",
  "semanticLeafInventoryComplete",
  "inventoryComplete",
  "aggregateSha256",
  "counts",
  "sourceFiles",
];
const SOURCE_FILE_KEYS = ["path", "sha256", "encoding", "bytes", "lineCount", "controlledRoot"];

async function makeTemporaryDirectory(t, prefix) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), prefix));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  return directory;
}

async function initRepository(root) {
  await fs.mkdir(root, { recursive: true });
  await git(root, ["init", "--initial-branch", "main"]);
  await git(root, ["config", "user.email", "inventory-test@example.invalid"]);
  await git(root, ["config", "user.name", "Inventory Test"]);
}

async function commitAll(root, message = "fixture") {
  await git(root, ["add", "."]);
  await git(root, ["commit", "-m", message]);
  return git(root, ["rev-parse", "HEAD"]);
}

async function writeCrystalSources(root) {
  const sources = {
    "Client/Client.cs": "public class ClientRoot { }\n",
    "Server/Behavior.cs": "public class Behavior {\n    public void Run() { }\n}\n",
    "Server/Zeta.cs": "public enum ClientPacketIds { Alpha = 1, Zed = 2 }\n",
    "Shared/Shared.cs": "public class SharedRoot { }\n",
  };
  for (const [relative, contents] of Object.entries(sources)) {
    const absolute = path.join(root, ...relative.split("/"));
    await fs.mkdir(path.dirname(absolute), { recursive: true });
    await fs.writeFile(absolute, contents);
  }
  return sources;
}

async function makeCrystalFixture(t, prefix = "mir2-inventory-v2-") {
  const base = await makeTemporaryDirectory(t, prefix);
  const root = path.join(base, "Crystal");
  await initRepository(root);
  const sources = await writeCrystalSources(root);
  const revision = await commitAll(root);
  return { base, root, sources, revision };
}

async function makeMonorepoFixture(t, prefix = "mir2-inventory-monorepo-") {
  const base = await makeTemporaryDirectory(t, prefix);
  const repositoryRoot = path.join(base, "repository");
  const crystalRoot = path.join(repositoryRoot, "Crystal");
  const siblingRoot = path.join(repositoryRoot, "Sibling");
  await initRepository(repositoryRoot);
  const sources = await writeCrystalSources(crystalRoot);
  await fs.mkdir(siblingRoot, { recursive: true });
  await fs.writeFile(path.join(siblingRoot, "Sibling.txt"), "committed sibling\n");
  const revision = await commitAll(repositoryRoot);
  return { base, repositoryRoot, crystalRoot, siblingRoot, sources, revision };
}

function digest(value) {
  return createHash("sha256").update(value).digest("hex");
}

function serialized(value) {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function manualAggregate(sourceFiles) {
  return digest(Buffer.from([
    ...DEFAULT_CONTROLLED_ROOTS.map((root) => `root\t${root}\n`),
    ...sourceFiles.map((file) =>
      `file\t${file.path}\t${file.bytes}\t${file.lineCount}\t${file.sha256}\n`),
  ].join(""), "utf8"));
}

function assertStrictInventoryShape(report) {
  assert.deepEqual(Object.keys(report), TOP_LEVEL_KEYS);
  assert.equal(report.schemaVersion, INVENTORY_SCHEMA_VERSION);
  assert.equal(report.generator, "generate-crystal-semantic-source-inventory.mjs");
  assert.equal(report.referenceRootRelative, REFERENCE_ROOT_RELATIVE);
  assert.deepEqual(report.controlledRoots, ["Client", "Server", "Shared"]);
  assert.deepEqual(Object.keys(report.counts), ["controlledRoots", "sourceFiles"]);
  assert.equal(report.counts.controlledRoots, 3);
  assert.equal(report.counts.sourceFiles, report.sourceFiles.length);
  assert.equal(typeof report.sourceRootClean, "boolean");
  assert.equal(typeof report.sourceFileInventoryComplete, "boolean");
  assert.equal(report.semanticLeafInventoryComplete, false);
  assert.equal(
    report.inventoryComplete,
    report.sourceFileInventoryComplete && report.semanticLeafInventoryComplete,
  );
  for (const file of report.sourceFiles) {
    assert.deepEqual(Object.keys(file), SOURCE_FILE_KEYS);
    assert.match(file.path, /^(Client|Server|Shared)\/.+\.cs$/i);
    assert.equal(path.isAbsolute(file.path), false);
    assert.equal(file.path.includes("\\"), false);
    assert.match(file.sha256, /^[0-9a-f]{64}$/);
    assert.equal(file.encoding, "utf-8");
    assert.ok(Number.isInteger(file.bytes) && file.bytes >= 0);
    assert.ok(Number.isInteger(file.lineCount) && file.lineCount >= 0);
    assert.ok(file.path.startsWith(`${file.controlledRoot}/`));
  }
}

test("strict inventory v2 is deterministic and matches verifier roots/count/aggregate", async (t) => {
  const fixture = await makeCrystalFixture(t);
  const first = await buildCrystalSemanticSourceInventory({
    referenceRoot: fixture.root,
    controlledRoots: ["Shared", "Client", "Server"],
  });
  const second = await buildCrystalSemanticSourceInventory({ referenceRoot: fixture.root });
  const relocatedRoot = path.join(fixture.base, "RelocatedCrystal");
  await fs.cp(fixture.root, relocatedRoot, { recursive: true });
  const relocated = await buildCrystalSemanticSourceInventory({ referenceRoot: relocatedRoot });

  assertStrictInventoryShape(first);
  assert.deepEqual(first, second);
  assert.deepEqual(first, relocated);
  assert.equal(digest(serialized(first)), digest(serialized(relocated)));
  assert.equal(first.crystalRevision, fixture.revision);
  assert.equal(first.sourceRootClean, true);
  assert.equal(first.sourceFiles.length, 4);
  assert.deepEqual(first.sourceFiles.map((file) => file.path), [
    "Client/Client.cs",
    "Server/Behavior.cs",
    "Server/Zeta.cs",
    "Shared/Shared.cs",
  ]);
  assert.equal(first.aggregateSha256, manualAggregate(first.sourceFiles));
  assert.equal(first.aggregateSha256, computeInventoryAggregate(first.controlledRoots, first.sourceFiles));
  assert.equal(JSON.stringify(first).includes(fixture.root), false);
  assert.equal(first.semanticLeafInventoryComplete, false);
  assert.equal(first.inventoryComplete, false);
  if (process.platform === "win32" || (fsConstants.O_NOFOLLOW ?? 0) === 0) {
    assert.equal(first.sourceFileInventoryComplete, false);
  } else {
    assert.equal(first.sourceFileInventoryComplete, true);
  }
});

test("semantic completion is never self-declared even when the file layer can be complete", () => {
  assert.deepEqual(deriveInventoryCompletion(true, true), {
    sourceFileInventoryComplete: true,
    semanticLeafInventoryComplete: false,
    inventoryComplete: false,
  });
  assert.deepEqual(deriveInventoryCompletion(true, false), {
    sourceFileInventoryComplete: false,
    semanticLeafInventoryComplete: false,
    inventoryComplete: false,
  });
  assert.throws(() => deriveInventoryCompletion(true, "true"), /inputs must be booleans/);
});

test("controlled roots require exactly three trusted roots and reject Windows path aliases", async (t) => {
  const fixture = await makeCrystalFixture(t, "mir2-inventory-roots-");
  const cases = [
    [["Client", "client", "Server", "Shared"], /Duplicate.*Windows case folding/i],
    [["Client", "client/Nested", "Server", "Shared"], /Overlapping.*Windows case folding/i],
    [["Client", "Server"], /exactly Client\/Server\/Shared/i],
    [["Client", "Server", "Shared", "tools"], /exactly Client\/Server\/Shared/i],
    [["../Client", "Server", "Shared"], /unsafe path segments/i],
    [["Client", "Server", "Shared/\u0001hidden"], /control character/i],
    [["Client", "Server", "Shared/Trailing."], /ending in a dot or space/i],
    [["Client", "Server", "Shared/Trailing "], /ending in a dot or space/i],
    [[path.resolve(fixture.root, "Client"), "Server", "Shared"], /unsafe characters|must be relative/i],
  ];
  for (const [controlledRoots, expected] of cases) {
    await assert.rejects(
      buildCrystalSemanticSourceInventory({ referenceRoot: fixture.root, controlledRoots }),
      expected,
    );
  }

  const deviceBasenames = [
    "CON",
    "prn.txt",
    "AUX.cs",
    "nul.data",
    "CLOCK$.cs",
    "com1.cs",
    "COM9.any",
    "lpt1.cs",
    "LPT9.more",
  ];
  for (const deviceBasename of deviceBasenames) {
    await assert.rejects(
      buildCrystalSemanticSourceInventory({
        referenceRoot: fixture.root,
        controlledRoots: ["Client", "Server", "Shared/" + deviceBasename],
      }),
      /Windows DOS device basename/i,
    );
  }
});

test("source paths reject DOS aliases and Windows case-fold collisions", async (t) => {
  if (process.platform !== "win32") {
    const reserved = await makeCrystalFixture(t, "mir2-inventory-device-alias-");
    await fs.writeFile(path.join(reserved.root, "Server", "CON.cs"), "public class Alias { }\n");
    await commitAll(reserved.root, "reserved source alias");
    await assert.rejects(
      buildCrystalSemanticSourceInventory({ referenceRoot: reserved.root }),
      /Windows DOS device basename/i,
    );
  } else {
    t.diagnostic("DOS device source fixture is unavailable on Windows; controlled-root cases cover validation");
  }

  const collision = await makeCrystalFixture(t, "mir2-inventory-case-fold-");
  const serverRoot = path.join(collision.root, "Server");
  await fs.writeFile(path.join(serverRoot, "Foo.cs"), "public class UpperAlias { }\n");
  await fs.writeFile(path.join(serverRoot, "foo.cs"), "public class LowerAlias { }\n");
  const names = await fs.readdir(serverRoot);
  if (!names.includes("Foo.cs") || !names.includes("foo.cs")) {
    t.diagnostic("case-sensitive source fixture is unavailable on this filesystem");
    return;
  }
  await commitAll(collision.root, "case-fold collision");
  await assert.rejects(
    buildCrystalSemanticSourceInventory({ referenceRoot: collision.root }),
    /unique under Windows case[- ]folding/i,
  );
});

test("a non-Git Crystal root fails closed", async (t) => {
  const base = await makeTemporaryDirectory(t, "mir2-inventory-non-git-");
  const root = path.join(base, "Crystal");
  await writeCrystalSources(root);
  await assert.rejects(
    buildCrystalSemanticSourceInventory({ referenceRoot: root }),
    /must be a Git worktree with a valid HEAD/,
  );
});

test("source symlinks/reparse points and invalid UTF-8 fail closed", async (t) => {
  const invalid = await makeCrystalFixture(t, "mir2-inventory-encoding-");
  await fs.writeFile(path.join(invalid.root, "Server", "Broken.cs"), Buffer.from([0xc3, 0x28]));
  await commitAll(invalid.root, "invalid utf8");
  await assert.rejects(
    buildCrystalSemanticSourceInventory({ referenceRoot: invalid.root }),
    /not UTF-8/,
  );

  const linked = await makeCrystalFixture(t, "mir2-inventory-link-");
  const outside = path.join(linked.base, "Outside.cs");
  await fs.writeFile(outside, "public class Outside { }\n");
  const link = path.join(linked.root, "Client", "Escape.cs");
  try {
    await fs.symlink(outside, link, "file");
  } catch (error) {
    t.diagnostic(`source symlink fixture unavailable: ${error.code ?? error.message}`);
    return;
  }
  await assert.rejects(
    buildCrystalSemanticSourceInventory({ referenceRoot: linked.root }),
    /symlink|reparse/i,
  );
});

test("dirty Crystal binds HEAD but cannot claim clean or complete", async (t) => {
  const fixture = await makeCrystalFixture(t, "mir2-inventory-dirty-");
  await fs.appendFile(path.join(fixture.root, "Server", "Behavior.cs"), "// dirty\n");
  const report = await buildCrystalSemanticSourceInventory({ referenceRoot: fixture.root });
  assert.equal(report.crystalRevision, fixture.revision);
  assert.equal(report.sourceRootClean, false);
  assert.equal(report.sourceFileInventoryComplete, false);
  assert.equal(report.semanticLeafInventoryComplete, false);
  assert.equal(report.inventoryComplete, false);
  assert.equal(report.aggregateSha256, manualAggregate(report.sourceFiles));
});

test("scoped Git status ignores dirty siblings but detects dirty Crystal", async (t) => {
  const fixture = await makeMonorepoFixture(t);
  await fs.appendFile(path.join(fixture.siblingRoot, "Sibling.txt"), "dirty sibling\n");

  const siblingDirty = await buildCrystalSemanticSourceInventory({
    referenceRoot: fixture.crystalRoot,
  });
  assert.equal(siblingDirty.crystalRevision, fixture.revision);
  assert.equal(siblingDirty.sourceRootClean, true);
  assert.equal(siblingDirty.semanticLeafInventoryComplete, false);
  assert.equal(siblingDirty.inventoryComplete, false);

  await fs.appendFile(path.join(fixture.crystalRoot, "Server", "Behavior.cs"), "// dirty Crystal\n");
  const crystalDirty = await buildCrystalSemanticSourceInventory({
    referenceRoot: fixture.crystalRoot,
  });
  assert.equal(crystalDirty.crystalRevision, fixture.revision);
  assert.equal(crystalDirty.sourceRootClean, false);
  assert.equal(crystalDirty.sourceFileInventoryComplete, false);
  assert.equal(crystalDirty.semanticLeafInventoryComplete, false);
  assert.equal(crystalDirty.inventoryComplete, false);
});

test("scoped Crystal status changes during scanning are rejected", async (t) => {
  const fixture = await makeMonorepoFixture(t, "mir2-inventory-status-race-");
  await assert.rejects(
    buildCrystalSemanticSourceInventory({
      referenceRoot: fixture.crystalRoot,
      testHooks: {
        beforeFinalGitStatus: async () => {
          await fs.appendFile(
            path.join(fixture.crystalRoot, "Server", "Behavior.cs"),
            "// changed after source scan\n",
          );
        },
      },
    }),
    /scoped worktree changed while inventory was being generated/,
  );
});

test("CLI output is exclusive, quiet, parent-safe, and leaves no temporary files", async (t) => {
  const fixture = await makeCrystalFixture(t, "mir2-inventory-output-");
  const outputPath = path.join(fixture.base, "inventory.json");
  const first = await execFile(process.execPath, [
    GENERATOR_PATH,
    "--root", fixture.root,
    "--output", outputPath,
    "--quiet",
  ]);
  assert.equal(first.stdout, "");
  assert.match(first.stderr, /^path=.* sha256=[0-9a-f]{64} aggregate=[0-9a-f]{64} counts=controlledRoots:3,sourceFiles:\d+\n$/);
  const original = await fs.readFile(outputPath);
  const originalHash = digest(original);
  const entriesBefore = (await fs.readdir(fixture.base)).sort();

  await assert.rejects(
    execFile(process.execPath, [GENERATOR_PATH, "--root", fixture.root, "--output", outputPath, "--quiet"]),
    (error) => error.code === 1 && /already exists|EEXIST/i.test(error.stderr),
  );
  assert.equal(digest(await fs.readFile(outputPath)), originalHash);
  assert.deepEqual((await fs.readdir(fixture.base)).sort(), entriesBefore);
  assert.equal(entriesBefore.some((entry) => /\.tmp|\.partial/i.test(entry)), false);

  const missingParent = path.join(fixture.base, "missing", "inventory.json");
  await assert.rejects(
    execFile(process.execPath, [GENERATOR_PATH, "--root", fixture.root, "--output", missingParent, "--quiet"]),
    (error) => error.code === 1 && /output parent.*does not exist/i.test(error.stderr),
  );
  await assert.rejects(fs.access(path.dirname(missingParent)));

  await assert.rejects(
    execFile(process.execPath, [GENERATOR_PATH, "--root", fixture.root, "--quiet"]),
    (error) => error.code === 1 && /--quiet requires --output/.test(error.stderr),
  );
  await assert.rejects(
    execFile(process.execPath, [
      GENERATOR_PATH,
      "--root", fixture.root,
      "--semantic-leaf-inventory-complete", "true",
    ]),
    (error) => error.code === 1 && /Unknown argument/.test(error.stderr),
  );

  const realParent = path.join(fixture.base, "real-parent");
  const linkedParent = path.join(fixture.base, "linked-parent");
  await fs.mkdir(realParent);
  try {
    await fs.symlink(realParent, linkedParent, "junction");
  } catch (error) {
    t.diagnostic(`output parent symlink fixture unavailable: ${error.code ?? error.message}`);
    return;
  }
  await assert.rejects(
    execFile(process.execPath, [GENERATOR_PATH, "--root", fixture.root, "--output", path.join(linkedParent, "inventory.json"), "--quiet"]),
    (error) => error.code === 1 && /symlink|reparse/i.test(error.stderr),
  );
});

test("generated inventory interoperates with Planck verifier and tampering is rejected", async (t) => {
  const verifierSource = await fs.readFile(VERIFIER_PATH, "utf8");
  if (!verifierSource.includes("sourceFileInventoryComplete")
      || !verifierSource.includes("semanticLeafInventoryComplete")) {
    t.skip("Planck verifier semantic completion gates update is still pending");
    return;
  }

  const base = await makeTemporaryDirectory(t, "mir2-inventory-interop-");
  const implementationRoot = path.join(base, "implementation");
  const crystalRoot = path.join(base, "Crystal");
  const copiedVerifier = path.join(implementationRoot, "apps", "web", "scripts", "verify-crystal-semantic-parity-ledger.mjs");
  const evidenceRoot = path.join(implementationRoot, "docs", "generated", "crystal-semantic-parity");
  const inventoryPath = path.join(evidenceRoot, "inventory", "report.json");
  const ledgerPath = path.join(evidenceRoot, "ledger.json");

  await initRepository(implementationRoot);
  await fs.mkdir(path.dirname(copiedVerifier), { recursive: true });
  await fs.copyFile(VERIFIER_PATH, copiedVerifier);
  await fs.writeFile(path.join(implementationRoot, ".gitignore"), "docs/generated/crystal-semantic-parity/\n");
  const implementationRevision = await commitAll(implementationRoot);

  await initRepository(crystalRoot);
  await writeCrystalSources(crystalRoot);
  const crystalRevision = await commitAll(crystalRoot);
  const inventory = await buildCrystalSemanticSourceInventory({ referenceRoot: crystalRoot });
  assert.equal(inventory.crystalRevision, crystalRevision);
  assert.equal(inventory.sourceRootClean, true);
  const firstSourceSha256 = inventory.sourceFiles[0].sha256;

  const ledger = {
    schemaVersion: "crystal-semantic-parity-ledger-v1",
    crystalRevision,
    implementationRevision,
    inventoryComplete: inventory.inventoryComplete,
    inventoryEvidence: {
      path: "inventory/report.json",
      sha256: "",
      schemaVersion: INVENTORY_SCHEMA_VERSION,
      createdAt: "2020-01-01T00:00:00Z",
    },
    policySha256: "0".repeat(64),
    releasePackageIdentity: null,
    capabilities: [],
  };

  const persist = async () => {
    await fs.mkdir(path.dirname(inventoryPath), { recursive: true });
    const bytes = Buffer.from(serialized(inventory), "utf8");
    await fs.writeFile(inventoryPath, bytes);
    ledger.inventoryEvidence.sha256 = digest(bytes);
    await fs.writeFile(ledgerPath, serialized(ledger));
  };

  await persist();
  const accepted = await execFile(process.execPath, [copiedVerifier, ledgerPath]);
  assert.match(accepted.stdout, /SOURCE_FILE_INVENTORY_INCOMPLETE|SEMANTIC_INVENTORY_INCOMPLETE/);
  assert.equal(accepted.stderr, "");

  inventory.aggregateSha256 = "0".repeat(64);
  await persist();
  await assert.rejects(
    execFile(process.execPath, [copiedVerifier, ledgerPath]),
    (error) => error.code === 1 && /aggregateSha256 mismatch/.test(error.stderr),
  );

  inventory.aggregateSha256 = manualAggregate(inventory.sourceFiles);
  inventory.sourceFiles[0].sha256 = "f".repeat(64);
  inventory.aggregateSha256 = manualAggregate(inventory.sourceFiles);
  await persist();
  await assert.rejects(
    execFile(process.execPath, [copiedVerifier, ledgerPath]),
    (error) => error.code === 1 && /sourceFiles do not match/.test(error.stderr),
  );

  inventory.sourceFiles[0].sha256 = firstSourceSha256;
  inventory.aggregateSha256 = manualAggregate(inventory.sourceFiles);
  inventory.semanticLeafInventoryComplete = true;
  inventory.inventoryComplete = true;
  ledger.inventoryComplete = true;
  await persist();
  await assert.rejects(
    execFile(process.execPath, [copiedVerifier, ledgerPath]),
    (error) => error.code === 1
      && /semanticLeafInventoryComplete|semantic leaf|inventoryComplete/i.test(error.stderr),
  );
});

async function git(cwd, args) {
  const { stdout } = await execFile("git", args, { cwd, encoding: "utf8" });
  return stdout.trim();
}