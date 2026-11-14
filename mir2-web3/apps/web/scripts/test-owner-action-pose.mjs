import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { fileURLToPath } from "node:url";
import ts from "typescript";

function loadTsModule(url) {
  const compiled = ts.transpileModule(readFileSync(url, "utf8"), {
    compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
    fileName: fileURLToPath(url),
  });
  const mod = { exports: {} };
  const require = (specifier) => {
    // Pose projection and FlamingSword creation do not resolve atlas frames.
    if (specifier === "./crystal-magic-effects") return {};
    throw new Error(`Unexpected dependency: ${specifier}`);
  };
  new Function("exports", "module", "require", compiled.outputText)(mod.exports, mod, require);
  return mod.exports;
}

const runtime = loadTsModule(new URL("../lib/scene-effect-runtime.ts", import.meta.url));
const pageSource = readFileSync(new URL("../app/page.tsx", import.meta.url), "utf8");
const authoritativeOwner = Object.freeze({
  objectId: "owner-1",
  x: 235,
  y: 602,
  direction: "Down",
  attackAnimation: "melee1",
  attackStartedAt: 32_000,
  attackUntil: 32_600,
});
const actionPackets = [
  ["ObjectAttack", "Left"],
  ["ObjectRangeAttack", "Up"],
  ["ObjectMagic", "Right"],
].map(([type, direction]) => ({
  type,
  objectId: "owner-1",
  location: { x: 236, y: 601 },
  direction,
}));

for (const packet of actionPackets) {
  test(`${packet.type} preserves the owner's newer UserLocation and updates direction`, () => {
    const projected = runtime.applyObjectActionScenePose(authoritativeOwner, packet, "owner-1");
    assert.deepEqual(projected, { ...authoritativeOwner, direction: packet.direction });
    assert.notEqual(projected, authoritativeOwner);
  });

  test(`${packet.type} continues applying remote action anchors`, () => {
    const remote = { ...authoritativeOwner, objectId: "remote-2" };
    assert.deepEqual(
      runtime.applyObjectActionScenePose(remote, { ...packet, objectId: "remote-2" }, "owner-1"),
      { ...remote, x: 236, y: 601, direction: packet.direction },
    );
  });

  test(`${packet.type} without owner identity retains legacy pose behavior`, () => {
    assert.deepEqual(
      runtime.applyObjectActionScenePose(authoritativeOwner, packet),
      { ...authoritativeOwner, x: 236, y: 601, direction: packet.direction },
    );
  });
}

test("consecutive late action snapshots never roll back or mutate the owner", () => {
  let pose = authoritativeOwner;
  for (const packet of actionPackets) {
    pose = runtime.applyObjectActionScenePose(pose, packet, "owner-1");
    assert.equal(pose.x, 235);
    assert.equal(pose.y, 602);
    assert.equal(pose.direction, packet.direction);
    assert.equal(pose.attackUntil, 32_600);
  }
  assert.equal(authoritativeOwner.direction, "Down");
  assert.equal(authoritativeOwner.x, 235);
  assert.equal(authoritativeOwner.y, 602);
});

test("owner attack reducer preserves position and still updates animation, timing and FlamingSword", () => {
  const remote = { ...authoritativeOwner, objectId: "remote-2" };
  const entities = Object.freeze([authoritativeOwner, remote]);
  const effects = Object.freeze([]);
  const packet = Object.freeze({ ...actionPackets[0], direction: "UpLeft", attackType: 2, spell: 8 });
  const projected = runtime.applyObjectAttackSceneState(
    entities, effects, packet, 33_000, () => 600, "owner-1",
  );
  assert.deepEqual(projected.entities[0], {
    ...authoritativeOwner,
    direction: "UpLeft",
    attackAnimation: "melee3",
    attackStartedAt: 33_000,
    attackUntil: 33_600,
  });
  assert.equal(projected.entities[1], remote);
  assert.equal(projected.effects.length, 1);
  assert.deepEqual(projected.effects[0], {
    key: "crystal-attack-overlay:FlamingSword:owner-1",
    source: "attackOverlay",
    spellOrEffect: 8,
    objectId: "owner-1",
    x: 236,
    y: 601,
    direction: 7,
    value: 0,
    startedAt: 33_000,
    expiresAt: 33_600,
  });
  assert.equal(entities[0], authoritativeOwner);
  assert.equal(effects.length, 0);
});

test("remote attack reducer applies anchor and animation while owner stays unchanged", () => {
  const projected = runtime.applyObjectAttackSceneState(
    [authoritativeOwner, { ...authoritativeOwner, objectId: "remote-2" }],
    [],
    { objectId: "remote-2", location: { x: 236, y: 601 }, direction: "Right", attackType: 1 },
    33_100,
    () => 600,
    "owner-1",
  );
  assert.equal(projected.entities[0], authoritativeOwner);
  assert.deepEqual(projected.entities[1], {
    ...authoritativeOwner,
    objectId: "remote-2",
    x: 236,
    y: 601,
    direction: "Right",
    attackAnimation: "melee2",
    attackStartedAt: 33_100,
    attackUntil: 33_700,
  });
});

test("attack reducer's optional owner argument preserves older callers", () => {
  const effects = [];
  const projected = runtime.applyObjectAttackSceneState(
    [authoritativeOwner], effects, actionPackets[0], 33_000, () => 600,
  );
  assert.equal(projected.entities[0].x, 236);
  assert.equal(projected.entities[0].y, 601);
  assert.equal(projected.effects, effects);
});

function pageFunction(name) {
  const start = pageSource.indexOf(`  function ${name}(`);
  assert.ok(start >= 0, `${name} must exist in the real page`);
  const end = pageSource.indexOf("\n  function ", start + 1);
  return pageSource.slice(start, end < 0 ? undefined : end);
}

test("page routes only the three action poses through owner protection", () => {
  assert.match(pageFunction("markWorldEntityAttack"),
    /applyObjectAttackSceneState\([\s\S]*crystalAttackActionDurationMs,\s*current\.playerObjectId/);
  assert.match(pageSource,
    /case "ObjectRangeAttack":\s*updateWorldEntityFromLocationPacket\(payload, true\);\s*gameBusRef[\s\S]*?spawnRangeProjectile\(payload\)/);
  assert.match(pageFunction("updateWorldEntityFromLocationPacket"),
    /ownerAction = false,[\s\S]*ownerAction\s*\? applyObjectActionScenePose\(entity, payload, current\.playerObjectId\)/);
  assert.match(pageFunction("markWorldEntityMagic"),
    /applyObjectActionScenePose\(entity, payload, current\.playerObjectId\)[\s\S]*attackAnimation: "spell"[\s\S]*attackUntil:/);
  assert.match(pageSource,
    /case "ObjectMagic":[\s\S]*?markWorldEntityMagic\(payload\);\s*enqueueSceneEffect\(payload, "spell"\);[\s\S]*?spawnRangeProjectile\(payload\)/);
  assert.equal([...pageSource.matchAll(/updateWorldEntityFromLocationPacket\(payload, true\)/g)].length, 1);
});

test("real movement, map and teleport packet paths retain position updates", () => {
  const movementStart = pageSource.indexOf('      case "UserLocation":');
  const movementEnd = pageSource.indexOf('      case "ObjectPlayer":', movementStart);
  assert.ok(movementStart >= 0 && movementEnd > movementStart);
  const movement = pageSource.slice(movementStart, movementEnd);
  for (const packet of ["UserLocation", "ObjectWalk", "ObjectRun", "ObjectPushed", "ObjectDash", "ObjectBackStep"]) {
    assert.ok(movement.includes(`case "${packet}":`), `${packet} must stay in the movement path`);
  }
  assert.match(movement, /withCrystalSelfPacketMovement\(\s*\{\s*\.\.\.entity,\s*x,\s*y,/);
  assert.match(movement, /withPacketMovementAnimation\(\s*\{\s*\.\.\.entity,\s*x,\s*y,/);
  assert.doesNotMatch(movement, /applyObjectActionScenePose|ownerAction/);
  for (const packet of ["ObjectTeleportOut", "ObjectTeleportIn"]) {
    assert.match(pageSource, new RegExp(`case "${packet}":\\s*updateWorldEntityFromLocationPacket\\(payload\\);`));
  }
  assert.match(pageSource, /case "ObjectHarvest":\s*case "ObjectHarvested":\s*updateWorldEntityFromLocationPacket\(payload\);/);
  for (const [packet, nextPacket] of [["MapInformation", "UserInformation"], ["MapChanged", "MapRegionChanged"]]) {
    const start = pageSource.indexOf(`      case "${packet}":`);
    assert.ok(start >= 0, `${packet} must retain its separate map handler`);
    const end = pageSource.indexOf(`      case "${nextPacket}":`, start + 1);
    const body = pageSource.slice(start, end < 0 ? pageSource.indexOf("\n      case ", start + 1) : end);
    assert.doesNotMatch(body, /applyObjectActionScenePose|ownerAction/);
  }
});
