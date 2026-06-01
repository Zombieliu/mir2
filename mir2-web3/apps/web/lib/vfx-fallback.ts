// Procedural (atlas-free) fallback for Crystal magic / map visual effects.
//
// The real renderer (lib/crystal-magic-effects.ts) consumes exported effect atlases and is the
// PREFERRED path. Until those atlases are exported, resolveSpellEffect / resolveMapEffectByNumber
// return null, so casting / skills / map-effects would otherwise render nothing. This module
// fills that gap with a tasteful, data-driven CSS fallback (per-element flash / expanding ring /
// projectile streak / impact burst) keyed by spell name or element, so the world is visibly
// reactive while the atlases are absent.
//
// It is deliberately framework-agnostic and pure: it derives a list of short-lived, positioned
// effect descriptors from the *existing* scene world snapshot (casting entities + in-flight
// projectiles) and exposes a per-frame style helper. The scene render hook draws them as plain
// inline-styled <div>s. When nothing is casting and no projectiles are live, the collectors
// return empty arrays, so the whole feature is a no-cost no-op.

import type { EffectAssets } from "./crystal-magic-effects";
import { resolveMapEffectByNumber, resolveSpellEffect } from "./crystal-magic-effects";

// ---------------------------------------------------------------------------
// Element model + data-driven spell/effect -> element mapping
// ---------------------------------------------------------------------------

export type VfxElement =
  | "fire"
  | "ice"
  | "lightning"
  | "holy"
  | "poison"
  | "dark"
  | "wind"
  | "arcane";

export type VfxPalette = {
  /** Bright core colour (CSS). */
  core: string;
  /** Outer glow / ring colour (CSS, usually translucent). */
  glow: string;
};

const ELEMENT_PALETTES: Record<VfxElement, VfxPalette> = {
  fire: { core: "rgba(255, 226, 158, 0.95)", glow: "rgba(255, 96, 24, 0.85)" },
  ice: { core: "rgba(224, 248, 255, 0.95)", glow: "rgba(86, 178, 255, 0.85)" },
  lightning: { core: "rgba(245, 240, 255, 0.95)", glow: "rgba(150, 120, 255, 0.9)" },
  holy: { core: "rgba(255, 252, 224, 0.95)", glow: "rgba(255, 214, 120, 0.85)" },
  poison: { core: "rgba(214, 255, 196, 0.95)", glow: "rgba(96, 200, 64, 0.85)" },
  dark: { core: "rgba(214, 196, 232, 0.9)", glow: "rgba(96, 48, 128, 0.85)" },
  wind: { core: "rgba(224, 255, 244, 0.95)", glow: "rgba(96, 220, 188, 0.8)" },
  arcane: { core: "rgba(224, 232, 255, 0.95)", glow: "rgba(120, 150, 255, 0.85)" },
};

// Keyword -> element. Matched case-insensitively against normalised spell / effect names so a
// single rule covers Crystal's many spell aliases (e.g. "GreatFireBall", "FireBang", "HellFire").
const ELEMENT_KEYWORDS: Array<{ element: VfxElement; keywords: string[] }> = [
  { element: "fire", keywords: ["fire", "flame", "hellfire", "explosion", "blaze", "meteor", "scorch"] },
  { element: "ice", keywords: ["ice", "frost", "frozen", "freeze", "icethrust", "icestorm", "snow"] },
  {
    element: "lightning",
    keywords: ["lightning", "thunder", "shock", "electric", "spark", "bolt", "shoulderdash"],
  },
  {
    element: "holy",
    keywords: ["heal", "holy", "light", "bless", "revive", "ultimate", "shield", "purify", "soulfire", "saint"],
  },
  { element: "poison", keywords: ["poison", "venom", "toxic", "plague", "decay", "corpse"] },
  { element: "dark", keywords: ["dark", "shadow", "curse", "death", "doom", "summon", "skeleton", "demon", "drain"] },
  { element: "wind", keywords: ["wind", "storm", "gust", "tornado", "wraith", "phantom", "blink", "dash", "teleport"] },
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
  for (const rule of ELEMENT_KEYWORDS) {
    if (rule.keywords.some((keyword) => normalized.includes(keyword))) {
      return rule.element;
    }
  }
  return "arcane";
}

/** Stable element for a numeric map/SpellEffect id when no name is known (keeps ids distinct). */
export function elementForNumber(effect: number): VfxElement {
  const order: VfxElement[] = ["fire", "ice", "lightning", "holy", "poison", "dark", "wind", "arcane"];
  const index = Math.abs(Math.trunc(effect)) % order.length;
  return order[index] ?? "arcane";
}

export function paletteForElement(element: VfxElement): VfxPalette {
  return ELEMENT_PALETTES[element] ?? ELEMENT_PALETTES.arcane;
}

// ---------------------------------------------------------------------------
// Fallback effect descriptors
// ---------------------------------------------------------------------------

export type FallbackVfxKind = "cast" | "streak" | "impact" | "aura";

/**
 * A single procedural effect placed in tile-delta space (relative to the render player), so the
 * render hook can position it with the same VIEWPORT_CELL_* maths as entities / projectiles and it
 * tracks the camera. `streak` effects span from (dx,dy) to (toDx,toDy); the rest are point effects.
 */
export type FallbackVfx = {
  key: string;
  kind: FallbackVfxKind;
  element: VfxElement;
  /** Tile-delta of the effect origin relative to the render player. */
  dx: number;
  dy: number;
  /** Tile-delta of the effect end (streaks only; equals origin otherwise). */
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

/** Per-effect render parameters at `now`, or null once the effect has expired. */
export function fallbackVfxStyle(effect: FallbackVfx, now: number): FallbackVfxStyle | null {
  const elapsed = now - effect.startedAt;
  if (elapsed < 0 || elapsed >= effect.durationMs) {
    return null;
  }
  const progress = elapsed / Math.max(effect.durationMs, 1);
  const palette = paletteForElement(effect.element);
  switch (effect.kind) {
    case "cast": {
      // Quick flash that fades, with a gentle pulse outward.
      const opacity = 1 - progress;
      const scale = 0.6 + progress * 0.5;
      return { progress, opacity, scale, palette };
    }
    case "aura": {
      // Expanding ground ring that fades as it grows.
      const opacity = (1 - progress) * 0.9;
      const scale = 0.4 + progress * 1.4;
      return { progress, opacity, scale, palette };
    }
    case "impact": {
      // Sharp burst: pops in fast, fades out.
      const opacity = progress < 0.25 ? progress / 0.25 : 1 - (progress - 0.25) / 0.75;
      const scale = 0.5 + progress * 1.1;
      return { progress, opacity: Math.max(0, opacity), scale, palette };
    }
    case "streak":
    default: {
      // Trail that stays mostly opaque then fades near the end.
      const opacity = progress > 0.7 ? 1 - (progress - 0.7) / 0.3 : 1;
      const scale = 1;
      return { progress, opacity: Math.max(0, opacity), scale, palette };
    }
  }
}

// ---------------------------------------------------------------------------
// World -> fallback effect collectors
// ---------------------------------------------------------------------------

// The render hook already receives the scene world snapshot; we only read it (no mutation, no
// packet wiring). A "range" attack animation is how a magic cast is represented on an entity, and
// projectiles carry the spell trajectory. We synthesise procedural effects from those signals.

const CAST_FALLBACK_DURATION_MS = 480;
const IMPACT_FALLBACK_DURATION_MS = 420;
const AURA_FALLBACK_DURATION_MS = 900;

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
 * Map-effect fallbacks: an expanding ground aura per active map effect. Map effects do not yet have
 * a place in the scene world snapshot (the MapEffect packet only logs today), so callers pass any
 * collected spawns explicitly; this resolves their element/lifetime. The atlas path wins when it
 * can resolve the numeric effect. Returns [] for an empty / undefined list.
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
    if (now < spawn.startedAt || now >= spawn.startedAt + AURA_FALLBACK_DURATION_MS) {
      continue;
    }
    if (assets && resolveMapEffectByNumber(assets, spawn.effect)) {
      continue; // atlas path is authoritative for this effect
    }
    const element = spawn.name ? elementForName(spawn.name) : elementForNumber(spawn.effect);
    out.push({
      key: `vfx-map-${spawn.effect}-${spawn.x}-${spawn.y}-${spawn.startedAt}`,
      kind: "aura",
      element,
      dx: spawn.x - origin.x,
      dy: spawn.y - origin.y,
      toDx: spawn.x - origin.x,
      toDy: spawn.y - origin.y,
      worldX: spawn.x,
      worldY: spawn.y,
      startedAt: spawn.startedAt,
      durationMs: AURA_FALLBACK_DURATION_MS,
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
  spellByCaster?: Map<string, string> | null;
};

/** Cast flashes from viewport entity sprites (tile-delta space). */
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
    const elapsed = now - entity.attackStartedAt;
    if (elapsed < 0 || elapsed >= CAST_FALLBACK_DURATION_MS) {
      continue;
    }
    const spell = spellByCaster?.get(entity.objectId) ?? null;
    if (spell && assets && resolveSpellEffect(assets, spell)) {
      continue;
    }
    out.push({
      key: `vfx-cast-${entity.objectId}-${entity.attackStartedAt}`,
      kind: "cast",
      element: elementForName(spell),
      dx: entity.dx,
      dy: entity.dy,
      toDx: entity.dx,
      toDy: entity.dy,
      worldX: entity.x,
      worldY: entity.y,
      startedAt: entity.attackStartedAt,
      durationMs: CAST_FALLBACK_DURATION_MS,
    });
  }
  return out;
}

/** Streaks + impact bursts from viewport projectiles (tile-delta space). */
export function collectViewportProjectileFallbacks(
  projectiles: readonly ViewportProjectileLike[],
  options: CollectViewportFallbackOptions,
): FallbackVfx[] {
  const { now } = options;
  const out: FallbackVfx[] = [];
  for (const projectile of projectiles) {
    if (now < projectile.startedAt || now >= projectile.expiresAt) {
      continue;
    }
    const element = elementForName(options.spellByCaster?.get(projectile.key) ?? null);
    const lifeMs = Math.max(projectile.expiresAt - projectile.startedAt, 1);
    out.push({
      key: `vfx-streak-${projectile.key}`,
      kind: "streak",
      element,
      dx: projectile.fromDx,
      dy: projectile.fromDy,
      toDx: projectile.toDx,
      toDy: projectile.toDy,
      worldX: projectile.toX,
      worldY: projectile.toY,
      startedAt: projectile.startedAt,
      durationMs: lifeMs,
    });
    const impactStart = projectile.startedAt + lifeMs * 0.66;
    if (now >= impactStart && now < impactStart + IMPACT_FALLBACK_DURATION_MS) {
      out.push({
        key: `vfx-impact-${projectile.key}`,
        kind: "impact",
        element,
        dx: projectile.toDx,
        dy: projectile.toDy,
        toDx: projectile.toDx,
        toDy: projectile.toDy,
        worldX: projectile.toX,
        worldY: projectile.toY,
        startedAt: impactStart,
        durationMs: IMPACT_FALLBACK_DURATION_MS,
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
