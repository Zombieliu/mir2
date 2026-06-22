// export-crystal-magic-effects.mjs — produces the effect atlas manifest `lib/crystal-magic-effects.ts`
// (loadEffectAssets) consumes, so spell casts render the REAL Crystal frames instead of the
// procedural CSS fallback (lib/vfx-fallback.ts). Until this ran, /original-effects/effects.generated.json
// was an empty stub → resolveSpellEffect returned null for every spell (see `npm run qa:vfx`).
//
// KEY: the effect frame PNGs already exist in the full-Crystal R2 release (it extracts EVERY .Lib),
// at /original-ui/Magic|Magic2|Magic3/<idx>.png + a /original-ui/<lib>/meta.json with geometry. So
// the fix is NOT to re-extract `.Lib` — it is to ASSEMBLE a small manifest that points the effect
// system at those already-uploaded frames:
//
//   PRIMARY mode (--assetBaseUrl <R2 release base> | $MIR2_ASSET_BASE_URL | $NEXT_PUBLIC_MIR2_ASSET_BASE_URL):
//     fetch /original-ui/<lib>/meta.json from R2, and for the frame ranges SPELL_EFFECTS references
//     write public/original-effects/<lib>/meta.json (frames keyed by index, path → /original-ui/<lib>/<idx>.png
//     so the SW R2-backfill serves them) + effects.generated.json. Runs locally, committable, no `.Lib`.
//
//   SECONDARY mode (--fromLib / CRYSTAL_CLIENT_ROOT): extract the frames straight from Magic*.Lib using
//     scripts/crystal-library.mjs (for a from-raw re-export when R2 isn't the source of truth).
//
// The spell→effect table is ported 1:1 from the Crystal CLIENT cast switch
// `Crystal/Client/MirObjects/PlayerObject.cs` `case MirAction.Spell:` (region per spell), where
// `Effects.Add(new Effect(Libraries.<Lib>, <base>, <count>, <duration>, this))` is the caster's cast
// animation. interval = per-frame ms (explicit ms → ms/count; `Frame.Count*FrameInterval` → ≈600/count;
// `N*FrameInterval` → ≈100). Directional spells export the dir-0 slice (the web EffectSpec is not yet
// direction-aware); `MapControl.Effects` casts are tile-anchored (kind:"ground"). Curated seed of ~62
// high-traffic spells; extend by adding a SPELL_EFFECTS row (151 player + 414 monster sites exist).
//
// Usage:  node ./scripts/export-crystal-magic-effects.mjs --assetBaseUrl https://assets.mir2.obelisk.build/mir2/v/<release>

import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  allPresentFrameIndices,
  decodeFrameRgba,
  encodePng,
  parseLibrary,
} from "./crystal-library.mjs";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const LOCAL_CRYSTAL_CLIENT_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const DEFAULT_PUBLIC_DIR = path.join(WORKSPACE_ROOT, "public", "original-effects");
const DEFAULT_DATA_DIR = "E:\\mir2\\Crystal\\Build\\Client\\Debug\\Data";

// ---------------------------------------------------------------------------
// Spell → cast effect, ported 1:1 from PlayerObject.cs `case MirAction.Spell:`.
// kind: "cast" = on the caster (Effects.Add(...this)); "ground" = MapControl.Effects (tile-anchored).
// directional: base is `base + Direction*stride`; we export the dir-0 slice.
// ---------------------------------------------------------------------------
export const SPELL_EFFECTS = [
  { spell: "FireBall", library: "Magic", base: 0, count: 10, interval: 60, kind: "cast" },
  { spell: "Healing", library: "Magic", base: 200, count: 10, interval: 60, kind: "cast" },
  { spell: "Repulsion", library: "Magic", base: 900, count: 6, interval: 100, kind: "cast" },
  { spell: "ElectricShock", library: "Magic", base: 1560, count: 10, interval: 60, kind: "cast" },
  { spell: "Poisoning", library: "Magic", base: 600, count: 10, interval: 60, kind: "cast" },
  { spell: "GreatFireBall", library: "Magic", base: 400, count: 10, interval: 60, kind: "cast" },
  { spell: "HellFire", library: "Magic", base: 920, count: 10, interval: 60, kind: "cast" },
  { spell: "ThunderBolt", library: "Magic2", base: 20, count: 3, interval: 100, kind: "cast" },
  { spell: "SummonSkeleton", library: "Magic", base: 1500, count: 10, interval: 60, kind: "cast" },
  { spell: "StormEscape", library: "Magic3", base: 590, count: 10, interval: 60, kind: "cast" },
  { spell: "Teleport", library: "Magic", base: 1590, count: 10, interval: 60, kind: "cast" },
  { spell: "Blink", library: "Magic", base: 1590, count: 10, interval: 60, kind: "cast" },
  { spell: "Hiding", library: "Magic", base: 1520, count: 10, interval: 60, kind: "cast" },
  { spell: "Haste", library: "Magic2", base: 2140, count: 6, interval: 100, kind: "cast", directional: 10 },
  { spell: "Fury", library: "Magic3", base: 200, count: 8, interval: 100, kind: "cast" },
  { spell: "ImmortalSkin", library: "Magic3", base: 550, count: 17, interval: 140, kind: "cast" },
  { spell: "FireBang", library: "Magic", base: 1650, count: 10, interval: 60, kind: "cast" },
  { spell: "FireWall", library: "Magic", base: 1620, count: 10, interval: 60, kind: "cast" },
  { spell: "HealingCircle", library: "Magic3", base: 620, count: 10, interval: 60, kind: "cast" },
  { spell: "MoonMist", library: "Magic3", base: 680, count: 25, interval: 72, kind: "ground" },
  { spell: "TrapHexagon", library: "Magic", base: 1380, count: 10, interval: 60, kind: "cast" },
  { spell: "EnergyRepulsor", library: "Magic2", base: 190, count: 6, interval: 100, kind: "cast" },
  { spell: "FireBurst", library: "Magic2", base: 2320, count: 10, interval: 60, kind: "cast" },
  { spell: "FlameDisruptor", library: "Magic2", base: 130, count: 6, interval: 100, kind: "cast" },
  { spell: "SummonShinsu", library: "Magic2", base: 0, count: 10, interval: 60, kind: "cast" },
  { spell: "UltimateEnhancer", library: "Magic2", base: 160, count: 15, interval: 67, kind: "cast" },
  { spell: "FrostCrunch", library: "Magic2", base: 400, count: 10, interval: 60, kind: "cast" },
  { spell: "Purification", library: "Magic2", base: 600, count: 10, interval: 60, kind: "cast" },
  { spell: "FlameField", library: "Magic2", base: 910, count: 23, interval: 78, kind: "ground" },
  { spell: "Trap", library: "Magic2", base: 2340, count: 11, interval: 100, kind: "cast" },
  { spell: "MoonLight", library: "Magic2", base: 2380, count: 10, interval: 60, kind: "cast" },
  { spell: "SwiftFeet", library: "Magic2", base: 2440, count: 16, interval: 100, kind: "cast" },
  { spell: "LightBody", library: "Magic2", base: 2470, count: 10, interval: 60, kind: "cast" },
  { spell: "PoisonSword", library: "Magic2", base: 2490, count: 10, interval: 110, kind: "cast", directional: 10 },
  { spell: "DarkBody", library: "Magic2", base: 2580, count: 10, interval: 100, kind: "cast" },
  { spell: "ThunderStorm", library: "Magic", base: 1680, count: 10, interval: 60, kind: "ground" },
  { spell: "MassHealing", library: "Magic", base: 1790, count: 10, interval: 60, kind: "cast" },
  { spell: "IceStorm", library: "Magic", base: 3840, count: 10, interval: 60, kind: "cast" },
  { spell: "MagicShield", library: "Magic", base: 3880, count: 10, interval: 60, kind: "cast" },
  { spell: "TurnUndead", library: "Magic", base: 3920, count: 10, interval: 60, kind: "cast" },
  { spell: "MagicBooster", library: "Magic3", base: 80, count: 9, interval: 100, kind: "cast" },
  { spell: "PetEnhancer", library: "Magic3", base: 200, count: 8, interval: 100, kind: "cast" },
  { spell: "Revelation", library: "Magic", base: 3960, count: 20, interval: 60, kind: "cast" },
  { spell: "ProtectionField", library: "Magic2", base: 1520, count: 10, interval: 60, kind: "cast" },
  { spell: "Rage", library: "Magic2", base: 1510, count: 10, interval: 60, kind: "cast" },
  { spell: "Vampirism", library: "Magic2", base: 1040, count: 7, interval: 86, kind: "cast" },
  { spell: "LionRoar", library: "Magic2", base: 710, count: 20, interval: 60, kind: "cast" },
  { spell: "BattleCry", library: "Magic2", base: 710, count: 20, interval: 60, kind: "cast" },
  { spell: "TwinDrakeBlade", library: "Magic2", base: 210, count: 6, interval: 83, kind: "cast" },
  { spell: "Entrapment", library: "Magic2", base: 990, count: 10, interval: 60, kind: "cast" },
  { spell: "BladeAvalanche", library: "Magic2", base: 740, count: 15, interval: 100, kind: "cast", directional: 20 },
  { spell: "SlashingBurst", library: "Magic2", base: 1700, count: 9, interval: 100, kind: "ground", directional: 10 },
  { spell: "CounterAttack", library: "Magic", base: 3480, count: 10, interval: 100, kind: "cast", directional: 10 },
  { spell: "CrescentSlash", library: "Magic2", base: 2620, count: 20, interval: 100, kind: "cast", directional: 20 },
  { spell: "Mirroring", library: "Magic2", base: 650, count: 10, interval: 60, kind: "cast" },
  { spell: "Blizzard", library: "Magic2", base: 1540, count: 8, interval: 75, kind: "cast" },
  { spell: "MeteorStrike", library: "Magic2", base: 1590, count: 10, interval: 60, kind: "cast" },
  { spell: "HeavenlySword", library: "Magic2", base: 2230, count: 8, interval: 100, kind: "cast", directional: 10 },
  { spell: "ElementalBarrier", library: "Magic3", base: 1880, count: 8, interval: 75, kind: "cast" },
  { spell: "PoisonShot", library: "Magic3", base: 2300, count: 8, interval: 125, kind: "cast" },
  { spell: "OneWithNature", library: "Magic3", base: 2710, count: 8, interval: 150, kind: "ground" },
  { spell: "FireBounce", library: "Magic", base: 400, count: 10, interval: 60, kind: "cast" },
];

// The frame indices a spell's effect spans (dir-0 slice for directional spells).
function frameRange(spec) {
  const out = [];
  for (let i = 0; i < spec.count; i += 1) out.push(spec.base + i);
  return out;
}

// lib -> Set(frame indices referenced by SPELL_EFFECTS).
function neededByLibrary() {
  const map = new Map();
  for (const spec of SPELL_EFFECTS) {
    if (!map.has(spec.library)) map.set(spec.library, new Set());
    const set = map.get(spec.library);
    for (const index of frameRange(spec)) set.add(index);
  }
  return map;
}

function effectSpecForManifest(spec) {
  return {
    spell: spec.spell,
    kind: spec.kind,
    library: spec.library,
    base: spec.base,
    count: spec.count,
    interval: spec.interval,
    blend: true,
    light: 0,
    repeat: false,
    offset: { x: 0, y: 0 },
  };
}

async function writeEffectsManifest(outputDir, available, mode) {
  const manifest = {
    generatedAt: new Date().toISOString(),
    source: "Crystal/Client/MirObjects/PlayerObject.cs (case MirAction.Spell)",
    mode,
    available: [...available].sort(),
    spell_effect_enum: [],
    spell_effects: SPELL_EFFECTS.map(effectSpecForManifest),
    ground_effects: SPELL_EFFECTS.filter((s) => s.kind === "ground").map(effectSpecForManifest),
    map_effects: [],
  };
  await writeFile(path.join(outputDir, "effects.generated.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}

// Index → frame meta from an extracted /original-ui/<lib>/meta.json (array `frames:[{index,...}]`
// or object `frames:{idx:{...}}`).
function indexFrameLookup(srcMeta) {
  const frames = srcMeta?.frames ?? {};
  if (Array.isArray(frames)) {
    return new Map(frames.filter(Boolean).map((fr) => [Number(fr.index), fr]));
  }
  return new Map(Object.entries(frames).map(([k, v]) => [Number(k), v]));
}

/**
 * PRIMARY: assemble the effect manifest from the already-extracted /original-ui/<lib>/meta.json
 * (the full-Crystal R2 release). Frame `path`s point at /original-ui/<lib>/<idx>.png (R2-backfilled),
 * so no frames are re-extracted or re-uploaded. Returns a summary; only touches `outputDir`.
 */
export async function assembleMagicEffectsFromMeta({ assetBaseUrl, outputDir = DEFAULT_PUBLIC_DIR, fetchImpl = fetch } = {}) {
  const base = String(assetBaseUrl ?? "").replace(/\/$/, "");
  await mkdir(outputDir, { recursive: true });

  const available = [];
  const perLibrary = {};
  for (const [library, indexSet] of neededByLibrary()) {
    const metaUrl = `${base}/original-ui/${library}/meta.json`;
    const res = await fetchImpl(metaUrl);
    if (!res.ok) throw new Error(`missing source meta: ${metaUrl} (${res.status})`);
    const lookup = indexFrameLookup(await res.json());

    const frames = {};
    let found = 0;
    let missing = 0;
    for (const index of [...indexSet].sort((a, b) => a - b)) {
      const fr = lookup.get(index);
      if (!fr) {
        missing += 1;
        continue;
      }
      frames[String(index)] = {
        path: `/original-ui/${library}/${index}.png`,
        width: fr.width,
        height: fr.height,
        x: fr.x,
        y: fr.y,
      };
      found += 1;
    }
    await mkdir(path.join(outputDir, library), { recursive: true });
    await writeFile(path.join(outputDir, library, "meta.json"), `${JSON.stringify({ frames }, null, 2)}\n`);
    available.push(library);
    perLibrary[library] = { requested: indexSet.size, found, missing };
  }

  await writeEffectsManifest(outputDir, available, "assemble-from-r2-meta");
  return { outputDir, available, spellCount: SPELL_EFFECTS.length, perLibrary };
}

/**
 * SECONDARY: extract the referenced effect frames straight from a real Crystal client's Magic*.Lib.
 * `dataDir` must point at <client>/Data. Use when R2 isn't the source of truth (a from-raw re-export).
 */
export async function runCrystalMagicEffectExport({ dataDir, outputDir = DEFAULT_PUBLIC_DIR, deflateLevel = 1 } = {}) {
  await mkdir(outputDir, { recursive: true });
  const available = [];
  const perLibrary = {};
  for (const [library, indexSet] of neededByLibrary()) {
    const libPath = path.join(dataDir, ...library.split("/")) + ".Lib";
    if (!existsSync(libPath)) {
      throw new Error(`missing effect library: ${libPath}`);
    }
    const lib = parseLibrary(await readFile(libPath));
    const present = new Set(allPresentFrameIndices(lib));
    const exportDir = path.join(outputDir, library);
    await mkdir(exportDir, { recursive: true });

    const frames = {};
    let written = 0;
    let missing = 0;
    for (const index of [...indexSet].sort((a, b) => a - b)) {
      const frame = lib.frames[index];
      if (!frame || !present.has(index)) {
        missing += 1;
        continue;
      }
      await writeFile(path.join(exportDir, `${index}.png`), encodePng(frame.width, frame.height, decodeFrameRgba(lib, frame), deflateLevel));
      frames[String(index)] = {
        path: `/original-effects/${library}/${index}.png`,
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
      };
      written += 1;
    }
    await writeFile(path.join(exportDir, "meta.json"), `${JSON.stringify({ count: lib.count, frames }, null, 2)}\n`);
    available.push(library);
    perLibrary[library] = { requested: indexSet.size, written, missing };
  }

  await writeEffectsManifest(outputDir, available, "extract-from-lib");
  return { outputDir, available, spellCount: SPELL_EFFECTS.length, perLibrary };
}

function dataDirFromClientRoot(clientRoot) {
  if (!clientRoot) return null;
  const root = path.resolve(clientRoot);
  return path.basename(root).toLowerCase() === "data" ? root : path.join(root, "Data");
}

function parseArgs(argv) {
  const parsed = { _: [] };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith("--")) {
      parsed._.push(arg);
      continue;
    }
    const eq = arg.indexOf("=");
    if (eq !== -1) {
      parsed[arg.slice(2, eq)] = arg.slice(eq + 1);
      continue;
    }
    const key = arg.slice(2);
    const next = argv[i + 1];
    if (next && !next.startsWith("--")) {
      parsed[key] = next;
      i += 1;
    } else {
      parsed[key] = "true";
    }
  }
  return parsed;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const outputDir = path.resolve(args.outputDir ?? DEFAULT_PUBLIC_DIR);
  const assetBaseUrl =
    args.assetBaseUrl ?? process.env.MIR2_ASSET_BASE_URL ?? process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
  const dataDir =
    args.fromLib === undefined
      ? null
      : args.dataDir ??
        args._[0] ??
        process.env.CRYSTAL_CLIENT_DATA_DIR ??
        dataDirFromClientRoot(process.env.CRYSTAL_CLIENT_ROOT) ??
        (existsSync(LOCAL_CRYSTAL_CLIENT_ROOT) ? path.join(LOCAL_CRYSTAL_CLIENT_ROOT, "Data") : DEFAULT_DATA_DIR);

  // PRIMARY: assemble from the already-extracted R2 frames (no .Lib needed).
  if (assetBaseUrl) {
    const summary = await assembleMagicEffectsFromMeta({ assetBaseUrl, outputDir });
    for (const [lib, s] of Object.entries(summary.perLibrary)) {
      console.log(`[magic-effects] ${lib}: ${s.found}/${s.requested} frames mapped${s.missing ? ` (${s.missing} missing in source meta)` : ""}`);
    }
    console.log(`[magic-effects] assembled ${summary.spellCount} spells from ${assetBaseUrl} → ${path.join(outputDir, "effects.generated.json")}`);
    return;
  }

  // SECONDARY: extract from a real client's .Lib.
  if (dataDir && existsSync(dataDir)) {
    const summary = await runCrystalMagicEffectExport({ dataDir, outputDir });
    for (const [lib, s] of Object.entries(summary.perLibrary)) {
      console.log(`[magic-effects] ${lib}: wrote ${s.written}/${s.requested} frames${s.missing ? ` (${s.missing} missing in .Lib)` : ""}`);
    }
    console.log(`[magic-effects] extracted ${summary.spellCount} spells from ${dataDir} → ${path.join(outputDir, "effects.generated.json")}`);
    return;
  }

  console.log(
    "[magic-effects] No source. Pass --assetBaseUrl <R2 release base> (preferred — the effect frames are already " +
      "in the full-Crystal R2 release at /original-ui/Magic*/), or --fromLib with CRYSTAL_CLIENT_ROOT to extract from .Lib. " +
      `Wrote nothing (procedural fallback stays). ${SPELL_EFFECTS.length} spells are mapped and ready.`,
  );
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
