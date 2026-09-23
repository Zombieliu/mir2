import { access, readFile } from "node:fs/promises";
import path from "node:path";
import { gunzipSync } from "node:zlib";

const REPO_ROOT = path.resolve(import.meta.dirname, "..", "..", "..", "..");
const DEFAULT_PACKAGED_MAP_ROOT = path.join(
  REPO_ROOT,
  "apps",
  "web",
  "lib",
  "generated",
  "crystal-map-pack",
);
const DEFAULT_CLIENT_ROOT = path.resolve(REPO_ROOT, "..", "downloads", "crystal-client-full");
const MAX_MAP_CELLS = 4_000_000;

const STEPS = Object.freeze([
  Object.freeze({ dx: 0, dy: -1, direction: "up", cost: 10 }),
  Object.freeze({ dx: 1, dy: -1, direction: "up+right", cost: 14 }),
  Object.freeze({ dx: 1, dy: 0, direction: "right", cost: 10 }),
  Object.freeze({ dx: 1, dy: 1, direction: "down+right", cost: 14 }),
  Object.freeze({ dx: 0, dy: 1, direction: "down", cost: 10 }),
  Object.freeze({ dx: -1, dy: 1, direction: "down+left", cost: 14 }),
  Object.freeze({ dx: -1, dy: 0, direction: "left", cost: 10 }),
  Object.freeze({ dx: -1, dy: -1, direction: "up+left", cost: 14 }),
]);
const TRANSFER_EXIT_STEPS = Object.freeze([
  ...STEPS.filter(({ dx, dy }) => dx === 0 || dy === 0),
  ...STEPS.filter(({ dx, dy }) => dx !== 0 && dy !== 0),
]);

/** Normalize a Crystal map id without allowing it to escape a local map root. */
export function normalizeProtocolMapFileName(value) {
  const normalized = String(value ?? "").trim().replace(/\.map$/i, "");
  if (!normalized || !/^[a-z0-9_-]+$/i.test(normalized)) {
    throw new TypeError(`Invalid Crystal mapFileName: ${JSON.stringify(value)}`);
  }
  return normalized;
}

/**
 * Resolve a map from the checked-in/generated gzip pack first, then a Crystal
 * client Map directory. Resolution is read-only and performs no network I/O.
 */
export async function resolveProtocolMapSource(mapFileName, options = {}) {
  const normalized = normalizeProtocolMapFileName(mapFileName);
  const packagedMapRoot = path.resolve(options.packagedMapRoot ?? DEFAULT_PACKAGED_MAP_ROOT);
  const candidates = [
    { path: path.join(packagedMapRoot, `${encodeURIComponent(normalized.toLowerCase())}.map.gz`), gzip: true },
  ];

  const clientRoots = [options.clientRoot, process.env.CRYSTAL_CLIENT_ROOT, DEFAULT_CLIENT_ROOT]
    .filter(Boolean)
    .map((root) => path.resolve(root));
  for (const clientRoot of new Set(clientRoots)) {
    const mapRoot = path.basename(clientRoot).toLowerCase() === "map"
      ? clientRoot
      : path.join(clientRoot, "Map");
    candidates.push({ path: path.join(mapRoot, `${normalized}.map`), gzip: false });
    candidates.push({ path: path.join(mapRoot, normalized), gzip: false });
  }
  if (options.mapRoot) {
    const mapRoot = path.resolve(options.mapRoot);
    candidates.splice(1, 0,
      { path: path.join(mapRoot, `${normalized}.map`), gzip: false },
      { path: path.join(mapRoot, normalized), gzip: false },
    );
  }

  for (const candidate of candidates) {
    try {
      await access(candidate.path);
      return { ...candidate, mapFileName: normalized };
    } catch {
      // Try the next explicitly bounded local source.
    }
  }
  throw new Error(
    `No local Crystal map resource found for ${normalized}; searched ${candidates.map((entry) => entry.path).join(", ")}`,
  );
}

export async function loadProtocolCollisionMap(mapFileName, options = {}) {
  const source = await resolveProtocolMapSource(mapFileName, options);
  const stored = await readFile(source.path);
  const bytes = source.gzip ? gunzipSync(stored) : stored;
  return {
    ...parseCrystalCollisionMap(bytes, source.mapFileName, options),
    sourcePath: source.path,
  };
}

/**
 * Parse Crystal's map formats into a compact row-major collision bitmap.
 * Closed door cells are blocked by default, matching an initial server map.
 */
export function parseCrystalCollisionMap(bytes, mapFileName = "unknown", options = {}) {
  if (!Buffer.isBuffer(bytes)) throw new TypeError("Crystal map bytes must be a Buffer");
  const type = detectMapType(bytes);
  const { width, height } = mapDimensions(bytes, type);
  validateDimensions(width, height, bytes.length);
  const blocked = new Uint8Array(width * height);
  const doors = [];
  const doorsBlocked = options.doorsBlocked !== false;
  const mark = (x, y, highWall, lowWall, doorIndex = 0) => {
    const closedDoor = doorsBlocked && doorIndex > 0;
    if (highWall || lowWall || closedDoor) blocked[y * width + x] = 1;
    if (doorIndex > 0) doors.push({ x, y, index: doorIndex & 0x7f, closed: doorsBlocked });
  };

  if (type === 5) {
    let offset = 28 + 3 * (Math.floor(width / 2) + (width % 2)) * Math.floor(height / 2);
    requireCellBytes(bytes, offset, width, height, 14, type);
    forEachCell(width, height, (x, y) => {
      const flag = bytes.readUInt8(offset);
      mark(x, y, (flag & 0x01) !== 1, (flag & 0x02) !== 2);
      offset += 14;
    });
  } else {
    const layout = cellLayout(bytes, type);
    let offset = layout.offset;
    requireCellBytes(bytes, offset, width, height, layout.stride, type);
    forEachCell(width, height, (x, y) => {
      const flags = layout.flags(bytes, offset);
      mark(x, y, flags.highWall, flags.lowWall, flags.doorIndex);
      offset += layout.stride;
    });
  }

  return {
    mapFileName: normalizeProtocolMapFileName(mapFileName),
    type,
    width,
    height,
    bounds: Object.freeze({ minX: 0, minY: 0, maxX: width - 1, maxY: height - 1 }),
    blocked,
    doors,
  };
}

export function protocolMapCellIsWalkable(
  map,
  point,
  dynamicObstacles = [],
  staticWalkableOverrides = [],
) {
  const normalized = normalizePoint(point, "point");
  if (!inMap(map, normalized.x, normalized.y)) return false;
  const key = pointKey(normalized.x, normalized.y);
  if (map.blocked[normalized.y * map.width + normalized.x] &&
      !pointSet(staticWalkableOverrides).has(key)) return false;
  return !pointSet(dynamicObstacles).has(pointKey(normalized.x, normalized.y));
}

/**
 * Choose one normal unit step that leaves the current live transfer bounds.
 * The destination must pass ordinary static collision and live occupancy;
 * transfer-source wall exemptions are intentionally not used for stepping out.
 */
export function findProtocolTransferExitStep({ map, start, bounds, dynamicObstacles = [] }) {
  validateCollisionMap(map);
  const origin = normalizePoint(start, "start");
  const normalizedBounds = normalizeBounds(bounds);
  if (!pointInBounds(origin, normalizedBounds)) return null;

  for (const step of TRANSFER_EXIT_STEPS) {
    const target = { x: origin.x + step.dx, y: origin.y + step.dy };
    if (pointInBounds(target, normalizedBounds)) continue;
    const path = findProtocolWalkPath({
      map,
      start: origin,
      target,
      dynamicObstacles,
      maxExpanded: 9,
    });
    if (path?.length === 2) return target;
  }
  return null;
}

/**
 * A* over eight-direction unit Walk cells. Diagonal movement requires both
 * adjoining cardinal cells to be free, preventing movement through a corner.
 * Returns a path including start and reached endpoint, or null if unreachable.
 */
export function findProtocolWalkPath({
  map,
  start,
  target,
  desiredDistance = 0,
  dynamicObstacles = [],
  staticWalkableOverrides = [],
  maxExpanded = map?.width * map?.height,
}) {
  validateCollisionMap(map);
  const origin = normalizePoint(start, "start");
  const destination = normalizePoint(target, "target");
  const radius = Math.max(0, Math.trunc(Number(desiredDistance) || 0));
  if (!inMap(map, origin.x, origin.y) || !inMap(map, destination.x, destination.y)) return null;

  const occupied = pointSet(dynamicObstacles);
  occupied.delete(pointKey(origin.x, origin.y));
  const staticallyAllowed = pointSet(staticWalkableOverrides);
  const cellCount = map.width * map.height;
  const limit = Math.max(1, Math.min(cellCount, Math.trunc(Number(maxExpanded) || cellCount)));
  const startIndex = cellIndex(map, origin.x, origin.y);
  const gScore = new Float64Array(cellCount);
  gScore.fill(Number.POSITIVE_INFINITY);
  gScore[startIndex] = 0;
  const predecessor = new Int32Array(cellCount);
  predecessor.fill(-2);
  predecessor[startIndex] = -1;
  const closed = new Uint8Array(cellCount);
  const heap = new MinHeap();
  heap.push({ index: startIndex, score: octile(origin.x, origin.y, destination.x, destination.y), cost: 0 });
  let expanded = 0;

  while (heap.length && expanded < limit) {
    const currentNode = heap.pop();
    if (closed[currentNode.index] || currentNode.cost !== gScore[currentNode.index]) continue;
    closed[currentNode.index] = 1;
    expanded += 1;
    const current = indexPoint(map, currentNode.index);
    if (chebyshev(current, destination) <= radius) {
      return reconstructPath(map, predecessor, currentNode.index);
    }

    for (const step of STEPS) {
      const x = current.x + step.dx;
      const y = current.y + step.dy;
      if (!walkableAt(map, x, y, occupied, staticallyAllowed)) continue;
      const enteringAuthoritativeTransfer = staticallyAllowed.has(pointKey(x, y));
      if (!enteringAuthoritativeTransfer && step.dx !== 0 && step.dy !== 0 && (
        !walkableAt(map, current.x + step.dx, current.y, occupied, staticallyAllowed) ||
        !walkableAt(map, current.x, current.y + step.dy, occupied, staticallyAllowed)
      )) continue;
      const nextIndex = cellIndex(map, x, y);
      if (closed[nextIndex]) continue;
      const nextCost = currentNode.cost + step.cost;
      if (nextCost >= gScore[nextIndex]) continue;
      gScore[nextIndex] = nextCost;
      predecessor[nextIndex] = currentNode.index;
      heap.push({ index: nextIndex, cost: nextCost, score: nextCost + octile(x, y, destination.x, destination.y) });
    }
  }
  return null;
}

/** Convert a point path into auditable one-cell normal Walk intents. */
export function protocolWalkSteps(pathPoints) {
  if (!Array.isArray(pathPoints) || pathPoints.length === 0) return [];
  return pathPoints.slice(1).map((to, index) => {
    const from = normalizePoint(pathPoints[index], `path[${index}]`);
    const destination = normalizePoint(to, `path[${index + 1}]`);
    const dx = destination.x - from.x;
    const dy = destination.y - from.y;
    const step = STEPS.find((entry) => entry.dx === dx && entry.dy === dy);
    if (!step) throw new TypeError(`Path contains a non-unit Walk step: ${pointKey(from.x, from.y)} -> ${pointKey(destination.x, destination.y)}`);
    return { from, to: destination, dx, dy, direction: step.direction };
  });
}

export function planProtocolNavigation(input) {
  const pathPoints = findProtocolWalkPath(input);
  if (!pathPoints) return null;
  return { path: pathPoints, steps: protocolWalkSteps(pathPoints) };
}

function cellLayout(bytes, type) {
  switch (type) {
    case 0:
      return { offset: 52, stride: 12, flags: (b, o) => wallFlags(b.readInt16LE(o), b.readInt16LE(o + 4), b.readUInt8(o + 6), true) };
    case 1: {
      const xor = bytes.readInt16LE(23);
      return { offset: 54, stride: 15, flags: (b, o) => wallFlags(b.readInt32LE(o) ^ 0xaa38aa38, b.readInt16LE(o + 6) ^ xor, b.readUInt8(o + 8)) };
    }
    case 2:
    case 3:
      return { offset: 52, stride: type === 3 ? 36 : 14, flags: (b, o) => wallFlags(b.readInt16LE(o), b.readInt16LE(o + 4), b.readUInt8(o + 6), true) };
    case 4: {
      const xor = bytes.readInt16LE(33);
      return { offset: 64, stride: 12, flags: (b, o) => wallFlags(b.readInt16LE(o) ^ xor, b.readInt16LE(o + 4) ^ xor, b.readUInt8(o + 6), true) };
    }
    case 6:
      return { offset: 40, stride: 20, flags: (b, o) => { const flag = b.readUInt8(o); return { highWall: (flag & 1) !== 1, lowWall: (flag & 2) !== 2, doorIndex: 0 }; } };
    case 7:
      return { offset: 54, stride: 15, flags: (b, o) => wallFlags(b.readInt32LE(o), b.readInt16LE(o + 6), b.readUInt8(o + 8)) };
    case 100:
      return { offset: 8, stride: 26, flags: (b, o) => wallFlags(b.readInt32LE(o + 2), b.readInt16LE(o + 12), b.readUInt8(o + 14)) };
    default:
      throw new Error(`Unsupported Crystal map type ${type}`);
  }
}

function wallFlags(backImage, frontImage, rawDoorIndex, sixteenBitBackImage = false) {
  return {
    highWall: (backImage & 0x20000000) !== 0 || (sixteenBitBackImage && (backImage & 0x8000) !== 0),
    lowWall: (frontImage & 0x8000) !== 0,
    doorIndex: rawDoorIndex & 0x7f,
  };
}

function detectMapType(bytes) {
  if (bytes.length < 8) throw new Error("Crystal map is shorter than its header");
  if (bytes[2] === 0x43 && bytes[3] === 0x23) return 100;
  if (bytes[0] === 0) return 5;
  if (bytes[0] === 0x0f && bytes[5] === 0x53 && bytes[14] === 0x33) return 6;
  if (bytes[0] === 0x15 && bytes[4] === 0x32 && bytes[6] === 0x41 && bytes[19] === 0x31) return 4;
  if (bytes[0] === 0x10 && bytes[2] === 0x61 && bytes[7] === 0x31 && bytes[14] === 0x31) return 1;
  if (bytes[4] === 0x0f || (bytes[4] === 0x03 && bytes[18] === 0x0d && bytes[19] === 0x0a)) {
    const width = bytes.readUInt16LE(0);
    const height = bytes.readUInt16LE(2);
    return bytes.length > 52 + width * height * 14 ? 3 : 2;
  }
  if (bytes[0] === 0x0d && bytes[1] === 0x4c && bytes[7] === 0x20 && bytes[11] === 0x6d) return 7;
  return 0;
}

function mapDimensions(bytes, type) {
  if (type === 100) return { width: bytes.readInt16LE(4), height: bytes.readInt16LE(6) };
  if (type === 1) return { width: bytes.readInt16LE(21) ^ bytes.readInt16LE(23), height: bytes.readInt16LE(25) ^ bytes.readInt16LE(23) };
  if (type === 4) return { width: bytes.readInt16LE(31) ^ bytes.readInt16LE(33), height: bytes.readInt16LE(35) ^ bytes.readInt16LE(33) };
  if (type === 5) return { width: bytes.readInt16LE(22), height: bytes.readInt16LE(24) };
  if (type === 6) return { width: bytes.readInt16LE(16), height: bytes.readInt16LE(18) };
  if (type === 7) return { width: bytes.readInt16LE(21), height: bytes.readInt16LE(25) };
  return { width: bytes.readInt16LE(0), height: bytes.readInt16LE(2) };
}

function validateDimensions(width, height, byteLength) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0 || width * height > MAX_MAP_CELLS) {
    throw new Error(`Invalid Crystal map dimensions ${width}x${height} (${byteLength} bytes)`);
  }
}

function requireCellBytes(bytes, offset, width, height, stride, type) {
  const required = offset + width * height * stride;
  if (required > bytes.length) throw new Error(`Truncated Crystal type ${type} map: requires ${required} bytes, received ${bytes.length}`);
}

function forEachCell(width, height, visit) {
  for (let x = 0; x < width; x += 1) for (let y = 0; y < height; y += 1) visit(x, y);
}

function validateCollisionMap(map) {
  if (!map || !Number.isInteger(map.width) || !Number.isInteger(map.height) || !(map.blocked instanceof Uint8Array) || map.blocked.length !== map.width * map.height) {
    throw new TypeError("map must be a parsed protocol collision map");
  }
}

function normalizePoint(value, label) {
  const x = Number(value?.x);
  const y = Number(value?.y);
  if (!Number.isInteger(x) || !Number.isInteger(y)) throw new TypeError(`${label} must contain integer x and y`);
  return { x, y };
}

function normalizeBounds(value) {
  const bounds = {
    minX: Number(value?.minX),
    maxX: Number(value?.maxX),
    minY: Number(value?.minY),
    maxY: Number(value?.maxY),
  };
  if (!Object.values(bounds).every(Number.isInteger) ||
      bounds.minX > bounds.maxX || bounds.minY > bounds.maxY) {
    throw new TypeError("bounds must contain ordered integer minX, maxX, minY, and maxY");
  }
  return bounds;
}

function pointInBounds(point, bounds) {
  return point.x >= bounds.minX && point.x <= bounds.maxX &&
    point.y >= bounds.minY && point.y <= bounds.maxY;
}

function pointSet(points) {
  const result = new Set();
  for (const value of points ?? []) {
    if (typeof value === "string" && /^-?\d+,-?\d+$/.test(value)) result.add(value);
    else {
      const point = normalizePoint(value, "dynamic obstacle");
      result.add(pointKey(point.x, point.y));
    }
  }
  return result;
}

function walkableAt(map, x, y, occupied, staticallyAllowed) {
  const key = pointKey(x, y);
  return inMap(map, x, y) &&
    (!map.blocked[cellIndex(map, x, y)] || staticallyAllowed.has(key)) &&
    !occupied.has(key);
}

function inMap(map, x, y) {
  return x >= 0 && y >= 0 && x < map.width && y < map.height;
}

function cellIndex(map, x, y) {
  return y * map.width + x;
}

function indexPoint(map, index) {
  return { x: index % map.width, y: Math.floor(index / map.width) };
}

function pointKey(x, y) {
  return `${x},${y}`;
}

function chebyshev(left, right) {
  return Math.max(Math.abs(left.x - right.x), Math.abs(left.y - right.y));
}

function octile(x, y, targetX, targetY) {
  const dx = Math.abs(targetX - x);
  const dy = Math.abs(targetY - y);
  return 10 * Math.max(dx, dy) + 4 * Math.min(dx, dy);
}

function reconstructPath(map, predecessor, endIndex) {
  const reversed = [];
  for (let index = endIndex; index >= 0; index = predecessor[index]) reversed.push(indexPoint(map, index));
  return reversed.reverse();
}

class MinHeap {
  #values = [];
  get length() { return this.#values.length; }
  push(value) {
    this.#values.push(value);
    let index = this.#values.length - 1;
    while (index > 0) {
      const parent = Math.floor((index - 1) / 2);
      if (!heapLess(value, this.#values[parent])) break;
      this.#values[index] = this.#values[parent];
      index = parent;
    }
    this.#values[index] = value;
  }
  pop() {
    const first = this.#values[0];
    const last = this.#values.pop();
    if (this.#values.length && last) {
      let index = 0;
      while (true) {
        const left = index * 2 + 1;
        const right = left + 1;
        if (left >= this.#values.length) break;
        let child = left;
        if (right < this.#values.length && heapLess(this.#values[right], this.#values[left])) child = right;
        if (!heapLess(this.#values[child], last)) break;
        this.#values[index] = this.#values[child];
        index = child;
      }
      this.#values[index] = last;
    }
    return first;
  }
}

function heapLess(left, right) {
  return left.score < right.score || (left.score === right.score && (left.cost < right.cost || (left.cost === right.cost && left.index < right.index)));
}
