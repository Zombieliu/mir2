// Read-only original Items.Lib / exported PNG geometry fixture generator.
// stdout is evidence only: neither assets nor the checked-in fixture are written.
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import sharp from "sharp";
import { parseLibrary, decodeFrameRgba } from "./crystal-library.mjs";

const dataDir = process.argv[2];
if (!dataDir || process.argv.length !== 3) {
  throw new Error("Usage: node apps/web/scripts/verify-item-true-size.mjs <Crystal-client-Data-directory>");
}
const sourceBytes = readFileSync(path.join(dataDir, "Items.Lib"));
const sourceSha256 = createHash("sha256").update(sourceBytes).digest("hex");
const library = parseLibrary(sourceBytes);
const imageDir = path.resolve(import.meta.dirname, "../public/original-ui/Items");
const metadata = JSON.parse(readFileSync(path.join(imageDir, "meta.json"), "utf8"));
const entries = [...metadata.frames].sort((a, b) => a.index - b.index);
const seen = new Set();
const fingerprint = createHash("sha256");
const frames = [];
let differentSizeCount = 0;
let different35PixelOffsetCount = 0;

// Follow MImage.GetTrueSize's four edge scans independently of the Rust
// renderer's bounding-box pass, including its all-transparent fallback.
function sourceTrueSize(pixels, width, height) {
  let left = 0, top = 0, right = width, bottom = height;
  const visible = (x, y) => pixels[(y * width + x) * 4 + 3] !== 0;
  leftScan: for (let x = 0; x < right; x += 1) {
    for (let y = 0; y < bottom; y += 1) {
      if (visible(x, y)) { left = x; break leftScan; }
    }
  }
  topScan: for (let y = 0; y < bottom; y += 1) {
    for (let x = left; x < right; x += 1) {
      if (visible(x, y)) { top = y; break topScan; }
    }
  }
  rightScan: for (let x = right - 1; x >= left; x -= 1) {
    for (let y = 0; y < bottom; y += 1) {
      if (visible(x, y)) { right = x + 1; break rightScan; }
    }
  }
  bottomScan: for (let y = bottom - 1; y >= top; y -= 1) {
    for (let x = left; x < right; x += 1) {
      if (visible(x, y)) { bottom = y + 1; break bottomScan; }
    }
  }
  return [right - left, bottom - top];
}

for (const entry of entries) {
  if (!Number.isInteger(entry.index) || seen.has(entry.index)) {
    throw new Error("Invalid or duplicate exported Items index");
  }
  seen.add(entry.index);
  const frame = library.frames[entry.index];
  if (!frame || frame.width <= 0 || frame.height <= 0
      || ["width", "height", "x", "y"].some((key) => frame[key] !== entry[key])) {
    throw new Error(`Invalid source/export geometry for Items/${entry.index}`);
  }
  const sourceRgba = decodeFrameRgba(library, frame);
  const png = readFileSync(path.join(imageDir, `${entry.index}.png`));
  const decoded = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  if (decoded.info.width !== frame.width || decoded.info.height !== frame.height
      || decoded.info.channels !== 4 || !decoded.data.equals(sourceRgba)) {
    throw new Error(`Original RGBA mismatch for Items/${entry.index}`);
  }
  const [trueWidth, trueHeight] = sourceTrueSize(sourceRgba, frame.width, frame.height);
  const row = [entry.index, frame.width, frame.height, trueWidth, trueHeight];
  frames.push(row);
  if (trueWidth !== frame.width || trueHeight !== frame.height) differentSizeCount += 1;
  if (Math.trunc((35 - trueWidth) / 2) !== Math.trunc((35 - frame.width) / 2)
      || Math.trunc((35 - trueHeight) / 2) !== Math.trunc((35 - frame.height) / 2)) {
    different35PixelOffsetCount += 1;
  }
  fingerprint.update(JSON.stringify(row) + "\n");
  fingerprint.update(sourceRgba);
}
if (frames.length !== 1003) {
  throw new Error(`Export denominator changed: ${frames.length}; inspect and update the complete fixture`);
}
const report = {
  schemaVersion: 1,
  crystalRevision: "92b4ce4ab488b11e65f63d3ad22de2e1f25ec08d",
  source: "Client/MirGraphics/MLibrary.cs:959-1059; original Items.Lib nonzero-alpha edge scans",
  sourceLibrarySha256: sourceSha256,
  sourceLibraryBytes: sourceBytes.length,
  verifiedOriginalPngCount: frames.length,
  rgbaAndGeometryFingerprintSha256: fingerprint.digest("hex"),
  fingerprintMethod: "ascending image index; UTF-8 JSON row plus LF, then original RGBA bytes",
  differentSizeCount,
  different35PixelOffsetCount,
  columns: ["index", "fullWidth", "fullHeight", "trueWidth", "trueHeight"],
  visualAccepted: false,
  accepted: false,
  globalParityPercent: null,
};
// One frame per line keeps the source fixture reviewable and reproducible.
console.log(JSON.stringify(report, null, 2).slice(0, -2)
  + ',\n  "frames": [\n'
  + frames.map((row) => "    " + JSON.stringify(row)).join(",\n")
  + "\n  ]\n}");
