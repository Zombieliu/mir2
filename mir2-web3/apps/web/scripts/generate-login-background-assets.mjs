#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import sharp from "sharp";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const webRoot = path.resolve(scriptDir, "..");
const publicRoot = path.join(webRoot, "public");
const inputPath = path.resolve(
  process.env.MIR2_LOGIN_BACKGROUND_SOURCE ??
    path.join(publicRoot, "original-ui", "ChrSel", "0.png"),
);
const outputRoot = path.resolve(
  process.env.MIR2_LOGIN_BACKGROUND_OUTPUT_ROOT ??
    path.join(publicRoot, "bootstrap", "login"),
);
const variants = [
  { width: 768, quality: 76, maxBytes: 240 * 1024 },
  { width: 1024, quality: 78, maxBytes: 380 * 1024 },
];

await main();

async function main() {
  const sourceExists = await exists(inputPath);
  const allowPrebuilt =
    process.env.MIR2_USE_PREBUILT_LOGIN_BACKGROUND === "1" ||
    process.env.MIR2_ORIGINAL_ASSET_MANIFEST_MODE === "remote-release";

  if (!sourceExists && !allowPrebuilt) {
    throw new Error(
      `Login background source is missing: ${inputPath}. ` +
        "Set MIR2_USE_PREBUILT_LOGIN_BACKGROUND=1 only when the committed bootstrap variants are present.",
    );
  }

  await fs.mkdir(outputRoot, { recursive: true });

  if (sourceExists) {
    for (const variant of variants) {
      const outputPath = path.join(outputRoot, `chrsel-0-${variant.width}.webp`);
      await sharp(inputPath)
        .resize({ width: variant.width, withoutEnlargement: true })
        .webp({ quality: variant.quality, effort: 6 })
        .toFile(outputPath);
    }
  }

  const generated = await inspectVariants();
  console.log(
    JSON.stringify(
      {
        ok: true,
        mode: sourceExists ? "generated" : "prebuilt",
        source: "/original-ui/ChrSel/0.png",
        generated,
      },
      null,
      2,
    ),
  );
}

async function inspectVariants() {
  const generated = [];
  for (const variant of variants) {
    const outputPath = path.join(outputRoot, `chrsel-0-${variant.width}.webp`);
    let stat;
    try {
      stat = await fs.stat(outputPath);
    } catch (error) {
      if (error?.code === "ENOENT") {
        throw new Error(`Prebuilt login background is missing: ${outputPath}`);
      }
      throw error;
    }
    const metadata = await sharp(outputPath).metadata();
    if (metadata.format !== "webp" || metadata.width !== variant.width) {
      throw new Error(
        `${path.relative(webRoot, outputPath)} must be a ${variant.width}px WebP; ` +
          `received ${metadata.width ?? "unknown"}px ${metadata.format ?? "unknown"}`,
      );
    }
    if (stat.size > variant.maxBytes) {
      throw new Error(
        `${path.relative(webRoot, outputPath)} is ${stat.size} bytes; budget is ${variant.maxBytes}`,
      );
    }
    generated.push({
      path: publicPathFor(outputPath),
      width: variant.width,
      bytes: stat.size,
    });
  }
  return generated;
}

function publicPathFor(filePath) {
  const relative = path.relative(publicRoot, filePath);
  if (!relative.startsWith("..") && !path.isAbsolute(relative)) {
    return `/${relative.split(path.sep).join("/")}`;
  }
  return filePath;
}

async function exists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}
