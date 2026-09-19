import assert from "node:assert/strict";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { gzipSync } from "node:zlib";

import {
  findProtocolWalkPath,
  findProtocolTransferExitStep,
  loadProtocolCollisionMap,
  normalizeProtocolMapFileName,
  parseCrystalCollisionMap,
  planProtocolNavigation,
  protocolMapCellIsWalkable,
  protocolWalkSteps,
} from "./protocol-navigation.mjs";

function collisionMap(width, height, blockedPoints = []) {
  const blocked = new Uint8Array(width * height);
  for (const { x, y } of blockedPoints) blocked[y * width + x] = 1;
  return { mapFileName: "fixture", type: 100, width, height, blocked };
}

function type100Fixture(width, height, { blocked = [], doors = [] } = {}) {
  const result = Buffer.alloc(8 + width * height * 26);
  result[2] = 0x43;
  result[3] = 0x23;
  result.writeInt16LE(width, 4);
  result.writeInt16LE(height, 6);
  const blockedKeys = new Set(blocked.map(({ x, y }) => `${x},${y}`));
  const doorByKey = new Map(doors.map(({ x, y, index }) => [`${x},${y}`, index]));
  let offset = 8;
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      const key = `${x},${y}`;
      if (blockedKeys.has(key)) result.writeInt32LE(0x20000000, offset + 2);
      result.writeUInt8(doorByKey.get(key) ?? 0, offset + 14);
      offset += 26;
    }
  }
  return result;
}

test("loads a mapFileName from the local packaged map resource", async () => {
  const tempRoot = await mkdtemp(path.join(os.tmpdir(), "protocol-navigation-"));
  try {
    const packagedMapRoot = path.join(tempRoot, "packaged");
    await mkdir(packagedMapRoot);
    await writeFile(path.join(packagedMapRoot, "test0.map.gz"), gzipSync(type100Fixture(4, 3, {
      blocked: [{ x: 2, y: 1 }],
      doors: [{ x: 1, y: 2, index: 7 }],
    })));
    const map = await loadProtocolCollisionMap("test0.map", { packagedMapRoot, clientRoot: path.join(tempRoot, "missing") });
    assert.equal(map.mapFileName, "test0");
    assert.deepEqual([map.width, map.height, map.type], [4, 3, 100]);
    assert.equal(map.blocked[1 * map.width + 2], 1);
    assert.equal(map.blocked[2 * map.width + 1], 1, "closed doors are initially impassable");
    assert.deepEqual(map.doors, [{ x: 1, y: 2, index: 7, closed: true }]);
  } finally {
    await rm(tempRoot, { recursive: true, force: true });
  }
});

test("rejects mapFileName traversal before reading local files", () => {
  assert.throws(() => normalizeProtocolMapFileName("../0"), /Invalid Crystal mapFileName/);
});

test("A* detours around a static wall and emits only unit Walk steps", () => {
  const map = collisionMap(7, 5, [
    { x: 3, y: 0 }, { x: 3, y: 1 }, { x: 3, y: 2 }, { x: 3, y: 3 },
  ]);
  const plan = planProtocolNavigation({ map, start: { x: 1, y: 2 }, target: { x: 5, y: 2 } });
  assert.ok(plan);
  assert.deepEqual(plan.path[0], { x: 1, y: 2 });
  assert.deepEqual(plan.path.at(-1), { x: 5, y: 2 });
  assert.ok(plan.path.some(({ y }) => y === 4), "path should use the wall opening");
  assert.equal(plan.steps.length, plan.path.length - 1);
  assert.ok(plan.steps.every(({ dx, dy }) => Math.max(Math.abs(dx), Math.abs(dy)) === 1));
});

test("dynamic obstacles participate in route planning", () => {
  const map = collisionMap(5, 3);
  const direct = findProtocolWalkPath({ map, start: { x: 0, y: 1 }, target: { x: 4, y: 1 } });
  const detour = findProtocolWalkPath({
    map,
    start: { x: 0, y: 1 },
    target: { x: 4, y: 1 },
    dynamicObstacles: [{ x: 2, y: 1 }],
  });
  assert.ok(direct.some(({ x, y }) => x === 2 && y === 1));
  assert.ok(detour);
  assert.ok(!detour.some(({ x, y }) => x === 2 && y === 1));
});

test("a static transfer override opens only its exact cell and never bypasses dynamic occupancy", () => {
  const transfer = { x: 2, y: 1 };
  const otherWall = { x: 3, y: 1 };
  const map = collisionMap(5, 3, [transfer, otherWall]);

  assert.equal(protocolMapCellIsWalkable(map, transfer), false);
  assert.equal(protocolMapCellIsWalkable(map, transfer, [], [transfer]), true);
  assert.equal(protocolMapCellIsWalkable(map, otherWall, [], [transfer]), false);
  assert.equal(protocolMapCellIsWalkable(map, transfer, [transfer], [transfer]), false);
});

test("transfer re-entry chooses a normal walkable unoccupied step outside the source bounds", () => {
  const source = { x: 2, y: 2 };
  const map = collisionMap(5, 5, [{ x: 2, y: 1 }]);
  assert.deepEqual(findProtocolTransferExitStep({
    map,
    start: source,
    bounds: { minX: 2, maxX: 2, minY: 2, maxY: 2 },
    dynamicObstacles: [{ x: 3, y: 2 }],
  }), { x: 2, y: 3 });

  assert.equal(findProtocolTransferExitStep({
    map: collisionMap(3, 3, [{ x: 1, y: 0 }, { x: 2, y: 1 }, { x: 1, y: 2 }]),
    start: { x: 1, y: 1 },
    bounds: { minX: 1, maxX: 1, minY: 1, maxY: 1 },
    dynamicObstacles: [{ x: 0, y: 1 }],
  }), null);
});

test("real Bichon MageHouse transfer source can be reached at distance zero only with its exact live override", async () => {
  const map = await loadProtocolCollisionMap("0");
  const start = { x: 285, y: 607 };
  const transfer = { x: 314, y: 474 };

  assert.equal(planProtocolNavigation({ map, start, target: transfer, desiredDistance: 0 }), null);
  const plan = planProtocolNavigation({
    map,
    start,
    target: transfer,
    desiredDistance: 0,
    staticWalkableOverrides: [transfer],
  });
  assert.ok(plan);
  assert.deepEqual(plan.path.at(-1), transfer);
  assert.deepEqual(plan.steps.at(-1).to, transfer);

  assert.equal(planProtocolNavigation({
    map,
    start,
    target: transfer,
    desiredDistance: 0,
    staticWalkableOverrides: [transfer],
    dynamicObstacles: [transfer],
  }), null);
});

test("diagonal Walk cannot cut between blocked cardinal neighbors", () => {
  const map = collisionMap(3, 3, [{ x: 1, y: 0 }, { x: 0, y: 1 }]);
  assert.equal(findProtocolWalkPath({ map, start: { x: 0, y: 0 }, target: { x: 1, y: 1 } }), null);
});

test("an exact live transfer override permits only the final diagonal door step", () => {
  const transfer = { x: 1, y: 1 };
  const map = collisionMap(3, 3, [{ x: 1, y: 0 }, { x: 0, y: 1 }, transfer]);
  assert.equal(findProtocolWalkPath({ map, start: { x: 0, y: 0 }, target: transfer }), null);
  assert.deepEqual(findProtocolWalkPath({
    map,
    start: { x: 0, y: 0 },
    target: transfer,
    staticWalkableOverrides: [transfer],
  }), [{ x: 0, y: 0 }, transfer]);
  assert.equal(findProtocolWalkPath({
    map,
    start: { x: 0, y: 0 },
    target: transfer,
    staticWalkableOverrides: [transfer],
    dynamicObstacles: [transfer],
  }), null);
});

test("real SerpentValley blacksmith transfer has a path through its authoritative diagonal door", async () => {
  const map = await loadProtocolCollisionMap("2");
  const start = { x: 411, y: 566 };
  const transfer = { x: 516, y: 492 };
  assert.equal(planProtocolNavigation({ map, start, target: transfer }), null);
  const plan = planProtocolNavigation({
    map,
    start,
    target: transfer,
    staticWalkableOverrides: [transfer],
  });
  assert.ok(plan);
  assert.deepEqual(plan.path.at(-1), transfer);
});

test("returns null for an unreachable target", () => {
  const map = collisionMap(5, 5, Array.from({ length: 5 }, (_, y) => ({ x: 2, y })));
  assert.equal(findProtocolWalkPath({ map, start: { x: 0, y: 2 }, target: { x: 4, y: 2 } }), null);
});

test("protocolWalkSteps rejects skips that cannot be sent as one normal Walk", () => {
  assert.throws(
    () => protocolWalkSteps([{ x: 1, y: 1 }, { x: 3, y: 1 }]),
    /non-unit Walk step/,
  );
});

test("type100 parser can leave doors open when live door state is supplied separately", () => {
  const map = parseCrystalCollisionMap(type100Fixture(2, 2, {
    doors: [{ x: 1, y: 1, index: 3 }],
  }), "door-map", { doorsBlocked: false });
  assert.equal(map.blocked[3], 0);
  assert.deepEqual(map.doors, [{ x: 1, y: 1, index: 3, closed: false }]);
});

test("type100 back-image index bit is not mistaken for the high-wall flag", () => {
  const bytes = type100Fixture(1, 1);
  bytes.writeInt32LE(0x00008000, 8 + 2);
  const map = parseCrystalCollisionMap(bytes, "image-index");
  assert.equal(map.blocked[0], 0);
});
