import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import ts from "typescript";

async function loadTypeScript(url) {
  const source = await readFile(new URL(url, import.meta.url), "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
  });
  const module = { exports: {} };
  new Function("exports", "module", compiled.outputText)(module.exports, module);
  return module.exports;
}

const { FishingFeedbackPresentation, readFishingFeedback } = await loadTypeScript("../lib/world-fishing-feedback.ts");
const { loadEffectAssets, resolveMapEffect } = await loadTypeScript("../lib/crystal-magic-effects.ts");
const worldKey = "session1:0";
const actor = { objectId: "7", kind: "selfPlayer", x: 342, y: 244 };
const ripple = { x: 339, y: 247, foundFish: false };
const bite = { ...ripple, foundFish: true };

function poses(action, fishingStartedAtMs, logicalFrameIndex, incarnation = 1, objectId = actor.objectId) {
  return { [objectId]: { objectId, action, incarnation, fishingStartedAtMs, logicalFrameIndex } };
}

test("fishing presentation only reads an explicit server tile and bite flag", () => {
  assert.deepEqual(readFishingFeedback({ fishingPoint: { x: 0, y: 247 }, foundFish: true }), { ...bite, x: 0 });
  for (const invalid of [
    { fishingPoint: { x: "339", y: 247 }, foundFish: true },
    { fishingPoint: { x: 339, y: 247.5 }, foundFish: true },
    { fishingPoint: { x: -1, y: 247 }, foundFish: true },
    { fishingPoint: { x: 339, y: 247 }, foundFish: "true" },
    { foundFish: false },
  ]) assert.equal(readFishingFeedback(invalid), undefined);
});

test("the actual shared pose boundary owns sounds and samples one feedback pair per wait cycle", () => {
  const presentation = new FishingFeedbackPresentation();
  const advance = (now, action, started, index, facts = ripple) =>
    presentation.advance(worldKey, now, [{ ...actor, fishingFeedback: facts }], poses(action, started, index));
  assert.deepEqual(advance(1000, "fishingCast", 1000, 0).sounds, []);
  // A throttled browser can first observe frame two. Frame one still occurred.
  assert.deepEqual(advance(1200, "fishingCast", 1000, 2).sounds, ["fishingThrow"]);
  assert.deepEqual(advance(1300, "fishingCast", 1000, 3).sounds, []);
  assert.equal(advance(1919, "fishingWait", 1800, 0).effects.length, 0);
  const quiet = advance(1920, "fishingWait", 1800, 1);
  assert.deepEqual(quiet.sounds, []);
  assert.deepEqual(quiet.effects.map(({ spellOrEffect, source, x, y, startedAt, expiresAt }) =>
    [spellOrEffect, source, x, y, startedAt, expiresAt]), [
    ["FishingRippleInner", "fishingWater", 339, 247, 1920, 2640],
    ["FishingRippleOuter", "fishingWater", 339, 247, 1920, 2640],
  ]);
  assert.strictEqual(advance(2040, "fishingWait", 1800, 2, bite).effects, quiet.effects);
  const positive = advance(2640, "fishingWait", 2520, 1, bite);
  assert.deepEqual(positive.sounds, ["fishing"]);
  assert.deepEqual(positive.effects.map(({ spellOrEffect }) => spellOrEffect), [
    "FishingBiteInner", "FishingBiteOuter", "FishingBiteNotice",
  ]);
  assert.equal(positive.effects[2].objectId, actor.objectId);
  assert.deepEqual(advance(2760, "fishingWait", 2520, 2, bite).sounds, []);
  assert.deepEqual(advance(3240, "fishingReel", 3240, 0).sounds, []);
  const reel = advance(3340, "fishingReel", 3240, 1);
  assert.deepEqual(reel.sounds, ["fishingPull"]);
  assert.equal(reel.effects.length, 3, "the already-triggered map ripple/marker finishes independently");
  assert.deepEqual(advance(3460, "fishingReel", 3240, 2), { effects: [], sounds: [] });
});

test("retiring the owner or map cannot leave a bite icon on an unrelated incarnation", () => {
  const presentation = new FishingFeedbackPresentation();
  const feedback = { ...actor, fishingFeedback: bite };
  const other = { ...feedback, objectId: "8" };
  const worldPoses = { ...poses("fishingWait", 1000, 1), ...poses("fishingWait", 1000, 1, 1, "8") };
  assert.equal(presentation.advance(worldKey, 1120, [feedback, other], worldPoses).effects.length, 6);
  const newPoses = { ...worldPoses, ...poses("standing", undefined, 0, 2) };
  const replaced = presentation.advance(worldKey, 1200, [actor, other], newPoses);
  assert.equal(replaced.effects.length, 5);
  assert.equal(replaced.effects.some(effect => effect.objectId === actor.objectId), false);
  const left = presentation.advance(worldKey, 1220, [], {});
  assert.equal(left.effects.length, 4, "each tile-anchored splash can finish outside its actor's view");
  assert.deepEqual(presentation.advance("session1:1", 1300, [actor], {}), { effects: [], sounds: [] });
});

test("the shipping manifest resolves exact Crystal effect ranges, intervals and offsets", async () => {
  const publicRoot = new URL("../public/", import.meta.url);
  const assets = await loadEffectAssets(async url => new Response(await readFile(new URL(url.slice(1), publicRoot))));
  for (const [name, library, start, count, interval, blend, first] of [
    ["FishingRippleInner", "Effect", 640, 6, 120, true, [23, -4, 4, 13]],
    ["FishingRippleOuter", "Effect", 650, 6, 120, true, [7, -4, 40, 27]],
    ["FishingBiteInner", "Effect", 665, 6, 120, true, [23, 0, 4, 9]],
    ["FishingBiteOuter", "Effect", 671, 6, 120, true, [17, 1, 16, 14]],
    ["FishingBiteNotice", "Prguse", 1350, 2, 360, false, [10, -89, 28, 32]],
  ]) {
    const animation = resolveMapEffect(assets, name);
    assert.ok(animation, name);
    assert.deepEqual([animation.interval, animation.durationMs, animation.blend, animation.light], [interval, 720, blend, 0]);
    assert.deepEqual([animation.frames[0].x, animation.frames[0].y, animation.frames[0].width, animation.frames[0].height], first);
    assert.deepEqual(animation.frames.map(frame => frame.path),
      Array.from({ length: count }, (_, i) => `/original-effects/${library}/${start + i}.png`));
    for (const frame of animation.frames) {
      const png = await readFile(new URL(frame.path.slice(1), publicRoot));
      assert.equal(png.subarray(1, 4).toString(), "PNG");
      assert.deepEqual([png.readUInt32BE(16), png.readUInt32BE(20)], [frame.width, frame.height]);
    }
  }
});
