import { createHash, randomUUID } from "node:crypto";
import { open, lstat, readFile, realpath, rename, unlink } from "node:fs/promises";
import path from "node:path";

const MAX_NFT_BYTES = 1024 * 1024;
const MAX_NFT_ROWS = 4096;
const TARGET_PARTS = ["server", "app", "api", "asset-manifest", "route.js.nft.json"];
const RELATIVE_TARGET = TARGET_PARTS.join("/");

type BuildMetadata = { projectDir: string; distDir: string };
type TraceRecord = { version: number; files: string[]; [key: string]: unknown };

export type FullPackTraceAudit = {
  target: string;
  beforeSha256: string;
  afterSha256: string;
  rowsBefore: number;
  rowsAfter: number;
  removedDirectoryRows: number;
  removedDescendantRows: number;
};

function sha256(bytes: Buffer): string {
  return createHash("sha256").update(bytes).digest("hex");
}

function within(parent: string, child: string): boolean {
  const relative = path.relative(parent, child);
  return relative !== "" && relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative);
}

function samePath(left: string, right: string): boolean {
  return path.relative(left, right) === "";
}

async function existingStat(candidate: string, label: string) {
  try {
    return await lstat(candidate);
  } catch {
    throw new Error(`Standalone full-pack trace: missing or unreadable ${label}`);
  }
}

async function requireOwnedDirectory(candidate: string, physicalProject: string, relative: string): Promise<void> {
  const stat = await existingStat(candidate, "owned output directory");
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error("Standalone full-pack trace: linked or non-directory output component");
  }
  let actual: string;
  try {
    actual = await realpath(candidate);
  } catch {
    throw new Error("Standalone full-pack trace: unreadable output component");
  }
  if (!samePath(actual, path.resolve(physicalProject, relative))) {
    throw new Error("Standalone full-pack trace: redirected output component");
  }
}

async function requireOwnedFile(candidate: string, physicalProject: string, relative: string) {
  const stat = await existingStat(candidate, "route NFT");
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error("Standalone full-pack trace: linked or non-regular route NFT");
  }
  let actual: string;
  try {
    actual = await realpath(candidate);
  } catch {
    throw new Error("Standalone full-pack trace: unreadable route NFT");
  }
  if (!samePath(actual, path.resolve(physicalProject, relative))) {
    throw new Error("Standalone full-pack trace: redirected route NFT");
  }
  return stat;
}

async function markerExists(candidate: string): Promise<boolean> {
  try {
    await lstat(candidate);
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw new Error("Standalone full-pack trace: unable to inspect bundler marker");
  }
}

function fullPackClass(row: string): "directory" | "descendant" | null {
  const normalized = path.posix.normalize(row.replace(/\\/g, "/"));
  const parts = normalized.split("/");
  for (let index = 0; index + 2 < parts.length; index += 1) {
    if (parts[index] === "generated" && parts[index + 1] === "crystal-packs" && parts[index + 2] === "full") {
      return index + 3 === parts.length ||
        (index + 4 === parts.length && parts.at(-1) === "") ? "directory" : "descendant";
    }
  }
  return null;
}

function parseTrace(bytes: Buffer): TraceRecord {
  let value: unknown;
  try {
    value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } catch {
    throw new Error("Standalone full-pack trace: malformed route NFT JSON");
  }
  if (value === null || Array.isArray(value) || typeof value !== "object") {
    throw new Error("Standalone full-pack trace: invalid route NFT object");
  }
  const record = value as Record<string, unknown>;
  if (record.version !== 1 || !Array.isArray(record.files) || record.files.length > MAX_NFT_ROWS ||
      record.files.some((row) => typeof row !== "string" || row.length === 0)) {
    throw new Error("Standalone full-pack trace: unsupported route NFT schema or row budget");
  }
  return record as TraceRecord;
}

/** Filter one owned route NFT before Next's serial standalone trace collection. */
export async function rewriteStandaloneFullPackTrace(
  metadata: BuildMetadata,
  expectedProjectDir: string,
): Promise<FullPackTraceAudit> {
  if (!metadata || typeof metadata.projectDir !== "string" || typeof metadata.distDir !== "string" ||
      typeof expectedProjectDir !== "string" ||
      !path.isAbsolute(metadata.projectDir) || !path.isAbsolute(metadata.distDir) ||
      !path.isAbsolute(expectedProjectDir)) {
    throw new Error("Standalone full-pack trace: build directories must be absolute");
  }
  const projectDir = path.resolve(metadata.projectDir);
  const distDir = path.resolve(metadata.distDir);
  let physicalProject: string;
  let physicalExpected: string;
  try {
    [physicalProject, physicalExpected] = await Promise.all([realpath(projectDir), realpath(expectedProjectDir)]);
  } catch {
    throw new Error("Standalone full-pack trace: unreadable project boundary");
  }
  if (!samePath(physicalProject, physicalExpected) || !within(projectDir, distDir)) {
    throw new Error("Standalone full-pack trace: output outside expected project");
  }

  const relativeDist = path.relative(projectDir, distDir);
  const outputParts = [...relativeDist.split(path.sep), ...TARGET_PARTS.slice(0, -1)];
  let candidate = projectDir;
  const walked: string[] = [];
  for (const part of outputParts) {
    walked.push(part);
    candidate = path.join(candidate, part);
    await requireOwnedDirectory(candidate, physicalProject, path.join(...walked));
  }
  if (await markerExists(path.join(distDir, "turbopack"))) {
    throw new Error("Standalone full-pack trace: Turbopack output is unsupported");
  }
  const target = path.join(candidate, TARGET_PARTS.at(-1)!);
  const relativeTarget = path.join(relativeDist, ...TARGET_PARTS);
  const stat = await requireOwnedFile(target, physicalProject, relativeTarget);
  if (stat.size > MAX_NFT_BYTES) {
    throw new Error("Standalone full-pack trace: route NFT byte budget exceeded");
  }
  let before: Buffer;
  try {
    before = await readFile(target);
  } catch {
    throw new Error("Standalone full-pack trace: unreadable route NFT");
  }
  if (before.length > MAX_NFT_BYTES) {
    throw new Error("Standalone full-pack trace: route NFT byte budget exceeded");
  }
  const trace = parseTrace(before);
  const retained: string[] = [];
  let removedDirectoryRows = 0;
  let removedDescendantRows = 0;
  for (const row of trace.files) {
    const kind = fullPackClass(row);
    if (kind === "directory") removedDirectoryRows += 1;
    else if (kind === "descendant") removedDescendantRows += 1;
    else retained.push(row);
  }
  const beforeSha256 = sha256(before);
  const audit: FullPackTraceAudit = {
    target: RELATIVE_TARGET,
    beforeSha256,
    afterSha256: beforeSha256,
    rowsBefore: trace.files.length,
    rowsAfter: retained.length,
    removedDirectoryRows,
    removedDescendantRows,
  };
  if (retained.length === trace.files.length) return audit;

  const nextBytes = Buffer.from(JSON.stringify({ ...trace, files: retained }));
  const temp = path.join(candidate, `.route.js.nft.json.full-pack-${process.pid}-${randomUUID()}.tmp`);
  if (!within(candidate, temp) || path.dirname(temp) !== candidate) {
    throw new Error("Standalone full-pack trace: invalid owned temporary path");
  }
  let owned = false;
  let ownedIdentity: { dev: number; ino: number } | null = null;
  try {
    const handle = await open(temp, "wx", 0o600);
    owned = true;
    try {
      const temporaryStat = await handle.stat();
      ownedIdentity = { dev: temporaryStat.dev, ino: temporaryStat.ino };
      await handle.writeFile(nextBytes);
      await handle.sync();
    } finally {
      await handle.close();
    }
    // A concurrent build must not cause this transform to overwrite a newer NFT.
    for (let index = 0; index < outputParts.length; index += 1) {
      const relative = path.join(...outputParts.slice(0, index + 1));
      await requireOwnedDirectory(path.join(projectDir, relative), physicalProject, relative);
    }
    await requireOwnedFile(target, physicalProject, relativeTarget);
    let current: Buffer;
    try {
      current = await readFile(target);
    } catch {
      throw new Error("Standalone full-pack trace: route NFT changed before replacement");
    }
    if (sha256(current) !== beforeSha256) {
      throw new Error("Standalone full-pack trace: route NFT changed before replacement");
    }
    await rename(temp, target);
    owned = false;
  } catch (error) {
    if (owned) {
      const tempStat = await existingStat(temp, "owned temporary NFT");
      if (!tempStat.isFile() || tempStat.isSymbolicLink() ||
          !ownedIdentity || tempStat.dev !== ownedIdentity.dev || tempStat.ino !== ownedIdentity.ino ||
          !within(candidate, temp) || path.dirname(temp) !== candidate) {
        throw new Error("Standalone full-pack trace: owned temporary NFT cleanup refused");
      }
      try {
        await unlink(temp);
      } catch {
        throw new Error("Standalone full-pack trace: owned temporary NFT cleanup failed");
      }
    }
    if (error instanceof Error && error.message.startsWith("Standalone full-pack trace:")) throw error;
    throw new Error("Standalone full-pack trace: atomic replacement failed");
  }
  audit.afterSha256 = sha256(nextBytes);
  return audit;
}
