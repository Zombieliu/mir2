#!/usr/bin/env node
import { promises as fs } from "node:fs";
import path from "node:path";

const DEFAULT_OUTPUT_DIR = ".vercel/output";
const DEFAULT_REPORT_PATH = "../../docs/generated/remote-assets/latest-vercel-output-prune.json";
const PRUNE_TARGETS = [
  {
    path: "static/original-ui",
    reason: "served from the versioned R2 asset origin through the player-domain proxy",
  },
  {
    path: "static/original-map",
    reason: "served from the versioned R2 asset origin through the player-domain proxy",
  },
  {
    path: "static/generated/original-map-blend",
    reason: "served from the versioned R2 asset origin through the player-domain proxy",
  },
];

const args = parseArgs(process.argv.slice(2));
const outputDir = path.resolve(process.cwd(), args.output ?? DEFAULT_OUTPUT_DIR);
const reportPath = args.report === false ? null : path.resolve(process.cwd(), args.report ?? DEFAULT_REPORT_PATH);
const dryRun = args.dryRun === true;

const outputStatsBefore = await collectStats(outputDir);
const targets = [];

for (const target of PRUNE_TARGETS) {
  const absolutePath = path.resolve(outputDir, target.path);
  assertInsideOutput(outputDir, absolutePath);
  const stats = await collectStats(absolutePath);
  targets.push({
    ...target,
    absolutePath,
    exists: stats.exists,
    bytes: stats.bytes,
    files: stats.files,
    directories: stats.directories,
  });
}

if (!dryRun) {
  for (const target of targets) {
    if (!target.exists) continue;
    await fs.rm(target.absolutePath, { force: true, recursive: true });
  }
}

const outputStatsAfter = await collectStats(outputDir);
const candidateRemovedBytes = targets.reduce((sum, target) => sum + target.bytes, 0);
const candidateRemovedFiles = targets.reduce((sum, target) => sum + target.files, 0);
const report = {
  ok: true,
  dryRun,
  outputDir,
  candidateRemovedBytes,
  candidateRemovedFiles,
  removedBytes: Math.max(0, outputStatsBefore.bytes - outputStatsAfter.bytes),
  removedFiles: Math.max(0, outputStatsBefore.files - outputStatsAfter.files),
  before: outputStatsBefore,
  after: outputStatsAfter,
  targets: targets.map(({ absolutePath, ...target }) => target),
};

if (reportPath) {
  await fs.mkdir(path.dirname(reportPath), { recursive: true });
  await fs.writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`, "utf8");
}

console.log(JSON.stringify(report, null, 2));

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--") continue;
    if (value === "--dryRun") {
      parsed.dryRun = parseBoolean(values[index + 1] ?? "true");
      if (values[index + 1] && !values[index + 1].startsWith("--")) index += 1;
      continue;
    }
    if (value === "--output") {
      parsed.output = requireValue(values, index, value);
      index += 1;
      continue;
    }
    if (value === "--report") {
      const next = requireValue(values, index, value);
      parsed.report = next === "false" ? false : next;
      index += 1;
      continue;
    }
    throw new Error(`Unknown argument: ${value}`);
  }
  return parsed;
}

function requireValue(values, index, flag) {
  const next = values[index + 1];
  if (!next || next.startsWith("--")) {
    throw new Error(`${flag} requires a value`);
  }
  return next;
}

function parseBoolean(value) {
  if (value === "true" || value === "1") return true;
  if (value === "false" || value === "0") return false;
  throw new Error(`Invalid boolean: ${value}`);
}

function assertInsideOutput(outputRoot, candidate) {
  const relative = path.relative(outputRoot, candidate);
  if (!relative || relative.startsWith("..") || path.isAbsolute(relative)) {
    throw new Error(`Refusing to prune outside output directory: ${candidate}`);
  }
}

async function collectStats(targetPath) {
  try {
    const stat = await fs.stat(targetPath);
    if (stat.isFile()) {
      return {
        exists: true,
        bytes: stat.size,
        files: 1,
        directories: 0,
      };
    }
    if (!stat.isDirectory()) {
      return {
        exists: true,
        bytes: 0,
        files: 0,
        directories: 0,
      };
    }
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return {
        exists: false,
        bytes: 0,
        files: 0,
        directories: 0,
      };
    }
    throw error;
  }

  let bytes = 0;
  let files = 0;
  let directories = 1;
  const entries = await fs.readdir(targetPath, { withFileTypes: true });

  for (const entry of entries) {
    const childPath = path.join(targetPath, entry.name);
    if (entry.isDirectory()) {
      const child = await collectStats(childPath);
      bytes += child.bytes;
      files += child.files;
      directories += child.directories;
      continue;
    }
    if (entry.isFile()) {
      const stat = await fs.stat(childPath);
      bytes += stat.size;
      files += 1;
    }
  }

  return {
    exists: true,
    bytes,
    files,
    directories,
  };
}
