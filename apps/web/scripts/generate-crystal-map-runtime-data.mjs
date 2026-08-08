#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { gzipSync } from "node:zlib";

const SCRIPT_DIR = path.resolve(import.meta.dirname);
const WEB_ROOT = path.resolve(SCRIPT_DIR, "..");
const REPO_ROOT = path.resolve(WEB_ROOT, "..", "..");
const DEFAULT_CLIENT_ROOT = path.resolve(REPO_ROOT, "..", "downloads", "crystal-client-full");
const DEFAULT_MAP_OUTPUT_DIR = path.join(WEB_ROOT, "lib", "generated", "crystal-map-pack");
const DEFAULT_LIBRARY_OUTPUT_DIR = path.join(WEB_ROOT, "lib", "generated", "crystal-map-library-meta");

const args = parseArgs(process.argv.slice(2));
const clientRoot = path.resolve(args.clientRoot ?? process.env.CRYSTAL_CLIENT_ROOT ?? DEFAULT_CLIENT_ROOT);
const mapDir = path.join(clientRoot, "Map");
const dataMapDir = path.join(clientRoot, "Data", "Map");
const mapOutputDir = path.resolve(args.mapOutputDir ?? DEFAULT_MAP_OUTPUT_DIR);
const libraryOutputDir = path.resolve(args.libraryOutputDir ?? DEFAULT_LIBRARY_OUTPUT_DIR);
const includeMaps = args.librariesOnly !== true;
const includeLibraries = args.mapsOnly !== true;

const report = {
  generatedAt: new Date().toISOString(),
  clientRoot,
  mapDir,
  dataMapDir,
  mapOutputDir,
  libraryOutputDir,
  maps: { count: 0, bytes: 0 },
  libraries: { count: 0, frames: 0, bytes: 0 },
};

if (includeMaps) {
  await fs.rm(mapOutputDir, { recursive: true, force: true });
  await fs.mkdir(mapOutputDir, { recursive: true });
  for (const filePath of (await listFilesRecursive(mapDir)).filter((entry) => entry.toLowerCase().endsWith(".map"))) {
    const bytes = await fs.readFile(filePath);
    const stem = path.basename(filePath).replace(/\.map$/i, "").toLowerCase();
    const outputPath = path.join(mapOutputDir, `${encodeURIComponent(stem)}.map.gz`);
    const packed = gzipSync(bytes, { level: 9 });
    await fs.writeFile(outputPath, packed);
    report.maps.count += 1;
    report.maps.bytes += packed.length;
  }
}

if (includeLibraries) {
  await fs.rm(libraryOutputDir, { recursive: true, force: true });
  await fs.mkdir(libraryOutputDir, { recursive: true });
  const seen = new Set();
  for (const filePath of (await listFilesRecursive(dataMapDir)).filter((entry) => /\.lib$/i.test(entry))) {
    const relative = path.relative(dataMapDir, filePath).split(path.sep).join("/");
    const libraryKey = relative.replace(/\.lib$/i, "");
    const seenKey = libraryKey.toLowerCase();
    if (seen.has(seenKey)) {
      continue;
    }
    seen.add(seenKey);

    const meta = await parseLibraryMeta(filePath);
    const outputPath = path.join(libraryOutputDir, ...libraryKey.split("/")) + ".json.gz";
    const packed = gzipSync(Buffer.from(JSON.stringify(meta)), { level: 9 });
    await fs.mkdir(path.dirname(outputPath), { recursive: true });
    await fs.writeFile(outputPath, packed);
    report.libraries.count += 1;
    report.libraries.frames += meta.count;
    report.libraries.bytes += packed.length;
  }
}

console.log(JSON.stringify(report, null, 2));

async function parseLibraryMeta(filePath) {
  const buffer = await fs.readFile(filePath);
  let offset = 0;
  const version = buffer.readInt32LE(offset);
  offset += 4;
  if (version < 2) {
    throw new Error(`Unsupported map library version ${version}: ${filePath}`);
  }
  const count = buffer.readInt32LE(offset);
  offset += 4;
  if (version >= 3) offset += 4;

  const frameOffsets = [];
  for (let index = 0; index < count; index += 1) {
    frameOffsets.push(buffer.readInt32LE(offset));
    offset += 4;
  }

  const frames = new Array(count).fill(null);
  for (let index = 0; index < frameOffsets.length; index += 1) {
    const frameOffset = frameOffsets[index];
    if (frameOffset <= 0 || frameOffset + 8 > buffer.length) continue;
    const width = buffer.readInt16LE(frameOffset);
    const height = buffer.readInt16LE(frameOffset + 2);
    if (width <= 0 || height <= 0) continue;
    frames[index] = [
      width,
      height,
      buffer.readInt16LE(frameOffset + 4),
      buffer.readInt16LE(frameOffset + 6),
    ];
  }

  return { version, count, frames };
}

async function listFilesRecursive(root) {
  const entries = await fs.readdir(root, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const childPath = path.join(root, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await listFilesRecursive(childPath)));
      continue;
    }
    if (entry.isFile()) files.push(childPath);
  }
  return files;
}

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--clientRoot") {
      parsed.clientRoot = requireValue(values, index, value);
      index += 1;
      continue;
    }
    if (value === "--mapOutputDir") {
      parsed.mapOutputDir = requireValue(values, index, value);
      index += 1;
      continue;
    }
    if (value === "--libraryOutputDir") {
      parsed.libraryOutputDir = requireValue(values, index, value);
      index += 1;
      continue;
    }
    if (value === "--mapsOnly") {
      parsed.mapsOnly = true;
      continue;
    }
    if (value === "--librariesOnly") {
      parsed.librariesOnly = true;
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
