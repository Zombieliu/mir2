import { execFile as execFileCallback } from "node:child_process";
import { createHash } from "node:crypto";
import { constants as fsConstants } from "node:fs";
import { lstat, open, readdir, realpath } from "node:fs/promises";
import path from "node:path";
import { promisify, TextDecoder } from "node:util";
import { fileURLToPath, pathToFileURL } from "node:url";

const execFile = promisify(execFileCallback);

export const INVENTORY_SCHEMA_VERSION = "crystal-semantic-source-inventory-v2";
export const DEFAULT_CONTROLLED_ROOTS = Object.freeze(["Client", "Server", "Shared"]);
export const REFERENCE_ROOT_RELATIVE = "../Crystal";

const SCRIPT_PATH = fileURLToPath(import.meta.url);
const WEB_ROOT = path.resolve(path.dirname(SCRIPT_PATH), "..");
const IMPLEMENTATION_ROOT = path.resolve(WEB_ROOT, "..", "..");
const DEFAULT_REFERENCE_ROOT = path.resolve(IMPLEMENTATION_ROOT, "..", "Crystal");
const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true });
const REPARSE_POINT = 0x400;
const NO_FOLLOW = fsConstants.O_NOFOLLOW ?? 0;
const STRONG_NO_FOLLOW = process.platform !== "win32" && NO_FOLLOW !== 0;
const REVISION_RE = /^[0-9a-f]{40}$/;
const CONTROL_CHARACTER_RE = /[\x00-\x1f\x7f-\x9f]/u;
const WINDOWS_DOS_DEVICE_BASENAME_RE = /^(?:CON|PRN|AUX|NUL|CLOCK\$|COM[1-9]|LPT[1-9])(?:\..*)?$/i;

if (isMainModule()) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}

export async function buildCrystalSemanticSourceInventory(options = {}) {
  const referenceRoot = path.resolve(options.referenceRoot ?? DEFAULT_REFERENCE_ROOT);
  const controlledRoots = validateControlledRoots(
    options.controlledRoots ?? DEFAULT_CONTROLLED_ROOTS,
  );
  await requireSafeDirectory(referenceRoot, "reference root");

  const gitBefore = await readCrystalGit(referenceRoot);
  const sourceFiles = [];
  let strongNoFollow = STRONG_NO_FOLLOW;

  for (const controlledRoot of controlledRoots) {
    const controlledRootPath = resolveContained(referenceRoot, controlledRoot, `controlled root ${controlledRoot}`);
    await requireSafeDirectory(controlledRootPath, `controlled root ${controlledRoot}`);
    const scan = await enumerateSourceRoot(referenceRoot, controlledRoot);
    sourceFiles.push(...scan.files);
    strongNoFollow &&= scan.strongNoFollow;
  }

  sourceFiles.sort(compareSourceFiles);
  const sourcePaths = sourceFiles.map((file) => file.path);
  if (new Set(sourcePaths).size !== sourcePaths.length) {
    throw new Error("Inventory source paths must be unique");
  }
  const caseFoldedSourcePaths = sourcePaths.map((sourcePath) => sourcePath.toLowerCase());
  if (new Set(caseFoldedSourcePaths).size !== caseFoldedSourcePaths.length) {
    throw new Error("Inventory source paths must be unique under Windows case-folding");
  }

  if (options.testHooks?.beforeFinalGitStatus !== undefined) {
    if (typeof options.testHooks.beforeFinalGitStatus !== "function") {
      throw new Error("testHooks.beforeFinalGitStatus must be a function");
    }
    await options.testHooks.beforeFinalGitStatus();
  }

  const gitAfter = await readCrystalGit(referenceRoot);
  if (gitBefore.revision !== gitAfter.revision || gitBefore.statusSha256 !== gitAfter.statusSha256) {
    throw new Error("Crystal HEAD or scoped worktree changed while inventory was being generated");
  }

  const sourceRootClean = gitBefore.clean && gitAfter.clean;
  const {
    sourceFileInventoryComplete,
    semanticLeafInventoryComplete,
    inventoryComplete,
  } = deriveInventoryCompletion(sourceRootClean, strongNoFollow);
  return {
    schemaVersion: INVENTORY_SCHEMA_VERSION,
    generator: "generate-crystal-semantic-source-inventory.mjs",
    referenceRootRelative: REFERENCE_ROOT_RELATIVE,
    controlledRoots,
    crystalRevision: gitBefore.revision,
    sourceRootClean,
    sourceFileInventoryComplete,
    semanticLeafInventoryComplete,
    inventoryComplete,
    aggregateSha256: computeInventoryAggregate(controlledRoots, sourceFiles),
    counts: {
      controlledRoots: controlledRoots.length,
      sourceFiles: sourceFiles.length,
    },
    sourceFiles,
  };
}

export function deriveInventoryCompletion(sourceRootClean, strongNoFollow) {
  if (typeof sourceRootClean !== "boolean" || typeof strongNoFollow !== "boolean") {
    throw new Error("Inventory completion inputs must be booleans");
  }
  const sourceFileInventoryComplete = sourceRootClean && strongNoFollow;
  const semanticLeafInventoryComplete = false;
  return {
    sourceFileInventoryComplete,
    semanticLeafInventoryComplete,
    inventoryComplete: sourceFileInventoryComplete && semanticLeafInventoryComplete,
  };
}

async function enumerateSourceRoot(referenceRoot, controlledRoot) {
  const files = [];
  let strongNoFollow = STRONG_NO_FOLLOW;
  const pending = [controlledRoot];

  while (pending.length > 0) {
    const relativeDirectory = pending.pop();
    const absoluteDirectory = resolveContained(referenceRoot, relativeDirectory, relativeDirectory);
    await requireSafeDirectory(absoluteDirectory, relativeDirectory);
    const entries = (await readdir(absoluteDirectory, { withFileTypes: true })).sort((left, right) =>
      compareText(left.name, right.name),
    );

    for (const entry of entries) {
      const relativePath = `${relativeDirectory}/${entry.name}`;
      validateSafeRelative(relativePath, relativePath);
      const absolutePath = resolveContained(referenceRoot, relativePath, relativePath);
      const info = await lstat(absolutePath);
      if (isSymlinkOrReparse(info)) {
        throw new Error(`Inventory source contains symlink/reparse: ${relativePath}`);
      }
      await requireResolvedIdentity(absolutePath, relativePath);

      if (info.isDirectory()) {
        pending.push(relativePath);
        continue;
      }
      if (!info.isFile() || !entry.name.toLowerCase().endsWith(".cs")) continue;

      const read = await readStableSourceFile(absolutePath, relativePath);
      strongNoFollow &&= read.strongNoFollow;
      files.push({
        path: relativePath,
        sha256: hashBuffer(read.bytes),
        encoding: "utf-8",
        bytes: read.bytes.length,
        lineCount: countLines(read.text),
        controlledRoot,
      });
    }
  }

  return { files, strongNoFollow };
}

async function readStableSourceFile(absolutePath, relativePath) {
  const before = await lstat(absolutePath, { bigint: true });
  if (!before.isFile() || before.isSymbolicLink()) {
    throw new Error(`Inventory source is not a regular file: ${relativePath}`);
  }

  let handle;
  try {
    handle = await open(absolutePath, fsConstants.O_RDONLY | NO_FOLLOW);
  } catch (error) {
    throw new Error(`Inventory source open failed: ${relativePath}: ${error.message}`);
  }

  let bytes;
  try {
    const opened = await handle.stat({ bigint: true });
    if (!opened.isFile() || !stableStat(before, opened)) {
      throw new Error(`Inventory source changed before open: ${relativePath}`);
    }
    bytes = await handle.readFile();
    const after = await handle.stat({ bigint: true });
    if (!stableStat(opened, after) || BigInt(bytes.length) !== after.size) {
      throw new Error(`Inventory source changed while reading: ${relativePath}`);
    }
  } finally {
    await handle.close();
  }

  let text;
  try {
    text = UTF8_DECODER.decode(bytes);
  } catch (error) {
    throw new Error(`Inventory source is not UTF-8: ${relativePath}: ${error.message}`);
  }
  return { bytes, text, strongNoFollow: STRONG_NO_FOLLOW };
}

function stableStat(left, right) {
  return left.dev === right.dev
    && left.ino === right.ino
    && left.size === right.size
    && left.mtimeNs === right.mtimeNs
    && left.ctimeNs === right.ctimeNs;
}

async function readCrystalGit(referenceRoot) {
  try {
    const { stdout: repositoryOutput } = await execFile(
      "git",
      ["rev-parse", "--show-toplevel"],
      { cwd: referenceRoot, encoding: "utf8" },
    );
    const repositoryRoot = await realpath(path.resolve(repositoryOutput.trim()));
    const canonicalReferenceRoot = await realpath(path.resolve(referenceRoot));
    if (!isContainedPath(repositoryRoot, canonicalReferenceRoot)) {
      throw new Error("Crystal reference root is outside its Git repository");
    }
    const pathspec = scopedGitPathspec(repositoryRoot, canonicalReferenceRoot);
    const { stdout: revisionOutput } = await execFile(
      "git",
      ["rev-parse", "--verify", "HEAD"],
      { cwd: repositoryRoot, encoding: "utf8" },
    );
    const revision = revisionOutput.trim();
    if (!REVISION_RE.test(revision)) throw new Error("Crystal HEAD is not a full lowercase Git revision");
    const statusArguments = ["status", "--porcelain=v1", "--untracked-files=all"];
    if (pathspec !== null) statusArguments.push("--", pathspec);
    const { stdout: statusOutput } = await execFile(
      "git",
      statusArguments,
      { cwd: repositoryRoot, encoding: "utf8" },
    );
    return {
      revision,
      clean: statusOutput.length === 0,
      statusSha256: hashText(statusOutput),
    };
  } catch (error) {
    throw new Error(`Crystal reference root must be a Git worktree with a valid HEAD: ${error.message}`);
  }
}

function scopedGitPathspec(repositoryRoot, referenceRoot) {
  const relative = path.relative(repositoryRoot, referenceRoot);
  if (relative === "") return null;
  if (relative === ".." || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error("Crystal reference root is outside its Git repository");
  }
  const normalized = relative.split(path.sep).join("/");
  const segments = normalized.split("/");
  if (segments.some((segment) => !segment || segment === "." || segment === ".." || segment.includes("\0"))) {
    throw new Error("Crystal reference root produced an unsafe Git pathspec");
  }
  return `:(top,literal)${normalized}`;
}

export function computeInventoryAggregate(controlledRoots, sourceFiles) {
  const canonical = [
    ...controlledRoots.map((root) => `root\t${root}\n`),
    ...sourceFiles.map((file) =>
      `file\t${file.path}\t${file.bytes}\t${file.lineCount}\t${file.sha256}\n`),
  ].join("");
  return hashBuffer(Buffer.from(canonical, "utf8"));
}

function validateControlledRoots(roots) {
  if (!Array.isArray(roots) || roots.length === 0) {
    throw new Error("controlledRoots must be a non-empty array");
  }
  const normalized = roots.map((root) => {
    validateSafeRelative(root, "controlled root");
    return root;
  });

  for (let leftIndex = 0; leftIndex < normalized.length; leftIndex += 1) {
    const left = normalized[leftIndex].toLowerCase();
    for (let rightIndex = leftIndex + 1; rightIndex < normalized.length; rightIndex += 1) {
      const right = normalized[rightIndex].toLowerCase();
      if (left === right) {
        throw new Error(`Duplicate controlled roots under Windows case folding: ${normalized[leftIndex]} and ${normalized[rightIndex]}`);
      }
      if (left.startsWith(`${right}/`) || right.startsWith(`${left}/`)) {
        throw new Error(`Overlapping controlled roots under Windows case folding: ${normalized[leftIndex]} and ${normalized[rightIndex]}`);
      }
    }
  }

  normalized.sort(compareText);
  if (JSON.stringify(normalized) !== JSON.stringify(DEFAULT_CONTROLLED_ROOTS)) {
    throw new Error("controlledRoots must be exactly Client/Server/Shared");
  }
  return normalized;
}

function validateSafeRelative(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${label} must be a non-empty relative path`);
  }
  if (CONTROL_CHARACTER_RE.test(value)) {
    throw new Error(label + " contains a control character");
  }
  if (value.includes("\0") || value.includes("\\")) {
    throw new Error(`${label} contains unsafe characters`);
  }
  if (path.posix.isAbsolute(value) || path.win32.isAbsolute(value) || /^[A-Za-z]:/.test(value)) {
    throw new Error(`${label} must be relative`);
  }
  const segments = value.split("/");
  if (segments.some((segment) => !segment || segment === "." || segment === "..")) {
    throw new Error(`${label} contains unsafe path segments`);
  }
  if (segments.some((segment) => /[. ]$/.test(segment))) {
    throw new Error(label + " contains a segment ending in a dot or space");
  }
  if (segments.some((segment) => WINDOWS_DOS_DEVICE_BASENAME_RE.test(segment))) {
    throw new Error(label + " contains a Windows DOS device basename");
  }
  if (segments.some((segment) => /[<>:"|?*]/.test(segment))) {
    throw new Error(`${label} contains unsafe Windows path characters`);
  }
}

function resolveContained(root, relativePath, label) {
  validateSafeRelative(relativePath, label);
  const candidate = path.resolve(root, ...relativePath.split("/"));
  if (!isContainedPath(root, candidate)) throw new Error(`${label} escapes reference root`);
  return candidate;
}

function isContainedPath(root, candidate) {
  const relative = path.relative(path.resolve(root), path.resolve(candidate));
  return relative === ""
    || (relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
}

async function requireSafeDirectory(directory, label) {
  const absolute = path.resolve(directory);
  await requireNoReparsePath(absolute, label);
  const info = await lstat(absolute);
  if (!info.isDirectory() || isSymlinkOrReparse(info)) {
    throw new Error(`${label} must be a normal directory`);
  }
  return absolute;
}

async function requireNoReparsePath(absolutePath, label) {
  const absolute = path.resolve(absolutePath);
  const parsed = path.parse(absolute);
  const relative = path.relative(parsed.root, absolute);
  let current = parsed.root;
  for (const segment of relative ? relative.split(path.sep) : []) {
    current = path.join(current, segment);
    const info = await lstat(current).catch((error) => {
      if (error?.code === "ENOENT") return null;
      throw error;
    });
    if (!info) throw new Error(`${label} does not exist`);
    if (isSymlinkOrReparse(info)) throw new Error(`${label} contains a symlink/reparse component`);
    await requireResolvedIdentity(current, label);
  }
  return absolute;
}

async function requireResolvedIdentity(candidate, label) {
  const resolved = await realpath(candidate);
  if (comparablePath(resolved) !== comparablePath(candidate)) {
    throw new Error(`${label} resolves through a junction/reparse component`);
  }
}

function comparablePath(value) {
  let normalized = path.normalize(path.resolve(value));
  if (process.platform === "win32" && normalized.startsWith("\\\\?\\")) normalized = normalized.slice(4);
  return process.platform === "win32" ? normalized.toLowerCase() : normalized;
}

function isSymlinkOrReparse(info) {
  return info.isSymbolicLink()
    || (typeof info.attributes === "number" && (info.attributes & REPARSE_POINT) !== 0);
}

function countLines(source) {
  return source.length === 0 ? 0 : source.split(/\r\n|\n|\r/).length;
}

function hashBuffer(value) {
  return createHash("sha256").update(value).digest("hex");
}

function hashText(value) {
  return createHash("sha256").update(value, "utf8").digest("hex");
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

function compareSourceFiles(left, right) {
  return compareText(left.path, right.path);
}

function isMainModule() {
  return process.argv[1]
    && pathToFileURL(path.resolve(process.argv[1])).href === pathToFileURL(SCRIPT_PATH).href;
}

async function main() {
  const { referenceRoot, controlledRoots, outputPath, quiet } = parseCli(process.argv.slice(2));
  const inventory = await buildCrystalSemanticSourceInventory({ referenceRoot, controlledRoots });
  const serialized = `${JSON.stringify(inventory, null, 2)}\n`;
  if (outputPath) {
    const absoluteOutputPath = await writeExclusiveOutput(outputPath, serialized);
    if (quiet) {
      process.stderr.write(formatQuietSummary(absoluteOutputPath, inventory, serialized));
      return;
    }
  }
  process.stdout.write(serialized);
}

async function writeExclusiveOutput(outputPath, contents) {
  const absoluteOutputPath = await validateOutputPath(outputPath);
  const handle = await open(absoluteOutputPath, "wx", 0o644);
  try {
    await handle.writeFile(contents, "utf8");
    await handle.sync();
  } finally {
    await handle.close();
  }
  return absoluteOutputPath;
}

async function validateOutputPath(outputPath) {
  const absoluteOutputPath = path.resolve(outputPath);
  const parent = path.dirname(absoluteOutputPath);
  await requireSafeDirectory(parent, "output parent");
  const existing = await lstat(absoluteOutputPath).catch((error) => {
    if (error?.code === "ENOENT") return null;
    throw error;
  });
  if (existing) throw new Error(`Output file already exists: ${absoluteOutputPath}`);
  return absoluteOutputPath;
}

function formatQuietSummary(outputPath, inventory, serialized) {
  return `path=${outputPath} sha256=${hashText(serialized)} aggregate=${inventory.aggregateSha256} counts=controlledRoots:${inventory.counts.controlledRoots},sourceFiles:${inventory.counts.sourceFiles}\n`;
}

function parseCli(args) {
  let referenceRoot = DEFAULT_REFERENCE_ROOT;
  const controlledRoots = [];
  let outputPath = null;
  let quiet = false;

  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--help" || argument === "-h") {
      process.stdout.write(
        "Usage: node generate-crystal-semantic-source-inventory.mjs [--root PATH] [--source-root Client --source-root Server --source-root Shared] [--output PATH [--quiet]]\n",
      );
      process.exit(0);
    }
    if (argument === "--root") {
      referenceRoot = args[++index];
      if (!referenceRoot) throw new Error("--root requires a path");
      continue;
    }
    if (argument === "--source-root") {
      controlledRoots.push(args[++index]);
      if (!controlledRoots.at(-1)) throw new Error("--source-root requires a relative path");
      continue;
    }
    if (argument === "--output") {
      outputPath = args[++index];
      if (!outputPath) throw new Error("--output requires a path");
      continue;
    }
    if (argument === "--quiet") {
      quiet = true;
      continue;
    }
    throw new Error(`Unknown argument: ${argument}`);
  }

  if (quiet && !outputPath) throw new Error("--quiet requires --output PATH");
  return {
    referenceRoot,
    controlledRoots: controlledRoots.length > 0 ? controlledRoots : DEFAULT_CONTROLLED_ROOTS,
    outputPath,
    quiet,
  };
}