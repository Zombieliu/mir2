import { createWriteStream } from "node:fs";
import fs from "node:fs/promises";
import path from "node:path";
import { once } from "node:events";

import { inspectFullPackClosure } from "./full-pack-closure.mjs";

const args = parseArgs(process.argv.slice(2));
const fullPackRoot = path.resolve(requireArg(args, "root"));
const outputPath = path.resolve(requireArg(args, "output"));
const expectedContentHash = String(args.expectedContentHash ?? "");
const archivePrefix = "generated/crystal-packs/full";

const closure = await inspectFullPackClosure({
  fullPackRoot,
  expectedContentHash,
  verifyPageHashes: false,
  rejectOrphans: true,
});
await fs.mkdir(path.dirname(outputPath), { recursive: true });
await fs.rm(outputPath, { force: true });

const output = createWriteStream(outputPath, { flags: "wx" });
try {
  for (const file of closure.files) {
    const archivePath = `${archivePrefix}/${file.relativePath}`;
    await writeChunk(output, createUstarHeader(archivePath, file.size));
    const input = await fs.open(file.absolutePath, "r");
    try {
      const buffer = Buffer.allocUnsafe(8 * 1024 * 1024);
      let position = 0;
      while (position < file.size) {
        const readLength = Math.min(buffer.byteLength, file.size - position);
        const { bytesRead } = await input.read(buffer, 0, readLength, position);
        if (bytesRead <= 0) throw new Error(`Unexpected EOF while archiving ${file.absolutePath}`);
        await writeChunk(output, buffer.subarray(0, bytesRead));
        position += bytesRead;
      }
    } finally {
      await input.close();
    }
    const padding = (512 - (file.size % 512)) % 512;
    if (padding) await writeChunk(output, Buffer.alloc(padding));
  }
  await writeChunk(output, Buffer.alloc(1024));
  output.end();
  await once(output, "finish");
} catch (error) {
  output.destroy();
  await fs.rm(outputPath, { force: true });
  throw error;
}

const stats = await fs.stat(outputPath);
console.log(JSON.stringify({
  ok: true,
  outputPath,
  contentHash: closure.contentHash,
  libraryCount: closure.libraryCount,
  pageCount: closure.pageCount,
  fileCount: closure.fileCount,
  archiveBytes: stats.size,
}, null, 2));

function createUstarHeader(archivePath, size) {
  if (!Number.isSafeInteger(size) || size < 0) throw new Error(`Invalid tar size for ${archivePath}: ${size}`);
  const { name, prefix } = splitUstarPath(archivePath);
  const header = Buffer.alloc(512);
  writeText(header, 0, 100, name);
  writeOctal(header, 100, 8, 0o644);
  writeOctal(header, 108, 8, 0);
  writeOctal(header, 116, 8, 0);
  writeOctal(header, 124, 12, size);
  writeOctal(header, 136, 12, 0);
  header.fill(0x20, 148, 156);
  header[156] = "0".charCodeAt(0);
  writeText(header, 257, 6, "ustar\0");
  writeText(header, 263, 2, "00");
  writeText(header, 265, 32, "root");
  writeText(header, 297, 32, "root");
  writeOctal(header, 329, 8, 0);
  writeOctal(header, 337, 8, 0);
  writeText(header, 345, 155, prefix);
  const checksum = header.reduce((sum, value) => sum + value, 0);
  const checksumText = checksum.toString(8).padStart(6, "0");
  writeText(header, 148, 6, checksumText);
  header[154] = 0;
  header[155] = 0x20;
  return header;
}

function splitUstarPath(value) {
  if (Buffer.byteLength(value) <= 100) return { name: value, prefix: "" };
  for (let index = value.lastIndexOf("/"); index > 0; index = value.lastIndexOf("/", index - 1)) {
    const prefix = value.slice(0, index);
    const name = value.slice(index + 1);
    if (Buffer.byteLength(prefix) <= 155 && Buffer.byteLength(name) <= 100) return { name, prefix };
  }
  throw new Error(`Path cannot be represented in USTAR: ${value}`);
}

function writeText(buffer, offset, length, value) {
  const bytes = Buffer.from(value, "utf8");
  if (bytes.byteLength > length) throw new Error(`USTAR field is too long: ${value}`);
  bytes.copy(buffer, offset);
}

function writeOctal(buffer, offset, length, value) {
  const text = value.toString(8).padStart(length - 1, "0");
  if (text.length > length - 1) throw new Error(`USTAR numeric field overflow: ${value}`);
  writeText(buffer, offset, length - 1, text);
  buffer[offset + length - 1] = 0;
}

async function writeChunk(stream, chunk) {
  if (!stream.write(chunk)) await once(stream, "drain");
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
