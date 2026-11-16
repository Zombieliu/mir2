import assert from "node:assert/strict";
import test from "node:test";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { gzipSync } from "node:zlib";
import { loadClientCollision, nextWalkDirection } from "../src/skills/navigation.mjs";

async function removeFixture(directory, prefix) {
  const target = resolve(directory);
  if (dirname(target) !== resolve(tmpdir()) || !basename(target).startsWith(prefix)) throw new Error("Refusing to remove a path outside the named test fixture.");
  await rm(target, { recursive: true, force: true });
}

test("distributed Bichon and safe interior assets decode their different real formats", async () => {
  const field = await loadClientCollision("0");
  const interior = await loadClientCollision("0141");
  assert.equal(field.width, 700);
  assert.equal(field.height, 700);
  assert.ok(field.blocked.size > 0);
  assert.equal(interior.width, 27);
  assert.equal(interior.height, 32);
  assert.ok(interior.blocked.size > 0);
  assert.equal(field.blocked.has("288,616"), false);
});

test("path traversal and unapproved maps fail before reading local paths", async () => {
  for (const map of ["../0", "0/../../secret", "D022", "0.map", "", 0]) {
    await assert.rejects(loadClientCollision(map), /only map/);
  }
});

test("walk intents avoid walls, occupied cells, diagonal corner cuts and invalid bounds", () => {
  const collision = { width: 4, height: 4, blocked: new Set(["1,0"]) };
  assert.equal(nextWalkDirection({ player: { x: 0, y: 0 }, target: { x: 1, y: 1 }, desiredDistance: 0, collision }), "Down");
  assert.equal(nextWalkDirection({ player: { x: 0, y: 0 }, target: { x: 3, y: 3 }, collision, occupied: [{ x: 0, y: 1 }] }), null);
  assert.equal(nextWalkDirection({ player: { x: 0, y: 0 }, target: { x: 0, y: 1 }, desiredDistance: 1, collision }), null);
  assert.equal(nextWalkDirection({ player: { x: 0, y: 0 }, target: { x: 99, y: 99 }, collision }), null);
  assert.equal(nextWalkDirection({ player: { x: NaN, y: 0 }, target: { x: 1, y: 1 }, collision }), null);
});

test("closed doors and exported transfer tiles are blocked even on an otherwise empty map", async (t) => {
  const dir = await mkdtemp(join(tmpdir(), "mir2-nav-"));
  t.after(() => removeFixture(dir, "mir2-nav-"));
  const mapPackRoot = join(dir, "maps");
  await mkdir(mapPackRoot);
  const bytes = Buffer.alloc(8 + 3 * 3 * 26);
  bytes.set([1, 0, 0x43, 0x23]); bytes.writeInt16LE(3, 4); bytes.writeInt16LE(3, 6);
  bytes[8 + (1 * 3 + 0) * 26 + 14] = 1;
  await writeFile(join(mapPackRoot, "0.map.gz"), gzipSync(bytes));
  await writeFile(join(dir, "crystal_respawn_manifest.json"), JSON.stringify({ maps: [{ map_file_name: "0", movements: [{ source: { x: 1, y: 1 } }] }] }));
  const collision = await loadClientCollision("0", { mapPackRoot });
  assert.equal(collision.blocked.has("1,0"), true);
  assert.equal(collision.blocked.has("1,1"), true);
  assert.notEqual(nextWalkDirection({ player: { x: 0, y: 0 }, target: { x: 2, y: 0 }, desiredDistance: 0, collision }), "Right");
});

test("truncated and unsupported client assets fail closed", async (t) => {
  const dir = await mkdtemp(join(tmpdir(), "mir2-nav-invalid-"));
  t.after(() => removeFixture(dir, "mir2-nav-invalid-"));
  await writeFile(join(dir, "0.map.gz"), gzipSync(Buffer.from("not-a-map")));
  await assert.rejects(loadClientCollision("0", { mapPackRoot: dir }), /Unsupported/);
  const bytes = Buffer.alloc(8); bytes.set([1, 0, 0x43, 0x23]); bytes.writeInt16LE(2, 4); bytes.writeInt16LE(2, 6);
  await writeFile(join(dir, "0.map.gz"), gzipSync(bytes));
  await assert.rejects(loadClientCollision("0", { mapPackRoot: dir }), /truncated/);
});
