import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { loadObservedMonsterLocations } from "./protocol-memory.mjs";

test("streams redacted trace observations and returns only recent location hints", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mir2-memory-"));
  const tracePath = path.join(directory, "Warrior.trace.jsonl");
  const rows = [
    "not json",
    packet("ObjectMonster", { name: "IgnoredBeforeMap", location: { x: 1, y: 1 }, objectId: 90, hp: 20 }),
    packet("MapInformation", { fileName: "0", title: "BichonProvince" }),
    packet("ObjectMonster", { name: "CannibalPlant", location: { x: 92, y: 446 }, objectId: 91, hp: 20 }),
    packet("ObjectMonster", { name: "CannibalPlant", location: { x: 93, y: 447 }, objectId: 92, hp: 0 }),
    JSON.stringify({ direction: "sent", packet: "ObjectMonster", payload: { name: "SentPhantom", location: { x: 5, y: 5 } } }),
    packet("MapChanged", { mapFileName: "D2042" }),
    JSON.stringify({ direction: "received", type: "worldSnapshot", payload: {
      mapFileName: "D2042",
      entities: [
        { kind: "monster", name: "Oma", x: 10, y: 20, objectId: 101, hp: 3 },
        { kind: "npc", name: "Guard", x: 11, y: 20, objectId: 102 },
      ],
    } }),
    packet("NewMonsterInfo", { name: "CannibalPlant", x: 94, y: 448, objectId: 93, hp: 20 }),
    packet("MapInformation", { fileName: "0" }),
    packet("ObjectMonster", { name: "CannibalPlant", location: { x: 92, y: 446 }, objectId: 94, hp: 99 }),
  ];
  await writeFile(tracePath, `${rows.join("\n")}\n`, "utf8");
  try {
    const locations = await loadObservedMonsterLocations(tracePath, { maxPerMonsterMap: 2 });
    assert.deepEqual(locations, [
      { monsterName: "CannibalPlant", mapFileName: "0", x: 92, y: 446 },
      { monsterName: "CannibalPlant", mapFileName: "D2042", x: 94, y: 448 },
      { monsterName: "Oma", mapFileName: "D2042", x: 10, y: 20 },
      { monsterName: "CannibalPlant", mapFileName: "0", x: 93, y: 447 },
    ]);
    assert.ok(locations.every(value => Object.keys(value).sort().join(",") === "mapFileName,monsterName,x,y"));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("a new isolated run starts with empty memory when no earlier trace exists", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "mir2-memory-missing-"));
  try {
    assert.deepEqual(await loadObservedMonsterLocations(path.join(directory, "missing.trace.jsonl")), []);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

function packet(name, payload) {
  return JSON.stringify({ direction: "received", type: "packet", packet: name, payload });
}
