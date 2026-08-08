import fs from "node:fs/promises";
import path from "node:path";
import sharp from "sharp";

// Crystal draws every Blend=true effect/object ADDITIVELY (Effect.cs:130, MLibrary.cs:692).
// Additive => black adds nothing => the dark matte / dithered halo disappears and only the bright
// core glows. We can't run an additive material on the DOM fallback path, so we fake it with a
// darkness->alpha "cleaned" sprite + CSS mix-blend-mode:screen.
//
// This was historically hardcoded to WemadeMir2/Objects frames 2723..2732 (the ten Bichon torches).
// That index range is NOT the Crystal blend criterion (it is the DrawBlend `offSet` argument at
// GameScene.cs:10928); the real gate is the per-cell FrontAnimationFrame high bit / MiddleAnimationFrame
// low nibble (GameScene.cs:10884-10892). Since the DOM path only has the sprite (not the cell byte),
// we approximate Crystal's "this art is a glow" set DATA-DRIVENLY from the pixels themselves: a frame
// is a glow if it has a large dark matte AND a meaningful bright core, reusing the same cutoffs the
// per-pixel ramp already uses. This generalizes the ten torches to the full glow inventory while
// keeping those ten byte-identical (same algorithm, same cutoffs).

const MATTE_CUTOFF = 56; // brightness < this => fully transparent (dark matte / halo)
const CORE_CUTOFF = 104; // brightness >= this => fully opaque (bright core); in between => ramped

// Frame-selection thresholds (share of OPAQUE pixels). darkMatte is the discriminator: glow art is
// mostly dark surround; brightCore guards against picking up dark-but-not-glowing scenery.
const DARK_MATTE_MIN = 0.5;
const BRIGHT_CORE_MIN = 0.1;

const OBJECTS_LIBS = ["Objects", "Objects20", "Objects21", "Objects23", "Objects4", "Objects5", "Objects6", "Objects9"];

const MAP_ROOT = path.resolve("public", "original-map", "WemadeMir2");
const OUTPUT_ROOT = path.resolve("public", "generated", "original-map-blend", "WemadeMir2");
const MANIFEST_PATH = path.resolve("public", "generated", "original-map-blend", "blend-frames.json");

// The historical hardcoded set — always emitted so the existing renderer stays byte-identical even if
// pixel selection drifts. (These ten always pass the pixel test, but listing them is belt-and-suspenders.)
const ALWAYS_INCLUDE = new Set();
for (let frame = 2723; frame <= 2732; frame += 1) {
  ALWAYS_INCLUDE.add(`Objects/${frame}`);
}

function applyDarknessRamp(buffer) {
  for (let offset = 0; offset < buffer.length; offset += 4) {
    const red = buffer[offset] ?? 0;
    const green = buffer[offset + 1] ?? 0;
    const blue = buffer[offset + 2] ?? 0;
    const alpha = buffer[offset + 3] ?? 0;
    const brightness = Math.max(red, green, blue);

    if (alpha === 0 || brightness < MATTE_CUTOFF) {
      buffer[offset + 3] = 0;
      continue;
    }

    if (brightness < CORE_CUTOFF) {
      buffer[offset + 3] = Math.round(alpha * ((brightness - MATTE_CUTOFF) / (CORE_CUTOFF - MATTE_CUTOFF)));
    }
  }
}

// Returns { darkMatte, brightCore } as a share of opaque pixels, or null if the frame is fully empty.
function classifyGlow(buffer) {
  let opaque = 0;
  let dark = 0;
  let bright = 0;
  for (let offset = 0; offset < buffer.length; offset += 4) {
    const alpha = buffer[offset + 3] ?? 0;
    if (alpha === 0) {
      continue;
    }
    opaque += 1;
    const brightness = Math.max(buffer[offset] ?? 0, buffer[offset + 1] ?? 0, buffer[offset + 2] ?? 0);
    if (brightness < MATTE_CUTOFF) {
      dark += 1;
    } else if (brightness >= CORE_CUTOFF) {
      bright += 1;
    }
  }
  if (opaque === 0) {
    return null;
  }
  return { darkMatte: dark / opaque, brightCore: bright / opaque };
}

async function listFrames(libDir) {
  let entries;
  try {
    entries = await fs.readdir(libDir);
  } catch {
    return [];
  }
  return entries
    .filter((name) => /^\d+\.png$/i.test(name))
    .map((name) => Number.parseInt(name, 10))
    .filter((n) => Number.isFinite(n))
    .sort((a, b) => a - b);
}

const onlyLib = process.env.BLEND_LIB ?? null; // optional: restrict to a single lib for fast iteration
const libs = onlyLib ? [onlyLib] : OBJECTS_LIBS;

const blendFrames = []; // manifest keys: "<lib>/<frame>"
const missingSources = [];
let generated = 0;
let scanned = 0;

for (const lib of libs) {
  const sourceDir = path.join(MAP_ROOT, lib);
  const outputDir = path.join(OUTPUT_ROOT, lib);
  const frames = await listFrames(sourceDir);
  if (frames.length === 0) {
    // Lib dir absent locally (R2-only) — record as missing so it is visible, but don't fail.
    missingSources.push(`${lib}/* (lib dir absent locally)`);
    continue;
  }

  let ensuredOutput = false;
  for (const frame of frames) {
    scanned += 1;
    const key = `${lib}/${frame}`;
    const sourcePath = path.join(sourceDir, `${frame}.png`);
    let raw;
    try {
      raw = await sharp(sourcePath).ensureAlpha().raw().toBuffer({ resolveWithObject: true });
    } catch {
      missingSources.push(key);
      continue;
    }

    const buffer = Buffer.from(raw.data);
    const stats = classifyGlow(buffer);
    const forced = ALWAYS_INCLUDE.has(key);
    const isGlow = forced || (stats !== null && stats.darkMatte >= DARK_MATTE_MIN && stats.brightCore >= BRIGHT_CORE_MIN);
    if (!isGlow) {
      continue;
    }

    applyDarknessRamp(buffer);

    if (!ensuredOutput) {
      await fs.mkdir(outputDir, { recursive: true });
      ensuredOutput = true;
    }
    await sharp(buffer, {
      raw: { width: raw.info.width, height: raw.info.height, channels: 4 },
    })
      .png({ compressionLevel: 9, adaptiveFiltering: false })
      .toFile(path.join(outputDir, `${frame}.png`));

    blendFrames.push(key);
    generated += 1;
  }
}

blendFrames.sort();
await fs.mkdir(path.dirname(MANIFEST_PATH), { recursive: true });
await fs.writeFile(
  MANIFEST_PATH,
  `${JSON.stringify({ lib: "WemadeMir2", frames: blendFrames }, null, 2)}\n`,
  "utf8",
);

console.log(`scanned ${scanned} frames; generated ${generated} blend assets across ${libs.length} lib(s).`);
console.log(`manifest: ${path.relative(process.cwd(), MANIFEST_PATH)} (${blendFrames.length} entries)`);
if (missingSources.length > 0) {
  console.log(`missing local sources (R2-only, skipped): ${missingSources.length}`);
  for (const key of missingSources.slice(0, 40)) {
    console.log(`  - ${key}`);
  }
  if (missingSources.length > 40) {
    console.log(`  ... and ${missingSources.length - 40} more`);
  }
}
