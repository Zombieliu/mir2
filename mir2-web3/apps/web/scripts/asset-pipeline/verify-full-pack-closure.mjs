import path from "node:path";

import { inspectFullPackClosure } from "./full-pack-closure.mjs";

const args = parseArgs(process.argv.slice(2));
const result = await inspectFullPackClosure({
  fullPackRoot: path.resolve(requireArg(args, "root")),
  expectedContentHash: String(args.expectedContentHash ?? ""),
  verifyPageHashes: booleanArg(args.verifyPages, true),
  pageHashConcurrency: positiveInteger(args.concurrency, 4),
  rejectOrphans: true,
});

console.log(JSON.stringify({
  ok: true,
  contentHash: result.contentHash,
  libraryCount: result.libraryCount,
  pageCount: result.pageCount,
  fileCount: result.fileCount,
  pageHashesVerified: result.pageHashesVerified,
}, null, 2));

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--") continue;
    if (!value.startsWith("--")) throw new Error(`Unexpected argument: ${value}`);
    const key = value.slice(2);
    const next = values[index + 1];
    if (!next || next.startsWith("--")) throw new Error(`${value} requires a value`);
    parsed[key] = next;
    index += 1;
  }
  return parsed;
}

function requireArg(values, key) {
  const value = String(values[key] ?? "").trim();
  if (!value) throw new Error(`--${key} is required`);
  return value;
}

function booleanArg(value, fallback) {
  if (value === undefined) return fallback;
  const normalized = String(value).toLowerCase();
  if (normalized === "true" || normalized === "1") return true;
  if (normalized === "false" || normalized === "0") return false;
  throw new Error(`Expected a boolean, received ${value}`);
}

function positiveInteger(value, fallback) {
  const number = Number(value ?? fallback);
  if (!Number.isSafeInteger(number) || number <= 0) throw new Error(`Expected a positive integer, received ${value}`);
  return number;
}
