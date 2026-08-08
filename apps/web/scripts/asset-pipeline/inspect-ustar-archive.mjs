import fs from "node:fs/promises";
import path from "node:path";

const args = parseArgs(process.argv.slice(2));
const archivePath = path.resolve(requireArg(args, "archive"));
const requiredPrefix = normalizeArchivePrefix(requireArg(args, "prefix"));
const file = await fs.open(archivePath, "r");
const seen = new Set();
let regularFileCount = 0;
let directoryCount = 0;
let offset = 0;
let zeroBlocks = 0;

try {
  const stats = await file.stat();
  if (!stats.isFile() || stats.size < 1024 || stats.size % 512 !== 0) {
    throw new Error(`Invalid USTAR archive size: ${archivePath}`);
  }
  while (offset < stats.size) {
    const header = Buffer.alloc(512);
    const { bytesRead } = await file.read(header, 0, 512, offset);
    if (bytesRead !== 512) throw new Error(`Unexpected EOF in USTAR header at byte ${offset}`);
    offset += 512;

    if (header.every((value) => value === 0)) {
      zeroBlocks += 1;
      if (zeroBlocks >= 2) break;
      continue;
    }
    if (zeroBlocks) throw new Error("USTAR archive contains data after a zero block");
    validateChecksum(header, offset - 512);

    const typeFlag = header[156];
    if (typeFlag !== 0 && typeFlag !== 0x30 && typeFlag !== 0x35) {
      throw new Error(`USTAR archive contains a link or special entry type ${String.fromCharCode(typeFlag)} at byte ${offset - 512}`);
    }
    const name = readText(header, 0, 100);
    const prefix = readText(header, 345, 155);
    const entryPath = prefix ? `${prefix}/${name}` : name;
    validateEntryPath(entryPath, requiredPrefix);
    if (seen.has(entryPath)) throw new Error(`USTAR archive contains a duplicate entry: ${entryPath}`);
    seen.add(entryPath);

    const size = readOctal(header, 124, 12, "size");
    if (typeFlag === 0x35) {
      if (size !== 0) throw new Error(`USTAR directory has a non-zero size: ${entryPath}`);
      directoryCount += 1;
    } else {
      if (entryPath === requiredPrefix) throw new Error(`USTAR root entry must be a directory: ${entryPath}`);
      regularFileCount += 1;
    }
    offset += Math.ceil(size / 512) * 512;
    if (offset > stats.size) throw new Error(`USTAR entry extends past archive end: ${entryPath}`);
  }
  if (zeroBlocks < 2) throw new Error("USTAR archive does not end with two zero blocks");
  while (offset < stats.size) {
    const trailing = Buffer.alloc(512);
    const { bytesRead } = await file.read(trailing, 0, 512, offset);
    if (bytesRead !== 512 || !trailing.every((value) => value === 0)) {
      throw new Error(`USTAR archive contains non-zero data after its terminator at byte ${offset}`);
    }
    offset += 512;
  }
  if (!seen.has(`${requiredPrefix}/index.json`)) throw new Error("USTAR archive is missing the full-pack index");

  console.log(JSON.stringify({ ok: true, archivePath, regularFileCount, directoryCount }, null, 2));
} finally {
  await file.close();
}

function validateChecksum(header, headerOffset) {
  const expected = readOctal(header, 148, 8, "checksum");
  const copy = Buffer.from(header);
  copy.fill(0x20, 148, 156);
  const actual = copy.reduce((sum, value) => sum + value, 0);
  if (actual !== expected) throw new Error(`USTAR checksum mismatch at byte ${headerOffset}`);
}

function validateEntryPath(entryPath, requiredPrefix) {
  if (
    !entryPath ||
    entryPath.startsWith("/") ||
    entryPath.includes("\\") ||
    entryPath.includes("\0") ||
    (entryPath !== requiredPrefix && !entryPath.startsWith(`${requiredPrefix}/`)) ||
    entryPath.split("/").some((segment) => !segment || segment === "." || segment === "..")
  ) {
    throw new Error(`Unsafe or unexpected USTAR entry path: ${entryPath}`);
  }
}

function readText(buffer, offset, length) {
  const end = buffer.indexOf(0, offset);
  const limit = end >= offset && end < offset + length ? end : offset + length;
  const value = buffer.subarray(offset, limit).toString("utf8");
  if (value.includes("�")) throw new Error("USTAR header contains invalid UTF-8");
  return value;
}

function readOctal(buffer, offset, length, label) {
  const value = readText(buffer, offset, length).trim();
  if (!/^[0-7]+$/.test(value)) throw new Error(`Invalid USTAR ${label}: ${value}`);
  const number = Number.parseInt(value, 8);
  if (!Number.isSafeInteger(number) || number < 0) throw new Error(`Invalid USTAR ${label}: ${value}`);
  return number;
}

function normalizeArchivePrefix(value) {
  const normalized = String(value).replace(/^\/+|\/+$/g, "");
  if (!normalized || normalized.includes("\\") || normalized.split("/").some((part) => !part || part === "." || part === "..")) {
    throw new Error(`Invalid required archive prefix: ${value}`);
  }
  return normalized;
}

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
