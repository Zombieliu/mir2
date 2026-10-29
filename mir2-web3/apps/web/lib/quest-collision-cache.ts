/** Complete static geometry for the shared Quest route query. No fallback cells. */
export type QuestCollisionGeometry = Readonly<{
  mapFileName: string;
  width: number;
  height: number;
  fingerprint: string;
  blockedBits: Uint8Array;
}>;

type Chunk = {
  schemaVersion: 1;
  source: "crystalMap";
  mapFileName: string;
  mapWidth: number;
  mapHeight: number;
  geometryFingerprint: string;
  bounds: { minX: number; maxX: number; minY: number; maxY: number };
  blockedCells: Array<{ x: number; y: number }>;
};
type Bounds = Chunk["bounds"];
type Fetcher = (url: string) => Promise<Pick<Response, "ok" | "json">>;
const SPAN = 256;
const MAX_CELLS = 16_777_216;
const object = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const size = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value > 0 && value <= 0xffff;
const coordinate = (value: unknown): value is number =>
  typeof value === "number" && Number.isInteger(value) && value >= 0 && value < 0xffff;

function validFile(file: string): boolean {
  return file.length <= 128 && /^[a-z0-9][a-z0-9_.-]*$/.test(file)
    && !file.includes("..") && !file.endsWith(".map");
}

function readChunk(value: unknown, file: string, requested: Bounds, expected?: Chunk): Chunk {
  if (!object(value) || value.schemaVersion !== 1 || value.source !== "crystalMap"
    || value.mapFileName !== file || !size(value.mapWidth) || !size(value.mapHeight)
    || value.mapWidth * value.mapHeight > MAX_CELLS
    || typeof value.geometryFingerprint !== "string" || !/^[a-f0-9]{64}$/.test(value.geometryFingerprint)
    || !object(value.bounds) || !Array.isArray(value.blockedCells)) throw new Error("Quest map collision is unavailable.");
  const bounds = value.bounds;
  const actual: Bounds = { minX: requested.minX, minY: requested.minY,
    maxX: Math.min(requested.maxX, value.mapWidth - 1), maxY: Math.min(requested.maxY, value.mapHeight - 1) };
  if (!coordinate(bounds.minX) || !coordinate(bounds.minY) || !coordinate(bounds.maxX) || !coordinate(bounds.maxY)
    || actual.maxX < actual.minX || actual.maxY < actual.minY
    || (Object.keys(actual) as Array<keyof Bounds>).some(key => bounds[key] !== actual[key])
    || value.blockedCells.length > (actual.maxX - actual.minX + 1) * (actual.maxY - actual.minY + 1)
    || expected && (value.mapWidth !== expected.mapWidth || value.mapHeight !== expected.mapHeight
      || value.geometryFingerprint !== expected.geometryFingerprint)) throw new Error("Quest map collision changed while loading.");
  const seen = new Set<number>();
  for (const cell of value.blockedCells) {
    if (!object(cell) || !coordinate(cell.x) || !coordinate(cell.y)
      || cell.x < actual.minX || cell.x > actual.maxX || cell.y < actual.minY || cell.y > actual.maxY)
      throw new Error("Quest map collision is incomplete.");
    const key = cell.x * value.mapHeight + cell.y;
    if (seen.has(key)) throw new Error("Quest map collision contains duplicate cells.");
    seen.add(key);
  }
  return value as Chunk;
}

export class QuestCollisionCache {
  private complete = new Map<string, QuestCollisionGeometry>();
  private pending = new Map<string, Promise<QuestCollisionGeometry>>();
  constructor(private readonly fetcher: Fetcher = async url => fetch(url, { cache: "no-cache" })) {}

  /** Cached geometry cannot carry or resume a player action. The Page owns that lifetime. */
  load(mapFileName: string): Promise<QuestCollisionGeometry> {
    if (!validFile(mapFileName)) return Promise.reject(new Error("The quest map filename is invalid."));
    const cached = this.complete.get(mapFileName);
    if (cached) return Promise.resolve(cached);
    const pending = this.pending.get(mapFileName);
    if (pending) return pending;
    const next = this.collect(mapFileName).then(geometry => {
      this.complete.set(mapFileName, geometry);
      while (this.complete.size > 2) this.complete.delete(this.complete.keys().next().value!);
      return geometry;
    }).finally(() => { if (this.pending.get(mapFileName) === next) this.pending.delete(mapFileName); });
    this.pending.set(mapFileName, next);
    return next;
  }

  private async chunk(file: string, bounds: Bounds, expected?: Chunk): Promise<Chunk> {
    const query = new URLSearchParams({ map: file, quest: "1", ...Object.fromEntries(
      Object.entries(bounds).map(([key, value]) => [key, String(value)])) });
    const response = await this.fetcher(`/api/scene/collision?${query}`);
    if (!response.ok) throw new Error("Quest map collision is unavailable.");
    return readChunk(await response.json(), file, bounds, expected);
  }

  private async collect(file: string): Promise<QuestCollisionGeometry> {
    const first = await this.chunk(file, { minX: 0, maxX: SPAN - 1, minY: 0, maxY: SPAN - 1 });
    const bits = new Uint8Array(Math.ceil(first.mapWidth * first.mapHeight / 8));
    const apply = (chunk: Chunk) => {
      for (const cell of chunk.blockedCells) {
        const index = cell.x * first.mapHeight + cell.y;
        bits[index >>> 3] |= 1 << (index & 7);
      }
    };
    apply(first);
    // Each region is requested exactly once. Unknown regions are never filled as walkable.
    for (let minX = 0; minX < first.mapWidth; minX += SPAN) {
      for (let minY = 0; minY < first.mapHeight; minY += SPAN) {
        if (minX === 0 && minY === 0) continue;
        apply(await this.chunk(file, { minX, minY, maxX: minX + SPAN - 1, maxY: minY + SPAN - 1 }, first));
      }
    }
    return { mapFileName: file, width: first.mapWidth, height: first.mapHeight,
      fingerprint: first.geometryFingerprint, blockedBits: bits };
  }
}
