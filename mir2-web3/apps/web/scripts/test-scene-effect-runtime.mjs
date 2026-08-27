import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTsModule(url, deps = {}) {
  const source = readFileSync(url, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    fileName: fileURLToPath(url),
  });
  const mod = { exports: {} };
  const require = (specifier) => {
    if (specifier in deps) return deps[specifier];
    throw new Error(`Unexpected dependency: ${specifier}`);
  };
  new Function("exports", "module", "require", compiled.outputText)(mod.exports, mod, require);
  return mod.exports;
}

const animation = {
  name: "FireBall",
  kind: "cast",
  frames: [
    { path: "/fx/0.png", width: 16, height: 20, x: -8, y: -15 },
    { path: "/fx/1.png", width: 18, height: 22, x: -9, y: -16 },
  ],
  interval: 100,
  blend: true,
  light: 2,
  repeat: false,
  offset: { x: 1, y: 2 },
  durationMs: 200,
};
const protectionAnimation = {
  ...animation,
  name: "ProtectionField",
  frames: [{ path: "/fx/protection.png", width: 20, height: 24, x: -10, y: -18 }],
  durationMs: 100,
};
const flamingSwordAnimation = (direction) => ({
  ...animation,
  name: "FlamingSword",
  kind: "attackOverlay",
  frames: Array.from({ length: 6 }, (_, frame) => ({
    path: `/fx/${3480 + direction * 10 + frame}.png`,
    width: 64,
    height: 72,
    x: -24,
    y: -52,
  })),
  interval: 100,
  opacity: 0.7,
  light: 0,
  repeat: false,
  offset: { x: 0, y: 0 },
  durationMs: 600,
});
const playerReviveAnimation = {
  ...animation,
  name: "PlayerRevive",
  kind: "target",
  frames: Array.from({ length: 20 }, (_, frame) => ({
    path: `/fx/revive-${1220 + frame}.png`,
    width: 64,
    height: 72,
    x: -24,
    y: -52,
  })),
  interval: 100,
  light: 6,
  durationMs: 2_000,
};
const crystal = {
  // Spell 12 and SpellEffect 12 deliberately have different names. The scene
  // runtime must resolve spell packets through the Spell enum, not Effect.
  effectNameForNumber: (_assets, value) => (value === 12 ? "Mine" : value === 31 ? "FireBall" : null),
  spellNameForNumber: (value) =>
    value === 8
      ? "FlamingSword"
      : value === 12
        ? "ProtectionField"
        : value === 31
          ? "FireBall"
          : null,
  resolveSpellEffect: (_assets, name, direction) => (name === "FireBall" && direction === 3 ? animation : null),
  resolveSpellCastEffect: (_assets, name, direction) => {
    if (direction !== 3) return null;
    if (name === "FireBall") return animation;
    if (name === "ProtectionField") return protectionAnimation;
    return null;
  },
  resolveSpellAttackOverlayEffect: (_assets, name, direction) =>
    name === "FlamingSword" && direction >= 0 && direction < 8
      ? flamingSwordAnimation(direction)
      : null,
  resolveMapEffect: (_assets, name) => {
    if (name === "FireBall") return { ...animation, repeat: true };
    if (name === "PlayerRevive") return playerReviveAnimation;
    return null;
  },
  resolveMapEffectByNumber: () => null,
  effectFrameAt: (instance, now) => {
    let index = Math.floor((now - instance.startedAt) / instance.animation.interval);
    if (instance.animation.repeat) index %= instance.animation.frames.length;
    return instance.animation.frames[index] ?? null;
  },
};
const runtime = loadTsModule(new URL("../lib/scene-effect-runtime.ts", import.meta.url), {
  "./crystal-magic-effects": crystal,
});
const visualLayersSource = readFileSync(
  new URL("../app/components/original-client-scene-visual-layers.tsx", import.meta.url),
  "utf8",
);
const shellSource = readFileSync(new URL("../app/original-client-shell.tsx", import.meta.url), "utf8");
const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const globalCssSource = readFileSync(new URL("../app/globals.css", import.meta.url), "utf8");
const resolvedEffectLayerStart = visualLayersSource.indexOf("displayResolvedEffectFrames.map");
const resolvedEffectLayerSource = visualLayersSource.slice(
  resolvedEffectLayerStart,
  visualLayersSource.indexOf('{screen === "game" && sceneLightClassName', resolvedEffectLayerStart),
);
assert.match(
  resolvedEffectLayerSource,
  /VIEWPORT_ENTITY_LEFT_ORIGIN[\s\S]*VIEWPORT_ENTITY_TOP_ORIGIN/,
  "Crystal effect frames must anchor from the tile top-left DrawLocation",
);
assert.match(
  visualLayersSource,
  /resolveSpellProjectileEffect[\s\S]*resolveSpellImpactEffect[\s\S]*data-projectile-phase/,
  "projectile and impact phases must render from source atlas frames",
);
assert.match(
  pageSource,
  /case "Magic":[\s\S]*enqueueSceneEffect\(\{ \.\.\.payload, objectId: worldRef\.current\.playerObjectId \}, "spell"\)[\s\S]*spawnRangeProjectile/,
  "the local player Magic packet must enter the same cast/projectile renderer as ObjectMagic",
);
assert.match(
  pageSource,
  /function markWorldEntityAttack[\s\S]*applyObjectAttackSceneState\([\s\S]*current\.entities,[\s\S]*current\.effects,[\s\S]*payload,[\s\S]*now,[\s\S]*crystalAttackActionDurationMs/,
  "ObjectAttack must project FlamingSword into the attacker-bound scene-effect store",
);
assert.match(
  pageSource,
  /travelEndsAt \+ \(spellOrEffect === undefined \? 0 : 2_100\)/,
  "magic projectile state must survive long enough to render the longest source impact phase",
);
assert.match(
  pageSource,
  /duplicateMagicProjectile[\s\S]*startedAt - entry\.startedAt <= 500/,
  "Magic, ObjectMagic and ObjectProjectile echoes must collapse to one visual projectile",
);
assert.match(
  pageSource,
  /entry\.spellOrEffect !== undefined &&\s*String\(entry\.spellOrEffect\) === String\(spellOrEffect\)/,
  "projectile deduplication must preserve distinct spells cast at the same target",
);
assert.match(
  pageSource,
  /duplicateTransientSpell[\s\S]*Math\.abs\(entry\.startedAt - startedAt\) <= 500/,
  "the local Magic/ObjectMagic echo must collapse to one caster effect",
);
assert.doesNotMatch(
  resolvedEffectLayerSource,
  /VIEWPORT_TILE_CENTER_[XY]/,
  "effect metadata offsets must not receive a second half-cell center offset",
);
assert.equal(runtime.CRYSTAL_ADDITIVE_MIX_BLEND_MODE, "plus-lighter");
assert.equal(
  runtime.crystalSceneEffectLayerOffset("objectSpell"),
  48,
  "persistent world spells render before the entity layer",
);
assert.equal(
  runtime.crystalSceneEffectLayerOffset("objectSpell", true),
  49,
  "a world-spell mask remains below the entity layer",
);
assert.equal(
  runtime.crystalSceneEffectLayerOffset("spell"),
  90,
  "transient combat spells remain above actors",
);
assert.equal(
  runtime.crystalSceneEffectLayerOffset("attackOverlay"),
  90,
  "attacker-bound overlays remain above the actor layer",
);
assert.equal(
  runtime.crystalSceneEffectLayerOffset("actorEffect"),
  90,
  "actor-bound revive effects remain above the actor layer",
);
assert.deepEqual(
  runtime.sceneEffectAnimationAssetUrls({
    ...animation,
    frames: [
      { ...animation.frames[0], maskPath: "/fx/mask.png" },
      { ...animation.frames[1], maskPath: "/fx/mask.png" },
    ],
  }),
  ["/fx/0.png", "/fx/mask.png", "/fx/1.png"],
  "persistent effects preload every body and mask frame exactly once",
);
assert.match(shellSource, /className={`game-world-composite/, "world renderers share one compositing root");
assert.match(
  globalCssSource,
  /\.game-world-composite\s*\{[^}]*isolation:\s*isolate;/s,
  "the shared world compositor bounds blending away from the HUD",
);
assert.match(
  globalCssSource,
  /\.viewport-sprite-overlay\s*\{[^}]*z-index:\s*auto;/s,
  "the sprite overlay must not isolate additive effects from the map backdrop",
);
assert.match(
  globalCssSource,
  /\.viewport-effect-overlay\s*\{[^}]*z-index:\s*auto;/s,
  "the effect overlay must remain a pass-through world-stacking layer",
);
assert.match(
  resolvedEffectLayerSource,
  /registerCameraSurface\(`effect:\$\{effect\.key\}`\)/,
  "each Crystal effect must follow camera motion without transforming its pass-through parent",
);
assert.match(
  resolvedEffectLayerSource,
  /effect\.objectId[\s\S]*viewportEntitySprites\.find[\s\S]*opacity:\s*animation\.opacity/,
  "attack overlays must follow the live attacker and preserve Crystal DrawBlend rate",
);
assert.match(
  globalCssSource,
  /\.belt-dialog-overlay\s*\{[^}]*opacity:\s*0;/s,
  "the opaque Crystal belt overlay must not darken the transparent item slots",
);
const base = {
  key: "fx-1",
  source: "spell",
  spellOrEffect: 31,
  x: 10,
  y: 20,
  direction: 3,
  value: 0,
  startedAt: 1_000,
  expiresAt: 2_000,
};

assert.equal(runtime.resolveSceneEffectFrame({}, base, 999), null, "delay is authoritative");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 1_000).frame.path, "/fx/0.png");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 1_100).frame.path, "/fx/1.png");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 1_200), null, "non-repeat animation ends");
assert.equal(runtime.resolveSceneEffectFrame({}, base, 2_000), null, "packet lifetime expires");
assert.equal(
  runtime.resolveSceneEffectFrame({}, { ...base, key: "protection", spellOrEffect: 12 }, 1_000).frame.path,
  "/fx/protection.png",
  "overlapping Spell and SpellEffect numbers must resolve through the Spell enum",
);
assert.equal(
  runtime.collectResolvedSceneEffectFrames({}, [base, { ...base, key: "unknown", spellOrEffect: 999 }], 1_000).length,
  1,
);

const reviveEffect = {
  ...base,
  key: "crystal-player-revive:42",
  source: "actorEffect",
  spellOrEffect: "PlayerRevive",
  objectId: "42",
  direction: 0,
  startedAt: 5_000,
  expiresAt: 7_000,
};
assert.equal(
  runtime.resolveSceneEffectFrame({}, reviveEffect, 5_000).frame.path,
  "/fx/revive-1220.png",
);
assert.equal(
  runtime.resolveSceneEffectFrame({}, reviveEffect, 6_999).frame.path,
  "/fx/revive-1239.png",
);
assert.equal(runtime.resolveSceneEffectFrame({}, reviveEffect, 7_000), null);

for (const [direction, directionName] of [
  [0, "Up"],
  [1, "UpRight"],
  [2, "Right"],
  [3, "DownRight"],
  [4, "Down"],
  [5, "DownLeft"],
  [6, "Left"],
  [7, "UpLeft"],
]) {
  const overlay = runtime.createFlamingSwordAttackOverlay(
    {
      objectId: 1000,
      location: { x: 288, y: 616 },
      direction: directionName,
      spell: 8,
    },
    10_000,
  );
  assert.equal(overlay.direction, direction);
  assert.equal(overlay.source, "attackOverlay");
  assert.equal(overlay.expiresAt, 10_600);
  assert.equal(
    runtime.resolveSceneEffectFrame({}, overlay, 10_000).frame.path,
    `/fx/${3480 + direction * 10}.png`,
  );
  assert.equal(
    runtime.resolveSceneEffectFrame({}, overlay, 10_000).animation.opacity,
    0.7,
  );
  assert.equal(
    runtime.resolveSceneEffectFrame({}, overlay, 10_599).frame.path,
    `/fx/${3485 + direction * 10}.png`,
  );
  assert.equal(runtime.resolveSceneEffectFrame({}, overlay, 10_600), null);
}

const firstFlame = runtime.createFlamingSwordAttackOverlay(
  { objectId: 1000, location: { x: 288, y: 616 }, direction: "Up", spell: 8 },
  20_000,
);
const restartedFlame = runtime.createFlamingSwordAttackOverlay(
  { objectId: 1000, location: { x: 289, y: 616 }, direction: "Right", spell: "FlamingSword" },
  20_200,
);
const otherFlame = runtime.createFlamingSwordAttackOverlay(
  { objectId: 1001, direction: "Down", spell: 8 },
  20_210,
  { x: 290, y: 616 },
);
assert.equal(
  runtime.upsertFlamingSwordAttackOverlay([firstFlame], restartedFlame, 20_200).length,
  1,
  "the same attacker restarts one stable overlay",
);
const coexisting = runtime.upsertFlamingSwordAttackOverlay(
  runtime.upsertFlamingSwordAttackOverlay([firstFlame], restartedFlame, 20_200),
  otherFlame,
  20_210,
);
assert.equal(coexisting.length, 2, "different attackers keep independent overlays");
assert.equal(coexisting[0].x, 289);
assert.equal(coexisting[1].x, 290);
const existingEffects = [{ ...base, key: "existing", expiresAt: 40_000 }];
assert.equal(
  runtime.applyObjectAttackSceneEffects(
    existingEffects,
    { objectId: 1000, location: { x: 288, y: 616 }, direction: "Down", spell: 0 },
    30_000,
  ),
  existingEffects,
  "the ObjectAttack reducer preserves ordinary attack state by reference",
);
const reducerFirst = runtime.applyObjectAttackSceneEffects(
  existingEffects,
  { objectId: 1000, location: { x: 288, y: 616 }, direction: "Up", spell: 8 },
  30_000,
);
const reducerRestarted = runtime.applyObjectAttackSceneEffects(
  reducerFirst,
  { objectId: 1000, location: { x: 289, y: 616 }, direction: "Right", spell: "FlamingSword" },
  30_200,
);
const reducerCoexisting = runtime.applyObjectAttackSceneEffects(
  reducerRestarted,
  { objectId: 1001, direction: "Down", spell: 8 },
  30_210,
  { x: 290, y: 616 },
);
assert.equal(reducerRestarted.filter((effect) => effect.source === "attackOverlay").length, 1);
assert.equal(reducerRestarted.at(-1).startedAt, 30_200);
assert.equal(reducerRestarted.at(-1).x, 289);
assert.equal(reducerCoexisting.filter((effect) => effect.source === "attackOverlay").length, 2);
const pageAttackEntity = {
  objectId: "self-1",
  x: 288,
  y: 616,
  direction: "Down",
};
const pageAttackState = runtime.applyObjectAttackSceneState(
  [pageAttackEntity],
  [],
  {
    objectId: "self-1",
    location: { x: 289, y: 616 },
    direction: "Right",
    spell: 8,
    attackType: 0,
  },
  31_000,
  () => 600,
);
assert.deepEqual(
  {
    x: pageAttackState.entities[0].x,
    direction: pageAttackState.entities[0].direction,
    attackAnimation: pageAttackState.entities[0].attackAnimation,
    attackStartedAt: pageAttackState.entities[0].attackStartedAt,
    attackUntil: pageAttackState.entities[0].attackUntil,
  },
  { x: 289, direction: "Right", attackAnimation: "melee1", attackStartedAt: 31_000, attackUntil: 31_600 },
  "the page-consumed reducer atomically applies actor pose and action timing",
);
assert.equal(pageAttackState.effects.length, 1);
assert.equal(pageAttackState.effects[0].source, "attackOverlay");
const pageRestartedState = runtime.applyObjectAttackSceneState(
  pageAttackState.entities,
  pageAttackState.effects,
  { objectId: "self-1", location: { x: 290, y: 616 }, direction: "Up", spell: 8 },
  31_200,
  () => 600,
);
const pageCoexistingState = runtime.applyObjectAttackSceneState(
  [...pageRestartedState.entities, { ...pageAttackEntity, objectId: "other-2", x: 291 }],
  pageRestartedState.effects,
  { objectId: "other-2", direction: "Down", spell: 8 },
  31_210,
  () => 600,
);
assert.equal(pageRestartedState.effects.length, 1, "same page actor restarts one overlay");
assert.equal(pageRestartedState.effects[0].startedAt, 31_200);
assert.equal(pageCoexistingState.effects.length, 2, "two page actors retain independent overlays");
const ordinaryPageState = runtime.applyObjectAttackSceneState(
  pageCoexistingState.entities,
  pageCoexistingState.effects,
  { objectId: "self-1", direction: "Down", spell: 0 },
  31_300,
  () => 600,
);
assert.equal(ordinaryPageState.effects, pageCoexistingState.effects);
assert.equal(runtime.resolveSceneEffectFrame({}, pageRestartedState.effects[0], 31_799).frame.path, "/fx/3485.png");
assert.equal(runtime.resolveSceneEffectFrame({}, pageRestartedState.effects[0], 31_800), null);
assert.equal(
  runtime.createFlamingSwordAttackOverlay(
    { objectId: 1000, location: { x: 288, y: 616 }, direction: "Down", spell: 0 },
    30_000,
  ),
  null,
  "ordinary attacks never create a FlamingSword overlay",
);

const worldSpell = {
  ...base,
  key: "crystal-world-spell:239",
  source: "objectSpell",
  objectId: "239",
  expiresAt: Number.MAX_SAFE_INTEGER,
};
assert.equal(
  runtime.resolveSceneEffectFrame({}, worldSpell, 1_200).frame.path,
  "/fx/0.png",
  "ObjectSpell resolves the repeating ground animation after the cast animation would end",
);

console.log("scene effect runtime: FlamingSword and legacy contracts passed");
