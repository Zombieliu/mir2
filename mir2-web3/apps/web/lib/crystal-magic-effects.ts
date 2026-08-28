// Client-side resolver + animation helpers for Crystal magic / map effects.
//
// Consumes the renderer manifest produced by scripts/export-crystal-magic-effects.mjs
// (/original-effects/effects.generated.json — spell/ground/map effect specs + which effect
// libraries were exported) plus each library's meta.json (frame index -> PNG path). When the
// effect assets are not present (no Crystal client source at build time), resolution returns
// null and callers fall back to their existing generic behaviour — so this is a no-op until
// the effect atlases are exported.

export type EffectFrameMeta = {
  path: string;
  width: number;
  height: number;
  x: number;
  y: number;
  shadowX?: number;
  shadowY?: number;
  maskPath?: string | null;
  maskWidth?: number;
  maskHeight?: number;
  maskX?: number;
  maskY?: number;
};

export type EffectSubSpec = {
  library: string;
  base: number;
  count: number;
  interval?: number;
  directionCount?: number;
  directionStride?: number;
  directionRanges?: Array<{ direction: number; base: number; end: number }>;
  kind?: "cast" | "projectile" | "impact" | "target" | "ground" | "return" | "attackOverlay";
  light?: number;
  blend?: boolean;
  rate?: number;
  repeat?: boolean;
  offset?: { x: number; y: number };
};

export type EffectSpec = {
  spell?: string;
  spellId?: number;
  effect?: string;
  effectId?: number;
  kind?: "cast" | "projectile" | "impact" | "target" | "ground" | "attackOverlay";
  library: string;
  base: number;
  count: number;
  interval: number;
  directionCount?: number;
  directionStride?: number;
  directionRanges?: Array<{ direction: number; base: number; end: number }>;
  valueCount?: number;
  valueStride?: number;
  valueRanges?: Array<{ value: number; base: number; end: number }>;
  light?: number;
  blend?: boolean;
  rate?: number;
  repeat?: boolean;
  offset?: { x: number; y: number };
  provenance?: { source: string; symbol: string };
  projectile?: EffectSubSpec;
  impact?: EffectSubSpec;
  returnEffect?: EffectSubSpec;
};

export type EffectAnimation = {
  name: string;
  kind: string;
  frames: EffectFrameMeta[];
  interval: number;
  blend: boolean;
  opacity: number;
  light: number;
  repeat: boolean;
  offset: { x: number; y: number };
  projectile?: EffectAnimation;
  impact?: EffectAnimation;
  returnEffect?: EffectAnimation;
  durationMs: number;
};

type LibraryMeta = { frames: Record<string, EffectFrameMeta> };
type NumericEffectName = { id: number; name: string };

export type EffectAssets = {
  available: Set<string>;
  libraries: Map<string, LibraryMeta>;
  spellByName: Map<string, EffectSpec>;
  mapByName: Map<string, EffectSpec>;
  groundBySpell: Map<string, EffectSpec>;
  // SpellEffect byte value -> enum name, from the manifest's spell_effect_enum (Crystal's
  // `SpellEffect` declaration order). The ObjectEffect / MapEffect packets carry the effect as a
  // raw number, so this maps it back to a name for resolveMapEffectByNumber.
  effectNameByNumber: Map<number, string>;
};

const EFFECTS_MANIFEST_URL = "/original-effects/effects.generated.json";

function libraryDirName(library: string): string {
  return library.replace(":", "_");
}

/**
 * Loads the effect renderer manifest and the per-library frame metadata. Returns an empty,
 * non-throwing asset set if the manifest or libraries are missing (assets not exported yet).
 */
export async function loadEffectAssets(
  fetchFn: typeof fetch = fetch,
): Promise<EffectAssets> {
  const empty: EffectAssets = {
    available: new Set(),
    libraries: new Map(),
    spellByName: new Map(),
    mapByName: new Map(),
    groundBySpell: new Map(),
    effectNameByNumber: new Map(),
  };
  let manifest: {
    available?: string[];
    spell_effect_enum?: Array<string | NumericEffectName>;
    spell_effect_map?: NumericEffectName[];
    spell_effects?: EffectSpec[];
    ground_effects?: EffectSpec[];
    client_effects?: EffectSpec[];
    object_effects?: EffectSpec[];
    map_effects?: EffectSpec[];
  };
  try {
    const response = await fetchFn(EFFECTS_MANIFEST_URL);
    if (!response.ok) {
      return empty;
    }
    manifest = await response.json();
  } catch {
    return empty;
  }

  const available = new Set(manifest.available ?? []);
  const libraries = new Map<string, LibraryMeta>();
  await Promise.all(
    [...available].map(async (library) => {
      try {
        const meta = await fetchFn(`/original-effects/${libraryDirName(library)}/meta.json`);
        if (meta.ok) {
          libraries.set(library, (await meta.json()) as LibraryMeta);
        }
      } catch {
        /* leave library unresolved */
      }
    }),
  );

  const spellByName = new Map<string, EffectSpec>();
  for (const entry of manifest.spell_effects ?? []) {
    if (entry.spell) spellByName.set(entry.spell, entry);
  }
  const groundBySpell = new Map<string, EffectSpec>();
  for (const entry of manifest.ground_effects ?? []) {
    if (entry.spell) groundBySpell.set(entry.spell, entry);
  }
  const mapByName = new Map<string, EffectSpec>();
  for (const entry of manifest.client_effects ?? []) {
    if (entry.effect) mapByName.set(entry.effect, entry);
  }
  for (const entry of manifest.object_effects ?? []) {
    if (entry.effect) mapByName.set(entry.effect, entry);
  }
  for (const entry of manifest.map_effects ?? []) {
    if (entry.effect) mapByName.set(entry.effect, entry);
  }
  const effectNameByNumber = new Map<number, string>();
  const numericEffectNames = manifest.spell_effect_map?.length
    ? manifest.spell_effect_map
    : (manifest.spell_effect_enum ?? []);
  numericEffectNames.forEach((entry: string | NumericEffectName, index) => {
    if (typeof entry === "string") {
      effectNameByNumber.set(index, entry);
    } else if (Number.isInteger(entry?.id) && typeof entry?.name === "string") {
      effectNameByNumber.set(entry.id, entry.name);
    }
  });

  return { available, libraries, spellByName, mapByName, groundBySpell, effectNameByNumber };
}

function resolveFrames(
  assets: EffectAssets,
  library: string,
  base: number,
  count: number,
): EffectFrameMeta[] {
  const meta = assets.libraries.get(library);
  if (!meta) {
    return [];
  }
  const frames: EffectFrameMeta[] = [];
  for (let i = 0; i < count; i += 1) {
    const frame = meta.frames[String(base + i)];
    if (frame) {
      frames.push(frame);
    }
  }
  return frames.length === count ? frames : [];
}

function resolveSub(
  assets: EffectAssets,
  sub: EffectSubSpec,
  name: string,
  fallbackKind: EffectSubSpec["kind"],
  direction = 0,
): EffectAnimation | undefined {
  if (!Number.isInteger(direction) || direction < 0) return undefined;
  if (sub.directionCount !== undefined && direction >= sub.directionCount) return undefined;
  const base = sub.base + direction * (sub.directionStride ?? 0);
  const frames = resolveFrames(assets, sub.library, base, sub.count);
  if (frames.length === 0) {
    return undefined;
  }
  const interval = sub.interval ?? 100;
  return {
    name,
    kind: sub.kind ?? fallbackKind ?? "impact",
    frames,
    interval,
    blend: sub.blend ?? true,
    opacity: Math.min(1, Math.max(0, sub.rate ?? 1)),
    light: sub.light ?? 0,
    repeat: sub.repeat ?? false,
    offset: sub.offset ?? { x: 0, y: 0 },
    durationMs: interval * frames.length,
  };
}

/** Resolves a spec entry to a playable animation, or null when its frames are unavailable. */
export function resolveAnimation(
  assets: EffectAssets,
  entry: EffectSpec,
  direction = 0,
  value = 0,
): EffectAnimation | null {
  if (!Number.isInteger(direction) || !Number.isInteger(value) || direction < 0 || value < 0) {
    return null;
  }
  if (entry.directionCount !== undefined && direction >= entry.directionCount) {
    return null;
  }
  if (entry.valueCount !== undefined && value >= entry.valueCount) {
    return null;
  }
  const base =
    entry.base +
    direction * (entry.directionStride ?? 0) +
    value * (entry.valueStride ?? 0);
  const frames = resolveFrames(assets, entry.library, base, entry.count);
  if (frames.length === 0) {
    return null;
  }
  return {
    name: entry.spell ?? entry.effect ?? "effect",
    kind: entry.kind ?? "impact",
    frames,
    interval: entry.interval,
    blend: entry.blend ?? true,
    opacity: Math.min(1, Math.max(0, entry.rate ?? 1)),
    light: entry.light ?? 6,
    repeat: entry.repeat ?? false,
    offset: entry.offset ?? { x: 0, y: 0 },
    projectile: entry.projectile
      ? resolveSub(assets, entry.projectile, entry.spell ?? entry.effect ?? "projectile", "projectile")
      : undefined,
    impact: entry.impact
      ? resolveSub(assets, entry.impact, entry.spell ?? entry.effect ?? "impact", "impact")
      : undefined,
    returnEffect: entry.returnEffect
      ? resolveSub(assets, entry.returnEffect, entry.spell ?? entry.effect ?? "return", "return")
      : undefined,
    durationMs: entry.interval * frames.length,
  };
}

export function resolveSpellEffect(
  assets: EffectAssets,
  spell: string,
  direction = 0,
): EffectAnimation | null {
  const entry = assets.spellByName.get(spell);
  return entry ? resolveAnimation(assets, entry, direction) : null;
}

/** Resolve only the actor/caster phase. Projectile/target-only entries must not be drawn on caster. */
export function resolveSpellCastEffect(
  assets: EffectAssets,
  spell: string,
  direction = 0,
): EffectAnimation | null {
  const entry = assets.spellByName.get(spell);
  if (!entry || entry.kind === "projectile" || entry.kind === "impact" || entry.kind === "target" || entry.kind === "attackOverlay") {
    return null;
  }
  return resolveAnimation(assets, entry, direction);
}

/** Resolve an attacker-bound Attack1 overlay without treating it as ObjectMagic cast art. */
export function resolveSpellAttackOverlayEffect(
  assets: EffectAssets,
  spell: string,
  direction = 0,
): EffectAnimation | null {
  const entry = assets.spellByName.get(spell);
  return entry?.kind === "attackOverlay" ? resolveAnimation(assets, entry, direction) : null;
}

export function resolveSpellProjectileEffect(
  assets: EffectAssets,
  spell: string,
  direction = 0,
): EffectAnimation | null {
  const entry = assets.spellByName.get(spell);
  if (!entry) return null;
  if (entry.projectile) {
    return resolveSub(assets, entry.projectile, spell, "projectile", direction) ?? null;
  }
  return entry.kind === "projectile" ? resolveAnimation(assets, entry) : null;
}

export function resolveSpellImpactEffect(
  assets: EffectAssets,
  spell: string,
): EffectAnimation | null {
  const entry = assets.spellByName.get(spell);
  return entry?.impact ? resolveSub(assets, entry.impact, spell, "impact") ?? null : null;
}

export function resolveSpellReturnEffect(
  assets: EffectAssets,
  spell: string,
): EffectAnimation | null {
  const entry = assets.spellByName.get(spell);
  return entry?.returnEffect
    ? resolveSub(assets, entry.returnEffect, spell, "return") ?? null
    : null;
}

export function resolveMapEffect(
  assets: EffectAssets,
  effect: string,
  value = 0,
): EffectAnimation | null {
  const entry = assets.mapByName.get(effect) ?? assets.groundBySpell.get(effect);
  return entry ? resolveAnimation(assets, entry, 0, value) : null;
}

// Built-in numeric id -> Spell enum name map, mirroring packages/protocol `Spell` (types.rs). The
// ObjectSpell/ObjectMagic packets carry the spell as this byte, and MapEffect/ObjectEffect carry a
// raw SpellEffect byte that shares the same id space for the values we care about. The exported
// manifest's `spell_effect_enum` (Crystal's SpellEffect declaration order) is PREFERRED whenever it
// is populated; this map is only a fallback so numeric ids still resolve to a stable name (for atlas
// lookup and for the procedural fallback's classification) when the manifest enum is absent.
const SPELL_NAME_BY_ID: Record<number, string> = {
  0: "None",
  1: "Fencing",
  2: "Slaying",
  3: "Thrusting",
  4: "HalfMoon",
  5: "ShoulderDash",
  6: "TwinDrakeBlade",
  7: "Entrapment",
  8: "FlamingSword",
  9: "LionRoar",
  10: "CrossHalfMoon",
  11: "BladeAvalanche",
  12: "ProtectionField",
  13: "Rage",
  14: "CounterAttack",
  15: "SlashingBurst",
  16: "Fury",
  17: "ImmortalSkin",
  31: "FireBall",
  32: "Repulsion",
  33: "ElectricShock",
  34: "GreatFireBall",
  35: "HellFire",
  36: "ThunderBolt",
  37: "Teleport",
  38: "FireBang",
  39: "FireWall",
  40: "Lightning",
  41: "FrostCrunch",
  42: "ThunderStorm",
  43: "MagicShield",
  44: "TurnUndead",
  45: "Vampirism",
  46: "IceStorm",
  47: "FlameDisruptor",
  48: "Mirroring",
  49: "FlameField",
  50: "Blizzard",
  51: "MagicBooster",
  52: "MeteorStrike",
  53: "IceThrust",
  54: "FastMove",
  55: "StormEscape",
  61: "Healing",
  62: "SpiritSword",
  63: "Poisoning",
  64: "SoulFireBall",
  65: "SummonSkeleton",
  67: "Hiding",
  68: "MassHiding",
  69: "SoulShield",
  70: "Revelation",
  71: "BlessedArmour",
  72: "EnergyRepulsor",
  73: "TrapHexagon",
  74: "Purification",
  75: "MassHealing",
  76: "Hallucination",
  77: "UltimateEnhancer",
  78: "SummonShinsu",
  79: "Reincarnation",
  80: "SummonHolyDeva",
  81: "Curse",
  82: "Plague",
  83: "PoisonCloud",
  84: "EnergyShield",
  85: "PetEnhancer",
  86: "HealingCircle",
  91: "FatalSword",
  92: "DoubleSlash",
  93: "Haste",
  94: "FlashDash",
  95: "LightBody",
  96: "HeavenlySword",
  97: "FireBurst",
  98: "Trap",
  99: "PoisonSword",
  100: "MoonLight",
  101: "MPEater",
  102: "SwiftFeet",
  103: "DarkBody",
  104: "Hemorrhage",
  105: "CrescentSlash",
  106: "MoonMist",
  107: "CatTongue",
  121: "Focus",
  122: "StraightShot",
  123: "DoubleShot",
  124: "ExplosiveTrap",
  125: "DelayedExplosion",
  126: "Meditation",
  127: "BackStep",
  128: "ElementalShot",
  129: "Concentration",
  130: "Stonetrap",
  131: "ElementalBarrier",
  132: "SummonVampire",
  133: "VampireShot",
  134: "SummonToad",
  135: "PoisonShot",
  136: "CrippleShot",
  137: "SummonSnakes",
  138: "NapalmShot",
  139: "OneWithNature",
  140: "BindingShot",
  141: "MentalState",
  151: "Blink",
  152: "Portal",
  153: "BattleCry",
  154: "FireBounce",
  155: "MeteorShower",
  200: "DigOutZombie",
  201: "Rubble",
  202: "MapLightning",
  203: "MapLava",
  204: "MapQuake1",
  205: "MapQuake2",
  206: "DigOutArmadillo",
  207: "GeneralMeowMeowThunder",
  208: "StoneGolemQuake",
  209: "EarthGolemPile",
  210: "TreeQueenRoot",
  211: "TreeQueenMassRoots",
  212: "TreeQueenGroundRoots",
  213: "TucsonGeneralRock",
  214: "FlyingStatueIceTornado",
  215: "DarkOmaKingNuke",
  216: "HornedSorcererDustTornado",
  217: "HornedCommanderRockFall",
  218: "HornedCommanderRockSpike",
};
const SPELL_ID_BY_NAME = new Map(
  Object.entries(SPELL_NAME_BY_ID).map(([id, name]) => [name, Number(id)]),
);

/**
 * Resolves a `MapEffect` / `ObjectEffect` packet's numeric `effect` (a raw SpellEffect byte) to a
 * playable animation, by mapping the number to its enum name and then resolving by name. Returns
 * null for unknown numbers, names with no map-effect entry, or unavailable frames.
 */
export function resolveMapEffectByNumber(
  assets: EffectAssets,
  effect: number,
  value = 0,
): EffectAnimation | null {
  const name = effectNameForNumber(assets, effect);
  return name ? resolveMapEffect(assets, name, value) : null;
}

/**
 * The effect/spell enum name for a numeric id. Prefers the exported manifest's `spell_effect_enum`
 * (authoritative when present) and falls back to the built-in protocol `Spell` id map, so numeric
 * ids still resolve to a stable name when the manifest enum has not been exported yet.
 */
export function effectNameForNumber(assets: EffectAssets, effect: number): string | null {
  return assets.effectNameByNumber.get(effect) ?? SPELL_NAME_BY_ID[Math.trunc(effect)] ?? null;
}

/** Spell packets use the protocol Spell enum, not the overlapping SpellEffect enum. */
export function spellNameForNumber(spell: number): string | null {
  return SPELL_NAME_BY_ID[Math.trunc(spell)] ?? null;
}

/** Convert an exact protocol Spell enum name back to its numeric wire id. */
export function spellNumberForName(spell: string): number | null {
  return SPELL_ID_BY_NAME.get(spell) ?? null;
}

/** A live effect placed at a tile, used by the render loop. */
export type EffectInstance = {
  key: string;
  animation: EffectAnimation;
  tileX: number;
  tileY: number;
  startedAt: number;
  expiresAt: number;
};

let effectKeySeq = 0;

export function spawnEffectInstance(
  animation: EffectAnimation,
  tileX: number,
  tileY: number,
  now: number,
): EffectInstance {
  effectKeySeq += 1;
  const lifetime = animation.repeat ? Math.max(animation.durationMs, 3000) : animation.durationMs;
  return {
    key: `fx-${effectKeySeq}`,
    animation,
    tileX,
    tileY,
    startedAt: now,
    expiresAt: now + Math.max(lifetime, 1),
  };
}

/** The frame to draw for an instance at the given time (handles looping), or null when done. */
export function effectFrameAt(instance: EffectInstance, now: number): EffectFrameMeta | null {
  const { animation } = instance;
  if (animation.frames.length === 0) {
    return null;
  }
  const elapsed = now - instance.startedAt;
  let frameIndex = Math.floor(elapsed / Math.max(animation.interval, 1));
  if (frameIndex >= animation.frames.length) {
    if (!animation.repeat) {
      return null;
    }
    frameIndex %= animation.frames.length;
  }
  return animation.frames[frameIndex] ?? null;
}
