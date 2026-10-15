import type { EquipmentSlot } from "./types";

/** Original server presentation metadata, independent of item-instance authority. */
export type GatewayItemPresentationSource = {
  tooltipSource?: unknown;
  stateImage?: number | null;
  equipSlot?: EquipmentSlot | null;
  grade?: string;
  addedAttack?: number;
  addedDefence?: number;
};

export type ItemPresentationMetadata = Omit<GatewayItemPresentationSource, "stateImage"> & {
  stateImage?: number;
};

/**
 * Keep the original Crystal Info/UserItem payload intact. The Rust renderer
 * validates/deserializes it; Web must neither rebuild it from labels nor use
 * its nested unique_id as authority for commands. Missing metadata replaces
 * old metadata instead of inheriting it from the previous item in this cell.
 * This projection also participates in the movement-only snapshot comparison.
 */
export function projectItemPresentation(source: GatewayItemPresentationSource): ItemPresentationMetadata {
  return {
    tooltipSource: source.tooltipSource ?? undefined,
    stateImage: source.stateImage ?? undefined,
    equipSlot: source.equipSlot ?? undefined,
    grade: source.grade,
    addedAttack: source.addedAttack,
    addedDefence: source.addedDefence,
  };
}
