// Read-only source comparison for the native Guild storage/amount-box slice.
// Compare decoded pixels, not PNG compression bytes, and never rewrite assets.
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import sharp from "sharp";

import { decodeFrameRgba, parseLibrary } from "./crystal-library.mjs";

const dataDir = process.argv[2];
if (!dataDir || process.argv.length !== 3) {
  throw new Error("Usage: node apps/web/scripts/verify-guild-storage-assets.mjs <Crystal-client-Data-directory>");
}

const publicDir = path.resolve(import.meta.dirname, "../public/original-ui");
const groups = {
  Prguse: [1851, 917, 918, 238],
  Prguse2: [197, 198, 199, 207, 208, 209, 206, 360, 361, 362],
  Title: [93, 94, 99, 100, 101, 102, 105, 106, 200, 201, 202, 203, 204, 205],
  Items: [0, 116, 3660, 3661, 3662, 3673, 3674, 2960, 3675, 3670, 3671, 2961, 3672],
};
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
// MLibrary.VisiblePixel/GetTrueSize uses alpha != 0 and falls back to the full
// size when no pixel is visible. It returns size only; Draw is not cropped.
function sourceTrueSize(rgba, width, height) {
  let left = width, top = height, right = 0, bottom = 0;
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      if (rgba[(y * width + x) * 4 + 3] === 0) continue;
      left = Math.min(left, x);
      top = Math.min(top, y);
      right = Math.max(right, x + 1);
      bottom = Math.max(bottom, y + 1);
    }
  }
  return right === 0 ? { width, height } : { width: right - left, height: bottom - top };
}
const report = {
  schemaVersion: 1,
  generatedAt: new Date().toISOString(),
  method: "original Lib BGRA-to-RGBA versus independently decoded exported PNG; width/height/x/y metadata",
  frameCount: 0,
  mismatchCount: 0,
  libraries: [],
  itemCenteringAudit: null,
  visualAccepted: false,
  accepted: false,
  globalParityPercent: null,
};

for (const [name, indices] of Object.entries(groups)) {
  const bytes = readFileSync(path.join(dataDir, `${name}.Lib`));
  const library = parseLibrary(bytes);
  const meta = JSON.parse(readFileSync(path.join(publicDir, name, "meta.json"), "utf8"));
  const result = {
    name,
    sourceBytes: bytes.length,
    sourceSha256: sha256(bytes),
    frames: [],
  };
  for (const index of indices) {
    const frame = library.frames[index];
    if (!frame || frame.width <= 0 || frame.height <= 0) {
      throw new Error(`Missing drawable original frame ${name}/${index}`);
    }
    const png = readFileSync(path.join(publicDir, name, `${index}.png`));
    const sourceRgba = decodeFrameRgba(library, frame);
    const actual = await sharp(png).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    const exactRgba = actual.info.width === frame.width
      && actual.info.height === frame.height
      && actual.info.channels === 4
      && actual.data.equals(sourceRgba);
    const matches = meta.frames.filter((entry) => entry.index === index);
    const metadata = matches.length === 1
      && ["width", "height", "x", "y"].every((key) => matches[0][key] === frame[key]);
    report.frameCount += 1;
    if (!exactRgba || !metadata) report.mismatchCount += 1;
    result.frames.push({
      index,
      width: frame.width,
      height: frame.height,
      x: frame.x,
      y: frame.y,
      pngSha256: sha256(png),
      rgbaSha256: sha256(sourceRgba),
      exactRgba,
      metadata,
      sourceTrueSize: name === "Items" ? sourceTrueSize(sourceRgba, frame.width, frame.height) : null,
    });
  }
  if (name === "Items") {
    const audit = {
      source: "Client/MirGraphics/MLibrary.cs:959-1059",
      scope: "original alpha bounds for all exported Items frames; not whole-client implementation or screenshots",
      exportedFrameCount: meta.frames.length,
      differentSizeImageIndices: [],
      different35PxCenterImageIndices: [],
      sizeMismatchCount: 0,
      centeringMismatchCount: 0,
    };
    for (const entry of meta.frames) {
      const frame = library.frames[entry.index];
      if (!frame || frame.width <= 0 || frame.height <= 0) {
        throw new Error(`Missing audited source item frame ${entry.index}`);
      }
      const trueSize = sourceTrueSize(decodeFrameRgba(library, frame), frame.width, frame.height);
      if (trueSize.width !== frame.width || trueSize.height !== frame.height) {
        audit.differentSizeImageIndices.push(entry.index);
      }
      if (Math.trunc((35 - trueSize.width) / 2) !== Math.trunc((35 - frame.width) / 2)
        || Math.trunc((35 - trueSize.height) / 2) !== Math.trunc((35 - frame.height) / 2)) {
        audit.different35PxCenterImageIndices.push(entry.index);
      }
    }
    audit.sizeMismatchCount = audit.differentSizeImageIndices.length;
    audit.centeringMismatchCount = audit.different35PxCenterImageIndices.length;
    report.itemCenteringAudit = audit;
  }
  report.libraries.push(result);
}

console.log(JSON.stringify(report, null, 2));
if (report.frameCount !== 41 || report.mismatchCount !== 0) process.exitCode = 1;
