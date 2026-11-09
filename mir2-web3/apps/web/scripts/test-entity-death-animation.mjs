import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";

import ts from "typescript";

const sourceUrl = new URL(
  "../app/components/original-client-entity-animation-runtime.ts",
  import.meta.url,
);
const source = readFileSync(sourceUrl, "utf8");
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
    strict: true,
  },
  fileName: fileURLToPath(sourceUrl),
  reportDiagnostics: true,
});
assert.deepEqual(
  (compiled.diagnostics ?? []).filter(
    (diagnostic) => diagnostic.category === ts.DiagnosticCategory.Error,
  ),
  [],
);
const loaded = { exports: {} };
new Function("exports", "module", compiled.outputText)(loaded.exports, loaded);

const { animationEventForEntity, resolveCrystalEntityAnimationPoses } = loaded.exports;

test("sends a live player movement hint only for the selected moving event", () => {
  const player = { objectId: "7", kind: "selfPlayer", x: 31, y: 44, direction: "Right" };
  const segment = { fromX: 30, fromY: 44, toX: 31, toY: 44,
    animationState: "walking", startedAt: 1000, expiresAt: 1600 };
  function input(entity, state, motionSnapshot, selfAuthority) {
    let delivered;
    const runtime = { resolveMir2EntityAnimationPoses(raw) {
      const { worldKey, nowMs, entities } = JSON.parse(raw);
      delivered = entities[0];
      return JSON.stringify({ worldKey, nowMs, poses: [] });
    } };
    resolveCrystalEntityAnimationPoses({ runtime, worldKey: "0:7", worldSeed: 7, now: 1100, selfAuthority,
      entities: [{ entity, state, motionSnapshot }] });
    return delivered;
  }
  const packet = { ...player, movementAnimation: "walking", movementStartedAt: 1000, movementUntil: 1600 };
  assert.equal(input(packet, "walking").movementActive, true);
  assert.equal(input({ ...packet, kind: "player" }, "walking", segment).movementActive, true);
  const source = { owner: { playerObjectId: 7 }, sourceKey: "0:7", revision: 1,
    self: player, bootstrapKnown: false, animationKnown: false };
  const ownerMotion = input(player, "walking", segment, source);
  assert.equal(ownerMotion.movementActive, true);
  assert.equal(ownerMotion.actionKnown, false);
  const sourcedMotion = input(packet, "walking", segment, { ...source, animationKnown: true,
    actionEvidence: { family: "walking", token: "action:1", receivedAtMs: 1000 } });
  assert.equal(sourcedMotion.actionToken, "action:1:move:1000:walking");
  assert.equal(sourcedMotion.movementActive, true);
  assert.equal(input(player, "walking", { ...segment, fromX: 30.5 }).movementActive, true);
  for (const entry of [
    input({ ...player, kind: "player" }, "walking", segment),
    input(player, "walking", { ...segment, expiresAt: 1100 }),
    input(player, "walking", { ...segment, startedAt: 1101 }),
    input(player, "walking", { ...segment, toX: 32 }),
    input(player, "walking", { ...segment, fromX: 31 }),
    input(player, "walking", { ...segment, fromX: 26 }),
    input(player, "walking", { ...segment, fromX: Infinity }),
    input({ ...packet, movementStartedAt: 1101 }, "walking", segment),
    input({ ...packet, movementUntil: 1100 }, "walking"),
    input({ ...packet, movementAnimation: "running" }, "walking", segment),
    input({ ...packet, kind: "monster" }, "walking", segment),
    input({ ...packet, attackAnimation: "spell", attackStartedAt: 1100 }, "attackRange", segment),
  ]) assert.notEqual(entry.movementActive, true);
});

test("maps the death window to Die and the corpse state to Dead", () => {
  const monster = {
    objectId: "monster-12",
    kind: "monster",
    direction: "Down",
    dead: true,
    dieStartedAt: 12_345,
  };

  assert.deepEqual(animationEventForEntity(monster, "dying"), {
    action: "die",
    actionToken: "life:12345",
  });
  assert.deepEqual(animationEventForEntity(monster, "dead"), {
    action: "dead",
    actionToken: "life:12345",
  });
});

test("a new death incarnation receives a new action token", () => {
  const monster = {
    objectId: "monster-12",
    kind: "monster",
    direction: "Down",
    dead: true,
    dieStartedAt: 12_346,
  };

  assert.equal(
    animationEventForEntity(monster, "dying").actionToken,
    "life:12346",
  );
});

test("maps a Scarecrow revive window to a distinct Revive action token", () => {
  const scarecrow = {
    objectId: "monster-2005",
    kind: "monster",
    direction: "Right",
    dead: false,
    reviveStartedAt: 20_005,
  };

  assert.deepEqual(animationEventForEntity(scarecrow, "reviving"), {
    action: "revive",
    actionToken: "revive:20005",
  });
});
