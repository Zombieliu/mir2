import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

import { decodeFrameRgba, encodePng, parseLibrary } from "./crystal-library.mjs";

const WEB_ROOT = path.resolve(import.meta.dirname, "..");
const MIR2_ROOT = path.resolve(WEB_ROOT, "..", "..", "..");
const DEFAULT_DATA_DIR = path.join(MIR2_ROOT, "Crystal", "Build", "Client", "Debug", "Data");
const DEFAULT_OUTPUT_DIR = path.join(WEB_ROOT, "public", "original-effects", "Weather");
const BASE_FRAMES = Object.freeze([0, 1, 43, 164, 359, 531, 587]);

const args = parseArgs(process.argv.slice(2));
const dataDir = path.resolve(args.dataDir ?? process.env.CRYSTAL_CLIENT_DATA_DIR ?? DEFAULT_DATA_DIR);
const outputDir = path.resolve(args.outputDir ?? DEFAULT_OUTPUT_DIR);
const libraryPath = path.join(dataDir, "Weather.Lib");

if (!existsSync(libraryPath)) {
  throw new Error(`Crystal Weather.Lib not found at ${libraryPath}`);
}

const library = parseLibrary(await readFile(libraryPath));
await mkdir(outputDir, { recursive: true });
const frames = {};
for (const frameIndex of BASE_FRAMES) {
  const frame = library.frames[frameIndex];
  if (!frame) throw new Error(`Crystal Weather.Lib frame ${frameIndex} is missing`);
  const rgba = decodeFrameRgba(library, frame);
  const png = encodePng(frame.width, frame.height, rgba, 9);
  const fileName = `${frameIndex}.png`;
  await writeFile(path.join(outputDir, fileName), png);
  frames[frameIndex] = {
    path: `/original-effects/Weather/${fileName}`,
    width: frame.width,
    height: frame.height,
    x: frame.x,
    y: frame.y,
    bytes: png.length,
  };
}

await writeFile(
  path.join(outputDir, "meta.json"),
  `${JSON.stringify({ source: "Weather.Lib", frames }, null, 2)}\n`,
);
console.log(`Exported ${BASE_FRAMES.length} Crystal weather base frames to ${outputDir}`);

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (!value.startsWith("--")) continue;
    const key = value.slice(2);
    parsed[key] = values[index + 1];
    index += 1;
  }
  return parsed;
}
