// Procedural (atlas-free) fallback for Crystal magic / map visual effects.
//
// The real renderer (lib/crystal-magic-effects.ts) consumes exported effect atlases and is the
// PREFERRED path. Until those atlases are exported, resolveSpellEffect / resolveMapEffectByNumber
// return null, so casting / skills / map-effects would otherwise render nothing. This module
// fills that gap with a tasteful, data-driven CSS fallback (per-element flash / expanding ring /
// projectile streak / impact burst / area nova / sustained aura ...) keyed by Crystal spell name
// or numeric Spell id, so the world is visibly reactive while the atlases are absent.
//
// It is deliberately framework-agnostic and pure: it derives a list of short-lived, positioned
// effect descriptors from the *existing* scene world snapshot (casting entities + in-flight
// projectiles) and exposes a per-frame style helper. The scene render hook draws them as plain
// inline-styled <div>s, animated on its own rAF clock (we never start a timer of our own). When
// nothing is casting and no projectiles are live, the collectors return empty arrays, so the
// whole feature is a no-cost no-op.

import type { EffectAssets } from "./crystal-magic-effects";
import { resolveMapEffectByNumber, resolveSpellEffect } from "./crystal-magic-effects";

// ---------------------------------------------------------------------------
// Element model + data-driven spell/effect -> element mapping
// ---------------------------------------------------------------------------

export type VfxElement =
  | "fire"
  | "ice"
  | "lightning"
  | "thunder"
  | "holy"
  | "poison"
  | "dark"
  | "blood"
  | "wind"
  | "nature"
  | "arcane";

export type VfxPalette = {
  /** Bright core colour (CSS). */
  core: string;
  /** Outer glow / ring colour (CSS, usually translucent). */
  glow: string;
  /**
   * Preferred CSS compositing mode for this element. Most energy elements read best added to the
   * scene ("screen"); darker/curse elements read better multiplied. The render hook may use this
   * to pick a blend mode; it has a safe default ("screen") so older callers keep working.
   */
  blend: "screen" | "plus-lighter" | "multiply";
};

const ELEMENT_PALETTES: Record<VfxElement, VfxPalette> = {
  fire: { core: "rgba(255, 226, 158, 0.95)", glow: "rgba(255, 96, 24, 0.85)", blend: "screen" },
  ice: { core: "rgba(224, 248, 255, 0.95)", glow: "rgba(86, 178, 255, 0.85)", blend: "screen" },
  lightning: { core: "rgba(245, 240, 255, 0.95)", glow: "rgba(150, 120, 255, 0.9)", blend: "screen" },
  // Thunder = heavier, area electric (mass / storm): cooler white core, electric-blue glow.
  thunder: { core: "rgba(236, 248, 255, 0.95)", glow: "rgba(96, 160, 255, 0.92)", blend: "plus-lighter" },
  holy: { core: "rgba(255, 252, 224, 0.97)", glow: "rgba(255, 214, 120, 0.85)", blend: "screen" },
  poison: { core: "rgba(214, 255, 196, 0.92)", glow: "rgba(96, 200, 64, 0.85)", blend: "screen" },
  dark: { core: "rgba(202, 178, 226, 0.9)", glow: "rgba(80, 36, 116, 0.9)", blend: "multiply" },
  // Blood = vampirism / drain / hemorrhage: crimson with a hot core.
  blood: { core: "rgba(255, 196, 196, 0.95)", glow: "rgba(186, 24, 36, 0.9)", blend: "screen" },
  wind: { core: "rgba(224, 255, 244, 0.95)", glow: "rgba(96, 220, 188, 0.8)", blend: "screen" },
  // Nature = summons / beasts / roots: earthy green.
  nature: { core: "rgba(226, 255, 206, 0.92)", glow: "rgba(120, 188, 72, 0.85)", blend: "screen" },
  arcane: { core: "rgba(224, 232, 255, 0.95)", glow: "rgba(120, 150, 255, 0.85)", blend: "screen" },
};

// Keyword -> element. Matched case-insensitively against normalised spell / effect names so a
// single rule covers Crystal's many spell aliases (e.g. "GreatFireBall", "FireBang", "HellFire").
// Order matters: earlier rules win, so put the more specific element (thunder/storm) before the
// broader one (lightning) where the keyword would otherwise be ambiguous.
const ELEMENT_KEYWORDS: Array<{ element: VfxElement; keywords: string[] }> = [
  {
    element: "fire",
    keywords: ["fire", "flame", "hellfire", "explosion", "blaze", "meteor", "scorch", "napalm", "lava", "burst"],
  },
  {
    element: "ice",
    keywords: ["ice", "frost", "frozen", "freeze", "icethrust", "icestorm", "snow", "blizzard", "crunch"],
  },
  // Thunder/storm = mass electric. Checked before "lightning" so ThunderStorm -> thunder.
  { element: "thunder", keywords: ["thunderstorm", "thunder", "meowmeowthunder"] },
  {
    element: "lightning",
    keywords: ["lightning", "shock", "electric", "spark", "bolt", "shoulderdash", "maplightning"],
  },
  // Blood / life-steal. Checked before generic dark so Vampirism/Hemorrhage read crimson.
  { element: "blood", keywords: ["vampir", "vampire", "hemorrhage", "drain", "bloodlust", "blood"] },
  {
    element: "holy",
    keywords: [
      "heal",
      "holy",
      "light",
      "bless",
      "revive",
      "reincarnation",
      "ultimate",
      "shield",
      "purif",
      "saint",
      "deva",
      "revelation",
      "field",
      "mirror",
      "protection",
      "magicboost",
      "enhancer",
      "concentrat",
      "focus",
      "meditation",
      "immortal",
    ],
  },
  { element: "poison", keywords: ["poison", "venom", "toxic", "plague", "decay", "corpse", "cloud", "zombie"] },
  {
    element: "dark",
    keywords: ["dark", "shadow", "curse", "death", "doom", "nuke", "skeleton", "demon", "undead", "hiding", "hallucinat"],
  },
  {
    element: "nature",
    keywords: ["summon", "shinsu", "toad", "snake", "snakes", "root", "roots", "tree", "rubble", "rock", "stone", "quake", "pile", "nature", "beast", "pet", "golem"],
  },
  {
    element: "wind",
    keywords: ["wind", "storm", "gust", "tornado", "wraith", "phantom", "blink", "dash", "teleport", "escape", "portal", "swift", "haste", "fastmove", "flashdash", "backstep", "repuls"],
  },
];

function normalizeName(value: string): string {
  return value.toLowerCase().replace(/[^a-z0-9]/g, "");
}

/** Maps a spell or map-effect name to an element, defaulting to "arcane" for the unknown. */
export function elementForName(name: string | null | undefined): VfxElement {
  if (!name) {
    return "arcane";
  }
  const normalized = normalizeName(name);
  // Prefer a known numeric-id mapping by name first (covers ids/names we classify deliberately),
  // then fall back to keyword scanning so aliases still resolve.
  const classified = SPELL_NAME_INDEX.get(normalized);
  if (classified) {
    return classified.element;
  }
  for (const rule of ELEMENT_KEYWORDS) {
    if (rule.keywords.some((keyword) => normalized.includes(keyword))) {
      return rule.element;
    }
  }
  return "arcane";
}

/** Stable element for a numeric map/SpellEffect id when no name is known (keeps ids distinct). */
export function elementForNumber(effect: number): VfxElement {
  const byId = SPELL_BY_ID.get(Math.trunc(effect));
  if (byId) {
    return byId.element;
  }
  const order: VfxElement[] = [
    "fire",
    "ice",
    "lightning",
    "thunder",
    "holy",
    "poison",
    "dark",
    "blood",
    "wind",
    "nature",
    "arcane",
  ];
  const index = Math.abs(Math.trunc(effect)) % order.length;
  return order[index] ?? "arcane";
}

export function paletteForElement(element: VfxElement): VfxPalette {
  return ELEMENT_PALETTES[element] ?? ELEMENT_PALETTES.arcane;
}

// ---------------------------------------------------------------------------
// Spell archetype model: maps a spell to *how* its fallback should look/behave
// ---------------------------------------------------------------------------
//
// Crystal magic spans single-target bolts, area blasts, sustained toggles, summons, teleports,
// heals and curses. The archetype captures that shape family so the procedural fallback can pick a
// distinct visual (and shape) per spell. Each archetype maps to one or more concrete FallbackVfx
// "kinds" depending on whether the cast spawned a projectile (target attack) or was an instant /
// self / area cast.

export type VfxArchetype =
  // Single-target projectile (FireBall, SoulFireBall, straight/double/poison shots ...).
  | "bolt"
  // Big area blast centred on the target / ground (FireBang, Blizzard, ThunderStorm, IceStorm ...).
  | "blast"
  // Sustained / toggle aura around the caster (MagicShield, EnergyShield, ProtectionField ...).
  | "aura"
  // Healing / holy glow rising on the target (Healing, MassHealing, HealingCircle ...).
  | "heal"
  // Buff / enhancer ring that snaps on (UltimateEnhancer, MagicBooster, Haste, BlessedArmour ...).
  | "buff"
  // Teleport / blink flash (Teleport, Blink, Portal, FastMove, StormEscape, FlashDash ...).
  | "flash"
  // Summon burst from the ground (SummonSkeleton/Shinsu/HolyDeva/Vampire/Toad/Snakes ...).
  | "summon"
  // Lingering poison / corruption cloud (PoisonCloud, Plague, Poisoning ...).
  | "cloud"
  // Freeze shards / ice spikes (FrostCrunch, IceThrust, IceStorm impact ...).
  | "shards"
  // Lightning chain / arc (Lightning, ThunderBolt, ElectricShock, FlameDisruptor beam ...).
  | "chain"
  // Curse / debuff sink that pulls inward and darkens (Curse, TurnUndead, Hallucination ...).
  | "curse"
  // Generic caster wind-up glow when we know nothing more specific.
  | "cast";

type SpellClass = { element: VfxElement; archetype: VfxArchetype };

// Authoritative numeric ids mirror packages/protocol `Spell` enum (types.rs). Only the spells whose
// fallback we want to bias deliberately are listed; everything else falls through to keyword/id
// heuristics. Names are normalised (lowercased, alnum-only) for matching against incoming strings.
const SPELL_TABLE: Array<{ id: number; name: string } & SpellClass> = [
  // --- Warrior / melee skills that read as elemental bursts -----------------
  { id: 5, name: "ShoulderDash", element: "wind", archetype: "flash" },
  { id: 8, name: "FlamingSword", element: "fire", archetype: "cast" },
  { id: 9, name: "LionRoar", element: "wind", archetype: "blast" },
  { id: 12, name: "ProtectionField", element: "holy", archetype: "aura" },
  { id: 13, name: "Rage", element: "blood", archetype: "buff" },
  { id: 16, name: "Fury", element: "fire", archetype: "buff" },
  { id: 17, name: "ImmortalSkin", element: "holy", archetype: "buff" },
  // --- Wizard ---------------------------------------------------------------
  { id: 31, name: "FireBall", element: "fire", archetype: "bolt" },
  { id: 32, name: "Repulsion", element: "wind", archetype: "blast" },
  { id: 33, name: "ElectricShock", element: "lightning", archetype: "chain" },
  { id: 34, name: "GreatFireBall", element: "fire", archetype: "bolt" },
  { id: 35, name: "HellFire", element: "fire", archetype: "blast" },
  { id: 36, name: "ThunderBolt", element: "lightning", archetype: "chain" },
  { id: 37, name: "Teleport", element: "wind", archetype: "flash" },
  { id: 38, name: "FireBang", element: "fire", archetype: "blast" },
  { id: 39, name: "FireWall", element: "fire", archetype: "cloud" },
  { id: 40, name: "Lightning", element: "lightning", archetype: "chain" },
  { id: 41, name: "FrostCrunch", element: "ice", archetype: "shards" },
  { id: 42, name: "ThunderStorm", element: "thunder", archetype: "blast" },
  { id: 43, name: "MagicShield", element: "holy", archetype: "aura" },
  { id: 44, name: "TurnUndead", element: "holy", archetype: "curse" },
  { id: 45, name: "Vampirism", element: "blood", archetype: "chain" },
  { id: 46, name: "IceStorm", element: "ice", archetype: "blast" },
  { id: 47, name: "FlameDisruptor", element: "fire", archetype: "chain" },
  { id: 48, name: "Mirroring", element: "holy", archetype: "buff" },
  { id: 49, name: "FlameField", element: "fire", archetype: "cloud" },
  { id: 50, name: "Blizzard", element: "ice", archetype: "blast" },
  { id: 51, name: "MagicBooster", element: "holy", archetype: "buff" },
  { id: 52, name: "MeteorStrike", element: "fire", archetype: "blast" },
  { id: 53, name: "IceThrust", element: "ice", archetype: "shards" },
  { id: 54, name: "FastMove", element: "wind", archetype: "flash" },
  { id: 55, name: "StormEscape", element: "wind", archetype: "flash" },
  // --- Taoist ---------------------------------------------------------------
  { id: 61, name: "Healing", element: "holy", archetype: "heal" },
  { id: 62, name: "SpiritSword", element: "holy", archetype: "buff" },
  { id: 63, name: "Poisoning", element: "poison", archetype: "cloud" },
  { id: 64, name: "SoulFireBall", element: "holy", archetype: "bolt" },
  { id: 65, name: "SummonSkeleton", element: "dark", archetype: "summon" },
  { id: 67, name: "Hiding", element: "dark", archetype: "buff" },
  { id: 68, name: "MassHiding", element: "dark", archetype: "blast" },
  { id: 69, name: "SoulShield", element: "holy", archetype: "aura" },
  { id: 70, name: "Revelation", element: "holy", archetype: "buff" },
  { id: 71, name: "BlessedArmour", element: "holy", archetype: "buff" },
  { id: 72, name: "EnergyRepulsor", element: "wind", archetype: "blast" },
  { id: 73, name: "TrapHexagon", element: "dark", archetype: "curse" },
  { id: 74, name: "Purification", element: "holy", archetype: "heal" },
  { id: 75, name: "MassHealing", element: "holy", archetype: "heal" },
  { id: 76, name: "Hallucination", element: "dark", archetype: "curse" },
  { id: 77, name: "UltimateEnhancer", element: "holy", archetype: "buff" },
  { id: 78, name: "SummonShinsu", element: "nature", archetype: "summon" },
  { id: 79, name: "Reincarnation", element: "holy", archetype: "heal" },
  { id: 80, name: "SummonHolyDeva", element: "holy", archetype: "summon" },
  { id: 81, name: "Curse", element: "dark", archetype: "curse" },
  { id: 82, name: "Plague", element: "poison", archetype: "cloud" },
  { id: 83, name: "PoisonCloud", element: "poison", archetype: "cloud" },
  { id: 84, name: "EnergyShield", element: "holy", archetype: "aura" },
  { id: 85, name: "PetEnhancer", element: "nature", archetype: "buff" },
  { id: 86, name: "HealingCircle", element: "holy", archetype: "heal" },
  // --- Assassin -------------------------------------------------------------
  { id: 91, name: "FatalSword", element: "dark", archetype: "buff" },
  { id: 93, name: "Haste", element: "wind", archetype: "buff" },
  { id: 94, name: "FlashDash", element: "wind", archetype: "flash" },
  { id: 95, name: "LightBody", element: "wind", archetype: "buff" },
  { id: 96, name: "HeavenlySword", element: "holy", archetype: "blast" },
  { id: 97, name: "FireBurst", element: "fire", archetype: "blast" },
  { id: 99, name: "PoisonSword", element: "poison", archetype: "buff" },
  { id: 100, name: "MoonLight", element: "dark", archetype: "buff" },
  { id: 101, name: "MPEater", element: "blood", archetype: "chain" },
  { id: 102, name: "SwiftFeet", element: "wind", archetype: "buff" },
  { id: 103, name: "DarkBody", element: "dark", archetype: "buff" },
  { id: 104, name: "Hemorrhage", element: "blood", archetype: "curse" },
  { id: 106, name: "MoonMist", element: "dark", archetype: "cloud" },
  { id: 107, name: "CatTongue", element: "poison", archetype: "curse" },
  // --- Archer ---------------------------------------------------------------
  { id: 121, name: "Focus", element: "holy", archetype: "buff" },
  { id: 122, name: "StraightShot", element: "wind", archetype: "bolt" },
  { id: 123, name: "DoubleShot", element: "wind", archetype: "bolt" },
  { id: 124, name: "ExplosiveTrap", element: "fire", archetype: "blast" },
  { id: 125, name: "DelayedExplosion", element: "fire", archetype: "blast" },
  { id: 126, name: "Meditation", element: "holy", archetype: "buff" },
  { id: 127, name: "BackStep", element: "wind", archetype: "flash" },
  { id: 128, name: "ElementalShot", element: "arcane", archetype: "bolt" },
  { id: 129, name: "Concentration", element: "holy", archetype: "buff" },
  { id: 130, name: "Stonetrap", element: "nature", archetype: "curse" },
  { id: 131, name: "ElementalBarrier", element: "arcane", archetype: "aura" },
  { id: 132, name: "SummonVampire", element: "blood", archetype: "summon" },
  { id: 133, name: "VampireShot", element: "blood", archetype: "bolt" },
  { id: 134, name: "SummonToad", element: "nature", archetype: "summon" },
  { id: 135, name: "PoisonShot", element: "poison", archetype: "bolt" },
  { id: 136, name: "CrippleShot", element: "dark", archetype: "curse" },
  { id: 137, name: "SummonSnakes", element: "nature", archetype: "summon" },
  { id: 138, name: "NapalmShot", element: "fire", archetype: "blast" },
  { id: 139, name: "OneWithNature", element: "nature", archetype: "buff" },
  { id: 140, name: "BindingShot", element: "nature", archetype: "curse" },
  { id: 141, name: "MentalState", element: "arcane", archetype: "buff" },
  // --- Misc / mobility ------------------------------------------------------
  { id: 151, name: "Blink", element: "wind", archetype: "flash" },
  { id: 152, name: "Portal", element: "wind", archetype: "flash" },
  { id: 153, name: "BattleCry", element: "dark", archetype: "curse" },
  { id: 154, name: "FireBounce", element: "fire", archetype: "bolt" },
  { id: 155, name: "MeteorShower", element: "fire", archetype: "blast" },
  // --- Monster / map effects ------------------------------------------------
  { id: 200, name: "DigOutZombie", element: "poison", archetype: "summon" },
  { id: 201, name: "Rubble", element: "nature", archetype: "blast" },
  { id: 202, name: "MapLightning", element: "lightning", archetype: "chain" },
  { id: 203, name: "MapLava", element: "fire", archetype: "cloud" },
  { id: 204, name: "MapQuake1", element: "nature", archetype: "blast" },
  { id: 205, name: "MapQuake2", element: "nature", archetype: "blast" },
  { id: 206, name: "DigOutArmadillo", element: "nature", archetype: "summon" },
  { id: 207, name: "GeneralMeowMeowThunder", element: "thunder", archetype: "blast" },
  { id: 208, name: "StoneGolemQuake", element: "nature", archetype: "blast" },
  { id: 209, name: "EarthGolemPile", element: "nature", archetype: "blast" },
  { id: 210, name: "TreeQueenRoot", element: "nature", archetype: "cloud" },
  { id: 211, name: "TreeQueenMassRoots", element: "nature", archetype: "blast" },
  { id: 212, name: "TreeQueenGroundRoots", element: "nature", archetype: "cloud" },
  { id: 213, name: "TucsonGeneralRock", element: "nature", archetype: "blast" },
  { id: 214, name: "FlyingStatueIceTornado", element: "ice", archetype: "blast" },
  { id: 215, name: "DarkOmaKingNuke", element: "dark", archetype: "blast" },
  { id: 216, name: "HornedSorcererDustTornado", element: "wind", archetype: "blast" },
  { id: 217, name: "HornedCommanderRockFall", element: "nature", archetype: "blast" },
  { id: 218, name: "HornedCommanderRockSpike", element: "nature", archetype: "shards" },
];

const SPELL_BY_ID = new Map<number, SpellClass>();
const SPELL_NAME_INDEX = new Map<string, SpellClass>();
for (const entry of SPELL_TABLE) {
  const cls: SpellClass = { element: entry.element, archetype: entry.archetype };
  SPELL_BY_ID.set(entry.id, cls);
  SPELL_NAME_INDEX.set(normalizeName(entry.name), cls);
}

// Keyword -> archetype, scanned when a spell name is not in the explicit table. Mirrors the element
// keyword list but biases the *shape* of the effect. First match wins.
const ARCHETYPE_KEYWORDS: Array<{ archetype: VfxArchetype; keywords: string[] }> = [
  { archetype: "heal", keywords: ["heal", "purif", "reincarn", "revive", "circle"] },
  { archetype: "flash", keywords: ["teleport", "blink", "portal", "dash", "escape", "fastmove", "backstep", "swift"] },
  { archetype: "summon", keywords: ["summon", "shinsu", "deva", "skeleton", "toad", "snake", "vampire", "digout", "armadillo"] },
  { archetype: "cloud", keywords: ["cloud", "poison", "plague", "venom", "firewall", "flamefield", "lava", "mist", "root"] },
  { archetype: "shards", keywords: ["frost", "icethrust", "crunch", "spike", "shard"] },
  { archetype: "chain", keywords: ["lightning", "thunderbolt", "electric", "shock", "bolt", "disruptor", "vampir", "drain", "eater"] },
  { archetype: "curse", keywords: ["curse", "turnundead", "hallucinat", "hexagon", "binding", "cripple", "hemorrhage", "battlecry", "stonetrap", "cattongue"] },
  {
    archetype: "buff",
    keywords: ["enhancer", "boost", "haste", "bless", "armour", "armor", "focus", "concentrat", "meditation", "rage", "fury", "mirror", "body", "feet", "spirit", "sword", "immortal", "nature", "pet", "mental"],
  },
  { archetype: "aura", keywords: ["shield", "field", "protection", "barrier"] },
  {
    archetype: "blast",
    keywords: ["blast", "bang", "storm", "blizzard", "meteor", "explos", "hellfire", "flame", "burst", "quake", "nuke", "roar", "repuls", "tornado", "shower", "rubble", "rockfall", "pile", "masshiding", "napalm"],
  },
  { archetype: "bolt", keywords: ["ball", "shot", "bounce", "fire"] },
];

/** Maps a spell name to its effect archetype, defaulting to a generic caster glow. */
export function archetypeForName(name: string | null | undefined): VfxArchetype {
  if (!name) {
    return "cast";
  }
  const normalized = normalizeName(name);
  const classified = SPELL_NAME_INDEX.get(normalized);
  if (classified) {
    return classified.archetype;
  }
  for (const rule of ARCHETYPE_KEYWORDS) {
    if (rule.keywords.some((keyword) => normalized.includes(keyword))) {
      return rule.archetype;
    }
  }
  return "cast";
}

/** Archetype for a numeric Spell/SpellEffect id, or "cast" when unknown. */
export function archetypeForNumber(effect: number): VfxArchetype {
  return SPELL_BY_ID.get(Math.trunc(effect))?.archetype ?? "cast";
}

/** Element + archetype for a spell name (single lookup, used by collectors). */
export function classifySpellName(name: string | null | undefined): SpellClass {
  return { element: elementForName(name), archetype: archetypeForName(name) };
}

// Whether an archetype is fundamentally an *area* effect (drawn flat on the ground, wider) versus a
// point/target effect. Used to bias shape + size in the collectors and render style.
const AREA_ARCHETYPES: ReadonlySet<VfxArchetype> = new Set<VfxArchetype>([
  "blast",
  "cloud",
  "aura",
  "summon",
  "heal",
  "buff",
]);

export function isAreaArchetype(archetype: VfxArchetype): boolean {
  return AREA_ARCHETYPES.has(archetype);
}

// ---------------------------------------------------------------------------
// Fallback effect descriptors
// ---------------------------------------------------------------------------

// Concrete render "kinds". The scene render hook special-cases "streak" (a rotated trail bar) and
// "aura" (a flat ring); every other kind is drawn as a centred radial burst whose size/opacity/scale
// come entirely from fallbackVfxStyle(). New kinds therefore render safely on the existing hook —
// their distinct look comes from the per-kind easing below plus the element palette.
export type FallbackVfxKind =
  | "cast"
  | "streak"
  | "impact"
  | "aura"
  | "nova"
  | "flash"
  | "cloud"
  | "summon"
  | "heal"
  | "curse"
  | "chain";

/**
 * A single procedural effect placed in tile-delta space (relative to the render player), so the
 * render hook can position it with the same VIEWPORT_CELL_* maths as entities / projectiles and it
 * tracks the camera. `streak`/`chain` effects span from (dx,dy) to (toDx,toDy); the rest are point
 * effects.
 */
export type FallbackVfx = {
  key: string;
  kind: FallbackVfxKind;
  element: VfxElement;
  /** The classified archetype (for callers that want richer shaping); never required by the hook. */
  archetype?: VfxArchetype;
  /** Tile-delta of the effect origin relative to the render player. */
  dx: number;
  dy: number;
  /** Tile-delta of the effect end (streaks/chains only; equals origin otherwise). */
  toDx: number;
  toDy: number;
  /** Absolute world tile of the effect (used only for depth sorting). */
  worldX: number;
  worldY: number;
  startedAt: number;
  durationMs: number;
};

export type FallbackVfxStyle = {
  /** 0..1 normalised progress through the effect lifetime. */
  progress: number;
  /** Current opacity, already eased. */
  opacity: number;
  /** Current scale multiplier for rings / bursts. */
  scale: number;
  palette: VfxPalette;
};

function clamp01(value: number): number {
  return value < 0 ? 0 : value > 1 ? 1 : value;
}

// Cheap easing helpers (no allocations, no state). Used to give each archetype a distinct feel.
function easeOutCubic(t: number): number {
  const inv = 1 - t;
  return 1 - inv * inv * inv;
}
function easeOutQuint(t: number): number {
  const inv = 1 - t;
  return 1 - inv * inv * inv * inv * inv;
}
function easeInCubic(t: number): number {
  return t * t * t;
}

/** Per-effect render parameters at `now`, or null once the effect has expired. */
export function fallbackVfxStyle(effect: FallbackVfx, now: number): FallbackVfxStyle | null {
  const elapsed = now - effect.startedAt;
  if (elapsed < 0 || elapsed >= effect.durationMs) {
    return null;
  }
  const progress = clamp01(elapsed / Math.max(effect.durationMs, 1));
  const palette = paletteForElement(effect.element);
  switch (effect.kind) {
    case "cast": {
      // Two-stage caster wind-up: a quick swell in, then a slow fade — reads as "gathering power".
      const grow = progress < 0.35 ? easeOutCubic(progress / 0.35) : 1;
      const opacity = progress < 0.35 ? grow : 1 - (progress - 0.35) / 0.65;
      const scale = 0.55 + grow * 0.45 - Math.max(0, progress - 0.35) * 0.2;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "aura": {
      // Expanding ground ring that fades as it grows (sustained shields / barriers / fields).
      const opacity = (1 - progress) * 0.9;
      const scale = 0.4 + easeOutCubic(progress) * 1.5;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "impact": {
      // Sharp burst: pops in fast, fades out. Single-target hit.
      const opacity = progress < 0.22 ? progress / 0.22 : 1 - (progress - 0.22) / 0.78;
      const scale = 0.5 + easeOutCubic(progress) * 1.15;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "nova": {
      // Area shockwave: snaps wide almost instantly then thins out — mass / blast spells.
      const opacity = progress < 0.12 ? progress / 0.12 : (1 - (progress - 0.12) / 0.88) * 0.95;
      const scale = 0.35 + easeOutQuint(progress) * 2.2;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "flash": {
      // Teleport / blink: instant white pop that collapses. Brightest at the very start.
      const opacity = progress < 0.18 ? 1 : Math.max(0, 1 - (progress - 0.18) / 0.82);
      const scale = progress < 0.18 ? 0.7 + progress * 2.0 : 1.06 - easeInCubic((progress - 0.18) / 0.82) * 0.7;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "cloud": {
      // Lingering cloud: rises slowly, holds, then dissipates. Soft, never fully opaque.
      const rise = easeOutCubic(clamp01(progress / 0.4));
      const fade = progress > 0.6 ? 1 - (progress - 0.6) / 0.4 : 1;
      const opacity = rise * fade * 0.7;
      const scale = 0.7 + progress * 0.8;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "summon": {
      // Ground eruption: a fast upward burst that settles — summons / digouts / quakes.
      const opacity = progress < 0.3 ? easeOutCubic(progress / 0.3) : 1 - (progress - 0.3) / 0.7;
      const scale = 0.5 + easeOutCubic(clamp01(progress / 0.45)) * 1.3;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "heal": {
      // Holy glow: gentle two-pulse swell that lingers warm — heals / purification / revives.
      const pulse = 0.5 + 0.5 * Math.sin(progress * Math.PI * 2);
      const envelope = progress < 0.25 ? progress / 0.25 : 1 - (progress - 0.25) / 0.75;
      const opacity = clamp01(envelope) * (0.6 + 0.4 * pulse);
      const scale = 0.6 + easeOutCubic(progress) * 0.7;
      return { progress, opacity: clamp01(opacity), scale, palette };
    }
    case "curse": {
      // Debuff sink: starts wide and dark, contracts inward onto the target.
      const opacity = progress < 0.2 ? progress / 0.2 : 1 - (progress - 0.2) / 0.8;
      const scale = 1.5 - easeOutCubic(progress) * 1.0;
      return { progress, opacity: clamp01(opacity * 0.95), scale, palette };
    }
    case "chain":
    case "streak":
    default: {
      // Trail / arc that stays mostly opaque then fades near the end (projectile + lightning chain).
      const opacity = progress > 0.7 ? 1 - (progress - 0.7) / 0.3 : 1;
      return { progress, opacity: clamp01(opacity), scale: 1, palette };
    }
  }
}

// ---------------------------------------------------------------------------
// World -> fallback effect collectors
// ---------------------------------------------------------------------------

// The render hook already receives the scene world snapshot; we only read it (no mutation, no
// packet wiring). A "range" attack animation is how a magic cast is represented on an entity, and
// projectiles carry the spell trajectory. We synthesise procedural effects from those signals.

const CAST_FALLBACK_DURATION_MS = 520;
const IMPACT_FALLBACK_DURATION_MS = 440;
const AURA_FALLBACK_DURATION_MS = 1100;
const CLOUD_FALLBACK_DURATION_MS = 1500;
const HEAL_FALLBACK_DURATION_MS = 900;
const FLASH_FALLBACK_DURATION_MS = 360;
const SUMMON_FALLBACK_DURATION_MS = 700;
const NOVA_FALLBACK_DURATION_MS = 620;
const CURSE_FALLBACK_DURATION_MS = 760;

// Concrete render kind + lifetime for a cast/instant of the given archetype. Projectile attacks are
// handled separately (their trail + impact is driven off the projectile lifetime).
function instantKindForArchetype(archetype: VfxArchetype): { kind: FallbackVfxKind; durationMs: number } {
  switch (archetype) {
    case "blast":
      return { kind: "nova", durationMs: NOVA_FALLBACK_DURATION_MS };
    case "aura":
      return { kind: "aura", durationMs: AURA_FALLBACK_DURATION_MS };
    case "cloud":
      return { kind: "cloud", durationMs: CLOUD_FALLBACK_DURATION_MS };
    case "heal":
      return { kind: "heal", durationMs: HEAL_FALLBACK_DURATION_MS };
    case "buff":
      // Buffs snap a bright ring onto the caster: a short aura.
      return { kind: "aura", durationMs: HEAL_FALLBACK_DURATION_MS };
    case "flash":
      return { kind: "flash", durationMs: FLASH_FALLBACK_DURATION_MS };
    case "summon":
      return { kind: "summon", durationMs: SUMMON_FALLBACK_DURATION_MS };
    case "curse":
      return { kind: "curse", durationMs: CURSE_FALLBACK_DURATION_MS };
    case "shards":
      // Ice spikes read as a sharp single-target impact.
      return { kind: "impact", durationMs: IMPACT_FALLBACK_DURATION_MS };
    case "chain":
      return { kind: "impact", durationMs: IMPACT_FALLBACK_DURATION_MS };
    case "bolt":
    case "cast":
    default:
      return { kind: "cast", durationMs: CAST_FALLBACK_DURATION_MS };
  }
}

// Impact kind for a projectile that *landed*, by archetype (single-target vs area splash, etc.).
function impactKindForArchetype(archetype: VfxArchetype): { kind: FallbackVfxKind; durationMs: number } {
  switch (archetype) {
    case "blast":
      return { kind: "nova", durationMs: NOVA_FALLBACK_DURATION_MS };
    case "cloud":
      return { kind: "cloud", durationMs: CLOUD_FALLBACK_DURATION_MS };
    case "curse":
      return { kind: "curse", durationMs: CURSE_FALLBACK_DURATION_MS };
    case "summon":
      return { kind: "summon", durationMs: SUMMON_FALLBACK_DURATION_MS };
    default:
      return { kind: "impact", durationMs: IMPACT_FALLBACK_DURATION_MS };
  }
}

export type CollectFallbackOptions = {
  /** Render player tile, used to convert world tiles into tile-delta space. */
  origin: { x: number; y: number };
  now: number;
  /**
   * Optional atlas asset set. When the real atlas resolves a map effect, the procedural fallback is
   * suppressed for it so the atlas path stays authoritative.
   */
  assets?: EffectAssets | null;
};

export type MapEffectSpawn = {
  /** SpellEffect numeric id from the MapEffect / ObjectEffect packet. */
  effect: number;
  x: number;
  y: number;
  startedAt: number;
  name?: string | null;
};

/**
 * Map-effect fallbacks: a procedural ground effect per active map effect, shaped by the effect's
 * archetype (area nova / lingering cloud / summon eruption / ...). Map effects do not yet have a
 * place in the scene world snapshot (the MapEffect packet only logs today), so callers pass any
 * collected spawns explicitly; this resolves their element/archetype/lifetime. The atlas path wins
 * when it can resolve the numeric effect. Returns [] for an empty / undefined list.
 */
export function collectMapEffectFallbacks(
  spawns: readonly MapEffectSpawn[] | null | undefined,
  options: CollectFallbackOptions,
): FallbackVfx[] {
  if (!spawns || spawns.length === 0) {
    return [];
  }
  const { origin, now, assets } = options;
  const out: FallbackVfx[] = [];
  for (const spawn of spawns) {
    if (assets && resolveMapEffectByNumber(assets, spawn.effect)) {
      continue; // atlas path is authoritative for this effect
    }
    const element = spawn.name ? elementForName(spawn.name) : elementForNumber(spawn.effect);
    const archetype = spawn.name ? archetypeForName(spawn.name) : archetypeForNumber(spawn.effect);
    // Map effects are inherently ground/area; default any non-area archetype to a nova shockwave.
    const shaped = isAreaArchetype(archetype)
      ? instantKindForArchetype(archetype)
      : { kind: "nova" as FallbackVfxKind, durationMs: NOVA_FALLBACK_DURATION_MS };
    if (now < spawn.startedAt || now >= spawn.startedAt + shaped.durationMs) {
      continue;
    }
    out.push({
      key: `vfx-map-${spawn.effect}-${spawn.x}-${spawn.y}-${spawn.startedAt}`,
      kind: shaped.kind,
      element,
      archetype,
      dx: spawn.x - origin.x,
      dy: spawn.y - origin.y,
      toDx: spawn.x - origin.x,
      toDy: spawn.y - origin.y,
      worldX: spawn.x,
      worldY: spawn.y,
      startedAt: spawn.startedAt,
      durationMs: shaped.durationMs,
    });
  }
  return out;
}

// ---------------------------------------------------------------------------
// Viewport-delta collectors
// ---------------------------------------------------------------------------
//
// The scene render hook already holds entities and projectiles in *tile-delta* space relative to
// the render player (entity.dx/dy, projectile.fromDx/toDx ...). These adapters consume that exact
// data so procedural effects share the same coordinate basis as the sprites they accompany — no
// origin reconstruction, no drift. They are the entry points used by the visual-layers component.

export type ViewportCasterLike = {
  objectId: string;
  x: number;
  y: number;
  dx: number;
  dy: number;
  attackAnimation?: string;
  attackStartedAt?: number;
};

export type ViewportProjectileLike = {
  key: string;
  fromDx: number;
  fromDy: number;
  toDx: number;
  toDy: number;
  toX: number;
  toY: number;
  startedAt: number;
  expiresAt: number;
};

export type CollectViewportFallbackOptions = {
  now: number;
  assets?: EffectAssets | null;
  /** Spell name per caster objectId (cast flashes) and per projectile key (streak/impact). */
  spellByCaster?: Map<string, string> | null;
};

/** Cast flashes from viewport entity sprites (tile-delta space), shaped by spell archetype. */
export function collectViewportCastFallbacks(
  entities: readonly ViewportCasterLike[],
  options: CollectViewportFallbackOptions,
): FallbackVfx[] {
  const { now, assets, spellByCaster } = options;
  const out: FallbackVfx[] = [];
  for (const entity of entities) {
    if (entity.attackAnimation !== "range" || typeof entity.attackStartedAt !== "number") {
      continue;
    }
    const spell = spellByCaster?.get(entity.objectId) ?? null;
    // Atlas path wins: if a real spell-cast animation resolves, skip the procedural fallback.
    if (spell && assets && resolveSpellEffect(assets, spell)) {
      continue;
    }
    const element = elementForName(spell);
    const archetype = archetypeForName(spell);
    const shaped = instantKindForArchetype(archetype);
    const elapsed = now - entity.attackStartedAt;
    if (elapsed < 0 || elapsed >= shaped.durationMs) {
      continue;
    }
    out.push({
      key: `vfx-cast-${entity.objectId}-${entity.attackStartedAt}`,
      kind: shaped.kind,
      element,
      archetype,
      dx: entity.dx,
      dy: entity.dy,
      toDx: entity.dx,
      toDy: entity.dy,
      worldX: entity.x,
      worldY: entity.y,
      startedAt: entity.attackStartedAt,
      durationMs: shaped.durationMs,
    });
  }
  return out;
}

/** Streaks + impact bursts from viewport projectiles (tile-delta space), shaped by spell archetype. */
export function collectViewportProjectileFallbacks(
  projectiles: readonly ViewportProjectileLike[],
  options: CollectViewportFallbackOptions,
): FallbackVfx[] {
  const { now, assets, spellByCaster } = options;
  const out: FallbackVfx[] = [];
  for (const projectile of projectiles) {
    if (now < projectile.startedAt || now >= projectile.expiresAt) {
      continue;
    }
    const spell = spellByCaster?.get(projectile.key) ?? null;
    // Atlas path wins for the projectile/impact animation too.
    if (spell && assets && resolveSpellEffect(assets, spell)) {
      continue;
    }
    const element = elementForName(spell);
    const archetype = archetypeForName(spell);
    const lifeMs = Math.max(projectile.expiresAt - projectile.startedAt, 1);
    // A "streak" draws the origin->target line in the render hook; the element palette gives each
    // projectile its identity (electric arc for lightning, fiery trail for fire, etc.). The
    // distinct landing burst comes from impactKindForArchetype below.
    out.push({
      key: `vfx-streak-${projectile.key}`,
      kind: "streak",
      element,
      archetype,
      dx: projectile.fromDx,
      dy: projectile.fromDy,
      toDx: projectile.toDx,
      toDy: projectile.toDy,
      worldX: projectile.toX,
      worldY: projectile.toY,
      startedAt: projectile.startedAt,
      durationMs: lifeMs,
    });
    const impact = impactKindForArchetype(archetype);
    const impactStart = projectile.startedAt + lifeMs * 0.66;
    if (now >= impactStart && now < impactStart + impact.durationMs) {
      out.push({
        key: `vfx-impact-${projectile.key}`,
        kind: impact.kind,
        element,
        archetype,
        dx: projectile.toDx,
        dy: projectile.toDy,
        toDx: projectile.toDx,
        toDy: projectile.toDy,
        worldX: projectile.toX,
        worldY: projectile.toY,
        startedAt: impactStart,
        durationMs: impact.durationMs,
      });
    }
  }
  return out;
}

/**
 * Aggregator over viewport-delta data used directly by the scene render hook. Returns [] on idle
 * frames so the feature costs nothing when nothing is casting and no projectiles are live.
 */
export function collectViewportFallbackVfx(
  input: {
    entities: readonly ViewportCasterLike[];
    projectiles: readonly ViewportProjectileLike[];
  },
  options: CollectViewportFallbackOptions,
): FallbackVfx[] {
  const casts = collectViewportCastFallbacks(input.entities, options);
  const streaks = collectViewportProjectileFallbacks(input.projectiles, options);
  if (casts.length === 0 && streaks.length === 0) {
    return [];
  }
  return [...streaks, ...casts];
}
