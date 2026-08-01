import type { DisplayEntity } from "./original-client-types";

export type CrystalSceneLightClassName = "dawn" | "evening" | "night";

export function crystalEffectiveLightSetting(
  mapLightSetting: number | null | undefined,
  timeOfDayLightSetting: number | null | undefined,
): number | null {
  return mapLightSetting && mapLightSetting >= 1 && mapLightSetting <= 4
    ? Math.trunc(mapLightSetting)
    : timeOfDayLightSetting && timeOfDayLightSetting >= 1 && timeOfDayLightSetting <= 4
      ? Math.trunc(timeOfDayLightSetting)
      : null;
}

export function crystalSceneDarknessColor(
  lightSetting: number | null | undefined,
  mapDarkLight: number | null | undefined,
): string | null {
  if (lightSetting === 1 || lightSetting === 3) return "rgb(50, 50, 50)";
  if (lightSetting !== 4) return null;
  switch (mapDarkLight) {
    case 1:
      return "rgb(20, 20, 20)";
    case 2:
      return "lightslategray";
    case 3:
      return "skyblue";
    case 4:
      return "goldenrod";
    default:
      return "rgb(0, 0, 0)";
  }
}

export type CrystalObjectLightSpec = {
  value: number;
  range: number;
  strengthBucket: number;
  width: number;
  height: number;
  placementWidth: number;
  placementHeight: number;
  opacity: number;
  tone: "neutral" | "merchant";
};

export type CrystalMapLightSpec = {
  value: number;
  range: number;
  width: number;
  height: number;
  placementWidth: number;
  placementHeight: number;
  opacity: number;
  tone: "neutral";
};

// DrawLights positions Lights[range] with LightSizes[range], but CreateLights
// built that texture from LightSizes[range + 1]. Preserve that native index
// mismatch: it is visible near the right and bottom edges of large lights.
const CRYSTAL_LIGHT_PLACEMENT_SIZES: ReadonlyArray<readonly [number, number]> = [
  [125, 95],
  [205, 156],
  [285, 217],
  [365, 277],
  [445, 338],
  [525, 399],
  [605, 460],
  [685, 521],
  [765, 581],
  [845, 642],
];

const CRYSTAL_LIGHT_TEXTURE_SIZES: ReadonlyArray<readonly [number, number]> = [
  [205, 156],
  [285, 217],
  [365, 277],
  [445, 338],
  [525, 399],
  [605, 460],
  [685, 521],
  [765, 581],
  [845, 642],
  [925, 703],
];

export function crystalLightTexturePath(range: number): string {
  if (!Number.isInteger(range) || range < 0 || range >= CRYSTAL_LIGHT_TEXTURE_SIZES.length) {
    throw new Error(`Crystal light texture range must be 0..${CRYSTAL_LIGHT_TEXTURE_SIZES.length - 1}.`);
  }
  return `/original-effects/Lighting/${range}.png`;
}

export function crystalSceneLightClassName(
  lightSetting: number | null | undefined,
): CrystalSceneLightClassName | null {
  switch (lightSetting) {
    case 1:
      return "dawn";
    case 3:
      return "evening";
    case 4:
      return "night";
    default:
      return null;
  }
}

export function crystalObjectLightSpec(
  entity: Pick<DisplayEntity, "kind" | "light" | "dead">,
  isSelf: boolean,
): CrystalObjectLightSpec | null {
  if (entity.dead && !isSelf) return null;

  const fallback = entity.kind === "npc" ? 10 : isSelf ? 3 : 0;
  const rawValue = entity.kind === "npc" ? 10 : entity.light ?? fallback;
  if (!Number.isFinite(rawValue) || rawValue <= 0) return null;

  const value = Math.trunc(rawValue);
  const range = Math.min(value % 15, CRYSTAL_LIGHT_TEXTURE_SIZES.length - 1);
  const [width, height] = CRYSTAL_LIGHT_TEXTURE_SIZES[range];
  const [placementWidth, placementHeight] = CRYSTAL_LIGHT_PLACEMENT_SIZES[range];
  const strengthBucket = Math.trunc(value / 15);
  const strength =
    entity.kind === "selfPlayer" || entity.kind === "player"
      ? [60, 120, 180, 240, 255][Math.min(strengthBucket, 4)]
      : entity.kind === "npc"
        ? 120
        : 255;

  return {
    value,
    range,
    strengthBucket,
    width,
    height,
    placementWidth,
    placementHeight,
    opacity: strength / 255,
    tone: entity.kind === "npc" ? "merchant" : "neutral",
  };
}

export function crystalMapLightSpec(lightValue: number): CrystalMapLightSpec | null {
  if (!Number.isFinite(lightValue)) return null;

  const value = Math.trunc(lightValue);
  // This deliberately mirrors the current Crystal DrawLights guard. Values
  // carrying the legacy colour bucket (10+) are skipped by the native client.
  if (value <= 0 || value >= 10) return null;

  const range = Math.min((value % 10) * 3, CRYSTAL_LIGHT_TEXTURE_SIZES.length - 1);
  const [width, height] = CRYSTAL_LIGHT_TEXTURE_SIZES[range];
  const [placementWidth, placementHeight] = CRYSTAL_LIGHT_PLACEMENT_SIZES[range];
  return {
    value,
    range,
    width,
    height,
    placementWidth,
    placementHeight,
    opacity: 1,
    tone: "neutral",
  };
}

export function crystalObjectLightTopLeft(
  drawX: number,
  drawY: number,
  spec: Pick<CrystalObjectLightSpec, "placementWidth" | "placementHeight">,
  cellWidth = 48,
  cellHeight = 32,
) {
  return {
    left: drawX - Math.floor(spec.placementWidth / 2) - Math.floor(cellWidth / 2),
    top: drawY - Math.floor(spec.placementHeight / 2) - Math.floor(cellHeight / 2) - 5,
  };
}

export function crystalMapLightTopLeft(
  drawX: number,
  drawY: number,
  offsetX: number,
  offsetY: number,
  spec: Pick<CrystalMapLightSpec, "placementWidth" | "placementHeight">,
  cellWidth = 48,
  cellHeight = 32,
) {
  return {
    left:
      drawX +
      offsetX -
      Math.floor(spec.placementWidth / 2) -
      Math.floor(cellWidth / 2) +
      10,
    top:
      drawY +
      cellHeight +
      offsetY -
      Math.floor(spec.placementHeight / 2) -
      Math.floor(cellHeight / 2) -
      5,
  };
}
