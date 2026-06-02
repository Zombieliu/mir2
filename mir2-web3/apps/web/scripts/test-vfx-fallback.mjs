// Behavioral tests for lib/vfx-fallback.ts.
//
// The procedural (atlas-free) magic/map VFX fallback. We cover:
//   1. Classification: known spell names + numeric ids map to the expected
//      element + archetype (table-driven and keyword heuristics).
//   2. Style bounds: fallbackVfxStyle stays finite and within range
//      (opacity 0..1, scale finite > 0) across an entire effect lifetime, for
//      every render kind; idle/expired frames return null.
//   3. Collectors: idle frames return [], casts/projectiles synthesise the
//      expected descriptors, and the atlas path suppresses the fallback when a
//      real animation resolves.
//
// vfx-fallback imports { resolveSpellEffect, resolveMapEffectByNumber } from
// ./crystal-magic-effects at runtime. We stub that module via the loader's
// requireMap so the atlas-suppression branch is exercised deterministically
// without depending on the real atlas internals.
//
// Pure logic only. Run with plain `node`; the .ts source is transpiled in-memory
// via the `typescript` devDependency.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTypeScriptModule(url, requireMap = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.CommonJS,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: fileURLToPath(url),
  });
  const module = { exports: {} };
  const require = (specifier) => {
    if (specifier in requireMap) return requireMap[specifier];
    throw new Error(`Unexpected require(${specifier}) while loading ${url}`);
  };
  const load = new Function("exports", "module", "require", compiled.outputText);
  load(module.exports, module, require);
  return module.exports;
}

// --- Stub for ./crystal-magic-effects -------------------------------------
// The fallback only checks truthiness of these resolvers. We make a stub that
// "resolves" (returns a truthy animation) for an allow-list of spell names /
// numeric effects, and returns null otherwise — so we can drive both branches.
const ATLAS_SPELLS = new Set(); // spell names the fake atlas can render
const ATLAS_EFFECTS = new Set(); // numeric map effects the fake atlas can render
const crystalMagicEffectsStub = {
  resolveSpellEffect: (_assets, spell) => (ATLAS_SPELLS.has(spell) ? { frames: ["x"] } : null),
  resolveMapEffectByNumber: (_assets, effect) =>
    ATLAS_EFFECTS.has(effect) ? { frames: ["x"] } : null,
};
// A sentinel assets object (its shape is irrelevant — only passed through to the stub).
const FAKE_ASSETS = { spellByName: new Map() };

const vfx = loadTypeScriptModule(new URL("../lib/vfx-fallback.ts", import.meta.url), {
  "./crystal-magic-effects": crystalMagicEffectsStub,
});

const {
  elementForName,
  elementForNumber,
  paletteForElement,
  archetypeForName,
  archetypeForNumber,
  classifySpellName,
  isAreaArchetype,
  fallbackVfxStyle,
  collectMapEffectFallbacks,
  collectViewportCastFallbacks,
  collectViewportProjectileFallbacks,
  collectViewportFallbackVfx,
} = vfx;

let passed = 0;
function check(label, fn) {
  fn();
  passed += 1;
}

const ELEMENTS = new Set([
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
]);
const ARCHETYPES = new Set([
  "bolt",
  "blast",
  "aura",
  "heal",
  "buff",
  "flash",
  "summon",
  "cloud",
  "shards",
  "chain",
  "curse",
  "cast",
]);
const KINDS = [
  "cast",
  "streak",
  "impact",
  "aura",
  "nova",
  "flash",
  "cloud",
  "summon",
  "heal",
  "curse",
  "chain",
];

// ---------------------------------------------------------------------------
// Classification: element by name (table-driven exact ids)
// ---------------------------------------------------------------------------

check("elementForName resolves known table spells exactly", () => {
  assert.equal(elementForName("FireBall"), "fire");
  assert.equal(elementForName("IceStorm"), "ice");
  assert.equal(elementForName("Lightning"), "lightning");
  assert.equal(elementForName("ThunderStorm"), "thunder", "thunder beats lightning for storm");
  assert.equal(elementForName("Healing"), "holy");
  assert.equal(elementForName("PoisonCloud"), "poison");
  assert.equal(elementForName("Curse"), "dark");
  assert.equal(elementForName("Vampirism"), "blood");
  assert.equal(elementForName("Teleport"), "wind");
  assert.equal(elementForName("SummonShinsu"), "nature");
});

check("elementForName is case/punctuation insensitive (normalized)", () => {
  assert.equal(elementForName("fire ball"), "fire");
  assert.equal(elementForName("FIRE_BALL"), "fire");
  assert.equal(elementForName("Fire-Ball"), "fire");
});

check("elementForName falls back to keyword heuristics for aliases", () => {
  // Not in SPELL_TABLE by exact name, but keyword "fire"/"flame" hits.
  assert.equal(elementForName("GreatHellBlaze"), "fire");
  assert.equal(elementForName("FrozenTomb"), "ice");
  assert.equal(elementForName("DrainLife"), "blood", "drain -> blood before generic dark");
});

check("elementForName defaults to arcane for unknown/empty", () => {
  assert.equal(elementForName("Xyzzy"), "arcane");
  assert.equal(elementForName(""), "arcane");
  assert.equal(elementForName(null), "arcane");
  assert.equal(elementForName(undefined), "arcane");
});

// ---------------------------------------------------------------------------
// Classification: element by numeric id
// ---------------------------------------------------------------------------

check("elementForNumber resolves known Spell ids from the table", () => {
  assert.equal(elementForNumber(31), "fire", "31 = FireBall");
  assert.equal(elementForNumber(40), "lightning", "40 = Lightning");
  assert.equal(elementForNumber(42), "thunder", "42 = ThunderStorm");
  assert.equal(elementForNumber(61), "holy", "61 = Healing");
  assert.equal(elementForNumber(83), "poison", "83 = PoisonCloud");
});

check("elementForNumber returns a stable in-set element for unknown ids", () => {
  for (const id of [-5, 0, 999, 1000, 7777]) {
    const el = elementForNumber(id);
    assert.ok(ELEMENTS.has(el), `unknown id ${id} -> valid element (${el})`);
  }
  // Deterministic: same id -> same element.
  assert.equal(elementForNumber(999), elementForNumber(999));
});

// ---------------------------------------------------------------------------
// Classification: archetype by name + number
// ---------------------------------------------------------------------------

check("archetypeForName resolves known table spells exactly", () => {
  assert.equal(archetypeForName("FireBall"), "bolt");
  assert.equal(archetypeForName("FireBang"), "blast");
  assert.equal(archetypeForName("MagicShield"), "aura");
  assert.equal(archetypeForName("Healing"), "heal");
  assert.equal(archetypeForName("Haste"), "buff");
  assert.equal(archetypeForName("Teleport"), "flash");
  assert.equal(archetypeForName("SummonSkeleton"), "summon");
  assert.equal(archetypeForName("PoisonCloud"), "cloud");
  assert.equal(archetypeForName("FrostCrunch"), "shards");
  assert.equal(archetypeForName("Lightning"), "chain");
  assert.equal(archetypeForName("Curse"), "curse");
});

check("archetypeForName falls back to keyword heuristics + cast default", () => {
  assert.equal(archetypeForName("MegaHealSurge"), "heal");
  assert.equal(archetypeForName("ShadowBlinkStep"), "flash");
  assert.equal(archetypeForName("Xyzzy"), "cast");
  assert.equal(archetypeForName(null), "cast");
  assert.equal(archetypeForName(undefined), "cast");
});

check("archetypeForNumber resolves table ids, else cast", () => {
  assert.equal(archetypeForNumber(31), "bolt");
  assert.equal(archetypeForNumber(38), "blast", "38 = FireBang");
  assert.equal(archetypeForNumber(61), "heal");
  assert.equal(archetypeForNumber(99999), "cast", "unknown id -> cast");
});

check("classifySpellName combines element + archetype", () => {
  assert.deepEqual(classifySpellName("FireBall"), { element: "fire", archetype: "bolt" });
  assert.deepEqual(classifySpellName("Healing"), { element: "holy", archetype: "heal" });
  assert.deepEqual(classifySpellName("Unknownish"), { element: "arcane", archetype: "cast" });
});

check("every table spell classifies to valid element + archetype", () => {
  // Spot-check a broad set of names spanning all classes/ids.
  const names = [
    "ShoulderDash", "FlamingSword", "ProtectionField", "Rage", "FireBall", "ElectricShock",
    "ThunderBolt", "FireWall", "FrostCrunch", "MagicShield", "TurnUndead", "Vampirism",
    "FlameDisruptor", "Mirroring", "Blizzard", "MeteorStrike", "IceThrust", "FastMove",
    "Healing", "Poisoning", "SoulFireBall", "SummonSkeleton", "Hiding", "Revelation",
    "Purification", "Hallucination", "UltimateEnhancer", "SummonShinsu", "Curse", "Plague",
    "EnergyShield", "HealingCircle", "FatalSword", "Haste", "FlashDash", "HeavenlySword",
    "FireBurst", "MPEater", "Hemorrhage", "CatTongue", "Focus", "StraightShot", "DoubleShot",
    "ExplosiveTrap", "BackStep", "ElementalShot", "Stonetrap", "ElementalBarrier",
    "SummonVampire", "VampireShot", "SummonToad", "PoisonShot", "CrippleShot", "SummonSnakes",
    "NapalmShot", "OneWithNature", "BindingShot", "Blink", "Portal", "BattleCry", "FireBounce",
    "MeteorShower", "DigOutZombie", "Rubble", "MapLightning", "MapLava", "GeneralMeowMeowThunder",
    "TreeQueenRoot", "FlyingStatueIceTornado", "DarkOmaKingNuke", "HornedCommanderRockSpike",
  ];
  for (const name of names) {
    const { element, archetype } = classifySpellName(name);
    assert.ok(ELEMENTS.has(element), `${name} element ${element}`);
    assert.ok(ARCHETYPES.has(archetype), `${name} archetype ${archetype}`);
  }
});

// ---------------------------------------------------------------------------
// paletteForElement / isAreaArchetype
// ---------------------------------------------------------------------------

check("paletteForElement returns a complete palette for every element", () => {
  for (const el of ELEMENTS) {
    const p = paletteForElement(el);
    assert.equal(typeof p.core, "string");
    assert.equal(typeof p.glow, "string");
    assert.ok(["screen", "plus-lighter", "multiply"].includes(p.blend), `blend for ${el}`);
  }
});

check("isAreaArchetype classifies area vs point effects", () => {
  for (const a of ["blast", "cloud", "aura", "summon", "heal", "buff"]) {
    assert.equal(isAreaArchetype(a), true, `${a} is area`);
  }
  for (const a of ["bolt", "flash", "shards", "chain", "curse", "cast"]) {
    assert.equal(isAreaArchetype(a), false, `${a} is point`);
  }
});

// ---------------------------------------------------------------------------
// fallbackVfxStyle: bounds + lifecycle for every kind
// ---------------------------------------------------------------------------

check("fallbackVfxStyle returns null before start and after expiry", () => {
  const effect = {
    key: "k",
    kind: "impact",
    element: "fire",
    dx: 0,
    dy: 0,
    toDx: 0,
    toDy: 0,
    worldX: 0,
    worldY: 0,
    startedAt: 1000,
    durationMs: 500,
  };
  assert.equal(fallbackVfxStyle(effect, 999), null, "before start -> null");
  assert.equal(fallbackVfxStyle(effect, 1500), null, "at/after end -> null");
  assert.equal(fallbackVfxStyle(effect, 2000), null, "well past end -> null");
  assert.ok(fallbackVfxStyle(effect, 1000), "exactly at start -> live");
  assert.ok(fallbackVfxStyle(effect, 1499), "just before end -> live");
});

check("fallbackVfxStyle stays finite + in-range across the whole lifetime (all kinds)", () => {
  const duration = 1000;
  const start = 5000;
  for (const kind of KINDS) {
    const effect = {
      key: `k-${kind}`,
      kind,
      element: "arcane",
      dx: 0,
      dy: 0,
      toDx: 1,
      toDy: 1,
      worldX: 0,
      worldY: 0,
      startedAt: start,
      durationMs: duration,
    };
    // Sample densely across the lifetime including the boundaries.
    for (let step = 0; step <= 100; step += 1) {
      const now = start + (duration * step) / 100;
      const style = fallbackVfxStyle(effect, now);
      if (now >= start + duration) {
        assert.equal(style, null, `${kind} expired frame is null`);
        continue;
      }
      assert.ok(style, `${kind} live at step ${step}`);
      // progress in [0,1]
      assert.ok(
        Number.isFinite(style.progress) && style.progress >= 0 && style.progress <= 1,
        `${kind} progress in range (${style.progress})`,
      );
      // opacity clamped to [0,1]
      assert.ok(
        Number.isFinite(style.opacity) && style.opacity >= 0 && style.opacity <= 1,
        `${kind} opacity in range at step ${step} (${style.opacity})`,
      );
      // scale finite and positive (never negative / NaN — would invert/hide the sprite)
      assert.ok(
        Number.isFinite(style.scale) && style.scale > 0,
        `${kind} scale finite > 0 at step ${step} (${style.scale})`,
      );
      // palette is attached
      assert.ok(style.palette && typeof style.palette.core === "string", `${kind} palette`);
    }
  }
});

check("fallbackVfxStyle guards a zero-duration effect (no divide-by-zero)", () => {
  const effect = {
    key: "z",
    kind: "nova",
    element: "fire",
    dx: 0,
    dy: 0,
    toDx: 0,
    toDy: 0,
    worldX: 0,
    worldY: 0,
    startedAt: 0,
    durationMs: 0,
  };
  // elapsed (0) >= durationMs (0) -> expired -> null, and no NaN leaks.
  assert.equal(fallbackVfxStyle(effect, 0), null);
});

// ---------------------------------------------------------------------------
// collectMapEffectFallbacks
// ---------------------------------------------------------------------------

const ORIGIN = { x: 100, y: 100 };

check("collectMapEffectFallbacks returns [] for empty/undefined spawns", () => {
  assert.deepEqual(collectMapEffectFallbacks(null, { origin: ORIGIN, now: 0 }), []);
  assert.deepEqual(collectMapEffectFallbacks(undefined, { origin: ORIGIN, now: 0 }), []);
  assert.deepEqual(collectMapEffectFallbacks([], { origin: ORIGIN, now: 0 }), []);
});

check("collectMapEffectFallbacks synthesises a ground effect in tile-delta space", () => {
  const out = collectMapEffectFallbacks(
    [{ effect: 202, x: 105, y: 97, startedAt: 1000, name: "MapLightning" }],
    { origin: ORIGIN, now: 1100 },
  );
  assert.equal(out.length, 1);
  const e = out[0];
  assert.equal(e.element, "lightning");
  // MapLightning archetype is "chain" (non-area) -> shaped to a nova for map ground effects.
  assert.equal(e.kind, "nova");
  assert.equal(e.dx, 5, "x - origin.x");
  assert.equal(e.dy, -3, "y - origin.y");
  assert.equal(e.worldX, 105);
  assert.equal(e.worldY, 97);
  assert.equal(e.startedAt, 1000);
  assert.ok(e.key.includes("202"));
});

check("collectMapEffectFallbacks keeps area archetypes' own shape", () => {
  // Rubble (id 201) is archetype "blast" -> area -> instantKindForArchetype -> nova.
  const rubble = collectMapEffectFallbacks(
    [{ effect: 201, x: 100, y: 100, startedAt: 0, name: "Rubble" }],
    { origin: ORIGIN, now: 10 },
  );
  assert.equal(rubble[0].kind, "nova");
  assert.equal(rubble[0].element, "nature");
  // TreeQueenRoot (id 210) is "cloud" -> area -> stays a cloud.
  const root = collectMapEffectFallbacks(
    [{ effect: 210, x: 100, y: 100, startedAt: 0, name: "TreeQueenRoot" }],
    { origin: ORIGIN, now: 10 },
  );
  assert.equal(root[0].kind, "cloud");
});

check("collectMapEffectFallbacks classifies by number when name is absent", () => {
  const out = collectMapEffectFallbacks(
    [{ effect: 31, x: 100, y: 100, startedAt: 0 }], // 31 = FireBall, no name
    { origin: ORIGIN, now: 10 },
  );
  assert.equal(out.length, 1);
  assert.equal(out[0].element, "fire");
});

check("collectMapEffectFallbacks excludes effects outside their lifetime", () => {
  const spawns = [{ effect: 201, x: 100, y: 100, startedAt: 1000, name: "Rubble" }];
  // now before start
  assert.deepEqual(collectMapEffectFallbacks(spawns, { origin: ORIGIN, now: 500 }), []);
  // now after start + duration (nova duration ~620ms)
  assert.deepEqual(collectMapEffectFallbacks(spawns, { origin: ORIGIN, now: 1000 + 5000 }), []);
});

check("collectMapEffectFallbacks: atlas path suppresses the fallback", () => {
  const spawns = [{ effect: 555, x: 100, y: 100, startedAt: 0, name: "AtlasOnly" }];
  // Without assets the fallback is produced.
  assert.equal(collectMapEffectFallbacks(spawns, { origin: ORIGIN, now: 10 }).length, 1);
  // With assets where the atlas resolves effect 555, the fallback is suppressed.
  ATLAS_EFFECTS.add(555);
  try {
    assert.deepEqual(
      collectMapEffectFallbacks(spawns, { origin: ORIGIN, now: 10, assets: FAKE_ASSETS }),
      [],
      "atlas-resolved map effect suppresses procedural fallback",
    );
    // A different, unresolved effect still falls back even when assets are present.
    const other = [{ effect: 556, x: 100, y: 100, startedAt: 0, name: "NotInAtlas" }];
    assert.equal(
      collectMapEffectFallbacks(other, { origin: ORIGIN, now: 10, assets: FAKE_ASSETS }).length,
      1,
    );
  } finally {
    ATLAS_EFFECTS.delete(555);
  }
});

// ---------------------------------------------------------------------------
// collectViewportCastFallbacks
// ---------------------------------------------------------------------------

const caster = (overrides = {}) => ({
  objectId: "mob-1",
  x: 50,
  y: 60,
  dx: 2,
  dy: -1,
  attackAnimation: "range",
  attackStartedAt: 1000,
  ...overrides,
});

check("collectViewportCastFallbacks ignores non-range / missing-start entities", () => {
  assert.deepEqual(
    collectViewportCastFallbacks([caster({ attackAnimation: "melee" })], { now: 1100 }),
    [],
  );
  assert.deepEqual(
    collectViewportCastFallbacks([caster({ attackStartedAt: undefined })], { now: 1100 }),
    [],
  );
  assert.deepEqual(collectViewportCastFallbacks([], { now: 1100 }), []);
});

check("collectViewportCastFallbacks emits a shaped cast in delta space", () => {
  const spellByCaster = new Map([["mob-1", "FireBang"]]); // blast -> nova
  const out = collectViewportCastFallbacks([caster()], { now: 1100, spellByCaster });
  assert.equal(out.length, 1);
  const e = out[0];
  assert.equal(e.kind, "nova", "FireBang blast -> nova cast shape");
  assert.equal(e.element, "fire");
  assert.equal(e.dx, 2);
  assert.equal(e.dy, -1);
  assert.equal(e.startedAt, 1000);
  assert.ok(e.key.includes("mob-1"));
});

check("collectViewportCastFallbacks excludes casts past their shaped duration", () => {
  const spellByCaster = new Map([["mob-1", "FireBall"]]); // bolt -> cast (520ms)
  // now within window
  assert.equal(
    collectViewportCastFallbacks([caster()], { now: 1100, spellByCaster }).length,
    1,
  );
  // now after start + 520ms
  assert.equal(
    collectViewportCastFallbacks([caster()], { now: 1000 + 600, spellByCaster }).length,
    0,
  );
  // now before start
  assert.equal(
    collectViewportCastFallbacks([caster()], { now: 500, spellByCaster }).length,
    0,
  );
});

check("collectViewportCastFallbacks: atlas path suppresses the cast fallback", () => {
  const spellByCaster = new Map([["mob-1", "AtlasSpell"]]);
  ATLAS_SPELLS.add("AtlasSpell");
  try {
    assert.deepEqual(
      collectViewportCastFallbacks([caster()], {
        now: 1100,
        spellByCaster,
        assets: FAKE_ASSETS,
      }),
      [],
      "atlas-resolved spell suppresses the procedural cast",
    );
  } finally {
    ATLAS_SPELLS.delete("AtlasSpell");
  }
});

// ---------------------------------------------------------------------------
// collectViewportProjectileFallbacks
// ---------------------------------------------------------------------------

const projectile = (overrides = {}) => ({
  key: "proj-1",
  fromDx: 0,
  fromDy: 0,
  toDx: 3,
  toDy: 4,
  toX: 53,
  toY: 64,
  startedAt: 1000,
  expiresAt: 1300,
  ...overrides,
});

check("collectViewportProjectileFallbacks emits a streak in delta space", () => {
  const spellByCaster = new Map([["proj-1", "FireBall"]]);
  // Early in flight (before the impact begins at startedAt + life*0.66 = 1198).
  const out = collectViewportProjectileFallbacks([projectile()], {
    now: 1050,
    spellByCaster,
  });
  assert.equal(out.length, 1, "just the streak while in flight");
  const streak = out[0];
  assert.equal(streak.kind, "streak");
  assert.equal(streak.element, "fire");
  assert.equal(streak.dx, 0);
  assert.equal(streak.dy, 0);
  assert.equal(streak.toDx, 3);
  assert.equal(streak.toDy, 4);
  assert.equal(streak.startedAt, 1000);
  assert.equal(streak.durationMs, 300, "lifetime = expiresAt - startedAt");
});

check("collectViewportProjectileFallbacks adds an impact burst near landing", () => {
  const spellByCaster = new Map([["proj-1", "FireBall"]]); // bolt -> impact
  // impactStart = 1000 + 300*0.66 = 1198. Sample after that.
  const out = collectViewportProjectileFallbacks([projectile()], {
    now: 1250,
    spellByCaster,
  });
  const kinds = out.map((e) => e.kind).sort();
  assert.deepEqual(kinds, ["impact", "streak"], "streak + landing impact while both live");
  const impact = out.find((e) => e.kind === "impact");
  assert.equal(impact.dx, 3, "impact sits at the projectile target");
  assert.equal(impact.dy, 4);
  assert.equal(impact.worldX, 53);
  assert.equal(impact.worldY, 64);
});

check("collectViewportProjectileFallbacks impact archetype follows the spell", () => {
  // A blast projectile lands as a nova, not a point impact.
  const spellByCaster = new Map([["proj-1", "MeteorShower"]]); // blast
  const out = collectViewportProjectileFallbacks([projectile()], {
    now: 1250,
    spellByCaster,
  });
  const impact = out.find((e) => e.kind !== "streak");
  assert.ok(impact, "an impact-phase effect exists");
  assert.equal(impact.kind, "nova", "blast projectile -> nova impact");
});

check("collectViewportProjectileFallbacks excludes projectiles outside life window", () => {
  assert.deepEqual(
    collectViewportProjectileFallbacks([projectile()], { now: 999 }),
    [],
    "before startedAt",
  );
  assert.deepEqual(
    collectViewportProjectileFallbacks([projectile()], { now: 1300 }),
    [],
    "at expiresAt",
  );
});

check("collectViewportProjectileFallbacks: atlas path suppresses streak + impact", () => {
  const spellByCaster = new Map([["proj-1", "AtlasBolt"]]);
  ATLAS_SPELLS.add("AtlasBolt");
  try {
    assert.deepEqual(
      collectViewportProjectileFallbacks([projectile()], {
        now: 1250,
        spellByCaster,
        assets: FAKE_ASSETS,
      }),
      [],
      "atlas-resolved projectile suppresses the procedural streak/impact",
    );
  } finally {
    ATLAS_SPELLS.delete("AtlasBolt");
  }
});

// ---------------------------------------------------------------------------
// collectViewportFallbackVfx (aggregator)
// ---------------------------------------------------------------------------

check("collectViewportFallbackVfx returns [] on idle frames", () => {
  // No entities, no projectiles.
  assert.deepEqual(
    collectViewportFallbackVfx({ entities: [], projectiles: [] }, { now: 1000 }),
    [],
  );
  // Entities present but none casting + projectiles present but expired => still [].
  assert.deepEqual(
    collectViewportFallbackVfx(
      {
        entities: [caster({ attackAnimation: "idle" })],
        projectiles: [projectile({ expiresAt: 1000 })],
      },
      { now: 5000 },
    ),
    [],
    "non-casting entities + dead projectiles => no effects",
  );
});

check("collectViewportFallbackVfx merges streaks then casts when active", () => {
  const spellByCaster = new Map([
    ["mob-1", "FireBall"],
    ["proj-1", "FireBall"],
  ]);
  const out = collectViewportFallbackVfx(
    { entities: [caster()], projectiles: [projectile()] },
    { now: 1050, spellByCaster },
  );
  assert.ok(out.length >= 2, "both a cast and a streak are present");
  // Order contract: streaks first, then casts.
  assert.equal(out[0].kind, "streak", "streaks are emitted before casts");
  assert.ok(out.some((e) => e.kind === "nova" || e.kind === "cast"), "a cast effect follows");
});

console.log(`vfx fallback tests passed (${passed} groups)`);
