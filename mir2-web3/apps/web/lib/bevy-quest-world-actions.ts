import { questCoordinate, questMapIndex, sameQuestWorldStamp, type QuestWorldStamp } from "./bevy-quest-world-context";

export type QuestRouteTarget =
  | Readonly<{ type: "entrance" }>
  | Readonly<{ type: "huntRegion"; monsterIndex: number; radius: number }>
  | Readonly<{ type: "supply"; vendor: "potions" | "general" | "poison" }>;

export type QuestWorldAction = QuestWorldStamp & (
  | Readonly<{ type: "openDestinationMap" }>
  | Readonly<{ type: "navigateQuestRoute"; questIndex: number; resetEpoch: number;
      mapIndex: number; x: number; y: number; routeTarget: QuestRouteTarget }>
  | Readonly<{ type: "attackTarget" | "attackQuestTarget"; objectId: number }>
  | Readonly<{ type: "pickUpObject"; objectId: number; x: number; y: number }>
  | Readonly<{ type: "pickUpTile"; x: number; y: number }>
);
export type QuestRouteAction = Extract<QuestWorldAction, { type: "navigateQuestRoute" }>;

const stampKeys = ["generation", "revision", "connectionGeneration", "sessionGeneration",
  "sceneRevision", "playerObjectId", "mapFileName"] as const;
const actionTypes = new Set(["openDestinationMap", "navigateQuestRoute", "attackTarget",
  "attackQuestTarget", "pickUpObject", "pickUpTile"]);
const object = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const positive = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value > 0;
const objectId = (value: unknown): value is number => positive(value) && value <= 0xffff_ffff;
const keys = (value: Record<string, unknown>, expected: readonly string[]) =>
  Object.keys(value).sort().join() === [...expected].sort().join();

export function isQuestWorldAction(type: unknown): boolean {
  return typeof type === "string" && actionTypes.has(type);
}

/** A local host action, never a Gateway command or a combat send proof. */
export function parseQuestWorldAction(value: unknown): QuestWorldAction | null {
  if (!object(value) || !isQuestWorldAction(value.type)) return null;
  const stamp = Object.fromEntries(stampKeys.map(key => [key, value[key]])) as QuestWorldStamp;
  if (!sameQuestWorldStamp(stamp, stamp)) return null;
  let fields: readonly string[];
  if (value.type === "navigateQuestRoute") {
    fields = ["questIndex", "resetEpoch", "mapIndex", "x", "y", "routeTarget"];
    if (!questCoordinate(value.questIndex) || !positive(value.resetEpoch)
      || value.resetEpoch !== stamp.sceneRevision || !questMapIndex(value.mapIndex)
      || !questCoordinate(value.x) || !questCoordinate(value.y) || !object(value.routeTarget)) return null;
    const target = value.routeTarget;
    if (target.type === "entrance") {
      if (!keys(target, ["type"])) return null;
    } else if (target.type === "huntRegion") {
      if (!keys(target, ["type", "monsterIndex", "radius"]) || !questCoordinate(target.monsterIndex)
        || !questCoordinate(target.radius) || target.radius < 0 || target.radius > 0xffff) return null;
    } else if (target.type === "supply") {
      if (!keys(target, ["type", "vendor"]) || !["potions", "general", "poison"].includes(String(target.vendor))) return null;
    } else return null;
  } else if (value.type === "attackTarget" || value.type === "attackQuestTarget") {
    fields = ["objectId"];
    if (!objectId(value.objectId)) return null;
  } else if (value.type === "pickUpObject") {
    fields = ["objectId", "x", "y"];
    if (!objectId(value.objectId) || !questCoordinate(value.x) || !questCoordinate(value.y)) return null;
  } else if (value.type === "pickUpTile") {
    fields = ["x", "y"];
    if (!questCoordinate(value.x) || !questCoordinate(value.y)) return null;
  } else fields = [];
  return keys(value, [...stampKeys, "type", ...fields]) ? value as QuestWorldAction : null;
}
