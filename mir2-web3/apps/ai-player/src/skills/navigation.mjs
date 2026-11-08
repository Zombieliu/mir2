import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { findCollisionGridPath } from "../../../web/scripts/quest-agent/policy.mjs";

const DEFAULT_MAP_ROOT = fileURLToPath(new URL("../../../web/lib/generated/crystal-map-pack/", import.meta.url));
const MAPS = new Set(["0", "0141"]);
const MAX_CELLS = 1_500_000;
const point = value => Number.isInteger(value?.x) && Number.isInteger(value?.y);
const key = (x, y) => `${x},${y}`;

/** Read distributed client map assets, never personal ECS or an admin world view. */
export async function loadClientCollision(mapFileName, { mapPackRoot = DEFAULT_MAP_ROOT } = {}) {
  if (!MAPS.has(mapFileName)) throw new Error("Companion navigation supports only map 0 and 0141.");
  const root = resolve(mapPackRoot);
  const bytes = gunzipSync(await readFile(resolve(root, `${mapFileName}.map.gz`)), { maxOutputLength: 64 * 1024 * 1024 });
  let width, height, header, stride, blockedAt;
  if (bytes.length >= 8 && bytes[2] === 0x43 && bytes[3] === 0x23) {
    width = bytes.readInt16LE(4);
    height = bytes.readInt16LE(6);
    header = 8;
    stride = 26;
    blockedAt = offset => Boolean((bytes.readInt32LE(offset + 2) & 0x2000_0000) || (bytes.readInt16LE(offset + 12) & 0x8000) || (bytes[offset + 14] & 0x7f));
  } else if (bytes.length >= 54 && bytes[0] === 0x10 && bytes[2] === 0x61 && bytes[7] === 0x31 && bytes[14] === 0x31) {
    const xor = bytes.readInt16LE(23);
    width = bytes.readInt16LE(21) ^ xor;
    height = bytes.readInt16LE(25) ^ xor;
    header = 54;
    stride = 15;
    blockedAt = offset => Boolean(((bytes.readInt32LE(offset) ^ 0xaa38aa38) & 0x2000_0000) || ((bytes.readInt16LE(offset + 6) ^ xor) & 0x8000) || (bytes[offset + 8] & 0x7f));
  } else throw new Error("Unsupported client map format.");
  if (width <= 0 || height <= 0 || width * height > MAX_CELLS || bytes.length !== header + width * height * stride) {
    throw new Error("Invalid or truncated client map dimensions.");
  }
  const blocked = new Set();
  for (let x = 0; x < width; x += 1) {
    for (let y = 0; y < height; y += 1) {
      if (blockedAt(header + (x * height + y) * stride)) blocked.add(key(x, y));
    }
  }
  // Ordinary following must not accidentally trigger a doorway/map transfer.
  // The same exported content catalog is distributed to the Player Web client.
  const manifest = JSON.parse(await readFile(resolve(dirname(root), "crystal_respawn_manifest.json"), "utf8"));
  const map = manifest.maps?.find(value => value.map_file_name === mapFileName);
  if (!map) throw new Error("Client map transfer catalog is missing the requested map.");
  for (const movement of map.movements ?? []) {
    if (point(movement.source) && movement.source.x >= 0 && movement.source.x < width && movement.source.y >= 0 && movement.source.y < height) {
      blocked.add(key(movement.source.x, movement.source.y));
    }
  }
  return { mapFileName, width, height, blocked };
}

/** One ordinary Walk intent. Replan after the acknowledged authoritative position. */
export function nextWalkDirection({ player, target, desiredDistance = 1, collision, occupied = [] }) {
  if (!point(player) || !point(target) || !collision || !(collision.blocked instanceof Set) || !Number.isSafeInteger(desiredDistance) || desiredDistance < 0) return null;
  const inside = value => value.x >= 0 && value.y >= 0 && value.x < collision.width && value.y < collision.height;
  if (!inside(player) || !inside(target)) return null;
  const path = findCollisionGridPath({
    start: player,
    target,
    desiredDistance,
    bounds: { minX: 0, minY: 0, maxX: collision.width - 1, maxY: collision.height - 1 },
    blocked: [...collision.blocked],
    occupied,
  });
  if (!path || path.length < 2) return null;
  const dx = path[1].x - player.x;
  const dy = path[1].y - player.y;
  return new Map([
    ["0,-1", "Up"], ["1,-1", "UpRight"], ["1,0", "Right"], ["1,1", "DownRight"],
    ["0,1", "Down"], ["-1,1", "DownLeft"], ["-1,0", "Left"], ["-1,-1", "UpLeft"],
  ]).get(key(dx, dy)) ?? null;
}
