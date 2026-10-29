import { sameQuestWorldStamp, type QuestWorldStamp } from "./bevy-quest-world-context";

export type QuestWorldControlRect = Readonly<{ type: string; left: number; top: number; width: number; height: number }>;
export type QuestWorldControls = Readonly<{ version: 1; stamp: QuestWorldStamp;
  logicalWidth: number; logicalHeight: number; rects: readonly QuestWorldControlRect[] }>;
const object = (value: unknown): value is Record<string, unknown> => typeof value === "object" && value !== null && !Array.isArray(value);
const keys = (value: Record<string, unknown>, expected: readonly string[]) => Object.keys(value).sort().join() === [...expected].sort().join();
const types = new Set(["navigateQuestRoute", "attackTarget", "attackQuestTarget", "pickUpObject", "pickUpTile",
  "toggleSupplies", "selectSupplyVendor", "showSupplyInventory", "openDestinationMap"]);

/** Only actual current Bevy button rectangles may claim a world pointer. */
export function readQuestWorldControls(runtime: { getMir2QuestWorldControlRects?: () => string },
  expected: QuestWorldStamp, width: number, height: number, allowEmpty = false): QuestWorldControls | null {
  try {
    const value: unknown = JSON.parse(runtime.getMir2QuestWorldControlRects?.() ?? "null");
    if (!object(value) || !keys(value, ["version", "stamp", "logicalWidth", "logicalHeight", "rects"])
      || value.version !== 1 || !object(value.stamp)
      || !keys(value.stamp, ["generation", "revision", "connectionGeneration", "sessionGeneration", "sceneRevision", "playerObjectId", "mapFileName"])
      || !sameQuestWorldStamp(value.stamp, expected) || value.logicalWidth !== width || value.logicalHeight !== height
      || !Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width <= 0 || height <= 0
      || !Array.isArray(value.rects) || !allowEmpty && value.rects.length === 0 || value.rects.length > 128) return null;
    for (const rect of value.rects) {
      if (!object(rect) || !keys(rect, ["type", "left", "top", "width", "height"])
        || typeof rect.type !== "string" || !types.has(rect.type)
        || [rect.left, rect.top, rect.width, rect.height].some(n => typeof n !== "number" || !Number.isFinite(n))
        || Number(rect.left) < 0 || Number(rect.top) < 0 || Number(rect.width) <= 0 || Number(rect.height) <= 0
        || Number(rect.left) + Number(rect.width) > width + 1
        || Number(rect.top) + Number(rect.height) > height + 1) return null;
    }
    return value as unknown as QuestWorldControls;
  } catch { return null; }
}

export function questWorldControlAt(controls: QuestWorldControls | null, x: number, y: number): boolean {
  return Boolean(controls && Number.isFinite(x) && Number.isFinite(y)
    && controls.rects.some(rect => x >= rect.left && y >= rect.top && x < rect.left + rect.width && y < rect.top + rect.height));
}
