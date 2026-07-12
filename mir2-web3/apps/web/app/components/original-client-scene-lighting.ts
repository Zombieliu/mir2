import type { DisplayEntity } from "./original-client-types";

export type CrystalSceneLightClassName = "dawn" | "evening" | "night";

export type CrystalObjectLightSpec = {
  value: number;
  range: number;
  strengthBucket: number;
  width: number;
  height: number;
  opacity: number;
  tone: "neutral" | "merchant";
};

// Crystal DXManager.LightSizes. The client creates ten textures and clamps the
// decoded `light % 15` index to the final available texture.
const CRYSTAL_LIGHT_SIZES: ReadonlyArray<readonly [number, number]> = [
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

// Crystal adds grayscale light into an offscreen mask and multiplies the scene.
// CSS `screen` is only the temporary bridge, so feeding it the raw 0..255 mask
// energy over-brightens overlapping merchant lights. This factor matches the
// single-source reveal without turning dense NPC scenes white.
const CSS_SCREEN_LIGHT_ENERGY = 0.28;

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
  const range = Math.min(value % 15, CRYSTAL_LIGHT_SIZES.length - 1);
  const [width, height] = CRYSTAL_LIGHT_SIZES[range];
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
    opacity: (strength / 255) * CSS_SCREEN_LIGHT_ENERGY,
    tone: entity.kind === "npc" ? "merchant" : "neutral",
  };
}
