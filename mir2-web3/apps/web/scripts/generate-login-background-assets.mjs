#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const inputPath = path.join(webRoot, "public", "original-ui", "ChrSel", "0.png");
const outputRoot = path.join(webRoot, "public", "bootstrap", "login");
const variants = [
  { width: 768, quality: 76, maxBytes: 240 * 1024 },
  { width: 1024, quality: 78, maxBytes: 380 * 1024 },
];

await fs.mkdir(outputRoot, { recursive: true });

const generated = [];
for (const variant of variants) {
  const outputPath = path.join(outputRoot, `chrsel-0-${variant.width}.webp`);
  await sharp(inputPath)
    .resize({ width: variant.width, withoutEnlargement: true })
    .webp({ quality: variant.quality, effort: 6 })
    .toFile(outputPath);
  const stat = await fs.stat(outputPath);
  if (stat.size > variant.maxBytes) {
    throw new Error(
      `${path.relative(webRoot, outputPath)} is ${stat.size} bytes; budget is ${variant.maxBytes}`,
    );
  }
  generated.push({
    path: `/${path.relative(path.join(webRoot, "public"), outputPath).split(path.sep).join("/")}`,
    width: variant.width,
    bytes: stat.size,
  });
}

console.log(JSON.stringify({ ok: true, source: "/original-ui/ChrSel/0.png", generated }, null, 2));
