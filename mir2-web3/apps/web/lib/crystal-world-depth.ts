/** Crystal DrawObjects paints the complete map row before that row's objects.
 * Keep the same raw units as the native producer, before the renderer's
 * /100000 world-Z conversion. Camera-relative coordinates retain f32 precision
 * for the one-unit body/hair/weapon differences even near the u16 map edge.
 */
export const CRYSTAL_MAP_DEPTH_GAIN = 10;
const ROW_STRIDE = 12_000;
const OBJECT_PASS = 6_000;
const COLUMN_STRIDE = 20;
const RELATIVE_LIMIT = 128;

export type CrystalDepthPoint = Readonly<{ x: number; y: number }>;

type CrystalDepthMotion = Readonly<{
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  sortX?: number;
  sortY?: number;
  startedAt: number;
  expiresAt: number;
  animationState: string;
}>;

export const CRYSTAL_ENTITY_LAYER_ORDER: Readonly<Record<string, number>> = {
  ground: 1,
  mount: 2,
  rearWeapon: 4,
  body: 5,
  hair: 6,
  frontWeapon: 7,
};

function relativeCell(value: number, center: number) {
  const coordinate = Number.isFinite(value) ? Math.trunc(value) : 0;
  const anchor = Number.isFinite(center) ? Math.trunc(center) : 0;
  return Math.max(-RELATIVE_LIMIT, Math.min(RELATIVE_LIMIT, coordinate - anchor));
}

function rowColumn(x: number, y: number, center: CrystalDepthPoint) {
  return (relativeCell(y, center.y) + RELATIVE_LIMIT) * ROW_STRIDE
    + (relativeCell(x, center.x) + RELATIVE_LIMIT) * COLUMN_STRIDE;
}

export function crystalObjectDepth(x: number, y: number, center: CrystalDepthPoint) {
  return rowColumn(x, y, center) + OBJECT_PASS;
}

export function crystalMapDepth(
  x: number,
  y: number,
  center: CrystalDepthPoint,
  layer = 0,
) {
  return rowColumn(x, y, center) + layer * CRYSTAL_MAP_DEPTH_GAIN;
}

export function crystalEntityLayerDepth(sortZ: number, role: string) {
  return sortZ + (CRYSTAL_ENTITY_LAYER_ORDER[role] ?? 0);
}

/** Pixel interpolation and sorting have separate anchors. A new segment may
 * start mid-glide, but it remains sorted at its authoritative source tile until
 * the segment ends. Never use an old/different target's motion for a new actor.
 */
export function crystalEntityDepth(
  entity: CrystalDepthPoint & Readonly<{ dead?: boolean; kind?: string }>,
  center: CrystalDepthPoint,
  motion: CrystalDepthMotion | null | undefined,
  now: number,
) {
  let motionSortX = entity.x;
  let motionSortY = entity.y;
  if (!entity.dead && entity.kind !== "npc" && motion
    && (motion.animationState === "walking" || motion.animationState === "running")
    && motion.toX === entity.x && motion.toY === entity.y
    && Number.isFinite(now) && Number.isFinite(motion.startedAt)
    && Number.isFinite(motion.expiresAt) && motion.expiresAt > motion.startedAt
    && now < motion.expiresAt) {
    const sourceX = motion.sortX ?? motion.fromX;
    const sourceY = motion.sortY ?? motion.fromY;
    if (Number.isInteger(sourceX) && Number.isInteger(sourceY)) {
      motionSortX = sourceX;
      motionSortY = sourceY;
    }
  }
  return {
    sortZ: crystalObjectDepth(motionSortX, motionSortY, center),
    motionSortX,
    motionSortY,
  };
}
