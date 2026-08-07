#!/usr/bin/env node

import crypto from "node:crypto";
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = path.resolve(scriptDir, "..");
const defaultSources = [
  "docs/generated/player-qa",
  "docs/generated/packet-traces",
  "docs/stage2-screenshots",
  "docs/stage5-screenshots",
];

export async function summarizeEvidence(options = {}) {
  const repoRoot = path.resolve(options.repoRoot ?? defaultRepoRoot);
  const sources = options.sources ?? defaultSources;
  const outputPath = path.resolve(repoRoot, options.output ?? "artifacts/qa-evidence/summary.json");
  const files = [];
  const sourceSummaries = [];

  for (const source of sources) {
    const sourcePath = resolveInside(repoRoot, source, "evidence source");
    const sourceFiles = await collectFiles(repoRoot, sourcePath);
    files.push(...sourceFiles);
    sourceSummaries.push({
      path: relative(repoRoot, sourcePath),
      fileCount: sourceFiles.length,
      bytes: sourceFiles.reduce((sum, file) => sum + file.size, 0),
    });
  }

  files.sort((left, right) => left.path.localeCompare(right.path));
  const digest = crypto.createHash("sha256");
  for (const file of files) digest.update(`${file.path}\0${file.size}\0${file.sha256}\n`);
  const summary = {
    schemaVersion: 1,
    kind: "mir2-qa-evidence-summary",
    generatedAt: new Date().toISOString(),
    fileCount: files.length,
    bytes: files.reduce((sum, file) => sum + file.size, 0),
    sha256: digest.digest("hex"),
    sources: sourceSummaries,
    files,
  };

  await fs.mkdir(path.dirname(outputPath), { recursive: true });
  await fs.writeFile(outputPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");
  return { summary, outputPath };
}

async function collectFiles(repoRoot, directory) {
  let entries;
  try {
    entries = await fs.readdir(directory, { withFileTypes: true });
  } catch (error) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }

  const files = [];
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name))) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isSymbolicLink()) throw new Error(`Refusing symlinked QA evidence path: ${entryPath}`);
    if (entry.isDirectory()) {
      files.push(...await collectFiles(repoRoot, entryPath));
      continue;
    }
    if (!entry.isFile()) continue;
    const bytes = await fs.readFile(entryPath);
    files.push({
      path: relative(repoRoot, entryPath),
      size: bytes.byteLength,
      sha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    });
  }
  return files;
}

function resolveInside(repoRoot, value, label) {
  const resolved = path.resolve(repoRoot, value);
  const relativePath = path.relative(repoRoot, resolved);
  if (relativePath.startsWith("..") || path.isAbsolute(relativePath)) {
    throw new Error(`Refusing ${label} outside the repository: ${value}`);
  }
  return resolved;
}

function relative(repoRoot, value) {
  return path.relative(repoRoot, value).split(path.sep).join("/");
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (!value.startsWith("--")) continue;
    const key = value.slice(2);
    const next = values[index + 1];
    parsed[key] = next && !next.startsWith("--") ? values[++index] : true;
  }
  return parsed;
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  const args = parseArgs(process.argv.slice(2));
  try {
    const sources = typeof args.sources === "string" ? args.sources.split(",").filter(Boolean) : undefined;
    const { summary, outputPath } = await summarizeEvidence({ output: args.output, sources });
    console.log(JSON.stringify({
      ok: true,
      outputPath,
      fileCount: summary.fileCount,
      bytes: summary.bytes,
      sha256: summary.sha256,
    }, null, 2));
  } catch (error) {
    console.error(`[qa-evidence] ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
