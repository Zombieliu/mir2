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
};

export type EffectSubSpec = {
  library: string;
  base: number;
  count: number;
  interval?: number;
};

export type EffectSpec = {
  spell?: string;
  effect?: string;
  kind?: "cast" | "projectile" | "impact" | "target";
  library: string;
  base: number;
  count: number;
  interval: number;
  light?: number;
  blend?: boolean;
  repeat?: boolean;
  offset?: { x: number; y: number };
  impact?: EffectSubSpec;
  returnEffect?: EffectSubSpec;
};

export type EffectAnimation = {
  name: string;
  kind: string;
  frames: EffectFrameMeta[];
  interval: number;
  blend: boolean;
  light: number;
  repeat: boolean;
  offset: { x: number; y: number };
  impact?: EffectAnimation;
  durationMs: number;
};

type LibraryMeta = { frames: Record<string, EffectFrameMeta> };

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
    spell_effect_enum?: string[];
    spell_effects?: EffectSpec[];
    ground_effects?: EffectSpec[];
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
  for (const entry of manifest.map_effects ?? []) {
    if (entry.effect) mapByName.set(entry.effect, entry);
  }
  const effectNameByNumber = new Map<number, string>();
  (manifest.spell_effect_enum ?? []).forEach((name, index) => {
    effectNameByNumber.set(index, name);
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
  return frames;
}

function resolveSub(assets: EffectAssets, sub: EffectSubSpec): EffectAnimation | undefined {
  const frames = resolveFrames(assets, sub.library, sub.base, sub.count);
  if (frames.length === 0) {
    return undefined;
  }
  const interval = sub.interval ?? 100;
  return {
    name: "impact",
    kind: "impact",
    frames,
    interval,
    blend: true,
    light: 0,
    repeat: false,
    offset: { x: 0, y: 0 },
    durationMs: interval * frames.length,
  };
}

/** Resolves a spec entry to a playable animation, or null when its frames are unavailable. */
export function resolveAnimation(
  assets: EffectAssets,
  entry: EffectSpec,
): EffectAnimation | null {
  const frames = resolveFrames(assets, entry.library, entry.base, entry.count);
  if (frames.length === 0) {
    return null;
  }
  return {
    name: entry.spell ?? entry.effect ?? "effect",
    kind: entry.kind ?? "impact",
    frames,
    interval: entry.interval,
    blend: entry.blend ?? true,
    light: entry.light ?? 0,
    repeat: entry.repeat ?? false,
    offset: entry.offset ?? { x: 0, y: 0 },
    impact: entry.impact ? resolveSub(assets, entry.impact) : undefined,
    durationMs: entry.interval * frames.length,
  };
}

export function resolveSpellEffect(
  assets: EffectAssets,
  spell: string,
): EffectAnimation | null {
  const entry = assets.spellByName.get(spell);
  return entry ? resolveAnimation(assets, entry) : null;
}

export function resolveMapEffect(
  assets: EffectAssets,
  effect: string,
): EffectAnimation | null {
  const entry = assets.mapByName.get(effect) ?? assets.groundBySpell.get(effect);
  return entry ? resolveAnimation(assets, entry) : null;
}

/**
 * Resolves a `MapEffect` / `ObjectEffect` packet's numeric `effect` (a raw SpellEffect byte) to a
 * playable animation, by mapping the number to its enum name and then resolving by name. Returns
 * null for unknown numbers, names with no map-effect entry, or unavailable frames.
 */
export function resolveMapEffectByNumber(
  assets: EffectAssets,
  effect: number,
): EffectAnimation | null {
  const name = assets.effectNameByNumber.get(effect);
  return name ? resolveMapEffect(assets, name) : null;
}

/** The SpellEffect enum name for a numeric effect id, or null when unknown. */
export function effectNameForNumber(assets: EffectAssets, effect: number): string | null {
  return assets.effectNameByNumber.get(effect) ?? null;
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
