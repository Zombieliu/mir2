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

const { appendFishingAnimationEdge, FishingFeedbackPresentation, fishingBiteButtonFrame, readFishingFeedback } =
  await loadTypeScript("../lib/world-fishing-feedback.ts");
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

test("packaged Web observer enters and re-enters active fishing in Wait without a recalled point", async () => {
  const manifest = JSON.parse(await readFile(new URL("../lib/generated/client_core_runtime.json", import.meta.url), "utf8"));
  const root = new URL("../public/client-core/" + manifest.presentation.version + "/", import.meta.url);
  const glue = await readFile(new URL("mir2_platform_web.js", root));
  const binary = await readFile(new URL("mir2_platform_web_bg.wasm", root));
  const wasm = await import("data:text/javascript;base64," + glue.toString("base64"));
  await wasm.default({ module_or_path: binary });
  const bridge = new wasm.EntityAnimationBridge();
  const { resolveCrystalEntityAnimationPoses } =
    await loadTypeScript("../app/components/original-client-entity-animation-runtime.ts");
  const presentation = new FishingFeedbackPresentation();
  const arrival = { ...actor, kind: "player", direction: "DownLeft", fishing: true };
  const tick = (now, entity) => {
    const entities = entity ? [entity] : [];
    const renderPoses = resolveCrystalEntityAnimationPoses({
      runtime: bridge, worldKey, worldSeed: 7, now,
      entities: entities.map(value => ({ entity: value, state: "standing" })),
    });
    return { pose: renderPoses[actor.objectId], feedback: presentation.advance(worldKey, now, entities, renderPoses) };
  };
  const quiet = { effects: [], sounds: [] };
  try {
    const entered = tick(1000, arrival);
    assert.equal(entered.pose.action, "fishingWait");
    assert.equal(entered.pose.logicalFrameIndex, 0);
    assert.deepEqual(tick(1120, arrival).feedback, quiet);
    assert.equal(tick(1200).pose, undefined);
    const returned = tick(1210, arrival);
    assert.equal(returned.pose.action, "fishingWait");
    assert.equal(returned.pose.logicalFrameIndex, 0);
    assert.equal(returned.pose.incarnation, entered.pose.incarnation + 1);
    assert.deepEqual(tick(1330, arrival).feedback, quiet);

    const reel = { ...arrival, fishing: false, fishingFeedback: ripple,
      fishingEdges: appendFishingAnimationEdge(undefined, true, false, 1340) };
    tick(1340, reel);
    assert.deepEqual(tick(2030, reel).feedback, { effects: [], sounds: ["fishingPull"] });
    assert.equal(tick(2730, reel).pose.action, "standing");
    const cast = { ...arrival, fishingFeedback: ripple,
      fishingEdges: appendFishingAnimationEdge(reel.fishingEdges, false, true, 2800) };
    assert.equal(tick(2800, cast).pose.action, "fishingCast");
    assert.deepEqual(tick(2900, cast).feedback, { effects: [], sounds: ["fishingThrow"] });
    const visibleRipple = tick(3720, cast).feedback;
    assert.deepEqual(visibleRipple.effects.map(effect => [effect.spellOrEffect, effect.x, effect.y]), [
      ["FishingRippleInner", 339, 247], ["FishingRippleOuter", 339, 247],
    ]);
    assert.deepEqual(visibleRipple.sounds, []);
    assert.equal(presentation.biteStartedAt, null);

    tick(3750);
    const enteredAgain = tick(3800, arrival);
    assert.equal(enteredAgain.pose.action, "fishingWait");
    assert.equal(enteredAgain.pose.logicalFrameIndex, 0);
    assert.equal(enteredAgain.pose.incarnation, returned.pose.incarnation + 1);
    assert.deepEqual(tick(3920, arrival).feedback, visibleRipple, "the prior map ripple can finish");
    assert.deepEqual(tick(4440, arrival).feedback, quiet, "re-entry never generates another ripple");
    assert.equal(presentation.biteStartedAt, null);
  } finally {
    bridge.free();
  }
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

test("the original reel button starts only on a self bite cue, loops and retires", () => {
  const presentation = new FishingFeedbackPresentation();
  const self = { ...actor, fishing: true, fishingFeedback: bite };
  const other = { ...self, kind: "player" };
  const advance = (entity, now, phase, started, index) =>
    presentation.advance(worldKey, now, [entity], poses(phase, started, index));
  advance(other, 1120, "fishingWait", 1000, 1);
  assert.equal(presentation.biteStartedAt, null);
  advance(self, 1720, "fishingWait", 1720, 0);
  assert.equal(presentation.biteStartedAt, null);
  advance(self, 1840, "fishingWait", 1720, 1);
  assert.equal(presentation.biteStartedAt, 1840);
  advance(self, 2560, "fishingWait", 2440, 1);
  assert.equal(presentation.biteStartedAt, 1840, "later wait cycles do not restart the button");
  assert.deepEqual([0, 129, 130, 1299, 1300].map(delta => fishingBiteButtonFrame(1840 + delta, 1840)),
    [170, 170, 171, 179, 170]);
  advance({ ...self, fishingFeedback: ripple }, 2600, "fishingWait", 2440, 1);
  assert.equal(presentation.biteStartedAt, null);
  advance(self, 3280, "fishingWait", 3160, 1);
  assert.equal(presentation.biteStartedAt, 3280);
  const recast = { ...self, fishingEdges: [{ active: false, receivedAtMs: 3300 }, { active: true, receivedAtMs: 3301 }] };
  advance(recast, 3310, "fishingWait", 3160, 1);
  assert.equal(presentation.biteStartedAt, null, "the new bite must wait for its own cast's pose");
  advance(recast, 4900, "fishingWait", 4780, 1);
  assert.equal(presentation.biteStartedAt, 4900);
  advance({ ...self, fishing: false }, 5000, "fishingWait", 4780, 1);
  assert.equal(presentation.biteStartedAt, null);
});

test("only actual flag transitions extend the bounded presentation history", () => {
  assert.equal(appendFishingAnimationEdge(undefined, undefined, true), undefined);
  let edges = appendFishingAnimationEdge(undefined, false, true);
  assert.deepEqual(edges.map(edge => edge.active), [false, true]);
  assert.strictEqual(appendFishingAnimationEdge(edges, true, true), edges);
  const first = edges[1];
  edges = appendFishingAnimationEdge(edges, true, false);
  edges = appendFishingAnimationEdge(edges, false, true);
  assert.strictEqual(edges[1], first);
  assert.deepEqual(edges.map(edge => edge.active), [false, true, false, true]);
  for (let index = 0; index < 50; index += 1) {
    const last = edges.at(-1).active;
    edges = appendFishingAnimationEdge(edges, last, !last);
  }
  assert.equal(edges.length, 16);
  assert(edges.every((edge, index) => index === 0 || edges[index - 1].active !== edge.active));
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
