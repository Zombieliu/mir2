import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";

const check = process.argv.slice(2).join(" ") === "--check";
assert(check || process.argv.length === 2, "Usage: generate-r16-standing-sprites.mjs [--check]");
const generated = new URL("../lib/generated/", import.meta.url);
const assets = new URL("../public/", import.meta.url);
const pins = JSON.parse(readFileSync(new URL("r16-entry-visuals.json", generated)));
const release = JSON.parse(readFileSync(new URL("../../../config/numeron-r16-web-assets.json", import.meta.url)));
assert.equal(pins.assetReleaseId, release.version);
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const pngPrefix = "data:image/png;base64,";
let total = 0;
for (const gender of ["male", "female"]) {
  const pin = pins.players[gender], bytes = readFileSync(new URL(pin.path.slice(1), assets));
  assert.equal(bytes.length, pin.bytes); assert.equal(sha(bytes), pin.sha256);
  const source = JSON.parse(bytes), base = gender === "male" ? 0 : 808;
  for (let direction = 0; direction < 8; direction++) {
    const images = {};
    // Crystal Frames.cs Player Standing: start 0, four frames, skip 0,
    // 500ms. Preserve every phase for this direction, not just its first pose.
    for (const library of ["CArmour/00", "CHair/00"]) {
      for (let phase = 0; phase < 4; phase++) {
        const index = base + direction * 4 + phase;
        const frame = source.libraries[library].frames.find(frame => frame.index === index);
        assert(frame && frame.path === "/original-ui/" + library + "/" + index + ".png");
        const png = source.images[frame.path]; assert(png.startsWith(pngPrefix));
        const raw = Buffer.from(png.slice(pngPrefix.length), "base64");
        assert.equal(png, pngPrefix + raw.toString("base64"));
        assert.equal(raw.subarray(0, 8).toString("hex"), "89504e470d0a1a0a");
        assert.equal(raw.toString("ascii", 12, 16), "IHDR");
        assert.equal(raw.readUInt32BE(16), frame.width); assert.equal(raw.readUInt32BE(20), frame.height);
        images[frame.path] = { png, width: frame.width, height: frame.height };
      }
    }
    const text = JSON.stringify({ schemaVersion: 1, assetReleaseId: pins.assetReleaseId,
      sourceSha256: pin.sha256, gender, direction, images }) + "\n";
    assert(Buffer.byteLength(text) <= 20_000, "A single direction chunk exceeds its decoded budget");
    const file = new URL("r16-standing/" + gender + "-" + direction + ".json", generated);
    if (check) assert.equal(readFileSync(file, "utf8"), text, "Stale standing chunk " + gender + direction);
    else { mkdirSync(new URL(".", file), { recursive: true }); writeFileSync(file, text); }
    total += Buffer.byteLength(text);
  }
}
console.log(JSON.stringify({ checked: check, chunks: 16, frames: 128, bytes: total, framesPerLibraryDirection: 4 }));
