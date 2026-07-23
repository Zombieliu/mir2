// Builds deterministic Crystal effect metadata from already-exported frame metadata or raw .Lib
// files. Crystal's Effect defaults are Blend=true, Light=6, Repeat=false, DrawOffset=Point.Empty.

import { existsSync } from "node:fs";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  allPresentFrameIndices,
  decodeFrameRgba,
  decodeMaskFrameRgba,
  encodePng,
  parseLibrary,
} from "./crystal-library.mjs";

const WORKSPACE_ROOT = path.resolve(import.meta.dirname, "..");
const REPO_ROOT = path.resolve(WORKSPACE_ROOT, "..", "..");
const MIR2_ROOT = path.resolve(REPO_ROOT, "..");
const LOCAL_CRYSTAL_CLIENT_ROOT = path.join(MIR2_ROOT, "downloads", "crystal-client-full");
const DEFAULT_PUBLIC_DIR = path.join(WORKSPACE_ROOT, "public", "original-effects");
const DEFAULT_DATA_DIR = "E:\\mir2\\Crystal\\Build\\Client\\Debug\\Data";
const DIRECTION_COUNT = 8;

const SPELL_IDS = {
  TwinDrakeBlade: 6, Entrapment: 7, LionRoar: 9, BladeAvalanche: 11,
  ProtectionField: 12, Rage: 13, CounterAttack: 14, SlashingBurst: 15, Fury: 16,
  ImmortalSkin: 17, FireBall: 31, Repulsion: 32, ElectricShock: 33,
  GreatFireBall: 34, HellFire: 35, ThunderBolt: 36, Teleport: 37, FireBang: 38,
  FireWall: 39, FrostCrunch: 41, ThunderStorm: 42, MagicShield: 43,
  TurnUndead: 44, Vampirism: 45, IceStorm: 46, FlameDisruptor: 47, Mirroring: 48,
  FlameField: 49, Blizzard: 50, MagicBooster: 51, MeteorStrike: 52,
  StormEscape: 55, Healing: 61, Poisoning: 63, SummonSkeleton: 65, Hiding: 67,
  Revelation: 70, EnergyRepulsor: 72, TrapHexagon: 73, Purification: 74,
  MassHealing: 75, UltimateEnhancer: 77, SummonShinsu: 78, PetEnhancer: 85,
  HealingCircle: 86, Haste: 93, LightBody: 95, HeavenlySword: 96, FireBurst: 97,
  Trap: 98, PoisonSword: 99, MoonLight: 100, SwiftFeet: 102, DarkBody: 103,
  CrescentSlash: 105, MoonMist: 106, ElementalBarrier: 131, PoisonShot: 135,
  OneWithNature: 139, Blink: 151, BattleCry: 153, FireBounce: 154,
};

export const SPELL_EFFECT_ENUM = [
  "None", "FatalSword", "Teleport", "Healing", "RedMoonEvil", "TwinDrakeBlade",
  "MagicShieldUp", "MagicShieldDown", "GreatFoxSpirit", "Entrapment", "Reflect",
  "Critical", "Mine", "ElementalBarrierUp", "ElementalBarrierDown", "DelayedExplosion",
  "MPEater", "Hemorrhage", "Bleeding", "AwakeningSuccess", "AwakeningFail",
  "AwakeningMiss", "AwakeningHit", "StormEscape", "TurtleKing", "Behemoth", "Stunned",
  "IcePillar", "KingGuard", "KingGuard2", "DeathCrawlerBreath", "FlamingMutantWeb",
  "FurbolgWarriorCritical", "Tester", "MoonMist",
].map((name, id) => ({ id, name }));

const PLAYER_SPELL_SOURCE = "Crystal/Client/MirObjects/PlayerObject.cs::MirAction.Spell";
const WORLD_SPELL_SOURCE = "Crystal/Client/MirObjects/SpellObject.cs::Load";
const OBJECT_EFFECT_SOURCE = "Crystal/Client/MirScenes/GameScene.cs::ObjectEffect";
const MAP_EFFECT_SOURCE = "Crystal/Client/MirScenes/GameScene.cs::MapEffect";

const spell = (name, library, base, count, interval, kind = "cast", directionStride) => ({
  spell: name,
  spellId: SPELL_IDS[name],
  library,
  base,
  count,
  interval,
  kind,
  ...(directionStride ? {
    directionCount: DIRECTION_COUNT,
    directionStride,
    directionRanges: Array.from({ length: DIRECTION_COUNT }, (_, direction) => ({
      direction,
      base: base + direction * directionStride,
      end: base + direction * directionStride + count - 1,
    })),
  } : {}),
  blend: true,
  light: 6,
  repeat: false,
  offset: { x: 0, y: 0 },
  provenance: { source: PLAYER_SPELL_SOURCE, symbol: `Spell.${name}` },
});

// Cast effects constructed immediately by PlayerObject's MirAction.Spell switch.
export const SPELL_EFFECTS = [
  spell("FireBall", "Magic", 0, 10, 60),
  spell("Healing", "Magic", 200, 10, 60),
  spell("Repulsion", "Magic", 900, 6, 100),
  spell("ElectricShock", "Magic", 1560, 10, 60),
  spell("Poisoning", "Magic", 600, 10, 60),
  spell("GreatFireBall", "Magic", 400, 10, 60),
  spell("HellFire", "Magic", 920, 10, 60),
  spell("ThunderBolt", "Magic2", 20, 3, 100),
  spell("SummonSkeleton", "Magic", 1500, 10, 60),
  spell("StormEscape", "Magic3", 590, 10, 60),
  spell("Teleport", "Magic", 1590, 10, 60),
  spell("Blink", "Magic", 1590, 10, 60),
  spell("Hiding", "Magic", 1520, 10, 60),
  spell("Haste", "Magic2", 2140, 6, 100, "cast", 10),
  spell("Fury", "Magic3", 200, 8, 100),
  spell("ImmortalSkin", "Magic3", 550, 17, 141),
  spell("FireBang", "Magic", 1650, 10, 60),
  spell("FireWall", "Magic", 1620, 10, 60),
  spell("HealingCircle", "Magic3", 620, 10, 60),
  spell("MoonMist", "Magic3", 680, 25, 72, "ground"),
  spell("TrapHexagon", "Magic", 1380, 10, 60),
  spell("EnergyRepulsor", "Magic2", 190, 6, 100),
  spell("FireBurst", "Magic2", 2320, 10, 60),
  spell("FlameDisruptor", "Magic2", 130, 6, 100),
  spell("SummonShinsu", "Magic2", 0, 10, 60),
  spell("UltimateEnhancer", "Magic2", 160, 15, 66),
  spell("FrostCrunch", "Magic2", 400, 10, 60),
  spell("Purification", "Magic2", 600, 10, 60),
  spell("FlameField", "Magic2", 910, 23, 78, "ground"),
  spell("Trap", "Magic2", 2340, 11, 100),
  spell("MoonLight", "Magic2", 2380, 10, 60),
  spell("SwiftFeet", "Magic2", 2440, 16, 100),
  spell("LightBody", "Magic2", 2470, 10, 60),
  spell("PoisonSword", "Magic2", 2490, 10, 110, "cast", 10),
  spell("DarkBody", "Magic2", 2580, 10, 100),
  spell("ThunderStorm", "Magic", 1680, 10, 60, "ground"),
  spell("MassHealing", "Magic", 1790, 10, 60),
  spell("IceStorm", "Magic", 3840, 10, 60),
  spell("MagicShield", "Magic", 3880, 10, 60),
  spell("TurnUndead", "Magic", 3920, 10, 60),
  spell("MagicBooster", "Magic3", 80, 9, 100),
  spell("PetEnhancer", "Magic3", 200, 8, 100),
  spell("Revelation", "Magic", 3960, 20, 60),
  spell("ProtectionField", "Magic2", 1520, 10, 60),
  spell("Rage", "Magic2", 1510, 10, 60),
  spell("Vampirism", "Magic2", 1040, 7, 85),
  spell("LionRoar", "Magic2", 710, 20, 60),
  spell("BattleCry", "Magic2", 710, 20, 60),
  spell("TwinDrakeBlade", "Magic2", 210, 6, 83),
  spell("Entrapment", "Magic2", 990, 10, 60),
  spell("BladeAvalanche", "Magic2", 740, 15, 100, "cast", 20),
  spell("SlashingBurst", "Magic2", 1700, 9, 100, "ground", 10),
  spell("CounterAttack", "Magic", 3480, 10, 100, "cast", 10),
  spell("CrescentSlash", "Magic2", 2620, 20, 100, "cast", 20),
  spell("Mirroring", "Magic2", 650, 10, 60),
  spell("Blizzard", "Magic2", 1540, 8, 75),
  spell("MeteorStrike", "Magic2", 1590, 10, 60),
  spell("HeavenlySword", "Magic2", 2230, 8, 100, "cast", 10),
  spell("ElementalBarrier", "Magic3", 1880, 8, 75),
  spell("PoisonShot", "Magic3", 2300, 8, 125),
  spell("OneWithNature", "Magic3", 2710, 8, 150, "ground"),
  spell("FireBounce", "Magic", 400, 10, 60),
];

const worldSpell = (name, library, base, count, interval, extra = {}) => ({
  spell: name,
  spellId: SPELL_IDS[name],
  library,
  base,
  count,
  interval,
  kind: "ground",
  blend: true,
  light: 0,
  repeat: true,
  offset: { x: 0, y: 0 },
  ...extra,
  provenance: { source: WORLD_SPELL_SOURCE, symbol: `Spell.${name}` },
});

// Packet-backed world spell objects use a different frame range from the
// caster's MirAction.Spell animation and remain alive until ObjectRemove.
export const WORLD_SPELL_EFFECTS = [
  worldSpell("TrapHexagon", "Magic", 1390, 10, 100),
];

const packetEffect = (effect, library, base, count, interval, source, extra = {}) => ({
  effect,
  effectId: SPELL_EFFECT_ENUM.find((entry) => entry.name === effect)?.id,
  kind: source === MAP_EFFECT_SOURCE ? "ground" : "target",
  library,
  base,
  count,
  interval,
  blend: true,
  light: 6,
  repeat: false,
  offset: { x: 0, y: 0 },
  ...extra,
  provenance: { source, symbol: `SpellEffect.${effect}` },
});

// Single-layer ObjectEffect cases whose frame selection is fully determined by the packet effect.
export const OBJECT_EFFECTS = [
  packetEffect("FatalSword", "Magic2", 1940, 4, 100, OBJECT_EFFECT_SOURCE),
  packetEffect("Teleport", "Magic", 1600, 10, 60, OBJECT_EFFECT_SOURCE),
  packetEffect("Healing", "Magic", 370, 10, 80, OBJECT_EFFECT_SOURCE),
  packetEffect("TwinDrakeBlade", "Magic2", 380, 6, 133, OBJECT_EFFECT_SOURCE),
  packetEffect("MagicShieldUp", "Magic", 3890, 3, 200, OBJECT_EFFECT_SOURCE, { repeat: true }),
  packetEffect("ElementalBarrierUp", "Magic3", 1890, 10, 200, OBJECT_EFFECT_SOURCE, { repeat: true }),
  packetEffect("ElementalBarrierDown", "Magic3", 1910, 7, 200, OBJECT_EFFECT_SOURCE),
  packetEffect("MPEater", "Magic2", 2400, 9, 100, OBJECT_EFFECT_SOURCE),
  packetEffect("Bleeding", "Magic3", 60, 3, 133, OBJECT_EFFECT_SOURCE),
  packetEffect("StormEscape", "Magic3", 610, 10, 60, OBJECT_EFFECT_SOURCE),
  packetEffect("MoonMist", "Magic3", 705, 10, 80, OBJECT_EFFECT_SOURCE),
];

export const MAP_EFFECTS = [
  packetEffect("Mine", "Effect", 0, 3, 80, MAP_EFFECT_SOURCE, {
    light: 0,
    valueCount: 8,
    valueStride: 8,
    valueRanges: Array.from({ length: 8 }, (_, value) => ({ value, base: value * 8, end: value * 8 + 2 })),
  }),
  packetEffect("Tester", "Effect", 328, 10, 50, MAP_EFFECT_SOURCE, { light: 0 }),
];

const allSpecs = () => [...SPELL_EFFECTS, ...WORLD_SPELL_EFFECTS, ...OBJECT_EFFECTS, ...MAP_EFFECTS];

function assertInteger(value, label, minimum = 0) {
  if (!Number.isInteger(value) || value < minimum) throw new Error(`${label} must be an integer >= ${minimum}`);
}

export function validateEffectDefinitions() {
  const enumIds = new Set();
  const enumNames = new Set();
  for (const entry of SPELL_EFFECT_ENUM) {
    assertInteger(entry.id, `SpellEffect.${entry.name} id`);
    if (!entry.name || enumIds.has(entry.id) || enumNames.has(entry.name)) throw new Error("duplicate/empty SpellEffect mapping");
    enumIds.add(entry.id);
    enumNames.add(entry.name);
  }
  const keys = new Set();
  for (const spec of allSpecs()) {
    const name = spec.spell ?? spec.effect;
    const key = `${spec.provenance.source}:${name}`;
    if (!name || keys.has(key)) throw new Error(`duplicate/empty effect definition: ${key}`);
    keys.add(key);
    if (!spec.library || !spec.provenance?.source || !spec.provenance?.symbol) throw new Error(`${name} lacks library/provenance`);
    for (const field of ["base", "count", "interval"]) assertInteger(spec[field], `${name}.${field}`, field === "base" ? 0 : 1);
    if (spec.directionStride !== undefined) {
      assertInteger(spec.directionCount, `${name}.directionCount`, 1);
      assertInteger(spec.directionStride, `${name}.directionStride`, spec.count);
      if (spec.directionRanges?.length !== spec.directionCount) throw new Error(`${name} lacks explicit direction ranges`);
      spec.directionRanges.forEach((range, direction) => {
        const base = spec.base + direction * spec.directionStride;
        if (range.direction !== direction || range.base !== base || range.end !== base + spec.count - 1) throw new Error(`${name} has an invalid direction range`);
      });
    }
    if (spec.valueStride !== undefined) assertInteger(spec.valueStride, `${name}.valueStride`, spec.count);
    if (spec.valueCount !== undefined) assertInteger(spec.valueCount, `${name}.valueCount`, 1);
    if (spec.valueCount !== undefined) {
      if (spec.valueRanges?.length !== spec.valueCount) throw new Error(`${name} lacks explicit value ranges`);
      spec.valueRanges.forEach((range, value) => {
        const base = spec.base + value * spec.valueStride;
        if (range.value !== value || range.base !== base || range.end !== base + spec.count - 1) throw new Error(`${name} has an invalid value range`);
      });
    }
    if (spec.spell && !Number.isInteger(spec.spellId)) throw new Error(`${name} lacks an authoritative Spell id`);
    if (spec.effect && (!Number.isInteger(spec.effectId) || !enumNames.has(spec.effect))) throw new Error(`${name} lacks an authoritative SpellEffect id`);
    if (typeof spec.blend !== "boolean" || typeof spec.repeat !== "boolean" || !Number.isInteger(spec.light)) throw new Error(`${name} lacks explicit render flags`);
  }
}

function frameIndices(spec) {
  const indices = [];
  const directions = spec.directionCount ?? 1;
  const values = spec.valueCount ?? 1;
  for (let value = 0; value < values; value += 1) {
    for (let direction = 0; direction < directions; direction += 1) {
      const rangeBase = spec.base + direction * (spec.directionStride ?? 0) + value * (spec.valueStride ?? 0);
      for (let frame = 0; frame < spec.count; frame += 1) indices.push(rangeBase + frame);
    }
  }
  return indices;
}

function additiveFrameIndicesByLibrary() {
  const result = new Map();
  for (const spec of allSpecs()) {
    if (!spec.blend) continue;
    if (!result.has(spec.library)) result.set(spec.library, new Set());
    for (const index of frameIndices(spec)) result.get(spec.library).add(index);
  }
  return result;
}

// DirectX additive blending treats dark texels as low energy, never as an
// opaque black overlay. Encode that same energy as alpha so DOM/Web fallback
// compositors remain safe even when CSS blend isolation prevents true add.
export function normalizeAdditiveRgba(rgba) {
  const output = Buffer.from(rgba);
  for (let offset = 0; offset + 3 < output.length; offset += 4) {
    const alpha = output[offset + 3];
    const energy = Math.max(output[offset], output[offset + 1], output[offset + 2]);
    if (alpha === 0 || energy === 0) {
      output[offset] = 0;
      output[offset + 1] = 0;
      output[offset + 2] = 0;
      output[offset + 3] = 0;
      continue;
    }
    output[offset] = Math.round((output[offset] * 255) / energy);
    output[offset + 1] = Math.round((output[offset + 1] * 255) / energy);
    output[offset + 2] = Math.round((output[offset + 2] * 255) / energy);
    output[offset + 3] = Math.round((alpha * energy) / 255);
  }
  return output;
}

function neededByLibrary() {
  const result = new Map();
  for (const spec of allSpecs()) {
    if (!result.has(spec.library)) result.set(spec.library, new Set());
    for (const index of frameIndices(spec)) result.get(spec.library).add(index);
  }
  return new Map([...result].sort(([left], [right]) => left.localeCompare(right)));
}

function manifestSpec(spec) {
  return { ...spec };
}

async function writeEffectsManifest(outputDir, available, mode) {
  validateEffectDefinitions();
  const byName = (left, right) => (left.spell ?? left.effect).localeCompare(right.spell ?? right.effect);
  const manifest = {
    schemaVersion: 2,
    generatedAt: null,
    source: "Crystal client source (see per-entry provenance)",
    mode,
    available: [...available].sort(),
    spell_effect_enum: SPELL_EFFECT_ENUM.map((entry) => entry.name),
    spell_effect_map: SPELL_EFFECT_ENUM,
    spell_effects: SPELL_EFFECTS.map(manifestSpec).sort(byName),
    ground_effects: [
      ...SPELL_EFFECTS.filter((entry) => entry.kind === "ground"),
      ...WORLD_SPELL_EFFECTS,
    ].map(manifestSpec).sort(byName),
    object_effects: OBJECT_EFFECTS.map(manifestSpec).sort(byName),
    map_effects: MAP_EFFECTS.map(manifestSpec).sort(byName),
  };
  await writeFile(path.join(outputDir, "effects.generated.json"), `${JSON.stringify(manifest, null, 2)}\n`);
}

function indexFrameLookup(srcMeta, source) {
  const frames = srcMeta?.frames;
  if (!frames || (typeof frames !== "object" && !Array.isArray(frames))) throw new Error(`invalid source meta: ${source} has no frames`);
  const entries = Array.isArray(frames)
    ? frames.filter(Boolean).map((frame) => [Number(frame.index), frame])
    : Object.entries(frames).map(([index, frame]) => [Number(index), frame]);
  const lookup = new Map();
  for (const [index, frame] of entries) {
    if (!Number.isInteger(index) || index < 0 || lookup.has(index)) throw new Error(`invalid/duplicate frame index in ${source}: ${index}`);
    for (const field of ["width", "height", "x", "y"]) {
      if (!Number.isInteger(frame?.[field])) throw new Error(`invalid ${source} frame ${index}.${field}`);
    }
    if (frame.width <= 0 || frame.height <= 0) throw new Error(`invalid ${source} frame ${index} dimensions`);
    lookup.set(index, frame);
  }
  return lookup;
}

function assertNoMissingFrames(library, missing) {
  if (missing.length) throw new Error(`${library} is missing ${missing.length} required frame(s): ${missing.join(", ")}`);
}

export async function assembleMagicEffectsFromMeta({ assetBaseUrl, outputDir = DEFAULT_PUBLIC_DIR, fetchImpl = fetch } = {}) {
  validateEffectDefinitions();
  const base = String(assetBaseUrl ?? "").replace(/\/$/, "");
  if (!base) throw new Error("assetBaseUrl is required");
  await mkdir(outputDir, { recursive: true });
  const available = [];
  const perLibrary = {};
  for (const [library, indexSet] of neededByLibrary()) {
    const metaUrl = `${base}/original-ui/${library}/meta.json`;
    const response = await fetchImpl(metaUrl);
    if (!response?.ok) throw new Error(`missing source meta: ${metaUrl} (${response?.status ?? "no response"})`);
    const lookup = indexFrameLookup(await response.json(), metaUrl);
    const frames = {};
    const missing = [];
    for (const index of [...indexSet].sort((left, right) => left - right)) {
      const frame = lookup.get(index);
      if (!frame) { missing.push(index); continue; }
      frames[String(index)] = {
        path: `/original-ui/${library}/${index}.png`,
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
        shadowX: frame.shadowX ?? 0,
        shadowY: frame.shadowY ?? 0,
        maskPath: frame.maskPath ?? null,
        maskWidth: frame.maskWidth ?? frame.width,
        maskHeight: frame.maskHeight ?? frame.height,
        maskX: frame.maskX ?? frame.x,
        maskY: frame.maskY ?? frame.y,
      };
    }
    assertNoMissingFrames(library, missing);
    await mkdir(path.join(outputDir, library), { recursive: true });
    await writeFile(path.join(outputDir, library, "meta.json"), `${JSON.stringify({ frames }, null, 2)}\n`);
    available.push(library);
    perLibrary[library] = { requested: indexSet.size, found: indexSet.size, missing: 0 };
  }
  await writeEffectsManifest(outputDir, available, "assemble-from-r2-meta");
  return { outputDir, available, spellCount: SPELL_EFFECTS.length, perLibrary };
}

export async function runCrystalMagicEffectExport({ dataDir, outputDir = DEFAULT_PUBLIC_DIR, deflateLevel = 1 } = {}) {
  validateEffectDefinitions();
  await mkdir(outputDir, { recursive: true });
  const available = [];
  const perLibrary = {};
  const additiveFrames = additiveFrameIndicesByLibrary();
  for (const [library, indexSet] of neededByLibrary()) {
    const libPath = path.join(dataDir, ...library.split("/")) + ".Lib";
    if (!existsSync(libPath)) throw new Error(`missing effect library: ${libPath}`);
    const libraryData = parseLibrary(await readFile(libPath));
    const present = new Set(allPresentFrameIndices(libraryData));
    const missing = [...indexSet].filter((index) => !libraryData.frames[index] || !present.has(index));
    assertNoMissingFrames(library, missing);
    const exportDir = path.join(outputDir, library);
    await mkdir(exportDir, { recursive: true });
    const frames = {};
    for (const index of [...indexSet].sort((left, right) => left - right)) {
      const frame = libraryData.frames[index];
      const rgba = decodeFrameRgba(libraryData, frame);
      const renderRgba = additiveFrames.get(library)?.has(index)
        ? normalizeAdditiveRgba(rgba)
        : rgba;
      await writeFile(path.join(exportDir, `${index}.png`), encodePng(frame.width, frame.height, renderRgba, deflateLevel));
      const maskPath = frame.hasMask ? `/original-effects/${library}/${index}.mask.png` : null;
      if (frame.hasMask) {
        const maskRgba = decodeMaskFrameRgba(libraryData, frame);
        const renderMaskRgba = additiveFrames.get(library)?.has(index)
          ? normalizeAdditiveRgba(maskRgba)
          : maskRgba;
        await writeFile(
          path.join(exportDir, `${index}.mask.png`),
          encodePng(frame.maskWidth, frame.maskHeight, renderMaskRgba, deflateLevel),
        );
      }
      frames[String(index)] = {
        path: `/original-effects/${library}/${index}.png`,
        width: frame.width,
        height: frame.height,
        x: frame.x,
        y: frame.y,
        shadowX: frame.shadowX,
        shadowY: frame.shadowY,
        maskPath,
        maskWidth: frame.maskWidth,
        maskHeight: frame.maskHeight,
        maskX: frame.maskX,
        maskY: frame.maskY,
      };
    }
    await writeFile(path.join(exportDir, "meta.json"), `${JSON.stringify({ count: libraryData.count, frames }, null, 2)}\n`);
    available.push(library);
    perLibrary[library] = { requested: indexSet.size, written: indexSet.size, missing: 0 };
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
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (!arg.startsWith("--")) { parsed._.push(arg); continue; }
    const equals = arg.indexOf("=");
    if (equals !== -1) { parsed[arg.slice(2, equals)] = arg.slice(equals + 1); continue; }
    const key = arg.slice(2);
    const next = argv[index + 1];
    if (next && !next.startsWith("--")) { parsed[key] = next; index += 1; } else parsed[key] = "true";
  }
  return parsed;
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const outputDir = path.resolve(args.outputDir ?? DEFAULT_PUBLIC_DIR);
  const assetBaseUrl = args.assetBaseUrl ?? process.env.MIR2_ASSET_BASE_URL ?? process.env.NEXT_PUBLIC_MIR2_ASSET_BASE_URL;
  const dataDir = args.fromLib === undefined ? null : args.dataDir ?? args._[0] ?? process.env.CRYSTAL_CLIENT_DATA_DIR ?? dataDirFromClientRoot(process.env.CRYSTAL_CLIENT_ROOT) ?? (existsSync(LOCAL_CRYSTAL_CLIENT_ROOT) ? path.join(LOCAL_CRYSTAL_CLIENT_ROOT, "Data") : DEFAULT_DATA_DIR);
  if (assetBaseUrl) {
    const summary = await assembleMagicEffectsFromMeta({ assetBaseUrl, outputDir });
    for (const [library, stats] of Object.entries(summary.perLibrary)) console.log(`[magic-effects] ${library}: ${stats.found}/${stats.requested} frames mapped`);
    console.log(`[magic-effects] assembled ${summary.spellCount} spells -> ${path.join(outputDir, "effects.generated.json")}`);
    return;
  }
  if (dataDir && existsSync(dataDir)) {
    const summary = await runCrystalMagicEffectExport({ dataDir, outputDir });
    for (const [library, stats] of Object.entries(summary.perLibrary)) console.log(`[magic-effects] ${library}: wrote ${stats.written}/${stats.requested} frames`);
    console.log(`[magic-effects] extracted ${summary.spellCount} spells -> ${path.join(outputDir, "effects.generated.json")}`);
    return;
  }
  console.log("[magic-effects] No source. Pass --assetBaseUrl <release base> or --fromLib with CRYSTAL_CLIENT_ROOT.");
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((error) => { console.error(error); process.exitCode = 1; });
}
