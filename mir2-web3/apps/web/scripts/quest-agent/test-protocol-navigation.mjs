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

import { spawnSync } from "node:child_process";

// Each child owns a fresh module/slot and restores its constructor hooks. The
// production API stays unchanged, with no workspace/test-only exports.
function runWorkspaceFixture(body) {
  const source = `
import assert from "node:assert/strict";
const { findProtocolWalkPath } = await import(${JSON.stringify(new URL("./protocol-navigation.mjs", import.meta.url).href)});
const native = { Float64Array, Int32Array, Uint8Array };
const nativeFill = Object.fromEntries(Object.entries(native).map(([name, type]) => [name, type.prototype.fill]));
const allocations = [], fills = [];
function map(width, height, obstacles = []) {
  const blocked = new native.Uint8Array(width * height);
  for (const { x, y } of obstacles) blocked[y * width + x] = 1;
  return { mapFileName: "workspace-fixture", type: 100, width, height, blocked };
}
function instrument(beforeAllocation = () => {}) {
  for (const [name, type] of Object.entries(native)) {
    globalThis[name] = new Proxy(type, { construct(target, args) {
      beforeAllocation(name, args);
      const array = Reflect.construct(target, args, target);
      allocations.push({ name, array, length: array.length });
      return array;
    } });
    type.prototype.fill = function (...args) {
      fills.push({ name, length: this.length, value: args[0] });
      return Reflect.apply(nativeFill[name], this, args);
    };
  }
}
try {
${body}
} finally {
  for (const [name, type] of Object.entries(native)) {
    globalThis[name] = type;
    type.prototype.fill = nativeFill[name];
  }
}
`;
  const result = spawnSync(process.execPath, ["--input-type=module", "--eval", source], {
    encoding: "utf8",
    timeout: 30000,
    windowsHide: true,
  });
  assert.equal(result.error, undefined, result.error?.message);
  assert.equal(result.signal, null, result.stderr);
  assert.equal(result.status, 0, result.stderr || result.stdout);
}

test("workspace reuses buffers without full fills across sizes, obstacles and expansion limits", () => {
  runWorkspaceFixture(`
    instrument();
    const firstMap = map(30, 20);
    const query = { map: firstMap, start: { x: 0, y: 0 }, target: { x: 4, y: 0 } };
    const first = findProtocolWalkPath(query);
    const expected = [0, 1, 2, 3, 4].map(x => ({ x, y: 0 }));
    assert.deepEqual(first, expected);
    assert.deepEqual(allocations.map(x => [x.name, x.length]), [
      ["Float64Array", 600], ["Int32Array", 600], ["Uint8Array", 600], ["Int32Array", 600],
    ]);
    assert.equal(allocations.reduce((n, x) => n + x.array.byteLength, 0), 17 * 600);
    assert.equal(fills.length, 2, "only the first score/predecessor initialization fills");
    assert.ok(allocations[0].array.every(x => x === Infinity));
    assert.ok(allocations[1].array.every(x => x === -2));
    assert.ok(allocations[2].array.every(x => x === 0));

    const detour = findProtocolWalkPath({ ...query, dynamicObstacles: [{ x: 2, y: 0 }] });
    assert.ok(detour && !detour.some(p => p.x === 2 && p.y === 0));
    assert.equal(findProtocolWalkPath({ ...query, maxExpanded: 1 }), null);
    assert.equal(findProtocolWalkPath({
      ...query, dynamicObstacles: Array.from({ length: 20 }, (_, y) => ({ x: 2, y })),
    }), null);
    assert.deepEqual(findProtocolWalkPath(query), expected);
    assert.deepEqual(findProtocolWalkPath({ ...query, map: map(20, 30) }), expected);
    assert.deepEqual(findProtocolWalkPath({ ...query, map: map(5, 3) }), expected);
    assert.deepEqual(findProtocolWalkPath({ ...query, desiredDistance: 2 }), expected.slice(0, 3));
    assert.equal(allocations.length, 4, "same or smaller capacity has no new arrays");
    assert.equal(fills.length, 2, "repeated searches never refill the map-sized arrays");
    assert.deepEqual(first, expected, "returned paths do not alias workspace state");
    assert.ok(allocations[0].array.every(x => x === Infinity));
    assert.ok(allocations[1].array.every(x => x === -2));
    assert.ok(allocations[2].array.every(x => x === 0));

    assert.deepEqual(findProtocolWalkPath({ ...query, map: map(40, 20) }), expected);
    assert.equal(allocations.length, 8, "a larger query replaces only the single slot");
    assert.equal(fills.length, 4);
    assert.deepEqual(first, expected);
  `);
});

test("workspace finally resets dirty cells and releases a lease after a search getter throws", () => {
  runWorkspaceFixture(`
    const faulty = map(10, 10), blocked = faulty.blocked;
    let reads = 0;
    Object.defineProperty(faulty, "blocked", { get() {
      if (++reads === 3) throw Error("controlled search read failure");
      return blocked;
    } });
    instrument();
    const query = { map: faulty, start: { x: 0, y: 0 }, target: { x: 5, y: 0 } };
    assert.throws(() => findProtocolWalkPath(query), /controlled search read failure/);
    assert.equal(allocations.length, 4);
    assert.ok(allocations[0].array.every(x => x === Infinity));
    assert.ok(allocations[1].array.every(x => x === -2));
    assert.ok(allocations[2].array.every(x => x === 0));
    const expected = [0, 1, 2, 3, 4, 5].map(x => ({ x, y: 0 }));
    assert.deepEqual(findProtocolWalkPath({ ...query, map: map(10, 10) }), expected);
    assert.deepEqual(findProtocolWalkPath({ ...query, map: map(10, 10) }), expected);
    assert.equal(allocations.length, 4, "an exception must not leave the lease busy");
    assert.equal(fills.length, 2);
  `);
});

test("workspace releases the reservation when its initial allocation throws", () => {
  runWorkspaceFixture(`
    let fail = true;
    instrument(name => {
      if (fail && name === "Float64Array") { fail = false; throw Error("controlled allocation failure"); }
    });
    const query = { map: map(8, 8), start: { x: 0, y: 0 }, target: { x: 3, y: 0 } };
    assert.throws(() => findProtocolWalkPath(query), /controlled allocation failure/);
    assert.equal(allocations.length, 0);
    const expected = [0, 1, 2, 3].map(x => ({ x, y: 0 }));
    assert.deepEqual(findProtocolWalkPath(query), expected);
    assert.equal(allocations.length, 4);
    assert.deepEqual(findProtocolWalkPath(query), expected);
    assert.equal(allocations.length, 4, "allocation failure cannot force later fresh fallback");
  `);
});

test("workspace synchronous reentry uses fresh arrays during allocation and during search", () => {
  runWorkspaceFixture(`
    let nested, once = true;
    const innerMap = map(4, 4), outerMap = map(12, 12);
    instrument(name => {
      if (once && name === "Float64Array") {
        once = false;
        nested = findProtocolWalkPath({ map: innerMap, start: { x: 0, y: 0 }, target: { x: 3, y: 3 } });
      }
    });
    const query = { map: outerMap, start: { x: 0, y: 0 }, target: { x: 11, y: 11 } };
    const expected = Array.from({ length: 12 }, (_, x) => ({ x, y: x }));
    assert.deepEqual(findProtocolWalkPath(query), expected);
    assert.deepEqual(nested, Array.from({ length: 4 }, (_, x) => ({ x, y: x })));
    assert.equal(allocations.filter(x => x.length === 16).length, 3, "nested fresh path has no dirty array");
    assert.equal(allocations.filter(x => x.length === 144).length, 4);
    assert.equal(allocations.length, 7);
    assert.deepEqual(findProtocolWalkPath(query), expected);
    assert.equal(allocations.length, 7, "nested allocation never replaces the outer slot");
  `);
  runWorkspaceFixture(`
    const outerMap = map(12, 12), blocked = outerMap.blocked, innerMap = map(12, 12);
    let reads = 0, nested;
    Object.defineProperty(outerMap, "blocked", { get() {
      if (++reads === 3) nested = findProtocolWalkPath({ map: innerMap, start: { x: 0, y: 0 }, target: { x: 11, y: 11 } });
      return blocked;
    } });
    instrument();
    const query = { map: outerMap, start: { x: 0, y: 11 }, target: { x: 11, y: 0 } };
    const expected = Array.from({ length: 12 }, (_, x) => ({ x, y: 11 - x }));
    assert.deepEqual(findProtocolWalkPath(query), expected);
    assert.deepEqual(nested, Array.from({ length: 12 }, (_, x) => ({ x, y: x })));
    assert.equal(allocations.length, 7);
    const scores = allocations.filter(x => x.name === "Float64Array");
    assert.notEqual(scores[0].array.buffer, scores[1].array.buffer);
    assert.ok(scores[0].array.every(x => x === Infinity), "outer dirty cells reset after nested search");
    assert.deepEqual(findProtocolWalkPath(query), expected);
    assert.equal(allocations.length, 7);
  `);
});

test("workspace retention stops at four million cells and oversized custom maps keep fresh behavior", () => {
  runWorkspaceFixture(`
    instrument();
    const maxMap = map(2000, 2000);
    const query = { start: { x: 0, y: 0 }, target: { x: 0, y: 0 }, maxExpanded: 1 };
    assert.deepEqual(findProtocolWalkPath({ ...query, map: maxMap }), [{ x: 0, y: 0 }]);
    assert.equal(allocations.length, 4);
    assert.equal(allocations.reduce((n, x) => n + x.array.byteLength, 0), 68_000_000);
    assert.equal(fills.length, 2);
    assert.deepEqual(findProtocolWalkPath({ ...query, map: map(2, 2) }), [{ x: 0, y: 0 }]);
    assert.equal(allocations.length, 4);
    assert.equal(fills.length, 2);

    // Public custom collision-map objects historically may exceed the parser's
    // cap. They must not gain a new rejection or evict the bounded retained slot.
    const oversized = map(4_000_001, 1);
    assert.deepEqual(findProtocolWalkPath({ ...query, map: oversized }), [{ x: 0, y: 0 }]);
    const fresh = allocations.filter(x => x.length === 4_000_001);
    assert.deepEqual(fresh.map(x => x.name), ["Float64Array", "Int32Array", "Uint8Array"]);
    assert.equal(allocations.length, 7);
    assert.equal(fills.length, 4);
    assert.deepEqual(findProtocolWalkPath({ ...query, map: map(2, 2) }), [{ x: 0, y: 0 }]);
    assert.equal(allocations.length, 7, "oversized fallback does not replace the bounded slot");
    assert.equal(fills.length, 4);
  `);
});


test("exact blocked destinations return null before allocating a search workspace", () => {
  runWorkspaceFixture(`
    const open = map(24, 24), blocked = map(24, 24, [{ x: 22, y: 22 }]);
    const start = { x: 1, y: 1 }, target = { x: 22, y: 22 };
    instrument();
    assert.equal(findProtocolWalkPath({ map: open, start, target, dynamicObstacles: [target] }), null);
    assert.equal(findProtocolWalkPath({ map: blocked, start, target }), null);
    assert.equal(findProtocolWalkPath({ map: blocked, start, target, staticWalkableOverrides: [target], dynamicObstacles: [target] }), null);
    assert.equal(allocations.length, 0, "an impossible exact goal must not explore or allocate a whole-map search");
    assert.equal(fills.length, 0);
  `);
});

test("exact goal guard preserves range, current origin and authoritative overrides", () => {
  const start = { x: 0, y: 0 }, target = { x: 2, y: 0 }, open = collisionMap(4, 3);
  const near = findProtocolWalkPath({ map: open, start, target, desiredDistance: 1, dynamicObstacles: [target] });
  assert.deepEqual(near, [start, { x: 1, y: 0 }], "a blocked target still permits an in-range neighbour");
  const blocked = collisionMap(4, 3, [start, target]);
  assert.deepEqual(findProtocolWalkPath({ map: blocked, start, target: start, dynamicObstacles: [start] }), [start]);
  assert.equal(findProtocolWalkPath({ map: blocked, start, target }), null);
  assert.deepEqual(findProtocolWalkPath({ map: blocked, start, target, staticWalkableOverrides: [target] }), [start, { x: 1, y: 0 }, target]);
  assert.equal(findProtocolWalkPath({ map: blocked, start, target, staticWalkableOverrides: [target], dynamicObstacles: [target] }), null);
  assert.equal(findProtocolWalkPath({ map: open, start, target, maxExpanded: 1 }), null);
  assert.deepEqual(findProtocolWalkPath({ map: open, start, target }), [start, { x: 1, y: 0 }, target]);
});

test("exact goal guard preserves input conversion and accessor-map failures", () => {
  const start = { x: 0, y: 0 }, target = { x: 3, y: 3 }, map = collisionMap(6, 6);
  const boom = new Error("original expansion-limit conversion");
  let reads = 0;
  assert.throws(() => findProtocolWalkPath({ map, start, target, dynamicObstacles: [target],
    get maxExpanded() { reads++; return { valueOf() { throw boom; } }; } }), error => error === boom);
  assert.equal(reads, 1);
  assert.throws(() => findProtocolWalkPath({ map, start, target, dynamicObstacles: [target], staticWalkableOverrides: [{ x: "invalid", y: 0 }] }), TypeError);
  assert.throws(() => findProtocolWalkPath({ map, start: { x: 0.5, y: 0 }, target, dynamicObstacles: [target] }), TypeError);
  const cells = map.blocked;
  let blockedReads = 0;
  Object.defineProperty(map, "blocked", { get() { if (++blockedReads === 3) throw boom; return cells; } });
  assert.throws(() => findProtocolWalkPath({ map, start, target, dynamicObstacles: [target] }), error => error === boom);
  assert.equal(blockedReads, 3, "accessor maps keep the original validated-search read order");
});
