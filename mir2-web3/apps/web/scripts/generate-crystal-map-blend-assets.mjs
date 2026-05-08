import fs from "node:fs/promises";
import path from "node:path";
import sharp from "sharp";

const SOURCE_DIR = path.resolve("public", "original-map", "WemadeMir2", "Objects");
const OUTPUT_DIR = path.resolve("public", "generated", "original-map-blend", "WemadeMir2", "Objects");

await fs.mkdir(OUTPUT_DIR, { recursive: true });

for (let frame = 2723; frame <= 2732; frame += 1) {
  const sourcePath = path.join(SOURCE_DIR, `${frame}.png`);
  const outputPath = path.join(OUTPUT_DIR, `${frame}.png`);
  const { data, info } = await sharp(sourcePath).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  const cleaned = Buffer.from(data);

  for (let offset = 0; offset < cleaned.length; offset += 4) {
    const red = cleaned[offset] ?? 0;
    const green = cleaned[offset + 1] ?? 0;
    const blue = cleaned[offset + 2] ?? 0;
    const alpha = cleaned[offset + 3] ?? 0;
    const brightness = Math.max(red, green, blue);

    if (alpha === 0 || brightness < 56) {
      cleaned[offset + 3] = 0;
      continue;
    }

    if (brightness < 104) {
      cleaned[offset + 3] = Math.round(alpha * ((brightness - 56) / 48));
    }
  }

  await sharp(cleaned, {
    raw: {
      width: info.width,
      height: info.height,
      channels: 4,
    },
  })
    .png({ compressionLevel: 9, adaptiveFiltering: false })
    .toFile(outputPath);
}
