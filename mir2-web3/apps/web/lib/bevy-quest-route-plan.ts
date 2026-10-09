import { questCoordinate, sameQuestWorldStamp, type QuestWorldStamp } from "./bevy-quest-world-context";
import type { QuestRouteAction } from "./bevy-quest-world-actions";
import type { QuestCollisionGeometry } from "./quest-collision-cache";

export type QuestRoutePoint = Readonly<{ x: number; y: number }>;
export type QuestRouteEdge = Readonly<{ from: QuestRoutePoint; to: QuestRoutePoint }>;
export type QuestRouteRuntime = {
  getMir2QuestRoutePlan?: (queryJson: string, blockedBits: Uint8Array) => string;
};
export type QuestRoutePlan = Readonly<{
  origin: QuestRoutePoint;
  destination: QuestRoutePoint;
  steps: readonly QuestRoutePoint[];
  radius: number;
}>;
const object = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const samePoint = (a: QuestRoutePoint, b: QuestRoutePoint) => a.x === b.x && a.y === b.y;
const distance = (a: QuestRoutePoint, b: QuestRoutePoint) => Math.max(Math.abs(a.x - b.x), Math.abs(a.y - b.y));

/** Only the common Rust query decides an endpoint or complete route. This adapter verifies its output. */
export function planBevyQuestRoute(
  runtime: QuestRouteRuntime,
  action: QuestRouteAction,
  geometry: QuestCollisionGeometry,
  origin: QuestRoutePoint,
  occupiedTiles: readonly QuestRoutePoint[],
  rejectedEdges: readonly QuestRouteEdge[],
): QuestRoutePlan | null {
  if (!runtime.getMir2QuestRoutePlan || geometry.mapFileName !== action.mapFileName
    || !Number.isInteger(geometry.width) || !Number.isInteger(geometry.height)
    || geometry.width <= 0 || geometry.height <= 0 || geometry.width > 65535 || geometry.height > 65535
    || geometry.width * geometry.height > 16_777_216
    || geometry.blockedBits.length !== Math.ceil(geometry.width * geometry.height / 8)) return null;
  const inside = (point: unknown): point is QuestRoutePoint => object(point)
    && questCoordinate(point.x) && questCoordinate(point.y)
    && point.x >= 0 && point.x < geometry.width && point.y >= 0 && point.y < geometry.height;
  if (!inside(origin)) return null;
  const { generation, revision, connectionGeneration, sessionGeneration, sceneRevision, playerObjectId, mapFileName } = action;
  const stamp: QuestWorldStamp = { generation, revision, connectionGeneration, sessionGeneration, sceneRevision, playerObjectId, mapFileName };
  const query = { version: 1, stamp, mapIndex: action.mapIndex, mapFileName,
    width: geometry.width, height: geometry.height, origin, destination: { x: action.x, y: action.y },
    routeTarget: action.routeTarget, occupiedTiles, rejectedEdges };
  try {
    const result: unknown = JSON.parse(runtime.getMir2QuestRoutePlan(JSON.stringify(query), geometry.blockedBits));
    if (!object(result) || result.version !== 1 || result.ok !== true
      || result.mapIndex !== action.mapIndex || result.mapFileName !== mapFileName
      || !object(result.stamp) || !sameQuestWorldStamp(result.stamp, stamp)
      || !inside(result.origin) || !samePoint(result.origin, origin)
      || !inside(result.destination) || !Array.isArray(result.steps) || result.steps.length > 250_000
      || !questCoordinate(result.radius) || result.radius < 0 || result.radius > Math.max(geometry.width, geometry.height)) return null;
    if (action.routeTarget.type === "entrance" && result.radius !== 0
      || action.routeTarget.type === "huntRegion" && result.radius !== action.routeTarget.radius
      || action.routeTarget.type === "supply" && result.radius !== 0 && result.radius !== 2) return null;
    let previous = origin;
    for (const step of result.steps) {
      if (!inside(step) || distance(previous, step) !== 1) return null;
      const bit = step.x * geometry.height + step.y;
      if (geometry.blockedBits[bit >>> 3] & 1 << (bit & 7)
        || occupiedTiles.some(tile => samePoint(tile, step))
        || rejectedEdges.some(edge => samePoint(edge.from, previous) && samePoint(edge.to, step))) return null;
      previous = step;
    }
    if (!samePoint(previous, result.destination)
      || distance(result.destination, { x: action.x, y: action.y }) > result.radius) return null;
    if (result.steps.length === 0) {
      const bit = origin.x * geometry.height + origin.y;
      if (geometry.blockedBits[bit >>> 3] & 1 << (bit & 7)
        || occupiedTiles.some(tile => samePoint(tile, origin))) return null;
    }
    return { origin: { ...origin }, destination: { ...result.destination },
      steps: result.steps.map(step => ({ x: Number(step.x), y: Number(step.y) })), radius: result.radius };
  } catch { return null; }
}
