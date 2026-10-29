import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import sharp from "sharp";
import { packIntoPages, renderMapAtlasPage } from "./build-map-atlas-pack.mjs";

test("packed frames own separate gutters across columns, rows and pages", () => {
  const sources = Array.from({ length: 100 }, (_, index) => ({
    filePath: `fixture-${index}.png`, frame: String(index), width: 96, height: 64,
  }));
  const pages = packIntoPages(sources);
  assert.ok(pages.length > 1);
  for (const page of pages) {
    const occupied = new Set();
    for (const frame of page.sources) {
      for (let y = frame.y - 1; y <= frame.y + frame.height; y += 1) {
        for (let x = frame.x - 1; x <= frame.x + frame.width; x += 1) {
          assert.ok(x >= 0 && y >= 0 && x < page.width && y < page.height);
          const pixel = y * page.width + x;
          assert.ok(!occupied.has(pixel), "neighbours must not share an extrusion pixel");
          occupied.add(pixel);
        }
      }
    }
  }
});

test("atlas preserves every source RGBA pixel and copies edge/corner alpha exactly", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "mir2-atlas-gutters-"));
  try {
    const originals = [
      Buffer.from([240, 30, 20, 255, 20, 170, 10, 128, 10, 30, 180, 0, 200, 180, 80, 255]),
      Buffer.from([10, 130, 20, 255, 120, 15, 210, 255, 25, 160, 50, 255, 170, 30, 120, 64]),
    ];
    const sources = [];
    for (const [index, pixels] of originals.entries()) {
      const filePath = path.join(root, `${index}.png`);
      await sharp(pixels, { raw: { width: 2, height: 2, channels: 4 } }).png().toFile(filePath);
      sources.push({ filePath, frame: String(index), width: 2, height: 2 });
    }
    const [page] = packIntoPages(sources);
    const atlas = await sharp(await renderMapAtlasPage(page)).ensureAlpha().raw().toBuffer();
    for (const frame of page.sources) {
      const original = originals[Number(frame.frame)];
      for (let y = -1; y <= frame.height; y += 1) {
        for (let x = -1; x <= frame.width; x += 1) {
          const src = (Math.max(0, Math.min(1, y)) * 2 + Math.max(0, Math.min(1, x))) * 4;
          const dst = ((frame.y + y) * page.width + frame.x + x) * 4;
          assert.deepEqual(atlas.subarray(dst, dst + 4), original.subarray(src, src + 4));
        }
      }
    }
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
